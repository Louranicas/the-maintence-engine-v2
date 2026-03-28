---
tags: [layer/L6, progressive-disclosure/L2, status/pending]
---

# L6: Consensus Layer

> **Status:** PENDING | **Target LOC:** ~6,500 | **Target Tests:** 300+

## Purpose

PBFT Byzantine fault-tolerant consensus for critical decisions. When ME V2 needs to take high-impact actions (L3 escalation tier), it requires agreement from a quorum of agents before proceeding.

## Modules

| ID | Module | File | Target LOC | Role |
|----|--------|------|-----------|------|
| M31 | PBFT Manager | `pbft.rs` | 800+ | Consensus protocol orchestration |
| M32 | Agent Coordinator | `agent.rs` | 1,200+ | 40-agent fleet management |
| M33 | Vote Collector | `voting.rs` | 700+ | Vote collection and tallying |
| M34 | View Change Handler | `view_change.rs` | 1,100+ | Leader election on failure |
| M35 | Dissent Tracker | `dissent.rs` | 800+ | NAM R3 dissent capture |
| M36 | Quorum Calculator | `quorum.rs` | 1,100+ | Dynamic quorum computation |

## PBFT Constants

```
PBFT_N = 40    // Total agents
PBFT_F = 13    // Byzantine tolerance (can tolerate 13 malicious/failed)
PBFT_Q = 27    // Quorum (2f + 1)
```

## Template Source

ME v1 `m6_consensus/` (6,051 LOC). Also extracted to `synops-tools/synops-consensus` crate.

---

Full spec: `../ai_specs/m6-consensus-specs/L6_CONSENSUS_SPEC.md` | Patterns: `../ai_specs/patterns/CONSENSUS_PATTERNS.md`
See [[HOME]] | Prev: [[L5 — Learning Layer]] | Next: [[L7 — Observer Layer]]
