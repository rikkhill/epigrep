"""Malformed input must surface as clean Python exceptions, never a panic.

The Rust core validates a pattern's structure before matching and treats an
invalid pattern reaching the matcher as an internal bug (it would panic). These
tests prove that the *public* Python surface — the constructors, JSON AST
loading, and the text parser — rejects malformed user input with a normal
``ValueError`` / ``TypeError`` first, so no malformed input can cross the FFI
boundary and trip an internal panic.

A pyo3 panic surfaces as ``pyo3_runtime.PanicException``, which does **not**
subclass ``Exception`` and so would not be caught by ``pytest.raises(ValueError)``
— such a test would error loudly rather than pass. So asserting a clean
``ValueError``/``TypeError`` here is itself the panic-safety guarantee.
"""

import pytest

import epigrep
from epigrep import Event, Pattern, match, explain, parse_pattern


def _atom(typ):
    return (
        f'{{"event_type": "{typ}", "predicates": [], '
        f'"reference_predicates": [], "captures": []}}'
    )


# JSON ASTs that are well-formed JSON but not valid patterns, plus plain
# garbage. Each must raise ValueError from pattern_from_json.
MALFORMED_JSON = [
    pytest.param("{ not json at all", id="garbage"),
    pytest.param("", id="empty-string"),
    pytest.param("[]", id="json-array-not-object"),
    pytest.param('{"steps": [], "consumption": "FirstSuccessorPerStart"}', id="no-steps"),
    pytest.param(
        '{"steps": [{"atom": ' + _atom("A") + ', "transition_from_previous": '
        '{"max_elapsed": null, "absence": []}}], '
        '"consumption": "FirstSuccessorPerStart"}',
        id="first-step-has-transition",
    ),
    pytest.param(
        '{"steps": ['
        '{"atom": ' + _atom("A") + ', "transition_from_previous": null}, '
        '{"atom": ' + _atom("B") + ', "transition_from_previous": null}], '
        '"consumption": "FirstSuccessorPerStart"}',
        id="later-step-missing-transition",
    ),
    pytest.param('{"consumption": "FirstSuccessorPerStart"}', id="missing-steps-field"),
    pytest.param(
        '{"steps": "not-a-list", "consumption": "FirstSuccessorPerStart"}',
        id="steps-wrong-type",
    ),
    pytest.param(
        '{"steps": [{"atom": ' + _atom("A") + ', "transition_from_previous": null}], '
        '"consumption": "NoSuchMode"}',
        id="unknown-consumption-variant",
    ),
]


@pytest.mark.parametrize("payload", MALFORMED_JSON)
def test_pattern_from_json_rejects_malformed_input(payload):
    with pytest.raises(ValueError):
        epigrep.pattern_from_json(payload)


@pytest.mark.parametrize(
    "text",
    ["", "   ", "->", "A ->", "A -> -> B", "-> B"],
)
def test_parse_pattern_rejects_bad_syntax(text):
    with pytest.raises(ValueError):
        parse_pattern(text)


# Adversarial untrusted input. The contract is "never a panic": each call must
# either return or raise a normal Python exception. A pyo3 panic surfaces as
# pyo3_runtime.PanicException, which subclasses BaseException (not Exception), so
# it escapes ``except Exception`` and fails the test loudly — which is the point.
ADVERSARIAL = [
    pytest.param("\x00\x00\x00", id="null-bytes"),
    pytest.param("A" * 10000, id="very-long-type"),
    pytest.param("[" * 2000, id="deep-open-brackets"),
    pytest.param("A -[<=" + "9" * 400 + "]-> B", id="huge-number"),
    pytest.param("A -[<=-5]-> B", id="negative-window"),
    pytest.param("Â -> 🦀 -[no ✨]-> ☃", id="unicode-soup"),
    pytest.param("A[score >= ]-> B", id="dangling-operator"),
    pytest.param("A $ $ $ -> B", id="stray-references"),
    pytest.param("\t\n\r\f\v", id="whitespace-controls"),
    pytest.param('{"steps": [' * 500 + "]" * 500, id="deeply-nested-json"),
    pytest.param("A -[<=1.5e400]-> B", id="overflow-float-window"),
]


def _must_not_panic(call, argument):
    """Call must return or raise a normal Exception, never let a panic through."""
    try:
        call(argument)
    except Exception:
        pass  # a clean Python exception is an acceptable outcome


@pytest.mark.parametrize("text", ADVERSARIAL)
def test_parse_pattern_never_panics_on_adversarial_input(text):
    _must_not_panic(parse_pattern, text)


@pytest.mark.parametrize("text", ADVERSARIAL)
def test_pattern_from_json_never_panics_on_adversarial_input(text):
    _must_not_panic(epigrep.pattern_from_json, text)


def test_event_rejects_unsupported_attribute_value_types():
    # Attribute values must be str/int/float/bool/None; a list is not coercible.
    with pytest.raises(TypeError):
        Event("p", 0, "A", {"bad": [1, 2, 3]})
    with pytest.raises(TypeError):
        Event("p", 0, "A", {"bad": {"nested": 1}})


def test_event_rejects_wrong_typed_positional_arguments():
    with pytest.raises(TypeError):
        Event("p", "not-an-int", "A")


def test_reference_to_unbound_binding_does_not_panic():
    # A reference predicate against a binding that was never captured must just
    # fail to match (no match, a clean near-miss) rather than panicking.
    ast = (
        '{"steps": ['
        '{"atom": {"event_type": "A", "predicates": [], '
        '"reference_predicates": [{"attribute": "id", "operator": "Eq", '
        '"binding": "never_captured"}], "captures": []}, '
        '"transition_from_previous": null}], '
        '"consumption": "FirstSuccessorPerStart"}'
    )
    pattern = epigrep.pattern_from_json(ast)
    events = [Event("p", 0, "A", {"id": 1})]
    assert match(pattern, events) == []
    # Explaining the same start must also stay panic-free.
    assert isinstance(explain(pattern, events), list)


def test_matching_empty_event_stream_is_safe():
    pattern = Pattern.event("A").then("B").build()
    assert match(pattern, []) == []
    assert explain(pattern, []) == []


def test_builder_pattern_is_always_valid_and_matches():
    # The stable builder surface cannot construct a structurally invalid pattern.
    pattern = Pattern.event("A").then("B", within=5).build()
    events = [Event("p", 0, "A"), Event("p", 2, "B")]
    assert [list(m.indices) for m in match(pattern, events)] == [[0, 1]]
