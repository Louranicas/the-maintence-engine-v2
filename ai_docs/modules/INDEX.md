# Module Documentation Index

> **57 Modules across 7 Layers** (47 deployed + 10 planned) | Last Updated: 2026-03-28

---

## Overview

The Maintenance Engine comprises 57 modules organized into 7 layers (47 deployed, 10 planned):

| Layer | Name | Deployed | Planned | Source Directory |
|-------|------|----------|---------|------------------|
| L1 | Foundation | M00-M08, M43 (10) | M48 (1) | `src/m1_foundation/` |
| L2 | Services | M09-M12 (4) | M49 (1) | `src/m2_services/` |
| L3 | Core Logic | M13-M18 (6) | M50 (1) | `src/m3_core_logic/` |
| L4 | Integration | M19-M24, M42, M46-M47 (9) | M51-M53 (3) | `src/m4_integration/` |
| L5 | Learning | M25-M30, M41, M49 (8) | M54-M55 (2) | `src/m5_learning/` |
| L6 | Consensus | M31-M36 (6) | M56-M57 (2) | `src/m6_consensus/` |
| L7 | Observer | M37-M40, M44-M45 (6) | — | `src/m7_observer/` |

**Note:** M40 (Thermal Monitor) lives in `m7_observer/`, M41 (Decay Scheduler) lives in `m5_learning/`, M42 (Cascade Bridge) lives in `m4_integration/`. These are HRS-001 V3 modules placed by cross-cutting concern, not by layer ownership.

---

## Layer 1: Foundation (M00-M08)

| Module | Name | File | Status |
|--------|------|------|--------|
| M00 | Shared Types | [M00_SHARED_TYPES.md](M00_SHARED_TYPES.md) | Complete |
| M01 | Error Taxonomy | [M01_ERROR_TAXONOMY.md](M01_ERROR_TAXONOMY.md) | Complete |
| M02 | Configuration Manager | [M02_CONFIGURATION_MANAGER.md](M02_CONFIGURATION_MANAGER.md) | Complete |
| M03 | Logging System | [M03_LOGGING_SYSTEM.md](M03_LOGGING_SYSTEM.md) | Complete |
| M04 | Metrics Collector | [M04_METRICS_COLLECTOR.md](M04_METRICS_COLLECTOR.md) | Complete |
| M05 | State Persistence | [M05_STATE_PERSISTENCE.md](M05_STATE_PERSISTENCE.md) | Complete |
| M06 | Resource Manager | [M06_RESOURCE_MANAGER.md](M06_RESOURCE_MANAGER.md) | Complete |
| M07 | Signal Bus | [M07_SIGNALS.md](M07_SIGNALS.md) | Complete |
| M08 | Tensor Registry | [M08_TENSOR_REGISTRY.md](M08_TENSOR_REGISTRY.md) | Complete |

---

## Layer 2: Services (M09-M12) — Refactored 2026-02-28

| Module | Name | File | Status |
|--------|------|------|--------|
| M09 | Service Registry | [M09_SERVICE_REGISTRY.md](M09_SERVICE_REGISTRY.md) | Complete |
| M10 | Health Monitor | [M10_HEALTH_MONITOR.md](M10_HEALTH_MONITOR.md) | Complete |
| M11 | Lifecycle Manager | [M11_LIFECYCLE_MANAGER.md](M11_LIFECYCLE_MANAGER.md) | Complete |
| M12 | Resilience Manager | [M12_RESILIENCE.md](M12_RESILIENCE.md) | Complete |

---

## Layer 3: Core Logic (M13-M18)

| Module | Name | File | Status |
|--------|------|------|--------|
| M13 | Pipeline Manager | [M13_PIPELINE_MANAGER.md](M13_PIPELINE_MANAGER.md) | Complete |
| M14 | Remediation Engine | [M14_REMEDIATION_ENGINE.md](M14_REMEDIATION_ENGINE.md) | Complete |
| M15 | Confidence Calculator | [M15_CONFIDENCE_CALCULATOR.md](M15_CONFIDENCE_CALCULATOR.md) | Complete |
| M16 | Action Executor | [M16_ACTION_EXECUTOR.md](M16_ACTION_EXECUTOR.md) | Complete |
| M17 | Outcome Recorder | [M17_OUTCOME_RECORDER.md](M17_OUTCOME_RECORDER.md) | Complete |
| M18 | Feedback Loop | [M18_FEEDBACK_LOOP.md](M18_FEEDBACK_LOOP.md) | Complete |

---

## Layer 4: Integration (M19-M24)

| Module | Name | File | Status |
|--------|------|------|--------|
| M19 | REST Client | [M19_REST_CLIENT.md](M19_REST_CLIENT.md) | Complete |
| M20 | gRPC Client | [M20_GRPC_CLIENT.md](M20_GRPC_CLIENT.md) | Complete |
| M21 | WebSocket Client | [M21_WEBSOCKET_CLIENT.md](M21_WEBSOCKET_CLIENT.md) | Complete |
| M22 | IPC Manager | [M22_IPC_MANAGER.md](M22_IPC_MANAGER.md) | Complete |
| M23 | Event Bus | [M23_EVENT_BUS.md](M23_EVENT_BUS.md) | Complete |
| M24 | Bridge Manager | [M24_BRIDGE_MANAGER.md](M24_BRIDGE_MANAGER.md) | Complete |

---

## Layer 5: Learning (M25-M30)

| Module | Name | File | Status |
|--------|------|------|--------|
| M25 | Hebbian Manager | [M25_HEBBIAN_MANAGER.md](M25_HEBBIAN_MANAGER.md) | Complete |
| M26 | STDP Processor | [M26_STDP_PROCESSOR.md](M26_STDP_PROCESSOR.md) | Complete |
| M27 | Pattern Recognizer | [M27_PATTERN_RECOGNIZER.md](M27_PATTERN_RECOGNIZER.md) | Complete |
| M28 | Pathway Pruner | [M28_PATHWAY_PRUNER.md](M28_PATHWAY_PRUNER.md) | Complete |
| M29 | Memory Consolidator | [M29_MEMORY_CONSOLIDATOR.md](M29_MEMORY_CONSOLIDATOR.md) | Complete |
| M30 | Anti-Pattern Detector | [M30_ANTIPATTERN_DETECTOR.md](M30_ANTIPATTERN_DETECTOR.md) | Complete |

---

## Layer 6: Consensus (M31-M36)

| Module | Name | File | Status |
|--------|------|------|--------|
| M31 | PBFT Manager | [M31_PBFT_MANAGER.md](M31_PBFT_MANAGER.md) | Complete |
| M32 | Agent Coordinator | [M32_AGENT_COORDINATOR.md](M32_AGENT_COORDINATOR.md) | Complete |
| M33 | Vote Collector | [M33_VOTE_COLLECTOR.md](M33_VOTE_COLLECTOR.md) | Complete |
| M34 | View Change Handler | [M34_VIEW_CHANGE_HANDLER.md](M34_VIEW_CHANGE_HANDLER.md) | Complete |
| M35 | Dissent Tracker | [M35_DISSENT_TRACKER.md](M35_DISSENT_TRACKER.md) | Complete |
| M36 | Quorum Calculator | [M36_QUORUM_CALCULATOR.md](M36_QUORUM_CALCULATOR.md) | Complete |

---

## Layer 7: Observer (M37-M40, M44-M45)

| Module | Name | File | Status |
|--------|------|------|--------|
| M37 | Log Correlator | [M37_LOG_CORRELATOR.md](M37_LOG_CORRELATOR.md) | Complete |
| M38 | Emergence Detector | [M38_EMERGENCE_DETECTOR.md](M38_EMERGENCE_DETECTOR.md) | Complete |
| M39 | Evolution Chamber | [M39_EVOLUTION_CHAMBER.md](M39_EVOLUTION_CHAMBER.md) | Complete |
| M40 | Thermal Monitor | [M40_THERMAL_MONITOR.md](M40_THERMAL_MONITOR.md) | Complete (HRS-001) |
| M44 | Observer Bus | [M44_OBSERVER_BUS.md](M44_OBSERVER_BUS.md) | Complete (infra, newly ID'd) |
| M45 | Fitness Evaluator | [M45_FITNESS_EVALUATOR.md](M45_FITNESS_EVALUATOR.md) | Complete (infra, newly ID'd) |

---

## Layer 8: Nexus Integration (N01-N06) — NEW in V2

| Module | Name | File | Status |
|--------|------|------|--------|
| N01 | Field Bridge | [N01_FIELD_BRIDGE.md](N01_FIELD_BRIDGE.md) | STUB |
| N02 | Intent Router | [N02_INTENT_ROUTER.md](N02_INTENT_ROUTER.md) | STUB |
| N03 | Regime Manager | [N03_REGIME_MANAGER.md](N03_REGIME_MANAGER.md) | STUB |
| N04 | STDP Bridge | [N04_STDP_BRIDGE.md](N04_STDP_BRIDGE.md) | STUB |
| N05 | Evolution Gate | [N05_EVOLUTION_GATE.md](N05_EVOLUTION_GATE.md) | STUB |
| N06 | Morphogenic Adapter | [N06_MORPHOGENIC_ADAPTER.md](N06_MORPHOGENIC_ADAPTER.md) | STUB |

---

## Cross-Cutting Infrastructure (M41-M43, M46-M47)

| Module | Name | Layer | File | Status |
|--------|------|-------|------|--------|
| M41 | Decay Scheduler | L5 | [M41_DECAY_SCHEDULER.md](M41_DECAY_SCHEDULER.md) | Complete (HRS-001) |
| M42 | Cascade Bridge | L4 | [M42_CASCADE_BRIDGE.md](M42_CASCADE_BRIDGE.md) | Complete (HRS-001) |
| M43 | NAM Utilities | L1 | [M43_NAM_UTILITIES.md](M43_NAM_UTILITIES.md) | Complete (infra, newly ID'd) |
| M46 | Peer Bridge | L4 | [M46_PEER_BRIDGE.md](M46_PEER_BRIDGE.md) | Complete (infra, newly ID'd) |
| M47 | Tool Registrar | L4 | [M47_TOOL_REGISTRAR.md](M47_TOOL_REGISTRAR.md) | Complete (infra, newly ID'd) |

---

## Planned New Modules (M48-M57)

| Module | Name | Target Layer | File | Status |
|--------|------|-------------|------|--------|
| M48 | Self Model | L1 | M48_SELF_MODEL.md | PLANNED |
| M49 | Traffic Manager | L2 | M49_TRAFFIC.md | PLANNED |
| M50 | Approval Workflow | L3 | M50_APPROVAL.md | PLANNED |
| M51 | Auth Handler | L4 | M51_AUTH.md | PLANNED |
| M52 | Rate Limiter | L4 | M52_RATE_LIMIT.md | PLANNED |
| M53 | ORAC Bridge | L4 | M53_ORAC_BRIDGE.md | PLANNED |
| M54 | Prediction Engine | L5 | M54_PREDICT.md | PLANNED |
| M55 | Sequence Detector | L5 | M55_SEQUENCE.md | PLANNED |
| M56 | Checkpoint Manager | L6 | M56_CHECKPOINT.md | PLANNED |
| M57 | Active Dissent | L6 | M57_ACTIVE_DISSENT.md | PLANNED |

---

## Quick Reference

### Module ID Format
- `Mxx` where xx is 00-57
- L1: M00-M08, M43, M48 (11 modules: 10 deployed + 1 planned)
- L2: M09-M12, M49 (5 modules: 4 deployed + 1 planned)
- L3: M13-M18, M50 (7 modules: 6 deployed + 1 planned)
- L4: M19-M24, M42, M46-M47, M51-M53 (12 modules: 9 deployed + 3 planned)
- L5: M25-M30, M41, M54-M55 (10 modules: 7 deployed + 2 planned w/ M49=decay_scheduler already deployed)
- L6: M31-M36, M56-M57 (8 modules: 6 deployed + 2 planned)
- L7: M37-M40, M44-M45 (6 modules: all deployed)

### Source Code Mapping
```
src/
├── m1_foundation/     # M00-M08, M43 (10 deployed) + M48 planned
├── m2_services/       # M09-M12 (4 deployed) + M49 planned
├── m3_core_logic/     # M13-M18 (6 deployed) + M50 planned
├── m4_integration/    # M19-M24, M42, M46-M47 (9 deployed) + M51-M53 planned
├── m5_learning/       # M25-M30, M41 (7 deployed) + M54-M55 planned
├── m6_consensus/      # M31-M36 (6 deployed) + M56-M57 planned
├── m7_observer/       # M37-M40, M44-M45 (6 deployed)
└── nexus/             # N01-N06 (6 stubs — NEW in V2)
```

---

## Navigation

| Link | Description |
|------|-------------|
| [INDEX.md](../INDEX.md) | AI Docs Index |
| [Layer Documentation](../layers/) | Layer specifications |
| [QUICKSTART.md](../QUICKSTART.md) | Getting started guide |

---

*The Maintenance Engine v1.0.0 | Module Documentation Index*
