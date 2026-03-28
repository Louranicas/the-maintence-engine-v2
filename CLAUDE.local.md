# The Maintenance Engine V2 - Local Development Context

## Habitat Bootstrap (New Context Window)

Run these 4 commands at the start of every new context window:

1. `/zellij-mastery` — Zellij config, layouts, plugins, dispatch stack, keybinds
2. `/primehabitat` — The Habitat: 17 services, IPC bus, memory systems, fleet
3. `/deephabitat` — deep substrate: wire protocol, databases, ecosystem, tools
4. `/sweep` — probes all 17 services + ORAC + thermal + field coherence

Then read this file for ME V2-specific context.

```json
{"v":"2.0.0","status":"SCAFFOLDED","modules_cloned":16,"layers_cloned":2,"cloned_loc":23907,"databases":12,"databases_size_mb":5.9,"supporting_files":155,"directories":55,"total_files":209,"nexus_modules":6,"target_loc":65000,"target_tests":2400}
```

---

## Session State

| Metric | Value |
|--------|-------|
| **Status** | SCAFFOLDED (M1+M2 cloned, L3-L8 await coding) |
| **Cloned LOC** | 23,907 (M1: 16,711 + M2: 7,196) |
| **Cloned Files** | 16 Rust source files |
| **Total Files** | 209 (source + docs + specs + configs + tests + benchmarks) |
| **Directories** | 55 |
| **Databases** | 12 cloned (5.9MB) |
| **Clippy** | 0 warnings on cloned code |
| **Target LOC** | 65,000+ |
| **Target Tests** | 2,400+ |

---

## Implementation Status

```
Phase 0: Scaffolding -- COMPLETE
  [x] Directory structure (55 dirs)
  [x] M1 Foundation cloned (11 files, 16,711 LOC)
  [x] M2 Services cloned (5 files, 7,196 LOC)
  [x] 12 databases cloned (5.9MB)
  [x] 11 SQL migrations
  [x] 10 TOML configs
  [x] 8 benchmark files
  [x] 17 integration test files
  [x] 10 NAM docs
  [x] 14 AI spec files
  [x] M1/M2 module specs
  [x] 12 pattern specs
  [x] 37 module docs (M01-M36)
  [x] 7 layer docs (L01-L07)
  [x] SCAFFOLDING_MASTER_PLAN.md
  [x] CLAUDE.md
  [x] CLAUDE.local.md
  [x] Architectural schematics (Mermaid)
  [x] Per-layer spec sheets (L3-L8)
  [ ] Cargo.toml (waiting for "start coding")
  [ ] lib.rs (waiting for "start coding")
  [ ] engine.rs (waiting for "start coding")
  [ ] main.rs (waiting for "start coding")

Phase 1: Foundation Verification -- PENDING
  [ ] Verify M1+M2 compile
  [ ] Verify M1+M2 tests pass
  [ ] Verify clippy clean

Phase 2: L3 Core Logic -- PENDING
  [ ] M13 Pipeline manager
  [ ] M14 Remediation engine
  [ ] M15 Confidence calculator
  [ ] M16 Action executor
  [ ] M17 Outcome recorder
  [ ] M18 Feedback loop
  [ ] mod.rs coordinator
  [ ] Quality gate pass

Phase 3: L4 Integration -- PENDING
  [ ] M19 REST client
  [ ] M20 gRPC client
  [ ] M21 WebSocket client
  [ ] M22 IPC manager
  [ ] M23 Event bus
  [ ] M24 Bridge manager
  [ ] M24b Peer bridge
  [ ] M24c Tool registrar
  [ ] mod.rs coordinator
  [ ] Quality gate pass

Phase 4: L5 Learning -- PENDING
  [ ] M25 Hebbian manager
  [ ] M26 STDP processor
  [ ] M27 Pattern recognizer
  [ ] M28 Pathway pruner
  [ ] M29 Memory consolidator
  [ ] M30 Anti-pattern detector
  [ ] mod.rs coordinator
  [ ] Quality gate pass

Phase 5: L6 Consensus -- PENDING
  [ ] M31 PBFT manager
  [ ] M32 Agent coordinator
  [ ] M33 Vote collector
  [ ] M34 View change handler
  [ ] M35 Dissent tracker
  [ ] M36 Quorum calculator
  [ ] mod.rs coordinator
  [ ] Quality gate pass

Phase 6: L7 Observer -- PENDING
  [ ] Observer bus
  [ ] Fitness evaluator
  [ ] M37 Log correlator
  [ ] M38 Emergence detector
  [ ] M39 Evolution chamber
  [ ] mod.rs coordinator
  [ ] Quality gate pass

Phase 7: L8 Nexus (NEW) -- PENDING
  [ ] N01 Field bridge
  [ ] N02 Intent router
  [ ] N03 Regime manager
  [ ] N04 STDP bridge
  [ ] N05 Evolution gate
  [ ] N06 Morphogenic adapter
  [ ] mod.rs coordinator
  [ ] Quality gate pass

Phase 8: Integration + Server -- PENDING
  [ ] engine.rs (MaintenanceEngineV2 orchestrator)
  [ ] main.rs (Axum HTTP server)
  [ ] database.rs (DatabaseManager)
  [ ] V3 homeostasis integration
  [ ] Tool Library registration
  [ ] Cross-layer wiring
  [ ] Full integration tests
  [ ] Benchmark suite
  [ ] Final quality gate
```

---

## Module Map (48+ Modules)

### L1: Foundation (CLONED — Gold Standard)

| Module | File | LOC | Tests | Status |
|--------|------|-----|-------|--------|
| M00 Shared Types | `shared_types.rs` | 1,049 | — | CLONED |
| M01 Error Taxonomy | `error.rs` | 1,396 | — | CLONED |
| M02 Configuration | `config.rs` | 1,755 | 14 | CLONED |
| M03 Logging | `logging.rs` | 854 | 15 | CLONED |
| M04 Metrics | `metrics.rs` | 1,920 | 12 | CLONED |
| M05 State Persistence | `state.rs` | 2,024 | 10 | CLONED |
| M06 Resource Manager | `resources.rs` | 1,906 | 16 | CLONED |
| M07 Signal Bus | `signals.rs` | 1,111 | — | CLONED |
| M08 Tensor Registry | `tensor_registry.rs` | 1,349 | — | CLONED |
| NAM Foundation | `nam.rs` | 645 | — | CLONED |
| Layer Coordinator | `mod.rs` | 2,702 | — | CLONED |
| **Subtotal** | | **16,711** | **67+** | |

### L2: Services (CLONED — Gold Standard)

| Module | File | LOC | Tests | Status |
|--------|------|-----|-------|--------|
| M09 Service Registry | `service_registry.rs` | 1,285 | 53 | CLONED |
| M10 Health Monitor | `health_monitor.rs` | 1,130 | 49 | CLONED |
| M11 Lifecycle Manager | `lifecycle.rs` | 1,898 | 75 | CLONED |
| M12 Resilience | `resilience.rs` | 2,189 | 82 | CLONED |
| Layer Coordinator | `mod.rs` | 694 | 20 | CLONED |
| **Subtotal** | | **7,196** | **279** | |

### L3-L8: PENDING (Target ~35,000 LOC)

| Layer | Modules | Target LOC | Target Tests |
|-------|---------|-----------|-------------|
| L3 Core Logic | M13-M18 | ~8,000 | 300+ |
| L4 Integration | M19-M24c | ~7,500 | 350+ |
| L5 Learning | M25-M30 | ~7,000 | 300+ |
| L6 Consensus | M31-M36 | ~6,500 | 300+ |
| L7 Observer | M37-M39 + infra | ~8,500 | 350+ |
| L8 Nexus | N01-N06 | ~6,000 | 300+ |

---

## Database Inventory (12 Databases, 5.9MB)

| Database | Size | Rows | Status |
|----------|------|------|--------|
| evolution_tracking.db | 3.6MB | 19,803 fitness | DATA |
| workflow_tracking.db | 280KB | 25 records | DATA |
| service_tracking.db | 260KB | 13 services | DATA |
| security_events.db | 256KB | — | SCHEMA |
| consensus_tracking.db | 248KB | 82 votes | DATA |
| hebbian_pulse.db | 240KB | 2 pulses | DATA |
| flow_state.db | 224KB | 5 states | DATA |
| system_synergy.db | 212KB | 40 pairs | DATA |
| performance_metrics.db | 204KB | 7 samples | DATA |
| episodic_memory.db | 192KB | 25 episodes | DATA |
| tensor_memory.db | 164KB | 6 snapshots | DATA |
| remediation_log.db | 0B | — | EMPTY |

---

## Gold Standard Patterns (from M1+M2)

### 6 Core Traits (L2)
- `ServiceDiscovery` (14 methods) — registry, discovery, health, dependencies
- `HealthMonitoring` (11 methods) — probes, results, aggregation
- `LifecycleOps` (13 methods) — FSM transitions, restart with backoff
- `CircuitBreakerOps` (12 methods) — FSM: Closed→Open→HalfOpen→Closed
- `LoadBalancing` (10 methods) — RoundRobin, Weighted, LeastConnections, Random
- `TensorContributor` (implemented by all modules)

### Key Architectural Patterns
- All `&self` methods with `parking_lot::RwLock` interior mutability
- `Timestamp` + `Duration` everywhere (zero chrono/SystemTime)
- Signal emission via `Arc<dyn SignalBusOps>` on state transitions
- Builder pattern for all constructors
- Result<T> everywhere (zero panic paths)
- Scoped lock guards (explicit drop)
- FMA for float precision

### 12D Tensor Contribution Map
- M09 → D0 (service_id), D2 (tier), D3 (deps), D4 (agents)
- M10 → D6 (health), D10 (error_rate)
- M11 → D6 (health), D7 (uptime)
- M12 → D9 (latency), D10 (error_rate)

---

## Architecture Constants

```rust
// PBFT Consensus
PBFT_N  = 40   // Total agents
PBFT_F  = 13   // Byzantine tolerance
PBFT_Q  = 27   // Quorum (2f + 1)

// STDP Learning
LTP_RATE     = 0.1
LTD_RATE     = 0.05
STDP_WINDOW  = 100ms
DECAY_RATE   = 0.1    // HRS-001 corrected
CO_ACTIVATION_DELTA = 0.05  // NEW: per-call increment

// Nexus / Kuramoto (NEW)
K_SWARM   = 0.5   // K < 1.0
K_FLEET   = 1.5   // 1.0 <= K < 2.0
K_ARMADA  = 3.0   // K >= 2.0
R_THRESHOLD = 0.05 // |r_delta| morphogenic trigger

// Escalation Tiers
L0 = confidence >= 0.9, severity <= MEDIUM
L1 = confidence >= 0.7, severity <= HIGH
L2 = confidence <  0.7 OR severity = HIGH
L3 = critical actions → PBFT consensus (27/40)

// 12D Tensor
// [service_id, port, tier, deps, agents, protocol,
//  health, uptime, synergy, latency, error_rate, temporal]
```

---

## Anti-Patterns (Never Do)

```
[x] unsafe { }           -> forbidden at compile time
[x] .unwrap()            -> denied by clippy
[x] .expect()            -> denied by clippy
[x] panic!()             -> use Result<T>
[x] println!() for logs  -> use tracing macros
[x] chrono::DateTime     -> use Timestamp
[x] SystemTime           -> use Duration
[x] Unbounded channels   -> always set capacity
[x] Clone where move ok  -> clippy::redundant_clone
[x] &mut self traits     -> &self + RwLock
[x] Returning references through RwLock -> clone/owned
```

---

## Cross-Reference Map

| Need | Path |
|------|------|
| Master blueprint | `SCAFFOLDING_MASTER_PLAN.md` |
| Module specs (M01-M36) | `ai_docs/modules/` |
| Layer specs (L01-L07) | `ai_docs/layers/` |
| L8 Nexus specs | `ai_specs/nexus-specs/` |
| Layer spec sheets (L3-L8) | `ai_specs/m3-core-logic-specs/` through `ai_specs/nexus-specs/` |
| Database schemas | `migrations/` + `config/database.toml` |
| Design patterns | `ai_specs/patterns/` |
| Architecture schematics | `ai_docs/schematics/` |
| Integration tests | `tests/` |
| Benchmarks | `benches/` |
| Configs | `config/` |
| ME v1 reference | `/home/louranicas/claude-code-workspace/the_maintenance_engine/` |
| DevOps v2 reference | `/home/louranicas/claude-code-workspace/devops_engine_v2/` |
| VMS reference | `/home/louranicas/claude-code-workspace/vortex-memory-system/` |
| SVF reference | `/home/louranicas/claude-code-workspace/sphere_vortex_framework/` |

---

## ULTRAPLATE Developer Environment

### Quick Start
```bash
~/.local/bin/devenv -c ~/.config/devenv/devenv.toml start
~/.local/bin/devenv -c ~/.config/devenv/devenv.toml status
~/.local/bin/devenv -c ~/.config/devenv/devenv.toml stop
```

### Health Endpoints
```bash
curl http://localhost:8080/api/health   # ME v1
curl http://localhost:8081/health       # DevOps Engine
curl http://localhost:8090/api/health   # SYNTHEX
curl http://localhost:8100/health       # SAN-K7
curl http://localhost:8101/health       # NAIS
curl http://localhost:8105/health       # Tool Library
curl http://localhost:8110/health       # CodeSynthor V7
curl http://localhost:8120/health       # Sphere-Vortex
```

---

## Obsidian Vault

**Active vault:** `/home/louranicas/projects/claude_code/`

Key notes:
- `ULTRAPLATE Developer Environment.md` — all 14 services, ports, commands
- `Vortex Sphere Brain-Body Architecture.md` — Pentagon, Kuramoto, tensor, evolution
- `Oscillating Vortex Memory.md` — field equations, fractal sphere, 47 MCP tools
- `Nexus Controller V2.md` — system state, 3 implementations, new modules

---

*The Maintenance Engine V2 | SCAFFOLDED | Awaiting "start coding"*
*Generated: 2026-03-06*


---

## Swarm Stack V2.1 — Bootstrap & Orchestrator (2026-03-07)

### Developer Environment Bootstrap
```bash
# 1. Source environment (tools + PATH)
source ~/.local/bin/swarm-env.sh

# 2. Full stack bootstrap (builds reasoning-memory, starts services, verifies)
bootstrap

# 3. Verify
evolution-metrics inventory    # All tools
ultraplate-bridge health       # ULTRAPLATE services
curl -s http://localhost:8131/health  # Reasoning memory
```

### Swarm Orchestrator (Zellij)
```bash
# Start Zellij with swarm layout
zellij --layout ~/.config/zellij/swarm-orchestrator.kdl

# Manual fleet dispatch (from Tab 1)
zellij action write-chars --target-pane 10 "claude --dangerously-skip-permissions" && zellij action write 10 13
# Wait for REPL, then dispatch task:
zellij action write-chars --target-pane 10 "your task prompt here" && zellij action write 10 13

# Fleet tools
fleet-ctl status              # Check all fleet tabs
fleet-ctl dispatch 10 "task"  # Send task to specific tab
fleet-heartbeat once           # Scan all fleet health
aggregate state                # Full system state
```

### Key Ports
| Service | Port | Health |
|---------|------|--------|
| Reasoning Memory V2 | 8131 | `/health` |
| ULTRAPLATE (13 svc) | 8080-8120 | Various |
| Swarm Fleet Tabs | 10-15 | `fleet-ctl status` |

### Obsidian Vault
Active: `/home/louranicas/projects/claude_code/`


---

## Habitat Bootstrap Protocol (Fresh Context Window)

**Execute these in order at the start of EVERY new context window:**

| # | Command | What It Loads |
|---|---------|---------------|
| 1 | `/primehabitat` | Zellij tabs, 17 services, IPC bus, memory systems |
| 2 | `/deephabitat` | Wire protocol, 173 DBs, devenv batches, 100+ binaries |
| 3 | Read `CLAUDE.local.md` | Current session state, phase tracking, session history |

**After bootstrap, WAIT for user instruction before taking action.**

### Operational Commands (use as needed after bootstrap)

| Command | When To Use |
|---------|-------------|
| `/gate` | Before every commit — 4-stage quality gate: check → clippy → pedantic → test |
| `/sweep` | Health check — probe all 17 services + ORAC + thermal + field |
| `/deploy-orac` | After ORAC code changes — build → deploy → verify (encodes all traps) |
| `/forge` | After ANY service code changes — generic build → deploy → verify |
| `/genesis` | Create new service from zero — scaffold + register + deploy |
| `/integrate` | Wire service into Habitat — hooks + bridges + PV2 + RM + POVM |
| `/acp` | Complex decisions — Adversarial Convergence Protocol (3 rounds) |
| `/battern` | Multi-pane work — fleet batch dispatch with roles + gates |
| `/nerve` | Live monitoring — continuous Nerve Center dashboard (10s refresh) |
| `/propagate` | After adding commands — push command table to all service CLAUDE.md files |
| `/nvim-mastery` | Neovim RPC: LSP, treesitter, 37 keymaps, 22 snacks features, structural analysis |
| `/atuin-mastery` | Shell history intelligence: search, stats, service density, time-of-day, KV store |
| `/bacon-mastery` | Continuous Rust quality: on_success chaining, socket control, export locations, headless CI |
| `/lazygit-mastery` | Git porcelain: 15 Habitat commands, worktrees, rebase, cherry-pick, bisect |
| `/worktree-mastery` | Multi-agent isolation: 4 worktree systems, fleet patterns, merge coordination |
| `/fzf-mastery` | Universal fuzzy query: --filter pipelines, clustered parallel search, --listen HTTP |
| `/topology` | Structural census across service directories |
| `/metabolic` | Cross-service composite: ME x ORAC x PV2 |
| `/intel` | 17ms habitat pulse: r, gen, fitness, LTP/LTD, thermal |
| `/tensor` | 6D system tensor in ~65ms (temporal, services, memory, synthesis) |
| `/save-session` | Persist session state to all 7 memory substrates |

> Commands defined at `orac-sidecar/.claude/commands/` and workspace `.claude/skills/`. Work from ANY service directory.
