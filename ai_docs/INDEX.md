# Maintenance Engine V2 - AI Documentation Index

> Documentation hub for Claude Code agents working on ME V2

---

## Quick Navigation

| Category | Path | Contents |
|----------|------|----------|
| Module docs | `modules/` | M01-M42 individual module documentation |
| Layer docs | `layers/` | L01-L07 layer overviews |
| Schematics | `schematics/` | 6 Mermaid architectural diagrams |
| Integration | `integration/` | Cross-layer integration guides |
| Security | `security/` | Security architecture documentation |
| Diagnostics | `diagnostics/` | Troubleshooting and debugging guides |

---

## Architectural Schematics

| Diagram | File | Description |
|---------|------|-------------|
| Layer Architecture | `schematics/layer_architecture.mmd` | 8-layer module hierarchy |
| Service Mesh | `schematics/service_mesh.mmd` | ULTRAPLATE connectivity |
| Nexus Integration | `schematics/nexus_integration.mmd` | L8 ↔ OVM topology |
| Data Flow | `schematics/data_flow.mmd` | Pipeline → Decision → Outcome |
| Tensor Contribution | `schematics/tensor_contribution.mmd` | 12D dimension ownership |
| Database Topology | `schematics/database_topology.mmd` | 12 database relationships |

Render with: `npx mmdc -i file.mmd -o file.png -t dark`

---

## Context Development (Pre-Coding Reference)

> These documents provide the complete pattern language for implementing L3-L8.
> Read GOLD_STANDARD_PATTERNS first, then ANTI_PATTERNS, then the exemplars.

| Document | Path | Description |
|----------|------|-------------|
| **Gold Standard Patterns** | `GOLD_STANDARD_PATTERNS.md` | 14 mandatory patterns (P1-P14) extracted from M1+M2 gold code |
| **Anti-Patterns** | `ANTI_PATTERNS.md` | 15 things to NEVER do (A1-A15) with severity + fixes |
| **Rust Exemplars** | `RUST_EXEMPLARS.md` | 12 copy-adaptable code blocks (E1-E12) from ME v1 |
| **Nexus Exemplars** | `NEXUS_EXEMPLARS.md` | VMS/SVF reference implementations for L8 N01-N06 |
| **Internet Gold Standards** | `INTERNET_GOLD_STANDARDS.md` | 60+ patterns from 100+ web sources (Axum, Tokio, RwLock, FSM, PBFT, STDP, Kuramoto) |

---

## Key References

| Document | Path | Description |
|----------|------|-------------|
| CLAUDE.md | `../CLAUDE.md` | Bootstrap context |
| CLAUDE.local.md | `../CLAUDE.local.md` | Local development context |
| Master Plan | `../SCAFFOLDING_MASTER_PLAN.md` | Scaffolding blueprint |
| AI Specs | `../ai_specs/INDEX.md` | Technical specifications |
| ME v1 | `/home/louranicas/claude-code-workspace/the_maintenance_engine/` | Primary exemplar |

---

## Module Documentation (M01-M42)

### L1: Foundation
- `modules/M01.md` — Error Taxonomy
- `modules/M02.md` — Configuration Manager
- `modules/M03.md` — Structured Logging
- `modules/M04.md` — Metrics Collector
- `modules/M05.md` — State Persistence
- `modules/M06.md` — Resource Manager

### L2: Services
- `modules/M09.md` — Service Registry
- `modules/M10.md` — Health Monitor
- `modules/M11.md` — Lifecycle Manager
- `modules/M12.md` — Circuit Breaker + Load Balancer

### L3: Core Logic
- `modules/M13.md` — Pipeline Manager
- `modules/M14.md` — Remediation Engine
- `modules/M15.md` — Confidence Calculator
- `modules/M16.md` — Action Executor
- `modules/M17.md` — Outcome Recorder
- `modules/M18.md` — Feedback Loop

### L4: Integration
- `modules/M19.md` — REST Client
- `modules/M20.md` — gRPC Client
- `modules/M21.md` — WebSocket Client
- `modules/M22.md` — IPC Manager
- `modules/M23.md` — Event Bus
- `modules/M24.md` — Bridge Manager

### L5: Learning
- `modules/M25.md` — Hebbian Manager
- `modules/M26.md` — STDP Processor
- `modules/M27.md` — Pattern Recognizer
- `modules/M28.md` — Pathway Pruner
- `modules/M29.md` — Memory Consolidator
- `modules/M30.md` — Anti-Pattern Detector

### L6: Consensus
- `modules/M31.md` — PBFT Manager
- `modules/M32.md` — Agent Coordinator
- `modules/M33.md` — Vote Collector
- `modules/M34.md` — View Change Handler
- `modules/M35.md` — Dissent Tracker
- `modules/M36.md` — Quorum Calculator

### L7: Observer
- `modules/M37.md` — Log Correlator
- `modules/M38.md` — Emergence Detector
- `modules/M39.md` — Evolution Chamber

### V3: Homeostasis
- `modules/M40.md` — PID Thermal Controller
- `modules/M41.md` — STDP Decay Auditor
- `modules/M42.md` — Diagnostics Engine

---

## Layer Documentation (L01-L07)

- `layers/L01.md` — Foundation Layer
- `layers/L02.md` — Services Layer
- `layers/L03.md` — Core Logic Layer
- `layers/L04.md` — Integration Layer
- `layers/L05.md` — Learning Layer
- `layers/L06.md` — Consensus Layer
- `layers/L07.md` — Observer Layer

---

## V2 New Documentation (To Be Created)

- `layers/L08.md` — Nexus Layer (NEW)
- `modules/N01.md` — Field Bridge
- `modules/N02.md` — Intent Router
- `modules/N03.md` — Regime Manager
- `modules/N04.md` — STDP Bridge
- `modules/N05.md` — Evolution Gate
- `modules/N06.md` — Morphogenic Adapter
