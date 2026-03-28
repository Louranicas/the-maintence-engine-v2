# L2 Services — Meta Tree Mind Map

> **Scope:** L2 ONLY (M09-M12, 4 modules, 5 files) | **LOC:** 7,196 | **Tests:** 320
> **Derived from:** 5 source files + 6 spec sheets | **Date:** 2026-03-01
> **Purpose:** Exhaustive hierarchical decomposition of every type, trait, function, constant,
> pattern, invariant, relationship, and test category within the L2 Services layer.

---

## Root: L2 Services Layer

```
L2 Services
├── Identity
│   ├── LAYER_ID = "L2"
│   ├── MODULE_COUNT = 4 (M09-M12)
│   ├── LOC = 7,196
│   ├── Unit Tests = 279
│   ├── Integration Tests = 41
│   ├── Total Tests = 320
│   ├── Traits = 6 (ServiceDiscovery, HealthMonitoring, LifecycleOps, CircuitBreakerOps, LoadBalancing, TensorContributor)
│   ├── Trait Methods = 65 (all &self)
│   ├── Clippy = 0 warnings (pedantic + nursery)
│   ├── Refactored = 2026-02-28
│   └── Constraints = C1-C10 all PASS
│
├── Files (5) — verified 2026-03-01
│   ├── mod.rs ................. Layer Coordinator (694 LOC, 20 tests)
│   ├── service_registry.rs .... M09 Service Discovery (1,285 LOC, 53 tests)
│   ├── health_monitor.rs ...... M10 Health Monitor (1,130 LOC, 49 tests)
│   ├── lifecycle.rs ........... M11 Lifecycle Manager (1,898 LOC, 75 tests)
│   └── resilience.rs .......... M12 Resilience Manager (2,189 LOC, 82 tests)
│
├── Module Tiers (2)
│   ├── SHARED (zero internal L2 deps — only depends on L1)
│   │   ├── mod.rs ──→ L1: Error, Timestamp, Duration, ModuleId, Tensor12D
│   │   ├── service_registry.rs ──→ L1: Error, Timestamp, SignalBus, TensorContributor, MetricsRegistry
│   │   ├── health_monitor.rs ──→ L1: Error, Timestamp, SignalBus, TensorContributor, MetricsRegistry
│   │   └── lifecycle.rs ──→ L1: Error, Timestamp, Duration, SignalBus, TensorContributor, MetricsRegistry
│   └── COMPOSITE (depends on L1 only — no cross-deps within L2)
│       └── resilience.rs ──→ L1: Error, Timestamp, Duration, Instant, SignalBus, TensorContributor, MetricsRegistry
│
├── Constraint Compliance Matrix (C1-C10)
│   ├── C1  No upward imports (L2→L1 only) ................ PASS
│   ├── C2  All trait methods &self ........................ PASS (65 methods)
│   ├── C3  TensorContributor impl on every manager ........ PASS (4/4)
│   ├── C4  Zero tolerance (0 unsafe, 0 unwrap, 0 expect) . PASS
│   ├── C5  No chrono/SystemTime — only Timestamp/Duration . PASS
│   ├── C6  Signal emission via Arc<SignalBus> ............. PASS
│   ├── C7  Owned returns through RwLock ................... PASS (all cloned)
│   ├── C8  Duration not milliseconds ...................... PASS (std::time::Duration)
│   ├── C9  Backward compat via re-exports ................. PASS (mod.rs re-exports all)
│   └── C10 280+ tests .................................... PASS (320 total)
│
├── Traits (6)
│   │
│   ├── [1] ServiceDiscovery (service_registry.rs)
│   │   ├── Bounds: Send + Sync + fmt::Debug
│   │   ├── Methods (14, 0 defaults)
│   │   │   ├── register(&self, def: ServiceDefinition) -> Result<()>
│   │   │   ├── deregister(&self, service_id: &str) -> Result<()>
│   │   │   ├── discover(&self, service_id: &str) -> Result<ServiceDefinition> [C7: owned]
│   │   │   ├── discover_by_tier(&self, tier: ServiceTier) -> Vec<ServiceDefinition> [C7: owned]
│   │   │   ├── discover_by_protocol(&self, protocol: &str) -> Vec<ServiceDefinition> [C7: owned, case-insensitive]
│   │   │   ├── list_services(&self) -> Vec<ServiceDefinition> [C7: owned]
│   │   │   ├── update_health(&self, service_id: &str, status: HealthStatus) -> Result<()> [C6: emits signal]
│   │   │   ├── get_health(&self, service_id: &str) -> Result<HealthStatus> [C7: owned]
│   │   │   ├── get_healthy_services(&self) -> Vec<ServiceDefinition> [C7: owned]
│   │   │   ├── add_dependency(&self, from: &str, to: &str) -> Result<()> [validates no self-dep]
│   │   │   ├── get_dependencies(&self, service_id: &str) -> Result<Vec<String>> [C7: owned]
│   │   │   ├── get_dependents(&self, service_id: &str) -> Result<Vec<String>> [C7: owned, reverse]
│   │   │   ├── service_count(&self) -> usize
│   │   │   └── is_registered(&self, service_id: &str) -> bool
│   │   ├── Implementor: ServiceRegistry
│   │   ├── Object Safety: YES (compile-tested)
│   │   └── Arc Boundary: Arc<dyn ServiceDiscovery> for DI
│   │
│   ├── [2] HealthMonitoring (health_monitor.rs)
│   │   ├── Bounds: Send + Sync + fmt::Debug
│   │   ├── Methods (11, 0 defaults)
│   │   │   ├── register_probe(&self, probe: HealthProbe) -> Result<()>
│   │   │   ├── unregister_probe(&self, service_id: &str) -> Result<()>
│   │   │   ├── probe_count(&self) -> usize
│   │   │   ├── record_result(&self, service_id: &str, result: HealthCheckResult) -> Result<()> [C6: emits signal]
│   │   │   ├── get_status(&self, service_id: &str) -> Result<HealthStatus> [C7: owned]
│   │   │   ├── get_history(&self, service_id: &str) -> Result<Vec<HealthCheckResult>> [C7: owned]
│   │   │   ├── get_all_statuses(&self) -> HashMap<String, HealthStatus> [C7: owned]
│   │   │   ├── aggregate_health(&self) -> f64 [0.0-1.0, weighted avg]
│   │   │   ├── get_degraded_services(&self) -> Vec<String>
│   │   │   ├── get_unhealthy_services(&self) -> Vec<String>
│   │   │   └── get_healthy_services(&self) -> Vec<String>
│   │   ├── Implementor: HealthMonitor
│   │   ├── Object Safety: YES (compile-tested)
│   │   └── Arc Boundary: Arc<dyn HealthMonitoring> for DI
│   │
│   ├── [3] LifecycleOps (lifecycle.rs)
│   │   ├── Bounds: Send + Sync + fmt::Debug
│   │   ├── Methods (18, 0 defaults)
│   │   │   ├── register(&self, service_id: &str, name: &str, tier: ServiceTier, config: RestartConfig) -> Result<()>
│   │   │   ├── deregister(&self, service_id: &str) -> Result<()>
│   │   │   ├── start_service(&self, service_id: &str) -> Result<()> [Stopped|Failed → Starting]
│   │   │   ├── mark_running(&self, service_id: &str) -> Result<()> [Starting → Running]
│   │   │   ├── mark_failed(&self, service_id: &str) -> Result<()> [Starting|Running → Failed]
│   │   │   ├── stop_service(&self, service_id: &str) -> Result<()> [Running → Stopping]
│   │   │   ├── mark_stopped(&self, service_id: &str) -> Result<()> [Stopping → Stopped, resets restarts]
│   │   │   ├── restart_service(&self, service_id: &str) -> Result<Duration> [Running|Failed → Starting, +backoff]
│   │   │   ├── get_status(&self, service_id: &str) -> Result<ServiceStatus> [C7: owned]
│   │   │   ├── get_entry(&self, service_id: &str) -> Result<LifecycleEntry> [C7: owned clone]
│   │   │   ├── get_history(&self, service_id: &str) -> Result<Vec<LifecycleTransition>> [C7: owned]
│   │   │   ├── can_restart(&self, service_id: &str) -> Result<bool>
│   │   │   ├── get_restart_backoff(&self, service_id: &str) -> Result<Duration>
│   │   │   ├── is_registered(&self, service_id: &str) -> bool
│   │   │   ├── service_count(&self) -> usize
│   │   │   ├── get_all_running(&self) -> Vec<String>
│   │   │   ├── get_all_failed(&self) -> Vec<String>
│   │   │   └── reset_restart_count(&self, service_id: &str) -> Result<()>
│   │   ├── Implementor: LifecycleManager
│   │   ├── Object Safety: YES (compile-tested)
│   │   └── Arc Boundary: Arc<dyn LifecycleOps> for DI
│   │
│   ├── [4] CircuitBreakerOps (resilience.rs)
│   │   ├── Bounds: Send + Sync + fmt::Debug
│   │   ├── Methods (12, 0 defaults)
│   │   │   ├── register_breaker(&self, service_id: &str, config: CircuitBreakerConfig) -> Result<()>
│   │   │   ├── register_default(&self, service_id: &str) -> Result<()>
│   │   │   ├── deregister_breaker(&self, service_id: &str) -> Result<()>
│   │   │   ├── record_success(&self, service_id: &str) -> Result<CircuitState> [C6: emits signal on transition]
│   │   │   ├── record_failure(&self, service_id: &str) -> Result<CircuitState> [C6: emits signal on transition]
│   │   │   ├── allow_request(&self, service_id: &str) -> Result<bool> [Open→HalfOpen after timeout]
│   │   │   ├── get_state(&self, service_id: &str) -> Result<CircuitState>
│   │   │   ├── get_breaker_stats(&self, service_id: &str) -> Result<CircuitBreakerStats> [C7: owned]
│   │   │   ├── reset(&self, service_id: &str) -> Result<()> [force Closed]
│   │   │   ├── get_open_circuits(&self) -> Vec<String>
│   │   │   ├── breaker_count(&self) -> usize
│   │   │   └── is_registered(&self, service_id: &str) -> bool
│   │   ├── Implementor: CircuitBreakerRegistry
│   │   ├── Object Safety: YES (compile-tested)
│   │   └── Arc Boundary: Arc<dyn CircuitBreakerOps> for DI
│   │
│   ├── [5] LoadBalancing (resilience.rs)
│   │   ├── Bounds: Send + Sync + fmt::Debug
│   │   ├── Methods (10, 0 defaults)
│   │   │   ├── create_pool(&self, service_id: &str, algorithm: LoadBalanceAlgorithm) -> Result<()>
│   │   │   ├── remove_pool(&self, service_id: &str) -> Result<()>
│   │   │   ├── add_endpoint(&self, service_id: &str, endpoint: Endpoint) -> Result<()> [dup check]
│   │   │   ├── remove_endpoint(&self, service_id: &str, endpoint_id: &str) -> Result<()>
│   │   │   ├── select_endpoint(&self, service_id: &str) -> Result<Endpoint> [C7: owned, +active_connections]
│   │   │   ├── mark_healthy(&self, service_id: &str, endpoint_id: &str) -> Result<()>
│   │   │   ├── mark_unhealthy(&self, service_id: &str, endpoint_id: &str) -> Result<()>
│   │   │   ├── record_request(&self, service_id: &str, endpoint_id: &str, success: bool) -> Result<()> [-active_connections]
│   │   │   ├── get_pool_stats(&self, service_id: &str) -> Result<PoolStats> [C7: owned]
│   │   │   └── get_load_distribution(&self, service_id: &str) -> Result<Vec<(String, f64)>> [C7: owned]
│   │   ├── Implementor: LoadBalancer
│   │   ├── Object Safety: YES (compile-tested)
│   │   └── Arc Boundary: Arc<dyn LoadBalancing> for DI
│   │
│   └── [6] TensorContributor (from L1, implemented by all 4 managers)
│       ├── Bounds: Send + Sync + Debug
│       ├── Methods (3, 0 defaults)
│       │   ├── contribute(&self) -> ContributedTensor
│       │   ├── contributor_kind(&self) -> ContributorKind
│       │   └── module_id(&self) -> &str
│       ├── Implementors
│       │   ├── ServiceRegistry ──→ D0, D2, D3, D4
│       │   ├── HealthMonitor ──→ D6, D10
│       │   ├── LifecycleManager ──→ D6, D7
│       │   └── ResilienceManager ──→ D9, D10
│       └── Object Safety: YES (via L1)
│
├── Types — Shared Enums (mod.rs)
│   │
│   ├── ServiceStatus
│   │   ├── Kind: enum
│   │   ├── Derives: Clone, Copy, Debug, PartialEq, Eq, Hash, Default
│   │   ├── Variants (5)
│   │   │   ├── Stopped [#[default]] ......... health_score = 0.0
│   │   │   ├── Starting .................... health_score = 0.5
│   │   │   ├── Running ..................... health_score = 1.0
│   │   │   ├── Stopping .................... health_score = 0.5
│   │   │   └── Failed ...................... health_score = 0.0
│   │   ├── Methods
│   │   │   ├── as_str(&self) -> &'static str [const fn]
│   │   │   └── is_operational(&self) -> bool [const fn, true for Running only]
│   │   └── Traits: Display (via as_str)
│   │
│   ├── HealthStatus
│   │   ├── Kind: enum
│   │   ├── Derives: Clone, Copy, Debug, PartialEq, Eq, Hash, Default
│   │   ├── Variants (4)
│   │   │   ├── Healthy [#[default]] ......... score = 1.0
│   │   │   ├── Degraded .................... score = 0.5
│   │   │   ├── Unhealthy ................... score = 0.0
│   │   │   └── Unknown ..................... score = 0.0
│   │   ├── Methods
│   │   │   ├── as_str(&self) -> &'static str [const fn]
│   │   │   └── score(&self) -> f64 [const fn, quantized: 1.0/0.5/0.0]
│   │   └── Traits: Display (via as_str)
│   │
│   ├── ServiceTier
│   │   ├── Kind: enum
│   │   ├── Derives: Clone, Copy, Debug, PartialEq, Eq, Hash
│   │   ├── Variants (5)
│   │   │   ├── Tier1 ......... weight = 1.5, normalized = 1/6
│   │   │   ├── Tier2 ......... weight = 1.3, normalized = 2/6
│   │   │   ├── Tier3 ......... weight = 1.2, normalized = 3/6
│   │   │   ├── Tier4 ......... weight = 1.1, normalized = 4/6
│   │   │   └── Tier5 ......... weight = 1.0 [#[default]], normalized = 5/6
│   │   ├── Methods
│   │   │   ├── weight(&self) -> f64 [const fn]
│   │   │   ├── number(&self) -> u8 [1-5]
│   │   │   └── normalized(&self) -> f64 [tier_number / 6.0, for tensor D2]
│   │   └── Traits: Display
│   │
│   └── CircuitState
│       ├── Kind: enum
│       ├── Derives: Clone, Copy, Debug, PartialEq, Eq, Hash, Default
│       ├── Variants (3)
│       │   ├── Closed [#[default]] ......... normal flow, score = 1.0
│       │   ├── Open ........................ reject all, score = 0.0
│       │   └── HalfOpen .................... probing, score = 0.5
│       ├── Methods
│       │   └── as_str(&self) -> &'static str [const fn]
│       └── Traits: Display (via as_str)
│
├── Types — Shared Structs (mod.rs)
│   │
│   ├── ServiceState
│   │   ├── Derives: Clone, Debug, Default
│   │   ├── Fields (16)
│   │   │   ├── id: String
│   │   │   ├── name: String
│   │   │   ├── status: ServiceStatus
│   │   │   ├── health_status: HealthStatus
│   │   │   ├── tier: ServiceTier
│   │   │   ├── port: u16
│   │   │   ├── pid: Option<u32>
│   │   │   ├── health_score: f64 .......... [0.0, 1.0]
│   │   │   ├── synergy_score: f64 ......... [0.0, 1.0]
│   │   │   ├── cpu_percent: f64
│   │   │   ├── memory_mb: f64
│   │   │   ├── uptime_seconds: u64
│   │   │   ├── restart_count: u32
│   │   │   ├── last_health_check: Timestamp [C5]
│   │   │   ├── module_id: ModuleId
│   │   │   └── tensor: Tensor12D
│   │   ├── Methods
│   │   │   ├── new() -> Self
│   │   │   ├── weighted_health(&self) -> f64 [health_score * tier.weight()]
│   │   │   ├── update_tensor(&mut self) [recomputes tensor from fields]
│   │   │   ├── is_operational(&self) -> bool [status == Running]
│   │   │   └── hash_to_float(s: &str) -> f64 [deterministic [0.0, 1.0], for tensor D0]
│   │   └── Traits: Display
│   │
│   ├── ServicesStatus
│   │   ├── Derives: Debug, Clone, PartialEq, Default
│   │   ├── Fields (8)
│   │   │   ├── layer_id: &'static str .......... "L2"
│   │   │   ├── module_count: usize .............. 4
│   │   │   ├── registered_services: usize
│   │   │   ├── healthy_services: usize
│   │   │   ├── running_services: usize
│   │   │   ├── open_circuits: usize
│   │   │   ├── health_score: f64
│   │   │   └── tensor: Tensor12D
│   │   └── Traits: Display ("{layer_id}: {healthy}/{registered} healthy, {open_circuits} open circuits")
│   │
│   └── RestartConfig
│       ├── Derives: Clone, Copy, Debug, PartialEq, Eq, Default
│       ├── Fields (3)
│       │   ├── max_restarts: u32 ................ default = 5
│       │   ├── initial_backoff: Duration ........ default = 1s [C8]
│       │   └── max_backoff: Duration ............ default = 30s [C8]
│       ├── Methods
│       │   ├── new() -> Self [const fn]
│       │   └── default() -> Self [same as new()]
│       └── Note: Copy because all fields are Copy
│
├── Types — Service Registry (M09 service_registry.rs)
│   │
│   ├── ServiceDefinition
│   │   ├── Derives: Clone, Debug
│   │   ├── Fields (12)
│   │   │   ├── service_id: String
│   │   │   ├── name: String
│   │   │   ├── version: String .............. default = "1.0.0"
│   │   │   ├── tier: ServiceTier ............ default = Tier5
│   │   │   ├── host: String ................. default = "127.0.0.1"
│   │   │   ├── port: u16 ................... default = 0
│   │   │   ├── protocol: String ............. default = "REST"
│   │   │   ├── health_path: String .......... default = "/health"
│   │   │   ├── metadata: HashMap<String, String>
│   │   │   ├── registered_at: Timestamp ..... C5
│   │   │   ├── ttl_seconds: Option<u64> ..... hint only, not auto-enforced
│   │   │   └── module_id: Option<ModuleId>
│   │   └── Traits: Display ("{name} ({service_id}) v{version} @ {host}:{port} [{tier}]")
│   │
│   ├── ServiceDefinitionBuilder
│   │   ├── Terminal: build(self) -> ServiceDefinition
│   │   ├── Setters [all consuming self]
│   │   │   ├── service_id(impl Into<String>)
│   │   │   ├── name(impl Into<String>)
│   │   │   ├── version(impl Into<String>)
│   │   │   ├── tier(ServiceTier)
│   │   │   ├── host(impl Into<String>)
│   │   │   ├── port(u16)
│   │   │   ├── protocol(impl Into<String>)
│   │   │   ├── health_path(impl Into<String>)
│   │   │   ├── metadata(key, value)
│   │   │   ├── ttl(u64)
│   │   │   └── module_id(ModuleId)
│   │   └── Validation: None (all fields have defaults)
│   │
│   ├── ServiceRegistry
│   │   ├── Internal State
│   │   │   ├── state: RwLock<RegistryState>
│   │   │   │   ├── services: HashMap<String, ServiceDefinition>
│   │   │   │   ├── health_map: HashMap<String, HealthStatus>
│   │   │   │   └── dependencies: HashMap<String, Vec<String>> [adjacency list]
│   │   │   ├── signal_bus: Option<Arc<SignalBus>>
│   │   │   └── metrics: Option<Arc<MetricsRegistry>>
│   │   ├── Constructors
│   │   │   ├── new() -> Self
│   │   │   ├── default() -> Self [same as new()]
│   │   │   ├── with_signal_bus(Arc<SignalBus>) -> Self
│   │   │   └── with_metrics(Arc<MetricsRegistry>) -> Self
│   │   ├── Implements: ServiceDiscovery (14 methods)
│   │   └── Implements: TensorContributor ──→ D0, D2, D3, D4
│   │
│   └── Free Functions
│       ├── register_service() [internal helper]
│       └── register_ultraplate_services(registry: &dyn ServiceDiscovery) -> Result<()>
│           ├── Registers 12 canonical ULTRAPLATE services
│           ├── Tier distribution: 2×Tier1, 3×Tier2, 3×Tier3, 3×Tier4, 1×Tier5
│           └── Sets correct ports, health_paths (/api/health for ME+SYNTHEX, /health for rest)
│
├── Types — Health Monitor (M10 health_monitor.rs)
│   │
│   ├── HealthProbe
│   │   ├── Derives: Clone, Debug
│   │   ├── Fields (6)
│   │   │   ├── service_id: String
│   │   │   ├── endpoint: String ............. e.g. "http://localhost:8090/api/health"
│   │   │   ├── interval_ms: u64
│   │   │   ├── timeout_ms: u64
│   │   │   ├── healthy_threshold: u32 ....... consecutive successes for Unknown→Healthy
│   │   │   └── unhealthy_threshold: u32 ..... consecutive failures for Unknown/Degraded→Unhealthy
│   │   └── Traits: Display ("Probe({service_id} @ {endpoint}, interval={interval_ms}ms)")
│   │
│   ├── HealthProbeBuilder
│   │   ├── Terminal: build(self) -> Result<HealthProbe>
│   │   ├── Setters: service_id, endpoint, interval_ms, timeout_ms, healthy_threshold, unhealthy_threshold
│   │   └── Validation
│   │       ├── healthy_threshold > 0
│   │       ├── unhealthy_threshold > 0
│   │       └── timeout_ms <= interval_ms
│   │
│   ├── HealthCheckResult
│   │   ├── Derives: Clone, Debug
│   │   ├── Fields (6)
│   │   │   ├── service_id: String
│   │   │   ├── status: HealthStatus
│   │   │   ├── response_time_ms: u64
│   │   │   ├── timestamp: Timestamp ......... C5
│   │   │   ├── message: Option<String>
│   │   │   └── status_code: Option<u16>
│   │   ├── Factories
│   │   │   ├── success(service_id, response_time_ms) -> Self
│   │   │   └── failure(service_id, message) -> Self
│   │   ├── Methods
│   │   │   └── is_success(&self) -> bool
│   │   └── Traits: Display ("HealthCheck({service_id}: {status}, {response_time_ms}ms)")
│   │
│   └── HealthMonitor
│       ├── Internal State
│       │   ├── state: RwLock<MonitorState>
│       │   │   ├── services: HashMap<String, ServiceHealthState>
│       │   │   │   ├── probe: HealthProbe
│       │   │   │   ├── current_status: HealthStatus [FSM state]
│       │   │   │   ├── consecutive_successes: u32
│       │   │   │   ├── consecutive_failures: u32
│       │   │   │   └── history: Vec<HealthCheckResult> [ring buffer, max 100]
│       │   │   └── max_history: usize [default: 100]
│       │   ├── signal_bus: Option<Arc<SignalBus>>
│       │   └── metrics: Option<Arc<MetricsRegistry>>
│       ├── Constructors
│       │   ├── new() / default() ──→ max_history = 100
│       │   ├── with_max_history(usize) -> Self
│       │   ├── with_signal_bus(Arc<SignalBus>) -> Self
│       │   └── with_metrics(Arc<MetricsRegistry>) -> Self
│       ├── Implements: HealthMonitoring (11 methods)
│       ├── Implements: TensorContributor ──→ D6, D10
│       └── Aggregation: Σ(status.score()) / probe_count [0.0 if empty]
│
├── Types — Lifecycle Manager (M11 lifecycle.rs)
│   │
│   ├── LifecycleAction (enum, 4 variants)
│   │   ├── Derives: Clone, Debug
│   │   ├── Variants
│   │   │   ├── Start { service_id: String }
│   │   │   ├── Stop { service_id: String, graceful: bool }
│   │   │   ├── Restart { service_id: String, reason: String }
│   │   │   └── HealthCheck { service_id: String }
│   │   └── Traits: Display
│   │
│   ├── LifecycleTransition
│   │   ├── Derives: Clone, Debug
│   │   ├── Fields (4)
│   │   │   ├── from: ServiceStatus
│   │   │   ├── to: ServiceStatus
│   │   │   ├── reason: String
│   │   │   └── timestamp: Timestamp [C5]
│   │   └── Traits: Display ("Transition: {from} → {to} ({reason})")
│   │
│   ├── LifecycleEntry
│   │   ├── Derives: Clone, Debug
│   │   ├── Fields (11)
│   │   │   ├── service_id: String
│   │   │   ├── name: String
│   │   │   ├── tier: ServiceTier
│   │   │   ├── current_state: ServiceStatus
│   │   │   ├── previous_state: Option<ServiceStatus>
│   │   │   ├── transition_history: Vec<LifecycleTransition>
│   │   │   ├── restart_count: u32
│   │   │   ├── config: RestartConfig
│   │   │   ├── current_backoff: Duration [C8]
│   │   │   ├── created_at: Timestamp [C5]
│   │   │   └── last_transition: Timestamp [C5]
│   │   └── Traits: Display ("{name} ({service_id}): {current_state} [restarts: {restart_count}/{max}]")
│   │
│   ├── LifecycleEntryBuilder
│   │   ├── Terminal: build(self) -> LifecycleEntry
│   │   ├── Setters: service_id, name, tier, config
│   │   └── Validation: None
│   │
│   ├── LifecycleManager
│   │   ├── Internal State
│   │   │   ├── services: RwLock<HashMap<String, LifecycleEntry>>
│   │   │   ├── signal_bus: Option<Arc<SignalBus>>
│   │   │   └── metrics: Option<Arc<MetricsRegistry>>
│   │   ├── Constructors
│   │   │   ├── new() / default()
│   │   │   ├── with_signal_bus(Arc<SignalBus>) -> Self
│   │   │   └── with_signal_bus_and_metrics(Arc<SignalBus>, Arc<MetricsRegistry>) -> Self
│   │   ├── Implements: LifecycleOps (18 methods)
│   │   └── Implements: TensorContributor ──→ D6, D7
│   │
│   └── Helper Functions
│       ├── is_valid_transition(from, to) -> bool [const fn, compile-time FSM]
│       │   ├── Valid: Stopped→Starting, Starting→Running, Starting→Failed
│       │   ├── Valid: Running→Stopping, Running→Failed
│       │   ├── Valid: Stopping→Stopped, Failed→Starting
│       │   └── All other combinations → false
│       └── status_health_score(status) -> f64 [const fn]
│           ├── Running = 1.0
│           ├── Starting | Stopping = 0.5
│           └── Stopped | Failed = 0.0
│
├── Types — Resilience (M12 resilience.rs)
│   │
│   ├── LoadBalanceAlgorithm (enum, 4 variants)
│   │   ├── Derives: Clone, Copy, Debug, PartialEq, Eq
│   │   ├── Variants
│   │   │   ├── RoundRobin .............. sequential rotation through healthy endpoints
│   │   │   ├── WeightedRoundRobin ...... cumulative weight distribution
│   │   │   ├── LeastConnections ........ min active_connections (first on tie)
│   │   │   └── Random .................. deterministic LCG hash (reproducible)
│   │   └── Traits: Display
│   │
│   ├── CircuitBreakerConfig
│   │   ├── Derives: Clone, Debug, Default
│   │   ├── Fields (5)
│   │   │   ├── failure_threshold: u32 ........... default = 5
│   │   │   ├── success_threshold: u32 ........... default = 3
│   │   │   ├── open_timeout: Duration ........... default = 30s [C8]
│   │   │   ├── half_open_max_requests: u32 ...... default = 1
│   │   │   └── monitoring_window: Duration ...... default = 60s [C8]
│   │   └── Builder: CircuitBreakerConfigBuilder (fluent API)
│   │
│   ├── CircuitBreakerStats
│   │   ├── Derives: Clone, Debug
│   │   ├── Fields (8)
│   │   │   ├── state: CircuitState
│   │   │   ├── failure_count: u32
│   │   │   ├── success_count: u32
│   │   │   ├── total_requests: u64
│   │   │   ├── total_failures: u64
│   │   │   ├── failure_rate: f64 ................ [0.0, 1.0]
│   │   │   ├── last_failure: Option<Timestamp> .. C5
│   │   │   └── last_state_change: Timestamp ..... C5
│   │   └── Note: Snapshot type — immutable once returned
│   │
│   ├── CircuitStateTransition
│   │   ├── Derives: Clone, Debug
│   │   ├── Fields: from (CircuitState), to (CircuitState), reason (String), timestamp (Timestamp)
│   │   └── Traits: Display ("Circuit: {from} → {to} ({reason})")
│   │
│   ├── Endpoint
│   │   ├── Derives: Clone, Debug
│   │   ├── Fields (8)
│   │   │   ├── id: String
│   │   │   ├── host: String
│   │   │   ├── port: u16
│   │   │   ├── weight: f64 ..................... [0.0, 1.0] clamped on construction
│   │   │   ├── active_connections: u32
│   │   │   ├── healthy: bool ................... default = true
│   │   │   ├── total_requests: u64
│   │   │   └── total_errors: u64
│   │   ├── Methods
│   │   │   ├── new(id, host, port, weight) -> Self [clamps weight]
│   │   │   └── error_rate(&self) -> f64 [errors/requests, 0.0 if no requests]
│   │   └── Traits: Display ("{id} @ {host}:{port} (weight={weight}, conns={active_connections})")
│   │
│   ├── PoolStats
│   │   ├── Derives: Clone, Debug, Default
│   │   ├── Fields: total_endpoints, healthy_endpoints, total_requests, total_errors, error_rate
│   │   └── Note: Aggregated snapshot across all endpoints in a pool
│   │
│   ├── CircuitBreakerRegistry
│   │   ├── Internal State
│   │   │   ├── breakers: RwLock<HashMap<String, CircuitBreakerEntry>>
│   │   │   │   ├── config: CircuitBreakerConfig
│   │   │   │   ├── state: CircuitState
│   │   │   │   ├── failure_count: u32
│   │   │   │   ├── success_count: u32
│   │   │   │   ├── consecutive_successes: u32 [for HalfOpen→Closed]
│   │   │   │   ├── total_requests: u64
│   │   │   │   ├── total_failures: u64
│   │   │   │   ├── last_failure_time: Option<Timestamp> [C5]
│   │   │   │   ├── last_success_time: Option<Timestamp> [C5]
│   │   │   │   ├── last_state_change: Timestamp [C5, wall clock]
│   │   │   │   ├── state_change_instant: Instant [C8, monotonic]
│   │   │   │   └── state_history: Vec<CircuitStateTransition>
│   │   │   ├── signal_bus: Option<Arc<SignalBus>>
│   │   │   └── metrics: Option<Arc<MetricsRegistry>>
│   │   ├── Constructors
│   │   │   ├── new() / default()
│   │   │   ├── with_signal_bus(Arc<SignalBus>) -> Self
│   │   │   └── with_signal_bus_and_metrics(bus, metrics) -> Self
│   │   ├── Helpers
│   │   │   ├── closed_fraction(&self) -> f64 [(total - open) / total]
│   │   │   └── average_failure_rate(&self) -> f64 [mean across all breakers]
│   │   └── Implements: CircuitBreakerOps (12 methods)
│   │
│   ├── LoadBalancer
│   │   ├── Internal State
│   │   │   ├── pools: RwLock<HashMap<String, EndpointPool>>
│   │   │   │   ├── service_id: String
│   │   │   │   ├── endpoints: Vec<Endpoint>
│   │   │   │   ├── algorithm: LoadBalanceAlgorithm
│   │   │   │   ├── current_index: usize [RoundRobin state]
│   │   │   │   └── selection_counter: u64 [Weighted/Random state]
│   │   │   └── metrics: Option<Arc<MetricsRegistry>>
│   │   ├── Constructors: new() / default() / with_metrics()
│   │   ├── Selection Algorithms
│   │   │   ├── RoundRobin: healthy_indices[current_index % len], ++current_index
│   │   │   ├── WeightedRoundRobin: cumsum(weights), target = counter % total, first exceeding
│   │   │   ├── LeastConnections: min_by(active_connections), first on tie
│   │   │   └── Random: LCG hash = (counter * 6364136223846793005 + 1442695040888963407) >> 33
│   │   └── Implements: LoadBalancing (10 methods)
│   │
│   └── ResilienceManager (facade)
│       ├── Derives: Debug, Default
│       ├── Fields
│       │   ├── circuit_breakers: CircuitBreakerRegistry
│       │   └── load_balancer: LoadBalancer
│       ├── Constructors
│       │   ├── new() / default()
│       │   ├── with_signal_bus(Arc<SignalBus>) -> Self
│       │   └── with_signal_bus_and_metrics(bus, metrics) -> Self
│       ├── Accessors
│       │   ├── circuit_breakers(&self) -> &CircuitBreakerRegistry [const fn]
│       │   └── load_balancer(&self) -> &LoadBalancer [const fn]
│       └── Implements: TensorContributor ──→ D9, D10
│
├── Finite State Machines (3)
│   │
│   ├── [FSM-1] Service Lifecycle (M11, 5 states, 7 valid transitions)
│   │   ├── States: Stopped, Starting, Running, Stopping, Failed
│   │   ├── Transitions
│   │   │   ├── Stopped → Starting ........... start_service()
│   │   │   ├── Starting → Running ........... mark_running()
│   │   │   ├── Starting → Failed ............ mark_failed()
│   │   │   ├── Running → Stopping ........... stop_service()
│   │   │   ├── Running → Failed ............. mark_failed()
│   │   │   ├── Stopping → Stopped ........... mark_stopped() [resets restart_count]
│   │   │   └── Failed → Starting ............ start_service() or restart_service()
│   │   ├── Restart Transition: Running|Failed → Starting
│   │   │   ├── restart_count += 1
│   │   │   ├── current_backoff = min(current_backoff * 2, max_backoff)
│   │   │   └── Returns: Duration (current backoff)
│   │   ├── Backoff Sequence (defaults): 1s, 2s, 4s, 8s, 16s, REJECT
│   │   ├── Validation: is_valid_transition() [const fn, 7 matches]
│   │   └── Health Scores: Running=1.0, Starting|Stopping=0.5, Stopped|Failed=0.0
│   │
│   ├── [FSM-2] Circuit Breaker (M12, 3 states)
│   │   ├── States: Closed, Open, HalfOpen
│   │   ├── Transitions
│   │   │   ├── Closed → Open ................ failure_count >= failure_threshold (default 5)
│   │   │   ├── Closed → Closed .............. failure below threshold OR success
│   │   │   ├── Open → HalfOpen .............. allow_request() + open_timeout elapsed (30s)
│   │   │   ├── Open → Open .................. allow_request() + timeout not elapsed (DENY)
│   │   │   ├── HalfOpen → Closed ............ consecutive_successes >= success_threshold (default 3)
│   │   │   ├── HalfOpen → HalfOpen .......... success below threshold
│   │   │   ├── HalfOpen → Open .............. any failure (immediate trip back)
│   │   │   └── * → Closed ................... reset() [force close]
│   │   ├── Two-Clock Strategy
│   │   │   ├── Timestamp (C5) ............... wall clock for history/stats/logging
│   │   │   └── Instant (C8) ................ monotonic for timeout (immune to NTP)
│   │   └── Signal Scores: Closed=1.0, HalfOpen=0.5, Open=0.0
│   │
│   └── [FSM-3] Health Monitor (M10, 4 states)
│       ├── States: Unknown, Healthy, Degraded, Unhealthy
│       ├── Transitions
│       │   ├── Unknown → Healthy ............ consecutive_successes >= healthy_threshold
│       │   ├── Unknown → Unhealthy .......... consecutive_failures >= unhealthy_threshold
│       │   ├── Healthy → Degraded ........... single failure (fast detection)
│       │   ├── Degraded → Unhealthy ......... consecutive_failures >= unhealthy_threshold
│       │   ├── Degraded → Healthy ........... consecutive_successes >= healthy_threshold
│       │   └── Unhealthy → Healthy .......... consecutive_successes >= healthy_threshold
│       ├── Counter Reset (hysteresis prevention)
│       │   ├── On success: consecutive_failures = 0, consecutive_successes += 1
│       │   └── On failure: consecutive_successes = 0, consecutive_failures += 1
│       ├── Health Scores: Healthy=1.0, Degraded=0.5, Unhealthy|Unknown=0.0
│       └── Design: Degraded prevents flapping (single failure ≠ unhealthy)
│
├── Re-exports (mod.rs ──→ downstream)
│   │
│   ├── From service_registry (4)
│   │   └── ServiceDefinition, ServiceDefinitionBuilder, ServiceDiscovery, ServiceRegistry
│   │
│   ├── From health_monitor (5)
│   │   └── HealthCheckResult, HealthMonitor, HealthMonitoring, HealthProbe, HealthProbeBuilder
│   │
│   ├── From lifecycle (6)
│   │   └── LifecycleAction, LifecycleEntry, LifecycleEntryBuilder, LifecycleManager, LifecycleOps, LifecycleTransition
│   │
│   └── From resilience (11)
│       └── CircuitBreakerConfig, CircuitBreakerConfigBuilder, CircuitBreakerOps
│       └── CircuitBreakerRegistry, CircuitBreakerStats, CircuitStateTransition
│       └── Endpoint, LoadBalanceAlgorithm, LoadBalancer, LoadBalancing
│       └── PoolStats, ResilienceManager
│
├── Concurrency Model
│   │
│   ├── parking_lot::RwLock (5 instances)
│   │   ├── ServiceRegistry.state: RwLock<RegistryState>
│   │   ├── HealthMonitor.state: RwLock<MonitorState>
│   │   ├── LifecycleManager.services: RwLock<HashMap<String, LifecycleEntry>>
│   │   ├── CircuitBreakerRegistry.breakers: RwLock<HashMap<String, CircuitBreakerEntry>>
│   │   └── LoadBalancer.pools: RwLock<HashMap<String, EndpointPool>>
│   │
│   ├── No std::sync::RwLock — all parking_lot (no poisoning, faster)
│   │
│   ├── No Nested Locks — each manager has exactly 1 RwLock
│   │
│   ├── Lock Protocol
│   │   ├── Read Path: state.read() → clone data → release → return owned
│   │   ├── Write Path: state.write() → mutate → compute signal → release → emit signal
│   │   └── Signal After Release: emit happens AFTER lock drop (deadlock prevention)
│   │
│   ├── Two Independent Locks (M12)
│   │   ├── CircuitBreakerRegistry lock — circuit breaker operations
│   │   └── LoadBalancer lock — load balancing operations
│   │   └── Never contend — different subsystems
│   │
│   └── Thread Safety
│       ├── All managers: Send + Sync [verified by tests]
│       ├── All traits: object-safe [verified by fn _assert(&dyn Trait)]
│       └── All Arc<dyn Trait> compatible [verified by tests]
│
├── 12D Tensor Dimension Map
│   │
│   ├── Covered by L2 (8/12 = 67%)
│   │   ├── D0 ServiceId ......... M09: registered_count / 12.0
│   │   ├── D2 Tier .............. M09: avg(tier.normalized())
│   │   ├── D3 DependencyCount ... M09: avg(dep_count) / 12.0
│   │   ├── D4 AgentCount ........ M09: healthy_count / total_count
│   │   ├── D6 HealthScore ....... M10: aggregate_health() | M11: fraction_running
│   │   ├── D7 Uptime ............ M11: 1.0 - avg(restart_count / max_restarts)
│   │   ├── D9 Latency ........... M12: closed_fraction() [circuit health proxy]
│   │   └── D10 ErrorRate ........ M10: 1.0 - aggregate_health | M12: avg_failure_rate
│   │
│   ├── Uncovered by L2 (4/12)
│   │   ├── D1 Port .............. provided by L1 Config
│   │   ├── D5 Protocol .......... provided by L1 LogContext, Resources
│   │   ├── D8 Synergy ........... provided by L4 Integration
│   │   └── D11 TemporalContext .. provided by L5 Learning
│   │
│   ├── Dimension Overlaps (intentional, resolved by compose())
│   │   ├── D6: M10 (probe-based health) + M11 (% running) → averaged
│   │   └── D10: M10 (service-level error) + M12 (request-level error) → averaged
│   │
│   └── All tensor values clamped [0.0, 1.0]
│
├── Builder Patterns (4)
│   │
│   ├── ServiceDefinitionBuilder ──→ build() -> ServiceDefinition [no validation]
│   ├── HealthProbeBuilder ──→ build() -> Result<HealthProbe> [validates thresholds, timeout]
│   ├── LifecycleEntryBuilder ──→ build() -> LifecycleEntry [no validation]
│   └── CircuitBreakerConfigBuilder ──→ build() -> CircuitBreakerConfig [no validation]
│   │
│   └── Pattern: All use consuming self (move semantics), fluent chaining
│
├── Error Taxonomy (all use L1 unified Error type)
│   │
│   ├── M09 ServiceRegistry
│   │   ├── AlreadyExists ............. "Service '{id}' already registered"
│   │   ├── NotFound .................. "Service '{id}' not found"
│   │   ├── NotFound .................. "Service '{id}' not registered" (deregister)
│   │   ├── InvalidInput .............. "Service cannot depend on itself"
│   │   ├── NotFound .................. "Source service '{id}' not registered"
│   │   └── NotFound .................. "Target service '{id}' not registered"
│   │
│   ├── M10 HealthMonitor
│   │   ├── AlreadyExists ............. "Probe for '{id}' already registered"
│   │   ├── NotFound .................. "No probe registered for '{id}'"
│   │   ├── InvalidInput .............. "Thresholds must be > 0"
│   │   └── InvalidInput .............. "Timeout must be <= interval"
│   │
│   ├── M11 LifecycleManager
│   │   ├── AlreadyExists ............. "Service '{id}' already registered"
│   │   ├── InvalidState .............. "Cannot transition {from} → {to}"
│   │   ├── NotFound .................. "Service '{id}' not registered"
│   │   └── ResourceExhausted ......... "Service '{id}' exceeded max restarts ({n})"
│   │
│   └── M12 ResilienceManager
│       ├── AlreadyExists ............. "Breaker/Pool for '{id}' already exists"
│       ├── NotFound .................. "No breaker/pool for '{id}'"
│       ├── Unavailable ............... "Circuit open for '{id}'"
│       ├── AlreadyExists ............. "Endpoint '{ep_id}' already in pool"
│       ├── NotFound .................. "Endpoint '{ep_id}' not found in pool"
│       └── Unavailable ............... "No healthy endpoints for '{id}'"
│
├── Signal Emission Topology (C6)
│   │
│   ├── All signals flow through Arc<SignalBus> from L1
│   │
│   ├── Emission Points (4)
│   │   ├── M09 ServiceRegistry.update_health() ──→ HealthSignal [on status transition]
│   │   ├── M10 HealthMonitor.record_result() ──→ HealthSignal [on FSM state change]
│   │   ├── M11 LifecycleManager.apply_transition() ──→ HealthSignal [on score change]
│   │   └── M12 CircuitBreakerRegistry.record_success/failure() ──→ HealthSignal [on state transition]
│   │
│   ├── Emission Rules
│   │   ├── Only on transitions — NOT on every API call
│   │   ├── M09: old_health != new_health
│   │   ├── M10: FSM state changes (e.g. Healthy→Degraded)
│   │   ├── M11: status_health_score(from) != status_health_score(to)
│   │   └── M12: any circuit state change (Closed↔Open↔HalfOpen)
│   │
│   ├── Signal Payload: HealthSignal { service_id, score, timestamp }
│   │
│   └── Consumers (L3-L7)
│       ├── L3 Pipeline ──→ on_health()
│       ├── L4 Integration ──→ cross-service events
│       ├── L5 Learning ──→ STDP co-activation
│       ├── L6 Consensus ──→ PBFT voting input
│       └── L7 Observer ──→ emergence detection
│
├── ULTRAPLATE Service Definitions (12)
│   │
│   ├── Tier 1 (weight 1.5) — Critical Infrastructure
│   │   ├── maintenance-engine ..... port 8080, /api/health
│   │   └── devops-engine .......... port 8081, /health
│   │
│   ├── Tier 2 (weight 1.3) — Core Services
│   │   ├── synthex ................ port 8090, /api/health
│   │   ├── san-k7-orchestrator .... port 8100, /health
│   │   └── codesynthor-v7 ......... port 8110, /health
│   │
│   ├── Tier 3 (weight 1.2) — Standard Services
│   │   ├── nais ................... port 8101, /health
│   │   ├── bash-engine ............ port 8102, /health
│   │   └── tool-maker ............. port 8103, /health
│   │
│   ├── Tier 4 (weight 1.1) — Auxiliary Services
│   │   ├── claude-context-manager . port 8104, /health
│   │   ├── tool-library ........... port 8105, /health
│   │   └── sphere-vortex .......... port 8120, /health
│   │
│   └── Tier 5 (weight 1.0) — Non-Critical
│       └── library-agent .......... port 8083, /health
│
├── Constants & Thresholds
│   │
│   ├── RestartConfig Defaults
│   │   ├── max_restarts = 5
│   │   ├── initial_backoff = 1s
│   │   └── max_backoff = 30s
│   │
│   ├── CircuitBreakerConfig Defaults
│   │   ├── failure_threshold = 5
│   │   ├── success_threshold = 3
│   │   ├── open_timeout = 30s
│   │   ├── half_open_max_requests = 1
│   │   └── monitoring_window = 60s
│   │
│   ├── HealthMonitor Defaults
│   │   └── max_history = 100 [ring buffer cap]
│   │
│   ├── ULTRAPLATE Fleet
│   │   └── ULTRAPLATE_SERVICE_COUNT = 12
│   │
│   ├── Endpoint Bounds
│   │   └── weight range = [0.0, 1.0] [clamped on construction]
│   │
│   └── Random LCG Constants
│       ├── multiplier = 6364136223846793005
│       └── increment = 1442695040888963407
│
├── Design Principles
│   │
│   ├── All trait methods &self: interior mutability via RwLock, not &mut self
│   ├── Owned returns (C7): all data crossing lock boundaries is cloned
│   ├── Signal after lock release: prevents deadlock in subscriber callbacks
│   ├── Two-clock strategy (M12): Timestamp for history, Instant for timeout
│   ├── No chrono/SystemTime (C5): only L1 Timestamp + Duration
│   ├── Zero unsafe, unwrap, expect (C4): compile-time + clippy enforcement
│   ├── Single lock per manager: no nested locks, no deadlock possible
│   ├── const fn where possible: is_valid_transition, status_health_score, as_str, weight
│   ├── Facade pattern (M12): ResilienceManager owns CircuitBreakerRegistry + LoadBalancer
│   ├── Display on all public types: 15+ implementations
│   ├── Builder patterns with validation: HealthProbeBuilder.build() returns Result
│   └── Deterministic "random": LCG from counter, reproducible for testing
│
├── Quality Gate Results
│   │
│   ├── cargo check ......................................... 0 errors
│   ├── cargo clippy -- -D warnings ........................ 0 warnings
│   ├── cargo clippy -- -D warnings -W clippy::pedantic .... 0 warnings
│   ├── cargo clippy -- -D warnings -W clippy::nursery ..... 0 warnings
│   ├── cargo test --lib m2_services ....................... 279 tests, 0 failures
│   ├── Integration tests (l2_services_integration.rs) ..... 41 tests, 0 failures
│   └── Zero-tolerance grep (unsafe/unwrap/expect) ......... 0 hits
│
├── Test Taxonomy (320 tests: 279 unit + 41 integration)
│   │
│   ├── mod.rs (20 tests, 6 groups)
│   │   ├── Group 1: Enum Display + as_str (5)
│   │   │   └── ServiceStatus, HealthStatus, CircuitState display and variant coverage
│   │   ├── Group 2: Health Scores (3)
│   │   │   └── HealthStatus.score(), ServiceTier.weight()
│   │   ├── Group 3: ServiceTier (3)
│   │   │   └── weight, number, normalized for all 5 tiers
│   │   ├── Group 4: ServiceState (4)
│   │   │   └── new, weighted_health, update_tensor, is_operational
│   │   ├── Group 5: ServicesStatus + RestartConfig (3)
│   │   │   └── Display, defaults, Copy trait
│   │   └── Group 6: hash_to_float (2)
│   │       └── Range validation [0.0, 1.0], determinism
│   │
│   ├── service_registry.rs — M09 (53 tests, 11 groups)
│   │   ├── Group 1: Trait Object Safety (2)
│   │   │   └── dyn ServiceDiscovery, Send+Sync
│   │   ├── Group 2: Registration CRUD (8)
│   │   │   └── Register, deregister, duplicate handling, re-register
│   │   ├── Group 3: Discovery (6)
│   │   │   └── By ID, by tier, by protocol (case-insensitive)
│   │   ├── Group 4: Health Management (5)
│   │   │   └── Update, query, transition detection, signal emission
│   │   ├── Group 5: Dependencies (7)
│   │   │   └── Add, get forward, get reverse, self-dep rejection
│   │   ├── Group 6: ULTRAPLATE Bootstrap (5)
│   │   │   └── 12-service registration, tier distribution, port mapping
│   │   ├── Group 7: Tensor Contribution (4)
│   │   │   └── D0, D2, D3, D4 values and coverage bitmap
│   │   ├── Group 8: Display/Default (6)
│   │   │   └── ServiceDefinition Display, builder defaults
│   │   ├── Group 9: Signal Emission (4)
│   │   │   └── Health transition signals, no signal on non-transition
│   │   ├── Group 10: Metrics (2)
│   │   │   └── Counter increments on registration
│   │   └── Group 11: Edge Cases (4)
│   │       └── Empty registry, deregister+re-register, filter empty results
│   │
│   ├── health_monitor.rs — M10 (49 tests, 11 groups)
│   │   ├── Group 1: Trait Object Safety (2)
│   │   │   └── dyn HealthMonitoring, Send+Sync
│   │   ├── Group 2: Probe Registration (6)
│   │   │   └── Register, unregister, duplicate, counting
│   │   ├── Group 3: FSM Transitions (10)
│   │   │   └── All paths: Unknown→Healthy, Unknown→Unhealthy, Healthy→Degraded,
│   │   │       Degraded→Unhealthy, Degraded→Healthy, Unhealthy→Healthy
│   │   ├── Group 4: Counter Reset (4)
│   │   │   └── Hysteresis: opposite outcome resets counter
│   │   ├── Group 5: History (4)
│   │   │   └── Recording, trimming at max_history, retrieval
│   │   ├── Group 6: Aggregation (5)
│   │   │   └── Empty=0.0, all healthy=1.0, mixed weighted avg
│   │   ├── Group 7: Status Partitioning (4)
│   │   │   └── Healthy/degraded/unhealthy disjoint sets
│   │   ├── Group 8: Builder Validation (4)
│   │   │   └── threshold>0, timeout<=interval
│   │   ├── Group 9: Signal Emission (3)
│   │   │   └── Transition signals, no signal on non-transition
│   │   ├── Group 10: Tensor Contribution (3)
│   │   │   └── D6, D10, coverage bitmap
│   │   └── Group 11: Display/Formatting (4)
│   │       └── HealthCheckResult Display, HealthProbe Display
│   │
│   ├── lifecycle.rs — M11 (75 tests, 15 groups)
│   │   ├── Group 1: Trait Object Safety (3)
│   │   │   └── dyn LifecycleOps, Send+Sync, Arc<dyn>
│   │   ├── Group 2: Registration (6)
│   │   │   └── Register, deregister, duplicate, counting
│   │   ├── Group 3: FSM Happy Path (7)
│   │   │   └── All 7 valid transitions
│   │   ├── Group 4: FSM Rejection (8)
│   │   │   └── All invalid transition pairs
│   │   ├── Group 5: Restart Mechanics (10)
│   │   │   └── Backoff doubling, counter increment, limit enforcement
│   │   ├── Group 6: History (5)
│   │   │   └── Recording, transition list, trimming
│   │   ├── Group 7: Entry Construction (5)
│   │   │   └── Builder, defaults, Display
│   │   ├── Group 8: Fleet Queries (4)
│   │   │   └── get_all_running, get_all_failed
│   │   ├── Group 9: Status Queries (4)
│   │   │   └── get_status, get_entry, is_registered
│   │   ├── Group 10: Backoff Computation (5)
│   │   │   └── Exponential growth, cap at max_backoff
│   │   ├── Group 11: Reset (3)
│   │   │   └── reset_restart_count, mark_stopped reset
│   │   ├── Group 12: Signal Emission (4)
│   │   │   └── Transition signals, score change detection
│   │   ├── Group 13: Tensor Contribution (4)
│   │   │   └── D6, D7, coverage, empty manager
│   │   ├── Group 14: LifecycleAction (3)
│   │   │   └── Variants, Display
│   │   └── Group 15: Helper Functions (4)
│   │       └── is_valid_transition (7 valid + 7 invalid), status_health_score
│   │
│   └── resilience.rs — M12 (82 tests, 18 groups)
│       ├── Group 1: Trait Object Safety (4)
│       │   └── dyn CircuitBreakerOps, dyn LoadBalancing, Send+Sync
│       ├── Group 2: Config Builder (4)
│       │   └── Defaults, custom params, builder chain
│       ├── Group 3: Circuit FSM (12)
│       │   └── Closed→Open→HalfOpen→Closed, all paths
│       ├── Group 4: Threshold Crossing (6)
│       │   └── Exact threshold, below threshold, boundary
│       ├── Group 5: Open Timeout (4)
│       │   └── Timeout elapsed, not elapsed, monotonic verification
│       ├── Group 6: Pool Operations (8)
│       │   └── Create, remove, add/remove endpoints, duplicate
│       ├── Group 7: RoundRobin (5)
│       │   └── Sequential selection, wrap-around, skip unhealthy
│       ├── Group 8: WeightedRoundRobin (4)
│       │   └── Weight-proportional distribution, edge weights
│       ├── Group 9: LeastConnections (4)
│       │   └── Min selection, tie-breaking
│       ├── Group 10: Random (3)
│       │   └── Deterministic LCG, distribution
│       ├── Group 11: Health Marking (4)
│       │   └── Healthy/unhealthy, selection exclusion
│       ├── Group 12: Request Recording (4)
│       │   └── active_connections tracking, error recording
│       ├── Group 13: Pool Stats (4)
│       │   └── Aggregation, error_rate computation
│       ├── Group 14: Load Distribution (3)
│       │   └── Percentage computation, normalization
│       ├── Group 15: Signal Emission (4)
│       │   └── Circuit state transition signals
│       ├── Group 16: Tensor Contribution (4)
│       │   └── D9, D10, coverage, empty manager
│       ├── Group 17: ResilienceManager (5)
│       │   └── Facade accessors, construction, defaults
│       └── Group 18: Display (4)
│           └── Endpoint, Stats, Config, CircuitStateTransition
│
├── Cross-Layer Export Boundaries (L2 → L3+)
│   │
│   ├── Core Types → ALL downstream layers
│   │   ├── ServiceStatus, HealthStatus, ServiceTier, CircuitState [enums]
│   │   ├── ServiceState, ServicesStatus, RestartConfig [structs]
│   │   └── All 26 re-exported types from mod.rs
│   │
│   ├── Trait Objects → L3 (Core Logic)
│   │   ├── Arc<dyn ServiceDiscovery> [service lookup]
│   │   ├── Arc<dyn HealthMonitoring> [health queries]
│   │   ├── Arc<dyn LifecycleOps> [lifecycle control]
│   │   ├── Arc<dyn CircuitBreakerOps> [fault isolation]
│   │   └── Arc<dyn LoadBalancing> [request routing]
│   │
│   ├── Concrete Types → Engine/Main
│   │   ├── ServiceRegistry [DI root]
│   │   ├── HealthMonitor [DI root]
│   │   ├── LifecycleManager [DI root]
│   │   └── ResilienceManager [DI root, facade]
│   │
│   ├── Bootstrap → Engine startup
│   │   └── register_ultraplate_services() [seeds 12 services]
│   │
│   └── Direction Rule: L2 depends only on L1 (C1). L3+ depends on L2. Never L2→L3+.
│
├── Active Connections Lifecycle (M12)
│   │
│   ├── select_endpoint() ──→ active_connections += 1
│   ├── (request in flight)
│   ├── record_request(success=true) ──→ active_connections -= 1, total_requests += 1
│   └── record_request(success=false) ──→ active_connections -= 1, total_requests += 1, total_errors += 1
│   │
│   └── Invariant: every select_endpoint() MUST pair with record_request() (leak prevention)
│
├── Dependency Graph (M09)
│   │
│   ├── Storage: HashMap<String, Vec<String>> [adjacency list, forward edges]
│   ├── Forward: get_dependencies(from) → [to1, to2, ...]
│   ├── Reverse: get_dependents(to) → scans all entries [O(n), acceptable for 12 services]
│   ├── Validation: both from and to must be registered, no self-dependency
│   └── No cycle detection — caller's responsibility
│
└── Clippy Allowances (documented)
    │
    └── (none) — zero #[allow(clippy::*)] in entire L2 layer
```

---

## Relationship Matrix (Internal Dependencies)

```
                mod    service_reg  health_mon   lifecycle   resilience
mod.rs          --     re-exports   re-exports   re-exports  re-exports
service_reg     ←      --           .            .           .
health_mon      ←      .            --           .           .
lifecycle       ←      .            .            --          .
resilience      ←      .            .            .           --

Legend: ← = imports shared enums/types from mod.rs
        . = no dependency
        All 4 modules depend on L1 Foundation (Error, Timestamp, SignalBus, etc.)
        No cross-dependencies between M09, M10, M11, M12
```

---

## Statistics Summary

| Category | Count |
|----------|-------|
| Source files | 5 |
| Modules | 4 (M09-M12) |
| Total LOC | 7,196 |
| Total tests | 320 (279 unit + 41 integration) |
| Traits (defined in L2) | 5 (ServiceDiscovery, HealthMonitoring, LifecycleOps, CircuitBreakerOps, LoadBalancing) |
| Traits (implemented from L1) | 1 (TensorContributor) |
| Trait methods | 65 (all &self, 0 defaults) |
| Public types | ~30 (structs + enums) |
| Public functions | 3 (is_valid_transition, status_health_score, register_ultraplate_services) |
| Constants | ~15 (defaults, LCG params, fleet size) |
| Builder patterns | 4 |
| Finite state machines | 3 (lifecycle, circuit breaker, health monitor) |
| FSM states total | 12 (5 + 3 + 4) |
| FSM transitions | 20 (7 + 8 + 5 valid) |
| Error scenarios | ~18 (across 4 modules) |
| Tensor dimensions covered | 8/12 (D0, D2, D3, D4, D6, D7, D9, D10) |
| Display impls | ~15 |
| const fn | 8 (as_str, weight, is_valid_transition, status_health_score, ...) |
| Concurrency primitives | RwLock × 5 |
| Signal emission points | 4 |
| Re-exported types | 26 |
| ULTRAPLATE services | 12 (pre-registered at startup) |
| Load balance algorithms | 4 (RoundRobin, Weighted, LeastConn, Random) |
| Clippy warnings | 0 (pedantic + nursery) |
| unsafe blocks | 0 (compile-time forbidden) |
| unwrap/expect | 0 (clippy denied) |
| #[allow(clippy::*)] | 0 (none needed) |

---

*L2 Services Meta Tree Mind Map v1.0 | 2026-03-01*
*Derived from 5 source files + 6 spec sheets (M2-META-TREE-MIND-MAP.md)*
