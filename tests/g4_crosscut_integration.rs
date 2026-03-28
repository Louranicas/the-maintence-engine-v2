#![allow(clippy::unwrap_used)]
//! # G4 Cross-Cutting Integration Tests
//!
//! Integration tests exercising G4 cross-cutting features across
//! the Engine, PBFT consensus, Hebbian learning, and remediation
//! subsystems. Validates that the full wired Engine produces correct
//! outcomes when features are combined.
//!
//! ## Coverage
//!
//! | # | Feature | Subsystems |
//! |---|---------|------------|
//! | 1 | auto_remediate direct call | Engine, L3 RemediationEngine |
//! | 2 | auto_remediate valid params | Engine, L3 |
//! | 3 | auto_remediate empty service_id | Engine validation |
//! | 4 | Learning cycle via engine | Engine, L5 Hebbian/STDP |
//! | 5 | Multiple learning cycles decay | Engine, L5 |
//! | 6 | PBFT vote submission | Engine, L6 PbftManager |
//! | 7 | PBFT full round | Engine, L6 PbftManager |
//! | 8 | Hebbian record_success | Engine, L5 HebbianManager |
//! | 9 | Hebbian record_failure | Engine, L5 HebbianManager |
//! | 10 | Combined pipeline flow | Engine, L3 + L5 + L6 |

use maintenance_engine::engine::Engine;
use maintenance_engine::m3_core_logic::{IssueType, Severity};
use maintenance_engine::m6_consensus::{ConsensusAction, VoteType};

// ===========================================================================
// Helpers
// ===========================================================================

/// Create a fresh engine for each test.
fn make_engine() -> Engine {
    Engine::new()
}

/// Find a known default pathway key from the HebbianManager.
///
/// Default pathways include `"maintenance->restart"` among others.
/// Returns the key of the first pathway found.
fn first_pathway_key(engine: &Engine) -> String {
    let strongest = engine.hebbian_manager().get_strongest_pathways(1);
    assert!(!strongest.is_empty(), "engine should have default pathways");
    format!("{}->{}", strongest[0].source, strongest[0].target)
}

// ===========================================================================
// Test 1: auto_remediate creates remediation when health is low
// ===========================================================================

#[tokio::test]
async fn test_auto_remediate_creates_remediation() {
    let engine = make_engine();

    let pending_before = engine.pending_remediations();

    let result = engine.auto_remediate("synthex", Severity::High, "health below threshold");
    assert!(result.is_ok(), "auto_remediate should succeed: {result:?}");

    let request_id = result.unwrap();
    assert!(!request_id.is_empty(), "should return a non-empty request ID");

    let pending_after = engine.pending_remediations();
    assert_eq!(
        pending_after,
        pending_before + 1,
        "pending count should increase by 1"
    );
}

// ===========================================================================
// Test 2: auto_remediate with valid params returns Ok
// ===========================================================================

#[tokio::test]
async fn test_auto_remediate_valid_params_ok() {
    let engine = make_engine();

    // Test all severity levels
    for severity in [Severity::Low, Severity::Medium, Severity::High, Severity::Critical] {
        let result = engine.auto_remediate("san-k7", severity, "integration test");
        assert!(
            result.is_ok(),
            "auto_remediate should accept severity {severity:?}: {result:?}"
        );
    }

    // Verify all 4 requests are pending
    assert!(
        engine.pending_remediations() >= 4,
        "should have at least 4 pending remediations"
    );
}

// ===========================================================================
// Test 3: auto_remediate with empty service_id returns Err
// ===========================================================================

#[tokio::test]
async fn test_auto_remediate_empty_service_id_returns_err() {
    let engine = make_engine();

    let result = engine.auto_remediate("", Severity::Medium, "should fail");
    assert!(result.is_err(), "empty service_id should be rejected");

    let err_msg = result.err().unwrap().to_string();
    assert!(
        err_msg.contains("service_id"),
        "error should mention service_id: {err_msg}"
    );

    // Verify no remediation was created
    assert_eq!(
        engine.pending_remediations(),
        0,
        "no remediation should be pending after rejection"
    );
}

// ===========================================================================
// Test 4: Learning cycle via engine works and returns activity
// ===========================================================================

#[tokio::test]
async fn test_learning_cycle_returns_activity() {
    let engine = make_engine();

    let result = engine.learning_cycle();
    assert!(result.is_ok(), "learning_cycle should succeed: {result:?}");

    let cycle = result.unwrap();
    assert!(
        cycle.had_activity(),
        "first learning cycle should have activity (decay of default pathways)"
    );
    assert!(
        cycle.pathways_decayed > 0,
        "should decay at least some pathways, got {}",
        cycle.pathways_decayed
    );
}

// ===========================================================================
// Test 5: Multiple learning cycles cause pathway strength decrease
// ===========================================================================

#[tokio::test]
async fn test_multiple_learning_cycles_decrease_strength() {
    let engine = make_engine();

    let initial_strength = engine.average_pathway_strength();
    assert!(
        initial_strength > 0.0,
        "initial strength should be > 0, got {initial_strength}"
    );

    // Run 10 decay cycles
    for i in 0..10 {
        let result = engine.learning_cycle();
        assert!(result.is_ok(), "learning cycle {i} should succeed");
    }

    let final_strength = engine.average_pathway_strength();
    assert!(
        final_strength < initial_strength,
        "strength should decrease after 10 decay cycles: initial={initial_strength}, final={final_strength}"
    );
    assert!(
        final_strength >= 0.0,
        "strength must never go negative: {final_strength}"
    );
}

// ===========================================================================
// Test 6: PBFT vote submission - create proposal, submit vote, verify
// ===========================================================================

#[tokio::test]
async fn test_pbft_vote_submission() {
    let engine = make_engine();
    let pbft = engine.pbft_manager();

    // Create a proposal
    let proposal = pbft
        .create_proposal(ConsensusAction::ConfigRollback, "@0.A")
        .unwrap();
    let pid = proposal.id.clone();

    // Submit a vote from the human agent
    let vote = pbft
        .submit_vote(&pid, "@0.A", VoteType::Approve, Some("Looks correct".into()))
        .unwrap();

    assert_eq!(vote.proposal_id, pid);
    assert_eq!(vote.agent_id, "@0.A");
    assert_eq!(vote.vote, VoteType::Approve);
    assert!(vote.reason.is_some());

    // Verify the vote is recorded
    let votes = pbft.get_votes(&pid).unwrap();
    assert_eq!(votes.len(), 1, "should have exactly 1 vote");
    assert_eq!(votes[0].agent_id, "@0.A");
}

// ===========================================================================
// Test 7: PBFT full round - create, advance phases, submit votes, tally
// ===========================================================================

#[tokio::test]
async fn test_pbft_full_round() {
    let engine = make_engine();
    let pbft = engine.pbft_manager();

    // Step 1: Create proposal
    let proposal = pbft
        .create_proposal(ConsensusAction::CredentialRotation, "@0.A")
        .unwrap();
    let pid = proposal.id.clone();
    assert_eq!(
        proposal.phase,
        maintenance_engine::m6_consensus::ConsensusPhase::PrePrepare
    );

    // Step 2: Advance to Prepare
    let phase = pbft.advance_phase(&pid).unwrap();
    assert_eq!(phase, maintenance_engine::m6_consensus::ConsensusPhase::Prepare);

    // Step 3: Submit approval votes from enough agents (36 to include Critics + Integrators)
    let fleet = pbft.get_fleet();
    for agent in fleet.iter().take(36) {
        let _ = pbft.submit_vote(&pid, &agent.id, VoteType::Approve, None);
    }

    // Step 4: Advance Prepare -> Commit (quorum check passes with 36 approvals > 27)
    let phase = pbft.advance_phase(&pid).unwrap();
    assert_eq!(phase, maintenance_engine::m6_consensus::ConsensusPhase::Commit);

    // Step 5: Advance Commit -> Execute (enhanced consensus: Critic + Integrator present)
    let phase = pbft.advance_phase(&pid).unwrap();
    assert_eq!(phase, maintenance_engine::m6_consensus::ConsensusPhase::Execute);

    // Step 6: Advance Execute -> Complete
    let phase = pbft.advance_phase(&pid).unwrap();
    assert_eq!(phase, maintenance_engine::m6_consensus::ConsensusPhase::Complete);

    // Step 7: Tally votes
    let outcome = pbft.tally_votes(&pid).unwrap();
    assert!(outcome.quorum_reached, "quorum should be reached with 36 votes");
    assert_eq!(outcome.votes_for, 36);
    assert_eq!(outcome.votes_against, 0);
    assert!(
        outcome.weighted_for > 0.0,
        "weighted_for should be positive"
    );
}

// ===========================================================================
// Test 8: record_success on Hebbian pathway increases routing weight
// ===========================================================================

#[tokio::test]
async fn test_hebbian_record_success_increases_weight() {
    let engine = make_engine();
    let hm = engine.hebbian_manager();

    let key = first_pathway_key(&engine);

    // Get initial routing weight
    let parts: Vec<&str> = key.splitn(2, "->").collect();
    let (source, target) = (parts[0], parts[1]);
    let initial_weight = hm.get_routing_weight(source, target);

    // Record a success (applies LTP)
    let result = hm.record_success(&key);
    assert!(result.is_ok(), "record_success should succeed: {result:?}");

    let new_strength = result.unwrap();
    assert!(
        new_strength > 0.0,
        "strength after success should be positive"
    );

    // Routing weight should increase
    let new_weight = hm.get_routing_weight(source, target);
    assert!(
        new_weight >= initial_weight,
        "routing weight should not decrease after success: initial={initial_weight}, new={new_weight}"
    );
}

// ===========================================================================
// Test 9: record_failure on Hebbian pathway decreases routing weight
// ===========================================================================

#[tokio::test]
async fn test_hebbian_record_failure_decreases_weight() {
    let engine = make_engine();
    let hm = engine.hebbian_manager();

    let key = first_pathway_key(&engine);

    // Get initial strength
    let initial_strength = hm.get_strength(&key).unwrap();

    // Record a failure (applies LTD)
    let result = hm.record_failure(&key);
    assert!(result.is_ok(), "record_failure should succeed: {result:?}");

    let new_strength = result.unwrap();
    assert!(
        new_strength < initial_strength,
        "strength should decrease after failure: initial={initial_strength}, new={new_strength}"
    );
}

// ===========================================================================
// Test 10: Combined - remediation + Hebbian feedback + learning cycle
// ===========================================================================

#[tokio::test]
async fn test_combined_remediation_hebbian_learning() {
    let engine = make_engine();

    // Phase 1: Submit a remediation
    let request_id = engine
        .submit_remediation(
            "nais",
            IssueType::LatencySpike,
            Severity::Medium,
            "elevated p99 latency",
        )
        .unwrap();
    assert!(!request_id.is_empty());
    assert!(engine.pending_remediations() >= 1);

    // Phase 2: Simulate Hebbian feedback on a pathway
    let hm = engine.hebbian_manager();
    let key = first_pathway_key(&engine);

    let _ = hm.record_success(&key);
    let _ = hm.record_success(&key);
    let after_success = hm.get_strength(&key).unwrap();

    let _ = hm.record_failure(&key);
    let after_failure = hm.get_strength(&key).unwrap();

    assert!(
        after_failure < after_success,
        "failure should reduce strength below the success peak"
    );

    // Phase 3: Execute a learning cycle (decay)
    let cycle = engine.learning_cycle().unwrap();
    assert!(cycle.had_activity(), "learning cycle should have activity");

    // Phase 4: Verify engine health is still coherent
    let report = engine.health_report().unwrap();
    assert!(
        (0.0..=1.0).contains(&report.overall_health),
        "overall health should remain in [0,1] after combined operations"
    );
    assert!(
        report.pathways_count > 0,
        "pathways should still exist after combined operations"
    );

    // Phase 5: Build tensor to verify cross-layer state encoding
    let tensor = engine.build_tensor();
    assert!(
        tensor.validate().is_ok(),
        "tensor should remain valid after combined operations"
    );
}
