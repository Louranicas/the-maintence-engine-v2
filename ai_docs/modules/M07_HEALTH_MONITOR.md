# Module M07: Health Monitor

> **M07_HEALTH_MONITOR** | Service Health Surveillance | Layer: L2 Services | [Back to Index](../INDEX.md)

---

## Navigation

| Direction | Link |
|-----------|------|
| Up | [INDEX.md](../INDEX.md) |
| Layer | [L02_SERVICES.md](../layers/L02_SERVICES.md) |
| Related | [M08_SERVICE_DISCOVERY.md](M08_SERVICE_DISCOVERY.md) |
| Related | [M12_CIRCUIT_BREAKER.md](M12_CIRCUIT_BREAKER.md) |
| Pattern | [PATTERN_001_CIRCUIT_BREAKER.md](../patterns/PATTERN_001_CIRCUIT_BREAKER.md) |
| L1 Foundation | [L01_FOUNDATION.md](../layers/L01_FOUNDATION.md) |
| L3 Learning | [L03_LEARNING.md](../layers/L03_LEARNING.md) |

---

## Module Specification

### Overview

The Health Monitor module provides continuous, multi-modal health surveillance for all services in the Maintenance Engine ecosystem. It implements active probing, passive observation, and predictive health assessment through integration with L1 metrics and L3 learning pathways.

### Module Properties

| Property | Value |
|----------|-------|
| Module ID | M07 |
| Module Name | Health Monitor |
| Layer | L2 (Services) |
| Version | 1.0.0 |
| Dependencies | M02 (Config), M04 (Metrics), M05 (State) |
| Dependents | M08 (Service Discovery), M09 (Mesh Controller), M12 (Circuit Breaker), L3 (Learning), L5 (Remediation) |

---

## Architecture Diagram

```
+-----------------------------------------------------------------------------------+
|                           M07: HEALTH MONITOR                                      |
+-----------------------------------------------------------------------------------+
|                                                                                   |
|  +-------------------------+    +-------------------------+    +--------------+   |
|  |    PROBE SCHEDULER      |    |    HEALTH AGGREGATOR    |    |  STATE FSM   |   |
|  |                         |    |                         |    |              |   |
|  | - Liveness probes       |    | - Multi-source fusion   |    | - UNKNOWN    |   |
|  | - Readiness probes      |--->| - Weighted scoring      |--->| - STARTING   |   |
|  | - Startup probes        |    | - Anomaly detection     |    | - HEALTHY    |   |
|  | - Deep health checks    |    | - Trend analysis        |    | - DEGRADED   |   |
|  +------------+------------+    +------------+------------+    | - UNHEALTHY  |   |
|               |                              |                 | - STOPPING   |   |
|               v                              v                 +--------------+   |
|  +-------------------------+    +-------------------------+          |            |
|  |   PROBE EXECUTORS       |    |   DEPENDENCY TRACKER    |          |            |
|  |                         |    |                         |          v            |
|  | - HTTP/HTTPS probes     |    | - Service graph         |    +--------------+   |
|  | - TCP socket probes     |    | - Cascade detection     |    | EVENT EMITTER|   |
|  | - gRPC health probes    |    | - Impact analysis       |    |              |   |
|  | - Custom script probes  |    | - Propagation tracking  |    | -> L3 Events |   |
|  +-------------------------+    +-------------------------+    | -> L5 Alerts |   |
|                                                                +--------------+   |
+-----------------------------------------------------------------------------------+
        ^                    ^                    ^                    |
        |                    |                    |                    v
   [L1: Config]        [L1: Metrics]       [L1: State]          [L3/L5: Events]
```

---

## Core Data Structures

### Health State Enumeration

```rust
/// Represents the current health state of a service
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HealthState {
    /// Unable to determine health status
    Unknown = 0,
    /// Service is starting up, not yet ready
    Starting = 1,
    /// All health checks passing, service fully operational
    Healthy = 2,
    /// Some checks failing but service still operational
    Degraded = 3,
    /// Critical checks failing, service non-operational
    Unhealthy = 4,
    /// Service is gracefully shutting down
    ShuttingDown = 5,
}

impl HealthState {
    /// Convert to numeric score for aggregation (0.0 = worst, 1.0 = best)
    pub fn to_score(&self) -> f64 {
        match self {
            Self::Unknown => 0.2,
            Self::Starting => 0.4,
            Self::Healthy => 1.0,
            Self::Degraded => 0.6,
            Self::Unhealthy => 0.0,
            Self::ShuttingDown => 0.3,
        }
    }

    /// Check if state allows traffic
    pub fn accepts_traffic(&self) -> bool {
        matches!(self, Self::Healthy | Self::Degraded)
    }
}
```

### Health Check Result

```rust
/// Result of a single health check execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckResult {
    /// Unique check identifier
    pub check_id: CheckId,

    /// Service being checked
    pub service_id: ServiceId,

    /// Type of check performed
    pub check_type: HealthCheckType,

    /// Whether the check passed
    pub passed: bool,

    /// Latency of the check in milliseconds
    pub latency_ms: u64,

    /// Optional status code (HTTP, gRPC, etc.)
    pub status_code: Option<u32>,

    /// Error message if check failed
    pub error: Option<String>,

    /// Timestamp of check execution
    pub timestamp: DateTime<Utc>,

    /// Additional check-specific metadata
    pub metadata: HashMap<String, Value>,
}

/// Types of health checks supported
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HealthCheckType {
    Liveness,
    Readiness,
    Startup,
    Deep,
    Custom,
}
```

### Aggregated Health Report

```rust
/// Comprehensive health report for a service
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthReport {
    /// Service identifier
    pub service_id: ServiceId,

    /// Current health state
    pub state: HealthState,

    /// Previous health state (for change detection)
    pub previous_state: Option<HealthState>,

    /// Aggregated health score [0.0, 1.0]
    pub health_score: f64,

    /// Individual check results
    pub checks: Vec<HealthCheckResult>,

    /// Dependency health summary
    pub dependencies: Vec<DependencyHealth>,

    /// Consecutive failure count
    pub failure_count: u32,

    /// Consecutive success count
    pub success_count: u32,

    /// Time in current state
    pub state_duration: Duration,

    /// Report generation timestamp
    pub timestamp: DateTime<Utc>,

    /// 11D error vector if unhealthy (from M01)
    pub error_vector: Option<ErrorVector>,
}

/// Health status of a dependency
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyHealth {
    pub service_id: ServiceId,
    pub state: HealthState,
    pub impact_weight: f64,
    pub last_checked: DateTime<Utc>,
}
```

### Health Probe Configuration

```rust
/// Configuration for a health probe
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthProbeConfig {
    /// Probe identifier
    pub id: ProbeId,

    /// Target service
    pub service_id: ServiceId,

    /// Probe type
    pub probe_type: ProbeType,

    /// Check interval
    pub interval: Duration,

    /// Check timeout
    pub timeout: Duration,

    /// Failures before marking unhealthy
    pub failure_threshold: u32,

    /// Successes before marking healthy
    pub success_threshold: u32,

    /// Initial delay before first check
    pub initial_delay: Duration,

    /// Probe-specific configuration
    pub config: ProbeTypeConfig,
}

/// Probe type-specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProbeTypeConfig {
    Http {
        url: String,
        method: HttpMethod,
        expected_status: Vec<u16>,
        headers: HashMap<String, String>,
        body: Option<String>,
    },
    Tcp {
        host: String,
        port: u16,
    },
    Grpc {
        address: String,
        service: String,
        use_tls: bool,
    },
    Exec {
        command: String,
        args: Vec<String>,
        expected_exit_code: i32,
    },
}
```

---

## Public API

### HealthMonitor Service

```rust
/// Main Health Monitor service
pub struct HealthMonitor {
    config: HealthMonitorConfig,
    probe_scheduler: ProbeScheduler,
    aggregator: HealthAggregator,
    state_machine: HealthStateMachine,
    event_emitter: EventEmitter<HealthEvent>,
    dependency_tracker: DependencyTracker,
    metrics: HealthMonitorMetrics,
}

impl HealthMonitor {
    /// Create a new HealthMonitor instance
    pub fn new(config: HealthMonitorConfig) -> Self;

    /// Start the health monitoring service
    pub async fn start(&mut self) -> Result<(), HealthMonitorError>;

    /// Stop the health monitoring service gracefully
    pub async fn stop(&mut self) -> Result<(), HealthMonitorError>;

    /// Register a new health probe
    pub fn register_probe(&mut self, probe: HealthProbeConfig) -> Result<ProbeId, HealthMonitorError>;

    /// Unregister an existing probe
    pub fn unregister_probe(&mut self, probe_id: &ProbeId) -> Result<(), HealthMonitorError>;

    /// Execute an immediate health check for a service
    pub async fn check_now(&self, service_id: &ServiceId) -> Result<HealthReport, HealthMonitorError>;

    /// Get the current health report for a service
    pub fn get_health(&self, service_id: &ServiceId) -> Option<HealthReport>;

    /// Get health reports for all monitored services
    pub fn get_all_health(&self) -> HashMap<ServiceId, HealthReport>;

    /// Subscribe to health events
    pub fn subscribe(&self) -> HealthEventStream;

    /// Get health history for a service
    pub fn get_history(
        &self,
        service_id: &ServiceId,
        since: DateTime<Utc>,
        limit: usize,
    ) -> Vec<HealthReport>;

    /// Update probe configuration dynamically
    pub fn update_probe(&mut self, probe_id: &ProbeId, config: HealthProbeConfig) -> Result<(), HealthMonitorError>;

    /// Force state transition (for testing/emergency)
    pub fn force_state(&mut self, service_id: &ServiceId, state: HealthState) -> Result<(), HealthMonitorError>;

    /// Get dependency graph for a service
    pub fn get_dependencies(&self, service_id: &ServiceId) -> Vec<DependencyHealth>;

    /// Calculate cascade impact if service fails
    pub fn calculate_cascade_impact(&self, service_id: &ServiceId) -> CascadeImpact;
}
```

### Probe Scheduler API

```rust
/// Manages probe scheduling and execution
pub struct ProbeScheduler {
    /// Schedule a new probe
    pub fn schedule(&mut self, probe: HealthProbeConfig) -> Result<(), SchedulerError>;

    /// Cancel a scheduled probe
    pub fn cancel(&mut self, probe_id: &ProbeId) -> Result<(), SchedulerError>;

    /// Pause all probes for a service
    pub fn pause_service(&mut self, service_id: &ServiceId);

    /// Resume probes for a service
    pub fn resume_service(&mut self, service_id: &ServiceId);

    /// Get next scheduled execution time
    pub fn next_execution(&self, probe_id: &ProbeId) -> Option<DateTime<Utc>>;

    /// Execute probe immediately (bypass schedule)
    pub async fn execute_now(&self, probe_id: &ProbeId) -> Result<HealthCheckResult, ProbeError>;
}
```

---

## Configuration

### TOML Configuration Example

```toml
[layer.L2.M07]
enabled = true
version = "1.0.0"

# Global defaults
[layer.L2.M07.defaults]
check_interval_ms = 5000
check_timeout_ms = 3000
failure_threshold = 3
success_threshold = 2
initial_delay_ms = 0
parallel_checks = true
max_concurrent_checks = 50

# Health aggregation settings
[layer.L2.M07.aggregation]
algorithm = "weighted_average"
anomaly_detection = true
anomaly_threshold_sigma = 3.0
trend_window_size = 100
score_decay_rate = 0.1

# State machine configuration
[layer.L2.M07.state_machine]
unknown_timeout_ms = 30000
starting_timeout_ms = 120000
debounce_transitions = true
debounce_window_ms = 5000

# Dependency tracking
[layer.L2.M07.dependencies]
enabled = true
cascade_detection = true
max_depth = 5
impact_calculation = true

# Event emission
[layer.L2.M07.events]
emit_all_checks = false
emit_state_changes = true
emit_anomalies = true
batch_window_ms = 1000

# Service-specific probe overrides
[[layer.L2.M07.probes]]
service = "synthex"
type = "http"
url = "http://localhost:8090/api/health"
interval_ms = 3000
timeout_ms = 2000
expected_status = [200]

[[layer.L2.M07.probes]]
service = "san-k7"
type = "http"
url = "http://localhost:8100/health"
interval_ms = 5000
timeout_ms = 3000
expected_status = [200]

[[layer.L2.M07.probes]]
service = "database"
type = "tcp"
host = "localhost"
port = 5432
interval_ms = 10000
timeout_ms = 5000
```

---

## Bi-Directional Data Flow

### Inbound Data Flow

The Health Monitor receives data from multiple sources across L1 Foundation and peer L2 modules.

#### Inbound Message Types

```rust
/// Messages received by Health Monitor from other modules/layers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HealthMonitorInbound {
    // From L1 Config (M02)
    ConfigUpdate {
        probe_configs: Vec<HealthProbeConfig>,
        global_settings: HealthMonitorConfig,
        timestamp: DateTime<Utc>,
    },

    // From L1 Metrics (M04)
    MetricsSnapshot {
        service_id: ServiceId,
        cpu_usage: f64,
        memory_usage: f64,
        request_rate: f64,
        error_rate: f64,
        latency_p99: Duration,
        timestamp: DateTime<Utc>,
    },

    // From L1 State (M05)
    StateRestored {
        service_states: HashMap<ServiceId, HealthState>,
        probe_history: Vec<HealthCheckResult>,
        timestamp: DateTime<Utc>,
    },

    // From M08 Service Discovery
    ServiceRegistered {
        service: ServiceDefinition,
        endpoints: Vec<Endpoint>,
        health_config: Option<HealthProbeConfig>,
    },

    ServiceDeregistered {
        service_id: ServiceId,
        reason: String,
    },

    // From L3 Learning
    PredictedDegradation {
        service_id: ServiceId,
        confidence: f64,
        predicted_time: DateTime<Utc>,
        contributing_factors: Vec<String>,
    },

    // From L5 Remediation
    RemediationStarted {
        service_id: ServiceId,
        action_type: ActionType,
        expected_duration: Duration,
    },

    RemediationCompleted {
        service_id: ServiceId,
        success: bool,
        action_type: ActionType,
    },
}
```

#### Inbound Flow Sources

| Source | Message Type | Trigger Condition | Frequency |
|--------|-------------|-------------------|-----------|
| L1 Config (M02) | ConfigUpdate | Config file change, hot reload | On change |
| L1 Metrics (M04) | MetricsSnapshot | Metric collection cycle | Every 15s |
| L1 State (M05) | StateRestored | System startup, recovery | On startup |
| M08 Discovery | ServiceRegistered | New service registration | On event |
| M08 Discovery | ServiceDeregistered | Service removal | On event |
| L3 Learning | PredictedDegradation | ML prediction triggers | As predicted |
| L5 Remediation | RemediationStarted | Action execution begins | On event |
| L5 Remediation | RemediationCompleted | Action execution ends | On event |

#### Inbound Sequence Diagram

```
    L1:Config    L1:Metrics    M08:Discovery    L3:Learning    L5:Remediation
        |             |              |               |               |
        |  ConfigUpdate              |               |               |
        |------------>|              |               |               |
        |             |              |               |               |
        |             | MetricsSnapshot              |               |
        |             |------------->|               |               |
        |             |              |               |               |
        |             |              | ServiceRegistered             |
        |             |              |-------------->|               |
        |             |              |               |               |
        |             |              |               | PredictedDegradation
        |             |              |               |-------------->|
        |             |              |               |               |
        |             |              |               |      RemediationStarted
        |             |              |               |<--------------|
        |             |              |               |               |
        +-------------+-------+------+---------------+---------------+
                              |
                              v
                    +-------------------+
                    |  M07 HEALTH       |
                    |  MONITOR          |
                    |                   |
                    | - Process inputs  |
                    | - Update state    |
                    | - Adjust probes   |
                    +-------------------+
```

### Outbound Data Flow

The Health Monitor emits health events and reports to multiple consumers.

#### Outbound Message Types

```rust
/// Messages emitted by Health Monitor to other modules/layers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HealthMonitorOutbound {
    // To L1 Metrics (M04)
    HealthMetrics {
        service_id: ServiceId,
        state: HealthState,
        health_score: f64,
        check_latency_ms: u64,
        failure_count: u32,
        timestamp: DateTime<Utc>,
    },

    // To L1 State (M05)
    StatePersist {
        service_id: ServiceId,
        state: HealthState,
        report: HealthReport,
        timestamp: DateTime<Utc>,
    },

    // To M08 Service Discovery
    HealthStateChanged {
        service_id: ServiceId,
        old_state: HealthState,
        new_state: HealthState,
        report: HealthReport,
    },

    // To M12 Circuit Breaker
    ServiceUnhealthy {
        service_id: ServiceId,
        failure_count: u32,
        last_error: Option<String>,
        recommendation: CircuitAction,
    },

    ServiceRecovered {
        service_id: ServiceId,
        recovery_duration: Duration,
    },

    // To L3 Learning
    HealthEvent {
        service_id: ServiceId,
        event_type: HealthEventType,
        health_report: HealthReport,
        error_vector: Option<ErrorVector>,
        context: HealthEventContext,
    },

    // To L5 Remediation
    RemediationRequired {
        service_id: ServiceId,
        severity: Severity,
        health_report: HealthReport,
        suggested_tier: EscalationTier,
        suggested_action: Option<ActionType>,
    },

    // Broadcast to all subscribers
    HealthBroadcast {
        reports: HashMap<ServiceId, HealthReport>,
        system_health_score: f64,
        timestamp: DateTime<Utc>,
    },
}

/// Types of health events for L3 learning
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HealthEventType {
    StateTransition { from: HealthState, to: HealthState },
    AnomalyDetected { metric: String, value: f64, threshold: f64 },
    DependencyImpact { affected_by: ServiceId },
    RecoveryCompleted { duration: Duration },
    DegradationProgressing { rate: f64 },
}
```

#### Outbound Flow Targets

| Target | Message Type | Trigger Condition | Priority |
|--------|-------------|-------------------|----------|
| L1 Metrics (M04) | HealthMetrics | Every health check | Normal |
| L1 State (M05) | StatePersist | State change | High |
| M08 Discovery | HealthStateChanged | State transition | High |
| M12 Circuit Breaker | ServiceUnhealthy | Unhealthy transition | Critical |
| M12 Circuit Breaker | ServiceRecovered | Recovery detected | High |
| L3 Learning | HealthEvent | All significant events | Normal |
| L5 Remediation | RemediationRequired | Unhealthy + threshold | Critical |
| All Subscribers | HealthBroadcast | Periodic broadcast | Low |

#### Outbound Sequence Diagram

```
                    +-------------------+
                    |  M07 HEALTH       |
                    |  MONITOR          |
                    +--------+----------+
                             |
        +--------------------+--------------------+
        |                    |                    |
        v                    v                    v
   +---------+         +---------+          +---------+
   |L1:Metrics|        |L1:State |          |M08:Disc |
   +---------+         +---------+          +---------+
        |                    |                    |
        |                    |                    |
        +--------------------+--------------------+
                             |
        +--------------------+--------------------+
        |                    |                    |
        v                    v                    v
   +---------+         +---------+          +---------+
   |M12:CB   |         |L3:Learn |          |L5:Remed |
   +---------+         +---------+          +---------+
        |                    |                    |
        | ServiceUnhealthy   | HealthEvent        | RemediationRequired
        | ServiceRecovered   | (for learning)     | (for action)
        v                    v                    v
   [Open/Close CB]     [Update pathways]    [Execute remediation]
```

### Cross-Module Dependencies

#### Bidirectional Dependency Matrix

| Module | Reads From M07 | Writes To M07 | Sync/Async | Error Path |
|--------|---------------|---------------|------------|------------|
| M02 Config | - | ConfigUpdate | Async | Fallback to defaults |
| M04 Metrics | HealthMetrics | MetricsSnapshot | Async | Continue without |
| M05 State | StatePersist | StateRestored | Async | Start fresh |
| M08 Discovery | HealthStateChanged | ServiceRegistered/Deregistered | Async | Skip service |
| M09 Mesh | HealthReport (query) | - | Sync | Use cached |
| M10 Traffic | HealthReport (query) | - | Sync | Route anyway |
| M11 Load Balancer | HealthReport (query) | - | Sync | Equal weights |
| M12 Circuit Breaker | ServiceUnhealthy/Recovered | - | Async | CB decides |
| L3 Learning | HealthEvent | PredictedDegradation | Async | Ignore prediction |
| L5 Remediation | RemediationRequired | RemediationStarted/Completed | Async | Monitor anyway |

#### Communication Patterns

```rust
/// Communication pattern definitions for M07
pub struct HealthMonitorComms {
    // Synchronous queries (blocking, immediate response needed)
    sync_queries: SyncQueryHandler,

    // Asynchronous events (fire-and-forget, buffered)
    async_events: AsyncEventEmitter,

    // Request-response patterns (async with callback)
    request_response: RequestResponseHandler,
}

impl HealthMonitorComms {
    /// Synchronous: Other modules query current health
    pub fn handle_sync_query(&self, query: HealthQuery) -> HealthReport {
        // Immediate response from cached state
        self.state_cache.get(&query.service_id).clone()
    }

    /// Asynchronous: Emit health events to subscribers
    pub async fn emit_event(&self, event: HealthMonitorOutbound) {
        // Non-blocking, buffered delivery
        self.event_bus.publish(event).await;
    }

    /// Request-Response: Request health check and await result
    pub async fn request_check(&self, service_id: ServiceId) -> Result<HealthReport, Error> {
        let (tx, rx) = oneshot::channel();
        self.probe_scheduler.execute_with_callback(service_id, tx);
        rx.await?
    }
}
```

#### Error Propagation Paths

```
M07 Health Error
       |
       +---> [Log to M03 Logging] ---> Structured log entry
       |
       +---> [Encode via M01 Error Taxonomy] ---> 11D Error Vector
       |            |
       |            +---> [Send to L3 Learning] ---> Pattern recognition
       |
       +---> [Emit to L5 Remediation] ---> Trigger action if threshold
       |
       +---> [Update M12 Circuit Breaker] ---> Protect downstream
```

### Contextual Flow: Health Data Transformation

#### State Machine Transitions

```
                                 +-------------------+
                                 |      UNKNOWN      |
                                 |                   |
                                 | Initial state,    |
                                 | no data yet       |
                                 +--------+----------+
                                          |
                        First successful  | First failed
                        startup probe     | probe
                         +----------------+----------------+
                         |                                 |
                         v                                 v
              +-------------------+             +-------------------+
              |     STARTING      |             |     UNHEALTHY     |
              |                   |             |                   |
              | Startup probes    |             | Critical failure  |
              | in progress       |             | detected          |
              +--------+----------+             +--------+----------+
                       |                                 ^
    All startup probes | pass                            | Failure threshold
                       |                                 | exceeded
                       v                                 |
              +-------------------+             +--------+----------+
              |      HEALTHY      |<------------|     DEGRADED      |
              |                   |             |                   |
              | All checks pass   | Recovery    | Some checks fail  |
              | Normal operation  | threshold   | Still operational |
              +--------+----------+ reached     +--------+----------+
                       |                                 ^
                       | Some checks   Success threshold |
                       | start failing not yet reached   |
                       +-------------------------------->+
                       |
                       | Shutdown signal received
                       v
              +-------------------+
              |   SHUTTING_DOWN   |
              |                   |
              | Graceful shutdown |
              | in progress       |
              +-------------------+
```

#### Data Lifecycle Within Module

```rust
/// Health data transformation pipeline
impl HealthMonitor {
    /// Complete health check lifecycle
    async fn health_check_lifecycle(&self, service_id: &ServiceId) -> HealthReport {
        // 1. COLLECT: Execute probes
        let probe_results = self.probe_scheduler.execute_all(service_id).await;

        // 2. AGGREGATE: Combine results
        let aggregated = self.aggregator.aggregate(&probe_results);

        // 3. ENRICH: Add dependency data
        let with_deps = self.dependency_tracker.enrich(aggregated);

        // 4. TRANSFORM: Apply state machine
        let new_state = self.state_machine.transition(service_id, &with_deps);

        // 5. ENCODE: Generate error vector if unhealthy
        let error_vector = if new_state == HealthState::Unhealthy {
            Some(self.error_taxonomy.encode(&with_deps))
        } else {
            None
        };

        // 6. EMIT: Send to consumers
        let report = HealthReport {
            service_id: service_id.clone(),
            state: new_state,
            health_score: with_deps.score,
            checks: probe_results,
            dependencies: with_deps.dependencies,
            error_vector,
            timestamp: Utc::now(),
            // ... other fields
        };

        self.emit_to_consumers(&report).await;

        report
    }
}
```

---

## Metrics

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `me_m07_health_checks_total` | Counter | service, type, result | Total health checks executed |
| `me_m07_health_check_duration_ms` | Histogram | service, type | Health check latency distribution |
| `me_m07_health_state` | Gauge | service | Current health state (0-5) |
| `me_m07_health_score` | Gauge | service | Current health score (0.0-1.0) |
| `me_m07_state_transitions_total` | Counter | service, from, to | State transition count |
| `me_m07_active_probes` | Gauge | type | Number of active probes by type |
| `me_m07_probe_failures_total` | Counter | service, type, error | Probe failure count |
| `me_m07_dependency_health` | Gauge | service, dependency | Dependency health scores |
| `me_m07_cascade_impact_score` | Gauge | service | Potential cascade impact |
| `me_m07_events_emitted_total` | Counter | event_type, target | Events emitted count |

---

## Error Codes

| Code | Name | Severity | Description | Recovery |
|------|------|----------|-------------|----------|
| E7001 | ProbeTimeout | Warning | Health probe timed out | Retry with backoff |
| E7002 | ProbeConnectionRefused | Error | Cannot connect to service | Check service status |
| E7003 | ProbeUnexpectedResponse | Warning | Unexpected response code | Verify probe config |
| E7004 | AggregationFailure | Error | Cannot aggregate health data | Use last known state |
| E7005 | StateTransitionInvalid | Error | Invalid state transition | Reset state machine |
| E7006 | DependencyGraphCycle | Warning | Circular dependency detected | Review service deps |
| E7007 | ConfigurationInvalid | Critical | Invalid probe configuration | Reject and alert |
| E7008 | StoragePersistFailed | Warning | Cannot persist health state | Retry, log to M03 |
| E7009 | EventEmissionFailed | Warning | Cannot emit health event | Buffer and retry |
| E7010 | CascadeImpactHigh | Critical | High cascade impact detected | Alert L5 immediately |

---

## Navigation

| Direction | Link |
|-----------|------|
| Up | [INDEX.md](../INDEX.md) |
| Layer | [L02_SERVICES.md](../layers/L02_SERVICES.md) |
| Previous | [M01_ERROR_TAXONOMY.md](M01_ERROR_TAXONOMY.md) |
| Next | [M08_SERVICE_DISCOVERY.md](M08_SERVICE_DISCOVERY.md) |
| Related | [M12_CIRCUIT_BREAKER.md](M12_CIRCUIT_BREAKER.md) |
| Pattern | [PATTERN_001_CIRCUIT_BREAKER.md](../patterns/PATTERN_001_CIRCUIT_BREAKER.md) |
| API | [REST_API.md](../integration/REST_API.md) |

---

*[Back to Index](../INDEX.md) | [Layer: L02 Services](../layers/L02_SERVICES.md)*
