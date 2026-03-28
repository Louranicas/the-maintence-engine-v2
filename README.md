# The Maintenance Engine V2

Next-generation maintenance framework for the ULTRAPLATE Developer Environment with Nexus Controller integration, Kuramoto field coherence tracking, and morphogenic adaptation.

**Port:** 8080 | **Layers:** 8 | **Modules:** 48+ | **LOC:** 62,522 | **Tests:** 2,288

---

## Quick Start

```bash
# Build
CARGO_TARGET_DIR=/tmp/cargo-maintenance-v2 cargo build --release

# Run
./target/release/maintenance_engine_v2 start --port 8080

# Health check
curl http://localhost:8080/api/health

# Quality gate (mandatory before commits)
CARGO_TARGET_DIR=/tmp/cargo-maintenance-v2 cargo check && \
cargo clippy -- -D warnings && \
cargo clippy -- -D warnings -W clippy::pedantic && \
CARGO_TARGET_DIR=/tmp/cargo-maintenance-v2 cargo test --lib --release
```

---

## Architecture

```
L8 NEXUS (NEW)     6 stubs    — Field bridge, intent routing, K-regime, STDP bridge
L7 OBSERVER        6 modules  — Correlation, emergence, RALPH evolution, thermal
L6 CONSENSUS       6 modules  — PBFT (n=40, f=13, q=27), agents, voting, dissent
L5 LEARNING        7 modules  — Hebbian, STDP, pattern recognition, pruning
L4 INTEGRATION     9 modules  — REST, gRPC, WebSocket, IPC, EventBus, bridges
L3 CORE LOGIC      6 modules  — Pipeline, remediation, confidence, action
L2 SERVICES        4 modules  — Registry, health, lifecycle, resilience
L1 FOUNDATION     10 modules  — Error, config, logging, metrics, signals, tensor
```

---

## V2 Enhancements

- **L8 Nexus Integration** — Kuramoto r-tracking, K-regime awareness, morphogenic adaptation
- **Advanced Evolution Chamber** — 3-tier architecture, field-gated mutations, 4-source Learn phase
- **NAM 95% target** — up from 92%, with Nexus field capture on every L4+ operation
- **STDP co-activation** — +0.05 per service interaction (C12 constraint)

---

## Documentation

| Document | Path | Purpose |
|----------|------|---------|
| **Master Index** | [`MASTER_INDEX.md`](MASTER_INDEX.md) | Complete project inventory |
| **Quick Start** | [`ai_docs/QUICKSTART.md`](ai_docs/QUICKSTART.md) | Build, run, navigate |
| **Scaffolding Plan** | [`SCAFFOLDING_MASTER_PLAN.md`](SCAFFOLDING_MASTER_PLAN.md) | V2 architecture blueprint |
| **Meta Tree Mind Map** | [`ai_docs/META_TREE_MIND_MAP_V2.md`](ai_docs/META_TREE_MIND_MAP_V2.md) | Full architecture tree + habitat wiring |
| **Evolution Chamber V2** | [`ai_docs/ADVANCED_EVOLUTION_CHAMBER_V2.md`](ai_docs/ADVANCED_EVOLUTION_CHAMBER_V2.md) | RALPH redesign from 200h+ live data |
| **Habitat Integration** | [`ai_specs/HABITAT_INTEGRATION_SPEC.md`](ai_specs/HABITAT_INTEGRATION_SPEC.md) | Full wiring map + schematics |
| **Module Index** | [`ai_docs/modules/INDEX.md`](ai_docs/modules/INDEX.md) | 55 module documentation files |
| **AI Specs** | [`ai_specs/INDEX.md`](ai_specs/INDEX.md) | 79 specification files |

### Claude Code Integration

| Resource | Path | Purpose |
|----------|------|---------|
| **Bootstrap Context** | [`CLAUDE.md`](CLAUDE.md) | Session bootstrap for Claude Code |
| **Local Dev Context** | [`CLAUDE.local.md`](CLAUDE.local.md) | Session state + phase tracking |
| **Machine Context** | [`.claude/context.json`](.claude/context.json) | Machine-readable module inventory |
| **Status Heartbeat** | [`.claude/status.json`](.claude/status.json) | Ultra-compact current state |
| **Patterns (P01-P22)** | [`.claude/patterns.json`](.claude/patterns.json) | 22 mandatory Rust patterns |
| **Alignment Check** | [`.claude/ALIGNMENT_VERIFICATION.md`](.claude/ALIGNMENT_VERIFICATION.md) | Triple alignment verification |

---

## Key Parameters

| Parameter | Value |
|-----------|-------|
| PBFT n/f/q | 40/13/27 |
| STDP LTP/LTD | 0.1/0.05 |
| Decay rate | 0.1 (HRS-001) |
| NAM target | 95% |
| Tensor dims | 12 |
| K Swarm/Fleet/Armada | 0.5/1.5/3.0 |
| r adaptation threshold | 0.05 |

---

## Related Projects

| Service | Port | Repository |
|---------|------|-----------|
| ME V1 (running) | 8080 | `the_maintenance_engine/` |
| ORAC Sidecar | 8133 | `orac-sidecar/` |
| Pane-Vortex V2 | 8132 | `pane-vortex-v2/` |
| SYNTHEX | 8090 | `developer_environment_manager/synthex/` |
| VMS | 8120 | `vortex-memory-system/` |

---

*Maintenance Engine V2 | 8 Layers | 62,522 LOC | 2,288 Tests | 0 Clippy Warnings*
