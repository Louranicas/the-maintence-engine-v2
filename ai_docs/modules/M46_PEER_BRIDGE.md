# M46: Peer Bridge — Module Specification

**Module ID:** M46
**Layer:** L4 (Integration)
**File:** `src/m4_integration/peer_bridge.rs`
**Status:** DEPLOYED (previously unnumbered infrastructure)
**LOC:** ~600
**Tests:** 53

---

## Purpose

Active health polling and communication bridge for ULTRAPLATE peer services. Manages tiered health polling with circuit breaker protection, synergy computation, and peer state tracking for all 12 registered ULTRAPLATE services.

## Key Types

| Type | Kind | Purpose |
|------|------|---------|
| `PeerBridgeManager` | Struct | Manages all peer health polling |
| `PeerConfig` | Struct | Per-peer polling configuration (tier, interval, timeout) |
| `PeerHealthResult` | Struct | Single poll result with latency and status |
| `PeerBridgeStats` | Struct | Aggregate polling statistics |

## Dependencies

- `crate::m1_foundation::{Error, Result, Timestamp, SignalBus, MetricsRegistry}` (L1)
- `crate::m2_services::ServiceTier` (L2)
- `parking_lot::RwLock`, `AtomicBool`

## Dependents

- `engine.rs` — Engine holds `Option<PeerBridgeManager>`, spawns polling in background
- `main.rs` spawn_peer_polling() — 30s polling cycle
- M55 ORAC Bridge (new) — follows same bridge pattern

## Related Documentation

- [L4 Integration](../layers/L04_INTEGRATION.md)
- [Bridge Manager](M24_BRIDGE_MANAGER.md)
