# M44: Observer Bus — Module Specification

**Module ID:** M44
**Layer:** L7 (Observer)
**File:** `src/m7_observer/observer_bus.rs`
**Status:** DEPLOYED (previously unnumbered infrastructure)
**LOC:** 975
**Tests:** 44

---

## Purpose

Internal L7 pub/sub bus connecting M37 (Log Correlator), M38 (Emergence Detector), and M39 (Evolution Chamber). Provides bounded-channel event distribution within the Observer layer with subscription management and event history tracking.

## Key Types

| Type | Kind | Purpose |
|------|------|---------|
| `ObserverBus` | Struct | Central pub/sub bus for L7 internal events |
| `ObserverEvent` | Enum | Typed events: Correlation, Emergence, Evolution, Thermal |
| `ObserverSubscription` | Struct | Subscriber binding with optional filter |

## Dependencies

- `crate::m1_foundation::{Error, Result}` (M01)
- `parking_lot::RwLock`

## Dependents

- M37 Log Correlator (publishes correlations)
- M38 Emergence Detector (subscribes + publishes)
- M39 Evolution Chamber (subscribes to emergence events)
- L7 mod.rs coordinator (owns bus instance)

## Related Documentation

- [Observer Bus Spec](../../ai_specs/evolution_chamber_ai_specs/OBSERVER_BUS_SPEC.md)
- [L7 Observer Layer](../layers/L07_OBSERVER.md)
- [Evolution Chamber AI Docs](../evolution_chamber_ai_docs/OBSERVER_BUS.md)
