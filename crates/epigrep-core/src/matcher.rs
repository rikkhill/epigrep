use crate::model::*;

/// A pattern compiled for execution as a forward NFA-style simulation.
///
/// This backend is deliberately a *different* execution model from the
/// recursive backtracking [`oracle_matches`]. It performs a single
/// left-to-right sweep over each partition while maintaining a set of in-flight
/// partial matches ("threads"). Each thread carries its own capture bindings
/// (the bounded registers), the index of its last consumed event (the anchor
/// for window and absence guards), and an absence-violation flag updated
/// incrementally as the sweep passes intervening events.
///
/// Because it shares no sequencing, window, absence, or consumption logic with
/// the oracle, the property tests comparing the two are a real check: a
/// divergence is a genuine semantic bug rather than a copy of one. Leaf
/// predicate evaluation ([`Atom::evaluate`]) is intentionally shared — it is
/// the atomic match unit, not the sequencing strategy under test.
#[derive(Debug, Clone, PartialEq)]
pub struct CompiledPattern {
    steps: Vec<CompiledStep>,
    consumption: MatchConsumption,
}

#[derive(Debug, Clone, PartialEq)]
struct CompiledStep {
    atom: Atom,
    /// Guard on the transition *into* this step. Empty/`None` for the first step.
    max_elapsed: Option<Timestamp>,
    absence: Vec<Atom>,
}

#[derive(Debug, Clone)]
struct Thread {
    /// Index of the next step this thread is trying to match.
    next_step: usize,
    participating: Vec<EventIndex>,
    bindings: Bindings,
    /// Index of the most recently consumed event; anchor for window + absence.
    last_index: EventIndex,
    /// Set once an absence atom has matched within the open interval
    /// `(last_index, current)` for the transition into `next_step`.
    absence_blocked: bool,
}

impl CompiledPattern {
    pub fn compile(pattern: &Pattern) -> Self {
        validate_pattern(pattern).expect("compile requires a structurally valid pattern");

        let steps = pattern
            .steps
            .iter()
            .map(|step| {
                let (max_elapsed, absence) = match &step.transition_from_previous {
                    Some(transition) => (transition.max_elapsed, transition.absence.clone()),
                    None => (None, Vec::new()),
                };
                CompiledStep {
                    atom: step.atom.clone(),
                    max_elapsed,
                    absence,
                }
            })
            .collect();

        Self {
            steps,
            consumption: pattern.consumption,
        }
    }

    pub fn matches(&self, events: &[Event]) -> Vec<Match> {
        debug_assert!(
            is_sorted_by_partition_time_index(events),
            "matcher input must be grouped by partition and sorted by (timestamp, index)"
        );

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
        let mut threads: Vec<Thread> = Vec::new();
        let mut matches = Vec::new();

        for index in partition_start..partition_end {
            let event = &events[index];
            let mut next_threads: Vec<Thread> = Vec::with_capacity(threads.len() + 1);

            for mut thread in threads.drain(..) {
                let step = &self.steps[thread.next_step];

                // A bounded window can only be exceeded further as the sweep
                // advances (timestamps are non-decreasing), so an expired thread
                // is dead for good and is dropped.
                if let Some(max_elapsed) = step.max_elapsed
                    && event.timestamp - events[thread.last_index].timestamp > max_elapsed
                {
                    continue;
                }

                let candidate = if thread.absence_blocked {
                    None
                } else {
                    step.atom.evaluate(event, &thread.bindings)
                };

                if let Some(next_bindings) = candidate {
                    let mut participating = thread.participating.clone();
                    participating.push(index);

                    if thread.next_step + 1 == self.steps.len() {
                        matches.push(build_match(events, participating, next_bindings));
                    } else {
                        next_threads.push(Thread {
                            next_step: thread.next_step + 1,
                            participating,
                            bindings: next_bindings,
                            last_index: index,
                            absence_blocked: false,
                        });
                    }

                    // First-successor commits to this candidate and drops the
                    // waiting thread; exhaustive keeps it alive to find more.
                    if self.consumption == MatchConsumption::ExhaustivePerStart {
                        if absence_matches(step, event, &thread.bindings) {
                            thread.absence_blocked = true;
                        }
                        next_threads.push(thread);
                    }
                } else {
                    // Not a candidate: a matching absence atom anywhere in the
                    // open interval blocks every later candidate for this thread.
                    if absence_matches(step, event, &thread.bindings) {
                        thread.absence_blocked = true;
                    }
                    next_threads.push(thread);
                }
            }

            threads = next_threads;

            // Every position is a candidate start, so overlapping matches are
            // found by spawning a fresh thread wherever the first step matches.
            if let Some(bindings) = self.steps[0].atom.evaluate(event, &Bindings::new()) {
                if self.steps.len() == 1 {
                    matches.push(build_match(events, vec![index], bindings));
                } else {
                    threads.push(Thread {
                        next_step: 1,
                        participating: vec![index],
                        bindings,
                        last_index: index,
                        absence_blocked: false,
                    });
                }
            }
        }

        // The oracle emits matches in depth-first order, which within a
        // partition is lexicographic by participating indices. Sort to match so
        // the two backends are comparable by value.
        matches.sort_by(|left, right| left.participating_indices.cmp(&right.participating_indices));
        matches
    }
}

fn absence_matches(step: &CompiledStep, event: &Event, bindings: &Bindings) -> bool {
    step.absence
        .iter()
        .any(|absent_atom| absent_atom.matches(event, bindings))
}

fn build_match(events: &[Event], participating: Vec<EventIndex>, bindings: Bindings) -> Match {
    let first = participating[0];
    let last = *participating.last().expect("match has at least one event");
    Match {
        partition: events[first].partition.clone(),
        participating_indices: participating,
        start_timestamp: events[first].timestamp,
        end_timestamp: events[last].timestamp,
        bindings,
    }
}

/// Naive reference matcher: depth-first backtracking over candidate paths.
///
/// This is the executable definition of epigrep's semantics. It is written for
/// obviousness, not speed, and is the source of truth that [`CompiledPattern`]
/// is checked against.
pub fn oracle_matches(events: &[Event], pattern: &Pattern) -> Vec<Match> {
    validate_pattern(pattern).expect("oracle_matches requires a structurally valid pattern");
    debug_assert!(
        is_sorted_by_partition_time_index(events),
        "matcher input must be grouped by partition and sorted by (timestamp, index)"
    );

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

/// Check a pattern's structural invariants: at least one step, a first step with
/// no incoming transition, and every later step carrying one.
///
/// This is the fallible validation path on the library surface. The matchers
/// ([`CompiledPattern::compile`], [`compiled_matches`], [`oracle_matches`])
/// assume a valid pattern and will panic on an invalid one, so a Rust caller
/// constructing a [`Pattern`] by hand should call this first. Patterns produced
/// by the Python builder, the text parser, or the JSON loader are already valid.
pub fn validate_pattern(pattern: &Pattern) -> Result<(), String> {
    if pattern.steps.is_empty() {
        return Err("pattern must contain at least one step".to_owned());
    }
    if pattern.steps[0].transition_from_previous.is_some() {
        return Err("the first pattern step cannot have a transition".to_owned());
    }
    if pattern.steps[1..]
        .iter()
        .any(|step| step.transition_from_previous.is_none())
    {
        return Err("every step after the first must have a transition".to_owned());
    }
    Ok(())
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
            matches.push(build_match(events, participating_indices, bindings));
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

            // First-successor semantics commit to the earliest candidate that
            // satisfies this step and its transition, regardless of whether the
            // remainder of the pattern completes from there.
            if consumption == MatchConsumption::FirstSuccessorPerStart {
                break;
            }
        }
    }

    paths
}

pub(crate) fn transition_allows(
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

    if let Some(max_elapsed) = transition.max_elapsed
        && candidate.timestamp - previous.timestamp > max_elapsed
    {
        return false;
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
