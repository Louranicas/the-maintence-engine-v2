# L7: Observer Layer Specification

> Target: ~8,500 LOC | 5 modules (M37-M39 + infrastructure) | 350+ tests

---

## Layer Purpose

The Observer layer provides system-wide observability through cross-layer log correlation, emergence detection, and evolution testing. It aggregates data from all lower layers into a unified fitness evaluation using the 12D tensor encoding, and runs the RALPH (Randomize And Learn Through Permutation Hunting) evolution loop.

---

## Module Specifications

### Observer Bus (`observer_bus.rs`)

**Purpose:** Internal pub/sub event bus for observer-specific events, separate from L4's general EventBus.

**Target:** ~1,000 LOC, 50+ tests

**Key Types:**
```rust
pub struct ObserverBus { inner: RwLock<ObserverBusInner> }

pub enum ObserverEvent {
    FitnessUpdated(FitnessSnapshot),
    EmergenceDetected(EmergenceEvent),
    EvolutionCompleted(EvolutionResult),
    CorrelationFound(Correlation),
    AnomalyDetected(Anomaly),
}
```

**Bounded channels:** Default capacity 256. Overflow policy: drop oldest.

---

### Fitness Evaluator (`fitness.rs`)

**Purpose:** Compute 12D tensor fitness scores from all layer contributions.

**Target:** ~1,000 LOC, 50+ tests

**Key Types:**
```rust
pub struct FitnessEvaluator { inner: RwLock<FitnessInner> }

pub struct FitnessSnapshot {
    tensor: [f64; 12],
    overall_score: f64,
    trend: FitnessTrend,
    timestamp: Timestamp,
}

pub enum FitnessTrend { Improving, Stable, Degrading, Critical }
```

**Computation:**
```rust
overall = weights.iter().zip(tensor.iter())
    .fold(0.0, |acc, (w, v)| w.mul_add(*v, acc))
    / weights.iter().sum::<f64>()
```

**Database:** Writes snapshots to `tensor_memory.db`

---

### M37: Log Correlator (`log_correlator.rs`)

**Purpose:** Correlate logs across layers to detect causal chains and periodic patterns.

**Target:** ~1,300 LOC, 50+ tests

**Key Traits:**
```rust
pub trait LogCorrelation: Send + Sync {
    fn ingest(&self, entry: LogEntry) -> Result<()>;
    fn correlate(&self, window: Duration) -> Result<Vec<Correlation>>;
    fn periodic_patterns(&self) -> Result<Vec<PeriodicPattern>>;
    fn causal_chain(&self, event_id: &EventId) -> Result<Vec<CausalLink>>;
}
```

**Detection Methods:**
- Temporal proximity (events within configurable window)
- Service dependency chain following
- Error propagation tracing
- Periodic recurrence (FFT-based frequency detection)

---

### M38: Emergence Detector (`emergence_detector.rs`)

**Purpose:** Detect emergent behaviors in the system.

**Target:** ~1,800 LOC, 50+ tests

**Emergence Types:**
```rust
pub enum EmergenceType {
    Cascade,      // Multi-service failure/recovery propagation
    Synergy,      // Unexpected positive interaction
    Resonance,    // Synchronized oscillation between services
    PhaseShift,   // Sudden system-wide state change
    Chimera,      // Mixed coherent/incoherent state (V2: Kuramoto)
}
```

**V2 Enhancement:** Chimera state detection via Kuramoto order parameter analysis. When r is intermediate (0.3 < r < 0.7), some oscillators are synchronized while others are not.

---

### M39: Evolution Chamber (`evolution_chamber.rs`)

**Purpose:** RALPH (Randomize And Learn Through Permutation Hunting) evolution loop for mutation testing.

**Target:** ~1,600 LOC, 50+ tests

**RALPH 5-Phase Loop:**
1. **Snapshot:** Capture current system state and fitness
2. **Mutate:** Apply random parameter perturbation
3. **Evaluate:** Measure fitness delta after mutation
4. **Select:** Accept if fitness improved, reject otherwise
5. **Archive:** Record result in evolution_tracking.db

**V2 Enhancement:** Evolution gate (N05) runs RALPH before deployments:
```
K=1.0, 500 steps, 5 spheres
Accept if r_after >= r_baseline
```

**Database:** `evolution_tracking.db` (19,803 existing fitness records)

---

## Layer Coordinator (`mod.rs`)

**Target:** ~1,300 LOC, 55+ tests

**Provides:**
- `ObserverLayer` aggregate struct with full lifecycle
- `observe()` — single observation cycle: collect → correlate → detect → evaluate → evolve
- `ObservationReport` — comprehensive system report
- Background tick task for continuous observation
- HTTP endpoint data for `/api/observer`, `/api/fitness`, `/api/emergence`, `/api/evolution`

---

## Design Constraints

- C1: Can import from all lower layers (L1-L6)
- C2: All trait methods `&self`
- C3: `TensorContributor` on FitnessEvaluator (all 12 dimensions)
- C4: Zero unsafe/unwrap/expect
- C11: Observer integrates with N01 field bridge for r-tracking
- V2: Chimera state detection for Kuramoto field analysis

---

## Test Strategy

- Unit tests: 50+ per module
- Integration: `tests/l7_observer_integration.rs`
- Benchmark: `benches/tensor_encoding.rs`
- Property: fitness always in [0.0, 1.0], emergence detection is idempotent, RALPH preserves best fitness
