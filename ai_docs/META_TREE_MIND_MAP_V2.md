# ME V2 — Meta Tree Mind Map (8 Layers + Nexus)

> **Scope:** 48+ modules across 8 layers | **LOC:** 62,522 | **Tests:** 2,288
> **V2 Enhancement:** L8 Nexus Integration (N01-N06) + Kuramoto + K-regime + morphogenic
> **Quality:** God-tier — zero unsafe, zero unwrap, zero clippy, zero pedantic
> **Date:** 2026-03-28 | **Session:** 068

---

## Root: ME V2 Architecture

```
ME V2 (62,522 LOC, 2,288 tests, 8 layers)
│
├── IDENTITY
│   ├── Name: maintenance_engine_v2
│   ├── Version: 2.0.0
│   ├── Port: 8080
│   ├── Status: COMPILED (L1-L7 deployed, L8 stubs)
│   ├── NAM target: 95%
│   ├── PBFT: n=40, f=13, q=27
│   ├── Nexus: K-regime adaptive (Swarm/Fleet/Armada)
│   └── Git: main @ 6804677
│
├── LAYER TREE
│   │
│   ├── L1 FOUNDATION (src/m1_foundation/) — 14,701 LOC, 625 tests
│   │   ├── M00 shared_types.rs ─── ai_docs/modules/M00_SHARED_TYPES.md
│   │   ├── M01 error.rs ────────── ai_docs/modules/M01_ERROR_TAXONOMY.md
│   │   ├── M02 config.rs ──────── ai_docs/modules/M02_CONFIGURATION_MANAGER.md
│   │   ├── M03 logging.rs ─────── ai_docs/modules/M03_LOGGING_SYSTEM.md
│   │   ├── M04 metrics.rs ─────── ai_docs/modules/M04_METRICS_COLLECTOR.md
│   │   ├── M05 state.rs ────────── ai_docs/modules/M05_STATE_PERSISTENCE.md
│   │   ├── M06 resources.rs ───── ai_docs/modules/M06_RESOURCE_MANAGER.md
│   │   ├── M07 signals.rs ─────── ai_docs/modules/M07_SIGNALS.md
│   │   ├── M08 tensor_registry ── ai_docs/modules/M08_TENSOR_REGISTRY.md
│   │   └── M43 nam.rs ──────────── ai_docs/modules/M43_NAM_UTILITIES.md
│   │   SPEC: ai_specs/m1-foundation-specs/ (14 files)
│   │   TRAITS: ErrorClassifier, ConfigProvider, MetricRecorder, StateStore,
│   │           ResourceCollector, SignalSubscriber, TensorContributor, CorrelationProvider
│   │
│   ├── L2 SERVICES (src/m2_services/) — 6,329 LOC, 279 tests
│   │   ├── M09 service_registry ── ai_docs/modules/M09_SERVICE_REGISTRY.md
│   │   ├── M10 health_monitor ──── ai_docs/modules/M10_HEALTH_MONITOR.md
│   │   ├── M11 lifecycle ────────── ai_docs/modules/M11_LIFECYCLE_MANAGER.md
│   │   └── M12 resilience ──────── ai_docs/modules/M12_RESILIENCE.md
│   │   SPEC: ai_specs/m2-services-specs/ (8 files)
│   │   TRAITS: ServiceDiscovery, HealthMonitoring, LifecycleOps,
│   │           CircuitBreakerOps, LoadBalancing, TensorContributor
│   │
│   ├── L3 CORE LOGIC (src/m3_core_logic/) — 6,981 LOC, 131 tests
│   │   ├── M13 pipeline.rs ────── ai_docs/modules/M13_PIPELINE_MANAGER.md
│   │   ├── M14 remediation.rs ── ai_docs/modules/M14_REMEDIATION_ENGINE.md
│   │   ├── M15 confidence.rs ──── ai_docs/modules/M15_CONFIDENCE_CALCULATOR.md
│   │   ├── M16 action.rs ──────── ai_docs/modules/M16_ACTION_EXECUTOR.md
│   │   ├── M17 outcome.rs ─────── ai_docs/modules/M17_OUTCOME_RECORDER.md
│   │   └── M18 feedback.rs ────── ai_docs/modules/M18_FEEDBACK_LOOP.md
│   │   SPEC: ai_specs/m3-core-logic-specs/
│   │
│   ├── L4 INTEGRATION (src/m4_integration/) — 7,403 LOC, 293 tests
│   │   ├── M19 rest.rs ────────── ai_docs/modules/M19_REST_CLIENT.md
│   │   ├── M20 grpc.rs ────────── ai_docs/modules/M20_GRPC_CLIENT.md
│   │   ├── M21 websocket.rs ──── ai_docs/modules/M21_WEBSOCKET_CLIENT.md
│   │   ├── M22 ipc.rs ──────────── ai_docs/modules/M22_IPC_MANAGER.md
│   │   ├── M23 event_bus.rs ────── ai_docs/modules/M23_EVENT_BUS.md
│   │   ├── M24 bridge.rs ──────── ai_docs/modules/M24_BRIDGE_MANAGER.md
│   │   ├── M42 cascade_bridge ── ai_docs/modules/M42_CASCADE_BRIDGE.md
│   │   ├── M46 peer_bridge ────── ai_docs/modules/M46_PEER_BRIDGE.md
│   │   └── M47 tool_registrar ── ai_docs/modules/M47_TOOL_REGISTRAR.md
│   │   SPEC: ai_specs/m4-integration-specs/
│   │
│   ├── L5 LEARNING (src/m5_learning/) — 6,349 LOC, 206 tests
│   │   ├── M25 hebbian.rs ─────── ai_docs/modules/M25_HEBBIAN_MANAGER.md
│   │   ├── M26 stdp.rs ────────── ai_docs/modules/M26_STDP_PROCESSOR.md
│   │   ├── M27 pattern.rs ─────── ai_docs/modules/M27_PATTERN_RECOGNIZER.md
│   │   ├── M28 pruner.rs ──────── ai_docs/modules/M28_PATHWAY_PRUNER.md
│   │   ├── M29 consolidator ──── ai_docs/modules/M29_MEMORY_CONSOLIDATOR.md
│   │   ├── M30 antipattern ────── ai_docs/modules/M30_ANTIPATTERN_DETECTOR.md
│   │   └── M41 decay_scheduler ── ai_docs/modules/M41_DECAY_SCHEDULER.md
│   │   SPEC: ai_specs/m5-learning-specs/
│   │
│   ├── L6 CONSENSUS (src/m6_consensus/) — 5,368 LOC, 200 tests
│   │   ├── M31 pbft.rs ────────── ai_docs/modules/M31_PBFT_MANAGER.md
│   │   ├── M32 agent.rs ────────── ai_docs/modules/M32_AGENT_COORDINATOR.md
│   │   ├── M33 voting.rs ──────── ai_docs/modules/M33_VOTE_COLLECTOR.md
│   │   ├── M34 view_change ────── ai_docs/modules/M34_VIEW_CHANGE_HANDLER.md
│   │   ├── M35 dissent.rs ─────── ai_docs/modules/M35_DISSENT_TRACKER.md
│   │   └── M36 quorum.rs ──────── ai_docs/modules/M36_QUORUM_CALCULATOR.md
│   │   SPEC: ai_specs/m6-consensus-specs/
│   │
│   ├── L7 OBSERVER (src/m7_observer/) — 7,920 LOC, 300 tests
│   │   ├── M37 log_correlator ── ai_docs/modules/M37_LOG_CORRELATOR.md
│   │   ├── M38 emergence_det ──── ai_docs/modules/M38_EMERGENCE_DETECTOR.md
│   │   ├── M39 evolution_ch ──── ai_docs/modules/M39_EVOLUTION_CHAMBER.md
│   │   ├── M40 thermal_mon ────── ai_docs/modules/M40_THERMAL_MONITOR.md
│   │   ├── M44 observer_bus ──── ai_docs/modules/M44_OBSERVER_BUS.md
│   │   └── M45 fitness ────────── ai_docs/modules/M45_FITNESS_EVALUATOR.md
│   │   SPEC: ai_specs/m7-observer-specs/
│   │   SPEC: ai_specs/evolution_chamber_ai_specs/ (10 files)
│   │
│   └── L8 NEXUS (src/nexus/) — STUBS, 0 LOC (target ~6,000)
│       ├── N01 field_bridge ────── ai_docs/modules/N01_FIELD_BRIDGE.md
│       ├── N02 intent_router ──── ai_docs/modules/N02_INTENT_ROUTER.md
│       ├── N03 regime_manager ── ai_docs/modules/N03_REGIME_MANAGER.md
│       ├── N04 stdp_bridge ────── ai_docs/modules/N04_STDP_BRIDGE.md
│       ├── N05 evolution_gate ── ai_docs/modules/N05_EVOLUTION_GATE.md
│       └── N06 morphogenic_ad ── ai_docs/modules/N06_MORPHOGENIC_ADAPTER.md
│       SPEC: ai_specs/nexus-specs/ (7 files)
│       TRAITS (planned): FieldBridge, IntentRouter, RegimeManager,
│                          StdpBridge, EvolutionGate, MorphogenicAdapter
│
├── TOP-LEVEL SOURCE
│   ├── lib.rs (333 LOC) — crate root, 8 layer declarations, Tensor12D, prelude
│   ├── main.rs (3,167 LOC) — Axum HTTP server, 30+ routes, 7 background tasks
│   ├── engine.rs (1,750 LOC) — MaintenanceEngine orchestrator, build_tensor()
│   └── database.rs (1,702 LOC) — DatabaseManager, 12 SQLite databases
│
├── DOCUMENTATION TREE
│   ├── ai_docs/
│   │   ├── INDEX.md — documentation hub
│   │   ├── QUICKSTART.md — build, run, navigate
│   │   ├── META_TREE_MIND_MAP_V2.md — THIS FILE
│   │   ├── META_TREE_MIND_MAP_M48_M57.md — V1 expansion plan (reference)
│   │   ├── modules/ — 50 module docs (M00-M47 + N01-N06)
│   │   ├── layers/ — 7 layer docs (L01-L07)
│   │   ├── schematics/ — Mermaid architecture diagrams
│   │   ├── diagnostics/ — runbook, API map, data flow, observability
│   │   └── security/ — security best practices
│   │
│   ├── ai_specs/
│   │   ├── INDEX.md — specs hub (27+ spec files)
│   │   ├── MODULE_MATRIX.md — module cross-reference
│   │   ├── LAYER_SPEC.md, SYSTEM_SPEC.md, TENSOR_SPEC.md, etc.
│   │   ├── m1-foundation-specs/ (14 files)
│   │   ├── m2-services-specs/ (8 files)
│   │   ├── m3-core-logic-specs/ through m7-observer-specs/
│   │   ├── nexus-specs/ (7 files — L8 specific)
│   │   ├── evolution_chamber_ai_specs/ (10 files — L7 specific)
│   │   └── patterns/ (10 pattern docs)
│   │
│   └── .claude/
│       ├── context.json — machine-readable module inventory
│       ├── status.json — ultra-compact heartbeat state
│       ├── patterns.json — 22 mandatory Rust patterns (P01-P22)
│       ├── ALIGNMENT_VERIFICATION.md — triple alignment procedures
│       └── skills/ — 8 ME-specific Claude Code skills
│
├── DATABASES (12, in data/databases/)
│   ├── service_tracking.db ──── services, health_checks, restarts
│   ├── system_synergy.db ────── connections, bridges, synergy_scores
│   ├── hebbian_pulse.db ─────── pathways, ltp_events, ltd_events
│   ├── consensus_tracking.db ── rounds, votes, dissent_log
│   ├── episodic_memory.db ──── episodes, contexts, outcomes
│   ├── tensor_memory.db ─────── tensors, snapshots, deltas
│   ├── performance_metrics.db ─ metrics, aggregations, alerts
│   ├── flow_state.db ────────── states, transitions, checkpoints
│   ├── security_events.db ──── events, threats, mitigations
│   ├── workflow_tracking.db ── workflows, steps, outcomes
│   ├── evolution_tracking.db ── fitness, mutations, emergence
│   └── remediation_log.db ──── actions, outcomes, confidence
│
├── 12D TENSOR COVERAGE
│   ├── D0  service_id ───── M09 (ServiceRegistry)
│   ├── D1  port ──────────── Engine (static)
│   ├── D2  tier ──────────── M09
│   ├── D3  dependency_count  M09 (placeholder 0.0 — needs M49 Traffic)
│   ├── D4  agent_count ───── Engine
│   ├── D5  protocol ──────── Engine (hardcoded — needs N02 IntentRouter)
│   ├── D6  health_score ──── M10, M11
│   ├── D7  uptime ────────── M11
│   ├── D8  synergy ────────── M24 BridgeManager, N01 FieldBridge (planned)
│   ├── D9  latency ────────── M12
│   ├── D10 error_rate ─────── M10, M12
│   └── D11 temporal_context ─ Engine (weak proxy — needs N01 FieldBridge)
│
├── QUALITY ENFORCEMENT
│   ├── #![forbid(unsafe_code)]
│   ├── #![deny(clippy::unwrap_used)]
│   ├── #![deny(clippy::expect_used)]
│   ├── #![warn(clippy::pedantic)]
│   ├── #![warn(missing_docs)]
│   ├── cargo check → clippy → pedantic → test (4-stage gate)
│   ├── 50+ tests per module minimum
│   ├── TensorContributor on every module (C3)
│   ├── parking_lot::RwLock for L1-L5,L7 / std::sync::RwLock for L6
│   └── Zero #[allow(...)] suppressions for root-cause issues
│
├── V2-SPECIFIC CONSTRAINTS
│   ├── C11: Every L4+ module has Nexus field capture (pre/post r)
│   ├── C12: All service interactions record STDP co-activation (+0.05/call)
│   ├── Kuramoto: K_SWARM=0.5, K_FLEET=1.5, K_ARMADA=3.0
│   ├── Morphogenic: |r_delta| > 0.05 triggers adaptation
│   └── Evolution gate: accept mutation only if r_after >= r_baseline
│
├── HABITAT WIRING (17 services)
│   │
│   ├── OUTBOUND HEALTH POLLING (12 services, 30s interval)
│   │   ├── ME:8080 ──GET──> DevOps:8081/health
│   │   ├── ME:8080 ──GET──> SYNTHEX:8090/api/health
│   │   ├── ME:8080 ──GET──> K7:8100/health (59 modules, 11 commands)
│   │   ├── ME:8080 ──GET──> NAIS:8101/health
│   │   ├── ME:8080 ──GET──> Bash:8102/health
│   │   ├── ME:8080 ──GET──> TM:8103/health
│   │   ├── ME:8080 ──GET──> CCM:8104/health
│   │   ├── ME:8080 ──GET──> TL:8105/health (15 tools registered)
│   │   ├── ME:8080 ──GET──> CSV7:8110/health
│   │   ├── ME:8080 ──GET──> POVM:8125/health
│   │   ├── ME:8080 ──GET──> RM:8130/health (64,400 entries)
│   │   └── ME:8080 ──GET──> PV2:8132/health (r=0.88, 83 spheres, K=1.5)
│   │
│   ├── OUTBOUND BRIDGES (active data flow)
│   │   ├── ME:8080 ──POST──> PV2:8132/bus/events (EventBus bridge, 10s, 6 channels)
│   │   ├── ME:8080 ──POST──> DevOps:8081/pipeline/trigger (startup once)
│   │   ├── ME:8080 ──GET───> SYNTHEX:8090/v3/thermal (60s, T=0.57, target=0.50)
│   │   ├── ME:8080 ──GET───> SYNTHEX:8090/v3/diagnostics (60s, cascade health=0.75)
│   │   ├── ME:8080 ──POST──> SYNTHEX:8090/v3/decay/trigger (scheduled decay)
│   │   └── ME:8080 ──POST──> TL:8105/api/tools (startup, 15 tools)
│   │
│   ├── INBOUND BRIDGES (services reading ME)
│   │   ├── ORAC:8133 ──m23_me_bridge──> ME:8080/api/health (fitness=0.61, 10s)
│   │   └── ORAC:8133 ──m23_me_bridge──> ME:8080/api/observer (subscribed=true)
│   │
│   ├── SYNTHEX WIRING (bidirectional thermal coupling)
│   │   ├── ME ──thermal_poll──> SYNTHEX:8090/v3/thermal (reads PID state)
│   │   ├── ME ──cascade_poll──> SYNTHEX:8090/v3/diagnostics (reads cascade)
│   │   ├── ME ──decay_trigger─> SYNTHEX:8090/v3/decay/trigger (writes)
│   │   ├── ORAC ──m22_synthex_bridge──> SYNTHEX:8090 (Hebbian writeback)
│   │   ├── SYNTHEX ──sync──> K7:8100 (synergy=92.0)
│   │   ├── SYNTHEX ──sync──> TL:8105 (synergy=90.0)
│   │   └── SYNTHEX ──sync──> NAIS:8101 (synergy=88.5)
│   │
│   ├── ORAC WIRING (6 hooks + bridge)
│   │   ├── ME ──orac-hook.sh──> ORAC:8133/hooks/SessionStart (5s)
│   │   ├── ME ──orac-hook.sh──> ORAC:8133/hooks/UserPromptSubmit (3s)
│   │   ├── ME ──orac-hook.sh──> ORAC:8133/hooks/PreToolUse (2s)
│   │   ├── ME ──orac-hook.sh──> ORAC:8133/hooks/PostToolUse (3s)
│   │   ├── ME ──orac-hook.sh──> ORAC:8133/hooks/Stop (5s)
│   │   └── ME ──orac-hook.sh──> ORAC:8133/hooks/PermissionRequest (2s)
│   │
│   ├── PV2 WIRING (field + bus)
│   │   ├── ME ──EventBus bridge──> PV2:8132/bus/events (10s, 6 channels)
│   │   ├── ME ──health poll──> PV2:8132/health (r, spheres, K, tick)
│   │   ├── PV2 bus subscribers: 1 (ME EventBus bridge)
│   │   └── PV2 bus events: 1000 (ring buffer)
│   │
│   ├── MEMORY SUBSTRATE WIRING
│   │   ├── POVM:8125 ──POST /memories──> crystallize session state
│   │   ├── POVM:8125 ──GET /hydrate──> restore on startup
│   │   ├── RM:8130 ──POST /put (TSV!)──> persist integration records
│   │   ├── RM:8130 ──GET /search?q=──> search cross-session memory
│   │   ├── PV2:8132 ──POST /sphere/*/register──> sphere lifecycle
│   │   └── SQLite (12 DBs) ──direct I/O──> all layers read/write
│   │
│   ├── PLANNED V2 WIRING (M48-M57 + L8)
│   │   ├── M53 OracBridge ──> ORAC:8133/health + /blackboard (bidirectional)
│   │   ├── M53 OracBridge ──> ORAC:8133/hooks/PostToolUse (push ME events)
│   │   ├── N01 FieldBridge ──> PV2:8132/health (Kuramoto r pre/post capture)
│   │   ├── N04 StdpBridge ──> VMS:8120/api/query (STDP from VMS patterns)
│   │   ├── M51 Auth ──> all outbound calls (token injection)
│   │   └── M52 RateLimit ──> all inbound calls (tier-based throttling)
│   │
│   └── SYNERGY SCORES (from system_synergy.db)
│       ├── ME → devenv: 95.0 (request_reply)
│       ├── ME → K7: 88.0 (sync)
│       ├── ME → NAIS: 86.0 (sync)
│       ├── ME → SYNTHEX: 85.0 (sync)
│       ├── ME → TL: 84.0 (sync)
│       ├── K7 → NAIS: 95.0 (sync)
│       ├── SYNTHEX → K7: 92.0 (sync)
│       ├── K7 → CSV7: 91.0 (sync)
│       └── SYNTHEX → TL: 90.0 (sync)
│
└── NEXT STEPS
    ├── Phase 7: Implement L8 Nexus (N01-N06) — ~6,000 LOC, 300 tests
    ├── Phase 8: engine.rs V2 orchestrator + main.rs V2 routes
    ├── Phase 9: Wire C11/C12 constraints across L4-L8
    └── Phase 10: Full integration test + NAM compliance audit (target 95%)
```

---

## Cross-References

| Resource | Path |
|----------|------|
| MASTER_INDEX | `MASTER_INDEX.md` |
| QUICKSTART | `ai_docs/QUICKSTART.md` |
| Scaffolding Plan | `SCAFFOLDING_MASTER_PLAN.md` |
| Module INDEX | `ai_docs/modules/INDEX.md` |
| V1 Mind Map | `ai_docs/META_TREE_MIND_MAP_M48_M57.md` |
| Nexus Specs | `ai_specs/nexus-specs/` |
| V1 Reference | `/home/louranicas/claude-code-workspace/the_maintenance_engine/` |
| ORAC Reference | `/home/louranicas/claude-code-workspace/orac-sidecar/` |

---

*Meta Tree Mind Map V2 | 8 Layers | 48+ Modules | 62,522 LOC | Session 068*
