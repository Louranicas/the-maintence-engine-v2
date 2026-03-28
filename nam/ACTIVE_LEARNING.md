# Active Learning Pipeline (NAM-04)

**Version:** 1.0.0
**Status:** Specification Complete
**NAM Compliance:** NAM-04 (Active Exploration)
**Impact:** Transform passive learning to active exploration

---

## 1. Overview

The Active Learning Pipeline (`PL-EXPLORE-001`) enables the maintenance engine to **actively explore uncertain pathways** rather than waiting passively for events. This transforms the system from reactive to proactive learning.

### 1.1 Philosophy

Current STDP implementation is **passive** - pathways only change in response to events. Biological systems exhibit curiosity, play, and active exploration. The system should **experiment** to reduce uncertainty.

> "The best way to learn is not to wait for mistakes, but to seek out uncertainty and resolve it."

### 1.2 Key Principles

1. **Uncertainty-Driven:** Target pathways with 0.3-0.7 strength (uncertain zone)
2. **Risk-Bounded:** Never experiment on production-critical services
3. **Budget-Constrained:** Maximum experiments per day
4. **Window-Aware:** Experiments during low-traffic periods only

---

## 2. Pipeline Definition (PL-EXPLORE-001)

### 2.1 YAML Specification

```yaml
pipeline_id: PL-EXPLORE-001
name: Autonomous Exploration Pipeline
version: 1.0.0
priority: 4
latency_slo_ms: 60000  # 60 seconds (exploration can take time)
throughput_target: 10  # experiments/hour

description: |
  Enable the system to actively explore uncertain pathways rather than
  waiting passively for events. Experiments are risk-assessed and
  bounded by exploration budget.

stages:
  # ============================================================
  # STAGE 1: SOURCE - Identify uncertain pathways
  # ============================================================
  - stage: SOURCE
    description: "Identify pathways in the uncertain zone (0.3-0.7 strength)"
    sources:
      - type: low_confidence_pathways
        database: hebbian_pulse.db
        query: |
          SELECT
            pathway_id,
            source_module,
            target_module,
            pathway_strength,
            activation_count,
            last_activation,
            julianday('now') - julianday(last_activation) as days_since_activation
          FROM hebbian_pathways
          WHERE pathway_strength BETWEEN 0.3 AND 0.7  -- Uncertain zone
          AND (
            last_activation IS NULL
            OR last_activation < datetime('now', '-24 hours')
          )
          ORDER BY
            -- Prioritize pathways with few activations (more uncertain)
            activation_count ASC,
            -- Then by age since last activation
            days_since_activation DESC
          LIMIT 10
        refresh_interval_seconds: 3600  # Re-query hourly

      - type: dormant_pathways
        database: hebbian_pulse.db
        query: |
          SELECT
            pathway_id,
            source_module,
            target_module,
            pathway_strength
          FROM hebbian_pathways
          WHERE last_activation IS NULL
          OR last_activation < datetime('now', '-30 days')
          LIMIT 5
        refresh_interval_seconds: 86400  # Re-query daily

  # ============================================================
  # STAGE 2: TRANSFORM - Generate experiments
  # ============================================================
  - stage: TRANSFORM
    description: "Generate safe experiments for uncertain pathways"
    processors:
      - name: experiment_generator
        input: uncertain_pathway
        output: experiment_proposal
        strategies:
          - id: probe_with_synthetic_load
            description: "Generate test traffic to probe pathway behavior"
            applicable_when: pathway.source_module == 'maintenance'
            risk_level: LOW

          - id: vary_timing_parameters
            description: "Adjust STDP windows to test timing sensitivity"
            applicable_when: pathway.activation_count > 0
            risk_level: LOW

          - id: simulate_failure_mode
            description: "Controlled fault injection to test recovery"
            applicable_when: pathway.target_module in ['service_restart', 'health_remediate']
            risk_level: MEDIUM

          - id: boundary_testing
            description: "Test threshold boundaries to find optimal values"
            applicable_when: pathway.source_module == 'maintenance'
            risk_level: MEDIUM

          - id: counterfactual_replay
            description: "Replay historical events with different pathway strengths"
            applicable_when: true  # Always applicable
            risk_level: LOW

      - name: risk_assessor
        input: experiment_proposal
        output: safe_experiment
        constraints:
          - max_impact_score: 0.1
          - require_rollback_plan: true
          - exclude_production_critical: true
          - exclude_during_incidents: true
        validation:
          - name: service_criticality_check
            rule: "target_service.tier > 3"  # Only tier 4-6 services
          - name: time_window_check
            rule: "current_hour in exploration_hours"
          - name: budget_check
            rule: "experiments_today < daily_budget"

  # ============================================================
  # STAGE 3: ROUTE - Decide experiment tier
  # ============================================================
  - stage: ROUTE
    description: "Route experiments based on risk level"
    rules:
      - condition: risk_score < 0.05
        target: [execute_sink]
        tier: L0_AUTO_EXECUTE
        description: "Very low risk - execute autonomously"

      - condition: risk_score < 0.2
        target: [notify_sink, execute_sink]
        tier: L1_NOTIFY_HUMAN
        description: "Low risk - notify human, then execute"

      - condition: risk_score < 0.4
        target: [defer_sink]
        tier: L2_REQUIRE_APPROVAL
        description: "Medium risk - queue for human approval"

      - default:
        target: [reject_sink]
        tier: REJECT
        description: "High risk - reject experiment"

  # ============================================================
  # STAGE 4: SINK - Execute and learn
  # ============================================================
  - stage: SINK
    description: "Execute experiments and record outcomes"
    sinks:
      - name: execute_sink
        type: experiment_executor
        timeout_ms: 30000
        on_complete: learning_sink
        on_failure: rollback_sink
        config:
          capture_before_state: true
          capture_after_state: true
          capture_timing: true

      - name: learning_sink
        type: database
        connection: hebbian_pulse.db
        table: pathway_experiments
        schema:
          - pathway_id: TEXT
          - experiment_type: TEXT
          - before_strength: REAL
          - after_strength: REAL
          - outcome: TEXT
          - duration_ms: INTEGER
          - timestamp: DATETIME

      - name: notify_sink
        type: notification
        channels: [log, websocket]
        template: |
          EXPERIMENT SCHEDULED
          Pathway: {pathway_id}
          Type: {experiment_type}
          Risk: {risk_score}
          Window: {execution_window}

      - name: defer_sink
        type: queue
        queue_name: pending_experiments
        ttl_hours: 24

      - name: rollback_sink
        type: rollback_executor
        strategy: restore_before_state
        notify_on_rollback: true

      - name: reject_sink
        type: log
        level: INFO
        message: "Experiment rejected: {rejection_reason}"
```

---

## 3. Experiment Types

### 3.1 Type Definitions

| Type | Description | Risk Level | Duration |
|------|-------------|------------|----------|
| probe_with_synthetic_load | Generate test traffic to probe behavior | LOW | 10-30s |
| vary_timing_parameters | Adjust STDP windows to test sensitivity | LOW | 5-15s |
| simulate_failure_mode | Controlled fault injection | MEDIUM | 30-60s |
| boundary_testing | Test threshold boundaries | MEDIUM | 15-45s |
| counterfactual_replay | Replay events with different weights | LOW | 5-20s |

### 3.2 Experiment Implementation

```rust
/// Experiment types for active learning
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExperimentType {
    /// Generate synthetic load to probe pathway behavior
    ProbeWithSyntheticLoad {
        load_level: f64,        // 0.0-1.0
        duration_seconds: u32,
    },

    /// Adjust STDP timing windows
    VaryTimingParameters {
        original_window_ms: u64,
        test_window_ms: u64,
    },

    /// Controlled fault injection
    SimulateFailureMode {
        failure_type: FailureType,
        recovery_expected_ms: u64,
    },

    /// Test threshold boundaries
    BoundaryTesting {
        threshold_name: String,
        test_values: Vec<f64>,
    },

    /// Replay with different pathway strengths
    CounterfactualReplay {
        event_id: String,
        hypothetical_strength: f64,
    },
}

impl ExperimentType {
    /// Calculate risk score for experiment
    pub fn risk_score(&self, context: &ExperimentContext) -> f64 {
        let base_risk = match self {
            ExperimentType::ProbeWithSyntheticLoad { load_level, .. } => {
                0.02 + (*load_level * 0.08)
            }
            ExperimentType::VaryTimingParameters { .. } => 0.03,
            ExperimentType::SimulateFailureMode { failure_type, .. } => {
                0.1 + failure_type.severity() * 0.2
            }
            ExperimentType::BoundaryTesting { .. } => 0.05,
            ExperimentType::CounterfactualReplay { .. } => 0.01,
        };

        // Adjust for context
        let tier_factor = 1.0 - (context.service_tier as f64 / 6.0);
        let time_factor = if context.is_maintenance_window { 0.5 } else { 1.5 };

        (base_risk * tier_factor * time_factor).clamp(0.0, 1.0)
    }
}
```

---

## 4. Risk Assessment

### 4.1 Risk Assessment Algorithm

```rust
/// Assess risk for a proposed experiment
pub struct RiskAssessor {
    max_impact_score: f64,
    require_rollback_plan: bool,
    exclude_tiers: Vec<u8>,
}

impl RiskAssessor {
    pub fn assess(&self, experiment: &ExperimentProposal) -> RiskAssessment {
        let mut risk_factors = Vec::new();
        let mut total_risk = 0.0;

        // Factor 1: Experiment type base risk
        let type_risk = experiment.experiment_type.base_risk();
        risk_factors.push(RiskFactor::new("experiment_type", type_risk));
        total_risk += type_risk * 0.3;

        // Factor 2: Target service criticality
        let service_risk = 1.0 - (experiment.target_service.tier as f64 / 6.0);
        risk_factors.push(RiskFactor::new("service_criticality", service_risk));
        total_risk += service_risk * 0.25;

        // Factor 3: Time window appropriateness
        let time_risk = if is_maintenance_window() { 0.1 } else { 0.5 };
        risk_factors.push(RiskFactor::new("time_window", time_risk));
        total_risk += time_risk * 0.2;

        // Factor 4: Historical experiment success rate
        let historical_risk = 1.0 - self.get_historical_success_rate(experiment);
        risk_factors.push(RiskFactor::new("historical_success", historical_risk));
        total_risk += historical_risk * 0.15;

        // Factor 5: Current system health
        let health_risk = 1.0 - get_current_system_health();
        risk_factors.push(RiskFactor::new("system_health", health_risk));
        total_risk += health_risk * 0.1;

        // Determine approval tier
        let tier = match total_risk {
            r if r < 0.05 => ExperimentTier::L0AutoExecute,
            r if r < 0.2 => ExperimentTier::L1NotifyHuman,
            r if r < 0.4 => ExperimentTier::L2RequireApproval,
            _ => ExperimentTier::Reject,
        };

        RiskAssessment {
            total_risk,
            risk_factors,
            tier,
            rollback_plan: experiment.rollback_plan.clone(),
            approved: total_risk <= self.max_impact_score,
        }
    }
}
```

### 4.2 Risk Constraints

```rust
/// Risk constraints for experiment approval
pub struct RiskConstraints {
    /// Maximum allowable impact score (0.0-1.0)
    pub max_impact_score: f64,

    /// Require a rollback plan for all experiments
    pub require_rollback_plan: bool,

    /// Exclude production-critical services
    pub exclude_production_critical: bool,

    /// Exclude during active incidents
    pub exclude_during_incidents: bool,

    /// Minimum time since last experiment on same pathway
    pub cooldown_hours: u32,
}

impl Default for RiskConstraints {
    fn default() -> Self {
        Self {
            max_impact_score: 0.1,
            require_rollback_plan: true,
            exclude_production_critical: true,
            exclude_during_incidents: true,
            cooldown_hours: 24,
        }
    }
}
```

---

## 5. Exploration Budget

### 5.1 Budget Configuration

```toml
[exploration]
# Maximum experiments per day
budget_per_day = 100

# Maximum cumulative risk score per day
risk_budget = 0.5

# Low-traffic windows for experimentation (UTC)
exploration_hours = ["02:00-06:00", "14:00-16:00"]

# Cooldown between experiments on same pathway
pathway_cooldown_hours = 24

# Maximum concurrent experiments
max_concurrent = 3

# Budget allocation by tier
[exploration.tier_budget]
tier_4 = 50  # 50% of budget for tier 4
tier_5 = 35  # 35% of budget for tier 5
tier_6 = 15  # 15% of budget for tier 6

# Rollover unused budget (max 20% of daily)
rollover_max_percentage = 20
```

### 5.2 Budget Enforcement

```rust
/// Exploration budget manager
pub struct ExplorationBudget {
    daily_limit: u32,
    risk_budget: f64,
    exploration_hours: Vec<(NaiveTime, NaiveTime)>,
    tier_allocation: HashMap<u8, f64>,
}

impl ExplorationBudget {
    /// Check if experiment is within budget
    pub async fn check_budget(&self, db: &Pool<Sqlite>, experiment: &ExperimentProposal) -> BudgetResult {
        // Count experiments today
        let today_count: i64 = sqlx::query_scalar!(
            r#"
            SELECT COUNT(*) FROM pathway_experiments
            WHERE date(timestamp) = date('now')
            "#
        )
        .fetch_one(db)
        .await?
        .unwrap_or(0);

        if today_count >= self.daily_limit as i64 {
            return BudgetResult::ExhaustedDaily;
        }

        // Check cumulative risk
        let today_risk: f64 = sqlx::query_scalar!(
            r#"
            SELECT COALESCE(SUM(risk_score), 0.0) FROM pathway_experiments
            WHERE date(timestamp) = date('now')
            "#
        )
        .fetch_one(db)
        .await?
        .unwrap_or(0.0);

        if today_risk + experiment.risk_score > self.risk_budget {
            return BudgetResult::ExhaustedRisk;
        }

        // Check time window
        let now = Utc::now().time();
        let in_window = self.exploration_hours.iter().any(|(start, end)| {
            now >= *start && now <= *end
        });

        if !in_window {
            return BudgetResult::OutsideWindow;
        }

        // Check tier allocation
        let tier = experiment.target_service.tier;
        let tier_count = self.count_tier_experiments_today(db, tier).await?;
        let tier_limit = (self.daily_limit as f64 * self.tier_allocation.get(&tier).unwrap_or(&0.0)) as u32;

        if tier_count >= tier_limit {
            return BudgetResult::TierLimitReached;
        }

        BudgetResult::Approved {
            remaining_daily: self.daily_limit - today_count as u32 - 1,
            remaining_risk: self.risk_budget - today_risk - experiment.risk_score,
        }
    }
}
```

---

## 6. Experiment Execution

### 6.1 Executor Implementation

```rust
/// Execute experiments safely with state capture
pub struct ExperimentExecutor {
    rollback_enabled: bool,
    capture_state: bool,
    timeout: Duration,
}

impl ExperimentExecutor {
    pub async fn execute(&self, experiment: &SafeExperiment) -> ExperimentOutcome {
        // Capture before state
        let before_state = if self.capture_state {
            Some(self.capture_system_state(&experiment.target_pathway).await?)
        } else {
            None
        };

        // Record start
        let start_time = Instant::now();

        // Execute with timeout
        let result = tokio::time::timeout(
            self.timeout,
            self.run_experiment(&experiment)
        ).await;

        let duration = start_time.elapsed();

        // Capture after state
        let after_state = if self.capture_state {
            Some(self.capture_system_state(&experiment.target_pathway).await?)
        } else {
            None
        };

        // Evaluate outcome
        match result {
            Ok(Ok(experiment_result)) => {
                ExperimentOutcome {
                    success: true,
                    before_strength: before_state.map(|s| s.pathway_strength),
                    after_strength: after_state.map(|s| s.pathway_strength),
                    duration_ms: duration.as_millis() as u64,
                    learned: experiment_result.learned_insights,
                    error: None,
                }
            }
            Ok(Err(e)) => {
                // Experiment failed - trigger rollback
                if self.rollback_enabled {
                    self.rollback(&experiment, &before_state).await?;
                }
                ExperimentOutcome {
                    success: false,
                    before_strength: before_state.map(|s| s.pathway_strength),
                    after_strength: None,
                    duration_ms: duration.as_millis() as u64,
                    learned: None,
                    error: Some(e.to_string()),
                }
            }
            Err(_) => {
                // Timeout - trigger rollback
                if self.rollback_enabled {
                    self.rollback(&experiment, &before_state).await?;
                }
                ExperimentOutcome {
                    success: false,
                    before_strength: before_state.map(|s| s.pathway_strength),
                    after_strength: None,
                    duration_ms: self.timeout.as_millis() as u64,
                    learned: None,
                    error: Some("Experiment timeout".to_string()),
                }
            }
        }
    }
}
```

---

## 7. Learning from Experiments

### 7.1 Outcome Recording

```sql
-- Record experiment outcomes for learning
CREATE TABLE pathway_experiments (
    id TEXT PRIMARY KEY,
    pathway_id TEXT NOT NULL,
    experiment_type TEXT NOT NULL,
    risk_score REAL NOT NULL,
    before_strength REAL,
    after_strength REAL,
    outcome TEXT NOT NULL CHECK(outcome IN ('success', 'failure', 'timeout', 'rollback')),
    duration_ms INTEGER NOT NULL,
    learned_insights TEXT,  -- JSON
    timestamp DATETIME DEFAULT CURRENT_TIMESTAMP,

    FOREIGN KEY (pathway_id) REFERENCES hebbian_pathways(pathway_id)
);

CREATE INDEX idx_experiments_pathway ON pathway_experiments(pathway_id);
CREATE INDEX idx_experiments_outcome ON pathway_experiments(outcome);
CREATE INDEX idx_experiments_date ON pathway_experiments(date(timestamp));
```

### 7.2 Learning Integration

```rust
/// Apply learnings from experiment to Hebbian pathways
pub async fn apply_experiment_learnings(
    db: &Pool<Sqlite>,
    outcome: &ExperimentOutcome,
    pathway_id: &str,
) -> Result<()> {
    // Calculate strength delta based on outcome
    let delta = match outcome.success {
        true => {
            // Successful experiment reduces uncertainty
            let uncertainty_reduction = 0.05;
            if outcome.after_strength.unwrap_or(0.5) > 0.5 {
                uncertainty_reduction  // Move toward 1.0
            } else {
                -uncertainty_reduction  // Move toward 0.0
            }
        }
        false => {
            // Failed experiment also provides information
            // Weaken pathway slightly (exploration revealed weakness)
            -0.02
        }
    };

    // Update pathway
    sqlx::query!(
        r#"
        UPDATE hebbian_pathways
        SET
            pathway_strength = MIN(1.0, MAX(0.01, pathway_strength + ?)),
            activation_count = activation_count + 1,
            last_activation = CURRENT_TIMESTAMP
        WHERE pathway_id = ?
        "#,
        delta,
        pathway_id
    )
    .execute(db)
    .await?;

    // Record learning event
    sqlx::query!(
        r#"
        INSERT INTO pulse_events (
            id, event_type, pathway_id, strength_delta, event_data
        ) VALUES (?, 'experiment_learning', ?, ?, ?)
        "#,
        Uuid::new_v4().to_string(),
        pathway_id,
        delta,
        serde_json::to_string(&outcome.learned_insights)?
    )
    .execute(db)
    .await?;

    Ok(())
}
```

---

## 8. Acceptance Criteria

- [ ] PL-EXPLORE-001 pipeline implemented
- [ ] Risk assessor gates all experiments
- [ ] Exploration budget enforced (100 experiments/day max)
- [ ] Learning outcomes recorded in hebbian_pulse.db
- [ ] Experiments during low-traffic windows only
- [ ] Rollback capability for failed experiments
- [ ] 5 experiment types operational

---

## 9. References

- **NAM Gap Analysis:** `NAM_GAP_ANALYSIS_REPORT.md`
- **Implementation Plan:** `PART_4_PHASE_5_NAM.md` (Section 5.9)
- **Hebbian Integration:** `nam/HEBBIAN_INTEGRATION.md`
- **Episodic Memory:** `nam/EPISODIC_MEMORY.md`

---

*Document generated for NAM Phase 5 compliance*
*Active Learning: Where the system seeks to understand, not just react*
