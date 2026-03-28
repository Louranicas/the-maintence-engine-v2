---
tags: [reference/exemplars, progressive-disclosure/L2]
aliases: [exemplars, code-examples]
---

# Rust Exemplars — Copy-Adaptable Code for L3-L8

> 12 production-tested code blocks from ME v1 (56K LOC, 2,327 tests, 0 clippy warnings)

## Exemplar Index

| ID | Pattern | Layer | Module | Key Takeaway |
|----|---------|-------|--------|-------------|
| E1 | Confidence FMA | L3 | M15 | FMA chain for float precision |
| E2 | Escalation tiers | L3 | M14 | Critical→L3, high conf→L0 |
| E3 | PBFT phases | L6 | M31 | Phase machine + quorum checks |
| E4 | Circuit breaker FSM | L2 | M12 | 3-state with timeout + signals |
| E5 | Hebbian interior mut | L5 | M25 | Multiple RwLocks, separate concerns |
| E6 | Health threshold FSM | L2 | M10 | Consecutive count → state change |
| E7 | TensorContributor | L2 | M10 | Coverage bitmap + D6/D10 mapping |
| E8 | Bounded event bus | L4 | M23 | Channel capacity 1000 |
| E9 | Agent fleet | L6 | M32 | 40+1 agents, 5 roles, human peer |
| E10 | Observer bus | L7 | — | Event-driven pub/sub for L7 |
| E11 | Lifecycle FSM | L2 | M11 | `matches!` for valid transitions |
| E12 | Test helper factory | L2 | All | Builder defaults, populated fixtures |

## Per-Layer Relevance

**Building L3?** Read E1 (confidence), E2 (escalation)
**Building L4?** Read E8 (event bus)
**Building L5?** Read E5 (Hebbian interior mut)
**Building L6?** Read E3 (PBFT phases), E9 (agent fleet)
**Building L7?** Read E10 (observer bus)
**Building L8?** Read [[Nexus Exemplars]]

---

**Full reference:** `ai_docs/RUST_EXEMPLARS.md` (with complete code blocks)

See [[Gold Standard Patterns]] | [[Nexus Exemplars]]
