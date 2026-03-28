//! # M49: Traffic Manager
//!
//! Traffic shaping and rate-limit monitoring for ULTRAPLATE services with
//! trait-based abstraction, interior mutability, and 12D tensor contribution.
//!
//! ## Layer: L2 (Services)
//! ## Module: M49
//! ## Dependencies: L1 (Error, Timestamp, `ModuleId`, `TensorContributor`)
//!
//! ## Trait: [`TrafficShaping`]
//!
//! All methods are `&self` (C2) with interior mutability via `RwLock`.
//! Methods returning data through `RwLock` return OWNED types (C7).
//!
//! ## 12D Tensor Contribution (C3)
//!
//! | Dimension | Value |
//! |-----------|-------|
//! | D9 (`latency`) | 1.0 - `mean_latency` / `max_latency` (clamped) |
//! | D10 (`error_rate`) | mean rejection rate across all windows |
//!
//! ## Ring Buffer
//!
//! Each [`TrafficWindow`] maintains a fixed-size ring buffer of
//! [`RequestObservation`]s. When the buffer reaches `window_size`, the
//! oldest entry is evicted before a new one is inserted — matching the
//! existing L5 pruner pattern.

use std::collections::HashMap;
use std::fmt;

use parking_lot::RwLock;

use crate::m1_foundation::shared_types::{CoverageBitmap, DimensionIndex, Timestamp};
use crate::m1_foundation::tensor_registry::{ContributedTensor, ContributorKind, TensorContributor};
use crate::{Error, Result, Tensor12D};

// ============================================================================
// Constants
// ============================================================================

/// Default maximum requests per second.
const DEFAULT_MAX_RPS: f64 = 1000.0;

/// Default maximum concurrent requests.
const DEFAULT_MAX_CONCURRENCY: usize = 100;

/// Default maximum queue depth.
const DEFAULT_MAX_QUEUE_DEPTH: usize = 500;

/// Default maximum acceptable latency in milliseconds.
const DEFAULT_MAX_LATENCY_MS: f64 = 2000.0;

/// Default rejection-rate threshold above which a service is "saturated".
const DEFAULT_REJECTION_THRESHOLD: f64 = 0.1;

/// Default sliding-window size (number of observations).
const DEFAULT_WINDOW_SIZE: usize = 100;

/// Weight of rejection rate in the traffic-health formula.
const REJECTION_WEIGHT: f64 = 0.4;

/// Weight of error rate in the traffic-health formula.
const ERROR_WEIGHT: f64 = 0.3;

/// Weight of latency ratio in the traffic-health formula.
const LATENCY_WEIGHT: f64 = 0.3;

// ============================================================================
// TrafficShaping (trait)
// ============================================================================

/// Trait for traffic shaping and rate-limit monitoring.
///
/// All methods are `&self` (C2). State mutation uses interior mutability.
/// Methods returning data through `RwLock` return owned types (C7).
pub trait TrafficShaping: Send + Sync + fmt::Debug {
    /// Register a service for traffic monitoring.
    ///
    /// # Errors
    /// Returns [`Error::Validation`] if `service_id` is empty or already registered.
    fn register_service(&self, service_id: &str, config: TrafficConfig) -> Result<()>;

    /// Remove a service from traffic monitoring.
    ///
    /// # Errors
    /// Returns [`Error::ServiceNotFound`] if no window exists for `service_id`.
    fn deregister_service(&self, service_id: &str) -> Result<()>;

    /// Record an inbound request observation (latency + success/failure).
    ///
    /// # Errors
    /// Returns [`Error::Validation`] if `service_id` is empty.
    /// Returns [`Error::ServiceNotFound`] if `service_id` is not registered.
    fn record_request(&self, service_id: &str, obs: RequestObservation) -> Result<()>;

    /// Record a rejected request (rate-limited / shed).
    ///
    /// # Errors
    /// Returns [`Error::Validation`] if `service_id` is empty.
    /// Returns [`Error::ServiceNotFound`] if `service_id` is not registered.
    fn record_rejection(&self, service_id: &str) -> Result<()>;

    /// Return a point-in-time snapshot for one service.
    ///
    /// # Errors
    /// Returns [`Error::Validation`] if `service_id` is empty.
    /// Returns [`Error::ServiceNotFound`] if `service_id` is not registered.
    fn get_snapshot(&self, service_id: &str) -> Result<TrafficSnapshot>;

    /// Return snapshots for every registered service.
    fn get_all_snapshots(&self) -> Vec<TrafficSnapshot>;

    /// Number of registered services.
    fn service_count(&self) -> usize;

    /// Weighted traffic-health score across all services (0.0–1.0).
    fn aggregate_traffic_health(&self) -> f64;

    /// Return IDs of services whose rejection rate exceeds their threshold.
    fn get_saturated_services(&self) -> Vec<String>;

    /// Mean latency across all windows (milliseconds).
    fn mean_latency_ms(&self) -> f64;

    /// Mean rejection rate across all windows (0.0–1.0).
    fn mean_rejection_rate(&self) -> f64;

    /// Reset all windows and counters.
    fn reset_all(&self);
}

// ============================================================================
// TrafficConfig
// ============================================================================

/// Configuration for a traffic-monitoring window.
///
/// Defines rate limits, concurrency caps, and health thresholds
/// for a single service.
#[derive(Clone, Debug, PartialEq)]
pub struct TrafficConfig {
    /// Maximum allowed requests per second.
    pub max_rps: f64,
    /// Maximum concurrent in-flight requests.
    pub max_concurrency: usize,
    /// Maximum queue depth before shedding.
    pub max_queue_depth: usize,
    /// Ceiling latency (ms) used as denominator in health formula.
    pub max_latency_ms: f64,
    /// Rejection-rate threshold above which a service is "saturated".
    pub rejection_threshold: f64,
    /// Sliding-window size (number of observations retained).
    pub window_size: usize,
}

impl Default for TrafficConfig {
    fn default() -> Self {
        Self {
            max_rps: DEFAULT_MAX_RPS,
            max_concurrency: DEFAULT_MAX_CONCURRENCY,
            max_queue_depth: DEFAULT_MAX_QUEUE_DEPTH,
            max_latency_ms: DEFAULT_MAX_LATENCY_MS,
            rejection_threshold: DEFAULT_REJECTION_THRESHOLD,
            window_size: DEFAULT_WINDOW_SIZE,
        }
    }
}

impl TrafficConfig {
    /// Create a builder for `TrafficConfig`.
    #[must_use]
    pub const fn builder() -> TrafficConfigBuilder {
        TrafficConfigBuilder::new()
    }
}

// ============================================================================
// TrafficConfigBuilder
// ============================================================================

/// Builder for [`TrafficConfig`].
#[derive(Clone, Debug)]
pub struct TrafficConfigBuilder {
    max_rps: f64,
    max_concurrency: usize,
    max_queue_depth: usize,
    max_latency_ms: f64,
    rejection_threshold: f64,
    window_size: usize,
}

impl TrafficConfigBuilder {
    /// Create a new builder with default values.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            max_rps: DEFAULT_MAX_RPS,
            max_concurrency: DEFAULT_MAX_CONCURRENCY,
            max_queue_depth: DEFAULT_MAX_QUEUE_DEPTH,
            max_latency_ms: DEFAULT_MAX_LATENCY_MS,
            rejection_threshold: DEFAULT_REJECTION_THRESHOLD,
            window_size: DEFAULT_WINDOW_SIZE,
        }
    }

    /// Set maximum requests per second.
    #[must_use]
    pub const fn max_rps(mut self, max_rps: f64) -> Self {
        self.max_rps = max_rps;
        self
    }

    /// Set maximum concurrency.
    #[must_use]
    pub const fn max_concurrency(mut self, max_concurrency: usize) -> Self {
        self.max_concurrency = max_concurrency;
        self
    }

    /// Set maximum queue depth.
    #[must_use]
    pub const fn max_queue_depth(mut self, max_queue_depth: usize) -> Self {
        self.max_queue_depth = max_queue_depth;
        self
    }

    /// Set maximum acceptable latency in milliseconds.
    #[must_use]
    pub const fn max_latency_ms(mut self, max_latency_ms: f64) -> Self {
        self.max_latency_ms = max_latency_ms;
        self
    }

    /// Set rejection-rate saturation threshold.
    #[must_use]
    pub const fn rejection_threshold(mut self, rejection_threshold: f64) -> Self {
        self.rejection_threshold = rejection_threshold;
        self
    }

    /// Set observation window size.
    #[must_use]
    pub const fn window_size(mut self, window_size: usize) -> Self {
        self.window_size = window_size;
        self
    }

    /// Build the [`TrafficConfig`].
    #[must_use]
    pub const fn build(self) -> TrafficConfig {
        TrafficConfig {
            max_rps: self.max_rps,
            max_concurrency: self.max_concurrency,
            max_queue_depth: self.max_queue_depth,
            max_latency_ms: self.max_latency_ms,
            rejection_threshold: self.rejection_threshold,
            window_size: self.window_size,
        }
    }
}

impl Default for TrafficConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// RequestObservation
// ============================================================================

/// A single request observation recording latency and outcome.
#[derive(Clone, Debug, PartialEq)]
pub struct RequestObservation {
    /// Measured latency in milliseconds.
    pub latency_ms: f64,
    /// Whether the request completed successfully.
    pub success: bool,
    /// Monotonic timestamp of the observation (C5: Timestamp, not `SystemTime`).
    pub timestamp: Timestamp,
}

impl RequestObservation {
    /// Create a new observation with the current timestamp.
    #[must_use]
    pub fn new(latency_ms: f64, success: bool) -> Self {
        Self {
            latency_ms,
            success,
            timestamp: Timestamp::now(),
        }
    }

    /// Create an observation with an explicit timestamp (for testing).
    #[must_use]
    pub const fn with_timestamp(latency_ms: f64, success: bool, timestamp: Timestamp) -> Self {
        Self {
            latency_ms,
            success,
            timestamp,
        }
    }
}

// ============================================================================
// TrafficSnapshot
// ============================================================================

/// Point-in-time snapshot of traffic statistics for one service.
#[derive(Clone, Debug, PartialEq)]
pub struct TrafficSnapshot {
    /// Service identifier.
    pub service_id: String,
    /// Current requests per second estimate.
    pub rps: f64,
    /// 50th-percentile latency (ms).
    pub p50_latency_ms: f64,
    /// 95th-percentile latency (ms).
    pub p95_latency_ms: f64,
    /// 99th-percentile latency (ms).
    pub p99_latency_ms: f64,
    /// Rejection rate (0.0–1.0).
    pub rejection_rate: f64,
    /// Error rate (fraction of failed observations, 0.0–1.0).
    pub error_rate: f64,
    /// Composite traffic health score (0.0–1.0).
    pub traffic_health: f64,
    /// Number of observations currently in the window.
    pub window_request_count: usize,
    /// Snapshot timestamp.
    pub timestamp: Timestamp,
}

// ============================================================================
// TrafficWindow (internal)
// ============================================================================

/// Per-service sliding window of observations plus rejection counter.
#[derive(Clone, Debug)]
struct TrafficWindow {
    /// Service identifier.
    service_id: String,
    /// Configuration governing this window.
    config: TrafficConfig,
    /// Ring buffer of recent observations (capped at `config.window_size`).
    observations: Vec<RequestObservation>,
    /// Cumulative rejection count.
    rejection_count: u64,
    /// Total request count (observations + rejections) — used for rejection rate.
    total_request_count: u64,
    /// Last time an observation or rejection was recorded.
    last_updated: Timestamp,
}

impl TrafficWindow {
    /// Create a new empty window.
    fn new(service_id: String, config: TrafficConfig) -> Self {
        Self {
            service_id,
            config,
            observations: Vec::new(),
            rejection_count: 0,
            total_request_count: 0,
            last_updated: Timestamp::now(),
        }
    }

    /// Push an observation, evicting the oldest if at capacity.
    fn push_observation(&mut self, obs: RequestObservation) {
        if self.observations.len() == self.config.window_size {
            self.observations.remove(0);
        }
        self.observations.push(obs);
        self.total_request_count = self.total_request_count.saturating_add(1);
        self.last_updated = Timestamp::now();
    }

    /// Record a rejection.
    fn record_rejection(&mut self) {
        self.rejection_count = self.rejection_count.saturating_add(1);
        self.total_request_count = self.total_request_count.saturating_add(1);
        self.last_updated = Timestamp::now();
    }

    /// Compute the rejection rate (0.0–1.0).
    fn rejection_rate(&self) -> f64 {
        if self.total_request_count == 0 {
            return 0.0;
        }
        #[allow(clippy::cast_precision_loss)]
        let rate = self.rejection_count as f64 / self.total_request_count as f64;
        rate
    }

    /// Compute the error rate from observations (fraction that failed).
    fn error_rate(&self) -> f64 {
        if self.observations.is_empty() {
            return 0.0;
        }
        #[allow(clippy::cast_precision_loss)]
        let failures = self.observations.iter().filter(|o| !o.success).count() as f64;
        #[allow(clippy::cast_precision_loss)]
        let total = self.observations.len() as f64;
        failures / total
    }

    /// Mean latency across current observations (ms).
    fn mean_latency_ms(&self) -> f64 {
        if self.observations.is_empty() {
            return 0.0;
        }
        let sum: f64 = self.observations.iter().map(|o| o.latency_ms).sum();
        #[allow(clippy::cast_precision_loss)]
        let mean = sum / self.observations.len() as f64;
        mean
    }

    /// Compute the p-th percentile latency (0.0–1.0 scale for `p`).
    fn percentile_latency(&self, p: f64) -> f64 {
        if self.observations.is_empty() {
            return 0.0;
        }
        let mut latencies: Vec<f64> = self.observations.iter().map(|o| o.latency_ms).collect();
        latencies.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let idx = ((p * latencies.len() as f64).floor() as usize).min(latencies.len() - 1);
        latencies[idx]
    }

    /// Compute RPS estimate from the observation window.
    ///
    /// Uses the tick delta between first and last observation as a proxy
    /// for time span (each tick is one monotonic increment).
    fn rps_estimate(&self) -> f64 {
        if self.observations.len() < 2 {
            return 0.0;
        }
        let first_tick = self.observations.first().map_or(0, |o| o.timestamp.ticks());
        let last_tick = self.observations.last().map_or(0, |o| o.timestamp.ticks());
        let span = last_tick.saturating_sub(first_tick);
        if span == 0 {
            return 0.0;
        }
        #[allow(clippy::cast_precision_loss)]
        let rps = self.observations.len() as f64 / span as f64;
        rps
    }

    /// Compute the composite traffic-health score (0.0–1.0).
    ///
    /// Formula: `1.0 - (rejection_rate * 0.4 + error_rate * 0.3 + latency_ratio * 0.3)`
    /// clamped to `[0, 1]`, using `mul_add` for FMA precision.
    fn traffic_health(&self) -> f64 {
        let rejection = self.rejection_rate();
        let error = self.error_rate();
        let latency_ratio = if self.config.max_latency_ms > 0.0 {
            (self.mean_latency_ms() / self.config.max_latency_ms).min(1.0)
        } else {
            0.0
        };

        // 1.0 - (REJECTION_WEIGHT * rejection + ERROR_WEIGHT * error + LATENCY_WEIGHT * latency_ratio)
        let penalty = REJECTION_WEIGHT.mul_add(
            rejection,
            ERROR_WEIGHT.mul_add(error, LATENCY_WEIGHT * latency_ratio),
        );
        (1.0 - penalty).clamp(0.0, 1.0)
    }

    /// Build a snapshot from the current window state.
    fn snapshot(&self) -> TrafficSnapshot {
        TrafficSnapshot {
            service_id: self.service_id.clone(),
            rps: self.rps_estimate(),
            p50_latency_ms: self.percentile_latency(0.50),
            p95_latency_ms: self.percentile_latency(0.95),
            p99_latency_ms: self.percentile_latency(0.99),
            rejection_rate: self.rejection_rate(),
            error_rate: self.error_rate(),
            traffic_health: self.traffic_health(),
            window_request_count: self.observations.len(),
            timestamp: Timestamp::now(),
        }
    }

    /// Whether the service is saturated (rejection rate above threshold).
    fn is_saturated(&self) -> bool {
        self.rejection_rate() > self.config.rejection_threshold
    }
}

// ============================================================================
// TrafficManager
// ============================================================================

/// Traffic manager providing rate-limit monitoring for all registered services.
///
/// Uses `parking_lot::RwLock` for interior mutability (C2).
/// All public methods accept `&self` and return owned types (C7).
#[derive(Debug)]
pub struct TrafficManager {
    /// Per-service traffic windows.
    windows: RwLock<HashMap<String, TrafficWindow>>,
}

impl TrafficManager {
    /// Create a new empty `TrafficManager`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            windows: RwLock::new(HashMap::new()),
        }
    }
}

impl Default for TrafficManager {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// TrafficShaping implementation
// ============================================================================

impl TrafficShaping for TrafficManager {
    fn register_service(&self, service_id: &str, config: TrafficConfig) -> Result<()> {
        if service_id.is_empty() {
            return Err(Error::Validation("service_id must not be empty".into()));
        }
        let mut windows = self.windows.write();
        if windows.contains_key(service_id) {
            return Err(Error::Validation(format!(
                "service '{service_id}' is already registered"
            )));
        }
        windows.insert(
            service_id.to_string(),
            TrafficWindow::new(service_id.to_string(), config),
        );
        drop(windows);
        Ok(())
    }

    fn deregister_service(&self, service_id: &str) -> Result<()> {
        let removed = self.windows.write().remove(service_id);
        if removed.is_none() {
            return Err(Error::ServiceNotFound(service_id.to_string()));
        }
        Ok(())
    }

    fn record_request(&self, service_id: &str, obs: RequestObservation) -> Result<()> {
        if service_id.is_empty() {
            return Err(Error::Validation("service_id must not be empty".into()));
        }
        self.windows
            .write()
            .get_mut(service_id)
            .ok_or_else(|| Error::ServiceNotFound(service_id.to_string()))?
            .push_observation(obs);
        Ok(())
    }

    fn record_rejection(&self, service_id: &str) -> Result<()> {
        if service_id.is_empty() {
            return Err(Error::Validation("service_id must not be empty".into()));
        }
        self.windows
            .write()
            .get_mut(service_id)
            .ok_or_else(|| Error::ServiceNotFound(service_id.to_string()))?
            .record_rejection();
        Ok(())
    }

    fn get_snapshot(&self, service_id: &str) -> Result<TrafficSnapshot> {
        if service_id.is_empty() {
            return Err(Error::Validation("service_id must not be empty".into()));
        }
        self.windows
            .read()
            .get(service_id)
            .map(TrafficWindow::snapshot)
            .ok_or_else(|| Error::ServiceNotFound(service_id.to_string()))
    }

    fn get_all_snapshots(&self) -> Vec<TrafficSnapshot> {
        let windows = self.windows.read();
        windows.values().map(TrafficWindow::snapshot).collect()
    }

    fn service_count(&self) -> usize {
        self.windows.read().len()
    }

    fn aggregate_traffic_health(&self) -> f64 {
        let windows = self.windows.read();
        if windows.is_empty() {
            return 1.0;
        }
        let sum: f64 = windows.values().map(TrafficWindow::traffic_health).sum();
        #[allow(clippy::cast_precision_loss)]
        let avg = sum / windows.len() as f64;
        avg
    }

    fn get_saturated_services(&self) -> Vec<String> {
        let windows = self.windows.read();
        windows
            .values()
            .filter(|w| w.is_saturated())
            .map(|w| w.service_id.clone())
            .collect()
    }

    fn mean_latency_ms(&self) -> f64 {
        let windows = self.windows.read();
        if windows.is_empty() {
            return 0.0;
        }
        let sum: f64 = windows.values().map(TrafficWindow::mean_latency_ms).sum();
        #[allow(clippy::cast_precision_loss)]
        let avg = sum / windows.len() as f64;
        avg
    }

    fn mean_rejection_rate(&self) -> f64 {
        let windows = self.windows.read();
        if windows.is_empty() {
            return 0.0;
        }
        let sum: f64 = windows.values().map(TrafficWindow::rejection_rate).sum();
        #[allow(clippy::cast_precision_loss)]
        let avg = sum / windows.len() as f64;
        avg
    }

    fn reset_all(&self) {
        self.windows.write().clear();
    }
}

// ============================================================================
// TensorContributor — TrafficManager: D9 (latency proxy), D10 (rejection rate)
// ============================================================================

impl TensorContributor for TrafficManager {
    fn contribute(&self) -> ContributedTensor {
        let windows = self.windows.read();

        let d9 = if windows.is_empty() {
            1.0
        } else {
            let sum_latency: f64 = windows.values().map(TrafficWindow::mean_latency_ms).sum();
            let sum_max: f64 = windows.values().map(|w| w.config.max_latency_ms).sum();
            if sum_max > 0.0 {
                (1.0 - (sum_latency / sum_max)).clamp(0.0, 1.0)
            } else {
                1.0
            }
        };

        let d10 = if windows.is_empty() {
            0.0
        } else {
            let sum: f64 = windows.values().map(TrafficWindow::rejection_rate).sum();
            #[allow(clippy::cast_precision_loss)]
            let avg = sum / windows.len() as f64;
            avg.clamp(0.0, 1.0)
        };

        drop(windows);

        let mut tensor = Tensor12D::default();
        let mut values = tensor.to_array();
        values[DimensionIndex::Latency as usize] = d9;
        values[DimensionIndex::ErrorRate as usize] = d10;
        tensor = Tensor12D::new(values);

        let coverage = CoverageBitmap::EMPTY
            .with_dimension(DimensionIndex::Latency)
            .with_dimension(DimensionIndex::ErrorRate);

        ContributedTensor {
            tensor,
            coverage,
            kind: ContributorKind::Stream,
        }
    }

    fn contributor_kind(&self) -> ContributorKind {
        ContributorKind::Stream
    }

    fn module_id(&self) -> &'static str {
        "M49"
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ---- helpers ----

    fn default_manager() -> TrafficManager {
        TrafficManager::new()
    }

    fn register_default(mgr: &TrafficManager, id: &str) {
        mgr.register_service(id, TrafficConfig::default())
            .unwrap_or_else(|e| panic!("register failed: {e}"));
    }

    fn obs(latency_ms: f64, success: bool) -> RequestObservation {
        RequestObservation::new(latency_ms, success)
    }

    // ---- register / deregister ----

    #[test]
    fn test_register_service() {
        let mgr = default_manager();
        assert!(mgr.register_service("svc-a", TrafficConfig::default()).is_ok());
        assert_eq!(mgr.service_count(), 1);
    }

    #[test]
    fn test_register_duplicate_fails() {
        let mgr = default_manager();
        register_default(&mgr, "svc-a");
        let err = mgr
            .register_service("svc-a", TrafficConfig::default())
            .unwrap_err();
        assert!(err.to_string().contains("already registered"));
    }

    #[test]
    fn test_register_empty_id_fails() {
        let mgr = default_manager();
        let err = mgr
            .register_service("", TrafficConfig::default())
            .unwrap_err();
        assert!(err.to_string().contains("must not be empty"));
    }

    #[test]
    fn test_deregister_service() {
        let mgr = default_manager();
        register_default(&mgr, "svc-a");
        assert!(mgr.deregister_service("svc-a").is_ok());
        assert_eq!(mgr.service_count(), 0);
    }

    #[test]
    fn test_deregister_missing_fails() {
        let mgr = default_manager();
        let err = mgr.deregister_service("no-such").unwrap_err();
        assert!(err.to_string().contains("no-such"));
    }

    #[test]
    fn test_register_after_deregister() {
        let mgr = default_manager();
        register_default(&mgr, "svc-a");
        assert!(mgr.deregister_service("svc-a").is_ok());
        assert!(mgr.register_service("svc-a", TrafficConfig::default()).is_ok());
        assert_eq!(mgr.service_count(), 1);
    }

    // ---- record_request ----

    #[test]
    fn test_record_request_success() {
        let mgr = default_manager();
        register_default(&mgr, "svc-a");
        assert!(mgr.record_request("svc-a", obs(10.0, true)).is_ok());
    }

    #[test]
    fn test_record_request_empty_id_fails() {
        let mgr = default_manager();
        let err = mgr.record_request("", obs(10.0, true)).unwrap_err();
        assert!(err.to_string().contains("must not be empty"));
    }

    #[test]
    fn test_record_request_missing_service() {
        let mgr = default_manager();
        let err = mgr.record_request("no-such", obs(10.0, true)).unwrap_err();
        assert!(err.to_string().contains("no-such"));
    }

    #[test]
    fn test_record_request_failure() {
        let mgr = default_manager();
        register_default(&mgr, "svc-a");
        assert!(mgr.record_request("svc-a", obs(100.0, false)).is_ok());
        let snap = mgr.get_snapshot("svc-a").unwrap_or_else(|e| panic!("{e}"));
        assert!((snap.error_rate - 1.0).abs() < f64::EPSILON);
    }

    // ---- record_rejection ----

    #[test]
    fn test_record_rejection() {
        let mgr = default_manager();
        register_default(&mgr, "svc-a");
        assert!(mgr.record_rejection("svc-a").is_ok());
    }

    #[test]
    fn test_record_rejection_empty_id_fails() {
        let mgr = default_manager();
        let err = mgr.record_rejection("").unwrap_err();
        assert!(err.to_string().contains("must not be empty"));
    }

    #[test]
    fn test_record_rejection_missing_service() {
        let mgr = default_manager();
        let err = mgr.record_rejection("no-such").unwrap_err();
        assert!(err.to_string().contains("no-such"));
    }

    #[test]
    fn test_rejection_rate_computation() {
        let mgr = default_manager();
        register_default(&mgr, "svc-a");
        // 1 observation + 1 rejection = 2 total, rejection_rate = 0.5
        assert!(mgr.record_request("svc-a", obs(10.0, true)).is_ok());
        assert!(mgr.record_rejection("svc-a").is_ok());
        let snap = mgr.get_snapshot("svc-a").unwrap_or_else(|e| panic!("{e}"));
        assert!((snap.rejection_rate - 0.5).abs() < f64::EPSILON);
    }

    // ---- ring buffer eviction ----

    #[test]
    fn test_ring_buffer_eviction() {
        let mgr = default_manager();
        let config = TrafficConfig::builder().window_size(3).build();
        mgr.register_service("svc-a", config)
            .unwrap_or_else(|e| panic!("{e}"));
        // Push 5 observations; window_size=3, so only last 3 remain
        for i in 0..5 {
            #[allow(clippy::cast_precision_loss)]
            let lat = (i + 1) as f64 * 10.0;
            assert!(mgr.record_request("svc-a", obs(lat, true)).is_ok());
        }
        let snap = mgr.get_snapshot("svc-a").unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(snap.window_request_count, 3);
    }

    #[test]
    fn test_ring_buffer_evicts_oldest() {
        let mgr = default_manager();
        let config = TrafficConfig::builder().window_size(2).build();
        mgr.register_service("svc-a", config)
            .unwrap_or_else(|e| panic!("{e}"));
        // Obs 1: latency=100 success, Obs 2: latency=200 fail, Obs 3: latency=300 success
        // After 3 pushes with window_size=2: only Obs 2 (200) and Obs 3 (300) remain
        assert!(mgr.record_request("svc-a", obs(100.0, true)).is_ok());
        assert!(mgr.record_request("svc-a", obs(200.0, false)).is_ok());
        assert!(mgr.record_request("svc-a", obs(300.0, true)).is_ok());
        let snap = mgr.get_snapshot("svc-a").unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(snap.window_request_count, 2);
        // p50 of [200, 300]: index floor(0.5 * 2) = 1 → sorted[1] = 300.0
        assert!((snap.p50_latency_ms - 300.0).abs() < f64::EPSILON);
        // Error rate should be 0.5 (1 fail out of 2)
        assert!((snap.error_rate - 0.5).abs() < f64::EPSILON);
    }

    // ---- percentile computation ----

    #[test]
    fn test_percentile_empty_window() {
        let mgr = default_manager();
        register_default(&mgr, "svc-a");
        let snap = mgr.get_snapshot("svc-a").unwrap_or_else(|e| panic!("{e}"));
        assert!((snap.p50_latency_ms).abs() < f64::EPSILON);
        assert!((snap.p95_latency_ms).abs() < f64::EPSILON);
        assert!((snap.p99_latency_ms).abs() < f64::EPSILON);
    }

    #[test]
    fn test_percentile_single_observation() {
        let mgr = default_manager();
        register_default(&mgr, "svc-a");
        assert!(mgr.record_request("svc-a", obs(42.0, true)).is_ok());
        let snap = mgr.get_snapshot("svc-a").unwrap_or_else(|e| panic!("{e}"));
        assert!((snap.p50_latency_ms - 42.0).abs() < f64::EPSILON);
        assert!((snap.p95_latency_ms - 42.0).abs() < f64::EPSILON);
        assert!((snap.p99_latency_ms - 42.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_percentile_multiple_observations() {
        let mgr = default_manager();
        let config = TrafficConfig::builder().window_size(10).build();
        mgr.register_service("svc-a", config)
            .unwrap_or_else(|e| panic!("{e}"));
        // Push latencies 1..=10
        for i in 1..=10 {
            #[allow(clippy::cast_precision_loss)]
            let lat = i as f64;
            assert!(mgr.record_request("svc-a", obs(lat, true)).is_ok());
        }
        let snap = mgr.get_snapshot("svc-a").unwrap_or_else(|e| panic!("{e}"));
        // p50 of [1..=10]: index floor(0.5 * 10) = 5 → sorted[5] = 6.0
        assert!((snap.p50_latency_ms - 6.0).abs() < f64::EPSILON);
        // p95: index floor(0.95 * 10) = 9 → sorted[9] = 10.0
        assert!((snap.p95_latency_ms - 10.0).abs() < f64::EPSILON);
        // p99: index floor(0.99 * 10) = 9 → sorted[9] = 10.0
        assert!((snap.p99_latency_ms - 10.0).abs() < f64::EPSILON);
    }

    // ---- traffic_health ----

    #[test]
    fn test_traffic_health_perfect() {
        let mgr = default_manager();
        register_default(&mgr, "svc-a");
        // All successes, low latency, no rejections → health ~1.0
        for _ in 0..10 {
            assert!(mgr.record_request("svc-a", obs(1.0, true)).is_ok());
        }
        let snap = mgr.get_snapshot("svc-a").unwrap_or_else(|e| panic!("{e}"));
        assert!(snap.traffic_health > 0.99);
    }

    #[test]
    fn test_traffic_health_degraded_by_errors() {
        let mgr = default_manager();
        register_default(&mgr, "svc-a");
        // All failures → error_rate=1.0, penalty += 0.3
        for _ in 0..10 {
            assert!(mgr.record_request("svc-a", obs(1.0, false)).is_ok());
        }
        let snap = mgr.get_snapshot("svc-a").unwrap_or_else(|e| panic!("{e}"));
        assert!(snap.traffic_health < 0.75);
    }

    #[test]
    fn test_traffic_health_degraded_by_latency() {
        let mgr = default_manager();
        let config = TrafficConfig::builder().max_latency_ms(100.0).build();
        mgr.register_service("svc-a", config)
            .unwrap_or_else(|e| panic!("{e}"));
        // Latency at ceiling → latency_ratio = 1.0, penalty += 0.3
        for _ in 0..10 {
            assert!(mgr.record_request("svc-a", obs(100.0, true)).is_ok());
        }
        let snap = mgr.get_snapshot("svc-a").unwrap_or_else(|e| panic!("{e}"));
        assert!((snap.traffic_health - 0.7).abs() < 0.01);
    }

    #[test]
    fn test_traffic_health_degraded_by_rejections() {
        let mgr = default_manager();
        register_default(&mgr, "svc-a");
        // 10 rejections, 0 observations → rejection_rate = 1.0, penalty += 0.4
        for _ in 0..10 {
            assert!(mgr.record_rejection("svc-a").is_ok());
        }
        let snap = mgr.get_snapshot("svc-a").unwrap_or_else(|e| panic!("{e}"));
        assert!((snap.traffic_health - 0.6).abs() < 0.01);
    }

    #[test]
    fn test_traffic_health_clamped_floor() {
        let mgr = default_manager();
        let config = TrafficConfig::builder().max_latency_ms(1.0).build();
        mgr.register_service("svc-a", config)
            .unwrap_or_else(|e| panic!("{e}"));
        // All failures + high latency + many rejections → health should clamp at 0
        for _ in 0..5 {
            assert!(mgr.record_request("svc-a", obs(1000.0, false)).is_ok());
            assert!(mgr.record_rejection("svc-a").is_ok());
        }
        let snap = mgr.get_snapshot("svc-a").unwrap_or_else(|e| panic!("{e}"));
        assert!(snap.traffic_health >= 0.0);
    }

    // ---- aggregate_traffic_health ----

    #[test]
    fn test_aggregate_traffic_health_no_services() {
        let mgr = default_manager();
        assert!((mgr.aggregate_traffic_health() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_aggregate_traffic_health_multiple() {
        let mgr = default_manager();
        register_default(&mgr, "svc-a");
        register_default(&mgr, "svc-b");
        // svc-a: all good
        for _ in 0..5 {
            assert!(mgr.record_request("svc-a", obs(1.0, true)).is_ok());
        }
        // svc-b: all failures
        for _ in 0..5 {
            assert!(mgr.record_request("svc-b", obs(1.0, false)).is_ok());
        }
        let health = mgr.aggregate_traffic_health();
        // Average of ~1.0 and ~0.7 → ~0.85
        assert!(health > 0.5);
        assert!(health < 1.0);
    }

    // ---- saturated services ----

    #[test]
    fn test_get_saturated_services_none() {
        let mgr = default_manager();
        register_default(&mgr, "svc-a");
        for _ in 0..10 {
            assert!(mgr.record_request("svc-a", obs(1.0, true)).is_ok());
        }
        assert!(mgr.get_saturated_services().is_empty());
    }

    #[test]
    fn test_get_saturated_services_above_threshold() {
        let mgr = default_manager();
        let config = TrafficConfig::builder().rejection_threshold(0.05).build();
        mgr.register_service("svc-a", config)
            .unwrap_or_else(|e| panic!("{e}"));
        // 1 obs + 1 rejection → rate = 0.5 > 0.05
        assert!(mgr.record_request("svc-a", obs(1.0, true)).is_ok());
        assert!(mgr.record_rejection("svc-a").is_ok());
        let saturated = mgr.get_saturated_services();
        assert_eq!(saturated.len(), 1);
        assert_eq!(saturated[0], "svc-a");
    }

    // ---- mean_latency_ms ----

    #[test]
    fn test_mean_latency_no_services() {
        let mgr = default_manager();
        assert!((mgr.mean_latency_ms()).abs() < f64::EPSILON);
    }

    #[test]
    fn test_mean_latency_single_service() {
        let mgr = default_manager();
        register_default(&mgr, "svc-a");
        assert!(mgr.record_request("svc-a", obs(20.0, true)).is_ok());
        assert!(mgr.record_request("svc-a", obs(40.0, true)).is_ok());
        assert!((mgr.mean_latency_ms() - 30.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_mean_latency_multiple_services() {
        let mgr = default_manager();
        register_default(&mgr, "svc-a");
        register_default(&mgr, "svc-b");
        assert!(mgr.record_request("svc-a", obs(10.0, true)).is_ok());
        assert!(mgr.record_request("svc-b", obs(30.0, true)).is_ok());
        // Mean of means: (10 + 30) / 2 = 20
        assert!((mgr.mean_latency_ms() - 20.0).abs() < f64::EPSILON);
    }

    // ---- mean_rejection_rate ----

    #[test]
    fn test_mean_rejection_rate_no_services() {
        let mgr = default_manager();
        assert!((mgr.mean_rejection_rate()).abs() < f64::EPSILON);
    }

    #[test]
    fn test_mean_rejection_rate_mixed() {
        let mgr = default_manager();
        register_default(&mgr, "svc-a");
        register_default(&mgr, "svc-b");
        // svc-a: 0 rejections / 2 total = 0
        assert!(mgr.record_request("svc-a", obs(1.0, true)).is_ok());
        assert!(mgr.record_request("svc-a", obs(1.0, true)).is_ok());
        // svc-b: 2 rejections / 2 total = 1.0
        assert!(mgr.record_rejection("svc-b").is_ok());
        assert!(mgr.record_rejection("svc-b").is_ok());
        // Average: (0 + 1.0) / 2 = 0.5
        assert!((mgr.mean_rejection_rate() - 0.5).abs() < f64::EPSILON);
    }

    // ---- reset_all ----

    #[test]
    fn test_reset_all() {
        let mgr = default_manager();
        register_default(&mgr, "svc-a");
        register_default(&mgr, "svc-b");
        assert_eq!(mgr.service_count(), 2);
        mgr.reset_all();
        assert_eq!(mgr.service_count(), 0);
    }

    // ---- get_snapshot ----

    #[test]
    fn test_get_snapshot_empty_id_fails() {
        let mgr = default_manager();
        let err = mgr.get_snapshot("").unwrap_err();
        assert!(err.to_string().contains("must not be empty"));
    }

    #[test]
    fn test_get_snapshot_missing_service() {
        let mgr = default_manager();
        let err = mgr.get_snapshot("no-such").unwrap_err();
        assert!(err.to_string().contains("no-such"));
    }

    #[test]
    fn test_get_snapshot_returns_owned() {
        let mgr = default_manager();
        register_default(&mgr, "svc-a");
        assert!(mgr.record_request("svc-a", obs(50.0, true)).is_ok());
        let snap = mgr.get_snapshot("svc-a").unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(snap.service_id, "svc-a");
        assert_eq!(snap.window_request_count, 1);
    }

    // ---- get_all_snapshots ----

    #[test]
    fn test_get_all_snapshots_empty() {
        let mgr = default_manager();
        assert!(mgr.get_all_snapshots().is_empty());
    }

    #[test]
    fn test_get_all_snapshots_multiple() {
        let mgr = default_manager();
        register_default(&mgr, "svc-a");
        register_default(&mgr, "svc-b");
        let snaps = mgr.get_all_snapshots();
        assert_eq!(snaps.len(), 2);
    }

    // ---- service_count ----

    #[test]
    fn test_service_count_empty() {
        let mgr = default_manager();
        assert_eq!(mgr.service_count(), 0);
    }

    #[test]
    fn test_service_count_after_register_deregister() {
        let mgr = default_manager();
        register_default(&mgr, "svc-a");
        register_default(&mgr, "svc-b");
        assert_eq!(mgr.service_count(), 2);
        assert!(mgr.deregister_service("svc-a").is_ok());
        assert_eq!(mgr.service_count(), 1);
    }

    // ---- empty state defaults ----

    #[test]
    fn test_empty_window_snapshot_all_zeros() {
        let mgr = default_manager();
        register_default(&mgr, "svc-a");
        let snap = mgr.get_snapshot("svc-a").unwrap_or_else(|e| panic!("{e}"));
        assert!((snap.rps).abs() < f64::EPSILON);
        assert!((snap.p50_latency_ms).abs() < f64::EPSILON);
        assert!((snap.p95_latency_ms).abs() < f64::EPSILON);
        assert!((snap.p99_latency_ms).abs() < f64::EPSILON);
        assert!((snap.rejection_rate).abs() < f64::EPSILON);
        assert!((snap.error_rate).abs() < f64::EPSILON);
        assert_eq!(snap.window_request_count, 0);
    }

    #[test]
    fn test_empty_window_traffic_health_is_one() {
        let mgr = default_manager();
        register_default(&mgr, "svc-a");
        let snap = mgr.get_snapshot("svc-a").unwrap_or_else(|e| panic!("{e}"));
        assert!((snap.traffic_health - 1.0).abs() < f64::EPSILON);
    }

    // ---- TrafficConfig builder ----

    #[test]
    fn test_config_builder_defaults() {
        let config = TrafficConfig::builder().build();
        assert!((config.max_rps - DEFAULT_MAX_RPS).abs() < f64::EPSILON);
        assert_eq!(config.max_concurrency, DEFAULT_MAX_CONCURRENCY);
        assert_eq!(config.max_queue_depth, DEFAULT_MAX_QUEUE_DEPTH);
        assert!((config.max_latency_ms - DEFAULT_MAX_LATENCY_MS).abs() < f64::EPSILON);
        assert!((config.rejection_threshold - DEFAULT_REJECTION_THRESHOLD).abs() < f64::EPSILON);
        assert_eq!(config.window_size, DEFAULT_WINDOW_SIZE);
    }

    #[test]
    fn test_config_builder_custom() {
        let config = TrafficConfig::builder()
            .max_rps(500.0)
            .max_concurrency(50)
            .max_queue_depth(250)
            .max_latency_ms(1000.0)
            .rejection_threshold(0.05)
            .window_size(50)
            .build();
        assert!((config.max_rps - 500.0).abs() < f64::EPSILON);
        assert_eq!(config.max_concurrency, 50);
        assert_eq!(config.max_queue_depth, 250);
        assert!((config.max_latency_ms - 1000.0).abs() < f64::EPSILON);
        assert!((config.rejection_threshold - 0.05).abs() < f64::EPSILON);
        assert_eq!(config.window_size, 50);
    }

    #[test]
    fn test_config_default_trait() {
        let config = TrafficConfig::default();
        assert!((config.max_rps - DEFAULT_MAX_RPS).abs() < f64::EPSILON);
    }

    #[test]
    fn test_config_builder_default_trait() {
        let builder = TrafficConfigBuilder::default();
        let config = builder.build();
        assert!((config.max_rps - DEFAULT_MAX_RPS).abs() < f64::EPSILON);
    }

    // ---- RequestObservation ----

    #[test]
    fn test_request_observation_new() {
        let obs = RequestObservation::new(42.0, true);
        assert!((obs.latency_ms - 42.0).abs() < f64::EPSILON);
        assert!(obs.success);
        assert!(obs.timestamp.ticks() > 0);
    }

    #[test]
    fn test_request_observation_with_timestamp() {
        let ts = Timestamp::from_raw(999);
        let obs = RequestObservation::with_timestamp(10.0, false, ts);
        assert!((obs.latency_ms - 10.0).abs() < f64::EPSILON);
        assert!(!obs.success);
        assert_eq!(obs.timestamp.ticks(), 999);
    }

    // ---- TensorContributor ----

    #[test]
    fn test_tensor_contributor_empty() {
        let mgr = default_manager();
        let ct = mgr.contribute();
        // No services → D9 = 1.0 (healthy), D10 = 0.0 (no errors)
        let values = ct.tensor.to_array();
        assert!((values[DimensionIndex::Latency as usize] - 1.0).abs() < f64::EPSILON);
        assert!((values[DimensionIndex::ErrorRate as usize]).abs() < f64::EPSILON);
    }

    #[test]
    fn test_tensor_contributor_with_data() {
        let mgr = default_manager();
        let config = TrafficConfig::builder().max_latency_ms(100.0).build();
        mgr.register_service("svc-a", config)
            .unwrap_or_else(|e| panic!("{e}"));
        // latency=50 → ratio=0.5, D9 = 1.0 - 0.5 = 0.5
        assert!(mgr.record_request("svc-a", obs(50.0, true)).is_ok());
        let ct = mgr.contribute();
        let values = ct.tensor.to_array();
        assert!((values[DimensionIndex::Latency as usize] - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_tensor_contributor_kind() {
        let mgr = default_manager();
        assert_eq!(mgr.contributor_kind(), ContributorKind::Stream);
    }

    #[test]
    fn test_tensor_contributor_module_id() {
        let mgr = default_manager();
        assert_eq!(mgr.module_id(), "M49");
    }

    #[test]
    fn test_tensor_coverage_dimensions() {
        let mgr = default_manager();
        let ct = mgr.contribute();
        assert!(ct.coverage.is_covered(DimensionIndex::Latency));
        assert!(ct.coverage.is_covered(DimensionIndex::ErrorRate));
        // Other dimensions should NOT be covered
        assert!(!ct.coverage.is_covered(DimensionIndex::ServiceId));
        assert!(!ct.coverage.is_covered(DimensionIndex::HealthScore));
    }

    // ---- concurrent access ----

    #[test]
    fn test_concurrent_register_and_record() {
        use std::sync::Arc;
        use std::thread;

        let mgr = Arc::new(default_manager());
        register_default(&mgr, "svc-shared");

        let handles: Vec<_> = (0..4)
            .map(|i| {
                let mgr = Arc::clone(&mgr);
                thread::spawn(move || {
                    for j in 0..25 {
                        #[allow(clippy::cast_precision_loss)]
                        let lat = (i * 25 + j) as f64;
                        let _ = mgr.record_request("svc-shared", obs(lat, true));
                    }
                })
            })
            .collect();

        for h in handles {
            h.join().unwrap_or_else(|e| panic!("thread panicked: {e:?}"));
        }

        let snap = mgr
            .get_snapshot("svc-shared")
            .unwrap_or_else(|e| panic!("{e}"));
        // 4 threads * 25 obs = 100 total, but window_size=100 so all fit
        assert_eq!(snap.window_request_count, 100);
    }

    #[test]
    fn test_concurrent_register_different_services() {
        use std::sync::Arc;
        use std::thread;

        let mgr = Arc::new(default_manager());

        let handles: Vec<_> = (0..10)
            .map(|i| {
                let mgr = Arc::clone(&mgr);
                thread::spawn(move || {
                    let id = format!("svc-{i}");
                    mgr.register_service(&id, TrafficConfig::default())
                })
            })
            .collect();

        let mut ok_count = 0;
        for h in handles {
            if h.join().unwrap_or_else(|e| panic!("thread panicked: {e:?}")).is_ok() {
                ok_count += 1;
            }
        }
        assert_eq!(ok_count, 10);
        assert_eq!(mgr.service_count(), 10);
    }

    // ---- rps estimation ----

    #[test]
    fn test_rps_zero_observations() {
        let mgr = default_manager();
        register_default(&mgr, "svc-a");
        let snap = mgr.get_snapshot("svc-a").unwrap_or_else(|e| panic!("{e}"));
        assert!((snap.rps).abs() < f64::EPSILON);
    }

    #[test]
    fn test_rps_single_observation() {
        let mgr = default_manager();
        register_default(&mgr, "svc-a");
        assert!(mgr.record_request("svc-a", obs(1.0, true)).is_ok());
        let snap = mgr.get_snapshot("svc-a").unwrap_or_else(|e| panic!("{e}"));
        // Single observation → rps = 0 (need >=2 for span)
        assert!((snap.rps).abs() < f64::EPSILON);
    }

    // ---- TrafficManager Debug ----

    #[test]
    fn test_traffic_manager_debug() {
        let mgr = default_manager();
        let debug = format!("{mgr:?}");
        assert!(debug.contains("TrafficManager"));
    }

    // ---- TrafficManager Default ----

    #[test]
    fn test_traffic_manager_default() {
        let mgr = TrafficManager::default();
        assert_eq!(mgr.service_count(), 0);
    }

    // ---- TrafficConfig Clone + PartialEq ----

    #[test]
    fn test_traffic_config_clone_eq() {
        let config = TrafficConfig::builder().max_rps(123.0).build();
        let clone = config.clone();
        assert_eq!(config, clone);
    }

    // ---- TrafficSnapshot fields ----

    #[test]
    fn test_snapshot_service_id() {
        let mgr = default_manager();
        register_default(&mgr, "my-svc");
        let snap = mgr.get_snapshot("my-svc").unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(snap.service_id, "my-svc");
    }

    // ---- edge cases ----

    #[test]
    fn test_max_latency_zero_no_panic() {
        let mgr = default_manager();
        let config = TrafficConfig::builder().max_latency_ms(0.0).build();
        mgr.register_service("svc-a", config)
            .unwrap_or_else(|e| panic!("{e}"));
        assert!(mgr.record_request("svc-a", obs(100.0, true)).is_ok());
        let snap = mgr.get_snapshot("svc-a").unwrap_or_else(|e| panic!("{e}"));
        // Should not panic, health still valid
        assert!(snap.traffic_health >= 0.0);
        assert!(snap.traffic_health <= 1.0);
    }

    #[test]
    fn test_window_size_one() {
        let mgr = default_manager();
        let config = TrafficConfig::builder().window_size(1).build();
        mgr.register_service("svc-a", config)
            .unwrap_or_else(|e| panic!("{e}"));
        assert!(mgr.record_request("svc-a", obs(10.0, true)).is_ok());
        assert!(mgr.record_request("svc-a", obs(20.0, true)).is_ok());
        let snap = mgr.get_snapshot("svc-a").unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(snap.window_request_count, 1);
        // Only the last observation remains (latency=20)
        assert!((snap.p50_latency_ms - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_many_rejections_no_overflow() {
        let mgr = default_manager();
        register_default(&mgr, "svc-a");
        for _ in 0..1000 {
            assert!(mgr.record_rejection("svc-a").is_ok());
        }
        let snap = mgr.get_snapshot("svc-a").unwrap_or_else(|e| panic!("{e}"));
        assert!((snap.rejection_rate - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_large_latency_values() {
        let mgr = default_manager();
        register_default(&mgr, "svc-a");
        assert!(mgr.record_request("svc-a", obs(999_999.0, true)).is_ok());
        let snap = mgr.get_snapshot("svc-a").unwrap_or_else(|e| panic!("{e}"));
        assert!((snap.p50_latency_ms - 999_999.0).abs() < f64::EPSILON);
    }
}
