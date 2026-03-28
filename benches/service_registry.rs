//! Service Registry Benchmarks
//!
//! Benchmarks for [`ServiceRegistry`] from `m2_services::service_registry`.
//! Uses the real crate types -- `ServiceRegistry`, `ServiceDefinition`,
//! `ServiceTier`, `HealthStatus`, and `register_ultraplate_services()`.
//!
//! ## Benchmarked operations
//!
//! | Group | What |
//! |-------|------|
//! | `registry_creation` | `register_ultraplate_services` (12 services) |
//! | `registry_lookup` | `discover`, `discover_by_tier`, `discover_by_protocol`, `list_services` |
//! | `registry_health` | `update_health`, `get_health`, `get_healthy_services` |
//! | `registry_dependency` | `add_dependency`, `get_dependencies`, `get_dependents` |
//! | `registry_mutation` | `register`, `deregister` cycles |

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use std::time::Duration;

use maintenance_engine::m2_services::service_registry::{
    register_ultraplate_services, ServiceDefinition, ServiceRegistry,
};
use maintenance_engine::m2_services::{HealthStatus, ServiceDiscovery, ServiceTier};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build a fully-populated ULTRAPLATE registry (12 services).
fn ultraplate_registry() -> ServiceRegistry {
    let reg = ServiceRegistry::new();
    let _ = register_ultraplate_services(&reg);
    reg
}

/// Build a registry with `n` synthetic services for scaling tests.
fn large_registry(n: usize) -> ServiceRegistry {
    let reg = ServiceRegistry::new();
    for i in 0..n {
        let tier = match i % 5 {
            0 => ServiceTier::Tier1,
            1 => ServiceTier::Tier2,
            2 => ServiceTier::Tier3,
            3 => ServiceTier::Tier4,
            _ => ServiceTier::Tier5,
        };
        let def = ServiceDefinition::builder(
            format!("svc-{i}"),
            format!("Service {i}"),
            "1.0.0",
        )
        .tier(tier)
        .port(8000 + i as u16)
        .protocol("REST")
        .build();
        let _ = reg.register(def);
    }
    reg
}

// ---------------------------------------------------------------------------
// Benchmark: Registry creation
// ---------------------------------------------------------------------------

fn bench_registry_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("registry_creation");
    group.measurement_time(Duration::from_secs(5));

    group.bench_function("register_ultraplate_12", |b| {
        b.iter(|| {
            let reg = ServiceRegistry::new();
            let _ = register_ultraplate_services(&reg);
            black_box(reg)
        });
    });

    group.bench_function("empty_registry", |b| {
        b.iter(|| black_box(ServiceRegistry::new()));
    });

    for size in [50, 100, 500] {
        group.bench_with_input(
            BenchmarkId::new("register_n_services", size),
            &size,
            |b, &n| {
                b.iter(|| black_box(large_registry(n)));
            },
        );
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// Benchmark: Lookup operations
// ---------------------------------------------------------------------------

fn bench_registry_lookup(c: &mut Criterion) {
    let mut group = c.benchmark_group("registry_lookup");
    group.measurement_time(Duration::from_secs(5));

    let reg = ultraplate_registry();

    // -- discover by ID -----------------------------------------------------
    let service_ids = [
        "synthex",
        "san-k7",
        "nais",
        "codesynthor-v7",
        "tool-library",
        "bash-engine",
    ];

    for id in &service_ids {
        group.bench_with_input(
            BenchmarkId::new("discover", *id),
            id,
            |b, &svc_id| {
                b.iter(|| {
                    let _ = black_box(reg.discover(svc_id));
                });
            },
        );
    }

    // -- discover miss (error path) -----------------------------------------
    group.bench_function("discover_miss", |b| {
        b.iter(|| {
            let _ = black_box(reg.discover("nonexistent-service"));
        });
    });

    // -- discover_by_tier ---------------------------------------------------
    for tier in [
        ServiceTier::Tier1,
        ServiceTier::Tier2,
        ServiceTier::Tier3,
        ServiceTier::Tier4,
        ServiceTier::Tier5,
    ] {
        group.bench_with_input(
            BenchmarkId::new("discover_by_tier", tier.number()),
            &tier,
            |b, &t| {
                b.iter(|| black_box(reg.discover_by_tier(t)));
            },
        );
    }

    // -- discover_by_protocol -----------------------------------------------
    group.bench_function("discover_by_protocol_REST", |b| {
        b.iter(|| black_box(reg.discover_by_protocol("REST")));
    });

    group.bench_function("discover_by_protocol_gRPC", |b| {
        b.iter(|| black_box(reg.discover_by_protocol("gRPC")));
    });

    // -- list_services ------------------------------------------------------
    group.bench_function("list_services", |b| {
        b.iter(|| black_box(reg.list_services()));
    });

    // -- service_count / is_registered --------------------------------------
    group.bench_function("service_count", |b| {
        b.iter(|| black_box(reg.service_count()));
    });

    group.bench_function("is_registered", |b| {
        b.iter(|| black_box(reg.is_registered("synthex")));
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// Benchmark: Health operations
// ---------------------------------------------------------------------------

fn bench_registry_health(c: &mut Criterion) {
    let mut group = c.benchmark_group("registry_health");
    group.measurement_time(Duration::from_secs(5));

    // -- update_health + get_health cycle -----------------------------------
    group.bench_function("update_and_get_health", |b| {
        b.iter_batched(
            ultraplate_registry,
            |reg| {
                let _ = reg.update_health("synthex", HealthStatus::Healthy);
                let _ = black_box(reg.get_health("synthex"));
                reg
            },
            criterion::BatchSize::SmallInput,
        );
    });

    // -- get_healthy_services (none healthy initially) ----------------------
    group.bench_function("get_healthy_services_none", |b| {
        let reg = ultraplate_registry();
        b.iter(|| black_box(reg.get_healthy_services()));
    });

    // -- get_healthy_services (all marked healthy) --------------------------
    group.bench_function("get_healthy_services_all", |b| {
        let reg = ultraplate_registry();
        let ids: Vec<String> = reg.list_services().iter().map(|s| s.service_id.clone()).collect();
        for id in &ids {
            let _ = reg.update_health(id, HealthStatus::Healthy);
        }
        b.iter(|| black_box(reg.get_healthy_services()));
    });

    // -- update all 12 services health in a batch ---------------------------
    group.bench_function("update_health_all_12", |b| {
        let ids: Vec<String> = {
            let reg = ultraplate_registry();
            reg.list_services().iter().map(|s| s.service_id.clone()).collect()
        };
        b.iter_batched(
            ultraplate_registry,
            |reg| {
                for id in &ids {
                    let _ = reg.update_health(id, HealthStatus::Healthy);
                }
                reg
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// Benchmark: Dependency operations
// ---------------------------------------------------------------------------

fn bench_registry_dependency(c: &mut Criterion) {
    let mut group = c.benchmark_group("registry_dependency");
    group.measurement_time(Duration::from_secs(5));

    // -- add_dependency -----------------------------------------------------
    group.bench_function("add_dependency", |b| {
        b.iter_batched(
            ultraplate_registry,
            |reg| {
                let _ = reg.add_dependency("synthex", "san-k7");
                reg
            },
            criterion::BatchSize::SmallInput,
        );
    });

    // -- get_dependencies after wiring up a graph ---------------------------
    group.bench_function("get_dependencies", |b| {
        let reg = ultraplate_registry();
        let _ = reg.add_dependency("synthex", "san-k7");
        let _ = reg.add_dependency("synthex", "nais");
        let _ = reg.add_dependency("san-k7", "nais");
        b.iter(|| {
            let _ = black_box(reg.get_dependencies("synthex"));
        });
    });

    // -- get_dependents after wiring up a graph -----------------------------
    group.bench_function("get_dependents", |b| {
        let reg = ultraplate_registry();
        let _ = reg.add_dependency("synthex", "nais");
        let _ = reg.add_dependency("san-k7", "nais");
        let _ = reg.add_dependency("codesynthor-v7", "nais");
        b.iter(|| {
            let _ = black_box(reg.get_dependents("nais"));
        });
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// Benchmark: Registration / deregistration mutations
// ---------------------------------------------------------------------------

fn bench_registry_mutation(c: &mut Criterion) {
    let mut group = c.benchmark_group("registry_mutation");
    group.measurement_time(Duration::from_secs(5));

    // -- register then deregister -------------------------------------------
    group.bench_function("register_deregister_cycle", |b| {
        b.iter_batched(
            ultraplate_registry,
            |reg| {
                let def = ServiceDefinition::builder("bench-svc", "Bench Service", "0.1.0")
                    .tier(ServiceTier::Tier5)
                    .port(9999)
                    .build();
                let _ = reg.register(def);
                let _ = reg.deregister("bench-svc");
                reg
            },
            criterion::BatchSize::SmallInput,
        );
    });

    // -- register batch of 50 new services ----------------------------------
    group.bench_function("register_batch_50", |b| {
        let defs: Vec<ServiceDefinition> = (0..50)
            .map(|i| {
                ServiceDefinition::builder(
                    format!("batch-{i}"),
                    format!("Batch Service {i}"),
                    "1.0.0",
                )
                .tier(ServiceTier::Tier3)
                .port(10000 + i)
                .build()
            })
            .collect();

        b.iter_batched(
            || (ultraplate_registry(), defs.clone()),
            |(reg, batch)| {
                for def in batch {
                    let _ = reg.register(def);
                }
                reg
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// Benchmark: Large registry scaling
// ---------------------------------------------------------------------------

fn bench_large_registry(c: &mut Criterion) {
    let mut group = c.benchmark_group("large_registry");
    group.measurement_time(Duration::from_secs(10));

    for size in [100, 500, 1000] {
        let reg = large_registry(size);

        group.bench_with_input(
            BenchmarkId::new("discover_by_id", size),
            &reg,
            |b, registry| {
                b.iter(|| {
                    let _ = black_box(registry.discover("svc-50"));
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("discover_by_tier", size),
            &reg,
            |b, registry| {
                b.iter(|| black_box(registry.discover_by_tier(ServiceTier::Tier1)));
            },
        );

        group.bench_with_input(
            BenchmarkId::new("list_services", size),
            &reg,
            |b, registry| {
                b.iter(|| black_box(registry.list_services()));
            },
        );
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// Criterion harness
// ---------------------------------------------------------------------------

criterion_group!(
    benches,
    bench_registry_creation,
    bench_registry_lookup,
    bench_registry_health,
    bench_registry_dependency,
    bench_registry_mutation,
    bench_large_registry
);

criterion_main!(benches);
