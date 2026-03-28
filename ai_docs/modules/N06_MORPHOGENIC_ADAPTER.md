# N06: Morphogenic Adapter — Module Specification

**Module ID:** N06
**Layer:** L8 (Nexus)
**File:** `src/nexus/morphogenic_adapter.rs`
**Status:** STUB

---

## Purpose

Triggers adaptation responses when field coherence shifts exceed the threshold |r_delta| > 0.05. Monitors r_delta from field bridge snapshots and applies regime-aware morphogenic strategies: recalibration in Swarm mode, rebalancing in Fleet mode, and synchronization reinforcement in Armada mode.

## Key Types

| Type | Kind | Purpose |
|------|------|---------|
| `MorphogenicAdapter` | Trait | Core trait for field-coherence-triggered adaptation |
| `AdaptationEvent` | Struct | r_delta, regime, strategy applied, timestamp |
| `AdaptationStrategy` | Enum | Recalibrate / Rebalance / Reinforce per K-regime |
| `R_ADAPTATION_THRESHOLD` | Const | 0.05 — |r_delta| trigger for morphogenic adaptation |
| `MORPHOGENIC_ADAPTER_MODULE_ID` | Const | "N06" — module identifier |

## Dependencies

- N01 (FieldBridge) — provides r_delta values from pre/post field state capture
- N03 (RegimeManager) — determines active K-regime for strategy selection

## Dependents

- Engine orchestrator — receives adaptation events for system-wide coordination
- M04 (MetricsCollector) — records adaptation frequency and strategy distribution

## Related Documentation

- [L8 Nexus Layer](../layers/L08_NEXUS.md)
- [Nexus Specs](../../ai_specs/nexus-specs/)
- [Field Capture Pattern](../../CLAUDE.md#field-capture-pattern)
- [Kuramoto Parameters](../../CLAUDE.md#nexus-integration-l8--new-in-v2)
