# M2 Services Layer — Specification Index

> Layer 2 (Services) of The Maintenance Engine v1.0.0
> Generated: 2026-03-01 | 30-Facet Taxonomy Extraction

---

## Navigation

| Spec | Module | Focus | Status |
|------|--------|-------|--------|
| [M2_LAYER_SPEC](M2_LAYER_SPEC.md) | L2 Overview | Layer architecture, wiring, constraints | COMPLETE |
| [M09_SERVICE_REGISTRY_SPEC](M09_SERVICE_REGISTRY_SPEC.md) | M09 | Service discovery & registration | COMPLETE |
| [M10_HEALTH_MONITOR_SPEC](M10_HEALTH_MONITOR_SPEC.md) | M10 | Health check orchestration & FSM | COMPLETE |
| [M11_LIFECYCLE_SPEC](M11_LIFECYCLE_SPEC.md) | M11 | Service lifecycle FSM & restart backoff | COMPLETE |
| [M12_RESILIENCE_SPEC](M12_RESILIENCE_SPEC.md) | M12 | Circuit breaker & load balancer | COMPLETE |
| [M2_ARCHITECTURAL_SCHEMATICS](M2_ARCHITECTURAL_SCHEMATICS.md) | All | Mermaid diagrams & visual architecture | COMPLETE |
| [M2-META-TREE-MIND-MAP](M2-META-TREE-MIND-MAP.md) | All | Exhaustive hierarchical decomposition (1,073 lines) | COMPLETE |

## Layer Summary

| Metric | Value |
|--------|-------|
| **Files** | 5 (mod.rs + 4 modules) |
| **LOC** | 7,196 |
| **Traits** | 6 (ServiceDiscovery, HealthMonitoring, LifecycleOps, CircuitBreakerOps, LoadBalancing, TensorContributor) |
| **Tests** | 320 (279 unit + 41 integration) |
| **Tensor Dimensions** | D0, D2, D3, D4, D6, D7, D9, D10 (8 of 12) |
| **Constraint Compliance** | C1-C10 all satisfied |
| **Clippy Status** | 0 warnings (pedantic + nursery) |

## Cross-References

- **Upstream:** [M1 Foundation Specs](../m1-foundation-specs/)
- **Downstream:** L3 Core Logic (M13-M18)
- **System:** [SYSTEM_SPEC](../SYSTEM_SPEC.md), [LAYER_SPEC](../LAYER_SPEC.md), [MODULE_MATRIX](../MODULE_MATRIX.md)
- **Patterns:** [RUST_CORE_PATTERNS](../patterns/RUST_CORE_PATTERNS.md), [CONCURRENCY_PATTERNS](../patterns/CONCURRENCY_PATTERNS.md)
- **Tensor:** [TENSOR_SPEC](../TENSOR_SPEC.md)
- **Schematics:** [M2_ARCHITECTURAL_SCHEMATICS](M2_ARCHITECTURAL_SCHEMATICS.md) (7 Mermaid diagrams)

---

*Generated: 2026-03-01 | The Maintenance Engine v1.0.0*
