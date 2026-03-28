# N04: STDP Bridge Module Specification

> Tool chain STDP learning from service interactions

---

## Purpose

Record STDP co-activations when services interact, building Hebbian pathways that represent learned service relationships. Each service interaction increments the pathway weight by +0.05 (C12).

## Interface

```rust
pub trait StdpBridgeOps: Send + Sync {
    fn record_interaction(&self, source: ServiceId, target: ServiceId) -> Result<()>;
    fn record_tool_chain(&self, chain: ToolChainRecord) -> Result<()>;
    fn co_activation_count(&self, source: &ServiceId, target: &ServiceId) -> Result<u64>;
    fn synergy_pairs(&self, threshold: f64) -> Result<Vec<(ServiceId, ServiceId, f64)>>;
}
```

## Co-Activation Rule

```
pathway_weight += CO_ACTIVATION_DELTA (0.05)
```

Applied on every cross-service call. Decayed by M41 Decay Auditor at rate 0.1.

## Integration

- Feeds into M25 Hebbian Manager (pathway creation/strengthening)
- Feeds into M26 STDP Processor (timing-based potentiation)
- Writes to `hebbian_pulse.db` and `system_synergy.db`

## Target

~900 LOC, 50+ tests
