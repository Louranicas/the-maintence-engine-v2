# Module M15: Confidence Calculator

> **M15_CONFIDENCE_CALCULATOR** | Action Confidence Scoring | Layer: L3 Core Logic | [Back to Index](../INDEX.md)

---

## Navigation

| Direction | Link |
|-----------|------|
| Up | [INDEX.md](../INDEX.md) |
| Layer | [L03_CORE_LOGIC.md](../layers/L03_CORE_LOGIC.md) |
| Related | [M14_REMEDIATION_ENGINE.md](M14_REMEDIATION_ENGINE.md) |
| Related | [M16_ACTION_EXECUTOR.md](M16_ACTION_EXECUTOR.md) |
| Related | [M17_OUTCOME_RECORDER.md](M17_OUTCOME_RECORDER.md) |
| Pattern | [PATTERN_CONFIDENCE.md](../patterns/PATTERN_CONFIDENCE.md) |
| L3 Learning | [L03_LEARNING.md](../layers/L03_LEARNING.md) |
| L4 Integration | [L04_INTEGRATION.md](../layers/L04_INTEGRATION.md) |

---

## Module Specification

### Overview

The Confidence Calculator module computes action confidence scores using a weighted multi-factor formula. It synthesizes historical success rates, pattern match strength, severity assessment, Hebbian pathway weights, and temporal factors to produce confidence scores (0.0-1.0) that drive escalation tier determination and remediation decision-making.

### Module Properties

| Property | Value |
|----------|-------|
| Module ID | M15 |
| Module Name | Confidence Calculator |
| Layer | L3 (Core Logic) |
| Version | 1.0.0 |
| Dependencies | M04 (Metrics), M13 (Pipeline), M14 (Remediation) |
| Dependents | M16 (Executor), M17 (Recorder), M18 (Feedback), L5 (Learning) |

---

## Architecture Diagram

```
+-----------------------------------------------------------------------------------+
|                  M15: CONFIDENCE CALCULATOR                                     |
+-----------------------------------------------------------------------------------+
|                                                                                   |
|  +------------------------+    +------------------------+    +--------------+   |
|  | HISTORICAL ANALYZER    |    | PATTERN MATCHER        |    | FACTOR SCORER|   |
|  |                        |    |                        |    |              |   |
|  | - Success/failure rate |    | - Issue pattern match  |    | - Severity   |   |
|  | - Action effectiveness |--->| - Temporal patterns    |--->| - Pathway    |   |
|  | - Time series analysis |    | - Context similarity   |    | - Temporal   |   |
|  +--------+----------------+    +--------+---------------+    +--------------+   |
|           |                              |                           |            |
|           v                              v                           v            |
|  +------------------------+    +------------------------+    +---------+     |
|  | SCORE AGGREGATOR       |    | CONFIDENCE COMPOSER    |    |VALIDATORS|     |
|  |                        |    |                        |    |          |     |
|  | - Weight application   |    | - Weighted combination |    | - Range  |     |
|  | - Normalization        |--->| - Bounds checking      |--->| - Sanity |     |
|  | - Factor combination   |    | - Quality scoring      |    | - Thresh |     |
|  +--------+----------------+    +--------+---------------+    +--+------+     |
|           |                              |                       |              |
|           +--------- Iterate to precision -----+                 |              |
|                                              |                   v              |
|                                         +---------+    +-------------------+   |
|                                         | CONFIDENCE | -> | EVENT STREAM    |   |
|                                         | (0.0-1.0)  |    |                 |   |
|                                         +---------+    | -> M16 Executor |   |
|                                                        | -> M14 Remediator|   |
|                                                        +-------------------+   |
+-----------------------------------------------------------------------------------+
        ^                    ^                    ^                    |
        |                    |                    |                    v
   [L1: Metrics]       [L5: Pathways]       [L3: History]        [L3 Modules]
```

---

## Core Data Structures

### Confidence Factors and Weights

```rust
/// Individual factors contributing to confidence score
#[derive(Clone, Debug)]
pub struct ConfidenceFactors {
    /// Historical success rate for this action (0.0-1.0)
    /// Weight: 0.3 (30%)
    pub historical_success_rate: f64,

    /// Strength of pattern match (0.0-1.0)
    /// Weight: 0.25 (25%)
    pub pattern_match_strength: f64,

    /// Severity-based score (higher severity = lower confidence)
    /// Weight: 0.2 (20%)
    pub severity_score: f64,

    /// Hebbian pathway weight from learning
    /// Weight: 0.15 (15%)
    pub pathway_weight: f64,

    /// Temporal factor (recency bias)
    /// Weight: 0.1 (10%)
    pub time_factor: f64,
}

impl ConfidenceFactors {
    /// Validate all factors are in [0.0, 1.0] range
    pub fn validate(&self) -> Result<(), ValidationError>;

    /// Get human-readable breakdown
    pub fn to_breakdown(&self) -> ConfidenceBreakdown;
}

/// Confidence score calculation weights
pub const CONFIDENCE_WEIGHTS: &[f64] = &[
    0.30,  // historical_success_rate
    0.25,  // pattern_match_strength
    0.20,  // severity_score
    0.15,  // pathway_weight
    0.10,  // time_factor
];
```

### Confidence Score

```rust
/// Computed confidence score with metadata
#[derive(Clone, Debug)]
pub struct ConfidenceScore {
    /// The final confidence value (0.0-1.0)
    pub score: f64,

    /// Breakdown of contributing factors
    pub factors: ConfidenceFactors,

    /// Individual component scores
    pub components: Vec<(String, f64)>,

    /// Confidence quality (high/medium/low)
    pub quality: ScoreQuality,

    /// Whether score meets thresholds for auto-execute
    pub auto_executable: bool,

    /// Whether score suggests escalation
    pub requires_escalation: bool,

    /// Timestamp of calculation
    pub timestamp: DateTime<Utc>,

    /// Source of confidence assessment
    pub source: ConfidenceSource,
}

pub enum ScoreQuality {
    /// High quality - based on sufficient historical data
    High,
    /// Medium quality - based on partial pattern matching
    Medium,
    /// Low quality - insufficient data, high uncertainty
    Low,
}

pub enum ConfidenceSource {
    Historical,
    PatternMatching,
    Learning,
    Expert,
    Ensemble,
}
```

### Confidence Breakdown

```rust
/// Human-readable confidence score breakdown
#[derive(Clone, Debug)]
pub struct ConfidenceBreakdown {
    /// Overall confidence score
    pub overall_score: f64,

    /// Historical analysis results
    pub historical: HistoricalAnalysis,

    /// Pattern matching results
    pub pattern: PatternMatchResult,

    /// Severity impact
    pub severity: SeverityImpact,

    /// Pathway strength from learning
    pub pathway: PathwayStrength,

    /// Temporal factors
    pub temporal: TemporalFactors,

    /// Contributing conditions
    pub conditions: Vec<String>,

    /// Risk factors
    pub risks: Vec<String>,

    /// Recommendations
    pub recommendations: Vec<String>,
}
```

---

## Public API

### Confidence Calculator Service

```rust
/// Main Confidence Calculator service
pub struct ConfidenceCalculator {
    config: CalculatorConfig,
    historical: HistoricalAnalyzer,
    pattern_matcher: PatternMatcher,
    factor_scorer: FactorScorer,
    aggregator: ScoreAggregator,
    validator: ScoreValidator,
    event_emitter: EventEmitter<ConfidenceEvent>,
}

impl ConfidenceCalculator {
    /// Create a new ConfidenceCalculator instance
    pub fn new(config: CalculatorConfig) -> Self;

    /// Calculate confidence for a remediation action
    pub fn calculate_confidence(
        &self,
        historical_success_rate: f64,
        pattern_match_strength: f64,
        severity_score: f64,
        pathway_weight: f64,
        time_factor: f64,
    ) -> f64;

    /// Calculate with full breakdown
    pub fn calculate_with_breakdown(
        &self,
        factors: ConfidenceFactors,
    ) -> Result<ConfidenceScore, Error>;

    /// Get confidence for cached action
    pub fn get_cached_confidence(
        &self,
        issue_type: IssueType,
        action: RemediationAction,
    ) -> Option<ConfidenceScore>;
}
```

### Factor Analysis

```rust
impl ConfidenceCalculator {
    /// Analyze historical success rate
    pub fn analyze_historical(
        &self,
        action: &RemediationAction,
        lookback_days: u32,
    ) -> Result<HistoricalAnalysis, Error>;

    /// Match against known patterns
    pub fn match_patterns(
        &self,
        issue_type: IssueType,
        context: &HashMap<String, String>,
    ) -> Result<PatternMatchResult, Error>;

    /// Score severity impact on confidence
    pub fn score_severity(
        &self,
        severity: Severity,
    ) -> f64;

    /// Get pathway weight from learning
    pub fn get_pathway_weight(
        &self,
        action_type: &str,
        context: &str,
    ) -> f64;

    /// Calculate temporal factor (recency bias)
    pub fn calculate_temporal_factor(
        &self,
        last_success: Option<DateTime<Utc>>,
        last_failure: Option<DateTime<Utc>>,
    ) -> f64;
}
```

### Confidence Composition

```rust
impl ConfidenceCalculator {
    /// Compose factors into confidence score
    pub fn compose_confidence(
        &self,
        factors: ConfidenceFactors,
    ) -> Result<f64, Error>;

    /// Apply weighted combination formula
    pub fn apply_weights(
        &self,
        components: &[f64],
        weights: &[f64],
    ) -> f64;

    /// Clamp score to valid range [0.0, 1.0]
    pub fn clamp_score(&self, score: f64) -> f64;

    /// Get breakdown text representation
    pub fn get_breakdown_text(&self, score: &ConfidenceScore) -> String;

    /// Explain confidence score to operators
    pub fn explain_score(&self, score: &ConfidenceScore) -> ConfidenceExplanation;
}
```

---

## Confidence Calculation Algorithm

```rust
/// Calculate confidence score for a remediation action
#[must_use]
pub fn calculate_confidence(
    historical_success_rate: f64,
    pattern_match_strength: f64,
    severity_score: f64,
    pathway_weight: f64,
    time_factor: f64,
) -> f64 {
    // Validate all inputs are in [0.0, 1.0]
    let inputs = [
        historical_success_rate,
        pattern_match_strength,
        severity_score,
        pathway_weight,
        time_factor,
    ];

    for input in inputs {
        assert!((0.0..=1.0).contains(&input),
                "Input must be in [0.0, 1.0] range");
    }

    // Weighted confidence formula
    let confidence = (0.3 * historical_success_rate)
        + (0.25 * pattern_match_strength)
        + (0.2 * severity_score)
        + (0.15 * pathway_weight)
        + (0.1 * time_factor);

    // Clamp result to valid range
    confidence.clamp(0.0, 1.0)
}
```

### Factor Computation Examples

```rust
// Historical Success Rate
// If action succeeded 9 times out of 10: 0.9

// Pattern Match Strength
// Similarity to known patterns: 0.8 (80% match)

// Severity Score
// Critical severity reduces confidence
// Severity::Critical -> 0.4
// Severity::High -> 0.6
// Severity::Medium -> 0.8
// Severity::Low -> 0.95

// Pathway Weight (from Hebbian Learning)
// Strong pathway (frequently used, high effectiveness): 0.85
// Weak pathway (rarely used or inconsistent): 0.4

// Time Factor (Recency Bias)
// Last successful 1 hour ago: 0.95 (very recent)
// Last successful 1 day ago: 0.75 (recent)
// Last successful 1 week ago: 0.5 (stale)
// No success history: 0.3 (very uncertain)
```

---

## Configuration

### TOML Configuration Example

```toml
[layer.L3.M15]
enabled = true
version = "1.0.0"

# Confidence calculation parameters
[layer.L3.M15.calculation]
historical_weight = 0.30
pattern_weight = 0.25
severity_weight = 0.20
pathway_weight = 0.15
temporal_weight = 0.10

# Quality thresholds
[layer.L3.M15.quality]
high_quality_threshold = 0.85
medium_quality_threshold = 0.60
low_quality_threshold = 0.0
min_data_points_high_quality = 20
min_data_points_medium_quality = 5

# Escalation thresholds
[layer.L3.M15.escalation]
auto_execute_threshold = 0.9
notify_human_threshold = 0.7
require_approval_threshold = 0.5
pbft_consensus_threshold = 0.95

# Caching
[layer.L3.M15.caching]
enable_result_caching = true
cache_ttl_seconds = 300
cache_size = 1000

# Historical analysis
[layer.L3.M15.historical]
lookback_days = 30
min_samples = 5
discount_old_data = true
discount_rate = 0.95

# Pattern matching
[layer.L3.M15.patterns]
similarity_threshold = 0.75
context_weight = 0.5
temporal_pattern_weight = 0.3

# Severity scoring
[layer.L3.M15.severity]
critical_base_score = 0.4
high_base_score = 0.6
medium_base_score = 0.8
low_base_score = 0.95

# Pathway integration
[layer.L3.M15.pathways]
use_hebbian_weights = true
apply_normalization = true
min_pathway_strength = 0.1

# Temporal factors
[layer.L3.M15.temporal]
use_recency_bias = true
recency_halflife_hours = 24
decay_rate = 0.99
min_temporal_score = 0.2
max_temporal_score = 0.99
```

---

## Bi-Directional Data Flow

### Inbound Data Flow

```rust
/// Messages received by Confidence Calculator
#[derive(Debug, Clone)]
pub enum ConfidenceCalculatorInbound {
    // From M14 Remediation Engine
    RemediationRequest {
        issue_type: IssueType,
        severity: Severity,
        service_id: String,
    },

    // From L1 Metrics
    HistoricalMetrics {
        action_type: String,
        success_count: u64,
        failure_count: u64,
        timestamp: DateTime<Utc>,
    },

    // From L5 Learning Pathways
    PathwayUpdate {
        pathway_id: String,
        strength: f64,
        activation_count: u64,
    },

    // From L3 Learning
    PatternDiscovery {
        pattern_id: String,
        issue_type: IssueType,
        effectiveness: f64,
    },
}
```

### Outbound Data Flow

```rust
/// Messages emitted by Confidence Calculator
#[derive(Debug, Clone)]
pub enum ConfidenceCalculatorOutbound {
    // To M14 Remediation Engine
    ConfidenceScore {
        request_id: String,
        confidence: f64,
        score_quality: ScoreQuality,
        recommended_tier: EscalationTier,
    },

    // To M16 Action Executor
    ExecutionPermission {
        action_id: String,
        confidence: f64,
        auto_executable: bool,
    },

    // To L5 Learning
    ConfidenceMetric {
        action_type: String,
        historical_rate: f64,
        pattern_match: f64,
        pathway_strength: f64,
    },

    // To subscribers
    ConfidenceEvent {
        score: ConfidenceScore,
        action: RemediationAction,
        timestamp: DateTime<Utc>,
    },
}
```

---

## Metrics

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `me_m15_confidence_score` | Gauge | action, issue_type | Current confidence score |
| `me_m15_calculations_total` | Counter | source, quality | Total calculations |
| `me_m15_factor_historical_avg` | Gauge | action | Average historical factor |
| `me_m15_factor_pattern_avg` | Gauge | action | Average pattern factor |
| `me_m15_factor_severity_avg` | Gauge | severity | Average severity factor |
| `me_m15_factor_pathway_avg` | Gauge | pathway_type | Average pathway factor |
| `me_m15_factor_temporal_avg` | Gauge | action | Average temporal factor |
| `me_m15_score_quality_high` | Gauge | action | High quality scores |
| `me_m15_score_distribution` | Histogram | action | Score distribution |
| `me_m15_cache_hits` | Counter | action | Cache hit count |
| `me_m15_cache_misses` | Counter | action | Cache miss count |

---

## Error Codes

| Code | Name | Severity | Description | Recovery |
|------|------|----------|-------------|----------|
| E15001 | InvalidFactorValue | Warning | Factor outside [0.0, 1.0] | Clamp to valid range |
| E15002 | HistoricalDataMissing | Warning | No historical data available | Use default factor |
| E15003 | PatternMatchFailed | Warning | Cannot match patterns | Use zero match strength |
| E15004 | PathwayNotFound | Warning | Pathway not in learning system | Use default weight |
| E15005 | CalculationError | Error | Arithmetic error in calculation | Return error to caller |
| E15006 | InvalidWeights | Critical | Weight configuration invalid | Check config |
| E15007 | CachingError | Warning | Cannot cache result | Continue without cache |
| E15008 | LowQualityScore | Warning | Score based on insufficient data | Mark quality as Low |
| E15009 | DataInconsistency | Error | Inconsistent input data | Investigate data source |
| E15010 | TimeoutExceeded | Error | Calculation took too long | Return timeout error |

---

## Related Modules

- **M14_REMEDIATION_ENGINE**: Uses confidence to drive escalation decisions
- **M16_ACTION_EXECUTOR**: Uses confidence to validate action execution
- **M17_OUTCOME_RECORDER**: Tracks confidence accuracy over time
- **M18_FEEDBACK_LOOP**: Learns from confidence prediction accuracy
- **L1_METRICS**: Provides historical performance data
- **L5_HEBBIAN_ENGINE**: Provides pathway strength weights
- **L5_LEARNING**: Provides pattern data and effectiveness metrics

---

## Navigation

| Direction | Link |
|-----------|------|
| Up | [INDEX.md](../INDEX.md) |
| Layer | [L03_CORE_LOGIC.md](../layers/L03_CORE_LOGIC.md) |
| Previous | [M14_REMEDIATION_ENGINE.md](M14_REMEDIATION_ENGINE.md) |
| Next | [M16_ACTION_EXECUTOR.md](M16_ACTION_EXECUTOR.md) |
| Related | [M17_OUTCOME_RECORDER.md](M17_OUTCOME_RECORDER.md) |
| API | [REST_API.md](../integration/REST_API.md) |

---

*[Back to Index](../INDEX.md) | [Layer: L03 Core Logic](../layers/L03_CORE_LOGIC.md)*
