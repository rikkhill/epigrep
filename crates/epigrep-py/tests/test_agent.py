"""The minimal agent/tool surface: schema -> AST -> validate -> run -> explain.

These tests stand in for the deterministic half of the demand-test loop: a model
would emit the ASTs, but the validate/run/explain/repair behaviour they depend on
is pinned here, including that bad input comes back as a repairable error rather
than an exception.
"""

import json

from epigrep import Pattern, Event, agent


def ast(pattern):
    """The JSON AST an agent would emit, as a dict."""
    return json.loads(pattern.to_json())


# A config-reload -> OOM stream: api-1 matches; api-0 is blocked by a readiness
# success in between (a useful near-miss).
EVENTS = [
    {"partition": "api-0", "ts": 0, "typ": "config_reload"},
    {"partition": "api-0", "ts": 30, "typ": "readiness_success"},
    {"partition": "api-0", "ts": 70, "typ": "oom_killed"},
    {"partition": "api-1", "ts": 0, "typ": "config_reload"},
    {"partition": "api-1", "ts": 90, "typ": "oom_killed"},
]

PATTERN = (
    Pattern.event("config_reload")
    .then("oom_killed", within=120, no="readiness_success")
    .build()
)


def test_describe_schema_grounds_the_query():
    result = agent.describe_schema(EVENTS)
    assert result["ok"] is True
    schema = result["schema"]
    assert schema["event_count"] == 5
    assert set(schema["partitions"]) == {"api-0", "api-1"}
    assert set(schema["event_types"]) == {
        "config_reload",
        "readiness_success",
        "oom_killed",
    }


def test_run_validates_and_executes_from_json_ast():
    result = agent.run(ast(PATTERN), EVENTS)
    assert result["ok"] is True
    assert result["match_count"] == 1
    assert result["partitions"] == ["api-1"]
    assert result["matches"][0]["types"] == ["config_reload", "oom_killed"]
    assert result["truncated"] is False


def test_run_accepts_event_objects_too():
    events = [Event(e["partition"], e["ts"], e["typ"]) for e in EVENTS]
    result = agent.run(ast(PATTERN), events)
    assert result["ok"] is True
    assert result["match_count"] == 1


def test_run_returns_repairable_error_for_invalid_ast_without_raising():
    broken = {"steps": [], "consumption": "FirstSuccessorPerStart"}  # empty: invalid
    result = agent.run(broken, EVENTS)
    assert result["ok"] is False
    assert result["stage"] == "pattern"
    assert isinstance(result["error"], str) and result["error"]


def test_repair_loop_recovers_after_fixing_the_ast():
    # First attempt: a typo'd consumption mode the validator rejects.
    bad = ast(PATTERN)
    bad["consumption"] = "NoSuchMode"
    first = agent.run(bad, EVENTS)
    assert first["ok"] is False and first["stage"] == "pattern"

    # The agent repairs using the error and retries successfully.
    fixed = ast(PATTERN)
    second = agent.run(fixed, EVENTS)
    assert second["ok"] is True
    assert second["match_count"] == 1


def test_explain_reports_near_misses_with_summaries():
    result = agent.explain(ast(PATTERN), EVENTS)
    assert result["ok"] is True
    assert result["near_miss_count"] >= 1
    reasons = {nm["reason"] for nm in result["near_misses"]}
    assert "absence_blocked" in reasons
    assert all(nm["summary"] for nm in result["near_misses"])


def test_explain_surfaces_pattern_errors_too():
    result = agent.explain("{ not json", EVENTS)
    assert result["ok"] is False
    assert result["stage"] == "pattern"


def test_malformed_event_record_is_a_repairable_error():
    events = EVENTS + [{"partition": "p", "ts": 0}]  # missing 'typ'
    result = agent.run(ast(PATTERN), events)
    assert result["ok"] is False
    assert result["stage"] == "events"
    assert "typ" in result["error"]


def test_limit_truncates_and_flags():
    # Two overlapping starts both reach the same later B.
    events = [
        {"partition": "p", "ts": 0, "typ": "A"},
        {"partition": "p", "ts": 1, "typ": "A"},
        {"partition": "p", "ts": 2, "typ": "B"},
    ]
    pattern = ast(Pattern.event("A").then("B").build())
    result = agent.run(pattern, events, limit=1)
    assert result["ok"] is True
    assert result["match_count"] == 2
    assert len(result["matches"]) == 1
    assert result["truncated"] is True
