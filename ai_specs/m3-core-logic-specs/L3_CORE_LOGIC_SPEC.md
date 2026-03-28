# L3: Core Logic Layer Specification

> Target: ~8,000 LOC | 6 modules (M13-M18) | 300+ tests

---

## Layer Purpose

The Core Logic layer implements the decision-making pipeline: detecting issues, computing remediation confidence, executing actions, recording outcomes, and closing the feedback loop. It consumes L2 health data and emits remediation signals downward to L2, while feeding learning data upward to L5.

---

## Module Specifications

### M13: Pipeline Manager (`pipeline.rs`)

**Purpose:** Orchestrate the 8 core processing pipelines with priority scheduling and SLO tracking.

**Target:** ~1,400 LOC, 50+ tests

**Key Types:**
```rust
pub struct PipelineManager { inner: RwLock<PipelineManagerInner> }

pub struct Pipeline {
    id: PipelineId,
    name: String,
    priority: Priority,
    slo: Duration,
    stages: Vec<PipelineStage>,
    status: PipelineStatus,
}

pub enum PipelineStatus { Idle, Running, Completed, Failed, Paused }
pub enum Priority { Critical = 1, High = 2, Normal = 3 }
```

**Key Traits:**
```rust
pub trait PipelineOps: Send + Sync {
    fn create_pipeline(&self, config: PipelineConfig) -> Result<PipelineId>;
    fn execute(&self, id: &PipelineId) -> Result<PipelineResult>;
    fn pause(&self, id: &PipelineId) -> Result<()>;
    fn resume(&self, id: &PipelineId) -> Result<()>;
    fn status(&self, id: &PipelineId) -> Result<PipelineStatus>;
    fn slo_compliance(&self, id: &PipelineId) -> Result<f64>;
}
```

**Tensor Contribution:** D9 (latency via pipeline execution time)

**Signals:** `PipelineStarted`, `PipelineCompleted`, `PipelineFailed`, `SloViolation`

---

### M14: Remediation Engine (`remediation.rs`)

**Purpose:** Execute auto-remediation actions with confidence-gated escalation.

**Target:** ~1,500 LOC, 50+ tests

**Key Types:**
```rust
pub struct RemediationEngine { inner: RwLock<RemediationEngineInner> }

pub struct RemediationAction {
    id: ActionId,
    target: ServiceId,
    action_type: RemediationType,
    confidence: f64,
    severity: Severity,
    escalation_tier: EscalationTier,
}

pub enum RemediationType { Restart, ScaleUp, ScaleDown, Failover, ConfigChange, DependencyFix }
pub enum EscalationTier { L0AutoExecute, L1NotifyHuman, L2RequireApproval, L3PbftConsensus }
```

**Key Traits:**
```rust
pub trait RemediationOps: Send + Sync {
    fn propose(&self, action: RemediationAction) -> Result<RemediationId>;
    fn execute(&self, id: &RemediationId) -> Result<RemediationOutcome>;
    fn rollback(&self, id: &RemediationId) -> Result<()>;
    fn history(&self, service_id: &ServiceId) -> Result<Vec<RemediationRecord>>;
    fn escalation_tier(&self, confidence: f64, severity: Severity) -> EscalationTier;
}
```

**Tensor Contribution:** D10 (error_rate reduction after remediation)

**Signals:** `RemediationProposed`, `RemediationExecuted`, `RemediationRolledBack`, `EscalationTriggered`

---

### M15: Confidence Calculator (`confidence.rs`)

**Purpose:** Compute confidence scores for proposed actions using multi-signal fusion.

**Target:** ~1,200 LOC, 50+ tests

**Key Types:**
```rust
pub struct ConfidenceCalculator { inner: RwLock<ConfidenceInner> }

pub struct ConfidenceScore {
    value: f64,           // 0.0 - 1.0
    components: Vec<ConfidenceComponent>,
    timestamp: Timestamp,
}

pub struct ConfidenceComponent {
    source: String,
    weight: f64,
    score: f64,
}
```

**Key Traits:**
```rust
pub trait ConfidenceOps: Send + Sync {
    fn calculate(&self, signals: &[Signal]) -> Result<ConfidenceScore>;
    fn threshold(&self, tier: EscalationTier) -> f64;
    fn adjust_weights(&self, outcome: &RemediationOutcome) -> Result<()>;
    fn decay(&self, elapsed: Duration) -> Result<()>;
}
```

**Computation:** `confidence = SUM(weight_i * signal_i) / SUM(weight_i)` with FMA precision

**Tensor Contribution:** None directly (feeds D10 indirectly via M14)

---

### M16: Action Executor (`action.rs`)

**Purpose:** Execute approved remediation actions with timeout and rollback support.

**Target:** ~1,500 LOC, 50+ tests

**Key Types:**
```rust
pub struct ActionExecutor { inner: RwLock<ActionExecutorInner> }

pub struct Action {
    id: ActionId,
    action_type: ActionType,
    target: ServiceId,
    timeout: Duration,
    rollback_plan: Option<RollbackPlan>,
    status: ActionStatus,
}

pub enum ActionStatus { Pending, Executing, Completed, Failed, RolledBack, TimedOut }
```

**Signals:** `ActionStarted`, `ActionCompleted`, `ActionFailed`, `ActionTimedOut`, `RollbackInitiated`

---

### M17: Outcome Recorder (`outcome.rs`)

**Purpose:** Record all action outcomes to episodic_memory.db and feed learning.

**Target:** ~900 LOC, 50+ tests

**Key Types:**
```rust
pub struct OutcomeRecorder { inner: RwLock<OutcomeRecorderInner> }

pub struct Outcome {
    action_id: ActionId,
    result: OutcomeResult,
    duration: Duration,
    confidence_before: f64,
    health_delta: f64,
    timestamp: Timestamp,
}

pub enum OutcomeResult { Success, PartialSuccess, Failure, Timeout }
```

**Database:** Writes to `episodic_memory.db` and `remediation_log.db`

---

### M18: Feedback Loop (`feedback.rs`)

**Purpose:** Close the loop by adjusting confidence weights based on outcome history.

**Target:** ~950 LOC, 50+ tests

**Key Types:**
```rust
pub struct FeedbackLoop { inner: RwLock<FeedbackInner> }

pub struct FeedbackEntry {
    outcome: Outcome,
    adjustment: f64,
    pathway_reinforcement: f64,
}
```

**Integration:**
- Reads from M17 outcomes
- Adjusts M15 confidence weights
- Reinforces M25 Hebbian pathways (upward to L5)
- Emits `FeedbackApplied`, `WeightAdjusted` signals

---

## Layer Coordinator (`mod.rs`)

**Target:** ~500 LOC, 20+ tests

**Provides:**
- `CoreLogicLayer` aggregate struct
- Builder pattern: `CoreLogicLayer::builder().pipeline(pm).remediation(re).build()?`
- `process_health_event()` — full pipeline: detect → confidence → escalate → execute → record → feedback
- Re-exports all public types

---

## Data Flow

```
L2 Health Events → M13 Pipeline → M15 Confidence → M14 Remediation
                                                        ↓
                                   M18 Feedback ← M17 Outcome ← M16 Action
                                        ↓
                                   L5 Hebbian Learning
```

---

## Design Constraints

- C1: Only imports from L1 and L2 (no L4+ imports)
- C2: All trait methods `&self`
- C3: `TensorContributor` on M13, M14, M16
- C4: Zero unsafe/unwrap/expect
- C5: `Timestamp` + `Duration` only
- C6: Signal emission on all state transitions
- C11: Nexus field capture on all remediation actions (N01 integration ready)
- C12: STDP co-activation recorded on pipeline completions

---

## Test Strategy

- Unit tests per module: 50+ each
- Integration: `tests/l3_core_logic_integration.rs`
- Benchmark: `benches/pipeline_execution.rs`
- Property: confidence always in [0.0, 1.0], escalation monotonic with severity
