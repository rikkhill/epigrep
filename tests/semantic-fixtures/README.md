# Semantic golden fixtures

A small, cross-language corpus that pins Epigrep's matching semantics. Each
fixture is a tiny, human-checkable example tied to one clause of the
[semantics contract](https://rikkhill.github.io/epigrep/semantics/): some events,
a pattern (as a JSON AST), and the **hand-specified** expected matches and
near-misses.

The same fixtures are consumed by two harnesses, so the core matcher and the
Python builder/JSON/FFI surface are both held to the same contract:

- Rust: `crates/epigrep-core/tests/semantic_fixtures.rs` — checks the compiled
  matcher against the expectation, asserts the naive oracle agrees with it, and
  checks near-misses.
- Python: `crates/epigrep-py/tests/test_semantic_fixtures.py` — checks
  `match` / `explain` and compiled/oracle parity from the user-facing API.

## Fixture format

```jsonc
{
  "name": "...",
  "semantics_clause": "the rule this fixture pins",
  "rationale": "why this example demonstrates it",
  "events":  [{"partition": "a", "ts": 0, "typ": "A", "attrs": {}}],
  "pattern_json": { /* core pattern AST, incl. "consumption" */ },
  "expected_matches":     [{"partition": "a", "indices": [0, 1], "captures": {}}],
  "expected_near_misses": [{"partition": "b", "indices": [2], "reason": "no_successor"}]
}
```

`indices` are positions in the canonical sorted event stream (events are authored
already in that order). `reason` is one of `predicate_failed`, `absence_blocked`,
`window_exceeded`, `no_successor`.

## Regenerating

Fixtures are produced by `_generate.py`, which builds each pattern through the
stable Python builder (so the emitted AST is always valid) and — crucially —
**asserts the live matcher agrees with the hand-written expectation** before
writing the file. A disagreement means the authored expectation is wrong or there
is a real matcher bug; investigate rather than overwrite.

```sh
python tests/semantic-fixtures/_generate.py
```

The expected values are the source of truth: a future semantic change should
require deliberately updating these fixtures (and the semantics page), not merely
adjusting implementation tests.
