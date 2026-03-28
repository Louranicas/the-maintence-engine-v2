#![allow(clippy::unwrap_used)]

//! # G1 Wiring Integration Tests
//!
//! Validates the top-level wiring between the [`Engine`], [`DatabaseManager`],
//! [`ObserverLayer`], and [`PeerBridgeManager`].
//!
//! Coverage:
//! 1. Engine construction and health report
//! 2. DatabaseManager initialization via `new_optional`
//! 3. DatabaseManager write/read fitness history
//! 4. Engine `build_tensor` produces valid tensors
//! 5. Observer `tick` returns valid observation report
//! 6. Fitness can be persisted after observer tick
//! 7. Engine `peer_bridge()` returns `Some` when available
//! 8. Peer bridge `mesh_summary()` returns data
//! 9. Combined: tick -> persist fitness -> read back = consistent

mod common;

use maintenance_engine::database::{DatabaseManager, FitnessHistoryEntry};
use maintenance_engine::engine::Engine;
use maintenance_engine::m7_observer::SystemState;
use maintenance_engine::Tensor12D;
use tempfile::TempDir;

// =========================================================================
// Helpers
// =========================================================================

fn make_engine() -> Engine {
    Engine::new()
}

async fn make_db() -> (DatabaseManager, TempDir) {
    let tmp = TempDir::new().unwrap();
    let mgr = DatabaseManager::new(tmp.path()).await.unwrap();
    (mgr, tmp)
}

fn make_fitness_entry(fitness: f64, state: &str, generation: u64) -> FitnessHistoryEntry {
    FitnessHistoryEntry {
        timestamp: "2026-01-30T10:00:00Z".to_string(),
        fitness,
        system_state: state.to_string(),
        tensor_hash: format!("hash-gen-{generation}"),
        generation,
    }
}

// =========================================================================
// 1. Engine construction and health report
// =========================================================================

#[test]
fn g1_engine_constructs_successfully() {
    let engine = make_engine();
    assert!(engine.service_count() < 10_000, "sanity check on service count");
}

#[test]
fn g1_engine_health_report_returns_ok() {
    let engine = make_engine();
    let report = engine.health_report();
    assert!(report.is_ok(), "health_report should return Ok");
}

#[test]
fn g1_engine_health_report_overall_in_range() {
    let engine = make_engine();
    let report = engine.health_report().unwrap();
    assert!(
        (0.0..=1.0).contains(&report.overall_health),
        "overall_health should be in [0,1], got {}",
        report.overall_health
    );
}

#[test]
fn g1_engine_health_report_has_seven_layers() {
    let engine = make_engine();
    let report = engine.health_report().unwrap();
    assert_eq!(
        report.layer_health.len(),
        7,
        "engine should report 7 layer health scores"
    );
}

#[test]
fn g1_engine_health_report_is_healthy() {
    let engine = make_engine();
    let report = engine.health_report().unwrap();
    // A freshly constructed engine should be healthy
    assert!(
        report.is_healthy(),
        "fresh engine should be healthy, overall_health={}",
        report.overall_health
    );
}

// =========================================================================
// 2. DatabaseManager initialization via new_optional
// =========================================================================

#[tokio::test]
async fn g1_database_manager_new_optional_returns_some() {
    let tmp = TempDir::new().unwrap();
    let mgr = DatabaseManager::new_optional(tmp.path()).await;
    assert!(
        mgr.is_some(),
        "new_optional should return Some for a valid temp directory"
    );
}

#[tokio::test]
async fn g1_database_manager_new_optional_health_check() {
    let tmp = TempDir::new().unwrap();
    let mgr = DatabaseManager::new_optional(tmp.path()).await.unwrap();
    let report = mgr.health_check_all().await.unwrap();
    assert!(report.all_healthy, "all databases should be healthy after init");
    assert_eq!(report.total_databases, 11, "should have 11 databases");
}

// =========================================================================
// 3. DatabaseManager write and read fitness history
// =========================================================================

#[tokio::test]
async fn g1_db_write_and_read_fitness_history() {
    let (mgr, _tmp) = make_db().await;

    let entry = make_fitness_entry(0.92, "healthy", 1);
    let affected = mgr.write_fitness_history(&entry).await.unwrap();
    assert_eq!(affected, 1, "write should affect 1 row");

    let loaded = mgr.load_fitness_history(10).await.unwrap();
    assert_eq!(loaded.len(), 1, "should load 1 entry");
    assert!((loaded[0].fitness - 0.92).abs() < f64::EPSILON);
    assert_eq!(loaded[0].system_state, "healthy");
    assert_eq!(loaded[0].generation, 1);
}

#[tokio::test]
async fn g1_db_read_latest_fitness_returns_most_recent() {
    let (mgr, _tmp) = make_db().await;

    mgr.write_fitness_history(&make_fitness_entry(0.80, "degraded", 1))
        .await
        .unwrap();
    mgr.write_fitness_history(&make_fitness_entry(0.95, "optimal", 2))
        .await
        .unwrap();

    let latest = mgr.read_latest_fitness().await.unwrap();
    assert!(latest.is_some(), "should have a latest entry");
    let latest = latest.unwrap();
    // Latest = newest first (ORDER BY created_at DESC)
    assert_eq!(latest.generation, 2);
    assert!((latest.fitness - 0.95).abs() < f64::EPSILON);
}

// =========================================================================
// 4. Engine build_tensor produces valid tensors
// =========================================================================

#[test]
fn g1_engine_build_tensor_produces_valid_tensor() {
    let engine = make_engine();
    let tensor = engine.build_tensor();
    assert!(
        tensor.validate().is_ok(),
        "build_tensor should produce a tensor with all dimensions in [0,1]"
    );
}

#[test]
fn g1_engine_build_tensor_health_score_reflects_engine() {
    let engine = make_engine();
    let tensor = engine.build_tensor();
    let report = engine.health_report().unwrap();
    // D6 = health_score should equal overall_health from the report
    assert!(
        (tensor.health_score - report.overall_health).abs() < f64::EPSILON,
        "tensor health_score ({}) should match engine overall_health ({})",
        tensor.health_score,
        report.overall_health
    );
}

#[test]
fn g1_engine_build_tensor_dimensions_in_range() {
    let engine = make_engine();
    let tensor = engine.build_tensor();
    let dims = tensor.to_array();
    for (i, &val) in dims.iter().enumerate() {
        assert!(
            (0.0..=1.0).contains(&val),
            "dimension {i} out of range: {val}"
        );
    }
}

#[test]
fn g1_engine_build_tensor_bytes_length() {
    let engine = make_engine();
    let tensor = engine.build_tensor();
    let bytes = tensor.to_bytes();
    assert_eq!(bytes.len(), 96, "12D tensor should be 96 bytes (12 * 8)");
}

// =========================================================================
// 5. Observer tick returns valid observation report
// =========================================================================

#[test]
fn g1_observer_tick_returns_valid_report() {
    let engine = make_engine();
    let obs = engine.observer();
    assert!(obs.is_some(), "observer should be initialized");

    let obs = obs.unwrap();
    let tensor = engine.build_tensor();
    let report = obs.tick(&tensor);
    assert!(report.is_ok(), "observer tick should succeed");

    let report = report.unwrap();
    assert_eq!(report.tick, 1, "first tick should be tick 1");
    assert!(
        (0.0..=1.0).contains(&report.current_fitness),
        "current_fitness should be in [0,1], got {}",
        report.current_fitness
    );
}

#[test]
fn g1_observer_tick_report_has_valid_system_state() {
    let engine = make_engine();
    let obs = engine.observer().unwrap();
    let tensor = engine.build_tensor();
    let report = obs.tick(&tensor).unwrap();

    // SystemState should be one of the valid variants
    let valid_states = [
        SystemState::Optimal,
        SystemState::Healthy,
        SystemState::Degraded,
        SystemState::Critical,
        SystemState::Failed,
    ];
    assert!(
        valid_states.contains(&report.system_state),
        "system_state should be a valid variant"
    );
}

#[test]
fn g1_observer_tick_increments_metrics() {
    let engine = make_engine();
    let obs = engine.observer().unwrap();
    let tensor = engine.build_tensor();

    let _ = obs.tick(&tensor).unwrap();
    let _ = obs.tick(&tensor).unwrap();

    let metrics = obs.metrics();
    assert_eq!(metrics.ticks_executed, 2, "should have 2 ticks");
    assert_eq!(metrics.reports_generated, 2, "should have 2 reports");
}

#[test]
fn g1_observer_tick_generation_and_tick_present() {
    let engine = make_engine();
    let obs = engine.observer().unwrap();
    let tensor = engine.build_tensor();

    let report = obs.tick(&tensor).unwrap();
    // generation comes from M39 evolution chamber, starts at 0
    assert_eq!(report.generation, 0);
    assert_eq!(report.tick, 1);
}

// =========================================================================
// 6. Fitness can be persisted after observer tick
// =========================================================================

#[tokio::test]
async fn g1_persist_fitness_after_observer_tick() {
    let engine = make_engine();
    let (mgr, _tmp) = make_db().await;

    let obs = engine.observer().unwrap();
    let tensor = engine.build_tensor();
    let report = obs.tick(&tensor).unwrap();

    // Build a fitness entry from the observation report
    let entry = FitnessHistoryEntry {
        timestamp: report.timestamp.to_rfc3339(),
        fitness: report.current_fitness,
        system_state: format!("{:?}", report.system_state),
        tensor_hash: format!("tick-{}", report.tick),
        generation: report.generation,
    };

    let affected = mgr.write_fitness_history(&entry).await.unwrap();
    assert_eq!(affected, 1);

    let loaded = mgr.read_latest_fitness().await.unwrap();
    assert!(loaded.is_some(), "should read back the persisted entry");
    let loaded = loaded.unwrap();
    assert!(
        (loaded.fitness - report.current_fitness).abs() < f64::EPSILON,
        "persisted fitness ({}) should match report fitness ({})",
        loaded.fitness,
        report.current_fitness
    );
}

#[tokio::test]
async fn g1_persist_multiple_ticks_fitness() {
    let engine = make_engine();
    let (mgr, _tmp) = make_db().await;
    let obs = engine.observer().unwrap();
    let tensor = engine.build_tensor();

    // Perform 5 ticks and persist each
    for _ in 0..5 {
        let report = obs.tick(&tensor).unwrap();
        let entry = FitnessHistoryEntry {
            timestamp: report.timestamp.to_rfc3339(),
            fitness: report.current_fitness,
            system_state: format!("{:?}", report.system_state),
            tensor_hash: format!("tick-{}", report.tick),
            generation: report.generation,
        };
        mgr.write_fitness_history(&entry).await.unwrap();
    }

    let history = mgr.load_fitness_history(10).await.unwrap();
    assert_eq!(history.len(), 5, "should have 5 persisted entries");
}

// =========================================================================
// 7. Engine peer_bridge returns Some when available
// =========================================================================

#[test]
fn g1_engine_peer_bridge_returns_some() {
    let engine = make_engine();
    let bridge = engine.peer_bridge();
    assert!(
        bridge.is_some(),
        "peer_bridge should be Some when PeerBridgeManager initializes successfully"
    );
}

#[test]
fn g1_engine_peer_bridge_has_peers() {
    let engine = make_engine();
    let bridge = engine.peer_bridge().unwrap();
    let summary = bridge.mesh_summary();
    assert!(
        summary.total_peers > 0,
        "mesh should have at least 1 configured peer, got {}",
        summary.total_peers
    );
}

// =========================================================================
// 8. Peer bridge mesh_summary returns data
// =========================================================================

#[test]
fn g1_mesh_summary_has_correct_structure() {
    let engine = make_engine();
    let bridge = engine.peer_bridge().unwrap();
    let summary = bridge.mesh_summary();

    // total_peers should match peers vec length
    assert_eq!(
        summary.total_peers,
        summary.peers.len(),
        "total_peers should equal peers.len()"
    );
    // mesh_synergy should be in [0,1]
    assert!(
        (0.0..=1.0).contains(&summary.mesh_synergy),
        "mesh_synergy should be in [0,1], got {}",
        summary.mesh_synergy
    );
}

#[test]
fn g1_mesh_summary_peers_have_service_ids() {
    let engine = make_engine();
    let bridge = engine.peer_bridge().unwrap();
    let summary = bridge.mesh_summary();

    for peer in &summary.peers {
        assert!(
            !peer.service_id.is_empty(),
            "every peer should have a non-empty service_id"
        );
        assert!(
            (0.0..=1.0).contains(&peer.health_score),
            "peer {} health_score {} not in [0,1]",
            peer.service_id,
            peer.health_score
        );
    }
}

#[test]
fn g1_mesh_summary_reachable_le_total() {
    let engine = make_engine();
    let bridge = engine.peer_bridge().unwrap();
    let summary = bridge.mesh_summary();

    assert!(
        summary.reachable_peers <= summary.total_peers,
        "reachable ({}) should be <= total ({})",
        summary.reachable_peers,
        summary.total_peers
    );
}

#[test]
fn g1_mesh_summary_serializes() {
    let engine = make_engine();
    let bridge = engine.peer_bridge().unwrap();
    let summary = bridge.mesh_summary();

    let json = serde_json::to_string(&summary);
    assert!(json.is_ok(), "mesh_summary should serialize to JSON");
    let json_str = json.unwrap();
    assert!(
        json_str.contains("total_peers"),
        "JSON should contain total_peers field"
    );
}

// =========================================================================
// 9. Combined: tick -> persist fitness -> read back = consistent
// =========================================================================

#[tokio::test]
async fn g1_combined_tick_persist_readback_consistent() {
    let engine = make_engine();
    let (mgr, _tmp) = make_db().await;
    let obs = engine.observer().unwrap();
    let tensor = engine.build_tensor();

    // Step 1: Tick the observer
    let report = obs.tick(&tensor).unwrap();
    let tick_fitness = report.current_fitness;
    let tick_generation = report.generation;
    let tick_state = format!("{:?}", report.system_state);

    // Step 2: Persist the fitness
    let entry = FitnessHistoryEntry {
        timestamp: report.timestamp.to_rfc3339(),
        fitness: tick_fitness,
        system_state: tick_state.clone(),
        tensor_hash: format!("combined-tick-{}", report.tick),
        generation: tick_generation,
    };
    mgr.write_fitness_history(&entry).await.unwrap();

    // Step 3: Read back
    let loaded = mgr.read_latest_fitness().await.unwrap().unwrap();

    // Step 4: Verify consistency
    assert!(
        (loaded.fitness - tick_fitness).abs() < f64::EPSILON,
        "readback fitness ({}) must match tick fitness ({})",
        loaded.fitness,
        tick_fitness
    );
    assert_eq!(loaded.generation, tick_generation);
    assert_eq!(loaded.system_state, tick_state);
    assert_eq!(loaded.tensor_hash, format!("combined-tick-{}", report.tick));
}

#[tokio::test]
async fn g1_combined_multiple_ticks_with_varying_tensors() {
    let engine = make_engine();
    let (mgr, _tmp) = make_db().await;
    let obs = engine.observer().unwrap();

    // Use different tensors to get varying fitness values
    let tensors = [
        Tensor12D::new([0.5; 12]),
        Tensor12D::new([0.8, 0.5, 0.3, 0.2, 0.5, 0.5, 0.9, 0.95, 0.8, 0.1, 0.05, 0.5]),
        Tensor12D::new([0.3, 0.3, 0.3, 0.3, 0.3, 0.3, 0.4, 0.5, 0.3, 0.6, 0.4, 0.3]),
    ];

    let mut persisted_fitnesses = Vec::new();
    for tensor in &tensors {
        let report = obs.tick(tensor).unwrap();
        persisted_fitnesses.push(report.current_fitness);

        let entry = FitnessHistoryEntry {
            timestamp: report.timestamp.to_rfc3339(),
            fitness: report.current_fitness,
            system_state: format!("{:?}", report.system_state),
            tensor_hash: format!("multi-tick-{}", report.tick),
            generation: report.generation,
        };
        mgr.write_fitness_history(&entry).await.unwrap();
    }

    let history = mgr.load_fitness_history(10).await.unwrap();
    assert_eq!(history.len(), 3, "should have 3 persisted entries");

    // History is newest-first, so index 0 = last tick
    assert!(
        (history[0].fitness - persisted_fitnesses[2]).abs() < f64::EPSILON,
        "most recent entry should match last tick fitness"
    );
}

#[tokio::test]
async fn g1_combined_engine_tensor_observer_db_full_cycle() {
    let engine = make_engine();
    let (mgr, _tmp) = make_db().await;

    // Full cycle: engine -> build_tensor -> observer tick -> persist -> read
    let tensor = engine.build_tensor();
    assert!(tensor.validate().is_ok(), "tensor must be valid");

    let obs = engine.observer().unwrap();
    let report = obs.tick(&tensor).unwrap();
    assert!(report.current_fitness > 0.0, "fitness should be positive");

    let entry = FitnessHistoryEntry {
        timestamp: report.timestamp.to_rfc3339(),
        fitness: report.current_fitness,
        system_state: format!("{:?}", report.system_state),
        tensor_hash: "full-cycle".to_string(),
        generation: report.generation,
    };
    mgr.write_fitness_history(&entry).await.unwrap();

    let readback = mgr.read_latest_fitness().await.unwrap().unwrap();
    assert!(
        (readback.fitness - report.current_fitness).abs() < f64::EPSILON,
        "full cycle: readback fitness should equal tick fitness"
    );

    // Verify engine health is still consistent after the cycle
    let health = engine.health_report().unwrap();
    assert!(health.is_healthy(), "engine should still be healthy");
}
