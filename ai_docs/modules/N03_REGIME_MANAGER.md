# N03: Regime Manager — Module Specification

**Module ID:** N03
**Layer:** L8 (Nexus)
**File:** `src/nexus/regime_manager.rs`
**Status:** STUB

---

## Purpose

Detects and manages K-regime transitions across three operational modes: Swarm (K < 1.0, independent parallel), Fleet (1.0 <= K < 2.0, coordinated), and Armada (K >= 2.0, synchronized convergence). Classifies the current coupling strength and triggers regime-appropriate coordination strategies.

## Key Types

| Type | Kind | Purpose |
|------|------|---------|
| `RegimeManager` | Trait | Core trait for K-regime detection and transition management |
| `KRegime` | Enum | Swarm / Fleet / Armada operational modes |
| `RegimeTransition` | Struct | Previous and new regime with transition timestamp |
| `K_SWARM` | Const | 0.5 — Swarm regime coupling constant |
| `K_FLEET` | Const | 1.5 — Fleet regime coupling constant |
| `K_ARMADA` | Const | 3.0 — Armada regime coupling constant |

## Dependencies

- N01 (FieldBridge) — reads current Kuramoto r for regime classification thresholds

## Dependents

- N06 (MorphogenicAdapter) — adapts morphogenic strategy based on active regime
- Engine orchestrator — selects coordination strategy per active K-regime

## Related Documentation

- [L8 Nexus Layer](../layers/L08_NEXUS.md)
- [Nexus Specs](../../ai_specs/nexus-specs/)
- [Kuramoto Parameters](../../CLAUDE.md#nexus-integration-l8--new-in-v2)
