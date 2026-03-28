# M40: ThermalMonitor — Module Specification

**Module ID:** M40
**Layer:** L5 (Learning) — cross-cutting into L7 (Observer)
**File:** `src/m5_learning/thermal.rs`
**Priority:** P1
**Estimated LOC:** ~200
**Tests:** ~15
**Root Causes Addressed:** RC3 (Pattern Accumulation), RC5 (No Thermal Controller)
**Alpha Corrections Applied:** H4, H5, M9, M10

---

## 1. Purpose

The ThermalMonitor adds a **system temperature dimension** to the Maintenance Engine. It computes a composite temperature (T ∈ [0.0, 1.0]) from 4 heat sources, compares against thermal zones, and emits cooling actions when thresholds are breached. This provides the ME with the ability to detect thermal runaway *before* it causes CLI freezes.

**Key Distinction:** SYNTHEX owns the PID controller (SPEC-004) that actively adjusts its own internals. The ME's ThermalMonitor is an *observer* — it reads temperatures and triggers remediation externally if SYNTHEX's internal controls fail.

---

## 2. Struct Definition

```rust
//! # M40: Thermal Monitor
//!
//! System temperature observer for the ULTRAPLATE Developer Environment.
//! Computes composite temperature from 4 heat sources and emits cooling
//! actions when thermal thresholds are breached.
//!
//! ## Layer: L5 (Learning) — cross-cutting
//! ## Dependencies: M01 (Error), M23 (EventBus), M42 (CascadeBridge)
//!
//! ## 12D Tensor Encoding
//! ```text
//! [40/42, 0.0, 5/6, 2, 0, 0.5, health, uptime, synergy, latency, error_rate, temporal]
//! ```

use parking_lot::RwLock;
use crate::{Error, Result};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Target equilibrium temperature.
const TARGET_TEMPERATURE: f64 = 0.50;

/// Warm zone threshold — above this, begin proactive cooling.
const WARM_THRESHOLD: f64 = 0.65;

/// Hot zone threshold — above this, trigger emergency cooling.
const HOT_THRESHOLD: f64 = 0.80;

/// Critical zone threshold — above this, circuit breakers trip.
const CRITICAL_THRESHOLD: f64 = 0.90;

/// Number of heat sources tracked.
const HEAT_SOURCE_COUNT: usize = 4;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Thermal zone classification based on temperature.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThermalZone {
    /// T ∈ [0.0, 0.50) — system is cool, no action needed.
    Cold,
    /// T ∈ [0.50, 0.65) — system at equilibrium.
    Normal,
    /// T ∈ [0.65, 0.80) — proactive cooling recommended.
    Warm,
    /// T ∈ [0.80, 0.90) — emergency cooling required.
    Hot,
    /// T ∈ [0.90, 1.0] — circuit breakers should trip.
    Critical,
}

/// A single heat source contributing to system temperature.
#[derive(Clone, Debug)]
pub struct HeatSource {
    /// Identifier (e.g., "pattern_density", "cascade_amplification").
    pub id: String,
    /// Raw heat value (0.0 – 1.0).
    pub value: f64,
    /// Weight in the composite temperature calculation.
    pub weight: f64,
}

/// Cooling action recommended by the thermal monitor.
#[derive(Clone, Debug)]
pub enum CoolingAction {
    /// No action needed.
    None,
    /// Increase decay rate slightly.
    IncreasedDecay { factor: f64 },
    /// Force an immediate decay cycle.
    ForcedDecayCycle,
    /// Trip cascade circuit breakers.
    TripCascadeBreakers,
    /// Emergency: request SYNTHEX restart.
    EmergencyRestart,
}

/// Snapshot of thermal state for API responses.
#[derive(Clone, Debug)]
pub struct ThermalSnapshot {
    /// Current composite temperature.
    pub temperature: f64,
    /// Current thermal zone.
    pub zone: ThermalZone,
    /// Individual heat source readings.
    pub sources: Vec<HeatSource>,
    /// Most recent cooling action taken.
    pub last_action: CoolingAction,
    /// Timestamp of last reading.
    pub last_reading: Option<chrono::DateTime<chrono::Utc>>,
}

// ---------------------------------------------------------------------------
// ThermalMonitor
// ---------------------------------------------------------------------------

/// System temperature observer.
///
/// Computes composite temperature from 4 heat sources and emits
/// `TemperatureReading` events through the `EventBus`. When temperature
/// exceeds zone thresholds, emits `ThermalAlert` events that trigger
/// `RemediationEngine` actions.
pub struct ThermalMonitor {
    /// Current composite temperature.
    temperature: RwLock<f64>,
    /// Heat source readings.
    sources: RwLock<Vec<HeatSource>>,
    /// Most recent cooling action.
    last_action: RwLock<CoolingAction>,
    /// Timestamp of last temperature calculation.
    last_reading: RwLock<Option<chrono::DateTime<chrono::Utc>>>,
    /// Number of consecutive readings above WARM threshold.
    warm_streak: RwLock<u32>,
}
```

---

## 3. Heat Source Definitions

| ID | Heat Source | Weight | Normalization | Data Source |
|----|-----------|--------|---------------|-------------|
| HS-001 | Pattern Density | 0.25 | `min(pattern_count / 100, 1.0)` | `SELECT COUNT(*) FROM neural_pathways` |
| HS-002 | Cascade Amplification | 0.35 | `min(amplification / 500.0, 1.0)` | M42 CascadeBridge |
| HS-003 | Average Pathway Strength | 0.20 | `avg_strength` (already 0–1) | `SELECT AVG(strength) FROM neural_pathways` |
| HS-004 | Staleness Index | 0.20 | `min(max_age_days / 120.0, 1.0)` | `julianday('now') - julianday(MIN(last_reinforced))` |

**Composite Temperature Formula:**

```
T = Σ(source_i.value × source_i.weight)
  = 0.25 × HS_001 + 0.35 × HS_002 + 0.20 × HS_003 + 0.20 × HS_004
```

**Current System (Broken):**
```
T = 0.25 × min(42/100, 1.0) + 0.35 × min(1814/500, 1.0) + 0.20 × 0.97 + 0.20 × min(117/120, 1.0)
  = 0.25 × 0.42 + 0.35 × 1.0 + 0.20 × 0.97 + 0.20 × 0.975
  = 0.105 + 0.350 + 0.194 + 0.195
  = 0.844  (HOT zone — approaching CRITICAL)
```

**Target System (Fixed):**
```
T = 0.25 × min(30/100, 1.0) + 0.35 × min(115/500, 1.0) + 0.20 × 0.50 + 0.20 × min(45/120, 1.0)
  = 0.25 × 0.30 + 0.35 × 0.23 + 0.20 × 0.50 + 0.20 × 0.375
  = 0.075 + 0.081 + 0.100 + 0.075
  = 0.331  (COLD zone — well below equilibrium)
```

---

## 4. Method Signatures & Logic

### 4.1 `new() → Self`

```rust
impl ThermalMonitor {
    /// Create a new ThermalMonitor with default heat sources.
    #[must_use]
    pub fn new() -> Self {
        let sources = vec![
            HeatSource { id: "pattern_density".into(),       value: 0.0, weight: 0.25 },
            HeatSource { id: "cascade_amplification".into(), value: 0.0, weight: 0.35 },
            HeatSource { id: "avg_pathway_strength".into(),  value: 0.0, weight: 0.20 },
            HeatSource { id: "staleness_index".into(),       value: 0.0, weight: 0.20 },
        ];
        Self {
            temperature: RwLock::new(0.0),
            sources: RwLock::new(sources),
            last_action: RwLock::new(CoolingAction::None),
            last_reading: RwLock::new(None),
            warm_streak: RwLock::new(0),
        }
    }
}
```

### 4.2 `update_source(id, value) → Result<()>`

Updates a single heat source reading. Called by M42 (for cascade) or by the background task (for DB sources).

```rust
/// Update a heat source value by ID.
///
/// # Errors
/// Returns `Error::Validation` if the source ID is unknown.
pub fn update_source(&self, id: &str, value: f64) -> Result<()> {
    let clamped = value.clamp(0.0, 1.0);
    let mut sources = self.sources.write();
    let source = sources.iter_mut()
        .find(|s| s.id == id)
        .ok_or_else(|| Error::Validation(format!("unknown heat source: {id}")))?;
    source.value = clamped;
    Ok(())
}
```

### 4.3 `calculate_temperature() → Result<f64>`

Computes composite temperature from all heat sources. This is the main tick method.

```rust
/// Calculate composite temperature from all heat sources.
///
/// Updates internal temperature state and returns the new value.
/// Also classifies the thermal zone and determines cooling actions.
pub fn calculate_temperature(&self) -> Result<f64> {
    let sources = self.sources.read();
    let temp: f64 = sources.iter()
        .map(|s| s.value * s.weight)
        .sum();
    let clamped = temp.clamp(0.0, 1.0);

    // Update internal state
    *self.temperature.write() = clamped;
    *self.last_reading.write() = Some(chrono::Utc::now());

    // Determine cooling action based on zone
    let zone = Self::classify_zone(clamped);
    let action = self.determine_cooling_action(clamped, zone);
    *self.last_action.write() = action;

    // Track warm streak for graduated response
    if clamped >= WARM_THRESHOLD {
        *self.warm_streak.write() += 1;
    } else {
        *self.warm_streak.write() = 0;
    }

    Ok(clamped)
}
```

### 4.4 `classify_zone(temperature) → ThermalZone`

```rust
/// Classify temperature into a thermal zone.
#[must_use]
const fn classify_zone(temp: f64) -> ThermalZone {
    if temp < TARGET_TEMPERATURE { ThermalZone::Cold }
    else if temp < WARM_THRESHOLD { ThermalZone::Normal }
    else if temp < HOT_THRESHOLD { ThermalZone::Warm }
    else if temp < CRITICAL_THRESHOLD { ThermalZone::Hot }
    else { ThermalZone::Critical }
}
```

### 4.5 `determine_cooling_action(temp, zone) → CoolingAction`

```rust
/// Determine the appropriate cooling action for the current state.
fn determine_cooling_action(&self, temp: f64, zone: ThermalZone) -> CoolingAction {
    let streak = *self.warm_streak.read();
    match zone {
        ThermalZone::Cold | ThermalZone::Normal => CoolingAction::None,
        ThermalZone::Warm => {
            // Graduated response: increase decay after 3 consecutive warm readings
            if streak >= 3 {
                CoolingAction::IncreasedDecay { factor: 1.5 }
            } else {
                CoolingAction::None
            }
        }
        ThermalZone::Hot => CoolingAction::ForcedDecayCycle,
        ThermalZone::Critical => {
            if streak >= 2 {
                CoolingAction::EmergencyRestart
            } else {
                CoolingAction::TripCascadeBreakers
            }
        }
    }
}
```

### 4.6 `snapshot() → ThermalSnapshot`

```rust
/// Create an immutable snapshot of current thermal state.
///
/// Used by HTTP handlers for API responses.
#[must_use]
pub fn snapshot(&self) -> ThermalSnapshot {
    ThermalSnapshot {
        temperature: *self.temperature.read(),
        zone: Self::classify_zone(*self.temperature.read()),
        sources: self.sources.read().clone(),
        last_action: self.last_action.read().clone(),
        last_reading: *self.last_reading.read(),
    }
}
```

---

## 5. Background Task (30s Interval)

```rust
// In main.rs spawn_background_tasks():
tokio::spawn(async move {
    let mut interval = tokio::time::interval(Duration::from_secs(30));
    loop {
        interval.tick().await;

        // 1. Read DB heat sources
        if let Ok(pattern_count) = db.query_scalar::<i64>(
            "SELECT COUNT(*) FROM neural_pathways"
        ).await {
            let normalized = (pattern_count as f64 / 100.0).min(1.0);
            let _ = thermal_monitor.update_source("pattern_density", normalized);
        }

        if let Ok(avg_strength) = db.query_scalar::<f64>(
            "SELECT AVG(strength) FROM neural_pathways"
        ).await {
            let _ = thermal_monitor.update_source("avg_pathway_strength", avg_strength);
        }

        if let Ok(max_age) = db.query_scalar::<f64>(
            "SELECT julianday('now') - julianday(MIN(last_reinforced)) FROM neural_pathways"
        ).await {
            let normalized = (max_age / 120.0).min(1.0);
            let _ = thermal_monitor.update_source("staleness_index", normalized);
        }

        // 2. Cascade source updated by M42 CascadeBridge (separate task)

        // 3. Calculate composite temperature
        if let Ok(temp) = thermal_monitor.calculate_temperature() {
            // 4. Publish to EventBus (string-based API — Alpha Correction H5)
            let snapshot = thermal_monitor.snapshot();
            let _ = event_bus.publish(
                "thermal",
                "TemperatureReading",
                &serde_json::json!({
                    "temperature": temp,
                    "zone": format!("{:?}", snapshot.zone),
                    "sources": snapshot.sources.iter().map(|s| s.value).collect::<Vec<_>>(),
                }).to_string(),
                "thermal_monitor",
            );

            // 5. If above WARM, emit ThermalAlert
            if temp >= WARM_THRESHOLD {
                let _ = event_bus.publish(
                    "thermal",
                    "ThermalAlert",
                    &serde_json::json!({
                        "temperature": temp,
                        "zone": format!("{:?}", snapshot.zone),
                        "action": format!("{:?}", snapshot.last_action),
                    }).to_string(),
                    "thermal_monitor",
                );
            }
        }
    }
});
```

---

## 6. Cross-Module Integration

### 6.1 Inbound Data Flow

```
M42 CascadeBridge ──[update_source("cascade_amplification", normalized)]──► M40
hebbian_pulse.db  ──[SELECT COUNT(*), AVG(strength), MIN(last_reinforced)]──► M40
```

### 6.2 Outbound Data Flow

```
M40 ──[TemperatureReading]──► M38 EmergenceDetector (thermal runaway detection)
M40 ──[TemperatureReading]──► M39 EvolutionChamber (RALPH parameter tuning)
M40 ──[TemperatureReading]──► FitnessEvaluator (D6 health dimension)
M40 ──[ThermalAlert]──► M14 RemediationEngine (cooling actions)
M40 ──[ThermalAlert]──► M12 CircuitBreakerRegistry (trip on critical)
```

### 6.3 Engine.rs Wiring

```rust
// In Engine struct, add field:
pub struct Engine {
    // ... existing fields ...
    thermal_monitor: ThermalMonitor,
}

// In Engine::new():
thermal_monitor: ThermalMonitor::new(),

// Accessor:
/// Returns a reference to the thermal monitor.
pub const fn thermal_monitor(&self) -> &ThermalMonitor {
    &self.thermal_monitor
}
```

### 6.4 HTTP Handler

```rust
// GET /api/thermal
async fn get_thermal_status(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let snapshot = state.engine.thermal_monitor().snapshot();
    Json(serde_json::json!({
        "temperature": snapshot.temperature,
        "zone": format!("{:?}", snapshot.zone),
        "sources": snapshot.sources.iter().map(|s| {
            serde_json::json!({
                "id": s.id,
                "value": s.value,
                "weight": s.weight,
            })
        }).collect::<Vec<_>>(),
        "last_action": format!("{:?}", snapshot.last_action),
        "last_reading": snapshot.last_reading.map(|t| t.to_rfc3339()),
    }))
}
```

---

## 7. Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_thermal_zone_classification() {
        assert_eq!(ThermalMonitor::classify_zone(0.30), ThermalZone::Cold);
        assert_eq!(ThermalMonitor::classify_zone(0.55), ThermalZone::Normal);
        assert_eq!(ThermalMonitor::classify_zone(0.70), ThermalZone::Warm);
        assert_eq!(ThermalMonitor::classify_zone(0.85), ThermalZone::Hot);
        assert_eq!(ThermalMonitor::classify_zone(0.95), ThermalZone::Critical);
    }

    #[test]
    fn test_update_source_valid() {
        let monitor = ThermalMonitor::new();
        assert!(monitor.update_source("pattern_density", 0.5).is_ok());
    }

    #[test]
    fn test_update_source_unknown_id() {
        let monitor = ThermalMonitor::new();
        assert!(monitor.update_source("nonexistent", 0.5).is_err());
    }

    #[test]
    fn test_update_source_clamping() {
        let monitor = ThermalMonitor::new();
        let _ = monitor.update_source("pattern_density", 2.0); // clamped to 1.0
        // Verify via snapshot (public API — Alpha Correction H6/M9)
        let snapshot = monitor.snapshot();
        let pd = snapshot.sources.iter().find(|s| s.id == "pattern_density");
        assert!(pd.is_some(), "pattern_density source should exist");
        if let Some(source) = pd {
            assert!((source.value - 1.0).abs() < f64::EPSILON);
        }
    }

    #[test]
    fn test_calculate_temperature_cold() {
        let monitor = ThermalMonitor::new();
        // All sources at 0 → T = 0 (Alpha Correction M9: no .unwrap())
        let result = monitor.calculate_temperature();
        assert!(result.is_ok());
        if let Ok(temp) = result {
            assert!(temp < TARGET_TEMPERATURE);
        }
    }

    #[test]
    fn test_calculate_temperature_hot() {
        let monitor = ThermalMonitor::new();
        let _ = monitor.update_source("pattern_density", 0.42);
        let _ = monitor.update_source("cascade_amplification", 1.0);
        let _ = monitor.update_source("avg_pathway_strength", 0.97);
        let _ = monitor.update_source("staleness_index", 0.975);
        let result = monitor.calculate_temperature();
        assert!(result.is_ok());
        if let Ok(temp) = result {
            assert!(temp > HOT_THRESHOLD, "should be ~0.844: {temp}");
        }
    }

    #[test]
    fn test_cooling_action_warm_streak() {
        let monitor = ThermalMonitor::new();
        // Simulate 3 warm readings
        *monitor.warm_streak.write() = 3;
        let action = monitor.determine_cooling_action(0.70, ThermalZone::Warm);
        // Alpha Correction M10: matches!() must be wrapped in assert!()
        assert!(matches!(action, CoolingAction::IncreasedDecay { .. }));
    }

    #[test]
    fn test_cooling_action_hot() {
        let monitor = ThermalMonitor::new();
        let action = monitor.determine_cooling_action(0.85, ThermalZone::Hot);
        // Alpha Correction M10: matches!() must be wrapped in assert!()
        assert!(matches!(action, CoolingAction::ForcedDecayCycle));
    }

    #[test]
    fn test_snapshot_consistency() {
        let monitor = ThermalMonitor::new();
        let result = monitor.calculate_temperature();
        assert!(result.is_ok());
        let snap = monitor.snapshot();
        // Verify snapshot matches calculated result (Alpha Correction M9)
        if let Ok(temp) = result {
            assert!((snap.temperature - temp).abs() < f64::EPSILON);
        }
        assert_eq!(snap.sources.len(), HEAT_SOURCE_COUNT);
    }
}
```

---

## 8. NAM Integration

This section addresses NAM gaps NAM-G15 through NAM-G20 (see [NAM_GAP_ANALYSIS.md](NAM_GAP_ANALYSIS.md)).

### 8.1 Escalation Gates (R-NAM-01) — Addresses NAM-G15, NAM-G16

Every cooling action MUST route through the escalation tier system. `EmergencyRestart` requires L3 PBFT consensus (27/40 quorum). `TripCascadeBreakers` and `ForcedDecayCycle` require L1 human notification.

```rust
/// Cooling action with NAM escalation tier enforcement.
#[derive(Clone, Debug)]
pub enum CoolingAction {
    /// No action needed — L0 auto-execute.
    None,
    /// Increase decay rate slightly — L0 auto-execute (low impact).
    IncreasedDecay { factor: f64 },
    /// Force an immediate decay cycle — L1 notify human.
    ForcedDecayCycle,
    /// Trip cascade circuit breakers — L1 notify human.
    TripCascadeBreakers,
    /// Emergency: request SYNTHEX restart — L3 PBFT consensus required.
    EmergencyRestart,
}

/// Determine the appropriate cooling action with NAM escalation enforcement.
fn determine_cooling_action(&self, temp: f64, zone: ThermalZone) -> CoolingAction {
    let streak = *self.warm_streak.read();
    match zone {
        ThermalZone::Cold | ThermalZone::Normal => CoolingAction::None,
        ThermalZone::Warm => {
            if streak >= 3 {
                CoolingAction::IncreasedDecay { factor: 1.5 } // L0: low impact
            } else {
                CoolingAction::None
            }
        }
        ThermalZone::Hot => CoolingAction::ForcedDecayCycle, // Gated by L1 at execution
        ThermalZone::Critical => {
            if streak >= 2 {
                CoolingAction::EmergencyRestart // Gated by L3 at execution
            } else {
                CoolingAction::TripCascadeBreakers // Gated by L1 at execution
            }
        }
    }
}
```

**Escalation enforcement in background task:**

```rust
// After determine_cooling_action():
match &snapshot.last_action {
    CoolingAction::EmergencyRestart => {
        // L3: PBFT Consensus required (ESCALATION_SPEC.md)
        let _ = event_bus.publish(
            "escalation", "ConsensusRequired",
            &serde_json::json!({
                "proposed_action": "EmergencyRestart",
                "tier": "L3",
                "reason": format!("Temperature {temp:.3} in CRITICAL zone, streak {}", streak),
                "quorum_required": 27,
                "timeout_secs": 60,
                "source": "thermal_monitor",
            }).to_string(),
            "thermal_monitor",
        );
        // Action MUST NOT execute until consensus is reached
    }
    CoolingAction::ForcedDecayCycle | CoolingAction::TripCascadeBreakers => {
        // L1: Notify Human @0.A
        let _ = event_bus.publish(
            "escalation", "HumanNotification",
            &serde_json::json!({
                "agent": "@0.A",
                "tier": "L1",
                "proposed_action": format!("{:?}", snapshot.last_action),
                "temperature": temp,
                "zone": format!("{:?}", snapshot.zone),
                "timeout_secs": 300,
            }).to_string(),
            "thermal_monitor",
        );
        // Proceed after notification — no blocking wait at L1
    }
    _ => {} // L0: auto-execute
}
```

### 8.2 12D Tensor Integration (R-NAM-03) — Addresses NAM-G18

Temperature state MUST propagate to the 12D tensor encoding for field visualization (R4).

```rust
// After calculate_temperature():
let clamped = temp.clamp(0.0, 1.0);

// R4: Update 12D tensor dimensions
// D6 (health) ← inverse of temperature: healthy system = low temperature
tensor_store.update_dimension(
    "maintenance_engine",
    TensorDimension::Health, // D6
    1.0 - clamped,
);

// D10 (error_rate) ← zone severity mapping
let zone_severity = match Self::classify_zone(clamped) {
    ThermalZone::Cold => 0.0,
    ThermalZone::Normal => 0.1,
    ThermalZone::Warm => 0.4,
    ThermalZone::Hot => 0.7,
    ThermalZone::Critical => 1.0,
};
tensor_store.update_dimension(
    "maintenance_engine",
    TensorDimension::ErrorRate, // D10
    zone_severity,
);
```

| Tensor Dimension | Update | Trigger | Value Range |
|------------------|--------|---------|-------------|
| D6 (health) | `1.0 - temperature` | Every `calculate_temperature()` | 0.0–1.0 |
| D10 (error_rate) | Zone severity mapping | Every zone classification | 0.0–1.0 |

### 8.3 Self-Query Introspection (R-NAM-04) — Addresses NAM-G19

After each temperature calculation, M40 validates its readings against historical baselines.

```rust
impl ThermalMonitor {
    /// R1: Self-assess temperature reading accuracy.
    ///
    /// Compares current reading against rolling history to detect
    /// anomalous spikes that may indicate sensor error rather than
    /// genuine thermal issues.
    fn self_query(&self, current_temp: f64) -> SelfAssessment {
        let history = self.temperature_history.read();
        let recent: Vec<f64> = history.iter().rev().take(5).copied().collect();

        let avg = if recent.is_empty() {
            current_temp
        } else {
            recent.iter().sum::<f64>() / recent.len() as f64
        };

        let deviation = (current_temp - avg).abs();

        SelfAssessment {
            reading_plausible: deviation < 0.3, // >0.3 jump is suspicious
            deviation_from_baseline: deviation,
            recommendation: if deviation >= 0.3 {
                "Large temperature jump detected — verify heat source readings before acting"
            } else {
                "Reading consistent with historical baseline"
            },
        }
    }
}
```

**Integration in background task:**

```rust
if let Ok(temp) = thermal_monitor.calculate_temperature() {
    // R1: Self-query before acting on reading
    let assessment = thermal_monitor.self_query(temp);
    if !assessment.reading_plausible {
        tracing::warn!(
            deviation = assessment.deviation_from_baseline,
            "Temperature self-query: large deviation detected — suppressing action"
        );
        continue; // Skip cooling action this cycle
    }
    // ... proceed with normal event publishing and cooling ...
}
```

### 8.4 Human @0.A Notification (R-NAM-06) — Addresses NAM-G20

Human @0.A is notified when temperature enters CRITICAL zone or when cooling actions are taken.

```rust
// In background task, after ThermalAlert event:
if temp >= CRITICAL_THRESHOLD {
    // R5: Notify Human @0.A — CRITICAL temperature
    let _ = event_bus.publish(
        "escalation", "HumanNotification",
        &serde_json::json!({
            "agent": "@0.A",
            "event": "thermal_critical",
            "temperature": temp,
            "zone": format!("{:?}", snapshot.zone),
            "recommended_action": format!("{:?}", snapshot.last_action),
            "urgency": "CRITICAL",
            "timeout_secs": 60,
            "sources": snapshot.sources.iter().map(|s| {
                serde_json::json!({ "id": s.id, "value": s.value })
            }).collect::<Vec<_>>(),
        }).to_string(),
        "thermal_monitor",
    );
}
```

### 8.5 NAM Compliance Summary

| NAM Requirement | Gap ID | Status | Implementation |
|-----------------|--------|--------|----------------|
| R1 SelfQuery | NAM-G19 | ADDRESSED | `self_query()` validates readings against baselines |
| R3 DissentCapture | NAM-G17 | PARTIAL | Dissent possible via L3 consensus on EmergencyRestart |
| R4 FieldVisualization | NAM-G18 | ADDRESSED | D6, D10 tensor updates after each calculation |
| R5 HumanAsAgent | NAM-G20 | ADDRESSED | @0.A notified on CRITICAL zone, L1/L3 escalation |
| Escalation | NAM-G15 | ADDRESSED | EmergencyRestart→L3, ForcedDecay/TripBreakers→L1 |
| Escalation | NAM-G16 | ADDRESSED | TripCascadeBreakers→L1 notification |

**Projected NAM Score: 8/100 → 72/100**

### 8.6 NAM-Specific Tests

```rust
#[cfg(test)]
mod nam_tests {
    use super::*;

    #[test]
    fn test_self_query_stable_reading() {
        let monitor = ThermalMonitor::new();
        // Simulate stable history
        for _ in 0..5 {
            let _ = monitor.update_source("pattern_density", 0.3);
            let _ = monitor.calculate_temperature();
        }
        let temp = *monitor.temperature.read();
        let assessment = monitor.self_query(temp);
        assert!(assessment.reading_plausible);
    }

    #[test]
    fn test_self_query_anomalous_spike() {
        let monitor = ThermalMonitor::new();
        // Simulate stable low readings, then spike
        let assessment = monitor.self_query(0.95); // Large jump from 0.0 baseline
        // First reading has no history, so deviation = 0
        // After building history:
        for _ in 0..5 {
            let _ = monitor.update_source("pattern_density", 0.1);
            let _ = monitor.calculate_temperature();
        }
        let spike_assessment = monitor.self_query(0.9);
        assert!(!spike_assessment.reading_plausible, "0.9 from ~0.025 should be implausible");
    }

    #[test]
    fn test_escalation_tier_emergency_restart() {
        // EmergencyRestart MUST require L3 PBFT consensus
        let monitor = ThermalMonitor::new();
        *monitor.warm_streak.write() = 3;
        let action = monitor.determine_cooling_action(0.95, ThermalZone::Critical);
        assert!(matches!(action, CoolingAction::EmergencyRestart));
        // The action itself doesn't encode the tier — the background task
        // enforces L3 by publishing to "escalation"/"ConsensusRequired"
    }

    #[test]
    fn test_escalation_tier_trip_breakers() {
        // TripCascadeBreakers MUST require L1 notification
        let monitor = ThermalMonitor::new();
        let action = monitor.determine_cooling_action(0.92, ThermalZone::Critical);
        assert!(matches!(action, CoolingAction::TripCascadeBreakers));
    }
}
```

---

*Document: ME_MODULE_M40_THERMAL_MONITOR.md (NAM Integration Applied — R-NAM-01, R-NAM-03, R-NAM-04, R-NAM-06)*
*Alpha Corrections: H4, H5, M9, M10*
*Location: generation_1_bug_fix/ai_docs/*
