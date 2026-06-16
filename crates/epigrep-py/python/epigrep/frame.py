"""pandas conversion helpers for events and matches."""

from __future__ import annotations

import pandas as pd


def events_to_frame(events) -> pd.DataFrame:
    """Return a DataFrame with one row per event.

    Columns: ``index`` (position in the given sequence), ``partition``, ``ts``,
    ``typ``, plus one column per attribute key encountered.
    """
    rows = []
    for index, event in enumerate(events):
        row = {
            "index": index,
            "partition": event.partition,
            "ts": event.ts,
            "typ": event.typ,
        }
        row.update(event.attrs)
        rows.append(row)
    return pd.DataFrame(rows)


def matches_to_frame(matches) -> pd.DataFrame:
    """Return a DataFrame with one row per match."""
    rows = [
        {
            "partition": match.partition,
            "start": match.start,
            "end": match.end,
            "indices": list(match.indices),
            "types": list(match.types),
            "captures": dict(match.captures),
        }
        for match in matches
    ]
    return pd.DataFrame(
        rows,
        columns=["partition", "start", "end", "indices", "types", "captures"],
    )


def near_misses_to_frame(near_misses) -> pd.DataFrame:
    """Return a DataFrame with one row per near-miss."""
    rows = [
        {
            "partition": near_miss.partition,
            "start": near_miss.start_index,
            "indices": list(near_miss.indices),
            "reached_steps": near_miss.reached_steps,
            "next_event_type": near_miss.next_event_type,
            "reason": near_miss.reason,
            "captures": dict(near_miss.captures),
        }
        for near_miss in near_misses
    ]
    return pd.DataFrame(
        rows,
        columns=[
            "partition",
            "start",
            "indices",
            "reached_steps",
            "next_event_type",
            "reason",
            "captures",
        ],
    )
