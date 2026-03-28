# N02: Intent Router — Module Specification

**Module ID:** N02
**Layer:** L8 (Nexus)
**File:** `src/nexus/intent_router.rs`
**Status:** STUB

---

## Purpose

Routes 12D IntentTensor vectors to appropriate services based on semantic similarity and learned pathway weights. Transforms high-dimensional intent representations into concrete service routing decisions, leveraging STDP-weighted pathways for adaptive routing that improves with usage patterns.

## Key Types

| Type | Kind | Purpose |
|------|------|---------|
| `IntentRouter` | Trait | Core trait for intent-to-service routing |
| `IntentTensor` | Struct | 12-dimensional intent vector for routing decisions |
| `RouteDecision` | Struct | Selected service target with confidence and reasoning |
| `RoutingTable` | Struct | Weighted mapping of intent patterns to service endpoints |
| `INTENT_ROUTER_MODULE_ID` | Const | "N02" — module identifier |

## Dependencies

- M09 (ServiceRegistry) — resolves service endpoints and availability for routing targets
- M20 (SemanticRouter) — provides semantic similarity matching from ORAC pattern library

## Dependents

- N04 (StdpBridge) — records co-activation from successful route completions
- L4+ modules — submit IntentTensors for cross-service operation routing

## Related Documentation

- [L8 Nexus Layer](../layers/L08_NEXUS.md)
- [Nexus Specs](../../ai_specs/nexus-specs/)
- [12D Tensor Encoding](../../CLAUDE.md#12d-tensor-encoding)
