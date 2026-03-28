//! Database Operations Benchmarks
//!
//! Async benchmarks for [`DatabaseManager`] from `maintenance_engine::database`.
//! Uses Criterion's `async_tokio` feature for benchmarking async SQLite
//! operations through `tempfile::TempDir` ephemeral databases.
//!
//! ## Benchmarked operations
//!
//! | Group | What |
//! |-------|------|
//! | `db_initialization` | `DatabaseManager::new()` |
//! | `db_write_fitness` | `write_fitness_history` |
//! | `db_write_tensor` | `write_tensor_snapshot` |
//! | `db_read_fitness` | `load_fitness_history` |
//! | `db_health_check` | `health_check_all` |

// Benchmarks are dev-only; allow unwrap for runtime/tempdir construction.
#![allow(clippy::unwrap_used)]

use criterion::{
    black_box, criterion_group, criterion_main, BenchmarkId, Criterion,
};
use std::time::Duration;
use tempfile::TempDir;

use maintenance_engine::database::{
    DatabaseManager, FitnessHistoryEntry, TensorSnapshot,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Create a fresh `DatabaseManager` backed by a temp directory.
/// Returns both the manager and the `TempDir` guard (must stay alive).
async fn fresh_manager() -> (DatabaseManager, TempDir) {
    let tmp = TempDir::new().unwrap();
    let mgr = DatabaseManager::new(tmp.path()).await.unwrap();
    (mgr, tmp)
}

fn sample_fitness_entry(generation: u64) -> FitnessHistoryEntry {
    FitnessHistoryEntry {
        timestamp: "2026-01-30T12:00:00Z".to_string(),
        fitness: 0.92,
        system_state: "healthy".to_string(),
        tensor_hash: "bench_hash_abc123".to_string(),
        generation,
    }
}

fn sample_tensor_snapshot(tick: u64) -> TensorSnapshot {
    TensorSnapshot {
        timestamp: "2026-01-30T12:00:00Z".to_string(),
        dimensions: [0.5, 0.12, 0.33, 0.25, 0.2, 0.0, 0.92, 0.99, 0.85, 0.05, 0.01, 0.75],
        source: "bench_evaluator".to_string(),
        tick,
    }
}

// ---------------------------------------------------------------------------
// Benchmark: DatabaseManager initialisation
// ---------------------------------------------------------------------------

fn bench_db_initialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("db_initialization");
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(20);

    let rt = tokio::runtime::Runtime::new().unwrap();

    group.bench_function("new_full", |b| {
        b.to_async(&rt).iter(|| async {
            let tmp = TempDir::new().unwrap();
            let mgr = DatabaseManager::new(tmp.path()).await;
            // Keep tmp alive until after mgr is created
            black_box((&mgr, &tmp));
        });
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// Benchmark: write_fitness_history
// ---------------------------------------------------------------------------

fn bench_db_write_fitness(c: &mut Criterion) {
    let mut group = c.benchmark_group("db_write_fitness");
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(30);

    let rt = tokio::runtime::Runtime::new().unwrap();

    // Pre-create manager outside the benchmark loop
    let (mgr, _tmp) = rt.block_on(fresh_manager());

    group.bench_function("single_write", |b| {
        let entry = sample_fitness_entry(1);
        b.to_async(&rt).iter(|| {
            let e = entry.clone();
            let m = &mgr;
            async move {
                let _ = black_box(m.write_fitness_history(&e).await);
            }
        });
    });

    // Batch of 10 sequential writes
    group.bench_function("batch_10_writes", |b| {
        b.to_async(&rt).iter(|| {
            let m = &mgr;
            async move {
                for i in 0_u64..10 {
                    let e = sample_fitness_entry(i);
                    let _ = m.write_fitness_history(&e).await;
                }
            }
        });
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// Benchmark: write_tensor_snapshot
// ---------------------------------------------------------------------------

fn bench_db_write_tensor(c: &mut Criterion) {
    let mut group = c.benchmark_group("db_write_tensor");
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(30);

    let rt = tokio::runtime::Runtime::new().unwrap();
    let (mgr, _tmp) = rt.block_on(fresh_manager());

    group.bench_function("single_write", |b| {
        let snap = sample_tensor_snapshot(1);
        b.to_async(&rt).iter(|| {
            let s = snap.clone();
            let m = &mgr;
            async move {
                let _ = black_box(m.write_tensor_snapshot(&s).await);
            }
        });
    });

    group.bench_function("batch_10_writes", |b| {
        b.to_async(&rt).iter(|| {
            let m = &mgr;
            async move {
                for i in 0_u64..10 {
                    let s = sample_tensor_snapshot(i);
                    let _ = m.write_tensor_snapshot(&s).await;
                }
            }
        });
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// Benchmark: load_fitness_history (read path)
// ---------------------------------------------------------------------------

fn bench_db_read_fitness(c: &mut Criterion) {
    let mut group = c.benchmark_group("db_read_fitness");
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(30);

    let rt = tokio::runtime::Runtime::new().unwrap();

    // Pre-populate with data
    let (mgr, _tmp) = rt.block_on(async {
        let (mgr, tmp) = fresh_manager().await;
        for i in 0_u64..100 {
            let entry = sample_fitness_entry(i);
            let _ = mgr.write_fitness_history(&entry).await;
        }
        (mgr, tmp)
    });

    for limit in [1_u32, 10, 50, 100] {
        group.bench_with_input(
            BenchmarkId::new("load_fitness_history", limit),
            &limit,
            |b, &lim| {
                b.to_async(&rt).iter(|| {
                    let m = &mgr;
                    async move {
                        let _ = black_box(m.load_fitness_history(lim).await);
                    }
                });
            },
        );
    }

    // read_latest_fitness (returns Option of 1 entry)
    group.bench_function("read_latest_fitness", |b| {
        b.to_async(&rt).iter(|| {
            let m = &mgr;
            async move {
                let _ = black_box(m.read_latest_fitness().await);
            }
        });
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// Benchmark: health_check_all
// ---------------------------------------------------------------------------

fn bench_db_health_check(c: &mut Criterion) {
    let mut group = c.benchmark_group("db_health_check");
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(20);

    let rt = tokio::runtime::Runtime::new().unwrap();
    let (mgr, _tmp) = rt.block_on(fresh_manager());

    group.bench_function("health_check_all", |b| {
        b.to_async(&rt).iter(|| {
            let m = &mgr;
            async move {
                let _ = black_box(m.health_check_all().await);
            }
        });
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// Criterion harness
// ---------------------------------------------------------------------------

criterion_group!(
    benches,
    bench_db_initialization,
    bench_db_write_fitness,
    bench_db_write_tensor,
    bench_db_read_fitness,
    bench_db_health_check
);

criterion_main!(benches);
