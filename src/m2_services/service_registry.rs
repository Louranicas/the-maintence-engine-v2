//! # M09: Service Registry
//!
//! Trait-backed service discovery and registration for the ULTRAPLATE Developer
//! Environment. Replaces the former `discovery.rs` (M10) with a principled
//! design using interior mutability (`parking_lot::RwLock`) and typed vocabulary
//! types from L1 Foundation.
//!
//! ## Layer: L2 (Services)
//! ## Module: M09
//! ## Dependencies: L1 (Error, Timestamp, `ModuleId`, `SignalBus`, `TensorContributor`)
//!
//! ## Trait: [`ServiceDiscovery`]
//!
//! All methods are `&self` (C2) with interior mutability via `RwLock`.
//! Methods returning data through `RwLock` return OWNED types (C7).
//!
//! ## 12D Tensor Contribution (C3)
//!
//! | Dimension | Value |
//! |-----------|-------|
//! | D0 (`service_id`) | `service_count` / 12.0 |
//! | D2 (tier) | average tier normalized |
//! | D3 (`dependency_count`) | average dependency count / 12.0 |
//! | D4 (`agent_count`) | `healthy_count` / `total_count` |
//!
//! ## Signal Emission (C6)
//!
//! `update_health()` emits [`HealthSignal`] on status transitions.
//!
//! ## Related Documentation
//! - [Layer Specification](../../ai_docs/layers/L02_SERVICES.md)
//! - [Service Registry](../../service_registry/SERVICE_REGISTRY.md)

use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use parking_lot::RwLock;

use super::{HealthStatus, ServiceTier};
use crate::m1_foundation::shared_types::{CoverageBitmap, DimensionIndex, ModuleId, Timestamp};
use crate::m1_foundation::signals::{HealthSignal, SignalBus};
use crate::m1_foundation::tensor_registry::{ContributedTensor, ContributorKind, TensorContributor};
use crate::m1_foundation::MetricsRegistry;
use crate::{Error, Result, Tensor12D};

// ============================================================================
// ServiceDiscovery (trait)
// ============================================================================

/// Trait for service registry operations.
///
/// All methods are `&self` (C2 constraint) with interior mutability via
/// `parking_lot::RwLock`. Methods returning data through `RwLock` return
/// owned types, not references (C7 constraint).
pub trait ServiceDiscovery: Send + Sync + fmt::Debug {
    /// Register a new service.
    ///
    /// # Errors
    /// Returns [`Error::Validation`] if a service with the same ID is already registered.
    fn register(&self, def: ServiceDefinition) -> Result<()>;

    /// Remove a registered service and clean up its dependency edges.
    ///
    /// # Errors
    /// Returns [`Error::ServiceNotFound`] if the service does not exist.
    fn deregister(&self, service_id: &str) -> Result<()>;

    /// Look up a service by ID, returning an owned clone (C7).
    ///
    /// # Errors
    /// Returns [`Error::ServiceNotFound`] if the service does not exist.
    fn discover(&self, service_id: &str) -> Result<ServiceDefinition>;

    /// Return all services in the given tier.
    fn discover_by_tier(&self, tier: ServiceTier) -> Vec<ServiceDefinition>;

    /// Return all services advertising the given protocol (case-insensitive).
    fn discover_by_protocol(&self, protocol: &str) -> Vec<ServiceDefinition>;

    /// Return all registered services.
    fn list_services(&self) -> Vec<ServiceDefinition>;

    /// Update the health status for a registered service.
    ///
    /// # Errors
    /// Returns [`Error::ServiceNotFound`] if the service does not exist.
    fn update_health(&self, service_id: &str, status: HealthStatus) -> Result<()>;

    /// Get the current health status for a service.
    ///
    /// # Errors
    /// Returns [`Error::ServiceNotFound`] if the service does not exist.
    fn get_health(&self, service_id: &str) -> Result<HealthStatus>;

    /// Return all services whose health is [`HealthStatus::Healthy`].
    fn get_healthy_services(&self) -> Vec<ServiceDefinition>;

    /// Record that service `from` depends on service `to`.
    ///
    /// # Errors
    /// Returns [`Error::ServiceNotFound`] if either service does not exist.
    /// Returns [`Error::Validation`] if `from == to`.
    fn add_dependency(&self, from: &str, to: &str) -> Result<()>;

    /// Get the IDs of services that `service_id` depends on.
    ///
    /// # Errors
    /// Returns [`Error::ServiceNotFound`] if the service does not exist.
    fn get_dependencies(&self, service_id: &str) -> Result<Vec<String>>;

    /// Get the IDs of services that depend on `service_id`.
    ///
    /// # Errors
    /// Returns [`Error::ServiceNotFound`] if the service does not exist.
    fn get_dependents(&self, service_id: &str) -> Result<Vec<String>>;

    /// Return the number of registered services.
    fn service_count(&self) -> usize;

    /// Check whether a service is registered.
    fn is_registered(&self, service_id: &str) -> bool;
}

// ============================================================================
// ServiceDefinition
// ============================================================================

/// A registered service in the ULTRAPLATE ecosystem.
///
/// Each service is uniquely identified by its `service_id` and carries
/// metadata such as host, port, protocol, tier, and an optional TTL.
///
/// # Examples
///
/// ```
/// use maintenance_engine::m2_services::service_registry::ServiceDefinition;
/// use maintenance_engine::m2_services::ServiceTier;
///
/// let def = ServiceDefinition::builder("synthex", "SYNTHEX Engine", "1.0.0")
///     .tier(ServiceTier::Tier1)
///     .host("localhost")
///     .port(8090)
///     .protocol("REST")
///     .health_path("/api/health")
///     .build();
/// assert_eq!(def.service_id, "synthex");
/// ```
#[derive(Clone, Debug)]
pub struct ServiceDefinition {
    /// Unique service identifier (e.g. `"synthex"`, `"san-k7"`).
    pub service_id: String,
    /// Human-readable service name.
    pub name: String,
    /// Semantic version string.
    pub version: String,
    /// Service tier for priority weighting.
    pub tier: ServiceTier,
    /// Host address where the service listens.
    pub host: String,
    /// TCP port number.
    pub port: u16,
    /// Wire protocol identifier (`"REST"`, `"gRPC"`, `"WS"`, `"IPC"`).
    pub protocol: String,
    /// HTTP path used for health checks.
    pub health_path: String,
    /// Arbitrary key-value metadata.
    pub metadata: HashMap<String, String>,
    /// Timestamp when the service was registered (C5: Timestamp, not `SystemTime`).
    pub registered_at: Timestamp,
    /// Optional time-to-live in seconds.
    pub ttl_seconds: Option<u64>,
    /// Optional module identity for ME-internal modules.
    pub module_id: Option<ModuleId>,
}

impl fmt::Display for ServiceDefinition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Service({} [{}] {}:{} {})",
            self.service_id, self.tier, self.host, self.port, self.protocol
        )
    }
}

// ============================================================================
// ServiceDefinitionBuilder
// ============================================================================

/// Builder for [`ServiceDefinition`] with sensible defaults.
///
/// # Examples
///
/// ```
/// use maintenance_engine::m2_services::service_registry::ServiceDefinitionBuilder;
/// use maintenance_engine::m2_services::ServiceTier;
///
/// let def = ServiceDefinitionBuilder::new("nais", "NAIS", "1.0.0")
///     .tier(ServiceTier::Tier2)
///     .port(8101)
///     .build();
/// assert_eq!(def.port, 8101);
/// ```
pub struct ServiceDefinitionBuilder {
    service_id: String,
    name: String,
    version: String,
    tier: ServiceTier,
    host: String,
    port: u16,
    protocol: String,
    health_path: String,
    metadata: HashMap<String, String>,
    ttl_seconds: Option<u64>,
    module_id: Option<ModuleId>,
}

impl ServiceDefinitionBuilder {
    /// Create a new builder with required fields.
    #[must_use]
    pub fn new(
        service_id: impl Into<String>,
        name: impl Into<String>,
        version: impl Into<String>,
    ) -> Self {
        Self {
            service_id: service_id.into(),
            name: name.into(),
            version: version.into(),
            tier: ServiceTier::Tier5,
            host: "localhost".to_owned(),
            port: 0,
            protocol: "REST".to_owned(),
            health_path: "/health".to_owned(),
            metadata: HashMap::new(),
            ttl_seconds: None,
            module_id: None,
        }
    }

    /// Set the service tier.
    #[must_use]
    pub const fn tier(mut self, tier: ServiceTier) -> Self {
        self.tier = tier;
        self
    }

    /// Set the host address.
    #[must_use]
    pub fn host(mut self, host: impl Into<String>) -> Self {
        self.host = host.into();
        self
    }

    /// Set the TCP port.
    #[must_use]
    pub const fn port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }

    /// Set the wire protocol identifier.
    #[must_use]
    pub fn protocol(mut self, protocol: impl Into<String>) -> Self {
        self.protocol = protocol.into();
        self
    }

    /// Set the health check path.
    #[must_use]
    pub fn health_path(mut self, path: impl Into<String>) -> Self {
        self.health_path = path.into();
        self
    }

    /// Insert a metadata key-value pair.
    #[must_use]
    pub fn metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Set the TTL in seconds.
    #[must_use]
    pub const fn ttl_seconds(mut self, ttl: u64) -> Self {
        self.ttl_seconds = Some(ttl);
        self
    }

    /// Set the module identity for ME-internal modules.
    #[must_use]
    pub const fn module_id(mut self, id: ModuleId) -> Self {
        self.module_id = Some(id);
        self
    }

    /// Consume the builder and produce a [`ServiceDefinition`].
    #[must_use]
    pub fn build(self) -> ServiceDefinition {
        ServiceDefinition {
            service_id: self.service_id,
            name: self.name,
            version: self.version,
            tier: self.tier,
            host: self.host,
            port: self.port,
            protocol: self.protocol,
            health_path: self.health_path,
            metadata: self.metadata,
            registered_at: Timestamp::now(),
            ttl_seconds: self.ttl_seconds,
            module_id: self.module_id,
        }
    }
}

impl ServiceDefinition {
    /// Convenience entry-point that returns a [`ServiceDefinitionBuilder`].
    #[must_use]
    pub fn builder(
        service_id: impl Into<String>,
        name: impl Into<String>,
        version: impl Into<String>,
    ) -> ServiceDefinitionBuilder {
        ServiceDefinitionBuilder::new(service_id, name, version)
    }
}

// ============================================================================
// Internal state
// ============================================================================

/// Interior state of the service registry, protected by `RwLock`.
#[derive(Debug, Default)]
struct RegistryState {
    /// All registered services, keyed by `service_id`.
    services: HashMap<String, ServiceDefinition>,
    /// Per-service health status.
    health: HashMap<String, HealthStatus>,
    /// Forward dependency edges: `from -> [to1, to2, ...]`.
    dependencies: HashMap<String, Vec<String>>,
}

// ============================================================================
// ServiceRegistry
// ============================================================================

/// Concrete implementation of [`ServiceDiscovery`] with interior mutability.
///
/// Uses `parking_lot::RwLock` for thread-safe `&self` access (C2).
/// Optionally emits health signals via `Arc<SignalBus>` (C6).
///
/// # Examples
///
/// ```
/// use maintenance_engine::m2_services::service_registry::{ServiceRegistry, ServiceDefinition, ServiceDiscovery};
/// use maintenance_engine::m2_services::ServiceTier;
///
/// let registry = ServiceRegistry::new();
/// let def = ServiceDefinition::builder("synthex", "SYNTHEX", "1.0.0")
///     .tier(ServiceTier::Tier1)
///     .port(8090)
///     .build();
/// registry.register(def).unwrap();
/// assert_eq!(registry.service_count(), 1);
/// ```
#[derive(Debug)]
pub struct ServiceRegistry {
    state: RwLock<RegistryState>,
    signal_bus: Option<Arc<SignalBus>>,
    metrics: Option<Arc<MetricsRegistry>>,
}

impl Default for ServiceRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ServiceRegistry {
    /// Create an empty registry with no signal bus or metrics.
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: RwLock::new(RegistryState::default()),
            signal_bus: None,
            metrics: None,
        }
    }

    /// Create a registry with a signal bus for health transition signals.
    #[must_use]
    pub fn with_signal_bus(mut self, bus: Arc<SignalBus>) -> Self {
        self.signal_bus = Some(bus);
        self
    }

    /// Create a registry with a metrics registry.
    #[must_use]
    pub fn with_metrics(mut self, metrics: Arc<MetricsRegistry>) -> Self {
        self.metrics = Some(metrics);
        self
    }

    /// Emit a health signal when status transitions.
    fn emit_health_transition(
        &self,
        service_id: &str,
        previous: HealthStatus,
        current: HealthStatus,
    ) {
        if let Some(ref bus) = self.signal_bus {
            let signal = HealthSignal::new(
                ModuleId::M09,
                previous.score(),
                current.score(),
                format!("Service '{service_id}' health: {previous} -> {current}"),
            );
            bus.emit_health(&signal);
        }
    }

    /// Record a metrics counter increment (if metrics available).
    const fn record_metric(&self, _name: &str) {
        // Metrics integration: counter increments tracked via MetricsRegistry.
        // Actual implementation deferred to MetricsRegistry API availability.
        let _ = &self.metrics;
    }
}

impl ServiceDiscovery for ServiceRegistry {
    fn register(&self, def: ServiceDefinition) -> Result<()> {
        let mut state = self.state.write();
        if state.services.contains_key(&def.service_id) {
            return Err(Error::Validation(format!(
                "Service '{}' is already registered",
                def.service_id
            )));
        }
        let id = def.service_id.clone();
        state.services.insert(id.clone(), def);
        state.health.insert(id, HealthStatus::Unknown);
        drop(state);
        self.record_metric("l2_service_registrations_total");
        Ok(())
    }

    fn deregister(&self, service_id: &str) -> Result<()> {
        let mut state = self.state.write();
        if state.services.remove(service_id).is_none() {
            return Err(Error::ServiceNotFound(service_id.to_owned()));
        }
        state.health.remove(service_id);
        state.dependencies.remove(service_id);
        for targets in state.dependencies.values_mut() {
            targets.retain(|t| t != service_id);
        }
        drop(state);
        Ok(())
    }

    fn discover(&self, service_id: &str) -> Result<ServiceDefinition> {
        let state = self.state.read();
        state
            .services
            .get(service_id)
            .cloned()
            .ok_or_else(|| Error::ServiceNotFound(service_id.to_owned()))
    }

    fn discover_by_tier(&self, tier: ServiceTier) -> Vec<ServiceDefinition> {
        let state = self.state.read();
        state
            .services
            .values()
            .filter(|s| s.tier == tier)
            .cloned()
            .collect()
    }

    fn discover_by_protocol(&self, protocol: &str) -> Vec<ServiceDefinition> {
        let upper = protocol.to_uppercase();
        let state = self.state.read();
        state
            .services
            .values()
            .filter(|s| s.protocol.to_uppercase() == upper)
            .cloned()
            .collect()
    }

    fn list_services(&self) -> Vec<ServiceDefinition> {
        let state = self.state.read();
        state.services.values().cloned().collect()
    }

    fn update_health(&self, service_id: &str, status: HealthStatus) -> Result<()> {
        let mut state = self.state.write();
        if !state.services.contains_key(service_id) {
            return Err(Error::ServiceNotFound(service_id.to_owned()));
        }
        let previous = state
            .health
            .insert(service_id.to_owned(), status)
            .unwrap_or(HealthStatus::Unknown);
        drop(state);

        if previous != status {
            self.emit_health_transition(service_id, previous, status);
        }
        self.record_metric("l2_health_updates_total");
        Ok(())
    }

    fn get_health(&self, service_id: &str) -> Result<HealthStatus> {
        let state = self.state.read();
        state
            .health
            .get(service_id)
            .copied()
            .ok_or_else(|| Error::ServiceNotFound(service_id.to_owned()))
    }

    fn get_healthy_services(&self) -> Vec<ServiceDefinition> {
        let state = self.state.read();
        state
            .services
            .values()
            .filter(|s| {
                state
                    .health
                    .get(&s.service_id)
                    .copied()
                    .unwrap_or(HealthStatus::Unknown)
                    == HealthStatus::Healthy
            })
            .cloned()
            .collect()
    }

    fn add_dependency(&self, from: &str, to: &str) -> Result<()> {
        if from == to {
            return Err(Error::Validation(
                "A service cannot depend on itself".to_owned(),
            ));
        }
        let mut state = self.state.write();
        if !state.services.contains_key(from) {
            return Err(Error::ServiceNotFound(from.to_owned()));
        }
        if !state.services.contains_key(to) {
            return Err(Error::ServiceNotFound(to.to_owned()));
        }
        let targets = state.dependencies.entry(from.to_owned()).or_default();
        if !targets.contains(&to.to_owned()) {
            targets.push(to.to_owned());
        }
        drop(state);
        Ok(())
    }

    fn get_dependencies(&self, service_id: &str) -> Result<Vec<String>> {
        let state = self.state.read();
        if !state.services.contains_key(service_id) {
            return Err(Error::ServiceNotFound(service_id.to_owned()));
        }
        Ok(state
            .dependencies
            .get(service_id)
            .cloned()
            .unwrap_or_default())
    }

    fn get_dependents(&self, service_id: &str) -> Result<Vec<String>> {
        let state = self.state.read();
        if !state.services.contains_key(service_id) {
            return Err(Error::ServiceNotFound(service_id.to_owned()));
        }
        let dependents: Vec<String> = state
            .dependencies
            .iter()
            .filter(|(_, targets)| targets.contains(&service_id.to_owned()))
            .map(|(source, _)| source.clone())
            .collect();
        drop(state);
        Ok(dependents)
    }

    fn service_count(&self) -> usize {
        self.state.read().services.len()
    }

    fn is_registered(&self, service_id: &str) -> bool {
        self.state.read().services.contains_key(service_id)
    }
}

// ============================================================================
// TensorContributor implementation (C3)
// ============================================================================

impl TensorContributor for ServiceRegistry {
    fn contribute(&self) -> ContributedTensor {
        let state = self.state.read();
        let total = state.services.len();

        #[allow(clippy::cast_precision_loss)]
        let service_count_norm = if total > 0 {
            (total as f64 / 12.0).min(1.0)
        } else {
            0.0
        };

        #[allow(clippy::cast_precision_loss)]
        let avg_tier = if total > 0 {
            let sum: f64 = state.services.values().map(|s| s.tier.normalized()).sum();
            sum / total as f64
        } else {
            0.0
        };

        #[allow(clippy::cast_precision_loss)]
        let avg_deps = if total > 0 {
            let dep_count: usize = state
                .dependencies
                .values()
                .map(Vec::len)
                .sum();
            (dep_count as f64 / (total as f64 * 12.0)).min(1.0)
        } else {
            0.0
        };

        #[allow(clippy::cast_precision_loss)]
        let healthy_ratio = if total > 0 {
            let healthy_count = state
                .health
                .values()
                .filter(|h| **h == HealthStatus::Healthy)
                .count();
            healthy_count as f64 / total as f64
        } else {
            0.0
        };

        drop(state);

        let tensor = Tensor12D::new([
            service_count_norm, // D0: service count
            0.0,               // D1: port (not applicable)
            avg_tier,          // D2: avg tier
            avg_deps,          // D3: avg deps
            healthy_ratio,     // D4: healthy count ratio
            0.0,               // D5: protocol (not applicable)
            0.0,               // D6: health (contributed by M10)
            0.0,               // D7: uptime (contributed by M11)
            0.0,               // D8: synergy
            0.0,               // D9: latency (contributed by M12)
            0.0,               // D10: error rate (contributed by M10/M12)
            0.0,               // D11: temporal
        ]);

        let coverage = CoverageBitmap::EMPTY
            .with_dimension(DimensionIndex::ServiceId)
            .with_dimension(DimensionIndex::Tier)
            .with_dimension(DimensionIndex::DependencyCount)
            .with_dimension(DimensionIndex::AgentCount);

        ContributedTensor::new(tensor, coverage, ContributorKind::Stream)
    }

    fn contributor_kind(&self) -> ContributorKind {
        ContributorKind::Stream
    }

    fn module_id(&self) -> &str {
        ModuleId::M09.as_str()
    }
}

// ============================================================================
// ULTRAPLATE pre-registration helper
// ============================================================================

/// Internal helper to register a single ULTRAPLATE service.
#[allow(clippy::too_many_arguments)]
fn register_service(
    registry: &dyn ServiceDiscovery,
    id: &str,
    name: &str,
    version: &str,
    tier: ServiceTier,
    host: &str,
    port: u16,
    protocol: &str,
    health_path: &str,
) -> Result<()> {
    let def = ServiceDefinition::builder(id, name, version)
        .tier(tier)
        .host(host)
        .port(port)
        .protocol(protocol)
        .health_path(health_path)
        .build();
    registry.register(def)
}

/// Pre-register all 12 ULTRAPLATE services with canonical ports and tiers.
///
/// # Errors
///
/// Propagates any [`Error`] from [`ServiceDiscovery::register`].
///
/// # Examples
///
/// ```
/// use maintenance_engine::m2_services::service_registry::{ServiceRegistry, register_ultraplate_services, ServiceDiscovery};
///
/// let registry = ServiceRegistry::new();
/// register_ultraplate_services(&registry).unwrap();
/// assert_eq!(registry.service_count(), 12);
/// ```
pub fn register_ultraplate_services(registry: &dyn ServiceDiscovery) -> Result<()> {
    register_service(registry, "synthex", "SYNTHEX Engine", "1.0.0", ServiceTier::Tier1, "localhost", 8090, "REST", "/api/health")?;
    register_service(registry, "san-k7", "SAN-K7 Orchestrator", "1.55.0", ServiceTier::Tier1, "localhost", 8100, "REST", "/health")?;
    register_service(registry, "nais", "NAIS", "1.0.0", ServiceTier::Tier2, "localhost", 8101, "REST", "/health")?;
    // codesynthor-v7 (8110) retired S091 — superseded by V8 (8111)
    // devops-engine V2 (8081) retired S091 — superseded by V3 (8082)
    // tool-library V1 (8105) retired S093 — superseded by V2 hb binary (CLI, no port)
    // library-agent (8083) removed: disabled in devenv, was dragging fitness tensor
    register_service(registry, "ccm", "Claude Context Manager", "1.0.0", ServiceTier::Tier3, "localhost", 8104, "REST", "/health")?;
    // prometheus-swarm V1 (10001) retired S088 — superseded by V2 (10002)
    // architect-agent (9001) retired S093 — absorbed by V2 + V8
    register_service(registry, "bash-engine", "Bash Engine", "1.0.0", ServiceTier::Tier5, "localhost", 8102, "REST", "/health")?;
    register_service(registry, "tool-maker", "Tool Maker", "1.55.0", ServiceTier::Tier5, "localhost", 8103, "REST", "/health")?;

    Ok(())
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ---- helpers ----------------------------------------------------------

    fn test_service(id: &str, tier: ServiceTier, port: u16) -> ServiceDefinition {
        ServiceDefinition::builder(id, id, "0.1.0")
            .tier(tier)
            .port(port)
            .protocol("REST")
            .build()
    }

    fn populated_registry() -> ServiceRegistry {
        let reg = ServiceRegistry::new();
        let _ = reg.register(test_service("alpha", ServiceTier::Tier1, 8001));
        let _ = reg.register(test_service("beta", ServiceTier::Tier2, 8002));
        let _ = reg.register(test_service("gamma", ServiceTier::Tier3, 8003));
        reg
    }

    // ==== [COMPILE] trait object safety + Send + Sync =====================

    #[test]
    fn test_service_discovery_is_object_safe() {
        fn accept_boxed(_r: Box<dyn ServiceDiscovery>) {}
        let reg = Box::new(ServiceRegistry::new());
        accept_boxed(reg);
    }

    #[test]
    fn test_service_registry_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<ServiceRegistry>();
    }

    #[test]
    fn test_service_discovery_arc_dyn() {
        fn accept_arc(_r: Arc<dyn ServiceDiscovery>) {}
        let reg: Arc<dyn ServiceDiscovery> = Arc::new(ServiceRegistry::new());
        accept_arc(reg);
    }

    // ==== [BASIC] happy path, construction, defaults =====================

    #[test]
    fn test_register_and_discover() {
        let reg = ServiceRegistry::new();
        let def = test_service("synthex", ServiceTier::Tier1, 8090);
        assert!(reg.register(def).is_ok());
        assert_eq!(reg.service_count(), 1);
        let found = reg.discover("synthex");
        assert!(found.is_ok());
        assert_eq!(found.map(|s| s.service_id).ok(), Some("synthex".to_owned()));
    }

    #[test]
    fn test_builder_defaults() {
        let def = ServiceDefinition::builder("svc", "Service", "0.1.0").build();
        assert_eq!(def.service_id, "svc");
        assert_eq!(def.host, "localhost");
        assert_eq!(def.port, 0);
        assert_eq!(def.protocol, "REST");
        assert_eq!(def.health_path, "/health");
        assert!(def.ttl_seconds.is_none());
        assert!(def.metadata.is_empty());
        assert_eq!(def.tier, ServiceTier::Tier5);
        assert!(def.module_id.is_none());
    }

    #[test]
    fn test_builder_all_fields() {
        let def = ServiceDefinition::builder("full", "Full Service", "2.0.0")
            .tier(ServiceTier::Tier1)
            .host("10.0.0.1")
            .port(9999)
            .protocol("gRPC")
            .health_path("/api/v2/health")
            .metadata("env", "production")
            .metadata("region", "us-east-1")
            .ttl_seconds(300)
            .module_id(ModuleId::M09)
            .build();

        assert_eq!(def.service_id, "full");
        assert_eq!(def.name, "Full Service");
        assert_eq!(def.version, "2.0.0");
        assert_eq!(def.tier, ServiceTier::Tier1);
        assert_eq!(def.host, "10.0.0.1");
        assert_eq!(def.port, 9999);
        assert_eq!(def.protocol, "gRPC");
        assert_eq!(def.health_path, "/api/v2/health");
        assert_eq!(def.metadata.len(), 2);
        assert_eq!(def.metadata.get("env").map(String::as_str), Some("production"));
        assert_eq!(def.ttl_seconds, Some(300));
        assert_eq!(def.module_id, Some(ModuleId::M09));
    }

    #[test]
    fn test_new_registry_empty() {
        let reg = ServiceRegistry::new();
        assert_eq!(reg.service_count(), 0);
        assert!(reg.list_services().is_empty());
    }

    #[test]
    fn test_registry_default() {
        let reg = ServiceRegistry::default();
        assert_eq!(reg.service_count(), 0);
    }

    #[test]
    fn test_service_definition_display() {
        let def = test_service("synthex", ServiceTier::Tier1, 8090);
        let display = def.to_string();
        assert!(display.contains("synthex"));
        assert!(display.contains("Tier1"));
    }

    #[test]
    fn test_service_definition_timestamp_is_timestamp() {
        let def = test_service("svc", ServiceTier::Tier5, 8000);
        assert!(def.registered_at.ticks() > 0);
    }

    // ==== [INVARIANT] FSM transitions, clamping, sort orders =============

    #[test]
    fn test_health_default_is_unknown() {
        let reg = populated_registry();
        let health = reg.get_health("alpha");
        assert!(health.is_ok());
        assert_eq!(health.ok(), Some(HealthStatus::Unknown));
    }

    #[test]
    fn test_deregister_cleans_dependency_edges() {
        let reg = populated_registry();
        let _ = reg.add_dependency("alpha", "beta");
        let _ = reg.add_dependency("gamma", "beta");
        let _ = reg.deregister("beta");
        let alpha_deps = reg.get_dependencies("alpha").unwrap_or_default();
        assert!(alpha_deps.is_empty());
    }

    #[test]
    fn test_deregister_cleans_health() {
        let reg = populated_registry();
        let _ = reg.update_health("alpha", HealthStatus::Healthy);
        let _ = reg.deregister("alpha");
        assert!(reg.get_health("alpha").is_err());
    }

    #[test]
    fn test_duplicate_dependency_idempotent() {
        let reg = populated_registry();
        let _ = reg.add_dependency("alpha", "beta");
        let _ = reg.add_dependency("alpha", "beta");
        let deps = reg.get_dependencies("alpha").unwrap_or_default();
        assert_eq!(deps.len(), 1);
    }

    // ==== [BOUNDARY] empty registry, single endpoint, threshold exact ====

    #[test]
    fn test_discover_empty_registry() {
        let reg = ServiceRegistry::new();
        assert!(reg.discover("missing").is_err());
    }

    #[test]
    fn test_discover_by_tier_empty() {
        let reg = ServiceRegistry::new();
        assert!(reg.discover_by_tier(ServiceTier::Tier1).is_empty());
    }

    #[test]
    fn test_list_services_single() {
        let reg = ServiceRegistry::new();
        let _ = reg.register(test_service("solo", ServiceTier::Tier5, 9000));
        assert_eq!(reg.list_services().len(), 1);
    }

    #[test]
    fn test_get_dependencies_no_deps() {
        let reg = populated_registry();
        let deps = reg.get_dependencies("alpha").unwrap_or_default();
        assert!(deps.is_empty());
    }

    #[test]
    fn test_get_dependents_no_dependents() {
        let reg = populated_registry();
        let dependents = reg.get_dependents("alpha").unwrap_or_default();
        assert!(dependents.is_empty());
    }

    // ==== [PROPERTY] health score always in [0,1], random sweeps =========

    #[test]
    fn test_healthy_services_subset_of_all() {
        let reg = populated_registry();
        let _ = reg.update_health("alpha", HealthStatus::Healthy);
        let healthy = reg.get_healthy_services();
        let all = reg.list_services();
        assert!(healthy.len() <= all.len());
    }

    #[test]
    fn test_service_count_matches_list_len() {
        let reg = populated_registry();
        assert_eq!(reg.service_count(), reg.list_services().len());
    }

    #[test]
    fn test_is_registered_consistent_with_discover() {
        let reg = populated_registry();
        assert!(reg.is_registered("alpha"));
        assert!(reg.discover("alpha").is_ok());
        assert!(!reg.is_registered("nonexistent"));
        assert!(reg.discover("nonexistent").is_err());
    }

    // ==== [NEGATIVE] unknown service, duplicate registration, errors =====

    #[test]
    fn test_duplicate_registration_fails() {
        let reg = ServiceRegistry::new();
        let def1 = test_service("synthex", ServiceTier::Tier1, 8090);
        let def2 = test_service("synthex", ServiceTier::Tier1, 8090);
        assert!(reg.register(def1).is_ok());
        let result = reg.register(def2);
        assert!(result.is_err());
    }

    #[test]
    fn test_deregister_unknown_service_fails() {
        let reg = ServiceRegistry::new();
        assert!(reg.deregister("nonexistent").is_err());
    }

    #[test]
    fn test_discover_not_found() {
        let reg = ServiceRegistry::new();
        assert!(reg.discover("missing").is_err());
    }

    #[test]
    fn test_update_health_unknown_service_fails() {
        let reg = ServiceRegistry::new();
        assert!(reg.update_health("ghost", HealthStatus::Healthy).is_err());
    }

    #[test]
    fn test_get_health_unknown_fails() {
        let reg = ServiceRegistry::new();
        assert!(reg.get_health("ghost").is_err());
    }

    #[test]
    fn test_dependency_on_unknown_service_fails() {
        let reg = populated_registry();
        assert!(reg.add_dependency("alpha", "nonexistent").is_err());
        assert!(reg.add_dependency("nonexistent", "alpha").is_err());
    }

    #[test]
    fn test_self_dependency_fails() {
        let reg = populated_registry();
        assert!(reg.add_dependency("alpha", "alpha").is_err());
    }

    #[test]
    fn test_get_dependencies_unknown_fails() {
        let reg = ServiceRegistry::new();
        assert!(reg.get_dependencies("unknown").is_err());
    }

    #[test]
    fn test_get_dependents_unknown_fails() {
        let reg = ServiceRegistry::new();
        assert!(reg.get_dependents("unknown").is_err());
    }

    // ==== [INTEGRATION] cross-module, signal emission ====================

    #[test]
    fn test_signal_bus_none_does_not_panic() {
        let reg = ServiceRegistry::new();
        let _ = reg.register(test_service("svc", ServiceTier::Tier5, 8000));
        // update_health with no signal bus should not panic
        assert!(reg.update_health("svc", HealthStatus::Healthy).is_ok());
    }

    #[test]
    fn test_health_transition_emits_signal() {
        let bus = Arc::new(SignalBus::new());
        let reg = ServiceRegistry::new().with_signal_bus(Arc::clone(&bus));
        let _ = reg.register(test_service("svc", ServiceTier::Tier5, 8000));

        // Unknown -> Healthy (transition should emit)
        let _ = reg.update_health("svc", HealthStatus::Healthy);
        assert_eq!(bus.stats().health_emitted, 1);

        // Healthy -> Healthy (no transition, no emit)
        let _ = reg.update_health("svc", HealthStatus::Healthy);
        assert_eq!(bus.stats().health_emitted, 1);

        // Healthy -> Degraded (transition should emit)
        let _ = reg.update_health("svc", HealthStatus::Degraded);
        assert_eq!(bus.stats().health_emitted, 2);
    }

    #[test]
    fn test_register_ultraplate_services() {
        let reg = ServiceRegistry::new();
        let result = register_ultraplate_services(&reg);
        assert!(result.is_ok());
        assert_eq!(reg.service_count(), 6); // 5 retired services dropped + library-agent removed
        assert!(reg.is_registered("synthex"));
        assert!(reg.is_registered("san-k7"));
        assert!(reg.is_registered("bash-engine"));
        assert!(reg.is_registered("tool-maker"));
    }

    #[test]
    fn test_ultraplate_tiers() {
        let reg = ServiceRegistry::new();
        let _ = register_ultraplate_services(&reg);

        assert_eq!(reg.discover_by_tier(ServiceTier::Tier1).len(), 2);
        assert_eq!(reg.discover_by_tier(ServiceTier::Tier2).len(), 1); // codesynthor-v7, devops-engine retired
        assert_eq!(reg.discover_by_tier(ServiceTier::Tier3).len(), 1); // tool-library retired, library-agent removed
        assert_eq!(reg.discover_by_tier(ServiceTier::Tier4).len(), 0); // prometheus-swarm, architect-agent retired
        assert_eq!(reg.discover_by_tier(ServiceTier::Tier5).len(), 2);
    }

    #[test]
    fn test_ultraplate_duplicate_registration_fails() {
        let reg = ServiceRegistry::new();
        let _ = register_ultraplate_services(&reg);
        let result = register_ultraplate_services(&reg);
        assert!(result.is_err());
    }

    // ==== discovery filters ==============================================

    #[test]
    fn test_discover_by_tier() {
        let reg = populated_registry();
        let tier1 = reg.discover_by_tier(ServiceTier::Tier1);
        assert_eq!(tier1.len(), 1);
        assert_eq!(tier1[0].service_id, "alpha");

        let tier4 = reg.discover_by_tier(ServiceTier::Tier4);
        assert!(tier4.is_empty());
    }

    #[test]
    fn test_discover_by_protocol() {
        let reg = ServiceRegistry::new();
        let rest = ServiceDefinition::builder("rest-svc", "REST Service", "1.0.0")
            .protocol("REST")
            .port(9000)
            .build();
        let grpc = ServiceDefinition::builder("grpc-svc", "gRPC Service", "1.0.0")
            .protocol("gRPC")
            .port(9001)
            .build();
        let _ = reg.register(rest);
        let _ = reg.register(grpc);

        assert_eq!(reg.discover_by_protocol("REST").len(), 1);
        // Case-insensitive
        assert_eq!(reg.discover_by_protocol("rest").len(), 1);
        assert_eq!(reg.discover_by_protocol("gRPC").len(), 1);
    }

    #[test]
    fn test_list_services() {
        let reg = populated_registry();
        assert_eq!(reg.list_services().len(), 3);
    }

    // ==== health ==========================================================

    #[test]
    fn test_health_update_and_query() {
        let reg = populated_registry();
        assert!(reg.update_health("alpha", HealthStatus::Healthy).is_ok());
        assert_eq!(reg.get_health("alpha").ok(), Some(HealthStatus::Healthy));
    }

    #[test]
    fn test_get_healthy_services() {
        let reg = populated_registry();
        assert!(reg.get_healthy_services().is_empty());

        let _ = reg.update_health("alpha", HealthStatus::Healthy);
        let _ = reg.update_health("gamma", HealthStatus::Healthy);
        assert_eq!(reg.get_healthy_services().len(), 2);
    }

    // ==== dependencies ====================================================

    #[test]
    fn test_add_and_get_dependencies() {
        let reg = populated_registry();
        assert!(reg.add_dependency("alpha", "beta").is_ok());
        assert!(reg.add_dependency("alpha", "gamma").is_ok());

        let deps = reg.get_dependencies("alpha").unwrap_or_default();
        assert_eq!(deps.len(), 2);
        assert!(deps.contains(&"beta".to_owned()));
        assert!(deps.contains(&"gamma".to_owned()));
    }

    #[test]
    fn test_get_dependents() {
        let reg = populated_registry();
        let _ = reg.add_dependency("alpha", "gamma");
        let _ = reg.add_dependency("beta", "gamma");

        let dependents = reg.get_dependents("gamma").unwrap_or_default();
        assert_eq!(dependents.len(), 2);
        assert!(dependents.contains(&"alpha".to_owned()));
        assert!(dependents.contains(&"beta".to_owned()));
    }

    // ==== [TENSOR] contribution ==========================================

    #[test]
    fn test_tensor_contributor_module_id() {
        let reg = ServiceRegistry::new();
        assert_eq!(reg.module_id(), "M09");
    }

    #[test]
    fn test_tensor_contributor_kind() {
        let reg = ServiceRegistry::new();
        assert_eq!(reg.contributor_kind(), ContributorKind::Stream);
    }

    #[test]
    fn test_tensor_empty_registry() {
        let reg = ServiceRegistry::new();
        let ct = reg.contribute();
        assert_eq!(ct.coverage.count(), 4);
        let arr = ct.tensor.to_array();
        assert!(arr[0].abs() < f64::EPSILON); // D0: 0 services
        assert!(arr[2].abs() < f64::EPSILON); // D2: no tier data
    }

    #[test]
    fn test_tensor_populated_registry() {
        let reg = ServiceRegistry::new();
        let _ = register_ultraplate_services(&reg);
        let ct = reg.contribute();

        // D0: 6/12.0 = 0.5 (12.0 normalization ceiling; library-agent + 5 retired services removed)
        #[allow(clippy::cast_precision_loss)]
        let expected_d0 = 6.0 / 12.0;
        assert!((ct.tensor.to_array()[0] - expected_d0).abs() < 1e-10);

        // D2: average tier should be > 0
        assert!(ct.tensor.to_array()[2] > 0.0);

        // Coverage should have D0, D2, D3, D4
        assert!(ct.coverage.is_covered(DimensionIndex::ServiceId));
        assert!(ct.coverage.is_covered(DimensionIndex::Tier));
        assert!(ct.coverage.is_covered(DimensionIndex::DependencyCount));
        assert!(ct.coverage.is_covered(DimensionIndex::AgentCount));
        assert!(!ct.coverage.is_covered(DimensionIndex::HealthScore));
    }

    #[test]
    fn test_tensor_all_dims_in_unit_interval() {
        let reg = ServiceRegistry::new();
        let _ = register_ultraplate_services(&reg);
        let ct = reg.contribute();
        for val in ct.tensor.to_array() {
            assert!(
                (0.0..=1.0).contains(&val),
                "Tensor value out of range: {val}"
            );
        }
    }

    #[test]
    fn test_tensor_healthy_ratio() {
        let reg = ServiceRegistry::new();
        let _ = register_ultraplate_services(&reg);
        // Mark half as healthy (3 of 6 active services)
        let _ = reg.update_health("synthex", HealthStatus::Healthy);
        let _ = reg.update_health("san-k7", HealthStatus::Healthy);
        let _ = reg.update_health("nais", HealthStatus::Healthy);

        let ct = reg.contribute();
        #[allow(clippy::cast_precision_loss)]
        let expected_d4 = 3.0 / 6.0; // 3 healthy out of 6 active services post-S097 cleanup
        let d4 = ct.tensor.to_array()[4]; // D4: healthy ratio
        assert!((d4 - expected_d4).abs() < 1e-10);
    }

    #[test]
    fn test_tensor_coverage_bitmap_matches_populated_dims() {
        let reg = ServiceRegistry::new();
        let _ = register_ultraplate_services(&reg);
        let ct = reg.contribute();
        let covered = ct.coverage.covered_dimensions();
        assert_eq!(covered.len(), 4);
    }

    // ==== [INTEGRATION] trait-via-arc =====================================

    #[test]
    fn test_trait_via_arc_dyn() {
        let registry: Arc<dyn ServiceDiscovery> = Arc::new(ServiceRegistry::new());
        let def = test_service("svc", ServiceTier::Tier5, 8000);
        assert!(registry.register(def).is_ok());
        assert_eq!(registry.service_count(), 1);
        assert!(registry.discover("svc").is_ok());
    }

    #[test]
    fn test_l2_uses_l1_error_type() {
        let reg = ServiceRegistry::new();
        let err = reg.discover("nonexistent").unwrap_err();
        // Error::ServiceNotFound is from L1
        let msg = err.to_string();
        assert!(msg.contains("nonexistent") || msg.contains("not found"));
    }

    #[test]
    fn test_l2_uses_timestamp_not_chrono() {
        let def = test_service("svc", ServiceTier::Tier5, 8000);
        // ServiceDefinition.registered_at is Timestamp (not SystemTime/chrono)
        let _ticks: u64 = def.registered_at.ticks();
    }
}
