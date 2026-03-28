#![allow(clippy::unwrap_used)]

//! # G2 Persistence Integration Tests
//!
//! Validates all [`DatabaseManager`] write/read operations, concurrency
//! safety, and edge cases across the 11-database backend.
//!
//! Coverage:
//! 1. Write and read fitness history (multiple entries)
//! 2. Write and read tensor snapshots
//! 3. Write emergence events
//! 4. Write service events, read back via `read_service_events_since()`
//! 5. Write correlation entries
//! 6. Write performance samples
//! 7. DatabaseManager handles concurrent writes (spawn multiple tasks)
//! 8. `read_latest_fitness` returns the most recent entry
//! 9. `load_fitness_history` with different limits

mod common;

use std::sync::Arc;

use maintenance_engine::database::{
    CorrelationEntry, DatabaseManager, EmergenceEntry, FitnessHistoryEntry,
    PerformanceSample, ServiceEventEntry, TensorSnapshot,
};
use tempfile::TempDir;

// =========================================================================
// Helpers
// =========================================================================

async fn make_db() -> (DatabaseManager, TempDir) {
    let tmp = TempDir::new().unwrap();
    let mgr = DatabaseManager::new(tmp.path()).await.unwrap();
    (mgr, tmp)
}

fn make_fitness(generation: u64, fitness: f64, state: &str) -> FitnessHistoryEntry {
    FitnessHistoryEntry {
        timestamp: format!("2026-01-30T10:{generation:02}:00Z"),
        fitness,
        system_state: state.to_string(),
        tensor_hash: format!("hash-{generation}"),
        generation,
    }
}

fn make_tensor_snapshot(tick: u64) -> TensorSnapshot {
    TensorSnapshot {
        timestamp: format!("2026-01-30T10:{tick:02}:00Z"),
        dimensions: [0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 0.05, 0.01, 0.99],
        source: "g2_test".to_string(),
        tick,
    }
}

fn make_emergence(id: &str, etype: &str) -> EmergenceEntry {
    EmergenceEntry {
        id: id.to_string(),
        emergence_type: etype.to_string(),
        confidence: 0.85,
        severity: 0.4,
        detected_at: "2026-01-30T10:00:00Z".to_string(),
        description: format!("{etype} emergence in test"),
    }
}

fn make_service_event(service_id: &str, score: f64) -> ServiceEventEntry {
    ServiceEventEntry {
        service_id: service_id.to_string(),
        event_type: "health_check".to_string(),
        health_score: score,
        latency_ms: 15.0,
        timestamp: "2026-01-30T10:00:00Z".to_string(),
    }
}

fn make_correlation(id: &str, channel: &str) -> CorrelationEntry {
    CorrelationEntry {
        id: id.to_string(),
        channel: channel.to_string(),
        event_type: "degradation".to_string(),
        link_count: 3,
        timestamp: "2026-01-30T10:00:00Z".to_string(),
    }
}

fn make_performance(metric: &str, value: f64) -> PerformanceSample {
    PerformanceSample {
        metric_name: metric.to_string(),
        value,
        unit: "ms".to_string(),
        timestamp: "2026-01-30T10:00:00Z".to_string(),
    }
}

// =========================================================================
// 1. Write and read fitness history (multiple entries)
// =========================================================================

#[tokio::test]
async fn g2_write_multiple_fitness_entries() {
    let (mgr, _tmp) = make_db().await;

    for i in 0_u64..10 {
        let entry = make_fitness(i, 0.5 + (i as f64) * 0.05, "healthy");
        mgr.write_fitness_history(&entry).await.unwrap();
    }

    let loaded = mgr.load_fitness_history(20).await.unwrap();
    assert_eq!(loaded.len(), 10, "should load all 10 entries");
}

#[tokio::test]
async fn g2_fitness_entries_ordered_newest_first() {
    let (mgr, _tmp) = make_db().await;

    for i in 0_u64..5 {
        let entry = make_fitness(i, 0.5 + (i as f64) * 0.1, "healthy");
        mgr.write_fitness_history(&entry).await.unwrap();
    }

    let loaded = mgr.load_fitness_history(10).await.unwrap();
    assert_eq!(loaded.len(), 5);
    // Newest first means highest generation should be first
    assert_eq!(
        loaded[0].generation, 4,
        "newest entry (gen 4) should be first"
    );
}

#[tokio::test]
async fn g2_fitness_history_preserves_all_fields() {
    let (mgr, _tmp) = make_db().await;

    let entry = FitnessHistoryEntry {
        timestamp: "2026-01-30T12:34:56Z".to_string(),
        fitness: 0.876,
        system_state: "degraded".to_string(),
        tensor_hash: "deadbeef1234".to_string(),
        generation: 42,
    };
    mgr.write_fitness_history(&entry).await.unwrap();

    let loaded = mgr.load_fitness_history(1).await.unwrap();
    assert_eq!(loaded.len(), 1);
    let got = &loaded[0];
    assert!((got.fitness - 0.876).abs() < f64::EPSILON);
    assert_eq!(got.system_state, "degraded");
    assert_eq!(got.tensor_hash, "deadbeef1234");
    assert_eq!(got.generation, 42);
    assert_eq!(got.timestamp, "2026-01-30T12:34:56Z");
}

// =========================================================================
// 2. Write and read tensor snapshots
// =========================================================================

#[tokio::test]
async fn g2_write_tensor_snapshot() {
    let (mgr, _tmp) = make_db().await;

    let snap = make_tensor_snapshot(1);
    let affected = mgr.write_tensor_snapshot(&snap).await.unwrap();
    assert_eq!(affected, 1);
}

#[tokio::test]
async fn g2_write_multiple_tensor_snapshots() {
    let (mgr, _tmp) = make_db().await;

    for tick in 0_u64..7 {
        let snap = make_tensor_snapshot(tick);
        mgr.write_tensor_snapshot(&snap).await.unwrap();
    }

    // Verify via raw count (no typed read for tensor_snapshots)
    let pool = mgr
        .persistence()
        .pool(maintenance_engine::m1_foundation::state::DatabaseType::TensorMemory)
        .unwrap();
    let rows = sqlx::query("SELECT COUNT(*) as cnt FROM tensor_snapshots")
        .fetch_one(pool.inner())
        .await
        .unwrap();
    let count: i64 = sqlx::Row::try_get(&rows, 0).unwrap();
    assert_eq!(count, 7, "should have 7 tensor snapshots");
}

#[tokio::test]
async fn g2_tensor_snapshot_dimensions_roundtrip() {
    let (mgr, _tmp) = make_db().await;

    let mut snap = make_tensor_snapshot(99);
    snap.dimensions = [0.0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0, 0.55];
    snap.source = "evolution_chamber".to_string();
    mgr.write_tensor_snapshot(&snap).await.unwrap();

    let pool = mgr
        .persistence()
        .pool(maintenance_engine::m1_foundation::state::DatabaseType::TensorMemory)
        .unwrap();
    let rows = sqlx::query("SELECT data FROM tensor_snapshots")
        .fetch_all(pool.inner())
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    let json_str: String = sqlx::Row::try_get(&rows[0], 0).unwrap();
    let loaded: TensorSnapshot = serde_json::from_str(&json_str).unwrap();
    assert!((loaded.dimensions[0] - 0.0).abs() < f64::EPSILON);
    assert!((loaded.dimensions[10] - 1.0).abs() < f64::EPSILON);
    assert!((loaded.dimensions[11] - 0.55).abs() < f64::EPSILON);
    assert_eq!(loaded.source, "evolution_chamber");
    assert_eq!(loaded.tick, 99);
}

// =========================================================================
// 3. Write emergence events
// =========================================================================

#[tokio::test]
async fn g2_write_emergence_event() {
    let (mgr, _tmp) = make_db().await;

    let entry = make_emergence("em-001", "cascade");
    let affected = mgr.write_emergence(&entry).await.unwrap();
    assert_eq!(affected, 1);
}

#[tokio::test]
async fn g2_write_multiple_emergence_types() {
    let (mgr, _tmp) = make_db().await;

    let types = ["cascade", "synergy", "resonance", "phase"];
    for (i, etype) in types.iter().enumerate() {
        let entry = make_emergence(&format!("em-{i:03}"), etype);
        mgr.write_emergence(&entry).await.unwrap();
    }

    let pool = mgr
        .persistence()
        .pool(maintenance_engine::m1_foundation::state::DatabaseType::EvolutionTracking)
        .unwrap();
    let rows = sqlx::query("SELECT data FROM emergence_log")
        .fetch_all(pool.inner())
        .await
        .unwrap();
    assert_eq!(rows.len(), 4, "should have 4 emergence events");

    // Verify the types are preserved
    for row in &rows {
        let json_str: String = sqlx::Row::try_get(row, 0).unwrap();
        let loaded: EmergenceEntry = serde_json::from_str(&json_str).unwrap();
        assert!(
            types.contains(&loaded.emergence_type.as_str()),
            "unexpected emergence type: {}",
            loaded.emergence_type
        );
    }
}

#[tokio::test]
async fn g2_emergence_confidence_and_severity_preserved() {
    let (mgr, _tmp) = make_db().await;

    let mut entry = make_emergence("em-custom", "resonance");
    entry.confidence = 0.99;
    entry.severity = 0.75;
    entry.description = "High-confidence resonance pattern".to_string();
    mgr.write_emergence(&entry).await.unwrap();

    let pool = mgr
        .persistence()
        .pool(maintenance_engine::m1_foundation::state::DatabaseType::EvolutionTracking)
        .unwrap();
    let rows = sqlx::query("SELECT data FROM emergence_log")
        .fetch_all(pool.inner())
        .await
        .unwrap();
    let json_str: String = sqlx::Row::try_get(&rows[0], 0).unwrap();
    let loaded: EmergenceEntry = serde_json::from_str(&json_str).unwrap();
    assert!((loaded.confidence - 0.99).abs() < f64::EPSILON);
    assert!((loaded.severity - 0.75).abs() < f64::EPSILON);
    assert_eq!(loaded.description, "High-confidence resonance pattern");
}

// =========================================================================
// 4. Write service events, read back via read_service_events_since()
// =========================================================================

#[tokio::test]
async fn g2_write_service_event() {
    let (mgr, _tmp) = make_db().await;

    let entry = make_service_event("synthex", 0.98);
    let affected = mgr.write_service_event(&entry).await.unwrap();
    assert_eq!(affected, 1);
}

#[tokio::test]
async fn g2_write_and_read_service_events() {
    let (mgr, _tmp) = make_db().await;

    let services = ["synthex", "san-k7", "nais", "codesynthor-v7"];
    for svc in &services {
        let entry = make_service_event(svc, 0.95);
        mgr.write_service_event(&entry).await.unwrap();
    }

    let loaded = mgr.load_service_health().await.unwrap();
    assert_eq!(loaded.len(), 4, "should have 4 service events");

    // All service IDs should be present
    let loaded_ids: Vec<&str> = loaded.iter().map(|e| e.service_id.as_str()).collect();
    for svc in &services {
        assert!(
            loaded_ids.contains(svc),
            "service {svc} should be in loaded events"
        );
    }
}

#[tokio::test]
async fn g2_read_service_events_since_filters_correctly() {
    let (mgr, _tmp) = make_db().await;

    // Write events -- they all get created_at = datetime('now') from SQLite
    for svc in &["synthex", "san-k7", "nais"] {
        let entry = make_service_event(svc, 0.95);
        mgr.write_service_event(&entry).await.unwrap();
    }

    // Reading with a very old date should return all events
    let all = mgr
        .read_service_events_since("2020-01-01T00:00:00")
        .await
        .unwrap();
    assert_eq!(all.len(), 3, "should get all 3 events since 2020");

    // Reading with a far future date should return no events
    let none = mgr
        .read_service_events_since("2099-01-01T00:00:00")
        .await
        .unwrap();
    assert!(
        none.is_empty(),
        "should get 0 events since far future, got {}",
        none.len()
    );
}

#[tokio::test]
async fn g2_service_event_latency_preserved() {
    let (mgr, _tmp) = make_db().await;

    let mut entry = make_service_event("tool-library", 0.88);
    entry.latency_ms = 350.25;
    mgr.write_service_event(&entry).await.unwrap();

    let loaded = mgr.load_service_health().await.unwrap();
    assert_eq!(loaded.len(), 1);
    assert!((loaded[0].latency_ms - 350.25).abs() < f64::EPSILON);
    assert_eq!(loaded[0].service_id, "tool-library");
}

// =========================================================================
// 5. Write correlation entries
// =========================================================================

#[tokio::test]
async fn g2_write_correlation_entry() {
    let (mgr, _tmp) = make_db().await;

    let entry = make_correlation("cor-001", "health");
    let affected = mgr.write_correlation(&entry).await.unwrap();
    assert_eq!(affected, 1);
}

#[tokio::test]
async fn g2_write_multiple_correlations() {
    let (mgr, _tmp) = make_db().await;

    let channels = ["health", "metrics", "remediation", "consensus"];
    for (i, ch) in channels.iter().enumerate() {
        let entry = make_correlation(&format!("cor-{i:03}"), ch);
        mgr.write_correlation(&entry).await.unwrap();
    }

    let pool = mgr
        .persistence()
        .pool(maintenance_engine::m1_foundation::state::DatabaseType::EvolutionTracking)
        .unwrap();
    let rows = sqlx::query("SELECT data FROM correlation_log")
        .fetch_all(pool.inner())
        .await
        .unwrap();
    assert_eq!(rows.len(), 4, "should have 4 correlation entries");
}

#[tokio::test]
async fn g2_correlation_fields_preserved() {
    let (mgr, _tmp) = make_db().await;

    let mut entry = make_correlation("cor-custom", "latency");
    entry.event_type = "spike_detected".to_string();
    entry.link_count = 7;
    mgr.write_correlation(&entry).await.unwrap();

    let pool = mgr
        .persistence()
        .pool(maintenance_engine::m1_foundation::state::DatabaseType::EvolutionTracking)
        .unwrap();
    let rows = sqlx::query("SELECT data FROM correlation_log")
        .fetch_all(pool.inner())
        .await
        .unwrap();
    let json_str: String = sqlx::Row::try_get(&rows[0], 0).unwrap();
    let loaded: CorrelationEntry = serde_json::from_str(&json_str).unwrap();
    assert_eq!(loaded.id, "cor-custom");
    assert_eq!(loaded.channel, "latency");
    assert_eq!(loaded.event_type, "spike_detected");
    assert_eq!(loaded.link_count, 7);
}

// =========================================================================
// 6. Write performance samples
// =========================================================================

#[tokio::test]
async fn g2_write_performance_sample() {
    let (mgr, _tmp) = make_db().await;

    let sample = make_performance("pipeline_latency", 45.2);
    let affected = mgr.write_performance_sample(&sample).await.unwrap();
    assert_eq!(affected, 1);
}

#[tokio::test]
async fn g2_write_multiple_performance_samples() {
    let (mgr, _tmp) = make_db().await;

    let metrics = [
        ("pipeline_latency", 42.0),
        ("consensus_time", 1200.0),
        ("hebbian_decay", 0.001),
        ("tensor_encode", 5.5),
        ("health_check", 12.0),
    ];

    for (name, value) in &metrics {
        let sample = make_performance(name, *value);
        mgr.write_performance_sample(&sample).await.unwrap();
    }

    let pool = mgr
        .persistence()
        .pool(maintenance_engine::m1_foundation::state::DatabaseType::PerformanceMetrics)
        .unwrap();
    let rows = sqlx::query("SELECT data FROM performance_samples")
        .fetch_all(pool.inner())
        .await
        .unwrap();
    assert_eq!(rows.len(), 5, "should have 5 performance samples");
}

#[tokio::test]
async fn g2_performance_sample_fields_preserved() {
    let (mgr, _tmp) = make_db().await;

    let mut sample = make_performance("memory_usage", 1024.5);
    sample.unit = "bytes".to_string();
    mgr.write_performance_sample(&sample).await.unwrap();

    let pool = mgr
        .persistence()
        .pool(maintenance_engine::m1_foundation::state::DatabaseType::PerformanceMetrics)
        .unwrap();
    let rows = sqlx::query("SELECT data FROM performance_samples")
        .fetch_all(pool.inner())
        .await
        .unwrap();
    let json_str: String = sqlx::Row::try_get(&rows[0], 0).unwrap();
    let loaded: PerformanceSample = serde_json::from_str(&json_str).unwrap();
    assert_eq!(loaded.metric_name, "memory_usage");
    assert!((loaded.value - 1024.5).abs() < f64::EPSILON);
    assert_eq!(loaded.unit, "bytes");
}

// =========================================================================
// 7. DatabaseManager handles concurrent writes
// =========================================================================

#[tokio::test]
async fn g2_concurrent_fitness_writes() {
    let (mgr, _tmp) = make_db().await;
    let mgr = Arc::new(mgr);

    let mut handles = Vec::new();
    for i in 0_u64..20 {
        let mgr_clone = Arc::clone(&mgr);
        handles.push(tokio::spawn(async move {
            let entry = make_fitness(i, 0.5 + (i as f64) * 0.02, "healthy");
            mgr_clone
                .write_fitness_history(&entry)
                .await
                .unwrap();
        }));
    }

    for handle in handles {
        handle.await.unwrap();
    }

    let loaded = mgr.load_fitness_history(30).await.unwrap();
    assert_eq!(loaded.len(), 20, "all 20 concurrent writes should persist");
}

#[tokio::test]
async fn g2_concurrent_mixed_table_writes() {
    let (mgr, _tmp) = make_db().await;
    let mgr = Arc::new(mgr);

    let mut handles = Vec::new();

    // Spawn fitness writes
    for i in 0_u64..5 {
        let m = Arc::clone(&mgr);
        handles.push(tokio::spawn(async move {
            let entry = make_fitness(i, 0.9, "healthy");
            m.write_fitness_history(&entry).await.unwrap();
        }));
    }

    // Spawn service event writes
    for i in 0_u64..5 {
        let m = Arc::clone(&mgr);
        handles.push(tokio::spawn(async move {
            let entry = make_service_event(&format!("svc-{i}"), 0.95);
            m.write_service_event(&entry).await.unwrap();
        }));
    }

    // Spawn emergence writes
    for i in 0_u64..5 {
        let m = Arc::clone(&mgr);
        handles.push(tokio::spawn(async move {
            let entry = make_emergence(&format!("em-{i}"), "cascade");
            m.write_emergence(&entry).await.unwrap();
        }));
    }

    for handle in handles {
        handle.await.unwrap();
    }

    // Verify all tables
    let fitness = mgr.load_fitness_history(10).await.unwrap();
    assert_eq!(fitness.len(), 5, "should have 5 fitness entries");

    let events = mgr.load_service_health().await.unwrap();
    assert_eq!(events.len(), 5, "should have 5 service events");

    let pool = mgr
        .persistence()
        .pool(maintenance_engine::m1_foundation::state::DatabaseType::EvolutionTracking)
        .unwrap();
    let rows = sqlx::query("SELECT COUNT(*) FROM emergence_log")
        .fetch_one(pool.inner())
        .await
        .unwrap();
    let emergence_count: i64 = sqlx::Row::try_get(&rows, 0).unwrap();
    assert_eq!(emergence_count, 5, "should have 5 emergence events");
}

#[tokio::test]
async fn g2_concurrent_writes_to_same_table_no_data_loss() {
    let (mgr, _tmp) = make_db().await;
    let mgr = Arc::new(mgr);

    let task_count = 50_u64;
    let mut handles = Vec::new();

    for i in 0..task_count {
        let m = Arc::clone(&mgr);
        handles.push(tokio::spawn(async move {
            let entry = make_fitness(i, 0.5 + (i as f64) * 0.01, "healthy");
            m.write_fitness_history(&entry).await.unwrap();
        }));
    }

    for handle in handles {
        handle.await.unwrap();
    }

    let loaded = mgr.load_fitness_history(100).await.unwrap();
    assert_eq!(
        loaded.len(),
        task_count as usize,
        "no data loss under concurrent writes: expected {task_count}, got {}",
        loaded.len()
    );
}

// =========================================================================
// 8. read_latest_fitness returns the most recent entry
// =========================================================================

#[tokio::test]
async fn g2_read_latest_fitness_empty_table() {
    let (mgr, _tmp) = make_db().await;

    let latest = mgr.read_latest_fitness().await.unwrap();
    assert!(latest.is_none(), "should return None for empty table");
}

#[tokio::test]
async fn g2_read_latest_fitness_single_entry() {
    let (mgr, _tmp) = make_db().await;

    let entry = make_fitness(1, 0.88, "healthy");
    mgr.write_fitness_history(&entry).await.unwrap();

    let latest = mgr.read_latest_fitness().await.unwrap().unwrap();
    assert_eq!(latest.generation, 1);
    assert!((latest.fitness - 0.88).abs() < f64::EPSILON);
}

#[tokio::test]
async fn g2_read_latest_fitness_returns_newest() {
    let (mgr, _tmp) = make_db().await;

    // Write entries in order
    for i in 0_u64..10 {
        let entry = make_fitness(i, 0.5 + (i as f64) * 0.05, "healthy");
        mgr.write_fitness_history(&entry).await.unwrap();
    }

    let latest = mgr.read_latest_fitness().await.unwrap().unwrap();
    // The last written entry (gen=9) should be the most recent
    assert_eq!(latest.generation, 9);
    assert!((latest.fitness - 0.95).abs() < f64::EPSILON);
}

#[tokio::test]
async fn g2_read_latest_fitness_after_multiple_writes() {
    let (mgr, _tmp) = make_db().await;

    // Write some entries
    mgr.write_fitness_history(&make_fitness(1, 0.7, "degraded"))
        .await
        .unwrap();
    mgr.write_fitness_history(&make_fitness(2, 0.8, "healthy"))
        .await
        .unwrap();
    mgr.write_fitness_history(&make_fitness(3, 0.9, "optimal"))
        .await
        .unwrap();

    let latest = mgr.read_latest_fitness().await.unwrap().unwrap();
    assert_eq!(latest.generation, 3);
    assert_eq!(latest.system_state, "optimal");
}

// =========================================================================
// 9. load_fitness_history with different limits
// =========================================================================

#[tokio::test]
async fn g2_load_fitness_history_limit_zero() {
    let (mgr, _tmp) = make_db().await;

    for i in 0_u64..5 {
        mgr.write_fitness_history(&make_fitness(i, 0.9, "healthy"))
            .await
            .unwrap();
    }

    let loaded = mgr.load_fitness_history(0).await.unwrap();
    assert!(
        loaded.is_empty(),
        "limit=0 should return empty, got {}",
        loaded.len()
    );
}

#[tokio::test]
async fn g2_load_fitness_history_limit_one() {
    let (mgr, _tmp) = make_db().await;

    for i in 0_u64..5 {
        mgr.write_fitness_history(&make_fitness(i, 0.9, "healthy"))
            .await
            .unwrap();
    }

    let loaded = mgr.load_fitness_history(1).await.unwrap();
    assert_eq!(loaded.len(), 1, "limit=1 should return exactly 1");
    // Should be the most recent (gen=4)
    assert_eq!(loaded[0].generation, 4);
}

#[tokio::test]
async fn g2_load_fitness_history_limit_exact_count() {
    let (mgr, _tmp) = make_db().await;

    for i in 0_u64..5 {
        mgr.write_fitness_history(&make_fitness(i, 0.9, "healthy"))
            .await
            .unwrap();
    }

    let loaded = mgr.load_fitness_history(5).await.unwrap();
    assert_eq!(loaded.len(), 5, "limit=5 should return all 5");
}

#[tokio::test]
async fn g2_load_fitness_history_limit_exceeds_count() {
    let (mgr, _tmp) = make_db().await;

    for i in 0_u64..3 {
        mgr.write_fitness_history(&make_fitness(i, 0.9, "healthy"))
            .await
            .unwrap();
    }

    let loaded = mgr.load_fitness_history(100).await.unwrap();
    assert_eq!(
        loaded.len(),
        3,
        "limit=100 with 3 entries should return 3"
    );
}

#[tokio::test]
async fn g2_load_fitness_history_limit_partial() {
    let (mgr, _tmp) = make_db().await;

    for i in 0_u64..10 {
        mgr.write_fitness_history(&make_fitness(i, 0.5 + (i as f64) * 0.05, "healthy"))
            .await
            .unwrap();
    }

    // Request exactly 5 out of 10
    let loaded = mgr.load_fitness_history(5).await.unwrap();
    assert_eq!(loaded.len(), 5, "limit=5 should return exactly 5");

    // Should be the 5 most recent (gen 5..9)
    for entry in &loaded {
        assert!(
            entry.generation >= 5,
            "entry gen {} should be >= 5 (most recent 5 of 10)",
            entry.generation
        );
    }
}

#[tokio::test]
async fn g2_load_fitness_history_different_limits_consistent() {
    let (mgr, _tmp) = make_db().await;

    for i in 0_u64..20 {
        mgr.write_fitness_history(&make_fitness(i, 0.9, "healthy"))
            .await
            .unwrap();
    }

    let limit_3 = mgr.load_fitness_history(3).await.unwrap();
    let limit_10 = mgr.load_fitness_history(10).await.unwrap();
    let limit_20 = mgr.load_fitness_history(20).await.unwrap();

    assert_eq!(limit_3.len(), 3);
    assert_eq!(limit_10.len(), 10);
    assert_eq!(limit_20.len(), 20);

    // The first entry of each result should be the same (most recent)
    assert_eq!(limit_3[0].generation, limit_10[0].generation);
    assert_eq!(limit_10[0].generation, limit_20[0].generation);
}
