# Near-miss explanations

The useful question is rarely "did it match?" but "why not?". `explain()` answers
that for the starts that did not complete.

```python
from epigrep import explain

for miss in explain(pattern, events):
    print(miss.partition, list(miss.indices), miss.reason)
```

## What counts as a near-miss

A start is a near-miss if and only if **no** full match exists from it, explored
exhaustively. This is independent of the match-consumption mode: a start that can
complete is reported by `match()`, and a start that cannot is reported by
`explain()`. The two are complementary.

For each near-miss you get the **deepest reachable partial path** — how far the
sequence got before it stalled — and a single **reason** for why the next step
could not be satisfied.

## The reasons

Reasons are reported by a "nearest miss" priority, so you see the most specific
explanation available:

| Reason | Meaning |
|--------|---------|
| `predicate_failed` | A candidate of the right type existed, but an attribute predicate, reference, or capture constraint failed. |
| `absence_blocked` | A successor existed in range, but a `no X` clause was violated by an event in the gap. |
| `window_exceeded` | A successor of the right type existed, but fell outside the `within` window. |
| `no_successor` | No event of the next step's type appeared after the frontier at all. |

Each reason carries structured detail — the failed clause with actual and
expected values, the blocking event for an absence, or the window overshoot —
which the example runner prints in full. For instance, a capture mismatch reads:

```
trace-b [3] predicate_failed: reached [3]; db_query at 4:
  would match if request_id == $request (bound 'req-2', was 'req-other')
```

## What it does and does not promise

A near-miss is a **heuristic explanation, not an exhaustive enumeration** of
every way a start could have failed. It reports one deepest path and one nearest
reason per start. If a start could have failed in several ways, you are shown the
deepest, highest-priority one — not all of them.

This is a deliberate trade-off: the single most relevant reason is usually what
you want, and enumerating every failure mode would be noisier and slower. If you
need the full picture for a particular start, narrow the events or the pattern
and re-run.

## Next

- [Logs-first recipes](logs-first-recipes.md) — near-misses in worked examples.
- [Semantics](semantics.md) — the precise definition of a match.
