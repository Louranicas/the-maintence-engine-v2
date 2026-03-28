# L0 Auto-Remediation Tier

**Version:** 1.0.0
**Status:** Specification Complete
**NAM Compliance:** Phase 5, P0-1 Priority
**Impact:** Autonomy +40%

---

## 1. Overview

L0 is the foundational autonomous remediation tier that executes maintenance actions without requiring human approval when system confidence exceeds established thresholds. This represents a fundamental shift from human-centric supervision to machine-first autonomous operation.

### 1.1 Design Philosophy

In traditional human-centric systems, all remediation requires human approval. L0 inverts this:
- **Default:** Autonomous action when confidence is high
- **Exception:** Human involvement when confidence is low or risk is high

### 1.2 NAM Alignment

| NAM Requirement | L0 Implementation |
|-----------------|-------------------|
| R1 SelfQuery | Autonomous health assessment loops |
| R2 HebbianRouting | Pathway-weighted confidence calculation |
| R5 HumanAsAgent | Human @0.A as peer participant, not supervisor |

---

## 2. Confidence Calculation Formula

### 2.1 Core Formula

```rust
/// Calculate confidence score for autonomous action eligibility
///
/// Returns value in [0.0, 1.0] where:
/// - 0.0 = No confidence, immediate escalation to L1
/// - 0.9+ = L0 autonomous execution eligible
/// - 1.0 = Maximum confidence, guaranteed safe action
pub fn calculate_confidence(action: &Action, context: &Context) -> f64 {
    // Query Hebbian pathway strength for this action type
    let pathway_strength = query_hebbian_pathway(&action.name);

    // Historical success rate over rolling 30-day window
    let historical_success = query_success_rate(&action.name, 30);

    // Current system health score (from 12D tensor)
    let system_health = get_system_health_score();

    // Time-of-day factor (higher during maintenance windows)
    let temporal_factor = get_temporal_confidence_factor();

    // Service criticality inverse (less critical = more autonomous)
    let criticality_factor = 1.0 - (context.service.tier as f64 / 6.0);

    // Weighted confidence formula
    // Pathway strength and historical success are weighted heavily (0.35 each)
    // System health provides safety check (0.15)
    // Temporal and criticality provide context adjustment (0.075 each)
    let base_confidence =
        (pathway_strength * 0.35) +
        (historical_success * 0.35) +
        (system_health * 0.15) +
        (temporal_factor * 0.075) +
        (criticality_factor * 0.075);

    // Apply safety bounds
    base_confidence.clamp(0.0, 1.0)
}
```

### 2.2 Component Details

#### 2.2.1 Pathway Strength Query

```rust
/// Query Hebbian pathway strength from hebbian_pulse.db
async fn query_hebbian_pathway(action_name: &str) -> f64 {
    let pathway_id = format!("maintenance_{}", action_name);

    sqlx::query_scalar!(
        r#"
        SELECT pathway_strength
        FROM hebbian_pathways
        WHERE pathway_id = ?
        AND source_module = 'maintenance'
        AND last_activation > datetime('now', '-7 days')
        "#,
        pathway_id
    )
    .fetch_optional(&db)
    .await
    .unwrap_or(Some(0.5))  // Default 0.5 for unknown pathways
    .unwrap_or(0.5)
}
```

#### 2.2.2 Historical Success Rate

```rust
/// Query success rate over specified day window
async fn query_success_rate(action_name: &str, days: u32) -> f64 {
    let result = sqlx::query!(
        r#"
        SELECT
            COUNT(*) FILTER (WHERE outcome = 'success') as successes,
            COUNT(*) as total
        FROM l0_actions
        WHERE action_type = ?
        AND executed_at > datetime('now', ? || ' days')
        "#,
        action_name,
        format!("-{}", days)
    )
    .fetch_one(&db)
    .await?;

    if result.total == 0 {
        return 0.5; // No history = neutral confidence
    }

    result.successes as f64 / result.total as f64
}
```

#### 2.2.3 System Health Score

```rust
/// Calculate system health from 12D tensor
fn get_system_health_score() -> f64 {
    let tensor = MaintenanceTensor::from_current_state();

    // D6 (health_score) weighted by D7 (uptime) and D8 (synergy)
    let health_component = tensor.0[6];  // D6: health_score
    let uptime_component = tensor.0[7];  // D7: uptime
    let synergy_component = tensor.0[8]; // D8: synergy

    // Inverse of D10 (error_rate)
    let error_penalty = 1.0 - tensor.0[10];

    // Weighted combination
    (health_component * 0.4) +
    (uptime_component * 0.2) +
    (synergy_component * 0.2) +
    (error_penalty * 0.2)
}
```

---

## 3. L0 Eligible Actions Table

| Action | Min Confidence | Max Severity | Conditions | Timeout | Rollback |
|--------|----------------|--------------|------------|---------|----------|
| **Service Restart** | 0.90 | MEDIUM | After 2+ health failures | 60s | Previous state snapshot |
| **Cache Cleanup** | 0.85 | LOW | When >80% capacity | 30s | None required |
| **Session Rotation** | 0.88 | LOW | When >5MB threshold | 45s | Previous session preserved |
| **Database VACUUM** | 0.92 | LOW | During maintenance window only | 300s | Transaction rollback |
| **Log Rotation** | 0.95 | LOW | Scheduled intervals | 30s | None required |
| **Config Refresh** | 0.91 | LOW | On config file change | 15s | Previous config cached |
| **Health Probe** | 0.75 | NONE | Continuous (every 30s) | 5s | N/A |
| **Metric Flush** | 0.80 | LOW | Buffer >1000 entries | 10s | Buffer retained |
| **Certificate Renewal** | 0.93 | MEDIUM | 30 days before expiry | 120s | Previous cert valid |
| **Connection Pool Trim** | 0.87 | LOW | Idle connections >50% | 20s | Gradual reduction |

### 3.1 Action Definitions

```rust
/// L0-eligible actions with their requirements
#[derive(Debug, Clone)]
pub struct L0Action {
    pub name: String,
    pub min_confidence: f64,
    pub max_severity: Severity,
    pub conditions: Vec<Condition>,
    pub timeout_seconds: u64,
    pub rollback_strategy: RollbackStrategy,
}

impl L0Action {
    pub const SERVICE_RESTART: L0Action = L0Action {
        name: String::from("service_restart"),
        min_confidence: 0.90,
        max_severity: Severity::Medium,
        conditions: vec![
            Condition::MinHealthFailures(2),
            Condition::ServiceNotCritical,
        ],
        timeout_seconds: 60,
        rollback_strategy: RollbackStrategy::StateSnapshot,
    };

    pub const CACHE_CLEANUP: L0Action = L0Action {
        name: String::from("cache_cleanup"),
        min_confidence: 0.85,
        max_severity: Severity::Low,
        conditions: vec![
            Condition::CacheCapacityExceeds(0.80),
        ],
        timeout_seconds: 30,
        rollback_strategy: RollbackStrategy::None,
    };

    // ... additional actions
}
```

---

## 4. Escalation Override Conditions

L0 autonomous actions can be overridden in the following scenarios:

### 4.1 Human @0.A Veto

```rust
/// Human can veto L0 action within grace period
struct HumanVeto {
    /// Time window for human to veto (default 60 seconds)
    grace_period_seconds: u64,

    /// Notification channel for human
    notification_channel: NotificationChannel,

    /// Whether action is delayed until grace period expires
    await_veto_window: bool,
}

impl L0Remediation {
    /// Check if human has vetoed this action
    async fn check_human_veto(&self, action_id: &str) -> bool {
        sqlx::query_scalar!(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM agent_actions
                WHERE agent_id = '@0.A'
                AND action_type = 'veto'
                AND target = ?
                AND timestamp > datetime('now', '-60 seconds')
            )
            "#,
            action_id
        )
        .fetch_one(&self.db)
        .await
        .unwrap_or(false)
    }
}
```

### 4.2 PBFT Consensus Dissent

```rust
/// L0 escalates to L1 if multiple agents dissent
const DISSENT_THRESHOLD: u32 = 3;

async fn check_agent_dissent(&self, action: &Action) -> EscalationDecision {
    let dissent_count = sqlx::query_scalar!(
        r#"
        SELECT COUNT(DISTINCT agent_id)
        FROM dissent_events
        WHERE proposed_action = ?
        AND timestamp > datetime('now', '-5 minutes')
        AND outcome = 'PENDING'
        "#,
        action.id
    )
    .fetch_one(&self.db)
    .await
    .unwrap_or(0);

    if dissent_count >= DISSENT_THRESHOLD {
        EscalationDecision::EscalateToL1 {
            reason: format!("{} agents dissented", dissent_count),
        }
    } else {
        EscalationDecision::ProceedL0
    }
}
```

### 4.3 Circuit Breaker State

```rust
/// L0 defers to L1 when circuit breaker is OPEN
#[derive(Debug, Clone, PartialEq)]
pub enum CircuitState {
    Closed,    // Normal operation, L0 eligible
    HalfOpen,  // Testing recovery, L0 with caution
    Open,      // Failure mode, escalate to L1
}

impl L0Remediation {
    async fn check_circuit_breaker(&self, service_id: &str) -> CircuitState {
        let state = self.circuit_breakers
            .get(service_id)
            .map(|cb| cb.state.clone())
            .unwrap_or(CircuitState::Closed);

        match state {
            CircuitState::Open => {
                // Log escalation reason
                tracing::info!(
                    service = service_id,
                    "Circuit breaker OPEN, escalating to L1"
                );
                state
            }
            _ => state
        }
    }
}
```

### 4.4 Override Decision Matrix

| Condition | L0 Behavior | Escalation |
|-----------|-------------|------------|
| Human veto within 60s | Abort action | L1 notification |
| 3+ agent dissents | Pause action | L1 with consensus |
| Circuit breaker OPEN | Defer action | L1 immediate |
| Confidence < threshold | Skip L0 | L1 standard |
| Severity > MAX_SEVERITY | Reject action | L2/L3 based on severity |
| Maintenance window closed | Queue action | L0 when window opens |

---

## 5. Outcome Recording for Hebbian Learning

### 5.1 Action Recording Schema

```sql
-- L0 action execution records
CREATE TABLE l0_actions (
    id TEXT PRIMARY KEY,
    action_id TEXT NOT NULL,
    action_type TEXT NOT NULL,
    service_id TEXT NOT NULL,
    confidence REAL NOT NULL,
    outcome TEXT NOT NULL CHECK(outcome IN ('success', 'failure', 'timeout', 'aborted')),
    duration_ms INTEGER NOT NULL,
    error_message TEXT,
    rollback_performed BOOLEAN DEFAULT FALSE,
    executed_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    context_tensor BLOB,  -- 12D tensor snapshot at execution

    -- Foreign keys
    FOREIGN KEY (service_id) REFERENCES services(id)
);

-- Index for pathway queries
CREATE INDEX idx_l0_actions_type ON l0_actions(action_type);
CREATE INDEX idx_l0_actions_outcome ON l0_actions(outcome);
CREATE INDEX idx_l0_actions_timestamp ON l0_actions(executed_at);
```

### 5.2 Outcome Recording Implementation

```rust
impl L0Remediation {
    /// Record action outcome and update Hebbian pathways
    pub async fn record_outcome(&self, action: &Action, outcome: &Outcome) -> Result<()> {
        // Insert action record
        let action_id = Uuid::new_v4().to_string();

        sqlx::query!(
            r#"
            INSERT INTO l0_actions (
                id, action_id, action_type, service_id,
                confidence, outcome, duration_ms, error_message,
                rollback_performed, context_tensor
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
            action_id,
            action.id,
            action.action_type,
            action.service_id,
            self.confidence,
            outcome.status.to_string(),
            outcome.duration_ms,
            outcome.error_message,
            outcome.rollback_performed,
            MaintenanceTensor::from_current_state().to_bytes()
        )
        .execute(&self.db)
        .await?;

        // Update Hebbian pathway based on outcome
        self.update_hebbian_pathway(action, outcome).await?;

        // Record pulse event for temporal learning
        self.record_pulse_event(action, outcome).await?;

        Ok(())
    }

    /// Update Hebbian pathway strength based on outcome
    async fn update_hebbian_pathway(&self, action: &Action, outcome: &Outcome) -> Result<()> {
        let pathway_id = format!("maintenance_{}", action.action_type);

        // Calculate STDP delta
        let delta = match outcome.status {
            OutcomeStatus::Success => {
                // LTP: Strengthen pathway on success
                let timing_factor = self.calculate_timing_factor(outcome.duration_ms);
                LTP_RATE * timing_factor
            }
            OutcomeStatus::Failure | OutcomeStatus::Timeout => {
                // LTD: Weaken pathway on failure
                -LTD_RATE
            }
            OutcomeStatus::Aborted => {
                // No change for aborted actions
                0.0
            }
        };

        if delta != 0.0 {
            sqlx::query!(
                r#"
                UPDATE hebbian_pathways
                SET
                    pathway_strength = MIN(1.0, MAX(0.0, pathway_strength + ?)),
                    last_activation = CURRENT_TIMESTAMP,
                    activation_count = activation_count + 1
                WHERE pathway_id = ?
                "#,
                delta,
                pathway_id
            )
            .execute(&self.hebbian_db)
            .await?;
        }

        Ok(())
    }
}
```

### 5.3 STDP Configuration

```rust
/// STDP (Spike-Timing-Dependent Plasticity) constants
pub const LTP_RATE: f64 = 0.1;      // Long-Term Potentiation rate
pub const LTD_RATE: f64 = 0.05;     // Long-Term Depression rate
pub const STDP_WINDOW_MS: u64 = 100; // Timing window for STDP effects

/// Calculate timing factor for STDP
/// Faster successful actions get stronger reinforcement
fn calculate_timing_factor(&self, duration_ms: u64) -> f64 {
    let expected_duration = self.action.timeout_seconds * 1000 / 2; // 50% of timeout

    if duration_ms <= expected_duration as u64 {
        // Faster than expected: bonus reinforcement
        1.0 + (1.0 - (duration_ms as f64 / expected_duration as f64)) * 0.5
    } else {
        // Slower than expected: reduced reinforcement
        0.5 + 0.5 * (expected_duration as f64 / duration_ms as f64)
    }
}
```

### 5.4 Pulse Event Recording

```sql
-- Record pulse event for temporal learning
INSERT INTO pulse_events (
    event_type,
    pathway_id,
    strength_delta,
    event_data
) VALUES (
    'l0_remediation',
    'maintenance_' || :action_type,
    :delta,
    json_object(
        'action_id', :action_id,
        'service_id', :service_id,
        'outcome', :outcome,
        'duration_ms', :duration_ms,
        'confidence', :confidence,
        'timestamp', datetime('now')
    )
);
```

---

## 6. L0 Execution Flow

```
┌─────────────────────────────────────────────────────────────────┐
│                    L0 REMEDIATION FLOW                          │
└─────────────────────────────────────────────────────────────────┘

       ┌──────────────┐
       │ Health Issue │
       │   Detected   │
       └──────┬───────┘
              │
              ▼
       ┌──────────────┐
       │  Calculate   │
       │  Confidence  │──────────────────────────┐
       └──────┬───────┘                          │
              │                                  │
              ▼                                  │
       ┌──────────────┐                          │
       │ Confidence   │                          │
       │  >= 0.90 ?   │                          │
       └──────┬───────┘                          │
              │                                  │
     ┌────────┴────────┐                         │
     │ YES             │ NO                      │
     ▼                 ▼                         │
┌──────────┐    ┌───────────┐                    │
│ Check    │    │ Escalate  │                    │
│ Overrides│    │  to L1    │                    │
└────┬─────┘    └───────────┘                    │
     │                                           │
     ▼                                           │
┌──────────────┐                                 │
│ Human Veto?  │─── YES ──▶ L1 Notification     │
│ Agent Dissent?│                                │
│ Circuit Open? │                                │
└──────┬───────┘                                 │
       │ NO                                      │
       ▼                                         │
┌──────────────┐                                 │
│  Execute     │                                 │
│   Action     │                                 │
└──────┬───────┘                                 │
       │                                         │
       ▼                                         │
┌──────────────┐                                 │
│  Record      │                                 │
│  Outcome     │◀────────────────────────────────┘
└──────┬───────┘
       │
       ▼
┌──────────────┐
│   Update     │
│  Hebbian     │
│  Pathways    │
└──────────────┘
```

---

## 7. Monitoring and Observability

### 7.1 L0 Metrics

```rust
/// Prometheus metrics for L0 monitoring
pub struct L0Metrics {
    /// Total L0 actions attempted
    pub actions_attempted: Counter,

    /// L0 actions by outcome
    pub actions_by_outcome: CounterVec<Outcome>,

    /// L0 confidence distribution
    pub confidence_histogram: Histogram,

    /// L0 action duration
    pub duration_histogram: HistogramVec<ActionType>,

    /// L0 escalations to L1
    pub escalations: CounterVec<Reason>,

    /// Current L0 eligibility rate
    pub eligibility_gauge: Gauge,
}
```

### 7.2 Dashboard Queries

```sql
-- L0 success rate over time
SELECT
    date(executed_at) as date,
    action_type,
    COUNT(*) FILTER (WHERE outcome = 'success') * 100.0 / COUNT(*) as success_rate,
    AVG(confidence) as avg_confidence,
    AVG(duration_ms) as avg_duration_ms
FROM l0_actions
WHERE executed_at > datetime('now', '-30 days')
GROUP BY date(executed_at), action_type
ORDER BY date DESC;

-- L0 vs L1 escalation ratio
SELECT
    (SELECT COUNT(*) FROM l0_actions WHERE executed_at > datetime('now', '-24 hours')) as l0_count,
    (SELECT COUNT(*) FROM l1_escalations WHERE escalated_at > datetime('now', '-24 hours')) as l1_count,
    ROUND(
        (SELECT COUNT(*) FROM l0_actions WHERE executed_at > datetime('now', '-24 hours')) * 100.0 /
        NULLIF(
            (SELECT COUNT(*) FROM l0_actions WHERE executed_at > datetime('now', '-24 hours')) +
            (SELECT COUNT(*) FROM l1_escalations WHERE escalated_at > datetime('now', '-24 hours')),
            0
        ),
        2
    ) as l0_autonomy_percentage;
```

---

## 8. References

- **Phase 5 Implementation Plan:** `PART_4_PHASE_5_NAM.md`
- **NAM Gap Analysis:** `NAM_GAP_ANALYSIS_REPORT.md`
- **Hebbian Integration:** `nam/HEBBIAN_INTEGRATION.md`
- **PBFT Consensus:** `nam/PBFT_CONSENSUS.md`
- **Human as Agent:** `nam/HUMAN_AS_AGENT.md`

---

*Document generated for NAM Phase 5 compliance*
*L0 Auto-Remediation: Where machines learn to heal themselves*
