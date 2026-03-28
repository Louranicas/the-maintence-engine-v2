# M10 HEALTH MONITOR SPECIFICATION

> Technical specification for The Maintenance Engine v1.0.0 — Module M10
> 30-Facet Taxonomy Extraction | Generated: 2026-03-01

---

## Overview

- **Module ID:** M10
- **Layer:** L2 Services
- **File:** `src/m2_services/health_monitor.rs`
- **LOC:** 1,130
- **Tests:** 49 unit tests
- **Status:** COMPLETE
- **Primary Trait:** `HealthMonitoring` (11 methods)
- **Purpose:** Health check probe management, FSM-based status tracking, threshold-driven transitions, fleet-wide aggregation

---

## 1. Public API Surface (F01)

### Trait: HealthMonitoring

```rust
pub trait HealthMonitoring: Send + Sync + fmt::Debug {
    /// Register a health probe configuration. Errors on duplicate.
    fn register_probe(&self, probe: HealthProbe) -> Result<()>;

    /// Remove a probe by service ID. Errors if not found.
    fn unregister_probe(&self, service_id: &str) -> Result<()>;

    /// Count of registered probes.
    fn probe_count(&self) -> usize;

    /// Record a health check result. Drives FSM transitions.
    /// Emits signal on status change (C6).
    fn record_result(&self, service_id: &str, result: HealthCheckResult) -> Result<()>;

    /// Get current health status for a service. Owned return (C7).
    fn get_status(&self, service_id: &str) -> Result<HealthStatus>;

    /// Get full result history for a service. Owned Vec (C7).
    fn get_history(&self, service_id: &str) -> Result<Vec<HealthCheckResult>>;

    /// Get all statuses as a snapshot. Owned HashMap (C7).
    fn get_all_statuses(&self) -> HashMap<String, HealthStatus>;

    /// Weighted average health across all probes. Range: [0.0, 1.0].
    fn aggregate_health(&self) -> f64;

    /// List service IDs with Degraded status.
    fn get_degraded_services(&self) -> Vec<String>;

    /// List service IDs with Unhealthy status.
    fn get_unhealthy_services(&self) -> Vec<String>;

    /// List service IDs with Healthy status.
    fn get_healthy_services(&self) -> Vec<String>;
}
```

---

## 2. Data Structures (F08, F09)

### HealthProbe

```rust
#[derive(Clone, Debug)]
pub struct HealthProbe {
    pub service_id: String,
    pub endpoint: String,           // e.g., "http://localhost:8090/api/health"
    pub interval_ms: u64,           // check frequency
    pub timeout_ms: u64,            // per-check timeout
    pub healthy_threshold: u32,     // consecutive successes for Unknown→Healthy
    pub unhealthy_threshold: u32,   // consecutive failures for Unknown→Unhealthy
}
```

**Display:** `"Probe({service_id} @ {endpoint}, interval={interval_ms}ms)"`

### HealthProbeBuilder

```rust
impl HealthProbeBuilder {
    pub fn new() -> Self;
    pub fn service_id(mut self, id: impl Into<String>) -> Self;
    pub fn endpoint(mut self, ep: impl Into<String>) -> Self;
    pub fn interval_ms(mut self, ms: u64) -> Self;
    pub fn timeout_ms(mut self, ms: u64) -> Self;
    pub fn healthy_threshold(mut self, t: u32) -> Self;
    pub fn unhealthy_threshold(mut self, t: u32) -> Self;
    pub fn build(self) -> Result<HealthProbe>;
}
```

**Validation on build():**
- `healthy_threshold > 0`
- `unhealthy_threshold > 0`
- `timeout_ms <= interval_ms`

### HealthCheckResult

```rust
#[derive(Clone, Debug)]
pub struct HealthCheckResult {
    pub service_id: String,
    pub status: HealthStatus,
    pub response_time_ms: u64,
    pub timestamp: Timestamp,       // C5
    pub message: Option<String>,
    pub status_code: Option<u16>,
}

impl HealthCheckResult {
    /// Construct a success result.
    pub fn success(service_id: impl Into<String>, response_time_ms: u64) -> Self;

    /// Construct a failure result with message.
    pub fn failure(service_id: impl Into<String>, message: impl Into<String>) -> Self;

    /// Whether this result indicates success.
    pub fn is_success(&self) -> bool;
}
```

**Display:** `"HealthCheck({service_id}: {status}, {response_time_ms}ms)"`

---

## 3. Health Status FSM (F03)

The health monitor implements a finite state machine with threshold-driven transitions:

```
                    consecutive_successes >= healthy_threshold
     ┌──────────┐ ──────────────────────────────────────────► ┌──────────┐
     │ Unknown  │                                              │ Healthy  │
     └──────────┘ ◄──────────────────────────────────────────  └──────────┘
                    (initial state only)                            │
                                                          single failure
     ┌──────────┐                                              │
     │Unhealthy │ ◄──────── consecutive_failures ─────────  ┌──▼───────┐
     └──────────┘    >= unhealthy_threshold                 │ Degraded │
                                                            └──────────┘
```

### Transition Rules

| From | Event | Condition | To |
|------|-------|-----------|-----|
| Unknown | Success | `consecutive_successes >= healthy_threshold` | Healthy |
| Unknown | Failure | `consecutive_failures >= unhealthy_threshold` | Unhealthy |
| Healthy | Failure | Single failure | Degraded |
| Degraded | Failure | `consecutive_failures >= unhealthy_threshold` | Unhealthy |
| Degraded | Success | `consecutive_successes >= healthy_threshold` | Healthy |
| Unhealthy | Success | `consecutive_successes >= healthy_threshold` | Healthy |

### Counter Reset Behavior

- On success: `consecutive_failures = 0`, `consecutive_successes += 1`
- On failure: `consecutive_successes = 0`, `consecutive_failures += 1`
- Counters reset on opposite outcome (hysteresis prevention)

---

## 4. Internal Architecture (F13)

### HealthMonitor

```rust
#[derive(Debug)]
pub struct HealthMonitor {
    state: RwLock<MonitorState>,
    signal_bus: Option<Arc<SignalBus>>,
    metrics: Option<Arc<MetricsRegistry>>,
}

struct MonitorState {
    services: HashMap<String, ServiceHealthState>,
    max_history: usize,  // default: 100
}

struct ServiceHealthState {
    probe: HealthProbe,
    current_status: HealthStatus,           // FSM state
    consecutive_successes: u32,
    consecutive_failures: u32,
    history: Vec<HealthCheckResult>,        // ring buffer (max_history)
}
```

**Constructors:**
- `new()` / `default()` — max_history = 100
- `with_max_history(n: usize)` — Custom history size
- `with_signal_bus(bus: Arc<SignalBus>)` — With signal emission
- `with_metrics(metrics: Arc<MetricsRegistry>)` — With metrics

---

## 5. Tensor Contribution (F06)

```rust
impl TensorContributor for HealthMonitor {
    fn contribute(&self) -> ContributedTensor {
        // D6: health = aggregate_health()  (weighted avg)
        // D10: error_rate = 1.0 - aggregate_health()
        ContributedTensor {
            dimensions: [(D6, aggregate), (D10, 1.0 - aggregate)],
            coverage: CoverageBitmap::from([6, 10]),
            source: ModuleId::M10,
        }
    }
}
```

| Dimension | Index | Formula | Interpretation |
|-----------|-------|---------|---------------|
| health | D6 | `aggregate_health()` | Fleet-wide health (1.0 = all healthy) |
| error_rate | D10 | `1.0 - aggregate_health()` | Inverse health (0.0 = no errors) |

### Aggregation Algorithm

```
aggregate_health = Σ(status.score()) / probe_count
```

Where `score()` returns: Healthy=1.0, Degraded=0.5, Unhealthy=0.0, Unknown=0.0.

Empty monitor returns 0.0.

---

## 6. Signal Emission (F05)

| Trigger | Condition | Signal |
|---------|-----------|--------|
| `record_result()` | `previous_status != new_status` | `HealthSignal { service_id, score }` |

Signal is emitted **only on transitions**, not on every check result.

---

## 7. History Management

- **Storage:** Vec-based ring buffer per service
- **Max size:** `max_history` (default 100)
- **Trim strategy:** When history exceeds max, oldest entries are dropped (FIFO)
- **Access:** `get_history()` returns full owned Vec (C7 clone)

---

## 8. Error Taxonomy (F07)

| Error Scenario | ErrorKind | Message Pattern |
|---------------|-----------|-----------------|
| Register duplicate probe | `AlreadyExists` | "Probe for '{id}' already registered" |
| Record for unknown service | `NotFound` | "No probe registered for '{id}'" |
| Get status for unknown | `NotFound` | "No probe registered for '{id}'" |
| Get history for unknown | `NotFound` | "No probe registered for '{id}'" |
| Unregister unknown | `NotFound` | "No probe registered for '{id}'" |
| Builder: threshold = 0 | `InvalidInput` | "Thresholds must be > 0" |
| Builder: timeout > interval | `InvalidInput` | "Timeout must be <= interval" |

---

## 9. Concurrency Model (F04)

- **Lock:** `parking_lot::RwLock<MonitorState>`
- **Read path:** `state.read()` → clone status/history → release
- **Write path:** `state.write()` → update FSM → trim history → emit signal → release
- **Thread safety:** `HealthMonitor: Send + Sync`
- **Object safety:** `dyn HealthMonitoring` is object-safe

---

## 10. Service Status Partitioning

Three methods partition the fleet by health status:

```rust
// Returns disjoint sets covering all registered services
fn get_healthy_services(&self) -> Vec<String>;   // HealthStatus::Healthy
fn get_degraded_services(&self) -> Vec<String>;  // HealthStatus::Degraded
fn get_unhealthy_services(&self) -> Vec<String>; // HealthStatus::Unhealthy
// Note: Unknown services appear in none of these lists
```

---

## 11. Test Architecture (F14)

### Test Categories (49 tests)

| Category | Count | Focus |
|----------|-------|-------|
| Trait object safety | 2 | `dyn HealthMonitoring`, Send+Sync |
| Probe registration | 6 | Register, unregister, duplicate, counting |
| FSM transitions | 10 | All transition paths, threshold crossing |
| Counter reset | 4 | Consecutive counter hysteresis |
| History | 4 | Recording, trimming, retrieval |
| Aggregation | 5 | Empty, all healthy, mixed, weighted |
| Status partitioning | 4 | Healthy/degraded/unhealthy lists |
| Builder validation | 4 | Required fields, threshold>0, timeout<=interval |
| Signal emission | 3 | Transition signals, no signal on non-transition |
| Tensor contribution | 3 | D6, D10, coverage bitmap |
| Display/formatting | 4 | Result Display, Probe Display |

---

## 12. Key Implementation Details

### Degraded as Intermediate State

The `Degraded` state exists between `Healthy` and `Unhealthy`:
- A **single failure** drops from Healthy → Degraded (fast detection)
- **Multiple consecutive failures** (unhealthy_threshold) drops to Unhealthy (confirmed problem)
- This two-stage design prevents flapping: transient failures cause Degraded, sustained failures cause Unhealthy

### Health Score Quantization

Scores are quantized to three levels only (no continuous gradient):
- `Healthy = 1.0` (fully operational)
- `Degraded = 0.5` (partially impaired)
- `Unhealthy / Unknown = 0.0` (non-functional)

This simplifies aggregation and tensor contribution at the cost of granularity.

---

## Version History

| Version | Date | Changes |
|---------|------|---------|
| 1.0.0 | 2026-03-01 | Initial 30-facet extraction |

---

*Generated: 2026-03-01 | The Maintenance Engine v1.0.0 | Module M10*
