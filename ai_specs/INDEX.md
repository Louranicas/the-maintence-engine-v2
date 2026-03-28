# Maintenance Engine V2 - AI Specifications Index

> 48+ module specifications across 8 layers + V3 homeostasis

---

## Layer Specifications

| Layer | Spec | Status | Target LOC |
|-------|------|--------|-----------|
| L1 Foundation | `m1-foundation-specs/` | CLONED (gold standard) | 16,711 |
| L2 Services | `m2-services-specs/` | CLONED (gold standard) | 7,196 |
| L3 Core Logic | `m3-core-logic-specs/L3_CORE_LOGIC_SPEC.md` | SPECIFIED | ~8,000 |
| L4 Integration | `m4-integration-specs/L4_INTEGRATION_SPEC.md` | SPECIFIED | ~7,500 |
| L5 Learning | `m5-learning-specs/L5_LEARNING_SPEC.md` | SPECIFIED | ~7,000 |
| L6 Consensus | `m6-consensus-specs/L6_CONSENSUS_SPEC.md` | SPECIFIED | ~6,500 |
| L7 Observer | `m7-observer-specs/L7_OBSERVER_SPEC.md` | SPECIFIED | ~8,500 |
| L8 Nexus | `nexus-specs/L8_NEXUS_SPEC.md` | SPECIFIED | ~6,000 |

---

## Cross-Cutting Specifications

| Spec | File | Description |
|------|------|-------------|
| **M1+M2 Unified** | `M1_M2_UNIFIED_ARCHITECTURE.md` | **Cross-layer integration diagrams, trait index, FSM reference, implementation templates** |
| API | `API_SPEC.md` | REST endpoints, request/response schemas |
| Database | `DATABASE_SPEC.md` | 12 database schemas, migration strategy |
| Escalation | `ESCALATION_SPEC.md` | L0-L3 escalation tiers |
| Hooks | `HOOKS_SPEC.md` | 14 Claude Code hooks |
| Layer | `LAYER_SPEC.md` | 8-layer architecture with module matrix |
| Module Matrix | `MODULE_MATRIX.md` | 48+ module cross-reference |
| NAM | `NAM_SPEC.md` | R1-R5 compliance targets (95%) |
| PBFT | `PBFT_SPEC.md` | Consensus protocol (n=40, f=13, q=27) |
| Pipeline | `PIPELINE_SPEC.md` | 8 core pipelines with SLOs |
| Security | `SECURITY_SPEC.md` | Token types, rate limiting |
| Service | `SERVICE_SPEC.md` | 12 ULTRAPLATE service definitions |
| STDP | `STDP_SPEC.md` | Learning parameters, HRS-001 fix |
| System | `SYSTEM_SPEC.md` | System-wide architecture overview |
| Tensor | `TENSOR_SPEC.md` | 12D tensor encoding |

---

## Pattern Specifications

| Pattern | File | Description |
|---------|------|-------------|
| Builder | `patterns/BUILDER.md` | Builder pattern for all constructors |
| Circuit Breaker | `patterns/CIRCUIT_BREAKER.md` | FSM: Closed→Open→HalfOpen→Closed |
| Error Handling | `patterns/ERROR_HANDLING.md` | Result<T> everywhere |
| Event Sourcing | `patterns/EVENT_SOURCING.md` | Event-driven state management |
| Interior Mutability | `patterns/INTERIOR_MUTABILITY.md` | RwLock + &self traits |
| Observer | `patterns/OBSERVER.md` | Pub/sub event distribution |
| Pipeline | `patterns/PIPELINE.md` | Stage-based processing |
| Repository | `patterns/REPOSITORY.md` | Database access patterns |
| Retry | `patterns/RETRY.md` | Exponential backoff with jitter |
| Signal | `patterns/SIGNAL.md` | Signal bus emission |
| State Machine | `patterns/STATE_MACHINE.md` | FSM transitions |
| Tensor | `patterns/TENSOR.md` | TensorContributor trait pattern |

---

## Nexus-Specific Specifications

| Spec | File | Description |
|------|------|-------------|
| Layer Spec | `nexus-specs/L8_NEXUS_SPEC.md` | Full L8 layer specification |
| N01 Field Bridge | `nexus-specs/N01_FIELD_BRIDGE.md` | Kuramoto r-tracking |
| N02 Intent Router | `nexus-specs/N02_INTENT_ROUTER.md` | 12D tensor routing |
| N03 Regime Manager | `nexus-specs/N03_REGIME_MANAGER.md` | K-regime awareness |
| N04 STDP Bridge | `nexus-specs/N04_STDP_BRIDGE.md` | Tool chain learning |
| N05 Evolution Gate | `nexus-specs/N05_EVOLUTION_GATE.md` | Mutation testing |
| N06 Morphogenic | `nexus-specs/N06_MORPHOGENIC_ADAPTER.md` | Adaptation triggers |

---

## Evolution Chamber Specifications

Located in `evolution_chamber_ai_specs/`:
- RALPH loop protocol
- Mutation strategies
- Fitness evaluation criteria
- Acceptance thresholds

---

## Design Constraints (C1-C12)

| ID | Constraint | Spec Reference |
|----|-----------|---------------|
| C1 | No upward imports | `LAYER_SPEC.md` |
| C2 | `&self` traits | `patterns/INTERIOR_MUTABILITY.md` |
| C3 | TensorContributor | `patterns/TENSOR.md` |
| C4 | Zero unsafe/unwrap/expect | `SYSTEM_SPEC.md` |
| C5 | No chrono/SystemTime | `SYSTEM_SPEC.md` |
| C6 | Signal bus emissions | `patterns/SIGNAL.md` |
| C7 | Owned returns through RwLock | `patterns/INTERIOR_MUTABILITY.md` |
| C8 | Duration timeouts | `SYSTEM_SPEC.md` |
| C9 | No downstream test breakage | `SYSTEM_SPEC.md` |
| C10 | 50+ tests per layer | `SYSTEM_SPEC.md` |
| C11 | Nexus field capture (L4+) | `nexus-specs/L8_NEXUS_SPEC.md` |
| C12 | STDP co-activation recording | `nexus-specs/N04_STDP_BRIDGE.md` |
