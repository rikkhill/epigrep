# epigrep

**Grep is good at lines. Epigrep is for sequences.**

Epigrep finds temporal patterns in partitioned, timestamped event streams — the
sort of question that is awkward to express in grep, SQL, or ad-hoc pandas:
*"a config reload followed by an OOM within two minutes, with no readiness
success in between, per pod"*. You describe the sequence; Epigrep returns the
matches, their spans and captured values, and — for the near-misses — an
explanation of why they did not match.

It is a small Rust core with Python bindings. The matching semantics are
written down and tested rather than implied by the implementation.

> Status: alpha (0.1.0). Published to PyPI — `pip install epigrep`. The Python
> API and JSON pattern format are the intended stable surface; the text DSL is
> experimental. MIT licensed.

## When it helps

You have structured logs or event traces — Kubernetes events, deploy and
readiness signals, request traces, pipeline steps — already parsed into typed
events with timestamps. Somewhere in there is a *sequence* you care about, and
it spans several lines, in order, within some time budget, possibly with a
"this must not happen in between" clause. That is the shape Epigrep is for.

It is **not** a database, a streaming platform, or a general anomaly detector.
It matches patterns over event sequences you already have in memory.

## A first match

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
    print(found.partition, list(found.indices))   # api-1 [3, 4]

for miss in explain(pattern, events):
    print(miss.partition, miss.reason)            # api-0 absence_blocked
```

`api-1` matches: a reload, then an OOM 90s later, nothing in between. `api-0`
does not — a `readiness_success` lands between the reload and the OOM, so the
`no=` clause rules it out. `explain()` tells you that, rather than leaving you
to work it out.

## Patterns

Two construction surfaces are stable:

- the **builder** (`Pattern.event(...).then(...)`) for code written by hand;
- a **JSON pattern format** for tools and agents that need to emit and validate
  patterns programmatically (`pattern_from_json` / `Pattern.to_json`).

A terse text DSL (`A[x as $u] -[<=5, no C]-> B[x == $u]`) also exists and is
used by the examples, but it is **experimental** and outside the 0.1 stability
guarantee — prefer the builder or JSON format.

See the [documentation](#documentation) for events and partitions, the full
pattern surface, the matching semantics, and near-miss explanations.

## Install

epigrep is on PyPI:

```sh
pip install epigrep
```

Prebuilt wheels cover Linux (x86_64, aarch64), macOS (Apple Silicon), and
Windows (x64); other platforms build from the source distribution, which needs a
Rust toolchain. To build from a local checkout instead, see the
[getting-started guide](docs/getting-started.md).

## Examples

Runnable logs-first fixtures live in [`examples/logs-first/`](examples/logs-first).
Each carries deterministic events, the pattern in both builder and JSON form,
and the expected matches and near-misses:

```sh
python examples/logs-first/run.py
```

They cover config-reload → OOM, deploy → error spike → rollback, repeated
readiness failure → restart, fatal error without a prior warning, and a
same-request capture constraint. The [recipes page](docs/logs-first-recipes.md)
walks through them.

## Documentation

The docs source lives in [`docs/`](docs) and is published with MkDocs:

| Page | What it covers |
|------|----------------|
| [What is Epigrep?](docs/index.md) | The idea, in one page |
| [Getting started](docs/getting-started.md) | Build, install, first match |
| [Events and partitions](docs/events-and-partitions.md) | Event shape, ordering, ties |
| [Patterns](docs/patterns.md) | Builder, JSON format, DSL status |
| [Semantics](docs/semantics.md) | What a match and a non-match mean |
| [Explanations](docs/explanations.md) | Near-misses and their guarantees |
| [Logs-first recipes](docs/logs-first-recipes.md) | The example fixtures, explained |
| [Limitations](docs/limitations.md) | What it does not do |

To preview the site locally:

```sh
pip install -r docs/requirements.txt
mkdocs serve
```

## What it does not do (yet)

Single-machine, in-memory matching over events you have already parsed into the
`(partition, timestamp, type, attributes)` shape. No streaming or late data, no
mining, no log-line parsing, no distributed execution. These are deliberate
0.1 boundaries, not oversights — see [limitations](docs/limitations.md).

## Development

```sh
cargo test                  # Rust core
cargo fmt --all --check
cargo clippy --all-targets
```

Python bindings and the Streamlit storyboard need a Python environment with
maturin; the [getting-started guide](docs/getting-started.md) has the full loop.

## Licence

MIT. See [LICENSE](LICENSE).
