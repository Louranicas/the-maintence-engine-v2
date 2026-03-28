---
tags: [reference/patterns, progressive-disclosure/L3]
---

# Pattern Library

> 12 pattern specs in `../ai_specs/patterns/`

## Index

| File | Topic |
|------|-------|
| `INDEX.md` | Pattern library navigation |
| `MODULE_PATTERNS.md` | Module architecture patterns |
| `RUST_CORE_PATTERNS.md` | Rust-specific patterns (ownership, lifetimes, traits) |
| `ERROR_PATTERNS.md` | Error handling patterns |
| `DATABASE_PATTERNS.md` | SQLite access patterns |
| `CONCURRENCY_PATTERNS.md` | Lock ordering, channel patterns |
| `TENSOR_PATTERNS.md` | 12D tensor contribution patterns |
| `LEARNING_PATTERNS.md` | Hebbian/STDP patterns |
| `CONSENSUS_PATTERNS.md` | PBFT consensus patterns |
| `INTEGRATION_PATTERNS.md` | Cross-service communication patterns |
| `ANTIPATTERNS.md` | What NOT to do |
| `PATTERN_001_CIRCUIT_BREAKER.md` | Circuit breaker implementation example |

## Key Patterns

- **Builder:** All constructors use builder pattern
- **Interior Mutability:** `&self` + `parking_lot::RwLock`
- **Signal Emission:** `Arc<dyn SignalBusOps>` on state transitions
- **Tensor Contribution:** Every module implements `TensorContributor`
- **Scoped Guards:** Explicit `drop()` for lock guards
- **FMA:** Fused multiply-add for float precision

---

See [[HOME]] | [[05 — Design Constraints]]
