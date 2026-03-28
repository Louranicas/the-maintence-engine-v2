# Observer Bus - Formal Specification

```json
{"v":"1.0.0","type":"MODULE_SPEC","module":"OBSERVER_BUS","name":"Observer Bus","layer":7,"estimated_loc":500,"estimated_tests":50}
```

**Version:** 1.0.0
**Layer:** L7 (Observer)
**Module:** Observer Bus (Internal Utility)
**Related:** [OBSERVER_LAYER_SPEC.md](OBSERVER_LAYER_SPEC.md), [EVENT_CHANNEL_SPEC.md](EVENT_CHANNEL_SPEC.md), [TYPE_DEFINITIONS_SPEC.md](TYPE_DEFINITIONS_SPEC.md)

---

## Navigation

| Direction | Target |
|-----------|--------|
| Up | [INDEX.md](INDEX.md) |
| Layer | [OBSERVER_LAYER_SPEC.md](OBSERVER_LAYER_SPEC.md) |
| Prev | [EVOLUTION_CHAMBER_SPEC.md](EVOLUTION_CHAMBER_SPEC.md) |
| Next | [RALPH_LOOP_SPEC.md](RALPH_LOOP_SPEC.md) |
| Related | [EVENT_CHANNEL_SPEC.md](EVENT_CHANNEL_SPEC.md) |
| Related | [LOG_CORRELATOR_SPEC.md](LOG_CORRELATOR_SPEC.md) |
| Related | [EMERGENCE_DETECTOR_SPEC.md](EMERGENCE_DETECTOR_SPEC.md) |
| M23 Spec | [../SERVICE_SPEC.md](../SERVICE_SPEC.md) |
| Doc | [../../ai_docs/evolution_chamber_ai_docs/OBSERVER_BUS.md](../../ai_docs/evolution_chamber_ai_docs/OBSERVER_BUS.md) |

---

## 1. Purpose

The Observer Bus is the internal typed publish/subscribe system within L7 that decouples the three observer modules (M37, M38, M39) from each other. It provides zero-copy, statically-typed message passing for high-frequency internal traffic while bridging selected events to the main EventBus (M23) for external consumption.

### Objectives

| Objective | Description |
|-----------|-------------|
| Internal decoupling | M37, M38, M39 communicate through the bus, not direct references |
| Zero-copy delivery | Pass `&T` references to handlers, no serialization overhead |
| Type safety | Three statically-typed channels eliminate runtime deserialization errors |
| Error isolation | Handler failures are counted but never propagate to the publisher |
| External bridging | Selected events forwarded to M23 EventBus on 3 new channels |
| Thread safety | `parking_lot::RwLock` with read-heavy access pattern |

### Design Rationale: Why Not Use M23 Directly

| Aspect | EventBus (M23) | ObserverBus (L7) |
|--------|----------------|-------------------|
| Scope | Cross-layer (L1-L6) | L7-internal only |
| Payload | `serde_json::Value` (string) | Typed Rust structs (`&T` references) |
| Serialization | Required (JSON round-trip) | None (zero-copy) |
| Frequency | Medium (~100 events/sec) | High (~1,000+ events/sec internal) |
| Delivery | At-Least-Once with retry | Fire-and-forget |
| Topics | String-based wildcard matching | Statically typed channels (3) |
| Thread Safety | `Arc<Mutex<...>>` | `parking_lot::RwLock` |
| External Visibility | Yes (all layers can subscribe) | No (L7-only, bridges to M23) |

**Key insight:** M37 produces `CorrelatedEvent` structs at high frequency (~100-1,000/sec). Serializing these to JSON for M23 and deserializing in M38 would waste ~50us per event. The ObserverBus passes `&CorrelatedEvent` references directly at <1us per handler invocation.

---

## 2. Complete Type Definitions

### 2.1 Core Struct

```rust
use parking_lot::RwLock;
use chrono::{DateTime, Utc};

/// Internal L7 typed publish/subscribe bus.
///
/// Provides zero-copy message routing between M37, M38, and M39
/// through three statically-typed channels. Handler closures receive
/// borrowed references to avoid serialization overhead.
///
/// # Thread Safety
/// All fields protected by `parking_lot::RwLock`. Subscriber list
/// locks (read) are never held simultaneously with the stats lock
/// (write). See Section 5 for lock ordering.
///
/// # Layer: L7 (Observer)
/// # Source: src/m7_observer/observer_bus.rs
pub struct ObserverBus {
    /// Handlers for CorrelatedEvent messages from M37 LogCorrelator.
    correlation_subscribers: RwLock<Vec<CorrelationHandler>>,

    /// Handlers for EmergenceRecord messages from M38 EmergenceDetector.
    emergence_subscribers: RwLock<Vec<EmergenceHandler>>,

    /// Handlers for MutationRecord messages from M39 EvolutionChamber.
    evolution_subscribers: RwLock<Vec<EvolutionHandler>>,

    /// Aggregate statistics for monitoring and self-observation.
    stats: RwLock<ObserverBusStats>,
}
```

### 2.2 Handler Type Aliases

```rust
/// Handler invoked when M37 publishes a correlated event.
/// Receives a borrowed reference (zero-copy, no serialization).
/// Must be Send + Sync for cross-thread registration.
pub type CorrelationHandler = Box<dyn Fn(&CorrelatedEvent) -> Result<()> + Send + Sync>;

/// Handler invoked when M38 publishes an emergence record.
pub type EmergenceHandler = Box<dyn Fn(&EmergenceRecord) -> Result<()> + Send + Sync>;

/// Handler invoked when M39 publishes a mutation record.
pub type EvolutionHandler = Box<dyn Fn(&MutationRecord) -> Result<()> + Send + Sync>;
```

### 2.3 Statistics Struct

```rust
/// Aggregate statistics for ObserverBus monitoring.
///
/// All counters are monotonically increasing during the lifetime
/// of the bus. No reset mechanism is provided (by design -- use
/// snapshots for delta computation).
#[derive(Clone, Debug, Default)]
pub struct ObserverBusStats {
    /// Total CorrelatedEvent messages published (monotonic).
    pub correlations_published: u64,

    /// Total EmergenceRecord messages published (monotonic).
    pub emergences_published: u64,

    /// Total MutationRecord messages published (monotonic).
    pub evolutions_published: u64,

    /// Total handler invocations that returned Err (monotonic).
    pub handler_errors: u64,

    /// Timestamp of most recent publish call (any channel).
    pub last_activity: Option<DateTime<Utc>>,
}
```

### 2.4 Internal Message Envelope (Optional)

```rust
/// Internal message wrapper for ObserverBus routing.
/// Used for stats tracking and bridge forwarding.
#[derive(Clone, Debug)]
pub struct ObserverMessage {
    /// Source module that published this message.
    pub source: ObserverSource,

    /// Message category for routing.
    pub message_type: ObserverMessageType,

    /// Timestamp when the message was published.
    pub timestamp: DateTime<Utc>,
}

/// Identifies the publishing module.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ObserverSource {
    LogCorrelator,
    EmergenceDetector,
    EvolutionChamber,
}

/// Categorizes the message for routing and stats.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ObserverMessageType {
    Correlation,
    Emergence,
    Evolution,
}
```

### 2.5 Type Summary

| Type | Kind | Fields | Traits | Size (est.) |
|------|------|--------|--------|-------------|
| `ObserverBus` | struct | 4 | -- | 128 bytes |
| `CorrelationHandler` | type alias | -- | `Fn + Send + Sync` | -- |
| `EmergenceHandler` | type alias | -- | `Fn + Send + Sync` | -- |
| `EvolutionHandler` | type alias | -- | `Fn + Send + Sync` | -- |
| `ObserverBusStats` | struct | 5 | `Clone, Debug, Default` | 48 bytes |
| `ObserverMessage` | struct | 3 | `Clone, Debug` | 40 bytes |
| `ObserverSource` | enum | 3 variants | `Clone, Debug, PartialEq, Eq` | 1 byte |
| `ObserverMessageType` | enum | 3 variants | `Clone, Debug, PartialEq, Eq` | 1 byte |

---

## 3. Message Routing Architecture

### 3.1 Channel Topology

```
+----------------------------------------------------------------------+
|                     ObserverBus Channel Topology                     |
+----------------------------------------------------------------------+
|                                                                      |
|  CHANNEL: correlation                                                |
|  ┌─────────────────┐      ┌──────────────────────────────────┐       |
|  │ M37 LogCorrel.  │─────>│ correlation_subscribers: Vec<FH> │       |
|  │ (Publisher)      │      │   [0] M38 EmergenceDetector      │       |
|  └─────────────────┘      │   [1] M23 Bridge (observation.*)  │       |
|                            └──────────────────────────────────┘       |
|                                                                      |
|  CHANNEL: emergence                                                  |
|  ┌─────────────────┐      ┌──────────────────────────────────┐       |
|  │ M38 Emergence   │─────>│ emergence_subscribers: Vec<FH>   │       |
|  │ (Publisher)      │      │   [0] M39 EvolutionChamber       │       |
|  └─────────────────┘      │   [1] M23 Bridge (emergence.*)   │       |
|                            └──────────────────────────────────┘       |
|                                                                      |
|  CHANNEL: evolution                                                  |
|  ┌─────────────────┐      ┌──────────────────────────────────┐       |
|  │ M39 Evolution   │─────>│ evolution_subscribers: Vec<FH>   │       |
|  │ (Publisher)      │      │   [0] M23 Bridge (evolution.*)   │       |
|  └─────────────────┘      └──────────────────────────────────┘       |
|                                                                      |
+----------------------------------------------------------------------+
```

### 3.2 Routing Algorithm

For each `publish_*` call:

```
PROCEDURE publish(channel, message):
  1. ACQUIRE read lock on channel's subscriber list
  2. FOR EACH handler in subscriber list:
     a. INVOKE handler(message)
     b. IF handler returns Err:
        - INCREMENT local error_count
        - CONTINUE to next handler (do NOT propagate)
  3. RELEASE read lock on subscriber list
  4. ACQUIRE write lock on stats
  5. INCREMENT channel counter (correlations/emergences/evolutions)
  6. ADD error_count to handler_errors
  7. SET last_activity = Utc::now()
  8. RELEASE write lock on stats
  9. RETURN Ok(())
```

**Invariant:** `publish_*` always returns `Ok(())`. Handler errors are absorbed and counted.

### 3.3 Subscription Algorithm

For each `on_*` call:

```
PROCEDURE subscribe(channel, handler):
  1. ACQUIRE write lock on channel's subscriber list
  2. LET idx = subscriber_list.len()
  3. PUSH handler onto subscriber list
  4. RELEASE write lock
  5. RETURN Ok(idx)
```

**Invariant:** Handler indices are stable (append-only Vec, no removal).

---

## 4. API Contract

### 4.1 Constructor

```rust
/// Create a new ObserverBus with empty subscriber lists
/// and zeroed statistics.
///
/// # Postconditions
/// - All subscriber lists are empty
/// - All stats counters are 0
/// - last_activity is None
/// - handler_count() returns (0, 0, 0)
pub fn new() -> Self
```

| Property | Value |
|----------|-------|
| Preconditions | None |
| Postconditions | Empty bus with zeroed stats |
| Errors | Never fails |
| Complexity | O(1) |

### 4.2 Subscription Methods

```rust
/// Register a handler for correlated events from M37.
///
/// # Preconditions
/// - `handler` must be Send + Sync (enforced at compile time)
///
/// # Postconditions
/// - Handler appended to correlation subscriber list
/// - Returned index equals previous list length
/// - Future publish_correlation calls will invoke this handler
///
/// # Errors
/// - Never fails under normal operation
pub fn on_correlation(&self, handler: CorrelationHandler) -> Result<usize>

/// Register a handler for emergence records from M38.
///
/// # Preconditions
/// - `handler` must be Send + Sync (enforced at compile time)
///
/// # Postconditions
/// - Handler appended to emergence subscriber list
/// - Returned index equals previous list length
///
/// # Errors
/// - Never fails under normal operation
pub fn on_emergence(&self, handler: EmergenceHandler) -> Result<usize>

/// Register a handler for mutation records from M39.
///
/// # Preconditions
/// - `handler` must be Send + Sync (enforced at compile time)
///
/// # Postconditions
/// - Handler appended to evolution subscriber list
/// - Returned index equals previous list length
///
/// # Errors
/// - Never fails under normal operation
pub fn on_evolution(&self, handler: EvolutionHandler) -> Result<usize>
```

| Method | Preconditions | Postconditions | Errors | Complexity |
|--------|---------------|----------------|--------|------------|
| `on_correlation` | handler: Send+Sync | Appended at index N | Never | O(1) amortized |
| `on_emergence` | handler: Send+Sync | Appended at index N | Never | O(1) amortized |
| `on_evolution` | handler: Send+Sync | Appended at index N | Never | O(1) amortized |

### 4.3 Publishing Methods

```rust
/// Publish a correlated event to all registered correlation handlers.
///
/// # Preconditions
/// - `event` is a valid CorrelatedEvent reference
///
/// # Postconditions
/// - All registered correlation handlers have been invoked with `event`
/// - stats.correlations_published incremented by 1
/// - stats.handler_errors incremented by count of handlers that returned Err
/// - stats.last_activity updated to current UTC time
///
/// # Errors
/// - Always returns Ok(()) -- handler errors are absorbed
///
/// # Thread Safety
/// - Acquires read lock on correlation_subscribers
/// - Then acquires write lock on stats (after releasing subscriber lock)
/// - Never holds both locks simultaneously
pub fn publish_correlation(&self, event: &CorrelatedEvent) -> Result<()>

/// Publish an emergence record to all registered emergence handlers.
///
/// # Preconditions
/// - `record` is a valid EmergenceRecord reference
///
/// # Postconditions
/// - All registered emergence handlers invoked with `record`
/// - stats.emergences_published incremented by 1
/// - stats.handler_errors incremented by failure count
/// - stats.last_activity updated
///
/// # Errors
/// - Always returns Ok(())
pub fn publish_emergence(&self, record: &EmergenceRecord) -> Result<()>

/// Publish a mutation record to all registered evolution handlers.
///
/// # Preconditions
/// - `mutation` is a valid MutationRecord reference
///
/// # Postconditions
/// - All registered evolution handlers invoked with `mutation`
/// - stats.evolutions_published incremented by 1
/// - stats.handler_errors incremented by failure count
/// - stats.last_activity updated
///
/// # Errors
/// - Always returns Ok(())
pub fn publish_evolution(&self, mutation: &MutationRecord) -> Result<()>
```

| Method | Preconditions | Postconditions | Errors | Complexity |
|--------|---------------|----------------|--------|------------|
| `publish_correlation` | Valid `&CorrelatedEvent` | All handlers invoked, stats updated | Never | O(n) where n = handler count |
| `publish_emergence` | Valid `&EmergenceRecord` | All handlers invoked, stats updated | Never | O(n) |
| `publish_evolution` | Valid `&MutationRecord` | All handlers invoked, stats updated | Never | O(n) |

### 4.4 Query Methods

```rust
/// Return a snapshot of aggregate statistics.
///
/// # Postconditions
/// - Returned value is a clone of current stats (point-in-time snapshot)
/// - No side effects
///
/// # Thread Safety
/// - Acquires read lock on stats (brief)
pub fn stats(&self) -> ObserverBusStats

/// Return the number of registered handlers per channel.
///
/// # Returns
/// Tuple: (correlation_count, emergence_count, evolution_count)
///
/// # Thread Safety
/// - Acquires read lock on each subscriber list sequentially
/// - Never holds more than one lock at a time
pub fn handler_count(&self) -> (usize, usize, usize)
```

| Method | Preconditions | Postconditions | Errors | Complexity |
|--------|---------------|----------------|--------|------------|
| `stats` | None | Returns cloned stats | Never | O(1) |
| `handler_count` | None | Returns (c, e, v) tuple | Never | O(1) |

### 4.5 Complete API Summary

| # | Method | Args | Returns | Mutates State |
|---|--------|------|---------|---------------|
| 1 | `new()` | -- | `Self` | -- |
| 2 | `on_correlation` | `CorrelationHandler` | `Result<usize>` | Appends to correlation list |
| 3 | `on_emergence` | `EmergenceHandler` | `Result<usize>` | Appends to emergence list |
| 4 | `on_evolution` | `EvolutionHandler` | `Result<usize>` | Appends to evolution list |
| 5 | `publish_correlation` | `&CorrelatedEvent` | `Result<()>` | Invokes handlers, updates stats |
| 6 | `publish_emergence` | `&EmergenceRecord` | `Result<()>` | Invokes handlers, updates stats |
| 7 | `publish_evolution` | `&MutationRecord` | `Result<()>` | Invokes handlers, updates stats |
| 8 | `stats` | -- | `ObserverBusStats` | None (read-only) |
| 9 | `handler_count` | -- | `(usize, usize, usize)` | None (read-only) |

---

## 5. Thread Safety & Concurrency Model

### 5.1 Lock Inventory

| Field | Lock Type | Access Pattern | Contention |
|-------|-----------|----------------|------------|
| `correlation_subscribers` | `parking_lot::RwLock<Vec<_>>` | Read-heavy (publish >> subscribe) | Very Low |
| `emergence_subscribers` | `parking_lot::RwLock<Vec<_>>` | Read-heavy (publish >> subscribe) | Very Low |
| `evolution_subscribers` | `parking_lot::RwLock<Vec<_>>` | Read-heavy (publish >> subscribe) | Very Low |
| `stats` | `parking_lot::RwLock<ObserverBusStats>` | Write on every publish | Low |

### 5.2 Lock Ordering

```
STRICT LOCK ORDER (within ObserverBus):

  1. Subscriber list lock (read)       -- held during handler invocation
  2. Stats lock (write)                -- held briefly after all handlers complete

  CONSTRAINT: Subscriber lock MUST be released BEFORE stats lock is acquired.
  CONSTRAINT: No two subscriber list locks may be held simultaneously.
  CONSTRAINT: Stats lock is always acquired last.
```

### 5.3 Lock Lifetime Patterns

#### Correct Pattern (publish)

```rust
// Step 1: Read subscriber list, invoke handlers
let error_count = {
    let subs = self.correlation_subscribers.read();
    let mut errors = 0u64;
    for handler in subs.iter() {
        if handler(event).is_err() {
            errors += 1;
        }
    }
    errors
};  // <-- subs lock dropped HERE (scoped block)

// Step 2: Update stats (subscriber lock already released)
{
    let mut stats = self.stats.write();
    stats.correlations_published += 1;
    stats.handler_errors += error_count;
    stats.last_activity = Some(Utc::now());
}  // <-- stats lock dropped HERE
```

#### Incorrect Pattern (would deadlock)

```rust
// WRONG: holding subscriber lock while acquiring stats lock
let subs = self.correlation_subscribers.read();
for handler in subs.iter() { handler(event)?; }
let mut stats = self.stats.write();  // potential deadlock if handler
                                      // also touches stats
stats.correlations_published += 1;
// both locks held simultaneously -- FORBIDDEN
```

### 5.4 Handler Thread Safety Requirements

All handler closures must satisfy `Send + Sync`:

| Valid Captures | Invalid Captures |
|---------------|-----------------|
| `Arc<T>` where T: Send + Sync | `Rc<T>` (not Send) |
| `Arc<AtomicU64>` | `RefCell<T>` (not Sync) |
| `Arc<RwLock<T>>` | `*mut T` (raw pointer) |
| `Arc<Mutex<T>>` | `Cell<T>` (not Sync) |
| Cloned values (owned) | `&T` with non-static lifetime |

### 5.5 Concurrent Access Guarantees

| Scenario | Behavior |
|----------|----------|
| Multiple threads calling `publish_correlation` simultaneously | Safe -- read lock allows concurrent readers |
| Thread A calling `publish_correlation` while Thread B calls `on_correlation` | Thread B blocks on write lock until A releases read lock |
| Thread A calling `publish_correlation` while Thread B calls `publish_emergence` | Safe -- independent locks, no interference |
| Thread A calling `stats()` while Thread B calls `publish_*` | Thread A may briefly block on stats read lock |

---

## 6. M23 Bridge Integration

### 6.1 Bridge Purpose

The M23 bridge forwards selected L7 internal events to the main EventBus (M23) for external visibility. This bridges the zero-copy internal domain to the serialized cross-layer domain.

### 6.2 Bridge Registration

```rust
/// Register bridge handlers that forward L7 events to M23 EventBus.
/// Called once during ObserverLayer initialization.
///
/// # Preconditions
/// - `bus` is a valid ObserverBus reference
/// - `event_bus` is a valid M23 EventBus reference
/// - The 3 new channels (observation, emergence, evolution) exist in M23
///
/// # Postconditions
/// - 3 bridge handlers registered (one per channel)
/// - Future publish_* calls will also serialize and forward to M23
///
/// # Errors
/// - Returns Err if handler registration fails
pub fn wire_m23_bridge(bus: &ObserverBus, event_bus: &EventBus) -> Result<()> {
    // Bridge: correlation -> observation.correlation
    let eb = event_bus.clone();
    bus.on_correlation(Box::new(move |event: &CorrelatedEvent| {
        let payload = serde_json::to_value(event)?;
        eb.publish_to_topic("observation.correlation", payload)?;
        Ok(())
    }))?;

    // Bridge: emergence -> emergence.detected
    let eb = event_bus.clone();
    bus.on_emergence(Box::new(move |record: &EmergenceRecord| {
        let payload = serde_json::to_value(record)?;
        eb.publish_to_topic("emergence.detected", payload)?;
        Ok(())
    }))?;

    // Bridge: evolution -> evolution.applied
    let eb = event_bus.clone();
    bus.on_evolution(Box::new(move |mutation: &MutationRecord| {
        let payload = serde_json::to_value(mutation)?;
        eb.publish_to_topic("evolution.applied", payload)?;
        Ok(())
    }))?;

    Ok(())
}
```

### 6.3 Bridge Channel Mapping

| ObserverBus Channel | M23 Topic | Serialization | Rate |
|---------------------|-----------|---------------|------|
| Correlation | `observation.correlation` | `serde_json::to_value(&CorrelatedEvent)` | ~100/s |
| Emergence | `emergence.detected` | `serde_json::to_value(&EmergenceRecord)` | ~1-10/min |
| Evolution | `evolution.applied` | `serde_json::to_value(&MutationRecord)` | ~1/min |

### 6.4 Bridge Error Handling

Bridge handlers follow the same fire-and-forget pattern as all ObserverBus handlers:

| Failure Mode | Impact | Recovery |
|-------------|--------|----------|
| Serialization error (`serde_json`) | Event not forwarded to M23 | Counted in `handler_errors`, internal routing unaffected |
| M23 publish failure | Event not visible externally | Counted in `handler_errors`, internal routing unaffected |
| M23 channel not found | Persistent bridge failure | Counted in `handler_errors`, check channel creation order |

**Priority:** Internal L7 processing always takes priority. Bridge failures never block or delay internal message delivery.

---

## 7. Error Handling

### 7.1 Error Model: Fire-and-Forget

| Property | Specification |
|----------|---------------|
| Model | Fire-and-forget (publish always succeeds) |
| Handler errors | Counted in `stats.handler_errors`, not propagated |
| Publisher isolation | A failing subscriber cannot block the publisher |
| Handler independence | One handler's failure does not prevent other handlers from executing |
| Handler execution order | Sequential within a single publish call (Vec iteration order) |
| Retry | None (by design) |

### 7.2 Error Flow

```
Publisher: publish_correlation(&event)
    |
    +---> handler[0](&event) -> Ok(())     // normal execution
    +---> handler[1](&event) -> Err(...)   // error counted, continue
    +---> handler[2](&event) -> Ok(())     // still executes
    |
    +---> stats.correlations_published += 1
    +---> stats.handler_errors += 1        // from handler[1]
    +---> return Ok(())                    // ALWAYS Ok
```

### 7.3 Error Severity Thresholds

| Condition | Severity | Recommended Action |
|-----------|----------|-------------------|
| `handler_errors == 0` | Normal | No action |
| `handler_errors < 10` per 1000 publishes | Low | Log, monitor trend |
| `handler_errors / total > 0.01` (1%) | Medium | Investigate specific handler health |
| `handler_errors / total > 0.10` (10%) | High | Handler is likely broken, check wiring |
| `handler_errors / total > 0.50` (50%) | Critical | Bus degraded, escalate to L2 |

### 7.4 Error Conditions Table

| Error | Source | Propagated? | Recovery |
|-------|--------|-------------|----------|
| Handler returns `Err(...)` | Any registered handler | No (counted in stats) | Continue to next handler |
| Lock poisoning | `parking_lot::RwLock` | N/A (`parking_lot` doesn't poison) | N/A |
| Memory allocation failure | `Vec::push` during registration | Yes (panic) | Extremely rare, system-level |

---

## 8. Performance Characteristics

### 8.1 Latency

| Operation | Expected Latency | Complexity | Notes |
|-----------|-----------------|------------|-------|
| `new()` | <1us | O(1) | 4 RwLock allocations |
| `on_correlation` | <1us | O(1) amortized | Vec push, write lock |
| `on_emergence` | <1us | O(1) amortized | Vec push, write lock |
| `on_evolution` | <1us | O(1) amortized | Vec push, write lock |
| `publish_correlation` | <1us + n*handler_time | O(n) | n = handler count |
| `publish_emergence` | <1us + n*handler_time | O(n) | n = handler count |
| `publish_evolution` | <1us + n*handler_time | O(n) | n = handler count |
| `stats()` | <100ns | O(1) | Clone of 5-field struct |
| `handler_count()` | <300ns | O(1) | 3 sequential read locks |

### 8.2 Per-Handler Invocation Cost

| Handler Type | Direct Call | Via Bridge (+ serialization) |
|-------------|-------------|------------------------------|
| Internal L7 handler | <1us (zero-copy `&T`) | N/A |
| M23 bridge handler | N/A | ~50us (serde_json + M23 publish) |

### 8.3 Memory Budget

| Component | Size | Notes |
|-----------|------|-------|
| ObserverBus struct | ~128 bytes | 4 RwLock containers |
| Per CorrelationHandler | ~128 bytes | Box<dyn Fn> + vtable + captured state |
| Per EmergenceHandler | ~128 bytes | Box<dyn Fn> + vtable + captured state |
| Per EvolutionHandler | ~128 bytes | Box<dyn Fn> + vtable + captured state |
| ObserverBusStats | ~48 bytes | 4 u64 + Option<DateTime> |
| **Total (typical: 6 handlers)** | **~1 KB** | 3 channels x 2 handlers each |

### 8.4 Throughput

| Metric | Value | Notes |
|--------|-------|-------|
| Max publish rate (uncontended) | >100,000/sec | Limited by handler execution time |
| Max publish rate (contended, 4 threads) | >50,000/sec | Read lock allows concurrent readers |
| Max subscriptions | Unbounded (Vec) | Recommend <20 per channel for latency |

---

## 9. Configuration

The ObserverBus has no runtime configuration parameters. Its behavior is fully determined by:

1. The handlers registered via `on_*` methods
2. The compile-time types of messages (`CorrelatedEvent`, `EmergenceRecord`, `MutationRecord`)

### 9.1 Implicit Constraints

| Constraint | Value | Enforcement |
|------------|-------|-------------|
| Max handlers per channel | Unbounded | Not enforced (recommend <20) |
| Handler removal | Not supported | Append-only Vec (by design) |
| Stats reset | Not supported | Use snapshots for delta computation |
| Channel creation | Not supported at runtime | 3 channels fixed at compile time |

### 9.2 Rationale for No Configuration

The ObserverBus is intentionally simple:
- No buffer sizes (messages are not stored, only forwarded)
- No rate limits (handled by publishers M37/M38/M39)
- No topic routing (3 fixed typed channels)
- No retry logic (fire-and-forget by design)
- No persistence (ephemeral in-memory routing)

---

## 10. Testing Matrix

### 10.1 Test Distribution

| Category | Count | Description |
|----------|-------|-------------|
| Construction | 3 | `new()` creates empty bus, stats zeroed, handler_count (0,0,0) |
| Handler Registration | 8 | Register single/multiple handlers, verify indices, all 3 channels |
| Publish/Subscribe Round-Trip | 12 | Register handler, publish message, verify handler received message |
| Statistics Tracking | 8 | Verify counters increment correctly, last_activity updates |
| Error Counting | 6 | Failing handlers counted in handler_errors, publish still returns Ok |
| Concurrent Access | 8 | Multi-threaded publish, subscribe-during-publish, concurrent channels |
| Handler Count Query | 3 | Verify handler_count() returns correct tuple after registrations |
| Edge Cases | 2 | Publish with zero subscribers, register 100+ handlers on one channel |
| **Total** | **50** | |

### 10.2 Test Invariants

| ID | Invariant | Verification |
|----|-----------|--------------|
| INV-1 | `publish_*` always returns `Ok(())` | Assert on every publish call |
| INV-2 | `stats.correlations_published` equals number of `publish_correlation` calls | Assert after each publish |
| INV-3 | `stats.handler_errors` is monotonically increasing | Assert old <= new across operations |
| INV-4 | `handler_count()` tuple matches number of `on_*` calls | Assert after each registration |
| INV-5 | Handler indices are sequential starting at 0 | Assert returned index == expected |
| INV-6 | Subscriber list locks and stats lock are never held simultaneously | Verified by code structure (scoped blocks) |
| INV-7 | All handlers execute even if earlier handlers fail | Assert via counter in each handler |

### 10.3 Key Test Cases

```rust
#[test]
fn test_new_bus_is_empty() {
    let bus = ObserverBus::new();
    assert_eq!(bus.handler_count(), (0, 0, 0));
    let stats = bus.stats();
    assert_eq!(stats.correlations_published, 0);
    assert_eq!(stats.emergences_published, 0);
    assert_eq!(stats.evolutions_published, 0);
    assert_eq!(stats.handler_errors, 0);
    assert!(stats.last_activity.is_none());
}

#[test]
fn test_correlation_round_trip() {
    let bus = ObserverBus::new();
    let received = Arc::new(AtomicBool::new(false));
    let received_clone = Arc::clone(&received);

    let idx = bus.on_correlation(Box::new(move |_event: &CorrelatedEvent| {
        received_clone.store(true, Ordering::SeqCst);
        Ok(())
    })).unwrap_or_default();

    assert_eq!(idx, 0);
    assert_eq!(bus.handler_count(), (1, 0, 0));

    let event = CorrelatedEvent::test_default();
    bus.publish_correlation(&event).unwrap_or_default();

    assert!(received.load(Ordering::SeqCst));
    assert_eq!(bus.stats().correlations_published, 1);
    assert_eq!(bus.stats().handler_errors, 0);
}

#[test]
fn test_error_counting_does_not_propagate() {
    let bus = ObserverBus::new();

    // Failing handler
    bus.on_emergence(Box::new(|_: &EmergenceRecord| {
        Err(crate::m1_foundation::Error::Internal("test".into()))
    })).unwrap_or_default();

    // Succeeding handler
    let ok_count = Arc::new(AtomicU64::new(0));
    let ok_clone = Arc::clone(&ok_count);
    bus.on_emergence(Box::new(move |_: &EmergenceRecord| {
        ok_clone.fetch_add(1, Ordering::Relaxed);
        Ok(())
    })).unwrap_or_default();

    let record = EmergenceRecord::test_default();
    let result = bus.publish_emergence(&record);

    assert!(result.is_ok());                       // INV-1: always Ok
    assert_eq!(ok_count.load(Ordering::SeqCst), 1); // INV-7: second handler ran
    assert_eq!(bus.stats().emergences_published, 1); // INV-2: counter incremented
    assert_eq!(bus.stats().handler_errors, 1);       // one failure counted
}

#[test]
fn test_concurrent_publish_from_multiple_threads() {
    let bus = Arc::new(ObserverBus::new());
    let counter = Arc::new(AtomicU64::new(0));

    let counter_clone = Arc::clone(&counter);
    bus.on_correlation(Box::new(move |_: &CorrelatedEvent| {
        counter_clone.fetch_add(1, Ordering::Relaxed);
        Ok(())
    })).unwrap_or_default();

    let handles: Vec<_> = (0..10)
        .map(|_| {
            let bus = Arc::clone(&bus);
            std::thread::spawn(move || {
                let event = CorrelatedEvent::test_default();
                for _ in 0..100 {
                    bus.publish_correlation(&event).unwrap_or_default();
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap_or_default();
    }

    assert_eq!(counter.load(Ordering::SeqCst), 1000);
    assert_eq!(bus.stats().correlations_published, 1000);
    assert_eq!(bus.stats().handler_errors, 0);
}

#[test]
fn test_publish_with_no_subscribers() {
    let bus = ObserverBus::new();
    let event = CorrelatedEvent::test_default();

    let result = bus.publish_correlation(&event);
    assert!(result.is_ok());
    assert_eq!(bus.stats().correlations_published, 1);
    assert_eq!(bus.stats().handler_errors, 0);
}

#[test]
fn test_multiple_handlers_per_channel() {
    let bus = ObserverBus::new();

    for i in 0..5 {
        let idx = bus.on_evolution(Box::new(move |_: &MutationRecord| {
            Ok(())
        })).unwrap_or_default();
        assert_eq!(idx, i);
    }

    assert_eq!(bus.handler_count(), (0, 0, 5));
}
```

---

## 11. Cross-Spec Dependencies

| Spec | Direction | Dependency |
|------|-----------|------------|
| [OBSERVER_LAYER_SPEC.md](OBSERVER_LAYER_SPEC.md) | Parent | ObserverBus is a component of the L7 layer |
| [LOG_CORRELATOR_SPEC.md](LOG_CORRELATOR_SPEC.md) | Upstream | M37 publishes `CorrelatedEvent` via `publish_correlation` |
| [EMERGENCE_DETECTOR_SPEC.md](EMERGENCE_DETECTOR_SPEC.md) | Bidirectional | M38 subscribes to correlations, publishes `EmergenceRecord` |
| [EVOLUTION_CHAMBER_SPEC.md](EVOLUTION_CHAMBER_SPEC.md) | Downstream | M39 subscribes to emergences, publishes `MutationRecord` |
| [EVENT_CHANNEL_SPEC.md](EVENT_CHANNEL_SPEC.md) | Bridge | Defines the 3 M23 channels that bridge handlers publish to |
| [TYPE_DEFINITIONS_SPEC.md](TYPE_DEFINITIONS_SPEC.md) | Types | Defines all message and handler types |
| [FITNESS_FUNCTION_SPEC.md](FITNESS_FUNCTION_SPEC.md) | Consumer | Fitness evaluator may subscribe to evolution channel |
| [RALPH_LOOP_SPEC.md](RALPH_LOOP_SPEC.md) | Protocol | RALPH loop drives M39 which publishes via ObserverBus |
| [../SYSTEM_SPEC.md](../SYSTEM_SPEC.md) | Architecture | System-level architecture context |
| [../SERVICE_SPEC.md](../SERVICE_SPEC.md) | M23 | Defines EventBus (M23) that bridge handlers forward to |

---

## 12. Implementation Constants

```rust
/// Observer Bus implementation constants.
pub mod observer_bus {
    /// Recommended maximum handlers per channel for latency guarantees.
    /// Not enforced -- advisory only.
    pub const RECOMMENDED_MAX_HANDLERS: usize = 20;

    /// Number of typed channels (fixed at compile time).
    pub const CHANNEL_COUNT: usize = 3;

    /// Channel names for logging and diagnostics.
    pub const CHANNEL_NAMES: [&str; 3] = ["correlation", "emergence", "evolution"];
}
```

---

## 13. Version History

| Version | Date | Changes |
|---------|------|---------|
| 1.0.0 | 2026-01-29 | Initial specification |

---

*Generated: 2026-01-29 | The Maintenance Engine v1.0.0 | L7 Observer Layer*
*[Back to Index](INDEX.md)*
