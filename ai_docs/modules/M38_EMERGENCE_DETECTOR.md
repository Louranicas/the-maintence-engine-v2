# Module M38: Emergence Detector

> **M38_EMERGENCE_DETECTOR** | Emergent system behavior detection | Layer: L7 Observer | [Back to Index](INDEX.md)

---

## Navigation

| Direction | Link |
|-----------|------|
| Up | [INDEX.md](INDEX.md) |
| Layer | [L07_OBSERVER.md](../layers/L07_OBSERVER.md) |
| Dependency | [M37_LOG_CORRELATOR.md](M37_LOG_CORRELATOR.md) |
| Dependency | [M01_ERROR_TAXONOMY.md](M01_ERROR_TAXONOMY.md) |
| Related | [M39_EVOLUTION_CHAMBER.md](M39_EVOLUTION_CHAMBER.md) |
| Related | [M37_LOG_CORRELATOR.md](M37_LOG_CORRELATOR.md) |

---

## Module Specification

### Overview

The Emergence Detector consumes correlated events from M37 (Log Correlator) via the Observer Bus and detects emergent system behaviors that cannot be predicted from individual component states alone. It implements 8 emergence type detectors -- each with its own confidence formula and threshold logic -- and records all detections in a bounded history ring buffer with severity classification.

Emergent behaviors are system-level phenomena arising from cross-layer interactions: cascade failures propagating across services, synergy shifts in cross-service coupling, resonance cycles from feedback loops, attractor formations from converging correlations, phase transitions in system metrics, beneficial self-healing pathways, cascade amplification exceeding safe bounds, and thermal runaway in the V3 homeostasis subsystem.

### Module Properties

| Property | Value |
|----------|-------|
| Module ID | M38 |
| Module Name | Emergence Detector |
| Layer | L7 (Observer) |
| Source File | `src/m7_observer/emergence_detector.rs` |
| LOC | 1,805 |
| Tests | 50 |
| Version | 1.0.0 |
| Lock Order | 3 (after LogCorrelator, before EvolutionChamber) |
| Dependencies | M37 (LogCorrelator), M01 (Error) |
| Dependents | M39 (Evolution Chamber), L7 Coordinator |
| Thread Safety | `parking_lot::RwLock` on all mutable state |
| External Crates | `chrono`, `parking_lot`, `serde`, `uuid` |

---

## Architecture

```
        M37 Log Correlator
              |
              v
     +--------+--------+
     | Observer Bus     |
     +--------+--------+
              |
              v
     +--------+--------+
     | EmergenceDetector|
     | (M38)            |
     +--+--+--+--+--+--+
        |  |  |  |  |  |  |  |
        v  v  v  v  v  v  v  v
       Cascade  Synergy  Resonance  Attractor  Phase   Beneficial  Cascade   Thermal
       Failure  Shift    Cycle      Formation  Trans.  Emergence   Amplif.   Runaway
        |  |  |  |  |  |  |  |
        +--+--+--+--+--+--+--+
              |
     +--------v--------+
     | EmergenceRecord  |
     | (detected_behaviors + behavior_history)
     +--------+---------+
              |
              v
     +--------+--------+
     | M39 Evolution    |
     | Chamber (RALPH)  |
     +-----------------+
```

---

## Emergence Types

### EmergenceType (Enum)

| Variant | Display | Detection Criteria |
|---------|---------|-------------------|
| `CascadeFailure` | `"cascade_failure"` | Correlated failures spanning >= `cascade_depth_threshold` services |
| `SynergyShift` | `"synergy_shift"` | Synergy delta exceeds `synergy_delta_threshold` |
| `ResonanceCycle` | `"resonance_cycle"` | Repeated correlation patterns >= `resonance_min_cycles` |
| `AttractorFormation` | `"attractor_formation"` | Multiple correlations converge on the same service/layer set |
| `PhaseTransition` | `"phase_transition"` | Metric value ratio crosses `PHASE_TRANSITION_RATIO` (2.0x) |
| `BeneficialEmergence` | `"beneficial_emergence"` | Self-healing pathway recovery faster than 5,000ms |
| `CascadeAmplification` | `"cascade_amplification"` | Signal amplification across pipeline stages surpasses threshold |
| `ThermalRunaway` | `"thermal_runaway"` | V3 system temperature exceeds target + margin |

### EmergenceSeverity (Enum)

| Variant | Score Range | Description |
|---------|-------------|-------------|
| `Low` | < 0.4 | Minor emergence, informational |
| `Medium` | [0.4, 0.7) | Notable emergence, monitor |
| `High` | [0.7, 0.9) | Significant emergence, investigate |
| `Critical` | >= 0.9 | Severe emergence, immediate action |

### Severity Formula

```
severity = 0.7 * confidence + 0.3 * breadth
breadth  = clamp(affected_count / 6.0, 0.0, 1.0)
```

### MonitorState (Enum)

| Variant | Description |
|---------|-------------|
| `Watching` | Actively watching for emergence signals |
| `Triggered` | Threshold exceeded; emergence detected |
| `Cooldown` | Recently triggered; suppressing duplicates |

---

## Core Data Structures

### EmergenceRecord

```rust
pub struct EmergenceRecord {
    pub id: String,                       // UUID v4
    pub emergence_type: EmergenceType,    // Classification
    pub confidence: f64,                  // Detection confidence [0.0, 1.0]
    pub severity: f64,                    // Severity assessment [0.0, 1.0]
    pub source_correlations: Vec<String>, // Contributing correlation IDs
    pub affected_layers: Vec<u8>,         // Layer indices affected
    pub affected_services: Vec<String>,   // Service identifiers affected
    pub description: String,             // Human-readable description
    pub detected_at: DateTime<Utc>,       // Detection timestamp
    pub recommended_action: Option<String>, // Suggested remediation
}
```

### EmergenceMonitor

```rust
pub struct EmergenceMonitor {
    pub monitor_id: String,              // UUID v4
    pub behavior_type: EmergenceType,    // Type this monitor tracks
    pub state: MonitorState,             // Current state
    pub accumulated_evidence: Vec<String>, // Evidence toward triggering
    pub confidence: f64,                 // Current accumulated confidence
    pub started_at: DateTime<Utc>,       // Monitor creation time
}
```

### EmergenceStats

```rust
pub struct EmergenceStats {
    pub total_detected: u64,                  // Total emergence records
    pub by_type: HashMap<String, u64>,        // By emergence type name
    pub by_severity_class: HashMap<String, u64>, // By severity class name
    pub active_monitors: usize,               // Currently active monitors
    pub detection_cycles: u64,                // Total detection cycles
}
```

### Type Aliases

| Alias | Target | Purpose |
|-------|--------|---------|
| `EmergentBehavior` | `EmergenceType` | Backward compatibility with `mod.rs` re-exports |

---

## Public API

### Construction

| Method | Signature | Description |
|--------|-----------|-------------|
| `new()` | `-> Self` | Creates with default configuration |
| `with_config(config)` | `-> Self` | Creates with custom configuration |
| `validate_config(config)` | `-> Result<()>` | Validates all configuration parameters |

### Detection Methods

| Method | Signature | Description |
|--------|-----------|-------------|
| `detect(events)` | `-> Result<Vec<EmergenceRecord>>` | Batch detection against correlated events from M37 |
| `detect_cascade(affected, depth, origin)` | `-> Result<Option<EmergenceRecord>>` | Cascade failure detection |
| `detect_synergy_shift(services, delta, pattern_id)` | `-> Result<Option<EmergenceRecord>>` | Synergy shift detection |
| `detect_resonance(layers, frequency_ms, cycles)` | `-> Result<Option<EmergenceRecord>>` | Resonance cycle detection |
| `detect_phase_transition(metric, old_value, new_value)` | `-> Result<Option<EmergenceRecord>>` | Phase transition detection |
| `detect_beneficial_emergence(service, pathway, recovery_ms)` | `-> Result<Option<EmergenceRecord>>` | Self-healing pathway detection |
| `analyze_correlations(correlation_ids, affected_layers)` | `-> Result<Option<EmergenceRecord>>` | Attractor formation detection |
| `detect_cascade_amplification(total_amplification, stage_count, threshold)` | `-> Result<Option<EmergenceRecord>>` | Cascade amplification detection |
| `detect_thermal_runaway(current_temp, target_temp, margin)` | `-> Result<Option<EmergenceRecord>>` | V3 thermal runaway detection |

### Query

| Method | Signature | Description |
|--------|-----------|-------------|
| `get_recent(n)` | `-> Vec<EmergenceRecord>` | Most recent N records from history |
| `recent_emergences(n)` | `-> Vec<EmergenceRecord>` | Alias for `get_recent` (used by L7 coordinator) |
| `get_by_type(emergence_type)` | `-> Vec<EmergenceRecord>` | All records of a given type (current session) |
| `get_record(id)` | `-> Option<EmergenceRecord>` | Lookup by record ID |

### Statistics

| Method | Signature | Description |
|--------|-----------|-------------|
| `stats()` | `-> EmergenceStats` | Aggregate statistics snapshot |
| `history_len()` | `-> usize` | History ring buffer length |
| `active_monitor_count()` | `-> usize` | Currently active monitors |
| `detected_count()` | `-> u64` | Total detected emergence events |
| `config()` | `-> &EmergenceDetectorConfig` | Immutable configuration reference |
| `classify_severity(confidence, affected_count)` | `-> f64` | Static severity classifier |

### Maintenance

| Method | Signature | Description |
|--------|-----------|-------------|
| `clear()` | `-> ()` | Clears all state: behaviors, history, monitors, stats |

---

## Configuration

### EmergenceDetectorConfig

| Parameter | Type | Default | Valid Range | Description |
|-----------|------|---------|-------------|-------------|
| `cascade_depth_threshold` | `u32` | 3 | >= 1 | Min cascade depth for CascadeFailure |
| `synergy_delta_threshold` | `f64` | 0.15 | (0.0, 1.0] | Min synergy delta for SynergyShift |
| `resonance_min_cycles` | `u32` | 3 | >= 2 | Min observed cycles for ResonanceCycle |
| `history_capacity` | `usize` | 1,000 | > 0 | Max emergence records retained |
| `detection_interval_ms` | `u64` | 1,000 | > 0 | Detection cycle interval (ms) |
| `min_confidence` | `f64` | 0.7 | [0.0, 1.0] | Min confidence to register emergence |

### Internal Constants

| Constant | Value | Description |
|----------|-------|-------------|
| `PHASE_TRANSITION_RATIO` | 2.0 | `new_value / old_value` must exceed this for phase change |
| `BENEFICIAL_RECOVERY_CEILING_MS` | 5,000 | Recovery faster than this indicates self-healing |

---

## Confidence Formulas

### CascadeFailure

```
depth_factor   = depth / (depth + 2.0)
breadth_factor = affected_count / (affected_count + 3.0)
confidence     = clamp(0.5 * depth_factor + 0.5 * breadth_factor, 0.0, 1.0)
```

### SynergyShift

```
excess_ratio = |delta| / synergy_delta_threshold
confidence   = clamp(excess_ratio / (excess_ratio + 1.0), 0.0, 1.0)
```

### ResonanceCycle

```
cycle_ratio = cycles / resonance_min_cycles
confidence  = clamp(1.0 - 1.0 / cycle_ratio, 0.0, 1.0)
```

### PhaseTransition

```
max_ratio  = max(new/old, old/new)
confidence = clamp(1.0 - PHASE_TRANSITION_RATIO / max_ratio, 0.0, 1.0)
```

### BeneficialEmergence

```
speed_factor = 1.0 - recovery_ms / BENEFICIAL_RECOVERY_CEILING_MS
path_factor  = pathway_len / (pathway_len + 2.0)
confidence   = clamp(0.6 * speed_factor + 0.4 * path_factor, 0.0, 1.0)
```

### AttractorFormation

```
corr_factor  = corr_count / (corr_count + 3.0)
layer_factor = affected_layers / 6.0
confidence   = clamp(0.6 * corr_factor + 0.4 * layer_factor, 0.0, 1.0)
```

### CascadeAmplification

```
excess_ratio = total_amplification / threshold
confidence   = clamp(1.0 - 1.0 / excess_ratio, 0.0, 1.0)
```

### ThermalRunaway

```
excess       = deviation - margin
max_possible = 1.0 - target_temp - margin
confidence   = clamp(excess / max_possible, 0.0, 1.0)   // 1.0 if max_possible <= 0
```

---

## Metrics

| Metric | Type | Source | Description |
|--------|------|--------|-------------|
| `total_detected` | Counter | `EmergenceStats` | Total emergence records |
| `by_type` | Counter(map) | `EmergenceStats` | Breakdown by emergence type |
| `by_severity_class` | Counter(map) | `EmergenceStats` | Breakdown by severity class |
| `active_monitors` | Gauge | `EmergenceStats` | Currently active monitors |
| `detection_cycles` | Counter | `EmergenceStats` | Total detection cycles executed |

---

## Error Codes

| Error Type | Condition | Raised By |
|------------|-----------|-----------|
| `Error::Config` | `cascade_depth_threshold` < 1 | `validate_config` |
| `Error::Config` | `synergy_delta_threshold` not in (0.0, 1.0] | `validate_config` |
| `Error::Config` | `resonance_min_cycles` < 2 | `validate_config` |
| `Error::Config` | `history_capacity` is 0 | `validate_config` |
| `Error::Config` | `detection_interval_ms` is 0 | `validate_config` |
| `Error::Config` | `min_confidence` not in [0.0, 1.0] | `validate_config` |
| `Error::Validation` | `affected` services is empty | `detect_cascade` |
| `Error::Validation` | `origin` is empty | `detect_cascade` |
| `Error::Validation` | `services` is empty | `detect_synergy_shift` |
| `Error::Validation` | `pattern_id` is empty | `detect_synergy_shift` |
| `Error::Validation` | `layers` is empty | `detect_resonance` |
| `Error::Validation` | `frequency_ms` is 0 | `detect_resonance` |
| `Error::Validation` | `metric` is empty | `detect_phase_transition` |
| `Error::Validation` | metric values are negative | `detect_phase_transition` |
| `Error::Validation` | `service` is empty | `detect_beneficial_emergence` |
| `Error::Validation` | `pathway` is empty | `detect_beneficial_emergence` |
| `Error::Validation` | `correlation_ids` is empty | `analyze_correlations` |
| `Error::Validation` | `stage_count` is 0 | `detect_cascade_amplification` |
| `Error::Validation` | `threshold` not positive | `detect_cascade_amplification` |
| `Error::Validation` | `current_temp` not in [0.0, 1.0] | `detect_thermal_runaway` |
| `Error::Validation` | `target_temp` not in [0.0, 1.0] | `detect_thermal_runaway` |
| `Error::Validation` | `margin` is negative | `detect_thermal_runaway` |

---

## Related Modules

| Module | Relationship | Purpose |
|--------|--------------|---------|
| M37 (Log Correlator) | Dependency | Source of CorrelatedEvents via Observer Bus |
| M01 (Error) | Dependency | Error taxonomy for Result types |
| M39 (Evolution Chamber) | Downstream | Consumes emergence records to drive mutations |
| Observer Bus | Input | Receives correlated events |
| Fitness Evaluator | Sibling | Shared L7 infrastructure |
| V3 Homeostasis | Related | Thermal runaway detection ties to M40-M42 |

---

## Testing

Key test cases (50 total):

```rust
#[test] fn test_new_detector_defaults()           // Verify initial state
#[test] fn test_validate_config_default_passes()   // Default config validation
#[test] fn test_cascade_detection()                // CascadeFailure detection
#[test] fn test_cascade_below_threshold()          // Sub-threshold returns None
#[test] fn test_synergy_shift_positive()           // Positive synergy shift
#[test] fn test_synergy_shift_negative()           // Negative synergy shift
#[test] fn test_resonance_detection()              // ResonanceCycle detection
#[test] fn test_phase_transition()                 // PhaseTransition detection
#[test] fn test_beneficial_emergence()             // Self-healing pathway
#[test] fn test_attractor_formation()              // AttractorFormation detection
#[test] fn test_cascade_amplification()            // CascadeAmplification detection
#[test] fn test_thermal_runaway()                  // ThermalRunaway detection
#[test] fn test_batch_detect()                     // Batch detection from events
```

---

## Performance Characteristics

| Operation | Time | Notes |
|-----------|------|-------|
| Individual detection (any type) | <2ms | Confidence computation + record creation |
| Batch detection (N events) | <10ms | Linear scan with per-event detection |
| History query (get_recent) | <1ms | Ring buffer slice + clone |
| Stats query | <1ms | Lock + clone |
| Severity classification | <1us | Pure arithmetic |

---

## Version History

| Version | Date | Changes |
|---------|------|---------|
| 1.0.0 | 2026-01-29 | Initial implementation (L7 Observer Layer) |
| 1.0.0+HRS-001 | 2026-02-20 | Added CascadeAmplification and ThermalRunaway types |

---

[INDEX.md](INDEX.md) | [L07_OBSERVER.md](../layers/L07_OBSERVER.md) | [M37_LOG_CORRELATOR.md](M37_LOG_CORRELATOR.md) | [M39_EVOLUTION_CHAMBER.md](M39_EVOLUTION_CHAMBER.md)

*The Maintenance Engine v1.0.0 | M38: Emergence Detector*
*Last Updated: 2026-02-20*
