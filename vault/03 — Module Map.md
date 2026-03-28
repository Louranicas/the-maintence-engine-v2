---
tags: [nav/modules, progressive-disclosure/L1]
---

# Module Map (48+ Modules)

## L1: Foundation (CLONED)

| ID | Module | File | LOC | Tests | Status |
|----|--------|------|-----|-------|--------|
| M00 | Shared Types | `shared_types.rs` | 1,049 | 30 | CLONED |
| M01 | Error Taxonomy | `error.rs` | 1,396 | 4 | CLONED |
| M02 | Configuration | `config.rs` | 1,755 | 14 | CLONED |
| M03 | Logging | `logging.rs` | 854 | 15 | CLONED |
| M04 | Metrics | `metrics.rs` | 1,920 | 12 | CLONED |
| M05 | State Persistence | `state.rs` | 2,024 | 10 | CLONED |
| M06 | Resource Manager | `resources.rs` | 1,906 | 16 | CLONED |
| M07 | Signal Bus | `signals.rs` | 1,111 | 30 | CLONED |
| M08 | Tensor Registry | `tensor_registry.rs` | 1,349 | 25 | CLONED |
| — | NAM Foundation | `nam.rs` | 645 | 20 | CLONED |
| — | Coordinator | `mod.rs` | 2,702 | — | CLONED |
| | **L1 Total** | | **16,711** | **176** | |

## L2: Services (CLONED)

| ID | Module | File | LOC | Tests | Status |
|----|--------|------|-----|-------|--------|
| M09 | Service Registry | `service_registry.rs` | 1,285 | 53 | CLONED |
| M10 | Health Monitor | `health_monitor.rs` | 1,130 | 49 | CLONED |
| M11 | Lifecycle Manager | `lifecycle.rs` | 1,898 | 75 | CLONED |
| M12 | Resilience | `resilience.rs` | 2,189 | 82 | CLONED |
| — | Coordinator | `mod.rs` | 694 | 20 | CLONED |
| | **L2 Total** | | **7,196** | **279** | |

## L3: Core Logic (PENDING)

| ID | Module | File | Target LOC | Target Tests |
|----|--------|------|-----------|-------------|
| M13 | Pipeline Manager | `pipeline.rs` | 1,500+ | 50+ |
| M14 | Remediation Engine | `remediation.rs` | 1,500+ | 50+ |
| M15 | Confidence Calculator | `confidence.rs` | 1,200+ | 50+ |
| M16 | Action Executor | `action.rs` | 1,500+ | 50+ |
| M17 | Outcome Recorder | `outcome.rs` | 900+ | 50+ |
| M18 | Feedback Loop | `feedback.rs` | 1,000+ | 50+ |

## L4: Integration (PENDING)

| ID | Module | File | Target LOC | Target Tests |
|----|--------|------|-----------|-------------|
| M19 | REST Client | `rest.rs` | 600+ | 50+ |
| M20 | gRPC Client | `grpc.rs` | 1,200+ | 50+ |
| M21 | WebSocket Client | `websocket.rs` | 1,000+ | 50+ |
| M22 | IPC Manager | `ipc.rs` | 1,000+ | 50+ |
| M23 | Event Bus | `event_bus.rs` | 700+ | 50+ |
| M24 | Bridge Manager | `bridge.rs` | 800+ | 50+ |

## L5: Learning (PENDING)

| ID | Module | File | Target LOC | Target Tests |
|----|--------|------|-----------|-------------|
| M25 | Hebbian Manager | `hebbian.rs` | 900+ | 50+ |
| M26 | STDP Processor | `stdp.rs` | 700+ | 50+ |
| M27 | Pattern Recognizer | `pattern.rs` | 1,000+ | 50+ |
| M28 | Pathway Pruner | `pruner.rs` | 1,300+ | 50+ |
| M29 | Memory Consolidator | `consolidator.rs` | 1,600+ | 50+ |
| M30 | Anti-Pattern Detector | `antipattern.rs` | 800+ | 50+ |

## L6: Consensus (PENDING)

| ID | Module | File | Target LOC | Target Tests |
|----|--------|------|-----------|-------------|
| M31 | PBFT Manager | `pbft.rs` | 800+ | 50+ |
| M32 | Agent Coordinator | `agent.rs` | 1,200+ | 50+ |
| M33 | Vote Collector | `voting.rs` | 700+ | 50+ |
| M34 | View Change Handler | `view_change.rs` | 1,100+ | 50+ |
| M35 | Dissent Tracker | `dissent.rs` | 800+ | 50+ |
| M36 | Quorum Calculator | `quorum.rs` | 1,100+ | 50+ |

## L7: Observer (PENDING)

| ID | Module | File | Target LOC | Target Tests |
|----|--------|------|-----------|-------------|
| M37 | Observer Bus | `observer_bus.rs` | 1,000+ | 50+ |
| M38 | Fitness Evaluator | `fitness.rs` | 1,000+ | 50+ |
| M39 | Log Correlator | `log_correlator.rs` | 1,300+ | 50+ |
| M40 | Emergence Detector | `emergence_detector.rs` | 1,800+ | 50+ |
| M41 | Evolution Chamber | `evolution_chamber.rs` | 1,600+ | 50+ |
| M42 | Thermal Monitor | `thermal_monitor.rs` | 400+ | 10+ |

## L8: Nexus — NEW (PENDING)

| ID | Module | File | Target LOC | Target Tests | Source |
|----|--------|------|-----------|-------------|--------|
| N01 | Field Bridge | `field_bridge.rs` | 800+ | 50+ | VMS HookEngine |
| N02 | Intent Router | `intent_router.rs` | 600+ | 50+ | VMS IntentEncoder |
| N03 | Regime Manager | `regime_manager.rs` | 500+ | 50+ | VMS SwarmCoordinator |
| N04 | STDP Bridge | `stdp_bridge.rs` | 700+ | 50+ | VMS StdpKernel |
| N05 | Evolution Gate | `evolution_gate.rs` | 600+ | 50+ | VMS EvolutionChamber |
| N06 | Morphogenic Adapter | `morphogenic_adapter.rs` | 500+ | 50+ | VMS MorphogenicEngine |

## V3: Homeostasis (PENDING)

| ID | Module | File | Target LOC | Target Tests |
|----|--------|------|-----------|-------------|
| M43 | Thermal Controller | `thermal.rs` | 400+ | 10+ |
| M44 | Decay Auditor | `decay_auditor.rs` | 400+ | 10+ |
| M45 | Diagnostics Engine | `diagnostics.rs` | 400+ | 10+ |

---

See [[HOME]] | Full specs: `../ai_docs/modules/` | Layer details: [[L1 — Foundation Layer]] etc.
