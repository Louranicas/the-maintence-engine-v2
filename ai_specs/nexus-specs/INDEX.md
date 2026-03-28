# L8 Nexus Integration — Spec Index

> **Layer:** L8 | **Modules:** N01-N06 (6) | **Status:** STUB | **Target LOC:** ~6,000

---

## Specs

| File | Module | Purpose |
|------|--------|---------|
| [L8_NEXUS_SPEC.md](L8_NEXUS_SPEC.md) | — | Layer architecture overview |
| [N01_FIELD_BRIDGE.md](N01_FIELD_BRIDGE.md) | N01 | Kuramoto r-tracking, field capture |
| [N02_INTENT_ROUTER.md](N02_INTENT_ROUTER.md) | N02 | 12D IntentTensor routing |
| [N03_REGIME_MANAGER.md](N03_REGIME_MANAGER.md) | N03 | K-regime detection (Swarm/Fleet/Armada) |
| [N04_STDP_BRIDGE.md](N04_STDP_BRIDGE.md) | N04 | Tool chain STDP learning |
| [N05_EVOLUTION_GATE.md](N05_EVOLUTION_GATE.md) | N05 | Mutation testing gate |
| [N06_MORPHOGENIC_ADAPTER.md](N06_MORPHOGENIC_ADAPTER.md) | N06 | Morphogenic adaptation |

## Key Constants

| Parameter | Value |
|-----------|-------|
| K_SWARM | 0.5 (K < 1.0) |
| K_FLEET | 1.5 (1.0 <= K < 2.0) |
| K_ARMADA | 3.0 (K >= 2.0) |
| r_adaptation_threshold | 0.05 |
| co_activation_delta | 0.05 |

## Navigation

- [Back to ai_specs/INDEX.md](../INDEX.md)
- [Module docs](../../ai_docs/modules/INDEX.md)
- [SCAFFOLDING_MASTER_PLAN](../../SCAFFOLDING_MASTER_PLAN.md)
