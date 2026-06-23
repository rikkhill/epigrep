"""Eventisation: turn tabular / record data into sorted :class:`Event` lists.

This is the shared ingestion primitive behind the dataframe helpers, the CLI,
and the dataset loaders. The model is always the same: every input record maps
to an ``Event(partition, ts, typ, attrs)``, and the result is sorted by
``(partition, ts, input order)`` exactly as the matcher expects.

Two entry points:

``eventise(records, ...)``
    The general form. ``partition`` / ``ts`` / ``typ`` are each either a key
    into a mapping record or a callable ``record -> value``. ``attrs`` is
    ``None`` (all remaining mapping keys), an explicit list of keys, or a
    callable ``record -> dict``.

``events_from_frame(frame, ...)``
    ``eventise`` specialised to a pandas / polars DataFrame or a pyarrow Table,
    using column names. It is the inverse of :func:`epigrep.events_to_frame`.

Index-alignment note
--------------------
:func:`epigrep.match` sorts its input and returns indices into the *sorted*
stream. If you need to map matches back onto the original frame rows, sort the
frame by ``(partition, ts)`` first and call ``match(..., assume_sorted=True)``;
then ``Match.indices`` line up with frame row positions and ``df.iloc[indices]``
recovers the participating rows.
"""

from __future__ import annotations

from typing import Any, Callable, Iterable, List, Optional, Sequence, Union

from ._core import Event, sort_events

# A field accessor: a key into a mapping record, or a callable record -> value.
Accessor = Union[str, Callable[[Any], Any]]
# An attribute selector: keys to keep, a callable record -> dict, or None (rest).
AttrSelector = Union[None, Sequence[str], Callable[[Any], dict]]


def _make_accessor(spec: Accessor) -> Callable[[Any], Any]:
    if callable(spec):
        return spec
    return lambda record: record[spec]


def _py_scalar(value: Any) -> Any:
    """Coerce a numpy/pandas scalar to a native Python scalar; NaN -> ``None``.

    Leaves native ``str``/``bytes``/``int``/``float``/``bool``/``None`` alone.
    numpy and pandas scalars expose ``.item()``; floating NaN becomes ``None`` so
    missing cells are treated as absent attributes rather than NaN values the
    core cannot store.
    """
    if isinstance(value, (str, bytes)):
        return value
    item = getattr(value, "item", None)
    if item is not None:
        try:
            value = item()
        except Exception:  # pragma: no cover - defensive
            pass
    if isinstance(value, float) and value != value:  # NaN
        return None
    return value


def _coerce_ts(value: Any) -> int:
    scalar = _py_scalar(value)
    if isinstance(scalar, bool):
        raise TypeError(f"timestamp must be an integer, not bool: {value!r}")
    if isinstance(scalar, int):
        return scalar
    if scalar is None:
        raise TypeError("timestamp is missing (null/NaN)")
    return int(scalar)


def eventise(
    records: Iterable[Any],
    *,
    partition: Accessor,
    ts: Accessor,
    typ: Accessor,
    attrs: AttrSelector = None,
    sort: bool = True,
) -> List[Event]:
    """Map ``records`` to :class:`Event` objects, sorted unless ``sort=False``.

    ``partition`` / ``ts`` / ``typ`` are key names (for mapping records) or
    callables. ``ts`` is coerced to ``int``; ``partition`` and ``typ`` are
    coerced to ``str``. Attribute values are coerced to native scalars and any
    ``None``/NaN-valued attribute is dropped (treated as absent).

    ``attrs=None`` keeps every mapping key except the ones named by ``partition``
    / ``ts`` / ``typ`` (only meaningful when those are given as string keys and
    the records are mappings). Pass an explicit key list or a callable when the
    accessors are callables or you want a subset.
    """
    get_partition = _make_accessor(partition)
    get_ts = _make_accessor(ts)
    get_typ = _make_accessor(typ)

    if attrs is None:
        reserved = {spec for spec in (partition, ts, typ) if isinstance(spec, str)}

        def get_attrs(record: Any) -> dict:
            return {k: v for k, v in record.items() if k not in reserved}

    elif callable(attrs):
        get_attrs = attrs  # type: ignore[assignment]
    else:
        attr_keys = list(attrs)

        def get_attrs(record: Any) -> dict:
            return {k: record[k] for k in attr_keys}

    events: List[Event] = []
    for record in records:
        clean_attrs = {}
        for key, raw in get_attrs(record).items():
            scalar = _py_scalar(raw)
            if scalar is not None:
                clean_attrs[str(key)] = scalar
        events.append(
            Event(
                str(_py_scalar(get_partition(record))),
                _coerce_ts(get_ts(record)),
                str(_py_scalar(get_typ(record))),
                clean_attrs,
            )
        )

    return sort_events(events) if sort else events


def _frame_records(frame: Any) -> List[dict]:
    """Extract a list of row-dicts from a pandas/polars frame or pyarrow table."""
    # polars DataFrame
    iter_rows = getattr(frame, "iter_rows", None)
    if callable(iter_rows):
        return list(iter_rows(named=True))
    # pyarrow Table / RecordBatch
    to_pylist = getattr(frame, "to_pylist", None)
    if callable(to_pylist):
        return to_pylist()
    # pandas DataFrame
    to_dict = getattr(frame, "to_dict", None)
    if callable(to_dict):
        return to_dict(orient="records")
    raise TypeError(
        "events_from_frame expects a pandas/polars DataFrame or a pyarrow Table; "
        f"got {type(frame).__name__}"
    )


def events_from_frame(
    frame: Any,
    *,
    partition_col: str,
    ts_col: str,
    type_col: str,
    attr_cols: Optional[Sequence[str]] = None,
    sort: bool = True,
) -> List[Event]:
    """Build sorted :class:`Event` objects from a dataframe / table.

    The inverse of :func:`epigrep.events_to_frame`. ``attr_cols=None`` uses every
    column except the three mapped ones. Works for pandas and polars DataFrames
    and pyarrow Tables (detected by duck-typing; no library is imported here, so
    none is a hard dependency).

    See the module docstring for how to keep ``Match.indices`` aligned with frame
    rows (pre-sort + ``assume_sorted=True``).
    """
    records = _frame_records(frame)
    return eventise(
        records,
        partition=partition_col,
        ts=ts_col,
        typ=type_col,
        attrs=attr_cols,
        sort=sort,
    )
