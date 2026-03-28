---
tags: [layer/L8, progressive-disclosure/L2, status/pending]
---

# L8: Nexus Layer (NEW in V2)

> **Status:** PENDING | **Target LOC:** ~6,000 | **Target Tests:** 300+

## Purpose

The defining layer of V2. Bridges the Maintenance Engine to the Nexus Controller and Oscillating Vortex Memory. Enables Kuramoto field coherence tracking, intent routing, K-regime awareness, STDP tool chain learning, evolution gating, and morphogenic adaptation.

## Modules

| ID | Module | File | Target LOC | Source Pattern |
|----|--------|------|-----------|----------------|
| N01 | Field Bridge | `field_bridge.rs` | 800+ | VMS HookEngine |
| N02 | Intent Router | `intent_router.rs` | 600+ | VMS IntentEncoder |
| N03 | Regime Manager | `regime_manager.rs` | 500+ | VMS SwarmCoordinator |
| N04 | STDP Bridge | `stdp_bridge.rs` | 700+ | VMS StdpKernel |
| N05 | Evolution Gate | `evolution_gate.rs` | 600+ | VMS EvolutionChamber |
| N06 | Morphogenic Adapter | `morphogenic_adapter.rs` | 500+ | VMS MorphogenicEngine |

## K-Regime Detection

```
K < 1.0  -> Swarm   (independent parallel agents, low coupling)
1.0-2.0  -> Fleet   (coordinated parallel work, medium coupling)
K >= 2.0 -> Armada  (synchronized convergence, max coupling)
```

## Morphogenic Trigger

When `|r_delta| > 0.05`, N06 fires an adaptation trigger. This can:
- Adjust service weights
- Trigger pathway pruning (L5)
- Request evolution chamber evaluation (L7)
- Escalate to PBFT consensus (L6) if severity warrants

## 12D Tensor Contributions

- N01 -> D11 (temporal_context)
- N04 -> D8 (synergy)

---

Full spec: `../ai_specs/nexus-specs/` (7 files: L8 layer + N01-N06 individual specs)
See [[HOME]] | Prev: [[L7 — Observer Layer]] | Next: [[V3 — Homeostasis]]
