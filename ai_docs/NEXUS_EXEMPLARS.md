# Nexus Layer (L8) — Reference Implementations from VMS + SVF

> Source code exemplars for N01-N06 modules
> Adapted from Vortex Memory System + Sphere Vortex Framework production code

---

## N01: Field Bridge — Kuramoto Order Parameter Tracking

### Reference: `sphere_vortex_framework/src/vortex/kuramoto.rs`

**Order Parameter Computation (r ∈ [0,1]):**
```rust
pub fn compute_order_parameter(&mut self) -> f64 {
    if self.states.is_empty() {
        self.order_parameter = 0.0;
        self.average_phase = 0.0;
        return 0.0;
    }
    let num = self.states.len() as f64;
    let (sum_cos, sum_sin): (f64, f64) = self.states.iter()
        .map(|s| (s.phi.cos(), s.phi.sin()))
        .fold((0.0, 0.0), |(ac, as_), (c, s)| (ac + c, as_ + s));
    let real = sum_cos / num;
    let imag = sum_sin / num;
    self.order_parameter = real.hypot(imag);  // r = |⟨e^(iφⱼ)⟩|
    self.average_phase = imag.atan2(real);    // ψ = arg(⟨e^(iφⱼ)⟩)
    self.order_parameter
}
```

**EMA-Smoothed Coherence Tracking:**

Reference: `vortex-memory-system/src/nexus/monitor/coherence_tracker.rs`

```rust
const EMA_ALPHA: f64 = 0.10;       // 10-cycle lookback
const HISTORY_CAPACITY: usize = 2000;

pub fn observe(&mut self, r: f64) {
    // EMA: r_ema = α·r + (1−α)·r_ema
    self.r_ema = EMA_ALPHA.mul_add(r - self.r_ema, self.r_ema);

    if r > self.peak_r { self.peak_r = r; }
    if r < self.trough_r { self.trough_r = r; }
    self.cycle += 1;

    self.history.push_back(RObservation { r, r_ema: self.r_ema, cycle: self.cycle });
    if self.history.len() > HISTORY_CAPACITY {
        self.history.pop_front();  // Ring buffer eviction
    }
}
```

### ME v2 N01 Adaptation Notes

- Use `Timestamp` cycle counter, not SystemTime
- Implement as `FieldBridgeOps` trait with `&self` + RwLock
- Store history in bounded `VecDeque<RObservation>`
- Emit `HealthSignal` when r crosses regime thresholds
- Trend detection: slope > 0.02 → Rising, < -0.02 → Falling, else Stable

---

## N02: Intent Router — 12D Cosine Similarity Routing

### Reference: `vortex-memory-system/src/nexus/dev_env/intent_encoder.rs` + `intent_router.rs`

**12D Dimension Map (Sparse Orthogonal):**
```
D0:  syntactic         (parsing, code structure)
D1:  novelty           (new concepts, generation)
D2:  arousal           (system activation energy)
D3:  temporal_urgency  (time-criticality)
D4:  spatial_orient    (memory recall, location)
D5:  social_context    (coordination, multi-agent)
D6:  causal_depth      (reasoning depth)
D7:  action_relevance  (immediate executable action)
D8:  uncertainty       (confidence, anomaly)
D9:  spatial_depth     (geometric, vortex-related)
D10: operator_reson    (system health, alignment)
D11: abstraction       (theoretical depth)
```

**Cosine Similarity:**
```rust
fn cosine_similarity(a: &[f64; 12], b: &[f64; 12]) -> f64 {
    let dot: f64 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let mag_a: f64 = a.iter().map(|x| x * x).sum::<f64>().sqrt();
    let mag_b: f64 = b.iter().map(|x| x * x).sum::<f64>().sqrt();
    if mag_a == 0.0 || mag_b == 0.0 { return 0.0; }
    dot / (mag_a * mag_b)
}
```

**Service Tensor Signatures:**
```rust
// Each ULTRAPLATE service has a fixed 12D signature
"synthex"       => [0.9, 0.8, 0.0, 0.0, 0.0, 0.0, 0.0, 0.9, 0.0, 0.0, 0.0, 0.0]
"san-k7"        => [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.9, 0.0, 0.0, 0.0, 0.9, 0.8]
"maintenance"   => [0.0, 0.0, 0.0, 0.9, 0.0, 0.0, 0.0, 0.0, 0.8, 0.0, 0.7, 0.0]
"code-synthor"  => [0.9, 0.0, 0.0, 0.0, 0.0, 0.0, 0.8, 0.8, 0.0, 0.0, 0.0, 0.0]
"sphere-vortex" => [0.0, 0.0, 0.0, 0.0, 0.9, 0.0, 0.0, 0.0, 0.0, 0.9, 0.0, 0.8]
```

**Routing Logic:**
```rust
pub fn route(&self, intent: &EncodedIntent) -> RouteResult {
    let mut best_name = String::new();
    let mut best_score: f64 = -1.0;

    for (name, tensor) in &self.service_tensors {
        let score = cosine_similarity(&intent.tensor, tensor);
        if score > best_score {
            best_score = score;
            name.clone_into(&mut best_name);
        }
    }

    if best_score >= 0.3 {  // FALLBACK_THRESHOLD
        RouteResult { service_name: best_name, resonance: best_score, fallback: false }
    } else {
        // Fallback to highest-health service
        let fallback = self.registry.healthiest_service();
        RouteResult { service_name: fallback, resonance: best_score, fallback: true }
    }
}
```

### ME v2 N02 Adaptation Notes

- ME v2's 12D tensor uses DIFFERENT dimensions (service_id, port, tier...) from VMS intent
- Map ME's 12D to N02's routing: D6(health), D8(synergy), D9(latency) → routing weights
- Maintain service tensor signatures as `HashMap<String, [f64; 12]>` in RwLock
- Fallback threshold 0.3 — routes below this go to healthiest service

---

## N03: Regime Manager — K-Regime Detection

### Reference: `vortex-memory-system/src/nexus/swarm/coordinator.rs`

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KRegime {
    Swarm,   // K < 1.0 — independent parallel, exploratory
    Fleet,   // 1.0 ≤ K < 2.0 — coordinated, default mode
    Armada,  // K ≥ 2.0 — synchronized convergence, critical
}

impl KRegime {
    pub const fn k_value(&self) -> f64 {
        match self {
            Self::Swarm => 0.5,
            Self::Fleet => 1.5,
            Self::Armada => 3.0,
        }
    }

    pub fn from_k(k: f64) -> Self {
        if k < 1.0 { Self::Swarm }
        else if k < 2.0 { Self::Fleet }
        else { Self::Armada }
    }
}

pub struct SwarmCoordinator {
    regime: KRegime,
    sphere_ids: Vec<String>,
}

impl SwarmCoordinator {
    pub fn set_regime(&mut self, regime: KRegime) { self.regime = regime; }
    pub fn current_k(&self) -> f64 { self.regime.k_value() }
    pub fn register_sphere(&mut self, id: String) { self.sphere_ids.push(id); }
}
```

### ME v2 N03 Adaptation Notes

- Wrap in `RwLock<SwarmState>` for `&self` trait compat
- Regime transitions emit `HealthSignal` (K change affects system coupling)
- Track transition history for trend analysis
- Default to Fleet; Swarm for parallel module builds; Armada for consensus-critical

---

## N04: STDP Bridge — Tool Chain Learning

### Reference: `vortex-memory-system/src/nexus/learning/stdp.rs`

```rust
const FLOOR: f64 = 0.10;     // Min pathway strength
const CEILING: f64 = 0.95;   // Max pathway strength
const LTP_RATE: f64 = 0.05;  // Potentiation step
const LTD_RATE: f64 = 0.02;  // Depression step
const MAX_PATHWAYS: usize = 500;

pub struct StdpPathway {
    pub from: String,
    pub to: String,
    pub strength: f64,
    pub potentiation_count: u64,
    pub depression_count: u64,
}

pub struct StdpKernel {
    pathways: HashMap<(String, String), StdpPathway>,
}

impl StdpKernel {
    pub fn potentiate(&mut self, from: &str, to: &str) {
        let key = (from.to_owned(), to.to_owned());
        let pathway = self.pathways.entry(key).or_insert_with(|| StdpPathway {
            from: from.to_owned(), to: to.to_owned(),
            strength: FLOOR, potentiation_count: 0, depression_count: 0,
        });
        pathway.strength = (pathway.strength + LTP_RATE).min(CEILING);
        pathway.potentiation_count += 1;

        // Anti-Hebbian: auto-depress reverse direction
        self.depress(to, from);
        self.evict_if_over_capacity();
    }

    pub fn depress(&mut self, from: &str, to: &str) {
        let key = (from.to_owned(), to.to_owned());
        let pathway = self.pathways.entry(key).or_insert_with(|| StdpPathway {
            from: from.to_owned(), to: to.to_owned(),
            strength: FLOOR, potentiation_count: 0, depression_count: 0,
        });
        pathway.strength = (pathway.strength - LTD_RATE).max(FLOOR);
        pathway.depression_count += 1;
    }

    pub fn decay_all(&mut self, factor: f64) {
        for pathway in self.pathways.values_mut() {
            pathway.strength = (pathway.strength * factor).clamp(FLOOR, CEILING);
        }
    }
}
```

### ME v2 N04 Adaptation Notes

- ME v2 uses CO_ACTIVATION_DELTA = 0.05 (matches VMS LTP_RATE)
- Wrap `StdpKernel` in RwLock for `&self` StdpBridgeOps trait
- Record to `hebbian_pulse.db` and `system_synergy.db` (existing schemas)
- Decay via M41 Decay Auditor at rate 0.1 (different from VMS)
- Geometric grounding optional for ME v2 (no sphere surface)

---

## N05: Evolution Gate — RALPH Mutation Testing

### Reference: `vortex-memory-system/src/nexus/evolution/chamber.rs`

```rust
const CHAMBER_K: f64 = 1.0;     // Near-critical coupling for sensitivity
const CHAMBER_STEPS: u64 = 500; // Integration steps per evaluation
const CHAMBER_SPHERES: usize = 5; // Isolated test network size

pub struct MutationCandidate {
    pub kind: String,
    pub description: String,
    pub data: serde_json::Value,
}

// Evaluation protocol:
// 1. Create transient Kuramoto network (5 oscillators, K=1.0)
// 2. Run baseline (500 steps, no perturbation) → measure r_baseline
// 3. Apply perturbation from MutationCandidate.data
// 4. Run perturbed (500 steps) → measure r_after
// 5. r_delta = r_after - r_baseline
// 6. Pass if r_after >= r_baseline, Fail otherwise

pub struct GateResult {
    pub r_baseline: f64,
    pub r_after: f64,
    pub r_delta: f64,
    pub verdict: String,   // "Pass" or "Fail"
    pub steps_run: u64,
    pub chamber_k: f64,
}
```

### ME v2 N05 Adaptation Notes

- Implement as `EvolutionGating` trait with `evaluate()` and `quick_check()` methods
- Write results to `evolution_tracking.db` (existing schema, 19,809 existing rows)
- Populate `correlation_log` table (currently 0 rows — fix A13)
- Gate all deployments and config changes
- Triggered by N06 Morphogenic Adapter on adaptation events

---

## N06: Morphogenic Adapter — Homeostatic Feedback

### Reference: `vortex-memory-system/src/nexus/learning/homeostatic.rs`

```rust
pub enum HomeostaticAction {
    IncreaseK,              // r too low → raise coupling
    DecreaseK,              // r too high → lower coupling
    InjectPhase { delta: f64 }, // Off-target → perturb
    AddSphere,              // Low stability → add oscillator
    NoAction,               // Within tolerance
}

const R_TARGET: f64 = 0.70;
const TOLERANCE: f64 = 0.05;  // Dead-band: no action within ±0.05

pub struct HomeostaticController {
    r_target: f64,
    last_r: f64,
    total_interventions: u64,
}

impl HomeostaticController {
    pub fn observe(&mut self, r: f64) -> HomeostaticAction {
        self.last_r = r;
        let r_error = r - self.r_target;

        if r_error.abs() <= TOLERANCE {
            return HomeostaticAction::NoAction;
        }

        self.total_interventions += 1;

        if r_error < -TOLERANCE {
            HomeostaticAction::IncreaseK   // Too incoherent
        } else {
            HomeostaticAction::DecreaseK   // Too rigid
        }
    }
}
```

### ME v2 N06 Adaptation Notes

- Map to `MorphogenicOps` trait: `should_adapt()`, `select_adaptation()`, `apply_adaptation()`
- Use ME v2 spec adaptation rules:
  - r_delta > +0.05 AND r > 0.95 → DecreaseK
  - r_delta < -0.05 AND r < 0.5 → IncreaseK
  - |r_delta| > 0.1 → TriggerPruning + RebalanceSTDP
  - 0.05 < |r_delta| < 0.1 → EmitWarning
- Cool-down period: 60s between adaptations (prevent oscillation)
- Modifies K via N03, triggers pruning via M28, adjusts STDP via M26

---

## Cross-Reference: VMS Source Files → ME v2 Modules

| VMS Source | ME v2 Module | Key Adaptation |
|-----------|-------------|---------------|
| `vortex/kuramoto.rs` | N01 Field Bridge | r computation algorithm |
| `nexus/monitor/coherence_tracker.rs` | N01 Field Bridge | EMA smoothing + trend |
| `nexus/dev_env/intent_encoder.rs` | N02 Intent Router | 12D encoding pattern |
| `nexus/dev_env/intent_router.rs` | N02 Intent Router | Cosine similarity routing |
| `nexus/swarm/coordinator.rs` | N03 Regime Manager | KRegime enum + state |
| `nexus/learning/stdp.rs` | N04 STDP Bridge | LTP/LTD/decay operations |
| `nexus/evolution/chamber.rs` | N05 Evolution Gate | RALPH evaluation protocol |
| `nexus/learning/homeostatic.rs` | N06 Morphogenic Adapter | Setpoint control loop |
| `morphogenic/engine.rs` | N06 Morphogenic Adapter | Substrate adaptation cycle |

---

*All exemplars from production VMS (664 tests, 0 clippy warnings) + SVF (82 tables)*
