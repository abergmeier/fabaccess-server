use criterion::{black_box, criterion_group, criterion_main, Criterion};
use executor::load_balancer::{core_count, get_cores, stats, SmpStats};
use executor::placement;
use std::thread;

fn stress_stats<S: SmpStats + Sync + Send>(stats: &'static S) {
    let mut handles = Vec::with_capacity(*core_count());
    for core in get_cores() {
        let handle = thread::spawn(move || {
            placement::set_for_current(*core);
            for i in 0..100 {
                stats.store_load(core.id, 10);
                if i % 3 == 0 {
                    let _sorted_load = stats.get_sorted_load();
                }
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }
}

// previous lock based stats benchmark 1,352,791 ns/iter (+/- 2,682,013)

// 158,278 ns/iter (+/- 117,103)
fn lockless_stats_bench(b: &mut Criterion) {
    b.bench_function("stress_stats", |b| {
        b.iter(|| {
            stress_stats(stats());
        })
    });
}

fn lockless_stats_bad_load(b: &mut Criterion) {
    let stats = stats();
    const MAX_CORE: usize = 256;
    for i in 0..MAX_CORE {
        // Generating the worst possible mergesort scenario
        // [0,2,4,6,8,10,1,3,5,7,9]...
        if i <= MAX_CORE / 2 {
            stats.store_load(i, i * 2);
        } else {
            stats.store_load(i, i - 1 - MAX_CORE / 2);
        }
    }

    b.bench_function("get_sorted_load", |b| {
        b.iter(|| {
            let _sorted_load = stats.get_sorted_load();
        })
    });
}

fn lockless_stats_good_load(b: &mut Criterion) {
    let stats = stats();
    const MAX_CORE: usize = 256;
    for i in 0..MAX_CORE {
        // Generating the best possible mergesort scenario
        // [0,1,2,3,4,5,6,7,8,9]...
        stats.store_load(i, i);
    }

    b.bench_function("get_sorted_load", |b| {
        b.iter(|| {
            let _sorted_load = stats.get_sorted_load();
        })
    });
}

criterion_group!(
    stats_bench,
    lockless_stats_bench,
    lockless_stats_bad_load,
    lockless_stats_good_load
);
criterion_main!(stats_bench);
