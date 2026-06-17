# epigrep

Temporal event-pattern matching over partitioned, timestamped event sequences —
grep-like matching for "what happened, in what order, within what time" — with
**explicit semantics** and **near-miss explanations**. A small, fast Rust core
with Python bindings.

> Give me partitioned timestamped events and a temporal pattern; I will return
> spans, captures, and explanations with explicit semantics.

## Install

```sh
pip install epigrep
```

Prebuilt wheels mean you do not need a Rust toolchain to use the package.

## Quick start

```python
from epigrep import Event, parse_pattern, match, explain, schema

stream = [
    Event("child-1", 0, "entered_care"),
    Event("child-1", 2, "placement_change"),
    Event("child-1", 5, "safeguarding_flag", {"severity": 4}),
    Event("child-2", 0, "entered_care"),
    Event("child-2", 4, "safeguarding_flag", {"severity": 4}),
]

pattern = parse_pattern(
    "entered_care -[<=5, no placement_change]-> safeguarding_flag[severity >= 3]"
)

for m in match(pattern, stream):
    print(m.partition, m.indices, m.captures)   # child-2 [3, 4] {}

# Why didn't the others match?
for nm in explain(pattern, stream):
    print(nm.reason)                             # child-1: absence_blocked
```

`schema(stream)` summarises the event types and attributes available;
`match(...)` runs a pattern (compiled by default, `exhaustive=` and `oracle=`
flags available); `explain(...)` returns near-misses with structured detail.

## Programmatic / agent use

Patterns round-trip through a **stable JSON AST**, the recommended interface for
tools and LLMs (emit/validate a structured pattern rather than DSL text):

```python
from epigrep import parse_pattern, pattern_from_json

ast = parse_pattern("A -[<=5]-> B").to_json()
pattern = pattern_from_json(ast)   # validated; safe to match
```

## Semantics

Matching has an explicit, tested contract (ordering and tie-breaking,
partitioning, match-consumption modes, inclusive windows, absence-between,
capture/register equality, overlapping matches, and near-miss guarantees). A
naive oracle matcher is the source of truth; a compiled matcher is checked
against it by property tests.

## Status

Alpha (0.1). The Python API and JSON AST are the intended stable surface; the
text DSL is experimental. MIT licensed.
