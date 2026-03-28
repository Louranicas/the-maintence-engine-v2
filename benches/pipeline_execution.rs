//! Pipeline Execution Benchmarks
//!
//! Benchmarks for the [`PipelineManager`] from `m3_core_logic::pipeline`.
//! Exercises creation, query, and lifecycle operations against the real
//! manager (which pre-loads 8 default pipelines on construction).
//!
//! ## Benchmarked operations
//!
//! | Group | What |
//! |-------|------|
//! | `pipeline_creation` | `PipelineManager::new()` (loads 8 defaults) |
//! | `pipeline_query` | `get_enabled_pipelines`, `get_pipelines_by_priority`, `get_pipeline` |
//! | `pipeline_lifecycle` | `disable_pipeline` / `enable_pipeline` cycles |
//! | `pipeline_stats` | `get_pipeline_stats`, `check_slo_compliance` |

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use std::time::Duration;

use maintenance_engine::m3_core_logic::pipeline::PipelineManager;

// ---------------------------------------------------------------------------
// Benchmark: PipelineManager construction
// ---------------------------------------------------------------------------

fn bench_pipeline_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("pipeline_creation");
    group.measurement_time(Duration::from_secs(5));

    group.bench_function("new_with_8_defaults", |b| {
        b.iter(|| black_box(PipelineManager::new()));
    });

    group.bench_function("pipeline_count", |b| {
        let mgr = PipelineManager::new();
        b.iter(|| black_box(mgr.pipeline_count()));
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// Benchmark: Query operations
// ---------------------------------------------------------------------------

fn bench_pipeline_query(c: &mut Criterion) {
    let mut group = c.benchmark_group("pipeline_query");
    group.measurement_time(Duration::from_secs(5));

    let mgr = PipelineManager::new();

    // -- get_enabled_pipelines (returns all 8 enabled by default) -----------
    group.bench_function("get_enabled_pipelines", |b| {
        b.iter(|| black_box(mgr.get_enabled_pipelines()));
    });

    // -- get_enabled_pipelines_batch: call 10 times -------------------------
    group.bench_function("get_enabled_pipelines_x10", |b| {
        b.iter(|| {
            for _ in 0..10 {
                let _ = black_box(mgr.get_enabled_pipelines());
            }
        });
    });

    // -- get_pipelines_by_priority for priorities 1, 2, 3 -------------------
    for priority in [1_u32, 2, 3] {
        group.bench_with_input(
            BenchmarkId::new("get_pipelines_by_priority", priority),
            &priority,
            |b, &p| {
                b.iter(|| black_box(mgr.get_pipelines_by_priority(p)));
            },
        );
    }

    // -- get_pipeline by known IDs ------------------------------------------
    let known_ids = [
        "PL-HEALTH-001",
        "PL-LOG-001",
        "PL-REMEDIATE-001",
        "PL-HEBBIAN-001",
        "PL-CONSENSUS-001",
        "PL-TENSOR-001",
        "PL-DISCOVERY-001",
        "PL-METRICS-001",
    ];

    for id in &known_ids {
        group.bench_with_input(
            BenchmarkId::new("get_pipeline", *id),
            id,
            |b, &pipeline_id| {
                b.iter(|| {
                    let _ = black_box(mgr.get_pipeline(pipeline_id));
                });
            },
        );
    }

    // -- get_pipeline for nonexistent ID (error path) -----------------------
    group.bench_function("get_pipeline_miss", |b| {
        b.iter(|| {
            let _ = black_box(mgr.get_pipeline("PL-DOES-NOT-EXIST"));
        });
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// Benchmark: Lifecycle operations (disable / enable cycles)
// ---------------------------------------------------------------------------

fn bench_pipeline_lifecycle(c: &mut Criterion) {
    let mut group = c.benchmark_group("pipeline_lifecycle");
    group.measurement_time(Duration::from_secs(5));

    // -- disable then enable a single pipeline ------------------------------
    group.bench_function("disable_enable_cycle", |b| {
        b.iter_batched(
            PipelineManager::new,
            |mgr| {
                let _ = mgr.disable_pipeline("PL-HEALTH-001");
                let _ = mgr.enable_pipeline("PL-HEALTH-001");
                mgr
            },
            criterion::BatchSize::SmallInput,
        );
    });

    // -- disable all 8 pipelines then re-enable all -------------------------
    group.bench_function("disable_enable_all_8", |b| {
        let ids = [
            "PL-HEALTH-001",
            "PL-LOG-001",
            "PL-REMEDIATE-001",
            "PL-HEBBIAN-001",
            "PL-CONSENSUS-001",
            "PL-TENSOR-001",
            "PL-DISCOVERY-001",
            "PL-METRICS-001",
        ];
        b.iter_batched(
            PipelineManager::new,
            |mgr| {
                for id in &ids {
                    let _ = mgr.disable_pipeline(id);
                }
                for id in &ids {
                    let _ = mgr.enable_pipeline(id);
                }
                mgr
            },
            criterion::BatchSize::SmallInput,
        );
    });

    // -- start and complete execution ---------------------------------------
    group.bench_function("start_complete_execution", |b| {
        b.iter_batched(
            PipelineManager::new,
            |mgr| {
                if let Ok(exec) = mgr.start_execution("PL-HEALTH-001") {
                    let _ = mgr.complete_execution(&exec.execution_id, vec![]);
                }
                mgr
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// Benchmark: Statistics and SLO compliance
// ---------------------------------------------------------------------------

fn bench_pipeline_stats(c: &mut Criterion) {
    let mut group = c.benchmark_group("pipeline_stats");
    group.measurement_time(Duration::from_secs(5));

    // Build a manager with some execution history
    let mgr = PipelineManager::new();
    for _ in 0..20 {
        if let Ok(exec) = mgr.start_execution("PL-HEALTH-001") {
            let _ = mgr.complete_execution(&exec.execution_id, vec![]);
        }
    }

    group.bench_function("get_pipeline_stats", |b| {
        b.iter(|| {
            let _ = black_box(mgr.get_pipeline_stats("PL-HEALTH-001"));
        });
    });

    group.bench_function("check_slo_compliance", |b| {
        b.iter(|| {
            let _ = black_box(mgr.check_slo_compliance("PL-HEALTH-001"));
        });
    });

    group.bench_function("get_active_executions", |b| {
        b.iter(|| black_box(mgr.get_active_executions()));
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// Criterion harness
// ---------------------------------------------------------------------------

criterion_group!(
    benches,
    bench_pipeline_creation,
    bench_pipeline_query,
    bench_pipeline_lifecycle,
    bench_pipeline_stats
);

criterion_main!(benches);
