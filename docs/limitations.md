# Limitations and non-goals

Epigrep is alpha (0.1). It does one thing — match temporal patterns over
partitioned event sequences — and the boundaries below are deliberate, not
oversights. Knowing them is the fastest way to decide whether it fits.

## What it expects of you

- **Events, not log lines.** Epigrep matches over typed events you have already
  parsed into `(partition, timestamp, type, attributes)`. It does not parse raw
  log text; that step is yours.
- **In memory.** It matches over the events you pass it, on one machine. There
  is no storage layer, no indexing, no query server.
- **Integer event-time timestamps.** Times are integers in a unit you choose;
  windows use the same unit.

## Not in the 0.1 pattern surface

- strict adjacency (`A` immediately followed by `B`);
- alternation, quantifiers, and grouping;
- a full, stable text DSL — the existing DSL is experimental, and the builder
  and JSON format are the stable surfaces.

## Explicitly out of scope

- **Streaming and late data** — no watermarks, no durable state, no incremental
  matching over an unbounded stream.
- **Mining** — Epigrep matches patterns you specify; it does not discover
  frequent episodes for you.
- **Log-line eventisation** — turning unstructured lines into typed events.
- **Time-series / numeric matching** — it matches discrete typed events, not
  shapes in continuous signals.
- **Distributed execution.**
- **Adapters** (for example a Loki source) and an interactive WASM demo.

## Honest performance note

The core is Rust and is fast enough for the interactive, single-machine use it is
built for. There are no benchmarked throughput or scale claims yet, so please do
not read "Rust core" as a promise of any particular number — measure on your own
data if it matters.

## Stability

The Python API (`Event`, `Pattern`, the builder, `match`, `explain`, `schema`)
and the JSON pattern format are the intended stable surface for 0.1. The text DSL
is experimental and may change. The package is published on PyPI — install it
with `pip install epigrep`, or build from source.

If a boundary here is the one thing standing between you and a use case, that is
useful to know — the roadmap is shaped by which of these turn out to matter.
