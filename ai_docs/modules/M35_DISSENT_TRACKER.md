# Module M35: Dissent Tracker

> **M35_DISSENT_TRACKER** | Disagreement capture (NAM R3) | Layer: L6 Consensus | [Back to Index](INDEX.md)

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
| Related | [M36_QUORUM_CALCULATOR.md](M36_QUORUM_CALCULATOR.md) |
| Learning | [M25_HEBBIAN_MANAGER.md](M25_HEBBIAN_MANAGER.md) |
| Learning | [M26_STDP_PROCESSOR.md](M26_STDP_PROCESSOR.md) |

---

## Module Specification

### Overview

The Dissent Tracker captures and analyzes minority disagreement during consensus voting, implementing the NAM R3 (DissentCapture) requirement. It records all dissenting votes, evaluates dissent value post-hoc, and feeds disagreement patterns into the learning system (Layer 5) for agent improvement and consensus refinement. Non-anthropocentric by design, it treats agent disagreement as a resource for system improvement.

### Module Properties

| Property | Value |
|----------|-------|
| Module ID | M35 |
| Module Name | Dissent Tracker |
| Layer | L6 (Consensus) |
| Version | 1.0.0 |
| NAM Requirement | R3 - DissentCapture |
| Dependencies | M31 (PBFT Manager), M33 (Vote Collector) |
| Dependents | M25-M30 (Learning Layer), Episode Recording |

---

## NAM R3: Dissent Capture Requirement

From NAM Framework specification:

**DissentCapture (R3):** The system must capture all minority opinion and dissenting votes during consensus processes, recording:
- Which agents dissented
- Why they disagreed
- What their roles were (CRITIC, EXPLORER, etc.)
- Whether the dissent was ultimately valuable

**Implementation Target:** 85% compliance

**Purpose:** Feed agent improvement and prevent groupthink

---

## Core Types

### DissentEvent Structure

```rust
pub struct DissentEvent {
    /// Unique dissent event identifier
    pub id: String,
    /// The proposed action that triggered disagreement
    pub proposed_action: String,
    /// ID of agent who disagreed
    pub dissenting_agent: String,
    /// Reason for dissenting (provided by agent or inferred)
    pub reason: String,
    /// Outcome of the proposal after decision was made
    pub outcome: Option<String>,
    /// Post-hoc evaluation: was dissent valuable?
    pub was_valuable: Option<bool>,
    /// Timestamp of dissent event
    pub timestamp: std::time::SystemTime,
}
```

### DissentEvent Fields

| Field | Type | Purpose | Example |
|-------|------|---------|---------|
| id | String | Unique dissent identifier | "dissent-2026-01-28-001" |
| proposed_action | String | Action that caused disagreement | "ServiceTermination on port 8080" |
| dissenting_agent | String | Agent that disagreed | "agent-29" (CRITIC) |
| reason | String | Why agent disagreed | "Insufficient error recovery" |
| outcome | Option<String> | Actual result after decision | "Proposal rejected, retried" |
| was_valuable | Option<bool> | Did dissent help? | true if proposal later failed |
| timestamp | SystemTime | When dissent recorded | Current time |

---

## Dissent Capture Patterns

### Pattern 1: Reject Vote

**Type:** Agent votes Reject on proposal

**Capture:**
```
Agent: agent-29 (CRITIC)
Vote: Reject
Proposal: DatabaseMigration
Reason: "Migration plan missing fallback"
Dissent Event: Recorded immediately
```

**Learning Potential:**
- If proposal later rejected: dissent was valuable ✓
- If proposal succeeds: dissent may indicate conservative bias
- If dissent reasons match actual failures: agent learning improved

### Pattern 2: Abstain Vote

**Type:** Agent votes Abstain (uncertain)

**Capture:**
```
Agent: agent-21 (EXPLORER)
Vote: Abstain
Proposal: CredentialRotation
Reason: "Unknown impact on dependent services"
Dissent Event: Recorded as uncertain opinion
```

**Learning Potential:**
- Abstention may indicate missing information
- Learning system can improve information availability
- Explorer role appropriate for uncertainty

### Pattern 3: Role-Based Dissent

**Type:** Entire role disagrees (e.g., CRITIC consensus)

**Capture:**
```
Proposal: ConfigRollback
CRITIC votes: 4 Approve, 2 Reject
Dissent: 2 CRITICs disagreed with majority
Focus: Why did minority CRITICs see flaws others missed?
```

**Learning Potential:**
- Identify knowledge gaps in critic evaluation
- Surface edge cases for system improvement
- Improve proposal framing for critics

### Pattern 4: Minority Cascade

**Type:** Multiple agents share similar dissent

**Capture:**
```
Proposal: CascadeRestart
4 agents (agents 21, 22, 23, 24) all vote Reject
Reason: All cite "Timing conflicts with scheduled maintenance"
Dissent: Grouped minority
```

**Learning Potential:**
- Coordinated dissent may indicate systematic issue
- Propose better timing in future
- Improve proposal scheduling logic

### Pattern 5: Enhanced Consensus Failure

**Type:** CRITIC or INTEGRATOR approval missing

**Capture:**
```
Proposal: ServiceTermination
Prepare votes: 30 Approve, 10 Reject
Enhanced check: CRITIC approval found, INTEGRATOR missing
Dissent: Entire INTEGRATOR cohort disagreed
Reason: Insufficient cross-system impact analysis
```

**Learning Potential:**
- Identify when system scope underestimated
- Improve INTEGRATOR involvement earlier
- Refactor proposal for broader impact consideration

---

## Dissent Recording Workflow

```
1. VOTE COLLECTION (M33)
   ├─> Agents submit votes on proposal
   ├─> Each vote includes: agent_id, role, vote, reason
   └─> M33 aggregates votes

2. DISSENT IDENTIFICATION
   ├─> Identify all non-Approve votes
   ├─> Extract agent_id from each dissent
   ├─> Note vote type: Reject vs. Abstain
   ├─> Extract reason field (if provided)
   └─> Group by dissenting agent

3. DISSENT EVENT CREATION
   ├─> For each dissent vote:
   │   ├─> Generate unique dissent ID
   │   ├─> Record proposed_action
   │   ├─> Record dissenting_agent
   │   ├─> Extract reason from vote
   │   ├─> Set outcome = None (TBD)
   │   ├─> Set was_valuable = None (TBD)
   │   └─> Timestamp current time

4. IMMEDIATE STORAGE
   ├─> Store in consensus_tracking.db
   ├─> Table: dissent_events
   ├─> Make immediately queryable
   └─> Ready for real-time analysis

5. ROLE-BASED GROUPING
   ├─> Group dissentions by agent role
   ├─> Analyze CRITIC consensus
   ├─> Analyze EXPLORER consensus
   ├─> Analyze INTEGRATOR consensus
   ├─> Identify patterns
   └─> Feed patterns to M35 aggregation

6. ENHANCED CONSENSUS TRACKING
   ├─> If enhanced_consensus_check fails:
   │   ├─> Record missing role (CRITIC or INTEGRATOR)
   │   ├─> Mark dissent as "critical role missing"
   │   ├─> Trigger view change (M34)
   │   └─> Log for pattern analysis

7. OUTCOME RECORDING
   ├─> After proposal finalized:
   │   ├─> Record actual outcome
   │   ├─> Update dissent_events.outcome
   │   ├─> Set completion timestamp
   │   └─> Ready for evaluation

8. DISSENT EVALUATION
   ├─> Post-hoc analysis (hours/days later):
   │   ├─> If action succeeded: was dissent valid concern?
   │   ├─> If action failed: did dissent predict failure?
   │   ├─> Update was_valuable = true/false
   │   └─> Feed insights to learning system

9. LEARNING SYSTEM INTEGRATION
   └─> M25-M30 (Learning Layer)
       ├─> Read dissent_events
       ├─> Analyze dissenting agent patterns
       ├─> Update agent success_rate
       ├─> Improve agent role fitness
       ├─> Record episodic memory (M28)
       └─> Hebbian pathway strengthening (M26)
```

---

## Dissent Event Lifecycle

### Creation Phase

```
Event Created: 2026-01-28 10:15:00
├─> id: "dissent-2026-01-28-001"
├─> proposed_action: "DatabaseMigration v5->v6"
├─> dissenting_agent: "agent-29" (CRITIC)
├─> reason: "Migration plan lacks rollback validation"
├─> outcome: None (proposal not yet finalized)
├─> was_valuable: None (not yet evaluated)
└─> timestamp: 2026-01-28 10:15:00
```

### Decision Phase

```
Proposal finalized: 2026-01-28 11:22:00
├─> Decision: APPROVED (27/40 votes despite dissent)
├─> Dissent votes: 13 total
│   ├─> 3 from CRITICS
│   ├─> 5 from EXPLORERS
│   └─> 5 from VALIDATORS
└─> All dissent events updated with outcome
```

### Evaluation Phase

```
Post-hoc evaluation: 2026-01-29 14:00:00
├─> Action result: DatabaseMigration succeeded
├─> No failures or rollbacks needed
├─> Evaluation: was_valuable = false
│   └─> Dissent was overly conservative
├─> Update dissent_events.was_valuable = false
└─> Use for agent behavioral adjustment
```

### Learning Phase

```
M26 STDP Learning processes dissent event:
├─> Identify agent-29 (CRITIC who dissented)
├─> Check: was dissent valuable? NO
├─> Adjust: Lower LTP for "conservative voting"
├─> Adjust: Reduce weight slightly for similar scenarios
├─> Update: agent-29 success_rate
└─> Result: Agent learns to trust process more
```

---

## Dissent Categories

### By Vote Type

| Type | Meaning | Recording |
|------|---------|-----------|
| Reject | Agent explicitly votes against | Recorded as strong dissent |
| Abstain | Agent uncertain/neutral | Recorded as weak dissent |
| Approve | Agent supports proposal | Not recorded as dissent |

### By Agent Role

| Role | Dissent Type | Significance |
|------|--------------|--------------|
| VALIDATOR | General disagreement | Indicates correctness concern |
| EXPLORER | Risk concern | Indicates alternative problems |
| CRITIC | Flaw detection | Indicates logical flaws found |
| INTEGRATOR | Impact concern | Indicates system-wide risk |
| HISTORIAN | Precedent mismatch | Indicates pattern violation |

### By Outcome (Post-Hoc)

| Outcome | Classification | Learning Value |
|---------|-----------------|-----------------|
| Valuable | Dissent was correct | High learning value |
| Incorrect | Dissent was wrong | Calibration learning |
| Neutral | Outcome same either way | Low learning value |
| Partially Correct | Dissent partially right | Refined learning |

---

## Dissent Analysis

### Valuable Dissent Indicators

Dissent is valuable when:
1. **Prediction:** Agent predicted a later-discovered problem
2. **Prevention:** Early action prevented the dissented-upon problem
3. **Efficiency:** Alternative suggested by dissent was more efficient
4. **Safety:** Risk identified by dissent actually manifested

### Dissent Evaluation Queries

```sql
-- Find valuable dissent (predictions that came true)
SELECT dissent_agent, reason, COUNT(*) as valuable_dissents
FROM dissent_events
WHERE was_valuable = true
GROUP BY dissent_agent
ORDER BY valuable_dissents DESC;

-- Find consistent dissenters
SELECT dissent_agent, role, COUNT(*) as dissent_count
FROM dissent_events
WHERE outcome IS NOT NULL
GROUP BY dissent_agent
ORDER BY dissent_count DESC;

-- Find dissent patterns by role
SELECT dissent_agent, role, reason, COUNT(*) as count
FROM dissent_events
WHERE was_valuable = true
GROUP BY role, reason
ORDER BY count DESC;

-- Find dissent that prevented failures
SELECT COUNT(*) as prevented_failures
FROM dissent_events
WHERE was_valuable = true
AND outcome LIKE '%prevented%';
```

---

## Integration with Learning Layer

### M25-M26 Integration

M35 feeds dissent events to learning modules:

```
M35 DissentEvent
  ├─> dissenting_agent: "agent-29"
  ├─> was_valuable: true
  ├─> reason: "Identified missing error handling"
  └─> M26 STDP Processor
      ├─> Long-Term Potentiation (LTP)
      ├─> Strengthen pathway: agent-29 → better CRITIC decisions
      ├─> Increase weight for similar scenarios
      └─> Update agent-29 success_rate
```

### M28 Integration (Episodic Memory)

M35 records episodes for learning:

```
Episode: "DatabaseMigration with dissent"
├─> Context: Proposal with CRITIC dissent
├─> Action: Approved despite concerns
├─> Outcome: Succeeded (dissent was incorrect)
├─> Learning: CRITICS can be overcautious
└─> Retain for pattern matching
```

### M29 Integration (Pattern Recognition)

M35 enables pattern discovery:

```
Pattern: "CRITIC dissents on timing"
├─> Frequency: 12 instances
├─> Success rate: 25% valuable
├─> Interpretation: CRITICs often miss timing context
├─> Action: Improve CRITIC domain knowledge
└─> Adjust: Weight slightly down for timing concerns
```

---

## NAM R3 Compliance

The Dissent Tracker implements NAM R3 through:

1. **Capture:** All dissent votes recorded immediately
2. **Analysis:** Dissent patterns extracted and analyzed
3. **Evaluation:** Post-hoc value assessment conducted
4. **Learning:** Feedback to agent improvement systems
5. **Transparency:** All dissent queryable and auditable

**Compliance Metrics:**
- % of dissent events recorded: Target 95%+
- % of outcomes evaluated: Target 85%+
- % valuable dissent identified: Target > 30%

---

## Performance Characteristics

| Operation | Time | Notes |
|-----------|------|-------|
| Dissent event creation | <2ms | Struct instantiation |
| Batch recording (40 votes) | <50ms | Database insert batch |
| Role grouping | <10ms | Hash aggregation |
| Outcome update | <5ms | Database update |
| Pattern analysis | <100ms | Linear scan |

---

## Related Modules

| Module | Relationship | Purpose |
|--------|--------------|---------|
| M31 | Dependency | Proposal context |
| M33 | Dependency | Vote source data |
| M34 | Dependency | View change tracking |
| M25-M30 | Dependent | Learning system feedback |
| M28 | Dependent | Episode recording |

---

## Database Schema

### dissent_events Table

```sql
CREATE TABLE dissent_events (
    id TEXT PRIMARY KEY,
    proposed_action TEXT NOT NULL,
    dissenting_agent TEXT NOT NULL,
    reason TEXT,
    outcome TEXT,
    was_valuable BOOLEAN,
    timestamp DATETIME NOT NULL,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_dissent_agent ON dissent_events(dissenting_agent);
CREATE INDEX idx_dissent_valuable ON dissent_events(was_valuable);
CREATE INDEX idx_dissent_timestamp ON dissent_events(timestamp);
```

---

## Testing

Key test cases:

```rust
#[test]
fn test_dissent_event_creation()    // Verify structure
#[test]
fn test_dissent_recording()         // Test vote-to-dissent mapping
#[test]
fn test_role_based_dissent()        // Test CRITIC/INTEGRATOR recording
#[test]
fn test_outcome_recording()         // Test post-hoc evaluation
#[test]
fn test_valuable_dissent_detection()// Test success prediction
#[test]
fn test_pattern_grouping()          // Test dissent pattern analysis
```

---

## Version History

| Version | Date | Changes |
|---------|------|---------|
| 1.0.0 | 2026-01-28 | Initial implementation (NAM R3) |

---

*The Maintenance Engine v1.0.0 | M35: Dissent Tracker (NAM R3)*
*Last Updated: 2026-01-28*
