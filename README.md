# epigrep

Epigrep is a temporal event-pattern matcher: grep-like matching over typed,
timestamped event sequences with explicit semantics and near-miss explanations.

The current repository is a 0.1 release-candidate work area, not a published
package. The core matcher, Python bindings, JSON AST, schema/match/explain
surface, and Streamlit storyboard exist; the current hardening slice is making
the logs-first examples, docs, and package artifacts boringly reproducible.

## Current Scope

The `epigrep-core` crate supports:

- partition-local matching over already-sorted event sequences;
- event type atoms;
- simple attribute predicates;
- non-contiguous sequence matching;
- inclusive time windows between participating events;
- absence-between guards over stable event order;
- capture/register equality across participating events;
- a tiny provisional parser for examples like
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

## Python API

For human-written examples, prefer the builder:

```python
from epigrep import Event, Pattern, explain, match

events = [
    Event("api-0", 0, "config_reload", {"pod": "api-0"}),
    Event("api-0", 30, "readiness_success", {"pod": "api-0"}),
    Event("api-0", 70, "oom_killed", {"pod": "api-0"}),
    Event("api-1", 0, "config_reload", {"pod": "api-1"}),
    Event("api-1", 90, "oom_killed", {"pod": "api-1"}),
]

pattern = (
    Pattern.event("config_reload")
    .then("oom_killed", within=120, no="readiness_success")
    .build()
)

for found in match(pattern, events):
    print(found.partition, list(found.indices), dict(found.captures))

for miss in explain(pattern, events):
    print(miss.partition, miss.reason)
```

For tools and agents, prefer the stable JSON AST:

```python
import json
from epigrep import pattern_from_json

pattern = pattern_from_json(json.dumps({
    "steps": [
        {
            "atom": {
                "event_type": "config_reload",
                "predicates": [],
                "reference_predicates": [],
                "captures": [],
            },
            "transition_from_previous": None,
        },
        {
            "atom": {
                "event_type": "oom_killed",
                "predicates": [],
                "reference_predicates": [],
                "captures": [],
            },
            "transition_from_previous": {
                "max_elapsed": 120,
                "absence": [{
                    "event_type": "readiness_success",
                    "predicates": [],
                    "reference_predicates": [],
                    "captures": [],
                }],
            },
        },
    ],
    "consumption": "FirstSuccessorPerStart",
}))
```

`parse_pattern(...)` remains importable for the Streamlit storyboard and quick
experiments, but the text DSL is experimental and outside the 0.1 stability
guarantee.

## Logs-First Examples

Executable logs-first fixtures live in `examples/logs-first/`. Each JSON file
contains deterministic events, a builder recipe, the stable JSON AST, expected
matches, expected near-misses, and short prose.

After installing the local package:

```sh
python examples/logs-first/run.py
python -m pytest crates/epigrep-py/tests/test_logs_first_examples.py
```

The fixture set currently covers:

- config reload -> OOM within two minutes, with no readiness success between;
- deploy -> error spike -> rollback;
- repeated readiness failure -> restart;
- fatal error with no prior warning;
- same-request capture/reference equality.

## Visual harness

The `epigrep-py` crate exposes the Rust core to Python via PyO3/maturin as the
`epigrep` package: construct events, build/parse patterns, run matches, and
inspect partitions, spans, captures, and bindings. Pandas helpers
(`events_to_frame`, `matches_to_frame`) and a demo-data module (`epigrep.data`)
support tests and the visual harness.

`epigrep.explain(pattern, events)` returns *near-misses* — starts that cannot
complete (in any consumption mode), each with its deepest reachable partial path
and the reason the next step failed (`predicate_failed`, `absence_blocked`,
`window_exceeded`, or `no_successor`). Pandas dataframe helpers are available
when pandas is installed; the core package does not require pandas just to
match/explain events.

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
.venv/bin/python examples/logs-first/run.py
.venv/bin/streamlit run apps/epigrep-storyboard/app.py
```

### Package artifact smoke

The package is not uploaded to PyPI/TestPyPI in this slice. To verify a local
wheel/sdist and install the wheel into a clean environment:

```sh
rm -rf dist /tmp/epigrep-smoke
.venv/bin/maturin build --release --manifest-path crates/epigrep-py/Cargo.toml --out dist
.venv/bin/maturin sdist --manifest-path crates/epigrep-py/Cargo.toml --out dist
python3 -m venv /tmp/epigrep-smoke
/tmp/epigrep-smoke/bin/python -m pip install --no-index --find-links dist epigrep
/tmp/epigrep-smoke/bin/python examples/logs-first/run.py
```
