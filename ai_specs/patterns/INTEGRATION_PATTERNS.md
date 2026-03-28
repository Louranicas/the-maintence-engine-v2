# Cross-System Integration Patterns Reference

> Integration Patterns for Claude Code & CodeSynthor V7
> Derived from SYNTHEX, The Maintenance Engine, and CodeSynthor V7

---

## Overview

| Metric | Value |
|--------|-------|
| **Patterns** | 12 |
| **Priority** | P2 |
| **Systems Integrated** | 12 ULTRAPLATE services |

---

## Pattern 1: Service Registry (P0)

```rust
use dashmap::DashMap;
use std::sync::Arc;

/// Central registry for all services in the ecosystem
pub struct ServiceRegistry {
    /// Services indexed by ID
    services: Arc<DashMap<String, ServiceRecord>>,

    /// Services indexed by port
    by_port: Arc<DashMap<u16, String>>,

    /// Services indexed by tier
    by_tier: Arc<DashMap<u8, Vec<String>>>,
}

#[derive(Clone, Debug)]
pub struct ServiceRecord {
    pub id: String,
    pub name: String,
    pub version: String,
    pub host: String,
    pub port: u16,
    pub tier: u8,
    pub weight: f64,
    pub protocol: Protocol,
    pub health_endpoint: String,
    pub status: ServiceStatus,
    pub last_health_check: DateTime<Utc>,
    pub metadata: HashMap<String, String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Protocol {
    Rest,
    Grpc,
    WebSocket,
    Tcp,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServiceStatus {
    Unknown,
    Starting,
    Healthy,
    Degraded,
    Unhealthy,
    Stopped,
}

impl ServiceRegistry {
    pub fn new() -> Self {
        Self {
            services: Arc::new(DashMap::new()),
            by_port: Arc::new(DashMap::new()),
            by_tier: Arc::new(DashMap::new()),
        }
    }

    /// Register a service
    pub fn register(&self, service: ServiceRecord) -> Result<()> {
        let id = service.id.clone();
        let port = service.port;
        let tier = service.tier;

        // Check for port conflict
        if let Some(existing) = self.by_port.get(&port) {
            if *existing != id {
                return Err(Error::Conflict(format!("Port {port} already in use")));
            }
        }

        self.by_port.insert(port, id.clone());
        self.by_tier.entry(tier)
            .or_insert_with(Vec::new)
            .push(id.clone());
        self.services.insert(id, service);

        Ok(())
    }

    /// Get service by ID
    pub fn get(&self, id: &str) -> Option<ServiceRecord> {
        self.services.get(id).map(|r| r.clone())
    }

    /// Get all services in a tier
    pub fn by_tier(&self, tier: u8) -> Vec<ServiceRecord> {
        self.by_tier.get(&tier)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.services.get(id).map(|r| r.clone()))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get healthy services
    pub fn healthy(&self) -> Vec<ServiceRecord> {
        self.services.iter()
            .filter(|r| r.status == ServiceStatus::Healthy)
            .map(|r| r.clone())
            .collect()
    }

    /// Update service status
    pub fn update_status(&self, id: &str, status: ServiceStatus) -> Result<()> {
        if let Some(mut service) = self.services.get_mut(id) {
            service.status = status;
            service.last_health_check = Utc::now();
            Ok(())
        } else {
            Err(Error::NotFound(format!("Service {id} not found")))
        }
    }
}
```

**Why**: Service registry enables discovery and health tracking.

---

## Pattern 2: Health Check Protocol (P0)

```rust
use reqwest::Client;
use tokio::time::{timeout, Duration};

/// Health check configuration and execution
pub struct HealthChecker {
    client: Client,
    timeout_ms: u64,
    retry_count: u32,
    retry_delay_ms: u64,
}

#[derive(Clone, Debug)]
pub struct HealthResult {
    pub service_id: String,
    pub status: ServiceStatus,
    pub latency_ms: u64,
    pub details: Option<HealthDetails>,
    pub timestamp: DateTime<Utc>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct HealthDetails {
    pub status: String,
    pub version: Option<String>,
    pub uptime_secs: Option<u64>,
    pub modules_healthy: Option<u32>,
    pub extra: HashMap<String, serde_json::Value>,
}

impl HealthChecker {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(5))
                .build()
                .expect("Failed to build HTTP client"),
            timeout_ms: 5000,
            retry_count: 3,
            retry_delay_ms: 1000,
        }
    }

    /// Check health of a service
    pub async fn check(&self, service: &ServiceRecord) -> HealthResult {
        let start = Instant::now();
        let url = format!("http://{}:{}{}", service.host, service.port, service.health_endpoint);

        let mut last_error = None;

        for attempt in 0..self.retry_count {
            match self.do_check(&url).await {
                Ok(details) => {
                    return HealthResult {
                        service_id: service.id.clone(),
                        status: if details.status == "healthy" {
                            ServiceStatus::Healthy
                        } else {
                            ServiceStatus::Degraded
                        },
                        latency_ms: start.elapsed().as_millis() as u64,
                        details: Some(details),
                        timestamp: Utc::now(),
                    };
                }
                Err(e) => {
                    last_error = Some(e);
                    if attempt < self.retry_count - 1 {
                        tokio::time::sleep(Duration::from_millis(self.retry_delay_ms)).await;
                    }
                }
            }
        }

        tracing::warn!(
            service = %service.id,
            error = ?last_error,
            "Health check failed after retries"
        );

        HealthResult {
            service_id: service.id.clone(),
            status: ServiceStatus::Unhealthy,
            latency_ms: start.elapsed().as_millis() as u64,
            details: None,
            timestamp: Utc::now(),
        }
    }

    async fn do_check(&self, url: &str) -> Result<HealthDetails> {
        let response = timeout(
            Duration::from_millis(self.timeout_ms),
            self.client.get(url).send(),
        )
        .await
        .map_err(|_| Error::Timeout("Health check timed out".into()))??;

        if !response.status().is_success() {
            return Err(Error::HealthCheck(format!(
                "Unhealthy status: {}",
                response.status()
            )));
        }

        response.json().await.map_err(|e| Error::HealthCheck(e.to_string()))
    }

    /// Check all services in parallel
    pub async fn check_all(&self, services: &[ServiceRecord]) -> Vec<HealthResult> {
        let futures: Vec<_> = services.iter()
            .map(|s| self.check(s))
            .collect();

        futures::future::join_all(futures).await
    }
}
```

**Why**: Health checks enable proactive failure detection.

---

## Pattern 3: Event Bus (P1)

```rust
use tokio::sync::broadcast;

/// Cross-service event bus
pub struct EventBus {
    sender: broadcast::Sender<Event>,
    capacity: usize,
}

#[derive(Clone, Debug)]
pub struct Event {
    pub id: String,
    pub event_type: EventType,
    pub source: String,
    pub payload: serde_json::Value,
    pub timestamp: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum EventType {
    ServiceStarted,
    ServiceStopped,
    ServiceHealthChanged,
    ConsensusReached,
    RemediationStarted,
    RemediationCompleted,
    LearningUpdate,
    ConfigChanged,
    AlertTriggered,
    Custom(String),
}

impl EventBus {
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender, capacity }
    }

    /// Publish event to all subscribers
    pub fn publish(&self, event: Event) -> Result<usize> {
        self.sender.send(event)
            .map_err(|_| Error::EventBus("No subscribers".into()))
    }

    /// Subscribe to events
    pub fn subscribe(&self) -> EventSubscriber {
        EventSubscriber {
            receiver: self.sender.subscribe(),
            filters: Vec::new(),
        }
    }
}

pub struct EventSubscriber {
    receiver: broadcast::Receiver<Event>,
    filters: Vec<EventType>,
}

impl EventSubscriber {
    /// Add filter for specific event types
    pub fn filter(mut self, event_type: EventType) -> Self {
        self.filters.push(event_type);
        self
    }

    /// Receive next event (blocking)
    pub async fn recv(&mut self) -> Result<Event> {
        loop {
            let event = self.receiver.recv().await
                .map_err(|e| Error::EventBus(e.to_string()))?;

            if self.filters.is_empty() || self.filters.contains(&event.event_type) {
                return Ok(event);
            }
        }
    }

    /// Try to receive without blocking
    pub fn try_recv(&mut self) -> Result<Option<Event>> {
        match self.receiver.try_recv() {
            Ok(event) => {
                if self.filters.is_empty() || self.filters.contains(&event.event_type) {
                    Ok(Some(event))
                } else {
                    Ok(None)
                }
            }
            Err(broadcast::error::TryRecvError::Empty) => Ok(None),
            Err(e) => Err(Error::EventBus(e.to_string())),
        }
    }
}
```

**Why**: Event bus enables loose coupling between services.

---

## Pattern 4: Circuit Breaker (P1)

```rust
use std::sync::atomic::{AtomicU32, Ordering};
use parking_lot::RwLock;

/// Circuit breaker for fault tolerance
pub struct CircuitBreaker {
    state: RwLock<CircuitState>,
    failure_count: AtomicU32,
    success_count: AtomicU32,
    failure_threshold: u32,
    success_threshold: u32,
    timeout: Duration,
    last_failure: RwLock<Option<Instant>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CircuitState {
    /// Circuit is closed, requests flow through
    Closed,
    /// Circuit is open, requests are rejected
    Open,
    /// Circuit is testing, limited requests allowed
    HalfOpen,
}

impl CircuitBreaker {
    pub fn new(failure_threshold: u32, success_threshold: u32, timeout: Duration) -> Self {
        Self {
            state: RwLock::new(CircuitState::Closed),
            failure_count: AtomicU32::new(0),
            success_count: AtomicU32::new(0),
            failure_threshold,
            success_threshold,
            timeout,
            last_failure: RwLock::new(None),
        }
    }

    /// Check if request is allowed
    pub fn allow_request(&self) -> bool {
        match *self.state.read() {
            CircuitState::Closed => true,
            CircuitState::Open => {
                // Check if timeout has elapsed
                if let Some(last) = *self.last_failure.read() {
                    if last.elapsed() >= self.timeout {
                        *self.state.write() = CircuitState::HalfOpen;
                        self.success_count.store(0, Ordering::SeqCst);
                        return true;
                    }
                }
                false
            }
            CircuitState::HalfOpen => {
                // Allow limited requests for testing
                true
            }
        }
    }

    /// Record successful request
    pub fn record_success(&self) {
        self.failure_count.store(0, Ordering::SeqCst);

        let state = *self.state.read();
        if state == CircuitState::HalfOpen {
            let count = self.success_count.fetch_add(1, Ordering::SeqCst) + 1;
            if count >= self.success_threshold {
                *self.state.write() = CircuitState::Closed;
                tracing::info!("Circuit breaker closed after {} successes", count);
            }
        }
    }

    /// Record failed request
    pub fn record_failure(&self) {
        let state = *self.state.read();

        match state {
            CircuitState::Closed => {
                let count = self.failure_count.fetch_add(1, Ordering::SeqCst) + 1;
                if count >= self.failure_threshold {
                    *self.state.write() = CircuitState::Open;
                    *self.last_failure.write() = Some(Instant::now());
                    tracing::warn!("Circuit breaker opened after {} failures", count);
                }
            }
            CircuitState::HalfOpen => {
                // Single failure in half-open reopens circuit
                *self.state.write() = CircuitState::Open;
                *self.last_failure.write() = Some(Instant::now());
                self.success_count.store(0, Ordering::SeqCst);
                tracing::warn!("Circuit breaker reopened after failure in half-open state");
            }
            CircuitState::Open => {
                // Already open, just update timestamp
                *self.last_failure.write() = Some(Instant::now());
            }
        }
    }

    /// Get current state
    pub fn state(&self) -> CircuitState {
        *self.state.read()
    }

    /// Execute with circuit breaker protection
    pub async fn execute<F, T, E>(&self, f: F) -> Result<T>
    where
        F: Future<Output = std::result::Result<T, E>>,
        E: std::fmt::Display,
    {
        if !self.allow_request() {
            return Err(Error::CircuitOpen("Circuit breaker is open".into()));
        }

        match f.await {
            Ok(result) => {
                self.record_success();
                Ok(result)
            }
            Err(e) => {
                self.record_failure();
                Err(Error::ServiceError(e.to_string()))
            }
        }
    }
}
```

**Why**: Circuit breakers prevent cascade failures.

---

## Pattern 5: Retry with Backoff (P1)

```rust
/// Retry configuration with exponential backoff
pub struct RetryPolicy {
    pub max_attempts: u32,
    pub initial_delay_ms: u64,
    pub max_delay_ms: u64,
    pub multiplier: f64,
    pub jitter: bool,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_delay_ms: 100,
            max_delay_ms: 10_000,
            multiplier: 2.0,
            jitter: true,
        }
    }
}

impl RetryPolicy {
    /// Calculate delay for attempt n (0-indexed)
    pub fn delay_for_attempt(&self, attempt: u32) -> Duration {
        let base_delay = self.initial_delay_ms as f64 * self.multiplier.powi(attempt as i32);
        let capped_delay = base_delay.min(self.max_delay_ms as f64);

        let final_delay = if self.jitter {
            let jitter_factor = 0.5 + rand::random::<f64>(); // 0.5 to 1.5
            capped_delay * jitter_factor
        } else {
            capped_delay
        };

        Duration::from_millis(final_delay as u64)
    }

    /// Execute with retry
    pub async fn execute<F, Fut, T, E>(&self, mut f: F) -> Result<T>
    where
        F: FnMut() -> Fut,
        Fut: Future<Output = std::result::Result<T, E>>,
        E: std::fmt::Display,
    {
        let mut last_error = None;

        for attempt in 0..self.max_attempts {
            match f().await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    last_error = Some(e.to_string());

                    if attempt < self.max_attempts - 1 {
                        let delay = self.delay_for_attempt(attempt);
                        tracing::debug!(
                            attempt = attempt + 1,
                            max = self.max_attempts,
                            delay_ms = delay.as_millis(),
                            "Retrying after failure"
                        );
                        tokio::time::sleep(delay).await;
                    }
                }
            }
        }

        Err(Error::MaxRetriesExceeded(last_error.unwrap_or_default()))
    }
}

/// Retry with circuit breaker
pub struct ResilientClient {
    circuit_breaker: CircuitBreaker,
    retry_policy: RetryPolicy,
}

impl ResilientClient {
    pub fn new() -> Self {
        Self {
            circuit_breaker: CircuitBreaker::new(5, 3, Duration::from_secs(30)),
            retry_policy: RetryPolicy::default(),
        }
    }

    pub async fn call<F, Fut, T, E>(&self, f: F) -> Result<T>
    where
        F: Fn() -> Fut + Clone,
        Fut: Future<Output = std::result::Result<T, E>>,
        E: std::fmt::Display,
    {
        self.circuit_breaker.execute(
            self.retry_policy.execute(f)
        ).await
    }
}
```

**Why**: Exponential backoff with jitter prevents thundering herd.

---

## Pattern 6: Service Mesh Client (P1)

```rust
/// Client for inter-service communication
pub struct ServiceClient {
    registry: Arc<ServiceRegistry>,
    clients: DashMap<String, ResilientClient>,
    http_client: reqwest::Client,
}

impl ServiceClient {
    pub fn new(registry: Arc<ServiceRegistry>) -> Self {
        Self {
            registry,
            clients: DashMap::new(),
            http_client: reqwest::Client::new(),
        }
    }

    /// Get or create resilient client for service
    fn get_client(&self, service_id: &str) -> ResilientClient {
        self.clients.entry(service_id.to_string())
            .or_insert_with(ResilientClient::new)
            .clone()
    }

    /// Call service endpoint
    pub async fn get(&self, service_id: &str, path: &str) -> Result<serde_json::Value> {
        let service = self.registry.get(service_id)
            .ok_or_else(|| Error::NotFound(format!("Service {service_id} not found")))?;

        let url = format!("http://{}:{}{}", service.host, service.port, path);
        let client = self.get_client(service_id);

        let http_client = self.http_client.clone();
        client.call(|| async {
            http_client.get(&url)
                .send()
                .await?
                .json::<serde_json::Value>()
                .await
        }).await
    }

    /// POST to service endpoint
    pub async fn post<T: Serialize>(&self, service_id: &str, path: &str, body: &T) -> Result<serde_json::Value> {
        let service = self.registry.get(service_id)
            .ok_or_else(|| Error::NotFound(format!("Service {service_id} not found")))?;

        let url = format!("http://{}:{}{}", service.host, service.port, path);
        let client = self.get_client(service_id);

        let http_client = self.http_client.clone();
        let body_json = serde_json::to_string(body)?;

        client.call(|| {
            let http_client = http_client.clone();
            let url = url.clone();
            let body = body_json.clone();
            async move {
                http_client.post(&url)
                    .header("Content-Type", "application/json")
                    .body(body)
                    .send()
                    .await?
                    .json::<serde_json::Value>()
                    .await
            }
        }).await
    }
}
```

**Why**: Service mesh client provides resilient inter-service communication.

---

## Pattern 7: Configuration Sync (P1)

```rust
/// Cross-service configuration synchronization
pub struct ConfigSync {
    local_config: Arc<RwLock<HashMap<String, serde_json::Value>>>,
    event_bus: Arc<EventBus>,
    version: AtomicU64,
}

impl ConfigSync {
    pub fn new(event_bus: Arc<EventBus>) -> Self {
        Self {
            local_config: Arc::new(RwLock::new(HashMap::new())),
            event_bus,
            version: AtomicU64::new(0),
        }
    }

    /// Set configuration value
    pub fn set(&self, key: &str, value: serde_json::Value) -> Result<()> {
        {
            let mut config = self.local_config.write();
            config.insert(key.to_string(), value.clone());
        }

        let version = self.version.fetch_add(1, Ordering::SeqCst) + 1;

        // Publish change event
        self.event_bus.publish(Event {
            id: uuid::Uuid::new_v4().to_string(),
            event_type: EventType::ConfigChanged,
            source: "config_sync".to_string(),
            payload: serde_json::json!({
                "key": key,
                "value": value,
                "version": version,
            }),
            timestamp: Utc::now(),
        })?;

        Ok(())
    }

    /// Get configuration value
    pub fn get(&self, key: &str) -> Option<serde_json::Value> {
        self.local_config.read().get(key).cloned()
    }

    /// Get typed configuration value
    pub fn get_typed<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>> {
        match self.get(key) {
            Some(value) => {
                let typed = serde_json::from_value(value)
                    .map_err(|e| Error::Config(e.to_string()))?;
                Ok(Some(typed))
            }
            None => Ok(None),
        }
    }

    /// Subscribe to config changes
    pub async fn watch(&self, key: &str) -> impl Stream<Item = serde_json::Value> {
        let key = key.to_string();
        let mut subscriber = self.event_bus.subscribe()
            .filter(EventType::ConfigChanged);

        async_stream::stream! {
            while let Ok(event) = subscriber.recv().await {
                if let Some(event_key) = event.payload.get("key").and_then(|k| k.as_str()) {
                    if event_key == key {
                        if let Some(value) = event.payload.get("value") {
                            yield value.clone();
                        }
                    }
                }
            }
        }
    }
}
```

**Why**: Configuration sync enables dynamic reconfiguration across services.

---

## Pattern 8: Distributed Tracing (P2)

```rust
use tracing::{span, Level, Instrument};

/// Distributed trace context
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TraceContext {
    pub trace_id: String,
    pub span_id: String,
    pub parent_span_id: Option<String>,
    pub service: String,
    pub operation: String,
    pub start_time: DateTime<Utc>,
    pub baggage: HashMap<String, String>,
}

impl TraceContext {
    /// Create new trace
    pub fn new(service: &str, operation: &str) -> Self {
        Self {
            trace_id: uuid::Uuid::new_v4().to_string(),
            span_id: uuid::Uuid::new_v4().to_string(),
            parent_span_id: None,
            service: service.to_string(),
            operation: operation.to_string(),
            start_time: Utc::now(),
            baggage: HashMap::new(),
        }
    }

    /// Create child span
    pub fn child(&self, operation: &str) -> Self {
        Self {
            trace_id: self.trace_id.clone(),
            span_id: uuid::Uuid::new_v4().to_string(),
            parent_span_id: Some(self.span_id.clone()),
            service: self.service.clone(),
            operation: operation.to_string(),
            start_time: Utc::now(),
            baggage: self.baggage.clone(),
        }
    }

    /// Serialize for header propagation
    pub fn to_header(&self) -> String {
        format!(
            "00-{}-{}-01",
            self.trace_id,
            self.span_id
        )
    }

    /// Parse from header
    pub fn from_header(header: &str, service: &str, operation: &str) -> Result<Self> {
        let parts: Vec<&str> = header.split('-').collect();
        if parts.len() < 4 {
            return Err(Error::Tracing("Invalid trace header format".into()));
        }

        Ok(Self {
            trace_id: parts[1].to_string(),
            span_id: uuid::Uuid::new_v4().to_string(),
            parent_span_id: Some(parts[2].to_string()),
            service: service.to_string(),
            operation: operation.to_string(),
            start_time: Utc::now(),
            baggage: HashMap::new(),
        })
    }
}

/// Trace span wrapper
pub struct TracedOperation {
    context: TraceContext,
    start: Instant,
}

impl TracedOperation {
    pub fn start(context: TraceContext) -> Self {
        tracing::info!(
            trace_id = %context.trace_id,
            span_id = %context.span_id,
            operation = %context.operation,
            "Span started"
        );

        Self {
            context,
            start: Instant::now(),
        }
    }

    pub fn finish(self, status: &str) {
        let duration = self.start.elapsed();

        tracing::info!(
            trace_id = %self.context.trace_id,
            span_id = %self.context.span_id,
            operation = %self.context.operation,
            duration_ms = duration.as_millis(),
            status = %status,
            "Span finished"
        );
    }

    pub fn error(self, error: &str) {
        let duration = self.start.elapsed();

        tracing::error!(
            trace_id = %self.context.trace_id,
            span_id = %self.context.span_id,
            operation = %self.context.operation,
            duration_ms = duration.as_millis(),
            error = %error,
            "Span failed"
        );
    }
}
```

**Why**: Distributed tracing enables debugging across service boundaries.

---

## Pattern 9: Synergy Score Calculation (P2)

```rust
/// Calculate synergy between services
pub struct SynergyCalculator {
    /// Weights for synergy factors
    weights: SynergyWeights,
}

#[derive(Clone, Debug)]
pub struct SynergyWeights {
    pub health_correlation: f64,
    pub call_success_rate: f64,
    pub latency_similarity: f64,
    pub shared_dependencies: f64,
    pub tier_adjacency: f64,
}

impl Default for SynergyWeights {
    fn default() -> Self {
        Self {
            health_correlation: 0.25,
            call_success_rate: 0.30,
            latency_similarity: 0.15,
            shared_dependencies: 0.15,
            tier_adjacency: 0.15,
        }
    }
}

#[derive(Clone, Debug)]
pub struct SynergyResult {
    pub source: String,
    pub target: String,
    pub score: f64,
    pub factors: SynergyFactors,
    pub calculated_at: DateTime<Utc>,
}

#[derive(Clone, Debug)]
pub struct SynergyFactors {
    pub health_correlation: f64,
    pub call_success_rate: f64,
    pub latency_similarity: f64,
    pub shared_dependencies: f64,
    pub tier_adjacency: f64,
}

impl SynergyCalculator {
    pub fn new() -> Self {
        Self {
            weights: SynergyWeights::default(),
        }
    }

    /// Calculate synergy between two services
    pub fn calculate(&self, source: &ServiceMetrics, target: &ServiceMetrics) -> SynergyResult {
        let factors = SynergyFactors {
            health_correlation: self.health_correlation(source, target),
            call_success_rate: self.call_success_rate(source, target),
            latency_similarity: self.latency_similarity(source, target),
            shared_dependencies: self.shared_dependencies(source, target),
            tier_adjacency: self.tier_adjacency(source, target),
        };

        let score =
            self.weights.health_correlation * factors.health_correlation +
            self.weights.call_success_rate * factors.call_success_rate +
            self.weights.latency_similarity * factors.latency_similarity +
            self.weights.shared_dependencies * factors.shared_dependencies +
            self.weights.tier_adjacency * factors.tier_adjacency;

        SynergyResult {
            source: source.service_id.clone(),
            target: target.service_id.clone(),
            score: score.clamp(0.0, 1.0),
            factors,
            calculated_at: Utc::now(),
        }
    }

    fn health_correlation(&self, source: &ServiceMetrics, target: &ServiceMetrics) -> f64 {
        // Correlation of health scores over time
        1.0 - (source.health_score - target.health_score).abs()
    }

    fn call_success_rate(&self, source: &ServiceMetrics, target: &ServiceMetrics) -> f64 {
        // Success rate of calls from source to target
        source.call_success_rate.get(&target.service_id)
            .copied()
            .unwrap_or(0.5)
    }

    fn latency_similarity(&self, source: &ServiceMetrics, target: &ServiceMetrics) -> f64 {
        // How similar are their latencies
        let max_latency = source.avg_latency_ms.max(target.avg_latency_ms);
        if max_latency == 0.0 {
            return 1.0;
        }
        let diff = (source.avg_latency_ms - target.avg_latency_ms).abs();
        1.0 - (diff / max_latency)
    }

    fn shared_dependencies(&self, source: &ServiceMetrics, target: &ServiceMetrics) -> f64 {
        // Jaccard similarity of dependencies
        let source_deps: HashSet<_> = source.dependencies.iter().collect();
        let target_deps: HashSet<_> = target.dependencies.iter().collect();

        let intersection = source_deps.intersection(&target_deps).count();
        let union = source_deps.union(&target_deps).count();

        if union == 0 {
            return 0.0;
        }

        intersection as f64 / union as f64
    }

    fn tier_adjacency(&self, source: &ServiceMetrics, target: &ServiceMetrics) -> f64 {
        // Adjacent tiers have higher synergy
        let tier_diff = (source.tier as i32 - target.tier as i32).abs();
        match tier_diff {
            0 => 1.0,   // Same tier
            1 => 0.8,   // Adjacent
            2 => 0.5,   // Two apart
            _ => 0.2,   // Distant
        }
    }
}

#[derive(Clone, Debug)]
pub struct ServiceMetrics {
    pub service_id: String,
    pub tier: u8,
    pub health_score: f64,
    pub avg_latency_ms: f64,
    pub dependencies: Vec<String>,
    pub call_success_rate: HashMap<String, f64>,
}
```

**Why**: Synergy scores enable intelligent routing and dependency management.

---

## Pattern 10: Escalation Tiers (P2)

```rust
/// Escalation tier determination
pub struct EscalationManager {
    thresholds: EscalationThresholds,
}

#[derive(Clone, Debug)]
pub struct EscalationThresholds {
    /// Confidence threshold for auto-execution
    pub auto_execute: f64,    // Default: 0.9
    /// Confidence threshold for notification
    pub notify: f64,          // Default: 0.7
    /// Maximum severity for auto-execution
    pub max_auto_severity: Severity,
}

impl Default for EscalationThresholds {
    fn default() -> Self {
        Self {
            auto_execute: 0.9,
            notify: 0.7,
            max_auto_severity: Severity::Medium,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum EscalationTier {
    /// Automatic execution
    L0AutoExecute,
    /// Notify human, proceed
    L1Notify,
    /// Require human approval
    L2Approval,
    /// Require PBFT consensus
    L3Consensus,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Clone, Debug)]
pub struct Action {
    pub id: String,
    pub action_type: ActionType,
    pub confidence: f64,
    pub severity: Severity,
    pub target_service: String,
    pub description: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ActionType {
    Restart,
    ScaleUp,
    ScaleDown,
    Migrate,
    ConfigChange,
    Kill,
    Custom(String),
}

impl EscalationManager {
    pub fn new() -> Self {
        Self {
            thresholds: EscalationThresholds::default(),
        }
    }

    /// Determine escalation tier for action
    pub fn determine_tier(&self, action: &Action) -> EscalationTier {
        // Critical actions always require consensus
        if action.severity == Severity::Critical {
            return EscalationTier::L3Consensus;
        }

        // Destructive actions require consensus
        if matches!(action.action_type, ActionType::Kill | ActionType::Migrate) {
            return EscalationTier::L3Consensus;
        }

        // High confidence + low severity = auto-execute
        if action.confidence >= self.thresholds.auto_execute
            && action.severity <= self.thresholds.max_auto_severity
        {
            return EscalationTier::L0AutoExecute;
        }

        // Medium confidence = notify
        if action.confidence >= self.thresholds.notify {
            return EscalationTier::L1Notify;
        }

        // Low confidence = require approval
        EscalationTier::L2Approval
    }

    /// Get timeout for tier
    pub fn timeout_for_tier(&self, tier: EscalationTier) -> Duration {
        match tier {
            EscalationTier::L0AutoExecute => Duration::ZERO,
            EscalationTier::L1Notify => Duration::from_secs(300),    // 5 min
            EscalationTier::L2Approval => Duration::from_secs(1800), // 30 min
            EscalationTier::L3Consensus => Duration::from_secs(300), // 5 min (PBFT)
        }
    }
}
```

**Why**: Escalation tiers balance automation with human oversight.

---

## Pattern 11: Multi-Database Query (P2)

```rust
/// Execute queries across multiple databases
pub struct CrossDatabaseQuery {
    pools: HashMap<String, DbPool>,
}

impl CrossDatabaseQuery {
    pub fn new() -> Self {
        Self {
            pools: HashMap::new(),
        }
    }

    /// Register database
    pub fn register(&mut self, name: &str, pool: DbPool) {
        self.pools.insert(name.to_string(), pool);
    }

    /// Query with ATTACH
    pub fn query_with_attach<T, F>(
        &self,
        primary: &str,
        attached: &[(&str, &str)], // (alias, db_name)
        sql: &str,
        mapper: F,
    ) -> Result<Vec<T>>
    where
        F: Fn(&rusqlite::Row<'_>) -> rusqlite::Result<T>,
    {
        let pool = self.pools.get(primary)
            .ok_or_else(|| Error::Database(format!("Unknown database: {primary}")))?;

        let conn = pool.get()?;

        // Attach databases
        for (alias, db_name) in attached {
            let db_pool = self.pools.get(*db_name)
                .ok_or_else(|| Error::Database(format!("Unknown database: {db_name}")))?;

            // Get path from pool (implementation-specific)
            let path = format!("data/{db_name}.db");
            conn.execute(&format!("ATTACH DATABASE ?1 AS {alias}"), [&path])?;
        }

        // Execute query
        let mut stmt = conn.prepare(sql)?;
        let rows = stmt.query_map([], mapper)?;
        let results: Vec<T> = rows.collect::<rusqlite::Result<Vec<_>>>()?;

        // Detach databases
        for (alias, _) in attached {
            conn.execute(&format!("DETACH DATABASE {alias}"), [])?;
        }

        Ok(results)
    }
}

// Example: Query across service_tracking and hebbian_pulse
// let results = cross_db.query_with_attach(
//     "service_tracking",
//     &[("hebbian", "hebbian_pulse")],
//     "SELECT s.name, h.strength
//      FROM services s
//      JOIN hebbian.pathways h ON s.id = h.source_id
//      WHERE h.strength > 0.5",
//     |row| Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?))
// )?;
```

**Why**: Cross-database queries enable unified views across subsystems.

---

## Pattern 12: Integration Health Dashboard (P2)

```rust
/// Aggregated integration health view
pub struct IntegrationDashboard {
    registry: Arc<ServiceRegistry>,
    health_checker: Arc<HealthChecker>,
    synergy_calculator: Arc<SynergyCalculator>,
}

#[derive(Clone, Debug, Serialize)]
pub struct DashboardState {
    pub timestamp: DateTime<Utc>,
    pub services: Vec<ServiceSummary>,
    pub overall_health: f64,
    pub overall_synergy: f64,
    pub active_circuits: Vec<CircuitStatus>,
    pub recent_events: Vec<EventSummary>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ServiceSummary {
    pub id: String,
    pub name: String,
    pub tier: u8,
    pub status: ServiceStatus,
    pub health_score: f64,
    pub synergy_score: f64,
    pub last_check: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize)]
pub struct CircuitStatus {
    pub service_id: String,
    pub state: CircuitState,
    pub failure_count: u32,
}

#[derive(Clone, Debug, Serialize)]
pub struct EventSummary {
    pub id: String,
    pub event_type: String,
    pub source: String,
    pub timestamp: DateTime<Utc>,
}

impl IntegrationDashboard {
    pub fn new(
        registry: Arc<ServiceRegistry>,
        health_checker: Arc<HealthChecker>,
        synergy_calculator: Arc<SynergyCalculator>,
    ) -> Self {
        Self {
            registry,
            health_checker,
            synergy_calculator,
        }
    }

    /// Get current dashboard state
    pub async fn state(&self) -> DashboardState {
        let services = self.registry.healthy();

        // Check health of all services
        let health_results = self.health_checker.check_all(&services).await;

        // Build service summaries
        let summaries: Vec<ServiceSummary> = services.iter()
            .zip(health_results.iter())
            .map(|(s, h)| ServiceSummary {
                id: s.id.clone(),
                name: s.name.clone(),
                tier: s.tier,
                status: h.status,
                health_score: h.details.as_ref()
                    .and_then(|d| d.extra.get("health_score"))
                    .and_then(|v| v.as_f64())
                    .unwrap_or(if h.status == ServiceStatus::Healthy { 1.0 } else { 0.0 }),
                synergy_score: 0.0, // Calculated separately
                last_check: h.timestamp,
            })
            .collect();

        // Calculate overall health
        let overall_health = if summaries.is_empty() {
            1.0
        } else {
            summaries.iter().map(|s| s.health_score).sum::<f64>() / summaries.len() as f64
        };

        DashboardState {
            timestamp: Utc::now(),
            services: summaries,
            overall_health,
            overall_synergy: 0.0, // Calculated from synergy matrix
            active_circuits: Vec::new(),
            recent_events: Vec::new(),
        }
    }
}
```

**Why**: Dashboard provides unified view of integration health.

---

## ULTRAPLATE Service Integration Matrix

```
                 │ SYN │ K7  │ NAI │ CSV │ DEV │ TL  │ LA  │ CCM │ PS  │ AA  │ BE  │ TM  │
─────────────────┼─────┼─────┼─────┼─────┼─────┼─────┼─────┼─────┼─────┼─────┼─────┼─────┤
SYNTHEX          │  -  │ ✓   │ ✓   │     │ ✓   │ ✓   │     │ ✓   │     │     │ ✓   │ ✓   │
SAN-K7           │ ✓   │  -  │ ✓   │ ✓   │     │ ✓   │ ✓   │     │ ✓   │ ✓   │     │     │
NAIS             │ ✓   │ ✓   │  -  │ ✓   │     │     │     │     │     │     │     │     │
CodeSynthor V7   │     │ ✓   │ ✓   │  -  │ ✓   │ ✓   │     │ ✓   │     │ ✓   │     │     │
DevOps Engine    │ ✓   │     │     │ ✓   │  -  │ ✓   │     │     │     │     │ ✓   │ ✓   │
Tool Library     │ ✓   │ ✓   │     │ ✓   │ ✓   │  -  │ ✓   │     │     │ ✓   │     │ ✓   │
Library Agent    │     │ ✓   │     │     │     │ ✓   │  -  │ ✓   │     │     │     │     │
CCM              │ ✓   │     │     │ ✓   │     │     │ ✓   │  -  │     │     │     │     │
Prometheus Swarm │     │ ✓   │     │     │     │     │     │     │  -  │     │     │     │
Architect Agent  │     │ ✓   │     │ ✓   │     │ ✓   │     │     │     │  -  │     │     │
Bash Engine      │ ✓   │     │     │     │ ✓   │     │     │     │     │     │  -  │ ✓   │
Tool Maker       │ ✓   │     │     │     │ ✓   │ ✓   │     │     │     │     │ ✓   │  -  │
```

---

*Generated: 2026-01-28 | The Maintenance Engine v1.0.0*
