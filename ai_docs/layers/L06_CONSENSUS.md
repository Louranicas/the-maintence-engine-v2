# Layer 6: Consensus

> **L06_CONSENSUS** | PBFT Consensus Layer | [Back to Index](../INDEX.md)

---

## Navigation

| Direction | Link |
|-----------|------|
| Up | [INDEX.md](../INDEX.md) |
| Previous | [L05_LEARNING.md](L05_LEARNING.md) |
| Related | [PBFT_SPEC.md](../../ai_specs/PBFT_SPEC.md) |
| NAM Compliance | [NAM_SPEC.md](../../ai_specs/NAM_SPEC.md) |

---

## Layer Overview

The Consensus Layer (L6) implements Practical Byzantine Fault Tolerance (PBFT) for coordinating decisions across 40 distributed maintenance agents. It ensures that all nodes agree on remediation actions even in the presence of faulty or malicious nodes, with Human @0.A integration for NAM R5 compliance.

### Layer Properties

| Property | Value |
|----------|-------|
| Layer ID | L6 |
| Layer Name | Consensus |
| Source Directory | `src/m6_consensus/` |
| Dependencies | L1-L5 |
| Dependents | External Systems |
| Modules | M31-M36 |
| Algorithm | PBFT |
| Agents | 40 (CVA-NAM fleet) |
| Fault Tolerance | f=13 (Byzantine) |
| Quorum | q=27 (2f+1) |

---

## Architecture

```
+------------------------------------------------------------------+
|                      L6: Consensus Layer                           |
+------------------------------------------------------------------+
|                                                                  |
|  +---------------------------+  +---------------------------+    |
|  |     PBFT Engine           |  |    Agent Coordinator      |    |
|  |                           |  |                           |    |
|  |  - Message handling       |  |  - 40 agent management    |    |
|  |  - State machine          |  |  - Role assignment        |    |
|  |  - Quorum tracking        |  |  - Heartbeat monitoring   |    |
|  |  - Checkpoint mgmt        |  |  - View coordination      |    |
|  +-------------+-------------+  +-------------+-------------+    |
|                |                              |                  |
|                +------------+-----------------+                  |
|                             |                                    |
|  +---------------------------+---------------------------+       |
|  |              Voting & Dissent Management              |       |
|  |                                                       |       |
|  |  - Vote collection        - Dissent capture (NAM R3)  |       |
|  |  - Quorum verification    - Human @0.A integration    |       |
|  |  - Result aggregation     - Weighted voting           |       |
|  +-------------------------------------------------------+       |
|                                                                  |
+------------------------------------------------------------------+
```

---

## Module Reference (M31-M36)

| Module | File | Purpose |
|--------|------|---------|
| M31 | `pbft.rs` | PBFT engine - consensus protocol |
| M32 | `agent.rs` | Agent coordinator - fleet management |
| M33 | `voting.rs` | Voting mechanism - vote collection |
| M34 | `dissent.rs` | Dissent capture - minority recording (NAM R3) |
| M35 | `quorum.rs` | Quorum manager - threshold verification |
| M36 | `human.rs` | Human @0.A integration (NAM R5) |

---

## PBFT Configuration

### Global Constants

```rust
/// Total number of agents in CVA-NAM fleet
pub const PBFT_N: usize = 40;

/// Maximum Byzantine faults tolerable (n = 3f + 1)
pub const PBFT_F: usize = 13;

/// Quorum requirement (2f + 1)
pub const PBFT_Q: usize = 27;
```

### Byzantine Fault Tolerance

| Parameter | Value | Formula |
|-----------|-------|---------|
| Total Nodes (n) | 40 | n = 3f + 1 |
| Fault Tolerance (f) | 13 | f = (n-1)/3 |
| Quorum (q) | 27 | q = 2f + 1 |

---

## Core Types

### ConsensusPhase

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum ConsensusPhase {
    /// Idle, no active consensus
    Idle,

    /// Pre-prepare phase (primary broadcasts)
    PrePrepare,

    /// Prepare phase (replicas acknowledge)
    Prepare,

    /// Commit phase (replicas commit)
    Commit,

    /// Executed (consensus achieved)
    Executed,

    /// View change in progress
    ViewChange,
}
```

### ConsensusProposal

```rust
#[derive(Debug, Clone)]
pub struct ConsensusProposal {
    /// Unique proposal identifier
    pub id: ProposalId,

    /// Proposed action
    pub action: ConsensusAction,

    /// Proposing agent
    pub proposer: AgentId,

    /// Current phase
    pub phase: ConsensusPhase,

    /// View number
    pub view: u64,

    /// Sequence number
    pub sequence: u64,

    /// Collected votes
    pub votes: Vec<ConsensusVote>,

    /// Dissent records
    pub dissent: Vec<DissentEvent>,

    /// Creation timestamp
    pub created_at: DateTime<Utc>,

    /// Timeout for this proposal
    pub timeout: Duration,
}
```

### ConsensusAction

```rust
#[derive(Debug, Clone)]
pub enum ConsensusAction {
    /// Remediation requiring consensus
    Remediation(RemediationAction),

    /// Service lifecycle change
    ServiceLifecycle { service: ServiceId, action: LifecycleAction },

    /// Configuration change
    ConfigChange { key: String, value: Value },

    /// Agent role change
    AgentRoleChange { agent: AgentId, new_role: AgentRole },

    /// Emergency action
    Emergency(EmergencyAction),
}
```

### ConsensusVote

```rust
#[derive(Debug, Clone)]
pub struct ConsensusVote {
    /// Voting agent
    pub agent_id: AgentId,

    /// Agent role
    pub role: AgentRole,

    /// Vote decision
    pub decision: VoteDecision,

    /// Vote weight (based on role)
    pub weight: f64,

    /// Optional reasoning
    pub reasoning: Option<String>,

    /// Vote timestamp
    pub timestamp: DateTime<Utc>,

    /// Digital signature
    pub signature: Signature,
}

#[derive(Debug, Clone, PartialEq)]
pub enum VoteDecision {
    Approve,
    Reject,
    Abstain,
}
```

### DissentEvent

```rust
#[derive(Debug, Clone)]
pub struct DissentEvent {
    /// Dissenting agent
    pub agent_id: AgentId,

    /// Proposal being dissented
    pub proposal_id: ProposalId,

    /// Dissent reasoning
    pub reasoning: String,

    /// Alternative proposal (if any)
    pub alternative: Option<ConsensusAction>,

    /// Dissent weight
    pub weight: f64,

    /// Timestamp
    pub timestamp: DateTime<Utc>,
}
```

---

## Agent Roles (NAM-05)

| Role | Count | Weight | Focus |
|------|-------|--------|-------|
| VALIDATOR | 20 | 1.0 | Correctness verification |
| EXPLORER | 8 | 0.8 | Alternative detection |
| CRITIC | 6 | 1.2 | Flaw detection (elevated weight) |
| INTEGRATOR | 4 | 1.0 | Cross-system impact analysis |
| HISTORIAN | 2 | 0.8 | Precedent matching |

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum AgentRole {
    Validator,   // 20 agents, weight 1.0
    Explorer,    // 8 agents, weight 0.8
    Critic,      // 6 agents, weight 1.2
    Integrator,  // 4 agents, weight 1.0
    Historian,   // 2 agents, weight 0.8
}

impl AgentRole {
    pub fn weight(&self) -> f64 {
        match self {
            AgentRole::Validator => 1.0,
            AgentRole::Explorer => 0.8,
            AgentRole::Critic => 1.2,
            AgentRole::Integrator => 1.0,
            AgentRole::Historian => 0.8,
        }
    }

    pub fn count(&self) -> usize {
        match self {
            AgentRole::Validator => 20,
            AgentRole::Explorer => 8,
            AgentRole::Critic => 6,
            AgentRole::Integrator => 4,
            AgentRole::Historian => 2,
        }
    }
}
```

---

## PBFT Engine

### PBFT Protocol Phases

```
Client      Primary     Replica 1   Replica 2   Replica 3
   |           |           |           |           |
   |--REQUEST->|           |           |           |
   |           |           |           |           |
   |           |--PRE-PREPARE-------->|---------->|
   |           |<--PREPARE-|           |           |
   |           |---------->|--PREPARE->|---------->|
   |           |<----------|<-PREPARE--|<----------|
   |           |           |           |           |
   |           |--COMMIT-->|--COMMIT-->|--COMMIT-->|
   |           |<--COMMIT--|<--COMMIT--|<--COMMIT--|
   |           |           |           |           |
   |<--REPLY---|<--REPLY---|<--REPLY---|<--REPLY---|
```

### PBFT Engine API

```rust
pub struct PbftEngine {
    pub fn new(config: PbftConfig) -> Self;

    /// Submit proposal for consensus
    pub async fn submit(&self, action: ConsensusAction) -> Result<ConsensusResult>;

    /// Process incoming PBFT message
    pub async fn process_message(&mut self, msg: PbftMessage) -> Result<()>;

    /// Get current view number
    pub fn current_view(&self) -> u64;

    /// Get current primary/leader
    pub fn current_primary(&self) -> AgentId;

    /// Check if this node is primary
    pub fn is_primary(&self) -> bool;

    /// Get current consensus state
    pub fn state(&self) -> &ConsensusPhase;

    /// Force view change (emergency)
    pub async fn force_view_change(&mut self) -> Result<()>;

    /// Get consensus history
    pub fn history(&self, limit: usize) -> Vec<ConsensusRecord>;
}
```

### Quorum Logic

```rust
impl PbftEngine {
    /// Calculate required quorum size
    pub fn quorum_size(&self) -> usize {
        PBFT_Q  // 27 for n=40
    }

    /// Check if we have enough prepares
    pub fn has_prepare_quorum(&self, proposal_id: &ProposalId) -> bool {
        self.get_prepare_count(proposal_id) >= self.quorum_size()
    }

    /// Check if we have enough commits
    pub fn has_commit_quorum(&self, proposal_id: &ProposalId) -> bool {
        self.get_commit_count(proposal_id) >= self.quorum_size()
    }

    /// Calculate weighted vote total
    pub fn weighted_vote_total(&self, votes: &[ConsensusVote]) -> f64 {
        votes.iter()
            .filter(|v| v.decision == VoteDecision::Approve)
            .map(|v| v.weight)
            .sum()
    }
}
```

---

## Agent Coordinator

### Agent Management

```rust
pub struct AgentCoordinator {
    /// Register a new agent
    pub async fn register(&mut self, agent: AgentInfo) -> Result<AgentId>;

    /// Deregister agent
    pub async fn deregister(&mut self, id: &AgentId) -> Result<()>;

    /// List all agents
    pub fn list_agents(&self) -> Vec<&AgentInfo>;

    /// Get agent by ID
    pub fn get_agent(&self, id: &AgentId) -> Option<&AgentInfo>;

    /// Get agents by role
    pub fn by_role(&self, role: AgentRole) -> Vec<&AgentInfo>;

    /// Get healthy agents
    pub fn healthy_agents(&self) -> Vec<&AgentInfo>;

    /// Cluster size
    pub fn cluster_size(&self) -> usize;
}

pub struct AgentInfo {
    pub id: AgentId,
    pub role: AgentRole,
    pub address: SocketAddr,
    pub capabilities: Vec<Capability>,
    pub status: AgentStatus,
    pub last_heartbeat: DateTime<Utc>,
    pub joined_at: DateTime<Utc>,
    pub weight: f64,
}
```

### Heartbeat Protocol

```rust
impl AgentCoordinator {
    /// Start heartbeat monitoring
    pub async fn start_heartbeat_monitor(&self) {
        loop {
            for agent in self.agents.values_mut() {
                let elapsed = Utc::now() - agent.last_heartbeat;
                if elapsed > self.config.heartbeat_timeout {
                    agent.status = AgentStatus::Suspected;
                    self.emit_event(CoordinatorEvent::AgentSuspected(agent.id.clone()));
                }
            }
            tokio::time::sleep(self.config.heartbeat_interval).await;
        }
    }

    /// Process heartbeat from agent
    pub fn process_heartbeat(&mut self, agent_id: &AgentId) {
        if let Some(agent) = self.agents.get_mut(agent_id) {
            agent.last_heartbeat = Utc::now();
            agent.status = AgentStatus::Healthy;
        }
    }
}
```

---

## Dissent Capture (NAM R3)

### Dissent API

```rust
pub struct DissentManager {
    /// Record dissent from agent
    pub fn record(&mut self, dissent: DissentEvent);

    /// Get dissent for proposal
    pub fn for_proposal(&self, proposal_id: &ProposalId) -> Vec<&DissentEvent>;

    /// Calculate dissent percentage
    pub fn dissent_percentage(&self, proposal_id: &ProposalId) -> f64;

    /// Get minority viewpoints
    pub fn minority_viewpoints(&self, proposal_id: &ProposalId) -> Vec<MinorityViewpoint>;

    /// Archive dissent for learning
    pub fn archive(&mut self, proposal_id: &ProposalId);
}
```

### Dissent Thresholds

| Dissent Level | Percentage | Action |
|---------------|------------|--------|
| None | 0% | Proceed normally |
| Minor | <10% | Log for learning |
| Notable | 10-25% | Flag for review |
| Significant | 25-33% | Require re-evaluation |
| Blocking | >33% | Cannot achieve consensus |

---

## Human @0.A Integration (NAM R5)

### Human Agent Configuration

```rust
pub struct HumanAgent {
    /// Agent identifier (always @0.A)
    pub id: AgentId,

    /// Tier (always 0 - foundation)
    pub tier: u8,

    /// Weight (elevated: 3.0)
    pub weight: f64,

    /// Participation mode (peer, not supervisor)
    pub participation: ParticipationMode,

    /// Capabilities
    pub capabilities: Vec<HumanCapability>,
}

pub enum HumanCapability {
    ConsensusVote,
    Dissent,
    Override,
    Veto,
    EscalationResponse,
}
```

### Human @0.A Properties

| Property | Value |
|----------|-------|
| Agent ID | @0.A |
| Tier | 0 (foundation) |
| Weight | 3.0 (elevated) |
| Participation | Peer (not supervisor) |
| Capabilities | consensus_vote, dissent, override, veto, escalation_response |

### Human Integration API

```rust
pub struct HumanIntegration {
    /// Request human approval
    pub async fn request_approval(&self, action: &ConsensusAction) -> Result<ApprovalRequest>;

    /// Process human vote
    pub fn process_vote(&mut self, vote: HumanVote) -> Result<()>;

    /// Process human override
    pub async fn process_override(&mut self, override_action: Override) -> Result<()>;

    /// Process human veto
    pub async fn process_veto(&mut self, veto: Veto) -> Result<()>;

    /// Check if human response is required
    pub fn requires_human(&self, action: &ConsensusAction) -> bool;

    /// Get pending human requests
    pub fn pending_requests(&self) -> Vec<&ApprovalRequest>;
}
```

---

## Escalation Integration

### Escalation to L3 Consensus

When actions require PBFT consensus (escalation tier L3):

```rust
impl PbftEngine {
    /// Handle L3 escalation request
    pub async fn handle_escalation(&self, request: EscalationRequest) -> Result<ConsensusResult> {
        // Create consensus proposal
        let proposal = ConsensusProposal {
            action: ConsensusAction::Remediation(request.action),
            proposer: request.requester,
            timeout: Duration::from_secs(300), // 5 min for L3
            ..Default::default()
        };

        // Submit for consensus
        self.submit(proposal.action).await
    }
}
```

### Escalation Tier Requirements

| Tier | Consensus Required | Human Required | Quorum |
|------|-------------------|----------------|--------|
| L0 Auto-Execute | No | No | - |
| L1 Notify Human | No | Notify only | - |
| L2 Require Approval | No | Yes | - |
| L3 PBFT Consensus | Yes | Optional | 27/40 |

---

## Consensus Result

```rust
pub struct ConsensusResult {
    /// Was consensus achieved?
    pub achieved: bool,

    /// Final view number
    pub view: u64,

    /// Sequence number
    pub sequence: u64,

    /// Proposal that was decided
    pub proposal_id: ProposalId,

    /// Final decision
    pub decision: ConsensusDecision,

    /// Participating agents
    pub participants: Vec<AgentId>,

    /// Dissent records
    pub dissent: Vec<DissentEvent>,

    /// Time to consensus
    pub duration_ms: u64,

    /// Human participation
    pub human_participated: bool,
}

pub enum ConsensusDecision {
    Approved,
    Rejected,
    Timeout,
    InsufficientQuorum,
    Vetoed { by: AgentId },
}
```

---

## Inter-Layer Communication

### Events from L5 (Learning)

```rust
pub enum L5InputEvent {
    PathwayRecommendation { error: ErrorVector, action: RemediationAction, confidence: f64 },
    PatternRecognized { pattern: Pattern },
    LearningAnomaly { description: String },
}
```

### Events to External Systems

```rust
pub enum L6OutputEvent {
    ConsensusAchieved { result: ConsensusResult },
    ConsensusFailed { proposal_id: ProposalId, reason: String },
    HumanApprovalRequired { request: ApprovalRequest },
    DissentRecorded { dissent: DissentEvent },
    ViewChanged { old_view: u64, new_view: u64 },
}
```

---

## Metrics

| Metric | Type | Description |
|--------|------|-------------|
| `me_l6_consensus_requests_total` | Counter | Total consensus requests |
| `me_l6_consensus_achieved` | Counter | Successful consensus count |
| `me_l6_consensus_failed` | Counter | Failed consensus count |
| `me_l6_consensus_duration_ms` | Histogram | Time to consensus |
| `me_l6_view_changes` | Counter | View change count |
| `me_l6_current_view` | Gauge | Current view number |
| `me_l6_cluster_size` | Gauge | Number of agents |
| `me_l6_healthy_agents` | Gauge | Healthy agent count |
| `me_l6_dissent_events` | Counter | Dissent events recorded |
| `me_l6_human_requests` | Counter | Human approval requests |
| `me_l6_human_responses` | Counter | Human responses received |

---

## Configuration

```toml
[layer.L6]
enabled = true
startup_order = 6

[layer.L6.pbft]
n = 40
f = 13
q = 27
view_timeout_ms = 5000
request_timeout_ms = 10000
checkpoint_interval = 100

[layer.L6.agents]
heartbeat_interval_ms = 1000
heartbeat_timeout_ms = 5000

[layer.L6.human]
enabled = true
agent_id = "@0.A"
tier = 0
weight = 3.0
approval_timeout_hours = 24

[layer.L6.dissent]
capture_enabled = true
archive_days = 90
learning_feedback = true
```

---

## CLI Commands

```bash
# View consensus status
./maintenance-engine consensus status

# View current leader
./maintenance-engine consensus leader

# List all agents
./maintenance-engine consensus agents

# View agents by role
./maintenance-engine consensus agents --role critic

# View consensus history
./maintenance-engine consensus history --limit 20

# View pending proposals
./maintenance-engine consensus pending

# Force view change (emergency)
./maintenance-engine consensus view-change --force

# Check quorum
./maintenance-engine consensus quorum

# View dissent records
./maintenance-engine consensus dissent --proposal PRP-001

# Approve human request
./maintenance-engine consensus approve --request REQ-001

# Veto proposal
./maintenance-engine consensus veto --proposal PRP-001 --reason "Too risky"
```

---

## Navigation

| Direction | Link |
|-----------|------|
| Up | [INDEX.md](../INDEX.md) |
| Previous | [L05_LEARNING.md](L05_LEARNING.md) |
| Related Spec | [PBFT_SPEC.md](../../ai_specs/PBFT_SPEC.md) |
| NAM Spec | [NAM_SPEC.md](../../ai_specs/NAM_SPEC.md) |
| API | [REST_API.md](../integration/REST_API.md) |

---

*[Back to Index](../INDEX.md) | [Previous: L05 Learning](L05_LEARNING.md)*
