//! # N03: Regime Manager
//!
//! K-regime detection for the Kuramoto coupling parameter. Classifies the
//! current system coordination mode into one of three regimes:
//!
//! - **Swarm** (K < 1.0): Independent parallel agents with minimal coupling
//! - **Fleet** (1.0 <= K < 2.0): Coordinated agents with moderate coupling
//! - **Armada** (K >= 2.0): Fully synchronized convergence with strong coupling
//!
//! ## Layer: L8 (Nexus Integration)
//! ## Module: N03
//! ## Dependencies: L1 (Error, Timestamp)
//!
//! ## Design Constraints
//!
//! - C2: All trait methods are `&self` (interior mutability via `RwLock`)
//! - C4: Zero `unsafe`, `unwrap`, `expect`
//! - C7: Owned returns through `RwLock` (never return references)
//!
//! ## Key Types
//!
//! | Type | Purpose |
//! |------|---------|
//! | [`KRegime`] | Enum representing the three coordination regimes |
//! | [`RegimeTransition`] | Record of a regime change event |
//! | [`RegimeHealth`] | Aggregated health summary with stability metric |
//! | [`RegimeManagerConfig`] | Configuration for history capacity |
//! | [`RegimeManagerCore`] | Production implementation of [`RegimeManager`] |
//!
//! ## Related Documentation
//! - [Nexus Specs](../../ai_specs/nexus-specs/)

use std::collections::VecDeque;
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};

use parking_lot::RwLock;

use crate::m1_foundation::shared_types::Timestamp;
use crate::{Error, Result};

// ============================================================================
// Constants
// ============================================================================

/// Coupling threshold below which the system operates in Swarm mode.
/// K < 1.0 = Swarm (independent parallel).
pub const K_SWARM_THRESHOLD: f64 = 1.0;

/// Coupling threshold at or above which the system operates in Armada mode.
/// K >= 2.0 = Armada (synchronized convergence).
pub const K_ARMADA_THRESHOLD: f64 = 2.0;

// ============================================================================
// KRegime
// ============================================================================

/// Kuramoto coupling regime classification.
///
/// Determines the coordination mode of the system based on the coupling
/// parameter K:
///
/// | Regime | K Range | Description |
/// |--------|---------|-------------|
/// | Swarm | K < 1.0 | Independent parallel agents |
/// | Fleet | 1.0 <= K < 2.0 | Coordinated agents |
/// | Armada | K >= 2.0 | Synchronized convergence |
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum KRegime {
    /// Independent parallel operation (K < 1.0)
    Swarm,
    /// Coordinated operation (1.0 <= K < 2.0)
    Fleet,
    /// Synchronized convergence (K >= 2.0)
    Armada,
}

impl fmt::Display for KRegime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Swarm => write!(f, "Swarm"),
            Self::Fleet => write!(f, "Fleet"),
            Self::Armada => write!(f, "Armada"),
        }
    }
}

impl KRegime {
    /// Return the array index for this regime (used for tick tracking).
    const fn index(self) -> usize {
        match self {
            Self::Swarm => 0,
            Self::Fleet => 1,
            Self::Armada => 2,
        }
    }
}

// ============================================================================
// RegimeTransition
// ============================================================================

/// Record of a regime change event.
///
/// Created by [`RegimeManager::update_k`] when the coupling parameter
/// crosses a regime boundary.
#[derive(Clone, Debug)]
pub struct RegimeTransition {
    /// Previous regime before the transition
    pub from: KRegime,
    /// New regime after the transition
    pub to: KRegime,
    /// The K value that caused the transition
    pub k_value: f64,
    /// Monotonic timestamp of the transition
    pub timestamp: Timestamp,
    /// Tick counter at transition time
    pub tick: u64,
}

// ============================================================================
// RegimeHealth
// ============================================================================

/// Aggregated health summary of the regime manager.
///
/// Includes time spent in each regime, total transitions, and a stability
/// metric where fewer transitions indicate greater stability.
#[derive(Clone, Debug)]
pub struct RegimeHealth {
    /// Current active regime
    pub current_regime: KRegime,
    /// Current coupling parameter K
    pub current_k: f64,
    /// Total number of regime transitions
    pub transitions_total: u64,
    /// Ticks spent in Swarm regime
    pub time_in_swarm: u64,
    /// Ticks spent in Fleet regime
    pub time_in_fleet: u64,
    /// Ticks spent in Armada regime
    pub time_in_armada: u64,
    /// Stability metric: `1.0 / (1.0 + transitions_total / 100.0)`, clamped to `[0, 1]`
    pub stability: f64,
}

// ============================================================================
// RegimeManagerConfig
// ============================================================================

/// Configuration for the Regime Manager.
///
/// Controls the maximum number of regime transitions retained in history.
#[derive(Clone, Debug)]
pub struct RegimeManagerConfig {
    /// Maximum number of transitions to retain in history
    pub history_capacity: usize,
}

impl Default for RegimeManagerConfig {
    fn default() -> Self {
        Self {
            history_capacity: 100,
        }
    }
}

// ============================================================================
// RegimeManager (trait)
// ============================================================================

/// K-regime detection and tracking.
///
/// All methods are `&self` (C2) with interior mutability via `RwLock`.
/// Methods returning data through `RwLock` return owned types (C7).
pub trait RegimeManager: Send + Sync + fmt::Debug {
    /// Return the current active regime.
    fn current_regime(&self) -> KRegime;

    /// Classify a K value into a regime without updating state.
    fn detect_regime(&self, k: f64) -> KRegime;

    /// Update the coupling parameter K and return any regime transition.
    ///
    /// If the new K value places the system in a different regime, a
    /// [`RegimeTransition`] is recorded and returned inside `Some`.
    /// If no transition occurs, `None` is returned inside `Ok`.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if K is NaN or infinite.
    fn update_k(&self, k: f64) -> Result<Option<RegimeTransition>>;

    /// Return the most recent regime transitions, up to `limit`.
    fn regime_history(&self, limit: usize) -> Vec<RegimeTransition>;

    /// Return the total number of regime transitions recorded.
    fn transition_count(&self) -> usize;

    /// Return the number of ticks spent in the given regime.
    fn time_in_regime(&self, regime: KRegime) -> u64;

    /// Compute and return the current regime health summary.
    fn regime_health(&self) -> RegimeHealth;

    /// Reset all internal state to defaults.
    fn reset(&self);
}

// ============================================================================
// RegimeManagerCore (implementation)
// ============================================================================

/// Production implementation of [`RegimeManager`].
///
/// Uses `parking_lot::RwLock` for interior mutability and `AtomicU64` for
/// lock-free counters. Maintains a bounded ring buffer for transition history
/// and per-regime tick counters.
pub struct RegimeManagerCore {
    /// Current regime and K value
    current: RwLock<(KRegime, f64)>,
    /// Ring buffer of regime transitions
    transitions: RwLock<VecDeque<RegimeTransition>>,
    /// Ticks spent in each regime: [Swarm, Fleet, Armada]
    ticks_per_regime: RwLock<[u64; 3]>,
    /// Configuration
    config: RegimeManagerConfig,
    /// Monotonic tick counter
    tick_counter: AtomicU64,
}

impl fmt::Debug for RegimeManagerCore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (regime, k) = *self.current.read();
        f.debug_struct("RegimeManagerCore")
            .field("current_regime", &regime)
            .field("current_k", &k)
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl RegimeManagerCore {
    /// Create a new `RegimeManagerCore` with default configuration.
    ///
    /// Initial regime is Swarm with K = 0.0.
    #[must_use]
    pub fn new() -> Self {
        Self::with_config(RegimeManagerConfig::default())
    }

    /// Create a new `RegimeManagerCore` with the given configuration.
    ///
    /// Initial regime is Swarm with K = 0.0.
    #[must_use]
    pub fn with_config(config: RegimeManagerConfig) -> Self {
        Self {
            current: RwLock::new((KRegime::Swarm, 0.0)),
            transitions: RwLock::new(VecDeque::with_capacity(config.history_capacity)),
            ticks_per_regime: RwLock::new([0u64; 3]),
            config,
            tick_counter: AtomicU64::new(0),
        }
    }

    /// Advance the internal tick counter and return the new value.
    fn next_tick(&self) -> u64 {
        self.tick_counter.fetch_add(1, Ordering::Relaxed)
    }

    /// Classify a K value into a regime (pure function).
    fn classify(k: f64) -> KRegime {
        if k < K_SWARM_THRESHOLD {
            KRegime::Swarm
        } else if k < K_ARMADA_THRESHOLD {
            KRegime::Fleet
        } else {
            KRegime::Armada
        }
    }

    /// Compute stability from transition count.
    /// Formula: `1.0 / (1.0 + transitions_total / 100.0)`, clamped to `[0, 1]`.
    #[allow(clippy::cast_precision_loss)] // transition counts are bounded by history capacity
    fn compute_stability(transitions_total: u64) -> f64 {
        let raw = 1.0 / (1.0 + transitions_total as f64 / 100.0);
        raw.clamp(0.0, 1.0)
    }
}

impl Default for RegimeManagerCore {
    fn default() -> Self {
        Self::new()
    }
}

impl RegimeManager for RegimeManagerCore {
    fn current_regime(&self) -> KRegime {
        self.current.read().0
    }

    fn detect_regime(&self, k: f64) -> KRegime {
        Self::classify(k)
    }

    fn update_k(&self, k: f64) -> Result<Option<RegimeTransition>> {
        if k.is_nan() || k.is_infinite() {
            return Err(Error::Validation(format!(
                "K must be a finite number, got {k}"
            )));
        }

        let new_regime = Self::classify(k);
        let tick = self.next_tick();

        // Increment tick counter for the current regime before potential transition
        {
            let current = self.current.read();
            let mut ticks = self.ticks_per_regime.write();
            ticks[current.0.index()] += 1;
        }

        let mut current = self.current.write();
        let old_regime = current.0;
        current.1 = k;

        if new_regime == old_regime {
            drop(current);
            Ok(None)
        } else {
            current.0 = new_regime;

            let transition = RegimeTransition {
                from: old_regime,
                to: new_regime,
                k_value: k,
                timestamp: Timestamp::now(),
                tick,
            };

            drop(current);

            {
                let mut transitions = self.transitions.write();
                if transitions.len() >= self.config.history_capacity {
                    transitions.pop_front();
                }
                transitions.push_back(transition.clone());
            }

            Ok(Some(transition))
        }
    }

    fn regime_history(&self, limit: usize) -> Vec<RegimeTransition> {
        let transitions = self.transitions.read();
        let start = transitions.len().saturating_sub(limit);
        transitions.iter().skip(start).cloned().collect()
    }

    fn transition_count(&self) -> usize {
        self.transitions.read().len()
    }

    fn time_in_regime(&self, regime: KRegime) -> u64 {
        let ticks = self.ticks_per_regime.read();
        ticks[regime.index()]
    }

    #[allow(clippy::cast_possible_truncation)] // bounded by history_capacity (max 100)
    fn regime_health(&self) -> RegimeHealth {
        let (current_regime, current_k) = *self.current.read();
        let ticks = *self.ticks_per_regime.read();
        let transitions_total = self.transitions.read().len() as u64;
        let stability = Self::compute_stability(transitions_total);

        RegimeHealth {
            current_regime,
            current_k,
            transitions_total,
            time_in_swarm: ticks[KRegime::Swarm.index()],
            time_in_fleet: ticks[KRegime::Fleet.index()],
            time_in_armada: ticks[KRegime::Armada.index()],
            stability,
        }
    }

    fn reset(&self) {
        *self.current.write() = (KRegime::Swarm, 0.0);
        self.transitions.write().clear();
        *self.ticks_per_regime.write() = [0u64; 3];
        self.tick_counter.store(0, Ordering::Relaxed);
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // --- KRegime tests ---

    #[test]
    fn test_regime_display_swarm() {
        assert_eq!(format!("{}", KRegime::Swarm), "Swarm");
    }

    #[test]
    fn test_regime_display_fleet() {
        assert_eq!(format!("{}", KRegime::Fleet), "Fleet");
    }

    #[test]
    fn test_regime_display_armada() {
        assert_eq!(format!("{}", KRegime::Armada), "Armada");
    }

    #[test]
    fn test_regime_clone() {
        let regime = KRegime::Fleet;
        let clone = regime;
        assert_eq!(regime, clone);
    }

    #[test]
    fn test_regime_copy() {
        let regime = KRegime::Armada;
        let copy = regime;
        assert_eq!(regime, copy);
    }

    #[test]
    fn test_regime_debug() {
        let debug = format!("{:?}", KRegime::Swarm);
        assert_eq!(debug, "Swarm");
    }

    #[test]
    fn test_regime_equality() {
        assert_eq!(KRegime::Swarm, KRegime::Swarm);
        assert_eq!(KRegime::Fleet, KRegime::Fleet);
        assert_eq!(KRegime::Armada, KRegime::Armada);
    }

    #[test]
    fn test_regime_inequality() {
        assert_ne!(KRegime::Swarm, KRegime::Fleet);
        assert_ne!(KRegime::Fleet, KRegime::Armada);
        assert_ne!(KRegime::Swarm, KRegime::Armada);
    }

    #[test]
    fn test_regime_index_values() {
        assert_eq!(KRegime::Swarm.index(), 0);
        assert_eq!(KRegime::Fleet.index(), 1);
        assert_eq!(KRegime::Armada.index(), 2);
    }

    // --- Constants tests ---

    #[test]
    fn test_swarm_threshold() {
        assert!((K_SWARM_THRESHOLD - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_armada_threshold() {
        assert!((K_ARMADA_THRESHOLD - 2.0).abs() < f64::EPSILON);
    }

    // --- RegimeManagerConfig tests ---

    #[test]
    fn test_config_defaults() {
        let config = RegimeManagerConfig::default();
        assert_eq!(config.history_capacity, 100);
    }

    #[test]
    fn test_config_custom() {
        let config = RegimeManagerConfig {
            history_capacity: 50,
        };
        assert_eq!(config.history_capacity, 50);
    }

    #[test]
    fn test_config_clone() {
        let config = RegimeManagerConfig::default();
        let clone = config.clone();
        assert_eq!(clone.history_capacity, config.history_capacity);
    }

    #[test]
    fn test_config_debug() {
        let config = RegimeManagerConfig::default();
        let debug = format!("{config:?}");
        assert!(debug.contains("RegimeManagerConfig"));
    }

    // --- detect_regime / classify tests ---

    #[test]
    fn test_detect_swarm_zero() {
        let core = RegimeManagerCore::new();
        assert_eq!(core.detect_regime(0.0), KRegime::Swarm);
    }

    #[test]
    fn test_detect_swarm_negative() {
        let core = RegimeManagerCore::new();
        assert_eq!(core.detect_regime(-1.0), KRegime::Swarm);
    }

    #[test]
    fn test_detect_swarm_just_below() {
        let core = RegimeManagerCore::new();
        assert_eq!(core.detect_regime(0.999), KRegime::Swarm);
    }

    #[test]
    fn test_detect_fleet_at_boundary() {
        let core = RegimeManagerCore::new();
        assert_eq!(core.detect_regime(1.0), KRegime::Fleet);
    }

    #[test]
    fn test_detect_fleet_mid() {
        let core = RegimeManagerCore::new();
        assert_eq!(core.detect_regime(1.5), KRegime::Fleet);
    }

    #[test]
    fn test_detect_fleet_just_below_armada() {
        let core = RegimeManagerCore::new();
        assert_eq!(core.detect_regime(1.999), KRegime::Fleet);
    }

    #[test]
    fn test_detect_armada_at_boundary() {
        let core = RegimeManagerCore::new();
        assert_eq!(core.detect_regime(2.0), KRegime::Armada);
    }

    #[test]
    fn test_detect_armada_high() {
        let core = RegimeManagerCore::new();
        assert_eq!(core.detect_regime(10.0), KRegime::Armada);
    }

    // --- RegimeManagerCore construction tests ---

    #[test]
    fn test_core_new() {
        let core = RegimeManagerCore::new();
        assert_eq!(core.current_regime(), KRegime::Swarm);
        assert_eq!(core.transition_count(), 0);
    }

    #[test]
    fn test_core_default() {
        let core = RegimeManagerCore::default();
        assert_eq!(core.current_regime(), KRegime::Swarm);
    }

    #[test]
    fn test_core_with_config() {
        let config = RegimeManagerConfig {
            history_capacity: 10,
        };
        let core = RegimeManagerCore::with_config(config);
        assert_eq!(core.current_regime(), KRegime::Swarm);
    }

    #[test]
    fn test_core_debug() {
        let core = RegimeManagerCore::new();
        let debug = format!("{core:?}");
        assert!(debug.contains("RegimeManagerCore"));
        assert!(debug.contains("Swarm"));
    }

    // --- update_k tests ---

    #[test]
    fn test_update_k_no_transition() {
        let core = RegimeManagerCore::new();
        let result = core.update_k(0.5);
        assert!(result.is_ok());
        if let Ok(transition) = result {
            assert!(transition.is_none());
        }
    }

    #[test]
    fn test_update_k_swarm_to_fleet() {
        let core = RegimeManagerCore::new();
        let result = core.update_k(1.5);
        assert!(result.is_ok());
        if let Ok(Some(t)) = result {
            assert_eq!(t.from, KRegime::Swarm);
            assert_eq!(t.to, KRegime::Fleet);
            assert!((t.k_value - 1.5).abs() < f64::EPSILON);
        }
        assert_eq!(core.current_regime(), KRegime::Fleet);
    }

    #[test]
    fn test_update_k_swarm_to_armada() {
        let core = RegimeManagerCore::new();
        let result = core.update_k(3.0);
        assert!(result.is_ok());
        if let Ok(Some(t)) = result {
            assert_eq!(t.from, KRegime::Swarm);
            assert_eq!(t.to, KRegime::Armada);
        }
        assert_eq!(core.current_regime(), KRegime::Armada);
    }

    #[test]
    fn test_update_k_fleet_to_armada() {
        let core = RegimeManagerCore::new();
        let _ = core.update_k(1.5); // Swarm -> Fleet
        let result = core.update_k(2.5);
        assert!(result.is_ok());
        if let Ok(Some(t)) = result {
            assert_eq!(t.from, KRegime::Fleet);
            assert_eq!(t.to, KRegime::Armada);
        }
    }

    #[test]
    fn test_update_k_armada_to_fleet() {
        let core = RegimeManagerCore::new();
        let _ = core.update_k(3.0); // Swarm -> Armada
        let result = core.update_k(1.5);
        assert!(result.is_ok());
        if let Ok(Some(t)) = result {
            assert_eq!(t.from, KRegime::Armada);
            assert_eq!(t.to, KRegime::Fleet);
        }
    }

    #[test]
    fn test_update_k_armada_to_swarm() {
        let core = RegimeManagerCore::new();
        let _ = core.update_k(3.0); // Swarm -> Armada
        let result = core.update_k(0.5);
        assert!(result.is_ok());
        if let Ok(Some(t)) = result {
            assert_eq!(t.from, KRegime::Armada);
            assert_eq!(t.to, KRegime::Swarm);
        }
    }

    #[test]
    fn test_update_k_fleet_to_swarm() {
        let core = RegimeManagerCore::new();
        let _ = core.update_k(1.5); // Swarm -> Fleet
        let result = core.update_k(0.5);
        assert!(result.is_ok());
        if let Ok(Some(t)) = result {
            assert_eq!(t.from, KRegime::Fleet);
            assert_eq!(t.to, KRegime::Swarm);
        }
    }

    #[test]
    fn test_update_k_nan_rejected() {
        let core = RegimeManagerCore::new();
        let result = core.update_k(f64::NAN);
        assert!(result.is_err());
    }

    #[test]
    fn test_update_k_positive_infinity_rejected() {
        let core = RegimeManagerCore::new();
        let result = core.update_k(f64::INFINITY);
        assert!(result.is_err());
    }

    #[test]
    fn test_update_k_negative_infinity_rejected() {
        let core = RegimeManagerCore::new();
        let result = core.update_k(f64::NEG_INFINITY);
        assert!(result.is_err());
    }

    #[test]
    fn test_update_k_at_swarm_boundary() {
        let core = RegimeManagerCore::new();
        let result = core.update_k(1.0);
        assert!(result.is_ok());
        // K=1.0 is Fleet, so transition from Swarm
        if let Ok(Some(t)) = result {
            assert_eq!(t.to, KRegime::Fleet);
        }
    }

    #[test]
    fn test_update_k_at_armada_boundary() {
        let core = RegimeManagerCore::new();
        let _ = core.update_k(1.5); // -> Fleet
        let result = core.update_k(2.0);
        assert!(result.is_ok());
        if let Ok(Some(t)) = result {
            assert_eq!(t.to, KRegime::Armada);
        }
    }

    // --- transition_count tests ---

    #[test]
    fn test_transition_count_initial() {
        let core = RegimeManagerCore::new();
        assert_eq!(core.transition_count(), 0);
    }

    #[test]
    fn test_transition_count_increments() {
        let core = RegimeManagerCore::new();
        let _ = core.update_k(1.5); // Swarm -> Fleet
        assert_eq!(core.transition_count(), 1);
        let _ = core.update_k(0.5); // Fleet -> Swarm
        assert_eq!(core.transition_count(), 2);
    }

    #[test]
    fn test_transition_count_no_change_same_regime() {
        let core = RegimeManagerCore::new();
        let _ = core.update_k(0.3);
        let _ = core.update_k(0.5);
        let _ = core.update_k(0.7);
        assert_eq!(core.transition_count(), 0);
    }

    // --- regime_history tests ---

    #[test]
    fn test_regime_history_empty() {
        let core = RegimeManagerCore::new();
        let history = core.regime_history(10);
        assert!(history.is_empty());
    }

    #[test]
    fn test_regime_history_returns_transitions() {
        let core = RegimeManagerCore::new();
        let _ = core.update_k(1.5); // Swarm -> Fleet
        let _ = core.update_k(0.5); // Fleet -> Swarm
        let history = core.regime_history(10);
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].from, KRegime::Swarm);
        assert_eq!(history[0].to, KRegime::Fleet);
        assert_eq!(history[1].from, KRegime::Fleet);
        assert_eq!(history[1].to, KRegime::Swarm);
    }

    #[test]
    fn test_regime_history_limit() {
        let core = RegimeManagerCore::new();
        // Create multiple transitions
        let _ = core.update_k(1.5); // Swarm -> Fleet
        let _ = core.update_k(0.5); // Fleet -> Swarm
        let _ = core.update_k(2.5); // Swarm -> Armada
        let history = core.regime_history(2);
        assert_eq!(history.len(), 2);
        // Should be the most recent 2
        assert_eq!(history[0].from, KRegime::Fleet);
        assert_eq!(history[1].from, KRegime::Swarm);
    }

    #[test]
    fn test_regime_history_capacity_bounded() {
        let config = RegimeManagerConfig {
            history_capacity: 3,
        };
        let core = RegimeManagerCore::with_config(config);
        // Create more transitions than capacity
        for i in 0..10u32 {
            let k = if i % 2 == 0 { 1.5 } else { 0.5 };
            let _ = core.update_k(k);
        }
        assert!(core.transition_count() <= 3);
    }

    // --- time_in_regime tests ---

    #[test]
    fn test_time_in_regime_initial() {
        let core = RegimeManagerCore::new();
        assert_eq!(core.time_in_regime(KRegime::Swarm), 0);
        assert_eq!(core.time_in_regime(KRegime::Fleet), 0);
        assert_eq!(core.time_in_regime(KRegime::Armada), 0);
    }

    #[test]
    fn test_time_in_regime_increments_on_update() {
        let core = RegimeManagerCore::new();
        // Stay in Swarm for 3 updates
        let _ = core.update_k(0.3);
        let _ = core.update_k(0.5);
        let _ = core.update_k(0.7);
        assert_eq!(core.time_in_regime(KRegime::Swarm), 3);
    }

    #[test]
    fn test_time_in_regime_tracks_transitions() {
        let core = RegimeManagerCore::new();
        // Each update_k ticks the current regime BEFORE checking transition.
        // update_k(0.3): ticks Swarm, stays Swarm (Swarm=1)
        let _ = core.update_k(0.3);
        // update_k(0.5): ticks Swarm, stays Swarm (Swarm=2)
        let _ = core.update_k(0.5);
        // update_k(1.5): ticks Swarm (was Swarm), transitions to Fleet (Swarm=3)
        let _ = core.update_k(1.5);
        // update_k(1.7): ticks Fleet, stays Fleet (Fleet=1)
        let _ = core.update_k(1.7);
        // update_k(1.9): ticks Fleet, stays Fleet (Fleet=2)
        let _ = core.update_k(1.9);
        assert_eq!(core.time_in_regime(KRegime::Swarm), 3);
        assert_eq!(core.time_in_regime(KRegime::Fleet), 2);
    }

    // --- regime_health tests ---

    #[test]
    fn test_regime_health_initial() {
        let core = RegimeManagerCore::new();
        let health = core.regime_health();
        assert_eq!(health.current_regime, KRegime::Swarm);
        assert!((health.current_k).abs() < f64::EPSILON);
        assert_eq!(health.transitions_total, 0);
        assert!((health.stability - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_regime_health_after_transitions() {
        let core = RegimeManagerCore::new();
        let _ = core.update_k(1.5); // Swarm -> Fleet
        let health = core.regime_health();
        assert_eq!(health.current_regime, KRegime::Fleet);
        assert!((health.current_k - 1.5).abs() < f64::EPSILON);
        assert_eq!(health.transitions_total, 1);
    }

    #[test]
    fn test_regime_health_stability_decreases() {
        let core = RegimeManagerCore::new();
        let health_before = core.regime_health();

        // Create transitions
        let _ = core.update_k(1.5);
        let _ = core.update_k(0.5);
        let _ = core.update_k(2.5);

        let health_after = core.regime_health();
        assert!(health_after.stability < health_before.stability);
    }

    #[test]
    fn test_regime_health_stability_formula() {
        let core = RegimeManagerCore::new();
        // 0 transitions -> stability = 1.0
        assert!((core.regime_health().stability - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_regime_health_time_tracking() {
        let core = RegimeManagerCore::new();
        let _ = core.update_k(0.5); // Swarm tick
        let _ = core.update_k(1.5); // Swarm tick + transition
        let _ = core.update_k(1.7); // Fleet tick

        let health = core.regime_health();
        assert_eq!(health.time_in_swarm, 2);
        assert_eq!(health.time_in_fleet, 1);
        assert_eq!(health.time_in_armada, 0);
    }

    // --- compute_stability tests ---

    #[test]
    fn test_compute_stability_zero_transitions() {
        let stability = RegimeManagerCore::compute_stability(0);
        assert!((stability - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_compute_stability_100_transitions() {
        let stability = RegimeManagerCore::compute_stability(100);
        assert!((stability - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_compute_stability_large_value() {
        let stability = RegimeManagerCore::compute_stability(10_000);
        assert!(stability > 0.0);
        assert!(stability < 0.02);
    }

    #[test]
    fn test_compute_stability_clamped() {
        let stability = RegimeManagerCore::compute_stability(u64::MAX);
        assert!(stability >= 0.0);
        assert!(stability <= 1.0);
    }

    // --- reset tests ---

    #[test]
    fn test_reset_clears_state() {
        let core = RegimeManagerCore::new();
        let _ = core.update_k(1.5);
        let _ = core.update_k(0.5);
        let _ = core.update_k(2.5);

        core.reset();

        assert_eq!(core.current_regime(), KRegime::Swarm);
        assert_eq!(core.transition_count(), 0);
        assert_eq!(core.time_in_regime(KRegime::Swarm), 0);
        assert_eq!(core.time_in_regime(KRegime::Fleet), 0);
        assert_eq!(core.time_in_regime(KRegime::Armada), 0);
    }

    #[test]
    fn test_reset_allows_reuse() {
        let core = RegimeManagerCore::new();
        let _ = core.update_k(2.5);
        core.reset();
        let result = core.update_k(1.5);
        assert!(result.is_ok());
        if let Ok(Some(t)) = result {
            assert_eq!(t.from, KRegime::Swarm);
            assert_eq!(t.to, KRegime::Fleet);
        }
    }

    #[test]
    fn test_multiple_resets() {
        let core = RegimeManagerCore::new();
        let _ = core.update_k(1.5);
        core.reset();
        let _ = core.update_k(2.5);
        core.reset();
        assert_eq!(core.current_regime(), KRegime::Swarm);
        assert_eq!(core.transition_count(), 0);
    }

    // --- RegimeTransition tests ---

    #[test]
    fn test_regime_transition_clone() {
        let t = RegimeTransition {
            from: KRegime::Swarm,
            to: KRegime::Fleet,
            k_value: 1.5,
            timestamp: Timestamp::now(),
            tick: 42,
        };
        let clone = t.clone();
        assert_eq!(clone.from, t.from);
        assert_eq!(clone.to, t.to);
        assert!((clone.k_value - t.k_value).abs() < f64::EPSILON);
    }

    #[test]
    fn test_regime_transition_debug() {
        let t = RegimeTransition {
            from: KRegime::Fleet,
            to: KRegime::Armada,
            k_value: 2.5,
            timestamp: Timestamp::now(),
            tick: 100,
        };
        let debug = format!("{t:?}");
        assert!(debug.contains("RegimeTransition"));
    }

    // --- RegimeHealth tests ---

    #[test]
    fn test_regime_health_clone() {
        let health = RegimeHealth {
            current_regime: KRegime::Fleet,
            current_k: 1.5,
            transitions_total: 3,
            time_in_swarm: 10,
            time_in_fleet: 5,
            time_in_armada: 2,
            stability: 0.97,
        };
        let clone = health.clone();
        assert_eq!(clone.current_regime, health.current_regime);
        assert!((clone.stability - health.stability).abs() < f64::EPSILON);
    }

    #[test]
    fn test_regime_health_debug() {
        let health = RegimeHealth {
            current_regime: KRegime::Armada,
            current_k: 3.0,
            transitions_total: 0,
            time_in_swarm: 0,
            time_in_fleet: 0,
            time_in_armada: 10,
            stability: 1.0,
        };
        let debug = format!("{health:?}");
        assert!(debug.contains("RegimeHealth"));
    }

    // --- trait object and Send+Sync tests ---

    #[test]
    fn test_trait_object_compatibility() {
        let core = RegimeManagerCore::new();
        let manager: &dyn RegimeManager = &core;
        assert_eq!(manager.current_regime(), KRegime::Swarm);
    }

    #[test]
    fn test_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<RegimeManagerCore>();
    }

    // --- integration / scenario tests ---

    #[test]
    fn test_full_lifecycle() {
        let core = RegimeManagerCore::new();

        // Start in Swarm
        assert_eq!(core.current_regime(), KRegime::Swarm);

        // Warm up in Swarm
        let _ = core.update_k(0.3);
        let _ = core.update_k(0.5);
        let _ = core.update_k(0.8);

        // Transition to Fleet
        let result = core.update_k(1.2);
        assert!(result.is_ok());
        assert_eq!(core.current_regime(), KRegime::Fleet);

        // Stay in Fleet
        let _ = core.update_k(1.5);
        let _ = core.update_k(1.8);

        // Transition to Armada
        let result = core.update_k(2.5);
        assert!(result.is_ok());
        assert_eq!(core.current_regime(), KRegime::Armada);

        // Back to Swarm
        let result = core.update_k(0.1);
        assert!(result.is_ok());
        assert_eq!(core.current_regime(), KRegime::Swarm);

        // Verify history
        let history = core.regime_history(10);
        assert_eq!(history.len(), 3);
        assert_eq!(core.transition_count(), 3);

        // Verify time tracking
        let health = core.regime_health();
        assert!(health.time_in_swarm > 0);
        assert!(health.time_in_fleet > 0);
        assert!(health.time_in_armada > 0);
    }

    #[test]
    fn test_rapid_transitions() {
        let core = RegimeManagerCore::new();
        let values = [0.5, 1.5, 2.5, 1.5, 0.5, 1.5, 2.5, 0.5];
        for &k in &values {
            let _ = core.update_k(k);
        }
        // Each change crosses a boundary
        assert!(core.transition_count() > 0);
        let health = core.regime_health();
        assert!(health.stability < 1.0);
    }

    #[test]
    fn test_negative_k_stays_swarm() {
        let core = RegimeManagerCore::new();
        let _ = core.update_k(-5.0);
        assert_eq!(core.current_regime(), KRegime::Swarm);
        let result = core.update_k(-10.0);
        assert!(result.is_ok());
        if let Ok(transition) = result {
            assert!(transition.is_none());
        }
    }

    #[test]
    fn test_very_large_k_is_armada() {
        let core = RegimeManagerCore::new();
        let _ = core.update_k(1_000_000.0);
        assert_eq!(core.current_regime(), KRegime::Armada);
    }

    #[test]
    fn test_regime_health_stability_with_many_transitions() {
        let core = RegimeManagerCore::new();
        for i in 0..50u32 {
            let k = if i % 2 == 0 { 1.5 } else { 0.5 };
            let _ = core.update_k(k);
        }
        let health = core.regime_health();
        assert!(health.stability < 0.7);
    }

    #[test]
    fn test_update_k_zero() {
        let core = RegimeManagerCore::new();
        let result = core.update_k(0.0);
        assert!(result.is_ok());
        assert_eq!(core.current_regime(), KRegime::Swarm);
    }
}
