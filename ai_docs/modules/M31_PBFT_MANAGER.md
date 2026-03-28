# Module M31: PBFT Manager

> **M31_PBFT_MANAGER** | Consensus orchestration | Layer: L6 Consensus | [Back to Index](INDEX.md)

---

## Navigation

| Direction | Link |
|-----------|------|
| Up | [INDEX.md](INDEX.md) |
| Layer | [L06_CONSENSUS.md](../layers/L06_CONSENSUS.md) |
| Related | [M32_AGENT_COORDINATOR.md](M32_AGENT_COORDINATOR.md) |
| Related | [M33_VOTE_COLLECTOR.md](M33_VOTE_COLLECTOR.md) |
| Related | [M34_VIEW_CHANGE_HANDLER.md](M34_VIEW_CHANGE_HANDLER.md) |
| Related | [M35_DISSENT_TRACKER.md](M35_DISSENT_TRACKER.md) |
| Related | [M36_QUORUM_CALCULATOR.md](M36_QUORUM_CALCULATOR.md) |

---

## Module Specification

### Overview

The PBFT Manager orchestrates Byzantine Fault Tolerant consensus across the CVA-NAM agent fleet. It manages the complete lifecycle of consensus proposals, from initial broadcast through execution, implementing the Practical Byzantine Fault Tolerance algorithm with n=40 agents and f=13 fault tolerance threshold.

### Module Properties

| Property | Value |
|----------|-------|
| Module ID | M31 |
| Module Name | PBFT Manager |
| Layer | L6 (Consensus) |
| Version | 1.0.0 |
| Dependencies | M32 (Agent Coordinator), M33 (Vote Collector), M36 (Quorum Calculator) |
| Dependents | M34 (View Change Handler), M35 (Dissent Tracker) |

---

## PBFT Configuration

| Parameter | Value | Description |
|-----------|-------|-------------|
| n (total agents) | 40 | CVA-NAM heterogeneous fleet |
| f (fault tolerance) | 13 | Byzantine fault tolerance threshold: (n-1)/3 |
| q (quorum) | 27 | Minimum consensus requirement: 2f+1 |

---

## Consensus Phases

The PBFT Manager orchestrates consensus through 6 sequential phases:

| Phase | Name | Description | Timeout |
|-------|------|-------------|---------|
| PrePrepare | Initial Broadcast | Primary proposes action, broadcasts to all replicas | Dynamic |
| Prepare | Vote Collection Phase 1 | Replicas validate proposal and vote on acceptance | Action-dependent |
| Commit | Vote Collection Phase 2 | Replicas commit to proposal execution | Action-dependent |
| Execute | Action Execution | Agreed action is executed by the system | Action-dependent |
| Complete | Consensus Success | Proposal successfully executed and committed | Immediate |
| Failed | Consensus Failed | Quorum not reached, proposal rejected | Immediate |

---

## Core Types

### ConsensusPhase Enumeration

```rust
pub enum ConsensusPhase {
    /// Initial proposal broadcast
    PrePrepare,
    /// Collecting prepare votes
    Prepare,
    /// Collecting commit votes
    Commit,
    /// Executing agreed action
    Execute,
    /// Consensus complete
    Complete,
    /// Consensus failed
    Failed,
}
```

### ConsensusProposal Structure

```rust
pub struct ConsensusProposal {
    /// Unique proposal ID (UUID)
    pub id: String,
    /// View number for leader election tracking
    pub view_number: u64,
    /// Sequence number for ordering proposals
    pub sequence_number: u64,
    /// Type of action requiring consensus
    pub action_type: ConsensusAction,
    /// JSON-serialized action payload
    pub action_payload: String,
    /// ID of agent that proposed this action
    pub proposer: String,
    /// Current consensus phase
    pub phase: ConsensusPhase,
    /// Creation timestamp
    pub timestamp: std::time::SystemTime,
    /// Timeout in milliseconds
    pub timeout_ms: u64,
}
```

### ConsensusAction Enumeration

Actions requiring PBFT consensus with dynamic timeouts:

```rust
pub enum ConsensusAction {
    /// Emergency service termination (kill -9) - 60s timeout
    ServiceTermination,
    /// Database migration - 300s timeout
    DatabaseMigration,
    /// Credential rotation - 120s timeout
    CredentialRotation,
    /// Multi-service cascade restart - 180s timeout
    CascadeRestart,
    /// Configuration rollback - 90s timeout
    ConfigRollback,
}
```

Each action has a `required_quorum()` method returning `PBFT_Q` (27 votes) and `default_timeout_seconds()` for action-specific timeouts.

### ConsensusOutcome Structure

```rust
pub struct ConsensusOutcome {
    /// Proposal ID that was voted on
    pub proposal_id: String,
    /// Whether quorum (27/40 votes) was reached
    pub quorum_reached: bool,
    /// Count of approval votes
    pub votes_for: u32,
    /// Count of rejection votes
    pub votes_against: u32,
    /// Count of abstention votes
    pub votes_abstained: u32,
    /// Weighted approval votes
    pub weighted_for: f64,
    /// Weighted rejection votes
    pub weighted_against: f64,
    /// Execution status (Pending, Executing, Success, Failed, Aborted, RolledBack)
    pub execution_status: ExecutionStatus,
    /// Timestamp of consensus completion
    pub completed_at: Option<std::time::SystemTime>,
}
```

### ExecutionStatus Enumeration

```rust
pub enum ExecutionStatus {
    /// Pending execution
    Pending,
    /// Currently executing
    Executing,
    /// Successfully executed
    Success,
    /// Execution failed
    Failed,
    /// Execution aborted
    Aborted,
    /// Changes rolled back
    RolledBack,
}
```

---

## Agent Roles (NAM-05)

The consensus system includes 40 agents plus Human @0.A in the following roles:

| Role | Count | Weight | Focus | Tier |
|------|-------|--------|-------|------|
| VALIDATOR | 20 | 1.0 | Correctness verification | 1 |
| EXPLORER | 8 | 0.8 | Alternative detection | 2 |
| CRITIC | 6 | 1.2 | Flaw detection | 3 |
| INTEGRATOR | 4 | 1.0 | Cross-system impact | 4 |
| HISTORIAN | 2 | 0.8 | Precedent matching | 5 |

---

## Human @0.A (NAM R5)

The human agent @0.A participates as a peer in consensus:

| Property | Value |
|----------|-------|
| Agent ID | @0.A |
| Tier | 0 (foundation) |
| Weight | 1.0 (standard voting weight) |
| Role | VALIDATOR (can act as any role) |
| Status | Active |
| Success Rate | 1.0 (always correct) |

The human agent can:
- Vote on any consensus proposal
- Raise dissent and capture disagreement
- Request consensus reconsideration
- Escalate decisions for further review

---

## API Functions

### Core Consensus Functions

#### is_quorum_reached

```rust
pub fn is_quorum_reached(votes_for: u32, total_votes: u32) -> bool
```

Returns true if minimum quorum (27 votes) is reached and total votes >= 27.

**Parameters:**
- `votes_for`: Number of approval votes
- `total_votes`: Total votes received

**Returns:** `bool` - true if quorum threshold met

#### calculate_weighted_votes

```rust
pub fn calculate_weighted_votes(votes: &[ConsensusVote]) -> (f64, f64, f64)
```

Calculates weighted vote totals based on agent roles and weights.

**Parameters:**
- `votes`: Slice of ConsensusVote structures

**Returns:** Tuple of (weighted_for, weighted_against, weighted_abstain)

#### enhanced_consensus_check

```rust
pub fn enhanced_consensus_check(votes: &[ConsensusVote]) -> bool
```

Verifies NAM-05 enhanced consensus requirements: at least 1 CRITIC and 1 INTEGRATOR must approve.

**Parameters:**
- `votes`: Slice of ConsensusVote structures

**Returns:** `bool` - true if both CRITIC and INTEGRATOR approval present

### ConsensusProposal Methods

#### new

```rust
pub fn new(
    id: impl Into<String>,
    view_number: u64,
    sequence_number: u64,
    action_type: ConsensusAction,
    proposer: impl Into<String>,
) -> Self
```

Creates a new consensus proposal in PrePrepare phase.

#### get_timeout

```rust
pub fn get_timeout(&self) -> u64
```

Returns action-specific timeout in milliseconds:
- ServiceTermination: 60,000 ms
- DatabaseMigration: 300,000 ms
- CredentialRotation: 120,000 ms
- CascadeRestart: 180,000 ms
- ConfigRollback: 90,000 ms

---

## Consensus Workflow

```
1. PROPOSAL CREATION
   └─> ConsensusProposal created with action_type
       ├─> ID assigned (UUID)
       ├─> Phase set to PrePrepare
       └─> Timeout calculated

2. BROADCAST PHASE (PrePrepare)
   └─> Primary broadcasts proposal to all agents
       ├─> M32 Agent Coordinator distributes
       └─> M33 Vote Collector initializes tracking

3. PREPARE PHASE
   └─> Agents vote on proposal validity
       ├─> Agent examines action_type
       ├─> Validates against local state
       ├─> Returns approve/reject/abstain
       └─> M33 aggregates votes

4. QUORUM CHECK
   └─> M36 Quorum Calculator verifies
       ├─> votes_for >= 27?
       ├─> enhanced_consensus_check passed?
       └─> If fail: goto FAILED phase

5. COMMIT PHASE
   └─> Agents commit to execution
       ├─> Prepare votes finalized
       ├─> Commit votes collected
       └─> M35 Dissent Tracker records any dissent

6. EXECUTE PHASE
   └─> Agreed action executed
       ├─> execution_status = Executing
       ├─> Action applied to system
       └─> Outcomes recorded

7. COMPLETION
   └─> Consensus outcome finalized
       ├─> execution_status updated
       ├─> completed_at timestamp set
       └─> Episode recorded for learning
```

---

## Error Handling

The PBFT Manager handles consensus failures through:

- **Quorum Failure**: If < 27 votes or < 27 total votes, proposal fails
- **Phase Timeout**: Each phase has action-specific timeout; if exceeded, view change triggered
- **Enhanced Consensus Failure**: If CRITIC or INTEGRATOR approval missing, consensus fails
- **Execution Failure**: If action execution fails, status set to Failed/RolledBack

---

## Related Modules

| Module | Relationship | Purpose |
|--------|--------------|---------|
| M32 | Dependency | Agent coordination and role management |
| M33 | Dependency | Vote collection and aggregation |
| M34 | Dependent | View changes and leader election |
| M35 | Dependent | Dissent event capture and learning |
| M36 | Dependency | Quorum threshold calculation |
| M28 | Integration | Episode recording for learning |

---

## Security Considerations

1. **Byzantine Fault Tolerance**: System tolerates up to 13 malicious agents
2. **Vote Weight Validation**: Each agent's weight verified before vote acceptance
3. **Proposal Integrity**: All proposals signed and timestamped
4. **Dissent Tracking**: All disagreement captured for analysis (M35)
5. **Human Override**: Human @0.A can escalate or veto proposals

---

## Performance Characteristics

| Metric | Value | Notes |
|--------|-------|-------|
| Proposal Creation | <1ms | Direct struct instantiation |
| Vote Aggregation | <100ms | Linear in agent count (40) |
| Quorum Check | <10ms | Simple threshold comparison |
| Phase Transition | <50ms | State machine update |
| Complete Consensus | 5-300s | Depends on action timeout |

---

## Testing

The module includes comprehensive tests:

```rust
#[test]
fn test_pbft_constants()    // Verify n=40, f=13, q=27
#[test]
fn test_quorum_check()      // Test quorum threshold logic
#[test]
fn test_human_agent()       // Verify @0.A agent properties
#[test]
fn test_agent_fleet()       // Validate agent role distribution
#[test]
fn test_enhanced_consensus()// Test NAM-05 requirement
```

---

## Version History

| Version | Date | Changes |
|---------|------|---------|
| 1.0.0 | 2026-01-28 | Initial implementation |

---

*The Maintenance Engine v1.0.0 | M31: PBFT Manager*
*Last Updated: 2026-01-28*
