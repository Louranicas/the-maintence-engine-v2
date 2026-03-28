//! # N04: STDP Bridge
//!
//! Tool chain STDP learning from service interactions.
//!
//! ## Layer: L8 (Nexus Integration)
//! ## Module: N04
//! ## Dependencies: L1 (Error, Timestamp)
//!
//! ## Trait
//!
//! | Trait | Purpose |
//! |-------|---------|
//! | [`StdpBridge`] | Record service interactions and maintain co-activation weights |
//!
//! All methods are `&self` (C2) with interior mutability via `RwLock`.
//! Methods returning data through `RwLock` return OWNED types (C7).
//!
//! ## C12 Enforcement
//!
//! Every service interaction records STDP co-activation:
//! - **Success:** weight += `co_activation_delta` (default 0.05)
//! - **Failure:** weight -= `failure_penalty` (default 0.02)
//! - Weights are clamped to `[weight_floor, weight_ceiling]` after each update.
//!
//! ## Decay
//!
//! [`StdpBridge::apply_decay`] multiplies all weights by `(1.0 - decay_rate)`.
//! Pathways that fall below `weight_floor` are removed.
//!
//! ## Related Documentation
//! - [Nexus Specs](../../ai_specs/nexus-specs/)
//! - [STDP Learning Parameters](../../CLAUDE.md)

use std::collections::HashMap;
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};

use parking_lot::RwLock;

use crate::m1_foundation::shared_types::Timestamp;
use crate::{Error, Result};

// ============================================================================
// PathwayRecord
// ============================================================================

/// A single STDP pathway between two services.
///
/// Tracks the co-activation weight, interaction count, and success rate.
#[derive(Clone, Debug)]
pub struct PathwayRecord {
    /// Source service identifier.
    source: String,
    /// Target service identifier.
    target: String,
    /// Co-activation weight in `[weight_floor, weight_ceiling]`.
    weight: f64,
    /// Total number of co-activations.
    co_activations: u64,
    /// Timestamp of the most recent interaction.
    last_interaction: Timestamp,
    /// Ratio of successful interactions to total interactions.
    success_rate: f64,
    /// Number of successful interactions (internal tracking).
    success_count: u64,
}

impl PathwayRecord {
    /// Access the source service.
    #[must_use]
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Access the target service.
    #[must_use]
    pub fn target(&self) -> &str {
        &self.target
    }

    /// Access the co-activation weight.
    #[must_use]
    pub const fn weight(&self) -> f64 {
        self.weight
    }

    /// Access the co-activation count.
    #[must_use]
    pub const fn co_activations(&self) -> u64 {
        self.co_activations
    }

    /// Access the last interaction timestamp.
    #[must_use]
    pub const fn last_interaction(&self) -> Timestamp {
        self.last_interaction
    }

    /// Access the success rate.
    #[must_use]
    pub const fn success_rate(&self) -> f64 {
        self.success_rate
    }
}

// ============================================================================
// InteractionStats
// ============================================================================

/// Snapshot of STDP bridge statistics.
#[derive(Clone, Debug)]
pub struct InteractionStats {
    /// Total interactions recorded.
    pub total_interactions: u64,
    /// Total successful interactions.
    pub total_success: u64,
    /// Total failed interactions.
    pub total_failure: u64,
    /// Number of active pathways.
    pub pathway_count: usize,
    /// Average weight across all pathways.
    pub avg_weight: f64,
    /// Overall success rate.
    pub success_rate: f64,
}

// ============================================================================
// StdpBridgeConfig
// ============================================================================

/// Configuration for the [`StdpBridgeCore`].
#[derive(Clone, Debug)]
pub struct StdpBridgeConfig {
    /// Weight increment on successful co-activation (C12: 0.05).
    co_activation_delta: f64,
    /// Weight decrement on failed interaction.
    failure_penalty: f64,
    /// Multiplicative decay rate applied by [`StdpBridge::apply_decay`].
    decay_rate: f64,
    /// Minimum weight — pathways below this are removed on decay.
    weight_floor: f64,
    /// Maximum weight — hard ceiling.
    weight_ceiling: f64,
    /// Maximum number of tracked pathways.
    max_pathways: usize,
}

impl StdpBridgeConfig {
    /// Create a new configuration with explicit values.
    ///
    /// All float values are clamped to sensible ranges.
    #[must_use]
    pub fn new(
        co_activation_delta: f64,
        failure_penalty: f64,
        decay_rate: f64,
        weight_floor: f64,
        weight_ceiling: f64,
        max_pathways: usize,
    ) -> Self {
        Self {
            co_activation_delta: co_activation_delta.clamp(0.0, 1.0),
            failure_penalty: failure_penalty.clamp(0.0, 1.0),
            decay_rate: decay_rate.clamp(0.0, 1.0),
            weight_floor: weight_floor.clamp(0.0, 1.0),
            weight_ceiling: weight_ceiling.clamp(0.0, 1.0),
            max_pathways: max_pathways.max(1),
        }
    }

    /// Access the co-activation delta.
    #[must_use]
    pub const fn co_activation_delta(&self) -> f64 {
        self.co_activation_delta
    }

    /// Access the failure penalty.
    #[must_use]
    pub const fn failure_penalty(&self) -> f64 {
        self.failure_penalty
    }

    /// Access the decay rate.
    #[must_use]
    pub const fn decay_rate(&self) -> f64 {
        self.decay_rate
    }

    /// Access the weight floor.
    #[must_use]
    pub const fn weight_floor(&self) -> f64 {
        self.weight_floor
    }

    /// Access the weight ceiling.
    #[must_use]
    pub const fn weight_ceiling(&self) -> f64 {
        self.weight_ceiling
    }

    /// Access the maximum pathway count.
    #[must_use]
    pub const fn max_pathways(&self) -> usize {
        self.max_pathways
    }
}

impl Default for StdpBridgeConfig {
    fn default() -> Self {
        Self {
            co_activation_delta: 0.05,
            failure_penalty: 0.02,
            decay_rate: 0.001,
            weight_floor: 0.01,
            weight_ceiling: 1.0,
            max_pathways: 10_000,
        }
    }
}

// ============================================================================
// StdpBridge (trait)
// ============================================================================

/// Trait for STDP-based service interaction learning.
///
/// All methods are `&self` (C2). State mutation uses interior mutability.
/// Methods returning data through `RwLock` return owned types (C7).
pub trait StdpBridge: Send + Sync + fmt::Debug {
    /// Record an interaction between two services.
    ///
    /// On success: weight += `co_activation_delta` (C12: +0.05).
    /// On failure: weight -= `failure_penalty`.
    /// Weight is clamped to `[weight_floor, weight_ceiling]`.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if the maximum pathway count is reached
    /// and neither source nor target has an existing pathway.
    fn record_interaction(&self, source: &str, target: &str, success: bool) -> Result<()>;

    /// Get the co-activation weight between two services.
    ///
    /// Returns `0.0` if no pathway exists.
    fn co_activation_weight(&self, source: &str, target: &str) -> f64;

    /// Get the strongest pathways, sorted by weight descending.
    fn strongest_pathways(&self, limit: usize) -> Vec<PathwayRecord>;

    /// Get the weakest pathways, sorted by weight ascending.
    fn weakest_pathways(&self, limit: usize) -> Vec<PathwayRecord>;

    /// Get the number of active pathways.
    fn pathway_count(&self) -> usize;

    /// Apply multiplicative decay to all pathways.
    ///
    /// Each weight is multiplied by `(1.0 - decay_rate)`.
    /// Pathways that fall below `weight_floor` are removed.
    fn apply_decay(&self);

    /// Get a snapshot of interaction statistics.
    fn interaction_stats(&self) -> InteractionStats;

    /// Clear all pathways and reset counters.
    fn reset(&self);
}

// ============================================================================
// StdpBridgeCore
// ============================================================================

/// Core implementation of [`StdpBridge`].
///
/// Uses `parking_lot::RwLock` for interior mutability and
/// `AtomicU64` for lock-free counters.
pub struct StdpBridgeCore {
    /// Active pathways keyed by `(source, target)`.
    pathways: RwLock<HashMap<(String, String), PathwayRecord>>,
    /// Configuration.
    config: StdpBridgeConfig,
    /// Total interactions recorded.
    total_interactions: AtomicU64,
    /// Total successful interactions.
    total_success: AtomicU64,
    /// Total failed interactions.
    total_failure: AtomicU64,
}

impl StdpBridgeCore {
    /// Create a new STDP bridge with the given configuration.
    #[must_use]
    pub fn new(config: StdpBridgeConfig) -> Self {
        Self {
            pathways: RwLock::new(HashMap::new()),
            config,
            total_interactions: AtomicU64::new(0),
            total_success: AtomicU64::new(0),
            total_failure: AtomicU64::new(0),
        }
    }

    /// Create a new STDP bridge with default configuration.
    #[must_use]
    pub fn with_defaults() -> Self {
        Self::new(StdpBridgeConfig::default())
    }

    /// Access the configuration.
    #[must_use]
    pub const fn config(&self) -> &StdpBridgeConfig {
        &self.config
    }
}

impl Default for StdpBridgeCore {
    fn default() -> Self {
        Self::with_defaults()
    }
}

impl fmt::Debug for StdpBridgeCore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let pw_count = self.pathways.read().len();
        f.debug_struct("StdpBridgeCore")
            .field("pathways", &pw_count)
            .field("config", &self.config)
            .field(
                "total_interactions",
                &self.total_interactions.load(Ordering::Relaxed),
            )
            .field(
                "total_success",
                &self.total_success.load(Ordering::Relaxed),
            )
            .field(
                "total_failure",
                &self.total_failure.load(Ordering::Relaxed),
            )
            .finish()
    }
}

impl StdpBridge for StdpBridgeCore {
    fn record_interaction(&self, source: &str, target: &str, success: bool) -> Result<()> {
        let key = (source.to_string(), target.to_string());

        let mut pathways = self.pathways.write();

        // Check if we need to create a new pathway.
        if !pathways.contains_key(&key) {
            if pathways.len() >= self.config.max_pathways {
                return Err(Error::Validation(format!(
                    "Maximum pathway count {} reached",
                    self.config.max_pathways
                )));
            }
            let initial_weight = self.config.weight_floor;
            pathways.insert(
                key.clone(),
                PathwayRecord {
                    source: source.to_string(),
                    target: target.to_string(),
                    weight: initial_weight,
                    co_activations: 0,
                    last_interaction: Timestamp::now(),
                    success_rate: 0.0,
                    success_count: 0,
                },
            );
        }

        // Update the pathway. Using `get_mut` is safe — we just ensured the key exists.
        if let Some(pathway) = pathways.get_mut(&key) {
            pathway.co_activations += 1;
            pathway.last_interaction = Timestamp::now();

            if success {
                pathway.weight = (pathway.weight + self.config.co_activation_delta)
                    .min(self.config.weight_ceiling);
                pathway.success_count += 1;
            } else {
                pathway.weight = (pathway.weight - self.config.failure_penalty)
                    .max(self.config.weight_floor);
            }

            // Recompute success rate.
            #[allow(clippy::cast_precision_loss)]
            {
                pathway.success_rate = if pathway.co_activations > 0 {
                    pathway.success_count as f64 / pathway.co_activations as f64
                } else {
                    0.0
                };
            }
        }

        drop(pathways);

        // Update atomic counters.
        self.total_interactions.fetch_add(1, Ordering::Relaxed);
        if success {
            self.total_success.fetch_add(1, Ordering::Relaxed);
        } else {
            self.total_failure.fetch_add(1, Ordering::Relaxed);
        }

        Ok(())
    }

    fn co_activation_weight(&self, source: &str, target: &str) -> f64 {
        let key = (source.to_string(), target.to_string());
        self.pathways
            .read()
            .get(&key)
            .map_or(0.0, |p| p.weight)
    }

    fn strongest_pathways(&self, limit: usize) -> Vec<PathwayRecord> {
        let mut sorted: Vec<PathwayRecord> = self.pathways.read().values().cloned().collect();
        sorted.sort_by(|a, b| b.weight.partial_cmp(&a.weight).unwrap_or(std::cmp::Ordering::Equal));
        sorted.truncate(limit);
        sorted
    }

    fn weakest_pathways(&self, limit: usize) -> Vec<PathwayRecord> {
        let mut sorted: Vec<PathwayRecord> = self.pathways.read().values().cloned().collect();
        sorted.sort_by(|a, b| a.weight.partial_cmp(&b.weight).unwrap_or(std::cmp::Ordering::Equal));
        sorted.truncate(limit);
        sorted
    }

    fn pathway_count(&self) -> usize {
        self.pathways.read().len()
    }

    fn apply_decay(&self) {
        let mut pathways = self.pathways.write();
        let decay_factor = 1.0 - self.config.decay_rate;
        let floor = self.config.weight_floor;

        // Apply decay to all pathways.
        for pathway in pathways.values_mut() {
            pathway.weight *= decay_factor;
        }

        // Remove pathways that fell below the floor.
        pathways.retain(|_, p| p.weight >= floor);
    }

    #[allow(clippy::cast_precision_loss)]
    fn interaction_stats(&self) -> InteractionStats {
        let (pw_count, weight_sum) = {
            let pathways = self.pathways.read();
            let count = pathways.len();
            let sum: f64 = pathways.values().map(|p| p.weight).sum();
            drop(pathways);
            (count, sum)
        };
        let avg_weight = if pw_count > 0 {
            weight_sum / pw_count as f64
        } else {
            0.0
        };

        let total = self.total_interactions.load(Ordering::Relaxed);
        let success = self.total_success.load(Ordering::Relaxed);
        let failure = self.total_failure.load(Ordering::Relaxed);
        let rate = if total > 0 {
            success as f64 / total as f64
        } else {
            0.0
        };

        InteractionStats {
            total_interactions: total,
            total_success: success,
            total_failure: failure,
            pathway_count: pw_count,
            avg_weight,
            success_rate: rate,
        }
    }

    fn reset(&self) {
        self.pathways.write().clear();
        self.total_interactions.store(0, Ordering::Relaxed);
        self.total_success.store(0, Ordering::Relaxed);
        self.total_failure.store(0, Ordering::Relaxed);
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ---- Helpers ----

    fn make_bridge() -> StdpBridgeCore {
        StdpBridgeCore::with_defaults()
    }

    fn make_bridge_with_config(
        delta: f64,
        penalty: f64,
        decay: f64,
        floor: f64,
        ceiling: f64,
        max: usize,
    ) -> StdpBridgeCore {
        StdpBridgeCore::new(StdpBridgeConfig::new(delta, penalty, decay, floor, ceiling, max))
    }

    // ---- Config ----

    #[test]
    fn test_config_default() {
        let cfg = StdpBridgeConfig::default();
        assert!((cfg.co_activation_delta() - 0.05).abs() < f64::EPSILON);
        assert!((cfg.failure_penalty() - 0.02).abs() < f64::EPSILON);
        assert!((cfg.decay_rate() - 0.001).abs() < f64::EPSILON);
        assert!((cfg.weight_floor() - 0.01).abs() < f64::EPSILON);
        assert!((cfg.weight_ceiling() - 1.0).abs() < f64::EPSILON);
        assert_eq!(cfg.max_pathways(), 10_000);
    }

    #[test]
    fn test_config_custom() {
        let cfg = StdpBridgeConfig::new(0.1, 0.05, 0.01, 0.02, 0.9, 5000);
        assert!((cfg.co_activation_delta() - 0.1).abs() < f64::EPSILON);
        assert!((cfg.failure_penalty() - 0.05).abs() < f64::EPSILON);
        assert!((cfg.decay_rate() - 0.01).abs() < f64::EPSILON);
        assert!((cfg.weight_floor() - 0.02).abs() < f64::EPSILON);
        assert!((cfg.weight_ceiling() - 0.9).abs() < f64::EPSILON);
        assert_eq!(cfg.max_pathways(), 5000);
    }

    #[test]
    fn test_config_clamps_values() {
        let cfg = StdpBridgeConfig::new(2.0, -1.0, 5.0, -0.5, 3.0, 0);
        assert!((cfg.co_activation_delta() - 1.0).abs() < f64::EPSILON);
        assert!(cfg.failure_penalty().abs() < f64::EPSILON);
        assert!((cfg.decay_rate() - 1.0).abs() < f64::EPSILON);
        assert!(cfg.weight_floor().abs() < f64::EPSILON);
        assert!((cfg.weight_ceiling() - 1.0).abs() < f64::EPSILON);
        assert_eq!(cfg.max_pathways(), 1);
    }

    #[test]
    fn test_config_clone() {
        let cfg = StdpBridgeConfig::default();
        let cloned = cfg.clone();
        assert!((cloned.co_activation_delta() - cfg.co_activation_delta()).abs() < f64::EPSILON);
    }

    // ---- Construction ----

    #[test]
    fn test_bridge_new_empty() {
        let bridge = make_bridge();
        assert_eq!(bridge.pathway_count(), 0);
        let stats = bridge.interaction_stats();
        assert_eq!(stats.total_interactions, 0);
        assert_eq!(stats.total_success, 0);
        assert_eq!(stats.total_failure, 0);
    }

    #[test]
    fn test_bridge_default() {
        let bridge = StdpBridgeCore::default();
        assert_eq!(bridge.pathway_count(), 0);
    }

    #[test]
    fn test_bridge_debug() {
        let bridge = make_bridge();
        let debug = format!("{bridge:?}");
        assert!(debug.contains("StdpBridgeCore"));
    }

    // ---- Record Interaction (Success) ----

    #[test]
    fn test_record_success_creates_pathway() {
        let bridge = make_bridge();
        assert!(bridge.record_interaction("a", "b", true).is_ok());
        assert_eq!(bridge.pathway_count(), 1);
    }

    #[test]
    fn test_record_success_increments_weight() {
        let bridge = make_bridge();
        bridge.record_interaction("a", "b", true).ok();
        // Initial weight = floor (0.01) + delta (0.05) = 0.06
        let weight = bridge.co_activation_weight("a", "b");
        assert!((weight - 0.06).abs() < f64::EPSILON);
    }

    #[test]
    fn test_record_multiple_successes() {
        let bridge = make_bridge();
        for _ in 0..5 {
            bridge.record_interaction("a", "b", true).ok();
        }
        // weight = floor + 5 * delta = 0.01 + 5 * 0.05 = 0.26
        let weight = bridge.co_activation_weight("a", "b");
        assert!((weight - 0.26).abs() < f64::EPSILON);
    }

    #[test]
    fn test_c12_enforcement_delta() {
        let bridge = make_bridge();
        bridge.record_interaction("svc-a", "svc-b", true).ok();
        let w1 = bridge.co_activation_weight("svc-a", "svc-b");
        bridge.record_interaction("svc-a", "svc-b", true).ok();
        let w2 = bridge.co_activation_weight("svc-a", "svc-b");
        // C12: +0.05 per successful call.
        assert!((w2 - w1 - 0.05).abs() < f64::EPSILON);
    }

    #[test]
    fn test_weight_does_not_exceed_ceiling() {
        let bridge = make_bridge();
        for _ in 0..100 {
            bridge.record_interaction("a", "b", true).ok();
        }
        let weight = bridge.co_activation_weight("a", "b");
        assert!(weight <= bridge.config().weight_ceiling());
    }

    // ---- Record Interaction (Failure) ----

    #[test]
    fn test_record_failure_creates_pathway() {
        let bridge = make_bridge();
        assert!(bridge.record_interaction("a", "b", false).is_ok());
        assert_eq!(bridge.pathway_count(), 1);
    }

    #[test]
    fn test_record_failure_decrements_weight() {
        let bridge = make_bridge();
        // Build up some weight first.
        for _ in 0..5 {
            bridge.record_interaction("a", "b", true).ok();
        }
        let before = bridge.co_activation_weight("a", "b");
        bridge.record_interaction("a", "b", false).ok();
        let after = bridge.co_activation_weight("a", "b");
        assert!((before - after - 0.02).abs() < f64::EPSILON);
    }

    #[test]
    fn test_weight_does_not_go_below_floor() {
        let bridge = make_bridge();
        for _ in 0..100 {
            bridge.record_interaction("a", "b", false).ok();
        }
        let weight = bridge.co_activation_weight("a", "b");
        assert!(weight >= bridge.config().weight_floor());
    }

    #[test]
    fn test_failure_penalty_from_initial() {
        let bridge = make_bridge();
        bridge.record_interaction("a", "b", false).ok();
        // Initial = floor (0.01), then -0.02 = -0.01, clamped to floor = 0.01
        let weight = bridge.co_activation_weight("a", "b");
        assert!((weight - bridge.config().weight_floor()).abs() < f64::EPSILON);
    }

    // ---- Co-activation Weight ----

    #[test]
    fn test_weight_nonexistent_pathway() {
        let bridge = make_bridge();
        assert!(bridge.co_activation_weight("x", "y").abs() < f64::EPSILON);
    }

    #[test]
    fn test_weight_directional() {
        let bridge = make_bridge();
        bridge.record_interaction("a", "b", true).ok();
        assert!(bridge.co_activation_weight("a", "b") > 0.0);
        // Reverse direction has no pathway.
        assert!(bridge.co_activation_weight("b", "a").abs() < f64::EPSILON);
    }

    // ---- Pathway Ranking ----

    #[test]
    fn test_strongest_pathways_empty() {
        let bridge = make_bridge();
        assert!(bridge.strongest_pathways(10).is_empty());
    }

    #[test]
    fn test_strongest_pathways_order() {
        let bridge = make_bridge();
        bridge.record_interaction("a", "b", true).ok();
        bridge.record_interaction("a", "b", true).ok();
        bridge.record_interaction("c", "d", true).ok();
        let strongest = bridge.strongest_pathways(10);
        assert_eq!(strongest.len(), 2);
        assert!(strongest[0].weight() >= strongest[1].weight());
    }

    #[test]
    fn test_strongest_pathways_limit() {
        let bridge = make_bridge();
        for i in 0..10 {
            let src = format!("s{i}");
            bridge.record_interaction(&src, "t", true).ok();
        }
        let strongest = bridge.strongest_pathways(3);
        assert_eq!(strongest.len(), 3);
    }

    #[test]
    fn test_weakest_pathways_order() {
        let bridge = make_bridge();
        bridge.record_interaction("a", "b", true).ok();
        bridge.record_interaction("a", "b", true).ok();
        bridge.record_interaction("c", "d", true).ok();
        let weakest = bridge.weakest_pathways(10);
        assert_eq!(weakest.len(), 2);
        assert!(weakest[0].weight() <= weakest[1].weight());
    }

    #[test]
    fn test_weakest_pathways_limit() {
        let bridge = make_bridge();
        for i in 0..10 {
            let src = format!("s{i}");
            bridge.record_interaction(&src, "t", true).ok();
        }
        let weakest = bridge.weakest_pathways(3);
        assert_eq!(weakest.len(), 3);
    }

    // ---- Decay ----

    #[test]
    fn test_decay_reduces_weights() {
        let bridge = make_bridge();
        for _ in 0..10 {
            bridge.record_interaction("a", "b", true).ok();
        }
        let before = bridge.co_activation_weight("a", "b");
        bridge.apply_decay();
        let after = bridge.co_activation_weight("a", "b");
        assert!(after < before);
    }

    #[test]
    fn test_decay_removes_below_floor() {
        // Config: high decay, low floor
        let bridge = make_bridge_with_config(0.05, 0.02, 0.99, 0.01, 1.0, 10_000);
        bridge.record_interaction("a", "b", true).ok();
        // Weight after success = 0.01 + 0.05 = 0.06
        bridge.apply_decay();
        // 0.06 * (1 - 0.99) = 0.0006 < 0.01 floor -> removed
        assert_eq!(bridge.pathway_count(), 0);
    }

    #[test]
    fn test_decay_preserves_strong_pathways() {
        let bridge = make_bridge();
        for _ in 0..15 {
            bridge.record_interaction("a", "b", true).ok();
        }
        // Weight = 0.01 + 15 * 0.05 = 0.76
        bridge.apply_decay();
        // 0.76 * 0.999 = 0.75924 > 0.01 floor -> preserved
        assert_eq!(bridge.pathway_count(), 1);
        assert!(bridge.co_activation_weight("a", "b") > 0.0);
    }

    #[test]
    fn test_decay_on_empty_does_nothing() {
        let bridge = make_bridge();
        bridge.apply_decay(); // should not panic
        assert_eq!(bridge.pathway_count(), 0);
    }

    #[test]
    fn test_repeated_decay_converges() {
        let bridge = make_bridge();
        for _ in 0..10 {
            bridge.record_interaction("a", "b", true).ok();
        }
        for _ in 0..100 {
            bridge.apply_decay();
        }
        let weight = bridge.co_activation_weight("a", "b");
        // After 100 rounds of 0.1% decay, weight should still be > floor
        // but much smaller than initial.
        if bridge.pathway_count() > 0 {
            assert!(weight >= bridge.config().weight_floor());
        }
    }

    // ---- Interaction Stats ----

    #[test]
    fn test_stats_initial() {
        let bridge = make_bridge();
        let stats = bridge.interaction_stats();
        assert_eq!(stats.total_interactions, 0);
        assert_eq!(stats.total_success, 0);
        assert_eq!(stats.total_failure, 0);
        assert_eq!(stats.pathway_count, 0);
        assert!(stats.avg_weight.abs() < f64::EPSILON);
        assert!(stats.success_rate.abs() < f64::EPSILON);
    }

    #[test]
    fn test_stats_after_interactions() {
        let bridge = make_bridge();
        bridge.record_interaction("a", "b", true).ok();
        bridge.record_interaction("a", "b", false).ok();
        bridge.record_interaction("c", "d", true).ok();
        let stats = bridge.interaction_stats();
        assert_eq!(stats.total_interactions, 3);
        assert_eq!(stats.total_success, 2);
        assert_eq!(stats.total_failure, 1);
        assert_eq!(stats.pathway_count, 2);
        assert!(stats.avg_weight > 0.0);
        // success_rate = 2/3 ≈ 0.6667
        assert!((stats.success_rate - 2.0 / 3.0).abs() < 1e-10);
    }

    #[test]
    fn test_stats_clone() {
        let stats = InteractionStats {
            total_interactions: 10,
            total_success: 7,
            total_failure: 3,
            pathway_count: 2,
            avg_weight: 0.5,
            success_rate: 0.7,
        };
        let cloned = stats.clone();
        assert_eq!(cloned.total_interactions, 10);
    }

    // ---- Success Rate ----

    #[test]
    fn test_pathway_success_rate_all_success() {
        let bridge = make_bridge();
        for _ in 0..5 {
            bridge.record_interaction("a", "b", true).ok();
        }
        let pathways = bridge.strongest_pathways(1);
        assert!((pathways[0].success_rate() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_pathway_success_rate_all_failure() {
        let bridge = make_bridge();
        for _ in 0..5 {
            bridge.record_interaction("a", "b", false).ok();
        }
        let pathways = bridge.strongest_pathways(1);
        assert!(pathways[0].success_rate().abs() < f64::EPSILON);
    }

    #[test]
    fn test_pathway_success_rate_mixed() {
        let bridge = make_bridge();
        bridge.record_interaction("a", "b", true).ok();
        bridge.record_interaction("a", "b", false).ok();
        bridge.record_interaction("a", "b", true).ok();
        let pathways = bridge.strongest_pathways(1);
        // 2 success out of 3 total = 0.6667
        assert!((pathways[0].success_rate() - 2.0 / 3.0).abs() < 1e-10);
    }

    // ---- Max Pathways ----

    #[test]
    fn test_max_pathways_enforced() {
        let bridge = make_bridge_with_config(0.05, 0.02, 0.001, 0.01, 1.0, 3);
        bridge.record_interaction("a", "b", true).ok();
        bridge.record_interaction("c", "d", true).ok();
        bridge.record_interaction("e", "f", true).ok();
        assert!(bridge.record_interaction("g", "h", true).is_err());
        assert_eq!(bridge.pathway_count(), 3);
    }

    #[test]
    fn test_max_pathways_allows_existing() {
        let bridge = make_bridge_with_config(0.05, 0.02, 0.001, 0.01, 1.0, 2);
        bridge.record_interaction("a", "b", true).ok();
        bridge.record_interaction("c", "d", true).ok();
        // Existing pathway should still be updated.
        assert!(bridge.record_interaction("a", "b", true).is_ok());
    }

    // ---- Reset ----

    #[test]
    fn test_reset_clears_everything() {
        let bridge = make_bridge();
        bridge.record_interaction("a", "b", true).ok();
        bridge.record_interaction("c", "d", false).ok();
        bridge.reset();
        assert_eq!(bridge.pathway_count(), 0);
        let stats = bridge.interaction_stats();
        assert_eq!(stats.total_interactions, 0);
        assert_eq!(stats.total_success, 0);
        assert_eq!(stats.total_failure, 0);
    }

    // ---- PathwayRecord Accessors ----

    #[test]
    fn test_pathway_record_accessors() {
        let bridge = make_bridge();
        bridge.record_interaction("src", "tgt", true).ok();
        let pathways = bridge.strongest_pathways(1);
        let pw = &pathways[0];
        assert_eq!(pw.source(), "src");
        assert_eq!(pw.target(), "tgt");
        assert!(pw.weight() > 0.0);
        assert_eq!(pw.co_activations(), 1);
        assert!(pw.last_interaction().ticks() > 0);
        assert!((pw.success_rate() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_pathway_record_clone() {
        let bridge = make_bridge();
        bridge.record_interaction("a", "b", true).ok();
        let pathways = bridge.strongest_pathways(1);
        let cloned = pathways[0].clone();
        assert_eq!(cloned.source(), "a");
        assert_eq!(cloned.target(), "b");
    }

    // ---- Edge Cases ----

    #[test]
    fn test_empty_service_names() {
        let bridge = make_bridge();
        assert!(bridge.record_interaction("", "", true).is_ok());
        assert_eq!(bridge.pathway_count(), 1);
        assert!(bridge.co_activation_weight("", "") > 0.0);
    }

    #[test]
    fn test_same_source_target() {
        let bridge = make_bridge();
        assert!(bridge.record_interaction("self", "self", true).is_ok());
        assert_eq!(bridge.pathway_count(), 1);
    }

    #[test]
    fn test_many_distinct_pathways() {
        let bridge = make_bridge();
        for i in 0..100 {
            let src = format!("src-{i}");
            let tgt = format!("tgt-{i}");
            bridge.record_interaction(&src, &tgt, true).ok();
        }
        assert_eq!(bridge.pathway_count(), 100);
    }

    #[test]
    fn test_interaction_updates_timestamp() {
        let bridge = make_bridge();
        bridge.record_interaction("a", "b", true).ok();
        let first = bridge.strongest_pathways(1)[0].last_interaction();
        bridge.record_interaction("a", "b", true).ok();
        let second = bridge.strongest_pathways(1)[0].last_interaction();
        assert!(second > first);
    }

    #[test]
    fn test_co_activation_count_increments() {
        let bridge = make_bridge();
        for i in 0..7 {
            bridge.record_interaction("a", "b", i % 2 == 0).ok();
        }
        let pathways = bridge.strongest_pathways(1);
        assert_eq!(pathways[0].co_activations(), 7);
    }

    #[test]
    fn test_mixed_success_failure_weight() {
        let bridge = make_bridge();
        // 3 successes: weight = 0.01 + 3*0.05 = 0.16
        for _ in 0..3 {
            bridge.record_interaction("a", "b", true).ok();
        }
        // 2 failures: weight = 0.16 - 2*0.02 = 0.12
        for _ in 0..2 {
            bridge.record_interaction("a", "b", false).ok();
        }
        let weight = bridge.co_activation_weight("a", "b");
        assert!((weight - 0.12).abs() < f64::EPSILON);
    }

    #[test]
    fn test_stats_avg_weight_accuracy() {
        let bridge = make_bridge();
        // Create two pathways with different weights.
        bridge.record_interaction("a", "b", true).ok(); // 0.01 + 0.05 = 0.06
        bridge.record_interaction("c", "d", true).ok();
        bridge.record_interaction("c", "d", true).ok(); // 0.01 + 2*0.05 = 0.11
        let stats = bridge.interaction_stats();
        // avg = (0.06 + 0.11) / 2 = 0.085
        assert!((stats.avg_weight - 0.085).abs() < 1e-10);
    }

    #[test]
    fn test_bridge_config_accessor() {
        let cfg = StdpBridgeConfig::new(0.1, 0.03, 0.005, 0.02, 0.95, 5000);
        let bridge = StdpBridgeCore::new(cfg);
        assert!((bridge.config().co_activation_delta() - 0.1).abs() < f64::EPSILON);
        assert!((bridge.config().failure_penalty() - 0.03).abs() < f64::EPSILON);
    }

    #[test]
    fn test_decay_factor_correctness() {
        let bridge = make_bridge_with_config(0.05, 0.02, 0.1, 0.001, 1.0, 10_000);
        // Build a strong pathway.
        for _ in 0..20 {
            bridge.record_interaction("a", "b", true).ok();
        }
        let before = bridge.co_activation_weight("a", "b");
        bridge.apply_decay();
        let after = bridge.co_activation_weight("a", "b");
        // after = before * (1 - 0.1) = before * 0.9
        assert!((after - before * 0.9).abs() < 1e-10);
    }

    #[test]
    fn test_interaction_stats_success_rate() {
        let bridge = StdpBridgeCore::default();
        bridge.record_interaction("a", "b", true).ok();
        bridge.record_interaction("a", "b", true).ok();
        bridge.record_interaction("a", "b", false).ok();
        let stats = bridge.interaction_stats();
        assert!((stats.success_rate - 2.0 / 3.0).abs() < 1e-10);
    }

    #[test]
    fn test_pathway_record_fields_after_mixed() {
        let bridge = StdpBridgeCore::default();
        bridge.record_interaction("x", "y", true).ok();
        bridge.record_interaction("x", "y", false).ok();
        let pathways = bridge.strongest_pathways(10);
        assert_eq!(pathways.len(), 1);
        assert_eq!(pathways[0].co_activations, 2);
        assert!((pathways[0].success_rate - 0.5).abs() < f64::EPSILON);
    }
}
