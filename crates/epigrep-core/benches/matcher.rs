use criterion::{Criterion, criterion_group, criterion_main};
use epigrep_core::{CompiledPattern, Event, compiled_matches, oracle_matches, parse_pattern};

fn synthetic_events() -> Vec<Event> {
    (0..10_000)
        .map(|index| {
            let event_type = match index % 7 {
                0 => "A",
                3 => "B",
                5 => "C",
                _ => "noise",
            };
            let user = match index % 3 {
                0 => "u1",
                1 => "u2",
                _ => "u3",
            };

            Event::new("partition-1", index, event_type)
                .with_attr("score", (index % 5).into())
                .with_attr("user_id", user.into())
        })
        .collect()
}

fn matcher_benchmarks(criterion: &mut Criterion) {
    let events = synthetic_events();
    let pattern = parse_pattern(
        "A[user_id as $u, score >= 2] -[<=10, no C[user_id == $u]]-> B[user_id == $u]",
    )
    .expect("benchmark pattern should parse");
    let compiled = CompiledPattern::compile(&pattern);

    criterion.bench_function("oracle matcher", |bencher| {
        bencher.iter(|| oracle_matches(&events, &pattern))
    });
    criterion.bench_function("compiled matcher", |bencher| {
        bencher.iter(|| compiled.matches(&events))
    });
    criterion.bench_function("compile and match", |bencher| {
        bencher.iter(|| compiled_matches(&events, &pattern))
    });
}

criterion_group!(benches, matcher_benchmarks);
criterion_main!(benches);
