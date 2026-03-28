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

    // ---------------------------------------------------------------
    // Additional tests to reach 50+
    // ---------------------------------------------------------------

    #[test]
    fn test_default_impl() {
        let sched = DecayScheduler::default();
        assert_eq!(sched.history_len(), 0);
    }

    #[test]
    fn test_config_default_url() {
        let config = DecaySchedulerConfig::default();
        assert_eq!(config.decay_url, DEFAULT_DECAY_URL);
    }

    #[test]
    fn test_config_default_interval() {
        let config = DecaySchedulerConfig::default();
        assert_eq!(config.trigger_interval_secs, DEFAULT_TRIGGER_INTERVAL_SECS);
    }

    #[test]
    fn test_config_default_capacity() {
        let config = DecaySchedulerConfig::default();
        assert_eq!(config.history_capacity, DEFAULT_HISTORY_CAPACITY);
    }

    #[test]
    fn test_success_event_fields() {
        let sched = DecayScheduler::new();
        sched.record_success(Some(5), Some(1), Some(0.45), Some(100));
        let events = sched.recent_events(1);
        assert_eq!(events.len(), 1);
        assert!(events[0].success);
        assert_eq!(events[0].pathways_decayed, Some(5));
        assert_eq!(events[0].pathways_pruned, Some(1));
        assert_eq!(events[0].duration_ms, Some(100));
        assert!(events[0].error.is_none());
    }

    #[test]
    fn test_failure_event_fields() {
        let sched = DecayScheduler::new();
        sched.record_failure("timeout".to_string());
        let events = sched.recent_events(1);
        assert_eq!(events.len(), 1);
        assert!(!events[0].success);
        assert!(events[0].pathways_decayed.is_none());
        assert_eq!(events[0].error.as_deref(), Some("timeout"));
    }

    #[test]
    fn test_success_with_none_fields() {
        let sched = DecayScheduler::new();
        sched.record_success(None, None, None, None);
        let events = sched.recent_events(1);
        assert!(events[0].success);
        assert!(events[0].pathways_decayed.is_none());
        assert!(events[0].avg_strength_after.is_none());
    }

    #[test]
    fn test_consecutive_failures_increment() {
        let sched = DecayScheduler::new();
        for i in 1..=5 {
            sched.record_failure(format!("err{i}"));
            assert_eq!(sched.consecutive_failures(), i);
        }
    }

    #[test]
    fn test_not_degraded_below_threshold() {
        let sched = DecayScheduler::new();
        for _ in 0..(MAX_CONSECUTIVE_FAILURES - 1) {
            sched.record_failure("err".to_string());
        }
        assert!(!sched.is_degraded());
    }

    #[test]
    fn test_degraded_exit_on_success() {
        let sched = DecayScheduler::new();
        for _ in 0..MAX_CONSECUTIVE_FAILURES {
            sched.record_failure("err".to_string());
        }
        assert!(sched.is_degraded());
        sched.record_success(None, None, None, None);
        assert!(!sched.is_degraded());
    }

    #[test]
    fn test_compliance_ratio_all_success() {
        let sched = DecayScheduler::new();
        for _ in 0..10 {
            sched.record_success(None, None, None, None);
        }
        let compliance = sched.compliance();
        assert!((compliance.compliance_ratio - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_compliance_ratio_all_failure() {
        let sched = DecayScheduler::new();
        for _ in 0..10 {
            sched.record_failure("err".to_string());
        }
        let compliance = sched.compliance();
        assert!((compliance.compliance_ratio).abs() < f64::EPSILON);
    }

    #[test]
    fn test_recent_events_empty() {
        let sched = DecayScheduler::new();
        let events = sched.recent_events(5);
        assert!(events.is_empty());
    }

    #[test]
    fn test_recent_events_limited() {
        let sched = DecayScheduler::new();
        for _ in 0..10 {
            sched.record_success(None, None, None, None);
        }
        let events = sched.recent_events(3);
        assert_eq!(events.len(), 3);
    }

    #[test]
    fn test_reset_clears_compliance() {
        let sched = DecayScheduler::new();
        sched.record_success(None, None, None, None);
        sched.record_failure("err".to_string());
        sched.reset();
        let compliance = sched.compliance();
        assert_eq!(compliance.total_attempts, 0);
        assert_eq!(compliance.successes, 0);
        assert_eq!(compliance.failures, 0);
    }

    #[test]
    fn test_custom_config_url() {
        let config = DecaySchedulerConfig {
            decay_url: "http://custom:9999/decay".to_string(),
            ..Default::default()
        };
        let sched = DecayScheduler::with_config(config);
        assert_eq!(sched.config().decay_url, "http://custom:9999/decay");
    }

    #[test]
    fn test_custom_config_interval() {
        let config = DecaySchedulerConfig {
            trigger_interval_secs: 60,
            ..Default::default()
        };
        let sched = DecayScheduler::with_config(config);
        assert_eq!(sched.config().trigger_interval_secs, 60);
    }

    #[test]
    fn test_interleaved_success_failure() {
        let sched = DecayScheduler::new();
        sched.record_success(None, None, None, None);
        sched.record_failure("e1".to_string());
        sched.record_success(None, None, None, None);
        sched.record_failure("e2".to_string());

        let compliance = sched.compliance();
        assert_eq!(compliance.total_attempts, 4);
        assert_eq!(compliance.successes, 2);
        assert_eq!(compliance.failures, 2);
        assert!((compliance.compliance_ratio - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_history_capacity_custom_small() {
        let config = DecaySchedulerConfig {
            history_capacity: 3,
            ..Default::default()
        };
        let sched = DecayScheduler::with_config(config);
        for _ in 0..10 {
            sched.record_success(None, None, None, None);
        }
        assert_eq!(sched.history_len(), 3);
    }

    #[test]
    fn test_compliance_last_event_is_most_recent() {
        let sched = DecayScheduler::new();
        sched.record_success(Some(1), None, None, None);
        sched.record_success(Some(2), None, None, None);
        sched.record_success(Some(99), None, None, None);

        let compliance = sched.compliance();
        if let Some(ref evt) = compliance.last_event {
            assert_eq!(evt.pathways_decayed, Some(99));
        }
    }

    #[test]
    fn test_consecutive_failures_in_compliance() {
        let sched = DecayScheduler::new();
        sched.record_failure("e1".to_string());
        sched.record_failure("e2".to_string());
        sched.record_failure("e3".to_string());

        let compliance = sched.compliance();
        assert_eq!(compliance.consecutive_failures, 3);
    }

    #[test]
    fn test_compliance_degraded_matches_is_degraded() {
        let sched = DecayScheduler::new();
        for _ in 0..MAX_CONSECUTIVE_FAILURES {
            sched.record_failure("err".to_string());
        }
        assert_eq!(sched.compliance().degraded, sched.is_degraded());
    }

    #[test]
    fn test_avg_strength_after_preserved() {
        let sched = DecayScheduler::new();
        sched.record_success(None, None, Some(0.789), None);
        let events = sched.recent_events(1);
        if let Some(avg) = events[0].avg_strength_after {
            assert!((avg - 0.789).abs() < f64::EPSILON);
        }
    }

    #[test]
    fn test_event_timestamp_set() {
        let sched = DecayScheduler::new();
        sched.record_success(None, None, None, None);
        let events = sched.recent_events(1);
        assert_eq!(events.len(), 1);
        // timestamp should be recent
        let diff = chrono::Utc::now() - events[0].timestamp;
        assert!(diff.num_seconds() < 2);
    }

    #[test]
    fn test_trigger_returns_error() {
        let sched = DecayScheduler::new();
        let result = sched.trigger();
        assert!(result.is_err());
    }

    #[test]
    fn test_history_len_after_reset() {
        let sched = DecayScheduler::new();
        sched.record_success(None, None, None, None);
        assert_eq!(sched.history_len(), 1);
        sched.reset();
        assert_eq!(sched.history_len(), 0);
    }

    #[test]
    fn test_concurrent_recording() {
        use std::sync::Arc;
        use std::thread;

        let sched = Arc::new(DecayScheduler::new());
        let mut handles = Vec::new();

        for _ in 0..4 {
            let s = Arc::clone(&sched);
            handles.push(thread::spawn(move || {
                for _ in 0..10 {
                    s.record_success(None, None, None, None);
                }
            }));
        }

        for handle in handles {
            let _ = handle.join();
        }

        let compliance = sched.compliance();
        assert_eq!(compliance.successes, 40);
    }

    #[test]
    fn test_history_capacity_one() {
        let config = DecaySchedulerConfig {
            history_capacity: 1,
            ..Default::default()
        };
        let sched = DecayScheduler::with_config(config);
        sched.record_success(Some(1), None, None, None);
        sched.record_success(Some(2), None, None, None);
        assert_eq!(sched.history_len(), 1);
        let events = sched.recent_events(5);
        assert_eq!(events[0].pathways_decayed, Some(2));
    }

    #[test]
    fn test_failure_error_message_preserved() {
        let sched = DecayScheduler::new();
        sched.record_failure("specific error message".to_string());
        let events = sched.recent_events(1);
        assert_eq!(events[0].error.as_deref(), Some("specific error message"));
    }

    #[test]
    fn test_success_timestamp_recent() {
        let sched = DecayScheduler::new();
        sched.record_success(None, None, None, None);
        let events = sched.recent_events(1);
        let diff = chrono::Utc::now() - events[0].timestamp;
        assert!(diff.num_seconds() < 2);
    }

    #[test]
    fn test_failure_timestamp_recent() {
        let sched = DecayScheduler::new();
        sched.record_failure("err".to_string());
        let events = sched.recent_events(1);
        let diff = chrono::Utc::now() - events[0].timestamp;
        assert!(diff.num_seconds() < 2);
    }

    #[test]
    fn test_recent_events_order_mixed() {
        let sched = DecayScheduler::new();
        sched.record_success(Some(1), None, None, None);
        sched.record_failure("err".to_string());
        sched.record_success(Some(3), None, None, None);

        let events = sched.recent_events(3);
        assert_eq!(events.len(), 3);
        assert_eq!(events[0].pathways_decayed, Some(3));
        assert!(!events[1].success);
        assert_eq!(events[2].pathways_decayed, Some(1));
    }

    #[test]
    fn test_compliance_after_many_operations() {
        let sched = DecayScheduler::new();
        for _ in 0..100 {
            sched.record_success(None, None, None, None);
        }
        for _ in 0..50 {
            sched.record_failure("err".to_string());
        }
        let compliance = sched.compliance();
        assert_eq!(compliance.total_attempts, 150);
        assert_eq!(compliance.successes, 100);
        assert_eq!(compliance.failures, 50);
    }

    #[test]
    fn test_consecutive_failures_exact_threshold() {
        let sched = DecayScheduler::new();
        for _ in 0..(MAX_CONSECUTIVE_FAILURES - 1) {
            sched.record_failure("err".to_string());
        }
        assert!(!sched.is_degraded());
        sched.record_failure("one_more".to_string());
        assert!(sched.is_degraded());
    }

    #[test]
    fn test_reset_and_reuse() {
        let sched = DecayScheduler::new();
        sched.record_success(None, None, None, None);
        sched.reset();
        sched.record_failure("new_err".to_string());
        assert_eq!(sched.history_len(), 1);
        assert_eq!(sched.consecutive_failures(), 1);
    }

    #[test]
    fn test_compliance_ratio_precision() {
        let sched = DecayScheduler::new();
        for _ in 0..3 {
            sched.record_success(None, None, None, None);
        }
        sched.record_failure("err".to_string());
        // 3/4 = 0.75
        let compliance = sched.compliance();
        assert!((compliance.compliance_ratio - 0.75).abs() < f64::EPSILON);
    }

    #[test]
    fn test_pathways_pruned_preserved() {
        let sched = DecayScheduler::new();
        sched.record_success(None, Some(7), None, None);
        let events = sched.recent_events(1);
        assert_eq!(events[0].pathways_pruned, Some(7));
    }
}
