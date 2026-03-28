//! # M42: Cascade Bridge
//!
//! Monitors the SYNTHEX V3 cascade pipeline by polling the diagnostics
//! endpoint and feeding amplification anomalies into M38 Emergence Detector.
//!
//! ## Layer: L4 (Integration)
//! ## Dependencies: M38 (Emergence Detector), SYNTHEX V3 `/v3/diagnostics`
//!
//! ## Polling Configuration
//!
//! - Default interval: 15 seconds
//! - Sliding window: 60 snapshots (15 minutes at 15s intervals)
//! - Fail-silent: continues operating if SYNTHEX is unreachable
//!
//! ## Related Documentation
//! - [V3 Diagnostics](../../developer_environment_manager/synthex/src/v3/diagnostics.rs)
//! - [V3 Cascade Pipeline](../../developer_environment_manager/synthex/src/v3/cascade.rs)

use std::collections::VecDeque;

use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

use crate::Result;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Default polling interval in seconds.
const DEFAULT_POLL_INTERVAL_SECS: u64 = 15;

/// Default sliding window capacity (60 snapshots = 15 minutes at 15s).
const DEFAULT_WINDOW_CAPACITY: usize = 60;

/// Maximum consecutive failures before entering degraded mode.
const MAX_CONSECUTIVE_FAILURES: u32 = 10;

/// Default SYNTHEX V3 diagnostics endpoint.
const DEFAULT_DIAGNOSTICS_URL: &str = "http://localhost:8090/v3/diagnostics";

/// Default amplification threshold for anomaly detection.
const DEFAULT_AMPLIFICATION_THRESHOLD: f64 = 500.0;

/// Number of stages in the cascade pipeline.
const CASCADE_STAGE_COUNT: u32 = 12;

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Configuration for the cascade bridge.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CascadeBridgeConfig {
    /// URL of the SYNTHEX V3 diagnostics endpoint.
    pub diagnostics_url: String,
    /// Polling interval in seconds.
    pub poll_interval_secs: u64,
    /// Sliding window capacity.
    pub window_capacity: usize,
    /// Amplification threshold for anomaly detection.
    pub amplification_threshold: f64,
}

impl Default for CascadeBridgeConfig {
    fn default() -> Self {
        Self {
            diagnostics_url: DEFAULT_DIAGNOSTICS_URL.to_string(),
            poll_interval_secs: DEFAULT_POLL_INTERVAL_SECS,
            window_capacity: DEFAULT_WINDOW_CAPACITY,
            amplification_threshold: DEFAULT_AMPLIFICATION_THRESHOLD,
        }
    }
}

// ---------------------------------------------------------------------------
// Cascade Stage Snapshot
// ---------------------------------------------------------------------------

/// A snapshot of a single cascade stage.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CascadeStageSnapshot {
    /// Stage index (0-based).
    pub stage_index: u32,
    /// Effective amplification at this stage.
    pub amplification: f64,
    /// Whether the circuit breaker is open.
    pub circuit_breaker_open: bool,
    /// Number of signals processed.
    pub signals_processed: u64,
}

// ---------------------------------------------------------------------------
// Cascade Pipeline Snapshot
// ---------------------------------------------------------------------------

/// A full snapshot of the cascade pipeline diagnostics.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CascadePipelineSnapshot {
    /// Per-stage snapshots.
    pub stages: Vec<CascadeStageSnapshot>,
    /// Total amplification across all stages.
    pub total_amplification: f64,
    /// Number of open circuit breakers.
    pub open_breakers: u32,
    /// Timestamp of the snapshot.
    pub timestamp: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Cascade Anomaly
// ---------------------------------------------------------------------------

/// A detected cascade anomaly.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CascadeAnomaly {
    /// Total amplification that exceeded the threshold.
    pub amplification: f64,
    /// Threshold that was exceeded.
    pub threshold: f64,
    /// Number of open circuit breakers at the time.
    pub open_breakers: u32,
    /// Timestamp of detection.
    pub detected_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Cascade Health
// ---------------------------------------------------------------------------

/// Summary of cascade pipeline health.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CascadeHealth {
    /// Latest total amplification.
    pub current_amplification: f64,
    /// Average amplification over the window.
    pub avg_amplification: f64,
    /// Maximum amplification observed in the window.
    pub max_amplification: f64,
    /// Number of open circuit breakers.
    pub open_breakers: u32,
    /// Whether amplification exceeds the threshold.
    pub anomaly_detected: bool,
    /// Number of snapshots in the window.
    pub window_size: usize,
    /// Number of consecutive poll failures.
    pub consecutive_failures: u32,
    /// Whether the bridge is in degraded mode.
    pub degraded: bool,
}

// ---------------------------------------------------------------------------
// Cascade Bridge
// ---------------------------------------------------------------------------

/// M42: Cascade bridge for monitoring V3 pipeline diagnostics.
///
/// Polls the SYNTHEX `/v3/diagnostics` endpoint at configurable intervals,
/// maintains a sliding window of pipeline snapshots, and flags amplification
/// anomalies for M38.
///
/// # Thread Safety
///
/// All mutable state is protected by `parking_lot::RwLock`.
pub struct CascadeBridge {
    /// Sliding window of pipeline snapshots.
    snapshots: RwLock<VecDeque<CascadePipelineSnapshot>>,
    /// Detected anomalies.
    anomalies: RwLock<Vec<CascadeAnomaly>>,
    /// Number of consecutive poll failures.
    consecutive_failures: RwLock<u32>,
    /// Configuration.
    config: CascadeBridgeConfig,
}

impl CascadeBridge {
    /// Create a new cascade bridge with default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self::with_config(CascadeBridgeConfig::default())
    }

    /// Create a new cascade bridge with the given configuration.
    #[must_use]
    pub fn with_config(config: CascadeBridgeConfig) -> Self {
        Self {
            snapshots: RwLock::new(VecDeque::with_capacity(config.window_capacity)),
            anomalies: RwLock::new(Vec::new()),
            consecutive_failures: RwLock::new(0),
            config,
        }
    }

    /// Record a successful pipeline snapshot.
    ///
    /// If the total amplification exceeds the configured threshold,
    /// a cascade anomaly is recorded.
    pub fn record_snapshot(&self, snapshot: CascadePipelineSnapshot) {
        // Check for anomaly
        if snapshot.total_amplification > self.config.amplification_threshold {
            self.anomalies.write().push(CascadeAnomaly {
                amplification: snapshot.total_amplification,
                threshold: self.config.amplification_threshold,
                open_breakers: snapshot.open_breakers,
                detected_at: Utc::now(),
            });
        }

        let mut guard = self.snapshots.write();
        if guard.len() >= self.config.window_capacity {
            guard.pop_front();
        }
        guard.push_back(snapshot);
        drop(guard);

        *self.consecutive_failures.write() = 0;
    }

    /// Record a poll failure.
    pub fn record_failure(&self) {
        *self.consecutive_failures.write() += 1;
    }

    /// Get the number of consecutive failures.
    #[must_use]
    pub fn consecutive_failures(&self) -> u32 {
        *self.consecutive_failures.read()
    }

    /// Check whether the bridge is in degraded mode.
    #[must_use]
    pub fn is_degraded(&self) -> bool {
        *self.consecutive_failures.read() >= MAX_CONSECUTIVE_FAILURES
    }

    /// Get the most recent pipeline snapshot, if any.
    #[must_use]
    pub fn latest_snapshot(&self) -> Option<CascadePipelineSnapshot> {
        self.snapshots.read().back().cloned()
    }

    /// Get the number of snapshots in the sliding window.
    #[must_use]
    pub fn window_size(&self) -> usize {
        self.snapshots.read().len()
    }

    /// Get all detected anomalies.
    #[must_use]
    pub fn anomalies(&self) -> Vec<CascadeAnomaly> {
        self.anomalies.read().clone()
    }

    /// Compute a cascade health summary.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn health(&self) -> CascadeHealth {
        let guard = self.snapshots.read();
        let failures = *self.consecutive_failures.read();

        if guard.is_empty() {
            return CascadeHealth {
                current_amplification: 0.0,
                avg_amplification: 0.0,
                max_amplification: 0.0,
                open_breakers: 0,
                anomaly_detected: false,
                window_size: 0,
                consecutive_failures: failures,
                degraded: failures >= MAX_CONSECUTIVE_FAILURES,
            };
        }

        let latest = guard.back().cloned().unwrap_or_else(|| CascadePipelineSnapshot {
            stages: Vec::new(),
            total_amplification: 0.0,
            open_breakers: 0,
            timestamp: Utc::now(),
        });

        let sum: f64 = guard.iter().map(|s| s.total_amplification).sum();
        let avg = sum / guard.len() as f64;

        let max = guard
            .iter()
            .map(|s| s.total_amplification)
            .fold(0.0_f64, f64::max);

        let anomaly = latest.total_amplification > self.config.amplification_threshold;

        CascadeHealth {
            current_amplification: latest.total_amplification,
            avg_amplification: avg,
            max_amplification: max,
            open_breakers: latest.open_breakers,
            anomaly_detected: anomaly,
            window_size: guard.len(),
            consecutive_failures: failures,
            degraded: failures >= MAX_CONSECUTIVE_FAILURES,
        }
    }

    /// Check if the latest snapshot indicates an amplification anomaly.
    ///
    /// Returns `Some((amplification, stage_count, threshold))` if anomalous.
    #[must_use]
    pub fn check_anomaly(&self) -> Option<(f64, u32, f64)> {
        let latest = { self.snapshots.read().back().cloned() }?;
        if latest.total_amplification > self.config.amplification_threshold {
            Some((
                latest.total_amplification,
                CASCADE_STAGE_COUNT,
                self.config.amplification_threshold,
            ))
        } else {
            None
        }
    }

    /// Get a reference to the configuration.
    #[must_use]
    pub const fn config(&self) -> &CascadeBridgeConfig {
        &self.config
    }

    /// Clear all snapshots and anomalies, reset failure count.
    pub fn reset(&self) {
        self.snapshots.write().clear();
        self.anomalies.write().clear();
        *self.consecutive_failures.write() = 0;
    }

    /// Poll the diagnostics endpoint (synchronous stub).
    ///
    /// # Errors
    ///
    /// Returns `Error::Other` as this is a stub.
    pub fn poll(&self) -> Result<CascadePipelineSnapshot> {
        Err(crate::Error::Other(
            "poll() is a stub; use the background task for live polling".into(),
        ))
    }
}

impl Default for CascadeBridge {
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

    fn make_snapshot(amp: f64, breakers: u32) -> CascadePipelineSnapshot {
        CascadePipelineSnapshot {
            stages: Vec::new(),
            total_amplification: amp,
            open_breakers: breakers,
            timestamp: Utc::now(),
        }
    }

    #[test]
    fn test_new_empty() {
        let bridge = CascadeBridge::new();
        assert_eq!(bridge.window_size(), 0);
        assert_eq!(bridge.consecutive_failures(), 0);
        assert!(!bridge.is_degraded());
        assert!(bridge.latest_snapshot().is_none());
        assert!(bridge.anomalies().is_empty());
    }

    #[test]
    fn test_record_snapshot() {
        let bridge = CascadeBridge::new();
        bridge.record_snapshot(make_snapshot(10.0, 0));
        assert_eq!(bridge.window_size(), 1);
        assert_eq!(bridge.consecutive_failures(), 0);
    }

    #[test]
    fn test_record_failure() {
        let bridge = CascadeBridge::new();
        for _ in 0..5 {
            bridge.record_failure();
        }
        assert_eq!(bridge.consecutive_failures(), 5);
        assert!(!bridge.is_degraded());

        for _ in 0..5 {
            bridge.record_failure();
        }
        assert!(bridge.is_degraded());
    }

    #[test]
    fn test_failure_reset_on_success() {
        let bridge = CascadeBridge::new();
        bridge.record_failure();
        bridge.record_failure();
        bridge.record_snapshot(make_snapshot(1.0, 0));
        assert_eq!(bridge.consecutive_failures(), 0);
    }

    #[test]
    fn test_window_capacity() {
        let config = CascadeBridgeConfig {
            window_capacity: 5,
            ..Default::default()
        };
        let bridge = CascadeBridge::with_config(config);
        for i in 0..10 {
            #[allow(clippy::cast_precision_loss)]
            bridge.record_snapshot(make_snapshot(i as f64, 0));
        }
        assert_eq!(bridge.window_size(), 5);
    }

    #[test]
    fn test_anomaly_detection() {
        let bridge = CascadeBridge::new();
        // Below threshold
        bridge.record_snapshot(make_snapshot(100.0, 0));
        assert!(bridge.anomalies().is_empty());
        assert!(bridge.check_anomaly().is_none());

        // Above threshold (500.0)
        bridge.record_snapshot(make_snapshot(1814.0, 3));
        assert_eq!(bridge.anomalies().len(), 1);
        let anomaly = bridge.check_anomaly();
        assert!(anomaly.is_some());
        if let Some((amp, stages, thresh)) = anomaly {
            assert!((amp - 1814.0).abs() < f64::EPSILON);
            assert_eq!(stages, CASCADE_STAGE_COUNT);
            assert!((thresh - 500.0).abs() < f64::EPSILON);
        }
    }

    #[test]
    fn test_health_empty() {
        let bridge = CascadeBridge::new();
        let h = bridge.health();
        assert!((h.current_amplification).abs() < f64::EPSILON);
        assert_eq!(h.window_size, 0);
        assert!(!h.anomaly_detected);
    }

    #[test]
    fn test_health_with_snapshots() {
        let bridge = CascadeBridge::new();
        bridge.record_snapshot(make_snapshot(100.0, 0));
        bridge.record_snapshot(make_snapshot(200.0, 1));
        bridge.record_snapshot(make_snapshot(300.0, 0));

        let h = bridge.health();
        assert_eq!(h.window_size, 3);
        assert!((h.current_amplification - 300.0).abs() < f64::EPSILON);
        assert!((h.avg_amplification - 200.0).abs() < f64::EPSILON);
        assert!((h.max_amplification - 300.0).abs() < f64::EPSILON);
        assert!(!h.anomaly_detected);
    }

    #[test]
    fn test_health_anomaly_flag() {
        let bridge = CascadeBridge::new();
        bridge.record_snapshot(make_snapshot(600.0, 2));

        let h = bridge.health();
        assert!(h.anomaly_detected);
        assert_eq!(h.open_breakers, 2);
    }

    #[test]
    fn test_reset() {
        let bridge = CascadeBridge::new();
        bridge.record_snapshot(make_snapshot(1000.0, 0));
        bridge.record_failure();
        bridge.reset();
        assert_eq!(bridge.window_size(), 0);
        assert!(bridge.anomalies().is_empty());
        assert_eq!(bridge.consecutive_failures(), 0);
    }

    #[test]
    fn test_poll_stub() {
        let bridge = CascadeBridge::new();
        assert!(bridge.poll().is_err());
    }

    #[test]
    fn test_config_accessor() {
        let bridge = CascadeBridge::new();
        assert_eq!(bridge.config().poll_interval_secs, DEFAULT_POLL_INTERVAL_SECS);
        assert!((bridge.config().amplification_threshold - DEFAULT_AMPLIFICATION_THRESHOLD).abs() < f64::EPSILON);
    }

    // --- Additional tests to reach 50+ ---

    #[test]
    fn test_default_creates_same_as_new() {
        let d = CascadeBridge::default();
        let n = CascadeBridge::new();
        assert_eq!(d.window_size(), n.window_size());
        assert_eq!(d.consecutive_failures(), n.consecutive_failures());
    }

    #[test]
    fn test_config_default_diagnostics_url() {
        let config = CascadeBridgeConfig::default();
        assert_eq!(config.diagnostics_url, DEFAULT_DIAGNOSTICS_URL);
    }

    #[test]
    fn test_config_default_poll_interval() {
        let config = CascadeBridgeConfig::default();
        assert_eq!(config.poll_interval_secs, DEFAULT_POLL_INTERVAL_SECS);
    }

    #[test]
    fn test_config_default_window_capacity() {
        let config = CascadeBridgeConfig::default();
        assert_eq!(config.window_capacity, DEFAULT_WINDOW_CAPACITY);
    }

    #[test]
    fn test_config_default_amplification_threshold() {
        let config = CascadeBridgeConfig::default();
        assert!((config.amplification_threshold - DEFAULT_AMPLIFICATION_THRESHOLD).abs() < f64::EPSILON);
    }

    #[test]
    fn test_custom_config_diagnostics_url() {
        let config = CascadeBridgeConfig {
            diagnostics_url: "http://custom:9090/diag".to_string(),
            ..Default::default()
        };
        let bridge = CascadeBridge::with_config(config);
        assert_eq!(bridge.config().diagnostics_url, "http://custom:9090/diag");
    }

    #[test]
    fn test_custom_config_threshold() {
        let config = CascadeBridgeConfig {
            amplification_threshold: 100.0,
            ..Default::default()
        };
        let bridge = CascadeBridge::with_config(config);
        // Below custom threshold => no anomaly
        bridge.record_snapshot(make_snapshot(99.0, 0));
        assert!(bridge.anomalies().is_empty());
        // Above custom threshold => anomaly
        bridge.record_snapshot(make_snapshot(101.0, 0));
        assert_eq!(bridge.anomalies().len(), 1);
    }

    #[test]
    fn test_latest_snapshot_returns_most_recent() {
        let bridge = CascadeBridge::new();
        bridge.record_snapshot(make_snapshot(10.0, 0));
        bridge.record_snapshot(make_snapshot(20.0, 1));
        bridge.record_snapshot(make_snapshot(30.0, 2));
        let latest = bridge.latest_snapshot();
        assert!(latest.is_some());
        let snap = latest.unwrap_or_else(|| make_snapshot(0.0, 0));
        assert!((snap.total_amplification - 30.0).abs() < f64::EPSILON);
        assert_eq!(snap.open_breakers, 2);
    }

    #[test]
    fn test_window_eviction_preserves_latest() {
        let config = CascadeBridgeConfig {
            window_capacity: 3,
            ..Default::default()
        };
        let bridge = CascadeBridge::with_config(config);
        for i in 0..5 {
            #[allow(clippy::cast_precision_loss)]
            bridge.record_snapshot(make_snapshot(i as f64 * 10.0, 0));
        }
        assert_eq!(bridge.window_size(), 3);
        let latest = bridge.latest_snapshot();
        assert!(latest.is_some());
        let snap = latest.unwrap_or_else(|| make_snapshot(0.0, 0));
        assert!((snap.total_amplification - 40.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_multiple_anomalies_accumulated() {
        let bridge = CascadeBridge::new();
        bridge.record_snapshot(make_snapshot(600.0, 1));
        bridge.record_snapshot(make_snapshot(700.0, 2));
        bridge.record_snapshot(make_snapshot(800.0, 3));
        assert_eq!(bridge.anomalies().len(), 3);
    }

    #[test]
    fn test_anomaly_records_correct_threshold() {
        let config = CascadeBridgeConfig {
            amplification_threshold: 250.0,
            ..Default::default()
        };
        let bridge = CascadeBridge::with_config(config);
        bridge.record_snapshot(make_snapshot(300.0, 0));
        let anomalies = bridge.anomalies();
        assert_eq!(anomalies.len(), 1);
        assert!((anomalies[0].threshold - 250.0).abs() < f64::EPSILON);
        assert!((anomalies[0].amplification - 300.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_anomaly_records_open_breakers() {
        let bridge = CascadeBridge::new();
        bridge.record_snapshot(make_snapshot(999.0, 5));
        let anomalies = bridge.anomalies();
        assert_eq!(anomalies.len(), 1);
        assert_eq!(anomalies[0].open_breakers, 5);
    }

    #[test]
    fn test_check_anomaly_returns_none_below_threshold() {
        let bridge = CascadeBridge::new();
        bridge.record_snapshot(make_snapshot(100.0, 0));
        assert!(bridge.check_anomaly().is_none());
    }

    #[test]
    fn test_check_anomaly_returns_none_empty() {
        let bridge = CascadeBridge::new();
        assert!(bridge.check_anomaly().is_none());
    }

    #[test]
    fn test_check_anomaly_returns_correct_stage_count() {
        let bridge = CascadeBridge::new();
        bridge.record_snapshot(make_snapshot(600.0, 0));
        if let Some((_, stages, _)) = bridge.check_anomaly() {
            assert_eq!(stages, CASCADE_STAGE_COUNT);
        } else {
            panic!("expected anomaly");
        }
    }

    #[test]
    fn test_degraded_at_exactly_max_failures() {
        let bridge = CascadeBridge::new();
        for _ in 0..MAX_CONSECUTIVE_FAILURES {
            bridge.record_failure();
        }
        assert!(bridge.is_degraded());
    }

    #[test]
    fn test_not_degraded_one_below_max_failures() {
        let bridge = CascadeBridge::new();
        for _ in 0..(MAX_CONSECUTIVE_FAILURES - 1) {
            bridge.record_failure();
        }
        assert!(!bridge.is_degraded());
    }

    #[test]
    fn test_health_degraded_flag() {
        let bridge = CascadeBridge::new();
        for _ in 0..MAX_CONSECUTIVE_FAILURES {
            bridge.record_failure();
        }
        let h = bridge.health();
        assert!(h.degraded);
        assert_eq!(h.consecutive_failures, MAX_CONSECUTIVE_FAILURES);
    }

    #[test]
    fn test_health_consecutive_failures_reported() {
        let bridge = CascadeBridge::new();
        bridge.record_failure();
        bridge.record_failure();
        bridge.record_failure();
        let h = bridge.health();
        assert_eq!(h.consecutive_failures, 3);
    }

    #[test]
    fn test_health_open_breakers_from_latest() {
        let bridge = CascadeBridge::new();
        bridge.record_snapshot(make_snapshot(10.0, 3));
        let h = bridge.health();
        assert_eq!(h.open_breakers, 3);
    }

    #[test]
    fn test_health_max_amplification() {
        let bridge = CascadeBridge::new();
        bridge.record_snapshot(make_snapshot(10.0, 0));
        bridge.record_snapshot(make_snapshot(400.0, 0));
        bridge.record_snapshot(make_snapshot(50.0, 0));
        let h = bridge.health();
        assert!((h.max_amplification - 400.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_health_avg_amplification_single_snapshot() {
        let bridge = CascadeBridge::new();
        bridge.record_snapshot(make_snapshot(42.0, 0));
        let h = bridge.health();
        assert!((h.avg_amplification - 42.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_reset_clears_anomalies() {
        let bridge = CascadeBridge::new();
        bridge.record_snapshot(make_snapshot(999.0, 0));
        assert!(!bridge.anomalies().is_empty());
        bridge.reset();
        assert!(bridge.anomalies().is_empty());
    }

    #[test]
    fn test_reset_clears_snapshots() {
        let bridge = CascadeBridge::new();
        bridge.record_snapshot(make_snapshot(10.0, 0));
        bridge.reset();
        assert_eq!(bridge.window_size(), 0);
        assert!(bridge.latest_snapshot().is_none());
    }

    #[test]
    fn test_reset_clears_failures() {
        let bridge = CascadeBridge::new();
        bridge.record_failure();
        bridge.record_failure();
        bridge.reset();
        assert_eq!(bridge.consecutive_failures(), 0);
    }

    #[test]
    fn test_snapshot_with_stages() {
        let snap = CascadePipelineSnapshot {
            stages: vec![
                CascadeStageSnapshot {
                    stage_index: 0,
                    amplification: 1.5,
                    circuit_breaker_open: false,
                    signals_processed: 100,
                },
                CascadeStageSnapshot {
                    stage_index: 1,
                    amplification: 2.0,
                    circuit_breaker_open: true,
                    signals_processed: 50,
                },
            ],
            total_amplification: 3.5,
            open_breakers: 1,
            timestamp: Utc::now(),
        };
        let bridge = CascadeBridge::new();
        bridge.record_snapshot(snap);
        let latest = bridge.latest_snapshot();
        assert!(latest.is_some());
        let s = latest.unwrap_or_else(|| make_snapshot(0.0, 0));
        assert_eq!(s.stages.len(), 2);
    }

    #[test]
    fn test_stage_snapshot_clone() {
        let stage = CascadeStageSnapshot {
            stage_index: 5,
            amplification: 2.5,
            circuit_breaker_open: true,
            signals_processed: 999,
        };
        let cloned = stage.clone();
        assert_eq!(cloned.stage_index, 5);
        assert!((cloned.amplification - 2.5).abs() < f64::EPSILON);
        assert!(cloned.circuit_breaker_open);
        assert_eq!(cloned.signals_processed, 999);
    }

    #[test]
    fn test_cascade_anomaly_clone() {
        let anomaly = CascadeAnomaly {
            amplification: 1000.0,
            threshold: 500.0,
            open_breakers: 4,
            detected_at: Utc::now(),
        };
        let cloned = anomaly.clone();
        assert!((cloned.amplification - 1000.0).abs() < f64::EPSILON);
        assert_eq!(cloned.open_breakers, 4);
    }

    #[test]
    fn test_cascade_health_clone() {
        let bridge = CascadeBridge::new();
        bridge.record_snapshot(make_snapshot(42.0, 1));
        let h = bridge.health();
        let cloned = h.clone();
        assert!((cloned.current_amplification - 42.0).abs() < f64::EPSILON);
        assert_eq!(cloned.open_breakers, 1);
    }

    #[test]
    fn test_config_serialization() {
        let config = CascadeBridgeConfig::default();
        let json = serde_json::to_string(&config);
        assert!(json.is_ok());
        let parsed: std::result::Result<CascadeBridgeConfig, _> =
            serde_json::from_str(&json.unwrap_or_default());
        assert!(parsed.is_ok());
    }

    #[test]
    fn test_pipeline_snapshot_serialization() {
        let snap = make_snapshot(42.0, 2);
        let json = serde_json::to_string(&snap);
        assert!(json.is_ok());
    }

    #[test]
    fn test_anomaly_serialization() {
        let anomaly = CascadeAnomaly {
            amplification: 600.0,
            threshold: 500.0,
            open_breakers: 1,
            detected_at: Utc::now(),
        };
        let json = serde_json::to_string(&anomaly);
        assert!(json.is_ok());
    }

    #[test]
    fn test_health_serialization() {
        let bridge = CascadeBridge::new();
        bridge.record_snapshot(make_snapshot(10.0, 0));
        let h = bridge.health();
        let json = serde_json::to_string(&h);
        assert!(json.is_ok());
    }

    #[test]
    fn test_record_snapshot_alternating_normal_anomaly() {
        let bridge = CascadeBridge::new();
        bridge.record_snapshot(make_snapshot(100.0, 0)); // normal
        bridge.record_snapshot(make_snapshot(600.0, 1)); // anomaly
        bridge.record_snapshot(make_snapshot(200.0, 0)); // normal
        bridge.record_snapshot(make_snapshot(700.0, 2)); // anomaly
        assert_eq!(bridge.anomalies().len(), 2);
        assert_eq!(bridge.window_size(), 4);
    }

    #[test]
    fn test_failure_then_success_then_failure() {
        let bridge = CascadeBridge::new();
        bridge.record_failure();
        bridge.record_failure();
        assert_eq!(bridge.consecutive_failures(), 2);
        bridge.record_snapshot(make_snapshot(1.0, 0)); // resets
        assert_eq!(bridge.consecutive_failures(), 0);
        bridge.record_failure();
        assert_eq!(bridge.consecutive_failures(), 1);
    }

    #[test]
    fn test_health_not_anomaly_when_below_threshold() {
        let bridge = CascadeBridge::new();
        bridge.record_snapshot(make_snapshot(499.0, 0));
        let h = bridge.health();
        assert!(!h.anomaly_detected);
    }

    #[test]
    fn test_health_anomaly_at_exact_threshold() {
        let bridge = CascadeBridge::new();
        // At exactly the threshold, it should NOT be anomalous (> not >=)
        bridge.record_snapshot(make_snapshot(500.0, 0));
        let h = bridge.health();
        assert!(!h.anomaly_detected);
    }

    #[test]
    fn test_health_anomaly_just_above_threshold() {
        let bridge = CascadeBridge::new();
        bridge.record_snapshot(make_snapshot(500.1, 0));
        let h = bridge.health();
        assert!(h.anomaly_detected);
    }

    #[test]
    fn test_window_capacity_one() {
        let config = CascadeBridgeConfig {
            window_capacity: 1,
            ..Default::default()
        };
        let bridge = CascadeBridge::with_config(config);
        bridge.record_snapshot(make_snapshot(10.0, 0));
        bridge.record_snapshot(make_snapshot(20.0, 0));
        assert_eq!(bridge.window_size(), 1);
        let latest = bridge.latest_snapshot();
        assert!(latest.is_some());
        let snap = latest.unwrap_or_else(|| make_snapshot(0.0, 0));
        assert!((snap.total_amplification - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_many_failures_beyond_degraded() {
        let bridge = CascadeBridge::new();
        for _ in 0..50 {
            bridge.record_failure();
        }
        assert!(bridge.is_degraded());
        assert_eq!(bridge.consecutive_failures(), 50);
    }

    #[test]
    fn test_config_clone() {
        let config = CascadeBridgeConfig {
            diagnostics_url: "http://test:1234".to_string(),
            poll_interval_secs: 42,
            window_capacity: 100,
            amplification_threshold: 999.0,
        };
        let cloned = config.clone();
        assert_eq!(cloned.diagnostics_url, "http://test:1234");
        assert_eq!(cloned.poll_interval_secs, 42);
        assert_eq!(cloned.window_capacity, 100);
        assert!((cloned.amplification_threshold - 999.0).abs() < f64::EPSILON);
    }
}
