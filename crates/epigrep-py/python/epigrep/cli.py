"""epigrep command-line interface — grep for event sequences.

Subcommands mirror the stable Python entrypoints so the CLI teaches the same
model:

  * ``epigrep match``   — run a pattern, print matching spans + captures
  * ``epigrep explain`` — print near-miss explanations for non-matching starts
  * ``epigrep schema``  — summarise what event types / attributes / partitions
                          a stream contains (the "what can I even query" step)

Data is read as JSONL: one event object per line, ``{"partition", "ts", "typ",
"attrs"}`` (``attrs`` optional) — the same shape as the bundled examples. Pass a
path, ``-``, or nothing to read from stdin.

Patterns come from ``--pattern-json FILE`` (the stable JSON AST) or, marked
experimental, ``--pattern TEXT`` (the provisional text DSL).

Exit codes follow grep so ``epigrep match ... && ...`` works as muscle memory
expects: ``0`` = at least one match, ``1`` = no match, ``2`` = error. ``explain``
and ``schema`` return ``0`` on success and ``2`` on error.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import List, Optional, Sequence

from ._core import Event, parse_pattern, pattern_from_json
from . import explain as run_explain
from . import match as run_match
from . import near_miss_summary, schema as run_schema

EXIT_OK = 0
EXIT_NO_MATCH = 1
EXIT_ERROR = 2


class CliError(Exception):
    """A user-facing error: reported to stderr, exits with code 2."""


def _read_events(data_arg: Optional[str]) -> List[Event]:
    if data_arg in (None, "-"):
        text = sys.stdin.read()
    else:
        try:
            text = Path(data_arg).read_text()
        except OSError as exc:
            raise CliError(f"cannot read {data_arg}: {exc}") from exc

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
    events = _read_events(args.data)
    matches = run_match(
        pattern, events, exhaustive=args.exhaustive, assume_sorted=args.assume_sorted
    )
    rows = _match_rows(matches)
    _emit(rows, args.format, _print_match_table)
    return EXIT_OK if rows else EXIT_NO_MATCH


def _cmd_explain(args: argparse.Namespace) -> int:
    pattern = _load_pattern(args)
    events = _read_events(args.data)
    misses = run_explain(pattern, events, assume_sorted=args.assume_sorted)
    _emit(_near_miss_rows(misses), args.format, _print_near_miss_table)
    return EXIT_OK


def _cmd_schema(args: argparse.Namespace) -> int:
    events = _read_events(args.data)
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
        help="JSONL events file, or - / omitted for stdin",
    )
    parser.add_argument(
        "--format",
        choices=("json", "table"),
        default="json",
        help="output format (default: json)",
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
