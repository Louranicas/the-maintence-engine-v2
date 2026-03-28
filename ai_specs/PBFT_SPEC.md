# PBFT Consensus Specification

## Overview

This specification defines the Practical Byzantine Fault Tolerance (PBFT) consensus protocol adapted for the 40-agent Prometheus Swarm system. The protocol ensures consensus despite up to 13 Byzantine (malicious or faulty) agents.

---

## Core Parameters

| Parameter | Value | Formula | Description |
|-----------|-------|---------|-------------|
| n | 40 | Total agents | Total number of agents in the swarm |
| f | 13 | (n-1)/3 | Maximum Byzantine agents tolerated |
| q | 27 | 2f+1 | Quorum required for consensus |

### Byzantine Fault Tolerance Proof
```
n = 3f + 1
40 = 3(13) + 1
40 = 39 + 1 = 40 ✓

Quorum: 2f + 1 = 2(13) + 1 = 27
```

---

## Agent Roles (NAM-05)

| Role | Count | Weight | Focus | Description |
|------|-------|--------|-------|-------------|
| VALIDATOR | 20 | 1.0 | Correctness | Verify proposals against rules and constraints |
| EXPLORER | 8 | 0.8 | Alternatives | Discover alternative solutions and edge cases |
| CRITIC | 6 | 1.2 | Flaws | Identify weaknesses, vulnerabilities, issues |
| INTEGRATOR | 4 | 1.0 | Cross-system | Ensure system-wide compatibility |
| HISTORIAN | 2 | 0.8 | Precedent | Reference past decisions and patterns |

### Role Distribution
```
Total: 20 + 8 + 6 + 4 + 2 = 40 agents

Weighted Votes:
- VALIDATOR:   20 × 1.0 = 20.0
- EXPLORER:     8 × 0.8 =  6.4
- CRITIC:       6 × 1.2 =  7.2
- INTEGRATOR:   4 × 1.0 =  4.0
- HISTORIAN:    2 × 0.8 =  1.6
- Total Weight:          = 39.2
```

### Role Responsibilities

#### VALIDATOR (20 agents)
- Primary consensus participants
- Verify correctness of proposals
- Check constraint satisfaction
- Vote on proposal validity

#### EXPLORER (8 agents)
- Search alternative solution spaces
- Identify edge cases
- Propose optimizations
- Challenge assumptions

#### CRITIC (6 agents)
- Adversarial analysis
- Security vulnerability detection
- Performance bottleneck identification
- Risk assessment

#### INTEGRATOR (4 agents)
- Cross-system compatibility checks
- Dependency impact analysis
- Integration testing coordination
- API contract validation

#### HISTORIAN (2 agents)
- Historical pattern matching
- Precedent lookup
- Regression detection
- Knowledge preservation

---

## Consensus Phases

### Phase 1: PRE-PREPARE
```
Leader → All Agents

Message: <PRE-PREPARE, v, n, d, m>
  - v: current view number
  - n: sequence number
  - d: digest of proposal m
  - m: proposal content
```
- Leader proposes operation
- Broadcasts to all replicas
- Agents verify message authenticity

### Phase 2: PREPARE
```
Agent_i → All Agents

Message: <PREPARE, v, n, d, i>
  - v: current view number
  - n: sequence number
  - d: digest of proposal
  - i: agent identifier
```
- Agents validate proposal
- Broadcast PREPARE vote
- Wait for 2f PREPARE messages

### Phase 3: COMMIT
```
Agent_i → All Agents

Message: <COMMIT, v, n, d, i>
  - v: current view number
  - n: sequence number
  - d: digest of proposal
  - i: agent identifier
```
- Agent enters prepared state
- Broadcasts COMMIT message
- Wait for 2f+1 COMMIT messages

### Phase 4: REPLY
```
Agent_i → Client

Message: <REPLY, v, t, c, i, r>
  - v: current view number
  - t: timestamp
  - c: client identifier
  - i: agent identifier
  - r: result
```
- Execute operation
- Send result to client
- Client waits for f+1 matching replies

---

## View Change Protocol

### Trigger Conditions
- Leader timeout (no proposal received)
- Leader suspected faulty
- f+1 VIEW-CHANGE requests

### View Change Process

#### Step 1: Initiate
```
Agent_i → All Agents

Message: <VIEW-CHANGE, v+1, n, C, P, i>
  - v+1: new view number
  - n: last stable checkpoint sequence
  - C: checkpoint messages (2f+1)
  - P: prepared messages set
  - i: agent identifier
```

#### Step 2: New Leader
```
New Leader = (v+1) mod n

Leader → All Agents

Message: <NEW-VIEW, v+1, V, O>
  - v+1: new view number
  - V: set of valid VIEW-CHANGE messages
  - O: set of PRE-PREPARE messages for pending requests
```

#### Step 3: Recovery
- Process NEW-VIEW message
- Re-execute pending operations
- Resume normal operation

### Leader Selection
```rust
fn select_leader(view: u64, total_agents: usize) -> usize {
    (view as usize) % total_agents
}
```

---

## Message Types

| Type | Fields | Size | Purpose |
|------|--------|------|---------|
| PrePrepare | view, seq, digest, proposal | ~1KB | Leader proposal broadcast |
| Prepare | view, seq, digest, agent_id | ~100B | Validation vote |
| Commit | view, seq, digest, agent_id | ~100B | Commitment confirmation |
| ViewChange | new_view, checkpoints, prepared | ~5KB | Leader change request |
| NewView | new_view, view_changes, ops | ~10KB | New leader announcement |
| Reply | view, timestamp, client, result | ~500B | Operation result |
| Checkpoint | seq, digest, agent_id | ~100B | State snapshot marker |

### Message Structure (Rust)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrePrepare {
    pub view: u64,
    pub sequence: u64,
    pub digest: [u8; 32],
    pub proposal: Vec<u8>,
    pub timestamp: u64,
    pub signature: [u8; 64],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Prepare {
    pub view: u64,
    pub sequence: u64,
    pub digest: [u8; 32],
    pub agent_id: AgentId,
    pub signature: [u8; 64],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Commit {
    pub view: u64,
    pub sequence: u64,
    pub digest: [u8; 32],
    pub agent_id: AgentId,
    pub signature: [u8; 64],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewChange {
    pub new_view: u64,
    pub last_checkpoint_seq: u64,
    pub checkpoint_proofs: Vec<CheckpointProof>,
    pub prepared_proofs: Vec<PreparedProof>,
    pub agent_id: AgentId,
    pub signature: [u8; 64],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewView {
    pub new_view: u64,
    pub view_changes: Vec<ViewChange>,
    pub pre_prepares: Vec<PrePrepare>,
    pub signature: [u8; 64],
}
```

---

## Timeouts

| Event | Timeout | Action | Escalation |
|-------|---------|--------|------------|
| Request | 5s | Escalate to leader | Retry with backoff |
| Prepare | 10s | Initiate view change | Broadcast VIEW-CHANGE |
| Commit | 10s | Initiate view change | Broadcast VIEW-CHANGE |
| ViewChange | 30s | Force view change | Skip to v+2 |
| Checkpoint | 60s | Log warning | Continue operation |

### Timeout Configuration (Rust)

```rust
pub struct TimeoutConfig {
    pub request_timeout: Duration,
    pub prepare_timeout: Duration,
    pub commit_timeout: Duration,
    pub view_change_timeout: Duration,
    pub checkpoint_interval: Duration,
}

impl Default for TimeoutConfig {
    fn default() -> Self {
        Self {
            request_timeout: Duration::from_secs(5),
            prepare_timeout: Duration::from_secs(10),
            commit_timeout: Duration::from_secs(10),
            view_change_timeout: Duration::from_secs(30),
            checkpoint_interval: Duration::from_secs(60),
        }
    }
}
```

---

## Human @0.A

### Registration
```rust
pub struct HumanAgent {
    pub id: AgentId::new("@0.A"),
    pub tier: 0,  // Foundation tier
    pub weight: 1.0,
    pub role: AgentRole::Human,
    pub capabilities: vec![
        Capability::Vote,
        Capability::Dissent,
        Capability::Override,
    ],
}
```

### Properties
| Property | Value | Description |
|----------|-------|-------------|
| Agent ID | @0.A | Unique human identifier |
| Tier | 0 | Foundation level (highest priority) |
| Weight | 1.0 | Standard voting weight |
| Role | Human | Special human agent role |

### Capabilities

#### Vote
- Participate in consensus votes
- Weight: 1.0 (equal to VALIDATOR)
- Counted toward quorum

#### Dissent
- Register formal objection
- Triggers review process
- Can block non-critical operations

#### Override
- Emergency override capability
- Requires explicit invocation
- Logged and audited
- Can force consensus on critical decisions

### Human Override Protocol
```
1. Human invokes override
2. System logs override event
3. All agents receive OVERRIDE message
4. Current consensus round suspended
5. Human decision applied immediately
6. Resume normal operation
```

---

## Checkpointing

### Checkpoint Interval
- Every 100 sequence numbers
- Or every 60 seconds (whichever comes first)

### Checkpoint Process
```rust
pub struct Checkpoint {
    pub sequence: u64,
    pub state_digest: [u8; 32],
    pub agent_id: AgentId,
    pub timestamp: u64,
}

impl Checkpoint {
    pub fn is_stable(&self, confirmations: usize, f: usize) -> bool {
        confirmations >= 2 * f + 1
    }
}
```

### Garbage Collection
- Discard messages before stable checkpoint
- Retain at least 2 stable checkpoints
- Archive to persistent storage

---

## Security Considerations

### Message Authentication
- All messages signed with agent private key
- Signatures verified before processing
- Replay protection via sequence numbers

### Byzantine Behavior Detection
- Vote inconsistency tracking
- Timeout pattern analysis
- Automatic agent flagging

### Audit Trail
- All decisions logged
- Human overrides specially marked
- Immutable audit log

---

## Performance Characteristics

### Message Complexity
| Phase | Messages | Complexity |
|-------|----------|------------|
| Pre-Prepare | 1 | O(1) |
| Prepare | n-1 | O(n) |
| Commit | n | O(n) |
| Total | 2n | O(n) |

### Latency
- Best case: 3 message delays
- Worst case: 3 message delays + view change

### Throughput
- Limited by leader capacity
- Batching improves efficiency
- Target: 100-1000 ops/second

---

## Implementation Constants

```rust
pub mod pbft {
    pub const TOTAL_AGENTS: usize = 40;
    pub const MAX_BYZANTINE: usize = 13;
    pub const QUORUM_SIZE: usize = 27;
    pub const CHECKPOINT_INTERVAL: u64 = 100;

    pub const VALIDATOR_COUNT: usize = 20;
    pub const EXPLORER_COUNT: usize = 8;
    pub const CRITIC_COUNT: usize = 6;
    pub const INTEGRATOR_COUNT: usize = 4;
    pub const HISTORIAN_COUNT: usize = 2;

    pub const VALIDATOR_WEIGHT: f64 = 1.0;
    pub const EXPLORER_WEIGHT: f64 = 0.8;
    pub const CRITIC_WEIGHT: f64 = 1.2;
    pub const INTEGRATOR_WEIGHT: f64 = 1.0;
    pub const HISTORIAN_WEIGHT: f64 = 0.8;
}
```

---

## State Machine

```
                    ┌─────────────────────────────────────┐
                    │                                     │
                    ▼                                     │
    ┌───────────────────────────┐                        │
    │         IDLE              │                        │
    │   (waiting for request)   │                        │
    └───────────┬───────────────┘                        │
                │ receive request                        │
                ▼                                        │
    ┌───────────────────────────┐                        │
    │       PRE-PREPARE         │                        │
    │   (leader broadcasts)     │                        │
    └───────────┬───────────────┘                        │
                │ valid pre-prepare                      │
                ▼                                        │
    ┌───────────────────────────┐      timeout          │
    │         PREPARE           │ ──────────────────────►│
    │   (collect 2f prepares)   │                        │
    └───────────┬───────────────┘                        │
                │ 2f prepares received                   │
                ▼                                        │
    ┌───────────────────────────┐      timeout          │
    │          COMMIT           │ ──────────────────────►│
    │   (collect 2f+1 commits)  │                        │
    └───────────┬───────────────┘                        │
                │ 2f+1 commits received                  │
                ▼                                        │
    ┌───────────────────────────┐                        │
    │         EXECUTE           │                        │
    │   (apply operation)       │                        │
    └───────────┬───────────────┘                        │
                │ execution complete                     │
                ▼                                        │
    ┌───────────────────────────┐                        │
    │          REPLY            │                        │
    │   (send result)           │────────────────────────┘
    └───────────────────────────┘


    View Change:
    ┌───────────────────────────┐
    │       VIEW-CHANGE         │
    │   (elect new leader)      │
    └───────────┬───────────────┘
                │ 2f+1 view-change messages
                ▼
    ┌───────────────────────────┐
    │        NEW-VIEW           │
    │   (leader announces)      │
    └───────────┬───────────────┘
                │ valid new-view
                ▼
              IDLE
```

---

## Version History

| Version | Date | Changes |
|---------|------|---------|
| 1.0 | 2026-01-28 | Initial specification |
