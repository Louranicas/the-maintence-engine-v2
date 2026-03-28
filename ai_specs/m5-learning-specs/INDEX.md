# L5 Learning — Spec Sheet Index

> **Layer:** L5 Learning | **Modules:** M25-M30 (6 modules, 7 files)
> **LOC:** ~7,000 (target) | **Tests:** 300+ (target) | **Quality Score:** PENDING
> **Status:** SPECIFIED — awaiting implementation | **Verified:** 2026-03-06

---

## Document Map

| Document | Description | Tokens |
|----------|-------------|--------|
| [L5_LEARNING_SPEC.md](L5_LEARNING_SPEC.md) | Full layer specification: STDP rules, Hebbian pathways, pattern detection, pruning, consolidation, anti-patterns | ~3K |

---

## Reading Protocol

```
QUICK START:    Read L5_LEARNING_SPEC.md (STDP architecture + learning cycle)
WRITING CODE:   Read the M25-M30 section relevant to the module you're implementing
CROSS-LAYER:    L5 receives feedback from L3, persists to hebbian_pulse.db, integrates with N04 STDP Bridge
CONSUMING L5:   Hebbian (M25) + STDP (M26) + Pruner (M28) — the pathway lifecycle
```

---

## Module Table

| # | Module | ID | File | Target LOC | Target Tests | Status |
|---|--------|----|------|-----------|-------------|--------|
| 1 | Hebbian Manager | M25 | `hebbian.rs` | ~850 | 50+ | PENDING |
| 2 | STDP Processor | M26 | `stdp.rs` | ~700 | 50+ | PENDING |
| 3 | Pattern Recognizer | M27 | `pattern.rs` | ~950 | 50+ | PENDING |
| 4 | Pathway Pruner | M28 | `pruner.rs` | ~1,300 | 50+ | PENDING |
| 5 | Memory Consolidator | M29 | `consolidator.rs` | ~1,500 | 50+ | PENDING |
| 6 | Anti-Pattern Detector | M30 | `antipattern.rs` | ~800 | 50+ | PENDING |
| 7 | Layer Coordinator | — | `mod.rs` | ~400 | 20+ | PENDING |
| | **Subtotal** | | | **~6,500** | **~320** | |

---

## Quick Reference — 5 Traits

| # | Trait | File | Bounds | Methods | Defaults |
|---|-------|------|--------|---------|----------|
| 1 | `HebbianOps` | hebbian.rs | Send+Sync | 7 | 0 |
| 2 | `StdpOps` | stdp.rs | Send+Sync | 4 | 0 |
| 3 | `PatternRecognition` | pattern.rs | Send+Sync | 4 | 0 |
| 4 | `PruningOps` | pruner.rs | Send+Sync | 3 | 0 |
| 5 | `TensorContributor` | (all modules) | Send+Sync+Debug | 3 | 0 |

---

## Quick Reference — Tensor Dimensions

| Dim | Name | L5 Contributors | Notes |
|-----|------|-----------------|-------|
| D0 | ServiceId | — | Inherited from L2 |
| D1 | Port | — | — |
| D2 | Tier | — | — |
| D3 | DependencyCount | — | — |
| D4 | AgentCount | — | — |
| D5 | Protocol | — | — |
| D6 | HealthScore | — | — |
| D7 | Uptime | — | — |
| D8 | Synergy | M25 (pathway strength as synergy) | Hebbian pathway weight aggregation |
| D9 | Latency | — | — |
| D10 | ErrorRate | — | — |
| D11 | TemporalContext | — | — |

---

## STDP Parameters

| Parameter | Value | Description |
|-----------|-------|-------------|
| `ltp_rate` | 0.1 | Long-Term Potentiation rate |
| `ltd_rate` | 0.05 | Long-Term Depression rate |
| `window_ms` | 100 | Timing window (ms) |
| `decay_rate` | 0.1 | Weight decay rate (HRS-001 corrected) |
| `healthy_ratio` | 2.0 - 4.0 | Healthy LTP:LTD balance range |
| `co_activation_delta` | 0.05 | Per-call STDP increment (C12) |

---

## Pruning Rules

| Rule | Threshold | Action |
|------|-----------|--------|
| Weight decay | < 0.1 after decay | Prune pathway |
| Inactivity | > 24h no activation | Candidate for pruning |
| LTD dominance | LTD:LTP > 3:1 | Weaken further |
| Pathway cap | > 10,000 total | Prune weakest |

---

## Detectable Anti-Patterns

| Pattern | Detection | Response |
|---------|-----------|----------|
| Restart loops | > 3 restarts in 5 min | LTD inhibition |
| Cascade amplification | Remediation widens failure | LTD on causal pathway |
| Confidence drift | Confidence rises without outcome improvement | Weight reset |
| Weight explosion | Unbounded growth (HRS-001) | Homeostatic clamp |

---

## Design Constraints

| ID | Constraint | Enforcement |
|----|-----------|-------------|
| C1 | Imports from L1-L4 only | Compile-time module DAG |
| C2 | All trait methods `&self` | Code review |
| C4 | Zero unsafe/unwrap/expect | `#![forbid(unsafe_code)]` + clippy deny |
| C5 | `Timestamp` + `Duration` only | Grep + clippy |
| C12 | All pathway updates record STDP co-activation | Architecture |
| HRS-001 | `decay_rate` = 0.1, never 0.001 | Compile-time constant + test |

---

## Cross-References

- **Upstream:** [M1 Foundation](../m1-foundation-specs/) | [M2 Services](../m2-services-specs/) | [M3 Core Logic](../m3-core-logic-specs/) | [M4 Integration](../m4-integration-specs/)
- **Downstream:** L6 Consensus (M31-M36), L7 Observer (M37-M39)
- **System:** [SYSTEM_SPEC](../SYSTEM_SPEC.md), [LAYER_SPEC](../LAYER_SPEC.md), [MODULE_MATRIX](../MODULE_MATRIX.md)
- **Patterns:** [OBSERVER](../patterns/OBSERVER.md), [REPOSITORY](../patterns/REPOSITORY.md)
- **STDP:** [STDP_SPEC](../STDP_SPEC.md) (full STDP parameter reference)
- **Tensor:** [TENSOR_SPEC](../TENSOR_SPEC.md)
- **Database:** [DATABASE_SPEC](../DATABASE_SPEC.md) (hebbian_pulse.db, episodic_memory.db, tensor_memory.db)
- **Nexus:** [N04 STDP Bridge](../nexus-specs/N04_STDP_BRIDGE.md) (cross-session persistence)
- **Tests:** `tests/l5_learning_integration.rs` | **Bench:** `benches/hebbian_learning.rs`

---

*L5 Learning Spec Sheet Index v1.0 | 2026-03-06*
