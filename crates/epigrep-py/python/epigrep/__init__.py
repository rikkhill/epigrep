"""epigrep: temporal event-pattern matching with a Rust core.

The Rust oracle and compiled matchers remain the semantic source of truth; this
package is a thin, pandas-friendly wrapper plus demo data for the visual harness.
"""

from ._core import (
    Event,
    Match,
    Pattern,
    PatternBuilder,
    parse_pattern,
    sort_events,
)
from ._core import match_events as _match_events
from .frame import events_to_frame, matches_to_frame


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


__all__ = [
    "Event",
    "Match",
    "Pattern",
    "PatternBuilder",
    "parse_pattern",
    "sort_events",
    "match",
    "events_to_frame",
    "matches_to_frame",
]
