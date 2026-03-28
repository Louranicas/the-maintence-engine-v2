# Layer 3: Core Logic

> **L03_CORE_LOGIC** | Remediation & Pipeline Layer | [Back to Index](../INDEX.md)

---

## Navigation

| Direction | Link |
|-----------|------|
| Up | [INDEX.md](../INDEX.md) |
| Previous | [L02_SERVICES.md](L02_SERVICES.md) |
| Next | [L04_INTEGRATION.md](L04_INTEGRATION.md) |
| Related | [M13-M18 Modules](../modules/) |

---

## Layer Overview

The Core Logic Layer (L3) implements the primary business logic for maintenance operations including remediation execution, pipeline management, confidence scoring, and workflow orchestration. This layer transforms detected issues into actionable remediation plans.

### Layer Properties

| Property | Value |
|----------|-------|
| Layer ID | L3 |
| Layer Name | Core Logic |
| Source Directory | `src/m3_core_logic/` |
| Dependencies | L1 (Foundation), L2 (Services) |
| Dependents | L4, L5, L6 |
| Modules | M13-M18 |
| Primary Functions | Remediation, Pipeline Execution, Confidence Scoring |

---

## Architecture

```
+------------------------------------------------------------------+
|                      L3: Core Logic Layer                          |
+------------------------------------------------------------------+
|                                                                  |
|  +---------------------------+  +---------------------------+    |
|  |   Remediation Engine      |  |   Pipeline Executor       |    |
|  |                           |  |                           |    |
|  |  - Action planning        |  |  - Stage execution        |    |
|  |  - Action execution       |  |  - Parallel processing    |    |
|  |  - Rollback handling      |  |  - Error handling         |    |
|  |  - Result tracking        |  |  - Progress tracking      |    |
|  +-------------+-------------+  +-------------+-------------+    |
|                |                              |                  |
|                +------------+-----------------+                  |
|                             |                                    |
|  +---------------------------+---------------------------+       |
|  |              Confidence & Workflow Engine             |       |
|  |                                                       |       |
|  |  - Confidence scoring       - Workflow orchestration  |       |
|  |  - Action prioritization    - Task scheduling         |       |
|  |  - Outcome prediction       - State management        |       |
|  +-------------------------------------------------------+       |
|                                                                  |
+------------------------------------------------------------------+
```

---

## Module Reference (M13-M18)

| Module | File | Purpose |
|--------|------|---------|
| M13 | `remediation.rs` | Remediation engine - action execution |
| M14 | `escalation.rs` | Escalation manager - tier progression |
| M15 | `confidence.rs` | Confidence scoring - action confidence |
| M16 | `pipeline.rs` | Pipeline executor - stage orchestration |
| M17 | `scheduler.rs` | Task scheduler - job scheduling |
| M18 | `workflow.rs` | Workflow engine - process orchestration |

---

## Core Types

### RemediationAction

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemediationAction {
    /// Unique action identifier
    pub id: ActionId,

    /// Type of remediation action
    pub action_type: ActionType,

    /// Target service or component
    pub target: Target,

    /// Action parameters
    pub params: ActionParams,

    /// Escalation tier (L0-L3)
    pub tier: EscalationTier,

    /// Maximum execution timeout
    pub timeout: Duration,

    /// Rollback action if this fails
    pub rollback: Option<Box<RemediationAction>>,

    /// Confidence score [0.0, 1.0]
    pub confidence: f64,
}
```

### ActionType

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum ActionType {
    // L0: Immediate
    Retry { max_attempts: u32, backoff: BackoffStrategy },
    Restart { graceful: bool },
    CircuitBreak { duration: Duration },

    // L1: Standard
    Rollback { version: String },
    Reconfigure { config_delta: ConfigDelta },
    ClearCache { cache_name: String },
    DrainConnections { timeout: Duration },

    // L2: Elevated (requires consensus)
    Failover { target: FailoverTarget },
    Scale { direction: ScaleDirection, amount: i32 },
    Migrate { from: NodeId, to: NodeId },
    IsolateNode { node: NodeId },

    // L3: Critical (requires human approval)
    Manual { instructions: String },
    Emergency { procedure: EmergencyProcedure },
}
```

### Pipeline

```rust
#[derive(Debug, Clone)]
pub struct Pipeline {
    /// Pipeline identifier
    pub id: PipelineId,

    /// Pipeline name
    pub name: String,

    /// Execution stages
    pub stages: Vec<PipelineStage>,

    /// Priority level
    pub priority: Priority,

    /// Service Level Objective
    pub slo: Duration,

    /// Current state
    pub state: PipelineState,
}

#[derive(Debug, Clone)]
pub struct PipelineStage {
    pub name: String,
    pub action: Box<dyn StageAction>,
    pub timeout: Duration,
    pub retry_policy: Option<RetryPolicy>,
    pub on_failure: FailureAction,
}
```

---

## Remediation Engine

### Engine API

```rust
pub struct RemediationEngine {
    /// Execute a remediation action
    pub async fn execute(&self, action: RemediationAction) -> Result<RemediationResult>;

    /// Execute with automatic rollback on failure
    pub async fn execute_with_rollback(&self, action: RemediationAction) -> Result<RemediationResult>;

    /// Cancel an in-progress action
    pub fn cancel(&self, action_id: &ActionId) -> Result<()>;

    /// Get action status
    pub fn status(&self, action_id: &ActionId) -> Option<ActionStatus>;

    /// List pending actions
    pub fn pending(&self) -> Vec<&RemediationAction>;

    /// Get execution history
    pub fn history(&self, limit: usize) -> Vec<RemediationRecord>;
}
```

### RemediationResult

```rust
pub struct RemediationResult {
    /// Action that was executed
    pub action_id: ActionId,

    /// Was the action successful?
    pub success: bool,

    /// Result status
    pub status: ActionStatus,

    /// Execution duration
    pub duration: Duration,

    /// Error if failed
    pub error: Option<RemediationError>,

    /// Did we need to escalate?
    pub escalated: bool,

    /// Final escalation tier reached
    pub final_tier: EscalationTier,

    /// Side effects of the action
    pub side_effects: Vec<SideEffect>,
}
```

---

## Pipeline Executor

### Executor API

```rust
pub struct PipelineExecutor {
    /// Execute a pipeline
    pub async fn execute(&self, pipeline: Pipeline) -> Result<PipelineResult>;

    /// Execute pipeline stages in parallel where possible
    pub async fn execute_parallel(&self, pipeline: Pipeline) -> Result<PipelineResult>;

    /// Pause pipeline execution
    pub fn pause(&self, pipeline_id: &PipelineId) -> Result<()>;

    /// Resume paused pipeline
    pub fn resume(&self, pipeline_id: &PipelineId) -> Result<()>;

    /// Cancel pipeline
    pub fn cancel(&self, pipeline_id: &PipelineId) -> Result<()>;

    /// Get pipeline status
    pub fn status(&self, pipeline_id: &PipelineId) -> Option<PipelineStatus>;
}
```

### 8 Core Pipelines

| ID | Name | Priority | SLO | Modules |
|----|------|----------|-----|---------|
| PL-HEALTH-001 | Health Monitoring | 1 | <100ms | M08, M07 |
| PL-LOG-001 | Log Processing | 2 | <50ms | M05, M06 |
| PL-REMEDIATE-001 | Auto-Remediation | 1 | <500ms | M13, M14, M15 |
| PL-HEBBIAN-001 | Neural Learning | 2 | <100ms | M25, M26, M27 |
| PL-CONSENSUS-001 | PBFT Consensus | 1 | <5s | M31, M32, M35 |
| PL-TENSOR-001 | Tensor Encoding | 3 | <10ms | M04, M03 |
| PL-DISCOVERY-001 | Service Discovery | 2 | <1s | M10, M12 |
| PL-METRICS-001 | Metrics Aggregation | 3 | <200ms | M06, M08 |

---

## Confidence Scoring

### Confidence Calculator

```rust
pub struct ConfidenceCalculator {
    /// Calculate confidence for a remediation action
    pub fn calculate(&self, action: &RemediationAction, context: &Context) -> f64;

    /// Get factors contributing to confidence
    pub fn explain(&self, action: &RemediationAction) -> Vec<ConfidenceFactor>;

    /// Adjust confidence based on historical outcomes
    pub fn adjust_from_history(&mut self, action_type: &ActionType, outcome: &Outcome);
}

pub struct ConfidenceFactor {
    pub name: String,
    pub weight: f64,
    pub contribution: f64,
    pub description: String,
}
```

### Confidence Thresholds (Escalation Tiers)

| Tier | Condition | Action |
|------|-----------|--------|
| L0 Auto-Execute | confidence >= 0.9, severity <= MEDIUM | Execute immediately |
| L1 Notify Human | confidence >= 0.7, severity <= HIGH | Notify, then execute |
| L2 Require Approval | confidence < 0.7 OR severity = HIGH | Wait for human approval |
| L3 PBFT Consensus | Critical actions | Require 27/40 agent votes |

---

## Escalation Manager

### Escalation API

```rust
pub struct EscalationManager {
    /// Get current tier for an incident
    pub fn current_tier(&self, incident_id: &IncidentId) -> EscalationTier;

    /// Escalate to next tier
    pub async fn escalate(&mut self, incident_id: &IncidentId) -> Result<EscalationTier>;

    /// Check if escalation is needed based on result
    pub fn should_escalate(&self, result: &RemediationResult) -> bool;

    /// Get available actions for tier
    pub fn actions_for_tier(&self, tier: EscalationTier) -> Vec<ActionType>;

    /// Request human approval (L3)
    pub async fn request_approval(&self, action: &RemediationAction) -> Result<ApprovalRequest>;
}
```

---

## Workflow Engine

### Workflow API

```rust
pub struct WorkflowEngine {
    /// Create a new workflow
    pub fn create(&mut self, definition: WorkflowDefinition) -> Result<WorkflowId>;

    /// Start workflow execution
    pub async fn start(&self, workflow_id: &WorkflowId) -> Result<()>;

    /// Get workflow state
    pub fn state(&self, workflow_id: &WorkflowId) -> Option<&WorkflowState>;

    /// Trigger workflow event
    pub async fn trigger(&self, workflow_id: &WorkflowId, event: WorkflowEvent) -> Result<()>;
}

pub struct WorkflowDefinition {
    pub name: String,
    pub states: Vec<WorkflowState>,
    pub transitions: Vec<Transition>,
    pub initial_state: String,
    pub final_states: Vec<String>,
}
```

---

## Inter-Layer Communication

### Events from L2 (Services)

```rust
pub enum L2InputEvent {
    ServiceHealthChanged { service: ServiceId, old: HealthState, new: HealthState },
    ErrorDetected { error: ErrorVector, context: ErrorContext },
    CircuitOpened { service: ServiceId, failures: u32 },
}
```

### Events to L4 (Integration)

```rust
pub enum L3OutputEvent {
    RemediationStarted { action: RemediationAction },
    RemediationCompleted { result: RemediationResult },
    EscalationTriggered { incident: IncidentId, tier: EscalationTier },
    PipelineCompleted { pipeline_id: PipelineId, result: PipelineResult },
}
```

---

## Metrics

| Metric | Type | Description |
|--------|------|-------------|
| `me_l3_remediations_total` | Counter | Total remediation actions by tier |
| `me_l3_remediations_success` | Counter | Successful actions |
| `me_l3_remediations_failed` | Counter | Failed actions |
| `me_l3_remediation_duration_ms` | Histogram | Action execution duration |
| `me_l3_escalations_total` | Counter | Escalation count by tier |
| `me_l3_pipeline_executions` | Counter | Pipeline execution count |
| `me_l3_pipeline_duration_ms` | Histogram | Pipeline execution duration |
| `me_l3_confidence_scores` | Histogram | Confidence score distribution |

---

## Configuration

```toml
[layer.L3]
enabled = true
startup_order = 3

[layer.L3.remediation]
auto_remediate = true
max_concurrent_actions = 10
default_timeout_ms = 30000

[layer.L3.escalation]
l0_timeout_ms = 1000
l1_timeout_ms = 30000
l2_timeout_ms = 300000
l3_requires_approval = true

[layer.L3.confidence]
default_threshold = 0.7
auto_execute_threshold = 0.9
learning_rate = 0.01

[layer.L3.pipelines]
max_concurrent = 5
default_slo_ms = 1000
checkpoint_interval = 10
```

---

## CLI Commands

```bash
# View remediation status
./maintenance-engine remediation status

# List pending remediations
./maintenance-engine remediation pending

# View remediation history
./maintenance-engine remediation history --limit 50

# Execute manual remediation
./maintenance-engine remediation execute --action restart --target synthex

# Approve L3 remediation
./maintenance-engine remediation approve --id REM-001

# View pipeline status
./maintenance-engine pipeline status --id PL-HEALTH-001

# View confidence factors
./maintenance-engine confidence explain --action restart --target synthex
```

---

## Navigation

| Direction | Link |
|-----------|------|
| Up | [INDEX.md](../INDEX.md) |
| Previous | [L02_SERVICES.md](L02_SERVICES.md) |
| Next | [L04_INTEGRATION.md](L04_INTEGRATION.md) |
| Related Spec | [../ai_specs/ESCALATION_SPEC.md](../../ai_specs/ESCALATION_SPEC.md) |
| API | [REST_API.md](../integration/REST_API.md) |

---

*[Back to Index](../INDEX.md) | [Previous: L02 Services](L02_SERVICES.md) | [Next: L04 Integration](L04_INTEGRATION.md)*
