---
tags: [reference/internet, progressive-disclosure/L2]
aliases: [internet-patterns, web-sources, external-exemplars]
---

# Internet Gold Standards — External Production Patterns

> 60+ patterns from 100+ authoritative web sources, compiled by 6 parallel research agents
> Every pattern is sourced, complete, and mapped to ME v2 architecture

## Domain Coverage

| Domain | Patterns | Key Sources |
|--------|----------|-------------|
| Axum + Tokio | A1-A8, T1-T7 | Official Axum/Tokio repos, mini-redis, SoftwareMill |
| RwLock + Builder | 6 patterns | parking_lot docs, Snoyman deadlock, Cliffle typestate |
| Error + SQLite | 7 patterns | GreptimeDB, RisingWave, Mozilla Firefox ConnExt |
| FSM + Consensus | 6 patterns | statig, Krustlet TransitionTo, TiKV raft-rs |
| Testing + Float | 7 patterns | proptest, rstest, float-cmp, Kahan summation |
| Kuramoto + Learning | 5 patterns | num-complex, STDP (michaelmelanson), genevo |

## Key Highlights

**Axum:** Graceful shutdown with `with_graceful_shutdown()`, `Router::merge` for 30+ routes, `AppError` with `IntoResponse`, `CancellationToken` hierarchy (parent → child per layer)

**RwLock:** Snoyman deadlock — `parking_lot` is task-fair (readers block when writer queued). Always own data before dropping scope. Multiple RwLocks for independent fields.

**SQLite:** WAL mode PRAGMA set (readers never block writers), `prepare_cached` for hot paths, Mozilla `ConnExt` trait for Connection + Transaction + Savepoint

**FSM:** `statig` for hierarchical states with entry/exit actions. DeisLabs `TransitionTo<S>` for compile-time edge enforcement. `rust-fsm` DSL for circuit breaker.

**PBFT:** `QuorumCollector` with `HashSet<AgentId>`, quorum = 2f+1 = 27. TiKV raft-rs drive loop: `step(msg)` + `tick()` + `Ready` batch pattern.

**Testing:** `proptest` with `f64::NORMAL` for Kuramoto/STDP invariants. `rstest` `#[once]` fixtures. `float-cmp` ULP comparison (never `==` for floats).

**Kuramoto:** `r = |mean(e^(iφ))| ∈ [0,1]` via `num-complex`. EMA smoothing with α=0.1.

**STDP:** Trace-based (pre/post traces decay exponentially). Lorentzian window `max_ltp / (1 + dt/half_life)`. Clamp to `[floor, ceiling]`.

---

**Full reference:** `ai_docs/INTERNET_GOLD_STANDARDS.md` (with complete code blocks and 100+ source URLs)

See [[Gold Standard Patterns]] | [[Rust Exemplars]] | [[Nexus Exemplars]]
