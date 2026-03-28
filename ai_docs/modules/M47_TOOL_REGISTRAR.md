# M47: Tool Registrar — Module Specification

**Module ID:** M47
**Layer:** L4 (Integration)
**File:** `src/m4_integration/tool_registrar.rs`
**Status:** DEPLOYED (previously unnumbered infrastructure)
**LOC:** 1,215
**Tests:** 53

---

## Purpose

Manages registration and deregistration of the 15 Maintenance Engine tools with the Tool Library service (port 8105). Provides typed tool definitions across 7 categories with POST endpoint registration at startup.

## Key Types

| Type | Kind | Purpose |
|------|------|---------|
| `ToolRegistrar` | Struct | Tool registration manager |
| `ToolDefinition` | Struct | Tool spec (name, description, category, parameters) |
| `ToolCategory` | Enum | Health/Remediation/Learning/Consensus/Observer/Tensor/Integration |
| `RegistrationResult` | Struct | Per-tool registration outcome |

## Tool Categories (15 tools across 7 files)

| File | Tools | Category |
|------|-------|----------|
| `tools/health_tools.rs` | service_health_check, system_health_report | Health |
| `tools/remediation_tools.rs` | auto_remediate, escalate_issue | Remediation |
| `tools/learning_tools.rs` | pathway_strength, stdp_update | Learning |
| `tools/consensus_tools.rs` | propose_action, check_quorum | Consensus |
| `tools/observer_tools.rs` | fitness_report, emergence_scan | Observer |
| `tools/tensor_tools.rs` | encode_tensor, tensor_distance | Tensor |
| `tools/mod.rs` | tool_invoke (dispatcher) | Integration |

## Dependencies

- `crate::m1_foundation::{Error, Result}` (M01)
- `AtomicBool` for registration state tracking

## Dependents

- `main.rs` spawn_tool_registration() — registers all 15 tools at startup
- Tool Library service (port 8105) — receives POST registrations

## Related Documentation

- [L4 Integration](../layers/L04_INTEGRATION.md)
