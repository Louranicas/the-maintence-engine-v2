# Heterogeneous Agent Role Taxonomy (NAM-05)

**Version:** 1.0.0
**Status:** Specification Complete
**NAM Compliance:** NAM-05 (Cognitive Diversity)
**Impact:** Richer consensus through perspective diversity

---

## 1. Overview

Current PBFT consensus treats all 40 agents as **identical**. Real distributed cognition benefits from specialization and cognitive diversity. This specification defines 5 distinct agent roles that provide different perspectives in consensus.

### 1.1 Philosophy

> "A swarm of identical agents reaches consensus quickly but may miss important considerations. A heterogeneous swarm takes longer but produces better decisions."

### 1.2 Key Principles

1. **Diversity over uniformity:** Different roles bring different perspectives
2. **Minority voice matters:** CRITIC and INTEGRATOR must approve for critical actions
3. **Weighted by competence:** Vote weights reflect role-specific expertise
4. **Learned specialization:** Roles emerge and strengthen through Hebbian learning

---

## 2. Role Distribution (40 Agents)

### 2.1 Role Summary Table

| Role | Count | Focus | Vote Weight | Bias | Description |
|------|-------|-------|-------------|------|-------------|
| VALIDATOR | 20 | Correctness | 1.0 | Conservative | Verify proposals meet requirements |
| EXPLORER | 8 | Alternatives | 0.8 | Novel solutions | Identify unconsidered alternatives |
| CRITIC | 6 | Flaws | 1.2 | Skeptical | Find weaknesses and edge cases |
| INTEGRATOR | 4 | Cross-system | 1.0 | Holistic | Assess systemic implications |
| HISTORIAN | 2 | Precedents | 0.8 | Past patterns | Match to historical outcomes |

### 2.2 Role Distribution YAML

```yaml
agent_roles:
  VALIDATOR:
    count: 20
    focus: correctness_verification
    vote_weight: 1.0
    bias: conservative
    description: |
      Validators ensure proposals meet all technical requirements,
      follow established patterns, and don't violate constraints.
      They form the majority and stabilize consensus.
    capabilities:
      - syntax_validation
      - constraint_checking
      - policy_compliance
      - threshold_verification

  EXPLORER:
    count: 8
    focus: alternative_detection
    vote_weight: 0.8
    bias: novel_solutions
    description: |
      Explorers actively seek alternative approaches that may not
      have been considered. They ask "What else could we do?"
    capabilities:
      - alternative_generation
      - creative_problem_solving
      - unconventional_patterns
      - experimentation_proposals

  CRITIC:
    count: 6
    focus: flaw_detection
    vote_weight: 1.2  # Extra weight for finding problems
    bias: skeptical
    description: |
      Critics actively seek problems, edge cases, and potential
      failures. They play devil's advocate to strengthen decisions.
    capabilities:
      - failure_mode_analysis
      - edge_case_detection
      - risk_identification
      - assumption_challenging

  INTEGRATOR:
    count: 4
    focus: cross_system_impact
    vote_weight: 1.0
    bias: holistic
    description: |
      Integrators assess how proposals affect other parts of the
      system. They see the big picture and identify ripple effects.
    capabilities:
      - dependency_analysis
      - cascade_prediction
      - synergy_assessment
      - cross_system_reasoning

  HISTORIAN:
    count: 2
    focus: precedent_matching
    vote_weight: 0.8
    bias: past_patterns
    description: |
      Historians remember past events and match current proposals
      to historical outcomes. They prevent repeating mistakes.
    capabilities:
      - pattern_recall
      - episode_matching
      - outcome_prediction
      - precedent_analysis
```

---

## 3. Vote Weights Per Role

### 3.1 Weight Calculation

```rust
/// Calculate effective vote weight for an agent
pub fn calculate_vote_weight(agent: &Agent, proposal: &Proposal) -> f64 {
    let base_weight = match agent.role {
        AgentRole::Validator => 1.0,
        AgentRole::Explorer => 0.8,
        AgentRole::Critic => 1.2,
        AgentRole::Integrator => 1.0,
        AgentRole::Historian => 0.8,
    };

    // Adjust for role relevance to proposal type
    let relevance_factor = calculate_role_relevance(agent.role, proposal.action_type);

    // Adjust for agent's Hebbian pathway strength in this domain
    let competence_factor = get_agent_competence(agent, proposal).await?;

    base_weight * relevance_factor * competence_factor
}

/// Calculate role relevance to proposal type
fn calculate_role_relevance(role: AgentRole, action_type: ActionType) -> f64 {
    match (role, action_type) {
        // Critics are more relevant for high-risk actions
        (AgentRole::Critic, ActionType::EmergencyKill) => 1.5,
        (AgentRole::Critic, ActionType::DatabaseMigration) => 1.3,

        // Integrators are more relevant for multi-service actions
        (AgentRole::Integrator, ActionType::MultiServiceRestart) => 1.4,
        (AgentRole::Integrator, ActionType::ConfigurationRollback) => 1.3,

        // Historians are more relevant for recurring issues
        (AgentRole::Historian, _) if is_recurring_pattern(&action_type) => 1.3,

        // Explorers are more relevant when standard approaches failed
        (AgentRole::Explorer, _) if previous_attempts_failed() => 1.4,

        // Default: neutral relevance
        _ => 1.0,
    }
}
```

### 3.2 Weighted Vote Summary

| Role | Base Weight | Typical Effective Range | Notes |
|------|-------------|------------------------|-------|
| VALIDATOR | 1.0 | 0.8 - 1.2 | Stable baseline |
| EXPLORER | 0.8 | 0.6 - 1.1 | Boosted when standard fails |
| CRITIC | 1.2 | 1.0 - 1.8 | Boosted for high-risk |
| INTEGRATOR | 1.0 | 0.9 - 1.4 | Boosted for multi-system |
| HISTORIAN | 0.8 | 0.7 - 1.0 | Boosted for patterns |

---

## 4. Enhanced Consensus Requirements

### 4.1 Standard PBFT vs NAM-Enhanced

```yaml
# Standard PBFT: 27/40 quorum (2f + 1)
standard_pbft:
  total_agents: 40
  byzantine_tolerance: 13
  quorum_required: 27

# NAM-Enhanced PBFT: Additional role requirements
nam_enhanced_consensus:
  standard_quorum: 27  # Still need 2f + 1 total votes

  # Minority approvals required
  minority_approvals:
    - role: CRITIC
      minimum: 1
      reason: "Ensure potential problems are considered"
      bypass: false  # Cannot bypass this requirement

    - role: INTEGRATOR
      minimum: 1
      reason: "Ensure systemic thinking occurs"
      bypass: false

  # Role-specific thresholds for certain actions
  action_specific:
    - action: database_migration
      additional_requirements:
        - role: HISTORIAN
          minimum: 1
          reason: "Check for migration precedents"

    - action: emergency_kill
      additional_requirements:
        - role: CRITIC
          minimum: 2
          reason: "Extra scrutiny for emergency actions"
```

### 4.2 Consensus Check Implementation

```rust
/// Check if consensus requirements are met
pub struct EnhancedConsensusChecker {
    standard_quorum: u32,
    minority_requirements: Vec<MinorityRequirement>,
}

impl EnhancedConsensusChecker {
    pub fn check_consensus(&self, votes: &[Vote], action: &Action) -> ConsensusResult {
        // Count total approvals
        let total_approvals: f64 = votes.iter()
            .filter(|v| v.vote == VoteType::Approve)
            .map(|v| v.effective_weight)
            .sum();

        // Check standard quorum
        if total_approvals < self.standard_quorum as f64 {
            return ConsensusResult::QuorumNotReached {
                required: self.standard_quorum,
                received: total_approvals as u32,
            };
        }

        // Check minority role requirements
        for requirement in &self.minority_requirements {
            let role_approvals = votes.iter()
                .filter(|v| v.vote == VoteType::Approve)
                .filter(|v| v.agent_role == requirement.role)
                .count();

            if role_approvals < requirement.minimum as usize {
                return ConsensusResult::MinorityNotMet {
                    role: requirement.role.clone(),
                    required: requirement.minimum,
                    received: role_approvals as u32,
                    reason: requirement.reason.clone(),
                };
            }
        }

        // Check action-specific requirements
        if let Some(action_reqs) = self.get_action_requirements(action) {
            for req in action_reqs {
                let role_approvals = votes.iter()
                    .filter(|v| v.vote == VoteType::Approve)
                    .filter(|v| v.agent_role == req.role)
                    .count();

                if role_approvals < req.minimum as usize {
                    return ConsensusResult::ActionRequirementNotMet {
                        action: action.action_type.clone(),
                        role: req.role.clone(),
                        required: req.minimum,
                        received: role_approvals as u32,
                    };
                }
            }
        }

        ConsensusResult::Reached {
            total_votes: total_approvals as u32,
            role_breakdown: self.calculate_role_breakdown(votes),
        }
    }
}
```

---

## 5. Role Assignment

### 5.1 Database Schema

```sql
-- Agent registry with role information
CREATE TABLE agent_registry (
    agent_id TEXT PRIMARY KEY,
    agent_type TEXT NOT NULL,  -- 'cva_nam', 'human'
    role TEXT NOT NULL CHECK(role IN (
        'VALIDATOR', 'EXPLORER', 'CRITIC', 'INTEGRATOR', 'HISTORIAN'
    )),
    vote_weight REAL NOT NULL DEFAULT 1.0,
    tier INTEGER NOT NULL CHECK(tier BETWEEN 0 AND 6),
    specializations TEXT,  -- JSON array of domain specializations
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    last_active DATETIME
);

-- Role performance tracking
CREATE TABLE role_performance (
    agent_id TEXT NOT NULL,
    role TEXT NOT NULL,
    action_type TEXT NOT NULL,
    vote_cast TEXT NOT NULL,
    outcome_matched BOOLEAN,  -- Did the outcome match their vote?
    timestamp DATETIME DEFAULT CURRENT_TIMESTAMP,

    FOREIGN KEY (agent_id) REFERENCES agent_registry(agent_id)
);

CREATE INDEX idx_role_performance_agent ON role_performance(agent_id);
CREATE INDEX idx_role_performance_role ON role_performance(role);
```

### 5.2 Initial Role Assignment

```sql
-- Register agents with roles across tiers
-- Tier 1: Foundation (5 agents)
INSERT INTO agent_registry (agent_id, agent_type, role, vote_weight, tier) VALUES
    ('@1.01', 'cva_nam', 'VALIDATOR', 1.0, 1),
    ('@1.02', 'cva_nam', 'VALIDATOR', 1.0, 1),
    ('@1.03', 'cva_nam', 'VALIDATOR', 1.0, 1),
    ('@1.04', 'cva_nam', 'INTEGRATOR', 1.0, 1),
    ('@1.05', 'cva_nam', 'CRITIC', 1.2, 1);

-- Tier 2: Core (8 agents)
INSERT INTO agent_registry (agent_id, agent_type, role, vote_weight, tier) VALUES
    ('@2.06', 'cva_nam', 'VALIDATOR', 1.0, 2),
    ('@2.07', 'cva_nam', 'VALIDATOR', 1.0, 2),
    ('@2.08', 'cva_nam', 'VALIDATOR', 1.0, 2),
    ('@2.09', 'cva_nam', 'VALIDATOR', 1.0, 2),
    ('@2.10', 'cva_nam', 'EXPLORER', 0.8, 2),
    ('@2.11', 'cva_nam', 'EXPLORER', 0.8, 2),
    ('@2.12', 'cva_nam', 'CRITIC', 1.2, 2),
    ('@2.13', 'cva_nam', 'HISTORIAN', 0.8, 2);

-- Tier 3: Testing (8 agents)
INSERT INTO agent_registry (agent_id, agent_type, role, vote_weight, tier) VALUES
    ('@3.14', 'cva_nam', 'VALIDATOR', 1.0, 3),
    ('@3.15', 'cva_nam', 'VALIDATOR', 1.0, 3),
    ('@3.16', 'cva_nam', 'VALIDATOR', 1.0, 3),
    ('@3.17', 'cva_nam', 'VALIDATOR', 1.0, 3),
    ('@3.18', 'cva_nam', 'EXPLORER', 0.8, 3),
    ('@3.19', 'cva_nam', 'EXPLORER', 0.8, 3),
    ('@3.20', 'cva_nam', 'CRITIC', 1.2, 3),
    ('@3.21', 'cva_nam', 'INTEGRATOR', 1.0, 3);

-- Tier 4: Optimization (5 agents)
INSERT INTO agent_registry (agent_id, agent_type, role, vote_weight, tier) VALUES
    ('@4.22', 'cva_nam', 'VALIDATOR', 1.0, 4),
    ('@4.23', 'cva_nam', 'VALIDATOR', 1.0, 4),
    ('@4.24', 'cva_nam', 'EXPLORER', 0.8, 4),
    ('@4.25', 'cva_nam', 'CRITIC', 1.2, 4),
    ('@4.26', 'cva_nam', 'INTEGRATOR', 1.0, 4);

-- Tier 5: Integration (6 agents)
INSERT INTO agent_registry (agent_id, agent_type, role, vote_weight, tier) VALUES
    ('@5.27', 'cva_nam', 'VALIDATOR', 1.0, 5),
    ('@5.28', 'cva_nam', 'VALIDATOR', 1.0, 5),
    ('@5.29', 'cva_nam', 'VALIDATOR', 1.0, 5),
    ('@5.30', 'cva_nam', 'EXPLORER', 0.8, 5),
    ('@5.31', 'cva_nam', 'EXPLORER', 0.8, 5),
    ('@5.32', 'cva_nam', 'INTEGRATOR', 1.0, 5);

-- Tier 6: Hardening (8 agents)
INSERT INTO agent_registry (agent_id, agent_type, role, vote_weight, tier) VALUES
    ('@6.33', 'cva_nam', 'VALIDATOR', 1.0, 6),
    ('@6.34', 'cva_nam', 'VALIDATOR', 1.0, 6),
    ('@6.35', 'cva_nam', 'VALIDATOR', 1.0, 6),
    ('@6.36', 'cva_nam', 'VALIDATOR', 1.0, 6),
    ('@6.37', 'cva_nam', 'VALIDATOR', 1.0, 6),
    ('@6.38', 'cva_nam', 'VALIDATOR', 1.0, 6),
    ('@6.39', 'cva_nam', 'CRITIC', 1.2, 6),
    ('@6.40', 'cva_nam', 'HISTORIAN', 0.8, 6);

-- Human agent
INSERT INTO agent_registry (agent_id, agent_type, role, vote_weight, tier) VALUES
    ('@0.A', 'human', 'INTEGRATOR', 1.0, 0);
```

---

## 6. Role Behavior Implementation

### 6.1 Role-Specific Voting Logic

```rust
/// Role-specific voting behavior
pub trait RoleBehavior {
    fn evaluate(&self, proposal: &Proposal, context: &Context) -> VoteDecision;
}

impl RoleBehavior for ValidatorRole {
    fn evaluate(&self, proposal: &Proposal, context: &Context) -> VoteDecision {
        // Validators check technical correctness
        let validations = vec![
            self.check_syntax_valid(proposal),
            self.check_constraints_satisfied(proposal),
            self.check_policy_compliance(proposal),
            self.check_thresholds_valid(proposal),
        ];

        if validations.iter().all(|v| v.passed) {
            VoteDecision::Approve { confidence: 0.9 }
        } else {
            VoteDecision::Reject {
                reason: validations.iter()
                    .filter(|v| !v.passed)
                    .map(|v| v.reason.clone())
                    .collect(),
            }
        }
    }
}

impl RoleBehavior for CriticRole {
    fn evaluate(&self, proposal: &Proposal, context: &Context) -> VoteDecision {
        // Critics actively look for problems
        let issues = vec![
            self.find_failure_modes(proposal),
            self.find_edge_cases(proposal),
            self.identify_risks(proposal),
            self.challenge_assumptions(proposal),
        ];

        let serious_issues: Vec<_> = issues.iter()
            .flatten()
            .filter(|i| i.severity >= Severity::Medium)
            .collect();

        if serious_issues.is_empty() {
            VoteDecision::Approve { confidence: 0.7 }  // Still cautious
        } else {
            VoteDecision::Reject {
                reason: serious_issues.iter()
                    .map(|i| i.description.clone())
                    .collect(),
            }
        }
    }
}

impl RoleBehavior for ExplorerRole {
    fn evaluate(&self, proposal: &Proposal, context: &Context) -> VoteDecision {
        // Explorers consider alternatives
        let alternatives = self.generate_alternatives(proposal, context);

        if alternatives.is_empty() || self.is_best_option(proposal, &alternatives) {
            VoteDecision::Approve { confidence: 0.8 }
        } else {
            VoteDecision::ApproveWithNote {
                confidence: 0.6,
                note: format!(
                    "Alternative considered: {}",
                    alternatives.first().unwrap().description
                ),
            }
        }
    }
}

impl RoleBehavior for IntegratorRole {
    fn evaluate(&self, proposal: &Proposal, context: &Context) -> VoteDecision {
        // Integrators assess cross-system impact
        let impacts = self.analyze_cross_system_impact(proposal);
        let cascade_risk = self.predict_cascade_effects(proposal);

        if cascade_risk > 0.5 || impacts.iter().any(|i| i.severity >= Severity::High) {
            VoteDecision::Reject {
                reason: vec![format!(
                    "Cross-system risk too high: cascade_risk={:.2}, impacts={:?}",
                    cascade_risk, impacts
                )],
            }
        } else {
            VoteDecision::Approve { confidence: 0.85 }
        }
    }
}

impl RoleBehavior for HistorianRole {
    fn evaluate(&self, proposal: &Proposal, context: &Context) -> VoteDecision {
        // Historians match to past events
        let similar_episodes = self.recall_similar_episodes(proposal);

        if let Some(bad_episode) = similar_episodes.iter().find(|e| e.outcome == Outcome::Failure) {
            VoteDecision::Reject {
                reason: vec![format!(
                    "Similar action failed on {}: {}",
                    bad_episode.timestamp, bad_episode.failure_reason
                )],
            }
        } else if similar_episodes.iter().all(|e| e.outcome == Outcome::Success) {
            VoteDecision::Approve {
                confidence: 0.9,
            }
        } else {
            VoteDecision::Approve { confidence: 0.7 }
        }
    }
}
```

---

## 7. Acceptance Criteria

- [ ] 5 agent roles defined with distinct biases
- [ ] Vote weights adjusted per role (CRITIC at 1.2x)
- [ ] CRITIC and INTEGRATOR approval required for critical actions
- [ ] Consensus queries respect role distribution
- [ ] Minority perspectives preserved in decision logs
- [ ] Role performance tracking implemented

---

## 8. References

- **PBFT Consensus:** `nam/PBFT_CONSENSUS.md`
- **Human as Agent:** `nam/HUMAN_AS_AGENT.md`
- **Dissent Generation:** `nam/DISSENT_GENERATION.md`
- **NAM Gap Analysis:** `NAM_GAP_ANALYSIS_REPORT.md`

---

*Document generated for NAM Phase 5 compliance*
*Agent Roles: Where cognitive diversity produces better decisions*
