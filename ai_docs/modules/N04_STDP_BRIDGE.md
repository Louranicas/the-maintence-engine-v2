# N04: STDP Bridge — Module Specification

**Module ID:** N04
**Layer:** L8 (Nexus)
**File:** `src/nexus/stdp_bridge.rs`
**Status:** STUB

---

## Purpose

Records tool chain STDP learning from service interactions at the Nexus layer. Every cross-service call records a +0.05 co-activation delta (C12 enforcement), strengthening pathways between frequently co-activated services and enabling adaptive routing based on learned interaction patterns.

## Key Types

| Type | Kind | Purpose |
|------|------|---------|
| `StdpBridge` | Trait | Core trait for STDP co-activation recording at Nexus level |
| `CoActivation` | Struct | Source/target service pair with timestamp and delta |
| `ToolChainRecord` | Struct | Sequence of service calls forming a learned tool chain |
| `CO_ACTIVATION_DELTA` | Const | 0.05 — per-call STDP increment (C12 enforcement) |
| `STDP_BRIDGE_MODULE_ID` | Const | "N04" — module identifier |

## Dependencies

- M25 (HebbianManager) — writes pathway weight updates from co-activation events
- M26 (StdpProcessor) — applies spike-timing dependent plasticity rules to weight changes

## Dependents

- N02 (IntentRouter) — reads STDP-weighted pathways for adaptive routing decisions
- N05 (EvolutionGate) — evaluates STDP pathway health as part of mutation fitness

## Related Documentation

- [L8 Nexus Layer](../layers/L08_NEXUS.md)
- [Nexus Specs](../../ai_specs/nexus-specs/)
- [STDP Learning Parameters](../../CLAUDE.md#stdp-learning-parameters)
- [Design Constraint C12](../../CLAUDE.md#design-constraints-c1-c12)
