"""epigrep: temporal event-pattern matching with a Rust core.

The Rust oracle and compiled matchers remain the semantic source of truth; this
package is a thin wrapper plus optional pandas helpers and demo data.
"""

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
from .schema import schema


def events_to_frame(events):
    """Return a pandas DataFrame for events.

    Requires pandas; install the ``frame`` or ``test`` extra, or install pandas
    directly.
    """
    from .frame import events_to_frame as _events_to_frame

    return _events_to_frame(events)


def matches_to_frame(matches):
    """Return a pandas DataFrame for matches."""
    from .frame import matches_to_frame as _matches_to_frame

    return _matches_to_frame(matches)


def near_misses_to_frame(near_misses):
    """Return a pandas DataFrame for near-misses."""
    from .frame import near_misses_to_frame as _near_misses_to_frame

    return _near_misses_to_frame(near_misses)


def match(pattern, events, *, exhaustive=False, oracle=False, assume_sorted=False):
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


def explain(pattern, events, *, assume_sorted=False):
    """Return near-misses: starts that did not match, each with its deepest
    partial path and the reason it could not continue.

    Explanation is existence-based and independent of consumption mode: a start
    is a near-miss only if no full match exists from it.
    """
    events = list(events)
    if not assume_sorted:
        events = sort_events(events)
    return _near_miss_events(pattern, events)


def near_miss_summary(near_miss) -> str:
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
    "parse_pattern",
    "pattern_from_json",
    "sort_events",
    "schema",
    "match",
    "explain",
    "near_miss_summary",
    "events_to_frame",
    "matches_to_frame",
    "near_misses_to_frame",
]
