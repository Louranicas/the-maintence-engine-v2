# N01: Field Bridge Module Specification

> Kuramoto order parameter r-tracking with pre/post field capture

---

## Purpose

Track Kuramoto field coherence (order parameter r) before and after every significant maintenance operation, providing field-awareness to all layers. This is the primary integration point between ME V2 and the Oscillating Vortex Memory (OVM).

## Interface

```rust
pub trait FieldBridgeOps: Send + Sync {
    fn current_r(&self) -> Result<f64>;
    fn capture_pre(&self) -> Result<f64>;
    fn capture_post(&self, r_before: f64, operation: &str) -> Result<FieldCapture>;
    fn field_snapshot(&self) -> Result<FieldSnapshot>;
    fn r_history(&self, limit: usize) -> Result<Vec<(Timestamp, f64)>>;
    fn is_coherent(&self) -> Result<bool>;
}
```

## Data Sources

- SVF WebSocket `/ws/field-evolution` (M21 WebSocket client)
- VMS MCP `coherence_report` tool (M19 REST client)
- Local r-simulation fallback when SVF unavailable

## Tensor Contribution

- D8 (synergy) — r value directly
- D11 (temporal) — r trend (rising/falling/stable)

## Signals

- `FieldCoherenceChanged { old_r, new_r }`
- `FieldVolatile { r, variance }`
- `FieldCritical { r }` — emitted when r < 0.3

## Database

- Writes to `tensor_memory.db` (r-history snapshots)

## Design Constraints

- C2: All methods `&self`
- C4: Zero unsafe/unwrap/expect
- C5: `Timestamp` only
- Graceful degradation when SVF unavailable (use last known r)

## Target

~1,000 LOC, 50+ tests
