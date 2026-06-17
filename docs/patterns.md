# Patterns

A pattern is a sequence of **steps**. The first step picks a start event; each
later step says what must come next, and on what terms. `A -> B` is
non-contiguous: other events may sit between the matched steps. Adjacency is not
part of the 0.1 surface.

There are two stable ways to construct a pattern and one experimental one.

## The builder (recommended for code)

`Pattern.event(...)` starts a pattern; `.then(...)` adds a step; `.build()`
finishes it.

```python
from epigrep import Pattern

pattern = (
    Pattern.event("config_reload")
    .then("oom_killed", within=120, no="readiness_success")
    .build()
)
```

The step options:

- `within=Δ` — the next event must occur within `Δ` time units of the previous
  participating event. The bound is **inclusive** on both ends.
- `no="X"` — absence: no event of type `X` may lie strictly between the two
  participating events.
- predicates, captures, and references — constraints on attributes (below).

### Predicates, captures, and references

Predicates, captures, and references are added with chained methods that apply to
the step most recently added. Predicates constrain an event's attributes against
a literal — `where_eq`, `where_ne`, `where_gt`, `where_gte`, `where_lt`,
`where_lte`:

```python
pattern = (
    Pattern.event("error").where_eq("level", "fatal")
    .build()
)
```

`capture(attr, name)` binds a value the first time a step matches;
`where_ref_eq(attr, name)` compares a later step against that bound value. This
expresses "same request throughout" — note the capture name is a plain string,
not a `$`-prefixed token:

```python
pattern = (
    Pattern.event("request_start").capture("request_id", "request")
    .then("db_query", within=30).where_ref_eq("request_id", "request")
    .build()
)
```

Re-capturing an existing name requires the same value, so captures act as
bounded registers and guards — not regular-expression backreferences. The exact
comparison rules are in [semantics](semantics.md).

## The JSON pattern format (recommended for tools and agents)

For programmatic use — anything emitting and validating patterns, including LLM
tooling — prefer the JSON format. It round-trips with the builder and is
validated on the way in.

```python
import json
from epigrep import Pattern, pattern_from_json

ast = Pattern.event("A").then("B", within=5).build().to_json()
pattern = pattern_from_json(ast)   # validated; safe to match
print(json.dumps(json.loads(ast), indent=2))
```

A two-step pattern looks like this:

```json
{
  "steps": [
    {
      "atom": {"event_type": "config_reload", "predicates": [], "reference_predicates": [], "captures": []},
      "transition_from_previous": null
    },
    {
      "atom": {"event_type": "oom_killed", "predicates": [], "reference_predicates": [], "captures": []},
      "transition_from_previous": {
        "max_elapsed": 120,
        "absence": [
          {"event_type": "readiness_success", "predicates": [], "reference_predicates": [], "captures": []}
        ]
      }
    }
  ],
  "consumption": "FirstSuccessorPerStart"
}
```

The first step has `transition_from_previous: null`; later steps carry the
window (`max_elapsed`) and absence (`absence`) for the gap before them. This JSON
shape is the stable interchange format; the builder and DSL are surfaces over the
same structure.

## The text DSL (experimental)

A terse text form exists and is used by the example fixtures:

```
request_start[request_id as $req] -[no db_error]-> db_query[request_id == $req]
```

It is convenient for demos and quick experiments, but it is **experimental** and
outside the 0.1 stability guarantee. `parse_pattern(...)` remains importable, but
build the patterns you depend on with the builder or the JSON format.

## Match consumption

A pattern carries a consumption mode that decides how a start commits to
successors:

- `FirstSuccessorPerStart` (default) — commit to the earliest satisfying
  successor at each step; a start yields at most one match.
- `ExhaustivePerStart` — explore every satisfying successor; a start may yield
  several matches.

These coincide for two-step patterns and can differ for three or more. The
[semantics page](semantics.md) explains the difference with an example.

## Next

- [Semantics](semantics.md) — the precise matching rules.
- [Explanations](explanations.md) — why a start did not match.
