"""Loaders for public event-sequence corpora, via the shared eventise primitive.

The first supported corpus is **SPMF** sequence-database format — the de-facto
standard for sequential-pattern-mining datasets. It is the cleanest real-world
shape to start with: integer items, no timestamp or log-template parsing, and a
companion library of reference pattern-mining results to validate against.

SPMF sequence format
--------------------
One sequence per line. Items are integers separated by spaces; ``-1`` ends an
itemset (a set of co-occurring items); ``-2`` ends the sequence. Example::

    1 -1 1 2 3 -1 1 3 -1 4 -1 3 6 -1 -2

Eventisation maps this to the epigrep model as: ``partition`` = sequence id,
``ts`` = itemset index within the sequence (items in one itemset are
simultaneous), ``typ`` = the item. There are no attributes.

Other corpora (LogHub, which needs log-template parsing; BPI/XES, which needs an
XES reader) reuse the same :func:`epigrep.eventise` contract when they land; they
are deliberately not implemented here yet.
"""

from __future__ import annotations

from pathlib import Path
from typing import Any, Dict, List, Union

from ._core import Event
from .eventise import eventise

# A tiny, hand-checkable SPMF excerpt: the canonical four-sequence example from
# the SPMF PrefixSpan documentation. Ground-truth sequential patterns over it are
# stable and used by the test-suite (e.g. "4 then 5" occurs only in sequence 1).
SPMF_SAMPLE = """\
1 -1 1 2 3 -1 1 3 -1 4 -1 3 6 -1 -2
1 4 -1 3 -1 2 3 -1 1 5 -1 -2
5 6 -1 1 2 -1 4 6 -1 3 -1 2 -1 -2
5 -1 7 -1 1 6 -1 3 -1 2 -1 3 -1 -2
"""


def parse_spmf(text: str) -> List[Dict[str, Any]]:
    """Parse SPMF sequence text into flat ``{sequence, position, item}`` records.

    ``sequence`` is the 0-based line index, ``position`` the 0-based itemset
    index within that sequence, and ``item`` the item token (kept as a string,
    since event types are strings). Blank lines are skipped.
    """
    records: List[Dict[str, Any]] = []
    sequence_id = 0
    for line in text.splitlines():
        stripped = line.strip()
        if not stripped:
            continue
        position = 0
        for token in stripped.split():
            if token == "-2":
                break
            if token == "-1":
                position += 1
                continue
            records.append(
                {"sequence": sequence_id, "position": position, "item": token}
            )
        sequence_id += 1
    return records


def load_spmf(text: str, *, partition_prefix: str = "seq-") -> List[Event]:
    """Eventise SPMF sequence *text* into sorted :class:`Event` objects."""
    records = parse_spmf(text)
    return eventise(
        records,
        partition=lambda record: f"{partition_prefix}{record['sequence']}",
        ts="position",
        typ="item",
        attrs=[],
    )


def load_spmf_file(
    path: Union[str, Path], *, partition_prefix: str = "seq-"
) -> List[Event]:
    """Eventise an SPMF sequence-database *file* into sorted :class:`Event`s."""
    return load_spmf(Path(path).read_text(), partition_prefix=partition_prefix)


def sample_spmf_events(*, partition_prefix: str = "seq-") -> List[Event]:
    """The bundled four-sequence SPMF sample, eventised. Handy for demos/tests."""
    return load_spmf(SPMF_SAMPLE, partition_prefix=partition_prefix)
