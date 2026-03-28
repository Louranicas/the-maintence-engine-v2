# Concurrency Patterns

> Claude Code Optimized - 12 Thread-Safe Patterns

---

## C01: DashMap for Lock-Free Access

**Priority**: P0 (Critical)
**Source**: SYNTHEX

```rust
use dashmap::DashMap;
use std::sync::Arc;

/// Lock-free concurrent registry
pub struct ServiceRegistry {
    services: Arc<DashMap<String, ServiceState>>,
}

impl ServiceRegistry {
    pub fn new() -> Self {
        Self {
            services: Arc::new(DashMap::new()),
        }
    }

    /// Non-blocking insert
    pub fn register(&self, id: String, state: ServiceState) {
        self.services.insert(id, state);
    }

    /// Non-blocking read (returns cloned value)
    pub fn get(&self, id: &str) -> Option<ServiceState> {
        self.services.get(id).map(|r| r.clone())
    }

    /// Update in place
    pub fn update_health(&self, id: &str, health: f64) {
        if let Some(mut entry) = self.services.get_mut(id) {
            entry.health_score = health;
        }
    }

    /// Iterate without blocking writers
    pub fn healthy_services(&self) -> Vec<ServiceState> {
        self.services
            .iter()
            .filter(|r| r.health_score > 0.8)
            .map(|r| r.clone())
            .collect()
    }

    /// Atomic remove and return
    pub fn deregister(&self, id: &str) -> Option<ServiceState> {
        self.services.remove(id).map(|(_, v)| v)
    }
}
```

**Why DashMap**:
- Readers never block readers
- Writers only block writers on same shard
- 16x sharding by default
- No deadlock risk

---

## C02: Arc + RwLock for Complex State

**Priority**: P1
**Source**: The Maintenance Engine

```rust
use std::sync::{Arc, RwLock};

/// State that needs atomic multi-field updates
pub struct ConsensusState {
    inner: Arc<RwLock<ConsensusStateInner>>,
}

struct ConsensusStateInner {
    view: u64,
    sequence: u64,
    leader_id: String,
    pending_proposals: Vec<Proposal>,
}

impl ConsensusState {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(ConsensusStateInner {
                view: 0,
                sequence: 0,
                leader_id: String::new(),
                pending_proposals: Vec::new(),
            })),
        }
    }

    /// Read-only access (many readers allowed)
    pub fn current_view(&self) -> u64 {
        self.inner.read().map(|s| s.view).unwrap_or(0)
    }

    /// Write access (exclusive)
    pub fn advance_view(&self) -> u64 {
        if let Ok(mut state) = self.inner.write() {
            state.view += 1;
            state.view
        } else {
            0
        }
    }

    /// Atomic multi-field update
    pub fn start_view_change(&self, new_leader: String) -> Result<()> {
        let mut state = self.inner.write()
            .map_err(|_| Error::Other("Lock poisoned".into()))?;

        state.view += 1;
        state.leader_id = new_leader;
        state.pending_proposals.clear();

        Ok(())
    }
}
```

**When to use RwLock**:
- Need atomic multi-field updates
- Read-heavy workload (>80% reads)
- Complex invariants between fields

---

## C03: Tokio Channels for Async Communication

**Priority**: P0
**Source**: All codebases

```rust
use tokio::sync::{mpsc, broadcast, oneshot};

/// MPSC: Multiple producers, single consumer
pub struct EventProcessor {
    tx: mpsc::Sender<Event>,
}

impl EventProcessor {
    pub fn new(buffer_size: usize) -> (Self, mpsc::Receiver<Event>) {
        let (tx, rx) = mpsc::channel(buffer_size);
        (Self { tx }, rx)
    }

    pub async fn send(&self, event: Event) -> Result<()> {
        self.tx.send(event).await
            .map_err(|e| Error::Other(format!("Channel closed: {}", e)))
    }
}

/// Broadcast: Single producer, multiple consumers
pub struct EventBroadcaster {
    tx: broadcast::Sender<Event>,
}

impl EventBroadcaster {
    pub fn new(capacity: usize) -> Self {
        let (tx, _) = broadcast::channel(capacity);
        Self { tx }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.tx.subscribe()
    }

    pub fn broadcast(&self, event: Event) -> Result<usize> {
        self.tx.send(event)
            .map_err(|_| Error::Other("No subscribers".into()))
    }
}

/// Oneshot: Single value, single response
pub async fn request_response(
    tx: mpsc::Sender<(Request, oneshot::Sender<Response>)>,
    request: Request,
) -> Result<Response> {
    let (response_tx, response_rx) = oneshot::channel();
    tx.send((request, response_tx)).await?;
    response_rx.await
        .map_err(|_| Error::Other("Response channel dropped".into()))
}
```

---

## C04: Async Mutex for Rare Writes

**Priority**: P1
**Source**: SYNTHEX

```rust
use tokio::sync::Mutex;

/// Use async Mutex when:
/// - Writes are rare (< 10% of operations)
/// - Lock is held across .await points
pub struct ConfigManager {
    config: Arc<Mutex<Config>>,
}

impl ConfigManager {
    pub async fn reload(&self, path: &str) -> Result<()> {
        let new_config = load_config(path).await?;

        let mut config = self.config.lock().await;
        *config = new_config;

        Ok(())
    }

    pub async fn get_timeout(&self) -> u64 {
        self.config.lock().await.timeout_ms
    }
}

// ANTI-PATTERN: Don't use std::sync::Mutex in async code
// let guard = std_mutex.lock().unwrap(); // Blocks thread!
// some_async_op().await; // Other tasks can't run!
```

---

## C05: Semaphore for Resource Limiting

**Priority**: P1
**Source**: The Maintenance Engine

```rust
use tokio::sync::Semaphore;

/// Limit concurrent database connections
pub struct ConnectionPool {
    semaphore: Arc<Semaphore>,
    max_connections: usize,
}

impl ConnectionPool {
    pub fn new(max_connections: usize) -> Self {
        Self {
            semaphore: Arc::new(Semaphore::new(max_connections)),
            max_connections,
        }
    }

    pub async fn acquire(&self) -> Result<Connection> {
        let permit = self.semaphore
            .acquire()
            .await
            .map_err(|_| Error::Other("Semaphore closed".into()))?;

        // Connection holds permit, releases on drop
        Ok(Connection::new(permit))
    }

    pub fn available(&self) -> usize {
        self.semaphore.available_permits()
    }
}

/// Limit concurrent API requests
pub async fn rate_limited_request<T, F, Fut>(
    semaphore: &Semaphore,
    f: F,
) -> Result<T>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<T>>,
{
    let _permit = semaphore.acquire().await
        .map_err(|_| Error::Other("Rate limit semaphore closed".into()))?;

    f().await
}
```

---

## C06: Notify for Event Signaling

**Priority**: P2
**Source**: CodeSynthor V7

```rust
use tokio::sync::Notify;

/// Efficient wake-up signaling
pub struct ShutdownSignal {
    notify: Arc<Notify>,
}

impl ShutdownSignal {
    pub fn new() -> Self {
        Self {
            notify: Arc::new(Notify::new()),
        }
    }

    pub fn trigger(&self) {
        self.notify.notify_waiters();
    }

    pub async fn wait(&self) {
        self.notify.notified().await;
    }
}

// Usage in service loop
async fn run_service(shutdown: ShutdownSignal) {
    loop {
        tokio::select! {
            _ = shutdown.wait() => {
                tracing::info!("Shutdown signal received");
                break;
            }
            result = process_next() => {
                if let Err(e) = result {
                    tracing::warn!(error = %e, "Processing error");
                }
            }
        }
    }
}
```

---

## C07: Watch for Configuration Updates

**Priority**: P1
**Source**: SYNTHEX

```rust
use tokio::sync::watch;

/// Broadcast configuration changes to all watchers
pub struct ConfigWatcher {
    tx: watch::Sender<Config>,
    rx: watch::Receiver<Config>,
}

impl ConfigWatcher {
    pub fn new(initial: Config) -> Self {
        let (tx, rx) = watch::channel(initial);
        Self { tx, rx }
    }

    pub fn update(&self, config: Config) -> Result<()> {
        self.tx.send(config)
            .map_err(|_| Error::Other("All receivers dropped".into()))
    }

    pub fn subscribe(&self) -> watch::Receiver<Config> {
        self.rx.clone()
    }
}

// Consumer usage
async fn config_consumer(mut rx: watch::Receiver<Config>) {
    while rx.changed().await.is_ok() {
        let config = rx.borrow();
        tracing::info!(timeout = config.timeout_ms, "Config updated");
    }
}
```

---

## C08: Actor Pattern

**Priority**: P2
**Source**: The Maintenance Engine

```rust
use tokio::sync::mpsc;

/// Actor message types
pub enum HealthActorMessage {
    Check { service_id: String, respond_to: oneshot::Sender<HealthStatus> },
    UpdateThreshold { new_threshold: f64 },
    Shutdown,
}

/// Actor with private state
pub struct HealthActor {
    receiver: mpsc::Receiver<HealthActorMessage>,
    threshold: f64,
    cache: HashMap<String, HealthStatus>,
}

impl HealthActor {
    pub fn new(receiver: mpsc::Receiver<HealthActorMessage>) -> Self {
        Self {
            receiver,
            threshold: 0.8,
            cache: HashMap::new(),
        }
    }

    pub async fn run(mut self) {
        while let Some(msg) = self.receiver.recv().await {
            match msg {
                HealthActorMessage::Check { service_id, respond_to } => {
                    let status = self.check_health(&service_id).await;
                    let _ = respond_to.send(status);
                }
                HealthActorMessage::UpdateThreshold { new_threshold } => {
                    self.threshold = new_threshold;
                }
                HealthActorMessage::Shutdown => {
                    break;
                }
            }
        }
    }

    async fn check_health(&mut self, service_id: &str) -> HealthStatus {
        // Actor owns its state - no locks needed
        if let Some(cached) = self.cache.get(service_id) {
            return cached.clone();
        }

        let status = perform_health_check(service_id).await;
        self.cache.insert(service_id.to_string(), status.clone());
        status
    }
}

/// Actor handle for sending messages
#[derive(Clone)]
pub struct HealthActorHandle {
    sender: mpsc::Sender<HealthActorMessage>,
}

impl HealthActorHandle {
    pub fn new() -> (Self, HealthActor) {
        let (sender, receiver) = mpsc::channel(100);
        (Self { sender }, HealthActor::new(receiver))
    }

    pub async fn check(&self, service_id: String) -> Result<HealthStatus> {
        let (tx, rx) = oneshot::channel();
        self.sender.send(HealthActorMessage::Check {
            service_id,
            respond_to: tx,
        }).await?;
        rx.await.map_err(|_| Error::Other("Actor stopped".into()))
    }
}
```

---

## C09: Parallel Processing with Rayon

**Priority**: P2
**Source**: SYNTHEX

```rust
use rayon::prelude::*;

/// CPU-bound parallel processing (not for I/O!)
pub fn compute_tensor_distances(
    tensors: &[Tensor12D],
    target: &Tensor12D,
) -> Vec<(usize, f64)> {
    tensors
        .par_iter()
        .enumerate()
        .map(|(i, t)| (i, t.distance(target)))
        .collect()
}

/// Find nearest neighbors in parallel
pub fn find_nearest_k(
    tensors: &[Tensor12D],
    target: &Tensor12D,
    k: usize,
) -> Vec<usize> {
    let mut distances: Vec<_> = compute_tensor_distances(tensors, target);
    distances.par_sort_unstable_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
    distances.into_iter().take(k).map(|(i, _)| i).collect()
}

/// Parallel aggregation
pub fn aggregate_health_scores(services: &[ServiceState]) -> f64 {
    let (sum, count) = services
        .par_iter()
        .map(|s| (s.health_score, 1usize))
        .reduce(|| (0.0, 0), |(a, b), (c, d)| (a + c, b + d));

    if count > 0 { sum / count as f64 } else { 0.0 }
}
```

**When to use Rayon vs Tokio**:
- Rayon: CPU-bound work (computation, sorting)
- Tokio: I/O-bound work (network, file system)

---

## C10: Atomic Operations

**Priority**: P1
**Source**: All codebases

```rust
use std::sync::atomic::{AtomicU64, AtomicBool, Ordering};

/// Lock-free counters and flags
pub struct Metrics {
    request_count: AtomicU64,
    error_count: AtomicU64,
    is_healthy: AtomicBool,
}

impl Metrics {
    pub fn new() -> Self {
        Self {
            request_count: AtomicU64::new(0),
            error_count: AtomicU64::new(0),
            is_healthy: AtomicBool::new(true),
        }
    }

    pub fn record_request(&self) {
        self.request_count.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_error(&self) {
        self.error_count.fetch_add(1, Ordering::Relaxed);
    }

    pub fn set_healthy(&self, healthy: bool) {
        self.is_healthy.store(healthy, Ordering::Release);
    }

    pub fn is_healthy(&self) -> bool {
        self.is_healthy.load(Ordering::Acquire)
    }

    pub fn error_rate(&self) -> f64 {
        let requests = self.request_count.load(Ordering::Relaxed);
        let errors = self.error_count.load(Ordering::Relaxed);
        if requests > 0 {
            errors as f64 / requests as f64
        } else {
            0.0
        }
    }
}
```

---

## C11: Spawn Task Patterns

**Priority**: P0
**Source**: All codebases

```rust
use tokio::task::JoinHandle;

/// Spawn fire-and-forget task with error logging
pub fn spawn_logged<F>(name: &'static str, future: F)
where
    F: std::future::Future<Output = Result<()>> + Send + 'static,
{
    tokio::spawn(async move {
        if let Err(e) = future.await {
            tracing::error!(task = name, error = %e, "Task failed");
        }
    });
}

/// Spawn with handle for cancellation
pub fn spawn_cancellable<F, T>(future: F) -> (JoinHandle<T>, tokio::sync::oneshot::Sender<()>)
where
    F: std::future::Future<Output = T> + Send + 'static,
    T: Send + 'static,
{
    let (cancel_tx, cancel_rx) = tokio::sync::oneshot::channel();

    let handle = tokio::spawn(async move {
        tokio::select! {
            result = future => result,
            _ = cancel_rx => panic!("Task cancelled"),
        }
    });

    (handle, cancel_tx)
}

/// Spawn with timeout
pub async fn spawn_with_timeout<F, T>(
    duration: Duration,
    future: F,
) -> Result<T>
where
    F: std::future::Future<Output = Result<T>> + Send + 'static,
    T: Send + 'static,
{
    let handle = tokio::spawn(future);

    tokio::time::timeout(duration, handle)
        .await
        .map_err(|_| Error::Timeout {
            endpoint: "spawned_task".into(),
            timeout_ms: duration.as_millis() as u64,
        })?
        .map_err(|e| Error::Other(format!("Task panicked: {}", e)))?
}
```

---

## C12: Select for Racing Futures

**Priority**: P0
**Source**: All codebases

```rust
use tokio::select;
use tokio::time::{sleep, Duration};

/// Race multiple futures
pub async fn health_check_with_timeout(
    service_id: &str,
    timeout: Duration,
) -> Result<HealthStatus> {
    select! {
        result = perform_health_check(service_id) => {
            result
        }
        _ = sleep(timeout) => {
            Err(Error::Timeout {
                endpoint: service_id.to_string(),
                timeout_ms: timeout.as_millis() as u64,
            })
        }
    }
}

/// Select with shutdown signal
pub async fn run_with_shutdown(
    mut shutdown_rx: tokio::sync::broadcast::Receiver<()>,
) -> Result<()> {
    loop {
        select! {
            biased; // Check shutdown first

            _ = shutdown_rx.recv() => {
                tracing::info!("Shutdown received");
                return Ok(());
            }

            result = process_work() => {
                if let Err(e) = result {
                    tracing::warn!(error = %e, "Work failed");
                }
            }
        }
    }
}

/// First successful result wins
pub async fn first_healthy_service(
    services: &[ServiceEndpoint],
) -> Result<ServiceEndpoint> {
    let futures: Vec<_> = services
        .iter()
        .map(|s| async move {
            if check_health(&s.id).await.is_ok() {
                Some(s.clone())
            } else {
                None
            }
        })
        .collect();

    // Return first Some result
    for future in futures {
        if let Some(service) = future.await {
            return Ok(service);
        }
    }

    Err(Error::Other("No healthy services".into()))
}
```

---

## Summary

| Pattern | Priority | Use Case |
|---------|----------|----------|
| DashMap | P0 | High-concurrency registry |
| Arc + RwLock | P1 | Complex shared state |
| Channels | P0 | Async communication |
| Async Mutex | P1 | Rare writes across await |
| Semaphore | P1 | Resource limiting |
| Notify | P2 | Event signaling |
| Watch | P1 | Config broadcasting |
| Actor | P2 | Isolated state machines |
| Rayon | P2 | CPU-bound parallelism |
| Atomics | P1 | Lock-free counters |
| Spawn | P0 | Task management |
| Select | P0 | Racing futures |

---

*Generated: 2026-01-28 | The Maintenance Engine v1.0.0*
