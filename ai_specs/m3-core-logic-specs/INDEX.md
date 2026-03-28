# L3 Core Logic — Spec Sheet Index

> **Layer:** L3 Core Logic | **Modules:** M13-M18 (6 modules, 7 files)
> **LOC:** ~8,000 (target) | **Tests:** 300+ (target) | **Quality Score:** PENDING
> **Status:** SPECIFIED — awaiting implementation | **Verified:** 2026-03-06

---

## Document Map

| Document | Description | Tokens |
|----------|-------------|--------|
| [L3_CORE_LOGIC_SPEC.md](L3_CORE_LOGIC_SPEC.md) | Full layer specification: data flow, design constraints, test strategy, all 6 modules | ~3K |

---

## Reading Protocol

```
QUICK START:    Read L3_CORE_LOGIC_SPEC.md (architecture + pipeline data flow)
WRITING CODE:   Read the M13-M18 section relevant to the module you're implementing
CROSS-LAYER:    L3 consumes L2 health data, emits remediation signals to L2, feeds learning data to L5
CONSUMING L3:   Pipeline (M13) + Confidence (M15) + Feedback (M18) — the decision loop
```

---

## Module Table

| # | Module | ID | File | Target LOC | Target Tests | Status |
|---|--------|----|------|-----------|-------------|--------|
| 1 | Pipeline Manager | M13 | `pipeline.rs` | ~1,400 | 50+ | PENDING |
| 2 | Remediation Engine | M14 | `remediation.rs` | ~1,500 | 50+ | PENDING |
| 3 | Confidence Calculator | M15 | `confidence.rs` | ~1,200 | 50+ | PENDING |
| 4 | Action Executor | M16 | `action.rs` | ~1,500 | 50+ | PENDING |
| 5 | Outcome Recorder | M17 | `outcome.rs` | ~900 | 50+ | PENDING |
| 6 | Feedback Loop | M18 | `feedback.rs` | ~950 | 50+ | PENDING |
| 7 | Layer Coordinator | — | `mod.rs` | ~500 | 20+ | PENDING |
| | **Subtotal** | | | **~7,950** | **~320** | |

---

## Quick Reference — 5 Traits

| # | Trait | File | Bounds | Methods | Defaults |
|---|-------|------|--------|---------|----------|
| 1 | `PipelineOps` | pipeline.rs | Send+Sync | 6 | 0 |
| 2 | `RemediationOps` | remediation.rs | Send+Sync | 5 | 0 |
| 3 | `ConfidenceOps` | confidence.rs | Send+Sync | 4 | 0 |
| 4 | `ActionExecutor` | action.rs | Send+Sync | 4 | 0 |
| 5 | `TensorContributor` | (all modules) | Send+Sync+Debug | 3 | 0 |

---

## Quick Reference — Tensor Dimensions

| Dim | Name | L3 Contributors | Notes |
|-----|------|-----------------|-------|
| D0 | ServiceId | — | Inherited from L2 |
| D1 | Port | — | — |
| D2 | Tier | — | — |
| D3 | DependencyCount | — | — |
| D4 | AgentCount | — | — |
| D5 | Protocol | — | — |
| D6 | HealthScore | — | — |
| D7 | Uptime | — | — |
| D8 | Synergy | — | — |
| D9 | Latency | M13 (pipeline execution time) | Pipeline SLO tracking |
| D10 | ErrorRate | M14 (remediation impact) | Error rate reduction after remediation |
| D11 | TemporalContext | — | — |

---

## Data Flow

```
L2 Health Events --> M13 Pipeline --> M15 Confidence --> M14 Remediation
                                                             |
                                      M18 Feedback <-- M17 Outcome <-- M16 Action
                                           |
                                      L5 Hebbian Learning
```

---

## Design Constraints

| ID | Constraint | Enforcement |
|----|-----------|-------------|
| C1 | Only imports from L1 and L2 | Compile-time module DAG |
| C2 | All trait methods `&self` | Code review |
| C3 | `TensorContributor` on M13, M14, M16 | Compile-time |
| C4 | Zero unsafe/unwrap/expect | `#![forbid(unsafe_code)]` + clippy deny |
| C5 | `Timestamp` + `Duration` only | Grep + clippy |
| C6 | Signal emission on all state transitions | Architecture |
| C11 | Nexus field capture on all remediation actions | N01 integration |
| C12 | STDP co-activation on pipeline completions | N04 integration |

---

## Cross-References

- **Upstream:** [M1 Foundation Specs](../m1-foundation-specs/) | [M2 Services Specs](../m2-services-specs/)
- **Downstream:** L4 Integration (M19-M24c), L5 Learning (M25-M30)
- **System:** [SYSTEM_SPEC](../SYSTEM_SPEC.md), [LAYER_SPEC](../LAYER_SPEC.md), [MODULE_MATRIX](../MODULE_MATRIX.md)
- **Patterns:** [PIPELINE](../patterns/PIPELINE.md), [ERROR_HANDLING](../patterns/ERROR_HANDLING.md), [STATE_MACHINE](../patterns/STATE_MACHINE.md)
- **Tensor:** [TENSOR_SPEC](../TENSOR_SPEC.md)
- **Escalation:** [ESCALATION_SPEC](../ESCALATION_SPEC.md) (L0-L3 tiers)
- **Tests:** `tests/l3_core_logic_integration.rs` | **Bench:** `benches/pipeline_execution.rs`

---

*L3 Core Logic Spec Sheet Index v1.0 | 2026-03-06*
