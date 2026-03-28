# N06: Morphogenic Adapter Module Specification

> Adaptation triggers when |r_delta| > 0.05

---

## Purpose

Trigger adaptive responses when field coherence shifts significantly, automatically adjusting system parameters to maintain homeostasis.

## Adaptation Rules

| Condition | Adaptation |
|-----------|-----------|
| r_delta > +0.05 AND r > 0.95 | DecreaseK (system too rigid) |
| r_delta < -0.05 AND r < 0.5 | IncreaseK (losing coherence) |
| \|r_delta\| > 0.1 | TriggerPruning + RebalanceSTDP |
| 0.05 < \|r_delta\| < 0.1 | EmitWarning (monitor only) |

## Interface

```rust
pub trait MorphogenicOps: Send + Sync {
    fn should_adapt(&self, capture: &FieldCapture) -> Result<bool>;
    fn select_adaptation(&self, capture: &FieldCapture) -> Result<AdaptationType>;
    fn apply_adaptation(&self, adaptation: AdaptationType) -> Result<AdaptationEvent>;
    fn adaptation_history(&self, limit: usize) -> Result<Vec<AdaptationEvent>>;
}
```

## Integration

- Receives FieldCapture from N01 Field Bridge
- Modifies K via N03 Regime Manager
- Triggers pruning via M28 Pathway Pruner
- Adjusts STDP rates via M26 STDP Processor
- Cool-down period: 60s between adaptations (prevent oscillation)

## Target

~1,000 LOC, 50+ tests
