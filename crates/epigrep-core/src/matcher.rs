use crate::model::*;

#[derive(Debug, Clone, PartialEq)]
pub struct CompiledPattern {
    steps: Vec<CompiledStep>,
    consumption: MatchConsumption,
}

#[derive(Debug, Clone, PartialEq)]
struct CompiledStep {
    atom: Atom,
    transition_from_previous: Option<Transition>,
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
