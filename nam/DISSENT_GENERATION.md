# Active Dissent Generation Pipeline (NAM-07)

**Version:** 1.0.0
**Status:** Specification Complete
**NAM Compliance:** NAM-07 (Proactive Disagreement)
**Impact:** R3 DissentCapture from reactive to proactive

---

## 1. Overview

Current dissent capture is **reactive** - dissent is captured when it occurs but not sought. Healthy systems **generate** dissent internally through red teaming and devil's advocacy.

### 1.1 Philosophy

> "The absence of dissent is not the presence of consensus - it's the absence of thinking."

High-confidence decisions (>0.9) should receive **extra scrutiny**, not less. Groupthink is the enemy of robust decision-making.

### 1.2 Key Principles

1. **Proactive, not reactive:** Generate counterarguments before they're needed
2. **High-confidence scrutiny:** "Obvious" decisions get more devil's advocacy
3. **Record everything:** Dissent is valuable even when not acted upon
4. **Learn from ignored dissent:** Track when ignored dissent was correct

---

## 2. Pipeline Definition (PL-DISSENT-001)

### 2.1 YAML Specification

```yaml
pipeline_id: PL-DISSENT-001
name: Active Dissent Generation Pipeline
version: 1.0.0
priority: 2
latency_slo_ms: 1000
throughput_target: 100  # dissents/hour (high for proactive)

description: |
  Proactively generate counterarguments for all proposals,
  especially those with high confidence (>0.9). This implements
  devil's advocacy at the system level.

stages:
  # ============================================================
  # STAGE 1: SOURCE - Capture proposals for dissent analysis
  # ============================================================
  - stage: SOURCE
    description: "Capture proposals and high-confidence decisions"
    sources:
      - type: pre_commit_proposals
        topic: consensus.prepare
        description: "All proposals entering PBFT prepare phase"
        filter: null  # All proposals get dissent analysis

      - type: high_confidence_decisions
        topic: l0.decisions
        filter: confidence > 0.9
        description: |
          Especially scrutinize "obvious" decisions with >90% confidence.
          These are most likely to suffer from groupthink.

      - type: recurring_patterns
        database: episodic_memory.db
        query: |
          SELECT DISTINCT trigger_event, COUNT(*) as frequency
          FROM episodes
          WHERE start_timestamp > datetime('now', '-30 days')
          GROUP BY trigger_event
          HAVING frequency > 5
        description: "Recurring patterns that may have become unexamined"

  # ============================================================
  # STAGE 2: TRANSFORM - Generate dissent
  # ============================================================
  - stage: TRANSFORM
    description: "Apply contrarian strategies to generate dissent"
    processors:
      - name: contrarian_analyzer
        input: proposal
        output: counterarguments
        strategies:
          - id: invert_assumptions
            description: "Negate each premise and explore consequences"
            always_apply: true

          - id: find_edge_cases
            description: "Identify boundary conditions that could fail"
            apply_when: confidence > 0.85

          - id: historical_failures
            description: "Match to past failures with similar patterns"
            apply_when: episodic_memory_available == true

          - id: systemic_side_effects
            description: "Analyze cross-system impact that may be overlooked"
            apply_when: affected_services.count > 1

      - name: minority_voice_generator
        input: counterarguments
        output: formal_dissent
        template: |
          DISSENT RECORD
          ==============
          Proposal ID: {proposal_id}
          Confidence: {confidence}
          Generated At: {timestamp}

          COUNTERARGUMENT
          ---------------
          {argument}

          RISK IF IGNORED
          ---------------
          {risk_assessment}

          ALTERNATIVE ACTION
          ------------------
          {alternative_action}

          DISSENT CONFIDENCE
          ------------------
          {dissent_confidence}

          SUPPORTING EVIDENCE
          -------------------
          {evidence}

      - name: dissent_prioritizer
        input: formal_dissent
        output: prioritized_dissent
        rules:
          - if: dissent_confidence > 0.8
            priority: HIGH
            action: immediate_attention
          - if: dissent_confidence > 0.5
            priority: MEDIUM
            action: record_and_flag
          - default:
            priority: LOW
            action: record_only

  # ============================================================
  # STAGE 3: SINK - Record and route dissent
  # ============================================================
  - stage: SINK
    description: "Record all dissent for learning and analysis"
    sinks:
      - name: dissent_record
        type: database
        connection: consensus_tracking.db
        table: generated_dissent
        # CRITICAL: Record even if not acted upon
        schema:
          - id: TEXT PRIMARY KEY
          - proposal_id: TEXT
          - strategy_used: TEXT
          - argument: TEXT
          - risk_assessment: TEXT
          - alternative_action: TEXT
          - confidence: REAL
          - priority: TEXT
          - acted_upon: BOOLEAN DEFAULT FALSE
          - timestamp: DATETIME

      - name: learning_feedback
        type: database
        connection: hebbian_pulse.db
        table: dissent_pathways
        description: "Track which dissent patterns proved valuable"
        schema:
          - dissent_type: TEXT
          - pathway_strength: REAL
          - times_correct: INTEGER
          - times_ignored_correctly: INTEGER

      - name: notification_sink
        type: notification
        condition: priority == 'HIGH'
        channels: [log, websocket]
        template: |
          HIGH PRIORITY DISSENT GENERATED
          Proposal: {proposal_id}
          Argument: {argument}
          Risk: {risk_assessment}
```

---

## 3. Contrarian Strategies

### 3.1 Strategy Definitions

| Strategy | Description | When Applied | Difficulty |
|----------|-------------|--------------|------------|
| invert_assumptions | Negate each premise and explore consequences | Always | Low |
| find_edge_cases | Identify boundary conditions that could fail | High-confidence proposals | Medium |
| historical_failures | Match to past failures with similar patterns | When episodic memory available | Medium |
| systemic_side_effects | Analyze cross-system impact overlooked | Multi-service proposals | High |

### 3.2 Strategy Implementation

```rust
/// Contrarian strategies for dissent generation
pub trait ContrarianStrategy {
    fn generate_dissent(&self, proposal: &Proposal, context: &Context) -> Vec<Counterargument>;
}

/// Invert each assumption and explore consequences
pub struct InvertAssumptions;

impl ContrarianStrategy for InvertAssumptions {
    fn generate_dissent(&self, proposal: &Proposal, _context: &Context) -> Vec<Counterargument> {
        let mut dissents = Vec::new();

        // Extract implicit assumptions
        let assumptions = extract_assumptions(proposal);

        for assumption in assumptions {
            let inverted = invert_assumption(&assumption);

            // Explore consequences of inverted assumption
            let consequences = analyze_consequences(&inverted, proposal);

            if consequences.severity > Severity::Low {
                dissents.push(Counterargument {
                    strategy: "invert_assumptions".to_string(),
                    original_assumption: assumption.clone(),
                    inverted_assumption: inverted,
                    argument: format!(
                        "If {} is false (not {}), then {}",
                        assumption.text,
                        inverted.text,
                        consequences.description
                    ),
                    risk: consequences.risk_description,
                    confidence: consequences.confidence,
                });
            }
        }

        dissents
    }
}

/// Find edge cases and boundary conditions
pub struct FindEdgeCases;

impl ContrarianStrategy for FindEdgeCases {
    fn generate_dissent(&self, proposal: &Proposal, context: &Context) -> Vec<Counterargument> {
        let mut dissents = Vec::new();

        // Identify parameters with boundaries
        let parameters = extract_parameters(proposal);

        for param in parameters {
            // Test at boundaries
            let edge_cases = vec![
                (param.min_value, "minimum"),
                (param.max_value, "maximum"),
                (param.zero_value, "zero"),
                (param.null_value, "null"),
            ];

            for (value, case_name) in edge_cases {
                if let Some(v) = value {
                    let outcome = simulate_with_value(proposal, &param.name, v);

                    if outcome.is_failure() {
                        dissents.push(Counterargument {
                            strategy: "find_edge_cases".to_string(),
                            original_assumption: format!(
                                "{} will not be at {} value",
                                param.name, case_name
                            ),
                            inverted_assumption: format!(
                                "{} could reach {} value ({})",
                                param.name, case_name, v
                            ),
                            argument: format!(
                                "At {} {} ({}), the action would fail because {}",
                                case_name, param.name, v, outcome.failure_reason
                            ),
                            risk: format!(
                                "{}% probability of hitting this edge case",
                                param.edge_case_probability * 100.0
                            ),
                            confidence: param.edge_case_probability,
                        });
                    }
                }
            }
        }

        dissents
    }
}

/// Match to historical failures
pub struct HistoricalFailures {
    episodic_memory: Arc<EpisodicMemory>,
}

impl ContrarianStrategy for HistoricalFailures {
    fn generate_dissent(&self, proposal: &Proposal, context: &Context) -> Vec<Counterargument> {
        let mut dissents = Vec::new();

        // Get current state tensor
        let current_tensor = MaintenanceTensor::from_current_state();

        // Find similar past episodes that failed
        let similar_failures = self.episodic_memory
            .recall_similar(&current_tensor, 10)
            .await?
            .into_iter()
            .filter(|e| e.episode.outcome == Some("failure".to_string()))
            .filter(|e| e.similarity > 0.6)
            .collect::<Vec<_>>();

        for failure in similar_failures {
            dissents.push(Counterargument {
                strategy: "historical_failures".to_string(),
                original_assumption: format!(
                    "This action will succeed (confidence: {:.1}%)",
                    proposal.confidence * 100.0
                ),
                inverted_assumption: format!(
                    "Similar action failed on {} with {:.1}% similarity",
                    failure.episode.start_timestamp,
                    failure.similarity * 100.0
                ),
                argument: format!(
                    "A similar situation occurred on {} and resulted in failure. " +
                    "The failure reason was: {}. Current similarity: {:.1}%",
                    failure.episode.start_timestamp,
                    failure.episode.narrative.as_deref().unwrap_or("Unknown"),
                    failure.similarity * 100.0
                ),
                risk: format!(
                    "Lessons from past failure: {:?}",
                    failure.episode.lessons_learned
                ),
                confidence: failure.similarity,
            });
        }

        dissents
    }
}

/// Analyze systemic side effects
pub struct SystemicSideEffects;

impl ContrarianStrategy for SystemicSideEffects {
    fn generate_dissent(&self, proposal: &Proposal, context: &Context) -> Vec<Counterargument> {
        let mut dissents = Vec::new();

        // Identify affected services
        let directly_affected = &proposal.target_services;

        // Find indirectly affected through dependency graph
        let indirectly_affected = find_dependent_services(directly_affected);

        for service in indirectly_affected {
            let impact = analyze_impact_on_service(&service, proposal);

            if impact.severity >= Severity::Medium {
                dissents.push(Counterargument {
                    strategy: "systemic_side_effects".to_string(),
                    original_assumption: format!(
                        "Action only affects {}",
                        directly_affected.join(", ")
                    ),
                    inverted_assumption: format!(
                        "{} is indirectly affected through dependency chain",
                        service.id
                    ),
                    argument: format!(
                        "Service {} depends on {} and will experience {} impact. " +
                        "Predicted effect: {}",
                        service.id,
                        impact.dependency_path.join(" -> "),
                        impact.severity,
                        impact.description
                    ),
                    risk: format!(
                        "Cascade risk: {:.1}%. Synergy impact: -{:.1}%",
                        impact.cascade_probability * 100.0,
                        impact.synergy_reduction * 100.0
                    ),
                    confidence: impact.confidence,
                });
            }
        }

        dissents
    }
}
```

---

## 4. Dissent Value Tracking

### 4.1 Outcome Tracking Schema

```sql
-- Track which generated dissent proved valuable
CREATE TABLE dissent_outcomes (
    id TEXT PRIMARY KEY,
    dissent_id TEXT NOT NULL,
    proposal_id TEXT NOT NULL,

    -- Dissent details
    strategy_used TEXT NOT NULL,
    dissent_confidence REAL NOT NULL,

    -- What happened
    was_acted_upon BOOLEAN NOT NULL,
    proposal_outcome TEXT,  -- success, failure, partial

    -- Post-hoc analysis
    dissent_would_have_helped BOOLEAN,
    analysis_notes TEXT,

    -- Learning applied
    learning_applied BOOLEAN DEFAULT FALSE,
    pathway_delta REAL,

    -- Timestamps
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    analyzed_at DATETIME,

    FOREIGN KEY (dissent_id) REFERENCES generated_dissent(id),
    FOREIGN KEY (proposal_id) REFERENCES consensus_proposals(id)
);

-- Index for finding valuable ignored dissent
CREATE INDEX idx_dissent_outcomes_ignored ON dissent_outcomes(was_acted_upon, dissent_would_have_helped);
```

### 4.2 Valuable Ignored Dissent View

```sql
-- View: Dissent that was correct but ignored
CREATE VIEW v_valuable_ignored_dissent AS
SELECT
    d.id as dissent_id,
    d.proposal_id,
    d.strategy_used,
    d.argument,
    d.risk_assessment,
    d.confidence as dissent_confidence,
    o.proposal_outcome,
    o.analysis_notes,
    p.action_type as proposal_action_type,
    p.proposer_agent
FROM generated_dissent d
JOIN dissent_outcomes o ON d.id = o.dissent_id
JOIN consensus_proposals p ON d.proposal_id = p.id
WHERE o.was_acted_upon = FALSE
AND o.dissent_would_have_helped = TRUE
ORDER BY d.timestamp DESC;

-- View: Most valuable dissent strategies
CREATE VIEW v_dissent_strategy_performance AS
SELECT
    strategy_used,
    COUNT(*) as times_generated,
    SUM(CASE WHEN was_acted_upon THEN 1 ELSE 0 END) as times_acted_upon,
    SUM(CASE WHEN dissent_would_have_helped THEN 1 ELSE 0 END) as times_valuable,
    AVG(dissent_confidence) as avg_confidence,
    SUM(CASE WHEN dissent_would_have_helped AND NOT was_acted_upon THEN 1 ELSE 0 END) as valuable_but_ignored
FROM dissent_outcomes
GROUP BY strategy_used
ORDER BY times_valuable DESC;
```

### 4.3 Learning from Ignored Dissent

```rust
/// Post-hoc analysis of dissent value
pub struct DissentAnalyzer {
    db: Pool<Sqlite>,
    hebbian: HebbianIntegration,
}

impl DissentAnalyzer {
    /// Analyze whether ignored dissent would have helped
    pub async fn analyze_post_hoc(
        &self,
        dissent_id: &str,
        proposal_outcome: &ProposalOutcome,
    ) -> Result<()> {
        let dissent = self.get_dissent(dissent_id).await?;

        // Determine if dissent would have helped
        let would_have_helped = match proposal_outcome.status {
            OutcomeStatus::Failure => {
                // Check if failure matches dissent prediction
                self.failure_matches_dissent(&dissent, proposal_outcome)
            }
            OutcomeStatus::PartialFailure => {
                // Partial failures may have been avoidable
                dissent.confidence > 0.7
            }
            OutcomeStatus::Success => {
                // Success despite dissent - dissent was wrong
                false
            }
        };

        // Record outcome
        sqlx::query!(
            r#"
            INSERT INTO dissent_outcomes (
                id, dissent_id, proposal_id, strategy_used,
                dissent_confidence, was_acted_upon, proposal_outcome,
                dissent_would_have_helped, analyzed_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP)
            "#,
            Uuid::new_v4().to_string(),
            dissent_id,
            dissent.proposal_id,
            dissent.strategy_used,
            dissent.confidence,
            dissent.acted_upon,
            proposal_outcome.status.to_string(),
            would_have_helped
        )
        .execute(&self.db)
        .await?;

        // Update Hebbian pathways
        if would_have_helped && !dissent.acted_upon {
            // Dissent was valuable but ignored - strengthen strategy
            self.hebbian.update_pathway(
                &format!("dissent_strategy_{}", dissent.strategy_used),
                LTP_RATE * 2.0,  // Double reinforcement for valuable ignored dissent
            ).await?;

            // Also strengthen the dissenting agent's pathway
            if let Some(agent) = &dissent.generating_agent {
                self.hebbian.update_pathway(
                    &format!("agent_{}_dissent", agent),
                    LTP_RATE * 1.5,
                ).await?;
            }
        } else if !would_have_helped && dissent.acted_upon {
            // Dissent was followed but was wrong - weaken slightly
            self.hebbian.update_pathway(
                &format!("dissent_strategy_{}", dissent.strategy_used),
                -LTD_RATE * 0.5,
            ).await?;
        }

        Ok(())
    }

    /// Check if proposal failure matches what dissent predicted
    fn failure_matches_dissent(
        &self,
        dissent: &GeneratedDissent,
        outcome: &ProposalOutcome,
    ) -> bool {
        // Extract keywords from dissent
        let dissent_keywords = extract_keywords(&dissent.argument);

        // Extract keywords from failure
        let failure_keywords = extract_keywords(
            outcome.failure_reason.as_deref().unwrap_or("")
        );

        // Calculate overlap
        let overlap = dissent_keywords.intersection(&failure_keywords).count();
        let total = dissent_keywords.len().max(1);

        // If >30% keyword overlap, consider it a match
        overlap as f64 / total as f64 > 0.3
    }
}
```

---

## 5. Generated Dissent Database

### 5.1 Full Schema

```sql
-- Generated dissent records
CREATE TABLE generated_dissent (
    id TEXT PRIMARY KEY,
    proposal_id TEXT NOT NULL,
    strategy_used TEXT NOT NULL,

    -- Dissent content
    original_assumption TEXT,
    inverted_assumption TEXT,
    argument TEXT NOT NULL,
    risk_assessment TEXT,
    alternative_action TEXT,

    -- Scoring
    confidence REAL NOT NULL,
    priority TEXT CHECK(priority IN ('HIGH', 'MEDIUM', 'LOW')),

    -- Status
    acted_upon BOOLEAN DEFAULT FALSE,
    acted_upon_at DATETIME,
    action_taken TEXT,

    -- Agent information
    generating_agent TEXT,  -- Which agent role generated this
    generating_strategy_version TEXT,

    -- Metadata
    timestamp DATETIME DEFAULT CURRENT_TIMESTAMP,

    FOREIGN KEY (proposal_id) REFERENCES consensus_proposals(id)
);

CREATE INDEX idx_dissent_proposal ON generated_dissent(proposal_id);
CREATE INDEX idx_dissent_strategy ON generated_dissent(strategy_used);
CREATE INDEX idx_dissent_priority ON generated_dissent(priority);
CREATE INDEX idx_dissent_acted ON generated_dissent(acted_upon);
```

---

## 6. Dissent Pathway Learning

### 6.1 Pathway Schema

```sql
-- Hebbian pathways for dissent strategies
INSERT INTO hebbian_pathways (
    pathway_id, source_module, target_module, pathway_strength
) VALUES
    ('dissent_strategy_invert_assumptions', 'dissent', 'invert_assumptions', 0.5),
    ('dissent_strategy_find_edge_cases', 'dissent', 'find_edge_cases', 0.5),
    ('dissent_strategy_historical_failures', 'dissent', 'historical_failures', 0.5),
    ('dissent_strategy_systemic_side_effects', 'dissent', 'systemic_side_effects', 0.5)
ON CONFLICT(pathway_id) DO NOTHING;
```

### 6.2 Strategy Selection Based on Pathway Strength

```rust
/// Select dissent strategies based on Hebbian pathway strength
pub async fn select_strategies(
    db: &Pool<Sqlite>,
    proposal: &Proposal,
) -> Vec<Box<dyn ContrarianStrategy>> {
    let mut strategies: Vec<(Box<dyn ContrarianStrategy>, f64)> = Vec::new();

    // Get pathway strengths for each strategy
    let strengths: HashMap<String, f64> = sqlx::query!(
        r#"
        SELECT pathway_id, pathway_strength
        FROM hebbian_pathways
        WHERE source_module = 'dissent'
        "#
    )
    .fetch_all(db)
    .await?
    .into_iter()
    .map(|r| (r.pathway_id, r.pathway_strength))
    .collect();

    // Always include invert_assumptions (base strategy)
    strategies.push((Box::new(InvertAssumptions), 1.0));

    // Include others based on pathway strength and context
    if proposal.confidence > 0.85 {
        let strength = *strengths.get("dissent_strategy_find_edge_cases").unwrap_or(&0.5);
        if strength > 0.3 {  // Threshold for inclusion
            strategies.push((Box::new(FindEdgeCases), strength));
        }
    }

    if episodic_memory_available() {
        let strength = *strengths.get("dissent_strategy_historical_failures").unwrap_or(&0.5);
        if strength > 0.3 {
            strategies.push((Box::new(HistoricalFailures::new()), strength));
        }
    }

    if proposal.target_services.len() > 1 {
        let strength = *strengths.get("dissent_strategy_systemic_side_effects").unwrap_or(&0.5);
        if strength > 0.3 {
            strategies.push((Box::new(SystemicSideEffects), strength));
        }
    }

    // Sort by pathway strength (strongest first)
    strategies.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

    strategies.into_iter().map(|(s, _)| s).collect()
}
```

---

## 7. Acceptance Criteria

- [ ] PL-DISSENT-001 pipeline implemented
- [ ] All 4 contrarian strategies active
- [ ] High-confidence decisions receive extra scrutiny (>0.9 confidence)
- [ ] Dissent recorded even when not acted upon
- [ ] Pathway weights updated when dissent proved correct
- [ ] v_valuable_ignored_dissent view operational

---

## 8. References

- **PBFT Consensus:** `nam/PBFT_CONSENSUS.md`
- **Agent Roles:** `nam/AGENT_ROLES.md`
- **Episodic Memory:** `nam/EPISODIC_MEMORY.md`
- **Hebbian Integration:** `nam/HEBBIAN_INTEGRATION.md`

---

*Document generated for NAM Phase 5 compliance*
*Dissent Generation: Where the system plays devil's advocate to itself*
