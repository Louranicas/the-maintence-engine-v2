# L1 Foundation — Spec Sheet Index

> **Layer:** L1 Foundation | **Modules:** M00-M08 (9 modules, 11 files)
> **LOC:** ~16,711 | **Tests:** 678 | **Quality Score:** 80.6/100
> **Status:** COMPLETE | **Baseline Commit:** 1a60c5e | **Verified:** 2026-03-01

---

## Document Map

| Document | Description | Tokens |
|----------|-------------|--------|
| [00-LAYER-OVERVIEW.md](00-LAYER-OVERVIEW.md) | Architecture, dependency graph, concurrency model, design principles | ~2K |
| [01-VOCABULARY-TYPES.md](01-VOCABULARY-TYPES.md) | M00 shared_types.rs — ModuleId, AgentId, Timestamp, CoverageBitmap, DimensionIndex, HealthReport | ~3K |
| [02-ERROR-TAXONOMY.md](02-ERROR-TAXONOMY.md) | M01 error.rs — 16 Error variants, Severity, ErrorClassifier, AnnotatedError | ~2K |
| [03-CONFIG.md](03-CONFIG.md) | M02 config.rs — ConfigProvider trait, ConfigBuilder, ConfigManager, hot-reload | ~2K |
| [04-LOGGING.md](04-LOGGING.md) | M03 logging.rs — CorrelationProvider trait, LogContext, structured logging | ~1.5K |
| [05-METRICS.md](05-METRICS.md) | M04 metrics.rs — MetricRecorder trait, Counter/Gauge/Histogram, Prometheus export | ~2K |
| [06-STATE.md](06-STATE.md) | M05 state.rs — StateStore trait, DatabasePool, QueryBuilder, 11 DatabaseTypes | ~2.5K |
| [07-RESOURCES.md](07-RESOURCES.md) | M06 resources.rs — ResourceCollector trait, SystemResources, adaptive limits | ~1.5K |
| [08-NAM-PRIMITIVES.md](08-NAM-PRIMITIVES.md) | NAM nam.rs — AgentOrigin, Confidence, Outcome, LearningSignal, Dissent | ~1.5K |
| [09-SIGNALS.md](09-SIGNALS.md) | M07 signals.rs — SignalSubscriber trait, SignalBus, 3 typed signal channels | ~2K |
| [10-TENSOR-REGISTRY.md](10-TENSOR-REGISTRY.md) | M08 tensor_registry.rs — TensorContributor trait, composition algorithm | ~2K |
| [11-ARCHITECTURAL-SCHEMATICS.md](11-ARCHITECTURAL-SCHEMATICS.md) | Mermaid diagrams: layer architecture, trait graph, signal flow, tensor pipeline, concurrency, cross-layer morphisms | ~4K |
| [12-META-TREE-MIND-MAP.md](12-META-TREE-MIND-MAP.md) | Exhaustive hierarchical decomposition: every type, trait, function, constant, pattern, invariant, relationship, test category | ~8K |

---

## Reading Protocol

```
QUICK START:    Read 00-LAYER-OVERVIEW.md (architecture + dependency graph)
WRITING CODE:   Read the specific module spec (01-10) for the module you're touching
CROSS-LAYER:    Read 11-ARCHITECTURAL-SCHEMATICS.md for morphism/signal/tensor diagrams
CONSUMING L1:   Read 01 (types) + 02 (errors) + 09 (signals) + 10 (tensors) — the 4 cross-cutting concerns
```

---

## Quick Reference — 8 Traits

| # | Trait | File | Bounds | Methods | Defaults |
|---|-------|------|--------|---------|----------|
| 1 | `ErrorClassifier` | error.rs | — | 5 | 2 |
| 2 | `ConfigProvider` | config.rs | Send+Sync | 5 | 2 |
| 3 | `CorrelationProvider` | logging.rs | Send+Sync | 3 | 1 |
| 4 | `MetricRecorder` | metrics.rs | Send+Sync | 4 | 0 |
| 5 | `StateStore` | state.rs | Send+Sync | 3 | 1 |
| 6 | `ResourceCollector` | resources.rs | Send+Sync | 5 | 2 |
| 7 | `SignalSubscriber` | signals.rs | Send+Sync+Debug | 4 | 3 |
| 8 | `TensorContributor` | tensor_registry.rs | Send+Sync+Debug | 3 | 0 |

---

## Quick Reference — Tensor Dimensions

| Dim | Name | L1 Contributors | L2 Contributors |
|-----|------|-----------------|-----------------|
| D0 | ServiceId | LogContext | M09 |
| D1 | Port | Config | — |
| D2 | Tier | All (1/6) | M09 |
| D3 | DependencyCount | — | M09 |
| D4 | AgentCount | — | M09 |
| D5 | Protocol | LogContext, Resources | — |
| D6 | HealthScore | Config, Resources, Metrics | M10, M11 |
| D7 | Uptime | — | M11 |
| D8 | Synergy | — | — (unused) |
| D9 | Latency | Resources | M12 |
| D10 | ErrorRate | Metrics, Resources | M10, M12 |
| D11 | TemporalContext | — | — (unused) |

---

---

## Errata & Notes (2026-03-01)

**LOC/Test count update:** Spec sheets were derived at commit `1a60c5e`. Subsequent development (L2 refactor integration tests, mod.rs test densification) increased L1 from ~12,908 LOC / 440 tests to **~16,711 LOC / 678 tests**. Per-file actuals:

| File | Spec LOC | Actual LOC | Spec Tests | Actual Tests |
|------|----------|------------|------------|--------------|
| mod.rs | ~800 | 1,307 | 71 | ~88 |
| shared_types.rs | ~1,210 | 1,209 | ~88 | ~88 |
| error.rs | ~1,530 | 1,528 | ~44 | ~44 |
| config.rs | ~1,114 | 1,871 | 14 | ~30 |
| logging.rs | ~854 | 1,497 | 15 | ~30 |
| metrics.rs | ~1,280 | 1,979 | 12 | ~25 |
| state.rs | ~1,209 | 2,105 | 10 | ~20 |
| resources.rs | ~1,271 | 2,006 | 16 | ~30 |
| nam.rs | ~707 | 707 | 35 | 35 |
| signals.rs | ~1,168 | 1,167 | ~55 | ~55 |
| tensor_registry.rs | ~1,335 | 1,335 | ~80 | ~80 |

**SignalBusOps / TensorRegistryOps — NOT IMPLEMENTED:** The L2 refactor plan (Phase 0B) specified extraction of `SignalBusOps` (7 methods) and `TensorRegistryOps` (5 methods) traits from `SignalBus` and `TensorRegistry`. These traits were **never created** — L2 modules use concrete `SignalBus` and `TensorRegistry` types directly. The system works correctly without them; they remain a potential future refactor if `Arc<dyn>` abstraction is needed at the bus/registry level.

---

*L1 Foundation Spec Sheet Index v1.1 | 2026-03-01*
