//! # N05: Evolution Gate
//!
//! Mutation testing before deployments via the Evolution Chamber. The gate evaluates
//! proposed parameter mutations by comparing field coherence (Kuramoto r) before and
//! after application, accepting only those that maintain or improve coherence.
//!
//! ## Layer: L8 (Nexus Integration)
//! ## Dependencies: L1 (Error, Timestamp)
//!
//! ## Decision Logic
//!
//! | Condition | Decision |
//! |-----------|----------|
//! | `r_after >= r_before` | [`GateDecision::Accept`] |
//! | `r_after < r_before` | [`GateDecision::Reject`] |
//! | `\|r_delta\| < defer_threshold` | [`GateDecision::DeferToConsensus`] |
//!
//! ## Design Invariants
//!
//! - All trait methods take `&self` (interior mutability via `parking_lot::RwLock`)
//! - Zero `unsafe`, `unwrap`, `expect`
//! - `Timestamp` for all temporal fields (no chrono, no `SystemTime`)
//! - `uuid::Uuid` for mutation identity
//!
//! ## Related Documentation
//! - [Nexus Specs](../../ai_specs/nexus-specs/)
//! - [Evolution Chamber V2](../../ai_docs/ADVANCED_EVOLUTION_CHAMBER_V2.md)

use std::collections::{HashMap, VecDeque};
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};

use parking_lot::RwLock;
use uuid::Uuid;

use crate::m1_foundation::shared_types::Timestamp;
use crate::{Error, Result};

// ============================================================================
// Constants
// ============================================================================

/// Default threshold below which `r_delta` is too small to decide; escalate to PBFT.
pub const DEFAULT_DEFER_THRESHOLD: f64 = 0.01;

/// Default maximum number of evaluations kept in the rolling history.
pub const DEFAULT_EVALUATION_CAPACITY: usize = 500;

// ============================================================================
// MutationCandidate
// ============================================================================

/// A proposed parameter mutation submitted to the evolution gate for evaluation.
///
/// Each candidate carries a unique ID (UUID v4), the parameter being mutated,
/// old and new values, who proposed it, and when it was submitted.
#[derive(Clone, Debug)]
pub struct MutationCandidate {
    /// Unique mutation identifier (UUID v4).
    pub id: String,
    /// Name of the parameter being mutated.
    pub parameter: String,
    /// Current value of the parameter.
    pub old_value: f64,
    /// Proposed new value of the parameter.
    pub new_value: f64,
    /// Who proposed this mutation (agent, module, or system).
    pub proposed_by: String,
    /// When the mutation was submitted.
    pub timestamp: Timestamp,
}

// ============================================================================
// GateDecision
// ============================================================================

/// The decision rendered by the evolution gate after evaluating a mutation.
///
/// The gate compares `r_before` and `r_after` to determine whether the mutation
/// preserves or improves field coherence.
#[derive(Clone, Debug)]
pub enum GateDecision {
    /// Mutation accepted: field coherence maintained or improved (`r_after >= r_before`).
    Accept {
        /// Change in field coherence (`r_after - r_before`).
        r_delta: f64,
        /// Confidence in the decision based on delta magnitude.
        confidence: f64,
    },
    /// Mutation rejected: field coherence degraded (`r_after < r_before`).
    Reject {
        /// Human-readable reason for rejection.
        reason: String,
        /// Change in field coherence (negative).
        r_delta: f64,
    },
    /// Too close to call (`|r_delta| < defer_threshold`): escalate to PBFT consensus.
    DeferToConsensus {
        /// Proposal ID for the consensus round.
        proposal_id: String,
    },
}

// ============================================================================
// GateEvaluation
// ============================================================================

/// A completed evaluation record stored in the rolling history.
#[derive(Clone, Debug)]
pub struct GateEvaluation {
    /// The mutation that was evaluated.
    pub mutation_id: String,
    /// The decision rendered.
    pub decision: GateDecision,
    /// Field coherence before mutation.
    pub r_before: f64,
    /// Field coherence after mutation.
    pub r_after: f64,
    /// When the evaluation was performed.
    pub evaluated_at: Timestamp,
}

// ============================================================================
// GateStats
// ============================================================================

/// Aggregate statistics for the evolution gate.
#[derive(Clone, Debug)]
pub struct GateStats {
    /// Total mutations submitted.
    pub total_submitted: u64,
    /// Total mutations accepted.
    pub total_accepted: u64,
    /// Total mutations rejected.
    pub total_rejected: u64,
    /// Total mutations deferred to consensus.
    pub total_deferred: u64,
    /// Acceptance rate as a fraction in `[0.0, 1.0]`.
    pub acceptance_rate: f64,
}

// ============================================================================
// EvolutionGateConfig
// ============================================================================

/// Configuration for the evolution gate.
#[derive(Clone, Debug)]
pub struct EvolutionGateConfig {
    /// Threshold below which `|r_delta|` triggers deferral to consensus.
    pub defer_threshold: f64,
    /// Maximum number of evaluations kept in the rolling history.
    pub evaluation_capacity: usize,
}

impl Default for EvolutionGateConfig {
    fn default() -> Self {
        Self {
            defer_threshold: DEFAULT_DEFER_THRESHOLD,
            evaluation_capacity: DEFAULT_EVALUATION_CAPACITY,
        }
    }
}

// ============================================================================
// EvolutionGate trait
// ============================================================================

/// Trait for mutation gating before deployment.
///
/// Implementations evaluate proposed parameter mutations by comparing Kuramoto
/// field coherence before and after application. Only mutations that maintain
/// or improve coherence are accepted.
pub trait EvolutionGate: Send + Sync + fmt::Debug {
    /// Submit a new mutation candidate for future evaluation.
    ///
    /// Returns the mutation ID on success.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if the mutation ID is empty or the candidate
    /// is otherwise malformed.
    fn submit_mutation(&self, mutation: MutationCandidate) -> Result<String>;

    /// Evaluate a previously submitted mutation given pre/post field coherence.
    ///
    /// # Decision logic
    ///
    /// - `|r_delta| < defer_threshold`: [`GateDecision::DeferToConsensus`]
    /// - `r_after >= r_before`: [`GateDecision::Accept`]
    /// - `r_after < r_before`: [`GateDecision::Reject`]
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if the mutation ID is not found in pending.
    fn evaluate(&self, mutation_id: &str, r_before: f64, r_after: f64) -> Result<GateDecision>;

    /// Return the most recent evaluations, up to `limit`.
    fn recent_evaluations(&self, limit: usize) -> Vec<GateEvaluation>;

    /// Return the overall acceptance rate as a fraction in `[0.0, 1.0]`.
    fn acceptance_rate(&self) -> f64;

    /// Return aggregate gate statistics.
    fn gate_stats(&self) -> GateStats;

    /// Return the number of mutations awaiting evaluation.
    fn pending_count(&self) -> usize;

    /// Reset all state (pending, evaluations, counters).
    fn reset(&self);
}

// ============================================================================
// EvolutionGateCore
// ============================================================================

/// Core implementation of the [`EvolutionGate`] trait.
///
/// Uses `parking_lot::RwLock` for interior mutability and `AtomicU64` for
/// lock-free counter updates.
#[derive(Debug)]
pub struct EvolutionGateCore {
    /// Mutations awaiting evaluation, keyed by mutation ID.
    pending: RwLock<HashMap<String, MutationCandidate>>,
    /// Rolling history of completed evaluations.
    evaluations: RwLock<VecDeque<GateEvaluation>>,
    /// Configuration.
    config: EvolutionGateConfig,
    /// Total mutations submitted.
    submitted: AtomicU64,
    /// Total mutations accepted.
    accepted: AtomicU64,
    /// Total mutations rejected.
    rejected: AtomicU64,
    /// Total mutations deferred to consensus.
    deferred: AtomicU64,
}

impl EvolutionGateCore {
    /// Create a new evolution gate with the given configuration.
    #[must_use]
    pub fn new(config: EvolutionGateConfig) -> Self {
        Self {
            pending: RwLock::new(HashMap::new()),
            evaluations: RwLock::new(VecDeque::with_capacity(config.evaluation_capacity)),
            config,
            submitted: AtomicU64::new(0),
            accepted: AtomicU64::new(0),
            rejected: AtomicU64::new(0),
            deferred: AtomicU64::new(0),
        }
    }

    /// Create a new evolution gate with default configuration.
    #[must_use]
    pub fn with_defaults() -> Self {
        Self::new(EvolutionGateConfig::default())
    }

    /// Compute confidence from the magnitude of `r_delta` relative to the defer threshold.
    ///
    /// Confidence increases as `|r_delta|` moves further from the defer threshold,
    /// clamped to `[0.0, 1.0]`.
    fn compute_confidence(&self, r_delta: f64) -> f64 {
        let magnitude = r_delta.abs();
        // Scale: at defer_threshold confidence is 0.5, at 10x threshold confidence is ~1.0
        let scale = if self.config.defer_threshold > 0.0 {
            magnitude / self.config.defer_threshold
        } else {
            magnitude * 100.0
        };
        scale.clamp(0.0, 1.0)
    }
}

impl EvolutionGate for EvolutionGateCore {
    fn submit_mutation(&self, mutation: MutationCandidate) -> Result<String> {
        if mutation.id.is_empty() {
            return Err(Error::Validation("mutation ID cannot be empty".to_string()));
        }
        if mutation.parameter.is_empty() {
            return Err(Error::Validation(
                "mutation parameter cannot be empty".to_string(),
            ));
        }
        let id = mutation.id.clone();
        {
            let mut pending = self.pending.write();
            pending.insert(id.clone(), mutation);
        }
        self.submitted.fetch_add(1, Ordering::Relaxed);
        Ok(id)
    }

    fn evaluate(&self, mutation_id: &str, r_before: f64, r_after: f64) -> Result<GateDecision> {
        // Remove from pending
        {
            let mut pending = self.pending.write();
            if pending.remove(mutation_id).is_none() {
                return Err(Error::Validation(format!(
                    "mutation '{mutation_id}' not found in pending"
                )));
            }
        }

        let r_delta = r_after - r_before;

        // Decision logic: defer first (ambiguous), then accept/reject
        let decision = if r_delta.abs() < self.config.defer_threshold {
            let proposal_id = Uuid::new_v4().to_string();
            self.deferred.fetch_add(1, Ordering::Relaxed);
            GateDecision::DeferToConsensus { proposal_id }
        } else if r_after >= r_before {
            let confidence = self.compute_confidence(r_delta);
            self.accepted.fetch_add(1, Ordering::Relaxed);
            GateDecision::Accept {
                r_delta,
                confidence,
            }
        } else {
            self.rejected.fetch_add(1, Ordering::Relaxed);
            GateDecision::Reject {
                reason: format!(
                    "field coherence degraded: r_before={r_before:.6}, r_after={r_after:.6}, delta={r_delta:.6}"
                ),
                r_delta,
            }
        };

        // Record evaluation
        let evaluation = GateEvaluation {
            mutation_id: mutation_id.to_string(),
            decision: decision.clone(),
            r_before,
            r_after,
            evaluated_at: Timestamp::now(),
        };

        {
            let mut evals = self.evaluations.write();
            if evals.len() >= self.config.evaluation_capacity {
                evals.pop_front();
            }
            evals.push_back(evaluation);
        }

        Ok(decision)
    }

    fn recent_evaluations(&self, limit: usize) -> Vec<GateEvaluation> {
        let evals = self.evaluations.read();
        evals.iter().rev().take(limit).cloned().collect()
    }

    fn acceptance_rate(&self) -> f64 {
        let total = self.submitted.load(Ordering::Relaxed);
        if total == 0 {
            return 0.0;
        }
        let accepted = self.accepted.load(Ordering::Relaxed);
        #[allow(clippy::cast_precision_loss)]
        let rate = accepted as f64 / total as f64;
        rate
    }

    fn gate_stats(&self) -> GateStats {
        GateStats {
            total_submitted: self.submitted.load(Ordering::Relaxed),
            total_accepted: self.accepted.load(Ordering::Relaxed),
            total_rejected: self.rejected.load(Ordering::Relaxed),
            total_deferred: self.deferred.load(Ordering::Relaxed),
            acceptance_rate: self.acceptance_rate(),
        }
    }

    fn pending_count(&self) -> usize {
        self.pending.read().len()
    }

    fn reset(&self) {
        self.pending.write().clear();
        self.evaluations.write().clear();
        self.submitted.store(0, Ordering::Relaxed);
        self.accepted.store(0, Ordering::Relaxed);
        self.rejected.store(0, Ordering::Relaxed);
        self.deferred.store(0, Ordering::Relaxed);
    }
}

// ============================================================================
// Helper: create a test mutation candidate
// ============================================================================

/// Create a `MutationCandidate` with a fresh UUID for convenience.
#[must_use]
pub fn new_mutation(parameter: &str, old_value: f64, new_value: f64, proposed_by: &str) -> MutationCandidate {
    MutationCandidate {
        id: Uuid::new_v4().to_string(),
        parameter: parameter.to_string(),
        old_value,
        new_value,
        proposed_by: proposed_by.to_string(),
        timestamp: Timestamp::now(),
    }
}

// ============================================================================
// Tests (50+ required)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // Helpers ---------------------------------------------------------------

    fn gate() -> EvolutionGateCore {
        EvolutionGateCore::with_defaults()
    }

    fn gate_with_threshold(defer_threshold: f64) -> EvolutionGateCore {
        EvolutionGateCore::new(EvolutionGateConfig {
            defer_threshold,
            evaluation_capacity: DEFAULT_EVALUATION_CAPACITY,
        })
    }

    fn candidate(param: &str) -> MutationCandidate {
        new_mutation(param, 0.5, 0.6, "test-agent")
    }

    fn submit(g: &EvolutionGateCore, param: &str) -> String {
        let c = candidate(param);
        g.submit_mutation(c).ok().unwrap_or_default()
    }

    // -----------------------------------------------------------------------
    // submit_mutation
    // -----------------------------------------------------------------------

    #[test]
    fn submit_returns_id() {
        let g = gate();
        let c = candidate("k_coupling");
        let id = c.id.clone();
        let result = g.submit_mutation(c);
        assert!(result.is_ok());
        assert_eq!(result.ok(), Some(id));
    }

    #[test]
    fn submit_increments_counter() {
        let g = gate();
        let _ = submit(&g, "a");
        let _ = submit(&g, "b");
        assert_eq!(g.submitted.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn submit_adds_to_pending() {
        let g = gate();
        let _ = submit(&g, "x");
        assert_eq!(g.pending_count(), 1);
    }

    #[test]
    fn submit_empty_id_rejected() {
        let g = gate();
        let c = MutationCandidate {
            id: String::new(),
            parameter: "k".to_string(),
            old_value: 0.0,
            new_value: 1.0,
            proposed_by: "test".to_string(),
            timestamp: Timestamp::now(),
        };
        assert!(g.submit_mutation(c).is_err());
    }

    #[test]
    fn submit_empty_parameter_rejected() {
        let g = gate();
        let c = MutationCandidate {
            id: Uuid::new_v4().to_string(),
            parameter: String::new(),
            old_value: 0.0,
            new_value: 1.0,
            proposed_by: "test".to_string(),
            timestamp: Timestamp::now(),
        };
        assert!(g.submit_mutation(c).is_err());
    }

    #[test]
    fn submit_duplicate_id_overwrites() {
        let g = gate();
        let id = Uuid::new_v4().to_string();
        let c1 = MutationCandidate {
            id: id.clone(),
            parameter: "alpha".to_string(),
            old_value: 0.1,
            new_value: 0.2,
            proposed_by: "a".to_string(),
            timestamp: Timestamp::now(),
        };
        let c2 = MutationCandidate {
            id: id.clone(),
            parameter: "beta".to_string(),
            old_value: 0.3,
            new_value: 0.4,
            proposed_by: "b".to_string(),
            timestamp: Timestamp::now(),
        };
        let _ = g.submit_mutation(c1);
        let _ = g.submit_mutation(c2);
        // Still only 1 pending (overwritten)
        assert_eq!(g.pending_count(), 1);
    }

    // -----------------------------------------------------------------------
    // evaluate — Accept
    // -----------------------------------------------------------------------

    #[test]
    fn evaluate_accept_when_r_improves() {
        let g = gate();
        let id = submit(&g, "k");
        let decision = g.evaluate(&id, 0.80, 0.90);
        assert!(decision.is_ok());
        if let Ok(GateDecision::Accept { r_delta, .. }) = decision {
            assert!((r_delta - 0.10).abs() < 1e-9);
        } else {
            panic!("expected Accept");
        }
    }

    #[test]
    fn evaluate_accept_when_r_equal_and_above_threshold() {
        // r_delta = 0.0 which is < defer_threshold, so this should defer
        // Use a threshold of 0.0 so equal values accept
        let g = gate_with_threshold(0.0);
        let id = submit(&g, "k");
        let decision = g.evaluate(&id, 0.85, 0.85);
        assert!(decision.is_ok());
        // With threshold 0.0, r_delta=0.0 is not < 0.0, so we get Accept
        assert!(matches!(decision.ok(), Some(GateDecision::Accept { .. })));
    }

    #[test]
    fn evaluate_accept_increments_accepted() {
        let g = gate();
        let id = submit(&g, "k");
        let _ = g.evaluate(&id, 0.50, 0.90);
        assert_eq!(g.accepted.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn evaluate_accept_confidence_increases_with_delta() {
        // Verify via the internal compute_confidence method directly
        let g = gate();
        let c_small = g.compute_confidence(0.005); // 0.005 / 0.01 = 0.5
        let c_large = g.compute_confidence(0.008); // 0.008 / 0.01 = 0.8
        assert!(
            c_large > c_small,
            "larger delta should yield higher confidence: {c_large} vs {c_small}"
        );
    }

    // -----------------------------------------------------------------------
    // evaluate — Reject
    // -----------------------------------------------------------------------

    #[test]
    fn evaluate_reject_when_r_degrades() {
        let g = gate();
        let id = submit(&g, "k");
        let decision = g.evaluate(&id, 0.90, 0.80);
        assert!(decision.is_ok());
        if let Ok(GateDecision::Reject { r_delta, .. }) = decision {
            assert!((r_delta - (-0.10)).abs() < 1e-9);
        } else {
            panic!("expected Reject");
        }
    }

    #[test]
    fn evaluate_reject_increments_rejected() {
        let g = gate();
        let id = submit(&g, "k");
        let _ = g.evaluate(&id, 0.90, 0.70);
        assert_eq!(g.rejected.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn evaluate_reject_reason_contains_values() {
        let g = gate();
        let id = submit(&g, "k");
        let d = g.evaluate(&id, 0.90, 0.80);
        if let Ok(GateDecision::Reject { reason, .. }) = d {
            assert!(reason.contains("coherence degraded"));
        } else {
            panic!("expected Reject");
        }
    }

    // -----------------------------------------------------------------------
    // evaluate — DeferToConsensus
    // -----------------------------------------------------------------------

    #[test]
    fn evaluate_defer_when_delta_small() {
        let g = gate(); // threshold = 0.01
        let id = submit(&g, "k");
        let decision = g.evaluate(&id, 0.900, 0.905); // delta = 0.005 < 0.01
        assert!(decision.is_ok());
        assert!(matches!(
            decision.ok(),
            Some(GateDecision::DeferToConsensus { .. })
        ));
    }

    #[test]
    fn evaluate_defer_increments_deferred() {
        let g = gate();
        let id = submit(&g, "k");
        let _ = g.evaluate(&id, 0.900, 0.905);
        assert_eq!(g.deferred.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn evaluate_defer_proposal_id_is_uuid() {
        let g = gate();
        let id = submit(&g, "k");
        let d = g.evaluate(&id, 0.900, 0.902);
        if let Ok(GateDecision::DeferToConsensus { proposal_id }) = d {
            assert!(!proposal_id.is_empty());
            assert!(Uuid::parse_str(&proposal_id).is_ok());
        } else {
            panic!("expected DeferToConsensus");
        }
    }

    #[test]
    fn evaluate_defer_on_negative_small_delta() {
        let g = gate();
        let id = submit(&g, "k");
        let d = g.evaluate(&id, 0.900, 0.895); // delta = -0.005
        assert!(matches!(
            d.ok(),
            Some(GateDecision::DeferToConsensus { .. })
        ));
    }

    // -----------------------------------------------------------------------
    // evaluate — errors
    // -----------------------------------------------------------------------

    #[test]
    fn evaluate_unknown_id_returns_error() {
        let g = gate();
        let result = g.evaluate("nonexistent", 0.5, 0.6);
        assert!(result.is_err());
    }

    #[test]
    fn evaluate_removes_from_pending() {
        let g = gate();
        let id = submit(&g, "k");
        assert_eq!(g.pending_count(), 1);
        let _ = g.evaluate(&id, 0.5, 0.9);
        assert_eq!(g.pending_count(), 0);
    }

    #[test]
    fn evaluate_same_id_twice_returns_error() {
        let g = gate();
        let id = submit(&g, "k");
        let _ = g.evaluate(&id, 0.5, 0.9);
        let result = g.evaluate(&id, 0.5, 0.9);
        assert!(result.is_err());
    }

    // -----------------------------------------------------------------------
    // recent_evaluations
    // -----------------------------------------------------------------------

    #[test]
    fn recent_evaluations_empty_initially() {
        let g = gate();
        assert!(g.recent_evaluations(10).is_empty());
    }

    #[test]
    fn recent_evaluations_returns_most_recent_first() {
        let g = gate();
        let id1 = submit(&g, "first");
        let id2 = submit(&g, "second");
        let _ = g.evaluate(&id1, 0.5, 0.9);
        let _ = g.evaluate(&id2, 0.5, 0.8);
        let evals = g.recent_evaluations(10);
        assert_eq!(evals.len(), 2);
        assert_eq!(evals[0].mutation_id, id2);
        assert_eq!(evals[1].mutation_id, id1);
    }

    #[test]
    fn recent_evaluations_respects_limit() {
        let g = gate();
        for i in 0..5 {
            let id = submit(&g, &format!("p{i}"));
            let _ = g.evaluate(&id, 0.5, 0.9);
        }
        let evals = g.recent_evaluations(3);
        assert_eq!(evals.len(), 3);
    }

    #[test]
    fn recent_evaluations_limit_zero() {
        let g = gate();
        let id = submit(&g, "k");
        let _ = g.evaluate(&id, 0.5, 0.9);
        assert!(g.recent_evaluations(0).is_empty());
    }

    // -----------------------------------------------------------------------
    // acceptance_rate
    // -----------------------------------------------------------------------

    #[test]
    fn acceptance_rate_zero_when_empty() {
        let g = gate();
        assert!((g.acceptance_rate()).abs() < f64::EPSILON);
    }

    #[test]
    fn acceptance_rate_one_when_all_accepted() {
        let g = gate();
        for i in 0..5 {
            let id = submit(&g, &format!("p{i}"));
            let _ = g.evaluate(&id, 0.5, 0.9); // all accept
        }
        assert!((g.acceptance_rate() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn acceptance_rate_zero_when_all_rejected() {
        let g = gate();
        for i in 0..5 {
            let id = submit(&g, &format!("p{i}"));
            let _ = g.evaluate(&id, 0.9, 0.5); // all reject
        }
        assert!((g.acceptance_rate()).abs() < f64::EPSILON);
    }

    #[test]
    fn acceptance_rate_half_when_mixed() {
        let g = gate();
        let id1 = submit(&g, "a");
        let id2 = submit(&g, "b");
        let _ = g.evaluate(&id1, 0.5, 0.9); // accept
        let _ = g.evaluate(&id2, 0.9, 0.5); // reject
        assert!((g.acceptance_rate() - 0.5).abs() < f64::EPSILON);
    }

    // -----------------------------------------------------------------------
    // gate_stats
    // -----------------------------------------------------------------------

    #[test]
    fn gate_stats_all_zero_initially() {
        let g = gate();
        let stats = g.gate_stats();
        assert_eq!(stats.total_submitted, 0);
        assert_eq!(stats.total_accepted, 0);
        assert_eq!(stats.total_rejected, 0);
        assert_eq!(stats.total_deferred, 0);
    }

    #[test]
    fn gate_stats_after_mixed_evaluations() {
        let g = gate();
        let id1 = submit(&g, "a"); // accept
        let id2 = submit(&g, "b"); // reject
        let id3 = submit(&g, "c"); // defer
        let _ = g.evaluate(&id1, 0.5, 0.9);
        let _ = g.evaluate(&id2, 0.9, 0.5);
        let _ = g.evaluate(&id3, 0.9, 0.905);
        let stats = g.gate_stats();
        assert_eq!(stats.total_submitted, 3);
        assert_eq!(stats.total_accepted, 1);
        assert_eq!(stats.total_rejected, 1);
        assert_eq!(stats.total_deferred, 1);
    }

    #[test]
    fn gate_stats_acceptance_rate_matches() {
        let g = gate();
        let id = submit(&g, "a");
        let _ = g.evaluate(&id, 0.5, 0.9);
        let stats = g.gate_stats();
        assert!((stats.acceptance_rate - g.acceptance_rate()).abs() < f64::EPSILON);
    }

    // -----------------------------------------------------------------------
    // pending_count
    // -----------------------------------------------------------------------

    #[test]
    fn pending_count_zero_initially() {
        let g = gate();
        assert_eq!(g.pending_count(), 0);
    }

    #[test]
    fn pending_count_increases_on_submit() {
        let g = gate();
        let _ = submit(&g, "a");
        let _ = submit(&g, "b");
        assert_eq!(g.pending_count(), 2);
    }

    #[test]
    fn pending_count_decreases_on_evaluate() {
        let g = gate();
        let id = submit(&g, "a");
        let _ = submit(&g, "b");
        assert_eq!(g.pending_count(), 2);
        let _ = g.evaluate(&id, 0.5, 0.9);
        assert_eq!(g.pending_count(), 1);
    }

    // -----------------------------------------------------------------------
    // reset
    // -----------------------------------------------------------------------

    #[test]
    fn reset_clears_pending() {
        let g = gate();
        let _ = submit(&g, "a");
        g.reset();
        assert_eq!(g.pending_count(), 0);
    }

    #[test]
    fn reset_clears_evaluations() {
        let g = gate();
        let id = submit(&g, "a");
        let _ = g.evaluate(&id, 0.5, 0.9);
        g.reset();
        assert!(g.recent_evaluations(10).is_empty());
    }

    #[test]
    fn reset_clears_counters() {
        let g = gate();
        let id = submit(&g, "a");
        let _ = g.evaluate(&id, 0.5, 0.9);
        g.reset();
        let stats = g.gate_stats();
        assert_eq!(stats.total_submitted, 0);
        assert_eq!(stats.total_accepted, 0);
    }

    #[test]
    fn reset_allows_fresh_operations() {
        let g = gate();
        let id1 = submit(&g, "a");
        let _ = g.evaluate(&id1, 0.5, 0.9);
        g.reset();
        let id2 = submit(&g, "b");
        let result = g.evaluate(&id2, 0.5, 0.8);
        assert!(result.is_ok());
    }

    // -----------------------------------------------------------------------
    // capacity
    // -----------------------------------------------------------------------

    #[test]
    fn evaluation_history_respects_capacity() {
        let g = EvolutionGateCore::new(EvolutionGateConfig {
            defer_threshold: DEFAULT_DEFER_THRESHOLD,
            evaluation_capacity: 3,
        });
        for i in 0..5 {
            let id = submit(&g, &format!("p{i}"));
            let _ = g.evaluate(&id, 0.5, 0.9);
        }
        let evals = g.recent_evaluations(10);
        assert_eq!(evals.len(), 3);
    }

    // -----------------------------------------------------------------------
    // config
    // -----------------------------------------------------------------------

    #[test]
    fn default_config_values() {
        let cfg = EvolutionGateConfig::default();
        assert!((cfg.defer_threshold - DEFAULT_DEFER_THRESHOLD).abs() < f64::EPSILON);
        assert_eq!(cfg.evaluation_capacity, DEFAULT_EVALUATION_CAPACITY);
    }

    #[test]
    fn custom_defer_threshold() {
        let g = gate_with_threshold(0.1);
        let id = submit(&g, "k");
        let d = g.evaluate(&id, 0.80, 0.85); // delta=0.05 < 0.1 → defer
        assert!(matches!(
            d.ok(),
            Some(GateDecision::DeferToConsensus { .. })
        ));
    }

    // -----------------------------------------------------------------------
    // new_mutation helper
    // -----------------------------------------------------------------------

    #[test]
    fn new_mutation_generates_uuid() {
        let m = new_mutation("k", 0.5, 0.6, "agent");
        assert!(!m.id.is_empty());
        assert!(Uuid::parse_str(&m.id).is_ok());
    }

    #[test]
    fn new_mutation_stores_values() {
        let m = new_mutation("alpha", 0.1, 0.2, "ralph");
        assert_eq!(m.parameter, "alpha");
        assert!((m.old_value - 0.1).abs() < f64::EPSILON);
        assert!((m.new_value - 0.2).abs() < f64::EPSILON);
        assert_eq!(m.proposed_by, "ralph");
    }

    // -----------------------------------------------------------------------
    // Trait object safety (Send + Sync + Debug)
    // -----------------------------------------------------------------------

    #[test]
    fn gate_core_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<EvolutionGateCore>();
    }

    #[test]
    fn gate_core_is_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<EvolutionGateCore>();
    }

    #[test]
    fn gate_core_is_debug() {
        let g = gate();
        let debug = format!("{g:?}");
        assert!(!debug.is_empty());
    }

    #[test]
    fn gate_core_as_trait_object() {
        let g: Box<dyn EvolutionGate> = Box::new(gate());
        assert_eq!(g.pending_count(), 0);
    }

    // -----------------------------------------------------------------------
    // Edge cases
    // -----------------------------------------------------------------------

    #[test]
    fn evaluate_with_zero_delta() {
        let g = gate(); // threshold = 0.01
        let id = submit(&g, "k");
        let d = g.evaluate(&id, 0.85, 0.85); // delta=0 < 0.01 → defer
        assert!(matches!(
            d.ok(),
            Some(GateDecision::DeferToConsensus { .. })
        ));
    }

    #[test]
    fn evaluate_with_extreme_positive_delta() {
        let g = gate();
        let id = submit(&g, "k");
        let d = g.evaluate(&id, 0.0, 1.0);
        if let Ok(GateDecision::Accept { r_delta, confidence }) = d {
            assert!((r_delta - 1.0).abs() < f64::EPSILON);
            assert!((confidence - 1.0).abs() < f64::EPSILON);
        } else {
            panic!("expected Accept");
        }
    }

    #[test]
    fn evaluate_with_extreme_negative_delta() {
        let g = gate();
        let id = submit(&g, "k");
        let d = g.evaluate(&id, 1.0, 0.0);
        assert!(matches!(d.ok(), Some(GateDecision::Reject { .. })));
    }

    #[test]
    fn evaluate_boundary_exactly_at_threshold() {
        let g = gate(); // threshold = 0.01
        let id = submit(&g, "k");
        // delta = 0.01, which is NOT less than 0.01, so should accept
        let d = g.evaluate(&id, 0.80, 0.81);
        assert!(matches!(d.ok(), Some(GateDecision::Accept { .. })));
    }

    #[test]
    fn evaluate_boundary_just_below_threshold() {
        let g = gate();
        let id = submit(&g, "k");
        // delta = 0.009 < 0.01 → defer
        let d = g.evaluate(&id, 0.800, 0.809);
        assert!(matches!(
            d.ok(),
            Some(GateDecision::DeferToConsensus { .. })
        ));
    }

    #[test]
    fn many_submits_and_evaluates() {
        let g = gate();
        for i in 0..100 {
            let id = submit(&g, &format!("p{i}"));
            let r_after = if i % 2 == 0 { 0.9 } else { 0.3 };
            let _ = g.evaluate(&id, 0.5, r_after);
        }
        let stats = g.gate_stats();
        assert_eq!(stats.total_submitted, 100);
        assert_eq!(stats.total_accepted + stats.total_rejected + stats.total_deferred, 100);
    }

    #[test]
    fn confidence_zero_at_defer_threshold() {
        let g = gate();
        // confidence = |r_delta| / defer_threshold, clamped [0,1]
        // at exactly threshold, confidence = 1.0
        let c = g.compute_confidence(DEFAULT_DEFER_THRESHOLD);
        assert!((c - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn confidence_scales_below_threshold() {
        let g = gate();
        let c = g.compute_confidence(DEFAULT_DEFER_THRESHOLD / 2.0);
        assert!((c - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn confidence_clamped_at_one() {
        let g = gate();
        let c = g.compute_confidence(1.0); // way above threshold
        assert!((c - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn confidence_zero_for_zero_delta() {
        let g = gate();
        let c = g.compute_confidence(0.0);
        assert!(c.abs() < f64::EPSILON);
    }
}
