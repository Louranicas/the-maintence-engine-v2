# L7 Observer — Spec Sheet Index

> **Layer:** L7 Observer | **Modules:** M37-M39 + infrastructure (5 modules, 6 files)
> **LOC:** ~8,500 (target) | **Tests:** 350+ (target) | **Quality Score:** PENDING
> **Status:** SPECIFIED — awaiting implementation | **Verified:** 2026-03-06

---

## Document Map

| Document | Description | Tokens |
|----------|-------------|--------|
| [L7_OBSERVER_SPEC.md](L7_OBSERVER_SPEC.md) | Full layer specification: observer bus, fitness evaluator, log correlation, emergence detection, RALPH evolution chamber | ~3K |

---

## Reading Protocol

```
QUICK START:    Read L7_OBSERVER_SPEC.md (observability architecture + RALPH loop)
WRITING CODE:   Read the module section relevant to what you're implementing
CROSS-LAYER:    L7 aggregates data from ALL lower layers (L1-L6) into unified fitness evaluation
CONSUMING L7:   Fitness Evaluator + Emergence Detector (M38) + Evolution Chamber (M39) — the observation triad
```

---

## Module Table

| # | Module | ID | File | Target LOC | Target Tests | Status |
|---|--------|----|------|-----------|-------------|--------|
| 1 | Observer Bus | — | `observer_bus.rs` | ~1,000 | 50+ | PENDING |
| 2 | Fitness Evaluator | — | `fitness.rs` | ~1,000 | 50+ | PENDING |
| 3 | Log Correlator | M37 | `log_correlator.rs` | ~1,300 | 50+ | PENDING |
| 4 | Emergence Detector | M38 | `emergence_detector.rs` | ~1,800 | 50+ | PENDING |
| 5 | Evolution Chamber | M39 | `evolution_chamber.rs` | ~1,600 | 50+ | PENDING |
| 6 | Layer Coordinator | — | `mod.rs` | ~1,300 | 55+ | PENDING |
| | **Subtotal** | | | **~8,000** | **~305** | |

---

## Quick Reference — 4 Traits

| # | Trait | File | Bounds | Methods | Defaults |
|---|-------|------|--------|---------|----------|
| 1 | `ObserverBusOps` | observer_bus.rs | Send+Sync | 4 | 0 |
| 2 | `LogCorrelation` | log_correlator.rs | Send+Sync | 4 | 0 |
| 3 | `EmergenceDetection` | emergence_detector.rs | Send+Sync | 3 | 0 |
| 4 | `TensorContributor` | (all modules) | Send+Sync+Debug | 3 | 0 |

---

## Quick Reference — Tensor Dimensions

| Dim | Name | L7 Contributors | Notes |
|-----|------|-----------------|-------|
| D0 | ServiceId | Fitness Evaluator (all) | Aggregated from all layers |
| D1 | Port | Fitness Evaluator (all) | Aggregated from all layers |
| D2 | Tier | Fitness Evaluator (all) | Aggregated from all layers |
| D3 | DependencyCount | Fitness Evaluator (all) | Aggregated from all layers |
| D4 | AgentCount | Fitness Evaluator (all) | Aggregated from all layers |
| D5 | Protocol | Fitness Evaluator (all) | Aggregated from all layers |
| D6 | HealthScore | Fitness Evaluator (all) | Aggregated from all layers |
| D7 | Uptime | Fitness Evaluator (all) | Aggregated from all layers |
| D8 | Synergy | Fitness Evaluator (all) | Aggregated from all layers |
| D9 | Latency | Fitness Evaluator (all) | Aggregated from all layers |
| D10 | ErrorRate | Fitness Evaluator (all) | Aggregated from all layers |
| D11 | TemporalContext | Fitness Evaluator (all) | Aggregated from all layers |

**Note:** The Fitness Evaluator is the terminal tensor aggregator -- it composes contributions from ALL 12 dimensions across all layers into a single `FitnessSnapshot`.

---

## Observer Event Types

| Event | Source | Description |
|-------|--------|-------------|
| `FitnessUpdated` | Fitness Evaluator | New fitness snapshot computed |
| `EmergenceDetected` | M38 | Emergent behavior identified |
| `EvolutionCompleted` | M39 | RALPH cycle completed |
| `CorrelationFound` | M37 | Cross-layer correlation discovered |
| `AnomalyDetected` | M37/M38 | Anomalous system behavior |

---

## Emergence Types

| Type | Detection | V2 Enhancement |
|------|-----------|----------------|
| Cascade | Multi-service failure/recovery propagation | — |
| Synergy | Unexpected positive interaction | — |
| Resonance | Synchronized oscillation between services | — |
| PhaseShift | Sudden system-wide state change | — |
| Chimera | Mixed coherent/incoherent state | Kuramoto r analysis (0.3 < r < 0.7) |

---

## RALPH Evolution Loop (5 Phases)

```
1. SNAPSHOT:   Capture current system state and fitness
2. MUTATE:     Apply random parameter perturbation
3. EVALUATE:   Measure fitness delta after mutation
4. SELECT:     Accept if fitness improved, reject otherwise
5. ARCHIVE:    Record result in evolution_tracking.db
```

**V2 Evolution Gate (N05):** K=1.0, 500 steps, 5 spheres. Accept only if `r_after >= r_baseline`.

---

## Log Correlation Methods

| Method | Description |
|--------|-------------|
| Temporal proximity | Events within configurable time window |
| Dependency chain | Following service dependency graph |
| Error propagation | Tracing error spread across layers |
| Periodic recurrence | FFT-based frequency detection |

---

## HTTP Endpoints (provided by mod.rs)

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/api/observer` | GET | Full observation report |
| `/api/fitness` | GET | Current fitness snapshot |
| `/api/emergence` | GET | Recent emergence events |
| `/api/evolution` | GET | Evolution status and history |

---

## Design Constraints

| ID | Constraint | Enforcement |
|----|-----------|-------------|
| C1 | Can import from ALL lower layers (L1-L6) | Compile-time module DAG |
| C2 | All trait methods `&self` | Code review |
| C3 | `TensorContributor` on Fitness Evaluator (all 12 dimensions) | Compile-time |
| C4 | Zero unsafe/unwrap/expect | `#![forbid(unsafe_code)]` + clippy deny |
| C11 | Observer integrates with N01 field bridge for r-tracking | Architecture |
| V2 | Chimera state detection for Kuramoto field analysis | M38 enhancement |

---

## Databases

| Database | Module | Usage |
|----------|--------|-------|
| `tensor_memory.db` | Fitness Evaluator | Fitness snapshot persistence |
| `evolution_tracking.db` | M39 | RALPH evolution records (19,803 existing rows) |
| `episodic_memory.db` | M37 | Log correlation source data |

---

## Cross-References

- **Upstream:** [M1 Foundation](../m1-foundation-specs/) | [M2 Services](../m2-services-specs/) | [M3 Core Logic](../m3-core-logic-specs/) | [M4 Integration](../m4-integration-specs/) | [M5 Learning](../m5-learning-specs/) | [M6 Consensus](../m6-consensus-specs/)
- **Downstream:** L8 Nexus (N01-N06)
- **System:** [SYSTEM_SPEC](../SYSTEM_SPEC.md), [LAYER_SPEC](../LAYER_SPEC.md), [MODULE_MATRIX](../MODULE_MATRIX.md)
- **Tensor:** [TENSOR_SPEC](../TENSOR_SPEC.md) (12D encoding reference)
- **NAM:** [NAM_SPEC](../NAM_SPEC.md) (R1 SelfQuery via fitness, R4 FieldVisualization via tensor)
- **Database:** [DATABASE_SPEC](../DATABASE_SPEC.md) (tensor_memory.db, evolution_tracking.db, episodic_memory.db)
- **Nexus:** [N01 Field Bridge](../nexus-specs/N01_FIELD_BRIDGE.md) | [N05 Evolution Gate](../nexus-specs/N05_EVOLUTION_GATE.md)
- **Evolution:** [Evolution Chamber Specs](../evolution_chamber_ai_specs/) (RALPH protocol, mutation strategies)
- **Tests:** `tests/l7_observer_integration.rs` | **Bench:** `benches/tensor_encoding.rs`

---

*L7 Observer Spec Sheet Index v1.0 | 2026-03-06*
