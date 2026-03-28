# Human as Agent (@0.A) Specification

**Version:** 1.0.0
**Status:** Specification Complete
**NAM Compliance:** Phase 5, P0-2 Priority
**Impact:** R5 HumanAsAgent +77%

---

## 1. Philosophy

In NAM (Non-Anthropocentric Memory) architecture, humans are not privileged external supervisors but **peer participants** in a multi-agent system. The human is registered as agent `@0.A` with the same capabilities, tracking, and accountability as machine agents.

### 1.1 Key Paradigm Shifts

| Traditional (Human-Centric) | NAM (Human as Agent) |
|----------------------------|---------------------|
| Human approves all critical actions | Human participates in consensus |
| Human decisions are final authority | Human decisions are weighted votes |
| Human overrides machine judgment | Human dissent is recorded for learning |
| Human actions are not tracked | Human actions logged as agent actions |
| Human has special privileges | Human is peer (@0.A) in agent registry |

### 1.2 Why @0.A?

The designation `@0.A` encodes semantic meaning:
- `@` = Agent namespace prefix
- `0` = Tier 0 (foundation tier, not superior tier)
- `A` = First agent (alpha) in human class

This creates symmetry with machine agents (`@1.01` through `@6.40` across tiers 1-6).

---

## 2. Agent Registration

### 2.1 Registration SQL

```sql
-- Register human as agent @0.A in the agent registry
-- This is a ONE-TIME operation during system initialization

INSERT INTO agent_registry (
    agent_id,
    agent_type,
    agent_class,
    tier,
    vote_weight,
    participation_mode,
    capabilities,
    created_at,
    last_active
) VALUES (
    '@0.A',                                    -- Unique agent identifier
    'human',                                   -- Agent type
    'biological',                              -- Agent class
    0,                                         -- Tier 0 (foundation)
    1.0,                                       -- Vote weight (equal to machine agents)
    'peer',                                    -- Not 'supervisor' or 'admin'
    json_array(
        'consensus_vote',                      -- Participate in PBFT consensus
        'dissent',                             -- Register disagreement
        'override_request',                    -- Request (not demand) override
        'escalation_response',                 -- Respond to L1+ escalations
        'hebbian_feedback',                    -- Provide learning feedback
        'episode_annotation'                   -- Annotate episodic memories
    ),
    CURRENT_TIMESTAMP,
    CURRENT_TIMESTAMP
);

-- Create index for fast @0.A lookups
CREATE INDEX IF NOT EXISTS idx_agent_registry_human
ON agent_registry(agent_id) WHERE agent_type = 'human';
```

### 2.2 Rust Registration

```rust
/// Human agent registration structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HumanAgent {
    pub agent_id: String,      // Always '@0.A' for primary human
    pub agent_type: AgentType,
    pub tier: u8,
    pub vote_weight: f64,
    pub participation_mode: ParticipationMode,
    pub capabilities: Vec<Capability>,
}

impl HumanAgent {
    /// Create the canonical @0.A human agent
    pub fn primary() -> Self {
        Self {
            agent_id: "@0.A".to_string(),
            agent_type: AgentType::Human,
            tier: 0,
            vote_weight: 1.0,
            participation_mode: ParticipationMode::Peer,
            capabilities: vec![
                Capability::ConsensusVote,
                Capability::Dissent,
                Capability::OverrideRequest,
                Capability::EscalationResponse,
                Capability::HebbianFeedback,
                Capability::EpisodeAnnotation,
            ],
        }
    }

    /// Register human agent in database
    pub async fn register(db: &Pool<Sqlite>) -> Result<()> {
        let agent = Self::primary();

        sqlx::query!(
            r#"
            INSERT OR REPLACE INTO agent_registry (
                agent_id, agent_type, tier, vote_weight,
                participation_mode, capabilities, last_active
            ) VALUES (?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP)
            "#,
            agent.agent_id,
            agent.agent_type.to_string(),
            agent.tier,
            agent.vote_weight,
            agent.participation_mode.to_string(),
            serde_json::to_string(&agent.capabilities)?
        )
        .execute(db)
        .await?;

        Ok(())
    }
}
```

---

## 3. Action Tracking

All human decisions are recorded as agent actions, identical to machine agent actions.

### 3.1 Action Recording Schema

```sql
-- Human actions table (shared with all agents)
CREATE TABLE IF NOT EXISTS agent_actions (
    id TEXT PRIMARY KEY,
    agent_id TEXT NOT NULL,
    action_type TEXT NOT NULL,
    target TEXT NOT NULL,
    outcome TEXT CHECK(outcome IN ('success', 'failure', 'pending', 'aborted')),
    weight_delta REAL DEFAULT 0.0,
    reasoning TEXT,                    -- Human reasoning captured
    confidence REAL,                   -- Human-reported confidence
    context_tensor BLOB,               -- 12D tensor at action time
    timestamp DATETIME DEFAULT CURRENT_TIMESTAMP,

    FOREIGN KEY (agent_id) REFERENCES agent_registry(agent_id)
);

CREATE INDEX idx_agent_actions_agent ON agent_actions(agent_id);
CREATE INDEX idx_agent_actions_type ON agent_actions(action_type);
CREATE INDEX idx_agent_actions_timestamp ON agent_actions(timestamp);
```

### 3.2 Recording Human Actions

```rust
/// Record a human action as @0.A agent action
pub async fn record_human_action(
    db: &Pool<Sqlite>,
    action: HumanAction,
) -> Result<String> {
    let action_id = Uuid::new_v4().to_string();

    // Capture current system state as tensor
    let context_tensor = MaintenanceTensor::from_current_state().to_bytes();

    sqlx::query!(
        r#"
        INSERT INTO agent_actions (
            id, agent_id, action_type, target, outcome,
            weight_delta, reasoning, confidence, context_tensor, timestamp
        ) VALUES (?, '@0.A', ?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP)
        "#,
        action_id,
        action.action_type.to_string(),
        action.target,
        action.outcome.to_string(),
        action.weight_delta,
        action.reasoning,
        action.confidence,
        context_tensor
    )
    .execute(db)
    .await?;

    // Update human agent's last_active timestamp
    sqlx::query!(
        "UPDATE agent_registry SET last_active = CURRENT_TIMESTAMP WHERE agent_id = '@0.A'"
    )
    .execute(db)
    .await?;

    Ok(action_id)
}
```

### 3.3 Example Human Actions

```sql
-- Human approves a remediation
INSERT INTO agent_actions (
    id, agent_id, action_type, target, outcome,
    weight_delta, reasoning, timestamp
) VALUES (
    'action_001',
    '@0.A',
    'approved_remediation',
    'synthex_restart',
    'success',
    0.05,
    'Service was unresponsive for >5 minutes, restart appropriate',
    CURRENT_TIMESTAMP
);

-- Human dissents from proposed action
INSERT INTO agent_actions (
    id, agent_id, action_type, target, outcome,
    weight_delta, reasoning, timestamp
) VALUES (
    'action_002',
    '@0.A',
    'dissent',
    'database_vacuum_during_peak',
    'pending',
    -0.02,
    'Peak traffic period, VACUUM should wait for maintenance window',
    CURRENT_TIMESTAMP
);

-- Human provides learning feedback
INSERT INTO agent_actions (
    id, agent_id, action_type, target, outcome,
    weight_delta, reasoning, timestamp
) VALUES (
    'action_003',
    '@0.A',
    'hebbian_feedback',
    'pathway_maintenance_restart',
    'success',
    0.03,
    'Restart resolved issue quickly, pathway should be strengthened',
    CURRENT_TIMESTAMP
);
```

---

## 4. PBFT Participation Rules

Human @0.A participates in PBFT consensus as an equal agent.

### 4.1 Voting Parameters

| Parameter | Human @0.A | Machine Agents |
|-----------|-----------|----------------|
| Vote Weight | 1.0 | 1.0 |
| Quorum Contribution | Counts toward 27/40 | Counts toward 27/40 |
| Veto Power | None (equal participant) | None |
| Dissent Recording | Yes (captured for learning) | Yes |
| Timeout for Vote | 60 seconds | 5 seconds |

### 4.2 Human Vote Collection

```rust
/// Collect human vote for PBFT consensus
pub async fn collect_human_vote(
    proposal: &ConsensusProposal,
    timeout: Duration,
) -> Result<Option<Vote>> {
    // Notify human of pending vote
    notify_human(NotificationType::ConsensusVote {
        proposal_id: proposal.id.clone(),
        proposal_type: proposal.action_type.clone(),
        deadline: Instant::now() + timeout,
    }).await?;

    // Wait for human response with timeout
    let vote = tokio::time::timeout(
        timeout,
        wait_for_human_vote(&proposal.id)
    ).await;

    match vote {
        Ok(Ok(vote)) => {
            // Record vote as agent action
            record_human_action(&HumanAction {
                action_type: ActionType::ConsensusVote,
                target: proposal.id.clone(),
                outcome: Outcome::Success,
                reasoning: vote.reasoning.clone(),
                ..Default::default()
            }).await?;

            Ok(Some(vote))
        }
        Ok(Err(e)) => Err(e),
        Err(_) => {
            // Timeout - human abstains (not blocks)
            tracing::info!(
                proposal_id = %proposal.id,
                "Human @0.A vote timeout, counting as abstain"
            );
            Ok(None)
        }
    }
}
```

### 4.3 Dissent Capture

```rust
/// Record human dissent for learning
pub async fn record_human_dissent(
    db: &Pool<Sqlite>,
    proposal_id: &str,
    dissent_reason: &str,
) -> Result<()> {
    // Record in dissent_events table
    sqlx::query!(
        r#"
        INSERT INTO dissent_events (
            id, proposed_action, dissenting_agent,
            dissent_reason, outcome, timestamp
        ) VALUES (?, ?, '@0.A', ?, 'PENDING', CURRENT_TIMESTAMP)
        "#,
        Uuid::new_v4().to_string(),
        proposal_id,
        dissent_reason
    )
    .execute(db)
    .await?;

    // Also record as agent action for synergy tracking
    record_human_action(&HumanAction {
        action_type: ActionType::Dissent,
        target: proposal_id.to_string(),
        outcome: Outcome::Pending,
        reasoning: Some(dissent_reason.to_string()),
        weight_delta: -0.02, // Small negative weight for dissent
        ..Default::default()
    }).await?;

    Ok(())
}
```

---

## 5. Synergy Tracking

Human synergy is tracked identically to machine agents.

### 5.1 Synergy Query

```sql
-- Human @0.A synergy over last 30 days
SELECT
    agent_id,
    AVG(CASE WHEN outcome = 'success' THEN 1.0 ELSE 0.0 END) as success_rate,
    COUNT(*) as total_actions,
    SUM(weight_delta) as total_contribution,
    AVG(confidence) as avg_confidence,
    MAX(timestamp) as last_active
FROM agent_actions
WHERE agent_id = '@0.A'
AND timestamp > datetime('now', '-30 days')
GROUP BY agent_id;
```

### 5.2 Cross-Agent Synergy

```sql
-- Synergy between human @0.A and machine agents
SELECT
    aa1.agent_id as agent_1,
    aa2.agent_id as agent_2,
    COUNT(*) as interactions,
    AVG(CASE
        WHEN aa1.outcome = aa2.outcome THEN 1.0
        ELSE 0.0
    END) as agreement_rate
FROM agent_actions aa1
JOIN agent_actions aa2 ON aa1.target = aa2.target
    AND aa1.agent_id = '@0.A'
    AND aa2.agent_id != '@0.A'
    AND ABS(julianday(aa1.timestamp) - julianday(aa2.timestamp)) < 0.01  -- Within ~15 minutes
WHERE aa1.timestamp > datetime('now', '-30 days')
GROUP BY aa2.agent_id
ORDER BY agreement_rate DESC;
```

---

## 6. Integration Points

### 6.1 With L0 Auto-Remediation

```rust
// L0 notifies human @0.A before autonomous action
impl L0Remediation {
    async fn notify_human_before_action(&self, action: &Action) -> Result<()> {
        if action.severity >= Severity::Medium {
            notify_human(NotificationType::L0Action {
                action_id: action.id.clone(),
                veto_window_seconds: 60,
            }).await?;
        }
        Ok(())
    }
}
```

### 6.2 With Hebbian Learning

```rust
// Human feedback strengthens/weakens pathways
impl HebbianIntegration {
    async fn apply_human_feedback(&self, feedback: &HumanFeedback) -> Result<()> {
        let delta = match feedback.direction {
            FeedbackDirection::Strengthen => LTP_RATE * 0.5, // Human boost
            FeedbackDirection::Weaken => -LTD_RATE * 0.5,
        };

        self.update_pathway(feedback.pathway_id, delta).await
    }
}
```

---

## 7. References

- **L0 Auto-Remediation:** `nam/L0_AUTO_REMEDIATION.md`
- **PBFT Consensus:** `nam/PBFT_CONSENSUS.md`
- **Hebbian Integration:** `nam/HEBBIAN_INTEGRATION.md`
- **NAM Gap Analysis:** `NAM_GAP_ANALYSIS_REPORT.md` (R5 HumanAsAgent)

---

*Document generated for NAM Phase 5 compliance*
*Human as Agent: Where biological and silicon minds collaborate as peers*
