# M39 Evolution Chamber - Formal Specification

```json
{"v":"1.0.0","type":"MODULE_SPEC","module":"M39","name":"Evolution Chamber","layer":7,"estimated_loc":1800,"estimated_tests":50}
```

| Property | Value |
|----------|-------|
| **Module** | M39 |
| **Name** | Evolution Chamber |
| **Layer** | L7 Observer |
| **Version** | 1.0.0 |
| **Estimated LOC** | 1,800 |
| **Estimated Tests** | 50 |
| **Status** | SPECIFIED |

---

## Navigation

| Direction | Target |
|-----------|--------|
| Up | [INDEX.md](INDEX.md) |
| Prev | [EMERGENCE_DETECTOR_SPEC.md](EMERGENCE_DETECTOR_SPEC.md) |
| Next | [RALPH_LOOP_SPEC.md](RALPH_LOOP_SPEC.md) |

| Related Spec | Relationship |
|--------------|-------------|
| [RALPH_LOOP_SPEC.md](RALPH_LOOP_SPEC.md) | Protocol executed by this module |
| [../../PBFT_SPEC.md](../../PBFT_SPEC.md) | Consensus for high-impact mutations |
| [../../STDP_SPEC.md](../../STDP_SPEC.md) | Learning pathway feedback target |
| [../../TENSOR_SPEC.md](../../TENSOR_SPEC.md) | 12D tensor consumed by FitnessEvaluator |
| [../../ESCALATION_SPEC.md](../../ESCALATION_SPEC.md) | Escalation tier shift mutations |

---

## 1. Purpose

The Evolution Chamber is the meta-learning engine of Layer 7 (Observer). It evolves system parameters through the RALPH (Recognize, Analyze, Learn, Plan, Harmonize) loop. The module consumes `EmergenceRecord` instances from M38 (Emergence Detector) and `FitnessReport` instances from the `FitnessEvaluator`, generates `Mutation` proposals, and manages the full mutation lifecycle:

```
generate -> consensus -> apply -> verify -> commit/rollback
```

The Evolution Chamber is the primary actuator for L7: while M37 (Correlation Engine) observes events, M38 (Emergence Detector) identifies emergent behaviors, and the `FitnessEvaluator` scores system health, M39 is the component that closes the loop by translating observations into parameter changes that drive the system toward higher fitness.

---

## 2. Complete Type Definitions

### 2.1 Primary Struct

```rust
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::AtomicU64;
use tokio::sync::RwLock;
use chrono::{DateTime, Utc};

/// Meta-learning engine that evolves system parameters through the RALPH loop.
///
/// The Evolution Chamber maintains a history of all mutations and their outcomes,
/// tracks active (in-verification) mutations, and uses fitness snapshots to
/// determine system state and mutation urgency.
///
/// # Thread Safety
/// All mutable state is behind `RwLock` or `AtomicU64`, making the struct
/// safe to share across async tasks via `Arc<EvolutionChamber>`.
///
/// # Invariants
/// - `generation` is monotonically increasing
/// - `active_mutations.len() <= config.max_concurrent_mutations`
/// - All parameter deltas are bounded by `config.max_mutation_delta`
pub struct EvolutionChamber {
    /// Complete history of all mutations (bounded by config.mutation_history_capacity)
    mutation_history: RwLock<VecDeque<MutationRecord>>,

    /// Currently active (applied but not yet committed/rolled-back) mutations
    active_mutations: RwLock<HashMap<String, ActiveMutation>>,

    /// Rolling window of fitness snapshots (bounded by config.fitness_history_capacity)
    fitness_history: RwLock<VecDeque<FitnessSnapshot>>,

    /// Monotonically increasing generation counter
    generation: AtomicU64,

    /// Immutable configuration (set at construction)
    config: EvolutionConfig,
}
```

### 2.2 Mutation Types

```rust
/// Enumeration of all possible system parameter mutations.
///
/// Each variant targets a specific subsystem and carries the data
/// necessary to apply and revert the change.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MutationType {
    /// Adjust Hebbian pathway weight between two services.
    /// Applied via M25 HebbianManager.
    PathwayAdjustment {
        /// Source-target key in "source->target" format
        pathway_key: String,
        /// Weight delta, clamped to [-max_mutation_delta, +max_mutation_delta]
        delta: f64,
    },

    /// Adjust a monitoring or operational threshold.
    /// Applied via M02 ConfigurationManager.
    ThresholdAdjustment {
        /// Metric identifier (e.g. "health_check_interval_ms")
        metric: String,
        /// Previous value (for rollback)
        old: f64,
        /// New value
        new: f64,
    },

    /// Adjust STDP learning parameters.
    /// Applied via M26 StdpProcessor.
    LearningRateAdjustment {
        /// Parameter name (e.g. "ltp_rate", "ltd_rate", "decay_rate")
        parameter: String,
        /// Previous value (for rollback)
        old: f64,
        /// New value
        new: f64,
    },

    /// Tune circuit breaker failure threshold for a service.
    /// Applied via M12 CircuitBreaker.
    CircuitBreakerTuning {
        /// Target service identifier
        service_id: String,
        /// New failure count before the breaker trips
        new_threshold: u32,
    },

    /// Adjust load balancer weight for a service.
    /// Applied via M11 LoadBalancer.
    LoadBalancerReweight {
        /// Target service identifier
        service_id: String,
        /// New weight, clamped to [0.1, 3.0]
        new_weight: f64,
    },

    /// Shift escalation tier boundaries.
    /// Applied via M15 ConfidenceCalculator / Escalation subsystem.
    EscalationTierShift {
        /// Source tier (e.g. "L1")
        from: String,
        /// Destination tier (e.g. "L0")
        to: String,
    },

    /// Adjust homeostatic target setpoints.
    /// Applied via HomeostaticController (STDP_SPEC Section 7).
    HomeostaticTargetAdjustment {
        /// Target name (e.g. "target_health", "target_synergy")
        target: String,
        /// Previous value (for rollback)
        old: f64,
        /// New value
        new: f64,
    },
}
```

### 2.3 Mutation Record

```rust
/// Complete record of a mutation from generation through final verdict.
///
/// Tracks the full lifecycle: generation -> consensus -> apply -> verify -> commit/rollback.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MutationRecord {
    /// Unique identifier (UUID v4)
    pub id: String,

    /// Generation in which this mutation was created
    pub generation: u64,

    /// The mutation specification
    pub mutation: MutationType,

    /// Predicted fitness change (from effectiveness model)
    pub expected_delta: f64,

    /// Measured fitness change after verification window (None if not yet verified)
    pub actual_delta: Option<f64>,

    /// Whether the mutation has been applied to the system
    pub applied: bool,

    /// Whether the mutation was rolled back after verification
    pub rolled_back: bool,

    /// Whether PBFT consensus is required (true if expected_delta >= auto_apply_threshold)
    pub consensus_required: bool,

    /// Result of PBFT vote (None if auto-applied or not yet voted)
    pub consensus_result: Option<bool>,

    /// What triggered this mutation
    pub trigger: MutationTrigger,

    /// Creation timestamp
    pub created_at: DateTime<Utc>,

    /// Timestamp when mutation was applied (None if not yet applied)
    pub applied_at: Option<DateTime<Utc>>,

    /// Timestamp when mutation was verified (None if not yet verified)
    pub verified_at: Option<DateTime<Utc>>,
}
```

### 2.4 Mutation Trigger

```rust
/// Identifies what caused the Evolution Chamber to generate a mutation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MutationTrigger {
    /// Response to a detected emergent behavior (carries emergence_id)
    EmergenceResponse(String),

    /// Response to a fitness dimension drifting below baseline (carries dimension name)
    FitnessDrift(String),

    /// Scheduled periodic optimization cycle
    PeriodicOptimization,

    /// Self-observed pattern from meta-learning (NAM R1 SelfQuery)
    MetaLearning,
}
```

### 2.5 Active Mutation

```rust
/// Tracks a mutation that has been applied but not yet committed or rolled back.
///
/// Active mutations are monitored until their verification deadline expires,
/// at which point a verdict (Commit, Rollback, or Extend) is rendered.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveMutation {
    /// References MutationRecord.id
    pub mutation_id: String,

    /// The mutation specification (for rollback)
    pub mutation: MutationType,

    /// System fitness at the moment of application
    pub fitness_before: f64,

    /// Timestamp when the mutation was applied
    pub applied_at: DateTime<Utc>,

    /// Deadline for verification (applied_at + mutation_verification_ms)
    pub verification_deadline: DateTime<Utc>,

    /// Number of times the verification window has been extended
    pub extensions_used: u32,
}
```

### 2.6 Fitness Snapshot

```rust
/// Point-in-time capture of system fitness at a specific generation.
///
/// Used to compute baselines, trends, and mutation effectiveness.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FitnessSnapshot {
    /// Generation number at the time of capture
    pub generation: u64,

    /// Aggregate fitness score [0.0, 1.0]
    pub overall_fitness: f64,

    /// Per-dimension scores matching the 12D tensor encoding
    /// [D0:service_id, D1:port, D2:tier, D3:deps, D4:agents, D5:protocol,
    ///  D6:health, D7:uptime, D8:synergy, D9:latency, D10:error_rate, D11:temporal]
    pub dimension_scores: [f64; 12],

    /// Timestamp of capture
    pub timestamp: DateTime<Utc>,
}
```

### 2.7 System State

```rust
/// Classification of current system health based on overall fitness score.
///
/// Used to determine mutation urgency and generation timing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SystemState {
    /// fitness >= 0.90 -- System is performing optimally
    Thriving,

    /// fitness >= 0.75 && < 0.90 -- System is healthy but not optimal
    Stable,

    /// fitness >= 0.50 && < 0.75 -- System is underperforming
    Degraded,

    /// fitness < 0.50 -- System requires immediate intervention
    Critical,
}

impl SystemState {
    /// Classify a fitness score into a SystemState.
    pub fn from_fitness(fitness: f64) -> Self {
        if fitness >= 0.90 {
            Self::Thriving
        } else if fitness >= 0.75 {
            Self::Stable
        } else if fitness >= 0.50 {
            Self::Degraded
        } else {
            Self::Critical
        }
    }
}
```

### 2.8 Mutation Verdict

```rust
/// Result of the verification phase for an active mutation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MutationVerdict {
    /// actual_delta >= rollback_threshold: mutation is beneficial, keep it
    Commit,

    /// actual_delta < rollback_threshold: mutation is harmful, revert it
    Rollback,

    /// Inconclusive result, extend the verification window
    Extend,
}
```

### 2.9 Evolution Configuration

```rust
/// Configuration parameters for the Evolution Chamber.
///
/// All values have safe defaults. Overrides are loaded from
/// `config/evolution.toml` via M02 ConfigurationManager.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvolutionConfig {
    /// Maximum number of mutations that can be active (applied but unverified)
    /// at any given time. Prevents mutation interference.
    /// Default: 3
    pub max_concurrent_mutations: usize,

    /// Duration (milliseconds) to wait after applying a mutation before
    /// measuring its effect. The verification window.
    /// Default: 30000 (30 seconds)
    pub mutation_verification_ms: u64,

    /// Maximum number of FitnessSnapshot records to retain in the
    /// rolling history buffer (EvolutionConfig).
    /// Not to be confused with FitnessConfig.history_capacity (200),
    /// which sizes the FitnessEvaluator's FitnessReport buffer.
    /// Default: 500
    pub fitness_history_capacity: usize,

    /// Maximum number of MutationRecord entries to retain in
    /// the rolling history buffer.
    /// Default: 1000
    pub mutation_history_capacity: usize,

    /// Mutations with expected_delta >= this threshold require
    /// PBFT consensus (L3 escalation). Below this threshold,
    /// mutations are auto-applied (L0).
    /// Default: 0.10
    pub auto_apply_threshold: f64,

    /// If actual_delta after verification falls below this value,
    /// the mutation is automatically rolled back.
    /// Default: -0.02
    pub rollback_threshold: f64,

    /// Minimum interval (milliseconds) between successive RALPH
    /// generation cycles. Prevents rapid mutation cycling.
    /// Default: 60000 (1 minute)
    pub min_generation_interval_ms: u64,

    /// Maximum absolute change a single mutation can make to any
    /// parameter. Hard safety bound.
    /// Default: 0.20
    pub max_mutation_delta: f64,

    /// Maximum number of times a verification window can be extended
    /// for an inconclusive mutation before forcing a verdict.
    /// Default: 2
    pub max_verification_extensions: u32,
}

impl Default for EvolutionConfig {
    fn default() -> Self {
        Self {
            max_concurrent_mutations: 3,
            mutation_verification_ms: 30_000,
            fitness_history_capacity: 500,
            mutation_history_capacity: 1_000,
            auto_apply_threshold: 0.10,
            rollback_threshold: -0.02,
            min_generation_interval_ms: 60_000,
            max_mutation_delta: 0.20,
            max_verification_extensions: 2,
        }
    }
}
```

---

## 3. Mutation Lifecycle State Machine

The following state machine governs every mutation from generation through final disposition.

```
                   +------------+
                   | Generated  |
                   +------+-----+
                          |
               +----------+----------+
               |                     |
               v                     v
     +------------------+    +------------------+
     |  Auto-Apply      |    |  PBFT Queued     |
     | (delta < 0.10)   |    | (delta >= 0.10)  |
     +--------+---------+    +--------+---------+
              |                       |
              |              +--------+--------+
              |              |                 |
              |              v                 v
              |      +-----------+     +----------+
              |      | Approved  |     | Rejected |
              |      | (27/40)   |     | (<27/40) |
              |      +-----+-----+     +-----+----+
              |            |                 |
              v            v                 v
      +-----------+                  +----------+
      |  Applied  |                  | Discarded|
      +-----+-----+                  +----------+
            |
      +-----+-----+
      | Verifying  |  (wait mutation_verification_ms)
      +-----+------+
            |
     +------+------+----------+
     |             |           |
     v             v           v
+---------+  +----------+  +---------+
| Commit  |  | Rollback |  | Extend  |
| (D>=0)  |  | (D<-0.02)|  | (maybe) |
+---------+  +----------+  +---------+
```

### State Transition Table

| From | Event | To | Condition | Action |
|------|-------|----|-----------|--------|
| Generated | classify | Auto-Apply | `expected_delta < auto_apply_threshold` | Apply immediately |
| Generated | classify | PBFT Queued | `expected_delta >= auto_apply_threshold` | Submit to M31 PBFT Manager |
| PBFT Queued | vote_complete | Approved | Weighted quorum >= 27/40 | Proceed to apply |
| PBFT Queued | vote_complete | Rejected | Weighted quorum < 27/40 | Discard, record rejection |
| Auto-Apply | apply | Applied | `active_mutations.len() < max_concurrent_mutations` | Record `fitness_before`, set deadline |
| Approved | apply | Applied | `active_mutations.len() < max_concurrent_mutations` | Record `fitness_before`, set deadline |
| Applied | deadline_reached | Verifying | `now >= verification_deadline` | Measure `fitness_after` |
| Verifying | measure | Commit | `actual_delta >= rollback_threshold` | Commit, update history |
| Verifying | measure | Rollback | `actual_delta < rollback_threshold` | Revert parameter, update history |
| Verifying | measure | Extend | Inconclusive AND `extensions_used < max_verification_extensions` | Extend deadline |
| Extend | deadline_reached | Verifying | `now >= new_deadline` | Re-measure |
| Rejected | - | Discarded | - | Record in mutation_history |

---

## 4. Mutation Generation Algorithms

### 4.1 From Emergence Response

When M38 (Emergence Detector) publishes an `EmergenceRecord`, the Evolution Chamber maps the emergent behavior to a mutation type.

| EmergentBehavior | Generated MutationType | Rationale |
|------------------|----------------------|-----------|
| `CascadingFailure` | `CircuitBreakerTuning` | Lower failure threshold for origin service to trip earlier |
| `SynergyAmplification` | `LoadBalancerReweight` | Increase weight for synergistic services to amplify the effect |
| `SelfOrganizingRecovery` | `PathwayAdjustment` | Strengthen the recovery pathway via Hebbian reinforcement |
| `ResonancePattern` | `ThresholdAdjustment` | Tune monitoring frequency to match the natural resonance |
| `LoadShedding` | `LoadBalancerReweight` | Formalize the observed shed pattern as a permanent weight |
| `PathwayConvergence` | `PathwayAdjustment` | Reinforce the convergence target pathway |
| `AdaptiveThreshold` | `HomeostaticTargetAdjustment` | Update the homeostatic setpoint to match the adaptive target |

```
ALGORITHM: generate_from_emergence(record: &EmergenceRecord) -> Vec<MutationRecord>

  MATCH record.behavior:
    CascadingFailure:
      service_id = record.participating_services[0]  // origin
      current_threshold = circuit_breaker.get_threshold(service_id)
      new_threshold = max(1, current_threshold - 1)
      RETURN [CircuitBreakerTuning { service_id, new_threshold }]

    SynergyAmplification:
      FOR each service_id IN record.participating_services:
        current_weight = load_balancer.get_weight(service_id)
        new_weight = min(3.0, current_weight * 1.1)
        YIELD LoadBalancerReweight { service_id, new_weight }

    SelfOrganizingRecovery:
      pathway_key = format!("{}->{}",
        record.participating_services[0],
        record.participating_services[1])
      delta = min(max_mutation_delta, record.confidence * 0.15)
      RETURN [PathwayAdjustment { pathway_key, delta }]

    ResonancePattern:
      metric = "health_check_interval_ms"
      old = config.get(metric)
      resonance_period_ms = record.metadata.period_ms
      new = resonance_period_ms / 2  // Nyquist: sample at 2x frequency
      RETURN [ThresholdAdjustment { metric, old, new }]

    LoadShedding:
      FOR each service_id IN record.shed_targets:
        current_weight = load_balancer.get_weight(service_id)
        new_weight = max(0.1, current_weight * 0.85)
        YIELD LoadBalancerReweight { service_id, new_weight }

    PathwayConvergence:
      target_key = record.convergence_target
      delta = min(max_mutation_delta, 0.10)
      RETURN [PathwayAdjustment { pathway_key: target_key, delta }]

    AdaptiveThreshold:
      target = record.adapted_metric
      old = homeostatic.get_target(target)
      new = record.adapted_value
      RETURN [HomeostaticTargetAdjustment { target, old, new }]
```

### 4.2 From Fitness Drift

When the `FitnessEvaluator` reports a dimension scoring below baseline, the Evolution Chamber generates a corrective mutation.

| Weakest Dimension | Generated MutationType | Target |
|-------------------|----------------------|--------|
| D6 (health_score) | `ThresholdAdjustment` | Health check frequency |
| D7 (uptime) | `CircuitBreakerTuning` | Least-available service |
| D8 (synergy) | `PathwayAdjustment` | Cross-service pathways |
| D9 (latency) | `LoadBalancerReweight` | Redistribute load |
| D10 (error_rate) | `EscalationTierShift` | Tighten escalation for error-prone services |
| D11 (temporal) | `LearningRateAdjustment` | Increase adaptation speed |

```
ALGORITHM: generate_from_fitness_drift(report: &FitnessReport) -> Vec<MutationRecord>

  weakest = report.weakest_dimension()  // Index and score

  MATCH weakest.index:
    6 (health_score):
      metric = "health_check_interval_ms"
      old = config.get(metric)
      new = max(500.0, old * 0.8)  // Check 20% more frequently
      RETURN [ThresholdAdjustment { metric, old, new }]

    7 (uptime):
      service_id = find_least_available_service()
      current_threshold = circuit_breaker.get_threshold(service_id)
      new_threshold = max(1, current_threshold - 1)
      RETURN [CircuitBreakerTuning { service_id, new_threshold }]

    8 (synergy):
      weakest_pathways = hebbian.get_weakest_pathways(3)
      FOR each pathway IN weakest_pathways:
        delta = min(max_mutation_delta, 0.08)
        YIELD PathwayAdjustment { pathway_key: pathway.key, delta }

    9 (latency):
      hottest_service = find_highest_load_service()
      current_weight = load_balancer.get_weight(hottest_service)
      new_weight = max(0.1, current_weight * 0.85)
      RETURN [LoadBalancerReweight { service_id: hottest_service, new_weight }]

    10 (error_rate):
      error_prone = find_highest_error_rate_service()
      RETURN [EscalationTierShift { from: "L1", to: "L0" }]
      // Tighten: promote error-prone service to auto-execute tier

    11 (temporal):
      old_ltp = stdp.get_ltp_rate()
      new_ltp = min(0.3, old_ltp * 1.15)  // Increase by 15%
      RETURN [LearningRateAdjustment {
        parameter: "ltp_rate", old: old_ltp, new: new_ltp
      }]
```

### 4.3 From Periodic Optimization

```
ALGORITHM: generate_periodic() -> Vec<MutationRecord>

  IF system_state == Thriving AND no recent mutations (last 5 generations):
    // Explore: try small perturbations to find even better configurations
    SELECT random dimension D from [D6..D11]
    GENERATE small ThresholdAdjustment (delta = +/- 0.03)
    SET trigger = PeriodicOptimization

  IF system_state == Stable:
    // Focus on weakest dimension
    DELEGATE to generate_from_fitness_drift(current_report)
    SET trigger = PeriodicOptimization
```

### 4.4 From Meta-Learning

```
ALGORITHM: generate_from_meta_learning() -> Vec<MutationRecord>

  // Query mutation history for patterns
  successful = filter(mutation_history, |m| m.actual_delta > 0.0)
  failed = filter(mutation_history, |m| m.actual_delta < rollback_threshold)

  // Find mutation types with consistently high success rates
  FOR each mutation_type T:
    success_rate = count(successful, type=T) / count(all, type=T)
    avg_delta = mean(successful.actual_delta, type=T)

    IF success_rate > 0.8 AND avg_delta > 0.03:
      // This type works well -- generate more of it
      GENERATE new mutation of type T with expected_delta = avg_delta
      SET trigger = MetaLearning

  // Identify time-of-day patterns
  FOR each hour_bucket H in [0..23]:
    success_rate_at_H = count(successful at H) / count(all at H)
    IF success_rate_at_H > 0.9 AND current_hour == H:
      BOOST urgency_multiplier by 1.2
```

---

## 5. Safety Constraints

| Constraint | Value | Enforcement | Purpose |
|------------|-------|-------------|---------|
| `max_concurrent_mutations` | 3 | Pre-apply check | Prevent mutation interference |
| `max_mutation_delta` | 0.20 | Generation-time clamp | Limit parameter change magnitude |
| `rollback_threshold` | -0.02 | Verification verdict | Auto-rollback on fitness drop |
| `min_generation_interval_ms` | 60,000 | RALPH cycle gating | Prevent rapid mutation cycling |
| PBFT consensus for high-impact | `expected_delta >= 0.10` | Classify step | Multi-agent validation |
| `max_verification_extensions` | 2 | Extend verdict | Limit inconclusive mutation time |
| `LoadBalancerReweight` bounds | [0.1, 3.0] | Generation-time clamp | Prevent service starvation or overload |
| Rollback restores exact values | - | Apply/rollback symmetry | No state drift from failed mutations |
| Single-target uniqueness | One active mutation per target | Pre-apply check | Prevent conflicting mutations |

### Safety Invariants (Formal)

```
INV-1: forall t: active_mutations.len() <= config.max_concurrent_mutations
INV-2: forall m in mutations: abs(m.delta) <= config.max_mutation_delta
INV-3: forall m in rolled_back: system_param == m.original_value
INV-4: generation(t+1) > generation(t)  // monotonically increasing
INV-5: forall m where m.consensus_required: m.consensus_result.is_some() before apply
INV-6: time(generation_n+1) - time(generation_n) >= min_generation_interval_ms
```

---

## 6. Meta-Learning (NAM R1 SelfQuery)

The Evolution Chamber observes its own mutation history to improve future mutation generation. This implements NAM Requirement R1 (SelfQuery) at the meta-learning level.

### 6.1 Self-Observation Queries

| Query | Input | Output | Frequency |
|-------|-------|--------|-----------|
| Success rate by MutationType | `mutation_history` | `HashMap<MutationType, f64>` | Every generation |
| Success rate by MutationTrigger | `mutation_history` | `HashMap<MutationTrigger, f64>` | Every generation |
| Optimal expected_delta ranges | `mutation_history` | `HashMap<MutationType, (f64, f64)>` | Every 5 generations |
| Time-of-day effectiveness | `mutation_history` | `[f64; 24]` | Every 10 generations |
| Mutation interference patterns | `mutation_history` | `Vec<(MutationType, MutationType)>` | Every 10 generations |

### 6.2 Feedback Loop

```
Self-Observation Pipeline:

  mutation_history
       |
       v
  [Aggregate by type, trigger, time]
       |
       v
  [Compute success rates, avg deltas]
       |
       v
  [Update internal effectiveness model]
       |
       v
  [Adjust generation probabilities]
       |
       v
  [Feed into RALPH Phase 3 (Learn)]
```

### 6.3 Anti-Pattern Detection

| Condition | Classification | Action |
|-----------|---------------|--------|
| Success rate < 0.3 over >= 5 attempts | Mutation anti-pattern | Reduce generation probability by 50% |
| Two mutations of same type both fail | Possible interference | Log warning, avoid concurrent same-type |
| Rollback rate > 50% in last 10 generations | Systemic issue | Widen verification window by 50% |
| All mutations in a generation fail | Generation anti-pattern | Double `min_generation_interval_ms` temporarily |

---

## 7. API Contract

### 7.1 Constructor

```rust
/// Create a new EvolutionChamber with the given configuration.
///
/// # Preconditions
/// - `config.max_concurrent_mutations >= 1`
/// - `config.mutation_verification_ms >= 1000`
/// - `config.auto_apply_threshold > 0.0`
/// - `config.max_mutation_delta > 0.0 && config.max_mutation_delta <= 1.0`
///
/// # Postconditions
/// - `generation == 0`
/// - `active_mutations` is empty
/// - `mutation_history` is empty
/// - `fitness_history` is empty
///
/// # Errors
/// - `MaintenanceError::InvalidConfig` if any precondition is violated
pub fn new(config: EvolutionConfig) -> Result<Self>
```

### 7.2 RALPH Cycle

```rust
/// Execute one complete RALPH cycle (Recognize -> Analyze -> Learn -> Plan -> Harmonize).
///
/// This is the primary entry point, called on each generation tick.
///
/// # Preconditions
/// - At least one FitnessReport must be available from FitnessEvaluator
/// - Time since last generation >= min_generation_interval_ms
///   (unless system_state == Critical, which halves the interval)
///
/// # Postconditions
/// - `generation` is incremented by 1
/// - New FitnessSnapshot is appended to fitness_history
/// - Generated mutations are in mutation_history
/// - Approved mutations are in active_mutations (up to max_concurrent)
/// - EvolutionReport is published to EventBus "evolution" channel
///
/// # Errors
/// - `MaintenanceError::TooSoon` if generation interval not elapsed
/// - `MaintenanceError::NoFitnessData` if no FitnessReport available
/// - `MaintenanceError::ConsensusTimeout` if PBFT vote times out
pub async fn run_ralph_cycle(
    &self,
    emergences: &[EmergenceRecord],
    fitness: &FitnessReport,
) -> Result<EvolutionReport>
```

### 7.3 Mutation Verification

```rust
/// Check all active mutations that have passed their verification deadline
/// and render verdicts (Commit, Rollback, or Extend).
///
/// # Preconditions
/// - Called periodically (recommended: every 5 seconds)
///
/// # Postconditions
/// - Committed mutations are removed from active_mutations, recorded in history
/// - Rolled-back mutations revert their parameter change, recorded in history
/// - Extended mutations have their deadline pushed forward
///
/// # Errors
/// - `MaintenanceError::RollbackFailed` if parameter reversion fails
pub async fn verify_active_mutations(
    &self,
    current_fitness: f64,
) -> Result<Vec<(String, MutationVerdict)>>
```

### 7.4 Query Methods

```rust
/// Return the current generation number.
///
/// Thread-safe: uses AtomicU64 relaxed load.
pub fn current_generation(&self) -> u64

/// Classify the current system state from a fitness score.
///
/// Pure function, no side effects.
pub fn classify_state(fitness: f64) -> SystemState

/// Return the count of currently active (in-verification) mutations.
///
/// # Postconditions
/// - Return value <= config.max_concurrent_mutations
pub async fn active_mutation_count(&self) -> usize

/// Return the most recent N mutation records from history.
///
/// # Preconditions
/// - `limit > 0`
///
/// # Postconditions
/// - `result.len() <= limit`
/// - Results ordered newest-first
pub async fn get_mutation_history(&self, limit: usize) -> Vec<MutationRecord>

/// Return the most recent N fitness snapshots from history.
///
/// # Preconditions
/// - `limit > 0`
///
/// # Postconditions
/// - `result.len() <= limit`
/// - Results ordered newest-first
pub async fn get_fitness_history(&self, limit: usize) -> Vec<FitnessSnapshot>
```

### 7.5 Meta-Learning Methods

```rust
/// Compute success rate for each MutationType across all mutation history.
///
/// # Postconditions
/// - All keys present in result have at least 1 attempt
/// - Values in [0.0, 1.0]
pub async fn mutation_type_effectiveness(&self) -> HashMap<String, f64>

/// Compute success rate for each MutationTrigger across all mutation history.
///
/// # Postconditions
/// - All keys present in result have at least 1 attempt
/// - Values in [0.0, 1.0]
pub async fn trigger_effectiveness(&self) -> HashMap<String, f64>

/// Return mutation types classified as anti-patterns
/// (success_rate < 0.3 over >= 5 attempts).
///
/// # Postconditions
/// - All returned types have been attempted >= 5 times
pub async fn get_mutation_anti_patterns(&self) -> Vec<String>
```

### 7.6 Manual Override

```rust
/// Manually force a mutation to be applied, bypassing consensus.
///
/// Used by Human @0.A via NAM R5 Override capability.
///
/// # Preconditions
/// - Caller has L0 (Human) authority
/// - `active_mutations.len() < max_concurrent_mutations`
///
/// # Postconditions
/// - Mutation is applied immediately
/// - Recorded in history with trigger = MetaLearning
/// - Audit log entry created
///
/// # Errors
/// - `MaintenanceError::Unauthorized` if caller lacks authority
/// - `MaintenanceError::TooManyActive` if at capacity
pub async fn force_apply_mutation(
    &self,
    mutation: MutationType,
    authority: &AgentId,
) -> Result<String>
```

---

## 8. Testing Matrix

| Category | Count | Description |
|----------|-------|-------------|
| RALPH cycle | 10 | Full cycle with each `SystemState` (Thriving, Stable, Degraded, Critical); cycle with no emergences; cycle with multiple emergences; cycle with fitness drift; cycle with meta-learning trigger; cycle respects `min_generation_interval_ms`; Critical state halves interval |
| Mutation generation | 8 | One test per `EmergentBehavior` trigger (7 variants) plus one for fitness drift across all dimensions |
| Mutation lifecycle | 8 | Auto-apply path; PBFT-approved path; PBFT-rejected path; verification commit; verification rollback; verification extend; extend then commit; extend then rollback |
| Safety constraints | 6 | Max concurrent mutations enforced; max delta clamped; rollback threshold triggers revert; generation interval enforced; single-target uniqueness; PBFT required for high-impact |
| Meta-learning | 5 | Success rate computation; trigger effectiveness; anti-pattern detection; time-of-day patterns; generation probability adjustment |
| Fitness trending | 5 | Baseline computation from last 10 generations; trend detection (improving, declining, stable); dimension deficit ranking; weighted impact scoring; empty history edge case |
| Generation management | 4 | Generation counter increments; generation counter monotonic; interval enforcement; state-dependent timing |
| Edge cases | 4 | Empty mutation history; all mutation slots active; concurrent RALPH cycles prevented; rollback restores exact value |

### Test Summary

| Metric | Value |
|--------|-------|
| **Total Tests** | 50 |
| **Unit Tests** | 42 |
| **Integration Tests** | 6 |
| **Property Tests** | 2 |

---

## 9. Cross-Spec Dependencies

| Dependency | Source Spec | Integration Point | Direction |
|------------|-------------|-------------------|-----------|
| M38 Emergence Detector | EMERGENCE_DETECTOR_SPEC | `EmergenceRecord` consumed by M39 | M38 -> M39 |
| FitnessEvaluator | FITNESS_FUNCTION_SPEC | `FitnessReport` consumed by M39 | FitnessEval -> M39 |
| M31 PBFT Manager | [PBFT_SPEC](../../PBFT_SPEC.md) | High-impact mutation consensus | M39 -> M31 |
| M25 Hebbian Manager | [STDP_SPEC](../../STDP_SPEC.md) | `PathwayAdjustment` application + LTP/LTD feedback | M39 -> M25 |
| M12 Circuit Breaker | [SERVICE_SPEC](../../SERVICE_SPEC.md) | `CircuitBreakerTuning` application | M39 -> M12 |
| M11 Load Balancer | [SERVICE_SPEC](../../SERVICE_SPEC.md) | `LoadBalancerReweight` application | M39 -> M11 |
| M23 Event Bus | [PIPELINE_SPEC](../../PIPELINE_SPEC.md) | `EvolutionReport` publication | M39 -> M23 |
| M02 Configuration | [SYSTEM_SPEC](../../SYSTEM_SPEC.md) | `ThresholdAdjustment` application | M39 -> M02 |
| M15 Confidence Calculator | [ESCALATION_SPEC](../../ESCALATION_SPEC.md) | `EscalationTierShift` application | M39 -> M15 |
| 12D Tensor | [TENSOR_SPEC](../../TENSOR_SPEC.md) | Dimension scores in `FitnessSnapshot` | Indirect via FitnessEvaluator |

---

## 10. Configuration File

```toml
[evolution]
max_concurrent_mutations = 3
mutation_verification_ms = 30000
fitness_history_capacity = 500
mutation_history_capacity = 1000
auto_apply_threshold = 0.10
rollback_threshold = -0.02
min_generation_interval_ms = 60000
max_mutation_delta = 0.20
max_verification_extensions = 2

[evolution.weights]
# Load balancer weight bounds
lb_weight_min = 0.1
lb_weight_max = 3.0

# Circuit breaker threshold bounds
cb_threshold_min = 1
cb_threshold_max = 20

# Learning rate bounds (matches STDP_SPEC Section 2.1)
ltp_rate_min = 0.01
ltp_rate_max = 0.30
ltd_rate_min = 0.01
ltd_rate_max = 0.20
```

---

## 11. Implementation Constants

```rust
pub mod evolution {
    /// Maximum concurrent active mutations
    pub const MAX_CONCURRENT_MUTATIONS: usize = 3;

    /// Default verification window (30 seconds)
    pub const DEFAULT_VERIFICATION_MS: u64 = 30_000;

    /// Default minimum generation interval (1 minute)
    pub const DEFAULT_GENERATION_INTERVAL_MS: u64 = 60_000;

    /// Maximum parameter delta per mutation
    pub const MAX_MUTATION_DELTA: f64 = 0.20;

    /// Threshold for requiring PBFT consensus
    pub const AUTO_APPLY_THRESHOLD: f64 = 0.10;

    /// Threshold for automatic rollback
    pub const ROLLBACK_THRESHOLD: f64 = -0.02;

    /// Maximum verification extensions before forced verdict
    pub const MAX_VERIFICATION_EXTENSIONS: u32 = 2;

    /// Fitness history buffer capacity
    pub const FITNESS_HISTORY_CAPACITY: usize = 500;

    /// Mutation history buffer capacity
    pub const MUTATION_HISTORY_CAPACITY: usize = 1_000;

    /// Load balancer weight bounds
    pub const LB_WEIGHT_MIN: f64 = 0.1;
    pub const LB_WEIGHT_MAX: f64 = 3.0;

    /// Anti-pattern detection: minimum attempts before classification
    pub const ANTI_PATTERN_MIN_ATTEMPTS: usize = 5;

    /// Anti-pattern detection: success rate threshold
    pub const ANTI_PATTERN_THRESHOLD: f64 = 0.3;

    /// SystemState fitness thresholds
    pub const THRIVING_THRESHOLD: f64 = 0.90;
    pub const STABLE_THRESHOLD: f64 = 0.75;
    pub const DEGRADED_THRESHOLD: f64 = 0.50;
}
```

---

## 12. Version History

| Version | Date | Changes |
|---------|------|---------|
| 1.0.0 | 2026-01-29 | Initial specification |

---

*Generated: 2026-01-29 | The Maintenance Engine v1.0.0 | L7 Observer Layer*
