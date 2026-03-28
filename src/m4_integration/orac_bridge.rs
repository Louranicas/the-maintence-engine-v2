//! # M53: ORAC Bridge
//!
//! Bridges the Maintenance Engine to the ORAC Sidecar (port 8133), enabling
//! health polling, blackboard queries, and hook event posting. Maintains a
//! sliding window of health snapshots for trend analysis and exposes
//! bridge-level health metrics.
//!
//! ## Layer: L4 (Integration)
//! ## Dependencies: M00 (Shared Types), ORAC Sidecar `/health`, `/blackboard`, `/hooks/PostToolUse`
//!
//! ## Polling Configuration
//!
//! - Default interval: 30 seconds
//! - Sliding window: 120 snapshots (60 minutes at 30s intervals)
//! - Fail-silent: continues operating if ORAC is unreachable
//!
//! ## Related Documentation
//! - [ORAC Sidecar Architecture](../../ai_docs/schematics/orac_sidecar.md)
//! - [Layer Specification](../../ai_docs/layers/L04_INTEGRATION.md)

#![allow(clippy::missing_errors_doc)]
#![allow(clippy::module_name_repetitions)]

use std::collections::{HashMap, VecDeque};
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};

use parking_lot::RwLock;

use crate::m1_foundation::shared_types::Timestamp;
use crate::{Error, Result};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Default ORAC Sidecar base URL.
const DEFAULT_ORAC_BASE_URL: &str = "http://localhost:8133";

/// Default health endpoint path.
const DEFAULT_HEALTH_PATH: &str = "/health";

/// Default blackboard endpoint path.
const DEFAULT_BLACKBOARD_PATH: &str = "/blackboard";

/// Default hooks endpoint path.
const DEFAULT_HOOKS_PATH: &str = "/hooks/PostToolUse";

/// Default polling interval in seconds.
const DEFAULT_POLL_INTERVAL_SECS: u64 = 30;

/// Default sliding window capacity (120 snapshots = 60 minutes at 30s).
const DEFAULT_WINDOW_CAPACITY: usize = 120;

/// Maximum consecutive failures before entering degraded mode.
const DEFAULT_MAX_CONSECUTIVE_FAILURES: u32 = 10;

/// Default hook log capacity.
const DEFAULT_HOOK_LOG_CAPACITY: usize = 200;

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Configuration for the ORAC bridge.
#[derive(Clone, Debug)]
pub struct OracBridgeConfig {
    /// Base URL of the ORAC Sidecar.
    pub orac_base_url: String,
    /// Health endpoint path.
    pub health_path: String,
    /// Blackboard endpoint path.
    pub blackboard_path: String,
    /// Hooks endpoint path.
    pub hooks_path: String,
    /// Polling interval in seconds.
    pub poll_interval_secs: u64,
    /// Sliding window capacity for health snapshots.
    pub window_capacity: usize,
    /// Maximum consecutive failures before degraded mode.
    pub max_consecutive_failures: u32,
    /// Maximum hook log entries to retain.
    pub hook_log_capacity: usize,
}

impl Default for OracBridgeConfig {
    fn default() -> Self {
        Self {
            orac_base_url: DEFAULT_ORAC_BASE_URL.to_string(),
            health_path: DEFAULT_HEALTH_PATH.to_string(),
            blackboard_path: DEFAULT_BLACKBOARD_PATH.to_string(),
            hooks_path: DEFAULT_HOOKS_PATH.to_string(),
            poll_interval_secs: DEFAULT_POLL_INTERVAL_SECS,
            window_capacity: DEFAULT_WINDOW_CAPACITY,
            max_consecutive_failures: DEFAULT_MAX_CONSECUTIVE_FAILURES,
            hook_log_capacity: DEFAULT_HOOK_LOG_CAPACITY,
        }
    }
}

impl OracBridgeConfig {
    /// Get the full health URL.
    #[must_use]
    pub fn health_url(&self) -> String {
        format!("{}{}", self.orac_base_url, self.health_path)
    }

    /// Get the full blackboard URL.
    #[must_use]
    pub fn blackboard_url(&self) -> String {
        format!("{}{}", self.orac_base_url, self.blackboard_path)
    }

    /// Get the full hooks URL.
    #[must_use]
    pub fn hooks_url(&self) -> String {
        format!("{}{}", self.orac_base_url, self.hooks_path)
    }
}

// ---------------------------------------------------------------------------
// OracHealthSnapshot
// ---------------------------------------------------------------------------

/// A point-in-time snapshot of ORAC Sidecar health.
///
/// Note: `Timestamp` is a monotonic cycle counter without Serde support,
/// so this type derives `Clone` + `Debug` only. Use the individual field
/// accessors for wire-format serialization.
#[derive(Clone, Debug)]
pub struct OracHealthSnapshot {
    /// RALPH fitness score (0.0 - 1.0).
    pub fitness: f64,
    /// Current RALPH generation.
    pub ralph_generation: u64,
    /// Total emergence events observed.
    pub emergence_events: u64,
    /// Mean coupling weight across all Hebbian pathways.
    pub coupling_weight_mean: f64,
    /// Kuramoto order parameter (field coherence).
    pub field_r: f64,
    /// Long-Term Potentiation event count.
    pub ltp_count: u64,
    /// Long-Term Depression event count.
    pub ltd_count: u64,
    /// Timestamp of this snapshot.
    pub timestamp: Timestamp,
    /// Raw HTTP status code from the health endpoint.
    pub raw_status_code: u16,
}

// ---------------------------------------------------------------------------
// OracBlackboardSnapshot
// ---------------------------------------------------------------------------

/// A point-in-time snapshot of the ORAC blackboard state.
#[derive(Clone, Debug)]
pub struct OracBlackboardSnapshot {
    /// Number of fleet tabs registered.
    pub fleet_tab_count: u32,
    /// Number of active panes.
    pub active_panes: u32,
    /// Number of consensus rounds pending.
    pub consensus_pending: u32,
    /// Total memory entries.
    pub memory_entries: u64,
    /// Raw JSON payload from the blackboard endpoint.
    pub raw_payload: String,
    /// Timestamp of this snapshot.
    pub timestamp: Timestamp,
}

// ---------------------------------------------------------------------------
// MeHookType
// ---------------------------------------------------------------------------

/// Types of hook events that ME can post to ORAC.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MeHookType {
    /// A tool was used by the maintenance engine.
    PostToolUse,
    /// A new session has started.
    SessionStart,
    /// A health check was performed.
    HealthCheck,
    /// An emergence event was detected.
    EmergenceDetected,
    /// PBFT consensus was reached.
    ConsensusReached,
    /// A learning cycle completed.
    LearningCycle,
    /// A thermal alert was triggered.
    ThermalAlert,
}

impl MeHookType {
    /// Return the string identifier for this hook type.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::PostToolUse => "PostToolUse",
            Self::SessionStart => "SessionStart",
            Self::HealthCheck => "HealthCheck",
            Self::EmergenceDetected => "EmergenceDetected",
            Self::ConsensusReached => "ConsensusReached",
            Self::LearningCycle => "LearningCycle",
            Self::ThermalAlert => "ThermalAlert",
        }
    }

    /// All known hook types.
    pub const ALL: [Self; 7] = [
        Self::PostToolUse,
        Self::SessionStart,
        Self::HealthCheck,
        Self::EmergenceDetected,
        Self::ConsensusReached,
        Self::LearningCycle,
        Self::ThermalAlert,
    ];
}

impl fmt::Display for MeHookType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// MeHookEvent
// ---------------------------------------------------------------------------

/// A hook event to post to the ORAC Sidecar.
#[derive(Clone, Debug)]
pub struct MeHookEvent {
    /// Type of hook event.
    pub hook_type: MeHookType,
    /// Name of the tool involved, if applicable.
    pub tool_name: Option<String>,
    /// Service that triggered the event.
    pub service_id: String,
    /// Timestamp of the event.
    pub timestamp: Timestamp,
    /// Session that generated the event.
    pub session_id: String,
}

// ---------------------------------------------------------------------------
// OracHookStats
// ---------------------------------------------------------------------------

/// Aggregate statistics for hook posting.
#[derive(Clone, Debug)]
pub struct OracHookStats {
    /// Total hooks posted (success + failure).
    pub total_posted: u64,
    /// Successful posts.
    pub successful_posts: u64,
    /// Failed posts.
    pub failed_posts: u64,
    /// Count of posts by hook type name.
    pub hooks_by_type: HashMap<String, u64>,
    /// Timestamp of the last successful post.
    pub last_posted_at: Option<Timestamp>,
}

// ---------------------------------------------------------------------------
// OracBridgeHealth
// ---------------------------------------------------------------------------

/// Computed health summary of the ORAC bridge.
#[derive(Clone, Debug)]
pub struct OracBridgeHealth {
    /// Most recent fitness score.
    pub current_fitness: f64,
    /// Average fitness across the sliding window.
    pub avg_fitness: f64,
    /// Maximum fitness observed in the window.
    pub max_fitness: f64,
    /// Minimum fitness observed in the window.
    pub min_fitness: f64,
    /// Most recent RALPH generation.
    pub current_ralph_gen: u64,
    /// Number of snapshots in the window.
    pub window_size: usize,
    /// Number of consecutive poll failures.
    pub consecutive_failures: u32,
    /// Whether the bridge is in degraded mode.
    pub degraded: bool,
    /// Hook success rate (0.0 - 1.0), NaN-safe.
    pub hook_success_rate: f64,
}

// ---------------------------------------------------------------------------
// OracBridge trait
// ---------------------------------------------------------------------------

/// Trait defining the ORAC Sidecar bridge interface.
///
/// All methods take `&self` with interior mutability via [`parking_lot::RwLock`]
/// and atomic counters, making the bridge safe to share across threads.
pub trait OracBridge: Send + Sync + fmt::Debug {
    /// Poll the ORAC health endpoint.
    ///
    /// # Errors
    ///
    /// Returns `Error::Other` when the HTTP client is not wired (stub mode).
    fn poll_health(&self) -> Result<OracHealthSnapshot>;

    /// Poll the ORAC blackboard endpoint.
    ///
    /// # Errors
    ///
    /// Returns `Error::Other` when the HTTP client is not wired (stub mode).
    fn poll_blackboard(&self) -> Result<OracBlackboardSnapshot>;

    /// Post a hook event to the ORAC Sidecar.
    ///
    /// # Errors
    ///
    /// Returns `Error::Other` when the HTTP client is not wired (stub mode).
    fn post_hook(&self, event: &MeHookEvent) -> Result<()>;

    /// Record a successful health poll snapshot into the sliding window.
    fn record_poll(&self, snapshot: OracHealthSnapshot);

    /// Record a poll failure (increments consecutive failure counter).
    fn record_failure(&self);

    /// Record the result of a hook post attempt.
    fn record_hook_post(&self, event_type: &str, success: bool);

    /// Get the most recent health snapshot, if any.
    fn latest_health(&self) -> Option<OracHealthSnapshot>;

    /// Get aggregate hook statistics.
    fn hook_stats(&self) -> OracHookStats;

    /// Compute and return the bridge health summary.
    fn bridge_health(&self) -> OracBridgeHealth;

    /// Check whether the bridge is in degraded mode.
    fn is_degraded(&self) -> bool;

    /// Reset all bridge state (snapshots, counters, hook log).
    fn reset(&self);
}

// ---------------------------------------------------------------------------
// OracBridgeManager
// ---------------------------------------------------------------------------

/// M53: ORAC Bridge manager.
///
/// Maintains a sliding window of ORAC health snapshots, tracks hook post
/// statistics, and computes bridge health metrics. HTTP calls are stubbed
/// until wired by [`crate::engine`] background tasks.
///
/// # Thread Safety
///
/// All mutable state is protected by [`parking_lot::RwLock`] or
/// [`AtomicU64`] counters.
pub struct OracBridgeManager {
    /// Sliding window of health snapshots.
    health_snapshots: RwLock<VecDeque<OracHealthSnapshot>>,
    /// Hook post log: `(event_type, success)` pairs.
    hook_log: RwLock<Vec<(String, bool)>>,
    /// Consecutive poll failure count.
    consecutive_failures: RwLock<u32>,
    /// Total hooks posted (atomic for concurrent access).
    total_posted: AtomicU64,
    /// Successful hook posts.
    successful_posted: AtomicU64,
    /// Failed hook posts.
    failed_posted: AtomicU64,
    /// Bridge configuration.
    config: OracBridgeConfig,
}

impl OracBridgeManager {
    /// Create a new ORAC bridge manager with default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self::with_config(OracBridgeConfig::default())
    }

    /// Create a new ORAC bridge manager with the given configuration.
    #[must_use]
    pub fn with_config(config: OracBridgeConfig) -> Self {
        Self {
            health_snapshots: RwLock::new(VecDeque::with_capacity(config.window_capacity)),
            hook_log: RwLock::new(Vec::with_capacity(config.hook_log_capacity)),
            consecutive_failures: RwLock::new(0),
            total_posted: AtomicU64::new(0),
            successful_posted: AtomicU64::new(0),
            failed_posted: AtomicU64::new(0),
            config,
        }
    }

    /// Get a reference to the configuration.
    #[must_use]
    pub const fn config(&self) -> &OracBridgeConfig {
        &self.config
    }

    /// Get the number of consecutive failures.
    #[must_use]
    pub fn consecutive_failures(&self) -> u32 {
        *self.consecutive_failures.read()
    }

    /// Get the current window size.
    #[must_use]
    pub fn window_size(&self) -> usize {
        self.health_snapshots.read().len()
    }

    /// Build a map of hook counts by event type from the log.
    fn hooks_by_type_map(&self) -> HashMap<String, u64> {
        let entries: Vec<_> = self.hook_log.read().iter().map(|(t, _)| t.clone()).collect();
        let mut map = HashMap::new();
        for event_type in &entries {
            *map.entry(event_type.clone()).or_insert(0) += 1;
        }
        map
    }

    /// Find the timestamp of the last successful hook post.
    fn last_successful_timestamp(&self) -> Option<Timestamp> {
        // We don't store timestamps in the hook log, so we derive from
        // health snapshots as a proxy. Returns the latest snapshot timestamp
        // if any hooks have been successfully posted.
        let successful = self.successful_posted.load(Ordering::Relaxed);
        if successful == 0 {
            return None;
        }
        self.health_snapshots.read().back().map(|s| s.timestamp)
    }
}

impl Default for OracBridgeManager {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for OracBridgeManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OracBridgeManager")
            .field("window_size", &self.health_snapshots.read().len())
            .field("hook_log_len", &self.hook_log.read().len())
            .field(
                "consecutive_failures",
                &*self.consecutive_failures.read(),
            )
            .field(
                "total_posted",
                &self.total_posted.load(Ordering::Relaxed),
            )
            .field(
                "successful_posted",
                &self.successful_posted.load(Ordering::Relaxed),
            )
            .field(
                "failed_posted",
                &self.failed_posted.load(Ordering::Relaxed),
            )
            .field("config", &self.config)
            .finish()
    }
}

impl OracBridge for OracBridgeManager {
    fn poll_health(&self) -> Result<OracHealthSnapshot> {
        Err(Error::Other(
            "stub: ORAC HTTP not wired".into(),
        ))
    }

    fn poll_blackboard(&self) -> Result<OracBlackboardSnapshot> {
        Err(Error::Other(
            "stub: ORAC HTTP not wired".into(),
        ))
    }

    fn post_hook(&self, _event: &MeHookEvent) -> Result<()> {
        Err(Error::Other(
            "stub: ORAC HTTP not wired".into(),
        ))
    }

    fn record_poll(&self, snapshot: OracHealthSnapshot) {
        let mut guard = self.health_snapshots.write();
        if guard.len() >= self.config.window_capacity {
            guard.pop_front();
        }
        guard.push_back(snapshot);
        drop(guard);

        // Reset consecutive failures on successful poll
        *self.consecutive_failures.write() = 0;
    }

    fn record_failure(&self) {
        *self.consecutive_failures.write() += 1;
    }

    fn record_hook_post(&self, event_type: &str, success: bool) {
        self.total_posted.fetch_add(1, Ordering::Relaxed);
        if success {
            self.successful_posted.fetch_add(1, Ordering::Relaxed);
        } else {
            self.failed_posted.fetch_add(1, Ordering::Relaxed);
        }

        let mut guard = self.hook_log.write();
        if guard.len() >= self.config.hook_log_capacity {
            // Remove oldest entry to stay within capacity
            guard.remove(0);
        }
        guard.push((event_type.to_string(), success));
    }

    fn latest_health(&self) -> Option<OracHealthSnapshot> {
        self.health_snapshots.read().back().cloned()
    }

    fn hook_stats(&self) -> OracHookStats {
        OracHookStats {
            total_posted: self.total_posted.load(Ordering::Relaxed),
            successful_posts: self.successful_posted.load(Ordering::Relaxed),
            failed_posts: self.failed_posted.load(Ordering::Relaxed),
            hooks_by_type: self.hooks_by_type_map(),
            last_posted_at: self.last_successful_timestamp(),
        }
    }

    #[allow(clippy::cast_precision_loss)]
    fn bridge_health(&self) -> OracBridgeHealth {
        let guard = self.health_snapshots.read();
        let failures = *self.consecutive_failures.read();

        if guard.is_empty() {
            let total = self.total_posted.load(Ordering::Relaxed);
            let success = self.successful_posted.load(Ordering::Relaxed);
            let hook_rate = if total == 0 {
                1.0
            } else {
                success as f64 / total as f64
            };

            return OracBridgeHealth {
                current_fitness: 0.0,
                avg_fitness: 0.0,
                max_fitness: 0.0,
                min_fitness: 0.0,
                current_ralph_gen: 0,
                window_size: 0,
                consecutive_failures: failures,
                degraded: failures >= self.config.max_consecutive_failures,
                hook_success_rate: hook_rate,
            };
        }

        let latest = guard
            .back()
            .cloned()
            .unwrap_or_else(|| OracHealthSnapshot {
                fitness: 0.0,
                ralph_generation: 0,
                emergence_events: 0,
                coupling_weight_mean: 0.0,
                field_r: 0.0,
                ltp_count: 0,
                ltd_count: 0,
                timestamp: Timestamp::now(),
                raw_status_code: 0,
            });

        let sum: f64 = guard.iter().map(|s| s.fitness).sum();
        let avg = sum / guard.len() as f64;

        let max = guard
            .iter()
            .map(|s| s.fitness)
            .fold(f64::NEG_INFINITY, f64::max);

        let min = guard
            .iter()
            .map(|s| s.fitness)
            .fold(f64::INFINITY, f64::min);

        let total = self.total_posted.load(Ordering::Relaxed);
        let success = self.successful_posted.load(Ordering::Relaxed);
        let hook_rate = if total == 0 {
            1.0
        } else {
            success as f64 / total as f64
        };

        OracBridgeHealth {
            current_fitness: latest.fitness,
            avg_fitness: avg,
            max_fitness: max,
            min_fitness: min,
            current_ralph_gen: latest.ralph_generation,
            window_size: guard.len(),
            consecutive_failures: failures,
            degraded: failures >= self.config.max_consecutive_failures,
            hook_success_rate: hook_rate,
        }
    }

    fn is_degraded(&self) -> bool {
        *self.consecutive_failures.read() >= self.config.max_consecutive_failures
    }

    fn reset(&self) {
        self.health_snapshots.write().clear();
        self.hook_log.write().clear();
        *self.consecutive_failures.write() = 0;
        self.total_posted.store(0, Ordering::Relaxed);
        self.successful_posted.store(0, Ordering::Relaxed);
        self.failed_posted.store(0, Ordering::Relaxed);
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    fn make_snapshot(fitness: f64, gen: u64) -> OracHealthSnapshot {
        OracHealthSnapshot {
            fitness,
            ralph_generation: gen,
            emergence_events: 0,
            coupling_weight_mean: 0.0,
            field_r: 0.0,
            ltp_count: 0,
            ltd_count: 0,
            timestamp: Timestamp::now(),
            raw_status_code: 200,
        }
    }

    fn make_full_snapshot(
        fitness: f64,
        gen: u64,
        emergence: u64,
        cwm: f64,
        field_r: f64,
        ltp: u64,
        ltd: u64,
    ) -> OracHealthSnapshot {
        OracHealthSnapshot {
            fitness,
            ralph_generation: gen,
            emergence_events: emergence,
            coupling_weight_mean: cwm,
            field_r,
            ltp_count: ltp,
            ltd_count: ltd,
            timestamp: Timestamp::now(),
            raw_status_code: 200,
        }
    }

    fn make_hook_event(hook_type: MeHookType) -> MeHookEvent {
        MeHookEvent {
            hook_type,
            tool_name: None,
            service_id: "maintenance-engine".to_string(),
            timestamp: Timestamp::now(),
            session_id: "test-session".to_string(),
        }
    }

    // -----------------------------------------------------------------------
    // Empty state tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_new_empty() {
        let bridge = OracBridgeManager::new();
        assert_eq!(bridge.window_size(), 0);
        assert_eq!(bridge.consecutive_failures(), 0);
        assert!(!bridge.is_degraded());
        assert!(bridge.latest_health().is_none());
    }

    #[test]
    fn test_default_config() {
        let config = OracBridgeConfig::default();
        assert_eq!(config.orac_base_url, "http://localhost:8133");
        assert_eq!(config.health_path, "/health");
        assert_eq!(config.blackboard_path, "/blackboard");
        assert_eq!(config.hooks_path, "/hooks/PostToolUse");
        assert_eq!(config.poll_interval_secs, 30);
        assert_eq!(config.window_capacity, 120);
        assert_eq!(config.max_consecutive_failures, 10);
        assert_eq!(config.hook_log_capacity, 200);
    }

    #[test]
    fn test_config_urls() {
        let config = OracBridgeConfig::default();
        assert_eq!(config.health_url(), "http://localhost:8133/health");
        assert_eq!(config.blackboard_url(), "http://localhost:8133/blackboard");
        assert_eq!(config.hooks_url(), "http://localhost:8133/hooks/PostToolUse");
    }

    #[test]
    fn test_config_custom_urls() {
        let config = OracBridgeConfig {
            orac_base_url: "http://orac:9999".to_string(),
            health_path: "/v2/health".to_string(),
            ..Default::default()
        };
        assert_eq!(config.health_url(), "http://orac:9999/v2/health");
    }

    #[test]
    fn test_empty_bridge_health() {
        let bridge = OracBridgeManager::new();
        let h = bridge.bridge_health();
        assert!((h.current_fitness).abs() < f64::EPSILON);
        assert!((h.avg_fitness).abs() < f64::EPSILON);
        assert!((h.max_fitness).abs() < f64::EPSILON);
        assert!((h.min_fitness).abs() < f64::EPSILON);
        assert_eq!(h.current_ralph_gen, 0);
        assert_eq!(h.window_size, 0);
        assert_eq!(h.consecutive_failures, 0);
        assert!(!h.degraded);
        assert!((h.hook_success_rate - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_empty_hook_stats() {
        let bridge = OracBridgeManager::new();
        let stats = bridge.hook_stats();
        assert_eq!(stats.total_posted, 0);
        assert_eq!(stats.successful_posts, 0);
        assert_eq!(stats.failed_posts, 0);
        assert!(stats.hooks_by_type.is_empty());
        assert!(stats.last_posted_at.is_none());
    }

    // -----------------------------------------------------------------------
    // Record poll tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_record_poll_single() {
        let bridge = OracBridgeManager::new();
        bridge.record_poll(make_snapshot(0.75, 100));
        assert_eq!(bridge.window_size(), 1);
        assert_eq!(bridge.consecutive_failures(), 0);
    }

    #[test]
    fn test_record_poll_multiple() {
        let bridge = OracBridgeManager::new();
        bridge.record_poll(make_snapshot(0.6, 10));
        bridge.record_poll(make_snapshot(0.7, 20));
        bridge.record_poll(make_snapshot(0.8, 30));
        assert_eq!(bridge.window_size(), 3);

        let latest = bridge.latest_health();
        assert!(latest.is_some());
        let snap = latest.unwrap_or_else(|| make_snapshot(0.0, 0));
        assert!((snap.fitness - 0.8).abs() < f64::EPSILON);
        assert_eq!(snap.ralph_generation, 30);
    }

    #[test]
    fn test_record_poll_builds_window() {
        let bridge = OracBridgeManager::new();
        for i in 0..10 {
            #[allow(clippy::cast_precision_loss)]
            bridge.record_poll(make_snapshot(i as f64 * 0.1, i));
        }
        assert_eq!(bridge.window_size(), 10);
    }

    // -----------------------------------------------------------------------
    // Window eviction tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_window_eviction() {
        let config = OracBridgeConfig {
            window_capacity: 5,
            ..Default::default()
        };
        let bridge = OracBridgeManager::with_config(config);
        for i in 0..10_u64 {
            #[allow(clippy::cast_precision_loss)]
            bridge.record_poll(make_snapshot(i as f64 * 0.1, i));
        }
        assert_eq!(bridge.window_size(), 5);

        // Latest should be the last recorded
        let latest = bridge.latest_health();
        assert!(latest.is_some());
        let snap = latest.unwrap_or_else(|| make_snapshot(0.0, 0));
        assert_eq!(snap.ralph_generation, 9);
    }

    #[test]
    fn test_window_eviction_preserves_latest() {
        let config = OracBridgeConfig {
            window_capacity: 3,
            ..Default::default()
        };
        let bridge = OracBridgeManager::with_config(config);
        bridge.record_poll(make_snapshot(0.1, 1));
        bridge.record_poll(make_snapshot(0.2, 2));
        bridge.record_poll(make_snapshot(0.3, 3));
        bridge.record_poll(make_snapshot(0.4, 4));
        assert_eq!(bridge.window_size(), 3);

        let latest = bridge.latest_health();
        let snap = latest.unwrap_or_else(|| make_snapshot(0.0, 0));
        assert!((snap.fitness - 0.4).abs() < f64::EPSILON);
    }

    #[test]
    fn test_window_capacity_one() {
        let config = OracBridgeConfig {
            window_capacity: 1,
            ..Default::default()
        };
        let bridge = OracBridgeManager::with_config(config);
        bridge.record_poll(make_snapshot(0.5, 1));
        bridge.record_poll(make_snapshot(0.9, 2));
        assert_eq!(bridge.window_size(), 1);
        let snap = bridge.latest_health().unwrap_or_else(|| make_snapshot(0.0, 0));
        assert!((snap.fitness - 0.9).abs() < f64::EPSILON);
    }

    // -----------------------------------------------------------------------
    // Failure tracking tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_record_failure() {
        let bridge = OracBridgeManager::new();
        for _ in 0..5 {
            bridge.record_failure();
        }
        assert_eq!(bridge.consecutive_failures(), 5);
        assert!(!bridge.is_degraded());
    }

    #[test]
    fn test_degraded_detection() {
        let bridge = OracBridgeManager::new();
        for _ in 0..10 {
            bridge.record_failure();
        }
        assert!(bridge.is_degraded());
    }

    #[test]
    fn test_degraded_at_exact_threshold() {
        let config = OracBridgeConfig {
            max_consecutive_failures: 3,
            ..Default::default()
        };
        let bridge = OracBridgeManager::with_config(config);
        bridge.record_failure();
        bridge.record_failure();
        assert!(!bridge.is_degraded());
        bridge.record_failure();
        assert!(bridge.is_degraded());
    }

    #[test]
    fn test_recovery_resets_failures() {
        let bridge = OracBridgeManager::new();
        bridge.record_failure();
        bridge.record_failure();
        bridge.record_failure();
        assert_eq!(bridge.consecutive_failures(), 3);

        bridge.record_poll(make_snapshot(0.8, 100));
        assert_eq!(bridge.consecutive_failures(), 0);
        assert!(!bridge.is_degraded());
    }

    #[test]
    fn test_degraded_recovery_via_poll() {
        let config = OracBridgeConfig {
            max_consecutive_failures: 2,
            ..Default::default()
        };
        let bridge = OracBridgeManager::with_config(config);
        bridge.record_failure();
        bridge.record_failure();
        assert!(bridge.is_degraded());

        bridge.record_poll(make_snapshot(0.5, 1));
        assert!(!bridge.is_degraded());
    }

    // -----------------------------------------------------------------------
    // Hook stats tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_record_hook_success() {
        let bridge = OracBridgeManager::new();
        bridge.record_hook_post("PostToolUse", true);
        let stats = bridge.hook_stats();
        assert_eq!(stats.total_posted, 1);
        assert_eq!(stats.successful_posts, 1);
        assert_eq!(stats.failed_posts, 0);
    }

    #[test]
    fn test_record_hook_failure() {
        let bridge = OracBridgeManager::new();
        bridge.record_hook_post("HealthCheck", false);
        let stats = bridge.hook_stats();
        assert_eq!(stats.total_posted, 1);
        assert_eq!(stats.successful_posts, 0);
        assert_eq!(stats.failed_posts, 1);
    }

    #[test]
    fn test_hook_stats_by_type() {
        let bridge = OracBridgeManager::new();
        bridge.record_hook_post("PostToolUse", true);
        bridge.record_hook_post("PostToolUse", true);
        bridge.record_hook_post("HealthCheck", false);
        bridge.record_hook_post("SessionStart", true);

        let stats = bridge.hook_stats();
        assert_eq!(stats.total_posted, 4);
        assert_eq!(stats.successful_posts, 3);
        assert_eq!(stats.failed_posts, 1);
        assert_eq!(stats.hooks_by_type.get("PostToolUse").copied().unwrap_or(0), 2);
        assert_eq!(stats.hooks_by_type.get("HealthCheck").copied().unwrap_or(0), 1);
        assert_eq!(stats.hooks_by_type.get("SessionStart").copied().unwrap_or(0), 1);
    }

    #[test]
    fn test_hook_log_capacity() {
        let config = OracBridgeConfig {
            hook_log_capacity: 3,
            ..Default::default()
        };
        let bridge = OracBridgeManager::with_config(config);
        bridge.record_hook_post("A", true);
        bridge.record_hook_post("B", true);
        bridge.record_hook_post("C", true);
        bridge.record_hook_post("D", true);
        bridge.record_hook_post("E", true);

        // Counters still track all
        let stats = bridge.hook_stats();
        assert_eq!(stats.total_posted, 5);

        // But log only has last 3 type entries
        let guard = bridge.hook_log.read();
        assert_eq!(guard.len(), 3);
    }

    #[test]
    fn test_hook_last_posted_at_none_when_no_success() {
        let bridge = OracBridgeManager::new();
        bridge.record_hook_post("PostToolUse", false);
        let stats = bridge.hook_stats();
        assert!(stats.last_posted_at.is_none());
    }

    #[test]
    fn test_hook_last_posted_at_present_after_success() {
        let bridge = OracBridgeManager::new();
        bridge.record_poll(make_snapshot(0.5, 1));
        bridge.record_hook_post("PostToolUse", true);
        let stats = bridge.hook_stats();
        // We have a snapshot and a successful hook, so last_posted_at should be Some
        assert!(stats.last_posted_at.is_some());
    }

    // -----------------------------------------------------------------------
    // Bridge health computation tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_bridge_health_single_snapshot() {
        let bridge = OracBridgeManager::new();
        bridge.record_poll(make_snapshot(0.75, 500));

        let h = bridge.bridge_health();
        assert!((h.current_fitness - 0.75).abs() < f64::EPSILON);
        assert!((h.avg_fitness - 0.75).abs() < f64::EPSILON);
        assert!((h.max_fitness - 0.75).abs() < f64::EPSILON);
        assert!((h.min_fitness - 0.75).abs() < f64::EPSILON);
        assert_eq!(h.current_ralph_gen, 500);
        assert_eq!(h.window_size, 1);
        assert!(!h.degraded);
    }

    #[test]
    fn test_bridge_health_multiple_snapshots() {
        let bridge = OracBridgeManager::new();
        bridge.record_poll(make_snapshot(0.6, 10));
        bridge.record_poll(make_snapshot(0.8, 20));
        bridge.record_poll(make_snapshot(0.7, 30));

        let h = bridge.bridge_health();
        assert!((h.current_fitness - 0.7).abs() < f64::EPSILON);
        assert!((h.avg_fitness - 0.7).abs() < f64::EPSILON);
        assert!((h.max_fitness - 0.8).abs() < f64::EPSILON);
        assert!((h.min_fitness - 0.6).abs() < f64::EPSILON);
        assert_eq!(h.current_ralph_gen, 30);
        assert_eq!(h.window_size, 3);
    }

    #[test]
    fn test_bridge_health_avg_computation() {
        let bridge = OracBridgeManager::new();
        bridge.record_poll(make_snapshot(0.2, 1));
        bridge.record_poll(make_snapshot(0.4, 2));
        bridge.record_poll(make_snapshot(0.6, 3));
        bridge.record_poll(make_snapshot(0.8, 4));

        let h = bridge.bridge_health();
        // avg = (0.2 + 0.4 + 0.6 + 0.8) / 4 = 0.5
        assert!((h.avg_fitness - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_bridge_health_hook_success_rate() {
        let bridge = OracBridgeManager::new();
        bridge.record_poll(make_snapshot(0.5, 1));
        bridge.record_hook_post("A", true);
        bridge.record_hook_post("B", true);
        bridge.record_hook_post("C", false);

        let h = bridge.bridge_health();
        // 2 success / 3 total = 0.666...
        let expected = 2.0 / 3.0;
        assert!((h.hook_success_rate - expected).abs() < 1e-10);
    }

    #[test]
    fn test_bridge_health_hook_rate_no_hooks() {
        let bridge = OracBridgeManager::new();
        bridge.record_poll(make_snapshot(0.5, 1));
        let h = bridge.bridge_health();
        assert!((h.hook_success_rate - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_bridge_health_degraded_flag() {
        let bridge = OracBridgeManager::new();
        bridge.record_poll(make_snapshot(0.5, 1));
        for _ in 0..10 {
            bridge.record_failure();
        }
        let h = bridge.bridge_health();
        assert!(h.degraded);
        assert_eq!(h.consecutive_failures, 10);
    }

    // -----------------------------------------------------------------------
    // MeHookType tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_hook_type_as_str() {
        assert_eq!(MeHookType::PostToolUse.as_str(), "PostToolUse");
        assert_eq!(MeHookType::SessionStart.as_str(), "SessionStart");
        assert_eq!(MeHookType::HealthCheck.as_str(), "HealthCheck");
        assert_eq!(MeHookType::EmergenceDetected.as_str(), "EmergenceDetected");
        assert_eq!(MeHookType::ConsensusReached.as_str(), "ConsensusReached");
        assert_eq!(MeHookType::LearningCycle.as_str(), "LearningCycle");
        assert_eq!(MeHookType::ThermalAlert.as_str(), "ThermalAlert");
    }

    #[test]
    fn test_hook_type_all_variants() {
        assert_eq!(MeHookType::ALL.len(), 7);
    }

    #[test]
    fn test_hook_type_display() {
        let s = format!("{}", MeHookType::PostToolUse);
        assert_eq!(s, "PostToolUse");
    }

    #[test]
    fn test_hook_type_equality() {
        assert_eq!(MeHookType::PostToolUse, MeHookType::PostToolUse);
        assert_ne!(MeHookType::PostToolUse, MeHookType::SessionStart);
    }

    #[test]
    fn test_hook_type_copy() {
        let a = MeHookType::ThermalAlert;
        let b = a;
        assert_eq!(a, b);
    }

    // -----------------------------------------------------------------------
    // Poll stubs return error
    // -----------------------------------------------------------------------

    #[test]
    fn test_poll_health_stub() {
        let bridge = OracBridgeManager::new();
        let result = bridge.poll_health();
        assert!(result.is_err());
    }

    #[test]
    fn test_poll_blackboard_stub() {
        let bridge = OracBridgeManager::new();
        let result = bridge.poll_blackboard();
        assert!(result.is_err());
    }

    #[test]
    fn test_post_hook_stub() {
        let bridge = OracBridgeManager::new();
        let event = make_hook_event(MeHookType::PostToolUse);
        let result = bridge.post_hook(&event);
        assert!(result.is_err());
    }

    // -----------------------------------------------------------------------
    // Reset tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_reset_clears_all() {
        let bridge = OracBridgeManager::new();
        bridge.record_poll(make_snapshot(0.8, 100));
        bridge.record_failure();
        bridge.record_hook_post("PostToolUse", true);

        bridge.reset();

        assert_eq!(bridge.window_size(), 0);
        assert_eq!(bridge.consecutive_failures(), 0);
        assert!(bridge.latest_health().is_none());
        assert!(!bridge.is_degraded());

        let stats = bridge.hook_stats();
        assert_eq!(stats.total_posted, 0);
        assert_eq!(stats.successful_posts, 0);
        assert_eq!(stats.failed_posts, 0);
        assert!(stats.hooks_by_type.is_empty());
    }

    #[test]
    fn test_reset_allows_fresh_recording() {
        let bridge = OracBridgeManager::new();
        bridge.record_poll(make_snapshot(0.5, 50));
        bridge.reset();
        bridge.record_poll(make_snapshot(0.9, 200));

        assert_eq!(bridge.window_size(), 1);
        let snap = bridge.latest_health().unwrap_or_else(|| make_snapshot(0.0, 0));
        assert!((snap.fitness - 0.9).abs() < f64::EPSILON);
    }

    // -----------------------------------------------------------------------
    // Config accessor tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_config_accessor() {
        let bridge = OracBridgeManager::new();
        assert_eq!(bridge.config().poll_interval_secs, DEFAULT_POLL_INTERVAL_SECS);
        assert_eq!(bridge.config().window_capacity, DEFAULT_WINDOW_CAPACITY);
    }

    #[test]
    fn test_config_custom() {
        let config = OracBridgeConfig {
            poll_interval_secs: 60,
            window_capacity: 240,
            max_consecutive_failures: 20,
            ..Default::default()
        };
        let bridge = OracBridgeManager::with_config(config);
        assert_eq!(bridge.config().poll_interval_secs, 60);
        assert_eq!(bridge.config().window_capacity, 240);
        assert_eq!(bridge.config().max_consecutive_failures, 20);
    }

    // -----------------------------------------------------------------------
    // Debug / Default trait tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_debug_format() {
        let bridge = OracBridgeManager::new();
        let debug = format!("{bridge:?}");
        assert!(debug.contains("OracBridgeManager"));
        assert!(debug.contains("window_size"));
    }

    #[test]
    fn test_default_trait() {
        let bridge = OracBridgeManager::default();
        assert_eq!(bridge.window_size(), 0);
        assert_eq!(bridge.consecutive_failures(), 0);
    }

    // -----------------------------------------------------------------------
    // Concurrent access tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_concurrent_record_poll() {
        use std::sync::Arc;
        use std::thread;

        let bridge = Arc::new(OracBridgeManager::new());
        let mut handles = Vec::new();

        for i in 0..10_u64 {
            let b = Arc::clone(&bridge);
            handles.push(thread::spawn(move || {
                #[allow(clippy::cast_precision_loss)]
                b.record_poll(make_snapshot(i as f64 * 0.1, i));
            }));
        }

        for h in handles {
            h.join().unwrap_or(());
        }

        assert_eq!(bridge.window_size(), 10);
    }

    #[test]
    fn test_concurrent_record_hook() {
        use std::sync::Arc;
        use std::thread;

        let bridge = Arc::new(OracBridgeManager::new());
        let mut handles = Vec::new();

        for _ in 0..20 {
            let b = Arc::clone(&bridge);
            handles.push(thread::spawn(move || {
                b.record_hook_post("PostToolUse", true);
            }));
        }

        for h in handles {
            h.join().unwrap_or(());
        }

        let stats = bridge.hook_stats();
        assert_eq!(stats.total_posted, 20);
        assert_eq!(stats.successful_posts, 20);
    }

    #[test]
    fn test_concurrent_mixed_operations() {
        use std::sync::Arc;
        use std::thread;

        let bridge = Arc::new(OracBridgeManager::new());
        let mut handles = Vec::new();

        // Poll threads
        for i in 0..5_u64 {
            let b = Arc::clone(&bridge);
            handles.push(thread::spawn(move || {
                #[allow(clippy::cast_precision_loss)]
                b.record_poll(make_snapshot(i as f64 * 0.1, i));
            }));
        }

        // Failure threads
        for _ in 0..3 {
            let b = Arc::clone(&bridge);
            handles.push(thread::spawn(move || {
                b.record_failure();
            }));
        }

        // Hook threads
        for _ in 0..4 {
            let b = Arc::clone(&bridge);
            handles.push(thread::spawn(move || {
                b.record_hook_post("HealthCheck", true);
            }));
        }

        for h in handles {
            h.join().unwrap_or(());
        }

        // All polls should be recorded
        assert_eq!(bridge.window_size(), 5);
        let stats = bridge.hook_stats();
        assert_eq!(stats.total_posted, 4);
    }

    // -----------------------------------------------------------------------
    // Full snapshot field tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_full_snapshot_fields() {
        let snap = make_full_snapshot(0.78, 5000, 3000, 0.45, 0.92, 1200, 800);
        assert!((snap.fitness - 0.78).abs() < f64::EPSILON);
        assert_eq!(snap.ralph_generation, 5000);
        assert_eq!(snap.emergence_events, 3000);
        assert!((snap.coupling_weight_mean - 0.45).abs() < f64::EPSILON);
        assert!((snap.field_r - 0.92).abs() < f64::EPSILON);
        assert_eq!(snap.ltp_count, 1200);
        assert_eq!(snap.ltd_count, 800);
        assert_eq!(snap.raw_status_code, 200);
    }

    #[test]
    fn test_snapshot_clone() {
        let snap = make_snapshot(0.65, 42);
        let cloned = snap.clone();
        assert!((cloned.fitness - 0.65).abs() < f64::EPSILON);
        assert_eq!(cloned.ralph_generation, 42);
    }

    #[test]
    fn test_snapshot_debug() {
        let snap = make_snapshot(0.5, 1);
        let debug = format!("{snap:?}");
        assert!(debug.contains("OracHealthSnapshot"));
    }

    // -----------------------------------------------------------------------
    // MeHookEvent tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_hook_event_with_tool() {
        let event = MeHookEvent {
            hook_type: MeHookType::PostToolUse,
            tool_name: Some("health_check".to_string()),
            service_id: "me-v2".to_string(),
            timestamp: Timestamp::now(),
            session_id: "sess-001".to_string(),
        };
        assert_eq!(event.hook_type, MeHookType::PostToolUse);
        assert!(event.tool_name.is_some());
    }

    #[test]
    fn test_hook_event_without_tool() {
        let event = make_hook_event(MeHookType::SessionStart);
        assert!(event.tool_name.is_none());
        assert_eq!(event.service_id, "maintenance-engine");
    }

    #[test]
    fn test_hook_event_clone() {
        let event = make_hook_event(MeHookType::LearningCycle);
        let cloned = event.clone();
        assert_eq!(cloned.hook_type, MeHookType::LearningCycle);
    }

    // -----------------------------------------------------------------------
    // OracBlackboardSnapshot tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_blackboard_snapshot() {
        let snap = OracBlackboardSnapshot {
            fleet_tab_count: 3,
            active_panes: 9,
            consensus_pending: 1,
            memory_entries: 2437,
            raw_payload: "{}".to_string(),
            timestamp: Timestamp::now(),
        };
        assert_eq!(snap.fleet_tab_count, 3);
        assert_eq!(snap.active_panes, 9);
        assert_eq!(snap.consensus_pending, 1);
        assert_eq!(snap.memory_entries, 2437);
    }

    #[test]
    fn test_blackboard_snapshot_clone() {
        let snap = OracBlackboardSnapshot {
            fleet_tab_count: 2,
            active_panes: 6,
            consensus_pending: 0,
            memory_entries: 100,
            raw_payload: r#"{"status":"ok"}"#.to_string(),
            timestamp: Timestamp::now(),
        };
        let cloned = snap.clone();
        assert_eq!(cloned.fleet_tab_count, 2);
        assert_eq!(cloned.raw_payload, r#"{"status":"ok"}"#);
    }

    // -----------------------------------------------------------------------
    // OracBridgeHealth tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_bridge_health_clone() {
        let h = OracBridgeHealth {
            current_fitness: 0.8,
            avg_fitness: 0.75,
            max_fitness: 0.9,
            min_fitness: 0.6,
            current_ralph_gen: 5000,
            window_size: 10,
            consecutive_failures: 0,
            degraded: false,
            hook_success_rate: 0.95,
        };
        let cloned = h.clone();
        assert!((cloned.current_fitness - 0.8).abs() < f64::EPSILON);
        assert_eq!(cloned.window_size, 10);
    }

    // -----------------------------------------------------------------------
    // OracHookStats tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_hook_stats_clone() {
        let stats = OracHookStats {
            total_posted: 10,
            successful_posts: 8,
            failed_posts: 2,
            hooks_by_type: HashMap::new(),
            last_posted_at: Some(Timestamp::now()),
        };
        let cloned = stats.clone();
        assert_eq!(cloned.total_posted, 10);
        assert!(cloned.last_posted_at.is_some());
    }

    // -----------------------------------------------------------------------
    // Edge case tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_fitness_zero() {
        let bridge = OracBridgeManager::new();
        bridge.record_poll(make_snapshot(0.0, 0));
        let h = bridge.bridge_health();
        assert!((h.current_fitness).abs() < f64::EPSILON);
        assert!((h.min_fitness).abs() < f64::EPSILON);
    }

    #[test]
    fn test_fitness_one() {
        let bridge = OracBridgeManager::new();
        bridge.record_poll(make_snapshot(1.0, 1));
        let h = bridge.bridge_health();
        assert!((h.current_fitness - 1.0).abs() < f64::EPSILON);
        assert!((h.max_fitness - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_many_failures_then_recovery() {
        let config = OracBridgeConfig {
            max_consecutive_failures: 5,
            ..Default::default()
        };
        let bridge = OracBridgeManager::with_config(config);

        // Drive into degraded
        for _ in 0..10 {
            bridge.record_failure();
        }
        assert!(bridge.is_degraded());
        assert_eq!(bridge.consecutive_failures(), 10);

        // Single poll recovers
        bridge.record_poll(make_snapshot(0.5, 1));
        assert!(!bridge.is_degraded());
        assert_eq!(bridge.consecutive_failures(), 0);
    }

    #[test]
    fn test_bridge_health_with_only_failures() {
        let bridge = OracBridgeManager::new();
        for _ in 0..15 {
            bridge.record_failure();
        }
        let h = bridge.bridge_health();
        assert_eq!(h.window_size, 0);
        assert!(h.degraded);
        assert_eq!(h.consecutive_failures, 15);
    }

    #[test]
    fn test_all_hooks_fail() {
        let bridge = OracBridgeManager::new();
        bridge.record_poll(make_snapshot(0.5, 1));
        for _ in 0..10 {
            bridge.record_hook_post("HealthCheck", false);
        }
        let h = bridge.bridge_health();
        assert!((h.hook_success_rate).abs() < f64::EPSILON);
    }

    #[test]
    fn test_all_hooks_succeed() {
        let bridge = OracBridgeManager::new();
        bridge.record_poll(make_snapshot(0.5, 1));
        for _ in 0..10 {
            bridge.record_hook_post("PostToolUse", true);
        }
        let h = bridge.bridge_health();
        assert!((h.hook_success_rate - 1.0).abs() < f64::EPSILON);
    }
}
