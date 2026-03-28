# 8 Core Pipelines Specification

**Version:** 1.1.0
**Related:** [MODULE_MATRIX.md](MODULE_MATRIX.md), [SERVICE_SPEC.md](SERVICE_SPEC.md), [STDP_SPEC.md](STDP_SPEC.md), [patterns/CONCURRENCY_PATTERNS.md](patterns/CONCURRENCY_PATTERNS.md)

---

## Overview

The 8 Core Pipelines provide the data processing backbone for The Maintenance Engine, handling health monitoring, learning, consensus, and metrics aggregation across all 12 ULTRAPLATE services.

---

## Pipeline Registry
| ID | Name | Priority | SLO | Trigger | Dependencies |
|----|------|----------|-----|---------|--------------|
| PL-HEALTH-001 | Health Monitoring | 1 | <100ms | 10s interval | None |
| PL-LOG-001 | Log Processing | 2 | <50ms | Continuous | None |
| PL-REMEDIATE-001 | Auto-Remediation | 1 | <500ms | On error | PL-HEALTH-001 |
| PL-HEBBIAN-001 | Neural Learning | 2 | <100ms | On activation | PL-TENSOR-001 |
| PL-CONSENSUS-001 | PBFT Consensus | 1 | <5s | On proposal | PL-REMEDIATE-001 |
| PL-TENSOR-001 | Tensor Encoding | 3 | <10ms | On state change | None |
| PL-DISCOVERY-001 | Service Discovery | 2 | <1s | On registration | None |
| PL-METRICS-001 | Metrics Aggregation | 3 | <200ms | 15s interval | PL-HEALTH-001 |

## Pipeline Stages

### PL-HEALTH-001: Health Monitoring
```
[Health Probe] → [Status Check] → [Aggregate] → [Report] → [Alert?]
```
- Stage 1: Send health probes to all 12 services
- Stage 2: Validate responses
- Stage 3: Aggregate health scores
- Stage 4: Update service_tracking.db
- Stage 5: Trigger alerts if health < threshold

### PL-LOG-001: Log Processing
```
[Ingest] → [Parse] → [Correlate] → [Store] → [Index]
```

### PL-REMEDIATE-001: Auto-Remediation
```
[Error] → [Classify] → [Route] → [Select Action] → [Execute] → [Verify] → [Learn]
```
- Uses 12D tensor for error classification
- Routes through Hebbian pathways
- Applies escalation tier logic

### PL-HEBBIAN-001: Neural Learning
```
[Activation] → [STDP Window] → [Weight Update] → [Decay] → [Prune]
```
- tau_plus/tau_minus: 20ms
- a_plus/a_minus: 0.01/0.012
- Decay rate: 0.001

### PL-CONSENSUS-001: PBFT Consensus
```
[Proposal] → [Pre-Prepare] → [Prepare] → [Commit] → [Execute] → [Reply]
```
- n=40, f=13, q=27
- View change on timeout

### PL-TENSOR-001: Tensor Encoding
```
[State] → [Encode 12D] → [Normalize] → [Store] → [Index]
```

### PL-DISCOVERY-001: Service Discovery
```
[Register] → [Validate] → [Announce] → [Health Check] → [Index]
```

### PL-METRICS-001: Metrics Aggregation
```
[Collect] → [Aggregate] → [Calculate SLOs] → [Store] → [Export]
```

## Pipeline Dependencies
| Pipeline | Depends On |
|----------|-----------|
| PL-REMEDIATE-001 | PL-HEALTH-001, PL-HEBBIAN-001, PL-TENSOR-001 |
| PL-HEBBIAN-001 | PL-TENSOR-001 |
| PL-CONSENSUS-001 | PL-REMEDIATE-001 (for L3 actions) |

## Data Flow Diagram
```
Services → PL-HEALTH-001 → PL-TENSOR-001 → tensor_memory.db
                ↓                              ↓
            Errors → PL-REMEDIATE-001 ← PL-HEBBIAN-001
                           ↓
                    [L0/L1/L2/L3]
                           ↓
                    PL-CONSENSUS-001 (if L3)
```

## Configuration
```toml
[pipeline.PL-HEALTH-001]
interval_ms = 10000
timeout_ms = 5000
retry_count = 3

[pipeline.PL-REMEDIATE-001]
confidence_threshold = 0.7
max_concurrent = 5
rollback_on_failure = true

[pipeline.PL-HEBBIAN-001]
batch_size = 100
flush_interval_ms = 1000
decay_interval_hours = 1

[pipeline.PL-TENSOR-001]
dimensions = 12
normalize = true
validate_bounds = true

[pipeline.PL-CONSENSUS-001]
n_agents = 40
f_tolerance = 13
quorum = 27
timeout_ms = 5000

[pipeline.PL-METRICS-001]
interval_ms = 15000
retention_days = 30
export_prometheus = true
```

---

## Pipeline-Module Mapping

| Pipeline | Primary Module | Supporting Modules |
|----------|---------------|-------------------|
| PL-HEALTH-001 | M08 (Health) | M01-M06 (Services) |
| PL-LOG-001 | M13 (Logging) | M14 (Analysis) |
| PL-REMEDIATE-001 | M15 (Remediation) | M08, M16 |
| PL-HEBBIAN-001 | M17 (Learning) | M18 (Pathways) |
| PL-CONSENSUS-001 | M19 (PBFT) | M20-M22 (Agents) |
| PL-TENSOR-001 | M23 (Tensor) | M24 (Encoding) |
| PL-DISCOVERY-001 | M25 (Discovery) | M26 (Registry) |
| PL-METRICS-001 | M27 (Metrics) | M28 (Prometheus) |

---

## Error Handling

Each pipeline implements circuit breaker patterns (see [patterns/ERROR_PATTERNS.md](patterns/ERROR_PATTERNS.md)):

| State | Behavior |
|-------|----------|
| Closed | Normal operation |
| Open | Fail fast, skip processing |
| HalfOpen | Allow test requests |

---

## Cross-Spec Dependencies

| Dependency | Source Spec | Integration Point |
|------------|-------------|-------------------|
| Service health | SERVICE_SPEC | PL-HEALTH-001 targets |
| STDP learning | STDP_SPEC | PL-HEBBIAN-001 parameters |
| PBFT consensus | PBFT_SPEC | PL-CONSENSUS-001 quorum |
| 12D Tensor | TENSOR_SPEC | PL-TENSOR-001 dimensions |
| Escalation | ESCALATION_SPEC | PL-REMEDIATE-001 tiers |

---

## Version History

| Version | Date | Changes |
|---------|------|---------|
| 1.1.0 | 2026-01-28 | Added dependencies, module mapping, cross-references |
| 1.0.0 | 2026-01-28 | Initial specification |

---

*Generated: 2026-01-28 | The Maintenance Engine v1.0.0*
