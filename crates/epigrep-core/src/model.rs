use std::collections::BTreeMap;

pub type Timestamp = i64;
pub type EventIndex = usize;
pub type Bindings = BTreeMap<String, Value>;

#[derive(Debug, Clone, PartialEq)]
pub struct Event {
    pub partition: String,
    pub timestamp: Timestamp,
    pub event_type: String,
    pub attributes: BTreeMap<String, Value>,
}

impl Event {
    pub fn new(
        partition: impl Into<String>,
        timestamp: Timestamp,
        event_type: impl Into<String>,
    ) -> Self {
        Self {
            partition: partition.into(),
            timestamp,
            event_type: event_type.into(),
            attributes: BTreeMap::new(),
        }
    }

    pub fn with_attr(mut self, key: impl Into<String>, value: Value) -> Self {
        self.attributes.insert(key.into(), value);
        self
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    String(String),
    Int(i64),
    Float(f64),
    Bool(bool),
    Null,
}

impl From<&str> for Value {
    fn from(value: &str) -> Self {
        Self::String(value.to_owned())
    }
}

impl From<i64> for Value {
    fn from(value: i64) -> Self {
        Self::Int(value)
    }
}

impl From<bool> for Value {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchConsumption {
    FirstSuccessorPerStart,
    ExhaustivePerStart,
}

impl Default for MatchConsumption {
    fn default() -> Self {
        Self::FirstSuccessorPerStart
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Pattern {
    pub steps: Vec<Step>,
    pub consumption: MatchConsumption,
}

impl Pattern {
    pub fn sequence(steps: impl Into<Vec<Step>>) -> Self {
        Self {
            steps: steps.into(),
            consumption: MatchConsumption::default(),
        }
    }

    pub fn with_consumption(mut self, consumption: MatchConsumption) -> Self {
        self.consumption = consumption;
        self
    }
}
#[derive(Debug, Clone, PartialEq)]
pub struct Step {
    pub atom: Atom,
    pub transition_from_previous: Option<Transition>,
}

impl Step {
    pub fn first(atom: Atom) -> Self {
        Self {
            atom,
            transition_from_previous: None,
        }
    }

    pub fn then(atom: Atom, transition: Transition) -> Self {
        Self {
            atom,
            transition_from_previous: Some(transition),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Atom {
    pub event_type: String,
    pub predicates: Vec<Predicate>,
    pub reference_predicates: Vec<ReferencePredicate>,
    pub captures: Vec<Capture>,
}

impl Atom {
    pub fn event_type(event_type: impl Into<String>) -> Self {
        Self {
            event_type: event_type.into(),
            predicates: Vec::new(),
            reference_predicates: Vec::new(),
            captures: Vec::new(),
        }
    }

    pub fn with_predicate(mut self, predicate: Predicate) -> Self {
        self.predicates.push(predicate);
        self
    }

    pub fn with_reference_predicate(mut self, predicate: ReferencePredicate) -> Self {
        self.reference_predicates.push(predicate);
        self
    }

    pub fn with_capture(mut self, capture: Capture) -> Self {
        self.captures.push(capture);
        self
    }

    pub(crate) fn matches(&self, event: &Event, bindings: &Bindings) -> bool {
        self.evaluate(event, bindings).is_some()
    }

    pub(crate) fn evaluate(&self, event: &Event, bindings: &Bindings) -> Option<Bindings> {
        if self.event_type != event.event_type {
            return None;
        }

        if !self
            .predicates
            .iter()
            .all(|predicate| predicate.matches(event))
        {
            return None;
        }

        if !self
            .reference_predicates
            .iter()
            .all(|predicate| predicate.matches(event, bindings))
        {
            return None;
        }

        let mut next_bindings = bindings.clone();
        for capture in &self.captures {
            let value = event
                .attributes
                .get(&capture.attribute)
                .cloned()
                .unwrap_or(Value::Null);

            if let Some(existing) = next_bindings.get(&capture.name) {
                if existing != &value {
                    return None;
                }
            } else {
                next_bindings.insert(capture.name.clone(), value);
            }
        }

        Some(next_bindings)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Capture {
    pub name: String,
    pub attribute: String,
}

impl Capture {
    pub fn new(name: impl Into<String>, attribute: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            attribute: attribute.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReferencePredicate {
    pub attribute: String,
    pub operator: ComparisonOperator,
    pub binding: String,
}

impl ReferencePredicate {
    pub fn new(
        attribute: impl Into<String>,
        operator: ComparisonOperator,
        binding: impl Into<String>,
    ) -> Self {
        Self {
            attribute: attribute.into(),
            operator,
            binding: binding.into(),
        }
    }

    fn matches(&self, event: &Event, bindings: &Bindings) -> bool {
        let Some(actual) = event.attributes.get(&self.attribute) else {
            return false;
        };
        let Some(expected) = bindings.get(&self.binding) else {
            return false;
        };
        self.operator.matches(actual, expected)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Predicate {
    pub attribute: String,
    pub operator: ComparisonOperator,
    pub value: Value,
}

impl Predicate {
    pub fn new(
        attribute: impl Into<String>,
        operator: ComparisonOperator,
        value: impl Into<Value>,
    ) -> Self {
        Self {
            attribute: attribute.into(),
            operator,
            value: value.into(),
        }
    }

    fn matches(&self, event: &Event) -> bool {
        let Some(actual) = event.attributes.get(&self.attribute) else {
            return false;
        };
        self.operator.matches(actual, &self.value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComparisonOperator {
    Eq,
    NotEq,
    Gt,
    Gte,
    Lt,
    Lte,
}

impl ComparisonOperator {
    fn matches(self, actual: &Value, expected: &Value) -> bool {
        match self {
            Self::Eq => actual == expected,
            Self::NotEq => actual != expected,
            Self::Gt => compare_numbers(actual, expected).is_some_and(|ordering| ordering > 0.0),
            Self::Gte => compare_numbers(actual, expected).is_some_and(|ordering| ordering >= 0.0),
            Self::Lt => compare_numbers(actual, expected).is_some_and(|ordering| ordering < 0.0),
            Self::Lte => compare_numbers(actual, expected).is_some_and(|ordering| ordering <= 0.0),
        }
    }
}

fn compare_numbers(actual: &Value, expected: &Value) -> Option<f64> {
    Some(number(actual)? - number(expected)?)
}

fn number(value: &Value) -> Option<f64> {
    match value {
        Value::Int(value) => Some(*value as f64),
        Value::Float(value) => Some(*value),
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Transition {
    pub max_elapsed: Option<Timestamp>,
    pub absence: Vec<Atom>,
}

impl Transition {
    pub fn any() -> Self {
        Self::default()
    }

    pub fn within(mut self, max_elapsed: Timestamp) -> Self {
        self.max_elapsed = Some(max_elapsed);
        self
    }

    pub fn with_absence(mut self, atom: Atom) -> Self {
        self.absence.push(atom);
        self
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Match {
    pub partition: String,
    pub participating_indices: Vec<EventIndex>,
    pub start_timestamp: Timestamp,
    pub end_timestamp: Timestamp,
    pub bindings: Bindings,
}
