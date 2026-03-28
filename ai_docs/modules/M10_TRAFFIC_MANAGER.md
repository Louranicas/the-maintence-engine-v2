# Module M10: Traffic Manager

> **M10_TRAFFIC_MANAGER** | Intelligent Request Routing & Traffic Shaping | Layer: L2 Services | [Back to Index](../INDEX.md)

---

## Navigation

| Direction | Link |
|-----------|------|
| Up | [INDEX.md](../INDEX.md) |
| Layer | [L02_SERVICES.md](../layers/L02_SERVICES.md) |
| Previous | [M09_SERVICE_MESH_CONTROLLER.md](M09_SERVICE_MESH_CONTROLLER.md) |
| Next | [M11_LOAD_BALANCER.md](M11_LOAD_BALANCER.md) |
| Related | [M12_CIRCUIT_BREAKER.md](M12_CIRCUIT_BREAKER.md) |
| L1 Foundation | [L01_FOUNDATION.md](../layers/L01_FOUNDATION.md) |
| L3 Learning | [L03_LEARNING.md](../layers/L03_LEARNING.md) |

---

## Module Specification

### Overview

The Traffic Manager module handles intelligent request routing, traffic splitting, canary deployments, A/B testing, and traffic shaping across the Maintenance Engine. It works closely with M09 Mesh Controller for policy enforcement and M11 Load Balancer for endpoint distribution, providing the routing intelligence layer of the L2 Services.

### Module Properties

| Property | Value |
|----------|-------|
| Module ID | M10 |
| Module Name | Traffic Manager |
| Layer | L2 (Services) |
| Version | 1.0.0 |
| Dependencies | M07 (Health Monitor), M08 (Service Discovery), M09 (Mesh Controller) |
| Dependents | M11 (Load Balancer), M12 (Circuit Breaker), L3 (Learning), L5 (Remediation) |

---

## Architecture Diagram

```
+-----------------------------------------------------------------------------------+
|                           M10: TRAFFIC MANAGER                                     |
+-----------------------------------------------------------------------------------+
|                                                                                   |
|  +-------------------------+    +-------------------------+    +--------------+   |
|  |   ROUTING ENGINE        |    |   TRAFFIC SPLITTER      |    | RATE CTRL    |   |
|  |                         |    |                         |    |              |   |
|  | - Route matching        |    | - Weighted routing      |    | - Request    |   |
|  | - Path rewriting        |--->| - Canary releases       |--->|   throttling |   |
|  | - Header manipulation   |    | - A/B testing           |    | - Burst      |   |
|  | - Priority routing      |    | - Shadow traffic        |    |   control    |   |
|  +------------+------------+    +------------+------------+    +--------------+   |
|               |                              |                        |           |
|               v                              v                        v           |
|  +-------------------------+    +-------------------------+    +--------------+   |
|  |   VIRTUAL SERVICE       |    |   DESTINATION RULES     |    | FAULT INJECT |   |
|  |                         |    |                         |    |              |   |
|  | - Host matching         |    | - Subset definitions    |    | - Delay      |   |
|  | - HTTP route matching   |    | - Version routing       |    | - Abort      |   |
|  | - gRPC route matching   |    | - Traffic policies      |    | - Chaos eng  |   |
|  +-------------------------+    +-------------------------+    +--------------+   |
|                                                                                   |
+-----------------------------------------------------------------------------------+
        ^                    ^                    ^                    |
        |                    |                    |                    v
   [M09: Mesh]         [M08: Discovery]    [M07: Health]         [M11: LB]
```

---

## Core Data Structures

### Virtual Service

```rust
/// Virtual service definition for traffic routing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VirtualService {
    /// Virtual service identifier
    pub id: VirtualServiceId,

    /// Service name
    pub name: String,

    /// Host names to match
    pub hosts: Vec<String>,

    /// Gateways to attach (for ingress)
    pub gateways: Vec<GatewayId>,

    /// HTTP routes (ordered by priority)
    pub http: Vec<HttpRoute>,

    /// TCP routes
    pub tcp: Vec<TcpRoute>,

    /// gRPC routes
    pub grpc: Vec<GrpcRoute>,

    /// Export to namespaces
    pub export_to: Vec<String>,

    /// Creation timestamp
    pub created_at: DateTime<Utc>,

    /// Last modified timestamp
    pub modified_at: DateTime<Utc>,
}

/// HTTP route definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpRoute {
    /// Route name
    pub name: String,

    /// Match conditions (any must match)
    pub matches: Vec<HttpMatchRequest>,

    /// Route destinations
    pub route: Vec<RouteDestination>,

    /// Redirect (mutually exclusive with route)
    pub redirect: Option<HttpRedirect>,

    /// Rewrite configuration
    pub rewrite: Option<HttpRewrite>,

    /// Timeout for this route
    pub timeout: Option<Duration>,

    /// Retry policy override
    pub retries: Option<HttpRetry>,

    /// Fault injection
    pub fault: Option<FaultInjection>,

    /// CORS policy
    pub cors_policy: Option<CorsPolicy>,

    /// Headers manipulation
    pub headers: Option<HeadersManipulation>,

    /// Mirror traffic
    pub mirror: Option<MirrorDestination>,

    /// Route priority (higher = more specific)
    pub priority: u32,
}

/// HTTP match request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpMatchRequest {
    /// URI match
    pub uri: Option<StringMatch>,

    /// Scheme match (http, https)
    pub scheme: Option<StringMatch>,

    /// Method match
    pub method: Option<StringMatch>,

    /// Authority/host match
    pub authority: Option<StringMatch>,

    /// Header matches
    pub headers: HashMap<String, StringMatch>,

    /// Query parameter matches
    pub query_params: HashMap<String, StringMatch>,

    /// Source labels (caller metadata)
    pub source_labels: HashMap<String, String>,

    /// Port to match
    pub port: Option<u16>,

    /// Ignore URI case
    pub ignore_uri_case: bool,
}

/// String matching modes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StringMatch {
    Exact(String),
    Prefix(String),
    Regex(String),
}
```

### Destination Rule

```rust
/// Destination rule for subset and policy configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DestinationRule {
    /// Rule identifier
    pub id: DestinationRuleId,

    /// Target host
    pub host: String,

    /// Traffic policy (default)
    pub traffic_policy: Option<TrafficPolicy>,

    /// Subsets (versioned endpoints)
    pub subsets: Vec<Subset>,

    /// Export to namespaces
    pub export_to: Vec<String>,
}

/// Subset definition for version-based routing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subset {
    /// Subset name
    pub name: String,

    /// Labels to match endpoints
    pub labels: HashMap<String, String>,

    /// Subset-specific traffic policy
    pub traffic_policy: Option<TrafficPolicy>,
}

/// Route destination
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteDestination {
    /// Destination host
    pub host: String,

    /// Destination subset
    pub subset: Option<String>,

    /// Destination port
    pub port: Option<PortSelector>,

    /// Weight (for traffic splitting)
    pub weight: u32,

    /// Headers to add/remove
    pub headers: Option<HeadersManipulation>,
}
```

### Traffic Split Configuration

```rust
/// Traffic split for canary/A-B testing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrafficSplit {
    /// Split identifier
    pub id: TrafficSplitId,

    /// Split name
    pub name: String,

    /// Target service
    pub service: ServiceId,

    /// Split type
    pub split_type: TrafficSplitType,

    /// Destination weights
    pub backends: Vec<TrafficSplitBackend>,

    /// Split strategy
    pub strategy: SplitStrategy,

    /// Current state
    pub state: TrafficSplitState,

    /// Rollout configuration (for canary)
    pub rollout: Option<RolloutConfig>,

    /// Analysis configuration
    pub analysis: Option<AnalysisConfig>,

    /// Creation timestamp
    pub created_at: DateTime<Utc>,
}

/// Traffic split types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrafficSplitType {
    /// Percentage-based split
    Weighted,
    /// Canary deployment with progressive rollout
    Canary,
    /// A/B testing based on user attributes
    ABTest,
    /// Blue-green deployment
    BlueGreen,
    /// Shadow/mirroring traffic
    Shadow,
}

/// Traffic split backend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrafficSplitBackend {
    /// Backend service/version
    pub service: ServiceId,

    /// Version/subset
    pub version: String,

    /// Current weight (0-100)
    pub weight: u32,

    /// Target weight (for progressive rollout)
    pub target_weight: Option<u32>,
}

/// Split strategy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SplitStrategy {
    /// Round-robin based on weight
    WeightedRoundRobin,
    /// Consistent hash for sticky sessions
    ConsistentHash { key: HashKey },
    /// Header-based routing
    HeaderBased { header: String, values: HashMap<String, ServiceId> },
    /// Cookie-based routing
    CookieBased { cookie: String, values: HashMap<String, ServiceId> },
    /// User attribute-based
    UserAttributeBased { attribute: String, rules: Vec<AttributeRule> },
}

/// Hash key for consistent hashing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HashKey {
    Header(String),
    Cookie(String),
    SourceIp,
    QueryParam(String),
    User,
}
```

### Fault Injection

```rust
/// Fault injection for chaos engineering
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FaultInjection {
    /// Delay fault
    pub delay: Option<DelayFault>,

    /// Abort fault
    pub abort: Option<AbortFault>,
}

/// Delay fault configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DelayFault {
    /// Percentage of requests to delay
    pub percentage: f64,

    /// Fixed delay duration
    pub fixed_delay: Duration,

    /// Exponential delay (mean)
    pub exponential_delay: Option<Duration>,
}

/// Abort fault configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AbortFault {
    /// Percentage of requests to abort
    pub percentage: f64,

    /// HTTP status code to return
    pub http_status: Option<u16>,

    /// gRPC status code to return
    pub grpc_status: Option<i32>,
}
```

### Rate Limiting

```rust
/// Rate limiting configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    /// Rate limit identifier
    pub id: RateLimitId,

    /// Target service or route
    pub target: RateLimitTarget,

    /// Rate limit rules
    pub rules: Vec<RateLimitRule>,

    /// Default action when limit exceeded
    pub default_action: RateLimitAction,

    /// Response headers to add
    pub response_headers: HashMap<String, String>,
}

/// Rate limit target
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RateLimitTarget {
    Service(ServiceId),
    VirtualService(VirtualServiceId),
    Route(String),
    Global,
}

/// Rate limit rule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitRule {
    /// Rule name
    pub name: String,

    /// Match conditions
    pub matches: Vec<RateLimitMatch>,

    /// Request limit
    pub limit: RequestLimit,

    /// Action when exceeded
    pub action: RateLimitAction,
}

/// Request limit definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestLimit {
    /// Requests per unit
    pub requests: u64,

    /// Time unit
    pub per: RateLimitUnit,

    /// Burst allowance
    pub burst: u64,
}

/// Rate limit time units
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RateLimitUnit {
    Second,
    Minute,
    Hour,
    Day,
}

/// Rate limit action
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RateLimitAction {
    /// Return 429 Too Many Requests
    Reject,
    /// Queue request
    Queue { max_queue_size: usize, queue_timeout: Duration },
    /// Log but allow
    LogOnly,
    /// Delay response
    Delay { duration: Duration },
}
```

---

## Public API

### TrafficManager Service

```rust
/// Main Traffic Manager service
pub struct TrafficManager {
    config: TrafficManagerConfig,
    routing_engine: RoutingEngine,
    traffic_splitter: TrafficSplitter,
    rate_controller: RateController,
    fault_injector: FaultInjector,
    metrics: TrafficManagerMetrics,
}

impl TrafficManager {
    /// Create a new TrafficManager instance
    pub fn new(config: TrafficManagerConfig) -> Self;

    /// Start the traffic manager
    pub async fn start(&mut self) -> Result<(), TrafficError>;

    /// Stop the traffic manager
    pub async fn stop(&mut self) -> Result<(), TrafficError>;

    // === Virtual Service API ===

    /// Create a virtual service
    pub async fn create_virtual_service(&mut self, vs: VirtualService) -> Result<VirtualServiceId, TrafficError>;

    /// Update a virtual service
    pub async fn update_virtual_service(&mut self, vs: VirtualService) -> Result<(), TrafficError>;

    /// Delete a virtual service
    pub async fn delete_virtual_service(&mut self, id: &VirtualServiceId) -> Result<(), TrafficError>;

    /// Get virtual service by ID
    pub fn get_virtual_service(&self, id: &VirtualServiceId) -> Option<&VirtualService>;

    /// List virtual services
    pub fn list_virtual_services(&self) -> Vec<&VirtualService>;

    // === Destination Rule API ===

    /// Create a destination rule
    pub async fn create_destination_rule(&mut self, dr: DestinationRule) -> Result<DestinationRuleId, TrafficError>;

    /// Update a destination rule
    pub async fn update_destination_rule(&mut self, dr: DestinationRule) -> Result<(), TrafficError>;

    /// Delete a destination rule
    pub async fn delete_destination_rule(&mut self, id: &DestinationRuleId) -> Result<(), TrafficError>;

    /// Get destination rule by ID
    pub fn get_destination_rule(&self, id: &DestinationRuleId) -> Option<&DestinationRule>;

    // === Traffic Split API ===

    /// Create a traffic split
    pub async fn create_traffic_split(&mut self, split: TrafficSplit) -> Result<TrafficSplitId, TrafficError>;

    /// Update traffic split weights
    pub async fn update_split_weights(&mut self, id: &TrafficSplitId, weights: Vec<(String, u32)>) -> Result<(), TrafficError>;

    /// Progress canary rollout
    pub async fn progress_canary(&mut self, id: &TrafficSplitId, increment: u32) -> Result<(), TrafficError>;

    /// Rollback traffic split
    pub async fn rollback_split(&mut self, id: &TrafficSplitId) -> Result<(), TrafficError>;

    /// Complete traffic split (100% to target)
    pub async fn complete_split(&mut self, id: &TrafficSplitId) -> Result<(), TrafficError>;

    /// Get traffic split status
    pub fn get_split_status(&self, id: &TrafficSplitId) -> Option<TrafficSplitStatus>;

    // === Rate Limiting API ===

    /// Create rate limit
    pub async fn create_rate_limit(&mut self, config: RateLimitConfig) -> Result<RateLimitId, TrafficError>;

    /// Update rate limit
    pub async fn update_rate_limit(&mut self, config: RateLimitConfig) -> Result<(), TrafficError>;

    /// Delete rate limit
    pub async fn delete_rate_limit(&mut self, id: &RateLimitId) -> Result<(), TrafficError>;

    /// Check rate limit for request
    pub fn check_rate_limit(&self, request: &RequestContext) -> RateLimitDecision;

    // === Fault Injection API ===

    /// Enable fault injection
    pub async fn enable_fault(&mut self, route: &str, fault: FaultInjection) -> Result<(), TrafficError>;

    /// Disable fault injection
    pub async fn disable_fault(&mut self, route: &str) -> Result<(), TrafficError>;

    /// List active faults
    pub fn list_active_faults(&self) -> Vec<(&str, &FaultInjection)>;

    // === Routing API ===

    /// Route a request
    pub fn route(&self, request: &RequestContext) -> RoutingDecision;

    /// Get routing table
    pub fn get_routing_table(&self) -> RoutingTable;

    /// Validate routing configuration
    pub fn validate_routing(&self) -> ValidationResult;
}
```

### RoutingEngine API

```rust
/// Handles request routing decisions
pub struct RoutingEngine {
    /// Match request to route
    pub fn match_route(&self, request: &RequestContext) -> Option<&HttpRoute>;

    /// Get destination for route
    pub fn get_destination(&self, route: &HttpRoute, request: &RequestContext) -> RouteDestination;

    /// Apply header manipulation
    pub fn apply_headers(&self, route: &HttpRoute, headers: &mut Headers);

    /// Apply URI rewrite
    pub fn apply_rewrite(&self, route: &HttpRoute, uri: &mut Uri);

    /// Get effective timeout
    pub fn get_timeout(&self, route: &HttpRoute) -> Duration;

    /// Get retry policy
    pub fn get_retry_policy(&self, route: &HttpRoute) -> Option<&HttpRetry>;
}
```

---

## Configuration

### TOML Configuration Example

```toml
[layer.L2.M10]
enabled = true
version = "1.0.0"

# Global settings
[layer.L2.M10.global]
default_timeout_ms = 15000
default_retries = 3
enable_fault_injection = false
enable_traffic_mirroring = false

# Routing engine settings
[layer.L2.M10.routing]
route_cache_enabled = true
route_cache_ttl_ms = 5000
max_routes = 10000
match_timeout_ms = 10

# Traffic splitting defaults
[layer.L2.M10.splitting]
canary_increment_percent = 10
canary_interval_minutes = 5
rollback_on_error_rate = 0.1
analysis_enabled = true

# Rate limiting defaults
[layer.L2.M10.rate_limiting]
enabled = true
default_limit = 1000
default_per = "second"
default_burst = 100
response_headers = true

# Fault injection (disabled by default)
[layer.L2.M10.fault_injection]
enabled = false
max_delay_ms = 5000
allowed_abort_codes = [500, 502, 503, 504]

# Virtual services
[[layer.L2.M10.virtual_services]]
name = "synthex-routes"
hosts = ["synthex.local", "localhost"]
[layer.L2.M10.virtual_services.http]
name = "api-route"
match_uri_prefix = "/api"
destination_host = "synthex"
destination_port = 8090
timeout_ms = 30000

[[layer.L2.M10.virtual_services]]
name = "san-k7-routes"
hosts = ["san-k7.local"]
[layer.L2.M10.virtual_services.http]
name = "health-route"
match_uri_exact = "/health"
destination_host = "san-k7"
destination_port = 8100

# Destination rules
[[layer.L2.M10.destination_rules]]
host = "synthex"
[layer.L2.M10.destination_rules.subsets]
name = "v1"
labels = { version = "1.0.0" }
[layer.L2.M10.destination_rules.subsets]
name = "v2-canary"
labels = { version = "2.0.0" }

# Traffic splits
[[layer.L2.M10.traffic_splits]]
name = "synthex-canary"
service = "synthex"
type = "canary"
[[layer.L2.M10.traffic_splits.backends]]
version = "v1"
weight = 90
[[layer.L2.M10.traffic_splits.backends]]
version = "v2-canary"
weight = 10

# Rate limits
[[layer.L2.M10.rate_limits]]
name = "global-limit"
target = "global"
requests = 10000
per = "second"
burst = 1000
```

---

## Bi-Directional Data Flow

### Inbound Data Flow

The Traffic Manager receives routing configuration, health data, and traffic policies from multiple sources.

#### Inbound Message Types

```rust
/// Messages received by Traffic Manager
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TrafficManagerInbound {
    // From L1 Config (M02)
    ConfigUpdate {
        virtual_services: Vec<VirtualService>,
        destination_rules: Vec<DestinationRule>,
        traffic_splits: Vec<TrafficSplit>,
        rate_limits: Vec<RateLimitConfig>,
        timestamp: DateTime<Utc>,
    },

    // From L1 State (M05)
    StateRestored {
        routing_table: RoutingTable,
        active_splits: Vec<TrafficSplit>,
        rate_limit_counters: HashMap<RateLimitId, RateLimitState>,
        timestamp: DateTime<Utc>,
    },

    // From M07 Health Monitor
    EndpointHealthUpdate {
        service_id: ServiceId,
        endpoint_health: HashMap<EndpointId, HealthState>,
        timestamp: DateTime<Utc>,
    },

    // From M08 Service Discovery
    RoutingTableUpdate {
        routes: Vec<RouteDefinition>,
        timestamp: DateTime<Utc>,
    },

    ServiceEndpointsChanged {
        service_id: ServiceId,
        endpoints: Vec<Endpoint>,
        timestamp: DateTime<Utc>,
    },

    // From M09 Mesh Controller
    TrafficPolicyUpdate {
        policies: Vec<TrafficPolicy>,
        routing_rules: Vec<RoutingRule>,
        timestamp: DateTime<Utc>,
    },

    // From L3 Learning
    TrafficPrediction {
        service_id: ServiceId,
        predicted_load: f64,
        predicted_time: DateTime<Utc>,
        confidence: f64,
        timestamp: DateTime<Utc>,
    },

    CanaryAnalysisResult {
        split_id: TrafficSplitId,
        analysis: CanaryAnalysis,
        recommendation: CanaryRecommendation,
        timestamp: DateTime<Utc>,
    },

    // From L5 Remediation
    TrafficShift {
        service_id: ServiceId,
        action: TrafficShiftAction,
        reason: String,
        timestamp: DateTime<Utc>,
    },

    RateLimitOverride {
        target: RateLimitTarget,
        new_limit: RequestLimit,
        duration: Duration,
        reason: String,
        timestamp: DateTime<Utc>,
    },
}

/// Canary analysis result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanaryAnalysis {
    pub error_rate_baseline: f64,
    pub error_rate_canary: f64,
    pub latency_baseline_p99: Duration,
    pub latency_canary_p99: Duration,
    pub sample_size: u64,
    pub statistical_significance: f64,
}

/// Canary recommendation
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum CanaryRecommendation {
    Continue,
    ProgressRollout,
    Pause,
    Rollback,
    Complete,
}

/// Traffic shift action
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TrafficShiftAction {
    DrainService,
    RestoreService,
    ShiftToBackup { backup_service: ServiceId },
    ReduceLoad { percentage: u32 },
}
```

#### Inbound Flow Sources

| Source | Message Type | Trigger Condition | Frequency |
|--------|-------------|-------------------|-----------|
| L1 Config (M02) | ConfigUpdate | Config file change | On change |
| L1 State (M05) | StateRestored | System startup | On startup |
| M07 Health | EndpointHealthUpdate | Health change | On change |
| M08 Discovery | RoutingTableUpdate | Route change | On change |
| M08 Discovery | ServiceEndpointsChanged | Endpoint change | On change |
| M09 Mesh | TrafficPolicyUpdate | Policy change | On change |
| L3 Learning | TrafficPrediction | ML prediction | Periodic |
| L3 Learning | CanaryAnalysisResult | Analysis complete | On analysis |
| L5 Remediation | TrafficShift | Remediation action | On action |
| L5 Remediation | RateLimitOverride | Emergency throttle | On action |

#### Inbound Sequence Diagram

```
  L1:Config   M07:Health   M08:Discovery   M09:Mesh   L3:Learning   L5:Remediation
      |           |             |              |            |              |
      | ConfigUpdate            |              |            |              |
      |---------->|             |              |            |              |
      |           |             |              |            |              |
      |           | HealthUpdate|              |            |              |
      |           |------------>|              |            |              |
      |           |             |              |            |              |
      |           |             | RoutingTableUpdate        |              |
      |           |             |------------->|            |              |
      |           |             |              |            |              |
      |           |             |              | PolicyUpdate             |
      |           |             |              |----------->|              |
      |           |             |              |            |              |
      |           |             |              |            | CanaryAnalysis
      |           |             |              |            |------------->|
      |           |             |              |            |              |
      |           |             |              |            |    TrafficShift
      |           |             |              |            |<-------------|
      |           |             |              |            |              |
      +-----------+-------------+--------------+------------+--------------+
                                       |
                                       v
                             +-------------------+
                             |  M10 TRAFFIC      |
                             |  MANAGER          |
                             |                   |
                             | - Update routes   |
                             | - Apply policies  |
                             | - Execute shifts  |
                             +-------------------+
```

### Outbound Data Flow

The Traffic Manager emits routing decisions, split progress, and traffic metrics.

#### Outbound Message Types

```rust
/// Messages emitted by Traffic Manager
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TrafficManagerOutbound {
    // To L1 Metrics (M04)
    TrafficMetrics {
        service_id: ServiceId,
        requests_total: u64,
        requests_per_second: f64,
        error_rate: f64,
        latency_p50: Duration,
        latency_p99: Duration,
        rate_limited_count: u64,
        timestamp: DateTime<Utc>,
    },

    // To L1 State (M05)
    StatePersist {
        routing_table: RoutingTable,
        active_splits: Vec<TrafficSplit>,
        rate_limit_state: HashMap<RateLimitId, RateLimitState>,
        timestamp: DateTime<Utc>,
    },

    // To M11 Load Balancer
    LoadBalancerUpdate {
        service_id: ServiceId,
        endpoint_weights: HashMap<EndpointId, u32>,
        routing_context: RoutingContext,
        timestamp: DateTime<Utc>,
    },

    // To M12 Circuit Breaker
    TrafficHealthReport {
        service_id: ServiceId,
        error_rate: f64,
        timeout_rate: f64,
        latency_trend: LatencyTrend,
        timestamp: DateTime<Utc>,
    },

    // To L3 Learning
    TrafficEvent {
        event_type: TrafficEventType,
        service_id: ServiceId,
        details: TrafficEventDetails,
        timestamp: DateTime<Utc>,
    },

    CanaryMetrics {
        split_id: TrafficSplitId,
        baseline_metrics: VersionMetrics,
        canary_metrics: VersionMetrics,
        timestamp: DateTime<Utc>,
    },

    // To L5 Remediation
    TrafficAlert {
        alert_type: TrafficAlertType,
        service_id: ServiceId,
        severity: Severity,
        details: String,
        suggested_action: Option<TrafficShiftAction>,
        timestamp: DateTime<Utc>,
    },

    SplitProgressReport {
        split_id: TrafficSplitId,
        current_weights: Vec<(String, u32)>,
        state: TrafficSplitState,
        timestamp: DateTime<Utc>,
    },

    // To L6 Integration
    RoutingTableExport {
        virtual_services: Vec<VirtualService>,
        destination_rules: Vec<DestinationRule>,
        active_splits: Vec<TrafficSplit>,
        timestamp: DateTime<Utc>,
    },
}

/// Traffic event types for L3 learning
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TrafficEventType {
    RouteMatched,
    RateLimitTriggered,
    FaultInjected,
    TrafficSplitProgressed,
    CanaryStarted,
    CanaryCompleted,
    CanaryRolledBack,
    TrafficShifted,
}

/// Traffic alert types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TrafficAlertType {
    HighErrorRate,
    LatencyDegradation,
    RateLimitExhausted,
    CanaryUnhealthy,
    TrafficImbalance,
    RoutingFailure,
}

/// Version-specific metrics for canary analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionMetrics {
    pub version: String,
    pub request_count: u64,
    pub error_count: u64,
    pub error_rate: f64,
    pub latency_p50: Duration,
    pub latency_p99: Duration,
}
```

#### Outbound Flow Targets

| Target | Message Type | Trigger Condition | Priority |
|--------|-------------|-------------------|----------|
| L1 Metrics (M04) | TrafficMetrics | Periodic collection | Normal |
| L1 State (M05) | StatePersist | State change | High |
| M11 Load Balancer | LoadBalancerUpdate | Weight change | High |
| M12 Circuit Breaker | TrafficHealthReport | Health degradation | High |
| L3 Learning | TrafficEvent | Significant events | Normal |
| L3 Learning | CanaryMetrics | Canary analysis | Normal |
| L5 Remediation | TrafficAlert | Threshold breached | Critical |
| L5 Remediation | SplitProgressReport | Split progress | Normal |
| L6 Integration | RoutingTableExport | Periodic/on-demand | Low |

#### Outbound Sequence Diagram

```
                             +-------------------+
                             |  M10 TRAFFIC      |
                             |  MANAGER          |
                             +--------+----------+
                                      |
       +------------------------------+------------------------------+
       |              |               |               |              |
       v              v               v               v              v
  +---------+   +---------+     +---------+     +---------+    +---------+
  |L1:Metrics|  |M11:LB   |     |M12:CB   |     |L3:Learn |    |L5:Remed |
  +---------+   +---------+     +---------+     +---------+    +---------+
       |              |               |               |              |
       |              |               |               |              |
       +------------------------------+------------------------------+
                                      |
       +------------------------------+------------------------------+
       |              |               |               |              |
       v              v               v               v              v
  +---------+   +---------+     +---------+     +---------+    +---------+
  |L1:State |   |L6:Integr|    |Svc Mesh |     |Canary UI|    |Alerts   |
  +---------+   +---------+     +---------+     +---------+    +---------+
```

### Cross-Module Dependencies

#### Bidirectional Dependency Matrix

| Module | Reads From M10 | Writes To M10 | Sync/Async | Error Path |
|--------|---------------|---------------|------------|------------|
| M02 Config | - | ConfigUpdate | Async | Use defaults |
| M04 Metrics | TrafficMetrics | - | Async | Skip metrics |
| M05 State | StatePersist | StateRestored | Async | Start fresh |
| M07 Health | - | EndpointHealthUpdate | Async | Ignore health |
| M08 Discovery | - | RoutingTableUpdate, EndpointsChanged | Async | Static routes |
| M09 Mesh | - | TrafficPolicyUpdate | Async | Default policies |
| M11 Load Balancer | LoadBalancerUpdate | - | Async | Equal weights |
| M12 Circuit Breaker | TrafficHealthReport | - | Async | No CB |
| L3 Learning | TrafficEvent, CanaryMetrics | TrafficPrediction, CanaryAnalysis | Async | Ignore predictions |
| L5 Remediation | TrafficAlert, SplitProgress | TrafficShift, RateLimitOverride | Async | Manual alert |

#### Communication Patterns

```rust
/// Communication patterns for M10
pub struct TrafficManagerComms {
    // Synchronous queries
    sync_queries: SyncQueryHandler,

    // Asynchronous events
    async_events: AsyncEventEmitter,

    // Streaming metrics
    metrics_stream: MetricsStreamer,

    // Request routing (hot path)
    routing_engine: RoutingEngine,
}

impl TrafficManagerComms {
    /// Synchronous: Route request (hot path)
    pub fn route_request(&self, ctx: &RequestContext) -> RoutingDecision {
        // Must be fast - cached route matching
        self.routing_engine.match_and_route(ctx)
    }

    /// Synchronous: Check rate limit (hot path)
    pub fn check_rate_limit(&self, ctx: &RequestContext) -> RateLimitDecision {
        self.rate_controller.check(ctx)
    }

    /// Asynchronous: Emit traffic events
    pub async fn emit_event(&self, event: TrafficManagerOutbound) {
        self.async_events.publish(event).await;
    }

    /// Streaming: Continuous traffic metrics
    pub fn metrics_stream(&self) -> impl Stream<Item = TrafficMetrics> {
        self.metrics_stream.subscribe()
    }
}
```

#### Error Propagation Paths

```
M10 Traffic Error
       |
       +---> [Log to M03 Logging] ---> Structured log entry
       |
       +---> [Encode via M01 Error Taxonomy] ---> 11D Error Vector
       |            |
       |            +---> [Send to L3 Learning] ---> Pattern recognition
       |
       +---> [Alert to L5 Remediation] ---> Trigger traffic remediation
       |
       +---> [Update M12 Circuit Breaker] ---> If error rate high
```

### Contextual Flow: Canary Deployment Lifecycle

#### Traffic Split State Machine

```
                                 +-------------------+
                                 |     CREATED       |
                                 |                   |
                                 | Split defined,    |
                                 | not yet active    |
                                 +--------+----------+
                                          |
                      Validation passed   | Validation failed
                      +-------------------+-------------------+
                      |                                       |
                      v                                       v
           +-------------------+                   +-------------------+
           |    INITIALIZING   |                   |    REJECTED       |
           |                   |                   |                   |
           | Setting up        |                   | Configuration     |
           | traffic split     |                   | invalid           |
           +--------+----------+                   +-------------------+
                    |
                    | Initial weights applied
                    v
           +-------------------+
           |    PROGRESSING    |<----+
           |                   |     |
           | Canary receiving  |     | Analysis positive,
           | traffic           |     | increment weights
           +--------+----------+-----+
                    |
                    | Analysis result
                    +-------------------+
                    |                   |
     Analysis       |                   | Analysis
     negative       |                   | positive
                    v                   v
         +-------------------+ +-------------------+
         |   ROLLING_BACK    | |   COMPLETING      |
         |                   | |                   |
         | Reverting to      | | Shifting 100%     |
         | baseline          | | to canary         |
         +--------+----------+ +--------+----------+
                  |                     |
                  v                     v
         +-------------------+ +-------------------+
         |    ROLLED_BACK    | |    COMPLETED      |
         |                   | |                   |
         | All traffic to    | | All traffic to    |
         | baseline          | | canary (new base) |
         +-------------------+ +-------------------+
                  |                     |
                  +----------+----------+
                             |
                             v
                  +-------------------+
                  |    ARCHIVED       |
                  |                   |
                  | Split complete,   |
                  | kept in history   |
                  +-------------------+
```

#### Data Lifecycle Within Module

```rust
/// Traffic routing data lifecycle
impl TrafficManager {
    /// Request routing lifecycle (hot path)
    pub fn route_request_lifecycle(&self, request: &RequestContext) -> RoutingDecision {
        // 1. RATE CHECK: Check rate limits first
        let rate_decision = self.rate_controller.check(request);
        if rate_decision.exceeded {
            return RoutingDecision::RateLimited(rate_decision);
        }

        // 2. ROUTE MATCH: Find matching virtual service/route
        let route = match self.routing_engine.match_route(request) {
            Some(r) => r,
            None => return RoutingDecision::NoMatch,
        };

        // 3. TRAFFIC SPLIT: Apply split if active
        let destination = if let Some(split) = self.get_active_split(&route.service_id) {
            self.traffic_splitter.select_backend(&split, request)
        } else {
            route.get_destination()
        };

        // 4. FAULT CHECK: Apply fault injection if enabled
        if let Some(fault) = self.fault_injector.check_fault(route) {
            return RoutingDecision::FaultInjected(fault, destination);
        }

        // 5. RETURN: Return routing decision
        RoutingDecision::Routed {
            destination,
            timeout: route.timeout,
            retry_policy: route.retries.clone(),
            headers: route.headers.clone(),
        }
    }

    /// Canary management lifecycle
    async fn canary_lifecycle(&mut self, split_id: &TrafficSplitId) -> Result<(), TrafficError> {
        let split = self.get_split_mut(split_id)?;

        loop {
            // 1. COLLECT: Gather metrics for both versions
            let metrics = self.collect_canary_metrics(&split).await;

            // 2. EMIT: Send to L3 for analysis
            self.emit_canary_metrics(&split, &metrics).await;

            // 3. WAIT: Wait for analysis result
            let analysis = self.wait_for_analysis(split_id).await;

            // 4. DECIDE: Act on recommendation
            match analysis.recommendation {
                CanaryRecommendation::Continue => {
                    // Stay at current weights
                    tokio::time::sleep(self.config.canary_interval).await;
                }
                CanaryRecommendation::ProgressRollout => {
                    // Increment canary weight
                    self.progress_canary_weights(&mut split);
                    if split.is_complete() {
                        break;
                    }
                }
                CanaryRecommendation::Pause => {
                    // Wait for manual intervention
                    self.wait_for_manual_resume(split_id).await;
                }
                CanaryRecommendation::Rollback => {
                    // Roll back to baseline
                    self.rollback_canary(&mut split).await?;
                    break;
                }
                CanaryRecommendation::Complete => {
                    // Complete rollout
                    self.complete_canary(&mut split).await?;
                    break;
                }
            }
        }

        Ok(())
    }
}
```

---

## Metrics

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `me_m10_requests_total` | Counter | service, route, status | Total routed requests |
| `me_m10_request_duration_ms` | Histogram | service, route | Request duration |
| `me_m10_routes_matched` | Counter | virtual_service, route | Route match count |
| `me_m10_routes_unmatched` | Counter | - | Unmatched request count |
| `me_m10_rate_limited_total` | Counter | service, rule | Rate limited requests |
| `me_m10_traffic_splits_active` | Gauge | type | Active traffic splits |
| `me_m10_canary_weight` | Gauge | split, version | Current canary weight |
| `me_m10_faults_injected` | Counter | route, type | Injected fault count |
| `me_m10_routing_errors` | Counter | error_type | Routing error count |
| `me_m10_virtual_services` | Gauge | - | Total virtual services |
| `me_m10_destination_rules` | Gauge | - | Total destination rules |

---

## Error Codes

| Code | Name | Severity | Description | Recovery |
|------|------|----------|-------------|----------|
| E10001 | RouteNotFound | Warning | No matching route for request | Return 404 |
| E10002 | InvalidVirtualService | Error | Virtual service config invalid | Reject config |
| E10003 | DestinationUnreachable | Error | Cannot reach destination | Retry/failover |
| E10004 | RateLimitExceeded | Warning | Rate limit triggered | Return 429 |
| E10005 | TrafficSplitFailed | Error | Cannot apply traffic split | Rollback |
| E10006 | CanaryUnhealthy | Warning | Canary version unhealthy | Pause/rollback |
| E10007 | WeightCalculationError | Error | Cannot calculate weights | Use defaults |
| E10008 | FaultInjectionError | Warning | Fault injection failed | Skip fault |
| E10009 | RoutingTableCorrupt | Critical | Routing table inconsistent | Rebuild table |
| E10010 | HeaderManipulationFailed | Warning | Cannot manipulate headers | Skip headers |

---

## Navigation

| Direction | Link |
|-----------|------|
| Up | [INDEX.md](../INDEX.md) |
| Layer | [L02_SERVICES.md](../layers/L02_SERVICES.md) |
| Previous | [M09_SERVICE_MESH_CONTROLLER.md](M09_SERVICE_MESH_CONTROLLER.md) |
| Next | [M11_LOAD_BALANCER.md](M11_LOAD_BALANCER.md) |
| Related | [M12_CIRCUIT_BREAKER.md](M12_CIRCUIT_BREAKER.md) |
| API | [REST_API.md](../integration/REST_API.md) |

---

*[Back to Index](../INDEX.md) | [Layer: L02 Services](../layers/L02_SERVICES.md)*
