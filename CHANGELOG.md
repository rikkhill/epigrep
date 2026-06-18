# Changelog

All notable changes to epigrep are documented here. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project aims to
follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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

[Unreleased]: https://github.com/rikkhill/epigrep/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/rikkhill/epigrep/releases/tag/v0.1.0
