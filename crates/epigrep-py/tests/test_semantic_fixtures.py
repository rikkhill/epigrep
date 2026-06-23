"""Run the cross-language semantic golden fixtures through the Python surface.

These fixtures live at the repository root (`tests/semantic-fixtures/`) and are
consumed by both this test and the Rust harness
(`crates/epigrep-core/tests/semantic_fixtures.rs`), so the JSON-AST/builder/FFI
path and the core matcher are both held to the same hand-specified contract.
Regenerate the corpus with `python tests/semantic-fixtures/_generate.py`.
"""

import json
from pathlib import Path

import pytest

import epigrep
from epigrep import Event

FIXTURE_DIR = Path(__file__).resolve().parents[3] / "tests" / "semantic-fixtures"


def fixture_paths():
    return sorted(FIXTURE_DIR.glob("[0-9]*.json"))


def load(path):
    return json.loads(path.read_text())


def events_of(fixture):
    return [
        Event(e["partition"], e["ts"], e["typ"], e.get("attrs", {}))
        for e in fixture["events"]
    ]


def match_key(matches):
    return sorted(
        (m.partition, list(m.indices), dict(m.captures)) for m in matches
    )


def near_miss_key(misses):
    return sorted((m.partition, list(m.indices), m.reason) for m in misses)


def expected_match_key(rows):
    return sorted((r["partition"], r["indices"], r.get("captures", {})) for r in rows)


def expected_near_miss_key(rows):
    return sorted((r["partition"], r["indices"], r["reason"]) for r in rows)


def test_fixture_corpus_is_present():
    # A floor so an empty/missing corpus fails loudly rather than passing vacuously.
    assert len(fixture_paths()) >= 15


@pytest.mark.parametrize("path", fixture_paths(), ids=lambda p: p.stem)
def test_matches_and_near_misses(path):
    fixture = load(path)
    events = events_of(fixture)
    pattern = epigrep.pattern_from_json(json.dumps(fixture["pattern_json"]))

    assert match_key(epigrep.match(pattern, events)) == expected_match_key(
        fixture["expected_matches"]
    )
    assert near_miss_key(epigrep.explain(pattern, events)) == expected_near_miss_key(
        fixture["expected_near_misses"]
    )


@pytest.mark.parametrize("path", fixture_paths(), ids=lambda p: p.stem)
def test_compiled_oracle_parity(path):
    """The compiled backend and the naive oracle must agree on every fixture."""
    fixture = load(path)
    events = epigrep.sort_events(events_of(fixture))
    pattern = epigrep.pattern_from_json(json.dumps(fixture["pattern_json"]))

    compiled = match_key(epigrep.match(pattern, events, assume_sorted=True))
    oracle = match_key(epigrep.match(pattern, events, oracle=True, assume_sorted=True))
    assert compiled == oracle
