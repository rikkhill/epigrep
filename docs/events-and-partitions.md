# Events and partitions

Epigrep matches over events you have already parsed into a small, typed shape. It
does not parse log lines; turning raw text into events is your job (and a
deliberate boundary — see [limitations](limitations.md)).

## The event shape

An event is `(partition, timestamp, event_type, attributes)`:

```python
Event("api-0", 70, "oom_killed", {"pod": "api-0", "service": "api"})
```

- **partition** — a key that scopes matching. Matches never cross partitions.
  Use whatever isolates one timeline: a pod, a request id, a session, a device.
- **timestamp** — an integer in event time. The unit is yours; windows use the
  same unit.
- **event_type** — the kind of thing that happened, matched by pattern atoms.
- **attributes** — a mapping of names to values. Values may be string, integer,
  float, boolean, or null. Predicates, captures, and references read these.

## Partitioning

Matching happens **within a single partition only**. There are no cross-partition
matches. Partitions are processed in first-seen order, and within the results a
partition's matches appear together.

Choosing the partition key is the main modelling decision. If you want "per pod"
behaviour, partition by pod; if you want "per request", partition by request id.
The same events partitioned differently answer different questions.

## Ordering and ties

Within a partition, events are ordered by `timestamp`, and for equal timestamps
by **input position** — the order they appear in the list you passed. Input
position is the stable tie-breaker used everywhere: windows, absence checks, and
match identity all rely on it.

You do not have to sort the input yourself. The Python `match()` and `explain()`
wrappers group by partition and sort by `(timestamp, input order)` before
matching. (The low-level Rust entry points expect already-sorted input and
reject anything else; the Python layer is the convenient front door.)

Because equal-timestamp events are ordered by input position, two events that
share a timestamp are still strictly ordered for the purposes of "between" and
"within" — which one comes first is decided by where it sat in your list.

## Indices in results

A match or near-miss reports **indices into the sorted event stream** — all
partitions grouped in first-seen order, each sorted by `(timestamp, input
order)`. In the getting-started example, `api-0` contributes three events
(positions 0–2) and `api-1`'s two events follow at positions 3 and 4, so the
`api-1` match reports `[3, 4]`. Match identity is the partition plus its
participating indices.

If you need to map indices back onto an original dataframe, sort the frame by
`(partition, ts)` and use `assume_sorted=True` so the indices line up with rows —
see [loading data](loading-data.md#mapping-matches-back-to-your-rows).

## Next

- [Patterns](patterns.md) — express what you are looking for.
- [Semantics](semantics.md) — the precise rules for matching.
