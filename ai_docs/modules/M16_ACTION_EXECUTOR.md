# Module M16: Action Executor

> **M16_ACTION_EXECUTOR** | Action Execution | Layer: L3 Core Logic | [Back to Index](../INDEX.md)

---

## Navigation

| Direction | Link |
|-----------|------|
| Up | [INDEX.md](../INDEX.md) |
| Layer | [L03_CORE_LOGIC.md](../layers/L03_CORE_LOGIC.md) |
| Related | [M14_REMEDIATION_ENGINE.md](M14_REMEDIATION_ENGINE.md) |
| Related | [M15_CONFIDENCE_CALCULATOR.md](M15_CONFIDENCE_CALCULATOR.md) |
| Related | [M17_OUTCOME_RECORDER.md](M17_OUTCOME_RECORDER.md) |
| Pattern | [PATTERN_EXECUTION.md](../patterns/PATTERN_EXECUTION.md) |
| L2 Services | [L02_SERVICES.md](../layers/L02_SERVICES.md) |
| L4 Integration | [L04_INTEGRATION.md](../layers/L04_INTEGRATION.md) |

---

## Module Specification

### Overview

The Action Executor module executes remediation actions determined by the Remediation Engine. It manages action execution at different escalation tiers (L0 auto-execute, L1 notify, L2 approval, L3 consensus), coordinates with ULTRAPLATE services via L4 integration bridges, and tracks execution progress with rollback capabilities.

### Module Properties

| Property | Value |
|----------|-------|
| Module ID | M16 |
| Module Name | Action Executor |
| Layer | L3 (Core Logic) |
| Version | 1.0.0 |
| Dependencies | M13 (Pipeline), M14 (Remediation), M15 (Confidence) |
| Dependents | M17 (Recorder), M18 (Feedback), L4 (Integration), L6 (Consensus) |

---

## Architecture Diagram

```
+-----------------------------------------------------------------------------------+
|                      M16: ACTION EXECUTOR                                       |
+-----------------------------------------------------------------------------------+
|                                                                                   |
|  +------------------------+    +------------------------+    +--------------+   |
|  | EXECUTION ROUTER       |    | TIER DISPATCHER        |    |VALIDATION    |   |
|  |                        |    |                        |    |              |   |
|  | - Action type dispatch |    | - L0 Auto-execute      |    | - Pre-checks |   |
|  | - Service resolution   |--->| - L1 Notify human      |--->| - Safety     |   |
|  | - Parameter binding    |    | - L2 Await approval    |    | - Dependencies
|  +--------+----------------+    | - L3 PBFT consensus    |    +--------------+   |
|           |                    +--------+---------------+            |            |
|           v                             |                           v            |
|  +------------------------+    +------------------------+    +---------+     |
|  | SERVICE BRIDGE MANAGER |    | EXECUTION MONITOR      |    |ROLLBACK |     |
|  |                        |    |                        |    |MANAGER  |     |
|  | - SYNTHEX integration  |    | - Progress tracking    |--->|         |     |
|  | - SAN-K7 integration   |    | - Timeout detection    |    | - Undo  |     |
|  | - DevOps integration   |    | - Failure handling     |    | - State |     |
|  | - Protocol translation |    | - Event emission       |    |Restore  |     |
|  +--------+----------------+    +--------+---------------+    +--+------+     |
|           |                              |                       |              |
|           +--------- Coordinate and Execute ------+              |              |
|                                              |                   v              |
|                                         +---------+    +-------------------+   |
|                                         | EXECUTION| -> | EVENT STREAM    |   |
|                                         | RESULT   |    |                 |   |
|                                         +---------+    | -> M17 Recorder |   |
|                                                        | -> M18 Feedback |   |
|                                                        +-------------------+   |
+-----------------------------------------------------------------------------------+
        ^                    ^                    ^                    |
        |                    |                    |                    v
   [M15: Confidence]    [L4: Bridges]       [L6: Consensus]     [L3 Modules]
```

---

## Core Data Structures

### Execution Request

```rust
/// Request to execute a remediation action
#[derive(Clone, Debug)]
pub struct ExecutionRequest {
    /// Unique request identifier
    pub request_id: String,

    /// Remediation action to execute
    pub action: RemediationAction,

    /// Target service ID
    pub service_id: String,

    /// Escalation tier determining execution path
    pub tier: EscalationTier,

    /// Confidence score from M15
    pub confidence: f64,

    /// Execution timeout in milliseconds
    pub timeout_ms: u64,

    /// Whether rollback is enabled
    pub enable_rollback: bool,

    /// Context data
    pub context: HashMap<String, String>,

    /// Timestamp of request creation
    pub timestamp: DateTime<Utc>,
}
```

### Execution Status

```rust
/// Current status of an execution
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExecutionStatus {
    /// Waiting for approval or resources
    Pending,

    /// Awaiting human approval
    AwaitingApproval,

    /// Awaiting PBFT consensus
    AwaitingConsensus,

    /// Currently executing
    InProgress,

    /// Successfully completed
    Completed,

    /// Failed and rolling back
    RollingBack,

    /// Execution failed
    Failed,

    /// Execution timed out
    TimedOut,

    /// Execution was cancelled
    Cancelled,
}
```

### Execution Result

```rust
/// Result of action execution
#[derive(Clone, Debug)]
pub struct ExecutionResult {
    /// Request ID
    pub request_id: String,

    /// Final execution status
    pub status: ExecutionStatus,

    /// Whether action succeeded
    pub success: bool,

    /// Duration in milliseconds
    pub duration_ms: u64,

    /// Error message if failed
    pub error: Option<String>,

    /// Start timestamp
    pub started_at: DateTime<Utc>,

    /// End timestamp
    pub completed_at: DateTime<Utc>,

    /// Execution output/response
    pub output: Option<String>,

    /// Whether rollback was performed
    pub rolled_back: bool,

    /// Rollback result if applicable
    pub rollback_result: Option<RollbackResult>,
}
```

### Rollback Result

```rust
/// Result of rollback operation
#[derive(Clone, Debug)]
pub struct RollbackResult {
    /// Whether rollback succeeded
    pub success: bool,

    /// Rollback duration in milliseconds
    pub duration_ms: u64,

    /// Error if rollback failed
    pub error: Option<String>,

    /// State restored to
    pub restored_state: HashMap<String, String>,
}
```

---

## Public API

### Action Executor Service

```rust
/// Main Action Executor service
pub struct ActionExecutor {
    config: ExecutorConfig,
    router: ExecutionRouter,
    dispatcher: TierDispatcher,
    bridge_manager: ServiceBridgeManager,
    monitor: ExecutionMonitor,
    rollback_manager: RollbackManager,
    event_emitter: EventEmitter<ExecutionEvent>,
}

impl ActionExecutor {
    /// Create a new ActionExecutor instance
    pub fn new(config: ExecutorConfig) -> Self;

    /// Submit an execution request
    pub async fn execute(&mut self, request: ExecutionRequest) -> Result<String, Error>;

    /// Get execution status
    pub fn get_status(&self, request_id: &str) -> Option<ExecutionStatus>;

    /// Get execution result
    pub fn get_result(&self, request_id: &str) -> Option<ExecutionResult>;

    /// Cancel pending execution
    pub fn cancel(&mut self, request_id: &str) -> Result<(), Error>;

    /// List in-progress executions
    pub fn list_in_progress(&self) -> Vec<ExecutionRequest>;
}
```

### Execution Control

```rust
impl ActionExecutor {
    /// Execute action at L0 (auto-execute)
    pub async fn execute_l0(&self, request: ExecutionRequest) -> Result<ExecutionResult, Error>;

    /// Execute action at L1 (notify human)
    pub async fn execute_l1(&self, request: ExecutionRequest) -> Result<ExecutionResult, Error>;

    /// Execute action at L2 (require approval)
    pub async fn execute_l2(&self, request: ExecutionRequest) -> Result<ExecutionResult, Error>;

    /// Execute action at L3 (PBFT consensus)
    pub async fn execute_l3(&self, request: ExecutionRequest) -> Result<ExecutionResult, Error>;

    /// Route request to appropriate tier
    pub async fn route_and_execute(&self, request: ExecutionRequest)
        -> Result<ExecutionResult, Error>;
}
```

### Service Bridge Operations

```rust
impl ActionExecutor {
    /// Send action to SYNTHEX service
    pub async fn execute_via_synthex(
        &self,
        service_id: &str,
        action: &RemediationAction,
    ) -> Result<ServiceResponse, Error>;

    /// Send action to SAN-K7 orchestrator
    pub async fn execute_via_sank7(
        &self,
        service_id: &str,
        action: &RemediationAction,
    ) -> Result<ServiceResponse, Error>;

    /// Send action to DevOps engine
    pub async fn execute_via_devops(
        &self,
        service_id: &str,
        action: &RemediationAction,
    ) -> Result<ServiceResponse, Error>;

    /// Direct service execution (for local operations)
    pub async fn execute_direct(
        &self,
        service_id: &str,
        action: &RemediationAction,
    ) -> Result<ServiceResponse, Error>;
}
```

### Rollback Management

```rust
impl ActionExecutor {
    /// Trigger rollback of failed execution
    pub async fn rollback(&mut self, request_id: &str) -> Result<RollbackResult, Error>;

    /// Save execution checkpoint for rollback
    pub fn save_checkpoint(&mut self, request_id: &str, state: HashMap<String, String>);

    /// Get saved checkpoint
    pub fn get_checkpoint(&self, request_id: &str) -> Option<HashMap<String, String>>;

    /// Clear checkpoint after successful completion
    pub fn clear_checkpoint(&mut self, request_id: &str);
}
```

---

## Configuration

### TOML Configuration Example

```toml
[layer.L3.M16]
enabled = true
version = "1.0.0"

# Global execution settings
[layer.L3.M16.execution]
default_timeout_ms = 60000
max_concurrent_executions = 10
enable_rollback = true
enable_checkpointing = true
checkpoint_retention_hours = 24

# L0 Auto-Execute tier
[layer.L3.M16.tiers.l0]
enabled = true
confidence_threshold = 0.9
auto_execute = true
approval_timeout_ms = 0
requires_human_confirmation = false

# L1 Notify Human tier
[layer.L3.M16.tiers.l1]
enabled = true
confidence_threshold = 0.7
auto_execute = false
approval_timeout_ms = 300000
notification_channel = "ops-alerts"
wait_for_response = false

# L2 Require Approval tier
[layer.L3.M16.tiers.l2]
enabled = true
confidence_threshold = 0.5
auto_execute = false
approval_timeout_ms = 1800000
requires_human_confirmation = true
min_approvers = 1
escalation_on_timeout = true

# L3 PBFT Consensus tier
[layer.L3.M16.tiers.l3]
enabled = true
confidence_threshold = 0.0
auto_execute = false
approval_timeout_ms = 300000
requires_consensus = true
quorum_requirement = 27
consensus_timeout_ms = 300000

# Service bridge configuration
[layer.L3.M16.bridges]
synthex_endpoint = "http://localhost:8090"
sank7_endpoint = "http://localhost:8100"
devops_endpoint = "http://localhost:8081"
bridge_timeout_ms = 30000
retry_attempts = 3
retry_backoff_ms = 1000

# Action-specific timeouts
[layer.L3.M16.action_timeouts]
retry_with_backoff_ms = 300000
circuit_breaker_reset_ms = 10000
service_restart_ms = 120000
graceful_degradation_ms = 30000
fallback_to_cached_ms = 5000
cache_cleanup_ms = 60000
session_rotation_ms = 30000
database_vacuum_ms = 600000
alert_human_ms = 300000

# Rollback settings
[layer.L3.M16.rollback]
enable_auto_rollback = true
rollback_delay_ms = 5000
rollback_timeout_ms = 120000
preserve_partial_state = true
max_rollback_retries = 3

# Monitoring
[layer.L3.M16.monitoring]
track_execution_duration = true
track_tier_distribution = true
track_success_rate = true
track_rollback_rate = true
alert_on_timeout = true
timeout_threshold_percent = 5
```

---

## Bi-Directional Data Flow

### Inbound Data Flow

```rust
/// Messages received by Action Executor
#[derive(Debug, Clone)]
pub enum ActionExecutorInbound {
    // From M14 Remediation Engine
    ExecutionRequest {
        request_id: String,
        action: RemediationAction,
        service_id: String,
        tier: EscalationTier,
        confidence: f64,
    },

    // From M15 Confidence Calculator
    ConfidenceUpdate {
        request_id: String,
        confidence: f64,
        new_tier: EscalationTier,
    },

    // From L6 Consensus
    ConsensusApproval {
        request_id: String,
        approved: bool,
        approval_count: u32,
    },

    // From human operator (L1/L2)
    HumanApproval {
        request_id: String,
        approved: bool,
        approver_id: String,
    },

    // From service bridges
    ExecutionFeedback {
        request_id: String,
        status: ServiceResponse,
        partial_completion: Option<f64>,
    },
}
```

### Outbound Data Flow

```rust
/// Messages emitted by Action Executor
#[derive(Debug, Clone)]
pub enum ActionExecutorOutbound {
    // To M17 Outcome Recorder
    ExecutionStarted {
        request_id: String,
        action: RemediationAction,
        tier: EscalationTier,
    },

    ExecutionCompleted {
        request_id: String,
        result: ExecutionResult,
    },

    // To M18 Feedback Loop
    ExecutionFeedback {
        request_id: String,
        action: RemediationAction,
        success: bool,
        duration_ms: u64,
    },

    // To L4 Integration Bridges
    RemoteExecution {
        service_id: String,
        action: RemediationAction,
        request_id: String,
    },

    // To L6 Consensus
    ConsensusRequest {
        request_id: String,
        action: RemediationAction,
        severity: Severity,
    },

    // To subscribers
    ExecutionEvent {
        request_id: String,
        status: ExecutionStatus,
        timestamp: DateTime<Utc>,
    },
}
```

---

## Metrics

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `me_m16_executions_total` | Counter | tier, action, result | Total executions |
| `me_m16_execution_duration_ms` | Histogram | tier, action | Execution duration |
| `me_m16_success_rate` | Gauge | tier, action | Success rate |
| `me_m16_tier_distribution` | Counter | tier | Executions by tier |
| `me_m16_approval_pending` | Gauge | tier | Pending approvals |
| `me_m16_execution_timeout` | Counter | action | Timeout count |
| `me_m16_rollback_total` | Counter | action, success | Rollback events |
| `me_m16_bridge_latency_ms` | Histogram | bridge | Bridge call latency |
| `me_m16_consensus_requests` | Counter | status | PBFT consensus count |
| `me_m16_in_progress` | Gauge | tier | Currently executing |

---

## Error Codes

| Code | Name | Severity | Description | Recovery |
|------|------|----------|-------------|----------|
| E16001 | InvalidExecutionRequest | Warning | Request validation failed | Check request format |
| E16002 | UnknownService | Error | Target service not found | Verify service exists |
| E16003 | BridgeUnavailable | Error | Cannot reach service bridge | Retry or use alternate |
| E16004 | ExecutionTimeout | Error | Action execution timed out | Rollback and retry |
| E16005 | ApprovalTimeout | Warning | Approval timeout exceeded | Escalate or auto-execute |
| E16006 | ConsensusFailed | Critical | PBFT consensus failed | Manual decision |
| E16007 | RollbackFailed | Critical | Cannot rollback failed action | Operational alert |
| E16008 | CheckpointError | Warning | Cannot save checkpoint | Continue without |
| E16009 | ActionNotSupported | Error | Action not supported for service | Select different action |
| E16010 | ConcurrencyLimitExceeded | Warning | Max concurrent executions | Queue and retry |

---

## Related Modules

- **M14_REMEDIATION_ENGINE**: Determines actions to execute
- **M15_CONFIDENCE_CALCULATOR**: Provides confidence scores
- **M17_OUTCOME_RECORDER**: Records execution outcomes
- **M18_FEEDBACK_LOOP**: Learns from execution results
- **L4_INTEGRATION**: Service bridges (SYNTHEX, SAN-K7, DevOps)
- **L6_CONSENSUS**: PBFT consensus for critical actions
- **L2_HEALTH_MONITOR**: Provides health status for validation

---

## Navigation

| Direction | Link |
|-----------|------|
| Up | [INDEX.md](../INDEX.md) |
| Layer | [L03_CORE_LOGIC.md](../layers/L03_CORE_LOGIC.md) |
| Previous | [M15_CONFIDENCE_CALCULATOR.md](M15_CONFIDENCE_CALCULATOR.md) |
| Next | [M17_OUTCOME_RECORDER.md](M17_OUTCOME_RECORDER.md) |
| Related | [M18_FEEDBACK_LOOP.md](M18_FEEDBACK_LOOP.md) |
| API | [REST_API.md](../integration/REST_API.md) |

---

*[Back to Index](../INDEX.md) | [Layer: L03 Core Logic](../layers/L03_CORE_LOGIC.md)*
