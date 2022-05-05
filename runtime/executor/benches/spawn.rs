use criterion::{black_box, criterion_group, criterion_main, Criterion};
use executor::load_balancer;
use executor::prelude::*;
use futures_timer::Delay;
use std::time::Duration;

#[cfg(feature = "tokio-runtime")]
mod benches {
    use super::*;
    pub fn spawn_lot(b: &mut Bencher) {
        tokio_test::block_on(async { _spawn_lot(b) });
    }
    pub fn spawn_single(b: &mut Bencher) {
        tokio_test::block_on(async {
            _spawn_single(b);
        });
    }
}

#[cfg(not(feature = "tokio-runtime"))]
mod benches {
    use super::*;

    pub fn spawn_lot(b: &mut Criterion) {
        _spawn_lot(b);
    }
    pub fn spawn_single(b: &mut Criterion) {
        _spawn_single(b);
    }
}

criterion_group!(spawn, benches::spawn_lot, benches::spawn_single);
criterion_main!(spawn);

// Benchmark for a 10K burst task spawn
fn _spawn_lot(b: &mut Criterion) {
    let executor = Executor::new();
    b.bench_function("spawn_lot", |b| {
        b.iter(|| {
            let _ = (0..10_000)
                .map(|_| {
                    executor.spawn(async {
                        let duration = Duration::from_millis(1);
                        Delay::new(duration).await;
                    })
                })
                .collect::<Vec<_>>();
        })
    });
}

// Benchmark for a single task spawn
fn _spawn_single(b: &mut Criterion) {
    let executor = Executor::new();
    b.bench_function("spawn single", |b| {
        b.iter(|| {
            executor.spawn(async {
                let duration = Duration::from_millis(1);
                Delay::new(duration).await;
            });
        })
    });
}
