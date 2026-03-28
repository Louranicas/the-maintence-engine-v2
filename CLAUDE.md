# The Maintenance Engine V2 - Bootstrap Context

[README](README.md) | [**MASTER_INDEX**](MASTER_INDEX.md) | [**MASTER_PLAN**](SCAFFOLDING_MASTER_PLAN.md) | [Quick Start](ai_docs/QUICKSTART.md) | [AI Docs](ai_docs/INDEX.md) | [AI Specs](ai_specs/INDEX.md) | [Mind Map](ai_docs/META_TREE_MIND_MAP_V2.md) | [Habitat Wiring](ai_specs/HABITAT_INTEGRATION_SPEC.md) | [Evolution V2](ai_docs/ADVANCED_EVOLUTION_CHAMBER_V2.md) | [Modules](ai_docs/modules/INDEX.md) | [.claude/](.claude/context.json)

```json
{"v":"2.0.0","status":"COMPILED","modules":48,"layers":8,"loc":62522,"tests":2288,"clippy":0,"databases":12,"pipelines":8,"services":13,"hooks":14,"tensor_dims":12,"port":8080,"nam_target":0.95,"pbft":{"n":40,"f":13,"q":27},"nexus":true,"ovm":true,"k_regime":"adaptive","source_files":73}
```

**Current State:** SCAFFOLDED — M1 Foundation (16,711 LOC) + M2 Services (7,196 LOC) cloned as gold standard. 12 databases cloned (5.9MB). 155+ supporting assets. L3-L8 await implementation.

---

## Overview

The Maintenance Engine V2 is the next-generation maintenance framework for the ULTRAPLATE Developer Environment, evolved from ME v1 with deep Nexus Controller and Oscillating Vortex Memory integration. It provides autonomous service management, health monitoring, Hebbian learning, PBFT consensus, NAM-compliant multi-agent coordination, Kuramoto field coherence tracking, and morphogenic adaptation.

**V2 Enhancements over V1:**
- L8 Nexus Integration Layer (6 new modules: N01-N06)
- Kuramoto r-tracking and K-regime awareness (Swarm/Fleet/Armada)
- STDP tool chain learning from VMS patterns
- Evolution Chamber mutation testing before deployments
- Morphogenic adaptation triggers (|r_delta| > 0.05)
- 384D semantic drift detection via Saturn Light
- Cross-session learning persistence

---

## Quick Commands

```bash
# Build
CARGO_TARGET_DIR=/tmp/cargo-maintenance-v2 cargo build --release

# Test
CARGO_TARGET_DIR=/tmp/cargo-maintenance-v2 cargo test --lib --release

# Clippy (god-tier)
CARGO_TARGET_DIR=/tmp/cargo-maintenance-v2 cargo clippy --release -- -D warnings -W clippy::pedantic

# Quality Gate Chain (MANDATORY order)
CARGO_TARGET_DIR=/tmp/cargo-maintenance-v2 cargo check 2>&1 | tail -20 && \
cargo clippy -- -D warnings 2>&1 | tail -20 && \
cargo clippy -- -D warnings -W clippy::pedantic 2>&1 | tail -20 && \
CARGO_TARGET_DIR=/tmp/cargo-maintenance-v2 cargo test --lib --release 2>&1 | tail -30

# Run
./target/release/maintenance_engine_v2 start --port 8080

# Health check
curl http://localhost:8080/api/health

# Database integrity
for db in data/databases/*.db; do echo "$(basename $db): $(sqlite3 $db 'PRAGMA integrity_check;')"; done
```

---

## Architecture: 8 Layers, 48+ Modules

```
src/
├── lib.rs                        # Crate root, prelude, module declarations
├── main.rs                       # Axum HTTP server, 30+ routes
├── engine.rs                     # MaintenanceEngineV2 orchestrator
├── database.rs                   # DatabaseManager (12 SQLite databases)
│
├── m1_foundation/                # L1: Foundation (CLONED — 16,711 LOC, gold standard)
│   ├── error.rs                  # M01: Error taxonomy
│   ├── config.rs                 # M02: Configuration manager
│   ├── logging.rs                # M03: Structured logging
│   ├── metrics.rs                # M04: Metrics collector
│   ├── state.rs                  # M05: State persistence
│   ├── resources.rs              # M06: Resource manager
│   ├── signals.rs                # M07: Signal bus
│   ├── tensor_registry.rs        # M08: Tensor registry
│   ├── shared_types.rs           # M00: Shared types (Timestamp, Duration, ServiceId)
│   ├── nam.rs                    # NAM foundation types
│   └── mod.rs                    # Layer coordinator
│
├── m2_services/                  # L2: Services (CLONED — 7,196 LOC, gold standard)
│   ├── service_registry.rs       # M09: Service Registry (53 tests)
│   ├── health_monitor.rs         # M10: Health Monitor (49 tests)
│   ├── lifecycle.rs              # M11: Lifecycle Manager (75 tests)
│   ├── resilience.rs             # M12: Circuit Breaker + Load Balancer (82 tests)
│   └── mod.rs                    # Layer coordinator (20 tests)
│
├── m3_core_logic/                # L3: Core Logic (PENDING — target ~8,000 LOC)
│   ├── pipeline.rs               # M13: Pipeline manager
│   ├── remediation.rs            # M14: Remediation engine
│   ├── confidence.rs             # M15: Confidence calculator
│   ├── action.rs                 # M16: Action executor
│   ├── outcome.rs                # M17: Outcome recorder
│   ├── feedback.rs               # M18: Feedback loop
│   └── mod.rs                    # Layer coordinator
│
├── m4_integration/               # L4: Integration (PENDING — target ~7,500 LOC)
│   ├── rest.rs                   # M19: REST client
│   ├── grpc.rs                   # M20: gRPC client
│   ├── websocket.rs              # M21: WebSocket client
│   ├── ipc.rs                    # M22: IPC manager
│   ├── event_bus.rs              # M23: Event bus
│   ├── bridge.rs                 # M24: Bridge manager
│   ├── peer_bridge.rs            # M24b: Peer bridge communication
│   ├── tool_registrar.rs         # M24c: Tool Library registration
│   └── mod.rs                    # Layer coordinator
│
├── m5_learning/                  # L5: Learning (PENDING — target ~7,000 LOC)
│   ├── hebbian.rs                # M25: Hebbian manager
│   ├── stdp.rs                   # M26: STDP processor
│   ├── pattern.rs                # M27: Pattern recognizer
│   ├── pruner.rs                 # M28: Pathway pruner
│   ├── consolidator.rs           # M29: Memory consolidator
│   ├── antipattern.rs            # M30: Anti-pattern detector
│   └── mod.rs                    # Layer coordinator
│
├── m6_consensus/                 # L6: Consensus (PENDING — target ~6,500 LOC)
│   ├── pbft.rs                   # M31: PBFT manager
│   ├── agent.rs                  # M32: Agent coordinator
│   ├── voting.rs                 # M33: Vote collector
│   ├── view_change.rs            # M34: View change handler
│   ├── dissent.rs                # M35: Dissent tracker
│   ├── quorum.rs                 # M36: Quorum calculator
│   └── mod.rs                    # Layer coordinator
│
├── m7_observer/                  # L7: Observer (PENDING — target ~8,500 LOC)
│   ├── observer_bus.rs           # Observer event bus
│   ├── fitness.rs                # 12D tensor fitness evaluator
│   ├── log_correlator.rs         # M37: Cross-layer correlation
│   ├── emergence_detector.rs     # M38: Emergence detection
│   ├── evolution_chamber.rs      # M39: RALPH evolution loop
│   └── mod.rs                    # L7 coordinator
│
├── nexus/                        # L8: NEXUS (NEW — target ~6,000 LOC)
│   ├── field_bridge.rs           # N01: Kuramoto r-tracking, field capture
│   ├── intent_router.rs          # N02: 12D IntentTensor → service routing
│   ├── regime_manager.rs         # N03: K-regime detection (Swarm/Fleet/Armada)
│   ├── stdp_bridge.rs            # N04: Tool chain STDP learning
│   ├── evolution_gate.rs         # N05: Mutation testing before deployments
│   ├── morphogenic_adapter.rs    # N06: Adaptation on |r_delta| > 0.05
│   └── mod.rs                    # Layer coordinator + NexusStatus
│
├── v3_homeostasis/               # HRS-001: Neural Homeostasis (~760 LOC)
│   ├── thermal.rs                # M40: PID thermal controller
│   ├── decay_auditor.rs          # M41: STDP decay audit + correction
│   └── diagnostics.rs            # M42: Diagnostics engine
│
└── tools/                        # Tool Library integration
    ├── mod.rs                    # Tool registrar
    ├── health.rs                 # Health tools
    ├── remediation.rs            # Remediation tools
    ├── learning.rs               # Learning tools
    ├── consensus.rs              # Consensus tools
    ├── observer.rs               # Observer tools
    └── tensor.rs                 # Tensor tools
```

---

## Design Constraints (C1-C12)

| ID | Constraint | Enforcement |
|----|-----------|-------------|
| C1 | No upward imports (strict layer DAG) | Compile-time |
| C2 | Trait methods always `&self` (interior mutability via RwLock) | Code review |
| C3 | Every module implements `TensorContributor` | Compile-time |
| C4 | Zero unsafe, unwrap, expect, clippy warnings | `#![forbid(unsafe_code)]` + clippy deny |
| C5 | No `chrono` or `SystemTime` — use `Timestamp` + `Duration` | Grep + clippy |
| C6 | Signal emissions via `Arc<SignalBus>` on state transitions | Architecture |
| C7 | Owned returns through `RwLock` (never return references) | Code review |
| C8 | Timeouts use `std::time::Duration` | Code review |
| C9 | Existing downstream tests must not break | CI gate |
| C10 | 50+ tests per layer minimum | CI gate |
| C11 | Every L4+ module has Nexus field capture (pre/post r) | Architecture |
| C12 | All service interactions record STDP co-activation (+0.05/call) | Architecture |

---

## 12D Tensor Encoding

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
| D8 | synergy | 0-1 | M24, N01, N04 |
| D9 | latency | 0-1 | M12 |
| D10 | error_rate | 0-1 | M10, M12 |
| D11 | temporal_context | 0-1 | M05, N01 |

---

## 12 Databases

| Database | Purpose | Key Tables |
|----------|---------|------------|
| service_tracking.db | Service lifecycle | services, health_checks, restarts |
| system_synergy.db | Cross-system integration | connections, bridges, synergy_scores |
| hebbian_pulse.db | Neural pathway learning | pathways, ltp_events, ltd_events |
| consensus_tracking.db | PBFT consensus | rounds, votes, dissent_log |
| episodic_memory.db | Episode recording | episodes, contexts, outcomes |
| tensor_memory.db | 12D tensor storage | tensors, snapshots, deltas |
| performance_metrics.db | Performance tracking | metrics, aggregations, alerts |
| flow_state.db | Flow state transitions | states, transitions, checkpoints |
| security_events.db | Security monitoring | events, threats, mitigations |
| workflow_tracking.db | Workflow orchestration | workflows, steps, outcomes |
| evolution_tracking.db | Evolution tracking | fitness, mutations, emergence |
| remediation_log.db | Remediation actions | actions, outcomes, confidence |

---

## PBFT Configuration

```toml
n = 40       # Total agents (CVA-NAM fleet)
f = 13       # Byzantine fault tolerance (n = 3f + 1)
q = 27       # Quorum requirement (2f + 1)
```

### Agent Roles (NAM-05)

| Role | Count | Weight | Focus |
|------|-------|--------|-------|
| VALIDATOR | 20 | 1.0 | Correctness verification |
| EXPLORER | 8 | 0.8 | Alternative detection |
| CRITIC | 6 | 1.2 | Flaw detection |
| INTEGRATOR | 4 | 1.0 | Cross-system impact |
| HISTORIAN | 2 | 0.8 | Precedent matching |

---

## STDP Learning Parameters

```toml
ltp_rate = 0.1           # Long-Term Potentiation
ltd_rate = 0.05          # Long-Term Depression
stdp_window_ms = 100     # Timing window
decay_rate = 0.1         # Weight decay (HRS-001 corrected)
healthy_ratio = [2.0, 4.0]  # LTP:LTD balance
co_activation_delta = 0.05  # Per-call STDP increment (NEW)
```

---

## Nexus Integration (L8 — NEW in V2)

### Kuramoto Parameters
```toml
K_swarm = 0.5            # Swarm regime (K < 1.0) — independent parallel
K_fleet = 1.5            # Fleet regime (1.0 <= K < 2.0) — coordinated
K_armada = 3.0           # Armada regime (K >= 2.0) — synchronized convergence
r_adaptation_threshold = 0.05  # |r_delta| trigger for morphogenic adaptation
```

### Field Capture Pattern
```rust
// Every L4+ operation captures pre/post field state
let r_before = nexus.field_coherence();
/* ... operation ... */
let r_after = nexus.field_coherence();
let r_delta = r_after - r_before;
if r_delta.abs() > 0.05 {
    nexus.trigger_morphogenic_adaptation(r_delta);
}
```

### Evolution Gate
```
Before deployment:
1. Snapshot current field state
2. Create mutation (parameter/code change)
3. Run in Evolution Chamber (K=1.0, 500 steps, 5 spheres)
4. Measure r_delta
5. Accept only if r_after >= r_baseline
```

---

## Escalation Tiers

| Tier | Condition | Timeout | Action |
|------|-----------|---------|--------|
| L0 Auto-Execute | confidence >= 0.9, severity <= MEDIUM | 0 | Execute immediately |
| L1 Notify Human | confidence >= 0.7, severity <= HIGH | 5min | Notify, proceed if no response |
| L2 Require Approval | confidence < 0.7 OR severity = HIGH | 30min | Wait for human approval |
| L3 PBFT Consensus | Critical actions (kill, migration) | Quorum | Require 27/40 agent votes |

---

## NAM Compliance (Target: 95%)

| Requirement | ID | Target | Description |
|-------------|-----|--------|-------------|
| SelfQuery | R1 | 93% | Self-observation via Nexus field capture |
| HebbianRouting | R2 | 92% | STDP pathway-based routing |
| DissentCapture | R3 | 90% | Minority opinion recording + cascade semantics |
| FieldVisualization | R4 | 95% | State visualization via 12D tensor + r |
| HumanAsAgent | R5 | 98% | Human @0.A integration at Tier 0 |

---

## Quality Gates

```json
{"unsafe":0,"unwrap":0,"expect":0,"warnings":0,"tests_target":2400,"clippy_pedantic":0,"clippy_nursery":0,"coverage":">=80%"}
```

| Gate | Requirement | Status |
|------|-------------|--------|
| `unsafe` code | Zero | ENFORCED (`#![forbid(unsafe_code)]`) |
| `.unwrap()` | Zero | ENFORCED (`#![deny(clippy::unwrap_used)]`) |
| `.expect()` | Zero | ENFORCED (`#![deny(clippy::expect_used)]`) |
| Clippy pedantic | Zero warnings | ENFORCED |
| Clippy nursery | Zero warnings | ENFORCED |
| Warning suppression | Never `#[allow(...)]` for root-cause fixes | ENFORCED |
| Tests per layer | >= 50 | REQUIRED |
| Test coverage | >= 80% | TARGET |

---

## Key Patterns (Inherited from Gold Standard)

```rust
// Builder pattern (all constructors)
ServiceDefinition::builder("id", "name").tier(2).port(8080).build()?;

// Result everywhere, never panic
fn operation(&self) -> Result<T> { ... }

// Interior mutability for trait compat
pub struct Module { inner: parking_lot::RwLock<ModuleInner> }

// TensorContributor (every module)
impl TensorContributor for Module {
    fn contribute_tensor(&self) -> TensorContribution { ... }
}

// Signal emission on state transitions
self.signal_bus.emit(Signal::HealthChanged { service_id, old, new });

// Scoped lock guards (drop early)
{ let guard = self.inner.read(); /* use */ }

// FMA for float precision
0.3f64.mul_add(a, 0.25f64.mul_add(b, 0.2 * c))
```

---

## Build Sequence (7 Phases)

| Phase | Action | Gate |
|-------|--------|------|
| 1 | Cargo.toml + lib.rs + mod declarations | `cargo check` clean |
| 2 | L3 Core Logic (M13-M18) | Quality gate pass |
| 3 | L4 Integration (M19-M24b) | Quality gate pass |
| 4 | L5 Learning (M25-M30) | Quality gate pass |
| 5 | L6 Consensus (M31-M36) | Quality gate pass |
| 6 | L7 Observer (M37-M39 + infra) | Quality gate pass |
| 7 | L8 Nexus (N01-N06) + engine.rs + main.rs | Full integration pass |

---

## Directory Cross-Reference

| Need | Path |
|------|------|
| Master scaffolding plan | `SCAFFOLDING_MASTER_PLAN.md` |
| Module specs | `ai_docs/modules/M{01-42}.md` |
| Layer specs | `ai_docs/layers/L{01-07}.md` |
| L8 Nexus specs | `ai_specs/nexus-specs/` |
| Database schemas | `migrations/` + `config/database.toml` |
| Design patterns | `ai_specs/patterns/*.md` |
| Integration tests | `tests/` |
| Benchmarks | `benches/` |
| Config files | `config/*.toml` |
| Architecture schematics | `ai_docs/schematics/` |

---

## ME v1 Reference (Primary Exemplar)

| Metric | V1 | V2 Target |
|--------|-----|-----------|
| Layers | 7 | 8 (+Nexus) |
| Modules | 45 | 48+ |
| LOC | 54,412 | 65,000+ |
| Tests | 1,536 | 2,400+ |
| Databases | 11 | 12 |
| Clippy warnings | 0 | 0 |
| NAM target | 92% | 95% |

---

## Obsidian Vault

**Active vault:** `/home/louranicas/projects/claude_code/`

Key notes:
- `ULTRAPLATE Developer Environment.md` — all 14 services, ports, commands
- `Vortex Sphere Brain-Body Architecture.md` — Pentagon, Kuramoto, tensor, evolution
- `Oscillating Vortex Memory.md` — field equations, fractal sphere, 47 MCP tools
- `Nexus Controller V2.md` — system state, 3 implementations, new modules

---

## GOD-TIER Bash Patterns

> Full reference in root workspace [CLAUDE.md](../CLAUDE.md). Key patterns:

| ID | Pattern | Use |
|----|---------|-----|
| B1 | SQLite State Query | `sqlite3 -header -column DB "SELECT ...;"` |
| B2 | Quality Gate Chain | `check → clippy → pedantic → test` |
| B3 | Health Check | `curl -s -o /dev/null -w "%{http_code}" URL` |
| B11 | Heredoc SQLite | N queries in 1 invocation |
| B17 | rg Multi-Pattern | `rg '(pat1\|pat2)' --type rs` |
| TC1 | Funnel Discovery | `Grep(files) → Read → Edit → Bash(verify)` |
| TC4 | SQLite State Loop | `Read → Act → Write` (130 tokens) |
| TC5 | Build-Fix Converge | `build\|tail → Read(offset) → Edit → verify\|tail-5` |

**Anti-patterns:** cat→Read, grep→Grep, find→Glob, sed→Edit, echo→Write, curl -sf→curl -s

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

> These commands work from ANY service directory. They are defined at `orac-sidecar/.claude/commands/`.

| Command | What It Does |
|---------|-------------|
| `/gate` | 4-stage quality gate: check → clippy → pedantic → test |
| `/sweep` | Probe 17 services + ORAC + thermal + field |
| `/deploy-orac` | Build → deploy → verify (encodes all traps) |
| `/acp` | Adversarial Convergence Protocol (3 rounds) |
| `/battern` | Fleet batch dispatch: roles → gate → collect |
| `/nerve` | Continuous Nerve Center dashboard (10s refresh) |


> These commands work from ANY service directory. They are defined at `orac-sidecar/.claude/commands/`.

| Command | What It Does |
|---------|-------------|
| `/gate` | 4-stage quality gate: check → clippy → pedantic → test |
| `/sweep` | Probe 17 services + ORAC + thermal + field |
| `/deploy-orac` | Build → deploy → verify (encodes all traps) |
| `/acp` | Adversarial Convergence Protocol (3 rounds) |
| `/battern` | Fleet batch dispatch: roles → gate → collect |
| `/nerve` | Continuous Nerve Center dashboard (10s refresh) |
| `/propagate` | Push command table to all service CLAUDE.md files |
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


## Habitat Slash Commands (Session 065)

> These commands work from ANY service directory. They are defined at `orac-sidecar/.claude/commands/` and workspace `.claude/skills/`.

| Command | What It Does |
|---------|-------------|
| `/gate` | 4-stage quality gate: check → clippy → pedantic → test |
| `/sweep` | Probe 17 services + ORAC + thermal + field |
| `/deploy-orac` | Build → deploy → verify (encodes all traps) |
| `/forge` | Generic build → deploy → verify for ANY Rust service |
| `/genesis` | New service from zero to running (scaffold + register + deploy) |
| `/integrate` | Wire service into Habitat (hooks + bridges + PV2 + RM + POVM) |
| `/acp` | Adversarial Convergence Protocol (3 rounds) |
| `/battern` | Fleet batch dispatch: roles → gate → collect |
| `/nerve` | Continuous Nerve Center dashboard (10s refresh) |
| `/propagate` | Push command table to all service CLAUDE.md files |
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

