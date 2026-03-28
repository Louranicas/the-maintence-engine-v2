# Gold Standard Patterns — ME v2 Implementation Reference

> Extracted from M1 Foundation (16,711 LOC) + M2 Services (7,196 LOC) + ME v1 (56K LOC)
> Every L3-L8 module MUST follow these patterns. No exceptions.

---

## P1: Module Structure Template

Every module follows this exact layout:

```rust
//! # M{NN}: Module Name
//!
//! Brief description.
//!
//! ## Layer: L{N}
//! ## 12D Tensor: D{X} (dimension_name)

use std::fmt;
use std::sync::Arc;
use parking_lot::RwLock;

use crate::m1_foundation::{
    error::Error,
    metrics::MetricsRegistry,
    shared_types::{ModuleId, Timestamp},
    signals::{HealthSignal, SignalBus},
    tensor_registry::{ContributedTensor, ContributorKind, TensorContributor},
};
use crate::{Result, Tensor12D};

// ============================================================================
// Trait Definition (public interface)
// ============================================================================

pub trait TraitName: Send + Sync + fmt::Debug {
    fn operation(&self, param: &str) -> Result<Output>;
    fn query(&self) -> Result<Vec<Item>>;
    fn count(&self) -> usize;
}

// ============================================================================
// Supporting Types
// ============================================================================

#[derive(Debug, Clone, PartialEq)]
pub struct Output { /* fields */ }

pub struct ModuleBuilder { /* fields with defaults */ }

// ============================================================================
// Interior State (NEVER pub)
// ============================================================================

#[derive(Debug, Default)]
struct InnerState {
    data: HashMap<String, Item>,
    version: u64,
}

// ============================================================================
// Module Implementation
// ============================================================================

#[derive(Debug)]
pub struct Module {
    state: RwLock<InnerState>,
    signal_bus: Option<Arc<SignalBus>>,
    metrics: Option<Arc<MetricsRegistry>>,
}

impl Module {
    pub fn new() -> Self {
        Self {
            state: RwLock::new(InnerState::default()),
            signal_bus: None,
            metrics: None,
        }
    }

    pub fn with_signal_bus(mut self, bus: Arc<SignalBus>) -> Self {
        self.signal_bus = Some(bus);
        self
    }

    pub fn with_metrics(mut self, metrics: Arc<MetricsRegistry>) -> Self {
        self.metrics = Some(metrics);
        self
    }
}

impl TraitName for Module {
    fn operation(&self, param: &str) -> Result<Output> {
        let previous = {
            let state = self.state.read();
            state.data.get(param).cloned()
        };

        {
            let mut state = self.state.write();
            state.data.insert(param.to_string(), new_item);
            state.version += 1;
        } // Lock dropped here

        // Signal emission AFTER lock release
        if let Some(ref bus) = self.signal_bus {
            bus.emit_health(&HealthSignal::new(
                ModuleId::M_NN,
                old_score,
                new_score,
                format!("Operation on '{param}'"),
            ));
        }

        Ok(output)
    }

    fn query(&self) -> Result<Vec<Item>> {
        let state = self.state.read();
        Ok(state.data.values().cloned().collect()) // Owned return (C7)
    }

    fn count(&self) -> usize {
        self.state.read().data.len()
    }
}

// ============================================================================
// TensorContributor (C3 — mandatory for every module)
// ============================================================================

impl TensorContributor for Module {
    #[allow(clippy::cast_precision_loss)]
    fn contribute(&self) -> ContributedTensor {
        let state = self.state.read();
        let value = /* calculation */;
        drop(state);

        let tensor = Tensor12D::new([
            0.0, 0.0, 0.0, 0.0, 0.0, 0.0,  // D0-D5
            value,                             // D6 (example)
            0.0, 0.0, 0.0, 0.0, 0.0,         // D7-D11
        ]);

        let coverage = CoverageBitmap::EMPTY
            .with_dimension(DimensionIndex::HealthScore);

        ContributedTensor::new(tensor, coverage, ContributorKind::Stream)
    }

    fn contributor_kind(&self) -> ContributorKind { ContributorKind::Stream }
    fn module_id(&self) -> &str { ModuleId::M_NN.as_str() }
}

// ============================================================================
// Tests (8 categories, 50+ per module minimum)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // Helpers
    fn make_item(id: &str) -> Item { /* sensible defaults */ }

    // [COMPILE] — trait object safety
    #[test]
    fn test_trait_is_object_safe() {
        fn accept(_: Box<dyn TraitName>) {}
        accept(Box::new(Module::new()));
    }

    // [BASIC] — happy path
    #[test]
    fn test_new_module_empty() {
        let m = Module::new();
        assert_eq!(m.count(), 0);
    }

    // [INVARIANT] — state machine transitions
    #[test]
    fn test_state_transition() { /* ... */ }

    // [BOUNDARY] — edge cases
    #[test]
    fn test_empty_collection_aggregate() { /* ... */ }

    // [PROPERTY] — invariants that always hold
    #[test]
    fn test_value_always_in_unit_interval() {
        let m = Module::new();
        /* operations */
        let v = m.aggregate();
        assert!((0.0..=1.0).contains(&v));
    }

    // [NEGATIVE] — error paths
    #[test]
    fn test_not_found_returns_error() {
        let m = Module::new();
        assert!(m.operation("ghost").is_err());
    }

    // [INTEGRATION] — signal emission
    #[test]
    fn test_signal_emitted_on_transition() {
        let bus = Arc::new(SignalBus::new());
        let m = Module::new().with_signal_bus(Arc::clone(&bus));
        /* trigger transition */
        assert_eq!(bus.stats().health_emitted, 1);
    }

    // [TENSOR] — contribution correctness
    #[test]
    fn test_tensor_module_id() {
        let m = Module::new();
        assert_eq!(m.module_id(), "M_NN");
    }

    #[test]
    fn test_tensor_dimensions_in_range() {
        let m = Module::new();
        let contrib = m.contribute();
        for dim in 0..12 {
            let v = contrib.tensor.get(dim);
            assert!((0.0..=1.0).contains(&v), "D{dim} out of range: {v}");
        }
    }
}
```

---

## P2: Interior Mutability (C2)

**Rule:** ALL trait methods take `&self`. Interior mutability via `parking_lot::RwLock`.

```rust
// CORRECT — &self with RwLock
pub trait Ops: Send + Sync {
    fn update(&self, key: &str, val: f64) -> Result<()>;
}
impl Ops for Module {
    fn update(&self, key: &str, val: f64) -> Result<()> {
        let mut state = self.state.write();
        state.map.insert(key.to_string(), val);
        Ok(())
    }
}

// WRONG — &mut self breaks Arc<dyn Trait>
pub trait BadOps {
    fn update(&mut self, key: &str, val: f64) -> Result<()>; // ← FORBIDDEN
}
```

---

## P3: Lock Guard Scoping (Critical)

**Rule:** Drop guards BEFORE signal emission to prevent deadlocks.

```rust
// CORRECT — 3 patterns
// Pattern A: Explicit drop
{
    let mut state = self.state.write();
    state.value = new_value;
    drop(state);  // ← Release before signal
}
self.emit_signal(old, new);

// Pattern B: Block scope
let snapshot = {
    let state = self.state.read();
    state.data.clone()  // ← Clone inside scope
};  // ← Lock auto-dropped at brace
expensive_work(&snapshot);

// Pattern C: Conditional mutation + signal
let old = { self.state.read().current };
{ self.state.write().current = new; }
if old != new { self.emit_signal(old, new); }

// WRONG — holding lock during signal emission
let mut state = self.state.write();
state.value = new_value;
self.signal_bus.emit(...);  // ← DEADLOCK if subscriber reads this module
drop(state);
```

---

## P4: Owned Returns Through RwLock (C7)

**Rule:** Never return references through lock guards. Always clone/collect.

```rust
// CORRECT — owned return
fn discover(&self, id: &str) -> Result<ServiceDefinition> {
    let state = self.state.read();
    state.services.get(id)
        .cloned()  // ← Owned clone
        .ok_or_else(|| Error::ServiceNotFound(id.to_owned()))
}

fn discover_all(&self) -> Vec<ServiceDefinition> {
    self.state.read()
        .services.values()
        .cloned()  // ← Owned collection
        .collect()
}

// WRONG — reference through guard
fn discover(&self, id: &str) -> Result<&ServiceDefinition> {  // ← FORBIDDEN
    let state = self.state.read();
    state.services.get(id).ok_or(...)  // Reference dies when guard drops
}
```

---

## P5: Builder Pattern (All Constructors)

**Rule:** All public constructors use builder pattern with validation in `build()`.

```rust
pub struct PipelineBuilder {
    name: String,
    stages: Vec<String>,
    timeout: std::time::Duration,
}

impl PipelineBuilder {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            stages: Vec::new(),
            timeout: std::time::Duration::from_secs(30),
        }
    }

    #[must_use]
    pub fn stage(mut self, stage: &str) -> Self {
        self.stages.push(stage.to_string());
        self
    }

    #[must_use]
    pub fn timeout(mut self, timeout: std::time::Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn build(self) -> Result<Pipeline> {
        if self.stages.is_empty() {
            return Err(Error::Validation("Pipeline needs at least one stage".into()));
        }
        Ok(Pipeline {
            state: RwLock::new(PipelineState {
                name: self.name,
                stages: self.stages,
                timeout: self.timeout,
            }),
            signal_bus: None,
        })
    }
}
```

---

## P6: Signal Emission on State Transitions (C6)

**Rule:** Emit signals via `Arc<SignalBus>` on every meaningful state change.

```rust
// 3 signal types
HealthSignal  — health score changes (D6, D10 impact)
LearningEvent — STDP/Hebbian pathway updates (D8 impact)
DissentEvent  — minority opinion recording (NAM R3)

// Emission pattern (used by M09-M12, extend to M13-M42+)
fn emit_health_transition(&self, svc: &str, from: HealthStatus, to: HealthStatus) {
    if let Some(ref bus) = self.signal_bus {
        let signal = HealthSignal::new(
            ModuleId::M_NN,       // source module
            from.score(),          // previous [0.0, 1.0]
            to.score(),            // current [0.0, 1.0]
            format!("'{svc}' {from} → {to}"),
        );
        bus.emit_health(&signal);
    }
}

// When to emit:
// - Service status changes (M11: Starting→Running, Running→Failed)
// - Health transitions (M10: Healthy→Degraded, Degraded→Unhealthy)
// - Circuit state changes (M12: Closed→Open, HalfOpen→Closed)
// - Pipeline completion (M13: executing→complete/failed)
// - Confidence recalculation (M15: significant delta)
// - Pathway strengthening (M25: LTP event above threshold)
// - Consensus phase advance (M31: Prepare→Commit→Execute)
// - Emergence detection (M38: cascade/synergy/resonance)
```

---

## P7: Timestamp (Not SystemTime, Not chrono) (C5)

**Rule:** Use `Timestamp` (monotonic cycle counter) for ALL temporal needs.

```rust
use crate::m1_foundation::shared_types::Timestamp;

// CORRECT
pub struct Event {
    pub timestamp: Timestamp,          // Cycle counter
    pub duration: std::time::Duration, // For timeouts only
}
let ts = Timestamp::now();             // Atomic increment
let elapsed = ts.elapsed_since(old);   // Cycle difference
let within = ts.within_window(other, 100); // STDP window

// CORRECT for real-time timeouts (circuit breaker, health check)
use std::time::Instant;
let start = Instant::now();
let elapsed = start.elapsed();         // Real Duration

// WRONG
use chrono::Utc;                       // ← FORBIDDEN
use std::time::SystemTime;             // ← FORBIDDEN
let now = SystemTime::now();           // ← FORBIDDEN
```

---

## P8: Error Handling (C4)

**Rule:** Zero unwrap, expect, panic. Use `Result<T>` everywhere.

```rust
// CORRECT — propagate with ?
pub fn operation(&self) -> Result<Output> {
    let data = self.fetch_data()?;           // ? propagation
    let processed = process(&data)?;         // ? propagation
    Ok(Output { data: processed })           // Explicit Ok
}

// CORRECT — fallible lookup
fn discover(&self, id: &str) -> Result<Item> {
    self.state.read().items.get(id)
        .cloned()
        .ok_or_else(|| Error::ServiceNotFound(id.to_owned()))
}

// CORRECT — in tests only, assertion-style
#[cfg(test)]
fn test_example() {
    let result = operation();
    assert!(result.is_ok(), "expected success, got: {result:?}");
}

// WRONG
let val = map.get(key).unwrap();         // ← FORBIDDEN (clippy::unwrap_used)
let val = map.get(key).expect("exists"); // ← FORBIDDEN (clippy::expect_used)
panic!("something went wrong");          // ← FORBIDDEN
```

---

## P9: FMA for Floating-Point Precision

**Rule:** Use fused multiply-add for weighted calculations.

```rust
// CORRECT — FMA chain (no intermediate rounding)
let confidence = 0.3f64.mul_add(
    historical_rate,
    0.25f64.mul_add(
        pattern_strength,
        0.2f64.mul_add(
            severity_score,
            0.15f64.mul_add(pathway_weight, 0.1 * time_factor),
        ),
    ),
);

// WRONG — naive addition (accumulates rounding error)
let confidence = 0.3 * historical_rate
    + 0.25 * pattern_strength
    + 0.2 * severity_score
    + 0.15 * pathway_weight
    + 0.1 * time_factor;
```

---

## P10: Bounded Collections

**Rule:** All channels, queues, and histories have capacity limits.

```rust
// CORRECT — bounded channel
let (tx, rx) = mpsc::channel(1000);

// CORRECT — history trimming
svc.history.push(result);
if svc.history.len() > MAX_HISTORY {
    let overflow = svc.history.len() - MAX_HISTORY;
    svc.history.drain(..overflow);
}

// CORRECT — eviction on capacity
if self.pathways.len() > MAX_PATHWAYS {
    let weakest = self.pathways.iter()
        .min_by(|a, b| a.1.strength.partial_cmp(&b.1.strength).unwrap_or(Ordering::Equal))
        .map(|(k, _)| k.clone());
    if let Some(key) = weakest {
        self.pathways.remove(&key);
    }
}

// WRONG — unbounded
let (tx, rx) = mpsc::unbounded_channel(); // ← FORBIDDEN
```

---

## P11: FSM State Transitions (Validated)

**Rule:** Validate transitions with pure function, reject invalid.

```rust
fn is_valid_transition(from: State, to: State) -> bool {
    matches!(
        (from, to),
        (State::Idle, State::Running)
        | (State::Running, State::Paused)
        | (State::Running, State::Complete)
        | (State::Running, State::Failed)
        | (State::Paused, State::Running)
        | (State::Failed, State::Idle)   // Recovery
    )
}

fn transition(&self, id: &str, to: State) -> Result<Transition> {
    let mut state = self.state.write();
    let entry = state.entries.get_mut(id)
        .ok_or_else(|| Error::ServiceNotFound(id.to_owned()))?;

    let from = entry.current;
    if !is_valid_transition(from, to) {
        return Err(Error::InvalidTransition(format!("{from} → {to}")));
    }

    entry.current = to;
    entry.last_transition = Timestamp::now();
    drop(state);

    self.emit_transition_signal(id, from, to);
    Ok(Transition { from, to, timestamp: Timestamp::now() })
}
```

---

## P12: NAM Compliance Types

**Rule:** All actions carry `AgentOrigin`; dissent is recorded, never suppressed.

```rust
use crate::m1_foundation::nam::{AgentOrigin, Confidence, Dissent, LearningSignal};

// Every action has an origin
pub struct Action {
    pub origin: AgentOrigin,
    pub confidence: Confidence,
    pub payload: ActionPayload,
}

// Human is a peer agent (R5)
let action = Action {
    origin: AgentOrigin::human(),  // "@0.A"
    confidence: Confidence::new(0.95, 0.90, 1.0),
    payload: ActionPayload::Approve,
};

// Dissent is recorded (R3)
let dissent = Dissent {
    agent: AgentOrigin::agent("critic-7", AgentRole::Critic),
    target: "proposal-42".into(),
    reasoning: "Insufficient test coverage for migration".into(),
    confidence: Confidence::new(0.85, 0.7, 0.95),
};
signal_bus.emit_dissent(&DissentEvent::new(dissent));
```

---

## P13: Atomic Counters for Unbounded Values

**Rule:** Use `AtomicU64` for monotonically increasing counters. Use RwLock for structured state.

```rust
use std::sync::atomic::{AtomicU64, Ordering};

pub struct PbftManager {
    current_view: AtomicU64,        // ← Atomic (unbounded counter)
    sequence_counter: AtomicU64,    // ← Atomic (unbounded counter)
    proposals: RwLock<HashMap<...>>,// ← RwLock (structured data)
    fleet: Vec<Agent>,              // ← Immutable after construction
}

impl PbftManager {
    pub fn next_sequence(&self) -> u64 {
        self.sequence_counter.fetch_add(1, Ordering::SeqCst)
    }
}
```

---

## P14: Layer Coordinator (mod.rs)

**Rule:** Re-export all public types. Provide aggregate status struct.

```rust
//! # Layer N: Name

pub mod module_a;
pub mod module_b;
pub mod module_c;

// Re-exports for downstream compatibility (C9)
pub use module_a::{TraitA, StructA, BuilderA};
pub use module_b::{TraitB, StructB, BuilderB};
pub use module_c::{TraitC, StructC, BuilderC};

// Shared types for this layer
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum LayerState {
    #[default]
    Idle,
    Active,
    Degraded,
}

// Aggregate status
#[derive(Debug, Clone, PartialEq)]
pub struct LayerStatus {
    pub layer_id: &'static str,
    pub module_count: u8,
    pub health_score: f64,
    pub tensor: Tensor12D,
}
```

---

## Pattern Dependency Map

```
P1  Module Structure ──────── ALL modules
P2  Interior Mutability ───── ALL modules (C2)
P3  Lock Guard Scoping ────── ALL modules with RwLock
P4  Owned Returns ─────────── ALL trait methods (C7)
P5  Builder Pattern ────────── ALL constructors
P6  Signal Emission ────────── ALL state transitions (C6)
P7  Timestamp ──────────────── ALL temporal needs (C5)
P8  Error Handling ─────────── ALL functions (C4)
P9  FMA ────────────────────── M15 confidence, M39 fitness, N01 field
P10 Bounded Collections ───── M23 event bus, M25 pathways, M37 history
P11 FSM Transitions ────────── M11 lifecycle, M12 circuit, M31 PBFT
P12 NAM Compliance ─────────── M14 escalation, M32 agents, M35 dissent
P13 Atomic Counters ────────── M31 PBFT sequence, M05 state version
P14 Layer Coordinator ──────── ALL mod.rs files
```

---

*Extracted from: M1 (16,711 LOC), M2 (7,196 LOC), ME v1 (56,017 LOC)*
*14 patterns, 12 constraints, 0 exceptions*
