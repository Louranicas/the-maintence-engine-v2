# N05: Evolution Gate Module Specification

> Mutation testing before deployments via RALPH loop

---

## Purpose

Gate all deployments and configuration changes through RALPH (Randomize And Learn Through Permutation Hunting) evolution testing to prevent regressions.

## Protocol

1. Capture baseline field state (r_baseline)
2. Apply proposed change in isolated sandbox
3. Run RALPH loop (K=1.0, 500 steps, 5 spheres)
4. Measure r_after
5. **Pass** if `r_after >= r_baseline`
6. **Fail** if `r_after < r_baseline`

## Interface

```rust
pub trait EvolutionGating: Send + Sync {
    fn evaluate(&self, request: GateRequest) -> Result<GateResult>;
    fn quick_check(&self, change: &ProposedChange) -> Result<GateVerdict>;
    fn history(&self, limit: usize) -> Result<Vec<GateResult>>;
}
```

## Database

- Writes gate results to `evolution_tracking.db`

## Integration

- Uses M39 Evolution Chamber for RALPH loop execution
- Uses N01 Field Bridge for r measurement
- Triggered by N06 Morphogenic Adapter on adaptation events

## Target

~1,000 LOC, 50+ tests
