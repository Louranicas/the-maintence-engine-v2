//! # Layer 2: Services
//!
//! Service registry, health monitoring, lifecycle management, and resilience
//! patterns for the ULTRAPLATE Developer Environment.
//!
//! ## Modules
//!
//! | Module | ID | Purpose |
//! |--------|----|---------|
//! | [`service_registry`] | M09 | Service discovery and registration |
//! | [`health_monitor`] | M10 | Health check orchestration |
//! | [`lifecycle`] | M11 | Service lifecycle management |
//! | [`resilience`] | M12 | Circuit breaker + load balancer |
//!
//! ## Traits (Dependency Inversion)
//!
//! | Trait | Module | Methods |
//! |-------|--------|---------|
//! | [`ServiceDiscovery`] | M09 | register, discover, health, deps |
//! | [`HealthMonitoring`] | M10 | probes, results, status, aggregation |
//! | [`LifecycleOps`] | M11 | start, stop, restart, transitions |
//! | [`CircuitBreakerOps`] | M12 | record, allow, state, reset |
//! | [`LoadBalancing`] | M12 | pool, endpoint, select, stats |
//!
//! ## 12D Tensor Contribution
//!
//! | Module | Dimensions |
//! |--------|-----------|
//! | M09 | D0 (service count), D2 (avg tier), D3 (avg deps), D4 (healthy count) |
//! | M10 | D6 (aggregate health), D10 (error rate) |
//! | M11 | D6 (% running), D7 (uptime proxy) |
//! | M12 | D9 (latency proxy), D10 (circuit failure rate) |
//!
//! ## Design Constraints (C1–C10)
//!
//! - **C1** No upward imports (L2 depends only on L1)
//! - **C2** All trait methods `&self` (interior mutability via `parking_lot::RwLock`)
//! - **C3** Every module implements [`TensorContributor`]
//! - **C4** Zero tolerance: no `unsafe`, no `unwrap`, no `expect`, 0 clippy pedantic
//! - **C5** No `chrono`, no `SystemTime` — [`Timestamp`] + [`Duration`] only
//! - **C6** Signal emissions use `Arc<SignalBus>`
//! - **C7** Methods returning data through `RwLock` return OWNED types
//! - **C8** Timeouts use `std::time::Duration`
//! - **C9** Existing downstream tests MUST NOT break
//! - **C10** Test target: 280+
//!
//! ## Related Documentation
//! - [Layer Specification](../../ai_docs/layers/L02_SERVICES.md)
//! - [Service Registry](../../service_registry/SERVICE_REGISTRY.md)

pub mod health_monitor;
pub mod lifecycle;
pub mod resilience;
pub mod service_registry;
pub mod traffic;

use std::fmt;

use crate::m1_foundation::{ModuleId, Timestamp};
use crate::Tensor12D;

// ============================================================================
// Re-exports — preserve downstream compatibility (C9)
// ============================================================================

// Engine.rs imports: CircuitBreakerRegistry, HealthMonitor, LifecycleManager
// lib.rs prelude: ServiceState
// tests/common: ServiceState, ServiceTier

pub use health_monitor::{
    HealthCheckResult, HealthMonitor, HealthMonitoring, HealthProbe, HealthProbeBuilder,
};
pub use lifecycle::{
    LifecycleAction, LifecycleEntry, LifecycleEntryBuilder, LifecycleManager, LifecycleOps,
    LifecycleTransition,
};
pub use resilience::{
    CircuitBreakerConfig, CircuitBreakerConfigBuilder, CircuitBreakerOps, CircuitBreakerRegistry,
    CircuitBreakerStats, CircuitStateTransition, Endpoint, LoadBalanceAlgorithm, LoadBalancer,
    LoadBalancing, PoolStats, ResilienceManager,
};
pub use service_registry::{
    ServiceDefinition, ServiceDefinitionBuilder, ServiceDiscovery, ServiceRegistry,
};
pub use traffic::{
    RequestObservation, TrafficConfig, TrafficConfigBuilder, TrafficManager, TrafficShaping,
    TrafficSnapshot,
};

// ============================================================================
// ServiceStatus
// ============================================================================

/// Service status enumeration.
///
/// Represents the current lifecycle state of a service in the
/// ULTRAPLATE Developer Environment.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum ServiceStatus {
    /// Service is stopped.
    #[default]
    Stopped,
    /// Service is starting.
    Starting,
    /// Service is running.
    Running,
    /// Service is stopping.
    Stopping,
    /// Service has failed.
    Failed,
}

impl ServiceStatus {
    /// Convert to string representation.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Stopped => "stopped",
            Self::Starting => "starting",
            Self::Running => "running",
            Self::Stopping => "stopping",
            Self::Failed => "failed",
        }
    }

    /// Whether this status represents an operational state.
    #[must_use]
    pub const fn is_operational(&self) -> bool {
        matches!(self, Self::Running)
    }
}

impl fmt::Display for ServiceStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ============================================================================
// HealthStatus
// ============================================================================

/// Health status enumeration.
///
/// Represents the observed health state of a service.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum HealthStatus {
    /// Service is healthy.
    #[default]
    Healthy,
    /// Service is degraded but operational.
    Degraded,
    /// Service is unhealthy.
    Unhealthy,
    /// Health status unknown.
    Unknown,
}

impl HealthStatus {
    /// Convert to string representation.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Healthy => "healthy",
            Self::Degraded => "degraded",
            Self::Unhealthy => "unhealthy",
            Self::Unknown => "unknown",
        }
    }

    /// Numeric score for tensor encoding: Healthy=1.0, Degraded=0.5, else 0.0.
    #[must_use]
    pub const fn score(&self) -> f64 {
        match self {
            Self::Healthy => 1.0,
            Self::Degraded => 0.5,
            Self::Unhealthy | Self::Unknown => 0.0,
        }
    }
}

impl fmt::Display for HealthStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ============================================================================
// ServiceTier
// ============================================================================

/// Service tier for priority weighting.
///
/// ULTRAPLATE services are ranked into 5 tiers. Higher tiers (lower numbers)
/// receive greater weight in health aggregation and scheduling priority.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ServiceTier {
    /// Tier 1: Core services (SYNTHEX, SAN-K7).
    Tier1,
    /// Tier 2: Intelligence services (NAIS, `CodeSynthor`).
    Tier2,
    /// Tier 3: Integration services (Tool Library, CCM).
    Tier3,
    /// Tier 4: Infrastructure (Prometheus, Architect).
    Tier4,
    /// Tier 5: Execution (Bash Engine, Tool Maker).
    Tier5,
}

impl ServiceTier {
    /// Weight multiplier for this tier.
    #[must_use]
    pub const fn weight(&self) -> f64 {
        match self {
            Self::Tier1 => 1.5,
            Self::Tier2 => 1.3,
            Self::Tier3 => 1.2,
            Self::Tier4 => 1.1,
            Self::Tier5 => 1.0,
        }
    }

    /// Tier number (1–5).
    #[must_use]
    pub const fn number(&self) -> u8 {
        match self {
            Self::Tier1 => 1,
            Self::Tier2 => 2,
            Self::Tier3 => 3,
            Self::Tier4 => 4,
            Self::Tier5 => 5,
        }
    }

    /// Normalized tier value for tensor encoding (tier / 6.0).
    #[must_use]
    pub const fn normalized(&self) -> f64 {
        self.number() as f64 / 6.0
    }
}

impl fmt::Display for ServiceTier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Tier{}", self.number())
    }
}

// ============================================================================
// CircuitState
// ============================================================================

/// Circuit breaker state.
///
/// Implements the standard circuit breaker pattern:
/// Closed (normal) → Open (rejecting) → `HalfOpen` (probing).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum CircuitState {
    /// Normal operation — requests flow through.
    #[default]
    Closed,
    /// Failures exceeded threshold — requests are rejected.
    Open,
    /// Testing if service recovered — limited probing allowed.
    HalfOpen,
}

impl CircuitState {
    /// Convert to string representation.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Closed => "closed",
            Self::Open => "open",
            Self::HalfOpen => "half_open",
        }
    }
}

impl fmt::Display for CircuitState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ============================================================================
// ServiceState — preserved for downstream compatibility (C9)
// ============================================================================

/// Service state representation.
///
/// Comprehensive snapshot of a service's operational state including
/// lifecycle status, health, performance metrics, and 12D tensor encoding.
#[derive(Clone, Debug)]
pub struct ServiceState {
    /// Service identifier.
    pub id: String,
    /// Service name.
    pub name: String,
    /// Current status.
    pub status: ServiceStatus,
    /// Health status.
    pub health_status: HealthStatus,
    /// Service tier.
    pub tier: ServiceTier,
    /// Port number.
    pub port: u16,
    /// Process ID (if running).
    pub pid: Option<u32>,
    /// Health score (0.0–1.0).
    pub health_score: f64,
    /// Synergy score with other services.
    pub synergy_score: f64,
    /// CPU usage percentage.
    pub cpu_percent: f64,
    /// Memory usage in MB.
    pub memory_mb: f64,
    /// Uptime in seconds.
    pub uptime_seconds: u64,
    /// Restart count.
    pub restart_count: u32,
    /// Last health check timestamp (C5: `Timestamp`, not `SystemTime`).
    pub last_health_check: Option<Timestamp>,
    /// Optional module identity for ME-internal modules.
    pub module_id: Option<ModuleId>,
    /// 12D tensor encoding.
    pub tensor: Tensor12D,
}

impl Default for ServiceState {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            status: ServiceStatus::default(),
            health_status: HealthStatus::default(),
            tier: ServiceTier::Tier5,
            port: 0,
            pid: None,
            health_score: 0.0,
            synergy_score: 0.0,
            cpu_percent: 0.0,
            memory_mb: 0.0,
            uptime_seconds: 0,
            restart_count: 0,
            last_health_check: None,
            module_id: None,
            tensor: Tensor12D::default(),
        }
    }
}

impl ServiceState {
    /// Create a new service state.
    #[must_use]
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        tier: ServiceTier,
        port: u16,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            tier,
            port,
            ..Default::default()
        }
    }

    /// Calculate weighted health score.
    #[must_use]
    pub fn weighted_health(&self) -> f64 {
        self.health_score * self.tier.weight()
    }

    /// Update tensor encoding from current state.
    #[allow(clippy::cast_precision_loss)]
    pub fn update_tensor(&mut self) {
        self.tensor = Tensor12D::new([
            hash_to_float(&self.id),
            f64::from(self.port) / 65535.0,
            f64::from(self.tier.number()) / 6.0,
            0.0, // dependency count — to be calculated
            0.0, // agent count — to be calculated
            0.5, // protocol encoding
            self.health_score,
            (self.uptime_seconds as f64 / (86400.0 * 30.0)).min(1.0),
            self.synergy_score,
            1.0 - (self.cpu_percent / 100.0).min(1.0), // latency proxy
            0.0, // error rate — to be calculated
            0.5, // temporal context
        ]);
    }

    /// Check if service is operational.
    #[must_use]
    pub fn is_operational(&self) -> bool {
        self.status == ServiceStatus::Running && self.health_status != HealthStatus::Unhealthy
    }
}

// ============================================================================
// ServicesStatus — Layer-level health aggregate
// ============================================================================

/// Aggregate status of the L2 Services layer.
///
/// Provides a single-point summary of all L2 subsystem health,
/// mirroring L1's `FoundationStatus`.
#[derive(Debug, Clone, PartialEq)]
pub struct ServicesStatus {
    /// Layer identifier (always "L2").
    pub layer_id: &'static str,
    /// Number of services modules (M09–M12).
    pub module_count: u8,
    /// Registered service count.
    pub registered_services: usize,
    /// Healthy service count.
    pub healthy_services: usize,
    /// Running service count (from lifecycle).
    pub running_services: usize,
    /// Open circuit breaker count.
    pub open_circuits: usize,
    /// Overall layer health score (0.0–1.0).
    pub health_score: f64,
    /// Composed 12D tensor representing layer state.
    pub tensor: Tensor12D,
}

impl Default for ServicesStatus {
    fn default() -> Self {
        Self {
            layer_id: "L2",
            module_count: 4,
            registered_services: 0,
            healthy_services: 0,
            running_services: 0,
            open_circuits: 0,
            health_score: 1.0,
            tensor: Tensor12D::default(),
        }
    }
}

impl fmt::Display for ServicesStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}: {}/{} healthy, {}/{} running, {} open circuits, health={:.2}",
            self.layer_id,
            self.healthy_services,
            self.registered_services,
            self.running_services,
            self.registered_services,
            self.open_circuits,
            self.health_score,
        )
    }
}

// ============================================================================
// RestartConfig — extracted for LifecycleOps trait
// ============================================================================

/// Configuration for service restart behaviour.
///
/// Controls the maximum number of restarts and the initial backoff
/// duration before exponential growth.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RestartConfig {
    /// Maximum allowed restarts before permanent failure.
    pub max_restarts: u32,
    /// Initial backoff duration between restarts.
    pub initial_backoff: std::time::Duration,
    /// Maximum backoff duration (cap for exponential growth).
    pub max_backoff: std::time::Duration,
}

impl Default for RestartConfig {
    fn default() -> Self {
        Self {
            max_restarts: 5,
            initial_backoff: std::time::Duration::from_secs(1),
            max_backoff: std::time::Duration::from_secs(30),
        }
    }
}

impl RestartConfig {
    /// Create a restart config with custom parameters.
    #[must_use]
    pub const fn new(
        max_restarts: u32,
        initial_backoff: std::time::Duration,
        max_backoff: std::time::Duration,
    ) -> Self {
        Self {
            max_restarts,
            initial_backoff,
            max_backoff,
        }
    }
}

// ============================================================================
// Utility
// ============================================================================

/// Hash a string to a float in [0, 1].
#[allow(clippy::cast_precision_loss)]
fn hash_to_float(s: &str) -> f64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    s.hash(&mut hasher);
    let hash = hasher.finish();
    (hash as f64) / (u64::MAX as f64)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ---- ServiceStatus ----

    #[test]
    fn test_service_status_display() {
        assert_eq!(ServiceStatus::Running.to_string(), "running");
        assert_eq!(ServiceStatus::Failed.to_string(), "failed");
        assert_eq!(ServiceStatus::Stopped.as_str(), "stopped");
    }

    #[test]
    fn test_service_status_default() {
        assert_eq!(ServiceStatus::default(), ServiceStatus::Stopped);
    }

    #[test]
    fn test_service_status_operational() {
        assert!(ServiceStatus::Running.is_operational());
        assert!(!ServiceStatus::Stopped.is_operational());
        assert!(!ServiceStatus::Failed.is_operational());
    }

    // ---- HealthStatus ----

    #[test]
    fn test_health_status_display() {
        assert_eq!(HealthStatus::Healthy.to_string(), "healthy");
        assert_eq!(HealthStatus::Degraded.to_string(), "degraded");
    }

    #[test]
    fn test_health_status_score() {
        assert!((HealthStatus::Healthy.score() - 1.0).abs() < f64::EPSILON);
        assert!((HealthStatus::Degraded.score() - 0.5).abs() < f64::EPSILON);
        assert!((HealthStatus::Unhealthy.score() - 0.0).abs() < f64::EPSILON);
        assert!((HealthStatus::Unknown.score() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_health_status_default() {
        assert_eq!(HealthStatus::default(), HealthStatus::Healthy);
    }

    // ---- ServiceTier ----

    #[test]
    fn test_service_tier_weights() {
        assert!((ServiceTier::Tier1.weight() - 1.5).abs() < f64::EPSILON);
        assert!((ServiceTier::Tier5.weight() - 1.0).abs() < f64::EPSILON);
        assert_eq!(ServiceTier::Tier3.number(), 3);
    }

    #[test]
    fn test_service_tier_display() {
        assert_eq!(ServiceTier::Tier1.to_string(), "Tier1");
        assert_eq!(ServiceTier::Tier5.to_string(), "Tier5");
    }

    #[test]
    fn test_service_tier_normalized() {
        assert!((ServiceTier::Tier3.normalized() - 0.5).abs() < f64::EPSILON);
    }

    // ---- CircuitState ----

    #[test]
    fn test_circuit_state_display() {
        assert_eq!(CircuitState::Closed.to_string(), "closed");
        assert_eq!(CircuitState::Open.to_string(), "open");
        assert_eq!(CircuitState::HalfOpen.to_string(), "half_open");
    }

    #[test]
    fn test_circuit_state_default() {
        assert_eq!(CircuitState::default(), CircuitState::Closed);
    }

    // ---- ServiceState ----

    #[test]
    fn test_service_state_construction() {
        let state = ServiceState::new("synthex", "SYNTHEX Engine", ServiceTier::Tier1, 8090);
        assert_eq!(state.id, "synthex");
        assert_eq!(state.name, "SYNTHEX Engine");
        assert_eq!(state.tier, ServiceTier::Tier1);
        assert_eq!(state.port, 8090);
        assert_eq!(state.status, ServiceStatus::Stopped);
        assert!(state.module_id.is_none());
    }

    #[test]
    fn test_service_state_weighted_health() {
        let mut state = ServiceState::new("synthex", "SYNTHEX", ServiceTier::Tier1, 8090);
        state.health_score = 0.95;
        assert!((state.weighted_health() - (0.95 * 1.5)).abs() < f64::EPSILON);
    }

    #[test]
    fn test_service_state_operational() {
        let mut state = ServiceState::new("svc", "Svc", ServiceTier::Tier5, 8000);
        state.status = ServiceStatus::Running;
        state.health_status = HealthStatus::Healthy;
        assert!(state.is_operational());

        state.health_status = HealthStatus::Unhealthy;
        assert!(!state.is_operational());
    }

    #[test]
    fn test_service_state_tensor_update() {
        let mut state = ServiceState::new("synthex", "SYNTHEX", ServiceTier::Tier1, 8090);
        state.health_score = 0.95;
        state.synergy_score = 0.98;
        state.status = ServiceStatus::Running;
        state.update_tensor();
        assert!(state.tensor.validate().is_ok());
    }

    #[test]
    fn test_service_state_default() {
        let state = ServiceState::default();
        assert!(state.id.is_empty());
        assert_eq!(state.status, ServiceStatus::Stopped);
        assert!(state.last_health_check.is_none());
    }

    // ---- ServicesStatus ----

    #[test]
    fn test_services_status_default() {
        let status = ServicesStatus::default();
        assert_eq!(status.layer_id, "L2");
        assert_eq!(status.module_count, 4);
        assert!((status.health_score - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_services_status_display() {
        let status = ServicesStatus {
            registered_services: 12,
            healthy_services: 10,
            running_services: 11,
            open_circuits: 1,
            health_score: 0.85,
            ..ServicesStatus::default()
        };
        let s = status.to_string();
        assert!(s.contains("10/12 healthy"));
        assert!(s.contains("11/12 running"));
        assert!(s.contains("1 open circuits"));
    }

    // ---- RestartConfig ----

    #[test]
    fn test_restart_config_default() {
        let config = RestartConfig::default();
        assert_eq!(config.max_restarts, 5);
        assert_eq!(config.initial_backoff, std::time::Duration::from_secs(1));
        assert_eq!(config.max_backoff, std::time::Duration::from_secs(30));
    }

    // ---- hash_to_float ----

    #[test]
    fn test_hash_to_float_range() {
        for s in &["synthex", "san-k7", "nais", "", "a very long string"] {
            let v = hash_to_float(s);
            assert!((0.0..=1.0).contains(&v), "hash_to_float({s:?}) = {v}");
        }
    }
}
