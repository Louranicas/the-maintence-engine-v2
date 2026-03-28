# Anti-Patterns — What ME v2 Must Never Do

> Compiled from ME v1 bugs, database forensics, and cross-codebase analysis
> Each anti-pattern has: the bad code, the fix, and WHY it matters

---

## A1: SystemTime / chrono Usage

**Severity:** HIGH — Breaks deterministic testing, incompatible with Vortex field

```rust
// ❌ ANTI-PATTERN (found in ME v1 M25 hebbian.rs:29, M31 pbft.rs:29)
use std::time::SystemTime;
pub struct HebbianPulse {
    pub timestamp: SystemTime,  // Wall clock — non-deterministic
}

// ✅ FIX
use crate::m1_foundation::shared_types::Timestamp;
pub struct HebbianPulse {
    pub timestamp: Timestamp,   // Monotonic cycle counter — deterministic
}
```

**Impact:** SystemTime makes tests non-reproducible and breaks Vortex Memory System field equations that expect monotonic cycle counters.

---

## A2: Returning References Through RwLock

**Severity:** CRITICAL — Causes lifetime errors, potential dangling references

```rust
// ❌ ANTI-PATTERN
fn get_service(&self, id: &str) -> Result<&ServiceDefinition> {
    let state = self.state.read();
    state.services.get(id).ok_or(Error::NotFound)
    // Reference dies when guard drops at end of function
}

// ✅ FIX
fn get_service(&self, id: &str) -> Result<ServiceDefinition> {
    let state = self.state.read();
    state.services.get(id)
        .cloned()  // Owned copy
        .ok_or_else(|| Error::ServiceNotFound(id.to_owned()))
}
```

---

## A3: Signal Emission While Holding Lock

**Severity:** CRITICAL — Deadlock if subscriber reads same module

```rust
// ❌ ANTI-PATTERN
fn update(&self, key: &str, value: f64) -> Result<()> {
    let mut state = self.state.write();
    let old = state.current;
    state.current = value;
    // Lock still held!
    self.signal_bus.emit(Signal::Changed { old, new: value });  // DEADLOCK
    drop(state);
    Ok(())
}

// ✅ FIX
fn update(&self, key: &str, value: f64) -> Result<()> {
    let old = {
        let mut state = self.state.write();
        let old = state.current;
        state.current = value;
        old
    };  // Lock dropped here
    self.signal_bus.emit(Signal::Changed { old, new: value });  // Safe
    Ok(())
}
```

---

## A4: Unbounded Channels / Collections

**Severity:** HIGH — Memory leak, eventual OOM under load

```rust
// ❌ ANTI-PATTERN
let (tx, rx) = mpsc::unbounded_channel();  // No backpressure
history.push(event);                        // Unbounded growth

// ✅ FIX
let (tx, rx) = mpsc::channel(1000);  // Bounded capacity

// With history trimming
history.push(event);
if history.len() > MAX_HISTORY {
    let overflow = history.len() - MAX_HISTORY;
    history.drain(..overflow);
}
```

---

## A5: &mut self on Trait Methods

**Severity:** CRITICAL — Breaks Arc<dyn Trait> usage, prevents shared ownership

```rust
// ❌ ANTI-PATTERN
pub trait Ops {
    fn update(&mut self, val: f64) -> Result<()>;  // Can't use with Arc
}

// ✅ FIX
pub trait Ops: Send + Sync {
    fn update(&self, val: f64) -> Result<()>;  // Interior mutability via RwLock
}
```

---

## A6: unwrap() / expect() / panic!() in Production Code

**Severity:** HIGH — Process crash on unexpected input

```rust
// ❌ ANTI-PATTERN
let val = map.get(key).unwrap();
let conn = db.connect().expect("db should exist");
panic!("unexpected state");

// ✅ FIX
let val = map.get(key)
    .ok_or_else(|| Error::NotFound(key.to_owned()))?;
let conn = db.connect()
    .map_err(|e| Error::Database(e.to_string()))?;
return Err(Error::Validation("unexpected state".into()));
```

**Exception:** In `#[cfg(test)]` modules, `.expect("test setup")` is acceptable.

---

## A7: Naive Float Arithmetic

**Severity:** MEDIUM — Accumulated rounding error in confidence scores

```rust
// ❌ ANTI-PATTERN
let confidence = 0.3 * a + 0.25 * b + 0.2 * c + 0.15 * d + 0.1 * e;

// ✅ FIX — FMA (fused multiply-add)
let confidence = 0.3f64.mul_add(a, 0.25f64.mul_add(b, 0.2f64.mul_add(c, 0.15f64.mul_add(d, 0.1 * e))));
```

---

## A8: Float Equality Comparison

**Severity:** MEDIUM — Flaky tests, incorrect branching

```rust
// ❌ ANTI-PATTERN
assert_eq!(health, 0.5);           // Float equality is unreliable
if confidence == 0.9 { /* ... */ }  // May never match

// ✅ FIX
assert!((health - 0.5).abs() < f64::EPSILON);
if (confidence - 0.9).abs() < f64::EPSILON { /* ... */ }
// Or for broader tolerance:
assert!((health - 0.5).abs() < 1e-10);
```

---

## A9: Missing TensorContributor Implementation

**Severity:** HIGH — Breaks TensorRegistry composition, invisible module

```rust
// ❌ ANTI-PATTERN — module exists but doesn't contribute to tensor
pub struct Pipeline { /* ... */ }
impl PipelineOps for Pipeline { /* ... */ }
// No TensorContributor impl → module is invisible to Observer

// ✅ FIX — every module implements TensorContributor (C3)
impl TensorContributor for Pipeline {
    fn contribute(&self) -> ContributedTensor {
        let state = self.state.read();
        let health = state.success_rate();
        let tensor = Tensor12D::new([0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
            health, 0.0, 0.0, 0.0, 0.0, 0.0]);
        let coverage = CoverageBitmap::EMPTY
            .with_dimension(DimensionIndex::HealthScore);
        ContributedTensor::new(tensor, coverage, ContributorKind::Stream)
    }
    fn contributor_kind(&self) -> ContributorKind { ContributorKind::Stream }
    fn module_id(&self) -> &str { "M13" }
}
```

---

## A10: Upward Imports (Layer DAG Violation)

**Severity:** CRITICAL — Circular dependencies, compile failure

```rust
// ❌ ANTI-PATTERN — L3 importing from L4
// In m3_core_logic/pipeline.rs:
use crate::m4_integration::rest::RestClient;  // FORBIDDEN

// ✅ FIX — define trait in lower layer, implement in upper
// In m3_core_logic/pipeline.rs:
pub trait ExternalCaller: Send + Sync {
    fn call(&self, endpoint: &str) -> Result<Response>;
}
// In m4_integration/rest.rs:
impl ExternalCaller for RestClient { /* ... */ }
// Wire via dependency injection in engine.rs
```

**Layer DAG:** L1 ← L2 ← L3 ← L4 ← L5 ← L6 ← L7 ← L8 (arrows = "depends on")

---

## A11: Suppressing Clippy Warnings

**Severity:** MEDIUM — Masks real issues, accumulates technical debt

```rust
// ❌ ANTI-PATTERN
#[allow(clippy::too_many_arguments)]  // Hiding design smell
fn create(a: u32, b: u32, c: u32, d: u32, e: u32, f: u32) -> Result<T> { }

// ✅ FIX — use builder pattern
struct CreateBuilder { a: u32, b: u32, /* ... */ }
impl CreateBuilder {
    fn build(self) -> Result<T> { /* validation */ }
}
```

**Only acceptable `#[allow]`:** `#[allow(clippy::cast_precision_loss)]` on known-safe casts.

---

## A12: Missing Validation in Constructors

**Severity:** MEDIUM — Invalid state propagates through system

```rust
// ❌ ANTI-PATTERN
pub fn new(port: u16, tier: u8) -> Self {
    Self { port, tier }  // No validation — port could be 0, tier could be 255
}

// ✅ FIX — builder with validation
pub fn build(self) -> Result<Service> {
    if self.port == 0 {
        return Err(Error::Validation("Port cannot be zero".into()));
    }
    if self.tier > 5 {
        return Err(Error::Validation(format!("Invalid tier: {}", self.tier)));
    }
    Ok(Service { port: self.port, tier: self.tier })
}
```

---

## A13: Stagnant Evolution Data (ME v1 Database Finding)

**Severity:** HIGH — 19,809 fitness records but 0 correlations

```
evolution_tracking.db:
  fitness_history: 19,809 rows  ← Measured everything
  emergence_log:   6 rows       ← Detected almost nothing
  correlation_log: 0 rows       ← Correlated nothing
  mutation_log:    2 rows       ← Barely mutated
```

**Root cause:** ME v1's Evolution Chamber measured but didn't learn from measurements.

**Fix for ME v2:**
- M39 (Evolution Chamber) must populate correlation_log on every RALPH cycle
- M38 (Emergence Detector) must auto-detect from fitness deltas, not wait for manual logging
- Mutation loop must run automatically, not just on explicit request

---

## A14: Missing Nexus Field Capture (C11 Violation)

**Severity:** HIGH — L4+ operations invisible to Nexus coherence tracking

```rust
// ❌ ANTI-PATTERN — operation without field capture
fn execute_remediation(&self, action: &Action) -> Result<Outcome> {
    let result = self.perform(action)?;
    Ok(result)
}

// ✅ FIX — pre/post field capture on every L4+ operation
fn execute_remediation(&self, action: &Action) -> Result<Outcome> {
    let r_before = self.nexus.field_coherence();
    let result = self.perform(action)?;
    let r_after = self.nexus.field_coherence();
    let r_delta = r_after - r_before;

    if r_delta.abs() > 0.05 {
        self.nexus.trigger_morphogenic_adaptation(r_delta);
    }

    // Record STDP co-activation (C12)
    self.nexus.record_interaction(self.service_id(), action.target_service());

    Ok(result)
}
```

---

## A15: Missing STDP Co-Activation Recording (C12 Violation)

**Severity:** MEDIUM — Service interactions not building Hebbian pathways

```rust
// ❌ ANTI-PATTERN — calling service without recording
fn call_service(&self, target: &str) -> Result<Response> {
    self.rest_client.get(target)
}

// ✅ FIX — record co-activation on every cross-service call
fn call_service(&self, target: &str) -> Result<Response> {
    let response = self.rest_client.get(target)?;
    // +0.05 pathway strength per interaction
    self.stdp_bridge.record_interaction(self.service_id(), target);
    Ok(response)
}
```

---

## Quick Reference: Anti-Pattern Severity Matrix

| ID | Anti-Pattern | Severity | Detection |
|----|-------------|----------|-----------|
| A1 | SystemTime/chrono | HIGH | `rg 'SystemTime\|chrono' --type rs` |
| A2 | Reference through RwLock | CRITICAL | Code review |
| A3 | Signal under lock | CRITICAL | Code review |
| A4 | Unbounded collection | HIGH | `rg 'unbounded' --type rs` |
| A5 | &mut self trait | CRITICAL | Compile error with Arc<dyn> |
| A6 | unwrap/expect/panic | HIGH | `clippy::unwrap_used` deny |
| A7 | Naive float math | MEDIUM | Code review |
| A8 | Float equality | MEDIUM | `clippy::float_cmp` |
| A9 | Missing TensorContributor | HIGH | Compile check |
| A10 | Upward import | CRITICAL | Compile error |
| A11 | Clippy suppression | MEDIUM | `rg '#\[allow' --type rs` |
| A12 | No constructor validation | MEDIUM | Code review |
| A13 | Stagnant evolution data | HIGH | DB query |
| A14 | Missing field capture (C11) | HIGH | Code review |
| A15 | Missing STDP recording (C12) | MEDIUM | Code review |

---

*15 anti-patterns catalogued from ME v1 forensics + gold standard analysis*
