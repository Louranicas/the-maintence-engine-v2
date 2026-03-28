//! `Tensor12D` Benchmarks using real crate types
//!
//! Benchmarks `Tensor12D` operations: creation, validation, distance, normalization

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use maintenance_engine::Tensor12D;

fn bench_tensor_creation(c: &mut Criterion) {
    c.bench_function("tensor_new_zeros", |b| {
        b.iter(|| black_box(Tensor12D::default()));
    });

    c.bench_function("tensor_new_from_array", |b| {
        let dims = [0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 0.05, 0.15, 0.25];
        b.iter(|| black_box(Tensor12D::new(dims)));
    });
}

fn bench_tensor_validate(c: &mut Criterion) {
    let valid = Tensor12D::new([0.5; 12]);
    c.bench_function("tensor_validate_valid", |b| {
        b.iter(|| {
            let _ = black_box(valid.validate());
        });
    });

    let mut invalid = Tensor12D::new([0.5; 12]);
    invalid.health_score = 1.5;
    c.bench_function("tensor_validate_invalid", |b| {
        b.iter(|| {
            let _ = black_box(invalid.validate());
        });
    });
}

fn bench_tensor_distance(c: &mut Criterion) {
    let t1 = Tensor12D::new([0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 0.05, 0.15, 0.25]);
    let t2 = Tensor12D::new([0.9, 0.8, 0.7, 0.6, 0.5, 0.4, 0.3, 0.2, 0.1, 0.95, 0.85, 0.75]);

    c.bench_function("tensor_distance_single", |b| {
        b.iter(|| black_box(t1.distance(&t2)));
    });

    let tensors: Vec<Tensor12D> = (0..100)
        .map(|i| {
            let v = f64::from(i) / 100.0;
            Tensor12D::new([v; 12])
        })
        .collect();

    for &count in &[10, 50, 100] {
        c.bench_with_input(
            BenchmarkId::new("tensor_pairwise_distance", count),
            &count,
            |b, &n| {
                b.iter(|| {
                    for i in 0..n {
                        for j in (i + 1)..n {
                            black_box(tensors[i].distance(&tensors[j]));
                        }
                    }
                });
            },
        );
    }
}

fn bench_tensor_clamp_normalize(c: &mut Criterion) {
    c.bench_function("tensor_clamp_normalize", |b| {
        b.iter(|| {
            let mut t = Tensor12D::new([1.5, -0.5, 2.0, -1.0, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5]);
            t.clamp_normalize();
            black_box(t);
        });
    });
}

fn bench_tensor_to_bytes(c: &mut Criterion) {
    let t = Tensor12D::new([0.5; 12]);
    c.bench_function("tensor_to_bytes", |b| {
        b.iter(|| black_box(t.to_bytes()));
    });
}

fn bench_tensor_to_array(c: &mut Criterion) {
    let t = Tensor12D::new([0.5; 12]);
    c.bench_function("tensor_to_array", |b| {
        b.iter(|| black_box(t.to_array()));
    });
}

criterion_group!(
    benches,
    bench_tensor_creation,
    bench_tensor_validate,
    bench_tensor_distance,
    bench_tensor_clamp_normalize,
    bench_tensor_to_bytes,
    bench_tensor_to_array,
);
criterion_main!(benches);
