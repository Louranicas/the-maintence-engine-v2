---
tags: [layer/L4, progressive-disclosure/L2, status/pending]
---

# L4: Integration Layer

> **Status:** PENDING | **Target LOC:** ~7,500 | **Target Tests:** 350+

## Purpose

All external communication protocols. REST, gRPC, WebSocket, IPC, event bus, and bridge management. Every outbound call from ME V2 to other ULTRAPLATE services goes through L4.

## Modules

| ID | Module | File | Target LOC | Role |
|----|--------|------|-----------|------|
| M19 | REST Client | `rest.rs` | 600+ | HTTP client for service health/API calls |
| M20 | gRPC Client | `grpc.rs` | 1,200+ | gRPC client for Tool Maker, SYNTHEX |
| M21 | WebSocket Client | `websocket.rs` | 1,000+ | WS client for field evolution streams |
| M22 | IPC Manager | `ipc.rs` | 1,000+ | Inter-process communication |
| M23 | Event Bus | `event_bus.rs` | 700+ | Internal async event routing |
| M24 | Bridge Manager | `bridge.rs` | 800+ | Cross-service bridge orchestration |

## 12D Tensor Contributions

- M19-M22 -> D5 (protocol)

## Template Source

ME v1 `m4_integration/` (5,460 LOC). V2 adds Nexus field capture (C11) and STDP co-activation recording (C12) on every outbound call.

---

Full spec: `../ai_specs/m4-integration-specs/L4_INTEGRATION_SPEC.md` | Module docs: `../ai_docs/modules/M19-M24`
See [[HOME]] | Prev: [[L3 — Core Logic Layer]] | Next: [[L5 — Learning Layer]]
