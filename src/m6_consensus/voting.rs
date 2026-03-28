//! # M33: Vote Collector
//!
//! Vote aggregation and ballot management for PBFT consensus.
//! Tracks individual ballots, prevents duplicate voting, computes tallies
//! with weighted votes, and records voting history.
//!
//! ## Layer: L6 (Consensus)
//!
//! ## Ballot Lifecycle
//!
//! ```text
//! Open -> (votes cast) -> Closed
//!                      \-> Expired
//!                      \-> Cancelled
//! ```
//!
//! ## Related Documentation
//! - [Module Specification](../../ai_docs/modules/M33_VOTE_COLLECTOR.md)

use std::collections::HashMap;
use std::sync::RwLock;
use std::time::SystemTime;

use crate::{Error, Result};

use super::{
    calculate_weighted_votes, is_quorum_reached, AgentRole, ConsensusVote, VoteType, PBFT_N,
};

/// Maximum number of vote history records retained.
const MAX_VOTE_HISTORY: usize = 1000;

/// Status of a ballot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BallotStatus {
    /// Ballot is open and accepting votes.
    Open,
    /// Ballot has been closed (no more votes accepted).
    Closed,
    /// Ballot has expired due to timeout.
    Expired,
    /// Ballot has been cancelled.
    Cancelled,
}

/// A ballot collecting votes for a single proposal.
#[derive(Clone, Debug)]
pub struct Ballot {
    /// The proposal this ballot is for.
    pub proposal_id: String,
    /// Votes cast on this ballot.
    pub votes: Vec<ConsensusVote>,
    /// When the ballot was opened.
    pub opened_at: SystemTime,
    /// When the ballot was closed (if closed).
    pub closed_at: Option<SystemTime>,
    /// Current status.
    pub status: BallotStatus,
    /// Whether quorum has been reached.
    pub quorum_reached: bool,
}

/// A historical record of a single vote cast.
#[derive(Clone, Debug)]
pub struct VoteRecord {
    /// The proposal voted on.
    pub proposal_id: String,
    /// The agent who voted.
    pub agent_id: String,
    /// How the agent voted.
    pub vote_type: VoteType,
    /// Vote weight.
    pub weight: f64,
    /// When the vote was cast.
    pub timestamp: SystemTime,
}

/// Aggregated tally for a proposal's votes.
#[derive(Clone, Debug)]
pub struct VoteTally {
    /// Proposal ID.
    pub proposal_id: String,
    /// Raw count of approval votes.
    pub votes_for: u32,
    /// Raw count of rejection votes.
    pub votes_against: u32,
    /// Raw count of abstention votes.
    pub votes_abstain: u32,
    /// Weighted sum of approval votes.
    pub weighted_for: f64,
    /// Weighted sum of rejection votes.
    pub weighted_against: f64,
    /// Weighted sum of abstention votes.
    pub weighted_abstain: f64,
    /// Whether quorum has been reached.
    pub quorum_reached: bool,
    /// Total number of votes cast.
    pub total_votes: u32,
    /// Participation rate as a fraction of `PBFT_N` (0.0 - 1.0).
    pub participation_rate: f64,
}

/// Vote collector for managing ballots and tallying votes.
///
/// Provides ballot lifecycle management, duplicate vote prevention,
/// role-based vote filtering, and participation tracking.
pub struct VoteCollector {
    /// Active and closed ballots keyed by proposal ID.
    ballots: RwLock<HashMap<String, Ballot>>,
    /// Historical vote records (capped at 1000).
    vote_history: RwLock<Vec<VoteRecord>>,
}

impl VoteCollector {
    /// Create a new empty vote collector.
    #[must_use]
    pub fn new() -> Self {
        Self {
            ballots: RwLock::new(HashMap::new()),
            vote_history: RwLock::new(Vec::new()),
        }
    }

    /// Open a new ballot for a proposal.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if a ballot is already open for the given proposal.
    pub fn open_ballot(&self, proposal_id: &str) -> Result<()> {
        let mut ballots = self
            .ballots
            .write()
            .map_err(|e| Error::Other(format!("Lock poisoned: {e}")))?;

        if ballots.contains_key(proposal_id) {
            return Err(Error::Validation(format!(
                "Ballot already exists for proposal: {proposal_id}"
            )));
        }

        ballots.insert(proposal_id.into(), Ballot {
            proposal_id: proposal_id.into(),
            votes: Vec::new(),
            opened_at: SystemTime::now(),
            closed_at: None,
            status: BallotStatus::Open,
            quorum_reached: false,
        });
        drop(ballots);
        Ok(())
    }

    /// Close a ballot, preventing further votes.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if the ballot is not found or is not open.
    pub fn close_ballot(&self, proposal_id: &str) -> Result<()> {
        let mut ballots = self
            .ballots
            .write()
            .map_err(|e| Error::Other(format!("Lock poisoned: {e}")))?;

        let ballot = ballots
            .get_mut(proposal_id)
            .ok_or_else(|| Error::Validation(format!("Ballot not found: {proposal_id}")))?;

        if ballot.status != BallotStatus::Open {
            return Err(Error::Validation(format!(
                "Ballot is not open (current status: {:?})",
                ballot.status
            )));
        }

        ballot.status = BallotStatus::Closed;
        ballot.closed_at = Some(SystemTime::now());
        drop(ballots);
        Ok(())
    }

    /// Cast a vote on an open ballot.
    ///
    /// Prevents duplicate votes from the same agent on the same proposal.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if:
    /// - The ballot is not found
    /// - The ballot is not open
    /// - The agent has already voted on this ballot
    pub fn cast_vote(&self, proposal_id: &str, vote: ConsensusVote) -> Result<()> {
        let mut ballots = self
            .ballots
            .write()
            .map_err(|e| Error::Other(format!("Lock poisoned: {e}")))?;

        let ballot = ballots
            .get_mut(proposal_id)
            .ok_or_else(|| Error::Validation(format!("Ballot not found: {proposal_id}")))?;

        if ballot.status != BallotStatus::Open {
            return Err(Error::Validation(format!(
                "Cannot vote on a non-open ballot (status: {:?})",
                ballot.status
            )));
        }

        // Prevent duplicate votes
        let already_voted = ballot.votes.iter().any(|v| v.agent_id == vote.agent_id);
        if already_voted {
            return Err(Error::Validation(format!(
                "Agent {} has already voted on proposal {}",
                vote.agent_id, proposal_id
            )));
        }

        // Record in history
        let record = VoteRecord {
            proposal_id: proposal_id.into(),
            agent_id: vote.agent_id.clone(),
            vote_type: vote.vote,
            weight: vote.weight,
            timestamp: vote.timestamp,
        };

        ballot.votes.push(vote);

        // Check quorum after adding vote
        #[allow(clippy::cast_possible_truncation)]
        let approve_count = ballot
            .votes
            .iter()
            .filter(|v| v.vote == VoteType::Approve)
            .count() as u32;
        #[allow(clippy::cast_possible_truncation)]
        let total = ballot.votes.len() as u32;
        ballot.quorum_reached = is_quorum_reached(approve_count, total);

        // Drop the ballots lock before acquiring history lock
        drop(ballots);

        let mut history = self
            .vote_history
            .write()
            .map_err(|e| Error::Other(format!("Lock poisoned: {e}")))?;
        if history.len() >= MAX_VOTE_HISTORY {
            history.remove(0);
        }
        history.push(record);
        drop(history);

        Ok(())
    }

    /// Check if an agent has already voted on a proposal.
    #[must_use]
    pub fn has_voted(&self, proposal_id: &str, agent_id: &str) -> bool {
        let Ok(ballots) = self.ballots.read() else {
            return false;
        };
        ballots.get(proposal_id).is_some_and(|ballot| {
            ballot.votes.iter().any(|v| v.agent_id == agent_id)
        })
    }

    /// Compute the vote tally for a proposal.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if the ballot is not found.
    #[allow(clippy::cast_possible_truncation)]
    pub fn get_tally(&self, proposal_id: &str) -> Result<VoteTally> {
        let ballot = self
            .ballots
            .read()
            .map_err(|e| Error::Other(format!("Lock poisoned: {e}")))?
            .get(proposal_id)
            .ok_or_else(|| Error::Validation(format!("Ballot not found: {proposal_id}")))?
            .clone();

        let votes_for = ballot
            .votes
            .iter()
            .filter(|v| v.vote == VoteType::Approve)
            .count() as u32;
        let votes_against = ballot
            .votes
            .iter()
            .filter(|v| v.vote == VoteType::Reject)
            .count() as u32;
        let votes_abstain = ballot
            .votes
            .iter()
            .filter(|v| v.vote == VoteType::Abstain)
            .count() as u32;

        let (weighted_for, weighted_against, weighted_abstain) =
            calculate_weighted_votes(&ballot.votes);

        let total_votes = ballot.votes.len() as u32;
        let quorum_reached = is_quorum_reached(votes_for, total_votes);

        // Participation rate relative to PBFT_N (40 agents, not counting Human @0.A)
        let participation_rate = if PBFT_N > 0 {
            f64::from(total_votes) / f64::from(PBFT_N)
        } else {
            0.0
        };

        Ok(VoteTally {
            proposal_id: proposal_id.into(),
            votes_for,
            votes_against,
            votes_abstain,
            weighted_for,
            weighted_against,
            weighted_abstain,
            quorum_reached,
            total_votes,
            participation_rate,
        })
    }

    /// Retrieve a ballot by proposal ID.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if the ballot is not found.
    pub fn get_ballot(&self, proposal_id: &str) -> Result<Ballot> {
        let ballots = self
            .ballots
            .read()
            .map_err(|e| Error::Other(format!("Lock poisoned: {e}")))?;
        ballots
            .get(proposal_id)
            .cloned()
            .ok_or_else(|| Error::Validation(format!("Ballot not found: {proposal_id}")))
    }

    /// Get all votes for a proposal cast by agents with a specific role.
    #[must_use]
    pub fn get_votes_by_role(&self, proposal_id: &str, role: AgentRole) -> Vec<ConsensusVote> {
        let Ok(ballots) = self.ballots.read() else {
            return Vec::new();
        };
        ballots.get(proposal_id).map_or_else(Vec::new, |ballot| {
            ballot
                .votes
                .iter()
                .filter(|v| v.role == role)
                .cloned()
                .collect()
        })
    }

    /// Get all dissenting (rejection) votes for a proposal.
    #[must_use]
    pub fn get_dissenting_votes(&self, proposal_id: &str) -> Vec<ConsensusVote> {
        let Ok(ballots) = self.ballots.read() else {
            return Vec::new();
        };
        ballots.get(proposal_id).map_or_else(Vec::new, |ballot| {
            ballot
                .votes
                .iter()
                .filter(|v| v.vote == VoteType::Reject)
                .cloned()
                .collect()
        })
    }

    /// Count the number of currently open ballots.
    #[must_use]
    pub fn open_ballot_count(&self) -> usize {
        let Ok(ballots) = self.ballots.read() else {
            return 0;
        };
        ballots
            .values()
            .filter(|b| b.status == BallotStatus::Open)
            .count()
    }

    /// Get the total number of votes cast across all ballots in the history.
    #[must_use]
    pub fn total_votes_cast(&self) -> u64 {
        let Ok(history) = self.vote_history.read() else {
            return 0;
        };
        history.len() as u64
    }
}

impl Default for VoteCollector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::ConsensusPhase;

    /// Helper to create a test vote.
    fn make_vote(
        proposal_id: &str,
        agent_id: &str,
        vote_type: VoteType,
        role: AgentRole,
        weight: f64,
    ) -> ConsensusVote {
        ConsensusVote {
            proposal_id: proposal_id.into(),
            agent_id: agent_id.into(),
            vote: vote_type,
            phase: ConsensusPhase::Prepare,
            role,
            weight,
            reason: None,
            timestamp: SystemTime::now(),
        }
    }

    #[test]
    fn test_open_ballot() {
        let collector = VoteCollector::new();
        let result = collector.open_ballot("prop-1");
        assert!(result.is_ok());

        let ballot = collector.get_ballot("prop-1").unwrap_or_else(|_| unreachable!());
        assert_eq!(ballot.status, BallotStatus::Open);
        assert_eq!(ballot.votes.len(), 0);
        assert!(ballot.closed_at.is_none());
    }

    #[test]
    fn test_open_ballot_duplicate_fails() {
        let collector = VoteCollector::new();
        let _ = collector.open_ballot("prop-1");
        let result = collector.open_ballot("prop-1");
        assert!(result.is_err());
    }

    #[test]
    fn test_close_ballot() {
        let collector = VoteCollector::new();
        let _ = collector.open_ballot("prop-1");
        let result = collector.close_ballot("prop-1");
        assert!(result.is_ok());

        let ballot = collector.get_ballot("prop-1").unwrap_or_else(|_| unreachable!());
        assert_eq!(ballot.status, BallotStatus::Closed);
        assert!(ballot.closed_at.is_some());
    }

    #[test]
    fn test_close_nonexistent_fails() {
        let collector = VoteCollector::new();
        let result = collector.close_ballot("nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_cast_vote() {
        let collector = VoteCollector::new();
        let _ = collector.open_ballot("prop-1");
        let vote = make_vote("prop-1", "agent-01", VoteType::Approve, AgentRole::Validator, 1.0);
        let result = collector.cast_vote("prop-1", vote);
        assert!(result.is_ok());

        let ballot = collector.get_ballot("prop-1").unwrap_or_else(|_| unreachable!());
        assert_eq!(ballot.votes.len(), 1);
    }

    #[test]
    fn test_duplicate_vote_fails() {
        let collector = VoteCollector::new();
        let _ = collector.open_ballot("prop-1");

        let vote1 = make_vote("prop-1", "agent-01", VoteType::Approve, AgentRole::Validator, 1.0);
        let result1 = collector.cast_vote("prop-1", vote1);
        assert!(result1.is_ok());

        let vote2 = make_vote("prop-1", "agent-01", VoteType::Reject, AgentRole::Validator, 1.0);
        let result2 = collector.cast_vote("prop-1", vote2);
        assert!(result2.is_err());
    }

    #[test]
    fn test_vote_on_closed_fails() {
        let collector = VoteCollector::new();
        let _ = collector.open_ballot("prop-1");
        let _ = collector.close_ballot("prop-1");

        let vote = make_vote("prop-1", "agent-01", VoteType::Approve, AgentRole::Validator, 1.0);
        let result = collector.cast_vote("prop-1", vote);
        assert!(result.is_err());
    }

    #[test]
    fn test_tally_calculation() {
        let collector = VoteCollector::new();
        let _ = collector.open_ballot("prop-1");

        // 5 approvals (validators), 2 rejections (critics), 1 abstention (explorer)
        for i in 1..=5 {
            let vote = make_vote(
                "prop-1",
                &format!("v-{i:02}"),
                VoteType::Approve,
                AgentRole::Validator,
                1.0,
            );
            let _ = collector.cast_vote("prop-1", vote);
        }
        for i in 1..=2 {
            let vote = make_vote(
                "prop-1",
                &format!("c-{i:02}"),
                VoteType::Reject,
                AgentRole::Critic,
                1.2,
            );
            let _ = collector.cast_vote("prop-1", vote);
        }
        let abstain = make_vote("prop-1", "e-01", VoteType::Abstain, AgentRole::Explorer, 0.8);
        let _ = collector.cast_vote("prop-1", abstain);

        let tally = collector.get_tally("prop-1").unwrap_or_else(|_| unreachable!());
        assert_eq!(tally.votes_for, 5);
        assert_eq!(tally.votes_against, 2);
        assert_eq!(tally.votes_abstain, 1);
        assert_eq!(tally.total_votes, 8);
        assert!((tally.weighted_for - 5.0).abs() < f64::EPSILON);
        assert!((tally.weighted_against - 2.4).abs() < f64::EPSILON);
        assert!((tally.weighted_abstain - 0.8).abs() < f64::EPSILON);
    }

    #[test]
    fn test_quorum_check() {
        let collector = VoteCollector::new();
        let _ = collector.open_ballot("prop-1");

        // Submit 27 approval votes to reach quorum
        for i in 1..=27 {
            let vote = make_vote(
                "prop-1",
                &format!("agent-{i:02}"),
                VoteType::Approve,
                AgentRole::Validator,
                1.0,
            );
            let _ = collector.cast_vote("prop-1", vote);
        }

        let tally = collector.get_tally("prop-1").unwrap_or_else(|_| unreachable!());
        assert!(tally.quorum_reached);
        assert_eq!(tally.votes_for, 27);
    }

    #[test]
    fn test_votes_by_role() {
        let collector = VoteCollector::new();
        let _ = collector.open_ballot("prop-1");

        let v1 = make_vote("prop-1", "v-01", VoteType::Approve, AgentRole::Validator, 1.0);
        let v2 = make_vote("prop-1", "c-01", VoteType::Reject, AgentRole::Critic, 1.2);
        let v3 = make_vote("prop-1", "c-02", VoteType::Approve, AgentRole::Critic, 1.2);

        let _ = collector.cast_vote("prop-1", v1);
        let _ = collector.cast_vote("prop-1", v2);
        let _ = collector.cast_vote("prop-1", v3);

        let critic_votes = collector.get_votes_by_role("prop-1", AgentRole::Critic);
        assert_eq!(critic_votes.len(), 2);

        let validator_votes = collector.get_votes_by_role("prop-1", AgentRole::Validator);
        assert_eq!(validator_votes.len(), 1);

        let explorer_votes = collector.get_votes_by_role("prop-1", AgentRole::Explorer);
        assert!(explorer_votes.is_empty());
    }

    #[test]
    fn test_dissenting_votes() {
        let collector = VoteCollector::new();
        let _ = collector.open_ballot("prop-1");

        let v1 = make_vote("prop-1", "agent-01", VoteType::Approve, AgentRole::Validator, 1.0);
        let v2 = make_vote("prop-1", "agent-02", VoteType::Reject, AgentRole::Critic, 1.2);
        let v3 = make_vote("prop-1", "agent-03", VoteType::Reject, AgentRole::Explorer, 0.8);
        let v4 = make_vote("prop-1", "agent-04", VoteType::Abstain, AgentRole::Historian, 0.8);

        let _ = collector.cast_vote("prop-1", v1);
        let _ = collector.cast_vote("prop-1", v2);
        let _ = collector.cast_vote("prop-1", v3);
        let _ = collector.cast_vote("prop-1", v4);

        let dissenting = collector.get_dissenting_votes("prop-1");
        assert_eq!(dissenting.len(), 2);
    }

    #[test]
    fn test_participation_rate() {
        let collector = VoteCollector::new();
        let _ = collector.open_ballot("prop-1");

        // Cast 20 votes out of PBFT_N=40 -> 50% participation
        for i in 1..=20 {
            let vote = make_vote(
                "prop-1",
                &format!("agent-{i:02}"),
                VoteType::Approve,
                AgentRole::Validator,
                1.0,
            );
            let _ = collector.cast_vote("prop-1", vote);
        }

        let tally = collector.get_tally("prop-1").unwrap_or_else(|_| unreachable!());
        assert!((tally.participation_rate - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_has_voted() {
        let collector = VoteCollector::new();
        let _ = collector.open_ballot("prop-1");

        assert!(!collector.has_voted("prop-1", "agent-01"));

        let vote = make_vote("prop-1", "agent-01", VoteType::Approve, AgentRole::Validator, 1.0);
        let _ = collector.cast_vote("prop-1", vote);

        assert!(collector.has_voted("prop-1", "agent-01"));
        assert!(!collector.has_voted("prop-1", "agent-02"));
    }

    #[test]
    fn test_open_ballot_count() {
        let collector = VoteCollector::new();
        assert_eq!(collector.open_ballot_count(), 0);

        let _ = collector.open_ballot("prop-1");
        let _ = collector.open_ballot("prop-2");
        assert_eq!(collector.open_ballot_count(), 2);

        let _ = collector.close_ballot("prop-1");
        assert_eq!(collector.open_ballot_count(), 1);
    }

    #[test]
    fn test_total_votes_cast() {
        let collector = VoteCollector::new();
        assert_eq!(collector.total_votes_cast(), 0);

        let _ = collector.open_ballot("prop-1");
        let v1 = make_vote("prop-1", "agent-01", VoteType::Approve, AgentRole::Validator, 1.0);
        let v2 = make_vote("prop-1", "agent-02", VoteType::Reject, AgentRole::Critic, 1.2);
        let _ = collector.cast_vote("prop-1", v1);
        let _ = collector.cast_vote("prop-1", v2);

        assert_eq!(collector.total_votes_cast(), 2);
    }

    // ---------------------------------------------------------------
    // Additional tests to reach 50+
    // ---------------------------------------------------------------

    #[test]
    fn test_default_impl() {
        let collector = VoteCollector::default();
        assert_eq!(collector.open_ballot_count(), 0);
        assert_eq!(collector.total_votes_cast(), 0);
    }

    #[test]
    fn test_close_already_closed_fails() {
        let collector = VoteCollector::new();
        let _ = collector.open_ballot("prop-1");
        let _ = collector.close_ballot("prop-1");
        let result = collector.close_ballot("prop-1");
        assert!(result.is_err());
    }

    #[test]
    fn test_vote_on_nonexistent_ballot_fails() {
        let collector = VoteCollector::new();
        let vote = make_vote("prop-99", "agent-01", VoteType::Approve, AgentRole::Validator, 1.0);
        let result = collector.cast_vote("prop-99", vote);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_ballot_nonexistent_fails() {
        let collector = VoteCollector::new();
        let result = collector.get_ballot("prop-99");
        assert!(result.is_err());
    }

    #[test]
    fn test_tally_nonexistent_fails() {
        let collector = VoteCollector::new();
        let result = collector.get_tally("prop-99");
        assert!(result.is_err());
    }

    #[test]
    fn test_has_voted_nonexistent_ballot() {
        let collector = VoteCollector::new();
        assert!(!collector.has_voted("nonexistent", "agent-01"));
    }

    #[test]
    fn test_multiple_ballots() {
        let collector = VoteCollector::new();
        let _ = collector.open_ballot("prop-1");
        let _ = collector.open_ballot("prop-2");
        let _ = collector.open_ballot("prop-3");
        assert_eq!(collector.open_ballot_count(), 3);
    }

    #[test]
    fn test_ballot_status_after_open() {
        let collector = VoteCollector::new();
        let _ = collector.open_ballot("prop-1");
        let ballot = collector.get_ballot("prop-1").unwrap_or_else(|_| unreachable!());
        assert_eq!(ballot.status, BallotStatus::Open);
        assert!(!ballot.quorum_reached);
    }

    #[test]
    fn test_quorum_not_reached_below_threshold() {
        let collector = VoteCollector::new();
        let _ = collector.open_ballot("prop-1");

        // Submit 10 approval votes (below quorum of 27)
        for i in 1..=10 {
            let vote = make_vote(
                "prop-1",
                &format!("agent-{i:02}"),
                VoteType::Approve,
                AgentRole::Validator,
                1.0,
            );
            let _ = collector.cast_vote("prop-1", vote);
        }

        let tally = collector.get_tally("prop-1").unwrap_or_else(|_| unreachable!());
        assert!(!tally.quorum_reached);
    }

    #[test]
    fn test_tally_empty_ballot() {
        let collector = VoteCollector::new();
        let _ = collector.open_ballot("prop-1");
        let tally = collector.get_tally("prop-1").unwrap_or_else(|_| unreachable!());
        assert_eq!(tally.total_votes, 0);
        assert_eq!(tally.votes_for, 0);
        assert_eq!(tally.votes_against, 0);
        assert_eq!(tally.votes_abstain, 0);
        assert!((tally.participation_rate).abs() < f64::EPSILON);
    }

    #[test]
    fn test_votes_by_role_empty() {
        let collector = VoteCollector::new();
        let _ = collector.open_ballot("prop-1");
        let votes = collector.get_votes_by_role("prop-1", AgentRole::Historian);
        assert!(votes.is_empty());
    }

    #[test]
    fn test_votes_by_role_nonexistent_ballot() {
        let collector = VoteCollector::new();
        let votes = collector.get_votes_by_role("nonexistent", AgentRole::Validator);
        assert!(votes.is_empty());
    }

    #[test]
    fn test_dissenting_votes_empty() {
        let collector = VoteCollector::new();
        let _ = collector.open_ballot("prop-1");
        let v1 = make_vote("prop-1", "agent-01", VoteType::Approve, AgentRole::Validator, 1.0);
        let _ = collector.cast_vote("prop-1", v1);
        let dissenting = collector.get_dissenting_votes("prop-1");
        assert!(dissenting.is_empty());
    }

    #[test]
    fn test_dissenting_votes_nonexistent_ballot() {
        let collector = VoteCollector::new();
        let votes = collector.get_dissenting_votes("nonexistent");
        assert!(votes.is_empty());
    }

    #[test]
    fn test_ballot_quorum_flag_set() {
        let collector = VoteCollector::new();
        let _ = collector.open_ballot("prop-1");

        for i in 1..=30 {
            let vote = make_vote(
                "prop-1",
                &format!("agent-{i:02}"),
                VoteType::Approve,
                AgentRole::Validator,
                1.0,
            );
            let _ = collector.cast_vote("prop-1", vote);
        }

        let ballot = collector.get_ballot("prop-1").unwrap_or_else(|_| unreachable!());
        assert!(ballot.quorum_reached);
    }

    #[test]
    fn test_total_votes_across_ballots() {
        let collector = VoteCollector::new();
        let _ = collector.open_ballot("prop-1");
        let _ = collector.open_ballot("prop-2");

        let v1 = make_vote("prop-1", "agent-01", VoteType::Approve, AgentRole::Validator, 1.0);
        let v2 = make_vote("prop-2", "agent-01", VoteType::Reject, AgentRole::Validator, 1.0);
        let _ = collector.cast_vote("prop-1", v1);
        let _ = collector.cast_vote("prop-2", v2);

        assert_eq!(collector.total_votes_cast(), 2);
    }

    #[test]
    fn test_same_agent_different_ballots() {
        let collector = VoteCollector::new();
        let _ = collector.open_ballot("prop-1");
        let _ = collector.open_ballot("prop-2");

        let v1 = make_vote("prop-1", "agent-01", VoteType::Approve, AgentRole::Validator, 1.0);
        let v2 = make_vote("prop-2", "agent-01", VoteType::Approve, AgentRole::Validator, 1.0);
        assert!(collector.cast_vote("prop-1", v1).is_ok());
        assert!(collector.cast_vote("prop-2", v2).is_ok());
    }

    #[test]
    fn test_weighted_for_calculation() {
        let collector = VoteCollector::new();
        let _ = collector.open_ballot("prop-1");

        let v1 = make_vote("prop-1", "v-01", VoteType::Approve, AgentRole::Validator, 1.0);
        let v2 = make_vote("prop-1", "c-01", VoteType::Approve, AgentRole::Critic, 1.2);
        let _ = collector.cast_vote("prop-1", v1);
        let _ = collector.cast_vote("prop-1", v2);

        let tally = collector.get_tally("prop-1").unwrap_or_else(|_| unreachable!());
        assert!((tally.weighted_for - 2.2).abs() < f64::EPSILON);
    }

    #[test]
    fn test_participation_rate_full() {
        let collector = VoteCollector::new();
        let _ = collector.open_ballot("prop-1");

        for i in 1..=40 {
            let vote = make_vote(
                "prop-1",
                &format!("agent-{i:02}"),
                VoteType::Approve,
                AgentRole::Validator,
                1.0,
            );
            let _ = collector.cast_vote("prop-1", vote);
        }

        let tally = collector.get_tally("prop-1").unwrap_or_else(|_| unreachable!());
        assert!((tally.participation_rate - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_close_ballot_multiple_independent() {
        let collector = VoteCollector::new();
        let _ = collector.open_ballot("prop-1");
        let _ = collector.open_ballot("prop-2");

        let _ = collector.close_ballot("prop-1");
        assert_eq!(collector.open_ballot_count(), 1);

        let _ = collector.close_ballot("prop-2");
        assert_eq!(collector.open_ballot_count(), 0);
    }

    #[test]
    fn test_ballot_vote_count() {
        let collector = VoteCollector::new();
        let _ = collector.open_ballot("prop-1");

        for i in 1..=5 {
            let vote = make_vote(
                "prop-1",
                &format!("agent-{i:02}"),
                VoteType::Approve,
                AgentRole::Validator,
                1.0,
            );
            let _ = collector.cast_vote("prop-1", vote);
        }

        let ballot = collector.get_ballot("prop-1").unwrap_or_else(|_| unreachable!());
        assert_eq!(ballot.votes.len(), 5);
    }

    #[test]
    fn test_all_abstain() {
        let collector = VoteCollector::new();
        let _ = collector.open_ballot("prop-1");

        for i in 1..=5 {
            let vote = make_vote(
                "prop-1",
                &format!("agent-{i:02}"),
                VoteType::Abstain,
                AgentRole::Explorer,
                0.8,
            );
            let _ = collector.cast_vote("prop-1", vote);
        }

        let tally = collector.get_tally("prop-1").unwrap_or_else(|_| unreachable!());
        assert_eq!(tally.votes_abstain, 5);
        assert_eq!(tally.votes_for, 0);
        assert!(!tally.quorum_reached);
    }

    #[test]
    fn test_all_reject() {
        let collector = VoteCollector::new();
        let _ = collector.open_ballot("prop-1");

        for i in 1..=30 {
            let vote = make_vote(
                "prop-1",
                &format!("agent-{i:02}"),
                VoteType::Reject,
                AgentRole::Critic,
                1.2,
            );
            let _ = collector.cast_vote("prop-1", vote);
        }

        let tally = collector.get_tally("prop-1").unwrap_or_else(|_| unreachable!());
        assert_eq!(tally.votes_against, 30);
        assert!(!tally.quorum_reached);
    }

    #[test]
    fn test_open_ballot_count_zero_initially() {
        let collector = VoteCollector::new();
        assert_eq!(collector.open_ballot_count(), 0);
    }

    #[test]
    fn test_ballot_quorum_not_reached_initially() {
        let collector = VoteCollector::new();
        let _ = collector.open_ballot("prop-1");
        let ballot = collector.get_ballot("prop-1").unwrap_or_else(|_| unreachable!());
        assert!(!ballot.quorum_reached);
    }

    #[test]
    fn test_cast_vote_role_preserved() {
        let collector = VoteCollector::new();
        let _ = collector.open_ballot("prop-1");
        let vote = make_vote("prop-1", "c-01", VoteType::Reject, AgentRole::Critic, 1.2);
        let _ = collector.cast_vote("prop-1", vote);
        let votes = collector.get_votes_by_role("prop-1", AgentRole::Critic);
        assert_eq!(votes.len(), 1);
        assert_eq!(votes[0].role, AgentRole::Critic);
    }

    #[test]
    fn test_dissenting_votes_only_rejects() {
        let collector = VoteCollector::new();
        let _ = collector.open_ballot("prop-1");
        let v1 = make_vote("prop-1", "agent-01", VoteType::Approve, AgentRole::Validator, 1.0);
        let v2 = make_vote("prop-1", "agent-02", VoteType::Reject, AgentRole::Validator, 1.0);
        let v3 = make_vote("prop-1", "agent-03", VoteType::Abstain, AgentRole::Explorer, 0.8);
        let _ = collector.cast_vote("prop-1", v1);
        let _ = collector.cast_vote("prop-1", v2);
        let _ = collector.cast_vote("prop-1", v3);
        let dissenting = collector.get_dissenting_votes("prop-1");
        assert_eq!(dissenting.len(), 1);
        assert_eq!(dissenting[0].vote, VoteType::Reject);
    }

    #[test]
    fn test_participation_rate_quarter() {
        let collector = VoteCollector::new();
        let _ = collector.open_ballot("prop-1");
        for i in 1..=10 {
            let vote = make_vote(
                "prop-1", &format!("agent-{i:02}"),
                VoteType::Approve, AgentRole::Validator, 1.0,
            );
            let _ = collector.cast_vote("prop-1", vote);
        }
        let tally = collector.get_tally("prop-1").unwrap_or_else(|_| unreachable!());
        assert!((tally.participation_rate - 0.25).abs() < f64::EPSILON);
    }

    #[test]
    fn test_get_ballot_proposal_id_preserved() {
        let collector = VoteCollector::new();
        let _ = collector.open_ballot("my-special-prop");
        let ballot = collector.get_ballot("my-special-prop").unwrap_or_else(|_| unreachable!());
        assert_eq!(ballot.proposal_id, "my-special-prop");
    }

    #[test]
    fn test_weighted_against_calculation() {
        let collector = VoteCollector::new();
        let _ = collector.open_ballot("prop-1");
        let v1 = make_vote("prop-1", "c-01", VoteType::Reject, AgentRole::Critic, 1.2);
        let v2 = make_vote("prop-1", "c-02", VoteType::Reject, AgentRole::Critic, 1.2);
        let _ = collector.cast_vote("prop-1", v1);
        let _ = collector.cast_vote("prop-1", v2);
        let tally = collector.get_tally("prop-1").unwrap_or_else(|_| unreachable!());
        assert!((tally.weighted_against - 2.4).abs() < f64::EPSILON);
    }

    #[test]
    fn test_weighted_abstain_calculation() {
        let collector = VoteCollector::new();
        let _ = collector.open_ballot("prop-1");
        let v1 = make_vote("prop-1", "e-01", VoteType::Abstain, AgentRole::Explorer, 0.8);
        let v2 = make_vote("prop-1", "e-02", VoteType::Abstain, AgentRole::Explorer, 0.8);
        let _ = collector.cast_vote("prop-1", v1);
        let _ = collector.cast_vote("prop-1", v2);
        let tally = collector.get_tally("prop-1").unwrap_or_else(|_| unreachable!());
        assert!((tally.weighted_abstain - 1.6).abs() < f64::EPSILON);
    }

    #[test]
    fn test_ballot_opened_at_set() {
        let collector = VoteCollector::new();
        let _ = collector.open_ballot("prop-1");
        let ballot = collector.get_ballot("prop-1").unwrap_or_else(|_| unreachable!());
        let elapsed = ballot.opened_at.elapsed();
        assert!(elapsed.is_ok());
    }

    #[test]
    fn test_tally_total_votes_correct() {
        let collector = VoteCollector::new();
        let _ = collector.open_ballot("prop-1");
        let v1 = make_vote("prop-1", "a1", VoteType::Approve, AgentRole::Validator, 1.0);
        let v2 = make_vote("prop-1", "a2", VoteType::Reject, AgentRole::Critic, 1.2);
        let v3 = make_vote("prop-1", "a3", VoteType::Abstain, AgentRole::Explorer, 0.8);
        let _ = collector.cast_vote("prop-1", v1);
        let _ = collector.cast_vote("prop-1", v2);
        let _ = collector.cast_vote("prop-1", v3);
        let tally = collector.get_tally("prop-1").unwrap_or_else(|_| unreachable!());
        assert_eq!(tally.total_votes, 3);
        assert_eq!(tally.votes_for + tally.votes_against + tally.votes_abstain, 3);
    }

    #[test]
    fn test_tally_proposal_id_preserved() {
        let collector = VoteCollector::new();
        let _ = collector.open_ballot("my-prop-42");
        let tally = collector.get_tally("my-prop-42").unwrap_or_else(|_| unreachable!());
        assert_eq!(tally.proposal_id, "my-prop-42");
    }

    #[test]
    fn test_ballot_status_enum_values() {
        assert_ne!(BallotStatus::Open, BallotStatus::Closed);
        assert_ne!(BallotStatus::Open, BallotStatus::Expired);
        assert_ne!(BallotStatus::Open, BallotStatus::Cancelled);
    }
}
