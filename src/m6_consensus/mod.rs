//! # Layer 6: Consensus
//!
//! PBFT consensus, agent coordination, and distributed decision-making.
//!
//! ## Modules
//!
//! | Module | Name | Purpose |
//! |--------|------|---------|
//! | M31 | PBFT Manager | Consensus orchestration |
//! | M32 | Agent Coordinator | Multi-agent coordination |
//! | M33 | Vote Collector | Vote aggregation |
//! | M34 | View Change Handler | Leader election |
//! | M35 | Dissent Tracker | Disagreement capture |
//! | M36 | Quorum Calculator | Quorum management |
//!
//! ## PBFT Configuration
//!
//! - n = 40 agents (CVA-NAM fleet)
//! - f = 13 (Byzantine fault tolerance)
//! - q = 27 (quorum requirement: 2f + 1)
//!
//! ## Related Documentation
//! - [Layer Specification](../../ai_docs/layers/L06_CONSENSUS.md)
//! - [PBFT Consensus](../../nam/PBFT_CONSENSUS.md)

pub mod active_dissent;
pub mod agent;
pub mod pbft;
pub mod voting;
pub mod dissent;
pub mod view_change;
pub mod quorum;

use crate::AgentRole;

/// PBFT total agents: n = 40 (CVA-NAM fleet)
pub const PBFT_N: u32 = 40;
/// PBFT Byzantine fault tolerance: f = 13, where n = 3f + 1
pub const PBFT_F: u32 = 13;
/// PBFT quorum requirement: q = 27 (2f + 1)
pub const PBFT_Q: u32 = 27;

/// Consensus phase
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConsensusPhase {
    /// Initial proposal broadcast
    PrePrepare,
    /// Collecting prepare votes
    Prepare,
    /// Collecting commit votes
    Commit,
    /// Executing agreed action
    Execute,
    /// Consensus complete
    Complete,
    /// Consensus failed
    Failed,
}

/// Consensus proposal
#[derive(Clone, Debug)]
pub struct ConsensusProposal {
    /// Unique proposal ID
    pub id: String,
    /// View number
    pub view_number: u64,
    /// Sequence number
    pub sequence_number: u64,
    /// Action type
    pub action_type: ConsensusAction,
    /// Action payload (JSON)
    pub action_payload: String,
    /// Proposer agent ID
    pub proposer: String,
    /// Current phase
    pub phase: ConsensusPhase,
    /// Timestamp
    pub timestamp: std::time::SystemTime,
    /// Timeout in milliseconds
    pub timeout_ms: u64,
}

impl ConsensusProposal {
    /// Create a new proposal
    #[must_use]
    pub fn new(
        id: impl Into<String>,
        view_number: u64,
        sequence_number: u64,
        action_type: ConsensusAction,
        proposer: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            view_number,
            sequence_number,
            action_type,
            action_payload: String::new(),
            proposer: proposer.into(),
            phase: ConsensusPhase::PrePrepare,
            timestamp: std::time::SystemTime::now(),
            timeout_ms: 5000,
        }
    }

    /// Get timeout for this action type
    #[must_use]
    pub const fn get_timeout(&self) -> u64 {
        match self.action_type {
            ConsensusAction::ServiceTermination => 60_000,
            ConsensusAction::DatabaseMigration => 300_000,
            ConsensusAction::CredentialRotation => 120_000,
            ConsensusAction::CascadeRestart => 180_000,
            ConsensusAction::ConfigRollback => 90_000,
        }
    }
}

/// Actions requiring PBFT consensus
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConsensusAction {
    /// Emergency service termination (kill -9)
    ServiceTermination,
    /// Database migration
    DatabaseMigration,
    /// Credential rotation
    CredentialRotation,
    /// Multi-service cascade restart
    CascadeRestart,
    /// Configuration rollback
    ConfigRollback,
}

impl ConsensusAction {
    /// Get the minimum quorum for this action
    #[must_use]
    pub const fn required_quorum(&self) -> u32 {
        PBFT_Q // All critical actions require standard quorum
    }

    /// Get the default timeout in seconds
    #[must_use]
    pub const fn default_timeout_seconds(&self) -> u64 {
        match self {
            Self::ServiceTermination => 60,
            Self::DatabaseMigration => 300,
            Self::CredentialRotation => 120,
            Self::CascadeRestart => 180,
            Self::ConfigRollback => 90,
        }
    }
}

/// Vote on a proposal
#[derive(Clone, Debug)]
pub struct ConsensusVote {
    /// Proposal ID
    pub proposal_id: String,
    /// Voting agent ID
    pub agent_id: String,
    /// Vote (approve/reject/abstain)
    pub vote: VoteType,
    /// Consensus phase
    pub phase: ConsensusPhase,
    /// Agent role
    pub role: AgentRole,
    /// Vote weight
    pub weight: f64,
    /// Reason (optional)
    pub reason: Option<String>,
    /// Timestamp
    pub timestamp: std::time::SystemTime,
}

/// Vote type
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VoteType {
    /// Approve the proposal
    Approve,
    /// Reject the proposal
    Reject,
    /// Abstain from voting
    Abstain,
}

/// Consensus outcome
#[derive(Clone, Debug)]
pub struct ConsensusOutcome {
    /// Proposal ID
    pub proposal_id: String,
    /// Quorum reached
    pub quorum_reached: bool,
    /// Votes for
    pub votes_for: u32,
    /// Votes against
    pub votes_against: u32,
    /// Votes abstained
    pub votes_abstained: u32,
    /// Weighted votes for
    pub weighted_for: f64,
    /// Weighted votes against
    pub weighted_against: f64,
    /// Execution status
    pub execution_status: ExecutionStatus,
    /// Completion timestamp
    pub completed_at: Option<std::time::SystemTime>,
}

/// Execution status
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExecutionStatus {
    /// Pending execution
    Pending,
    /// Executing
    Executing,
    /// Successfully executed
    Success,
    /// Execution failed
    Failed,
    /// Execution aborted
    Aborted,
    /// Rolled back
    RolledBack,
}

/// Dissent event for learning
#[derive(Clone, Debug)]
pub struct DissentEvent {
    /// Unique ID
    pub id: String,
    /// Proposed action
    pub proposed_action: String,
    /// Dissenting agent
    pub dissenting_agent: String,
    /// Dissent reason
    pub reason: String,
    /// Outcome of the proposal (after decision)
    pub outcome: Option<String>,
    /// Whether dissent was valuable (post-hoc evaluation)
    pub was_valuable: Option<bool>,
    /// Timestamp
    pub timestamp: std::time::SystemTime,
}

/// Calculate if quorum is reached
#[must_use]
pub const fn is_quorum_reached(votes_for: u32, total_votes: u32) -> bool {
    votes_for >= PBFT_Q && total_votes >= PBFT_Q
}

/// Calculate weighted vote totals
#[must_use]
pub fn calculate_weighted_votes(votes: &[ConsensusVote]) -> (f64, f64, f64) {
    let mut for_weight = 0.0;
    let mut against_weight = 0.0;
    let mut abstain_weight = 0.0;

    for vote in votes {
        match vote.vote {
            VoteType::Approve => for_weight += vote.weight,
            VoteType::Reject => against_weight += vote.weight,
            VoteType::Abstain => abstain_weight += vote.weight,
        }
    }

    (for_weight, against_weight, abstain_weight)
}

/// Check if enhanced consensus requirements are met (NAM-05)
/// Requires at least 1 CRITIC and 1 INTEGRATOR approval
#[must_use]
pub fn enhanced_consensus_check(votes: &[ConsensusVote]) -> bool {
    let critic_approval = votes
        .iter()
        .any(|v| v.role == AgentRole::Critic && v.vote == VoteType::Approve);

    let integrator_approval = votes
        .iter()
        .any(|v| v.role == AgentRole::Integrator && v.vote == VoteType::Approve);

    critic_approval && integrator_approval
}

/// Agent participating in consensus
#[derive(Clone, Debug)]
pub struct ConsensusAgent {
    /// Agent ID
    pub id: String,
    /// Agent role
    pub role: AgentRole,
    /// Vote weight
    pub weight: f64,
    /// Tier (0 = Human @0.A)
    pub tier: u8,
    /// Status
    pub status: AgentStatus,
    /// Success rate
    pub success_rate: f64,
    /// Last heartbeat
    pub last_heartbeat: Option<std::time::SystemTime>,
}

/// Agent status
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum AgentStatus {
    /// Agent is idle
    #[default]
    Idle,
    /// Agent is active
    Active,
    /// Agent is busy
    Busy,
    /// Agent has failed
    Failed,
    /// Agent is offline
    Offline,
}

/// Create the Human @0.A agent (NAM R5)
#[must_use]
pub fn create_human_agent() -> ConsensusAgent {
    ConsensusAgent {
        id: "@0.A".into(),
        role: AgentRole::Validator, // Human can act as any role
        weight: 1.0,
        tier: 0, // Tier 0 (foundation)
        status: AgentStatus::Active,
        success_rate: 1.0,
        last_heartbeat: Some(std::time::SystemTime::now()),
    }
}

/// Default agent fleet (40 agents with heterogeneous roles)
#[must_use]
pub fn default_agent_fleet() -> Vec<ConsensusAgent> {
    let mut agents = vec![create_human_agent()];

    // Validators (20)
    for i in 1..=20 {
        agents.push(ConsensusAgent {
            id: format!("agent-{i:02}"),
            role: AgentRole::Validator,
            weight: 1.0,
            tier: 1,
            status: AgentStatus::Idle,
            success_rate: 0.5,
            last_heartbeat: None,
        });
    }

    // Explorers (8)
    for i in 21..=28 {
        agents.push(ConsensusAgent {
            id: format!("agent-{i:02}"),
            role: AgentRole::Explorer,
            weight: 0.8,
            tier: 2,
            status: AgentStatus::Idle,
            success_rate: 0.5,
            last_heartbeat: None,
        });
    }

    // Critics (6)
    for i in 29..=34 {
        agents.push(ConsensusAgent {
            id: format!("agent-{i:02}"),
            role: AgentRole::Critic,
            weight: 1.2,
            tier: 3,
            status: AgentStatus::Idle,
            success_rate: 0.5,
            last_heartbeat: None,
        });
    }

    // Integrators (4)
    for i in 35..=38 {
        agents.push(ConsensusAgent {
            id: format!("agent-{i:02}"),
            role: AgentRole::Integrator,
            weight: 1.0,
            tier: 4,
            status: AgentStatus::Idle,
            success_rate: 0.5,
            last_heartbeat: None,
        });
    }

    // Historians (2)
    for i in 39..=40 {
        agents.push(ConsensusAgent {
            id: format!("agent-{i:02}"),
            role: AgentRole::Historian,
            weight: 0.8,
            tier: 5,
            status: AgentStatus::Idle,
            success_rate: 0.5,
            last_heartbeat: None,
        });
    }

    agents
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pbft_constants() {
        assert_eq!(PBFT_N, 40);
        assert_eq!(PBFT_F, 13);
        assert_eq!(PBFT_Q, 27);
        // Verify Byzantine formula: q = 2f + 1
        assert_eq!(PBFT_Q, 2 * PBFT_F + 1);
    }

    #[test]
    fn test_quorum_check() {
        assert!(is_quorum_reached(27, 40));
        assert!(is_quorum_reached(30, 40));
        assert!(!is_quorum_reached(26, 40));
        assert!(!is_quorum_reached(27, 26));
    }

    #[test]
    fn test_human_agent() {
        let human = create_human_agent();
        assert_eq!(human.id, "@0.A");
        assert_eq!(human.tier, 0);
        assert!((human.weight - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_agent_fleet() {
        let fleet = default_agent_fleet();
        assert_eq!(fleet.len(), 41); // 40 + Human @0.A

        let validators = fleet
            .iter()
            .filter(|a| a.role == AgentRole::Validator)
            .count();
        let critics = fleet.iter().filter(|a| a.role == AgentRole::Critic).count();
        let integrators = fleet
            .iter()
            .filter(|a| a.role == AgentRole::Integrator)
            .count();

        assert_eq!(validators, 21); // 20 + Human
        assert_eq!(critics, 6);
        assert_eq!(integrators, 4);
    }

    #[test]
    fn test_enhanced_consensus() {
        let votes = vec![
            ConsensusVote {
                proposal_id: "test".into(),
                agent_id: "agent-29".into(),
                vote: VoteType::Approve,
                phase: ConsensusPhase::Prepare,
                role: AgentRole::Critic,
                weight: 1.2,
                reason: None,
                timestamp: std::time::SystemTime::now(),
            },
            ConsensusVote {
                proposal_id: "test".into(),
                agent_id: "agent-35".into(),
                vote: VoteType::Approve,
                phase: ConsensusPhase::Prepare,
                role: AgentRole::Integrator,
                weight: 1.0,
                reason: None,
                timestamp: std::time::SystemTime::now(),
            },
        ];

        assert!(enhanced_consensus_check(&votes));
    }
}
