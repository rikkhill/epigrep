//! Python bindings for the epigrep temporal event-pattern matcher.
//!
//! This crate is a thin wrapper around `epigrep-core`. The Rust oracle and
//! compiled matchers remain the semantic source of truth; this layer only
//! marshals events, patterns, and matches across the Python boundary.

// pyo3 0.22's #[pyfunction]/#[pymethods] expansions emit identity `.into()`
// calls on PyErr that trip clippy::useless_conversion. The lint points at
// macro-generated code and is not actionable from this crate's source.
#![allow(clippy::useless_conversion)]

use epigrep_core as core;
use pyo3::exceptions::{PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyBool, PyDict, PyList};

/// Convert a Python attribute value into a core `Value`.
///
/// `bool` is checked before `int` because Python booleans are a subclass of
/// `int` and would otherwise be silently coerced to integers.
fn py_to_value(obj: &Bound<'_, PyAny>) -> PyResult<core::Value> {
    if obj.is_none() {
        return Ok(core::Value::Null);
    }
    if let Ok(value) = obj.downcast::<PyBool>() {
        return Ok(core::Value::Bool(value.is_true()));
    }
    if let Ok(value) = obj.extract::<i64>() {
        return Ok(core::Value::Int(value));
    }
    if let Ok(value) = obj.extract::<f64>() {
        return Ok(core::Value::Float(value));
    }
    if let Ok(value) = obj.extract::<String>() {
        return Ok(core::Value::String(value));
    }
    Err(PyTypeError::new_err(
        "event attribute values must be str, int, float, bool, or None",
    ))
}

/// Convert a core `Value` back into a Python object.
fn value_to_py(py: Python<'_>, value: &core::Value) -> PyObject {
    match value {
        core::Value::String(value) => value.into_py(py),
        core::Value::Int(value) => value.into_py(py),
        core::Value::Float(value) => value.into_py(py),
        core::Value::Bool(value) => value.into_py(py),
        core::Value::Null => py.None(),
    }
}

/// A single typed, timestamped event within a partition.
#[pyclass(name = "Event")]
#[derive(Clone)]
struct PyEvent {
    inner: core::Event,
}

#[pymethods]
impl PyEvent {
    #[new]
    #[pyo3(signature = (partition, ts, typ, attrs = None))]
    fn new(
        partition: String,
        ts: i64,
        typ: String,
        attrs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Self> {
        let mut event = core::Event::new(partition, ts, typ);
        if let Some(attrs) = attrs {
            for (key, value) in attrs.iter() {
                let key: String = key.extract()?;
                event = event.with_attr(key, py_to_value(&value)?);
            }
        }
        Ok(Self { inner: event })
    }

    #[getter]
    fn partition(&self) -> &str {
        &self.inner.partition
    }

    #[getter]
    fn ts(&self) -> i64 {
        self.inner.timestamp
    }

    #[getter]
    fn typ(&self) -> &str {
        &self.inner.event_type
    }

    #[getter]
    fn attrs(&self, py: Python<'_>) -> PyResult<Py<PyDict>> {
        let dict = PyDict::new_bound(py);
        for (key, value) in &self.inner.attributes {
            dict.set_item(key, value_to_py(py, value))?;
        }
        Ok(dict.unbind())
    }

    fn __repr__(&self) -> String {
        format!(
            "Event(partition={:?}, ts={}, typ={:?})",
            self.inner.partition, self.inner.timestamp, self.inner.event_type
        )
    }
}

/// A compiled or parsed pattern, ready to match.
#[pyclass(name = "Pattern")]
#[derive(Clone)]
struct PyPattern {
    inner: core::Pattern,
}

#[pymethods]
impl PyPattern {
    /// Start a builder for a pattern whose first step matches `typ`.
    #[staticmethod]
    fn event(typ: String) -> PatternBuilder {
        PatternBuilder {
            steps: vec![core::Step::first(core::Atom::event_type(typ))],
        }
    }

    /// Serialise the pattern to the stable JSON AST.
    #[pyo3(signature = (pretty = false))]
    fn to_json(&self, pretty: bool) -> String {
        if pretty {
            core::pattern_to_json_pretty(&self.inner)
        } else {
            core::pattern_to_json(&self.inner)
        }
    }

    fn __repr__(&self) -> String {
        format!("Pattern(steps={})", self.inner.steps.len())
    }
}

/// A small fluent builder for the stable human-facing construction path.
///
/// The text DSL remains experimental; this builder covers the common
/// event-type, predicate, capture/reference, window, and absence subset used by
/// the examples.
#[pyclass]
#[derive(Clone)]
struct PatternBuilder {
    steps: Vec<core::Step>,
}

impl PatternBuilder {
    fn with_last_atom(&self, update: impl FnOnce(core::Atom) -> core::Atom) -> Self {
        let mut steps = self.steps.clone();
        let last = steps
            .last_mut()
            .expect("PatternBuilder always contains at least one step");
        last.atom = update(last.atom.clone());
        Self { steps }
    }
}

fn normalize_binding_name(name: &str) -> String {
    name.trim()
        .strip_prefix('$')
        .unwrap_or(name.trim())
        .to_owned()
}

#[pymethods]
impl PatternBuilder {
    /// Add `attribute == value` to the current step.
    fn where_eq(&self, attribute: String, value: &Bound<'_, PyAny>) -> PyResult<PatternBuilder> {
        let value = py_to_value(value)?;
        Ok(self.with_last_atom(|atom| {
            atom.with_predicate(core::Predicate::new(
                attribute,
                core::ComparisonOperator::Eq,
                value,
            ))
        }))
    }

    /// Add `attribute != value` to the current step.
    fn where_ne(&self, attribute: String, value: &Bound<'_, PyAny>) -> PyResult<PatternBuilder> {
        let value = py_to_value(value)?;
        Ok(self.with_last_atom(|atom| {
            atom.with_predicate(core::Predicate::new(
                attribute,
                core::ComparisonOperator::NotEq,
                value,
            ))
        }))
    }

    /// Add `attribute > value` to the current step.
    fn where_gt(&self, attribute: String, value: &Bound<'_, PyAny>) -> PyResult<PatternBuilder> {
        let value = py_to_value(value)?;
        Ok(self.with_last_atom(|atom| {
            atom.with_predicate(core::Predicate::new(
                attribute,
                core::ComparisonOperator::Gt,
                value,
            ))
        }))
    }

    /// Add `attribute >= value` to the current step.
    fn where_gte(&self, attribute: String, value: &Bound<'_, PyAny>) -> PyResult<PatternBuilder> {
        let value = py_to_value(value)?;
        Ok(self.with_last_atom(|atom| {
            atom.with_predicate(core::Predicate::new(
                attribute,
                core::ComparisonOperator::Gte,
                value,
            ))
        }))
    }

    /// Add `attribute < value` to the current step.
    fn where_lt(&self, attribute: String, value: &Bound<'_, PyAny>) -> PyResult<PatternBuilder> {
        let value = py_to_value(value)?;
        Ok(self.with_last_atom(|atom| {
            atom.with_predicate(core::Predicate::new(
                attribute,
                core::ComparisonOperator::Lt,
                value,
            ))
        }))
    }

    /// Add `attribute <= value` to the current step.
    fn where_lte(&self, attribute: String, value: &Bound<'_, PyAny>) -> PyResult<PatternBuilder> {
        let value = py_to_value(value)?;
        Ok(self.with_last_atom(|atom| {
            atom.with_predicate(core::Predicate::new(
                attribute,
                core::ComparisonOperator::Lte,
                value,
            ))
        }))
    }

    /// Capture `attribute` from the current step as `$name`.
    fn capture(&self, attribute: String, name: String) -> PatternBuilder {
        let name = normalize_binding_name(&name);
        self.with_last_atom(|atom| atom.with_capture(core::Capture::new(name, attribute)))
    }

    /// Add `attribute == $name` to the current step.
    fn where_ref_eq(&self, attribute: String, name: String) -> PatternBuilder {
        let name = normalize_binding_name(&name);
        self.with_last_atom(|atom| {
            atom.with_reference_predicate(core::ReferencePredicate::new(
                attribute,
                core::ComparisonOperator::Eq,
                name,
            ))
        })
    }

    /// Append a step matching `typ`, optionally within `within` time units of
    /// the previous step and forbidding an intervening event of type `no`.
    #[pyo3(signature = (typ, within = None, no = None))]
    fn then(&self, typ: String, within: Option<i64>, no: Option<String>) -> PatternBuilder {
        let mut transition = core::Transition::any();
        if let Some(within) = within {
            transition = transition.within(within);
        }
        if let Some(absent) = no {
            transition = transition.with_absence(core::Atom::event_type(absent));
        }
        let mut steps = self.steps.clone();
        steps.push(core::Step::then(core::Atom::event_type(typ), transition));
        PatternBuilder { steps }
    }

    /// Finalise the builder into a [`PyPattern`].
    fn build(&self) -> PyPattern {
        PyPattern {
            inner: core::Pattern::sequence(self.steps.clone()),
        }
    }

    fn __repr__(&self) -> String {
        format!("PatternBuilder(steps={})", self.steps.len())
    }
}

/// A single match, carrying its participating events and captured bindings.
#[pyclass(name = "Match")]
#[derive(Clone)]
struct PyMatch {
    partition: String,
    indices: Vec<usize>,
    start: i64,
    end: i64,
    types: Vec<String>,
    bindings: core::Bindings,
    events: Vec<PyEvent>,
}

impl PyMatch {
    fn from_core(value: &core::Match, events: &[core::Event]) -> Self {
        let types = value
            .participating_indices
            .iter()
            .map(|&index| events[index].event_type.clone())
            .collect();
        let participating = value
            .participating_indices
            .iter()
            .map(|&index| PyEvent {
                inner: events[index].clone(),
            })
            .collect();
        Self {
            partition: value.partition.clone(),
            indices: value.participating_indices.clone(),
            start: value.start_timestamp,
            end: value.end_timestamp,
            types,
            bindings: value.bindings.clone(),
            events: participating,
        }
    }
}

#[pymethods]
impl PyMatch {
    #[getter]
    fn partition(&self) -> &str {
        &self.partition
    }

    #[getter]
    fn indices(&self) -> Vec<usize> {
        self.indices.clone()
    }

    #[getter]
    fn start(&self) -> i64 {
        self.start
    }

    #[getter]
    fn end(&self) -> i64 {
        self.end
    }

    #[getter]
    fn types(&self) -> Vec<String> {
        self.types.clone()
    }

    #[getter]
    fn events(&self) -> Vec<PyEvent> {
        self.events.clone()
    }

    /// Captured bindings (registers) as a dict of name to value.
    #[getter]
    fn captures(&self, py: Python<'_>) -> PyResult<Py<PyDict>> {
        let captures = PyDict::new_bound(py);
        for (key, value) in &self.bindings {
            captures.set_item(key, value_to_py(py, value))?;
        }
        Ok(captures.unbind())
    }

    fn __repr__(&self) -> String {
        format!(
            "Match(partition={:?}, indices={:?}, span=({}, {}))",
            self.partition, self.indices, self.start, self.end
        )
    }
}

fn reason_str(reason: core::NearMissReason) -> &'static str {
    match reason {
        core::NearMissReason::PredicateFailed => "predicate_failed",
        core::NearMissReason::AbsenceBlocked => "absence_blocked",
        core::NearMissReason::WindowExceeded => "window_exceeded",
        core::NearMissReason::NoSuccessor => "no_successor",
    }
}

fn optional_value_to_py(py: Python<'_>, value: &Option<core::Value>) -> PyObject {
    match value {
        Some(value) => value_to_py(py, value),
        None => py.None(),
    }
}

fn failure_to_py(py: Python<'_>, failure: &core::PredicateFailure) -> PyResult<Py<PyDict>> {
    let dict = PyDict::new_bound(py);
    match failure {
        core::PredicateFailure::Predicate {
            attribute,
            operator,
            expected,
            actual,
        } => {
            dict.set_item("type", "predicate")?;
            dict.set_item("attribute", attribute)?;
            dict.set_item("operator", operator.symbol())?;
            dict.set_item("expected", value_to_py(py, expected))?;
            dict.set_item("actual", optional_value_to_py(py, actual))?;
        }
        core::PredicateFailure::Reference {
            attribute,
            operator,
            binding,
            bound,
            actual,
        } => {
            dict.set_item("type", "reference")?;
            dict.set_item("attribute", attribute)?;
            dict.set_item("operator", operator.symbol())?;
            dict.set_item("binding", binding)?;
            dict.set_item("bound", optional_value_to_py(py, bound))?;
            dict.set_item("actual", optional_value_to_py(py, actual))?;
        }
        core::PredicateFailure::Capture {
            name,
            attribute,
            bound,
            actual,
        } => {
            dict.set_item("type", "capture")?;
            dict.set_item("name", name)?;
            dict.set_item("attribute", attribute)?;
            dict.set_item("bound", value_to_py(py, bound))?;
            dict.set_item("actual", optional_value_to_py(py, actual))?;
        }
    }
    Ok(dict.unbind())
}

fn detail_to_py(py: Python<'_>, detail: &core::NearMissDetail) -> PyResult<Py<PyDict>> {
    let dict = PyDict::new_bound(py);
    match detail {
        core::NearMissDetail::PredicateFailed {
            event_index,
            failures,
        } => {
            dict.set_item("kind", "predicate_failed")?;
            dict.set_item("event_index", *event_index)?;
            let list = PyList::empty_bound(py);
            for failure in failures {
                list.append(failure_to_py(py, failure)?)?;
            }
            dict.set_item("failures", list)?;
        }
        core::NearMissDetail::AbsenceBlocked {
            candidate_index,
            blocking_index,
            blocking_event_type,
            candidate_satisfies,
        } => {
            dict.set_item("kind", "absence_blocked")?;
            dict.set_item("candidate_index", *candidate_index)?;
            dict.set_item("blocking_index", *blocking_index)?;
            dict.set_item("blocking_event_type", blocking_event_type)?;
            dict.set_item("candidate_satisfies", *candidate_satisfies)?;
        }
        core::NearMissDetail::WindowExceeded {
            candidate_index,
            gap,
            max_elapsed,
        } => {
            dict.set_item("kind", "window_exceeded")?;
            dict.set_item("candidate_index", *candidate_index)?;
            dict.set_item("gap", *gap)?;
            dict.set_item("max_elapsed", *max_elapsed)?;
        }
        core::NearMissDetail::NoSuccessor => {
            dict.set_item("kind", "no_successor")?;
        }
    }
    Ok(dict.unbind())
}

/// A start that did not match, with its deepest partial path and the reason.
#[pyclass(name = "NearMiss")]
#[derive(Clone)]
struct PyNearMiss {
    partition: String,
    start_index: usize,
    indices: Vec<usize>,
    reached_steps: usize,
    next_event_type: String,
    detail: core::NearMissDetail,
    bindings: core::Bindings,
    events: Vec<PyEvent>,
}

impl PyNearMiss {
    fn from_core(value: &core::NearMiss, events: &[core::Event]) -> Self {
        let participating = value
            .participating_indices
            .iter()
            .map(|&index| PyEvent {
                inner: events[index].clone(),
            })
            .collect();
        Self {
            partition: value.partition.clone(),
            start_index: value.start_index,
            indices: value.participating_indices.clone(),
            reached_steps: value.reached_steps,
            next_event_type: value.next_event_type.clone(),
            detail: value.detail.clone(),
            bindings: value.bindings.clone(),
            events: participating,
        }
    }
}

#[pymethods]
impl PyNearMiss {
    #[getter]
    fn partition(&self) -> &str {
        &self.partition
    }

    #[getter]
    fn start_index(&self) -> usize {
        self.start_index
    }

    #[getter]
    fn indices(&self) -> Vec<usize> {
        self.indices.clone()
    }

    #[getter]
    fn reached_steps(&self) -> usize {
        self.reached_steps
    }

    #[getter]
    fn next_event_type(&self) -> &str {
        &self.next_event_type
    }

    /// One of: predicate_failed, absence_blocked, window_exceeded, no_successor.
    #[getter]
    fn reason(&self) -> &'static str {
        reason_str(self.detail.reason())
    }

    /// Reason-specific specifics as a dict (keyed by "kind"); includes failed
    /// clauses, blocking events, or window counterfactuals.
    #[getter]
    fn detail(&self, py: Python<'_>) -> PyResult<Py<PyDict>> {
        detail_to_py(py, &self.detail)
    }

    #[getter]
    fn events(&self) -> Vec<PyEvent> {
        self.events.clone()
    }

    #[getter]
    fn captures(&self, py: Python<'_>) -> PyResult<Py<PyDict>> {
        let captures = PyDict::new_bound(py);
        for (key, value) in &self.bindings {
            captures.set_item(key, value_to_py(py, value))?;
        }
        Ok(captures.unbind())
    }

    fn __repr__(&self) -> String {
        format!(
            "NearMiss(partition={:?}, indices={:?}, next={:?}, reason={:?})",
            self.partition,
            self.indices,
            self.next_event_type,
            reason_str(self.detail.reason())
        )
    }
}

fn coerce_pattern(obj: &Bound<'_, PyAny>) -> PyResult<core::Pattern> {
    if let Ok(pattern) = obj.extract::<PyPattern>() {
        return Ok(pattern.inner);
    }
    if let Ok(builder) = obj.extract::<PatternBuilder>() {
        return Ok(core::Pattern::sequence(builder.steps));
    }
    Err(PyTypeError::new_err(
        "expected a Pattern, a pattern builder, or the result of parse_pattern()",
    ))
}

/// Parse the Phase 1 text subset into a [`PyPattern`].
#[pyfunction]
fn parse_pattern(text: &str) -> PyResult<PyPattern> {
    core::parse_pattern(text)
        .map(|inner| PyPattern { inner })
        .map_err(|error| PyValueError::new_err(error.message().to_string()))
}

/// Build a pattern from the stable JSON AST, validating its structure.
#[pyfunction]
fn pattern_from_json(json: &str) -> PyResult<PyPattern> {
    core::pattern_from_json(json)
        .map(|inner| PyPattern { inner })
        .map_err(PyValueError::new_err)
}

/// Return a copy of `events` sorted by partition then `(timestamp, input order)`.
#[pyfunction]
fn sort_events(mut events: Vec<PyEvent>) -> Vec<PyEvent> {
    // Stable sort so original input order remains the per-timestamp tie-break.
    events.sort_by(|left, right| {
        left.inner
            .partition
            .cmp(&right.inner.partition)
            .then(left.inner.timestamp.cmp(&right.inner.timestamp))
    });
    events
}

/// Run a pattern over already-sorted events.
///
/// Raises `ValueError` if the events are not grouped by partition and sorted by
/// `(timestamp, input order)`; callers should use the high-level `match`
/// wrapper or [`sort_events`] first.
#[pyfunction]
#[pyo3(signature = (pattern, events, exhaustive = false, use_oracle = false))]
fn match_events(
    pattern: &Bound<'_, PyAny>,
    events: Vec<PyEvent>,
    exhaustive: bool,
    use_oracle: bool,
) -> PyResult<Vec<PyMatch>> {
    let core_events: Vec<core::Event> = events.into_iter().map(|event| event.inner).collect();
    if !core::is_sorted_by_partition_time_index(&core_events) {
        return Err(PyValueError::new_err(
            "events must be grouped by partition and sorted by (timestamp, input order); \
             call sort_events first",
        ));
    }

    let mut pattern = coerce_pattern(pattern)?;
    // Validate at the boundary so a structurally invalid pattern (however it was
    // obtained) surfaces as a Python ValueError rather than tripping the matcher's
    // internal `validate_pattern(...).expect(...)` and panicking across FFI.
    core::validate_pattern(&pattern).map_err(PyValueError::new_err)?;
    if exhaustive {
        pattern = pattern.with_consumption(core::MatchConsumption::ExhaustivePerStart);
    }

    let matches = if use_oracle {
        core::oracle_matches(&core_events, &pattern)
    } else {
        core::compiled_matches(&core_events, &pattern)
    };

    Ok(matches
        .iter()
        .map(|value| PyMatch::from_core(value, &core_events))
        .collect())
}

/// Explain near-misses: starts that did not match, with their deepest partial
/// path and the reason. Requires already-sorted events (see [`match_events`]).
#[pyfunction]
fn near_miss_events(pattern: &Bound<'_, PyAny>, events: Vec<PyEvent>) -> PyResult<Vec<PyNearMiss>> {
    let core_events: Vec<core::Event> = events.into_iter().map(|event| event.inner).collect();
    if !core::is_sorted_by_partition_time_index(&core_events) {
        return Err(PyValueError::new_err(
            "events must be grouped by partition and sorted by (timestamp, input order); \
             call sort_events first",
        ));
    }

    let pattern = coerce_pattern(pattern)?;
    // Same boundary validation as `match_events`: keep malformed patterns from
    // reaching the explainer's internal `expect` and panicking across FFI.
    core::validate_pattern(&pattern).map_err(PyValueError::new_err)?;
    Ok(core::near_misses(&core_events, &pattern)
        .iter()
        .map(|value| PyNearMiss::from_core(value, &core_events))
        .collect())
}

#[pymodule]
fn _core(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyEvent>()?;
    module.add_class::<PyPattern>()?;
    module.add_class::<PatternBuilder>()?;
    module.add_class::<PyMatch>()?;
    module.add_class::<PyNearMiss>()?;
    module.add_function(wrap_pyfunction!(parse_pattern, module)?)?;
    module.add_function(wrap_pyfunction!(pattern_from_json, module)?)?;
    module.add_function(wrap_pyfunction!(sort_events, module)?)?;
    module.add_function(wrap_pyfunction!(match_events, module)?)?;
    module.add_function(wrap_pyfunction!(near_miss_events, module)?)?;
    Ok(())
}
