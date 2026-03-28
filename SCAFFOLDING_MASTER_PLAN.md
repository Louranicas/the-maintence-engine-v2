# Maintenance Engine V2 — Master Scaffolding Plan

> God-Tier Scaffolding Blueprint | Nexus + OVM Synergy | Full Feature Set
> Generated: 2026-03-06

---

## Executive Summary

The Maintenance Engine V2 is the next-generation maintenance framework for the ULTRAPLATE Developer Environment, evolved from ME v1 (45 modules, 54K LOC, 1,536 tests) with deep Nexus Controller and Oscillating Vortex Memory integration.

**V2 Enhancements over V1:**
- Nexus Controller integration (ControlNexus trait, IntentTensor routing, SwarmDirective)
- Oscillating Vortex Memory field coherence monitoring (Kuramoto r-tracking)
- STDP tool chain learning from VMS patterns
- Evolution Chamber mutation testing before deployments
- Morphogenic adaptation triggers (|r_delta| > 0.05)
- K-regime awareness (Swarm/Fleet/Armada)
- 384D semantic drift detection via Saturn Light
- Cross-session learning persistence

---

## Gold Standard Exemplars

| Source | Status | What It Provides |
|--------|--------|------------------|
| ME v1 `m1_foundation/` | CLONED (16,711 LOC) | Foundation layer — error, config, logging, metrics, state, resources, signals, tensor, NAM |
| ME v1 `m2_services/` | CLONED (7,196 LOC) | Services layer — registry, health, lifecycle, resilience (6 traits, 279 tests) |
| ME v1 `m3_core_logic/` | TEMPLATE (7,902 LOC) | Core logic patterns — pipeline, remediation, confidence, action, outcome, feedback |
| ME v1 `m4_integration/` | TEMPLATE (5,460 LOC) | Integration patterns — REST, gRPC, WebSocket, IPC, event bus, bridge |
| ME v1 `m5_learning/` | TEMPLATE (6,494 LOC) | Learning patterns — Hebbian, STDP, pattern, pruner, consolidator, antipattern |
| ME v1 `m6_consensus/` | TEMPLATE (6,051 LOC) | Consensus patterns — PBFT, agent, voting, view change, dissent, quorum |
| ME v1 `m7_observer/` | TEMPLATE (8,005 LOC) | Observer patterns — bus, fitness, correlator, emergence, evolution |
| DevOps Engine v2 | REFERENCE | 13 modules, 741 tests, 7-phase pipeline, error taxonomy (56 codes) |
| VMS Nexus | REFERENCE | SDK Bridge, HookEngine, STDP, ToolChain, EvolutionChamber, SwarmCoordinator |
| SVF Nexus | REFERENCE | Collision detection, query routing, synergy tracking |

---

## Architecture: 8 Layers, 48+ Modules

### Layer Map

```
L8: NEXUS (NEW)      — Nexus Controller bridge, field coherence, K-regime
L7: OBSERVER          — Log correlation, emergence, evolution, thermal
L6: CONSENSUS          — PBFT, agents, voting, view change, dissent, quorum
L5: LEARNING           — Hebbian, STDP, pattern, pruner, consolidator, antipattern
L4: INTEGRATION        — REST, gRPC, WebSocket, IPC, event bus, bridge, peer, tools
L3: CORE LOGIC         — Pipeline, remediation, confidence, action, outcome, feedback
L2: SERVICES           — Registry, health, lifecycle, resilience (CLONED)
L1: FOUNDATION         — Error, config, logging, metrics, state, resources (CLONED)
V3: HOMEOSTASIS        — Thermal, decay auditor, diagnostics (HRS-001)
```

### New L8: Nexus Integration Layer

| Module | File | Purpose | Source Pattern |
|--------|------|---------|---------------|
| N01 | `field_bridge.rs` | Kuramoto r-tracking, pre/post field capture | VMS HookEngine |
| N02 | `intent_router.rs` | 12D IntentTensor → service routing | VMS IntentEncoder |
| N03 | `regime_manager.rs` | K-regime detection (Swarm/Fleet/Armada) | VMS SwarmCoordinator |
| N04 | `stdp_bridge.rs` | Tool chain STDP learning from service interactions | VMS StdpKernel |
| N05 | `evolution_gate.rs` | Mutation testing before deployments | VMS EvolutionChamber |
| N06 | `morphogenic_adapter.rs` | Adaptation triggers on |r_delta| > 0.05 | VMS MorphogenicEngine |
| mod.rs | `mod.rs` | Layer coordinator + NexusStatus aggregate | ME v1 pattern |

---

## Design Constraints (C1-C12)

Inherited from M1/M2 gold standard + V2 extensions:

| ID | Constraint | Enforcement |
|----|-----------|-------------|
| C1 | No upward imports (strict DAG) | Compile-time |
| C2 | Trait methods always `&self` (interior mutability) | Code review |
| C3 | Every module implements `TensorContributor` | Compile-time |
| C4 | Zero unsafe, unwrap, expect, clippy warnings | `#![forbid(unsafe_code)]` + clippy deny |
| C5 | No `chrono` or `SystemTime` — use `Timestamp` + `Duration` | Grep + clippy |
| C6 | Signal emissions via `Arc<SignalBus>` | Architecture |
| C7 | Owned returns through `RwLock` | Code review |
| C8 | Timeouts use `std::time::Duration` | Code review |
| C9 | Existing downstream tests must not break | CI gate |
| C10 | 50+ tests per layer minimum | CI gate |
| C11 | **NEW:** Every L4+ module has Nexus field capture | Architecture |
| C12 | **NEW:** All service interactions record STDP co-activation | Architecture |

---

## 12D Tensor Encoding (Inherited + Enhanced)

| Dim | Name | Range | Layer Contributions |
|-----|------|-------|-------------------|
| D0 | service_id | 0-1 | M09 |
| D1 | port | 0-1 | M09 |
| D2 | tier | 0-1 | M09 |
| D3 | dependency_count | 0-1 | M09 |
| D4 | agent_count | 0-1 | M09 |
| D5 | protocol | 0-1 | M19-M22 |
| D6 | health_score | 0-1 | M10, M11 |
| D7 | uptime | 0-1 | M11 |
| D8 | synergy | 0-1 | N04 (STDP bridge) |
| D9 | latency | 0-1 | M12 |
| D10 | error_rate | 0-1 | M10, M12 |
| D11 | temporal_context | 0-1 | N01 (field bridge) |

---

## Module Specifications (48 Modules)

### L1: Foundation (M01-M06 + M00, M07, M08) — CLONED

| ID | Module | File | LOC | Tests | Status |
|----|--------|------|-----|-------|--------|
| M00 | Shared Types | shared_types.rs | 1,049 | 30 | CLONED |
| M01 | Error Taxonomy | error.rs | 1,396 | 4 | CLONED |
| M02 | Configuration | config.rs | 1,755 | 14 | CLONED |
| M03 | Logging | logging.rs | 854 | 15 | CLONED |
| M04 | Metrics | metrics.rs | 1,920 | 12 | CLONED |
| M05 | State Persistence | state.rs | 2,024 | 10 | CLONED |
| M06 | Resource Manager | resources.rs | 1,906 | 16 | CLONED |
| M07 | Signal Bus | signals.rs | 1,111 | 30 | CLONED |
| M08 | Tensor Registry | tensor_registry.rs | 1,349 | 25 | CLONED |
| — | NAM Foundation | nam.rs | 645 | 20 | CLONED |

### L2: Services (M09-M12) — CLONED

| ID | Module | File | LOC | Tests | Status |
|----|--------|------|-----|-------|--------|
| M09 | Service Registry | service_registry.rs | 1,285 | 53 | CLONED |
| M10 | Health Monitor | health_monitor.rs | 1,130 | 49 | CLONED |
| M11 | Lifecycle Manager | lifecycle.rs | 1,898 | 75 | CLONED |
| M12 | Resilience | resilience.rs | 2,189 | 82 | CLONED |

### L3: Core Logic (M13-M18) — TO BUILD

| ID | Module | File | Target LOC | Target Tests | Template |
|----|--------|------|-----------|-------------|----------|
| M13 | Pipeline Manager | pipeline.rs | 1,500+ | 50+ | ME v1 M13 |
| M14 | Remediation Engine | remediation.rs | 1,500+ | 50+ | ME v1 M14 |
| M15 | Confidence Calculator | confidence.rs | 1,200+ | 50+ | ME v1 M15 |
| M16 | Action Executor | action.rs | 1,500+ | 50+ | ME v1 M16 |
| M17 | Outcome Recorder | outcome.rs | 900+ | 50+ | ME v1 M17 |
| M18 | Feedback Loop | feedback.rs | 1,000+ | 50+ | ME v1 M18 |

### L4: Integration (M19-M24) — TO BUILD

| ID | Module | File | Target LOC | Target Tests | Template |
|----|--------|------|-----------|-------------|----------|
| M19 | REST Client | rest.rs | 600+ | 50+ | ME v1 M19 |
| M20 | gRPC Client | grpc.rs | 1,200+ | 50+ | ME v1 M20 |
| M21 | WebSocket Client | websocket.rs | 1,000+ | 50+ | ME v1 M21 |
| M22 | IPC Manager | ipc.rs | 1,000+ | 50+ | ME v1 M22 |
| M23 | Event Bus | event_bus.rs | 700+ | 50+ | ME v1 M23 |
| M24 | Bridge Manager | bridge.rs | 800+ | 50+ | ME v1 M24 |

### L5: Learning (M25-M30) — TO BUILD

| ID | Module | File | Target LOC | Target Tests | Template |
|----|--------|------|-----------|-------------|----------|
| M25 | Hebbian Manager | hebbian.rs | 900+ | 50+ | ME v1 M25 |
| M26 | STDP Processor | stdp.rs | 700+ | 50+ | ME v1 M26 |
| M27 | Pattern Recognizer | pattern.rs | 1,000+ | 50+ | ME v1 M27 |
| M28 | Pathway Pruner | pruner.rs | 1,300+ | 50+ | ME v1 M28 |
| M29 | Memory Consolidator | consolidator.rs | 1,600+ | 50+ | ME v1 M29 |
| M30 | Anti-Pattern Detector | antipattern.rs | 800+ | 50+ | ME v1 M30 |

### L6: Consensus (M31-M36) — TO BUILD

| ID | Module | File | Target LOC | Target Tests | Template |
|----|--------|------|-----------|-------------|----------|
| M31 | PBFT Manager | pbft.rs | 800+ | 50+ | ME v1 M31 |
| M32 | Agent Coordinator | agent.rs | 1,200+ | 50+ | ME v1 M32 |
| M33 | Vote Collector | voting.rs | 700+ | 50+ | ME v1 M33 |
| M34 | View Change Handler | view_change.rs | 1,100+ | 50+ | ME v1 M34 |
| M35 | Dissent Tracker | dissent.rs | 800+ | 50+ | ME v1 M35 |
| M36 | Quorum Calculator | quorum.rs | 1,100+ | 50+ | ME v1 M36 |

### L7: Observer (M37-M42) — TO BUILD

| ID | Module | File | Target LOC | Target Tests | Template |
|----|--------|------|-----------|-------------|----------|
| M37 | Observer Bus | observer_bus.rs | 1,000+ | 50+ | ME v1 Observer Bus |
| M38 | Fitness Evaluator | fitness.rs | 1,000+ | 50+ | ME v1 Fitness |
| M39 | Log Correlator | log_correlator.rs | 1,300+ | 50+ | ME v1 M37 |
| M40 | Emergence Detector | emergence_detector.rs | 1,800+ | 50+ | ME v1 M38 |
| M41 | Evolution Chamber | evolution_chamber.rs | 1,600+ | 50+ | ME v1 M39 |
| M42 | Thermal Monitor | thermal_monitor.rs | 400+ | 10+ | ME v1 HRS-001 |

### L8: Nexus (N01-N06) — NEW, TO BUILD

| ID | Module | File | Target LOC | Target Tests | Source |
|----|--------|------|-----------|-------------|--------|
| N01 | Field Bridge | field_bridge.rs | 800+ | 50+ | VMS HookEngine |
| N02 | Intent Router | intent_router.rs | 600+ | 50+ | VMS IntentEncoder |
| N03 | Regime Manager | regime_manager.rs | 500+ | 50+ | VMS SwarmCoordinator |
| N04 | STDP Bridge | stdp_bridge.rs | 700+ | 50+ | VMS StdpKernel |
| N05 | Evolution Gate | evolution_gate.rs | 600+ | 50+ | VMS EvolutionChamber |
| N06 | Morphogenic Adapter | morphogenic_adapter.rs | 500+ | 50+ | VMS MorphogenicEngine |

### V3: Homeostasis (M43-M45) — TO BUILD

| ID | Module | File | Target LOC | Target Tests | Template |
|----|--------|------|-----------|-------------|----------|
| M43 | Thermal Controller | thermal.rs | 400+ | 10+ | ME v1 M40 |
| M44 | Decay Auditor | decay_auditor.rs | 400+ | 10+ | ME v1 M41 |
| M45 | Diagnostics Engine | diagnostics.rs | 400+ | 10+ | ME v1 M42 |

---

## Targets

| Metric | V1 Actual | V2 Target |
|--------|-----------|-----------|
| Layers | 7 | 8 (+L8 Nexus) |
| Modules | 45 | 48+ |
| LOC | 54,412 | 65,000+ |
| Unit Tests | 1,492 | 2,400+ |
| Doc Tests | 44 | 50+ |
| Integration Tests | 41 | 60+ |
| Clippy Warnings | 0 | 0 |
| Unsafe Code | 0 | 0 |
| Databases | 11 | 12 (+ nexus_state.db) |
| Benchmarks | 8 | 10+ |
| MCP Tools | 15 | 20+ |
| NAM Compliance | 0% → 92% target | 92%+ |

---

## Build Sequence (When "Start Coding" Given)

### Phase 1: Cargo.toml + lib.rs + main.rs
1. Write Cargo.toml (inherit ME v1 deps + add VMS integration)
2. Write lib.rs (8-layer module declarations, lint configuration)
3. Write main.rs (Axum server stub, CLI)

### Phase 2: Verify M1 + M2 Compile
4. Verify M1 foundation compiles
5. Verify M2 services compiles
6. Run quality gate (check → clippy → test)

### Phase 3: Build L3-L7 (Template from ME v1)
7. M3 Core Logic (6 modules)
8. M4 Integration (6 modules)
9. M5 Learning (6 modules)
10. M6 Consensus (6 modules)
11. M7 Observer (6 modules)

### Phase 4: Build L8 Nexus (NEW)
12. N01 Field Bridge
13. N02 Intent Router
14. N03 Regime Manager
15. N04 STDP Bridge
16. N05 Evolution Gate
17. N06 Morphogenic Adapter

### Phase 5: V3 Homeostasis + Tools
18. M43-M45 Homeostasis
19. Tools layer (7 tool files)

### Phase 6: Integration + Wiring
20. engine.rs (central orchestrator)
21. database.rs (database manager)
22. main.rs (full HTTP routes)

### Phase 7: Quality Gate
23. cargo check
24. cargo clippy -- -D warnings -W clippy::pedantic
25. cargo test --lib --release

---

## Cloned Assets Summary

| Asset | Count | Size |
|-------|-------|------|
| M1 Foundation source | 11 files | 533 KB |
| M2 Services source | 5 files | 236 KB |
| Databases | 12 files | 5.9 MB |
| Migrations | 11 files | 240 KB |
| Configs | 10 files | 12 KB |
| Benchmarks | 8 files | 40 KB |
| Tests | 17 files | 480 KB |
| NAM docs | 10 files | 212 KB |
| AI Specs | 14+ files | 396 KB |
| Module docs | 37 files | ~800 KB |
| Layer docs | 7 files | ~200 KB |
| Pattern specs | 12 files | ~150 KB |
| **Total** | **155+ files** | **~9.2 MB** |

---

## Nexus Integration Checklist (V2 Specific)

From VMS/SVF analysis:

- [ ] Access Pattern Tracking — tag all writes with regions
- [ ] Coherence Measurement — pre/post capture on health API calls
- [ ] K-Regime Management — detect Swarm/Fleet/Armada from system state
- [ ] STDP Learning — record service-to-service calls as potentiation
- [ ] Morphogenic Adaptation — recalibrate on |r_delta| > 0.05
- [ ] Evolution Chamber — test mutations in isolated Kuramoto network
- [ ] Homeostatic Regulation — target r ~ 0.7
- [ ] MCP Tool Dispatch — tiered tool architecture (T0-T9)
- [ ] Intent Tensor Routing — accept operational intents via 12D tensor
- [ ] Synergy Tracking — record collisions between service pairs

---

*Maintenance Engine V2 Scaffolding Plan | Generated 2026-03-06*
