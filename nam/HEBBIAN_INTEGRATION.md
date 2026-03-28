# Hebbian Learning Integration

**Version:** 1.0.0
**Status:** Specification Complete
**NAM Compliance:** Phase 5, P0-4 Priority
**Impact:** Learning +60%, R2 HebbianRouting

---

## 1. Overview

Connect maintenance actions to `hebbian_pulse.db` for pathway-based learning. Replace static thresholds with dynamically learned pathway weights that adapt based on action outcomes.

### 1.1 Core Principle

> "Neurons that fire together, wire together." - Donald Hebb

In the maintenance context:
- **Actions that succeed together strengthen their pathways**
- **Actions that fail together weaken their pathways**
- **Timing matters: faster success = stronger reinforcement**

### 1.2 NAM Alignment

| NAM Requirement | Hebbian Implementation |
|-----------------|----------------------|
| R2 HebbianRouting | Pathway-weighted decision routing |
| R1 SelfQuery | Query pathway strengths autonomously |
| R3 DissentCapture | Learn from disagreement outcomes |

---

## 2. STDP Configuration

Spike-Timing-Dependent Plasticity (STDP) governs how pathway strengths change based on timing.

### 2.1 Core Constants

```rust
/// STDP Configuration for Maintenance Engine
pub struct STDPConfig {
    /// Long-Term Potentiation rate (strengthening)
    /// Applied when action succeeds
    pub ltp_rate: f64,

    /// Long-Term Depression rate (weakening)
    /// Applied when action fails
    pub ltd_rate: f64,

    /// Timing window for STDP effects (milliseconds)
    /// Events within this window influence each other
    pub stdp_window_ms: u64,

    /// Minimum pathway strength (prevents complete extinction)
    pub min_strength: f64,

    /// Maximum pathway strength (prevents runaway potentiation)
    pub max_strength: f64,

    /// Decay rate for unused pathways (per day)
    pub decay_rate: f64,
}

impl Default for STDPConfig {
    fn default() -> Self {
        Self {
            ltp_rate: 0.1,       // 10% strengthening per success
            ltd_rate: 0.05,      // 5% weakening per failure
            stdp_window_ms: 100, // 100ms timing window
            min_strength: 0.01,  // Never go below 1%
            max_strength: 1.0,   // Never exceed 100%
            decay_rate: 0.001,   // 0.1% decay per day of inactivity
        }
    }
}
```

### 2.2 STDP Timing Curve

```rust
/// Calculate STDP weight delta based on timing
///
/// Pre-post timing (action before positive outcome): positive delta (LTP)
/// Post-pre timing (outcome before expected action): negative delta (LTD)
pub fn calculate_stdp_delta(
    config: &STDPConfig,
    timing_ms: i64,  // Positive = pre-post, Negative = post-pre
    success: bool,
) -> f64 {
    let timing_factor = if timing_ms.abs() as u64 <= config.stdp_window_ms {
        // Within window: full effect
        1.0 - (timing_ms.abs() as f64 / config.stdp_window_ms as f64)
    } else {
        // Outside window: exponential decay
        let decay = (-(timing_ms.abs() as f64 / config.stdp_window_ms as f64)).exp();
        decay * 0.5 // Max 50% effect outside window
    };

    if success {
        // LTP: Strengthen pathway
        config.ltp_rate * timing_factor * if timing_ms >= 0 { 1.0 } else { 0.5 }
    } else {
        // LTD: Weaken pathway
        -config.ltd_rate * timing_factor
    }
}
```

---

## 3. Maintenance Pathways Registration

Register maintenance-specific pathways in `hebbian_pulse.db`.

### 3.1 Pathway Schema

```sql
-- Hebbian pathways table (in hebbian_pulse.db)
CREATE TABLE IF NOT EXISTS hebbian_pathways (
    pathway_id TEXT PRIMARY KEY,
    source_module TEXT NOT NULL,      -- 'maintenance' for all maintenance pathways
    target_module TEXT NOT NULL,      -- Action type (e.g., 'service_restart')
    pathway_strength REAL NOT NULL DEFAULT 0.5,
    last_activation DATETIME,
    activation_count INTEGER DEFAULT 0,
    success_count INTEGER DEFAULT 0,
    failure_count INTEGER DEFAULT 0,
    avg_duration_ms REAL,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_pathways_source ON hebbian_pathways(source_module);
CREATE INDEX idx_pathways_strength ON hebbian_pathways(pathway_strength);
CREATE INDEX idx_pathways_active ON hebbian_pathways(last_activation);
```

### 3.2 Initial Pathway Registration

```sql
-- Register maintenance pathways with neutral initial strength
INSERT INTO hebbian_pathways (
    pathway_id,
    source_module,
    target_module,
    pathway_strength,
    last_activation,
    activation_count
) VALUES
    -- Service operations
    ('maintenance_restart', 'maintenance', 'service_restart', 0.5, NULL, 0),
    ('maintenance_stop', 'maintenance', 'service_stop', 0.5, NULL, 0),
    ('maintenance_start', 'maintenance', 'service_start', 0.5, NULL, 0),

    -- Database operations
    ('maintenance_vacuum', 'maintenance', 'database_vacuum', 0.5, NULL, 0),
    ('maintenance_backup', 'maintenance', 'database_backup', 0.5, NULL, 0),
    ('maintenance_checkpoint', 'maintenance', 'database_checkpoint', 0.5, NULL, 0),

    -- Cache operations
    ('maintenance_cache_cleanup', 'maintenance', 'cache_cleanup', 0.5, NULL, 0),
    ('maintenance_cache_warm', 'maintenance', 'cache_warm', 0.5, NULL, 0),

    -- Session operations
    ('maintenance_session_rotation', 'maintenance', 'session_rotation', 0.5, NULL, 0),
    ('maintenance_session_cleanup', 'maintenance', 'session_cleanup', 0.5, NULL, 0),

    -- Configuration operations
    ('maintenance_config_refresh', 'maintenance', 'config_refresh', 0.5, NULL, 0),
    ('maintenance_config_rollback', 'maintenance', 'config_rollback', 0.5, NULL, 0),

    -- Health operations
    ('maintenance_health_probe', 'maintenance', 'health_probe', 0.75, NULL, 0),
    ('maintenance_health_remediate', 'maintenance', 'health_remediate', 0.5, NULL, 0),

    -- Security operations
    ('maintenance_cert_renewal', 'maintenance', 'certificate_renewal', 0.5, NULL, 0),
    ('maintenance_cred_rotation', 'maintenance', 'credential_rotation', 0.5, NULL, 0),

    -- Connection operations
    ('maintenance_pool_trim', 'maintenance', 'connection_pool_trim', 0.5, NULL, 0),
    ('maintenance_pool_refresh', 'maintenance', 'connection_pool_refresh', 0.5, NULL, 0)

ON CONFLICT(pathway_id) DO NOTHING;
```

### 3.3 Rust Pathway Registration

```rust
/// Register maintenance pathways in hebbian_pulse.db
pub async fn register_maintenance_pathways(db: &Pool<Sqlite>) -> Result<()> {
    let pathways = vec![
        Pathway::new("maintenance", "service_restart", 0.5),
        Pathway::new("maintenance", "database_vacuum", 0.5),
        Pathway::new("maintenance", "cache_cleanup", 0.5),
        Pathway::new("maintenance", "session_rotation", 0.5),
        Pathway::new("maintenance", "config_refresh", 0.5),
        Pathway::new("maintenance", "health_probe", 0.75), // Higher initial for health
        Pathway::new("maintenance", "certificate_renewal", 0.5),
        Pathway::new("maintenance", "connection_pool_trim", 0.5),
    ];

    for pathway in pathways {
        sqlx::query!(
            r#"
            INSERT INTO hebbian_pathways (
                pathway_id, source_module, target_module, pathway_strength
            ) VALUES (?, ?, ?, ?)
            ON CONFLICT(pathway_id) DO NOTHING
            "#,
            format!("{}_{}", pathway.source, pathway.target),
            pathway.source,
            pathway.target,
            pathway.initial_strength
        )
        .execute(db)
        .await?;
    }

    Ok(())
}
```

---

## 4. Outcome Recording

Record maintenance outcomes for pathway learning.

### 4.1 Implementation

```rust
impl MaintenanceEngine {
    /// Record action outcome and update Hebbian pathways
    pub async fn on_action_complete(
        &self,
        action: &Action,
        success: bool,
        duration_ms: u64,
    ) -> Result<()> {
        let pathway = format!("maintenance_{}", action.name);

        // Calculate weight delta based on STDP
        let delta = if success {
            // Faster actions get more reinforcement
            let timing_factor = self.calculate_timing_factor(duration_ms);
            self.stdp_config.ltp_rate * timing_factor
        } else {
            -self.stdp_config.ltd_rate
        };

        // Update pathway strength with bounds checking
        sqlx::query!(
            r#"
            UPDATE hebbian_pathways
            SET
                pathway_strength = MIN(?, MAX(?, pathway_strength + ?)),
                last_activation = CURRENT_TIMESTAMP,
                activation_count = activation_count + 1,
                success_count = success_count + ?,
                failure_count = failure_count + ?,
                avg_duration_ms = COALESCE(
                    (avg_duration_ms * activation_count + ?) / (activation_count + 1),
                    ?
                ),
                updated_at = CURRENT_TIMESTAMP
            WHERE pathway_id = ?
            "#,
            self.stdp_config.max_strength,
            self.stdp_config.min_strength,
            delta,
            if success { 1 } else { 0 },
            if success { 0 } else { 1 },
            duration_ms as f64,
            duration_ms as f64,
            pathway
        )
        .execute(&self.hebbian_db)
        .await?;

        // Record pulse event for temporal analysis
        self.record_pulse_event(&pathway, delta, action, success, duration_ms).await?;

        Ok(())
    }

    /// Calculate timing factor for STDP reinforcement
    fn calculate_timing_factor(&self, duration_ms: u64) -> f64 {
        // Faster than expected: bonus reinforcement (up to 1.5x)
        // Slower than expected: reduced reinforcement (down to 0.5x)
        let expected_ms = 1000.0; // 1 second baseline

        if duration_ms <= expected_ms as u64 {
            1.0 + (1.0 - (duration_ms as f64 / expected_ms)) * 0.5
        } else {
            0.5 + 0.5 * (expected_ms / duration_ms as f64)
        }
    }
}
```

### 4.2 Pulse Event Recording

```rust
/// Record pulse event for temporal learning patterns
async fn record_pulse_event(
    &self,
    pathway_id: &str,
    strength_delta: f64,
    action: &Action,
    success: bool,
    duration_ms: u64,
) -> Result<()> {
    sqlx::query!(
        r#"
        INSERT INTO pulse_events (
            id,
            event_type,
            pathway_id,
            strength_delta,
            event_data,
            timestamp
        ) VALUES (?, ?, ?, ?, ?, CURRENT_TIMESTAMP)
        "#,
        Uuid::new_v4().to_string(),
        "maintenance_stdp",
        pathway_id,
        strength_delta,
        serde_json::to_string(&PulseEventData {
            action_id: action.id.clone(),
            action_type: action.action_type.clone(),
            service_id: action.service_id.clone(),
            success,
            duration_ms,
            context_tensor: MaintenanceTensor::from_current_state().to_bytes(),
        })?
    )
    .execute(&self.hebbian_db)
    .await?;

    Ok(())
}
```

---

## 5. Threshold Replacement with Hebbian Queries

Replace static thresholds with dynamically learned values from Hebbian pathways.

### 5.1 Before: Static Thresholds

```rust
// OLD: Static threshold (inflexible)
const SESSION_SIZE_WARNING: u64 = 3_500_000;  // 3.5MB
const SESSION_SIZE_CRITICAL: u64 = 5_000_000; // 5MB
const SESSION_SIZE_MAX: u64 = 7_000_000;      // 7MB

fn should_rotate_session(size_bytes: u64) -> RotationDecision {
    if size_bytes > SESSION_SIZE_MAX {
        RotationDecision::Critical
    } else if size_bytes > SESSION_SIZE_CRITICAL {
        RotationDecision::Recommended
    } else if size_bytes > SESSION_SIZE_WARNING {
        RotationDecision::Warning
    } else {
        RotationDecision::Ok
    }
}
```

### 5.2 After: Hebbian-Learned Thresholds

```rust
/// NEW: Threshold derived from Hebbian pathway strength
impl MaintenanceEngine {
    /// Get session size threshold based on learned pathway strength
    pub async fn get_session_size_threshold(&self) -> Result<u64> {
        let pathway_strength: f64 = sqlx::query_scalar!(
            r#"
            SELECT pathway_strength
            FROM hebbian_pathways
            WHERE pathway_id = 'maintenance_session_rotation'
            "#
        )
        .fetch_one(&self.hebbian_db)
        .await?
        .unwrap_or(0.5);

        // Map pathway strength (0-1) to threshold range
        // Strong pathway (high success rate) = earlier rotation (lower threshold)
        // Weak pathway (low success rate) = later rotation (higher threshold)

        let min_threshold = 2_000_000u64;  // 2MB minimum
        let max_threshold = 7_000_000u64;  // 7MB maximum
        let range = max_threshold - min_threshold;

        // Inverse relationship: stronger pathway = lower threshold
        let threshold = max_threshold - (pathway_strength * range as f64) as u64;

        Ok(threshold)
    }

    /// Dynamic rotation decision based on learned thresholds
    pub async fn should_rotate_session(&self, size_bytes: u64) -> Result<RotationDecision> {
        let base_threshold = self.get_session_size_threshold().await?;

        // Query recent outcomes to adjust sensitivity
        let recent_success_rate = self.get_recent_success_rate("session_rotation", 7).await?;

        // Adjust threshold based on recent performance
        let adjusted_threshold = if recent_success_rate > 0.9 {
            // Very successful recently: can be more aggressive
            (base_threshold as f64 * 0.9) as u64
        } else if recent_success_rate < 0.7 {
            // Less successful: be more conservative
            (base_threshold as f64 * 1.1) as u64
        } else {
            base_threshold
        };

        // Decision based on percentage of threshold
        let percentage = size_bytes as f64 / adjusted_threshold as f64;

        Ok(match percentage {
            p if p >= 1.0 => RotationDecision::Critical,
            p if p >= 0.85 => RotationDecision::Recommended,
            p if p >= 0.7 => RotationDecision::Warning,
            _ => RotationDecision::Ok,
        })
    }
}
```

### 5.3 Other Threshold Replacements

```rust
/// Health score threshold from pathway
pub async fn get_health_threshold(&self) -> Result<f64> {
    let pathway_strength = self.query_pathway("maintenance_health_remediate").await?;

    // Map: strong pathway = lower threshold (more sensitive)
    // Range: 0.6 (very sensitive) to 0.9 (conservative)
    Ok(0.9 - (pathway_strength * 0.3))
}

/// Cache cleanup threshold from pathway
pub async fn get_cache_threshold(&self) -> Result<f64> {
    let pathway_strength = self.query_pathway("maintenance_cache_cleanup").await?;

    // Map: strong pathway = lower threshold (earlier cleanup)
    // Range: 0.5 (aggressive) to 0.9 (conservative)
    Ok(0.9 - (pathway_strength * 0.4))
}

/// Connection pool trim threshold from pathway
pub async fn get_pool_trim_threshold(&self) -> Result<f64> {
    let pathway_strength = self.query_pathway("maintenance_pool_trim").await?;

    // Map: strong pathway = lower threshold (more aggressive trimming)
    // Range: 0.3 (very aggressive) to 0.7 (conservative)
    Ok(0.7 - (pathway_strength * 0.4))
}
```

---

## 6. STDP Timing for Remediation Chains

Track timing between related maintenance events for chain learning.

### 6.1 Chain Recording

```sql
-- Track STDP timing between maintenance events
INSERT INTO pulse_events (
    id,
    event_type,
    pathway_id,
    strength_delta,
    event_data,
    timestamp
) VALUES (
    ?,
    'maintenance_chain_stdp',
    'restart_after_failure',
    0.05,
    json_object(
        'pre_event', 'service_failure',
        'post_event', 'service_restart',
        'timing_ms', 150,
        'within_window', true,
        'chain_id', ?,
        'chain_position', 2
    ),
    CURRENT_TIMESTAMP
);
```

### 6.2 Chain Learning Implementation

```rust
/// Track and learn from remediation chains
pub struct RemediationChainTracker {
    db: Pool<Sqlite>,
    stdp_config: STDPConfig,
    active_chains: DashMap<String, Chain>,
}

impl RemediationChainTracker {
    /// Record an event in a remediation chain
    pub async fn record_chain_event(
        &self,
        chain_id: &str,
        event: &MaintenanceEvent,
    ) -> Result<()> {
        let chain = self.active_chains
            .entry(chain_id.to_string())
            .or_insert_with(Chain::new);

        // Calculate timing from previous event
        let timing_ms = if let Some(prev) = chain.last_event() {
            (event.timestamp - prev.timestamp).num_milliseconds()
        } else {
            0
        };

        // Add event to chain
        chain.add_event(event.clone());

        // If within STDP window, create pathway between events
        if timing_ms > 0 && timing_ms <= self.stdp_config.stdp_window_ms as i64 {
            let pathway_id = format!(
                "{}_after_{}",
                event.event_type,
                chain.last_event().unwrap().event_type
            );

            // Strengthen or weaken based on outcome
            let delta = calculate_stdp_delta(
                &self.stdp_config,
                timing_ms,
                event.is_success(),
            );

            self.update_chain_pathway(&pathway_id, delta, event, timing_ms).await?;
        }

        Ok(())
    }

    /// Update pathway for remediation chain
    async fn update_chain_pathway(
        &self,
        pathway_id: &str,
        delta: f64,
        event: &MaintenanceEvent,
        timing_ms: i64,
    ) -> Result<()> {
        // Ensure pathway exists
        sqlx::query!(
            r#"
            INSERT INTO hebbian_pathways (
                pathway_id, source_module, target_module, pathway_strength
            ) VALUES (?, 'chain', ?, 0.5)
            ON CONFLICT(pathway_id) DO NOTHING
            "#,
            pathway_id,
            event.event_type
        )
        .execute(&self.db)
        .await?;

        // Update pathway strength
        sqlx::query!(
            r#"
            UPDATE hebbian_pathways
            SET
                pathway_strength = MIN(1.0, MAX(0.01, pathway_strength + ?)),
                last_activation = CURRENT_TIMESTAMP,
                activation_count = activation_count + 1
            WHERE pathway_id = ?
            "#,
            delta,
            pathway_id
        )
        .execute(&self.db)
        .await?;

        // Record pulse event for chain
        sqlx::query!(
            r#"
            INSERT INTO pulse_events (
                id, event_type, pathway_id, strength_delta, event_data
            ) VALUES (?, 'maintenance_chain_stdp', ?, ?, ?)
            "#,
            Uuid::new_v4().to_string(),
            pathway_id,
            delta,
            serde_json::to_string(&json!({
                "timing_ms": timing_ms,
                "within_window": timing_ms <= self.stdp_config.stdp_window_ms as i64,
                "event_type": event.event_type,
                "success": event.is_success(),
            }))?
        )
        .execute(&self.db)
        .await?;

        Ok(())
    }
}
```

---

## 7. Pathway Queries

### 7.1 Query Strong Pathways

```sql
-- Find strongest maintenance pathways (most reliable actions)
SELECT
    pathway_id,
    target_module as action_type,
    pathway_strength,
    activation_count,
    ROUND(success_count * 100.0 / NULLIF(activation_count, 0), 1) as success_rate,
    ROUND(avg_duration_ms) as avg_duration_ms,
    last_activation
FROM hebbian_pathways
WHERE source_module = 'maintenance'
AND activation_count >= 10  -- Minimum sample size
ORDER BY pathway_strength DESC
LIMIT 10;
```

### 7.2 Query Weak Pathways

```sql
-- Find weakest pathways (may need investigation)
SELECT
    pathway_id,
    target_module as action_type,
    pathway_strength,
    activation_count,
    failure_count,
    ROUND(failure_count * 100.0 / NULLIF(activation_count, 0), 1) as failure_rate
FROM hebbian_pathways
WHERE source_module = 'maintenance'
AND pathway_strength < 0.3
AND activation_count >= 5
ORDER BY pathway_strength ASC;
```

### 7.3 Query Dormant Pathways

```sql
-- Find pathways that haven't been used recently (may need pruning)
SELECT
    pathway_id,
    target_module,
    pathway_strength,
    last_activation,
    julianday('now') - julianday(last_activation) as days_inactive
FROM hebbian_pathways
WHERE source_module = 'maintenance'
AND (last_activation IS NULL OR last_activation < datetime('now', '-30 days'))
ORDER BY last_activation ASC NULLS FIRST;
```

---

## 8. References

- **Hebb, D.O. (1949):** "The Organization of Behavior"
- **STDP Literature:** Bi & Poo (1998), Song et al. (2000)
- **L0 Auto-Remediation:** `nam/L0_AUTO_REMEDIATION.md`
- **Episodic Memory:** `nam/EPISODIC_MEMORY.md`
- **NAM Gap Analysis:** `NAM_GAP_ANALYSIS_REPORT.md` (R2 HebbianRouting)

---

*Document generated for NAM Phase 5 compliance*
*Hebbian Integration: Where actions that succeed together, strengthen together*
