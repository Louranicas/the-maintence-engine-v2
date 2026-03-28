# Module M18: Feedback Loop

> **M18_FEEDBACK_LOOP** | Learning Feedback | Layer: L3 Core Logic | [Back to Index](../INDEX.md)

---

## Navigation

| Direction | Link |
|-----------|------|
| Up | [INDEX.md](../INDEX.md) |
| Layer | [L03_CORE_LOGIC.md](../layers/L03_CORE_LOGIC.md) |
| Related | [M17_OUTCOME_RECORDER.md](M17_OUTCOME_RECORDER.md) |
| Related | [M15_CONFIDENCE_CALCULATOR.md](M15_CONFIDENCE_CALCULATOR.md) |
| Related | [M14_REMEDIATION_ENGINE.md](M14_REMEDIATION_ENGINE.md) |
| Pattern | [PATTERN_FEEDBACK.md](../patterns/PATTERN_FEEDBACK.md) |
| L5 Learning | [L03_LEARNING.md](../layers/L03_LEARNING.md) |
| L5 STDP | [M26_STDP_LEARNING.md](../modules/M26_STDP_LEARNING.md) |

---

## Module Specification

### Overview

The Feedback Loop module implements closed-loop learning by consuming remediation outcomes and feeding them back into learning systems. It calculates effectiveness scores, determines Hebbian pathway updates, triggers confidence recalibration, and coordinates learning across Hebbian pathways, pattern recognition, and adaptive systems to continuously improve remediation decision-making.

### Module Properties

| Property | Value |
|----------|-------|
| Module ID | M18 |
| Module Name | Feedback Loop |
| Layer | L3 (Core Logic) |
| Version | 1.0.0 |
| Dependencies | M14 (Remediation), M15 (Confidence), M17 (Recorder) |
| Dependents | L5 (Learning), L5 (STDP), L5 (Homeostatic), L5 (Patterns) |

---

## Architecture Diagram

```
+-----------------------------------------------------------------------------------+
|                      M18: FEEDBACK LOOP                                         |
+-----------------------------------------------------------------------------------+
|                                                                                   |
|  +------------------------+    +------------------------+    +--------------+   |
|  | OUTCOME INTERPRETER    |    | EFFECTIVENESS SCORER   |    | PATHWAY        |   |
|  |                        |    |                        |    | UPDATER        |   |
|  | - Parse outcomes       |    | - Calculate scores     |    |                |   |
|  | - Extract signals      |--->| - Normalize results    |--->| - Weight delta |   |
|  | - Classify patterns    |    | - Compare to baseline  |    | - LTP/LTD      |   |
|  | - Detect anomalies     |    | - Quality assessment   |    | - Strength mod |   |
|  +--------+----------------+    +--------+---------------+    +--------------+   |
|           |                              |                           |            |
|           v                              v                           v            |
|  +------------------------+    +------------------------+    +---------+     |
|  | LEARNING COORDINATOR   |    | CONFIDENCE CALIBRATOR  |    |FEEDBACK |     |
|  |                        |    |                        |    |EVENT    |     |
|  | - Route to L5 modules  |    | - Update success rates |    |EMITTER  |     |
|  | - Parallel learning    |--->| - Adjust thresholds    |--->| - Events|     |
|  | - Consensus learning   |    | - Retrain models       |    | - Alerts|     |
|  | - Feedback synthesis   |    | - Recalibrate weights  |    | - Metrics|    |
|  +--------+----------------+    +--------+---------------+    +--+------+     |
|           |                              |                       |              |
|           +--------- Coordinate and Learn ------+                |              |
|                                              |                   v              |
|                                         +---------+    +-------------------+   |
|                                         | LEARNING|    | EVENT STREAM    |   |
|                                         | SIGNALS  |    |                 |   |
|                                         +---------+    | -> L5 STDP      |   |
|                                                        | -> L5 Homeostatic
|                                                        | -> L5 Patterns  |   |
|                                                        | -> M15 Conf Cal |   |
|                                                        +-------------------+   |
+-----------------------------------------------------------------------------------+
        ^                    ^                    ^                    |
        |                    |                    |                    v
   [M17: Recorder]     [M14: Remediation]  [M15: Confidence]   [L5 Modules]
```

---

## Core Data Structures

### Feedback Signal

```rust
/// Signal fed back to learning systems
#[derive(Clone, Debug)]
pub struct FeedbackSignal {
    /// Unique feedback ID
    pub feedback_id: String,

    /// Original outcome record ID
    pub outcome_id: String,

    /// Remediation action that was executed
    pub action: RemediationAction,

    /// Issue type addressed
    pub issue_type: IssueType,

    /// Whether outcome was successful
    pub success: bool,

    /// Effectiveness score (0.0-1.0)
    pub effectiveness: f64,

    /// Efficiency score (normalized duration)
    pub efficiency: f64,

    /// Hebbian pathway delta for LTP/LTD
    pub pathway_delta: f64,

    /// Confidence accuracy (was confidence prediction correct?)
    pub confidence_accuracy: f64,

    /// Pattern relevance for learning systems
    pub pattern_relevance: f64,

    /// Signal timestamp
    pub timestamp: DateTime<Utc>,
}
```

### Learning Recommendation

```rust
/// Recommendation for learning system updates
#[derive(Clone, Debug)]
pub struct LearningRecommendation {
    /// Recommendation ID
    pub recommendation_id: String,

    /// Target learning system (STDP, Homeostatic, Pattern, etc.)
    pub target_system: LearningSystem,

    /// Type of update (Strengthen, Weaken, Add, Remove)
    pub update_type: UpdateType,

    /// Entity being updated (pathway, pattern, weight, etc.)
    pub entity: LearningEntity,

    /// Magnitude of recommended change
    pub magnitude: f64,

    /// Supporting evidence/feedback signals
    pub supporting_signals: Vec<String>,

    /// Priority (1 = highest)
    pub priority: u8,

    /// Recommended confidence for change
    pub confidence: f64,

    /// Timestamp of recommendation
    pub timestamp: DateTime<Utc>,
}

pub enum LearningSystem {
    StdpLearning,
    HomeostasisControl,
    PatternRecognition,
    ConfidenceCalibration,
    EpisodicMemory,
    AdaptiveControl,
}

pub enum UpdateType {
    Strengthen,
    Weaken,
    Add,
    Remove,
    Calibrate,
    Reset,
}

pub enum LearningEntity {
    Pathway(String),
    Pattern(String),
    Weight(String),
    Threshold(String),
    Model(String),
    Parameter(String),
}
```

### Feedback Statistics

```rust
/// Aggregated feedback statistics
#[derive(Clone, Debug)]
pub struct FeedbackStatistics {
    /// Period being analyzed
    pub period_start: DateTime<Utc>,
    pub period_end: DateTime<Utc>,

    /// Total feedback signals processed
    pub total_signals: u64,

    /// Average effectiveness across signals
    pub avg_effectiveness: f64,

    /// Average confidence accuracy
    pub avg_confidence_accuracy: f64,

    /// Standard deviation of effectiveness
    pub effectiveness_stddev: f64,

    /// Number of anomalies detected
    pub anomaly_count: u64,

    /// Learning rate estimate
    pub learning_rate: f64,

    /// Convergence status
    pub converged: bool,

    /// Recommended actions
    pub recommendations: Vec<LearningRecommendation>,
}
```

---

## Public API

### Feedback Loop Service

```rust
/// Main Feedback Loop service
pub struct FeedbackLoop {
    config: FeedbackLoopConfig,
    interpreter: OutcomeInterpreter,
    effectiveness_scorer: EffectivenessScorer,
    pathway_updater: PathwayUpdater,
    coordinator: LearningCoordinator,
    calibrator: ConfidenceCalibrator,
    event_emitter: EventEmitter<FeedbackEvent>,
}

impl FeedbackLoop {
    /// Create a new FeedbackLoop instance
    pub fn new(config: FeedbackLoopConfig) -> Self;

    /// Process an outcome and generate feedback signals
    pub async fn process_outcome(
        &mut self,
        outcome: &OutcomeRecord,
    ) -> Result<FeedbackSignal, Error>;

    /// Get feedback signal by ID
    pub fn get_feedback(&self, feedback_id: &str) -> Option<FeedbackSignal>;

    /// List recent feedback signals
    pub fn list_feedback(
        &self,
        since: DateTime<Utc>,
        limit: usize,
    ) -> Vec<FeedbackSignal>;

    /// Get feedback statistics
    pub fn get_statistics(
        &self,
        period_start: DateTime<Utc>,
        period_end: DateTime<Utc>,
    ) -> Result<FeedbackStatistics, Error>;
}
```

### Outcome Processing

```rust
impl FeedbackLoop {
    /// Interpret outcome as feedback signal
    pub fn interpret_outcome(
        &self,
        outcome: &OutcomeRecord,
    ) -> Result<FeedbackSignal, Error>;

    /// Extract learning signals from outcome
    pub fn extract_signals(
        &self,
        outcome: &OutcomeRecord,
    ) -> Vec<LearningSignal>;

    /// Detect anomalies in outcome
    pub fn detect_anomaly(
        &self,
        outcome: &OutcomeRecord,
    ) -> Option<AnomalyDetection>;

    /// Classify outcome pattern
    pub fn classify_pattern(
        &self,
        outcome: &OutcomeRecord,
    ) -> Option<PatternClassification>;
}
```

### Effectiveness Scoring

```rust
impl FeedbackLoop {
    /// Calculate effectiveness score (0.0-1.0)
    pub fn calculate_effectiveness(
        &self,
        outcome: &OutcomeRecord,
    ) -> f64;

    /// Calculate efficiency score
    pub fn calculate_efficiency(
        &self,
        duration_ms: u64,
        severity: Severity,
    ) -> f64;

    /// Compare to baseline effectiveness
    pub fn compare_to_baseline(
        &self,
        effectiveness: f64,
        action: &RemediationAction,
    ) -> EffectivenessComparison;

    /// Assess score quality
    pub fn assess_score_quality(
        &self,
        signal: &FeedbackSignal,
    ) -> ScoreQuality;
}
```

### Learning Coordination

```rust
impl FeedbackLoop {
    /// Generate learning recommendations
    pub fn generate_recommendations(
        &self,
        signal: &FeedbackSignal,
    ) -> Vec<LearningRecommendation>;

    /// Route feedback to L5 learning systems
    pub async fn route_feedback(
        &self,
        signal: &FeedbackSignal,
    ) -> Result<(), Error>;

    /// Coordinate parallel learning updates
    pub async fn coordinate_learning(
        &self,
        signals: &[FeedbackSignal],
    ) -> Result<LearningResult, Error>;

    /// Synthesize learning from multiple signals
    pub fn synthesize_learning(
        &self,
        signals: &[FeedbackSignal],
    ) -> SynthesizedLearning;
}
```

### Confidence Calibration

```rust
impl FeedbackLoop {
    /// Update confidence model based on feedback
    pub async fn calibrate_confidence(
        &self,
        signals: &[FeedbackSignal],
    ) -> Result<CalibrationResult, Error>;

    /// Calculate confidence prediction accuracy
    pub fn calculate_confidence_accuracy(
        &self,
        predicted_confidence: f64,
        actual_outcome: bool,
    ) -> f64;

    /// Adjust confidence thresholds
    pub fn adjust_thresholds(
        &self,
        stats: &FeedbackStatistics,
    ) -> ThresholdAdjustment;

    /// Recommend confidence model retraining
    pub fn recommend_retraining(
        &self,
        stats: &FeedbackStatistics,
    ) -> bool;
}
```

---

## Configuration

### TOML Configuration Example

```toml
[layer.L3.M18]
enabled = true
version = "1.0.0"

# Feedback processing settings
[layer.L3.M18.processing]
enable_feedback_processing = true
batch_size = 50
batch_timeout_ms = 5000
auto_calibration_enabled = true
learning_coordination_enabled = true

# Effectiveness scoring
[layer.L3.M18.effectiveness]
success_baseline = 1.0
partial_success_baseline = 0.5
failure_baseline = 0.0
duration_normalization_threshold_ms = 5000
anomaly_detection_enabled = true
anomaly_threshold_stddev = 3.0

# Efficiency scoring
[layer.L3.M18.efficiency]
fast_execution_threshold_ms = 1000
slow_execution_threshold_ms = 10000
fast_execution_score = 1.0
normal_execution_score = 0.8
slow_execution_score = 0.5

# Pathway updates (Hebbian STDP)
[layer.L3.M18.pathway_updates]
enable_ltp = true
enable_ltd = true
ltp_rate = 0.1
ltd_rate = 0.05
stdp_window_ms = 100
weight_decay_rate = 0.001

# Confidence calibration
[layer.L3.M18.calibration]
enable_auto_calibration = true
calibration_interval_hours = 1
min_signals_for_calibration = 10
adjust_thresholds = true
retrain_model = true
recalibration_threshold = 0.1

# Learning recommendations
[layer.L3.M18.recommendations]
enable_recommendations = true
min_confidence_threshold = 0.7
recommend_ltp = true
recommend_ltd = true
recommend_pattern_update = true
recommend_confidence_adjust = true

# Anomaly detection
[layer.L3.M18.anomalies]
enable_anomaly_detection = true
detection_method = "statistical"
sensitivity = "medium"
notification_on_anomaly = true

# Learning routing
[layer.L3.M18.routing]
route_to_stdp = true
route_to_homeostatic = true
route_to_patterns = true
route_to_episodic = true
parallel_learning = true
consensus_learning = false

# Feedback event emission
[layer.L3.M18.events]
emit_signals = true
emit_recommendations = true
emit_statistics = true
emit_anomalies = true
batch_events = true
event_batch_window_ms = 1000

# Statistics aggregation
[layer.L3.M18.statistics]
enable_aggregation = true
aggregation_period = "hourly"
retention_days = 30
convergence_threshold = 0.05
```

---

## Bi-Directional Data Flow

### Inbound Data Flow

```rust
/// Messages received by Feedback Loop
#[derive(Debug, Clone)]
pub enum FeedbackLoopInbound {
    // From M17 Outcome Recorder
    OutcomeRecorded {
        outcome_id: String,
        outcome: OutcomeRecord,
        metrics: OutcomeMetrics,
    },

    // From M15 Confidence Calculator
    ConfidencePrediction {
        request_id: String,
        predicted_confidence: f64,
        actual_outcome: Option<bool>,
    },

    // From L5 Learning Pathways
    PathwayStrengthRequest {
        action_type: String,
        current_strength: f64,
    },

    // From L5 STDP Learning
    StdpUpdateResult {
        pathway_id: String,
        weight_change: f64,
        ltp_count: u32,
        ltd_count: u32,
    },
}
```

### Outbound Data Flow

```rust
/// Messages emitted by Feedback Loop
#[derive(Debug, Clone)]
pub enum FeedbackLoopOutbound {
    // To L5 STDP Learning
    FeedbackSignal {
        feedback_id: String,
        outcome_id: String,
        action: RemediationAction,
        success: bool,
        effectiveness: f64,
        pathway_delta: f64,
    },

    // To L5 Homeostatic Control
    HomeostasisSignal {
        metric_type: String,
        deviation: f64,
        correction_direction: String,
    },

    // To L5 Pattern Recognition
    PatternFeedback {
        outcome_id: String,
        pattern_id: Option<String>,
        pattern_relevance: f64,
        should_update: bool,
    },

    // To M15 Confidence Calculator
    CalibrationUpdate {
        new_threshold_l0: f64,
        new_threshold_l1: f64,
        new_threshold_l2: f64,
        model_accuracy: f64,
    },

    // To subscribers
    FeedbackEvent {
        feedback_id: String,
        signal: FeedbackSignal,
        timestamp: DateTime<Utc>,
    },

    StatisticsUpdated {
        period: String,
        stats: FeedbackStatistics,
    },

    AnomalyDetected {
        outcome_id: String,
        anomaly_type: String,
        severity: String,
    },
}
```

---

## Metrics

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `me_m18_feedback_signals_total` | Counter | action, result | Total feedback signals |
| `me_m18_effectiveness_avg` | Gauge | action, issue_type | Average effectiveness |
| `me_m18_confidence_accuracy` | Gauge | confidence_bucket | Prediction accuracy by confidence level |
| `me_m18_learning_recommendations_total` | Counter | system, update_type | Recommendations generated |
| `me_m18_pathway_delta_applied` | Gauge | pathway_id | Pathway weight changes |
| `me_m18_ltp_events_total` | Counter | pathway | Long-Term Potentiation events |
| `me_m18_ltd_events_total` | Counter | pathway | Long-Term Depression events |
| `me_m18_calibration_updates_total` | Counter | metric | Calibration updates |
| `me_m18_anomalies_detected` | Counter | type | Anomalies found |
| `me_m18_processing_latency_ms` | Histogram | operation | Processing time |

---

## Error Codes

| Code | Name | Severity | Description | Recovery |
|------|------|----------|-------------|----------|
| E18001 | InvalidOutcome | Warning | Outcome data invalid | Skip feedback |
| E18002 | EffectivenessCalculationError | Warning | Cannot calculate effectiveness | Use default |
| E18003 | LearningRoutingFailed | Error | Cannot route to L5 system | Retry with backoff |
| E18004 | CalibrationFailed | Error | Confidence calibration failed | Use previous model |
| E18005 | AnomalyDetectionError | Warning | Cannot detect anomalies | Continue without |
| E18006 | PathwayUpdateFailed | Error | Cannot update pathways | Manual review |
| E18007 | PatternClassificationError | Warning | Cannot classify pattern | Use default |
| E18008 | SynthesisError | Warning | Cannot synthesize learning | Use individual signals |
| E18009 | StatisticsComputationError | Warning | Cannot compute statistics | Use partial data |
| E18010 | EventEmissionFailed | Warning | Cannot emit feedback event | Log and continue |

---

## Related Modules

- **M14_REMEDIATION_ENGINE**: Generates actions that produce outcomes
- **M15_CONFIDENCE_CALCULATOR**: Gets recalibrated based on feedback
- **M17_OUTCOME_RECORDER**: Provides outcomes to process
- **L5_STDP_LEARNING**: Receives LTP/LTD signals for pathway learning
- **L5_HOMEOSTATIC_CONTROL**: Receives homeostasis signals
- **L5_PATTERN_RECOGNITION**: Updates learned patterns
- **L5_EPISODIC_MEMORY**: Records episodes for future reference
- **L5_ADAPTIVE_LEARNING**: General learning system updates

---

## Navigation

| Direction | Link |
|-----------|------|
| Up | [INDEX.md](../INDEX.md) |
| Layer | [L03_CORE_LOGIC.md](../layers/L03_CORE_LOGIC.md) |
| Previous | [M17_OUTCOME_RECORDER.md](M17_OUTCOME_RECORDER.md) |
| Related | [M15_CONFIDENCE_CALCULATOR.md](M15_CONFIDENCE_CALCULATOR.md) |
| Learning | [L03_LEARNING.md](../layers/L03_LEARNING.md) |
| API | [REST_API.md](../integration/REST_API.md) |

---

*[Back to Index](../INDEX.md) | [Layer: L03 Core Logic](../layers/L03_CORE_LOGIC.md)*
