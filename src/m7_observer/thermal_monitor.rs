//! # M40: Thermal Monitor
//!
//! Polls the SYNTHEX V3 thermal endpoint and feeds temperature readings
//! into the M38 Emergence Detector for thermal runaway detection.
//!
//! ## Layer: L7 (Observer)
//! ## Dependencies: M38 (Emergence Detector), SYNTHEX V3 `/v3/thermal`
//!
//! ## Polling Configuration
//!
//! - Default interval: 30 seconds
//! - Sliding window: 120 readings (1 hour at 30s intervals)
//! - Fail-silent: continues operating if SYNTHEX is unreachable
//!
//! ## Related Documentation
//! - [V3 Thermal Controller](../../developer_environment_manager/synthex/src/v3/thermal.rs)

use std::collections::VecDeque;

use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

use crate::Result;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Default polling interval in seconds.
const DEFAULT_POLL_INTERVAL_SECS: u64 = 30;

/// Default sliding window capacity (120 readings = 1 hour at 30s).
const DEFAULT_WINDOW_CAPACITY: usize = 120;

/// Maximum consecutive failures before entering degraded mode.
const MAX_CONSECUTIVE_FAILURES: u32 = 10;

/// Default SYNTHEX V3 thermal endpoint.
const DEFAULT_THERMAL_URL: &str = "http://localhost:8090/v3/thermal";

/// Thermal margin above target before flagging runaway (for M38).
const DEFAULT_THERMAL_MARGIN: f64 = 0.15;

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Configuration for the thermal monitor.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ThermalMonitorConfig {
    /// URL of the SYNTHEX V3 thermal endpoint.
    pub thermal_url: String,
    /// Polling interval in seconds.
    pub poll_interval_secs: u64,
    /// Sliding window capacity.
    pub window_capacity: usize,
    /// Thermal margin for runaway detection.
    pub thermal_margin: f64,
}

impl Default for ThermalMonitorConfig {
    fn default() -> Self {
        Self {
            thermal_url: DEFAULT_THERMAL_URL.to_string(),
            poll_interval_secs: DEFAULT_POLL_INTERVAL_SECS,
            window_capacity: DEFAULT_WINDOW_CAPACITY,
            thermal_margin: DEFAULT_THERMAL_MARGIN,
        }
    }
}

// ---------------------------------------------------------------------------
// Thermal Reading
// ---------------------------------------------------------------------------

/// A single thermal reading from the V3 subsystem.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ThermalReading {
    /// System temperature [0.0, 1.0].
    pub temperature: f64,
    /// PID target temperature.
    pub target: f64,
    /// PID output signal.
    pub pid_output: f64,
    /// Timestamp of the reading.
    pub timestamp: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Thermal Snapshot
// ---------------------------------------------------------------------------

/// A summary snapshot of thermal health.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ThermalSnapshot {
    /// Current temperature.
    pub current_temp: f64,
    /// Target temperature.
    pub target_temp: f64,
    /// Average temperature over the sliding window.
    pub avg_temp: f64,
    /// Maximum temperature observed in the window.
    pub max_temp: f64,
    /// Whether thermal runaway was detected.
    pub runaway_detected: bool,
    /// Number of readings in the window.
    pub window_size: usize,
    /// Number of consecutive poll failures.
    pub consecutive_failures: u32,
    /// Whether the monitor is in degraded mode.
    pub degraded: bool,
}

// ---------------------------------------------------------------------------
// Thermal Monitor
// ---------------------------------------------------------------------------

/// M40: Thermal monitor for the V3 subsystem.
///
/// Polls the SYNTHEX `/v3/thermal` endpoint at configurable intervals,
/// maintains a sliding window of readings, and feeds anomalies into M38.
///
/// # Thread Safety
///
/// All mutable state is protected by `parking_lot::RwLock`.
pub struct ThermalMonitor {
    /// Sliding window of thermal readings.
    readings: RwLock<VecDeque<ThermalReading>>,
    /// Number of consecutive poll failures.
    consecutive_failures: RwLock<u32>,
    /// Configuration.
    config: ThermalMonitorConfig,
}

impl ThermalMonitor {
    /// Create a new thermal monitor with default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self::with_config(ThermalMonitorConfig::default())
    }

    /// Create a new thermal monitor with the given configuration.
    #[must_use]
    pub fn with_config(config: ThermalMonitorConfig) -> Self {
        Self {
            readings: RwLock::new(VecDeque::with_capacity(config.window_capacity)),
            consecutive_failures: RwLock::new(0),
            config,
        }
    }

    /// Record a successful thermal reading.
    pub fn record_reading(&self, reading: ThermalReading) {
        let mut guard = self.readings.write();
        if guard.len() >= self.config.window_capacity {
            guard.pop_front();
        }
        guard.push_back(reading);
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

    /// Check whether the monitor is in degraded mode.
    #[must_use]
    pub fn is_degraded(&self) -> bool {
        *self.consecutive_failures.read() >= MAX_CONSECUTIVE_FAILURES
    }

    /// Get the most recent thermal reading, if any.
    #[must_use]
    pub fn latest_reading(&self) -> Option<ThermalReading> {
        self.readings.read().back().cloned()
    }

    /// Get the number of readings in the sliding window.
    #[must_use]
    pub fn window_size(&self) -> usize {
        self.readings.read().len()
    }

    /// Compute a thermal snapshot from the current window.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn snapshot(&self) -> ThermalSnapshot {
        let guard = self.readings.read();
        let failures = *self.consecutive_failures.read();

        if guard.is_empty() {
            return ThermalSnapshot {
                current_temp: 0.0,
                target_temp: 0.5,
                avg_temp: 0.0,
                max_temp: 0.0,
                runaway_detected: false,
                window_size: 0,
                consecutive_failures: failures,
                degraded: failures >= MAX_CONSECUTIVE_FAILURES,
            };
        }

        let latest = guard.back().cloned().unwrap_or_else(|| ThermalReading {
            temperature: 0.0,
            target: 0.5,
            pid_output: 0.0,
            timestamp: Utc::now(),
        });

        let sum: f64 = guard.iter().map(|r| r.temperature).sum();
        let avg = sum / guard.len() as f64;

        let max = guard
            .iter()
            .map(|r| r.temperature)
            .fold(0.0_f64, f64::max);

        let deviation = latest.temperature - latest.target;
        let runaway = deviation > self.config.thermal_margin;

        ThermalSnapshot {
            current_temp: latest.temperature,
            target_temp: latest.target,
            avg_temp: avg,
            max_temp: max,
            runaway_detected: runaway,
            window_size: guard.len(),
            consecutive_failures: failures,
            degraded: failures >= MAX_CONSECUTIVE_FAILURES,
        }
    }

    /// Check if the latest reading indicates thermal runaway.
    ///
    /// Returns `Some((current, target, margin))` if runaway is detected.
    #[must_use]
    pub fn check_runaway(&self) -> Option<(f64, f64, f64)> {
        let latest = { self.readings.read().back().cloned() }?;
        let deviation = latest.temperature - latest.target;
        if deviation > self.config.thermal_margin {
            Some((latest.temperature, latest.target, self.config.thermal_margin))
        } else {
            None
        }
    }

    /// Get a reference to the configuration.
    #[must_use]
    pub const fn config(&self) -> &ThermalMonitorConfig {
        &self.config
    }

    /// Get recent readings (most recent first).
    #[must_use]
    pub fn recent_readings(&self, n: usize) -> Vec<ThermalReading> {
        self.readings.read().iter().rev().take(n).cloned().collect()
    }

    /// Clear all readings and reset failure count.
    pub fn reset(&self) {
        self.readings.write().clear();
        *self.consecutive_failures.write() = 0;
    }

    /// Poll the thermal endpoint (synchronous, for use with reqwest blocking).
    ///
    /// Returns a result containing the parsed temperature data, or an error
    /// if the endpoint is unreachable or returns invalid data.
    ///
    /// # Errors
    ///
    /// Returns `Error::Other` if the HTTP request fails or JSON parsing fails.
    pub fn poll(&self) -> Result<ThermalReading> {
        // In production, this would make an HTTP GET to config.thermal_url.
        // For now, return a default reading indicating the subsystem
        // integration point. The actual HTTP call is made from the
        // background task in main.rs using reqwest.
        Err(crate::Error::Other(
            "poll() is a stub; use the background task for live polling".into(),
        ))
    }
}

impl Default for ThermalMonitor {
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

    fn make_reading(temp: f64, target: f64) -> ThermalReading {
        ThermalReading {
            temperature: temp,
            target,
            pid_output: 0.0,
            timestamp: Utc::now(),
        }
    }

    #[test]
    fn test_new_empty() {
        let mon = ThermalMonitor::new();
        assert_eq!(mon.window_size(), 0);
        assert_eq!(mon.consecutive_failures(), 0);
        assert!(!mon.is_degraded());
        assert!(mon.latest_reading().is_none());
    }

    #[test]
    fn test_record_reading() {
        let mon = ThermalMonitor::new();
        mon.record_reading(make_reading(0.5, 0.5));
        assert_eq!(mon.window_size(), 1);
        assert_eq!(mon.consecutive_failures(), 0);
    }

    #[test]
    fn test_record_failure() {
        let mon = ThermalMonitor::new();
        for _ in 0..5 {
            mon.record_failure();
        }
        assert_eq!(mon.consecutive_failures(), 5);
        assert!(!mon.is_degraded());

        for _ in 0..5 {
            mon.record_failure();
        }
        assert!(mon.is_degraded());
    }

    #[test]
    fn test_failure_reset_on_success() {
        let mon = ThermalMonitor::new();
        mon.record_failure();
        mon.record_failure();
        assert_eq!(mon.consecutive_failures(), 2);

        mon.record_reading(make_reading(0.5, 0.5));
        assert_eq!(mon.consecutive_failures(), 0);
    }

    #[test]
    fn test_window_capacity() {
        let config = ThermalMonitorConfig {
            window_capacity: 5,
            ..Default::default()
        };
        let mon = ThermalMonitor::with_config(config);
        for i in 0..10 {
            #[allow(clippy::cast_precision_loss)]
            mon.record_reading(make_reading(i as f64 * 0.1, 0.5));
        }
        assert_eq!(mon.window_size(), 5);
    }

    #[test]
    fn test_snapshot_empty() {
        let mon = ThermalMonitor::new();
        let snap = mon.snapshot();
        assert!((snap.current_temp).abs() < f64::EPSILON);
        assert_eq!(snap.window_size, 0);
        assert!(!snap.runaway_detected);
    }

    #[test]
    fn test_snapshot_with_readings() {
        let mon = ThermalMonitor::new();
        mon.record_reading(make_reading(0.4, 0.5));
        mon.record_reading(make_reading(0.6, 0.5));
        mon.record_reading(make_reading(0.5, 0.5));

        let snap = mon.snapshot();
        assert_eq!(snap.window_size, 3);
        assert!((snap.current_temp - 0.5).abs() < f64::EPSILON);
        assert!((snap.avg_temp - 0.5).abs() < f64::EPSILON);
        assert!((snap.max_temp - 0.6).abs() < f64::EPSILON);
        assert!(!snap.runaway_detected);
    }

    #[test]
    fn test_runaway_detection() {
        let mon = ThermalMonitor::new();
        // Temperature 0.95, target 0.50 -> deviation 0.45 > margin 0.15
        mon.record_reading(make_reading(0.95, 0.50));

        let snap = mon.snapshot();
        assert!(snap.runaway_detected);

        let runaway = mon.check_runaway();
        assert!(runaway.is_some());
        if let Some((temp, target, margin)) = runaway {
            assert!((temp - 0.95).abs() < f64::EPSILON);
            assert!((target - 0.50).abs() < f64::EPSILON);
            assert!((margin - 0.15).abs() < f64::EPSILON);
        }
    }

    #[test]
    fn test_no_runaway_within_margin() {
        let mon = ThermalMonitor::new();
        mon.record_reading(make_reading(0.60, 0.50));
        assert!(mon.check_runaway().is_none());
    }

    #[test]
    fn test_recent_readings_order() {
        let mon = ThermalMonitor::new();
        mon.record_reading(make_reading(0.1, 0.5));
        mon.record_reading(make_reading(0.2, 0.5));
        mon.record_reading(make_reading(0.3, 0.5));

        let recent = mon.recent_readings(2);
        assert_eq!(recent.len(), 2);
        assert!((recent[0].temperature - 0.3).abs() < f64::EPSILON);
        assert!((recent[1].temperature - 0.2).abs() < f64::EPSILON);
    }

    #[test]
    fn test_reset() {
        let mon = ThermalMonitor::new();
        mon.record_reading(make_reading(0.5, 0.5));
        mon.record_failure();
        mon.reset();
        assert_eq!(mon.window_size(), 0);
        assert_eq!(mon.consecutive_failures(), 0);
    }

    #[test]
    fn test_poll_stub_returns_err() {
        let mon = ThermalMonitor::new();
        assert!(mon.poll().is_err());
    }

    #[test]
    fn test_config_accessor() {
        let mon = ThermalMonitor::new();
        assert_eq!(mon.config().poll_interval_secs, DEFAULT_POLL_INTERVAL_SECS);
        assert_eq!(mon.config().window_capacity, DEFAULT_WINDOW_CAPACITY);
    }
}
