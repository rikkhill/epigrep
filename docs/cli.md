# Command-line interface

Installing epigrep puts an `epigrep` command on your `PATH` — grep, but for
event *sequences* rather than lines. The same thing is available as
`python -m epigrep` if you prefer.

```sh
epigrep --version
```

The CLI has three subcommands that mirror the Python entrypoints, so it teaches
the same model:

| Command | Does |
|---|---|
| `epigrep match`   | print the spans that match a pattern |
| `epigrep explain` | print near-miss explanations for starts that did not match |
| `epigrep schema`  | summarise the event types, attributes, and partitions in a stream |

## Input: JSONL events

Events are read as **JSONL** — one JSON object per line, in the same shape as the
[event model](events-and-partitions.md):

```json
{"partition": "api-0", "ts": 0,  "typ": "config_reload"}
{"partition": "api-0", "ts": 30, "typ": "readiness_success"}
{"partition": "api-0", "ts": 70, "typ": "oom_killed"}
{"partition": "api-1", "ts": 0,  "typ": "config_reload"}
{"partition": "api-1", "ts": 90, "typ": "oom_killed", "attrs": {"pod": "api-1"}}
```

`attrs` is optional. Pass a file path, `-`, or nothing at all to read from
standard input, so the CLI drops into pipelines:

```sh
cat events.jsonl | epigrep schema -
```

## Patterns

A pattern comes from one of two flags:

- `--pattern-json FILE` — a [JSON pattern AST](patterns.md#the-json-pattern-format-recommended-for-tools-and-agents),
  the **stable** construction surface. Produce one from the builder with
  `Pattern(...).to_json()`.
- `--pattern TEXT` — the text DSL. This is **experimental** and outside the 0.1
  stability guarantee; prefer the JSON form for anything you want to keep.

```sh
epigrep match --pattern-json pattern.json events.jsonl
```

## Output

The default output is JSON, which is easy to pipe into `jq` or another tool. Pass
`--format table` for a compact human view.

```sh
$ epigrep match --pattern-json pattern.json --format table events.jsonl
api-1  [0..90]  config_reload -> oom_killed

$ epigrep explain --pattern-json pattern.json --format table events.jsonl
api-0  reached [0]; would match if readiness_success at 1 were absent (before oom_killed at 2)

$ epigrep schema --format table events.jsonl
events:     5
partitions: 2 ['api-0', 'api-1']
time range: [0, 90]
event types:
  config_reload  (x2)
      pod: string
  ...
```

In JSON mode, `match` emits one object per span (`partition`, `start`, `end`,
`indices`, `types`, `captures`) and `explain` emits one per near-miss, including
a ready-made `summary` string.

## Exit codes

`match` follows **grep's exit-code contract**, so it composes in shell logic:

| Code | Meaning |
|---|---|
| `0` | at least one match |
| `1` | no match |
| `2` | error (bad pattern, unreadable file, malformed JSONL) |

```sh
# Only deploy if no pod has the config-reload → OOM sequence.
if ! epigrep match --pattern-json pattern.json events.jsonl >/dev/null; then
    ./deploy.sh
fi
```

`explain` and `schema` return `0` on success and `2` on error.

## Other flags

- `--exhaustive` (on `match`) — emit every satisfying successor per start instead
  of committing to the first one. See [semantics](semantics.md).
- `--assume-sorted` — declare that the input is already grouped by partition and
  sorted by `(ts, input order)`, skipping the internal sort. With this set, the
  `indices` in the output line up with input line order.

## Next

- [Loading data](loading-data.md) — the Python ingestion helpers behind the CLI.
- [Patterns](patterns.md) — build the patterns you feed it.
