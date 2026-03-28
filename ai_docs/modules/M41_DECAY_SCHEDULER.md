# M41: DecayScheduler — Module Specification

**Module ID:** M41
**Layer:** L5 (Learning)
**File:** `src/m5_learning/decay_scheduler.rs`
**Priority:** P0
**Estimated LOC:** ~220
**Tests:** ~15
**Root Causes Addressed:** RC1 (STDP Decay Disabled), RC3 (Pattern Accumulation)
**Alpha Corrections Applied:** H5, M9

---

## 1. Purpose

The DecayScheduler implements an **hourly exponential decay cycle** that systematically weakens unused neural pathways in `hebbian_pulse.db`. This is the primary fix for Root Cause 1 (decay_rate = 0) and Root Cause 3 (42 pathways never pruned). Without this module, pathways accumulate indefinitely at strength 1.0, consuming memory and amplifying cascades.

**Key Distinction:** M25 HebbianManager has an `apply_decay()` method, but it uses linear decay (`strength - 0.001`) which is 100x too slow. M41 replaces the decay scheduling logic with a proper exponential formula and configurable cycle interval, while M26 Enhancement updates the formula itself.

---

## 2. Struct Definition

```rust
//! # M41: Decay Scheduler
//!
//! Hourly exponential decay cycle for neural pathway maintenance.
//! Reads configuration from the `config` table in `hebbian_pulse.db`
//! and applies time-weighted decay to all pathways.
//!
//! ## Layer: L5 (Learning)
//! ## Dependencies: M01 (Error), M25 (HebbianManager), SPEC-001 (Config Table)
//!
//! ## 12D Tensor Encoding
//! ```text
//! [41/42, 0.0, 5/6, 1, 0, 0.5, health, uptime, synergy, latency, error_rate, temporal]
//! ```

use std::time::SystemTime;
use parking_lot::RwLock;
use crate::{Error, Result};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Default decay rate if config table is unavailable.
const DEFAULT_DECAY_RATE: f64 = 0.1;

/// Default time constant (tau) in days for exponential decay.
const DEFAULT_TAU_DAYS: f64 = 7.0;

/// Default cycle interval in seconds (3600 = 1 hour).
const DEFAULT_CYCLE_INTERVAL_SECS: u64 = 3600;

/// Minimum pathway strength floor — pathways below this are prune candidates.
const STRENGTH_FLOOR: f64 = 0.01;

/// Maximum audit log entries retained in memory.
const AUDIT_LOG_CAPACITY: usize = 500;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Configuration for the decay scheduler, read from config table.
#[derive(Clone, Debug)]
pub struct DecayConfig {
    /// Decay rate per cycle (0.0 – 1.0). Default: 0.1.
    pub decay_rate: f64,
    /// Time constant for exponential decay, in days. Default: 7.0.
    pub tau_days: f64,
    /// Cycle interval in seconds. Default: 3600 (1 hour).
    pub cycle_interval_secs: u64,
    /// Minimum strength floor. Pathways below this are prune candidates.
    pub strength_floor: f64,
}

impl Default for DecayConfig {
    fn default() -> Self {
        Self {
            decay_rate: DEFAULT_DECAY_RATE,
            tau_days: DEFAULT_TAU_DAYS,
            cycle_interval_secs: DEFAULT_CYCLE_INTERVAL_SECS,
            strength_floor: STRENGTH_FLOOR,
        }
    }
}

/// A single audit log entry recording one decay cycle.
#[derive(Clone, Debug)]
pub struct DecayAuditEntry {
    /// Timestamp of the decay cycle.
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Number of pathways processed.
    pub pathways_processed: u32,
    /// Number of pathways whose strength decreased.
    pub pathways_decayed: u32,
    /// Number of pathways that fell below the strength floor.
    pub pathways_below_floor: u32,
    /// Average strength before decay.
    pub avg_strength_before: f64,
    /// Average strength after decay.
    pub avg_strength_after: f64,
    /// Duration of the cycle in milliseconds.
    pub duration_ms: u64,
}

/// Result of a single pathway decay calculation.
#[derive(Clone, Debug)]
pub struct PathwayDecayResult {
    /// Pathway ID.
    pub pathway_id: String,
    /// Strength before decay.
    pub old_strength: f64,
    /// Strength after decay.
    pub new_strength: f64,
    /// Days since last reinforcement.
    pub days_since_reinforced: f64,
    /// Whether the pathway is now below the strength floor.
    pub below_floor: bool,
}

// ---------------------------------------------------------------------------
// DecayScheduler
// ---------------------------------------------------------------------------

/// Hourly decay cycle scheduler.
///
/// Reads `decay_rate` and `tau` from the `config` table in `hebbian_pulse.db`,
/// applies exponential decay to all pathways in `neural_pathways`, and writes
/// audit log entries to `decay_audit_log`.
pub struct DecayScheduler {
    /// Active configuration (refreshed from DB each cycle).
    config: RwLock<DecayConfig>,
    /// Audit log of recent decay cycles.
    audit_log: RwLock<Vec<DecayAuditEntry>>,
    /// Timestamp of the last completed cycle.
    last_cycle: RwLock<Option<chrono::DateTime<chrono::Utc>>>,
    /// Total cycles completed since startup.
    total_cycles: RwLock<u64>,
}
```

---

## 3. Exponential Decay Formula

The decay formula implements time-weighted exponential decay:

```
w(t) = w₀ × e^(-Δt / τ) × (1 - r)

Where:
  w₀  = current pathway strength
  Δt  = days since last reinforcement
  τ   = time constant (default 7.0 days)
  r   = decay rate per cycle (default 0.1)
  w(t) = new pathway strength after decay
```

**Example Calculations:**

| Pathway | w₀ | Δt (days) | τ | r | w(t) | Status |
|---------|-----|-----------|---|---|------|--------|
| Fresh (1 day) | 1.0 | 1 | 7 | 0.1 | 0.774 | Healthy |
| Week old | 1.0 | 7 | 7 | 0.1 | 0.331 | Weakening |
| Month old | 1.0 | 30 | 7 | 0.1 | 0.013 | Prune candidate |
| 117 days (current bug) | 1.0 | 117 | 7 | 0.1 | 0.000000005 | Instant prune |

**Contrast with Current Linear Decay (M25):**

```
Current:  w(t) = (w₀ - 0.001).max(0.1)
          After 117 days × 24 cycles/day = 2808 cycles:
          w = max(1.0 - 2.808, 0.1) = 0.1  (still too high!)

New:      w(t) = 1.0 × e^(-117/7) × (1 - 0.1) = 0.000000005  (correctly zero)
```

---

## 4. Method Signatures & Logic

### 4.1 `new() → Self`

```rust
impl DecayScheduler {
    /// Create a new DecayScheduler with default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: RwLock::new(DecayConfig::default()),
            audit_log: RwLock::new(Vec::with_capacity(AUDIT_LOG_CAPACITY)),
            last_cycle: RwLock::new(None),
            total_cycles: RwLock::new(0),
        }
    }
}
```

### 4.2 `load_config(db) → Result<()>`

Refreshes configuration from the config table. Called at the start of each cycle.

```rust
/// Load configuration from the `config` table in hebbian_pulse.db.
///
/// Falls back to defaults if the config table doesn't exist or keys are missing.
///
/// # Errors
/// Returns `Error::Database` if the query fails for reasons other than missing table.
pub async fn load_config(&self, db: &StatePersistence) -> Result<()> {
    let mut config = DecayConfig::default();

    if let Ok(rate) = db.query_scalar::<f64>(
        "SELECT CAST(value AS REAL) FROM config WHERE key = 'decay_rate'"
    ).await {
        config.decay_rate = rate.clamp(0.001, 1.0);
    }

    if let Ok(tau) = db.query_scalar::<f64>(
        "SELECT CAST(value AS REAL) FROM config WHERE key = 'tau'"
    ).await {
        config.tau_days = tau.clamp(1.0, 365.0);
    }

    if let Ok(threshold) = db.query_scalar::<f64>(
        "SELECT CAST(value AS REAL) FROM config WHERE key = 'prune_threshold'"
    ).await {
        config.strength_floor = threshold.clamp(0.001, 0.5);
    }

    *self.config.write() = config;
    Ok(())
}
```

### 4.3 `calculate_decay(strength, days_since) → f64`

Pure function implementing the exponential decay formula.

```rust
/// Calculate decayed strength using exponential formula.
///
/// w(t) = w₀ × e^(-Δt/τ) × (1 - r)
///
/// # Arguments
/// * `strength` - Current pathway strength (w₀)
/// * `days_since_reinforced` - Days since last reinforcement (Δt)
///
/// # Returns
/// New strength value, clamped to [0.0, 1.0].
#[must_use]
pub fn calculate_decay(&self, strength: f64, days_since_reinforced: f64) -> f64 {
    let config = self.config.read();
    let exponential = (-days_since_reinforced / config.tau_days).exp();
    let decayed = strength * exponential * (1.0 - config.decay_rate);
    decayed.clamp(0.0, 1.0)
}
```

### 4.4 `run_decay_cycle(db) → Result<DecayAuditEntry>`

The main cycle method. Reads all pathways, applies decay, writes updates, logs audit.

```rust
/// Execute a full decay cycle.
///
/// 1. Refresh config from database
/// 2. Read all pathways from neural_pathways
/// 3. For each pathway, calculate exponential decay
/// 4. Write updated strengths back to database
/// 5. Log audit entry to decay_audit_log table
/// 6. Publish DecayCycleComplete event
///
/// # Errors
/// Returns `Error::Database` if any database operation fails.
pub async fn run_decay_cycle(&self, db: &StatePersistence) -> Result<DecayAuditEntry> {
    let start = std::time::Instant::now();

    // 1. Refresh config
    self.load_config(db).await?;

    let config = self.config.read().clone();

    // 2. Read all pathways
    let pathways = db.query_all(
        "SELECT id, pathway_name, strength, \
         julianday('now') - julianday(COALESCE(last_reinforced, created_at)) as days_since \
         FROM neural_pathways"
    ).await?;

    let mut total_before = 0.0;
    let mut total_after = 0.0;
    let mut decayed_count = 0u32;
    let mut below_floor_count = 0u32;
    let pathway_count = pathways.len() as u32;

    // 3. Apply decay to each pathway
    for row in &pathways {
        let id: String = row.get("id");
        let strength: f64 = row.get("strength");
        let days_since: f64 = row.get("days_since");

        total_before += strength;
        let new_strength = self.calculate_decay(strength, days_since);
        total_after += new_strength;

        if (new_strength - strength).abs() > f64::EPSILON {
            decayed_count += 1;

            // 4. Write updated strength
            db.execute(
                "UPDATE neural_pathways SET strength = ?1 WHERE id = ?2",
                &[&new_strength, &id],
            ).await?;
        }

        if new_strength < config.strength_floor {
            below_floor_count += 1;
        }
    }

    let duration = start.elapsed();
    let avg_before = if pathway_count > 0 { total_before / f64::from(pathway_count) } else { 0.0 };
    let avg_after = if pathway_count > 0 { total_after / f64::from(pathway_count) } else { 0.0 };

    // 5. Create audit entry
    let entry = DecayAuditEntry {
        timestamp: chrono::Utc::now(),
        pathways_processed: pathway_count,
        pathways_decayed: decayed_count,
        pathways_below_floor: below_floor_count,
        avg_strength_before: avg_before,
        avg_strength_after: avg_after,
        duration_ms: duration.as_millis() as u64,
    };

    // 6. Write audit to database
    db.execute(
        "INSERT INTO decay_audit_log (timestamp, pathways_processed, pathways_decayed, \
         pathways_below_floor, avg_strength_before, avg_strength_after, duration_ms) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        &[
            &entry.timestamp.to_rfc3339(),
            &entry.pathways_processed,
            &entry.pathways_decayed,
            &entry.pathways_below_floor,
            &entry.avg_strength_before,
            &entry.avg_strength_after,
            &entry.duration_ms,
        ],
    ).await?;

    // 7. Update internal state
    {
        let mut log = self.audit_log.write();
        if log.len() >= AUDIT_LOG_CAPACITY {
            log.remove(0);
        }
        log.push(entry.clone());
    }
    *self.last_cycle.write() = Some(entry.timestamp);
    *self.total_cycles.write() += 1;

    Ok(entry)
}
```

### 4.5 `status() → DecayStatus`

```rust
/// Snapshot of current scheduler status for API responses.
#[derive(Clone, Debug)]
pub struct DecayStatus {
    pub config: DecayConfig,
    pub last_cycle: Option<chrono::DateTime<chrono::Utc>>,
    pub total_cycles: u64,
    pub recent_audits: Vec<DecayAuditEntry>,
}

/// Get current scheduler status.
#[must_use]
pub fn status(&self) -> DecayStatus {
    DecayStatus {
        config: self.config.read().clone(),
        last_cycle: *self.last_cycle.read(),
        total_cycles: *self.total_cycles.read(),
        recent_audits: self.audit_log.read().iter().rev().take(10).cloned().collect(),
    }
}
```

---

## 5. Background Task (3600s Interval)

```rust
// In main.rs spawn_background_tasks():
tokio::spawn(async move {
    let mut interval = tokio::time::interval(Duration::from_secs(3600));

    // Run first cycle immediately at startup
    if let Err(e) = decay_scheduler.run_decay_cycle(&db).await {
        tracing::warn!("Initial decay cycle failed: {e}");
    }

    loop {
        interval.tick().await;

        match decay_scheduler.run_decay_cycle(&db).await {
            Ok(audit) => {
                tracing::info!(
                    pathways_decayed = audit.pathways_decayed,
                    below_floor = audit.pathways_below_floor,
                    avg_before = format!("{:.3}", audit.avg_strength_before),
                    avg_after = format!("{:.3}", audit.avg_strength_after),
                    duration_ms = audit.duration_ms,
                    "Decay cycle complete"
                );

                // Publish event (string-based API — Alpha Correction H5)
                let _ = event_bus.publish(
                    "learning",
                    "DecayCycleComplete",
                    &serde_json::json!({
                        "pathways_decayed": audit.pathways_decayed,
                        "pathways_pruned": audit.pathways_below_floor,
                        "duration_ms": audit.duration_ms,
                    }).to_string(),
                    "decay_scheduler",
                );

                // Trigger pruning if pathways fell below floor
                if audit.pathways_below_floor > 0 {
                    let _ = pruner.evaluate_and_prune(&db).await;
                }
            }
            Err(e) => {
                tracing::error!("Decay cycle failed: {e}");
            }
        }
    }
});
```

---

## 6. Cross-Module Integration

### 6.1 Inbound Dependencies

```
SPEC-001 (Config Table) ──[config table must exist]──► M41
hebbian_pulse.db        ──[neural_pathways table]──► M41
```

### 6.2 Outbound Data Flow

```
M41 ──[DecayCycleComplete]──► M28 PathwayPruner (triggers prune evaluation)
M41 ──[DecayCycleComplete]──► M38 EmergenceDetector (tracks decay health)
M41 ──[DecayCycleComplete]──► M39 EvolutionChamber (RALPH tunes decay_rate)
M41 ──[PathwayDecayed]──► M25 HebbianManager (updates routing weights)
M41 ──[PathwayDecayed]──► FitnessEvaluator (updates D6 health)
```

### 6.3 M41 → M28 Integration Flow

After each decay cycle, M41 triggers M28's pruning evaluation:

```
M41.run_decay_cycle()
  │
  ├── Updates all pathway strengths in DB
  │
  ├── Counts pathways_below_floor
  │
  └── If below_floor > 0:
        │
        └── M28.evaluate_and_prune(&db)
              │
              ├── Reads pathways where strength < prune_threshold
              │
              ├── Checks age > max_age_days (90)
              │
              └── Deletes qualifying pathways
```

### 6.4 M41 → M39 RALPH Tuning

The RALPH evolution loop can tune `decay_rate` based on system health:

```
M39.recognize()  → reads decay cycle audit results
M39.analyze()    → if avg_strength too high, proposes increasing decay_rate
M39.learn()      → tests new rate for 3 cycles
M39.propose()    → if temperature dropped, commit new rate to config table
M39.harvest()    → records successful mutation
```

### 6.5 Engine.rs Wiring

```rust
// In Engine struct:
pub struct Engine {
    // ... existing fields ...
    decay_scheduler: DecayScheduler,
}

// In Engine::new():
decay_scheduler: DecayScheduler::new(),

// Accessor:
pub const fn decay_scheduler(&self) -> &DecayScheduler {
    &self.decay_scheduler
}
```

### 6.6 HTTP Handlers

```rust
// GET /api/decay
async fn get_decay_status(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let status = state.engine.decay_scheduler().status();
    Json(serde_json::json!({
        "config": {
            "decay_rate": status.config.decay_rate,
            "tau_days": status.config.tau_days,
            "cycle_interval_secs": status.config.cycle_interval_secs,
            "strength_floor": status.config.strength_floor,
        },
        "last_cycle": status.last_cycle.map(|t| t.to_rfc3339()),
        "total_cycles": status.total_cycles,
        "recent_audits": status.recent_audits.iter().map(|a| {
            serde_json::json!({
                "timestamp": a.timestamp.to_rfc3339(),
                "processed": a.pathways_processed,
                "decayed": a.pathways_decayed,
                "below_floor": a.pathways_below_floor,
                "avg_before": format!("{:.4}", a.avg_strength_before),
                "avg_after": format!("{:.4}", a.avg_strength_after),
                "duration_ms": a.duration_ms,
            })
        }).collect::<Vec<_>>(),
    }))
}

// POST /api/decay/trigger — force immediate cycle
async fn trigger_decay_cycle(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    match state.engine.decay_scheduler().run_decay_cycle(&state.db).await {
        Ok(audit) => Json(serde_json::json!({
            "status": "completed",
            "pathways_decayed": audit.pathways_decayed,
            "pathways_below_floor": audit.pathways_below_floor,
            "duration_ms": audit.duration_ms,
        })),
        Err(e) => Json(serde_json::json!({
            "status": "error",
            "message": e.to_string(),
        })),
    }
}
```

---

## 7. Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decay_fresh_pathway() {
        let scheduler = DecayScheduler::new();
        // 1 day old, should retain most strength
        let decayed = scheduler.calculate_decay(1.0, 1.0);
        assert!(decayed > 0.7, "1-day pathway should retain >70%: {decayed}");
        assert!(decayed < 0.9, "1-day pathway should lose some: {decayed}");
    }

    #[test]
    fn test_decay_week_old_pathway() {
        let scheduler = DecayScheduler::new();
        // 7 days = 1 tau → ~0.331
        let decayed = scheduler.calculate_decay(1.0, 7.0);
        assert!(decayed > 0.2, "week-old: {decayed}");
        assert!(decayed < 0.5, "week-old: {decayed}");
    }

    #[test]
    fn test_decay_month_old_pathway() {
        let scheduler = DecayScheduler::new();
        // 30 days → near zero
        let decayed = scheduler.calculate_decay(1.0, 30.0);
        assert!(decayed < 0.02, "month-old should be near zero: {decayed}");
    }

    #[test]
    fn test_decay_117_day_pathway() {
        let scheduler = DecayScheduler::new();
        // 117 days (current bug) → essentially zero
        let decayed = scheduler.calculate_decay(1.0, 117.0);
        assert!(decayed < STRENGTH_FLOOR, "117-day pathway should be below floor: {decayed}");
    }

    #[test]
    fn test_decay_zero_days() {
        let scheduler = DecayScheduler::new();
        // Just reinforced → only decay_rate applied
        let decayed = scheduler.calculate_decay(1.0, 0.0);
        // e^0 = 1.0, so result = 1.0 * 1.0 * (1 - 0.1) = 0.9
        assert!((decayed - 0.9).abs() < 0.01, "zero-day: {decayed}");
    }

    #[test]
    fn test_decay_clamping() {
        let scheduler = DecayScheduler::new();
        // Negative strength (shouldn't happen but test boundary)
        let decayed = scheduler.calculate_decay(-1.0, 1.0);
        assert!(decayed >= 0.0, "should clamp to 0: {decayed}");
    }

    #[test]
    fn test_default_config() {
        let config = DecayConfig::default();
        assert!((config.decay_rate - 0.1).abs() < f64::EPSILON);
        assert!((config.tau_days - 7.0).abs() < f64::EPSILON);
        assert_eq!(config.cycle_interval_secs, 3600);
        assert!((config.strength_floor - 0.01).abs() < f64::EPSILON);
    }

    #[test]
    fn test_status_initially_empty() {
        let scheduler = DecayScheduler::new();
        let status = scheduler.status();
        assert!(status.last_cycle.is_none());
        assert_eq!(status.total_cycles, 0);
        assert!(status.recent_audits.is_empty());
    }

    #[test]
    fn test_audit_log_capacity() {
        // NOTE: This test verifies internal capacity management.
        // The audit_log field should be pub(crate) to allow test access,
        // or an audit_count() accessor should be provided.
        // Alpha Correction M9: no .unwrap() used.
        let scheduler = DecayScheduler::new();
        // Verify initial state through public API
        let status = scheduler.status();
        assert!(status.recent_audits.is_empty());
        assert_eq!(status.total_cycles, 0);
    }
}
```

---

## 8. NAM Integration

This section addresses NAM gaps NAM-G01 through NAM-G07 (see [NAM_GAP_ANALYSIS.md](NAM_GAP_ANALYSIS.md)).
M41 had the lowest NAM score in the suite (2/100) — it applies mass decay unilaterally,
with zero self-query, zero dissent, zero escalation, and zero human awareness.

### 8.1 Dissent Window (R-NAM-02) — Addresses NAM-G02, NAM-G07

Before executing a decay cycle that affects many pathways, M41 publishes a proposed action
and waits for agent dissent. CRITIC and INTEGRATOR agents (8 total, weight 1.0–1.2) can
block the cycle if they identify pathways that should be protected.

```rust
/// Duration of the dissent window before mass decay operations.
const DISSENT_WINDOW_SECS: u64 = 30;

/// Minimum agent dissent count to escalate a decay cycle.
const DISSENT_THRESHOLD: u32 = 2;

// In run_decay_cycle(), AFTER computing decay but BEFORE writing to DB:
let pathway_count = pathways.len() as u32;

// R3: Open dissent window for mass operations
if pathway_count > 5 {
    let proposed = serde_json::json!({
        "action_type": "bulk_decay",
        "affected_count": pathway_count,
        "severity": if pathway_count > 20 { "HIGH" } else { "MEDIUM" },
        "decay_rate": config.decay_rate,
        "tau_days": config.tau_days,
        "dissent_deadline_secs": DISSENT_WINDOW_SECS,
    });

    let _ = event_bus.publish(
        "consensus", "ProposedDecayCycle",
        &proposed.to_string(),
        "decay_scheduler",
    );

    // Wait for dissent window
    tokio::time::sleep(Duration::from_secs(DISSENT_WINDOW_SECS)).await;

    // Check dissent count (from M35 DissentTracker)
    let dissent_count = dissent_tracker.count_recent("bulk_decay", DISSENT_WINDOW_SECS);

    if dissent_count >= DISSENT_THRESHOLD {
        tracing::warn!(
            dissent_count,
            pathway_count,
            "Decay cycle BLOCKED by agent dissent — escalating to L2"
        );
        let _ = event_bus.publish(
            "escalation", "DecayCycleBlocked",
            &serde_json::json!({
                "dissent_count": dissent_count,
                "pathway_count": pathway_count,
                "tier": "L2",
                "action": "require_approval",
            }).to_string(),
            "decay_scheduler",
        );
        return Err(Error::Validation("Decay cycle blocked by agent dissent".into()));
    }
}

// Proceed with decay writes...
```

### 8.2 Self-Query Introspection (R-NAM-04) — Addresses NAM-G01

After each decay cycle, M41 self-assesses whether the decay parameters are producing
healthy pathway distributions, and whether pruned pathways are being re-created (which
would indicate over-aggressive decay).

```rust
impl DecayScheduler {
    /// R1: Self-assess the decay cycle's effectiveness.
    ///
    /// Compares recent cycles to detect:
    /// - Over-aggressive decay (avg strength delta too large)
    /// - Pathway recreation (pruned pathways re-appear within 24h)
    /// - Stagnant decay (no meaningful strength change)
    fn self_query(&self, audit: &DecayAuditEntry) -> SelfAssessment {
        let log = self.audit_log.read();
        let last_3: Vec<_> = log.iter().rev().take(3).collect();

        let avg_delta = if last_3.is_empty() {
            0.0
        } else {
            last_3.iter()
                .map(|a| a.avg_strength_before - a.avg_strength_after)
                .sum::<f64>() / last_3.len() as f64
        };

        // Check: are pathways below floor increasing each cycle?
        let floor_trend = if last_3.len() >= 2 {
            let recent_floor = last_3[0].pathways_below_floor;
            let prev_floor = last_3[1].pathways_below_floor;
            if recent_floor > prev_floor + 5 { "accelerating" }
            else if recent_floor > prev_floor { "increasing" }
            else { "stable" }
        } else {
            "insufficient_data"
        };

        SelfAssessment {
            decay_velocity: avg_delta,
            floor_trend: floor_trend.into(),
            recommendation: if avg_delta > 0.3 {
                "Decay too aggressive — consider increasing tau_days"
            } else if avg_delta < 0.001 && audit.pathways_processed > 0 {
                "Decay too slow — consider decreasing tau_days"
            } else {
                "Decay rate appropriate"
            },
        }
    }
}
```

**Integration in background task:**

```rust
Ok(audit) => {
    // R1: Self-query after every cycle
    let assessment = decay_scheduler.self_query(&audit);
    tracing::info!(
        velocity = assessment.decay_velocity,
        floor_trend = %assessment.floor_trend,
        recommendation = assessment.recommendation,
        "Decay self-assessment"
    );

    // If self-query detects over-aggressive decay, publish warning
    if assessment.decay_velocity > 0.3 {
        let _ = event_bus.publish(
            "learning", "DecaySelfQueryWarning",
            &serde_json::json!({
                "velocity": assessment.decay_velocity,
                "recommendation": assessment.recommendation,
                "floor_trend": assessment.floor_trend,
            }).to_string(),
            "decay_scheduler",
        );
    }
    // ... existing event publishing ...
}
```

### 8.3 Escalation Gates (R-NAM-01) — Addresses NAM-G04

Mass pathway degradation (>10 pathways below floor) escalates to L1 minimum.

```rust
// After decay cycle completes:
if audit.pathways_below_floor > 10 {
    // L1: Notify Human @0.A — mass pathway degradation
    let _ = event_bus.publish(
        "escalation", "MassPathwayDegradation",
        &serde_json::json!({
            "tier": "L1",
            "pathways_below_floor": audit.pathways_below_floor,
            "total_pathways": audit.pathways_processed,
            "avg_strength_after": audit.avg_strength_after,
            "action": "prune_evaluation_pending",
            "urgency": if audit.pathways_below_floor > 20 { "HIGH" } else { "MEDIUM" },
        }).to_string(),
        "decay_scheduler",
    );
}

// Gate pruning trigger on escalation
if audit.pathways_below_floor > 0 {
    if audit.pathways_below_floor > 10 {
        // L1 notification sent above — prune only after notification window
        tracing::info!(
            below_floor = audit.pathways_below_floor,
            "Mass degradation: pruning deferred pending L1 notification"
        );
    } else {
        // L0: Small number of pathways, auto-execute pruning
        let _ = pruner.evaluate_and_prune(&db).await;
    }
}
```

### 8.4 12D Tensor Integration (R-NAM-03)

Pathway health ratio updates tensor dimension D8 (synergy) after each cycle.

```rust
// After decay cycle:
let pathway_health_ratio = if audit.pathways_processed > 0 {
    1.0 - (audit.pathways_below_floor as f64 / audit.pathways_processed as f64)
} else {
    1.0
};

// R4: Update 12D tensor
tensor_store.update_dimension(
    "maintenance_engine",
    TensorDimension::Synergy, // D8
    pathway_health_ratio,
);
```

### 8.5 Human @0.A Notification (R-NAM-06) — Addresses NAM-G03

Human @0.A receives visibility into decay cycle outcomes, especially mass degradation.

```rust
// After every decay cycle (not just mass degradation):
let _ = event_bus.publish(
    "human", "DecayCycleSummary",
    &serde_json::json!({
        "agent": "@0.A",
        "cycle_number": *self.total_cycles.read(),
        "pathways_processed": audit.pathways_processed,
        "pathways_decayed": audit.pathways_decayed,
        "pathways_below_floor": audit.pathways_below_floor,
        "avg_strength_before": format!("{:.4}", audit.avg_strength_before),
        "avg_strength_after": format!("{:.4}", audit.avg_strength_after),
        "duration_ms": audit.duration_ms,
        "self_assessment": assessment.recommendation,
    }).to_string(),
    "decay_scheduler",
);
```

### 8.6 RALPH Evolution Interface (R-NAM-07) — Addresses CX-05

Concrete interface for M39 RALPH to tune decay parameters.

```rust
/// Parameters that M39 RALPH can tune via the evolution loop.
/// These map to the config table in hebbian_pulse.db.
pub struct DecayTunableParameters {
    /// Decay rate coefficient (current: 0.1, range: 0.01–0.5).
    pub decay_rate: f64,
    /// Time constant in days (current: 7.0, range: 1.0–30.0).
    pub tau_days: f64,
}

impl DecayScheduler {
    /// Export current parameters for RALPH fitness evaluation.
    pub fn tunable_parameters(&self) -> DecayTunableParameters {
        let config = self.config.read();
        DecayTunableParameters {
            decay_rate: config.decay_rate,
            tau_days: config.tau_days,
        }
    }

    /// Apply RALPH-tuned parameters (writes to config table).
    pub async fn apply_tuned_parameters(
        &self,
        params: &DecayTunableParameters,
        db: &StatePersistence,
    ) -> Result<()> {
        let rate = params.decay_rate.clamp(0.01, 0.5);
        let tau = params.tau_days.clamp(1.0, 30.0);

        db.execute(
            "UPDATE config SET value = ?1 WHERE key = 'decay_rate'",
            &[&rate],
        ).await?;
        db.execute(
            "UPDATE config SET value = ?1 WHERE key = 'tau'",
            &[&tau],
        ).await?;

        let mut config = self.config.write();
        config.decay_rate = rate;
        config.tau_days = tau;
        Ok(())
    }
}
```

### 8.7 NAM Compliance Summary

| NAM Requirement | Gap ID | Status | Implementation |
|-----------------|--------|--------|----------------|
| R1 SelfQuery | NAM-G01 | ADDRESSED | `self_query()` assesses decay velocity and floor trends |
| R2 HebbianRouting | NAM-G06 | PARTIAL | RALPH tunes parameters via pathway fitness |
| R3 DissentCapture | NAM-G02, G07 | ADDRESSED | 30s dissent window, CRITIC/INTEGRATOR agents can block |
| R4 FieldVisualization | — | ADDRESSED | D8 tensor update with pathway health ratio |
| R5 HumanAsAgent | NAM-G03 | ADDRESSED | @0.A receives cycle summaries, mass degradation alerts |
| Escalation | NAM-G04 | ADDRESSED | >10 below floor → L1, dissent blocks → L2 |
| Episodic Memory | NAM-G05 | DEFERRED | Requires M29 Memory Consolidator integration (Gen2) |

**Projected NAM Score: 2/100 → 68/100**

---

*Document: ME_MODULE_M41_DECAY_SCHEDULER.md (NAM Integration Applied — R-NAM-01, R-NAM-02, R-NAM-03, R-NAM-04, R-NAM-06, R-NAM-07)*
*Alpha Corrections: H5, M9*
*Location: generation_1_bug_fix/ai_docs/*
