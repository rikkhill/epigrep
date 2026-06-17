# Semantics

Epigrep's matching has an explicit, tested contract. This page is the
user-facing version of it: exactly what a match, a non-match, and an explanation
mean. A naive reference matcher is the executable source of truth, and a second,
independent implementation is checked against it by property tests, so both obey
what is written here.

## Input model

- Events are `(partition, timestamp, event_type, attributes)`. Timestamps are
  integers in event time. Attribute values are string, integer, float, boolean,
  or null.
- Input is matched grouped by partition and sorted by `(timestamp, input
  order)`. The Python `match()` / `explain()` wrappers do this for you; the
  low-level entry points expect sorted input and reject anything else.

## Ordering and tie-breaking

Within a partition, events are ordered by `timestamp`, then by **input
position** for equal timestamps. Input position is the stable tie-breaker
throughout — windows, absence, and match identity all use it.

## Partitioning

Matching is **within a single partition only**; there are no cross-partition
matches. Partitions are processed in first-seen order.

## Sequence and skipping

A pattern is a sequence of steps. `A -> B` is **non-contiguous**: irrelevant
events may occur between matched steps. Strict adjacency is not in the 0.1
surface.

## Windows

A transition window `within Δ` requires `0 <= timestamp(next) -
timestamp(previous) <= Δ`. The bound is **inclusive** at both ends. `Δ` is in the
same integer unit as the timestamps.

## Absence-between

`no X` on a transition forbids any event matching atom `X` that lies **strictly
between** the two participating events in `(timestamp, input order)`.
Equal-timestamp events are included or excluded by input position, not by
timestamp alone. The absent atom's own predicates apply, so only a genuinely
matching `X` blocks; reference predicates in an absent atom are evaluated against
the bindings accumulated up to the previous step.

## Predicates

Attribute predicates compare an attribute to a literal with `== != > >= < <=`. A
missing attribute fails the predicate. Numbers compare by value across integer
and float (so `1` equals `1.0`), and `==` is consistent with the ordering
operators; integer/integer comparisons are exact. Non-numeric or NaN operands
are incomparable, so ordering comparisons against them are false.

## Captures and references

- A capture `attr as $name` binds the event's value of `attr` (or null if
  absent) the first time it is seen along a path.
- A reference predicate `attr op $name` compares the event's `attr` against the
  bound value; it fails if the attribute is missing or `$name` is unbound.
- Re-capturing an existing `$name` requires the **same** value, otherwise the
  path fails. These are bounded registers and guards, not regular-language
  backreferences.

## Match consumption

There are two modes; the choice is explicit and the default is first-successor.

- **`FirstSuccessorPerStart`** (default): from each start, commit to the earliest
  successor that satisfies the next step and its transition, per step. A start
  yields at most one match, and commitment is per-step — if the committed
  successor later dead-ends, the start does **not** backtrack to a different
  successor.
- **`ExhaustivePerStart`**: explore every satisfying successor at each step; a
  start may yield multiple matches.

The two modes coincide for two-step patterns and can differ for three or more.
With a pattern `A -> B -> C` where a start sees two candidate `B`s, first-successor
commits to the first `B` and fails if no `C` follows it, even when the second `B`
could have reached a `C`; exhaustive would find that completion.

## Overlapping matches and match identity

**Every** start position is reported, and match spans may overlap.
Leftmost-longest or non-overlapping display is a presentation concern, not part
of the core. A match is identified by its partition plus its participating event
indices. Results are ordered, within a partition, lexicographically by
participating indices; partitions appear in first-seen order. A match carries its
partition, participating indices, start and end timestamps, and captured
bindings.

## Near-miss explanations

Explanation is **existence-based and independent of consumption mode**: a start
is a near-miss if and only if no full match exists from it, explored
exhaustively. Starts that can complete are reported as matches, not near-misses.
A near-miss reports the deepest reachable partial path and one nearest reason for
the failed step. See [explanations](explanations.md) for the detail and the
non-guarantees.

## Out of scope for 0.1

Strict adjacency, alternation, quantifiers and grouping, a full DSL,
time-series eventisation, approximate matching, mining, streaming and late data,
and distributed execution. See [limitations](limitations.md).
