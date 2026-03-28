---
tags: [nav/constraints, progressive-disclosure/L1]
---

# Design Constraints (C1-C12)

Inherited from M1/M2 gold standard + V2 extensions. These are inviolable.

| ID | Constraint | Enforcement |
|----|-----------|-------------|
| C1 | No upward imports (strict DAG) | Compile-time |
| C2 | Trait methods always `&self` (interior mutability via `parking_lot::RwLock`) | Code review |
| C3 | Every module implements `TensorContributor` | Compile-time |
| C4 | Zero unsafe, unwrap, expect, clippy warnings | `#![forbid(unsafe_code)]` + clippy deny |
| C5 | No `chrono` or `SystemTime` — use `Timestamp` + `Duration` | Grep + clippy |
| C6 | Signal emissions via `Arc<SignalBus>` on state transitions | Architecture |
| C7 | Owned returns through `RwLock` (never return references) | Code review |
| C8 | Timeouts use `std::time::Duration` | Code review |
| C9 | Existing downstream tests must not break | CI gate |
| C10 | 50+ tests per layer minimum | CI gate |
| C11 | Every L4+ module has Nexus field capture | Architecture (NEW) |
| C12 | All service interactions record STDP co-activation | Architecture (NEW) |

## Anti-Patterns (Never Do)

| Forbidden | Use Instead |
|-----------|-------------|
| `unsafe { }` | Safe Rust only |
| `.unwrap()` | `?` or match |
| `.expect()` | `?` with context |
| `panic!()` | `Result<T>` |
| `println!()` for logs | `tracing` macros |
| `chrono::DateTime` | `Timestamp` |
| `SystemTime` | `Duration` |
| Unbounded channels | Always set capacity |
| `Clone` where move works | Move semantics |
| `&mut self` traits | `&self` + `RwLock` |

## Gold Standard Patterns (from L1/L2)

- **6 Core Traits:** ServiceDiscovery (14), HealthMonitoring (11), LifecycleOps (13), CircuitBreakerOps (12), LoadBalancing (10), TensorContributor
- **Builder pattern** for all constructors
- **Scoped lock guards** with explicit drop
- **FMA** for float precision
- **Signal emission** on every state transition

---

See [[HOME]] | [[02 — Build & Quality Gate]]
