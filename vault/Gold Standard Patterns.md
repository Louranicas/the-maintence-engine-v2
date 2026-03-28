---
tags: [reference/patterns, progressive-disclosure/L2]
aliases: [patterns, gold-standard]
---

# Gold Standard Patterns

> 14 mandatory patterns extracted from M1 Foundation + M2 Services + ME v1
> Every L3-L8 module MUST follow these. No exceptions.

## Pattern Index

| ID | Pattern | Constraint | Used By |
|----|---------|-----------|---------|
| P1 | [[#P1 Module Structure Template]] | ALL | Every module |
| P2 | [[#P2 Interior Mutability]] | C2 | Every struct |
| P3 | [[#P3 Lock Guard Scoping]] | C2 | Every RwLock |
| P4 | [[#P4 Owned Returns]] | C7 | Every trait method |
| P5 | [[#P5 Builder Pattern]] | — | Every constructor |
| P6 | [[#P6 Signal Emission]] | C6 | State transitions |
| P7 | [[#P7 Timestamp]] | C5 | All temporal |
| P8 | [[#P8 Error Handling]] | C4 | All functions |
| P9 | [[#P9 FMA Precision]] | — | Float calculations |
| P10 | [[#P10 Bounded Collections]] | — | Channels, histories |
| P11 | [[#P11 FSM Validation]] | — | State machines |
| P12 | [[#P12 NAM Compliance]] | — | Actions, dissent |
| P13 | [[#P13 Atomic Counters]] | — | Monotonic values |
| P14 | [[#P14 Layer Coordinator]] | C9 | mod.rs files |

## P1: Module Structure Template

```rust
pub struct Module {
    state: RwLock<InnerState>,
    signal_bus: Option<Arc<SignalBus>>,
}
```

Every module: trait definition → supporting types → inner state → implementation → TensorContributor → tests (8 categories, 50+ per module).

## P2: Interior Mutability

All trait methods `&self`. Mutable state in `parking_lot::RwLock<InnerState>`.

```rust
pub trait Ops: Send + Sync {
    fn update(&self, key: &str) -> Result<()>;  // &self, never &mut self
}
```

## P3: Lock Guard Scoping

Drop guards BEFORE signal emission. Three patterns: explicit `drop()`, block scope, conditional mutation.

```rust
let old = { self.state.read().value };
{ self.state.write().value = new; }
if old != new { self.emit_signal(old, new); }  // Lock NOT held
```

## P4: Owned Returns

Never return references through RwLock. Always `.cloned()` or `.collect()`.

## P5: Builder Pattern

All constructors via builder with validation in `build() -> Result<T>`.

## P6: Signal Emission

Three types: `HealthSignal`, `LearningEvent`, `DissentEvent`. Emit AFTER lock release.

## P7: Timestamp

`Timestamp::now()` = monotonic cycle counter. `std::time::Instant` for real-time timeouts. NO chrono, NO SystemTime.

## P8: Error Handling

`Result<T>` everywhere. Zero `unwrap()`, `expect()`, `panic!()`. Use `?` propagation.

## P9: FMA Precision

```rust
let c = 0.3f64.mul_add(a, 0.25f64.mul_add(b, 0.2f64.mul_add(c, 0.15f64.mul_add(d, 0.1 * e))));
```

## P10: Bounded Collections

Channels: `mpsc::channel(1000)`. Histories: `drain(..overflow)`. Pathways: evict weakest at capacity.

## P11: FSM Validation

Pure `is_valid_transition(from, to) -> bool` via `matches!()`. Reject invalid with `Error::InvalidTransition`.

## P12: NAM Compliance

All actions carry `AgentOrigin`. Human is `@0.A` (R5). Dissent recorded via `DissentEvent` (R3).

## P13: Atomic Counters

`AtomicU64` for monotonic counters (PBFT sequence, view number). RwLock for structured state.

## P14: Layer Coordinator

Re-export all public types. Shared enums. `LayerStatus` aggregate struct.

---

**Full reference:** `ai_docs/GOLD_STANDARD_PATTERNS.md` (with complete code snippets)

See [[05 — Design Constraints]] | [[Anti-Patterns]] | [[Rust Exemplars]]
