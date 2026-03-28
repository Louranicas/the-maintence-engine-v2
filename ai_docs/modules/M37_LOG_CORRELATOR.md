# Module M37: Log Correlator

> **M37_LOG_CORRELATOR** | Cross-layer event correlation engine | Layer: L7 Observer | [Back to Index](INDEX.md)

---

## Navigation

| Direction | Link |
|-----------|------|
| Up | [INDEX.md](INDEX.md) |
| Layer | [L07_OBSERVER.md](../layers/L07_OBSERVER.md) |
| Dependency | [M23_EVENT_BUS.md](M23_EVENT_BUS.md) |
| Dependency | [M01_ERROR_TAXONOMY.md](M01_ERROR_TAXONOMY.md) |
| Related | [M38_EMERGENCE_DETECTOR.md](M38_EMERGENCE_DETECTOR.md) |
| Related | [M39_EVOLUTION_CHAMBER.md](M39_EVOLUTION_CHAMBER.md) |

---

## Module Specification

### Overview

The Log Correlator is the L7 Observer Layer's ingestion and correlation engine. It subscribes to all 6 `EventBus` channels (metrics, health, remediation, integration, learning, consensus) and detects temporal, causal, semantic, and recurring correlations between events from different layers (L1-L6). Correlated events are emitted to the Observer Bus for downstream consumption by M38 (Emergence Detector).

Events flow through a sliding-window correlation pipeline: raw events are ingested, matched against existing buffered events using four correlation algorithms, grouped into time-bounded correlation windows, and analyzed for recurring periodic patterns.

### Module Properties

| Property | Value |
|----------|-------|
| Module ID | M37 |
| Module Name | Log Correlator |
| Layer | L7 (Observer) |
| Source File | `src/m7_observer/log_correlator.rs` |
| LOC | 1,301 |
| Tests | 39 |
| Version | 1.0.0 |
| Lock Order | 2 (after ObserverBus, before EmergenceDetector) |
| Dependencies | M23 (EventBus), M01 (Error) |
| Dependents | M38 (Emergence Detector), L7 Coordinator |
| Thread Safety | `parking_lot::RwLock` on all mutable state |
| External Crates | `chrono`, `parking_lot`, `serde`, `uuid` |

---

## Architecture

```
                         EventBus (M23)
                              |
              +------+--------+--------+------+------+
              |      |        |        |      |      |
          metrics  health  remediation integration learning consensus
          (L1)    (L2)      (L3)       (L4)    (L5)    (L6)
              |      |        |        |      |      |
              +------+--------+--------+------+------+
                              |
                     +--------v--------+
                     |  LogCorrelator  |
                     |  (M37)          |
                     +--------+--------+
                     |                 |
         +-----------+     +-----------+----------+
         |                 |                      |
    Ingested Log      Correlation           Recurring Pattern
    (ring buffer)     Windows               Detector
         |           (time-bounded)              |
         |                 |                     |
         +--------+--------+---------------------+
                  |
         +--------v--------+
         |  Event Buffer   |
         |  (CorrelatedEvent ring) |
         +--------+--------+
                  |
                  v
           Observer Bus --> M38 Emergence Detector
```

---

## Correlation Types

| Type | Formula | Description |
|------|---------|-------------|
| Temporal | `1.0 - (\|delta_ms\| / temporal_tolerance_ms)` | Events from different channels occurring close in time |
| Causal | `0.8 * (1.0 - delta_ms / window_size_ms)` | Same event type in a downstream layer, after the source event |
| Semantic | `layers_count / 6.0` | Same event type spread across multiple layers |
| Recurring | `1.0 - (stddev / mean)` | Periodic repetition of the same event type on the same channel |

### Channel-to-Layer Mapping

| Channel | Layer |
|---------|-------|
| `metrics` | L1 (Foundation) |
| `health` | L2 (Services) |
| `remediation` | L3 (Core Logic) |
| `integration` | L4 (Integration) |
| `learning` | L5 (Learning) |
| `consensus` | L6 (Consensus) |

---

## Core Data Structures

### IngestedEvent

```rust
pub struct IngestedEvent {
    pub event_id: String,          // Original EventBus event ID
    pub channel: String,           // Source channel
    pub event_type: String,        // Event type tag
    pub payload: String,           // JSON payload
    pub source_layer: u8,          // Source layer (1-6)
    pub ingested_at: DateTime<Utc>, // Ingestion timestamp
}
```

### CorrelationLink

```rust
pub struct CorrelationLink {
    pub source_event_id: String,   // Source event ID
    pub target_event_id: String,   // Target event ID
    pub link_type: CorrelationLinkType, // Temporal, Causal, Semantic, Recurring
    pub confidence: f64,           // Link confidence [0.0, 1.0]
    pub temporal_offset_ms: i64,   // Temporal offset (signed)
}
```

### CorrelatedEvent

```rust
pub struct CorrelatedEvent {
    pub id: String,                       // Correlation ID (UUID v4)
    pub primary_event: IngestedEvent,     // Primary event
    pub related_events: Vec<IngestedEvent>, // Related events found
    pub links: Vec<CorrelationLink>,      // Correlation links
    pub confidence: f64,                  // Overall confidence [0.0, 1.0]
    pub discovered_at: DateTime<Utc>,     // Discovery timestamp
}
```

### CorrelationWindow

```rust
pub struct CorrelationWindow {
    pub window_id: String,         // UUID v4
    pub events: Vec<IngestedEvent>, // Events in this window
    pub links: Vec<CorrelationLink>, // Links within this window
    pub start_time: DateTime<Utc>, // Window start
    pub end_time: DateTime<Utc>,   // Window end
    pub correlation_count: u32,    // Total correlations found
    pub finalized: bool,           // Whether window is finalized
}
```

### RecurringPattern

```rust
pub struct RecurringPattern {
    pub pattern_id: String,        // UUID v4
    pub event_sequence: Vec<String>, // Ordered event type sequence
    pub channel_sequence: Vec<String>, // Ordered channel sequence
    pub occurrence_count: u64,     // Total occurrences observed
    pub average_interval_ms: u64,  // Mean interval between occurrences (ms)
    pub stddev_interval_ms: f64,   // Std dev of intervals (ms)
    pub confidence: f64,           // Confidence based on regularity
    pub first_seen: DateTime<Utc>, // First observation
    pub last_seen: DateTime<Utc>,  // Most recent observation
}
```

### CorrelationStats

```rust
pub struct CorrelationStats {
    pub total_events_ingested: u64,        // Total events since startup
    pub total_correlations_found: u64,     // Total links discovered
    pub active_windows: usize,             // Non-expired windows
    pub recurring_patterns: usize,         // Detected recurring patterns
    pub buffer_utilization: f64,           // Buffer usage ratio
    pub avg_correlations_per_event: f64,   // Average links per event
    pub correlation_type_counts: HashMap<String, u64>, // Breakdown by type
}
```

### CorrelationLinkType (Enum)

| Variant | Display | Description |
|---------|---------|-------------|
| `Temporal` | `"temporal"` | Events close in time, different channels |
| `Causal` | `"causal"` | Same type, downstream layer, after source |
| `Semantic` | `"semantic"` | Same type across multiple layers |
| `Recurring` | `"recurring"` | Periodic repetition pattern |

---

## Public API

### Construction

| Method | Signature | Description |
|--------|-----------|-------------|
| `new()` | `-> Self` | Creates with default configuration |
| `with_config(config)` | `-> Self` | Creates with custom configuration |
| `validate_config(config)` | `-> Result<()>` | Validates configuration parameters |

### Ingestion

| Method | Signature | Description |
|--------|-----------|-------------|
| `ingest_event(event_id, channel, event_type, payload, timestamp)` | `-> Result<CorrelatedEvent>` | Ingests a raw event, computes correlations, stores result |

### Query

| Method | Signature | Description |
|--------|-----------|-------------|
| `get_correlations(event_id)` | `-> Vec<CorrelationLink>` | Returns all links for a specific event |
| `get_recurring_patterns()` | `-> Vec<RecurringPattern>` | Returns all patterns sorted by confidence |
| `get_events_in_range(start, end)` | `-> Vec<CorrelatedEvent>` | Returns events within a time range |
| `get_window(window_id)` | `-> Result<CorrelationWindow>` | Returns a specific window by ID |
| `recent_events(n)` | `-> Vec<CorrelatedEvent>` | Returns most recent N events (newest last) |
| `window_ids()` | `-> Vec<String>` | Returns all window IDs |

### Statistics

| Method | Signature | Description |
|--------|-----------|-------------|
| `stats()` | `-> CorrelationStats` | Returns aggregate statistics |
| `buffer_len()` | `-> usize` | Event buffer length |
| `ingested_count()` | `-> usize` | Ingested log length |
| `active_window_count()` | `-> usize` | Non-finalized window count |
| `finalized_window_count()` | `-> usize` | Finalized window count |
| `total_links()` | `-> usize` | Total correlation links in buffer |
| `config()` | `-> &LogCorrelatorConfig` | Returns immutable configuration |

### Pattern Detection

| Method | Signature | Description |
|--------|-----------|-------------|
| `detect_periodic_patterns()` | `-> Vec<RecurringPattern>` | Runs periodic pattern detection against buffer |

### Maintenance

| Method | Signature | Description |
|--------|-----------|-------------|
| `prune_before(timestamp)` | `-> usize` | Removes events older than timestamp, returns count |
| `clear()` | `-> ()` | Clears all buffers and resets statistics |

---

## Configuration

### LogCorrelatorConfig

| Parameter | Type | Default | Range | Description |
|-----------|------|---------|-------|-------------|
| `window_size_ms` | `u64` | 5,000 | [100, 60,000] | Correlation window duration (ms) |
| `max_buffer_size` | `usize` | 10,000 | > 0 | Maximum events buffered |
| `min_correlation_confidence` | `f64` | 0.6 | [0.0, 1.0] | Minimum confidence to emit |
| `min_recurring_count` | `u32` | 3 | >= 1 | Minimum occurrences for pattern |
| `temporal_tolerance_ms` | `u64` | 500 | > 0 | Max time offset for temporal correlation |
| `max_correlations_per_event` | `usize` | 20 | >= 1 | Max links per ingested event |

### Internal Constants

| Constant | Value | Description |
|----------|-------|-------------|
| `MAX_RECURRING_PATTERNS` | 100 | Maximum retained recurring patterns |

---

## Metrics

| Metric | Type | Source | Description |
|--------|------|--------|-------------|
| `total_events_ingested` | Counter | `CorrelationStats` | Total events processed |
| `total_correlations_found` | Counter | `CorrelationStats` | Total links discovered |
| `active_windows` | Gauge | `CorrelationStats` | Non-expired correlation windows |
| `recurring_patterns` | Gauge | `CorrelationStats` | Detected recurring patterns |
| `buffer_utilization` | Gauge | `CorrelationStats` | Buffer usage ratio [0.0, 1.0] |
| `avg_correlations_per_event` | Gauge | `CorrelationStats` | Average links per event |
| `correlation_type_counts` | Counter(map) | `CorrelationStats` | Breakdown by link type |

---

## Error Codes

| Error Type | Condition | Raised By |
|------------|-----------|-----------|
| `Error::Config` | `window_size_ms` outside [100, 60000] | `validate_config` |
| `Error::Config` | `max_buffer_size` is 0 | `validate_config` |
| `Error::Config` | `min_correlation_confidence` outside [0.0, 1.0] | `validate_config` |
| `Error::Config` | `temporal_tolerance_ms` is 0 | `validate_config` |
| `Error::Validation` | `event_id` is empty | `ingest_event` |
| `Error::Validation` | `channel` is empty | `ingest_event` |
| `Error::Validation` | Window not found by ID | `get_window` |

---

## Related Modules

| Module | Relationship | Purpose |
|--------|--------------|---------|
| M23 (EventBus) | Dependency | Source of raw events from all 6 channels |
| M01 (Error) | Dependency | Error taxonomy for Result types |
| M38 (Emergence Detector) | Downstream | Consumes CorrelatedEvents for emergence detection |
| M39 (Evolution Chamber) | Downstream | Indirectly fed via emergence records |
| Observer Bus | Upstream/Downstream | Event distribution within L7 |
| Fitness Evaluator | Sibling | Shares L7 Observer Bus |

---

## Testing

Key test cases (39 total):

```rust
#[test] fn test_new_correlator_defaults()        // Verify initial state
#[test] fn test_ingest_single_event()             // Single event ingestion
#[test] fn test_ingest_empty_event_id_fails()     // Validation: empty event_id
#[test] fn test_ingest_empty_channel_fails()      // Validation: empty channel
#[test] fn test_temporal_correlation()             // Cross-channel temporal match
#[test] fn test_causal_correlation()               // Same type, downstream layer
#[test] fn test_no_self_correlation()              // Prevents self-linking
```

---

## Performance Characteristics

| Operation | Time | Notes |
|-----------|------|-------|
| Event ingestion + correlation | <5ms | Linear scan of buffer (bounded) |
| Periodic pattern detection | <50ms | Groups by type, computes intervals |
| Window management | <2ms | HashMap insert/retain |
| Buffer pruning | <10ms | Single pass filter |
| Stats query | <1ms | Lock + clone |

---

## Version History

| Version | Date | Changes |
|---------|------|---------|
| 1.0.0 | 2026-01-29 | Initial implementation (L7 Observer Layer) |

---

[INDEX.md](INDEX.md) | [L07_OBSERVER.md](../layers/L07_OBSERVER.md) | [M38_EMERGENCE_DETECTOR.md](M38_EMERGENCE_DETECTOR.md)

*The Maintenance Engine v1.0.0 | M37: Log Correlator*
*Last Updated: 2026-01-29*
