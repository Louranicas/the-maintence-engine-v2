//! # N01: Field Bridge
//!
//! Kuramoto r-tracking with pre/post field state capture on every L4+ operation.
//!
//! The Field Bridge provides the interface between the Maintenance Engine and the
//! Nexus Controller's Kuramoto coherence field. Every L4+ operation captures a
//! pre-state and post-state snapshot of the field, computes deltas, and triggers
//! morphogenic adaptation when `|r_delta| > adaptation_threshold`.
//!
//! ## Layer: L8 (Nexus Integration)
//! ## Module: N01
//! ## Dependencies: L1 (Error, Timestamp)
//!
//! ## Design Constraints
//!
//! - C2: All trait methods are `&self` (interior mutability via `RwLock`)
//! - C4: Zero `unsafe`, `unwrap`, `expect`
//! - C7: Owned returns through `RwLock` (never return references)
//! - C11: Every L4+ module has Nexus field capture (pre/post r)
//!
//! ## Key Types
//!
//! | Type | Purpose |
//! |------|---------|
//! | [`FieldCapture`] | Snapshot of field state at a point in time |
//! | [`FieldDelta`] | Difference between two captures |
//! | [`FieldHealth`] | Aggregated health summary of the field |
//! | [`FieldBridgeConfig`] | Configuration for history capacity and thresholds |
//! | [`FieldBridgeCore`] | Production implementation of [`FieldBridge`] |
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
// FieldCapture
// ============================================================================

/// Snapshot of the Kuramoto field state at a specific point in time.
///
/// Captures the coherence parameter `r`, coupling strength `k`, active sphere
/// count, monotonic timestamp, and tick counter. Used as the primitive for
/// pre/post field capture on every L4+ operation.
#[derive(Clone, Debug)]
pub struct FieldCapture {
    /// Kuramoto order parameter (0.0 = incoherent, 1.0 = fully synchronized)
    pub r: f64,
    /// Coupling strength K
    pub k: f64,
    /// Number of active spheres in the field
    pub spheres: u32,
    /// Monotonic timestamp of this capture
    pub timestamp: Timestamp,
    /// Tick counter at capture time
    pub tick: u64,
}

impl FieldCapture {
    /// Create a new field capture with the given parameters.
    #[must_use]
    pub const fn new(r: f64, k: f64, spheres: u32, timestamp: Timestamp, tick: u64) -> Self {
        Self {
            r,
            k,
            spheres,
            timestamp,
            tick,
        }
    }

    /// Create a zero-valued field capture (used as initial state).
    #[must_use]
    pub const fn zero() -> Self {
        Self {
            r: 0.0,
            k: 0.0,
            spheres: 0,
            timestamp: Timestamp::ZERO,
            tick: 0,
        }
    }
}

// ============================================================================
// FieldDelta
// ============================================================================

/// Difference between two field captures, representing a state transition.
///
/// Computed by [`FieldBridge::record_delta`] from a pre-capture and post-capture.
/// When `|r_delta| > adaptation_threshold`, the delta triggers morphogenic
/// adaptation and the `triggered_adaptation` flag is set.
#[derive(Clone, Debug)]
pub struct FieldDelta {
    /// Field state before the operation
    pub pre: FieldCapture,
    /// Field state after the operation
    pub post: FieldCapture,
    /// Change in coherence parameter (post.r - pre.r)
    pub r_delta: f64,
    /// Change in coupling strength (post.k - pre.k)
    pub k_delta: f64,
    /// Duration of the operation in ticks
    pub duration_ticks: u64,
    /// Whether this delta triggered morphogenic adaptation
    pub triggered_adaptation: bool,
}

// ============================================================================
// FieldHealth
// ============================================================================

/// Aggregated health summary of the Kuramoto coherence field.
///
/// Provides a snapshot of current field state, historical statistics,
/// and a boolean healthy flag. The field is considered healthy when
/// `current_r > 0.5` and `r_variance < 0.1`.
#[derive(Clone, Debug)]
pub struct FieldHealth {
    /// Current coherence parameter r
    pub current_r: f64,
    /// Average r over the history window
    pub avg_r: f64,
    /// Variance of r over the history window
    pub r_variance: f64,
    /// Current coupling strength K
    pub current_k: f64,
    /// Total number of captures recorded
    pub total_captures: u64,
    /// Number of times adaptation was triggered
    pub adaptation_triggers: u64,
    /// Whether the field is considered healthy
    pub healthy: bool,
}

// ============================================================================
// FieldBridgeConfig
// ============================================================================

/// Configuration for the Field Bridge.
///
/// Controls history buffer sizes and the adaptation threshold that
/// determines when a coherence delta triggers morphogenic adaptation.
#[derive(Clone, Debug)]
pub struct FieldBridgeConfig {
    /// Maximum number of r-values to retain in history
    pub r_history_capacity: usize,
    /// Maximum number of deltas to retain in history
    pub delta_history_capacity: usize,
    /// Minimum `|r_delta|` to trigger morphogenic adaptation
    pub adaptation_threshold: f64,
}

impl Default for FieldBridgeConfig {
    fn default() -> Self {
        Self {
            r_history_capacity: 500,
            delta_history_capacity: 200,
            adaptation_threshold: 0.05,
        }
    }
}

// ============================================================================
// FieldBridge (trait)
// ============================================================================

/// Kuramoto field bridge for pre/post state capture.
///
/// All methods are `&self` (C2) with interior mutability via `RwLock`.
/// Methods returning data through `RwLock` return owned types (C7).
pub trait FieldBridge: Send + Sync + fmt::Debug {
    /// Capture the current field state as a pre-operation snapshot.
    ///
    /// # Errors
    ///
    /// Returns `Error::Other` if the internal state cannot be read.
    fn capture_pre(&self) -> Result<FieldCapture>;

    /// Capture the current field state as a post-operation snapshot.
    ///
    /// # Errors
    ///
    /// Returns `Error::Other` if the internal state cannot be read.
    fn capture_post(&self) -> Result<FieldCapture>;

    /// Record a delta between pre and post captures.
    ///
    /// Computes `r_delta`, `k_delta`, `duration_ticks`, and checks whether
    /// morphogenic adaptation should be triggered.
    ///
    /// # Errors
    ///
    /// Returns `Error::Other` if internal state cannot be updated.
    fn record_delta(&self, pre: &FieldCapture, post: &FieldCapture) -> Result<FieldDelta>;

    /// Return the current coherence parameter r.
    fn current_r(&self) -> f64;

    /// Return the current coupling strength K.
    fn current_k(&self) -> f64;

    /// Return the most recent r-values, up to `limit`.
    fn r_history(&self, limit: usize) -> Vec<f64>;

    /// Compute and return the current field health summary.
    fn field_health(&self) -> FieldHealth;

    /// Update the internal field state with new values.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if `r` is not in `[0.0, 1.0]`.
    fn update_field_state(&self, r: f64, k: f64, spheres: u32) -> Result<()>;

    /// Return the total number of deltas recorded.
    fn delta_count(&self) -> usize;

    /// Return the most recent deltas, up to `limit`.
    fn recent_deltas(&self, limit: usize) -> Vec<FieldDelta>;

    /// Reset all internal state to defaults.
    fn reset(&self);
}

// ============================================================================
// FieldBridgeCore (implementation)
// ============================================================================

/// Production implementation of [`FieldBridge`].
///
/// Uses `parking_lot::RwLock` for interior mutability and `AtomicU64` for
/// lock-free counters. Maintains bounded ring buffers for r-history and
/// delta history.
pub struct FieldBridgeCore {
    /// Ring buffer of historical r-values
    r_history: RwLock<VecDeque<f64>>,
    /// Ring buffer of historical deltas
    deltas: RwLock<VecDeque<FieldDelta>>,
    /// Current field state snapshot
    current_state: RwLock<FieldCapture>,
    /// Configuration
    config: FieldBridgeConfig,
    /// Total captures counter (lock-free)
    total_captures: AtomicU64,
    /// Adaptation trigger counter (lock-free)
    adaptation_triggers: AtomicU64,
    /// Monotonic tick counter for captures
    tick_counter: AtomicU64,
}

impl fmt::Debug for FieldBridgeCore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FieldBridgeCore")
            .field("config", &self.config)
            .field(
                "total_captures",
                &self.total_captures.load(Ordering::Relaxed),
            )
            .field(
                "adaptation_triggers",
                &self.adaptation_triggers.load(Ordering::Relaxed),
            )
            .finish_non_exhaustive()
    }
}

impl FieldBridgeCore {
    /// Create a new `FieldBridgeCore` with default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self::with_config(FieldBridgeConfig::default())
    }

    /// Create a new `FieldBridgeCore` with the given configuration.
    #[must_use]
    pub fn with_config(config: FieldBridgeConfig) -> Self {
        Self {
            r_history: RwLock::new(VecDeque::with_capacity(config.r_history_capacity)),
            deltas: RwLock::new(VecDeque::with_capacity(config.delta_history_capacity)),
            current_state: RwLock::new(FieldCapture::zero()),
            config,
            total_captures: AtomicU64::new(0),
            adaptation_triggers: AtomicU64::new(0),
            tick_counter: AtomicU64::new(0),
        }
    }

    /// Advance the internal tick counter and return the new value.
    fn next_tick(&self) -> u64 {
        self.tick_counter.fetch_add(1, Ordering::Relaxed)
    }

    /// Compute mean and variance from a slice of f64 values.
    #[allow(clippy::cast_precision_loss)] // history buffers are capped at 500 entries
    fn compute_stats(values: &[f64]) -> (f64, f64) {
        if values.is_empty() {
            return (0.0, 0.0);
        }
        let n = values.len() as f64;
        let mean = values.iter().sum::<f64>() / n;
        let variance = if values.len() < 2 {
            0.0
        } else {
            values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / n
        };
        (mean, variance)
    }
}

impl Default for FieldBridgeCore {
    fn default() -> Self {
        Self::new()
    }
}

impl FieldBridge for FieldBridgeCore {
    fn capture_pre(&self) -> Result<FieldCapture> {
        self.total_captures.fetch_add(1, Ordering::Relaxed);
        let state = self.current_state.read();
        Ok(FieldCapture {
            r: state.r,
            k: state.k,
            spheres: state.spheres,
            timestamp: Timestamp::now(),
            tick: self.next_tick(),
        })
    }

    fn capture_post(&self) -> Result<FieldCapture> {
        self.total_captures.fetch_add(1, Ordering::Relaxed);
        let state = self.current_state.read();
        Ok(FieldCapture {
            r: state.r,
            k: state.k,
            spheres: state.spheres,
            timestamp: Timestamp::now(),
            tick: self.next_tick(),
        })
    }

    fn record_delta(&self, pre: &FieldCapture, post: &FieldCapture) -> Result<FieldDelta> {
        let r_delta = post.r - pre.r;
        let k_delta = post.k - pre.k;
        let duration_ticks = post.tick.saturating_sub(pre.tick);
        let triggered_adaptation = r_delta.abs() > self.config.adaptation_threshold;

        if triggered_adaptation {
            self.adaptation_triggers.fetch_add(1, Ordering::Relaxed);
        }

        let delta = FieldDelta {
            pre: pre.clone(),
            post: post.clone(),
            r_delta,
            k_delta,
            duration_ticks,
            triggered_adaptation,
        };

        {
            let mut deltas = self.deltas.write();
            if deltas.len() >= self.config.delta_history_capacity {
                deltas.pop_front();
            }
            deltas.push_back(delta.clone());
        }

        {
            let mut history = self.r_history.write();
            if history.len() >= self.config.r_history_capacity {
                history.pop_front();
            }
            history.push_back(post.r);
        }

        Ok(delta)
    }

    fn current_r(&self) -> f64 {
        self.current_state.read().r
    }

    fn current_k(&self) -> f64 {
        self.current_state.read().k
    }

    fn r_history(&self, limit: usize) -> Vec<f64> {
        let history = self.r_history.read();
        let start = history.len().saturating_sub(limit);
        history.iter().skip(start).copied().collect()
    }

    fn field_health(&self) -> FieldHealth {
        let state = self.current_state.read();
        let current_r = state.r;
        let current_k = state.k;
        drop(state);

        let history = self.r_history.read();
        let values: Vec<f64> = history.iter().copied().collect();
        drop(history);

        let (avg_r, r_variance) = Self::compute_stats(&values);
        let total_captures = self.total_captures.load(Ordering::Relaxed);
        let adaptation_triggers = self.adaptation_triggers.load(Ordering::Relaxed);
        let healthy = current_r > 0.5 && r_variance < 0.1;

        FieldHealth {
            current_r,
            avg_r,
            r_variance,
            current_k,
            total_captures,
            adaptation_triggers,
            healthy,
        }
    }

    fn update_field_state(&self, r: f64, k: f64, spheres: u32) -> Result<()> {
        if !(0.0..=1.0).contains(&r) {
            return Err(Error::Validation(format!(
                "r must be in [0.0, 1.0], got {r}"
            )));
        }

        {
            let mut state = self.current_state.write();
            state.r = r;
            state.k = k;
            state.spheres = spheres;
            state.timestamp = Timestamp::now();
            state.tick = self.next_tick();
        }

        Ok(())
    }

    fn delta_count(&self) -> usize {
        self.deltas.read().len()
    }

    fn recent_deltas(&self, limit: usize) -> Vec<FieldDelta> {
        let deltas = self.deltas.read();
        let start = deltas.len().saturating_sub(limit);
        deltas.iter().skip(start).cloned().collect()
    }

    fn reset(&self) {
        self.r_history.write().clear();
        self.deltas.write().clear();
        *self.current_state.write() = FieldCapture::zero();
        self.total_captures.store(0, Ordering::Relaxed);
        self.adaptation_triggers.store(0, Ordering::Relaxed);
        self.tick_counter.store(0, Ordering::Relaxed);
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // --- FieldCapture tests ---

    #[test]
    fn test_field_capture_new() {
        let ts = Timestamp::now();
        let cap = FieldCapture::new(0.85, 1.5, 10, ts, 42);
        assert!((cap.r - 0.85).abs() < f64::EPSILON);
        assert!((cap.k - 1.5).abs() < f64::EPSILON);
        assert_eq!(cap.spheres, 10);
        assert_eq!(cap.tick, 42);
    }

    #[test]
    fn test_field_capture_zero() {
        let cap = FieldCapture::zero();
        assert!((cap.r).abs() < f64::EPSILON);
        assert!((cap.k).abs() < f64::EPSILON);
        assert_eq!(cap.spheres, 0);
        assert_eq!(cap.tick, 0);
    }

    #[test]
    fn test_field_capture_clone() {
        let cap = FieldCapture::new(0.9, 2.0, 5, Timestamp::now(), 10);
        let clone = cap.clone();
        assert!((clone.r - cap.r).abs() < f64::EPSILON);
        assert_eq!(clone.spheres, cap.spheres);
    }

    #[test]
    fn test_field_capture_debug() {
        let cap = FieldCapture::zero();
        let debug = format!("{cap:?}");
        assert!(debug.contains("FieldCapture"));
    }

    // --- FieldBridgeConfig tests ---

    #[test]
    fn test_config_defaults() {
        let config = FieldBridgeConfig::default();
        assert_eq!(config.r_history_capacity, 500);
        assert_eq!(config.delta_history_capacity, 200);
        assert!((config.adaptation_threshold - 0.05).abs() < f64::EPSILON);
    }

    #[test]
    fn test_config_custom() {
        let config = FieldBridgeConfig {
            r_history_capacity: 100,
            delta_history_capacity: 50,
            adaptation_threshold: 0.1,
        };
        assert_eq!(config.r_history_capacity, 100);
        assert_eq!(config.delta_history_capacity, 50);
        assert!((config.adaptation_threshold - 0.1).abs() < f64::EPSILON);
    }

    #[test]
    fn test_config_clone() {
        let config = FieldBridgeConfig::default();
        let clone = config.clone();
        assert_eq!(clone.r_history_capacity, config.r_history_capacity);
    }

    #[test]
    fn test_config_debug() {
        let config = FieldBridgeConfig::default();
        let debug = format!("{config:?}");
        assert!(debug.contains("FieldBridgeConfig"));
    }

    // --- FieldBridgeCore construction tests ---

    #[test]
    fn test_core_new() {
        let core = FieldBridgeCore::new();
        assert!((core.current_r()).abs() < f64::EPSILON);
        assert!((core.current_k()).abs() < f64::EPSILON);
        assert_eq!(core.delta_count(), 0);
    }

    #[test]
    fn test_core_default() {
        let core = FieldBridgeCore::default();
        assert!((core.current_r()).abs() < f64::EPSILON);
    }

    #[test]
    fn test_core_with_config() {
        let config = FieldBridgeConfig {
            r_history_capacity: 10,
            delta_history_capacity: 5,
            adaptation_threshold: 0.2,
        };
        let core = FieldBridgeCore::with_config(config);
        assert_eq!(core.delta_count(), 0);
    }

    #[test]
    fn test_core_debug() {
        let core = FieldBridgeCore::new();
        let debug = format!("{core:?}");
        assert!(debug.contains("FieldBridgeCore"));
        assert!(debug.contains("total_captures"));
    }

    // --- capture tests ---

    #[test]
    fn test_capture_pre() {
        let core = FieldBridgeCore::new();
        core.update_field_state(0.8, 1.5, 10).ok();
        let cap = core.capture_pre();
        assert!(cap.is_ok());
        let cap = cap.ok();
        assert!(cap.is_some());
        let cap = cap.map(|c| c.r);
        assert!(cap.is_some());
    }

    #[test]
    fn test_capture_post() {
        let core = FieldBridgeCore::new();
        core.update_field_state(0.7, 1.2, 8).ok();
        let cap = core.capture_post();
        assert!(cap.is_ok());
    }

    #[test]
    fn test_capture_pre_reads_current_state() {
        let core = FieldBridgeCore::new();
        core.update_field_state(0.65, 1.1, 7).ok();
        let cap = core.capture_pre();
        assert!(cap.is_ok());
        if let Ok(c) = cap {
            assert!((c.r - 0.65).abs() < f64::EPSILON);
            assert!((c.k - 1.1).abs() < f64::EPSILON);
            assert_eq!(c.spheres, 7);
        }
    }

    #[test]
    fn test_capture_post_reads_current_state() {
        let core = FieldBridgeCore::new();
        core.update_field_state(0.55, 0.9, 4).ok();
        let cap = core.capture_post();
        assert!(cap.is_ok());
        if let Ok(c) = cap {
            assert!((c.r - 0.55).abs() < f64::EPSILON);
        }
    }

    #[test]
    fn test_captures_increment_total() {
        let core = FieldBridgeCore::new();
        let _ = core.capture_pre();
        let _ = core.capture_post();
        let health = core.field_health();
        assert_eq!(health.total_captures, 2);
    }

    // --- update_field_state tests ---

    #[test]
    fn test_update_field_state_valid() {
        let core = FieldBridgeCore::new();
        let result = core.update_field_state(0.9, 2.0, 15);
        assert!(result.is_ok());
        assert!((core.current_r() - 0.9).abs() < f64::EPSILON);
        assert!((core.current_k() - 2.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_update_field_state_r_zero() {
        let core = FieldBridgeCore::new();
        let result = core.update_field_state(0.0, 0.0, 0);
        assert!(result.is_ok());
    }

    #[test]
    fn test_update_field_state_r_one() {
        let core = FieldBridgeCore::new();
        let result = core.update_field_state(1.0, 3.0, 20);
        assert!(result.is_ok());
    }

    #[test]
    fn test_update_field_state_r_too_high() {
        let core = FieldBridgeCore::new();
        let result = core.update_field_state(1.1, 1.0, 5);
        assert!(result.is_err());
    }

    #[test]
    fn test_update_field_state_r_negative() {
        let core = FieldBridgeCore::new();
        let result = core.update_field_state(-0.1, 1.0, 5);
        assert!(result.is_err());
    }

    #[test]
    fn test_update_field_state_negative_k_allowed() {
        let core = FieldBridgeCore::new();
        let result = core.update_field_state(0.5, -1.0, 5);
        assert!(result.is_ok());
        assert!((core.current_k() - (-1.0)).abs() < f64::EPSILON);
    }

    // --- record_delta tests ---

    #[test]
    fn test_record_delta_basic() {
        let core = FieldBridgeCore::new();
        let pre = FieldCapture::new(0.5, 1.0, 5, Timestamp::now(), 0);
        let post = FieldCapture::new(0.52, 1.1, 6, Timestamp::now(), 1);
        let delta = core.record_delta(&pre, &post);
        assert!(delta.is_ok());
        let delta = delta.ok();
        assert!(delta.is_some());
    }

    #[test]
    fn test_record_delta_computes_r_delta() {
        let core = FieldBridgeCore::new();
        let pre = FieldCapture::new(0.5, 1.0, 5, Timestamp::now(), 0);
        let post = FieldCapture::new(0.7, 1.0, 5, Timestamp::now(), 1);
        if let Ok(delta) = core.record_delta(&pre, &post) {
            assert!((delta.r_delta - 0.2).abs() < f64::EPSILON);
        }
    }

    #[test]
    fn test_record_delta_computes_k_delta() {
        let core = FieldBridgeCore::new();
        let pre = FieldCapture::new(0.5, 1.0, 5, Timestamp::now(), 0);
        let post = FieldCapture::new(0.5, 2.5, 5, Timestamp::now(), 1);
        if let Ok(delta) = core.record_delta(&pre, &post) {
            assert!((delta.k_delta - 1.5).abs() < f64::EPSILON);
        }
    }

    #[test]
    fn test_record_delta_duration_ticks() {
        let core = FieldBridgeCore::new();
        let pre = FieldCapture::new(0.5, 1.0, 5, Timestamp::now(), 10);
        let post = FieldCapture::new(0.5, 1.0, 5, Timestamp::now(), 25);
        if let Ok(delta) = core.record_delta(&pre, &post) {
            assert_eq!(delta.duration_ticks, 15);
        }
    }

    #[test]
    fn test_record_delta_triggers_adaptation_positive() {
        let config = FieldBridgeConfig {
            adaptation_threshold: 0.05,
            ..FieldBridgeConfig::default()
        };
        let core = FieldBridgeCore::with_config(config);
        let pre = FieldCapture::new(0.5, 1.0, 5, Timestamp::now(), 0);
        let post = FieldCapture::new(0.56, 1.0, 5, Timestamp::now(), 1);
        if let Ok(delta) = core.record_delta(&pre, &post) {
            assert!(delta.triggered_adaptation);
        }
    }

    #[test]
    fn test_record_delta_triggers_adaptation_negative() {
        let config = FieldBridgeConfig {
            adaptation_threshold: 0.05,
            ..FieldBridgeConfig::default()
        };
        let core = FieldBridgeCore::with_config(config);
        let pre = FieldCapture::new(0.5, 1.0, 5, Timestamp::now(), 0);
        let post = FieldCapture::new(0.44, 1.0, 5, Timestamp::now(), 1);
        if let Ok(delta) = core.record_delta(&pre, &post) {
            assert!(delta.triggered_adaptation);
        }
    }

    #[test]
    fn test_record_delta_no_adaptation_within_threshold() {
        let config = FieldBridgeConfig {
            adaptation_threshold: 0.05,
            ..FieldBridgeConfig::default()
        };
        let core = FieldBridgeCore::with_config(config);
        let pre = FieldCapture::new(0.5, 1.0, 5, Timestamp::now(), 0);
        let post = FieldCapture::new(0.54, 1.0, 5, Timestamp::now(), 1);
        if let Ok(delta) = core.record_delta(&pre, &post) {
            assert!(!delta.triggered_adaptation);
        }
    }

    #[test]
    fn test_record_delta_exactly_at_threshold() {
        // Use a threshold where we can construct exact values
        let config = FieldBridgeConfig {
            adaptation_threshold: 0.25,
            ..FieldBridgeConfig::default()
        };
        let core = FieldBridgeCore::with_config(config);
        let pre = FieldCapture::new(0.5, 1.0, 5, Timestamp::now(), 0);
        let post = FieldCapture::new(0.75, 1.0, 5, Timestamp::now(), 1);
        if let Ok(delta) = core.record_delta(&pre, &post) {
            // |0.25| is NOT > 0.25, so no adaptation
            assert!(!delta.triggered_adaptation);
        }
    }

    #[test]
    fn test_record_delta_increments_delta_count() {
        let core = FieldBridgeCore::new();
        assert_eq!(core.delta_count(), 0);
        let pre = FieldCapture::new(0.5, 1.0, 5, Timestamp::now(), 0);
        let post = FieldCapture::new(0.6, 1.0, 5, Timestamp::now(), 1);
        let _ = core.record_delta(&pre, &post);
        assert_eq!(core.delta_count(), 1);
    }

    #[test]
    fn test_record_delta_adds_to_r_history() {
        let core = FieldBridgeCore::new();
        let pre = FieldCapture::new(0.5, 1.0, 5, Timestamp::now(), 0);
        let post = FieldCapture::new(0.7, 1.0, 5, Timestamp::now(), 1);
        let _ = core.record_delta(&pre, &post);
        let history = core.r_history(10);
        assert_eq!(history.len(), 1);
        assert!((history[0] - 0.7).abs() < f64::EPSILON);
    }

    // --- r_history tests ---

    #[test]
    fn test_r_history_empty() {
        let core = FieldBridgeCore::new();
        let history = core.r_history(10);
        assert!(history.is_empty());
    }

    #[test]
    fn test_r_history_limit() {
        let core = FieldBridgeCore::new();
        for i in 0..10u64 {
            let r = f64::from(i as u32) / 10.0;
            let pre = FieldCapture::new(0.0, 0.0, 0, Timestamp::now(), i * 2);
            let post = FieldCapture::new(r, 0.0, 0, Timestamp::now(), i * 2 + 1);
            let _ = core.record_delta(&pre, &post);
        }
        let history = core.r_history(5);
        assert_eq!(history.len(), 5);
    }

    #[test]
    fn test_r_history_returns_most_recent() {
        let core = FieldBridgeCore::new();
        for i in 0..5u64 {
            let r = f64::from(i as u32) / 10.0;
            let pre = FieldCapture::new(0.0, 0.0, 0, Timestamp::now(), i * 2);
            let post = FieldCapture::new(r, 0.0, 0, Timestamp::now(), i * 2 + 1);
            let _ = core.record_delta(&pre, &post);
        }
        let history = core.r_history(3);
        assert_eq!(history.len(), 3);
        // Last 3 values: 0.2, 0.3, 0.4
        assert!((history[0] - 0.2).abs() < f64::EPSILON);
        assert!((history[2] - 0.4).abs() < f64::EPSILON);
    }

    // --- capacity tests ---

    #[test]
    fn test_r_history_capacity_bounded() {
        let config = FieldBridgeConfig {
            r_history_capacity: 3,
            ..FieldBridgeConfig::default()
        };
        let core = FieldBridgeCore::with_config(config);
        for i in 0..10u64 {
            let r = f64::from(i as u32) / 10.0;
            let pre = FieldCapture::new(0.0, 0.0, 0, Timestamp::now(), i * 2);
            let post = FieldCapture::new(r, 0.0, 0, Timestamp::now(), i * 2 + 1);
            let _ = core.record_delta(&pre, &post);
        }
        let history = core.r_history(100);
        assert_eq!(history.len(), 3);
    }

    #[test]
    fn test_delta_history_capacity_bounded() {
        let config = FieldBridgeConfig {
            delta_history_capacity: 3,
            ..FieldBridgeConfig::default()
        };
        let core = FieldBridgeCore::with_config(config);
        for i in 0..10u64 {
            let pre = FieldCapture::new(0.0, 0.0, 0, Timestamp::now(), i * 2);
            let post = FieldCapture::new(0.5, 0.0, 0, Timestamp::now(), i * 2 + 1);
            let _ = core.record_delta(&pre, &post);
        }
        assert_eq!(core.delta_count(), 3);
    }

    // --- field_health tests ---

    #[test]
    fn test_field_health_empty() {
        let core = FieldBridgeCore::new();
        let health = core.field_health();
        assert!((health.current_r).abs() < f64::EPSILON);
        assert!((health.avg_r).abs() < f64::EPSILON);
        assert!((health.r_variance).abs() < f64::EPSILON);
        assert_eq!(health.total_captures, 0);
        assert_eq!(health.adaptation_triggers, 0);
        assert!(!health.healthy);
    }

    #[test]
    fn test_field_health_healthy() {
        let core = FieldBridgeCore::new();
        core.update_field_state(0.8, 1.5, 10).ok();
        // Add some consistent r values to history
        for i in 0..5u64 {
            let pre = FieldCapture::new(0.79, 1.5, 10, Timestamp::now(), i * 2);
            let post = FieldCapture::new(0.8, 1.5, 10, Timestamp::now(), i * 2 + 1);
            let _ = core.record_delta(&pre, &post);
        }
        let health = core.field_health();
        assert!(health.healthy);
        assert!(health.current_r > 0.5);
        assert!(health.r_variance < 0.1);
    }

    #[test]
    fn test_field_health_unhealthy_low_r() {
        let core = FieldBridgeCore::new();
        core.update_field_state(0.3, 0.5, 2).ok();
        let health = core.field_health();
        assert!(!health.healthy);
    }

    #[test]
    fn test_field_health_unhealthy_high_variance() {
        let core = FieldBridgeCore::new();
        core.update_field_state(0.8, 1.5, 10).ok();
        // Add wildly varying r values
        let values = [0.1, 0.9, 0.2, 0.8, 0.15, 0.85];
        for (i, &r) in values.iter().enumerate() {
            let pre = FieldCapture::new(0.5, 1.0, 5, Timestamp::now(), (i * 2) as u64);
            let post = FieldCapture::new(r, 1.0, 5, Timestamp::now(), (i * 2 + 1) as u64);
            let _ = core.record_delta(&pre, &post);
        }
        let health = core.field_health();
        assert!(health.r_variance > 0.1);
        assert!(!health.healthy);
    }

    #[test]
    fn test_field_health_adaptation_count() {
        let config = FieldBridgeConfig {
            adaptation_threshold: 0.05,
            ..FieldBridgeConfig::default()
        };
        let core = FieldBridgeCore::with_config(config);
        // Trigger 3 adaptations
        for i in 0..3u64 {
            let pre = FieldCapture::new(0.5, 1.0, 5, Timestamp::now(), i * 2);
            let post = FieldCapture::new(0.7, 1.0, 5, Timestamp::now(), i * 2 + 1);
            let _ = core.record_delta(&pre, &post);
        }
        let health = core.field_health();
        assert_eq!(health.adaptation_triggers, 3);
    }

    // --- recent_deltas tests ---

    #[test]
    fn test_recent_deltas_empty() {
        let core = FieldBridgeCore::new();
        let deltas = core.recent_deltas(10);
        assert!(deltas.is_empty());
    }

    #[test]
    fn test_recent_deltas_returns_most_recent() {
        let core = FieldBridgeCore::new();
        for i in 0..5u64 {
            let r = f64::from(i as u32) / 10.0;
            let pre = FieldCapture::new(0.0, 0.0, 0, Timestamp::now(), i * 2);
            let post = FieldCapture::new(r, 0.0, 0, Timestamp::now(), i * 2 + 1);
            let _ = core.record_delta(&pre, &post);
        }
        let deltas = core.recent_deltas(2);
        assert_eq!(deltas.len(), 2);
    }

    #[test]
    fn test_recent_deltas_limit_larger_than_count() {
        let core = FieldBridgeCore::new();
        let pre = FieldCapture::new(0.5, 1.0, 5, Timestamp::now(), 0);
        let post = FieldCapture::new(0.6, 1.0, 5, Timestamp::now(), 1);
        let _ = core.record_delta(&pre, &post);
        let deltas = core.recent_deltas(100);
        assert_eq!(deltas.len(), 1);
    }

    // --- reset tests ---

    #[test]
    fn test_reset_clears_state() {
        let core = FieldBridgeCore::new();
        core.update_field_state(0.8, 1.5, 10).ok();
        let pre = FieldCapture::new(0.5, 1.0, 5, Timestamp::now(), 0);
        let post = FieldCapture::new(0.7, 1.0, 5, Timestamp::now(), 1);
        let _ = core.record_delta(&pre, &post);

        core.reset();

        assert!((core.current_r()).abs() < f64::EPSILON);
        assert!((core.current_k()).abs() < f64::EPSILON);
        assert_eq!(core.delta_count(), 0);
        assert!(core.r_history(100).is_empty());
        let health = core.field_health();
        assert_eq!(health.total_captures, 0);
        assert_eq!(health.adaptation_triggers, 0);
    }

    #[test]
    fn test_reset_allows_reuse() {
        let core = FieldBridgeCore::new();
        core.update_field_state(0.8, 1.5, 10).ok();
        core.reset();
        let result = core.update_field_state(0.6, 1.0, 5);
        assert!(result.is_ok());
        assert!((core.current_r() - 0.6).abs() < f64::EPSILON);
    }

    // --- compute_stats tests ---

    #[test]
    fn test_compute_stats_empty() {
        let (mean, variance) = FieldBridgeCore::compute_stats(&[]);
        assert!((mean).abs() < f64::EPSILON);
        assert!((variance).abs() < f64::EPSILON);
    }

    #[test]
    fn test_compute_stats_single() {
        let (mean, variance) = FieldBridgeCore::compute_stats(&[0.5]);
        assert!((mean - 0.5).abs() < f64::EPSILON);
        assert!((variance).abs() < f64::EPSILON);
    }

    #[test]
    fn test_compute_stats_uniform() {
        let (mean, variance) = FieldBridgeCore::compute_stats(&[0.5, 0.5, 0.5]);
        assert!((mean - 0.5).abs() < f64::EPSILON);
        assert!((variance).abs() < f64::EPSILON);
    }

    #[test]
    fn test_compute_stats_varied() {
        let (mean, _variance) = FieldBridgeCore::compute_stats(&[0.0, 1.0]);
        assert!((mean - 0.5).abs() < f64::EPSILON);
    }

    // --- integration / scenario tests ---

    #[test]
    fn test_full_capture_delta_cycle() {
        let core = FieldBridgeCore::new();
        core.update_field_state(0.5, 1.0, 5).ok();
        let pre = core.capture_pre();
        assert!(pre.is_ok());
        let pre = pre.ok();

        core.update_field_state(0.7, 1.5, 8).ok();
        let post = core.capture_post();
        assert!(post.is_ok());
        let post = post.ok();

        if let (Some(pre), Some(post)) = (pre, post) {
            let delta = core.record_delta(&pre, &post);
            assert!(delta.is_ok());
            if let Ok(d) = delta {
                assert!((d.r_delta - 0.2).abs() < f64::EPSILON);
                assert!(d.triggered_adaptation); // |0.2| > 0.05
            }
        }
    }

    #[test]
    fn test_multiple_deltas_accumulate() {
        let core = FieldBridgeCore::new();
        for i in 0..20u64 {
            let r_pre = f64::from(i as u32) / 100.0;
            let r_post = f64::from(i as u32 + 1) / 100.0;
            let pre = FieldCapture::new(r_pre, 1.0, 5, Timestamp::now(), i * 2);
            let post = FieldCapture::new(r_post, 1.0, 5, Timestamp::now(), i * 2 + 1);
            let _ = core.record_delta(&pre, &post);
        }
        assert_eq!(core.delta_count(), 20);
        assert_eq!(core.r_history(100).len(), 20);
    }

    #[test]
    fn test_field_delta_clone() {
        let core = FieldBridgeCore::new();
        let pre = FieldCapture::new(0.5, 1.0, 5, Timestamp::now(), 0);
        let post = FieldCapture::new(0.6, 1.0, 5, Timestamp::now(), 1);
        if let Ok(delta) = core.record_delta(&pre, &post) {
            let clone = delta.clone();
            assert!((clone.r_delta - delta.r_delta).abs() < f64::EPSILON);
        }
    }

    #[test]
    fn test_field_delta_debug() {
        let core = FieldBridgeCore::new();
        let pre = FieldCapture::new(0.5, 1.0, 5, Timestamp::now(), 0);
        let post = FieldCapture::new(0.6, 1.0, 5, Timestamp::now(), 1);
        if let Ok(delta) = core.record_delta(&pre, &post) {
            let debug = format!("{delta:?}");
            assert!(debug.contains("FieldDelta"));
        }
    }

    #[test]
    fn test_field_health_clone() {
        let core = FieldBridgeCore::new();
        let health = core.field_health();
        let clone = health.clone();
        assert!((clone.current_r - health.current_r).abs() < f64::EPSILON);
    }

    #[test]
    fn test_field_health_debug() {
        let core = FieldBridgeCore::new();
        let health = core.field_health();
        let debug = format!("{health:?}");
        assert!(debug.contains("FieldHealth"));
    }

    #[test]
    fn test_current_r_after_updates() {
        let core = FieldBridgeCore::new();
        core.update_field_state(0.3, 1.0, 5).ok();
        assert!((core.current_r() - 0.3).abs() < f64::EPSILON);
        core.update_field_state(0.7, 2.0, 10).ok();
        assert!((core.current_r() - 0.7).abs() < f64::EPSILON);
    }

    #[test]
    fn test_current_k_after_updates() {
        let core = FieldBridgeCore::new();
        core.update_field_state(0.5, 0.5, 3).ok();
        assert!((core.current_k() - 0.5).abs() < f64::EPSILON);
        core.update_field_state(0.5, 3.0, 3).ok();
        assert!((core.current_k() - 3.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_delta_saturating_sub_ticks() {
        let core = FieldBridgeCore::new();
        // pre.tick > post.tick should saturate to 0
        let pre = FieldCapture::new(0.5, 1.0, 5, Timestamp::now(), 100);
        let post = FieldCapture::new(0.6, 1.0, 5, Timestamp::now(), 50);
        if let Ok(delta) = core.record_delta(&pre, &post) {
            assert_eq!(delta.duration_ticks, 0);
        }
    }

    #[test]
    fn test_trait_object_compatibility() {
        let core = FieldBridgeCore::new();
        let bridge: &dyn FieldBridge = &core;
        assert!((bridge.current_r()).abs() < f64::EPSILON);
    }

    #[test]
    fn test_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<FieldBridgeCore>();
    }

    #[test]
    fn test_multiple_resets() {
        let core = FieldBridgeCore::new();
        core.update_field_state(0.8, 1.5, 10).ok();
        core.reset();
        core.update_field_state(0.6, 1.0, 5).ok();
        core.reset();
        assert!((core.current_r()).abs() < f64::EPSILON);
        assert_eq!(core.delta_count(), 0);
    }

    #[test]
    fn test_zero_adaptation_threshold() {
        let config = FieldBridgeConfig {
            adaptation_threshold: 0.0,
            ..FieldBridgeConfig::default()
        };
        let core = FieldBridgeCore::with_config(config);
        let pre = FieldCapture::new(0.5, 1.0, 5, Timestamp::now(), 0);
        let post = FieldCapture::new(0.5001, 1.0, 5, Timestamp::now(), 1);
        if let Ok(delta) = core.record_delta(&pre, &post) {
            assert!(delta.triggered_adaptation);
        }
    }

    #[test]
    fn test_large_adaptation_threshold() {
        let config = FieldBridgeConfig {
            adaptation_threshold: 10.0,
            ..FieldBridgeConfig::default()
        };
        let core = FieldBridgeCore::with_config(config);
        let pre = FieldCapture::new(0.0, 0.0, 0, Timestamp::now(), 0);
        let post = FieldCapture::new(1.0, 5.0, 20, Timestamp::now(), 1);
        if let Ok(delta) = core.record_delta(&pre, &post) {
            assert!(!delta.triggered_adaptation);
        }
    }

    #[test]
    fn test_health_avg_r_matches_history() {
        let core = FieldBridgeCore::new();
        let values = [0.4, 0.6, 0.8];
        for (i, &r) in values.iter().enumerate() {
            let pre = FieldCapture::new(0.0, 0.0, 0, Timestamp::now(), (i * 2) as u64);
            let post = FieldCapture::new(r, 0.0, 0, Timestamp::now(), (i * 2 + 1) as u64);
            let _ = core.record_delta(&pre, &post);
        }
        let health = core.field_health();
        let expected_avg = (0.4 + 0.6 + 0.8) / 3.0;
        assert!((health.avg_r - expected_avg).abs() < f64::EPSILON);
    }
}
