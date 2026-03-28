---
tags: [layer/L1, progressive-disclosure/L2, status/cloned]
---

# L1: Foundation Layer

> **Status:** CLONED (Gold Standard) | **LOC:** 16,711 | **Tests:** 176 | **Files:** 11

## Purpose

The foundation layer provides all shared types, error handling, configuration, logging, metrics, state persistence, resource management, signal bus, tensor registry, and NAM primitives. Every other layer depends on L1.

## Modules

| ID | Module | File | LOC | Role |
|----|--------|------|-----|------|
| M00 | Shared Types | `shared_types.rs` | 1,049 | `Timestamp`, `Duration`, `ServiceId`, `AgentId` |
| M01 | Error Taxonomy | `error.rs` | 1,396 | `MaintenanceError` enum, error codes, recovery hints |
| M02 | Configuration | `config.rs` | 1,755 | TOML-based config, runtime reload, validation |
| M03 | Logging | `logging.rs` | 854 | Structured `tracing` with span context |
| M04 | Metrics | `metrics.rs` | 1,920 | Prometheus-compatible counters, histograms, gauges |
| M05 | State Persistence | `state.rs` | 2,024 | SQLite-backed state with transactional writes |
| M06 | Resource Manager | `resources.rs` | 1,906 | CPU, memory, disk tracking with thresholds |
| M07 | Signal Bus | `signals.rs` | 1,111 | `Arc<dyn SignalBusOps>`, typed signal emission |
| M08 | Tensor Registry | `tensor_registry.rs` | 1,349 | 12D tensor aggregation from all modules |
| — | NAM Foundation | `nam.rs` | 645 | NAM axiom types, compliance tracking |
| — | Coordinator | `mod.rs` | 2,702 | Layer init, cross-module wiring |

## Key Types Exported

```
Timestamp, Duration, ServiceId, AgentId, TensorContributor,
MaintenanceError, Result<T>, Config, SignalBus, TensorRegistry,
MetricsCollector, StateManager, ResourceManager, NamFoundation
```

## 12D Tensor Contributions

- M09 -> D0 (service_id), D2 (tier), D3 (deps), D4 (agents)
- M10 -> D6 (health), D10 (error_rate)
- M11 -> D6 (health), D7 (uptime)
- M12 -> D9 (latency), D10 (error_rate)

---

Full spec: `../ai_specs/m1-foundation-specs/` (13 files) | Module docs: `../ai_docs/modules/M01-M08`
See [[HOME]] | [[03 — Module Map]] | Next: [[L2 — Services Layer]]
