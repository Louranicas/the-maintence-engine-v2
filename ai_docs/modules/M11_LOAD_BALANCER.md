# Module M11: Load Balancer

> **M11_LOAD_BALANCER** | Intelligent Request Distribution & Endpoint Selection | Layer: L2 Services | [Back to Index](../INDEX.md)

---

## Navigation

| Direction | Link |
|-----------|------|
| Up | [INDEX.md](../INDEX.md) |
| Layer | [L02_SERVICES.md](../layers/L02_SERVICES.md) |
| Previous | [M10_TRAFFIC_MANAGER.md](M10_TRAFFIC_MANAGER.md) |
| Next | [M12_CIRCUIT_BREAKER.md](M12_CIRCUIT_BREAKER.md) |
| Related | [M08_SERVICE_DISCOVERY.md](M08_SERVICE_DISCOVERY.md) |
| L1 Foundation | [L01_FOUNDATION.md](../layers/L01_FOUNDATION.md) |
| L3 Learning | [L03_LEARNING.md](../layers/L03_LEARNING.md) |

---

## Module Specification

### Overview

The Load Balancer module provides intelligent request distribution across service endpoints, implementing multiple load balancing algorithms, health-aware routing, and adaptive weight adjustment. It integrates with M07 Health Monitor for endpoint health, M08 Service Discovery for endpoint pools, and M10 Traffic Manager for routing context.

### Module Properties

| Property | Value |
|----------|-------|
| Module ID | M11 |
| Module Name | Load Balancer |
| Layer | L2 (Services) |
| Version | 1.0.0 |
| Dependencies | M07 (Health Monitor), M08 (Service Discovery), M10 (Traffic Manager) |
| Dependents | M12 (Circuit Breaker), L3 (Learning), L5 (Remediation), L6 (Integration) |

---

## Architecture Diagram

```
+-----------------------------------------------------------------------------------+
|                           M11: LOAD BALANCER                                       |
+-----------------------------------------------------------------------------------+
|                                                                                   |
|  +-------------------------+    +-------------------------+    +--------------+   |
|  |   ENDPOINT POOL MGR     |    |   ALGORITHM ENGINE      |    | WEIGHT CALC  |   |
|  |                         |    |                         |    |              |   |
|  | - Pool lifecycle        |    | - Round Robin           |    | - Health     |   |
|  | - Endpoint tracking     |--->| - Weighted RR           |--->|   weights    |   |
|  | - Health integration    |    | - Least Connections     |    | - Latency    |   |
|  | - Zone awareness        |    | - Consistent Hash       |    |   weights    |   |
|  +------------+------------+    +------------+------------+    | - Adaptive   |   |
|               |                              |                 +--------------+   |
|               v                              v                        |           |
|  +-------------------------+    +-------------------------+           |           |
|  |   SESSION AFFINITY      |    |   OUTLIER DETECTION     |           |           |
|  |                         |    |                         |           |           |
|  | - Cookie-based          |    | - Error rate ejection   |           v           |
|  | - Header-based          |    | - Latency ejection      |    +--------------+   |
|  | - IP hash               |    | - Gradual restoration   |    | ENDPOINT SEL |   |
|  +-------------------------+    +-------------------------+    |              |   |
|                                                                | -> M12 CB    |   |
|                                                                | -> Response  |   |
|                                                                +--------------+   |
+-----------------------------------------------------------------------------------+
        ^                    ^                    ^                    |
        |                    |                    |                    v
   [M08: Discovery]    [M07: Health]       [M10: Traffic]       [Request Flow]
```

---

## Core Data Structures

### Endpoint Pool

```rust
/// Pool of endpoints for a service
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointPool {
    /// Pool identifier
    pub id: PoolId,

    /// Service this pool belongs to
    pub service_id: ServiceId,

    /// Active endpoints
    pub endpoints: Vec<PoolEndpoint>,

    /// Load balancing algorithm
    pub algorithm: LoadBalanceAlgorithm,

    /// Health check configuration
    pub health_check: HealthCheckConfig,

    /// Session affinity configuration
    pub affinity: Option<AffinityConfig>,

    /// Outlier detection settings
    pub outlier_detection: OutlierDetectionConfig,

    /// Pool statistics
    pub stats: PoolStats,

    /// Last updated timestamp
    pub updated_at: DateTime<Utc>,
}

/// Individual endpoint in a pool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolEndpoint {
    /// Endpoint identifier
    pub id: EndpointId,

    /// Endpoint address
    pub address: SocketAddr,

    /// Current weight (0-1000)
    pub weight: u32,

    /// Base weight (before adjustments)
    pub base_weight: u32,

    /// Current health state
    pub health: EndpointHealth,

    /// Connection count
    pub active_connections: u32,

    /// Request statistics
    pub stats: EndpointStats,

    /// Availability zone
    pub zone: Option<String>,

    /// Is this endpoint ejected?
    pub ejected: bool,

    /// Ejection expiry time
    pub ejection_expires: Option<DateTime<Utc>>,

    /// Metadata
    pub metadata: HashMap<String, String>,
}

/// Endpoint health status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointHealth {
    /// Health state
    pub state: HealthState,

    /// Health score (0.0 - 1.0)
    pub score: f64,

    /// Last health check time
    pub last_check: DateTime<Utc>,

    /// Consecutive failures
    pub failure_count: u32,

    /// Consecutive successes
    pub success_count: u32,
}

/// Endpoint statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointStats {
    /// Total requests
    pub requests_total: u64,

    /// Successful requests
    pub requests_success: u64,

    /// Failed requests
    pub requests_failed: u64,

    /// Average latency (ms)
    pub latency_avg_ms: f64,

    /// P99 latency (ms)
    pub latency_p99_ms: f64,

    /// Error rate (0.0 - 1.0)
    pub error_rate: f64,

    /// Requests in last minute
    pub requests_per_minute: u64,

    /// Last request time
    pub last_request: Option<DateTime<Utc>>,
}
```

### Load Balancing Algorithms

```rust
/// Available load balancing algorithms
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LoadBalanceAlgorithm {
    /// Simple round-robin
    RoundRobin,

    /// Weighted round-robin
    WeightedRoundRobin,

    /// Random selection
    Random,

    /// Weighted random
    WeightedRandom,

    /// Least connections
    LeastConnections,

    /// Weighted least connections
    WeightedLeastConnections,

    /// Least latency
    LeastLatency,

    /// Consistent hash
    ConsistentHash,

    /// Ring hash
    RingHash,

    /// Maglev hash
    Maglev,

    /// P2C (Power of Two Choices)
    PowerOfTwoChoices,

    /// Adaptive (ML-driven)
    Adaptive,
}

/// Consistent hash configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsistentHashConfig {
    /// Hash key source
    pub hash_key: HashKeySource,

    /// Virtual nodes per endpoint
    pub virtual_nodes: u32,

    /// Hash function
    pub hash_function: HashFunction,
}

/// Hash key sources
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HashKeySource {
    /// Use source IP
    SourceIp,

    /// Use specific header
    Header(String),

    /// Use cookie
    Cookie(String),

    /// Use query parameter
    QueryParam(String),

    /// Use request path
    Path,

    /// Custom expression
    Custom(String),
}

/// Hash functions available
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HashFunction {
    Xxhash,
    Murmur3,
    Crc32,
    Md5,
}
```

### Session Affinity

```rust
/// Session affinity configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AffinityConfig {
    /// Affinity type
    pub affinity_type: AffinityType,

    /// TTL for affinity mapping
    pub ttl: Duration,

    /// Cookie configuration (for cookie-based)
    pub cookie: Option<AffinityCookieConfig>,

    /// Header name (for header-based)
    pub header: Option<String>,
}

/// Affinity types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AffinityType {
    /// No affinity
    None,

    /// Client IP-based
    ClientIp,

    /// Cookie-based
    Cookie,

    /// Header-based
    Header,

    /// Consistent hash
    ConsistentHash,
}

/// Cookie configuration for affinity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AffinityCookieConfig {
    /// Cookie name
    pub name: String,

    /// Cookie path
    pub path: String,

    /// Cookie TTL
    pub ttl: Duration,

    /// HTTP only flag
    pub http_only: bool,

    /// Secure flag
    pub secure: bool,

    /// Same-site policy
    pub same_site: SameSitePolicy,
}
```

### Outlier Detection

```rust
/// Outlier detection configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutlierDetectionConfig {
    /// Enable outlier detection
    pub enabled: bool,

    /// Consecutive errors before ejection
    pub consecutive_errors: u32,

    /// Error rate threshold for ejection
    pub error_rate_threshold: f64,

    /// Latency threshold (P99) for ejection
    pub latency_threshold_ms: u64,

    /// Check interval
    pub interval: Duration,

    /// Base ejection time
    pub base_ejection_time: Duration,

    /// Maximum ejection time
    pub max_ejection_time: Duration,

    /// Maximum ejection percentage
    pub max_ejection_percent: f64,

    /// Success rate before restoration
    pub success_rate_minimum: f64,

    /// Minimum request volume for analysis
    pub min_request_volume: u64,
}

/// Outlier ejection record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutlierEjection {
    /// Endpoint ID
    pub endpoint_id: EndpointId,

    /// Ejection reason
    pub reason: EjectionReason,

    /// Ejection start time
    pub ejected_at: DateTime<Utc>,

    /// Ejection end time
    pub expires_at: DateTime<Utc>,

    /// Number of times ejected
    pub ejection_count: u32,

    /// Stats at ejection time
    pub stats_at_ejection: EndpointStats,
}

/// Reasons for ejection
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EjectionReason {
    ConsecutiveErrors,
    HighErrorRate,
    HighLatency,
    HealthCheckFailure,
    ManualEjection,
}
```

### Weight Calculation

```rust
/// Weight calculation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeightCalculation {
    /// Endpoint ID
    pub endpoint_id: EndpointId,

    /// Calculated weight
    pub weight: u32,

    /// Weight components
    pub components: WeightComponents,

    /// Calculation timestamp
    pub calculated_at: DateTime<Utc>,
}

/// Weight calculation components
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeightComponents {
    /// Base weight (from config)
    pub base: u32,

    /// Health adjustment factor (0.0 - 1.0)
    pub health_factor: f64,

    /// Latency adjustment factor (0.0 - 1.0)
    pub latency_factor: f64,

    /// Error rate adjustment factor (0.0 - 1.0)
    pub error_factor: f64,

    /// Connection count factor (0.0 - 1.0)
    pub connection_factor: f64,

    /// Zone preference factor (0.0 - 1.0)
    pub zone_factor: f64,

    /// Final multiplier
    pub final_multiplier: f64,
}
```

---

## Public API

### LoadBalancer Service

```rust
/// Main Load Balancer service
pub struct LoadBalancer {
    config: LoadBalancerConfig,
    pool_manager: EndpointPoolManager,
    algorithm_engine: AlgorithmEngine,
    weight_calculator: WeightCalculator,
    affinity_manager: AffinityManager,
    outlier_detector: OutlierDetector,
    metrics: LoadBalancerMetrics,
}

impl LoadBalancer {
    /// Create a new LoadBalancer instance
    pub fn new(config: LoadBalancerConfig) -> Self;

    /// Start the load balancer
    pub async fn start(&mut self) -> Result<(), LoadBalancerError>;

    /// Stop the load balancer
    pub async fn stop(&mut self) -> Result<(), LoadBalancerError>;

    // === Endpoint Pool API ===

    /// Create an endpoint pool
    pub async fn create_pool(&mut self, pool: EndpointPool) -> Result<PoolId, LoadBalancerError>;

    /// Update an endpoint pool
    pub async fn update_pool(&mut self, pool: EndpointPool) -> Result<(), LoadBalancerError>;

    /// Delete an endpoint pool
    pub async fn delete_pool(&mut self, pool_id: &PoolId) -> Result<(), LoadBalancerError>;

    /// Get endpoint pool
    pub fn get_pool(&self, pool_id: &PoolId) -> Option<&EndpointPool>;

    /// Get pool by service ID
    pub fn get_pool_by_service(&self, service_id: &ServiceId) -> Option<&EndpointPool>;

    /// List all pools
    pub fn list_pools(&self) -> Vec<&EndpointPool>;

    // === Endpoint Management API ===

    /// Add endpoint to pool
    pub async fn add_endpoint(&mut self, pool_id: &PoolId, endpoint: PoolEndpoint) -> Result<(), LoadBalancerError>;

    /// Remove endpoint from pool
    pub async fn remove_endpoint(&mut self, pool_id: &PoolId, endpoint_id: &EndpointId) -> Result<(), LoadBalancerError>;

    /// Update endpoint weight
    pub fn update_endpoint_weight(&mut self, pool_id: &PoolId, endpoint_id: &EndpointId, weight: u32) -> Result<(), LoadBalancerError>;

    /// Get endpoint
    pub fn get_endpoint(&self, pool_id: &PoolId, endpoint_id: &EndpointId) -> Option<&PoolEndpoint>;

    // === Load Balancing API ===

    /// Select an endpoint (main entry point)
    pub fn select(&self, pool_id: &PoolId, request: &RequestContext) -> Result<&PoolEndpoint, LoadBalancerError>;

    /// Select endpoint with specific algorithm
    pub fn select_with_algorithm(&self, pool_id: &PoolId, algorithm: LoadBalanceAlgorithm, request: &RequestContext) -> Result<&PoolEndpoint, LoadBalancerError>;

    /// Get current algorithm for pool
    pub fn get_algorithm(&self, pool_id: &PoolId) -> Option<LoadBalanceAlgorithm>;

    /// Set algorithm for pool
    pub fn set_algorithm(&mut self, pool_id: &PoolId, algorithm: LoadBalanceAlgorithm) -> Result<(), LoadBalancerError>;

    // === Health Integration API ===

    /// Update endpoint health from M07
    pub fn update_endpoint_health(&mut self, pool_id: &PoolId, endpoint_id: &EndpointId, health: EndpointHealth);

    /// Mark endpoint as unhealthy
    pub fn mark_unhealthy(&mut self, pool_id: &PoolId, endpoint_id: &EndpointId);

    /// Mark endpoint as healthy
    pub fn mark_healthy(&mut self, pool_id: &PoolId, endpoint_id: &EndpointId);

    // === Outlier Detection API ===

    /// Eject endpoint
    pub fn eject_endpoint(&mut self, pool_id: &PoolId, endpoint_id: &EndpointId, reason: EjectionReason) -> Result<(), LoadBalancerError>;

    /// Restore ejected endpoint
    pub fn restore_endpoint(&mut self, pool_id: &PoolId, endpoint_id: &EndpointId) -> Result<(), LoadBalancerError>;

    /// Get ejected endpoints
    pub fn get_ejected(&self, pool_id: &PoolId) -> Vec<&OutlierEjection>;

    // === Connection Tracking API ===

    /// Report connection start
    pub fn connection_start(&mut self, pool_id: &PoolId, endpoint_id: &EndpointId);

    /// Report connection end
    pub fn connection_end(&mut self, pool_id: &PoolId, endpoint_id: &EndpointId);

    /// Report request completion
    pub fn request_complete(&mut self, pool_id: &PoolId, endpoint_id: &EndpointId, success: bool, latency: Duration);

    // === Statistics API ===

    /// Get pool statistics
    pub fn get_pool_stats(&self, pool_id: &PoolId) -> Option<&PoolStats>;

    /// Get endpoint statistics
    pub fn get_endpoint_stats(&self, pool_id: &PoolId, endpoint_id: &EndpointId) -> Option<&EndpointStats>;

    /// Get load distribution
    pub fn get_load_distribution(&self, pool_id: &PoolId) -> LoadDistribution;
}
```

### AlgorithmEngine API

```rust
/// Implements load balancing algorithms
pub struct AlgorithmEngine {
    /// Select endpoint using round-robin
    pub fn round_robin(&mut self, pool: &EndpointPool) -> Option<&PoolEndpoint>;

    /// Select endpoint using weighted round-robin
    pub fn weighted_round_robin(&mut self, pool: &EndpointPool) -> Option<&PoolEndpoint>;

    /// Select endpoint using least connections
    pub fn least_connections(&self, pool: &EndpointPool) -> Option<&PoolEndpoint>;

    /// Select endpoint using least latency
    pub fn least_latency(&self, pool: &EndpointPool) -> Option<&PoolEndpoint>;

    /// Select endpoint using consistent hash
    pub fn consistent_hash(&self, pool: &EndpointPool, key: &[u8]) -> Option<&PoolEndpoint>;

    /// Select endpoint using P2C
    pub fn power_of_two_choices(&self, pool: &EndpointPool) -> Option<&PoolEndpoint>;

    /// Select endpoint using adaptive algorithm
    pub fn adaptive(&self, pool: &EndpointPool, context: &RequestContext) -> Option<&PoolEndpoint>;
}
```

---

## Configuration

### TOML Configuration Example

```toml
[layer.L2.M11]
enabled = true
version = "1.0.0"

# Global settings
[layer.L2.M11.global]
default_algorithm = "weighted_round_robin"
health_aware = true
zone_aware = true
panic_threshold = 0.5  # Disable health-aware if >50% unhealthy

# Weight calculation
[layer.L2.M11.weights]
health_weight = 0.3
latency_weight = 0.3
error_rate_weight = 0.2
connection_weight = 0.2
recalculation_interval_ms = 5000

# Outlier detection defaults
[layer.L2.M11.outlier_detection]
enabled = true
consecutive_errors = 5
error_rate_threshold = 0.5
latency_threshold_ms = 5000
interval_ms = 10000
base_ejection_time_ms = 30000
max_ejection_time_ms = 300000
max_ejection_percent = 30.0
success_rate_minimum = 0.8
min_request_volume = 100

# Session affinity defaults
[layer.L2.M11.affinity]
default_type = "none"
default_ttl_seconds = 3600

# Consistent hash settings
[layer.L2.M11.consistent_hash]
virtual_nodes = 150
hash_function = "xxhash"

# Zone-aware settings
[layer.L2.M11.zone_awareness]
enabled = true
local_zone_preference = 0.8
min_zone_endpoints = 2

# Connection pooling
[layer.L2.M11.connection]
max_connections_per_endpoint = 100
idle_timeout_ms = 60000
connection_timeout_ms = 5000

# Service-specific pool configurations
[[layer.L2.M11.pools]]
service = "synthex"
algorithm = "least_connections"
[layer.L2.M11.pools.outlier_detection]
consecutive_errors = 3
error_rate_threshold = 0.3

[[layer.L2.M11.pools]]
service = "san-k7"
algorithm = "weighted_round_robin"
[layer.L2.M11.pools.affinity]
type = "cookie"
cookie_name = "SANK7_AFFINITY"
cookie_ttl_seconds = 1800

[[layer.L2.M11.pools]]
service = "nais"
algorithm = "consistent_hash"
[layer.L2.M11.pools.consistent_hash]
hash_key = "header"
header_name = "X-Request-ID"
```

---

## Bi-Directional Data Flow

### Inbound Data Flow

The Load Balancer receives endpoint information, health data, and routing context.

#### Inbound Message Types

```rust
/// Messages received by Load Balancer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LoadBalancerInbound {
    // From L1 Config (M02)
    ConfigUpdate {
        global_config: LoadBalancerConfig,
        pool_configs: Vec<PoolConfig>,
        algorithm_configs: HashMap<ServiceId, LoadBalanceAlgorithm>,
        timestamp: DateTime<Utc>,
    },

    // From L1 State (M05)
    StateRestored {
        pools: Vec<EndpointPool>,
        affinity_mappings: HashMap<AffinityKey, EndpointId>,
        outlier_state: Vec<OutlierEjection>,
        round_robin_state: HashMap<PoolId, usize>,
        timestamp: DateTime<Utc>,
    },

    // From M07 Health Monitor
    EndpointHealthUpdate {
        pool_id: PoolId,
        endpoint_id: EndpointId,
        health: EndpointHealth,
        timestamp: DateTime<Utc>,
    },

    BulkHealthUpdate {
        updates: Vec<(PoolId, EndpointId, EndpointHealth)>,
        timestamp: DateTime<Utc>,
    },

    // From M08 Service Discovery
    EndpointPoolUpdate {
        service_id: ServiceId,
        endpoints: Vec<EndpointWithWeight>,
        strategy: LoadBalanceStrategy,
        timestamp: DateTime<Utc>,
    },

    EndpointAdded {
        pool_id: PoolId,
        endpoint: PoolEndpoint,
        timestamp: DateTime<Utc>,
    },

    EndpointRemoved {
        pool_id: PoolId,
        endpoint_id: EndpointId,
        reason: String,
        timestamp: DateTime<Utc>,
    },

    // From M10 Traffic Manager
    LoadBalancerUpdate {
        service_id: ServiceId,
        endpoint_weights: HashMap<EndpointId, u32>,
        routing_context: RoutingContext,
        timestamp: DateTime<Utc>,
    },

    // From L3 Learning
    WeightRecommendation {
        pool_id: PoolId,
        recommended_weights: HashMap<EndpointId, u32>,
        confidence: f64,
        rationale: String,
        timestamp: DateTime<Utc>,
    },

    AlgorithmRecommendation {
        pool_id: PoolId,
        recommended_algorithm: LoadBalanceAlgorithm,
        predicted_improvement: f64,
        timestamp: DateTime<Utc>,
    },

    // From L5 Remediation
    ForceEndpointEjection {
        pool_id: PoolId,
        endpoint_id: EndpointId,
        duration: Duration,
        reason: String,
        timestamp: DateTime<Utc>,
    },

    ForceEndpointRestoration {
        pool_id: PoolId,
        endpoint_id: EndpointId,
        timestamp: DateTime<Utc>,
    },

    WeightOverride {
        pool_id: PoolId,
        weights: HashMap<EndpointId, u32>,
        duration: Duration,
        reason: String,
        timestamp: DateTime<Utc>,
    },
}
```

#### Inbound Flow Sources

| Source | Message Type | Trigger Condition | Frequency |
|--------|-------------|-------------------|-----------|
| L1 Config (M02) | ConfigUpdate | Config file change | On change |
| L1 State (M05) | StateRestored | System startup | On startup |
| M07 Health | EndpointHealthUpdate | Health change | On change |
| M07 Health | BulkHealthUpdate | Periodic sync | Every 5s |
| M08 Discovery | EndpointPoolUpdate | Pool change | On change |
| M08 Discovery | EndpointAdded/Removed | Endpoint change | On event |
| M10 Traffic | LoadBalancerUpdate | Weight update | On change |
| L3 Learning | WeightRecommendation | ML analysis | Periodic |
| L3 Learning | AlgorithmRecommendation | Performance analysis | Periodic |
| L5 Remediation | ForceEndpoint* | Remediation action | On action |
| L5 Remediation | WeightOverride | Emergency override | On action |

#### Inbound Sequence Diagram

```
  L1:Config   M07:Health   M08:Discovery   M10:Traffic   L3:Learning   L5:Remediation
      |           |             |              |              |              |
      | ConfigUpdate            |              |              |              |
      |---------->|             |              |              |              |
      |           |             |              |              |              |
      |           | HealthUpdate|              |              |              |
      |           |------------>|              |              |              |
      |           |             |              |              |              |
      |           |             | EndpointPoolUpdate          |              |
      |           |             |------------->|              |              |
      |           |             |              |              |              |
      |           |             |              | LBUpdate     |              |
      |           |             |              |------------->|              |
      |           |             |              |              |              |
      |           |             |              |              | WeightRec    |
      |           |             |              |              |------------->|
      |           |             |              |              |              |
      |           |             |              |              |     ForceEjection
      |           |             |              |              |<-------------|
      |           |             |              |              |              |
      +-----------+-------------+--------------+--------------+--------------+
                                       |
                                       v
                             +-------------------+
                             |  M11 LOAD         |
                             |  BALANCER         |
                             |                   |
                             | - Update pools    |
                             | - Adjust weights  |
                             | - Apply algorithm |
                             +-------------------+
```

### Outbound Data Flow

The Load Balancer emits selection decisions, statistics, and health reports.

#### Outbound Message Types

```rust
/// Messages emitted by Load Balancer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LoadBalancerOutbound {
    // To L1 Metrics (M04)
    LoadBalancerMetrics {
        pool_id: PoolId,
        requests_total: u64,
        requests_per_second: f64,
        endpoints_active: u32,
        endpoints_ejected: u32,
        load_distribution: LoadDistribution,
        avg_selection_time_ns: u64,
        timestamp: DateTime<Utc>,
    },

    EndpointMetrics {
        pool_id: PoolId,
        endpoint_id: EndpointId,
        stats: EndpointStats,
        weight: u32,
        connections: u32,
        timestamp: DateTime<Utc>,
    },

    // To L1 State (M05)
    StatePersist {
        pools: Vec<EndpointPool>,
        affinity_mappings: HashMap<AffinityKey, EndpointId>,
        outlier_state: Vec<OutlierEjection>,
        round_robin_state: HashMap<PoolId, usize>,
        timestamp: DateTime<Utc>,
    },

    // To M12 Circuit Breaker
    EndpointSelection {
        pool_id: PoolId,
        endpoint_id: EndpointId,
        endpoint_address: SocketAddr,
        algorithm_used: LoadBalanceAlgorithm,
        timestamp: DateTime<Utc>,
    },

    EndpointEjectionNotice {
        pool_id: PoolId,
        endpoint_id: EndpointId,
        reason: EjectionReason,
        duration: Duration,
        timestamp: DateTime<Utc>,
    },

    // To L3 Learning
    LoadBalancingEvent {
        event_type: LBEventType,
        pool_id: PoolId,
        details: LBEventDetails,
        timestamp: DateTime<Utc>,
    },

    SelectionPattern {
        pool_id: PoolId,
        selections: Vec<EndpointSelection>,
        algorithm: LoadBalanceAlgorithm,
        distribution: LoadDistribution,
        period: Duration,
        timestamp: DateTime<Utc>,
    },

    // To L5 Remediation
    LoadBalancerAlert {
        alert_type: LBAlertType,
        pool_id: PoolId,
        severity: Severity,
        details: String,
        affected_endpoints: Vec<EndpointId>,
        suggested_action: Option<LBRemediationAction>,
        timestamp: DateTime<Utc>,
    },

    OutlierReport {
        pool_id: PoolId,
        ejections: Vec<OutlierEjection>,
        healthy_count: u32,
        ejected_count: u32,
        ejection_rate: f64,
        timestamp: DateTime<Utc>,
    },

    // To L6 Integration
    PoolStatusExport {
        pools: Vec<PoolStatus>,
        timestamp: DateTime<Utc>,
    },
}

/// Load balancing event types for L3
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LBEventType {
    EndpointSelected,
    EndpointEjected,
    EndpointRestored,
    WeightAdjusted,
    AlgorithmChanged,
    AffinityMapped,
    LoadImbalanceDetected,
}

/// Load balancer alert types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LBAlertType {
    HighEjectionRate,
    NoHealthyEndpoints,
    LoadImbalance,
    AffinityOverload,
    WeightCalculationFailure,
    PoolCapacityReached,
}

/// Load distribution statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadDistribution {
    /// Per-endpoint load percentages
    pub endpoint_load: HashMap<EndpointId, f64>,

    /// Gini coefficient (0 = perfect balance, 1 = complete imbalance)
    pub gini_coefficient: f64,

    /// Standard deviation of load
    pub load_std_dev: f64,

    /// Most loaded endpoint percentage
    pub max_load_percent: f64,

    /// Least loaded endpoint percentage
    pub min_load_percent: f64,
}
```

#### Outbound Flow Targets

| Target | Message Type | Trigger Condition | Priority |
|--------|-------------|-------------------|----------|
| L1 Metrics (M04) | LoadBalancerMetrics | Periodic collection | Normal |
| L1 Metrics (M04) | EndpointMetrics | Per-endpoint stats | Normal |
| L1 State (M05) | StatePersist | State change | High |
| M12 Circuit Breaker | EndpointSelection | Each request | Critical |
| M12 Circuit Breaker | EndpointEjectionNotice | Ejection event | High |
| L3 Learning | LoadBalancingEvent | Significant events | Normal |
| L3 Learning | SelectionPattern | Analysis window | Normal |
| L5 Remediation | LoadBalancerAlert | Threshold breached | Critical |
| L5 Remediation | OutlierReport | Ejection changes | High |
| L6 Integration | PoolStatusExport | Periodic/on-demand | Low |

#### Outbound Sequence Diagram

```
                             +-------------------+
                             |  M11 LOAD         |
                             |  BALANCER         |
                             +--------+----------+
                                      |
       +------------------------------+------------------------------+
       |              |               |               |              |
       v              v               v               v              v
  +---------+   +---------+     +---------+     +---------+    +---------+
  |L1:Metrics|  |L1:State |     |M12:CB   |     |L3:Learn |    |L5:Remed |
  +---------+   +---------+     +---------+     +---------+    +---------+
       |              |               |               |              |
       |              |               |               |              |
       +------------------------------+------------------------------+
                                      |
                                      v
                             +-------------------+
                             |  L6:Integration   |
                             +-------------------+
```

### Cross-Module Dependencies

#### Bidirectional Dependency Matrix

| Module | Reads From M11 | Writes To M11 | Sync/Async | Error Path |
|--------|---------------|---------------|------------|------------|
| M02 Config | - | ConfigUpdate | Async | Use defaults |
| M04 Metrics | LBMetrics, EndpointMetrics | - | Async | Skip metrics |
| M05 State | StatePersist | StateRestored | Async | Start fresh |
| M07 Health | - | HealthUpdate | Async | Skip health-aware |
| M08 Discovery | - | EndpointPoolUpdate | Async | Use cached pool |
| M10 Traffic | - | LoadBalancerUpdate | Async | Equal weights |
| M12 Circuit Breaker | EndpointSelection, EjectionNotice | - | Sync/Async | No CB check |
| L3 Learning | LBEvent, SelectionPattern | WeightRec, AlgorithmRec | Async | Ignore recs |
| L5 Remediation | LBAlert, OutlierReport | ForceEjection, WeightOverride | Async | Manual alert |

#### Communication Patterns

```rust
/// Communication patterns for M11
pub struct LoadBalancerComms {
    // Synchronous endpoint selection (hot path)
    selection_engine: SelectionEngine,

    // Asynchronous events
    async_events: AsyncEventEmitter,

    // Statistics aggregation
    stats_aggregator: StatsAggregator,

    // State persistence
    state_persister: StatePersister,
}

impl LoadBalancerComms {
    /// Synchronous: Select endpoint (hot path, must be fast)
    pub fn select_endpoint(&self, pool_id: &PoolId, ctx: &RequestContext) -> Result<&PoolEndpoint, Error> {
        // Sub-microsecond operation
        self.selection_engine.select(pool_id, ctx)
    }

    /// Synchronous: Report request completion (hot path)
    pub fn report_completion(&mut self, pool_id: &PoolId, endpoint_id: &EndpointId, result: &RequestResult) {
        // Update stats in-place
        self.stats_aggregator.record(pool_id, endpoint_id, result);
    }

    /// Asynchronous: Emit load balancing events
    pub async fn emit_event(&self, event: LoadBalancerOutbound) {
        self.async_events.publish(event).await;
    }

    /// Background: Persist state periodically
    pub async fn persist_state(&self) {
        self.state_persister.persist_async().await;
    }
}
```

#### Error Propagation Paths

```
M11 Load Balancer Error
       |
       +---> [Log to M03 Logging] ---> Structured log entry
       |
       +---> [Encode via M01 Error Taxonomy] ---> 11D Error Vector
       |            |
       |            +---> [Send to L3 Learning] ---> Pattern recognition
       |
       +---> [Alert to L5 Remediation] ---> Trigger LB remediation
       |
       +---> [Notify M12 Circuit Breaker] ---> If no endpoints available
```

### Contextual Flow: Endpoint Selection Lifecycle

#### Outlier Detection State Machine

```
                                 +-------------------+
                                 |     HEALTHY       |
                                 |                   |
                                 | Endpoint in       |
                                 | active rotation   |
                                 +--------+----------+
                                          |
                      Threshold exceeded  | (errors, latency)
                                          |
                                          v
                               +-------------------+
                               |    EJECTED        |<----+
                               |                   |     |
                               | Removed from      |     | Still failing
                               | active rotation   |-----+ during probe
                               +--------+----------+
                                        |
                        Ejection time   | expired
                        (base * 2^n)    |
                                        v
                               +-------------------+
                               |    PROBING        |
                               |                   |
                               | Testing with      |
                               | limited traffic   |
                               +--------+----------+
                                        |
                   +--------------------+--------------------+
                   |                                         |
     Probe success | (meets success rate)     Probe fails    |
                   |                                         |
                   v                                         v
        +-------------------+                     +-------------------+
        |   RECOVERING      |                     |    EJECTED        |
        |                   |                     |   (extended)      |
        | Gradually         |                     | Ejection time     |
        | increasing weight |                     | doubled           |
        +--------+----------+                     +-------------------+
                 |
                 | Full weight restored
                 v
        +-------------------+
        |     HEALTHY       |
        +-------------------+
```

#### Data Lifecycle Within Module

```rust
/// Load balancer request lifecycle
impl LoadBalancer {
    /// Complete endpoint selection lifecycle (hot path)
    pub fn select_lifecycle(&self, pool_id: &PoolId, request: &RequestContext) -> Result<EndpointSelection, LoadBalancerError> {
        // 1. GET POOL: Retrieve endpoint pool (cached)
        let pool = self.pool_manager.get(pool_id)
            .ok_or(LoadBalancerError::PoolNotFound)?;

        // 2. FILTER: Get healthy, non-ejected endpoints
        let available = self.filter_available_endpoints(pool);
        if available.is_empty() {
            return Err(LoadBalancerError::NoHealthyEndpoints);
        }

        // 3. AFFINITY: Check session affinity
        if let Some(affinity) = &pool.affinity {
            if let Some(endpoint) = self.affinity_manager.lookup(request, affinity) {
                if available.contains(&endpoint.id) {
                    return Ok(self.create_selection(pool_id, &endpoint));
                }
            }
        }

        // 4. SELECT: Apply algorithm
        let endpoint = self.algorithm_engine.select(pool, &available, request)?;

        // 5. RECORD AFFINITY: If affinity enabled, record mapping
        if pool.affinity.is_some() {
            self.affinity_manager.record(request, &endpoint.id);
        }

        // 6. UPDATE STATS: Increment connection count
        self.connection_tracker.increment(pool_id, &endpoint.id);

        // 7. EMIT: Send selection event
        let selection = self.create_selection(pool_id, endpoint);
        self.emit_selection_event(&selection);

        Ok(selection)
    }

    /// Weight recalculation lifecycle (periodic)
    fn recalculate_weights_lifecycle(&mut self, pool_id: &PoolId) {
        let pool = match self.pool_manager.get_mut(pool_id) {
            Some(p) => p,
            None => return,
        };

        for endpoint in &mut pool.endpoints {
            // 1. GATHER: Collect all factors
            let health_factor = self.calculate_health_factor(&endpoint.health);
            let latency_factor = self.calculate_latency_factor(&endpoint.stats);
            let error_factor = self.calculate_error_factor(&endpoint.stats);
            let connection_factor = self.calculate_connection_factor(endpoint.active_connections);
            let zone_factor = self.calculate_zone_factor(&endpoint.zone);

            // 2. CALCULATE: Compute new weight
            let components = WeightComponents {
                base: endpoint.base_weight,
                health_factor,
                latency_factor,
                error_factor,
                connection_factor,
                zone_factor,
                final_multiplier: self.config.weights.calculate_multiplier(&[
                    (health_factor, self.config.weights.health_weight),
                    (latency_factor, self.config.weights.latency_weight),
                    (error_factor, self.config.weights.error_rate_weight),
                    (connection_factor, self.config.weights.connection_weight),
                ]),
            };

            let new_weight = (endpoint.base_weight as f64 * components.final_multiplier) as u32;

            // 3. APPLY: Update endpoint weight
            endpoint.weight = new_weight.max(1); // Minimum weight of 1

            // 4. EMIT: If significant change, emit event
            if self.is_significant_weight_change(endpoint.weight, new_weight) {
                self.emit_weight_changed(pool_id, &endpoint.id, endpoint.weight, new_weight);
            }
        }
    }
}
```

---

## Metrics

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `me_m11_selections_total` | Counter | pool, algorithm | Total endpoint selections |
| `me_m11_selection_duration_ns` | Histogram | pool, algorithm | Selection latency |
| `me_m11_endpoints_active` | Gauge | pool | Active endpoint count |
| `me_m11_endpoints_ejected` | Gauge | pool | Ejected endpoint count |
| `me_m11_ejections_total` | Counter | pool, reason | Ejection event count |
| `me_m11_restorations_total` | Counter | pool | Restoration event count |
| `me_m11_endpoint_weight` | Gauge | pool, endpoint | Current endpoint weight |
| `me_m11_endpoint_connections` | Gauge | pool, endpoint | Active connections |
| `me_m11_load_gini_coefficient` | Gauge | pool | Load balance Gini coefficient |
| `me_m11_affinity_hits` | Counter | pool | Affinity cache hits |
| `me_m11_affinity_misses` | Counter | pool | Affinity cache misses |
| `me_m11_no_endpoints_errors` | Counter | pool | No healthy endpoints errors |

---

## Error Codes

| Code | Name | Severity | Description | Recovery |
|------|------|----------|-------------|----------|
| E11001 | PoolNotFound | Error | Endpoint pool not found | Create pool |
| E11002 | NoHealthyEndpoints | Critical | No healthy endpoints available | Alert L5 |
| E11003 | EndpointNotFound | Warning | Specific endpoint not found | Refresh pool |
| E11004 | EjectionFailed | Warning | Cannot eject endpoint | Log and continue |
| E11005 | RestorationFailed | Warning | Cannot restore endpoint | Retry later |
| E11006 | WeightCalculationFailed | Warning | Weight calculation error | Use base weight |
| E11007 | AffinityLookupFailed | Warning | Affinity mapping error | Skip affinity |
| E11008 | AlgorithmError | Error | Algorithm execution failed | Fallback to RR |
| E11009 | ConnectionPoolExhausted | Warning | Too many connections | Queue or reject |
| E11010 | LoadImbalance | Warning | Severe load imbalance detected | Rebalance |

---

## Navigation

| Direction | Link |
|-----------|------|
| Up | [INDEX.md](../INDEX.md) |
| Layer | [L02_SERVICES.md](../layers/L02_SERVICES.md) |
| Previous | [M10_TRAFFIC_MANAGER.md](M10_TRAFFIC_MANAGER.md) |
| Next | [M12_CIRCUIT_BREAKER.md](M12_CIRCUIT_BREAKER.md) |
| Related | [M08_SERVICE_DISCOVERY.md](M08_SERVICE_DISCOVERY.md) |
| API | [REST_API.md](../integration/REST_API.md) |

---

*[Back to Index](../INDEX.md) | [Layer: L02 Services](../layers/L02_SERVICES.md)*
