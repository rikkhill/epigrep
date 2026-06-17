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
