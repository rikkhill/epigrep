# epigrep

[![PyPI](https://img.shields.io/pypi/v/epigrep)](https://pypi.org/project/epigrep/)
[![Python versions](https://img.shields.io/pypi/pyversions/epigrep)](https://pypi.org/project/epigrep/)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://github.com/rikkhill/epigrep/blob/main/LICENSE)

**Grep is good at lines. Epigrep is for sequences.**

Epigrep finds temporal patterns in partitioned, timestamped event streams — the
kind of question that is awkward in grep, SQL, or ad-hoc pandas: *"a config
reload followed by an OOM within two minutes, with no readiness success in
between, per pod."* You describe the sequence; Epigrep returns the matches,
their spans and captured values, and — for the near-misses — an explanation of
why they did not match.

It is a small Rust core with Python bindings and an explicit, tested matching
contract.

- **Documentation:** <https://rikkhill.github.io/epigrep/>
- **Source:** <https://github.com/rikkhill/epigrep>

## Install

```sh
pip install --pre epigrep
```

The current release is a release candidate, hence `--pre`. Prebuilt wheels cover
Linux (x86_64, aarch64), macOS (Apple Silicon), and Windows (x64); other
platforms build from the source distribution, which needs a Rust toolchain.

## Quick start

```python
from epigrep import Event, Pattern, explain, match

stream = [
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

for m in match(pattern, stream):
    print(m.partition, list(m.indices))   # api-1 [3, 4]

# Why didn't the others match?
for nm in explain(pattern, stream):
    print(nm.partition, nm.reason)        # api-0 absence_blocked
```

`api-1` matches: a reload, then an OOM 90s later, nothing in between. `api-0`
does not — a `readiness_success` lands in the gap, so the `no=` clause rules it
out, and `explain()` tells you that rather than leaving you to work it out.

## What you get back

For each start position, Epigrep returns either:

- a **match** — participating event indices, the span (start and end
  timestamps), and any captured attribute values; or
- a **near-miss** — for starts that cannot complete, the deepest partial path it
  reached and the reason the next step failed (`predicate_failed`,
  `absence_blocked`, `window_exceeded`, or `no_successor`), with structured
  detail.

`schema(stream)` summarises the event types and attributes present. Pandas
dataframe helpers are available when pandas is installed; matching and
explanation do not require it.

## Patterns

Two construction surfaces are stable: the **builder** (above) for code written
by hand, and a **JSON pattern format** for tools and agents that emit and
validate patterns programmatically.

```python
from epigrep import Pattern, pattern_from_json

ast = Pattern.event("A").then("B", within=5).build().to_json()
pattern = pattern_from_json(ast)   # validated; safe to match
```

A terse text DSL also exists (`parse_pattern`), but it is **experimental** and
outside the 0.1 stability guarantee — prefer the builder or JSON format.

## Semantics

Matching has an explicit, tested contract: ordering and tie-breaking,
partitioning, match-consumption modes, inclusive windows, absence-between,
capture/register equality, overlapping matches, and near-miss guarantees. A
naive reference matcher is the source of truth; a second, independent
implementation is checked against it by property tests. See the
[documentation](https://rikkhill.github.io/epigrep/semantics/).

## Status

Alpha, published as a release candidate. The Python API and JSON pattern format
are the intended stable surface; the text DSL is experimental. MIT licensed.
