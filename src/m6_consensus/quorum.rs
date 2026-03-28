//! # M36: Quorum Calculator
//!
//! Determines whether consensus requirements are met for PBFT proposal
//! acceptance. Supports simple quorum (raw vote count), weighted quorum
//! (role-based weights), and enhanced quorum (requiring CRITIC + INTEGRATOR
//! approval).
//!
//! ## Layer: L6 (Consensus)
//!
//! ## Quorum Modes
//!
//! | Mode | Description | Threshold |
//! |------|-------------|-----------|
//! | Simple | Raw vote count >= `PBFT_Q` | 27/40 |
//! | Weighted | Weighted vote sum >= threshold | 27.0 |
//! | Enhanced | Simple + CRITIC + INTEGRATOR | 27/40 + roles |
//!
//! ## Related Documentation
//! - [Module Specification](../../ai_docs/modules/M36_QUORUM_CALCULATOR.md)
//! - [PBFT Consensus](../../nam/PBFT_CONSENSUS.md)

use std::sync::RwLock;
use std::time::SystemTime;

use crate::{Error, Result, AgentRole};

use super::{
    ConsensusAction, ConsensusVote, VoteType,
    PBFT_N, PBFT_Q,
    calculate_weighted_votes, is_quorum_reached,
};

/// Maximum number of quorum history records retained.
const MAX_QUORUM_HISTORY: usize = 500;

/// Quorum requirement mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuorumRequirement {
    /// Simple majority: raw vote count >= `PBFT_Q`.
    Simple,
    /// Weighted majority: weighted vote sum >= configured threshold.
    Weighted,
    /// Enhanced: simple quorum + at least one CRITIC and one INTEGRATOR approval.
    Enhanced,
}

/// Result of a quorum evaluation.
#[derive(Clone, Debug)]
pub struct QuorumResult {
    /// Whether the quorum requirement was met.
    pub met: bool,
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
    /// Total number of votes cast.
    pub total_votes: u32,
    /// Participation rate as a fraction of `PBFT_N` (0.0 - 1.0).
    pub participation_rate: f64,
    /// Whether at least one CRITIC voted Approve.
    pub has_critic_approval: bool,
    /// Whether at least one INTEGRATOR voted Approve.
    pub has_integrator_approval: bool,
    /// The quorum requirement that was evaluated.
    pub requirement: QuorumRequirement,
    /// Human-readable evaluation details.
    pub evaluation_details: String,
}

/// Configuration for quorum evaluation.
#[derive(Clone, Debug)]
pub struct QuorumConfig {
    /// Minimum fraction of `PBFT_N` that must vote (0.0 - 1.0).
    pub min_participation_rate: f64,
    /// Whether a CRITIC approval is required.
    pub require_critic: bool,
    /// Whether an INTEGRATOR approval is required.
    pub require_integrator: bool,
    /// Weighted vote threshold for the Weighted quorum mode.
    pub weighted_threshold: f64,
}

impl Default for QuorumConfig {
    fn default() -> Self {
        Self {
            min_participation_rate: 0.5,
            require_critic: true,
            require_integrator: true,
            weighted_threshold: 27.0,
        }
    }
}

/// A historical record of a quorum evaluation.
#[derive(Clone, Debug)]
pub struct QuorumHistory {
    /// The proposal that was evaluated.
    pub proposal_id: String,
    /// The quorum evaluation result.
    pub result: QuorumResult,
    /// The consensus action type.
    pub action_type: ConsensusAction,
    /// When the evaluation occurred.
    pub timestamp: SystemTime,
}

/// Quorum calculator for PBFT consensus.
///
/// Evaluates whether a set of votes meets the configured quorum requirements.
/// Tracks evaluation history and supports runtime configuration changes.
pub struct QuorumCalculator {
    /// Active configuration.
    config: RwLock<QuorumConfig>,
    /// Evaluation history (capped at [`MAX_QUORUM_HISTORY`]).
    history: RwLock<Vec<QuorumHistory>>,
    /// Total number of evaluations performed.
    evaluations: RwLock<u64>,
}

impl QuorumCalculator {
    /// Create a new quorum calculator with default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: RwLock::new(QuorumConfig::default()),
            history: RwLock::new(Vec::new()),
            evaluations: RwLock::new(0),
        }
    }

    /// Create a new quorum calculator with the given configuration.
    #[must_use]
    #[allow(clippy::missing_const_for_fn)]
    pub fn with_config(config: QuorumConfig) -> Self {
        Self {
            config: RwLock::new(config),
            history: RwLock::new(Vec::new()),
            evaluations: RwLock::new(0),
        }
    }

    /// Evaluate whether quorum requirements are met for a proposal.
    ///
    /// Performs all checks: simple quorum, weighted quorum, enhanced quorum,
    /// and participation rate. For critical actions (`ServiceTermination`,
    /// `DatabaseMigration`, `CascadeRestart`), the Enhanced requirement is
    /// always enforced regardless of configuration.
    ///
    /// # Errors
    ///
    /// Returns `Error::Other` if a lock is poisoned.
    #[allow(clippy::cast_possible_truncation)]
    pub fn evaluate(
        &self,
        proposal_id: &str,
        action: ConsensusAction,
        votes: &[ConsensusVote],
    ) -> Result<QuorumResult> {
        let config = self
            .config
            .read()
            .map_err(|_| Error::Other("Lock poisoned".into()))?;

        // Determine the requirement level: critical actions always need
        // Enhanced; non-critical use Enhanced if config requires roles.
        let requirement = if Self::is_critical_action(&action)
            || config.require_critic
            || config.require_integrator
        {
            QuorumRequirement::Enhanced
        } else {
            QuorumRequirement::Simple
        };

        // Simple quorum check
        let (simple_met, votes_for, votes_against, votes_abstain) =
            Self::simple_quorum_check(votes);

        // Weighted quorum check
        let (weighted_met, weighted_for, weighted_against, weighted_abstain) =
            Self::weighted_quorum_check(votes);

        // Enhanced quorum check
        let (has_critic_approval, has_integrator_approval) =
            Self::enhanced_quorum_check(votes);

        // Participation rate
        let total_votes = votes_for + votes_against + votes_abstain;
        let participation = Self::participation_rate(total_votes);

        // Evaluate whether quorum is met based on requirement level
        let participation_ok = participation >= config.min_participation_rate;

        let met = match requirement {
            QuorumRequirement::Simple => simple_met && participation_ok,
            QuorumRequirement::Weighted => {
                weighted_for >= config.weighted_threshold && participation_ok
            }
            QuorumRequirement::Enhanced => {
                let role_ok = (!config.require_critic || has_critic_approval)
                    && (!config.require_integrator || has_integrator_approval);
                simple_met && role_ok && participation_ok
            }
        };

        // Build evaluation details
        let details = format!(
            "requirement={requirement:?}, simple={simple_met}, weighted={weighted_met} \
             (for={weighted_for:.1}, threshold={threshold:.1}), \
             critic={has_critic_approval}, integrator={has_integrator_approval}, \
             participation={participation:.2} (min={min_p:.2}), result={met}",
            threshold = config.weighted_threshold,
            min_p = config.min_participation_rate,
        );

        let result = QuorumResult {
            met,
            votes_for,
            votes_against,
            votes_abstain,
            weighted_for,
            weighted_against,
            weighted_abstain,
            total_votes,
            participation_rate: participation,
            has_critic_approval,
            has_integrator_approval,
            requirement,
            evaluation_details: details,
        };

        // Drop config before acquiring write locks
        drop(config);

        // Record in history
        {
            let mut history = self
                .history
                .write()
                .map_err(|_| Error::Other("Lock poisoned".into()))?;
            if history.len() >= MAX_QUORUM_HISTORY {
                history.remove(0);
            }
            history.push(QuorumHistory {
                proposal_id: proposal_id.into(),
                result: result.clone(),
                action_type: action,
                timestamp: SystemTime::now(),
            });
        }

        // Increment evaluation counter
        {
            let mut count = self
                .evaluations
                .write()
                .map_err(|_| Error::Other("Lock poisoned".into()))?;
            *count += 1;
        }

        Ok(result)
    }

    /// Perform a simple quorum check: raw approval vote count >= `PBFT_Q`.
    ///
    /// Returns `(quorum_met, votes_for, votes_against, votes_abstain)`.
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub fn simple_quorum_check(votes: &[ConsensusVote]) -> (bool, u32, u32, u32) {
        let votes_for = votes
            .iter()
            .filter(|v| v.vote == VoteType::Approve)
            .count() as u32;
        let votes_against = votes
            .iter()
            .filter(|v| v.vote == VoteType::Reject)
            .count() as u32;
        let votes_abstain = votes
            .iter()
            .filter(|v| v.vote == VoteType::Abstain)
            .count() as u32;
        let total = votes_for + votes_against + votes_abstain;
        let met = is_quorum_reached(votes_for, total);
        (met, votes_for, votes_against, votes_abstain)
    }

    /// Perform a weighted quorum check using role-based vote weights.
    ///
    /// Returns `(weighted_met, weighted_for, weighted_against, weighted_abstain)`.
    /// Weighted quorum is met when `weighted_for >= PBFT_Q` as a float.
    #[must_use]
    pub fn weighted_quorum_check(votes: &[ConsensusVote]) -> (bool, f64, f64, f64) {
        let (weighted_for, weighted_against, weighted_abstain) =
            calculate_weighted_votes(votes);
        let met = weighted_for >= f64::from(PBFT_Q);
        (met, weighted_for, weighted_against, weighted_abstain)
    }

    /// Check whether at least one CRITIC and one INTEGRATOR approved.
    ///
    /// Returns `(has_critic_approval, has_integrator_approval)`.
    #[must_use]
    pub fn enhanced_quorum_check(votes: &[ConsensusVote]) -> (bool, bool) {
        let has_critic = votes
            .iter()
            .any(|v| v.role == AgentRole::Critic && v.vote == VoteType::Approve);
        let has_integrator = votes
            .iter()
            .any(|v| v.role == AgentRole::Integrator && v.vote == VoteType::Approve);
        (has_critic, has_integrator)
    }

    /// Calculate participation rate as a fraction of `PBFT_N`.
    ///
    /// Returns a value in the range `[0.0, inf)` where 1.0 means all
    /// `PBFT_N` agents voted. Values above 1.0 are theoretically impossible
    /// but are not clamped to support diagnostic visibility.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn participation_rate(total_votes: u32) -> f64 {
        if PBFT_N == 0 {
            return 0.0;
        }
        f64::from(total_votes) / f64::from(PBFT_N)
    }

    /// Determine whether an action is critical (always requires Enhanced quorum).
    ///
    /// Critical actions: `ServiceTermination`, `DatabaseMigration`, `CascadeRestart`.
    #[must_use]
    pub const fn is_critical_action(action: &ConsensusAction) -> bool {
        matches!(
            action,
            ConsensusAction::ServiceTermination
                | ConsensusAction::DatabaseMigration
                | ConsensusAction::CascadeRestart
        )
    }

    /// Retrieve a snapshot of the quorum evaluation history.
    ///
    /// Returns a cloned copy of all recorded history entries
    /// (up to [`MAX_QUORUM_HISTORY`]).
    #[must_use]
    pub fn quorum_history(&self) -> Vec<QuorumHistory> {
        let Ok(history) = self.history.read() else {
            return Vec::new();
        };
        history.clone()
    }

    /// Get the total number of evaluations performed.
    #[must_use]
    pub fn evaluation_count(&self) -> u64 {
        let Ok(count) = self.evaluations.read() else {
            return 0;
        };
        *count
    }

    /// Calculate the approval rate from the evaluation history.
    ///
    /// Returns the fraction of evaluations where quorum was met.
    /// Returns 0.0 if no evaluations have been recorded.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn approval_rate(&self) -> f64 {
        let Ok(history) = self.history.read() else {
            return 0.0;
        };
        if history.is_empty() {
            return 0.0;
        }
        let approved = history.iter().filter(|h| h.result.met).count();
        approved as f64 / history.len() as f64
    }

    /// Update the quorum configuration at runtime.
    ///
    /// # Errors
    ///
    /// Returns `Error::Other` if the lock is poisoned.
    pub fn set_config(&self, config: QuorumConfig) -> Result<()> {
        let mut guard = self
            .config
            .write()
            .map_err(|_| Error::Other("Lock poisoned".into()))?;
        *guard = config;
        drop(guard);
        Ok(())
    }

    /// Retrieve a snapshot of the current quorum configuration.
    ///
    /// # Errors
    ///
    /// Returns `Error::Other` if the lock is poisoned.
    pub fn get_config(&self) -> Result<QuorumConfig> {
        let guard = self
            .config
            .read()
            .map_err(|_| Error::Other("Lock poisoned".into()))?;
        Ok(guard.clone())
    }

    /// Return the minimum number of approval votes needed for simple quorum.
    ///
    /// This is the PBFT quorum constant `q = 2f + 1 = 27`.
    #[must_use]
    pub const fn minimum_votes_needed() -> u32 {
        PBFT_Q
    }

    /// Clear all history entries.
    ///
    /// Does not reset the evaluation counter.
    pub fn clear_history(&self) {
        let Ok(mut history) = self.history.write() else {
            return;
        };
        history.clear();
    }
}

impl Default for QuorumCalculator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::ConsensusPhase;

    /// Helper: create a vote with the given parameters.
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

    /// Helper: create N approval votes from Validators (weight 1.0).
    fn make_validator_approvals(n: usize) -> Vec<ConsensusVote> {
        (0..n)
            .map(|i| make_vote("prop-1", &format!("v-{i:03}"), VoteType::Approve, AgentRole::Validator, 1.0))
            .collect()
    }

    /// Helper: build a mixed vote set with critic and integrator approvals.
    fn make_full_quorum_votes() -> Vec<ConsensusVote> {
        let mut votes: Vec<ConsensusVote> = (0..25)
            .map(|i| make_vote("prop-1", &format!("v-{i:03}"), VoteType::Approve, AgentRole::Validator, 1.0))
            .collect();
        votes.push(make_vote("prop-1", "critic-01", VoteType::Approve, AgentRole::Critic, 1.2));
        votes.push(make_vote("prop-1", "integ-01", VoteType::Approve, AgentRole::Integrator, 1.0));
        votes
    }

    // ========================================================================
    // Simple quorum check tests (1-6)
    // ========================================================================

    #[test]
    fn test_simple_quorum_met_at_27() {
        let votes = make_validator_approvals(27);
        let (met, vf, va, ab) = QuorumCalculator::simple_quorum_check(&votes);
        assert!(met);
        assert_eq!(vf, 27);
        assert_eq!(va, 0);
        assert_eq!(ab, 0);
    }

    #[test]
    fn test_simple_quorum_not_met_at_26() {
        let votes = make_validator_approvals(26);
        let (met, vf, _, _) = QuorumCalculator::simple_quorum_check(&votes);
        assert!(!met);
        assert_eq!(vf, 26);
    }

    #[test]
    fn test_simple_quorum_met_above_threshold() {
        let votes = make_validator_approvals(35);
        let (met, vf, _, _) = QuorumCalculator::simple_quorum_check(&votes);
        assert!(met);
        assert_eq!(vf, 35);
    }

    #[test]
    fn test_simple_quorum_no_votes() {
        let votes: Vec<ConsensusVote> = Vec::new();
        let (met, vf, va, ab) = QuorumCalculator::simple_quorum_check(&votes);
        assert!(!met);
        assert_eq!(vf, 0);
        assert_eq!(va, 0);
        assert_eq!(ab, 0);
    }

    #[test]
    fn test_simple_quorum_all_reject() {
        let votes: Vec<ConsensusVote> = (0..30)
            .map(|i| make_vote("prop-1", &format!("v-{i:03}"), VoteType::Reject, AgentRole::Validator, 1.0))
            .collect();
        let (met, vf, va, _) = QuorumCalculator::simple_quorum_check(&votes);
        assert!(!met);
        assert_eq!(vf, 0);
        assert_eq!(va, 30);
    }

    #[test]
    fn test_simple_quorum_mixed_votes() {
        let mut votes = make_validator_approvals(20);
        for i in 0..5 {
            votes.push(make_vote("prop-1", &format!("r-{i:03}"), VoteType::Reject, AgentRole::Validator, 1.0));
        }
        for i in 0..3 {
            votes.push(make_vote("prop-1", &format!("a-{i:03}"), VoteType::Abstain, AgentRole::Explorer, 0.8));
        }
        let (met, vf, va, ab) = QuorumCalculator::simple_quorum_check(&votes);
        assert!(!met);
        assert_eq!(vf, 20);
        assert_eq!(va, 5);
        assert_eq!(ab, 3);
    }

    // ========================================================================
    // Weighted quorum check tests (7-12)
    // ========================================================================

    #[test]
    fn test_weighted_quorum_met_validators() {
        let votes = make_validator_approvals(27);
        let (met, wf, wa, wab) = QuorumCalculator::weighted_quorum_check(&votes);
        assert!(met);
        assert!((wf - 27.0).abs() < f64::EPSILON);
        assert!((wa - 0.0).abs() < f64::EPSILON);
        assert!((wab - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_weighted_quorum_not_met_below_threshold() {
        let votes = make_validator_approvals(26);
        let (met, wf, _, _) = QuorumCalculator::weighted_quorum_check(&votes);
        assert!(!met);
        assert!((wf - 26.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_weighted_quorum_critic_weight() {
        // 22 validators (22.0) + 5 critics at 1.2 (6.0) = 28.0 >= 27.0
        let mut votes = make_validator_approvals(22);
        for i in 0..5 {
            votes.push(make_vote("prop-1", &format!("c-{i:03}"), VoteType::Approve, AgentRole::Critic, 1.2));
        }
        let (met, wf, _, _) = QuorumCalculator::weighted_quorum_check(&votes);
        assert!(met);
        assert!((wf - 28.0).abs() < 1e-10);
    }

    #[test]
    fn test_weighted_quorum_explorer_weight() {
        // Explorers have weight 0.8: 27 explorers => 21.6 < 27.0
        let votes: Vec<ConsensusVote> = (0..27)
            .map(|i| make_vote("prop-1", &format!("e-{i:03}"), VoteType::Approve, AgentRole::Explorer, 0.8))
            .collect();
        let (met, wf, _, _) = QuorumCalculator::weighted_quorum_check(&votes);
        assert!(!met);
        assert!((wf - 21.6).abs() < 0.001);
    }

    #[test]
    fn test_weighted_quorum_reject_weight() {
        let votes: Vec<ConsensusVote> = (0..30)
            .map(|i| make_vote("prop-1", &format!("v-{i:03}"), VoteType::Reject, AgentRole::Critic, 1.2))
            .collect();
        let (met, _, wa, _) = QuorumCalculator::weighted_quorum_check(&votes);
        assert!(!met);
        assert!((wa - 36.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_weighted_quorum_abstain_weight() {
        let votes: Vec<ConsensusVote> = (0..10)
            .map(|i| make_vote("prop-1", &format!("h-{i:03}"), VoteType::Abstain, AgentRole::Historian, 0.8))
            .collect();
        let (met, _, _, wab) = QuorumCalculator::weighted_quorum_check(&votes);
        assert!(!met);
        assert!((wab - 8.0).abs() < 1e-10);
    }

    // ========================================================================
    // Enhanced quorum check tests (13-18)
    // ========================================================================

    #[test]
    fn test_enhanced_both_present() {
        let votes = vec![
            make_vote("prop-1", "c-01", VoteType::Approve, AgentRole::Critic, 1.2),
            make_vote("prop-1", "i-01", VoteType::Approve, AgentRole::Integrator, 1.0),
        ];
        let (critic, integrator) = QuorumCalculator::enhanced_quorum_check(&votes);
        assert!(critic);
        assert!(integrator);
    }

    #[test]
    fn test_enhanced_critic_only() {
        let votes = vec![
            make_vote("prop-1", "c-01", VoteType::Approve, AgentRole::Critic, 1.2),
        ];
        let (critic, integrator) = QuorumCalculator::enhanced_quorum_check(&votes);
        assert!(critic);
        assert!(!integrator);
    }

    #[test]
    fn test_enhanced_integrator_only() {
        let votes = vec![
            make_vote("prop-1", "i-01", VoteType::Approve, AgentRole::Integrator, 1.0),
        ];
        let (critic, integrator) = QuorumCalculator::enhanced_quorum_check(&votes);
        assert!(!critic);
        assert!(integrator);
    }

    #[test]
    fn test_enhanced_neither_present() {
        let votes = make_validator_approvals(30);
        let (critic, integrator) = QuorumCalculator::enhanced_quorum_check(&votes);
        assert!(!critic);
        assert!(!integrator);
    }

    #[test]
    fn test_enhanced_critic_rejects() {
        let votes = vec![
            make_vote("prop-1", "c-01", VoteType::Reject, AgentRole::Critic, 1.2),
            make_vote("prop-1", "i-01", VoteType::Approve, AgentRole::Integrator, 1.0),
        ];
        let (critic, integrator) = QuorumCalculator::enhanced_quorum_check(&votes);
        assert!(!critic);
        assert!(integrator);
    }

    #[test]
    fn test_enhanced_integrator_abstains() {
        let votes = vec![
            make_vote("prop-1", "c-01", VoteType::Approve, AgentRole::Critic, 1.2),
            make_vote("prop-1", "i-01", VoteType::Abstain, AgentRole::Integrator, 1.0),
        ];
        let (critic, integrator) = QuorumCalculator::enhanced_quorum_check(&votes);
        assert!(critic);
        assert!(!integrator);
    }

    // ========================================================================
    // Critical action detection tests (19-23)
    // ========================================================================

    #[test]
    fn test_critical_service_termination() {
        assert!(QuorumCalculator::is_critical_action(&ConsensusAction::ServiceTermination));
    }

    #[test]
    fn test_critical_database_migration() {
        assert!(QuorumCalculator::is_critical_action(&ConsensusAction::DatabaseMigration));
    }

    #[test]
    fn test_critical_cascade_restart() {
        assert!(QuorumCalculator::is_critical_action(&ConsensusAction::CascadeRestart));
    }

    #[test]
    fn test_non_critical_credential_rotation() {
        assert!(!QuorumCalculator::is_critical_action(&ConsensusAction::CredentialRotation));
    }

    #[test]
    fn test_non_critical_config_rollback() {
        assert!(!QuorumCalculator::is_critical_action(&ConsensusAction::ConfigRollback));
    }

    // ========================================================================
    // Participation rate tests (24-27)
    // ========================================================================

    #[test]
    fn test_participation_rate_full() {
        let rate = QuorumCalculator::participation_rate(PBFT_N);
        assert!((rate - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_participation_rate_half() {
        let rate = QuorumCalculator::participation_rate(20);
        assert!((rate - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_participation_rate_zero() {
        let rate = QuorumCalculator::participation_rate(0);
        assert!((rate - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_participation_rate_quorum() {
        let rate = QuorumCalculator::participation_rate(PBFT_Q);
        // 27 / 40 = 0.675
        assert!((rate - 0.675).abs() < f64::EPSILON);
    }

    // ========================================================================
    // Full evaluate flow tests (28-37)
    // ========================================================================

    #[test]
    fn test_evaluate_full_quorum_met() {
        let calc = QuorumCalculator::new();
        let votes = make_full_quorum_votes();
        let result = calc
            .evaluate("prop-1", ConsensusAction::ConfigRollback, &votes)
            .unwrap_or_else(|_| unreachable!());
        assert!(result.met);
        assert_eq!(result.votes_for, 27);
        assert_eq!(result.total_votes, 27);
        assert!(result.has_critic_approval);
        assert!(result.has_integrator_approval);
    }

    #[test]
    fn test_evaluate_quorum_not_met_insufficient_votes() {
        let calc = QuorumCalculator::new();
        let votes = make_validator_approvals(10);
        let result = calc
            .evaluate("prop-1", ConsensusAction::ConfigRollback, &votes)
            .unwrap_or_else(|_| unreachable!());
        assert!(!result.met);
        assert_eq!(result.votes_for, 10);
    }

    #[test]
    fn test_evaluate_critical_action_needs_enhanced() {
        let calc = QuorumCalculator::new();
        // 27 validators but no critic or integrator
        let votes = make_validator_approvals(27);
        let result = calc
            .evaluate("prop-1", ConsensusAction::ServiceTermination, &votes)
            .unwrap_or_else(|_| unreachable!());
        assert!(!result.met);
        assert_eq!(result.requirement, QuorumRequirement::Enhanced);
        assert!(!result.has_critic_approval);
        assert!(!result.has_integrator_approval);
    }

    #[test]
    fn test_evaluate_critical_action_with_roles_met() {
        let calc = QuorumCalculator::new();
        let votes = make_full_quorum_votes();
        let result = calc
            .evaluate("prop-1", ConsensusAction::DatabaseMigration, &votes)
            .unwrap_or_else(|_| unreachable!());
        assert!(result.met);
        assert_eq!(result.requirement, QuorumRequirement::Enhanced);
    }

    #[test]
    fn test_evaluate_low_participation_fails() {
        let config = QuorumConfig {
            min_participation_rate: 0.9,
            require_critic: false,
            require_integrator: false,
            weighted_threshold: 27.0,
        };
        let calc = QuorumCalculator::with_config(config);
        // 27 votes out of 40 => 67.5% < 90% required
        let votes = make_validator_approvals(27);
        let result = calc
            .evaluate("prop-1", ConsensusAction::CredentialRotation, &votes)
            .unwrap_or_else(|_| unreachable!());
        assert!(!result.met);
    }

    #[test]
    fn test_evaluate_empty_votes() {
        let calc = QuorumCalculator::new();
        let votes: Vec<ConsensusVote> = Vec::new();
        let result = calc
            .evaluate("prop-1", ConsensusAction::ConfigRollback, &votes)
            .unwrap_or_else(|_| unreachable!());
        assert!(!result.met);
        assert_eq!(result.total_votes, 0);
        assert!((result.participation_rate - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_evaluate_all_abstain() {
        let calc = QuorumCalculator::new();
        let votes: Vec<ConsensusVote> = (0..30)
            .map(|i| make_vote("prop-1", &format!("v-{i:03}"), VoteType::Abstain, AgentRole::Validator, 1.0))
            .collect();
        let result = calc
            .evaluate("prop-1", ConsensusAction::ConfigRollback, &votes)
            .unwrap_or_else(|_| unreachable!());
        assert!(!result.met);
        assert_eq!(result.votes_for, 0);
        assert_eq!(result.votes_abstain, 30);
    }

    #[test]
    fn test_evaluate_details_populated() {
        let calc = QuorumCalculator::new();
        let votes = make_full_quorum_votes();
        let result = calc
            .evaluate("prop-1", ConsensusAction::ConfigRollback, &votes)
            .unwrap_or_else(|_| unreachable!());
        assert!(!result.evaluation_details.is_empty());
        assert!(result.evaluation_details.contains("requirement="));
        assert!(result.evaluation_details.contains("result="));
    }

    #[test]
    fn test_evaluate_non_critical_without_roles_config() {
        let config = QuorumConfig {
            min_participation_rate: 0.5,
            require_critic: false,
            require_integrator: false,
            weighted_threshold: 27.0,
        };
        let calc = QuorumCalculator::with_config(config);
        // No critic or integrator, but config says they are not required
        let votes = make_validator_approvals(27);
        let result = calc
            .evaluate("prop-1", ConsensusAction::CredentialRotation, &votes)
            .unwrap_or_else(|_| unreachable!());
        assert!(result.met);
        assert_eq!(result.requirement, QuorumRequirement::Simple);
    }

    #[test]
    fn test_evaluate_exactly_at_participation_threshold() {
        let config = QuorumConfig {
            min_participation_rate: 0.675,
            require_critic: false,
            require_integrator: false,
            weighted_threshold: 27.0,
        };
        let calc = QuorumCalculator::with_config(config);
        // 27 votes / 40 = 0.675 (exact match)
        let votes = make_validator_approvals(27);
        let result = calc
            .evaluate("prop-1", ConsensusAction::CredentialRotation, &votes)
            .unwrap_or_else(|_| unreachable!());
        assert!(result.met);
    }

    // ========================================================================
    // History tracking tests (38-42)
    // ========================================================================

    #[test]
    fn test_history_recorded() {
        let calc = QuorumCalculator::new();
        let votes = make_full_quorum_votes();
        let _ = calc.evaluate("prop-1", ConsensusAction::ConfigRollback, &votes);
        let _ = calc.evaluate("prop-2", ConsensusAction::DatabaseMigration, &votes);

        let history = calc.quorum_history();
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].proposal_id, "prop-1");
        assert_eq!(history[1].proposal_id, "prop-2");
    }

    #[test]
    fn test_history_bounded() {
        let config = QuorumConfig {
            min_participation_rate: 0.0,
            require_critic: false,
            require_integrator: false,
            weighted_threshold: 0.0,
        };
        let calc = QuorumCalculator::with_config(config);
        let votes = make_validator_approvals(1);

        for i in 0..510 {
            let _ = calc.evaluate(&format!("prop-{i}"), ConsensusAction::ConfigRollback, &votes);
        }

        let history = calc.quorum_history();
        assert_eq!(history.len(), MAX_QUORUM_HISTORY);
        // Oldest entries should have been evicted; first entry should be prop-10
        assert_eq!(history[0].proposal_id, "prop-10");
    }

    #[test]
    fn test_history_clear() {
        let calc = QuorumCalculator::new();
        let votes = make_full_quorum_votes();
        let _ = calc.evaluate("prop-1", ConsensusAction::ConfigRollback, &votes);
        assert_eq!(calc.quorum_history().len(), 1);

        calc.clear_history();
        assert!(calc.quorum_history().is_empty());
    }

    #[test]
    fn test_history_action_type_recorded() {
        let calc = QuorumCalculator::new();
        let votes = make_full_quorum_votes();
        let _ = calc.evaluate("prop-1", ConsensusAction::ServiceTermination, &votes);

        let history = calc.quorum_history();
        assert_eq!(history[0].action_type, ConsensusAction::ServiceTermination);
    }

    #[test]
    fn test_history_timestamp_recorded() {
        let calc = QuorumCalculator::new();
        let before = SystemTime::now();
        let votes = make_full_quorum_votes();
        let _ = calc.evaluate("prop-1", ConsensusAction::ConfigRollback, &votes);
        let after = SystemTime::now();

        let history = calc.quorum_history();
        assert!(history[0].timestamp >= before);
        assert!(history[0].timestamp <= after);
    }

    // ========================================================================
    // Config change tests (43-46)
    // ========================================================================

    #[test]
    fn test_set_config() {
        let calc = QuorumCalculator::new();
        let new_config = QuorumConfig {
            min_participation_rate: 0.8,
            require_critic: false,
            require_integrator: false,
            weighted_threshold: 30.0,
        };
        let result = calc.set_config(new_config);
        assert!(result.is_ok());

        let config = calc.get_config().unwrap_or_else(|_| unreachable!());
        assert!((config.min_participation_rate - 0.8).abs() < f64::EPSILON);
        assert!(!config.require_critic);
        assert!(!config.require_integrator);
        assert!((config.weighted_threshold - 30.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_default_config() {
        let calc = QuorumCalculator::new();
        let config = calc.get_config().unwrap_or_else(|_| unreachable!());
        assert!((config.min_participation_rate - 0.5).abs() < f64::EPSILON);
        assert!(config.require_critic);
        assert!(config.require_integrator);
        assert!((config.weighted_threshold - 27.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_with_config_constructor() {
        let config = QuorumConfig {
            min_participation_rate: 0.3,
            require_critic: false,
            require_integrator: true,
            weighted_threshold: 20.0,
        };
        let calc = QuorumCalculator::with_config(config);
        let retrieved = calc.get_config().unwrap_or_else(|_| unreachable!());
        assert!((retrieved.min_participation_rate - 0.3).abs() < f64::EPSILON);
        assert!(!retrieved.require_critic);
        assert!(retrieved.require_integrator);
        assert!((retrieved.weighted_threshold - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_config_affects_evaluate() {
        let calc = QuorumCalculator::new();
        let votes = make_validator_approvals(27);

        // Default config requires critic+integrator -> should fail (no roles)
        let result1 = calc
            .evaluate("prop-1", ConsensusAction::CredentialRotation, &votes)
            .unwrap_or_else(|_| unreachable!());
        assert!(!result1.met);

        // Relax config
        let _ = calc.set_config(QuorumConfig {
            min_participation_rate: 0.5,
            require_critic: false,
            require_integrator: false,
            weighted_threshold: 27.0,
        });

        // Same votes now pass
        let result2 = calc
            .evaluate("prop-2", ConsensusAction::CredentialRotation, &votes)
            .unwrap_or_else(|_| unreachable!());
        assert!(result2.met);
    }

    // ========================================================================
    // Evaluation count and approval rate tests (47-50)
    // ========================================================================

    #[test]
    fn test_evaluation_count() {
        let calc = QuorumCalculator::new();
        assert_eq!(calc.evaluation_count(), 0);

        let votes = make_full_quorum_votes();
        let _ = calc.evaluate("prop-1", ConsensusAction::ConfigRollback, &votes);
        assert_eq!(calc.evaluation_count(), 1);

        let _ = calc.evaluate("prop-2", ConsensusAction::ConfigRollback, &votes);
        assert_eq!(calc.evaluation_count(), 2);
    }

    #[test]
    fn test_approval_rate_empty() {
        let calc = QuorumCalculator::new();
        assert!((calc.approval_rate() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_approval_rate_all_approved() {
        let calc = QuorumCalculator::new();
        let votes = make_full_quorum_votes();
        let _ = calc.evaluate("prop-1", ConsensusAction::ConfigRollback, &votes);
        let _ = calc.evaluate("prop-2", ConsensusAction::ConfigRollback, &votes);

        assert!((calc.approval_rate() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_approval_rate_mixed() {
        let calc = QuorumCalculator::new();
        let good_votes = make_full_quorum_votes();
        let bad_votes: Vec<ConsensusVote> = Vec::new();

        // 1 approved, 1 not approved
        let _ = calc.evaluate("prop-1", ConsensusAction::ConfigRollback, &good_votes);
        let _ = calc.evaluate("prop-2", ConsensusAction::ConfigRollback, &bad_votes);

        assert!((calc.approval_rate() - 0.5).abs() < f64::EPSILON);
    }

}
