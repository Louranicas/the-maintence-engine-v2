# Module M12: Circuit Breaker

> **M12_CIRCUIT_BREAKER** | Fault Tolerance & Cascade Prevention | Layer: L2 Services | [Back to Index](../INDEX.md)

---

## Navigation

| Direction | Link |
|-----------|------|
| Up | [INDEX.md](../INDEX.md) |
| Layer | [L02_SERVICES.md](../layers/L02_SERVICES.md) |
| Previous | [M11_LOAD_BALANCER.md](M11_LOAD_BALANCER.md) |
| Related | [M07_HEALTH_MONITOR.md](M07_HEALTH_MONITOR.md) |
| Pattern | [PATTERN_001_CIRCUIT_BREAKER.md](../patterns/PATTERN_001_CIRCUIT_BREAKER.md) |
| L1 Foundation | [L01_FOUNDATION.md](../layers/L01_FOUNDATION.md) |
| L3 Learning | [L03_LEARNING.md](../layers/L03_LEARNING.md) |
| L5 Remediation | [L05_REMEDIATION.md](../layers/L05_REMEDIATION.md) |

---

## Module Specification

### Overview

The Circuit Breaker module prevents cascading failures by monitoring service health and temporarily blocking requests to failing services. It implements the circuit breaker pattern with three states (CLOSED, OPEN, HALF_OPEN), integrating with M07 Health Monitor for health data, M11 Load Balancer for endpoint-level protection, and L3 Learning for adaptive threshold tuning.

### Module Properties

| Property | Value |
|----------|-------|
| Module ID | M12 |
| Module Name | Circuit Breaker |
| Layer | L2 (Services) |
| Version | 1.0.0 |
| Dependencies | M02 (Config), M07 (Health Monitor), M11 (Load Balancer) |
| Dependents | M09 (Mesh Controller), M10 (Traffic Manager), L3 (Learning), L5 (Remediation), L6 (Integration) |

---

## Architecture Diagram

```
+-----------------------------------------------------------------------------------+
|                           M12: CIRCUIT BREAKER                                     |
+-----------------------------------------------------------------------------------+
|                                                                                   |
|  +-------------------------+    +-------------------------+    +--------------+   |
|  |   CIRCUIT REGISTRY      |    |   STATE MACHINE         |    | THRESHOLD    |   |
|  |                         |    |                         |    | MANAGER      |   |
|  | - Per-service circuits  |    | - CLOSED state          |    |              |   |
|  | - Per-endpoint circuits |--->| - OPEN state            |--->| - Failure %  |   |
|  | - Circuit lifecycle     |    | - HALF_OPEN state       |    | - Latency    |   |
|  | - Circuit groups        |    | - State transitions     |    | - Volume     |   |
|  +------------+------------+    +------------+------------+    +--------------+   |
|               |                              |                        |           |
|               v                              v                        v           |
|  +-------------------------+    +-------------------------+    +--------------+   |
|  |   FAILURE DETECTOR      |    |   RECOVERY MANAGER      |    | FALLBACK MGR |   |
|  |                         |    |                         |    |              |   |
|  | - Error classification  |    | - Probe scheduling      |    | - Default    |   |
|  | - Sliding window        |    | - Recovery validation   |    |   responses  |   |
|  | - Threshold evaluation  |    | - Gradual restoration   |    | - Cached data|   |
|  +-------------------------+    +-------------------------+    | - Degraded   |   |
|                                                                |   mode       |   |
|                                                                +--------------+   |
+-----------------------------------------------------------------------------------+
        ^                    ^                    ^                    |
        |                    |                    |                    v
   [M07: Health]       [M11: Load Balancer] [L5: Remediation]    [Request Flow]
```

---

## Core Data Structures

### Circuit Breaker State

```rust
/// Circuit breaker states
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CircuitState {
    /// Circuit is closed, requests flow normally
    Closed,
    /// Circuit is open, requests are blocked
    Open,
    /// Circuit is testing recovery
    HalfOpen,
}

impl CircuitState {
    /// Check if requests should be allowed
    pub fn allows_request(&self) -> bool {
        matches!(self, Self::Closed | Self::HalfOpen)
    }

    /// Get state name for metrics
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Closed => "closed",
            Self::Open => "open",
            Self::HalfOpen => "half_open",
        }
    }
}

/// Complete circuit breaker instance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitBreaker {
    /// Circuit identifier
    pub id: CircuitId,

    /// Target (service or endpoint)
    pub target: CircuitTarget,

    /// Current state
    pub state: CircuitState,

    /// Configuration
    pub config: CircuitBreakerConfig,

    /// Failure tracking
    pub failure_tracker: FailureTracker,

    /// Recovery tracking
    pub recovery_tracker: RecoveryTracker,

    /// Circuit statistics
    pub stats: CircuitStats,

    /// State history
    pub history: Vec<StateTransition>,

    /// Creation timestamp
    pub created_at: DateTime<Utc>,

    /// Last state change
    pub last_state_change: DateTime<Utc>,
}

/// Circuit target types
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CircuitTarget {
    /// Service-level circuit
    Service(ServiceId),
    /// Endpoint-level circuit
    Endpoint(ServiceId, EndpointId),
    /// Operation-level circuit
    Operation(ServiceId, String),
    /// Custom circuit group
    Group(String, Vec<CircuitTarget>),
}
```

### Circuit Breaker Configuration

```rust
/// Circuit breaker configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitBreakerConfig {
    /// Failure threshold to open circuit
    pub failure_threshold: FailureThreshold,

    /// Time window for failure counting
    pub failure_window: Duration,

    /// Time to wait before attempting recovery
    pub recovery_timeout: Duration,

    /// Number of probe requests in half-open state
    pub probe_requests: u32,

    /// Success threshold to close circuit
    pub success_threshold: u32,

    /// Request timeout (triggers failure)
    pub request_timeout: Duration,

    /// Errors that should trip the circuit
    pub tripable_errors: Vec<TripableError>,

    /// Fallback configuration
    pub fallback: Option<FallbackConfig>,

    /// Minimum request volume for evaluation
    pub min_request_volume: u64,

    /// Slow call threshold (latency-based tripping)
    pub slow_call_threshold: Option<SlowCallThreshold>,

    /// Enable adaptive thresholds
    pub adaptive: bool,
}

/// Failure threshold types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FailureThreshold {
    /// Fixed count of failures
    Count(u32),
    /// Percentage of failures (0.0 - 1.0)
    Percentage(f64),
    /// Both count and percentage must be exceeded
    Both { count: u32, percentage: f64 },
    /// Either count or percentage triggers
    Either { count: u32, percentage: f64 },
}

/// Errors that can trip the circuit
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TripableError {
    /// Connection failures
    ConnectionError,
    /// Request timeouts
    Timeout,
    /// HTTP 5xx errors
    ServerError,
    /// HTTP 429 (too many requests)
    RateLimited,
    /// Specific HTTP status codes
    HttpStatus(Vec<u16>),
    /// Specific gRPC status codes
    GrpcStatus(Vec<i32>),
    /// Specific error categories (from M01)
    ErrorCategory(ErrorCategory),
    /// All errors
    AllErrors,
}

/// Slow call threshold configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlowCallThreshold {
    /// Latency threshold (ms)
    pub latency_ms: u64,
    /// Percentage of slow calls to trigger (0.0 - 1.0)
    pub percentage: f64,
}
```

### Failure Tracking

```rust
/// Tracks failures for threshold evaluation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailureTracker {
    /// Sliding window of requests
    pub window: SlidingWindow,

    /// Current failure count
    pub failure_count: u32,

    /// Current success count
    pub success_count: u32,

    /// Current failure rate
    pub failure_rate: f64,

    /// Consecutive failures
    pub consecutive_failures: u32,

    /// Consecutive successes
    pub consecutive_successes: u32,

    /// Slow call tracking
    pub slow_calls: SlowCallTracker,

    /// Last failure details
    pub last_failure: Option<FailureRecord>,
}

/// Sliding window for time-based failure tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlidingWindow {
    /// Window duration
    pub duration: Duration,

    /// Request records
    pub records: VecDeque<RequestRecord>,

    /// Total requests in window
    pub total_requests: u64,

    /// Failed requests in window
    pub failed_requests: u64,

    /// Slow requests in window
    pub slow_requests: u64,
}

/// Individual request record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestRecord {
    /// Request timestamp
    pub timestamp: DateTime<Utc>,

    /// Was request successful?
    pub success: bool,

    /// Request latency
    pub latency: Duration,

    /// Error type (if failed)
    pub error: Option<TripableError>,
}

/// Failure record for diagnostics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailureRecord {
    /// Failure timestamp
    pub timestamp: DateTime<Utc>,

    /// Error that caused failure
    pub error: TripableError,

    /// Error message
    pub message: String,

    /// Request context
    pub context: HashMap<String, String>,
}
```

### Recovery Tracking

```rust
/// Tracks recovery attempts
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryTracker {
    /// When the circuit opened
    pub opened_at: Option<DateTime<Utc>>,

    /// When recovery timeout expires
    pub recovery_at: Option<DateTime<Utc>>,

    /// Current probe count
    pub probe_count: u32,

    /// Successful probes
    pub successful_probes: u32,

    /// Failed probes
    pub failed_probes: u32,

    /// Recovery attempt count
    pub attempt_count: u32,

    /// Last recovery attempt
    pub last_attempt: Option<DateTime<Utc>>,
}

/// State transition record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateTransition {
    /// Previous state
    pub from: CircuitState,

    /// New state
    pub to: CircuitState,

    /// Transition reason
    pub reason: TransitionReason,

    /// Transition timestamp
    pub timestamp: DateTime<Utc>,

    /// Stats at transition time
    pub stats_snapshot: CircuitStats,
}

/// Reasons for state transitions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TransitionReason {
    /// Failure threshold exceeded
    FailureThresholdExceeded { failures: u32, threshold: u32 },
    /// Failure rate exceeded
    FailureRateExceeded { rate: f64, threshold: f64 },
    /// Slow call rate exceeded
    SlowCallRateExceeded { rate: f64, threshold: f64 },
    /// Recovery timeout expired
    RecoveryTimeoutExpired,
    /// Probe succeeded
    ProbeSuccess { successes: u32, required: u32 },
    /// Probe failed
    ProbeFailed,
    /// Manual intervention
    ManualOverride { by: String, reason: String },
    /// Health monitor triggered
    HealthMonitorTriggered { health_state: HealthState },
    /// Adaptive threshold adjustment
    AdaptiveAdjustment { new_threshold: f64 },
}
```

### Circuit Statistics

```rust
/// Circuit breaker statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitStats {
    /// Total requests
    pub requests_total: u64,

    /// Allowed requests
    pub requests_allowed: u64,

    /// Blocked requests (circuit open)
    pub requests_blocked: u64,

    /// Successful requests
    pub requests_success: u64,

    /// Failed requests
    pub requests_failed: u64,

    /// Timed out requests
    pub requests_timeout: u64,

    /// Requests using fallback
    pub requests_fallback: u64,

    /// Times circuit opened
    pub times_opened: u64,

    /// Times circuit closed (recovered)
    pub times_closed: u64,

    /// Current failure rate
    pub current_failure_rate: f64,

    /// Average time in open state
    pub avg_open_duration: Duration,

    /// Last successful request
    pub last_success: Option<DateTime<Utc>>,

    /// Last failure
    pub last_failure: Option<DateTime<Utc>>,
}
```

### Fallback Configuration

```rust
/// Fallback behavior configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FallbackConfig {
    /// Fallback type
    pub fallback_type: FallbackType,

    /// Timeout for fallback execution
    pub timeout: Duration,

    /// Cache TTL for cached fallback
    pub cache_ttl: Option<Duration>,

    /// Custom fallback handler name
    pub handler: Option<String>,
}

/// Types of fallback behavior
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FallbackType {
    /// Return default value
    DefaultValue(Value),
    /// Return cached response
    CachedResponse,
    /// Return error immediately
    FailFast,
    /// Route to backup service
    BackupService(ServiceId),
    /// Execute custom handler
    CustomHandler(String),
    /// Return degraded response
    DegradedResponse(Value),
    /// Queue for later
    QueueRequest { max_size: usize, timeout: Duration },
}

/// Fallback result
#[derive(Debug, Clone)]
pub enum FallbackResult {
    /// Fallback succeeded
    Success(Value),
    /// Fallback also failed
    Failed(String),
    /// No fallback configured
    None,
}
```

---

## Public API

### CircuitBreakerService

```rust
/// Main Circuit Breaker service
pub struct CircuitBreakerService {
    config: GlobalCircuitBreakerConfig,
    registry: CircuitRegistry,
    state_machine: CircuitStateMachine,
    failure_detector: FailureDetector,
    recovery_manager: RecoveryManager,
    fallback_manager: FallbackManager,
    metrics: CircuitBreakerMetrics,
}

impl CircuitBreakerService {
    /// Create a new CircuitBreakerService
    pub fn new(config: GlobalCircuitBreakerConfig) -> Self;

    /// Start the circuit breaker service
    pub async fn start(&mut self) -> Result<(), CircuitBreakerError>;

    /// Stop the circuit breaker service
    pub async fn stop(&mut self) -> Result<(), CircuitBreakerError>;

    // === Circuit Management API ===

    /// Create a circuit breaker
    pub fn create_circuit(&mut self, target: CircuitTarget, config: CircuitBreakerConfig) -> Result<CircuitId, CircuitBreakerError>;

    /// Get circuit by ID
    pub fn get_circuit(&self, circuit_id: &CircuitId) -> Option<&CircuitBreaker>;

    /// Get circuit by target
    pub fn get_circuit_by_target(&self, target: &CircuitTarget) -> Option<&CircuitBreaker>;

    /// Remove a circuit breaker
    pub fn remove_circuit(&mut self, circuit_id: &CircuitId) -> Result<(), CircuitBreakerError>;

    /// List all circuits
    pub fn list_circuits(&self) -> Vec<&CircuitBreaker>;

    /// Update circuit configuration
    pub fn update_config(&mut self, circuit_id: &CircuitId, config: CircuitBreakerConfig) -> Result<(), CircuitBreakerError>;

    // === Request Execution API ===

    /// Execute a request through circuit breaker
    pub async fn execute<F, T>(&self, circuit_id: &CircuitId, f: F) -> Result<T, CircuitBreakerError>
    where
        F: Future<Output = Result<T, ServiceError>>;

    /// Execute with fallback
    pub async fn execute_with_fallback<F, T, FB>(&self, circuit_id: &CircuitId, f: F, fallback: FB) -> Result<T, CircuitBreakerError>
    where
        F: Future<Output = Result<T, ServiceError>>,
        FB: FnOnce() -> T;

    /// Check if circuit allows request (without executing)
    pub fn allows_request(&self, circuit_id: &CircuitId) -> bool;

    /// Get current state
    pub fn get_state(&self, circuit_id: &CircuitId) -> Option<CircuitState>;

    // === Manual Control API ===

    /// Force circuit open
    pub fn force_open(&mut self, circuit_id: &CircuitId, reason: &str) -> Result<(), CircuitBreakerError>;

    /// Force circuit closed
    pub fn force_close(&mut self, circuit_id: &CircuitId, reason: &str) -> Result<(), CircuitBreakerError>;

    /// Force circuit to half-open (probe)
    pub fn force_probe(&mut self, circuit_id: &CircuitId) -> Result<(), CircuitBreakerError>;

    /// Reset circuit to initial state
    pub fn reset(&mut self, circuit_id: &CircuitId) -> Result<(), CircuitBreakerError>;

    // === Reporting API ===

    /// Record success
    pub fn record_success(&mut self, circuit_id: &CircuitId, latency: Duration);

    /// Record failure
    pub fn record_failure(&mut self, circuit_id: &CircuitId, error: TripableError, latency: Duration);

    /// Get circuit statistics
    pub fn get_stats(&self, circuit_id: &CircuitId) -> Option<&CircuitStats>;

    /// Get state history
    pub fn get_history(&self, circuit_id: &CircuitId) -> Option<&[StateTransition]>;

    // === Event Subscription API ===

    /// Subscribe to circuit events
    pub fn subscribe(&self) -> CircuitEventStream;

    /// Subscribe to specific circuit events
    pub fn subscribe_circuit(&self, circuit_id: &CircuitId) -> CircuitEventStream;
}
```

### FailureDetector API

```rust
/// Detects failures and evaluates thresholds
pub struct FailureDetector {
    /// Record a request result
    pub fn record(&mut self, circuit_id: &CircuitId, result: &RequestRecord);

    /// Check if threshold is exceeded
    pub fn is_threshold_exceeded(&self, circuit_id: &CircuitId) -> bool;

    /// Get current failure rate
    pub fn get_failure_rate(&self, circuit_id: &CircuitId) -> f64;

    /// Get slow call rate
    pub fn get_slow_call_rate(&self, circuit_id: &CircuitId) -> f64;

    /// Reset failure tracking
    pub fn reset(&mut self, circuit_id: &CircuitId);

    /// Classify error
    pub fn classify_error(&self, error: &ServiceError) -> TripableError;

    /// Check if error is tripable
    pub fn is_tripable(&self, error: &TripableError, config: &CircuitBreakerConfig) -> bool;
}
```

---

## Configuration

### TOML Configuration Example

```toml
[layer.L2.M12]
enabled = true
version = "1.0.0"

# Global defaults
[layer.L2.M12.defaults]
failure_threshold_percentage = 0.5
failure_window_ms = 60000
recovery_timeout_ms = 30000
probe_requests = 3
success_threshold = 2
request_timeout_ms = 5000
min_request_volume = 10
adaptive_thresholds = true

# Slow call configuration
[layer.L2.M12.defaults.slow_call]
enabled = true
latency_threshold_ms = 3000
percentage_threshold = 0.5

# Default tripable errors
[layer.L2.M12.defaults.tripable_errors]
connection_error = true
timeout = true
server_error = true
rate_limited = true
http_status = [500, 502, 503, 504]

# Default fallback
[layer.L2.M12.defaults.fallback]
type = "fail_fast"
timeout_ms = 1000

# State machine settings
[layer.L2.M12.state_machine]
transition_debounce_ms = 1000
history_size = 100
emit_all_transitions = true

# Recovery settings
[layer.L2.M12.recovery]
probe_interval_ms = 5000
max_probe_retries = 5
gradual_restoration = true
restoration_increment = 0.2

# Service-specific circuit configurations
[[layer.L2.M12.circuits]]
target = { type = "service", id = "synthex" }
failure_threshold_percentage = 0.3
recovery_timeout_ms = 60000
[layer.L2.M12.circuits.fallback]
type = "cached_response"
cache_ttl_ms = 30000

[[layer.L2.M12.circuits]]
target = { type = "service", id = "san-k7" }
failure_threshold_count = 5
recovery_timeout_ms = 30000
[layer.L2.M12.circuits.fallback]
type = "backup_service"
backup = "san-k7-replica"

[[layer.L2.M12.circuits]]
target = { type = "service", id = "database" }
failure_threshold_percentage = 0.1
recovery_timeout_ms = 120000
min_request_volume = 100
[layer.L2.M12.circuits.slow_call]
latency_threshold_ms = 1000
percentage_threshold = 0.3
```

---

## Bi-Directional Data Flow

### Inbound Data Flow

The Circuit Breaker receives health data, request results, and configuration updates.

#### Inbound Message Types

```rust
/// Messages received by Circuit Breaker
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CircuitBreakerInbound {
    // From L1 Config (M02)
    ConfigUpdate {
        global_config: GlobalCircuitBreakerConfig,
        circuit_configs: Vec<(CircuitTarget, CircuitBreakerConfig)>,
        timestamp: DateTime<Utc>,
    },

    // From L1 State (M05)
    StateRestored {
        circuits: Vec<CircuitBreaker>,
        timestamp: DateTime<Utc>,
    },

    // From M07 Health Monitor
    ServiceUnhealthy {
        service_id: ServiceId,
        failure_count: u32,
        last_error: Option<String>,
        recommendation: CircuitAction,
        timestamp: DateTime<Utc>,
    },

    ServiceRecovered {
        service_id: ServiceId,
        recovery_duration: Duration,
        timestamp: DateTime<Utc>,
    },

    // From M09 Mesh Controller
    CircuitBreakerPolicy {
        service_id: ServiceId,
        thresholds: CircuitBreakerThresholds,
        timestamp: DateTime<Utc>,
    },

    // From M10 Traffic Manager
    TrafficHealthReport {
        service_id: ServiceId,
        error_rate: f64,
        timeout_rate: f64,
        latency_trend: LatencyTrend,
        timestamp: DateTime<Utc>,
    },

    // From M11 Load Balancer
    EndpointSelection {
        pool_id: PoolId,
        endpoint_id: EndpointId,
        endpoint_address: SocketAddr,
        algorithm_used: LoadBalanceAlgorithm,
        timestamp: DateTime<Utc>,
    },

    EndpointEjectionNotice {
        pool_id: PoolId,
        endpoint_id: EndpointId,
        reason: EjectionReason,
        duration: Duration,
        timestamp: DateTime<Utc>,
    },

    // From L3 Learning
    ThresholdRecommendation {
        circuit_id: CircuitId,
        recommended_threshold: FailureThreshold,
        confidence: f64,
        rationale: String,
        timestamp: DateTime<Utc>,
    },

    PatternDetected {
        circuit_id: CircuitId,
        pattern: FailurePattern,
        predicted_outcome: PredictedOutcome,
        timestamp: DateTime<Utc>,
    },

    // From L5 Remediation
    ForceCircuitState {
        circuit_id: CircuitId,
        target_state: CircuitState,
        reason: String,
        duration: Option<Duration>,
        timestamp: DateTime<Utc>,
    },

    CircuitRemediationAction {
        circuit_id: CircuitId,
        action: CircuitRemediationAction,
        timestamp: DateTime<Utc>,
    },
}

/// Circuit action recommendations
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum CircuitAction {
    Open,
    Close,
    Probe,
    AdjustThreshold,
    NoAction,
}

/// Failure patterns detected by L3
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FailurePattern {
    TransientSpike,
    GradualDegradation,
    PeriodicFailure { period: Duration },
    CascadeFailure { source: ServiceId },
    ResourceExhaustion,
}

/// Predicted outcomes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PredictedOutcome {
    WillRecover { estimated_time: Duration },
    WillFail { confidence: f64 },
    Uncertain,
}

/// Circuit remediation actions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CircuitRemediationAction {
    ResetCircuit,
    AdjustThreshold { new_threshold: FailureThreshold },
    ExtendRecoveryTimeout { duration: Duration },
    DisableCircuit { duration: Duration },
    EnableFallback { fallback: FallbackConfig },
}
```

#### Inbound Flow Sources

| Source | Message Type | Trigger Condition | Frequency |
|--------|-------------|-------------------|-----------|
| L1 Config (M02) | ConfigUpdate | Config file change | On change |
| L1 State (M05) | StateRestored | System startup | On startup |
| M07 Health | ServiceUnhealthy | Health degradation | On change |
| M07 Health | ServiceRecovered | Health recovery | On change |
| M09 Mesh | CircuitBreakerPolicy | Policy update | On change |
| M10 Traffic | TrafficHealthReport | Traffic analysis | Periodic |
| M11 Load Balancer | EndpointSelection | Each request | Per-request |
| M11 Load Balancer | EndpointEjectionNotice | Ejection event | On event |
| L3 Learning | ThresholdRecommendation | ML analysis | Periodic |
| L3 Learning | PatternDetected | Pattern matched | On detection |
| L5 Remediation | ForceCircuitState | Manual override | On action |
| L5 Remediation | CircuitRemediationAction | Remediation | On action |

#### Inbound Sequence Diagram

```
  L1:Config   M07:Health   M09:Mesh   M10:Traffic   M11:LB   L3:Learning   L5:Remed
      |           |            |           |          |           |           |
      | ConfigUpdate           |           |          |           |           |
      |---------->|            |           |          |           |           |
      |           |            |           |          |           |           |
      |           | ServiceUnhealthy       |          |           |           |
      |           |----------->|           |          |           |           |
      |           |            |           |          |           |           |
      |           |            | CBPolicy  |          |           |           |
      |           |            |---------->|          |           |           |
      |           |            |           |          |           |           |
      |           |            |           | HealthReport         |           |
      |           |            |           |--------->|           |           |
      |           |            |           |          |           |           |
      |           |            |           |          | Selection |           |
      |           |            |           |          |---------->|           |
      |           |            |           |          |           |           |
      |           |            |           |          |           | ThresholdRec
      |           |            |           |          |           |---------->|
      |           |            |           |          |           |           |
      |           |            |           |          |           |    ForceState
      |           |            |           |          |           |<----------|
      |           |            |           |          |           |           |
      +-----------+------------+-----------+----------+-----------+-----------+
                                       |
                                       v
                             +-------------------+
                             |  M12 CIRCUIT      |
                             |  BREAKER          |
                             |                   |
                             | - Update state    |
                             | - Apply thresholds|
                             | - Execute fallback|
                             +-------------------+
```

### Outbound Data Flow

The Circuit Breaker emits state changes, metrics, and events.

#### Outbound Message Types

```rust
/// Messages emitted by Circuit Breaker
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CircuitBreakerOutbound {
    // To L1 Metrics (M04)
    CircuitMetrics {
        circuit_id: CircuitId,
        state: CircuitState,
        failure_rate: f64,
        requests_allowed: u64,
        requests_blocked: u64,
        fallback_count: u64,
        timestamp: DateTime<Utc>,
    },

    // To L1 State (M05)
    StatePersist {
        circuits: Vec<CircuitBreaker>,
        timestamp: DateTime<Utc>,
    },

    // To M09 Mesh Controller
    CircuitStateChanged {
        circuit_id: CircuitId,
        target: CircuitTarget,
        old_state: CircuitState,
        new_state: CircuitState,
        reason: TransitionReason,
        timestamp: DateTime<Utc>,
    },

    // To M10 Traffic Manager
    CircuitOpened {
        circuit_id: CircuitId,
        target: CircuitTarget,
        duration: Duration,
        fallback_available: bool,
        timestamp: DateTime<Utc>,
    },

    CircuitClosed {
        circuit_id: CircuitId,
        target: CircuitTarget,
        recovery_duration: Duration,
        timestamp: DateTime<Utc>,
    },

    // To L3 Learning
    CircuitEvent {
        event_type: CircuitEventType,
        circuit_id: CircuitId,
        details: CircuitEventDetails,
        timestamp: DateTime<Utc>,
    },

    FailureData {
        circuit_id: CircuitId,
        failure_window: Vec<RequestRecord>,
        state_transitions: Vec<StateTransition>,
        timestamp: DateTime<Utc>,
    },

    // To L5 Remediation
    CircuitAlert {
        alert_type: CircuitAlertType,
        circuit_id: CircuitId,
        severity: Severity,
        details: String,
        stats: CircuitStats,
        suggested_action: Option<CircuitRemediationAction>,
        timestamp: DateTime<Utc>,
    },

    CircuitRecoveryFailed {
        circuit_id: CircuitId,
        attempts: u32,
        last_error: String,
        timestamp: DateTime<Utc>,
    },

    // To L6 Integration
    CircuitStatusExport {
        circuits: Vec<CircuitStatus>,
        timestamp: DateTime<Utc>,
    },

    // Broadcast to subscribers
    CircuitEventBroadcast {
        event: CircuitEvent,
        timestamp: DateTime<Utc>,
    },
}

/// Circuit event types for L3 learning
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CircuitEventType {
    StateTransition { from: CircuitState, to: CircuitState },
    ThresholdExceeded { failure_rate: f64, threshold: f64 },
    RecoveryStarted,
    RecoverySucceeded { duration: Duration },
    RecoveryFailed { attempts: u32 },
    FallbackExecuted { fallback_type: FallbackType },
    ManualOverride { action: String },
    AdaptiveAdjustment { old_threshold: f64, new_threshold: f64 },
}

/// Circuit alert types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CircuitAlertType {
    CircuitOpened,
    RecoveryFailed,
    HighFailureRate,
    CascadeRisk,
    ThresholdReached,
    ProlongedOpen,
    FallbackFailed,
}

/// Circuit status for export
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitStatus {
    pub circuit_id: CircuitId,
    pub target: CircuitTarget,
    pub state: CircuitState,
    pub failure_rate: f64,
    pub last_transition: DateTime<Utc>,
    pub stats: CircuitStats,
}
```

#### Outbound Flow Targets

| Target | Message Type | Trigger Condition | Priority |
|--------|-------------|-------------------|----------|
| L1 Metrics (M04) | CircuitMetrics | Periodic collection | Normal |
| L1 State (M05) | StatePersist | State change | High |
| M09 Mesh | CircuitStateChanged | State transition | High |
| M10 Traffic | CircuitOpened/Closed | State transition | Critical |
| L3 Learning | CircuitEvent | Significant events | Normal |
| L3 Learning | FailureData | Analysis window | Normal |
| L5 Remediation | CircuitAlert | Threshold breached | Critical |
| L5 Remediation | CircuitRecoveryFailed | Recovery failed | Critical |
| L6 Integration | CircuitStatusExport | Periodic/on-demand | Low |
| Subscribers | CircuitEventBroadcast | All events | Varies |

#### Outbound Sequence Diagram

```
                             +-------------------+
                             |  M12 CIRCUIT      |
                             |  BREAKER          |
                             +--------+----------+
                                      |
       +------------------------------+------------------------------+
       |              |               |               |              |
       v              v               v               v              v
  +---------+   +---------+     +---------+     +---------+    +---------+
  |L1:Metrics|  |L1:State |     |M09:Mesh |     |M10:Traffic|   |L3:Learn |
  +---------+   +---------+     +---------+     +---------+    +---------+
       |              |               |               |              |
       |              |               |               |              |
       +------------------------------+------------------------------+
                                      |
       +------------------------------+------------------------------+
       |              |               |               |              |
       v              v               v               v              v
  +---------+   +---------+     +---------+     +---------+    +---------+
  |L5:Remed |   |L6:Integr|    |Subscribers|   |Dashboard |    |Alerts   |
  +---------+   +---------+     +---------+     +---------+    +---------+
```

### Cross-Module Dependencies

#### Bidirectional Dependency Matrix

| Module | Reads From M12 | Writes To M12 | Sync/Async | Error Path |
|--------|---------------|---------------|------------|------------|
| M02 Config | - | ConfigUpdate | Async | Use defaults |
| M04 Metrics | CircuitMetrics | - | Async | Skip metrics |
| M05 State | StatePersist | StateRestored | Async | Start fresh |
| M07 Health | - | ServiceUnhealthy/Recovered | Async | Ignore |
| M09 Mesh | CircuitStateChanged | CircuitBreakerPolicy | Async | Default policy |
| M10 Traffic | CircuitOpened/Closed | TrafficHealthReport | Async | No CB |
| M11 Load Balancer | - | EndpointSelection, EjectionNotice | Async | No CB |
| L3 Learning | CircuitEvent, FailureData | ThresholdRec, PatternDetected | Async | Static threshold |
| L5 Remediation | CircuitAlert, RecoveryFailed | ForceState, RemediationAction | Async | Manual alert |

#### Communication Patterns

```rust
/// Communication patterns for M12
pub struct CircuitBreakerComms {
    // Synchronous execution check (hot path)
    execution_check: ExecutionChecker,

    // Asynchronous events
    async_events: AsyncEventEmitter,

    // State change notifications
    state_notifier: StateNotifier,

    // Subscription management
    subscribers: SubscriberManager,
}

impl CircuitBreakerComms {
    /// Synchronous: Check if request allowed (hot path)
    pub fn allows_request(&self, circuit_id: &CircuitId) -> bool {
        // Must be fast - simple state check
        self.execution_check.allows(circuit_id)
    }

    /// Synchronous: Record result (hot path)
    pub fn record_result(&mut self, circuit_id: &CircuitId, success: bool, latency: Duration) {
        // Update failure tracker in-place
        self.execution_check.record(circuit_id, success, latency);
    }

    /// Asynchronous: Emit circuit events
    pub async fn emit_event(&self, event: CircuitBreakerOutbound) {
        self.async_events.publish(event).await;
    }

    /// Notification: State change broadcast
    pub async fn notify_state_change(&self, circuit_id: &CircuitId, from: CircuitState, to: CircuitState) {
        self.state_notifier.notify(circuit_id, from, to).await;
        self.subscribers.broadcast(CircuitEvent::StateChanged { circuit_id, from, to });
    }
}
```

#### Error Propagation Paths

```
M12 Circuit Breaker Error
       |
       +---> [Log to M03 Logging] ---> Structured log entry
       |
       +---> [Encode via M01 Error Taxonomy] ---> 11D Error Vector
       |            |
       |            +---> [Send to L3 Learning] ---> Pattern recognition
       |
       +---> [Alert to L5 Remediation] ---> Trigger CB remediation
       |
       +---> [Notify M10 Traffic Manager] ---> Adjust routing
       |
       +---> [Execute Fallback] ---> Return degraded response
```

### Contextual Flow: Circuit Breaker State Machine

#### State Machine Transitions

```
                    +--------------------------------------------------+
                    |                                                  |
                    |              +-------------------+               |
                    |              |      CLOSED       |               |
                    |              |                   |               |
                    |              | Normal operation  |               |
                    |              | Requests allowed  |               |
                    |              | Track failures    |               |
                    |              +--------+----------+               |
                    |                       |                          |
                    |    Failure threshold  | exceeded                 |
                    |    (count or %)       |                          |
                    |                       v                          |
                    |              +-------------------+               |
            Success |              |       OPEN        |               |
            during  |              |                   |               |
            probe   |              | Requests blocked  |               |
         +----------+              | Return error/     |               |
         |                         | fallback          |               |
         |                         | Start timer       |               |
         |                         +--------+----------+               |
         |                                  |                          |
         |                  Recovery timeout| expires                  |
         |                                  |                          |
         |                                  v                          |
         |                         +-------------------+               |
         |                         |    HALF_OPEN      |               |
         +-------------------------|                   |               |
                                   | Allow probe       |               |
                                   | requests          |               |
                                   | Test recovery     |               |
                                   +--------+----------+               |
                                            |                          |
                                Probe fails | (any failure)            |
                                            |                          |
                                            +------------------------->+
                                                    Back to OPEN


    Manual override can force transition to any state at any time
```

#### Data Lifecycle Within Module

```rust
/// Circuit breaker request lifecycle
impl CircuitBreakerService {
    /// Execute request through circuit breaker (main entry point)
    pub async fn execute_lifecycle<F, T>(&self, circuit_id: &CircuitId, f: F) -> Result<T, CircuitBreakerError>
    where
        F: Future<Output = Result<T, ServiceError>>,
    {
        let circuit = self.registry.get(circuit_id)
            .ok_or(CircuitBreakerError::CircuitNotFound)?;

        // 1. STATE CHECK: Is the circuit allowing requests?
        match circuit.state {
            CircuitState::Closed => {
                // Normal operation - execute request
            }
            CircuitState::Open => {
                // Circuit open - check if recovery timeout expired
                if self.recovery_manager.should_probe(circuit) {
                    self.transition_to_half_open(circuit_id)?;
                    // Fall through to execute probe
                } else {
                    // Still in open state - execute fallback
                    return self.execute_fallback(circuit_id);
                }
            }
            CircuitState::HalfOpen => {
                // Allow probe request if under limit
                if !self.recovery_manager.can_probe(circuit) {
                    return self.execute_fallback(circuit_id);
                }
            }
        }

        // 2. EXECUTE: Run the actual request with timeout
        let start = Instant::now();
        let result = timeout(circuit.config.request_timeout, f).await;
        let latency = start.elapsed();

        // 3. RECORD: Update failure tracker based on result
        match &result {
            Ok(Ok(_)) => {
                self.record_success(circuit_id, latency)?;
            }
            Ok(Err(e)) => {
                let error_type = self.failure_detector.classify_error(e);
                if self.failure_detector.is_tripable(&error_type, &circuit.config) {
                    self.record_failure(circuit_id, error_type, latency)?;
                }
            }
            Err(_timeout) => {
                self.record_failure(circuit_id, TripableError::Timeout, latency)?;
            }
        }

        // 4. EVALUATE: Check if state transition needed
        self.evaluate_state_transition(circuit_id)?;

        // 5. RETURN: Return result to caller
        result.map_err(|_| CircuitBreakerError::Timeout)?
              .map_err(CircuitBreakerError::ServiceError)
    }

    /// State transition evaluation
    fn evaluate_state_transition(&mut self, circuit_id: &CircuitId) -> Result<(), CircuitBreakerError> {
        let circuit = self.registry.get(circuit_id).unwrap();

        match circuit.state {
            CircuitState::Closed => {
                // Check if we should open
                if self.failure_detector.is_threshold_exceeded(circuit_id) {
                    self.transition_to_open(circuit_id, TransitionReason::FailureThresholdExceeded {
                        failures: circuit.failure_tracker.failure_count,
                        threshold: self.get_threshold_value(&circuit.config.failure_threshold),
                    })?;
                }
            }
            CircuitState::HalfOpen => {
                // Check probe results
                if circuit.recovery_tracker.successful_probes >= circuit.config.success_threshold {
                    // Enough successes - close circuit
                    self.transition_to_closed(circuit_id, TransitionReason::ProbeSuccess {
                        successes: circuit.recovery_tracker.successful_probes,
                        required: circuit.config.success_threshold,
                    })?;
                } else if circuit.recovery_tracker.failed_probes > 0 {
                    // Any failure - reopen circuit
                    self.transition_to_open(circuit_id, TransitionReason::ProbeFailed)?;
                }
            }
            CircuitState::Open => {
                // State is managed by recovery timeout
            }
        }

        Ok(())
    }
}
```

---

## Metrics

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `me_m12_circuit_state` | Gauge | circuit, target | Current circuit state (0=closed, 1=open, 2=half_open) |
| `me_m12_requests_total` | Counter | circuit, result | Total requests (allowed/blocked) |
| `me_m12_failures_total` | Counter | circuit, error_type | Total failures by type |
| `me_m12_fallback_total` | Counter | circuit, result | Fallback executions |
| `me_m12_state_transitions` | Counter | circuit, from, to | State transition count |
| `me_m12_failure_rate` | Gauge | circuit | Current failure rate |
| `me_m12_open_duration_seconds` | Histogram | circuit | Time spent in open state |
| `me_m12_recovery_attempts` | Counter | circuit, result | Recovery attempt count |
| `me_m12_probe_success_rate` | Gauge | circuit | Probe success rate |
| `me_m12_active_circuits` | Gauge | state | Number of circuits by state |

---

## Error Codes

| Code | Name | Severity | Description | Recovery |
|------|------|----------|-------------|----------|
| E12001 | CircuitNotFound | Warning | Circuit ID not found | Create circuit |
| E12002 | CircuitOpen | Info | Circuit is open, request blocked | Use fallback |
| E12003 | CircuitHalfOpenLimit | Info | Half-open probe limit reached | Wait for probes |
| E12004 | ThresholdExceeded | Warning | Failure threshold exceeded | Monitor recovery |
| E12005 | FallbackFailed | Error | Fallback execution failed | Return error |
| E12006 | RecoveryFailed | Warning | Recovery attempt failed | Extend timeout |
| E12007 | ConfigInvalid | Error | Invalid circuit configuration | Fix config |
| E12008 | TransitionInvalid | Error | Invalid state transition | Reset circuit |
| E12009 | TimeoutExceeded | Warning | Request timeout exceeded | Record failure |
| E12010 | CascadeDetected | Critical | Cascade failure detected | Alert L5 |

---

## Navigation

| Direction | Link |
|-----------|------|
| Up | [INDEX.md](../INDEX.md) |
| Layer | [L02_SERVICES.md](../layers/L02_SERVICES.md) |
| Previous | [M11_LOAD_BALANCER.md](M11_LOAD_BALANCER.md) |
| Related | [M07_HEALTH_MONITOR.md](M07_HEALTH_MONITOR.md) |
| Pattern | [PATTERN_001_CIRCUIT_BREAKER.md](../patterns/PATTERN_001_CIRCUIT_BREAKER.md) |
| L5 Remediation | [L05_REMEDIATION.md](../layers/L05_REMEDIATION.md) |
| API | [REST_API.md](../integration/REST_API.md) |

---

*[Back to Index](../INDEX.md) | [Layer: L02 Services](../layers/L02_SERVICES.md)*
