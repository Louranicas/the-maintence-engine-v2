//! # M01: Error Taxonomy
//!
//! Unified error handling for the Maintenance Engine.
//!
//! ## Layer: L1 (Foundation)
//! ## Dependencies: None (foundational)
//!
//! ## 12D Tensor Encoding
//! ```text
//! [1/36, 0.0, 1/6, 0, 0, 0.5, health, uptime, synergy, latency, error_rate, temporal]
//! ```
//!
//! ## Error Categories
//!
//! | Category | Code Range | Description |
//! |----------|------------|-------------|
//! | Config | 1000-1099 | Configuration errors |
//! | Database | 1100-1199 | Database operations |
//! | Network | 1200-1299 | Network/wire errors |
//! | Consensus | 1300-1399 | PBFT consensus |
//! | Learning | 1400-1499 | Hebbian learning |
//! | Validation | 1500-1599 | Input validation |
//!
//! ## Traits
//!
//! - [`ErrorClassifier`]: Classify errors by retryability, transience, and severity
//!
//! ## Related Documentation
//! - [Module Specification](../../ai_docs/modules/M01_ERROR_TAXONOMY.md)

use std::fmt;

// ============================================================================
// Severity
// ============================================================================

/// Severity level for error classification.
///
/// Ordered from least to most severe, enabling comparisons.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    /// Informational — no action required
    Low,
    /// Warning — should be monitored
    Medium,
    /// Error — requires attention
    High,
    /// Fatal — immediate action required
    Critical,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Low => write!(f, "LOW"),
            Self::Medium => write!(f, "MEDIUM"),
            Self::High => write!(f, "HIGH"),
            Self::Critical => write!(f, "CRITICAL"),
        }
    }
}

// ============================================================================
// ErrorClassifier trait
// ============================================================================

/// Classify errors by operational characteristics.
///
/// Enables upper layers to make intelligent decisions about error handling:
/// - Retry logic based on [`is_retryable`](ErrorClassifier::is_retryable)
/// - Transient vs permanent failure distinction via [`is_transient`](ErrorClassifier::is_transient)
/// - Severity-based escalation via [`severity`](ErrorClassifier::severity)
pub trait ErrorClassifier {
    /// Whether the operation that caused this error can be retried.
    ///
    /// Returns `true` for errors like network timeouts, circuit breaker open,
    /// and transient I/O failures.
    fn is_retryable(&self) -> bool;

    /// Whether the error is transient (likely to resolve on its own).
    ///
    /// Returns `true` for temporary conditions like network blips,
    /// circuit breaker cooldowns, and consensus quorum shortfalls.
    fn is_transient(&self) -> bool;

    /// The severity level of this error.
    fn severity(&self) -> Severity;

    /// Numeric error code for machine-readable classification (NAM R2).
    ///
    /// Code ranges: Config=1000, Database=1100, Network=1200,
    /// Consensus=1300, Learning=1400, Validation=1500, IO=1600, Other=1900.
    fn error_code(&self) -> u32 {
        0
    }

    /// Human-readable error category string (NAM R2).
    fn error_category(&self) -> &'static str {
        "other"
    }
}

// ============================================================================
// Error enum
// ============================================================================

/// Unified error type for the Maintenance Engine.
///
/// All modules return this error type through the [`Result`] type alias.
/// Implements [`ErrorClassifier`] for intelligent error handling.
#[derive(Debug)]
pub enum Error {
    /// Configuration error
    Config(String),

    /// Database operation error
    Database(String),

    /// Network/wire communication error
    Network {
        /// Target service
        target: String,
        /// Error message
        message: String,
    },

    /// Circuit breaker is open
    CircuitOpen {
        /// Service ID
        service_id: String,
        /// Time until half-open
        retry_after_ms: u64,
    },

    /// Consensus quorum not reached
    ConsensusQuorum {
        /// Required votes
        required: u32,
        /// Received votes
        received: u32,
    },

    /// PBFT view change required
    ViewChange {
        /// Current view
        current_view: u64,
        /// Proposed view
        new_view: u64,
    },

    /// Hebbian pathway not found
    PathwayNotFound {
        /// Source module
        source: String,
        /// Target module
        target: String,
    },

    /// Tensor validation error
    TensorValidation {
        /// Dimension index
        dimension: usize,
        /// Invalid value
        value: f64,
    },

    /// Service not found
    ServiceNotFound(String),

    /// Health check failed
    HealthCheckFailed {
        /// Service ID
        service_id: String,
        /// Failure reason
        reason: String,
    },

    /// Escalation required
    EscalationRequired {
        /// Current tier
        from_tier: String,
        /// Target tier
        to_tier: String,
        /// Reason for escalation
        reason: String,
    },

    /// Timeout error
    Timeout {
        /// Operation name
        operation: String,
        /// Timeout duration in ms
        timeout_ms: u64,
    },

    /// Pipeline operation error
    Pipeline(String),

    /// Validation error
    Validation(String),

    /// IO error
    Io(std::io::Error),

    /// Generic error
    Other(String),
}

// ============================================================================
// Clone (manual — std::io::Error is not Clone)
// ============================================================================

impl Clone for Error {
    fn clone(&self) -> Self {
        match self {
            Self::Config(s) => Self::Config(s.clone()),
            Self::Database(s) => Self::Database(s.clone()),
            Self::Network { target, message } => Self::Network {
                target: target.clone(),
                message: message.clone(),
            },
            Self::CircuitOpen {
                service_id,
                retry_after_ms,
            } => Self::CircuitOpen {
                service_id: service_id.clone(),
                retry_after_ms: *retry_after_ms,
            },
            Self::ConsensusQuorum { required, received } => Self::ConsensusQuorum {
                required: *required,
                received: *received,
            },
            Self::ViewChange {
                current_view,
                new_view,
            } => Self::ViewChange {
                current_view: *current_view,
                new_view: *new_view,
            },
            Self::PathwayNotFound { source, target } => Self::PathwayNotFound {
                source: source.clone(),
                target: target.clone(),
            },
            Self::TensorValidation { dimension, value } => Self::TensorValidation {
                dimension: *dimension,
                value: *value,
            },
            Self::ServiceNotFound(s) => Self::ServiceNotFound(s.clone()),
            Self::HealthCheckFailed { service_id, reason } => Self::HealthCheckFailed {
                service_id: service_id.clone(),
                reason: reason.clone(),
            },
            Self::EscalationRequired {
                from_tier,
                to_tier,
                reason,
            } => Self::EscalationRequired {
                from_tier: from_tier.clone(),
                to_tier: to_tier.clone(),
                reason: reason.clone(),
            },
            Self::Timeout {
                operation,
                timeout_ms,
            } => Self::Timeout {
                operation: operation.clone(),
                timeout_ms: *timeout_ms,
            },
            Self::Pipeline(s) => Self::Pipeline(s.clone()),
            Self::Validation(s) => Self::Validation(s.clone()),
            Self::Io(e) => Self::Io(std::io::Error::new(e.kind(), e.to_string())),
            Self::Other(s) => Self::Other(s.clone()),
        }
    }
}

// ============================================================================
// PartialEq / Eq (manual — std::io::Error is not PartialEq)
// ============================================================================

impl PartialEq for Error {
    fn eq(&self, other: &Self) -> bool {
        // Delegate to a helper to keep the eq method body within line limits.
        eq_impl(self, other)
    }
}

/// Equality comparison for [`Error`] — extracted to satisfy function-length lint.
fn eq_impl(a: &Error, b: &Error) -> bool {
    match (a, b) {
        (Error::Config(a), Error::Config(b))
        | (Error::Database(a), Error::Database(b))
        | (Error::Pipeline(a), Error::Pipeline(b))
        | (Error::Validation(a), Error::Validation(b))
        | (Error::ServiceNotFound(a), Error::ServiceNotFound(b))
        | (Error::Other(a), Error::Other(b)) => a == b,
        (
            Error::Network { target: t1, message: m1 },
            Error::Network { target: t2, message: m2 },
        )
        | (
            Error::PathwayNotFound { source: t1, target: m1 },
            Error::PathwayNotFound { source: t2, target: m2 },
        ) => t1 == t2 && m1 == m2,
        (
            Error::HealthCheckFailed { service_id: s1, reason: r1 },
            Error::HealthCheckFailed { service_id: s2, reason: r2 },
        ) => s1 == s2 && r1 == r2,
        (
            Error::CircuitOpen { service_id: s1, retry_after_ms: r1 },
            Error::CircuitOpen { service_id: s2, retry_after_ms: r2 },
        ) => s1 == s2 && r1 == r2,
        (
            Error::ConsensusQuorum { required: r1, received: v1 },
            Error::ConsensusQuorum { required: r2, received: v2 },
        ) => r1 == r2 && v1 == v2,
        (
            Error::ViewChange { current_view: c1, new_view: n1 },
            Error::ViewChange { current_view: c2, new_view: n2 },
        ) => c1 == c2 && n1 == n2,
        (
            Error::TensorValidation { dimension: d1, value: v1 },
            Error::TensorValidation { dimension: d2, value: v2 },
        ) => d1 == d2 && v1.to_bits() == v2.to_bits(),
        (
            Error::EscalationRequired { from_tier: f1, to_tier: t1, reason: r1 },
            Error::EscalationRequired { from_tier: f2, to_tier: t2, reason: r2 },
        ) => f1 == f2 && t1 == t2 && r1 == r2,
        (
            Error::Timeout { operation: o1, timeout_ms: t1 },
            Error::Timeout { operation: o2, timeout_ms: t2 },
        ) => o1 == o2 && t1 == t2,
        (Error::Io(a), Error::Io(b)) => a.kind() == b.kind() && a.to_string() == b.to_string(),
        _ => false,
    }
}

impl Eq for Error {}

// ============================================================================
// Display
// ============================================================================

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Config(msg) => write!(f, "Configuration error: {msg}"),
            Self::Database(msg) => write!(f, "Database error: {msg}"),
            Self::Network { target, message } => {
                write!(f, "Network error to {target}: {message}")
            }
            Self::CircuitOpen {
                service_id,
                retry_after_ms,
            } => {
                write!(
                    f,
                    "Circuit breaker open for {service_id}, retry after {retry_after_ms}ms"
                )
            }
            Self::ConsensusQuorum { required, received } => {
                write!(f, "Consensus quorum not reached: {received}/{required}")
            }
            Self::ViewChange {
                current_view,
                new_view,
            } => {
                write!(f, "View change required: {current_view} -> {new_view}")
            }
            Self::PathwayNotFound { source, target } => {
                write!(f, "Hebbian pathway not found: {source} -> {target}")
            }
            Self::TensorValidation { dimension, value } => {
                write!(
                    f,
                    "Tensor validation failed: dimension {dimension} has invalid value {value}"
                )
            }
            Self::ServiceNotFound(id) => write!(f, "Service not found: {id}"),
            Self::HealthCheckFailed { service_id, reason } => {
                write!(f, "Health check failed for {service_id}: {reason}")
            }
            Self::EscalationRequired {
                from_tier,
                to_tier,
                reason,
            } => {
                write!(
                    f,
                    "Escalation required from {from_tier} to {to_tier}: {reason}"
                )
            }
            Self::Timeout {
                operation,
                timeout_ms,
            } => {
                write!(f, "Operation '{operation}' timed out after {timeout_ms}ms")
            }
            Self::Pipeline(msg) => write!(f, "Pipeline error: {msg}"),
            Self::Validation(msg) => write!(f, "Validation error: {msg}"),
            Self::Io(e) => write!(f, "IO error: {e}"),
            Self::Other(msg) => write!(f, "{msg}"),
        }
    }
}

// ============================================================================
// std::error::Error
// ============================================================================

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            _ => None,
        }
    }
}

// ============================================================================
// From impls
// ============================================================================

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<String> for Error {
    fn from(s: String) -> Self {
        Self::Other(s)
    }
}

// ============================================================================
// ErrorClassifier impl
// ============================================================================

impl ErrorClassifier for Error {
    fn is_retryable(&self) -> bool {
        match self {
            Self::Network { .. }
            | Self::CircuitOpen { .. }
            | Self::Timeout { .. }
            | Self::ConsensusQuorum { .. } => true,
            Self::Io(e) => matches!(
                e.kind(),
                std::io::ErrorKind::ConnectionRefused
                    | std::io::ErrorKind::ConnectionReset
                    | std::io::ErrorKind::TimedOut
                    | std::io::ErrorKind::Interrupted
                    | std::io::ErrorKind::WouldBlock
            ),
            Self::Database(msg) => msg.contains("locked") || msg.contains("busy"),
            Self::Config(_)
            | Self::Validation(_)
            | Self::Pipeline(_)
            | Self::PathwayNotFound { .. }
            | Self::ServiceNotFound(_)
            | Self::TensorValidation { .. }
            | Self::ViewChange { .. }
            | Self::HealthCheckFailed { .. }
            | Self::EscalationRequired { .. }
            | Self::Other(_) => false,
        }
    }

    fn is_transient(&self) -> bool {
        match self {
            Self::Network { .. }
            | Self::CircuitOpen { .. }
            | Self::Timeout { .. }
            | Self::ConsensusQuorum { .. } => true,
            Self::Io(e) => matches!(
                e.kind(),
                std::io::ErrorKind::TimedOut
                    | std::io::ErrorKind::WouldBlock
                    | std::io::ErrorKind::Interrupted
            ),
            Self::Config(_)
            | Self::Database(_)
            | Self::Validation(_)
            | Self::Pipeline(_)
            | Self::PathwayNotFound { .. }
            | Self::ServiceNotFound(_)
            | Self::TensorValidation { .. }
            | Self::ViewChange { .. }
            | Self::HealthCheckFailed { .. }
            | Self::EscalationRequired { .. }
            | Self::Other(_) => false,
        }
    }

    fn severity(&self) -> Severity {
        match self {
            Self::ViewChange { .. } | Self::EscalationRequired { .. } => Severity::Critical,

            Self::HealthCheckFailed { .. }
            | Self::Pipeline(_)
            | Self::ConsensusQuorum { .. } => Severity::High,

            Self::Network { .. }
            | Self::CircuitOpen { .. }
            | Self::Timeout { .. }
            | Self::Database(_)
            | Self::TensorValidation { .. }
            | Self::Io(_) => Severity::Medium,

            Self::Config(_)
            | Self::Validation(_)
            | Self::PathwayNotFound { .. }
            | Self::ServiceNotFound(_)
            | Self::Other(_) => Severity::Low,
        }
    }

    fn error_code(&self) -> u32 {
        match self {
            Self::Config(_) => 1000,
            Self::Database(_) => 1100,
            Self::Network { .. } => 1200,
            Self::CircuitOpen { .. } => 1201,
            Self::Timeout { .. } => 1202,
            Self::ConsensusQuorum { .. } => 1300,
            Self::ViewChange { .. } => 1301,
            Self::PathwayNotFound { .. } => 1400,
            Self::TensorValidation { .. } => 1401,
            Self::Validation(_) => 1500,
            Self::Io(_) => 1600,
            Self::Pipeline(_) => 1700,
            Self::ServiceNotFound(_) => 1800,
            Self::HealthCheckFailed { .. } => 1801,
            Self::EscalationRequired { .. } => 1802,
            Self::Other(_) => 1900,
        }
    }

    fn error_category(&self) -> &'static str {
        match self {
            Self::Config(_) => "config",
            Self::Database(_) => "database",
            Self::Network { .. } | Self::CircuitOpen { .. } | Self::Timeout { .. } => "network",
            Self::ConsensusQuorum { .. } | Self::ViewChange { .. } => "consensus",
            Self::PathwayNotFound { .. } | Self::TensorValidation { .. } => "learning",
            Self::Validation(_) => "validation",
            Self::Io(_) => "io",
            Self::Pipeline(_)
            | Self::ServiceNotFound(_)
            | Self::HealthCheckFailed { .. }
            | Self::EscalationRequired { .. }
            | Self::Other(_) => "other",
        }
    }
}

// ============================================================================
// NAM Tensor Signal (R2 HebbianRouting)
// ============================================================================

impl Error {
    /// Map this error to a 12D tensor signal for Hebbian routing (NAM R2).
    ///
    /// D6 (health) is set inversely to severity; D10 (`error_rate`) is set high;
    /// D2 (tier) is mapped from category.
    #[must_use]
    pub fn to_tensor_signal(&self) -> crate::Tensor12D {
        let health = match self.severity() {
            Severity::Low => 0.8,
            Severity::Medium => 0.5,
            Severity::High => 0.2,
            Severity::Critical => 0.0,
        };

        let tier = match self.error_category() {
            "network" => 2.0 / 6.0,
            "consensus" => 4.0 / 6.0,
            "config" | "database" | "validation" | "io" => 1.0 / 6.0,
            // "learning", "other", and anything else map to wildcard
            _ => 0.5,
        };

        let error_rate = match self.severity() {
            Severity::Low => 0.2,
            Severity::Medium => 0.5,
            Severity::High => 0.8,
            Severity::Critical => 1.0,
        };

        let mut tensor = crate::Tensor12D {
            service_id: 0.0,
            port: 0.0,
            tier,
            dependency_count: 0.0,
            agent_count: 0.0,
            protocol: 0.5,
            health_score: health,
            uptime: 0.0,
            synergy: 0.0,
            latency: 0.0,
            error_rate,
            temporal_context: 0.0,
        };
        tensor.clamp_normalize();
        tensor
    }
}

// ============================================================================
// AnnotatedError (R5 HumanAsAgent)
// ============================================================================

/// Error annotated with agent origin and confidence (NAM R5).
///
/// Wraps a base [`Error`] with provenance information — who triggered
/// the operation that failed, and how confident we are in the classification.
#[derive(Debug, Clone, PartialEq)]
pub struct AnnotatedError {
    /// The underlying error.
    pub error: Error,
    /// Agent that triggered the failing operation (if known).
    pub origin: Option<super::nam::AgentOrigin>,
    /// Confidence in the error classification.
    pub confidence: super::nam::Confidence,
}

impl AnnotatedError {
    /// Create an annotated error with default confidence (certain).
    #[must_use]
    pub const fn new(error: Error) -> Self {
        Self {
            error,
            origin: None,
            confidence: super::nam::Confidence::certain(),
        }
    }

    /// Attach agent origin to this error.
    #[must_use]
    pub fn with_origin(mut self, origin: super::nam::AgentOrigin) -> Self {
        self.origin = Some(origin);
        self
    }

    /// Set confidence in the error classification.
    #[must_use]
    pub const fn with_confidence(mut self, confidence: super::nam::Confidence) -> Self {
        self.confidence = confidence;
        self
    }
}

impl fmt::Display for AnnotatedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.error)?;
        if let Some(ref origin) = self.origin {
            write!(f, " [origin={origin}]")?;
        }
        Ok(())
    }
}

impl std::error::Error for AnnotatedError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.error)
    }
}

/// Result type alias using the unified Error type
pub type Result<T> = std::result::Result<T, Error>;

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ====================================================================
    // Display tests — every variant
    // ====================================================================

    #[test]
    fn test_display_config() {
        let err = Error::Config("invalid port".into());
        assert_eq!(err.to_string(), "Configuration error: invalid port");
    }

    #[test]
    fn test_display_database() {
        let err = Error::Database("connection refused".into());
        assert_eq!(err.to_string(), "Database error: connection refused");
    }

    #[test]
    fn test_display_network() {
        let err = Error::Network {
            target: "synthex".into(),
            message: "timeout".into(),
        };
        assert_eq!(err.to_string(), "Network error to synthex: timeout");
    }

    #[test]
    fn test_display_circuit_open() {
        let err = Error::CircuitOpen {
            service_id: "synthex".into(),
            retry_after_ms: 30000,
        };
        assert!(err.to_string().contains("Circuit breaker open"));
        assert!(err.to_string().contains("synthex"));
        assert!(err.to_string().contains("30000"));
    }

    #[test]
    fn test_display_consensus_quorum() {
        let err = Error::ConsensusQuorum {
            required: 27,
            received: 20,
        };
        assert_eq!(
            err.to_string(),
            "Consensus quorum not reached: 20/27"
        );
    }

    #[test]
    fn test_display_view_change() {
        let err = Error::ViewChange {
            current_view: 1,
            new_view: 2,
        };
        assert_eq!(err.to_string(), "View change required: 1 -> 2");
    }

    #[test]
    fn test_display_pathway_not_found() {
        let err = Error::PathwayNotFound {
            source: "M01".into(),
            target: "M02".into(),
        };
        assert_eq!(
            err.to_string(),
            "Hebbian pathway not found: M01 -> M02"
        );
    }

    #[test]
    fn test_display_tensor_validation() {
        let err = Error::TensorValidation {
            dimension: 6,
            value: 1.5,
        };
        assert!(err.to_string().contains("dimension 6"));
        assert!(err.to_string().contains("1.5"));
    }

    #[test]
    fn test_display_service_not_found() {
        let err = Error::ServiceNotFound("unknown-svc".into());
        assert_eq!(err.to_string(), "Service not found: unknown-svc");
    }

    #[test]
    fn test_display_health_check_failed() {
        let err = Error::HealthCheckFailed {
            service_id: "nais".into(),
            reason: "port unreachable".into(),
        };
        assert!(err.to_string().contains("nais"));
        assert!(err.to_string().contains("port unreachable"));
    }

    #[test]
    fn test_display_escalation_required() {
        let err = Error::EscalationRequired {
            from_tier: "L0".into(),
            to_tier: "L2".into(),
            reason: "low confidence".into(),
        };
        assert!(err.to_string().contains("L0"));
        assert!(err.to_string().contains("L2"));
        assert!(err.to_string().contains("low confidence"));
    }

    #[test]
    fn test_display_timeout() {
        let err = Error::Timeout {
            operation: "health_check".into(),
            timeout_ms: 5000,
        };
        assert_eq!(
            err.to_string(),
            "Operation 'health_check' timed out after 5000ms"
        );
    }

    #[test]
    fn test_display_pipeline() {
        let err = Error::Pipeline("stage 3 failed".into());
        assert_eq!(err.to_string(), "Pipeline error: stage 3 failed");
    }

    #[test]
    fn test_display_validation() {
        let err = Error::Validation("port must be > 0".into());
        assert_eq!(err.to_string(), "Validation error: port must be > 0");
    }

    #[test]
    fn test_display_io() {
        let err = Error::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "file missing",
        ));
        assert!(err.to_string().contains("IO error"));
        assert!(err.to_string().contains("file missing"));
    }

    #[test]
    fn test_display_other() {
        let err = Error::Other("something unexpected".into());
        assert_eq!(err.to_string(), "something unexpected");
    }

    // ====================================================================
    // source() chain verification
    // ====================================================================

    #[test]
    fn test_source_io_returns_inner() {
        let inner = std::io::Error::new(std::io::ErrorKind::NotFound, "gone");
        let err = Error::Io(inner);
        let source = std::error::Error::source(&err);
        assert!(source.is_some());
    }

    #[test]
    fn test_source_config_returns_none() {
        let err = Error::Config("bad".into());
        assert!(std::error::Error::source(&err).is_none());
    }

    #[test]
    fn test_source_network_returns_none() {
        let err = Error::Network {
            target: "x".into(),
            message: "y".into(),
        };
        assert!(std::error::Error::source(&err).is_none());
    }

    #[test]
    fn test_source_database_returns_none() {
        let err = Error::Database("fail".into());
        assert!(std::error::Error::source(&err).is_none());
    }

    // ====================================================================
    // From conversions
    // ====================================================================

    #[test]
    fn test_from_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "not found");
        let err: Error = io_err.into();
        assert!(matches!(err, Error::Io(_)));
    }

    #[test]
    fn test_from_io_error_preserves_kind() {
        let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied");
        let err: Error = io_err.into();
        if let Error::Io(ref inner) = err {
            assert_eq!(inner.kind(), std::io::ErrorKind::PermissionDenied);
        } else {
            panic!("expected Io variant");
        }
    }

    #[test]
    fn test_from_string() {
        let err: Error = "generic failure".to_string().into();
        assert_eq!(err, Error::Other("generic failure".into()));
    }

    // ====================================================================
    // ErrorClassifier trait
    // ====================================================================

    #[test]
    fn test_classifier_network_is_retryable() {
        let err = Error::Network {
            target: "svc".into(),
            message: "refused".into(),
        };
        assert!(err.is_retryable());
        assert!(err.is_transient());
    }

    #[test]
    fn test_classifier_circuit_open_is_retryable() {
        let err = Error::CircuitOpen {
            service_id: "svc".into(),
            retry_after_ms: 1000,
        };
        assert!(err.is_retryable());
        assert!(err.is_transient());
    }

    #[test]
    fn test_classifier_timeout_is_retryable() {
        let err = Error::Timeout {
            operation: "query".into(),
            timeout_ms: 5000,
        };
        assert!(err.is_retryable());
        assert!(err.is_transient());
    }

    #[test]
    fn test_classifier_config_not_retryable() {
        let err = Error::Config("bad".into());
        assert!(!err.is_retryable());
        assert!(!err.is_transient());
    }

    #[test]
    fn test_classifier_validation_not_retryable() {
        let err = Error::Validation("invalid".into());
        assert!(!err.is_retryable());
        assert!(!err.is_transient());
    }

    #[test]
    fn test_classifier_io_retryable_for_connection_refused() {
        let err = Error::Io(std::io::Error::new(
            std::io::ErrorKind::ConnectionRefused,
            "refused",
        ));
        assert!(err.is_retryable());
    }

    #[test]
    fn test_classifier_io_not_retryable_for_not_found() {
        let err = Error::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "missing",
        ));
        assert!(!err.is_retryable());
    }

    #[test]
    fn test_classifier_database_locked_is_retryable() {
        let err = Error::Database("database is locked".into());
        assert!(err.is_retryable());
    }

    #[test]
    fn test_classifier_database_generic_not_retryable() {
        let err = Error::Database("constraint violation".into());
        assert!(!err.is_retryable());
    }

    #[test]
    fn test_severity_critical() {
        assert_eq!(
            Error::ViewChange {
                current_view: 1,
                new_view: 2
            }
            .severity(),
            Severity::Critical
        );
        assert_eq!(
            Error::EscalationRequired {
                from_tier: "L0".into(),
                to_tier: "L3".into(),
                reason: "critical".into()
            }
            .severity(),
            Severity::Critical
        );
    }

    #[test]
    fn test_severity_high() {
        assert_eq!(
            Error::HealthCheckFailed {
                service_id: "svc".into(),
                reason: "down".into()
            }
            .severity(),
            Severity::High
        );
        assert_eq!(Error::Pipeline("fail".into()).severity(), Severity::High);
        assert_eq!(
            Error::ConsensusQuorum {
                required: 27,
                received: 10
            }
            .severity(),
            Severity::High
        );
    }

    #[test]
    fn test_severity_medium() {
        assert_eq!(
            Error::Network {
                target: "x".into(),
                message: "y".into()
            }
            .severity(),
            Severity::Medium
        );
        assert_eq!(
            Error::Timeout {
                operation: "op".into(),
                timeout_ms: 100
            }
            .severity(),
            Severity::Medium
        );
        assert_eq!(Error::Database("err".into()).severity(), Severity::Medium);
    }

    #[test]
    fn test_severity_low() {
        assert_eq!(Error::Config("err".into()).severity(), Severity::Low);
        assert_eq!(Error::Validation("err".into()).severity(), Severity::Low);
        assert_eq!(
            Error::ServiceNotFound("x".into()).severity(),
            Severity::Low
        );
        assert_eq!(Error::Other("err".into()).severity(), Severity::Low);
    }

    // ====================================================================
    // Clone behavior
    // ====================================================================

    #[test]
    fn test_clone_string_variant() {
        let err = Error::Config("test".into());
        let cloned = err.clone();
        assert_eq!(err, cloned);
    }

    #[test]
    fn test_clone_struct_variant() {
        let err = Error::Network {
            target: "svc".into(),
            message: "fail".into(),
        };
        let cloned = err.clone();
        assert_eq!(err, cloned);
    }

    #[test]
    fn test_clone_io_variant() {
        let err = Error::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "missing",
        ));
        let cloned = err.clone();
        assert_eq!(err, cloned);
    }

    #[test]
    fn test_clone_numeric_variant() {
        let err = Error::ConsensusQuorum {
            required: 27,
            received: 15,
        };
        let cloned = err.clone();
        assert_eq!(err, cloned);
    }

    // ====================================================================
    // PartialEq behavior
    // ====================================================================

    #[test]
    fn test_eq_same_variant_same_data() {
        assert_eq!(
            Error::Config("x".into()),
            Error::Config("x".into())
        );
    }

    #[test]
    fn test_neq_same_variant_different_data() {
        assert_ne!(
            Error::Config("x".into()),
            Error::Config("y".into())
        );
    }

    #[test]
    fn test_neq_different_variants() {
        assert_ne!(
            Error::Config("x".into()),
            Error::Database("x".into())
        );
    }

    #[test]
    fn test_eq_tensor_nan() {
        // NaN equality by bit comparison
        let err1 = Error::TensorValidation {
            dimension: 0,
            value: f64::NAN,
        };
        let err2 = Error::TensorValidation {
            dimension: 0,
            value: f64::NAN,
        };
        assert_eq!(err1, err2);
    }

    #[test]
    fn test_eq_circuit_open() {
        let err1 = Error::CircuitOpen {
            service_id: "svc".into(),
            retry_after_ms: 5000,
        };
        let err2 = Error::CircuitOpen {
            service_id: "svc".into(),
            retry_after_ms: 5000,
        };
        assert_eq!(err1, err2);
    }

    // ====================================================================
    // Error variant pattern matching
    // ====================================================================

    #[test]
    fn test_match_config() {
        let err = Error::Config("bad port".into());
        assert!(matches!(err, Error::Config(ref msg) if msg.contains("port")));
    }

    #[test]
    fn test_match_network_target() {
        let err = Error::Network {
            target: "synthex".into(),
            message: "refused".into(),
        };
        assert!(matches!(err, Error::Network { ref target, .. } if target == "synthex"));
    }

    #[test]
    fn test_match_timeout_operation() {
        let err = Error::Timeout {
            operation: "query".into(),
            timeout_ms: 1000,
        };
        assert!(matches!(err, Error::Timeout { timeout_ms, .. } if timeout_ms == 1000));
    }

    #[test]
    fn test_match_escalation_tiers() {
        let err = Error::EscalationRequired {
            from_tier: "L1".into(),
            to_tier: "L3".into(),
            reason: "critical".into(),
        };
        assert!(matches!(err, Error::EscalationRequired { ref from_tier, ref to_tier, .. }
            if from_tier == "L1" && to_tier == "L3"));
    }

    #[test]
    fn test_match_quorum_values() {
        let err = Error::ConsensusQuorum {
            required: 27,
            received: 20,
        };
        assert!(matches!(err, Error::ConsensusQuorum { required: 27, received: 20 }));
    }

    // ====================================================================
    // Edge cases
    // ====================================================================

    #[test]
    fn test_empty_string_config() {
        let err = Error::Config(String::new());
        assert_eq!(err.to_string(), "Configuration error: ");
    }

    #[test]
    fn test_empty_string_other() {
        let err = Error::Other(String::new());
        assert_eq!(err.to_string(), "");
    }

    #[test]
    fn test_max_u64_retry_after() {
        let err = Error::CircuitOpen {
            service_id: "svc".into(),
            retry_after_ms: u64::MAX,
        };
        assert!(err.to_string().contains(&u64::MAX.to_string()));
    }

    #[test]
    fn test_max_u32_quorum() {
        let err = Error::ConsensusQuorum {
            required: u32::MAX,
            received: 0,
        };
        assert!(err.to_string().contains(&u32::MAX.to_string()));
    }

    #[test]
    fn test_zero_dimension_tensor() {
        let err = Error::TensorValidation {
            dimension: 0,
            value: 0.0,
        };
        assert!(err.to_string().contains("dimension 0"));
    }

    #[test]
    fn test_max_dimension_tensor() {
        let err = Error::TensorValidation {
            dimension: 11,
            value: -1.0,
        };
        assert!(err.to_string().contains("dimension 11"));
    }

    #[test]
    fn test_infinity_tensor_value() {
        let err = Error::TensorValidation {
            dimension: 6,
            value: f64::INFINITY,
        };
        assert!(err.to_string().contains("inf"));
    }

    #[test]
    fn test_nan_tensor_value() {
        let err = Error::TensorValidation {
            dimension: 6,
            value: f64::NAN,
        };
        assert!(err.to_string().contains("NaN"));
    }

    #[test]
    fn test_long_string_error() {
        let long_msg = "x".repeat(10_000);
        let err = Error::Config(long_msg.clone());
        assert!(err.to_string().contains(&long_msg));
    }

    #[test]
    fn test_unicode_string_error() {
        let err = Error::Other("Ошибка конфигурации 🔥".into());
        assert_eq!(err.to_string(), "Ошибка конфигурации 🔥");
    }

    // ====================================================================
    // Severity ordering
    // ====================================================================

    #[test]
    fn test_severity_ordering() {
        assert!(Severity::Low < Severity::Medium);
        assert!(Severity::Medium < Severity::High);
        assert!(Severity::High < Severity::Critical);
    }

    #[test]
    fn test_severity_display() {
        assert_eq!(Severity::Low.to_string(), "LOW");
        assert_eq!(Severity::Medium.to_string(), "MEDIUM");
        assert_eq!(Severity::High.to_string(), "HIGH");
        assert_eq!(Severity::Critical.to_string(), "CRITICAL");
    }

    // ====================================================================
    // Consensus quorum transient
    // ====================================================================

    #[test]
    fn test_consensus_quorum_is_transient() {
        let err = Error::ConsensusQuorum {
            required: 27,
            received: 20,
        };
        assert!(err.is_transient());
        assert!(err.is_retryable());
    }

    // ====================================================================
    // Collect errors in Vec
    // ====================================================================

    #[test]
    fn test_errors_in_collection() {
        let errors: Vec<Error> = vec![
            Error::Config("a".into()),
            Error::Database("b".into()),
            Error::Other("c".into()),
        ];
        assert_eq!(errors.len(), 3);
        let cloned = errors.clone();
        assert_eq!(errors, cloned);
    }

    // ====================================================================
    // NAM: error_code() tests
    // ====================================================================

    #[test]
    fn test_error_code_ranges() {
        assert_eq!(Error::Config("x".into()).error_code(), 1000);
        assert_eq!(Error::Database("x".into()).error_code(), 1100);
        assert_eq!(
            Error::Network {
                target: "x".into(),
                message: "y".into()
            }
            .error_code(),
            1200
        );
        assert_eq!(
            Error::ConsensusQuorum {
                required: 27,
                received: 10
            }
            .error_code(),
            1300
        );
        assert_eq!(
            Error::PathwayNotFound {
                source: "a".into(),
                target: "b".into()
            }
            .error_code(),
            1400
        );
        assert_eq!(Error::Validation("x".into()).error_code(), 1500);
    }

    #[test]
    fn test_error_code_io_pipeline_other() {
        let io_err = Error::Io(std::io::Error::new(std::io::ErrorKind::NotFound, "x"));
        assert_eq!(io_err.error_code(), 1600);
        assert_eq!(Error::Pipeline("x".into()).error_code(), 1700);
        assert_eq!(Error::Other("x".into()).error_code(), 1900);
    }

    #[test]
    fn test_error_code_service_related() {
        assert_eq!(Error::ServiceNotFound("x".into()).error_code(), 1800);
        assert_eq!(
            Error::HealthCheckFailed {
                service_id: "x".into(),
                reason: "y".into()
            }
            .error_code(),
            1801
        );
        assert_eq!(
            Error::EscalationRequired {
                from_tier: "L0".into(),
                to_tier: "L3".into(),
                reason: "critical".into()
            }
            .error_code(),
            1802
        );
    }

    // ====================================================================
    // NAM: error_category() tests
    // ====================================================================

    #[test]
    fn test_error_category_mapping() {
        assert_eq!(Error::Config("x".into()).error_category(), "config");
        assert_eq!(Error::Database("x".into()).error_category(), "database");
        assert_eq!(
            Error::Network {
                target: "x".into(),
                message: "y".into()
            }
            .error_category(),
            "network"
        );
        assert_eq!(
            Error::ConsensusQuorum {
                required: 27,
                received: 10
            }
            .error_category(),
            "consensus"
        );
        assert_eq!(
            Error::PathwayNotFound {
                source: "a".into(),
                target: "b".into()
            }
            .error_category(),
            "learning"
        );
        assert_eq!(Error::Validation("x".into()).error_category(), "validation");
    }

    #[test]
    fn test_error_category_other_group() {
        assert_eq!(Error::Other("x".into()).error_category(), "other");
        assert_eq!(Error::ServiceNotFound("x".into()).error_category(), "other");
        assert_eq!(Error::Pipeline("x".into()).error_category(), "other");
    }

    // ====================================================================
    // NAM: to_tensor_signal() tests
    // ====================================================================

    #[test]
    fn test_tensor_signal_dims_in_range() {
        let errors = [
            Error::Config("x".into()),
            Error::Database("x".into()),
            Error::Network {
                target: "x".into(),
                message: "y".into(),
            },
            Error::ConsensusQuorum {
                required: 27,
                received: 10,
            },
            Error::ViewChange {
                current_view: 1,
                new_view: 2,
            },
        ];
        for err in &errors {
            let tensor = err.to_tensor_signal();
            assert!(tensor.validate().is_ok(), "tensor invalid for {err:?}");
        }
    }

    #[test]
    fn test_tensor_signal_severity_maps_health() {
        let low = Error::Config("x".into()).to_tensor_signal();
        let critical = Error::ViewChange {
            current_view: 1,
            new_view: 2,
        }
        .to_tensor_signal();
        assert!(
            low.health_score > critical.health_score,
            "Low severity should have higher health"
        );
    }

    #[test]
    fn test_tensor_signal_severity_maps_error_rate() {
        let low = Error::Config("x".into()).to_tensor_signal();
        let critical = Error::ViewChange {
            current_view: 1,
            new_view: 2,
        }
        .to_tensor_signal();
        assert!(
            critical.error_rate > low.error_rate,
            "Critical severity should have higher error_rate"
        );
    }

    // ====================================================================
    // NAM: AnnotatedError tests
    // ====================================================================

    #[test]
    fn test_annotated_error_construction() {
        let annotated = AnnotatedError::new(Error::Config("bad".into()));
        assert_eq!(annotated.error, Error::Config("bad".into()));
        assert!(annotated.origin.is_none());
        assert_eq!(
            annotated.confidence,
            super::super::nam::Confidence::certain()
        );
    }

    #[test]
    fn test_annotated_error_with_origin() {
        let annotated = AnnotatedError::new(Error::Database("down".into()))
            .with_origin(super::super::nam::AgentOrigin::human());
        assert!(annotated.origin.is_some());
        assert_eq!(
            annotated.origin,
            Some(super::super::nam::AgentOrigin::human())
        );
    }

    #[test]
    fn test_annotated_error_with_confidence() {
        let conf = super::super::nam::Confidence::uncertain();
        let annotated = AnnotatedError::new(Error::Other("x".into())).with_confidence(conf);
        assert_eq!(annotated.confidence, super::super::nam::Confidence::uncertain());
    }

    #[test]
    fn test_annotated_error_display() {
        let annotated = AnnotatedError::new(Error::Config("bad port".into()))
            .with_origin(super::super::nam::AgentOrigin::human());
        let display = annotated.to_string();
        assert!(display.contains("Configuration error: bad port"));
        assert!(display.contains("[origin=Human(@0.A)]"));
    }

    #[test]
    fn test_annotated_error_display_no_origin() {
        let annotated = AnnotatedError::new(Error::Other("fail".into()));
        let display = annotated.to_string();
        assert_eq!(display, "fail");
        assert!(!display.contains("[origin="));
    }
}
