"""Run the logs-first Epigrep examples from their JSON fixtures.

From a source checkout with the package installed locally:

    python examples/logs-first/run.py
"""

from __future__ import annotations

import json
from pathlib import Path

import epigrep
from epigrep import Event, Pattern, pattern_from_json


HERE = Path(__file__).resolve().parent


def events_from_fixture(fixture: dict) -> list[Event]:
    return [
        Event(row["partition"], row["ts"], row["typ"], row.get("attrs", {}))
        for row in fixture["events"]
    ]


def build_from_recipe(recipe: list[dict]):
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
            raise ValueError(f"unsupported builder op: {op}")
    return current


def main() -> None:
    for path in sorted(HERE.glob("*.json")):
        fixture = json.loads(path.read_text())
        events = events_from_fixture(fixture)
        pattern = pattern_from_json(json.dumps(fixture["pattern_json"]))
        builder_pattern = build_from_recipe(fixture["builder"])
        assert json.loads(builder_pattern.to_json()) == fixture["pattern_json"]

        matches = epigrep.match(pattern, events)
        misses = epigrep.explain(pattern, events)
        print(f"\n{fixture['title']}")
        print("matches:")
        for match in matches:
            print(f"  {match.partition} {list(match.indices)} {dict(match.captures)}")
        print("near-misses:")
        for miss in misses:
            print(f"  {miss.partition} {list(miss.indices)} {miss.reason}: {epigrep.near_miss_summary(miss)}")


if __name__ == "__main__":
    main()
