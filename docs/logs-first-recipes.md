# Logs-first recipes

These are the patterns Epigrep was built for: sequences in operational event
streams that are awkward to express otherwise. Each one ships as a runnable
fixture in [`examples/logs-first/`](https://github.com/rikkhill/epigrep/tree/main/examples/logs-first)
with deterministic events, the pattern in builder and JSON form, and the expected
matches and near-misses. Run them all with:

```sh
python examples/logs-first/run.py
```

Throughout, events are partitioned by the thing whose timeline you care about —
a pod, a service instance, a request, a worker.

## Config reload → OOM, nothing in between

A reload followed by an out-of-memory kill within two minutes, with no readiness
success in the gap — the signature of a bad config that starves the process.

```python
Pattern.event("config_reload").then("oom_killed", within=120, no="readiness_success").build()
```

`api-1` matches (`[3, 4]`). `api-0` is a near-miss: `absence_blocked`, because a
`readiness_success` lands between its reload and OOM — the process recovered, so
it is not the failure we are hunting.

## Deploy → error spike → rollback

A deploy followed by an error spike and then a rollback — a deployment that went
wrong and was backed out.

```python
Pattern.event("deploy").then("error_spike", within=60).then("rollback", within=60).build()
```

`svc-a` matches the full three-step sequence (`[0, 1, 2]`). `svc-b` is a
near-miss: it reached the error spike but `no_successor` — no rollback followed,
so the bad deploy was left in place. That near-miss is arguably the more
interesting result.

## Repeated readiness failure → restart

Two readiness failures within 30s of each other and then a restart — a flapping
pod that the orchestrator eventually bounced.

```python
Pattern.event("readiness_failure").then("readiness_failure", within=30).then("restart", within=30).build()
```

`worker-0` matches (`[0, 1, 2]`). `worker-1` produces near-misses where the chain
ran out of successors (`no_successor`). This example also shows that **every**
start is considered: a single worker can contribute both a match from one start
and near-misses from others.

## Fatal error without a prior warning

A request start followed by a fatal error within 60s, with no warning in
between — a failure that gave no notice.

```python
Pattern.event("request_start").then("fatal_error", within=60, no="warning").build()
```

`request-a` matches (`[0, 1]`). `request-b` is `absence_blocked`: a `warning`
landed in the gap, so it does not count as a silent failure.

## Same request throughout (capture equality)

A request that starts, issues a database query, and sends a response — all
*under the same request id*, using a capture to tie the steps together.

```python
(
    Pattern.event("request_start").capture("request_id", "request")
    .then("db_query", within=30).where_ref_eq("request_id", "request")
    .then("response_sent", within=30).where_ref_eq("request_id", "request")
    .build()
)
```

`trace-a` matches and reports the captured binding (`{'request': 'req-1'}`).
`trace-b` is a `predicate_failed` near-miss: a `db_query` existed, but its
`request_id` did not equal the captured value — a different request's query, so
the reference guard rejects it. The explanation names the bound value and the
actual one.

## Reading the output

`run.py` prints, per fixture, the matches (partition, indices, captures) and the
near-misses (partition, indices, reason, and structured detail). The detail is
where the value is: not just that `trace-b` failed, but that it failed because
`request_id == $req` did not hold, with both values shown. See
[explanations](explanations.md) for the reason taxonomy.
