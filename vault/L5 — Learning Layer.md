---
tags: [layer/L5, progressive-disclosure/L2, status/pending]
---

# L5: Learning Layer

> **Status:** PENDING | **Target LOC:** ~7,000 | **Target Tests:** 300+

## Purpose

Hebbian STDP learning, pattern recognition, pathway management, and anti-pattern detection. This layer enables ME V2 to learn from its own actions and improve over time.

## Modules

| ID | Module | File | Target LOC | Role |
|----|--------|------|-----------|------|
| M25 | Hebbian Manager | `hebbian.rs` | 900+ | Pathway creation, strengthening, decay |
| M26 | STDP Processor | `stdp.rs` | 700+ | Spike-timing dependent plasticity |
| M27 | Pattern Recognizer | `pattern.rs` | 1,000+ | Learned pattern matching |
| M28 | Pathway Pruner | `pruner.rs` | 1,300+ | Remove weak/redundant pathways |
| M29 | Memory Consolidator | `consolidator.rs` | 1,600+ | Cross-session learning persistence |
| M30 | Anti-Pattern Detector | `antipattern.rs` | 800+ | Detect harmful recurring patterns |

## STDP Constants

```
LTP_RATE     = 0.1     // Long-term potentiation
LTD_RATE     = 0.05    // Long-term depression
STDP_WINDOW  = 100ms   // Temporal window
DECAY_RATE   = 0.1     // HRS-001 corrected
CO_ACTIVATION_DELTA = 0.05  // Per-call increment
```

## Template Source

ME v1 `m5_learning/` (6,494 LOC). V2 integrates with N04 STDP Bridge for cross-service tool chain learning.

---

Full spec: `../ai_specs/m5-learning-specs/L5_LEARNING_SPEC.md` | Patterns: `../ai_specs/patterns/LEARNING_PATTERNS.md`
See [[HOME]] | Prev: [[L4 — Integration Layer]] | Next: [[L6 — Consensus Layer]]
