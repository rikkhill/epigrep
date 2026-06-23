"""SPMF dataset loader + a ground-truth sequential-pattern recovery check."""

from pathlib import Path

import epigrep
from epigrep import Pattern, match
from epigrep.datasets import (
    SPMF_SAMPLE,
    load_spmf,
    load_spmf_file,
    parse_spmf,
    sample_spmf_events,
)

FIXTURE = Path(__file__).resolve().parents[3] / "examples" / "datasets" / "spmf-sample.txt"


def _partitions_with_match(events, pattern):
    return {m.partition for m in match(pattern, events)}


def test_parse_spmf_itemset_timestamps():
    records = parse_spmf("1 2 -1 3 -1 -2")
    # Two items in the first itemset share position 0; the third is position 1.
    assert records == [
        {"sequence": 0, "position": 0, "item": "1"},
        {"sequence": 0, "position": 0, "item": "2"},
        {"sequence": 0, "position": 1, "item": "3"},
    ]


def test_load_spmf_sample_shape():
    events = sample_spmf_events()
    # Four sequences -> four partitions.
    assert {e.partition for e in events} == {"seq-0", "seq-1", "seq-2", "seq-3"}
    # Event types are the item tokens; no attributes.
    assert all(e.attrs == {} for e in events)


def test_recovers_discriminating_pattern_4_then_5():
    """"4 then 5" occurs only in sequence 1 of the canonical SPMF example."""
    events = sample_spmf_events()
    pattern = Pattern.event("4").then("5").build()
    assert _partitions_with_match(events, pattern) == {"seq-1"}


def test_recovers_ubiquitous_pattern_1_then_3():
    """"1 then 3" occurs in every sequence of the sample."""
    events = sample_spmf_events()
    pattern = Pattern.event("1").then("3").build()
    assert _partitions_with_match(events, pattern) == {
        "seq-0",
        "seq-1",
        "seq-2",
        "seq-3",
    }


def test_fixture_file_matches_bundled_sample():
    assert FIXTURE.exists()
    from_file = load_spmf_file(FIXTURE)
    from_text = load_spmf(SPMF_SAMPLE)
    assert [(e.partition, e.ts, e.typ) for e in from_file] == [
        (e.partition, e.ts, e.typ) for e in from_text
    ]
