//! Integration tests for the `DatabaseManager`.
//!
//! All tests use `tempfile::TempDir` for `SQLite` isolation and
//! `#[tokio::test]` for async database operations.

mod common;

use maintenance_engine::database::{
    CorrelationEntry, DatabaseManager, EmergenceEntry, FitnessHistoryEntry,
    MutationEntry, PerformanceSample, ServiceEventEntry, TensorSnapshot,
    DatabaseHealthReport,
};
use maintenance_engine::m1_foundation::state::DatabaseType;
use tempfile::TempDir;

// =========================================================================
// Helpers
// =========================================================================

fn make_fitness_entry() -> FitnessHistoryEntry {
    FitnessHistoryEntry {
        timestamp: "2026-01-29T12:00:00Z".to_string(),
        fitness: 0.95,
        system_state: "healthy".to_string(),
        tensor_hash: "abc123".to_string(),
        generation: 1,
    }
}

fn make_tensor_snapshot() -> TensorSnapshot {
    TensorSnapshot {
        timestamp: "2026-01-29T12:00:00Z".to_string(),
        dimensions: [0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 0.05, 0.01, 0.99],
        source: "fitness_evaluator".to_string(),
        tick: 42,
    }
}

fn make_emergence_entry() -> EmergenceEntry {
    EmergenceEntry {
        id: "em-001".to_string(),
        emergence_type: "cascade".to_string(),
        confidence: 0.87,
        severity: 0.4,
        detected_at: "2026-01-29T12:00:00Z".to_string(),
        description: "Cascade detected".to_string(),
    }
}

fn make_mutation_entry() -> MutationEntry {
    MutationEntry {
        id: "mut-001".to_string(),
        generation: 5,
        target_parameter: "ltp_rate".to_string(),
        original_value: 0.1,
        mutated_value: 0.12,
        applied: true,
        rolled_back: false,
        timestamp: "2026-01-29T12:00:00Z".to_string(),
    }
}

fn make_correlation_entry() -> CorrelationEntry {
    CorrelationEntry {
        id: "cor-001".to_string(),
        channel: "health".to_string(),
        event_type: "degradation".to_string(),
        link_count: 3,
        timestamp: "2026-01-29T12:00:00Z".to_string(),
    }
}

fn make_service_event() -> ServiceEventEntry {
    ServiceEventEntry {
        service_id: "synthex".to_string(),
        event_type: "health_check".to_string(),
        health_score: 0.98,
        latency_ms: 12.5,
        timestamp: "2026-01-29T12:00:00Z".to_string(),
    }
}

fn make_performance_sample() -> PerformanceSample {
    PerformanceSample {
        metric_name: "pipeline_latency".to_string(),
        value: 45.2,
        unit: "ms".to_string(),
        timestamp: "2026-01-29T12:00:00Z".to_string(),
    }
}

async fn create_full_manager() -> (DatabaseManager, TempDir) {
    let temp = TempDir::new().expect("create temp dir");
    let mgr = DatabaseManager::new(temp.path()).await.expect("create manager");
    (mgr, temp)
}

async fn create_subset_manager(db_types: &[DatabaseType]) -> (DatabaseManager, TempDir) {
    let temp = TempDir::new().expect("create temp dir");
    let mgr = DatabaseManager::with_databases(temp.path(), db_types)
        .await
        .expect("create subset manager");
    (mgr, temp)
}

// =========================================================================
// Group 1: Construction (5 tests)
// =========================================================================

#[tokio::test]
async fn test_new_creates_manager_all_databases() {
    let (mgr, _tmp) = create_full_manager().await;
    let report = mgr.health_check_all().await.expect("health check");
    assert_eq!(report.total_databases, 11);
    assert!(report.all_healthy);
}

#[tokio::test]
async fn test_with_databases_subset() {
    let types = [DatabaseType::EvolutionTracking, DatabaseType::TensorMemory];
    let (mgr, _tmp) = create_subset_manager(&types).await;
    let report = mgr.health_check_all().await.expect("health check");
    assert_eq!(report.total_databases, 2);
    assert!(report.all_healthy);
}

#[tokio::test]
async fn test_with_single_database() {
    let types = [DatabaseType::ServiceTracking];
    let (mgr, _tmp) = create_subset_manager(&types).await;
    let report = mgr.health_check_all().await.expect("health check");
    assert_eq!(report.total_databases, 1);
}

#[tokio::test]
async fn test_with_empty_database_list() {
    let types: &[DatabaseType] = &[];
    let (mgr, _tmp) = create_subset_manager(types).await;
    let report = mgr.health_check_all().await.expect("health check");
    assert_eq!(report.total_databases, 0);
    assert!(report.all_healthy);
}

#[tokio::test]
async fn test_persistence_accessor() {
    let (mgr, _tmp) = create_full_manager().await;
    let pool = mgr.persistence().pool(DatabaseType::EvolutionTracking);
    assert!(pool.is_ok());
}

// =========================================================================
// Group 2: Fitness History (6 tests)
// =========================================================================

#[tokio::test]
async fn test_write_fitness_history() {
    let (mgr, _tmp) = create_full_manager().await;
    let entry = make_fitness_entry();
    let affected = mgr.write_fitness_history(&entry).await.expect("write");
    assert_eq!(affected, 1);
}

#[tokio::test]
async fn test_fitness_history_roundtrip() {
    let (mgr, _tmp) = create_full_manager().await;
    let entry = make_fitness_entry();
    mgr.write_fitness_history(&entry).await.expect("write");
    let loaded = mgr.load_fitness_history(10).await.expect("load");
    assert_eq!(loaded.len(), 1);
    assert!((loaded[0].fitness - 0.95).abs() < f64::EPSILON);
    assert_eq!(loaded[0].system_state, "healthy");
}

#[tokio::test]
async fn test_empty_fitness_history() {
    let (mgr, _tmp) = create_full_manager().await;
    let loaded = mgr.load_fitness_history(10).await.expect("load");
    assert!(loaded.is_empty());
}

#[tokio::test]
async fn test_multiple_fitness_writes() {
    let (mgr, _tmp) = create_full_manager().await;
    for i in 0_u32..5 {
        let mut entry = make_fitness_entry();
        entry.generation = u64::from(i);
        mgr.write_fitness_history(&entry).await.expect("write");
    }
    let loaded = mgr.load_fitness_history(10).await.expect("load");
    assert_eq!(loaded.len(), 5);
}

#[tokio::test]
async fn test_fitness_history_limit() {
    let (mgr, _tmp) = create_full_manager().await;
    for i in 0_u32..10 {
        let mut entry = make_fitness_entry();
        entry.generation = u64::from(i);
        mgr.write_fitness_history(&entry).await.expect("write");
    }
    let loaded = mgr.load_fitness_history(3).await.expect("load");
    assert_eq!(loaded.len(), 3);
}

#[tokio::test]
async fn test_fitness_generation_preserved() {
    let (mgr, _tmp) = create_full_manager().await;
    let mut entry = make_fitness_entry();
    entry.generation = 999;
    mgr.write_fitness_history(&entry).await.expect("write");
    let loaded = mgr.load_fitness_history(1).await.expect("load");
    assert_eq!(loaded[0].generation, 999);
}

// =========================================================================
// Group 3: Tensor Snapshots (4 tests)
// =========================================================================

#[tokio::test]
async fn test_write_tensor_snapshot() {
    let (mgr, _tmp) = create_full_manager().await;
    let snap = make_tensor_snapshot();
    let affected = mgr.write_tensor_snapshot(&snap).await.expect("write");
    assert_eq!(affected, 1);
}

#[tokio::test]
async fn test_tensor_dimensions_preserved() {
    let types = [DatabaseType::TensorMemory];
    let (mgr, _tmp) = create_subset_manager(&types).await;
    let snap = make_tensor_snapshot();
    mgr.write_tensor_snapshot(&snap).await.expect("write");

    let pool = mgr.persistence().pool(DatabaseType::TensorMemory).expect("pool");
    let rows = sqlx::query("SELECT data FROM tensor_snapshots")
        .fetch_all(pool.inner())
        .await
        .expect("fetch");
    assert_eq!(rows.len(), 1);

    let json_str: String = sqlx::Row::try_get(&rows[0], 0).expect("get data");
    let loaded: TensorSnapshot = serde_json::from_str(&json_str).expect("parse");
    assert!((loaded.dimensions[0] - 0.1).abs() < f64::EPSILON);
    assert!((loaded.dimensions[11] - 0.99).abs() < f64::EPSILON);
}

#[tokio::test]
async fn test_tensor_source_preserved() {
    let (mgr, _tmp) = create_full_manager().await;
    let mut snap = make_tensor_snapshot();
    snap.source = "evolution_chamber".to_string();
    mgr.write_tensor_snapshot(&snap).await.expect("write");

    let pool = mgr.persistence().pool(DatabaseType::TensorMemory).expect("pool");
    let rows = sqlx::query("SELECT data FROM tensor_snapshots")
        .fetch_all(pool.inner())
        .await
        .expect("fetch");
    let json_str: String = sqlx::Row::try_get(&rows[0], 0).expect("get");
    let loaded: TensorSnapshot = serde_json::from_str(&json_str).expect("parse");
    assert_eq!(loaded.source, "evolution_chamber");
}

#[tokio::test]
async fn test_tensor_tick_preserved() {
    let (mgr, _tmp) = create_full_manager().await;
    let mut snap = make_tensor_snapshot();
    snap.tick = 12345;
    mgr.write_tensor_snapshot(&snap).await.expect("write");

    let pool = mgr.persistence().pool(DatabaseType::TensorMemory).expect("pool");
    let rows = sqlx::query("SELECT data FROM tensor_snapshots")
        .fetch_all(pool.inner())
        .await
        .expect("fetch");
    let json_str: String = sqlx::Row::try_get(&rows[0], 0).expect("get");
    let loaded: TensorSnapshot = serde_json::from_str(&json_str).expect("parse");
    assert_eq!(loaded.tick, 12345);
}

// =========================================================================
// Group 4: Emergence Events (3 tests)
// =========================================================================

#[tokio::test]
async fn test_write_emergence() {
    let (mgr, _tmp) = create_full_manager().await;
    let entry = make_emergence_entry();
    let affected = mgr.write_emergence(&entry).await.expect("write");
    assert_eq!(affected, 1);
}

#[tokio::test]
async fn test_emergence_type_preserved() {
    let (mgr, _tmp) = create_full_manager().await;
    let entry = make_emergence_entry();
    mgr.write_emergence(&entry).await.expect("write");

    let pool = mgr.persistence().pool(DatabaseType::EvolutionTracking).expect("pool");
    let rows = sqlx::query("SELECT data FROM emergence_log")
        .fetch_all(pool.inner())
        .await
        .expect("fetch");
    let json_str: String = sqlx::Row::try_get(&rows[0], 0).expect("get");
    let loaded: EmergenceEntry = serde_json::from_str(&json_str).expect("parse");
    assert_eq!(loaded.emergence_type, "cascade");
    assert!((loaded.confidence - 0.87).abs() < f64::EPSILON);
}

#[tokio::test]
async fn test_emergence_description_preserved() {
    let (mgr, _tmp) = create_full_manager().await;
    let mut entry = make_emergence_entry();
    entry.description = "Resonance in L5".to_string();
    mgr.write_emergence(&entry).await.expect("write");

    let pool = mgr.persistence().pool(DatabaseType::EvolutionTracking).expect("pool");
    let rows = sqlx::query("SELECT data FROM emergence_log")
        .fetch_all(pool.inner())
        .await
        .expect("fetch");
    let json_str: String = sqlx::Row::try_get(&rows[0], 0).expect("get");
    let loaded: EmergenceEntry = serde_json::from_str(&json_str).expect("parse");
    assert_eq!(loaded.description, "Resonance in L5");
}

// =========================================================================
// Group 5: Mutations (4 tests)
// =========================================================================

#[tokio::test]
async fn test_write_mutation() {
    let (mgr, _tmp) = create_full_manager().await;
    let entry = make_mutation_entry();
    let affected = mgr.write_mutation(&entry).await.expect("write");
    assert_eq!(affected, 1);
}

#[tokio::test]
async fn test_mutation_roundtrip() {
    let (mgr, _tmp) = create_full_manager().await;
    let entry = make_mutation_entry();
    mgr.write_mutation(&entry).await.expect("write");
    let loaded = mgr.load_recent_mutations(10).await.expect("load");
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].target_parameter, "ltp_rate");
}

#[tokio::test]
async fn test_mutation_applied_flag() {
    let (mgr, _tmp) = create_full_manager().await;
    let mut entry = make_mutation_entry();
    entry.applied = false;
    entry.rolled_back = true;
    mgr.write_mutation(&entry).await.expect("write");
    let loaded = mgr.load_recent_mutations(1).await.expect("load");
    assert!(!loaded[0].applied);
    assert!(loaded[0].rolled_back);
}

#[tokio::test]
async fn test_mutation_limit() {
    let (mgr, _tmp) = create_full_manager().await;
    for i in 0_u32..8 {
        let mut entry = make_mutation_entry();
        entry.generation = u64::from(i);
        mgr.write_mutation(&entry).await.expect("write");
    }
    let loaded = mgr.load_recent_mutations(3).await.expect("load");
    assert_eq!(loaded.len(), 3);
}

// =========================================================================
// Group 6: Correlations (3 tests)
// =========================================================================

#[tokio::test]
async fn test_write_correlation() {
    let (mgr, _tmp) = create_full_manager().await;
    let entry = make_correlation_entry();
    let affected = mgr.write_correlation(&entry).await.expect("write");
    assert_eq!(affected, 1);
}

#[tokio::test]
async fn test_correlation_channel_preserved() {
    let (mgr, _tmp) = create_full_manager().await;
    let entry = make_correlation_entry();
    mgr.write_correlation(&entry).await.expect("write");

    let pool = mgr.persistence().pool(DatabaseType::EvolutionTracking).expect("pool");
    let rows = sqlx::query("SELECT data FROM correlation_log")
        .fetch_all(pool.inner())
        .await
        .expect("fetch");
    let json_str: String = sqlx::Row::try_get(&rows[0], 0).expect("get");
    let loaded: CorrelationEntry = serde_json::from_str(&json_str).expect("parse");
    assert_eq!(loaded.channel, "health");
    assert_eq!(loaded.link_count, 3);
}

#[tokio::test]
async fn test_correlation_event_type_preserved() {
    let (mgr, _tmp) = create_full_manager().await;
    let mut entry = make_correlation_entry();
    entry.event_type = "latency_spike".to_string();
    mgr.write_correlation(&entry).await.expect("write");

    let pool = mgr.persistence().pool(DatabaseType::EvolutionTracking).expect("pool");
    let rows = sqlx::query("SELECT data FROM correlation_log")
        .fetch_all(pool.inner())
        .await
        .expect("fetch");
    let json_str: String = sqlx::Row::try_get(&rows[0], 0).expect("get");
    let loaded: CorrelationEntry = serde_json::from_str(&json_str).expect("parse");
    assert_eq!(loaded.event_type, "latency_spike");
}

// =========================================================================
// Group 7: Service Events (4 tests)
// =========================================================================

#[tokio::test]
async fn test_write_service_event() {
    let (mgr, _tmp) = create_full_manager().await;
    let entry = make_service_event();
    let affected = mgr.write_service_event(&entry).await.expect("write");
    assert_eq!(affected, 1);
}

#[tokio::test]
async fn test_service_event_roundtrip() {
    let (mgr, _tmp) = create_full_manager().await;
    let entry = make_service_event();
    mgr.write_service_event(&entry).await.expect("write");
    let loaded = mgr.load_service_health().await.expect("load");
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].service_id, "synthex");
    assert!((loaded[0].health_score - 0.98).abs() < f64::EPSILON);
}

#[tokio::test]
async fn test_service_latency_preserved() {
    let (mgr, _tmp) = create_full_manager().await;
    let mut entry = make_service_event();
    entry.latency_ms = 250.75;
    mgr.write_service_event(&entry).await.expect("write");
    let loaded = mgr.load_service_health().await.expect("load");
    assert!((loaded[0].latency_ms - 250.75).abs() < f64::EPSILON);
}

#[tokio::test]
async fn test_multiple_service_events() {
    let (mgr, _tmp) = create_full_manager().await;
    for svc in &["synthex", "san-k7", "nais", "codesynthor"] {
        let mut entry = make_service_event();
        entry.service_id = (*svc).to_string();
        mgr.write_service_event(&entry).await.expect("write");
    }
    let loaded = mgr.load_service_health().await.expect("load");
    assert_eq!(loaded.len(), 4);
}

// =========================================================================
// Group 8: Performance Samples (3 tests)
// =========================================================================

#[tokio::test]
async fn test_write_performance_sample() {
    let (mgr, _tmp) = create_full_manager().await;
    let sample = make_performance_sample();
    let affected = mgr.write_performance_sample(&sample).await.expect("write");
    assert_eq!(affected, 1);
}

#[tokio::test]
async fn test_performance_unit_preserved() {
    let (mgr, _tmp) = create_full_manager().await;
    let mut sample = make_performance_sample();
    sample.unit = "bytes".to_string();
    mgr.write_performance_sample(&sample).await.expect("write");

    let pool = mgr.persistence().pool(DatabaseType::PerformanceMetrics).expect("pool");
    let rows = sqlx::query("SELECT data FROM performance_samples")
        .fetch_all(pool.inner())
        .await
        .expect("fetch");
    let json_str: String = sqlx::Row::try_get(&rows[0], 0).expect("get");
    let loaded: PerformanceSample = serde_json::from_str(&json_str).expect("parse");
    assert_eq!(loaded.unit, "bytes");
}

#[tokio::test]
async fn test_multiple_performance_samples() {
    let (mgr, _tmp) = create_full_manager().await;
    for i in 0..6 {
        let mut sample = make_performance_sample();
        sample.value = f64::from(i) * 10.0;
        mgr.write_performance_sample(&sample).await.expect("write");
    }
    let pool = mgr.persistence().pool(DatabaseType::PerformanceMetrics).expect("pool");
    let rows = sqlx::query("SELECT data FROM performance_samples")
        .fetch_all(pool.inner())
        .await
        .expect("fetch");
    assert_eq!(rows.len(), 6);
}

// =========================================================================
// Group 9: Health Checks (4 tests)
// =========================================================================

#[tokio::test]
async fn test_health_check_all_healthy() {
    let (mgr, _tmp) = create_full_manager().await;
    let report = mgr.health_check_all().await.expect("check");
    assert!(report.all_healthy);
    assert_eq!(report.total_databases, 11);
    assert_eq!(report.healthy_databases, 11);
}

#[tokio::test]
async fn test_health_check_has_timestamp() {
    let (mgr, _tmp) = create_full_manager().await;
    let report = mgr.health_check_all().await.expect("check");
    assert!(!report.checked_at.is_empty());
}

#[tokio::test]
async fn test_health_check_subset() {
    let types = [DatabaseType::EvolutionTracking, DatabaseType::ServiceTracking];
    let (mgr, _tmp) = create_subset_manager(&types).await;
    let report = mgr.health_check_all().await.expect("check");
    assert_eq!(report.total_databases, 2);
    assert!(report.all_healthy);
}

#[tokio::test]
async fn test_health_report_serialization() {
    let report = DatabaseHealthReport {
        total_databases: 11,
        healthy_databases: 11,
        all_healthy: true,
        checked_at: "2026-01-29T12:00:00Z".to_string(),
    };
    let json = serde_json::to_string(&report).expect("serialize");
    let deser: DatabaseHealthReport = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(deser.total_databases, 11);
    assert!(deser.all_healthy);
}

// =========================================================================
// Group 10: Edge Cases (4 tests)
// =========================================================================

#[tokio::test]
async fn test_write_to_subset_fails_for_missing_pool() {
    let types = [DatabaseType::EvolutionTracking];
    let (mgr, _tmp) = create_subset_manager(&types).await;

    // Evolution tracking should work
    let entry = make_fitness_entry();
    assert!(mgr.write_fitness_history(&entry).await.is_ok());

    // Service tracking should fail
    let svc = make_service_event();
    assert!(mgr.write_service_event(&svc).await.is_err());
}

#[tokio::test]
async fn test_data_types_serde_roundtrip() {
    let fitness_json = serde_json::to_string(&make_fitness_entry()).expect("ser");
    let _: FitnessHistoryEntry = serde_json::from_str(&fitness_json).expect("deser");

    let tensor_json = serde_json::to_string(&make_tensor_snapshot()).expect("ser");
    let _: TensorSnapshot = serde_json::from_str(&tensor_json).expect("deser");

    let emergence_json = serde_json::to_string(&make_emergence_entry()).expect("ser");
    let _: EmergenceEntry = serde_json::from_str(&emergence_json).expect("deser");

    let mutation_json = serde_json::to_string(&make_mutation_entry()).expect("ser");
    let _: MutationEntry = serde_json::from_str(&mutation_json).expect("deser");
}

#[tokio::test]
async fn test_concurrent_writes() {
    let (mgr, _tmp) = create_full_manager().await;
    let mgr = std::sync::Arc::new(mgr);

    let mut handles = Vec::new();
    for i in 0_u32..10 {
        let mgr_clone = std::sync::Arc::clone(&mgr);
        handles.push(tokio::spawn(async move {
            let mut entry = make_fitness_entry();
            entry.generation = u64::from(i);
            mgr_clone.write_fitness_history(&entry).await.expect("write");
        }));
    }
    for handle in handles {
        handle.await.expect("join");
    }

    let loaded = mgr.load_fitness_history(20).await.expect("load");
    assert_eq!(loaded.len(), 10);
}

#[tokio::test]
async fn test_empty_mutations_read() {
    let (mgr, _tmp) = create_full_manager().await;
    let loaded = mgr.load_recent_mutations(10).await.expect("load");
    assert!(loaded.is_empty());
}
