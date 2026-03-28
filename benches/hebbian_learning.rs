//! # Hebbian Learning Benchmarks
//!
//! Benchmarks for Hebbian learning operations using real crate types (M25-M26).
//! Target SLO: <100ms for pathway updates.
//! STDP Parameters: LTP=0.1, LTD=0.05, window=100ms, decay=0.001

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::time::Duration;

use maintenance_engine::m5_learning::hebbian::HebbianManager;
use maintenance_engine::m5_learning::PulseTrigger;

// ---------------------------------------------------------------------------
// Constants for benchmark parameterisation
// ---------------------------------------------------------------------------

/// Default pathway keys known to exist after `HebbianManager::new()`.
/// These come from `default_pathways()` in the m5_learning module.
const DEFAULT_PATHWAY_KEYS: &[&str] = &[
    "maintenance->service_restart",
    "maintenance->database_vacuum",
    "maintenance->cache_cleanup",
    "maintenance->session_rotation",
    "health_failure->service_restart",
    "latency_spike->cache_cleanup",
    "memory_pressure->session_rotation",
    "consensus_proposal->agent_vote",
    "dissent_detected->learning_update",
];

// ---------------------------------------------------------------------------
// Benchmark: apply_decay
// ---------------------------------------------------------------------------

fn bench_apply_decay(c: &mut Criterion) {
    let mut group = c.benchmark_group("hebbian_apply_decay");
    group.measurement_time(Duration::from_secs(5));

    // Single decay cycle on default pathways
    group.bench_function("single_cycle", |b| {
        let manager = HebbianManager::new();
        b.iter(|| black_box(manager.apply_decay()));
    });

    // Batch decay cycles (simulating periodic maintenance)
    for cycles in [10_u64, 50, 100] {
        group.throughput(Throughput::Elements(cycles));
        group.bench_with_input(
            BenchmarkId::new("batch_cycles", cycles),
            &cycles,
            |b, &cycles| {
                b.iter(|| {
                    let manager = HebbianManager::new();
                    let mut total_affected = 0_usize;
                    for _ in 0..cycles {
                        total_affected += manager.apply_decay();
                    }
                    black_box(total_affected)
                });
            },
        );
    }

    // Decay with varying pathway counts (add extra pathways)
    for extra in [0_usize, 20, 50] {
        let label = format!("{}_pathways", 9 + extra);
        group.bench_function(label, |b| {
            b.iter_with_setup(
                || {
                    let manager = HebbianManager::new();
                    for i in 0..extra {
                        let _ = manager.add_pathway(
                            format!("bench_src_{i}"),
                            format!("bench_tgt_{i}"),
                            maintenance_engine::m5_learning::PathwayType::ServiceToService,
                        );
                    }
                    manager
                },
                |manager| {
                    black_box(manager.apply_decay());
                },
            );
        });
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// Benchmark: strengthen / weaken
// ---------------------------------------------------------------------------

fn bench_strengthen_weaken(c: &mut Criterion) {
    let mut group = c.benchmark_group("hebbian_strengthen_weaken");
    group.measurement_time(Duration::from_secs(5));

    // Single strengthen operation
    group.bench_function("single_strengthen", |b| {
        let manager = HebbianManager::new();
        let key = DEFAULT_PATHWAY_KEYS[0];
        b.iter(|| black_box(manager.strengthen(key).ok()));
    });

    // Single weaken operation
    group.bench_function("single_weaken", |b| {
        let manager = HebbianManager::new();
        let key = DEFAULT_PATHWAY_KEYS[0];
        b.iter(|| black_box(manager.weaken(key).ok()));
    });

    // Batch strengthen across all default pathways
    group.bench_function("strengthen_all_defaults", |b| {
        let manager = HebbianManager::new();
        b.iter(|| {
            for key in DEFAULT_PATHWAY_KEYS {
                let _ = manager.strengthen(key);
            }
            black_box(manager.pathway_count())
        });
    });

    // Batch weaken across all default pathways
    group.bench_function("weaken_all_defaults", |b| {
        let manager = HebbianManager::new();
        b.iter(|| {
            for key in DEFAULT_PATHWAY_KEYS {
                let _ = manager.weaken(key);
            }
            black_box(manager.pathway_count())
        });
    });

    // Repeated strengthen on single pathway (approaches ceiling)
    for iterations in [5_u64, 10, 20] {
        group.throughput(Throughput::Elements(iterations));
        group.bench_with_input(
            BenchmarkId::new("repeated_strengthen", iterations),
            &iterations,
            |b, &count| {
                let manager = HebbianManager::new();
                let key = DEFAULT_PATHWAY_KEYS[0];
                b.iter(|| {
                    for _ in 0..count {
                        let _ = manager.strengthen(key);
                    }
                    black_box(manager.get_strongest_pathways(1))
                });
            },
        );
    }

    // Alternating strengthen/weaken (simulating noisy signal)
    group.bench_function("alternating_ltp_ltd", |b| {
        let manager = HebbianManager::new();
        let key = DEFAULT_PATHWAY_KEYS[0];
        b.iter(|| {
            for i in 0..20_u32 {
                if i % 2 == 0 {
                    let _ = manager.strengthen(key);
                } else {
                    let _ = manager.weaken(key);
                }
            }
            black_box(())
        });
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// Benchmark: record_success / record_failure
// ---------------------------------------------------------------------------

fn bench_record_success_failure(c: &mut Criterion) {
    let mut group = c.benchmark_group("hebbian_record_events");
    group.measurement_time(Duration::from_secs(5));

    // Single success
    group.bench_function("single_success", |b| {
        let manager = HebbianManager::new();
        let key = DEFAULT_PATHWAY_KEYS[0];
        b.iter(|| black_box(manager.record_success(key).ok()));
    });

    // Single failure
    group.bench_function("single_failure", |b| {
        let manager = HebbianManager::new();
        let key = DEFAULT_PATHWAY_KEYS[0];
        b.iter(|| black_box(manager.record_failure(key).ok()));
    });

    // Batch successes across all default pathways
    group.bench_function("batch_success_all_defaults", |b| {
        let manager = HebbianManager::new();
        b.iter(|| {
            for key in DEFAULT_PATHWAY_KEYS {
                let _ = manager.record_success(key);
            }
            black_box(())
        });
    });

    // Batch failures across all default pathways
    group.bench_function("batch_failure_all_defaults", |b| {
        let manager = HebbianManager::new();
        b.iter(|| {
            for key in DEFAULT_PATHWAY_KEYS {
                let _ = manager.record_failure(key);
            }
            black_box(())
        });
    });

    // Mixed success/failure (realistic workload)
    for event_count in [20_u64, 50, 100] {
        group.throughput(Throughput::Elements(event_count));
        group.bench_with_input(
            BenchmarkId::new("mixed_events", event_count),
            &event_count,
            |b, &count| {
                let manager = HebbianManager::new();
                b.iter(|| {
                    for i in 0..count {
                        let key = DEFAULT_PATHWAY_KEYS[(i as usize) % DEFAULT_PATHWAY_KEYS.len()];
                        // 70% success, 30% failure
                        if i % 10 < 7 {
                            let _ = manager.record_success(key);
                        } else {
                            let _ = manager.record_failure(key);
                        }
                    }
                    black_box(())
                });
            },
        );
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// Benchmark: get_strongest_pathways / get_weakest_pathways
// ---------------------------------------------------------------------------

fn bench_pathway_queries(c: &mut Criterion) {
    let mut group = c.benchmark_group("hebbian_pathway_queries");
    group.measurement_time(Duration::from_secs(5));

    // Strongest pathways with default pathways (9 total)
    for n in [1_usize, 3, 5, 9] {
        group.bench_function(format!("strongest_top_{n}"), |b| {
            let manager = HebbianManager::new();
            b.iter(|| black_box(manager.get_strongest_pathways(n)));
        });
    }

    // Weakest pathways with default pathways
    for n in [1_usize, 3, 5, 9] {
        group.bench_function(format!("weakest_bottom_{n}"), |b| {
            let manager = HebbianManager::new();
            b.iter(|| black_box(manager.get_weakest_pathways(n)));
        });
    }

    // Strongest pathways after varied strengthening (sorted output test)
    group.bench_function("strongest_after_varied_strength", |b| {
        b.iter_with_setup(
            || {
                let manager = HebbianManager::new();
                // Strengthen different pathways by different amounts
                for (i, key) in DEFAULT_PATHWAY_KEYS.iter().enumerate() {
                    for _ in 0..i {
                        let _ = manager.strengthen(key);
                    }
                }
                manager
            },
            |manager| {
                black_box(manager.get_strongest_pathways(3));
            },
        );
    });

    // Weakest pathways after decay cycles
    group.bench_function("weakest_after_decay", |b| {
        b.iter_with_setup(
            || {
                let manager = HebbianManager::new();
                for _ in 0..50 {
                    manager.apply_decay();
                }
                manager
            },
            |manager| {
                black_box(manager.get_weakest_pathways(3));
            },
        );
    });

    // With larger pathway count
    group.bench_function("strongest_top5_50_pathways", |b| {
        b.iter_with_setup(
            || {
                let manager = HebbianManager::new();
                for i in 0..41 {
                    let _ = manager.add_pathway(
                        format!("src_{i}"),
                        format!("tgt_{i}"),
                        maintenance_engine::m5_learning::PathwayType::ServiceToService,
                    );
                }
                manager
            },
            |manager| {
                black_box(manager.get_strongest_pathways(5));
            },
        );
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// Benchmark: trigger_pulse
// ---------------------------------------------------------------------------

fn bench_trigger_pulse(c: &mut Criterion) {
    let mut group = c.benchmark_group("hebbian_trigger_pulse");
    group.measurement_time(Duration::from_secs(5));

    // Single pulse with default pathways
    let triggers = [
        ("manual", PulseTrigger::Manual),
        ("time_interval", PulseTrigger::TimeInterval),
        ("action_count", PulseTrigger::ActionCount),
        ("pattern_detected", PulseTrigger::PatternDetected),
    ];

    for (name, trigger) in &triggers {
        group.bench_function(format!("single_{name}"), |b| {
            let manager = HebbianManager::new();
            b.iter(|| black_box(manager.trigger_pulse(*trigger).ok()));
        });
    }

    // Pulse after strengthening operations (populated LTP/LTD counts)
    group.bench_function("pulse_after_learning", |b| {
        b.iter_with_setup(
            || {
                let manager = HebbianManager::new();
                // Simulate a learning session
                for key in DEFAULT_PATHWAY_KEYS {
                    for _ in 0..5 {
                        let _ = manager.record_success(key);
                    }
                    for _ in 0..2 {
                        let _ = manager.record_failure(key);
                    }
                }
                manager
            },
            |manager| {
                black_box(manager.trigger_pulse(PulseTrigger::Manual).ok());
            },
        );
    });

    // Sequential pulses (simulate periodic pulse firing)
    for pulse_count in [5_u64, 10, 20] {
        group.throughput(Throughput::Elements(pulse_count));
        group.bench_with_input(
            BenchmarkId::new("sequential_pulses", pulse_count),
            &pulse_count,
            |b, &count| {
                let manager = HebbianManager::new();
                b.iter(|| {
                    for _ in 0..count {
                        let _ = manager.trigger_pulse(PulseTrigger::TimeInterval);
                    }
                    black_box(())
                });
            },
        );
    }

    // Pulse with larger pathway count
    group.bench_function("pulse_50_pathways", |b| {
        b.iter_with_setup(
            || {
                let manager = HebbianManager::new();
                for i in 0..41 {
                    let _ = manager.add_pathway(
                        format!("pulse_src_{i}"),
                        format!("pulse_tgt_{i}"),
                        maintenance_engine::m5_learning::PathwayType::MetricToAction,
                    );
                }
                manager
            },
            |manager| {
                black_box(manager.trigger_pulse(PulseTrigger::Manual).ok());
            },
        );
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// Benchmark: get_routing_weight
// ---------------------------------------------------------------------------

fn bench_routing_weight(c: &mut Criterion) {
    let mut group = c.benchmark_group("hebbian_routing_weight");
    group.measurement_time(Duration::from_secs(5));

    // Routing weight for existing pathway
    group.bench_function("existing_pathway", |b| {
        let manager = HebbianManager::new();
        b.iter(|| black_box(manager.get_routing_weight("maintenance", "service_restart")));
    });

    // Routing weight for non-existent pathway (returns 0.0)
    group.bench_function("nonexistent_pathway", |b| {
        let manager = HebbianManager::new();
        b.iter(|| black_box(manager.get_routing_weight("nonexistent", "path")));
    });

    // Routing weight after learning activity (affects success rate)
    group.bench_function("after_successes", |b| {
        b.iter_with_setup(
            || {
                let manager = HebbianManager::new();
                let key = "maintenance->service_restart";
                for _ in 0..10 {
                    let _ = manager.record_success(key);
                }
                manager
            },
            |manager| {
                black_box(manager.get_routing_weight("maintenance", "service_restart"));
            },
        );
    });

    group.bench_function("after_failures", |b| {
        b.iter_with_setup(
            || {
                let manager = HebbianManager::new();
                let key = "maintenance->service_restart";
                for _ in 0..10 {
                    let _ = manager.record_failure(key);
                }
                manager
            },
            |manager| {
                black_box(manager.get_routing_weight("maintenance", "service_restart"));
            },
        );
    });

    // Batch routing weight lookups (simulating routing decisions)
    group.bench_function("batch_all_defaults", |b| {
        let manager = HebbianManager::new();
        // Pre-extract source/target pairs from keys
        let pairs: Vec<(&str, &str)> = DEFAULT_PATHWAY_KEYS
            .iter()
            .filter_map(|key| key.split_once("->"))
            .collect();
        b.iter(|| {
            let mut total = 0.0_f64;
            for &(src, tgt) in &pairs {
                total += manager.get_routing_weight(src, tgt);
            }
            black_box(total)
        });
    });

    // Routing weight stability over many lookups (same manager, no mutations)
    group.bench_function("repeated_lookup_100", |b| {
        let manager = HebbianManager::new();
        b.iter(|| {
            let mut total = 0.0_f64;
            for _ in 0..100 {
                total += manager.get_routing_weight("maintenance", "service_restart");
            }
            black_box(total)
        });
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// Criterion harness
// ---------------------------------------------------------------------------

criterion_group!(
    benches,
    bench_apply_decay,
    bench_strengthen_weaken,
    bench_record_success_failure,
    bench_pathway_queries,
    bench_trigger_pulse,
    bench_routing_weight
);

criterion_main!(benches);
