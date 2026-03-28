# Module M32: Agent Coordinator

> **M32_AGENT_COORDINATOR** | Multi-agent coordination | Layer: L6 Consensus | [Back to Index](INDEX.md)

---

## Navigation

| Direction | Link |
|-----------|------|
| Up | [INDEX.md](INDEX.md) |
| Layer | [L06_CONSENSUS.md](../layers/L06_CONSENSUS.md) |
| Related | [M31_PBFT_MANAGER.md](M31_PBFT_MANAGER.md) |
| Related | [M33_VOTE_COLLECTOR.md](M33_VOTE_COLLECTOR.md) |
| Related | [M34_VIEW_CHANGE_HANDLER.md](M34_VIEW_CHANGE_HANDLER.md) |
| Related | [M35_DISSENT_TRACKER.md](M35_DISSENT_TRACKER.md) |
| Related | [M36_QUORUM_CALCULATOR.md](M36_QUORUM_CALCULATOR.md) |

---

## Module Specification

### Overview

The Agent Coordinator manages the lifecycle and coordination of 40 CVA-NAM heterogeneous agents plus the Human @0.A during PBFT consensus operations. It tracks agent status, manages role-based participation, ensures quorum representation, and coordinates message passing between agents and the consensus manager.

### Module Properties

| Property | Value |
|----------|-------|
| Module ID | M32 |
| Module Name | Agent Coordinator |
| Layer | L6 (Consensus) |
| Version | 1.0.0 |
| Dependencies | M31 (PBFT Manager), M36 (Quorum Calculator) |
| Dependents | M33 (Vote Collector), M34 (View Change Handler), M35 (Dissent Tracker) |

---

## PBFT Configuration

| Parameter | Value | Description |
|-----------|-------|-------------|
| Total Agents | 40 | Main CVA-NAM fleet (plus Human @0.A) |
| Quorum Size | 27 | Minimum agents for consensus |
| Byzantine Threshold | 13 | Maximum tolerable faulty agents |

---

## Agent Fleet Composition

The default agent fleet includes 41 total agents (40 + Human @0.A):

### Role Distribution

| Role | Count | Weight | Purpose | Tier |
|------|-------|--------|---------|------|
| VALIDATOR | 21 | 1.0 | Correctness verification | 1 |
| EXPLORER | 8 | 0.8 | Alternative path detection | 2 |
| CRITIC | 6 | 1.2 | Flaw and risk detection | 3 |
| INTEGRATOR | 4 | 1.0 | Cross-system impact analysis | 4 |
| HISTORIAN | 2 | 0.8 | Precedent and pattern matching | 5 |

**Note:** VALIDATOR count includes Human @0.A (tier 0)

---

## Core Types

### ConsensusAgent Structure

```rust
pub struct ConsensusAgent {
    /// Unique agent identifier
    pub id: String,
    /// Agent's role in consensus (NAM-05)
    pub role: AgentRole,
    /// Vote weight in consensus (1.0 = standard, 1.2 = critic, etc.)
    pub weight: f64,
    /// Tier level (0 = Human @0.A, 1-5 = AI agents)
    pub tier: u8,
    /// Current operational status
    pub status: AgentStatus,
    /// Historical success rate (0.0-1.0)
    pub success_rate: f64,
    /// Most recent heartbeat timestamp
    pub last_heartbeat: Option<std::time::SystemTime>,
}
```

### AgentRole Enumeration (NAM-05)

```rust
pub enum AgentRole {
    /// Validator: verifies correctness and compliance
    Validator,
    /// Explorer: identifies alternative solutions and risks
    Explorer,
    /// Critic: detects flaws and inconsistencies
    Critic,
    /// Integrator: evaluates cross-system impacts
    Integrator,
    /// Historian: matches against precedents and patterns
    Historian,
}
```

### AgentStatus Enumeration

```rust
pub enum AgentStatus {
    /// Agent is idle, ready to participate
    Idle,
    /// Agent is actively participating in consensus
    Active,
    /// Agent is busy with other work, may not respond quickly
    Busy,
    /// Agent has failed and cannot participate
    Failed,
    /// Agent is offline and unreachable
    Offline,
}
```

---

## Human @0.A (NAM R5)

The human agent is registered with special properties:

```rust
pub fn create_human_agent() -> ConsensusAgent {
    ConsensusAgent {
        id: "@0.A".into(),
        role: AgentRole::Validator,  // Can act as any role
        weight: 1.0,
        tier: 0,  // Foundation tier
        status: AgentStatus::Active,
        success_rate: 1.0,  // Always correct
        last_heartbeat: Some(std::time::SystemTime::now()),
    }
}
```

### Human @0.A Properties

| Property | Value | Purpose |
|----------|-------|---------|
| Agent ID | @0.A | Unique identifier |
| Tier | 0 | Foundation (most important) |
| Role | VALIDATOR | Can participate in any consensus |
| Weight | 1.0 | Standard voting power |
| Success Rate | 1.0 | Always correct decisions |
| Status | Active | Always available |
| Last Heartbeat | Current | Continuously monitored |

---

## Default Agent Fleet

The `default_agent_fleet()` function creates all 41 agents with proper distribution:

### Agent Assignment Pattern

- **Agent @0.A**: Tier 0, VALIDATOR, Human
- **Agents 01-20**: Tier 1, VALIDATOR (20 agents)
- **Agents 21-28**: Tier 2, EXPLORER (8 agents)
- **Agents 29-34**: Tier 3, CRITIC (6 agents)
- **Agents 35-38**: Tier 4, INTEGRATOR (4 agents)
- **Agents 39-40**: Tier 5, HISTORIAN (2 agents)

### Tier Significance

| Tier | Meaning | Examples | Responsibilities |
|------|---------|----------|------------------|
| 0 | Foundation | Human @0.A | Peer consensus participant |
| 1 | Primary | Validators | Core consensus verification |
| 2 | Secondary | Explorers | Risk and alternative analysis |
| 3 | Specialized | Critics | Flaw detection and validation |
| 4 | Integration | Integrators | System-wide impact assessment |
| 5 | Historical | Historians | Precedent and pattern matching |

---

## API Functions

### Agent Fleet Management

#### create_human_agent

```rust
pub fn create_human_agent() -> ConsensusAgent
```

Creates the Human @0.A agent with tier 0 and standard voting weight.

**Returns:** ConsensusAgent configured as Human @0.A

**Properties:**
- ID: "@0.A"
- Tier: 0 (foundation)
- Role: VALIDATOR
- Weight: 1.0
- Status: Active
- Success Rate: 1.0

#### default_agent_fleet

```rust
pub fn default_agent_fleet() -> Vec<ConsensusAgent>
```

Creates all 41 consensus agents (40 + Human @0.A) with proper role distribution.

**Returns:** Vector of 41 ConsensusAgent structures

**Fleet Composition:**
- 1 Human (@0.A, tier 0)
- 20 Validators (tier 1)
- 8 Explorers (tier 2)
- 6 Critics (tier 3)
- 4 Integrators (tier 4)
- 2 Historians (tier 5)

---

## Agent Coordination Workflow

### Initialization Phase

```
1. CREATE FLEET
   └─> default_agent_fleet() called
       ├─> Human @0.A created with tier 0
       ├─> Validators assigned (1.0 weight)
       ├─> Explorers assigned (0.8 weight)
       ├─> Critics assigned (1.2 weight)
       ├─> Integrators assigned (1.0 weight)
       └─> Historians assigned (0.8 weight)

2. STATUS INITIALIZATION
   └─> All agents initialized to Idle
       ├─> Heartbeat timestamps cleared (except Human)
       ├─> Success rates initialized to 0.5
       └─> Ready for consensus participation
```

### Consensus Participation Phase

```
1. PROPOSAL ANNOUNCEMENT
   └─> M31 broadcasts proposal to all agents
       └─> M32 routes to active agents
           ├─> Filter by status != Failed/Offline
           ├─> Respect agent role requirements
           └─> Track role distribution

2. AGENT EVALUATION
   └─> Each agent evaluates proposal
       ├─> Validators: Verify correctness
       ├─> Explorers: Identify alternatives
       ├─> Critics: Detect flaws
       ├─> Integrators: Assess impacts
       └─> Historians: Match precedents

3. VOTE PREPARATION
   └─> Agents prepare votes with context
       ├─> Attach role information
       ├─> Include weight multiplier
       ├─> Add reasoning (optional)
       └─> Timestamp vote

4. VOTE SUBMISSION
   └─> Agents submit votes to M33
       ├─> M32 validates submission
       ├─> Update agent status
       └─> Record last_heartbeat
```

### Status Management

```
1. HEARTBEAT TRACKING
   └─> Update last_heartbeat on agent activity
       ├─> Vote submission
       ├─> Message receipt
       └─> Status query

2. FAILURE DETECTION
   └─> Mark agents as Failed/Offline
       ├─> Heartbeat timeout > 5s
       ├─> Vote submission error
       ├─> Network unreachability
       └─> Status update propagated to M31

3. RECOVERY PROTOCOLS
   └─> Attempt agent recovery
       ├─> Ping offline agent
       ├─> Reset to Idle if responsive
       ├─> Update success_rate
       └─> Resume consensus participation
```

---

## Role-Based Voting Patterns

### VALIDATOR Pattern (20 agents)

**Role:** Verify proposal correctness and compliance

**Vote Criteria:**
- Action complies with system policies? ✓
- Action matches expected behavior? ✓
- Configuration valid? ✓
- All prerequisites met? ✓

**Weight Impact:** 1.0x (standard)

### EXPLORER Pattern (8 agents)

**Role:** Identify alternative solutions and risks

**Vote Criteria:**
- Are there safer alternatives? ✓
- What risks exist? ✓
- Could action cause cascades? ✓
- Better phased approach? ✓

**Weight Impact:** 0.8x (slightly reduced)

### CRITIC Pattern (6 agents)

**Role:** Detect flaws and inconsistencies (NAM-05 requires ≥1 approval)

**Vote Criteria:**
- Are there logical flaws? ✓
- Insufficient error handling? ✓
- Race conditions possible? ✓
- Reversibility ensured? ✓

**Weight Impact:** 1.2x (enhanced)

### INTEGRATOR Pattern (4 agents)

**Role:** Evaluate cross-system impacts (NAM-05 requires ≥1 approval)

**Vote Criteria:**
- Impact on other services? ✓
- Dependency violations? ✓
- Timing conflicts? ✓
- Coherence with other changes? ✓

**Weight Impact:** 1.0x (standard)

### HISTORIAN Pattern (2 agents)

**Role:** Match against precedents and patterns

**Vote Criteria:**
- Similar situation in past? ✓
- What was outcome? ✓
- Changed conditions? ✓
- Applicable lessons? ✓

**Weight Impact:** 0.8x (slightly reduced)

---

## Enhanced Consensus Requirements (NAM-05)

For critical actions, M32 enforces:

1. **At least 1 CRITIC approval required**
   - Ensures flaw detection step completed
   - Detects potential logical errors
   - Risk assessment conducted

2. **At least 1 INTEGRATOR approval required**
   - Ensures cross-system impact analysis
   - Prevents isolated decision-making
   - System coherence verified

**Enforcement:** M31 calls `enhanced_consensus_check()` on vote set

---

## Agent Communication Protocol

### Message Types

| Type | Source | Target | Purpose |
|------|--------|--------|---------|
| Proposal | M31 | All agents | Broadcast consensus question |
| Evaluate | Agent | M32 | Internal evaluation request |
| Vote | Agent | M33 | Submit vote on proposal |
| Heartbeat | Agent | M32 | Periodic status ping |
| Status | M32 | M31 | Fleet status update |

---

## Performance Characteristics

| Operation | Time | Notes |
|-----------|------|-------|
| Fleet creation | <5ms | Allocates 41 agents |
| Agent lookup | <1ms | HashMap or linear scan |
| Status update | <1ms | In-memory state change |
| Vote aggregation | <10ms | Per-agent weight calculation |
| Heartbeat check | <50ms | Check all agents |

---

## Related Modules

| Module | Relationship | Purpose |
|--------|--------------|---------|
| M31 | Dependency | PBFT consensus orchestration |
| M33 | Dependent | Vote collection and aggregation |
| M34 | Dependent | Leader election and view changes |
| M35 | Dependent | Dissent event tracking |
| M36 | Integration | Quorum threshold management |

---

## Testing

Key test cases include:

```rust
#[test]
fn test_human_agent()       // Verify @0.A properties (tier 0)
#[test]
fn test_agent_fleet()       // Validate 41 agents, 5 roles
#[test]
fn test_role_distribution() // Check role counts and weights
#[test]
fn test_status_transitions()// Verify status state machine
#[test]
fn test_enhanced_consensus()// Test CRITIC/INTEGRATOR requirements
```

---

## Version History

| Version | Date | Changes |
|---------|------|---------|
| 1.0.0 | 2026-01-28 | Initial implementation |

---

*The Maintenance Engine v1.0.0 | M32: Agent Coordinator*
*Last Updated: 2026-01-28*
