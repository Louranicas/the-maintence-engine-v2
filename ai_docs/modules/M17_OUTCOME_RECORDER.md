# Module M17: Outcome Recorder

> **M17_OUTCOME_RECORDER** | Outcome Tracking | Layer: L3 Core Logic | [Back to Index](../INDEX.md)

---

## Navigation

| Direction | Link |
|-----------|------|
| Up | [INDEX.md](../INDEX.md) |
| Layer | [L03_CORE_LOGIC.md](../layers/L03_CORE_LOGIC.md) |
| Related | [M16_ACTION_EXECUTOR.md](M16_ACTION_EXECUTOR.md) |
| Related | [M18_FEEDBACK_LOOP.md](M18_FEEDBACK_LOOP.md) |
| Related | [M15_CONFIDENCE_CALCULATOR.md](M15_CONFIDENCE_CALCULATOR.md) |
| Pattern | [PATTERN_RECORDING.md](../patterns/PATTERN_RECORDING.md) |
| L1 State | [M05_STATE_PERSISTENCE.md](M05_STATE_PERSISTENCE.md) |
| L5 Learning | [L03_LEARNING.md](../layers/L03_LEARNING.md) |

---

## Module Specification

### Overview

The Outcome Recorder module tracks and persists outcomes from remediation action execution. It records execution results, calculates Hebbian pathway deltas, maintains outcome history for pattern analysis, and feeds outcome data to learning systems for model refinement and feedback loop optimization.

### Module Properties

| Property | Value |
|----------|-------|
| Module ID | M17 |
| Module Name | Outcome Recorder |
| Layer | L3 (Core Logic) |
| Version | 1.0.0 |
| Dependencies | M05 (State), M14 (Remediation), M16 (Executor) |
| Dependents | M18 (Feedback), L5 (Learning), L1 (State Persistence) |

---

## Architecture Diagram

```
+-----------------------------------------------------------------------------------+
|                      M17: OUTCOME RECORDER                                      |
+-----------------------------------------------------------------------------------+
|                                                                                   |
|  +------------------------+    +------------------------+    +--------------+   |
|  | OUTCOME COLLECTOR      |    | OUTCOME ANALYZER       |    | METRIC        |   |
|  |                        |    |                        |    | CALCULATOR    |   |
|  | - Collect results      |    | - Success/failure      |    |               |   |
|  | - Extract metadata     |--->| - Latency analysis     |--->| - Effectiveness|   |
|  | - Normalize data       |    | - Root cause detection |    | - Efficiency  |   |
|  +--------+----------------+    +--------+---------------+    | - Delta calc  |   |
|           |                              |                   +--------------+   |
|           v                              v                            |          |
|  +------------------------+    +------------------------+          |          |
|  | STORAGE MANAGER        |    | HISTORY AGGREGATOR     |          |          |
|  |                        |    |                        |          v          |
|  | - Persistent storage   |    | - Time-series tracking |    +---------+     |
|  | - Database write       |    | - Trend analysis       |    | PATHWAY |     |
|  | - Backup/recovery      |    | - Pattern extraction   |    | DELTA   |     |
|  | - Durability guarantee |    | - Periodic aggregation |    |WEIGHT   |     |
|  +--------+----------------+    +--------+---------------+    +--+------+     |
|           |                              |                       |              |
|           +--------- Coordinate and Store ------+                |              |
|                                              |                   v              |
|                                         +---------+    +-------------------+   |
|                                         | OUTCOME | -> | EVENT STREAM    |   |
|                                         | RECORD  |    |                 |   |
|                                         +---------+    | -> M18 Feedback |   |
|                                                        | -> L5 Learning  |   |
|                                                        | -> L1 State     |   |
|                                                        +-------------------+   |
+-----------------------------------------------------------------------------------+
        ^                    ^                    ^                    |
        |                    |                    |                    v
   [M16: Executor]     [M14: Remediation]  [L1: State]         [L5 Modules]
```

---

## Core Data Structures

### Remediation Outcome (from M14)

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

### Execution Outcome with Context

```rust
/// Complete outcome record with context and metrics
#[derive(Clone, Debug)]
pub struct OutcomeRecord {
    /// Unique outcome ID
    pub outcome_id: String,

    /// Original execution request ID
    pub request_id: String,

    /// Target service ID
    pub service_id: String,

    /// Issue type that was addressed
    pub issue_type: IssueType,

    /// Severity of the issue
    pub severity: Severity,

    /// Remediation action that was executed
    pub action: RemediationAction,

    /// Whether execution succeeded
    pub success: bool,

    /// Execution duration in milliseconds
    pub duration_ms: u64,

    /// Error message if failed
    pub error: Option<String>,

    /// Confidence score used for decision
    pub confidence_score: f64,

    /// Escalation tier used
    pub escalation_tier: EscalationTier,

    /// Execution start timestamp
    pub started_at: DateTime<Utc>,

    /// Execution completion timestamp
    pub completed_at: DateTime<Utc>,

    /// Recording timestamp
    pub recorded_at: DateTime<Utc>,

    /// Hebbian pathway delta
    pub pathway_delta: f64,

    /// Outcome metrics
    pub metrics: OutcomeMetrics,

    /// Context data
    pub context: HashMap<String, String>,
}
```

### Outcome Metrics

```rust
/// Calculated metrics from outcome
#[derive(Clone, Debug)]
pub struct OutcomeMetrics {
    /// Effectiveness score (0.0-1.0)
    pub effectiveness: f64,

    /// Efficiency score (duration normalized)
    pub efficiency: f64,

    /// Whether outcome meets SLO
    pub meets_slo: bool,

    /// Impact on service health
    pub health_impact: f64,

    /// Unexpected behavior detected
    pub anomaly_detected: bool,

    /// Learning confidence for this outcome
    pub learning_confidence: f64,

    /// Recommended pathway update weight
    pub pathway_update_weight: f64,
}
```

### Outcome Statistics

```rust
/// Aggregated statistics for action type
#[derive(Clone, Debug)]
pub struct OutcomeStatistics {
    /// Action being analyzed
    pub action_type: String,

    /// Issue type context
    pub issue_type: IssueType,

    /// Total executions
    pub total_count: u64,

    /// Successful executions
    pub success_count: u64,

    /// Failed executions
    pub failure_count: u64,

    /// Success rate (0.0-1.0)
    pub success_rate: f64,

    /// Average duration in milliseconds
    pub avg_duration_ms: f64,

    /// Median duration in milliseconds
    pub median_duration_ms: f64,

    /// P99 latency in milliseconds
    pub p99_duration_ms: f64,

    /// Average effectiveness
    pub avg_effectiveness: f64,

    /// Average pathway delta
    pub avg_pathway_delta: f64,

    /// Time period covered
    pub period_start: DateTime<Utc>,

    /// Time period covered
    pub period_end: DateTime<Utc>,
}
```

---

## Public API

### Outcome Recorder Service

```rust
/// Main Outcome Recorder service
pub struct OutcomeRecorder {
    config: RecorderConfig,
    collector: OutcomeCollector,
    analyzer: OutcomeAnalyzer,
    storage: StorageManager,
    history: HistoryAggregator,
    event_emitter: EventEmitter<OutcomeEvent>,
}

impl OutcomeRecorder {
    /// Create a new OutcomeRecorder instance
    pub fn new(config: RecorderConfig) -> Self;

    /// Record a remediation outcome
    pub async fn record_outcome(
        &mut self,
        outcome: RemediationOutcome,
        context: OutcomeContext,
    ) -> Result<String, Error>;

    /// Get outcome record by ID
    pub fn get_outcome(&self, outcome_id: &str) -> Option<OutcomeRecord>;

    /// Get outcome by request ID
    pub fn get_by_request_id(&self, request_id: &str) -> Option<OutcomeRecord>;

    /// List outcomes by criteria
    pub fn list_outcomes(
        &self,
        service_id: Option<&str>,
        since: Option<DateTime<Utc>>,
        limit: usize,
    ) -> Vec<OutcomeRecord>;
}
```

### Outcome Analysis

```rust
impl OutcomeRecorder {
    /// Calculate metrics for an outcome
    pub fn calculate_metrics(
        &self,
        outcome: &RemediationOutcome,
        context: &OutcomeContext,
    ) -> OutcomeMetrics;

    /// Get statistics for action type
    pub fn get_action_statistics(
        &self,
        action_type: &str,
        lookback_days: u32,
    ) -> Result<OutcomeStatistics, Error>;

    /// Get statistics for issue type
    pub fn get_issue_statistics(
        &self,
        issue_type: IssueType,
        lookback_days: u32,
    ) -> Result<OutcomeStatistics, Error>;

    /// Analyze outcome effectiveness
    pub fn analyze_effectiveness(
        &self,
        outcome: &OutcomeRecord,
    ) -> EffectivenessAnalysis;

    /// Detect anomalies in outcomes
    pub fn detect_anomalies(
        &self,
        outcomes: &[OutcomeRecord],
    ) -> Vec<AnomalyDetection>;
}
```

### Outcome Persistence

```rust
impl OutcomeRecorder {
    /// Persist outcome to storage
    pub async fn persist_outcome(
        &mut self,
        outcome: &OutcomeRecord,
    ) -> Result<(), StorageError>;

    /// Query historical outcomes
    pub async fn query_history(
        &self,
        query: HistoryQuery,
    ) -> Result<Vec<OutcomeRecord>, Error>;

    /// Aggregate outcomes by time period
    pub async fn aggregate_by_period(
        &self,
        action: &str,
        period: AggregationPeriod,
        lookback_days: u32,
    ) -> Result<Vec<OutcomeStatistics>, Error>;

    /// Export outcomes to file
    pub async fn export_outcomes(
        &self,
        path: &str,
        since: DateTime<Utc>,
    ) -> Result<(), Error>;
}
```

---

## Configuration

### TOML Configuration Example

```toml
[layer.L3.M17]
enabled = true
version = "1.0.0"

# Storage configuration
[layer.L3.M17.storage]
database = "sqlite:data/outcomes.db"
enable_persistence = true
enable_backup = true
backup_interval_hours = 24
retention_days = 90
compression_enabled = true

# Outcome processing
[layer.L3.M17.processing]
batch_size = 100
batch_timeout_ms = 5000
calculate_metrics = true
detect_anomalies = true
anomaly_threshold_stddev = 3.0

# Pathway delta calculation
[layer.L3.M17.pathway_delta]
success_delta = 0.1
partial_success_delta = 0.05
failure_delta = -0.05
baseline_weight = 0.5

# Effectiveness calculation
[layer.L3.M17.effectiveness]
success_score = 1.0
partial_success_score = 0.5
failure_score = 0.0
duration_weight = 0.2
latency_threshold_ms = 5000

# History aggregation
[layer.L3.M17.history]
enable_aggregation = true
aggregation_period = "hourly"
retention_granular_days = 7
retention_aggregate_days = 90
min_samples = 5

# Event configuration
[layer.L3.M17.events]
emit_outcome_recorded = true
emit_statistics_updated = true
emit_anomaly_detected = true
emit_threshold_violations = true
batch_emission = true
batch_window_ms = 1000

# Analysis settings
[layer.L3.M17.analysis]
enable_trend_detection = true
enable_correlation_detection = true
trend_window_size = 100
correlation_threshold = 0.7
```

---

## Bi-Directional Data Flow

### Inbound Data Flow

```rust
/// Messages received by Outcome Recorder
#[derive(Debug, Clone)]
pub enum OutcomeRecorderInbound {
    // From M16 Action Executor
    ExecutionResult {
        request_id: String,
        action: RemediationAction,
        success: bool,
        duration_ms: u64,
        error: Option<String>,
    },

    // From M14 Remediation Engine
    RemediationOutcome {
        request_id: String,
        outcome: RemediationOutcome,
    },

    // From M15 Confidence Calculator
    ConfidenceMetrics {
        request_id: String,
        confidence_score: f64,
    },

    // From M16 with extended context
    ExecutionContext {
        request_id: String,
        service_id: String,
        issue_type: IssueType,
        severity: Severity,
        escalation_tier: EscalationTier,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
    },

    // From L5 Learning
    PathwayFeedback {
        action_type: String,
        effectiveness_update: f64,
    },
}
```

### Outbound Data Flow

```rust
/// Messages emitted by Outcome Recorder
#[derive(Debug, Clone)]
pub enum OutcomeRecorderOutbound {
    // To M18 Feedback Loop
    OutcomeRecorded {
        outcome_id: String,
        request_id: String,
        success: bool,
        metrics: OutcomeMetrics,
    },

    StatisticsUpdated {
        action_type: String,
        issue_type: IssueType,
        statistics: OutcomeStatistics,
    },

    // To L5 Learning
    LearningEvent {
        outcome_id: String,
        action: RemediationAction,
        issue_type: IssueType,
        success: bool,
        pathway_delta: f64,
        effectiveness: f64,
    },

    // To L1 State Persistence
    OutcomeSnapshot {
        timestamp: DateTime<Utc>,
        outcomes: Vec<OutcomeRecord>,
    },

    // To subscribers
    OutcomeEvent {
        outcome_id: String,
        timestamp: DateTime<Utc>,
        outcome: OutcomeRecord,
    },

    AnomalyDetected {
        outcome_id: String,
        anomaly: AnomalyDetection,
        severity: String,
    },
}
```

---

## Metrics

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `me_m17_outcomes_recorded_total` | Counter | action, issue_type, result | Total outcomes recorded |
| `me_m17_outcome_duration_ms` | Histogram | action | Duration distribution |
| `me_m17_action_success_rate` | Gauge | action, issue_type | Success rate by action |
| `me_m17_action_effectiveness` | Gauge | action | Effectiveness score |
| `me_m17_pathway_delta_avg` | Gauge | action | Average pathway change |
| `me_m17_storage_records` | Gauge | table | Records in storage |
| `me_m17_storage_size_bytes` | Gauge | database | Database size |
| `me_m17_anomalies_detected` | Counter | action, type | Anomaly count |
| `me_m17_statistics_updated` | Counter | period | Statistics updates |
| `me_m17_query_latency_ms` | Histogram | query_type | Query response time |

---

## Error Codes

| Code | Name | Severity | Description | Recovery |
|------|------|----------|-------------|----------|
| E17001 | InvalidOutcome | Warning | Outcome data invalid | Reject and log |
| E17002 | StorageWriteFailed | Error | Cannot persist outcome | Retry with backoff |
| E17003 | DatabaseConnectionError | Error | Database connection lost | Reconnect and retry |
| E17004 | MetricsCalculationError | Warning | Cannot calculate metrics | Use defaults |
| E17005 | AnalysisError | Warning | Analysis computation failed | Skip analysis |
| E17006 | AnomalyDetectionFailed | Warning | Cannot detect anomalies | Continue without |
| E17007 | HistoryQueryError | Error | Cannot query history | Return empty result |
| E17008 | AggregationError | Warning | Cannot aggregate outcomes | Use raw data |
| E17009 | StorageQuotaExceeded | Critical | Storage full | Archive old data |
| E17010 | CorruptedOutcomeData | Critical | Data integrity error | Quarantine record |

---

## Related Modules

- **M14_REMEDIATION_ENGINE**: Provides initial outcome from remediation request
- **M16_ACTION_EXECUTOR**: Provides execution results
- **M18_FEEDBACK_LOOP**: Consumes outcome data for learning
- **M15_CONFIDENCE_CALCULATOR**: Provides confidence scores
- **M05_STATE_PERSISTENCE**: Stores outcome snapshots
- **L5_HEBBIAN_ENGINE**: Learns from outcomes and adjusts pathways
- **L5_LEARNING**: Uses outcomes for pattern recognition

---

## Navigation

| Direction | Link |
|-----------|------|
| Up | [INDEX.md](../INDEX.md) |
| Layer | [L03_CORE_LOGIC.md](../layers/L03_CORE_LOGIC.md) |
| Previous | [M16_ACTION_EXECUTOR.md](M16_ACTION_EXECUTOR.md) |
| Next | [M18_FEEDBACK_LOOP.md](M18_FEEDBACK_LOOP.md) |
| Related | [M15_CONFIDENCE_CALCULATOR.md](M15_CONFIDENCE_CALCULATOR.md) |
| API | [REST_API.md](../integration/REST_API.md) |

---

*[Back to Index](../INDEX.md) | [Layer: L03 Core Logic](../layers/L03_CORE_LOGIC.md)*
