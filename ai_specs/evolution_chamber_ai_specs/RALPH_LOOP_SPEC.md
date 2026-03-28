# RALPH Loop Protocol - Formal Specification

```json
{"v":"1.0.0","type":"PROTOCOL_SPEC","name":"RALPH Loop","version":"2.0","phases":5,"layer":7}
```

| Property | Value |
|----------|-------|
| **Protocol** | RALPH (Recognize, Analyze, Learn, Plan, Harmonize) |
| **Version** | 2.0 (current) |
| **Phases** | 5 |
| **Layer** | L7 Observer |
| **Executor** | M39 Evolution Chamber |
| **Status** | SPECIFIED |

---

## Navigation

| Direction | Target |
|-----------|--------|
| Up | [INDEX.md](INDEX.md) |
| Prev | [OBSERVER_BUS_SPEC.md](OBSERVER_BUS_SPEC.md) |
| Next | [FITNESS_FUNCTION_SPEC.md](FITNESS_FUNCTION_SPEC.md) |

| Related Spec | Relationship |
|--------------|-------------|
| [EVOLUTION_CHAMBER_SPEC.md](EVOLUTION_CHAMBER_SPEC.md) | Module that executes this protocol |
| [EMERGENCE_DETECTOR_SPEC.md](EMERGENCE_DETECTOR_SPEC.md) | Provides `EmergenceRecord` input |
| [FITNESS_FUNCTION_SPEC.md](FITNESS_FUNCTION_SPEC.md) | Provides `FitnessReport` input |
| [../../PBFT_SPEC.md](../../PBFT_SPEC.md) | Consensus protocol for Phase 4/5 |
| [../../STDP_SPEC.md](../../STDP_SPEC.md) | Learning pathway feedback in Phase 3 |
| [../../ESCALATION_SPEC.md](../../ESCALATION_SPEC.md) | Escalation tier integration |

---

## 1. Protocol Overview

**RALPH** = **R**ecognize, **A**nalyze, **L**earn, **P**lan, **H**armonize

A 5-phase meta-learning loop that drives continuous system evolution. Each complete execution of all 5 phases is called a **generation**. The generation counter is monotonically increasing and persisted across restarts.

### 1.1 Version History

| Version | Status | Description |
|---------|--------|-------------|
| 1.0 | DEPRECATED | Initial 8-iteration loop; achieved 37% to 98% fitness progression during bootstrap; deprecated after 2 uses due to fixed iteration count |
| 2.0 | CURRENT | Continuous evolution with generation-based adaptive timing; no fixed iteration count; runs indefinitely |

### 1.2 Protocol Summary

```
+---------------------------------------------------------------+
|                    RALPH v2.0 Generation Cycle                 |
+---------------------------------------------------------------+
|                                                                |
|  Phase 1: RECOGNIZE                                            |
|  Input:  EmergenceRecord[], FitnessReport                      |
|  Output: SystemState, StateContext, Urgency                    |
|                                                                |
|  Phase 2: ANALYZE                                              |
|  Input:  SystemState, StateContext, FitnessReport,             |
|          EmergenceRecord[]                                     |
|  Output: AnalysisResult (root causes, underperformers)         |
|                                                                |
|  Phase 3: LEARN                                                |
|  Input:  AnalysisResult, MutationHistory                       |
|  Output: LearningUpdate (effectiveness model, anti-patterns)   |
|                                                                |
|  Phase 4: PLAN                                                 |
|  Input:  AnalysisResult, LearningUpdate                        |
|  Output: Vec<MutationRecord> (ranked, filtered, classified)    |
|                                                                |
|  Phase 5: HARMONIZE                                            |
|  Input:  Vec<MutationRecord> (approved)                        |
|  Output: EvolutionReport (mutations applied, fitness delta)    |
|                                                                |
+---------------------------------------------------------------+
|  Increment generation counter                                  |
|  Wait for next generation interval (state-dependent)           |
+---------------------------------------------------------------+
```

---

## 2. Phase Specifications

### 2.1 Phase 1: RECOGNIZE

Classify the current system state and determine urgency.

#### Input

| Parameter | Type | Source |
|-----------|------|--------|
| `emergences` | `&[EmergenceRecord]` | M38 Emergence Detector (via Observer Bus) |
| `fitness` | `&FitnessReport` | `FitnessEvaluator` |

#### Output

| Parameter | Type | Description |
|-----------|------|-------------|
| `system_state` | `SystemState` | Current health classification |
| `state_context` | `StateContext` | Contextual metadata |
| `urgency` | `Urgency` | Mutation urgency level |

#### Procedure

```
PHASE 1: RECOGNIZE

  STEP 1 - Classify SystemState from FitnessReport.overall_fitness:
    IF fitness >= 0.90 THEN state = Thriving
    ELIF fitness >= 0.75 THEN state = Stable
    ELIF fitness >= 0.50 THEN state = Degraded
    ELSE state = Critical

  STEP 2 - Build StateContext:
    state_context = StateContext {
      active_emergences: count of unacknowledged EmergenceRecords,
      fitness_trend:     FitnessReport.trend,       // Improving | Stable | Declining | Volatile
      active_mutations:  count of in-progress mutations from EvolutionChamber,
      generation:        current generation number,
      last_state:        previous generation's SystemState,
      state_duration:    consecutive generations in current state,
    }

  STEP 3 - Determine urgency:
    MATCH (state, fitness_trend):
      (Critical,  Declining)  => URGENT    // Skip normal intervals
      (Critical,  _)          => URGENT    // Any critical is urgent
      (Degraded,  Declining)  => HIGH
      (Degraded,  Stable)     => HIGH
      (Degraded,  Improving)  => NORMAL    // Already recovering
      (Stable,    Declining)  => NORMAL
      (Stable,    _)          => NORMAL
      (Thriving,  Declining)  => NORMAL    // Watch but don't rush
      (Thriving,  Improving)  => LOW       // Skip optional mutations
      (Thriving,  Stable)     => LOW

  RETURN (system_state, state_context, urgency)
```

#### Type Definitions

```rust
/// Contextual metadata accompanying SystemState classification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateContext {
    /// Number of unacknowledged EmergenceRecords from M38
    pub active_emergences: usize,

    /// Fitness trend over recent generations
    pub fitness_trend: FitnessTrend,

    /// Number of mutations currently in verification
    pub active_mutations: usize,

    /// Current generation number
    pub generation: u64,

    /// SystemState from previous generation (None if first generation)
    pub last_state: Option<SystemState>,

    /// Number of consecutive generations in the current state
    pub state_duration: u64,
}

/// Urgency classification for the current generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Urgency {
    /// System is optimal; skip optional mutations
    Low,

    /// Standard operation; apply normal mutation logic
    Normal,

    /// System needs attention; prioritize mutations
    High,

    /// System in crisis; skip normal intervals, apply immediately
    Urgent,
}
```

#### Timing

| Metric | Target | Maximum |
|--------|--------|---------|
| Phase 1 latency | < 1ms | 10ms |
| Memory allocation | 0 heap allocations | - |

---

### 2.2 Phase 2: ANALYZE

Compare current fitness to historical baselines and identify root causes of underperformance.

#### Input

| Parameter | Type | Source |
|-----------|------|--------|
| `system_state` | `SystemState` | Phase 1 output |
| `state_context` | `StateContext` | Phase 1 output |
| `fitness` | `&FitnessReport` | `FitnessEvaluator` |
| `emergences` | `&[EmergenceRecord]` | M38 Emergence Detector |

#### Output

| Parameter | Type | Description |
|-----------|------|-------------|
| `analysis` | `AnalysisResult` | Root causes, underperformers, correlations |

#### Procedure

```
PHASE 2: ANALYZE

  STEP 1 - Compute baseline from historical fitness:
    baseline = mean(fitness_history[last 10 generations].overall_fitness)
    delta_from_baseline = current_fitness - baseline

    IF fitness_history.len() < 10:
      baseline = mean(fitness_history[all].overall_fitness)
    IF fitness_history.is_empty():
      baseline = 0.75  // Conservative default

  STEP 2 - Identify underperforming dimensions:
    underperformers = []
    FOR each dimension D in [D0..D11]:
      baseline_score_D = mean(fitness_history[last 10].dimension_scores[D])
      current_score_D  = fitness.dimension_scores[D]

      IF current_score_D < baseline_score_D * 0.95:
        deficit = baseline_score_D - current_score_D
        weight  = DIMENSION_WEIGHTS[D]
        impact  = weight * deficit
        underperformers.push(UnderperformerRecord {
          dimension: D,
          current: current_score_D,
          baseline: baseline_score_D,
          deficit: deficit,
          impact: impact,
        })

    SORT underperformers BY impact DESC

  STEP 3 - Correlate emergent behaviors with fitness deltas:
    FOR each EmergenceRecord E in emergences:
      fitness_at_emergence = lookup_fitness_at(E.detected_at)
      fitness_now = current_fitness

      E.fitness_impact = fitness_now - fitness_at_emergence
      // Positive impact = emergence helped, Negative = emergence hurt

  STEP 4 - Determine root cause candidates:
    root_causes = []

    // From underperforming dimensions
    FOR each U in underperformers[top 3]:
      root_causes.push(RootCause {
        source: RootCauseSource::FitnessDrift(U.dimension),
        impact: U.impact,
        confidence: min(1.0, U.deficit / 0.10),  // Higher deficit = more confident
      })

    // From emergence records
    FOR each E in emergences WHERE E.fitness_impact < -0.01:
      root_causes.push(RootCause {
        source: RootCauseSource::Emergence(E.id),
        impact: abs(E.fitness_impact),
        confidence: E.confidence,
      })

    SORT root_causes BY (impact * confidence) DESC

  RETURN AnalysisResult {
    baseline,
    delta_from_baseline,
    underperformers,
    root_causes,
    emergence_correlations,
  }
```

#### Type Definitions

```rust
/// Result of the Analyze phase.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisResult {
    /// Historical baseline fitness (mean of last 10 generations)
    pub baseline: f64,

    /// Current fitness minus baseline
    pub delta_from_baseline: f64,

    /// Dimensions performing below baseline, sorted by impact
    pub underperformers: Vec<UnderperformerRecord>,

    /// Prioritized root causes combining fitness drift and emergence correlation
    pub root_causes: Vec<RootCause>,

    /// Emergence records annotated with fitness impact
    pub emergence_correlations: Vec<EmergenceCorrelation>,
}

/// A dimension performing below its historical baseline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnderperformerRecord {
    /// Dimension index [0..11]
    pub dimension: usize,

    /// Current score for this dimension
    pub current: f64,

    /// Historical baseline for this dimension
    pub baseline: f64,

    /// baseline - current
    pub deficit: f64,

    /// weight * deficit (higher = more impactful)
    pub impact: f64,
}

/// Identified root cause for system underperformance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RootCause {
    /// What produced this root cause
    pub source: RootCauseSource,

    /// Magnitude of impact [0.0, 1.0]
    pub impact: f64,

    /// Confidence in this root cause [0.0, 1.0]
    pub confidence: f64,
}

/// Source of a root cause.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RootCauseSource {
    /// A 12D tensor dimension drifting below baseline (carries dimension index)
    FitnessDrift(usize),

    /// An emergent behavior correlated with fitness decline (carries emergence_id)
    Emergence(String),
}

/// An EmergenceRecord annotated with its fitness impact.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmergenceCorrelation {
    /// Reference to the EmergenceRecord
    pub emergence_id: String,

    /// Fitness delta since emergence was detected
    pub fitness_impact: f64,

    /// The emergent behavior type
    pub behavior: String,
}
```

#### Dimension Weight Table

| Dimension | Index | Weight | Justification |
|-----------|-------|--------|---------------|
| D0 service_id | 0 | 0.02 | Identifier, rarely drifts |
| D1 port | 1 | 0.02 | Identifier, rarely drifts |
| D2 tier | 2 | 0.03 | Structural, rarely changes |
| D3 deps | 3 | 0.05 | Dependency health matters |
| D4 agents | 4 | 0.05 | Agent allocation stability |
| D5 protocol | 5 | 0.03 | Protocol stability |
| D6 health | 6 | 0.18 | Primary health indicator |
| D7 uptime | 7 | 0.15 | Availability is critical |
| D8 synergy | 8 | 0.15 | Cross-service coupling |
| D9 latency | 9 | 0.12 | Performance indicator |
| D10 error_rate | 10 | 0.12 | Reliability indicator |
| D11 temporal | 11 | 0.08 | Adaptation speed |
| **Total** | - | **1.00** | - |

#### Timing

| Metric | Target | Maximum |
|--------|--------|---------|
| Phase 2 latency | < 5ms | 50ms |
| Fitness history lookups | O(1) per generation | - |

---

### 2.3 Phase 3: LEARN

Review recent mutation outcomes, update the effectiveness model, and emit learning signals.

#### Input

| Parameter | Type | Source |
|-----------|------|--------|
| `analysis` | `AnalysisResult` | Phase 2 output |
| `mutation_history` | `&VecDeque<MutationRecord>` | EvolutionChamber internal state |

#### Output

| Parameter | Type | Description |
|-----------|------|-------------|
| `learning_update` | `LearningUpdate` | Updated model, anti-patterns, pathway signals |

#### Procedure

```
PHASE 3: LEARN

  STEP 1 - Review recent mutation outcomes:
    recent_window = last N generations (N = min(10, mutation_history.len()))
    completed = filter(mutation_history, |m| m.verified_at.is_some()
                       AND m.generation >= current_generation - N)

    successes = []
    failures = []
    FOR each MutationRecord M in completed:
      IF M.actual_delta.unwrap_or(0.0) > 0.0:
        successes.push((M.mutation.type_name(), M.trigger, M.actual_delta))
      ELSE:
        failures.push((M.mutation.type_name(), M.trigger, M.actual_delta))

  STEP 2 - Update mutation effectiveness model:
    FOR each MutationType T:
      attempts_T   = count(completed, type = T)
      successes_T  = count(successes, type = T)

      IF attempts_T > 0:
        success_rate_T = successes_T / attempts_T
        avg_delta_T    = mean(successes.actual_delta, type = T)
        UPDATE model[T] = EffectivenessEntry {
          success_rate: success_rate_T,
          avg_delta: avg_delta_T,
          attempts: attempts_T,
          last_updated: now(),
        }

  STEP 3 - Pathway feedback (L5 integration via M25 HebbianManager):
    FOR each successful PathwayAdjustment mutation:
      pathway_key = mutation.pathway_key
      EMIT LTP signal to HebbianManager(M25):
        target: pathway_key
        strength: actual_delta * 0.5  // Scale signal to half the fitness gain

    FOR each failed PathwayAdjustment mutation:
      pathway_key = mutation.pathway_key
      EMIT LTD signal to HebbianManager(M25):
        target: pathway_key
        strength: abs(actual_delta) * 0.3  // Weaker signal for failure

  STEP 4 - Anti-pattern recording:
    FOR each MutationType T:
      IF model[T].attempts >= ANTI_PATTERN_MIN_ATTEMPTS (5):
        IF model[T].success_rate < ANTI_PATTERN_THRESHOLD (0.3):
          IF T NOT IN anti_patterns:
            anti_patterns.insert(T)
            LOG warn: "Mutation type {T} classified as anti-pattern:
                       success_rate={success_rate:.2}, attempts={attempts}"

          // Reduce generation probability
          model[T].generation_probability *= 0.5

  RETURN LearningUpdate {
    effectiveness_model: model,
    anti_patterns: anti_patterns,
    pathway_signals_emitted: signal_count,
    learning_events: completed.len(),
  }
```

#### Type Definitions

```rust
/// Result of the Learn phase.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearningUpdate {
    /// Updated effectiveness model keyed by mutation type name
    pub effectiveness_model: HashMap<String, EffectivenessEntry>,

    /// Set of mutation type names classified as anti-patterns
    pub anti_patterns: HashSet<String>,

    /// Number of LTP/LTD pathway signals emitted to M25
    pub pathway_signals_emitted: usize,

    /// Number of completed mutations reviewed this cycle
    pub learning_events: usize,
}

/// Effectiveness statistics for a single MutationType.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EffectivenessEntry {
    /// Fraction of attempts that resulted in positive fitness delta
    pub success_rate: f64,

    /// Mean actual_delta for successful mutations
    pub avg_delta: f64,

    /// Total number of attempts
    pub attempts: usize,

    /// Probability multiplier for generation (reduced for anti-patterns)
    pub generation_probability: f64,

    /// Last time this entry was updated
    pub last_updated: DateTime<Utc>,
}

impl Default for EffectivenessEntry {
    fn default() -> Self {
        Self {
            success_rate: 0.5,       // Neutral prior
            avg_delta: 0.0,
            attempts: 0,
            generation_probability: 1.0,
            last_updated: Utc::now(),
        }
    }
}
```

#### Timing

| Metric | Target | Maximum |
|--------|--------|---------|
| Phase 3 latency | < 10ms | 100ms |
| Pathway signal emission | Async (non-blocking) | - |

---

### 2.4 Phase 4: PLAN

Generate, filter, score, and classify mutation candidates.

#### Input

| Parameter | Type | Source |
|-----------|------|--------|
| `analysis` | `AnalysisResult` | Phase 2 output |
| `learning` | `LearningUpdate` | Phase 3 output |
| `urgency` | `Urgency` | Phase 1 output |

#### Output

| Parameter | Type | Description |
|-----------|------|-------------|
| `candidates` | `Vec<MutationRecord>` | Ranked, filtered, classified mutation candidates |

#### Procedure

```
PHASE 4: PLAN

  STEP 1 - Generate candidate mutations:
    candidates = []
    FOR each root_cause R in analysis.root_causes:
      MATCH R.source:
        FitnessDrift(dim):
          new_mutations = generate_from_fitness_drift(dim)
          // See EVOLUTION_CHAMBER_SPEC Section 4.2
        Emergence(id):
          record = lookup_emergence(id)
          new_mutations = generate_from_emergence(record)
          // See EVOLUTION_CHAMBER_SPEC Section 4.1

      FOR each mutation M in new_mutations:
        M.expected_delta = estimate_delta(M, learning.effectiveness_model)
        M.trigger = match R.source {
          FitnessDrift(dim) => MutationTrigger::FitnessDrift(dim_name(dim)),
          Emergence(id)     => MutationTrigger::EmergenceResponse(id),
        }
        candidates.push(M)

  STEP 2 - Filter by safety constraints:
    candidates.retain(|M| {
      // Magnitude check
      abs(M.expected_delta) <= config.max_mutation_delta
      // Capacity check
      AND active_mutations.len() + approved_count < config.max_concurrent_mutations
      // Uniqueness check: no active mutation for same target
      AND NOT active_mutations.contains_key(M.target_key())
      // Anti-pattern check
      AND NOT learning.anti_patterns.contains(M.mutation.type_name())
      // Generation probability check
      AND random() < learning.effectiveness_model[M.type_name()].generation_probability
    })

  STEP 3 - Score and rank candidates:
    FOR each candidate M in candidates:
      confidence = learning.effectiveness_model[M.type_name()].success_rate
      urgency_multiplier = MATCH urgency:
        Urgent => 2.0
        High   => 1.5
        Normal => 1.0
        Low    => 0.5

      M.score = M.expected_delta * confidence * urgency_multiplier

    SORT candidates BY score DESC

  STEP 4 - Classify by consensus requirement:
    FOR each candidate M in candidates:
      IF abs(M.expected_delta) >= config.auto_apply_threshold (0.10):
        M.consensus_required = true   // Queue for L3 PBFT
      ELSE:
        M.consensus_required = false  // L0 auto-apply

  STEP 5 - Select top candidates:
    available_slots = config.max_concurrent_mutations - active_mutations.len()
    selected = candidates[..min(available_slots, candidates.len())]

  RETURN selected
```

#### Urgency Multiplier Table

| Urgency | Multiplier | Effect |
|---------|-----------|--------|
| Urgent | 2.0 | Double scoring weight; system in crisis |
| High | 1.5 | Elevated priority; degraded and declining |
| Normal | 1.0 | Standard operation |
| Low | 0.5 | Halved priority; system is thriving |

#### Timing

| Metric | Target | Maximum |
|--------|--------|---------|
| Phase 4 latency | < 10ms | 100ms |
| Candidate generation | O(root_causes * variants) | - |

---

### 2.5 Phase 5: HARMONIZE

Apply approved mutations, verify outcomes, update homeostatic targets, and publish the evolution report.

#### Input

| Parameter | Type | Source |
|-----------|------|--------|
| `mutations` | `Vec<MutationRecord>` | Phase 4 output (after PBFT approval where required) |
| `system_state` | `SystemState` | Phase 1 output |
| `state_context` | `StateContext` | Phase 1 output |

#### Output

| Parameter | Type | Description |
|-----------|------|-------------|
| `report` | `EvolutionReport` | Summary of generation outcomes |

#### Procedure

```
PHASE 5: HARMONIZE

  STEP 1 - Apply approved mutations:
    applied = []
    FOR each mutation M in mutations:
      IF M.consensus_required AND NOT M.consensus_result.unwrap_or(false):
        SKIP  // PBFT rejected

      fitness_before = current_fitness()

      MATCH M.mutation:
        PathwayAdjustment { pathway_key, delta }:
          hebbian_manager.adjust_weight(pathway_key, delta)?

        ThresholdAdjustment { metric, old, new }:
          config_manager.set(metric, new)?

        LearningRateAdjustment { parameter, old, new }:
          stdp_processor.set_parameter(parameter, new)?

        CircuitBreakerTuning { service_id, new_threshold }:
          circuit_breaker.set_threshold(service_id, new_threshold)?

        LoadBalancerReweight { service_id, new_weight }:
          load_balancer.set_weight(service_id, new_weight)?

        EscalationTierShift { from, to }:
          escalation.shift_tier(from, to)?

        HomeostaticTargetAdjustment { target, old, new }:
          homeostatic.set_target(target, new)?

      verification_deadline = now() + Duration::from_millis(config.mutation_verification_ms)
      active_mutations.insert(M.id, ActiveMutation {
        mutation_id: M.id,
        mutation: M.mutation,
        fitness_before,
        applied_at: now(),
        verification_deadline,
        extensions_used: 0,
      })
      M.applied = true
      M.applied_at = Some(now())
      mutation_history.push(M)
      applied.push(M.id)

  STEP 2 - Verify active mutations past deadline:
    verdicts = []
    FOR each (id, active) in active_mutations WHERE now() >= active.verification_deadline:
      fitness_after = current_fitness()
      actual_delta = fitness_after - active.fitness_before

      verdict = MATCH actual_delta:
        d IF d >= config.rollback_threshold (-0.02):
          MutationVerdict::Commit

        d IF d < config.rollback_threshold
             AND active.extensions_used < config.max_verification_extensions:
          MutationVerdict::Extend

        _:
          MutationVerdict::Rollback

      MATCH verdict:
        Commit:
          UPDATE mutation_history[id]:
            actual_delta = Some(actual_delta)
            verified_at = Some(now())
          REMOVE from active_mutations
          verdicts.push((id, Commit))

        Rollback:
          // Revert the parameter to its original value
          REVERT active.mutation  // Uses old/original values stored in MutationType
          UPDATE mutation_history[id]:
            actual_delta = Some(actual_delta)
            rolled_back = true
            verified_at = Some(now())
          REMOVE from active_mutations
          verdicts.push((id, Rollback))

        Extend:
          active.verification_deadline = now() + Duration::from_millis(config.mutation_verification_ms)
          active.extensions_used += 1
          verdicts.push((id, Extend))

  STEP 3 - Update homeostatic targets:
    IF system_state == Thriving AND state_context.state_duration >= 3:
      // System has been thriving for 3+ consecutive generations
      // Raise the bar: increase homeostatic targets
      homeostatic.adjust_targets(+0.01)
      // e.g. target_health: 0.85 -> 0.86, target_synergy: 0.90 -> 0.91

    IF system_state == Degraded AND state_context.state_duration >= 2:
      // System has been degraded for 2+ consecutive generations
      // Lower expectations temporarily to stabilize
      homeostatic.adjust_targets(-0.01)
      // e.g. target_health: 0.85 -> 0.84

  STEP 4 - Publish evolution report:
    report = EvolutionReport {
      generation: current_generation,
      system_state,
      mutations_generated: mutations.len(),
      mutations_applied: applied.len(),
      mutations_committed: count(verdicts, Commit),
      mutations_rolled_back: count(verdicts, Rollback),
      mutations_extended: count(verdicts, Extend),
      fitness_before: fitness_at_generation_start,
      fitness_after: current_fitness(),
      fitness_delta: current_fitness() - fitness_at_generation_start,
      timestamp: now(),
    }

    event_bus.publish("evolution", &report)?

  STEP 5 - Increment generation counter:
    generation.fetch_add(1, Ordering::SeqCst)

  RETURN report
```

#### Type Definitions

```rust
/// Summary report published at the end of each RALPH generation.
///
/// Published to EventBus channel "evolution" for consumption by
/// other L7 components and the monitoring dashboard.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvolutionReport {
    /// Generation number for this report
    pub generation: u64,

    /// System state at time of report
    pub system_state: SystemState,

    /// Total mutations generated during Plan phase
    pub mutations_generated: usize,

    /// Mutations successfully applied (may be fewer due to PBFT rejection or capacity)
    pub mutations_applied: usize,

    /// Mutations that passed verification and were committed
    pub mutations_committed: usize,

    /// Mutations that failed verification and were rolled back
    pub mutations_rolled_back: usize,

    /// Mutations whose verification was extended (inconclusive)
    pub mutations_extended: usize,

    /// System fitness at the start of this generation
    pub fitness_before: f64,

    /// System fitness at the end of this generation
    pub fitness_after: f64,

    /// fitness_after - fitness_before
    pub fitness_delta: f64,

    /// Timestamp of report generation
    pub timestamp: DateTime<Utc>,
}
```

#### Timing

| Metric | Target | Maximum |
|--------|--------|---------|
| Phase 5 latency (apply) | < 50ms | 500ms |
| Phase 5 latency (verify) | < 10ms per mutation | 100ms |
| EventBus publish | < 1ms | 10ms |

---

## 3. Generation Timing

The interval between consecutive RALPH cycles adapts to the current system state. More critical states trigger faster evolution.

### 3.1 Interval Calculation

```
FUNCTION compute_generation_interval(state: SystemState, config: &EvolutionConfig) -> Duration:
  base = config.min_generation_interval_ms  // 60,000ms default

  interval_ms = MATCH state:
    Critical  => base / 2        //  30,000ms (30s)
    Degraded  => base            //  60,000ms (60s)
    Stable    => base * 2        // 120,000ms (2min)
    Thriving  => base * 5        // 300,000ms (5min)

  RETURN Duration::from_millis(interval_ms)
```

### 3.2 Interval Table

| SystemState | Multiplier | Interval (default base) | Rationale |
|-------------|-----------|------------------------|-----------|
| Critical | 0.5x | 30 seconds | Rapid response needed |
| Degraded | 1.0x | 60 seconds | Active correction |
| Stable | 2.0x | 2 minutes | Monitoring, occasional optimization |
| Thriving | 5.0x | 5 minutes | Minimal intervention, exploration only |

### 3.3 Urgency Override

```
IF urgency == Urgent:
  interval = 0  // Run immediately, ignore minimum interval
  LOG warn: "URGENT generation triggered: system_state={state}, trend={trend}"
```

### 3.4 Generation Lifecycle

```
+-------------------+
| Wait for interval |<----+
+--------+----------+     |
         |                |
         v                |
+-------------------+     |
| Run RALPH Cycle   |     |
| (Phases 1-5)      |     |
+--------+----------+     |
         |                |
         v                |
+-------------------+     |
| Compute next      |     |
| interval from     |-----+
| new SystemState   |
+-------------------+
```

---

## 4. PBFT Integration for High-Impact Mutations

Mutations classified as high-impact (`expected_delta >= auto_apply_threshold`) are submitted to the L6 PBFT Manager (M31) for multi-agent consensus before application.

### 4.1 Consensus Flow

```
WHEN consensus_required == true:

  STEP 1 - Package mutation as PBFT proposal:
    proposal = PbftProposal {
      proposal_type: "evolution_mutation",
      payload: serialize(&mutation_record),
      urgency: urgency,
      requester: "M39_EvolutionChamber",
      generation: current_generation,
    }

  STEP 2 - Submit to M31 PBFT Manager:
    pbft_manager.submit_proposal(proposal)?

  STEP 3 - Agent role-based evaluation:
    // Each agent role evaluates the mutation differently
    // See Role Evaluation Table below

  STEP 4 - Require 27/40 weighted quorum:
    IF weighted_votes_for >= 27:
      consensus_result = true   // Approved
    ELSE:
      consensus_result = false  // Rejected

  STEP 5 - Proceed or discard:
    IF approved:
      proceed to HARMONIZE Phase 5 apply step
      LOG info: "Mutation {id} approved by PBFT ({votes_for}/40)"
    ELSE:
      DISCARD mutation
      RECORD rejection in mutation_history with:
        consensus_result = Some(false)
        reason = dissent_summary from CRITIC agents
      LOG info: "Mutation {id} rejected by PBFT ({votes_for}/40)"
```

### 4.2 Agent Role Evaluation Table

| Role | Count | Weight | Evaluation Focus | Approval Criteria |
|------|-------|--------|-----------------|-------------------|
| VALIDATOR | 20 | 1.0 | Parameter bounds and safety constraints | Mutation within bounds, no constraint violation |
| EXPLORER | 8 | 0.8 | Alternative mutations that might be better | Approve if no clearly better alternative found |
| CRITIC | 6 | 1.2 | Potential negative effects and risks | Approve only if no significant risk identified |
| INTEGRATOR | 4 | 1.0 | Cross-system impact assessment | Approve if no adverse cross-system effects |
| HISTORIAN | 2 | 0.8 | Historical precedent from mutation_history | Approve if similar mutations succeeded before |

### 4.3 Weighted Vote Calculation

```
total_weight = 0.0
for_weight   = 0.0

FOR each agent_vote V:
  total_weight += role_weight(V.role)
  IF V.vote == Approve:
    for_weight += role_weight(V.role)

// Quorum is 27/40 of agent count, but weighted:
// Weighted total = 20*1.0 + 8*0.8 + 6*1.2 + 4*1.0 + 2*0.8 = 39.2
// Simple majority would be 19.6, but we use super-majority
approved = for_weight >= 27.0 * (39.2 / 40.0)
// Equivalent: for_weight >= 26.46
```

### 4.4 PBFT Timeout

| Parameter | Value | Action on Timeout |
|-----------|-------|-------------------|
| Vote collection timeout | 30 seconds | Discard mutation, record as `ConsensusTimeout` |
| Urgent override | 10 seconds | Reduced timeout for `Urgency::Urgent` |

---

## 5. Invariants

The following invariants hold across all RALPH cycle executions and are verified by the test suite.

| ID | Invariant | Enforcement |
|----|-----------|-------------|
| INV-R1 | Generation counter is monotonically increasing | `AtomicU64::fetch_add(1, SeqCst)` at end of Phase 5 |
| INV-R2 | At most `max_concurrent_mutations` active at any time | Pre-apply check in Phase 5 Step 1 |
| INV-R3 | No mutation changes a parameter by more than `max_mutation_delta` | Generation-time clamp in Phase 4 Step 2 |
| INV-R4 | Rolled-back mutations restore exact original values | `old` field in `MutationType` variants |
| INV-R5 | PBFT consensus is required for all mutations with `expected_delta >= auto_apply_threshold` | Classification in Phase 4 Step 4 |
| INV-R6 | Minimum interval between generations is enforced (unless `Urgency::Urgent`) | Gating check before RALPH cycle entry |
| INV-R7 | All 5 phases execute in order within a single generation | Sequential execution in `run_ralph_cycle` |
| INV-R8 | Fitness history capacity is bounded | `VecDeque` with `pop_front` when at capacity |
| INV-R9 | Mutation history capacity is bounded | `VecDeque` with `pop_front` when at capacity |
| INV-R10 | Anti-pattern classification requires minimum 5 attempts | `ANTI_PATTERN_MIN_ATTEMPTS` constant |

---

## 6. Error Handling

| Error | Phase | Recovery | Escalation |
|-------|-------|----------|------------|
| `NoFitnessData` | 1 (Recognize) | Skip generation, retry after interval | L1 Notify if persists > 3 cycles |
| `HistoryCorrupted` | 2 (Analyze) | Reset baseline to 0.75, clear history | L2 Require Approval |
| `HebbianSignalFailed` | 3 (Learn) | Log warning, continue without pathway feedback | L1 Notify |
| `MutationGenerationFailed` | 4 (Plan) | Skip failed root cause, continue with others | L0 Auto |
| `ApplyFailed` | 5 (Harmonize) | Mark mutation as failed, do not add to active | L1 Notify |
| `RollbackFailed` | 5 (Harmonize) | CRITICAL: Manual intervention required | L3 PBFT + Human @0.A override |
| `ConsensusTimeout` | 4/5 (PBFT) | Discard mutation, log timeout | L1 Notify |
| `EventBusPublishFailed` | 5 (Harmonize) | Buffer report, retry on next cycle | L0 Auto |

---

## 7. Data Flow Diagram

```
                  M37 Correlation Engine
                          |
                  CorrelatedEvents
                          |
                          v
                  M38 Emergence Detector
                          |
                  EmergenceRecord[]
                          |
     FitnessEvaluator     |
          |               |
     FitnessReport        |
          |               |
          +-------+-------+
                  |
                  v
    +-----------------------------------+
    |      M39 Evolution Chamber        |
    |                                   |
    |  Phase 1: RECOGNIZE               |
    |  SystemState + Urgency            |
    |         |                         |
    |  Phase 2: ANALYZE                 |
    |  AnalysisResult (root causes)     |
    |         |                         |
    |  Phase 3: LEARN                   |
    |  LearningUpdate       +---------+ |
    |  |                    | M25     | |
    |  | LTP/LTD signals -->| Hebbian | |
    |  |                    +---------+ |
    |  Phase 4: PLAN                    |
    |  Vec<MutationRecord>              |
    |  |                                |
    |  | (high-impact)    +-----------+ |
    |  +----------------->| M31 PBFT  | |
    |  | (approved)  <----+-----------+ |
    |  |                                |
    |  Phase 5: HARMONIZE               |
    |  |                                |
    |  | (apply)    +----------------+  |
    |  +----------->| M02 Config     |  |
    |  +----------->| M11 Balancer   |  |
    |  +----------->| M12 Circuit    |  |
    |  +----------->| M25 Hebbian    |  |
    |  +----------->| M26 STDP       |  |
    |  +----------->| M15 Confidence |  |
    |  |            +----------------+  |
    |  |                                |
    |  | EvolutionReport                |
    |  +----------->| M23 EventBus   |  |
    |               +----------------+  |
    +-----------------------------------+
                  |
           generation++
                  |
                  v
          [Wait interval]
                  |
          [Next generation]
```

---

## 8. Performance Characteristics

### 8.1 Per-Phase Latency Budget

| Phase | Target | Maximum | Bottleneck |
|-------|--------|---------|-----------|
| 1. Recognize | < 1ms | 10ms | Fitness classification (pure computation) |
| 2. Analyze | < 5ms | 50ms | History lookup (read lock) |
| 3. Learn | < 10ms | 100ms | Pathway signal emission (async I/O) |
| 4. Plan | < 10ms | 100ms | Mutation generation + filtering |
| 5. Harmonize | < 50ms | 500ms | Parameter application (cross-module calls) |
| **Total cycle** | **< 76ms** | **< 760ms** | - |

### 8.2 Memory Budget

| Component | Size | Bound |
|-----------|------|-------|
| `fitness_history` | ~120 bytes per snapshot | 500 entries = ~60 KB |
| `mutation_history` | ~512 bytes per record | 1,000 entries = ~500 KB |
| `active_mutations` | ~256 bytes per entry | 3 entries = ~768 bytes |
| `effectiveness_model` | ~128 bytes per type | 7 types = ~896 bytes |
| **Total steady-state** | - | **< 600 KB** |

### 8.3 Throughput

| Metric | Value | Condition |
|--------|-------|-----------|
| Generations per minute (Critical) | 2 | 30s interval |
| Generations per minute (Degraded) | 1 | 60s interval |
| Generations per minute (Stable) | 0.5 | 120s interval |
| Generations per minute (Thriving) | 0.2 | 300s interval |
| Mutations per generation (max) | 3 | `max_concurrent_mutations` |
| PBFT votes per high-impact mutation | 40 | Full agent fleet |

---

## 9. Testing Requirements

### 9.1 Unit Tests

| Test Group | Count | Description |
|------------|-------|-------------|
| Phase 1: Recognize | 6 | All SystemState classifications; urgency matrix; state context building |
| Phase 2: Analyze | 8 | Baseline computation; underperformer detection; emergence correlation; root cause ranking; empty history fallback |
| Phase 3: Learn | 6 | Effectiveness model update; pathway signal emission; anti-pattern detection; generation probability reduction |
| Phase 4: Plan | 8 | Candidate generation; safety filtering; scoring with urgency; consensus classification; capacity limiting |
| Phase 5: Harmonize | 8 | Mutation application (all 7 types); verification commit; verification rollback; verification extend; homeostatic adjustment |
| Generation timing | 4 | State-dependent intervals; urgency override; minimum interval enforcement; monotonic generation counter |

### 9.2 Integration Tests

| Test | Description |
|------|-------------|
| Full RALPH cycle (Thriving) | End-to-end with fitness 0.95, no emergences, verify LOW urgency |
| Full RALPH cycle (Critical) | End-to-end with fitness 0.40, cascading failure, verify URGENT |
| PBFT integration | High-impact mutation through PBFT approval pipeline |
| Rollback integrity | Apply mutation, verify fitness drop, confirm exact rollback |
| Multi-generation learning | Run 10 generations, verify effectiveness model converges |
| Homeostatic target drift | Run 5 Thriving generations, verify targets increase by 0.05 |

### 9.3 Property Tests

| Property | Description |
|----------|-------------|
| Generation monotonicity | `forall g1, g2: g1 < g2 => generation(g1) < generation(g2)` |
| Rollback symmetry | `forall m: rollback(apply(m, state)) == state` |

---

## 10. Cross-Spec Dependencies

| Dependency | Spec | Phase | Direction |
|------------|------|-------|-----------|
| `EmergenceRecord` | [EMERGENCE_DETECTOR_SPEC](EMERGENCE_DETECTOR_SPEC.md) | 1, 2 | Input |
| `FitnessReport` | [FITNESS_FUNCTION_SPEC](FITNESS_FUNCTION_SPEC.md) | 1, 2, 5 | Input |
| `EvolutionChamber` types | [EVOLUTION_CHAMBER_SPEC](EVOLUTION_CHAMBER_SPEC.md) | All | Shared types |
| PBFT consensus | [PBFT_SPEC](../../PBFT_SPEC.md) | 4, 5 | High-impact mutations |
| Hebbian pathways | [STDP_SPEC](../../STDP_SPEC.md) | 3, 5 | LTP/LTD signals, PathwayAdjustment |
| 12D Tensor | [TENSOR_SPEC](../../TENSOR_SPEC.md) | 2 | Dimension weights and scoring |
| Escalation tiers | [ESCALATION_SPEC](../../ESCALATION_SPEC.md) | 5 | EscalationTierShift application |
| Event Bus | [PIPELINE_SPEC](../../PIPELINE_SPEC.md) | 5 | EvolutionReport publication |

---

## 11. Version History

| Version | Date | Changes |
|---------|------|---------|
| 1.0.0 | 2026-01-29 | Initial specification (RALPH v2.0 protocol) |

---

*Generated: 2026-01-29 | The Maintenance Engine v1.0.0 | L7 Observer Layer*
