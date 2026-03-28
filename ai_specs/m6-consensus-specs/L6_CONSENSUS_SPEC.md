# L6: Consensus Layer Specification

> Target: ~6,500 LOC | 6 modules (M31-M36) | 300+ tests

---

## Layer Purpose

The Consensus layer implements Practical Byzantine Fault Tolerance (PBFT) for critical maintenance decisions requiring multi-agent agreement. It coordinates a 40-agent fleet (CVA-NAM) with 5 specialized roles, handles view changes during leader failure, captures dissenting opinions (NAM R3), and computes dynamic quorum thresholds.

---

## PBFT Configuration

```
n = 40    (total agents)
f = 13    (max Byzantine faults: n = 3f + 1)
q = 27    (quorum: 2f + 1)
```

---

## Module Specifications

### M31: PBFT Manager (`pbft.rs`)

**Purpose:** Orchestrate the PBFT consensus protocol phases.

**Target:** ~800 LOC, 50+ tests

**PBFT Phases:**
1. **Pre-Prepare:** Leader proposes action
2. **Prepare:** Agents validate and vote
3. **Commit:** Upon 2f+1 prepare votes, commit
4. **Execute:** Upon 2f+1 commit votes, execute action

**Key Traits:**
```rust
pub trait PbftOps: Send + Sync {
    fn propose(&self, action: ConsensusAction) -> Result<RoundId>;
    fn phase(&self, round: &RoundId) -> Result<PbftPhase>;
    fn is_quorum_reached(&self, round: &RoundId) -> Result<bool>;
    fn execute_committed(&self, round: &RoundId) -> Result<ConsensusOutcome>;
    fn active_rounds(&self) -> Result<Vec<RoundId>>;
}
```

**Database:** `consensus_tracking.db`

---

### M32: Agent Coordinator (`agent.rs`)

**Purpose:** Manage the 40-agent fleet with role-based weighting.

**Target:** ~1,200 LOC, 50+ tests

**Agent Roles:**
| Role | Count | Weight | Focus |
|------|-------|--------|-------|
| VALIDATOR | 20 | 1.0 | Correctness |
| EXPLORER | 8 | 0.8 | Alternatives |
| CRITIC | 6 | 1.2 | Flaw detection |
| INTEGRATOR | 4 | 1.0 | Cross-system |
| HISTORIAN | 2 | 0.8 | Precedent |

**Key Traits:**
```rust
pub trait AgentCoordinator: Send + Sync {
    fn register_agent(&self, agent: AgentConfig) -> Result<AgentId>;
    fn assign_role(&self, id: &AgentId, role: AgentRole) -> Result<()>;
    fn active_agents(&self) -> Result<Vec<AgentInfo>>;
    fn agent_vote(&self, id: &AgentId, round: &RoundId, vote: Vote) -> Result<()>;
    fn weighted_tally(&self, round: &RoundId) -> Result<WeightedTally>;
}
```

**Human @0.A:** Agent `@0.A` with Tier 0, Weight 3.0, peer participation

---

### M33: Vote Collector (`voting.rs`)

**Purpose:** Collect, validate, and tally votes with Byzantine fault detection.

**Target:** ~700 LOC, 50+ tests

**Key Types:**
```rust
pub struct Vote {
    agent_id: AgentId,
    round_id: RoundId,
    phase: PbftPhase,
    decision: VoteDecision,  // Approve, Reject, Abstain
    rationale: Option<String>,
    timestamp: Timestamp,
}
```

**Byzantine Detection:** Duplicate votes, contradictory votes across phases, votes from unregistered agents.

---

### M34: View Change Handler (`view_change.rs`)

**Purpose:** Handle leader failure and view change protocol.

**Target:** ~1,100 LOC, 50+ tests

**Trigger Conditions:**
- Leader timeout (configurable, default 30s)
- f+1 agents request view change
- Byzantine behavior detected in leader

**Protocol:** View number incremented, new leader = `(view_number % n)`, pending rounds re-proposed.

---

### M35: Dissent Tracker (`dissent.rs`)

**Purpose:** Record and analyze dissenting opinions (NAM R3 compliance).

**Target:** ~750 LOC, 50+ tests

**Key Types:**
```rust
pub struct Dissent {
    agent_id: AgentId,
    round_id: RoundId,
    rationale: String,
    evidence: Vec<Evidence>,
    weight: f64,
    timestamp: Timestamp,
}
```

**NAM R3:** Minority opinions are never discarded. They are recorded, analyzed for patterns, and used to improve future confidence calculations.

---

### M36: Quorum Calculator (`quorum.rs`)

**Purpose:** Compute dynamic quorum thresholds based on available agents and severity.

**Target:** ~1,050 LOC, 50+ tests

**Key Traits:**
```rust
pub trait QuorumOps: Send + Sync {
    fn standard_quorum(&self) -> u32;  // 2f + 1 = 27
    fn dynamic_quorum(&self, severity: Severity, available: u32) -> Result<u32>;
    fn is_quorum(&self, votes: u32, total: u32) -> bool;
    fn supermajority(&self) -> u32;    // 3f + 1 = 40 (unanimous)
}
```

---

## Layer Coordinator (`mod.rs`)

**Target:** ~500 LOC, 20+ tests

**Provides:**
- `ConsensusLayer` aggregate struct
- `propose_and_resolve()` — full PBFT round from proposal to execution
- Integration with L3 escalation (L3 tier triggers PBFT)

---

## Design Constraints

- C1: Imports from L1-L5 only
- C2: All trait methods `&self`
- C3: `TensorContributor` on M31 (consensus health), M32 (agent count → D4)
- C4: Zero unsafe/unwrap/expect
- NAM R3: All dissent recorded, never suppressed
- NAM R5: Human @0.A has veto capability

---

## Test Strategy

- Unit tests: 50+ per module
- Integration: `tests/l6_consensus_integration.rs`
- Benchmark: `benches/pbft_consensus.rs`
- Property: quorum always >= 2f+1, Byzantine tolerance holds for f <= 13
