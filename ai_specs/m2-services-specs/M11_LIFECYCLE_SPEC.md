# M11 LIFECYCLE SPECIFICATION

> Technical specification for The Maintenance Engine v1.0.0 — Module M11
> 30-Facet Taxonomy Extraction | Generated: 2026-03-01

---

## Overview

- **Module ID:** M11
- **Layer:** L2 Services
- **File:** `src/m2_services/lifecycle.rs`
- **LOC:** 1,898
- **Tests:** 75 unit tests
- **Status:** COMPLETE
- **Primary Trait:** `LifecycleOps` (18 methods)
- **Purpose:** Service state machine management with exponential backoff restarts, transition history, and fleet-wide lifecycle coordination

---

## 1. Public API Surface (F01)

### Trait: LifecycleOps

```rust
pub trait LifecycleOps: Send + Sync + fmt::Debug {
    // === Registration ===

    /// Register a new service for lifecycle management.
    /// Initial state: Stopped. Errors on duplicate.
    fn register(&self, service_id: &str, name: &str,
                tier: ServiceTier, config: RestartConfig) -> Result<()>;

    /// Remove a service from lifecycle management.
    fn deregister(&self, service_id: &str) -> Result<()>;

    // === State Transitions ===

    /// Stopped|Failed → Starting. Errors if already Starting/Running/Stopping.
    fn start_service(&self, service_id: &str) -> Result<()>;

    /// Starting → Running. Errors if not Starting.
    fn mark_running(&self, service_id: &str) -> Result<()>;

    /// Starting|Running → Failed. Errors if Stopped/Stopping.
    fn mark_failed(&self, service_id: &str) -> Result<()>;

    /// Running → Stopping. Errors if not Running.
    fn stop_service(&self, service_id: &str) -> Result<()>;

    /// Stopping → Stopped. Resets restart count. Errors if not Stopping.
    fn mark_stopped(&self, service_id: &str) -> Result<()>;

    /// Running|Failed → Starting. Increments restart_count,
    /// doubles backoff (capped at max_backoff). Returns backoff Duration.
    fn restart_service(&self, service_id: &str) -> Result<Duration>;

    // === Queries ===

    /// Current status. Owned return (C7).
    fn get_status(&self, service_id: &str) -> Result<ServiceStatus>;

    /// Full lifecycle entry clone. Owned return (C7).
    fn get_entry(&self, service_id: &str) -> Result<LifecycleEntry>;

    /// Transition history. Owned Vec (C7).
    fn get_history(&self, service_id: &str) -> Result<Vec<LifecycleTransition>>;

    /// Whether restart_count < max_restarts.
    fn can_restart(&self, service_id: &str) -> Result<bool>;

    /// Current backoff duration.
    fn get_restart_backoff(&self, service_id: &str) -> Result<Duration>;

    /// Check registration.
    fn is_registered(&self, service_id: &str) -> bool;

    /// Count of managed services.
    fn service_count(&self) -> usize;

    /// All service IDs in Running state.
    fn get_all_running(&self) -> Vec<String>;

    /// All service IDs in Failed state.
    fn get_all_failed(&self) -> Vec<String>;

    /// Reset restart_count to 0 and backoff to initial.
    fn reset_restart_count(&self, service_id: &str) -> Result<()>;
}
```

---

## 2. Service Lifecycle FSM (F03)

### Valid Transitions (7)

```
                 start_service()          mark_running()
  ┌─────────┐ ──────────────────► ┌──────────┐ ──────────────► ┌─────────┐
  │ Stopped │                     │ Starting │                  │ Running │
  └─────────┘                     └──────────┘                  └─────────┘
       ▲                               │                             │
       │ mark_stopped()                │ mark_failed()              │ stop_service()
       │                               ▼                             ▼
  ┌──────────┐                    ┌──────────┐                 ┌──────────┐
  │ Stopping │ ◄──────────────── │  Failed  │                 │ Stopping │
  └──────────┘   (not direct)    └──────────┘                 └──────────┘
                                      │                             │
                                      │ start_service()            │ mark_stopped()
                                      ▼                             ▼
                                 ┌──────────┐                 ┌─────────┐
                                 │ Starting │                 │ Stopped │
                                 └──────────┘                 └─────────┘
```

### Transition Table

| From | To | Method | Side Effects |
|------|----|--------|-------------|
| Stopped | Starting | `start_service()` | Record transition |
| Starting | Running | `mark_running()` | Record transition |
| Starting | Failed | `mark_failed()` | Record transition |
| Running | Stopping | `stop_service()` | Record transition |
| Running | Failed | `mark_failed()` | Record transition |
| Stopping | Stopped | `mark_stopped()` | Reset restart count, record transition |
| Failed | Starting | `start_service()` | Record transition |
| Running/Failed | Starting | `restart_service()` | Increment restart_count, double backoff, record transition |

### Invalid Transitions (All rejected with Error)

| From | Attempted | Rejection Reason |
|------|-----------|-----------------|
| Starting | start_service() | Already starting |
| Running | start_service() | Already running |
| Stopping | start_service() | Must complete stop first |
| Stopped | mark_running() | Must start first |
| Stopped | stop_service() | Already stopped |
| Failed | stop_service() | Must restart, not stop |
| Stopped | mark_failed() | Cannot fail when stopped |

### Validation Function

```rust
/// Const function — compile-time FSM validation
pub const fn is_valid_transition(from: ServiceStatus, to: ServiceStatus) -> bool {
    matches!((from, to),
        (Stopped, Starting) |
        (Starting, Running) |
        (Starting, Failed) |
        (Running, Stopping) |
        (Running, Failed) |
        (Stopping, Stopped) |
        (Failed, Starting)
    )
}
```

---

## 3. Restart Backoff Algorithm

### Exponential Backoff with Cap

```
backoff(n) = min(initial_backoff * 2^n, max_backoff)
```

| Restart # | Backoff (defaults) | Calculation |
|-----------|--------------------|-------------|
| 0 | 1s | 1s * 2^0 |
| 1 | 2s | 1s * 2^1 |
| 2 | 4s | 1s * 2^2 |
| 3 | 8s | 1s * 2^3 |
| 4 | 16s | 1s * 2^4 |
| 5 | REJECTED | restart_count >= max_restarts |

### Restart Flow

```rust
fn restart_service(&self, service_id: &str) -> Result<Duration> {
    // 1. Validate: must be Running or Failed
    // 2. Check: restart_count < max_restarts
    // 3. Transition: current → Starting
    // 4. Increment: restart_count += 1
    // 5. Double: current_backoff = min(current_backoff * 2, max_backoff)
    // 6. Record: transition in history
    // 7. Emit: signal if health score changed
    // 8. Return: current_backoff Duration
}
```

### Reset Conditions

- `mark_stopped()` — Resets restart_count to 0, backoff to initial
- `reset_restart_count()` — Manual reset (e.g., after successful long-running period)

---

## 4. Data Structures (F08, F09)

### LifecycleAction

```rust
#[derive(Clone, Debug)]
pub enum LifecycleAction {
    Start { service_id: String },
    Stop { service_id: String, graceful: bool },
    Restart { service_id: String, reason: String },
    HealthCheck { service_id: String },
}
```

**Display:** Human-readable action description.

### LifecycleTransition

```rust
#[derive(Clone, Debug)]
pub struct LifecycleTransition {
    pub from: ServiceStatus,
    pub to: ServiceStatus,
    pub reason: String,
    pub timestamp: Timestamp,  // C5
}
```

**Display:** `"Transition: {from} → {to} ({reason})"`

### LifecycleEntry

```rust
#[derive(Clone, Debug)]
pub struct LifecycleEntry {
    pub service_id: String,
    pub name: String,
    pub tier: ServiceTier,
    pub current_state: ServiceStatus,
    pub previous_state: Option<ServiceStatus>,
    pub transition_history: Vec<LifecycleTransition>,
    pub restart_count: u32,
    pub config: RestartConfig,
    pub current_backoff: Duration,
    pub created_at: Timestamp,         // C5
    pub last_transition: Timestamp,    // C5
}
```

**Display:** `"{name} ({service_id}): {current_state} [restarts: {restart_count}/{max}]"`

### LifecycleEntryBuilder

Fluent API for constructing `LifecycleEntry` with defaults:
```rust
LifecycleEntryBuilder::new()
    .service_id("synthex")
    .name("SYNTHEX Engine")
    .tier(ServiceTier::Tier2)
    .config(RestartConfig::default())
    .build()
```

---

## 5. Internal Architecture (F13)

### LifecycleManager

```rust
#[derive(Debug)]
pub struct LifecycleManager {
    services: RwLock<HashMap<String, LifecycleEntry>>,
    signal_bus: Option<Arc<SignalBus>>,
    metrics: Option<Arc<MetricsRegistry>>,
}
```

**Constructors:**
- `new()` / `default()` — Empty
- `with_signal_bus(bus: Arc<SignalBus>)` — With signal emission
- `with_signal_bus_and_metrics(bus, metrics)` — Full instrumentation

---

## 6. Tensor Contribution (F06)

```rust
impl TensorContributor for LifecycleManager {
    fn contribute(&self) -> ContributedTensor {
        // D6: health = fraction_running (running_count / total_count)
        // D7: uptime = 1.0 - avg(restart_count / max_restarts)
        ContributedTensor {
            dimensions: [(D6, fraction_running), (D7, uptime_proxy)],
            coverage: CoverageBitmap::from([6, 7]),
            source: ModuleId::M11,
        }
    }
}
```

| Dimension | Index | Formula | Interpretation |
|-----------|-------|---------|---------------|
| health | D6 | `running / total` | Fleet running ratio (1.0 = all running) |
| uptime | D7 | `1.0 - avg(restart_count / max_restarts)` | Stability proxy (1.0 = no restarts) |

### D6 Overlap with M10

Both M10 and M11 contribute to D6 (health). The tensor merging logic in L1 handles this via coverage bitmap — the dimension with higher confidence (more data points) takes precedence, or they are averaged.

---

## 7. Signal Emission (F05)

| Trigger | Condition | Signal |
|---------|-----------|--------|
| `apply_transition()` | Health score changes | `HealthSignal { service_id, score }` |

**Health score function:**
```rust
const fn status_health_score(status: ServiceStatus) -> f64 {
    match status {
        Running => 1.0,
        Starting | Stopping => 0.5,
        Stopped | Failed => 0.0,
    }
}
```

Signal emitted when `status_health_score(from) != status_health_score(to)`.

---

## 8. Helper Functions (F18)

```rust
/// Validate FSM transition. Compile-time evaluable.
pub const fn is_valid_transition(from: ServiceStatus, to: ServiceStatus) -> bool;

/// Map status to health score. Compile-time evaluable.
pub const fn status_health_score(status: ServiceStatus) -> f64;
```

Both functions are `const fn` — usable in const contexts and guaranteed zero-cost.

---

## 9. Error Taxonomy (F07)

| Error Scenario | ErrorKind | Message Pattern |
|---------------|-----------|-----------------|
| Register duplicate | `AlreadyExists` | "Service '{id}' already registered" |
| Invalid transition | `InvalidState` | "Cannot transition {from} → {to}" |
| Not registered | `NotFound` | "Service '{id}' not registered" |
| Restart limit reached | `ResourceExhausted` | "Service '{id}' exceeded max restarts ({n})" |
| Deregister not found | `NotFound` | "Service '{id}' not registered" |

---

## 10. Concurrency Model (F04)

- **Lock:** `parking_lot::RwLock<HashMap<String, LifecycleEntry>>`
- **All trait methods:** `&self` with interior mutability
- **Thread safety:** `LifecycleManager: Send + Sync`
- **Object safety:** `dyn LifecycleOps` is object-safe
- **History trimming:** Configurable max (prevents unbounded growth)

---

## 11. Test Architecture (F14)

### Test Categories (75 tests)

| Category | Count | Focus |
|----------|-------|-------|
| Trait object safety | 3 | dyn LifecycleOps, Send+Sync, Arc<dyn> |
| Registration | 6 | Register, deregister, duplicate, counting |
| FSM happy path | 7 | All 7 valid transitions |
| FSM rejection | 8 | All invalid transition pairs |
| Restart mechanics | 10 | Backoff doubling, counter increment, limit |
| History | 5 | Recording, transition list, trimming |
| Entry construction | 5 | Builder, defaults, Display |
| Fleet queries | 4 | get_all_running, get_all_failed |
| Status queries | 4 | get_status, get_entry, is_registered |
| Backoff computation | 5 | Exponential growth, cap at max_backoff |
| Reset | 3 | reset_restart_count, mark_stopped reset |
| Signal emission | 4 | Transition signals, score change detection |
| Tensor contribution | 4 | D6, D7, coverage, empty manager |
| LifecycleAction | 3 | Variants, Display |
| Helper functions | 4 | is_valid_transition, status_health_score |

---

## 12. Key Implementation Details

### Graceful Shutdown Sequence

The expected shutdown flow is:
1. `stop_service()` — Running → Stopping
2. (external: wait for in-flight requests to drain)
3. `mark_stopped()` — Stopping → Stopped (resets restart count)

Calling `mark_failed()` from Stopping is invalid — if a service fails during shutdown, it must complete the stop first.

### Restart vs Start

- `start_service()` — For cold starts from Stopped or Failed. Does NOT increment restart_count.
- `restart_service()` — For hot restarts from Running or Failed. Increments restart_count and applies backoff.

This distinction allows manual restart count management: an operator can `stop_service()` + `start_service()` to bypass the restart counter.

### History Trimming

Transition history is stored as a Vec. When it exceeds a configurable maximum, the oldest entries are dropped. This prevents memory growth in long-running services with many restarts.

---

## Version History

| Version | Date | Changes |
|---------|------|---------|
| 1.0.0 | 2026-03-01 | Initial 30-facet extraction |

---

*Generated: 2026-03-01 | The Maintenance Engine v1.0.0 | Module M11*
