"""Epigrep storyboard: a Streamlit lens on the matcher.

This is a development/debugging tool, not the product UI. It makes the matcher's
semantics visible: timelines, match spans, participating events, captures, and
ground-truth comparison for planted examples.

Run with:

    streamlit run apps/epigrep-storyboard/app.py

(from a Python environment where the `epigrep` package is installed, e.g. the
project's .venv after `maturin develop`).
"""

from __future__ import annotations

import altair as alt
import pandas as pd
import streamlit as st

import epigrep
from epigrep import data, match, parse_pattern


SEMANTIC_NOTES = """
**Active Phase 1 semantics**

- **Match consumption** — *first-successor* commits to the earliest successor
  satisfying each step and its transition (at most one match per start);
  *exhaustive* explores every satisfying successor.
- **Window** — `within Δ` is inclusive: `0 <= ts(next) - ts(prev) <= Δ`.
- **Absence-between** — `no X` forbids a matching X strictly between two
  participating events, ordered by `(timestamp, input index)`.
- **Tie-breaking** — equal timestamps are ordered by input position.
- **Overlapping matches** — every start position is reported.
"""


def events_dataframe(story, matches) -> pd.DataFrame:
    """One row per event, flagged with whether it participates in any match."""
    participating = {index for m in matches for index in m.indices}
    frame = epigrep.events_to_frame(story.events)
    frame["role"] = [
        "participating" if index in participating else "other"
        for index in frame["index"]
    ]
    return frame


def spans_dataframe(matches) -> pd.DataFrame:
    """One row per match span, used to draw rules across the timeline."""
    rows = [
        {
            "partition": m.partition,
            "start": m.start,
            "end": m.end,
            "label": "-".join(str(i) for i in m.indices),
        }
        for m in matches
    ]
    return pd.DataFrame(rows, columns=["partition", "start", "end", "label"])


def build_chart(story, matches) -> alt.LayerChart:
    """Build the partitioned timeline with match spans and event markers."""
    events = events_dataframe(story, matches)
    spans = spans_dataframe(matches)

    base = alt.Chart(events)
    points = base.mark_point(size=160, filled=True).encode(
        x=alt.X("ts:Q", title="time"),
        y=alt.Y("partition:N", title="partition"),
        color=alt.Color("typ:N", title="event type"),
        shape=alt.Shape(
            "role:N",
            title="role",
            scale=alt.Scale(
                domain=["participating", "other"],
                range=["diamond", "circle"],
            ),
        ),
        opacity=alt.Opacity(
            "role:N",
            scale=alt.Scale(domain=["participating", "other"], range=[1.0, 0.45]),
            legend=None,
        ),
        tooltip=["index:Q", "partition:N", "ts:Q", "typ:N"],
    )
    labels = base.mark_text(dy=-14, fontSize=10).encode(
        x="ts:Q",
        y="partition:N",
        text="index:Q",
    )

    layers = [points, labels]
    if not spans.empty:
        span_marks = (
            alt.Chart(spans)
            .mark_rule(strokeWidth=6, opacity=0.3, color="#444")
            .encode(
                x="start:Q",
                x2="end:Q",
                y="partition:N",
                tooltip=["partition:N", "start:Q", "end:Q", "label:N"],
            )
        )
        layers.insert(0, span_marks)

    return alt.layer(*layers).properties(height=120 + 40 * events["partition"].nunique())


def ground_truth_table(story, matches, exhaustive: bool) -> pd.DataFrame:
    """Expected-vs-found classification for stories with known ground truth."""
    found = {tuple(m.indices) for m in matches}
    expected = {tuple(span) for span in (story.expected or [])}
    rows = []
    for span in sorted(expected | found):
        if span in expected and span in found:
            verdict = "true positive"
        elif span in found:
            verdict = "false positive" if not exhaustive else "extra (exhaustive)"
        else:
            verdict = "false negative"
        rows.append({"span": "-".join(map(str, span)), "verdict": verdict})
    return pd.DataFrame(rows, columns=["span", "verdict"])


def main() -> None:
    st.set_page_config(page_title="Epigrep storyboard", layout="wide")
    st.title("Epigrep storyboard")
    st.caption("A working lens on the matcher engine — semantics made visible.")

    stories = {story.title: story for story in data.all_stories()}

    with st.sidebar:
        st.header("Inputs")
        title = st.selectbox("Dataset", list(stories))
        story = stories[title]
        st.write(story.description)
        pattern_text = st.text_area("Pattern", value=story.pattern_text, height=80)
        mode = st.radio("Consumption", ["first-successor", "exhaustive"], index=0)
        backend = st.radio("Backend", ["compiled", "oracle"], index=0)

    exhaustive = mode == "exhaustive"
    use_oracle = backend == "oracle"

    try:
        pattern = parse_pattern(pattern_text)
    except ValueError as error:
        st.error(f"Could not parse pattern: {error}")
        st.stop()

    matches = match(pattern, story.events, exhaustive=exhaustive, oracle=use_oracle)

    left, right = st.columns([3, 2])

    with left:
        st.subheader("Timeline")
        st.altair_chart(build_chart(story, matches), use_container_width=True)
        st.caption(
            "Diamonds are participating events; numbers are event indices; grey "
            "bars are match spans."
        )

        st.subheader(f"Matches ({len(matches)})")
        if matches:
            st.dataframe(
                epigrep.matches_to_frame(matches), use_container_width=True
            )
        else:
            st.info("No matches for this pattern, dataset, and mode.")

    with right:
        st.subheader("Captures")
        captures = [m.captures for m in matches if m.captures]
        if captures:
            st.json(captures)
        else:
            st.caption("No captured bindings in these matches.")

        if story.expected is not None:
            st.subheader("Ground truth")
            st.caption("Expected spans are defined for first-successor mode.")
            st.dataframe(
                ground_truth_table(story, matches, exhaustive),
                use_container_width=True,
            )

        st.subheader("Semantics")
        st.markdown(SEMANTIC_NOTES)
        if story.notes:
            st.info(story.notes)

        # A live cross-check that the two backends agree on the current view.
        other = match(pattern, story.events, exhaustive=exhaustive, oracle=not use_oracle)
        agree = [m.indices for m in matches] == [m.indices for m in other]
        st.caption(
            ("✅ oracle and compiled agree" if agree else "❌ backends disagree!")
            + " on this pattern/dataset/mode."
        )


if __name__ == "__main__":
    main()
