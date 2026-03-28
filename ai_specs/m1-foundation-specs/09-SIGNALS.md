# M07 Signals — signals.rs

> **File:** `src/m1_foundation/signals.rs` | **LOC:** ~1,168 | **Tests:** ~55
> **Role:** Push-based signal bus with 3 typed channels (Health, Learning, Dissent)

---

## SignalSubscriber Trait

```rust
pub trait SignalSubscriber: Send + Sync + fmt::Debug {
    fn on_health(&self, signal: &HealthSignal) {}       // default: no-op
    fn on_learning(&self, event: &LearningEvent) {}     // default: no-op
    fn on_dissent(&self, event: &DissentEvent) {}       // default: no-op
    fn subscriber_id(&self) -> &str;                     // required
}
```

**Object safety:** verified (compile-test). Used as `Arc<dyn SignalSubscriber>`.

3 of 4 methods have default no-op implementations — subscribers only override channels they care about.

---

## Signal Types

### HealthSignal
```rust
#[derive(Debug, Clone, PartialEq)]
pub struct HealthSignal {
    pub module_id: ModuleId,
    pub previous_health: f64,    // clamped [0.0, 1.0]
    pub current_health: f64,     // clamped [0.0, 1.0]
    pub reason: String,
    pub timestamp: Timestamp,
    pub context: SignalContext,
}
```

| Method | Returns | Notes |
|--------|---------|-------|
| `new(module, prev, curr, reason)` | `Self` | Clamps both health values |
| `with_timestamp(Timestamp)` | `Self` | const fn, for testing |
| `is_degradation()` | `bool` | current < previous |
| `is_improvement()` | `bool` | current > previous |
| `delta()` | `f64` | current - previous (signed) |

### LearningEvent
```rust
#[derive(Debug, Clone, PartialEq)]
pub struct LearningEvent {
    pub signal: LearningSignal,   // from nam.rs
    pub timestamp: Timestamp,
    pub context: SignalContext,
}
```

### DissentEvent
```rust
#[derive(Debug, Clone, PartialEq)]
pub struct DissentEvent {
    pub dissent: Dissent,         // from nam.rs
    pub source_module: ModuleId,
    pub timestamp: Timestamp,
}
```

### SignalContext
```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignalContext {
    pub source_module: ModuleId,
    pub timestamp: Timestamp,
    pub correlation_id: Option<String>,
}
```

### Signal (Unified Enum)
```rust
#[derive(Debug, Clone)]
pub enum Signal {
    Health(HealthSignal),
    Learning(LearningEvent),
    Dissent(DissentEvent),
}
```

All types implement `Display`. All constructors are `#[must_use]`. All have `const fn with_timestamp()`.

---

## SignalBus

```rust
pub struct SignalBus {
    subscribers: Arc<RwLock<Vec<Arc<dyn SignalSubscriber>>>>,
    config: SignalBusConfig,
    stats: Arc<RwLock<SignalBusStats>>,
}
```

| Method | Signature | Notes |
|--------|-----------|-------|
| `new()` | `-> Self` | max_subscribers=256 |
| `with_config(config)` | `-> Self` | Custom config |
| `subscribe(subscriber)` | `-> Result<()>` | Error if at capacity |
| `emit_health(&signal)` | `()` | Synchronous, in-order delivery |
| `emit_learning(&event)` | `()` | Synchronous, in-order delivery |
| `emit_dissent(&event)` | `()` | Synchronous, in-order delivery |
| `stats()` | `-> SignalBusStats` | Copy out of RwLock |
| `subscriber_count()` | `-> usize` | |
| `config()` | `-> &SignalBusConfig` | const fn |

---

## Concurrency Model (Critical)

**Locking protocol for emit:**
```rust
// 1. Acquire subscribers read lock
let subs = self.subscribers.read();
// 2. Iterate and call on_health/on_learning/on_dissent
for sub in subs.iter() { sub.on_health(signal); }
// 3. Drop subscribers guard
drop(subs);
// 4. Acquire stats write lock (AFTER subscribers released)
let mut stats = self.stats.write();
stats.health_emitted += 1;
```

**Why:** If a subscriber tries to read SignalBus state during callback (e.g., to check subscriber_count), holding both locks would deadlock. Drop-before-write eliminates this.

**Subscribe locking:**
```rust
let mut subs = self.subscribers.write();
if subs.len() >= self.config.max_subscribers { return Err(...); }
subs.push(subscriber);
drop(subs);
let mut stats = self.stats.write();
stats.subscriber_count = self.subscribers.read().len();
```

---

## SignalBusConfig / SignalBusStats

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SignalBusConfig { pub max_subscribers: usize }
// Default: max_subscribers = 256

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SignalBusStats {
    pub health_emitted: u64,
    pub learning_emitted: u64,
    pub dissent_emitted: u64,
    pub subscriber_count: usize,
}
// total_emitted() -> u64 (const fn, sum of 3 counters)
```

---

## Error Conditions

- `subscribe()`: `Error::Config("Signal bus at capacity ({max})")` when `len >= max_subscribers`
- All other methods: infallible

---

*M07 Signals Spec v1.0 | 2026-03-01*
