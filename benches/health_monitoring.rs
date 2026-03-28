//! Engine Benchmarks using real crate types
//!
//! Benchmarks `Engine::health_report()`, `Engine::build_tensor()`, `Engine::learning_cycle()`

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use maintenance_engine::engine::Engine;

fn bench_health_report(c: &mut Criterion) {
    let engine = Engine::new();
    c.bench_function("engine_health_report", |b| {
        b.iter(|| {
            let _ = black_box(engine.health_report());
        });
    });
}

fn bench_build_tensor(c: &mut Criterion) {
    let engine = Engine::new();
    c.bench_function("engine_build_tensor", |b| {
        b.iter(|| {
            black_box(engine.build_tensor());
        });
    });
}

fn bench_learning_cycle(c: &mut Criterion) {
    let engine = Engine::new();
    c.bench_function("engine_learning_cycle", |b| {
        b.iter(|| {
            let _ = black_box(engine.learning_cycle());
        });
    });
}

fn bench_health_report_batch(c: &mut Criterion) {
    let engine = Engine::new();
    for count in [10, 50, 100] {
        c.bench_function(&format!("engine_health_report_x{count}"), |b| {
            b.iter(|| {
                for _ in 0..count {
                    let _ = black_box(engine.health_report());
                }
            });
        });
    }
}

fn bench_weakest_layer(c: &mut Criterion) {
    let engine = Engine::new();
    c.bench_function("engine_weakest_layer", |b| {
        b.iter(|| {
            if let Ok(report) = engine.health_report() {
                black_box(report.weakest_layer());
            }
        });
    });
}

criterion_group!(
    benches,
    bench_health_report,
    bench_build_tensor,
    bench_learning_cycle,
    bench_health_report_batch,
    bench_weakest_layer,
);
criterion_main!(benches);
