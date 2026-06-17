# epigrep

Temporal event-pattern matching over partitioned, timestamped event sequences —
grep-like matching for "what happened, in what order, within what time" — with
**explicit semantics** and **near-miss explanations**.

> Give me partitioned timestamped events and a temporal pattern; I will return
> spans, captures, and explanations with explicit semantics.

## Install

epigrep is on PyPI. The current release is a pre-release (release candidate),
so install it with `--pre`:

```sh
pip install --pre epigrep
```

Prebuilt wheels are published for Linux (x86_64, aarch64), macOS (Apple
Silicon), and Windows (x64); other platforms build from the source
distribution, which needs a Rust toolchain.

## Quick Start

```python
from epigrep import Event, Pattern, explain, match, schema

stream = [
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

for m in match(pattern, stream):
    print(m.partition, list(m.indices), dict(m.captures))   # api-1 [3, 4] {}

# Why didn't the others match?
for nm in explain(pattern, stream):
    print(nm.partition, nm.reason)                # api-0 absence_blocked
```

`schema(stream)` summarises the event types and attributes available;
`match(...)` runs a pattern (compiled by default, `exhaustive=` and `oracle=`
flags available); `explain(...)` returns near-misses with structured detail.
Pandas dataframe helpers are available when pandas is installed; matching and
explanation do not require pandas.

## Programmatic / agent use

Patterns round-trip through a **stable JSON AST**, the recommended interface for
tools and LLMs (emit/validate a structured pattern rather than DSL text):

```python
from epigrep import Pattern, pattern_from_json

ast = Pattern.event("A").then("B", within=5).build().to_json()
pattern = pattern_from_json(ast)   # validated; safe to match
```

`parse_pattern(...)` remains importable for demos and experiments, but the text
DSL is provisional and outside the 0.1 stability guarantee.

## Semantics

Matching has an explicit, tested contract (ordering and tie-breaking,
partitioning, match-consumption modes, inclusive windows, absence-between,
capture/register equality, overlapping matches, and near-miss guarantees). A
naive oracle matcher is the source of truth; a compiled matcher is checked
against it by property tests.

## Status

Alpha. Published to PyPI as a release candidate (`0.1.0rc1`). The Python API
and JSON AST are the intended stable surface; the text DSL is experimental.
MIT licensed.
