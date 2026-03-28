# L1 Foundation — Layer Overview

> **Module Range:** M00-M08 (9 modules) | **Files:** 11 | **LOC:** ~16,711 | **Tests:** 678

---

## Purpose

L1 Foundation provides the vocabulary types, error taxonomy, infrastructure services, and cross-cutting abstractions (signals, tensors) that ALL upper layers depend on. It is the only layer with zero upward dependencies — every import flows downward from L7→L6→L5→L4→L3→L2→L1.

---

## File Map

```
src/m1_foundation/
├── mod.rs              # M00 — Layer coordinator, re-exports, FoundationStatus, build_foundation_tensor()
├── shared_types.rs     # M00 — ModuleId(42), AgentId, Timestamp(atomic), HealthReport, DimensionIndex(12), CoverageBitmap
├── error.rs            # M01 — Error(16 variants), Severity(4), ErrorClassifier trait, AnnotatedError, Result<T>
├── config.rs           # M02 — ConfigProvider trait, Config, ConfigBuilder, ConfigManager(hot-reload)
├── logging.rs          # M03 — CorrelationProvider trait, LogContext, LogFormat, LogLevel, init_logging
├── metrics.rs          # M04 — MetricRecorder trait, Counter/Gauge/Histogram, MetricsRegistry, Prometheus export
├── state.rs            # M05 — StateStore trait, DatabaseConfig/Pool, QueryBuilder, 11 DatabaseTypes, StatePersistence
├── resources.rs        # M06 — ResourceCollector trait, SystemResources, ResourceLimits, AdaptiveResourceLimits
├── nam.rs              # NAM — AgentOrigin(4 variants), Confidence, Outcome, LearningSignal, Dissent
├── signals.rs          # M07 — SignalSubscriber trait, SignalBus, HealthSignal, LearningEvent, DissentEvent
└── tensor_registry.rs  # M08 — TensorContributor trait, TensorRegistry, ComposedTensor, CoverageBitmap composition
```

---

## Dependency Graph (Internal)

```
mod.rs ─────────────────→ ALL sub-modules (re-exports)
shared_types.rs           → (no internal deps — leaf)
error.rs                  → nam.rs (AgentOrigin, Confidence for AnnotatedError)
config.rs                 → shared_types, error
logging.rs                → shared_types, error
metrics.rs                → shared_types, error
state.rs                  → shared_types, error, config
resources.rs              → shared_types, error
nam.rs                    → shared_types (AgentId)
signals.rs                → shared_types, error, nam (LearningSignal, Dissent, AgentOrigin)
tensor_registry.rs        → shared_types (DimensionIndex, CoverageBitmap)
```

---

## Concurrency Architecture

L1 uses three synchronization strategies:

| Strategy | Where | Pattern |
|----------|-------|---------|
| **Atomic primitives** | `GLOBAL_TICK` (AtomicU64), `ConfigManager.reload_flag` (AtomicBool) | Lock-free, single-word operations |
| **parking_lot::RwLock** | ConfigManager, Counter/Gauge/Histogram, MetricsRegistry, SignalBus | Read-heavy access with infrequent writes |
| **OnceLock** | `LOGGING_INITIALIZED` | Single-writer, set-once global |
| **No sync (value types)** | All vocabulary types, TensorRegistry, ResourceManager | Caller responsible for external wrapping |

All traits use `&self` (not `&mut self`) to support `Arc<dyn Trait>` patterns. Two exceptions:
- `TensorRegistry::register(&mut self)` — setup-phase-only, not behind Arc at runtime
- `ResourceManager` methods use `&mut self` — wrapped externally by L2

---

## Design Principles

1. **Value types are `Copy` where possible** — ModuleId, Timestamp, DimensionIndex, CoverageBitmap, Severity, Outcome, Confidence
2. **All f64 outputs clamped to [0.0, 1.0]** — health scores, tensor dimensions, coverage ratios
3. **`#[must_use]` on every pure function and builder method** — enforced across 200+ annotations
4. **`const fn` on all computation that permits it** — 57 const fn across L1
5. **No `unsafe`, no `unwrap`, no `expect`** — compile-time `#![forbid(unsafe_code)]` + clippy deny
6. **No `chrono`, no `SystemTime` for temporal logic** — `Timestamp` (atomic tick counter) + `std::time::Duration`
7. **`Display` on all public types** — 27 implementations for debugging and logging
8. **Builder patterns with validation** — ConfigBuilder.build() returns Result, enforces port conflict detection

---

## Error Strategy

All L1 modules use a single `Error` enum (16 variants) with error codes 1000-1900:

| Code Range | Category | Source Modules |
|------------|----------|----------------|
| 1000 | Config | config.rs, logging.rs |
| 1100 | Database | state.rs |
| 1200-1202 | Network/Circuit/Timeout | (L2+) |
| 1300-1301 | Consensus | (L6) |
| 1400-1401 | Learning/Tensor | (L5+) |
| 1500 | Validation | config.rs, metrics.rs, resources.rs |
| 1600 | IO | error.rs (From<std::io::Error>) |
| 1700 | Pipeline | (L3) |
| 1800-1802 | Service/Health/Escalation | (L2+) |
| 1900 | Other | resources.rs |

---

## NAM Compliance

| NAM Req | L1 Implementation |
|---------|-------------------|
| R1 SelfQuery | `ErrorClassifier.is_retryable()`, `Confidence.is_valid()` |
| R2 HebbianRouting | `LearningSignal`, `Outcome` (Success/Failure/Partial) |
| R3 DissentCapture | `Dissent`, `DissentEvent`, `SignalBus.emit_dissent()` |
| R4 FieldVisualization | `TensorContributor`, `ComposedTensor`, 12D tensor composition |
| R5 HumanAsAgent | `AgentOrigin::Human`, `HUMAN_AGENT_TAG="@0.A"`, `agent_id()` on all traits |

---

## Quality Gate Results

```
cargo check                                    ✅ 0 errors
cargo clippy -- -D warnings                    ✅ 0 warnings
cargo clippy -- -D warnings -W clippy::pedantic ✅ 0 warnings
cargo clippy -- -D warnings -W clippy::nursery  ✅ 0 warnings
cargo test --lib m1_foundation                 ✅ 678 tests, 0 failures
Zero-tolerance grep (unsafe/unwrap/expect/etc) ✅ 0 hits
```

---

---

## Notes

**SignalBusOps / TensorRegistryOps:** The L2 refactor plan called for extracting these traits from `SignalBus` and `TensorRegistry` (Phase 0B). They were **never implemented** — L2 modules use concrete types directly. The system works correctly without them.

---

*L1 Foundation Layer Overview v1.1 | 2026-03-01*
