"""CLI behaviour: subcommands, output formats, and grep-style exit codes."""

import json

import pytest

from epigrep import cli

# Two pods; api-1 is a config_reload -> oom within window (a match), api-0 has a
# readiness_success in between (blocked).
EVENTS_JSONL = "\n".join(
    json.dumps(row)
    for row in [
        {"partition": "api-0", "ts": 0, "typ": "config_reload"},
        {"partition": "api-0", "ts": 30, "typ": "readiness_success"},
        {"partition": "api-0", "ts": 70, "typ": "oom_killed"},
        {"partition": "api-1", "ts": 0, "typ": "config_reload"},
        {"partition": "api-1", "ts": 90, "typ": "oom_killed"},
    ]
)

PATTERN_AST = {
    "steps": [
        {
            "atom": {
                "event_type": "config_reload",
                "predicates": [],
                "reference_predicates": [],
                "captures": [],
            },
            "transition_from_previous": None,
        },
        {
            "atom": {
                "event_type": "oom_killed",
                "predicates": [],
                "reference_predicates": [],
                "captures": [],
            },
            "transition_from_previous": {
                "max_elapsed": 120,
                "absence": [
                    {
                        "event_type": "readiness_success",
                        "predicates": [],
                        "reference_predicates": [],
                        "captures": [],
                    }
                ],
            },
        },
    ],
    "consumption": "FirstSuccessorPerStart",
}


@pytest.fixture()
def data_file(tmp_path):
    path = tmp_path / "events.jsonl"
    path.write_text(EVENTS_JSONL + "\n")
    return path


@pytest.fixture()
def pattern_file(tmp_path):
    path = tmp_path / "pattern.json"
    path.write_text(json.dumps(PATTERN_AST))
    return path


def test_match_found_exits_zero_and_emits_json(data_file, pattern_file, capsys):
    code = cli.main(["match", "--pattern-json", str(pattern_file), str(data_file)])
    out = json.loads(capsys.readouterr().out)
    assert code == cli.EXIT_OK
    assert [row["partition"] for row in out] == ["api-1"]
    assert out[0]["types"] == ["config_reload", "oom_killed"]


def test_match_none_exits_one(tmp_path, pattern_file, capsys):
    # A stream with no oom_killed -> no match -> grep-style exit code 1.
    data = tmp_path / "nomatch.jsonl"
    data.write_text(json.dumps({"partition": "p", "ts": 0, "typ": "config_reload"}) + "\n")
    code = cli.main(["match", "--pattern-json", str(pattern_file), str(data)])
    assert json.loads(capsys.readouterr().out) == []
    assert code == cli.EXIT_NO_MATCH


def test_match_reads_stdin(monkeypatch, pattern_file, capsys):
    import io

    monkeypatch.setattr("sys.stdin", io.StringIO(EVENTS_JSONL))
    code = cli.main(["match", "--pattern-json", str(pattern_file)])
    assert code == cli.EXIT_OK
    assert len(json.loads(capsys.readouterr().out)) == 1


def test_explain_emits_near_miss(data_file, pattern_file, capsys):
    code = cli.main(["explain", "--pattern-json", str(pattern_file), str(data_file)])
    out = json.loads(capsys.readouterr().out)
    assert code == cli.EXIT_OK
    reasons = {row["reason"] for row in out}
    assert "absence_blocked" in reasons
    assert all("summary" in row for row in out)


def test_schema_summarises_stream(data_file, capsys):
    code = cli.main(["schema", str(data_file)])
    schema = json.loads(capsys.readouterr().out)
    assert code == cli.EXIT_OK
    assert schema["event_count"] == 5
    assert set(schema["partitions"]) == {"api-0", "api-1"}
    assert set(schema["event_types"]) == {
        "config_reload",
        "readiness_success",
        "oom_killed",
    }


def test_table_format_is_human_readable(data_file, pattern_file, capsys):
    code = cli.main(
        ["match", "--pattern-json", str(pattern_file), "--format", "table", str(data_file)]
    )
    out = capsys.readouterr().out
    assert code == cli.EXIT_OK
    assert "api-1" in out
    assert "config_reload -> oom_killed" in out


def test_bad_pattern_file_exits_error(data_file, capsys):
    code = cli.main(["match", "--pattern-json", "/no/such/pattern.json", str(data_file)])
    assert code == cli.EXIT_ERROR
    assert "epigrep:" in capsys.readouterr().err


def test_malformed_jsonl_exits_error(tmp_path, pattern_file, capsys):
    data = tmp_path / "bad.jsonl"
    data.write_text("{not json}\n")
    code = cli.main(["match", "--pattern-json", str(pattern_file), str(data)])
    assert code == cli.EXIT_ERROR
    assert "line 1" in capsys.readouterr().err


def test_missing_required_field_exits_error(tmp_path, pattern_file, capsys):
    data = tmp_path / "missing.jsonl"
    data.write_text(json.dumps({"partition": "p", "ts": 0}) + "\n")
    code = cli.main(["match", "--pattern-json", str(pattern_file), str(data)])
    assert code == cli.EXIT_ERROR
    assert "typ" in capsys.readouterr().err
