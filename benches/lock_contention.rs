//! Lock Contention Benchmarks
//!
//! Measures RwLock contention on Engine under concurrent access.

#![allow(clippy::unwrap_used)]

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use maintenance_engine::engine::Engine;
use std::sync::Arc;

fn bench_concurrent_health_reports(c: &mut Criterion) {
    let engine = Arc::new(Engine::new());
    let mut group = c.benchmark_group("concurrent_health_report");

    for threads in [1, 2, 4, 8] {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{threads}t")),
            &threads,
            |b, &thread_count| {
                b.iter(|| {
                    let handles: Vec<_> = (0..thread_count)
                        .map(|_| {
                            let e = Arc::clone(&engine);
                            std::thread::spawn(move || {
                                let _ = black_box(e.health_report());
                            })
                        })
                        .collect();
                    for h in handles {
                        let _ = h.join();
                    }
                });
            },
        );
    }
    group.finish();
}

fn bench_concurrent_build_tensor(c: &mut Criterion) {
    let engine = Arc::new(Engine::new());
    let mut group = c.benchmark_group("concurrent_build_tensor");

    for threads in [1, 2, 4, 8] {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{threads}t")),
            &threads,
            |b, &thread_count| {
                b.iter(|| {
                    let handles: Vec<_> = (0..thread_count)
                        .map(|_| {
                            let e = Arc::clone(&engine);
                            std::thread::spawn(move || {
                                black_box(e.build_tensor());
                            })
                        })
                        .collect();
                    for h in handles {
                        let _ = h.join();
                    }
                });
            },
        );
    }
    group.finish();
}

fn bench_mixed_concurrent_workload(c: &mut Criterion) {
    let engine = Arc::new(Engine::new());
    let mut group = c.benchmark_group("mixed_workload");

    for threads in [2, 4, 8] {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{threads}t")),
            &threads,
            |b, &thread_count| {
                b.iter(|| {
                    let handles: Vec<_> = (0..thread_count)
                        .map(|i| {
                            let e = Arc::clone(&engine);
                            std::thread::spawn(move || match i % 3 {
                                0 => { let _ = black_box(e.health_report()); }
                                1 => { let _ = black_box(e.learning_cycle()); }
                                _ => { black_box(e.build_tensor()); }
                            })
                        })
                        .collect();
                    for h in handles {
                        let _ = h.join();
                    }
                });
            },
        );
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_concurrent_health_reports,
    bench_concurrent_build_tensor,
    bench_mixed_concurrent_workload,
);
criterion_main!(benches);
