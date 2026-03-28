//! # Layer 6 Consensus Integration Tests
//!
//! Comprehensive integration tests covering the full L6 consensus module surface:
//! - PBFT constants and Byzantine invariants
//! - `ConsensusProposal` creation and phase lifecycle
//! - `PbftManager` proposal/vote/tally workflows
//! - `VoteCollector` ballot management and duplicate rejection
//! - `DissentTracker` recording, marking, and analysis
//! - `AgentCoordinator` fleet creation and role distribution
//! - `ViewChangeHandler` view change lifecycle
//! - `QuorumCalculator` simple, weighted, and enhanced quorum
//! - Default fleet composition (41 agents)
//! - Full consensus flow: propose -> vote -> tally -> quorum -> execute/reject

mod common;

use maintenance_engine::m6_consensus::{
    self,
    pbft::PbftManager,
    agent::AgentCoordinator,
    voting::{BallotStatus, VoteCollector},
    view_change::{ViewChangeHandler, ViewChangeReason, ViewChangeState},
    dissent::DissentTracker,
    quorum::{QuorumCalculator, QuorumConfig, QuorumRequirement},
    ConsensusAction, ConsensusPhase, ConsensusProposal, ConsensusVote,
    ExecutionStatus, VoteType,
    PBFT_F, PBFT_N, PBFT_Q,
};
use maintenance_engine::AgentRole;

use common::{
    make_approve_vote, make_proposal, make_reject_vote,
};

// =========================================================================
// Category 1: PBFT Constants Verification
// =========================================================================

#[test]
fn pbft_n_equals_40() {
    assert_eq!(PBFT_N, 40, "PBFT total agents must be 40");
}

#[test]
fn pbft_f_equals_13() {
    assert_eq!(PBFT_F, 13, "Byzantine fault tolerance must be 13");
}

#[test]
fn pbft_q_equals_27() {
    assert_eq!(PBFT_Q, 27, "Quorum requirement must be 27");
}

#[test]
fn pbft_invariant_n_equals_3f_plus_1() {
    // The PBFT invariant: n = 3f + 1
    assert_eq!(PBFT_N, 3 * PBFT_F + 1, "PBFT invariant n = 3f + 1 violated");
}

#[test]
fn pbft_invariant_q_equals_2f_plus_1() {
    // The quorum invariant: q = 2f + 1
    assert_eq!(PBFT_Q, 2 * PBFT_F + 1, "Quorum invariant q = 2f + 1 violated");
}

// =========================================================================
// Category 2: ConsensusProposal Creation and Phase Advancement
// =========================================================================

#[test]
fn proposal_creation_sets_preprepare_phase() {
    let proposal = ConsensusProposal::new(
        "test-001",
        0,
        1,
        ConsensusAction::ServiceTermination,
        "@0.A",
    );
    assert_eq!(proposal.phase, ConsensusPhase::PrePrepare);
    assert_eq!(proposal.id, "test-001");
    assert_eq!(proposal.view_number, 0);
    assert_eq!(proposal.sequence_number, 1);
    assert_eq!(proposal.action_type, ConsensusAction::ServiceTermination);
    assert_eq!(proposal.proposer, "@0.A");
}

#[test]
fn proposal_get_timeout_per_action_type() {
    let term = make_proposal("t1", ConsensusAction::ServiceTermination);
    let migr = make_proposal("t2", ConsensusAction::DatabaseMigration);
    let cred = make_proposal("t3", ConsensusAction::CredentialRotation);
    let casc = make_proposal("t4", ConsensusAction::CascadeRestart);
    let roll = make_proposal("t5", ConsensusAction::ConfigRollback);

    assert_eq!(term.get_timeout(), 60_000);
    assert_eq!(migr.get_timeout(), 300_000);
    assert_eq!(cred.get_timeout(), 120_000);
    assert_eq!(casc.get_timeout(), 180_000);
    assert_eq!(roll.get_timeout(), 90_000);
}

#[test]
fn consensus_action_required_quorum_is_pbft_q() {
    let actions = [
        ConsensusAction::ServiceTermination,
        ConsensusAction::DatabaseMigration,
        ConsensusAction::CredentialRotation,
        ConsensusAction::CascadeRestart,
        ConsensusAction::ConfigRollback,
    ];
    for action in &actions {
        assert_eq!(
            action.required_quorum(),
            PBFT_Q,
            "Action {action:?} must require PBFT_Q quorum"
        );
    }
}

// =========================================================================
// Category 3: PbftManager Proposal Submission, Voting, and Quorum
// =========================================================================

#[test]
fn pbft_manager_create_proposal_and_submit_votes() {
    let mgr = PbftManager::new();
    let proposal = mgr
        .create_proposal(ConsensusAction::ConfigRollback, "@0.A")
        .unwrap_or_else(|_| unreachable!());

    assert_eq!(proposal.phase, ConsensusPhase::PrePrepare);
    assert_eq!(mgr.proposal_count(), 1);

    let pid = proposal.id;

    // Submit a vote from the human agent
    let vote = mgr
        .submit_vote(&pid, "@0.A", VoteType::Approve, None)
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(vote.agent_id, "@0.A");
    assert_eq!(vote.vote, VoteType::Approve);
    assert_eq!(vote.role, AgentRole::Validator);

    let votes = mgr.get_votes(&pid).unwrap_or_else(|_| unreachable!());
    assert_eq!(votes.len(), 1);
}

#[test]
fn pbft_manager_empty_proposer_rejected() {
    let mgr = PbftManager::new();
    let result = mgr.create_proposal(ConsensusAction::ServiceTermination, "");
    assert!(result.is_err());
}

#[test]
fn pbft_manager_full_phase_lifecycle() {
    let mgr = PbftManager::new();
    let proposal = mgr
        .create_proposal(ConsensusAction::ConfigRollback, "@0.A")
        .unwrap_or_else(|_| unreachable!());
    let pid = proposal.id;

    // PrePrepare -> Prepare
    let phase = mgr.advance_phase(&pid).unwrap_or_else(|_| unreachable!());
    assert_eq!(phase, ConsensusPhase::Prepare);

    // Submit 36 approval votes (includes validators, explorers, critics, integrators)
    let fleet = mgr.get_fleet();
    for agent in fleet.iter().take(36) {
        let _ = mgr.submit_vote(&pid, &agent.id, VoteType::Approve, None);
    }

    // Prepare -> Commit (quorum check passes with 36 >= 27)
    let phase = mgr.advance_phase(&pid).unwrap_or_else(|_| unreachable!());
    assert_eq!(phase, ConsensusPhase::Commit);

    // Commit -> Execute (enhanced consensus: critic + integrator present in first 36)
    let phase = mgr.advance_phase(&pid).unwrap_or_else(|_| unreachable!());
    assert_eq!(phase, ConsensusPhase::Execute);

    // Execute -> Complete
    let phase = mgr.advance_phase(&pid).unwrap_or_else(|_| unreachable!());
    assert_eq!(phase, ConsensusPhase::Complete);
}

#[test]
fn pbft_manager_quorum_rejection_on_insufficient_votes() {
    let mgr = PbftManager::new();
    let proposal = mgr
        .create_proposal(ConsensusAction::DatabaseMigration, "@0.A")
        .unwrap_or_else(|_| unreachable!());
    let pid = proposal.id;

    // Advance to Prepare
    let _ = mgr.advance_phase(&pid);

    // Submit only 10 approval votes (below quorum of 27)
    let fleet = mgr.get_fleet();
    for agent in fleet.iter().take(10) {
        let _ = mgr.submit_vote(&pid, &agent.id, VoteType::Approve, None);
    }

    // Prepare -> Commit should fail due to insufficient quorum
    let result = mgr.advance_phase(&pid);
    assert!(result.is_err());
}

#[test]
fn pbft_manager_tally_with_quorum_reached() {
    let mgr = PbftManager::new();
    let proposal = mgr
        .create_proposal(ConsensusAction::CascadeRestart, "@0.A")
        .unwrap_or_else(|_| unreachable!());
    let pid = proposal.id;

    // Submit 30 approval votes
    let fleet = mgr.get_fleet();
    for agent in fleet.iter().take(30) {
        let _ = mgr.submit_vote(&pid, &agent.id, VoteType::Approve, None);
    }

    let outcome = mgr.tally_votes(&pid).unwrap_or_else(|_| unreachable!());
    assert!(outcome.quorum_reached);
    assert_eq!(outcome.votes_for, 30);
    assert_eq!(outcome.votes_against, 0);
    assert_eq!(outcome.votes_abstained, 0);
    assert_eq!(outcome.execution_status, ExecutionStatus::Pending);
}

#[test]
fn pbft_manager_tally_without_quorum_aborts() {
    let mgr = PbftManager::new();
    let proposal = mgr
        .create_proposal(ConsensusAction::CredentialRotation, "@0.A")
        .unwrap_or_else(|_| unreachable!());
    let pid = proposal.id;

    // Submit only 5 votes
    let fleet = mgr.get_fleet();
    for agent in fleet.iter().take(5) {
        let _ = mgr.submit_vote(&pid, &agent.id, VoteType::Approve, None);
    }

    let outcome = mgr.tally_votes(&pid).unwrap_or_else(|_| unreachable!());
    assert!(!outcome.quorum_reached);
    assert_eq!(outcome.votes_for, 5);
    assert_eq!(outcome.execution_status, ExecutionStatus::Aborted);
}

// =========================================================================
// Category 4: VoteCollector - Ballot Lifecycle and Duplicate Rejection
// =========================================================================

#[test]
fn vote_collector_open_and_close_ballot() {
    let collector = VoteCollector::new();
    let result = collector.open_ballot("prop-1");
    assert!(result.is_ok());

    let ballot = collector.get_ballot("prop-1").unwrap_or_else(|_| unreachable!());
    assert_eq!(ballot.status, BallotStatus::Open);
    assert!(ballot.votes.is_empty());
    assert!(ballot.closed_at.is_none());

    let close_result = collector.close_ballot("prop-1");
    assert!(close_result.is_ok());

    let closed_ballot = collector.get_ballot("prop-1").unwrap_or_else(|_| unreachable!());
    assert_eq!(closed_ballot.status, BallotStatus::Closed);
    assert!(closed_ballot.closed_at.is_some());
}

#[test]
fn vote_collector_cast_and_tally_votes() {
    let collector = VoteCollector::new();
    let _ = collector.open_ballot("prop-1");

    // Cast 5 approval, 2 rejection, 1 abstention
    for i in 1..=5 {
        let vote = make_approve_vote("prop-1", &format!("v-{i:02}"), AgentRole::Validator);
        let result = collector.cast_vote("prop-1", vote);
        assert!(result.is_ok());
    }
    for i in 1..=2 {
        let vote = make_reject_vote("prop-1", &format!("c-{i:02}"), AgentRole::Critic);
        let result = collector.cast_vote("prop-1", vote);
        assert!(result.is_ok());
    }
    let abstain_vote = ConsensusVote {
        proposal_id: "prop-1".into(),
        agent_id: "e-01".into(),
        vote: VoteType::Abstain,
        phase: ConsensusPhase::Prepare,
        role: AgentRole::Explorer,
        weight: 0.8,
        reason: None,
        timestamp: std::time::SystemTime::now(),
    };
    let _ = collector.cast_vote("prop-1", abstain_vote);

    let tally = collector.get_tally("prop-1").unwrap_or_else(|_| unreachable!());
    assert_eq!(tally.votes_for, 5);
    assert_eq!(tally.votes_against, 2);
    assert_eq!(tally.votes_abstain, 1);
    assert_eq!(tally.total_votes, 8);
}

#[test]
fn vote_collector_rejects_duplicate_vote() {
    let collector = VoteCollector::new();
    let _ = collector.open_ballot("prop-1");

    let vote1 = make_approve_vote("prop-1", "agent-01", AgentRole::Validator);
    let result1 = collector.cast_vote("prop-1", vote1);
    assert!(result1.is_ok());

    // Second vote from same agent should fail
    let vote2 = make_reject_vote("prop-1", "agent-01", AgentRole::Validator);
    let result2 = collector.cast_vote("prop-1", vote2);
    assert!(result2.is_err());
}

#[test]
fn vote_collector_cannot_vote_on_closed_ballot() {
    let collector = VoteCollector::new();
    let _ = collector.open_ballot("prop-1");
    let _ = collector.close_ballot("prop-1");

    let vote = make_approve_vote("prop-1", "agent-01", AgentRole::Validator);
    let result = collector.cast_vote("prop-1", vote);
    assert!(result.is_err());
}

#[test]
fn vote_collector_votes_by_role_filter() {
    let collector = VoteCollector::new();
    let _ = collector.open_ballot("prop-1");

    let _ = collector.cast_vote("prop-1", make_approve_vote("prop-1", "v-01", AgentRole::Validator));
    let _ = collector.cast_vote("prop-1", make_reject_vote("prop-1", "c-01", AgentRole::Critic));
    let _ = collector.cast_vote("prop-1", make_approve_vote("prop-1", "c-02", AgentRole::Critic));

    let critic_votes = collector.get_votes_by_role("prop-1", AgentRole::Critic);
    assert_eq!(critic_votes.len(), 2);

    let validator_votes = collector.get_votes_by_role("prop-1", AgentRole::Validator);
    assert_eq!(validator_votes.len(), 1);

    let explorer_votes = collector.get_votes_by_role("prop-1", AgentRole::Explorer);
    assert!(explorer_votes.is_empty());
}

// =========================================================================
// Category 5: DissentTracker
// =========================================================================

#[test]
fn dissent_tracker_record_and_retrieve() {
    let tracker = DissentTracker::new();
    let event = tracker
        .record_dissent(
            "prop-1",
            "agent-29",
            AgentRole::Critic,
            "Risk of data loss during migration".into(),
        )
        .unwrap_or_else(|_| unreachable!());

    assert_eq!(event.proposed_action, "prop-1");
    assert!(event.dissenting_agent.contains("agent-29"));
    assert!(event.dissenting_agent.contains("Critic"));
    assert!(event.was_valuable.is_none());
    assert_eq!(tracker.total_dissent(), 1);

    let prop_dissent = tracker.get_dissent_for_proposal("prop-1");
    assert_eq!(prop_dissent.len(), 1);
}

#[test]
fn dissent_tracker_mark_valuable_and_analysis() {
    let tracker = DissentTracker::new();

    let e1 = tracker
        .record_dissent("prop-1", "agent-29", AgentRole::Critic, "Too risky".into())
        .unwrap_or_else(|_| unreachable!());
    let _ = tracker.record_dissent("prop-1", "agent-30", AgentRole::Critic, "Too risky".into());
    let _ = tracker.record_dissent(
        "prop-1",
        "agent-35",
        AgentRole::Integrator,
        "Cross-system impact".into(),
    );

    // Mark first event as valuable
    let mark_result = tracker.mark_valuable(&e1.id);
    assert!(mark_result.is_ok());

    let valuable = tracker.get_valuable_dissent();
    assert_eq!(valuable.len(), 1);

    // Analyze dissent for prop-1
    let analysis = tracker
        .analyze_dissent("prop-1")
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(analysis.total_dissent, 3);
    assert_eq!(analysis.valuable_count, 1);
    assert_eq!(analysis.dissent_by_role.get("Critic"), Some(&2));
    assert_eq!(analysis.dissent_by_role.get("Integrator"), Some(&1));
}

#[test]
fn dissent_tracker_rate_calculation() {
    let tracker = DissentTracker::new();
    assert!((tracker.dissent_rate() - 0.0).abs() < f64::EPSILON);

    // 3 dissent events across 2 proposals -> rate = 3/2 = 1.5
    let _ = tracker.record_dissent("prop-1", "agent-29", AgentRole::Critic, "Risk A".into());
    let _ = tracker.record_dissent("prop-1", "agent-30", AgentRole::Critic, "Risk B".into());
    let _ = tracker.record_dissent("prop-2", "agent-29", AgentRole::Critic, "Risk C".into());

    let rate = tracker.dissent_rate();
    assert!((rate - 1.5).abs() < f64::EPSILON);
}

#[test]
fn dissent_tracker_validation_rejects_empty_fields() {
    let tracker = DissentTracker::new();

    let r1 = tracker.record_dissent("", "agent-01", AgentRole::Validator, "reason".into());
    assert!(r1.is_err());

    let r2 = tracker.record_dissent("prop-1", "", AgentRole::Validator, "reason".into());
    assert!(r2.is_err());

    let r3 = tracker.record_dissent("prop-1", "agent-01", AgentRole::Validator, String::new());
    assert!(r3.is_err());
}

// =========================================================================
// Category 6: AgentCoordinator - Fleet Creation and Role Distribution
// =========================================================================

#[test]
fn agent_coordinator_has_41_agents() {
    let coord = AgentCoordinator::new();
    assert_eq!(coord.agent_count(), 41);
}

#[test]
fn agent_coordinator_human_at_0a_is_present() {
    let coord = AgentCoordinator::new();
    let human = coord.human_agent();
    assert!(human.is_some());

    let human = human.unwrap_or_else(|| unreachable!());
    assert_eq!(human.id, "@0.A");
    assert_eq!(human.tier, 0);
    assert_eq!(human.role, AgentRole::Validator);
    assert!((human.weight - 1.0).abs() < f64::EPSILON);
    assert!((human.success_rate - 1.0).abs() < f64::EPSILON);
}

#[test]
fn agent_coordinator_role_counts_match_spec() {
    let coord = AgentCoordinator::new();

    // 20 validators + 1 Human @0.A (who is also Validator)
    let validators = coord.agents_by_role(AgentRole::Validator);
    assert_eq!(validators.len(), 21);

    let explorers = coord.agents_by_role(AgentRole::Explorer);
    assert_eq!(explorers.len(), 8);

    let critics = coord.agents_by_role(AgentRole::Critic);
    assert_eq!(critics.len(), 6);

    let integrators = coord.agents_by_role(AgentRole::Integrator);
    assert_eq!(integrators.len(), 4);

    let historians = coord.agents_by_role(AgentRole::Historian);
    assert_eq!(historians.len(), 2);
}

#[test]
fn agent_coordinator_task_assignment_and_completion() {
    let coord = AgentCoordinator::new();

    let task_id = coord
        .assign_task("agent-01", "Run health check")
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(task_id, "task-0000");
    assert_eq!(coord.task_count(), 1);

    // Agent should be busy
    let agent = coord.get_agent("agent-01").unwrap_or_else(|| unreachable!());
    assert_eq!(agent.status, m6_consensus::AgentStatus::Busy);

    // Complete the task
    let result = coord.complete_task(&task_id, true);
    assert!(result.is_ok());

    // Agent should be active now
    let agent_after = coord.get_agent("agent-01").unwrap_or_else(|| unreachable!());
    assert_eq!(agent_after.status, m6_consensus::AgentStatus::Active);
}

#[test]
fn agent_coordinator_fleet_health_default() {
    let coord = AgentCoordinator::new();
    let health = coord.fleet_health();
    assert_eq!(health.total_agents, 41);
    // Human is Active, 40 others are Idle -> all operational
    assert_eq!(health.active_agents, 1);
    assert_eq!(health.idle_agents, 40);
    assert_eq!(health.failed_agents, 0);
    assert!((health.health_score - 1.0).abs() < f64::EPSILON);
}

// =========================================================================
// Category 7: ViewChangeHandler
// =========================================================================

#[test]
fn view_change_initial_state() {
    let fleet_ids: Vec<String> = (0..41).map(|i| format!("agent-{i:02}")).collect();
    let handler = ViewChangeHandler::new(fleet_ids);
    assert_eq!(handler.current_view(), 0);

    let primary = handler.current_primary().unwrap_or_else(|_| unreachable!());
    assert_eq!(primary, "agent-00");
    assert!(!handler.is_view_change_in_progress());
    assert_eq!(handler.change_count(), 0);
}

#[test]
fn view_change_request_and_execute() {
    let fleet_ids: Vec<String> = (0..5).map(|i| format!("agent-{i:02}")).collect();
    let handler = ViewChangeHandler::new(fleet_ids);

    // Request view change
    let req_result =
        handler.request_view_change(ViewChangeReason::ProposalTimeout, "agent-01");
    assert!(req_result.is_ok());
    assert!(handler.is_view_change_in_progress());

    let state = handler.view_state().unwrap_or_else(|_| unreachable!());
    assert_eq!(state.state, ViewChangeState::Requested);

    // Execute view change
    let record = handler.execute_view_change().unwrap_or_else(|_| unreachable!());
    assert_eq!(record.from_view, 0);
    assert_eq!(record.to_view, 1);
    assert!(record.success);
    assert_eq!(record.old_primary, "agent-00");
    assert_eq!(record.new_primary, "agent-01");

    // View should have advanced
    assert_eq!(handler.current_view(), 1);
    assert!(!handler.is_view_change_in_progress());
    assert_eq!(handler.change_count(), 1);
}

#[test]
fn view_change_timeout_action_values() {
    let fleet_ids: Vec<String> = (0..5).map(|i| format!("agent-{i:02}")).collect();
    let handler = ViewChangeHandler::new(fleet_ids);

    assert_eq!(
        handler.timeout_for_action(&ConsensusAction::ServiceTermination),
        60_000
    );
    assert_eq!(
        handler.timeout_for_action(&ConsensusAction::DatabaseMigration),
        300_000
    );
    assert_eq!(
        handler.timeout_for_action(&ConsensusAction::CredentialRotation),
        120_000
    );
    assert_eq!(
        handler.timeout_for_action(&ConsensusAction::CascadeRestart),
        180_000
    );
    assert_eq!(
        handler.timeout_for_action(&ConsensusAction::ConfigRollback),
        90_000
    );
}

#[test]
fn view_change_primary_rotates_deterministically() {
    let fleet_ids: Vec<String> = (0..3).map(|i| format!("agent-{i:02}")).collect();
    let handler = ViewChangeHandler::new(fleet_ids);

    // View 0 -> agent-00
    assert_eq!(handler.primary_for_view(0), "agent-00");
    // View 1 -> agent-01
    assert_eq!(handler.primary_for_view(1), "agent-01");
    // View 2 -> agent-02
    assert_eq!(handler.primary_for_view(2), "agent-02");
    // View 3 -> wraps to agent-00
    assert_eq!(handler.primary_for_view(3), "agent-00");
}

#[test]
fn view_change_execute_without_request_fails() {
    let fleet_ids: Vec<String> = (0..5).map(|i| format!("agent-{i:02}")).collect();
    let handler = ViewChangeHandler::new(fleet_ids);
    let result = handler.execute_view_change();
    assert!(result.is_err());
}

// =========================================================================
// Category 8: QuorumCalculator - Standard, Weighted, and Enhanced
// =========================================================================

#[test]
fn quorum_calculator_simple_quorum_met_at_27() {
    let votes: Vec<ConsensusVote> = (0..27)
        .map(|i| make_approve_vote("prop-1", &format!("v-{i:03}"), AgentRole::Validator))
        .collect();
    let (met, vf, va, ab) = QuorumCalculator::simple_quorum_check(&votes);
    assert!(met);
    assert_eq!(vf, 27);
    assert_eq!(va, 0);
    assert_eq!(ab, 0);
}

#[test]
fn quorum_calculator_simple_quorum_not_met_at_26() {
    let votes: Vec<ConsensusVote> = (0..26)
        .map(|i| make_approve_vote("prop-1", &format!("v-{i:03}"), AgentRole::Validator))
        .collect();
    let (met, vf, _, _) = QuorumCalculator::simple_quorum_check(&votes);
    assert!(!met);
    assert_eq!(vf, 26);
}

#[test]
fn quorum_calculator_weighted_quorum_with_critic_weight() {
    // 22 validators (22.0) + 5 critics at 1.2 (6.0) = 28.0 >= 27.0
    let mut votes: Vec<ConsensusVote> = (0..22)
        .map(|i| make_approve_vote("prop-1", &format!("v-{i:03}"), AgentRole::Validator))
        .collect();
    for i in 0..5 {
        votes.push(make_approve_vote(
            "prop-1",
            &format!("c-{i:03}"),
            AgentRole::Critic,
        ));
    }
    let (met, wf, _, _) = QuorumCalculator::weighted_quorum_check(&votes);
    assert!(met);
    assert!((wf - 28.0).abs() < 1e-10);
}

#[test]
fn quorum_calculator_enhanced_requires_critic_and_integrator() {
    // Only critic approval, no integrator
    let critic_only = vec![make_approve_vote("prop-1", "c-01", AgentRole::Critic)];
    let (has_critic, has_integrator) = QuorumCalculator::enhanced_quorum_check(&critic_only);
    assert!(has_critic);
    assert!(!has_integrator);

    // Only integrator approval, no critic
    let integrator_only = vec![make_approve_vote("prop-1", "i-01", AgentRole::Integrator)];
    let (has_critic2, has_integrator2) =
        QuorumCalculator::enhanced_quorum_check(&integrator_only);
    assert!(!has_critic2);
    assert!(has_integrator2);

    // Both present
    let both = vec![
        make_approve_vote("prop-1", "c-01", AgentRole::Critic),
        make_approve_vote("prop-1", "i-01", AgentRole::Integrator),
    ];
    let (hc, hi) = QuorumCalculator::enhanced_quorum_check(&both);
    assert!(hc);
    assert!(hi);
}

#[test]
fn quorum_calculator_evaluate_critical_action_requires_enhanced() {
    let calc = QuorumCalculator::new();
    // 27 validators but no critic/integrator
    let votes: Vec<ConsensusVote> = (0..27)
        .map(|i| make_approve_vote("prop-1", &format!("v-{i:03}"), AgentRole::Validator))
        .collect();
    let result = calc
        .evaluate("prop-1", ConsensusAction::ServiceTermination, &votes)
        .unwrap_or_else(|_| unreachable!());
    assert!(!result.met);
    assert_eq!(result.requirement, QuorumRequirement::Enhanced);
    assert!(!result.has_critic_approval);
    assert!(!result.has_integrator_approval);
}

#[test]
fn quorum_calculator_full_quorum_with_roles_passes() {
    let calc = QuorumCalculator::new();

    let mut votes: Vec<ConsensusVote> = (0..25)
        .map(|i| make_approve_vote("prop-1", &format!("v-{i:03}"), AgentRole::Validator))
        .collect();
    votes.push(make_approve_vote("prop-1", "critic-01", AgentRole::Critic));
    votes.push(make_approve_vote(
        "prop-1",
        "integ-01",
        AgentRole::Integrator,
    ));

    let result = calc
        .evaluate("prop-1", ConsensusAction::ConfigRollback, &votes)
        .unwrap_or_else(|_| unreachable!());
    assert!(result.met);
    assert_eq!(result.votes_for, 27);
    assert!(result.has_critic_approval);
    assert!(result.has_integrator_approval);
}

// =========================================================================
// Category 9: Default Fleet Composition (41 agents)
// =========================================================================

#[test]
fn default_fleet_total_is_41() {
    let fleet = m6_consensus::default_agent_fleet();
    assert_eq!(fleet.len(), 41, "Fleet must contain 41 agents (40 + Human @0.A)");
}

#[test]
fn default_fleet_has_20_validators_plus_human() {
    let fleet = m6_consensus::default_agent_fleet();
    let validator_count = fleet.iter().filter(|a| a.role == AgentRole::Validator).count();
    // 20 CVA-NAM validators + 1 Human @0.A (who is Validator role)
    assert_eq!(validator_count, 21);
}

#[test]
fn default_fleet_has_8_explorers() {
    let fleet = m6_consensus::default_agent_fleet();
    let count = fleet.iter().filter(|a| a.role == AgentRole::Explorer).count();
    assert_eq!(count, 8);
}

#[test]
fn default_fleet_has_6_critics() {
    let fleet = m6_consensus::default_agent_fleet();
    let count = fleet.iter().filter(|a| a.role == AgentRole::Critic).count();
    assert_eq!(count, 6);
}

#[test]
fn default_fleet_has_4_integrators() {
    let fleet = m6_consensus::default_agent_fleet();
    let count = fleet.iter().filter(|a| a.role == AgentRole::Integrator).count();
    assert_eq!(count, 4);
}

#[test]
fn default_fleet_has_2_historians() {
    let fleet = m6_consensus::default_agent_fleet();
    let count = fleet.iter().filter(|a| a.role == AgentRole::Historian).count();
    assert_eq!(count, 2);
}

#[test]
fn default_fleet_human_agent_is_tier_zero() {
    let human = m6_consensus::create_human_agent();
    assert_eq!(human.id, "@0.A");
    assert_eq!(human.tier, 0);
    assert_eq!(human.role, AgentRole::Validator);
    assert_eq!(human.status, m6_consensus::AgentStatus::Active);
    assert!((human.success_rate - 1.0).abs() < f64::EPSILON);
}

// =========================================================================
// Category 10: Full Consensus Flow
// =========================================================================

#[test]
fn full_consensus_flow_propose_vote_tally_quorum_execute() {
    // 1. Create PbftManager and proposal
    let mgr = PbftManager::new();
    let proposal = mgr
        .create_proposal(ConsensusAction::ConfigRollback, "@0.A")
        .unwrap_or_else(|_| unreachable!());
    let pid = proposal.id;
    assert_eq!(proposal.phase, ConsensusPhase::PrePrepare);

    // 2. Advance to Prepare
    let phase = mgr.advance_phase(&pid).unwrap_or_else(|_| unreachable!());
    assert_eq!(phase, ConsensusPhase::Prepare);

    // 3. Submit votes from 36 agents (includes critics and integrators)
    let fleet = mgr.get_fleet();
    for agent in fleet.iter().take(36) {
        let _ = mgr.submit_vote(&pid, &agent.id, VoteType::Approve, None);
    }

    // 4. Tally votes
    let outcome = mgr.tally_votes(&pid).unwrap_or_else(|_| unreachable!());
    assert!(outcome.quorum_reached);
    assert_eq!(outcome.votes_for, 36);
    assert_eq!(outcome.execution_status, ExecutionStatus::Pending);

    // 5. Advance Prepare -> Commit (quorum check passes)
    let phase = mgr.advance_phase(&pid).unwrap_or_else(|_| unreachable!());
    assert_eq!(phase, ConsensusPhase::Commit);

    // 6. Advance Commit -> Execute (enhanced consensus passes)
    let phase = mgr.advance_phase(&pid).unwrap_or_else(|_| unreachable!());
    assert_eq!(phase, ConsensusPhase::Execute);

    // 7. Advance Execute -> Complete
    let phase = mgr.advance_phase(&pid).unwrap_or_else(|_| unreachable!());
    assert_eq!(phase, ConsensusPhase::Complete);

    // 8. Verify final state
    let final_proposal = mgr.get_proposal(&pid).unwrap_or_else(|_| unreachable!());
    assert_eq!(final_proposal.phase, ConsensusPhase::Complete);
}

#[test]
fn full_consensus_flow_rejected_due_to_no_quorum() {
    let mgr = PbftManager::new();
    let proposal = mgr
        .create_proposal(ConsensusAction::ServiceTermination, "@0.A")
        .unwrap_or_else(|_| unreachable!());
    let pid = proposal.id;

    // Advance to Prepare
    let _ = mgr.advance_phase(&pid);

    // Only 5 agents approve, 10 reject
    let fleet = mgr.get_fleet();
    for agent in fleet.iter().take(5) {
        let _ = mgr.submit_vote(&pid, &agent.id, VoteType::Approve, None);
    }
    for agent in fleet.iter().skip(5).take(10) {
        let _ = mgr.submit_vote(&pid, &agent.id, VoteType::Reject, None);
    }

    // Tally shows quorum not reached
    let outcome = mgr.tally_votes(&pid).unwrap_or_else(|_| unreachable!());
    assert!(!outcome.quorum_reached);
    assert_eq!(outcome.votes_for, 5);
    assert_eq!(outcome.votes_against, 10);
    assert_eq!(outcome.execution_status, ExecutionStatus::Aborted);

    // Attempting to advance to Commit should fail
    let advance_result = mgr.advance_phase(&pid);
    assert!(advance_result.is_err());
}

// =========================================================================
// Category 11: Module-Level Functions
// =========================================================================

#[test]
fn is_quorum_reached_boundary_cases() {
    // Exact quorum
    assert!(m6_consensus::is_quorum_reached(27, 40));
    // Above quorum
    assert!(m6_consensus::is_quorum_reached(30, 40));
    // Below quorum by one
    assert!(!m6_consensus::is_quorum_reached(26, 40));
    // Enough votes_for but total too low
    assert!(!m6_consensus::is_quorum_reached(27, 26));
    // Zero votes
    assert!(!m6_consensus::is_quorum_reached(0, 0));
    // Both at quorum exactly
    assert!(m6_consensus::is_quorum_reached(27, 27));
}

#[test]
fn calculate_weighted_votes_mixed_roles() {
    let votes = vec![
        make_approve_vote("prop-1", "v-01", AgentRole::Validator),   // weight 1.0
        make_reject_vote("prop-1", "c-01", AgentRole::Critic),       // weight 1.2
        ConsensusVote {
            proposal_id: "prop-1".into(),
            agent_id: "e-01".into(),
            vote: VoteType::Abstain,
            phase: ConsensusPhase::Prepare,
            role: AgentRole::Explorer,
            weight: AgentRole::Explorer.vote_weight(),
            reason: None,
            timestamp: std::time::SystemTime::now(),
        },
    ];

    let (for_w, against_w, abstain_w) = m6_consensus::calculate_weighted_votes(&votes);
    assert!((for_w - 1.0).abs() < f64::EPSILON);
    assert!((against_w - 1.2).abs() < f64::EPSILON);
    assert!((abstain_w - 0.8).abs() < f64::EPSILON);
}

#[test]
fn enhanced_consensus_check_requires_both_roles() {
    // No votes -> false
    let empty: Vec<ConsensusVote> = Vec::new();
    assert!(!m6_consensus::enhanced_consensus_check(&empty));

    // Only validators -> false
    let validators: Vec<ConsensusVote> = (0..30)
        .map(|i| make_approve_vote("prop-1", &format!("v-{i:03}"), AgentRole::Validator))
        .collect();
    assert!(!m6_consensus::enhanced_consensus_check(&validators));

    // Critic + Integrator present -> true
    let both = vec![
        make_approve_vote("prop-1", "c-01", AgentRole::Critic),
        make_approve_vote("prop-1", "i-01", AgentRole::Integrator),
    ];
    assert!(m6_consensus::enhanced_consensus_check(&both));
}

#[test]
fn quorum_calculator_participation_rate_correct() {
    // Full participation: 40/40 = 1.0
    let rate_full = QuorumCalculator::participation_rate(PBFT_N);
    assert!((rate_full - 1.0).abs() < f64::EPSILON);

    // Half participation: 20/40 = 0.5
    let rate_half = QuorumCalculator::participation_rate(20);
    assert!((rate_half - 0.5).abs() < f64::EPSILON);

    // Zero participation
    let rate_zero = QuorumCalculator::participation_rate(0);
    assert!((rate_zero - 0.0).abs() < f64::EPSILON);

    // Quorum level: 27/40 = 0.675
    let rate_quorum = QuorumCalculator::participation_rate(PBFT_Q);
    assert!((rate_quorum - 0.675).abs() < f64::EPSILON);
}

#[test]
fn quorum_calculator_minimum_votes_needed() {
    assert_eq!(QuorumCalculator::minimum_votes_needed(), PBFT_Q);
    assert_eq!(QuorumCalculator::minimum_votes_needed(), 27);
}

#[test]
fn quorum_calculator_config_affects_evaluation() {
    let calc = QuorumCalculator::new();
    let votes: Vec<ConsensusVote> = (0..27)
        .map(|i| make_approve_vote("prop-1", &format!("v-{i:03}"), AgentRole::Validator))
        .collect();

    // Default config requires critic + integrator -> should fail
    let r1 = calc
        .evaluate("prop-1", ConsensusAction::CredentialRotation, &votes)
        .unwrap_or_else(|_| unreachable!());
    assert!(!r1.met);

    // Relax config: no critic/integrator required
    let relaxed_config = QuorumConfig {
        min_participation_rate: 0.5,
        require_critic: false,
        require_integrator: false,
        weighted_threshold: 27.0,
    };
    let set_result = calc.set_config(relaxed_config);
    assert!(set_result.is_ok());

    // Same votes should now pass
    let r2 = calc
        .evaluate("prop-2", ConsensusAction::CredentialRotation, &votes)
        .unwrap_or_else(|_| unreachable!());
    assert!(r2.met);
    assert_eq!(r2.requirement, QuorumRequirement::Simple);
}
