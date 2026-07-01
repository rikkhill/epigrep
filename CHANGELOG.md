# Changelog

All notable changes to epigrep are documented here. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project aims to
follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.0] — 2026-07-01

### Added

- Experimental `epigrep.agent` surface: a thin, JSON-in/JSON-out wrapper
  (`describe_schema`, `run`, `explain`) for LLM/agent use, where `run` validates
  a JSON pattern AST and executes it in one call and bad input returns a
  repairable `{"ok": False, "stage", "error"}` dict instead of raising. Outside
  the 0.1 stability guarantee.
- Eventisation primitive (`eventise`) turning record/tabular data into the sorted
  `Event` list the matcher expects, with `partition` / `ts` / `typ` given as keys
  or callables and attributes selected explicitly or by remainder. Numpy/pandas
  scalars are coerced to native values and null/NaN attributes dropped.
- `events_from_frame`: build events from a pandas or polars `DataFrame` or a
  pyarrow `Table` (detected by duck-typing, so no dataframe library is a hard
  dependency). The inverse of `events_to_frame`; documents the pre-sorted +
  `assume_sorted=True` contract for aligning `Match.indices` with frame rows.
- `epigrep` command-line interface (`epigrep` console script / `python -m
  epigrep`) with `match`, `explain`, and `schema` subcommands over JSONL (file or
  stdin), JSON and table output, and grep-style exit codes (`0` match, `1` no
  match, `2` error). CSV and parquet inputs are also accepted (`--input-format`,
  or inferred from the extension), mapped to events by `--partition-col` /
  `--ts-col` / `--type-col` / `--attr-cols` via the same `eventise` primitive.
- `epigrep.datasets`: an SPMF sequence-database loader (`load_spmf` /
  `load_spmf_file`) plus a bundled four-sequence sample and an
  `examples/datasets/spmf-sample.txt` fixture, with a ground-truth
  sequential-pattern recovery test.
- Type information: the package now ships `py.typed` and a `_core.pyi` stub for
  the compiled extension, and the pure-Python wrappers carry type hints. Type
  checkers see the real signatures instead of `Any`.
- Panic-safety tests proving the public Python surface (constructors, JSON AST
  loading, and the text parser) rejects malformed input with `ValueError` /
  `TypeError` rather than letting it reach an internal Rust panic.

### Changed

- The matchers' entrypoints (`match` / `explain`) now validate a pattern's
  structure at the Python boundary and raise `ValueError`, so a malformed
  pattern can no longer trip the core's internal `validate_pattern(...).expect`
  and panic across FFI.
- `__all__` documents the public API; the stable surfaces (builder, JSON AST,
  `match` / `explain` / `schema`) are distinguished from the provisional text
  DSL (`parse_pattern`) in the package docstring.
- The version-sync guard (`scripts/check_version_sync.py`) now also checks the
  resolved `epigrep-py` version in `Cargo.lock`, not just the two manifests.
- `docs/RELEASE-GATE.md` is now a durable next-release runbook rather than a
  one-off pre-1.0 gate.
- Documentation is guarded against API drift: every runnable `python` snippet in
  the READMEs and docs is executed against the installed package in CI, so a
  renamed function or stale import in the docs fails tests instead of shipping.

## [0.1.0] — 2026-06-18

First stable release: a small Rust core with a Python API for matching ordered
patterns over partitioned, timestamped event sequences.

### Added

- Matching over `(partition, timestamp, type, attributes)` events: ordered,
  non-contiguous steps with inclusive `within` windows, absence-between (`no`),
  attribute predicates, and capture/reference equality.
- Two stable construction surfaces — the Python builder and a JSON pattern
  format (`pattern_from_json` / `Pattern.to_json`) for tools and agents. The
  text DSL (`parse_pattern`) is provisional and outside the 0.1 stability
  guarantee.
- Near-miss explanations (`explain`): for starts that cannot complete, the
  deepest partial path and the nearest reason (`predicate_failed`,
  `absence_blocked`, `window_exceeded`, `no_successor`) with structured detail.
- `schema()` for summarising the event types and attributes present.
- Optional pandas dataframe helpers (`events_to_frame`, `matches_to_frame`),
  available when pandas is installed.
- Runnable logs-first example fixtures (`examples/logs-first/`).
- Explicit, tested matching semantics: a naive reference matcher is the source
  of truth, checked against a second independent implementation by property
  tests.

### Packaging

- Published to PyPI with prebuilt wheels for Linux (x86_64, aarch64), macOS
  (Apple Silicon), and Windows (x64); other platforms (including Intel macOS)
  build from the source distribution. Requires Python ≥ 3.9.
- Published via GitHub trusted publishing (OIDC); MIT licensed.

[Unreleased]: https://github.com/rikkhill/epigrep/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/rikkhill/epigrep/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/rikkhill/epigrep/releases/tag/v0.1.0
