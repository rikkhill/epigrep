"""Schema discovery over an event stream.

Part of the minimal agent surface (schema / run / explain): summarise the shape
of a stream so a caller — human or LLM — knows what event types and attributes
exist before constructing a pattern.
"""

from __future__ import annotations

# Exact-type lookup (not isinstance) so Python's bool-is-an-int does not
# misclassify booleans as integers.
_TYPE_NAMES = {
    bool: "bool",
    int: "int",
    float: "float",
    str: "string",
    type(None): "null",
}


def _type_name(value) -> str:
    return _TYPE_NAMES.get(type(value), type(value).__name__)


def schema(events) -> dict:
    """Summarise ``events`` for pattern construction.

    Returns a dict with the event count, the partitions (in first-seen order),
    the timestamp range, and per event type a count and the attribute keys with
    the set of value types observed for each.
    """
    events = list(events)
    partitions: list = []
    seen_partitions: set = set()
    event_types: dict = {}
    min_ts = None
    max_ts = None

    for event in events:
        if event.partition not in seen_partitions:
            seen_partitions.add(event.partition)
            partitions.append(event.partition)
        min_ts = event.ts if min_ts is None else min(min_ts, event.ts)
        max_ts = event.ts if max_ts is None else max(max_ts, event.ts)

        entry = event_types.setdefault(event.typ, {"count": 0, "attributes": {}})
        entry["count"] += 1
        for key, value in event.attrs.items():
            entry["attributes"].setdefault(key, set()).add(_type_name(value))

    for entry in event_types.values():
        entry["attributes"] = {
            key: sorted(types) for key, types in entry["attributes"].items()
        }

    return {
        "event_count": len(events),
        "partitions": partitions,
        "partition_count": len(partitions),
        "time_range": [min_ts, max_ts] if events else None,
        "event_types": event_types,
    }
