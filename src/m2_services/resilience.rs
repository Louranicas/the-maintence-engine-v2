//! # M12: Resilience Manager
//!
//! Circuit breaker registry and load balancer for the ULTRAPLATE Developer
//! Environment. Merges the former `circuit.rs` and `balancer.rs` into a
//! unified resilience module with trait-based abstractions.
//!
//! ## Layer: L2 (Services)
//! ## Module: M12
//! ## Dependencies: L1 (Error, Timestamp, `ModuleId`, `SignalBus`, `TensorContributor`)
//!
//! ## Traits
//!
//! | Trait | Purpose |
//! |-------|---------|
//! | [`CircuitBreakerOps`] | Circuit breaker state machine |
//! | [`LoadBalancing`] | Endpoint pool management |
//!
//! All methods are `&self` (C2) with interior mutability via `RwLock`.
//! Methods returning data through `RwLock` return OWNED types (C7).
//!
//! ## Circuit Breaker State Machine
//!
//! ```text
//! Closed --[failures >= threshold]--> Open
//! Open   --[timeout elapsed]-------> HalfOpen
//! HalfOpen --[successes >= threshold]--> Closed
//! HalfOpen --[any failure]-----------> Open
//! ```
//!
//! ## 12D Tensor Contribution (C3)
//!
//! | Dimension | Value |
//! |-----------|-------|
//! | D9 (latency) | 1.0 - (`open_breakers` / `total_breakers`) |
//! | D10 (`error_rate`) | average failure rate across breakers |
//!
//! ## Signal Emission (C6)
//!
//! - Closed→Open: emits degradation signal
//! - HalfOpen→Closed: emits improvement signal

use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::RwLock;

use super::CircuitState;
use crate::m1_foundation::shared_types::{CoverageBitmap, DimensionIndex, ModuleId, Timestamp};
use crate::m1_foundation::signals::{HealthSignal, SignalBus};
use crate::m1_foundation::tensor_registry::{ContributedTensor, ContributorKind, TensorContributor};
use crate::m1_foundation::MetricsRegistry;
use crate::{Error, Result, Tensor12D};

// ============================================================================
// CircuitBreakerOps (trait)
// ============================================================================

/// Circuit breaker operations trait.
///
/// All methods are `&self` (C2). State mutation uses interior mutability.
/// Methods returning data through `RwLock` return owned types (C7).
pub trait CircuitBreakerOps: Send + Sync + fmt::Debug {
    /// Register a circuit breaker with custom configuration.
    ///
    /// # Errors
    /// Returns `Error::Validation` if a breaker is already registered for this service.
    fn register_breaker(&self, service_id: &str, config: CircuitBreakerConfig) -> Result<()>;
    /// Register a circuit breaker with default configuration.
    ///
    /// # Errors
    /// Returns `Error::Validation` if a breaker is already registered for this service.
    fn register_default(&self, service_id: &str) -> Result<()>;
    /// Remove a circuit breaker.
    ///
    /// # Errors
    /// Returns `Error::ServiceNotFound` if no breaker is registered for this service.
    fn deregister_breaker(&self, service_id: &str) -> Result<()>;
    /// Record a successful request.
    ///
    /// # Errors
    /// Returns `Error::ServiceNotFound` if no breaker is registered for this service.
    fn record_success(&self, service_id: &str) -> Result<CircuitState>;
    /// Record a failed request.
    ///
    /// # Errors
    /// Returns `Error::ServiceNotFound` if no breaker is registered for this service.
    fn record_failure(&self, service_id: &str) -> Result<CircuitState>;
    /// Check whether a request should be allowed.
    ///
    /// # Errors
    /// Returns `Error::ServiceNotFound` if no breaker is registered for this service.
    fn allow_request(&self, service_id: &str) -> Result<bool>;
    /// Get the current circuit state.
    ///
    /// # Errors
    /// Returns `Error::ServiceNotFound` if no breaker is registered for this service.
    fn get_state(&self, service_id: &str) -> Result<CircuitState>;
    /// Get statistics snapshot.
    ///
    /// # Errors
    /// Returns `Error::ServiceNotFound` if no breaker is registered for this service.
    fn get_breaker_stats(&self, service_id: &str) -> Result<CircuitBreakerStats>;
    /// Force-reset to Closed state.
    ///
    /// # Errors
    /// Returns `Error::ServiceNotFound` if no breaker is registered for this service.
    fn reset(&self, service_id: &str) -> Result<()>;
    /// Get IDs of all Open circuit breakers.
    fn get_open_circuits(&self) -> Vec<String>;
    /// Get the number of registered breakers.
    fn breaker_count(&self) -> usize;
    /// Check whether a breaker is registered.
    fn is_registered(&self, service_id: &str) -> bool;
}

// ============================================================================
// LoadBalancing (trait)
// ============================================================================

/// Load balancing operations trait.
///
/// All methods are `&self` (C2). Pool mutation uses interior mutability.
/// Endpoint selection returns owned types (C7).
pub trait LoadBalancing: Send + Sync + fmt::Debug {
    /// Create a new endpoint pool.
    ///
    /// # Errors
    /// Returns `Error::Validation` if a pool already exists for this service.
    fn create_pool(&self, service_id: &str, algorithm: LoadBalanceAlgorithm) -> Result<()>;
    /// Remove an endpoint pool.
    ///
    /// # Errors
    /// Returns `Error::ServiceNotFound` if no pool exists for this service.
    fn remove_pool(&self, service_id: &str) -> Result<()>;
    /// Add an endpoint to a pool.
    ///
    /// # Errors
    /// Returns `Error::ServiceNotFound` if the pool does not exist, or
    /// `Error::Validation` if the endpoint is already present.
    fn add_endpoint(&self, service_id: &str, endpoint: Endpoint) -> Result<()>;
    /// Remove an endpoint from a pool.
    ///
    /// # Errors
    /// Returns `Error::ServiceNotFound` if the pool or endpoint does not exist.
    fn remove_endpoint(&self, service_id: &str, endpoint_id: &str) -> Result<()>;
    /// Select an endpoint using the configured algorithm. Returns owned (C7).
    ///
    /// # Errors
    /// Returns `Error::ServiceNotFound` if the pool does not exist or has no
    /// healthy endpoints.
    fn select_endpoint(&self, service_id: &str) -> Result<Endpoint>;
    /// Mark an endpoint as healthy.
    ///
    /// # Errors
    /// Returns `Error::ServiceNotFound` if the pool or endpoint does not exist.
    fn mark_healthy(&self, service_id: &str, endpoint_id: &str) -> Result<()>;
    /// Mark an endpoint as unhealthy.
    ///
    /// # Errors
    /// Returns `Error::ServiceNotFound` if the pool or endpoint does not exist.
    fn mark_unhealthy(&self, service_id: &str, endpoint_id: &str) -> Result<()>;
    /// Record a completed request outcome.
    ///
    /// # Errors
    /// Returns `Error::ServiceNotFound` if the pool or endpoint does not exist.
    fn record_request(&self, service_id: &str, endpoint_id: &str, success: bool) -> Result<()>;
    /// Get aggregate pool statistics.
    ///
    /// # Errors
    /// Returns `Error::ServiceNotFound` if no pool exists for this service.
    fn get_pool_stats(&self, service_id: &str) -> Result<PoolStats>;
    /// Get load distribution across endpoints.
    ///
    /// # Errors
    /// Returns `Error::ServiceNotFound` if no pool exists for this service.
    fn get_load_distribution(&self, service_id: &str) -> Result<Vec<(String, f64)>>;
}

// ============================================================================
// CircuitBreakerConfig
// ============================================================================

/// Configuration for a circuit breaker instance.
#[derive(Clone, Debug)]
pub struct CircuitBreakerConfig {
    /// Number of failures before Closed → Open.
    pub failure_threshold: u32,
    /// Consecutive successes in `HalfOpen` before → Closed.
    pub success_threshold: u32,
    /// Duration to remain Open before → `HalfOpen` (C8: Duration, not ms).
    pub open_timeout: Duration,
    /// Max concurrent requests in `HalfOpen`.
    pub half_open_max_requests: u32,
    /// Sliding window for failure rate computation.
    pub monitoring_window: Duration,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            success_threshold: 3,
            open_timeout: Duration::from_secs(30),
            half_open_max_requests: 1,
            monitoring_window: Duration::from_secs(60),
        }
    }
}

impl CircuitBreakerConfig {
    /// Create a builder.
    #[must_use]
    pub const fn builder() -> CircuitBreakerConfigBuilder {
        CircuitBreakerConfigBuilder::new()
    }
}

/// Builder for [`CircuitBreakerConfig`].
#[derive(Clone, Debug)]
pub struct CircuitBreakerConfigBuilder {
    failure_threshold: u32,
    success_threshold: u32,
    open_timeout: Duration,
    half_open_max_requests: u32,
    monitoring_window: Duration,
}

impl CircuitBreakerConfigBuilder {
    /// Create a new builder with defaults.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            failure_threshold: 5,
            success_threshold: 3,
            open_timeout: Duration::from_secs(30),
            half_open_max_requests: 1,
            monitoring_window: Duration::from_secs(60),
        }
    }

    /// Set failure threshold.
    #[must_use]
    pub const fn failure_threshold(mut self, threshold: u32) -> Self {
        self.failure_threshold = threshold;
        self
    }

    /// Set success threshold.
    #[must_use]
    pub const fn success_threshold(mut self, threshold: u32) -> Self {
        self.success_threshold = threshold;
        self
    }

    /// Set open timeout duration (C8).
    #[must_use]
    pub const fn open_timeout(mut self, timeout: Duration) -> Self {
        self.open_timeout = timeout;
        self
    }

    /// Set max requests in `HalfOpen`.
    #[must_use]
    pub const fn half_open_max_requests(mut self, max: u32) -> Self {
        self.half_open_max_requests = max;
        self
    }

    /// Set monitoring window duration.
    #[must_use]
    pub const fn monitoring_window(mut self, window: Duration) -> Self {
        self.monitoring_window = window;
        self
    }

    /// Build the config.
    #[must_use]
    pub const fn build(self) -> CircuitBreakerConfig {
        CircuitBreakerConfig {
            failure_threshold: self.failure_threshold,
            success_threshold: self.success_threshold,
            open_timeout: self.open_timeout,
            half_open_max_requests: self.half_open_max_requests,
            monitoring_window: self.monitoring_window,
        }
    }
}

impl Default for CircuitBreakerConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// CircuitStateTransition
// ============================================================================

/// Records a single circuit breaker state transition.
#[derive(Clone, Debug)]
pub struct CircuitStateTransition {
    /// State before transition.
    pub from: CircuitState,
    /// State after transition.
    pub to: CircuitState,
    /// Reason for the transition.
    pub reason: String,
    /// Timestamp of the transition (C5: Timestamp, not chrono).
    pub timestamp: Timestamp,
}

impl fmt::Display for CircuitStateTransition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} → {} ({})", self.from, self.to, self.reason)
    }
}

// ============================================================================
// CircuitBreakerEntry (internal)
// ============================================================================

/// Internal circuit breaker state. Not exposed publicly.
#[derive(Debug)]
struct CircuitBreakerEntry {
    config: CircuitBreakerConfig,
    state: CircuitState,
    failure_count: u32,
    success_count: u32,
    consecutive_successes: u32,
    total_requests: u64,
    total_failures: u64,
    last_failure_time: Option<Timestamp>,
    last_success_time: Option<Timestamp>,
    last_state_change: Timestamp,
    /// Monotonic instant for timeout computation (not chrono, not `SystemTime`).
    state_change_instant: Instant,
    state_history: Vec<CircuitStateTransition>,
    /// F-05: Concurrent probe requests admitted while in `HalfOpen`.
    /// Capped by `config.half_open_max_requests`. Decremented on
    /// `record_success`/`record_failure`. Reset to 0 on state transition
    /// away from `HalfOpen` and when entering `HalfOpen` from `Open`.
    in_flight_probes: u32,
}

impl CircuitBreakerEntry {
    fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            config,
            state: CircuitState::Closed,
            failure_count: 0,
            success_count: 0,
            consecutive_successes: 0,
            total_requests: 0,
            total_failures: 0,
            last_failure_time: None,
            last_success_time: None,
            last_state_change: Timestamp::now(),
            state_change_instant: Instant::now(),
            state_history: Vec::new(),
            in_flight_probes: 0,
        }
    }

    fn transition_to(&mut self, new_state: CircuitState, reason: impl Into<String>) {
        let now = Timestamp::now();
        self.state_history.push(CircuitStateTransition {
            from: self.state,
            to: new_state,
            reason: reason.into(),
            timestamp: now,
        });
        self.state = new_state;
        self.last_state_change = now;
        self.state_change_instant = Instant::now();
        // F-05: Any state transition resets the in-flight probe count.
        // Entering `HalfOpen` starts fresh at 0; leaving clears stale accounting.
        self.in_flight_probes = 0;
    }

    fn is_open_timeout_elapsed(&self) -> bool {
        self.state_change_instant.elapsed() >= self.config.open_timeout
    }

    #[allow(clippy::cast_precision_loss)]
    fn failure_rate(&self) -> f64 {
        if self.total_requests == 0 {
            return 0.0;
        }
        self.total_failures as f64 / self.total_requests as f64
    }
}

// ============================================================================
// CircuitBreakerStats
// ============================================================================

/// Read-only statistics snapshot for a circuit breaker.
#[derive(Clone, Debug)]
pub struct CircuitBreakerStats {
    /// Current circuit state.
    pub state: CircuitState,
    /// Current failure count.
    pub failure_count: u32,
    /// Current success count.
    pub success_count: u32,
    /// Total lifetime requests.
    pub total_requests: u64,
    /// Total lifetime failures.
    pub total_failures: u64,
    /// Failure rate (0.0–1.0).
    pub failure_rate: f64,
    /// Last failure timestamp (C5: Timestamp).
    pub last_failure: Option<Timestamp>,
    /// Last state change timestamp (C5: Timestamp).
    pub last_state_change: Timestamp,
}

// ============================================================================
// ProbePermit (ME-001 F-05 regression fix: RAII guard for half_open probe slot)
// ============================================================================

/// RAII guard returned by [`CircuitBreakerRegistry::allow_probe`].
///
/// Dropping the permit without calling [`Self::record_success`] or
/// [`Self::record_failure`] automatically releases the `in_flight_probes` slot,
/// preventing permanent `HalfOpen` lockout when a caller panics between the
/// admission check and the outcome record.
///
/// ME-001 regression fix: the S099 F-05 implementation tracked the probe counter
/// but assumed callers always reach `record_success`/`record_failure`. A panic
/// in caller code leaked the slot; enough leaks and `HalfOpen` denied all traffic.
pub struct ProbePermit<'a> {
    registry: &'a CircuitBreakerRegistry,
    service_id: String,
    consumed: bool,
}

impl ProbePermit<'_> {
    /// Consume the permit and delegate to `record_success`.
    ///
    /// # Errors
    /// Returns `Error::ServiceNotFound` if the breaker was deregistered mid-probe.
    pub fn record_success(mut self) -> Result<CircuitState> {
        self.consumed = true;
        self.registry.record_success(&self.service_id)
    }

    /// Consume the permit and delegate to `record_failure`.
    ///
    /// # Errors
    /// Returns `Error::ServiceNotFound` if the breaker was deregistered mid-probe.
    pub fn record_failure(mut self) -> Result<CircuitState> {
        self.consumed = true;
        self.registry.record_failure(&self.service_id)
    }
}

impl Drop for ProbePermit<'_> {
    fn drop(&mut self) {
        if self.consumed {
            return;
        }
        // Permit leaked (caller panic / early return). Release the slot so the
        // HalfOpen probe quota isn't exhausted by dead permits.
        // `saturating_sub` prevents underflow if a concurrent `transition_to`
        // already reset the counter.
        if let Some(entry) = self.registry.breakers.write().get_mut(&self.service_id) {
            entry.in_flight_probes = entry.in_flight_probes.saturating_sub(1);
        }
    }
}

// ============================================================================
// CircuitBreakerRegistry
// ============================================================================

/// Thread-safe registry of circuit breakers.
pub struct CircuitBreakerRegistry {
    breakers: RwLock<HashMap<String, CircuitBreakerEntry>>,
    signal_bus: Option<Arc<SignalBus>>,
    _metrics: Option<Arc<MetricsRegistry>>,
}

impl fmt::Debug for CircuitBreakerRegistry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CircuitBreakerRegistry")
            .field("breaker_count", &self.breakers.read().len())
            .field("signal_bus", &self.signal_bus)
            .finish_non_exhaustive()
    }
}

impl Default for CircuitBreakerRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl CircuitBreakerRegistry {
    /// Create a new empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            breakers: RwLock::new(HashMap::new()),
            signal_bus: None,
            _metrics: None,
        }
    }

    /// Create with a signal bus for health signal emission.
    #[must_use]
    pub fn with_signal_bus(signal_bus: Arc<SignalBus>) -> Self {
        Self {
            breakers: RwLock::new(HashMap::new()),
            signal_bus: Some(signal_bus),
            _metrics: None,
        }
    }

    /// Create with signal bus and metrics registry.
    #[must_use]
    pub fn with_signal_bus_and_metrics(
        signal_bus: Arc<SignalBus>,
        metrics: Arc<MetricsRegistry>,
    ) -> Self {
        Self {
            breakers: RwLock::new(HashMap::new()),
            signal_bus: Some(signal_bus),
            _metrics: Some(metrics),
        }
    }

    /// Emit a health signal (fire-and-forget, lock must be released first).
    fn emit_signal(&self, previous_health: f64, current_health: f64, reason: String) {
        if let Some(bus) = &self.signal_bus {
            bus.emit_health(&HealthSignal {
                module_id: ModuleId::M12,
                previous_health,
                current_health,
                timestamp: Timestamp::now(),
                reason,
            });
        }
    }

    /// Average failure rate across all registered breakers.
    #[allow(clippy::cast_precision_loss)]
    fn average_failure_rate(&self) -> f64 {
        let breakers = self.breakers.read();
        if breakers.is_empty() {
            return 0.0;
        }
        let total: f64 = breakers.values().map(CircuitBreakerEntry::failure_rate).sum();
        total / breakers.len() as f64
    }

    /// Fraction of breakers NOT in Open state (latency proxy).
    #[allow(clippy::cast_precision_loss)]
    fn closed_fraction(&self) -> f64 {
        let breakers = self.breakers.read();
        if breakers.is_empty() {
            return 1.0;
        }
        let non_open = breakers
            .values()
            .filter(|e| e.state != CircuitState::Open)
            .count();
        non_open as f64 / breakers.len() as f64
    }
}

impl CircuitBreakerOps for CircuitBreakerRegistry {
    fn register_breaker(&self, service_id: &str, config: CircuitBreakerConfig) -> Result<()> {
        let mut breakers = self.breakers.write();
        if breakers.contains_key(service_id) {
            return Err(Error::Validation(format!(
                "Circuit breaker already registered for '{service_id}'"
            )));
        }
        breakers.insert(service_id.to_owned(), CircuitBreakerEntry::new(config));
        drop(breakers);
        Ok(())
    }

    fn register_default(&self, service_id: &str) -> Result<()> {
        self.register_breaker(service_id, CircuitBreakerConfig::default())
    }

    fn deregister_breaker(&self, service_id: &str) -> Result<()> {
        self.breakers
            .write()
            .remove(service_id)
            .map(|_| ())
            .ok_or_else(|| Error::ServiceNotFound(service_id.to_owned()))
    }

    fn record_success(&self, service_id: &str) -> Result<CircuitState> {
        let (state, signal_data) = {
            let mut breakers = self.breakers.write();
            let entry = breakers
                .get_mut(service_id)
                .ok_or_else(|| Error::ServiceNotFound(service_id.to_owned()))?;

            entry.total_requests += 1;
            entry.last_success_time = Some(Timestamp::now());

            let old_state = entry.state;

            match entry.state {
                CircuitState::Closed => {
                    entry.failure_count = 0;
                }
                CircuitState::HalfOpen => {
                    // F-05: Release one probe slot — this success represents the
                    // outcome of a probe that `allow_request` admitted.
                    entry.in_flight_probes = entry.in_flight_probes.saturating_sub(1);
                    entry.success_count += 1;
                    entry.consecutive_successes += 1;
                    if entry.consecutive_successes >= entry.config.success_threshold {
                        entry.transition_to(
                            CircuitState::Closed,
                            format!(
                                "Consecutive successes ({}) reached threshold ({})",
                                entry.consecutive_successes, entry.config.success_threshold
                            ),
                        );
                        entry.failure_count = 0;
                        entry.success_count = 0;
                        entry.consecutive_successes = 0;
                    }
                }
                CircuitState::Open => {}
            }

            let signal = if old_state == CircuitState::HalfOpen
                && entry.state == CircuitState::Closed
            {
                Some((0.5, 1.0, format!("Circuit recovered for '{service_id}'")))
            } else {
                None
            };

            let result = (entry.state, signal);
            drop(breakers);
            result
        };

        if let Some((prev, curr, reason)) = signal_data {
            self.emit_signal(prev, curr, reason);
        }

        Ok(state)
    }

    fn record_failure(&self, service_id: &str) -> Result<CircuitState> {
        let (state, signal_data) = {
            let mut breakers = self.breakers.write();
            let entry = breakers
                .get_mut(service_id)
                .ok_or_else(|| Error::ServiceNotFound(service_id.to_owned()))?;

            entry.total_requests += 1;
            entry.total_failures += 1;
            entry.last_failure_time = Some(Timestamp::now());

            let old_state = entry.state;

            match entry.state {
                CircuitState::Closed => {
                    entry.failure_count += 1;
                    if entry.failure_count >= entry.config.failure_threshold {
                        entry.transition_to(
                            CircuitState::Open,
                            format!(
                                "Failure count ({}) reached threshold ({})",
                                entry.failure_count, entry.config.failure_threshold
                            ),
                        );
                    }
                }
                CircuitState::HalfOpen => {
                    // F-05: `transition_to` resets `in_flight_probes` to 0,
                    // so no explicit decrement is required here.
                    entry.transition_to(
                        CircuitState::Open,
                        "Failure during HalfOpen probe".to_owned(),
                    );
                    entry.success_count = 0;
                    entry.consecutive_successes = 0;
                }
                CircuitState::Open => {}
            }

            let signal = if old_state != CircuitState::Open
                && entry.state == CircuitState::Open
            {
                Some((1.0, 0.0, format!("Circuit breaker opened for '{service_id}'")))
            } else {
                None
            };

            let result = (entry.state, signal);
            drop(breakers);
            result
        };

        if let Some((prev, curr, reason)) = signal_data {
            self.emit_signal(prev, curr, reason);
        }

        Ok(state)
    }

    fn allow_request(&self, service_id: &str) -> Result<bool> {
        let mut breakers = self.breakers.write();
        let entry = breakers
            .get_mut(service_id)
            .ok_or_else(|| Error::ServiceNotFound(service_id.to_owned()))?;

        let allowed = match entry.state {
            CircuitState::Closed => true,
            CircuitState::HalfOpen => {
                // F-05: Admit at most `half_open_max_requests` concurrent probes.
                // Additional callers are rejected until an outstanding probe
                // completes via `record_success`/`record_failure`.
                if entry.in_flight_probes < entry.config.half_open_max_requests {
                    entry.in_flight_probes = entry.in_flight_probes.saturating_add(1);
                    true
                } else {
                    false
                }
            }
            CircuitState::Open => {
                if entry.is_open_timeout_elapsed() {
                    entry.transition_to(
                        CircuitState::HalfOpen,
                        "Open timeout elapsed, probing".to_owned(),
                    );
                    entry.success_count = 0;
                    entry.consecutive_successes = 0;
                    // F-05: Count this caller as the first probe since we
                    // transitioned to HalfOpen in response to their request.
                    if entry.in_flight_probes < entry.config.half_open_max_requests {
                        entry.in_flight_probes = entry.in_flight_probes.saturating_add(1);
                        true
                    } else {
                        // Edge case: half_open_max_requests == 0 (misconfig).
                        // Treat as no probing allowed.
                        false
                    }
                } else {
                    false
                }
            }
        };
        drop(breakers);
        Ok(allowed)
    }

    fn get_state(&self, service_id: &str) -> Result<CircuitState> {
        self.breakers
            .read()
            .get(service_id)
            .map(|e| e.state)
            .ok_or_else(|| Error::ServiceNotFound(service_id.to_owned()))
    }

    fn get_breaker_stats(&self, service_id: &str) -> Result<CircuitBreakerStats> {
        let breakers = self.breakers.read();
        let entry = breakers
            .get(service_id)
            .ok_or_else(|| Error::ServiceNotFound(service_id.to_owned()))?;

        let stats = CircuitBreakerStats {
            state: entry.state,
            failure_count: entry.failure_count,
            success_count: entry.success_count,
            total_requests: entry.total_requests,
            total_failures: entry.total_failures,
            failure_rate: entry.failure_rate(),
            last_failure: entry.last_failure_time,
            last_state_change: entry.last_state_change,
        };
        drop(breakers);
        Ok(stats)
    }

    fn reset(&self, service_id: &str) -> Result<()> {
        let mut breakers = self.breakers.write();
        let entry = breakers
            .get_mut(service_id)
            .ok_or_else(|| Error::ServiceNotFound(service_id.to_owned()))?;

        if entry.state != CircuitState::Closed {
            entry.transition_to(CircuitState::Closed, "Manual reset".to_owned());
        }
        entry.failure_count = 0;
        entry.success_count = 0;
        entry.consecutive_successes = 0;
        // F-05: Manual reset also clears any in-flight probe accounting even
        // when already Closed (where `transition_to` would not fire).
        entry.in_flight_probes = 0;
        drop(breakers);
        Ok(())
    }

    fn get_open_circuits(&self) -> Vec<String> {
        self.breakers
            .read()
            .iter()
            .filter(|(_, e)| e.state == CircuitState::Open)
            .map(|(id, _)| id.clone())
            .collect()
    }

    fn breaker_count(&self) -> usize {
        self.breakers.read().len()
    }

    fn is_registered(&self, service_id: &str) -> bool {
        self.breakers.read().contains_key(service_id)
    }
}

// ============================================================================
// ME-001 F-05 regression fix: RAII probe-slot variant of `allow_request`
// ============================================================================

impl CircuitBreakerRegistry {
    /// Probe-slot RAII variant of [`CircuitBreakerOps::allow_request`].
    ///
    /// Returns `Ok(Some(permit))` when a `HalfOpen` probe slot is admitted; the
    /// permit's [`ProbePermit::record_success`]/`record_failure` consume it
    /// and record the outcome, while its `Drop` releases the slot if the
    /// caller panics or returns early without calling either. Returns
    /// `Ok(None)` when denied.
    ///
    /// Use this over `allow_request` for any code path where the outcome-
    /// record might be skipped by unwinding. `allow_request` remains valid
    /// when the caller can guarantee outcome-record is always reached.
    ///
    /// # Errors
    /// Returns `Error::ServiceNotFound` if no breaker is registered for
    /// `service_id`.
    pub fn allow_probe<'a>(&'a self, service_id: &str) -> Result<Option<ProbePermit<'a>>> {
        let allowed = <Self as CircuitBreakerOps>::allow_request(self, service_id)?;
        if allowed {
            Ok(Some(ProbePermit {
                registry: self,
                service_id: service_id.to_owned(),
                consumed: false,
            }))
        } else {
            Ok(None)
        }
    }
}

// ============================================================================
// LoadBalanceAlgorithm
// ============================================================================

/// Load balancing algorithm for endpoint selection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoadBalanceAlgorithm {
    /// Sequential rotation among healthy endpoints.
    RoundRobin,
    /// Weight-proportional selection.
    WeightedRoundRobin,
    /// Select endpoint with fewest active connections.
    LeastConnections,
    /// Deterministic hash-based selection.
    Random,
}

impl fmt::Display for LoadBalanceAlgorithm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RoundRobin => f.write_str("round_robin"),
            Self::WeightedRoundRobin => f.write_str("weighted_round_robin"),
            Self::LeastConnections => f.write_str("least_connections"),
            Self::Random => f.write_str("random"),
        }
    }
}

// ============================================================================
// Endpoint
// ============================================================================

/// A single endpoint in a load balancer pool.
#[derive(Clone, Debug)]
pub struct Endpoint {
    /// Unique identifier.
    pub id: String,
    /// Hostname or IP address.
    pub host: String,
    /// TCP port number.
    pub port: u16,
    /// Weight for weighted algorithms (clamped 0.0–1.0).
    pub weight: f64,
    /// Active connection count.
    pub active_connections: u32,
    /// Whether this endpoint is healthy.
    pub healthy: bool,
    /// Total requests routed here.
    pub total_requests: u64,
    /// Total failed requests.
    pub total_errors: u64,
}

impl Endpoint {
    /// Create a new healthy endpoint with zero counters.
    #[must_use]
    pub fn new(id: impl Into<String>, host: impl Into<String>, port: u16, weight: f64) -> Self {
        Self {
            id: id.into(),
            host: host.into(),
            port,
            weight: weight.clamp(0.0, 1.0),
            active_connections: 0,
            healthy: true,
            total_requests: 0,
            total_errors: 0,
        }
    }

    /// Error rate (0.0 if no requests).
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn error_rate(&self) -> f64 {
        if self.total_requests == 0 {
            return 0.0;
        }
        self.total_errors as f64 / self.total_requests as f64
    }
}

impl fmt::Display for Endpoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}@{}:{}", self.id, self.host, self.port)
    }
}

// ============================================================================
// EndpointPool (internal)
// ============================================================================

/// Internal pool of endpoints for a service.
#[derive(Clone, Debug)]
struct EndpointPool {
    service_id: String,
    endpoints: Vec<Endpoint>,
    algorithm: LoadBalanceAlgorithm,
    current_index: usize,
    selection_counter: u64,
}

impl EndpointPool {
    fn new(service_id: impl Into<String>, algorithm: LoadBalanceAlgorithm) -> Self {
        Self {
            service_id: service_id.into(),
            endpoints: Vec::new(),
            algorithm,
            current_index: 0,
            selection_counter: 0,
        }
    }

    fn healthy_indices(&self) -> Vec<usize> {
        self.endpoints
            .iter()
            .enumerate()
            .filter(|(_, ep)| ep.healthy)
            .map(|(i, _)| i)
            .collect()
    }

    fn select_index(&mut self) -> Result<usize> {
        let healthy = self.healthy_indices();
        if healthy.is_empty() {
            return Err(Error::ServiceNotFound(format!(
                "No healthy endpoints for '{}'",
                self.service_id
            )));
        }

        match self.algorithm {
            LoadBalanceAlgorithm::RoundRobin => self.select_round_robin(&healthy),
            LoadBalanceAlgorithm::WeightedRoundRobin => self.select_weighted(&healthy),
            LoadBalanceAlgorithm::LeastConnections => Self::select_least_connections(&self.endpoints, &healthy),
            LoadBalanceAlgorithm::Random => self.select_random(&healthy),
        }
    }

    fn select_round_robin(&mut self, healthy: &[usize]) -> Result<usize> {
        let pos = self.current_index % healthy.len();
        self.current_index = self.current_index.wrapping_add(1);
        healthy.get(pos).copied().ok_or_else(|| {
            Error::ServiceNotFound(format!("Round-robin index OOB for '{}'", self.service_id))
        })
    }

    #[allow(clippy::cast_precision_loss)]
    fn select_weighted(&mut self, healthy: &[usize]) -> Result<usize> {
        let total_weight: f64 = healthy.iter().map(|&i| self.endpoints[i].weight).sum();
        if total_weight <= 0.0 {
            return self.select_round_robin(healthy);
        }

        self.selection_counter = self.selection_counter.wrapping_add(1);
        let position = (self.selection_counter as f64 % (total_weight * 1000.0)) / 1000.0;

        let mut cumulative = 0.0;
        for &idx in healthy {
            cumulative += self.endpoints[idx].weight;
            if position < cumulative {
                return Ok(idx);
            }
        }

        healthy.last().copied().ok_or_else(|| {
            Error::ServiceNotFound(format!("Weighted selection failed for '{}'", self.service_id))
        })
    }

    fn select_least_connections(endpoints: &[Endpoint], healthy: &[usize]) -> Result<usize> {
        healthy
            .iter()
            .copied()
            .min_by_key(|&i| endpoints[i].active_connections)
            .ok_or_else(|| Error::ServiceNotFound("Least-connections failed".to_owned()))
    }

    #[allow(clippy::cast_possible_truncation)]
    fn select_random(&mut self, healthy: &[usize]) -> Result<usize> {
        self.selection_counter = self.selection_counter.wrapping_add(1);
        let hash = self
            .selection_counter
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        let idx = (hash as usize) % healthy.len();
        healthy.get(idx).copied().ok_or_else(|| {
            Error::ServiceNotFound(format!("Random selection failed for '{}'", self.service_id))
        })
    }
}

// ============================================================================
// PoolStats
// ============================================================================

/// Aggregate statistics for an endpoint pool.
#[derive(Clone, Debug, Default)]
pub struct PoolStats {
    /// Total endpoints.
    pub total_endpoints: usize,
    /// Healthy endpoints.
    pub healthy_endpoints: usize,
    /// Total requests across all endpoints.
    pub total_requests: u64,
    /// Total errors across all endpoints.
    pub total_errors: u64,
    /// Overall error rate (0.0–1.0).
    pub error_rate: f64,
}

// ============================================================================
// LoadBalancer
// ============================================================================

/// Thread-safe load balancer with multiple endpoint pools.
pub struct LoadBalancer {
    pools: RwLock<HashMap<String, EndpointPool>>,
    _metrics: Option<Arc<MetricsRegistry>>,
}

impl fmt::Debug for LoadBalancer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LoadBalancer")
            .field("pool_count", &self.pools.read().len())
            .finish_non_exhaustive()
    }
}

impl Default for LoadBalancer {
    fn default() -> Self {
        Self::new()
    }
}

impl LoadBalancer {
    /// Create a new empty load balancer.
    #[must_use]
    pub fn new() -> Self {
        Self {
            pools: RwLock::new(HashMap::new()),
            _metrics: None,
        }
    }

    /// Create with a metrics registry.
    #[must_use]
    pub fn with_metrics(metrics: Arc<MetricsRegistry>) -> Self {
        Self {
            pools: RwLock::new(HashMap::new()),
            _metrics: Some(metrics),
        }
    }

    /// Number of pools.
    #[must_use]
    pub fn pool_count(&self) -> usize {
        self.pools.read().len()
    }
}

impl LoadBalancing for LoadBalancer {
    fn create_pool(&self, service_id: &str, algorithm: LoadBalanceAlgorithm) -> Result<()> {
        let mut pools = self.pools.write();
        if pools.contains_key(service_id) {
            return Err(Error::Validation(format!(
                "Pool already exists for '{service_id}'"
            )));
        }
        pools.insert(
            service_id.to_owned(),
            EndpointPool::new(service_id, algorithm),
        );
        drop(pools);
        Ok(())
    }

    fn remove_pool(&self, service_id: &str) -> Result<()> {
        self.pools
            .write()
            .remove(service_id)
            .map(|_| ())
            .ok_or_else(|| Error::ServiceNotFound(format!("Pool not found: '{service_id}'")))
    }

    fn add_endpoint(&self, service_id: &str, endpoint: Endpoint) -> Result<()> {
        let mut pools = self.pools.write();
        let pool = pools
            .get_mut(service_id)
            .ok_or_else(|| Error::ServiceNotFound(format!("Pool not found: '{service_id}'")))?;

        if pool.endpoints.iter().any(|ep| ep.id == endpoint.id) {
            return Err(Error::Validation(format!(
                "Endpoint '{}' already exists in pool '{service_id}'",
                endpoint.id
            )));
        }

        pool.endpoints.push(endpoint);
        drop(pools);
        Ok(())
    }

    fn remove_endpoint(&self, service_id: &str, endpoint_id: &str) -> Result<()> {
        let mut pools = self.pools.write();
        let pool = pools
            .get_mut(service_id)
            .ok_or_else(|| Error::ServiceNotFound(format!("Pool not found: '{service_id}'")))?;

        let pos = pool
            .endpoints
            .iter()
            .position(|ep| ep.id == endpoint_id)
            .ok_or_else(|| {
                Error::ServiceNotFound(format!(
                    "Endpoint '{endpoint_id}' not found in pool '{service_id}'"
                ))
            })?;

        pool.endpoints.remove(pos);
        if pool.current_index >= pool.endpoints.len() && !pool.endpoints.is_empty() {
            pool.current_index = 0;
        }
        drop(pools);
        Ok(())
    }

    fn select_endpoint(&self, service_id: &str) -> Result<Endpoint> {
        let mut pools = self.pools.write();
        let pool = pools
            .get_mut(service_id)
            .ok_or_else(|| Error::ServiceNotFound(format!("Pool not found: '{service_id}'")))?;

        let idx = pool.select_index()?;
        pool.endpoints[idx].total_requests += 1;
        pool.endpoints[idx].active_connections += 1;
        let endpoint = pool.endpoints[idx].clone();
        drop(pools);
        Ok(endpoint)
    }

    fn mark_healthy(&self, service_id: &str, endpoint_id: &str) -> Result<()> {
        let mut pools = self.pools.write();
        let ep = Self::find_endpoint_mut(&mut pools, service_id, endpoint_id)?;
        ep.healthy = true;
        drop(pools);
        Ok(())
    }

    fn mark_unhealthy(&self, service_id: &str, endpoint_id: &str) -> Result<()> {
        let mut pools = self.pools.write();
        let ep = Self::find_endpoint_mut(&mut pools, service_id, endpoint_id)?;
        ep.healthy = false;
        drop(pools);
        Ok(())
    }

    fn record_request(&self, service_id: &str, endpoint_id: &str, success: bool) -> Result<()> {
        let mut pools = self.pools.write();
        let ep = Self::find_endpoint_mut(&mut pools, service_id, endpoint_id)?;
        ep.active_connections = ep.active_connections.saturating_sub(1);
        if !success {
            ep.total_errors += 1;
        }
        drop(pools);
        Ok(())
    }

    #[allow(clippy::cast_precision_loss)]
    fn get_pool_stats(&self, service_id: &str) -> Result<PoolStats> {
        let pools = self.pools.read();
        let pool = pools
            .get(service_id)
            .ok_or_else(|| Error::ServiceNotFound(format!("Pool not found: '{service_id}'")))?;

        let total_endpoints = pool.endpoints.len();
        let healthy_endpoints = pool.endpoints.iter().filter(|ep| ep.healthy).count();
        let total_requests: u64 = pool.endpoints.iter().map(|ep| ep.total_requests).sum();
        let total_errors: u64 = pool.endpoints.iter().map(|ep| ep.total_errors).sum();
        let error_rate = if total_requests == 0 {
            0.0
        } else {
            total_errors as f64 / total_requests as f64
        };

        let stats = PoolStats {
            total_endpoints,
            healthy_endpoints,
            total_requests,
            total_errors,
            error_rate,
        };
        drop(pools);
        Ok(stats)
    }

    #[allow(clippy::cast_precision_loss)]
    fn get_load_distribution(&self, service_id: &str) -> Result<Vec<(String, f64)>> {
        let pools = self.pools.read();
        let pool = pools
            .get(service_id)
            .ok_or_else(|| Error::ServiceNotFound(format!("Pool not found: '{service_id}'")))?;

        let total_requests: u64 = pool.endpoints.iter().map(|ep| ep.total_requests).sum();

        let distribution: Vec<(String, f64)> = pool
            .endpoints
            .iter()
            .map(|ep| {
                let pct = if total_requests == 0 {
                    0.0
                } else {
                    ep.total_requests as f64 / total_requests as f64
                };
                (ep.id.clone(), pct)
            })
            .collect();
        drop(pools);
        Ok(distribution)
    }
}

impl LoadBalancer {
    /// Find a mutable endpoint within the pools map (already locked).
    fn find_endpoint_mut<'a>(
        pools: &'a mut HashMap<String, EndpointPool>,
        service_id: &str,
        endpoint_id: &str,
    ) -> Result<&'a mut Endpoint> {
        let pool = pools
            .get_mut(service_id)
            .ok_or_else(|| Error::ServiceNotFound(format!("Pool not found: '{service_id}'")))?;

        pool.endpoints
            .iter_mut()
            .find(|ep| ep.id == endpoint_id)
            .ok_or_else(|| {
                Error::ServiceNotFound(format!(
                    "Endpoint '{endpoint_id}' not found in pool '{service_id}'"
                ))
            })
    }
}

// ============================================================================
// ResilienceManager
// ============================================================================

/// Facade owning both circuit breaker and load balancer subsystems.
///
/// Implements [`TensorContributor`] for M12 12D tensor composition.
pub struct ResilienceManager {
    circuit_breakers: CircuitBreakerRegistry,
    load_balancer: LoadBalancer,
}

impl fmt::Debug for ResilienceManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ResilienceManager")
            .field("circuit_breakers", &self.circuit_breakers)
            .field("load_balancer", &self.load_balancer)
            .finish()
    }
}

impl Default for ResilienceManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ResilienceManager {
    /// Create a new resilience manager.
    #[must_use]
    pub fn new() -> Self {
        Self {
            circuit_breakers: CircuitBreakerRegistry::new(),
            load_balancer: LoadBalancer::new(),
        }
    }

    /// Create with signal bus.
    #[must_use]
    pub fn with_signal_bus(signal_bus: Arc<SignalBus>) -> Self {
        Self {
            circuit_breakers: CircuitBreakerRegistry::with_signal_bus(signal_bus),
            load_balancer: LoadBalancer::new(),
        }
    }

    /// Create with signal bus and metrics.
    #[must_use]
    pub fn with_signal_bus_and_metrics(
        signal_bus: Arc<SignalBus>,
        metrics: Arc<MetricsRegistry>,
    ) -> Self {
        Self {
            circuit_breakers: CircuitBreakerRegistry::with_signal_bus_and_metrics(
                signal_bus,
                Arc::clone(&metrics),
            ),
            load_balancer: LoadBalancer::with_metrics(metrics),
        }
    }

    /// Access the circuit breaker registry.
    #[must_use]
    pub const fn circuit_breakers(&self) -> &CircuitBreakerRegistry {
        &self.circuit_breakers
    }

    /// Access the load balancer.
    #[must_use]
    pub const fn load_balancer(&self) -> &LoadBalancer {
        &self.load_balancer
    }
}

// ============================================================================
// TensorContributor — CircuitBreakerRegistry: D9 (latency proxy), D10 (error rate)
// ============================================================================

impl TensorContributor for CircuitBreakerRegistry {
    fn contribute(&self) -> ContributedTensor {
        let d9 = self.closed_fraction();
        let d10 = self.average_failure_rate();

        let mut tensor = Tensor12D::default();
        let mut values = tensor.to_array();
        values[DimensionIndex::Latency as usize] = d9.clamp(0.0, 1.0);
        values[DimensionIndex::ErrorRate as usize] = d10.clamp(0.0, 1.0);
        tensor = Tensor12D::new(values);

        let coverage = CoverageBitmap::EMPTY
            .with_dimension(DimensionIndex::Latency)
            .with_dimension(DimensionIndex::ErrorRate);

        ContributedTensor {
            tensor,
            coverage,
            kind: ContributorKind::Stream,
        }
    }

    fn contributor_kind(&self) -> ContributorKind {
        ContributorKind::Stream
    }

    fn module_id(&self) -> &'static str {
        "M12-CB"
    }
}

// ============================================================================
// TensorContributor — ResilienceManager: D9 (latency proxy), D10 (error rate)
// ============================================================================

impl TensorContributor for ResilienceManager {
    fn contribute(&self) -> ContributedTensor {
        let d9 = self.circuit_breakers.closed_fraction();
        let d10 = self.circuit_breakers.average_failure_rate();

        let mut tensor = Tensor12D::default();
        let mut values = tensor.to_array();
        values[DimensionIndex::Latency as usize] = d9.clamp(0.0, 1.0);
        values[DimensionIndex::ErrorRate as usize] = d10.clamp(0.0, 1.0);
        tensor = Tensor12D::new(values);

        let coverage = CoverageBitmap::EMPTY
            .with_dimension(DimensionIndex::Latency)
            .with_dimension(DimensionIndex::ErrorRate);

        ContributedTensor {
            tensor,
            coverage,
            kind: ContributorKind::Stream,
        }
    }

    fn contributor_kind(&self) -> ContributorKind {
        ContributorKind::Stream
    }

    fn module_id(&self) -> &'static str {
        "M12"
    }
}

// ============================================================================
// Tests — Part 1 (will be extended)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ---- COMPILE: Send + Sync + Object Safety ----

    #[test]
    fn test_circuit_breaker_registry_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<CircuitBreakerRegistry>();
    }

    #[test]
    fn test_load_balancer_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<LoadBalancer>();
    }

    #[test]
    fn test_resilience_manager_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<ResilienceManager>();
    }

    // ---- BASIC: CircuitBreakerConfig ----

    #[test]
    fn test_default_config() {
        let config = CircuitBreakerConfig::default();
        assert_eq!(config.failure_threshold, 5);
        assert_eq!(config.success_threshold, 3);
        assert_eq!(config.open_timeout, Duration::from_secs(30));
        assert_eq!(config.half_open_max_requests, 1);
        assert_eq!(config.monitoring_window, Duration::from_secs(60));
    }

    #[test]
    fn test_config_builder() {
        let config = CircuitBreakerConfig::builder()
            .failure_threshold(10)
            .success_threshold(5)
            .open_timeout(Duration::from_secs(60))
            .half_open_max_requests(3)
            .monitoring_window(Duration::from_secs(120))
            .build();
        assert_eq!(config.failure_threshold, 10);
        assert_eq!(config.success_threshold, 5);
        assert_eq!(config.open_timeout, Duration::from_secs(60));
    }

    #[test]
    fn test_config_builder_default_trait() {
        let builder = CircuitBreakerConfigBuilder::default();
        let config = builder.build();
        assert_eq!(config.failure_threshold, 5);
    }

    // ---- BASIC: CircuitBreakerRegistry ----

    #[test]
    fn test_register_breaker() {
        let reg = CircuitBreakerRegistry::new();
        assert!(reg.register_breaker("svc-1", CircuitBreakerConfig::default()).is_ok());
        assert!(reg.is_registered("svc-1"));
        assert_eq!(reg.breaker_count(), 1);
    }

    #[test]
    fn test_register_default_breaker() {
        let reg = CircuitBreakerRegistry::new();
        assert!(reg.register_default("svc-1").is_ok());
        assert!(reg.is_registered("svc-1"));
    }

    #[test]
    fn test_deregister_breaker() {
        let reg = CircuitBreakerRegistry::new();
        reg.register_default("svc-1").ok();
        assert!(reg.deregister_breaker("svc-1").is_ok());
        assert!(!reg.is_registered("svc-1"));
    }

    #[test]
    fn test_registry_default_trait() {
        let reg = CircuitBreakerRegistry::default();
        assert_eq!(reg.breaker_count(), 0);
    }

    // ---- INVARIANT: Circuit Breaker FSM ----

    #[test]
    fn test_closed_to_open_at_failure_threshold() {
        let reg = CircuitBreakerRegistry::new();
        let config = CircuitBreakerConfig::builder().failure_threshold(3).build();
        reg.register_breaker("svc", config).ok();

        for _ in 0..3 {
            reg.record_failure("svc").ok();
        }
        assert_eq!(reg.get_state("svc").ok(), Some(CircuitState::Open));
    }

    #[test]
    fn test_open_to_halfopen_after_timeout() {
        let reg = CircuitBreakerRegistry::new();
        let config = CircuitBreakerConfig::builder()
            .failure_threshold(1)
            .open_timeout(Duration::ZERO)
            .build();
        reg.register_breaker("svc", config).ok();

        reg.record_failure("svc").ok();
        assert_eq!(reg.get_state("svc").ok(), Some(CircuitState::Open));

        reg.allow_request("svc").ok();
        assert_eq!(reg.get_state("svc").ok(), Some(CircuitState::HalfOpen));
    }

    #[test]
    fn test_halfopen_to_closed_on_success_threshold() {
        let reg = CircuitBreakerRegistry::new();
        let config = CircuitBreakerConfig::builder()
            .failure_threshold(1)
            .success_threshold(2)
            .open_timeout(Duration::ZERO)
            .build();
        reg.register_breaker("svc", config).ok();

        reg.record_failure("svc").ok();
        reg.allow_request("svc").ok();
        assert_eq!(reg.get_state("svc").ok(), Some(CircuitState::HalfOpen));

        reg.record_success("svc").ok();
        assert_eq!(reg.get_state("svc").ok(), Some(CircuitState::HalfOpen));
        reg.record_success("svc").ok();
        assert_eq!(reg.get_state("svc").ok(), Some(CircuitState::Closed));
    }

    #[test]
    fn test_halfopen_to_open_on_failure() {
        let reg = CircuitBreakerRegistry::new();
        let config = CircuitBreakerConfig::builder()
            .failure_threshold(1)
            .success_threshold(3)
            .open_timeout(Duration::ZERO)
            .build();
        reg.register_breaker("svc", config).ok();

        reg.record_failure("svc").ok();
        reg.allow_request("svc").ok();
        assert_eq!(reg.get_state("svc").ok(), Some(CircuitState::HalfOpen));

        reg.record_failure("svc").ok();
        assert_eq!(reg.get_state("svc").ok(), Some(CircuitState::Open));
    }

    #[test]
    fn test_closed_allows_requests() {
        let reg = CircuitBreakerRegistry::new();
        reg.register_default("svc").ok();
        assert_eq!(reg.allow_request("svc").ok(), Some(true));
    }

    #[test]
    fn test_open_blocks_requests() {
        let reg = CircuitBreakerRegistry::new();
        let config = CircuitBreakerConfig::builder()
            .failure_threshold(2)
            .open_timeout(Duration::from_secs(3600))
            .build();
        reg.register_breaker("svc", config).ok();
        reg.record_failure("svc").ok();
        reg.record_failure("svc").ok();
        assert_eq!(reg.allow_request("svc").ok(), Some(false));
    }

    #[test]
    fn test_success_resets_failure_count() {
        let reg = CircuitBreakerRegistry::new();
        let config = CircuitBreakerConfig::builder().failure_threshold(5).build();
        reg.register_breaker("svc", config).ok();

        reg.record_failure("svc").ok();
        reg.record_failure("svc").ok();
        reg.record_success("svc").ok();

        if let Ok(stats) = reg.get_breaker_stats("svc") {
            assert_eq!(stats.failure_count, 0);
            assert_eq!(stats.state, CircuitState::Closed);
        }
    }

    #[test]
    fn test_reset_circuit() {
        let reg = CircuitBreakerRegistry::new();
        let config = CircuitBreakerConfig::builder()
            .failure_threshold(1)
            .open_timeout(Duration::from_secs(3600))
            .build();
        reg.register_breaker("svc", config).ok();
        reg.record_failure("svc").ok();
        assert_eq!(reg.get_state("svc").ok(), Some(CircuitState::Open));

        reg.reset("svc").ok();
        assert_eq!(reg.get_state("svc").ok(), Some(CircuitState::Closed));
        assert_eq!(reg.allow_request("svc").ok(), Some(true));
    }

    #[test]
    fn test_reset_already_closed() {
        let reg = CircuitBreakerRegistry::new();
        reg.register_default("svc").ok();
        assert!(reg.reset("svc").is_ok());
        assert_eq!(reg.get_state("svc").ok(), Some(CircuitState::Closed));
    }

    #[test]
    fn test_get_open_circuits() {
        let reg = CircuitBreakerRegistry::new();
        let config = CircuitBreakerConfig::builder()
            .failure_threshold(1)
            .open_timeout(Duration::from_secs(3600))
            .build();
        reg.register_breaker("a", config.clone()).ok();
        reg.register_breaker("b", config).ok();
        reg.register_default("c").ok();

        reg.record_failure("a").ok();
        reg.record_failure("b").ok();

        let mut open = reg.get_open_circuits();
        open.sort();
        assert_eq!(open.len(), 2);
        assert!(open.contains(&"a".to_owned()));
        assert!(open.contains(&"b".to_owned()));
    }

    // ---- BASIC: Stats ----

    #[test]
    fn test_get_breaker_stats() {
        let reg = CircuitBreakerRegistry::new();
        reg.register_default("svc").ok();
        reg.record_success("svc").ok();
        reg.record_success("svc").ok();
        reg.record_failure("svc").ok();

        if let Ok(stats) = reg.get_breaker_stats("svc") {
            assert_eq!(stats.total_requests, 3);
            assert_eq!(stats.total_failures, 1);
            assert!(stats.last_failure.is_some());
        }
    }

    #[test]
    fn test_failure_rate_calculation() {
        let reg = CircuitBreakerRegistry::new();
        reg.register_default("svc").ok();
        reg.record_success("svc").ok();
        reg.record_success("svc").ok();
        reg.record_success("svc").ok();
        reg.record_failure("svc").ok();

        if let Ok(stats) = reg.get_breaker_stats("svc") {
            assert!((stats.failure_rate - 0.25).abs() < f64::EPSILON);
        }
    }

    #[test]
    fn test_failure_rate_zero_requests() {
        let reg = CircuitBreakerRegistry::new();
        reg.register_default("svc").ok();
        if let Ok(stats) = reg.get_breaker_stats("svc") {
            assert!((stats.failure_rate).abs() < f64::EPSILON);
        }
    }

    #[test]
    fn test_state_history_recorded() {
        let reg = CircuitBreakerRegistry::new();
        let config = CircuitBreakerConfig::builder()
            .failure_threshold(1)
            .success_threshold(1)
            .open_timeout(Duration::ZERO)
            .build();
        reg.register_breaker("svc", config).ok();

        reg.record_failure("svc").ok();
        reg.allow_request("svc").ok();
        reg.record_success("svc").ok();

        let breakers = reg.breakers.read();
        if let Some(entry) = breakers.get("svc") {
            assert_eq!(entry.state_history.len(), 3);
            assert_eq!(entry.state_history[0].from, CircuitState::Closed);
            assert_eq!(entry.state_history[0].to, CircuitState::Open);
            assert_eq!(entry.state_history[1].from, CircuitState::Open);
            assert_eq!(entry.state_history[1].to, CircuitState::HalfOpen);
            assert_eq!(entry.state_history[2].from, CircuitState::HalfOpen);
            assert_eq!(entry.state_history[2].to, CircuitState::Closed);
        }
    }

    // ---- NEGATIVE: Circuit Breaker ----

    #[test]
    fn test_register_duplicate_fails() {
        let reg = CircuitBreakerRegistry::new();
        reg.register_default("svc").ok();
        assert!(reg.register_default("svc").is_err());
    }

    #[test]
    fn test_deregister_nonexistent_fails() {
        let reg = CircuitBreakerRegistry::new();
        assert!(reg.deregister_breaker("nope").is_err());
    }

    #[test]
    fn test_get_state_not_found() {
        let reg = CircuitBreakerRegistry::new();
        assert!(reg.get_state("nope").is_err());
    }

    #[test]
    fn test_record_success_not_found() {
        let reg = CircuitBreakerRegistry::new();
        assert!(reg.record_success("nope").is_err());
    }

    #[test]
    fn test_record_failure_not_found() {
        let reg = CircuitBreakerRegistry::new();
        assert!(reg.record_failure("nope").is_err());
    }

    #[test]
    fn test_allow_request_not_found() {
        let reg = CircuitBreakerRegistry::new();
        assert!(reg.allow_request("nope").is_err());
    }

    #[test]
    fn test_get_breaker_stats_not_found() {
        let reg = CircuitBreakerRegistry::new();
        assert!(reg.get_breaker_stats("nope").is_err());
    }

    #[test]
    fn test_reset_not_found() {
        let reg = CircuitBreakerRegistry::new();
        assert!(reg.reset("nope").is_err());
    }

    // ---- BASIC: LoadBalancer ----

    #[test]
    fn test_create_pool() {
        let lb = LoadBalancer::new();
        assert!(lb.create_pool("svc", LoadBalanceAlgorithm::RoundRobin).is_ok());
        assert!(lb.get_pool_stats("svc").is_ok());
    }

    #[test]
    fn test_remove_pool() {
        let lb = LoadBalancer::new();
        lb.create_pool("svc", LoadBalanceAlgorithm::RoundRobin).ok();
        assert!(lb.remove_pool("svc").is_ok());
        assert!(lb.get_pool_stats("svc").is_err());
    }

    #[test]
    fn test_add_and_remove_endpoint() {
        let lb = LoadBalancer::new();
        lb.create_pool("svc", LoadBalanceAlgorithm::RoundRobin).ok();
        assert!(lb.add_endpoint("svc", Endpoint::new("ep1", "10.0.0.1", 8080, 1.0)).is_ok());

        if let Ok(stats) = lb.get_pool_stats("svc") {
            assert_eq!(stats.total_endpoints, 1);
        }

        assert!(lb.remove_endpoint("svc", "ep1").is_ok());
        if let Ok(stats) = lb.get_pool_stats("svc") {
            assert_eq!(stats.total_endpoints, 0);
        }
    }

    #[test]
    fn test_load_balancer_default() {
        let lb = LoadBalancer::default();
        assert_eq!(lb.pool_count(), 0);
    }

    // ---- INVARIANT: Load Balancing Algorithms ----

    #[test]
    fn test_round_robin_cycles() {
        let lb = LoadBalancer::new();
        lb.create_pool("svc", LoadBalanceAlgorithm::RoundRobin).ok();
        lb.add_endpoint("svc", Endpoint::new("ep1", "10.0.0.1", 8080, 1.0)).ok();
        lb.add_endpoint("svc", Endpoint::new("ep2", "10.0.0.2", 8080, 1.0)).ok();
        lb.add_endpoint("svc", Endpoint::new("ep3", "10.0.0.3", 8080, 1.0)).ok();

        let ids: Vec<String> = (0..6)
            .filter_map(|_| lb.select_endpoint("svc").ok().map(|ep| ep.id))
            .collect();

        assert_eq!(ids.len(), 6);
        assert_eq!(ids[0], ids[3]);
        assert_eq!(ids[1], ids[4]);
        assert_eq!(ids[2], ids[5]);
        assert!(ids.contains(&"ep1".to_owned()));
        assert!(ids.contains(&"ep2".to_owned()));
        assert!(ids.contains(&"ep3".to_owned()));
    }

    #[test]
    fn test_weighted_distribution() {
        let lb = LoadBalancer::new();
        lb.create_pool("svc", LoadBalanceAlgorithm::WeightedRoundRobin).ok();
        lb.add_endpoint("svc", Endpoint::new("heavy", "10.0.0.1", 8080, 0.8)).ok();
        lb.add_endpoint("svc", Endpoint::new("light", "10.0.0.2", 8080, 0.2)).ok();

        let mut heavy_count = 0u64;
        let mut light_count = 0u64;
        for _ in 0..1000 {
            if let Ok(ep) = lb.select_endpoint("svc") {
                if ep.id == "heavy" {
                    heavy_count += 1;
                } else {
                    light_count += 1;
                }
            }
        }
        assert!(heavy_count > light_count);
    }

    #[test]
    fn test_least_connections_selection() {
        let lb = LoadBalancer::new();
        lb.create_pool("svc", LoadBalanceAlgorithm::LeastConnections).ok();
        lb.add_endpoint("svc", Endpoint::new("busy", "10.0.0.1", 8080, 1.0)).ok();
        lb.add_endpoint("svc", Endpoint::new("idle", "10.0.0.2", 8080, 1.0)).ok();

        // Set busy endpoint to many connections
        {
            let mut pools = lb.pools.write();
            if let Some(p) = pools.get_mut("svc") {
                p.endpoints[0].active_connections = 10;
                p.endpoints[1].active_connections = 1;
            }
        }

        if let Ok(ep) = lb.select_endpoint("svc") {
            assert_eq!(ep.id, "idle");
        }
    }

    #[test]
    fn test_random_selection_covers_endpoints() {
        let lb = LoadBalancer::new();
        lb.create_pool("svc", LoadBalanceAlgorithm::Random).ok();
        lb.add_endpoint("svc", Endpoint::new("ep1", "10.0.0.1", 8080, 1.0)).ok();
        lb.add_endpoint("svc", Endpoint::new("ep2", "10.0.0.2", 8080, 1.0)).ok();
        lb.add_endpoint("svc", Endpoint::new("ep3", "10.0.0.3", 8080, 1.0)).ok();

        let mut seen = std::collections::HashSet::new();
        for _ in 0..100 {
            if let Ok(ep) = lb.select_endpoint("svc") {
                seen.insert(ep.id);
            }
        }
        assert!(seen.len() >= 2);
    }

    // ---- INVARIANT: Health-aware selection ----

    #[test]
    fn test_unhealthy_endpoint_excluded() {
        let lb = LoadBalancer::new();
        lb.create_pool("svc", LoadBalanceAlgorithm::RoundRobin).ok();
        lb.add_endpoint("svc", Endpoint::new("ep1", "10.0.0.1", 8080, 1.0)).ok();
        lb.add_endpoint("svc", Endpoint::new("ep2", "10.0.0.2", 8080, 1.0)).ok();
        lb.mark_unhealthy("svc", "ep1").ok();

        for _ in 0..10 {
            if let Ok(ep) = lb.select_endpoint("svc") {
                assert_eq!(ep.id, "ep2");
            }
        }
    }

    #[test]
    fn test_mark_healthy_restores_selection() {
        let lb = LoadBalancer::new();
        lb.create_pool("svc", LoadBalanceAlgorithm::RoundRobin).ok();
        lb.add_endpoint("svc", Endpoint::new("ep1", "10.0.0.1", 8080, 1.0)).ok();

        lb.mark_unhealthy("svc", "ep1").ok();
        assert!(lb.select_endpoint("svc").is_err());

        lb.mark_healthy("svc", "ep1").ok();
        assert!(lb.select_endpoint("svc").is_ok());
    }

    // ---- BOUNDARY: Load Balancer ----

    #[test]
    fn test_empty_pool_returns_error() {
        let lb = LoadBalancer::new();
        lb.create_pool("svc", LoadBalanceAlgorithm::RoundRobin).ok();
        assert!(lb.select_endpoint("svc").is_err());
    }

    #[test]
    fn test_no_healthy_endpoints_returns_error() {
        let lb = LoadBalancer::new();
        lb.create_pool("svc", LoadBalanceAlgorithm::RoundRobin).ok();
        lb.add_endpoint("svc", Endpoint::new("ep1", "10.0.0.1", 8080, 1.0)).ok();
        lb.mark_unhealthy("svc", "ep1").ok();
        assert!(lb.select_endpoint("svc").is_err());
    }

    #[test]
    fn test_endpoint_weight_clamping() {
        let ep_high = Endpoint::new("h", "10.0.0.1", 8080, 1.5);
        assert!((ep_high.weight - 1.0).abs() < f64::EPSILON);
        let ep_low = Endpoint::new("l", "10.0.0.2", 8080, -0.5);
        assert!((ep_low.weight).abs() < f64::EPSILON);
    }

    #[test]
    fn test_load_distribution_no_requests() {
        let lb = LoadBalancer::new();
        lb.create_pool("svc", LoadBalanceAlgorithm::RoundRobin).ok();
        lb.add_endpoint("svc", Endpoint::new("ep1", "10.0.0.1", 8080, 1.0)).ok();

        if let Ok(dist) = lb.get_load_distribution("svc") {
            assert_eq!(dist.len(), 1);
            assert!((dist[0].1).abs() < f64::EPSILON);
        }
    }

    #[test]
    fn test_load_distribution_balanced() {
        let lb = LoadBalancer::new();
        lb.create_pool("svc", LoadBalanceAlgorithm::RoundRobin).ok();
        lb.add_endpoint("svc", Endpoint::new("ep1", "10.0.0.1", 8080, 1.0)).ok();
        lb.add_endpoint("svc", Endpoint::new("ep2", "10.0.0.2", 8080, 1.0)).ok();

        for _ in 0..10 {
            lb.select_endpoint("svc").ok();
        }

        if let Ok(dist) = lb.get_load_distribution("svc") {
            assert_eq!(dist.len(), 2);
            for (_, pct) in &dist {
                assert!((*pct - 0.5).abs() < f64::EPSILON);
            }
        }
    }

    // ---- BASIC: Request recording ----

    #[test]
    fn test_record_request_success() {
        let lb = LoadBalancer::new();
        lb.create_pool("svc", LoadBalanceAlgorithm::RoundRobin).ok();
        lb.add_endpoint("svc", Endpoint::new("ep1", "10.0.0.1", 8080, 1.0)).ok();
        lb.select_endpoint("svc").ok();
        assert!(lb.record_request("svc", "ep1", true).is_ok());
    }

    #[test]
    fn test_record_request_failure_increments_errors() {
        let lb = LoadBalancer::new();
        lb.create_pool("svc", LoadBalanceAlgorithm::RoundRobin).ok();
        lb.add_endpoint("svc", Endpoint::new("ep1", "10.0.0.1", 8080, 1.0)).ok();
        lb.select_endpoint("svc").ok();
        lb.record_request("svc", "ep1", false).ok();

        if let Ok(stats) = lb.get_pool_stats("svc") {
            assert_eq!(stats.total_errors, 1);
        }
    }

    #[test]
    fn test_endpoint_error_rate() {
        let mut ep = Endpoint::new("ep1", "10.0.0.1", 8080, 1.0);
        assert!((ep.error_rate()).abs() < f64::EPSILON);
        ep.total_requests = 100;
        ep.total_errors = 25;
        assert!((ep.error_rate() - 0.25).abs() < f64::EPSILON);
    }

    // ---- NEGATIVE: Load Balancer ----

    #[test]
    fn test_create_duplicate_pool_fails() {
        let lb = LoadBalancer::new();
        lb.create_pool("svc", LoadBalanceAlgorithm::RoundRobin).ok();
        assert!(lb.create_pool("svc", LoadBalanceAlgorithm::LeastConnections).is_err());
    }

    #[test]
    fn test_remove_nonexistent_pool_fails() {
        let lb = LoadBalancer::new();
        assert!(lb.remove_pool("nope").is_err());
    }

    #[test]
    fn test_add_endpoint_nonexistent_pool_fails() {
        let lb = LoadBalancer::new();
        assert!(lb.add_endpoint("nope", Endpoint::new("ep1", "10.0.0.1", 8080, 1.0)).is_err());
    }

    #[test]
    fn test_add_duplicate_endpoint_fails() {
        let lb = LoadBalancer::new();
        lb.create_pool("svc", LoadBalanceAlgorithm::RoundRobin).ok();
        lb.add_endpoint("svc", Endpoint::new("ep1", "10.0.0.1", 8080, 1.0)).ok();
        assert!(lb.add_endpoint("svc", Endpoint::new("ep1", "10.0.0.2", 9090, 0.5)).is_err());
    }

    #[test]
    fn test_select_nonexistent_pool_fails() {
        let lb = LoadBalancer::new();
        assert!(lb.select_endpoint("nope").is_err());
    }

    #[test]
    fn test_mark_health_nonexistent_fails() {
        let lb = LoadBalancer::new();
        assert!(lb.mark_healthy("nope", "ep1").is_err());
        assert!(lb.mark_unhealthy("nope", "ep1").is_err());
    }

    #[test]
    fn test_record_request_nonexistent_endpoint_fails() {
        let lb = LoadBalancer::new();
        lb.create_pool("svc", LoadBalanceAlgorithm::RoundRobin).ok();
        assert!(lb.record_request("svc", "missing", true).is_err());
    }

    // ---- BASIC: ResilienceManager ----

    #[test]
    fn test_resilience_manager_new() {
        let rm = ResilienceManager::new();
        assert_eq!(rm.circuit_breakers().breaker_count(), 0);
        assert_eq!(rm.load_balancer().pool_count(), 0);
    }

    #[test]
    fn test_resilience_manager_default() {
        let rm = ResilienceManager::default();
        assert_eq!(rm.circuit_breakers().breaker_count(), 0);
    }

    #[test]
    fn test_resilience_manager_circuit_breaker_access() {
        let rm = ResilienceManager::new();
        rm.circuit_breakers().register_default("svc").ok();
        assert!(rm.circuit_breakers().is_registered("svc"));
    }

    #[test]
    fn test_resilience_manager_load_balancer_access() {
        let rm = ResilienceManager::new();
        rm.load_balancer().create_pool("svc", LoadBalanceAlgorithm::RoundRobin).ok();
        assert_eq!(rm.load_balancer().pool_count(), 1);
    }

    // ---- PROPERTY: Tensor Contribution ----

    #[test]
    fn test_tensor_contributor_empty() {
        let rm = ResilienceManager::new();
        let tensor = rm.contribute();
        let vals = tensor.tensor.to_array();
        // No breakers: D9=1.0 (all closed), D10=0.0 (no failures)
        assert!((vals[DimensionIndex::Latency as usize] - 1.0).abs() < f64::EPSILON);
        assert!((vals[DimensionIndex::ErrorRate as usize]).abs() < f64::EPSILON);
    }

    #[test]
    fn test_tensor_contributor_with_open_circuit() {
        let rm = ResilienceManager::new();
        let config = CircuitBreakerConfig::builder()
            .failure_threshold(1)
            .open_timeout(Duration::from_secs(3600))
            .build();
        rm.circuit_breakers().register_breaker("a", config.clone()).ok();
        rm.circuit_breakers().register_breaker("b", config).ok();
        rm.circuit_breakers().record_failure("a").ok();

        let tensor = rm.contribute();
        let vals = tensor.tensor.to_array();
        // 1 of 2 open: D9 = 0.5
        assert!((vals[DimensionIndex::Latency as usize] - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_tensor_contributor_coverage() {
        let rm = ResilienceManager::new();
        let tensor = rm.contribute();
        assert!(tensor.coverage.is_covered(DimensionIndex::Latency));
        assert!(tensor.coverage.is_covered(DimensionIndex::ErrorRate));
        assert!(!tensor.coverage.is_covered(DimensionIndex::ServiceId));
    }

    #[test]
    fn test_tensor_contributor_module_id() {
        let rm = ResilienceManager::new();
        assert_eq!(rm.module_id(), "M12");
        assert_eq!(rm.contributor_kind(), ContributorKind::Stream);
    }

    #[test]
    fn test_tensor_dims_in_unit_interval() {
        let rm = ResilienceManager::new();
        rm.circuit_breakers().register_default("svc").ok();
        for _ in 0..10 {
            rm.circuit_breakers().record_failure("svc").ok();
        }
        let tensor = rm.contribute();
        let vals = tensor.tensor.to_array();
        for val in &vals {
            assert!(*val >= 0.0 && *val <= 1.0);
        }
    }

    // ---- INTEGRATION: Signal Emission ----

    #[test]
    fn test_signal_bus_none_does_not_panic() {
        let reg = CircuitBreakerRegistry::new();
        let config = CircuitBreakerConfig::builder().failure_threshold(1).build();
        reg.register_breaker("svc", config).ok();
        // Should not panic even without signal bus
        assert!(reg.record_failure("svc").is_ok());
    }

    #[test]
    fn test_circuit_trip_emits_signal() {
        let bus = Arc::new(SignalBus::new());
        let reg = CircuitBreakerRegistry::with_signal_bus(Arc::clone(&bus));
        let config = CircuitBreakerConfig::builder().failure_threshold(1).build();
        reg.register_breaker("svc", config).ok();

        let before = bus.stats().health_emitted;
        reg.record_failure("svc").ok();
        let after = bus.stats().health_emitted;
        assert!(after > before, "Signal should be emitted on circuit trip");
    }

    #[test]
    fn test_circuit_recovery_emits_signal() {
        let bus = Arc::new(SignalBus::new());
        let reg = CircuitBreakerRegistry::with_signal_bus(Arc::clone(&bus));
        let config = CircuitBreakerConfig::builder()
            .failure_threshold(1)
            .success_threshold(1)
            .open_timeout(Duration::ZERO)
            .build();
        reg.register_breaker("svc", config).ok();

        reg.record_failure("svc").ok();
        reg.allow_request("svc").ok();

        let before = bus.stats().health_emitted;
        reg.record_success("svc").ok();
        let after = bus.stats().health_emitted;
        assert!(after > before, "Signal should be emitted on recovery");
    }

    // ---- INTEGRATION: Cross-subsystem ----

    #[test]
    fn test_resilience_manager_with_signal_bus() {
        let bus = Arc::new(SignalBus::new());
        let rm = ResilienceManager::with_signal_bus(bus);
        rm.circuit_breakers().register_default("svc").ok();
        rm.load_balancer().create_pool("svc", LoadBalanceAlgorithm::RoundRobin).ok();
        assert!(rm.circuit_breakers().is_registered("svc"));
        assert_eq!(rm.load_balancer().pool_count(), 1);
    }

    // ---- PROPERTY: Display impls ----

    #[test]
    fn test_algorithm_display() {
        assert_eq!(format!("{}", LoadBalanceAlgorithm::RoundRobin), "round_robin");
        assert_eq!(format!("{}", LoadBalanceAlgorithm::LeastConnections), "least_connections");
    }

    #[test]
    fn test_endpoint_display() {
        let ep = Endpoint::new("ep1", "10.0.0.1", 8080, 1.0);
        assert_eq!(format!("{ep}"), "ep1@10.0.0.1:8080");
    }

    #[test]
    fn test_transition_display() {
        let t = CircuitStateTransition {
            from: CircuitState::Closed,
            to: CircuitState::Open,
            reason: "threshold".to_owned(),
            timestamp: Timestamp::now(),
        };
        let s = format!("{t}");
        assert!(s.contains("closed"));
        assert!(s.contains("open"));
    }

    // ---- PROPERTY: Debug impls ----

    #[test]
    fn test_registry_debug() {
        let reg = CircuitBreakerRegistry::new();
        let dbg = format!("{reg:?}");
        assert!(dbg.contains("CircuitBreakerRegistry"));
    }

    #[test]
    fn test_load_balancer_debug() {
        let lb = LoadBalancer::new();
        let dbg = format!("{lb:?}");
        assert!(dbg.contains("LoadBalancer"));
    }

    #[test]
    fn test_resilience_manager_debug() {
        let rm = ResilienceManager::new();
        let dbg = format!("{rm:?}");
        assert!(dbg.contains("ResilienceManager"));
    }

    // ---- BOUNDARY: Circuit Breaker ----

    #[test]
    fn test_single_failure_below_threshold_stays_closed() {
        let reg = CircuitBreakerRegistry::new();
        let config = CircuitBreakerConfig::builder().failure_threshold(3).build();
        reg.register_breaker("svc", config).ok();
        reg.record_failure("svc").ok();
        assert_eq!(reg.get_state("svc").ok(), Some(CircuitState::Closed));
    }

    #[test]
    fn test_exact_threshold_opens() {
        let reg = CircuitBreakerRegistry::new();
        let config = CircuitBreakerConfig::builder().failure_threshold(1).build();
        reg.register_breaker("svc", config).ok();
        reg.record_failure("svc").ok();
        assert_eq!(reg.get_state("svc").ok(), Some(CircuitState::Open));
    }

    #[test]
    fn test_multiple_resets_idempotent() {
        let reg = CircuitBreakerRegistry::new();
        reg.register_default("svc").ok();
        reg.reset("svc").ok();
        reg.reset("svc").ok();
        assert_eq!(reg.get_state("svc").ok(), Some(CircuitState::Closed));
    }

    // ---- INTEGRATION: L1 boundary ----

    #[test]
    fn test_l2_uses_timestamp_not_chrono() {
        let reg = CircuitBreakerRegistry::new();
        reg.register_default("svc").ok();
        reg.record_failure("svc").ok();
        if let Ok(stats) = reg.get_breaker_stats("svc") {
            // Timestamp is our type, not chrono
            let _ts: Timestamp = stats.last_state_change;
            assert!(stats.last_failure.is_some());
        }
    }

    #[test]
    fn test_l2_uses_duration_not_ms() {
        let config = CircuitBreakerConfig::builder()
            .open_timeout(Duration::from_millis(500))
            .build();
        assert_eq!(config.open_timeout, Duration::from_millis(500));
    }

    #[test]
    fn test_l2_uses_l1_error_type() {
        let reg = CircuitBreakerRegistry::new();
        let result = reg.get_state("nope");
        assert!(result.is_err());
        // Verify it's our Error type
        if let Err(e) = result {
            let _msg = format!("{e}");
        }
    }

    // ========================================================================
    // F-05: `half_open_max_requests` concurrency cap enforcement
    //
    // Regression guard for Session 099 bug-hunt finding F-05:
    // Prior to the fix, `allow_request` unconditionally admitted every caller
    // while the breaker was in `HalfOpen`, ignoring `half_open_max_requests`.
    // Under a still-flaky downstream this would burst all concurrent callers
    // at the first probe window and almost certainly re-open the circuit.
    // ========================================================================

    /// Drive a breaker into `HalfOpen` by opening it then advancing past the
    /// open-timeout. Uses `Duration::ZERO` so the next `allow_request` will
    /// probe immediately.
    fn make_halfopen(max_probes: u32, success_threshold: u32) -> CircuitBreakerRegistry {
        let reg = CircuitBreakerRegistry::new();
        let config = CircuitBreakerConfig::builder()
            .failure_threshold(1)
            .success_threshold(success_threshold)
            .open_timeout(Duration::ZERO)
            .half_open_max_requests(max_probes)
            .build();
        reg.register_breaker("svc", config).ok();
        // Drive Closed -> Open via a single failure.
        reg.record_failure("svc").ok();
        assert_eq!(reg.get_state("svc").ok(), Some(CircuitState::Open));
        reg
    }

    #[test]
    fn f05_halfopen_admits_exactly_one_probe_by_default() {
        // Default config: `half_open_max_requests = 1`.
        // First `allow_request` transitions Open -> HalfOpen AND consumes the
        // single probe slot. Second call must be rejected.
        let reg = make_halfopen(1, 3);

        assert_eq!(reg.allow_request("svc").ok(), Some(true), "first probe admitted");
        assert_eq!(reg.get_state("svc").ok(), Some(CircuitState::HalfOpen));
        assert_eq!(
            reg.allow_request("svc").ok(),
            Some(false),
            "second probe MUST be rejected while first in-flight"
        );
    }

    #[test]
    fn f05_halfopen_admits_up_to_n_probes() {
        // `half_open_max_requests = 3`: first three admitted, fourth rejected.
        let reg = make_halfopen(3, 10);

        assert_eq!(reg.allow_request("svc").ok(), Some(true));
        assert_eq!(reg.get_state("svc").ok(), Some(CircuitState::HalfOpen));
        assert_eq!(reg.allow_request("svc").ok(), Some(true));
        assert_eq!(reg.allow_request("svc").ok(), Some(true));
        assert_eq!(
            reg.allow_request("svc").ok(),
            Some(false),
            "4th probe MUST be rejected — cap is 3"
        );
    }

    #[test]
    fn f05_probe_slot_released_on_success() {
        // After a successful probe, the slot should be released and a
        // subsequent caller admitted. Uses success_threshold=10 so a single
        // success does NOT flip the circuit Closed.
        let reg = make_halfopen(1, 10);

        assert_eq!(reg.allow_request("svc").ok(), Some(true));
        assert_eq!(reg.allow_request("svc").ok(), Some(false), "slot occupied");

        reg.record_success("svc").ok();
        assert_eq!(
            reg.get_state("svc").ok(),
            Some(CircuitState::HalfOpen),
            "threshold not reached; still probing"
        );
        assert_eq!(
            reg.allow_request("svc").ok(),
            Some(true),
            "slot released by record_success"
        );
    }

    #[test]
    fn f05_probe_accounting_cleared_on_failure_transition_to_open() {
        // `record_failure` in HalfOpen transitions to Open. The in-flight
        // counter must be cleared by `transition_to` so that the next
        // open-timeout elapse admits a fresh probe.
        let reg = make_halfopen(2, 5);

        assert_eq!(reg.allow_request("svc").ok(), Some(true));
        assert_eq!(reg.allow_request("svc").ok(), Some(true));
        // Probe fails: HalfOpen -> Open, in_flight reset to 0.
        reg.record_failure("svc").ok();
        assert_eq!(reg.get_state("svc").ok(), Some(CircuitState::Open));

        // Open timeout is Duration::ZERO so the next allow_request probes again.
        assert_eq!(
            reg.allow_request("svc").ok(),
            Some(true),
            "fresh probe after re-open"
        );
        assert_eq!(reg.get_state("svc").ok(), Some(CircuitState::HalfOpen));
        // And the cap is restored in the new HalfOpen episode.
        assert_eq!(reg.allow_request("svc").ok(), Some(true));
        assert_eq!(
            reg.allow_request("svc").ok(),
            Some(false),
            "cap=2 honoured in new probe window"
        );
    }

    #[test]
    fn f05_reset_clears_in_flight_probes() {
        // Manual reset from HalfOpen must also zero the probe counter so a
        // freshly-closed breaker has a clean slate.
        let reg = make_halfopen(1, 10);
        reg.allow_request("svc").ok();
        assert_eq!(reg.allow_request("svc").ok(), Some(false));

        reg.reset("svc").ok();
        assert_eq!(reg.get_state("svc").ok(), Some(CircuitState::Closed));

        // After reset, a future HalfOpen episode should admit probes normally.
        reg.record_failure("svc").ok();
        assert_eq!(reg.get_state("svc").ok(), Some(CircuitState::Open));
        assert_eq!(reg.allow_request("svc").ok(), Some(true));
        assert_eq!(
            reg.allow_request("svc").ok(),
            Some(false),
            "cap=1 enforced cleanly post-reset"
        );
    }

    #[test]
    fn f05_zero_max_probes_rejects_all_probes() {
        // Pathological config: `half_open_max_requests = 0`.
        // Prior code admitted every caller; the fix must reject all — the
        // breaker effectively stays closed-to-traffic until operator reset.
        let reg = CircuitBreakerRegistry::new();
        let config = CircuitBreakerConfig::builder()
            .failure_threshold(1)
            .success_threshold(1)
            .open_timeout(Duration::ZERO)
            .half_open_max_requests(0)
            .build();
        reg.register_breaker("svc", config).ok();
        reg.record_failure("svc").ok();

        // First allow_request transitions Open -> HalfOpen but cap=0 means
        // this caller itself is rejected.
        assert_eq!(reg.allow_request("svc").ok(), Some(false));
        assert_eq!(reg.get_state("svc").ok(), Some(CircuitState::HalfOpen));
    }

    #[test]
    fn f05_concurrent_callers_respect_cap() {
        // Simulate concurrent callers hitting `allow_request` in parallel.
        // With cap=2 across a burst of 8 callers, exactly 2 must be admitted
        // and 6 rejected. Validates that the atomic-under-write-lock accounting
        // holds under real thread contention.
        use std::sync::Arc as StdArc;
        use std::sync::atomic::{AtomicU32, Ordering};
        use std::thread;

        let reg = StdArc::new(make_halfopen(2, 100));

        let admitted = StdArc::new(AtomicU32::new(0));
        let rejected = StdArc::new(AtomicU32::new(0));

        let mut handles = Vec::with_capacity(8);
        for _ in 0..8 {
            let reg_clone = StdArc::clone(&reg);
            let admitted_clone = StdArc::clone(&admitted);
            let rejected_clone = StdArc::clone(&rejected);
            handles.push(thread::spawn(move || {
                match reg_clone.allow_request("svc") {
                    Ok(true) => {
                        admitted_clone.fetch_add(1, Ordering::SeqCst);
                    }
                    Ok(false) => {
                        rejected_clone.fetch_add(1, Ordering::SeqCst);
                    }
                    Err(_) => unreachable!("svc is registered"),
                }
            }));
        }
        for h in handles {
            h.join().unwrap_or_else(|_| unreachable!("thread panic"));
        }

        assert_eq!(
            admitted.load(Ordering::SeqCst),
            2,
            "exactly half_open_max_requests=2 probes admitted under contention"
        );
        assert_eq!(
            rejected.load(Ordering::SeqCst),
            6,
            "remaining 6 callers rejected by the cap"
        );
    }

    #[test]
    fn f05_property_cap_never_exceeded_under_arbitrary_sequences() {
        // Property test via proptest: for any sequence of (cap, burst_size)
        // combinations, the number of admissions in a single HalfOpen window
        // must be <= cap. Uses proptest's default config (256 cases).
        use proptest::prelude::*;

        proptest!(|(cap in 1u32..=16u32, burst in 0usize..=32usize)| {
            let reg = make_halfopen(cap, 1000);
            let mut admits = 0u32;
            for _ in 0..burst {
                if reg.allow_request("svc").unwrap_or(false) {
                    admits = admits.saturating_add(1);
                }
            }
            // Invariant: admissions in a single HalfOpen window <= cap.
            prop_assert!(
                admits <= cap,
                "admitted {admits} probes, cap was {cap}"
            );
        });
    }

    // ── ME-001 regression tests: ProbePermit RAII slot release ──

    /// Reads `in_flight_probes` through the read guard. Used only by tests.
    fn in_flight(reg: &CircuitBreakerRegistry, id: &str) -> u32 {
        reg.breakers
            .read()
            .get(id)
            .map_or(0, |e| e.in_flight_probes)
    }

    #[test]
    fn me001_probe_permit_drop_without_record_releases_slot() {
        let reg = CircuitBreakerRegistry::new();
        let config = CircuitBreakerConfig::builder()
            .failure_threshold(1)
            .open_timeout(Duration::ZERO)
            .half_open_max_requests(2)
            .build();
        reg.register_breaker("svc", config).ok();
        // Drive Closed -> Open -> HalfOpen (first probe consumed by transition).
        reg.record_failure("svc").ok();
        let permit = reg
            .allow_probe("svc")
            .expect("allow_probe should not error")
            .expect("first probe should be admitted");
        assert_eq!(in_flight(&reg, "svc"), 1);
        drop(permit);
        assert_eq!(in_flight(&reg, "svc"), 0);
    }

    #[test]
    fn me001_probe_permit_consumed_by_record_success_no_double_decrement() {
        let reg = CircuitBreakerRegistry::new();
        let config = CircuitBreakerConfig::builder()
            .failure_threshold(1)
            .success_threshold(3)
            .open_timeout(Duration::ZERO)
            .half_open_max_requests(2)
            .build();
        reg.register_breaker("svc", config).ok();
        reg.record_failure("svc").ok();
        let permit = reg.allow_probe("svc").unwrap_or(None).expect("admitted");
        assert_eq!(in_flight(&reg, "svc"), 1);
        // record_success consumes the permit; Drop fires after but skips.
        permit.record_success().ok();
        assert_eq!(in_flight(&reg, "svc"), 0);
    }

    #[test]
    fn me001_probe_permit_panic_releases_slot() {
        use std::sync::Arc;
        let reg = Arc::new(CircuitBreakerRegistry::new());
        let config = CircuitBreakerConfig::builder()
            .failure_threshold(1)
            .open_timeout(Duration::ZERO)
            .half_open_max_requests(2)
            .build();
        reg.register_breaker("svc", config).ok();
        reg.record_failure("svc").ok();
        let reg2 = Arc::clone(&reg);
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
            let _permit = reg2
                .allow_probe("svc")
                .expect("allow_probe")
                .expect("first probe admitted");
            panic!("simulated caller panic mid-probe");
        }));
        assert!(outcome.is_err(), "panic should propagate out of catch_unwind");
        // The permit's Drop must have released the slot despite the panic.
        assert_eq!(in_flight(&reg, "svc"), 0);
    }
}
