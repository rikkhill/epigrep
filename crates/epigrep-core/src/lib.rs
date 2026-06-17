mod explain;
mod json;
mod matcher;
mod model;
mod parser;

pub use explain::{NearMiss, NearMissDetail, NearMissReason, PredicateFailure, near_misses};
pub use json::{pattern_from_json, pattern_to_json, pattern_to_json_pretty};
pub use matcher::{
    CompiledPattern, compiled_matches, is_sorted_by_partition_time_index, oracle_matches,
};
pub use model::{
    Atom, Bindings, Capture, ComparisonOperator, Event, EventIndex, Match, MatchConsumption,
    Pattern, Predicate, ReferencePredicate, Step, Timestamp, Transition, Value,
};
pub use parser::{ParseError, parse_pattern};

#[cfg(test)]
mod tests;
