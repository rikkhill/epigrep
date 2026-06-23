"""epigrep: temporal event-pattern matching with a Rust core.

The Rust oracle and compiled matchers remain the semantic source of truth; this
package is a thin wrapper plus optional pandas helpers and demo data.

Public API stability
--------------------
The supported, stable construction surfaces are the :class:`PatternBuilder`
(reached via ``Pattern.event(...)``) and the JSON pattern AST
(:func:`pattern_from_json` / :meth:`Pattern.to_json`). :func:`match`,
:func:`explain`, and :func:`schema` are the stable execution/inspection
entrypoints.

:func:`eventise` / :func:`events_from_frame` ingest record/dataframe data into
the sorted :class:`Event` list the matcher expects; they are the shared
ingestion primitive behind the dataframe helpers, the ``epigrep`` CLI, and the
dataset loaders in :mod:`epigrep.datasets`.

:func:`parse_pattern` (the text DSL) is **provisional**: importable and
documented, but outside the 0.1 stability guarantee and may change. The pandas
helpers (``*_to_frame``) require pandas and are convenience-tier.

Everything exported here is listed in :data:`__all__`; names not listed (e.g.
the underscored low-level matchers) are internal.
"""

from __future__ import annotations

from typing import TYPE_CHECKING, Any, Iterable, List, Union

from ._core import (
    Event,
    Match,
    NearMiss,
    Pattern,
    PatternBuilder,
    parse_pattern,
    pattern_from_json,
    sort_events,
)
from ._core import match_events as _match_events
from ._core import near_miss_events as _near_miss_events
from .eventise import eventise, events_from_frame
from .schema import schema

if TYPE_CHECKING:  # pragma: no cover - typing only
    import pandas as pd

# A pattern accepted by the execution entrypoints.
PatternLike = Union[Pattern, PatternBuilder]


def events_to_frame(events: Iterable[Event]) -> "pd.DataFrame":
    """Return a pandas DataFrame for events.

    Requires pandas; install the ``frame`` or ``test`` extra, or install pandas
    directly.
    """
    from .frame import events_to_frame as _events_to_frame

    return _events_to_frame(events)


def matches_to_frame(matches: Iterable[Match]) -> "pd.DataFrame":
    """Return a pandas DataFrame for matches."""
    from .frame import matches_to_frame as _matches_to_frame

    return _matches_to_frame(matches)


def near_misses_to_frame(near_misses: Iterable[NearMiss]) -> "pd.DataFrame":
    """Return a pandas DataFrame for near-misses."""
    from .frame import near_misses_to_frame as _near_misses_to_frame

    return _near_misses_to_frame(near_misses)


def match(
    pattern: PatternLike,
    events: Iterable[Event],
    *,
    exhaustive: bool = False,
    oracle: bool = False,
    assume_sorted: bool = False,
) -> List[Match]:
    """Run ``pattern`` over ``events`` and return a list of :class:`Match`.

    Parameters
    ----------
    pattern:
        A :class:`Pattern`, a :class:`PatternBuilder`, or the result of
        :func:`parse_pattern`.
    events:
        An iterable of :class:`Event`.
    exhaustive:
        If ``True``, emit every satisfying successor per start instead of
        committing to the first one (``MatchConsumption::ExhaustivePerStart``).
    oracle:
        If ``True``, run the naive reference matcher instead of the compiled
        backend. They are expected to agree; this is for parity demos.
    assume_sorted:
        Skip the partition/time sort. Only set this if the input is already
        grouped by partition and sorted by ``(timestamp, input order)``.
    """
    events = list(events)
    if not assume_sorted:
        events = sort_events(events)
    return _match_events(pattern, events, exhaustive, oracle)


def explain(
    pattern: PatternLike,
    events: Iterable[Event],
    *,
    assume_sorted: bool = False,
) -> List[NearMiss]:
    """Return near-misses: starts that did not match, each with its deepest
    partial path and the reason it could not continue.

    Explanation is existence-based and independent of consumption mode: a start
    is a near-miss only if no full match exists from it.
    """
    events = list(events)
    if not assume_sorted:
        events = sort_events(events)
    return _near_miss_events(pattern, events)


def near_miss_summary(near_miss: NearMiss) -> str:
    """A short human-readable explanation of a near-miss, from its detail."""
    detail = near_miss.detail
    kind = detail["kind"]
    reached = "->".join(str(i) for i in near_miss.indices)
    head = f"reached [{reached}]"
    nxt = near_miss.next_event_type

    if kind == "predicate_failed":
        parts = []
        for failure in detail["failures"]:
            if failure["type"] == "predicate":
                parts.append(
                    f"would match if {failure['attribute']} {failure['operator']} "
                    f"{failure['expected']!r} (was {failure['actual']!r})"
                )
            elif failure["type"] == "reference":
                parts.append(
                    f"would match if {failure['attribute']} {failure['operator']} "
                    f"${failure['binding']} (bound {failure['bound']!r}, "
                    f"was {failure['actual']!r})"
                )
            else:  # capture conflict
                parts.append(
                    f"{failure['attribute']} as ${failure['name']} conflicts "
                    f"(bound {failure['bound']!r}, was {failure['actual']!r})"
                )
        clauses = "; ".join(parts) if parts else "predicate failed"
        return f"{head}; {nxt} at {detail['event_index']}: {clauses}"

    if kind == "absence_blocked":
        blocker = f"{detail['blocking_event_type']} at {detail['blocking_index']}"
        if detail["candidate_satisfies"]:
            return (
                f"{head}; would match if {blocker} were absent (before "
                f"{nxt} at {detail['candidate_index']})"
            )
        return (
            f"{head}; {nxt} at {detail['candidate_index']} blocked by {blocker}, "
            f"and also fails its own predicate"
        )

    if kind == "window_exceeded":
        return (
            f"{head}; nearest {nxt} at {detail['candidate_index']} is "
            f"{detail['gap']} away; would match if window >= {detail['gap']} "
            f"(currently {detail['max_elapsed']})"
        )

    return f"{head}; no {nxt} after frontier"


__all__ = [
    "Event",
    "Match",
    "NearMiss",
    "Pattern",
    "PatternBuilder",
    "PatternLike",
    "parse_pattern",
    "pattern_from_json",
    "sort_events",
    "schema",
    "match",
    "explain",
    "near_miss_summary",
    "eventise",
    "events_from_frame",
    "events_to_frame",
    "matches_to_frame",
    "near_misses_to_frame",
]
