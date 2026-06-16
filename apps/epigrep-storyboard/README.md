# Epigrep storyboard

A Streamlit harness that makes the epigrep matcher's behaviour visible. It is a
development and debugging tool — a working lens on the engine — not the eventual
product UI.

## What it shows

- a **dataset picker** for the demo stories (care pathway, observability trace,
  planted noise, and a dead-end / failed-continuation case);
- a **pattern editor** using the Phase 1 text parser;
- a **partitioned timeline** (one swimlane per partition) with event indices,
  event types, participating events highlighted, and match spans drawn as bars;
- a **match table**, a **near-miss** panel (deepest partial path + why the next
  step failed), and a **captures** panel;
- a **ground-truth comparison** for stories with known answers (true/false
  positives and negatives);
- a **semantics panel** that keeps consumption mode, window inclusivity, and
  absence rules in view, plus a live check that the oracle and compiled backends
  agree on the current view.

## Running

From the repo root, in an environment where the `epigrep` package is installed
(see the top-level README for `maturin develop`):

```sh
.venv/bin/streamlit run apps/epigrep-storyboard/app.py
```

Then open the printed URL (default <http://localhost:8501>).

## Design guardrails

- Keep demo data tiny enough to inspect by hand.
- Prefer a crude but transparent plot over a polished chart that hides the exact
  participating events.
- Do not let Streamlit-specific code leak into the `epigrep` package; the app
  only consumes the public Python API.
