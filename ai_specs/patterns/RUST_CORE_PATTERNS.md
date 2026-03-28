# Rust Core Patterns

> Claude Code Optimized - 25 Essential Patterns

---

## P01: Result<T> Type Alias

**Priority**: P0 (Critical)
**Source**: All codebases

```rust
// Define at crate root
pub type Result<T> = std::result::Result<T, Error>;

// Use throughout crate
pub fn process(data: &[u8]) -> Result<Output> {
    let parsed = parse(data)?;
    let validated = validate(parsed)?;
    Ok(transform(validated))
}
```

**Why**: Eliminates `.unwrap()` temptation, enables `?` operator.

---

## P02: Builder Pattern

**Priority**: P0
**Source**: CodeSynthor V7

```rust
pub struct ServiceConfig {
    name: String,
    port: u16,
    timeout_ms: u64,
    retries: u32,
}

impl ServiceConfig {
    pub fn builder() -> ServiceConfigBuilder {
        ServiceConfigBuilder::default()
    }
}

#[derive(Default)]
pub struct ServiceConfigBuilder {
    name: Option<String>,
    port: Option<u16>,
    timeout_ms: u64,
    retries: u32,
}

impl ServiceConfigBuilder {
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn port(mut self, port: u16) -> Self {
        self.port = Some(port);
        self
    }

    pub fn timeout_ms(mut self, timeout: u64) -> Self {
        self.timeout_ms = timeout;
        self
    }

    pub fn retries(mut self, retries: u32) -> Self {
        self.retries = retries;
        self
    }

    pub fn build(self) -> Result<ServiceConfig> {
        Ok(ServiceConfig {
            name: self.name.ok_or(Error::Config("name required".into()))?,
            port: self.port.ok_or(Error::Config("port required".into()))?,
            timeout_ms: self.timeout_ms,
            retries: self.retries,
        })
    }
}
```

**Usage**:
```rust
let config = ServiceConfig::builder()
    .name("synthex")
    .port(8090)
    .timeout_ms(5000)
    .retries(3)
    .build()?;
```

---

## P03: Newtype Pattern

**Priority**: P0
**Source**: The Maintenance Engine

```rust
// Strong typing prevents mixing IDs
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ServiceId(String);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AgentId(String);

impl ServiceId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for ServiceId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl std::fmt::Display for ServiceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
```

**Why**: Compiler catches `agent_function(service_id)` mistakes.

---

## P04: Default Implementations

**Priority**: P1
**Source**: SYNTHEX

```rust
#[derive(Debug, Clone)]
pub struct HebbianConfig {
    pub ltp_rate: f64,
    pub ltd_rate: f64,
    pub stdp_window_ms: u64,
    pub decay_rate: f64,
    pub prune_threshold: f64,
}

impl Default for HebbianConfig {
    fn default() -> Self {
        Self {
            ltp_rate: 0.1,
            ltd_rate: 0.05,
            stdp_window_ms: 100,
            decay_rate: 0.001,
            prune_threshold: 0.1,
        }
    }
}

// Usage with partial override
let config = HebbianConfig {
    ltp_rate: 0.15,
    ..Default::default()
};
```

---

## P05: Enum State Machines

**Priority**: P0
**Source**: All codebases

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceStatus {
    Stopped,
    Starting,
    Running,
    Stopping,
    Failed,
}

impl ServiceStatus {
    /// Valid state transitions
    pub fn can_transition_to(&self, target: ServiceStatus) -> bool {
        matches!(
            (self, target),
            (Self::Stopped, Self::Starting)
                | (Self::Starting, Self::Running)
                | (Self::Starting, Self::Failed)
                | (Self::Running, Self::Stopping)
                | (Self::Running, Self::Failed)
                | (Self::Stopping, Self::Stopped)
                | (Self::Failed, Self::Starting)
        )
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Stopped | Self::Failed)
    }
}
```

---

## P06: From/Into Conversions

**Priority**: P1
**Source**: SYNTHEX

```rust
// External error conversion
impl From<rusqlite::Error> for Error {
    fn from(err: rusqlite::Error) -> Self {
        Self::Database(err.to_string())
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
    }
}

impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        Self::Serialization(err.to_string())
    }
}

// Enables: let result = operation()?; // Auto-converts
```

---

## P07: Const Generics

**Priority**: P2
**Source**: The Maintenance Engine

```rust
/// Fixed-size tensor with compile-time dimension checking
#[derive(Clone, Copy, Debug)]
pub struct Tensor<const N: usize> {
    data: [f64; N],
}

impl<const N: usize> Tensor<N> {
    pub const fn new(data: [f64; N]) -> Self {
        Self { data }
    }

    pub fn magnitude(&self) -> f64 {
        self.data.iter().map(|x| x * x).sum::<f64>().sqrt()
    }

    pub fn distance(&self, other: &Self) -> f64 {
        self.data
            .iter()
            .zip(other.data.iter())
            .map(|(a, b)| (a - b).powi(2))
            .sum::<f64>()
            .sqrt()
    }
}

// Type alias for 12D tensor
pub type Tensor12D = Tensor<12>;
```

---

## P08: Option Combinators

**Priority**: P1
**Source**: SYNTHEX

```rust
// Prefer combinators over match
fn get_service_port(id: &str) -> Option<u16> {
    services.get(id)
        .filter(|s| s.is_healthy())
        .map(|s| s.port)
}

// Chain operations
fn get_health_score(id: &str) -> f64 {
    services.get(id)
        .map(|s| s.health_score)
        .unwrap_or(0.0)  // OK for defaults, not for errors
}

// Use ok_or for Result conversion
fn require_service(id: &str) -> Result<&Service> {
    services.get(id)
        .ok_or_else(|| Error::ServiceNotFound(id.to_string()))
}
```

---

## P09: Iterator Chains

**Priority**: P1
**Source**: All codebases

```rust
// Efficient processing without intermediate collections
let healthy_services: Vec<&Service> = services
    .values()
    .filter(|s| s.status == ServiceStatus::Running)
    .filter(|s| s.health_score > 0.8)
    .collect();

// Aggregations
let total_cpu: f64 = services
    .values()
    .map(|s| s.cpu_percent)
    .sum();

// Find operations
let critical_service = services
    .values()
    .find(|s| s.needs_attention);

// Partitioning
let (healthy, unhealthy): (Vec<_>, Vec<_>) = services
    .values()
    .partition(|s| s.is_healthy);
```

---

## P10: AsRef/AsMut Bounds

**Priority**: P2
**Source**: CodeSynthor V7

```rust
// Accept String, &str, or anything that converts
pub fn log_event(service: impl AsRef<str>, message: impl AsRef<str>) {
    let service = service.as_ref();
    let message = message.as_ref();
    println!("[{}] {}", service, message);
}

// Callers can use:
log_event("synthex", "started");
log_event(String::from("synthex"), String::from("started"));
log_event(&service_id, &format!("health: {}", score));
```

---

## P11: Cow for Flexible Ownership

**Priority**: P2
**Source**: SYNTHEX

```rust
use std::borrow::Cow;

pub struct LogEntry<'a> {
    pub service: Cow<'a, str>,
    pub message: Cow<'a, str>,
    pub timestamp: i64,
}

impl<'a> LogEntry<'a> {
    // Accepts both borrowed and owned strings
    pub fn new(
        service: impl Into<Cow<'a, str>>,
        message: impl Into<Cow<'a, str>>,
    ) -> Self {
        Self {
            service: service.into(),
            message: message.into(),
            timestamp: chrono::Utc::now().timestamp(),
        }
    }

    // Convert to owned for storage
    pub fn into_owned(self) -> LogEntry<'static> {
        LogEntry {
            service: Cow::Owned(self.service.into_owned()),
            message: Cow::Owned(self.message.into_owned()),
            timestamp: self.timestamp,
        }
    }
}
```

---

## P12: Derive Macros

**Priority**: P0
**Source**: All codebases

```rust
// Standard derives for data types
#[derive(Debug, Clone, PartialEq)]
pub struct ServiceState {
    pub id: String,
    pub status: ServiceStatus,
    pub health_score: f64,
}

// Hash for map keys
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PathwayId {
    pub source: String,
    pub target: String,
}

// Serialization (with serde)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub name: String,
    pub port: u16,
    #[serde(default)]
    pub timeout_ms: u64,
}

// Default for optional fields
#[derive(Debug, Clone, Default)]
pub struct Metrics {
    pub cpu: f64,
    pub memory: f64,
    pub requests: u64,
}
```

---

## P13: Phantom Data

**Priority**: P2
**Source**: CodeSynthor V7

```rust
use std::marker::PhantomData;

// Type-state pattern for compile-time state tracking
pub struct Connection<State> {
    handle: u64,
    _state: PhantomData<State>,
}

pub struct Disconnected;
pub struct Connected;
pub struct Authenticated;

impl Connection<Disconnected> {
    pub fn new() -> Self {
        Self { handle: 0, _state: PhantomData }
    }

    pub fn connect(self, addr: &str) -> Result<Connection<Connected>> {
        // connect logic
        Ok(Connection { handle: 1, _state: PhantomData })
    }
}

impl Connection<Connected> {
    pub fn authenticate(self, token: &str) -> Result<Connection<Authenticated>> {
        // auth logic
        Ok(Connection { handle: self.handle, _state: PhantomData })
    }
}

impl Connection<Authenticated> {
    pub fn execute(&self, query: &str) -> Result<Vec<Row>> {
        // Only authenticated connections can execute
        todo!()
    }
}
```

---

## P14: Interior Mutability

**Priority**: P1
**Source**: SYNTHEX

```rust
use std::cell::RefCell;
use std::sync::RwLock;

// Single-threaded: RefCell
pub struct LocalCache {
    items: RefCell<HashMap<String, Value>>,
}

impl LocalCache {
    pub fn get(&self, key: &str) -> Option<Value> {
        self.items.borrow().get(key).cloned()
    }

    pub fn set(&self, key: String, value: Value) {
        self.items.borrow_mut().insert(key, value);
    }
}

// Multi-threaded: RwLock (readers don't block readers)
pub struct SharedCache {
    items: RwLock<HashMap<String, Value>>,
}

impl SharedCache {
    pub fn get(&self, key: &str) -> Option<Value> {
        self.items.read().ok()?.get(key).cloned()
    }

    pub fn set(&self, key: String, value: Value) {
        if let Ok(mut guard) = self.items.write() {
            guard.insert(key, value);
        }
    }
}
```

---

## P15: Extension Traits

**Priority**: P2
**Source**: The Maintenance Engine

```rust
// Extend existing types with new methods
pub trait StringExt {
    fn truncate_ellipsis(&self, max_len: usize) -> String;
    fn to_snake_case(&self) -> String;
}

impl StringExt for str {
    fn truncate_ellipsis(&self, max_len: usize) -> String {
        if self.len() <= max_len {
            self.to_string()
        } else {
            format!("{}...", &self[..max_len.saturating_sub(3)])
        }
    }

    fn to_snake_case(&self) -> String {
        let mut result = String::new();
        for (i, c) in self.chars().enumerate() {
            if c.is_uppercase() && i > 0 {
                result.push('_');
            }
            result.push(c.to_lowercase().next().unwrap_or(c));
        }
        result
    }
}

// Usage
let name = "ServiceStatus".to_snake_case(); // "service_status"
```

---

## P16: Smart Pointer Patterns

**Priority**: P1
**Source**: All codebases

```rust
use std::sync::Arc;

// Shared ownership across threads
pub struct Engine {
    config: Arc<Config>,
    registry: Arc<ServiceRegistry>,
    cache: Arc<TieredCache>,
}

impl Engine {
    pub fn new(config: Config) -> Self {
        let config = Arc::new(config);
        Self {
            config: Arc::clone(&config),
            registry: Arc::new(ServiceRegistry::new()),
            cache: Arc::new(TieredCache::new()),
        }
    }

    // Cheap clone for spawning tasks
    pub fn spawn_worker(&self) {
        let registry = Arc::clone(&self.registry);
        tokio::spawn(async move {
            // registry is accessible here
        });
    }
}
```

---

## P17: Conditional Compilation

**Priority**: P1
**Source**: SYNTHEX

```rust
// Feature flags in Cargo.toml:
// [features]
// default = []
// api = ["axum", "tower"]
// consensus = []

#[cfg(feature = "api")]
pub mod rest_server;

#[cfg(feature = "api")]
pub async fn start_api() -> Result<()> {
    rest_server::run().await
}

#[cfg(not(feature = "api"))]
pub async fn start_api() -> Result<()> {
    Err(Error::Config("API feature not enabled".into()))
}

// Platform-specific code
#[cfg(target_os = "linux")]
fn get_process_info() -> ProcessInfo { /* linux impl */ }

#[cfg(target_os = "macos")]
fn get_process_info() -> ProcessInfo { /* macos impl */ }
```

---

## P18: Documentation Patterns

**Priority**: P1
**Source**: All codebases

```rust
//! # Service Registry Module
//!
//! Provides service discovery and health tracking.
//!
//! ## Example
//!
//! ```rust
//! let registry = ServiceRegistry::new();
//! registry.register(service)?;
//! ```

/// A registered service with health metrics.
///
/// # Fields
///
/// * `id` - Unique service identifier
/// * `health_score` - Current health [0.0, 1.0]
///
/// # Example
///
/// ```rust
/// let service = Service::new("synthex", 8090);
/// assert_eq!(service.health_score, 1.0);
/// ```
#[derive(Debug, Clone)]
pub struct Service {
    /// Unique service identifier
    pub id: ServiceId,
    /// Current health score between 0.0 and 1.0
    pub health_score: f64,
}

impl Service {
    /// Creates a new service with default health.
    ///
    /// # Arguments
    ///
    /// * `id` - Service identifier
    /// * `port` - Port number (1-65535)
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if port is 0.
    pub fn new(id: impl Into<String>, port: u16) -> Result<Self> {
        // ...
    }
}
```

---

## P19: Test Organization

**Priority**: P0
**Source**: All codebases

```rust
// Unit tests in same file
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_service_creation() {
        let service = Service::new("test", 8080).unwrap();
        assert_eq!(service.health_score, 1.0);
    }

    #[test]
    fn test_invalid_port() {
        let result = Service::new("test", 0);
        assert!(result.is_err());
    }

    // Async tests
    #[tokio::test]
    async fn test_health_check() {
        let service = Service::new("test", 8080).unwrap();
        let health = service.check_health().await.unwrap();
        assert!(health.is_healthy);
    }
}

// Integration tests in tests/ directory
// tests/integration_test.rs
#[test]
fn test_full_workflow() {
    // ...
}
```

---

## P20: Workspace Organization

**Priority**: P1
**Source**: All codebases

```toml
# Cargo.toml (workspace root)
[workspace]
members = [
    "crates/core",
    "crates/api",
    "crates/learning",
    "crates/consensus",
]
resolver = "2"

[workspace.package]
version = "1.0.0"
edition = "2021"
license = "MIT"

[workspace.dependencies]
tokio = { version = "1.35", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
thiserror = "1.0"
```

```toml
# crates/core/Cargo.toml
[package]
name = "maintenance-core"
version.workspace = true
edition.workspace = true

[dependencies]
tokio.workspace = true
serde.workspace = true
```

---

## P21: Logging Patterns

**Priority**: P1
**Source**: SYNTHEX

```rust
use tracing::{info, warn, error, debug, instrument};

#[instrument(skip(self), fields(service_id = %id))]
pub async fn health_check(&self, id: &str) -> Result<HealthStatus> {
    debug!("Starting health check");

    let status = match self.ping(id).await {
        Ok(latency) => {
            info!(latency_ms = latency, "Health check passed");
            HealthStatus::Healthy
        }
        Err(e) => {
            warn!(error = %e, "Health check failed");
            HealthStatus::Unhealthy
        }
    };

    Ok(status)
}

// Structured logging
info!(
    service = %service_id,
    health = health_score,
    cpu = cpu_percent,
    "Service metrics updated"
);
```

---

## P22: Validation Patterns

**Priority**: P1
**Source**: The Maintenance Engine

```rust
impl Tensor12D {
    pub fn validate(&self) -> Result<()> {
        // Check all dimensions are in valid range
        let dimensions = [
            ("service_id", self.d0_service_id),
            ("port", self.d1_port),
            ("tier", self.d2_tier),
            // ... more dimensions
        ];

        for (name, value) in dimensions {
            if !(0.0..=1.0).contains(&value) {
                return Err(Error::TensorValidation {
                    dimension: name.to_string(),
                    value,
                    expected: "[0.0, 1.0]".to_string(),
                });
            }

            if value.is_nan() || value.is_infinite() {
                return Err(Error::TensorValidation {
                    dimension: name.to_string(),
                    value,
                    expected: "finite number".to_string(),
                });
            }
        }

        Ok(())
    }
}
```

---

## P23: Resource Cleanup (Drop)

**Priority**: P1
**Source**: CodeSynthor V7

```rust
pub struct DatabaseConnection {
    handle: u64,
    pool: Arc<Pool>,
}

impl Drop for DatabaseConnection {
    fn drop(&mut self) {
        // Return connection to pool
        if let Err(e) = self.pool.return_connection(self.handle) {
            eprintln!("Failed to return connection: {}", e);
        }
    }
}

// RAII pattern for locks
pub struct ScopedLock<'a> {
    lock_manager: &'a LockManager,
    resource_id: String,
}

impl<'a> ScopedLock<'a> {
    pub fn acquire(manager: &'a LockManager, id: &str) -> Result<Self> {
        manager.acquire(id)?;
        Ok(Self {
            lock_manager: manager,
            resource_id: id.to_string(),
        })
    }
}

impl Drop for ScopedLock<'_> {
    fn drop(&mut self) {
        let _ = self.lock_manager.release(&self.resource_id);
    }
}
```

---

## P24: Zero-Copy Parsing

**Priority**: P2
**Source**: SYNTHEX

```rust
use std::borrow::Cow;

/// Parse log line without allocation when possible
pub fn parse_log_line(line: &str) -> LogEntry<'_> {
    let parts: Vec<&str> = line.splitn(3, ' ').collect();

    LogEntry {
        timestamp: parts.get(0).map(|s| Cow::Borrowed(*s)).unwrap_or(Cow::Borrowed("")),
        level: parts.get(1).map(|s| Cow::Borrowed(*s)).unwrap_or(Cow::Borrowed("INFO")),
        message: parts.get(2).map(|s| Cow::Borrowed(*s)).unwrap_or(Cow::Borrowed("")),
    }
}

/// Only allocate when modification needed
pub fn sanitize_message(msg: &str) -> Cow<'_, str> {
    if msg.contains('\0') {
        Cow::Owned(msg.replace('\0', ""))
    } else {
        Cow::Borrowed(msg)
    }
}
```

---

## P25: Compile-Time Assertions

**Priority**: P2
**Source**: The Maintenance Engine

```rust
// Static assertions
const _: () = assert!(std::mem::size_of::<Tensor12D>() == 96);
const _: () = assert!(PBFT_Q > PBFT_F * 2); // Quorum > 2f

// Type-level assertions
trait Sealed {}
impl Sealed for ServiceId {}
impl Sealed for AgentId {}

// Ensures trait can't be implemented outside crate
pub trait Identifier: Sealed {
    fn as_str(&self) -> &str;
}
```

---

## Summary

| Pattern | Priority | Use Case |
|---------|----------|----------|
| Result<T> | P0 | All fallible operations |
| Builder | P0 | Complex object construction |
| Newtype | P0 | Type-safe identifiers |
| State Machine | P0 | Status transitions |
| From/Into | P1 | Error conversion |
| Iterators | P1 | Collection processing |
| Derive | P0 | Automatic trait impl |
| Arc/RwLock | P1 | Shared state |
| Feature Flags | P1 | Optional modules |
| Tests | P0 | Quality assurance |

---

*Generated: 2026-01-28 | The Maintenance Engine v1.0.0*
