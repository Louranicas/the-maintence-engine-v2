# M2 SERVICES LAYER — ARCHITECTURAL SCHEMATICS

> Visual architecture for The Maintenance Engine v1.0.0 — Layer 2 (Services)
> 7 Mermaid diagrams | Generated: 2026-03-01

---

## Table of Contents

1. [L2 Layer Architecture](#1-l2-layer-architecture)
2. [M11 Service Lifecycle FSM](#2-m11-service-lifecycle-fsm)
3. [M12 Circuit Breaker FSM](#3-m12-circuit-breaker-fsm)
4. [M10 Health Monitor FSM](#4-m10-health-monitor-fsm)
5. [Tensor Contribution Map](#5-tensor-contribution-map)
6. [Signal Flow Topology](#6-signal-flow-topology)
7. [Data Flow & Integration](#7-data-flow--integration)

---

## 1. L2 Layer Architecture

Complete component diagram showing all modules, traits, structs, and L1 dependencies.

```mermaid
graph TB
    subgraph L2["Layer 2: Services (7,196 LOC | 320 tests)"]
        direction TB

        subgraph MOD["mod.rs — Coordinator (694 LOC)"]
            SS["ServiceStatus<br/>Stopped|Starting|Running|Stopping|Failed"]
            HS["HealthStatus<br/>Healthy|Degraded|Unhealthy|Unknown"]
            ST["ServiceTier<br/>Tier1 w=1.5 .. Tier5 w=1.0"]
            CS["CircuitState<br/>Closed|Open|HalfOpen"]
            SVS["ServiceState<br/>id, name, status, health, tier,<br/>port, pid, scores, tensor"]
            RC["RestartConfig<br/>max_restarts: 5<br/>initial_backoff: 1s<br/>max_backoff: 30s"]
        end

        subgraph M09["M09: ServiceRegistry (1,285 LOC | 53 tests)"]
            SR["ServiceRegistry<br/>RwLock RegistryState"]
            SD_TRAIT["trait ServiceDiscovery<br/>14 methods"]
            SDEF["ServiceDefinition<br/>id, name, version, tier,<br/>host, port, protocol"]
            ULTRA["register_ultraplate_services<br/>12 services bootstrap"]
        end

        subgraph M10["M10: HealthMonitor (1,130 LOC | 49 tests)"]
            HM["HealthMonitor<br/>RwLock MonitorState"]
            HM_TRAIT["trait HealthMonitoring<br/>11 methods"]
            HP["HealthProbe<br/>endpoint, interval,<br/>thresholds"]
            HCR["HealthCheckResult<br/>status, response_time,<br/>message"]
        end

        subgraph M11["M11: LifecycleManager (1,898 LOC | 75 tests)"]
            LM["LifecycleManager<br/>RwLock HashMap"]
            LM_TRAIT["trait LifecycleOps<br/>18 methods"]
            LE["LifecycleEntry<br/>state, restart_count,<br/>backoff, history"]
            LA["LifecycleAction<br/>Start|Stop|Restart|HealthCheck"]
        end

        subgraph M12["M12: ResilienceManager (2,189 LOC | 82 tests)"]
            RM["ResilienceManager<br/>Facade"]
            CBR["CircuitBreakerRegistry<br/>RwLock HashMap"]
            LB["LoadBalancer<br/>RwLock HashMap"]
            CB_TRAIT["trait CircuitBreakerOps<br/>12 methods"]
            LB_TRAIT["trait LoadBalancing<br/>10 methods"]
            EP["Endpoint<br/>host, port, weight,<br/>connections, healthy"]
            LBA["LoadBalanceAlgorithm<br/>RoundRobin|Weighted|<br/>LeastConn|Random"]
        end
    end

    subgraph L1["Layer 1: Foundation"]
        ERR["Error / Result"]
        TS["Timestamp / Duration"]
        SB["Arc SignalBus"]
        TC["trait TensorContributor"]
        MR["Arc MetricsRegistry"]
        T12["Tensor12D"]
    end

    SR --> SD_TRAIT
    HM --> HM_TRAIT
    LM --> LM_TRAIT
    CBR --> CB_TRAIT
    LB --> LB_TRAIT
    RM --> CBR
    RM --> LB

    SR -.-> ERR
    SR -.-> SB
    SR -.-> TC
    HM -.-> ERR
    HM -.-> SB
    HM -.-> TC
    LM -.-> ERR
    LM -.-> SB
    LM -.-> TC
    RM -.-> TC
    RM -.-> MR

    SR -.-> TS
    HM -.-> TS
    LM -.-> TS
    CBR -.-> TS

    style L2 fill:#e3f2fd,stroke:#1565c0,stroke-width:2px
    style L1 fill:#f3e5f5,stroke:#7b1fa2,stroke-width:2px
    style MOD fill:#fff3e0,stroke:#e65100
    style M09 fill:#e8f5e9,stroke:#2e7d32
    style M10 fill:#e8f5e9,stroke:#2e7d32
    style M11 fill:#e8f5e9,stroke:#2e7d32
    style M12 fill:#e8f5e9,stroke:#2e7d32
```

---

## 2. M11 Service Lifecycle FSM

State machine governing service operational states with restart backoff.

```mermaid
stateDiagram-v2
    [*] --> Stopped : register()

    Stopped --> Starting : start_service()
    Starting --> Running : mark_running()
    Starting --> Failed : mark_failed()
    Running --> Stopping : stop_service()
    Running --> Failed : mark_failed()
    Running --> Starting : restart_service()\n+restart_count\n+backoff*2
    Stopping --> Stopped : mark_stopped()\nreset restart_count
    Failed --> Starting : start_service()\nor restart_service()

    state Stopped {
        [*] : health_score = 0.0
    }
    state Starting {
        [*] : health_score = 0.5
    }
    state Running {
        [*] : health_score = 1.0
    }
    state Stopping {
        [*] : health_score = 0.5
    }
    state Failed {
        [*] : health_score = 0.0
    }
```

### Backoff Sequence (defaults)

| Restart # | Backoff | Cumulative |
|-----------|---------|------------|
| 1 | 1s | 1s |
| 2 | 2s | 3s |
| 3 | 4s | 7s |
| 4 | 8s | 15s |
| 5 | 16s | 31s |
| 6+ | REJECTED | max_restarts exceeded |

---

## 3. M12 Circuit Breaker FSM

Three-state circuit breaker with timeout-based recovery probing.

```mermaid
stateDiagram-v2
    [*] --> Closed : register_breaker()

    Closed --> Open : record_failure()\nfailure_count >= threshold
    Closed --> Closed : record_failure()\nfailure_count < threshold
    Closed --> Closed : record_success()

    Open --> HalfOpen : allow_request()\nopen_timeout elapsed
    Open --> Open : allow_request()\ntimeout not elapsed\n(request DENIED)

    HalfOpen --> Closed : record_success()\nconsecutive >= threshold
    HalfOpen --> HalfOpen : record_success()\nconsecutive < threshold
    HalfOpen --> Open : record_failure()\n(any failure = immediate trip)

    Closed --> Closed : reset()
    Open --> Closed : reset()
    HalfOpen --> Closed : reset()

    note right of Closed
        Normal operation
        All requests allowed
        Tracking failures
        Signal score = 1.0
    end note

    note right of Open
        Rejecting ALL requests
        Waiting for timeout (30s)
        Uses Instant (monotonic)
        Signal score = 0.0
    end note

    note right of HalfOpen
        Probing with 1 request
        Success → Close
        Failure → Open
        Signal score = 0.5
    end note
```

### Default Thresholds

```
Closed→Open:     5 failures
HalfOpen→Closed: 3 consecutive successes
Open→HalfOpen:   30s timeout (monotonic Instant)
```

---

## 4. M10 Health Monitor FSM

Threshold-driven health status with Degraded intermediate state for flap prevention.

```mermaid
stateDiagram-v2
    [*] --> Unknown : register_probe()

    Unknown --> Healthy : consecutive_successes >= healthy_threshold
    Unknown --> Unhealthy : consecutive_failures >= unhealthy_threshold
    Unknown --> Unknown : below thresholds

    Healthy --> Degraded : single failure (fast detection)

    Degraded --> Unhealthy : consecutive_failures >= unhealthy_threshold
    Degraded --> Healthy : consecutive_successes >= healthy_threshold

    Unhealthy --> Healthy : consecutive_successes >= healthy_threshold

    note right of Unknown
        Initial state
        score = 0.0
        Counters at 0
    end note

    note right of Healthy
        score = 1.0
        Single failure drops to Degraded
        (fast anomaly detection)
    end note

    note right of Degraded
        score = 0.5
        Intermediate state
        Prevents false Unhealthy
        Multiple failures confirm
    end note

    note right of Unhealthy
        score = 0.0
        Confirmed problem
        Needs sustained recovery
        (healthy_threshold successes)
    end note
```

### Counter Reset Behavior

```
On success: consecutive_failures = 0, consecutive_successes += 1
On failure: consecutive_successes = 0, consecutive_failures += 1
```

This hysteresis mechanism ensures a single good/bad result resets the opposite counter, preventing oscillation.

---

## 5. Tensor Contribution Map

How L2's 4 modules populate 8 of 12 tensor dimensions.

```mermaid
graph LR
    subgraph T12["12D Tensor Dimensions"]
        D0["D0: service_id"]
        D1["D1: port<br/>(uncovered)"]
        D2["D2: tier"]
        D3["D3: deps"]
        D4["D4: agents"]
        D5["D5: protocol<br/>(uncovered)"]
        D6["D6: health"]
        D7["D7: uptime"]
        D8["D8: synergy<br/>(uncovered)"]
        D9["D9: latency"]
        D10["D10: error_rate"]
        D11["D11: temporal<br/>(uncovered)"]
    end

    M09["M09 ServiceRegistry"] -->|"count/12"| D0
    M09 -->|"avg tier"| D2
    M09 -->|"avg deps/12"| D3
    M09 -->|"healthy ratio"| D4

    M10["M10 HealthMonitor"] -->|"aggregate_health"| D6
    M10 -->|"1 - health"| D10

    M11["M11 LifecycleManager"] -->|"fraction_running"| D6
    M11 -->|"1 - avg(restarts/max)"| D7

    M12["M12 ResilienceManager"] -->|"closed_fraction"| D9
    M12 -->|"avg_failure_rate"| D10

    style D0 fill:#c8e6c9,stroke:#2e7d32
    style D2 fill:#c8e6c9,stroke:#2e7d32
    style D3 fill:#c8e6c9,stroke:#2e7d32
    style D4 fill:#c8e6c9,stroke:#2e7d32
    style D6 fill:#c8e6c9,stroke:#2e7d32
    style D7 fill:#c8e6c9,stroke:#2e7d32
    style D9 fill:#c8e6c9,stroke:#2e7d32
    style D10 fill:#c8e6c9,stroke:#2e7d32
    style D1 fill:#ffcdd2,stroke:#c62828
    style D5 fill:#ffcdd2,stroke:#c62828
    style D8 fill:#ffcdd2,stroke:#c62828
    style D11 fill:#ffcdd2,stroke:#c62828
    style M09 fill:#bbdefb,stroke:#1565c0
    style M10 fill:#bbdefb,stroke:#1565c0
    style M11 fill:#bbdefb,stroke:#1565c0
    style M12 fill:#bbdefb,stroke:#1565c0
```

### Dimension Overlap Notes

| Dimension | Contributors | Resolution |
|-----------|-------------|------------|
| D6 (health) | M10 + M11 | CoverageBitmap merge — averaged or higher-confidence wins |
| D10 (error_rate) | M10 + M12 | M10 = service-level health, M12 = request-level reliability |

### Coverage Summary

```
L2 covers: D0, D2, D3, D4, D6, D7, D9, D10  (8/12 = 67%)
Uncovered:  D1 (port), D5 (protocol), D8 (synergy), D11 (temporal)
            → Provided by L3 (Core Logic) and L4 (Integration)
```

---

## 6. Signal Flow Topology

How L2 modules emit health signals through the shared SignalBus.

```mermaid
graph TB
    subgraph SIGNALS["Signal Flow Topology"]
        direction TB

        SB["Arc SignalBus<br/>(L1 Foundation)"]

        subgraph EMITTERS["L2 Signal Emitters"]
            E1["M09: update_health()<br/>HealthStatus transition"]
            E2["M10: record_result()<br/>FSM status transition"]
            E3["M11: apply_transition()<br/>health_score delta != 0"]
            E4["M12: record_success/failure()<br/>CircuitState transition"]
        end

        subgraph CONSUMERS["Signal Consumers (L3+)"]
            C1["L3: Core Logic<br/>Tensor aggregation"]
            C2["L4: Integration<br/>Cross-service events"]
            C3["L5: Learning<br/>STDP co-activation"]
            C4["L6: Consensus<br/>PBFT voting input"]
        end

        E1 -->|"HealthSignal"| SB
        E2 -->|"HealthSignal"| SB
        E3 -->|"HealthSignal"| SB
        E4 -->|"HealthSignal"| SB

        SB -->|"broadcast"| C1
        SB -->|"broadcast"| C2
        SB -->|"broadcast"| C3
        SB -->|"broadcast"| C4
    end

    style SB fill:#fff176,stroke:#f57f17,stroke-width:3px
    style EMITTERS fill:#e8f5e9,stroke:#2e7d32
    style CONSUMERS fill:#e3f2fd,stroke:#1565c0
```

### Emission Rules

- Signals are emitted **only on state transitions**, not on every API call
- M09: emits when `old_health != new_health`
- M10: emits when FSM state changes (e.g., Healthy→Degraded)
- M11: emits when `status_health_score(from) != status_health_score(to)`
- M12: emits on any circuit state change (Closed↔Open↔HalfOpen)

### Signal Payload

```rust
HealthSignal {
    service_id: String,
    score: f64,        // 0.0-1.0, quantized
    timestamp: Timestamp,
}
```

---

## 7. Data Flow & Integration

End-to-end request flow through all L2 modules showing 4 operational phases.

```mermaid
graph TB
    subgraph DATAFLOW["L2 Services — Data Flow & Integration"]
        direction TB

        EXT["External Request / Health Probe"]
        ORCH["Orchestrator (L3+)"]

        subgraph REGISTER["Phase 1: Registration"]
            BOOT["Bootstrap<br/>register_ultraplate_services()"]
            SR["M09: ServiceRegistry<br/>14 methods"]
            SDEF["12 ServiceDefinitions<br/>ports, tiers, protocols"]
        end

        subgraph MONITOR["Phase 2: Monitoring"]
            HP["HealthProbe Config"]
            HM["M10: HealthMonitor<br/>11 methods"]
            HCR["HealthCheckResult<br/>success / failure + latency"]
            FSM_H["Health FSM<br/>Unknown - Healthy - Degraded - Unhealthy"]
        end

        subgraph LIFECYCLE["Phase 3: Lifecycle"]
            LM["M11: LifecycleManager<br/>18 methods"]
            FSM_L["Lifecycle FSM<br/>Stopped - Starting - Running - Stopping - Failed"]
            BACK["Exponential Backoff<br/>1s - 2s - 4s - 8s - 16s"]
        end

        subgraph PROTECT["Phase 4: Protection"]
            CBR["M12: CircuitBreakerRegistry<br/>12 methods"]
            LB["M12: LoadBalancer<br/>10 methods"]
            FSM_C["Circuit FSM<br/>Closed - Open - HalfOpen"]
            ALG["Algorithm Selection<br/>RR | WRR | LC | Random"]
            EP["Endpoint Pool<br/>healthy filtering +<br/>connection tracking"]
        end

        subgraph TENSOR["Tensor Output"]
            T12["Tensor12D<br/>8 of 12 dimensions covered"]
        end

        BOOT --> SR
        SR --> SDEF

        EXT --> HCR
        HCR --> HM
        HM --> FSM_H

        ORCH --> LM
        LM --> FSM_L
        FSM_L -.-> BACK

        EXT --> CBR
        CBR --> FSM_C
        EXT --> LB
        LB --> ALG
        ALG --> EP

        SR -.->|"D0,D2,D3,D4"| T12
        HM -.->|"D6,D10"| T12
        LM -.->|"D6,D7"| T12
        CBR -.->|"D9,D10"| T12
    end

    style REGISTER fill:#e8f5e9,stroke:#2e7d32
    style MONITOR fill:#fff3e0,stroke:#e65100
    style LIFECYCLE fill:#e3f2fd,stroke:#1565c0
    style PROTECT fill:#fce4ec,stroke:#c62828
    style TENSOR fill:#f3e5f5,stroke:#7b1fa2
```

### Phase Dependencies

```
Phase 1 (Registration) → Independent, runs at startup
Phase 2 (Monitoring)   → Depends on Phase 1 for service definitions
Phase 3 (Lifecycle)    → Driven by orchestrator, may trigger Phase 2 re-checks
Phase 4 (Protection)   → Independent, operates on request path
                          Circuit breakers may trigger Phase 3 restarts
```

### Request Path (Phase 4 Detail)

```
Incoming Request
    │
    ▼
allow_request(service_id)  ← CircuitBreakerOps
    │
    ├── Denied (Open) → Error response
    │
    └── Allowed (Closed/HalfOpen)
            │
            ▼
        select_endpoint(service_id)  ← LoadBalancing
            │                         (active_connections++)
            ▼
        Forward to endpoint
            │
            ├── Success → record_request(success)
            │              record_success() on circuit
            │              (active_connections--)
            │
            └── Failure → record_request(failure)
                           record_failure() on circuit
                           (active_connections--, errors++)
```

---

## Appendix: Concurrency Architecture

All L2 managers share this interior mutability pattern:

```mermaid
graph LR
    subgraph MANAGER["L2 Manager (any)"]
        LOCK["RwLock&lt;InternalState&gt;"]
        BUS["Option&lt;Arc&lt;SignalBus&gt;&gt;"]
        MET["Option&lt;Arc&lt;MetricsRegistry&gt;&gt;"]
    end

    subgraph READ["Read Path"]
        R1["state.read()"]
        R2["clone data"]
        R3["release lock"]
        R4["return owned T"]
    end

    subgraph WRITE["Write Path"]
        W1["state.write()"]
        W2["mutate state"]
        W3["compute signal"]
        W4["release lock"]
        W5["emit signal (if bus)"]
        W6["record metric (if metrics)"]
    end

    LOCK --> R1
    R1 --> R2
    R2 --> R3
    R3 --> R4

    LOCK --> W1
    W1 --> W2
    W2 --> W3
    W3 --> W4
    W4 --> W5
    W5 --> W6
    BUS -.-> W5
    MET -.-> W6

    style MANAGER fill:#fff3e0,stroke:#e65100
    style READ fill:#e8f5e9,stroke:#2e7d32
    style WRITE fill:#e3f2fd,stroke:#1565c0
```

### Key Properties

- **No `std::sync::RwLock`** — All locks are `parking_lot::RwLock` (no poisoning, faster)
- **Signal emission after lock release** — Prevents deadlock if signal handler acquires another lock
- **Owned returns (C7)** — All data crossing lock boundaries is cloned, preventing lock lifetime leaks
- **No nested locks** — Each manager has exactly one `RwLock`, never holds two simultaneously

---

*Generated: 2026-03-01 | The Maintenance Engine v1.0.0 | 7 Architectural Schematics*
