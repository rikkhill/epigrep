"""End-to-end test for the epigrep MCP stdio server.

Launches ``server.py`` as a real subprocess and drives it over stdio exactly as
an MCP client would — initialize, tools/list, tools/call — asserting that a
planted pattern round-trips through the protocol and recovers its match, that a
malformed pattern comes back as a repairable ``isError`` result rather than
crashing the server, and that an unknown method/tool is a clean JSON-RPC error.

Standard library only (subprocess + json), matching the server's dependency-light
posture. Run with ``.venv/bin/python -m pytest apps/epigrep-mcp/test_server.py``.
"""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

import pytest

from epigrep import Pattern

SERVER = Path(__file__).with_name("server.py")

# A tiny planted case: config_reload followed (within window) by oom_killed, with
# an unrelated readiness_success in between as noise. The pattern must match
# indices [0, 2].
EVENTS = [
    {"partition": "svc", "ts": 0, "typ": "config_reload"},
    {"partition": "svc", "ts": 30, "typ": "readiness_success"},
    {"partition": "svc", "ts": 70, "typ": "oom_killed"},
]
PATTERN_JSON = Pattern.event("config_reload").then("oom_killed", within=120).build().to_json()


class Client:
    """Minimal MCP stdio client for the test."""

    def __init__(self, proc: subprocess.Popen):
        self.proc = proc
        self._id = 0

    def request(self, method: str, params: dict | None = None) -> dict:
        self._id += 1
        self._send({"jsonrpc": "2.0", "id": self._id, "method": method, "params": params or {}})
        line = self.proc.stdout.readline()
        assert line, f"server closed the stream without replying to {method}"
        return json.loads(line)

    def notify(self, method: str, params: dict | None = None) -> None:
        self._send({"jsonrpc": "2.0", "method": method, "params": params or {}})

    def _send(self, obj: dict) -> None:
        self.proc.stdin.write(json.dumps(obj) + "\n")
        self.proc.stdin.flush()


@pytest.fixture()
def client():
    proc = subprocess.Popen(
        [sys.executable, str(SERVER)],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    try:
        c = Client(proc)
        init = c.request("initialize", {"protocolVersion": "2025-06-18", "capabilities": {}})
        assert init["result"]["serverInfo"]["name"] == "epigrep-mcp"
        assert init["result"]["protocolVersion"] == "2025-06-18"
        c.notify("notifications/initialized")
        yield c
    finally:
        proc.stdin.close()
        try:
            proc.wait(timeout=5)
        except subprocess.TimeoutExpired:  # pragma: no cover
            proc.kill()


def _tool_payload(response: dict) -> dict:
    """Pull the structured dict back out of an MCP tool result."""
    result = response["result"]
    # structuredContent and the text content must agree.
    text_payload = json.loads(result["content"][0]["text"])
    assert result["structuredContent"] == text_payload
    return result


def test_tools_list_exposes_the_three_surfaces(client):
    names = {t["name"] for t in client.request("tools/list")["result"]["tools"]}
    assert names == {"describe_schema", "run", "explain"}


def test_run_recovers_the_planted_match(client):
    response = client.request(
        "tools/call", {"name": "run", "arguments": {"pattern": PATTERN_JSON, "events": EVENTS}}
    )
    result = _tool_payload(response)
    assert result["isError"] is False
    payload = result["structuredContent"]
    assert payload["ok"] is True
    assert payload["match_count"] == 1
    assert payload["matches"][0]["indices"] == [0, 2]


def test_describe_schema_reports_event_types(client):
    response = client.request(
        "tools/call", {"name": "describe_schema", "arguments": {"events": EVENTS}}
    )
    payload = _tool_payload(response)["structuredContent"]
    assert payload["ok"] is True
    types = payload["schema"]["event_types"]
    # event_types may be a list or a mapping keyed by type name; both must mention
    # the planted types.
    haystack = json.dumps(types)
    assert "config_reload" in haystack and "oom_killed" in haystack


def test_bad_pattern_is_a_repairable_error_not_a_crash(client):
    response = client.request(
        "tools/call",
        {"name": "run", "arguments": {"pattern": {"steps": "not-a-list"}, "events": EVENTS}},
    )
    result = _tool_payload(response)
    assert result["isError"] is True
    payload = result["structuredContent"]
    assert payload["ok"] is False
    assert payload["stage"] == "pattern"
    # The server is still alive and serving after a bad call.
    assert client.request("tools/list")["result"]["tools"]


def test_missing_argument_is_reported(client):
    response = client.request(
        "tools/call", {"name": "run", "arguments": {"events": EVENTS}}
    )
    payload = _tool_payload(response)["structuredContent"]
    assert payload["ok"] is False
    assert payload["stage"] == "arguments"


def test_unknown_tool_is_a_json_rpc_error(client):
    response = client.request(
        "tools/call", {"name": "nonesuch", "arguments": {}}
    )
    assert response["error"]["code"] == -32602
    assert "nonesuch" in response["error"]["message"]


def test_unknown_method_is_a_json_rpc_error(client):
    response = client.request("does/not/exist")
    assert response["error"]["code"] == -32601
