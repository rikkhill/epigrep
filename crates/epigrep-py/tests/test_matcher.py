"""Pytest coverage for the epigrep Python bindings.

These mirror the Rust worked examples and assert the same semantics across the
Python boundary, plus oracle/compiled parity on every demo story.
"""

import pytest

import epigrep
from epigrep import Event, match, parse_pattern, sort_events
from epigrep import data


def idx(matches):
    return [list(m.indices) for m in matches]


def test_basic_sequence():
    events = [Event("p", 0, "A"), Event("p", 1, "B")]
    assert idx(match(parse_pattern("A -> B"), events)) == [[0, 1]]


def test_window_boundary_is_inclusive():
    events = [Event("p", 10, "A"), Event("p", 15, "B")]
    assert idx(match(parse_pattern("A -[<=5]-> B"), events)) == [[0, 1]]


def test_window_excludes_after_boundary():
    events = [Event("p", 10, "A"), Event("p", 16, "B")]
    assert match(parse_pattern("A -[<=5]-> B"), events) == []


def test_absence_between_blocks():
    events = [Event("p", 0, "A"), Event("p", 1, "C"), Event("p", 2, "B")]
    assert match(parse_pattern("A -[no C]-> B"), events) == []


def test_partition_isolation():
    events = [
        Event("child-1", 0, "A"),
        Event("child-1", 1, "B"),
        Event("child-2", 0, "A"),
        Event("child-2", 2, "B"),
    ]
    assert idx(match(parse_pattern("A -> B"), events)) == [[0, 1], [2, 3]]


def test_capture_and_reference_equality():
    events = [
        Event("p", 0, "A", {"user_id": "u1"}),
        Event("p", 1, "B", {"user_id": "u2"}),
        Event("p", 2, "B", {"user_id": "u1"}),
    ]
    matches = match(parse_pattern("A[user_id as $u] -> B[user_id == $u]"), events)
    assert idx(matches) == [[0, 2]]
    assert matches[0].captures == {"u": "u1"}


def test_first_successor_vs_exhaustive():
    story = data.dead_end_story()
    pattern = parse_pattern(story.pattern_text)
    # First-successor commits to the dead-ending B and finds nothing.
    assert idx(match(pattern, story.events)) == []
    # Exhaustive explores the later continuation.
    assert idx(match(pattern, story.events, exhaustive=True)) == [[0, 3, 4]]


def test_numeric_predicate_crosses_int_and_float():
    events = [Event("p", 0, "A", {"score": 1})]
    assert idx(match(parse_pattern("A[score >= 1.0]"), events)) == [[0]]
    assert idx(match(parse_pattern("A[score == 1.0]"), events)) == [[0]]


def test_builder_api():
    pattern = epigrep.Pattern.event("A").then("B", within=5, no="C")
    events = [Event("p", 0, "A"), Event("p", 3, "B")]
    assert idx(match(pattern, events)) == [[0, 1]]


def test_unsorted_input_is_rejected_by_low_level_api():
    # The high-level match() sorts for you; the low-level entry point is strict.
    events = [Event("p", 5, "A"), Event("p", 1, "B")]
    with pytest.raises(ValueError):
        epigrep._core.match_events(parse_pattern("A -> B"), events, False, False)
    # Sorting first makes it well-defined (no match here: A is after B in time).
    assert match(parse_pattern("A -> B"), events) == []


def test_parser_rejects_bad_syntax():
    with pytest.raises(ValueError):
        parse_pattern("A[score ~= 2] -> B")


@pytest.mark.parametrize("story", data.all_stories(), ids=lambda s: s.key)
def test_story_oracle_compiled_parity(story):
    pattern = parse_pattern(story.pattern_text)
    for exhaustive in (False, True):
        compiled = match(pattern, story.events, exhaustive=exhaustive)
        oracle = match(pattern, story.events, exhaustive=exhaustive, oracle=True)
        assert idx(compiled) == idx(oracle)


@pytest.mark.parametrize(
    "story",
    [s for s in data.all_stories() if s.expected is not None],
    ids=lambda s: s.key,
)
def test_story_ground_truth(story):
    pattern = parse_pattern(story.pattern_text)
    found = idx(match(pattern, story.events, exhaustive=story.default_exhaustive))
    assert found == story.expected


def test_pandas_helpers_roundtrip():
    story = data.care_pathway()
    events_frame = epigrep.events_to_frame(story.events)
    assert list(events_frame["typ"])[:1] == ["entered_care"]
    matches_frame = epigrep.matches_to_frame(match(parse_pattern(story.pattern_text), story.events))
    assert len(matches_frame) == 1
    assert matches_frame.iloc[0]["indices"] == [3, 4]


def test_sort_events_is_stable_on_ties():
    # Same timestamp: original order must be preserved as the tie-break.
    events = [Event("p", 5, "A"), Event("p", 5, "B"), Event("p", 5, "C")]
    assert [e.typ for e in sort_events(events)] == ["A", "B", "C"]
