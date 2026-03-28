# Self-Model View Specification (NAM-01)

**Version:** 1.0.0
**Status:** Specification Complete
**NAM Compliance:** NAM-01 (SelfQuery Enhancement)
**Impact:** Enable recursive self-introspection

---

## 1. Overview

The `v_self_model` view enables the maintenance engine to answer the fundamental question: **"What am I doing right now?"** This represents a shift from external monitoring to internal self-awareness.

### 1.1 Philosophy

In NAM architecture, systems should not rely on external observers to understand their own state. The system should be able to query itself recursively to understand:
- Current activities
- Active pathways
- Resource utilization
- Decision context
- Temporal position in remediation chains

### 1.2 NAM-01 Requirement

> R1 SelfQuery: The system must be able to introspect its own state through autonomous tensor/SQL query loops.

---

## 2. v_self_model View Specification

### 2.1 Core View Definition

```sql
-- v_self_model: Recursive self-introspection view
-- Answers: "What am I doing right now?"

CREATE VIEW v_self_model AS
WITH

-- Current active actions
active_actions AS (
    SELECT
        action_id,
        action_type,
        service_id,
        confidence,
        started_at,
        (julianday('now') - julianday(started_at)) * 86400 as duration_seconds,
        status
    FROM active_remediation_actions
    WHERE status IN ('running', 'pending')
),

-- Active Hebbian pathways (recently activated)
active_pathways AS (
    SELECT
        pathway_id,
        source_module,
        target_module,
        pathway_strength,
        last_activation,
        activation_count
    FROM hebbian_pathways
    WHERE last_activation > datetime('now', '-5 minutes')
    ORDER BY pathway_strength DESC
    LIMIT 10
),

-- Current consensus proposals
pending_consensus AS (
    SELECT
        id as proposal_id,
        action_type,
        proposer_agent,
        status,
        (SELECT COUNT(*) FROM consensus_votes WHERE proposal_id = cp.id AND vote = 'approve') as votes_for,
        (SELECT COUNT(*) FROM consensus_votes WHERE proposal_id = cp.id AND vote = 'reject') as votes_against
    FROM consensus_proposals cp
    WHERE status IN ('pending', 'prepared')
),

-- System health snapshot
system_health AS (
    SELECT
        (SELECT AVG(health_score) FROM services WHERE status = 'running') as avg_health,
        (SELECT AVG(synergy_score) FROM system_synergy) as avg_synergy,
        (SELECT COUNT(*) FROM services WHERE health_score < 0.8) as unhealthy_count,
        (SELECT COUNT(*) FROM active_alerts WHERE severity >= 'medium') as active_alerts
),

-- Current L0/L1/L2 activity distribution
escalation_distribution AS (
    SELECT
        COALESCE(
            (SELECT COUNT(*) FROM l0_actions WHERE executed_at > datetime('now', '-1 hour')), 0
        ) as l0_count,
        COALESCE(
            (SELECT COUNT(*) FROM l1_escalations WHERE escalated_at > datetime('now', '-1 hour')), 0
        ) as l1_count,
        COALESCE(
            (SELECT COUNT(*) FROM l2_escalations WHERE escalated_at > datetime('now', '-1 hour')), 0
        ) as l2_count
)

SELECT
    -- Timestamp of self-query
    datetime('now') as query_timestamp,

    -- Active actions summary
    (SELECT json_group_array(json_object(
        'action_id', action_id,
        'action_type', action_type,
        'service_id', service_id,
        'confidence', confidence,
        'duration_seconds', duration_seconds
    )) FROM active_actions) as current_actions,

    -- Active pathways summary
    (SELECT json_group_array(json_object(
        'pathway_id', pathway_id,
        'strength', pathway_strength,
        'source', source_module,
        'target', target_module
    )) FROM active_pathways) as hot_pathways,

    -- Pending consensus
    (SELECT json_group_array(json_object(
        'proposal_id', proposal_id,
        'action_type', action_type,
        'votes_for', votes_for,
        'votes_against', votes_against
    )) FROM pending_consensus) as pending_consensus,

    -- Health snapshot
    (SELECT avg_health FROM system_health) as system_health,
    (SELECT avg_synergy FROM system_health) as system_synergy,
    (SELECT unhealthy_count FROM system_health) as unhealthy_services,
    (SELECT active_alerts FROM system_health) as active_alert_count,

    -- Escalation activity
    (SELECT l0_count FROM escalation_distribution) as l0_actions_last_hour,
    (SELECT l1_count FROM escalation_distribution) as l1_escalations_last_hour,
    (SELECT l2_count FROM escalation_distribution) as l2_escalations_last_hour,

    -- Autonomy ratio (L0 / total)
    CASE
        WHEN (SELECT l0_count + l1_count + l2_count FROM escalation_distribution) > 0
        THEN ROUND(
            (SELECT l0_count FROM escalation_distribution) * 100.0 /
            (SELECT l0_count + l1_count + l2_count FROM escalation_distribution),
            1
        )
        ELSE 100.0
    END as autonomy_percentage,

    -- Self-description
    CASE
        WHEN (SELECT COUNT(*) FROM active_actions) = 0
            THEN 'IDLE: No active remediation actions'
        WHEN (SELECT COUNT(*) FROM active_actions) = 1
            THEN 'WORKING: Single action in progress'
        WHEN (SELECT COUNT(*) FROM pending_consensus) > 0
            THEN 'COORDINATING: Awaiting consensus on critical action'
        ELSE 'BUSY: Multiple concurrent remediations'
    END as self_state_description;
```

### 2.2 Query Example

```sql
-- "What am I doing right now?"
SELECT * FROM v_self_model;

-- Response example:
-- {
--   "query_timestamp": "2026-01-28T10:30:00Z",
--   "current_actions": [
--     {"action_id": "act_001", "action_type": "service_restart", "service_id": "synthex", "confidence": 0.92, "duration_seconds": 15.3}
--   ],
--   "hot_pathways": [
--     {"pathway_id": "maintenance_restart", "strength": 0.87, "source": "maintenance", "target": "service_restart"}
--   ],
--   "pending_consensus": [],
--   "system_health": 0.94,
--   "system_synergy": 0.97,
--   "unhealthy_services": 1,
--   "active_alert_count": 0,
--   "l0_actions_last_hour": 12,
--   "l1_escalations_last_hour": 2,
--   "l2_escalations_last_hour": 0,
--   "autonomy_percentage": 85.7,
--   "self_state_description": "WORKING: Single action in progress"
-- }
```

---

## 3. Rust Implementation

```rust
/// Self-model for recursive introspection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelfModel {
    pub query_timestamp: DateTime<Utc>,
    pub current_actions: Vec<ActiveAction>,
    pub hot_pathways: Vec<ActivePathway>,
    pub pending_consensus: Vec<PendingProposal>,
    pub system_health: f64,
    pub system_synergy: f64,
    pub unhealthy_services: u32,
    pub active_alert_count: u32,
    pub l0_actions_last_hour: u32,
    pub l1_escalations_last_hour: u32,
    pub l2_escalations_last_hour: u32,
    pub autonomy_percentage: f64,
    pub self_state_description: String,
}

impl SelfModel {
    /// Query the system's current self-model
    pub async fn query(db: &Pool<Sqlite>) -> Result<Self> {
        let row = sqlx::query!(
            "SELECT * FROM v_self_model"
        )
        .fetch_one(db)
        .await?;

        Ok(Self {
            query_timestamp: Utc::now(),
            current_actions: serde_json::from_str(&row.current_actions.unwrap_or_default())?,
            hot_pathways: serde_json::from_str(&row.hot_pathways.unwrap_or_default())?,
            pending_consensus: serde_json::from_str(&row.pending_consensus.unwrap_or_default())?,
            system_health: row.system_health.unwrap_or(0.0),
            system_synergy: row.system_synergy.unwrap_or(0.0),
            unhealthy_services: row.unhealthy_services.unwrap_or(0) as u32,
            active_alert_count: row.active_alert_count.unwrap_or(0) as u32,
            l0_actions_last_hour: row.l0_actions_last_hour.unwrap_or(0) as u32,
            l1_escalations_last_hour: row.l1_escalations_last_hour.unwrap_or(0) as u32,
            l2_escalations_last_hour: row.l2_escalations_last_hour.unwrap_or(0) as u32,
            autonomy_percentage: row.autonomy_percentage.unwrap_or(100.0),
            self_state_description: row.self_state_description.unwrap_or_default(),
        })
    }

    /// Check if system is currently busy
    pub fn is_busy(&self) -> bool {
        !self.current_actions.is_empty() || !self.pending_consensus.is_empty()
    }

    /// Check if system is healthy
    pub fn is_healthy(&self) -> bool {
        self.system_health >= 0.8 && self.unhealthy_services == 0
    }

    /// Get dominant activity
    pub fn dominant_activity(&self) -> &str {
        if !self.pending_consensus.is_empty() {
            "consensus"
        } else if !self.current_actions.is_empty() {
            "remediation"
        } else {
            "monitoring"
        }
    }
}
```

---

## 4. Self-Query Loop

```rust
/// Autonomous self-query loop for continuous introspection
impl MaintenanceEngine {
    /// Run self-query loop (every 30 seconds)
    pub async fn run_self_query_loop(&self) -> Result<()> {
        let mut interval = tokio::time::interval(Duration::from_secs(30));

        loop {
            interval.tick().await;

            let self_model = SelfModel::query(&self.db).await?;

            // Log current state for observability
            tracing::info!(
                state = %self_model.self_state_description,
                health = self_model.system_health,
                autonomy = self_model.autonomy_percentage,
                "Self-query result"
            );

            // Take action based on self-awareness
            if self_model.system_health < 0.7 {
                self.trigger_health_remediation().await?;
            }

            if self_model.autonomy_percentage < 50.0 {
                tracing::warn!(
                    "Low autonomy detected ({}%), investigating escalation patterns",
                    self_model.autonomy_percentage
                );
            }

            // Store self-model snapshot for episodic memory
            self.record_self_model_snapshot(&self_model).await?;
        }
    }
}
```

---

## 5. References

- **NAM Gap Analysis:** `NAM_GAP_ANALYSIS_REPORT.md` (R1 SelfQuery)
- **Implementation Plan:** `PART_4_PHASE_5_NAM.md` (Section 5.9)
- **Episodic Memory:** `nam/EPISODIC_MEMORY.md`

---

*Document generated for NAM Phase 5 compliance*
*Self-Model: The system that knows what it is doing*
