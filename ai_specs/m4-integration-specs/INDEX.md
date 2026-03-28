# L4 Integration — Spec Sheet Index

> **Layer:** L4 Integration | **Modules:** M19-M24c (8 modules, 9 files)
> **LOC:** ~7,500 (target) | **Tests:** 350+ (target) | **Quality Score:** PENDING
> **Status:** SPECIFIED — awaiting implementation | **Verified:** 2026-03-06

---

## Document Map

| Document | Description | Tokens |
|----------|-------------|--------|
| [L4_INTEGRATION_SPEC.md](L4_INTEGRATION_SPEC.md) | Full layer specification: protocol clients, event bus, bridges, tool registrar, all 8 modules | ~3K |

---

## Reading Protocol

```
QUICK START:    Read L4_INTEGRATION_SPEC.md (architecture + protocol wiring)
WRITING CODE:   Read the M19-M24c section relevant to the module you're implementing
CROSS-LAYER:    L4 connects ME V2 to 13 ULTRAPLATE services; consumes L1-L3, feeds L5-L7
CONSUMING L4:   EventBus (M23) + Bridge (M24) + Tool Registrar (M24c) — the integration triad
```

---

## Module Table

| # | Module | ID | File | Target LOC | Target Tests | Status |
|---|--------|----|------|-----------|-------------|--------|
| 1 | REST Client | M19 | `rest.rs` | ~600 | 50+ | PENDING |
| 2 | gRPC Client | M20 | `grpc.rs` | ~1,200 | 50+ | PENDING |
| 3 | WebSocket Client | M21 | `websocket.rs` | ~1,000 | 50+ | PENDING |
| 4 | IPC Manager | M22 | `ipc.rs` | ~950 | 50+ | PENDING |
| 5 | Event Bus | M23 | `event_bus.rs` | ~700 | 50+ | PENDING |
| 6 | Bridge Manager | M24 | `bridge.rs` | ~750 | 50+ | PENDING |
| 7 | Peer Bridge | M24b | `peer_bridge.rs` | ~600 | 50+ | PENDING |
| 8 | Tool Registrar | M24c | `tool_registrar.rs` | ~1,200 | 50+ | PENDING |
| 9 | Layer Coordinator | — | `mod.rs` | ~500 | 20+ | PENDING |
| | **Subtotal** | | | **~7,500** | **~420** | |

---

## Quick Reference — 7 Traits

| # | Trait | File | Bounds | Methods | Defaults |
|---|-------|------|--------|---------|----------|
| 1 | `RestClient` | rest.rs | Send+Sync | 4 | 0 |
| 2 | `GrpcClient` | grpc.rs | Send+Sync | 3 | 0 |
| 3 | `WebSocketClient` | websocket.rs | Send+Sync | 4 | 0 |
| 4 | `IpcManager` | ipc.rs | Send+Sync | 3 | 0 |
| 5 | `EventBusOps` | event_bus.rs | Send+Sync | 4 | 0 |
| 6 | `BridgeManager` | bridge.rs | Send+Sync | 4 | 0 |
| 7 | `TensorContributor` | (all modules) | Send+Sync+Debug | 3 | 0 |

---

## Quick Reference — Tensor Dimensions

| Dim | Name | L4 Contributors | Notes |
|-----|------|-----------------|-------|
| D0 | ServiceId | — | Inherited from L2 |
| D1 | Port | — | — |
| D2 | Tier | — | — |
| D3 | DependencyCount | — | — |
| D4 | AgentCount | — | — |
| D5 | Protocol | M19 (REST=0), M20 (gRPC=0.33), M21 (WS=0.67), M22 (IPC=1.0) | Protocol enum encoding |
| D6 | HealthScore | — | — |
| D7 | Uptime | — | — |
| D8 | Synergy | M24 (bridge synergy score) | Cross-service synergy tracking |
| D9 | Latency | M19 (REST latency), M20 (gRPC latency) | Per-protocol latency measurement |
| D10 | ErrorRate | — | — |
| D11 | TemporalContext | — | — |

---

## Peer Bridge Polling Tiers

| Tier | Services | Interval |
|------|----------|----------|
| 1 | SYNTHEX, SAN-K7 | 5s |
| 2 | NAIS, CodeSynthor, DevOps Engine | 10s |
| 3 | Tool Library, Context Manager | 30s |
| 4+ | Prometheus Swarm, Architect Agent | 60s |

---

## Tool Registrar — 15 Tools

| Category | Tools |
|----------|-------|
| Health | health_check, health_batch, service_status |
| Remediation | remediation_execute, remediation_rollback |
| Learning | hebbian_query, stdp_status |
| Consensus | consensus_propose, consensus_status |
| Observer | observer_report, emergence_detect |
| Tensor | tensor_encode, tensor_query |
| Evolution | evolution_evaluate, evolution_status |

---

## Design Constraints

| ID | Constraint | Enforcement |
|----|-----------|-------------|
| C1 | Imports from L1, L2, L3 only | Compile-time module DAG |
| C2 | All trait methods `&self` | Code review |
| C3 | `TensorContributor` on M19-M24 | Compile-time |
| C4 | Zero unsafe/unwrap/expect | `#![forbid(unsafe_code)]` + clippy deny |
| C11 | Pre/post r-capture on all external calls | Nexus field capture pattern |
| C12 | STDP increment (+0.05) on each service interaction | N04 integration |

---

## Cross-References

- **Upstream:** [M1 Foundation](../m1-foundation-specs/) | [M2 Services](../m2-services-specs/) | [M3 Core Logic](../m3-core-logic-specs/)
- **Downstream:** L5 Learning (M25-M30), L6 Consensus (M31-M36), L7 Observer (M37-M39)
- **System:** [SYSTEM_SPEC](../SYSTEM_SPEC.md), [LAYER_SPEC](../LAYER_SPEC.md), [MODULE_MATRIX](../MODULE_MATRIX.md)
- **Patterns:** [PIPELINE](../patterns/PIPELINE.md), [CIRCUIT_BREAKER](../patterns/CIRCUIT_BREAKER.md), [EVENT_SOURCING](../patterns/EVENT_SOURCING.md)
- **Tensor:** [TENSOR_SPEC](../TENSOR_SPEC.md)
- **Service:** [SERVICE_SPEC](../SERVICE_SPEC.md) (13 ULTRAPLATE service definitions)
- **API:** [API_SPEC](../API_SPEC.md) (REST endpoint schemas)
- **Tests:** `tests/l4_integration_layer.rs`

---

*L4 Integration Spec Sheet Index v1.0 | 2026-03-06*
