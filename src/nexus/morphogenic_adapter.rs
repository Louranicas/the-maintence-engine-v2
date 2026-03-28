//! # N06: Morphogenic Adapter
//!
//! Adaptation triggers when `|r_delta| > 0.05`. The adapter monitors changes in
//! Kuramoto field coherence and recommends corrective actions to maintain healthy
//! oscillator dynamics: strengthening coupling when coherence drops, reducing
//! over-synchronization when coherence saturates, or triggering weight decay when
//! the system becomes too uniform.
//!
//! ## Layer: L8 (Nexus Integration)
//! ## Dependencies: L1 (Error, Timestamp)
//!
//! ## Decision Logic
//!
//! | Condition | Action |
//! |-----------|--------|
//! | `r_delta < -threshold` | [`AdaptationAction::IncreaseCoupling`] |
//! | `r_delta > threshold` AND `current_k > 2.0` | [`AdaptationAction::DecreaseCoupling`] |
//! | `r_delta > threshold` AND `current_k <= 2.0` | [`AdaptationAction::TriggerDecay`] |
//! | `\|r_delta\| <= threshold` | [`AdaptationAction::None`] |
//!
//! ## Design Invariants
//!
//! - All trait methods take `&self` (interior mutability via `parking_lot::RwLock`)
//! - Zero `unsafe`, `unwrap`, `expect`
//! - `Timestamp` for all temporal fields (no chrono, no `SystemTime`)
//!
//! ## Related Documentation
//! - [Nexus Specs](../../ai_specs/nexus-specs/)
//! - [Kuramoto Parameters](../../config/nexus.toml)

use std::collections::VecDeque;
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};

use parking_lot::RwLock;

use crate::m1_foundation::shared_types::Timestamp;
use crate::{Error, Result};

// ============================================================================
// Constants
// ============================================================================

/// Default threshold for `|r_delta|` below which no adaptation is triggered.
pub const DEFAULT_R_THRESHOLD: f64 = 0.05;

/// Default coupling delta applied per adaptation step.
const DEFAULT_K_COUPLING_DELTA: f64 = 0.1;

/// Default maximum number of adaptation records kept in the rolling history.
const DEFAULT_HISTORY_CAPACITY: usize = 200;

/// Coupling strength boundary between "fleet" and "armada" regimes.
/// Above this value, `DecreaseCoupling` is preferred over `TriggerDecay`.
const K_ARMADA_BOUNDARY: f64 = 2.0;

// ============================================================================
// AdaptationAction
// ============================================================================

/// A corrective action recommended by the morphogenic adapter.
///
/// Actions are determined by the sign and magnitude of `r_delta` together with
/// the current coupling strength `k`.
#[derive(Clone, Debug, PartialEq)]
pub enum AdaptationAction {
    /// Strengthen coupling to recover lost coherence (r dropping).
    IncreaseCoupling {
        /// Amount to increase K by.
        k_delta: f64,
    },
    /// Reduce coupling to prevent over-synchronization (r saturated, K high).
    DecreaseCoupling {
        /// Amount to decrease K by.
        k_delta: f64,
    },
    /// Trigger STDP weight decay to restore pathway diversity (r saturated, K low).
    TriggerDecay,
    /// Spawn a diversifier agent to inject exploration noise.
    SpawnDiversifier,
    /// No adaptation needed (`|r_delta| <= threshold`).
    None,
}

impl fmt::Display for AdaptationAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IncreaseCoupling { k_delta } => {
                write!(f, "IncreaseCoupling(+{k_delta:.4})")
            }
            Self::DecreaseCoupling { k_delta } => {
                write!(f, "DecreaseCoupling(-{k_delta:.4})")
            }
            Self::TriggerDecay => write!(f, "TriggerDecay"),
            Self::SpawnDiversifier => write!(f, "SpawnDiversifier"),
            Self::None => write!(f, "None"),
        }
    }
}

// ============================================================================
// AdaptationRecord
// ============================================================================

/// A record of a completed adaptation action stored in the rolling history.
#[derive(Clone, Debug)]
pub struct AdaptationRecord {
    /// The action that was taken.
    pub action: AdaptationAction,
    /// The `r_delta` that triggered the adaptation.
    pub r_delta: f64,
    /// The coupling strength K at the time of adaptation.
    pub k_at_time: f64,
    /// When the adaptation was recorded.
    pub timestamp: Timestamp,
}

// ============================================================================
// AdaptationStats
// ============================================================================

/// Aggregate statistics for the morphogenic adapter.
#[derive(Clone, Debug)]
pub struct AdaptationStats {
    /// Total adaptations triggered (excluding `None`).
    pub total_adaptations: u64,
    /// Number of `IncreaseCoupling` actions.
    pub increase_coupling: u64,
    /// Number of `DecreaseCoupling` actions.
    pub decrease_coupling: u64,
    /// Number of `TriggerDecay` actions.
    pub trigger_decay: u64,
    /// Number of `SpawnDiversifier` actions.
    pub spawn_diversifier: u64,
    /// Average `|r_delta|` magnitude across all adaptations.
    pub avg_r_delta_magnitude: f64,
}

// ============================================================================
// MorphogenicConfig
// ============================================================================

/// Configuration for the morphogenic adapter.
#[derive(Clone, Debug)]
pub struct MorphogenicConfig {
    /// Threshold for `|r_delta|` below which no adaptation is triggered.
    pub r_threshold: f64,
    /// Coupling delta applied per adaptation step.
    pub k_coupling_delta: f64,
    /// Maximum number of adaptation records kept in the rolling history.
    pub history_capacity: usize,
}

impl Default for MorphogenicConfig {
    fn default() -> Self {
        Self {
            r_threshold: DEFAULT_R_THRESHOLD,
            k_coupling_delta: DEFAULT_K_COUPLING_DELTA,
            history_capacity: DEFAULT_HISTORY_CAPACITY,
        }
    }
}

// ============================================================================
// MorphogenicAdapter trait
// ============================================================================

/// Trait for morphogenic adaptation based on field coherence changes.
///
/// Implementations monitor `r_delta` and recommend coupling adjustments,
/// decay triggers, or diversifier spawns to maintain healthy Kuramoto dynamics.
pub trait MorphogenicAdapter: Send + Sync + fmt::Debug {
    /// Check whether an adaptation is needed given `r_delta` and `current_k`.
    ///
    /// Returns `Some(action)` with the recommended action (which may be
    /// [`AdaptationAction::None`] if within threshold), or `None` to indicate
    /// the adapter could not determine an action.
    fn check_trigger(&self, r_delta: f64, current_k: f64) -> Option<AdaptationAction>;

    /// Record that an adaptation action was applied, along with the triggering delta.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if the action is `None` (no-ops should not be recorded).
    fn record_adaptation(&self, action: &AdaptationAction, r_delta: f64) -> Result<()>;

    /// Return the most recent adaptation records, up to `limit`.
    fn adaptation_history(&self, limit: usize) -> Vec<AdaptationRecord>;

    /// Return the total number of adaptations recorded.
    fn adaptation_count(&self) -> usize;

    /// Return aggregate adaptation statistics.
    fn adaptation_stats(&self) -> AdaptationStats;

    /// Reset all state (history, counters).
    fn reset(&self);
}

// ============================================================================
// MorphogenicAdapterCore
// ============================================================================

/// Core implementation of the [`MorphogenicAdapter`] trait.
///
/// Uses `parking_lot::RwLock` for interior mutability and `AtomicU64` for
/// lock-free counter updates.
#[derive(Debug)]
pub struct MorphogenicAdapterCore {
    /// Rolling history of adaptation records.
    history: RwLock<VecDeque<AdaptationRecord>>,
    /// Configuration.
    config: MorphogenicConfig,
    /// Total adaptations (excluding `None`).
    total: AtomicU64,
    /// Count of `IncreaseCoupling` actions.
    increase_count: AtomicU64,
    /// Count of `DecreaseCoupling` actions.
    decrease_count: AtomicU64,
    /// Count of `TriggerDecay` actions.
    decay_count: AtomicU64,
    /// Count of `SpawnDiversifier` actions.
    diversifier_count: AtomicU64,
    /// Running sum of `|r_delta|` for average computation.
    r_delta_sum: RwLock<f64>,
}

impl MorphogenicAdapterCore {
    /// Create a new morphogenic adapter with the given configuration.
    #[must_use]
    pub fn new(config: MorphogenicConfig) -> Self {
        Self {
            history: RwLock::new(VecDeque::with_capacity(config.history_capacity)),
            config,
            total: AtomicU64::new(0),
            increase_count: AtomicU64::new(0),
            decrease_count: AtomicU64::new(0),
            decay_count: AtomicU64::new(0),
            diversifier_count: AtomicU64::new(0),
            r_delta_sum: RwLock::new(0.0),
        }
    }

    /// Create a new morphogenic adapter with default configuration.
    #[must_use]
    pub fn with_defaults() -> Self {
        Self::new(MorphogenicConfig::default())
    }

    /// Increment the counter for a specific action type.
    fn increment_action_counter(&self, action: &AdaptationAction) {
        match action {
            AdaptationAction::IncreaseCoupling { .. } => {
                self.increase_count.fetch_add(1, Ordering::Relaxed);
            }
            AdaptationAction::DecreaseCoupling { .. } => {
                self.decrease_count.fetch_add(1, Ordering::Relaxed);
            }
            AdaptationAction::TriggerDecay => {
                self.decay_count.fetch_add(1, Ordering::Relaxed);
            }
            AdaptationAction::SpawnDiversifier => {
                self.diversifier_count.fetch_add(1, Ordering::Relaxed);
            }
            AdaptationAction::None => {}
        }
    }
}

impl MorphogenicAdapter for MorphogenicAdapterCore {
    fn check_trigger(&self, r_delta: f64, current_k: f64) -> Option<AdaptationAction> {
        if r_delta.is_nan() || current_k.is_nan() {
            return Some(AdaptationAction::None);
        }

        if r_delta.abs() <= self.config.r_threshold {
            return Some(AdaptationAction::None);
        }

        // r dropping: coherence lost, strengthen coupling
        if r_delta < -self.config.r_threshold {
            return Some(AdaptationAction::IncreaseCoupling {
                k_delta: self.config.k_coupling_delta,
            });
        }

        // r saturated (positive spike): over-synchronization
        if r_delta > self.config.r_threshold {
            if current_k > K_ARMADA_BOUNDARY {
                return Some(AdaptationAction::DecreaseCoupling {
                    k_delta: self.config.k_coupling_delta,
                });
            }
            return Some(AdaptationAction::TriggerDecay);
        }

        Some(AdaptationAction::None)
    }

    fn record_adaptation(&self, action: &AdaptationAction, r_delta: f64) -> Result<()> {
        if matches!(action, AdaptationAction::None) {
            return Err(Error::Validation(
                "cannot record AdaptationAction::None".to_string(),
            ));
        }

        self.increment_action_counter(action);
        self.total.fetch_add(1, Ordering::Relaxed);

        {
            let mut sum = self.r_delta_sum.write();
            *sum += r_delta.abs();
        }

        let record = AdaptationRecord {
            action: action.clone(),
            r_delta,
            k_at_time: 0.0, // Caller should provide via a richer API; default for now
            timestamp: Timestamp::now(),
        };

        {
            let mut hist = self.history.write();
            if hist.len() >= self.config.history_capacity {
                hist.pop_front();
            }
            hist.push_back(record);
        }

        Ok(())
    }

    fn adaptation_history(&self, limit: usize) -> Vec<AdaptationRecord> {
        let hist = self.history.read();
        hist.iter().rev().take(limit).cloned().collect()
    }

    fn adaptation_count(&self) -> usize {
        // Truncation is acceptable: adaptation count will never exceed usize::MAX
        #[allow(clippy::cast_possible_truncation)]
        let count = self.total.load(Ordering::Relaxed) as usize;
        count
    }

    fn adaptation_stats(&self) -> AdaptationStats {
        let total = self.total.load(Ordering::Relaxed);
        let sum = { *self.r_delta_sum.read() };
        #[allow(clippy::cast_precision_loss)]
        let avg = if total > 0 {
            sum / total as f64
        } else {
            0.0
        };
        AdaptationStats {
            total_adaptations: total,
            increase_coupling: self.increase_count.load(Ordering::Relaxed),
            decrease_coupling: self.decrease_count.load(Ordering::Relaxed),
            trigger_decay: self.decay_count.load(Ordering::Relaxed),
            spawn_diversifier: self.diversifier_count.load(Ordering::Relaxed),
            avg_r_delta_magnitude: avg,
        }
    }

    fn reset(&self) {
        self.history.write().clear();
        self.total.store(0, Ordering::Relaxed);
        self.increase_count.store(0, Ordering::Relaxed);
        self.decrease_count.store(0, Ordering::Relaxed);
        self.decay_count.store(0, Ordering::Relaxed);
        self.diversifier_count.store(0, Ordering::Relaxed);
        *self.r_delta_sum.write() = 0.0;
    }
}

// ============================================================================
// Tests (50+ required)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // Helpers ---------------------------------------------------------------

    fn adapter() -> MorphogenicAdapterCore {
        MorphogenicAdapterCore::with_defaults()
    }

    fn adapter_with_threshold(threshold: f64) -> MorphogenicAdapterCore {
        MorphogenicAdapterCore::new(MorphogenicConfig {
            r_threshold: threshold,
            k_coupling_delta: DEFAULT_K_COUPLING_DELTA,
            history_capacity: DEFAULT_HISTORY_CAPACITY,
        })
    }

    // -----------------------------------------------------------------------
    // check_trigger — None (within threshold)
    // -----------------------------------------------------------------------

    #[test]
    fn check_trigger_none_when_delta_zero() {
        let a = adapter();
        let action = a.check_trigger(0.0, 1.0);
        assert_eq!(action, Some(AdaptationAction::None));
    }

    #[test]
    fn check_trigger_none_when_delta_small_positive() {
        let a = adapter();
        let action = a.check_trigger(0.03, 1.0);
        assert_eq!(action, Some(AdaptationAction::None));
    }

    #[test]
    fn check_trigger_none_when_delta_small_negative() {
        let a = adapter();
        let action = a.check_trigger(-0.03, 1.0);
        assert_eq!(action, Some(AdaptationAction::None));
    }

    #[test]
    fn check_trigger_none_when_delta_exactly_threshold() {
        let a = adapter();
        let action = a.check_trigger(0.05, 1.0);
        assert_eq!(action, Some(AdaptationAction::None));
    }

    #[test]
    fn check_trigger_none_when_delta_exactly_neg_threshold() {
        let a = adapter();
        let action = a.check_trigger(-0.05, 1.0);
        assert_eq!(action, Some(AdaptationAction::None));
    }

    // -----------------------------------------------------------------------
    // check_trigger — IncreaseCoupling (r dropping)
    // -----------------------------------------------------------------------

    #[test]
    fn check_trigger_increase_coupling_on_negative_delta() {
        let a = adapter();
        let action = a.check_trigger(-0.10, 1.5);
        assert_eq!(
            action,
            Some(AdaptationAction::IncreaseCoupling {
                k_delta: DEFAULT_K_COUPLING_DELTA
            })
        );
    }

    #[test]
    fn check_trigger_increase_coupling_large_drop() {
        let a = adapter();
        let action = a.check_trigger(-0.50, 0.5);
        assert_eq!(
            action,
            Some(AdaptationAction::IncreaseCoupling {
                k_delta: DEFAULT_K_COUPLING_DELTA
            })
        );
    }

    #[test]
    fn check_trigger_increase_coupling_just_past_threshold() {
        let a = adapter();
        // -0.051 is just past -0.05 threshold
        let action = a.check_trigger(-0.051, 1.0);
        assert!(matches!(
            action,
            Some(AdaptationAction::IncreaseCoupling { .. })
        ));
    }

    // -----------------------------------------------------------------------
    // check_trigger — DecreaseCoupling (r saturated, K high)
    // -----------------------------------------------------------------------

    #[test]
    fn check_trigger_decrease_coupling_when_k_high() {
        let a = adapter();
        let action = a.check_trigger(0.10, 2.5); // k > 2.0
        assert_eq!(
            action,
            Some(AdaptationAction::DecreaseCoupling {
                k_delta: DEFAULT_K_COUPLING_DELTA
            })
        );
    }

    #[test]
    fn check_trigger_decrease_coupling_at_armada_regime() {
        let a = adapter();
        let action = a.check_trigger(0.15, 3.0);
        assert!(matches!(
            action,
            Some(AdaptationAction::DecreaseCoupling { .. })
        ));
    }

    #[test]
    fn check_trigger_decrease_coupling_just_above_k_boundary() {
        let a = adapter();
        let action = a.check_trigger(0.10, 2.01);
        assert!(matches!(
            action,
            Some(AdaptationAction::DecreaseCoupling { .. })
        ));
    }

    // -----------------------------------------------------------------------
    // check_trigger — TriggerDecay (r saturated, K low)
    // -----------------------------------------------------------------------

    #[test]
    fn check_trigger_decay_when_k_low() {
        let a = adapter();
        let action = a.check_trigger(0.10, 1.5); // k <= 2.0
        assert_eq!(action, Some(AdaptationAction::TriggerDecay));
    }

    #[test]
    fn check_trigger_decay_at_k_boundary() {
        let a = adapter();
        let action = a.check_trigger(0.10, 2.0); // k == 2.0 (not > 2.0)
        assert_eq!(action, Some(AdaptationAction::TriggerDecay));
    }

    #[test]
    fn check_trigger_decay_at_low_k() {
        let a = adapter();
        let action = a.check_trigger(0.10, 0.5); // swarm regime
        assert_eq!(action, Some(AdaptationAction::TriggerDecay));
    }

    // -----------------------------------------------------------------------
    // check_trigger — NaN handling
    // -----------------------------------------------------------------------

    #[test]
    fn check_trigger_nan_r_delta() {
        let a = adapter();
        let action = a.check_trigger(f64::NAN, 1.0);
        assert_eq!(action, Some(AdaptationAction::None));
    }

    #[test]
    fn check_trigger_nan_current_k() {
        let a = adapter();
        let action = a.check_trigger(0.1, f64::NAN);
        assert_eq!(action, Some(AdaptationAction::None));
    }

    // -----------------------------------------------------------------------
    // check_trigger — custom threshold
    // -----------------------------------------------------------------------

    #[test]
    fn custom_threshold_affects_trigger() {
        let a = adapter_with_threshold(0.2);
        // 0.15 is below the 0.2 threshold
        let action = a.check_trigger(0.15, 1.0);
        assert_eq!(action, Some(AdaptationAction::None));
    }

    #[test]
    fn custom_threshold_triggers_above() {
        let a = adapter_with_threshold(0.02);
        // 0.03 is above the 0.02 threshold
        let action = a.check_trigger(0.03, 1.5);
        assert_eq!(action, Some(AdaptationAction::TriggerDecay));
    }

    // -----------------------------------------------------------------------
    // record_adaptation
    // -----------------------------------------------------------------------

    #[test]
    fn record_adaptation_success() {
        let a = adapter();
        let action = AdaptationAction::IncreaseCoupling { k_delta: 0.1 };
        let result = a.record_adaptation(&action, -0.1);
        assert!(result.is_ok());
    }

    #[test]
    fn record_adaptation_none_returns_error() {
        let a = adapter();
        let result = a.record_adaptation(&AdaptationAction::None, 0.0);
        assert!(result.is_err());
    }

    #[test]
    fn record_adaptation_increments_total() {
        let a = adapter();
        let action = AdaptationAction::TriggerDecay;
        let _ = a.record_adaptation(&action, 0.1);
        assert_eq!(a.adaptation_count(), 1);
    }

    #[test]
    fn record_adaptation_increments_type_counter() {
        let a = adapter();
        let _ = a.record_adaptation(&AdaptationAction::TriggerDecay, 0.1);
        let _ = a.record_adaptation(&AdaptationAction::TriggerDecay, 0.2);
        let stats = a.adaptation_stats();
        assert_eq!(stats.trigger_decay, 2);
    }

    #[test]
    fn record_adaptation_adds_to_history() {
        let a = adapter();
        let _ = a.record_adaptation(
            &AdaptationAction::IncreaseCoupling { k_delta: 0.1 },
            -0.15,
        );
        let hist = a.adaptation_history(10);
        assert_eq!(hist.len(), 1);
        assert!((hist[0].r_delta - (-0.15)).abs() < f64::EPSILON);
    }

    #[test]
    fn record_decrease_coupling() {
        let a = adapter();
        let _ = a.record_adaptation(
            &AdaptationAction::DecreaseCoupling { k_delta: 0.1 },
            0.15,
        );
        let stats = a.adaptation_stats();
        assert_eq!(stats.decrease_coupling, 1);
    }

    #[test]
    fn record_spawn_diversifier() {
        let a = adapter();
        let _ = a.record_adaptation(&AdaptationAction::SpawnDiversifier, 0.2);
        let stats = a.adaptation_stats();
        assert_eq!(stats.spawn_diversifier, 1);
    }

    // -----------------------------------------------------------------------
    // adaptation_history
    // -----------------------------------------------------------------------

    #[test]
    fn adaptation_history_empty_initially() {
        let a = adapter();
        assert!(a.adaptation_history(10).is_empty());
    }

    #[test]
    fn adaptation_history_most_recent_first() {
        let a = adapter();
        let _ = a.record_adaptation(&AdaptationAction::TriggerDecay, 0.1);
        let _ = a.record_adaptation(
            &AdaptationAction::IncreaseCoupling { k_delta: 0.1 },
            -0.2,
        );
        let hist = a.adaptation_history(10);
        assert_eq!(hist.len(), 2);
        // Most recent (IncreaseCoupling) should be first
        assert!(matches!(
            hist[0].action,
            AdaptationAction::IncreaseCoupling { .. }
        ));
        assert!(matches!(hist[1].action, AdaptationAction::TriggerDecay));
    }

    #[test]
    fn adaptation_history_respects_limit() {
        let a = adapter();
        for _ in 0..5 {
            let _ = a.record_adaptation(&AdaptationAction::TriggerDecay, 0.1);
        }
        let hist = a.adaptation_history(3);
        assert_eq!(hist.len(), 3);
    }

    #[test]
    fn adaptation_history_limit_zero() {
        let a = adapter();
        let _ = a.record_adaptation(&AdaptationAction::TriggerDecay, 0.1);
        assert!(a.adaptation_history(0).is_empty());
    }

    // -----------------------------------------------------------------------
    // adaptation_count
    // -----------------------------------------------------------------------

    #[test]
    fn adaptation_count_zero_initially() {
        let a = adapter();
        assert_eq!(a.adaptation_count(), 0);
    }

    #[test]
    fn adaptation_count_after_multiple() {
        let a = adapter();
        let _ = a.record_adaptation(&AdaptationAction::TriggerDecay, 0.1);
        let _ = a.record_adaptation(
            &AdaptationAction::IncreaseCoupling { k_delta: 0.1 },
            -0.1,
        );
        assert_eq!(a.adaptation_count(), 2);
    }

    // -----------------------------------------------------------------------
    // adaptation_stats
    // -----------------------------------------------------------------------

    #[test]
    fn stats_all_zero_initially() {
        let a = adapter();
        let stats = a.adaptation_stats();
        assert_eq!(stats.total_adaptations, 0);
        assert_eq!(stats.increase_coupling, 0);
        assert_eq!(stats.decrease_coupling, 0);
        assert_eq!(stats.trigger_decay, 0);
        assert_eq!(stats.spawn_diversifier, 0);
        assert!(stats.avg_r_delta_magnitude.abs() < f64::EPSILON);
    }

    #[test]
    fn stats_after_mixed_actions() {
        let a = adapter();
        let _ = a.record_adaptation(
            &AdaptationAction::IncreaseCoupling { k_delta: 0.1 },
            -0.1,
        );
        let _ = a.record_adaptation(
            &AdaptationAction::DecreaseCoupling { k_delta: 0.1 },
            0.15,
        );
        let _ = a.record_adaptation(&AdaptationAction::TriggerDecay, 0.2);
        let stats = a.adaptation_stats();
        assert_eq!(stats.total_adaptations, 3);
        assert_eq!(stats.increase_coupling, 1);
        assert_eq!(stats.decrease_coupling, 1);
        assert_eq!(stats.trigger_decay, 1);
    }

    #[test]
    fn stats_avg_r_delta_magnitude() {
        let a = adapter();
        let _ = a.record_adaptation(&AdaptationAction::TriggerDecay, 0.10);
        let _ = a.record_adaptation(
            &AdaptationAction::IncreaseCoupling { k_delta: 0.1 },
            -0.20,
        );
        let stats = a.adaptation_stats();
        // avg = (0.10 + 0.20) / 2 = 0.15
        assert!((stats.avg_r_delta_magnitude - 0.15).abs() < 1e-9);
    }

    // -----------------------------------------------------------------------
    // reset
    // -----------------------------------------------------------------------

    #[test]
    fn reset_clears_history() {
        let a = adapter();
        let _ = a.record_adaptation(&AdaptationAction::TriggerDecay, 0.1);
        a.reset();
        assert!(a.adaptation_history(10).is_empty());
    }

    #[test]
    fn reset_clears_counters() {
        let a = adapter();
        let _ = a.record_adaptation(&AdaptationAction::TriggerDecay, 0.1);
        a.reset();
        let stats = a.adaptation_stats();
        assert_eq!(stats.total_adaptations, 0);
        assert_eq!(stats.trigger_decay, 0);
    }

    #[test]
    fn reset_clears_avg() {
        let a = adapter();
        let _ = a.record_adaptation(&AdaptationAction::TriggerDecay, 0.1);
        a.reset();
        let stats = a.adaptation_stats();
        assert!(stats.avg_r_delta_magnitude.abs() < f64::EPSILON);
    }

    #[test]
    fn reset_allows_fresh_operations() {
        let a = adapter();
        let _ = a.record_adaptation(&AdaptationAction::TriggerDecay, 0.1);
        a.reset();
        let action = a.check_trigger(-0.1, 1.0);
        assert!(matches!(
            action,
            Some(AdaptationAction::IncreaseCoupling { .. })
        ));
        let _ = a.record_adaptation(
            &AdaptationAction::IncreaseCoupling { k_delta: 0.1 },
            -0.1,
        );
        assert_eq!(a.adaptation_count(), 1);
    }

    // -----------------------------------------------------------------------
    // history capacity
    // -----------------------------------------------------------------------

    #[test]
    fn history_respects_capacity() {
        let a = MorphogenicAdapterCore::new(MorphogenicConfig {
            r_threshold: DEFAULT_R_THRESHOLD,
            k_coupling_delta: DEFAULT_K_COUPLING_DELTA,
            history_capacity: 3,
        });
        for _ in 0..5 {
            let _ = a.record_adaptation(&AdaptationAction::TriggerDecay, 0.1);
        }
        let hist = a.adaptation_history(10);
        assert_eq!(hist.len(), 3);
    }

    // -----------------------------------------------------------------------
    // config
    // -----------------------------------------------------------------------

    #[test]
    fn default_config_values() {
        let cfg = MorphogenicConfig::default();
        assert!((cfg.r_threshold - DEFAULT_R_THRESHOLD).abs() < f64::EPSILON);
        assert!((cfg.k_coupling_delta - DEFAULT_K_COUPLING_DELTA).abs() < f64::EPSILON);
        assert_eq!(cfg.history_capacity, DEFAULT_HISTORY_CAPACITY);
    }

    #[test]
    fn config_clone() {
        let cfg = MorphogenicConfig::default();
        let cfg2 = cfg.clone();
        assert!((cfg.r_threshold - cfg2.r_threshold).abs() < f64::EPSILON);
    }

    // -----------------------------------------------------------------------
    // AdaptationAction Display
    // -----------------------------------------------------------------------

    #[test]
    fn display_increase_coupling() {
        let a = AdaptationAction::IncreaseCoupling { k_delta: 0.1 };
        let s = format!("{a}");
        assert!(s.contains("IncreaseCoupling"));
    }

    #[test]
    fn display_decrease_coupling() {
        let a = AdaptationAction::DecreaseCoupling { k_delta: 0.1 };
        let s = format!("{a}");
        assert!(s.contains("DecreaseCoupling"));
    }

    #[test]
    fn display_trigger_decay() {
        let a = AdaptationAction::TriggerDecay;
        assert_eq!(format!("{a}"), "TriggerDecay");
    }

    #[test]
    fn display_spawn_diversifier() {
        let a = AdaptationAction::SpawnDiversifier;
        assert_eq!(format!("{a}"), "SpawnDiversifier");
    }

    #[test]
    fn display_none() {
        let a = AdaptationAction::None;
        assert_eq!(format!("{a}"), "None");
    }

    // -----------------------------------------------------------------------
    // Trait object safety (Send + Sync + Debug)
    // -----------------------------------------------------------------------

    #[test]
    fn adapter_core_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<MorphogenicAdapterCore>();
    }

    #[test]
    fn adapter_core_is_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<MorphogenicAdapterCore>();
    }

    #[test]
    fn adapter_core_is_debug() {
        let a = adapter();
        let debug = format!("{a:?}");
        assert!(!debug.is_empty());
    }

    #[test]
    fn adapter_core_as_trait_object() {
        let a: Box<dyn MorphogenicAdapter> = Box::new(adapter());
        assert_eq!(a.adaptation_count(), 0);
    }

    // -----------------------------------------------------------------------
    // Edge cases
    // -----------------------------------------------------------------------

    #[test]
    fn many_adaptations() {
        let a = adapter();
        for i in 0..100 {
            #[allow(clippy::cast_precision_loss)]
            let delta = if i % 2 == 0 { -0.1 } else { 0.1 };
            if let Some(action) = a.check_trigger(delta, 1.5) {
                if !matches!(action, AdaptationAction::None) {
                    let _ = a.record_adaptation(&action, delta);
                }
            }
        }
        assert_eq!(a.adaptation_count(), 100);
    }

    #[test]
    fn adaptation_action_equality() {
        assert_eq!(AdaptationAction::TriggerDecay, AdaptationAction::TriggerDecay);
        assert_eq!(AdaptationAction::None, AdaptationAction::None);
        assert_eq!(AdaptationAction::SpawnDiversifier, AdaptationAction::SpawnDiversifier);
        assert_ne!(AdaptationAction::TriggerDecay, AdaptationAction::None);
    }

    #[test]
    fn adaptation_action_clone() {
        let a = AdaptationAction::IncreaseCoupling { k_delta: 0.1 };
        let b = a.clone();
        assert_eq!(a, b);
    }

    #[test]
    fn extreme_positive_delta() {
        let a = adapter();
        let action = a.check_trigger(1.0, 3.0);
        assert!(matches!(
            action,
            Some(AdaptationAction::DecreaseCoupling { .. })
        ));
    }

    #[test]
    fn extreme_negative_delta() {
        let a = adapter();
        let action = a.check_trigger(-1.0, 0.5);
        assert!(matches!(
            action,
            Some(AdaptationAction::IncreaseCoupling { .. })
        ));
    }

    #[test]
    fn zero_threshold_always_triggers() {
        let a = adapter_with_threshold(0.0);
        // Even tiny positive delta should trigger
        let action = a.check_trigger(0.001, 1.0);
        // With threshold 0.0, 0.001 > 0.0 → TriggerDecay (k=1.0 <= 2.0)
        assert_eq!(action, Some(AdaptationAction::TriggerDecay));
    }
}
