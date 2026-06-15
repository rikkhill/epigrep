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
pub struct CompiledPattern {
    steps: Vec<CompiledStep>,
    consumption: MatchConsumption,
}

impl CompiledPattern {
    pub fn compile(pattern: &Pattern) -> Self {
        validate_pattern(pattern);

        Self {
            steps: pattern
                .steps
                .iter()
                .map(|step| CompiledStep {
                    atom: step.atom.clone(),
                    transition_from_previous: step.transition_from_previous.clone(),
                })
                .collect(),
            consumption: pattern.consumption,
        }
    }

    pub fn matches(&self, events: &[Event]) -> Vec<Match> {
        let mut matches = Vec::new();
        let mut partition_start = 0;
        while partition_start < events.len() {
            let partition = &events[partition_start].partition;
            let partition_end = events[partition_start..]
                .iter()
                .position(|event| event.partition != *partition)
                .map_or(events.len(), |offset| partition_start + offset);

            matches.extend(self.match_partition(events, partition_start, partition_end));
            partition_start = partition_end;
        }

        matches
    }

    fn match_partition(
        &self,
        events: &[Event],
        partition_start: usize,
        partition_end: usize,
    ) -> Vec<Match> {
        let mut matches = Vec::new();

        for start_index in partition_start..partition_end {
            let Some(bindings) = self.steps[0]
                .atom
                .evaluate(&events[start_index], &Bindings::new())
            else {
                continue;
            };

            let paths = self.extend_path(events, partition_end, 1, vec![start_index], bindings);

            for (participating_indices, bindings) in paths {
                let first = participating_indices[0];
                let last = *participating_indices.last().expect("path is non-empty");
                matches.push(Match {
                    partition: events[first].partition.clone(),
                    participating_indices,
                    start_timestamp: events[first].timestamp,
                    end_timestamp: events[last].timestamp,
                    bindings,
                });
            }
        }

        matches
    }

    fn extend_path(
        &self,
        events: &[Event],
        partition_end: usize,
        step_index: usize,
        path: Vec<EventIndex>,
        bindings: Bindings,
    ) -> Vec<(Vec<EventIndex>, Bindings)> {
        if step_index == self.steps.len() {
            return vec![(path, bindings)];
        }

        let previous_index = *path.last().expect("path is non-empty");
        let step = &self.steps[step_index];
        let transition = step
            .transition_from_previous
            .as_ref()
            .expect("transition exists after first step");

        let mut paths = Vec::new();
        for candidate_index in previous_index + 1..partition_end {
            if transition_allows(
                events,
                previous_index,
                candidate_index,
                transition,
                &bindings,
            ) && let Some(next_bindings) =
                step.atom.evaluate(&events[candidate_index], &bindings)
            {
                let mut next_path = path.clone();
                next_path.push(candidate_index);
                paths.extend(self.extend_path(
                    events,
                    partition_end,
                    step_index + 1,
                    next_path,
                    next_bindings,
                ));

                if self.consumption == MatchConsumption::FirstSuccessorPerStart && !paths.is_empty()
                {
                    break;
                }
            }
        }

        paths
    }
}

#[derive(Debug, Clone, PartialEq)]
struct CompiledStep {
    atom: Atom,
    transition_from_previous: Option<Transition>,
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

    fn matches(&self, event: &Event, bindings: &Bindings) -> bool {
        self.evaluate(event, bindings).is_some()
    }

    fn evaluate(&self, event: &Event, bindings: &Bindings) -> Option<Bindings> {
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

pub fn oracle_matches(events: &[Event], pattern: &Pattern) -> Vec<Match> {
    validate_pattern(pattern);

    let mut matches = Vec::new();
    let mut partition_start = 0;
    while partition_start < events.len() {
        let partition = &events[partition_start].partition;
        let partition_end = events[partition_start..]
            .iter()
            .position(|event| event.partition != *partition)
            .map_or(events.len(), |offset| partition_start + offset);

        matches.extend(match_partition(
            events,
            partition_start,
            partition_end,
            pattern,
        ));
        partition_start = partition_end;
    }

    matches
}

pub fn compiled_matches(events: &[Event], pattern: &Pattern) -> Vec<Match> {
    CompiledPattern::compile(pattern).matches(events)
}

pub fn is_sorted_by_partition_time_index(events: &[Event]) -> bool {
    events.windows(2).all(|pair| {
        let previous = &pair[0];
        let next = &pair[1];
        previous.partition < next.partition
            || (previous.partition == next.partition && previous.timestamp <= next.timestamp)
    })
}

fn validate_pattern(pattern: &Pattern) {
    assert!(
        !pattern.steps.is_empty(),
        "patterns must contain at least one step"
    );
    assert!(
        pattern.steps[0].transition_from_previous.is_none(),
        "the first pattern step cannot have a transition"
    );
    assert!(
        pattern.steps[1..]
            .iter()
            .all(|step| step.transition_from_previous.is_some()),
        "every step after the first must have a transition"
    );
}

fn match_partition(
    events: &[Event],
    partition_start: usize,
    partition_end: usize,
    pattern: &Pattern,
) -> Vec<Match> {
    let mut matches = Vec::new();

    for start_index in partition_start..partition_end {
        let Some(bindings) = pattern.steps[0]
            .atom
            .evaluate(&events[start_index], &Bindings::new())
        else {
            continue;
        };

        let paths = extend_path(
            events,
            partition_end,
            pattern,
            1,
            vec![start_index],
            bindings,
            pattern.consumption,
        );

        for (participating_indices, bindings) in paths {
            let first = participating_indices[0];
            let last = *participating_indices.last().expect("path is non-empty");
            matches.push(Match {
                partition: events[first].partition.clone(),
                participating_indices,
                start_timestamp: events[first].timestamp,
                end_timestamp: events[last].timestamp,
                bindings,
            });
        }
    }

    matches
}

fn extend_path(
    events: &[Event],
    partition_end: usize,
    pattern: &Pattern,
    step_index: usize,
    path: Vec<EventIndex>,
    bindings: Bindings,
    consumption: MatchConsumption,
) -> Vec<(Vec<EventIndex>, Bindings)> {
    if step_index == pattern.steps.len() {
        return vec![(path, bindings)];
    }

    let previous_index = *path.last().expect("path is non-empty");
    let step = &pattern.steps[step_index];
    let transition = step
        .transition_from_previous
        .as_ref()
        .expect("transition exists after first step");

    let mut paths = Vec::new();
    for candidate_index in previous_index + 1..partition_end {
        if transition_allows(
            events,
            previous_index,
            candidate_index,
            transition,
            &bindings,
        ) && let Some(next_bindings) = step.atom.evaluate(&events[candidate_index], &bindings)
        {
            let mut next_path = path.clone();
            next_path.push(candidate_index);
            paths.extend(extend_path(
                events,
                partition_end,
                pattern,
                step_index + 1,
                next_path,
                next_bindings,
                consumption,
            ));

            if consumption == MatchConsumption::FirstSuccessorPerStart && !paths.is_empty() {
                break;
            }
        }
    }

    paths
}

fn transition_allows(
    events: &[Event],
    previous_index: EventIndex,
    candidate_index: EventIndex,
    transition: &Transition,
    bindings: &Bindings,
) -> bool {
    let previous = &events[previous_index];
    let candidate = &events[candidate_index];

    if candidate.timestamp < previous.timestamp {
        return false;
    }

    if let Some(max_elapsed) = transition.max_elapsed {
        if candidate.timestamp - previous.timestamp > max_elapsed {
            return false;
        }
    }

    !events[previous_index + 1..candidate_index]
        .iter()
        .any(|event| {
            transition
                .absence
                .iter()
                .any(|absent_atom| absent_atom.matches(event, bindings))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn atom(event_type: &str) -> Atom {
        Atom::event_type(event_type)
    }

    fn sequence(first: &str, second: &str, transition: Transition) -> Pattern {
        Pattern::sequence(vec![
            Step::first(atom(first)),
            Step::then(atom(second), transition),
        ])
    }

    fn indices(matches: &[Match]) -> Vec<Vec<EventIndex>> {
        matches
            .iter()
            .map(|match_| match_.participating_indices.clone())
            .collect()
    }

    #[test]
    fn matches_independently_within_each_partition() {
        let events = vec![
            Event::new("child-1", 0, "A"),
            Event::new("child-1", 1, "B"),
            Event::new("child-2", 0, "A"),
            Event::new("child-2", 1, "noise"),
            Event::new("child-2", 2, "B"),
        ];
        let pattern = sequence("A", "B", Transition::any());

        assert_eq!(
            indices(&oracle_matches(&events, &pattern)),
            vec![vec![0, 1], vec![2, 4]]
        );
        assert_eq!(
            oracle_matches(&events, &pattern),
            compiled_matches(&events, &pattern)
        );
    }

    #[test]
    fn window_boundary_is_inclusive() {
        let events = vec![Event::new("p", 10, "A"), Event::new("p", 15, "B")];
        let pattern = sequence("A", "B", Transition::any().within(5));

        assert_eq!(
            indices(&oracle_matches(&events, &pattern)),
            vec![vec![0, 1]]
        );
    }

    #[test]
    fn window_excludes_events_after_the_boundary() {
        let events = vec![Event::new("p", 10, "A"), Event::new("p", 16, "B")];
        let pattern = sequence("A", "B", Transition::any().within(5));

        assert!(oracle_matches(&events, &pattern).is_empty());
    }

    #[test]
    fn same_timestamp_order_uses_input_index_tie_breaking() {
        let events = vec![
            Event::new("p", 10, "A"),
            Event::new("p", 10, "B"),
            Event::new("p", 10, "A"),
        ];
        let pattern = sequence("A", "B", Transition::any().within(0));

        assert_eq!(
            indices(&oracle_matches(&events, &pattern)),
            vec![vec![0, 1]]
        );
    }

    #[test]
    fn absence_between_blocks_same_timestamp_event_ordered_between_a_and_b() {
        let events = vec![
            Event::new("p", 10, "A"),
            Event::new("p", 10, "C"),
            Event::new("p", 10, "B"),
        ];
        let pattern = sequence("A", "B", Transition::any().with_absence(atom("C")));

        assert!(oracle_matches(&events, &pattern).is_empty());
    }

    #[test]
    fn absence_between_blocks_event_at_b_timestamp_ordered_before_b() {
        let events = vec![
            Event::new("p", 9, "A"),
            Event::new("p", 10, "C"),
            Event::new("p", 10, "B"),
        ];
        let pattern = sequence("A", "B", Transition::any().with_absence(atom("C")));

        assert!(oracle_matches(&events, &pattern).is_empty());
    }

    #[test]
    fn absence_between_ignores_same_timestamp_event_outside_ordered_interval() {
        let events = vec![
            Event::new("p", 10, "C"),
            Event::new("p", 10, "A"),
            Event::new("p", 10, "B"),
            Event::new("p", 10, "C"),
        ];
        let pattern = sequence("A", "B", Transition::any().with_absence(atom("C")));

        assert_eq!(
            indices(&oracle_matches(&events, &pattern)),
            vec![vec![1, 2]]
        );
    }

    #[test]
    fn absence_between_respects_absent_atom_predicates() {
        let events = vec![
            Event::new("p", 0, "A"),
            Event::new("p", 1, "C").with_attr("kind", "allowed".into()),
            Event::new("p", 2, "B"),
        ];
        let absent =
            atom("C").with_predicate(Predicate::new("kind", ComparisonOperator::Eq, "blocked"));
        let pattern = sequence("A", "B", Transition::any().with_absence(absent));

        assert_eq!(
            indices(&oracle_matches(&events, &pattern)),
            vec![vec![0, 2]]
        );
    }

    #[test]
    fn returns_overlapping_matches_by_start_position() {
        let events = vec![
            Event::new("p", 0, "A"),
            Event::new("p", 1, "A"),
            Event::new("p", 2, "B"),
        ];
        let pattern = sequence("A", "B", Transition::any());

        assert_eq!(
            indices(&oracle_matches(&events, &pattern)),
            vec![vec![0, 2], vec![1, 2]]
        );
    }

    #[test]
    fn exhaustive_consumption_returns_every_successor_per_start() {
        let events = vec![
            Event::new("p", 0, "A"),
            Event::new("p", 1, "B"),
            Event::new("p", 2, "B"),
        ];
        let pattern = sequence("A", "B", Transition::any())
            .with_consumption(MatchConsumption::ExhaustivePerStart);

        assert_eq!(
            indices(&oracle_matches(&events, &pattern)),
            vec![vec![0, 1], vec![0, 2]]
        );
    }

    #[test]
    fn predicates_filter_event_atoms() {
        let events = vec![
            Event::new("p", 0, "A"),
            Event::new("p", 1, "B").with_attr("score", 2_i64.into()),
            Event::new("p", 2, "B").with_attr("score", 4_i64.into()),
        ];
        let pattern = Pattern::sequence(vec![
            Step::first(atom("A")),
            Step::then(
                atom("B").with_predicate(Predicate::new("score", ComparisonOperator::Gt, 3_i64)),
                Transition::any(),
            ),
        ]);

        assert_eq!(
            indices(&oracle_matches(&events, &pattern)),
            vec![vec![0, 2]]
        );
    }

    #[test]
    fn care_pathway_shaped_example() {
        let events = vec![
            Event::new("child-1", 0, "entered_care"),
            Event::new("child-1", 2, "placement_change"),
            Event::new("child-1", 5, "safeguarding_flag").with_attr("severity", 4_i64.into()),
            Event::new("child-2", 0, "entered_care"),
            Event::new("child-2", 7, "safeguarding_flag").with_attr("severity", 4_i64.into()),
        ];
        let pattern = Pattern::sequence(vec![
            Step::first(atom("entered_care")),
            Step::then(
                atom("safeguarding_flag").with_predicate(Predicate::new(
                    "severity",
                    ComparisonOperator::Gte,
                    3_i64,
                )),
                Transition::any()
                    .within(5)
                    .with_absence(atom("placement_change")),
            ),
        ]);

        assert_eq!(
            indices(&oracle_matches(&events, &pattern)),
            Vec::<Vec<usize>>::new()
        );
    }

    #[test]
    fn log_trace_shaped_example() {
        let events = vec![
            Event::new("node-a", 0, "config_reload"),
            Event::new("node-a", 60, "heartbeat"),
            Event::new("node-a", 119, "oom_killed").with_attr("pod", "api".into()),
            Event::new("node-b", 0, "config_reload"),
            Event::new("node-b", 121, "oom_killed").with_attr("pod", "worker".into()),
        ];
        let pattern = Pattern::sequence(vec![
            Step::first(atom("config_reload")),
            Step::then(atom("oom_killed"), Transition::any().within(120)),
        ]);

        assert_eq!(
            indices(&oracle_matches(&events, &pattern)),
            vec![vec![0, 2]]
        );
    }

    #[test]
    fn capture_binding_requires_later_reference_equality() {
        let events = vec![
            Event::new("p", 0, "A").with_attr("user_id", "u1".into()),
            Event::new("p", 1, "B").with_attr("user_id", "u2".into()),
            Event::new("p", 2, "B").with_attr("user_id", "u1".into()),
        ];
        let pattern = Pattern::sequence(vec![
            Step::first(atom("A").with_capture(Capture::new("a_user", "user_id"))),
            Step::then(
                atom("B").with_reference_predicate(ReferencePredicate::new(
                    "user_id",
                    ComparisonOperator::Eq,
                    "a_user",
                )),
                Transition::any(),
            ),
        ]);

        let matches = oracle_matches(&events, &pattern);

        assert_eq!(indices(&matches), vec![vec![0, 2]]);
        assert_eq!(
            matches[0].bindings.get("a_user"),
            Some(&Value::String("u1".to_owned()))
        );
        assert_eq!(matches, compiled_matches(&events, &pattern));
    }

    #[test]
    fn reference_predicate_without_a_binding_does_not_match() {
        let events = vec![
            Event::new("p", 0, "A"),
            Event::new("p", 1, "B").with_attr("user_id", "u1".into()),
        ];
        let pattern = Pattern::sequence(vec![
            Step::first(atom("A")),
            Step::then(
                atom("B").with_reference_predicate(ReferencePredicate::new(
                    "user_id",
                    ComparisonOperator::Eq,
                    "a_user",
                )),
                Transition::any(),
            ),
        ]);

        assert!(oracle_matches(&events, &pattern).is_empty());
        assert_eq!(
            oracle_matches(&events, &pattern),
            compiled_matches(&events, &pattern)
        );
    }

    #[test]
    fn recapturing_existing_binding_requires_the_same_value() {
        let events = vec![
            Event::new("p", 0, "A").with_attr("user_id", "u1".into()),
            Event::new("p", 1, "B").with_attr("user_id", "u2".into()),
            Event::new("p", 2, "B").with_attr("user_id", "u1".into()),
        ];
        let pattern = Pattern::sequence(vec![
            Step::first(atom("A").with_capture(Capture::new("user", "user_id"))),
            Step::then(
                atom("B").with_capture(Capture::new("user", "user_id")),
                Transition::any(),
            ),
        ]);

        assert_eq!(
            indices(&oracle_matches(&events, &pattern)),
            vec![vec![0, 2]]
        );
    }

    #[test]
    fn absence_guard_can_use_a_captured_binding() {
        let events = vec![
            Event::new("p", 0, "A").with_attr("user_id", "u1".into()),
            Event::new("p", 1, "C").with_attr("user_id", "u2".into()),
            Event::new("p", 2, "B").with_attr("user_id", "u1".into()),
            Event::new("p", 3, "A").with_attr("user_id", "u3".into()),
            Event::new("p", 4, "C").with_attr("user_id", "u3".into()),
            Event::new("p", 5, "B").with_attr("user_id", "u3".into()),
        ];
        let absent = atom("C").with_reference_predicate(ReferencePredicate::new(
            "user_id",
            ComparisonOperator::Eq,
            "user",
        ));
        let pattern = Pattern::sequence(vec![
            Step::first(atom("A").with_capture(Capture::new("user", "user_id"))),
            Step::then(
                atom("B").with_reference_predicate(ReferencePredicate::new(
                    "user_id",
                    ComparisonOperator::Eq,
                    "user",
                )),
                Transition::any().with_absence(absent),
            ),
        ])
        .with_consumption(MatchConsumption::ExhaustivePerStart);

        assert_eq!(
            indices(&oracle_matches(&events, &pattern)),
            vec![vec![0, 2]]
        );
        assert_eq!(
            oracle_matches(&events, &pattern),
            compiled_matches(&events, &pattern)
        );
    }

    #[test]
    fn compiled_matcher_matches_oracle_for_small_generated_streams() {
        let event_types = ["A", "B", "C"];
        let patterns = vec![
            sequence("A", "B", Transition::any()),
            sequence("A", "B", Transition::any().within(1)),
            sequence("A", "B", Transition::any().with_absence(atom("C"))),
            sequence(
                "A",
                "B",
                Transition::any().within(2).with_absence(atom("C")),
            ),
            sequence("A", "B", Transition::any())
                .with_consumption(MatchConsumption::ExhaustivePerStart),
        ];

        for stream_len in 0..=4 {
            let stream_count = event_types.len().pow(stream_len);
            for stream_id in 0..stream_count {
                let mut remaining = stream_id;
                let mut events = Vec::new();

                for index in 0..stream_len {
                    let event_type = event_types[remaining % event_types.len()];
                    remaining /= event_types.len();
                    events.push(Event::new("p", index as Timestamp / 2, event_type));
                }

                for pattern in &patterns {
                    assert_eq!(
                        oracle_matches(&events, pattern),
                        compiled_matches(&events, pattern),
                        "events: {events:?}, pattern: {pattern:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn compiled_pattern_can_be_reused_across_event_streams() {
        let pattern = sequence("A", "B", Transition::any().within(5));
        let compiled = CompiledPattern::compile(&pattern);

        let first_stream = vec![Event::new("p", 0, "A"), Event::new("p", 5, "B")];
        let second_stream = vec![Event::new("p", 0, "A"), Event::new("p", 6, "B")];

        assert_eq!(indices(&compiled.matches(&first_stream)), vec![vec![0, 1]]);
        assert!(compiled.matches(&second_stream).is_empty());
    }
}

#[cfg(test)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;

    fn event_strategy() -> impl Strategy<Value = Event> {
        (
            prop_oneof![Just("p1"), Just("p2")],
            0_i64..=8,
            prop_oneof![Just("A"), Just("B"), Just("C")],
            prop::option::of(0_i64..=5),
            prop::option::of(prop_oneof![Just("u1"), Just("u2"), Just("u3")]),
        )
            .prop_map(|(partition, timestamp, event_type, score, user_id)| {
                let mut event = Event::new(partition, timestamp, event_type);
                if let Some(score) = score {
                    event = event.with_attr("score", score.into());
                }
                if let Some(user_id) = user_id {
                    event = event.with_attr("user_id", user_id.into());
                }
                event
            })
    }

    fn stream_strategy() -> impl Strategy<Value = Vec<Event>> {
        prop::collection::vec(event_strategy(), 0..=12).prop_map(|mut events| {
            events.sort_by(|left, right| {
                left.partition
                    .cmp(&right.partition)
                    .then(left.timestamp.cmp(&right.timestamp))
            });
            events
        })
    }

    fn atom_strategy() -> impl Strategy<Value = Atom> {
        (
            prop_oneof![Just("A"), Just("B"), Just("C")],
            any::<bool>(),
            any::<bool>(),
            any::<bool>(),
            any::<bool>(),
        )
            .prop_map(
                |(event_type, require_score, capture_user, reference_user, reference_score)| {
                    let mut atom = Atom::event_type(event_type);
                    if require_score {
                        atom = atom.with_predicate(Predicate::new(
                            "score",
                            ComparisonOperator::Gte,
                            2_i64,
                        ));
                    }
                    if capture_user {
                        atom = atom.with_capture(Capture::new("user", "user_id"));
                    }
                    if reference_user {
                        atom = atom.with_reference_predicate(ReferencePredicate::new(
                            "user_id",
                            ComparisonOperator::Eq,
                            "user",
                        ));
                    }
                    if reference_score {
                        atom = atom.with_reference_predicate(ReferencePredicate::new(
                            "score",
                            ComparisonOperator::Gte,
                            "score",
                        ));
                    }
                    atom
                },
            )
    }

    fn absent_atom_strategy() -> impl Strategy<Value = Atom> {
        (
            prop_oneof![Just("A"), Just("B"), Just("C")],
            any::<bool>(),
            any::<bool>(),
        )
            .prop_map(|(event_type, require_score, reference_user)| {
                let mut atom = Atom::event_type(event_type);
                if require_score {
                    atom = atom.with_predicate(Predicate::new(
                        "score",
                        ComparisonOperator::Gte,
                        2_i64,
                    ));
                }
                if reference_user {
                    atom = atom.with_reference_predicate(ReferencePredicate::new(
                        "user_id",
                        ComparisonOperator::Eq,
                        "user",
                    ));
                }
                atom
            })
    }

    fn transition_strategy() -> impl Strategy<Value = Transition> {
        (
            prop::option::of(0_i64..=4),
            prop::option::of(absent_atom_strategy()),
        )
            .prop_map(|(max_elapsed, absent_atom)| {
                let mut transition = Transition::any();
                if let Some(max_elapsed) = max_elapsed {
                    transition = transition.within(max_elapsed);
                }
                if let Some(absent_atom) = absent_atom {
                    transition = transition.with_absence(absent_atom);
                }
                transition
            })
    }

    fn pattern_strategy() -> impl Strategy<Value = Pattern> {
        (
            atom_strategy(),
            atom_strategy(),
            prop::option::of(atom_strategy()),
            transition_strategy(),
            transition_strategy(),
            prop_oneof![
                Just(MatchConsumption::FirstSuccessorPerStart),
                Just(MatchConsumption::ExhaustivePerStart),
            ],
        )
            .prop_map(
                |(first, second, third, first_transition, second_transition, consumption)| {
                    let mut steps = vec![Step::first(first), Step::then(second, first_transition)];
                    if let Some(third) = third {
                        steps.push(Step::then(third, second_transition));
                    }
                    Pattern::sequence(steps).with_consumption(consumption)
                },
            )
    }

    proptest! {
        #[test]
        fn compiled_matches_oracle_for_generated_streams_and_patterns(
            events in stream_strategy(),
            pattern in pattern_strategy(),
        ) {
            prop_assert_eq!(
                compiled_matches(&events, &pattern),
                oracle_matches(&events, &pattern)
            );
        }
    }
}
