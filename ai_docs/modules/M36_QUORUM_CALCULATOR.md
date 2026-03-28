# Module M36: Quorum Calculator

> **M36_QUORUM_CALCULATOR** | Quorum management | Layer: L6 Consensus | [Back to Index](INDEX.md)

---

## Navigation

| Direction | Link |
|-----------|------|
| Up | [INDEX.md](INDEX.md) |
| Layer | [L06_CONSENSUS.md](../layers/L06_CONSENSUS.md) |
| Related | [M31_PBFT_MANAGER.md](M31_PBFT_MANAGER.md) |
| Related | [M32_AGENT_COORDINATOR.md](M32_AGENT_COORDINATOR.md) |
| Related | [M33_VOTE_COLLECTOR.md](M33_VOTE_COLLECTOR.md) |
| Related | [M34_VIEW_CHANGE_HANDLER.md](M34_VIEW_CHANGE_HANDLER.md) |
| Related | [M35_DISSENT_TRACKER.md](M35_DISSENT_TRACKER.md) |

---

## Module Specification

### Overview

The Quorum Calculator determines whether consensus requirements have been met for PBFT proposal acceptance. It evaluates vote totals against Byzantine quorum thresholds (27/40 agents), verifies weighted vote aggregations, and confirms enhanced consensus requirements (CRITIC and INTEGRATOR approval). Operating as the final validation gate before proposal execution, it prevents inadequate consensus from proceeding.

### Module Properties

| Property | Value |
|----------|-------|
| Module ID | M36 |
| Module Name | Quorum Calculator |
| Layer | L6 (Consensus) |
| Version | 1.0.0 |
| Dependencies | M33 (Vote Collector), M31 (PBFT Manager) |
| Dependents | M31 (Final decision), M34 (View change triggers) |

---

## PBFT Quorum Configuration

### Byzantine Fault Tolerance Math

The PBFT quorum is calculated from Byzantine Fault Tolerance requirements:

```
Total agents: n = 40
Byzantine tolerance: f = ⌊(n-1)/3⌋ = 13
Quorum requirement: q = 2f + 1 = 27
```

**Formula Verification:**
- n = 3f + 1 (Byzantine consensus requirement)
- 40 = 3(13) + 1 ✓
- q = 2f + 1 (minimum for deciding without faulty nodes)
- 27 = 2(13) + 1 ✓

### Security Properties

With n=40 and f=13:
- System tolerates up to 13 Byzantine (malicious) agents
- Any group of 27 agents includes at least 14 honest agents
- No subset of 13 agents can block consensus (27 > 3×13)
- No subset of 13 agents can forge false consensus

---

## Quorum Thresholds

### Simple Quorum

Basic quorum check for proposal acceptance:

| Requirement | Threshold | Description |
|-------------|-----------|-------------|
| Approval votes | votes_for ≥ 27 | At least 27 agents approve |
| Total votes | total_votes ≥ 27 | At least 27 agents participated |
| Both conditions | AND | Both must be true |

**Quorum Reached:** votes_for ≥ 27 AND total_votes ≥ 27

### Weighted Quorum

After role-based vote weighting (M33):

| Role | Weight | Contribution |
|------|--------|--------------|
| VALIDATOR | 1.0 | 1.0 per vote |
| EXPLORER | 0.8 | 0.8 per vote |
| CRITIC | 1.2 | 1.2 per vote |
| INTEGRATOR | 1.0 | 1.0 per vote |
| HISTORIAN | 0.8 | 0.8 per vote |

**Weighted Quorum:** weighted_for ≥ minimum_weighted_threshold

**Minimum Weighted Threshold Calculation:**
```
27 agents × 1.0 average weight = 27.0 minimum weighted votes
(Accounting for role distribution: 21 VAL, 8 EXP, 6 CRI, 4 INT, 2 HIS)
Weighted sum: 21×1.0 + 8×0.8 + 6×1.2 + 4×1.0 + 2×0.8 = 39.6 total capacity
Quorum: weighted_for ≥ 27.0
```

### Enhanced Quorum (NAM-05)

Critical actions require additional validation:

| Requirement | Description |
|-------------|-------------|
| At least 1 CRITIC approval | Ensures flaw detection |
| At least 1 INTEGRATOR approval | Ensures impact analysis |

Both conditions must be true for critical actions:
- ServiceTermination
- DatabaseMigration
- CascadeRestart

---

## Core Quorum Functions

### is_quorum_reached

```rust
pub fn is_quorum_reached(votes_for: u32, total_votes: u32) -> bool {
    votes_for >= PBFT_Q && total_votes >= PBFT_Q
}
```

**Purpose:** Determine if basic quorum threshold met

**Parameters:**
- `votes_for`: Number of approval votes received
- `total_votes`: Total votes received (approve + reject + abstain)

**Returns:** `bool` - true if quorum met

**Logic:**
1. Check votes_for >= 27
2. Check total_votes >= 27
3. Return true only if BOTH conditions met

**Examples:**

| votes_for | total_votes | Result | Reason |
|-----------|-------------|--------|--------|
| 27 | 40 | TRUE | Meets both thresholds |
| 30 | 40 | TRUE | Exceeds both thresholds |
| 26 | 40 | FALSE | Approval < 27 |
| 27 | 26 | FALSE | Total < 27 |
| 27 | 27 | TRUE | Minimum quorum met |

### enhanced_consensus_check

```rust
pub fn enhanced_consensus_check(votes: &[ConsensusVote]) -> bool {
    let critic_approval = votes.iter().any(|v| {
        v.role == AgentRole::Critic && v.vote == VoteType::Approve
    });

    let integrator_approval = votes.iter().any(|v| {
        v.role == AgentRole::Integrator && v.vote == VoteType::Approve
    });

    critic_approval && integrator_approval
}
```

**Purpose:** Verify NAM-05 enhanced requirements (CRITIC + INTEGRATOR)

**Parameters:**
- `votes`: Slice of ConsensusVote structures

**Returns:** `bool` - true if both requirements met

**Requirements:**
1. At least 1 CRITIC must have voted Approve
2. At least 1 INTEGRATOR must have voted Approve

**Examples:**

| CRITICs | INTEGRATORs | Result | Reason |
|---------|-------------|--------|--------|
| 1+ Approve | 1+ Approve | TRUE | Both roles approve |
| 0 Approve | 1+ Approve | FALSE | No CRITIC approval |
| 1+ Approve | 0 Approve | FALSE | No INTEGRATOR approval |
| All Reject | 1+ Approve | FALSE | CRITIC consensus is reject |
| 1+ Approve | All Abstain | FALSE | INTEGRATOR no approval |

---

## Quorum Evaluation Workflow

```
1. VOTE AGGREGATION (M33)
   ├─> All votes collected
   ├─> Simple vote counts: for, against, abstained
   ├─> Weighted vote sums: weighted_for, weighted_against, weighted_abstain
   └─> Results passed to M36

2. SIMPLE QUORUM CHECK
   ├─> Call is_quorum_reached(votes_for, total_votes)
   ├─> If votes_for >= 27 AND total_votes >= 27:
   │   └─> Simple quorum = TRUE
   └─> Else:
       └─> Simple quorum = FALSE → PROPOSAL FAILS

3. ROLE-BASED DISTRIBUTION CHECK
   ├─> Verify at least one agent from key roles
   ├─> At least 1 VALIDATOR: yes (20 available)
   ├─> At least 1 CRITIC: yes (6 available)
   ├─> At least 1 INTEGRATOR: yes (4 available)
   └─> Continue if roles represented

4. ENHANCED CONSENSUS CHECK (Critical Actions Only)
   ├─> Call enhanced_consensus_check(votes)
   ├─> If action is: ServiceTermination, DatabaseMigration, CascadeRestart
   │   ├─> CRITIC approval required: check votes
   │   ├─> INTEGRATOR approval required: check votes
   │   └─> If either missing: PROPOSAL FAILS → TRIGGER VIEW CHANGE
   └─> Else: Skip enhanced check

5. WEIGHTED VOTE ANALYSIS
   ├─> Minimum weighted threshold: 27.0
   ├─> Calculate total weighted approvals
   ├─> If weighted_for >= 27.0:
   │   └─> Weighted quorum = TRUE
   └─> Else:
       └─> Weighted quorum = FALSE (minor dissenters)

6. FINAL DECISION
   ├─> All checks passed?
   │   ├─> Simple quorum: YES
   │   ├─> Enhanced (if applicable): YES
   │   ├─> Role distribution: YES
   │   └─> CONSENSUS = APPROVED
   └─> Any check failed?
       ├─> CONSENSUS = FAILED
       ├─> Trigger view change (M34)
       └─> Log to dissent tracker (M35)

7. EXECUTION DECISION
   ├─> If APPROVED:
   │   ├─> Set phase = Execute
   │   ├─> Action proceeds to execution
   │   └─> Success probability: high
   └─> If FAILED:
       ├─> Set phase = Failed
       ├─> Trigger view change
       └─> New primary will retry
```

---

## Quorum Scenarios

### Scenario 1: Standard Approval

```
Proposal: ConfigRollback
Votes: 35 Approve, 3 Reject, 2 Abstain

Simple Quorum Check:
  votes_for = 35 (>= 27 ✓)
  total_votes = 40 (>= 27 ✓)
  → QUORUM REACHED

Enhanced Consensus (not critical):
  → SKIPPED (ConfigRollback not critical)

Result: PROPOSAL APPROVED
Phase: Execute
```

### Scenario 2: Minimal Quorum

```
Proposal: CredentialRotation
Votes: 27 Approve, 13 Reject

Simple Quorum Check:
  votes_for = 27 (>= 27 ✓)
  total_votes = 40 (>= 27 ✓)
  → QUORUM REACHED

Enhanced Consensus (not critical):
  → SKIPPED

Result: PROPOSAL APPROVED (barely)
Phase: Execute
Note: Only 2 votes more and proposal would fail
```

### Scenario 3: Quorum Failure - Insufficient Approvals

```
Proposal: ServiceTermination (CRITICAL)
Votes: 20 Approve, 15 Reject, 5 Abstain

Simple Quorum Check:
  votes_for = 20 (< 27 ✗)
  total_votes = 40 (>= 27 ✓)
  → QUORUM FAILED

Result: PROPOSAL REJECTED
Phase: Failed
Action: Trigger view change (M34)
Reason: Insufficient approve votes (20/27)
```

### Scenario 4: Quorum Failure - Enhanced Consensus Missing

```
Proposal: DatabaseMigration (CRITICAL)
Votes: 30 Approve (all from VALIDATORS, EXPLORERS, HISTORIANS)
       8 Reject (all from CRITICS, INTEGRATORS)
       2 Abstain

Simple Quorum Check:
  votes_for = 30 (>= 27 ✓)
  total_votes = 40 (>= 27 ✓)
  → SIMPLE QUORUM REACHED

Enhanced Consensus Check:
  CRITIC approvals: 0 (all CRITICs voted Reject)
  INTEGRATOR approvals: 0 (all INTEGRATORs voted Reject)
  → ENHANCED QUORUM FAILED

Result: PROPOSAL REJECTED (despite simple majority)
Phase: Failed
Action: Trigger view change (M34)
Reason: Enhanced consensus requirements not met
Dissent: Record CRITIC and INTEGRATOR concerns
```

### Scenario 5: Weighted Quorum with Mixed Roles

```
Proposal: CascadeRestart (CRITICAL)

Votes:
  21 VALIDATORS approve     = 21 × 1.0 = 21.0
   5 EXPLORERS approve      = 5 × 0.8 = 4.0
   1 CRITIC approves        = 1 × 1.2 = 1.2 ✓ (enhanced requirement)
   1 INTEGRATOR approves    = 1 × 1.0 = 1.0 ✓ (enhanced requirement)
   6 CRITICS reject         = 6 × 1.2 = 7.2 (dissent tracked)

Total weighted approvals: 21.0 + 4.0 + 1.2 + 1.0 = 27.2
Simple votes: 28 Approve out of 40

Quorum Check:
  Simple: 28 >= 27 ✓
  Total: 40 >= 27 ✓
  Enhanced: CRITIC ✓, INTEGRATOR ✓
  Weighted: 27.2 >= 27.0 ✓

Result: PROPOSAL APPROVED
Phase: Execute
Dissent: 6 Critics dissenting recorded by M35
```

---

## Vote Count Formulas

### Simple Vote Count

```
votes_for = COUNT(vote == Approve)
votes_against = COUNT(vote == Reject)
votes_abstained = COUNT(vote == Abstain)
total_votes = votes_for + votes_against + votes_abstained
```

### Weighted Vote Count

```
weighted_for = SUM(weight WHERE vote == Approve)
weighted_against = SUM(weight WHERE vote == Reject)
weighted_abstained = SUM(weight WHERE vote == Abstain)
```

### Quorum Evaluation

```
simple_quorum = (votes_for >= 27) AND (total_votes >= 27)
weighted_quorum = (weighted_for >= 27.0)
enhanced_quorum = (critic_approvals > 0) AND (integrator_approvals > 0)

consensus_approved = simple_quorum AND weighted_quorum AND
                     (enhanced_quorum OR action_not_critical)
```

---

## Critical Actions Requiring Enhanced Consensus

These actions always require CRITIC and INTEGRATOR approval:

| Action | Timeout | Risk Level | Rationale |
|--------|---------|-----------|-----------|
| ServiceTermination | 60s | CRITICAL | Immediate service loss |
| DatabaseMigration | 300s | CRITICAL | Data integrity risk |
| CascadeRestart | 180s | CRITICAL | System-wide impact |
| CredentialRotation | 120s | HIGH | Security implications |
| ConfigRollback | 90s | MEDIUM | System state change |

**Enhanced Requirement:** ServiceTermination, DatabaseMigration, CascadeRestart

---

## Performance Characteristics

| Operation | Time | Notes |
|-----------|------|-------|
| Simple quorum check | <1ms | Two integer comparisons |
| Weighted calculation | <10ms | Linear in agent count |
| Enhanced check | <5ms | Hash table lookups |
| Full validation | <20ms | All checks combined |

---

## Related Modules

| Module | Relationship | Purpose |
|--------|--------------|---------|
| M31 | Dependent | Proposal decision point |
| M33 | Dependency | Vote aggregation source |
| M34 | Dependent | View change trigger |
| M35 | Dependent | Dissent tracking rationale |

---

## Testing

Key test cases:

```rust
#[test]
fn test_pbft_constants()            // Verify n=40, f=13, q=27
#[test]
fn test_quorum_reached()            // Test threshold logic
#[test]
fn test_minimal_quorum()            // Test 27/40 boundary
#[test]
fn test_insufficient_approvals()    // Test < 27 failure
#[test]
fn test_insufficient_total_votes()  // Test total < 27 failure
#[test]
fn test_enhanced_consensus()        // Test CRITIC/INTEGRATOR requirements
#[test]
fn test_enhanced_failure()          // Test missing role failure
#[test]
fn test_weighted_quorum()           // Test role-based weighting
```

---

## Database Integration

### Quorum Decision Logging

```sql
CREATE TABLE consensus_decisions (
    proposal_id TEXT PRIMARY KEY,
    votes_for INTEGER,
    votes_against INTEGER,
    votes_abstained INTEGER,
    weighted_for REAL,
    quorum_reached BOOLEAN,
    enhanced_passed BOOLEAN,
    decision TEXT,  -- APPROVED or REJECTED
    timestamp DATETIME
);

-- Query to find failed quorum decisions
SELECT proposal_id, votes_for, votes_against,
       weighted_for, decision
FROM consensus_decisions
WHERE quorum_reached = false
ORDER BY timestamp DESC;
```

---

## Version History

| Version | Date | Changes |
|---------|------|---------|
| 1.0.0 | 2026-01-28 | Initial implementation |

---

*The Maintenance Engine v1.0.0 | M36: Quorum Calculator*
*Last Updated: 2026-01-28*
