# Error Handling Patterns

> Claude Code Optimized - 15 Error Patterns

---

## E01: Unified Error Enum

**Priority**: P0 (Critical)
**Source**: All codebases

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    // Configuration errors (1000-1099)
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Config file not found: {path}")]
    ConfigNotFound { path: String },

    // Database errors (1100-1199)
    #[error("Database error: {0}")]
    Database(String),

    #[error("Query failed: {query} - {reason}")]
    QueryFailed { query: String, reason: String },

    // Network errors (1200-1299)
    #[error("Network error connecting to {target}: {message}")]
    Network { target: String, message: String },

    #[error("Request timeout after {timeout_ms}ms to {endpoint}")]
    Timeout { endpoint: String, timeout_ms: u64 },

    // Service errors (1300-1399)
    #[error("Service not found: {0}")]
    ServiceNotFound(String),

    #[error("Health check failed for {service_id}: {reason}")]
    HealthCheckFailed { service_id: String, reason: String },

    // Consensus errors (1400-1499)
    #[error("Quorum not reached: required {required}, received {received}")]
    QuorumNotReached { required: u32, received: u32 },

    #[error("Byzantine fault detected from agent {agent_id}")]
    ByzantineFault { agent_id: String },

    // Learning errors (1500-1599)
    #[error("Pathway not found: {source} -> {target}")]
    PathwayNotFound { source: String, target: String },

    #[error("STDP calculation error: {reason}")]
    StdpError { reason: String },

    // Tensor errors (1600-1699)
    #[error("Tensor dimension {dimension} out of range: {value} (expected {expected})")]
    TensorValidation { dimension: String, value: f64, expected: String },

    // Escalation errors (1700-1799)
    #[error("Escalation required from {from_tier} to {to_tier}: {reason}")]
    EscalationRequired { from_tier: String, to_tier: String, reason: String },

    // Standard library wrappers
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(String),

    // Catch-all
    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, Error>;
```

---

## E02: Error Context Builder

**Priority**: P1
**Source**: SYNTHEX

```rust
#[derive(Debug, Clone)]
pub struct ErrorContext {
    pub module: String,
    pub operation: String,
    pub metadata: Vec<(String, String)>,
}

impl ErrorContext {
    pub fn new(module: impl Into<String>) -> Self {
        Self {
            module: module.into(),
            operation: String::new(),
            metadata: Vec::new(),
        }
    }

    pub fn operation(mut self, op: impl Into<String>) -> Self {
        self.operation = op.into();
        self
    }

    pub fn with(mut self, key: impl Into<String>, value: impl ToString) -> Self {
        self.metadata.push((key.into(), value.to_string()));
        self
    }
}

impl Error {
    pub fn with_context(self, ctx: ErrorContext) -> Self {
        // Wrap error with context
        Self::Other(format!(
            "[{}::{}] {} ({})",
            ctx.module,
            ctx.operation,
            self,
            ctx.metadata
                .iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect::<Vec<_>>()
                .join(", ")
        ))
    }
}

// Usage
fn process_service(id: &str) -> Result<()> {
    operation()
        .map_err(|e| e.with_context(
            ErrorContext::new("M7_HealthMonitor")
                .operation("check_health")
                .with("service_id", id)
                .with("attempt", 3)
        ))
}
```

---

## E03: From Trait Implementations

**Priority**: P0
**Source**: All codebases

```rust
// Standard library errors
impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
    }
}

// Database errors
impl From<rusqlite::Error> for Error {
    fn from(err: rusqlite::Error) -> Self {
        Self::Database(err.to_string())
    }
}

// JSON errors
impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        Self::Serialization(err.to_string())
    }
}

// Timeout errors (tokio)
impl From<tokio::time::error::Elapsed> for Error {
    fn from(_: tokio::time::error::Elapsed) -> Self {
        Self::Timeout {
            endpoint: "unknown".to_string(),
            timeout_ms: 0,
        }
    }
}

// Channel errors
impl<T> From<tokio::sync::mpsc::error::SendError<T>> for Error {
    fn from(err: tokio::sync::mpsc::error::SendError<T>) -> Self {
        Self::Other(format!("Channel send failed: {}", err))
    }
}
```

---

## E04: Error Code Ranges

**Priority**: P1
**Source**: CodeSynthor V7

```rust
impl Error {
    /// Returns numeric error code for programmatic handling
    pub fn code(&self) -> u32 {
        match self {
            // Configuration: 1000-1099
            Self::Config(_) => 1000,
            Self::ConfigNotFound { .. } => 1001,

            // Database: 1100-1199
            Self::Database(_) => 1100,
            Self::QueryFailed { .. } => 1101,

            // Network: 1200-1299
            Self::Network { .. } => 1200,
            Self::Timeout { .. } => 1201,

            // Service: 1300-1399
            Self::ServiceNotFound(_) => 1300,
            Self::HealthCheckFailed { .. } => 1301,

            // Consensus: 1400-1499
            Self::QuorumNotReached { .. } => 1400,
            Self::ByzantineFault { .. } => 1401,

            // Learning: 1500-1599
            Self::PathwayNotFound { .. } => 1500,
            Self::StdpError { .. } => 1501,

            // Tensor: 1600-1699
            Self::TensorValidation { .. } => 1600,

            // Escalation: 1700-1799
            Self::EscalationRequired { .. } => 1700,

            // I/O: 1800-1899
            Self::Io(_) => 1800,
            Self::Serialization(_) => 1801,

            // Other: 9000+
            Self::Other(_) => 9000,
        }
    }

    /// Returns error category for routing
    pub fn category(&self) -> &'static str {
        match self.code() / 100 {
            10 => "config",
            11 => "database",
            12 => "network",
            13 => "service",
            14 => "consensus",
            15 => "learning",
            16 => "tensor",
            17 => "escalation",
            18 => "io",
            _ => "other",
        }
    }
}
```

---

## E05: Recoverable vs Fatal Errors

**Priority**: P0
**Source**: The Maintenance Engine

```rust
impl Error {
    /// Whether the error can be retried
    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            Self::Network { .. }
                | Self::Timeout { .. }
                | Self::HealthCheckFailed { .. }
                | Self::QueryFailed { .. }
        )
    }

    /// Whether the error should trigger circuit breaker
    pub fn is_circuit_breaker_trigger(&self) -> bool {
        matches!(
            self,
            Self::Network { .. }
                | Self::Timeout { .. }
                | Self::HealthCheckFailed { .. }
        )
    }

    /// Suggested retry delay in milliseconds
    pub fn retry_delay_ms(&self) -> Option<u64> {
        match self {
            Self::Network { .. } => Some(1000),
            Self::Timeout { .. } => Some(2000),
            Self::HealthCheckFailed { .. } => Some(5000),
            Self::QueryFailed { .. } => Some(100),
            _ => None,
        }
    }
}
```

---

## E06: Result Extension Traits

**Priority**: P1
**Source**: SYNTHEX

```rust
pub trait ResultExt<T> {
    /// Log error and convert to Option
    fn log_err(self, context: &str) -> Option<T>;

    /// Convert error with additional context
    fn context(self, msg: impl Into<String>) -> Result<T>;

    /// Retry with exponential backoff
    async fn retry(self, max_attempts: u32) -> Result<T>
    where
        Self: Sized;
}

impl<T> ResultExt<T> for Result<T> {
    fn log_err(self, context: &str) -> Option<T> {
        match self {
            Ok(v) => Some(v),
            Err(e) => {
                tracing::warn!(error = %e, context = context, "Operation failed");
                None
            }
        }
    }

    fn context(self, msg: impl Into<String>) -> Result<T> {
        self.map_err(|e| Error::Other(format!("{}: {}", msg.into(), e)))
    }

    async fn retry(self, max_attempts: u32) -> Result<T>
    where
        Self: Sized,
    {
        // Implementation in E07
        todo!()
    }
}
```

---

## E07: Retry with Backoff

**Priority**: P0
**Source**: All codebases

```rust
use std::time::Duration;
use tokio::time::sleep;

pub struct RetryConfig {
    pub max_attempts: u32,
    pub initial_delay_ms: u64,
    pub max_delay_ms: u64,
    pub backoff_factor: f64,
    pub jitter: bool,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_delay_ms: 100,
            max_delay_ms: 10000,
            backoff_factor: 2.0,
            jitter: true,
        }
    }
}

pub async fn retry_with_backoff<T, F, Fut>(
    config: &RetryConfig,
    mut operation: F,
) -> Result<T>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T>>,
{
    let mut delay_ms = config.initial_delay_ms;
    let mut last_error = None;

    for attempt in 1..=config.max_attempts {
        match operation().await {
            Ok(value) => return Ok(value),
            Err(e) if e.is_recoverable() && attempt < config.max_attempts => {
                tracing::warn!(
                    attempt = attempt,
                    max = config.max_attempts,
                    delay_ms = delay_ms,
                    error = %e,
                    "Retrying after error"
                );

                let jitter = if config.jitter {
                    (rand::random::<f64>() - 0.5) * 0.2 * delay_ms as f64
                } else {
                    0.0
                };

                sleep(Duration::from_millis((delay_ms as f64 + jitter) as u64)).await;

                delay_ms = (delay_ms as f64 * config.backoff_factor) as u64;
                delay_ms = delay_ms.min(config.max_delay_ms);
                last_error = Some(e);
            }
            Err(e) => return Err(e),
        }
    }

    Err(last_error.unwrap_or_else(|| Error::Other("Retry exhausted".into())))
}
```

---

## E08: Circuit Breaker Error Handling

**Priority**: P0
**Source**: The Maintenance Engine

```rust
#[derive(Debug, Clone)]
pub struct CircuitBreaker {
    state: CircuitState,
    failure_count: u32,
    success_count: u32,
    last_failure: Option<std::time::Instant>,
    config: CircuitConfig,
}

#[derive(Debug, Clone, Copy)]
pub enum CircuitState {
    Closed,
    Open,
    HalfOpen,
}

#[derive(Debug, Clone)]
pub struct CircuitConfig {
    pub failure_threshold: u32,
    pub success_threshold: u32,
    pub timeout_ms: u64,
}

impl CircuitBreaker {
    pub fn record_success(&mut self) {
        self.failure_count = 0;
        match self.state {
            CircuitState::HalfOpen => {
                self.success_count += 1;
                if self.success_count >= self.config.success_threshold {
                    self.state = CircuitState::Closed;
                    self.success_count = 0;
                }
            }
            _ => {}
        }
    }

    pub fn record_failure(&mut self, error: &Error) {
        if !error.is_circuit_breaker_trigger() {
            return;
        }

        self.failure_count += 1;
        self.last_failure = Some(std::time::Instant::now());

        match self.state {
            CircuitState::Closed if self.failure_count >= self.config.failure_threshold => {
                self.state = CircuitState::Open;
                tracing::warn!("Circuit breaker opened after {} failures", self.failure_count);
            }
            CircuitState::HalfOpen => {
                self.state = CircuitState::Open;
                self.success_count = 0;
            }
            _ => {}
        }
    }

    pub fn allow_request(&mut self) -> bool {
        match self.state {
            CircuitState::Closed => true,
            CircuitState::Open => {
                if let Some(last) = self.last_failure {
                    if last.elapsed().as_millis() as u64 > self.config.timeout_ms {
                        self.state = CircuitState::HalfOpen;
                        true
                    } else {
                        false
                    }
                } else {
                    true
                }
            }
            CircuitState::HalfOpen => true,
        }
    }
}
```

---

## E09: Error Aggregation

**Priority**: P1
**Source**: SYNTHEX

```rust
/// Collects multiple errors from parallel operations
#[derive(Debug)]
pub struct ErrorCollection {
    errors: Vec<Error>,
}

impl ErrorCollection {
    pub fn new() -> Self {
        Self { errors: Vec::new() }
    }

    pub fn push(&mut self, error: Error) {
        self.errors.push(error);
    }

    pub fn is_empty(&self) -> bool {
        self.errors.is_empty()
    }

    pub fn len(&self) -> usize {
        self.errors.len()
    }

    pub fn into_result<T>(self, value: T) -> Result<T> {
        if self.is_empty() {
            Ok(value)
        } else {
            Err(self.into())
        }
    }
}

impl From<ErrorCollection> for Error {
    fn from(collection: ErrorCollection) -> Self {
        if collection.len() == 1 {
            collection.errors.into_iter().next().unwrap()
        } else {
            Self::Other(format!(
                "Multiple errors ({}): {}",
                collection.len(),
                collection
                    .errors
                    .iter()
                    .map(|e| e.to_string())
                    .collect::<Vec<_>>()
                    .join("; ")
            ))
        }
    }
}

// Usage
async fn check_all_services(ids: &[&str]) -> Result<Vec<HealthStatus>> {
    let mut results = Vec::new();
    let mut errors = ErrorCollection::new();

    for id in ids {
        match check_service(id).await {
            Ok(status) => results.push(status),
            Err(e) => errors.push(e),
        }
    }

    errors.into_result(results)
}
```

---

## E10: Panic Prevention

**Priority**: P0
**Source**: All codebases

```rust
// lib.rs - crate-wide panic prevention
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]

// Safe alternatives to unwrap
fn get_config_value(key: &str) -> Result<String> {
    // BAD: config.get(key).unwrap()
    // GOOD:
    config.get(key)
        .ok_or_else(|| Error::Config(format!("Missing key: {}", key)))
}

// Safe indexing
fn get_element(vec: &[u8], idx: usize) -> Result<u8> {
    // BAD: vec[idx]
    // GOOD:
    vec.get(idx)
        .copied()
        .ok_or_else(|| Error::Other(format!("Index {} out of bounds", idx)))
}

// Safe parsing
fn parse_port(s: &str) -> Result<u16> {
    // BAD: s.parse().unwrap()
    // GOOD:
    s.parse()
        .map_err(|_| Error::Validation(format!("Invalid port: {}", s)))
}

// Safe channel operations
async fn send_event(tx: &Sender<Event>, event: Event) -> Result<()> {
    // BAD: tx.send(event).unwrap()
    // GOOD:
    tx.send(event)
        .await
        .map_err(|e| Error::Other(format!("Channel send failed: {}", e)))
}
```

---

## E11: Error Logging

**Priority**: P1
**Source**: SYNTHEX

```rust
use tracing::{error, warn, info};

/// Log error with appropriate level based on severity
pub fn log_error(err: &Error) {
    let code = err.code();
    let category = err.category();

    match err {
        // Critical errors - immediate attention needed
        Error::ByzantineFault { .. }
        | Error::EscalationRequired { .. } => {
            error!(
                code = code,
                category = category,
                error = %err,
                "Critical error requiring immediate attention"
            );
        }

        // Warning errors - recoverable
        Error::Network { .. }
        | Error::Timeout { .. }
        | Error::HealthCheckFailed { .. } => {
            warn!(
                code = code,
                category = category,
                error = %err,
                "Recoverable error"
            );
        }

        // Info errors - expected failures
        Error::ServiceNotFound(_)
        | Error::PathwayNotFound { .. } => {
            info!(
                code = code,
                category = category,
                error = %err,
                "Expected error condition"
            );
        }

        // All others
        _ => {
            warn!(
                code = code,
                category = category,
                error = %err,
                "Unclassified error"
            );
        }
    }
}
```

---

## E12: Error Response Formatting

**Priority**: P1
**Source**: CodeSynthor V7

```rust
use serde::Serialize;

#[derive(Serialize)]
pub struct ErrorResponse {
    pub success: bool,
    pub error: ErrorDetail,
}

#[derive(Serialize)]
pub struct ErrorDetail {
    pub code: u32,
    pub category: String,
    pub message: String,
    pub recoverable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_after_ms: Option<u64>,
}

impl From<&Error> for ErrorResponse {
    fn from(err: &Error) -> Self {
        Self {
            success: false,
            error: ErrorDetail {
                code: err.code(),
                category: err.category().to_string(),
                message: err.to_string(),
                recoverable: err.is_recoverable(),
                retry_after_ms: err.retry_delay_ms(),
            },
        }
    }
}

// API handler usage
async fn handle_request(req: Request) -> impl Response {
    match process(req).await {
        Ok(result) => Json(SuccessResponse { success: true, data: result }),
        Err(e) => {
            log_error(&e);
            let status = match e.code() / 100 {
                12 => StatusCode::BAD_GATEWAY,
                13 => StatusCode::NOT_FOUND,
                14 => StatusCode::SERVICE_UNAVAILABLE,
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            };
            (status, Json(ErrorResponse::from(&e)))
        }
    }
}
```

---

## E13: Validation Error Builder

**Priority**: P1
**Source**: The Maintenance Engine

```rust
#[derive(Debug)]
pub struct ValidationErrors {
    field_errors: Vec<FieldError>,
}

#[derive(Debug)]
pub struct FieldError {
    pub field: String,
    pub message: String,
    pub value: Option<String>,
}

impl ValidationErrors {
    pub fn new() -> Self {
        Self { field_errors: Vec::new() }
    }

    pub fn add(&mut self, field: impl Into<String>, message: impl Into<String>) {
        self.field_errors.push(FieldError {
            field: field.into(),
            message: message.into(),
            value: None,
        });
    }

    pub fn add_with_value(
        &mut self,
        field: impl Into<String>,
        message: impl Into<String>,
        value: impl ToString,
    ) {
        self.field_errors.push(FieldError {
            field: field.into(),
            message: message.into(),
            value: Some(value.to_string()),
        });
    }

    pub fn is_empty(&self) -> bool {
        self.field_errors.is_empty()
    }

    pub fn into_result<T>(self, value: T) -> Result<T> {
        if self.is_empty() {
            Ok(value)
        } else {
            Err(Error::Validation(
                self.field_errors
                    .iter()
                    .map(|e| format!("{}: {}", e.field, e.message))
                    .collect::<Vec<_>>()
                    .join(", ")
            ))
        }
    }
}

// Usage
fn validate_config(config: &Config) -> Result<()> {
    let mut errors = ValidationErrors::new();

    if config.port == 0 {
        errors.add_with_value("port", "must be > 0", config.port);
    }

    if config.name.is_empty() {
        errors.add("name", "cannot be empty");
    }

    if config.timeout_ms > 60000 {
        errors.add_with_value("timeout_ms", "max 60000", config.timeout_ms);
    }

    errors.into_result(())
}
```

---

## E14: Async Error Handling

**Priority**: P0
**Source**: SYNTHEX

```rust
use futures::future::join_all;

/// Execute multiple async operations, collecting all errors
pub async fn execute_all<T, F, Fut>(
    operations: impl IntoIterator<Item = F>,
) -> Result<Vec<T>>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<T>>,
{
    let futures: Vec<_> = operations.into_iter().map(|f| f()).collect();
    let results = join_all(futures).await;

    let mut successes = Vec::new();
    let mut errors = ErrorCollection::new();

    for result in results {
        match result {
            Ok(v) => successes.push(v),
            Err(e) => errors.push(e),
        }
    }

    errors.into_result(successes)
}

/// Execute with timeout
pub async fn with_timeout<T>(
    duration: Duration,
    future: impl std::future::Future<Output = Result<T>>,
) -> Result<T> {
    tokio::time::timeout(duration, future)
        .await
        .map_err(|_| Error::Timeout {
            endpoint: "operation".to_string(),
            timeout_ms: duration.as_millis() as u64,
        })?
}
```

---

## E15: Error Testing Patterns

**Priority**: P0
**Source**: All codebases

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_is_recoverable() {
        let network_err = Error::Network {
            target: "localhost".into(),
            message: "connection refused".into(),
        };
        assert!(network_err.is_recoverable());

        let config_err = Error::Config("bad config".into());
        assert!(!config_err.is_recoverable());
    }

    #[test]
    fn test_error_codes() {
        let err = Error::Database("test".into());
        assert_eq!(err.code(), 1100);
        assert_eq!(err.category(), "database");
    }

    #[test]
    fn test_error_conversion() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let err: Error = io_err.into();
        assert!(matches!(err, Error::Io(_)));
    }

    #[tokio::test]
    async fn test_retry_with_backoff() {
        let mut attempts = 0;
        let result = retry_with_backoff(
            &RetryConfig { max_attempts: 3, ..Default::default() },
            || {
                attempts += 1;
                async move {
                    if attempts < 3 {
                        Err(Error::Network {
                            target: "test".into(),
                            message: "fail".into(),
                        })
                    } else {
                        Ok("success")
                    }
                }
            },
        ).await;

        assert!(result.is_ok());
        assert_eq!(attempts, 3);
    }

    #[test]
    fn test_validation_errors() {
        let mut errors = ValidationErrors::new();
        errors.add("field1", "error1");
        errors.add("field2", "error2");

        let result: Result<()> = errors.into_result(());
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("field1"));
    }
}
```

---

## Summary

| Pattern | Priority | Use Case |
|---------|----------|----------|
| Unified Error Enum | P0 | All modules |
| Error Context | P1 | Debugging |
| From Traits | P0 | Error conversion |
| Error Codes | P1 | Programmatic handling |
| Recoverable Check | P0 | Retry logic |
| Result Extensions | P1 | Fluent error handling |
| Retry Backoff | P0 | Network resilience |
| Circuit Breaker | P0 | Failure isolation |
| Error Aggregation | P1 | Parallel operations |
| Panic Prevention | P0 | Stability |

---

*Generated: 2026-01-28 | The Maintenance Engine v1.0.0*
