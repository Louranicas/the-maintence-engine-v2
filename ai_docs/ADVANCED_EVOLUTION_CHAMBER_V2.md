# Advanced Evolution Chamber V2 — Design from Live Data Learnings

> **Source data:** ORAC gen 13,860 (26h, fitness 0.76) vs ME gen 31 (17h, fitness 0.61)
> **Learning window:** Sessions 050-068, ~200 hours of autonomous evolution
> **Purpose:** Redesign M39 Evolution Chamber for V2 using empirical evidence

---

## 1. Learnings from V1 (ME) Evolution Chamber

### What the Data Shows

| Metric | Value | Implication |
|--------|-------|-------------|
| 31 generations in 17h | 1.8 gen/h | **STALLED** — generation interval too long |
| 236 RALPH cycles for 31 gens | 7.6 cycles/gen | Most cycles produce no mutation |
| 6 mutations applied, 1 rollback | 17% rollback rate | Mutations are low-quality |
| Fitness range 0.44-0.71 | Oscillating, not climbing | No directional pressure |
| 9,688 fitness records, all gen=31 | Clock stopped | Generation counter stuck |
| 1,000 emergences (capped) | Only 2 types: AttractorFormation + CascadeFailure | Narrow detection |
| 193,714 correlations, 0 types | Correlations exist but untyped | Cannot learn from them |
| Mutation log: all fields NULL | Parameters not recorded | **CRITICAL** — blind evolution |
| Cognitive state: 1 row, never changes | No checkpointing | Restarts from zero |
| STDP decay 0.001 | HRS-001 fix never propagated | Pathways never decay properly |
| L2=0.33, L5=0.31 | Structural deficits dominate | Evolution can't fix structural problems |

### Root Causes of ME Stagnation

**Problem 1: Blind Mutations**
The mutation log shows every field as NULL. The evolution chamber proposes mutations but doesn't record WHAT was mutated, the old/new values, or the fitness delta. Without this feedback, the Learn phase has zero data to learn from. The chamber is randomly poking parameters with no memory.

**Problem 2: Generation Clock Stuck at 31**
236 RALPH cycles but only 31 generations advanced. This means 87% of cycles end in "no mutation proposed" — the proposal threshold is too conservative or the fitness signal is too noisy to trigger proposals.

**Problem 3: No Hint-Guided Selection**
ME's evolution chamber uses plain round-robin parameter selection. There's no equivalent of ORAC's 3-source Learn phase (emergence → dimension → pathway). Mutations have no causal relationship to observed problems.

**Problem 4: Structural Bottleneck Invisible to Evolution**
L2 health=0.33 because of the health scoring bug, L5=0.31 because of STDP decay misconfiguration. These are CODE BUGS, not tunable parameters. The evolution chamber cycles endlessly trying to find parameter combinations that compensate for code defects — it can never succeed.

**Problem 5: No Rollback Memory**
Only 1 cognitive state row, never rotated. When ME restarts, RALPH starts from gen 0 with no memory of prior evolution. 31 generations of learning are discarded on every restart.

---

## 2. Learnings from ORAC Evolution Chamber

### What Works (13,860 generations, fitness 0.76)

| Feature | Impact | Evidence |
|---------|--------|----------|
| **30s tick interval** (vs ME's 60s) | 2x faster iteration | 533 gen/h vs 1.8 gen/h |
| **Hint-guided Learn phase** | Mutations target actual problems | 3 sources: emergence→dimension→pathway |
| **BUG-035 diversity gate** | No mono-parameter monopoly | Round-robin + 10-gen cooldown + 50% diversity |
| **Snapshot/rollback** | Safe experimentation | Ring buffer of 10 snapshots, revert on >10% drop |
| **Multi-parameter mutation** | Escape local optima | 2-5 params per proposal, tournament selection |
| **Autonomous 20h+ runs** | Continuous improvement | Gen 1,711→5,678 in session 062 (unattended) |
| **Convergence detection** | Avoids wasted cycles | Pauses when variance < 0.001 over 50 gens |
| **1,362 LTP / 0 LTD** | Idle gating works | G1-G3 gates prevent spurious LTD |

### What ORAC Gets Right That ME Gets Wrong

1. **Mutation recording** — ORAC logs parameter name, old/new value, fitness delta, accepted/rejected
2. **Feedback loops wired** — correlation→mutation, emergence→strategy, dimension→parameter (Session 065)
3. **Tick rate** — 30s not 60s, doubling iteration speed
4. **Multiple emergence types** — 8 types vs ME's 2 (AttractorFormation + CascadeFailure)
5. **Coupling weight persistence** — ORAC saves/restores coupling weights (Session 064 fix)
6. **Circuit breaker gating** — mutations don't apply when bridges are circuit-open

### What ORAC Still Lacks

1. **No field-coherence gating** — mutations apply regardless of Kuramoto r state
2. **No pre-deployment testing** — mutations go live immediately, rollback is reactive
3. **No multi-strategy evolution** — single fitness function, no population diversity
4. **No cross-service evolution** — ORAC evolves its own parameters but can't evolve ME or PV2
5. **No morphogenic triggers** — no adaptation on |r_delta| > threshold

---

## 3. V2 Advanced Evolution Chamber Design

### Architecture: 3-Tier Evolution

```
TIER 3: META-EVOLUTION (new)
  ├── Strategy selection: which Tier 1 strategy to use
  ├── Horizon adaptation: short-term vs long-term fitness tradeoffs
  └── Cross-service coordination: evolve ME+ORAC+PV2 jointly

TIER 2: GATED EVOLUTION (new — N05 Evolution Gate)
  ├── Pre-deployment field coherence test
  ├── Shadow execution in isolation (K=1.0, 500 steps, 5 spheres)
  ├── Accept only if r_after >= r_baseline
  └── Morphogenic adaptation triggers (N06)

TIER 1: CORE RALPH (improved from V1)
  ├── Recognize → Analyze → Learn → Propose → Harvest
  ├── Hint-guided mutations (3-source Learn phase from ORAC)
  ├── Multi-parameter proposals (2-5 params, diversity gate)
  ├── Snapshot/rollback ring buffer
  └── Convergence detection + auto-pause
```

### V2 Evolution Chamber Module Spec

**File:** `src/m7_observer/evolution_chamber.rs` (M39, enhanced)
**Est LOC:** ~2,500 (up from 1,619 — +881 for new features)
**Est tests:** 80 (up from 50)

### New Traits

```rust
/// Advanced evolution with field-awareness and gating
pub trait AdvancedEvolution: Send + Sync + fmt::Debug {
    // ── Core RALPH (inherited) ──
    fn tick(&self, tensor: &Tensor12D, tick: u64) -> Result<EvolutionReport>;
    fn propose_mutation(&self, target: &str, old: f64, new: f64, confidence: f64) -> Result<String>;
    fn apply_mutation(&self, mutation_id: &str) -> Result<()>;
    fn rollback_mutation(&self, mutation_id: &str) -> Result<MutationRecord>;

    // ── V2 Enhancements ──

    /// Hint-guided Learn phase: emergence → dimension → pathway → structural
    fn learn_with_hints(
        &self,
        emergence: &[EmergenceRecord],
        dimension_analysis: Option<&DimensionAnalysis>,
        pathways: &[EstablishedPathway],
    ) -> Option<MutationHint>;

    /// Pre-apply field coherence gate (N05 integration point)
    fn gate_mutation(&self, mutation_id: &str, r_before: f64) -> Result<GateDecision>;

    /// Record mutation outcome with full parameter tracking
    fn record_outcome(
        &self,
        mutation_id: &str,
        fitness_before: f64,
        fitness_after: f64,
        r_before: f64,
        r_after: f64,
    ) -> Result<()>;

    /// Multi-strategy evolution: select strategy based on system state
    fn select_strategy(&self, system_state: &SystemState) -> EvolutionStrategy;

    /// Cross-service mutation: propose parameter change for another service
    fn propose_cross_service(
        &self,
        target_service: &str,
        parameter: &str,
        delta: f64,
    ) -> Result<String>;

    /// Cognitive state persistence (survives restarts)
    fn save_cognitive_state(&self) -> Result<CognitiveCheckpoint>;
    fn restore_cognitive_state(&self, checkpoint: &CognitiveCheckpoint) -> Result<()>;

    /// Morphogenic adaptation trigger
    fn check_morphogenic_trigger(&self, r_delta: f64) -> Option<AdaptationAction>;
}
```

### New Types

```rust
/// Hint from Learn phase guiding mutation selection
pub struct MutationHint {
    pub parameter: String,
    pub source: HintSource,
    pub confidence: f64,
    pub reason: String,
}

pub enum HintSource {
    Emergence(EmergenceType),      // Source 1: urgent system event
    DimensionAnalysis(String),      // Source 2: weakest fitness dimension
    EstablishedPathway(String),     // Source 3: historical correlation
    StructuralDeficit(String),      // Source 4 (NEW): L2/L5 health scoring
}

/// Gate decision from N05 Evolution Gate
pub enum GateDecision {
    Accept { r_after: f64, confidence: f64 },
    Reject { reason: String, r_delta: f64 },
    DeferToConsensus { proposal_id: String },  // Escalate to PBFT
}

/// Evolution strategy selection
pub enum EvolutionStrategy {
    /// Normal: single parameter, conservative delta
    Conservative,
    /// Explore: multi-parameter, larger delta, higher rollback tolerance
    Exploratory,
    /// Repair: target known deficits (L2/L5 structural issues)
    StructuralRepair,
    /// Converge: narrow search around current optimum
    Convergence,
    /// Morphogenic: field-driven adaptation (|r_delta| > 0.05)
    Morphogenic,
}

/// Morphogenic adaptation action
pub enum AdaptationAction {
    /// Increase K (stronger coupling) when r is dropping
    IncreaseCoupling { k_delta: f64 },
    /// Decrease K when r is saturated (CoherenceLock)
    DecreaseCoupling { k_delta: f64 },
    /// Trigger STDP decay cycle when weights are saturated
    TriggerDecay,
    /// Spawn new sphere to introduce diversity
    SpawnDiversifier,
    /// No action needed
    None,
}

/// Full mutation record (fixes ME's NULL-field problem)
pub struct MutationRecord {
    pub id: String,
    pub parameter: String,           // NEVER null
    pub old_value: f64,              // NEVER null
    pub new_value: f64,              // NEVER null
    pub fitness_before: f64,         // NEVER null
    pub fitness_after: Option<f64>,  // None until verified
    pub r_before: f64,               // NEW: field state tracking
    pub r_after: Option<f64>,        // NEW
    pub strategy: EvolutionStrategy, // NEW: which strategy produced this
    pub hint: Option<MutationHint>,  // NEW: what guided this mutation
    pub status: MutationStatus,
    pub proposed_at: Timestamp,
    pub resolved_at: Option<Timestamp>,
}

/// Cognitive state that persists across restarts
pub struct CognitiveCheckpoint {
    pub generation: u64,
    pub fitness_history: Vec<f64>,        // Last 50 values
    pub mutation_success_rate: f64,       // Lifetime metric
    pub best_fitness: f64,
    pub best_parameters: HashMap<String, f64>,
    pub strategy_effectiveness: HashMap<String, f64>,  // NEW
    pub hint_accuracy: HashMap<String, f64>,            // NEW
    pub r_baseline: f64,                               // NEW
    pub total_mutations: u64,
    pub total_accepted: u64,
    pub total_rolled_back: u64,
    pub saved_at: Timestamp,
}
```

### Key Design Decisions

#### Decision 1: 4-Source Learn Phase (vs ORAC's 3)

ORAC's Learn phase queries 3 sources: emergence → dimension → pathway. V2 adds a 4th source:

**Source 4: Structural Deficit Detection**
The ME data shows that evolution spent 17 hours trying to compensate for L2=0.33 and L5=0.31 — problems caused by bugs, not parameters. V2's Learn phase checks layer health scores and, when a layer is below 0.5, emits a `StructuralDeficit` hint instead of a parameter hint. This triggers `EvolutionStrategy::StructuralRepair` which logs the deficit and skips mutation (no parameter can fix a bug).

```rust
// Source 4: Structural deficit check (NEW)
if hint.is_none() {
    let layer_scores = self.self_model.layer_health();
    for (layer, score) in layer_scores.iter().enumerate() {
        if *score < 0.5 {
            hint = Some(MutationHint {
                parameter: format!("L{}_structural", layer + 1),
                source: HintSource::StructuralDeficit(format!("L{} health={:.2}", layer + 1, score)),
                confidence: 0.99,
                reason: format!("Layer {} below 0.5 — bug, not tunable parameter", layer + 1),
            });
            break;
        }
    }
}
```

This prevents the "blind cycling" observed in ME's 236 cycles producing only 6 mutations.

#### Decision 2: Field-Coherence Gate (N05 Integration)

ME applies mutations immediately. ORAC applies immediately then rollbacks reactively. V2 **gates mutations through N05** before applying:

```
1. Capture r_before (Kuramoto field coherence)
2. Apply mutation in shadow mode (in-memory only)
3. Run Evolution Chamber simulation (K=1.0, 500 steps, 5 test spheres)
4. Measure r_after
5. IF r_after >= r_baseline: commit mutation to live state
6. ELSE: discard mutation, record as gate-rejected
```

This is the key architectural difference between V1 (apply-then-rollback) and V2 (test-then-apply).

#### Decision 3: Strategy Selection Based on System State

ME uses one strategy. ORAC uses hint-guided but still one strategy. V2 selects from 5 strategies:

```rust
fn select_strategy(&self, state: &SystemState) -> EvolutionStrategy {
    match state {
        // Fitness declining or below 0.5: repair mode
        _ if state.fitness < 0.5 || state.trend == FitnessTrend::Declining =>
            EvolutionStrategy::StructuralRepair,

        // Fitness stagnant for 50+ gens: explore
        _ if state.stagnation_gens > 50 =>
            EvolutionStrategy::Exploratory,

        // Field coherence disrupted (|r_delta| > 0.05): morphogenic
        _ if state.r_delta.abs() > 0.05 =>
            EvolutionStrategy::Morphogenic,

        // Fitness variance < 0.001 for 50 gens: converging
        _ if state.variance < 0.001 && state.fitness > 0.8 =>
            EvolutionStrategy::Convergence,

        // Default: conservative single-parameter mutation
        _ => EvolutionStrategy::Conservative,
    }
}
```

Each strategy has different delta ranges, rollback thresholds, and parameter selection logic.

#### Decision 4: Mandatory Mutation Recording

The ME data shows NULL fields in every mutation log row. V2 enforces complete recording:

```rust
// V2: propose_mutation MUST record all fields
pub fn propose_mutation(&self, target: &str, old: f64, new: f64, confidence: f64) -> Result<String> {
    // Validation: refuse empty/null parameter names
    if target.is_empty() {
        return Err(Error::Validation("parameter name must not be empty".into()));
    }

    let record = MutationRecord {
        id: Uuid::new_v4().to_string(),
        parameter: target.to_owned(),     // NEVER null
        old_value: old,                    // NEVER null
        new_value: new,                    // NEVER null
        fitness_before: self.current_fitness(), // NEVER null
        fitness_after: None,               // Set on verify
        r_before: self.current_r(),        // NEW: field state
        r_after: None,                     // Set on verify
        strategy: self.current_strategy(), // NEW: which strategy
        hint: self.last_hint.read().clone(), // NEW: what guided this
        status: MutationStatus::Proposed,
        proposed_at: Timestamp::now(),
        resolved_at: None,
    };
    // ... store and return ID
}
```

#### Decision 5: Cognitive State Persistence (M56 Integration)

ME has 1 cognitive state row that never rotates. V2 integrates with M56 Checkpoint Manager:

- Save checkpoint every 10 generations (not every tick)
- Ring buffer of 20 checkpoints
- On restart: restore from latest checkpoint, resume from last generation
- Track strategy effectiveness and hint accuracy across restarts

#### Decision 6: 15s Tick Interval (vs ME's 60s, ORAC's 30s)

ME's 60s tick means ~1.8 gen/h. ORAC's 30s tick gives ~533 gen/h. V2 uses 15s for the evolution tick (separate from observer tick):

- Observer tick: 60s (unchanged — health polling, tensor build)
- Evolution tick: 15s (new — RALPH cycle only)
- This decouples evolution speed from observation speed

### RALPH V2 Phase Enhancements

```
Phase 1: RECOGNIZE (enhanced)
  V1: Check if parameters drift from targets
  V2: + Check layer health scores for structural deficits
      + Check r_delta for morphogenic triggers
      + Check stagnation counter for strategy switch

Phase 2: ANALYZE (enhanced)
  V1: Compute deltas and rank candidates
  V2: + Compute r_delta trend (3-point moving average)
      + Classify system state → select strategy
      + Check N05 gate availability

Phase 3: LEARN (enhanced — 4 sources)
  V1: No learning (ME), 3-source hints (ORAC)
  V2: 4-source: emergence → dimension → pathway → structural deficit
      + Track hint accuracy over time (which sources produce accepted mutations)
      + Adaptive source weighting (sources that produce rollbacks get lower priority)

Phase 4: PROPOSE (enhanced)
  V1: Round-robin parameter selection
  V2: Hint-guided with diversity gate + strategy-specific delta ranges
      + Conservative: delta ±5%, 1 param
      + Exploratory: delta ±20%, 2-5 params, tournament selection
      + StructuralRepair: skip mutation, log deficit, advance gen
      + Convergence: delta ±1%, 1 param, narrowing search
      + Morphogenic: delta computed from r_delta magnitude

Phase 5: HARVEST (enhanced)
  V1: Accept/rollback based on fitness delta
  V2: + N05 field coherence gate before commit
      + Record full mutation details (parameter, values, r_before/after)
      + Update strategy effectiveness tracking
      + Update hint accuracy tracking
      + Persist cognitive state every 10 gens (M56)
```

### Database Schema (evolution_tracking.db additions)

```sql
-- V2: Enhanced mutation log (fixes NULL-field problem)
CREATE TABLE IF NOT EXISTS mutation_log_v2 (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    mutation_id TEXT NOT NULL UNIQUE,
    parameter TEXT NOT NULL,           -- NEVER null
    old_value REAL NOT NULL,           -- NEVER null
    new_value REAL NOT NULL,           -- NEVER null
    fitness_before REAL NOT NULL,      -- NEVER null
    fitness_after REAL,                -- NULL until verified
    r_before REAL NOT NULL,            -- NEW
    r_after REAL,                      -- NEW, NULL until verified
    strategy TEXT NOT NULL,            -- NEW: Conservative/Exploratory/etc
    hint_source TEXT,                  -- NEW: Emergence/Dimension/Pathway/Structural
    hint_parameter TEXT,               -- NEW: what the hint suggested
    status TEXT NOT NULL,              -- Proposed/Verifying/Accepted/RolledBack/GateRejected
    gate_decision TEXT,                -- NEW: Accept/Reject/DeferToConsensus
    proposed_at TEXT NOT NULL,
    resolved_at TEXT,
    generation INTEGER NOT NULL
);

-- V2: Strategy effectiveness tracking
CREATE TABLE IF NOT EXISTS strategy_effectiveness (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    strategy TEXT NOT NULL,
    total_proposed INTEGER NOT NULL DEFAULT 0,
    total_accepted INTEGER NOT NULL DEFAULT 0,
    total_rolled_back INTEGER NOT NULL DEFAULT 0,
    total_gate_rejected INTEGER NOT NULL DEFAULT 0,
    avg_fitness_delta REAL NOT NULL DEFAULT 0.0,
    last_used_at TEXT NOT NULL,
    UNIQUE(strategy)
);

-- V2: Hint source accuracy tracking
CREATE TABLE IF NOT EXISTS hint_accuracy (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    source TEXT NOT NULL,              -- Emergence/Dimension/Pathway/Structural
    parameter TEXT NOT NULL,
    times_suggested INTEGER NOT NULL DEFAULT 0,
    times_accepted INTEGER NOT NULL DEFAULT 0,
    times_rolled_back INTEGER NOT NULL DEFAULT 0,
    avg_fitness_improvement REAL NOT NULL DEFAULT 0.0,
    UNIQUE(source, parameter)
);

-- Indexes
CREATE INDEX IF NOT EXISTS idx_mutation_v2_status ON mutation_log_v2(status);
CREATE INDEX IF NOT EXISTS idx_mutation_v2_strategy ON mutation_log_v2(strategy);
CREATE INDEX IF NOT EXISTS idx_mutation_v2_gen ON mutation_log_v2(generation);
```

---

## 4. Implementation Plan

### Files to Modify

| File | Change | Est LOC |
|------|--------|---------|
| `m7_observer/evolution_chamber.rs` | Enhance M39 with V2 features | +881 (1,619→2,500) |
| `m7_observer/mod.rs` | Wire new evolution types into coordinator | +50 |
| `nexus/evolution_gate.rs` | Implement N05 from stub | +800 |
| `nexus/morphogenic_adapter.rs` | Implement N06 from stub | +600 |
| `m7_observer/emergence_detector.rs` | Add 6 missing emergence types | +200 |
| `migrations/` | Add mutation_log_v2 + strategy + hint tables | +40 |

### Dependency Order

```
1. Enhanced M39 (core RALPH V2 — no N05/N06 dependency)
2. N05 Evolution Gate (depends on M39 for mutation API)
3. N06 Morphogenic Adapter (depends on N01 + N03 for field/regime state)
4. Wire N05→M39→N06 feedback loop in engine.rs
```

### Test Plan (80 tests for enhanced M39)

- **Hint-guided Learn**: 15 tests (4 sources × 3 scenarios + edge cases)
- **Strategy selection**: 10 tests (5 strategies × 2 state scenarios)
- **Field coherence gate**: 10 tests (accept/reject/defer paths)
- **Mutation recording**: 10 tests (no NULL fields, complete lifecycle)
- **Cognitive state persistence**: 10 tests (save/restore/restart recovery)
- **Morphogenic triggers**: 10 tests (threshold crossing, action selection)
- **Strategy effectiveness tracking**: 8 tests (accumulation, accuracy computation)
- **Convergence detection**: 7 tests (stagnation, variance, auto-pause)

---

## 5. Expected Impact

| Metric | ME V1 (current) | V2 (projected) |
|--------|-----------------|-----------------|
| Generation rate | 1.8 gen/h | 240+ gen/h (15s tick) |
| Mutation acceptance rate | 83% (5/6) | 60-70% (gated, more experimental) |
| Fitness ceiling | 0.61-0.71 (oscillating) | 0.85+ (structural deficits identified, not masked) |
| Blind mutations | 100% (all NULL) | 0% (mandatory recording) |
| Restart recovery | Gen 0 (no memory) | Resume from checkpoint |
| Strategy diversity | 1 (conservative) | 5 (state-adaptive) |
| Field awareness | None | Full (N05 gate + N06 morphogenic) |
| Cross-service evolution | None | Via N04 STDP Bridge |

---

## 6. Cross-References

| Resource | Path |
|----------|------|
| ME V1 Evolution Chamber | `the_maintenance_engine/src/m7_observer/evolution_chamber.rs` |
| ORAC RALPH Engine | `orac-sidecar/src/m8_evolution/m36_ralph_engine.rs` |
| ORAC Mutation Selector | `orac-sidecar/src/m8_evolution/m40_mutation_selector.rs` |
| ORAC Emergence Detector | `orac-sidecar/src/m8_evolution/m37_emergence_detector.rs` |
| ORAC Fitness Tensor | `orac-sidecar/src/m8_evolution/m39_fitness_tensor.rs` |
| N05 Evolution Gate Spec | `ai_specs/nexus-specs/N05_EVOLUTION_GATE.md` |
| N06 Morphogenic Adapter Spec | `ai_specs/nexus-specs/N06_MORPHOGENIC_ADAPTER.md` |
| Session 065 Feedback Loops | `~/projects/shared-context/Session 065 — Evolution Chamber Feedback Loop Wiring.md` |
| BUG-035 Fix (diversity gate) | `~/projects/claude_code/ORAC — RALPH Multi-Parameter Mutation Fix.md` |
| Evolution Tracking DB | `data/databases/evolution_tracking.db` |

---

*Advanced Evolution Chamber V2 Design | From 200h+ Live Data | Session 068*
