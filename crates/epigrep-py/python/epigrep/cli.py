"""epigrep command-line interface — grep for event sequences.

Subcommands mirror the stable Python entrypoints so the CLI teaches the same
model:

  * ``epigrep match``   — run a pattern, print matching spans + captures
  * ``epigrep explain`` — print near-miss explanations for non-matching starts
  * ``epigrep schema``  — summarise what event types / attributes / partitions
                          a stream contains (the "what can I even query" step)

Data is read as JSONL by default: one event object per line, ``{"partition",
"ts", "typ", "attrs"}`` (``attrs`` optional) — the same shape as the bundled
examples. CSV and parquet are also supported (``--input-format``, or inferred
from the file extension): their flat rows are mapped to events by the
``--partition-col`` / ``--ts-col`` / ``--type-col`` / ``--attr-cols`` options,
reusing the same eventise primitive as the Python API. Pass a path, ``-``, or
nothing to read from stdin (parquet requires a file path).

Patterns come from ``--pattern-json FILE`` (the stable JSON AST) or, marked
experimental, ``--pattern TEXT`` (the provisional text DSL).

Exit codes follow grep so ``epigrep match ... && ...`` works as muscle memory
expects: ``0`` = at least one match, ``1`` = no match, ``2`` = error. ``explain``
and ``schema`` return ``0`` on success and ``2`` on error.
"""

from __future__ import annotations

import argparse
import csv
import io
import json
import sys
from pathlib import Path
from typing import List, Optional, Sequence

from ._core import Event, parse_pattern, pattern_from_json
from . import explain as run_explain
from . import match as run_match
from . import near_miss_summary, schema as run_schema
from .eventise import eventise, events_from_frame

EXIT_OK = 0
EXIT_NO_MATCH = 1
EXIT_ERROR = 2


class CliError(Exception):
    """A user-facing error: reported to stderr, exits with code 2."""


def _read_text(data_arg: Optional[str]) -> str:
    if data_arg in (None, "-"):
        return sys.stdin.read()
    try:
        return Path(data_arg).read_text()
    except OSError as exc:
        raise CliError(f"cannot read {data_arg}: {exc}") from exc


def _resolve_format(data_arg: Optional[str], requested: str) -> str:
    """Pick the input format: an explicit choice, or inferred from extension."""
    if requested != "auto":
        return requested
    if data_arg and data_arg != "-":
        suffix = Path(data_arg).suffix.lower()
        if suffix == ".csv":
            return "csv"
        if suffix in (".parquet", ".pq"):
            return "parquet"
    return "jsonl"


def _attr_cols(args: argparse.Namespace) -> Optional[List[str]]:
    if not args.attr_cols:
        return None
    return [name.strip() for name in args.attr_cols.split(",") if name.strip()]


def _require_columns(present, args: argparse.Namespace, kind: str) -> None:
    for column in (args.partition_col, args.ts_col, args.type_col):
        if column not in present:
            raise CliError(
                f"{kind} is missing required column {column!r}; "
                f"columns present: {list(present)}"
            )


def _read_jsonl_events(text: str) -> List[Event]:
    events: List[Event] = []
    for lineno, line in enumerate(text.splitlines(), start=1):
        stripped = line.strip()
        if not stripped:
            continue
        try:
            record = json.loads(stripped)
        except json.JSONDecodeError as exc:
            raise CliError(f"line {lineno}: invalid JSON: {exc}") from exc
        if not isinstance(record, dict):
            raise CliError(f"line {lineno}: each line must be a JSON object")
        for field in ("partition", "ts", "typ"):
            if field not in record:
                raise CliError(f"line {lineno}: missing required field {field!r}")
        try:
            events.append(
                Event(
                    record["partition"],
                    record["ts"],
                    record["typ"],
                    record.get("attrs", {}),
                )
            )
        except (TypeError, ValueError) as exc:
            raise CliError(f"line {lineno}: {exc}") from exc
    return events


def _read_csv_events(text: str, args: argparse.Namespace) -> List[Event]:
    reader = csv.DictReader(io.StringIO(text))
    if reader.fieldnames is None:
        return []
    _require_columns(reader.fieldnames, args, "CSV")
    try:
        return eventise(
            list(reader),
            partition=args.partition_col,
            ts=args.ts_col,
            typ=args.type_col,
            attrs=_attr_cols(args),
        )
    except (TypeError, ValueError) as exc:
        raise CliError(f"CSV: {exc}") from exc


def _read_parquet_events(data_arg: Optional[str], args: argparse.Namespace) -> List[Event]:
    if data_arg in (None, "-"):
        raise CliError("parquet input must be a file path, not stdin")
    try:
        import pyarrow.parquet as pq
    except ImportError as exc:
        raise CliError("reading parquet requires pyarrow (pip install pyarrow)") from exc
    try:
        table = pq.read_table(data_arg)
    except Exception as exc:  # pyarrow raises a variety of types
        raise CliError(f"cannot read parquet {data_arg}: {exc}") from exc
    _require_columns(table.column_names, args, "parquet")
    try:
        return events_from_frame(
            table,
            partition_col=args.partition_col,
            ts_col=args.ts_col,
            type_col=args.type_col,
            attr_cols=_attr_cols(args),
        )
    except (TypeError, ValueError) as exc:
        raise CliError(f"parquet: {exc}") from exc


def _read_events(data_arg: Optional[str], args: argparse.Namespace) -> List[Event]:
    fmt = _resolve_format(data_arg, args.input_format)
    if fmt == "jsonl":
        return _read_jsonl_events(_read_text(data_arg))
    if fmt == "csv":
        return _read_csv_events(_read_text(data_arg), args)
    if fmt == "parquet":
        return _read_parquet_events(data_arg, args)
    raise CliError(f"unknown input format: {fmt}")  # pragma: no cover


def _load_pattern(args: argparse.Namespace):
    try:
        if args.pattern_json:
            text = Path(args.pattern_json).read_text()
            return pattern_from_json(text)
        return parse_pattern(args.pattern)
    except OSError as exc:
        raise CliError(f"cannot read pattern: {exc}") from exc
    except ValueError as exc:
        raise CliError(f"invalid pattern: {exc}") from exc


def _match_rows(matches) -> List[dict]:
    return [
        {
            "partition": m.partition,
            "start": m.start,
            "end": m.end,
            "indices": list(m.indices),
            "types": list(m.types),
            "captures": dict(m.captures),
        }
        for m in matches
    ]


def _near_miss_rows(misses) -> List[dict]:
    return [
        {
            "partition": miss.partition,
            "start_index": miss.start_index,
            "indices": list(miss.indices),
            "reached_steps": miss.reached_steps,
            "next_event_type": miss.next_event_type,
            "reason": miss.reason,
            "summary": near_miss_summary(miss),
        }
        for miss in misses
    ]


def _emit(rows, fmt: str, table_fn) -> None:
    if fmt == "json":
        json.dump(rows, sys.stdout, indent=2, default=str)
        sys.stdout.write("\n")
    else:
        table_fn(rows)


def _print_match_table(rows: List[dict]) -> None:
    if not rows:
        print("(no matches)")
        return
    for row in rows:
        types = " -> ".join(row["types"])
        line = f"{row['partition']}  [{row['start']}..{row['end']}]  {types}"
        if row["captures"]:
            caps = ", ".join(f"{k}={v!r}" for k, v in row["captures"].items())
            line += f"  ({caps})"
        print(line)


def _print_near_miss_table(rows: List[dict]) -> None:
    if not rows:
        print("(no near-misses)")
        return
    for row in rows:
        print(f"{row['partition']}  {row['summary']}")


def _print_schema_table(schema: dict) -> None:
    print(f"events:     {schema['event_count']}")
    print(f"partitions: {schema['partition_count']} {schema['partitions']}")
    print(f"time range: {schema['time_range']}")
    print("event types:")
    for typ, info in schema["event_types"].items():
        print(f"  {typ}  (x{info['count']})")
        for attr, kinds in info["attributes"].items():
            print(f"      {attr}: {'|'.join(kinds)}")


def _cmd_match(args: argparse.Namespace) -> int:
    pattern = _load_pattern(args)
    events = _read_events(args.data, args)
    matches = run_match(
        pattern, events, exhaustive=args.exhaustive, assume_sorted=args.assume_sorted
    )
    rows = _match_rows(matches)
    _emit(rows, args.format, _print_match_table)
    return EXIT_OK if rows else EXIT_NO_MATCH


def _cmd_explain(args: argparse.Namespace) -> int:
    pattern = _load_pattern(args)
    events = _read_events(args.data, args)
    misses = run_explain(pattern, events, assume_sorted=args.assume_sorted)
    _emit(_near_miss_rows(misses), args.format, _print_near_miss_table)
    return EXIT_OK


def _cmd_schema(args: argparse.Namespace) -> int:
    events = _read_events(args.data, args)
    schema = run_schema(events)
    if args.format == "json":
        json.dump(schema, sys.stdout, indent=2, default=str)
        sys.stdout.write("\n")
    else:
        _print_schema_table(schema)
    return EXIT_OK


def _add_pattern_args(parser: argparse.ArgumentParser) -> None:
    group = parser.add_mutually_exclusive_group(required=True)
    group.add_argument(
        "--pattern-json",
        metavar="FILE",
        help="path to a JSON pattern AST (the stable construction surface)",
    )
    group.add_argument(
        "--pattern",
        metavar="TEXT",
        help="pattern in the text DSL (EXPERIMENTAL; not covered by the 0.1 "
        "stability guarantee)",
    )


def _add_common_args(parser: argparse.ArgumentParser) -> None:
    parser.add_argument(
        "data",
        nargs="?",
        default=None,
        help="events file, or - / omitted for stdin",
    )
    parser.add_argument(
        "--format",
        choices=("json", "table"),
        default="json",
        help="output format (default: json)",
    )
    parser.add_argument(
        "--input-format",
        choices=("auto", "jsonl", "csv", "parquet"),
        default="auto",
        help="input format (default: auto — inferred from file extension, else "
        "jsonl). jsonl events are {partition, ts, typ, attrs}; csv/parquet rows "
        "are flat records mapped by the --*-col options",
    )
    parser.add_argument(
        "--partition-col",
        default="partition",
        help="csv/parquet column for the partition key (default: partition)",
    )
    parser.add_argument(
        "--ts-col",
        default="ts",
        help="csv/parquet column for the timestamp (default: ts)",
    )
    parser.add_argument(
        "--type-col",
        default="typ",
        help="csv/parquet column for the event type (default: typ)",
    )
    parser.add_argument(
        "--attr-cols",
        default=None,
        help="comma-separated csv/parquet columns to keep as attributes "
        "(default: every column except the three mapped ones)",
    )
    parser.add_argument(
        "--assume-sorted",
        action="store_true",
        help="input is already grouped by partition and sorted by (ts, order); "
        "skip the internal sort so indices match input order",
    )


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="epigrep",
        description="grep for event sequences: temporal pattern matching over "
        "partitioned, timestamped events.",
    )
    parser.add_argument(
        "--version", action="version", version=f"epigrep {_version()}"
    )
    sub = parser.add_subparsers(dest="command", required=True)

    p_match = sub.add_parser("match", help="print matching spans")
    _add_pattern_args(p_match)
    _add_common_args(p_match)
    p_match.add_argument(
        "--exhaustive",
        action="store_true",
        help="emit every satisfying successor per start, not just the first",
    )
    p_match.set_defaults(func=_cmd_match)

    p_explain = sub.add_parser("explain", help="print near-miss explanations")
    _add_pattern_args(p_explain)
    _add_common_args(p_explain)
    p_explain.set_defaults(func=_cmd_explain)

    p_schema = sub.add_parser("schema", help="summarise event types/attributes")
    _add_common_args(p_schema)
    p_schema.set_defaults(func=_cmd_schema)

    return parser


def _version() -> str:
    try:
        from importlib.metadata import PackageNotFoundError, version

        return version("epigrep")
    except Exception:  # pragma: no cover - metadata may be unavailable
        return "unknown"


def main(argv: Optional[Sequence[str]] = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    try:
        return args.func(args)
    except CliError as exc:
        print(f"epigrep: {exc}", file=sys.stderr)
        return EXIT_ERROR


if __name__ == "__main__":  # pragma: no cover
    sys.exit(main())
