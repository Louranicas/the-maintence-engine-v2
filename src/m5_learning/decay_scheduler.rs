//! # M41: Decay Scheduler
//!
//! Coordinates cross-service decay by periodically triggering the SYNTHEX V3
//! decay endpoint. Tracks compliance and falls back gracefully if SYNTHEX
//! is unreachable.
//!
//! ## Layer: L5 (Learning)
//! ## Dependencies: SYNTHEX V3 `POST /v3/decay/trigger`
//!
//! ## Scheduling Configuration
//!
//! - Default interval: 300 seconds (5 minutes)
//! - Fail-silent: continues operating if SYNTHEX is unreachable
//! - Compliance tracking: records trigger success/failure history
//!
//! ## Related Documentation
//! - [V3 Decay Controller](../../developer_environment_manager/synthex/src/v3/decay.rs)

use std::collections::VecDeque;

use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

use crate::Result;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Default decay trigger interval in seconds.
const DEFAULT_TRIGGER_INTERVAL_SECS: u64 = 300;

/// Maximum history of decay events retained.
const DEFAULT_HISTORY_CAPACITY: usize = 200;

/// Maximum consecutive failures before entering degraded mode.
const MAX_CONSECUTIVE_FAILURES: u32 = 10;

/// Default SYNTHEX V3 decay trigger endpoint.
const DEFAULT_DECAY_URL: &str = "http://localhost:8090/v3/decay/trigger";

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Configuration for the decay scheduler.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DecaySchedulerConfig {
    /// URL of the SYNTHEX V3 decay trigger endpoint.
    pub decay_url: String,
    /// Trigger interval in seconds.
    pub trigger_interval_secs: u64,
    /// Maximum event history capacity.
    pub history_capacity: usize,
}

impl Default for DecaySchedulerConfig {
    fn default() -> Self {
        Self {
            decay_url: DEFAULT_DECAY_URL.to_string(),
            trigger_interval_secs: DEFAULT_TRIGGER_INTERVAL_SECS,
            history_capacity: DEFAULT_HISTORY_CAPACITY,
        }
    }
}

// ---------------------------------------------------------------------------
// Decay Event
// ---------------------------------------------------------------------------

/// A recorded decay trigger event.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DecayEvent {
    /// Whether the trigger was successful.
    pub success: bool,
    /// Number of pathways decayed (from V3 response), if available.
    pub pathways_decayed: Option<u64>,
    /// Number of pathways pruned (from V3 response), if available.
    pub pathways_pruned: Option<u64>,
    /// Average strength after decay (from V3 response), if available.
    pub avg_strength_after: Option<f64>,
    /// Duration of the decay cycle in milliseconds, if available.
    pub duration_ms: Option<u64>,
    /// Error message if the trigger failed.
    pub error: Option<String>,
    /// Timestamp of the event.
    pub timestamp: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Decay Compliance
// ---------------------------------------------------------------------------

/// Summary of decay scheduling compliance.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DecayCompliance {
    /// Total triggers attempted.
    pub total_attempts: u64,
    /// Successful triggers.
    pub successes: u64,
    /// Failed triggers.
    pub failures: u64,
    /// Compliance ratio [0.0, 1.0].
    pub compliance_ratio: f64,
    /// Number of consecutive failures.
    pub consecutive_failures: u32,
    /// Whether the scheduler is in degraded mode.
    pub degraded: bool,
    /// Most recent decay event, if any.
    pub last_event: Option<DecayEvent>,
}

// ---------------------------------------------------------------------------
// Decay Scheduler
// ---------------------------------------------------------------------------

/// M41: Decay scheduler for cross-service V3 integration.
///
/// Periodically triggers the SYNTHEX V3 decay endpoint and tracks
/// compliance. Operates fail-silent: if SYNTHEX is unreachable, the
/// scheduler records the failure and continues.
///
/// # Thread Safety
///
/// All mutable state is protected by `parking_lot::RwLock`.
pub struct DecayScheduler {
    /// History of decay trigger events.
    history: RwLock<VecDeque<DecayEvent>>,
    /// Running count of successful triggers.
    successes: RwLock<u64>,
    /// Running count of failed triggers.
    failures: RwLock<u64>,
    /// Number of consecutive failures.
    consecutive_failures: RwLock<u32>,
    /// Configuration.
    config: DecaySchedulerConfig,
}

impl DecayScheduler {
    /// Create a new decay scheduler with default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self::with_config(DecaySchedulerConfig::default())
    }

    /// Create a new decay scheduler with the given configuration.
    #[must_use]
    pub fn with_config(config: DecaySchedulerConfig) -> Self {
        Self {
            history: RwLock::new(VecDeque::with_capacity(config.history_capacity)),
            successes: RwLock::new(0),
            failures: RwLock::new(0),
            consecutive_failures: RwLock::new(0),
            config,
        }
    }

    /// Record a successful decay trigger.
    pub fn record_success(
        &self,
        pathways_decayed: Option<u64>,
        pathways_pruned: Option<u64>,
        avg_strength_after: Option<f64>,
        duration_ms: Option<u64>,
    ) {
        let event = DecayEvent {
            success: true,
            pathways_decayed,
            pathways_pruned,
            avg_strength_after,
            duration_ms,
            error: None,
            timestamp: Utc::now(),
        };

        self.append_event(event);
        *self.successes.write() += 1;
        *self.consecutive_failures.write() = 0;
    }

    /// Record a failed decay trigger.
    pub fn record_failure(&self, error: String) {
        let event = DecayEvent {
            success: false,
            pathways_decayed: None,
            pathways_pruned: None,
            avg_strength_after: None,
            duration_ms: None,
            error: Some(error),
            timestamp: Utc::now(),
        };

        self.append_event(event);
        *self.failures.write() += 1;
        *self.consecutive_failures.write() += 1;
    }

    /// Get the number of consecutive failures.
    #[must_use]
    pub fn consecutive_failures(&self) -> u32 {
        *self.consecutive_failures.read()
    }

    /// Check whether the scheduler is in degraded mode.
    #[must_use]
    pub fn is_degraded(&self) -> bool {
        *self.consecutive_failures.read() >= MAX_CONSECUTIVE_FAILURES
    }

    /// Get a compliance summary.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn compliance(&self) -> DecayCompliance {
        let successes = *self.successes.read();
        let failures = *self.failures.read();
        let total = successes + failures;
        let ratio = if total > 0 {
            successes as f64 / total as f64
        } else {
            1.0
        };
        let consec = *self.consecutive_failures.read();

        DecayCompliance {
            total_attempts: total,
            successes,
            failures,
            compliance_ratio: ratio,
            consecutive_failures: consec,
            degraded: consec >= MAX_CONSECUTIVE_FAILURES,
            last_event: self.history.read().back().cloned(),
        }
    }

    /// Get the most recent `n` decay events (newest first).
    #[must_use]
    pub fn recent_events(&self, n: usize) -> Vec<DecayEvent> {
        self.history.read().iter().rev().take(n).cloned().collect()
    }

    /// Get a reference to the configuration.
    #[must_use]
    pub const fn config(&self) -> &DecaySchedulerConfig {
        &self.config
    }

    /// Get the total number of events in the history.
    #[must_use]
    pub fn history_len(&self) -> usize {
        self.history.read().len()
    }

    /// Trigger a decay cycle (synchronous stub).
    ///
    /// In production, this would POST to `config.decay_url`.
    /// The actual HTTP call is made from the background task in `main.rs`.
    ///
    /// # Errors
    ///
    /// Returns `Error::Other` as this is a stub.
    pub fn trigger(&self) -> Result<()> {
        Err(crate::Error::Other(
            "trigger() is a stub; use the background task for live triggering".into(),
        ))
    }

    /// Clear all history and reset counters.
    pub fn reset(&self) {
        self.history.write().clear();
        *self.successes.write() = 0;
        *self.failures.write() = 0;
        *self.consecutive_failures.write() = 0;
    }

    // ---- Private helpers ----

    /// Append an event to the bounded history.
    fn append_event(&self, event: DecayEvent) {
        let mut guard = self.history.write();
        if guard.len() >= self.config.history_capacity {
            guard.pop_front();
        }
        guard.push_back(event);
    }
}

impl Default for DecayScheduler {
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

    #[test]
    fn test_new_empty() {
        let sched = DecayScheduler::new();
        assert_eq!(sched.history_len(), 0);
        assert_eq!(sched.consecutive_failures(), 0);
        assert!(!sched.is_degraded());
    }

    #[test]
    fn test_record_success() {
        let sched = DecayScheduler::new();
        sched.record_success(Some(10), Some(2), Some(0.45), Some(150));
        assert_eq!(sched.history_len(), 1);

        let compliance = sched.compliance();
        assert_eq!(compliance.total_attempts, 1);
        assert_eq!(compliance.successes, 1);
        assert_eq!(compliance.failures, 0);
        assert!((compliance.compliance_ratio - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_record_failure() {
        let sched = DecayScheduler::new();
        sched.record_failure("connection refused".to_string());
        assert_eq!(sched.history_len(), 1);
        assert_eq!(sched.consecutive_failures(), 1);

        let compliance = sched.compliance();
        assert_eq!(compliance.failures, 1);
        assert!((compliance.compliance_ratio).abs() < f64::EPSILON);
    }

    #[test]
    fn test_failure_reset_on_success() {
        let sched = DecayScheduler::new();
        sched.record_failure("err1".to_string());
        sched.record_failure("err2".to_string());
        assert_eq!(sched.consecutive_failures(), 2);

        sched.record_success(None, None, None, None);
        assert_eq!(sched.consecutive_failures(), 0);
    }

    #[test]
    fn test_degraded_mode() {
        let sched = DecayScheduler::new();
        for i in 0..MAX_CONSECUTIVE_FAILURES {
            sched.record_failure(format!("error {i}"));
        }
        assert!(sched.is_degraded());
        assert!(sched.compliance().degraded);
    }

    #[test]
    fn test_compliance_ratio() {
        let sched = DecayScheduler::new();
        sched.record_success(None, None, None, None);
        sched.record_success(None, None, None, None);
        sched.record_failure("err".to_string());

        let compliance = sched.compliance();
        assert_eq!(compliance.total_attempts, 3);
        // 2/3 ≈ 0.6667
        assert!((compliance.compliance_ratio - 2.0 / 3.0).abs() < 1e-10);
    }

    #[test]
    fn test_history_capacity() {
        let config = DecaySchedulerConfig {
            history_capacity: 5,
            ..Default::default()
        };
        let sched = DecayScheduler::with_config(config);
        for _ in 0..10 {
            sched.record_success(None, None, None, None);
        }
        assert_eq!(sched.history_len(), 5);
    }

    #[test]
    fn test_recent_events_order() {
        let sched = DecayScheduler::new();
        sched.record_success(Some(1), None, None, None);
        sched.record_success(Some(2), None, None, None);
        sched.record_success(Some(3), None, None, None);

        let recent = sched.recent_events(2);
        assert_eq!(recent.len(), 2);
        // Most recent first
        assert_eq!(recent[0].pathways_decayed, Some(3));
        assert_eq!(recent[1].pathways_decayed, Some(2));
    }

    #[test]
    fn test_reset() {
        let sched = DecayScheduler::new();
        sched.record_success(None, None, None, None);
        sched.record_failure("err".to_string());
        sched.reset();
        assert_eq!(sched.history_len(), 0);
        assert_eq!(sched.consecutive_failures(), 0);
        assert_eq!(sched.compliance().total_attempts, 0);
    }

    #[test]
    fn test_trigger_stub() {
        let sched = DecayScheduler::new();
        assert!(sched.trigger().is_err());
    }

    #[test]
    fn test_config_accessor() {
        let sched = DecayScheduler::new();
        assert_eq!(sched.config().trigger_interval_secs, DEFAULT_TRIGGER_INTERVAL_SECS);
    }

    #[test]
    fn test_compliance_empty() {
        let sched = DecayScheduler::new();
        let compliance = sched.compliance();
        assert_eq!(compliance.total_attempts, 0);
        assert!((compliance.compliance_ratio - 1.0).abs() < f64::EPSILON);
        assert!(compliance.last_event.is_none());
    }

    #[test]
    fn test_last_event_in_compliance() {
        let sched = DecayScheduler::new();
        sched.record_success(Some(42), Some(3), Some(0.3), Some(200));

        let compliance = sched.compliance();
        assert!(compliance.last_event.is_some());
        if let Some(ref evt) = compliance.last_event {
            assert!(evt.success);
            assert_eq!(evt.pathways_decayed, Some(42));
        }
    }
}
