# Module M34: View Change Handler

> **M34_VIEW_CHANGE_HANDLER** | Leader election | Layer: L6 Consensus | [Back to Index](INDEX.md)

---

## Navigation

| Direction | Link |
|-----------|------|
| Up | [INDEX.md](INDEX.md) |
| Layer | [L06_CONSENSUS.md](../layers/L06_CONSENSUS.md) |
| Related | [M31_PBFT_MANAGER.md](M31_PBFT_MANAGER.md) |
| Related | [M32_AGENT_COORDINATOR.md](M32_AGENT_COORDINATOR.md) |
| Related | [M33_VOTE_COLLECTOR.md](M33_VOTE_COLLECTOR.md) |
| Related | [M35_DISSENT_TRACKER.md](M35_DISSENT_TRACKER.md) |
| Related | [M36_QUORUM_CALCULATOR.md](M36_QUORUM_CALCULATOR.md) |

---

## Module Specification

### Overview

The View Change Handler manages leader election and consensus round recovery in PBFT consensus. It detects consensus failures, triggers view changes when the primary leader fails or produces invalid proposals, and orchestrates the election of a new leader. Operating based on view_number from ConsensusProposal, it maintains system liveness and ensures consensus can continue despite primary failures.

### Module Properties

| Property | Value |
|----------|-------|
| Module ID | M34 |
| Module Name | View Change Handler |
| Layer | L6 (Consensus) |
| Version | 1.0.0 |
| Dependencies | M31 (PBFT Manager), M32 (Agent Coordinator), M33 (Vote Collector) |
| Dependents | M35 (Dissent Tracker) |

---

## PBFT Configuration

| Parameter | Value | Description |
|-----------|-------|-------------|
| Primary Timeout | 5-300s | Action-dependent; triggers view change |
| View Number Increment | +1 | Next view upon change |
| Leader Selection | Deterministic | Primary = view_number % n |
| Timeout Multiplier | 2.0x | Double timeout each retry |

---

## View Change Triggers

### Trigger Conditions

View changes are triggered by:

| Trigger | Condition | Response |
|---------|-----------|----------|
| **Proposal Timeout** | No proposal received within phase timeout | Initiate view change |
| **Prepare Timeout** | Prepare phase exceeds timeout | Request new view |
| **Commit Timeout** | Commit phase exceeds timeout | Request new view |
| **Quorum Failure** | Votes collected but < 27 threshold | Move to new view |
| **Invalid Proposal** | Primary sends malformed proposal | Challenge and view change |
| **Enhanced Consensus Fail** | CRITIC or INTEGRATOR approval missing | Retry with new leader |
| **Execution Failure** | Action execution fails repeatedly | Elect new coordinator |

### Timeout Values (From M31)

| Action | Timeout (ms) | Timeout (s) |
|--------|--------------|------------|
| ServiceTermination | 60,000 | 60 |
| DatabaseMigration | 300,000 | 300 |
| CredentialRotation | 120,000 | 120 |
| CascadeRestart | 180,000 | 180 |
| ConfigRollback | 90,000 | 90 |

---

## Leader Selection Algorithm

### Deterministic Primary Selection

```
Primary = view_number % n

Example with n=40:
  View 0: Primary = agent-00 (or closest valid)
  View 1: Primary = agent-01
  View 2: Primary = agent-02
  ...
  View 40: Primary = agent-00 (wraps around)
  View 41: Primary = agent-01
```

### Valid Primary Requirements

A primary must:
1. Be in ACTIVE or IDLE status (not Failed/Offline)
2. Have successful history (success_rate > 0.0)
3. Be available in current agent fleet
4. Accept the role (not in deliberate timeout)

### Fallback Selection

If selected primary is unavailable:
1. Try next agent: (view_number + 1) % n
2. Continue sequentially until valid primary found
3. Record skipped agents for investigation
4. Log view change rationale to M35

---

## Core Consensus Phase Structure

### ConsensusPhase with View Semantics

From M31, ConsensusPhase tracks progress:

```rust
pub enum ConsensusPhase {
    PrePrepare,    // Primary broadcasts proposal (contains view_number)
    Prepare,       // Collect initial votes (with view validation)
    Commit,        // Confirm execution intent (with view validation)
    Execute,       // Action execution (view committed)
    Complete,      // Success - view finalized
    Failed,        // Failure - triggers new view
}
```

### View Number Tracking

Each ConsensusProposal includes:
```rust
pub view_number: u64,  // Incremented on view change
```

---

## View Change Workflow

### Detection Phase

```
1. TIMEOUT DETECTION
   ├─> Monitor phase timeout (action-specific)
   ├─> If timeout exceeded:
   │   ├─> Check current votes collected
   │   ├─> If < 27 votes: insufficient quorum
   │   ├─> If >= 27 but failed other checks: enhanced consensus failed
   │   └─> Trigger view change

2. FAILURE DETECTION
   ├─> Monitor proposal validity
   ├─> If primary sends invalid proposal:
   │   ├─> Agents reject
   │   ├─> Quorum never reaches 27
   │   └─> Timeout fires -> view change

3. DISSENT RECORDING
   └─> M35 Dissent Tracker logs reasons
       ├─> Timeout type (Prepare/Commit/Execute)
       ├─> Votes received (< 27)
       ├─> Primary ID
       └─> View number
```

### View Change Initiation

```
1. VIEW INCREMENT
   ├─> view_number += 1
   ├─> Update in M31
   └─> Propagate to all agents (M32)

2. PRIMARY SELECTION
   ├─> Calculate new_primary = view_number % n
   ├─> Verify new_primary is ACTIVE/IDLE
   ├─> If offline: try (view_number + 1) % n, etc.
   ├─> Announce new primary to fleet
   └─> Log selection to dissent tracker (M35)

3. STATE RESET
   ├─> Clear previous proposal (or retain for retry)
   ├─> Reset vote collection
   ├─> Clear Prepare phase votes
   ├─> Clear Commit phase votes
   ├─> Timeout counter reset
   └─> Ready for new round with new view

4. ANNOUNCEMENT PHASE
   ├─> New primary prepares proposal
   ├─> New proposal created with:
   │   ├─> view_number = incremented value
   │   ├─> sequence_number = same or incremented
   │   └─> same action_type (retry)
   ├─> Broadcast to all agents
   └─> Begin new Prepare phase
```

### Multi-View Retry Strategy

```
View 0: Primary = agent-00, timeout = 60s
  └─> Fails

View 1: Primary = agent-01, timeout = 120s (2x)
  └─> Fails

View 2: Primary = agent-02, timeout = 240s (4x)
  └─> Succeeds OR timeout triggers failover

Maximum Retries: ceil(log2(n)) = 6 views before escalation
```

---

## Agent Role in View Changes

### Agent Responsibilities

Each agent must:
1. **Detect timeout** - Monitor proposal receipt
2. **Initiate view change request** - Signal new primary needed
3. **Validate new primary** - Confirm selection is correct
4. **Clear state** - Prepare for new view round
5. **Participate in new round** - Vote in fresh proposal

### Primary Responsibilities

The selected primary must:
1. **Prepare new proposal** - Create valid proposal for action
2. **Broadcast** - Send to all agents in PrePrepare phase
3. **Coordinate** - Collect votes and manage phases
4. **Handle failures** - If this view also fails, trigger another

---

## Integration with Other Modules

### M31 (PBFT Manager) Integration

M34 interacts with M31:
- Receives notification of phase timeout
- Increments view_number in current proposal
- Creates new proposal with incremented view
- Handles proposal state across view changes

### M32 (Agent Coordinator) Integration

M34 uses M32 for:
- Agent status queries (ACTIVE/IDLE/Failed/Offline)
- Primary selection verification
- Fleet communication (broadcast new view)
- Agent state reset for new view

### M33 (Vote Collector) Integration

M34 coordinates with M33:
- Request vote collection reset
- Discard votes from previous view
- Begin new vote collection in new view
- Verify quorum in context of new primary

### M35 (Dissent Tracker) Integration

M34 reports to M35:
- View change trigger (why it happened)
- Primary transition (from → to)
- Vote analysis (insufficient quorum, enhanced check failed)
- Retry pattern (which views tried, outcomes)

---

## View Change Scenarios

### Scenario 1: Prepare Phase Timeout

```
View 0, Primary = agent-00
  ├─> Sends proposal in PrePrepare
  ├─> Agents begin Prepare phase voting
  ├─> After 60s timeout, only 20 votes received (< 27)
  ├─> View change triggered
  └─> View 1, Primary = agent-01
      ├─> New proposal sent
      ├─> 30 votes collected (> 27) ✓
      └─> Proceeds to Commit phase

Dissent recorded:
  - View 0 failed: Insufficient prepare votes (20/40)
  - Primary agent-00: May need investigation
  - Successful retry under agent-01
```

### Scenario 2: Enhanced Consensus Failure

```
View 0, Primary = agent-00
  ├─> Proposal broadcast
  ├─> Prepare phase: 35 votes collected (> 27) ✓
  ├─> Commit phase: 30 votes collected (> 27) ✓
  ├─> BUT: No CRITIC approval (role-specific failure)
  ├─> enhanced_consensus_check() fails
  └─> View change triggered
      ├─> View 1, Primary = agent-01
      ├─> Proposal includes additional safeguards
      ├─> Solicits explicit CRITIC approval
      └─> Succeeds with enhanced consensus ✓

Dissent recorded:
  - View 0: Enhanced consensus failed
  - Reason: CRITIC approval missing
  - Action: Reframe proposal for critics
```

### Scenario 3: Primary Offline

```
View 5, Primary selected = agent-32 (offline)
  ├─> M32 queries agent status
  ├─> agent-32 is OFFLINE
  ├─> Fallback to agent-33 (online)
  └─> View 5, Primary = agent-33 ✓
      ├─> Proposal broadcast
      ├─> Voting proceeds normally
      └─> Consensus reached

Logged:
  - Primary fallback: agent-32 → agent-33
  - Reason: Original primary offline
  - Recovery: Successful on fallback
```

---

## Termination Conditions

View changes continue until one of:

1. **Consensus Success**
   - Quorum reached (27+ votes)
   - Enhanced consensus check passed
   - Action executed successfully
   - Proposal finalized

2. **Escalation**
   - Maximum view changes exceeded (e.g., 8 views)
   - Too many agents consistently voting against
   - System-level failure detected
   - Escalate to human @0.A

3. **Timeout Limit**
   - Total time exceeds action-specific maximum
   - E.g., ServiceTermination max 60s total
   - Abort and return Failed status

---

## Performance Characteristics

| Operation | Time | Notes |
|-----------|------|-------|
| View change detection | <100ms | Timeout monitoring |
| View number increment | <1ms | Counter update |
| Primary selection | <5ms | Modulo operation + lookup |
| State reset | <10ms | Vote collection clear |
| Announcement | <50ms | Broadcast to 40 agents |

---

## Related Modules

| Module | Relationship | Purpose |
|--------|--------------|---------|
| M31 | Dependency | Proposal and phase management |
| M32 | Dependency | Agent status and communication |
| M33 | Dependency | Vote collection management |
| M35 | Dependent | Dissent and retry logging |

---

## Testing

Key test scenarios:

```rust
#[test]
fn test_view_number_increment()     // Verify view += 1
#[test]
fn test_primary_selection()          // Test view_number % n
#[test]
fn test_fallback_primary()           // Test offline primary recovery
#[test]
fn test_timeout_detection()          // Verify timeout firing
#[test]
fn test_state_reset_on_view_change() // Verify vote clearing
#[test]
fn test_enhanced_consensus_retrigger()// Test CRITIC/INTEGRATOR failure
```

---

## Version History

| Version | Date | Changes |
|---------|------|---------|
| 1.0.0 | 2026-01-28 | Initial implementation |

---

*The Maintenance Engine v1.0.0 | M34: View Change Handler*
*Last Updated: 2026-01-28*
