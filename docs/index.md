# What is Epigrep?

**Grep is good at lines. Epigrep is for sequences.**

Epigrep finds temporal patterns in partitioned, timestamped event streams. Where
grep matches a line and SQL matches a row, Epigrep matches an ordered *sequence*
of events that unfolds over time, within one partition, inside a time budget,
optionally with a clause saying what must *not* happen in between.

![A swimlane of events per pod, with one sequence matched and one blocked](assets/swimlane.svg)

The question above — *config reload, then OOM within two minutes, with no
readiness success in between, per pod* — is painful to write in grep (no order,
no windows), awkward in SQL (self-joins and `NOT EXISTS` per gap), and fiddly in
pandas (manual grouping, sorting, and index bookkeeping). In Epigrep it is one
pattern:

```python
pattern = (
    Pattern.event("config_reload")
    .then("oom_killed", within=120, no="readiness_success")
    .build()
)
```

## What you get back

For each start position Epigrep tells you one of two things:

- a **match** — the participating event indices, the span (start and end
  timestamps), and any captured attribute values; or
- a **near-miss explanation** — for starts that cannot complete, the deepest
  partial path it reached and the reason the next step failed (a predicate, a
  blocking event, an exceeded window, or simply no successor).

The explanation is the part that is hard to get from grep, SQL, or pandas: not
just *that* something did not match, but *why*.

## Design in one breath

- A small **Rust core** does the matching; **Python bindings** are the surface.
- The **semantics are written down and tested**, not implied by the code. A
  naive reference matcher is the source of truth, and a second, independent
  implementation is checked against it by property tests.
- Patterns are built with a **Python builder** or a **stable JSON format**; the
  text DSL is experimental.

## Where next

- [Getting started](getting-started.md) — build it and run your first match.
- [Events and partitions](events-and-partitions.md) — the input model.
- [Patterns](patterns.md) — how to express what you are looking for.
- [Semantics](semantics.md) — exactly what a match and a non-match mean.
- [Explanations](explanations.md) — near-misses and what they do and do not promise.
- [Logs-first recipes](logs-first-recipes.md) — worked observability examples.
- [Limitations](limitations.md) — the boundaries of the 0.1 release.

!!! note "Status"
    Alpha (0.1). The Python API and JSON pattern format are the intended stable
    surface; the text DSL is experimental. Not yet on PyPI — install from
    source. MIT licensed.
