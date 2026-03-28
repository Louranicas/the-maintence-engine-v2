# Maintenance Engine V2 — Master Index

```json
{"v":"2.0.0","status":"COMPILED","modules":48,"layers":8,"loc":62522,"tests":2288,"clippy":0,"databases":12,"pipelines":8,"services":13,"port":8080,"nam_target":0.95,"pbft":{"n":40,"f":13,"q":27},"nexus":true,"source_files":66}
```

> **8 Layers | 48+ Modules | 62,522 LOC | 2,288 Tests | 0 Clippy Warnings**
> **Date:** 2026-03-28 | **Branch:** main | **Commit:** 6804677

---

## Navigation

| Resource | Path | Description |
|----------|------|-------------|
| **This File** | `MASTER_INDEX.md` | Complete project inventory |
| **README** | [`README.md`](README.md) | Project overview + quick start |
| **Scaffolding Plan** | [`SCAFFOLDING_MASTER_PLAN.md`](SCAFFOLDING_MASTER_PLAN.md) | V2 architecture blueprint |
| **Bootstrap Context** | [`CLAUDE.md`](CLAUDE.md) | AI context for Claude Code sessions |
| **Local Dev Context** | [`CLAUDE.local.md`](CLAUDE.local.md) | Session state and phase tracking |
| **Quick Start** | [`ai_docs/QUICKSTART.md`](ai_docs/QUICKSTART.md) | Build, run, navigate |
| **Meta Tree Mind Map** | [`ai_docs/META_TREE_MIND_MAP_V2.md`](ai_docs/META_TREE_MIND_MAP_V2.md) | Full architecture tree + wiring |
| **Evolution Chamber V2** | [`ai_docs/ADVANCED_EVOLUTION_CHAMBER_V2.md`](ai_docs/ADVANCED_EVOLUTION_CHAMBER_V2.md) | RALPH redesign from live data |
| **Habitat Integration** | [`ai_specs/HABITAT_INTEGRATION_SPEC.md`](ai_specs/HABITAT_INTEGRATION_SPEC.md) | Full wiring map + schematics |
| **AI Docs Index** | [`ai_docs/INDEX.md`](ai_docs/INDEX.md) | Documentation hub |
| **AI Specs Index** | [`ai_specs/INDEX.md`](ai_specs/INDEX.md) | Specifications hub |
| **Module Index** | [`ai_docs/modules/INDEX.md`](ai_docs/modules/INDEX.md) | 55 module docs (48 M + 6 N + INDEX) |
| **Context (machine)** | [`.claude/context.json`](.claude/context.json) | Machine-readable module inventory |
| **Status (compact)** | [`.claude/status.json`](.claude/status.json) | Ultra-compact heartbeat state |
| **Patterns** | [`.claude/patterns.json`](.claude/patterns.json) | 22 mandatory Rust patterns (P01-P22) |
| **Alignment Check** | [`.claude/ALIGNMENT_VERIFICATION.md`](.claude/ALIGNMENT_VERIFICATION.md) | Triple alignment verification procedures |

---

## Architecture Overview

```
L8: NEXUS (NEW)       src/nexus/            N01-N06 (6 stubs)         0 LOC
L7: OBSERVER           src/m7_observer/      M37-M40, M44-M45 (6)    7,920 LOC   300 tests
L6: CONSENSUS           src/m6_consensus/      M31-M36 (6)             5,368 LOC   200 tests
L5: LEARNING            src/m5_learning/       M25-M30, M41 (7)        6,349 LOC   206 tests
L4: INTEGRATION         src/m4_integration/    M19-M24, M42, M46-M47 (9) 7,403 LOC 293 tests
L3: CORE LOGIC          src/m3_core_logic/     M13-M18 (6)             6,981 LOC   131 tests
L2: SERVICES            src/m2_services/       M09-M12 (4)             6,329 LOC   279 tests
L1: FOUNDATION          src/m1_foundation/     M00-M08, M43 (10)       14,701 LOC  625 tests
V3: HOMEOSTASIS         (in L5/L7)            M40-M42 (3)             (included)
TOP: Infrastructure     src/{lib,main,engine,database}.rs              6,952 LOC   67 tests
TOOLS                   src/tools/             7 files                 1,044 LOC   76 tests
```

**Total:** 66 source files, 62,522 LOC, 2,288 tests, 0 clippy warnings (pedantic)

---

## Module Registry (48 Deployed + 6 Nexus Stubs)

### L1 Foundation (10 modules)

| ID | Name | File | Status |
|----|------|------|--------|
| M00 | Shared Types | `m1_foundation/shared_types.rs` | DEPLOYED |
| M01 | Error Taxonomy | `m1_foundation/error.rs` | DEPLOYED |
| M02 | Configuration | `m1_foundation/config.rs` | DEPLOYED |
| M03 | Logging | `m1_foundation/logging.rs` | DEPLOYED |
| M04 | Metrics | `m1_foundation/metrics.rs` | DEPLOYED |
| M05 | State Persistence | `m1_foundation/state.rs` | DEPLOYED |
| M06 | Resource Manager | `m1_foundation/resources.rs` | DEPLOYED |
| M07 | Signal Bus | `m1_foundation/signals.rs` | DEPLOYED |
| M08 | Tensor Registry | `m1_foundation/tensor_registry.rs` | DEPLOYED |
| M43 | NAM Utilities | `m1_foundation/nam.rs` | DEPLOYED |

### L2 Services (4 modules)

| ID | Name | File | Status |
|----|------|------|--------|
| M09 | Service Registry | `m2_services/service_registry.rs` | DEPLOYED |
| M10 | Health Monitor | `m2_services/health_monitor.rs` | DEPLOYED |
| M11 | Lifecycle Manager | `m2_services/lifecycle.rs` | DEPLOYED |
| M12 | Resilience | `m2_services/resilience.rs` | DEPLOYED |

### L3 Core Logic (6 modules)

| ID | Name | File | Status |
|----|------|------|--------|
| M13 | Pipeline Manager | `m3_core_logic/pipeline.rs` | DEPLOYED |
| M14 | Remediation Engine | `m3_core_logic/remediation.rs` | DEPLOYED |
| M15 | Confidence Calculator | `m3_core_logic/confidence.rs` | DEPLOYED |
| M16 | Action Executor | `m3_core_logic/action.rs` | DEPLOYED |
| M17 | Outcome Recorder | `m3_core_logic/outcome.rs` | DEPLOYED |
| M18 | Feedback Loop | `m3_core_logic/feedback.rs` | DEPLOYED |

### L4 Integration (9 modules)

| ID | Name | File | Status |
|----|------|------|--------|
| M19 | REST Client | `m4_integration/rest.rs` | DEPLOYED |
| M20 | gRPC Client | `m4_integration/grpc.rs` | DEPLOYED |
| M21 | WebSocket Client | `m4_integration/websocket.rs` | DEPLOYED |
| M22 | IPC Manager | `m4_integration/ipc.rs` | DEPLOYED |
| M23 | Event Bus | `m4_integration/event_bus.rs` | DEPLOYED |
| M24 | Bridge Manager | `m4_integration/bridge.rs` | DEPLOYED |
| M42 | Cascade Bridge | `m4_integration/cascade_bridge.rs` | DEPLOYED |
| M46 | Peer Bridge | `m4_integration/peer_bridge.rs` | DEPLOYED |
| M47 | Tool Registrar | `m4_integration/tool_registrar.rs` | DEPLOYED |

### L5 Learning (7 modules)

| ID | Name | File | Status |
|----|------|------|--------|
| M25 | Hebbian Manager | `m5_learning/hebbian.rs` | DEPLOYED |
| M26 | STDP Processor | `m5_learning/stdp.rs` | DEPLOYED |
| M27 | Pattern Recognizer | `m5_learning/pattern.rs` | DEPLOYED |
| M28 | Pathway Pruner | `m5_learning/pruner.rs` | DEPLOYED |
| M29 | Memory Consolidator | `m5_learning/consolidator.rs` | DEPLOYED |
| M30 | Anti-Pattern Detector | `m5_learning/antipattern.rs` | DEPLOYED |
| M41 | Decay Scheduler | `m5_learning/decay_scheduler.rs` | DEPLOYED |

### L6 Consensus (6 modules)

| ID | Name | File | Status |
|----|------|------|--------|
| M31 | PBFT Manager | `m6_consensus/pbft.rs` | DEPLOYED |
| M32 | Agent Coordinator | `m6_consensus/agent.rs` | DEPLOYED |
| M33 | Vote Collector | `m6_consensus/voting.rs` | DEPLOYED |
| M34 | View Change Handler | `m6_consensus/view_change.rs` | DEPLOYED |
| M35 | Dissent Tracker | `m6_consensus/dissent.rs` | DEPLOYED |
| M36 | Quorum Calculator | `m6_consensus/quorum.rs` | DEPLOYED |

### L7 Observer (6 modules)

| ID | Name | File | Status |
|----|------|------|--------|
| M37 | Log Correlator | `m7_observer/log_correlator.rs` | DEPLOYED |
| M38 | Emergence Detector | `m7_observer/emergence_detector.rs` | DEPLOYED |
| M39 | Evolution Chamber | `m7_observer/evolution_chamber.rs` | DEPLOYED |
| M40 | Thermal Monitor | `m7_observer/thermal_monitor.rs` | DEPLOYED |
| M44 | Observer Bus | `m7_observer/observer_bus.rs` | DEPLOYED |
| M45 | Fitness Evaluator | `m7_observer/fitness.rs` | DEPLOYED |

### L8 Nexus (6 module stubs — NEW in V2)

| ID | Name | File | Status |
|----|------|------|--------|
| N01 | Field Bridge | `nexus/field_bridge.rs` | STUB |
| N02 | Intent Router | `nexus/intent_router.rs` | STUB |
| N03 | Regime Manager | `nexus/regime_manager.rs` | STUB |
| N04 | STDP Bridge | `nexus/stdp_bridge.rs` | STUB |
| N05 | Evolution Gate | `nexus/evolution_gate.rs` | STUB |
| N06 | Morphogenic Adapter | `nexus/morphogenic_adapter.rs` | STUB |

---

## Database Inventory (12 databases)

| # | Database | Purpose | Status |
|---|----------|---------|--------|
| 1 | service_tracking.db | Service lifecycle, health history | DATA |
| 2 | system_synergy.db | Cross-system integration scoring | DATA |
| 3 | hebbian_pulse.db | Neural pathway learning | DATA |
| 4 | consensus_tracking.db | PBFT consensus rounds | DATA |
| 5 | episodic_memory.db | Episode recording | DATA |
| 6 | tensor_memory.db | 12D tensor storage | DATA |
| 7 | performance_metrics.db | Performance tracking | DATA |
| 8 | flow_state.db | Flow state transitions | DATA |
| 9 | security_events.db | Security monitoring | SCHEMA |
| 10 | workflow_tracking.db | Workflow orchestration | DATA |
| 11 | evolution_tracking.db | RALPH evolution tracking | DATA |
| 12 | remediation_log.db | Remediation actions | EMPTY |

---

## Quality Gate

```bash
CARGO_TARGET_DIR=/tmp/cargo-maintenance-v2 cargo check 2>&1 | tail -5
CARGO_TARGET_DIR=/tmp/cargo-maintenance-v2 cargo clippy -- -D warnings 2>&1 | tail -5
CARGO_TARGET_DIR=/tmp/cargo-maintenance-v2 cargo clippy -- -D warnings -W clippy::pedantic 2>&1 | tail -5
CARGO_TARGET_DIR=/tmp/cargo-maintenance-v2 cargo test --lib --release 2>&1 | tail -5
```

**Last gate:** 2026-03-28 — 4/4 PASS (0 errors, 0 warnings, 2,288 tests)

---

## Key Constants

| Constant | Value |
|----------|-------|
| Port | 8080 |
| PBFT n/f/q | 40/13/27 |
| STDP LTP/LTD | 0.1/0.05 |
| Decay rate | 0.1 (HRS-001 corrected) |
| NAM target | 95% |
| Tensor dims | 12 |
| K Swarm/Fleet/Armada | 0.5/1.5/3.0 |
| r adaptation threshold | 0.05 |

---

## Cross-References

| Source | Location |
|--------|----------|
| ME V1 (running binary) | `/home/louranicas/claude-code-workspace/the_maintenance_engine/` |
| ORAC Sidecar | `/home/louranicas/claude-code-workspace/orac-sidecar/` |
| PV2 | `/home/louranicas/claude-code-workspace/pane-vortex-v2/` |
| VMS | `/home/louranicas/claude-code-workspace/vortex-memory-system/` |
| Obsidian Vault | `/home/louranicas/projects/claude_code/` |
| Shared Context | `~/projects/shared-context/` |

---

*Maintenance Engine V2 | COMPILED | 8 Layers | 62,522 LOC | 2,288 Tests*
*Generated: 2026-03-28*
