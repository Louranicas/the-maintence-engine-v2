# L6 Consensus — Spec Sheet Index

> **Layer:** L6 Consensus | **Modules:** M31-M36 (6 modules, 7 files)
> **LOC:** ~6,500 (target) | **Tests:** 300+ (target) | **Quality Score:** PENDING
> **Status:** SPECIFIED — awaiting implementation | **Verified:** 2026-03-06

---

## Document Map

| Document | Description | Tokens |
|----------|-------------|--------|
| [L6_CONSENSUS_SPEC.md](L6_CONSENSUS_SPEC.md) | Full layer specification: PBFT protocol, agent roles, view changes, dissent tracking, quorum calculation | ~3K |

---

## Reading Protocol

```
QUICK START:    Read L6_CONSENSUS_SPEC.md (PBFT architecture + agent fleet)
WRITING CODE:   Read the M31-M36 section relevant to the module you're implementing
CROSS-LAYER:    L6 receives L3 escalation tier triggers; coordinates 40-agent CVA-NAM fleet
CONSUMING L6:   PBFT (M31) + Agent (M32) + Quorum (M36) — the consensus triad
```

---

## Module Table

| # | Module | ID | File | Target LOC | Target Tests | Status |
|---|--------|----|------|-----------|-------------|--------|
| 1 | PBFT Manager | M31 | `pbft.rs` | ~800 | 50+ | PENDING |
| 2 | Agent Coordinator | M32 | `agent.rs` | ~1,200 | 50+ | PENDING |
| 3 | Vote Collector | M33 | `voting.rs` | ~700 | 50+ | PENDING |
| 4 | View Change Handler | M34 | `view_change.rs` | ~1,100 | 50+ | PENDING |
| 5 | Dissent Tracker | M35 | `dissent.rs` | ~750 | 50+ | PENDING |
| 6 | Quorum Calculator | M36 | `quorum.rs` | ~1,050 | 50+ | PENDING |
| 7 | Layer Coordinator | — | `mod.rs` | ~500 | 20+ | PENDING |
| | **Subtotal** | | | **~6,100** | **~320** | |

---

## Quick Reference — 5 Traits

| # | Trait | File | Bounds | Methods | Defaults |
|---|-------|------|--------|---------|----------|
| 1 | `PbftOps` | pbft.rs | Send+Sync | 5 | 0 |
| 2 | `AgentCoordinator` | agent.rs | Send+Sync | 5 | 0 |
| 3 | `VoteCollector` | voting.rs | Send+Sync | 4 | 0 |
| 4 | `QuorumOps` | quorum.rs | Send+Sync | 4 | 0 |
| 5 | `TensorContributor` | (all modules) | Send+Sync+Debug | 3 | 0 |

---

## Quick Reference — Tensor Dimensions

| Dim | Name | L6 Contributors | Notes |
|-----|------|-----------------|-------|
| D0 | ServiceId | — | Inherited from L2 |
| D1 | Port | — | — |
| D2 | Tier | — | — |
| D3 | DependencyCount | — | — |
| D4 | AgentCount | M32 (active agent count / 40) | CVA-NAM fleet size |
| D5 | Protocol | — | — |
| D6 | HealthScore | M31 (consensus health: rounds completed / proposed) | PBFT round success rate |
| D7 | Uptime | — | — |
| D8 | Synergy | — | — |
| D9 | Latency | — | — |
| D10 | ErrorRate | — | — |
| D11 | TemporalContext | — | — |

---

## PBFT Configuration

| Parameter | Value | Description |
|-----------|-------|-------------|
| `n` | 40 | Total agents (CVA-NAM fleet) |
| `f` | 13 | Max Byzantine faults (n = 3f + 1) |
| `q` | 27 | Quorum requirement (2f + 1) |
| Leader timeout | 30s | View change trigger |

---

## Agent Fleet — 5 Roles (NAM-05)

| Role | Count | Weight | Focus |
|------|-------|--------|-------|
| VALIDATOR | 20 | 1.0 | Correctness verification |
| EXPLORER | 8 | 0.8 | Alternative detection |
| CRITIC | 6 | 1.2 | Flaw detection (highest weight) |
| INTEGRATOR | 4 | 1.0 | Cross-system impact analysis |
| HISTORIAN | 2 | 0.8 | Precedent matching |
| **Human @0.A** | 1 | 3.0 | Veto capability (NAM R5) |

---

## PBFT Phases

```
1. PRE-PREPARE:  Leader proposes action
2. PREPARE:      Agents validate and vote
3. COMMIT:       Upon 2f+1 (27) prepare votes, commit
4. EXECUTE:      Upon 2f+1 (27) commit votes, execute action
```

---

## View Change Triggers

| Trigger | Condition |
|---------|-----------|
| Leader timeout | No response within 30s |
| Agent request | f+1 (14) agents request view change |
| Byzantine detection | Leader sends contradictory messages |

**Protocol:** View number incremented, new leader = `view_number % n`, pending rounds re-proposed.

---

## Design Constraints

| ID | Constraint | Enforcement |
|----|-----------|-------------|
| C1 | Imports from L1-L5 only | Compile-time module DAG |
| C2 | All trait methods `&self` | Code review |
| C3 | `TensorContributor` on M31 (consensus health), M32 (agent count) | Compile-time |
| C4 | Zero unsafe/unwrap/expect | `#![forbid(unsafe_code)]` + clippy deny |
| NAM R3 | All dissent recorded, never suppressed | M35 enforcement |
| NAM R5 | Human @0.A has veto capability | M32 agent weight 3.0 |

---

## Cross-References

- **Upstream:** [M1 Foundation](../m1-foundation-specs/) | [M2 Services](../m2-services-specs/) | [M3 Core Logic](../m3-core-logic-specs/) | [M4 Integration](../m4-integration-specs/) | [M5 Learning](../m5-learning-specs/)
- **Downstream:** L7 Observer (M37-M39)
- **System:** [SYSTEM_SPEC](../SYSTEM_SPEC.md), [LAYER_SPEC](../LAYER_SPEC.md), [MODULE_MATRIX](../MODULE_MATRIX.md)
- **PBFT:** [PBFT_SPEC](../PBFT_SPEC.md) (full PBFT protocol reference)
- **NAM:** [NAM_SPEC](../NAM_SPEC.md) (R1-R5 compliance, dissent semantics)
- **Tensor:** [TENSOR_SPEC](../TENSOR_SPEC.md)
- **Database:** [DATABASE_SPEC](../DATABASE_SPEC.md) (consensus_tracking.db)
- **Escalation:** [ESCALATION_SPEC](../ESCALATION_SPEC.md) (L3 tier triggers PBFT)
- **Tests:** `tests/l6_consensus_integration.rs` | **Bench:** `benches/pbft_consensus.rs`

---

*L6 Consensus Spec Sheet Index v1.0 | 2026-03-06*
