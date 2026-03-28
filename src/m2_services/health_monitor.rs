//! # M10: Health Monitor
//!
//! Health check orchestration for ULTRAPLATE services with trait-based
//! abstraction, interior mutability, and 12D tensor contribution.
//!
//! ## Layer: L2 (Services)
//! ## Module: M10
//! ## Dependencies: L1 (Error, Timestamp, `ModuleId`, `SignalBus`, `TensorContributor`)
//!
//! ## Trait: [`HealthMonitoring`]
//!
//! All methods are `&self` (C2) with interior mutability via `RwLock`.
//! Methods returning data through `RwLock` return OWNED types (C7).
//!
//! ## 12D Tensor Contribution (C3)
//!
//! | Dimension | Value |
//! |-----------|-------|
//! | D6 (`health_score`) | `aggregate_health()` across all probes |
//! | D10 (`error_rate`) | fraction of unhealthy services |
//!
//! ## Signal Emission (C6)
//!
//! `record_result()` emits [`HealthSignal`] on consecutive threshold crossings.

use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use parking_lot::RwLock;

use super::HealthStatus;
use crate::m1_foundation::shared_types::{CoverageBitmap, DimensionIndex, ModuleId, Timestamp};
use crate::m1_foundation::signals::{HealthSignal, SignalBus};
use crate::m1_foundation::tensor_registry::{ContributedTensor, ContributorKind, TensorContributor};
use crate::m1_foundation::MetricsRegistry;
use crate::{Error, Result, Tensor12D};

// ============================================================================
// Constants
// ============================================================================

/// Default health check interval in milliseconds.
const DEFAULT_INTERVAL_MS: u64 = 30_000;

/// Default health check timeout in milliseconds.
const DEFAULT_TIMEOUT_MS: u64 = 5_000;

/// Default consecutive successes to mark healthy.
const DEFAULT_HEALTHY_THRESHOLD: u32 = 3;

/// Default consecutive failures to mark unhealthy.
const DEFAULT_UNHEALTHY_THRESHOLD: u32 = 3;

/// Default maximum history entries per service.
const DEFAULT_MAX_HISTORY: usize = 100;

// ============================================================================
// HealthMonitoring (trait)
// ============================================================================

/// Trait for health monitoring operations.
///
/// All methods are `&self` (C2) with interior mutability via `RwLock`.
pub trait HealthMonitoring: Send + Sync + fmt::Debug {
    /// Register a new health probe.
    ///
    /// # Errors
    /// Returns [`Error::Validation`] if a probe with the same `service_id` exists.
    fn register_probe(&self, probe: HealthProbe) -> Result<()>;

    /// Remove a probe and its history.
    ///
    /// # Errors
    /// Returns [`Error::ServiceNotFound`] if no probe exists for the service.
    fn unregister_probe(&self, service_id: &str) -> Result<()>;

    /// Return the number of registered probes.
    fn probe_count(&self) -> usize;

    /// Record a health check result and update status.
    ///
    /// # Errors
    /// Returns [`Error::ServiceNotFound`] if no probe exists for the service.
    fn record_result(&self, service_id: &str, result: HealthCheckResult) -> Result<()>;

    /// Get the current resolved status for a service.
    ///
    /// # Errors
    /// Returns [`Error::ServiceNotFound`] if no probe exists.
    fn get_status(&self, service_id: &str) -> Result<HealthStatus>;

    /// Get the check history for a service (owned, C7).
    ///
    /// # Errors
    /// Returns [`Error::ServiceNotFound`] if no probe exists.
    fn get_history(&self, service_id: &str) -> Result<Vec<HealthCheckResult>>;

    /// Get all current statuses.
    fn get_all_statuses(&self) -> HashMap<String, HealthStatus>;

    /// Compute aggregate health score across all probes (0.0–1.0).
    fn aggregate_health(&self) -> f64;

    /// Return IDs of degraded services.
    fn get_degraded_services(&self) -> Vec<String>;

    /// Return IDs of unhealthy services.
    fn get_unhealthy_services(&self) -> Vec<String>;

    /// Return IDs of healthy services.
    fn get_healthy_services(&self) -> Vec<String>;
}

// ============================================================================
// HealthProbe
// ============================================================================

/// Configuration for a health check probe.
#[derive(Clone, Debug)]
pub struct HealthProbe {
    /// Service being probed.
    pub service_id: String,
    /// Endpoint URL.
    pub endpoint: String,
    /// Interval between checks in milliseconds.
    pub interval_ms: u64,
    /// Timeout in milliseconds.
    pub timeout_ms: u64,
    /// Consecutive successes to mark healthy.
    pub healthy_threshold: u32,
    /// Consecutive failures to mark unhealthy.
    pub unhealthy_threshold: u32,
}

impl fmt::Display for HealthProbe {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Probe({} -> {} every {}ms)",
            self.service_id, self.endpoint, self.interval_ms
        )
    }
}

// ============================================================================
// HealthProbeBuilder
// ============================================================================

/// Builder for [`HealthProbe`] with ergonomic defaults.
///
/// # Examples
///
/// ```rust
/// use maintenance_engine::m2_services::health_monitor::HealthProbeBuilder;
///
/// let probe = HealthProbeBuilder::new("nais", "http://localhost:8101/health")
///     .interval_ms(15_000)
///     .healthy_threshold(2)
///     .unhealthy_threshold(5)
///     .build()
///     .expect("valid probe config");
///
/// assert_eq!(probe.service_id, "nais");
/// assert_eq!(probe.interval_ms, 15_000);
/// ```
#[derive(Clone, Debug)]
pub struct HealthProbeBuilder {
    service_id: String,
    endpoint: String,
    interval_ms: u64,
    timeout_ms: u64,
    healthy_threshold: u32,
    unhealthy_threshold: u32,
}

impl HealthProbeBuilder {
    /// Create a new builder with required fields.
    #[must_use]
    pub fn new(service_id: impl Into<String>, endpoint: impl Into<String>) -> Self {
        Self {
            service_id: service_id.into(),
            endpoint: endpoint.into(),
            interval_ms: DEFAULT_INTERVAL_MS,
            timeout_ms: DEFAULT_TIMEOUT_MS,
            healthy_threshold: DEFAULT_HEALTHY_THRESHOLD,
            unhealthy_threshold: DEFAULT_UNHEALTHY_THRESHOLD,
        }
    }

    /// Set the check interval in milliseconds.
    #[must_use]
    pub const fn interval_ms(mut self, ms: u64) -> Self {
        self.interval_ms = ms;
        self
    }

    /// Set the request timeout in milliseconds.
    #[must_use]
    pub const fn timeout_ms(mut self, ms: u64) -> Self {
        self.timeout_ms = ms;
        self
    }

    /// Set the consecutive-success threshold.
    #[must_use]
    pub const fn healthy_threshold(mut self, threshold: u32) -> Self {
        self.healthy_threshold = threshold;
        self
    }

    /// Set the consecutive-failure threshold.
    #[must_use]
    pub const fn unhealthy_threshold(mut self, threshold: u32) -> Self {
        self.unhealthy_threshold = threshold;
        self
    }

    /// Build the probe, validating all fields.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] if any field is invalid.
    pub fn build(self) -> Result<HealthProbe> {
        if self.service_id.is_empty() {
            return Err(Error::Validation(
                "HealthProbe service_id must not be empty".into(),
            ));
        }
        if self.endpoint.is_empty() {
            return Err(Error::Validation(
                "HealthProbe endpoint must not be empty".into(),
            ));
        }
        if self.interval_ms == 0 {
            return Err(Error::Validation(
                "HealthProbe interval_ms must be greater than zero".into(),
            ));
        }
        if self.timeout_ms == 0 {
            return Err(Error::Validation(
                "HealthProbe timeout_ms must be greater than zero".into(),
            ));
        }
        if self.timeout_ms > self.interval_ms {
            return Err(Error::Validation(
                "HealthProbe timeout_ms must not exceed interval_ms".into(),
            ));
        }
        if self.healthy_threshold == 0 {
            return Err(Error::Validation(
                "HealthProbe healthy_threshold must be greater than zero".into(),
            ));
        }
        if self.unhealthy_threshold == 0 {
            return Err(Error::Validation(
                "HealthProbe unhealthy_threshold must be greater than zero".into(),
            ));
        }

        Ok(HealthProbe {
            service_id: self.service_id,
            endpoint: self.endpoint,
            interval_ms: self.interval_ms,
            timeout_ms: self.timeout_ms,
            healthy_threshold: self.healthy_threshold,
            unhealthy_threshold: self.unhealthy_threshold,
        })
    }
}

// ============================================================================
// HealthCheckResult
// ============================================================================

/// Outcome of a single health check execution.
#[derive(Clone, Debug)]
pub struct HealthCheckResult {
    /// Service that was checked.
    pub service_id: String,
    /// Observed health status.
    pub status: HealthStatus,
    /// Round-trip response time in milliseconds.
    pub response_time_ms: u64,
    /// Timestamp of the check (C5: Timestamp, not `SystemTime`).
    pub timestamp: Timestamp,
    /// Optional diagnostic message.
    pub message: Option<String>,
    /// Optional HTTP status code.
    pub status_code: Option<u16>,
}

impl HealthCheckResult {
    /// Create a successful health check result.
    #[must_use]
    pub fn success(service_id: impl Into<String>, response_time_ms: u64) -> Self {
        Self {
            service_id: service_id.into(),
            status: HealthStatus::Healthy,
            response_time_ms,
            timestamp: Timestamp::now(),
            message: Some("Health check passed".into()),
            status_code: Some(200),
        }
    }

    /// Create a failed health check result.
    #[must_use]
    pub fn failure(service_id: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            service_id: service_id.into(),
            status: HealthStatus::Unhealthy,
            response_time_ms: 0,
            timestamp: Timestamp::now(),
            message: Some(message.into()),
            status_code: None,
        }
    }

    /// Whether the check was successful.
    #[must_use]
    pub fn is_success(&self) -> bool {
        self.status == HealthStatus::Healthy
    }
}

impl fmt::Display for HealthCheckResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Check({} {} {}ms at {})",
            self.service_id, self.status, self.response_time_ms, self.timestamp
        )
    }
}

// ============================================================================
// Internal state
// ============================================================================

/// Per-service monitoring state.
#[derive(Debug)]
struct ServiceMonitorState {
    probe: HealthProbe,
    history: Vec<HealthCheckResult>,
    current_status: HealthStatus,
    consecutive_successes: u32,
    consecutive_failures: u32,
}

/// Interior state of the health monitor, protected by `RwLock`.
#[derive(Debug, Default)]
struct MonitorState {
    services: HashMap<String, ServiceMonitorState>,
    max_history: usize,
}

// ============================================================================
// HealthMonitor
// ============================================================================

/// Concrete implementation of [`HealthMonitoring`] with interior mutability.
///
/// # Examples
///
/// ```rust
/// use maintenance_engine::m2_services::health_monitor::{HealthMonitor, HealthMonitoring, HealthProbeBuilder};
///
/// let monitor = HealthMonitor::new();
/// let probe = HealthProbeBuilder::new("synthex", "http://localhost:8090/api/health")
///     .build()
///     .expect("valid probe");
///
/// monitor.register_probe(probe).expect("register");
/// assert_eq!(monitor.probe_count(), 1);
/// ```
#[derive(Debug)]
pub struct HealthMonitor {
    state: RwLock<MonitorState>,
    signal_bus: Option<Arc<SignalBus>>,
    metrics: Option<Arc<MetricsRegistry>>,
}

impl Default for HealthMonitor {
    fn default() -> Self {
        Self::new()
    }
}

impl HealthMonitor {
    /// Create a new health monitor with default settings.
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: RwLock::new(MonitorState {
                services: HashMap::new(),
                max_history: DEFAULT_MAX_HISTORY,
            }),
            signal_bus: None,
            metrics: None,
        }
    }

    /// Create with custom max history.
    #[must_use]
    pub fn with_max_history(max_history: usize) -> Self {
        Self {
            state: RwLock::new(MonitorState {
                services: HashMap::new(),
                max_history,
            }),
            signal_bus: None,
            metrics: None,
        }
    }

    /// Attach a signal bus for health transition signals.
    #[must_use]
    pub fn with_signal_bus(mut self, bus: Arc<SignalBus>) -> Self {
        self.signal_bus = Some(bus);
        self
    }

    /// Attach a metrics registry.
    #[must_use]
    pub fn with_metrics(mut self, metrics: Arc<MetricsRegistry>) -> Self {
        self.metrics = Some(metrics);
        self
    }

    /// Emit a health signal on status transition.
    fn emit_health_transition(&self, service_id: &str, previous: HealthStatus, current: HealthStatus) {
        if let Some(ref bus) = self.signal_bus {
            let signal = HealthSignal::new(
                ModuleId::M10,
                previous.score(),
                current.score(),
                format!("Health monitor: '{service_id}' {previous} -> {current}"),
            );
            bus.emit_health(&signal);
        }
    }
}

impl HealthMonitoring for HealthMonitor {
    fn register_probe(&self, probe: HealthProbe) -> Result<()> {
        let mut state = self.state.write();
        if state.services.contains_key(&probe.service_id) {
            return Err(Error::Validation(format!(
                "Probe already registered for service '{}'",
                probe.service_id
            )));
        }
        let id = probe.service_id.clone();
        state.services.insert(id, ServiceMonitorState {
            probe,
            history: Vec::new(),
            current_status: HealthStatus::Unknown,
            consecutive_successes: 0,
            consecutive_failures: 0,
        });
        drop(state);
        Ok(())
    }

    fn unregister_probe(&self, service_id: &str) -> Result<()> {
        let mut state = self.state.write();
        if state.services.remove(service_id).is_none() {
            return Err(Error::ServiceNotFound(service_id.to_owned()));
        }
        drop(state);
        Ok(())
    }

    fn probe_count(&self) -> usize {
        self.state.read().services.len()
    }

    fn record_result(&self, service_id: &str, result: HealthCheckResult) -> Result<()> {
        let mut state = self.state.write();
        let max_history = state.max_history;
        let svc = state
            .services
            .get_mut(service_id)
            .ok_or_else(|| Error::ServiceNotFound(service_id.to_owned()))?;

        let previous_status = svc.current_status;
        let healthy_threshold = svc.probe.healthy_threshold;
        let unhealthy_threshold = svc.probe.unhealthy_threshold;

        // Update consecutive counters
        if result.is_success() {
            svc.consecutive_successes += 1;
            svc.consecutive_failures = 0;
        } else {
            svc.consecutive_failures += 1;
            svc.consecutive_successes = 0;
        }

        // Threshold-based status transitions
        if svc.consecutive_successes >= healthy_threshold {
            svc.current_status = HealthStatus::Healthy;
        } else if svc.consecutive_failures >= unhealthy_threshold {
            svc.current_status = HealthStatus::Unhealthy;
        } else if svc.consecutive_failures > 0 && svc.current_status == HealthStatus::Healthy {
            svc.current_status = HealthStatus::Degraded;
        }

        let new_status = svc.current_status;

        // Push to history, trim if needed
        svc.history.push(result);
        if svc.history.len() > max_history {
            let overflow = svc.history.len() - max_history;
            svc.history.drain(..overflow);
        }

        drop(state);

        // Emit signal on transition
        if previous_status != new_status {
            self.emit_health_transition(service_id, previous_status, new_status);
        }
        Ok(())
    }

    fn get_status(&self, service_id: &str) -> Result<HealthStatus> {
        let state = self.state.read();
        state
            .services
            .get(service_id)
            .map(|s| s.current_status)
            .ok_or_else(|| Error::ServiceNotFound(service_id.to_owned()))
    }

    fn get_history(&self, service_id: &str) -> Result<Vec<HealthCheckResult>> {
        let state = self.state.read();
        state
            .services
            .get(service_id)
            .map(|s| s.history.clone())
            .ok_or_else(|| Error::ServiceNotFound(service_id.to_owned()))
    }

    fn get_all_statuses(&self) -> HashMap<String, HealthStatus> {
        let state = self.state.read();
        state
            .services
            .iter()
            .map(|(id, s)| (id.clone(), s.current_status))
            .collect()
    }

    #[allow(clippy::cast_precision_loss)]
    fn aggregate_health(&self) -> f64 {
        let state = self.state.read();
        let total = state.services.len();
        if total == 0 {
            return 1.0;
        }
        let sum: f64 = state.services.values().map(|s| s.current_status.score()).sum();
        drop(state);
        sum / total as f64
    }

    fn get_degraded_services(&self) -> Vec<String> {
        let state = self.state.read();
        state
            .services
            .iter()
            .filter(|(_, s)| s.current_status == HealthStatus::Degraded)
            .map(|(id, _)| id.clone())
            .collect()
    }

    fn get_unhealthy_services(&self) -> Vec<String> {
        let state = self.state.read();
        state
            .services
            .iter()
            .filter(|(_, s)| s.current_status == HealthStatus::Unhealthy)
            .map(|(id, _)| id.clone())
            .collect()
    }

    fn get_healthy_services(&self) -> Vec<String> {
        let state = self.state.read();
        state
            .services
            .iter()
            .filter(|(_, s)| s.current_status == HealthStatus::Healthy)
            .map(|(id, _)| id.clone())
            .collect()
    }
}

// ============================================================================
// TensorContributor implementation (C3)
// ============================================================================

impl TensorContributor for HealthMonitor {
    #[allow(clippy::cast_precision_loss)]
    fn contribute(&self) -> ContributedTensor {
        let state = self.state.read();
        let total = state.services.len();

        let health_score = if total > 0 {
            let sum: f64 = state.services.values().map(|s| s.current_status.score()).sum();
            sum / total as f64
        } else {
            1.0
        };

        let error_rate = if total > 0 {
            let unhealthy = state
                .services
                .values()
                .filter(|s| s.current_status == HealthStatus::Unhealthy)
                .count();
            unhealthy as f64 / total as f64
        } else {
            0.0
        };

        drop(state);

        let tensor = Tensor12D::new([
            0.0,         // D0
            0.0,         // D1
            0.0,         // D2
            0.0,         // D3
            0.0,         // D4
            0.0,         // D5
            health_score, // D6
            0.0,         // D7
            0.0,         // D8
            0.0,         // D9
            error_rate,  // D10
            0.0,         // D11
        ]);

        let coverage = CoverageBitmap::EMPTY
            .with_dimension(DimensionIndex::HealthScore)
            .with_dimension(DimensionIndex::ErrorRate);

        ContributedTensor::new(tensor, coverage, ContributorKind::Stream)
    }

    fn contributor_kind(&self) -> ContributorKind {
        ContributorKind::Stream
    }

    fn module_id(&self) -> &str {
        ModuleId::M10.as_str()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ---- helpers ----------------------------------------------------------

    fn make_probe(id: &str) -> HealthProbe {
        HealthProbeBuilder::new(id, format!("http://localhost/{id}"))
            .interval_ms(10_000)
            .timeout_ms(2_000)
            .healthy_threshold(2)
            .unhealthy_threshold(2)
            .build()
            .unwrap_or_else(|e| panic!("test probe failed: {e}"))
    }

    fn populated_monitor() -> HealthMonitor {
        let m = HealthMonitor::new();
        let _ = m.register_probe(make_probe("alpha"));
        let _ = m.register_probe(make_probe("beta"));
        let _ = m.register_probe(make_probe("gamma"));
        m
    }

    // ==== [COMPILE] ======================================================

    #[test]
    fn test_health_monitoring_is_object_safe() {
        fn accept_boxed(_m: Box<dyn HealthMonitoring>) {}
        accept_boxed(Box::new(HealthMonitor::new()));
    }

    #[test]
    fn test_health_monitor_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<HealthMonitor>();
    }

    #[test]
    fn test_health_monitoring_arc_dyn() {
        fn accept_arc(_m: Arc<dyn HealthMonitoring>) {}
        let m: Arc<dyn HealthMonitoring> = Arc::new(HealthMonitor::new());
        accept_arc(m);
    }

    // ==== [BASIC] ========================================================

    #[test]
    fn test_new_monitor_empty() {
        let m = HealthMonitor::new();
        assert_eq!(m.probe_count(), 0);
        assert!((m.aggregate_health() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_monitor_default() {
        let m = HealthMonitor::default();
        assert_eq!(m.probe_count(), 0);
    }

    #[test]
    fn test_register_probe() {
        let m = HealthMonitor::new();
        assert!(m.register_probe(make_probe("svc")).is_ok());
        assert_eq!(m.probe_count(), 1);
    }

    #[test]
    fn test_unregister_probe() {
        let m = populated_monitor();
        assert!(m.unregister_probe("beta").is_ok());
        assert_eq!(m.probe_count(), 2);
    }

    #[test]
    fn test_initial_status_is_unknown() {
        let m = populated_monitor();
        assert_eq!(m.get_status("alpha").ok(), Some(HealthStatus::Unknown));
    }

    #[test]
    fn test_health_check_result_success() {
        let r = HealthCheckResult::success("svc", 42);
        assert!(r.is_success());
        assert_eq!(r.status, HealthStatus::Healthy);
        assert_eq!(r.response_time_ms, 42);
    }

    #[test]
    fn test_health_check_result_failure() {
        let r = HealthCheckResult::failure("svc", "timeout");
        assert!(!r.is_success());
        assert_eq!(r.status, HealthStatus::Unhealthy);
    }

    #[test]
    fn test_health_check_result_display() {
        let r = HealthCheckResult::success("synthex", 5);
        let display = r.to_string();
        assert!(display.contains("synthex"));
        assert!(display.contains("healthy"));
    }

    #[test]
    fn test_health_probe_display() {
        let p = make_probe("nais");
        let display = p.to_string();
        assert!(display.contains("nais"));
    }

    // ==== [INVARIANT] FSM transitions ====================================

    #[test]
    fn test_transition_to_healthy_after_threshold() {
        let m = HealthMonitor::new();
        let _ = m.register_probe(make_probe("svc"));
        // Threshold is 2 successes
        let _ = m.record_result("svc", HealthCheckResult::success("svc", 5));
        assert_eq!(m.get_status("svc").ok(), Some(HealthStatus::Unknown));
        let _ = m.record_result("svc", HealthCheckResult::success("svc", 5));
        assert_eq!(m.get_status("svc").ok(), Some(HealthStatus::Healthy));
    }

    #[test]
    fn test_transition_to_unhealthy_after_threshold() {
        let m = HealthMonitor::new();
        let _ = m.register_probe(make_probe("svc"));
        let _ = m.record_result("svc", HealthCheckResult::failure("svc", "err"));
        assert_ne!(m.get_status("svc").ok(), Some(HealthStatus::Unhealthy));
        let _ = m.record_result("svc", HealthCheckResult::failure("svc", "err"));
        assert_eq!(m.get_status("svc").ok(), Some(HealthStatus::Unhealthy));
    }

    #[test]
    fn test_transition_healthy_to_degraded_on_failure() {
        let m = HealthMonitor::new();
        let _ = m.register_probe(make_probe("svc"));
        // First reach healthy
        let _ = m.record_result("svc", HealthCheckResult::success("svc", 5));
        let _ = m.record_result("svc", HealthCheckResult::success("svc", 5));
        assert_eq!(m.get_status("svc").ok(), Some(HealthStatus::Healthy));
        // Single failure -> degraded (not yet unhealthy)
        let _ = m.record_result("svc", HealthCheckResult::failure("svc", "blip"));
        assert_eq!(m.get_status("svc").ok(), Some(HealthStatus::Degraded));
    }

    #[test]
    fn test_recovery_resets_failure_count() {
        let m = HealthMonitor::new();
        let _ = m.register_probe(make_probe("svc"));
        let _ = m.record_result("svc", HealthCheckResult::failure("svc", "err"));
        // One success resets failure counter
        let _ = m.record_result("svc", HealthCheckResult::success("svc", 5));
        // This second failure is only #1, not #2 — should not trip unhealthy
        let _ = m.record_result("svc", HealthCheckResult::failure("svc", "err"));
        assert_ne!(m.get_status("svc").ok(), Some(HealthStatus::Unhealthy));
    }

    #[test]
    fn test_consecutive_counter_reset_on_opposite() {
        let m = HealthMonitor::new();
        let _ = m.register_probe(make_probe("svc"));
        // 1 success, 1 failure, 1 success — never hits threshold
        let _ = m.record_result("svc", HealthCheckResult::success("svc", 5));
        let _ = m.record_result("svc", HealthCheckResult::failure("svc", "x"));
        let _ = m.record_result("svc", HealthCheckResult::success("svc", 5));
        assert_eq!(m.get_status("svc").ok(), Some(HealthStatus::Unknown));
    }

    // ==== [BOUNDARY] =====================================================

    #[test]
    fn test_aggregate_health_empty() {
        let m = HealthMonitor::new();
        assert!((m.aggregate_health() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_aggregate_health_all_healthy() {
        let m = populated_monitor();
        for svc in ["alpha", "beta", "gamma"] {
            let _ = m.record_result(svc, HealthCheckResult::success(svc, 5));
            let _ = m.record_result(svc, HealthCheckResult::success(svc, 5));
        }
        assert!((m.aggregate_health() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_aggregate_health_mixed() {
        let m = HealthMonitor::new();
        let _ = m.register_probe(make_probe("a"));
        let _ = m.register_probe(make_probe("b"));
        // a -> healthy (score 1.0)
        let _ = m.record_result("a", HealthCheckResult::success("a", 5));
        let _ = m.record_result("a", HealthCheckResult::success("a", 5));
        // b -> unhealthy (score 0.0)
        let _ = m.record_result("b", HealthCheckResult::failure("b", "err"));
        let _ = m.record_result("b", HealthCheckResult::failure("b", "err"));
        assert!((m.aggregate_health() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_history_trimming() {
        let m = HealthMonitor::with_max_history(3);
        let _ = m.register_probe(make_probe("svc"));
        for _ in 0..10 {
            let _ = m.record_result("svc", HealthCheckResult::success("svc", 5));
        }
        let history = m.get_history("svc").unwrap_or_default();
        assert_eq!(history.len(), 3);
    }

    #[test]
    fn test_get_history_empty() {
        let m = HealthMonitor::new();
        let _ = m.register_probe(make_probe("svc"));
        let history = m.get_history("svc").unwrap_or_default();
        assert!(history.is_empty());
    }

    // ==== [PROPERTY] =====================================================

    #[test]
    fn test_aggregate_health_always_in_unit_interval() {
        let m = populated_monitor();
        // Various states
        let _ = m.record_result("alpha", HealthCheckResult::success("alpha", 5));
        let _ = m.record_result("beta", HealthCheckResult::failure("beta", "x"));
        let health = m.aggregate_health();
        assert!((0.0..=1.0).contains(&health));
    }

    #[test]
    fn test_status_lists_partition_all_services() {
        let m = populated_monitor();
        let _ = m.record_result("alpha", HealthCheckResult::success("alpha", 5));
        let _ = m.record_result("alpha", HealthCheckResult::success("alpha", 5));
        let _ = m.record_result("beta", HealthCheckResult::failure("beta", "x"));
        let _ = m.record_result("beta", HealthCheckResult::failure("beta", "x"));

        let healthy = m.get_healthy_services().len();
        let unhealthy = m.get_unhealthy_services().len();
        let degraded = m.get_degraded_services().len();
        let total = m.probe_count();
        // healthy + unhealthy + degraded + unknown = total
        assert!(healthy + unhealthy + degraded <= total);
    }

    #[test]
    fn test_get_all_statuses_length() {
        let m = populated_monitor();
        assert_eq!(m.get_all_statuses().len(), 3);
    }

    // ==== [NEGATIVE] =====================================================

    #[test]
    fn test_duplicate_probe_registration_fails() {
        let m = HealthMonitor::new();
        assert!(m.register_probe(make_probe("svc")).is_ok());
        assert!(m.register_probe(make_probe("svc")).is_err());
    }

    #[test]
    fn test_unregister_unknown_probe_fails() {
        let m = HealthMonitor::new();
        assert!(m.unregister_probe("ghost").is_err());
    }

    #[test]
    fn test_record_result_unknown_service_fails() {
        let m = HealthMonitor::new();
        assert!(m.record_result("ghost", HealthCheckResult::success("ghost", 5)).is_err());
    }

    #[test]
    fn test_get_status_unknown_fails() {
        let m = HealthMonitor::new();
        assert!(m.get_status("ghost").is_err());
    }

    #[test]
    fn test_get_history_unknown_fails() {
        let m = HealthMonitor::new();
        assert!(m.get_history("ghost").is_err());
    }

    // ==== Builder validation ==============================================

    #[test]
    fn test_builder_empty_service_id_fails() {
        let result = HealthProbeBuilder::new("", "http://x").build();
        assert!(result.is_err());
    }

    #[test]
    fn test_builder_empty_endpoint_fails() {
        let result = HealthProbeBuilder::new("svc", "").build();
        assert!(result.is_err());
    }

    #[test]
    fn test_builder_zero_interval_fails() {
        let result = HealthProbeBuilder::new("svc", "http://x")
            .interval_ms(0)
            .build();
        assert!(result.is_err());
    }

    #[test]
    fn test_builder_zero_timeout_fails() {
        let result = HealthProbeBuilder::new("svc", "http://x")
            .timeout_ms(0)
            .build();
        assert!(result.is_err());
    }

    #[test]
    fn test_builder_timeout_exceeds_interval_fails() {
        let result = HealthProbeBuilder::new("svc", "http://x")
            .interval_ms(1000)
            .timeout_ms(2000)
            .build();
        assert!(result.is_err());
    }

    #[test]
    fn test_builder_zero_healthy_threshold_fails() {
        let result = HealthProbeBuilder::new("svc", "http://x")
            .healthy_threshold(0)
            .build();
        assert!(result.is_err());
    }

    #[test]
    fn test_builder_zero_unhealthy_threshold_fails() {
        let result = HealthProbeBuilder::new("svc", "http://x")
            .unhealthy_threshold(0)
            .build();
        assert!(result.is_err());
    }

    // ==== [INTEGRATION] signal emission ==================================

    #[test]
    fn test_signal_bus_none_does_not_panic() {
        let m = HealthMonitor::new();
        let _ = m.register_probe(make_probe("svc"));
        let _ = m.record_result("svc", HealthCheckResult::success("svc", 5));
        let _ = m.record_result("svc", HealthCheckResult::success("svc", 5));
        // Should not panic even without a signal bus
    }

    #[test]
    fn test_health_transition_emits_signal() {
        let bus = Arc::new(SignalBus::new());
        let m = HealthMonitor::new().with_signal_bus(Arc::clone(&bus));
        let _ = m.register_probe(make_probe("svc"));

        // Unknown -> Healthy (2 successes with threshold 2)
        let _ = m.record_result("svc", HealthCheckResult::success("svc", 5));
        assert_eq!(bus.stats().health_emitted, 0); // Not yet threshold
        let _ = m.record_result("svc", HealthCheckResult::success("svc", 5));
        assert_eq!(bus.stats().health_emitted, 1); // Unknown -> Healthy

        // Healthy -> Degraded
        let _ = m.record_result("svc", HealthCheckResult::failure("svc", "blip"));
        assert_eq!(bus.stats().health_emitted, 2); // Healthy -> Degraded
    }

    // ==== [TENSOR] =======================================================

    #[test]
    fn test_tensor_contributor_module_id() {
        let m = HealthMonitor::new();
        assert_eq!(m.module_id(), "M10");
    }

    #[test]
    fn test_tensor_contributor_kind() {
        let m = HealthMonitor::new();
        assert_eq!(m.contributor_kind(), ContributorKind::Stream);
    }

    #[test]
    fn test_tensor_empty_monitor() {
        let m = HealthMonitor::new();
        let ct = m.contribute();
        // D6 = 1.0 (no probes = healthy default)
        assert!((ct.tensor.to_array()[6] - 1.0).abs() < f64::EPSILON);
        // D10 = 0.0 (no errors)
        assert!(ct.tensor.to_array()[10].abs() < f64::EPSILON);
    }

    #[test]
    fn test_tensor_populated_monitor() {
        let m = populated_monitor();
        for svc in ["alpha", "beta", "gamma"] {
            let _ = m.record_result(svc, HealthCheckResult::success(svc, 5));
            let _ = m.record_result(svc, HealthCheckResult::success(svc, 5));
        }
        let ct = m.contribute();
        assert!((ct.tensor.to_array()[6] - 1.0).abs() < f64::EPSILON);
        assert!(ct.tensor.to_array()[10].abs() < f64::EPSILON);
    }

    #[test]
    fn test_tensor_with_unhealthy() {
        let m = HealthMonitor::new();
        let _ = m.register_probe(make_probe("a"));
        let _ = m.register_probe(make_probe("b"));
        // a healthy, b unhealthy
        let _ = m.record_result("a", HealthCheckResult::success("a", 5));
        let _ = m.record_result("a", HealthCheckResult::success("a", 5));
        let _ = m.record_result("b", HealthCheckResult::failure("b", "x"));
        let _ = m.record_result("b", HealthCheckResult::failure("b", "x"));
        let ct = m.contribute();
        // D6 = 0.5 (1 healthy, 1 unhealthy)
        assert!((ct.tensor.to_array()[6] - 0.5).abs() < f64::EPSILON);
        // D10 = 0.5 (1 of 2 unhealthy)
        assert!((ct.tensor.to_array()[10] - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_tensor_coverage() {
        let m = HealthMonitor::new();
        let ct = m.contribute();
        assert!(ct.coverage.is_covered(DimensionIndex::HealthScore));
        assert!(ct.coverage.is_covered(DimensionIndex::ErrorRate));
        assert!(!ct.coverage.is_covered(DimensionIndex::ServiceId));
        assert_eq!(ct.coverage.count(), 2);
    }

    #[test]
    fn test_tensor_all_dims_in_unit_interval() {
        let m = populated_monitor();
        let _ = m.record_result("alpha", HealthCheckResult::failure("alpha", "x"));
        let ct = m.contribute();
        for val in ct.tensor.to_array() {
            assert!(
                (0.0..=1.0).contains(&val),
                "Tensor value out of range: {val}"
            );
        }
    }

    // ==== [INTEGRATION] trait via Arc<dyn> ================================

    #[test]
    fn test_trait_via_arc_dyn() {
        let monitor: Arc<dyn HealthMonitoring> = Arc::new(HealthMonitor::new());
        assert!(monitor.register_probe(make_probe("svc")).is_ok());
        assert_eq!(monitor.probe_count(), 1);
    }

    #[test]
    fn test_l2_uses_timestamp_not_system_time() {
        let r = HealthCheckResult::success("svc", 5);
        let _ticks: u64 = r.timestamp.ticks();
    }

    #[test]
    fn test_l2_uses_l1_error_type() {
        let m = HealthMonitor::new();
        let err = m.get_status("ghost").unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("ghost") || msg.contains("not found"));
    }
}
