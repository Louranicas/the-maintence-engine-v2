#![allow(clippy::unwrap_used)]
//! # Stress Tests
//!
//! Concurrent stress tests for the Maintenance Engine. Exercises the
//! internal `RwLock` synchronization by running many tasks in parallel
//! via `tokio::spawn` with `Arc<Engine>`.
//!
//! ## Coverage
//!
//! | # | Workload | Concurrency |
//! |---|----------|-------------|
//! | 1 | 100 concurrent health_report() | 100 tokio::spawn |
//! | 2 | 100 concurrent build_tensor() | 100 tokio::spawn |
//! | 3 | 50 concurrent submit_remediation() | 50 tokio::spawn |
//! | 4 | 50 concurrent learning_cycle() | 50 tokio::spawn |
//! | 5 | Mixed workload (health + tensor + learning) | 20 tokio::spawn |
//! | 6 | 100 concurrent DB fitness writes | 100 tokio::spawn |
//! | 7 | State consistency after stress | Sequential assertions |

use std::sync::Arc;

use maintenance_engine::database::{DatabaseManager, FitnessHistoryEntry, PerformanceSample};
use maintenance_engine::engine::Engine;
use maintenance_engine::m3_core_logic::{IssueType, Severity};

// ===========================================================================
// Helpers
// ===========================================================================

/// Create an `Arc<Engine>` suitable for sharing across tasks.
fn arc_engine() -> Arc<Engine> {
    Arc::new(Engine::new())
}

/// Create a `FitnessHistoryEntry` with the given generation.
fn make_fitness_entry(generation: u64) -> FitnessHistoryEntry {
    FitnessHistoryEntry {
        timestamp: "2026-01-30T00:00:00Z".to_string(),
        fitness: 0.85 + (generation as f64 * 0.001),
        system_state: "healthy".to_string(),
        tensor_hash: format!("hash-{generation:04}"),
        generation,
    }
}

/// Create a `PerformanceSample` with the given index.
fn make_perf_sample(idx: u32) -> PerformanceSample {
    PerformanceSample {
        metric_name: format!("stress_metric_{idx}"),
        value: f64::from(idx) * 1.5,
        unit: "ms".to_string(),
        timestamp: "2026-01-30T00:00:00Z".to_string(),
    }
}

// ===========================================================================
// Test 1: 100 concurrent health_report() calls
// ===========================================================================

#[tokio::test]
async fn test_stress_concurrent_health_reports() {
    let engine = arc_engine();
    let mut handles = Vec::with_capacity(100);

    for _ in 0..100 {
        let e = Arc::clone(&engine);
        handles.push(tokio::spawn(async move {
            let report = e.health_report();
            assert!(report.is_ok(), "health_report should not fail under concurrency");
            let r = report.unwrap();
            assert!(
                (0.0..=1.0).contains(&r.overall_health),
                "health should be in [0,1]"
            );
            r.overall_health
        }));
    }

    let mut results = Vec::with_capacity(100);
    for handle in handles {
        results.push(handle.await.unwrap());
    }

    // All 100 results should be valid scores
    assert_eq!(results.len(), 100);
    for &score in &results {
        assert!((0.0..=1.0).contains(&score));
    }
}

// ===========================================================================
// Test 2: 100 concurrent build_tensor() calls
// ===========================================================================

#[tokio::test]
async fn test_stress_concurrent_build_tensor() {
    let engine = arc_engine();
    let mut handles = Vec::with_capacity(100);

    for _ in 0..100 {
        let e = Arc::clone(&engine);
        handles.push(tokio::spawn(async move {
            let tensor = e.build_tensor();
            assert!(
                tensor.validate().is_ok(),
                "tensor should remain valid under concurrency"
            );
            tensor.health_score
        }));
    }

    let mut results = Vec::with_capacity(100);
    for handle in handles {
        results.push(handle.await.unwrap());
    }

    assert_eq!(results.len(), 100);
    for &score in &results {
        assert!(
            (0.0..=1.0).contains(&score),
            "health_score out of range: {score}"
        );
    }
}

// ===========================================================================
// Test 3: 50 concurrent submit_remediation() calls
// ===========================================================================

#[tokio::test]
async fn test_stress_concurrent_submit_remediation() {
    let engine = arc_engine();
    let mut handles = Vec::with_capacity(50);

    for i in 0_u32..50 {
        let e = Arc::clone(&engine);
        handles.push(tokio::spawn(async move {
            let service_id = format!("stress-svc-{i:03}");
            let result = e.submit_remediation(
                &service_id,
                IssueType::HealthFailure,
                Severity::Medium,
                "stress test remediation",
            );
            assert!(
                result.is_ok(),
                "submit_remediation should succeed for {service_id}: {result:?}"
            );
            result.unwrap()
        }));
    }

    let mut ids = Vec::with_capacity(50);
    for handle in handles {
        ids.push(handle.await.unwrap());
    }

    // All 50 IDs should be unique
    assert_eq!(ids.len(), 50);
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), 50, "all remediation IDs should be unique");

    // Engine should reflect the pending count
    assert!(
        engine.pending_remediations() >= 50,
        "should have at least 50 pending, got {}",
        engine.pending_remediations()
    );
}

// ===========================================================================
// Test 4: 50 concurrent learning_cycle() calls
// ===========================================================================

#[tokio::test]
async fn test_stress_concurrent_learning_cycles() {
    let engine = arc_engine();

    let initial_strength = engine.average_pathway_strength();

    let mut handles = Vec::with_capacity(50);
    for _ in 0..50 {
        let e = Arc::clone(&engine);
        handles.push(tokio::spawn(async move {
            let result = e.learning_cycle();
            assert!(
                result.is_ok(),
                "learning_cycle should not fail under concurrency"
            );
            result.unwrap().pathways_decayed
        }));
    }

    let mut total_decayed = 0_usize;
    for handle in handles {
        total_decayed += handle.await.unwrap();
    }

    // At least some pathways should have been decayed
    assert!(
        total_decayed > 0,
        "across 50 cycles, some pathways should decay"
    );

    // Average strength should have decreased
    let final_strength = engine.average_pathway_strength();
    assert!(
        final_strength <= initial_strength + f64::EPSILON,
        "50 concurrent decay cycles should not increase average strength: \
         initial={initial_strength}, final={final_strength}"
    );
}

// ===========================================================================
// Test 5: Mixed workload - 20 threads each doing health + tensor + learning
// ===========================================================================

#[tokio::test]
async fn test_stress_mixed_workload() {
    let engine = arc_engine();
    let mut handles = Vec::with_capacity(20);

    for i in 0_u32..20 {
        let e = Arc::clone(&engine);
        handles.push(tokio::spawn(async move {
            // Operation A: health report
            let report = e.health_report();
            assert!(
                report.is_ok(),
                "mixed workload {i}: health_report failed"
            );

            // Operation B: build tensor
            let tensor = e.build_tensor();
            assert!(
                tensor.validate().is_ok(),
                "mixed workload {i}: tensor invalid"
            );

            // Operation C: learning cycle
            let cycle = e.learning_cycle();
            assert!(
                cycle.is_ok(),
                "mixed workload {i}: learning_cycle failed"
            );

            // Return combined result for verification
            (
                report.unwrap().overall_health,
                tensor.health_score,
                cycle.unwrap().had_activity(),
            )
        }));
    }

    let mut results = Vec::with_capacity(20);
    for handle in handles {
        results.push(handle.await.unwrap());
    }

    assert_eq!(results.len(), 20);

    for (health, tensor_health, had_activity) in &results {
        assert!(
            (0.0..=1.0).contains(health),
            "health out of range: {health}"
        );
        assert!(
            (0.0..=1.0).contains(tensor_health),
            "tensor health out of range: {tensor_health}"
        );
        // At least the first few cycles should produce activity (decay).
        // Later cycles may not if pathways are already at MIN_STRENGTH.
        let _ = had_activity; // validated below
    }

    // At least some of the 20 tasks should have produced activity
    let active_count = results.iter().filter(|(_, _, a)| *a).count();
    assert!(
        active_count > 0,
        "at least some tasks should produce learning activity"
    );
}

// ===========================================================================
// Test 6: Concurrent DB writes - 100 fitness entries in parallel
// ===========================================================================

#[tokio::test]
async fn test_stress_concurrent_db_writes() {
    let temp = tempfile::TempDir::new().unwrap();
    let db = Arc::new(DatabaseManager::new(temp.path()).await.unwrap());

    let mut handles = Vec::with_capacity(100);

    for i in 0_u64..100 {
        let db_clone = Arc::clone(&db);
        handles.push(tokio::spawn(async move {
            let entry = make_fitness_entry(i);
            let result = db_clone.write_fitness_history(&entry).await;
            assert!(
                result.is_ok(),
                "concurrent fitness write {i} should succeed: {result:?}"
            );
        }));
    }

    for handle in handles {
        handle.await.unwrap();
    }

    // Verify all 100 entries were written
    let entries = db.load_fitness_history(200).await.unwrap();
    assert_eq!(
        entries.len(),
        100,
        "all 100 fitness entries should be persisted"
    );

    // Verify generations cover the full range
    let mut generations: Vec<u64> = entries.iter().map(|e| e.generation).collect();
    generations.sort();
    generations.dedup();
    assert_eq!(
        generations.len(),
        100,
        "all 100 distinct generations should be present"
    );
}

// ===========================================================================
// Test 7: Verify engine state consistency after stress
// ===========================================================================

#[tokio::test]
async fn test_stress_state_consistency_after_load() {
    let engine = arc_engine();

    // Phase 1: Apply heavy concurrent load
    let mut handles = Vec::new();

    // 30 health reports
    for _ in 0..30 {
        let e = Arc::clone(&engine);
        handles.push(tokio::spawn(async move {
            let _ = e.health_report();
        }));
    }

    // 20 tensor builds
    for _ in 0..20 {
        let e = Arc::clone(&engine);
        handles.push(tokio::spawn(async move {
            let _ = e.build_tensor();
        }));
    }

    // 20 learning cycles
    for _ in 0..20 {
        let e = Arc::clone(&engine);
        handles.push(tokio::spawn(async move {
            let _ = e.learning_cycle();
        }));
    }

    // 15 remediations
    for i in 0_u32..15 {
        let e = Arc::clone(&engine);
        handles.push(tokio::spawn(async move {
            let _ = e.submit_remediation(
                &format!("consistency-svc-{i}"),
                IssueType::ErrorRateHigh,
                Severity::Low,
                "consistency check",
            );
        }));
    }

    // Wait for all tasks
    for handle in handles {
        handle.await.unwrap();
    }

    // Phase 2: Verify consistency
    let report = engine.health_report().unwrap();

    // Overall health should still be in valid range
    assert!(
        (0.0..=1.0).contains(&report.overall_health),
        "overall health out of range after stress: {}",
        report.overall_health
    );

    // All 7 layer scores should be valid
    for (i, &score) in report.layer_health.iter().enumerate() {
        assert!(
            (0.0..=1.0).contains(&score),
            "layer {i} health out of range: {score}"
        );
    }

    // Pipelines should remain at default count (stress should not mutate them)
    assert_eq!(
        report.pipelines_active, 8,
        "pipeline count should remain at 8 after stress"
    );

    // Pathways should still exist
    assert!(
        report.pathways_count > 0,
        "pathways should still exist after stress"
    );

    // Tensor should be valid
    let tensor = engine.build_tensor();
    assert!(
        tensor.validate().is_ok(),
        "tensor should be valid after stress"
    );

    // Remediations should all be accounted for
    assert!(
        engine.pending_remediations() >= 15,
        "at least 15 remediations should be pending, got {}",
        engine.pending_remediations()
    );

    // Consecutive health reports should be deterministic (no mutation between calls)
    let r1 = engine.health_report().unwrap();
    let r2 = engine.health_report().unwrap();
    assert!(
        (r1.overall_health - r2.overall_health).abs() < f64::EPSILON,
        "consecutive health reports should match: {} vs {}",
        r1.overall_health,
        r2.overall_health
    );
}
