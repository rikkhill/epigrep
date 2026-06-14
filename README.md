# epigrep

Epigrep is a temporal event-pattern matcher: grep-like matching over typed,
timestamped event sequences.

This repository is currently in Phase 1: the retrospective Rust matcher core.
The first goal is boringly reliable semantics, not performance or product
surface area.

## Current Scope

The initial `epigrep-core` crate supports:

- partition-local matching over already-sorted event sequences;
- event type atoms;
- simple attribute predicates;
- non-contiguous sequence matching;
- inclusive time windows between participating events;
- absence-between guards over stable event order;
- explicit match consumption mode:
  - `FirstSuccessorPerStart` as the Phase 1 default;
  - `ExhaustivePerStart` for parity and future semantics work.

The oracle matcher is the semantic source of truth. The compiled matcher entry
point is present, but currently delegates to the same implementation until the
test surface is broad enough to support optimisation work.

## Non-Goals For Phase 1

- Python bindings;
- Arrow, pandas, or polars integration;
- mining;
- Loki or observability adapters;
- WASM demo;
- distributed streaming, watermarks, late data, or durable state.

## Development

Run the current test suite:

```sh
cargo test
```

Format check:

```sh
cargo fmt --all --check
```
