# L1 Foundation — Architectural Schematics

> **Purpose:** Visual reference for L1 architecture, dependencies, signal flow, tensor composition, and concurrency model.
> All diagrams are Mermaid markdown — render in any compatible viewer.

---

## 1. Layer Architecture

```mermaid
graph TB
    subgraph L1["L1 Foundation Layer (12,908 LOC, 440 tests)"]
        direction TB
        subgraph M00["M00 mod.rs — Layer Coordinator"]
            FS["FoundationStatus"]
            BFT["build_foundation_tensor()"]
        end

        subgraph Vocabulary["Vocabulary Types (leaf — no deps)"]
            ST["shared_types.rs<br/>ModuleId, AgentId, Timestamp<br/>CoverageBitmap, DimensionIndex<br/>HealthReport"]
            ERR["error.rs<br/>Error (16 variants)<br/>Severity, ErrorClassifier<br/>AnnotatedError"]
            NAM["nam.rs<br/>AgentOrigin, Confidence<br/>Outcome, LearningSignal<br/>Dissent"]
        end

        subgraph Infra["Infrastructure Services"]
            CFG["config.rs<br/>ConfigProvider trait<br/>ConfigManager + hot-reload"]
            LOG["logging.rs<br/>CorrelationProvider trait<br/>LogContext + tracing"]
            MET["metrics.rs<br/>MetricRecorder trait<br/>Counter/Gauge/Histogram<br/>Prometheus export"]
        end

        subgraph Platform["Platform & State"]
            STA["state.rs<br/>StateStore trait<br/>DatabasePool, QueryBuilder<br/>11 DatabaseTypes"]
            RES["resources.rs<br/>ResourceCollector trait<br/>SystemResources<br/>AdaptiveResourceLimits"]
        end

        subgraph Extension["L1 Extension (M07-M08)"]
            SIG["signals.rs<br/>SignalSubscriber trait<br/>SignalBus (3 channels)<br/>HealthSignal, LearningEvent<br/>DissentEvent"]
            TEN["tensor_registry.rs<br/>TensorContributor trait<br/>TensorRegistry<br/>ComposedTensor"]
        end
    end

    M00 --> Vocabulary & Infra & Platform & Extension
    CFG --> ST & ERR
    LOG --> ST & ERR
    MET --> ST & ERR
    STA --> ST & ERR & CFG
    RES --> ST & ERR
    SIG --> ST & ERR & NAM
    TEN --> ST
    ERR --> NAM
    BFT --> CFG & RES & MET
```

---

## 2. Trait Dependency Graph

```mermaid
graph TB
    subgraph Traits["8 Traits Defined in L1"]
        EC["ErrorClassifier<br/>5 methods (2 defaults)<br/>no bounds"]
        CP["ConfigProvider<br/>Send+Sync<br/>5 methods (2 defaults)"]
        CR["CorrelationProvider<br/>Send+Sync<br/>3 methods (1 default)"]
        MR["MetricRecorder<br/>Send+Sync<br/>4 methods (0 defaults)"]
        SS["StateStore<br/>Send+Sync<br/>3 methods (1 default)"]
        RC["ResourceCollector<br/>Send+Sync<br/>5 methods (2 defaults)"]
        SB["SignalSubscriber<br/>Send+Sync+Debug<br/>4 methods (3 defaults)"]
        TC["TensorContributor<br/>Send+Sync+Debug<br/>3 methods (0 defaults)"]
    end

    subgraph Impls["Concrete Implementors in L1"]
        CM["ConfigManager"]
        MReg["MetricsRegistry"]
        DP["DatabasePool"]
        RM["ResourceManager"]
        Err["Error"]
        LC["LogContext"]
    end

    subgraph ArcDyn["Arc&lt;dyn Trait&gt; Boundaries"]
        A1["Arc&lt;dyn SignalSubscriber&gt;<br/>→ stored in SignalBus"]
        A2["Arc&lt;dyn TensorContributor&gt;<br/>→ stored in TensorRegistry"]
        A3["Arc&lt;dyn ConfigProvider&gt;<br/>→ dependency injection"]
    end

    CP --> CM
    MR --> MReg
    SS --> DP
    RC --> RM
    EC --> Err
    CR --> LC

    SB --> A1
    TC --> A2
    CP --> A3
```

---

## 3. Signal Flow Topology

```mermaid
flowchart LR
    subgraph Sources["Signal Emitters (L2-L6)"]
        L2H["M09-M12<br/>Health transitions"]
        L5L["M25-M30<br/>Learning events"]
        L6D["M31-M36<br/>Dissent events"]
    end

    subgraph Bus["SignalBus (M07)"]
        direction TB
        EH["emit_health()"]
        EL["emit_learning()"]
        ED["emit_dissent()"]
        SUBS["subscribers:<br/>Arc&lt;RwLock&lt;Vec&lt;...&gt;&gt;&gt;"]
        STATS["stats:<br/>Arc&lt;RwLock&lt;SignalBusStats&gt;&gt;"]
    end

    subgraph Subscribers["Signal Consumers"]
        S1["L7 Observer<br/>on_health()"]
        S2["L5 Hebbian<br/>on_learning()"]
        S3["L6 Consensus<br/>on_dissent()"]
        S4["L3 Pipeline<br/>on_health()"]
    end

    L2H --> EH
    L5L --> EL
    L6D --> ED

    EH & EL & ED --> SUBS
    SUBS -->|"1. read lock<br/>2. iterate<br/>3. drop guard"| S1 & S2 & S3 & S4
    SUBS -->|"4. write lock stats<br/>(after guard drop)"| STATS
```

**Locking protocol:** Read subscribers → call callbacks → drop guard → write stats. Guards are never held simultaneously (deadlock prevention).

---

## 4. Tensor Composition Pipeline

```mermaid
flowchart TB
    subgraph Contributors["TensorContributor Implementations"]
        M09["M09 ServiceRegistry<br/>D0,D2,D3,D4"]
        M10["M10 HealthMonitor<br/>D6,D10"]
        M11["M11 Lifecycle<br/>D6,D7"]
        M12["M12 Resilience<br/>D9,D10"]
    end

    subgraph Registry["TensorRegistry (M08)"]
        REG["Vec&lt;Arc&lt;dyn TensorContributor&gt;&gt;"]
        COMPOSE["compose()"]
    end

    subgraph Algorithm["Composition Steps"]
        A1["1. call contribute() on each"]
        A2["2. accumulate sum + count per covered dim"]
        A3["3. union all CoverageBitmaps"]
        A4["4. avg = sum/count, clamp [0,1]"]
    end

    subgraph Output["ComposedTensor"]
        T12["Tensor12D<br/>[D0..D11]"]
        COV["CoverageBitmap<br/>8/12 covered"]
        COUNTS["snap=4, stream=0"]
    end

    M09 & M10 & M11 & M12 -->|"Arc&lt;dyn&gt;"| REG
    REG --> COMPOSE --> A1 --> A2 --> A3 --> A4
    A4 --> T12 & COV & COUNTS

    subgraph Freestanding["L1 Freestanding Tensors"]
        CFG_T["Config.to_tensor()<br/>D1,D2,D6"]
        MET_T["MetricSnapshot.to_tensor()<br/>D2,D6,D10"]
        RES_T["SystemResources.to_tensor()<br/>D2,D5,D6,D9,D10"]
    end

    CFG_T & MET_T & RES_T -->|"averaged"| BFT["build_foundation_tensor()"]
```

---

## 5. 12D Tensor Dimension Map

```mermaid
graph LR
    subgraph Active["Active Dimensions (10/12)"]
        D0["D0 ServiceId<br/>M09, LogContext"]
        D1["D1 Port<br/>Config"]
        D2["D2 Tier<br/>All L1: 1/6<br/>M09: avg tier"]
        D3["D3 DependencyCount<br/>M09"]
        D4["D4 AgentCount<br/>M09: healthy"]
        D5["D5 Protocol<br/>LogCtx, Resources"]
        D6["D6 HealthScore<br/>M10+M11 (overlap)<br/>Config, Resources"]
        D7["D7 Uptime<br/>M11"]
        D9["D9 Latency<br/>M12, Resources"]
        D10["D10 ErrorRate<br/>M10+M12 (overlap)<br/>Metrics, Resources"]
    end

    subgraph Unused["Unused Dimensions (2/12)"]
        D8["D8 Synergy<br/>(no contributor)"]
        D11["D11 TemporalContext<br/>(no contributor)"]
    end
```

**D6 overlap:** M10 (probe-based health) + M11 (% running) → averaged. Intentional — complementary views.
**D10 overlap:** M10 (health check error rate) + M12 (circuit breaker failure rate) → averaged. Both valid signals.

---

## 6. Concurrency Model

```mermaid
graph TB
    subgraph Atomic["Atomic Primitives"]
        AT1["GLOBAL_TICK<br/>AtomicU64, Relaxed<br/>Timestamp::now()"]
        AT2["ConfigManager.reload_flag<br/>AtomicBool, SeqCst<br/>SIGHUP handler"]
    end

    subgraph RwLock["parking_lot::RwLock"]
        RW1["ConfigManager.config<br/>RwLock&lt;Config&gt;"]
        RW2["Counter/Gauge/Histogram.values<br/>RwLock&lt;HashMap&lt;Labels, AtomicU64&gt;&gt;"]
        RW3["MetricsRegistry.{counters,gauges,histograms}<br/>RwLock&lt;HashMap&lt;String, Arc&lt;T&gt;&gt;&gt;"]
        RW4["SignalBus.subscribers<br/>Arc&lt;RwLock&lt;Vec&lt;Arc&lt;dyn&gt;&gt;&gt;&gt;"]
        RW5["SignalBus.stats<br/>Arc&lt;RwLock&lt;SignalBusStats&gt;&gt;"]
    end

    subgraph Once["OnceLock"]
        OL1["LOGGING_INITIALIZED<br/>OnceLock&lt;bool&gt;<br/>set-once, init guard"]
    end

    subgraph NoSync["No Internal Sync (caller wraps)"]
        NS1["TensorRegistry<br/>Vec&lt;Arc&lt;dyn&gt;&gt;<br/>&mut self register"]
        NS2["ResourceManager<br/>&mut self methods"]
        NS3["All vocabulary types<br/>Copy/Clone values"]
    end
```

**Lock ordering:** SignalBus always acquires subscribers lock BEFORE stats lock. Guards are dropped between acquisitions (not nested).

---

## 7. Cross-Layer Morphisms

```mermaid
graph TB
    subgraph L1["L1 Foundation Exports"]
        subgraph Core["Core Types"]
            Error
            Timestamp
            ModuleId
            Result["Result&lt;T&gt;"]
        end
        subgraph NAM["NAM Types"]
            AgentOrigin
            Confidence
            Outcome
            LearningSignal
            Dissent
        end
        subgraph Cross["Cross-Cutting"]
            SignalBus
            TensorContributor
            CoverageBitmap
            HealthSignal
        end
    end

    subgraph L2["L2 Services"]
        SR["M09 ServiceRegistry"]
        HM["M10 HealthMonitor"]
        LC["M11 Lifecycle"]
        RS["M12 Resilience"]
    end

    subgraph L3["L3 Core Logic"]
        PL["M13 Pipeline"]
        RM["M14 Remediation"]
        CF["M15 Confidence"]
    end

    subgraph L5["L5 Learning"]
        HB["M25 Hebbian"]
        ST["M26 STDP"]
    end

    subgraph L6["L6 Consensus"]
        PB["M31 PBFT"]
        AG["M32 Agent"]
    end

    Error --> L2 & L3 & L5 & L6
    Timestamp --> L2
    ModuleId --> L2
    SignalBus -->|"Arc&lt;dyn SignalBusOps&gt;"| L2
    TensorContributor -->|"impl on M09-M12"| L2
    HealthSignal --> L2
    AgentOrigin --> L3 & L6
    Confidence --> L3
    Outcome --> L3 & L5
    LearningSignal --> L5
    Dissent --> L6
```

**Direction:** Strictly downward (L7→L1). No L1 module imports from L2+.

---

## 8. Error Code Topology

```mermaid
graph LR
    subgraph Codes["Error Code Ranges"]
        C1000["1000 Config<br/>config.rs, logging.rs"]
        C1100["1100 Database<br/>state.rs"]
        C1200["1200 Network<br/>1201 CircuitOpen<br/>1202 Timeout"]
        C1300["1300 ConsensusQuorum<br/>1301 ViewChange"]
        C1400["1400 PathwayNotFound<br/>1401 TensorValidation"]
        C1500["1500 Validation<br/>config, metrics, resources"]
        C1600["1600 Io<br/>From&lt;std::io::Error&gt;"]
        C1700["1700 Pipeline"]
        C1800["1800 ServiceNotFound<br/>1801 HealthCheckFailed<br/>1802 EscalationRequired"]
        C1900["1900 Other"]
    end

    subgraph Class["ErrorClassifier"]
        RET["is_retryable()"]
        TRA["is_transient()"]
        SEV["severity()"]
    end

    subgraph Tensor["Tensor Signal"]
        TS["to_tensor_signal()<br/>D6=health, D10=error_rate"]
    end

    Codes --> Class
    Codes --> Tensor
```

---

## 9. Builder Pattern Inventory

```mermaid
graph TB
    subgraph Builders["L1 Builder Patterns"]
        B1["ConfigBuilder<br/>→ build() -> Result&lt;Config&gt;<br/>validates ports, log level"]
        B2["Labels<br/>→ fluent .service().layer()...<br/>immutable chaining"]
        B3["QueryBuilder<br/>→ .select().from().where_eq()...<br/>→ build() -> &str"]
        B4["StatePersistenceBuilder<br/>→ .with_database()...<br/>→ build().await -> Result"]
        B5["DatabaseConfig<br/>→ .with_max_connections()...<br/>const fn setters"]
        B6["CoverageBitmap<br/>→ .with_dimension()...<br/>const fn, functional"]
        B7["HealthReport<br/>→ .with_details().with_timestamp()<br/>consuming chain"]
        B8["AnnotatedError<br/>→ .with_origin().with_confidence()<br/>consuming chain"]
        B9["LogContext<br/>→ .with_module().with_agent()<br/>contextual factory"]
        B10["Dissent<br/>→ .with_confidence().with_alternative()<br/>const fn clamp"]
    end
```

All builder setters marked `#[must_use]`. Terminal methods that can fail return `Result`.

---

*L1 Foundation Architectural Schematics v1.0 | 2026-03-01*
