//! Integration tests for the Engine orchestrator.

mod common;

use maintenance_engine::engine::Engine;
use maintenance_engine::m3_core_logic::{IssueType, Severity};

// =========================================================================
// Group 1: Construction and Defaults (8 tests)
// =========================================================================

#[test]
fn test_engine_construction() {
    let engine = Engine::new();
    assert!(engine.pipeline_count() > 0);
}

#[test]
fn test_engine_default_trait() {
    let engine = Engine::default();
    assert!(engine.pipeline_count() > 0);
}

#[test]
fn test_engine_default_pipelines() {
    let engine = Engine::new();
    assert_eq!(engine.pipeline_count(), 8);
}

#[test]
fn test_engine_default_pathways() {
    let engine = Engine::new();
    assert!(
        engine.pathway_count() >= 9,
        "should have >= 9 default pathways, got {}",
        engine.pathway_count()
    );
}

#[test]
fn test_engine_default_event_channels() {
    let engine = Engine::new();
    assert_eq!(engine.event_channel_count(), 6);
}

#[test]
fn test_engine_observer_enabled() {
    let engine = Engine::new();
    assert!(engine.observer_enabled());
    assert!(engine.observer().is_some());
}

#[test]
fn test_engine_pbft_fleet_size() {
    let engine = Engine::new();
    let fleet = engine.pbft_manager().get_fleet();
    assert_eq!(fleet.len(), 41, "fleet should have 41 agents (40 + Human @0.A)");
}

#[test]
fn test_engine_human_agent_in_fleet() {
    let engine = Engine::new();
    let fleet = engine.pbft_manager().get_fleet();
    let human = fleet.iter().find(|a| a.id == "@0.A");
    assert!(human.is_some(), "Human @0.A should be in the fleet");
}

// =========================================================================
// Group 2: Health Reporting (10 tests)
// =========================================================================

#[test]
fn test_health_report_returns_ok() {
    let engine = Engine::new();
    assert!(engine.health_report().is_ok());
}

#[test]
fn test_health_report_seven_layers() {
    let engine = Engine::new();
    if let Ok(report) = engine.health_report() {
        assert_eq!(report.layer_health.len(), 7);
    }
}

#[test]
fn test_health_report_foundation_always_one() {
    let engine = Engine::new();
    if let Ok(report) = engine.health_report() {
        assert!(
            (report.layer_health[0] - 1.0).abs() < f64::EPSILON,
            "L1 Foundation should be 1.0, got {}",
            report.layer_health[0]
        );
    }
}

#[test]
fn test_health_report_overall_in_range() {
    let engine = Engine::new();
    if let Ok(report) = engine.health_report() {
        assert!(
            (0.0..=1.0).contains(&report.overall_health),
            "overall_health should be in [0,1], got {}",
            report.overall_health
        );
    }
}

#[test]
fn test_health_report_is_healthy() {
    let engine = Engine::new();
    if let Ok(report) = engine.health_report() {
        // Fresh engine should be healthy
        assert!(
            report.is_healthy(),
            "fresh engine should be healthy, overall={}",
            report.overall_health
        );
    }
}

#[test]
fn test_health_report_weakest_layer() {
    let engine = Engine::new();
    if let Ok(report) = engine.health_report() {
        let (idx, score) = report.weakest_layer();
        assert!(idx < 7);
        for &ls in &report.layer_health {
            assert!(score <= ls + f64::EPSILON);
        }
    }
}

#[test]
fn test_health_report_pipelines_match() {
    let engine = Engine::new();
    if let Ok(report) = engine.health_report() {
        assert_eq!(report.pipelines_active, engine.pipeline_count());
    }
}

#[test]
fn test_health_report_pathways_match() {
    let engine = Engine::new();
    if let Ok(report) = engine.health_report() {
        assert_eq!(report.pathways_count, engine.pathway_count());
    }
}

#[test]
fn test_health_report_stability() {
    let engine = Engine::new();
    let r1 = engine.health_report();
    let r2 = engine.health_report();
    if let (Ok(a), Ok(b)) = (r1, r2) {
        assert!(
            (a.overall_health - b.overall_health).abs() < f64::EPSILON,
            "consecutive reports should be identical"
        );
    }
}

#[test]
fn test_health_report_observer_layer_healthy() {
    let engine = Engine::new();
    if let Ok(report) = engine.health_report() {
        assert!(
            (report.layer_health[6] - 1.0).abs() < f64::EPSILON,
            "L7 should be 1.0 before any ticks, got {}",
            report.layer_health[6]
        );
    }
}

// =========================================================================
// Group 3: Remediation (8 tests)
// =========================================================================

#[test]
fn test_submit_remediation_success() {
    let engine = Engine::new();
    let result = engine.submit_remediation(
        "synthex",
        IssueType::HealthFailure,
        Severity::High,
        "test issue",
    );
    assert!(result.is_ok());
}

#[test]
fn test_submit_remediation_unique_ids() {
    let engine = Engine::new();
    let id1 = engine.submit_remediation("synthex", IssueType::HealthFailure, Severity::High, "a");
    let id2 = engine.submit_remediation("synthex", IssueType::HealthFailure, Severity::High, "b");
    assert_ne!(id1.ok(), id2.ok());
}

#[test]
fn test_submit_remediation_increments_pending() {
    let engine = Engine::new();
    let before = engine.pending_remediations();
    let _ = engine.submit_remediation("san-k7", IssueType::LatencySpike, Severity::Medium, "spike");
    assert_eq!(engine.pending_remediations(), before + 1);
}

#[test]
fn test_submit_remediation_empty_service_rejected() {
    let engine = Engine::new();
    let result = engine.submit_remediation("", IssueType::HealthFailure, Severity::Low, "desc");
    assert!(result.is_err());
}

#[test]
fn test_submit_remediation_empty_description_rejected() {
    let engine = Engine::new();
    let result = engine.submit_remediation("synthex", IssueType::HealthFailure, Severity::Low, "");
    assert!(result.is_err());
}

#[test]
fn test_submit_remediation_all_severities() {
    let engine = Engine::new();
    for severity in [Severity::Low, Severity::Medium, Severity::High, Severity::Critical] {
        let result = engine.submit_remediation("nais", IssueType::ErrorRateHigh, severity, "test");
        assert!(result.is_ok(), "should accept severity {severity:?}");
    }
}

#[test]
fn test_submit_remediation_all_issue_types() {
    let engine = Engine::new();
    let types = [
        IssueType::HealthFailure,
        IssueType::LatencySpike,
        IssueType::ErrorRateHigh,
        IssueType::MemoryPressure,
        IssueType::DiskPressure,
        IssueType::ConnectionFailure,
        IssueType::Timeout,
        IssueType::Crash,
    ];
    for issue_type in types {
        let result = engine.submit_remediation("synthex", issue_type, Severity::Medium, "test");
        assert!(result.is_ok(), "should accept issue type {issue_type:?}");
    }
}

#[test]
fn test_remediation_success_rate_in_range() {
    let engine = Engine::new();
    let rate = engine.remediation_success_rate();
    assert!(
        (0.0..=1.0).contains(&rate),
        "success rate should be in [0,1], got {rate}"
    );
}

// =========================================================================
// Group 4: Learning Cycle (8 tests)
// =========================================================================

#[test]
fn test_learning_cycle_returns_ok() {
    let engine = Engine::new();
    assert!(engine.learning_cycle().is_ok());
}

#[test]
fn test_learning_cycle_decay_occurs() {
    let engine = Engine::new();
    if let Ok(result) = engine.learning_cycle() {
        assert!(
            result.pathways_decayed > 0,
            "should have decayed pathways with defaults loaded"
        );
    }
}

#[test]
fn test_learning_cycle_had_activity() {
    let engine = Engine::new();
    if let Ok(result) = engine.learning_cycle() {
        assert!(result.had_activity());
    }
}

#[test]
fn test_learning_cycle_strength_decreases() {
    let engine = Engine::new();
    let before = engine.average_pathway_strength();
    let _ = engine.learning_cycle();
    let after = engine.average_pathway_strength();
    assert!(
        after <= before + f64::EPSILON,
        "decay should not increase strength: before={before}, after={after}"
    );
}

#[test]
fn test_learning_cycle_multiple_stable() {
    let engine = Engine::new();
    for i in 0..10 {
        let result = engine.learning_cycle();
        assert!(result.is_ok(), "cycle {i} should succeed");
    }
}

#[test]
fn test_learning_cycle_bounded_after_many() {
    let engine = Engine::new();
    for _ in 0..50 {
        let _ = engine.learning_cycle();
    }
    let strength = engine.average_pathway_strength();
    assert!(
        strength >= 0.0,
        "strength must not go negative: {strength}"
    );
}

#[test]
fn test_average_pathway_strength_in_range() {
    let engine = Engine::new();
    let strength = engine.average_pathway_strength();
    assert!(
        (0.0..=1.0).contains(&strength),
        "average strength should be in [0,1], got {strength}"
    );
}

#[test]
fn test_antipattern_detector_defaults() {
    let engine = Engine::new();
    assert_eq!(engine.antipattern_detector().violation_count(), 0);
    assert!(engine.antipattern_detector().pattern_count() >= 15);
}

// =========================================================================
// Group 5: Consensus (5 tests)
// =========================================================================

#[test]
fn test_open_ballot_count_zero() {
    let engine = Engine::new();
    assert_eq!(engine.open_ballot_count(), 0);
}

#[test]
fn test_total_dissent_zero() {
    let engine = Engine::new();
    assert_eq!(engine.total_dissent(), 0);
}

#[test]
fn test_current_view_number_zero() {
    let engine = Engine::new();
    assert_eq!(engine.current_view_number(), 0);
}

#[test]
fn test_active_proposals_zero() {
    let engine = Engine::new();
    assert_eq!(engine.active_proposals(), 0);
}

#[test]
fn test_consensus_fleet_accessible() {
    let engine = Engine::new();
    let fleet = engine.pbft_manager().get_fleet();
    assert_eq!(fleet.len(), 41);
}

// =========================================================================
// Group 6: Integration Layer (4 tests)
// =========================================================================

#[test]
fn test_event_channel_count() {
    let engine = Engine::new();
    assert_eq!(engine.event_channel_count(), 6);
}

#[test]
fn test_bridge_count() {
    let engine = Engine::new();
    let count = engine.bridge_count();
    // Bridges must be explicitly registered; starts at 0
    assert_eq!(count, 0);
}

#[test]
fn test_overall_synergy() {
    let engine = Engine::new();
    let synergy = engine.overall_synergy();
    assert!(
        (0.0..=1.0).contains(&synergy),
        "synergy should be in [0,1], got {synergy}"
    );
}

#[test]
fn test_service_count() {
    let engine = Engine::new();
    let count = engine.service_count();
    assert!(count < 10_000, "sanity check");
}

// =========================================================================
// Group 7: Tensor & Observer via Engine (6 tests)
// =========================================================================

#[test]
fn test_build_tensor_valid() {
    let engine = Engine::new();
    let tensor = engine.build_tensor();
    assert!(tensor.validate().is_ok());
}

#[test]
fn test_build_tensor_all_dimensions_in_range() {
    let engine = Engine::new();
    let tensor = engine.build_tensor();
    let arr = tensor.to_array();
    for (i, &val) in arr.iter().enumerate() {
        assert!(
            (0.0..=1.0).contains(&val),
            "dimension {i} out of [0,1]: {val}"
        );
    }
}

#[test]
fn test_build_tensor_health_matches_report() {
    let engine = Engine::new();
    let tensor = engine.build_tensor();
    if let Ok(report) = engine.health_report() {
        assert!(
            (tensor.health_score - report.overall_health).abs() < f64::EPSILON,
            "tensor health should match report overall health"
        );
    }
}

#[test]
fn test_observer_tick_with_engine_tensor() {
    let engine = Engine::new();
    let tensor = engine.build_tensor();
    if let Some(obs) = engine.observer() {
        let result = obs.tick(&tensor);
        assert!(result.is_ok());
        assert_eq!(obs.tick_count(), 1);
    }
}

#[test]
fn test_observer_metrics_after_engine_tick() {
    let engine = Engine::new();
    let tensor = engine.build_tensor();
    if let Some(obs) = engine.observer() {
        let _ = obs.tick(&tensor);
        let m = obs.metrics();
        assert_eq!(m.ticks_executed, 1);
        assert_eq!(m.reports_generated, 1);
    }
}

#[test]
fn test_build_tensor_synergy_matches() {
    let engine = Engine::new();
    let tensor = engine.build_tensor();
    let synergy = engine.overall_synergy().clamp(0.0, 1.0);
    assert!(
        (tensor.synergy - synergy).abs() < f64::EPSILON,
        "tensor synergy should match engine synergy"
    );
}

// =========================================================================
// Group 8: Accessor Methods (7 tests)
// =========================================================================

#[test]
fn test_accessor_health_monitor() {
    let engine = Engine::new();
    let _ = engine.health_monitor().probe_count();
}

#[test]
fn test_accessor_lifecycle_manager() {
    let engine = Engine::new();
    let _ = engine.lifecycle_manager();
}

#[test]
fn test_accessor_circuit_breaker() {
    let engine = Engine::new();
    let _ = engine.circuit_breaker();
}

#[test]
fn test_accessor_pipeline_manager() {
    let engine = Engine::new();
    assert_eq!(engine.pipeline_manager().pipeline_count(), 8);
}

#[test]
fn test_accessor_remediator() {
    let engine = Engine::new();
    assert_eq!(engine.remediator().pending_count(), 0);
}

#[test]
fn test_accessor_hebbian_manager() {
    let engine = Engine::new();
    assert!(engine.hebbian_manager().pathway_count() >= 9);
}

#[test]
fn test_accessor_peer_bridge() {
    let engine = Engine::new();
    // Peer bridge may or may not be available, just ensure no panic
    let _peer = engine.peer_bridge();
}

// =========================================================================
// Group 9: Edge Cases (4 tests)
// =========================================================================

#[test]
fn test_many_remediations() {
    let engine = Engine::new();
    for i in 0..20 {
        let _ = engine.submit_remediation(
            &format!("svc-{i}"),
            IssueType::HealthFailure,
            Severity::Low,
            "test",
        );
    }
    assert!(engine.pending_remediations() >= 20);
}

#[test]
fn test_repeated_learning_then_health() {
    let engine = Engine::new();
    for _ in 0..10 {
        let _ = engine.learning_cycle();
    }
    let report = engine.health_report();
    assert!(report.is_ok());
}

#[test]
fn test_remediation_then_learning() {
    let engine = Engine::new();
    let _ = engine.submit_remediation("synthex", IssueType::HealthFailure, Severity::High, "fail");
    let result = engine.learning_cycle();
    assert!(result.is_ok());
}

#[test]
fn test_learning_does_not_affect_consensus() {
    let engine = Engine::new();
    let proposals_before = engine.active_proposals();
    let _ = engine.learning_cycle();
    assert_eq!(engine.active_proposals(), proposals_before);
}
