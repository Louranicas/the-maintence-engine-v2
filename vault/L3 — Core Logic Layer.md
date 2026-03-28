---
tags: [layer/L3, progressive-disclosure/L2, status/pending]
---

# L3: Core Logic Layer

> **Status:** PENDING | **Target LOC:** ~8,000 | **Target Tests:** 300+

## Purpose

The decision-making engine. Manages maintenance pipelines, automated remediation, confidence scoring, action execution, outcome recording, and feedback loops. This is where the system decides *what* to do and *whether* to do it.

## Modules

| ID | Module | File | Target LOC | Role |
|----|--------|------|-----------|------|
| M13 | Pipeline Manager | `pipeline.rs` | 1,500+ | 5-phase maintenance pipelines |
| M14 | Remediation Engine | `remediation.rs` | 1,500+ | Automated issue resolution |
| M15 | Confidence Calculator | `confidence.rs` | 1,200+ | Bayesian confidence scoring |
| M16 | Action Executor | `action.rs` | 1,500+ | Safe action dispatch with rollback |
| M17 | Outcome Recorder | `outcome.rs` | 900+ | Result tracking and persistence |
| M18 | Feedback Loop | `feedback.rs` | 1,000+ | Closed-loop learning from outcomes |

## Escalation Tiers (from M15)

```
L0: confidence >= 0.9, severity <= MEDIUM  -> auto-remediate
L1: confidence >= 0.7, severity <= HIGH    -> remediate with logging
L2: confidence <  0.7 OR severity = HIGH   -> escalate to human
L3: critical actions                       -> PBFT consensus (27/40)
```

## Template Source

ME v1 `m3_core_logic/` (7,902 LOC). V2 adds Nexus field capture (C11) and STDP co-activation (C12).

---

Full spec: `../ai_specs/m3-core-logic-specs/L3_CORE_LOGIC_SPEC.md` | Module docs: `../ai_docs/modules/M13-M18`
See [[HOME]] | Prev: [[L2 — Services Layer]] | Next: [[L4 — Integration Layer]]
