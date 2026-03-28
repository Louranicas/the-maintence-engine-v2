# Escalation Tier Specification

**Version:** 1.1.0
**Related:** [PBFT_SPEC.md](PBFT_SPEC.md), [NAM_SPEC.md](NAM_SPEC.md), [SERVICE_SPEC.md](SERVICE_SPEC.md), [patterns/ERROR_PATTERNS.md](patterns/ERROR_PATTERNS.md)

---

## Overview

The Escalation System provides a tiered approach to action authorization, ensuring appropriate human oversight for high-risk operations while allowing autonomous execution of routine maintenance tasks.

---

## Escalation Tiers

### Tier Definitions
| Tier | Name | Condition | Timeout | Action |
|------|------|-----------|---------|--------|
| L0 | Auto-Execute | confidence >= 0.9, severity <= MEDIUM | 0 | Execute immediately |
| L1 | Notify Human | confidence >= 0.7, severity <= HIGH | 5min | Notify, then execute |
| L2 | Require Approval | confidence < 0.7 OR severity = HIGH | 30min | Wait for human |
| L3 | PBFT Consensus | Critical actions | Quorum | Multi-agent vote (27/40) |

### Confidence Thresholds
| Confidence | Tier Assignment |
|------------|-----------------|
| 0.9 - 1.0 | L0 (if severity ≤ MEDIUM) |
| 0.7 - 0.9 | L1 (if severity ≤ HIGH) |
| 0.5 - 0.7 | L2 |
| < 0.5 | L3 (always) |

### Severity Levels
| Level | Code | Auto-Execute | Description |
|-------|------|--------------|-------------|
| LOW | 1 | L0-L1 | Minor impact |
| MEDIUM | 2 | L0-L1 | Moderate impact |
| HIGH | 3 | L2 | Significant impact |
| CRITICAL | 4 | L3 only | System-wide impact |

### Escalation Flow
```
Error Detected
     ↓
Classify (M01)
     ↓
Calculate Confidence
     ↓
Determine Tier
     ↓
L0 → Execute → Feedback
L1 → Notify → [Timeout] → Execute → Feedback
L2 → Request Approval → [Approve/Reject] → Execute/Cancel
L3 → PBFT Vote → [Quorum 27/40] → Execute/Cancel
```

### Tier Actions by Type
| Action Type | L0 | L1 | L2 | L3 |
|-------------|----|----|----|----|
| Restart Service | ✓ | ✓ | | |
| Scale Replicas | ✓ | ✓ | | |
| Failover | | ✓ | ✓ | |
| Config Change | | | ✓ | |
| Kill Process | | | | ✓ |
| Data Migration | | | | ✓ |

### Rollback Rules
| Condition | Rollback |
|-----------|----------|
| Health < 0.5 after action | Automatic |
| Error rate > 2x baseline | Automatic |
| Human override | Manual |
| PBFT revote | Consensus |

---

## Integration with 12D Tensor

The escalation system uses tensor dimensions for decision-making:

| Tensor Dimension | Escalation Use |
|------------------|----------------|
| D6 (health) | Health threshold for rollback |
| D9 (latency) | Response time monitoring |
| D10 (error_rate) | Error rate tracking for rollback |
| D11 (temporal) | Time-based escalation patterns |

---

## PBFT Integration (L3)

L3 escalations require PBFT consensus from the CVA-NAM agent swarm:

| Parameter | Value |
|-----------|-------|
| Total Agents (n) | 40 |
| Byzantine Tolerance (f) | 13 |
| Quorum Requirement (q) | 27 |
| Timeout | View change on timeout |

See [PBFT_SPEC.md](PBFT_SPEC.md) for full consensus specification.

---

## Human Agent (@0.A)

The human is registered as agent `@0.A` per NAM-05:

| Capability | L0 | L1 | L2 | L3 |
|------------|----|----|----|----|
| View status | Yes | Yes | Yes | Yes |
| Receive notifications | No | Yes | Yes | Yes |
| Approve/Reject | No | No | Yes | Yes |
| Override | No | No | Yes | Yes |
| Consensus vote | No | No | No | Yes |

---

## Cross-Spec Dependencies

| Dependency | Source Spec | Integration Point |
|------------|-------------|-------------------|
| PBFT Parameters | PBFT_SPEC | L3 consensus |
| Human Agent | NAM_SPEC R5 | @0.A registration |
| Tensor Health | TENSOR_SPEC D6 | Rollback threshold |
| Error Patterns | patterns/ERROR_PATTERNS.md | Error classification |

---

## Version History

| Version | Date | Changes |
|---------|------|---------|
| 1.1.0 | 2026-01-28 | Added tensor integration, cross-references |
| 1.0.0 | 2026-01-28 | Initial specification |

---

*Generated: 2026-01-28 | The Maintenance Engine v1.0.0*
