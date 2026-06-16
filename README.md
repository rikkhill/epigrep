# epigrep

Epigrep is a temporal event-pattern matcher: grep-like matching over typed,
timestamped event sequences.

Phase 1 (the retrospective Rust matcher core) is complete. Phase 2 adds Python
bindings and a Streamlit visual harness so the matcher's semantics are
observable in a browser. The guiding goal remains boringly reliable semantics,
not performance or product surface area.

## Current Scope

The initial `epigrep-core` crate supports:

- partition-local matching over already-sorted event sequences;
- event type atoms;
- simple attribute predicates;
- non-contiguous sequence matching;
- inclusive time windows between participating events;
- absence-between guards over stable event order;
- capture/register equality across participating events;
- a tiny Phase 1 parser for examples like
  `A[user_id as $u] -[<=5, no C]-> B[user_id == $u]`;
- explicit match consumption mode:
  - `FirstSuccessorPerStart` as the Phase 1 default;
  - `ExhaustivePerStart` for parity and future semantics work.

There are two independent matcher backends:

- the **oracle** (`oracle_matches`), a naive depth-first backtracking matcher
  that is the executable semantic source of truth;
- the **compiled** matcher (`CompiledPattern`), a forward NFA-style simulation
  that sweeps each partition once, carrying in-flight partial matches as
  "threads" with their own bindings, window anchor, and absence state.

The two share only leaf predicate evaluation, not sequencing logic, so the
property tests comparing them are a genuine cross-check rather than a tautology:
a divergence is a real semantic bug. (Introducing the second backend already
surfaced one — the first-successor consumption mode now commits to the earliest
satisfying successor per step, rather than backtracking to find a completion.)

## Phase 2: Python bindings and visual harness

The `epigrep-py` crate exposes the Rust core to Python via PyO3/maturin as the
`epigrep` package: construct events, parse/build patterns, run matches, and
inspect partitions, spans, captures, and bindings. Pandas helpers
(`events_to_frame`, `matches_to_frame`) and a demo-data module (`epigrep.data`)
support tests and the visual harness.

```python
from epigrep import Event, parse_pattern, match

stream = [
    Event("child-1", 0, "entered_care"),
    Event("child-1", 4, "safeguarding_flag", {"severity": 4}),
]
results = match(parse_pattern("entered_care -[<=5]-> safeguarding_flag[severity >= 3]"), stream)
```

`epigrep.explain(pattern, events)` returns *near-misses* — starts that cannot
complete (in any consumption mode), each with its deepest reachable partial path
and the reason the next step failed (`predicate_failed`, `absence_blocked`,
`window_exceeded`, or `no_successor`).

The Rust oracle/compiled parity tests remain authoritative; the Python API and
the Streamlit app are wrappers, not the semantic source of truth.

## Non-Goals (still deferred)

- mining;
- Loki or observability adapters;
- WASM / static public demo;
- robust/numeric time-series eventisation;
- distributed streaming, watermarks, late data, or durable state.

## Development

### Rust core

```sh
cargo test                 # core test suite (default workspace member)
cargo fmt --all --check
cargo clippy --all-targets
```

`epigrep-py` is excluded from the default workspace set because it is a Python
extension linked against the active interpreter; build it with maturin, not
bare `cargo build`.

### Python bindings + visual harness

Requires a Python environment with `maturin` (and, for the app, `streamlit`).
Using `uv`:

```sh
uv venv --python 3.12 .venv
uv pip install --python .venv maturin pytest pandas streamlit altair
.venv/bin/maturin develop --manifest-path crates/epigrep-py/Cargo.toml
.venv/bin/python -m pytest crates/epigrep-py/tests/
.venv/bin/streamlit run apps/epigrep-storyboard/app.py
```
