# Fitness Function - Formal Specification

```json
{"v":"1.0.0","type":"MODULE_SPEC","module":"FITNESS","name":"Fitness Evaluator","layer":7,"dimensions":12,"estimated_loc":800,"estimated_tests":50}
```

**Version:** 1.0.0
**Layer:** L7 (Observer)
**Module:** FITNESS (supporting M39 Evolution Chamber)
**Related:** [SYSTEM_SPEC.md](../SYSTEM_SPEC.md), [TENSOR_SPEC.md](../TENSOR_SPEC.md), [STDP_SPEC.md](../STDP_SPEC.md), [SERVICE_SPEC.md](../SERVICE_SPEC.md)

---

## Navigation

| Direction | Target |
|-----------|--------|
| Up | [INDEX.md](INDEX.md) |
| Prev | [RALPH_LOOP_SPEC.md](RALPH_LOOP_SPEC.md) |
| Next | [EVENT_CHANNEL_SPEC.md](EVENT_CHANNEL_SPEC.md) |

---

## 1. Purpose

The Fitness Evaluator computes system fitness from the 12D tensor representation (`Tensor12D` from `lib.rs`). The fitness score drives the RALPH loop in M39 Evolution Chamber. It supports both single-service evaluation (one `Tensor12D`) and fleet-wide aggregation (a `Vec<Tensor12D>` weighted by service tier). The evaluator maintains a rolling history of fitness reports and performs linear regression trend analysis to classify the system trajectory as Improving, Stable, Declining, or Volatile.

### Objectives

| Objective | Description |
|-----------|-------------|
| Single-service fitness | Compute a weighted sum over all 12 normalized tensor dimensions |
| Fleet aggregation | Aggregate per-service fitness using tier-based weighting |
| Trend analysis | Classify fitness trajectory via linear regression over a sliding window |
| Weight adjustment | Allow M39 Evolution Chamber to tune dimension weights under safety constraints |
| History tracking | Maintain a bounded FIFO history of `FitnessReport` snapshots |

---

## 2. Complete Type Definitions

### 2.1 Core Structure

```rust
use std::collections::VecDeque;
use chrono::{DateTime, Utc};
use parking_lot::RwLock;

/// FITNESS: System fitness evaluator for the L7 Observer Layer.
///
/// Computes weighted fitness scores from 12D tensor representations
/// and tracks fitness trend over time via linear regression.
///
/// # Layer: L7 (Observer)
/// # Dependencies: Tensor12D (lib.rs), M39 (EvolutionChamber)
pub struct FitnessEvaluator {
    /// Dimension weights applied to normalized tensor scores.
    /// Protected by RwLock because M39 may adjust weights at runtime.
    /// Invariant: weights.iter().sum() == 1.0 (tolerance: +/-0.001).
    weights: RwLock<[f64; 12]>,

    /// Rolling FIFO history of fitness reports.
    /// Capacity bounded by config.history_capacity.
    history: RwLock<VecDeque<FitnessReport>>,

    /// Immutable configuration loaded at construction time.
    config: FitnessConfig,
}
```

### 2.2 Fitness Report

```rust
/// A snapshot of system fitness at a point in time.
#[derive(Clone, Debug)]
pub struct FitnessReport {
    /// Unique report identifier (UUID v4).
    pub id: String,

    /// Overall fitness score in [0.0, 1.0].
    /// Computed as the clamped weighted sum of dimension scores.
    pub overall_fitness: f64,

    /// Raw dimension values extracted (and optionally inverted) from
    /// the source Tensor12D. Length: 12.
    pub dimension_scores: [f64; 12],

    /// Weighted dimension scores: weight[i] * dimension_scores[i].
    /// Length: 12.
    pub weighted_scores: [f64; 12],

    /// Index of the dimension with the minimum weighted_score.
    /// Identifies the weakest contributor to overall fitness.
    pub weakest_dimension: usize,

    /// Index of the dimension with the maximum weighted_score.
    /// Identifies the strongest contributor to overall fitness.
    pub strongest_dimension: usize,

    /// Fitness trend classification based on linear regression
    /// over the most recent trend_window reports.
    pub trend: FitnessTrend,

    /// Timestamp of report generation.
    pub timestamp: DateTime<Utc>,
}
```

### 2.3 Fitness Trend

```rust
/// Classification of the fitness trajectory over a sliding window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FitnessTrend {
    /// Linear regression slope > stability_tolerance.
    Improving,

    /// |slope| <= stability_tolerance.
    Stable,

    /// slope < -stability_tolerance.
    Declining,

    /// Standard deviation > volatility_threshold (checked first).
    Volatile,
}
```

### 2.4 Configuration

```rust
/// Configuration for the FitnessEvaluator.
/// All fields have sensible defaults; override via config/observer.toml.
#[derive(Clone, Debug)]
pub struct FitnessConfig {
    /// Maximum number of FitnessReports retained in history (FitnessConfig).
    /// Not to be confused with EvolutionConfig.fitness_history_capacity (500),
    /// which sizes M39's internal FitnessSnapshot buffer.
    /// Default: 200.
    pub history_capacity: usize,

    /// Number of most-recent reports used for trend analysis.
    /// Default: 10.
    pub trend_window: usize,

    /// Absolute slope threshold below which the trend is Stable.
    /// Default: 0.02.
    pub stability_tolerance: f64,

    /// Standard deviation threshold above which the trend is Volatile.
    /// Default: 0.05.
    pub volatility_threshold: f64,
}

impl Default for FitnessConfig {
    fn default() -> Self {
        Self {
            history_capacity: 200,
            trend_window: 10,
            stability_tolerance: 0.02,
            volatility_threshold: 0.05,
        }
    }
}
```

---

## 3. Fitness Computation Formula

### 3.1 Single-Service Fitness

Given a `Tensor12D` `T` with dimensions `[d0, d1, ..., d11]`:

```
Step 1: Normalize scores
  score[i] = T.dimension[i]       for i in {0..9, 11}
  score[10] = 1.0 - T.error_rate  (INVERTED: lower error = higher fitness)

Step 2: Apply weights
  weighted[i] = weight[i] * score[i]

Step 3: Sum
  fitness = Sigma(weighted[i]) for i in 0..12

Step 4: Clamp
  fitness = fitness.clamp(0.0, 1.0)
```

**Rust implementation note:** Use FMA (`mul_add`) for float computation to match god-tier Rust patterns established in L3/L5 modules:

```rust
// Instead of: weight[i] * score[i] accumulated via +
// Use: weight[i].mul_add(score[i], running_sum)
let fitness = weights.iter()
    .zip(scores.iter())
    .fold(0.0_f64, |acc, (&w, &s)| w.mul_add(s, acc))
    .clamp(0.0, 1.0);
```

### 3.2 Dimension Score Extraction

| Index | Dimension | Score Formula | Notes |
|-------|-----------|--------------|-------|
| D0 | service_id | `T.service_id` | Direct |
| D1 | port | `T.port` | Direct |
| D2 | tier | `T.tier` | Direct |
| D3 | dependency_count | `T.dependency_count` | Direct |
| D4 | agent_count | `T.agent_count` | Direct |
| D5 | protocol | `T.protocol` | Direct |
| D6 | health_score | `T.health_score` | Direct |
| D7 | uptime | `T.uptime` | Direct |
| D8 | synergy | `T.synergy` | Direct |
| D9 | latency | `T.latency` | Direct |
| D10 | error_rate | `1.0 - T.error_rate` | **INVERTED** |
| D11 | temporal_context | `T.temporal_context` | Direct |

### 3.3 Fleet Fitness (Aggregate)

Given `Vec<Tensor12D>` fleet with per-service tier information:

```
Step 1: Compute per-service fitness
  service_fitness[j] = compute_fitness(fleet[j])

Step 2: Weighted average by tier
  fleet_fitness = Sigma(service_fitness[j] * tier_weight[j]) / Sigma(tier_weight[j])
```

**Tier weight mapping:**

| Tier | Weight | Services (from SERVICE_SPEC) |
|------|--------|------------------------------|
| 1 | 1.5 | SYNTHEX, SAN-K7 |
| 2 | 1.3 | NAIS, CodeSynthor V7, DevOps Engine |
| 3 | 1.2 | Tool Library, Library Agent, CCM |
| 4 | 1.1 | Prometheus Swarm, Architect Agent |
| 5 | 1.0 | Bash Engine, Tool Maker |

**Tier extraction from Tensor12D:**

```rust
/// Extract tier from D2 (tier dimension) and map to tier weight.
/// D2 is encoded as tier/6.0 in Tensor12D, so:
///   tier_number = (T.tier * 6.0).round() as u8
fn tier_weight(tensor: &Tensor12D) -> f64 {
    let tier = (tensor.tier * 6.0).round() as u8;
    match tier {
        1 => 1.5,
        2 => 1.3,
        3 => 1.2,
        4 => 1.1,
        _ => 1.0, // Default for tier 5 or unknown
    }
}
```

---

## 4. Default Dimension Weights

| Index | Dimension | Weight | Category | Rationale |
|-------|-----------|--------|----------|-----------|
| D0 | service_id | 0.02 | Identity | Identifier hash, minimal quality signal |
| D1 | port | 0.01 | Identity | Port number, no quality signal |
| D2 | tier | 0.03 | Context | Higher tier = more important |
| D3 | dependency_count | 0.05 | Context | Fewer deps = more resilient |
| D4 | agent_count | 0.04 | Context | More agents = better monitoring |
| D5 | protocol | 0.02 | Identity | Protocol encoding |
| D6 | health_score | 0.20 | **Primary** | Direct health measurement |
| D7 | uptime | 0.18 | **Primary** | Availability measurement |
| D8 | synergy | 0.15 | **Secondary** | Cross-service cooperation |
| D9 | latency | 0.12 | **Secondary** | Performance measurement |
| D10 | error_rate | 0.10 | **Secondary** | Reliability (inverted) |
| D11 | temporal_context | 0.08 | Context | Time relevance |
| | **Total** | **1.00** | | Must always sum to 1.0 |

### Weight Category Summary

| Category | Dimensions | Total Weight | Purpose |
|----------|-----------|-------------|---------|
| **Primary** | D6, D7 | 0.38 (38%) | Core health indicators |
| **Secondary** | D8, D9, D10 | 0.37 (37%) | Operational quality |
| **Context** | D2, D3, D4, D11 | 0.20 (20%) | Structural context |
| **Identity** | D0, D1, D5 | 0.05 (5%) | Identification only |

### Default Weights Constant

```rust
/// Default dimension weights. Must sum to 1.0.
pub const DEFAULT_WEIGHTS: [f64; 12] = [
    0.02, // D0  service_id       (Identity)
    0.01, // D1  port             (Identity)
    0.03, // D2  tier             (Context)
    0.05, // D3  dependency_count (Context)
    0.04, // D4  agent_count      (Context)
    0.02, // D5  protocol         (Identity)
    0.20, // D6  health_score     (Primary)
    0.18, // D7  uptime           (Primary)
    0.15, // D8  synergy          (Secondary)
    0.12, // D9  latency          (Secondary)
    0.10, // D10 error_rate       (Secondary)
    0.08, // D11 temporal_context (Context)
];
```

---

## 5. Weight Adjustment Rules

Weights can be adjusted by M39 `EvolutionChamber` via `adjust_weights()`:

### Preconditions

| Constraint | Rule | Tolerance |
|------------|------|-----------|
| Sum to 1.0 | `new_weights.iter().sum::<f64>()` must equal 1.0 | +/-0.001 |
| No domination | Each weight must be in [0.0, 0.5] | Exact |
| Primary floor | D6 (health_score) weight >= 0.10 | Exact |
| Primary floor | D7 (uptime) weight >= 0.10 | Exact |
| Identity ceiling | D0 (service_id) weight <= 0.05 | Exact |
| Identity ceiling | D1 (port) weight <= 0.05 | Exact |
| Identity ceiling | D5 (protocol) weight <= 0.05 | Exact |

### Postconditions

| Condition | Behavior |
|-----------|----------|
| Old weights | NOT stored in history (only FitnessReports are) |
| Logging | Weight changes are logged to evolution channel |
| Immediate effect | New weights apply to the next `evaluate()` call |

### Validation Code

```rust
/// Validate proposed weights against all adjustment constraints.
///
/// # Errors
/// - `Error::Validation("weights must sum to 1.0")` if sum outside tolerance
/// - `Error::Validation("weight exceeds 0.5 cap")` if any weight > 0.5
/// - `Error::Validation("primary weight below minimum")` if D6 or D7 < 0.10
/// - `Error::Validation("identity weight above maximum")` if D0, D1, or D5 > 0.05
fn validate_weights(weights: &[f64; 12]) -> Result<()> {
    let sum: f64 = weights.iter().sum();
    if (sum - 1.0).abs() > 0.001 {
        return Err(Error::Validation("weights must sum to 1.0".into()));
    }
    for (i, &w) in weights.iter().enumerate() {
        if w < 0.0 || w > 0.5 {
            return Err(Error::Validation(
                format!("weight[{i}] = {w:.4} outside [0.0, 0.5]")
            ));
        }
    }
    // Primary floor: D6 >= 0.10, D7 >= 0.10
    if weights[6] < 0.10 {
        return Err(Error::Validation("D6 health_score weight below 0.10".into()));
    }
    if weights[7] < 0.10 {
        return Err(Error::Validation("D7 uptime weight below 0.10".into()));
    }
    // Identity ceiling: D0, D1, D5 <= 0.05
    for &idx in &[0_usize, 1, 5] {
        if weights[idx] > 0.05 {
            return Err(Error::Validation(
                format!("identity weight[{idx}] = {:.4} exceeds 0.05", weights[idx])
            ));
        }
    }
    Ok(())
}
```

---

## 6. Trend Analysis Algorithm

### 6.1 Linear Regression over Trend Window

Given the last `N` fitness values `[f1, f2, ..., fN]` where `N = trend_window`:

```
x = [1, 2, ..., N]
y = [f1, f2, ..., fN]

x_sum   = N * (N + 1) / 2
x_sq_sum = N * (N + 1) * (2N + 1) / 6
y_sum   = Sigma(yi)
xy_sum  = Sigma(xi * yi)

slope = (N * xy_sum - x_sum * y_sum) / (N * x_sq_sum - x_sum * x_sum)

mean  = y_sum / N
stddev = sqrt(Sigma((yi - mean)^2) / N)
```

### 6.2 Trend Classification

```
IF stddev > volatility_threshold:
  trend = Volatile
ELIF slope > stability_tolerance:
  trend = Improving
ELIF slope < -stability_tolerance:
  trend = Declining
ELSE:
  trend = Stable
```

**Note:** Volatility check is performed FIRST. A volatile system with a positive slope is still classified as Volatile because the high variance makes the trend unreliable.

### 6.3 Rust Implementation Sketch

```rust
fn compute_trend(&self, history: &VecDeque<FitnessReport>) -> FitnessTrend {
    let n = history.len().min(self.config.trend_window);
    if n < 2 {
        return FitnessTrend::Stable; // Insufficient data
    }

    let values: Vec<f64> = history.iter()
        .rev()
        .take(n)
        .map(|r| r.overall_fitness)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();

    let n_f64 = n as f64;

    // Mean and standard deviation
    let mean = values.iter().sum::<f64>() / n_f64;
    let variance = values.iter()
        .map(|&y| (y - mean).powi(2))
        .sum::<f64>() / n_f64;
    let stddev = variance.sqrt();

    // Volatility check first
    if stddev > self.config.volatility_threshold {
        return FitnessTrend::Volatile;
    }

    // Linear regression slope (using FMA)
    let mut xy_sum = 0.0_f64;
    let mut y_sum = 0.0_f64;
    for (i, &y) in values.iter().enumerate() {
        let x = (i + 1) as f64;
        xy_sum = x.mul_add(y, xy_sum);
        y_sum += y;
    }

    let x_sum = n_f64.mul_add(n_f64 + 1.0, 0.0) / 2.0;
    let x_sq_sum = n_f64.mul_add(
        (n_f64 + 1.0).mul_add(2.0_f64.mul_add(n_f64, 1.0), 0.0),
        0.0,
    ) / 6.0;

    let denominator = n_f64.mul_add(x_sq_sum, -(x_sum * x_sum));
    if denominator.abs() < f64::EPSILON {
        return FitnessTrend::Stable;
    }

    let slope = n_f64.mul_add(xy_sum, -(x_sum * y_sum)) / denominator;

    if slope > self.config.stability_tolerance {
        FitnessTrend::Improving
    } else if slope < -self.config.stability_tolerance {
        FitnessTrend::Declining
    } else {
        FitnessTrend::Stable
    }
}
```

### 6.4 Trend Decision Table

| Condition | stddev | slope | Result |
|-----------|--------|-------|--------|
| High variance | > 0.05 | any | **Volatile** |
| Positive slope | <= 0.05 | > 0.02 | **Improving** |
| Negative slope | <= 0.05 | < -0.02 | **Declining** |
| Flat | <= 0.05 | [-0.02, 0.02] | **Stable** |
| Insufficient data | -- | -- | **Stable** (default) |

---

## 7. API Contract

### 7.1 Constructor

```rust
/// Creates a new FitnessEvaluator with the given configuration.
///
/// # Preconditions
/// - config.history_capacity > 0
/// - config.trend_window >= 2
/// - config.stability_tolerance > 0.0
/// - config.volatility_threshold > 0.0
/// - config.volatility_threshold > config.stability_tolerance
///
/// # Postconditions
/// - weights initialized to DEFAULT_WEIGHTS
/// - history is empty
///
/// # Errors
/// - `Error::Validation` if any precondition is violated
pub fn new(config: FitnessConfig) -> Result<Self>;
```

### 7.2 Evaluation Methods

```rust
/// Evaluate a single service tensor and produce a FitnessReport.
///
/// # Preconditions
/// - tensor passes Tensor12D::validate() (all dimensions in [0.0, 1.0])
///
/// # Postconditions
/// - FitnessReport pushed to history (FIFO eviction if full)
/// - overall_fitness in [0.0, 1.0]
/// - dimension_scores[10] = 1.0 - tensor.error_rate (inverted)
/// - trend computed from updated history
///
/// # Errors
/// - `Error::TensorValidation` if tensor has invalid dimensions
pub fn evaluate(&self, tensor: &Tensor12D) -> Result<FitnessReport>;

/// Evaluate a fleet of service tensors and produce an aggregate FitnessReport.
///
/// # Preconditions
/// - tensors is non-empty
/// - all tensors pass Tensor12D::validate()
///
/// # Postconditions
/// - Fleet fitness = tier-weighted average of per-service fitness
/// - Single aggregate FitnessReport pushed to history
/// - weakest_dimension = globally weakest across all services
/// - strongest_dimension = globally strongest across all services
///
/// # Errors
/// - `Error::Validation("empty fleet")` if tensors is empty
/// - `Error::TensorValidation` if any tensor is invalid
pub fn evaluate_fleet(&self, tensors: &[Tensor12D]) -> Result<FitnessReport>;
```

### 7.3 Accessors

```rust
/// Returns the current fitness trend based on history.
///
/// # Postconditions
/// - Returns Stable if history has fewer than 2 entries
pub fn get_trend(&self) -> FitnessTrend;

/// Returns the most recent N fitness reports (newest first).
///
/// # Postconditions
/// - Returns min(limit, history.len()) reports
/// - Reports ordered by timestamp descending
pub fn get_history(&self, limit: usize) -> Vec<FitnessReport>;

/// Returns the current dimension weights.
///
/// # Postconditions
/// - Returns a copy of the active weights array
pub fn current_weights(&self) -> [f64; 12];
```

### 7.4 Weight Adjustment

```rust
/// Adjust dimension weights (called by M39 EvolutionChamber).
///
/// # Preconditions
/// - new_weights must sum to 1.0 (tolerance: +/-0.001)
/// - Each weight must be in [0.0, 0.5]
/// - D6 (health_score) and D7 (uptime) must each be >= 0.10
/// - D0 (service_id), D1 (port), D5 (protocol) must each be <= 0.05
///
/// # Postconditions
/// - Active weights replaced with new_weights
/// - Old weights are NOT stored in history
/// - Weight change logged to evolution channel
///
/// # Errors
/// - `Error::Validation` if any constraint is violated
pub fn adjust_weights(&self, weights: [f64; 12]) -> Result<()>;
```

### 7.5 API Summary Table

| Method | Input | Output | Errors |
|--------|-------|--------|--------|
| `new(config)` | `FitnessConfig` | `Result<Self>` | Validation if config invalid |
| `evaluate(tensor)` | `&Tensor12D` | `Result<FitnessReport>` | Validation if tensor invalid |
| `evaluate_fleet(tensors)` | `&[Tensor12D]` | `Result<FitnessReport>` | Validation if empty or invalid |
| `get_trend()` | -- | `FitnessTrend` | -- |
| `get_history(limit)` | `usize` | `Vec<FitnessReport>` | -- |
| `adjust_weights(weights)` | `[f64; 12]` | `Result<()>` | Validation if sum != 1.0 |
| `current_weights()` | -- | `[f64; 12]` | -- |

---

## 8. Performance Characteristics

| Operation | Time Complexity | Space Complexity | Expected Latency |
|-----------|----------------|------------------|------------------|
| `evaluate` | O(12) dimensions + O(trend_window) regression | O(1) | <1ms |
| `evaluate_fleet` | O(n * 12) where n = fleet size | O(n) | <5ms |
| `get_trend` | O(trend_window) | O(trend_window) | <1ms |
| `get_history` | O(limit) | O(limit) | <1ms |
| `adjust_weights` | O(12) validation | O(1) | <1ms |
| `current_weights` | O(1) RwLock read | O(1) | <1ms |

### Memory Footprint

| Component | Estimate |
|-----------|----------|
| Weights array (12 x f64) | 96 bytes |
| History (200 reports, ~300 bytes each) | ~60 KB |
| Config | ~40 bytes |
| RwLock overhead | ~128 bytes |
| **Total** | **~61 KB** |

---

## 9. Configuration (TOML)

```toml
[observer.fitness]
history_capacity = 200
trend_window = 10
stability_tolerance = 0.02
volatility_threshold = 0.05
```

---

## 10. Error Conditions

| Error | Cause | Recovery |
|-------|-------|----------|
| `Error::TensorValidation` | Tensor dimension outside [0.0, 1.0] or NaN/Inf | Caller should `clamp_normalize()` before evaluation |
| `Error::Validation("weights must sum to 1.0")` | Weight array sum outside 1.0 +/-0.001 | Re-normalize weights before retry |
| `Error::Validation("weight exceeds 0.5 cap")` | Single weight > 0.5 | Redistribute excess to other dimensions |
| `Error::Validation("primary weight below minimum")` | D6 or D7 < 0.10 | Increase primary weight to minimum |
| `Error::Validation("identity weight above maximum")` | D0, D1, or D5 > 0.05 | Decrease identity weight to ceiling |
| `Error::Validation("empty fleet")` | Empty slice passed to `evaluate_fleet` | Ensure at least one tensor |
| `Error::Validation("invalid config")` | Config parameter out of range | Use `FitnessConfig::default()` |

---

## 11. Testing Matrix

| Category | Count | Description |
|----------|-------|-------------|
| Weight validation | 8 | Sum to 1.0, bounds check, primary floor enforcement, identity ceiling enforcement, single-weight cap at 0.5, negative weight rejection, zero-weight allowed, tolerance boundary (+/-0.001) |
| Single fitness | 8 | Each dimension contributes correctly, weighted sum accuracy, all-zero tensor (fitness = 0.0), all-one tensor (max fitness), mid-range values, FMA precision matches naive sum |
| D10 inversion | 3 | error_rate=0.0 yields score=1.0, error_rate=1.0 yields score=0.0, error_rate=0.5 yields score=0.5 |
| Fleet aggregation | 6 | Tier weighting correctness (T1 > T5 influence), single-service fleet equals single-service, mixed tiers, all same tier, weighted average vs simple average difference, empty fleet rejection |
| Trend detection | 10 | Monotonically increasing = Improving, monotonically decreasing = Declining, constant values = Stable, random noise = Volatile, 2-element history (minimum), trend_window boundary, slope at exact tolerance boundary, stddev at exact threshold, mixed improving-then-declining, insufficient history (< 2) = Stable |
| Edge cases | 5 | All tensor dimensions = 0.0, all tensor dimensions = 1.0, single-service fleet, NaN dimension rejection, history at exact capacity (200th insert triggers no eviction, 201st triggers eviction) |
| Weight adjustment | 5 | Valid adjustment accepted, sum != 1.0 rejected, primary floor violated rejected, identity ceiling violated rejected, cap at 0.5 violated rejected |
| History management | 5 | FIFO ordering, capacity enforcement (oldest evicted), empty history returns empty vec, limit > history.len() returns all, get_history(0) returns empty |
| **Total** | **50** | |

### Test Invariants

| Invariant | Assertion |
|-----------|-----------|
| Fitness bounded | All `overall_fitness` values in [0.0, 1.0] |
| Weights sum | `weights.iter().sum()` within 0.001 of 1.0 at all times |
| History bounded | `history.len() <= config.history_capacity` at all times |
| D10 inverted | `dimension_scores[10] == 1.0 - tensor.error_rate` |
| Trend window | Trend analysis uses at most `config.trend_window` recent reports |
| No panic | All methods return `Result` or infallible types (no `.unwrap()`, no `.expect()`) |

---

## 12. Cross-Spec Dependencies

| Dependency | Source Spec | Integration Point |
|------------|-------------|-------------------|
| Tensor12D | [lib.rs](../../src/lib.rs) | Input to `evaluate()` and `evaluate_fleet()` |
| Error taxonomy | M01 Error (L1) | Error::Validation, Error::TensorValidation variants |
| Service tiers | [SERVICE_SPEC.md](../SERVICE_SPEC.md) | Tier-to-weight mapping for fleet aggregation |
| Evolution Channel | [EVENT_CHANNEL_SPEC.md](EVENT_CHANNEL_SPEC.md) | Weight adjustment events published to `evolution` channel |
| RALPH Loop (M39) | [RALPH_LOOP_SPEC.md](RALPH_LOOP_SPEC.md) | Consumer of FitnessReport, caller of adjust_weights() |
| STDP parameters | [STDP_SPEC.md](../STDP_SPEC.md) | Learning rates referenced by dimension weight tuning |
| EventBus (M23) | [PIPELINE_SPEC.md](../PIPELINE_SPEC.md) | Fitness reports published to `evolution` channel |

---

## 13. Version History

| Version | Date | Changes |
|---------|------|---------|
| 1.0.0 | 2026-01-29 | Initial specification |

---

*Generated: 2026-01-29 | The Maintenance Engine v1.0.0 | L7 Observer Layer*
