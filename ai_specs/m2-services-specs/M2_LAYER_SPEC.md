# M2 SERVICES LAYER SPECIFICATION

> Technical specification for The Maintenance Engine v1.0.0 — Layer 2 (Services)
> 30-Facet Taxonomy Extraction | Generated: 2026-03-01

---

## Overview

- **Layer:** L2 — Services
- **Modules:** M09 (ServiceRegistry), M10 (HealthMonitor), M11 (Lifecycle), M12 (Resilience)
- **Version:** 1.0.0
- **Status:** COMPLETE
- **LOC:** 7,196 across 5 files
- **Tests:** 320 (279 unit + 41 integration)
- **Dependencies:** L1 Foundation only (C1 — no upward imports)

---

## 1. Layer Architecture

### File Layout

```
src/m2_services/
├── mod.rs                 (694 LOC, 20 tests)   — Coordinator: enums, structs, re-exports
├── service_registry.rs  (1,285 LOC, 53 tests)   — M09: Service discovery & registration
├── health_monitor.rs    (1,130 LOC, 49 tests)   — M10: Health check orchestration
├── lifecycle.rs         (1,898 LOC, 75 tests)   — M11: Service lifecycle FSM
└── resilience.rs        (2,189 LOC, 82 tests)   — M12: Circuit breaker + load balancer
```

### Module Responsibilities

| Module | ID | Role | Trait | Interior State |
|--------|----|------|-------|---------------|
| ServiceRegistry | M09 | Discovery, registration, dependency graph | `ServiceDiscovery` (14 methods) | `RwLock<RegistryState>` |
| HealthMonitor | M10 | Health FSM, thresholds, aggregation | `HealthMonitoring` (11 methods) | `RwLock<MonitorState>` |
| LifecycleManager | M11 | State transitions, restart backoff | `LifecycleOps` (18 methods) | `RwLock<HashMap<String, LifecycleEntry>>` |
| ResilienceManager | M12 | Fault tolerance, load distribution | `CircuitBreakerOps` (12 methods) + `LoadBalancing` (10 methods) | `RwLock<HashMap>` x2 |

---

## 2. Shared Types (mod.rs)

### Enums

```rust
/// Service operational state
pub enum ServiceStatus {
    Stopped,   // default — not running
    Starting,  // boot sequence in progress
    Running,   // fully operational
    Stopping,  // graceful shutdown
    Failed,    // crashed or unrecoverable
}

/// Health assessment outcome
pub enum HealthStatus {
    Healthy,   // default — score 1.0
    Degraded,  // score 0.5
    Unhealthy, // score 0.0
    Unknown,   // score 0.0
}

/// Service priority tier (affects tensor weighting)
pub enum ServiceTier {
    Tier1,  // weight 1.5 — critical infrastructure
    Tier2,  // weight 1.3 — core services
    Tier3,  // weight 1.2 — standard services
    Tier4,  // weight 1.1 — auxiliary
    Tier5,  // weight 1.0 — default, non-critical
}

/// Circuit breaker state
pub enum CircuitState {
    Closed,   // default — normal flow
    Open,     // rejecting all requests
    HalfOpen, // probing with limited requests
}
```

### Core Structs

```rust
/// Per-service runtime state snapshot
pub struct ServiceState {
    pub id: String,
    pub name: String,
    pub status: ServiceStatus,
    pub health_status: HealthStatus,
    pub tier: ServiceTier,
    pub port: u16,
    pub pid: Option<u32>,
    pub health_score: f64,        // [0.0, 1.0]
    pub synergy_score: f64,       // [0.0, 1.0]
    pub cpu_percent: f64,
    pub memory_mb: f64,
    pub uptime_seconds: u64,
    pub restart_count: u32,
    pub last_health_check: Timestamp,  // C5
    pub module_id: ModuleId,
    pub tensor: Tensor12D,
}

/// Layer-wide aggregated status
pub struct ServicesStatus {
    pub layer_id: &'static str,     // "L2"
    pub module_count: usize,        // 4
    pub registered_services: usize,
    pub healthy_services: usize,
    pub running_services: usize,
    pub open_circuits: usize,
    pub health_score: f64,
    pub tensor: Tensor12D,
}

/// Restart policy configuration
pub struct RestartConfig {
    pub max_restarts: u32,           // default: 5
    pub initial_backoff: Duration,   // default: 1s
    pub max_backoff: Duration,       // default: 30s
}
```

---

## 3. Constraint Compliance Matrix

| ID | Constraint | Status | Evidence |
|----|-----------|--------|----------|
| C1 | No upward imports (L2 depends only on L1) | PASS | All `use crate::` reference L1 types only |
| C2 | All trait methods `&self` | PASS | 65 trait methods, all `&self` with interior mutability |
| C3 | TensorContributor impl on every manager | PASS | 4/4 managers implement trait |
| C4 | Zero tolerance (0 unsafe, 0 unwrap, 0 expect) | PASS | No occurrences in any file |
| C5 | No chrono/SystemTime — only Timestamp/Duration | PASS | All temporal values are L1 types |
| C6 | Signal emission via Arc<SignalBus> | PASS | All state transitions emit signals |
| C7 | Owned returns through RwLock | PASS | All lock-crossing returns are cloned/owned |
| C8 | Duration not milliseconds | PASS | All timeouts use `std::time::Duration` |
| C9 | Backward compatibility via re-exports | PASS | mod.rs re-exports all public types |
| C10 | 280+ tests | PASS | 320 tests (279 unit + 41 integration) |

---

## 4. Tensor Contribution Map

L2 covers **8 of 12 dimensions** across its 4 modules:

| Dimension | Index | Module | Formula | Range |
|-----------|-------|--------|---------|-------|
| D0: service_id | 0 | M09 | `service_count / 12.0` | [0.0, 1.0] |
| D2: tier | 2 | M09 | `avg(tier.normalized())` | [0.0, 1.0] |
| D3: deps | 3 | M09 | `avg(dep_count) / 12.0` | [0.0, 1.0] |
| D4: agents | 4 | M09 | `healthy_count / total_count` | [0.0, 1.0] |
| D6: health | 6 | M10, M11 | M10: `aggregate_health()`, M11: `fraction_running` | [0.0, 1.0] |
| D7: uptime | 7 | M11 | `1.0 - avg(restart_count / max_restarts)` | [0.0, 1.0] |
| D9: latency | 9 | M12 | `closed_fraction()` (circuit breakers) | [0.0, 1.0] |
| D10: error_rate | 10 | M10, M12 | M10: `1.0 - aggregate_health`, M12: `avg_failure_rate` | [0.0, 1.0] |

**Uncovered dimensions:** D1 (port), D5 (protocol), D8 (synergy), D11 (temporal) — provided by other layers.

---

## 5. Signal Topology (C6)

All signal emission flows through `Arc<SignalBus>` from L1:

| Source Module | Trigger Event | Signal Type |
|--------------|---------------|-------------|
| M09 ServiceRegistry | `update_health()` transition | `HealthSignal` |
| M10 HealthMonitor | Status FSM transition | `HealthSignal` |
| M11 LifecycleManager | State transition (start/stop/fail/restart) | `HealthSignal` (score change) |
| M12 CircuitBreakerRegistry | Circuit state transition | `HealthSignal` |

**Emission pattern:** Signal is emitted only when the health score *changes*, not on every call.

---

## 6. Interior Mutability Architecture

All L2 managers use `parking_lot::RwLock` for concurrent access:

```
┌─────────────────────────────────────────────────┐
│                 L2 Manager                       │
│  ┌─────────────────────────────────────────┐    │
│  │  RwLock<InternalState>                  │    │
│  │  ├── HashMap<String, Entry>             │    │
│  │  └── metadata (counters, config)        │    │
│  └─────────────────────────────────────────┘    │
│  ┌─────────────────────────────────────────┐    │
│  │  Option<Arc<SignalBus>>    (C6)         │    │
│  └─────────────────────────────────────────┘    │
│  ┌─────────────────────────────────────────┐    │
│  │  Option<Arc<MetricsRegistry>>           │    │
│  └─────────────────────────────────────────┘    │
└─────────────────────────────────────────────────┘
```

**Read path:** `state.read()` → clone data → release lock → return owned
**Write path:** `state.write()` → mutate → emit signal → release lock → return result

---

## 7. Re-Export Topology (F25)

`mod.rs` re-exports all public types for downstream consumers:

```rust
// From service_registry
pub use service_registry::{ServiceDefinition, ServiceDefinitionBuilder,
                           ServiceDiscovery, ServiceRegistry};

// From health_monitor
pub use health_monitor::{HealthCheckResult, HealthMonitor, HealthMonitoring,
                         HealthProbe, HealthProbeBuilder};

// From lifecycle
pub use lifecycle::{LifecycleAction, LifecycleEntry, LifecycleEntryBuilder,
                    LifecycleManager, LifecycleOps, LifecycleTransition};

// From resilience
pub use resilience::{CircuitBreakerConfig, CircuitBreakerConfigBuilder,
                     CircuitBreakerOps, CircuitBreakerRegistry,
                     CircuitBreakerStats, CircuitStateTransition,
                     Endpoint, LoadBalanceAlgorithm, LoadBalancer,
                     LoadBalancing, PoolStats, ResilienceManager};
```

**Downstream import pattern:** `use crate::m2_services::{ServiceRegistry, HealthMonitor, ...};`

---

## 8. Error Taxonomy (F07)

All L2 modules use the unified `Error` type from L1 Foundation. Error construction patterns:

| Module | Error Scenarios | Construction |
|--------|----------------|--------------|
| M09 | Duplicate service, not found, self-dependency | `Error::new(ErrorKind::NotFound, msg)` |
| M10 | Duplicate probe, unknown service, validation failure | `Error::new(ErrorKind::InvalidInput, msg)` |
| M11 | Invalid transition, not registered, restart limit | `Error::new(ErrorKind::InvalidState, msg)` |
| M12 | Duplicate breaker, no healthy endpoints, pool not found | `Error::new(ErrorKind::NotFound, msg)` |

**No module defines its own error enum** — all errors flow through L1's `Error` type.

---

## 9. Configuration Surfaces (F19)

| Config Struct | Module | Tunable Fields | Defaults |
|--------------|--------|---------------|----------|
| `RestartConfig` | mod.rs/M11 | max_restarts, initial_backoff, max_backoff | 5, 1s, 30s |
| `CircuitBreakerConfig` | M12 | failure_threshold, success_threshold, open_timeout, half_open_max_requests, monitoring_window | 5, 3, 30s, 1, 60s |
| `HealthProbe` | M10 | interval_ms, timeout_ms, healthy_threshold, unhealthy_threshold | per-registration |

---

## 10. Constants & Thresholds (F20)

| Constant | Module | Value | Purpose |
|----------|--------|-------|---------|
| Default max_restarts | mod.rs | 5 | Restart ceiling |
| Default initial_backoff | mod.rs | 1s | First retry delay |
| Default max_backoff | mod.rs | 30s | Maximum retry delay (exponential cap) |
| Default failure_threshold | M12 | 5 | Failures before Open |
| Default success_threshold | M12 | 3 | Successes to close from HalfOpen |
| Default open_timeout | M12 | 30s | Time before probing |
| Default monitoring_window | M12 | 60s | Stats window |
| Default max_history | M10 | 100 | Health check result ring buffer |
| ULTRAPLATE_SERVICE_COUNT | M09 | 12 | Canonical fleet size |
| Weight clamp range | M12 | [0.0, 1.0] | Endpoint weight bounds |

---

## 11. Builder Patterns (F08)

| Builder | Target | Required Fields | Validation |
|---------|--------|----------------|------------|
| `ServiceDefinitionBuilder` | `ServiceDefinition` | service_id, name | None (defaults for port/tier) |
| `HealthProbeBuilder` | `HealthProbe` | service_id, endpoint | thresholds > 0, timeout <= interval |
| `LifecycleEntryBuilder` | `LifecycleEntry` | service_id, name | None |
| `CircuitBreakerConfigBuilder` | `CircuitBreakerConfig` | None (all defaults) | None |

**Pattern:** All builders use fluent `.field(value)` chaining → `.build() -> Result<T>` or `.build() -> T`.

---

## 12. Test Architecture (F14)

### Test Distribution

| Module | Unit Tests | Category Focus |
|--------|-----------|----------------|
| mod.rs | 20 | Enum variants, Display, Default, hash_to_float |
| M09 | 53 | Registration CRUD, discovery filters, dependency graph, ULTRAPLATE bootstrap |
| M10 | 49 | Probe registration, FSM transitions, threshold crossing, aggregation |
| M11 | 75 | FSM transitions (7 valid + invalid rejection), restart backoff, history |
| M12 | 82 | Circuit breaker FSM, load balancing (4 algorithms), pool stats, distribution |

### Test Patterns

- **Trait object safety:** Every trait tested with `fn _assert_trait_object(_: &dyn TraitName) {}`
- **Send + Sync:** Explicit `fn _assert_send_sync<T: Send + Sync>() {}` for all managers
- **Arc<dyn Trait>:** Verified for all 6 traits
- **Signal emission:** Tests with `SignalBus` verify signals are emitted on transitions
- **Tensor contribution:** All 4 managers tested for dimension values in [0.0, 1.0]

---

## 13. Cross-Module Data Flow

```
External Request
      │
      ▼
┌─────────────┐     register      ┌──────────────┐
│ M09 Registry │◄────────────────│ Bootstrap     │
│ (discovery)  │                  │ (startup)     │
└──────┬───────┘                  └──────────────┘
       │ discover()
       ▼
┌─────────────┐     record_result  ┌──────────────┐
│ M10 Health   │◄─────────────────│ Health Probe  │
│ (monitoring) │                   │ (external)    │
└──────┬───────┘                   └──────────────┘
       │ status transition
       ▼
┌─────────────┐     state change   ┌──────────────┐
│ M11 Lifecycle│◄─────────────────│ Orchestrator  │
│ (FSM)        │                   │ (L3+)        │
└──────┬───────┘                   └──────────────┘
       │ failure detected
       ▼
┌──────────────┐    allow_request   ┌──────────────┐
│ M12 Resilience│◄─────────────────│ Request Path  │
│ (protection)  │                   │ (L3+)        │
└──────────────┘                   └──────────────┘
```

---

## 14. ULTRAPLATE Service Bootstrap

M09 provides `register_ultraplate_services()` which pre-registers all 12 canonical services:

| Service ID | Port | Tier | Protocol |
|-----------|------|------|----------|
| maintenance-engine | 8080 | Tier1 | REST |
| devops-engine | 8081 | Tier1 | REST |
| synthex | 8090 | Tier2 | REST |
| san-k7-orchestrator | 8100 | Tier2 | REST |
| nais | 8101 | Tier3 | REST |
| bash-engine | 8102 | Tier3 | REST |
| tool-maker | 8103 | Tier3 | REST |
| claude-context-manager | 8104 | Tier4 | REST |
| tool-library | 8105 | Tier4 | REST |
| codesynthor-v7 | 8110 | Tier2 | REST |
| sphere-vortex | 8120 | Tier4 | REST |
| library-agent | 8083 | Tier5 | REST |

**Tier distribution:** 2x Tier1, 3x Tier2, 3x Tier3, 3x Tier4, 1x Tier5

---

## Version History

| Version | Date | Changes |
|---------|------|---------|
| 1.0.0 | 2026-03-01 | Initial 30-facet extraction from m2_services source |

---

*Generated: 2026-03-01 | The Maintenance Engine v1.0.0 | 30-Facet Taxonomy*
