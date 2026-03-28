# Module M14: Remediation Engine

> **M14_REMEDIATION_ENGINE** | Auto-Remediation Logic | Layer: L3 Core Logic | [Back to Index](../INDEX.md)

---

## Navigation

| Direction | Link |
|-----------|------|
| Up | [INDEX.md](../INDEX.md) |
| Layer | [L03_CORE_LOGIC.md](../layers/L03_CORE_LOGIC.md) |
| Related | [M13_PIPELINE_MANAGER.md](M13_PIPELINE_MANAGER.md) |
| Related | [M15_CONFIDENCE_CALCULATOR.md](M15_CONFIDENCE_CALCULATOR.md) |
| Related | [M16_ACTION_EXECUTOR.md](M16_ACTION_EXECUTOR.md) |
| Pattern | [PATTERN_REMEDIATION.md](../patterns/PATTERN_REMEDIATION.md) |
| L2 Services | [L02_SERVICES.md](../layers/L02_SERVICES.md) |
| L4 Integration | [L04_INTEGRATION.md](../layers/L04_INTEGRATION.md) |

---

## Module Specification

### Overview

The Remediation Engine module implements auto-remediation logic for service issues. It processes remediation requests, determines appropriate remediation actions based on issue type and severity, and orchestrates remediation execution with escalation tier management. Supports 9 distinct remediation action types covering retry strategies, circuit breaker resets, service restarts, graceful degradation, caching strategies, session management, and database maintenance.

### Module Properties

| Property | Value |
|----------|-------|
| Module ID | M14 |
| Module Name | Remediation Engine |
| Layer | L3 (Core Logic) |
| Version | 1.0.0 |
| Dependencies | M02 (Config), M13 (Pipeline), M15 (Confidence) |
| Dependents | M16 (Executor), M17 (Recorder), M18 (Feedback), M19-M24 (Integration) |

---

## Architecture Diagram

```
+-----------------------------------------------------------------------------------+
|                    M14: REMEDIATION ENGINE                                      |
+-----------------------------------------------------------------------------------+
|                                                                                   |
|  +------------------------+    +------------------------+    +--------------+   |
|  | REMEDIATION CLASSIFIER |    | ACTION DETERMINER      |    | ESCALATOR    |   |
|  |                        |    |                        |    |              |   |
|  | - Issue type analysis  |    | - Pattern matching     |    | - L0 Auto    |   |
|  | - Severity assessment  |--->| - Action selection     |--->| - L1 Notify  |   |
|  | - Context extraction   |    | - Parameter binding    |    | - L2 Approve |   |
|  +--------+----------------+    +--------+---------------+    | - L3 PBFT    |   |
|           |                              |                   +--------------+   |
|           v                              v                            |          |
|  +------------------------+    +------------------------+          |          |
|  |  REQUEST QUEUE         |    |  EXECUTION SCHEDULER   |          v          |
|  |                        |    |                        |    +--------------+   |
|  | - Remediation requests |    | - Priority ordering    |    | EVENT STREAM |   |
|  | - Deduplication        |    | - Rate limiting        |    |              |   |
|  | - Timeout tracking     |    | - Concurrent execution |    | -> M16 Exec  |   |
|  | - Persistence          |    | - Rollback management  |    | -> M17 Record|   |
|  +------------------------+    +------------------------+    | -> M18 Learn |   |
|                                                                +--------------+   |
+-----------------------------------------------------------------------------------+
        ^                    ^                    ^                    |
        |                    |                    |                    v
   [L1: Config]        [L2: Health]        [L3: Confidence]    [L3/L4: Services]
```

---

## Core Data Structures

### Remediation Action Types

```rust
/// Enumeration of all supported remediation actions
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RemediationAction {
    /// Retry with exponential backoff strategy
    RetryWithBackoff {
        /// Maximum retry attempts
        max_retries: u32,
        /// Initial delay in milliseconds
        initial_delay_ms: u64,
    },

    /// Reset circuit breaker for a service
    CircuitBreakerReset {
        /// Target service ID
        service_id: String,
    },

    /// Restart a service (graceful or forced)
    ServiceRestart {
        /// Target service ID
        service_id: String,
        /// Use graceful shutdown (true) or force kill (false)
        graceful: bool,
    },

    /// Graceful degradation to reduce load
    GracefulDegradation {
        /// Target service ID
        service_id: String,
        /// Degradation level (0-10, higher = more severe)
        level: u8,
    },

    /// Fallback to cached data
    FallbackToCached {
        /// Cache key
        key: String,
        /// TTL in seconds
        ttl_seconds: u64,
    },

    /// Cache cleanup/eviction
    CacheCleanup {
        /// Target service ID
        service_id: String,
        /// Cleanup threshold in percent
        threshold_percent: u8,
    },

    /// Session rotation for connection pools
    SessionRotation {
        /// Session ID to rotate
        session_id: String,
    },

    /// Database maintenance operation
    DatabaseVacuum {
        /// Target database name
        database: String,
    },

    /// Alert human operator
    AlertHuman {
        /// Alert message
        message: String,
        /// Severity level (LOW, MEDIUM, HIGH, CRITICAL)
        severity: String,
    },
}
```

### Issue Type Classification

```rust
/// Classification of system issues requiring remediation
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IssueType {
    /// Health check failure
    HealthFailure,

    /// Latency spike above thresholds
    LatencySpike,

    /// High error rate exceeding budget
    ErrorRateHigh,

    /// Memory pressure/high usage
    MemoryPressure,

    /// Disk space pressure/high usage
    DiskPressure,

    /// Service connection failure
    ConnectionFailure,

    /// Request timeout
    Timeout,

    /// Service crash/unexpected termination
    Crash,
}
```

### Severity Levels

```rust
/// Severity classification for issues
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    /// Low severity issues
    Low,

    /// Medium severity issues
    Medium,

    /// High severity issues (service degradation)
    High,

    /// Critical severity issues (service down)
    Critical,
}
```

### Remediation Request

```rust
/// Request for remediation action on a service issue
#[derive(Clone, Debug)]
pub struct RemediationRequest {
    /// Unique request identifier
    pub id: String,

    /// Target service ID
    pub service_id: String,

    /// Type of issue detected
    pub issue_type: IssueType,

    /// Severity level of the issue
    pub severity: Severity,

    /// Confidence score (0.0 - 1.0)
    pub confidence: f64,

    /// Suggested remediation action
    pub suggested_action: RemediationAction,

    /// Escalation tier determined by confidence/severity
    pub tier: EscalationTier,

    /// Context data for the issue
    pub context: std::collections::HashMap<String, String>,
}
```

### Remediation Outcome

```rust
/// Result of a remediation action execution
#[derive(Clone, Debug)]
pub struct RemediationOutcome {
    /// Original request ID
    pub request_id: String,

    /// Whether remediation succeeded
    pub success: bool,

    /// Duration of remediation in milliseconds
    pub duration_ms: u64,

    /// Error message if remediation failed
    pub error: Option<String>,

    /// Hebbian pathway weight change from this remediation
    pub pathway_delta: f64,
}
```

---

## Public API

### Remediation Engine Service

```rust
/// Main Remediation Engine service
pub struct RemediationEngine {
    config: RemediationEngineConfig,
    classifier: IssueClassifier,
    determiner: ActionDeterminer,
    escalator: TierEscalator,
    queue: RemediationQueue,
    scheduler: ExecutionScheduler,
    event_emitter: EventEmitter<RemediationEvent>,
}

impl RemediationEngine {
    /// Create a new RemediationEngine instance
    pub fn new(config: RemediationEngineConfig) -> Self;

    /// Submit a remediation request
    pub async fn submit_request(&mut self, request: RemediationRequest) -> Result<String, Error>;

    /// Get request status
    pub fn get_request_status(&self, request_id: &str) -> Option<RequestStatus>;

    /// Get request outcome
    pub fn get_outcome(&self, request_id: &str) -> Option<RemediationOutcome>;

    /// Cancel pending request
    pub fn cancel_request(&mut self, request_id: &str) -> Result<(), Error>;

    /// List pending requests
    pub fn list_pending(&self) -> Vec<RemediationRequest>;

    /// Get remediation history
    pub fn get_history(
        &self,
        service_id: &str,
        since: DateTime<Utc>,
        limit: usize,
    ) -> Vec<RemediationOutcome>;
}
```

### Issue Classification and Analysis

```rust
impl RemediationEngine {
    /// Classify an issue from health/metric data
    pub fn classify_issue(
        &self,
        service_id: &str,
        metrics: &SystemMetrics,
    ) -> Result<IssueType, ClassificationError>;

    /// Assess severity of an issue
    pub fn assess_severity(
        &self,
        issue_type: IssueType,
        metrics: &SystemMetrics,
    ) -> Severity;

    /// Extract context data from issue
    pub fn extract_context(
        &self,
        service_id: &str,
        issue_type: IssueType,
    ) -> HashMap<String, String>;

    /// Get recommended actions for issue
    pub fn recommend_actions(
        &self,
        issue_type: IssueType,
        severity: Severity,
    ) -> Vec<RemediationAction>;
}
```

### Action Determination

```rust
impl RemediationEngine {
    /// Determine best remediation action
    pub fn determine_action(
        &self,
        issue_type: IssueType,
        severity: Severity,
        available_actions: &[RemediationAction],
    ) -> RemediationAction;

    /// Bind parameters for an action
    pub fn bind_parameters(
        &self,
        action: &RemediationAction,
        context: &HashMap<String, String>,
    ) -> Result<RemediationAction, Error>;

    /// Validate action is safe for target service
    pub fn validate_action(
        &self,
        service_id: &str,
        action: &RemediationAction,
    ) -> Result<(), ValidationError>;
}
```

### Escalation Tier Management

```rust
impl RemediationEngine {
    /// Determine escalation tier
    pub fn determine_tier(
        &self,
        confidence: f64,
        severity: Severity,
        action: &RemediationAction,
    ) -> EscalationTier;

    /// Check if action requires PBFT consensus
    pub fn requires_consensus(&self, action: &RemediationAction) -> bool;

    /// Check if action requires human approval
    pub fn requires_approval(&self, action: &RemediationAction) -> bool;

    /// Check if action can auto-execute
    pub fn can_auto_execute(&self, tier: EscalationTier) -> bool;
}
```

---

## Escalation Tier Determination

```rust
/// Determine escalation tier based on confidence and severity
#[must_use]
pub fn determine_tier(
    confidence: f64,
    severity: Severity,
    action: &RemediationAction,
) -> EscalationTier {
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
```

---

## Configuration

### TOML Configuration Example

```toml
[layer.L3.M14]
enabled = true
version = "1.0.0"

# Global remediation settings
[layer.L3.M14.defaults]
enable_auto_remediation = true
max_concurrent_remediations = 5
default_timeout_ms = 60000
enable_rollback = true
enable_learning = true

# Queue configuration
[layer.L3.M14.queue]
max_pending_requests = 1000
deduplication_window_ms = 5000
request_retention_hours = 24
priority_levels = 5

# Escalation thresholds
[layer.L3.M14.escalation]
auto_execute_confidence_threshold = 0.9
notify_human_confidence_threshold = 0.7
require_approval_confidence_threshold = 0.5
pbft_consensus_services = ["ServiceRestart", "DatabaseVacuum"]

# Action-specific configuration
[layer.L3.M14.actions.retry_with_backoff]
enabled = true
default_max_retries = 3
default_initial_delay_ms = 100
max_delay_ms = 30000
backoff_multiplier = 2.0

[layer.L3.M14.actions.circuit_breaker_reset]
enabled = true
cooldown_ms = 5000
success_threshold = 2

[layer.L3.M14.actions.service_restart]
enabled = true
graceful_timeout_ms = 30000
force_kill_timeout_ms = 60000

[layer.L3.M14.actions.graceful_degradation]
enabled = true
min_level = 1
max_level = 10
degradation_step = 2

[layer.L3.M14.actions.fallback_to_cached]
enabled = true
default_ttl_seconds = 300
max_age_seconds = 3600

[layer.L3.M14.actions.cache_cleanup]
enabled = true
default_threshold_percent = 80
cleanup_percent = 50

[layer.L3.M14.actions.session_rotation]
enabled = true
batch_size = 10
rotation_interval_ms = 60000

[layer.L3.M14.actions.database_vacuum]
enabled = true
requires_approval = true
estimated_duration_ms = 120000

[layer.L3.M14.actions.alert_human]
enabled = true
alert_channel = "ops-channel"
notification_timeout_ms = 300000
```

---

## Bi-Directional Data Flow

### Inbound Data Flow

```rust
/// Messages received by Remediation Engine
#[derive(Debug, Clone)]
pub enum RemediationEngineInbound {
    // From M13 Pipeline Manager
    PipelineExecutionEvent {
        pipeline_id: String,
        success: bool,
        duration_ms: u64,
        timestamp: DateTime<Utc>,
    },

    // From L2 Health Monitor
    ServiceHealthEvent {
        service_id: String,
        issue_type: IssueType,
        severity: Severity,
        timestamp: DateTime<Utc>,
    },

    // From L3 Learning
    RemediationSuggestion {
        service_id: String,
        issue_type: IssueType,
        recommended_action: RemediationAction,
        confidence: f64,
    },

    // From L6 Consensus
    ApprovalEvent {
        request_id: String,
        approved: bool,
        approver_id: String,
        timestamp: DateTime<Utc>,
    },

    // From L5 Learning Pathways
    PathwayUpdate {
        issue_type: IssueType,
        action: RemediationAction,
        effectiveness: f64,
    },
}
```

### Outbound Data Flow

```rust
/// Messages emitted by Remediation Engine
#[derive(Debug, Clone)]
pub enum RemediationEngineOutbound {
    // To M16 Action Executor
    ExecutionRequest {
        request_id: String,
        action: RemediationAction,
        service_id: String,
        tier: EscalationTier,
    },

    // To M17 Outcome Recorder
    OutcomeNotification {
        request_id: String,
        outcome: RemediationOutcome,
    },

    // To M18 Feedback Loop
    FeedbackEvent {
        request_id: String,
        issue_type: IssueType,
        action: RemediationAction,
        outcome: RemediationOutcome,
    },

    // To L6 Consensus (for PBFT)
    ConsensusRequest {
        request_id: String,
        action: RemediationAction,
        severity: Severity,
        context: HashMap<String, String>,
    },

    // To subscribers
    RemediationEvent {
        request_id: String,
        service_id: String,
        status: RemediationStatus,
        action: RemediationAction,
        timestamp: DateTime<Utc>,
    },
}
```

---

## Metrics

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `me_m14_remediation_requests_total` | Counter | service, issue_type, action | Total remediation requests |
| `me_m14_remediation_success_rate` | Gauge | service, action | Success rate by service/action |
| `me_m14_remediation_duration_ms` | Histogram | action, tier | Remediation execution time |
| `me_m14_queue_length` | Gauge | tier | Pending requests in queue |
| `me_m14_escalation_total` | Counter | from_tier, to_tier | Escalation count |
| `me_m14_action_effectiveness` | Gauge | action, issue_type | Effectiveness score |
| `me_m14_consensus_requests_total` | Counter | status | PBFT consensus requests |
| `me_m14_approval_pending` | Gauge | severity | Requests awaiting approval |
| `me_m14_rollback_total` | Counter | action | Rollback events |
| `me_m14_learning_pathway_delta` | Gauge | action, issue_type | Pathway weight changes |

---

## Error Codes

| Code | Name | Severity | Description | Recovery |
|------|------|----------|-------------|----------|
| E14001 | UnknownIssueType | Warning | Issue type not recognized | Use generic remediation |
| E14002 | ClassificationFailed | Error | Cannot classify issue | Manual review required |
| E14003 | NoActionAvailable | Error | No applicable actions | Escalate to human |
| E14004 | ActionValidationFailed | Error | Action invalid for target | Select alternate action |
| E14005 | ExecutionTimeout | Error | Remediation exceeded timeout | Rollback and retry |
| E14006 | EscalationFailed | Error | Cannot escalate to next tier | Manual intervention |
| E14007 | ConsensusFailed | Critical | PBFT consensus failed | Manual decision required |
| E14008 | ApprovalTimeout | Warning | Human approval timed out | Escalate or auto-execute |
| E14009 | RollbackFailed | Critical | Cannot rollback failed action | Operational alert |
| E14010 | QueueFull | Warning | Remediation queue full | Drop lowest priority |

---

## Related Modules

- **M13_PIPELINE_MANAGER**: Triggers remediation on pipeline issues
- **M15_CONFIDENCE_CALCULATOR**: Scores remediation confidence
- **M16_ACTION_EXECUTOR**: Executes determined remediation actions
- **M17_OUTCOME_RECORDER**: Records remediation outcomes
- **M18_FEEDBACK_LOOP**: Learns from remediation effectiveness
- **L2_HEALTH_MONITOR**: Provides health metrics for issue detection
- **L5_HEBBIAN_ENGINE**: Learns action effectiveness patterns
- **L6_PBFT_CONSENSUS**: Requires consensus for critical actions

---

## Navigation

| Direction | Link |
|-----------|------|
| Up | [INDEX.md](../INDEX.md) |
| Layer | [L03_CORE_LOGIC.md](../layers/L03_CORE_LOGIC.md) |
| Previous | [M13_PIPELINE_MANAGER.md](M13_PIPELINE_MANAGER.md) |
| Next | [M15_CONFIDENCE_CALCULATOR.md](M15_CONFIDENCE_CALCULATOR.md) |
| Related | [M16_ACTION_EXECUTOR.md](M16_ACTION_EXECUTOR.md) |
| API | [REST_API.md](../integration/REST_API.md) |

---

*[Back to Index](../INDEX.md) | [Layer: L03 Core Logic](../layers/L03_CORE_LOGIC.md)*
