# M12 RESILIENCE SPECIFICATION

> Technical specification for The Maintenance Engine v1.0.0 — Module M12
> 30-Facet Taxonomy Extraction | Generated: 2026-03-01

---

## Overview

- **Module ID:** M12
- **Layer:** L2 Services
- **File:** `src/m2_services/resilience.rs`
- **LOC:** 2,189 (largest in L2)
- **Tests:** 82 unit tests
- **Status:** COMPLETE
- **Primary Traits:** `CircuitBreakerOps` (12 methods) + `LoadBalancing` (10 methods)
- **Purpose:** Fault isolation via circuit breakers + intelligent request distribution via load balancing. Two subsystems unified under `ResilienceManager` facade.

---

## 1. Public API Surface (F01)

### Trait: CircuitBreakerOps

```rust
pub trait CircuitBreakerOps: Send + Sync + fmt::Debug {
    /// Register a breaker with custom config. Errors on duplicate.
    fn register_breaker(&self, service_id: &str,
                        config: CircuitBreakerConfig) -> Result<()>;

    /// Register a breaker with default config.
    fn register_default(&self, service_id: &str) -> Result<()>;

    /// Remove a breaker. Errors if not found.
    fn deregister_breaker(&self, service_id: &str) -> Result<()>;

    /// Record a successful request. May transition HalfOpen→Closed.
    /// Returns new state.
    fn record_success(&self, service_id: &str) -> Result<CircuitState>;

    /// Record a failed request. May transition Closed→Open.
    /// Returns new state.
    fn record_failure(&self, service_id: &str) -> Result<CircuitState>;

    /// Check if a request should be allowed through.
    /// May transition Open→HalfOpen after timeout.
    fn allow_request(&self, service_id: &str) -> Result<bool>;

    /// Current circuit state.
    fn get_state(&self, service_id: &str) -> Result<CircuitState>;

    /// Full stats snapshot. Owned return (C7).
    fn get_breaker_stats(&self, service_id: &str) -> Result<CircuitBreakerStats>;

    /// Force reset to Closed state.
    fn reset(&self, service_id: &str) -> Result<()>;

    /// List all service IDs with Open circuits.
    fn get_open_circuits(&self) -> Vec<String>;

    /// Count of registered breakers.
    fn breaker_count(&self) -> usize;

    /// Check registration.
    fn is_registered(&self, service_id: &str) -> bool;
}
```

### Trait: LoadBalancing

```rust
pub trait LoadBalancing: Send + Sync + fmt::Debug {
    /// Create a new endpoint pool with given algorithm.
    fn create_pool(&self, service_id: &str,
                   algorithm: LoadBalanceAlgorithm) -> Result<()>;

    /// Remove a pool. Errors if not found.
    fn remove_pool(&self, service_id: &str) -> Result<()>;

    /// Add an endpoint to a pool. Errors on duplicate endpoint_id.
    fn add_endpoint(&self, service_id: &str, endpoint: Endpoint) -> Result<()>;

    /// Remove an endpoint from a pool.
    fn remove_endpoint(&self, service_id: &str, endpoint_id: &str) -> Result<()>;

    /// Select the next endpoint per algorithm. Increments active_connections.
    /// Returns owned clone (C7).
    fn select_endpoint(&self, service_id: &str) -> Result<Endpoint>;

    /// Mark an endpoint as healthy (eligible for selection).
    fn mark_healthy(&self, service_id: &str, endpoint_id: &str) -> Result<()>;

    /// Mark an endpoint as unhealthy (excluded from selection).
    fn mark_unhealthy(&self, service_id: &str, endpoint_id: &str) -> Result<()>;

    /// Record a completed request. Decrements active_connections.
    /// Records error if success=false.
    fn record_request(&self, service_id: &str, endpoint_id: &str,
                      success: bool) -> Result<()>;

    /// Aggregated pool statistics. Owned return (C7).
    fn get_pool_stats(&self, service_id: &str) -> Result<PoolStats>;

    /// Load distribution percentages per endpoint. Owned Vec (C7).
    fn get_load_distribution(&self, service_id: &str) -> Result<Vec<(String, f64)>>;
}
```

---

## 2. Circuit Breaker FSM (F03)

```
            failure_count >= failure_threshold
  ┌────────┐ ──────────────────────────────────► ┌────────┐
  │ Closed │                                      │  Open  │
  │(normal)│ ◄──────────────────────────────────  │(reject)│
  └────────┘  consecutive_successes >=            └────────┘
              success_threshold (from HalfOpen)        │
       ▲                                               │
       │                                    open_timeout elapsed
       │                                               │
       │         ┌──────────┐                          │
       └─────────│ HalfOpen │ ◄────────────────────────┘
                 │ (probe)  │
                 └──────────┘
                      │
                  failure → back to Open
```

### Transition Table

| From | Event | Condition | To |
|------|-------|-----------|-----|
| Closed | `record_failure()` | `failure_count >= failure_threshold` | Open |
| Closed | `record_failure()` | `failure_count < failure_threshold` | Closed |
| Closed | `record_success()` | always | Closed |
| Open | `allow_request()` | `elapsed >= open_timeout` | HalfOpen |
| Open | `allow_request()` | `elapsed < open_timeout` | Open (deny) |
| HalfOpen | `record_success()` | `consecutive_successes >= success_threshold` | Closed |
| HalfOpen | `record_success()` | `consecutive_successes < success_threshold` | HalfOpen |
| HalfOpen | `record_failure()` | any | Open (immediate) |

### Timeout Mechanism

Uses `std::time::Instant` (monotonic clock, C8) to track time since last state change:
```rust
fn is_open_timeout_elapsed(&self) -> bool {
    self.state_change_instant.elapsed() >= self.config.open_timeout
}
```

---

## 3. Load Balancing Algorithms (F03)

### LoadBalanceAlgorithm

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoadBalanceAlgorithm {
    RoundRobin,          // Sequential rotation through healthy endpoints
    WeightedRoundRobin,  // Weight-proportional distribution
    LeastConnections,    // Route to endpoint with fewest active connections
    Random,              // Deterministic hash-based (reproducible)
}
```

### Algorithm Details

#### RoundRobin

```
healthy_indices = [i for i in endpoints if endpoint[i].healthy]
selected = healthy_indices[current_index % len(healthy_indices)]
current_index += 1
```

#### WeightedRoundRobin

```
cumulative_weights = cumsum([ep.weight for ep in healthy_endpoints])
total = cumulative_weights[-1]
target = (selection_counter % (total * 100)) / 100.0
selected = first endpoint where cumulative_weight > target
```

#### LeastConnections

```
selected = healthy_endpoints.min_by(|ep| ep.active_connections)
// Tie-breaking: first in list order
```

#### Random (Deterministic)

```
// LCG-based hash from selection_counter (reproducible)
hash = (selection_counter * 6364136223846793005 + 1442695040888963407) >> 33
selected = healthy_indices[hash % len(healthy_indices)]
```

---

## 4. Data Structures (F08, F09)

### CircuitBreakerConfig

```rust
#[derive(Clone, Debug)]
pub struct CircuitBreakerConfig {
    pub failure_threshold: u32,        // default: 5
    pub success_threshold: u32,        // default: 3
    pub open_timeout: Duration,        // default: 30s (C8)
    pub half_open_max_requests: u32,   // default: 1
    pub monitoring_window: Duration,   // default: 60s (C8)
}
```

**Builder:** `CircuitBreakerConfigBuilder` with fluent API.

### CircuitBreakerStats

```rust
#[derive(Clone, Debug)]
pub struct CircuitBreakerStats {
    pub state: CircuitState,
    pub failure_count: u32,
    pub success_count: u32,
    pub total_requests: u64,
    pub total_failures: u64,
    pub failure_rate: f64,             // [0.0, 1.0]
    pub last_failure: Option<Timestamp>,    // C5
    pub last_state_change: Timestamp,       // C5
}
```

### CircuitStateTransition

```rust
#[derive(Clone, Debug)]
pub struct CircuitStateTransition {
    pub from: CircuitState,
    pub to: CircuitState,
    pub reason: String,
    pub timestamp: Timestamp,  // C5
}
```

**Display:** `"Circuit: {from} → {to} ({reason})"`

### Endpoint

```rust
#[derive(Clone, Debug)]
pub struct Endpoint {
    pub id: String,
    pub host: String,
    pub port: u16,
    pub weight: f64,                // [0.0, 1.0] — clamped on construction
    pub active_connections: u32,
    pub healthy: bool,              // default: true
    pub total_requests: u64,
    pub total_errors: u64,
}

impl Endpoint {
    pub fn new(id: impl Into<String>, host: impl Into<String>,
               port: u16, weight: f64) -> Self;
    pub fn error_rate(&self) -> f64;  // total_errors / total_requests (0.0 if no requests)
}
```

**Display:** `"{id} @ {host}:{port} (weight={weight}, conns={active_connections}, healthy={healthy})"`

### PoolStats

```rust
#[derive(Clone, Debug, Default)]
pub struct PoolStats {
    pub total_endpoints: usize,
    pub healthy_endpoints: usize,
    pub total_requests: u64,
    pub total_errors: u64,
    pub error_rate: f64,  // [0.0, 1.0]
}
```

---

## 5. Internal Architecture (F13)

### CircuitBreakerRegistry

```rust
#[derive(Debug)]
pub struct CircuitBreakerRegistry {
    breakers: RwLock<HashMap<String, CircuitBreakerEntry>>,
    signal_bus: Option<Arc<SignalBus>>,
    metrics: Option<Arc<MetricsRegistry>>,
}

/// Internal state per circuit breaker
struct CircuitBreakerEntry {
    config: CircuitBreakerConfig,
    state: CircuitState,
    failure_count: u32,
    success_count: u32,
    consecutive_successes: u32,       // for HalfOpen→Closed threshold
    total_requests: u64,
    total_failures: u64,
    last_failure_time: Option<Timestamp>,     // C5
    last_success_time: Option<Timestamp>,     // C5
    last_state_change: Timestamp,             // C5
    state_change_instant: Instant,            // C8: monotonic for timeout
    state_history: Vec<CircuitStateTransition>,
}
```

**Helper methods:**
- `closed_fraction() -> f64` — Fraction of breakers NOT in Open state
- `average_failure_rate() -> f64` — Mean failure_rate across all breakers

### LoadBalancer

```rust
#[derive(Debug, Default)]
pub struct LoadBalancer {
    pools: RwLock<HashMap<String, EndpointPool>>,
    metrics: Option<Arc<MetricsRegistry>>,
}

/// Internal pool state
struct EndpointPool {
    service_id: String,
    endpoints: Vec<Endpoint>,
    algorithm: LoadBalanceAlgorithm,
    current_index: usize,          // for RoundRobin
    selection_counter: u64,        // for Weighted/Random
}
```

### ResilienceManager (Facade)

```rust
#[derive(Debug, Default)]
pub struct ResilienceManager {
    circuit_breakers: CircuitBreakerRegistry,
    load_balancer: LoadBalancer,
}

impl ResilienceManager {
    pub fn new() -> Self;
    pub fn default() -> Self;
    pub fn with_signal_bus(bus: Arc<SignalBus>) -> Self;
    pub fn with_signal_bus_and_metrics(bus: Arc<SignalBus>,
                                       metrics: Arc<MetricsRegistry>) -> Self;

    /// Access circuit breaker subsystem.
    pub const fn circuit_breakers(&self) -> &CircuitBreakerRegistry;

    /// Access load balancer subsystem.
    pub const fn load_balancer(&self) -> &LoadBalancer;
}
```

---

## 6. Tensor Contribution (F06)

```rust
impl TensorContributor for ResilienceManager {
    fn contribute(&self) -> ContributedTensor {
        // D9: latency = closed_fraction() (fraction of non-Open circuits)
        // D10: error_rate = average_failure_rate() across all breakers
        ContributedTensor {
            dimensions: [(D9, closed_fraction), (D10, avg_failure_rate)],
            coverage: CoverageBitmap::from([9, 10]),
            source: ModuleId::M12,
        }
    }
}
```

| Dimension | Index | Formula | Interpretation |
|-----------|-------|---------|---------------|
| latency | D9 | `(total - open_count) / total` | Circuit health (1.0 = all closed) |
| error_rate | D10 | `avg(total_failures / total_requests)` | Request failure rate (0.0 = no failures) |

### D10 Overlap with M10

Both M10 and M12 contribute to D10. M10 provides health-based error rate, M12 provides request-based error rate. These complement each other: M10 measures service-level health, M12 measures request-level reliability.

---

## 7. Signal Emission (F05)

| Trigger | Condition | Signal |
|---------|-----------|--------|
| `record_failure()` → Open | State transition | `HealthSignal { service_id, score: 0.0 }` |
| `record_success()` → Closed | State transition | `HealthSignal { service_id, score: 1.0 }` |
| `allow_request()` → HalfOpen | State transition | `HealthSignal { service_id, score: 0.5 }` |

---

## 8. Configuration Surface (F19)

### CircuitBreakerConfig Defaults

| Parameter | Default | Range | Description |
|-----------|---------|-------|-------------|
| failure_threshold | 5 | 1+ | Failures before Open |
| success_threshold | 3 | 1+ | Successes to close from HalfOpen |
| open_timeout | 30s | Duration | Wait before probing |
| half_open_max_requests | 1 | 1+ | Concurrent probes in HalfOpen |
| monitoring_window | 60s | Duration | Stats reset window |

### Weight Clamping

Endpoint weights are clamped to `[0.0, 1.0]`:
```rust
weight: weight.clamp(0.0, 1.0)
```

---

## 9. Error Taxonomy (F07)

### Circuit Breaker Errors

| Error Scenario | ErrorKind | Message Pattern |
|---------------|-----------|-----------------|
| Register duplicate | `AlreadyExists` | "Breaker for '{id}' already registered" |
| Not registered | `NotFound` | "No breaker for '{id}'" |
| Request denied (Open) | `Unavailable` | "Circuit open for '{id}'" |

### Load Balancer Errors

| Error Scenario | ErrorKind | Message Pattern |
|---------------|-----------|-----------------|
| Pool not found | `NotFound` | "No pool for '{id}'" |
| Duplicate pool | `AlreadyExists` | "Pool for '{id}' already exists" |
| Duplicate endpoint | `AlreadyExists` | "Endpoint '{ep_id}' already in pool" |
| Endpoint not found | `NotFound` | "Endpoint '{ep_id}' not found in pool" |
| No healthy endpoints | `Unavailable` | "No healthy endpoints for '{id}'" |

---

## 10. Concurrency Model (F04)

- **Circuit Breaker Lock:** `parking_lot::RwLock<HashMap<String, CircuitBreakerEntry>>`
- **Load Balancer Lock:** `parking_lot::RwLock<HashMap<String, EndpointPool>>`
- **Two independent locks** — circuit breaker and load balancer operations do not contend
- **Thread safety:** All types are `Send + Sync`
- **Object safety:** Both `dyn CircuitBreakerOps` and `dyn LoadBalancing` are object-safe
- **active_connections tracking:** Incremented on `select_endpoint()`, decremented on `record_request()`

---

## 11. Active Connections Lifecycle

```
select_endpoint() ──► active_connections += 1 ──► (request in flight)
                                                        │
                                                        ▼
record_request(success) ──► active_connections -= 1, total_requests += 1
record_request(failure) ──► active_connections -= 1, total_requests += 1,
                            total_errors += 1
```

**Invariant:** Every `select_endpoint()` must be paired with a `record_request()` to prevent connection counter leaks.

---

## 12. Test Architecture (F14)

### Test Categories (82 tests)

| Category | Count | Focus |
|----------|-------|-------|
| Trait object safety | 4 | dyn CircuitBreakerOps, dyn LoadBalancing, Send+Sync |
| Config builder | 4 | Defaults, custom params, builder chain |
| Circuit FSM | 12 | Closed→Open→HalfOpen→Closed, all paths |
| Threshold crossing | 6 | Exact threshold, below threshold, boundary |
| Open timeout | 4 | Timeout elapsed, not elapsed, monotonic |
| Pool operations | 8 | Create, remove, add/remove endpoints |
| RoundRobin | 5 | Sequential selection, wrap-around, skip unhealthy |
| WeightedRoundRobin | 4 | Weight-proportional, edge weights |
| LeastConnections | 4 | Min selection, tie-breaking |
| Random | 3 | Deterministic, distribution |
| Health marking | 4 | Healthy/unhealthy, selection exclusion |
| Request recording | 4 | active_connections tracking, error recording |
| Pool stats | 4 | Aggregation, error_rate computation |
| Load distribution | 3 | Percentage computation, normalization |
| Signal emission | 4 | Circuit state transition signals |
| Tensor contribution | 4 | D9, D10, coverage, empty manager |
| ResilienceManager | 5 | Facade accessors, construction, defaults |
| Display | 4 | Endpoint Display, Stats Display, Config Display |

---

## 13. Key Implementation Details

### Two-Clock Strategy

The circuit breaker uses **two time sources**:
1. `Timestamp` (C5) — For recording history, stats, logging (wall clock)
2. `Instant` (C8) — For timeout calculation (monotonic, immune to clock adjustments)

This ensures timeout decisions are never affected by NTP jumps or manual clock changes.

### Failure Rate Calculation

```rust
fn failure_rate(&self) -> f64 {
    if self.total_requests == 0 { 0.0 }
    else { self.total_failures as f64 / self.total_requests as f64 }
}
```

### Deterministic "Random"

The Random algorithm uses a Linear Congruential Generator (LCG) seeded from `selection_counter`:
```rust
hash = (counter * 6364136223846793005 + 1442695040888963407) >> 33
```

This provides uniform distribution while being fully deterministic and reproducible — essential for testing and debugging.

---

## Version History

| Version | Date | Changes |
|---------|------|---------|
| 1.0.0 | 2026-03-01 | Initial 30-facet extraction |

---

*Generated: 2026-03-01 | The Maintenance Engine v1.0.0 | Module M12*
