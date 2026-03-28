---
tags: [layer/L2, progressive-disclosure/L2, status/cloned]
---

# L2: Services Layer

> **Status:** CLONED (Gold Standard) | **LOC:** 7,196 | **Tests:** 279 | **Files:** 5

## Purpose

Service lifecycle management, health monitoring, resilience patterns, and load balancing. Provides the 6 core traits that every service-facing module must implement.

## Modules

| ID | Module | File | LOC | Tests | Role |
|----|--------|------|-----|-------|------|
| M09 | Service Registry | `service_registry.rs` | 1,285 | 53 | Discovery, registration, dependency tracking |
| M10 | Health Monitor | `health_monitor.rs` | 1,130 | 49 | Probes, aggregation, status reporting |
| M11 | Lifecycle Manager | `lifecycle.rs` | 1,898 | 75 | FSM transitions, restart with backoff |
| M12 | Resilience | `resilience.rs` | 2,189 | 82 | Circuit breaker FSM, load balancing |
| — | Coordinator | `mod.rs` | 694 | 20 | Layer init, trait re-exports |

## 6 Core Traits

| Trait | Methods | Implementors |
|-------|---------|-------------|
| `ServiceDiscovery` | 14 | M09 |
| `HealthMonitoring` | 11 | M10 |
| `LifecycleOps` | 13 | M11 |
| `CircuitBreakerOps` | 12 | M12 |
| `LoadBalancing` | 10 | M12 |
| `TensorContributor` | 3 | All modules |

## Circuit Breaker FSM

```
Closed --(failure_threshold)--> Open --(timeout)--> HalfOpen
HalfOpen --(success)--> Closed
HalfOpen --(failure)--> Open
```

## Audit History

- L2 Services audit: **91/100**
- Greptile final review: **96/100**
- PAI assessment: **97/100**

---

Full spec: `../ai_specs/m2-services-specs/` (7 files) | Module docs: `../ai_docs/modules/M09-M12`
See [[HOME]] | Prev: [[L1 — Foundation Layer]] | Next: [[L3 — Core Logic Layer]]
