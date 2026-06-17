# Getting started

Epigrep is not yet on PyPI, so for now you build it from source. The core is
Rust; the package is built with [maturin](https://www.maturin.rs/), which
compiles the extension and produces an installable wheel.

## Requirements

- Rust (stable) with `cargo`.
- Python 3.9 or newer.
- `maturin` (`pip install maturin`).

## Install from source

Build a wheel and install it into your environment:

```sh
pip install maturin
maturin build --release --manifest-path crates/epigrep-py/Cargo.toml --out dist
pip install --no-index --find-links dist epigrep
```

For an iterative development loop, `maturin develop` builds the extension and
installs it in place:

```sh
python -m venv .venv && source .venv/bin/activate
pip install maturin pytest
maturin develop --manifest-path crates/epigrep-py/Cargo.toml
```

Pandas is optional. Matching and explanation do not need it; the dataframe
helpers (`events_to_frame`, `matches_to_frame`) and the Streamlit storyboard do.

## Your first match

```python
from epigrep import Event, Pattern, explain, match

events = [
    Event("api-0", 0,  "config_reload",     {"pod": "api-0"}),
    Event("api-0", 30, "readiness_success", {"pod": "api-0"}),
    Event("api-0", 70, "oom_killed",        {"pod": "api-0"}),
    Event("api-1", 0,  "config_reload",     {"pod": "api-1"}),
    Event("api-1", 90, "oom_killed",        {"pod": "api-1"}),
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

```
api-1 [3, 4] {}
api-0 absence_blocked
```

`match()` and `explain()` group events by partition and sort them for you, so
you can pass an unsorted list. The indices in a result refer to positions within
that event's partition, in `(timestamp, input order)`.

## Inspecting your data

`schema(events)` summarises the event types and attributes present, which is
useful before writing a pattern:

```python
from epigrep import schema
print(schema(events))
```

## Running the examples

The repository ships runnable logs-first fixtures:

```sh
python examples/logs-first/run.py
```

Each fixture is a JSON file in `examples/logs-first/` with deterministic events,
the pattern in builder and JSON form, and the expected matches and near-misses.
The [recipes page](logs-first-recipes.md) explains them.

## Next

- [Events and partitions](events-and-partitions.md) for the input model.
- [Patterns](patterns.md) for the full construction surface.
