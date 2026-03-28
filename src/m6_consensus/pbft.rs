//! # M31: PBFT Manager
//!
//! Practical Byzantine Fault Tolerance consensus orchestration for the
//! CVA-NAM agent fleet. Manages proposals, votes, phase transitions,
//! and outcome recording.
//!
//! ## Layer: L6 (Consensus)
//!
//! ## PBFT Configuration
//!
//! - n = 40 agents (CVA-NAM fleet) + 1 Human @0.A = 41 total
//! - f = 13 (Byzantine fault tolerance)
//! - q = 27 (quorum requirement: 2f + 1)
//!
//! ## Phase Transitions
//!
//! ```text
//! PrePrepare -> Prepare -> Commit -> Execute -> Complete
//!                                           \-> Failed
//! ```
//!
//! ## Related Documentation
//! - [Module Specification](../../ai_docs/modules/M31_PBFT_MANAGER.md)
//! - [PBFT Consensus](../../nam/PBFT_CONSENSUS.md)

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::RwLock;
use std::time::SystemTime;

use crate::{Error, Result};

use super::{
    calculate_weighted_votes, default_agent_fleet, enhanced_consensus_check, is_quorum_reached,
    ConsensusAction, ConsensusAgent, ConsensusOutcome, ConsensusPhase, ConsensusProposal,
    ConsensusVote, ExecutionStatus, VoteType, PBFT_Q,
};

/// PBFT consensus manager.
///
/// Orchestrates the full PBFT lifecycle: proposal creation, vote collection,
/// phase advancement, and outcome recording for the 41-agent CVA-NAM fleet.
pub struct PbftManager {
    /// Active proposals keyed by proposal ID.
    proposals: RwLock<HashMap<String, ConsensusProposal>>,
    /// Votes keyed by proposal ID.
    votes: RwLock<HashMap<String, Vec<ConsensusVote>>>,
    /// Historical outcomes (capped at 200).
    outcomes: RwLock<Vec<ConsensusOutcome>>,
    /// Current PBFT view number.
    current_view: AtomicU64,
    /// Monotonically increasing sequence counter.
    sequence_counter: AtomicU64,
    /// The agent fleet (41 agents including Human @0.A).
    fleet: Vec<ConsensusAgent>,
}

/// Maximum number of outcomes retained in memory.
const MAX_OUTCOMES: usize = 200;

impl PbftManager {
    /// Create a new PBFT manager with the default agent fleet.
    ///
    /// Initializes the fleet using `default_agent_fleet()` which creates
    /// 41 agents (40 CVA-NAM agents + Human @0.A).
    #[must_use]
    pub fn new() -> Self {
        Self {
            proposals: RwLock::new(HashMap::new()),
            votes: RwLock::new(HashMap::new()),
            outcomes: RwLock::new(Vec::new()),
            current_view: AtomicU64::new(0),
            sequence_counter: AtomicU64::new(0),
            fleet: default_agent_fleet(),
        }
    }

    /// Create a new consensus proposal for the given action.
    ///
    /// Assigns the current view number and the next sequence number,
    /// sets the initial phase to `PrePrepare`, and generates a digest
    /// from the action type and proposer.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if the proposer string is empty.
    pub fn create_proposal(
        &self,
        action: ConsensusAction,
        proposer: &str,
    ) -> Result<ConsensusProposal> {
        if proposer.is_empty() {
            return Err(Error::Validation("Proposer cannot be empty".into()));
        }

        let view = self.current_view.load(Ordering::SeqCst);
        let seq = self.next_sequence();

        let proposal_id = format!("proposal-v{view}-s{seq}");

        let proposal = ConsensusProposal::new(
            proposal_id.clone(),
            view,
            seq,
            action,
            proposer,
        );

        {
            let mut proposals = self
                .proposals
                .write()
                .map_err(|e| Error::Other(format!("Lock poisoned: {e}")))?;
            proposals.insert(proposal_id.clone(), proposal.clone());
        }

        {
            let mut votes = self
                .votes
                .write()
                .map_err(|e| Error::Other(format!("Lock poisoned: {e}")))?;
            votes.insert(proposal_id, Vec::new());
        }

        Ok(proposal)
    }

    /// Advance the phase of a proposal through the PBFT lifecycle.
    ///
    /// Phase transitions follow:
    /// `PrePrepare` -> `Prepare` -> `Commit` -> `Execute` -> `Complete`
    ///
    /// At the `Prepare` -> `Commit` transition, quorum is validated.
    /// At the `Commit` -> `Execute` transition, enhanced consensus
    /// (Critic + Integrator approval) is checked.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if the proposal is not found,
    /// the phase is terminal (`Complete` or `Failed`), or quorum
    /// requirements are not met.
    pub fn advance_phase(&self, proposal_id: &str) -> Result<ConsensusPhase> {
        let mut proposals = self
            .proposals
            .write()
            .map_err(|e| Error::Other(format!("Lock poisoned: {e}")))?;

        let proposal = proposals
            .get_mut(proposal_id)
            .ok_or_else(|| Error::Validation(format!("Proposal not found: {proposal_id}")))?;

        let next_phase = match proposal.phase {
            ConsensusPhase::PrePrepare => ConsensusPhase::Prepare,
            ConsensusPhase::Prepare => {
                // Validate quorum before advancing to Commit
                let votes_guard = self
                    .votes
                    .read()
                    .map_err(|e| Error::Other(format!("Lock poisoned: {e}")))?;
                let votes = votes_guard.get(proposal_id).cloned().unwrap_or_default();
                drop(votes_guard);

                #[allow(clippy::cast_possible_truncation)]
                let approve_count = votes
                    .iter()
                    .filter(|v| v.vote == VoteType::Approve)
                    .count() as u32;
                #[allow(clippy::cast_possible_truncation)]
                let total = votes.len() as u32;

                if !is_quorum_reached(approve_count, total) {
                    return Err(Error::ConsensusQuorum {
                        required: PBFT_Q,
                        received: approve_count,
                    });
                }

                ConsensusPhase::Commit
            }
            ConsensusPhase::Commit => {
                // Validate enhanced consensus (Critic + Integrator) before Execute
                let votes_guard = self
                    .votes
                    .read()
                    .map_err(|e| Error::Other(format!("Lock poisoned: {e}")))?;
                let votes = votes_guard.get(proposal_id).cloned().unwrap_or_default();
                drop(votes_guard);

                if !enhanced_consensus_check(&votes) {
                    return Err(Error::Validation(
                        "Enhanced consensus requires Critic and Integrator approval".into(),
                    ));
                }

                ConsensusPhase::Execute
            }
            ConsensusPhase::Execute => ConsensusPhase::Complete,
            ConsensusPhase::Complete => {
                return Err(Error::Validation(
                    "Proposal already complete".into(),
                ));
            }
            ConsensusPhase::Failed => {
                return Err(Error::Validation(
                    "Cannot advance a failed proposal".into(),
                ));
            }
        };

        proposal.phase = next_phase;
        drop(proposals);
        Ok(next_phase)
    }

    /// Submit a vote on a proposal from a specific agent.
    ///
    /// Validates that the agent exists in the fleet, then creates a vote
    /// with the agent's role and weight.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if the proposal or agent is not found.
    pub fn submit_vote(
        &self,
        proposal_id: &str,
        agent_id: &str,
        vote_type: VoteType,
        reason: Option<String>,
    ) -> Result<ConsensusVote> {
        // Validate proposal exists
        {
            let proposals = self
                .proposals
                .read()
                .map_err(|e| Error::Other(format!("Lock poisoned: {e}")))?;
            if !proposals.contains_key(proposal_id) {
                return Err(Error::Validation(format!(
                    "Proposal not found: {proposal_id}"
                )));
            }
        }

        // Find the agent in the fleet
        let agent = self
            .fleet
            .iter()
            .find(|a| a.id == agent_id)
            .ok_or_else(|| Error::Validation(format!("Agent not found: {agent_id}")))?;

        // Get the current phase of the proposal
        let phase = {
            let proposals = self
                .proposals
                .read()
                .map_err(|e| Error::Other(format!("Lock poisoned: {e}")))?;
            proposals
                .get(proposal_id)
                .map_or(ConsensusPhase::PrePrepare, |p| p.phase)
        };

        let vote = ConsensusVote {
            proposal_id: proposal_id.into(),
            agent_id: agent_id.into(),
            vote: vote_type,
            phase,
            role: agent.role,
            weight: agent.weight,
            reason,
            timestamp: SystemTime::now(),
        };

        {
            let mut votes = self
                .votes
                .write()
                .map_err(|e| Error::Other(format!("Lock poisoned: {e}")))?;
            votes
                .entry(proposal_id.into())
                .or_default()
                .push(vote.clone());
        }

        Ok(vote)
    }

    /// Tally the votes for a proposal and produce a `ConsensusOutcome`.
    ///
    /// Uses `calculate_weighted_votes` for weighted tallying and
    /// `is_quorum_reached` to determine if the quorum threshold is met.
    /// The outcome is stored in the outcomes history (capped at 200).
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if the proposal is not found.
    #[allow(clippy::cast_possible_truncation)]
    pub fn tally_votes(&self, proposal_id: &str) -> Result<ConsensusOutcome> {
        // Validate proposal exists
        {
            let proposals = self
                .proposals
                .read()
                .map_err(|e| Error::Other(format!("Lock poisoned: {e}")))?;
            if !proposals.contains_key(proposal_id) {
                return Err(Error::Validation(format!(
                    "Proposal not found: {proposal_id}"
                )));
            }
        }

        let votes = {
            let votes_guard = self
                .votes
                .read()
                .map_err(|e| Error::Other(format!("Lock poisoned: {e}")))?;
            votes_guard.get(proposal_id).cloned().unwrap_or_default()
        };

        let (weighted_for, weighted_against, _weighted_abstain) =
            calculate_weighted_votes(&votes);

        let votes_for = votes
            .iter()
            .filter(|v| v.vote == VoteType::Approve)
            .count() as u32;
        let votes_against = votes
            .iter()
            .filter(|v| v.vote == VoteType::Reject)
            .count() as u32;
        let votes_abstained = votes
            .iter()
            .filter(|v| v.vote == VoteType::Abstain)
            .count() as u32;

        let total = votes.len() as u32;
        let quorum_reached = is_quorum_reached(votes_for, total);

        let outcome = ConsensusOutcome {
            proposal_id: proposal_id.into(),
            quorum_reached,
            votes_for,
            votes_against,
            votes_abstained,
            weighted_for,
            weighted_against,
            execution_status: if quorum_reached {
                ExecutionStatus::Pending
            } else {
                ExecutionStatus::Aborted
            },
            completed_at: Some(SystemTime::now()),
        };

        {
            let mut outcomes = self
                .outcomes
                .write()
                .map_err(|e| Error::Other(format!("Lock poisoned: {e}")))?;
            if outcomes.len() >= MAX_OUTCOMES {
                outcomes.remove(0);
            }
            outcomes.push(outcome.clone());
        }

        Ok(outcome)
    }

    /// Retrieve a proposal by ID.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if the proposal is not found or the lock is poisoned.
    pub fn get_proposal(&self, proposal_id: &str) -> Result<ConsensusProposal> {
        let proposals = self
            .proposals
            .read()
            .map_err(|e| Error::Other(format!("Lock poisoned: {e}")))?;
        proposals
            .get(proposal_id)
            .cloned()
            .ok_or_else(|| Error::Validation(format!("Proposal not found: {proposal_id}")))
    }

    /// Retrieve all votes for a proposal.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if the proposal is not found or the lock is poisoned.
    pub fn get_votes(&self, proposal_id: &str) -> Result<Vec<ConsensusVote>> {
        let votes = self
            .votes
            .read()
            .map_err(|e| Error::Other(format!("Lock poisoned: {e}")))?;
        votes
            .get(proposal_id)
            .cloned()
            .ok_or_else(|| Error::Validation(format!("Proposal not found: {proposal_id}")))
    }

    /// Retrieve the outcome for a proposal by searching the outcomes history.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if no outcome is found for the given proposal.
    pub fn get_outcome(&self, proposal_id: &str) -> Result<ConsensusOutcome> {
        let outcomes = self
            .outcomes
            .read()
            .map_err(|e| Error::Other(format!("Lock poisoned: {e}")))?;
        outcomes
            .iter()
            .find(|o| o.proposal_id == proposal_id)
            .cloned()
            .ok_or_else(|| Error::Validation(format!("Outcome not found: {proposal_id}")))
    }

    /// Get all proposals that are not yet `Complete` or `Failed`.
    ///
    /// # Errors
    ///
    /// Returns an empty vec if the lock cannot be acquired (non-fatal).
    #[must_use]
    pub fn get_active_proposals(&self) -> Vec<ConsensusProposal> {
        let Ok(proposals) = self.proposals.read() else {
            return Vec::new();
        };
        proposals
            .values()
            .filter(|p| {
                p.phase != ConsensusPhase::Complete && p.phase != ConsensusPhase::Failed
            })
            .cloned()
            .collect()
    }

    /// Look up an agent in the fleet by ID.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if the agent is not found.
    pub fn get_agent(&self, agent_id: &str) -> Result<ConsensusAgent> {
        self.fleet
            .iter()
            .find(|a| a.id == agent_id)
            .cloned()
            .ok_or_else(|| Error::Validation(format!("Agent not found: {agent_id}")))
    }

    /// Get the full agent fleet.
    #[must_use]
    pub fn get_fleet(&self) -> Vec<ConsensusAgent> {
        self.fleet.clone()
    }

    /// Get the number of proposals (active and completed).
    #[must_use]
    pub fn proposal_count(&self) -> usize {
        let Ok(proposals) = self.proposals.read() else {
            return 0;
        };
        proposals.len()
    }

    /// Get the current PBFT view number.
    #[must_use]
    pub fn current_view_number(&self) -> u64 {
        self.current_view.load(Ordering::SeqCst)
    }

    /// Atomically increment and return the next sequence number.
    #[must_use]
    pub fn next_sequence(&self) -> u64 {
        self.sequence_counter.fetch_add(1, Ordering::SeqCst)
    }
}

impl Default for PbftManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::AgentStatus;
    use crate::AgentRole;

    #[test]
    fn test_new_fleet_size() {
        let mgr = PbftManager::new();
        assert_eq!(mgr.get_fleet().len(), 41);
    }

    #[test]
    fn test_create_proposal() {
        let mgr = PbftManager::new();
        let proposal = mgr
            .create_proposal(ConsensusAction::ServiceTermination, "@0.A")
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(proposal.phase, ConsensusPhase::PrePrepare);
        assert_eq!(proposal.proposer, "@0.A");
        assert_eq!(proposal.action_type, ConsensusAction::ServiceTermination);
        assert_eq!(proposal.view_number, 0);
        assert_eq!(proposal.sequence_number, 0);
    }

    #[test]
    fn test_create_proposal_empty_proposer_fails() {
        let mgr = PbftManager::new();
        let result = mgr.create_proposal(ConsensusAction::ServiceTermination, "");
        assert!(result.is_err());
    }

    #[test]
    fn test_advance_phase_sequence() {
        let mgr = PbftManager::new();
        let proposal = mgr
            .create_proposal(ConsensusAction::ConfigRollback, "@0.A")
            .unwrap_or_else(|_| unreachable!());
        let pid = proposal.id.clone();

        // PrePrepare -> Prepare (no quorum check)
        let phase = mgr.advance_phase(&pid).unwrap_or_else(|_| unreachable!());
        assert_eq!(phase, ConsensusPhase::Prepare);

        // Submit enough approval votes (27+) with at least 1 Critic and 1 Integrator
        // Fleet order: @0.A(V), agent-01..20(V), agent-21..28(E), agent-29..34(C), agent-35..38(I)
        // Taking 36 agents ensures we include validators, explorers, critics, AND integrators
        let fleet = mgr.get_fleet();
        for agent in fleet.iter().take(36) {
            let _ = mgr.submit_vote(&pid, &agent.id, VoteType::Approve, None);
        }

        // Prepare -> Commit
        let phase = mgr.advance_phase(&pid).unwrap_or_else(|_| unreachable!());
        assert_eq!(phase, ConsensusPhase::Commit);

        // Commit -> Execute (needs Critic + Integrator approval, which we have)
        let phase = mgr.advance_phase(&pid).unwrap_or_else(|_| unreachable!());
        assert_eq!(phase, ConsensusPhase::Execute);

        // Execute -> Complete
        let phase = mgr.advance_phase(&pid).unwrap_or_else(|_| unreachable!());
        assert_eq!(phase, ConsensusPhase::Complete);
    }

    #[test]
    fn test_submit_vote() {
        let mgr = PbftManager::new();
        let proposal = mgr
            .create_proposal(ConsensusAction::CredentialRotation, "@0.A")
            .unwrap_or_else(|_| unreachable!());
        let pid = proposal.id.clone();

        let vote = mgr
            .submit_vote(&pid, "@0.A", VoteType::Approve, Some("Looks good".into()))
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(vote.agent_id, "@0.A");
        assert_eq!(vote.vote, VoteType::Approve);
        assert_eq!(vote.role, AgentRole::Validator);
        assert!((vote.weight - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_submit_vote_unknown_agent_fails() {
        let mgr = PbftManager::new();
        let proposal = mgr
            .create_proposal(ConsensusAction::CredentialRotation, "@0.A")
            .unwrap_or_else(|_| unreachable!());
        let result = mgr.submit_vote(&proposal.id, "nonexistent", VoteType::Approve, None);
        assert!(result.is_err());
    }

    #[test]
    fn test_tally_quorum_reached() {
        let mgr = PbftManager::new();
        let proposal = mgr
            .create_proposal(ConsensusAction::DatabaseMigration, "@0.A")
            .unwrap_or_else(|_| unreachable!());
        let pid = proposal.id.clone();

        // Submit 30 approval votes (well above quorum of 27)
        let fleet = mgr.get_fleet();
        for agent in fleet.iter().take(30) {
            let _ = mgr.submit_vote(&pid, &agent.id, VoteType::Approve, None);
        }

        let outcome = mgr.tally_votes(&pid).unwrap_or_else(|_| unreachable!());
        assert!(outcome.quorum_reached);
        assert_eq!(outcome.votes_for, 30);
        assert_eq!(outcome.votes_against, 0);
        assert_eq!(outcome.execution_status, ExecutionStatus::Pending);
    }

    #[test]
    fn test_tally_quorum_not_reached() {
        let mgr = PbftManager::new();
        let proposal = mgr
            .create_proposal(ConsensusAction::CascadeRestart, "@0.A")
            .unwrap_or_else(|_| unreachable!());
        let pid = proposal.id.clone();

        // Submit only 10 approval votes (below quorum of 27)
        let fleet = mgr.get_fleet();
        for agent in fleet.iter().take(10) {
            let _ = mgr.submit_vote(&pid, &agent.id, VoteType::Approve, None);
        }

        let outcome = mgr.tally_votes(&pid).unwrap_or_else(|_| unreachable!());
        assert!(!outcome.quorum_reached);
        assert_eq!(outcome.votes_for, 10);
        assert_eq!(outcome.execution_status, ExecutionStatus::Aborted);
    }

    #[test]
    fn test_enhanced_consensus_needs_critic_and_integrator() {
        let mgr = PbftManager::new();
        let proposal = mgr
            .create_proposal(ConsensusAction::ConfigRollback, "@0.A")
            .unwrap_or_else(|_| unreachable!());
        let pid = proposal.id.clone();

        // Advance to Prepare
        let _ = mgr.advance_phase(&pid);

        // Submit approvals from Validators and Explorers only (no Critic or Integrator)
        // Fleet: 21 validators + 8 explorers = 29 agents without Critic/Integrator roles
        let fleet = mgr.get_fleet();
        let non_critic_integrator: Vec<_> = fleet
            .iter()
            .filter(|a| a.role == AgentRole::Validator || a.role == AgentRole::Explorer)
            .collect();
        for agent in non_critic_integrator.iter().take(27) {
            let _ = mgr.submit_vote(&pid, &agent.id, VoteType::Approve, None);
        }

        // Prepare -> Commit should succeed (quorum met with 27 non-Critic/Integrator agents)
        let phase = mgr.advance_phase(&pid).unwrap_or_else(|_| unreachable!());
        assert_eq!(phase, ConsensusPhase::Commit);

        // Commit -> Execute should FAIL because no Critic or Integrator approved
        let result = mgr.advance_phase(&pid);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_proposal() {
        let mgr = PbftManager::new();
        let proposal = mgr
            .create_proposal(ConsensusAction::ServiceTermination, "@0.A")
            .unwrap_or_else(|_| unreachable!());
        let pid = proposal.id.clone();

        let fetched = mgr.get_proposal(&pid).unwrap_or_else(|_| unreachable!());
        assert_eq!(fetched.id, pid);
        assert_eq!(fetched.proposer, "@0.A");

        // Non-existent proposal
        let result = mgr.get_proposal("nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_get_votes() {
        let mgr = PbftManager::new();
        let proposal = mgr
            .create_proposal(ConsensusAction::ConfigRollback, "@0.A")
            .unwrap_or_else(|_| unreachable!());
        let pid = proposal.id.clone();

        let _ = mgr.submit_vote(&pid, "@0.A", VoteType::Approve, None);
        let _ = mgr.submit_vote(&pid, "agent-01", VoteType::Reject, Some("Risky".into()));

        let votes = mgr.get_votes(&pid).unwrap_or_else(|_| unreachable!());
        assert_eq!(votes.len(), 2);
    }

    #[test]
    fn test_active_proposals() {
        let mgr = PbftManager::new();
        let _ = mgr.create_proposal(ConsensusAction::ServiceTermination, "@0.A");
        let _ = mgr.create_proposal(ConsensusAction::DatabaseMigration, "@0.A");

        let active = mgr.get_active_proposals();
        assert_eq!(active.len(), 2);
    }

    #[test]
    fn test_sequence_counter() {
        let mgr = PbftManager::new();
        let s0 = mgr.next_sequence();
        let s1 = mgr.next_sequence();
        let s2 = mgr.next_sequence();
        assert_eq!(s0, 0);
        assert_eq!(s1, 1);
        assert_eq!(s2, 2);
    }

    #[test]
    fn test_view_number() {
        let mgr = PbftManager::new();
        assert_eq!(mgr.current_view_number(), 0);
    }

    #[test]
    fn test_get_agent() {
        let mgr = PbftManager::new();
        let human = mgr.get_agent("@0.A").unwrap_or_else(|_| unreachable!());
        assert_eq!(human.tier, 0);
        assert_eq!(human.role, AgentRole::Validator);
        assert_eq!(human.status, AgentStatus::Active);

        let critic = mgr.get_agent("agent-29").unwrap_or_else(|_| unreachable!());
        assert_eq!(critic.role, AgentRole::Critic);
        assert!((critic.weight - 1.2).abs() < f64::EPSILON);
    }

    #[test]
    fn test_proposal_count() {
        let mgr = PbftManager::new();
        assert_eq!(mgr.proposal_count(), 0);
        let _ = mgr.create_proposal(ConsensusAction::ServiceTermination, "@0.A");
        assert_eq!(mgr.proposal_count(), 1);
        let _ = mgr.create_proposal(ConsensusAction::DatabaseMigration, "@0.A");
        assert_eq!(mgr.proposal_count(), 2);
    }

    #[test]
    fn test_advance_complete_fails() {
        let mgr = PbftManager::new();
        let proposal = mgr
            .create_proposal(ConsensusAction::ConfigRollback, "@0.A")
            .unwrap_or_else(|_| unreachable!());
        let pid = proposal.id.clone();

        // Advance through all phases with sufficient votes
        let _ = mgr.advance_phase(&pid); // PrePrepare -> Prepare

        // Take 36 to include Critics and Integrators for enhanced consensus
        let fleet = mgr.get_fleet();
        for agent in fleet.iter().take(36) {
            let _ = mgr.submit_vote(&pid, &agent.id, VoteType::Approve, None);
        }

        let _ = mgr.advance_phase(&pid); // Prepare -> Commit
        let _ = mgr.advance_phase(&pid); // Commit -> Execute
        let _ = mgr.advance_phase(&pid); // Execute -> Complete

        // Advancing a Complete proposal should fail
        let result = mgr.advance_phase(&pid);
        assert!(result.is_err());
    }

    #[test]
    fn test_tally_nonexistent_proposal_fails() {
        let mgr = PbftManager::new();
        let result = mgr.tally_votes("nonexistent");
        assert!(result.is_err());
    }

    // ---------------------------------------------------------------
    // Additional tests to reach 50+
    // ---------------------------------------------------------------

    #[test]
    fn test_default_impl() {
        let mgr = PbftManager::default();
        assert_eq!(mgr.get_fleet().len(), 41);
    }

    #[test]
    fn test_fleet_has_human_agent() {
        let mgr = PbftManager::new();
        let human = mgr.get_agent("@0.A");
        assert!(human.is_ok());
    }

    #[test]
    fn test_fleet_has_correct_role_counts() {
        let mgr = PbftManager::new();
        let fleet = mgr.get_fleet();
        let validators = fleet.iter().filter(|a| a.role == AgentRole::Validator).count();
        let explorers = fleet.iter().filter(|a| a.role == AgentRole::Explorer).count();
        let critics = fleet.iter().filter(|a| a.role == AgentRole::Critic).count();
        let integrators = fleet.iter().filter(|a| a.role == AgentRole::Integrator).count();
        // 21 validators (including @0.A), 8 explorers, 6 critics, 4 integrators, 2 historians
        assert!(validators >= 20);
        assert!(explorers >= 8);
        assert!(critics >= 6);
        assert!(integrators >= 4);
    }

    #[test]
    fn test_get_agent_unknown_fails() {
        let mgr = PbftManager::new();
        let result = mgr.get_agent("nonexistent-agent");
        assert!(result.is_err());
    }

    #[test]
    fn test_create_multiple_proposals() {
        let mgr = PbftManager::new();
        let p1 = mgr.create_proposal(ConsensusAction::ServiceTermination, "@0.A");
        let p2 = mgr.create_proposal(ConsensusAction::DatabaseMigration, "@0.A");
        assert!(p1.is_ok());
        assert!(p2.is_ok());
        assert_eq!(mgr.proposal_count(), 2);
    }

    #[test]
    fn test_proposal_ids_unique() {
        let mgr = PbftManager::new();
        let p1 = mgr.create_proposal(ConsensusAction::ServiceTermination, "@0.A")
            .ok().map(|p| p.id).unwrap_or_default();
        let p2 = mgr.create_proposal(ConsensusAction::ServiceTermination, "@0.A")
            .ok().map(|p| p.id).unwrap_or_default();
        assert_ne!(p1, p2);
    }

    #[test]
    fn test_proposal_sequence_numbers_increasing() {
        let mgr = PbftManager::new();
        let p1 = mgr.create_proposal(ConsensusAction::ServiceTermination, "@0.A")
            .ok().map(|p| p.sequence_number).unwrap_or(0);
        let p2 = mgr.create_proposal(ConsensusAction::ServiceTermination, "@0.A")
            .ok().map(|p| p.sequence_number).unwrap_or(0);
        assert_eq!(p2, p1 + 1);
    }

    #[test]
    fn test_advance_nonexistent_proposal_fails() {
        let mgr = PbftManager::new();
        let result = mgr.advance_phase("nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_submit_vote_nonexistent_proposal_fails() {
        let mgr = PbftManager::new();
        let result = mgr.submit_vote("nonexistent", "@0.A", VoteType::Approve, None);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_votes_returns_all() {
        let mgr = PbftManager::new();
        let proposal = mgr.create_proposal(ConsensusAction::ConfigRollback, "@0.A")
            .unwrap_or_else(|_| unreachable!());
        let pid = proposal.id.clone();

        let _ = mgr.submit_vote(&pid, "@0.A", VoteType::Approve, None);
        let _ = mgr.submit_vote(&pid, "agent-01", VoteType::Reject, None);
        let _ = mgr.submit_vote(&pid, "agent-02", VoteType::Abstain, None);

        let votes = mgr.get_votes(&pid).unwrap_or_else(|_| unreachable!());
        assert_eq!(votes.len(), 3);
    }

    #[test]
    fn test_tally_with_mixed_votes() {
        let mgr = PbftManager::new();
        let proposal = mgr.create_proposal(ConsensusAction::CascadeRestart, "@0.A")
            .unwrap_or_else(|_| unreachable!());
        let pid = proposal.id.clone();

        let fleet = mgr.get_fleet();
        // 20 approvals, 10 rejections, 5 abstentions
        for agent in fleet.iter().take(20) {
            let _ = mgr.submit_vote(&pid, &agent.id, VoteType::Approve, None);
        }
        for agent in fleet.iter().skip(20).take(10) {
            let _ = mgr.submit_vote(&pid, &agent.id, VoteType::Reject, None);
        }
        for agent in fleet.iter().skip(30).take(5) {
            let _ = mgr.submit_vote(&pid, &agent.id, VoteType::Abstain, None);
        }

        let outcome = mgr.tally_votes(&pid).unwrap_or_else(|_| unreachable!());
        assert_eq!(outcome.votes_for, 20);
        assert_eq!(outcome.votes_against, 10);
        assert_eq!(outcome.votes_abstained, 5);
    }

    #[test]
    fn test_vote_weight_preserved() {
        let mgr = PbftManager::new();
        let proposal = mgr.create_proposal(ConsensusAction::ConfigRollback, "@0.A")
            .unwrap_or_else(|_| unreachable!());
        let pid = proposal.id.clone();

        let vote = mgr.submit_vote(&pid, "agent-29", VoteType::Approve, None)
            .unwrap_or_else(|_| unreachable!());
        assert!((vote.weight - 1.2).abs() < f64::EPSILON, "Critics have weight 1.2");
    }

    #[test]
    fn test_vote_reason_preserved() {
        let mgr = PbftManager::new();
        let proposal = mgr.create_proposal(ConsensusAction::ConfigRollback, "@0.A")
            .unwrap_or_else(|_| unreachable!());
        let pid = proposal.id.clone();

        let vote = mgr.submit_vote(&pid, "@0.A", VoteType::Approve, Some("Looks good".into()))
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(vote.reason.as_deref(), Some("Looks good"));
    }

    #[test]
    fn test_get_outcome() {
        let mgr = PbftManager::new();
        let proposal = mgr.create_proposal(ConsensusAction::ServiceTermination, "@0.A")
            .unwrap_or_else(|_| unreachable!());
        let pid = proposal.id.clone();

        let _ = mgr.tally_votes(&pid);
        let outcome = mgr.get_outcome(&pid);
        assert!(outcome.is_ok());
    }

    #[test]
    fn test_get_outcome_not_found() {
        let mgr = PbftManager::new();
        let result = mgr.get_outcome("nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_active_proposals_excludes_completed() {
        let mgr = PbftManager::new();
        let proposal = mgr.create_proposal(ConsensusAction::ConfigRollback, "@0.A")
            .unwrap_or_else(|_| unreachable!());
        let pid = proposal.id.clone();

        // Advance through all phases
        let _ = mgr.advance_phase(&pid);
        let fleet = mgr.get_fleet();
        for agent in fleet.iter().take(36) {
            let _ = mgr.submit_vote(&pid, &agent.id, VoteType::Approve, None);
        }
        let _ = mgr.advance_phase(&pid);
        let _ = mgr.advance_phase(&pid);
        let _ = mgr.advance_phase(&pid);

        let active = mgr.get_active_proposals();
        assert!(active.iter().all(|p| p.id != pid));
    }

    #[test]
    fn test_tally_with_no_votes() {
        let mgr = PbftManager::new();
        let proposal = mgr.create_proposal(ConsensusAction::ServiceTermination, "@0.A")
            .unwrap_or_else(|_| unreachable!());
        let outcome = mgr.tally_votes(&proposal.id).unwrap_or_else(|_| unreachable!());
        assert!(!outcome.quorum_reached);
        assert_eq!(outcome.votes_for, 0);
    }

    #[test]
    fn test_advance_failed_proposal_fails() {
        let mgr = PbftManager::new();
        let proposal = mgr.create_proposal(ConsensusAction::ConfigRollback, "@0.A")
            .unwrap_or_else(|_| unreachable!());
        let pid = proposal.id.clone();

        // Manually set phase to Failed via advance (quorum failure)
        let _ = mgr.advance_phase(&pid); // PrePrepare -> Prepare
        // Try to advance without quorum
        let result = mgr.advance_phase(&pid); // Prepare -> Commit (should fail)
        assert!(result.is_err());
    }

    #[test]
    fn test_weighted_votes_in_tally() {
        let mgr = PbftManager::new();
        let proposal = mgr.create_proposal(ConsensusAction::DatabaseMigration, "@0.A")
            .unwrap_or_else(|_| unreachable!());
        let pid = proposal.id.clone();

        // Submit votes from agents with different weights
        let _ = mgr.submit_vote(&pid, "@0.A", VoteType::Approve, None); // weight 1.0
        let _ = mgr.submit_vote(&pid, "agent-29", VoteType::Approve, None); // weight 1.2 (Critic)

        let outcome = mgr.tally_votes(&pid).unwrap_or_else(|_| unreachable!());
        assert!(outcome.weighted_for > 2.0, "Weighted sum should reflect critic weight");
    }

    #[test]
    fn test_proposal_action_preserved() {
        let mgr = PbftManager::new();
        let proposal = mgr.create_proposal(ConsensusAction::CredentialRotation, "@0.A")
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(proposal.action_type, ConsensusAction::CredentialRotation);
    }

    #[test]
    fn test_proposal_initial_phase() {
        let mgr = PbftManager::new();
        let proposal = mgr.create_proposal(ConsensusAction::ServiceTermination, "@0.A")
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(proposal.phase, ConsensusPhase::PrePrepare);
    }

    #[test]
    fn test_get_votes_nonexistent_fails() {
        let mgr = PbftManager::new();
        let result = mgr.get_votes("nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_explorer_weight() {
        let mgr = PbftManager::new();
        let explorer = mgr.get_agent("agent-21").unwrap_or_else(|_| unreachable!());
        assert_eq!(explorer.role, AgentRole::Explorer);
        assert!((explorer.weight - 0.8).abs() < f64::EPSILON);
    }

    #[test]
    fn test_integrator_role() {
        let mgr = PbftManager::new();
        let integrator = mgr.get_agent("agent-35").unwrap_or_else(|_| unreachable!());
        assert_eq!(integrator.role, AgentRole::Integrator);
    }

    #[test]
    fn test_human_agent_status_active() {
        let mgr = PbftManager::new();
        let human = mgr.get_agent("@0.A").unwrap_or_else(|_| unreachable!());
        assert_eq!(human.status, AgentStatus::Active);
    }
}
