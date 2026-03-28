# Module M13: Pipeline Manager

> **M13_PIPELINE_MANAGER** | Pipeline Orchestration and Execution | Layer: L3 Core Logic | [Back to Index](../INDEX.md)

---

## Navigation

| Direction | Link |
|-----------|------|
| Up | [INDEX.md](../INDEX.md) |
| Layer | [L03_CORE_LOGIC.md](../layers/L03_CORE_LOGIC.md) |
| Related | [M14_REMEDIATION_ENGINE.md](M14_REMEDIATION_ENGINE.md) |
| Related | [M15_CONFIDENCE_CALCULATOR.md](M15_CONFIDENCE_CALCULATOR.md) |
| Pattern | [PATTERN_REMEDIATION.md](../patterns/PATTERN_REMEDIATION.md) |
| L2 Services | [L02_SERVICES.md](../layers/L02_SERVICES.md) |
| L4 Integration | [L04_INTEGRATION.md](../layers/L04_INTEGRATION.md) |

---

## Module Specification

### Overview

The Pipeline Manager module provides orchestration and execution of core business logic pipelines. It manages pipeline definitions, execution stages, prioritization, and SLO tracking across 8 standard pipelines including health monitoring, log processing, auto-remediation, neural learning, PBFT consensus, tensor encoding, service discovery, and metrics aggregation.

### Module Properties

| Property | Value |
|----------|-------|
| Module ID | M13 |
| Module Name | Pipeline Manager |
| Layer | L3 (Core Logic) |
| Version | 1.0.0 |
| Dependencies | M02 (Config), M04 (Metrics), M05 (State) |
| Dependents | M14 (Remediation), M15 (Confidence), M16 (Executor), M17 (Scheduler), M18 (Feedback) |

---

## Architecture Diagram

```
+-----------------------------------------------------------------------------------+
|                      M13: PIPELINE MANAGER                                      |
+-----------------------------------------------------------------------------------+
|                                                                                   |
|  +------------------------+    +------------------------+    +--------------+   |
|  |   PIPELINE REGISTRY    |    |  PIPELINE SCHEDULER    |    | STAGE ROUTER |   |
|  |                        |    |                        |    |              |   |
|  | - 8 standard pipelines |    | - Priority ordering    |    | - Source     |   |
|  | - Dynamic registration |--->| - Latency tracking     |--->| - Ingress    |   |
|  | - Enabled/disabled     |    | - Throughput targeting |    | - Transform  |   |
|  | - SLO definitions      |    | - Error budget mgmt    |    | - Route      |   |
|  +--------+----------------+    +--------+---------------+    | - Sink       |   |
|           |                              |                   | - Feedback   |   |
|           v                              v                   +--------------+   |
|  +------------------------+    +------------------------+          |            |
|  |  EXECUTION ENGINE      |    |  PERFORMANCE MONITOR   |          |            |
|  |                        |    |                        |          v            |
|  | - Pipeline execution   |    | - Latency SLO tracking |    +--------------+   |
|  | - Stage transitions    |    | - Throughput tracking  |    | EVENT STREAM |   |
|  | - Error handling       |    | - Error budget tracking|    |              |   |
|  | - Result aggregation   |    | - Violation alerts     |    | -> M14-M18   |   |
|  +------------------------+    +------------------------+    +--------------+   |
|                                                                                   |
+-----------------------------------------------------------------------------------+
        ^                    ^                    ^                    |
        |                    |                    |                    v
   [L1: Config]        [L1: Metrics]       [L2: Services]      [L3 Modules]
```

---

## Core Data Structures

### Pipeline Definition

```rust
/// Pipeline definition with SLO configuration
#[derive(Clone, Debug)]
pub struct Pipeline {
    /// Pipeline ID (e.g., "PL-HEALTH-001")
    pub id: String,

    /// Pipeline name
    pub name: String,

    /// Priority (1-10, where 1 is highest)
    pub priority: u8,

    /// Latency SLO in milliseconds
    pub latency_slo_ms: u64,

    /// Throughput target (events per second)
    pub throughput_target: u64,

    /// Error budget (0.0 - 1.0)
    pub error_budget: f64,

    /// Enabled/disabled flag
    pub enabled: bool,
}
```

### Pipeline Stage Enumeration

```rust
/// Pipeline execution stage
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PipelineStage {
    /// Data source stage
    Source,

    /// Entry point with validation
    Ingress,

    /// Processing and transformation
    Transform,

    /// Conditional routing
    Route,

    /// Result destination
    Sink,

    /// Feedback collection
    Feedback,
}
```

### Standard Pipelines

```rust
/// Default 8 core pipelines
pub fn default_pipelines() -> Vec<Pipeline> {
    vec![
        // 1. Health Monitoring Pipeline
        Pipeline {
            id: "PL-HEALTH-001".into(),
            name: "Health Monitoring Pipeline".into(),
            priority: 1,
            latency_slo_ms: 100,
            throughput_target: 1000,
            error_budget: 0.001,
            enabled: true,
        },

        // 2. Log Processing Pipeline
        Pipeline {
            id: "PL-LOG-001".into(),
            name: "Log Processing Pipeline".into(),
            priority: 2,
            latency_slo_ms: 50,
            throughput_target: 500_000,
            error_budget: 0.005,
            enabled: true,
        },

        // 3. Auto-Remediation Pipeline
        Pipeline {
            id: "PL-REMEDIATE-001".into(),
            name: "Auto-Remediation Pipeline".into(),
            priority: 1,
            latency_slo_ms: 500,
            throughput_target: 100,
            error_budget: 0.0001,
            enabled: true,
        },

        // 4. Neural Pathway Learning Pipeline
        Pipeline {
            id: "PL-HEBBIAN-001".into(),
            name: "Neural Pathway Learning Pipeline".into(),
            priority: 2,
            latency_slo_ms: 100,
            throughput_target: 10_000,
            error_budget: 0.01,
            enabled: true,
        },

        // 5. PBFT Consensus Pipeline
        Pipeline {
            id: "PL-CONSENSUS-001".into(),
            name: "PBFT Consensus Pipeline".into(),
            priority: 1,
            latency_slo_ms: 5000,
            throughput_target: 10,
            error_budget: 0.0001,
            enabled: true,
        },

        // 6. Tensor Encoding Pipeline
        Pipeline {
            id: "PL-TENSOR-001".into(),
            name: "Tensor Encoding Pipeline".into(),
            priority: 3,
            latency_slo_ms: 10,
            throughput_target: 100_000,
            error_budget: 0.005,
            enabled: true,
        },

        // 7. Service Discovery Pipeline
        Pipeline {
            id: "PL-DISCOVERY-001".into(),
            name: "Service Discovery Pipeline".into(),
            priority: 2,
            latency_slo_ms: 1000,
            throughput_target: 100,
            error_budget: 0.01,
            enabled: true,
        },

        // 8. Metrics Aggregation Pipeline
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
```

---

## Public API

### Pipeline Manager Service

```rust
/// Main Pipeline Manager service
pub struct PipelineManager {
    pipelines: Vec<Pipeline>,
    registry: PipelineRegistry,
    scheduler: PipelineScheduler,
    executor: ExecutionEngine,
    monitor: PerformanceMonitor,
    event_emitter: EventEmitter<PipelineEvent>,
}

impl Pipeline {
    /// Create a new pipeline with default configuration
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
```

### Pipeline Creation and Configuration

```rust
impl PipelineManager {
    /// Create a new pipeline
    pub fn new(config: PipelineManagerConfig) -> Self;

    /// Register a new pipeline
    pub fn register_pipeline(&mut self, pipeline: Pipeline) -> Result<(), Error>;

    /// Unregister a pipeline
    pub fn unregister_pipeline(&mut self, pipeline_id: &str) -> Result<(), Error>;

    /// Get pipeline by ID
    pub fn get_pipeline(&self, pipeline_id: &str) -> Option<&Pipeline>;

    /// Enable a pipeline
    pub fn enable_pipeline(&mut self, pipeline_id: &str) -> Result<(), Error>;

    /// Disable a pipeline
    pub fn disable_pipeline(&mut self, pipeline_id: &str) -> Result<(), Error>;

    /// Update pipeline configuration
    pub fn update_pipeline(&mut self, pipeline: Pipeline) -> Result<(), Error>;

    /// Get all pipelines
    pub fn list_pipelines(&self) -> Vec<&Pipeline>;
}
```

### Execution Control

```rust
/// Pipeline execution operations
impl PipelineManager {
    /// Start a pipeline
    pub async fn start(&mut self, pipeline_id: &str) -> Result<(), Error>;

    /// Stop a pipeline
    pub async fn stop(&mut self, pipeline_id: &str) -> Result<(), Error>;

    /// Execute a single pipeline cycle
    pub async fn execute(&self, pipeline_id: &str) -> Result<ExecutionResult, Error>;

    /// Execute all enabled pipelines
    pub async fn execute_all(&self) -> Result<Vec<ExecutionResult>, Error>;

    /// Get current execution status
    pub fn get_status(&self, pipeline_id: &str) -> Option<ExecutionStatus>;

    /// Get execution metrics
    pub fn get_metrics(&self, pipeline_id: &str) -> Option<PipelineMetrics>;
}
```

### Performance Monitoring

```rust
/// SLO and performance tracking
impl PipelineManager {
    /// Check if pipeline meets latency SLO
    pub fn check_latency_slo(&self, pipeline_id: &str) -> bool;

    /// Check if pipeline meets throughput target
    pub fn check_throughput_target(&self, pipeline_id: &str) -> bool;

    /// Check if pipeline is within error budget
    pub fn check_error_budget(&self, pipeline_id: &str) -> bool;

    /// Get SLO violation report
    pub fn get_slo_violations(&self) -> Vec<SLOViolation>;

    /// Alert on SLO violations
    pub async fn report_violation(&self, violation: SLOViolation);
}
```

---

## Configuration

### TOML Configuration Example

```toml
[layer.L3.M13]
enabled = true
version = "1.0.0"

# Global pipeline defaults
[layer.L3.M13.defaults]
enable_monitoring = true
max_concurrent_executions = 10
execution_timeout_ms = 30000
enable_event_emission = true

# SLO tracking
[layer.L3.M13.slo]
track_latency = true
track_throughput = true
track_error_budget = true
alert_on_violation = true
violation_threshold_percent = 5.0

# Individual pipeline overrides
[[layer.L3.M13.pipelines]]
id = "PL-HEALTH-001"
name = "Health Monitoring Pipeline"
priority = 1
latency_slo_ms = 100
throughput_target = 1000
error_budget = 0.001
enabled = true

[[layer.L3.M13.pipelines]]
id = "PL-REMEDIATE-001"
name = "Auto-Remediation Pipeline"
priority = 1
latency_slo_ms = 500
throughput_target = 100
error_budget = 0.0001
enabled = true

[[layer.L3.M13.pipelines]]
id = "PL-CONSENSUS-001"
name = "PBFT Consensus Pipeline"
priority = 1
latency_slo_ms = 5000
throughput_target = 10
error_budget = 0.0001
enabled = true
```

---

## Bi-Directional Data Flow

### Inbound Data Flow

```rust
/// Messages received by Pipeline Manager
#[derive(Debug, Clone)]
pub enum PipelineManagerInbound {
    // From L1 Config (M02)
    ConfigUpdate {
        pipelines: Vec<Pipeline>,
        timestamp: DateTime<Utc>,
    },

    // From L1 Metrics (M04)
    MetricsSnapshot {
        pipeline_id: String,
        latency_ms: u64,
        throughput: u64,
        error_count: u64,
        timestamp: DateTime<Utc>,
    },

    // From L3 Learning (M25)
    PerformancePrediction {
        pipeline_id: String,
        predicted_latency: u64,
        confidence: f64,
        timestamp: DateTime<Utc>,
    },

    // From L5 Remediation
    RemediationEvent {
        pipeline_id: String,
        action: RemediationAction,
        expected_impact: String,
    },
}
```

### Outbound Data Flow

```rust
/// Messages emitted by Pipeline Manager
#[derive(Debug, Clone)]
pub enum PipelineManagerOutbound {
    // To L1 Metrics
    PipelineMetrics {
        pipeline_id: String,
        latency_ms: u64,
        throughput: u64,
        error_rate: f64,
        timestamp: DateTime<Utc>,
    },

    // To L3 Learning
    ExecutionEvent {
        pipeline_id: String,
        stage: PipelineStage,
        duration_ms: u64,
        success: bool,
        timestamp: DateTime<Utc>,
    },

    // To consumers
    SLOViolation {
        pipeline_id: String,
        metric: String,
        violation_type: String,
        severity: Severity,
    },
}
```

---

## Metrics

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `me_m13_pipeline_executions_total` | Counter | pipeline, stage, result | Total pipeline executions |
| `me_m13_pipeline_duration_ms` | Histogram | pipeline, stage | Pipeline execution duration |
| `me_m13_pipeline_throughput` | Gauge | pipeline | Current throughput (events/sec) |
| `me_m13_pipeline_error_rate` | Gauge | pipeline | Current error rate (0.0-1.0) |
| `me_m13_slo_violations_total` | Counter | pipeline, metric | SLO violation count |
| `me_m13_latency_slo_met` | Gauge | pipeline | Latency SLO met (1=yes, 0=no) |
| `me_m13_throughput_target_met` | Gauge | pipeline | Throughput target met (1=yes, 0=no) |
| `me_m13_error_budget_remaining` | Gauge | pipeline | Remaining error budget (0.0-1.0) |
| `me_m13_active_executions` | Gauge | pipeline | Currently executing pipelines |
| `me_m13_stage_duration_ms` | Histogram | pipeline, stage | Duration by stage |

---

## Error Codes

| Code | Name | Severity | Description | Recovery |
|------|------|----------|-------------|----------|
| E13001 | PipelineNotFound | Warning | Pipeline ID not found | Check pipeline registry |
| E13002 | PipelineDisabled | Warning | Pipeline is disabled | Enable pipeline or check config |
| E13003 | ExecutionTimeout | Error | Pipeline execution exceeded timeout | Increase timeout or optimize |
| E13004 | StageFailed | Error | Pipeline stage failed | Check stage logs, manual review |
| E13005 | SLOViolated | Warning | Pipeline missed SLO | Alert L5 for remediation |
| E13006 | ErrorBudgetExceeded | Critical | Pipeline exceeded error budget | Escalate to L5 remediation |
| E13007 | ThroughputDegraded | Warning | Throughput below target | Check pipeline dependencies |
| E13008 | LatencyIncreased | Warning | Latency exceeds SLO | Optimize pipeline stages |
| E13009 | InvalidConfiguration | Critical | Invalid pipeline config | Review and correct config |
| E13010 | RegistrationFailed | Error | Cannot register pipeline | Check for duplicates |

---

## Related Modules

- **M14_REMEDIATION_ENGINE**: Receives execution feedback, triggers auto-remediation
- **M15_CONFIDENCE_CALCULATOR**: Scores pipeline confidence for remediation decisions
- **M16_ACTION_EXECUTOR**: Executes pipeline-triggered actions
- **M17_OUTCOME_RECORDER**: Records pipeline execution outcomes
- **M18_FEEDBACK_LOOP**: Learns from pipeline performance patterns
- **L2_HEALTH_MONITOR**: Provides health data for pipeline routing decisions
- **L2_SERVICE_DISCOVERY**: Discovers services that participate in pipelines
- **L5_LEARNING**: Provides predictive insights for pipeline optimization

---

## Navigation

| Direction | Link |
|-----------|------|
| Up | [INDEX.md](../INDEX.md) |
| Layer | [L03_CORE_LOGIC.md](../layers/L03_CORE_LOGIC.md) |
| Previous | [M12_CIRCUIT_BREAKER.md](M12_CIRCUIT_BREAKER.md) |
| Next | [M14_REMEDIATION_ENGINE.md](M14_REMEDIATION_ENGINE.md) |
| Related | [M15_CONFIDENCE_CALCULATOR.md](M15_CONFIDENCE_CALCULATOR.md) |
| API | [REST_API.md](../integration/REST_API.md) |

---

*[Back to Index](../INDEX.md) | [Layer: L03 Core Logic](../layers/L03_CORE_LOGIC.md)*
