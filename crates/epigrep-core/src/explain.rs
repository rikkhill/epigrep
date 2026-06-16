//! Near-miss explanations: for each candidate start that does not produce a
//! full match, report how far it reached and why it could not continue.
//!
//! This is a diagnostic pass, deliberately built on the oracle's exhaustive
//! backtracking style rather than the compiled NFA — it favours clarity over
//! speed and never affects matching itself.
//!
//! Semantics (explicit, like the rest of the kernel):
//!
//! * A *start* is any event satisfying the pattern's first step.
//! * Explanation is **existence-based and independent of match consumption**: a
//!   start is a near-miss iff *no* full match exists from it (explored
//!   exhaustively). Starts that can complete are reported by the matcher, not
//!   here.
//! * For a near-miss, the reported path is the **deepest reachable** partial
//!   path from that start (ties broken by lexicographically smallest indices),
//!   and the reason classifies why the next step could not be satisfied from the
//!   path's frontier, by "nearest miss" priority:
//!   `PredicateFailed` > `AbsenceBlocked` > `WindowExceeded` > `NoSuccessor`.

use crate::matcher::{transition_allows, validate_pattern};
use crate::model::*;

/// Why a near-miss could not extend past its frontier.
///
/// Ordered from nearest to furthest miss; see [`NearMissReason::priority`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NearMissReason {
    /// A right-type, in-window, absence-clear candidate existed, but its
    /// predicate or reference check failed.
    PredicateFailed,
    /// A right-type, in-window candidate existed, but an absence atom matched an
    /// event between the frontier and that candidate.
    AbsenceBlocked,
    /// A right-type candidate existed, but only beyond the window.
    WindowExceeded,
    /// No event of the required type occurs after the frontier.
    NoSuccessor,
}

impl NearMissReason {
    fn priority(self) -> u8 {
        match self {
            Self::NoSuccessor => 0,
            Self::WindowExceeded => 1,
            Self::AbsenceBlocked => 2,
            Self::PredicateFailed => 3,
        }
    }

    /// Keep whichever reason is the nearer miss.
    fn nearer(self, other: Self) -> Self {
        if other.priority() > self.priority() {
            other
        } else {
            self
        }
    }
}

/// A start that did not complete, with its deepest partial path and the reason.
#[derive(Debug, Clone, PartialEq)]
pub struct NearMiss {
    pub partition: String,
    pub start_index: EventIndex,
    /// The deepest reachable partial path, including the start event.
    pub participating_indices: Vec<EventIndex>,
    /// Number of steps satisfied (== `participating_indices.len()`, always >= 1).
    pub reached_steps: usize,
    /// Index of the step that could not be satisfied.
    pub next_step_index: usize,
    /// Event type the failing step required.
    pub next_event_type: String,
    pub reason: NearMissReason,
    /// Bindings captured along the deepest partial path.
    pub bindings: Bindings,
}

struct Reach {
    path: Vec<EventIndex>,
    bindings: Bindings,
}

impl Reach {
    fn reached(&self) -> usize {
        self.path.len()
    }
}

/// Compute near-misses for `pattern` over `events`.
pub fn near_misses(events: &[Event], pattern: &Pattern) -> Vec<NearMiss> {
    validate_pattern(pattern);
    debug_assert!(
        crate::is_sorted_by_partition_time_index(events),
        "near_misses input must be grouped by partition and sorted by (timestamp, index)"
    );

    let mut near_misses = Vec::new();
    let mut partition_start = 0;
    while partition_start < events.len() {
        let partition = &events[partition_start].partition;
        let partition_end = events[partition_start..]
            .iter()
            .position(|event| event.partition != *partition)
            .map_or(events.len(), |offset| partition_start + offset);

        for start_index in partition_start..partition_end {
            let Some(bindings) = pattern.steps[0]
                .atom
                .evaluate(&events[start_index], &Bindings::new())
            else {
                continue;
            };

            let deepest = deepest_path(
                events,
                partition_end,
                pattern,
                1,
                vec![start_index],
                bindings,
            );

            // A full match exists from this start: reported by the matcher, not here.
            if deepest.reached() == pattern.steps.len() {
                continue;
            }

            let frontier = *deepest.path.last().expect("path is non-empty");
            let failed_step_index = deepest.reached();
            let failed_step = &pattern.steps[failed_step_index];
            let reason = classify(
                events,
                partition_end,
                frontier,
                failed_step,
                &deepest.bindings,
            );

            near_misses.push(NearMiss {
                partition: events[start_index].partition.clone(),
                start_index,
                participating_indices: deepest.path,
                reached_steps: failed_step_index,
                next_step_index: failed_step_index,
                next_event_type: failed_step.atom.event_type.clone(),
                reason,
                bindings: deepest.bindings,
            });
        }

        partition_start = partition_end;
    }

    near_misses
}

fn deepest_path(
    events: &[Event],
    partition_end: usize,
    pattern: &Pattern,
    step_index: usize,
    path: Vec<EventIndex>,
    bindings: Bindings,
) -> Reach {
    if step_index == pattern.steps.len() {
        return Reach { path, bindings };
    }

    let previous_index = *path.last().expect("path is non-empty");
    let step = &pattern.steps[step_index];
    let transition = step
        .transition_from_previous
        .as_ref()
        .expect("transition exists after first step");

    let mut best: Option<Reach> = None;
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
            let reach = deepest_path(
                events,
                partition_end,
                pattern,
                step_index + 1,
                next_path,
                next_bindings,
            );
            best = Some(match best {
                None => reach,
                Some(current) => choose_deeper(current, reach),
            });
        }
    }

    best.unwrap_or(Reach { path, bindings })
}

fn choose_deeper(left: Reach, right: Reach) -> Reach {
    // Prefer the deeper reach; break ties by lexicographically smaller path so
    // the result is deterministic and consistent with match ordering.
    match right.reached().cmp(&left.reached()) {
        std::cmp::Ordering::Greater => right,
        std::cmp::Ordering::Less => left,
        std::cmp::Ordering::Equal => {
            if right.path < left.path {
                right
            } else {
                left
            }
        }
    }
}

fn classify(
    events: &[Event],
    partition_end: usize,
    frontier: EventIndex,
    step: &Step,
    bindings: &Bindings,
) -> NearMissReason {
    let transition = step
        .transition_from_previous
        .as_ref()
        .expect("transition exists after first step");
    let atom = &step.atom;

    let mut reason = NearMissReason::NoSuccessor;
    for candidate_index in frontier + 1..partition_end {
        let candidate = &events[candidate_index];
        if candidate.event_type != atom.event_type {
            continue;
        }

        let within_window = match transition.max_elapsed {
            Some(max_elapsed) => candidate.timestamp - events[frontier].timestamp <= max_elapsed,
            None => true,
        };
        if !within_window {
            reason = reason.nearer(NearMissReason::WindowExceeded);
            continue;
        }

        let absence_blocked = events[frontier + 1..candidate_index].iter().any(|event| {
            transition
                .absence
                .iter()
                .any(|absent_atom| absent_atom.matches(event, bindings))
        });
        if absence_blocked {
            reason = reason.nearer(NearMissReason::AbsenceBlocked);
            continue;
        }

        // Right type, in window, absence clear: had the predicate/reference
        // passed, the path would have extended, so this is a predicate miss.
        reason = reason.nearer(NearMissReason::PredicateFailed);
    }

    reason
}
