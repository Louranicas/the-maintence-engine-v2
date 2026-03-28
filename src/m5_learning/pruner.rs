//! # M28: Pathway Pruner
//!
//! Removes weak, ineffective, or stale Hebbian pathways from the learning layer.
//!
//! The pruner evaluates each pathway against configurable thresholds for strength,
//! activity, success rate, and recency. Pathways that fall below these thresholds
//! are identified as pruning candidates and can be removed in bulk. Three built-in
//! policies (Conservative, Moderate, Aggressive) provide convenient preset
//! configurations, and custom thresholds can be supplied via [`PruningConfig`].
//!
//! ## Layer: L5 (Learning)
//! ## Dependencies: M01 (Error), L5 types (`HebbianPathway`, `PathwayType`)
//! ## Tests: 50
//!
//! ## 12D Tensor Encoding
//! ```text
//! [28/36, 0.0, 5/6, 1, 0, 0.5, health, uptime, synergy, latency, error_rate, temporal]
//! ```
//!
//! ## Health Score Formula
//!
//! ```text
//! composite = 0.30 * strength_score
//!           + 0.25 * activity_score
//!           + 0.25 * success_score
//!           + 0.20 * recency_score
//! ```
//!
//! ## Related Documentation
//! - [Layer Specification](../../ai_docs/layers/L05_LEARNING.md)
//! - [Hebbian Integration](../../nam/HEBBIAN_INTEGRATION.md)

use std::collections::HashMap;
use std::time::SystemTime;

use parking_lot::RwLock;

use super::HebbianPathway;
use crate::{Error, Result};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Maximum number of pruning reports retained in history.
const PRUNING_HISTORY_CAPACITY: usize = 100;

/// Maximum number of pathways the pruner can manage.
const MAX_PATHWAYS: usize = 10_000;

/// Number of seconds in one day.
const SECONDS_PER_DAY: u64 = 86_400;

/// Recency decay half-life in days.
///
/// A pathway last activated this many days ago receives a recency score of 0.5.
const RECENCY_HALF_LIFE_DAYS: f64 = 7.0;

// ---------------------------------------------------------------------------
// PruningPolicy
// ---------------------------------------------------------------------------

/// Pre-defined pruning aggressiveness levels.
///
/// Each policy maps to a set of [`PruningConfig`] thresholds tuned for the
/// desired level of pathway retention.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PruningPolicy {
    /// Retain most pathways; only prune clearly dead pathways.
    Conservative,
    /// Balanced pruning suitable for steady-state operation.
    Moderate,
    /// Aggressively prune to keep only the strongest pathways.
    Aggressive,
}

impl PruningPolicy {
    /// Convert this policy into its default [`PruningConfig`].
    #[must_use]
    pub const fn to_config(self) -> PruningConfig {
        match self {
            Self::Conservative => PruningConfig {
                min_strength: 0.1,
                inactive_days: 30,
                min_activations: 3,
                min_success_rate: 0.1,
                max_age_days: 180,
                age_strength_threshold: 0.3,
            },
            Self::Moderate => PruningConfig {
                min_strength: 0.15,
                inactive_days: 14,
                min_activations: 5,
                min_success_rate: 0.2,
                max_age_days: 90,
                age_strength_threshold: 0.5,
            },
            Self::Aggressive => PruningConfig {
                min_strength: 0.3,
                inactive_days: 7,
                min_activations: 10,
                min_success_rate: 0.3,
                max_age_days: 30,
                age_strength_threshold: 0.7,
            },
        }
    }
}

// ---------------------------------------------------------------------------
// PruningConfig
// ---------------------------------------------------------------------------

/// Configuration thresholds for pathway pruning.
///
/// Pathways that fall below any of these thresholds will be flagged as
/// pruning candidates with the corresponding [`PruneReason`].
#[derive(Clone, Copy, Debug)]
pub struct PruningConfig {
    /// Minimum pathway strength to avoid `WeakStrength` pruning.
    pub min_strength: f64,
    /// Days of inactivity before `Stale` pruning applies.
    pub inactive_days: u64,
    /// Minimum activation count to avoid `LowActivity` pruning.
    pub min_activations: u64,
    /// Minimum success rate to avoid `LowSuccessRate` pruning.
    pub min_success_rate: f64,
    /// Maximum pathway age (days of inactivity) for `AgedOut` pruning.
    /// Pathways inactive longer than this AND below `age_strength_threshold`
    /// are pruned regardless of other metrics.
    pub max_age_days: u64,
    /// Strength threshold for age-based pruning. Pathways older than
    /// `max_age_days` with strength below this value are pruned as `AgedOut`.
    pub age_strength_threshold: f64,
}

impl Default for PruningConfig {
    fn default() -> Self {
        PruningPolicy::Moderate.to_config()
    }
}

// ---------------------------------------------------------------------------
// PruneReason
// ---------------------------------------------------------------------------

/// Reason a pathway was flagged for pruning.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PruneReason {
    /// Pathway strength is below the configured minimum.
    WeakStrength,
    /// Pathway has too few activations.
    LowActivity,
    /// Pathway success rate is below the configured minimum.
    LowSuccessRate,
    /// Pathway has not been activated within the configured window.
    Stale,
    /// Pathway was manually selected for pruning.
    ManualPrune,
    /// Pathway exceeded maximum age with insufficient strength.
    /// Triggered when inactive > `max_age_days` AND strength < `age_strength_threshold`.
    AgedOut,
}

impl std::fmt::Display for PruneReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::WeakStrength => write!(f, "WeakStrength"),
            Self::LowActivity => write!(f, "LowActivity"),
            Self::LowSuccessRate => write!(f, "LowSuccessRate"),
            Self::Stale => write!(f, "Stale"),
            Self::ManualPrune => write!(f, "ManualPrune"),
            Self::AgedOut => write!(f, "AgedOut"),
        }
    }
}

// ---------------------------------------------------------------------------
// PruningCandidate
// ---------------------------------------------------------------------------

/// A pathway identified for potential pruning.
///
/// Contains the reason for flagging, a computed health score, and a snapshot
/// of the pathway at the time of evaluation.
#[derive(Clone, Debug)]
pub struct PruningCandidate {
    /// Pathway identifier.
    pub pathway_id: String,
    /// Reason the pathway was flagged.
    pub reason: PruneReason,
    /// Composite health score at the time of evaluation.
    pub health_score: f64,
    /// Snapshot of the pathway state.
    pub pathway: HebbianPathway,
}

// ---------------------------------------------------------------------------
// PruningReport
// ---------------------------------------------------------------------------

/// Summary report produced after a pruning cycle.
#[derive(Clone, Debug)]
pub struct PruningReport {
    /// Total pathways evaluated.
    pub total_evaluated: usize,
    /// Number of pathways pruned.
    pub pruned_count: usize,
    /// Number of pathways retained.
    pub retained_count: usize,
    /// Candidates that were pruned.
    pub candidates: Vec<PruningCandidate>,
    /// Timestamp when the pruning cycle ran.
    pub timestamp: SystemTime,
}

// ---------------------------------------------------------------------------
// PathwayHealthScore
// ---------------------------------------------------------------------------

/// Decomposed health score for a single pathway.
///
/// Each sub-score is in [0.0, 1.0] and the composite is a weighted average:
/// `0.30 * strength + 0.25 * activity + 0.25 * success + 0.20 * recency`.
#[derive(Clone, Debug)]
pub struct PathwayHealthScore {
    /// Pathway identifier.
    pub pathway_id: String,
    /// Score derived from pathway strength.
    pub strength_score: f64,
    /// Score derived from activation count (saturates at 100).
    pub activity_score: f64,
    /// Score derived from success rate.
    pub success_score: f64,
    /// Score derived from recency of last activation.
    pub recency_score: f64,
    /// Weighted composite score.
    pub composite: f64,
}

// ---------------------------------------------------------------------------
// PathwayPruner
// ---------------------------------------------------------------------------

/// Thread-safe pathway pruner for removing weak Hebbian pathways.
///
/// Maintains a set of pathways, evaluates their health, identifies candidates
/// for pruning, and removes them. All mutable state is guarded by
/// `parking_lot::RwLock` for concurrent access.
///
/// # Example
///
/// ```rust
/// use maintenance_engine::m5_learning::pruner::PathwayPruner;
///
/// let pruner = PathwayPruner::new();
/// assert_eq!(pruner.pathway_count(), 0);
/// ```
pub struct PathwayPruner {
    /// Pathways keyed by their ID.
    pathways: RwLock<HashMap<String, HebbianPathway>>,
    /// Bounded history of pruning reports.
    pruning_history: RwLock<Vec<PruningReport>>,
    /// Active pruning configuration.
    config: RwLock<PruningConfig>,
}

impl PathwayPruner {
    // -------------------------------------------------------------------
    // Construction
    // -------------------------------------------------------------------

    /// Create a new `PathwayPruner` with the default (Moderate) configuration.
    #[must_use]
    pub fn new() -> Self {
        Self {
            pathways: RwLock::new(HashMap::new()),
            pruning_history: RwLock::new(Vec::new()),
            config: RwLock::new(PruningConfig::default()),
        }
    }

    /// Create a new `PathwayPruner` with the given configuration.
    #[must_use]
    pub fn with_config(config: PruningConfig) -> Self {
        Self {
            pathways: RwLock::new(HashMap::new()),
            pruning_history: RwLock::new(Vec::new()),
            config: RwLock::new(config),
        }
    }

    // -------------------------------------------------------------------
    // Pathway CRUD
    // -------------------------------------------------------------------

    /// Add a pathway to the pruner.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] if the pathway ID is empty or if the
    /// maximum pathway capacity has been reached.
    pub fn add_pathway(&self, pathway: HebbianPathway) -> Result<()> {
        if pathway.id.is_empty() {
            return Err(Error::Validation(
                "Pathway ID must not be empty".to_string(),
            ));
        }

        let mut guard = self.pathways.write();
        if guard.len() >= MAX_PATHWAYS {
            return Err(Error::Validation(format!(
                "Maximum pathway capacity ({MAX_PATHWAYS}) reached"
            )));
        }
        guard.insert(pathway.id.clone(), pathway);
        drop(guard);

        Ok(())
    }

    /// Remove a pathway by its ID.
    ///
    /// # Errors
    ///
    /// Returns [`Error::PathwayNotFound`] if the ID does not exist.
    pub fn remove_pathway(&self, id: &str) -> Result<()> {
        let mut guard = self.pathways.write();
        if guard.remove(id).is_none() {
            return Err(Error::PathwayNotFound {
                source: id.to_string(),
                target: String::new(),
            });
        }
        drop(guard);
        Ok(())
    }

    /// Retrieve a clone of a pathway by its ID.
    #[must_use]
    pub fn get_pathway(&self, id: &str) -> Option<HebbianPathway> {
        self.pathways.read().get(id).cloned()
    }

    /// Get the number of pathways currently managed.
    #[must_use]
    pub fn pathway_count(&self) -> usize {
        self.pathways.read().len()
    }

    /// Get the IDs of all managed pathways.
    #[must_use]
    pub fn pathway_ids(&self) -> Vec<String> {
        self.pathways.read().keys().cloned().collect()
    }

    // -------------------------------------------------------------------
    // Health Calculation
    // -------------------------------------------------------------------

    /// Calculate the decomposed health score of a pathway.
    ///
    /// Sub-scores:
    /// - **strength**: pathway strength clamped to [0.0, 1.0]
    /// - **activity**: `min(activation_count / 100.0, 1.0)`
    /// - **success**: pathway success rate (0.5 if no data)
    /// - **recency**: exponential decay from last activation (1.0 if recent)
    ///
    /// Composite: `0.30*strength + 0.25*activity + 0.25*success + 0.20*recency`
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn calculate_health(&self, pathway: &HebbianPathway) -> PathwayHealthScore {
        let strength_score = pathway.strength.clamp(0.0, 1.0);

        let activity_score = (pathway.activation_count as f64 / 100.0).min(1.0);

        let success_score = pathway.success_rate();

        let recency_score = Self::compute_recency_score(pathway.last_activation);

        // composite = 0.30*strength + 0.25*activity + 0.25*success + 0.20*recency
        let composite = 0.30_f64.mul_add(
            strength_score,
            0.25_f64.mul_add(
                activity_score,
                0.25_f64.mul_add(success_score, 0.20 * recency_score),
            ),
        );

        PathwayHealthScore {
            pathway_id: pathway.id.clone(),
            strength_score,
            activity_score,
            success_score,
            recency_score,
            composite,
        }
    }

    /// Compute the recency score from an optional last-activation timestamp.
    ///
    /// Returns 1.0 if the activation was very recent, decaying exponentially
    /// towards 0.0 as the elapsed time grows. If no activation has ever
    /// occurred, returns 0.0.
    fn compute_recency_score(last_activation: Option<SystemTime>) -> f64 {
        let Some(last) = last_activation else {
            return 0.0;
        };

        let elapsed_secs = SystemTime::now()
            .duration_since(last)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        if elapsed_secs == 0 {
            return 1.0;
        }

        // Exponential decay: score = exp(-elapsed_days / half_life)
        #[allow(clippy::cast_precision_loss)]
        let elapsed_days = elapsed_secs as f64 / SECONDS_PER_DAY as f64;
        let decay_constant = RECENCY_HALF_LIFE_DAYS / std::f64::consts::LN_2;
        (-elapsed_days / decay_constant).exp()
    }

    // -------------------------------------------------------------------
    // Candidate Identification
    // -------------------------------------------------------------------

    /// Identify pathways that are candidates for pruning.
    ///
    /// Evaluates every managed pathway against the current [`PruningConfig`]
    /// thresholds. A pathway may be flagged for multiple reasons; only the
    /// first matching reason is recorded.
    #[must_use]
    pub fn identify_candidates(&self) -> Vec<PruningCandidate> {
        let config = *self.config.read();
        let guard = self.pathways.read();
        let mut candidates = Vec::new();

        for pathway in guard.values() {
            if let Some(reason) = Self::evaluate_pathway(pathway, &config) {
                let health = self.calculate_health(pathway);
                candidates.push(PruningCandidate {
                    pathway_id: pathway.id.clone(),
                    reason,
                    health_score: health.composite,
                    pathway: pathway.clone(),
                });
            }
        }

        drop(guard);
        candidates
    }

    /// Evaluate a single pathway against the config, returning the first
    /// matching [`PruneReason`] or `None` if the pathway is healthy.
    fn evaluate_pathway(
        pathway: &HebbianPathway,
        config: &PruningConfig,
    ) -> Option<PruneReason> {
        // Check strength
        if pathway.strength < config.min_strength {
            return Some(PruneReason::WeakStrength);
        }

        // Check activity
        if pathway.activation_count < config.min_activations {
            return Some(PruneReason::LowActivity);
        }

        // Check success rate
        if pathway.success_rate() < config.min_success_rate {
            return Some(PruneReason::LowSuccessRate);
        }

        // Check staleness
        if Self::is_stale(pathway, config.inactive_days) {
            return Some(PruneReason::Stale);
        }

        // Check age-based pruning: old pathways with decayed strength
        if config.max_age_days > 0
            && pathway.strength < config.age_strength_threshold
            && Self::is_stale(pathway, config.max_age_days)
        {
            return Some(PruneReason::AgedOut);
        }

        None
    }

    /// Determine whether a pathway is stale (not activated within `inactive_days`).
    fn is_stale(pathway: &HebbianPathway, inactive_days: u64) -> bool {
        let Some(last) = pathway.last_activation else {
            // Never activated -- considered stale if inactive_days > 0
            return inactive_days > 0;
        };

        SystemTime::now()
            .duration_since(last)
            .map(|d| d.as_secs() > inactive_days.saturating_mul(SECONDS_PER_DAY))
            .unwrap_or(false)
    }

    // -------------------------------------------------------------------
    // Pruning Execution
    // -------------------------------------------------------------------

    /// Execute a full pruning cycle.
    ///
    /// Identifies candidates, removes them from the pathway registry, and
    /// appends a [`PruningReport`] to the bounded history.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Other`] if the pruning cycle encounters an
    /// inconsistency (e.g. a candidate is missing from the registry).
    pub fn prune(&self) -> Result<PruningReport> {
        let candidates = self.identify_candidates();
        let total_evaluated = self.pathway_count();

        {
            let mut guard = self.pathways.write();
            for candidate in &candidates {
                guard.remove(&candidate.pathway_id);
            }
        }

        let pruned_count = candidates.len();
        let retained_count = total_evaluated.saturating_sub(pruned_count);

        let report = PruningReport {
            total_evaluated,
            pruned_count,
            retained_count,
            candidates,
            timestamp: SystemTime::now(),
        };

        {
            let mut history = self.pruning_history.write();
            if history.len() >= PRUNING_HISTORY_CAPACITY {
                history.remove(0);
            }
            history.push(report.clone());
        }

        Ok(report)
    }

    /// Prune specific pathways by their IDs.
    ///
    /// Returns the number of pathways actually removed. IDs that do not
    /// match any existing pathway are silently skipped.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] if the `ids` slice is empty.
    pub fn prune_by_ids(&self, ids: &[&str]) -> Result<usize> {
        if ids.is_empty() {
            return Err(Error::Validation(
                "Must provide at least one pathway ID to prune".to_string(),
            ));
        }

        let mut guard = self.pathways.write();
        let mut removed = 0_usize;
        let mut candidates = Vec::new();

        for &id in ids {
            if let Some(pathway) = guard.remove(id) {
                let health = self.calculate_health(&pathway);
                candidates.push(PruningCandidate {
                    pathway_id: id.to_string(),
                    reason: PruneReason::ManualPrune,
                    health_score: health.composite,
                    pathway,
                });
                removed += 1;
            }
        }

        let total_evaluated = guard.len() + removed;
        drop(guard);

        if removed > 0 {
            let report = PruningReport {
                total_evaluated,
                pruned_count: removed,
                retained_count: total_evaluated.saturating_sub(removed),
                candidates,
                timestamp: SystemTime::now(),
            };

            let mut history = self.pruning_history.write();
            if history.len() >= PRUNING_HISTORY_CAPACITY {
                history.remove(0);
            }
            history.push(report);
        }

        Ok(removed)
    }

    // -------------------------------------------------------------------
    // Configuration
    // -------------------------------------------------------------------

    /// Replace the current pruning configuration.
    pub fn set_config(&self, config: PruningConfig) {
        *self.config.write() = config;
    }

    /// Get a copy of the current pruning configuration.
    #[must_use]
    pub fn get_config(&self) -> PruningConfig {
        *self.config.read()
    }

    // -------------------------------------------------------------------
    // History & Statistics
    // -------------------------------------------------------------------

    /// Get the pruning history.
    #[must_use]
    pub fn pruning_history(&self) -> Vec<PruningReport> {
        self.pruning_history.read().clone()
    }

    /// Count pathways with a composite health score above 0.5.
    #[must_use]
    pub fn healthy_pathway_count(&self) -> usize {
        let guard = self.pathways.read();
        guard
            .values()
            .filter(|p| self.calculate_health(p).composite > 0.5)
            .count()
    }

    /// Calculate the average composite health across all pathways.
    ///
    /// Returns 0.0 if no pathways are managed.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn average_health(&self) -> f64 {
        let guard = self.pathways.read();
        if guard.is_empty() {
            return 0.0;
        }
        let sum: f64 = guard
            .values()
            .map(|p| self.calculate_health(p).composite)
            .sum();
        sum / guard.len() as f64
    }

    /// Get the `n` weakest pathways sorted by ascending composite health.
    #[must_use]
    pub fn weakest_pathways(&self, n: usize) -> Vec<PathwayHealthScore> {
        let guard = self.pathways.read();
        let mut scores: Vec<PathwayHealthScore> =
            guard.values().map(|p| self.calculate_health(p)).collect();
        drop(guard);

        scores.sort_by(|a, b| {
            a.composite
                .partial_cmp(&b.composite)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        scores.truncate(n);
        scores
    }

    /// Get the `n` strongest pathways sorted by descending composite health.
    #[must_use]
    pub fn strongest_pathways(&self, n: usize) -> Vec<PathwayHealthScore> {
        let guard = self.pathways.read();
        let mut scores: Vec<PathwayHealthScore> =
            guard.values().map(|p| self.calculate_health(p)).collect();
        drop(guard);

        scores.sort_by(|a, b| {
            b.composite
                .partial_cmp(&a.composite)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        scores.truncate(n);
        scores
    }
}

impl Default for PathwayPruner {
    fn default() -> Self {
        Self::new()
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::m5_learning::PathwayType;

    // -----------------------------------------------------------------------
    // Test Helpers
    // -----------------------------------------------------------------------

    /// Create a pathway with the given id and strength.
    fn make_pathway(id: &str, strength: f64) -> HebbianPathway {
        HebbianPathway {
            id: id.to_string(),
            source: format!("{id}_src"),
            target: format!("{id}_tgt"),
            strength,
            pathway_type: PathwayType::ServiceToService,
            ltp_count: 0,
            ltd_count: 0,
            activation_count: 0,
            stdp_delta: 0.0,
            success_count: 0,
            failure_count: 0,
            last_activation: None,
            last_success: None,
        }
    }

    /// Create a pathway that passes all moderate-policy checks.
    fn make_healthy_pathway(id: &str) -> HebbianPathway {
        HebbianPathway {
            id: id.to_string(),
            source: format!("{id}_src"),
            target: format!("{id}_tgt"),
            strength: 0.8,
            pathway_type: PathwayType::ServiceToService,
            ltp_count: 5,
            ltd_count: 1,
            activation_count: 20,
            stdp_delta: 0.0,
            success_count: 15,
            failure_count: 5,
            last_activation: Some(SystemTime::now()),
            last_success: Some(SystemTime::now()),
        }
    }

    /// Create a pathway with low strength (below moderate threshold).
    fn make_weak_pathway(id: &str) -> HebbianPathway {
        let mut p = make_pathway(id, 0.05);
        p.activation_count = 20;
        p.success_count = 15;
        p.failure_count = 5;
        p.last_activation = Some(SystemTime::now());
        p
    }

    /// Create a pathway with zero activations.
    fn make_inactive_pathway(id: &str) -> HebbianPathway {
        let mut p = make_pathway(id, 0.5);
        p.activation_count = 0;
        p.last_activation = Some(SystemTime::now());
        p
    }

    // -----------------------------------------------------------------------
    // 1-2: Construction
    // -----------------------------------------------------------------------

    #[test]
    fn test_new_default() {
        let pruner = PathwayPruner::new();
        assert_eq!(pruner.pathway_count(), 0);
        let config = pruner.get_config();
        // Default is Moderate
        assert!((config.min_strength - 0.15).abs() < f64::EPSILON);
        assert_eq!(config.inactive_days, 14);
        assert_eq!(config.min_activations, 5);
        assert!((config.min_success_rate - 0.2).abs() < f64::EPSILON);
        assert_eq!(config.max_age_days, 90);
        assert!((config.age_strength_threshold - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_with_config() {
        let config = PruningPolicy::Aggressive.to_config();
        let pruner = PathwayPruner::with_config(config);
        assert_eq!(pruner.pathway_count(), 0);
        let cfg = pruner.get_config();
        assert!((cfg.min_strength - 0.3).abs() < f64::EPSILON);
        assert_eq!(cfg.inactive_days, 7);
    }

    // -----------------------------------------------------------------------
    // 3-8: add / remove / get pathways
    // -----------------------------------------------------------------------

    #[test]
    fn test_add_pathway() {
        let pruner = PathwayPruner::new();
        let p = make_pathway("p1", 0.5);
        assert!(pruner.add_pathway(p).is_ok());
        assert_eq!(pruner.pathway_count(), 1);
    }

    #[test]
    fn test_add_pathway_empty_id() {
        let pruner = PathwayPruner::new();
        let p = make_pathway("", 0.5);
        assert!(pruner.add_pathway(p).is_err());
    }

    #[test]
    fn test_add_pathway_overwrites_same_id() {
        let pruner = PathwayPruner::new();
        let p1 = make_pathway("dup", 0.3);
        let p2 = make_pathway("dup", 0.9);
        assert!(pruner.add_pathway(p1).is_ok());
        assert!(pruner.add_pathway(p2).is_ok());
        assert_eq!(pruner.pathway_count(), 1);
        let fetched = pruner.get_pathway("dup");
        assert!(fetched.is_some());
        if let Some(f) = fetched {
            assert!((f.strength - 0.9).abs() < f64::EPSILON);
        }
    }

    #[test]
    fn test_remove_pathway() {
        let pruner = PathwayPruner::new();
        let p = make_pathway("rm1", 0.5);
        assert!(pruner.add_pathway(p).is_ok());
        assert!(pruner.remove_pathway("rm1").is_ok());
        assert_eq!(pruner.pathway_count(), 0);
    }

    #[test]
    fn test_remove_nonexistent_pathway() {
        let pruner = PathwayPruner::new();
        assert!(pruner.remove_pathway("ghost").is_err());
    }

    #[test]
    fn test_get_pathway_exists() {
        let pruner = PathwayPruner::new();
        let p = make_pathway("get1", 0.7);
        let _ = pruner.add_pathway(p);
        let fetched = pruner.get_pathway("get1");
        assert!(fetched.is_some());
        if let Some(f) = fetched {
            assert_eq!(f.id, "get1");
        }
    }

    // -----------------------------------------------------------------------
    // 9-10: pathway_count and pathway_ids
    // -----------------------------------------------------------------------

    #[test]
    fn test_pathway_ids() {
        let pruner = PathwayPruner::new();
        let _ = pruner.add_pathway(make_pathway("a", 0.5));
        let _ = pruner.add_pathway(make_pathway("b", 0.5));
        let _ = pruner.add_pathway(make_pathway("c", 0.5));
        let mut ids = pruner.pathway_ids();
        ids.sort();
        assert_eq!(ids, vec!["a", "b", "c"]);
    }

    #[test]
    fn test_pathway_count_empty() {
        let pruner = PathwayPruner::new();
        assert_eq!(pruner.pathway_count(), 0);
    }

    // -----------------------------------------------------------------------
    // 11-18: Health score calculation
    // -----------------------------------------------------------------------

    #[test]
    fn test_health_default_pathway() {
        let pruner = PathwayPruner::new();
        let p = make_pathway("h1", 0.5);
        let health = pruner.calculate_health(&p);
        // strength_score = 0.5
        // activity_score = 0.0 (0 activations)
        // success_score = 0.5 (default neutral)
        // recency_score = 0.0 (no activation)
        // composite = 0.30*0.5 + 0.25*0.0 + 0.25*0.5 + 0.20*0.0 = 0.15 + 0.125 = 0.275
        assert!((health.strength_score - 0.5).abs() < f64::EPSILON);
        assert!(health.activity_score.abs() < f64::EPSILON);
        assert!((health.success_score - 0.5).abs() < f64::EPSILON);
        assert!(health.recency_score.abs() < f64::EPSILON);
        assert!((health.composite - 0.275).abs() < 1e-10);
    }

    #[test]
    fn test_health_perfect_pathway() {
        let pruner = PathwayPruner::new();
        let mut p = make_pathway("perf", 1.0);
        p.activation_count = 200;
        p.success_count = 100;
        p.failure_count = 0;
        p.last_activation = Some(SystemTime::now());

        let health = pruner.calculate_health(&p);
        assert!((health.strength_score - 1.0).abs() < f64::EPSILON);
        assert!((health.activity_score - 1.0).abs() < f64::EPSILON);
        assert!((health.success_score - 1.0).abs() < f64::EPSILON);
        // recency_score ~ 1.0 for very recent activation
        assert!(health.recency_score > 0.99);
        assert!(health.composite > 0.95);
    }

    #[test]
    fn test_health_zero_strength() {
        let pruner = PathwayPruner::new();
        let p = make_pathway("zero", 0.0);
        let health = pruner.calculate_health(&p);
        assert!(health.strength_score.abs() < f64::EPSILON);
        assert!(health.composite >= 0.0);
    }

    #[test]
    fn test_health_clamped_strength() {
        let pruner = PathwayPruner::new();
        let p = make_pathway("clamp", 1.5);
        let health = pruner.calculate_health(&p);
        assert!((health.strength_score - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_health_activity_saturation() {
        let pruner = PathwayPruner::new();
        let mut p = make_pathway("sat", 0.5);
        p.activation_count = 200;
        let health = pruner.calculate_health(&p);
        assert!((health.activity_score - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_health_partial_activity() {
        let pruner = PathwayPruner::new();
        let mut p = make_pathway("pa", 0.5);
        p.activation_count = 50;
        let health = pruner.calculate_health(&p);
        assert!((health.activity_score - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_health_success_rate_computation() {
        let pruner = PathwayPruner::new();
        let mut p = make_pathway("sr", 0.5);
        p.success_count = 3;
        p.failure_count = 7;
        let health = pruner.calculate_health(&p);
        assert!((health.success_score - 0.3).abs() < f64::EPSILON);
    }

    #[test]
    fn test_health_recency_recent_activation() {
        let pruner = PathwayPruner::new();
        let mut p = make_pathway("rec", 0.5);
        p.last_activation = Some(SystemTime::now());
        let health = pruner.calculate_health(&p);
        assert!(health.recency_score > 0.99);
    }

    // -----------------------------------------------------------------------
    // 19-24: Candidate identification
    // -----------------------------------------------------------------------

    #[test]
    fn test_identify_candidates_empty() {
        let pruner = PathwayPruner::new();
        let candidates = pruner.identify_candidates();
        assert!(candidates.is_empty());
    }

    #[test]
    fn test_identify_candidates_weak_strength() {
        let pruner = PathwayPruner::new();
        let _ = pruner.add_pathway(make_weak_pathway("weak1"));
        let candidates = pruner.identify_candidates();
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].reason, PruneReason::WeakStrength);
    }

    #[test]
    fn test_identify_candidates_low_activity() {
        let pruner = PathwayPruner::new();
        let _ = pruner.add_pathway(make_inactive_pathway("lazy1"));
        let candidates = pruner.identify_candidates();
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].reason, PruneReason::LowActivity);
    }

    #[test]
    fn test_identify_candidates_healthy_not_flagged() {
        let pruner = PathwayPruner::new();
        let _ = pruner.add_pathway(make_healthy_pathway("strong1"));
        let candidates = pruner.identify_candidates();
        assert!(
            candidates.is_empty(),
            "Healthy pathways should not be flagged"
        );
    }

    #[test]
    fn test_identify_candidates_low_success_rate() {
        let pruner = PathwayPruner::new();
        let mut p = make_pathway("lowsr", 0.5);
        p.activation_count = 20;
        p.success_count = 1;
        p.failure_count = 19;
        p.last_activation = Some(SystemTime::now());
        let _ = pruner.add_pathway(p);
        let candidates = pruner.identify_candidates();
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].reason, PruneReason::LowSuccessRate);
    }

    #[test]
    fn test_identify_candidates_mixed() {
        let pruner = PathwayPruner::new();
        let _ = pruner.add_pathway(make_healthy_pathway("ok1"));
        let _ = pruner.add_pathway(make_weak_pathway("weak2"));
        let _ = pruner.add_pathway(make_inactive_pathway("lazy2"));
        let candidates = pruner.identify_candidates();
        assert_eq!(candidates.len(), 2, "Should flag weak and inactive");
    }

    // -----------------------------------------------------------------------
    // 25-31: Pruning execution
    // -----------------------------------------------------------------------

    #[test]
    fn test_prune_empty() {
        let pruner = PathwayPruner::new();
        let report = pruner.prune();
        assert!(report.is_ok());
        if let Ok(r) = report {
            assert_eq!(r.total_evaluated, 0);
            assert_eq!(r.pruned_count, 0);
            assert_eq!(r.retained_count, 0);
        }
    }

    #[test]
    fn test_prune_removes_candidates() {
        let pruner = PathwayPruner::new();
        let _ = pruner.add_pathway(make_weak_pathway("prune1"));
        let _ = pruner.add_pathway(make_healthy_pathway("keep1"));

        let report = pruner.prune();
        assert!(report.is_ok());
        if let Ok(r) = report {
            assert_eq!(r.pruned_count, 1);
            assert_eq!(r.retained_count, 1);
        }
        assert_eq!(pruner.pathway_count(), 1);
        assert!(pruner.get_pathway("prune1").is_none());
        assert!(pruner.get_pathway("keep1").is_some());
    }

    #[test]
    fn test_prune_report_contents() {
        let pruner = PathwayPruner::new();
        let _ = pruner.add_pathway(make_weak_pathway("rp1"));

        let report = pruner.prune();
        assert!(report.is_ok());
        if let Ok(r) = report {
            assert_eq!(r.total_evaluated, 1);
            assert_eq!(r.pruned_count, 1);
            assert_eq!(r.candidates.len(), 1);
            assert_eq!(r.candidates[0].pathway_id, "rp1");
        }
    }

    #[test]
    fn test_prune_by_ids() {
        let pruner = PathwayPruner::new();
        let _ = pruner.add_pathway(make_healthy_pathway("m1"));
        let _ = pruner.add_pathway(make_healthy_pathway("m2"));
        let _ = pruner.add_pathway(make_healthy_pathway("m3"));

        let result = pruner.prune_by_ids(&["m1", "m3"]);
        assert!(result.is_ok());
        assert_eq!(result.ok().unwrap_or(0), 2);
        assert_eq!(pruner.pathway_count(), 1);
        assert!(pruner.get_pathway("m2").is_some());
    }

    #[test]
    fn test_prune_by_ids_empty_slice() {
        let pruner = PathwayPruner::new();
        let ids: &[&str] = &[];
        assert!(pruner.prune_by_ids(ids).is_err());
    }

    #[test]
    fn test_prune_by_ids_nonexistent() {
        let pruner = PathwayPruner::new();
        let _ = pruner.add_pathway(make_healthy_pathway("exists"));
        let result = pruner.prune_by_ids(&["ghost1", "ghost2"]);
        assert!(result.is_ok());
        assert_eq!(result.ok().unwrap_or(99), 0);
        assert_eq!(pruner.pathway_count(), 1);
    }

    #[test]
    fn test_prune_by_ids_manual_reason() {
        let pruner = PathwayPruner::new();
        let _ = pruner.add_pathway(make_healthy_pathway("man1"));
        let _ = pruner.prune_by_ids(&["man1"]);

        let history = pruner.pruning_history();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].candidates[0].reason, PruneReason::ManualPrune);
    }

    // -----------------------------------------------------------------------
    // 32-34: Pruning reports and history
    // -----------------------------------------------------------------------

    #[test]
    fn test_pruning_history_tracking() {
        let pruner = PathwayPruner::new();
        for i in 0..3 {
            let _ = pruner.add_pathway(make_weak_pathway(&format!("h{i}")));
            let _ = pruner.prune();
        }
        assert_eq!(pruner.pruning_history().len(), 3);
    }

    #[test]
    fn test_pruning_history_bounded() {
        let pruner = PathwayPruner::new();
        for i in 0..110 {
            let _ = pruner.add_pathway(make_weak_pathway(&format!("b{i}")));
            let _ = pruner.prune();
        }
        assert!(pruner.pruning_history().len() <= PRUNING_HISTORY_CAPACITY);
    }

    #[test]
    fn test_pruning_history_empty_initially() {
        let pruner = PathwayPruner::new();
        assert!(pruner.pruning_history().is_empty());
    }

    // -----------------------------------------------------------------------
    // 35-37: Config changes
    // -----------------------------------------------------------------------

    #[test]
    fn test_set_config() {
        let pruner = PathwayPruner::new();
        let aggressive = PruningPolicy::Aggressive.to_config();
        pruner.set_config(aggressive);
        let cfg = pruner.get_config();
        assert!((cfg.min_strength - 0.3).abs() < f64::EPSILON);
    }

    #[test]
    fn test_config_affects_pruning() {
        let pruner = PathwayPruner::new();
        // Pathway with strength 0.15 -- passes conservative but not moderate
        let mut p = make_pathway("border", 0.15);
        p.activation_count = 20;
        p.success_count = 10;
        p.failure_count = 10;
        p.last_activation = Some(SystemTime::now());
        let _ = pruner.add_pathway(p);

        // With conservative config (min_strength=0.1), should NOT be pruned
        pruner.set_config(PruningPolicy::Conservative.to_config());
        let candidates = pruner.identify_candidates();
        assert!(
            candidates.is_empty(),
            "Conservative policy should keep strength=0.15"
        );

        // With aggressive config (min_strength=0.3), SHOULD be pruned
        pruner.set_config(PruningPolicy::Aggressive.to_config());
        let candidates = pruner.identify_candidates();
        assert_eq!(candidates.len(), 1, "Aggressive policy should flag strength=0.15");
    }

    #[test]
    fn test_get_config_returns_copy() {
        let pruner = PathwayPruner::new();
        let cfg1 = pruner.get_config();
        pruner.set_config(PruningPolicy::Aggressive.to_config());
        let cfg2 = pruner.get_config();
        // cfg1 should still reflect old config (Moderate: 0.15)
        assert!((cfg1.min_strength - 0.15).abs() < f64::EPSILON);
        assert!((cfg2.min_strength - 0.3).abs() < f64::EPSILON);
    }

    // -----------------------------------------------------------------------
    // 38-40: Policy defaults
    // -----------------------------------------------------------------------

    #[test]
    fn test_conservative_policy() {
        let config = PruningPolicy::Conservative.to_config();
        assert!((config.min_strength - 0.1).abs() < f64::EPSILON);
        assert_eq!(config.inactive_days, 30);
        assert_eq!(config.min_activations, 3);
        assert!((config.min_success_rate - 0.1).abs() < f64::EPSILON);
    }

    #[test]
    fn test_moderate_policy() {
        let config = PruningPolicy::Moderate.to_config();
        assert!((config.min_strength - 0.15).abs() < f64::EPSILON);
        assert_eq!(config.inactive_days, 14);
        assert_eq!(config.min_activations, 5);
        assert!((config.min_success_rate - 0.2).abs() < f64::EPSILON);
        assert_eq!(config.max_age_days, 90);
        assert!((config.age_strength_threshold - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_aggressive_policy() {
        let config = PruningPolicy::Aggressive.to_config();
        assert!((config.min_strength - 0.3).abs() < f64::EPSILON);
        assert_eq!(config.inactive_days, 7);
        assert_eq!(config.min_activations, 10);
        assert!((config.min_success_rate - 0.3).abs() < f64::EPSILON);
    }

    // -----------------------------------------------------------------------
    // 41-44: Edge cases
    // -----------------------------------------------------------------------

    #[test]
    fn test_single_pathway_prune() {
        let pruner = PathwayPruner::new();
        let _ = pruner.add_pathway(make_weak_pathway("solo"));
        let report = pruner.prune();
        assert!(report.is_ok());
        if let Ok(r) = report {
            assert_eq!(r.total_evaluated, 1);
            assert_eq!(r.pruned_count, 1);
        }
        assert_eq!(pruner.pathway_count(), 0);
    }

    #[test]
    fn test_boundary_strength_at_threshold() {
        let pruner = PathwayPruner::new();
        // Strength exactly at moderate threshold (0.2) should NOT be flagged for WeakStrength
        let mut p = make_pathway("boundary", 0.2);
        p.activation_count = 20;
        p.success_count = 10;
        p.failure_count = 10;
        p.last_activation = Some(SystemTime::now());
        let _ = pruner.add_pathway(p);

        let candidates = pruner.identify_candidates();
        // The pathway has success_rate = 0.5 which is > 0.2 threshold
        // Strength 0.2 is NOT < 0.2, so no WeakStrength
        // 20 activations > 5, so no LowActivity
        // 0.5 success rate > 0.2 min, so no LowSuccessRate
        assert!(
            candidates.is_empty(),
            "Pathway exactly at threshold should not be pruned"
        );
    }

    #[test]
    fn test_get_pathway_nonexistent() {
        let pruner = PathwayPruner::new();
        assert!(pruner.get_pathway("nope").is_none());
    }

    #[test]
    fn test_default_trait_impl() {
        let pruner = PathwayPruner::default();
        assert_eq!(pruner.pathway_count(), 0);
        let cfg = pruner.get_config();
        assert!((cfg.min_strength - 0.15).abs() < f64::EPSILON);
    }

    // -----------------------------------------------------------------------
    // 45-47: Weakest / strongest sorting
    // -----------------------------------------------------------------------

    #[test]
    fn test_weakest_pathways_sorted() {
        let pruner = PathwayPruner::new();
        let _ = pruner.add_pathway(make_healthy_pathway("strong"));
        let _ = pruner.add_pathway(make_pathway("weak", 0.1));
        let _ = pruner.add_pathway(make_pathway("mid", 0.4));

        let weakest = pruner.weakest_pathways(3);
        assert_eq!(weakest.len(), 3);
        // Should be sorted ascending by composite
        assert!(weakest[0].composite <= weakest[1].composite);
        assert!(weakest[1].composite <= weakest[2].composite);
    }

    #[test]
    fn test_strongest_pathways_sorted() {
        let pruner = PathwayPruner::new();
        let _ = pruner.add_pathway(make_healthy_pathway("top"));
        let _ = pruner.add_pathway(make_pathway("low", 0.1));

        let strongest = pruner.strongest_pathways(2);
        assert_eq!(strongest.len(), 2);
        assert!(strongest[0].composite >= strongest[1].composite);
    }

    #[test]
    fn test_weakest_pathways_truncated() {
        let pruner = PathwayPruner::new();
        let _ = pruner.add_pathway(make_pathway("a", 0.1));
        let _ = pruner.add_pathway(make_pathway("b", 0.2));
        let _ = pruner.add_pathway(make_pathway("c", 0.3));

        let weakest = pruner.weakest_pathways(2);
        assert_eq!(weakest.len(), 2);
    }

    // -----------------------------------------------------------------------
    // 48-50: Healthy count, average health, and prune reason display
    // -----------------------------------------------------------------------

    #[test]
    fn test_healthy_pathway_count() {
        let pruner = PathwayPruner::new();
        let _ = pruner.add_pathway(make_healthy_pathway("h1"));
        let _ = pruner.add_pathway(make_healthy_pathway("h2"));
        let _ = pruner.add_pathway(make_pathway("low", 0.0));

        let healthy = pruner.healthy_pathway_count();
        assert_eq!(healthy, 2, "Only healthy pathways should be counted");
    }

    #[test]
    fn test_average_health() {
        let pruner = PathwayPruner::new();
        assert!(
            pruner.average_health().abs() < f64::EPSILON,
            "Average health of empty pruner should be 0.0"
        );

        let _ = pruner.add_pathway(make_healthy_pathway("avg1"));
        let avg = pruner.average_health();
        assert!(avg > 0.0, "Average health should be positive for healthy pathway");
    }

    #[test]
    fn test_prune_reason_display() {
        assert_eq!(PruneReason::WeakStrength.to_string(), "WeakStrength");
        assert_eq!(PruneReason::LowActivity.to_string(), "LowActivity");
        assert_eq!(PruneReason::LowSuccessRate.to_string(), "LowSuccessRate");
        assert_eq!(PruneReason::Stale.to_string(), "Stale");
        assert_eq!(PruneReason::ManualPrune.to_string(), "ManualPrune");
        assert_eq!(PruneReason::AgedOut.to_string(), "AgedOut");
    }

    // -----------------------------------------------------------------------
    // 51-55: Age-based pruning (V3 integration)
    // -----------------------------------------------------------------------

    #[test]
    fn test_aged_out_config_fields_in_policies() {
        let conservative = PruningPolicy::Conservative.to_config();
        assert_eq!(conservative.max_age_days, 180);
        assert!((conservative.age_strength_threshold - 0.3).abs() < f64::EPSILON);

        let moderate = PruningPolicy::Moderate.to_config();
        assert_eq!(moderate.max_age_days, 90);
        assert!((moderate.age_strength_threshold - 0.5).abs() < f64::EPSILON);

        let aggressive = PruningPolicy::Aggressive.to_config();
        assert_eq!(aggressive.max_age_days, 30);
        assert!((aggressive.age_strength_threshold - 0.7).abs() < f64::EPSILON);
    }

    #[test]
    fn test_aged_out_not_triggered_for_strong_pathway() {
        // A pathway with strength above age_strength_threshold should NOT be
        // flagged as AgedOut, even if it's very old.
        let pruner = PathwayPruner::new();
        let mut p = make_pathway("old_strong", 0.8);
        p.activation_count = 20;
        p.success_count = 15;
        p.failure_count = 5;
        // last_activation long ago (will trigger Stale but not AgedOut)
        p.last_activation = Some(
            SystemTime::now()
                - std::time::Duration::from_secs(100 * SECONDS_PER_DAY),
        );
        let _ = pruner.add_pathway(p);

        let candidates = pruner.identify_candidates();
        // Should be Stale (inactive_days=14), NOT AgedOut (strength 0.8 > 0.5)
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].reason, PruneReason::Stale);
    }

    #[test]
    fn test_aged_out_triggered_for_decayed_old_pathway() {
        // Pathway with strength below age_strength_threshold AND old enough
        // for max_age_days should be AgedOut. But Stale fires first at
        // inactive_days=14, so we need to set inactive_days > max_age_days
        // to test AgedOut in isolation.
        let config = PruningConfig {
            min_strength: 0.1,
            inactive_days: 365, // Disable Stale by setting very high
            min_activations: 0,
            min_success_rate: 0.0,
            max_age_days: 60,
            age_strength_threshold: 0.5,
        };
        let pruner = PathwayPruner::with_config(config);
        let mut p = make_pathway("decayed_old", 0.35); // below 0.5 threshold
        p.activation_count = 20;
        p.success_count = 15;
        p.failure_count = 5;
        p.last_activation = Some(
            SystemTime::now()
                - std::time::Duration::from_secs(90 * SECONDS_PER_DAY),
        );
        let _ = pruner.add_pathway(p);

        let candidates = pruner.identify_candidates();
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].reason, PruneReason::AgedOut);
    }

    #[test]
    fn test_aged_out_not_triggered_when_too_young() {
        let config = PruningConfig {
            min_strength: 0.1,
            inactive_days: 365,
            min_activations: 0,
            min_success_rate: 0.0,
            max_age_days: 60,
            age_strength_threshold: 0.5,
        };
        let pruner = PathwayPruner::with_config(config);
        let mut p = make_pathway("young_weak", 0.35);
        p.activation_count = 20;
        p.success_count = 15;
        p.failure_count = 5;
        // Only 30 days old -- below max_age_days of 60
        p.last_activation = Some(
            SystemTime::now()
                - std::time::Duration::from_secs(30 * SECONDS_PER_DAY),
        );
        let _ = pruner.add_pathway(p);

        let candidates = pruner.identify_candidates();
        assert!(
            candidates.is_empty(),
            "Pathway younger than max_age_days should not be AgedOut"
        );
    }

    #[test]
    fn test_aged_out_prune_reason_in_report() {
        let config = PruningConfig {
            min_strength: 0.1,
            inactive_days: 365,
            min_activations: 0,
            min_success_rate: 0.0,
            max_age_days: 60,
            age_strength_threshold: 0.5,
        };
        let pruner = PathwayPruner::with_config(config);
        let mut p = make_pathway("aged_prune", 0.2);
        p.activation_count = 5;
        p.success_count = 3;
        p.failure_count = 2;
        p.last_activation = Some(
            SystemTime::now()
                - std::time::Duration::from_secs(100 * SECONDS_PER_DAY),
        );
        let _ = pruner.add_pathway(p);

        let report = pruner.prune();
        assert!(report.is_ok());
        if let Ok(r) = report {
            assert_eq!(r.pruned_count, 1);
            assert_eq!(r.candidates[0].reason, PruneReason::AgedOut);
        }
    }
}
