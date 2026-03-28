# N03: Regime Manager Module Specification

> K-regime detection and management (Swarm/Fleet/Armada)

---

## Purpose

Detect and manage Kuramoto coupling strength (K) regime transitions, enabling appropriate coordination strategies for different task complexities.

## Regimes

| Regime | K Range | Coupling | Strategy |
|--------|---------|----------|----------|
| Swarm | K < 1.0 | Low | Independent parallel agents |
| Fleet | 1.0 <= K < 2.0 | Medium | Coordinated parallel work |
| Armada | K >= 2.0 | High | Synchronized convergence |

## Interface

```rust
pub trait RegimeOps: Send + Sync {
    fn current_regime(&self) -> Result<KRegime>;
    fn current_k(&self) -> Result<f64>;
    fn suggest_regime(&self, task_complexity: f64, agent_count: u32) -> Result<KRegime>;
    fn transition_to(&self, target: KRegime) -> Result<RegimeTransition>;
    fn is_stable(&self) -> Result<bool>;
}
```

## Selection Heuristic

- `task_complexity < 0.3` → Swarm
- `0.3 <= task_complexity < 0.7` → Fleet
- `task_complexity >= 0.7` → Armada

## Target

~900 LOC, 50+ tests
