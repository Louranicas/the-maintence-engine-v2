# M09 SERVICE REGISTRY SPECIFICATION

> Technical specification for The Maintenance Engine v1.0.0 — Module M09
> 30-Facet Taxonomy Extraction | Generated: 2026-03-01

---

## Overview

- **Module ID:** M09
- **Layer:** L2 Services
- **File:** `src/m2_services/service_registry.rs`
- **LOC:** 1,285
- **Tests:** 53 unit tests
- **Status:** COMPLETE
- **Primary Trait:** `ServiceDiscovery` (14 methods)
- **Purpose:** Service discovery, registration, dependency tracking, and ULTRAPLATE fleet bootstrap

---

## 1. Public API Surface (F01)

### Trait: ServiceDiscovery

```rust
pub trait ServiceDiscovery: Send + Sync + fmt::Debug {
    /// Register a new service definition. Errors on duplicate service_id.
    fn register(&self, def: ServiceDefinition) -> Result<()>;

    /// Remove a service by ID. Errors if not found.
    fn deregister(&self, service_id: &str) -> Result<()>;

    /// Look up a service by ID. Returns owned clone (C7).
    fn discover(&self, service_id: &str) -> Result<ServiceDefinition>;

    /// Filter services by tier. Returns owned Vec (C7).
    fn discover_by_tier(&self, tier: ServiceTier) -> Vec<ServiceDefinition>;

    /// Filter services by protocol (case-insensitive). Returns owned Vec (C7).
    fn discover_by_protocol(&self, protocol: &str) -> Vec<ServiceDefinition>;

    /// List all registered services. Returns owned Vec (C7).
    fn list_services(&self) -> Vec<ServiceDefinition>;

    /// Update health status. Emits signal on transition (C6).
    fn update_health(&self, service_id: &str, status: HealthStatus) -> Result<()>;

    /// Get current health status. Returns owned (C7).
    fn get_health(&self, service_id: &str) -> Result<HealthStatus>;

    /// Get all services with Healthy status.
    fn get_healthy_services(&self) -> Vec<ServiceDefinition>;

    /// Record a dependency edge (from depends on to). Validates no self-dependency.
    fn add_dependency(&self, from: &str, to: &str) -> Result<()>;

    /// Get forward dependencies. Returns owned Vec (C7).
    fn get_dependencies(&self, service_id: &str) -> Result<Vec<String>>;

    /// Get reverse dependencies (who depends on me). Returns owned Vec (C7).
    fn get_dependents(&self, service_id: &str) -> Result<Vec<String>>;

    /// Count of registered services.
    fn service_count(&self) -> usize;

    /// Check if a service is registered.
    fn is_registered(&self, service_id: &str) -> bool;
}
```

---

## 2. Data Structures (F08, F09)

### ServiceDefinition

```rust
#[derive(Clone, Debug)]
pub struct ServiceDefinition {
    pub service_id: String,
    pub name: String,
    pub version: String,         // default: "1.0.0"
    pub tier: ServiceTier,       // default: Tier5
    pub host: String,            // default: "127.0.0.1"
    pub port: u16,               // default: 0
    pub protocol: String,        // default: "REST"
    pub health_path: String,     // default: "/health"
    pub metadata: HashMap<String, String>,
    pub registered_at: Timestamp, // C5: no chrono
    pub ttl_seconds: Option<u64>,
    pub module_id: Option<ModuleId>,
}
```

**Derives:** Clone, Debug
**Display:** `"{name} ({service_id}) v{version} @ {host}:{port} [{tier}]"`

### ServiceDefinitionBuilder

```rust
pub struct ServiceDefinitionBuilder {
    // mirrors ServiceDefinition fields with defaults
}

impl ServiceDefinitionBuilder {
    pub fn new() -> Self;
    pub fn service_id(mut self, id: impl Into<String>) -> Self;
    pub fn name(mut self, name: impl Into<String>) -> Self;
    pub fn version(mut self, version: impl Into<String>) -> Self;
    pub fn tier(mut self, tier: ServiceTier) -> Self;
    pub fn host(mut self, host: impl Into<String>) -> Self;
    pub fn port(mut self, port: u16) -> Self;
    pub fn protocol(mut self, protocol: impl Into<String>) -> Self;
    pub fn health_path(mut self, path: impl Into<String>) -> Self;
    pub fn metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self;
    pub fn ttl(mut self, ttl_seconds: u64) -> Self;
    pub fn module_id(mut self, id: ModuleId) -> Self;
    pub fn build(self) -> ServiceDefinition;
}
```

---

## 3. Internal Architecture (F13)

### ServiceRegistry

```rust
#[derive(Debug)]
pub struct ServiceRegistry {
    state: RwLock<RegistryState>,
    signal_bus: Option<Arc<SignalBus>>,
    metrics: Option<Arc<MetricsRegistry>>,
}

struct RegistryState {
    services: HashMap<String, ServiceDefinition>,
    health_map: HashMap<String, HealthStatus>,
    dependencies: HashMap<String, Vec<String>>,  // forward deps
}
```

**Constructors:**
- `new()` — Empty registry
- `default()` — Same as `new()`
- `with_signal_bus(bus: Arc<SignalBus>)` — With signal emission
- `with_metrics(metrics: Arc<MetricsRegistry>)` — With metrics recording

---

## 4. Tensor Contribution (F06)

```rust
impl TensorContributor for ServiceRegistry {
    fn contribute(&self) -> ContributedTensor {
        // D0: service_id = service_count / 12.0
        // D2: tier = avg(tier.normalized())   where normalized = tier_number / 6.0
        // D3: deps = avg(dep_count) / 12.0
        // D4: agents = healthy_count / total_count
        ContributedTensor {
            dimensions: [(D0, val), (D2, val), (D3, val), (D4, val)],
            coverage: CoverageBitmap::from([0, 2, 3, 4]),
            source: ModuleId::M09,
        }
    }
}
```

| Dimension | Index | Formula | Interpretation |
|-----------|-------|---------|---------------|
| service_id | D0 | `registered / 12.0` | Fleet completeness (1.0 = all 12 ULTRAPLATE services) |
| tier | D2 | `avg(tier / 6.0)` | Average criticality (lower = more critical) |
| deps | D3 | `avg(deps) / 12.0` | Coupling density |
| agents | D4 | `healthy / total` | Health ratio |

---

## 5. Signal Emission (F05)

| Trigger | Condition | Signal |
|---------|-----------|--------|
| `update_health()` | `old_status != new_status` | `HealthSignal { service_id, old, new, score }` |

**Pattern:** Read current status → compare → if different, emit signal → write new status.

---

## 6. Cross-Module Morphisms (F17)

### Imports from L1 Foundation

```rust
use crate::m1_foundation::{
    Error, Result,                    // Error handling
    Timestamp,                        // C5: temporal
    ModuleId,                         // Module identification
    SignalBus,                        // C6: signal emission
    MetricsRegistry,                  // Metrics recording
    TensorContributor,                // C3: tensor contribution
    ContributedTensor, CoverageBitmap, DimensionIndex,
    Tensor12D,                        // 12D encoding
};
use crate::m2_services::{
    ServiceTier, HealthStatus,        // Shared L2 enums
};
```

### Exports consumed by L3+

- `ServiceDefinition` — Used by lifecycle and health managers
- `ServiceDiscovery` — Trait object used for dependency injection
- `ServiceRegistry` — Concrete type for composition
- `register_ultraplate_services()` — Bootstrap function

---

## 7. ULTRAPLATE Bootstrap Function (F18)

```rust
/// Pre-register all 12 canonical ULTRAPLATE services.
/// Called during system initialization to seed the registry.
pub fn register_ultraplate_services(registry: &dyn ServiceDiscovery) -> Result<()>;
```

Registers these services with correct ports, tiers, health paths, and metadata:

| Service | Port | Tier | Health Path |
|---------|------|------|------------|
| maintenance-engine | 8080 | Tier1 | /api/health |
| devops-engine | 8081 | Tier1 | /health |
| synthex | 8090 | Tier2 | /api/health |
| san-k7-orchestrator | 8100 | Tier2 | /health |
| nais | 8101 | Tier3 | /health |
| bash-engine | 8102 | Tier3 | /health |
| tool-maker | 8103 | Tier3 | /health |
| claude-context-manager | 8104 | Tier4 | /health |
| tool-library | 8105 | Tier4 | /health |
| codesynthor-v7 | 8110 | Tier2 | /health |
| sphere-vortex | 8120 | Tier4 | /health |
| library-agent | 8083 | Tier5 | /health |

---

## 8. Error Taxonomy (F07)

| Error Scenario | ErrorKind | Message Pattern |
|---------------|-----------|-----------------|
| Register duplicate | `AlreadyExists` | "Service '{id}' already registered" |
| Discover not found | `NotFound` | "Service '{id}' not found" |
| Deregister not found | `NotFound` | "Service '{id}' not registered" |
| Self-dependency | `InvalidInput` | "Service cannot depend on itself" |
| Dependency source not found | `NotFound` | "Source service '{id}' not registered" |
| Dependency target not found | `NotFound` | "Target service '{id}' not registered" |
| Health update not found | `NotFound` | "Service '{id}' not registered" |

---

## 9. Concurrency Model (F04)

- **Lock type:** `parking_lot::RwLock` (not `std::sync::RwLock`)
- **Read path:** `.read()` → clone needed data → release → return owned
- **Write path:** `.write()` → mutate → optionally emit signal → release
- **Thread safety:** `ServiceRegistry: Send + Sync` (verified by tests)
- **Trait object safety:** `dyn ServiceDiscovery` is object-safe (verified by tests)
- **No deadlock risk:** Single lock per manager, never held across await points

---

## 10. Test Architecture (F14)

### Test Categories (53 tests)

| Category | Count | Focus |
|----------|-------|-------|
| Trait object safety | 2 | `dyn ServiceDiscovery`, Send+Sync |
| Registration | 8 | CRUD operations, duplicate handling |
| Discovery | 6 | By ID, tier, protocol, healthy filter |
| Health management | 5 | Update, query, transition detection |
| Dependencies | 7 | Add, get forward/reverse, self-dep rejection |
| ULTRAPLATE bootstrap | 5 | 12-service registration, tier distribution |
| Tensor contribution | 4 | Dimension values, coverage bitmap |
| Display/Default | 6 | ServiceDefinition Display, builder defaults |
| Signal emission | 4 | Health transition signals |
| Metrics | 2 | Counter increments |
| Edge cases | 4 | Empty registry, deregister+re-register |

---

## 11. Key Implementation Details

### Dependency Graph

The dependency graph is stored as a `HashMap<String, Vec<String>>` (adjacency list):
- **Forward edges:** `dependencies[from] = [to1, to2, ...]`
- **Reverse lookup:** `get_dependents()` scans all entries (O(n) — acceptable for 12-service fleet)
- **Validation:** Both `from` and `to` must be registered; self-dependency rejected

### Protocol Matching

`discover_by_protocol()` performs case-insensitive comparison:
```rust
service.protocol.to_lowercase() == protocol.to_lowercase()
```

### TTL (Time-to-Live)

`ttl_seconds: Option<u64>` is stored but **not automatically enforced** — expiration must be checked externally. This is a registration-time hint for cache invalidation.

---

## Version History

| Version | Date | Changes |
|---------|------|---------|
| 1.0.0 | 2026-03-01 | Initial 30-facet extraction |

---

*Generated: 2026-03-01 | The Maintenance Engine v1.0.0 | Module M09*
