use crate::*;

fn atom(event_type: &str) -> Atom {
    Atom::event_type(event_type)
}

fn sequence(first: &str, second: &str, transition: Transition) -> Pattern {
    Pattern::sequence(vec![
        Step::first(atom(first)),
        Step::then(atom(second), transition),
    ])
}

fn single(predicate: Predicate) -> Pattern {
    Pattern::sequence(vec![Step::first(atom("A").with_predicate(predicate))])
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

#[test]
fn parses_sequence_window_predicate_and_absence_subset() {
    let pattern = parse_pattern(r#"A[score >= 2] -[<=5, no C[kind == "blocked"]]-> B"#)
        .expect("pattern should parse");
    let events = vec![
        Event::new("p", 0, "A").with_attr("score", 3_i64.into()),
        Event::new("p", 2, "C").with_attr("kind", "allowed".into()),
        Event::new("p", 5, "B"),
        Event::new("p", 6, "A").with_attr("score", 3_i64.into()),
        Event::new("p", 7, "C").with_attr("kind", "blocked".into()),
        Event::new("p", 8, "B"),
    ];

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
fn parses_capture_and_reference_predicates() {
    let pattern =
        parse_pattern("A[user_id as $u] -> B[user_id == $u]").expect("pattern should parse");
    let events = vec![
        Event::new("p", 0, "A").with_attr("user_id", "u1".into()),
        Event::new("p", 1, "B").with_attr("user_id", "u2".into()),
        Event::new("p", 2, "B").with_attr("user_id", "u1".into()),
    ];

    let matches = oracle_matches(&events, &pattern);

    assert_eq!(indices(&matches), vec![vec![0, 2]]);
    assert_eq!(matches[0].bindings.get("u"), Some(&Value::from("u1")));
}

#[test]
fn parser_rejects_unsupported_predicate_syntax() {
    let error = parse_pattern("A[score ~= 2] -> B").expect_err("pattern should fail");

    assert!(
        error.message().contains("unsupported atom predicate"),
        "{error}"
    );
}

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
        prop::collection::vec(event_strategy(), 0..=14).prop_map(|mut events| {
            // Stable sort so input order remains the (timestamp, index) tie-break.
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
            prop::option::of(atom_strategy()),
            transition_strategy(),
            transition_strategy(),
            transition_strategy(),
            prop_oneof![
                Just(MatchConsumption::FirstSuccessorPerStart),
                Just(MatchConsumption::ExhaustivePerStart),
            ],
        )
            .prop_map(|(first, second, third, fourth, t1, t2, t3, consumption)| {
                // 2 to 4 steps; a fourth step only exists if a third does.
                let mut steps = vec![Step::first(first), Step::then(second, t1)];
                if let Some(third) = third {
                    steps.push(Step::then(third, t2));
                    if let Some(fourth) = fourth {
                        steps.push(Step::then(fourth, t3));
                    }
                }
                Pattern::sequence(steps).with_consumption(consumption)
            })
    }

    // A stream and pattern deliberately shaped to produce early-success /
    // late-dead-end paths: an early successor satisfies its transition but the
    // remaining steps fail (tight final window, absence guard, or no final
    // event), while a later successor might complete. This is the class the
    // generic generators did not discover on their own, which hid the
    // FirstSuccessorPerStart consumption bug.
    fn dead_end_stream() -> impl Strategy<Value = Vec<Event>> {
        prop::collection::vec(
            (prop_oneof![Just("A"), Just("B"), Just("C")], 0_i64..=8),
            3..=14,
        )
        .prop_map(|specs| {
            let mut events: Vec<Event> = specs
                .into_iter()
                .map(|(event_type, timestamp)| Event::new("p", timestamp, event_type))
                .collect();
            // Stable sort: input order is the (timestamp, index) tie-break.
            events.sort_by_key(|event| event.timestamp);
            events
        })
    }

    fn dead_end_pattern() -> impl Strategy<Value = Pattern> {
        (
            0_i64..=3,                   // tight final window forces dead-ends
            prop::option::of(0_i64..=4), // optional middle window
            any::<bool>(),               // optional absence guard on the final step
            prop_oneof![
                Just(MatchConsumption::FirstSuccessorPerStart),
                Just(MatchConsumption::ExhaustivePerStart),
            ],
        )
            .prop_map(
                |(final_window, middle_window, guard_absence, consumption)| {
                    let middle = match middle_window {
                        Some(window) => Transition::any().within(window),
                        None => Transition::any(),
                    };
                    let mut final_transition = Transition::any().within(final_window);
                    if guard_absence {
                        final_transition = final_transition.with_absence(atom("A"));
                    }
                    Pattern::sequence(vec![
                        Step::first(atom("A")),
                        Step::then(atom("B"), middle),
                        Step::then(atom("C"), final_transition),
                    ])
                    .with_consumption(consumption)
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

        #[test]
        fn compiled_matches_oracle_on_multi_step_dead_ends(
            events in dead_end_stream(),
            pattern in dead_end_pattern(),
        ) {
            prop_assert_eq!(
                compiled_matches(&events, &pattern),
                oracle_matches(&events, &pattern)
            );
        }
    }

    // ---- Metamorphic properties (pass 2 of the test-hardening strategy) ----
    //
    // Each transformation below changes the input in a way whose effect on the
    // output is fixed by the semantics contract. Matches are normalised by a
    // stable per-event id so that index renumbering (after insertion or
    // duplication) cannot masquerade as a behavioural change. Properties that are
    // false under FirstSuccessorPerStart's commitment (e.g. window monotonicity)
    // are tested under ExhaustivePerStart, as the strategy notes.

    /// Tag each event with a stable `__eid` the patterns never reference, so a
    /// match can be identified by *which* events participate regardless of their
    /// position after a transformation.
    fn with_eids(events: Vec<Event>) -> Vec<Event> {
        events
            .into_iter()
            .enumerate()
            .map(|(index, event)| event.with_attr("__eid", Value::Int(index as i64)))
            .collect()
    }

    fn eid(events: &[Event], index: EventIndex) -> i64 {
        match events[index].attributes.get("__eid") {
            Some(Value::Int(value)) => *value,
            _ => panic!("event is missing its __eid tag"),
        }
    }

    /// (partition, participating event ids, canonical bindings): stable across
    /// index renumbering, and `Ord`, so it can go in a set for subset checks.
    fn signature(events: &[Event], m: &Match) -> (String, Vec<i64>, String) {
        (
            m.partition.clone(),
            m.participating_indices
                .iter()
                .map(|&index| eid(events, index))
                .collect(),
            serde_json::to_string(&m.bindings).unwrap(),
        )
    }

    fn signatures(events: &[Event], pattern: &Pattern) -> Vec<(String, Vec<i64>, String)> {
        let mut rows: Vec<_> = compiled_matches(events, pattern)
            .iter()
            .map(|m| signature(events, m))
            .collect();
        rows.sort();
        rows
    }

    fn map_events(events: &[Event], f: impl FnMut(&Event) -> Event) -> Vec<Event> {
        events.iter().map(f).collect()
    }

    fn relocate(event: &Event, partition: String, timestamp: Timestamp) -> Event {
        let mut moved = Event::new(partition, timestamp, event.event_type.clone());
        for (key, value) in &event.attributes {
            moved = moved.with_attr(key.clone(), value.clone());
        }
        moved
    }

    /// Widen every transition window to unbounded, keeping absence guards.
    fn widen_windows(pattern: &Pattern) -> Pattern {
        let steps: Vec<Step> = pattern
            .steps
            .iter()
            .map(|step| Step {
                atom: step.atom.clone(),
                transition_from_previous: step.transition_from_previous.as_ref().map(|t| {
                    Transition {
                        max_elapsed: None,
                        absence: t.absence.clone(),
                    }
                }),
            })
            .collect();
        Pattern::sequence(steps).with_consumption(pattern.consumption)
    }

    proptest! {
        /// JSON round-trip preserves the pattern and its match/explain behaviour.
        #[test]
        fn json_round_trip_preserves_behaviour(
            events in stream_strategy(),
            pattern in pattern_strategy(),
        ) {
            let parsed = pattern_from_json(&pattern_to_json(&pattern))
                .expect("a valid generated pattern round-trips");
            prop_assert_eq!(&parsed, &pattern);
            prop_assert_eq!(
                compiled_matches(&events, &parsed),
                compiled_matches(&events, &pattern)
            );
            prop_assert_eq!(
                near_misses(&events, &parsed),
                near_misses(&events, &pattern)
            );
        }

        /// Shifting every timestamp by a constant preserves the participating
        /// events and bindings, and shifts each match's span by the same amount.
        #[test]
        fn timestamp_translation_shifts_spans_and_preserves_structure(
            events in stream_strategy(),
            pattern in pattern_strategy(),
            delta in 1_i64..=1000,
        ) {
            let shifted = map_events(&events, |event| {
                relocate(event, event.partition.clone(), event.timestamp + delta)
            });
            let original = compiled_matches(&events, &pattern);
            let translated = compiled_matches(&shifted, &pattern);

            prop_assert_eq!(original.len(), translated.len());
            for (before, after) in original.iter().zip(translated.iter()) {
                prop_assert_eq!(&before.partition, &after.partition);
                prop_assert_eq!(&before.participating_indices, &after.participating_indices);
                prop_assert_eq!(&before.bindings, &after.bindings);
                prop_assert_eq!(before.start_timestamp + delta, after.start_timestamp);
                prop_assert_eq!(before.end_timestamp + delta, after.end_timestamp);
            }
        }

        /// Renaming partitions order-preservingly preserves match structure,
        /// changing only the partition label.
        #[test]
        fn partition_rename_preserves_structure(
            events in stream_strategy(),
            pattern in pattern_strategy(),
        ) {
            let renamed = map_events(&events, |event| {
                relocate(event, format!("x-{}", event.partition), event.timestamp)
            });
            let original = compiled_matches(&events, &pattern);
            let after = compiled_matches(&renamed, &pattern);

            prop_assert_eq!(original.len(), after.len());
            for (before, now) in original.iter().zip(after.iter()) {
                prop_assert_eq!(format!("x-{}", before.partition), now.partition.clone());
                prop_assert_eq!(&before.participating_indices, &now.participating_indices);
                prop_assert_eq!(&before.bindings, &now.bindings);
                prop_assert_eq!(before.start_timestamp, now.start_timestamp);
            }
        }

        /// Duplicating a partition under a new name duplicates its matches
        /// independently and leaves the other partitions untouched — no
        /// cross-partition contamination.
        #[test]
        fn partition_duplication_is_independent(
            events in stream_strategy(),
            pattern in pattern_strategy(),
        ) {
            let tagged = with_eids(events);
            // "p3" sorts after the generator's "p1"/"p2", so appending the copied
            // block keeps the stream grouped-by-partition and time-sorted.
            let mut combined = tagged.clone();
            for event in tagged.iter().filter(|event| event.partition == "p1") {
                combined.push(relocate(event, "p3".to_owned(), event.timestamp));
            }

            let strip = |rows: Vec<(String, Vec<i64>, String)>, keep_p3: bool| {
                let mut out: Vec<(Vec<i64>, String)> = rows
                    .into_iter()
                    .filter(|(partition, _, _)| (partition == "p3") == keep_p3)
                    .map(|(_, ids, bindings)| (ids, bindings))
                    .collect();
                out.sort();
                out
            };

            let original = signatures(&tagged, &pattern);
            let after = signatures(&combined, &pattern);

            // The new "p3" matches mirror "p1"'s, by participating ids and bindings.
            let original_p1: Vec<_> = original
                .iter()
                .filter(|row| row.0 == "p1")
                .cloned()
                .collect();
            prop_assert_eq!(strip(after.clone(), true), strip(original_p1, false));

            // Every non-p3 match is exactly as it was before duplication.
            let after_non_p3: Vec<_> = after
                .iter()
                .filter(|row| row.0 != "p3")
                .cloned()
                .collect();
            prop_assert_eq!(after_non_p3, original);
        }

        /// Inserting events of a type that appears nowhere in the pattern (steps
        /// or absence atoms) cannot change any match's identity. Normalised by
        /// event id so the index renumbering from insertion is ignored.
        #[test]
        fn irrelevant_event_insertion_is_inert(
            events in stream_strategy(),
            pattern in pattern_strategy(),
            noise in prop::collection::vec(
                (prop_oneof![Just("p1"), Just("p2")], 0_i64..=8),
                0..=6,
            ),
        ) {
            // "Z" is never produced by the atom/event generators, so it can match
            // no step and block no absence guard.
            let tagged = with_eids(events);
            let mut combined = tagged.clone();
            for (offset, (partition, timestamp)) in noise.into_iter().enumerate() {
                let marker = Event::new(partition, timestamp, "Z")
                    .with_attr("__eid", Value::Int(1_000_000 + offset as i64));
                combined.push(marker);
            }
            // Re-establish grouped-by-partition, time-sorted order (stable, so the
            // original events keep their relative input-order tie-break).
            combined.sort_by(|left, right| {
                left.partition
                    .cmp(&right.partition)
                    .then(left.timestamp.cmp(&right.timestamp))
            });

            prop_assert_eq!(signatures(&combined, &pattern), signatures(&tagged, &pattern));
        }

        /// Under ExhaustivePerStart, widening transition windows can only add
        /// matches: every match at the narrow windows still holds when widened.
        #[test]
        fn exhaustive_window_widening_is_monotonic(
            events in stream_strategy(),
            pattern in pattern_strategy(),
        ) {
            let tagged = with_eids(events);
            let narrow = pattern.with_consumption(MatchConsumption::ExhaustivePerStart);
            let wide = widen_windows(&narrow);

            let narrow_set: std::collections::BTreeSet<_> =
                signatures(&tagged, &narrow).into_iter().collect();
            let wide_set: std::collections::BTreeSet<_> =
                signatures(&tagged, &wide).into_iter().collect();

            prop_assert!(
                narrow_set.is_subset(&wide_set),
                "widening windows dropped matches under exhaustive consumption"
            );
        }
    }
}

#[test]
fn first_successor_commits_per_step_not_on_completion() {
    // A -> B -> C with a <=1 window on B->C. The first B (idx 1) satisfies
    // A->B but no C is reachable from it; a later B (idx 3) would complete.
    let events = vec![
        Event::new("p", 0, "A"),
        Event::new("p", 0, "B"),
        Event::new("p", 5, "C"),
        Event::new("p", 5, "B"),
        Event::new("p", 5, "C"),
    ];
    let pattern = Pattern::sequence(vec![
        Step::first(atom("A")),
        Step::then(atom("B"), Transition::any()),
        Step::then(atom("C"), Transition::any().within(1)),
    ]);

    // First-successor commits to B@1 and then dead-ends, so there is no match.
    // It must NOT backtrack to B@3 to manufacture a completion.
    assert_eq!(
        indices(&oracle_matches(&events, &pattern)),
        Vec::<Vec<usize>>::new()
    );
    assert_eq!(
        oracle_matches(&events, &pattern),
        compiled_matches(&events, &pattern)
    );
}

#[test]
fn numeric_equality_and_ordering_agree_across_int_and_float() {
    let events = vec![Event::new("p", 0, "A").with_attr("v", Value::Int(1))];

    // `==`, `>=`, and `<=` must all treat Int(1) as equal to the float literal
    // 1.0 — previously `==` used strict variant equality and disagreed.
    for operator in [
        ComparisonOperator::Eq,
        ComparisonOperator::Gte,
        ComparisonOperator::Lte,
    ] {
        let pattern = single(Predicate::new("v", operator, 1.0_f64));
        assert_eq!(
            indices(&oracle_matches(&events, &pattern)),
            vec![vec![0]],
            "Int(1) should satisfy {operator:?} 1.0"
        );
        assert_eq!(
            oracle_matches(&events, &pattern),
            compiled_matches(&events, &pattern)
        );
    }

    // Cross-type comparison stays strict for non-numbers: an int is never equal
    // to a string literal.
    let pattern = single(Predicate::new("v", ComparisonOperator::Eq, "1"));
    assert!(oracle_matches(&events, &pattern).is_empty());
}

fn only_near_miss(events: &[Event], pattern: &Pattern) -> NearMiss {
    let mut misses = near_misses(events, pattern);
    assert_eq!(misses.len(), 1, "expected exactly one near-miss");
    misses.pop().unwrap()
}

#[test]
fn near_miss_window_exceeded() {
    let events = vec![Event::new("p", 0, "A"), Event::new("p", 10, "B")];
    let pattern = sequence("A", "B", Transition::any().within(5));

    let miss = only_near_miss(&events, &pattern);
    assert_eq!(miss.participating_indices, vec![0]);
    assert_eq!(miss.reached_steps, 1);
    assert_eq!(miss.next_event_type, "B");
    assert_eq!(miss.reason(), NearMissReason::WindowExceeded);
    // Counterfactual: B at index 1 is 10 apart; would match with window >= 10.
    assert_eq!(
        miss.detail,
        NearMissDetail::WindowExceeded {
            candidate_index: 1,
            gap: 10,
            max_elapsed: 5,
        }
    );
}

#[test]
fn near_miss_absence_blocked() {
    let events = vec![
        Event::new("p", 0, "A"),
        Event::new("p", 1, "C"),
        Event::new("p", 2, "B"),
    ];
    let pattern = sequence("A", "B", Transition::any().with_absence(atom("C")));

    let miss = only_near_miss(&events, &pattern);
    assert_eq!(miss.reason(), NearMissReason::AbsenceBlocked);
    // The C at index 1 blocked the B at index 2.
    assert_eq!(
        miss.detail,
        NearMissDetail::AbsenceBlocked {
            candidate_index: 2,
            blocking_index: 1,
            blocking_event_type: "C".to_owned(),
            candidate_satisfies: true,
        }
    );
}

#[test]
fn near_miss_absence_blocked_candidate_also_failing_predicate() {
    // The only B in window is blocked by C and also fails its predicate, so the
    // counterfactual "remove C" would not, on its own, produce a match.
    let events = vec![
        Event::new("p", 0, "A"),
        Event::new("p", 1, "C"),
        Event::new("p", 2, "B").with_attr("score", 1_i64.into()),
    ];
    let pattern = Pattern::sequence(vec![
        Step::first(atom("A")),
        Step::then(
            atom("B").with_predicate(Predicate::new("score", ComparisonOperator::Gte, 3_i64)),
            Transition::any().with_absence(atom("C")),
        ),
    ]);

    let miss = only_near_miss(&events, &pattern);
    assert_eq!(
        miss.detail,
        NearMissDetail::AbsenceBlocked {
            candidate_index: 2,
            blocking_index: 1,
            blocking_event_type: "C".to_owned(),
            candidate_satisfies: false,
        }
    );
}

#[test]
fn near_miss_predicate_failed() {
    let events = vec![
        Event::new("p", 0, "A"),
        Event::new("p", 1, "B").with_attr("score", 1_i64.into()),
    ];
    let pattern = Pattern::sequence(vec![
        Step::first(atom("A")),
        Step::then(
            atom("B").with_predicate(Predicate::new("score", ComparisonOperator::Gte, 3_i64)),
            Transition::any(),
        ),
    ]);

    let miss = only_near_miss(&events, &pattern);
    assert_eq!(miss.reason(), NearMissReason::PredicateFailed);
    // Names the offending clause and the actual value on the candidate.
    assert_eq!(
        miss.detail,
        NearMissDetail::PredicateFailed {
            event_index: 1,
            failures: vec![PredicateFailure::Predicate {
                attribute: "score".to_owned(),
                operator: ComparisonOperator::Gte,
                expected: Value::Int(3),
                actual: Some(Value::Int(1)),
            }],
        }
    );
}

#[test]
fn near_miss_reference_failure_reports_binding_and_actual() {
    // A captures user_id=u1; B's reference user_id == $u fails with u2.
    let events = vec![
        Event::new("p", 0, "A").with_attr("user_id", "u1".into()),
        Event::new("p", 1, "B").with_attr("user_id", "u2".into()),
    ];
    let pattern = Pattern::sequence(vec![
        Step::first(atom("A").with_capture(Capture::new("u", "user_id"))),
        Step::then(
            atom("B").with_reference_predicate(ReferencePredicate::new(
                "user_id",
                ComparisonOperator::Eq,
                "u",
            )),
            Transition::any(),
        ),
    ]);

    let miss = only_near_miss(&events, &pattern);
    assert_eq!(
        miss.detail,
        NearMissDetail::PredicateFailed {
            event_index: 1,
            failures: vec![PredicateFailure::Reference {
                attribute: "user_id".to_owned(),
                operator: ComparisonOperator::Eq,
                binding: "u".to_owned(),
                bound: Some(Value::String("u1".to_owned())),
                actual: Some(Value::String("u2".to_owned())),
            }],
        }
    );
}

#[test]
fn near_miss_no_successor() {
    let events = vec![Event::new("p", 0, "A"), Event::new("p", 1, "X")];
    let pattern = sequence("A", "B", Transition::any());

    let miss = only_near_miss(&events, &pattern);
    assert_eq!(miss.reason(), NearMissReason::NoSuccessor);
}

#[test]
fn near_miss_reports_deepest_partial_path() {
    // Reaches A->B but no C is ever available: deepest path is [0, 1].
    let events = vec![
        Event::new("p", 0, "A"),
        Event::new("p", 1, "B"),
        Event::new("p", 2, "X"),
    ];
    let pattern = Pattern::sequence(vec![
        Step::first(atom("A")),
        Step::then(atom("B"), Transition::any()),
        Step::then(atom("C"), Transition::any()),
    ]);

    let miss = only_near_miss(&events, &pattern);
    assert_eq!(miss.participating_indices, vec![0, 1]);
    assert_eq!(miss.reached_steps, 2);
    assert_eq!(miss.next_event_type, "C");
    assert_eq!(miss.reason(), NearMissReason::NoSuccessor);
}

#[test]
fn near_miss_excludes_starts_that_fully_match() {
    let events = vec![Event::new("p", 0, "A"), Event::new("p", 1, "B")];
    let pattern = sequence("A", "B", Transition::any());

    assert!(near_misses(&events, &pattern).is_empty());
}

#[test]
fn near_miss_prefers_the_nearest_reason() {
    // For start A@0: a same-type B exists in window but fails the predicate,
    // and another B exists out of window. Predicate-failure is the nearer miss.
    let events = vec![
        Event::new("p", 0, "A"),
        Event::new("p", 1, "B").with_attr("score", 1_i64.into()),
        Event::new("p", 50, "B").with_attr("score", 9_i64.into()),
    ];
    let pattern = Pattern::sequence(vec![
        Step::first(atom("A")),
        Step::then(
            atom("B").with_predicate(Predicate::new("score", ComparisonOperator::Gte, 3_i64)),
            Transition::any().within(5),
        ),
    ]);

    let miss = only_near_miss(&events, &pattern);
    assert_eq!(miss.reason(), NearMissReason::PredicateFailed);
}

#[test]
fn pattern_json_round_trips_through_the_ast() {
    let pattern = Pattern::sequence(vec![
        Step::first(atom("A").with_capture(Capture::new("u", "user_id"))),
        Step::then(
            atom("B")
                .with_predicate(Predicate::new("score", ComparisonOperator::Gte, 3_i64))
                .with_reference_predicate(ReferencePredicate::new(
                    "user_id",
                    ComparisonOperator::Eq,
                    "u",
                )),
            Transition::any().within(5).with_absence(atom("C")),
        ),
    ])
    .with_consumption(MatchConsumption::ExhaustivePerStart);

    let json = pattern_to_json(&pattern);
    let parsed = pattern_from_json(&json).expect("round-trips");
    assert_eq!(parsed, pattern);
}

#[test]
fn pattern_from_json_rejects_malformed_json() {
    assert!(pattern_from_json("{ not json").is_err());
}

#[test]
fn pattern_from_json_rejects_structurally_invalid_patterns() {
    // First step must not carry a transition.
    let bad = Pattern::sequence(vec![Step::then(atom("A"), Transition::any())]);
    let json = pattern_to_json(&bad);
    assert!(pattern_from_json(&json).is_err());
}

#[test]
fn validate_pattern_accepts_well_formed_patterns() {
    assert!(validate_pattern(&Pattern::sequence(vec![Step::first(atom("A"))])).is_ok());
    assert!(validate_pattern(&sequence("A", "B", Transition::any())).is_ok());
}

#[test]
fn validate_pattern_rejects_an_empty_pattern() {
    assert!(validate_pattern(&Pattern::sequence(vec![])).is_err());
}

#[test]
fn validate_pattern_rejects_a_first_step_with_a_transition() {
    let bad = Pattern::sequence(vec![Step::then(atom("A"), Transition::any())]);
    assert!(validate_pattern(&bad).is_err());
}

#[test]
fn validate_pattern_rejects_a_later_step_without_a_transition() {
    let bad = Pattern::sequence(vec![Step::first(atom("A")), Step::first(atom("B"))]);
    assert!(validate_pattern(&bad).is_err());
}
