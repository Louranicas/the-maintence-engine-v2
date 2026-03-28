# N01: Field Bridge — Module Specification

**Module ID:** N01
**Layer:** L8 (Nexus)
**File:** `src/nexus/field_bridge.rs`
**Status:** STUB

---

## Purpose

Kuramoto r-tracking and pre/post field state capture for all L4+ operations. Provides real-time field coherence measurement, records r_before/r_after deltas, and emits morphogenic adaptation signals when |r_delta| exceeds the 0.05 threshold. Central field state provider for the entire Nexus layer.

## Key Types

| Type | Kind | Purpose |
|------|------|---------|
| `FieldBridge` | Trait | Core trait for field coherence tracking and capture |
| `FieldSnapshot` | Struct | Pre/post r values with timestamp and operation context |
| `FieldDelta` | Struct | r_delta with magnitude and direction (converging/diverging) |
| `CoherenceHistory` | Struct | Rolling window of r values for trend analysis |
| `FIELD_BRIDGE_MODULE_ID` | Const | "N01" — module identifier |

## Dependencies

- M07 (SignalBus) — emits field state change signals on r_delta transitions
- M08 (TensorRegistry) — contributes D8 (synergy) and D11 (temporal) tensor dimensions
- M44 (ObserverBus) — publishes field snapshots for observer layer consumption

## Dependents

- N03 (RegimeManager) — reads current r for K-regime classification
- N05 (EvolutionGate) — compares r_after vs r_baseline for mutation acceptance
- N06 (MorphogenicAdapter) — monitors |r_delta| > 0.05 for adaptation triggers
- All L4+ modules — call `field_coherence()` for pre/post operation capture

## Related Documentation

- [L8 Nexus Layer](../layers/L08_NEXUS.md)
- [Nexus Specs](../../ai_specs/nexus-specs/)
- [12D Tensor Encoding](../../CLAUDE.md#12d-tensor-encoding)
