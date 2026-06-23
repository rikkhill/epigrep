# Loading data

Matching runs over a list of [`Event`](events-and-partitions.md) objects. You can
build those by hand, but most real data starts life in a dataframe, a list of
records, or a dataset file. epigrep ships one ingestion primitive — `eventise` —
and a couple of conveniences on top of it.

All of these produce the **same sorted `Event` list** the matcher expects, so
whichever door you come in through, the rest of the library behaves identically.

## From records: `eventise`

`eventise` turns any iterable of records into sorted events. You say how to find
the partition, timestamp, and type; everything else can become attributes.

```python
from epigrep import eventise, match, Pattern

records = [
    {"pod": "api-1", "t": 90, "kind": "oom_killed", "service": "api"},
    {"pod": "api-1", "t": 0,  "kind": "config_reload", "service": "api"},
]

events = eventise(records, partition="pod", ts="t", typ="kind")
# events are sorted by (partition, ts); 'service' became an attribute
```

Each of `partition`, `ts`, and `typ` is either a **key** into the record or a
**callable** `record -> value`:

```python
events = eventise(
    records,
    partition=lambda r: f"pod:{r['pod']}",
    ts="t",
    typ="kind",
    attrs=["service"],          # keep only these as attributes
)
```

`attrs` controls which fields become event attributes:

- `None` (default) — every remaining key, except the three you mapped.
- a list of keys — exactly those.
- a callable `record -> dict` — full control.

`ts` is coerced to `int`; `partition` and `typ` to `str`. Numpy/pandas scalars
are converted to native Python values, and any attribute whose value is `None` or
`NaN` is dropped (treated as absent), so frames with missing cells just work.

## From a dataframe: `events_from_frame`

`events_from_frame` is `eventise` specialised to **pandas** and **polars**
`DataFrame`s and **pyarrow** `Table`s, addressed by column name. It is the inverse
of [`events_to_frame`](events-and-partitions.md).

```python
import pandas as pd
from epigrep import events_from_frame

df = pd.DataFrame([
    {"partition": "api-1", "ts": 0,  "typ": "config_reload"},
    {"partition": "api-1", "ts": 90, "typ": "oom_killed", "pod": "api-1"},
])

events = events_from_frame(df, partition_col="partition", ts_col="ts", type_col="typ")
```

`attr_cols=None` keeps every column except the three you mapped; pass a list to
restrict it. No dataframe library is a hard dependency — the function detects the
object you pass, so you only need whichever library you already use.

### Mapping matches back to your rows

`match()` sorts its input and returns `indices` into that **sorted stream**. If
you want to attach matches back onto the original dataframe rows, sort the frame
the same way the matcher does — by `(partition, ts)` — and tell `match` the input
is already ordered:

```python
df_sorted = df.sort_values(["partition", "ts"]).reset_index(drop=True)
events = events_from_frame(
    df_sorted, partition_col="partition", ts_col="ts", type_col="typ", sort=False
)

pattern = Pattern.event("config_reload").then("oom_killed", within=120).build()
for m in match(pattern, events, assume_sorted=True):
    rows = df_sorted.iloc[m.indices]   # the participating rows, in order
    print(rows)
```

With `assume_sorted=True` the event order equals the row order, so `m.indices`
index `df_sorted` directly. This is the supported, predictable round-trip; without
the pre-sort, `indices` still index the internally sorted stream, just not your
original row order.

## From a dataset: `epigrep.datasets`

`epigrep.datasets` loads public event-sequence corpora through the same
eventise contract. The first supported format is **SPMF** sequence-database files
— the standard shape for sequential-pattern-mining datasets.

```python
from epigrep.datasets import load_spmf_file, sample_spmf_events
from epigrep import Pattern, match

events = sample_spmf_events()            # a bundled four-sequence example
# or: events = load_spmf_file("mydata.txt")

# "item 4 then item 5" — event types are the item tokens
pattern = Pattern.event("4").then("5").build()
print({m.partition for m in match(pattern, events)})
```

In SPMF format each line is a sequence, integers are items, `-1` ends an itemset
(co-occurring items, given the same timestamp), and `-2` ends the sequence.
Eventisation maps **sequence id → partition**, **itemset index → timestamp**, and
**item → event type**. A small sample ships at `examples/datasets/spmf-sample.txt`.

Other corpora (LogHub, which needs log-template parsing; BPI/XES event logs, which
need an XES reader) reuse the same `eventise` contract and are not bundled yet.

## Next

- [Command-line interface](cli.md) — the same ingestion, from the shell.
- [Patterns](patterns.md) — what to search for once your data is loaded.
