# L7 Observer Layer Architecture Specification

> **Complete L7 Observer Layer Specification** | The Maintenance Engine v1.0.0

```json
{"v":"1.0.0","type":"LAYER_SPEC","layer":7,"name":"Observer","modules":3,"utilities":2,"estimated_loc":6600,"estimated_tests":300,"channels":3,"databases":1}
```

**Version:** 1.0.0
**Status:** SPECIFICATION
**Related:** [INDEX.md](INDEX.md) | [LAYER_SPEC.md](../LAYER_SPEC.md) | [TENSOR_SPEC.md](../TENSOR_SPEC.md) | [NAM_SPEC.md](../NAM_SPEC.md) | [ai_docs/evolution_chamber_ai_docs/L07_OBSERVER_LAYER.md](../../ai_docs/evolution_chamber_ai_docs/L07_OBSERVER_LAYER.md)

---

## Table of Contents

| # | Section | Description |
|---|---------|-------------|
| 1 | [Architecture Overview](#1-architecture-overview) | Design philosophy and integration model |
| 2 | [Module Inventory](#2-module-inventory) | Complete module and utility listing |
| 3 | [Dependency Graph](#3-dependency-graph) | Internal and external dependencies |
| 4 | [EventBus Integration](#4-eventbus-integration) | Channel subscriptions and publications |
| 5 | [Channel Specifications](#5-channel-specifications) | Payload schemas and rate targets |
| 6 | [Rust Type Definitions](#6-rust-type-definitions) | Core types for all L7 components |
| 7 | [Configuration Specification](#7-configuration-specification) | TOML config parameters |
| 8 | [Lock Ordering Constraints](#8-lock-ordering-constraints) | Concurrency safety rules |
| 9 | [Error Handling](#9-error-handling) | Failure modes and recovery |
| 10 | [Performance Budget](#10-performance-budget) | Latency and throughput targets |
| 11 | [Quality Requirements](#11-quality-requirements) | Code quality gates |
| 12 | [NAM Compliance Contributions](#12-nam-compliance-contributions) | R1, R2, R4 contributions |
| 13 | [Security Considerations](#13-security-considerations) | Read-only observer, mutation safety, PBFT gate, bounded resources |
| 14 | [Version History](#14-version-history) | Changelog |

---

## 1. Architecture Overview

### 1.1 Design Philosophy

L7 is a **cross-cutting observer layer** that monitors L1-L6 without modifying them. It follows the Observer Pattern: subscribe-only access to existing EventBus channels, with zero coupling to existing module internals. L7 is designed to be entirely optional -- the engine operates identically with or without it.

### 1.2 Core Principles

| Principle | Implementation |
|-----------|---------------|
| **Non-invasive** | Subscribe-only access; never writes to L1-L6 state |
| **Optional integration** | `Option<ObserverLayer>` in `MaintenanceEngine` struct |
| **Zero cost when disabled** | No allocations, no subscriptions, no event processing |
| **Fail-silent** | L7 errors are logged and counted, never propagated |
| **Cross-cutting** | Observes all 6 layers through unified EventBus subscriptions |
| **Self-observing** | Evolution Chamber monitors its own mutation effectiveness (NAM R1) |

### 1.3 Integration Model

```
+------------------------------------------------------------------+
|                  EXISTING ENGINE (L1-L6)                          |
|                                                                  |
|  L1 Foundation  L2 Services  L3 Core Logic                      |
|  L4 Integration L5 Learning  L6 Consensus                       |
|       |              |             |                             |
|       +------+-------+------+------+                             |
|              |              |                                    |
|         [ EventBus M23 ]   |                                    |
|              |              |                                    |
+--------------+--------------+------------------------------------+
               |
          SUBSCRIBES (6 channels)
               |
+--------------v---------------------------------------------------+
|                  L7: OBSERVER LAYER                               |
|                                                                  |
|  +-----------+   +-------------+   +------------------+          |
|  |    M37    |   |     M38     |   |       M39        |          |
|  |    Log    |-->|  Emergence  |-->|    Evolution      |          |
|  | Correlator|   |  Detector   |   |     Chamber      |          |
|  +-----------+   +-------------+   +------------------+          |
|       |               |                    |                     |
|       +-------+-------+--------------------+                     |
|               |                                                  |
|        [Observer Bus]                                            |
|               |                                                  |
|  +------------+---------------+                                  |
|  |                            |                                  |
|  v                            v                                  |
|  Fitness Evaluator       Layer Coordinator (mod.rs)              |
|                                                                  |
|  PUBLISHES TO: observation, emergence, evolution                 |
+-----------------------------------------------------------------+
```

### 1.4 Engine Integration Point

```rust
/// MaintenanceEngine with optional L7 observer
pub struct MaintenanceEngine {
    // L1-L6 fields (existing, unchanged)
    foundation: Foundation,
    services: Services,
    core_logic: CoreLogic,
    integration: Integration,
    learning: Learning,
    consensus: Consensus,

    // L7: Observer Layer (optional, zero-cost when None)
    observer: Option<ObserverLayer>,
}
```

---

## 2. Module Inventory

### 2.1 Module Table

| ID | Module | File | Est. LOC | Est. Tests | Dependencies | Purpose |
|----|--------|------|----------|------------|-------------|---------|
| M37 | Log Correlator | `log_correlator.rs` | ~1,400 | 50 | M23 EventBus | Cross-layer event correlation, temporal windowing, pattern detection |
| M38 | Emergence Detector | `emergence_detector.rs` | ~1,500 | 50 | M37 | Cascade analysis, synergy delta detection, resonance cycle identification |
| M39 | Evolution Chamber | `evolution_chamber.rs` | ~1,800 | 50 | M38, Fitness | RALPH loop meta-learning, mutation generation, verification, rollback |
| -- | Observer Bus | `observer_bus.rs` | ~500 | 50 | Internal | Internal L7 pub/sub connecting M37, M38, M39 |
| -- | Fitness Evaluator | `fitness.rs` | ~800 | 50 | Tensor12D (lib.rs) | 12D tensor fitness scoring, trend analysis, stability evaluation |
| -- | Layer Coordinator | `mod.rs` | ~600 | 50 | All L7 modules | Lifecycle management, EventBus wiring, shutdown coordination |

### 2.2 Totals

| Metric | Value |
|--------|-------|
| **Core Modules** | 3 (M37, M38, M39) |
| **Utility Modules** | 2 (Observer Bus, Fitness Evaluator) |
| **Coordination** | 1 (mod.rs) |
| **Source Files** | 6 |
| **Total Est. LOC** | ~6,600 |
| **Total Est. Tests** | ~300 (50 per module) |

### 2.3 File Layout

```
src/
└── m7_observer/                   # L7: Observer Layer (~6,600 LOC)
    ├── mod.rs                     # Layer Coordinator (~600 LOC, 50 tests)
    ├── log_correlator.rs          # M37: Log Correlator (~1,400 LOC, 50 tests)
    ├── emergence_detector.rs      # M38: Emergence Detector (~1,500 LOC, 50 tests)
    ├── evolution_chamber.rs       # M39: Evolution Chamber (~1,800 LOC, 50 tests)
    ├── observer_bus.rs            # Observer Bus utility (~500 LOC, 50 tests)
    └── fitness.rs                 # Fitness Evaluator utility (~800 LOC, 50 tests)
```

---

## 3. Dependency Graph

### 3.1 Internal Dependencies

```
                    EventBus (M23)
                         |
                    +----+----+
                    |         |
                    v         |
              LogCorrelator   |
                (M37)         |
                    |         |
                    v         |
              ObserverBus <---+
                    |
            +-------+----------+
            |       |          |
            v       v          v
      Emergence   Evolution   External
      Detector    Chamber     (M23 bridge)
        (M38)      (M39)
                    |
                    v
              FitnessEvaluator
```

### 3.2 External Dependencies (Crates)

| Crate | Version | Usage |
|-------|---------|-------|
| `parking_lot` | 0.12 | RwLock for all L7 mutable state |
| `chrono` | 0.4 | Timestamps for events, correlation windows |
| `uuid` | 1.x | Unique IDs for events, mutations, generations |
| `serde` | 1.x | Serialization of channel payloads |
| `serde_json` | 1.x | JSON serialization for EventBus publishing |
| `tracing` | 0.1 | Structured logging for observer operations |

### 3.3 Internal Dependencies (Existing Modules)

| Module | Dependency | Access Pattern |
|--------|-----------|----------------|
| M23 EventBus | Subscribe to channels | Read-only (subscribe + receive) |
| M23 EventBus | Publish to new channels | Write (publish typed JSON) |
| M01 Error | Error enum | Use existing Error variants |
| lib.rs | Tensor12D | Read struct for fitness evaluation |

---

## 4. EventBus Integration

### 4.1 Subscribed Channels (6 Existing)

L7 subscribes as `"l7_observer"` to all 6 existing EventBus channels. These subscriptions are read-only; L7 never publishes to existing channels.

| Channel | Source Layer | Event Types Consumed | L7 Consumer |
|---------|-------------|---------------------|-------------|
| `health` | L2 Services | HealthCheck, ServiceDown, ServiceUp | M37, M38 |
| `remediation` | L3 Core Logic | ActionProposed, ActionExecuted, OutcomeRecorded | M37, M38, M39 |
| `learning` | L5 Learning | PathwayStrengthened, PatternRecognized, Pruned | M38, M39 |
| `consensus` | L6 Consensus | ProposalSubmitted, VoteCast, ConsensusAchieved | M37, M38 |
| `integration` | L4 Integration | BridgeEvent, ServiceConnected, ServiceDisconnected | M37 |
| `metrics` | L1 Foundation | MetricRecorded, ThresholdBreached, AnomalyDetected | M37, M38 |

### 4.2 Published Channels (3 New)

L7 creates and owns 3 new EventBus channels. These channels carry typed JSON payloads published by specific L7 modules.

| Channel | Owner | Payload Type | Rate Profile | Consumers |
|---------|-------|-------------|--------------|-----------|
| `observation` | M37 LogCorrelator | CorrelatedEvent, CorrelationLink[] | High (~100/s during active correlation) | External dashboards, M23 subscribers |
| `emergence` | M38 EmergenceDetector | EmergenceRecord | Low (~1-10/min, bursty during incidents) | L3 Escalation, human operators, M39 |
| `evolution` | M39 EvolutionChamber | MutationRecord, FitnessReport | Very low (~1/min per generation) | Audit log, human operators, L5/L6 |

### 4.3 Subscription Registration

```rust
/// Register L7 as a subscriber on all 6 existing channels
fn register_subscriptions(event_bus: &EventBus) -> Result<Vec<String>> {
    let channels = ["health", "remediation", "learning",
                    "consensus", "integration", "metrics"];
    let mut sub_ids = Vec::with_capacity(channels.len());
    for channel in &channels {
        let sub_id = event_bus.subscribe(channel, "l7_observer", None)?;
        sub_ids.push(sub_id);
    }
    Ok(sub_ids)
}

/// Create 3 new L7-owned channels
fn create_observer_channels(event_bus: &EventBus) -> Result<()> {
    event_bus.create_channel("observation")?;
    event_bus.create_channel("emergence")?;
    event_bus.create_channel("evolution")?;
    Ok(())
}
```

---

## 5. Channel Specifications

### 5.1 `observation` Channel

| Property | Value |
|----------|-------|
| **Source** | M37 LogCorrelator |
| **Payload** | Serialized `CorrelatedEvent` + `CorrelationLink[]` |
| **Rate** | High (~100/s during active correlation) |
| **Consumers** | External dashboards, M23 subscribers |
| **Delivery** | Best-effort (non-blocking publish) |
| **Buffer** | 1,000 events (rolling) |

**Payload Schema:**

```rust
/// A correlated event published to the observation channel
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ObservationPayload {
    /// Unique observation ID
    pub observation_id: String,
    /// The primary correlated event
    pub correlated_event: CorrelatedEvent,
    /// Links discovered during correlation
    pub links: Vec<CorrelationLink>,
    /// Correlation confidence (0.0 - 1.0)
    pub confidence: f64,
    /// Timestamp of correlation discovery
    pub timestamp: DateTime<Utc>,
    /// Source layers involved
    pub source_layers: Vec<u8>,
}
```

### 5.2 `emergence` Channel

| Property | Value |
|----------|-------|
| **Source** | M38 EmergenceDetector |
| **Payload** | Serialized `EmergenceRecord` |
| **Rate** | Low (~1-10/min, bursty during incidents) |
| **Consumers** | L3 Escalation, human operators, M39 |
| **Delivery** | Guaranteed (retry up to 3 times) |
| **Buffer** | 100 events (rolling) |

**Payload Schema:**

```rust
/// An emergence event published to the emergence channel
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EmergencePayload {
    /// Unique emergence record ID
    pub emergence_id: String,
    /// The detected emergence record
    pub record: EmergenceRecord,
    /// Severity assessment (0.0 - 1.0)
    pub severity: f64,
    /// Whether this requires human attention
    pub requires_attention: bool,
    /// Timestamp of detection
    pub timestamp: DateTime<Utc>,
    /// Related observation IDs
    pub related_observations: Vec<String>,
}
```

### 5.3 `evolution` Channel

| Property | Value |
|----------|-------|
| **Source** | M39 EvolutionChamber |
| **Payload** | Serialized `MutationRecord` + `FitnessReport` |
| **Rate** | Very low (~1/min per generation) |
| **Consumers** | Audit log, human operators, L5/L6 |
| **Delivery** | Guaranteed (retry up to 3 times) |
| **Buffer** | 50 events (rolling) |

**Payload Schema:**

```rust
/// An evolution event published to the evolution channel
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EvolutionPayload {
    /// Unique mutation record ID
    pub mutation_id: String,
    /// Generation number
    pub generation: u64,
    /// The mutation record
    pub mutation: MutationRecord,
    /// Fitness report for this mutation
    pub fitness_report: FitnessReport,
    /// Whether the mutation was applied
    pub applied: bool,
    /// Whether the mutation was rolled back
    pub rolled_back: bool,
    /// Timestamp of evolution event
    pub timestamp: DateTime<Utc>,
}
```

---

## 6. Rust Type Definitions

### 6.1 Layer Coordinator Types

```rust
/// L7 Observer Layer -- top-level coordinator
pub struct ObserverLayer {
    /// M37: Cross-layer event correlation
    pub log_correlator: LogCorrelator,
    /// M38: Emergent behavior detection
    pub emergence_detector: EmergenceDetector,
    /// M39: RALPH-loop evolution
    pub evolution_chamber: EvolutionChamber,
    /// Internal L7 event bus
    pub observer_bus: ObserverBus,
    /// 12D tensor fitness scoring
    pub fitness_evaluator: FitnessEvaluator,
    /// Observer configuration
    pub config: ObserverConfig,
    /// Layer-level metrics
    pub metrics: ObserverMetrics,
}
```

### 6.2 M37 Log Correlator Types

```rust
/// M37: Cross-layer event correlation engine
pub struct LogCorrelator {
    /// Active correlation windows (keyed by window ID)
    windows: RwLock<HashMap<String, CorrelationWindow>>,
    /// Completed correlations (bounded ring buffer)
    correlations: RwLock<Vec<CorrelatedEvent>>,
    /// Configuration
    config: LogCorrelatorConfig,
    /// Metrics
    metrics: CorrelatorMetrics,
}

/// A temporal correlation window
#[derive(Clone, Debug)]
pub struct CorrelationWindow {
    /// Window ID
    pub id: String,
    /// Window start time
    pub start: DateTime<Utc>,
    /// Window end time (start + window_size_ms)
    pub end: DateTime<Utc>,
    /// Events captured in this window
    pub events: Vec<IngestedEvent>,
    /// Discovered links
    pub links: Vec<CorrelationLink>,
    /// Whether this window has been finalized
    pub finalized: bool,
}

/// An event ingested from the EventBus for correlation
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IngestedEvent {
    /// Original EventBus event ID
    pub event_id: String,
    /// Source channel
    pub channel: String,
    /// Event type tag
    pub event_type: String,
    /// JSON payload
    pub payload: String,
    /// Source layer (1-6)
    pub source_layer: u8,
    /// Ingestion timestamp
    pub ingested_at: DateTime<Utc>,
}

/// A correlated event with discovered links
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CorrelatedEvent {
    /// Correlation ID
    pub id: String,
    /// Primary event
    pub primary_event: IngestedEvent,
    /// Related events
    pub related_events: Vec<IngestedEvent>,
    /// Correlation links
    pub links: Vec<CorrelationLink>,
    /// Overall correlation confidence (0.0 - 1.0)
    pub confidence: f64,
    /// Discovery timestamp
    pub discovered_at: DateTime<Utc>,
}

/// A link between two correlated events
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CorrelationLink {
    /// Source event ID
    pub source_event_id: String,
    /// Target event ID
    pub target_event_id: String,
    /// Link type (temporal, causal, semantic)
    pub link_type: CorrelationLinkType,
    /// Link confidence (0.0 - 1.0)
    pub confidence: f64,
    /// Temporal offset in milliseconds
    pub temporal_offset_ms: i64,
}

/// Types of correlation links
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum CorrelationLinkType {
    /// Events occurred within temporal tolerance
    Temporal,
    /// Events share causal relationship (same service, cascading errors)
    Causal,
    /// Events share semantic similarity (same event type, related services)
    Semantic,
    /// Events form a recurring pattern
    Recurring,
}
```

### 6.3 M38 Emergence Detector Types

```rust
/// M38: Emergent behavior detection engine
pub struct EmergenceDetector {
    /// Detection history (bounded ring buffer)
    history: RwLock<Vec<EmergenceRecord>>,
    /// Active cascade tracking
    active_cascades: RwLock<Vec<CascadeTracker>>,
    /// Configuration
    config: EmergenceDetectorConfig,
    /// Metrics
    metrics: DetectorMetrics,
}

/// A detected emergent behavior record
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EmergenceRecord {
    /// Unique record ID
    pub id: String,
    /// Type of emergence detected
    pub emergence_type: EmergenceType,
    /// Detection confidence (0.0 - 1.0)
    pub confidence: f64,
    /// Severity assessment (0.0 - 1.0)
    pub severity: f64,
    /// Source correlation IDs
    pub source_correlations: Vec<String>,
    /// Affected layers
    pub affected_layers: Vec<u8>,
    /// Affected services
    pub affected_services: Vec<String>,
    /// Description of the emergent behavior
    pub description: String,
    /// Detection timestamp
    pub detected_at: DateTime<Utc>,
    /// Recommended action (if any)
    pub recommended_action: Option<String>,
}

/// Types of emergent behavior
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum EmergenceType {
    /// Error cascade across multiple layers
    CascadeFailure,
    /// Significant synergy score delta
    SynergyShift,
    /// Repeating oscillatory pattern
    ResonanceCycle,
    /// System settling into attractor state
    AttractorFormation,
    /// Phase transition in system behavior
    PhaseTransition,
    /// Positive emergent property (self-healing, etc.)
    BeneficialEmergence,
}

/// Tracks an active cascade for depth analysis
#[derive(Clone, Debug)]
pub struct CascadeTracker {
    /// Cascade ID
    pub id: String,
    /// Cascade origin event
    pub origin_event_id: String,
    /// Current cascade depth
    pub depth: u32,
    /// Events in the cascade chain
    pub chain: Vec<String>,
    /// Layers touched by the cascade
    pub layers_touched: Vec<u8>,
    /// Cascade start time
    pub started_at: DateTime<Utc>,
    /// Whether this cascade is still active
    pub active: bool,
}
```

### 6.4 M39 Evolution Chamber Types

```rust
/// M39: RALPH loop evolution engine
pub struct EvolutionChamber {
    /// Current generation number
    generation: RwLock<u64>,
    /// Active mutations (bounded by max_concurrent_mutations)
    active_mutations: RwLock<Vec<ActiveMutation>>,
    /// Mutation history (bounded ring buffer)
    mutation_history: RwLock<Vec<MutationRecord>>,
    /// RALPH loop state
    ralph_state: RwLock<RalphState>,
    /// Configuration
    config: EvolutionChamberConfig,
    /// Metrics
    metrics: ChamberMetrics,
}

/// A record of a completed mutation
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MutationRecord {
    /// Unique mutation ID
    pub id: String,
    /// Generation this mutation belongs to
    pub generation: u64,
    /// RALPH phase that generated this mutation
    pub source_phase: RalphPhase,
    /// Parameter being mutated
    pub target_parameter: String,
    /// Original value
    pub original_value: f64,
    /// Mutated value
    pub mutated_value: f64,
    /// Delta applied
    pub delta: f64,
    /// Fitness before mutation
    pub fitness_before: f64,
    /// Fitness after mutation
    pub fitness_after: f64,
    /// Whether the mutation was applied permanently
    pub applied: bool,
    /// Whether the mutation was rolled back
    pub rolled_back: bool,
    /// Mutation timestamp
    pub timestamp: DateTime<Utc>,
    /// Verification duration in milliseconds
    pub verification_ms: u64,
}

/// An active in-flight mutation
#[derive(Clone, Debug)]
pub struct ActiveMutation {
    /// Mutation ID
    pub id: String,
    /// Generation number
    pub generation: u64,
    /// Target parameter
    pub target_parameter: String,
    /// Original value (for rollback)
    pub original_value: f64,
    /// Applied value
    pub applied_value: f64,
    /// When the mutation was applied
    pub applied_at: DateTime<Utc>,
    /// Verification deadline
    pub verification_deadline: DateTime<Utc>,
    /// Current status
    pub status: MutationStatus,
}

/// Status of an in-flight mutation
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum MutationStatus {
    /// Mutation proposed, awaiting application
    Proposed,
    /// Mutation applied, under verification
    Verifying,
    /// Mutation verified and accepted
    Accepted,
    /// Mutation rejected, rollback complete
    RolledBack,
    /// Mutation failed during application
    Failed,
}

/// RALPH loop phases
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum RalphPhase {
    /// Recognize: Identify patterns from M38 emergence data
    Recognize,
    /// Analyze: Evaluate patterns against fitness landscape
    Analyze,
    /// Learn: Extract mutation candidates from analysis
    Learn,
    /// Propose: Generate concrete parameter mutations
    Propose,
    /// Harvest: Collect results and update fitness history
    Harvest,
}

/// Current state of the RALPH loop
#[derive(Clone, Debug)]
pub struct RalphState {
    /// Current phase
    pub current_phase: RalphPhase,
    /// Current cycle number
    pub cycle_number: u64,
    /// Last cycle start time
    pub cycle_started_at: Option<DateTime<Utc>>,
    /// Last cycle completion time
    pub cycle_completed_at: Option<DateTime<Utc>>,
    /// Number of mutations proposed this cycle
    pub mutations_proposed: u32,
    /// Number of mutations applied this cycle
    pub mutations_applied: u32,
    /// Whether the loop is paused
    pub paused: bool,
}
```

### 6.5 Fitness Evaluator Types

```rust
/// 12D tensor fitness evaluator
pub struct FitnessEvaluator {
    /// Fitness history (bounded ring buffer)
    history: RwLock<Vec<FitnessSnapshot>>,
    /// Configuration
    config: FitnessConfig,
}

/// A fitness evaluation report
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FitnessReport {
    /// Report ID
    pub id: String,
    /// Evaluated tensor
    pub tensor: [f64; 12],
    /// Overall fitness score (0.0 - 1.0)
    pub overall_score: f64,
    /// Per-dimension scores
    pub dimension_scores: [f64; 12],
    /// Weighted dimension contributions
    pub weighted_contributions: [f64; 12],
    /// Trend direction (-1.0 declining to +1.0 improving)
    pub trend: f64,
    /// Stability score (0.0 volatile to 1.0 stable)
    pub stability: f64,
    /// Evaluation timestamp
    pub evaluated_at: DateTime<Utc>,
}

/// A historical fitness snapshot
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FitnessSnapshot {
    /// Snapshot timestamp
    pub timestamp: DateTime<Utc>,
    /// Overall fitness at this point
    pub fitness: f64,
    /// Tensor state
    pub tensor: [f64; 12],
    /// Generation number (if during evolution)
    pub generation: Option<u64>,
}

/// Dimension weights for fitness scoring
pub const FITNESS_DIMENSION_WEIGHTS: [f64; 12] = [
    0.05,  // D0: service_id (low -- identification only)
    0.02,  // D1: port (low -- identification only)
    0.08,  // D2: tier (moderate -- criticality matters)
    0.05,  // D3: deps (low-moderate)
    0.05,  // D4: agents (low-moderate)
    0.03,  // D5: protocol (low)
    0.20,  // D6: health (HIGH -- primary fitness indicator)
    0.15,  // D7: uptime (HIGH -- availability critical)
    0.15,  // D8: synergy (HIGH -- cross-system health)
    0.10,  // D9: latency (moderate-high)
    0.10,  // D10: error_rate (moderate-high)
    0.02,  // D11: temporal (low -- context only)
];
// Sum: 1.00
```

### 6.6 Observer Bus Types

```rust
/// Internal L7 pub/sub bus
pub struct ObserverBus {
    /// Internal channels (keyed by channel name)
    channels: RwLock<HashMap<String, Vec<ObserverMessage>>>,
    /// Message ID counter
    next_id: RwLock<u64>,
    /// Configuration
    config: ObserverBusConfig,
}

/// An internal observer message
#[derive(Clone, Debug)]
pub struct ObserverMessage {
    /// Message ID
    pub id: u64,
    /// Source module
    pub source: ObserverSource,
    /// Message type
    pub message_type: ObserverMessageType,
    /// JSON payload
    pub payload: String,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
}

/// Source module for observer messages
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObserverSource {
    /// M37 Log Correlator
    LogCorrelator,
    /// M38 Emergence Detector
    EmergenceDetector,
    /// M39 Evolution Chamber
    EvolutionChamber,
    /// Fitness Evaluator
    FitnessEvaluator,
    /// Layer Coordinator
    Coordinator,
}

/// Types of internal observer messages
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObserverMessageType {
    /// New correlation discovered
    CorrelationFound,
    /// Emergence behavior detected
    EmergenceDetected,
    /// Mutation proposed
    MutationProposed,
    /// Mutation result (applied/rolled back)
    MutationResult,
    /// Fitness evaluation complete
    FitnessEvaluated,
    /// RALPH phase transition
    PhaseTransition,
}
```

### 6.7 Configuration Types

```rust
/// Top-level observer configuration
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ObserverConfig {
    /// Whether L7 is enabled
    pub enabled: bool,
    /// Log correlator configuration
    pub log_correlator: LogCorrelatorConfig,
    /// Emergence detector configuration
    pub emergence_detector: EmergenceDetectorConfig,
    /// Evolution chamber configuration
    pub evolution_chamber: EvolutionChamberConfig,
    /// Fitness evaluator configuration
    pub fitness: FitnessConfig,
}

/// M37 Log Correlator configuration
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LogCorrelatorConfig {
    /// Correlation window size in milliseconds
    pub window_size_ms: u64,
    /// Maximum events buffered per window
    pub max_buffer_size: usize,
    /// Minimum confidence to emit a correlation
    pub min_correlation_confidence: f64,
    /// Minimum recurring count to flag a pattern
    pub min_recurring_count: u32,
    /// Temporal tolerance for "simultaneous" events
    pub temporal_tolerance_ms: u64,
}

/// M38 Emergence Detector configuration
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EmergenceDetectorConfig {
    /// Cascade depth before triggering alert
    pub cascade_depth_threshold: u32,
    /// Synergy score delta to trigger detection
    pub synergy_delta_threshold: f64,
    /// Minimum cycles for resonance detection
    pub resonance_min_cycles: u32,
    /// Maximum history entries retained
    pub history_capacity: usize,
    /// Detection interval in milliseconds
    pub detection_interval_ms: u64,
    /// Minimum confidence to emit an emergence record
    pub min_confidence: f64,
}

/// M39 Evolution Chamber configuration
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EvolutionChamberConfig {
    /// Maximum mutations running concurrently
    pub max_concurrent_mutations: u32,
    /// Time allowed for mutation verification in milliseconds
    pub mutation_verification_ms: u64,
    /// Maximum fitness snapshots retained (EvolutionConfig).
    /// This is M39's internal FitnessSnapshot buffer (default 500, ~8h at 1-min intervals).
    /// Distinct from FitnessConfig.history_capacity (200), which sizes the
    /// FitnessEvaluator's FitnessReport buffer.
    pub fitness_history_capacity: usize,
    /// Maximum mutation records retained
    pub mutation_history_capacity: usize,
    /// Minimum fitness improvement to auto-apply a mutation
    pub auto_apply_threshold: f64,
    /// Fitness decline that triggers automatic rollback
    pub rollback_threshold: f64,
    /// Minimum interval between generations in milliseconds
    pub min_generation_interval_ms: u64,
    /// Maximum parameter delta per mutation (clamp)
    pub max_mutation_delta: f64,
}

/// Fitness evaluator configuration
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FitnessConfig {
    /// Maximum snapshots retained
    pub history_capacity: usize,
    /// Number of snapshots for trend calculation
    pub trend_window: usize,
    /// Stability tolerance (std dev below this = stable)
    pub stability_tolerance: f64,
    /// Volatility threshold (std dev above this = volatile)
    pub volatility_threshold: f64,
}
```

### 6.8 Metrics Types

```rust
/// L7 layer-level metrics
#[derive(Clone, Debug, Default)]
pub struct ObserverMetrics {
    /// Total events ingested from EventBus
    pub events_ingested: u64,
    /// Total correlations discovered
    pub correlations_found: u64,
    /// Total emergence events detected
    pub emergences_detected: u64,
    /// Total mutations proposed
    pub mutations_proposed: u64,
    /// Total mutations applied
    pub mutations_applied: u64,
    /// Total mutations rolled back
    pub mutations_rolled_back: u64,
    /// Total RALPH cycles completed
    pub ralph_cycles: u64,
    /// Total L7 errors (logged, not propagated)
    pub observer_errors: u64,
    /// Average event ingestion latency (ms)
    pub avg_ingestion_latency_ms: f64,
    /// Average correlation latency (ms)
    pub avg_correlation_latency_ms: f64,
}

/// M37-specific metrics
#[derive(Clone, Debug, Default)]
pub struct CorrelatorMetrics {
    pub windows_created: u64,
    pub windows_finalized: u64,
    pub events_correlated: u64,
    pub links_discovered: u64,
    pub avg_window_events: f64,
}

/// M38-specific metrics
#[derive(Clone, Debug, Default)]
pub struct DetectorMetrics {
    pub detection_cycles: u64,
    pub cascades_tracked: u64,
    pub emergences_detected: u64,
    pub false_positives: u64,
    pub avg_detection_latency_ms: f64,
}

/// M39-specific metrics
#[derive(Clone, Debug, Default)]
pub struct ChamberMetrics {
    pub generations_completed: u64,
    pub mutations_total: u64,
    pub mutations_accepted: u64,
    pub mutations_rolled_back: u64,
    pub mutations_failed: u64,
    pub avg_fitness: f64,
    pub best_fitness: f64,
    pub avg_ralph_cycle_ms: f64,
}
```

---

## 7. Configuration Specification

### 7.1 TOML Configuration

```toml
[observer]
enabled = true

[observer.log_correlator]
window_size_ms = 5000
max_buffer_size = 10000
min_correlation_confidence = 0.6
min_recurring_count = 3
temporal_tolerance_ms = 500

[observer.emergence_detector]
cascade_depth_threshold = 3
synergy_delta_threshold = 0.15
resonance_min_cycles = 3
history_capacity = 1000
detection_interval_ms = 1000
min_confidence = 0.7

[observer.evolution_chamber]
max_concurrent_mutations = 3
mutation_verification_ms = 30000
fitness_history_capacity = 500
mutation_history_capacity = 1000
auto_apply_threshold = 0.10
rollback_threshold = -0.02
min_generation_interval_ms = 60000
max_mutation_delta = 0.20

[observer.fitness]
history_capacity = 200
trend_window = 10
stability_tolerance = 0.02
volatility_threshold = 0.05
```

### 7.2 Configuration Parameter Reference

#### observer.log_correlator

| Parameter | Type | Default | Range | Description |
|-----------|------|---------|-------|-------------|
| `window_size_ms` | u64 | 5000 | [1000, 30000] | Duration of each correlation window |
| `max_buffer_size` | usize | 10000 | [100, 100000] | Maximum events buffered per window |
| `min_correlation_confidence` | f64 | 0.6 | [0.0, 1.0] | Minimum confidence to emit correlation |
| `min_recurring_count` | u32 | 3 | [2, 100] | Minimum recurrences to flag pattern |
| `temporal_tolerance_ms` | u64 | 500 | [10, 5000] | Time tolerance for "simultaneous" events |

#### observer.emergence_detector

| Parameter | Type | Default | Range | Description |
|-----------|------|---------|-------|-------------|
| `cascade_depth_threshold` | u32 | 3 | [2, 10] | Cascade depth to trigger alert |
| `synergy_delta_threshold` | f64 | 0.15 | [0.01, 0.50] | Synergy score change to trigger detection |
| `resonance_min_cycles` | u32 | 3 | [2, 20] | Minimum cycles for resonance detection |
| `history_capacity` | usize | 1000 | [100, 10000] | Maximum emergence records retained |
| `detection_interval_ms` | u64 | 1000 | [100, 10000] | Interval between detection cycles |
| `min_confidence` | f64 | 0.7 | [0.0, 1.0] | Minimum confidence to emit emergence |

#### observer.evolution_chamber

| Parameter | Type | Default | Range | Description |
|-----------|------|---------|-------|-------------|
| `max_concurrent_mutations` | u32 | 3 | [1, 10] | Maximum mutations running in parallel |
| `mutation_verification_ms` | u64 | 30000 | [5000, 300000] | Verification window duration |
| `fitness_history_capacity` | usize | 500 | [50, 5000] | Maximum fitness snapshots retained (EvolutionConfig -- not FitnessConfig) |
| `mutation_history_capacity` | usize | 1000 | [100, 10000] | Maximum mutation records retained |
| `auto_apply_threshold` | f64 | 0.10 | [0.01, 0.50] | Fitness improvement to auto-apply |
| `rollback_threshold` | f64 | -0.02 | [-0.20, 0.0] | Fitness decline triggering rollback |
| `min_generation_interval_ms` | u64 | 60000 | [10000, 600000] | Minimum interval between generations |
| `max_mutation_delta` | f64 | 0.20 | [0.01, 0.50] | Maximum parameter change per mutation |

#### observer.fitness

| Parameter | Type | Default | Range | Description |
|-----------|------|---------|-------|-------------|
| `history_capacity` | usize | 200 | [10, 1000] | Maximum FitnessReports retained (FitnessConfig -- not EvolutionConfig) |
| `trend_window` | usize | 10 | [3, 50] | Number of snapshots for trend calculation |
| `stability_tolerance` | f64 | 0.02 | [0.001, 0.10] | Std dev below this = stable |
| `volatility_threshold` | f64 | 0.05 | [0.01, 0.20] | Std dev above this = volatile |

---

## 8. Lock Ordering Constraints

### 8.1 Internal Lock Order

All L7 locks must be acquired in the following strict order to prevent deadlocks:

```
1. ObserverBus
2. LogCorrelator
3. EmergenceDetector
4. EvolutionChamber
5. FitnessEvaluator
```

**Rule:** A thread holding lock N may only acquire lock M where M > N. A thread holding lock M must never acquire lock N where N < M.

### 8.2 External Constraints

| Constraint | Rule | Rationale |
|------------|------|-----------|
| **Never hold L7 lock while acquiring L1-L6 lock** | L7 is a read-only observer; it must never block lower layers | Prevents priority inversion and system-wide deadlock |
| **EventBus publish is a separate lock scope** | Publish calls acquire/release the EventBus lock independently | Safe because publish is fire-and-forget |
| **Config reads do not hold locks** | Config is read at initialization and on explicit reload | Avoids config lock contention |

### 8.3 Lock Type

All L7 mutable state uses `parking_lot::RwLock`, consistent with L4 and L5 layer patterns.

| Lock | Type | Read Contention | Write Contention |
|------|------|-----------------|------------------|
| ObserverBus.channels | RwLock | Low (internal only) | Low (batch updates) |
| LogCorrelator.windows | RwLock | Medium (event ingestion reads) | Low (window finalization) |
| LogCorrelator.correlations | RwLock | Low (query reads) | Low (append only) |
| EmergenceDetector.history | RwLock | Low (query reads) | Low (detection cycle writes) |
| EmergenceDetector.active_cascades | RwLock | Low | Medium (cascade updates) |
| EvolutionChamber.generation | RwLock | Low | Very low (generation increment) |
| EvolutionChamber.active_mutations | RwLock | Low | Low (mutation lifecycle) |
| EvolutionChamber.mutation_history | RwLock | Low | Low (append only) |
| EvolutionChamber.ralph_state | RwLock | Medium (status queries) | Low (phase transitions) |
| FitnessEvaluator.history | RwLock | Low | Low (snapshot append) |

### 8.4 Lock Scope Best Practices

```rust
// CORRECT: Scoped lock, dropped before EventBus publish
{
    let guard = self.correlations.write();
    guard.push(correlated_event.clone());
}  // guard dropped here
event_bus.publish("observation", &payload)?;

// INCORRECT: Holding lock across EventBus publish
let guard = self.correlations.write();
guard.push(correlated_event.clone());
event_bus.publish("observation", &payload)?;  // DEADLOCK RISK
drop(guard);
```

---

## 9. Error Handling

### 9.1 Design Principle

L7 errors never crash the system. Observer failures are logged and counted, never propagated to L1-L6. The engine continues operating normally if L7 encounters any error.

### 9.2 Error Mapping

L7 uses the existing `Error` enum from M01 (`src/m1_foundation/error.rs`). New error contexts map to existing variants:

| L7 Error Context | Maps To | Recovery |
|-----------------|---------|----------|
| Event ingestion failure | `Error::Validation` | Log + skip event |
| Correlation computation error | `Error::Validation` | Log + discard window |
| Emergence detection error | `Error::Other` | Log + skip cycle |
| Mutation application failure | `Error::Other` | Log + rollback mutation |
| Fitness evaluation error | `Error::Validation` | Log + use last known fitness |
| EventBus subscription failure | `Error::Other` | Log + retry with backoff |
| EventBus publish failure | `Error::Other` | Log + increment error counter |
| Configuration parse error | `Error::Config` | Log + use defaults |
| Lock acquisition timeout | `Error::Other` | Log + skip operation |

### 9.3 Error Counting

```rust
/// Increment the observer error counter (never propagate)
fn handle_observer_error(metrics: &mut ObserverMetrics, error: &Error, context: &str) {
    metrics.observer_errors += 1;
    tracing::warn!(
        error = %error,
        context = context,
        total_errors = metrics.observer_errors,
        "L7 observer error (non-fatal)"
    );
}
```

### 9.4 Graceful Degradation

| Failure Mode | System Impact | Recovery |
|-------------|---------------|----------|
| M37 fails | No correlations published | Engine unaffected; M38/M39 idle |
| M38 fails | No emergence detection | Engine unaffected; M39 idle |
| M39 fails | No evolution mutations | Engine unaffected; all L1-L6 stable |
| Observer Bus fails | Internal L7 communication lost | Modules operate independently |
| Fitness Evaluator fails | No fitness scoring | Mutations paused; last known fitness used |
| All L7 fails | Zero observation capability | Engine operates identically to pre-L7 |

### 9.5 L7 Error Variant Mapping

L7 modules reuse existing `Error` enum variants from `m1_foundation::Error`. No new variants are needed. The following table maps each L7 public operation to the specific `Error` variant it returns on failure, based on the actual enum defined in `src/m1_foundation/error.rs`.

| L7 Operation | Error Variant | When |
|-------------|---------------|------|
| `LogCorrelator::ingest_event` | `Error::Validation(String)` | Invalid event format or missing fields |
| `LogCorrelator::get_correlations` | `Error::Validation(String)` | Invalid window ID |
| `EmergenceDetector::detect` | `Error::Validation(String)` | Empty correlation input |
| `EmergenceDetector::acknowledge` | `Error::ServiceNotFound(String)` | Unknown emergence ID |
| `EvolutionChamber::apply_mutation` | `Error::Pipeline(String)` | Mutation application failure |
| `EvolutionChamber::run_ralph_cycle` | `Error::Pipeline(String)` | RALPH phase failure |
| `EvolutionChamber::rollback_mutation` | `Error::Pipeline(String)` | Rollback failure |
| `FitnessEvaluator::evaluate` | `Error::TensorValidation { dimension, value }` | Invalid Tensor12D values (e.g., out-of-range dimension score) |
| `FitnessEvaluator::adjust_weights` | `Error::Validation(String)` | Weights do not sum to 1.0 |
| `ObserverBus::wire_m23_bridge` | `Error::Network { target, message }` | EventBus channel not found or unreachable |
| `ObserverLayer::subscribe_to_event_bus` | `Error::Network { target, message }` | Subscription failure on one or more channels |
| PBFT consensus request | `Error::ConsensusQuorum { required, received }` | Quorum not reached for mutation approval |
| `EvolutionChamber::verify_mutation` | `Error::Timeout { operation, timeout_ms }` | Verification window expired before fitness assessment |
| `ObserverLayer::initialize` | `Error::Config(String)` | Invalid or missing observer configuration parameters |

**Note:** All variants above exist in the `Error` enum at `src/m1_foundation/error.rs`. The L7 layer does not define or require any additional error variants. Per Section 9.1, all errors are logged and counted via `handle_observer_error()` but never propagated to L1-L6.

---

## 10. Performance Budget

### 10.1 Latency Targets

| Operation | Target | Max | Notes |
|-----------|--------|-----|-------|
| Event ingestion | <5ms | <10ms | Per event from EventBus |
| Correlation computation | <20ms | <50ms | Per correlation window |
| Emergence detection | <50ms | <100ms | Per detection cycle |
| Fitness evaluation | <10ms | <20ms | Per tensor evaluation |
| RALPH cycle | <200ms | <500ms | Full 5-phase cycle |
| Mutation application | <10ms | <50ms | Parameter update only |
| PBFT consensus | <5s | <10s | When consensus required |

### 10.2 Throughput Targets

| Component | Throughput | Notes |
|-----------|-----------|-------|
| Event ingestion | 1,000 events/s | Across all 6 subscribed channels |
| Correlation windows | 200 windows/s | Parallel window processing |
| Emergence detection | 60 cycles/min | One detection cycle per second |
| RALPH cycles | 1 cycle/min | Minimum generation interval enforced |
| Observation publishes | 100 events/s | High-frequency correlation output |
| Emergence publishes | 10 events/min | Low-frequency emergence output |
| Evolution publishes | 1 event/min | Very low-frequency evolution output |

### 10.3 Memory Budget

| Component | Allocation | Notes |
|-----------|-----------|-------|
| Correlation window buffer | ~10,000 events | Bounded by `max_buffer_size` |
| Correlation history | ~1,000 entries | Bounded ring buffer |
| Emergence history | ~1,000 entries | Bounded by `history_capacity` |
| Cascade trackers | ~100 entries | Active cascades only |
| Mutation history | ~1,000 entries | Bounded by `mutation_history_capacity` |
| Fitness history | ~200 entries | Bounded by `history_capacity` |
| Observer Bus messages | ~500 entries | Internal only, bounded |
| **Total estimated** | **~8 MB** | Conservative upper bound |

### 10.4 Zero-Cost When Disabled

When `observer.enabled = false`:

| Resource | Allocation | Cost |
|----------|-----------|------|
| Memory | 0 bytes | `Option::None` |
| CPU | 0 cycles | No subscriptions, no processing |
| EventBus subscriptions | 0 | Not registered |
| EventBus channels | 0 new | Not created |
| Locks | 0 | No state allocated |

---

## 11. Quality Requirements

### 11.1 Code Quality Gates

| Gate | Requirement | Enforcement |
|------|-------------|-------------|
| unsafe code | Zero | `#![forbid(unsafe_code)]` (compile-time) |
| `.unwrap()` | Zero | `#![deny(clippy::unwrap_used)]` (clippy) |
| `.expect()` | Zero | `#![deny(clippy::expect_used)]` (clippy) |
| Clippy pedantic | Zero warnings | `-W clippy::pedantic` |
| Clippy nursery | Zero warnings | `-W clippy::nursery` |
| Warning suppression | Never `#[allow(...)]` | Code review enforcement |
| Unit tests | 50 per module (300 total) | CI/CD gate |
| All existing tests | Must continue passing (1,013) | CI/CD gate |
| Documentation | 100% public items | `-W missing_docs` |

### 11.2 Test Distribution

| Module | Unit Tests | Coverage Target |
|--------|-----------|----------------|
| M37 LogCorrelator | 50 | >= 80% |
| M38 EmergenceDetector | 50 | >= 80% |
| M39 EvolutionChamber | 50 | >= 80% |
| Observer Bus | 50 | >= 80% |
| Fitness Evaluator | 50 | >= 80% |
| Layer Coordinator (mod.rs) | 50 | >= 80% |
| **Total** | **300** | **>= 80%** |

### 11.3 Test Categories per Module

| Category | Count | Description |
|----------|-------|-------------|
| Happy path | 15 | Normal operation flows |
| Edge cases | 10 | Boundary values, empty inputs, capacity limits |
| Error cases | 10 | Failure modes, invalid inputs, error recovery |
| Concurrency | 5 | Multi-threaded access, lock contention |
| Configuration | 5 | Default values, custom config, invalid config |
| Integration | 5 | Cross-module interaction within L7 |

### 11.4 Regression Safety

Adding L7 must not break any existing test:

```bash
# Before L7: 1,013 tests passing
cargo test --lib --release
# Expected: 1,013 passed, 0 failed

# After L7: 1,313 tests passing (1,013 existing + 300 new)
cargo test --lib --release
# Expected: 1,313 passed, 0 failed
```

---

## 12. NAM Compliance Contributions

### 12.1 Overview

L7 contributes to 3 of the 5 NAM requirements, improving the overall NAM compliance target of 92%.

| Requirement | Contribution | Mechanism | Current | After L7 |
|-------------|-------------|-----------|---------|----------|
| R1 SelfQuery | +20% | EvolutionChamber observes own mutation effectiveness | 0% | 20% |
| R2 HebbianRouting | +10% | FitnessEvaluator adjusts pathway weights | 0% | 10% |
| R4 FieldVisualization | +15% | ObservationReports provide system state view | 0% | 15% |

### 12.2 R1 SelfQuery Contribution (+20%)

The Evolution Chamber (M39) implements self-observation by:
- Tracking mutation effectiveness over time
- Computing fitness trends for its own mutations
- Adjusting RALPH parameters based on historical success rates
- Reporting on its own learning trajectory via the `evolution` channel

### 12.3 R2 HebbianRouting Contribution (+10%)

The Fitness Evaluator feeds back to Hebbian pathways by:
- Scoring pathway effectiveness using 12D tensor fitness
- Providing weight adjustment suggestions to M25 (Hebbian Manager) via EventBus
- Correlating pathway activation patterns with system health improvements

### 12.4 R4 FieldVisualization Contribution (+15%)

The ObservationReports published on the `observation` channel provide:
- Cross-layer correlation maps (which layers interact, how frequently)
- Emergence topology (cascade paths, synergy shifts, resonance patterns)
- Evolution state (current generation, fitness landscape, mutation history)
- Real-time system state view consumable by external dashboards

---

## 13. Security Considerations

### 13.1 Read-Only Observer

L7 is strictly read-only with respect to L1-L6. It subscribes to existing EventBus channels but never writes to L1-L6 state, never modifies running service configurations, and never directly alters lower-layer behavior. All mutations generated by the Evolution Chamber (M39) flow through the RALPH loop with mandatory verification, never bypassing the controlled mutation pipeline.

### 13.2 Mutation Safety

All mutations generated by M39 are subject to safety constraints:

| Safety Mechanism | Default | Description |
|-----------------|---------|-------------|
| Verification period | 30,000ms (30s) | Every mutation must pass a verification window before acceptance |
| Auto-apply threshold | 0.10 | Only mutations producing fitness improvement >= 10% may bypass PBFT consensus |
| Rollback threshold | -0.02 | Any fitness decline exceeding 2% triggers automatic rollback |
| Max mutation delta | 0.20 | Parameter changes are clamped to a maximum of 20% per mutation |
| Max concurrent mutations | 3 | At most 3 mutations may be in-flight simultaneously |
| Min generation interval | 60,000ms (60s) | Minimum time between evolution generations |

Negative fitness changes trigger automatic rollback to the original parameter value. Failed mutations are recorded in mutation history for future avoidance.

### 13.3 PBFT Consensus Gate

Mutations exceeding the `auto_apply_threshold` (default 0.10) require PBFT consensus before application. This means 27 out of 40 agents (quorum q=27, Byzantine tolerance f=13) must approve the mutation. This prevents:

- Unauthorized parameter modifications
- Harmful mutations that appear locally beneficial but have system-wide negative effects
- Runaway evolution cycles that could destabilize the system

The PBFT gate ensures that only mutations with broad agent agreement are applied to critical parameters.

### 13.4 Bounded Resources

All L7 data structures have configurable capacity limits to prevent unbounded memory growth:

| Resource | Capacity Limit | Configuration Parameter |
|----------|---------------|------------------------|
| Correlation window buffer | 10,000 events | `observer.log_correlator.max_buffer_size` |
| Emergence history | 1,000 entries | `observer.emergence_detector.history_capacity` |
| Fitness history (FitnessEvaluator) | 200 snapshots | `observer.fitness.history_capacity` |
| Fitness history (EvolutionChamber) | 500 snapshots | `observer.evolution_chamber.fitness_history_capacity` |
| Mutation history | 1,000 records | `observer.evolution_chamber.mutation_history_capacity` |
| Observer Bus messages | 500 entries | Internal bounded buffer |
| Observation channel buffer | 1,000 events | Rolling buffer |
| Emergence channel buffer | 100 events | Rolling buffer |
| Evolution channel buffer | 50 events | Rolling buffer |

All buffers use ring buffer semantics: when capacity is reached, the oldest entries are evicted. No unbounded growth is possible.

### 13.5 No Network Exposure

L7 does not expose any new network endpoints, ports, or external interfaces. All communication is strictly internal:

- **Inbound:** EventBus subscriptions (M23) -- internal pub/sub, no network sockets
- **Internal:** Observer Bus -- in-process message passing between M37, M38, M39
- **Outbound:** EventBus publications to `observation`, `emergence`, `evolution` channels -- internal only

No HTTP, gRPC, WebSocket, or IPC endpoints are created by L7. External systems that wish to consume L7 data must subscribe to EventBus channels through existing M23 infrastructure.

### 13.6 Configuration Validation

All configuration parameters have defined valid ranges (see Section 7.2). At startup, the observer configuration is validated against these ranges:

| Validation | Action on Failure |
|-----------|-------------------|
| Parameter out of range | `Error::Config(String)` -- startup rejected |
| Missing required parameter | `Error::Config(String)` -- startup rejected |
| Invalid type | `Error::Config(String)` -- startup rejected |
| TOML parse error | `Error::Config(String)` -- startup rejected |

Configuration is read once at initialization and on explicit reload. No hot-patching of configuration is permitted without validation. This prevents runtime injection of unsafe parameters.

### 13.7 Error Isolation

L7 errors are fully isolated from L1-L6 (see Section 9). The isolation guarantee is:

- L7 errors are logged via `tracing::warn!` and counted in `ObserverMetrics.observer_errors`
- L7 errors are **never** propagated to L1-L6 call stacks
- A complete L7 failure (all modules down) has **zero impact** on system operation
- The engine operates identically with or without L7 (`Option<ObserverLayer>` = `None`)
- No L7 lock is ever held while acquiring an L1-L6 lock (Section 8.2)

### 13.8 No Credential Access

L7 does not access, store, or transmit any security credentials:

- No service tokens (12 ULTRAPLATE service auth tokens)
- No agent tokens (40 CVA-NAM agent auth tokens)
- No human @0.A tokens
- No API keys or external integration secrets
- No database credentials (L7 has no direct database access)

L7 observes only: health metrics, log events, remediation actions, learning events, consensus outcomes, integration events, and system metrics. All data consumed by L7 is operational telemetry, not security-sensitive.

---

## 14. Version History

| Version | Date | Changes |
|---------|------|---------|
| 1.1.0 | 2026-01-29 | Added Section 13: Security Considerations (GAP-013) |
| 1.0.0 | 2026-01-29 | Initial specification: architecture overview, module inventory, dependency graph, EventBus integration, channel specifications, Rust type definitions, configuration specification, lock ordering, error handling, performance budget, quality requirements, NAM compliance |

---

## Navigation

| Direction | Link |
|-----------|------|
| Up (Spec Index) | [INDEX.md](INDEX.md) |
| Up (Parent Specs) | [ai_specs/INDEX.md](../INDEX.md) |
| Companion Docs | [ai_docs/evolution_chamber_ai_docs/L07_OBSERVER_LAYER.md](../../ai_docs/evolution_chamber_ai_docs/L07_OBSERVER_LAYER.md) |
| Tensor Spec | [ai_specs/TENSOR_SPEC.md](../TENSOR_SPEC.md) |
| STDP Spec | [ai_specs/STDP_SPEC.md](../STDP_SPEC.md) |
| NAM Spec | [ai_specs/NAM_SPEC.md](../NAM_SPEC.md) |
| PBFT Spec | [ai_specs/PBFT_SPEC.md](../PBFT_SPEC.md) |
| EventBus (M23) | [ai_docs/modules/M23_EVENT_BUS.md](../../ai_docs/modules/M23_EVENT_BUS.md) |

---

*The Maintenance Engine v1.0.0 | L7 Observer Layer Architecture Specification*
*Last Updated: 2026-01-29*
