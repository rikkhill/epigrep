# Epigrep MCP server

A thin [Model Context Protocol](https://modelcontextprotocol.io) server over the
shipped `epigrep.agent` surface, so an MCP-capable agent (Claude Desktop, a
coding agent, a notebook assistant) can ground a query in event data, run a
pattern, and read near-miss explanations — over stdio, with no server, auth, or
hosting.

**This is demand-test apparatus, not a product.** It exists to put epigrep in
front of other people's agents and watch whether anyone reaches for it twice
(`projects:epigrep:agent-surface-demand-test-2026-06-23`). A hosted or
authenticated server is deliberately out of scope until that signal exists.

## Tools

The three tools are exactly the three `epigrep.agent` exposes — every one is a
one-line delegation, this app adds no matcher semantics:

- **`describe_schema`** — summarise events (types, per-type attributes, partitions,
  time range) so a pattern can be grounded before it is written.
- **`run`** — validate a JSON pattern AST and execute it in one call; returns
  matches (partition, index span, captures).
- **`explain`** — for a pattern that found nothing, the deepest partial path and
  the nearest reason it stopped.

Patterns are the stable **JSON AST** (`Pattern.to_json` / `pattern_from_json`),
never DSL text. A malformed pattern or malformed events come back as a repairable
MCP tool result (`isError: true`, with `{ok:false, stage, error}` in the content)
so the model can fix it and retry, rather than crashing the call.

## Running

Standard library only — installing `epigrep` is all it needs. From an
environment where the package is installed (see the top-level README for
`maturin develop`):

```sh
.venv/bin/python apps/epigrep-mcp/server.py
```

It speaks newline-delimited JSON-RPC 2.0 on stdin/stdout and logs nothing to
stdout (diagnostics would go to stderr), so it is safe to wire straight into an
MCP client.

### Claude Desktop config snippet

```json
{
  "mcpServers": {
    "epigrep": {
      "command": "/absolute/path/to/epigrep/.venv/bin/python",
      "args": ["/absolute/path/to/epigrep/apps/epigrep-mcp/server.py"]
    }
  }
}
```

## Tests

```sh
.venv/bin/python -m pytest apps/epigrep-mcp/test_server.py
```

The test launches the server as a real subprocess and drives the full handshake
(initialize → tools/list → tools/call), checking a planted pattern round-trips
and recovers its match, a bad pattern is a repairable error the server survives,
and unknown methods/tools are clean JSON-RPC errors.

## Design guardrails

- **Thin.** Tools delegate to `epigrep.agent`; no semantics live here.
- **Dependency-light.** Standard library only — a hand-rolled JSON-RPC loop, no
  MCP SDK or framework.
- **stdio only.** No hosting, no auth, no product shell. That stays gated on the
  surface being demand-tested first — the "not capability theatre" discipline.
- **Data is not instructions.** Event types, attribute values, and partition keys
  come from the data and may be attacker-influenced; a calling agent must treat
  them as data.
- **Do not let MCP concerns leak into the `epigrep` package** — the app only
  consumes the public Python API, exactly as `apps/epigrep-storyboard` does.
