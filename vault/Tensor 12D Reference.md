---
tags: [reference/tensor, progressive-disclosure/L3]
---

# 12D Tensor Encoding Reference

Every module implements `TensorContributor` and writes to specific dimensions.

## Dimensions

| Dim | Name | Range | Contributing Modules |
|-----|------|-------|---------------------|
| D0 | service_id | 0-1 | M09 |
| D1 | port | 0-1 | M09 |
| D2 | tier | 0-1 | M09 |
| D3 | dependency_count | 0-1 | M09 |
| D4 | agent_count | 0-1 | M09 |
| D5 | protocol | 0-1 | M19-M22 |
| D6 | health_score | 0-1 | M10, M11 |
| D7 | uptime | 0-1 | M11 |
| D8 | synergy | 0-1 | N04 (STDP bridge) |
| D9 | latency | 0-1 | M12 |
| D10 | error_rate | 0-1 | M10, M12 |
| D11 | temporal_context | 0-1 | N01 (field bridge) |

## Aggregation

The `TensorRegistry` (M08) collects contributions from all modules and produces a single 12D vector per service, per tick. This tensor is used by:
- N02 Intent Router (cosine similarity for service routing)
- L5 Learning (pattern recognition)
- L7 Observer (emergence detection)

## Cross-System

VMS uses the same 12D encoding (8 Saturn PCA dims + 4 hint dims), enabling direct tensor comparison between ME V2 service state and VMS memory state.

---

See [[HOME]] | Full spec: `../ai_specs/TENSOR_SPEC.md` | Patterns: `../ai_specs/patterns/TENSOR_PATTERNS.md`
