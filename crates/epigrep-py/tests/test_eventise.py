"""Eventisation: record/dataframe ingestion into sorted Event lists."""

import pytest

import epigrep
from epigrep import Event, Pattern, eventise, events_from_frame, match


def test_eventise_key_accessors_and_sorting():
    records = [
        {"who": "p", "t": 5, "kind": "B", "sev": 2},
        {"who": "p", "t": 1, "kind": "A", "sev": 9},
    ]
    events = eventise(records, partition="who", ts="t", typ="kind")
    # Sorted by (partition, ts): the t=1 A comes first.
    assert [(e.partition, e.ts, e.typ) for e in events] == [("p", 1, "A"), ("p", 5, "B")]
    # Remaining keys become attributes.
    assert events[0].attrs == {"sev": 9}


def test_eventise_callable_accessors_and_explicit_attrs():
    records = [{"id": 1, "stamp": 10, "event": "login", "ip": "x", "noise": "drop"}]
    events = eventise(
        records,
        partition=lambda r: f"user-{r['id']}",
        ts="stamp",
        typ="event",
        attrs=["ip"],
    )
    assert events[0].partition == "user-1"
    assert events[0].attrs == {"ip": "x"}  # 'noise' excluded by explicit attr list


def test_eventise_coerces_types_and_drops_null_attrs():
    records = [{"p": 7, "t": 3.0, "typ": 42, "a": None, "b": "keep"}]
    events = eventise(records, partition="p", ts="t", typ="typ")
    e = events[0]
    assert e.partition == "7" and e.ts == 3 and e.typ == "42"
    assert e.attrs == {"b": "keep"}  # None-valued attr dropped


def test_eventise_rejects_bool_timestamp():
    with pytest.raises(TypeError):
        eventise([{"p": "x", "t": True, "k": "A"}], partition="p", ts="t", typ="k")


def test_eventise_result_matches_through_the_engine():
    records = [
        {"p": "svc", "t": 0, "k": "deploy"},
        {"p": "svc", "t": 50, "k": "oom_killed"},
    ]
    events = eventise(records, partition="p", ts="t", typ="k")
    pattern = Pattern.event("deploy").then("oom_killed", within=120).build()
    assert [list(m.indices) for m in match(pattern, events)] == [[0, 1]]


def test_events_from_frame_pandas_round_trips_through_events_to_frame():
    pd = pytest.importorskip("pandas")
    events = [
        Event("api-0", 0, "config_reload", {"pod": "api-0"}),
        Event("api-0", 70, "oom_killed", {"pod": "api-0"}),
    ]
    frame = epigrep.events_to_frame(events)
    rebuilt = events_from_frame(
        frame, partition_col="partition", ts_col="ts", type_col="typ", attr_cols=["pod"]
    )
    assert [(e.partition, e.ts, e.typ, e.attrs) for e in rebuilt] == [
        ("api-0", 0, "config_reload", {"pod": "api-0"}),
        ("api-0", 70, "oom_killed", {"pod": "api-0"}),
    ]


def test_events_from_frame_pandas_default_attr_cols():
    pd = pytest.importorskip("pandas")
    frame = pd.DataFrame(
        [
            {"partition": "n", "ts": 1, "typ": "A", "x": 1},
            {"partition": "n", "ts": 2, "typ": "B", "x": 2},
        ]
    )
    events = events_from_frame(frame, partition_col="partition", ts_col="ts", type_col="typ")
    assert events[0].attrs == {"x": 1}
    assert events[1].attrs == {"x": 2}


def test_events_from_frame_pyarrow():
    pa = pytest.importorskip("pyarrow")
    table = pa.table(
        {
            "partition": ["p", "p"],
            "ts": [2, 1],
            "typ": ["B", "A"],
            "v": [20, 10],
        }
    )
    events = events_from_frame(table, partition_col="partition", ts_col="ts", type_col="typ")
    # Sorted by ts, attribute carried through.
    assert [(e.ts, e.typ, e.attrs) for e in events] == [
        (1, "A", {"v": 10}),
        (2, "B", {"v": 20}),
    ]


def test_events_from_frame_polars_if_available():
    pl = pytest.importorskip("polars")
    frame = pl.DataFrame(
        {"partition": ["p"], "ts": [0], "typ": ["A"], "v": [1]}
    )
    events = events_from_frame(frame, partition_col="partition", ts_col="ts", type_col="typ")
    assert (events[0].partition, events[0].ts, events[0].typ, events[0].attrs) == (
        "p",
        0,
        "A",
        {"v": 1},
    )


def test_events_from_frame_rejects_non_frame():
    with pytest.raises(TypeError):
        events_from_frame(
            [{"partition": "p", "ts": 0, "typ": "A"}],
            partition_col="partition",
            ts_col="ts",
            type_col="typ",
        )


def test_presorted_round_trip_indices_align_with_frame_rows():
    """The documented dataframe round-trip: pre-sort + assume_sorted -> Match
    indices index frame rows directly."""
    pd = pytest.importorskip("pandas")
    frame = pd.DataFrame(
        [
            {"partition": "svc", "ts": 0, "typ": "deploy"},
            {"partition": "svc", "ts": 50, "typ": "oom_killed"},
        ]
    )
    events = events_from_frame(
        frame, partition_col="partition", ts_col="ts", type_col="typ"
    )
    pattern = Pattern.event("deploy").then("oom_killed", within=120).build()
    matches = match(pattern, events, assume_sorted=True)
    assert len(matches) == 1
    rows = frame.iloc[matches[0].indices]
    assert list(rows["typ"]) == ["deploy", "oom_killed"]
