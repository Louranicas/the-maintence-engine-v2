# M45: Fitness Evaluator — Module Specification

**Module ID:** M45
**Layer:** L7 (Observer)
**File:** `src/m7_observer/fitness.rs`
**Status:** DEPLOYED (previously unnumbered infrastructure)
**LOC:** 1,006
**Tests:** 43

---

## Purpose

12D tensor fitness scoring utility for the L7 Observer Layer. Evaluates system health using weighted dimension analysis, trend detection, and stability assessment against the `Tensor12D` encoding. Produces `FitnessReport` consumed by M39 Evolution Chamber for RALPH fitness tracking.

## Key Types

| Type | Kind | Purpose |
|------|------|---------|
| `FitnessEvaluator` | Struct | Weighted 12D scoring engine |
| `FitnessReport` | Struct | Scored result with per-dimension contributions |
| `FitnessTrend` | Enum | Improving/Stable/Declining/Unknown |
| `SystemState` | Enum | Optimal/Healthy/Degraded/Critical/Failed |
| `DIMENSION_WEIGHTS` | Const [f64; 12] | Sum=1.0 weights for fitness scoring |

## Dimension Weights

| Dim | Name | Weight | Category |
|-----|------|--------|----------|
| D6 | health_score | 0.20 | Primary |
| D7 | uptime | 0.15 | Primary |
| D8 | synergy | 0.15 | Primary |
| D9 | latency | 0.10 | Secondary |
| D10 | error_rate | 0.10 | Secondary |
| D2 | tier | 0.08 | Secondary |

## Dependencies

- `crate::{Error, Result, Tensor12D}` (M01, lib.rs)
- `chrono`, `parking_lot::RwLock`, `serde`

## Dependents

- M39 Evolution Chamber (consumes FitnessReport for RALPH evaluation)
- L7 mod.rs coordinator (calls evaluate() per tick)
- `engine.rs` build_tensor() (uses dimension weights)

## Related Documentation

- [Fitness Function Spec](../../ai_specs/evolution_chamber_ai_specs/FITNESS_FUNCTION_SPEC.md)
- [Fitness Evaluator AI Doc](../evolution_chamber_ai_docs/FITNESS_EVALUATOR.md)
- [L7 Observer Layer](../layers/L07_OBSERVER.md)
