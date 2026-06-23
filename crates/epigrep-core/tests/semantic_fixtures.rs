//! Cross-language semantic golden fixtures, run against the core matcher.
//!
//! The corpus lives at the repository root (`tests/semantic-fixtures/`) and is
//! shared with the Python harness (`crates/epigrep-py/tests/test_semantic_fixtures.py`).
//! Each fixture pins one clause of the semantics contract with a tiny example and
//! hand-specified expected matches and near-misses. Here we check three things per
//! fixture: the compiled matcher matches the expectation, the naive oracle agrees
//! with the compiled matcher, and the near-misses match the expectation.
//!
//! Regenerate the corpus with `python tests/semantic-fixtures/_generate.py`.

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use epigrep_core::{
    Event, Match, NearMiss, NearMissReason, Value, compiled_matches, near_misses, oracle_matches,
    pattern_from_json,
};
use serde::Deserialize;
use serde_json::Value as Json;

#[derive(Deserialize)]
struct Fixture {
    name: String,
    events: Vec<FixtureEvent>,
    pattern_json: Json,
    expected_matches: Vec<ExpectedMatch>,
    expected_near_misses: Vec<ExpectedNearMiss>,
}

#[derive(Deserialize)]
struct FixtureEvent {
    partition: String,
    ts: i64,
    typ: String,
    #[serde(default)]
    attrs: BTreeMap<String, Json>,
}

#[derive(Deserialize)]
struct ExpectedMatch {
    partition: String,
    indices: Vec<usize>,
    #[serde(default)]
    captures: BTreeMap<String, Json>,
}

#[derive(Deserialize)]
struct ExpectedNearMiss {
    partition: String,
    indices: Vec<usize>,
    reason: String,
}

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/semantic-fixtures")
}

fn load_fixtures() -> Vec<Fixture> {
    let mut paths: Vec<PathBuf> = fs::read_dir(fixtures_dir())
        .expect("semantic-fixtures directory must exist")
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.extension().is_some_and(|ext| ext == "json")
                && path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with(|c: char| c.is_ascii_digit()))
        })
        .collect();
    paths.sort();
    paths
        .into_iter()
        .map(|path| {
            let text = fs::read_to_string(&path).unwrap();
            serde_json::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
        })
        .collect()
}

fn json_scalar_to_value(json: &Json) -> Value {
    match json {
        Json::String(s) => Value::String(s.clone()),
        Json::Bool(b) => Value::Bool(*b),
        Json::Null => Value::Null,
        Json::Number(n) => {
            if let Some(i) = n.as_i64() {
                Value::Int(i)
            } else {
                Value::Float(n.as_f64().unwrap())
            }
        }
        other => panic!("unsupported attribute/capture value: {other}"),
    }
}

fn value_to_json(value: &Value) -> Json {
    match value {
        Value::String(s) => Json::String(s.clone()),
        Value::Int(i) => Json::Number((*i).into()),
        Value::Float(f) => serde_json::json!(f),
        Value::Bool(b) => Json::Bool(*b),
        Value::Null => Json::Null,
    }
}

fn to_event(fixture_event: &FixtureEvent) -> Event {
    let mut event = Event::new(
        fixture_event.partition.clone(),
        fixture_event.ts,
        fixture_event.typ.clone(),
    );
    for (key, raw) in &fixture_event.attrs {
        event = event.with_attr(key.clone(), json_scalar_to_value(raw));
    }
    event
}

/// A match rendered into a comparable, sortable shape: captures become
/// `(name, canonical-json-string)` pairs so the whole row is `Ord`/`Eq`.
type MatchRow = (String, Vec<usize>, Vec<(String, String)>);

fn match_rows(matches: &[Match]) -> Vec<MatchRow> {
    let mut rows: Vec<MatchRow> = matches
        .iter()
        .map(|m| {
            let captures = m
                .bindings
                .iter()
                .map(|(name, value)| (name.clone(), value_to_json(value).to_string()))
                .collect();
            (
                m.partition.clone(),
                m.participating_indices.clone(),
                captures,
            )
        })
        .collect();
    rows.sort();
    rows
}

fn expected_match_rows(expected: &[ExpectedMatch]) -> Vec<MatchRow> {
    let mut rows: Vec<MatchRow> = expected
        .iter()
        .map(|m| {
            let captures = m
                .captures
                .iter()
                .map(|(name, value)| (name.clone(), value.to_string()))
                .collect();
            (m.partition.clone(), m.indices.clone(), captures)
        })
        .collect();
    rows.sort();
    rows
}

fn reason_str(reason: NearMissReason) -> &'static str {
    match reason {
        NearMissReason::PredicateFailed => "predicate_failed",
        NearMissReason::AbsenceBlocked => "absence_blocked",
        NearMissReason::WindowExceeded => "window_exceeded",
        NearMissReason::NoSuccessor => "no_successor",
    }
}

type NearMissRow = (String, Vec<usize>, String);

fn near_miss_rows(misses: &[NearMiss]) -> Vec<NearMissRow> {
    let mut rows: Vec<NearMissRow> = misses
        .iter()
        .map(|m| {
            (
                m.partition.clone(),
                m.participating_indices.clone(),
                reason_str(m.reason()).to_string(),
            )
        })
        .collect();
    rows.sort();
    rows
}

fn expected_near_miss_rows(expected: &[ExpectedNearMiss]) -> Vec<NearMissRow> {
    let mut rows: Vec<NearMissRow> = expected
        .iter()
        .map(|m| (m.partition.clone(), m.indices.clone(), m.reason.clone()))
        .collect();
    rows.sort();
    rows
}

#[test]
fn corpus_is_present() {
    assert!(
        load_fixtures().len() >= 15,
        "expected the semantic-fixture corpus to be present"
    );
}

#[test]
fn fixtures_match_expectations_and_oracle_agrees() {
    for fixture in load_fixtures() {
        let events: Vec<Event> = fixture.events.iter().map(to_event).collect();
        let pattern = pattern_from_json(&fixture.pattern_json.to_string())
            .unwrap_or_else(|e| panic!("{}: invalid pattern: {e}", fixture.name));

        let compiled = compiled_matches(&events, &pattern);
        let oracle = oracle_matches(&events, &pattern);

        // Compiled backend agrees with the naive oracle.
        assert_eq!(
            match_rows(&compiled),
            match_rows(&oracle),
            "{}: compiled/oracle parity",
            fixture.name
        );

        // Compiled backend matches the hand-specified expectation.
        assert_eq!(
            match_rows(&compiled),
            expected_match_rows(&fixture.expected_matches),
            "{}: matches",
            fixture.name
        );

        // Near-misses match the hand-specified expectation.
        assert_eq!(
            near_miss_rows(&near_misses(&events, &pattern)),
            expected_near_miss_rows(&fixture.expected_near_misses),
            "{}: near-misses",
            fixture.name
        );
    }
}
