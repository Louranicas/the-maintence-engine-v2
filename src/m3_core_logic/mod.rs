//! # Layer 3: Core Logic
//!
//! Core business logic including remediation pipelines and
//! decision-making components.
//!
//! ## Modules
//!
//! | Module | Name | Purpose |
//! |--------|------|---------|
//! | M13 | Pipeline Manager | Pipeline orchestration |
//! | M14 | Remediation Engine | Auto-remediation logic |
//! | M15 | Confidence Calculator | Action confidence scoring |
//! | M16 | Action Executor | Action execution |
//! | M17 | Outcome Recorder | Outcome tracking |
//! | M18 | Feedback Loop | Learning feedback |
//!
//! ## Related Documentation
//! - [Layer Specification](../../ai_docs/layers/L03_CORE_LOGIC.md)
//! - [Auto-Remediation](../../nam/L0_AUTO_REMEDIATION.md)

pub mod action;
pub mod confidence;
pub mod feedback;
pub mod outcome;
pub mod pipeline;
pub mod remediation;

use crate::EscalationTier;

/// Remediation action types representing various corrective measures
/// that can be taken in response to detected issues.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RemediationAction {
    /// Retry with exponential backoff.
    ///
    /// Used for transient failures where retrying after increasing delays
    /// may succeed.
    RetryWithBackoff {
        /// Maximum number of retry attempts before giving up.
        max_retries: u32,
        /// Initial delay in milliseconds between retries (doubles each attempt).
        initial_delay_ms: u64,
    },
    /// Reset circuit breaker.
    ///
    /// Used to restore service connectivity after a circuit breaker has tripped.
    CircuitBreakerReset {
        /// The ID of the service whose circuit breaker to reset.
        service_id: String,
    },
    /// Restart service.
    ///
    /// Used when a service is in an unrecoverable state and needs a fresh start.
    ServiceRestart {
        /// The ID of the service to restart.
        service_id: String,
        /// Whether to perform a graceful shutdown (true) or force kill (false).
        graceful: bool,
    },
    /// Graceful degradation.
    ///
    /// Reduces service functionality to maintain availability under stress.
    GracefulDegradation {
        /// The ID of the service to degrade.
        service_id: String,
        /// Degradation level (0 = full, 1-255 = progressively reduced).
        level: u8,
    },
    /// Fallback to cached data.
    ///
    /// Returns stale cached data when the primary data source is unavailable.
    FallbackToCached {
        /// Cache key to retrieve fallback data from.
        key: String,
        /// Time-to-live in seconds for accepting stale cache entries.
        ttl_seconds: u64,
    },
    /// Cache cleanup.
    ///
    /// Evicts cache entries to free memory when under pressure.
    CacheCleanup {
        /// The ID of the service whose cache to clean.
        service_id: String,
        /// Percentage of cache to clear (0-100).
        threshold_percent: u8,
    },
    /// Session rotation.
    ///
    /// Rotates session credentials or tokens for security purposes.
    SessionRotation {
        /// The ID of the session to rotate.
        session_id: String,
    },
    /// Database vacuum.
    ///
    /// Reclaims disk space and optimizes database performance.
    /// Requires PBFT consensus due to potential service disruption.
    DatabaseVacuum {
        /// The database file or connection name to vacuum.
        database: String,
    },
    /// Alert human operator.
    ///
    /// Escalates to human intervention when automated remediation is insufficient.
    AlertHuman {
        /// The alert message describing the issue.
        message: String,
        /// Severity level of the alert (e.g., "low", "medium", "high", "critical").
        severity: String,
    },
}

/// Remediation request
#[derive(Clone, Debug)]
pub struct RemediationRequest {
    /// Unique request ID
    pub id: String,
    /// Target service
    pub service_id: String,
    /// Issue type
    pub issue_type: IssueType,
    /// Severity level
    pub severity: Severity,
    /// Confidence score (0.0 - 1.0)
    pub confidence: f64,
    /// Suggested action
    pub suggested_action: RemediationAction,
    /// Escalation tier
    pub tier: EscalationTier,
    /// Context data
    pub context: std::collections::HashMap<String, String>,
}

/// Issue type classification
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum IssueType {
    /// Health check failure
    HealthFailure,
    /// Latency spike
    LatencySpike,
    /// High error rate
    ErrorRateHigh,
    /// Memory pressure
    MemoryPressure,
    /// Disk pressure
    DiskPressure,
    /// Connection failure
    ConnectionFailure,
    /// Request timeout
    Timeout,
    /// Service crash
    Crash,
}

/// Severity levels
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    /// Low severity
    Low,
    /// Medium severity
    Medium,
    /// High severity
    High,
    /// Critical severity
    Critical,
}

impl Severity {
    /// Get the normalized severity score (0.0 - 1.0).
    ///
    /// Maps each severity level to a score suitable for the confidence formula:
    /// - Low: 0.25
    /// - Medium: 0.5
    /// - High: 0.75
    /// - Critical: 1.0
    #[must_use]
    pub const fn score(&self) -> f64 {
        match self {
            Self::Low => 0.25,
            Self::Medium => 0.5,
            Self::High => 0.75,
            Self::Critical => 1.0,
        }
    }
}

impl IssueType {
    /// Get a string representation of the issue type for use as a map key.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::HealthFailure => "health_failure",
            Self::LatencySpike => "latency_spike",
            Self::ErrorRateHigh => "error_rate_high",
            Self::MemoryPressure => "memory_pressure",
            Self::DiskPressure => "disk_pressure",
            Self::ConnectionFailure => "connection_failure",
            Self::Timeout => "timeout",
            Self::Crash => "crash",
        }
    }
}

/// Remediation outcome
#[derive(Clone, Debug)]
pub struct RemediationOutcome {
    /// Request ID
    pub request_id: String,
    /// Success flag
    pub success: bool,
    /// Duration in milliseconds
    pub duration_ms: u64,
    /// Error message (if failed)
    pub error: Option<String>,
    /// Hebbian pathway delta
    pub pathway_delta: f64,
}

/// Calculate confidence score for a remediation action
#[must_use]
pub fn calculate_confidence(
    historical_success_rate: f64,
    pattern_match_strength: f64,
    severity_score: f64,
    pathway_weight: f64,
    time_factor: f64,
) -> f64 {
    // Weighted confidence formula from implementation plan
    let confidence = 0.3f64
        .mul_add(historical_success_rate, 0.25f64.mul_add(
            pattern_match_strength,
            0.2f64.mul_add(severity_score, 0.15f64.mul_add(pathway_weight, 0.1 * time_factor)),
        ));

    confidence.clamp(0.0, 1.0)
}

/// Determine escalation tier based on confidence and severity
#[must_use]
pub fn determine_tier(confidence: f64, severity: Severity, action: &RemediationAction) -> EscalationTier {
    // L3 PBFT consensus required for critical actions
    if matches!(
        action,
        RemediationAction::ServiceRestart { graceful: false, .. }
            | RemediationAction::DatabaseVacuum { .. }
    ) {
        return EscalationTier::L3PbftConsensus;
    }

    // L0 auto-execute for high confidence, low severity
    if confidence >= 0.9 && severity <= Severity::Medium {
        return EscalationTier::L0AutoExecute;
    }

    // L1 notify for moderate confidence
    if confidence >= 0.7 && severity <= Severity::High {
        return EscalationTier::L1NotifyHuman;
    }

    // L2 require approval for low confidence or high severity
    EscalationTier::L2RequireApproval
}

/// Pipeline stage
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PipelineStage {
    /// Data source
    Source,
    /// Entry point with validation
    Ingress,
    /// Processing stages
    Transform,
    /// Conditional routing
    Route,
    /// Destination
    Sink,
    /// Result feedback
    Feedback,
}

/// Pipeline definition
#[derive(Clone, Debug)]
pub struct Pipeline {
    /// Pipeline ID
    pub id: String,
    /// Pipeline name
    pub name: String,
    /// Priority (1-10, 1 = highest)
    pub priority: u8,
    /// Latency SLO in milliseconds
    pub latency_slo_ms: u64,
    /// Throughput target (events/sec)
    pub throughput_target: u64,
    /// Error budget (0.0 - 1.0)
    pub error_budget: f64,
    /// Enabled flag
    pub enabled: bool,
}

impl Pipeline {
    /// Create a new pipeline
    #[must_use]
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            priority: 5,
            latency_slo_ms: 100,
            throughput_target: 1000,
            error_budget: 0.01,
            enabled: true,
        }
    }
}

/// Default pipelines from the implementation plan
#[must_use]
pub fn default_pipelines() -> Vec<Pipeline> {
    vec![
        Pipeline {
            id: "PL-HEALTH-001".into(),
            name: "Health Monitoring Pipeline".into(),
            priority: 1,
            latency_slo_ms: 100,
            throughput_target: 1000,
            error_budget: 0.001,
            enabled: true,
        },
        Pipeline {
            id: "PL-LOG-001".into(),
            name: "Log Processing Pipeline".into(),
            priority: 2,
            latency_slo_ms: 50,
            throughput_target: 500_000,
            error_budget: 0.005,
            enabled: true,
        },
        Pipeline {
            id: "PL-REMEDIATE-001".into(),
            name: "Auto-Remediation Pipeline".into(),
            priority: 1,
            latency_slo_ms: 500,
            throughput_target: 100,
            error_budget: 0.0001,
            enabled: true,
        },
        Pipeline {
            id: "PL-HEBBIAN-001".into(),
            name: "Neural Pathway Learning Pipeline".into(),
            priority: 2,
            latency_slo_ms: 100,
            throughput_target: 10_000,
            error_budget: 0.01,
            enabled: true,
        },
        Pipeline {
            id: "PL-CONSENSUS-001".into(),
            name: "PBFT Consensus Pipeline".into(),
            priority: 1,
            latency_slo_ms: 5000,
            throughput_target: 10,
            error_budget: 0.0001,
            enabled: true,
        },
        Pipeline {
            id: "PL-TENSOR-001".into(),
            name: "Tensor Encoding Pipeline".into(),
            priority: 3,
            latency_slo_ms: 10,
            throughput_target: 100_000,
            error_budget: 0.005,
            enabled: true,
        },
        Pipeline {
            id: "PL-DISCOVERY-001".into(),
            name: "Service Discovery Pipeline".into(),
            priority: 2,
            latency_slo_ms: 1000,
            throughput_target: 100,
            error_budget: 0.01,
            enabled: true,
        },
        Pipeline {
            id: "PL-METRICS-001".into(),
            name: "Metrics Aggregation Pipeline".into(),
            priority: 3,
            latency_slo_ms: 200,
            throughput_target: 50_000,
            error_budget: 0.01,
            enabled: true,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_confidence_calculation() {
        let confidence = calculate_confidence(0.9, 0.8, 0.6, 0.7, 0.5);
        assert!(confidence > 0.0 && confidence <= 1.0);
    }

    #[test]
    fn test_tier_determination() {
        let action = RemediationAction::CacheCleanup {
            service_id: "test".into(),
            threshold_percent: 80,
        };

        // High confidence, low severity -> L0
        let tier = determine_tier(0.95, Severity::Low, &action);
        assert_eq!(tier, EscalationTier::L0AutoExecute);

        // Low confidence -> L2
        let tier = determine_tier(0.5, Severity::Low, &action);
        assert_eq!(tier, EscalationTier::L2RequireApproval);

        // Database vacuum always L3
        let vacuum = RemediationAction::DatabaseVacuum {
            database: "test.db".into(),
        };
        let tier = determine_tier(0.99, Severity::Low, &vacuum);
        assert_eq!(tier, EscalationTier::L3PbftConsensus);
    }

    #[test]
    fn test_default_pipelines() {
        let pipelines = default_pipelines();
        assert_eq!(pipelines.len(), 8);
        assert!(pipelines.iter().any(|p| p.id == "PL-HEALTH-001"));
    }
}
