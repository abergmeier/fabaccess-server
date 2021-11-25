use executor::prelude::*;
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn increment(b: &mut Criterion) {
    let mut sum = 0;
    let executor = Executor::new();

    b.bench_function("Executor::run", |b| b.iter(|| {
        executor.run(
            async {
                (0..10_000_000).for_each(|_| {
                    sum += 1;
                });
            },
        );
    }));

    black_box(sum);
}

criterion_group!(perf, increment);
criterion_main!(perf);