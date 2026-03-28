# Module M08: Service Discovery

> **M08_SERVICE_DISCOVERY** | Dynamic Service Registration & Resolution | Layer: L2 Services | [Back to Index](../INDEX.md)

---

## Navigation

| Direction | Link |
|-----------|------|
| Up | [INDEX.md](../INDEX.md) |
| Layer | [L02_SERVICES.md](../layers/L02_SERVICES.md) |
| Previous | [M07_HEALTH_MONITOR.md](M07_HEALTH_MONITOR.md) |
| Next | [M09_SERVICE_MESH_CONTROLLER.md](M09_SERVICE_MESH_CONTROLLER.md) |
| Related | [M11_LOAD_BALANCER.md](M11_LOAD_BALANCER.md) |
| L1 Foundation | [L01_FOUNDATION.md](../layers/L01_FOUNDATION.md) |
| L3 Learning | [L03_LEARNING.md](../layers/L03_LEARNING.md) |

---

## Module Specification

### Overview

The Service Discovery module provides dynamic service registration, endpoint resolution, and service catalog management. It maintains a real-time registry of all services in the Maintenance Engine ecosystem, enabling service-to-service communication without hardcoded endpoints. Integrates with M07 Health Monitor for health-aware discovery and M11 Load Balancer for intelligent routing.

### Module Properties

| Property | Value |
|----------|-------|
| Module ID | M08 |
| Module Name | Service Discovery |
| Layer | L2 (Services) |
| Version | 1.0.0 |
| Dependencies | M02 (Config), M05 (State), M07 (Health Monitor) |
| Dependents | M09 (Mesh Controller), M10 (Traffic Manager), M11 (Load Balancer), L3 (Learning), L6 (Integration) |

---

## Architecture Diagram

```
+-----------------------------------------------------------------------------------+
|                           M08: SERVICE DISCOVERY                                   |
+-----------------------------------------------------------------------------------+
|                                                                                   |
|  +-------------------------+    +-------------------------+    +--------------+   |
|  |   SERVICE REGISTRY      |    |   ENDPOINT RESOLVER     |    | CATALOG MGR  |   |
|  |                         |    |                         |    |              |   |
|  | - Registration API      |    | - DNS resolution        |    | - Service    |   |
|  | - Deregistration API    |--->| - Round-robin           |--->|   metadata   |   |
|  | - Health integration    |    | - Weighted selection    |    | - Versioning |   |
|  | - TTL management        |    | - Affinity rules        |    | - Tagging    |   |
|  +------------+------------+    +------------+------------+    +--------------+   |
|               |                              |                        |           |
|               v                              v                        v           |
|  +-------------------------+    +-------------------------+    +--------------+   |
|  |   WATCHER SUBSYSTEM     |    |   CACHE LAYER           |    | EVENT BUS    |   |
|  |                         |    |                         |    |              |   |
|  | - Change notifications  |    | - Endpoint cache        |    | -> M07 Health|   |
|  | - Subscription mgmt     |    | - TTL-based expiry      |    | -> M11 LB    |   |
|  | - Filtering             |    | - Invalidation          |    | -> L3 Learn  |   |
|  +-------------------------+    +-------------------------+    +--------------+   |
|                                                                                   |
+-----------------------------------------------------------------------------------+
        ^                    ^                    ^                    |
        |                    |                    |                    v
   [L1: Config]        [L1: State]         [M07: Health]        [Consumers]
```

---

## Core Data Structures

### Service Definition

```rust
/// Complete definition of a registered service
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceDefinition {
    /// Unique service identifier
    pub id: ServiceId,

    /// Human-readable service name
    pub name: String,

    /// Service version (semver)
    pub version: String,

    /// Service namespace for grouping
    pub namespace: String,

    /// Available endpoints
    pub endpoints: Vec<Endpoint>,

    /// Service metadata
    pub metadata: ServiceMetadata,

    /// Health check configuration
    pub health_check: Option<HealthCheckConfig>,

    /// Tags for filtering and routing
    pub tags: HashSet<String>,

    /// Service dependencies
    pub dependencies: Vec<ServiceId>,

    /// Registration timestamp
    pub registered_at: DateTime<Utc>,

    /// Last heartbeat timestamp
    pub last_heartbeat: DateTime<Utc>,

    /// Time-to-live for registration
    pub ttl: Duration,

    /// Current status
    pub status: ServiceStatus,
}

/// Service endpoint definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Endpoint {
    /// Endpoint identifier
    pub id: EndpointId,

    /// Protocol (http, https, grpc, tcp)
    pub protocol: Protocol,

    /// Host address
    pub host: String,

    /// Port number
    pub port: u16,

    /// Path prefix (for HTTP)
    pub path: Option<String>,

    /// Weight for load balancing
    pub weight: u32,

    /// Endpoint-specific metadata
    pub metadata: HashMap<String, String>,

    /// Availability zone
    pub zone: Option<String>,

    /// Whether endpoint is primary
    pub primary: bool,
}

/// Service metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceMetadata {
    /// Service description
    pub description: Option<String>,

    /// Team/owner
    pub owner: Option<String>,

    /// Documentation URL
    pub docs_url: Option<String>,

    /// Service tier (1=critical, 5=development)
    pub tier: u8,

    /// Custom attributes
    pub attributes: HashMap<String, Value>,
}

/// Service operational status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ServiceStatus {
    /// Service is registering
    Registering,
    /// Service is active and healthy
    Active,
    /// Service is active but degraded
    Degraded,
    /// Service is being drained
    Draining,
    /// Service is deregistering
    Deregistering,
    /// Service registration expired
    Expired,
}
```

### Service Query

```rust
/// Query for discovering services
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceQuery {
    /// Service name pattern (supports wildcards)
    pub name: Option<String>,

    /// Namespace filter
    pub namespace: Option<String>,

    /// Version constraint (semver range)
    pub version: Option<VersionReq>,

    /// Required tags (all must match)
    pub tags: HashSet<String>,

    /// Excluded tags (none must match)
    pub exclude_tags: HashSet<String>,

    /// Minimum health state
    pub min_health: Option<HealthState>,

    /// Availability zone preference
    pub zone: Option<String>,

    /// Maximum results
    pub limit: Option<usize>,

    /// Include metadata in response
    pub include_metadata: bool,
}

/// Result of service discovery query
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryResult {
    /// Matching services
    pub services: Vec<ServiceDefinition>,

    /// Total count (before limit)
    pub total_count: usize,

    /// Query execution time
    pub query_time_ms: u64,

    /// Result freshness
    pub timestamp: DateTime<Utc>,

    /// Cache hit indicator
    pub from_cache: bool,
}
```

### Service Watch

```rust
/// Watch subscription for service changes
#[derive(Debug, Clone)]
pub struct ServiceWatch {
    /// Watch identifier
    pub id: WatchId,

    /// Query filter for watching
    pub query: ServiceQuery,

    /// Event types to watch
    pub event_types: HashSet<WatchEventType>,

    /// Callback channel
    pub callback: mpsc::Sender<WatchEvent>,

    /// Watch creation time
    pub created_at: DateTime<Utc>,
}

/// Types of watch events
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WatchEventType {
    ServiceRegistered,
    ServiceDeregistered,
    ServiceUpdated,
    EndpointAdded,
    EndpointRemoved,
    HealthChanged,
    StatusChanged,
}

/// Watch event payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchEvent {
    /// Event type
    pub event_type: WatchEventType,

    /// Affected service
    pub service_id: ServiceId,

    /// Previous state (for updates)
    pub previous: Option<ServiceDefinition>,

    /// Current state
    pub current: Option<ServiceDefinition>,

    /// Event timestamp
    pub timestamp: DateTime<Utc>,
}
```

---

## Public API

### ServiceDiscovery Service

```rust
/// Main Service Discovery service
pub struct ServiceDiscovery {
    config: ServiceDiscoveryConfig,
    registry: ServiceRegistry,
    resolver: EndpointResolver,
    catalog: ServiceCatalog,
    watcher: WatcherSubsystem,
    cache: DiscoveryCache,
    metrics: ServiceDiscoveryMetrics,
}

impl ServiceDiscovery {
    /// Create a new ServiceDiscovery instance
    pub fn new(config: ServiceDiscoveryConfig) -> Self;

    /// Start the service discovery system
    pub async fn start(&mut self) -> Result<(), DiscoveryError>;

    /// Stop the service discovery system
    pub async fn stop(&mut self) -> Result<(), DiscoveryError>;

    // === Registration API ===

    /// Register a new service
    pub async fn register(&mut self, service: ServiceDefinition) -> Result<ServiceId, DiscoveryError>;

    /// Update an existing service registration
    pub async fn update(&mut self, service: ServiceDefinition) -> Result<(), DiscoveryError>;

    /// Deregister a service
    pub async fn deregister(&mut self, service_id: &ServiceId) -> Result<(), DiscoveryError>;

    /// Send heartbeat to keep registration alive
    pub async fn heartbeat(&mut self, service_id: &ServiceId) -> Result<(), DiscoveryError>;

    // === Discovery API ===

    /// Discover services matching query
    pub async fn discover(&self, query: ServiceQuery) -> Result<DiscoveryResult, DiscoveryError>;

    /// Get a single service by ID
    pub fn get_service(&self, service_id: &ServiceId) -> Option<ServiceDefinition>;

    /// Resolve endpoints for a service
    pub async fn resolve(&self, service_id: &ServiceId) -> Result<Vec<Endpoint>, DiscoveryError>;

    /// Resolve single endpoint (with load balancing)
    pub async fn resolve_one(&self, service_id: &ServiceId, strategy: LoadBalanceStrategy) -> Result<Endpoint, DiscoveryError>;

    // === Watch API ===

    /// Subscribe to service changes
    pub fn watch(&mut self, query: ServiceQuery, events: HashSet<WatchEventType>) -> Result<WatchId, DiscoveryError>;

    /// Unsubscribe from watch
    pub fn unwatch(&mut self, watch_id: &WatchId) -> Result<(), DiscoveryError>;

    /// Get watch event stream
    pub fn watch_stream(&self, watch_id: &WatchId) -> Option<impl Stream<Item = WatchEvent>>;

    // === Catalog API ===

    /// List all registered services
    pub fn list_services(&self) -> Vec<ServiceId>;

    /// Get service catalog (all metadata)
    pub fn get_catalog(&self) -> ServiceCatalog;

    /// Get services by namespace
    pub fn get_by_namespace(&self, namespace: &str) -> Vec<ServiceDefinition>;

    /// Get service dependencies
    pub fn get_dependencies(&self, service_id: &ServiceId) -> Vec<ServiceId>;

    /// Get service dependents (reverse deps)
    pub fn get_dependents(&self, service_id: &ServiceId) -> Vec<ServiceId>;

    // === Health Integration ===

    /// Update service health from M07
    pub fn update_health(&mut self, service_id: &ServiceId, health: HealthState);

    /// Get healthy services only
    pub fn get_healthy(&self, query: ServiceQuery) -> DiscoveryResult;
}
```

### Endpoint Resolver API

```rust
/// Resolves service IDs to concrete endpoints
pub struct EndpointResolver {
    /// Resolve all endpoints for a service
    pub async fn resolve_all(&self, service_id: &ServiceId) -> Vec<Endpoint>;

    /// Resolve single endpoint with strategy
    pub async fn resolve_one(&self, service_id: &ServiceId, strategy: LoadBalanceStrategy) -> Option<Endpoint>;

    /// Resolve with affinity (sticky sessions)
    pub async fn resolve_with_affinity(&self, service_id: &ServiceId, affinity_key: &str) -> Option<Endpoint>;

    /// Resolve to specific zone
    pub async fn resolve_in_zone(&self, service_id: &ServiceId, zone: &str) -> Vec<Endpoint>;

    /// Get endpoint by ID
    pub fn get_endpoint(&self, endpoint_id: &EndpointId) -> Option<Endpoint>;
}

/// Load balancing strategies
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadBalanceStrategy {
    RoundRobin,
    Random,
    LeastConnections,
    WeightedRoundRobin,
    ConsistentHash,
    HealthAware,
}
```

---

## Configuration

### TOML Configuration Example

```toml
[layer.L2.M08]
enabled = true
version = "1.0.0"

# Registry settings
[layer.L2.M08.registry]
default_ttl_seconds = 30
heartbeat_interval_ms = 10000
expiry_check_interval_ms = 5000
max_services = 10000
max_endpoints_per_service = 100

# Resolver settings
[layer.L2.M08.resolver]
default_strategy = "health_aware"
cache_ttl_ms = 5000
dns_timeout_ms = 1000
retry_count = 3
retry_backoff_ms = 100

# Cache settings
[layer.L2.M08.cache]
enabled = true
max_entries = 50000
ttl_ms = 10000
refresh_ahead_ms = 2000

# Watch settings
[layer.L2.M08.watch]
max_watches_per_client = 100
event_buffer_size = 1000
batch_interval_ms = 100

# Health integration
[layer.L2.M08.health]
integration_enabled = true
health_weight = 0.7
exclude_unhealthy = true
degraded_weight_factor = 0.5

# Namespace defaults
[[layer.L2.M08.namespaces]]
name = "production"
default_tier = 1
auto_register = true

[[layer.L2.M08.namespaces]]
name = "development"
default_tier = 5
auto_register = false

# Pre-registered services (12 ULTRAPLATE services)
[[layer.L2.M08.services]]
id = "synthex"
name = "SYNTHEX Engine"
namespace = "production"
version = "1.0.0"
tier = 1
endpoints = [
    { protocol = "http", host = "localhost", port = 8090, path = "/api" },
    { protocol = "ws", host = "localhost", port = 8091, path = "/ws" }
]
tags = ["core", "engine", "api"]

[[layer.L2.M08.services]]
id = "san-k7"
name = "SAN-K7 Orchestrator"
namespace = "production"
version = "1.55.0"
tier = 1
endpoints = [
    { protocol = "http", host = "localhost", port = 8100 }
]
tags = ["core", "orchestrator"]
```

---

## Bi-Directional Data Flow

### Inbound Data Flow

The Service Discovery module receives registration and update messages from services and health data from M07.

#### Inbound Message Types

```rust
/// Messages received by Service Discovery from other modules/layers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ServiceDiscoveryInbound {
    // From L1 Config (M02)
    ConfigUpdate {
        registry_config: RegistryConfig,
        pre_registered_services: Vec<ServiceDefinition>,
        namespace_config: Vec<NamespaceConfig>,
        timestamp: DateTime<Utc>,
    },

    // From L1 State (M05)
    StateRestored {
        registered_services: Vec<ServiceDefinition>,
        watch_subscriptions: Vec<ServiceWatch>,
        catalog_metadata: ServiceCatalog,
        timestamp: DateTime<Utc>,
    },

    // From M07 Health Monitor
    HealthUpdate {
        service_id: ServiceId,
        health_state: HealthState,
        health_score: f64,
        timestamp: DateTime<Utc>,
    },

    HealthBatchUpdate {
        updates: HashMap<ServiceId, (HealthState, f64)>,
        timestamp: DateTime<Utc>,
    },

    // From Services (via API)
    ServiceRegistration {
        service: ServiceDefinition,
        client_id: ClientId,
        timestamp: DateTime<Utc>,
    },

    ServiceHeartbeat {
        service_id: ServiceId,
        client_id: ClientId,
        metadata_update: Option<HashMap<String, Value>>,
        timestamp: DateTime<Utc>,
    },

    ServiceDeregistration {
        service_id: ServiceId,
        client_id: ClientId,
        reason: String,
        timestamp: DateTime<Utc>,
    },

    // From L3 Learning
    ServicePrediction {
        service_id: ServiceId,
        predicted_failure: bool,
        confidence: f64,
        recommended_action: Option<String>,
        timestamp: DateTime<Utc>,
    },

    // From L6 Integration
    ExternalServiceDiscovered {
        external_source: String,
        services: Vec<ServiceDefinition>,
        timestamp: DateTime<Utc>,
    },
}
```

#### Inbound Flow Sources

| Source | Message Type | Trigger Condition | Frequency |
|--------|-------------|-------------------|-----------|
| L1 Config (M02) | ConfigUpdate | Config file change | On change |
| L1 State (M05) | StateRestored | System startup | On startup |
| M07 Health | HealthUpdate | Health state change | On change |
| M07 Health | HealthBatchUpdate | Periodic sync | Every 5s |
| Services | ServiceRegistration | Service startup | On registration |
| Services | ServiceHeartbeat | Keep-alive | Every 10s |
| Services | ServiceDeregistration | Service shutdown | On shutdown |
| L3 Learning | ServicePrediction | ML prediction | As predicted |
| L6 Integration | ExternalServiceDiscovered | External sync | Periodic |

#### Inbound Sequence Diagram

```
  Services      L1:Config    L1:State    M07:Health   L3:Learning   L6:Integration
      |             |            |            |             |              |
      | Register    |            |            |             |              |
      |------------>|            |            |             |              |
      |             |            |            |             |              |
      |             | ConfigUpdate            |             |              |
      |             |----------->|            |             |              |
      |             |            |            |             |              |
      |             |            | StateRestored            |              |
      |             |            |----------->|             |              |
      |             |            |            |             |              |
      |             |            |            | HealthUpdate|              |
      |             |            |            |------------>|              |
      |             |            |            |             |              |
      |             |            |            |             | ServicePrediction
      |             |            |            |             |------------->|
      |             |            |            |             |              |
      |             |            |            |             |              | ExternalDiscovered
      |             |            |            |             |              |------------->
      |             |            |            |             |              |
      +-------------+------------+------------+-------------+--------------+
                                        |
                                        v
                              +-------------------+
                              |  M08 SERVICE      |
                              |  DISCOVERY        |
                              |                   |
                              | - Update registry |
                              | - Refresh cache   |
                              | - Emit events     |
                              +-------------------+
```

### Outbound Data Flow

The Service Discovery module emits service change events and provides resolution data to consumers.

#### Outbound Message Types

```rust
/// Messages emitted by Service Discovery to other modules/layers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ServiceDiscoveryOutbound {
    // To L1 Metrics (M04)
    DiscoveryMetrics {
        registered_services: usize,
        total_endpoints: usize,
        healthy_percentage: f64,
        cache_hit_rate: f64,
        queries_per_second: f64,
        timestamp: DateTime<Utc>,
    },

    // To L1 State (M05)
    StatePersist {
        services: Vec<ServiceDefinition>,
        watches: Vec<WatchSerialized>,
        catalog: ServiceCatalog,
        timestamp: DateTime<Utc>,
    },

    // To M07 Health Monitor
    ServiceRegistered {
        service: ServiceDefinition,
        health_config: Option<HealthCheckConfig>,
        timestamp: DateTime<Utc>,
    },

    ServiceDeregistered {
        service_id: ServiceId,
        reason: String,
        timestamp: DateTime<Utc>,
    },

    EndpointChanged {
        service_id: ServiceId,
        added: Vec<Endpoint>,
        removed: Vec<Endpoint>,
        timestamp: DateTime<Utc>,
    },

    // To M09 Mesh Controller
    ServiceTopologyUpdate {
        services: Vec<ServiceDefinition>,
        dependencies: HashMap<ServiceId, Vec<ServiceId>>,
        timestamp: DateTime<Utc>,
    },

    // To M10 Traffic Manager
    RoutingTableUpdate {
        routes: Vec<RouteDefinition>,
        timestamp: DateTime<Utc>,
    },

    // To M11 Load Balancer
    EndpointPoolUpdate {
        service_id: ServiceId,
        endpoints: Vec<EndpointWithWeight>,
        strategy: LoadBalanceStrategy,
        timestamp: DateTime<Utc>,
    },

    // To L3 Learning
    ServiceEvent {
        event_type: ServiceEventType,
        service: ServiceDefinition,
        context: ServiceEventContext,
        timestamp: DateTime<Utc>,
    },

    // To L6 Integration
    CatalogSync {
        catalog: ServiceCatalog,
        changed_since: Option<DateTime<Utc>>,
        timestamp: DateTime<Utc>,
    },

    // Watch events (to subscribers)
    WatchNotification {
        watch_id: WatchId,
        event: WatchEvent,
    },
}

/// Types of service events for L3 learning
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ServiceEventType {
    Registered,
    Deregistered,
    Updated,
    EndpointAdded,
    EndpointRemoved,
    HealthChanged,
    DependencyChanged,
    ExpiredDueToTTL,
}

/// Context for service events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceEventContext {
    pub previous_state: Option<ServiceDefinition>,
    pub trigger_source: String,
    pub client_id: Option<ClientId>,
    pub correlation_id: Option<String>,
}
```

#### Outbound Flow Targets

| Target | Message Type | Trigger Condition | Priority |
|--------|-------------|-------------------|----------|
| L1 Metrics (M04) | DiscoveryMetrics | Periodic collection | Normal |
| L1 State (M05) | StatePersist | State change | High |
| M07 Health | ServiceRegistered | New registration | High |
| M07 Health | ServiceDeregistered | Deregistration | High |
| M07 Health | EndpointChanged | Endpoint update | Normal |
| M09 Mesh | ServiceTopologyUpdate | Topology change | High |
| M10 Traffic | RoutingTableUpdate | Route change | High |
| M11 Load Balancer | EndpointPoolUpdate | Endpoint change | High |
| L3 Learning | ServiceEvent | All events | Normal |
| L6 Integration | CatalogSync | Periodic/on-demand | Normal |
| Watchers | WatchNotification | Matching event | High |

#### Outbound Sequence Diagram

```
                              +-------------------+
                              |  M08 SERVICE      |
                              |  DISCOVERY        |
                              +--------+----------+
                                       |
          +----------------------------+----------------------------+
          |              |             |             |              |
          v              v             v             v              v
    +---------+    +---------+   +---------+   +---------+    +---------+
    |L1:Metrics|   |L1:State |   |M07:Health|  |M09:Mesh |    |M11:LB   |
    +---------+    +---------+   +---------+   +---------+    +---------+
          |              |             |             |              |
          |              |             |             |              |
          +----------------------------+----------------------------+
                                       |
          +----------------------------+----------------------------+
          |              |             |             |              |
          v              v             v             v              v
    +---------+    +---------+   +---------+   +---------+    +---------+
    |M10:Traffic|  |L3:Learn |   |L6:Integr|  |Watchers |    |Clients  |
    +---------+    +---------+   +---------+   +---------+    +---------+
```

### Cross-Module Dependencies

#### Bidirectional Dependency Matrix

| Module | Reads From M08 | Writes To M08 | Sync/Async | Error Path |
|--------|---------------|---------------|------------|------------|
| M02 Config | - | ConfigUpdate | Async | Use defaults |
| M04 Metrics | DiscoveryMetrics | - | Async | Skip metrics |
| M05 State | StatePersist | StateRestored | Async | Start empty |
| M07 Health | ServiceRegistered/Deregistered | HealthUpdate | Async | Exclude from health |
| M09 Mesh | ServiceTopologyUpdate | - | Async | Stale topology |
| M10 Traffic | RoutingTableUpdate | - | Async | Static routes |
| M11 LB | EndpointPoolUpdate | - | Async | Equal weights |
| M12 Circuit Breaker | Endpoint resolution (query) | - | Sync | Fail open |
| L3 Learning | ServiceEvent | ServicePrediction | Async | Ignore prediction |
| L6 Integration | CatalogSync | ExternalServiceDiscovered | Async | Local only |

#### Communication Patterns

```rust
/// Communication pattern definitions for M08
pub struct ServiceDiscoveryComms {
    // Synchronous queries (blocking, immediate response)
    sync_resolver: SyncResolver,

    // Asynchronous events (fire-and-forget, buffered)
    async_events: AsyncEventEmitter,

    // Watch pattern (continuous stream)
    watch_manager: WatchManager,

    // Request-response (async with callback)
    request_handler: RequestHandler,
}

impl ServiceDiscoveryComms {
    /// Synchronous: Resolve service endpoint immediately
    pub fn resolve_sync(&self, service_id: &ServiceId) -> Option<Endpoint> {
        // Immediate response from cache or registry
        self.sync_resolver.resolve(service_id)
    }

    /// Asynchronous: Emit service events
    pub async fn emit_event(&self, event: ServiceDiscoveryOutbound) {
        // Non-blocking, buffered delivery
        self.async_events.publish(event).await;
    }

    /// Watch: Continuous stream of service changes
    pub fn create_watch(&mut self, query: ServiceQuery) -> impl Stream<Item = WatchEvent> {
        self.watch_manager.subscribe(query)
    }

    /// Request-Response: Register service with confirmation
    pub async fn register_with_confirm(&self, service: ServiceDefinition) -> Result<ServiceId, Error> {
        let (tx, rx) = oneshot::channel();
        self.request_handler.register(service, tx);
        rx.await?
    }
}
```

#### Error Propagation Paths

```
M08 Discovery Error
       |
       +---> [Log to M03 Logging] ---> Structured log entry
       |
       +---> [Encode via M01 Error Taxonomy] ---> 11D Error Vector
       |            |
       |            +---> [Send to L3 Learning] ---> Pattern recognition
       |
       +---> [Notify Watchers] ---> Error event to subscribers
       |
       +---> [Update Metrics] ---> Error counter increment
```

### Contextual Flow: Service Lifecycle

#### Service Registration State Machine

```
                                 +-------------------+
                                 |    UNREGISTERED   |
                                 |                   |
                                 | Initial state,    |
                                 | not in registry   |
                                 +--------+----------+
                                          |
                      Register request    |
                      with valid data     |
                                          v
                               +-------------------+
                               |    REGISTERING    |
                               |                   |
                               | Validating and    |
                               | adding to registry|
                               +--------+----------+
                                        |
                    Validation passed   | Validation failed
                    +-------------------+-------------------+
                    |                                       |
                    v                                       v
         +-------------------+                   +-------------------+
         |      ACTIVE       |                   |    REJECTED       |
         |                   |                   |                   |
         | Service is        |                   | Registration      |
         | registered and    |                   | denied            |
         | accepting traffic |                   +-------------------+
         +--------+----------+
                  |
                  | Health degraded OR
                  | Heartbeat timeout
                  v
         +-------------------+
         |     DEGRADED      |<----+
         |                   |     |
         | Service unhealthy |     | Still receiving
         | but still in      |     | heartbeats
         | registry          |-----+
         +--------+----------+
                  |
                  | TTL expired without
                  | heartbeat
                  v
         +-------------------+
         |     DRAINING      |
         |                   |
         | Graceful removal, |
         | existing requests |
         | complete          |
         +--------+----------+
                  |
                  | Drain complete OR
                  | Explicit deregister
                  v
         +-------------------+
         |   DEREGISTERING   |
         |                   |
         | Removing from     |
         | registry, notifying|
         | watchers          |
         +--------+----------+
                  |
                  v
         +-------------------+
         |    UNREGISTERED   |
         +-------------------+
```

#### Data Lifecycle Within Module

```rust
/// Service discovery data transformation pipeline
impl ServiceDiscovery {
    /// Complete service registration lifecycle
    async fn registration_lifecycle(&mut self, request: ServiceRegistration) -> Result<ServiceId, DiscoveryError> {
        // 1. VALIDATE: Check service definition
        let validated = self.validate_registration(&request.service)?;

        // 2. ENRICH: Add system metadata
        let enriched = self.enrich_service(validated);

        // 3. STORE: Add to registry
        let service_id = self.registry.insert(enriched.clone()).await?;

        // 4. CACHE: Update resolution cache
        self.cache.invalidate_for_service(&service_id);
        self.cache.warm(&service_id, &enriched.endpoints).await;

        // 5. NOTIFY: Inform watchers and dependents
        self.notify_registration(&enriched).await;

        // 6. PERSIST: Save to L1 State
        self.persist_state().await?;

        // 7. INTEGRATE: Inform M07 for health monitoring
        self.inform_health_monitor(&enriched).await;

        Ok(service_id)
    }

    /// Service query and resolution lifecycle
    async fn query_lifecycle(&self, query: ServiceQuery) -> DiscoveryResult {
        // 1. CACHE CHECK: Try cache first
        if let Some(cached) = self.cache.get(&query) {
            return DiscoveryResult { from_cache: true, ..cached };
        }

        // 2. QUERY: Search registry
        let matches = self.registry.query(&query);

        // 3. FILTER: Apply health filters
        let healthy = self.filter_by_health(matches, query.min_health);

        // 4. RESOLVE: Get endpoints for each service
        let with_endpoints = self.resolver.resolve_batch(&healthy).await;

        // 5. SORT: Apply strategy-based ordering
        let sorted = self.sort_by_strategy(with_endpoints, query.strategy);

        // 6. CACHE: Store result
        let result = DiscoveryResult {
            services: sorted,
            total_count: matches.len(),
            from_cache: false,
            timestamp: Utc::now(),
            query_time_ms: timer.elapsed().as_millis() as u64,
        };

        self.cache.put(&query, &result);

        result
    }
}
```

---

## Metrics

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `me_m08_registered_services` | Gauge | namespace | Current registered service count |
| `me_m08_total_endpoints` | Gauge | namespace | Total endpoint count |
| `me_m08_registrations_total` | Counter | namespace, result | Registration attempts |
| `me_m08_deregistrations_total` | Counter | namespace, reason | Deregistration count |
| `me_m08_heartbeats_total` | Counter | service | Heartbeat count |
| `me_m08_heartbeat_misses` | Counter | service | Missed heartbeats |
| `me_m08_queries_total` | Counter | type, cached | Discovery query count |
| `me_m08_query_duration_ms` | Histogram | type | Query latency distribution |
| `me_m08_cache_hit_rate` | Gauge | - | Cache hit percentage |
| `me_m08_watches_active` | Gauge | - | Active watch subscriptions |
| `me_m08_watch_events_total` | Counter | event_type | Watch events emitted |
| `me_m08_ttl_expirations` | Counter | namespace | Services expired due to TTL |

---

## Error Codes

| Code | Name | Severity | Description | Recovery |
|------|------|----------|-------------|----------|
| E8001 | ServiceNotFound | Warning | Service ID not in registry | Return empty result |
| E8002 | DuplicateRegistration | Warning | Service already registered | Update existing |
| E8003 | InvalidServiceDefinition | Error | Service definition validation failed | Reject registration |
| E8004 | EndpointResolutionFailed | Warning | Cannot resolve endpoints | Use cached |
| E8005 | RegistryFull | Critical | Max services limit reached | Reject, alert admin |
| E8006 | HeartbeatTimeout | Warning | Service missed heartbeats | Mark degraded |
| E8007 | WatchLimitExceeded | Warning | Max watches per client exceeded | Reject new watch |
| E8008 | CacheCorruption | Error | Cache inconsistency detected | Rebuild cache |
| E8009 | StatePersistFailed | Warning | Cannot persist to L1 State | Retry, log |
| E8010 | CircularDependency | Error | Circular service dependency | Reject, notify |

---

## Navigation

| Direction | Link |
|-----------|------|
| Up | [INDEX.md](../INDEX.md) |
| Layer | [L02_SERVICES.md](../layers/L02_SERVICES.md) |
| Previous | [M07_HEALTH_MONITOR.md](M07_HEALTH_MONITOR.md) |
| Next | [M09_SERVICE_MESH_CONTROLLER.md](M09_SERVICE_MESH_CONTROLLER.md) |
| Related | [M11_LOAD_BALANCER.md](M11_LOAD_BALANCER.md) |
| API | [REST_API.md](../integration/REST_API.md) |

---

*[Back to Index](../INDEX.md) | [Layer: L02 Services](../layers/L02_SERVICES.md)*
