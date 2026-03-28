# L5: Learning Layer Specification

> Target: ~7,000 LOC | 6 modules (M25-M30) | 300+ tests

---

## Layer Purpose

The Learning layer implements Spike-Timing Dependent Plasticity (STDP) for Hebbian learning across service interaction pathways. It recognizes patterns, prunes weak pathways, consolidates episodic memories, and detects anti-patterns. V2 extends this with cross-session persistence and Nexus STDP bridge integration.

---

## Module Specifications

### M25: Hebbian Manager (`hebbian.rs`)

**Purpose:** Manage Hebbian learning pathways between services and modules.

**Target:** ~850 LOC, 50+ tests

**Key Types:**
```rust
pub struct HebbianManager { inner: RwLock<HebbianManagerInner> }

pub struct Pathway {
    source: ModuleId,
    target: ModuleId,
    weight: f64,          // 0.0 - 1.0
    activation_count: u64,
    last_activated: Timestamp,
    ltp_count: u64,
    ltd_count: u64,
}
```

**Key Traits:**
```rust
pub trait HebbianOps: Send + Sync {
    fn create_pathway(&self, source: ModuleId, target: ModuleId) -> Result<PathwayId>;
    fn strengthen(&self, id: &PathwayId, delta: f64) -> Result<f64>; // LTP
    fn weaken(&self, id: &PathwayId, delta: f64) -> Result<f64>;     // LTD
    fn weight(&self, id: &PathwayId) -> Result<f64>;
    fn co_activate(&self, source: ModuleId, target: ModuleId) -> Result<()>; // +0.05 (C12)
    fn top_pathways(&self, limit: usize) -> Result<Vec<Pathway>>;
    fn ltp_ltd_ratio(&self) -> Result<f64>; // Healthy: 2.0-4.0
}
```

**Database:** `hebbian_pulse.db`

**Tensor Contribution:** D8 (synergy via pathway strength)

---

### M26: STDP Processor (`stdp.rs`)

**Purpose:** Implement spike-timing dependent plasticity rules.

**Target:** ~700 LOC, 50+ tests

**Parameters:**
```rust
pub struct StdpConfig {
    ltp_rate: f64,          // 0.1 (Long-Term Potentiation)
    ltd_rate: f64,          // 0.05 (Long-Term Depression)
    window_ms: u64,         // 100ms timing window
    decay_rate: f64,        // 0.1 (HRS-001 corrected)
    healthy_ratio: (f64, f64), // (2.0, 4.0) LTP:LTD balance
}
```

**Key Traits:**
```rust
pub trait StdpOps: Send + Sync {
    fn process_spike(&self, pre: Timestamp, post: Timestamp, pathway: &PathwayId) -> Result<f64>;
    fn apply_decay(&self) -> Result<DecayReport>;
    fn ltp_ltd_balance(&self) -> Result<f64>;
    fn homeostatic_check(&self) -> Result<HomeostaticStatus>;
}
```

**Rule:** If pre fires before post within window → LTP (+ltp_rate). If post fires before pre → LTD (-ltd_rate).

---

### M27: Pattern Recognizer (`pattern.rs`)

**Purpose:** Detect recurring patterns in service interactions and remediation outcomes.

**Target:** ~950 LOC, 50+ tests

**Key Types:**
```rust
pub struct PatternRecognizer { inner: RwLock<PatternInner> }

pub struct Pattern {
    id: PatternId,
    signature: Vec<f64>,  // Feature vector
    frequency: u64,
    confidence: f64,
    first_seen: Timestamp,
    last_seen: Timestamp,
}
```

**Detectable Patterns:**
- Cascade failures (A→B→C health degradation)
- Periodic failures (time-correlated)
- Load spikes (correlated across services)
- Recovery patterns (successful remediation sequences)

---

### M28: Pathway Pruner (`pruner.rs`)

**Purpose:** Remove weak or inactive pathways to prevent unbounded growth.

**Target:** ~1,300 LOC, 50+ tests

**Pruning Rules:**
- Weight < 0.1 after decay → prune
- No activation for > 24h → candidate
- LTD:LTP ratio > 3:1 → weaken further
- Maximum pathway count: 10,000

---

### M29: Memory Consolidator (`consolidator.rs`)

**Purpose:** Consolidate short-term episodic memories into long-term patterns.

**Target:** ~1,500 LOC, 50+ tests

**Process:**
1. Buffer episodes in episodic_memory.db
2. Every N episodes, scan for recurring patterns
3. Consolidate successful patterns into pathway reinforcement
4. Archive old episodes (>30 days)

**V2 Enhancement:** Cross-session persistence via snapshot/restore to `tensor_memory.db`

---

### M30: Anti-Pattern Detector (`antipattern.rs`)

**Purpose:** Detect harmful patterns and inhibit them via LTD.

**Target:** ~800 LOC, 50+ tests

**Detectable Anti-Patterns:**
- Restart loops (>3 restarts in 5 minutes)
- Cascade amplification (remediation causing wider failure)
- Confidence drift (confidence increasing without outcome improvement)
- Pathway weight explosion (unbounded growth — HRS-001 scenario)

---

## Layer Coordinator (`mod.rs`)

**Target:** ~400 LOC, 20+ tests

**Provides:**
- `LearningLayer` aggregate struct
- Full learning cycle: `process_interaction()` → STDP → pattern → prune → consolidate
- Integration with N04 STDP Bridge for Nexus learning

---

## Design Constraints

- C1: Imports from L1-L4 only
- C4: Zero unsafe/unwrap/expect
- C5: `Timestamp` + `Duration` only
- C12: All pathway updates record STDP co-activation
- HRS-001: decay_rate = 0.1 (never 0.001)

---

## Test Strategy

- Unit tests: 50+ per module
- Integration: `tests/l5_learning_integration.rs`
- Benchmark: `benches/hebbian_learning.rs`
- Property: weights always in [0.0, 1.0], LTP:LTD ratio convergence, decay monotonic
