# Module M39: Evolution Chamber

> **M39_EVOLUTION_CHAMBER** | RALPH 5-phase meta-learning loop | Layer: L7 Observer | [Back to Index](INDEX.md)

---

## Navigation

| Direction | Link |
|-----------|------|
| Up | [INDEX.md](INDEX.md) |
| Layer | [L07_OBSERVER.md](../layers/L07_OBSERVER.md) |
| Dependency | [M01_ERROR_TAXONOMY.md](M01_ERROR_TAXONOMY.md) |
| Related | [M37_LOG_CORRELATOR.md](M37_LOG_CORRELATOR.md) |
| Related | [M38_EMERGENCE_DETECTOR.md](M38_EMERGENCE_DETECTOR.md) |
| Learning | [M25_HEBBIAN_MANAGER.md](M25_HEBBIAN_MANAGER.md) |
| Learning | [M26_STDP_PROCESSOR.md](M26_STDP_PROCESSOR.md) |

---

## Module Specification

### Overview

The Evolution Chamber implements the RALPH (Recognize, Analyze, Learn, Propose, Harvest) 5-phase meta-learning loop for system parameter evolution. It manages the full mutation lifecycle -- proposal, application, verification, and acceptance or rollback -- driven by fitness evaluation from the Fitness Evaluator utility using 12D tensor dimensions.

The chamber operates as a generation-based evolutionary system: each generation consists of one RALPH cycle during which parameter mutations are proposed, bounded by configurable delta limits, applied to the live system, verified against fitness measurements, and either accepted (if fitness improves above threshold) or rolled back (if fitness regresses below threshold). Multiple mutations can run concurrently up to a configurable limit.

### Module Properties

| Property | Value |
|----------|-------|
| Module ID | M39 |
| Module Name | Evolution Chamber |
| Layer | L7 (Observer) |
| Source File | `src/m7_observer/evolution_chamber.rs` |
| LOC | 1,619 |
| Tests | 50 |
| Version | 1.0.0 |
| Lock Order | 4 (after EmergenceDetector) |
| Dependencies | M01 (Error), Fitness Evaluator (utility) |
| Dependents | L7 Coordinator |
| Thread Safety | `parking_lot::RwLock` on all mutable state |
| External Crates | `chrono`, `parking_lot`, `serde`, `uuid` |

---

## Architecture

```
                 Fitness Evaluator (12D Tensor)
                          |
                   fitness scores
                          |
                          v
     +--------------------+--------------------+
     |          EvolutionChamber (M39)          |
     |                                         |
     |   +-----------------------------------+ |
     |   |        RALPH Cycle Loop           | |
     |   |                                   | |
     |   |  [Recognize] --> [Analyze]        | |
     |   |       ^               |           | |
     |   |       |               v           | |
     |   |  [Harvest]  <-- [Propose]         | |
     |   |       ^               |           | |
     |   |       |               v           | |
     |   |       +-------- [Learn]           | |
     |   +-----------------------------------+ |
     |                                         |
     |   +-----------------------------------+ |
     |   | Active Mutations (in-flight)      | |
     |   | Proposed --> Verifying --> Accepted| |
     |   |                      \-> RolledBack| |
     |   +-----------------------------------+ |
     |                                         |
     |   +-----------------------------------+ |
     |   | Mutation History (ring buffer)    | |
     |   +-----------------------------------+ |
     |                                         |
     |   +-----------------------------------+ |
     |   | Fitness Snapshots (ring buffer)   | |
     |   +-----------------------------------+ |
     +--------------------------------------------+
```

---

## RALPH Phases

### RalphPhase (Enum)

| Phase | Ordinal | Purpose | Next Phase |
|-------|---------|---------|------------|
| `Recognize` | 0 | Identify parameters drifting from targets | Analyze |
| `Analyze` | 1 | Compute deltas and rank candidates | Learn |
| `Learn` | 2 | Extract patterns from mutation history | Propose |
| `Propose` | 3 | Generate bounded mutations | Harvest |
| `Harvest` | 4 | Accept beneficial mutations, rollback harmful ones | Recognize |

### RALPH Cycle Lifecycle

```
1. start_cycle()    --> Cycle starts, generation incremented, phase = Recognize
2. advance_phase()  --> Recognize -> Analyze -> Learn -> Propose -> Harvest
3. propose_mutation() --> Create mutations during Propose phase
4. apply_mutation()   --> Apply proposed mutation, status -> Verifying
5. verify_mutation()  --> Fitness measured, mutation -> Accepted (history)
   OR rollback_mutation() --> Fitness regressed, mutation -> RolledBack (history)
6. complete_cycle() --> Cycle ends, stats updated
```

---

## Mutation Lifecycle

### MutationStatus (Enum)

| Status | Display | Description |
|--------|---------|-------------|
| `Proposed` | `"Proposed"` | Created but not yet applied |
| `Verifying` | `"Verifying"` | Applied, awaiting fitness verification |
| `Accepted` | `"Accepted"` | Verified, fitness improved |
| `RolledBack` | `"RolledBack"` | Rolled back, fitness regressed |
| `Failed` | `"Failed"` | Failed during application or verification |

### State Transitions

```
Proposed --> Verifying --> Accepted   (moved to history)
                     \--> RolledBack  (moved to history)
                     \--> Failed      (moved to history)
```

---

## Core Data Structures

### MutationRecord

```rust
pub struct MutationRecord {
    pub id: String,                // UUID v4
    pub generation: u64,           // Generation when proposed
    pub source_phase: RalphPhase,  // RALPH phase that created it
    pub target_parameter: String,  // Parameter name being mutated
    pub original_value: f64,       // Value before mutation
    pub mutated_value: f64,        // Value after mutation
    pub delta: f64,                // mutated_value - original_value
    pub fitness_before: f64,       // Fitness before application
    pub fitness_after: f64,        // Fitness after verification (0.0 if unverified)
    pub applied: bool,             // Whether applied to live system
    pub rolled_back: bool,         // Whether subsequently rolled back
    pub timestamp: DateTime<Utc>,  // Record creation time
    pub verification_ms: u64,      // Verification latency (0 if unverified)
}
```

### ActiveMutation

```rust
pub struct ActiveMutation {
    pub id: String,                     // UUID v4
    pub generation: u64,                // Generation when created
    pub target_parameter: String,       // Parameter targeted
    pub original_value: f64,            // Original value
    pub applied_value: f64,             // Mutated value applied
    pub applied_at: DateTime<Utc>,      // Application timestamp
    pub verification_deadline: DateTime<Utc>, // Verification deadline
    pub status: MutationStatus,         // Current status
}
```

### RalphState

```rust
pub struct RalphState {
    pub current_phase: RalphPhase,               // Active phase
    pub cycle_number: u64,                        // Monotonically increasing
    pub cycle_started_at: Option<DateTime<Utc>>,  // None if not started
    pub cycle_completed_at: Option<DateTime<Utc>>, // None if still running
    pub mutations_proposed: u32,                   // Per-cycle counter
    pub mutations_applied: u32,                    // Per-cycle counter
    pub paused: bool,                              // Whether loop is paused
}
```

### FitnessSnapshot

```rust
pub struct FitnessSnapshot {
    pub timestamp: DateTime<Utc>,  // Snapshot time
    pub fitness: f64,              // Overall fitness score
    pub tensor: [f64; 12],         // Full 12D tensor state
    pub generation: Option<u64>,   // Generation number (if during active cycle)
}
```

### ChamberStats

```rust
pub struct ChamberStats {
    pub total_mutations_proposed: u64,     // Total proposed since creation
    pub total_mutations_applied: u64,      // Total applied
    pub total_mutations_rolled_back: u64,  // Total rolled back
    pub total_ralph_cycles: u64,           // Total completed RALPH cycles
    pub current_generation: u64,           // Current generation number
    pub current_phase: Option<RalphPhase>, // Current phase (None if no cycle)
}
```

---

## Public API

### Construction

| Method | Signature | Description |
|--------|-----------|-------------|
| `new()` | `-> Self` | Creates with default configuration |
| `with_config(config)` | `-> Self` | Creates with custom configuration |
| `validate_config(config)` | `-> Result<()>` | Validates all configuration parameters |

### Mutation Lifecycle

| Method | Signature | Description |
|--------|-----------|-------------|
| `propose_mutation(target, original, mutated, fitness_before)` | `-> Result<MutationRecord>` | Proposes a new mutation (does not apply) |
| `apply_mutation(mutation_id)` | `-> Result<()>` | Applies a proposed mutation (Proposed -> Verifying) |
| `verify_mutation(mutation_id, fitness_after)` | `-> Result<MutationRecord>` | Verifies and accepts a mutation |
| `rollback_mutation(mutation_id)` | `-> Result<MutationRecord>` | Rolls back a mutation to original value |

### RALPH Cycle Management

| Method | Signature | Description |
|--------|-----------|-------------|
| `start_cycle()` | `-> Result<u64>` | Starts a new RALPH cycle, returns cycle number |
| `advance_phase()` | `-> Result<RalphPhase>` | Advances to next phase, returns new phase |
| `complete_cycle()` | `-> Result<()>` | Completes the current RALPH cycle |
| `pause()` | `-> ()` | Pauses the RALPH loop |
| `resume()` | `-> ()` | Resumes the RALPH loop after pause |

### Fitness Tracking

| Method | Signature | Description |
|--------|-----------|-------------|
| `record_fitness(fitness, tensor)` | `-> FitnessSnapshot` | Records a fitness snapshot with 12D tensor |

### Query / Accessors

| Method | Signature | Description |
|--------|-----------|-------------|
| `generation()` | `-> u64` | Current generation number |
| `ralph_state()` | `-> RalphState` | Clone of current RALPH state |
| `active_mutation_count()` | `-> usize` | Number of in-flight mutations |
| `get_mutation(id)` | `-> Option<MutationRecord>` | Lookup mutation in history by ID |
| `recent_mutations(n)` | `-> Vec<MutationRecord>` | Most recent N mutations (newest last) |
| `fitness_history(n)` | `-> Vec<FitnessSnapshot>` | Most recent N fitness snapshots (newest last) |
| `stats()` | `-> ChamberStats` | Aggregate statistics snapshot |
| `should_auto_apply(fitness_delta)` | `-> bool` | Whether delta meets auto-apply threshold |
| `should_rollback(fitness_delta)` | `-> bool` | Whether delta meets rollback threshold |
| `config()` | `-> &EvolutionChamberConfig` | Immutable configuration reference |

### Maintenance

| Method | Signature | Description |
|--------|-----------|-------------|
| `clear()` | `-> ()` | Resets all state: mutations, history, snapshots, stats, generation, RALPH |

---

## Configuration

### EvolutionChamberConfig

| Parameter | Type | Default | Valid Range | Description |
|-----------|------|---------|-------------|-------------|
| `max_concurrent_mutations` | `u32` | 3 | > 0 | Max simultaneous in-flight mutations |
| `mutation_verification_ms` | `u64` | 30,000 | > 0 | Verification timeout per mutation (ms) |
| `fitness_history_capacity` | `usize` | 500 | > 0 | Max fitness snapshots retained |
| `mutation_history_capacity` | `usize` | 1,000 | > 0 | Max mutation records retained |
| `auto_apply_threshold` | `f64` | 0.10 | [0.0, 1.0] | Min fitness delta for auto-accept |
| `rollback_threshold` | `f64` | -0.02 | <= 0.0 | Max fitness delta for auto-rollback |
| `min_generation_interval_ms` | `u64` | 60,000 | > 0 | Min milliseconds between generations |
| `max_mutation_delta` | `f64` | 0.20 | (0.0, 1.0] | Max absolute delta per mutation |

### Decision Thresholds

```
If fitness_delta >= auto_apply_threshold (0.10):  AUTO-ACCEPT
If fitness_delta <= rollback_threshold  (-0.02):  AUTO-ROLLBACK
Otherwise:                                        MANUAL DECISION
```

---

## 12D Tensor Integration

The Evolution Chamber records fitness snapshots that include the full 12D tensor state:

| Dim | Name | Purpose in Evolution |
|-----|------|---------------------|
| D0 | service_id | Identify which service is being mutated |
| D1 | port | Network endpoint tracking |
| D2 | tier | Service tier for weighted evaluation |
| D3 | dependency_count | Mutation impact scope |
| D4 | agent_count | Agent participation tracking |
| D5 | protocol | Communication protocol context |
| D6 | health_score | Primary fitness signal |
| D7 | uptime | Availability tracking |
| D8 | synergy | Cross-service coupling (from M38) |
| D9 | latency | Performance signal |
| D10 | error_rate | Reliability signal |
| D11 | temporal_context | Time-decay relevance |

---

## Metrics

| Metric | Type | Source | Description |
|--------|------|--------|-------------|
| `total_mutations_proposed` | Counter | `ChamberStats` | Total mutations proposed |
| `total_mutations_applied` | Counter | `ChamberStats` | Total mutations applied |
| `total_mutations_rolled_back` | Counter | `ChamberStats` | Total mutations rolled back |
| `total_ralph_cycles` | Counter | `ChamberStats` | Total completed RALPH cycles |
| `current_generation` | Gauge | `ChamberStats` | Current generation number |
| `current_phase` | State | `ChamberStats` | Active RALPH phase (or None) |

---

## Error Codes

| Error Type | Condition | Raised By |
|------------|-----------|-----------|
| `Error::Validation` | `max_concurrent_mutations` is 0 | `validate_config` |
| `Error::Validation` | `mutation_verification_ms` is 0 | `validate_config` |
| `Error::Validation` | `fitness_history_capacity` is 0 | `validate_config` |
| `Error::Validation` | `mutation_history_capacity` is 0 | `validate_config` |
| `Error::Validation` | `auto_apply_threshold` not in [0.0, 1.0] | `validate_config` |
| `Error::Validation` | `rollback_threshold` > 0.0 | `validate_config` |
| `Error::Validation` | `max_mutation_delta` not in (0.0, 1.0] | `validate_config` |
| `Error::Validation` | `min_generation_interval_ms` is 0 | `validate_config` |
| `Error::Validation` | Target parameter name is empty | `propose_mutation` |
| `Error::Validation` | Delta exceeds `max_mutation_delta` | `propose_mutation` |
| `Error::Validation` | Concurrent mutation limit reached | `propose_mutation` |
| `Error::Validation` | Mutation ID not found in active mutations | `apply_mutation`, `verify_mutation`, `rollback_mutation` |
| `Error::Validation` | Mutation not in expected state (Proposed/Verifying) | `apply_mutation`, `verify_mutation` |
| `Error::Validation` | RALPH loop is paused | `advance_phase`, `start_cycle` |
| `Error::Validation` | No active cycle (start_cycle not called) | `advance_phase` |
| `Error::Validation` | Cycle already in progress | `start_cycle` |
| `Error::Validation` | No cycle to complete | `complete_cycle` |

---

## Related Modules

| Module | Relationship | Purpose |
|--------|--------------|---------|
| M01 (Error) | Dependency | Error taxonomy for Result types |
| Fitness Evaluator | Dependency | 12D tensor fitness scoring |
| M38 (Emergence Detector) | Upstream | Emergence records inform mutation targets |
| M37 (Log Correlator) | Upstream | Correlation data feeds emergence detection |
| M25-M26 (Learning) | Cross-layer | Hebbian/STDP learning from mutation outcomes |
| L7 Coordinator | Parent | Orchestrates RALPH cycles and fitness evaluation |

---

## Testing

Key test cases (50 total):

```rust
#[test] fn test_01_new_chamber_initial_state()        // Verify initial state
#[test] fn test_02_default_and_with_config()           // Config construction
#[test] fn test_03_validate_config_default_passes()    // Default config validation
#[test] fn test_04_validate_config_rejects_zero_concurrent() // Config boundary
#[test] fn test_07_validate_config_rejects_bad_thresholds()  // Threshold validation
#[test] fn test_propose_mutation()                     // Mutation proposal
#[test] fn test_apply_mutation()                       // Proposed -> Verifying
#[test] fn test_verify_mutation()                      // Verifying -> Accepted
#[test] fn test_rollback_mutation()                    // Verifying -> RolledBack
#[test] fn test_concurrent_mutation_limit()            // Enforces max concurrent
#[test] fn test_delta_exceeds_max()                    // Rejects oversized delta
#[test] fn test_start_cycle()                          // RALPH cycle start
#[test] fn test_advance_phase()                        // Phase advancement
#[test] fn test_complete_cycle()                       // Cycle completion
#[test] fn test_pause_resume()                         // Pause/resume semantics
#[test] fn test_record_fitness()                       // Fitness snapshot recording
#[test] fn test_fitness_history_bounded()              // Ring buffer bounds
#[test] fn test_should_auto_apply()                    // Threshold check
#[test] fn test_should_rollback()                      // Rollback threshold check
#[test] fn test_clear()                                // Full state reset
```

---

## Performance Characteristics

| Operation | Time | Notes |
|-----------|------|-------|
| Mutation proposal | <1ms | UUID generation + vec push |
| Mutation apply/verify/rollback | <1ms | Vec scan + removal |
| Fitness snapshot recording | <1ms | Ring buffer push |
| RALPH phase advance | <1ms | Lock + enum match |
| Cycle start/complete | <1ms | Lock + counter update |
| Stats query | <1ms | Lock + clone |
| History query (recent N) | <1ms | Slice + clone |

---

## Version History

| Version | Date | Changes |
|---------|------|---------|
| 1.0.0 | 2026-01-29 | Initial implementation (L7 Observer Layer, RALPH loop) |

---

[INDEX.md](INDEX.md) | [L07_OBSERVER.md](../layers/L07_OBSERVER.md) | [M37_LOG_CORRELATOR.md](M37_LOG_CORRELATOR.md) | [M38_EMERGENCE_DETECTOR.md](M38_EMERGENCE_DETECTOR.md)

*The Maintenance Engine v1.0.0 | M39: Evolution Chamber (RALPH)*
*Last Updated: 2026-01-29*
