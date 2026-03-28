# L1 Foundation — Meta Tree Mind Map

> **Scope:** L1 ONLY (M00-M08, 9 modules, 11 files) | **LOC:** ~16,711 | **Tests:** 678
> **Derived from:** 11 source files + 12 spec sheets | **Date:** 2026-03-01
> **Purpose:** Exhaustive hierarchical decomposition of every type, trait, function, constant,
> pattern, invariant, relationship, and test category within the L1 Foundation layer.

---

## Root: L1 Foundation Layer

```
L1 Foundation
├── Identity
│   ├── LAYER_ID = "L1"
│   ├── MODULE_COUNT = 9 (M00-M08)
│   ├── LOC = ~16,711
│   ├── Tests = 678
│   ├── Quality Score = 80.6/100
│   └── Commit = 1a60c5e
│
├── Files (11) — verified 2026-03-01
│   ├── mod.rs .............. M00 Layer Coordinator (1,307 LOC, ~88 tests)
│   ├── shared_types.rs .... M00 Vocabulary Types (1,209 LOC, ~88 tests)
│   ├── error.rs ........... M01 Error Taxonomy (1,528 LOC, ~44 tests)
│   ├── config.rs .......... M02 Configuration (1,871 LOC, ~30 tests)
│   ├── logging.rs ......... M03 Logging (1,497 LOC, ~30 tests)
│   ├── metrics.rs ......... M04 Metrics (1,979 LOC, ~25 tests)
│   ├── state.rs ........... M05 State Persistence (2,105 LOC, ~20 tests)
│   ├── resources.rs ....... M06 Resources (2,006 LOC, ~30 tests)
│   ├── nam.rs ............. NAM Primitives (707 LOC, 35 tests)
│   ├── signals.rs ......... M07 Signal Bus (1,167 LOC, ~55 tests)
│   └── tensor_registry.rs  M08 Tensor Registry (1,335 LOC, ~80 tests)
│
├── Module Tiers (3)
│   ├── LEAF (zero internal deps)
│   │   ├── shared_types.rs
│   │   └── nam.rs (depends only on shared_types::AgentId)
│   ├── INFRASTRUCTURE (depends on leaf modules)
│   │   ├── error.rs ──→ nam.rs
│   │   ├── config.rs ──→ shared_types, error
│   │   ├── logging.rs ──→ shared_types, error
│   │   ├── metrics.rs ──→ shared_types, error
│   │   ├── state.rs ──→ shared_types, error, config
│   │   └── resources.rs ──→ shared_types, error
│   └── EXTENSION (depends on leaf + infrastructure)
│       ├── signals.rs ──→ shared_types, error, nam
│       └── tensor_registry.rs ──→ shared_types
│
├── Traits (8)
│   │
│   ├── [1] ErrorClassifier (error.rs)
│   │   ├── Bounds: (none)
│   │   ├── Methods (5)
│   │   │   ├── is_retryable(&self) -> bool [REQUIRED]
│   │   │   ├── is_transient(&self) -> bool [REQUIRED]
│   │   │   ├── severity(&self) -> Severity [REQUIRED]
│   │   │   ├── error_code(&self) -> u32 [DEFAULT: 0]
│   │   │   └── error_category(&self) -> &'static str [DEFAULT: "other"]
│   │   ├── Implementor: Error
│   │   └── Object Safety: YES (compile-tested)
│   │
│   ├── [2] ConfigProvider (config.rs)
│   │   ├── Bounds: Send + Sync
│   │   ├── Methods (5)
│   │   │   ├── get(&self) -> Result<Config> [REQUIRED]
│   │   │   ├── validate(&self) -> Result<()> [REQUIRED]
│   │   │   ├── reload(&self) -> Result<Config> [REQUIRED]
│   │   │   ├── change_history(&self) -> Vec<ConfigChangeEvent> [DEFAULT: empty]
│   │   │   └── agent_id(&self) -> Option<&str> [DEFAULT: None] (NAM R5)
│   │   ├── Implementor: ConfigManager
│   │   ├── Object Safety: YES
│   │   └── Arc Boundary: Arc<dyn ConfigProvider> for DI
│   │
│   ├── [3] CorrelationProvider (logging.rs)
│   │   ├── Bounds: Send + Sync
│   │   ├── Methods (3)
│   │   │   ├── correlation_id(&self) -> &str [REQUIRED]
│   │   │   ├── child(&self, operation: &str) -> Box<dyn CorrelationProvider> [REQUIRED]
│   │   │   └── agent_id(&self) -> Option<&str> [DEFAULT: None] (NAM R5)
│   │   ├── Implementor: LogContext
│   │   └── Object Safety: YES
│   │
│   ├── [4] MetricRecorder (metrics.rs)
│   │   ├── Bounds: Send + Sync
│   │   ├── Methods (4, 0 defaults)
│   │   │   ├── increment_counter(&self, name, labels) -> Result<()>
│   │   │   ├── set_gauge(&self, name, value, labels) -> Result<()>
│   │   │   ├── observe_histogram(&self, name, value, labels) -> Result<()>
│   │   │   └── snapshot(&self) -> Result<MetricSnapshot>
│   │   ├── Implementor: MetricsRegistry
│   │   └── Object Safety: YES
│   │
│   ├── [5] StateStore (state.rs)
│   │   ├── Bounds: Send + Sync
│   │   ├── Methods (3)
│   │   │   ├── pool(&self) -> &DatabasePool [REQUIRED]
│   │   │   ├── store_name(&self) -> &str [REQUIRED]
│   │   │   └── agent_id(&self) -> Option<&str> [DEFAULT: None] (NAM R5)
│   │   ├── Implementor: DatabasePool (blanket impl)
│   │   └── Object Safety: YES
│   │
│   ├── [6] ResourceCollector (resources.rs)
│   │   ├── Bounds: Send + Sync
│   │   ├── Methods (5)
│   │   │   ├── collect(&self) -> Result<SystemResources> [REQUIRED]
│   │   │   ├── check_limits(&self) -> Vec<ResourceAlert> [REQUIRED]
│   │   │   ├── health_score(&self) -> f64 [REQUIRED]
│   │   │   ├── agent_id(&self) -> Option<&str> [DEFAULT: None] (NAM R5)
│   │   │   └── to_tensor(&self) -> Tensor12D [DEFAULT: zeros] (NAM R4)
│   │   ├── Implementor: ResourceManager
│   │   └── Object Safety: YES
│   │
│   ├── [7] SignalSubscriber (signals.rs)
│   │   ├── Bounds: Send + Sync + Debug
│   │   ├── Methods (4)
│   │   │   ├── on_health(&self, signal: &HealthSignal) [DEFAULT: no-op]
│   │   │   ├── on_learning(&self, event: &LearningEvent) [DEFAULT: no-op]
│   │   │   ├── on_dissent(&self, event: &DissentEvent) [DEFAULT: no-op]
│   │   │   └── subscriber_id(&self) -> &str [REQUIRED]
│   │   ├── Arc Boundary: Arc<dyn SignalSubscriber> stored in SignalBus
│   │   └── Object Safety: YES (compile-tested)
│   │
│   └── [8] TensorContributor (tensor_registry.rs)
│       ├── Bounds: Send + Sync + Debug
│       ├── Methods (3, 0 defaults)
│       │   ├── contribute(&self) -> ContributedTensor
│       │   ├── contributor_kind(&self) -> ContributorKind
│       │   └── module_id(&self) -> &str
│       ├── Arc Boundary: Arc<dyn TensorContributor> stored in TensorRegistry
│       └── Object Safety: YES (compile-tested)
│
├── Types — Vocabulary (M00 shared_types.rs)
│   │
│   ├── ModuleId
│   │   ├── Kind: newtype(&'static str)
│   │   ├── Derives: Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord
│   │   ├── Constants: M01..M42 (42 total) + ALL: [Self; 42]
│   │   ├── Methods
│   │   │   ├── new(id: &'static str) -> Self [const fn, #[must_use]]
│   │   │   ├── as_str(&self) -> &'static str [const fn, #[must_use]]
│   │   │   ├── number(&self) -> Option<u8> [parses "M{N}"]
│   │   │   └── layer(&self) -> Option<u8> [M01-06→L1, M07-12→L2, ...]
│   │   └── Traits: Display ("M04"), AsRef<str>
│   │
│   ├── AgentId
│   │   ├── Kind: newtype(String)
│   │   ├── Derives: Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord
│   │   ├── Factories
│   │   │   ├── system() -> "sys:system"
│   │   │   ├── human() -> "human:@0.A"
│   │   │   ├── service(id) -> "svc:{id}"
│   │   │   ├── agent(id) -> "agent:{id}"
│   │   │   └── from_raw(s) -> raw string (unchecked)
│   │   ├── Query Methods
│   │   │   ├── is_system() -> bool
│   │   │   ├── is_human() -> bool
│   │   │   ├── is_service() -> bool
│   │   │   ├── is_agent() -> bool
│   │   │   ├── prefix() -> &str
│   │   │   └── as_str() -> &str
│   │   └── Traits: Display, AsRef<str>, From<AgentId> for String
│   │
│   ├── Timestamp
│   │   ├── Kind: newtype(u64)
│   │   ├── Derives: Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash
│   │   ├── Global State: static GLOBAL_TICK: AtomicU64 (Relaxed ordering)
│   │   ├── Constants: ZERO = Timestamp(0)
│   │   ├── Methods
│   │   │   ├── now() -> Self [atomic fetch_add(1), strictly increasing]
│   │   │   ├── from_raw(ticks: u64) -> Self [const fn, for testing]
│   │   │   ├── ticks(&self) -> u64 [const fn]
│   │   │   ├── elapsed_since(&self, earlier: Self) -> u64 [const fn, saturating]
│   │   │   └── within_window(&self, other: Self, window: u64) -> bool [const fn]
│   │   └── Traits: Display ("T999"), Default (ZERO)
│   │
│   ├── HealthReport
│   │   ├── Kind: struct
│   │   ├── Derives: Debug, Clone, PartialEq
│   │   ├── Fields
│   │   │   ├── module_id: ModuleId
│   │   │   ├── health_score: f64 [clamped 0.0-1.0]
│   │   │   ├── timestamp: Timestamp
│   │   │   └── details: Option<String>
│   │   ├── Methods
│   │   │   ├── new(module_id, health_score) [clamps, sets timestamp=now()]
│   │   │   ├── with_details(impl Into<String>) -> Self [builder]
│   │   │   ├── with_timestamp(Timestamp) -> Self [const fn, testing]
│   │   │   ├── is_healthy() -> bool [score >= 0.5]
│   │   │   └── is_critical() -> bool [score < 0.2]
│   │   └── Traits: Display ("Health(M04: 0.950 at T123)")
│   │
│   ├── DimensionIndex
│   │   ├── Kind: enum #[repr(u8)]
│   │   ├── Derives: Debug, Clone, Copy, PartialEq, Eq, Hash
│   │   ├── Variants (12)
│   │   │   ├── ServiceId = 0
│   │   │   ├── Port = 1
│   │   │   ├── Tier = 2
│   │   │   ├── DependencyCount = 3
│   │   │   ├── AgentCount = 4
│   │   │   ├── Protocol = 5
│   │   │   ├── HealthScore = 6
│   │   │   ├── Uptime = 7
│   │   │   ├── Synergy = 8
│   │   │   ├── Latency = 9
│   │   │   ├── ErrorRate = 10
│   │   │   └── TemporalContext = 11
│   │   ├── Constants: ALL: [Self; 12]
│   │   ├── Methods
│   │   │   ├── index(self) -> usize [const fn, 0..=11]
│   │   │   ├── name(self) -> &'static str [const fn]
│   │   │   ├── from_index(usize) -> Option<Self> [const fn]
│   │   │   └── from_name(&str) -> Option<Self>
│   │   └── Traits: Display ("D6:health_score")
│   │
│   └── CoverageBitmap
│       ├── Kind: newtype(u16), bottom 12 bits only
│       ├── Derives: Debug, Clone, Copy, PartialEq, Eq, Hash, Default
│       ├── Constants: EMPTY = CoverageBitmap(0), FULL = CoverageBitmap(0x0FFF)
│       ├── Methods
│       │   ├── from_raw(bits: u16) -> Self [const fn, masked]
│       │   ├── with_dimension(self, dim) -> Self [const fn, chainable]
│       │   ├── is_covered(self, dim) -> bool [const fn]
│       │   ├── count(self) -> u32 [const fn, popcount]
│       │   ├── union(self, other) -> Self [const fn, bitwise OR]
│       │   ├── intersection(self, other) -> Self [const fn, bitwise AND]
│       │   ├── coverage_ratio(self) -> f64 [count/12]
│       │   ├── covered_dimensions(self) -> Vec<DimensionIndex>
│       │   └── uncovered_dimensions(self) -> Vec<DimensionIndex>
│       └── Traits: Display ("Coverage(4/12 = 33%)"), Default (EMPTY)
│
├── Types — Error Taxonomy (M01 error.rs)
│   │
│   ├── Error (enum, 16 variants)
│   │   ├── Derives: Debug (manual Clone, PartialEq, Eq)
│   │   ├── Type Alias: pub type Result<T> = std::result::Result<T, Error>
│   │   ├── Variants
│   │   │   ├── Config(String) ................. code=1000, sev=Low, retry=No
│   │   │   ├── Database(String) ............... code=1100, sev=Med, retry=if "locked"/"busy"
│   │   │   ├── Network { target, message } .... code=1200, sev=Med, retry=Yes, transient=Yes
│   │   │   ├── CircuitOpen { service_id, retry_after_ms } .. code=1201, sev=Med, retry=Yes
│   │   │   ├── Timeout { operation, timeout_ms } .......... code=1202, sev=Med, retry=Yes
│   │   │   ├── ConsensusQuorum { required, received } ..... code=1300, sev=High, retry=Yes
│   │   │   ├── ViewChange { current_view, new_view } ...... code=1301, sev=Crit, retry=No
│   │   │   ├── PathwayNotFound { source, target } ......... code=1400, sev=Low, retry=No
│   │   │   ├── TensorValidation { dimension, value } ...... code=1401, sev=Med, retry=No
│   │   │   ├── Validation(String) ............. code=1500, sev=Low, retry=No
│   │   │   ├── Io(std::io::Error) ............. code=1600, sev=Med, retry=conditional
│   │   │   ├── Pipeline(String) ............... code=1700, sev=High, retry=No
│   │   │   ├── ServiceNotFound(String) ........ code=1800, sev=Low, retry=No
│   │   │   ├── HealthCheckFailed { service_id, reason } ... code=1801, sev=High, retry=No
│   │   │   ├── EscalationRequired { from/to_tier, reason }  code=1802, sev=Crit, retry=No
│   │   │   └── Other(String) .................. code=1900, sev=Low, retry=No
│   │   ├── Methods
│   │   │   └── to_tensor_signal(&self) -> Tensor12D [D6=health, D2=tier, D10=error_rate]
│   │   ├── From Conversions
│   │   │   ├── From<std::io::Error> -> Error::Io
│   │   │   └── From<String> -> Error::Other
│   │   ├── Manual Impls
│   │   │   ├── Clone (deep clone; Io variant via io::Error::new)
│   │   │   ├── PartialEq (Io compared by kind + to_string)
│   │   │   ├── Eq (marker)
│   │   │   └── std::error::Error (source = Some for Io only)
│   │   └── Implements: ErrorClassifier (all 5 methods populated)
│   │
│   ├── Severity (enum, 4 variants)
│   │   ├── Derives: Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash
│   │   ├── Variants: Low < Medium < High < Critical
│   │   └── Traits: Display ("LOW"/"MEDIUM"/"HIGH"/"CRITICAL")
│   │
│   └── AnnotatedError
│       ├── Derives: Debug, Clone, PartialEq
│       ├── Fields
│       │   ├── error: Error
│       │   ├── origin: Option<AgentOrigin>
│       │   └── confidence: Confidence
│       ├── Methods
│       │   ├── new(error: Error) -> Self [const fn, origin=None, confidence=certain()]
│       │   ├── with_origin(self, AgentOrigin) -> Self [builder]
│       │   └── with_confidence(self, Confidence) -> Self [const fn, builder]
│       └── Implements: std::error::Error (source = always Some(&self.error))
│
├── Types — NAM Primitives (nam.rs)
│   │
│   ├── Constants
│   │   ├── HUMAN_AGENT_TAG: &str = "@0.A" (NAM R5)
│   │   ├── LAYER_ID: &str = "L1"
│   │   └── MODULE_COUNT: u8 = 9
│   │
│   ├── AgentOrigin (enum, 4 variants)
│   │   ├── Derives: Debug, Clone, Default, PartialEq, Eq, Hash
│   │   ├── Variants
│   │   │   ├── Human { tag: String }
│   │   │   ├── Service { service_id: String }
│   │   │   ├── Agent { agent_id: String, role: AgentRole }
│   │   │   └── System [#[default]]
│   │   ├── Factories
│   │   │   ├── human() -> Human { tag: "@0.A" }
│   │   │   ├── service(id) -> Service { service_id }
│   │   │   └── agent(id, role) -> Agent { agent_id, role }
│   │   ├── Traits: Display, From<&AgentOrigin> for AgentId
│   │   └── NAM: R5 (HumanAsAgent)
│   │
│   ├── Confidence
│   │   ├── Derives: Debug, Clone, Copy, PartialEq
│   │   ├── Fields: value: f64, lower: f64, upper: f64 [all clamped 0.0-1.0]
│   │   ├── Factories
│   │   │   ├── certain() -> value=1.0, lower=1.0, upper=1.0 [const fn]
│   │   │   ├── uncertain() -> value=0.5, lower=0.0, upper=1.0 [const fn]
│   │   │   └── new(v, lo, hi) -> clamped, lo/hi swapped if inverted
│   │   ├── Methods: is_valid() -> bool [all in [0,1], lower <= upper]
│   │   ├── Traits: Display ("0.700 [0.500, 0.900]"), Default (certain())
│   │   └── NAM: R1 (SelfQuery via is_valid())
│   │
│   ├── Outcome (enum, 3 variants)
│   │   ├── Derives: Debug, Clone, Copy, PartialEq, Eq, Hash
│   │   ├── Variants: Success, Failure, Partial
│   │   ├── Traits: Display
│   │   └── NAM: R2 (Success→LTP, Failure→LTD, Partial→magnitude-scaled)
│   │
│   ├── LearningSignal
│   │   ├── Derives: Debug, Clone, PartialEq
│   │   ├── Fields
│   │   │   ├── source: String
│   │   │   ├── outcome: Outcome
│   │   │   ├── magnitude: f64 [clamped 0.0-1.0]
│   │   │   └── pathway_id: Option<String>
│   │   ├── Factories
│   │   │   ├── success(source) -> magnitude=1.0
│   │   │   ├── failure(source) -> magnitude=1.0
│   │   │   └── partial(source, magnitude) -> clamped
│   │   ├── Builder: .with_pathway(id) [#[must_use]]
│   │   └── NAM: R2 (HebbianRouting)
│   │
│   └── Dissent
│       ├── Derives: Debug, Clone, PartialEq
│       ├── Fields
│       │   ├── agent: AgentOrigin
│       │   ├── target: String
│       │   ├── reasoning: String
│       │   ├── confidence: f64 [clamped 0.0-1.0]
│       │   └── alternative: Option<String>
│       ├── Methods
│       │   ├── new(agent, target, reasoning) -> Self
│       │   ├── with_confidence(self, f64) -> Self [const fn, manual clamp]
│       │   ├── with_alternative(self, &str) -> Self
│       │   └── is_valid() -> bool [confidence in [0.0, 1.0]]
│       ├── Traits: Display
│       └── NAM: R3 (DissentCapture)
│
├── Types — Configuration (M02 config.rs)
│   │
│   ├── Config
│   │   ├── Derives: Clone, Debug, Serialize, Deserialize, PartialEq, Eq
│   │   ├── Fields
│   │   │   ├── host: String [default "0.0.0.0", env ME_HOST]
│   │   │   ├── port: u16 [default 8080, env ME_PORT]
│   │   │   ├── grpc_port: u16 [default 8081, env ME_GRPC_PORT]
│   │   │   ├── ws_port: u16 [default 8082, env ME_WS_PORT]
│   │   │   ├── database_path: String [default "data/maintenance.db"]
│   │   │   └── log_level: String [default "info"]
│   │   ├── Methods
│   │   │   ├── load() -> Result<Self> [TOML + env + validation]
│   │   │   ├── load_from_path(&Path) -> Result<Self>
│   │   │   ├── validate(&self) -> Result<()>
│   │   │   ├── defaults() -> Self [#[must_use]]
│   │   │   └── to_tensor() -> Tensor12D [D1=port/65535, D2=1/6, D6=1.0]
│   │   └── Validation: port conflicts, valid log level, non-empty fields
│   │
│   ├── ConfigBuilder
│   │   ├── Terminal: build() -> Result<Config> [validates]
│   │   ├── Setters [all #[must_use]]
│   │   │   ├── with_base_path(&path)
│   │   │   ├── skip_files() [const fn]
│   │   │   ├── skip_env() [const fn]
│   │   │   ├── host(&str)
│   │   │   ├── port(u16) [const fn]
│   │   │   ├── grpc_port(u16) [const fn]
│   │   │   ├── ws_port(u16) [const fn]
│   │   │   ├── database_path(&str)
│   │   │   └── log_level(&str)
│   │   └── Invariant: build() validates port conflicts
│   │
│   ├── ConfigManager
│   │   ├── Internal State
│   │   │   ├── config: Arc<parking_lot::RwLock<Config>>
│   │   │   └── reload_flag: Arc<AtomicBool>
│   │   ├── Construction
│   │   │   ├── new() -> Result<Self> [loads from default path]
│   │   │   ├── with_base_path(path) -> Result<Self>
│   │   │   └── from_config(Config) -> Self [direct, no file]
│   │   ├── Operations
│   │   │   ├── get() -> Config [read lock, clone]
│   │   │   ├── read() -> RwLockReadGuard [borrowed]
│   │   │   ├── reload() -> Result<ConfigChangeEvent> [preserves prev on error]
│   │   │   ├── validate() -> ValidationResult
│   │   │   ├── request_reload() [sets AtomicBool]
│   │   │   ├── reload_requested() -> bool
│   │   │   └── start_hot_reload() -> Result<()> [async, SIGHUP, Unix only]
│   │   └── Implements: ConfigProvider
│   │
│   ├── ConfigChangeEvent
│   │   ├── Fields: change_id, timestamp, changed_keys, previous, new, requested_by
│   │   └── Note: timestamp uses chrono::DateTime<Utc> (legacy — not yet migrated)
│   │
│   ├── ValidationResult { valid, errors, warnings }
│   ├── ValidationError { key, code, message }
│   ├── ValidationWarning { key, code, message }
│   │
│   └── Constants
│       ├── ENV_PREFIX: &str = "ME_"
│       ├── DEFAULT_CONFIG_PATH: &str = "config/default.toml"
│       └── LOCAL_CONFIG_PATH: &str = "config/local.toml"
│
├── Types — Logging (M03 logging.rs)
│   │
│   ├── LogContext
│   │   ├── Derives: Debug, Clone, Default
│   │   ├── Fields
│   │   │   ├── correlation_id: String
│   │   │   ├── service_id: Option<String>
│   │   │   ├── layer: Option<String>
│   │   │   ├── module: Option<String>
│   │   │   └── agent_id: Option<String> (NAM R5)
│   │   ├── Methods [all #[must_use]]
│   │   │   ├── new() -> Self [generates correlation_id]
│   │   │   ├── with_context(service, layer, module) -> Self
│   │   │   ├── child_context() -> Self [new corr_id, inherits]
│   │   │   ├── with_module(module) -> Self
│   │   │   ├── with_layer(layer) -> Self
│   │   │   ├── with_agent(agent_id) -> Self (NAM R5)
│   │   │   └── to_tensor_position() -> Tensor12D [D0, D2, D5, D6]
│   │   └── Implements: CorrelationProvider
│   │
│   ├── LogFormat (enum)
│   │   ├── Variants: Json, Pretty [#[default]], Compact
│   │   └── Traits: Display, FromStr, Debug, Clone, Copy, PartialEq, Eq, Default
│   │
│   ├── LogLevel (enum)
│   │   ├── Variants: Trace < Debug < Info [#[default]] < Warn < Error
│   │   ├── Method: to_tracing_level(self) -> Level [const fn]
│   │   └── Traits: Display, FromStr (accepts "warning" for Warn)
│   │
│   ├── LogConfig
│   │   ├── Fields: level, format, include_timestamps/targets/file_line/thread_ids/span_events
│   │   ├── Factories: default(), development(), production(), from_env()
│   │   └── Profiles
│   │       ├── default: Pretty, info, timestamps+targets
│   │       ├── development: Pretty, debug, file/line
│   │       └── production: JSON, info, thread_ids
│   │
│   ├── Free Functions
│   │   ├── init_logging(&LogConfig) -> Result<()> [errors if already initialized]
│   │   ├── try_init_logging(&LogConfig) [infallible, safe for tests]
│   │   ├── is_logging_initialized() -> bool
│   │   ├── with_context<F, R>(ctx, f) -> R [tracing span]
│   │   ├── with_context_async<F, R>(ctx, f) -> R [async tracing span]
│   │   ├── generate_correlation_id() -> String [UUID v4, 36 chars]
│   │   └── generate_short_correlation_id() -> String [first 8 chars]
│   │
│   ├── Concurrency: static LOGGING_INITIALIZED: OnceLock<bool>
│   │
│   └── Re-exports: tracing::{debug, error, info, trace, warn, *_span}
│
├── Types — Metrics (M04 metrics.rs)
│   │
│   ├── Labels (fluent builder)
│   │   ├── Kind: Vec<(String, String)> sorted for consistent hashing
│   │   ├── Derives: Clone, Debug, Default, PartialEq, Eq, Hash
│   │   ├── Methods [all #[must_use]]
│   │   │   ├── new() [const fn]
│   │   │   ├── service(&str), layer(&str), module(&str), tier(&str)
│   │   │   ├── status(&str), agent(&str) (NAM R5)
│   │   │   ├── with(key, value) [generic]
│   │   │   ├── from_pairs(&[(&str, &str)])
│   │   │   └── is_empty() [const fn]
│   │   └── Sorted-key invariant: ensures hash stability
│   │
│   ├── Counter
│   │   ├── Internal: RwLock<HashMap<Labels, AtomicU64>>
│   │   └── Methods: inc(), inc_by(), get() [#[must_use]], reset()
│   │
│   ├── Gauge
│   │   ├── Internal: RwLock<HashMap<Labels, AtomicU64>> (GAUGE_SCALE=1e6 fixed-point)
│   │   └── Methods: set(), inc(), dec(), add(), get() [#[must_use]]
│   │
│   ├── Histogram
│   │   ├── Internal: RwLock<HashMap<Labels, Arc<HistogramData>>>
│   │   └── Methods: observe(), get_sum(), get_count(), get_buckets() [all #[must_use]]
│   │
│   ├── MetricsRegistry
│   │   ├── Internal: RwLock<HashMap<String, Arc<T>>> for each metric type
│   │   ├── Methods
│   │   │   ├── new(), with_prefix(prefix)
│   │   │   ├── register_counter/gauge/histogram/histogram_default -> Result<Arc<T>>
│   │   │   ├── get_counter/gauge/histogram -> Option<Arc<T>>
│   │   │   ├── export_metrics() -> String [Prometheus text format]
│   │   │   ├── metric_count(), list_metrics() -> Vec<String>
│   │   │   └── snapshot() -> MetricSnapshot
│   │   ├── Name Validation: [a-zA-Z_:][a-zA-Z0-9_:]*
│   │   └── Implements: MetricRecorder
│   │
│   ├── MetricSnapshot
│   │   ├── Fields: timestamp, counters, gauges, histograms (HashMaps)
│   │   └── Method: to_tensor() -> Tensor12D [D2=tier, D6=health, D10=error_rate]
│   │
│   ├── MetricDelta { counter_deltas, gauge_deltas, duration_between }
│   ├── HistogramSummary { count, sum, p50, p95, p99 }
│   │
│   ├── Free Functions
│   │   ├── create_registry() -> MetricsRegistry
│   │   ├── create_maintenance_registry() -> MetricsRegistry [prefix="maintenance_"]
│   │   ├── increment_counter/set_gauge/observe_histogram [convenience]
│   │   ├── export_metrics(registry) -> String
│   │   ├── register_default_metrics(registry) -> Result<()>
│   │   └── snapshot_delta(prev, next) -> MetricDelta
│   │
│   ├── Constants
│   │   ├── DEFAULT_LATENCY_BUCKETS: [f64; 11] [0.005..10.0]
│   │   └── DEFAULT_SIZE_BUCKETS: [f64; 6] [100..10M]
│   │
│   └── Concurrency: Lock upgrade pattern (read→atomic→drop; write→insert→operate)
│
├── Types — State Persistence (M05 state.rs)
│   │
│   ├── DatabaseType (enum, 11 variants)
│   │   ├── Derives: Clone, Copy, Debug, PartialEq, Eq, Hash
│   │   ├── Variants
│   │   │   ├── ServiceTracking, SystemSynergy, HebbianPulse
│   │   │   ├── ConsensusTracking, EpisodicMemory, TensorMemory
│   │   │   ├── PerformanceMetrics, FlowState, SecurityEvents
│   │   │   ├── WorkflowTracking, EvolutionTracking
│   │   ├── Methods
│   │   │   ├── filename(&self) -> &'static str [const fn]
│   │   │   ├── migration_number(&self) -> u32 [const fn, 1-11]
│   │   │   └── all() -> [Self; 11] [const fn]
│   │   └── Traits: Display
│   │
│   ├── DatabaseConfig (builder)
│   │   ├── Fields: path, max_connections, min_connections, acquire_timeout_secs, wal_mode, create_if_missing
│   │   ├── Defaults: path="data/maintenance.db", max=10, min=2, timeout=30, wal=true
│   │   └── Setters: with_max_connections/min_connections/acquire_timeout/wal_mode [all const fn]
│   │
│   ├── DatabasePool
│   │   ├── Backed by: SqlitePool (internally Arc-shared, Clone-safe)
│   │   ├── Methods
│   │   │   ├── database_name(), path() -> &str
│   │   │   ├── inner() -> &SqlitePool [const fn]
│   │   │   ├── stats() -> PoolStats
│   │   │   └── health_check(&self) -> Result<bool> [async]
│   │   └── Implements: StateStore (blanket)
│   │
│   ├── QueryBuilder (fluent, 4 entry points)
│   │   ├── SELECT: .select(&[cols]).from(table).where_eq().and_eq().or_eq().order_by().limit().offset()
│   │   ├── INSERT: .insert_into(table, &[cols]).values(&[vals])
│   │   ├── UPDATE: .update(table).set(col, val).where_eq()
│   │   ├── DELETE: .delete_from(table).where_eq()
│   │   └── Terminal: .build() -> &str, .params() -> Vec<&str>, .params_owned() -> &[String]
│   │
│   ├── StatePersistence (multi-database manager)
│   │   ├── Construction: StatePersistenceBuilder .base_dir().migrations_dir().config().with_database().build().await
│   │   ├── Methods
│   │   │   ├── pool(&self, DatabaseType) -> Result<&DatabasePool>
│   │   │   ├── health_check_all() -> HashMap<DatabaseType, bool>
│   │   │   ├── stats_all() -> HashMap<DatabaseType, PoolStats>
│   │   │   └── to_tensor() -> Tensor12D [D2=tier, D3=db_count/11, D6=1.0]
│   │   └── Internal: Arc<HashMap<DatabaseType, DatabasePool>>
│   │
│   ├── Transaction { commit(), rollback(), execute(), fetch_one(), fetch_all() }
│   ├── PoolStats { size, idle }
│   │
│   └── Free Functions (all async)
│       ├── connect(&DatabaseConfig) -> DatabasePool
│       ├── execute/fetch_one/fetch_all/fetch_optional
│       ├── begin_transaction(pool) -> Transaction
│       ├── run_migrations(pool, dir)
│       ├── save/save_with_provenance(NAM R5)/save_versioned/load/delete/exists/count
│       └── 14 total async functions
│
├── Types — Resources (M06 resources.rs)
│   │
│   ├── SystemResources
│   │   ├── Derives: Clone, Debug
│   │   ├── Fields (9)
│   │   │   ├── cpu_usage_percent: f64
│   │   │   ├── memory_usage_percent: f64
│   │   │   ├── memory_total_bytes: u64
│   │   │   ├── memory_available_bytes: u64
│   │   │   ├── disk_usage_percent: f64
│   │   │   ├── disk_total_bytes: u64
│   │   │   ├── disk_available_bytes: u64
│   │   │   ├── open_file_descriptors: u32
│   │   │   └── timestamp: SystemTime
│   │   └── Method: to_tensor() -> Tensor12D [D2=tier, D5=protocol, D6=1-cpu, D9=1.0, D10=cpu]
│   │
│   ├── ResourceLimits
│   │   ├── Fields: max_cpu_percent, max_memory_percent, max_disk_percent, max_open_files
│   │   ├── Defaults: 80%, 85%, 90%, 1000
│   │   ├── Constructor: new(max_cpu, max_memory, max_disk, max_files) [const fn]
│   │   └── Method: validate(&self) -> Result<()> [rejects outside [0.0, 100.0]]
│   │
│   ├── AdaptiveResourceLimits (NAM R2)
│   │   ├── Fields: base_limits: ResourceLimits, pathway_strength: f64
│   │   └── Method: effective_limits() -> ResourceLimits [Hebbian-adjusted]
│   │
│   ├── ResourceAlert (enum, 4 variants)
│   │   ├── CpuHigh { current, threshold }
│   │   ├── MemoryHigh { current, threshold }
│   │   ├── DiskHigh { current, threshold }
│   │   └── OpenFilesHigh { current, threshold }
│   │
│   ├── ResourceManager
│   │   ├── Methods
│   │   │   ├── new() -> Self [default limits]
│   │   │   ├── with_limits(ResourceLimits) -> Self
│   │   │   ├── collect_and_check(&mut self) -> Result<(SystemResources, Vec<ResourceAlert>)>
│   │   │   ├── set_limits(&mut self, limits) -> Result<()>
│   │   │   ├── is_healthy() -> bool
│   │   │   ├── health_score() -> f64
│   │   │   ├── alert_history() -> &[(SystemTime, ResourceAlert)]
│   │   │   └── utilization_summary() -> HashMap<String, f64>
│   │   ├── Concurrency: &mut self methods, no interior mutability
│   │   ├── Alert History: bounded ring buffer (max 100)
│   │   └── Implements: ResourceCollector
│   │
│   ├── ProcessInfo { pid, threads, vm_size, rss, open_fds, status }
│   │
│   ├── Free Functions
│   │   ├── collect_resources() -> Result<SystemResources> [reads /proc]
│   │   ├── get_process_info() -> Result<ProcessInfo> [reads /proc/{pid}]
│   │   ├── check_limits(resources, limits) -> Vec<ResourceAlert> [pure]
│   │   ├── format_resources(resources) -> String
│   │   ├── format_alerts(alerts) -> String
│   │   └── compute_health_score(snapshot, limits) -> f64 [1.0 if None]
│   │
│   └── Platform: Linux-specific (/proc), non-Linux returns zeros (soft degradation)
│
├── Types — Signals (M07 signals.rs)
│   │
│   ├── Signal Types
│   │   ├── HealthSignal
│   │   │   ├── Fields (6): module_id, previous_health, current_health, reason, timestamp, context
│   │   │   ├── Health values: clamped [0.0, 1.0]
│   │   │   └── Methods
│   │   │       ├── new(module, prev, curr, reason) [clamps both]
│   │   │       ├── with_timestamp(Timestamp) [const fn]
│   │   │       ├── is_degradation() -> bool [current < previous]
│   │   │       ├── is_improvement() -> bool [current > previous]
│   │   │       └── delta() -> f64 [current - previous, signed]
│   │   │
│   │   ├── LearningEvent { signal: LearningSignal, timestamp, context }
│   │   ├── DissentEvent { dissent: Dissent, source_module, timestamp }
│   │   ├── SignalContext { source_module, timestamp, correlation_id: Option<String> }
│   │   └── Signal (unified enum): Health(HealthSignal), Learning(LearningEvent), Dissent(DissentEvent)
│   │
│   ├── SignalBus
│   │   ├── Internal State
│   │   │   ├── subscribers: Arc<RwLock<Vec<Arc<dyn SignalSubscriber>>>>
│   │   │   ├── config: SignalBusConfig
│   │   │   └── stats: Arc<RwLock<SignalBusStats>>
│   │   ├── Methods
│   │   │   ├── new() -> Self [max_subscribers=256]
│   │   │   ├── with_config(config) -> Self
│   │   │   ├── subscribe(subscriber) -> Result<()> [error at capacity]
│   │   │   ├── emit_health(&signal) [synchronous, in-order]
│   │   │   ├── emit_learning(&event) [synchronous, in-order]
│   │   │   ├── emit_dissent(&event) [synchronous, in-order]
│   │   │   ├── stats() -> SignalBusStats [copy out]
│   │   │   ├── subscriber_count() -> usize
│   │   │   └── config() -> &SignalBusConfig [const fn]
│   │   └── Locking Protocol
│   │       ├── 1. Acquire subscribers read lock
│   │       ├── 2. Iterate and call callbacks
│   │       ├── 3. Drop subscribers guard
│   │       └── 4. Acquire stats write lock (AFTER drop — deadlock prevention)
│   │
│   ├── SignalBusConfig { max_subscribers: usize } [default 256]
│   └── SignalBusStats { health_emitted, learning_emitted, dissent_emitted, subscriber_count }
│       └── Method: total_emitted() -> u64 [const fn]
│
├── Types — Tensor Registry (M08 tensor_registry.rs)
│   │
│   ├── ContributorKind (enum)
│   │   ├── Variants: Snapshot, Stream
│   │   └── Traits: Debug, Clone, Copy, PartialEq, Eq, Hash, Display
│   │
│   ├── ContributedTensor
│   │   ├── Fields: tensor: Tensor12D, coverage: CoverageBitmap, kind: ContributorKind
│   │   ├── Methods
│   │   │   ├── new(tensor, coverage, kind) [const fn]
│   │   │   └── dimension_value(dim) -> Option<f64> [Some if covered]
│   │   └── Traits: Display ("Contributed(Snapshot, 4/12)")
│   │
│   ├── ComposedTensor
│   │   ├── Fields (5): tensor, coverage, contributor_count, snapshot_count, stream_count
│   │   ├── Methods
│   │   │   ├── coverage_ratio() -> f64 [count/12]
│   │   │   ├── is_fully_covered() -> bool
│   │   │   └── dead_dimensions() -> Vec<DimensionIndex>
│   │   └── Traits: Display ("Composed(12/12, contributors=4, snap=2, stream=2)")
│   │
│   ├── TensorRegistry
│   │   ├── Internal: Vec<Arc<dyn TensorContributor>>
│   │   ├── Methods
│   │   │   ├── new() -> Self
│   │   │   ├── register(&mut self, contributor) [appends, no limit]
│   │   │   ├── contributor_count() -> usize
│   │   │   ├── compose() -> ComposedTensor [all contributors]
│   │   │   ├── compose_filtered(kind) -> ComposedTensor [filtered]
│   │   │   └── inventory() -> Vec<ContributorInventoryEntry>
│   │   ├── Composition Algorithm
│   │   │   ├── 1. Call contribute() on each
│   │   │   ├── 2. Accumulate sum + count per covered dimension
│   │   │   ├── 3. Union all CoverageBitmaps
│   │   │   └── 4. avg = sum/count, clamp [0.0, 1.0]
│   │   ├── Concurrency: NO internal sync (setup-once, compose-many)
│   │   └── Key Invariant: output always in [0.0, 1.0] per dimension
│   │
│   ├── ContributorInventoryEntry { module_id, kind, coverage }
│   └── Type Alias: pub type TensorDimension = DimensionIndex
│
├── Types — Layer Coordinator (M00 mod.rs)
│   │
│   ├── FoundationStatus
│   │   ├── Fields (8)
│   │   │   ├── layer_id: &'static str ["L1"]
│   │   │   ├── module_count: u8 [9]
│   │   │   ├── logging_initialized: bool
│   │   │   ├── config_valid: bool
│   │   │   ├── metrics_count: usize
│   │   │   ├── resources_healthy: bool
│   │   │   ├── health_score: f64
│   │   │   └── tensor: Tensor12D
│   │   └── Default: layer_id="L1", module_count=9, health_score=1.0, all healthy
│   │
│   └── build_foundation_tensor(config_t, resources_t, metrics_t) -> Tensor12D
│       ├── Averages all 12 dimensions across 3 source tensors
│       ├── Clamps to [0.0, 1.0]
│       └── #[must_use]
│
├── Re-exports (~125 items from mod.rs)
│   │
│   ├── NAM Primitives (7)
│   │   └── AgentOrigin, Confidence, Dissent, LearningSignal, Outcome, HUMAN_AGENT_TAG, LAYER_ID, MODULE_COUNT
│   │
│   ├── Error (5)
│   │   └── AnnotatedError, Error, ErrorClassifier, Result, Severity
│   │
│   ├── Config (8)
│   │   └── Config, ConfigBuilder, ConfigChangeEvent, ConfigManager, ConfigProvider, ValidationError, ValidationResult, ValidationWarning
│   │
│   ├── Metrics (16)
│   │   └── Counter, Gauge, Histogram, HistogramSummary, Labels, MetricDelta, MetricRecorder, MetricSnapshot, MetricsRegistry
│   │   └── create_maintenance_registry, create_registry, export_metrics, increment_counter, observe_histogram, register_default_metrics, set_gauge, snapshot_delta
│   │   └── DEFAULT_LATENCY_BUCKETS, DEFAULT_SIZE_BUCKETS
│   │
│   ├── Resources (14)
│   │   └── AdaptiveResourceLimits, ProcessInfo, ResourceAlert, ResourceCollector, ResourceLimits, ResourceManager, SystemResources
│   │   └── check_limits, collect_resources, compute_health_score, format_alerts, format_resources, get_process_info
│   │
│   ├── State (17)
│   │   └── DatabaseConfig, DatabasePool, DatabaseType, PoolStats, QueryBuilder, StatePersistence, StatePersistenceBuilder, StateStore, Transaction
│   │   └── begin_transaction, connect, count, delete, execute, exists, fetch_all, fetch_one, fetch_optional, load, run_migrations, save, save_versioned, save_with_provenance
│   │
│   ├── Logging (12)
│   │   └── CorrelationProvider, LogConfig, LogContext, LogFormat, LogLevel
│   │   └── generate_correlation_id, generate_short_correlation_id, init_logging, is_logging_initialized, try_init_logging, with_context, with_context_async
│   │
│   ├── Shared Types (6)
│   │   └── AgentId, CoverageBitmap, DimensionIndex, HealthReport, ModuleId, Timestamp
│   │
│   ├── Signals (9)
│   │   └── DissentEvent, HealthSignal, LearningEvent, Signal, SignalBus, SignalBusConfig, SignalBusStats, SignalContext, SignalSubscriber
│   │
│   └── Tensor Registry (7)
│       └── ComposedTensor, ContributedTensor, ContributorInventoryEntry, ContributorKind, TensorContributor, TensorDimension, TensorRegistry
│
├── Concurrency Model
│   │
│   ├── Atomic Primitives
│   │   ├── GLOBAL_TICK: AtomicU64, Relaxed [Timestamp::now()]
│   │   └── ConfigManager.reload_flag: AtomicBool, SeqCst [SIGHUP handler]
│   │
│   ├── parking_lot::RwLock (5 uses)
│   │   ├── ConfigManager.config: RwLock<Config>
│   │   ├── Counter/Gauge/Histogram.values: RwLock<HashMap<Labels, AtomicU64>>
│   │   ├── MetricsRegistry.{counters,gauges,histograms}: RwLock<HashMap<String, Arc<T>>>
│   │   ├── SignalBus.subscribers: Arc<RwLock<Vec<Arc<dyn SignalSubscriber>>>>
│   │   └── SignalBus.stats: Arc<RwLock<SignalBusStats>>
│   │
│   ├── OnceLock (1 use)
│   │   └── LOGGING_INITIALIZED: OnceLock<bool> [set-once init guard]
│   │
│   ├── No Internal Sync (caller wraps)
│   │   ├── TensorRegistry: Vec<Arc<dyn>> + &mut self register
│   │   ├── ResourceManager: &mut self methods
│   │   └── All vocabulary types: Copy/Clone values
│   │
│   └── Lock Ordering Rule
│       └── SignalBus: subscribers lock BEFORE stats lock, guards dropped between (never nested)
│
├── 12D Tensor Dimension Map
│   │
│   ├── Active in L1 (10/12)
│   │   ├── D0 ServiceId ....... LogContext
│   │   ├── D1 Port ............ Config
│   │   ├── D2 Tier ............ All L1 types (1/6)
│   │   ├── D3 DependencyCount . (via StatePersistence db_count/11)
│   │   ├── D5 Protocol ........ LogContext, Resources
│   │   ├── D6 HealthScore ..... Config, Resources, Metrics, Error
│   │   ├── D9 Latency ......... Resources
│   │   └── D10 ErrorRate ...... Metrics, Resources, Error
│   │
│   ├── Unused in L1 (2/12) — activated by L2
│   │   ├── D4 AgentCount (activated by M09)
│   │   └── D7 Uptime (activated by M11)
│   │
│   ├── Never Used (2/12)
│   │   ├── D8 Synergy (no contributor in L1 or L2)
│   │   └── D11 TemporalContext (no contributor in L1 or L2)
│   │
│   ├── Freestanding Tensors (3)
│   │   ├── Config.to_tensor() ──→ D1, D2, D6
│   │   ├── MetricSnapshot.to_tensor() ──→ D2, D6, D10
│   │   └── SystemResources.to_tensor() ──→ D2, D5, D6, D9, D10
│   │
│   └── D6 Overlap (intentional)
│       ├── In L1: Config (always 1.0) + Resources (1-cpu) + Metrics (avg gauge)
│       └── In L2: M10 (probe-based) + M11 (% running) → averaged by compose()
│
├── Builder Patterns (10)
│   │
│   ├── ConfigBuilder ──→ build() -> Result<Config> [validates ports, log level]
│   ├── Labels ──→ fluent .service().layer()... [immutable chaining]
│   ├── QueryBuilder ──→ .select().from().where_eq()... → .build() -> &str
│   ├── StatePersistenceBuilder ──→ .with_database()... → .build().await -> Result
│   ├── DatabaseConfig ──→ .with_max_connections()... [const fn setters]
│   ├── CoverageBitmap ──→ .with_dimension()... [const fn, functional]
│   ├── HealthReport ──→ .with_details().with_timestamp() [consuming chain]
│   ├── AnnotatedError ──→ .with_origin().with_confidence() [consuming chain]
│   ├── LogContext ──→ .with_module().with_agent() [contextual factory]
│   └── Dissent ──→ .with_confidence().with_alternative() [const fn clamp]
│   │
│   └── Invariants
│       ├── All builder setters marked #[must_use]
│       ├── Terminal methods that can fail return Result
│       └── All const fn where compiler permits
│
├── Error Code Topology
│   │
│   ├── 1000 Config ........... config.rs, logging.rs
│   ├── 1100 Database ......... state.rs
│   ├── 1200 Network .......... (L2+)
│   ├── 1201 CircuitOpen ...... (L2+)
│   ├── 1202 Timeout .......... (L2+)
│   ├── 1300 ConsensusQuorum .. (L6)
│   ├── 1301 ViewChange ....... (L6)
│   ├── 1400 PathwayNotFound .. (L5+)
│   ├── 1401 TensorValidation . (L5+)
│   ├── 1500 Validation ....... config, metrics, resources
│   ├── 1600 Io ............... error.rs (From<std::io::Error>)
│   ├── 1700 Pipeline ......... (L3)
│   ├── 1800 ServiceNotFound .. (L2+)
│   ├── 1801 HealthCheckFailed  (L2+)
│   ├── 1802 EscalationRequired (L2+)
│   └── 1900 Other ............ resources.rs
│   │
│   ├── Classification Axes
│   │   ├── is_retryable(): Network, CircuitOpen, Timeout, ConsensusQuorum, Io(conditional), Database(conditional)
│   │   ├── is_transient(): Network, CircuitOpen, Timeout, ConsensusQuorum, Io(conditional)
│   │   └── severity(): Low(6), Medium(5), High(3), Critical(2)
│   │
│   └── Tensor Signal: to_tensor_signal()
│       ├── D6 = health: Critical→0.1, High→0.3, Medium→0.5, Low→0.8
│       ├── D2 = tier: maps error_category to tier weight
│       └── D10 = error_rate: Critical→0.9, High→0.7, Medium→0.5, Low→0.2
│
├── NAM Compliance Map
│   │
│   ├── R1 SelfQuery
│   │   ├── ErrorClassifier.is_retryable()
│   │   ├── Confidence.is_valid()
│   │   └── Dissent.is_valid()
│   │
│   ├── R2 HebbianRouting
│   │   ├── LearningSignal (Success→LTP, Failure→LTD, Partial→magnitude)
│   │   ├── Outcome enum
│   │   └── AdaptiveResourceLimits.pathway_strength
│   │
│   ├── R3 DissentCapture
│   │   ├── Dissent struct
│   │   ├── DissentEvent
│   │   └── SignalBus.emit_dissent()
│   │
│   ├── R4 FieldVisualization
│   │   ├── TensorContributor trait
│   │   ├── TensorRegistry.compose()
│   │   ├── ComposedTensor
│   │   ├── CoverageBitmap
│   │   └── build_foundation_tensor()
│   │
│   └── R5 HumanAsAgent
│       ├── AgentOrigin::Human + HUMAN_AGENT_TAG="@0.A"
│       ├── agent_id() default method on 5 traits (ConfigProvider, CorrelationProvider, StateStore, ResourceCollector, SignalSubscriber)
│       ├── From<&AgentOrigin> for AgentId
│       ├── Labels.agent() for metrics
│       └── save_with_provenance() for DB writes
│
├── Design Principles
│   │
│   ├── Copy where possible: ModuleId, Timestamp, DimensionIndex, CoverageBitmap, Severity, Outcome, Confidence
│   ├── All f64 outputs clamped [0.0, 1.0]: health scores, tensor dims, coverage ratios
│   ├── #[must_use] on every pure function and builder method: 200+ annotations
│   ├── const fn where possible: 57 const fn across L1
│   ├── Zero unsafe, unwrap, expect: compile-time #![forbid(unsafe_code)] + clippy deny
│   ├── No chrono, no SystemTime for temporal logic: Timestamp (atomic tick) + Duration
│   ├── Display on all public types: 27 implementations
│   └── Builder patterns with validation: ConfigBuilder.build() returns Result
│
├── Quality Gate Results
│   │
│   ├── cargo check ......................................... 0 errors
│   ├── cargo clippy -- -D warnings ........................ 0 warnings
│   ├── cargo clippy -- -D warnings -W clippy::pedantic .... 0 warnings
│   ├── cargo clippy -- -D warnings -W clippy::nursery ..... 0 warnings
│   ├── cargo test --lib m1_foundation ..................... 440 tests, 0 failures
│   └── Zero-tolerance grep (unsafe/unwrap/expect) ......... 0 hits
│
├── Test Taxonomy (440 tests across 11 files)
│   │
│   ├── mod.rs (71 tests, 10 groups)
│   │   ├── Group 1: Trait Importability & Object Safety (6)
│   │   ├── Group 2: ErrorClassifier Integration (8)
│   │   ├── Group 3: Config Integration (7)
│   │   ├── Group 4: Logging Integration (8)
│   │   ├── Group 5: Metrics Integration (8)
│   │   ├── Group 6: State/Persistence Integration (7)
│   │   ├── Group 7: Resources Integration (8)
│   │   ├── Group 8: Cross-Module & Re-export Integration (8)
│   │   ├── Group 9: Constants & Bucket Re-exports (3)
│   │   └── Group 10: NAM Type Re-export Completeness (12)
│   │       ├── Config integration → validation, change events, builder chain
│   │       ├── Logging integration → correlation IDs, levels, formats
│   │       ├── Metrics integration → counter/gauge/histogram lifecycle
│   │       ├── Resources integration → limits, alerts, health scoring
│   │       └── NAM integration → AgentOrigin, AnnotatedError, Foundation tensor
│   │
│   ├── shared_types.rs (~88 tests)
│   │   ├── ModuleId: construction, numbering, layer mapping, ALL array
│   │   ├── AgentId: factories, prefixes, query methods
│   │   ├── Timestamp: now() uniqueness, elapsed_since, within_window
│   │   ├── HealthReport: clamping, is_healthy, is_critical
│   │   ├── DimensionIndex: ALL variants, from_index/from_name round-trip
│   │   └── CoverageBitmap: union, intersection, coverage_ratio, EMPTY/FULL
│   │
│   ├── error.rs (~44 tests)
│   │   ├── All 16 variants: construction, display, clone, equality
│   │   ├── ErrorClassifier: retryable, transient, severity per variant
│   │   ├── From conversions: io::Error, String
│   │   ├── AnnotatedError: builder chain, origin, confidence
│   │   ├── Severity: ordering, display
│   │   └── to_tensor_signal: D6/D2/D10 mapping
│   │
│   ├── config.rs (14 tests)
│   │   ├── Config: defaults, builder, validation
│   │   ├── ConfigManager: get, reload, hot-reload flag
│   │   └── ConfigProvider trait: object safety
│   │
│   ├── logging.rs (15 tests)
│   │   ├── LogContext: new, with_context, child, agent
│   │   ├── LogLevel/LogFormat: parse, display, ordering
│   │   ├── Correlation IDs: uniqueness, format
│   │   └── init_logging: idempotency
│   │
│   ├── metrics.rs (12 tests)
│   │   ├── Counter/Gauge/Histogram: lifecycle, labels, reset
│   │   ├── MetricsRegistry: register, get, export
│   │   ├── Labels: builder, from_pairs, sorted invariant
│   │   └── MetricSnapshot: default, to_tensor
│   │
│   ├── state.rs (10 tests)
│   │   ├── DatabaseType: all variants, filenames, migration numbers
│   │   ├── DatabaseConfig: builder, defaults
│   │   ├── QueryBuilder: SELECT/INSERT/UPDATE/DELETE, params
│   │   └── PoolStats: construction
│   │
│   ├── resources.rs (16 tests)
│   │   ├── ResourceLimits: defaults, custom, validation
│   │   ├── ResourceManager: new, with_limits, health_score
│   │   ├── ResourceAlert: display all variants
│   │   ├── check_limits: under/over threshold
│   │   └── AdaptiveResourceLimits: effective_limits
│   │
│   ├── nam.rs (35 tests)
│   │   ├── AgentOrigin: all 4 variants, factories, display
│   │   ├── Confidence: certain, uncertain, new, clamping, is_valid
│   │   ├── Outcome: all 3 variants, display
│   │   ├── LearningSignal: success/failure/partial, with_pathway
│   │   └── Dissent: new, with_confidence, with_alternative, is_valid
│   │
│   ├── signals.rs (~55 tests)
│   │   ├── SignalBus: subscribe, emit, capacity limit
│   │   ├── HealthSignal: is_degradation/improvement, delta
│   │   ├── LearningEvent/DissentEvent: construction
│   │   ├── SignalBusStats: total_emitted
│   │   └── Locking: concurrent subscribe + emit safety
│   │
│   └── tensor_registry.rs (~80 tests)
│       ├── TensorContributor: object safety, contribute
│       ├── ContributedTensor: dimension_value, coverage
│       ├── ComposedTensor: coverage_ratio, dead_dimensions
│       ├── TensorRegistry: register, compose, compose_filtered
│       ├── Composition algorithm: averaging, clamping, union coverage
│       └── ContributorInventoryEntry: display, fields
│
├── Cross-Layer Export Boundaries (L1 → L2+)
│   │
│   ├── Core Types → ALL layers
│   │   ├── Error, Result<T>
│   │   ├── Timestamp
│   │   └── ModuleId
│   │
│   ├── NAM Types → L3, L5, L6
│   │   ├── AgentOrigin → L3, L6
│   │   ├── Confidence → L3
│   │   ├── Outcome → L3, L5
│   │   ├── LearningSignal → L5
│   │   └── Dissent → L6
│   │
│   ├── Cross-Cutting → L2
│   │   ├── SignalBus → Arc<dyn SignalBusOps>
│   │   ├── TensorContributor → impl on M09-M12
│   │   ├── CoverageBitmap → tensor composition
│   │   └── HealthSignal → health transitions
│   │
│   └── Direction Rule: Strictly downward (L7→L1). No L1 module imports from L2+.
│
├── Clippy Allowances (documented)
│   │
│   ├── resources.rs
│   │   ├── cast_precision_loss: u64 → f64 for percentage
│   │   └── cast_possible_truncation: u64 → u32 for fd count
│   │
│   └── metrics.rs
│       ├── cast_possible_truncation, cast_sign_loss, cast_precision_loss, cast_possible_wrap
│       │   └── Fixed-point arithmetic for gauge f64 storage
│       └── format_push_string: Prometheus text format export
│
└── Signal Flow Topology
    │
    ├── Sources (L2-L6 emit into L1 bus)
    │   ├── M09-M12: Health transitions
    │   ├── M25-M30: Learning events
    │   └── M31-M36: Dissent events
    │
    ├── Bus (SignalBus, M07)
    │   ├── emit_health() → synchronous, in-order delivery
    │   ├── emit_learning() → synchronous, in-order delivery
    │   └── emit_dissent() → synchronous, in-order delivery
    │
    ├── Consumers (subscribe via Arc<dyn SignalSubscriber>)
    │   ├── L7 Observer → on_health()
    │   ├── L5 Hebbian → on_learning()
    │   ├── L6 Consensus → on_dissent()
    │   └── L3 Pipeline → on_health()
    │
    └── Locking Protocol
        ├── Read subscribers → call callbacks → drop guard
        └── Write stats (AFTER guard drop — deadlock prevention)
```

---

## Relationship Matrix (Internal Dependencies)

```
                mod  shared  error  config  logging  metrics  state  resources  nam  signals  tensor_reg
mod.rs          --   re-exp  re-exp re-exp  re-exp   re-exp   re-exp re-exp     re-exp re-exp re-exp
shared_types.rs  .   --      .      .       .        .        .      .          .    .        .
error.rs         .   .       --     .       .        .        .      .          ←    .        .
config.rs        .   ←       ←      --      .        .        .      .          .    .        .
logging.rs       .   ←       ←      .       --       .        .      .          .    .        .
metrics.rs       .   ←       ←      .       .        --       .      .          .    .        .
state.rs         .   ←       ←      ←       .        .        --     .          .    .        .
resources.rs     .   ←       ←      .       .        .        .      --         .    .        .
nam.rs           .   ←       .      .       .        .        .      .          --   .        .
signals.rs       .   ←       ←      .       .        .        .      .          ←    --       .
tensor_reg.rs    .   ←       .      .       .        .        .      .          .    .        --

Legend: ← = depends on (column imports from row)
        . = no dependency
```

---

## Statistics Summary

| Category | Count |
|----------|-------|
| Source files | 11 |
| Modules | 9 (M00-M08) |
| Total LOC | ~12,908 |
| Total tests | 440 |
| Traits | 8 |
| Trait methods | 32 (total), 11 with defaults |
| Public types | ~50 (structs + enums) |
| Public functions | ~35 (free functions) |
| Constants | ~55 (ModuleId M01-M42 + buckets + NAM + paths) |
| Builder patterns | 10 |
| Error variants | 16 (codes 1000-1900) |
| Tensor dimensions | 12 (10 active in L1) |
| Display impls | 27 |
| const fn | 57 |
| #[must_use] annotations | 200+ |
| Concurrency primitives | AtomicU64(1), AtomicBool(1), RwLock(5), OnceLock(1) |
| NAM requirements covered | 5/5 (R1-R5) |
| Clippy warnings | 0 (pedantic + nursery) |
| unsafe blocks | 0 (compile-time forbidden) |
| unwrap/expect | 0 (clippy denied) |

---

*L1 Foundation Meta Tree Mind Map v1.0 | 2026-03-01*
*Derived from 11 source files + 13 spec sheets (12-META-TREE-MIND-MAP.md)*
