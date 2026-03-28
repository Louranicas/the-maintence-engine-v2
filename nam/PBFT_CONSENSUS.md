# PBFT Consensus Layer

**Version:** 1.0.0
**Status:** Specification Complete
**NAM Compliance:** Phase 5, P0-3 Priority
**Impact:** Byzantine Fault Tolerance for Critical Decisions

---

## 1. Overview

The PBFT (Practical Byzantine Fault Tolerance) consensus layer enables the maintenance engine to make critical decisions through distributed agreement among the 40-agent CVA-NAM fleet. This ensures that no single point of failure or Byzantine (malicious/faulty) agent can compromise system integrity.

### 1.1 Configuration

```
n = 40 agents (CVA-NAM fleet)
f = 13 (Byzantine fault tolerance: floor((n-1)/3))
q = 27 (quorum requirement: 2f + 1)

Safety: Tolerates up to 13 Byzantine (faulty/malicious) agents
Liveness: Requires 27+ honest agents for progress
```

### 1.2 Agent Distribution

| Tier | Agent Range | Count | Role | Vote Weight |
|------|-------------|-------|------|-------------|
| 0 | @0.A | 1 | Human (Peer) | 1.0 |
| 1 | @1.01-@1.05 | 5 | Foundation | 1.0 |
| 2 | @2.06-@2.13 | 8 | Core | 1.0 |
| 3 | @3.14-@3.21 | 8 | Testing | 1.0 |
| 4 | @4.22-@4.26 | 5 | Optimization | 1.0 |
| 5 | @5.27-@5.32 | 6 | Integration | 1.0 |
| 6 | @6.33-@6.40 | 8 | Hardening | 1.0 |

---

## 2. Consensus-Required Actions

### 2.1 Action Categories

| Action Category | Quorum | Timeout | Fallback | Risk Level |
|-----------------|--------|---------|----------|------------|
| Emergency kill -9 | 27 | 60s | Human @0.A decides | CRITICAL |
| Database migration | 27 | 300s | Abort | CRITICAL |
| Credential rotation | 27 | 120s | Retry with backoff | HIGH |
| Multi-service restart | 27 | 180s | Sequential L1 | HIGH |
| Configuration rollback | 27 | 90s | Abort | MEDIUM |
| Tier promotion/demotion | 27 | 120s | Maintain current | MEDIUM |
| Fleet scaling (add/remove agents) | 27 | 180s | Maintain current | MEDIUM |
| Security policy change | 27 | 240s | Abort | CRITICAL |
| Compliance exception | 27 | 300s | Human @0.A required | CRITICAL |

### 2.2 Determining Consensus Requirement

```rust
/// Determine if an action requires PBFT consensus
pub fn requires_consensus(action: &Action) -> bool {
    matches!(action.action_type,
        ActionType::EmergencyKill |
        ActionType::DatabaseMigration |
        ActionType::CredentialRotation |
        ActionType::MultiServiceRestart |
        ActionType::ConfigurationRollback |
        ActionType::TierChange |
        ActionType::FleetScaling |
        ActionType::SecurityPolicyChange |
        ActionType::ComplianceException
    )
}

/// Get consensus parameters for an action
pub fn get_consensus_params(action: &Action) -> ConsensusParams {
    match action.action_type {
        ActionType::EmergencyKill => ConsensusParams {
            quorum: 27,
            timeout: Duration::from_secs(60),
            fallback: Fallback::HumanDecision,
        },
        ActionType::DatabaseMigration => ConsensusParams {
            quorum: 27,
            timeout: Duration::from_secs(300),
            fallback: Fallback::Abort,
        },
        // ... other action types
        _ => ConsensusParams::default(),
    }
}
```

---

## 3. PBFT Phases

### 3.1 Phase 1: Pre-Prepare

The proposer agent broadcasts the action to all 40 agents.

```rust
/// Pre-prepare message structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrePrepare {
    pub view: u64,             // Current view number
    pub sequence: u64,         // Sequence number for ordering
    pub action: CriticalAction,
    pub proposer: AgentId,
    pub timestamp: DateTime<Utc>,
    pub digest: [u8; 32],      // SHA-256 of action
    pub signature: Vec<u8>,    // Proposer's signature
}

impl PrePrepare {
    /// Create a new pre-prepare message
    pub fn new(action: CriticalAction, proposer: AgentId, view: u64, sequence: u64) -> Self {
        let digest = action.compute_digest();
        Self {
            view,
            sequence,
            action,
            proposer,
            timestamp: Utc::now(),
            digest,
            signature: vec![], // Filled by signing
        }
    }
}

/// Broadcast pre-prepare to all 40 agents
pub async fn broadcast_pre_prepare(
    pp: PrePrepare,
    agents: &[Agent],
) -> Result<Vec<PrePareResult>> {
    let futures = agents.iter().map(|agent| {
        let pp_clone = pp.clone();
        async move {
            let result = agent.send(Message::PrePrepare(pp_clone)).await;
            PrePareResult {
                agent_id: agent.id.clone(),
                accepted: result.is_ok(),
                error: result.err().map(|e| e.to_string()),
            }
        }
    });

    let results = futures::future::join_all(futures).await;

    // Log broadcast results
    tracing::info!(
        accepted = results.iter().filter(|r| r.accepted).count(),
        rejected = results.iter().filter(|r| !r.accepted).count(),
        "Pre-prepare broadcast complete"
    );

    Ok(results)
}
```

### 3.2 Phase 2: Prepare

Each agent validates the pre-prepare and broadcasts a prepare message if accepted.

```rust
/// Prepare message structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Prepare {
    pub view: u64,
    pub sequence: u64,
    pub digest: [u8; 32],
    pub agent_id: AgentId,
    pub vote: Vote,
    pub reasoning: Option<String>,
    pub timestamp: DateTime<Utc>,
    pub signature: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Vote {
    Approve,
    Reject { reason: String },
    Abstain,
}

/// Collect prepare messages until quorum or timeout
pub async fn collect_prepares(
    view: u64,
    sequence: u64,
    timeout: Duration,
) -> Result<PrepareResult> {
    let mut prepares: Vec<Prepare> = Vec::new();
    let mut approve_count = 0;
    let mut reject_count = 0;
    let deadline = Instant::now() + timeout;

    while Instant::now() < deadline {
        // Check if we have enough votes to decide
        if approve_count >= QUORUM {
            return Ok(PrepareResult::QuorumReached { prepares });
        }
        if reject_count > (N - QUORUM) {
            return Ok(PrepareResult::QuorumImpossible { prepares });
        }

        // Wait for next prepare message
        match tokio::time::timeout(
            Duration::from_millis(100),
            receive_prepare(view, sequence)
        ).await {
            Ok(Ok(prepare)) => {
                match prepare.vote {
                    Vote::Approve => approve_count += 1,
                    Vote::Reject { .. } => reject_count += 1,
                    Vote::Abstain => {}
                }
                prepares.push(prepare);
            }
            Ok(Err(e)) => {
                tracing::warn!(error = %e, "Error receiving prepare");
            }
            Err(_) => {
                // Timeout on this receive, continue loop
            }
        }
    }

    // Timeout reached
    if approve_count >= QUORUM {
        Ok(PrepareResult::QuorumReached { prepares })
    } else {
        Ok(PrepareResult::Timeout {
            prepares,
            approve_count,
            reject_count,
        })
    }
}
```

### 3.3 Phase 3: Commit

Once prepare quorum is reached, agents broadcast commit messages and execute the action.

```rust
/// Commit message structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Commit {
    pub view: u64,
    pub sequence: u64,
    pub digest: [u8; 32],
    pub agent_id: AgentId,
    pub timestamp: DateTime<Utc>,
    pub signature: Vec<u8>,
}

/// Execute action after commit quorum
pub async fn commit_action(
    action: CriticalAction,
    prepares: Vec<Prepare>,
) -> Result<CommitResult> {
    // Verify quorum
    let approve_count = prepares.iter()
        .filter(|p| matches!(p.vote, Vote::Approve))
        .count();

    if approve_count < QUORUM {
        return Err(ConsensusError::InsufficientVotes {
            required: QUORUM,
            received: approve_count,
        });
    }

    // Record consensus decision before execution
    record_consensus_decision(&action, &prepares).await?;

    // Execute the action
    let execution_result = action.execute().await;

    // Broadcast commit to all agents
    let commit = Commit {
        view: action.view,
        sequence: action.sequence,
        digest: action.digest,
        agent_id: get_local_agent_id(),
        timestamp: Utc::now(),
        signature: sign_commit(&action)?,
    };

    broadcast_commit(commit).await?;

    // Record execution outcome
    record_execution_outcome(&action, &execution_result).await?;

    Ok(CommitResult {
        action_id: action.id,
        executed: execution_result.is_ok(),
        votes_for: approve_count as u32,
        votes_against: (prepares.len() - approve_count) as u32,
    })
}
```

---

## 4. Dissent Handling

When an agent dissents (votes Reject), the dissent is captured for learning.

### 4.1 Dissent Schema

```sql
-- Dissent events table for capturing disagreements
CREATE TABLE dissent_events (
    id TEXT PRIMARY KEY,
    proposal_id TEXT NOT NULL,
    proposed_action TEXT NOT NULL,
    dissenting_agent TEXT NOT NULL,
    dissent_reason TEXT NOT NULL,
    dissent_confidence REAL,           -- Agent's confidence in dissent
    outcome TEXT CHECK(outcome IN (
        'PENDING',                      -- Not yet resolved
        'OVERRIDDEN',                   -- Dissent overridden by quorum
        'ACCEPTED',                     -- Dissent led to action abort
        'RESOLVED',                     -- Compromise found
        'VINDICATED'                    -- Post-hoc proved dissent correct
    )),
    learned_pattern TEXT,              -- What was learned from this dissent
    timestamp DATETIME DEFAULT CURRENT_TIMESTAMP,
    resolved_at DATETIME,

    FOREIGN KEY (dissenting_agent) REFERENCES agent_registry(agent_id),
    FOREIGN KEY (proposal_id) REFERENCES consensus_proposals(id)
);

CREATE INDEX idx_dissent_agent ON dissent_events(dissenting_agent);
CREATE INDEX idx_dissent_outcome ON dissent_events(outcome);
CREATE INDEX idx_dissent_pending ON dissent_events(outcome) WHERE outcome = 'PENDING';
```

### 4.2 Recording Dissent

```rust
/// Record agent dissent with full context
pub async fn record_dissent(
    db: &Pool<Sqlite>,
    proposal_id: &str,
    dissenting_agent: &AgentId,
    reason: &str,
    confidence: f64,
) -> Result<String> {
    let dissent_id = Uuid::new_v4().to_string();

    // Get the proposed action for context
    let proposal = get_proposal(db, proposal_id).await?;

    sqlx::query!(
        r#"
        INSERT INTO dissent_events (
            id, proposal_id, proposed_action, dissenting_agent,
            dissent_reason, dissent_confidence, outcome, timestamp
        ) VALUES (?, ?, ?, ?, ?, ?, 'PENDING', CURRENT_TIMESTAMP)
        "#,
        dissent_id,
        proposal_id,
        proposal.action_type,
        dissenting_agent.to_string(),
        reason,
        confidence
    )
    .execute(db)
    .await?;

    // Notify interested parties of dissent
    publish_dissent_event(DissentEvent {
        dissent_id: dissent_id.clone(),
        proposal_id: proposal_id.to_string(),
        dissenting_agent: dissenting_agent.clone(),
        reason: reason.to_string(),
    }).await?;

    Ok(dissent_id)
}
```

### 4.3 Post-Hoc Dissent Validation

```rust
/// After action execution, check if dissent was correct
pub async fn validate_dissent_post_hoc(
    db: &Pool<Sqlite>,
    dissent_id: &str,
    action_outcome: &ActionOutcome,
) -> Result<()> {
    // Get the dissent record
    let dissent = get_dissent(db, dissent_id).await?;

    // Determine if dissent was vindicated
    let vindicated = match &action_outcome.status {
        OutcomeStatus::Failure | OutcomeStatus::PartialFailure => {
            // Action failed - dissent may have been correct
            true
        }
        OutcomeStatus::Success => {
            // Action succeeded - dissent was not vindicated
            false
        }
        _ => false,
    };

    // Update dissent outcome
    let new_outcome = if vindicated {
        "VINDICATED"
    } else if dissent.outcome == "PENDING" {
        "OVERRIDDEN"
    } else {
        &dissent.outcome
    };

    sqlx::query!(
        r#"
        UPDATE dissent_events
        SET outcome = ?, resolved_at = CURRENT_TIMESTAMP
        WHERE id = ?
        "#,
        new_outcome,
        dissent_id
    )
    .execute(db)
    .await?;

    // If vindicated, strengthen the dissenting agent's pathway
    if vindicated {
        update_hebbian_pathway(
            &format!("agent_{}_dissent", dissent.dissenting_agent),
            LTP_RATE * 2.0, // Double reinforcement for correct dissent
        ).await?;

        // Record learned pattern
        record_learned_pattern(dissent_id, action_outcome).await?;
    }

    Ok(())
}
```

---

## 5. Consensus Database Schema

### 5.1 Core Tables

```sql
-- Consensus proposals
CREATE TABLE consensus_proposals (
    id TEXT PRIMARY KEY,
    view_number INTEGER NOT NULL,
    sequence_number INTEGER NOT NULL,
    action_type TEXT NOT NULL,
    action_data TEXT NOT NULL,         -- JSON serialized action
    proposer_agent TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending' CHECK(status IN (
        'pending',                      -- Awaiting votes
        'prepared',                     -- Prepare quorum reached
        'committed',                    -- Commit phase complete
        'executed',                     -- Action executed
        'aborted',                      -- Consensus failed
        'timeout'                       -- Deadline exceeded
    )),
    digest BLOB NOT NULL,              -- SHA-256 of action_data
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    prepared_at DATETIME,
    committed_at DATETIME,
    executed_at DATETIME,

    FOREIGN KEY (proposer_agent) REFERENCES agent_registry(agent_id)
);

-- Consensus votes
CREATE TABLE consensus_votes (
    proposal_id TEXT NOT NULL,
    agent_id TEXT NOT NULL,
    vote TEXT NOT NULL CHECK(vote IN ('approve', 'reject', 'abstain')),
    phase TEXT NOT NULL CHECK(phase IN ('prepare', 'commit')),
    reasoning TEXT,
    timestamp DATETIME DEFAULT CURRENT_TIMESTAMP,
    signature BLOB,

    PRIMARY KEY (proposal_id, agent_id, phase),
    FOREIGN KEY (proposal_id) REFERENCES consensus_proposals(id),
    FOREIGN KEY (agent_id) REFERENCES agent_registry(agent_id)
);

-- Consensus outcomes
CREATE TABLE consensus_outcomes (
    proposal_id TEXT PRIMARY KEY,
    quorum_reached BOOLEAN NOT NULL,
    votes_for INTEGER NOT NULL,
    votes_against INTEGER NOT NULL,
    votes_abstain INTEGER NOT NULL,
    execution_status TEXT CHECK(execution_status IN (
        'success', 'failure', 'partial', 'rollback'
    )),
    execution_error TEXT,
    completed_at DATETIME,

    FOREIGN KEY (proposal_id) REFERENCES consensus_proposals(id)
);
```

### 5.2 Indexes

```sql
-- Performance indexes
CREATE INDEX idx_proposals_status ON consensus_proposals(status);
CREATE INDEX idx_proposals_view ON consensus_proposals(view_number);
CREATE INDEX idx_proposals_sequence ON consensus_proposals(sequence_number);
CREATE INDEX idx_votes_proposal ON consensus_votes(proposal_id);
CREATE INDEX idx_votes_agent ON consensus_votes(agent_id);
CREATE INDEX idx_outcomes_status ON consensus_outcomes(execution_status);
```

### 5.3 Views

```sql
-- Active proposals requiring votes
CREATE VIEW v_pending_proposals AS
SELECT
    p.id,
    p.action_type,
    p.proposer_agent,
    p.created_at,
    COUNT(v.agent_id) as vote_count,
    SUM(CASE WHEN v.vote = 'approve' THEN 1 ELSE 0 END) as approve_count,
    SUM(CASE WHEN v.vote = 'reject' THEN 1 ELSE 0 END) as reject_count
FROM consensus_proposals p
LEFT JOIN consensus_votes v ON p.id = v.proposal_id AND v.phase = 'prepare'
WHERE p.status = 'pending'
GROUP BY p.id;

-- Consensus success rate by action type
CREATE VIEW v_consensus_stats AS
SELECT
    p.action_type,
    COUNT(*) as total_proposals,
    SUM(CASE WHEN o.quorum_reached THEN 1 ELSE 0 END) as quorum_reached_count,
    SUM(CASE WHEN o.execution_status = 'success' THEN 1 ELSE 0 END) as success_count,
    AVG(o.votes_for) as avg_votes_for,
    AVG(julianday(p.executed_at) - julianday(p.created_at)) * 86400 as avg_duration_seconds
FROM consensus_proposals p
JOIN consensus_outcomes o ON p.id = o.proposal_id
WHERE p.status IN ('executed', 'aborted')
GROUP BY p.action_type;
```

---

## 6. View Change Protocol

When the primary (proposer) fails, view change ensures continued progress.

### 6.1 View Change Trigger

```rust
/// Conditions that trigger view change
pub enum ViewChangeTrigger {
    PrimaryTimeout,           // Primary hasn't proposed in expected time
    PrimaryByzantine,         // Primary sending conflicting messages
    QuorumRequest,            // 2f+1 agents request view change
}

/// Initiate view change
pub async fn initiate_view_change(
    trigger: ViewChangeTrigger,
    current_view: u64,
) -> Result<u64> {
    let new_view = current_view + 1;
    let new_primary = select_primary(new_view);

    // Broadcast view change request
    let vc_request = ViewChangeRequest {
        new_view,
        trigger,
        requesting_agent: get_local_agent_id(),
        prepared_proofs: get_prepared_proofs(current_view).await?,
        timestamp: Utc::now(),
    };

    broadcast_view_change(vc_request).await?;

    // Wait for view change quorum
    let vc_responses = collect_view_change_responses(new_view).await?;

    if vc_responses.len() >= QUORUM {
        // New view established
        tracing::info!(new_view = new_view, new_primary = %new_primary, "View change complete");
        Ok(new_view)
    } else {
        Err(ConsensusError::ViewChangeFailed)
    }
}

/// Select primary for a given view
fn select_primary(view: u64) -> AgentId {
    // Round-robin selection across non-human agents
    let agent_index = (view as usize) % (N - 1); // Exclude @0.A
    AgentId::from_index(agent_index + 1) // Start from @1.01
}
```

---

## 7. Integration with Other NAM Components

### 7.1 With L0 Auto-Remediation

```rust
// L0 escalates to PBFT when confidence is low or action is critical
impl L0Remediation {
    async fn maybe_escalate_to_pbft(&self, action: &Action) -> Result<EscalationDecision> {
        if requires_consensus(action) {
            return Ok(EscalationDecision::RequiresPBFT);
        }

        if self.confidence < 0.9 && action.severity >= Severity::Medium {
            return Ok(EscalationDecision::RecommendPBFT);
        }

        Ok(EscalationDecision::L0Eligible)
    }
}
```

### 7.2 With Human @0.A

```rust
// Include human in consensus with extended timeout
impl PBFTConsensus {
    async fn include_human_vote(&self, proposal: &Proposal) -> Result<Option<Vote>> {
        // Human gets longer timeout (60s vs 5s for machines)
        collect_human_vote(proposal, Duration::from_secs(60)).await
    }
}
```

### 7.3 With Hebbian Learning

```rust
// Update pathways based on consensus outcomes
impl HebbianIntegration {
    async fn on_consensus_complete(&self, outcome: &ConsensusOutcome) -> Result<()> {
        // Strengthen pathways for agents who voted correctly
        for vote in &outcome.votes {
            let correct = (vote.vote == Vote::Approve) == outcome.success;
            let delta = if correct { LTP_RATE } else { -LTD_RATE };

            self.update_agent_pathway(&vote.agent_id, delta).await?;
        }

        Ok(())
    }
}
```

---

## 8. Monitoring and Observability

### 8.1 Prometheus Metrics

```rust
pub struct PBFTMetrics {
    pub proposals_total: CounterVec<ActionType>,
    pub quorum_reached: CounterVec<ActionType>,
    pub consensus_duration: HistogramVec<ActionType>,
    pub votes_per_proposal: Histogram,
    pub view_changes: Counter,
    pub dissent_count: CounterVec<AgentId>,
}
```

### 8.2 Audit Log

```sql
-- All consensus activity for compliance
CREATE TABLE consensus_audit_log (
    id TEXT PRIMARY KEY,
    event_type TEXT NOT NULL,
    proposal_id TEXT,
    agent_id TEXT,
    details TEXT,
    timestamp DATETIME DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_audit_proposal ON consensus_audit_log(proposal_id);
CREATE INDEX idx_audit_agent ON consensus_audit_log(agent_id);
CREATE INDEX idx_audit_timestamp ON consensus_audit_log(timestamp);
```

---

## 9. References

- **Castro & Liskov (1999):** "Practical Byzantine Fault Tolerance"
- **L0 Auto-Remediation:** `nam/L0_AUTO_REMEDIATION.md`
- **Human as Agent:** `nam/HUMAN_AS_AGENT.md`
- **Hebbian Integration:** `nam/HEBBIAN_INTEGRATION.md`
- **Dissent Generation:** `nam/DISSENT_GENERATION.md`

---

*Document generated for NAM Phase 5 compliance*
*PBFT Consensus: Where 40 minds agree before acting*
