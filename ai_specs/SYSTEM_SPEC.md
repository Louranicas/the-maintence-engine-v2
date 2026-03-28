# System Specification

## Version
```
Name: The Maintenance Engine
Version: 1.0.0
Codename: Prometheus
```

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│ L6: Orchestration Layer                                     │
├─────────────────────────────────────────────────────────────┤
│ L5: Consensus Layer (PBFT)                                  │
├─────────────────────────────────────────────────────────────┤
│ L4: Intelligence Layer (NAM/ANAM)                           │
├─────────────────────────────────────────────────────────────┤
│ L3: Integration Layer                                       │
├─────────────────────────────────────────────────────────────┤
│ L2: Processing Layer                                        │
├─────────────────────────────────────────────────────────────┤
│ L1: Foundation Layer                                        │
└─────────────────────────────────────────────────────────────┘
```

## Key Parameters

| Parameter | Value | Description |
|-----------|-------|-------------|
| Total Layers | 6 | Hierarchical architecture |
| Total Modules | 36 | 6 modules per layer |
| PBFT Nodes (n) | 40 | Total validator nodes |
| PBFT Faulty (f) | 13 | Max Byzantine faults |
| PBFT Quorum (q) | 27 | Required consensus |
| Tensor Dimensions | 12 | Neural encoding dims |
| Database Count | 9 | Specialized databases |
| Pipeline Count | 8 | Data processing flows |
| NAM Target | 92% | Accuracy threshold |
| Max Latency | 100ms | P99 response time |

## Port Assignments

| Service | Port | Protocol | TLS |
|---------|------|----------|-----|
| REST API | 8080 | HTTP/1.1 | Optional |
| gRPC | 8081 | HTTP/2 | Required |
| WebSocket | 8082 | WS | Optional |
| Metrics | 9090 | HTTP/1.1 | No |
| Health | 9091 | HTTP/1.1 | No |

## PBFT Configuration

| Parameter | Value | Formula |
|-----------|-------|---------|
| n (nodes) | 40 | Total validators |
| f (faulty) | 13 | floor((n-1)/3) |
| q (quorum) | 27 | 2f + 1 |
| View timeout | 5s | Initial timeout |
| Checkpoint interval | 100 | Blocks per checkpoint |
| Water marks | 200 | Low/High difference |

### Consensus Phases
```
1. PRE-PREPARE  → Leader broadcasts proposal
2. PREPARE      → Nodes acknowledge (q required)
3. COMMIT       → Nodes commit (q required)
4. REPLY        → Response to client
```

## STDP Parameters

| Parameter | Value | Unit |
|-----------|-------|------|
| LTP Rate | 0.1 | - |
| LTD Rate | 0.05 | - |
| Time Window | 100 | ms |
| Decay Rate | 0.001 | per ms |
| Min Weight | 0.0 | - |
| Max Weight | 1.0 | - |

### STDP Formula
```
Δw = {
  +LTP × exp(-Δt/τ)  if Δt > 0 (pre before post)
  -LTD × exp(+Δt/τ)  if Δt < 0 (post before pre)
}
```

## Escalation Tiers

| Tier | Model | Complexity | Use Case |
|------|-------|------------|----------|
| L0 | Haiku 4.5 | 0.0 - 0.7 | Standard queries, validation |
| L1 | Sonnet 4.5 | 0.7 - 0.9 | Complex analysis, multi-step |
| L2 | Opus 4.5 | 0.9 - 1.0 | Critical decisions, architecture |
| L3 | Opus + ULTRATHINK | Security | Security audit, consensus |

### Escalation Triggers
```rust
enum EscalationTrigger {
    ComplexityThreshold(f64),
    SecurityContext,
    ConsensusRequired,
    UserRequest,
    FailedAttempts(u32),
}
```

## Quality Gates

### Clippy Lints
| Lint | Level | Category |
|------|-------|----------|
| unsafe_code | forbid | Safety |
| unwrap_used | deny | Error handling |
| expect_used | deny | Error handling |
| panic | deny | Stability |
| todo | warn | Completeness |
| dbg_macro | deny | Production |
| print_stdout | warn | Logging |
| missing_docs | warn | Documentation |

### Coverage Requirements
| Metric | Minimum | Target |
|--------|---------|--------|
| Line Coverage | 80% | 90% |
| Branch Coverage | 75% | 85% |
| Function Coverage | 90% | 95% |

## Dependencies

### Core Dependencies
| Crate | Version | Purpose |
|-------|---------|---------|
| tokio | 1.x | Async runtime |
| serde | 1.x | Serialization |
| tracing | 0.1.x | Observability |
| thiserror | 1.x | Error handling |
| anyhow | 1.x | Error propagation |

### Database Dependencies
| Crate | Version | Purpose |
|-------|---------|---------|
| sqlx | 0.7.x | SQL async |
| redis | 0.24.x | Caching |
| clickhouse | 0.11.x | Analytics |

### Network Dependencies
| Crate | Version | Purpose |
|-------|---------|---------|
| tonic | 0.11.x | gRPC |
| axum | 0.7.x | REST API |
| tokio-tungstenite | 0.21.x | WebSocket |

## Resource Limits

| Resource | Default | Max |
|----------|---------|-----|
| Memory per node | 4 GB | 16 GB |
| CPU cores | 4 | 16 |
| Connections | 1000 | 10000 |
| Request size | 1 MB | 16 MB |
| Timeout | 30s | 300s |

## Monitoring

| Metric | Type | Labels |
|--------|------|--------|
| request_duration_seconds | histogram | layer, module, method |
| request_total | counter | layer, module, status |
| active_connections | gauge | layer, protocol |
| consensus_rounds | counter | view, outcome |
| nam_accuracy | gauge | model_tier |
