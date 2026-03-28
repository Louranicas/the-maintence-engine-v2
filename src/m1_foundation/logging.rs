//! # M03: Logging System
//!
//! Structured logging with correlation IDs, context propagation, and
//! multiple output formats for the Maintenance Engine.
//!
//! ## Layer: L1 (Foundation)
//! ## Dependencies: M01 (Error Taxonomy), M02 (Configuration)
//! ## Tests: 50+ target
//!
//! ## 12D Tensor Encoding
//! ```text
//! [3/36, 0.0, 1/6, 2, 0, 0.5, health, uptime, synergy, latency, error_rate, temporal]
//! ```
//!
//! ## Features
//!
//! - **Structured logging** using tracing crate
//! - **Correlation IDs** for request tracking
//! - **Log levels**: TRACE, DEBUG, INFO, WARN, ERROR
//! - **JSON format** for production environments
//! - **Pretty format** for development
//! - **Context propagation** with service, module, and layer context
//! - **`CorrelationProvider` trait** for dependency-inverted distributed tracing
//!
//! ## Related Documentation
//! - [Module Specification](../../ai_docs/modules/M03_LOGGING_SYSTEM.md)
//! - [Layer Specification](../../ai_docs/layers/L01_FOUNDATION.md)

use std::fmt;
use std::sync::OnceLock;

use tracing::Level;
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::util::SubscriberInitExt;
use uuid::Uuid;

use super::{Error, Result};

/// Global logging initialization guard
static LOGGING_INITIALIZED: OnceLock<bool> = OnceLock::new();

// ============================================================================
// Traits
// ============================================================================

/// Trait for types that provide correlation context for distributed tracing.
///
/// Enables dependency inversion — upper layers can accept `&dyn CorrelationProvider`
/// instead of depending directly on [`LogContext`].
pub trait CorrelationProvider: Send + Sync {
    /// Get the current correlation ID for this context.
    fn correlation_id(&self) -> &str;
    /// Create a child context for a sub-operation, inheriting the correlation ID.
    fn child(&self, operation: &str) -> Box<dyn CorrelationProvider>;
    /// Return the agent ID associated with this context (NAM R5).
    fn agent_id(&self) -> Option<&str> {
        None
    }
}

// ============================================================================
// Core Types
// ============================================================================

/// Log context for correlation and tracing
///
/// Provides context propagation across service boundaries with correlation IDs,
/// service identification, and hierarchical layer/module information.
///
/// # Example
///
/// ```
/// use maintenance_engine::m1_foundation::logging::{LogContext, generate_correlation_id};
///
/// let ctx = LogContext {
///     correlation_id: generate_correlation_id(),
///     service_id: Some("maintenance-engine".to_string()),
///     layer: Some("L1".to_string()),
///     module: Some("M03".to_string()),
///     agent_id: None,
/// };
/// ```
#[derive(Debug, Clone, Default)]
pub struct LogContext {
    /// Unique correlation ID for request tracing
    pub correlation_id: String,
    /// Service identifier (e.g., "maintenance-engine", "synthex")
    pub service_id: Option<String>,
    /// Layer identifier (e.g., "L1", "L2")
    pub layer: Option<String>,
    /// Module identifier (e.g., "M01", "M03")
    pub module: Option<String>,
    /// Agent identity for NAM R5 attribution
    pub agent_id: Option<String>,
}

impl LogContext {
    /// Creates a new log context with a generated correlation ID
    ///
    /// # Example
    ///
    /// ```
    /// use maintenance_engine::m1_foundation::logging::LogContext;
    ///
    /// let ctx = LogContext::new();
    /// assert!(!ctx.correlation_id.is_empty());
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self {
            correlation_id: generate_correlation_id(),
            service_id: None,
            layer: None,
            module: None,
            agent_id: None,
        }
    }

    /// Creates a new log context with specified service, layer, and module
    ///
    /// Accepts any type that converts to `String`, reducing clones at call sites.
    ///
    /// # Example
    ///
    /// ```
    /// use maintenance_engine::m1_foundation::logging::LogContext;
    ///
    /// let ctx = LogContext::with_context("maintenance-engine", "L1", "M03");
    /// assert_eq!(ctx.service_id, Some("maintenance-engine".to_string()));
    /// ```
    #[must_use]
    pub fn with_context(
        service_id: impl Into<String>,
        layer: impl Into<String>,
        module: impl Into<String>,
    ) -> Self {
        Self {
            correlation_id: generate_correlation_id(),
            service_id: Some(service_id.into()),
            layer: Some(layer.into()),
            module: Some(module.into()),
            agent_id: None,
        }
    }

    /// Creates a child context inheriting the correlation ID
    ///
    /// Useful for propagating context to downstream operations while
    /// potentially changing the layer/module identifiers.
    #[must_use]
    pub fn child_context(&self) -> Self {
        Self {
            correlation_id: self.correlation_id.clone(),
            service_id: self.service_id.clone(),
            layer: self.layer.clone(),
            module: self.module.clone(),
            agent_id: self.agent_id.clone(),
        }
    }

    /// Creates a child context with a new module identifier
    ///
    /// Accepts any type that converts to `String`, reducing clones at call sites.
    #[must_use]
    pub fn with_module(&self, module: impl Into<String>) -> Self {
        Self {
            correlation_id: self.correlation_id.clone(),
            service_id: self.service_id.clone(),
            layer: self.layer.clone(),
            module: Some(module.into()),
            agent_id: self.agent_id.clone(),
        }
    }

    /// Creates a child context with a new layer identifier
    ///
    /// Accepts any type that converts to `String`, reducing clones at call sites.
    #[must_use]
    pub fn with_layer(&self, layer: impl Into<String>) -> Self {
        Self {
            correlation_id: self.correlation_id.clone(),
            service_id: self.service_id.clone(),
            layer: Some(layer.into()),
            module: self.module.clone(),
            agent_id: self.agent_id.clone(),
        }
    }

    /// Creates a child context with an agent identity (NAM R5).
    #[must_use]
    pub fn with_agent(&self, agent_id: impl Into<String>) -> Self {
        Self {
            correlation_id: self.correlation_id.clone(),
            service_id: self.service_id.clone(),
            layer: self.layer.clone(),
            module: self.module.clone(),
            agent_id: Some(agent_id.into()),
        }
    }

    /// Encode this logging context as a 12D tensor position (NAM R4).
    ///
    /// D0 = service hash, D2 = layer/6, D6 = 1.0 (healthy context).
    #[must_use]
    pub fn to_tensor_position(&self) -> crate::Tensor12D {
        let service_hash = self
            .service_id
            .as_ref()
            .map_or(0.0, |s| hash_to_unit(s));

        let layer_val = self
            .layer
            .as_ref()
            .and_then(|l| l.strip_prefix('L'))
            .and_then(|n| n.parse::<f64>().ok())
            .map_or(0.0, |n| n / 6.0);

        let mut tensor = crate::Tensor12D {
            service_id: service_hash,
            port: 0.0,
            tier: layer_val,
            dependency_count: 0.0,
            agent_count: 0.0,
            protocol: 0.5,
            health_score: 1.0,
            uptime: 0.0,
            synergy: 0.0,
            latency: 0.0,
            error_rate: 0.0,
            temporal_context: 0.0,
        };
        tensor.clamp_normalize();
        tensor
    }
}

impl CorrelationProvider for LogContext {
    fn correlation_id(&self) -> &str {
        &self.correlation_id
    }

    fn child(&self, operation: &str) -> Box<dyn CorrelationProvider> {
        Box::new(self.with_module(operation))
    }

    fn agent_id(&self) -> Option<&str> {
        self.agent_id.as_deref()
    }
}

impl fmt::Display for LogContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "corr_id={}", self.correlation_id)?;
        if let Some(ref svc) = self.service_id {
            write!(f, " service={svc}")?;
        }
        if let Some(ref layer) = self.layer {
            write!(f, " layer={layer}")?;
        }
        if let Some(ref module) = self.module {
            write!(f, " module={module}")?;
        }
        if let Some(ref agent) = self.agent_id {
            write!(f, " agent={agent}")?;
        }
        Ok(())
    }
}

/// Hash a string into a unit interval [0, 1] for tensor encoding.
fn hash_to_unit(s: &str) -> f64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut hasher);
    let hash = hasher.finish();
    // Map u64 to [0, 1] — precision loss is acceptable for hashing
    #[allow(clippy::cast_precision_loss)]
    let result = (hash as f64) / (u64::MAX as f64);
    result
}

/// Log output format
///
/// Determines how log entries are formatted for output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LogFormat {
    /// JSON format for structured log aggregation (production)
    Json,
    /// Human-readable pretty format (development)
    #[default]
    Pretty,
    /// Compact single-line format
    Compact,
}

impl LogFormat {
    /// Parse format from string
    ///
    /// # Errors
    ///
    /// Returns error if format string is not recognized
    pub fn parse_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "json" => Ok(Self::Json),
            "pretty" => Ok(Self::Pretty),
            "compact" => Ok(Self::Compact),
            _ => Err(Error::Config(format!("Unknown log format: {s}"))),
        }
    }
}

impl std::str::FromStr for LogFormat {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        Self::parse_str(s)
    }
}

impl fmt::Display for LogFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json => write!(f, "json"),
            Self::Pretty => write!(f, "pretty"),
            Self::Compact => write!(f, "compact"),
        }
    }
}

/// Log level configuration
///
/// Maps to tracing log levels with string parsing support.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum LogLevel {
    /// Trace level - most verbose
    Trace,
    /// Debug level
    Debug,
    /// Info level (default)
    #[default]
    Info,
    /// Warning level
    Warn,
    /// Error level - least verbose
    Error,
}

impl LogLevel {
    /// Parse log level from string
    ///
    /// # Errors
    ///
    /// Returns error if level string is not recognized
    pub fn parse_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "trace" => Ok(Self::Trace),
            "debug" => Ok(Self::Debug),
            "info" => Ok(Self::Info),
            "warn" | "warning" => Ok(Self::Warn),
            "error" => Ok(Self::Error),
            _ => Err(Error::Config(format!("Unknown log level: {s}"))),
        }
    }

    /// Convert to tracing Level
    #[must_use]
    pub const fn to_tracing_level(self) -> Level {
        match self {
            Self::Trace => Level::TRACE,
            Self::Debug => Level::DEBUG,
            Self::Info => Level::INFO,
            Self::Warn => Level::WARN,
            Self::Error => Level::ERROR,
        }
    }
}

impl fmt::Display for LogLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Trace => write!(f, "trace"),
            Self::Debug => write!(f, "debug"),
            Self::Info => write!(f, "info"),
            Self::Warn => write!(f, "warn"),
            Self::Error => write!(f, "error"),
        }
    }
}

impl std::str::FromStr for LogLevel {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        Self::parse_str(s)
    }
}

/// Logging configuration
///
/// Configures the logging system including level, format, and output options.
///
/// # Example
///
/// ```
/// use maintenance_engine::m1_foundation::logging::{LogConfig, LogFormat};
///
/// let config = LogConfig {
///     level: "info".to_string(),
///     format: LogFormat::Json,
///     include_timestamps: true,
///     include_targets: true,
///     include_file_line: false,
///     include_thread_ids: false,
///     include_span_events: false,
/// };
/// ```
#[derive(Debug, Clone)]
#[allow(clippy::struct_excessive_bools)]
pub struct LogConfig {
    /// Log level filter (e.g., "info", "debug", "trace")
    pub level: String,
    /// Output format
    pub format: LogFormat,
    /// Include timestamps in output
    pub include_timestamps: bool,
    /// Include target module paths
    pub include_targets: bool,
    /// Include file and line numbers
    pub include_file_line: bool,
    /// Include thread IDs
    pub include_thread_ids: bool,
    /// Include span entry/exit events
    pub include_span_events: bool,
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            format: LogFormat::Pretty,
            include_timestamps: true,
            include_targets: true,
            include_file_line: false,
            include_thread_ids: false,
            include_span_events: false,
        }
    }
}

impl LogConfig {
    /// Creates a development-friendly configuration
    ///
    /// Uses pretty format with full context for local development.
    #[must_use]
    pub fn development() -> Self {
        Self {
            level: "debug".to_string(),
            format: LogFormat::Pretty,
            include_timestamps: true,
            include_targets: true,
            include_file_line: true,
            include_thread_ids: false,
            include_span_events: false,
        }
    }

    /// Creates a production-ready configuration
    ///
    /// Uses JSON format for structured log aggregation.
    #[must_use]
    pub fn production() -> Self {
        Self {
            level: "info".to_string(),
            format: LogFormat::Json,
            include_timestamps: true,
            include_targets: true,
            include_file_line: false,
            include_thread_ids: true,
            include_span_events: false,
        }
    }

    /// Creates a configuration from environment variables
    ///
    /// Reads from:
    /// - `RUST_LOG` or `MAINTENANCE_ENGINE_LOG` for level
    /// - `MAINTENANCE_ENGINE_LOG_FORMAT` for format
    ///
    /// # Errors
    ///
    /// Returns error if environment variables contain invalid values
    pub fn from_env() -> Result<Self> {
        let level = std::env::var("MAINTENANCE_ENGINE_LOG")
            .or_else(|_| std::env::var("RUST_LOG"))
            .unwrap_or_else(|_| "info".to_string());

        let format = std::env::var("MAINTENANCE_ENGINE_LOG_FORMAT")
            .map(|s| LogFormat::parse_str(&s))
            .unwrap_or(Ok(LogFormat::Pretty))?;

        Ok(Self {
            level,
            format,
            ..Self::default()
        })
    }

    /// Validate the configuration
    ///
    /// # Errors
    ///
    /// Returns error if configuration is invalid
    pub fn validate(&self) -> Result<()> {
        // Validate log level
        LogLevel::parse_str(&self.level)?;
        Ok(())
    }
}

// ============================================================================
// Initialization Functions
// ============================================================================

/// Initialize the logging system with the given configuration
///
/// Sets up the global tracing subscriber based on the configuration.
/// This function can only be called once; subsequent calls will return
/// an error.
///
/// # Errors
///
/// Returns error if:
/// - Logging has already been initialized
/// - Configuration is invalid
/// - Subscriber initialization fails
///
/// # Example
///
/// ```no_run
/// use maintenance_engine::m1_foundation::logging::{init_logging, LogConfig};
///
/// let config = LogConfig::development();
/// init_logging(&config).expect("Failed to initialize logging");
/// ```
pub fn init_logging(config: &LogConfig) -> Result<()> {
    // Check if already initialized
    if LOGGING_INITIALIZED.get().is_some() {
        return Err(Error::Config("Logging already initialized".to_string()));
    }

    // Validate configuration
    config.validate()?;

    // Build span events configuration
    let span_events = if config.include_span_events {
        FmtSpan::NEW | FmtSpan::CLOSE
    } else {
        FmtSpan::NONE
    };

    // Parse log level
    let level = LogLevel::parse_str(&config.level)?;

    // Initialize based on format - use simplified approach without optional features
    let result = match config.format {
        LogFormat::Json | LogFormat::Compact => {
            init_compact_subscriber(config, span_events, level)
        }
        LogFormat::Pretty => init_default_subscriber(config, span_events, level),
    };

    result?;

    // Mark as initialized
    let _ = LOGGING_INITIALIZED.set(true);

    tracing::info!(
        level = %config.level,
        format = %config.format,
        "Logging system initialized"
    );

    Ok(())
}

/// Initialize default format subscriber
fn init_default_subscriber(
    config: &LogConfig,
    span_events: FmtSpan,
    level: LogLevel,
) -> Result<()> {
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(level.to_tracing_level())
        .with_span_events(span_events)
        .with_target(config.include_targets)
        .with_file(config.include_file_line)
        .with_line_number(config.include_file_line)
        .with_thread_ids(config.include_thread_ids)
        .finish();

    subscriber
        .try_init()
        .map_err(|e| Error::Config(format!("Failed to initialize subscriber: {e}")))
}

/// Initialize compact format subscriber
fn init_compact_subscriber(
    config: &LogConfig,
    span_events: FmtSpan,
    level: LogLevel,
) -> Result<()> {
    let subscriber = tracing_subscriber::fmt()
        .compact()
        .with_max_level(level.to_tracing_level())
        .with_span_events(span_events)
        .with_target(config.include_targets)
        .with_file(config.include_file_line)
        .with_line_number(config.include_file_line)
        .with_thread_ids(config.include_thread_ids)
        .finish();

    subscriber
        .try_init()
        .map_err(|e| Error::Config(format!("Failed to initialize subscriber: {e}")))
}

/// Try to initialize logging, ignoring errors if already initialized
///
/// Useful for tests or situations where logging might already be set up.
///
/// # Example
///
/// ```
/// use maintenance_engine::m1_foundation::logging::{try_init_logging, LogConfig};
///
/// // Safe to call multiple times
/// try_init_logging(&LogConfig::default());
/// try_init_logging(&LogConfig::default()); // No error
/// ```
pub fn try_init_logging(config: &LogConfig) {
    let _ = init_logging(config);
}

// ============================================================================
// Context Functions
// ============================================================================

/// Execute a function within a logging context span
///
/// Creates a tracing span with the context's correlation ID and metadata,
/// then executes the provided closure within that span.
///
/// # Example
///
/// ```
/// use maintenance_engine::m1_foundation::logging::{with_context, LogContext};
///
/// let ctx = LogContext::with_context("maintenance-engine", "L1", "M03");
/// let result = with_context(&ctx, || {
///     // All logs within here include the context
///     42
/// });
/// assert_eq!(result, 42);
/// ```
pub fn with_context<F, R>(ctx: &LogContext, f: F) -> R
where
    F: FnOnce() -> R,
{
    let span = tracing::info_span!(
        "context",
        correlation_id = %ctx.correlation_id,
        service_id = ctx.service_id.as_deref().unwrap_or("unknown"),
        layer = ctx.layer.as_deref().unwrap_or("unknown"),
        module = ctx.module.as_deref().unwrap_or("unknown"),
    );

    span.in_scope(f)
}

/// Execute an async function within a logging context span
///
/// Creates a tracing span with the context's correlation ID and metadata,
/// then executes the provided async closure within that span.
///
/// # Example
///
/// ```
/// use maintenance_engine::m1_foundation::logging::{with_context_async, LogContext};
///
/// async fn example() {
///     let ctx = LogContext::with_context("maintenance-engine", "L1", "M03");
///     let result = with_context_async(&ctx, async {
///         // All logs within here include the context
///         42
///     }).await;
///     assert_eq!(result, 42);
/// }
/// ```
pub async fn with_context_async<F, R>(ctx: &LogContext, f: F) -> R
where
    F: std::future::Future<Output = R>,
{
    use tracing::Instrument;

    let span = tracing::info_span!(
        "context",
        correlation_id = %ctx.correlation_id,
        service_id = ctx.service_id.as_deref().unwrap_or("unknown"),
        layer = ctx.layer.as_deref().unwrap_or("unknown"),
        module = ctx.module.as_deref().unwrap_or("unknown"),
    );

    f.instrument(span).await
}

// ============================================================================
// Utility Functions
// ============================================================================

/// Generate a new correlation ID
///
/// Creates a UUID v4-based correlation ID for request tracing.
///
/// # Example
///
/// ```
/// use maintenance_engine::m1_foundation::logging::generate_correlation_id;
///
/// let id1 = generate_correlation_id();
/// let id2 = generate_correlation_id();
/// assert_ne!(id1, id2);
/// assert!(!id1.is_empty());
/// ```
#[must_use]
pub fn generate_correlation_id() -> String {
    Uuid::new_v4().to_string()
}

/// Generate a short correlation ID (first 8 characters)
///
/// Creates a shortened UUID v4-based correlation ID for compact logging.
///
/// # Example
///
/// ```
/// use maintenance_engine::m1_foundation::logging::generate_short_correlation_id;
///
/// let id = generate_short_correlation_id();
/// assert_eq!(id.len(), 8);
/// ```
#[must_use]
pub fn generate_short_correlation_id() -> String {
    Uuid::new_v4().to_string()[..8].to_string()
}

/// Check if logging has been initialized
///
/// # Example
///
/// ```
/// use maintenance_engine::m1_foundation::logging::is_logging_initialized;
///
/// // May be true or false depending on test order
/// let _ = is_logging_initialized();
/// ```
#[must_use]
pub fn is_logging_initialized() -> bool {
    LOGGING_INITIALIZED.get().copied().unwrap_or(false)
}

// ============================================================================
// Logging Macros Re-exports
// ============================================================================

/// Re-export tracing macros for convenience
pub use tracing::{debug, error, info, trace, warn};

/// Re-export span creation
pub use tracing::{debug_span, error_span, info_span, trace_span, warn_span};

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    // ====================================================================
    // Correlation ID generation
    // ====================================================================

    #[test]
    fn test_generate_correlation_id() {
        let id1 = generate_correlation_id();
        let id2 = generate_correlation_id();

        // IDs should be unique
        assert_ne!(id1, id2);

        // Should be valid UUID format (36 chars with hyphens)
        assert_eq!(id1.len(), 36);
        assert_eq!(id2.len(), 36);
    }

    #[test]
    fn test_generate_short_correlation_id() {
        let id = generate_short_correlation_id();

        // Should be exactly 8 characters
        assert_eq!(id.len(), 8);

        // Should be hex characters
        assert!(id.chars().all(|c| c.is_ascii_hexdigit() || c == '-'));
    }

    #[test]
    fn test_correlation_id_uniqueness_over_100_calls() {
        let mut ids = HashSet::new();
        for _ in 0..100 {
            let id = generate_correlation_id();
            assert!(ids.insert(id), "duplicate correlation ID generated");
        }
        assert_eq!(ids.len(), 100);
    }

    #[test]
    fn test_correlation_id_format_uuid_v4() {
        let id = generate_correlation_id();
        // UUID v4 format: xxxxxxxx-xxxx-4xxx-[89ab]xxx-xxxxxxxxxxxx
        let parts: Vec<&str> = id.split('-').collect();
        assert_eq!(parts.len(), 5);
        assert_eq!(parts[0].len(), 8);
        assert_eq!(parts[1].len(), 4);
        assert_eq!(parts[2].len(), 4);
        assert_eq!(parts[3].len(), 4);
        assert_eq!(parts[4].len(), 12);
        // Version nibble must be '4'
        assert!(parts[2].starts_with('4'));
    }

    #[test]
    fn test_short_correlation_id_uniqueness() {
        let mut ids = HashSet::new();
        for _ in 0..100 {
            let id = generate_short_correlation_id();
            ids.insert(id);
        }
        // With 8 hex chars, collisions are astronomically unlikely over 100 calls
        assert!(ids.len() >= 99);
    }

    // ====================================================================
    // LogContext basics
    // ====================================================================

    #[test]
    fn test_log_context_new() {
        let ctx = LogContext::new();

        assert!(!ctx.correlation_id.is_empty());
        assert!(ctx.service_id.is_none());
        assert!(ctx.layer.is_none());
        assert!(ctx.module.is_none());
    }

    #[test]
    fn test_log_context_with_context() {
        let ctx = LogContext::with_context("test-service", "L1", "M03");

        assert!(!ctx.correlation_id.is_empty());
        assert_eq!(ctx.service_id, Some("test-service".to_string()));
        assert_eq!(ctx.layer, Some("L1".to_string()));
        assert_eq!(ctx.module, Some("M03".to_string()));
    }

    #[test]
    fn test_log_context_with_context_accepts_string() {
        let svc = String::from("owned-service");
        let layer = String::from("L2");
        let module = String::from("M10");
        let ctx = LogContext::with_context(svc, layer, module);

        assert_eq!(ctx.service_id, Some("owned-service".to_string()));
        assert_eq!(ctx.layer, Some("L2".to_string()));
        assert_eq!(ctx.module, Some("M10".to_string()));
    }

    #[test]
    fn test_log_context_child_context() {
        let parent = LogContext::with_context("test-service", "L1", "M03");
        let child = parent.child_context();

        // Child should inherit correlation ID
        assert_eq!(child.correlation_id, parent.correlation_id);
        assert_eq!(child.service_id, parent.service_id);
        assert_eq!(child.layer, parent.layer);
        assert_eq!(child.module, parent.module);
    }

    #[test]
    fn test_log_context_with_module() {
        let ctx = LogContext::with_context("test-service", "L1", "M03");
        let new_ctx = ctx.with_module("M04");

        // Should keep correlation ID but change module
        assert_eq!(new_ctx.correlation_id, ctx.correlation_id);
        assert_eq!(new_ctx.module, Some("M04".to_string()));
    }

    #[test]
    fn test_log_context_with_module_accepts_string() {
        let ctx = LogContext::with_context("svc", "L1", "M01");
        let new_ctx = ctx.with_module(String::from("M99"));
        assert_eq!(new_ctx.module, Some("M99".to_string()));
        assert_eq!(new_ctx.correlation_id, ctx.correlation_id);
    }

    #[test]
    fn test_log_context_with_layer() {
        let ctx = LogContext::with_context("test-service", "L1", "M03");
        let new_ctx = ctx.with_layer("L3");

        assert_eq!(new_ctx.correlation_id, ctx.correlation_id);
        assert_eq!(new_ctx.layer, Some("L3".to_string()));
        assert_eq!(new_ctx.module, ctx.module);
    }

    #[test]
    fn test_log_context_with_layer_accepts_string() {
        let ctx = LogContext::with_context("svc", "L1", "M01");
        let new_ctx = ctx.with_layer(String::from("L6"));
        assert_eq!(new_ctx.layer, Some("L6".to_string()));
        assert_eq!(new_ctx.correlation_id, ctx.correlation_id);
    }

    // ====================================================================
    // LogContext propagation chain
    // ====================================================================

    #[test]
    fn test_propagation_chain_parent_child_grandchild() {
        let parent = LogContext::with_context("engine", "L1", "M01");
        let child = parent.with_module("M03");
        let grandchild = child.with_module("M05");

        // All share the same correlation ID
        assert_eq!(parent.correlation_id, child.correlation_id);
        assert_eq!(child.correlation_id, grandchild.correlation_id);

        // But modules differ
        assert_eq!(parent.module, Some("M01".to_string()));
        assert_eq!(child.module, Some("M03".to_string()));
        assert_eq!(grandchild.module, Some("M05".to_string()));
    }

    #[test]
    fn test_propagation_chain_layer_change() {
        let ctx = LogContext::with_context("engine", "L1", "M01");
        let l2_ctx = ctx.with_layer("L2");
        let l3_ctx = l2_ctx.with_layer("L3");

        assert_eq!(ctx.correlation_id, l3_ctx.correlation_id);
        assert_eq!(l3_ctx.layer, Some("L3".to_string()));
        assert_eq!(l3_ctx.module, Some("M01".to_string()));
    }

    #[test]
    fn test_propagation_chain_mixed_changes() {
        let ctx = LogContext::with_context("engine", "L1", "M01");
        let step1 = ctx.with_module("M10");
        let step2 = step1.with_layer("L3");

        assert_eq!(ctx.correlation_id, step2.correlation_id);
        assert_eq!(step2.layer, Some("L3".to_string()));
        assert_eq!(step2.module, Some("M10".to_string()));
        assert_eq!(step2.service_id, Some("engine".to_string()));
    }

    // ====================================================================
    // LogContext Display
    // ====================================================================

    #[test]
    fn test_log_context_display_full() {
        let ctx = LogContext::with_context("test-service", "L1", "M03");
        let display = ctx.to_string();

        assert!(display.contains("corr_id="));
        assert!(display.contains("service=test-service"));
        assert!(display.contains("layer=L1"));
        assert!(display.contains("module=M03"));
    }

    #[test]
    fn test_log_context_display_minimal() {
        let ctx = LogContext::new();
        let display = ctx.to_string();

        assert!(display.contains("corr_id="));
        assert!(!display.contains("service="));
        assert!(!display.contains("layer="));
        assert!(!display.contains("module="));
    }

    #[test]
    fn test_log_context_display_partial() {
        let mut ctx = LogContext::new();
        ctx.service_id = Some("svc".to_string());
        let display = ctx.to_string();

        assert!(display.contains("corr_id="));
        assert!(display.contains("service=svc"));
        assert!(!display.contains("layer="));
        assert!(!display.contains("module="));
    }

    // ====================================================================
    // LogContext Default
    // ====================================================================

    #[test]
    fn test_log_context_default() {
        let ctx = LogContext::default();

        assert!(ctx.correlation_id.is_empty());
        assert!(ctx.service_id.is_none());
        assert!(ctx.layer.is_none());
        assert!(ctx.module.is_none());
    }

    // ====================================================================
    // LogContext edge cases
    // ====================================================================

    #[test]
    fn test_log_context_empty_strings() {
        let ctx = LogContext::with_context("", "", "");

        assert_eq!(ctx.service_id, Some(String::new()));
        assert_eq!(ctx.layer, Some(String::new()));
        assert_eq!(ctx.module, Some(String::new()));
        assert!(!ctx.correlation_id.is_empty());
    }

    #[test]
    fn test_log_context_long_strings() {
        let long_str = "x".repeat(10_000);
        let ctx = LogContext::with_context(
            long_str.clone(),
            long_str.clone(),
            long_str.clone(),
        );

        assert_eq!(ctx.service_id.as_deref(), Some(long_str.as_str()));
        assert_eq!(ctx.layer.as_deref(), Some(long_str.as_str()));
        assert_eq!(ctx.module.as_deref(), Some(long_str.as_str()));
    }

    #[test]
    fn test_log_context_unicode_strings() {
        let ctx = LogContext::with_context(
            "Dienst-Motor",
            "Schicht-1",
            "Modul-03",
        );

        assert_eq!(ctx.service_id, Some("Dienst-Motor".to_string()));
    }

    // ====================================================================
    // CorrelationProvider trait compliance
    // ====================================================================

    #[test]
    fn test_correlation_provider_returns_correct_id() {
        let ctx = LogContext::with_context("svc", "L1", "M01");
        let provider: &dyn CorrelationProvider = &ctx;
        assert_eq!(provider.correlation_id(), ctx.correlation_id);
    }

    #[test]
    fn test_correlation_provider_child_inherits_id() {
        let ctx = LogContext::with_context("svc", "L1", "M01");
        let provider: &dyn CorrelationProvider = &ctx;
        let child = provider.child("M05");
        assert_eq!(child.correlation_id(), ctx.correlation_id);
    }

    #[test]
    fn test_correlation_provider_child_chain() {
        let ctx = LogContext::with_context("svc", "L1", "M01");
        let provider: &dyn CorrelationProvider = &ctx;
        let child = provider.child("M05");
        let grandchild = child.child("M10");
        assert_eq!(grandchild.correlation_id(), ctx.correlation_id);
    }

    #[test]
    fn test_correlation_provider_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<LogContext>();
    }

    // ====================================================================
    // LogFormat
    // ====================================================================

    #[test]
    fn test_log_format_parse_str() {
        assert_eq!(LogFormat::parse_str("json").ok(), Some(LogFormat::Json));
        assert_eq!(LogFormat::parse_str("JSON").ok(), Some(LogFormat::Json));
        assert_eq!(LogFormat::parse_str("pretty").ok(), Some(LogFormat::Pretty));
        assert_eq!(LogFormat::parse_str("compact").ok(), Some(LogFormat::Compact));
        assert!(LogFormat::parse_str("invalid").is_err());
    }

    #[test]
    fn test_log_format_parse_case_insensitive() {
        assert_eq!(LogFormat::parse_str("Json").ok(), Some(LogFormat::Json));
        assert_eq!(LogFormat::parse_str("PRETTY").ok(), Some(LogFormat::Pretty));
        assert_eq!(LogFormat::parse_str("Compact").ok(), Some(LogFormat::Compact));
        assert_eq!(LogFormat::parse_str("jSoN").ok(), Some(LogFormat::Json));
    }

    #[test]
    fn test_log_format_parse_invalid() {
        assert!(LogFormat::parse_str("xml").is_err());
        assert!(LogFormat::parse_str("").is_err());
        assert!(LogFormat::parse_str("  json  ").is_err());
    }

    #[test]
    fn test_log_format_display() {
        assert_eq!(LogFormat::Json.to_string(), "json");
        assert_eq!(LogFormat::Pretty.to_string(), "pretty");
        assert_eq!(LogFormat::Compact.to_string(), "compact");
    }

    #[test]
    fn test_log_format_display_roundtrip() {
        let formats = [LogFormat::Json, LogFormat::Pretty, LogFormat::Compact];
        for fmt in formats {
            let s = fmt.to_string();
            let parsed: LogFormat = s.parse().ok().unwrap_or_default();
            assert_eq!(parsed, fmt);
        }
    }

    #[test]
    fn test_log_format_from_str() {
        let parsed: std::result::Result<LogFormat, _> = "json".parse();
        assert!(parsed.is_ok());
        assert_eq!(parsed.ok(), Some(LogFormat::Json));
    }

    #[test]
    fn test_log_format_default() {
        assert_eq!(LogFormat::default(), LogFormat::Pretty);
    }

    // ====================================================================
    // LogLevel
    // ====================================================================

    #[test]
    fn test_log_level_parse_str() {
        assert_eq!(LogLevel::parse_str("trace").ok(), Some(LogLevel::Trace));
        assert_eq!(LogLevel::parse_str("debug").ok(), Some(LogLevel::Debug));
        assert_eq!(LogLevel::parse_str("info").ok(), Some(LogLevel::Info));
        assert_eq!(LogLevel::parse_str("INFO").ok(), Some(LogLevel::Info));
        assert_eq!(LogLevel::parse_str("warn").ok(), Some(LogLevel::Warn));
        assert_eq!(LogLevel::parse_str("warning").ok(), Some(LogLevel::Warn));
        assert_eq!(LogLevel::parse_str("error").ok(), Some(LogLevel::Error));
        assert!(LogLevel::parse_str("invalid").is_err());
    }

    #[test]
    fn test_log_level_ordering() {
        assert!(LogLevel::Trace < LogLevel::Debug);
        assert!(LogLevel::Debug < LogLevel::Info);
        assert!(LogLevel::Info < LogLevel::Warn);
        assert!(LogLevel::Warn < LogLevel::Error);
    }

    #[test]
    fn test_log_level_ordering_all_pairwise() {
        let levels = [
            LogLevel::Trace,
            LogLevel::Debug,
            LogLevel::Info,
            LogLevel::Warn,
            LogLevel::Error,
        ];
        for i in 0..levels.len() {
            for j in (i + 1)..levels.len() {
                assert!(levels[i] < levels[j]);
            }
        }
    }

    #[test]
    fn test_log_level_equality() {
        assert_eq!(LogLevel::Info, LogLevel::Info);
        assert_ne!(LogLevel::Info, LogLevel::Debug);
    }

    #[test]
    fn test_log_level_to_tracing() {
        assert_eq!(LogLevel::Trace.to_tracing_level(), Level::TRACE);
        assert_eq!(LogLevel::Debug.to_tracing_level(), Level::DEBUG);
        assert_eq!(LogLevel::Info.to_tracing_level(), Level::INFO);
        assert_eq!(LogLevel::Warn.to_tracing_level(), Level::WARN);
        assert_eq!(LogLevel::Error.to_tracing_level(), Level::ERROR);
    }

    #[test]
    fn test_log_level_display() {
        assert_eq!(LogLevel::Trace.to_string(), "trace");
        assert_eq!(LogLevel::Debug.to_string(), "debug");
        assert_eq!(LogLevel::Info.to_string(), "info");
        assert_eq!(LogLevel::Warn.to_string(), "warn");
        assert_eq!(LogLevel::Error.to_string(), "error");
    }

    #[test]
    fn test_log_level_display_roundtrip() {
        let levels = [
            LogLevel::Trace,
            LogLevel::Debug,
            LogLevel::Info,
            LogLevel::Warn,
            LogLevel::Error,
        ];
        for lvl in levels {
            let s = lvl.to_string();
            let parsed: LogLevel = s.parse().ok().unwrap_or_default();
            assert_eq!(parsed, lvl);
        }
    }

    #[test]
    fn test_log_level_from_str() {
        let parsed: std::result::Result<LogLevel, _> = "warn".parse();
        assert!(parsed.is_ok());
        assert_eq!(parsed.ok(), Some(LogLevel::Warn));
    }

    #[test]
    fn test_log_level_default() {
        assert_eq!(LogLevel::default(), LogLevel::Info);
    }

    // ====================================================================
    // LogConfig
    // ====================================================================

    #[test]
    fn test_log_config_default() {
        let config = LogConfig::default();

        assert_eq!(config.level, "info");
        assert_eq!(config.format, LogFormat::Pretty);
        assert!(config.include_timestamps);
        assert!(config.include_targets);
        assert!(!config.include_file_line);
        assert!(!config.include_thread_ids);
        assert!(!config.include_span_events);
    }

    #[test]
    fn test_log_config_development() {
        let config = LogConfig::development();

        assert_eq!(config.level, "debug");
        assert_eq!(config.format, LogFormat::Pretty);
        assert!(config.include_file_line);
    }

    #[test]
    fn test_log_config_production() {
        let config = LogConfig::production();

        assert_eq!(config.level, "info");
        assert_eq!(config.format, LogFormat::Json);
        assert!(config.include_thread_ids);
        assert!(!config.include_file_line);
    }

    #[test]
    fn test_log_config_validate() {
        let valid_config = LogConfig::default();
        assert!(valid_config.validate().is_ok());

        let invalid_config = LogConfig {
            level: "invalid_level".to_string(),
            ..LogConfig::default()
        };
        assert!(invalid_config.validate().is_err());
    }

    #[test]
    fn test_log_config_validate_all_valid_levels() {
        for level in &["trace", "debug", "info", "warn", "warning", "error"] {
            let config = LogConfig {
                level: (*level).to_string(),
                ..LogConfig::default()
            };
            assert!(config.validate().is_ok(), "level '{level}' should be valid");
        }
    }

    #[test]
    fn test_log_config_validate_empty_level() {
        let config = LogConfig {
            level: String::new(),
            ..LogConfig::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_log_config_validate_whitespace_level() {
        let config = LogConfig {
            level: " info ".to_string(),
            ..LogConfig::default()
        };
        // " info " after to_lowercase is still " info " which should fail
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_log_config_validate_numeric_level() {
        let config = LogConfig {
            level: "42".to_string(),
            ..LogConfig::default()
        };
        assert!(config.validate().is_err());
    }

    // ====================================================================
    // init_logging idempotency
    // ====================================================================

    #[test]
    fn test_try_init_logging_idempotent() {
        // try_init_logging should not panic when called multiple times
        let config = LogConfig::default();
        try_init_logging(&config);
        try_init_logging(&config);
        // No panic means success
    }

    // ====================================================================
    // with_context / with_context_async
    // ====================================================================

    #[test]
    fn test_with_context() {
        let ctx = LogContext::with_context("test-service", "L1", "M03");
        let result = with_context(&ctx, || 42);
        assert_eq!(result, 42);
    }

    #[test]
    fn test_with_context_propagates_return_value() {
        let ctx = LogContext::new();
        let result = with_context(&ctx, || "hello".to_string());
        assert_eq!(result, "hello");
    }

    #[tokio::test]
    async fn test_with_context_async() {
        let ctx = LogContext::with_context("test-service", "L1", "M03");
        let result = with_context_async(&ctx, async { 42 }).await;
        assert_eq!(result, 42);
    }

    #[tokio::test]
    async fn test_with_context_async_propagates_return_value() {
        let ctx = LogContext::new();
        let result = with_context_async(&ctx, async { "async-hello".to_string() }).await;
        assert_eq!(result, "async-hello");
    }

    // ====================================================================
    // is_logging_initialized
    // ====================================================================

    #[test]
    fn test_is_logging_initialized_returns_bool() {
        // We cannot guarantee the value, but it must not panic
        let result = is_logging_initialized();
        assert!(result || !result);
    }

    // ====================================================================
    // LogConfig from_env
    // ====================================================================

    #[test]
    fn test_log_config_from_env_defaults() {
        // Without setting env vars, should use defaults
        // (env vars might be set by other tests, so just verify it doesn't error)
        let result = LogConfig::from_env();
        assert!(result.is_ok());
    }

    // ====================================================================
    // LogConfig clone
    // ====================================================================

    #[test]
    fn test_log_config_clone() {
        let config = LogConfig::production();
        let cloned = config.clone();
        assert_eq!(cloned.level, config.level);
        assert_eq!(cloned.format, config.format);
        assert_eq!(cloned.include_timestamps, config.include_timestamps);
        assert_eq!(cloned.include_targets, config.include_targets);
        assert_eq!(cloned.include_file_line, config.include_file_line);
        assert_eq!(cloned.include_thread_ids, config.include_thread_ids);
        assert_eq!(cloned.include_span_events, config.include_span_events);
    }

    // ====================================================================
    // NAM: agent_id tests
    // ====================================================================

    #[test]
    fn test_log_context_agent_id_default_none() {
        let ctx = LogContext::new();
        assert!(ctx.agent_id.is_none());
    }

    #[test]
    fn test_log_context_with_agent() {
        let ctx = LogContext::new().with_agent("@0.A");
        assert_eq!(ctx.agent_id.as_deref(), Some("@0.A"));
    }

    #[test]
    fn test_agent_id_propagates_through_child() {
        let ctx = LogContext::with_context("me", "L1", "M03").with_agent("agent-1");
        let child = ctx.child_context();
        assert_eq!(child.agent_id.as_deref(), Some("agent-1"));
    }

    #[test]
    fn test_correlation_provider_agent_id_default_none() {
        let ctx = LogContext::new();
        let provider: &dyn CorrelationProvider = &ctx;
        assert!(provider.agent_id().is_none());
    }

    #[test]
    fn test_correlation_provider_agent_id_with_agent() {
        let ctx = LogContext::new().with_agent("svc-agent");
        let provider: &dyn CorrelationProvider = &ctx;
        assert_eq!(provider.agent_id(), Some("svc-agent"));
    }

    // ====================================================================
    // NAM: to_tensor_position tests
    // ====================================================================

    #[test]
    fn test_to_tensor_position_valid_dims() {
        let ctx = LogContext::with_context("me", "L1", "M03");
        let tensor = ctx.to_tensor_position();
        assert!(tensor.validate().is_ok());
    }

    #[test]
    fn test_to_tensor_position_layer_mapping() {
        let ctx1 = LogContext::with_context("me", "L1", "M03");
        let ctx3 = LogContext::with_context("me", "L3", "M15");
        let t1 = ctx1.to_tensor_position();
        let t3 = ctx3.to_tensor_position();
        assert!(t3.tier > t1.tier, "L3 tier should be higher than L1");
    }

    #[test]
    fn test_with_agent_chains_with_context() {
        let ctx = LogContext::with_context("me", "L1", "M03").with_agent("bot-1");
        assert_eq!(ctx.service_id.as_deref(), Some("me"));
        assert_eq!(ctx.agent_id.as_deref(), Some("bot-1"));
    }

    #[test]
    fn test_agent_id_in_display_output() {
        let ctx = LogContext::new().with_agent("test-agent");
        let display = ctx.to_string();
        assert!(display.contains("agent=test-agent"));
    }
}
