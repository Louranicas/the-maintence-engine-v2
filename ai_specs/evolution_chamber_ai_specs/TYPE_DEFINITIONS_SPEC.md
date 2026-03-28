# L7 Observer Layer -- Type Definitions Specification

> **All L7 Rust Types in One Reference** | The Maintenance Engine v1.0.0

```json
{"v":"1.0.0","type":"TYPE_SPEC","layer":7,"structs":22,"enums":10,"type_aliases":3,"constants":25}
```

**Version:** 1.0.0
**Status:** SPECIFICATION
**Related:** [INDEX.md](INDEX.md) | [OBSERVER_LAYER_SPEC.md](OBSERVER_LAYER_SPEC.md) | [EVENT_CHANNEL_SPEC.md](EVENT_CHANNEL_SPEC.md) | [LOG_CORRELATOR_SPEC.md](LOG_CORRELATOR_SPEC.md)

---

## Navigation

| Direction | Target |
|-----------|--------|
| Up | [INDEX.md](INDEX.md) |
| Prev | [EVENT_CHANNEL_SPEC.md](EVENT_CHANNEL_SPEC.md) |

---

## Table of Contents

| # | Section | Description |
|---|---------|-------------|
| 1 | [Purpose](#1-purpose) | Role of this specification |
| 2 | [Module: m7_observer/mod.rs](#2-module-m7_observermodrs-layer-coordinator) | Layer coordinator types |
| 3 | [Module: m7_observer/observer_bus.rs](#3-module-m7_observerobserver_busrs-internal-pubsub) | Internal pub/sub types |
| 4 | [Module: m7_observer/log_correlator.rs](#4-module-m7_observerlog_correlatorrs-m37) | Log correlator types |
| 5 | [Module: m7_observer/emergence_detector.rs](#5-module-m7_observeremergence_detectorrs-m38) | Emergence detector types |
| 6 | [Module: m7_observer/evolution_chamber.rs](#6-module-m7_observerevolution_chamberrs-m39) | Evolution chamber types |
| 7 | [Module: m7_observer/fitness.rs](#7-module-m7_observerfitnessrs-fitness-evaluator) | Fitness evaluator types |
| 8 | [Constants Reference Table](#8-constants-reference-table) | All L7 constants with defaults |
| 9 | [Dimension Weight Defaults](#9-dimension-weight-defaults) | 12D fitness dimension weights |
| 10 | [Cross-Layer Type Dependencies](#10-cross-layer-type-dependencies) | External type imports |
| 11 | [Trait Implementations Required](#11-trait-implementations-required) | Derive and manual trait table |
| 12 | [Type Inventory Summary](#12-type-inventory-summary) | Complete type count by category |
| 13 | [Version History](#13-version-history) | Changelog |

---

## 1. Purpose

This specification is the **single-source-of-truth reference** for every Rust type definition in the L7 Observer Layer. It consolidates every struct, enum, type alias, and constant across all L7 modules into one document for easy reference during implementation.

### Scope

| Property | Value |
|----------|-------|
| **Layer** | L7 Observer |
| **Structs** | 22 |
| **Enums** | 10 |
| **Type Aliases** | 3 |
| **Constants** | 25 |
| **Source Files** | 6 (`mod.rs`, `observer_bus.rs`, `log_correlator.rs`, `emergence_detector.rs`, `evolution_chamber.rs`, `fitness.rs`) |
| **Concurrency Primitive** | `parking_lot::RwLock` (consistent with L4/L5) |
| **ID Format** | UUID v4 (`uuid::Uuid::new_v4().to_string()`) |
| **Timestamp Type** | `chrono::DateTime<Utc>` |
| **Serialization** | `serde::{Serialize, Deserialize}` on all channel payload types |

### Design Constraints

| Constraint | Rule |
|------------|------|
| No `unsafe` code | `#![forbid(unsafe_code)]` enforced at compile time |
| No `.unwrap()` | `#![deny(clippy::unwrap_used)]` enforced by clippy |
| No `.expect()` | `#![deny(clippy::expect_used)]` enforced by clippy |
| All public items documented | `-W missing_docs` enforced |
| All configs implement `Default` | Constructor fallback guarantee |
| All report types implement `Clone + Debug` | Logging and snapshot support |
| All enums implement `Debug + Clone` | Pattern matching and storage |

---

## 2. Module: `m7_observer/mod.rs` (Layer Coordinator)

### 2.1 Structs

```rust
// =================================================================
// Layer Coordinator Types
// =================================================================

use chrono::{DateTime, Utc};

/// Main L7 entry point. Stored as `Option<ObserverLayer>` in `MaintenanceEngine`.
/// When `None`, zero cost -- no allocations, no subscriptions, no processing.
///
/// # Layer: L7 (Observer)
/// # Integration: `MaintenanceEngine.observer: Option<ObserverLayer>`
pub struct ObserverLayer {
    /// M37: Cross-layer event correlation engine.
    pub log_correlator: LogCorrelator,

    /// M38: Emergent behavior detection engine.
    pub emergence_detector: EmergenceDetector,

    /// M39: RALPH-loop evolution engine.
    pub evolution_chamber: EvolutionChamber,

    /// 12D tensor fitness scoring utility.
    pub fitness_evaluator: FitnessEvaluator,

    /// Internal L7 pub/sub bus connecting M37, M38, M39.
    pub observer_bus: ObserverBus,

    /// Immutable configuration for the entire L7 layer.
    pub config: ObserverConfig,

    /// Layer-level aggregate metrics.
    pub metrics: ObserverMetrics,

    /// Timestamp of layer initialization.
    pub started_at: DateTime<Utc>,
}

/// Configuration for the entire L7 layer.
/// Loaded from `[observer]` section of `config/observer.toml`.
///
/// # Default: All sub-configs use their own `Default` implementations.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ObserverConfig {
    /// Whether L7 is enabled. Default: `true`.
    /// When `false`, `ObserverLayer` is not constructed (`Option::None`).
    pub enabled: bool,

    /// M37 Log Correlator configuration.
    pub log_correlator: LogCorrelatorConfig,

    /// M38 Emergence Detector configuration.
    pub emergence_detector: EmergenceDetectorConfig,

    /// M39 Evolution Chamber configuration.
    pub evolution_chamber: EvolutionChamberConfig,

    /// Fitness Evaluator configuration.
    pub fitness: FitnessConfig,
}

/// Periodic observation report published to the `"observation"` EventBus channel.
/// Aggregates L7 state for external consumers (dashboards, audit logs).
///
/// # Published by: Layer Coordinator (`mod.rs`)
/// # Channel: `"observation"`
/// # Rate: ~1/tick (configurable tick interval)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ObservationReport {
    /// Unique report identifier (UUID v4).
    pub id: String,

    /// Timestamp of report generation.
    pub timestamp: DateTime<Utc>,

    /// Number of correlations discovered since last report.
    pub correlations_since_last: u64,

    /// Number of emergence events detected since last report.
    pub emergences_since_last: u64,

    /// Number of mutations proposed since last report.
    pub mutations_since_last: u64,

    /// Current overall system fitness score [0.0, 1.0].
    pub current_fitness: f64,

    /// Current system state classification.
    pub system_state: SystemState,

    /// Current fitness trend direction.
    pub fitness_trend: FitnessTrend,

    /// Count of currently active (in-flight) mutations.
    pub active_mutations: usize,

    /// Current evolution generation number.
    pub generation: u64,
}

/// L7 layer-level aggregate metrics.
/// Tracked internally; exposed via `ObserverLayer::metrics()`.
///
/// # Thread Safety: Protected by `RwLock` inside `ObserverLayer`.
#[derive(Clone, Debug, Default)]
pub struct ObserverMetrics {
    /// Total events ingested from EventBus across all 6 channels.
    pub events_ingested: u64,

    /// Total correlation links discovered by M37.
    pub correlations_found: u64,

    /// Total emergence events detected by M38.
    pub emergences_detected: u64,

    /// Total mutations proposed by M39.
    pub mutations_proposed: u64,

    /// Total mutations applied (accepted after verification).
    pub mutations_applied: u64,

    /// Total mutations rolled back (fitness decline detected).
    pub mutations_rolled_back: u64,

    /// Total RALPH 5-phase cycles completed by M39.
    pub ralph_cycles: u64,

    /// Total L7 errors (logged and counted, never propagated).
    pub observer_errors: u64,

    /// Average event ingestion latency in milliseconds.
    pub avg_ingestion_latency_ms: f64,

    /// Average correlation computation latency in milliseconds.
    pub avg_correlation_latency_ms: f64,
}
```

### 2.2 Public API

```rust
impl ObserverLayer {
    /// Constructs a new ObserverLayer from the given configuration.
    /// Returns `Err` if any sub-config validation fails.
    pub fn new(config: ObserverConfig) -> Result<Self>;

    /// Registers L7 as subscriber `"l7_observer"` on all 6 existing EventBus
    /// channels and creates 3 new L7-owned channels (observation, emergence,
    /// evolution).
    pub fn subscribe_to_event_bus(&self, event_bus: &EventBus) -> Result<()>;

    /// Executes one observation tick: ingest pending events, run correlation,
    /// run emergence detection, run RALPH cycle if due, evaluate fitness,
    /// publish ObservationReport. Called periodically by the engine.
    pub fn tick(&self) -> Result<ObservationReport>;

    /// Returns the most recent ObservationReport without triggering a tick.
    pub fn get_report(&self) -> ObservationReport;

    /// Returns `true` if L7 is enabled and initialized.
    pub fn is_enabled(&self) -> bool;

    /// Returns the current evolution generation number from M39.
    pub fn generation(&self) -> u64;

    /// Returns the current system state classification from M39.
    pub fn system_state(&self) -> SystemState;

    /// Returns a snapshot of layer-level aggregate metrics.
    pub fn metrics(&self) -> ObserverMetrics;
}
```

### 2.3 Default Implementation

```rust
impl Default for ObserverConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            log_correlator: LogCorrelatorConfig::default(),
            emergence_detector: EmergenceDetectorConfig::default(),
            evolution_chamber: EvolutionChamberConfig::default(),
            fitness: FitnessConfig::default(),
        }
    }
}
```

---

## 3. Module: `m7_observer/observer_bus.rs` (Internal Pub/Sub)

### 3.1 Structs

```rust
// =================================================================
// Observer Bus Types
// =================================================================

use std::collections::HashMap;
use chrono::{DateTime, Utc};
use parking_lot::RwLock;

/// Internal L7 pub/sub bus connecting M37, M38, M39, and the Fitness
/// Evaluator. Decouples internal L7 communication from the external
/// EventBus (M23).
///
/// # Thread Safety: All mutable state protected by `parking_lot::RwLock`.
/// # Lock Order: 1 (acquired before all other L7 locks).
pub struct ObserverBus {
    /// Registered correlation event handlers.
    correlation_subscribers: RwLock<Vec<CorrelationHandler>>,

    /// Registered emergence event handlers.
    emergence_subscribers: RwLock<Vec<EmergenceHandler>>,

    /// Registered evolution event handlers.
    evolution_subscribers: RwLock<Vec<EvolutionHandler>>,

    /// Aggregate bus statistics.
    stats: RwLock<ObserverBusStats>,
}

/// Aggregate statistics for the Observer Bus.
#[derive(Clone, Debug, Default)]
pub struct ObserverBusStats {
    /// Total correlation events published through the bus.
    pub correlations_published: u64,

    /// Total emergence events published through the bus.
    pub emergences_published: u64,

    /// Total evolution events published through the bus.
    pub evolutions_published: u64,

    /// Total handler invocation errors (logged, not propagated).
    pub handler_errors: u64,

    /// Timestamp of the most recent bus activity.
    pub last_activity: Option<DateTime<Utc>>,
}

/// An internal observer message passed through the bus.
/// Used for structured logging and debugging; handlers receive
/// typed references directly.
#[derive(Clone, Debug)]
pub struct ObserverMessage {
    /// Monotonically increasing message ID.
    pub id: u64,

    /// Source module that published this message.
    pub source: ObserverSource,

    /// Classification of the message content.
    pub message_type: ObserverMessageType,

    /// JSON-serialized payload (for audit logging).
    pub payload: String,

    /// Timestamp of message creation.
    pub timestamp: DateTime<Utc>,
}
```

### 3.2 Enums

```rust
/// Source module for internal observer messages.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObserverSource {
    /// M37 Log Correlator.
    LogCorrelator,
    /// M38 Emergence Detector.
    EmergenceDetector,
    /// M39 Evolution Chamber.
    EvolutionChamber,
    /// Fitness Evaluator utility.
    FitnessEvaluator,
    /// Layer Coordinator (mod.rs).
    Coordinator,
}

/// Classification of internal observer message types.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObserverMessageType {
    /// New correlation discovered by M37.
    CorrelationFound,
    /// Emergent behavior detected by M38.
    EmergenceDetected,
    /// Mutation proposed by M39.
    MutationProposed,
    /// Mutation result (applied or rolled back) from M39.
    MutationResult,
    /// Fitness evaluation completed.
    FitnessEvaluated,
    /// RALPH phase transition in M39.
    PhaseTransition,
}
```

### 3.3 Type Aliases

```rust
/// Handler invoked when a new correlation is published.
/// Receives a reference to the CorrelatedEvent. Must be `Send + Sync`
/// for cross-thread handler registration.
pub type CorrelationHandler = Box<dyn Fn(&CorrelatedEvent) -> Result<()> + Send + Sync>;

/// Handler invoked when a new emergence is published.
/// Receives a reference to the EmergenceRecord.
pub type EmergenceHandler = Box<dyn Fn(&EmergenceRecord) -> Result<()> + Send + Sync>;

/// Handler invoked when a new evolution event is published.
/// Receives a reference to the MutationRecord.
pub type EvolutionHandler = Box<dyn Fn(&MutationRecord) -> Result<()> + Send + Sync>;
```

### 3.4 Type Alias Summary

| Alias | Signature | Used By |
|-------|-----------|---------|
| `CorrelationHandler` | `Box<dyn Fn(&CorrelatedEvent) -> Result<()> + Send + Sync>` | M38, Fitness Evaluator |
| `EmergenceHandler` | `Box<dyn Fn(&EmergenceRecord) -> Result<()> + Send + Sync>` | M39, Layer Coordinator |
| `EvolutionHandler` | `Box<dyn Fn(&MutationRecord) -> Result<()> + Send + Sync>` | Fitness Evaluator, Layer Coordinator |

### 3.5 Public API

```rust
impl ObserverBus {
    /// Constructs a new empty Observer Bus with no subscribers.
    pub fn new() -> Self;

    /// Registers a correlation handler. Returns the handler index.
    pub fn on_correlation(&self, handler: CorrelationHandler) -> Result<usize>;

    /// Registers an emergence handler. Returns the handler index.
    pub fn on_emergence(&self, handler: EmergenceHandler) -> Result<usize>;

    /// Registers an evolution handler. Returns the handler index.
    pub fn on_evolution(&self, handler: EvolutionHandler) -> Result<usize>;

    /// Publishes a correlated event to all registered correlation handlers.
    /// Handler errors are logged and counted, never propagated.
    pub fn publish_correlation(&self, event: &CorrelatedEvent) -> Result<()>;

    /// Publishes an emergence record to all registered emergence handlers.
    /// Handler errors are logged and counted, never propagated.
    pub fn publish_emergence(&self, record: &EmergenceRecord) -> Result<()>;

    /// Publishes a mutation record to all registered evolution handlers.
    /// Handler errors are logged and counted, never propagated.
    pub fn publish_evolution(&self, mutation: &MutationRecord) -> Result<()>;

    /// Returns a snapshot of aggregate bus statistics.
    pub fn stats(&self) -> ObserverBusStats;

    /// Returns handler counts as (correlation, emergence, evolution).
    pub fn handler_count(&self) -> (usize, usize, usize);
}
```

---

## 4. Module: `m7_observer/log_correlator.rs` (M37)

### 4.1 Structs

```rust
// =================================================================
// Log Correlator Types (M37)
// =================================================================

use std::collections::{HashMap, VecDeque};
use chrono::{DateTime, Utc};
use parking_lot::RwLock;

/// M37: Cross-layer event correlation engine.
///
/// Subscribes to all 6 EventBus channels and detects temporal, causal,
/// resonance, cascade, and periodic correlations between events from
/// different layers (L1-L6).
///
/// # Layer: L7 (Observer)
/// # Lock Order: 2 (after ObserverBus, before EmergenceDetector)
/// # Dependencies: M23 (EventBus), M01 (Error)
pub struct LogCorrelator {
    /// Ring buffer of correlated events, FIFO eviction at max capacity.
    event_buffer: RwLock<VecDeque<CorrelatedEvent>>,

    /// Active correlation windows keyed by window_id (UUID v4).
    correlation_windows: RwLock<HashMap<String, CorrelationWindow>>,

    /// Detected recurring patterns persisted across buffer evictions.
    recurring_patterns: RwLock<Vec<RecurringPattern>>,

    /// Immutable configuration loaded at construction time.
    config: LogCorrelatorConfig,
}

/// An event ingested by the correlator, enriched with correlation metadata.
///
/// # Serialization: Implements `Serialize, Deserialize` for EventBus publishing.
/// # Published on: `"observation"` channel (wrapped in `ObservationPayload`).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CorrelatedEvent {
    /// Unique identifier for this correlated event (UUID v4).
    pub id: String,

    /// Original event ID from `EventRecord.id` in the source EventBus channel.
    pub original_event_id: String,

    /// Source EventBus channel name.
    /// One of: `health`, `remediation`, `learning`, `consensus`,
    /// `integration`, `metrics`.
    pub channel: String,

    /// Application-defined event classification from `EventRecord.event_type`.
    pub event_type: String,

    /// FNV-1a hash of the original event payload for fast equality checks.
    pub payload_hash: u64,

    /// Timestamp of the original event (preserved from `EventRecord.timestamp`).
    pub timestamp: DateTime<Utc>,

    /// Correlation links discovered for this event against the buffer.
    pub correlations: Vec<CorrelationLink>,

    /// Layer of origin (1-6) indicating which architectural layer generated
    /// the original event.
    pub layer_origin: u8,
}

/// A directed link between two correlated events.
///
/// # Invariant: `confidence` is always in `[0.0, 1.0]`.
/// # Invariant: No self-links (`source_event_id != target_event_id`).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CorrelationLink {
    /// The correlated event's ID (target of the link).
    pub target_event_id: String,

    /// Classification of the correlation relationship.
    pub correlation_type: CorrelationType,

    /// Confidence score in the range [0.0, 1.0].
    pub confidence: f64,

    /// Signed time delta in milliseconds.
    /// Positive: target event occurred after source event.
    /// Negative: target event occurred before source event.
    pub time_delta_ms: i64,
}

/// A time-bounded window grouping correlated events.
///
/// Windows are created on first event ingestion and closed when
/// `end_time + window_size_ms < now`.
#[derive(Clone, Debug)]
pub struct CorrelationWindow {
    /// Unique window identifier (UUID v4).
    pub window_id: String,

    /// Correlated event IDs belonging to this window.
    pub events: Vec<String>,

    /// Timestamp of the earliest event in the window.
    pub start_time: DateTime<Utc>,

    /// Timestamp of the latest event in the window.
    pub end_time: DateTime<Utc>,

    /// Total number of correlations discovered within this window.
    pub correlation_count: u32,
}

/// A recurring pattern detected from repeated event sequences.
///
/// Patterns persist across buffer evictions until `clear_patterns()` is called.
/// Updated in-place when re-detected (occurrence_count, interval stats).
#[derive(Clone, Debug)]
pub struct RecurringPattern {
    /// Unique pattern identifier (UUID v4).
    pub pattern_id: String,

    /// Ordered sequence of `event_type` values forming the pattern.
    pub event_sequence: Vec<String>,

    /// Ordered sequence of channel names corresponding to the events.
    pub channel_sequence: Vec<String>,

    /// Total number of times this pattern has been observed.
    pub occurrence_count: u64,

    /// Mean interval between consecutive pattern occurrences (ms).
    pub average_interval_ms: u64,

    /// Standard deviation of intervals between occurrences (ms).
    pub stddev_interval_ms: f64,

    /// Confidence score based on regularity: `1.0 - (stddev / mean)`.
    pub confidence: f64,

    /// Timestamp of the first observation of this pattern.
    pub first_seen: DateTime<Utc>,

    /// Timestamp of the most recent observation of this pattern.
    pub last_seen: DateTime<Utc>,
}

/// Configuration for the LogCorrelator (M37).
/// All fields have sensible defaults; override via `config/observer.toml`.
///
/// # TOML Section: `[observer.log_correlator]`
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LogCorrelatorConfig {
    /// Maximum time span (ms) for events to be considered in the same
    /// correlation window. Default: 5000. Range: [1000, 30000].
    pub window_size_ms: u64,

    /// Maximum number of `CorrelatedEvent`s retained in the ring buffer.
    /// Default: 10000. Range: [100, 100000].
    pub max_buffer_size: usize,

    /// Minimum confidence threshold for a correlation link to be recorded.
    /// Default: 0.6. Range: [0.0, 1.0].
    pub min_correlation_confidence: f64,

    /// Minimum occurrence count before a sequence is promoted to
    /// `RecurringPattern`. Default: 3. Range: [2, 100].
    pub min_recurring_count: u64,

    /// Maximum time delta (ms) for Temporal correlation detection.
    /// Default: 500. Range: [10, 5000].
    pub temporal_tolerance_ms: u64,

    /// Maximum number of `CorrelationLink`s per `CorrelatedEvent`.
    /// Default: 20. Range: [1, 100].
    pub max_correlations_per_event: usize,

    /// Maximum ratio of stddev to mean interval for Periodic detection.
    /// Default: 0.2 (20%). Range: (0.0, 1.0].
    pub periodic_stddev_ratio: f64,
}

/// Aggregate statistics for the LogCorrelator.
///
/// Returned by `LogCorrelator::stats()` as a snapshot clone.
#[derive(Clone, Debug, Default)]
pub struct CorrelationStats {
    /// Total events ingested since startup.
    pub total_events_ingested: u64,

    /// Total correlation links discovered since startup.
    pub total_correlations_found: u64,

    /// Number of currently open (non-expired) correlation windows.
    pub active_windows: usize,

    /// Number of detected recurring patterns.
    pub recurring_patterns: usize,

    /// Buffer utilization ratio: `current_size / max_buffer_size`.
    pub buffer_utilization: f64,

    /// Average number of correlation links per ingested event.
    pub avg_correlations_per_event: f64,

    /// Breakdown of correlations by `CorrelationType` name.
    pub correlation_type_counts: HashMap<String, u64>,
}
```

### 4.2 Enums

```rust
/// Classification of correlation relationships between events.
///
/// Each variant has a distinct confidence formula documented inline.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CorrelationType {
    /// Events within the same time window across different channels.
    /// Confidence = `1.0 - (|delta_ms| / temporal_tolerance_ms)`.
    Temporal,

    /// Event A from upstream service triggers Event B in downstream service.
    /// Confidence = `0.8 * (1.0 - delta_ms / window_size_ms)`.
    Causal,

    /// Same `event_type` fires across 2+ different layers within a window.
    /// Confidence = `layers_count / 6.0`.
    Resonance,

    /// Failure propagates through service dependency chain: A -> B -> C.
    /// Confidence = `min(0.95, 0.7 + depth * 0.1)`.
    Cascade,

    /// Events recur at regular intervals with stddev < 20% of mean interval.
    /// Confidence = `1.0 - (stddev / mean)`.
    Periodic,
}
```

### 4.3 Default Implementation

```rust
impl Default for LogCorrelatorConfig {
    fn default() -> Self {
        Self {
            window_size_ms: 5000,
            max_buffer_size: 10_000,
            min_correlation_confidence: 0.6,
            min_recurring_count: 3,
            temporal_tolerance_ms: 500,
            max_correlations_per_event: 20,
            periodic_stddev_ratio: 0.2,
        }
    }
}
```

### 4.4 Public API

```rust
impl LogCorrelator {
    /// Creates a new LogCorrelator with the given configuration.
    /// Returns `Err(Error::Validation)` if any config parameter is out of range.
    pub fn new(config: LogCorrelatorConfig) -> Result<Self>;

    /// Ingests a raw EventRecord, computes correlations against the buffer,
    /// and stores the enriched CorrelatedEvent.
    /// FIFO eviction if buffer is at capacity.
    pub fn ingest_event(&self, event: &EventRecord, layer_origin: u8) -> Result<CorrelatedEvent>;

    /// Returns all correlation links for a specific event by ID.
    /// Returns empty `Vec` if event not found.
    pub fn get_correlations(&self, event_id: &str) -> Vec<CorrelationLink>;

    /// Returns all detected recurring patterns, ordered by confidence descending.
    pub fn get_recurring_patterns(&self) -> Vec<RecurringPattern>;

    /// Returns a specific correlation window by ID.
    /// Returns `Err(Error::Validation)` if window not found.
    pub fn get_correlation_window(&self, window_id: &str) -> Result<CorrelationWindow>;

    /// Returns events from the buffer within the given time range,
    /// ordered by timestamp ascending.
    pub fn get_events_in_window(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Vec<CorrelatedEvent>;

    /// Removes events older than `window_size_ms` from the buffer.
    /// Returns the number of events pruned.
    pub fn prune_old_events(&self, older_than: DateTime<Utc>) -> usize;

    /// Returns current aggregate statistics as a snapshot.
    pub fn correlation_stats(&self) -> CorrelationStats;

    /// Runs periodic pattern detection against the current buffer.
    /// Updates existing patterns and creates new ones.
    pub fn detect_periodic_patterns(&self) -> Vec<RecurringPattern>;
}
```

---

## 5. Module: `m7_observer/emergence_detector.rs` (M38)

### 5.1 Structs

```rust
// =================================================================
// Emergence Detector Types (M38)
// =================================================================

use std::collections::{HashMap, VecDeque};
use chrono::{DateTime, Utc};
use parking_lot::RwLock;

/// M38: Emergent behavior detection engine.
///
/// Consumes correlated events from M37 (via Observer Bus) and detects
/// emergent system behaviors: cascading failures, synergy amplification,
/// self-organizing recovery, resonance patterns, load shedding,
/// pathway convergence, and adaptive threshold shifts.
///
/// # Layer: L7 (Observer)
/// # Lock Order: 3 (after ObserverBus and LogCorrelator)
/// # Dependencies: M37 (LogCorrelator), Observer Bus
pub struct EmergenceDetector {
    /// History of detected emergence records (bounded ring buffer).
    detected_behaviors: RwLock<Vec<EmergenceRecord>>,

    /// Active emergence monitors keyed by monitor_id (UUID v4).
    active_monitors: RwLock<HashMap<String, EmergenceMonitor>>,

    /// Historical emergence records (bounded VecDeque).
    behavior_history: RwLock<VecDeque<EmergenceRecord>>,

    /// Immutable configuration loaded at construction time.
    config: EmergenceConfig,
}

/// A detected emergent behavior record.
///
/// # Serialization: Implements `Serialize, Deserialize` for EventBus publishing.
/// # Published on: `"emergence"` channel (wrapped in `EmergencePayload`).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EmergenceRecord {
    /// Unique record identifier (UUID v4).
    pub id: String,

    /// The detected emergent behavior with variant-specific data.
    pub behavior: EmergentBehavior,

    /// Timestamp of detection.
    pub detected_at: DateTime<Utc>,

    /// Detection confidence score [0.0, 1.0].
    pub confidence: f64,

    /// IDs of correlated events that contributed to this detection.
    pub contributing_events: Vec<String>,

    /// Severity classification of the emergent behavior.
    pub severity: EmergenceSeverity,

    /// Whether a human operator has acknowledged this emergence.
    pub acknowledged: bool,
}

/// An active emergence monitor tracking potential emergent behavior.
///
/// Monitors transition through states: Watching -> Triggered -> Cooldown.
#[derive(Clone, Debug)]
pub struct EmergenceMonitor {
    /// Unique monitor identifier (UUID v4).
    pub monitor_id: String,

    /// Type of behavior being monitored (string tag for the `EmergentBehavior` variant).
    pub behavior_type: String,

    /// Current monitor state.
    pub state: MonitorState,

    /// Event IDs accumulated as evidence for this monitor.
    pub accumulated_evidence: Vec<String>,

    /// Current confidence level based on accumulated evidence [0.0, 1.0].
    pub confidence: f64,

    /// Timestamp when this monitor was created.
    pub started_at: DateTime<Utc>,
}

/// Configuration for the Emergence Detector (M38).
///
/// # TOML Section: `[observer.emergence_detector]`
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EmergenceConfig {
    /// Minimum cascade depth before triggering a `CascadingFailure` detection.
    /// Default: 3. Range: [2, 10].
    pub cascade_depth_threshold: u32,

    /// Minimum synergy score delta to trigger `SynergyAmplification` detection.
    /// Default: 0.15. Range: [0.01, 0.50].
    pub synergy_delta_threshold: f64,

    /// Minimum oscillation cycles to trigger `ResonancePattern` detection.
    /// Default: 3. Range: [2, 20].
    pub resonance_min_cycles: u32,

    /// Maximum number of EmergenceRecords retained in history.
    /// Default: 1000. Range: [100, 10000].
    pub history_capacity: usize,

    /// Interval between emergence detection cycles in milliseconds.
    /// Default: 1000. Range: [100, 10000].
    pub detection_interval_ms: u64,

    /// Minimum confidence to emit an emergence record.
    /// Default: 0.7. Range: [0.0, 1.0].
    pub min_confidence: f64,
}

/// Aggregate statistics for the Emergence Detector.
///
/// Returned by `EmergenceDetector::stats()` as a snapshot clone.
#[derive(Clone, Debug, Default)]
pub struct EmergenceStats {
    /// Total emergence events detected since startup.
    pub total_detected: u64,

    /// Breakdown by `EmergentBehavior` variant name.
    pub by_type: HashMap<String, u64>,

    /// Breakdown by `EmergenceSeverity` variant name.
    pub by_severity: HashMap<String, u64>,

    /// Estimated false positive rate based on acknowledged vs total.
    pub false_positive_rate: f64,

    /// Number of currently active monitors.
    pub active_monitors: usize,
}

/// M38-specific metrics for internal tracking.
#[derive(Clone, Debug, Default)]
pub struct DetectorMetrics {
    /// Total detection cycles executed.
    pub detection_cycles: u64,

    /// Total cascades tracked (including resolved).
    pub cascades_tracked: u64,

    /// Total emergences detected.
    pub emergences_detected: u64,

    /// Total false positives (acknowledged as non-issues).
    pub false_positives: u64,

    /// Average detection cycle latency in milliseconds.
    pub avg_detection_latency_ms: f64,
}
```

### 5.2 Enums

```rust
/// Classification of emergent system behaviors detected by M38.
///
/// Each variant carries behavior-specific data fields.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum EmergentBehavior {
    /// Error cascade propagating across multiple services.
    /// Triggered when cascade depth >= `cascade_depth_threshold`.
    CascadingFailure {
        /// Service where the cascade originated.
        origin_service: String,
        /// Services affected by the cascade (in propagation order).
        affected_services: Vec<String>,
        /// Depth of the cascade chain.
        propagation_depth: u32,
        /// Estimated blast radius as fraction of total services [0.0, 1.0].
        estimated_blast_radius: f64,
    },

    /// Positive synergy amplification between cooperating services.
    /// Triggered when synergy delta >= `synergy_delta_threshold`.
    SynergyAmplification {
        /// Services exhibiting synergistic behavior.
        services: Vec<String>,
        /// Change in synergy score (positive = improvement).
        synergy_delta: f64,
        /// Pattern ID from M37 that triggered this detection.
        trigger_pattern: String,
    },

    /// System self-healing without human intervention.
    /// Detected when a failed service recovers through automated pathways.
    SelfOrganizingRecovery {
        /// Service that failed and recovered.
        failed_service: String,
        /// Ordered recovery pathway (services/modules involved).
        recovery_pathway: Vec<String>,
        /// Total recovery time in milliseconds.
        recovery_time_ms: u64,
        /// Whether human intervention was required.
        human_intervention: bool,
    },

    /// Oscillatory pattern detected across multiple layers.
    /// Triggered when cycle count >= `resonance_min_cycles`.
    ResonancePattern {
        /// Layers exhibiting the resonance pattern (1-6).
        layers: Vec<u8>,
        /// Oscillation period in milliseconds.
        frequency_ms: u64,
        /// Amplitude of the oscillation (magnitude of metric change).
        amplitude: f64,
        /// Phase alignment score across layers [0.0, 1.0].
        phase_alignment: f64,
    },

    /// System shedding load to protect critical services.
    /// Detected when an overloaded service reduces non-essential traffic.
    LoadShedding {
        /// Service initiating load shedding.
        overloaded_service: String,
        /// Services receiving shed traffic.
        shed_targets: Vec<String>,
        /// Load level before shedding [0.0, 1.0].
        load_before: f64,
        /// Load level after shedding [0.0, 1.0].
        load_after: f64,
    },

    /// Multiple independent pathways converging on the same outcome.
    /// Detected when parallel remediation/learning paths reach the same target.
    PathwayConvergence {
        /// Pathway identifiers that are converging.
        converging_pathways: Vec<String>,
        /// The common convergence target (service, metric, or module).
        convergence_point: String,
        /// Combined pathway strength [0.0, 1.0].
        combined_strength: f64,
    },

    /// System autonomously adjusting a threshold based on observed behavior.
    /// Detected when a metric threshold changes without explicit configuration.
    AdaptiveThreshold {
        /// Metric whose threshold was adjusted.
        metric: String,
        /// Previous threshold value.
        old_threshold: f64,
        /// New threshold value.
        new_threshold: f64,
        /// Event or pattern that triggered the adaptation.
        adaptation_trigger: String,
    },
}

/// Severity classification for emergence events.
///
/// Used for filtering, escalation decisions, and dashboard prioritization.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EmergenceSeverity {
    /// Low-priority observation; no action required.
    Informational,
    /// Noteworthy behavior; worth monitoring.
    Notable,
    /// Potential issue; may require attention.
    Warning,
    /// Urgent issue; immediate attention recommended.
    Critical,
}

/// State machine for emergence monitors.
///
/// Lifecycle: `Watching` -> `Triggered` -> `Cooldown` -> (recycled or dropped).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MonitorState {
    /// Accumulating evidence; not yet confident enough to trigger.
    Watching,
    /// Confidence threshold met; emergence record emitted.
    Triggered,
    /// Post-trigger cooldown to avoid duplicate detections.
    Cooldown,
}
```

### 5.3 Default Implementation

```rust
impl Default for EmergenceConfig {
    fn default() -> Self {
        Self {
            cascade_depth_threshold: 3,
            synergy_delta_threshold: 0.15,
            resonance_min_cycles: 3,
            history_capacity: 1000,
            detection_interval_ms: 1000,
            min_confidence: 0.7,
        }
    }
}
```

### 5.4 Public API

```rust
impl EmergenceDetector {
    /// Creates a new EmergenceDetector with the given configuration.
    /// Returns `Err(Error::Validation)` if any config parameter is out of range.
    pub fn new(config: EmergenceConfig) -> Result<Self>;

    /// Runs one detection cycle against the provided correlated events.
    /// Returns newly detected emergence records (if any).
    pub fn detect(
        &self,
        correlated_events: &[CorrelatedEvent],
    ) -> Result<Vec<EmergenceRecord>>;

    /// Returns the most recent emergence records, up to `limit`.
    pub fn get_recent_emergences(&self, limit: usize) -> Vec<EmergenceRecord>;

    /// Returns all emergence records matching the given behavior type tag.
    pub fn get_emergences_by_type(&self, behavior_type: &str) -> Vec<EmergenceRecord>;

    /// Acknowledges an emergence record by ID. Sets `acknowledged = true`.
    /// Returns `Err(Error::Validation)` if emergence_id not found.
    pub fn acknowledge(&self, emergence_id: &str) -> Result<()>;

    /// Returns all active emergence monitors.
    pub fn get_active_monitors(&self) -> Vec<EmergenceMonitor>;

    /// Returns emergence counts grouped by severity.
    pub fn emergence_count_by_severity(&self) -> HashMap<String, usize>;

    /// Returns aggregate detection statistics.
    pub fn stats(&self) -> EmergenceStats;
}
```

---

## 6. Module: `m7_observer/evolution_chamber.rs` (M39)

### 6.1 Structs

```rust
// =================================================================
// Evolution Chamber Types (M39)
// =================================================================

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::AtomicU64;
use chrono::{DateTime, Utc};
use parking_lot::RwLock;

/// M39: RALPH-loop evolution engine.
///
/// Implements the 5-phase RALPH (Recognize-Analyze-Learn-Propose-Harvest)
/// meta-learning cycle. Generates parameter mutations based on M38 emergence
/// data, verifies them against fitness evaluations, and rolls back mutations
/// that degrade system fitness.
///
/// # Layer: L7 (Observer)
/// # Lock Order: 4 (after ObserverBus, LogCorrelator, EmergenceDetector)
/// # Dependencies: M38 (EmergenceDetector), Fitness Evaluator, Observer Bus
pub struct EvolutionChamber {
    /// Mutation history (bounded VecDeque, FIFO eviction).
    mutation_history: RwLock<VecDeque<MutationRecord>>,

    /// Active in-flight mutations keyed by mutation_id (UUID v4).
    active_mutations: RwLock<HashMap<String, ActiveMutation>>,

    /// Fitness snapshot history (bounded VecDeque).
    fitness_history: RwLock<VecDeque<FitnessSnapshot>>,

    /// Monotonically increasing generation counter.
    /// Uses `AtomicU64` for lock-free reads.
    generation: AtomicU64,

    /// RALPH loop state machine.
    ralph_state: RwLock<RalphState>,

    /// Immutable configuration loaded at construction time.
    config: EvolutionConfig,
}

/// A record of a proposed or completed mutation.
///
/// # Serialization: Implements `Serialize, Deserialize` for EventBus publishing.
/// # Published on: `"evolution"` channel (wrapped in `EvolutionPayload`).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MutationRecord {
    /// Unique mutation identifier (UUID v4).
    pub id: String,

    /// Generation number when this mutation was proposed.
    pub generation: u64,

    /// The mutation operation with variant-specific parameters.
    pub mutation: MutationType,

    /// Expected fitness improvement from this mutation.
    pub expected_delta: f64,

    /// Actual fitness delta observed after verification.
    /// `None` if not yet verified.
    pub actual_delta: Option<f64>,

    /// Whether the mutation was successfully applied.
    pub applied: bool,

    /// Whether the mutation was rolled back after verification.
    pub rolled_back: bool,

    /// Whether this mutation requires PBFT consensus (L6) before application.
    /// `true` when `expected_delta >= auto_apply_threshold`.
    pub consensus_required: bool,

    /// Result of PBFT consensus vote. `None` if consensus not required
    /// or not yet completed.
    pub consensus_result: Option<bool>,

    /// What triggered this mutation.
    pub trigger: MutationTrigger,

    /// Timestamp when the mutation was proposed.
    pub created_at: DateTime<Utc>,

    /// Timestamp when the mutation was applied. `None` if not applied.
    pub applied_at: Option<DateTime<Utc>>,

    /// Timestamp when the mutation was verified. `None` if not yet verified.
    pub verified_at: Option<DateTime<Utc>>,

    /// RALPH phase that generated this mutation.
    pub source_phase: RalphPhase,

    /// Fitness score before mutation application.
    pub fitness_before: f64,

    /// Fitness score after mutation verification.
    /// Equal to `fitness_before + actual_delta` when verified.
    pub fitness_after: f64,

    /// Verification duration in milliseconds (wall clock).
    pub verification_ms: u64,
}

/// An active in-flight mutation under verification.
///
/// Tracks the mutation from application through the verification window.
/// Rolled back automatically if `verification_deadline` is exceeded
/// or fitness declines below `rollback_threshold`.
#[derive(Clone, Debug)]
pub struct ActiveMutation {
    /// Mutation ID (references `MutationRecord.id`).
    pub mutation_id: String,

    /// The mutation operation being verified.
    pub mutation: MutationType,

    /// System fitness score immediately before mutation application.
    pub fitness_before: f64,

    /// Timestamp when the mutation was applied.
    pub applied_at: DateTime<Utc>,

    /// Deadline for verification completion.
    /// Computed as `applied_at + mutation_verification_ms`.
    pub verification_deadline: DateTime<Utc>,
}

/// A historical fitness snapshot for trend analysis.
///
/// Stored in the fitness history ring buffer.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FitnessSnapshot {
    /// Evolution generation number at snapshot time.
    pub generation: u64,

    /// Overall system fitness score [0.0, 1.0].
    pub overall_fitness: f64,

    /// Per-dimension fitness scores for the 12D tensor.
    pub dimension_scores: [f64; 12],

    /// Timestamp of the snapshot.
    pub timestamp: DateTime<Utc>,
}

/// Current state of the RALPH loop state machine.
///
/// Tracks the 5-phase cycle: Recognize -> Analyze -> Learn -> Propose -> Harvest.
#[derive(Clone, Debug)]
pub struct RalphState {
    /// Current RALPH phase.
    pub current_phase: RalphPhase,

    /// Current cycle number (incremented on each Harvest completion).
    pub cycle_number: u64,

    /// Timestamp when the current cycle started.
    pub cycle_started_at: Option<DateTime<Utc>>,

    /// Timestamp when the last cycle completed.
    pub cycle_completed_at: Option<DateTime<Utc>>,

    /// Number of mutations proposed during the current cycle.
    pub mutations_proposed: u32,

    /// Number of mutations applied during the current cycle.
    pub mutations_applied: u32,

    /// Whether the RALPH loop is paused (e.g., during manual intervention).
    pub paused: bool,
}

/// Configuration for the Evolution Chamber (M39).
///
/// # TOML Section: `[observer.evolution_chamber]`
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EvolutionConfig {
    /// Maximum mutations running concurrently.
    /// Default: 3. Range: [1, 10].
    pub max_concurrent_mutations: usize,

    /// Time allowed for mutation verification in milliseconds.
    /// Default: 30000. Range: [5000, 300000].
    pub mutation_verification_ms: u64,

    /// Maximum fitness snapshots retained in history (EvolutionConfig).
    /// This is M39's internal FitnessSnapshot buffer (~8 hours at 1-min intervals).
    /// NOT to be confused with `FitnessConfig.history_capacity` (200), which is
    /// the FitnessEvaluator's FitnessReport buffer (~3.3 hours at 1-min intervals).
    /// Default: 500. Range: [50, 5000].
    pub fitness_history_capacity: usize,

    /// Maximum mutation records retained in history.
    /// Default: 1000. Range: [100, 10000].
    pub mutation_history_capacity: usize,

    /// Minimum fitness improvement to auto-apply a mutation without consensus.
    /// Mutations with `expected_delta >= auto_apply_threshold` require PBFT.
    /// Default: 0.10. Range: [0.01, 0.50].
    pub auto_apply_threshold: f64,

    /// Fitness decline that triggers automatic rollback.
    /// Default: -0.02. Range: [-0.20, 0.0].
    pub rollback_threshold: f64,

    /// Minimum interval between generation increments in milliseconds.
    /// Prevents rapid mutation churn.
    /// Default: 60000. Range: [10000, 600000].
    pub min_generation_interval_ms: u64,

    /// Maximum parameter change per mutation (absolute value clamp).
    /// Default: 0.20. Range: [0.01, 0.50].
    pub max_mutation_delta: f64,

    /// Maximum number of times a verification window can be extended.
    /// Default: 2. Range: [0, 5].
    pub max_verification_extensions: u32,
}

/// M39-specific metrics for internal tracking.
#[derive(Clone, Debug, Default)]
pub struct ChamberMetrics {
    /// Total RALPH generations completed.
    pub generations_completed: u64,

    /// Total mutations proposed across all generations.
    pub mutations_total: u64,

    /// Total mutations accepted (passed verification).
    pub mutations_accepted: u64,

    /// Total mutations rolled back (failed verification).
    pub mutations_rolled_back: u64,

    /// Total mutations that failed during application.
    pub mutations_failed: u64,

    /// Average overall fitness across all snapshots.
    pub avg_fitness: f64,

    /// Best (highest) fitness score ever recorded.
    pub best_fitness: f64,

    /// Average RALPH cycle duration in milliseconds.
    pub avg_ralph_cycle_ms: f64,
}
```

### 6.2 Enums

```rust
/// Classification of mutation operations generated by M39.
///
/// Each variant specifies the parameter being mutated and the delta applied.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum MutationType {
    /// Adjust a Hebbian pathway weight (L5).
    PathwayAdjustment {
        /// Pathway key in the Hebbian manager.
        pathway_key: String,
        /// Weight delta to apply (clamped by `max_mutation_delta`).
        delta: f64,
    },

    /// Adjust a metric threshold (L1/L2).
    ThresholdAdjustment {
        /// Metric name being adjusted.
        metric: String,
        /// Previous threshold value.
        old: f64,
        /// New threshold value.
        new: f64,
    },

    /// Adjust a learning rate parameter (L5).
    LearningRateAdjustment {
        /// Parameter name (e.g., "ltp_rate", "ltd_rate").
        parameter: String,
        /// Previous learning rate.
        old: f64,
        /// New learning rate.
        new: f64,
    },

    /// Tune a circuit breaker threshold (L2).
    CircuitBreakerTuning {
        /// Service ID whose circuit breaker is being tuned.
        service_id: String,
        /// New failure count threshold.
        new_threshold: u32,
    },

    /// Reweight a service in the load balancer (L2).
    LoadBalancerReweight {
        /// Service ID being reweighted.
        service_id: String,
        /// New load balancer weight.
        new_weight: f64,
    },

    /// Shift an escalation tier boundary (L3).
    EscalationTierShift {
        /// Previous tier designation.
        from: String,
        /// New tier designation.
        to: String,
    },

    /// Adjust a homeostatic target (L5).
    HomeostaticTargetAdjustment {
        /// Target name (e.g., "target_health", "target_synergy").
        target: String,
        /// Previous target value.
        old: f64,
        /// New target value.
        new: f64,
    },
}

/// What triggered a mutation proposal.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum MutationTrigger {
    /// Mutation proposed in response to an emergence event.
    /// Contains the emergence record ID.
    EmergenceResponse(String),

    /// Mutation proposed due to fitness score drift.
    /// Contains the fitness dimension name.
    FitnessDrift(String),

    /// Mutation proposed as part of periodic optimization.
    PeriodicOptimization,

    /// Mutation proposed by the meta-learning subsystem.
    MetaLearning,
}

/// System state classification derived from overall fitness and trend.
///
/// Used by the Layer Coordinator for reporting and by M39 for
/// mutation urgency decisions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SystemState {
    /// Fitness > 0.9 and trend is Improving or Stable.
    Thriving,
    /// Fitness in [0.7, 0.9] or trend is Stable.
    Stable,
    /// Fitness in [0.5, 0.7) or trend is Declining.
    Degraded,
    /// Fitness < 0.5 or trend is Volatile.
    Critical,
}

/// Verdict returned by mutation verification.
///
/// Determines whether a mutation is committed, rolled back, or extended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MutationVerdict {
    /// Mutation improved fitness; commit permanently.
    Commit,
    /// Mutation degraded fitness; rollback immediately.
    Rollback,
    /// Insufficient data; extend verification window.
    Extend,
}

/// RALPH loop phases (5-phase cycle).
///
/// Each phase has a distinct responsibility in the meta-learning cycle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RalphPhase {
    /// Recognize: Identify patterns from M38 emergence data.
    Recognize,
    /// Analyze: Evaluate patterns against the fitness landscape.
    Analyze,
    /// Learn: Extract mutation candidates from analysis results.
    Learn,
    /// Propose: Generate concrete parameter mutations.
    Propose,
    /// Harvest: Collect results and update fitness history.
    Harvest,
}

/// Status of an in-flight mutation through its lifecycle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MutationStatus {
    /// Mutation proposed, awaiting application.
    Proposed,
    /// Mutation applied, under verification.
    Verifying,
    /// Mutation verified and permanently accepted.
    Accepted,
    /// Mutation rejected, rollback completed.
    RolledBack,
    /// Mutation failed during application (error).
    Failed,
}
```

### 6.3 Default Implementation

```rust
impl Default for EvolutionConfig {
    fn default() -> Self {
        Self {
            max_concurrent_mutations: 3,
            mutation_verification_ms: 30_000,
            fitness_history_capacity: 500,
            mutation_history_capacity: 1000,
            auto_apply_threshold: 0.10,
            rollback_threshold: -0.02,
            min_generation_interval_ms: 60_000,
            max_mutation_delta: 0.20,
            max_verification_extensions: 2,
        }
    }
}
```

### 6.4 Public API

```rust
impl EvolutionChamber {
    /// Creates a new EvolutionChamber with the given configuration.
    /// Initializes generation counter to 0.
    /// Returns `Err(Error::Validation)` if any config parameter is out of range.
    pub fn new(config: EvolutionConfig) -> Result<Self>;

    /// Executes one full RALPH cycle: Recognize -> Analyze -> Learn ->
    /// Propose -> Harvest. Returns proposed mutations (if any).
    pub fn run_ralph_cycle(
        &self,
        emergences: &[EmergenceRecord],
        fitness: &FitnessReport,
    ) -> Result<Vec<MutationRecord>>;

    /// Applies a mutation by ID. Transitions status from Proposed to Verifying.
    /// Returns `Err` if mutation not found or already applied.
    pub fn apply_mutation(&self, mutation_id: &str) -> Result<()>;

    /// Verifies a mutation by comparing current fitness against pre-mutation fitness.
    /// Returns the verdict: Commit, Rollback, or Extend.
    pub fn verify_mutation(
        &self,
        mutation_id: &str,
        current_fitness: f64,
    ) -> Result<MutationVerdict>;

    /// Rolls back a mutation by restoring the original parameter value.
    /// Transitions status to RolledBack.
    pub fn rollback_mutation(&self, mutation_id: &str) -> Result<()>;

    /// Returns the current generation number (lock-free atomic read).
    pub fn get_generation(&self) -> u64;

    /// Returns fitness snapshots for trend analysis.
    /// Returns the most recent `window` snapshots.
    pub fn get_fitness_trend(&self, window: usize) -> Vec<FitnessSnapshot>;

    /// Returns the mutation success rate: `accepted / (accepted + rolled_back)`.
    /// Returns 0.0 if no mutations have been completed.
    pub fn get_mutation_success_rate(&self) -> f64;

    /// Returns the current system state classification based on fitness and trend.
    pub fn get_system_state(&self) -> SystemState;

    /// Returns all currently active (in-flight) mutations.
    pub fn get_active_mutations(&self) -> Vec<ActiveMutation>;

    /// Returns mutation history, most recent first, up to `limit`.
    pub fn get_mutation_history(&self, limit: usize) -> Vec<MutationRecord>;
}
```

---

## 7. Module: `m7_observer/fitness.rs` (Fitness Evaluator)

### 7.1 Structs

```rust
// =================================================================
// Fitness Evaluator Types
// =================================================================

use std::collections::VecDeque;
use chrono::{DateTime, Utc};
use parking_lot::RwLock;

/// 12D tensor fitness evaluator.
///
/// Computes a weighted fitness score from the 12-dimensional tensor encoding,
/// tracks fitness history, and provides trend and stability analysis.
///
/// # Layer: L7 (Observer)
/// # Lock Order: 5 (last in the L7 lock order)
/// # Dependencies: Tensor12D (lib.rs)
pub struct FitnessEvaluator {
    /// Mutable dimension weights (adjustable by M39 Evolution Chamber).
    weights: RwLock<[f64; 12]>,

    /// Fitness report history (bounded ring buffer).
    history: RwLock<VecDeque<FitnessReport>>,

    /// Immutable configuration loaded at construction time.
    config: FitnessConfig,
}

/// A complete fitness evaluation report.
///
/// # Serialization: Implements `Serialize, Deserialize` for EventBus publishing.
/// # Published on: `"evolution"` channel (as part of `EvolutionPayload`).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FitnessReport {
    /// Unique report identifier (UUID v4).
    pub id: String,

    /// Overall weighted fitness score [0.0, 1.0].
    pub overall_fitness: f64,

    /// Raw per-dimension fitness scores [0.0, 1.0] for each of 12 dimensions.
    pub dimension_scores: [f64; 12],

    /// Weighted per-dimension contributions (dimension_scores * weights).
    pub weighted_scores: [f64; 12],

    /// Index of the dimension with the lowest score (0-11).
    pub weakest_dimension: usize,

    /// Index of the dimension with the highest score (0-11).
    pub strongest_dimension: usize,

    /// Current fitness trend direction.
    pub trend: FitnessTrend,

    /// Timestamp of the evaluation.
    pub timestamp: DateTime<Utc>,
}

/// Configuration for the Fitness Evaluator.
///
/// # TOML Section: `[observer.fitness]`
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FitnessConfig {
    /// Maximum fitness reports retained in history (FitnessConfig).
    /// This is the FitnessEvaluator's FitnessReport buffer (~3.3 hours at 1-min intervals).
    /// NOT to be confused with `EvolutionConfig.fitness_history_capacity` (500), which is
    /// M39's internal FitnessSnapshot buffer (~8 hours at 1-min intervals).
    /// Default: 200. Range: [10, 1000].
    pub history_capacity: usize,

    /// Number of recent reports used for trend calculation (linear regression).
    /// Default: 10. Range: [3, 50].
    pub trend_window: usize,

    /// Standard deviation below this value = Stable trend.
    /// Default: 0.02. Range: [0.001, 0.10].
    pub stability_tolerance: f64,

    /// Standard deviation above this value = Volatile trend.
    /// Default: 0.05. Range: [0.01, 0.20].
    pub volatility_threshold: f64,
}
```

### 7.2 Enums

```rust
/// Fitness trend direction derived from recent fitness history.
///
/// Computed using linear regression over the `trend_window` most recent
/// fitness snapshots.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FitnessTrend {
    /// Positive slope above `stability_tolerance`.
    Improving,
    /// Slope magnitude within `stability_tolerance`.
    Stable,
    /// Negative slope below `-stability_tolerance`.
    Declining,
    /// Standard deviation above `volatility_threshold`.
    Volatile,
}
```

### 7.3 Default Implementation

```rust
impl Default for FitnessConfig {
    fn default() -> Self {
        Self {
            history_capacity: 200,
            trend_window: 10,
            stability_tolerance: 0.02,
            volatility_threshold: 0.05,
        }
    }
}
```

### 7.4 Public API

```rust
impl FitnessEvaluator {
    /// Creates a new FitnessEvaluator with the given configuration.
    /// Initializes weights to `FITNESS_DIMENSION_WEIGHTS` defaults.
    /// Returns `Err(Error::Validation)` if any config parameter is out of range.
    pub fn new(config: FitnessConfig) -> Result<Self>;

    /// Evaluates a single 12D tensor and returns a FitnessReport.
    /// Stores the report in history (FIFO eviction at capacity).
    pub fn evaluate(&self, tensor: &Tensor12D) -> Result<FitnessReport>;

    /// Evaluates a fleet of 12D tensors and returns an aggregate FitnessReport.
    /// The aggregate uses the mean of each dimension across all tensors.
    pub fn evaluate_fleet(&self, tensors: &[Tensor12D]) -> Result<FitnessReport>;

    /// Returns the current fitness trend based on the most recent
    /// `trend_window` reports.
    pub fn get_trend(&self) -> FitnessTrend;

    /// Returns fitness report history, most recent first, up to `limit`.
    pub fn get_history(&self, limit: usize) -> Vec<FitnessReport>;

    /// Adjusts the dimension weights used for fitness scoring.
    /// Weights must sum to 1.0 (+/- 0.001 tolerance).
    /// Returns `Err(Error::Validation)` if weights don't sum to ~1.0.
    pub fn adjust_weights(&self, new_weights: [f64; 12]) -> Result<()>;

    /// Returns the current dimension weights.
    pub fn current_weights(&self) -> [f64; 12];
}
```

---

## 8. Constants Reference Table

### 8.1 M37 Log Correlator Constants

| Constant | Type | Default | Range | Description |
|----------|------|---------|-------|-------------|
| `WINDOW_SIZE_MS` | `u64` | `5000` | [1000, 30000] | Correlation window duration |
| `MAX_BUFFER_SIZE` | `usize` | `10000` | [100, 100000] | Max correlated events in ring buffer |
| `MIN_CORRELATION_CONFIDENCE` | `f64` | `0.6` | [0.0, 1.0] | Min confidence to record a link |
| `MIN_RECURRING_COUNT` | `u64` | `3` | [2, 100] | Min occurrences for recurring pattern |
| `TEMPORAL_TOLERANCE_MS` | `u64` | `500` | [10, 5000] | Temporal correlation tolerance window |
| `MAX_CORRELATIONS_PER_EVENT` | `usize` | `20` | [1, 100] | Cap on links per event |
| `PERIODIC_STDDEV_RATIO` | `f64` | `0.2` | (0.0, 1.0] | Max stddev/mean for periodic detection |

### 8.2 M38 Emergence Detector Constants

| Constant | Type | Default | Range | Description |
|----------|------|---------|-------|-------------|
| `CASCADE_DEPTH_THRESHOLD` | `u32` | `3` | [2, 10] | Min cascade depth to trigger detection |
| `SYNERGY_DELTA_THRESHOLD` | `f64` | `0.15` | [0.01, 0.50] | Min synergy change to trigger detection |
| `RESONANCE_MIN_CYCLES` | `u32` | `3` | [2, 20] | Min oscillation cycles for resonance |
| `EMERGENCE_HISTORY_CAPACITY` | `usize` | `1000` | [100, 10000] | Max emergence records retained |
| `DETECTION_INTERVAL_MS` | `u64` | `1000` | [100, 10000] | Emergence detection cycle interval |
| `EMERGENCE_MIN_CONFIDENCE` | `f64` | `0.7` | [0.0, 1.0] | Min confidence to emit emergence |

### 8.3 M39 Evolution Chamber Constants

| Constant | Type | Default | Range | Description |
|----------|------|---------|-------|-------------|
| `MAX_CONCURRENT_MUTATIONS` | `usize` | `3` | [1, 10] | Max active in-flight mutations |
| `MUTATION_VERIFICATION_MS` | `u64` | `30000` | [5000, 300000] | Verification window duration |
| `FITNESS_HISTORY_CAPACITY` | `usize` | `500` | [50, 5000] | Fitness snapshot history size (EvolutionConfig -- not FitnessConfig) |
| `MUTATION_HISTORY_CAPACITY` | `usize` | `1000` | [100, 10000] | Mutation record history size |
| `AUTO_APPLY_THRESHOLD` | `f64` | `0.10` | [0.01, 0.50] | Auto-apply vs consensus threshold |
| `ROLLBACK_THRESHOLD` | `f64` | `-0.02` | [-0.20, 0.0] | Fitness drop triggering rollback |
| `MIN_GENERATION_INTERVAL_MS` | `u64` | `60000` | [10000, 600000] | Min time between generations |
| `MAX_MUTATION_DELTA` | `f64` | `0.20` | [0.01, 0.50] | Max parameter change per mutation |
| `MAX_VERIFICATION_EXTENSIONS` | `u32` | `2` | [0, 5] | Max verification window extensions |

### 8.4 Fitness Evaluator Constants

| Constant | Type | Default | Range | Description |
|----------|------|---------|-------|-------------|
| `FITNESS_TREND_WINDOW` | `usize` | `10` | [3, 50] | Trend regression window size |
| `STABILITY_TOLERANCE` | `f64` | `0.02` | [0.001, 0.10] | Slope magnitude for Stable trend |
| `VOLATILITY_THRESHOLD` | `f64` | `0.05` | [0.01, 0.20] | Std dev threshold for Volatile |

### 8.5 Rust Constant Definitions

```rust
// =================================================================
// L7 Observer Layer Constants
// =================================================================

// --- M37 Log Correlator ---
pub const WINDOW_SIZE_MS: u64 = 5000;
pub const MAX_BUFFER_SIZE: usize = 10_000;
pub const MIN_CORRELATION_CONFIDENCE: f64 = 0.6;
pub const MIN_RECURRING_COUNT: u64 = 3;
pub const TEMPORAL_TOLERANCE_MS: u64 = 500;
pub const MAX_CORRELATIONS_PER_EVENT: usize = 20;
pub const PERIODIC_STDDEV_RATIO: f64 = 0.2;

// --- M38 Emergence Detector ---
pub const CASCADE_DEPTH_THRESHOLD: u32 = 3;
pub const SYNERGY_DELTA_THRESHOLD: f64 = 0.15;
pub const RESONANCE_MIN_CYCLES: u32 = 3;
pub const EMERGENCE_HISTORY_CAPACITY: usize = 1000;
pub const DETECTION_INTERVAL_MS: u64 = 1000;
pub const EMERGENCE_MIN_CONFIDENCE: f64 = 0.7;

// --- M39 Evolution Chamber ---
pub const MAX_CONCURRENT_MUTATIONS: usize = 3;
pub const MUTATION_VERIFICATION_MS: u64 = 30_000;
pub const FITNESS_HISTORY_CAPACITY: usize = 500;
pub const MUTATION_HISTORY_CAPACITY: usize = 1000;
pub const AUTO_APPLY_THRESHOLD: f64 = 0.10;
pub const ROLLBACK_THRESHOLD: f64 = -0.02;
pub const MIN_GENERATION_INTERVAL_MS: u64 = 60_000;
pub const MAX_MUTATION_DELTA: f64 = 0.20;
pub const MAX_VERIFICATION_EXTENSIONS: u32 = 2;

// --- Fitness Evaluator ---
pub const FITNESS_TREND_WINDOW: usize = 10;
pub const STABILITY_TOLERANCE: f64 = 0.02;
pub const VOLATILITY_THRESHOLD: f64 = 0.05;

/// Default dimension weights for 12D fitness scoring.
/// Sum: 1.00.
pub const FITNESS_DIMENSION_WEIGHTS: [f64; 12] = [
    0.02,  // D0:  service_id
    0.01,  // D1:  port
    0.03,  // D2:  tier
    0.05,  // D3:  dependency_count
    0.04,  // D4:  agent_count
    0.02,  // D5:  protocol
    0.20,  // D6:  health_score
    0.18,  // D7:  uptime
    0.15,  // D8:  synergy
    0.12,  // D9:  latency
    0.10,  // D10: error_rate
    0.08,  // D11: temporal_context
];
```

---

## 9. Dimension Weight Defaults

| Index | Dimension Name | Weight | Priority | Rationale |
|-------|---------------|--------|----------|-----------|
| D0 | `service_id` | 0.02 | Low | Identification only; no quality signal |
| D1 | `port` | 0.01 | Low | Identification only; no quality signal |
| D2 | `tier` | 0.03 | Low | Criticality indicator, indirect signal |
| D3 | `dependency_count` | 0.05 | Low-Moderate | Complexity proxy |
| D4 | `agent_count` | 0.04 | Low-Moderate | Resource allocation proxy |
| D5 | `protocol` | 0.02 | Low | Communication mode; no quality signal |
| D6 | `health_score` | 0.20 | **HIGH** | Primary fitness indicator |
| D7 | `uptime` | 0.18 | **HIGH** | Availability is critical |
| D8 | `synergy` | 0.15 | **HIGH** | Cross-system cooperation health |
| D9 | `latency` | 0.12 | Moderate-High | Performance quality signal |
| D10 | `error_rate` | 0.10 | Moderate-High | Reliability quality signal |
| D11 | `temporal_context` | 0.08 | Moderate | Time relevance context |
| | **Sum** | **1.00** | | |

### Weight Distribution Visualization

```
D6  health_score     [====================] 0.20
D7  uptime           [==================  ] 0.18
D8  synergy          [===============     ] 0.15
D9  latency          [============        ] 0.12
D10 error_rate       [==========          ] 0.10
D11 temporal_context [========            ] 0.08
D3  dependency_count [=====               ] 0.05
D4  agent_count      [====                ] 0.04
D2  tier             [===                 ] 0.03
D0  service_id       [==                  ] 0.02
D5  protocol         [==                  ] 0.02
D1  port             [=                   ] 0.01
```

---

## 10. Cross-Layer Type Dependencies

### 10.1 External Type Imports

| L7 Type | Depends On | Source Module | Access Pattern |
|---------|-----------|---------------|----------------|
| `LogCorrelator.ingest_event()` | `EventRecord` | `m4_integration/event_bus.rs` (M23) | Read-only input parameter |
| `FitnessEvaluator.evaluate()` | `Tensor12D` | `lib.rs` | Read-only input parameter |
| `EvolutionChamber` mutations | `HebbianPathway` | `m5_learning/mod.rs` (M25) | Parameter adjustment target |
| `EmergenceDetector` escalation | `EscalationTier` | `lib.rs` | Severity mapping |
| All L7 modules | `Error`, `Result` | `m1_foundation/error.rs` (M01) | Error propagation |
| All L7 modules | `DateTime<Utc>` | `chrono` (external crate) | Timestamping |
| All L7 locks | `RwLock` | `parking_lot` (external crate) | Concurrency |
| `EvolutionChamber.generation` | `AtomicU64` | `std::sync::atomic` (stdlib) | Lock-free counter |
| All ID generation | `Uuid` | `uuid` (external crate) | UUID v4 generation |
| All channel payloads | `Serialize`, `Deserialize` | `serde` (external crate) | JSON serialization |

### 10.2 Dependency Direction

```
External Crates          Existing Engine (L1-L6)         L7 Observer
+-----------+            +-------------------+            +-----------+
| chrono    |<-----------| DateTime<Utc>     |<-----------| All L7    |
| parking_lot|<----------| RwLock            |<-----------| All L7    |
| uuid      |<-----------| Uuid              |<-----------| All L7    |
| serde     |<-----------| Serialize/Deser.  |<-----------| Payloads  |
+-----------+            +-------------------+            +-----------+
                         | M01: Error, Result|<-----------| All L7    |
                         | M23: EventRecord  |<-----------| M37       |
                         | M23: EventBus     |<-----------| mod.rs    |
                         | lib.rs: Tensor12D |<-----------| Fitness   |
                         | M25: HebbianPath  |<-----------| M39       |
                         | lib.rs: EscalTier |<-----------| M38       |
                         +-------------------+            +-----------+
```

### 10.3 Zero New Dependencies on L7

L7 introduces **zero new dependencies** from L1-L6 to L7. The dependency arrow is strictly one-directional: L7 depends on L1-L6, never the reverse. This ensures L7 can be added or removed without modifying any existing module.

| Direction | Allowed | Rationale |
|-----------|---------|-----------|
| L7 -> L1-L6 | Yes (read-only) | Observer pattern: subscribe and read |
| L1-L6 -> L7 | **No** | Zero coupling: engine operates identically without L7 |
| L7 -> External crates | Yes | Same crates already used by L1-L6 |

---

## 11. Trait Implementations Required

### 11.1 Enum Trait Requirements

| Type | `Clone` | `Copy` | `Debug` | `PartialEq` | `Eq` | `Hash` | `Serialize` | `Deserialize` | Notes |
|------|---------|--------|---------|-------------|------|--------|-------------|---------------|-------|
| `CorrelationType` | Yes | No | Yes | Yes | Yes | Yes | Yes | Yes | HashMap key in `CorrelationStats` |
| `EmergentBehavior` | Yes | No | Yes | No | No | No | Yes | Yes | Variant data prevents `Copy`/`Eq` (f64 fields) |
| `EmergenceSeverity` | Yes | Yes | Yes | Yes | Yes | Yes | Yes | Yes | HashMap key in `EmergenceStats` |
| `MutationType` | Yes | No | Yes | No | No | No | Yes | Yes | Variant data prevents `Copy`/`Eq` |
| `MutationTrigger` | Yes | No | Yes | No | No | No | Yes | Yes | String field prevents `Copy` |
| `SystemState` | Yes | Yes | Yes | Yes | Yes | No | Yes | Yes | Lightweight status enum |
| `MutationVerdict` | Yes | Yes | Yes | Yes | Yes | No | No | No | Internal decision enum |
| `FitnessTrend` | Yes | Yes | Yes | Yes | Yes | No | Yes | Yes | Lightweight status enum |
| `MonitorState` | Yes | Yes | Yes | Yes | Yes | No | No | No | Internal state machine |
| `RalphPhase` | Yes | Yes | Yes | Yes | Yes | No | Yes | Yes | State machine phase |
| `MutationStatus` | Yes | Yes | Yes | Yes | Yes | No | Yes | Yes | Lifecycle status |
| `ObserverSource` | Yes | Yes | Yes | Yes | Yes | No | No | No | Internal bus routing |
| `ObserverMessageType` | Yes | Yes | Yes | Yes | Yes | No | No | No | Internal bus routing |

### 11.2 Struct Trait Requirements

| Type | `Clone` | `Debug` | `Default` | `Serialize` | `Deserialize` | Notes |
|------|---------|---------|-----------|-------------|---------------|-------|
| `ObserverConfig` | Yes | Yes | Yes | Yes | Yes | TOML config deserialization |
| `LogCorrelatorConfig` | Yes | Yes | Yes | Yes | Yes | TOML config deserialization |
| `EmergenceConfig` | Yes | Yes | Yes | Yes | Yes | TOML config deserialization |
| `EvolutionConfig` | Yes | Yes | Yes | Yes | Yes | TOML config deserialization |
| `FitnessConfig` | Yes | Yes | Yes | Yes | Yes | TOML config deserialization |
| `ObservationReport` | Yes | Yes | No | Yes | Yes | EventBus payload |
| `CorrelatedEvent` | Yes | Yes | No | Yes | Yes | EventBus payload |
| `CorrelationLink` | Yes | Yes | No | Yes | Yes | EventBus payload |
| `CorrelationWindow` | Yes | Yes | No | No | No | Internal only |
| `RecurringPattern` | Yes | Yes | No | No | No | Internal only |
| `EmergenceRecord` | Yes | Yes | No | Yes | Yes | EventBus payload |
| `EmergenceMonitor` | Yes | Yes | No | No | No | Internal only |
| `MutationRecord` | Yes | Yes | No | Yes | Yes | EventBus payload |
| `ActiveMutation` | Yes | Yes | No | No | No | Internal only |
| `FitnessSnapshot` | Yes | Yes | No | Yes | Yes | History storage |
| `FitnessReport` | Yes | Yes | No | Yes | Yes | EventBus payload |
| `RalphState` | Yes | Yes | No | No | No | Internal only |
| `ObserverMessage` | Yes | Yes | No | No | No | Internal bus message |
| `ObserverMetrics` | Yes | Yes | Yes | No | No | Aggregate counters |
| `CorrelationStats` | Yes | Yes | Yes | No | No | Aggregate counters |
| `EmergenceStats` | Yes | Yes | Yes | No | No | Aggregate counters |
| `ObserverBusStats` | Yes | Yes | Yes | No | No | Aggregate counters |
| `DetectorMetrics` | Yes | Yes | Yes | No | No | Aggregate counters |
| `ChamberMetrics` | Yes | Yes | Yes | No | No | Aggregate counters |

### 11.3 Derive Macro Summary

```rust
// --- Channel payload types (Serialize + Deserialize) ---
#[derive(Clone, Debug, Serialize, Deserialize)]

// --- Internal-only types ---
#[derive(Clone, Debug)]

// --- Lightweight enums (Copy-safe) ---
#[derive(Clone, Copy, Debug, PartialEq, Eq)]

// --- HashMap key enums ---
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]

// --- Serialized enums with data variants ---
#[derive(Clone, Debug, Serialize, Deserialize)]

// --- Aggregate counters ---
#[derive(Clone, Debug, Default)]

// --- Config types ---
#[derive(Clone, Debug, Serialize, Deserialize)]
```

---

## 12. Type Inventory Summary

### 12.1 By Category

| Category | Count | Types |
|----------|-------|-------|
| **Structs** | 22 | `ObserverLayer`, `ObserverConfig`, `ObservationReport`, `ObserverMetrics`, `ObserverBus`, `ObserverBusStats`, `ObserverMessage`, `LogCorrelator`, `CorrelatedEvent`, `CorrelationLink`, `CorrelationWindow`, `RecurringPattern`, `LogCorrelatorConfig`, `CorrelationStats`, `EmergenceDetector`, `EmergenceRecord`, `EmergenceMonitor`, `EmergenceConfig`, `EmergenceStats`, `EvolutionChamber`, `MutationRecord`, `ActiveMutation` |
| **Structs** (cont.) | +8 | `FitnessSnapshot`, `RalphState`, `EvolutionConfig`, `ChamberMetrics`, `DetectorMetrics`, `FitnessEvaluator`, `FitnessReport`, `FitnessConfig` |
| **Enums** | 10 | `CorrelationType`, `EmergentBehavior`, `EmergenceSeverity`, `MonitorState`, `MutationType`, `MutationTrigger`, `SystemState`, `MutationVerdict`, `FitnessTrend`, `RalphPhase` |
| **Enums** (cont.) | +2 | `MutationStatus`, `ObserverSource`, `ObserverMessageType` |
| **Type Aliases** | 3 | `CorrelationHandler`, `EmergenceHandler`, `EvolutionHandler` |
| **Constants** | 25 | See [Section 8](#8-constants-reference-table) |

### 12.2 By Module

| Module | File | Structs | Enums | Type Aliases | Constants |
|--------|------|---------|-------|-------------|-----------|
| Layer Coordinator | `mod.rs` | 4 | 0 | 0 | 0 |
| Observer Bus | `observer_bus.rs` | 3 | 2 | 3 | 0 |
| M37 Log Correlator | `log_correlator.rs` | 7 | 1 | 0 | 7 |
| M38 Emergence Detector | `emergence_detector.rs` | 5 | 3 | 0 | 6 |
| M39 Evolution Chamber | `evolution_chamber.rs` | 6 | 6 | 0 | 9 |
| Fitness Evaluator | `fitness.rs` | 3 | 1 | 0 | 3 + 1 array |
| **Total** | **6 files** | **28** | **13** | **3** | **25 + 1** |

### 12.3 Estimated Memory Footprint (Per Instance)

| Type | Estimated Size | Bounded By | Notes |
|------|---------------|-----------|-------|
| `CorrelatedEvent` | ~400 bytes | `MAX_BUFFER_SIZE` (10,000) | Largest per-event type |
| `CorrelationLink` | ~80 bytes | `MAX_CORRELATIONS_PER_EVENT` (20) | Per-link overhead |
| `CorrelationWindow` | ~200 bytes + events | Active window count | Transient |
| `RecurringPattern` | ~300 bytes | Unbounded (monitored) | Persistent |
| `EmergenceRecord` | ~500 bytes | `EMERGENCE_HISTORY_CAPACITY` (1,000) | Largest per-record type |
| `MutationRecord` | ~400 bytes | `MUTATION_HISTORY_CAPACITY` (1,000) | Full lifecycle data |
| `FitnessSnapshot` | ~120 bytes | `FITNESS_HISTORY_CAPACITY` (500) | Compact |
| `FitnessReport` | ~200 bytes | History capacity | Full report |

---

## 13. Version History

| Version | Date | Changes |
|---------|------|---------|
| 1.0.0 | 2026-01-29 | Initial specification: 22 structs, 10 enums, 3 type aliases, 25 constants across 6 source files. Complete type definitions for all L7 Observer Layer components (M37 Log Correlator, M38 Emergence Detector, M39 Evolution Chamber, Observer Bus, Fitness Evaluator, Layer Coordinator). |

---

## Navigation

| Direction | Link |
|-----------|------|
| Up (Spec Index) | [INDEX.md](INDEX.md) |
| Up (Parent Specs) | [ai_specs/INDEX.md](../INDEX.md) |
| Prev | [EVENT_CHANNEL_SPEC.md](EVENT_CHANNEL_SPEC.md) |
| Layer Spec | [OBSERVER_LAYER_SPEC.md](OBSERVER_LAYER_SPEC.md) |
| Log Correlator Spec | [LOG_CORRELATOR_SPEC.md](LOG_CORRELATOR_SPEC.md) |
| Companion Docs | [ai_docs/evolution_chamber_ai_docs/INDEX.md](../../ai_docs/evolution_chamber_ai_docs/INDEX.md) |
| Tensor Spec | [ai_specs/TENSOR_SPEC.md](../TENSOR_SPEC.md) |
| NAM Spec | [ai_specs/NAM_SPEC.md](../NAM_SPEC.md) |
| EventBus (M23) | [ai_docs/modules/M23_EVENT_BUS.md](../../ai_docs/modules/M23_EVENT_BUS.md) |

---

*The Maintenance Engine v1.0.0 | L7 Observer Layer Type Definitions Specification*
*Last Updated: 2026-01-29*
