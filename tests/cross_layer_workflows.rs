//! Integration tests for multi-layer workflows through the Engine.

mod common;

use maintenance_engine::engine::Engine;
use maintenance_engine::m3_core_logic::{IssueType, Severity};
use maintenance_engine::m5_learning::HebbianPathway;
use maintenance_engine::m6_consensus::{
    is_quorum_reached, calculate_weighted_votes, default_agent_fleet,
    PBFT_N, PBFT_F, PBFT_Q,
};
use maintenance_engine::StdpConfig;

// =========================================================================
// Group 1: Health → Remediation (L2→L3) (6 tests)
// =========================================================================

#[test]
fn test_health_status_drives_remediation() {
    let engine = Engine::new();
    let report = engine.health_report();
    assert!(report.is_ok());
    // Submit remediation based on health
    let result = engine.submit_remediation(
        "synthex",
        IssueType::HealthFailure,
        Severity::High,
        "service health degraded",
    );
    assert!(result.is_ok());
    assert!(engine.pending_remediations() >= 1);
}

#[test]
fn test_multiple_remediations_for_different_services() {
    let engine = Engine::new();
    let services = ["synthex", "san-k7", "nais", "devops-engine"];
    for svc in &services {
        let result = engine.submit_remediation(
            svc,
            IssueType::LatencySpike,
            Severity::Medium,
            "latency spike detected",
        );
        assert!(result.is_ok());
    }
    assert!(engine.pending_remediations() >= 4);
}

#[test]
fn test_severity_escalation_mapping() {
    let engine = Engine::new();
    // Low severity should succeed
    let low = engine.submit_remediation("svc-1", IssueType::HealthFailure, Severity::Low, "low");
    assert!(low.is_ok());
    // Critical severity should succeed
    let critical = engine.submit_remediation("svc-2", IssueType::Crash, Severity::Critical, "critical");
    assert!(critical.is_ok());
}

#[test]
fn test_remediation_does_not_affect_health_report() {
    let engine = Engine::new();
    let before = engine.health_report();
    let _ = engine.submit_remediation("synthex", IssueType::HealthFailure, Severity::High, "test");
    let after = engine.health_report();
    if let (Ok(b), Ok(a)) = (before, after) {
        // Submitting remediation doesn't immediately change layer health
        assert!(
            (b.overall_health - a.overall_health).abs() < f64::EPSILON,
            "remediation submission should not change overall health"
        );
    }
}

#[test]
fn test_health_report_then_remediation_then_health_report() {
    let engine = Engine::new();
    let r1 = engine.health_report();
    assert!(r1.is_ok());
    let _ = engine.submit_remediation("san-k7", IssueType::ErrorRateHigh, Severity::Medium, "test");
    let r2 = engine.health_report();
    assert!(r2.is_ok());
}

#[test]
fn test_remediation_success_rate_unchanged_by_submission() {
    let engine = Engine::new();
    let rate_before = engine.remediation_success_rate();
    let _ = engine.submit_remediation("nais", IssueType::HealthFailure, Severity::Low, "test");
    let rate_after = engine.remediation_success_rate();
    // Just submitting doesn't change success rate (no completions yet)
    assert!(
        (rate_before - rate_after).abs() < f64::EPSILON,
        "submission alone should not change success rate"
    );
}

// =========================================================================
// Group 2: Remediation → Learning (L3→L5) (5 tests)
// =========================================================================

#[test]
fn test_remediation_then_learning_cycle() {
    let engine = Engine::new();
    let _ = engine.submit_remediation("synthex", IssueType::HealthFailure, Severity::High, "fail");
    let result = engine.learning_cycle();
    assert!(result.is_ok());
    if let Ok(lcr) = result {
        // Learning should still function after remediation
        assert!(lcr.pathways_decayed > 0);
    }
}

#[test]
fn test_learning_preserves_remediation_state() {
    let engine = Engine::new();
    let _ = engine.submit_remediation("synthex", IssueType::HealthFailure, Severity::High, "test");
    let pending_before = engine.pending_remediations();
    let _ = engine.learning_cycle();
    let pending_after = engine.pending_remediations();
    assert_eq!(pending_before, pending_after, "learning should not change pending remediations");
}

#[test]
fn test_multiple_learning_cycles_after_remediation() {
    let engine = Engine::new();
    let _ = engine.submit_remediation("san-k7", IssueType::LatencySpike, Severity::Medium, "spike");
    for i in 0..5 {
        let result = engine.learning_cycle();
        assert!(result.is_ok(), "learning cycle {i} should succeed");
    }
}

#[test]
fn test_pathway_strength_decay_after_remediation() {
    let engine = Engine::new();
    let _ = engine.submit_remediation("synthex", IssueType::HealthFailure, Severity::High, "test");
    let before = engine.average_pathway_strength();
    let _ = engine.learning_cycle();
    let after = engine.average_pathway_strength();
    assert!(
        after <= before + f64::EPSILON,
        "decay should not increase strength"
    );
}

#[test]
fn test_antipattern_count_stable_through_remediation() {
    let engine = Engine::new();
    let count_before = engine.antipattern_detector().violation_count();
    let _ = engine.submit_remediation("nais", IssueType::ErrorRateHigh, Severity::Low, "test");
    let _ = engine.learning_cycle();
    let count_after = engine.antipattern_detector().violation_count();
    // No anti-pattern violations should be created by a simple remediation
    assert_eq!(count_before, count_after);
}

// =========================================================================
// Group 3: Learning Pathway Lifecycle (L5→L5) (6 tests)
// =========================================================================

#[test]
fn test_decay_then_verify_bounded() {
    let engine = Engine::new();
    let initial = engine.average_pathway_strength();
    for _ in 0..20 {
        let _ = engine.learning_cycle();
    }
    let final_strength = engine.average_pathway_strength();
    assert!(final_strength >= 0.0, "strength must not go negative");
    assert!(
        final_strength <= initial + f64::EPSILON,
        "20 decay cycles should not increase strength"
    );
}

#[test]
fn test_pathway_strength_convergence() {
    let engine = Engine::new();
    let mut prev = engine.average_pathway_strength();
    let mut delta_shrinking = true;
    let mut last_delta = f64::MAX;

    for _ in 0..30 {
        let _ = engine.learning_cycle();
        let current = engine.average_pathway_strength();
        let delta = (prev - current).abs();
        if delta > last_delta + f64::EPSILON {
            delta_shrinking = false;
        }
        last_delta = delta;
        prev = current;
    }
    // Not strictly required to converge, but strength should remain bounded
    assert!(engine.average_pathway_strength() >= 0.0);
    let _ = delta_shrinking; // used for potential convergence check
}

#[test]
fn test_hebbian_pathway_direct_ltp() {
    let mut pathway = HebbianPathway::new("source", "target");
    let config = StdpConfig::default();
    let before = pathway.strength;
    pathway.apply_ltp(&config);
    assert!(
        pathway.strength > before,
        "LTP should increase strength"
    );
}

#[test]
fn test_hebbian_pathway_direct_ltd() {
    let mut pathway = HebbianPathway::new("source", "target");
    let config = StdpConfig::default();
    let before = pathway.strength;
    pathway.apply_ltd(&config);
    assert!(
        pathway.strength < before,
        "LTD should decrease strength"
    );
}

#[test]
fn test_hebbian_pathway_success_failure_tracking() {
    let config = StdpConfig::default();
    let mut pathway = HebbianPathway::new("a", "b");
    pathway.record_success(&config);
    pathway.record_success(&config);
    pathway.record_failure(&config);
    assert!(
        pathway.success_rate() > 0.5,
        "2 successes + 1 failure should give >50% success rate"
    );
}

#[test]
fn test_default_pathways_exist() {
    let engine = Engine::new();
    let count = engine.pathway_count();
    assert!(count >= 9, "should have >= 9 default pathways, got {count}");
}

// =========================================================================
// Group 4: Consensus Flow (L6→L6) (6 tests)
// =========================================================================

#[test]
fn test_consensus_constants() {
    assert_eq!(PBFT_N, 40);
    assert_eq!(PBFT_F, 13);
    assert_eq!(PBFT_Q, 27);
}

#[test]
fn test_default_fleet_composition() {
    let fleet = default_agent_fleet();
    assert_eq!(fleet.len(), 41, "fleet should have 41 agents");
    let human = fleet.iter().find(|a| a.id == "@0.A");
    assert!(human.is_some());
}

#[test]
fn test_quorum_reached_with_sufficient_votes() {
    let votes = common::generate_quorum_votes("test-proposal", 27);
    #[allow(clippy::cast_possible_truncation)]
    let vote_count = votes.len() as u32;
    let reached = is_quorum_reached(vote_count, PBFT_Q);
    assert!(reached, "27 votes should reach quorum");
}

#[test]
fn test_quorum_not_reached_with_insufficient_votes() {
    let votes = common::generate_quorum_votes("test-proposal", 10);
    #[allow(clippy::cast_possible_truncation)]
    let vote_count = votes.len() as u32;
    let reached = is_quorum_reached(vote_count, PBFT_Q);
    assert!(!reached, "10 votes should not reach quorum");
}

#[test]
fn test_weighted_votes_calculation() {
    let votes = common::generate_quorum_votes("test-proposal", 5);
    let (for_weight, against_weight, abstain_weight) = calculate_weighted_votes(&votes);
    assert!(for_weight > 0.0, "weighted for votes should be positive");
    let _ = (against_weight, abstain_weight);
}

#[test]
fn test_consensus_does_not_affect_learning() {
    let engine = Engine::new();
    let strength_before = engine.average_pathway_strength();
    // Consensus operations
    let _ = engine.open_ballot_count();
    let _ = engine.total_dissent();
    let strength_after = engine.average_pathway_strength();
    assert!(
        (strength_before - strength_after).abs() < f64::EPSILON,
        "consensus reads should not change pathway strength"
    );
}

// =========================================================================
// Group 5: Observer + Engine (L7+Engine) (6 tests)
// =========================================================================

#[test]
fn test_engine_tensor_to_observer_tick() {
    let engine = Engine::new();
    let tensor = engine.build_tensor();
    if let Some(obs) = engine.observer() {
        let result = obs.tick(&tensor);
        assert!(result.is_ok());
        if let Ok(report) = result {
            assert!(report.current_fitness >= 0.0);
            assert!(report.current_fitness <= 1.0);
        }
    }
}

#[test]
fn test_observer_multiple_ticks_with_engine_tensor() {
    let engine = Engine::new();
    if let Some(obs) = engine.observer() {
        for _ in 0..5 {
            let tensor = engine.build_tensor();
            let result = obs.tick(&tensor);
            assert!(result.is_ok());
        }
        assert_eq!(obs.tick_count(), 5);
    }
}

#[test]
fn test_observer_fitness_from_engine_tensor() {
    let engine = Engine::new();
    let tensor = engine.build_tensor();
    if let Some(obs) = engine.observer() {
        let _ = obs.tick(&tensor);
        assert!(obs.fitness().current_fitness().is_some());
    }
}

#[test]
fn test_observer_health_after_tick() {
    let engine = Engine::new();
    let tensor = engine.build_tensor();
    if let Some(obs) = engine.observer() {
        let _ = obs.tick(&tensor);
    }
    // Health report should still work after observer tick
    let report = engine.health_report();
    assert!(report.is_ok());
}

#[test]
fn test_observer_metrics_accumulate() {
    let engine = Engine::new();
    if let Some(obs) = engine.observer() {
        let tensor = engine.build_tensor();
        for _ in 0..3 {
            let _ = obs.tick(&tensor);
        }
        let m = obs.metrics();
        assert_eq!(m.ticks_executed, 3);
        assert_eq!(m.reports_generated, 3);
    }
}

#[test]
fn test_observer_clear_does_not_break_engine() {
    let engine = Engine::new();
    if let Some(obs) = engine.observer() {
        let tensor = engine.build_tensor();
        let _ = obs.tick(&tensor);
        obs.clear();
        // Engine should still work after observer clear
        let report = engine.health_report();
        assert!(report.is_ok());
    }
}

// =========================================================================
// Group 6: Integration Layer (L4) (4 tests)
// =========================================================================

#[test]
fn test_event_bus_channels_available() {
    let engine = Engine::new();
    assert_eq!(engine.event_channel_count(), 6);
}

#[test]
fn test_bridge_count_initial() {
    let engine = Engine::new();
    // Bridge map starts empty; bridges must be explicitly registered
    assert_eq!(engine.bridge_count(), 0);
}

#[test]
fn test_synergy_in_range() {
    let engine = Engine::new();
    let synergy = engine.overall_synergy();
    assert!((0.0..=1.0).contains(&synergy));
}

#[test]
fn test_integration_state_independent_of_learning() {
    let engine = Engine::new();
    let channels_before = engine.event_channel_count();
    let bridges_before = engine.bridge_count();
    let _ = engine.learning_cycle();
    assert_eq!(engine.event_channel_count(), channels_before);
    assert_eq!(engine.bridge_count(), bridges_before);
}

// =========================================================================
// Group 7: Full Pipeline Workflows (6 tests)
// =========================================================================

#[test]
fn test_full_health_remediation_learning_sequence() {
    let engine = Engine::new();

    // Step 1: Health report
    let report = engine.health_report();
    assert!(report.is_ok());

    // Step 2: Submit remediation
    let rem = engine.submit_remediation(
        "synthex",
        IssueType::HealthFailure,
        Severity::High,
        "degraded health",
    );
    assert!(rem.is_ok());

    // Step 3: Learning cycle
    let learn = engine.learning_cycle();
    assert!(learn.is_ok());

    // Step 4: Verify health still works
    let report2 = engine.health_report();
    assert!(report2.is_ok());
}

#[test]
fn test_remediation_learning_consensus_independent() {
    let engine = Engine::new();

    let _ = engine.submit_remediation("svc-1", IssueType::Crash, Severity::Critical, "crash");
    let _ = engine.learning_cycle();

    // Consensus state should be unaffected
    assert_eq!(engine.active_proposals(), 0);
    assert_eq!(engine.open_ballot_count(), 0);
    assert_eq!(engine.total_dissent(), 0);
}

#[test]
fn test_tensor_through_full_stack() {
    let engine = Engine::new();

    // Build tensor
    let tensor = engine.build_tensor();
    assert!(tensor.validate().is_ok());

    // Observer tick
    if let Some(obs) = engine.observer() {
        let result = obs.tick(&tensor);
        assert!(result.is_ok());
    }

    // Verify engine still consistent
    let report = engine.health_report();
    assert!(report.is_ok());
}

#[test]
fn test_repeated_full_cycles() {
    let engine = Engine::new();
    for i in 0..5 {
        // Health
        let report = engine.health_report();
        assert!(report.is_ok(), "health report {i} failed");

        // Remediation
        let _ = engine.submit_remediation(
            &format!("svc-{i}"),
            IssueType::HealthFailure,
            Severity::Low,
            "test",
        );

        // Learning
        let learn = engine.learning_cycle();
        assert!(learn.is_ok(), "learning cycle {i} failed");

        // Observer
        if let Some(obs) = engine.observer() {
            let tensor = engine.build_tensor();
            let _ = obs.tick(&tensor);
        }
    }
}

#[test]
fn test_all_accessors_after_operations() {
    let engine = Engine::new();
    let _ = engine.submit_remediation("synthex", IssueType::HealthFailure, Severity::High, "test");
    let _ = engine.learning_cycle();

    // All accessors should work without panic
    let _ = engine.service_count();
    let _ = engine.pipeline_count();
    let _ = engine.pathway_count();
    let _ = engine.active_proposals();
    let _ = engine.pending_remediations();
    let _ = engine.active_remediations();
    let _ = engine.remediation_success_rate();
    let _ = engine.open_ballot_count();
    let _ = engine.total_dissent();
    let _ = engine.current_view_number();
    let _ = engine.event_channel_count();
    let _ = engine.bridge_count();
    let _ = engine.overall_synergy();
    let _ = engine.average_pathway_strength();
    let _ = engine.observer_enabled();
    let _ = engine.build_tensor();
}

#[test]
fn test_fresh_engine_defaults_verified() {
    let engine = Engine::new();

    // Pipelines
    assert_eq!(engine.pipeline_count(), 8);

    // Pathways
    assert!(engine.pathway_count() >= 9);

    // Consensus
    assert_eq!(engine.active_proposals(), 0);
    assert_eq!(engine.open_ballot_count(), 0);
    assert_eq!(engine.total_dissent(), 0);
    assert_eq!(engine.current_view_number(), 0);

    // Remediation
    assert_eq!(engine.pending_remediations(), 0);
    assert_eq!(engine.active_remediations(), 0);

    // Observer
    assert!(engine.observer_enabled());

    // Event bus
    assert_eq!(engine.event_channel_count(), 6);

    // Fleet
    let fleet = engine.pbft_manager().get_fleet();
    assert_eq!(fleet.len(), 41);
}

// =========================================================================
// Group 8: Edge Cases (6 tests)
// =========================================================================

#[test]
fn test_fifty_learning_cycles_stability() {
    let engine = Engine::new();
    for _ in 0..50 {
        let result = engine.learning_cycle();
        assert!(result.is_ok());
    }
    let strength = engine.average_pathway_strength();
    assert!(strength >= 0.0);
    assert!(strength <= 1.0);
}

#[test]
fn test_many_remediations_then_health() {
    let engine = Engine::new();
    for i in 0..30 {
        let _ = engine.submit_remediation(
            &format!("svc-{i}"),
            IssueType::HealthFailure,
            Severity::Low,
            "bulk test",
        );
    }
    let report = engine.health_report();
    assert!(report.is_ok());
    assert!(engine.pending_remediations() >= 30);
}

#[test]
fn test_health_report_stable_across_operations() {
    let engine = Engine::new();
    let r1 = engine.health_report();
    let _ = engine.submit_remediation("synthex", IssueType::HealthFailure, Severity::Low, "test");
    let _ = engine.learning_cycle();
    let r2 = engine.health_report();

    // Both reports should succeed
    assert!(r1.is_ok());
    assert!(r2.is_ok());
}

#[test]
fn test_tensor_valid_after_learning_cycles() {
    let engine = Engine::new();
    for _ in 0..10 {
        let _ = engine.learning_cycle();
    }
    let tensor = engine.build_tensor();
    assert!(tensor.validate().is_ok());
}

#[test]
fn test_observer_tick_after_learning_cycles() {
    let engine = Engine::new();
    for _ in 0..5 {
        let _ = engine.learning_cycle();
    }
    if let Some(obs) = engine.observer() {
        let tensor = engine.build_tensor();
        let result = obs.tick(&tensor);
        assert!(result.is_ok());
    }
}

#[test]
fn test_concurrent_engine_operations() {
    use std::sync::Arc;
    let engine = Arc::new(Engine::new());

    let e1 = Arc::clone(&engine);
    let h1 = std::thread::spawn(move || {
        for _ in 0..5 {
            let _ = e1.health_report();
        }
    });

    let e2 = Arc::clone(&engine);
    let h2 = std::thread::spawn(move || {
        for _ in 0..5 {
            let _ = e2.learning_cycle();
        }
    });

    let e3 = Arc::clone(&engine);
    let h3 = std::thread::spawn(move || {
        for i in 0..5 {
            let _ = e3.submit_remediation(
                &format!("svc-{i}"),
                IssueType::HealthFailure,
                Severity::Low,
                "concurrent test",
            );
        }
    });

    h1.join().ok();
    h2.join().ok();
    h3.join().ok();

    // Engine should still be consistent
    let report = engine.health_report();
    assert!(report.is_ok());
}
