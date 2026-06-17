import json
from pathlib import Path

import epigrep
from epigrep import Event, Pattern, pattern_from_json


FIXTURE_DIR = Path(__file__).resolve().parents[3] / "examples" / "logs-first"


def fixture_paths():
    return sorted(FIXTURE_DIR.glob("*.json"))


def load_fixture(path):
    return json.loads(path.read_text())


def events_from_fixture(fixture):
    return [
        Event(row["partition"], row["ts"], row["typ"], row.get("attrs", {}))
        for row in fixture["events"]
    ]


def build_from_recipe(recipe):
    current = None
    for step in recipe:
        op = step["op"]
        if op == "event":
            current = Pattern.event(step["typ"])
        elif op == "then":
            current = current.then(step["typ"], within=step.get("within"), no=step.get("no"))
        elif op == "capture":
            current = current.capture(step["attribute"], step["name"])
        elif op == "where_ref_eq":
            current = current.where_ref_eq(step["attribute"], step["name"])
        elif op == "build":
            current = current.build()
        else:
            raise AssertionError(f"unsupported builder op: {op}")
    return current


def match_summary(matches):
    return [
        {
            "partition": match.partition,
            "indices": list(match.indices),
            "captures": dict(match.captures),
        }
        for match in matches
    ]


def near_miss_summary(misses):
    return [
        {
            "partition": miss.partition,
            "indices": list(miss.indices),
            "reason": miss.reason,
        }
        for miss in misses
    ]


def test_logs_first_fixtures_exist():
    assert len(fixture_paths()) == 5


def test_logs_first_fixtures_match_expected_results():
    saw_positive = False
    saw_useful_near_miss = False

    for path in fixture_paths():
        fixture = load_fixture(path)
        events = events_from_fixture(fixture)
        pattern = pattern_from_json(json.dumps(fixture["pattern_json"]))

        assert match_summary(epigrep.match(pattern, events)) == fixture["expected_matches"]
        assert near_miss_summary(epigrep.explain(pattern, events)) == fixture["expected_near_misses"]

        saw_positive = saw_positive or bool(fixture["expected_matches"])
        saw_useful_near_miss = saw_useful_near_miss or any(
            miss["reason"] in {"absence_blocked", "predicate_failed", "window_exceeded"}
            for miss in fixture["expected_near_misses"]
        )

    assert saw_positive
    assert saw_useful_near_miss


def test_builder_recipes_round_trip_to_fixture_json_ast():
    for path in fixture_paths():
        fixture = load_fixture(path)
        pattern = build_from_recipe(fixture["builder"])
        assert json.loads(pattern.to_json()) == fixture["pattern_json"]
