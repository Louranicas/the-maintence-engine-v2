---
tags: [nav/architecture, progressive-disclosure/L1]
---

# Architecture Overview

> 8 Layers | 48+ Modules | 12D Tensor | PBFT (n=40, f=13, q=27) | Kuramoto K-Regime

## Layer Stack (bottom to top)

```
L8: NEXUS (NEW)     — Field bridge, intent routing, K-regime, STDP, evolution, morphogenic
L7: OBSERVER         — Observer bus, fitness, log correlator, emergence, evolution, thermal
L6: CONSENSUS        — PBFT, agents, voting, view change, dissent, quorum
L5: LEARNING         — Hebbian, STDP, pattern, pruner, consolidator, antipattern
L4: INTEGRATION      — REST, gRPC, WebSocket, IPC, event bus, bridge, peer, tools
L3: CORE LOGIC       — Pipeline, remediation, confidence, action, outcome, feedback
L2: SERVICES         — Registry, health, lifecycle, resilience        [CLONED]
L1: FOUNDATION       — Error, config, logging, metrics, state, resources  [CLONED]
V3: HOMEOSTASIS      — Thermal, decay auditor, diagnostics (HRS-001)
```

## Dependency DAG

```
L8 depends on: L5, L7, VMS (external)
L7 depends on: L3, L5, L6
L6 depends on: L1
L5 depends on: L1
L4 depends on: L1, L2
L3 depends on: L1, L2
L2 depends on: L1
L1 depends on: nothing
V3 depends on: L1
```

**Rule C1:** No upward imports. L3 cannot import from L4+. Enforced at compile time.

## V2 Enhancements Over V1

- **L8 Nexus** — 6 new modules bridging to VMS/Nexus Controller (Kuramoto, STDP, evolution)
- **K-Regime Awareness** — Swarm (K<1), Fleet (1<=K<2), Armada (K>=2)
- **Morphogenic Triggers** — Adaptation on |r_delta| > 0.05
- **384D Semantic Drift** — Saturn Light embedding detection
- **Cross-Session Learning** — STDP persistence across Claude Code sessions

## Key Files

| File | Purpose |
|------|---------|
| `src/lib.rs` | Crate root, prelude, module declarations |
| `src/main.rs` | Axum HTTP server, 30+ routes |
| `src/engine.rs` | MaintenanceEngineV2 orchestrator |
| `src/database.rs` | DatabaseManager (12 SQLite) |

---

See [[HOME]] | Per-layer details: [[L1 — Foundation Layer]] through [[L8 — Nexus Layer]]
