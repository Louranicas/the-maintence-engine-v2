# M1+M2 Unified Architecture — Claude Code Reference

> **Purpose:** Single-document cross-layer integration view for Claude Code instances implementing L3-L8.
> **Scope:** L1 Foundation (M00-M08, 16,711 LOC) + L2 Services (M09-M12, 7,196 LOC)
> **Optimized for:** Fast context reconstruction, trait lookup, dimension mapping, implementation guidance.
> **Generated:** 2026-03-07 | **Source:** 16 Rust files verified against source

---

## Quick Reference: What L3+ Needs From L1+L2

```
IMPORT:  use crate::m1_foundation::{Error, Result, Timestamp, ModuleId, ...};
         use crate::m2_services::{ServiceRegistry, HealthMonitor, ...};
TRAITS:  impl TensorContributor for YourModule { ... }
SIGNALS: self.signal_bus.as_ref().map(|bus| bus.emit_health(...));
LOCK:    parking_lot::RwLock<InternalState> — all methods &self
BUILD:   YourStruct::builder("id").field(val).build()?
TEST:    fn _assert_object_safe(_: &dyn YourTrait) {}
```

---

## 1. Full System Topology (M1+M2 Unified)

```mermaid
graph TB
    subgraph L1["L1 FOUNDATION (16,711 LOC | 678 tests)"]
        direction TB

        subgraph LEAF["Tier 0: Leaf Types (zero deps)"]
            ST["M00: shared_types.rs<br/>ModuleId AgentId Timestamp<br/>HealthReport DimensionIndex<br/>CoverageBitmap Tensor12D"]
            NAM["NAM: nam.rs<br/>AgentOrigin Confidence<br/>Outcome LearningSignal<br/>Dissent"]
        end

        subgraph INFRA["Tier 1: Infrastructure"]
            ERR["M01: error.rs<br/>Error(16 variants) Result&lt;T&gt;<br/>Severity ErrorClassifier<br/>AnnotatedError"]
            CFG["M02: config.rs<br/>ConfigProvider trait<br/>Config ConfigBuilder<br/>ConfigManager(hot-reload)"]
            LOG["M03: logging.rs<br/>CorrelationProvider trait<br/>LogContext init_logging"]
            MET["M04: metrics.rs<br/>MetricRecorder trait<br/>Counter Gauge Histogram<br/>MetricsRegistry"]
        end

        subgraph PLATFORM["Tier 2: Platform"]
            STA["M05: state.rs<br/>StateStore trait<br/>DatabasePool QueryBuilder<br/>11 DatabaseTypes"]
            RES["M06: resources.rs<br/>ResourceCollector trait<br/>SystemResources<br/>AdaptiveResourceLimits"]
        end

        subgraph EXTENSION["Tier 3: Extension"]
            SIG["M07: signals.rs<br/>SignalSubscriber trait<br/>SignalBus(3 channels)<br/>HealthSignal LearningEvent<br/>DissentEvent"]
            TEN["M08: tensor_registry.rs<br/>TensorContributor trait<br/>TensorRegistry<br/>ComposedTensor"]
        end
    end

    subgraph L2["L2 SERVICES (7,196 LOC | 320 tests)"]
        direction TB

        subgraph L2COORD["Coordinator"]
            MOD2["mod.rs<br/>ServiceStatus HealthStatus<br/>ServiceTier CircuitState<br/>ServiceState RestartConfig"]
        end

        subgraph L2MODULES["Modules"]
            M09["M09: ServiceRegistry<br/>ServiceDiscovery (14 methods)<br/>12 ULTRAPLATE services<br/>RwLock&lt;RegistryState&gt;"]
            M10["M10: HealthMonitor<br/>HealthMonitoring (11 methods)<br/>FSM: Unknown-Healthy-<br/>Degraded-Unhealthy"]
            M11["M11: LifecycleManager<br/>LifecycleOps (18 methods)<br/>FSM: Stopped-Starting-<br/>Running-Stopping-Failed"]
            M12["M12: ResilienceManager<br/>CircuitBreakerOps (12)<br/>LoadBalancing (10)<br/>FSM: Closed-Open-HalfOpen"]
        end
    end

    %% L1 internal deps
    ERR --> NAM
    CFG --> ST & ERR
    LOG --> ST & ERR
    MET --> ST & ERR
    STA --> ST & ERR & CFG
    RES --> ST & ERR
    SIG --> ST & ERR & NAM
    TEN --> ST

    %% L2 depends on L1 (C1: no upward imports)
    M09 -.->|"Error Timestamp SignalBus TensorContributor"| L1
    M10 -.->|"Error Timestamp SignalBus TensorContributor"| L1
    M11 -.->|"Error Timestamp SignalBus TensorContributor"| L1
    M12 -.->|"Error TensorContributor MetricsRegistry"| L1

    style L1 fill:#f3e5f5,stroke:#7b1fa2,stroke-width:2px
    style L2 fill:#e3f2fd,stroke:#1565c0,stroke-width:2px
    style LEAF fill:#e8f5e9,stroke:#2e7d32
    style INFRA fill:#fff3e0,stroke:#e65100
    style PLATFORM fill:#fce4ec,stroke:#c62828
    style EXTENSION fill:#fff176,stroke:#f57f17
```

---

## 2. Trait → Implementor → Dimension Map

Complete lookup table for every trait defined in L1+L2 and what implements it.

```mermaid
graph LR
    subgraph TRAITS["8 L1 Traits"]
        EC["ErrorClassifier<br/>5 methods"]
        CP["ConfigProvider<br/>Send+Sync, 5 methods"]
        CR["CorrelationProvider<br/>Send+Sync, 3 methods"]
        MR["MetricRecorder<br/>Send+Sync, 4 methods"]
        SS["StateStore<br/>Send+Sync, 3 methods"]
        RC["ResourceCollector<br/>Send+Sync, 5 methods"]
        SB["SignalSubscriber<br/>Send+Sync+Debug, 4 methods"]
        TC["TensorContributor<br/>Send+Sync+Debug, 3 methods"]
    end

    subgraph L1_IMPL["L1 Implementors"]
        I_ERR["Error"]
        I_CM["ConfigManager"]
        I_LC["LogContext"]
        I_MR["MetricsRegistry"]
        I_DP["DatabasePool"]
        I_RM["ResourceManager"]
    end

    subgraph L2_IMPL["L2 Implementors"]
        I_SR["ServiceRegistry"]
        I_HM["HealthMonitor"]
        I_LM["LifecycleManager"]
        I_CBR["CircuitBreakerRegistry"]
    end

    subgraph DIMS["Tensor Dimensions"]
        D0["D0 service_id"]
        D2["D2 tier"]
        D3["D3 deps"]
        D4["D4 agents"]
        D6["D6 health"]
        D7["D7 uptime"]
        D9["D9 latency"]
        D10["D10 error_rate"]
    end

    EC --> I_ERR
    CP --> I_CM
    CR --> I_LC
    MR --> I_MR
    SS --> I_DP
    RC --> I_RM

    TC --> I_SR & I_HM & I_LM & I_CBR

    I_SR --> D0 & D2 & D3 & D4
    I_HM --> D6 & D10
    I_LM --> D6 & D7
    I_CBR --> D9 & D10

    style TRAITS fill:#f3e5f5,stroke:#7b1fa2
    style L1_IMPL fill:#e8f5e9,stroke:#2e7d32
    style L2_IMPL fill:#e3f2fd,stroke:#1565c0
    style DIMS fill:#fff3e0,stroke:#e65100
```

### Machine-Parseable Trait Index

| Trait | Module | Bounds | Methods | Defaults | Object-Safe | Arc&lt;dyn&gt; |
|-------|--------|--------|---------|----------|-------------|------------|
| `ErrorClassifier` | M01 | none | 5 | 2 | YES | no |
| `ConfigProvider` | M02 | Send+Sync | 5 | 2 | YES | `Arc<dyn ConfigProvider>` |
| `CorrelationProvider` | M03 | Send+Sync | 3 | 1 | YES | no |
| `MetricRecorder` | M04 | Send+Sync | 4 | 0 | YES | no |
| `StateStore` | M05 | Send+Sync | 3 | 1 | YES | no |
| `ResourceCollector` | M06 | Send+Sync | 5 | 2 | YES | no |
| `SignalSubscriber` | M07 | Send+Sync+Debug | 4 | 3 | YES | `Arc<dyn SignalSubscriber>` |
| `TensorContributor` | M08 | Send+Sync+Debug | 3 | 0 | YES | `Arc<dyn TensorContributor>` |
| `ServiceDiscovery` | M09 | Send+Sync | 14 | 0 | YES | no |
| `HealthMonitoring` | M10 | Send+Sync | 11 | 0 | YES | no |
| `LifecycleOps` | M11 | Send+Sync | 18 | 0 | YES | no |
| `CircuitBreakerOps` | M12 | Send+Sync | 12 | 0 | YES | no |
| `LoadBalancing` | M12 | Send+Sync | 10 | 0 | YES | no |

### Dimension Coverage Matrix

| Dim | Name | M09 | M10 | M11 | M12 | L1 Freestanding | L3+ Target |
|-----|------|-----|-----|-----|-----|-----------------|-----------|
| D0 | service_id | `count/12` | | | | LogContext | |
| D1 | port | | | | | Config | |
| D2 | tier | `avg tier` | | | | Config, Resources | |
| D3 | deps | `avg/12` | | | | | |
| D4 | agents | `healthy%` | | | | | |
| D5 | protocol | | | | | LogContext | M19-M22 |
| D6 | health | | `aggregate` | `%running` | | Config, Resources | |
| D7 | uptime | | | `1-restarts` | | | |
| D8 | synergy | | | | | | M24, N01, N04 |
| D9 | latency | | | | `closed%` | Resources | |
| D10 | error_rate | | `1-health` | | `fail_rate` | Metrics, Resources | |
| D11 | temporal | | | | | | M05, N01 |

**Coverage:** L1+L2 = 8/12 (67%). Gaps: D1(port), D5(protocol), D8(synergy), D11(temporal) — filled by L3+.

---

## 3. End-to-End Request Lifecycle

Complete sequence showing how a request flows through M1 types and M2 services.

```mermaid
sequenceDiagram
    participant EXT as External Request
    participant M12_CB as M12: CircuitBreaker
    participant M12_LB as M12: LoadBalancer
    participant M10 as M10: HealthMonitor
    participant M11 as M11: Lifecycle
    participant M09 as M09: Registry
    participant SIG as M07: SignalBus
    participant TEN as M08: TensorRegistry
    participant L3 as L3+: Consumer

    Note over EXT,L3: Phase 1: Startup (one-time)
    M09->>M09: register_ultraplate_services()
    M09->>TEN: register(Arc<dyn TensorContributor>)
    M10->>TEN: register(Arc<dyn TensorContributor>)
    M11->>TEN: register(Arc<dyn TensorContributor>)
    M12_CB->>TEN: register(Arc<dyn TensorContributor>)

    Note over EXT,L3: Phase 2: Request Path
    EXT->>M12_CB: allow_request(service_id)
    alt Circuit OPEN
        M12_CB-->>EXT: DENIED (Error::CircuitOpen)
    else Circuit CLOSED/HALF_OPEN
        M12_CB-->>EXT: ALLOWED
        EXT->>M12_LB: select_endpoint(service_id)
        M12_LB-->>EXT: Endpoint{host, port}
        EXT->>EXT: forward request
        alt Success
            EXT->>M12_CB: record_success()
            M12_CB->>SIG: emit_health(score=1.0)
        else Failure
            EXT->>M12_CB: record_failure()
            M12_CB->>SIG: emit_health(score=0.0)
            Note over M12_CB: If failures >= threshold:<br/>Closed → Open
        end
    end

    Note over EXT,L3: Phase 3: Health Monitoring (periodic)
    M10->>M10: check_health(probe)
    M10->>M10: FSM transition
    M10->>SIG: emit_health(HealthSignal)
    SIG->>L3: broadcast to subscribers

    Note over EXT,L3: Phase 4: Lifecycle Response
    L3->>M11: restart_service(id)
    M11->>M11: FSM: Failed → Starting
    M11->>SIG: emit_health(score=0.5)
    M11->>M11: backoff(2^n seconds)
    M11->>M11: FSM: Starting → Running
    M11->>SIG: emit_health(score=1.0)

    Note over EXT,L3: Phase 5: Tensor Composition (on-demand)
    L3->>TEN: compose()
    TEN->>M09: contribute_tensor()
    TEN->>M10: contribute_tensor()
    TEN->>M11: contribute_tensor()
    TEN->>M12_CB: contribute_tensor()
    TEN-->>L3: ComposedTensor{12D, coverage=8/12}
```

---

## 4. Three FSMs (Side-by-Side Reference)

### 4a. M10 Health Monitor FSM

```mermaid
stateDiagram-v2
    [*] --> Unknown : register_probe()
    Unknown --> Healthy : successes >= healthy_threshold
    Unknown --> Unhealthy : failures >= unhealthy_threshold
    Healthy --> Degraded : single failure
    Degraded --> Unhealthy : failures >= unhealthy_threshold
    Degraded --> Healthy : successes >= healthy_threshold
    Unhealthy --> Healthy : successes >= healthy_threshold

    note right of Healthy : score=1.0
    note right of Degraded : score=0.5
    note right of Unhealthy : score=0.0
    note right of Unknown : score=0.0
```

### 4b. M11 Lifecycle FSM

```mermaid
stateDiagram-v2
    [*] --> Stopped : register()
    Stopped --> Starting : start_service()
    Starting --> Running : mark_running()
    Starting --> Failed : mark_failed()
    Running --> Stopping : stop_service()
    Running --> Failed : mark_failed()
    Running --> Starting : restart_service()
    Stopping --> Stopped : mark_stopped()
    Failed --> Starting : start_service()

    note right of Stopped : score=0.0 restart_count=0
    note right of Starting : score=0.5
    note right of Running : score=1.0
    note right of Stopping : score=0.5
    note right of Failed : score=0.0
```

### 4c. M12 Circuit Breaker FSM

```mermaid
stateDiagram-v2
    [*] --> Closed : register_breaker()
    Closed --> Open : failures >= 5
    Open --> HalfOpen : timeout(30s) elapsed
    HalfOpen --> Closed : 3 consecutive successes
    HalfOpen --> Open : any failure

    note right of Closed : score=1.0 all requests allowed
    note right of Open : score=0.0 all requests DENIED
    note right of HalfOpen : score=0.5 probing
```

### FSM Quick Reference

| FSM | States | Healthy State | Degraded State | Trigger Events |
|-----|--------|--------------|----------------|----------------|
| Health (M10) | Unknown→Healthy→Degraded→Unhealthy | Healthy(1.0) | Degraded(0.5) | `record_result()` |
| Lifecycle (M11) | Stopped→Starting→Running→Stopping→Failed | Running(1.0) | Starting/Stopping(0.5) | `start/stop/restart_service()` |
| Circuit (M12) | Closed→Open→HalfOpen | Closed(1.0) | HalfOpen(0.5) | `record_success/failure()` |

---

## 5. Signal Emission & Consumption Flow

```mermaid
graph TB
    subgraph EMITTERS["Signal Emitters (L2)"]
        E1["M09: update_health()<br/>on HealthStatus change"]
        E2["M10: record_result()<br/>on FSM transition"]
        E3["M11: apply_transition()<br/>on score delta != 0"]
        E4["M12: record_success/failure()<br/>on CircuitState change"]
    end

    subgraph BUS["SignalBus (M07) — 3 Channels"]
        CH_H["emit_health()<br/>HealthSignal{service_id, score, timestamp}"]
        CH_L["emit_learning()<br/>LearningEvent{signal, outcome, agent}"]
        CH_D["emit_dissent()<br/>DissentEvent{dissent, agent}"]
        SUBS["subscribers: Arc&lt;RwLock&lt;Vec&lt;Arc&lt;dyn SignalSubscriber&gt;&gt;&gt;&gt;"]
    end

    subgraph CONSUMERS["Signal Consumers (L3+)"]
        C_L3["L3 Pipeline<br/>on_health() → trigger remediation"]
        C_L5["L5 Hebbian<br/>on_learning() → STDP update"]
        C_L6["L6 Consensus<br/>on_dissent() → PBFT vote"]
        C_L7["L7 Observer<br/>on_health() → emergence detect"]
    end

    E1 & E2 & E3 & E4 -->|"HealthSignal"| CH_H
    CH_H --> SUBS
    SUBS -->|"read lock → iterate → drop → write stats"| C_L3 & C_L5 & C_L7
    CH_L --> SUBS
    CH_D --> SUBS
    SUBS --> C_L6

    style BUS fill:#fff176,stroke:#f57f17,stroke-width:2px
    style EMITTERS fill:#e8f5e9,stroke:#2e7d32
    style CONSUMERS fill:#e3f2fd,stroke:#1565c0
```

**Emission Rule:** Signals fire ONLY on state transitions (old != new), not on every API call.

**Lock Protocol:** Read subscribers → call callbacks → drop guard → write stats. Never nested.

---

## 6. Concurrency & Lock Ordering

```mermaid
graph TB
    subgraph ATOMICS["Atomic (Lock-Free)"]
        A1["GLOBAL_TICK<br/>AtomicU64 Relaxed"]
        A2["ConfigManager.reload_flag<br/>AtomicBool SeqCst"]
    end

    subgraph RWLOCK["parking_lot::RwLock (Read-Heavy)"]
        R1["ConfigManager.config"]
        R2["Counter/Gauge/Histogram.values"]
        R3["MetricsRegistry.{counters,gauges,histograms}"]
        R4["SignalBus.subscribers"]
        R5["SignalBus.stats"]
        R6["ServiceRegistry.state"]
        R7["HealthMonitor.state"]
        R8["LifecycleManager.entries"]
        R9["CircuitBreakerRegistry.breakers"]
        R10["LoadBalancer.pools"]
    end

    subgraph ONCE["OnceLock (Set-Once)"]
        O1["LOGGING_INITIALIZED"]
    end

    subgraph NOSYNC["No Internal Sync (Caller Wraps)"]
        N1["TensorRegistry"]
        N2["ResourceManager"]
        N3["All vocabulary types"]
    end

    R4 -->|"ALWAYS before"| R5

    style ATOMICS fill:#e8f5e9,stroke:#2e7d32
    style RWLOCK fill:#fff3e0,stroke:#e65100
    style ONCE fill:#f3e5f5,stroke:#7b1fa2
    style NOSYNC fill:#ffcdd2,stroke:#c62828
```

### Lock Ordering Rules (L3+ MUST follow)

1. **SignalBus:** subscribers lock BEFORE stats lock (never reversed)
2. **L2 Managers:** Each has exactly ONE `RwLock` — no nesting possible
3. **Signal emission:** ALWAYS release the manager's lock BEFORE calling `signal_bus.emit()`
4. **Cross-module:** Never hold locks from two different modules simultaneously
5. **All data crossing lock boundaries:** Clone/owned — never return references through guards

---

## 7. Error Propagation Topology

```mermaid
graph TB
    subgraph ERROR_TYPES["Error Enum (16 Variants)"]
        E_CFG["Config(1000)"]
        E_DB["Database(1100)"]
        E_NET["Network(1200)<br/>CircuitOpen(1201)<br/>Timeout(1202)"]
        E_CON["ConsensusQuorum(1300)<br/>ViewChange(1301)"]
        E_LRN["PathwayNotFound(1400)<br/>TensorValidation(1401)"]
        E_VAL["Validation(1500)"]
        E_IO["Io(1600)"]
        E_PIP["Pipeline(1700)"]
        E_SVC["ServiceNotFound(1800)<br/>HealthCheckFailed(1801)<br/>EscalationRequired(1802)"]
        E_OTH["Other(1900)"]
    end

    subgraph CLASSIFIERS["ErrorClassifier Trait"]
        IS_R["is_retryable()"]
        IS_T["is_transient()"]
        SEV["severity()"]
        TO_T["to_tensor_signal()<br/>D6=health D10=error_rate"]
    end

    subgraph SEVERITY["Severity Levels"]
        S_L["Low"]
        S_M["Medium"]
        S_H["High"]
        S_C["Critical"]
    end

    subgraph LAYERS["Error Sources by Layer"]
        L1_SRC["L1: Config, Database, Validation, IO"]
        L2_SRC["L2: ServiceNotFound, HealthCheckFailed,<br/>CircuitOpen, Timeout"]
        L3_SRC["L3: Pipeline, EscalationRequired"]
        L5_SRC["L5: PathwayNotFound, TensorValidation"]
        L6_SRC["L6: ConsensusQuorum, ViewChange"]
    end

    ERROR_TYPES --> CLASSIFIERS
    CLASSIFIERS --> SEVERITY

    L1_SRC --> E_CFG & E_DB & E_VAL & E_IO
    L2_SRC --> E_SVC & E_NET
    L3_SRC --> E_PIP & E_SVC
    L5_SRC --> E_LRN
    L6_SRC --> E_CON

    style ERROR_TYPES fill:#ffcdd2,stroke:#c62828
    style CLASSIFIERS fill:#fff3e0,stroke:#e65100
    style SEVERITY fill:#f3e5f5,stroke:#7b1fa2
```

### Retryability Map

| Error | Retryable | Transient | Severity | Tensor Impact |
|-------|-----------|-----------|----------|---------------|
| Config | NO | NO | Medium | D6=0.5 |
| Database | YES | YES | High | D6=0.3, D10=0.7 |
| Network | YES | YES | Medium | D9=0.8 |
| CircuitOpen | NO | YES | High | D6=0.0, D9=1.0 |
| Timeout | YES | YES | Medium | D9=0.9 |
| Validation | NO | NO | Low | D10=0.3 |
| ServiceNotFound | NO | NO | High | D6=0.0 |
| Pipeline | YES | NO | High | D6=0.3 |

---

## 8. Interior Mutability Pattern (Template for L3+)

Every L2 manager follows this exact pattern. L3+ modules MUST follow it too.

```mermaid
graph LR
    subgraph STRUCT["pub struct ModuleName"]
        LOCK["inner: RwLock&lt;InternalState&gt;"]
        BUS["signal_bus: Option&lt;Arc&lt;SignalBus&gt;&gt;"]
        METRICS["metrics: Option&lt;Arc&lt;MetricsRegistry&gt;&gt;"]
    end

    subgraph READ_PATH["Read Path (&self)"]
        R1["1. let guard = self.inner.read()"]
        R2["2. let data = guard.field.clone()"]
        R3["3. drop(guard) // implicit"]
        R4["4. return Ok(data)  // OWNED"]
    end

    subgraph WRITE_PATH["Write Path (&self)"]
        W1["1. let mut guard = self.inner.write()"]
        W2["2. let old = guard.state.clone()"]
        W3["3. guard.state = new_state"]
        W4["4. let signal = compute_signal(old, new)"]
        W5["5. drop(guard) // EXPLICIT — before emit"]
        W6["6. self.emit_signal(signal)"]
        W7["7. self.record_metric(delta)"]
    end

    STRUCT --> READ_PATH
    STRUCT --> WRITE_PATH

    style STRUCT fill:#fff3e0,stroke:#e65100
    style READ_PATH fill:#e8f5e9,stroke:#2e7d32
    style WRITE_PATH fill:#e3f2fd,stroke:#1565c0
```

### Rust Template

```rust
use parking_lot::RwLock;
use crate::m1_foundation::{
    Error, Result, Timestamp, ModuleId, Tensor12D,
    CoverageBitmap, DimensionIndex, SignalBus, MetricsRegistry,
    TensorContributor, TensorContribution,
};

pub struct YourModule {
    inner: RwLock<YourState>,
    signal_bus: Option<Arc<SignalBus>>,
    metrics: Option<Arc<MetricsRegistry>>,
}

struct YourState {
    // ... internal mutable state
}

impl YourModule {
    #[must_use]
    pub fn new() -> Self { /* ... */ }

    pub fn with_signal_bus(mut self, bus: Arc<SignalBus>) -> Self {
        self.signal_bus = Some(bus);
        self
    }

    // Read: clone through lock, return owned
    pub fn get_status(&self) -> Result<YourStatus> {
        let guard = self.inner.read();
        Ok(guard.status.clone())
    }

    // Write: mutate, compute signal, drop lock, THEN emit
    pub fn update(&self, input: Input) -> Result<()> {
        let signal = {
            let mut guard = self.inner.write();
            let old = guard.value;
            guard.value = input.new_value;
            (old != guard.value).then(|| HealthSignal { /* ... */ })
        }; // guard dropped here
        if let Some(sig) = signal {
            if let Some(bus) = &self.signal_bus {
                bus.emit_health(sig);
            }
        }
        Ok(())
    }
}

impl TensorContributor for YourModule {
    fn module_id(&self) -> ModuleId { ModuleId::new(YOUR_MODULE_ID) }

    fn contribute_tensor(&self) -> TensorContribution {
        let guard = self.inner.read();
        let mut tensor = Tensor12D::default();
        let mut coverage = CoverageBitmap::empty();
        // Set your dimensions:
        tensor[DimensionIndex::D6] = guard.health_score;
        coverage = coverage.with_dimension(DimensionIndex::D6);
        TensorContribution::Snapshot { tensor, coverage }
    }

    fn contribution_type(&self) -> &'static str { "snapshot" }
}
```

---

## 9. ULTRAPLATE Service Bootstrap Map

```mermaid
graph LR
    subgraph SERVICES["12 ULTRAPLATE Services (M09 Bootstrap)"]
        subgraph T1["Tier 1 (w=1.5)"]
            S1["maintenance-engine :8080"]
            S2["devops-engine :8081"]
        end
        subgraph T2["Tier 2 (w=1.3)"]
            S3["synthex :8090"]
            S4["san-k7 :8100"]
            S5["codesynthor-v7 :8110"]
        end
        subgraph T3["Tier 3 (w=1.2)"]
            S6["nais :8101"]
            S7["bash-engine :8102"]
            S8["tool-maker :8103"]
        end
        subgraph T4["Tier 4 (w=1.1)"]
            S9["claude-context-manager :8104"]
            S10["tool-library :8105"]
            S11["sphere-vortex :8120"]
        end
        subgraph T5["Tier 5 (w=1.0)"]
            S12["library-agent :8083"]
        end
    end

    style T1 fill:#ffcdd2,stroke:#c62828
    style T2 fill:#fff3e0,stroke:#e65100
    style T3 fill:#fff9c4,stroke:#f9a825
    style T4 fill:#e8f5e9,stroke:#2e7d32
    style T5 fill:#e3f2fd,stroke:#1565c0
```

---

## 10. Tensor Composition Pipeline (L1+L2 Unified)

```mermaid
flowchart TB
    subgraph CONTRIB["TensorContributor Implementations"]
        direction LR
        M09C["M09<br/>D0,D2,D3,D4"]
        M10C["M10<br/>D6,D10"]
        M11C["M11<br/>D6,D7"]
        M12C["M12<br/>D9,D10"]
    end

    subgraph L1FREE["L1 Freestanding Tensors"]
        direction LR
        CFG_T["Config.to_tensor()<br/>D1,D2,D6"]
        MET_T["MetricSnapshot.to_tensor()<br/>D2,D6,D10"]
        RES_T["SystemResources.to_tensor()<br/>D2,D5,D6,D9,D10"]
    end

    subgraph REGISTRY["TensorRegistry (M08)"]
        REG["Vec&lt;Arc&lt;dyn TensorContributor&gt;&gt;"]
        COMPOSE["compose()"]
    end

    subgraph ALGO["Composition Algorithm"]
        A1["1. Call contribute() on each contributor"]
        A2["2. Accumulate sum + count per dimension"]
        A3["3. Union all CoverageBitmaps"]
        A4["4. avg = sum/count, clamp [0.0, 1.0]"]
    end

    subgraph OUTPUT["ComposedTensor"]
        T12["Tensor12D [D0..D11]"]
        COV["CoverageBitmap: 8/12"]
        META["snap_count=4, stream_count=0"]
    end

    M09C & M10C & M11C & M12C -->|"Arc&lt;dyn&gt;"| REG
    REG --> COMPOSE
    COMPOSE --> A1 --> A2 --> A3 --> A4 --> T12 & COV & META

    CFG_T & MET_T & RES_T -->|"averaged"| BFT["build_foundation_tensor()"]
    BFT -.->|"separate path"| OUTPUT

    style CONTRIB fill:#e3f2fd,stroke:#1565c0
    style L1FREE fill:#f3e5f5,stroke:#7b1fa2
    style REGISTRY fill:#fff176,stroke:#f57f17
    style OUTPUT fill:#e8f5e9,stroke:#2e7d32
```

---

## 11. Implementation Checklist for L3+ Modules

Every new module MUST satisfy these requirements (derived from L1+L2 gold standard):

| # | Requirement | Pattern | Verified By |
|---|-------------|---------|-------------|
| 1 | `impl TensorContributor` | See template in Section 8 | Compile-time (C3) |
| 2 | All methods `&self` | `parking_lot::RwLock<Inner>` | Code review (C2) |
| 3 | No upward imports | `use crate::m1_foundation::*` only | Compile-time (C1) |
| 4 | Zero unsafe/unwrap/expect | `#![forbid(unsafe_code)]` | Clippy (C4) |
| 5 | Signal emission on transitions | `Arc<SignalBus>` field | Architecture (C6) |
| 6 | Owned returns through locks | Clone before returning | Code review (C7) |
| 7 | Builder pattern for constructors | `YourBuilder::new().field(v).build()?` | Convention |
| 8 | `#[must_use]` on pure functions | All getters, builders | Clippy pedantic |
| 9 | `Result<T>` everywhere | No panic paths | Clippy deny |
| 10 | 50+ tests per layer | Unit + FSM + signal + tensor | CI gate (C10) |
| 11 | Timestamp/Duration only | No chrono/SystemTime | Grep (C5) |
| 12 | Drop lock before emit | Explicit scope or drop() | Code review |

---

## 12. Cross-Layer Type Flow Summary

```
L1 EXPORTS → L2 CONSUMES → L3+ CONSUMES
─────────────────────────────────────────
Error, Result<T>          → All modules      → All modules
Timestamp, Duration       → All time fields  → All time fields
ModuleId (42 IDs)         → ServiceState     → Pipeline, Agent
AgentOrigin (4 variants)  → —                → Agent, PBFT
Confidence (0.0-1.0)      → —                → Pipeline, Consensus
Outcome (3 variants)      → —                → Learning, Feedback
LearningSignal            → —                → Hebbian, STDP
Dissent                   → —                → PBFT, Consensus
SignalBus (3 channels)    → All managers      → All modules
TensorContributor         → 4 impls (M09-12) → All new modules
CoverageBitmap            → TensorContrib    → TensorContrib
Tensor12D                 → ServiceState      → All tensor ops
HealthSignal              → emit on change   → subscribe in L3+
DimensionIndex (12 dims)  → contribute()     → contribute()
```

---

*M1+M2 Unified Architecture Reference v1.0 | 2026-03-07 | Optimized for Claude Code L3+ Implementation*
