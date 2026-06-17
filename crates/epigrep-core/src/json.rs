//! JSON serialisation of the pattern AST.
//!
//! This is the stable, structured surface intended for tools and agents: rather
//! than emitting raw DSL text, a caller can construct or validate a pattern as
//! JSON that round-trips exactly through the AST. The text parser remains a
//! convenience; this is the contract for programmatic use.

use crate::model::*;

/// Serialise a pattern to compact JSON. Infallible for the AST's own types.
pub fn pattern_to_json(pattern: &Pattern) -> String {
    serde_json::to_string(pattern).expect("the pattern AST always serialises to JSON")
}

/// Serialise a pattern to pretty-printed JSON.
pub fn pattern_to_json_pretty(pattern: &Pattern) -> String {
    serde_json::to_string_pretty(pattern).expect("the pattern AST always serialises to JSON")
}

/// Parse a pattern from JSON, validating its structure.
///
/// Returns an error message on malformed JSON or a structurally invalid pattern
/// (empty, a first step carrying a transition, or a later step missing one), so
/// the result is always safe to pass to the matcher.
pub fn pattern_from_json(json: &str) -> Result<Pattern, String> {
    let pattern: Pattern = serde_json::from_str(json).map_err(|error| error.to_string())?;
    validate_structure(&pattern)?;
    Ok(pattern)
}

fn validate_structure(pattern: &Pattern) -> Result<(), String> {
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
