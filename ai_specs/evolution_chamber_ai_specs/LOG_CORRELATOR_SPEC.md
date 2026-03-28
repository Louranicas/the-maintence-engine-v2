# M37 Log Correlator - Formal Specification

```json
{"v":"1.0.0","type":"MODULE_SPEC","module":"M37","name":"Log Correlator","layer":7,"estimated_loc":1400,"estimated_tests":50}
```

**Version:** 1.0.0
**Layer:** L7 (Observer)
**Module:** M37
**Related:** [SYSTEM_SPEC.md](../SYSTEM_SPEC.md), [PIPELINE_SPEC.md](../PIPELINE_SPEC.md), [STDP_SPEC.md](../STDP_SPEC.md), [ESCALATION_SPEC.md](../ESCALATION_SPEC.md)

---

## Navigation

| Direction | Target |
|-----------|--------|
| Up | [INDEX.md](INDEX.md) |
| Next | [EMERGENCE_DETECTOR_SPEC.md](EMERGENCE_DETECTOR_SPEC.md) |

---

## 1. Purpose

The Log Correlator is a cross-layer event correlation engine forming the foundational component of the L7 Observer Layer. It subscribes to all 6 existing EventBus channels (`health`, `remediation`, `learning`, `consensus`, `integration`, `metrics`) and detects temporal, causal, resonance, cascade, and periodic correlations between events originating from different layers (L1-L6).

### Objectives

| Objective | Description |
|-----------|-------------|
| Cross-layer visibility | Correlate events from all 6 EventBus channels into unified correlation windows |
| Temporal pattern detection | Identify events occurring within configurable time windows across layers |
| Causal chain identification | Trace upstream-downstream event dependencies through the service mesh |
| Resonance detection | Detect identical event types firing across multiple layers simultaneously |
| Cascade tracking | Follow failure propagation through service dependency chains |
| Periodic pattern recognition | Identify recurring event sequences with statistical regularity |

### EventBus Channel Subscriptions

| Channel | Source Layer | Event Types Monitored |
|---------|-------------|----------------------|
| `health` | L2 (Services) | Health checks, service status changes, threshold breaches |
| `remediation` | L3 (Core Logic) | Auto-remediation triggers, action outcomes, rollback events |
| `learning` | L5 (Learning) | LTP/LTD events, pathway updates, pattern recognitions |
| `consensus` | L6 (Consensus) | PBFT rounds, vote results, view changes, dissent records |
| `integration` | L4 (Integration) | Bridge status, protocol changes, connection events |
| `metrics` | L1 (Foundation) | Performance metrics, resource utilization, threshold alerts |

---

## 2. Complete Type Definitions

### 2.1 Core Structures

```rust
use std::collections::{HashMap, VecDeque};
use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use uuid::Uuid;

/// M37: Cross-layer event correlation engine.
///
/// Subscribes to all 6 EventBus channels and detects temporal, causal,
/// resonance, cascade, and periodic correlations between events from
/// different layers (L1-L6).
///
/// # Layer: L7 (Observer)
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
```

### 2.2 Event Structures

```rust
/// An event ingested by the correlator, enriched with correlation metadata.
#[derive(Clone, Debug)]
pub struct CorrelatedEvent {
    /// Unique identifier for this correlated event (UUID v4).
    pub id: String,

    /// Original event ID from EventRecord.id in the source EventBus channel.
    pub original_event_id: String,

    /// Source EventBus channel name (one of: health, remediation, learning,
    /// consensus, integration, metrics).
    pub channel: String,

    /// Application-defined event classification from EventRecord.event_type.
    pub event_type: String,

    /// FNV-1a hash of the original event payload for fast equality checks.
    pub payload_hash: u64,

    /// Timestamp of the original event (preserved from EventRecord.timestamp).
    pub timestamp: DateTime<Utc>,

    /// Correlation links discovered for this event against the buffer.
    pub correlations: Vec<CorrelationLink>,

    /// Layer of origin (1-6) indicating which architectural layer generated
    /// the original event.
    pub layer_origin: u8,
}
```

### 2.3 Correlation Types

```rust
/// A directed link between two correlated events.
#[derive(Clone, Debug)]
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

/// Classification of correlation relationships between events.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum CorrelationType {
    /// Events within the same time window across different channels.
    /// Confidence = 1.0 - (|delta_ms| / temporal_tolerance_ms).
    Temporal,

    /// Event A from upstream service triggers Event B in downstream service.
    /// Confidence = 0.8 * (1.0 - delta_ms / window_size_ms).
    Causal,

    /// Same event_type fires across 2+ different layers within a window.
    /// Confidence = layers_count / 6.0.
    Resonance,

    /// Failure propagates through service dependency chain: A -> B -> C.
    /// Confidence = min(0.95, 0.7 + depth * 0.1).
    Cascade,

    /// Events recur at regular intervals with stddev < 20% of mean interval.
    /// Confidence = 1.0 - (stddev / mean).
    Periodic,
}
```

### 2.4 Window and Pattern Structures

```rust
/// A time-bounded window grouping correlated events.
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
#[derive(Clone, Debug)]
pub struct RecurringPattern {
    /// Unique pattern identifier (UUID v4).
    pub pattern_id: String,

    /// Ordered sequence of event_type values forming the pattern.
    pub event_sequence: Vec<String>,

    /// Ordered sequence of channel names corresponding to the events.
    pub channel_sequence: Vec<String>,

    /// Total number of times this pattern has been observed.
    pub occurrence_count: u64,

    /// Mean interval between consecutive pattern occurrences (ms).
    pub average_interval_ms: u64,

    /// Standard deviation of intervals between occurrences (ms).
    pub stddev_interval_ms: f64,

    /// Confidence score based on regularity: 1.0 - (stddev / mean).
    pub confidence: f64,

    /// Timestamp of the first observation of this pattern.
    pub first_seen: DateTime<Utc>,

    /// Timestamp of the most recent observation of this pattern.
    pub last_seen: DateTime<Utc>,
}
```

### 2.5 Configuration

```rust
/// Configuration for the LogCorrelator.
/// All fields have sensible defaults; override via config/observer.toml.
#[derive(Clone, Debug)]
pub struct LogCorrelatorConfig {
    /// Maximum time span (ms) for events to be considered in the same
    /// correlation window. Default: 5000.
    pub window_size_ms: u64,

    /// Maximum number of CorrelatedEvents retained in the ring buffer.
    /// Default: 10000.
    pub max_buffer_size: usize,

    /// Minimum confidence threshold for a correlation link to be recorded.
    /// Default: 0.6.
    pub min_correlation_confidence: f64,

    /// Minimum occurrence count before a sequence is promoted to
    /// RecurringPattern. Default: 3.
    pub min_recurring_count: u64,

    /// Maximum time delta (ms) for Temporal correlation detection.
    /// Default: 500.
    pub temporal_tolerance_ms: u64,

    /// Maximum number of CorrelationLinks per CorrelatedEvent.
    /// Default: 20.
    pub max_correlations_per_event: usize,

    /// Maximum ratio of stddev to mean interval for Periodic detection.
    /// Default: 0.2 (20%).
    pub periodic_stddev_ratio: f64,
}

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

### 2.6 Statistics

```rust
/// Aggregate statistics for the LogCorrelator.
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

    /// Buffer utilization ratio: current_size / max_buffer_size.
    pub buffer_utilization: f64,

    /// Average number of correlation links per ingested event.
    pub avg_correlations_per_event: f64,

    /// Breakdown of correlations by CorrelationType name.
    pub correlation_type_counts: HashMap<String, u64>,
}
```

---

## 3. Correlation Algorithms

### 3.1 Temporal Correlation

Detects events occurring within the same time window across different EventBus channels.

```
ALGORITHM: Temporal Correlation
INPUT: new event E, event buffer B
OUTPUT: set of CorrelationLinks

FOR each new event E:
  FOR each event E' in buffer B
    WHERE |E.timestamp - E'.timestamp| < temporal_tolerance_ms:
    IF E.channel != E'.channel:                   // Cross-layer only
      delta_ms = E.timestamp - E'.timestamp       // Signed delta
      abs_delta = |delta_ms|
      confidence = 1.0 - (abs_delta as f64 / temporal_tolerance_ms as f64)
      IF confidence >= min_correlation_confidence:
        CREATE CorrelationLink {
          target_event_id: E'.id,
          correlation_type: CorrelationType::Temporal,
          confidence: confidence,
          time_delta_ms: delta_ms,
        }
```

| Property | Value |
|----------|-------|
| Constraint | Cross-channel only (E.channel != E'.channel) |
| Confidence Formula | `1.0 - (|delta_ms| / temporal_tolerance_ms)` |
| Confidence Range | [min_correlation_confidence, 1.0] |
| Default Tolerance | 500ms |
| Max Confidence | 1.0 (simultaneous events, delta = 0) |

### 3.2 Causal Correlation

Identifies upstream-downstream event dependencies based on the layer ordering (L1 -> L2 -> L3 -> L4 -> L5 -> L6) and service dependency chains.

```
ALGORITHM: Causal Correlation
INPUT: new event E, event buffer B, dependency_map D
OUTPUT: set of CorrelationLinks

FOR each new event E:
  IF E is an action event OR response event:
    SEARCH buffer B for trigger event E' WHERE:
      (1) E'.channel is upstream of E.channel
          Upstream ordering: metrics < health < remediation < integration < learning < consensus
      AND (2) E'.event_type matches known trigger patterns for E.event_type
      AND (3) E'.timestamp < E.timestamp            // Cause precedes effect
      AND (4) (E.timestamp - E'.timestamp) < window_size_ms

    FOR each matching E':
      delta_ms = E.timestamp - E'.timestamp
      confidence = 0.8 * (1.0 - delta_ms as f64 / window_size_ms as f64)
      IF confidence >= min_correlation_confidence:
        CREATE CorrelationLink {
          target_event_id: E'.id,
          correlation_type: CorrelationType::Causal,
          confidence: confidence,
          time_delta_ms: delta_ms,
        }
```

| Property | Value |
|----------|-------|
| Upstream Ordering | metrics (L1) -> health (L2) -> remediation (L3) -> integration (L4) -> learning (L5) -> consensus (L6) |
| Constraint | E'.timestamp < E.timestamp (strict temporal ordering) |
| Confidence Formula | `0.8 * (1.0 - delta_ms / window_size_ms)` |
| Confidence Range | [min_correlation_confidence, 0.8] |
| Max Confidence | 0.8 (near-instantaneous causal link) |

### 3.3 Resonance Detection

Detects identical event types firing across multiple layers within a single correlation window.

```
ALGORITHM: Resonance Detection
INPUT: new event E, event buffer B
OUTPUT: set of CorrelationLinks

FOR each new event E:
  GROUP events in buffer B by event_type
  FOR each group G WHERE G.event_type == E.event_type:
    distinct_layers = DISTINCT(G.events.map(|e| e.layer_origin))
    IF distinct_layers.len() >= 2:
      window_events = G.events.filter(|e| |e.timestamp - E.timestamp| < window_size_ms)
      IF window_events.len() >= 2:
        layers_in_window = DISTINCT(window_events.map(|e| e.layer_origin))
        resonance_confidence = layers_in_window.len() as f64 / 6.0
        avg_delta = MEAN(window_events.map(|e| e.timestamp - E.timestamp))
        IF resonance_confidence >= min_correlation_confidence:
          FOR each event E' in window_events WHERE E'.id != E.id:
            CREATE CorrelationLink {
              target_event_id: E'.id,
              correlation_type: CorrelationType::Resonance,
              confidence: resonance_confidence,
              time_delta_ms: avg_delta,
            }
```

| Property | Value |
|----------|-------|
| Minimum Layers | 2 (events from at least 2 different layers) |
| Confidence Formula | `layers_count / 6.0` |
| Confidence Range | [0.333..., 1.0] |
| Max Confidence | 1.0 (all 6 layers resonate) |
| Grouping Key | `event_type` field |

### 3.4 Cascade Detection

Follows failure event propagation through service dependency chains (service A fails, then dependent service B fails, then C fails).

```
ALGORITHM: Cascade Detection
INPUT: failure event E, event buffer B, service_dependency_map SDM
OUTPUT: set of CorrelationLinks, cascade_depth

cascade_depth = 0
current_service = E.source_service_id
visited = {current_service}

FOR each failure event E:
  dependents = SDM.get_dependents(current_service)
  FOR each dependent service S in dependents:
    IF S not in visited:
      SEARCH buffer B for failure event E' WHERE:
        E'.source_service_id == S
        AND E'.timestamp > E.timestamp
        AND (E'.timestamp - E.timestamp) < window_size_ms
      IF found E':
        cascade_depth += 1
        confidence = min(0.95, 0.7 + cascade_depth as f64 * 0.1)
        delta_ms = E'.timestamp - E.timestamp
        CREATE CorrelationLink {
          target_event_id: E'.id,
          correlation_type: CorrelationType::Cascade,
          confidence: confidence,
          time_delta_ms: delta_ms,
        }
        visited.insert(S)
        RECURSE with E' as new root, updated visited set
```

| Property | Value |
|----------|-------|
| Trigger | Failure events only (health degradation, error, timeout) |
| Confidence Formula | `min(0.95, 0.7 + depth * 0.1)` |
| Depth = 1 | Confidence = 0.80 |
| Depth = 2 | Confidence = 0.90 |
| Depth >= 3 | Confidence = 0.95 (capped) |
| Max Depth | Bounded by service dependency graph (max 12 services) |
| Cycle Prevention | Visited set prevents infinite recursion |

### 3.5 Periodic Detection

Identifies recurring event sequences that fire at statistically regular intervals.

```
ALGORITHM: Periodic Detection
INPUT: event buffer B, existing recurring_patterns P
OUTPUT: updated recurring_patterns P'

GROUP events in buffer B by (event_type, channel) tuple
FOR each group G WITH occurrence_count >= min_recurring_count:
  SORT G.events by timestamp ASC
  intervals = []
  FOR i in 1..G.events.len():
    interval = G.events[i].timestamp - G.events[i-1].timestamp
    intervals.push(interval.as_millis())

  mean_interval = MEAN(intervals)
  stddev_interval = STDDEV(intervals)

  IF mean_interval > 0 AND stddev_interval / mean_interval < periodic_stddev_ratio:
    confidence = 1.0 - (stddev_interval / mean_interval)
    SEARCH P for existing pattern with matching (event_type, channel):
      IF found:
        UPDATE pattern.occurrence_count += G.events.len()
        UPDATE pattern.average_interval_ms = mean_interval
        UPDATE pattern.stddev_interval_ms = stddev_interval
        UPDATE pattern.confidence = confidence
        UPDATE pattern.last_seen = G.events.last().timestamp
      ELSE:
        CREATE RecurringPattern {
          pattern_id: Uuid::new_v4(),
          event_sequence: [G.event_type],
          channel_sequence: [G.channel],
          occurrence_count: G.events.len(),
          average_interval_ms: mean_interval,
          stddev_interval_ms: stddev_interval,
          confidence: confidence,
          first_seen: G.events.first().timestamp,
          last_seen: G.events.last().timestamp,
        }
```

| Property | Value |
|----------|-------|
| Minimum Occurrences | `min_recurring_count` (default: 3) |
| Regularity Threshold | `stddev / mean < periodic_stddev_ratio` (default: 0.2) |
| Confidence Formula | `1.0 - (stddev / mean)` |
| Confidence Range | [0.8, 1.0) for patterns passing threshold |
| Grouping Key | `(event_type, channel)` tuple |
| Persistence | RecurringPatterns persist until manually cleared |

---

## 4. API Contract

### 4.1 Constructor

```rust
/// Creates a new LogCorrelator with the given configuration.
///
/// # Preconditions
/// - config.window_size_ms > 0
/// - config.max_buffer_size > 0
/// - config.min_correlation_confidence in [0.0, 1.0]
/// - config.temporal_tolerance_ms > 0 AND <= config.window_size_ms
/// - config.max_correlations_per_event > 0
/// - config.periodic_stddev_ratio in (0.0, 1.0]
///
/// # Postconditions
/// - event_buffer is empty
/// - correlation_windows is empty
/// - recurring_patterns is empty
///
/// # Errors
/// - `Error::Validation` if any precondition is violated
pub fn new(config: LogCorrelatorConfig) -> Result<Self>;
```

### 4.2 Event Ingestion

```rust
/// Ingests a raw EventRecord from an EventBus channel, computes correlations
/// against the existing buffer, and stores the enriched CorrelatedEvent.
///
/// # Preconditions
/// - event.channel is one of: health, remediation, learning, consensus, integration, metrics
/// - event.id is a valid UUID v4
/// - event.timestamp is not in the future (tolerance: 1 second)
/// - event.timestamp is within window_size_ms of the current time
///
/// # Postconditions
/// - CorrelatedEvent added to event_buffer (FIFO eviction if full)
/// - All qualifying correlation links computed and attached
/// - Active correlation windows updated
/// - stats.total_events_ingested incremented
///
/// # Errors
/// - `Error::Validation("invalid channel")` if channel is unknown
/// - `Error::Validation("event too old")` if timestamp exceeds window
/// - `Error::Validation("buffer full")` if FIFO eviction fails (should not happen)
pub fn ingest_event(&self, event: &EventRecord, layer_origin: u8) -> Result<CorrelatedEvent>;
```

### 4.3 Query Methods

```rust
/// Returns all correlation links for a specific event by its correlated event ID.
///
/// # Preconditions
/// - event_id is a valid UUID v4 string
///
/// # Postconditions
/// - Returns correlations for the event if found, empty vec otherwise
///
/// # Complexity: O(1) via internal HashMap lookup
pub fn get_correlations(&self, event_id: &str) -> Vec<CorrelationLink>;

/// Returns all events within a specific correlation window.
///
/// # Preconditions
/// - window_id is a valid UUID v4 string
///
/// # Postconditions
/// - Returns the window with its event list if found
///
/// # Errors
/// - `Error::Validation("window not found")` if window_id does not exist
pub fn get_window(&self, window_id: &str) -> Result<CorrelationWindow>;

/// Returns all currently detected recurring patterns.
///
/// # Postconditions
/// - Returns a snapshot clone of all recurring patterns
/// - Patterns are ordered by confidence descending
pub fn get_recurring_patterns(&self) -> Vec<RecurringPattern>;

/// Returns events from the buffer matching the given channel and time range.
///
/// # Preconditions
/// - channel is one of the 6 default channels
/// - start <= end
///
/// # Postconditions
/// - Returns events ordered by timestamp ascending
///
/// # Errors
/// - `Error::Validation("invalid channel")` if channel is unknown
/// - `Error::Validation("invalid time range")` if start > end
pub fn query_events(
    &self,
    channel: &str,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
) -> Result<Vec<CorrelatedEvent>>;
```

### 4.4 Maintenance Methods

```rust
/// Removes events older than window_size_ms from the buffer and closes
/// expired correlation windows.
///
/// # Postconditions
/// - All events with timestamp < (now - window_size_ms) removed
/// - Expired windows moved to closed state
/// - Buffer utilization updated in stats
///
/// # Complexity: O(n) where n = buffer size
pub fn prune_old_events(&self) -> usize;

/// Clears all recurring patterns.
///
/// # Postconditions
/// - recurring_patterns is empty
/// - stats.recurring_patterns = 0
pub fn clear_patterns(&self);

/// Returns current aggregate statistics.
///
/// # Postconditions
/// - Returns a snapshot of CorrelationStats
/// - stats.buffer_utilization is current
pub fn stats(&self) -> CorrelationStats;

/// Runs the periodic detection algorithm against the current buffer.
///
/// # Postconditions
/// - recurring_patterns updated with any newly detected patterns
/// - Existing patterns updated with new occurrence data
///
/// # Complexity: O(n log n) per (event_type, channel) group
pub fn detect_periodic_patterns(&self) -> Vec<RecurringPattern>;
```

---

## 5. Buffer Management

### 5.1 FIFO Eviction Policy

| Trigger | Action |
|---------|--------|
| Buffer size reaches `max_buffer_size` | Remove oldest event (front of VecDeque) |
| FIFO eviction fails (theoretically impossible) | Clear oldest 10% of buffer |
| Correlation window expires (`end_time + window_size_ms < now`) | Close window, retain events |

### 5.2 Buffer Lifecycle

```
1. Event arrives via ingest_event()
2. IF buffer.len() >= max_buffer_size:
     EVICT oldest event from front of VecDeque
3. Compute correlations against existing buffer
4. Push new CorrelatedEvent to back of VecDeque
5. Update or create CorrelationWindow
6. Periodically run prune_old_events() (recommended: every 1000 ingestions)
```

### 5.3 Recurring Pattern Persistence

| Property | Behavior |
|----------|----------|
| Lifetime | Persists until `clear_patterns()` is called |
| Update | Existing patterns updated on re-detection (occurrence_count, interval stats) |
| Independence | Not affected by buffer eviction (patterns survive buffer rotation) |
| Memory Bound | No hard limit; monitored via `stats().recurring_patterns` |

---

## 6. Performance Characteristics

| Operation | Time Complexity | Space Complexity | Expected Latency |
|-----------|----------------|------------------|------------------|
| `ingest_event` | O(buffer_size * max_correlations_per_event) | O(max_correlations_per_event) | <20ms |
| `get_correlations` | O(1) via HashMap lookup | O(correlations_count) | <1ms |
| `get_window` | O(1) via HashMap lookup | O(events_in_window) | <1ms |
| `get_recurring_patterns` | O(p log p) where p = pattern count | O(p) | <1ms |
| `query_events` | O(n) scan of buffer | O(k) where k = matching events | <5ms |
| `detect_periodic_patterns` | O(n log n) per group | O(n) | <10ms |
| `prune_old_events` | O(n) | O(1) | <5ms |
| `stats` | O(1) | O(type_count) | <1ms |

### Memory Footprint

| Component | Estimate |
|-----------|----------|
| Event buffer (10,000 events) | ~4 MB (est. 400 bytes/event) |
| Correlation windows (active) | ~100 KB (est. 100 windows) |
| Recurring patterns | ~10 KB (est. 50 patterns) |
| Index structures (HashMaps) | ~500 KB |
| **Total** | **~5 MB** |

---

## 7. Error Conditions

| Error | Cause | Recovery |
|-------|-------|----------|
| `Error::Validation("buffer full")` | Buffer at max capacity and FIFO eviction failed | Clear oldest 10% of buffer |
| `Error::Validation("invalid channel")` | Channel name not in default channels list | Log warning and skip event |
| `Error::Validation("event too old")` | Event timestamp > window_size_ms in the past | Skip silently (event is stale) |
| `Error::Validation("invalid time range")` | query_events called with start > end | Return error to caller |
| `Error::Validation("window not found")` | get_window called with unknown window_id | Return error to caller |
| `Error::Validation("invalid config")` | Config parameter out of valid range | Return error from constructor |

### Error Rate Expectations

| Scenario | Expected Error Rate |
|----------|-------------------|
| Normal operation | < 0.01% (mostly stale events) |
| High load (>1000 events/sec) | < 0.1% (buffer pressure) |
| Service outage | 0% errors (events simply not generated) |

---

## 8. Configuration (TOML)

```toml
[observer.log_correlator]
window_size_ms = 5000
max_buffer_size = 10000
min_correlation_confidence = 0.6
min_recurring_count = 3
temporal_tolerance_ms = 500
max_correlations_per_event = 20
periodic_stddev_ratio = 0.2
```

---

## 9. Testing Matrix

| Test Category | Count | Description |
|---------------|-------|-------------|
| Temporal correlation | 10 | Window accuracy, boundary conditions (exact tolerance, off-by-one), same-channel rejection, confidence gradient from 0.6 to 1.0, simultaneous events |
| Causal correlation | 8 | Dependency chain detection, upstream ordering validation, strict temporal ordering, multi-hop causal chains, confidence decay with distance |
| Resonance detection | 8 | Cross-layer same-type matching, minimum 2-layer requirement, confidence scaling (2/6 through 6/6), window boundary enforcement |
| Cascade detection | 8 | Failure propagation chains (depth 1-5), confidence capping at 0.95, cycle prevention via visited set, multi-branch cascades |
| Periodic detection | 8 | Regularity threshold (stddev/mean = 0.19 passes, 0.21 fails), minimum occurrence count, pattern update on re-detection, confidence calculation accuracy |
| Buffer management | 5 | FIFO eviction at capacity, prune_old_events correctness, buffer utilization tracking, emergency 10% clear, zero-event buffer |
| Edge cases | 3 | Empty buffer ingestion, single event (no correlations possible), max_correlations_per_event cap enforcement |
| **Total** | **50** | |

### Test Invariants

| Invariant | Assertion |
|-----------|-----------|
| Buffer bounded | `buffer.len() <= max_buffer_size` at all times |
| Confidence bounded | All `confidence` values in [0.0, 1.0] |
| Cross-channel only (Temporal) | No Temporal links between events on the same channel |
| Causal ordering | All Causal links have `time_delta_ms > 0` |
| Cascade depth | Cascade confidence never exceeds 0.95 |
| Periodic regularity | All RecurringPatterns have `stddev / mean < periodic_stddev_ratio` |
| No self-correlation | No event is ever linked to itself |

---

## 10. Cross-Spec Dependencies

| Dependency | Source Spec | Integration Point |
|------------|-------------|-------------------|
| EventBus channels (6) | [PIPELINE_SPEC.md](../PIPELINE_SPEC.md) | Subscription to health, remediation, learning, consensus, integration, metrics |
| EventRecord type | M23 Event Bus (L4) | Input to ingest_event() |
| Service dependency graph | [SERVICE_SPEC.md](../SERVICE_SPEC.md) | Cascade detection requires dependency map |
| Error taxonomy | M01 Error (L1) | Error::Validation variant |
| 12D Tensor D6 (health) | [TENSOR_SPEC.md](../TENSOR_SPEC.md) | Health events carry tensor health dimension |
| Escalation integration | [ESCALATION_SPEC.md](../ESCALATION_SPEC.md) | Cascade detection feeds L2/L3 escalation |
| Emergence Detector (M38) | [EMERGENCE_DETECTOR_SPEC.md](EMERGENCE_DETECTOR_SPEC.md) | Downstream consumer of CorrelatedEvents |

---

## 11. ObserverBus Output

The LogCorrelator publishes enriched correlation data to the `observer` channel (L7 extension of the EventBus) for consumption by downstream Observer Layer modules.

| Output Event | Trigger | Consumer |
|--------------|---------|----------|
| `correlation.temporal` | Temporal link discovered | M38 Emergence Detector |
| `correlation.causal` | Causal link discovered | M38 Emergence Detector |
| `correlation.resonance` | Resonance detected | M38 Emergence Detector |
| `correlation.cascade` | Cascade chain detected | M38 Emergence Detector, Escalation |
| `correlation.periodic` | New recurring pattern | M38 Emergence Detector |

---

## 12. Version History

| Version | Date | Changes |
|---------|------|---------|
| 1.0.0 | 2026-01-29 | Initial specification |

---

*Generated: 2026-01-29 | The Maintenance Engine v1.0.0 | L7 Observer Layer*
