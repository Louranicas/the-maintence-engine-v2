# Maintenance Engine V2 — Quick Start Guide

> **Port:** 8080 | **Layers:** 8 | **Modules:** 48+ | **LOC:** 62,522 | **Tests:** 2,288

---

## Build

```bash
cd /home/louranicas/claude-code-workspace/the_maintenance_engine_v2

# Debug build (fast)
CARGO_TARGET_DIR=/tmp/cargo-maintenance-v2 cargo build

# Release build
CARGO_TARGET_DIR=/tmp/cargo-maintenance-v2 cargo build --release

# Binary location
ls -lh /tmp/cargo-maintenance-v2/release/maintenance_engine_v2
```

## Run

```bash
# Start server
./target/release/maintenance_engine_v2 start --port 8080

# Health check
curl http://localhost:8080/api/health

# Status
./target/release/maintenance_engine_v2 status

# Version
./target/release/maintenance_engine_v2 --version
```

## Quality Gate (MANDATORY — run before every commit)

```bash
CARGO_TARGET_DIR=/tmp/cargo-maintenance-v2 cargo check 2>&1 | tail -5 && \
CARGO_TARGET_DIR=/tmp/cargo-maintenance-v2 cargo clippy -- -D warnings 2>&1 | tail -5 && \
CARGO_TARGET_DIR=/tmp/cargo-maintenance-v2 cargo clippy -- -D warnings -W clippy::pedantic 2>&1 | tail -5 && \
CARGO_TARGET_DIR=/tmp/cargo-maintenance-v2 cargo test --lib --release 2>&1 | tail -5
```

All 4 stages must show **zero errors** and **zero warnings**.

## Architecture (8 Layers)

```
src/
├── m1_foundation/     L1: Error, config, logging, metrics, state, signals, tensor (10 modules)
├── m2_services/       L2: Service registry, health, lifecycle, resilience (4 modules)
├── m3_core_logic/     L3: Pipeline, remediation, confidence, action, outcome (6 modules)
├── m4_integration/    L4: REST, gRPC, WS, IPC, EventBus, bridges (9 modules)
├── m5_learning/       L5: Hebbian, STDP, pattern, pruner, consolidator (7 modules)
├── m6_consensus/      L6: PBFT, agents, voting, view change, dissent, quorum (6 modules)
├── m7_observer/       L7: Correlator, emergence, evolution, thermal, fitness (6 modules)
├── nexus/             L8: Field bridge, intent router, K-regime, STDP bridge (6 stubs — NEW)
├── tools/             Tool Library integration (7 files)
├── lib.rs             Crate root + Tensor12D + prelude
├── main.rs            Axum HTTP server (30+ routes, background tasks)
├── engine.rs          MaintenanceEngine orchestrator
└── database.rs        DatabaseManager (12 SQLite databases)
```

## Key Endpoints

| Method | Path | Purpose |
|--------|------|---------|
| GET | `/api/health` | Health check (JSON) |
| GET | `/api/status` | Full engine status |
| GET | `/api/services` | Service mesh overview |
| GET | `/api/layers` | Per-layer health breakdown |
| GET | `/api/observer` | Observer state (fitness, RALPH) |
| GET | `/api/learning` | Hebbian learning state |
| GET | `/api/consensus` | PBFT consensus state |
| GET | `/api/eventbus` | EventBus channel stats |
| GET | `/metrics` | Prometheus metrics |

## Database Check

```bash
for db in data/databases/*.db; do
  echo "$(basename $db): $(sqlite3 $db 'PRAGMA integrity_check;')"
done
```

## Navigation

| Need | Go To |
|------|-------|
| Full inventory | [MASTER_INDEX.md](../MASTER_INDEX.md) |
| README | [README.md](../README.md) |
| Architecture blueprint | [SCAFFOLDING_MASTER_PLAN.md](../SCAFFOLDING_MASTER_PLAN.md) |
| Bootstrap context | [CLAUDE.md](../CLAUDE.md) |
| Module specs | [modules/INDEX.md](modules/INDEX.md) |
| Layer specs | [layers/](layers/) |
| AI specs | [ai_specs/INDEX.md](../ai_specs/INDEX.md) |
| Nexus specs | [ai_specs/nexus-specs/](../ai_specs/nexus-specs/) |
| Habitat wiring | [HABITAT_INTEGRATION_SPEC.md](../ai_specs/HABITAT_INTEGRATION_SPEC.md) |
| Evolution Chamber V2 | [ADVANCED_EVOLUTION_CHAMBER_V2.md](ADVANCED_EVOLUTION_CHAMBER_V2.md) |
| Design patterns | [ai_specs/patterns/](../ai_specs/patterns/) |
| Meta tree mind map | [META_TREE_MIND_MAP_V2.md](META_TREE_MIND_MAP_V2.md) |
| V1 expansion plan | [META_TREE_MIND_MAP_M48_M57.md](META_TREE_MIND_MAP_M48_M57.md) |
| Machine context | [.claude/context.json](../.claude/context.json) |
| Patterns (P01-P22) | [.claude/patterns.json](../.claude/patterns.json) |
| Alignment checks | [.claude/ALIGNMENT_VERIFICATION.md](../.claude/ALIGNMENT_VERIFICATION.md) |
| V1 reference | `/home/louranicas/claude-code-workspace/the_maintenance_engine/` |

## V2 Enhancements over V1

| Feature | V1 | V2 |
|---------|----|----|
| Layers | 7 | 8 (+L8 Nexus) |
| Modules | 47 | 48+ |
| NAM target | 92% | 95% |
| Kuramoto r-tracking | No | Yes (N01 Field Bridge) |
| K-regime awareness | No | Yes (N03 Regime Manager) |
| STDP co-activation | No | Yes (N04 +0.05/call) |
| Evolution gate | No | Yes (N05 pre-deployment testing) |
| Morphogenic adaptation | No | Yes (N06 |r_delta| > 0.05 trigger) |

## Rust Conventions (Non-Negotiable)

- `#![forbid(unsafe_code)]` — zero unsafe
- `#![deny(clippy::unwrap_used)]` — zero unwrap
- `parking_lot::RwLock` for interior mutability (L1-L5, L7)
- `std::sync::RwLock` for L6 only (matches convention)
- `Timestamp` newtype — never `chrono::SystemTime`
- `Result<T>` everywhere — never panic
- 50+ tests per module
- `TensorContributor` trait on every module

---

*Maintenance Engine V2 Quick Start | 62,522 LOC | 2,288 Tests*
