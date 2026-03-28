# Module M33: Vote Collector

> **M33_VOTE_COLLECTOR** | Vote aggregation | Layer: L6 Consensus | [Back to Index](INDEX.md)

---

## Navigation

| Direction | Link |
|-----------|------|
| Up | [INDEX.md](INDEX.md) |
| Layer | [L06_CONSENSUS.md](../layers/L06_CONSENSUS.md) |
| Related | [M31_PBFT_MANAGER.md](M31_PBFT_MANAGER.md) |
| Related | [M32_AGENT_COORDINATOR.md](M32_AGENT_COORDINATOR.md) |
| Related | [M34_VIEW_CHANGE_HANDLER.md](M34_VIEW_CHANGE_HANDLER.md) |
| Related | [M35_DISSENT_TRACKER.md](M35_DISSENT_TRACKER.md) |
| Related | [M36_QUORUM_CALCULATOR.md](M36_QUORUM_CALCULATOR.md) |

---

## Module Specification

### Overview

The Vote Collector aggregates individual agent votes into consensus outcomes. It manages vote submission, validates voting credentials, calculates weighted vote totals, and triggers quorum verification. Operating across PBFT Prepare and Commit phases, it ensures accurate vote tabulation and enables dissent tracking for learning.

### Module Properties

| Property | Value |
|----------|-------|
| Module ID | M33 |
| Module Name | Vote Collector |
| Layer | L6 (Consensus) |
| Version | 1.0.0 |
| Dependencies | M32 (Agent Coordinator), M31 (PBFT Manager) |
| Dependents | M34 (View Change Handler), M35 (Dissent Tracker), M36 (Quorum Calculator) |

---

## PBFT Configuration

| Parameter | Value | Description |
|-----------|-------|-------------|
| Total Agents | 40 | CVA-NAM fleet size |
| Quorum Requirement | 27 | 2f+1 threshold for consensus |
| Byzantine Tolerance | 13 | Maximum faulty agents (f) |

---

## Core Types

### ConsensusVote Structure

```rust
pub struct ConsensusVote {
    /// Unique identifier of the proposal being voted on
    pub proposal_id: String,
    /// ID of the agent submitting this vote
    pub agent_id: String,
    /// The vote itself (Approve/Reject/Abstain)
    pub vote: VoteType,
    /// Consensus phase when vote was cast (Prepare/Commit)
    pub phase: ConsensusPhase,
    /// Agent's role in consensus (VALIDATOR, EXPLORER, etc.)
    pub role: AgentRole,
    /// Vote weight multiplier (1.0 standard, 1.2 for CRITIC, etc.)
    pub weight: f64,
    /// Optional reasoning for the vote
    pub reason: Option<String>,
    /// Timestamp when vote was submitted
    pub timestamp: std::time::SystemTime,
}
```

### VoteType Enumeration

```rust
pub enum VoteType {
    /// Vote to approve the proposal
    Approve,
    /// Vote to reject the proposal
    Reject,
    /// Vote to abstain from decision
    Abstain,
}
```

### Vote Metadata

Each vote includes:

| Field | Type | Purpose |
|-------|------|---------|
| proposal_id | String | Links vote to specific proposal |
| agent_id | String | Identifies voting agent |
| vote | VoteType | Approve/Reject/Abstain |
| phase | ConsensusPhase | Prepare or Commit phase |
| role | AgentRole | VALIDATOR, EXPLORER, CRITIC, INTEGRATOR, HISTORIAN |
| weight | f64 | Role-based vote weight (0.8-1.2) |
| reason | Option<String> | Optional vote justification |
| timestamp | SystemTime | Vote submission time |

---

## Vote Weights (NAM-05)

Each agent role has a vote weight multiplier:

| Role | Weight | Multiplier | Effect |
|------|--------|-----------|--------|
| VALIDATOR | 1.0 | 1.0x | Standard weight |
| EXPLORER | 0.8 | 0.8x | Reduced weight |
| CRITIC | 1.2 | 1.2x | Enhanced weight (flaw detection) |
| INTEGRATOR | 1.0 | 1.0x | Standard weight |
| HISTORIAN | 0.8 | 0.8x | Reduced weight |

**Weighted Vote Calculation Example:**
- 27 VALIDATOR approvals = 27.0 weighted votes
- 6 CRITIC approvals = 6 × 1.2 = 7.2 weighted votes
- 8 EXPLORER approvals = 8 × 0.8 = 6.4 weighted votes
- **Total weighted approval = 40.6**

---

## API Functions

### Vote Aggregation Functions

#### calculate_weighted_votes

```rust
pub fn calculate_weighted_votes(votes: &[ConsensusVote]) -> (f64, f64, f64)
```

Aggregates votes accounting for agent role weights.

**Parameters:**
- `votes`: Slice of ConsensusVote structures to aggregate

**Returns:** Tuple of three f64 values:
- First element: Sum of weights for Approve votes
- Second element: Sum of weights for Reject votes
- Third element: Sum of weights for Abstain votes

**Algorithm:**
```rust
for vote in votes {
    match vote.vote {
        VoteType::Approve => for_weight += vote.weight,
        VoteType::Reject => against_weight += vote.weight,
        VoteType::Abstain => abstain_weight += vote.weight,
    }
}
(for_weight, against_weight, abstain_weight)
```

**Example:**
```
Input votes:
  - @0.A (VALIDATOR, weight=1.0): Approve
  - agent-29 (CRITIC, weight=1.2): Approve
  - agent-21 (EXPLORER, weight=0.8): Reject

Output: (2.0, 0.8, 0.0)
```

#### enhanced_consensus_check

```rust
pub fn enhanced_consensus_check(votes: &[ConsensusVote]) -> bool
```

Verifies NAM-05 enhanced consensus requirements for critical actions.

**Requirements:**
- At least 1 CRITIC approval present (flaw detection)
- At least 1 INTEGRATOR approval present (impact assessment)

**Parameters:**
- `votes`: Slice of ConsensusVote structures

**Returns:** `bool` - true if both CRITIC and INTEGRATOR approval found

**Implementation:**
```rust
let critic_approval = votes.iter().any(|v| {
    v.role == AgentRole::Critic && v.vote == VoteType::Approve
});

let integrator_approval = votes.iter().any(|v| {
    v.role == AgentRole::Integrator && v.vote == VoteType::Approve
});

critic_approval && integrator_approval
```

---

## Vote Collection Phases

### Prepare Phase

The first phase of vote collection:

**Phase Name:** Prepare

**Purpose:** Collect initial votes on proposal validity

**Participation:**
- All 40 agents should participate
- Human @0.A included
- Agent votes unweighted initially

**Timeout:** Action-dependent (5-300 seconds)

**Outcome:** Determination of quorum

### Commit Phase

The second phase of vote collection:

**Phase Name:** Commit

**Purpose:** Commit agents to execution decision

**Participation:**
- Agents from Prepare phase continue
- Opportunity to change votes if new info emerges
- Role-based weighting applied

**Timeout:** Action-dependent (5-300 seconds)

**Outcome:** Final consensus decision

---

## Vote Collection Workflow

```
1. PREPARE PHASE INITIATION
   ├─> Proposal broadcast to all agents (M32)
   ├─> Vote collection initialized
   ├─> Phase set to ConsensusPhase::Prepare
   └─> Timeout started

2. AGENT EVALUATION
   ├─> Each agent examines proposal
   ├─> Validates against local state
   ├─> Considers action type and impact
   ├─> Applies role-specific logic
   └─> Prepares vote

3. VOTE SUBMISSION
   ├─> Agent submits ConsensusVote
   ├─> Includes: proposal_id, agent_id, vote, phase, role, weight
   ├─> Optional reason field populated if available
   ├─> Timestamp recorded
   └─> M33 receives and stores vote

4. VOTE VALIDATION
   ├─> Verify proposal_id matches current
   ├─> Verify agent_id is valid agent
   ├─> Verify vote is valid (Approve/Reject/Abstain)
   ├─> Verify role matches agent's assigned role
   ├─> Verify weight matches role specification
   └─> Add to vote collection if valid

5. PREPARE PHASE COMPLETION
   ├─> Wait for timeout or all votes received
   ├─> Calculate simple vote counts
   ├─> Tally: votes_for, votes_against, votes_abstained
   ├─> Check quorum: votes_for >= 27 AND total >= 27
   └─> If quorum failed: GOTO FAILED

6. COMMIT PHASE INITIATION
   ├─> Phase set to ConsensusPhase::Commit
   ├─> New timeout started
   ├─> Agents invited to commit
   └─> Votes may be reconsidered

7. COMMIT VOTE SUBMISSION
   ├─> Agents submit commit-phase votes
   ├─> Same validation as Prepare
   ├─> M35 Dissent Tracker records any role change
   └─> M33 aggregates commit votes

8. WEIGHTED AGGREGATION
   ├─> Call calculate_weighted_votes()
   ├─> Multiply each vote by agent's role weight
   ├─> Calculate: weighted_for, weighted_against, weighted_abstain
   └─> Determine consensus outcome

9. ENHANCED CONSENSUS CHECK
   ├─> Call enhanced_consensus_check()
   ├─> Verify CRITIC approval present
   ├─> Verify INTEGRATOR approval present
   └─> If either missing: consensus fails

10. QUORUM VERIFICATION
    ├─> M36 Quorum Calculator called
    ├─> Verify: votes_for >= 27 AND total >= 27
    ├─> Verify: weighted consensus achieved
    └─> If failed: GOTO FAILED

11. CONSENSUS DECISION
    ├─> If all checks pass: APPROVED
    ├─> If any check fails: REJECTED
    ├─> M31 transitions to Execute or Failed phase
    └─> M35 records dissent if applicable
```

---

## Vote Tracking and Validation

### Vote Validation Rules

1. **Proposal ID Validation**
   - Must match current proposal being voted on
   - Prevents votes on wrong proposals

2. **Agent ID Validation**
   - Agent must be in active fleet
   - Agent must not be Failed or Offline

3. **Vote Type Validation**
   - Must be Approve, Reject, or Abstain
   - No other values accepted

4. **Role Verification**
   - Agent's role must match submitted vote's role
   - Weight must match role specification
   - Prevents vote spoofing

5. **Weight Verification**
   - VALIDATOR: exactly 1.0
   - EXPLORER: exactly 0.8
   - CRITIC: exactly 1.2
   - INTEGRATOR: exactly 1.0
   - HISTORIAN: exactly 0.8

6. **Timestamp Validation**
   - Vote must be within consensus timeout window
   - Prevents stale votes

7. **Uniqueness Validation**
   - Each agent can vote once per phase
   - Duplicate votes rejected or replaced

---

## Weighted Vote Examples

### Example 1: Simple Approval

```
Votes:
  @0.A (VALIDATOR, 1.0): Approve
  agent-01 (VALIDATOR, 1.0): Approve
  agent-21 (EXPLORER, 0.8): Approve

Calculation:
  weighted_for = 1.0 + 1.0 + 0.8 = 2.8
  weighted_against = 0.0
  weighted_abstain = 0.0
```

### Example 2: Critic and Integrator Required

```
Votes:
  Multiple validators approve
  agent-29 (CRITIC, 1.2): Approve      ← REQUIRED
  agent-35 (INTEGRATOR, 1.0): Approve  ← REQUIRED

enhanced_consensus_check(votes) = true  ✓
```

### Example 3: Dissent Pattern

```
Votes:
  27 VALIDATOR approvals = 27.0
  6 CRITIC: 3 Approve (3.6), 3 Reject (3.6)

weighted_for = 27.0 + 3.6 = 30.6
weighted_against = 3.6

Dissent captured: 3 Critics disagreed
  → M35 records for learning
```

---

## ConsensusOutcome Structure

After vote aggregation, M33 produces ConsensusOutcome:

```rust
pub struct ConsensusOutcome {
    pub proposal_id: String,          // Which proposal
    pub quorum_reached: bool,         // >= 27 votes?
    pub votes_for: u32,               // Count of Approve votes
    pub votes_against: u32,           // Count of Reject votes
    pub votes_abstained: u32,         // Count of Abstain votes
    pub weighted_for: f64,            // Weighted approval sum
    pub weighted_against: f64,        // Weighted rejection sum
    pub execution_status: ExecutionStatus,  // Pending/Executing/Success/Failed
    pub completed_at: Option<SystemTime>,   // Completion timestamp
}
```

---

## Integration with Dissent Tracking

M33 provides vote data to M35 (Dissent Tracker):

- **Dissent Events:** When agents disagree on proposal
- **Role-Specific Dissent:** CRITIC or INTEGRATOR disagreement flagged
- **Minority Votes:** All rejections and abstentions recorded
- **Learning Input:** Dissent events used for agent improvement

---

## Performance Characteristics

| Operation | Time | Notes |
|-----------|------|-------|
| Vote submission | <1ms | Direct struct storage |
| Vote validation | <5ms | 7 validation checks |
| Weighted calculation | <10ms | Linear in agent count |
| Enhanced check | <5ms | HashMap lookups |
| Phase transition | <1ms | State change |

---

## Related Modules

| Module | Relationship | Purpose |
|--------|--------------|---------|
| M31 | Dependent | PBFT consensus orchestration |
| M32 | Dependency | Agent coordination and roles |
| M34 | Dependent | View change on timeout |
| M35 | Dependent | Dissent event recording |
| M36 | Dependent | Quorum threshold verification |

---

## Testing

Key test cases:

```rust
#[test]
fn test_weighted_votes()          // Verify weight calculations
#[test]
fn test_enhanced_consensus()      // Test CRITIC/INTEGRATOR requirements
#[test]
fn test_vote_validation()         // Verify invalid votes rejected
#[test]
fn test_quorum_reached()          // Test threshold logic
#[test]
fn test_dissent_recording()       // Verify disagreement capture
```

---

## Version History

| Version | Date | Changes |
|---------|------|---------|
| 1.0.0 | 2026-01-28 | Initial implementation |

---

*The Maintenance Engine v1.0.0 | M33: Vote Collector*
*Last Updated: 2026-01-28*
