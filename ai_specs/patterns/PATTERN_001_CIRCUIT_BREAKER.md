# Pattern 001: Circuit Breaker

> **PATTERN_001_CIRCUIT_BREAKER** | Fault Tolerance Pattern | [Back to Index](../INDEX.md)

---

## Navigation

| Direction | Link |
|-----------|------|
| Up | [INDEX.md](../INDEX.md) |
| Layer | [L02_SERVICES.md](../layers/L02_SERVICES.md) |
| Related | [L05_REMEDIATION.md](../layers/L05_REMEDIATION.md) |
| Module | [M01_ERROR_TAXONOMY.md](../modules/M01_ERROR_TAXONOMY.md) |

---

## Pattern Overview

The Circuit Breaker pattern prevents cascading failures by detecting failures and temporarily stopping requests to failing services. It allows the system to fail fast, recover gracefully, and maintain overall system stability.

### Pattern Properties

| Property | Value |
|----------|-------|
| Pattern ID | PATTERN_001 |
| Pattern Name | Circuit Breaker |
| Category | Fault Tolerance |
| Layers | L2 (Services), L5 (Remediation) |
| States | CLOSED, OPEN, HALF_OPEN |

---

## State Machine

```
                    ┌──────────────────────────────────────────┐
                    │                                          │
                    │    ┌────────────────────────────────┐    │
                    │    │                                │    │
                    │    │           CLOSED               │    │
                    │    │                                │    │
                    │    │  - Normal operation            │    │
                    │    │  - Requests pass through       │    │
                    │    │  - Track failure count         │    │
                    │    │                                │    │
                    │    └───────────────┬────────────────┘    │
                    │                    │                     │
                    │                    │ Failure threshold   │
                    │                    │ exceeded            │
                    │                    │                     │
                    │                    v                     │
                    │    ┌────────────────────────────────┐    │
          Success   │    │                                │    │
          on probe  │    │            OPEN                │    │
                    │    │                                │    │
┌───────────────────┤    │  - Block all requests         │    │
│                   │    │  - Return error immediately   │    │
│                   │    │  - Start recovery timer       │    │
│                   │    │                                │    │
│                   │    └───────────────┬────────────────┘    │
│                   │                    │                     │
│                   │                    │ Recovery timeout    │
│                   │                    │ expires             │
│                   │                    │                     │
│                   │                    v                     │
│                   │    ┌────────────────────────────────┐    │
│                   │    │                                │    │
│                   └────│         HALF_OPEN              │    │
│                        │                                │    │
│                        │  - Allow limited requests      │    │
│                        │  - Probe service health        │    │
│                        │  - Track probe results         │    │
│                        │                                │    │
│                        └───────────────┬────────────────┘    │
│                                        │                     │
│                                        │ Probe fails         │
│                                        │                     │
└────────────────────────────────────────┴─────────────────────┘
                                         │
                                         v
                                  Back to OPEN
```

---

## States

### CLOSED (Normal Operation)

In the CLOSED state, the circuit breaker allows all requests to pass through to the downstream service.

```rust
pub struct ClosedState {
    /// Number of consecutive failures
    failure_count: u32,

    /// Number of consecutive successes
    success_count: u32,

    /// Window for counting failures
    failure_window: Duration,

    /// Failures within the current window
    failures_in_window: VecDeque<Instant>,
}

impl ClosedState {
    pub fn record_success(&mut self) {
        self.success_count += 1;
        self.failure_count = 0;
    }

    pub fn record_failure(&mut self) {
        self.failure_count += 1;
        self.success_count = 0;
        self.failures_in_window.push_back(Instant::now());
        self.prune_old_failures();
    }

    pub fn should_open(&self, config: &CircuitBreakerConfig) -> bool {
        self.failure_count >= config.failure_threshold
            || self.failures_in_window.len() >= config.failure_count_threshold
    }
}
```

**Behavior:**
- All requests pass through
- Track success/failure counts
- Monitor failure rate within sliding window
- Transition to OPEN when failure threshold exceeded

---

### OPEN (Circuit Tripped)

In the OPEN state, the circuit breaker blocks all requests and returns an error immediately without attempting to call the downstream service.

```rust
pub struct OpenState {
    /// When the circuit opened
    opened_at: Instant,

    /// When to attempt recovery
    recovery_timeout: Duration,

    /// Error to return while open
    open_error: CircuitOpenError,
}

impl OpenState {
    pub fn should_attempt_recovery(&self) -> bool {
        self.opened_at.elapsed() >= self.recovery_timeout
    }

    pub fn get_error(&self) -> CircuitOpenError {
        self.open_error.clone()
    }
}
```

**Behavior:**
- Block all requests immediately
- Return `CircuitOpenError` without calling downstream
- Wait for recovery timeout
- Transition to HALF_OPEN after timeout

---

### HALF_OPEN (Testing Recovery)

In the HALF_OPEN state, the circuit breaker allows a limited number of probe requests to test if the downstream service has recovered.

```rust
pub struct HalfOpenState {
    /// Maximum probe requests to allow
    max_probe_requests: u32,

    /// Current probe request count
    probe_count: u32,

    /// Successful probes
    success_count: u32,

    /// Failed probes
    failure_count: u32,

    /// When we entered half-open
    entered_at: Instant,
}

impl HalfOpenState {
    pub fn can_probe(&self) -> bool {
        self.probe_count < self.max_probe_requests
    }

    pub fn record_probe_success(&mut self) {
        self.probe_count += 1;
        self.success_count += 1;
    }

    pub fn record_probe_failure(&mut self) {
        self.probe_count += 1;
        self.failure_count += 1;
    }

    pub fn should_close(&self, config: &CircuitBreakerConfig) -> bool {
        self.success_count >= config.success_threshold_to_close
    }

    pub fn should_reopen(&self) -> bool {
        self.failure_count > 0
    }
}
```

**Behavior:**
- Allow limited probe requests
- Track probe success/failure
- Transition to CLOSED if probes succeed
- Transition back to OPEN if any probe fails

---

## Implementation

### Circuit Breaker Structure

```rust
pub struct CircuitBreaker {
    /// Unique identifier
    id: CircuitBreakerId,

    /// Target service
    service: ServiceId,

    /// Current state
    state: RwLock<CircuitState>,

    /// Configuration
    config: CircuitBreakerConfig,

    /// Metrics
    metrics: CircuitBreakerMetrics,

    /// Event emitter
    events: EventEmitter<CircuitBreakerEvent>,
}

#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    /// Failures before opening circuit
    pub failure_threshold: u32,

    /// Time window for failure counting
    pub failure_window: Duration,

    /// Maximum failures in window
    pub failure_count_threshold: usize,

    /// Time to wait before attempting recovery
    pub recovery_timeout: Duration,

    /// Probe requests in half-open state
    pub half_open_max_requests: u32,

    /// Successes needed to close circuit
    pub success_threshold_to_close: u32,

    /// Timeout for individual requests
    pub request_timeout: Duration,

    /// Errors that should trip the circuit
    pub tripable_errors: Vec<ErrorCategory>,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            failure_window: Duration::from_secs(60),
            failure_count_threshold: 10,
            recovery_timeout: Duration::from_secs(30),
            half_open_max_requests: 3,
            success_threshold_to_close: 2,
            request_timeout: Duration::from_secs(5),
            tripable_errors: vec![
                ErrorCategory::Infrastructure,
                ErrorCategory::Application,
            ],
        }
    }
}
```

### Circuit Breaker API

```rust
impl CircuitBreaker {
    /// Create new circuit breaker
    pub fn new(service: ServiceId, config: CircuitBreakerConfig) -> Self;

    /// Execute a request through the circuit breaker
    pub async fn execute<F, T>(&self, f: F) -> Result<T, CircuitBreakerError>
    where
        F: Future<Output = Result<T, ServiceError>>;

    /// Get current state
    pub fn state(&self) -> CircuitState;

    /// Check if circuit is allowing requests
    pub fn is_closed(&self) -> bool;

    /// Check if circuit is blocking requests
    pub fn is_open(&self) -> bool;

    /// Force circuit to open
    pub fn force_open(&self);

    /// Force circuit to close
    pub fn force_close(&self);

    /// Reset circuit to initial state
    pub fn reset(&self);

    /// Get metrics
    pub fn metrics(&self) -> &CircuitBreakerMetrics;

    /// Subscribe to state change events
    pub fn on_state_change(&self) -> impl Stream<Item = CircuitBreakerEvent>;
}
```

### Execute Method Implementation

```rust
impl CircuitBreaker {
    pub async fn execute<F, T>(&self, f: F) -> Result<T, CircuitBreakerError>
    where
        F: Future<Output = Result<T, ServiceError>>,
    {
        // Check if request is allowed
        let can_execute = {
            let state = self.state.read().await;
            match &*state {
                CircuitState::Closed(_) => true,
                CircuitState::Open(s) => {
                    if s.should_attempt_recovery() {
                        // Transition to half-open
                        drop(state);
                        self.transition_to_half_open().await;
                        true
                    } else {
                        false
                    }
                }
                CircuitState::HalfOpen(s) => s.can_probe(),
            }
        };

        if !can_execute {
            self.metrics.rejected.inc();
            return Err(CircuitBreakerError::CircuitOpen);
        }

        // Execute the request with timeout
        let result = timeout(self.config.request_timeout, f).await;

        match result {
            Ok(Ok(response)) => {
                self.record_success().await;
                Ok(response)
            }
            Ok(Err(e)) if self.is_tripable_error(&e) => {
                self.record_failure().await;
                Err(CircuitBreakerError::ServiceError(e))
            }
            Ok(Err(e)) => {
                // Non-tripable error, don't affect circuit state
                Err(CircuitBreakerError::ServiceError(e))
            }
            Err(_) => {
                self.record_failure().await;
                Err(CircuitBreakerError::Timeout)
            }
        }
    }

    async fn record_success(&self) {
        let mut state = self.state.write().await;
        match &mut *state {
            CircuitState::Closed(s) => {
                s.record_success();
            }
            CircuitState::HalfOpen(s) => {
                s.record_probe_success();
                if s.should_close(&self.config) {
                    *state = CircuitState::Closed(ClosedState::default());
                    self.emit_event(CircuitBreakerEvent::Closed);
                }
            }
            CircuitState::Open(_) => {}
        }
        self.metrics.successes.inc();
    }

    async fn record_failure(&self) {
        let mut state = self.state.write().await;
        match &mut *state {
            CircuitState::Closed(s) => {
                s.record_failure();
                if s.should_open(&self.config) {
                    *state = CircuitState::Open(OpenState::new(self.config.recovery_timeout));
                    self.emit_event(CircuitBreakerEvent::Opened);
                }
            }
            CircuitState::HalfOpen(s) => {
                s.record_probe_failure();
                if s.should_reopen() {
                    *state = CircuitState::Open(OpenState::new(self.config.recovery_timeout));
                    self.emit_event(CircuitBreakerEvent::Reopened);
                }
            }
            CircuitState::Open(_) => {}
        }
        self.metrics.failures.inc();
    }
}
```

---

## Usage Examples

### Basic Usage

```rust
// Create circuit breaker for a service
let cb = CircuitBreaker::new(
    ServiceId::from("payment-service"),
    CircuitBreakerConfig::default(),
);

// Execute request through circuit breaker
let result = cb.execute(async {
    payment_service.process_payment(order).await
}).await;

match result {
    Ok(response) => {
        println!("Payment processed: {:?}", response);
    }
    Err(CircuitBreakerError::CircuitOpen) => {
        println!("Circuit is open, using fallback");
        // Use cached result or default behavior
    }
    Err(CircuitBreakerError::ServiceError(e)) => {
        println!("Service error: {:?}", e);
    }
    Err(CircuitBreakerError::Timeout) => {
        println!("Request timed out");
    }
}
```

### With Custom Configuration

```rust
let config = CircuitBreakerConfig {
    failure_threshold: 3,
    failure_window: Duration::from_secs(30),
    failure_count_threshold: 5,
    recovery_timeout: Duration::from_secs(60),
    half_open_max_requests: 2,
    success_threshold_to_close: 2,
    request_timeout: Duration::from_secs(10),
    tripable_errors: vec![
        ErrorCategory::Infrastructure,
        ErrorCategory::Performance,
    ],
};

let cb = CircuitBreaker::new(ServiceId::from("database"), config);
```

### Listening to State Changes

```rust
let cb = CircuitBreaker::new(service_id, config);

// Subscribe to state changes
let mut events = cb.on_state_change();

tokio::spawn(async move {
    while let Some(event) = events.next().await {
        match event {
            CircuitBreakerEvent::Opened => {
                alert_team("Circuit opened for service");
            }
            CircuitBreakerEvent::Closed => {
                log::info!("Circuit closed, service recovered");
            }
            CircuitBreakerEvent::HalfOpen => {
                log::info!("Testing service recovery");
            }
            CircuitBreakerEvent::Reopened => {
                log::warn!("Service still unhealthy, reopening circuit");
            }
        }
    }
});
```

---

## Integration with Maintenance Engine

### L2 Services Layer

The circuit breaker is integrated into the L2 Services layer for health monitoring:

```rust
impl HealthMonitor {
    pub async fn check_with_circuit_breaker(&self, service: &ServiceId) -> HealthResult {
        let cb = self.circuit_breakers.get(service);

        if let Some(cb) = cb {
            if cb.is_open() {
                return HealthResult::unhealthy("Circuit breaker open");
            }

            cb.execute(async {
                self.perform_health_check(service).await
            }).await.unwrap_or_else(|e| {
                HealthResult::unhealthy(format!("Health check failed: {:?}", e))
            })
        } else {
            self.perform_health_check(service).await
        }
    }
}
```

### L5 Remediation Layer

Circuit breaker opening triggers L0 remediation:

```rust
impl RemediationEngine {
    pub async fn handle_circuit_breaker_event(&self, event: CircuitBreakerEvent) {
        match event {
            CircuitBreakerEvent::Opened { service, .. } => {
                // Create L0 remediation action
                let action = RemediationAction {
                    tier: EscalationTier::L0,
                    action_type: ActionType::CircuitBreak {
                        duration: Duration::from_secs(30),
                    },
                    target: Target::Service(service),
                    ..Default::default()
                };

                self.execute(action).await;
            }
            _ => {}
        }
    }
}
```

---

## Metrics

| Metric | Type | Description |
|--------|------|-------------|
| `me_circuit_breaker_state` | Gauge | Current state (0=closed, 1=open, 2=half-open) |
| `me_circuit_breaker_requests_total` | Counter | Total requests by result |
| `me_circuit_breaker_failures_total` | Counter | Total failures |
| `me_circuit_breaker_successes_total` | Counter | Total successes |
| `me_circuit_breaker_rejected_total` | Counter | Requests rejected (circuit open) |
| `me_circuit_breaker_state_changes` | Counter | State transitions by type |

---

## Configuration

```toml
[layer.L2.circuit_breaker]
enabled = true
default_failure_threshold = 5
default_failure_window_ms = 60000
default_recovery_timeout_ms = 30000
default_half_open_requests = 3
default_request_timeout_ms = 5000

[[layer.L2.circuit_breaker.overrides]]
service = "payment-service"
failure_threshold = 3
recovery_timeout_ms = 60000

[[layer.L2.circuit_breaker.overrides]]
service = "notification-service"
failure_threshold = 10
recovery_timeout_ms = 10000
```

---

## Best Practices

1. **Set appropriate thresholds**: Too low causes unnecessary trips; too high delays failure detection
2. **Use sliding windows**: Better than simple counters for detecting intermittent issues
3. **Implement fallbacks**: Always have degraded operation mode when circuit is open
4. **Monitor metrics**: Alert on circuit state changes
5. **Test circuit behavior**: Regularly verify circuit trips and recovers correctly
6. **Differentiate errors**: Not all errors should trip the circuit
7. **Consider request priority**: Critical requests may bypass circuit in emergencies

---

## Navigation

| Direction | Link |
|-----------|------|
| Up | [INDEX.md](../INDEX.md) |
| Layer | [L02_SERVICES.md](../layers/L02_SERVICES.md) |
| Remediation | [L05_REMEDIATION.md](../layers/L05_REMEDIATION.md) |
| Module | [M01_ERROR_TAXONOMY.md](../modules/M01_ERROR_TAXONOMY.md) |
| API | [REST_API.md](../integration/REST_API.md) |

---

*[Back to Index](../INDEX.md) | [L02 Services](../layers/L02_SERVICES.md)*
