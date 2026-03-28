# ME V2 — Habitat Integration Specification

> **Purpose:** Complete wiring map for assimilating ME V2 into the ULTRAPLATE Habitat
> **Services:** 17 active | **Bridges:** 5 outbound + 1 inbound | **Endpoints:** 30+ HTTP routes
> **Generated from:** Live system probes + source code analysis | **Date:** 2026-03-28

---

## 1. Service Identity

```
SERVICE:     maintenance-engine (V2)
PORT:        8080
HEALTH:      /api/health
PROTOCOL:    REST (primary) + gRPC (8081) + WebSocket (8082)
DEVENV ID:   maintenance-engine
DEVENV BATCH: 2 (depends on devops-engine)
BINARY:      ./bin/maintenance_engine_v2
```

---

## 2. Connection Topology

### 2.1 Outbound Connections (ME → Services)

```
                              ┌─ DevOps:8081 ─── /health (30s poll) + POST /pipeline/trigger (startup)
                              ├─ SYNTHEX:8090 ── /api/health (30s poll) + /v3/thermal + /v3/diagnostics
                              ├─ K7:8100 ─────── /health (30s poll)
                              ├─ NAIS:8101 ────── /health (30s poll)
                              ├─ Bash:8102 ────── /health (30s poll)
                              ├─ TM:8103 ──────── /health (30s poll)
                              ├─ CCM:8104 ─────── /health (30s poll)
   ME:8080 ──health poll(30s)─┤  TL:8105 ──────── /health (30s poll)
                              ├─ CSV7:8110 ────── /health (30s poll)
                              ├─ POVM:8125 ────── /health (30s poll)
                              ├─ RM:8130 ──────── /health (30s poll)
                              └─ PV2:8132 ─────── /health (30s poll)

   ME:8080 ──EventBus bridge──> PV2:8132/bus/events (10s poll, 6 channels)
   ME:8080 ──pipeline trigger──> DevOps:8081/pipeline/trigger (startup once)
   ME:8080 ──thermal poll──────> SYNTHEX:8090/v3/thermal (60s poll)
   ME:8080 ──cascade poll──────> SYNTHEX:8090/v3/diagnostics (60s poll)
   ME:8080 ──decay trigger─────> SYNTHEX:8090/v3/decay/trigger (on schedule)
   ME:8080 ──tool register─────> TL:8105/api/tools (startup registration, 15 tools)
```

### 2.2 Inbound Connections (Services → ME)

```
   ORAC:8133 ──m23_me_bridge──> ME:8080/api/health (fitness polling, 10s)
   ORAC:8133 ──m23_me_bridge──> ME:8080/api/observer (observer state, 10s)
   Any client ─────────────────> ME:8080/api/* (30+ REST endpoints)
```

### 2.3 Connection Matrix

| From | To | Path | Protocol | Interval | Purpose |
|------|----|------|----------|----------|---------|
| ME | DevOps:8081 | /health | HTTP GET | 30s | Health poll |
| ME | DevOps:8081 | /pipeline/trigger | HTTP POST | Startup | Trigger health-check pipeline |
| ME | SYNTHEX:8090 | /api/health | HTTP GET | 30s | Health poll |
| ME | SYNTHEX:8090 | /v3/thermal | HTTP GET | 60s | Thermal state read |
| ME | SYNTHEX:8090 | /v3/diagnostics | HTTP GET | 60s | Cascade state read |
| ME | SYNTHEX:8090 | /v3/decay/trigger | HTTP POST | On schedule | Trigger STDP decay cycle |
| ME | K7:8100 | /health | HTTP GET | 30s | Health poll |
| ME | NAIS:8101 | /health | HTTP GET | 30s | Health poll |
| ME | Bash:8102 | /health | HTTP GET | 30s | Health poll |
| ME | TM:8103 | /health | HTTP GET | 30s | Health poll |
| ME | CCM:8104 | /health | HTTP GET | 30s | Health poll |
| ME | TL:8105 | /health | HTTP GET | 30s | Health poll |
| ME | TL:8105 | /api/tools | HTTP POST | Startup | Register 15 tools |
| ME | CSV7:8110 | /health | HTTP GET | 30s | Health poll |
| ME | POVM:8125 | /health | HTTP GET | 30s | Health poll |
| ME | RM:8130 | /health | HTTP GET | 30s | Health poll |
| ME | PV2:8132 | /health | HTTP GET | 30s | Health poll |
| ME | PV2:8132 | /bus/events | HTTP POST | 10s | EventBus bridge (6 channels) |
| ORAC | ME:8080 | /api/health | HTTP GET | 10s | Fitness read (m23_me_bridge) |
| ORAC | ME:8080 | /api/observer | HTTP GET | 10s | Observer state (m23_me_bridge) |

### 2.4 Planned V2 Connections (M48-M57 + L8 Nexus)

| Module | From | To | Path | Purpose |
|--------|------|----|------|---------|
| M51 Auth | ME | All services | (header injection) | Token-based auth on outbound calls |
| M52 RateLimit | ME | (internal) | — | Tier-based request throttling |
| M53 OracBridge | ME | ORAC:8133 | /health, /blackboard | Bidirectional ORAC integration |
| M53 OracBridge | ME | ORAC:8133 | /hooks/PostToolUse | Push ME events to ORAC |
| N01 FieldBridge | ME | PV2:8132 | /health | Kuramoto r-tracking (pre/post capture) |
| N04 StdpBridge | ME | VMS:8120 | /api/query | STDP learning from VMS patterns |

---

## 3. Background Tasks (12 spawned in main.rs)

| Task | Function | Interval | Target | Source Line |
|------|----------|----------|--------|-------------|
| Observer tick | `spawn_observer_tick` | 60s | Internal M37-M39 pipeline | main.rs:541 |
| Tool registration | `spawn_tool_registration` | Startup | TL:8105 | main.rs:1126 |
| Peer polling | `spawn_peer_polling` | 30s | 12 services | main.rs:1141 |
| Learning cycle | `spawn_learning_cycle` | 120s | Internal L5 STDP | main.rs:1179 |
| Thermal polling | `spawn_thermal_polling` | 60s | SYNTHEX:8090/v3/thermal | main.rs:1232 |
| Cascade polling | `spawn_cascade_polling` | 60s | SYNTHEX:8090/v3/diagnostics | main.rs:1262 |
| Decay scheduler | `spawn_decay_scheduler` | 300s | SYNTHEX:8090/v3/decay/trigger | main.rs:1290 |
| Health polling | `spawn_health_polling` | 30s | 12 services | main.rs:397 |
| DevOps pipeline | `spawn_devops_pipeline_trigger` | Startup (30s delay) | DevOps:8081 | main.rs:260 |
| PV2 EventBus bridge | `spawn_pv2_eventbus_bridge` | 10s | PV2:8132/bus/events | main.rs:303 |
| Heartbeat | `spawn_heartbeat` | 60s | Log only | main.rs:529 |

---

## 4. HTTP API Endpoints (30+ routes)

### Core API

| Method | Path | Purpose | Response Type |
|--------|------|---------|---------------|
| GET | `/api/health` | Health + fitness + uptime | `{status, last_fitness, overall_health, uptime_secs}` |
| GET | `/api/status` | Full engine status | `{architecture, health, nam, pbft, port, version}` |
| GET | `/api/engine` | Engine health report | `{overall_health, layers, services}` |
| GET | `/api/services` | Service mesh overview | `{services: [{id, health_score}]}` |
| GET | `/api/layers` | Per-layer health | `{layers: [{layer, name, health, modules}]}` |

### Observer API

| Method | Path | Purpose | Response Type |
|--------|------|---------|---------------|
| GET | `/api/observer` | RALPH state + metrics | `{system_state, fitness_trend, generation, metrics}` |
| GET | `/api/fitness` | 12D fitness breakdown | `{current_fitness, dimension_contributions}` |
| GET | `/api/emergence` | Emergence events | `{events, types, count}` |
| GET | `/api/evolution` | Evolution chamber state | `{generation, mutations, phase}` |

### Learning & Consensus API

| Method | Path | Purpose |
|--------|------|---------|
| GET | `/api/learning` | STDP + pathway state |
| GET | `/api/consensus` | PBFT fleet + roles |
| GET | `/api/remediation` | Remediation queue |
| GET | `/api/integration` | Bridge status |
| GET | `/api/peers` | Peer bridge stats |

### V3 Neural Homeostasis API

| Method | Path | Purpose |
|--------|------|---------|
| GET | `/api/v3/thermal` | PID thermal state (reads SYNTHEX) |
| GET | `/api/v3/cascade` | Cascade pipeline state |
| GET | `/api/v3/decay` | Decay scheduler state |
| GET | `/api/v3/health` | V3 subsystem health |

### Tool Library API

| Method | Path | Purpose |
|--------|------|---------|
| GET | `/api/tools/registration` | Registration status |
| POST | `/api/tools/health-check` | Invoke health check tool |
| POST | `/api/tools/layer-health` | Invoke layer health tool |
| POST | `/api/tools/service-discovery` | Invoke discovery tool |
| POST | `/api/tools/circuit-status` | Invoke circuit breaker tool |
| POST | `/api/tools/submit-remediation` | Submit remediation request |
| POST | `/api/tools/remediation-status` | Check remediation status |

### Metabolic Control API

| Method | Path | Purpose |
|--------|------|---------|
| POST | `/api/metabolic/pause` | Pause EventBus publishing |
| POST | `/api/metabolic/resume` | Resume EventBus publishing |
| GET | `/api/cognitive-state` | Cognitive state snapshot |
| GET | `/api/eventbus/stats` | EventBus channel statistics |
| GET | `/api/field` | Cached PV2 field state |

---

## 5. EventBus Channels (6 default + 2 planned)

| Channel | Publishers | Purpose |
|---------|-----------|---------|
| `health` | health_poller, M10 | Per-service health checks, cycle summaries |
| `remediation` | M14 | Remediation actions, outcomes |
| `learning` | L5 modules | STDP events, pathway changes |
| `consensus` | M31-M36 | PBFT proposals, votes, outcomes |
| `integration` | M19-M24 | Bridge events, peer status |
| `metrics` | health_poller | Performance metrics, throughput |
| `auth` (V2) | M51 | Auth success/failure events |
| `prediction` (V2) | M54 | Failure predictions, sequence detections |

### EventBus → PV2 Bridge

The `spawn_pv2_eventbus_bridge` at main.rs:303 polls all 6 channels every 10s and POSTs new events to `PV2:8132/bus/events`. Uses last-seen event ID per channel to avoid re-posting.

---

## 6. ORAC Integration (Bidirectional)

### Current: ORAC → ME (m23_me_bridge)

```rust
// orac-sidecar/src/m5_bridges/m23_me_bridge.rs
pub struct MeBridge {
    addr: String,           // "127.0.0.1:8080"
    poll_interval_secs: u64, // 10
}

// Polls:
//   GET /api/health   → extracts last_fitness
//   GET /api/observer → extracts fitness, correlations, ralph_cycles, frozen status
```

ORAC's MeBridge records:
- `me_fitness`: f64 (current fitness from /api/health)
- `me_frozen`: bool (whether ME is metabolically paused)
- `me_observer_subscribed`: bool (whether observer is receiving events)

### Planned V2: ME → ORAC (M53 OracBridge)

```
ME:8080 ──OracBridge──> ORAC:8133/health (read fitness, RALPH gen, emergence, coupling)
ME:8080 ──OracBridge──> ORAC:8133/blackboard (read fleet state)
ME:8080 ──OracBridge──> ORAC:8133/hooks/PostToolUse (push ME events to ORAC)
```

This creates the missing inbound leg — ME can see ORAC's intelligence layer.

---

## 7. Database Wiring

| Database | Written By | Read By | Connection |
|----------|-----------|---------|------------|
| service_tracking.db | ME health_poller | ME L2, ORAC me_bridge | Direct SQLite |
| system_synergy.db | ME peer_bridge | ME L4 | Direct SQLite |
| hebbian_pulse.db | ME L5 learning | ME L7 observer | Direct SQLite |
| consensus_tracking.db | ME L6 PBFT | ME L7 observer | Direct SQLite |
| evolution_tracking.db | ME L7 observer | ME L7 RALPH | Direct SQLite |
| performance_metrics.db | ME observer_tick | ME L7 fitness | Direct SQLite |
| tensor_memory.db | ME observer_tick | ME L7 fitness | Direct SQLite |

---

## 8. Wiring Schematic (ASCII)

```
                                  ULTRAPLATE HABITAT
    ┌─────────────────────────────────────────────────────────────────────┐
    │                                                                     │
    │   ┌──────────┐     ┌──────────┐     ┌──────────┐                   │
    │   │ DevOps   │     │ SYNTHEX  │     │  SAN-K7  │                   │
    │   │  :8081   │     │  :8090   │     │  :8100   │                   │
    │   └────┬─────┘     └────┬─────┘     └────┬─────┘                   │
    │        │health          │health+v3       │health                    │
    │        │pipeline        │thermal         │                          │
    │        ▼                ▼                 ▼                          │
    │   ┌─────────────────────────────────────────────┐                   │
    │   │                                             │                   │
    │   │          MAINTENANCE ENGINE V2              │                   │
    │   │               :8080                         │                   │
    │   │                                             │                   │
    │   │  ┌─L1 Foundation (10 mod)──────────────┐    │                   │
    │   │  │ Error Config Logging Metrics State   │    │                   │
    │   │  │ Resources Signals TensorReg NAM      │    │                   │
    │   │  └──────────────────────────────────────┘    │                   │
    │   │  ┌─L2 Services (4 mod)──┐ ┌─L3 Core (6)─┐  │                   │
    │   │  │ Registry Health      │ │ Pipeline     │  │                   │
    │   │  │ Lifecycle Resilience │ │ Remediation  │  │                   │
    │   │  └──────────────────────┘ └──────────────┘  │                   │
    │   │  ┌─L4 Integration (9 mod)──────────────┐    │                   │
    │   │  │ REST gRPC WS IPC EventBus Bridge    │    │                   │
    │   │  │ CascadeBridge PeerBridge ToolReg     │    │                   │
    │   │  └──────────────────────────────────────┘    │                   │
    │   │  ┌─L5 Learning (7)──┐ ┌─L6 Consensus (6)┐  │                   │
    │   │  │ Hebbian STDP     │ │ PBFT Agents      │  │                   │
    │   │  │ Pattern Pruner   │ │ Voting Dissent   │  │                   │
    │   │  └──────────────────┘ └──────────────────┘  │                   │
    │   │  ┌─L7 Observer (6 mod)─────────────────┐    │                   │
    │   │  │ LogCorrelator Emergence Evolution    │    │                   │
    │   │  │ Thermal ObserverBus Fitness          │    │                   │
    │   │  └──────────────────────────────────────┘    │                   │
    │   │  ┌─L8 Nexus (6 stubs — V2 NEW)─────────┐   │                   │
    │   │  │ FieldBridge IntentRouter RegimeMgr   │   │                   │
    │   │  │ StdpBridge EvolutionGate Morphogenic │   │                   │
    │   │  └──────────────────────────────────────┘   │                   │
    │   │                                             │                   │
    │   └──────────────┬────────────┬─────────────────┘                   │
    │                  │            │                                      │
    │         EventBus │    ┌───────┘ /api/*                              │
    │         bridge   │    │                                              │
    │         10s POST │    │ 10s GET                                      │
    │                  ▼    ▼                                              │
    │   ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐          │
    │   │  PV2     │  │  ORAC    │  │  POVM    │  │   RM     │          │
    │   │  :8132   │  │  :8133   │  │  :8125   │  │  :8130   │          │
    │   │ bus/events│  │ me_bridge│  │ /health  │  │ /health  │          │
    │   └──────────┘  └──────────┘  └──────────┘  └──────────┘          │
    │                                                                     │
    │   ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐          │
    │   │  NAIS    │  │  Bash    │  │   TM     │  │   CCM    │          │
    │   │  :8101   │  │  :8102   │  │  :8103   │  │  :8104   │          │
    │   └──────────┘  └──────────┘  └──────────┘  └──────────┘          │
    │                                                                     │
    │   ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐          │
    │   │   TL     │  │  CSV7   │  │  VMS     │  │  Arch    │          │
    │   │  :8105   │  │  :8110   │  │  :8120   │  │  :9001   │          │
    │   │ tools reg│  └──────────┘  └──────────┘  └──────────┘          │
    │   └──────────┘                                                     │
    │                              ┌──────────┐                           │
    │                              │  Prom    │                           │
    │                              │  :10001  │                           │
    │                              └──────────┘                           │
    └─────────────────────────────────────────────────────────────────────┘
```

---

## 9. DevEnv Registration

```toml
[[services]]
id = "maintenance-engine-v2"
name = "Maintenance Engine V2"
description = "8-layer maintenance framework with Nexus integration (Port 8080)"
working_dir = "/home/louranicas/claude-code-workspace/the_maintenance_engine_v2"
command = "./bin/maintenance_engine_v2"
args = ["start", "--port", "8080"]
auto_start = true
auto_restart = true
max_restart_attempts = 5
restart_delay_secs = 3
health_check_interval_secs = 30
startup_timeout_secs = 30
shutdown_timeout_secs = 15
dependencies = ["devops-engine"]

[services.env]
RUST_LOG = "maintenance_engine_v2=info"
PORT = "8080"

[services.resource_limits]
max_memory_mb = 256
max_cpu_percent = 25
```

---

## 10. ORAC Hook Integration

ME V2 uses ORAC hooks via `hooks/orac-hook.sh` forwarder:

| Event | ORAC Endpoint | Timeout | What It Does |
|-------|--------------|---------|--------------|
| SessionStart | `/hooks/SessionStart` | 5s | Register sphere, hydrate POVM+RM |
| UserPromptSubmit | `/hooks/UserPromptSubmit` | 3s | Inject field state + pending tasks |
| PreToolUse | `/hooks/PreToolUse` | 2s | SYNTHEX thermal gate |
| PostToolUse | `/hooks/PostToolUse` | 3s | Record memory, 1-in-5 task poll |
| Stop | `/hooks/Stop` | 5s | Fail tasks, crystallize, deregister |
| PermissionRequest | `/hooks/PermissionRequest` | 2s | Auto-approve/deny policy |

---

## 11. Memory Substrate Connections

| Substrate | Protocol | Write | Read |
|-----------|----------|-------|------|
| POVM:8125 | HTTP POST /memories | Session crystallize | /hydrate on startup |
| RM:8130 | HTTP POST /put (TSV!) | Integration records | /search?q= |
| PV2:8132 | HTTP POST /sphere/*/register | Sphere lifecycle | /health, /spheres |
| ORAC:8133 | Via hooks | PostToolUse events | /health (planned M53) |
| SQLite (12 DBs) | Direct file I/O | All layers | All layers |

**TRAP:** RM accepts ONLY TSV format: `category\tagent\tconfidence\tttl\tcontent`. NEVER JSON.

---

## 12. Synergy Scores (from system_synergy.db)

| Source | Target | Score | Type |
|--------|--------|-------|------|
| ME | devenv | 95.0 | request_reply |
| ME | K7 | 88.0 | sync |
| ME | NAIS | 86.0 | sync |
| ME | SYNTHEX | 85.0 | sync |
| ME | TL | 84.0 | sync |

---

*Habitat Integration Spec | 17 Services | 30+ Endpoints | 12 Background Tasks*
*Generated: 2026-03-28 | Session 068*
