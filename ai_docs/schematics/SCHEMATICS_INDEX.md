# Maintenance Engine V2 - Architectural Schematics

> Mermaid source files for rendering in Obsidian, mermaid.live, or `npx mmdc`

## Diagrams

| File | Description | Render |
|------|-------------|--------|
| `layer_architecture.mmd` | 8-layer module hierarchy with all 48+ modules | `npx mmdc -i layer_architecture.mmd -o layer_architecture.png` |
| `service_mesh.mmd` | ULTRAPLATE service connectivity matrix (13 services) | `npx mmdc -i service_mesh.mmd -o service_mesh.png` |
| `nexus_integration.mmd` | L8 Nexus ↔ OVM/Kuramoto field integration topology | `npx mmdc -i nexus_integration.mmd -o nexus_integration.png` |
| `data_flow.mmd` | Pipeline → Decision → Outcome data flow with escalation | `npx mmdc -i data_flow.mmd -o data_flow.png` |
| `tensor_contribution.mmd` | 12D tensor dimension ownership across modules | `npx mmdc -i tensor_contribution.mmd -o tensor_contribution.png` |
| `database_topology.mmd` | 12 database relationships and data flow | `npx mmdc -i database_topology.mmd -o database_topology.png` |

## Diagnostic Reference

| File | Description |
|------|-------------|
| `ME_V2_DIAGNOSTIC_SCHEMATICS.md` | God-tier diagnostic and tuning reference — 16 sections covering fitness trees, metabolic health, observer pipeline, EventBus channels, RALPH evolution, lock ordering, wiring gaps, tensor dimensions, service mesh, cross-service data flow, background tasks, databases, escalation, STDP learning, thermal subsystem, and tuning knobs |

## Rendering

```bash
# Render all diagrams
for f in *.mmd; do npx mmdc -i "$f" -o "${f%.mmd}.png" -t dark; done

# Or view in Obsidian (paste content into ```mermaid blocks)
# Or paste into https://mermaid.live
```
