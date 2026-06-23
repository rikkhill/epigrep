#!/usr/bin/env python3
"""Generate the semantic golden-fixture corpus from hand-specified expectations.

Each fixture pins one clause of the semantics contract (see
projects:epigrep:semantics) as a tiny, human-checkable example: events, a
pattern (built through the stable Python builder so the emitted JSON AST is
always valid), and the *hand-written* expected matches and near-misses.

The generator does two things before writing a fixture file:

  1. asserts the authored event list is already in canonical sorted order, so a
     fixture's indices are simply positions in that list (no hidden re-sort), and
  2. runs the live matcher and asserts it agrees with the hand-specified
     expectation. A disagreement means either the authored expectation is wrong
     or there is a genuine matcher bug — stop and investigate, do not paper over.

The committed fixtures are the source of truth for the Rust and Python harnesses
(`crates/epigrep-core/tests/semantic_fixtures.rs`,
`crates/epigrep-py/tests/test_semantic_fixtures.py`). Regenerate with:

    python tests/semantic-fixtures/_generate.py
"""

from __future__ import annotations

import json
from pathlib import Path

import epigrep
from epigrep import Event, Pattern, sort_events

HERE = Path(__file__).resolve().parent


def ev(partition, ts, typ, **attrs):
    return Event(partition, ts, typ, attrs)


def match_summary(matches):
    return sorted(
        (m.partition, list(m.indices), dict(m.captures)) for m in matches
    )


def near_miss_summary(misses):
    return sorted((m.partition, list(m.indices), m.reason) for m in misses)


def expected_key(rows, has_captures):
    out = []
    for row in rows:
        if has_captures:
            out.append((row["partition"], row["indices"], row.get("captures", {})))
        else:
            out.append((row["partition"], row["indices"], row["reason"]))
    return sorted(out)


def with_consumption(pattern, consumption):
    """Return the pattern's JSON AST, overriding the consumption mode."""
    ast = json.loads(pattern.to_json())
    ast["consumption"] = consumption
    return ast


# Each fixture: name, semantics_clause, rationale, events, the built pattern (or a
# JSON AST via consumption override), and hand-specified expected results.
FIXTURES = []


def fixture(name, clause, rationale, events, pattern, expected_matches,
            expected_near_misses, consumption="FirstSuccessorPerStart"):
    ast = with_consumption(pattern, consumption)
    fixture_raw(name, clause, rationale, events, ast, expected_matches,
                expected_near_misses)


def fixture_raw(name, clause, rationale, events, pattern_ast, expected_matches,
                expected_near_misses):
    """Register a fixture from a hand-authored JSON AST.

    Used for patterns the stable Python builder cannot express — notably absence
    atoms carrying predicates or reference predicates.
    """
    FIXTURES.append(
        {
            "name": name,
            "semantics_clause": clause,
            "rationale": rationale,
            "events": events,
            "pattern_ast": pattern_ast,
            "expected_matches": expected_matches,
            "expected_near_misses": expected_near_misses,
        }
    )


# --- Helpers for hand-authored ASTs (the builder cannot reach absence guards) ---

def atom(event_type, predicates=None, references=None, captures=None):
    return {
        "event_type": event_type,
        "predicates": predicates or [],
        "reference_predicates": references or [],
        "captures": captures or [],
    }


def predicate(attribute, operator, value):
    return {"attribute": attribute, "operator": operator, "value": value}


def reference(attribute, operator, binding):
    return {"attribute": attribute, "operator": operator, "binding": binding}


def capture(name, attribute):
    return {"name": name, "attribute": attribute}


def transition(max_elapsed=None, absence=None):
    return {"max_elapsed": max_elapsed, "absence": absence or []}


def step(atom_, transition_=None):
    return {"atom": atom_, "transition_from_previous": transition_}


def raw_ast(steps, consumption="FirstSuccessorPerStart"):
    return {"steps": steps, "consumption": consumption}


A_then_B = Pattern.event("A").then("B").build()

fixture(
    "partition-isolation",
    "Partitioning: matches never cross partitions.",
    "Partition 'a' has A then B and matches; partition 'b' has a lone A that "
    "cannot borrow partition 'a''s B. No cross-partition match is produced.",
    [
        {"partition": "a", "ts": 0, "typ": "A"},
        {"partition": "a", "ts": 1, "typ": "B"},
        {"partition": "b", "ts": 0, "typ": "A"},
    ],
    A_then_B,
    [{"partition": "a", "indices": [0, 1], "captures": {}}],
    [{"partition": "b", "indices": [2], "reason": "no_successor"}],
)

fixture(
    "same-timestamp-tiebreak-match",
    "Ordering: equal timestamps are ordered by input position.",
    "A and B share ts=0 but A appears first in input, so B counts as 'after' A.",
    [
        {"partition": "a", "ts": 0, "typ": "A"},
        {"partition": "a", "ts": 0, "typ": "B"},
    ],
    A_then_B,
    [{"partition": "a", "indices": [0, 1], "captures": {}}],
    [],
)

fixture(
    "same-timestamp-tiebreak-no-match",
    "Ordering: equal timestamps are ordered by input position.",
    "B appears before A at the same ts, so there is no B strictly after A.",
    [
        {"partition": "a", "ts": 0, "typ": "B"},
        {"partition": "a", "ts": 0, "typ": "A"},
    ],
    A_then_B,
    [],
    [{"partition": "a", "indices": [1], "reason": "no_successor"}],
)

fixture(
    "non-contiguous-match",
    "Sequencing matches non-contiguous events.",
    "An unrelated X sits between A and B; A -> B still matches across it.",
    [
        {"partition": "a", "ts": 0, "typ": "A"},
        {"partition": "a", "ts": 1, "typ": "X"},
        {"partition": "a", "ts": 2, "typ": "B"},
    ],
    A_then_B,
    [{"partition": "a", "indices": [0, 2], "captures": {}}],
    [],
)

A_then_B_within5 = Pattern.event("A").then("B", within=5).build()

fixture(
    "inclusive-window-boundary",
    "Windows are inclusive: elapsed == within still matches.",
    "B falls exactly 5 units after A with within=5, on the boundary, and matches.",
    [
        {"partition": "a", "ts": 0, "typ": "A"},
        {"partition": "a", "ts": 5, "typ": "B"},
    ],
    A_then_B_within5,
    [{"partition": "a", "indices": [0, 1], "captures": {}}],
    [],
)

fixture(
    "window-exceeded",
    "Windows are exclusive past the bound: elapsed > within fails.",
    "B is 6 units after A with within=5; the only candidate is just out of "
    "window, giving a window_exceeded near-miss.",
    [
        {"partition": "a", "ts": 0, "typ": "A"},
        {"partition": "a", "ts": 6, "typ": "B"},
    ],
    A_then_B_within5,
    [],
    [{"partition": "a", "indices": [0], "reason": "window_exceeded"}],
)

fixture(
    "zero-duration-window",
    "A within=0 window admits same-timestamp successors ordered after the start.",
    "B shares A's timestamp but follows it in input order; elapsed 0 <= 0 matches.",
    [
        {"partition": "a", "ts": 0, "typ": "A"},
        {"partition": "a", "ts": 0, "typ": "B"},
    ],
    Pattern.event("A").then("B", within=0).build(),
    [{"partition": "a", "indices": [0, 1], "captures": {}}],
    [],
)

A_no_C_B = Pattern.event("A").then("B", no="C").build()

fixture(
    "absence-between-blocks",
    "Absence-between: a forbidden event between frontier and successor blocks.",
    "A C sits between A and B, so 'A -> B with no C between' is blocked.",
    [
        {"partition": "a", "ts": 0, "typ": "A"},
        {"partition": "a", "ts": 1, "typ": "C"},
        {"partition": "a", "ts": 2, "typ": "B"},
    ],
    A_no_C_B,
    [],
    [{"partition": "a", "indices": [0], "reason": "absence_blocked"}],
)

fixture(
    "absence-between-clear",
    "Absence-between: with no forbidden event present, the match stands.",
    "No C exists, so 'A -> B with no C between' matches cleanly.",
    [
        {"partition": "a", "ts": 0, "typ": "A"},
        {"partition": "a", "ts": 2, "typ": "B"},
    ],
    A_no_C_B,
    [{"partition": "a", "indices": [0, 1], "captures": {}}],
    [],
)

fixture(
    "capture-and-reference-equality",
    "Captures bind a value; a reference predicate constrains a later event to it.",
    "deploy captures pod as $p; oom must have the same pod. 'a' matches and "
    "captures pod=api; 'b' has a mismatched oom pod, a predicate_failed near-miss.",
    [
        {"partition": "a", "ts": 0, "typ": "deploy", "attrs": {"pod": "api"}},
        {"partition": "a", "ts": 1, "typ": "oom", "attrs": {"pod": "api"}},
        {"partition": "b", "ts": 0, "typ": "deploy", "attrs": {"pod": "worker"}},
        {"partition": "b", "ts": 1, "typ": "oom", "attrs": {"pod": "db"}},
    ],
    Pattern.event("deploy").capture("pod", "p").then("oom").where_ref_eq("pod", "p").build(),
    [{"partition": "a", "indices": [0, 1], "captures": {"p": "api"}}],
    [{"partition": "b", "indices": [2], "reason": "predicate_failed"}],
)

fixture(
    "missing-attribute-predicate",
    "A predicate over a missing attribute fails (no match).",
    "B carries no 'sev' attribute, so B[sev >= 3] cannot be satisfied.",
    [
        {"partition": "a", "ts": 0, "typ": "A"},
        {"partition": "a", "ts": 1, "typ": "B"},
    ],
    Pattern.event("A").then("B").where_gte("sev", 3).build(),
    [],
    [{"partition": "a", "indices": [0], "reason": "predicate_failed"}],
)

fixture(
    "numeric-float-predicate",
    "Numeric comparisons hold across int and float values.",
    "B has float val=3.5 and B[val >= 3] (an int bound) matches.",
    [
        {"partition": "a", "ts": 0, "typ": "A"},
        {"partition": "a", "ts": 1, "typ": "B", "attrs": {"val": 3.5}},
    ],
    Pattern.event("A").then("B").where_gte("val", 3).build(),
    [{"partition": "a", "indices": [0, 1], "captures": {}}],
    [],
)

fixture(
    "overlapping-starts-reported",
    "Overlapping matches from distinct starts are all reported.",
    "Two A starts each reach the same later B; both [0,2] and [1,2] are returned.",
    [
        {"partition": "a", "ts": 0, "typ": "A"},
        {"partition": "a", "ts": 1, "typ": "A"},
        {"partition": "a", "ts": 2, "typ": "B"},
    ],
    A_then_B,
    [
        {"partition": "a", "indices": [0, 2], "captures": {}},
        {"partition": "a", "indices": [1, 2], "captures": {}},
    ],
    [],
)

# A three-step dead-end: the first B cannot reach a C in its tight window. The two
# fixtures below share events and pattern and differ ONLY in consumption mode.
DEAD_END_EVENTS = [
    {"partition": "p", "ts": 0, "typ": "A"},
    {"partition": "p", "ts": 0, "typ": "B"},
    {"partition": "p", "ts": 5, "typ": "C"},
    {"partition": "p", "ts": 5, "typ": "B"},
    {"partition": "p", "ts": 5, "typ": "C"},
]
DEAD_END_PATTERN = Pattern.event("A").then("B").then("C", within=1).build()

fixture(
    "first-successor-dead-end",
    "FirstSuccessorPerStart commits to the first successor and does not backtrack; "
    "explanation is existence-based and consumption-independent.",
    "A commits to the first B (ts 0) and no C lands within 1 of it, so match() "
    "returns nothing. explain() still reports NO near-miss, because a full match "
    "exists from this start via the later B (explanation is existence-based, not "
    "consumption-based) — the contrast with exhaustive-finds-continuation.",
    DEAD_END_EVENTS,
    DEAD_END_PATTERN,
    [],
    [],
    consumption="FirstSuccessorPerStart",
)

fixture(
    "exhaustive-finds-continuation",
    "ExhaustivePerStart explores every successor, so the later B path matches.",
    "The same start that dead-ends under first-successor finds A -> B(ts5) -> "
    "C(ts5) under exhaustive consumption.",
    DEAD_END_EVENTS,
    DEAD_END_PATTERN,
    [{"partition": "p", "indices": [0, 3, 4], "captures": {}}],
    [],
    consumption="ExhaustivePerStart",
)

# Absence atoms carrying predicates / references — not expressible via the builder,
# so authored as raw ASTs.

fixture_raw(
    "absence-with-predicate",
    "An absent atom's own predicate decides whether it blocks: only a matching "
    "absent event blocks.",
    "'A -> B with no C[sev >= 3] between'. In partition a the intervening C has "
    "sev=1, fails the absent atom's predicate, and does NOT block, so A->B "
    "matches. In partition b the C has sev=5, matches, and blocks.",
    [
        {"partition": "a", "ts": 0, "typ": "A"},
        {"partition": "a", "ts": 1, "typ": "C", "attrs": {"sev": 1}},
        {"partition": "a", "ts": 2, "typ": "B"},
        {"partition": "b", "ts": 0, "typ": "A"},
        {"partition": "b", "ts": 1, "typ": "C", "attrs": {"sev": 5}},
        {"partition": "b", "ts": 2, "typ": "B"},
    ],
    raw_ast(
        [
            step(atom("A")),
            step(
                atom("B"),
                transition(
                    absence=[atom("C", predicates=[predicate("sev", "Gte", {"Int": 3})])]
                ),
            ),
        ]
    ),
    [{"partition": "a", "indices": [0, 2], "captures": {}}],
    [{"partition": "b", "indices": [3], "reason": "absence_blocked"}],
)

fixture_raw(
    "absence-with-captured-reference",
    "An absence guard can reference a captured binding: only an absent event that "
    "matches the binding blocks.",
    "A captures user_id as $u; 'A -> B with no warning[user_id == $u] between'. In "
    "partition a the warning has a different user (u2) and does not block, so A->B "
    "matches and captures u=u1. In partition b the warning shares the captured "
    "user (u1) and blocks.",
    [
        {"partition": "a", "ts": 0, "typ": "A", "attrs": {"user_id": "u1"}},
        {"partition": "a", "ts": 1, "typ": "warning", "attrs": {"user_id": "u2"}},
        {"partition": "a", "ts": 2, "typ": "B"},
        {"partition": "b", "ts": 0, "typ": "A", "attrs": {"user_id": "u1"}},
        {"partition": "b", "ts": 1, "typ": "warning", "attrs": {"user_id": "u1"}},
        {"partition": "b", "ts": 2, "typ": "B"},
    ],
    raw_ast(
        [
            step(atom("A", captures=[capture("u", "user_id")])),
            step(
                atom("B"),
                transition(
                    absence=[
                        atom("warning", references=[reference("user_id", "Eq", "u")])
                    ]
                ),
            ),
        ]
    ),
    [{"partition": "a", "indices": [0, 2], "captures": {"u": "u1"}}],
    [{"partition": "b", "indices": [3], "reason": "absence_blocked"}],
)


def build():
    HERE.mkdir(parents=True, exist_ok=True)
    written = []
    for index, spec in enumerate(FIXTURES, start=1):
        events = [
            Event(e["partition"], e["ts"], e["typ"], e.get("attrs", {}))
            for e in spec["events"]
        ]
        # 1) Authored events must already be in canonical sorted order, so that
        #    indices are plain positions in the list.
        sorted_events = sort_events(events)
        authored = [(e.partition, e.ts, e.typ) for e in events]
        canonical = [(e.partition, e.ts, e.typ) for e in sorted_events]
        assert authored == canonical, (
            f"{spec['name']}: events are not in canonical sorted order; "
            f"authored {authored} != sorted {canonical}"
        )

        pattern = epigrep.pattern_from_json(json.dumps(spec["pattern_ast"]))

        # 2) Live matcher must agree with the hand-specified expectation.
        got_matches = match_summary(epigrep.match(pattern, events))
        want_matches = expected_key(spec["expected_matches"], has_captures=True)
        assert got_matches == want_matches, (
            f"{spec['name']}: matches mismatch\n  want {want_matches}\n  got  {got_matches}"
        )

        got_misses = near_miss_summary(epigrep.explain(pattern, events))
        want_misses = expected_key(spec["expected_near_misses"], has_captures=False)
        assert got_misses == want_misses, (
            f"{spec['name']}: near-misses mismatch\n  want {want_misses}\n  got  {got_misses}"
        )

        out = {
            "name": spec["name"],
            "semantics_clause": spec["semantics_clause"],
            "rationale": spec["rationale"],
            "events": spec["events"],
            "pattern_json": spec["pattern_ast"],
            "expected_matches": spec["expected_matches"],
            "expected_near_misses": spec["expected_near_misses"],
        }
        path = HERE / f"{index:02d}-{spec['name']}.json"
        path.write_text(json.dumps(out, indent=2) + "\n")
        written.append(path.name)

    print(f"wrote {len(written)} fixtures to {HERE}:")
    for name in written:
        print(f"  {name}")


if __name__ == "__main__":
    build()
