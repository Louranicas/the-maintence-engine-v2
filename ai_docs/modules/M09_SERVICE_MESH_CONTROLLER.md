# Module M09: Service Mesh Controller

> **M09_SERVICE_MESH_CONTROLLER** | Mesh Traffic & Policy Orchestration | Layer: L2 Services | [Back to Index](../INDEX.md)

---

## Navigation

| Direction | Link |
|-----------|------|
| Up | [INDEX.md](../INDEX.md) |
| Layer | [L02_SERVICES.md](../layers/L02_SERVICES.md) |
| Previous | [M08_SERVICE_DISCOVERY.md](M08_SERVICE_DISCOVERY.md) |
| Next | [M10_TRAFFIC_MANAGER.md](M10_TRAFFIC_MANAGER.md) |
| Related | [M12_CIRCUIT_BREAKER.md](M12_CIRCUIT_BREAKER.md) |
| L1 Foundation | [L01_FOUNDATION.md](../layers/L01_FOUNDATION.md) |
| L3 Learning | [L03_LEARNING.md](../layers/L03_LEARNING.md) |

---

## Module Specification

### Overview

The Service Mesh Controller provides centralized policy management, traffic orchestration, and security enforcement across all service-to-service communication in the Maintenance Engine. It implements mTLS, observability injection, retry policies, and traffic shaping while integrating with M10 Traffic Manager and M11 Load Balancer for comprehensive mesh control.

### Module Properties

| Property | Value |
|----------|-------|
| Module ID | M09 |
| Module Name | Service Mesh Controller |
| Layer | L2 (Services) |
| Version | 1.0.0 |
| Dependencies | M02 (Config), M07 (Health Monitor), M08 (Service Discovery) |
| Dependents | M10 (Traffic Manager), M11 (Load Balancer), M12 (Circuit Breaker), L3 (Learning), L4 (Consensus), L5 (Remediation) |

---

## Architecture Diagram

```
+-----------------------------------------------------------------------------------+
|                        M09: SERVICE MESH CONTROLLER                                |
+-----------------------------------------------------------------------------------+
|                                                                                   |
|  +-------------------------+    +-------------------------+    +--------------+   |
|  |   CONTROL PLANE         |    |   DATA PLANE PROXY      |    | POLICY MGR   |   |
|  |                         |    |                         |    |              |   |
|  | - Config distribution   |    | - Sidecar injection     |    | - Auth       |   |
|  | - Service mesh state    |--->| - Traffic interception  |--->|   policies   |   |
|  | - Topology management   |    | - Protocol detection    |    | - Rate limit |   |
|  | - Certificate rotation  |    | - Load balancing        |    | - Quota      |   |
|  +------------+------------+    +------------+------------+    +--------------+   |
|               |                              |                        |           |
|               v                              v                        v           |
|  +-------------------------+    +-------------------------+    +--------------+   |
|  |   mTLS MANAGER          |    |   OBSERVABILITY         |    | TRAFFIC CTRL |   |
|  |                         |    |                         |    |              |   |
|  | - Certificate issuance  |    | - Distributed tracing   |    | -> M10       |   |
|  | - Key rotation          |    | - Metrics collection    |    | -> M11       |   |
|  | - Trust anchors         |    | - Access logging        |    | -> M12       |   |
|  +-------------------------+    +-------------------------+    +--------------+   |
|                                                                                   |
+-----------------------------------------------------------------------------------+
        ^                    ^                    ^                    |
        |                    |                    |                    v
   [L1: Config]        [M08: Discovery]    [M07: Health]        [Downstream]
```

---

## Core Data Structures

### Mesh Configuration

```rust
/// Complete mesh configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeshConfig {
    /// Mesh identifier
    pub mesh_id: MeshId,

    /// Mesh name
    pub name: String,

    /// Global mesh settings
    pub global: GlobalMeshSettings,

    /// mTLS configuration
    pub mtls: MtlsConfig,

    /// Observability settings
    pub observability: ObservabilityConfig,

    /// Default traffic policies
    pub default_policies: DefaultPolicies,

    /// Service-specific overrides
    pub service_overrides: HashMap<ServiceId, ServiceMeshConfig>,
}

/// Global mesh settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalMeshSettings {
    /// Enable mesh for all services
    pub enabled: bool,

    /// Default protocol (auto, http, grpc, tcp)
    pub default_protocol: Protocol,

    /// Enable automatic sidecar injection
    pub auto_inject_sidecar: bool,

    /// Access log format
    pub access_log_format: String,

    /// Egress policy (allow_any, registry_only)
    pub egress_policy: EgressPolicy,

    /// Mesh-wide timeout
    pub global_timeout: Duration,
}

/// mTLS configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MtlsConfig {
    /// mTLS mode (disabled, permissive, strict)
    pub mode: MtlsMode,

    /// Minimum TLS version
    pub min_tls_version: TlsVersion,

    /// Certificate authority configuration
    pub ca_config: CaConfig,

    /// Certificate rotation interval
    pub cert_rotation_interval: Duration,

    /// Trust domains
    pub trust_domains: Vec<String>,

    /// Skip verification for specified services
    pub skip_verify: HashSet<ServiceId>,
}

/// mTLS modes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MtlsMode {
    /// mTLS disabled
    Disabled,
    /// Accept both mTLS and plaintext
    Permissive,
    /// Require mTLS for all traffic
    Strict,
}
```

### Traffic Policy

```rust
/// Traffic policy for a service or route
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrafficPolicy {
    /// Policy identifier
    pub id: PolicyId,

    /// Policy name
    pub name: String,

    /// Target services (empty = all)
    pub targets: Vec<ServiceSelector>,

    /// Connection pool settings
    pub connection_pool: ConnectionPoolSettings,

    /// Load balancing settings
    pub load_balancing: LoadBalancingSettings,

    /// Outlier detection (ejection)
    pub outlier_detection: OutlierDetectionSettings,

    /// TLS settings
    pub tls: TlsSettings,

    /// Retry policy
    pub retry_policy: RetryPolicy,

    /// Timeout policy
    pub timeout_policy: TimeoutPolicy,

    /// Rate limiting
    pub rate_limit: Option<RateLimitPolicy>,

    /// Policy priority (higher = more specific)
    pub priority: u32,

    /// Policy status
    pub status: PolicyStatus,
}

/// Connection pool settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionPoolSettings {
    /// TCP connection pool
    pub tcp: TcpConnectionPool,

    /// HTTP connection pool
    pub http: HttpConnectionPool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TcpConnectionPool {
    /// Maximum connections
    pub max_connections: u32,

    /// Connection timeout
    pub connect_timeout: Duration,

    /// TCP keepalive
    pub tcp_keepalive: Option<Duration>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpConnectionPool {
    /// HTTP/1.1 max pending requests
    pub h1_max_pending_requests: u32,

    /// HTTP/2 max requests per connection
    pub h2_max_requests: u32,

    /// Max requests per connection
    pub max_requests_per_connection: u32,

    /// Max retries
    pub max_retries: u32,

    /// Idle timeout
    pub idle_timeout: Duration,
}

/// Retry policy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryPolicy {
    /// Number of retries
    pub attempts: u32,

    /// Per-try timeout
    pub per_try_timeout: Duration,

    /// Retry conditions
    pub retry_on: Vec<RetryCondition>,

    /// Retriable status codes
    pub retriable_status_codes: Vec<u16>,

    /// Backoff configuration
    pub backoff: BackoffConfig,
}

/// Retry conditions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RetryCondition {
    FiveXx,
    GatewayError,
    Reset,
    ConnectFailure,
    Refused,
    Retriable4xx,
    Cancelled,
    DeadlineExceeded,
    ResourceExhausted,
    Unavailable,
}
```

### Service Entry (for external services)

```rust
/// External service entry in mesh
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceEntry {
    /// Entry identifier
    pub id: ServiceEntryId,

    /// Service name (DNS)
    pub hosts: Vec<String>,

    /// Service addresses
    pub addresses: Vec<String>,

    /// Service ports
    pub ports: Vec<ServicePort>,

    /// Location (mesh_internal, mesh_external)
    pub location: ServiceLocation,

    /// Resolution mode (none, static, dns)
    pub resolution: ResolutionMode,

    /// Endpoints for static resolution
    pub endpoints: Vec<WorkloadEntry>,

    /// Export to namespaces
    pub export_to: Vec<String>,
}

/// Service port definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServicePort {
    pub number: u16,
    pub protocol: Protocol,
    pub name: String,
    pub target_port: Option<u16>,
}
```

### Sidecar Configuration

```rust
/// Sidecar proxy configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SidecarConfig {
    /// Target workload selector
    pub workload_selector: WorkloadSelector,

    /// Inbound listener configuration
    pub inbound: InboundConfig,

    /// Outbound listener configuration
    pub outbound: OutboundConfig,

    /// Egress rules
    pub egress: Vec<EgressRule>,

    /// Ingress rules
    pub ingress: Vec<IngressRule>,
}

/// Inbound traffic configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InboundConfig {
    /// Capture mode (default, none, all)
    pub capture_mode: CaptureMode,

    /// Default endpoint port
    pub default_endpoint_port: u16,

    /// Port overrides
    pub port_overrides: HashMap<u16, PortConfig>,
}

/// Outbound traffic configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutboundConfig {
    /// Outbound traffic policy
    pub policy: OutboundPolicy,

    /// Allowed hosts for egress
    pub hosts: Vec<String>,
}

/// Outbound traffic policies
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OutboundPolicy {
    /// Allow traffic to any host
    AllowAny,
    /// Only allow registered services
    RegistryOnly,
}
```

---

## Public API

### ServiceMeshController Service

```rust
/// Main Service Mesh Controller
pub struct ServiceMeshController {
    config: MeshConfig,
    control_plane: ControlPlane,
    data_plane: DataPlaneProxy,
    policy_manager: PolicyManager,
    mtls_manager: MtlsManager,
    observability: ObservabilityManager,
    metrics: MeshMetrics,
}

impl ServiceMeshController {
    /// Create a new ServiceMeshController instance
    pub fn new(config: MeshConfig) -> Self;

    /// Start the mesh controller
    pub async fn start(&mut self) -> Result<(), MeshError>;

    /// Stop the mesh controller
    pub async fn stop(&mut self) -> Result<(), MeshError>;

    // === Configuration API ===

    /// Apply mesh configuration
    pub async fn apply_config(&mut self, config: MeshConfig) -> Result<(), MeshError>;

    /// Get current mesh configuration
    pub fn get_config(&self) -> &MeshConfig;

    /// Validate configuration before applying
    pub fn validate_config(&self, config: &MeshConfig) -> ValidationResult;

    // === Policy Management API ===

    /// Create a new traffic policy
    pub async fn create_policy(&mut self, policy: TrafficPolicy) -> Result<PolicyId, MeshError>;

    /// Update an existing policy
    pub async fn update_policy(&mut self, policy: TrafficPolicy) -> Result<(), MeshError>;

    /// Delete a policy
    pub async fn delete_policy(&mut self, policy_id: &PolicyId) -> Result<(), MeshError>;

    /// Get policy by ID
    pub fn get_policy(&self, policy_id: &PolicyId) -> Option<&TrafficPolicy>;

    /// List all policies
    pub fn list_policies(&self) -> Vec<&TrafficPolicy>;

    /// Get effective policy for a service
    pub fn get_effective_policy(&self, service_id: &ServiceId) -> TrafficPolicy;

    // === mTLS API ===

    /// Configure mTLS mode
    pub async fn set_mtls_mode(&mut self, mode: MtlsMode) -> Result<(), MeshError>;

    /// Get current mTLS mode
    pub fn get_mtls_mode(&self) -> MtlsMode;

    /// Issue certificate for a workload
    pub async fn issue_certificate(&self, workload_id: &WorkloadId) -> Result<Certificate, MeshError>;

    /// Rotate certificates for all workloads
    pub async fn rotate_certificates(&mut self) -> Result<RotationReport, MeshError>;

    /// Get certificate status
    pub fn get_certificate_status(&self, workload_id: &WorkloadId) -> Option<CertificateStatus>;

    // === Service Entry API ===

    /// Register external service
    pub async fn register_service_entry(&mut self, entry: ServiceEntry) -> Result<ServiceEntryId, MeshError>;

    /// Remove external service
    pub async fn remove_service_entry(&mut self, entry_id: &ServiceEntryId) -> Result<(), MeshError>;

    /// List external services
    pub fn list_service_entries(&self) -> Vec<&ServiceEntry>;

    // === Sidecar API ===

    /// Configure sidecar for workload
    pub async fn configure_sidecar(&mut self, config: SidecarConfig) -> Result<(), MeshError>;

    /// Get sidecar configuration
    pub fn get_sidecar_config(&self, workload_id: &WorkloadId) -> Option<&SidecarConfig>;

    /// Inject sidecar into workload (if auto-inject disabled)
    pub async fn inject_sidecar(&mut self, workload_id: &WorkloadId) -> Result<(), MeshError>;

    // === Observability API ===

    /// Get mesh topology
    pub fn get_topology(&self) -> MeshTopology;

    /// Get traffic metrics between services
    pub fn get_traffic_metrics(&self, from: &ServiceId, to: &ServiceId) -> Option<TrafficMetrics>;

    /// Get access logs
    pub fn get_access_logs(&self, filter: AccessLogFilter) -> Vec<AccessLogEntry>;

    /// Get distributed traces
    pub async fn get_traces(&self, filter: TraceFilter) -> Vec<DistributedTrace>;
}
```

### PolicyManager API

```rust
/// Manages traffic policies
pub struct PolicyManager {
    /// Create policy
    pub async fn create(&mut self, policy: TrafficPolicy) -> Result<PolicyId, PolicyError>;

    /// Update policy
    pub async fn update(&mut self, policy: TrafficPolicy) -> Result<(), PolicyError>;

    /// Delete policy
    pub async fn delete(&mut self, id: &PolicyId) -> Result<(), PolicyError>;

    /// Get policy
    pub fn get(&self, id: &PolicyId) -> Option<&TrafficPolicy>;

    /// List policies matching selector
    pub fn list(&self, selector: PolicySelector) -> Vec<&TrafficPolicy>;

    /// Compute effective policy for service
    pub fn compute_effective(&self, service_id: &ServiceId) -> TrafficPolicy;

    /// Validate policy
    pub fn validate(&self, policy: &TrafficPolicy) -> ValidationResult;
}
```

---

## Configuration

### TOML Configuration Example

```toml
[layer.L2.M09]
enabled = true
version = "1.0.0"

# Global mesh settings
[layer.L2.M09.global]
enabled = true
default_protocol = "auto"
auto_inject_sidecar = true
access_log_format = "[%START_TIME%] \"%REQ(:METHOD)% %REQ(X-ENVOY-ORIGINAL-PATH?:PATH)% %PROTOCOL%\" %RESPONSE_CODE%"
egress_policy = "registry_only"
global_timeout_ms = 30000

# mTLS configuration
[layer.L2.M09.mtls]
mode = "strict"
min_tls_version = "1.3"
cert_rotation_interval_hours = 24
trust_domains = ["cluster.local", "maintenance-engine.local"]

[layer.L2.M09.mtls.ca]
type = "self_signed"
key_size = 4096
validity_days = 365
organization = "MaintenanceEngine"

# Observability settings
[layer.L2.M09.observability]
tracing_enabled = true
tracing_sample_rate = 0.1
metrics_enabled = true
access_logging_enabled = true
log_format = "json"

# Default policies
[layer.L2.M09.defaults.connection_pool.tcp]
max_connections = 1000
connect_timeout_ms = 5000

[layer.L2.M09.defaults.connection_pool.http]
h1_max_pending_requests = 1024
h2_max_requests = 1000
max_requests_per_connection = 100
idle_timeout_ms = 60000

[layer.L2.M09.defaults.retry]
attempts = 3
per_try_timeout_ms = 2000
retry_on = ["5xx", "reset", "connect-failure"]
backoff_base_ms = 25
backoff_max_ms = 250

[layer.L2.M09.defaults.timeout]
request_timeout_ms = 15000
idle_timeout_ms = 300000

[layer.L2.M09.defaults.outlier_detection]
consecutive_errors = 5
interval_ms = 10000
base_ejection_time_ms = 30000
max_ejection_percent = 50

# Service-specific overrides
[[layer.L2.M09.service_overrides]]
service = "synthex"
retry_attempts = 5
timeout_ms = 60000
connection_pool_max = 500

[[layer.L2.M09.service_overrides]]
service = "san-k7"
retry_attempts = 3
timeout_ms = 30000

# External service entries
[[layer.L2.M09.service_entries]]
name = "external-api"
hosts = ["api.external.com"]
ports = [{ number = 443, protocol = "https", name = "https" }]
resolution = "dns"
location = "mesh_external"
```

---

## Bi-Directional Data Flow

### Inbound Data Flow

The Service Mesh Controller receives configuration, topology, and health data from multiple sources.

#### Inbound Message Types

```rust
/// Messages received by Service Mesh Controller
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MeshControllerInbound {
    // From L1 Config (M02)
    ConfigUpdate {
        mesh_config: MeshConfig,
        policies: Vec<TrafficPolicy>,
        service_entries: Vec<ServiceEntry>,
        timestamp: DateTime<Utc>,
    },

    // From L1 State (M05)
    StateRestored {
        policies: Vec<TrafficPolicy>,
        certificates: HashMap<WorkloadId, CertificateState>,
        sidecar_configs: Vec<SidecarConfig>,
        timestamp: DateTime<Utc>,
    },

    // From M07 Health Monitor
    ServiceHealthUpdate {
        service_id: ServiceId,
        health_state: HealthState,
        endpoints_health: HashMap<EndpointId, HealthState>,
        timestamp: DateTime<Utc>,
    },

    // From M08 Service Discovery
    ServiceTopologyUpdate {
        services: Vec<ServiceDefinition>,
        dependencies: HashMap<ServiceId, Vec<ServiceId>>,
        timestamp: DateTime<Utc>,
    },

    ServiceRegistered {
        service: ServiceDefinition,
        timestamp: DateTime<Utc>,
    },

    ServiceDeregistered {
        service_id: ServiceId,
        timestamp: DateTime<Utc>,
    },

    // From L3 Learning
    PolicyRecommendation {
        service_id: ServiceId,
        recommended_policy: TrafficPolicy,
        confidence: f64,
        rationale: String,
        timestamp: DateTime<Utc>,
    },

    AnomalyPattern {
        pattern_type: TrafficAnomalyType,
        affected_services: Vec<ServiceId>,
        severity: Severity,
        timestamp: DateTime<Utc>,
    },

    // From L4 Consensus
    PolicyConsensusResult {
        policy_id: PolicyId,
        approved: bool,
        votes: ConsensusVotes,
        timestamp: DateTime<Utc>,
    },

    // From L5 Remediation
    RemediationAction {
        action_type: MeshRemediationAction,
        target: ServiceId,
        parameters: HashMap<String, Value>,
        timestamp: DateTime<Utc>,
    },
}

/// Traffic anomaly types detected by L3
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TrafficAnomalyType {
    LatencySpike,
    ErrorRateIncrease,
    TrafficSurge,
    ConnectionPoolExhaustion,
    CertificateExpiringSoon,
    UnusualTrafficPattern,
}

/// Mesh-specific remediation actions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MeshRemediationAction {
    IsolateService,
    AdjustRateLimit,
    RotateCertificate,
    UpdateRetryPolicy,
    ResetConnectionPool,
    EnableCircuitBreaker,
}
```

#### Inbound Flow Sources

| Source | Message Type | Trigger Condition | Frequency |
|--------|-------------|-------------------|-----------|
| L1 Config (M02) | ConfigUpdate | Config file change | On change |
| L1 State (M05) | StateRestored | System startup | On startup |
| M07 Health | ServiceHealthUpdate | Health state change | On change |
| M08 Discovery | ServiceTopologyUpdate | Topology change | On change |
| M08 Discovery | ServiceRegistered | New service | On registration |
| M08 Discovery | ServiceDeregistered | Service removal | On deregistration |
| L3 Learning | PolicyRecommendation | ML recommendation | As generated |
| L3 Learning | AnomalyPattern | Anomaly detected | On detection |
| L4 Consensus | PolicyConsensusResult | Policy vote complete | On consensus |
| L5 Remediation | RemediationAction | Remediation triggered | On action |

#### Inbound Sequence Diagram

```
  L1:Config   L1:State   M07:Health   M08:Discovery   L3:Learning   L4:Consensus
      |           |           |             |              |              |
      | ConfigUpdate          |             |              |              |
      |---------->|           |             |              |              |
      |           |           |             |              |              |
      |           | StateRestored           |              |              |
      |           |---------->|             |              |              |
      |           |           |             |              |              |
      |           |           | HealthUpdate|              |              |
      |           |           |------------>|              |              |
      |           |           |             |              |              |
      |           |           |             | TopologyUpdate             |
      |           |           |             |------------->|              |
      |           |           |             |              |              |
      |           |           |             |              | PolicyRecommendation
      |           |           |             |              |------------->|
      |           |           |             |              |              |
      |           |           |             |              |     ConsensusResult
      |           |           |             |              |<-------------|
      |           |           |             |              |              |
      +-----------+-----------+-------------+--------------+--------------+
                                     |
                                     v
                           +-------------------+
                           |  M09 MESH         |
                           |  CONTROLLER       |
                           |                   |
                           | - Update config   |
                           | - Apply policies  |
                           | - Manage certs    |
                           +-------------------+
```

### Outbound Data Flow

The Service Mesh Controller emits policy updates, traffic data, and observability information.

#### Outbound Message Types

```rust
/// Messages emitted by Service Mesh Controller
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MeshControllerOutbound {
    // To L1 Metrics (M04)
    MeshMetrics {
        total_requests: u64,
        success_rate: f64,
        avg_latency_ms: u64,
        p99_latency_ms: u64,
        active_connections: u64,
        mtls_coverage: f64,
        timestamp: DateTime<Utc>,
    },

    // To L1 State (M05)
    StatePersist {
        policies: Vec<TrafficPolicy>,
        certificates: HashMap<WorkloadId, CertificateState>,
        sidecar_configs: Vec<SidecarConfig>,
        timestamp: DateTime<Utc>,
    },

    // To M10 Traffic Manager
    TrafficPolicyUpdate {
        policies: Vec<TrafficPolicy>,
        routing_rules: Vec<RoutingRule>,
        timestamp: DateTime<Utc>,
    },

    // To M11 Load Balancer
    LoadBalancingUpdate {
        service_id: ServiceId,
        lb_config: LoadBalancingSettings,
        outlier_config: OutlierDetectionSettings,
        timestamp: DateTime<Utc>,
    },

    // To M12 Circuit Breaker
    CircuitBreakerPolicy {
        service_id: ServiceId,
        thresholds: CircuitBreakerThresholds,
        timestamp: DateTime<Utc>,
    },

    // To L3 Learning
    MeshEvent {
        event_type: MeshEventType,
        service_id: ServiceId,
        details: MeshEventDetails,
        context: MeshEventContext,
        timestamp: DateTime<Utc>,
    },

    TrafficData {
        source: ServiceId,
        destination: ServiceId,
        metrics: TrafficMetrics,
        timestamp: DateTime<Utc>,
    },

    // To L4 Consensus
    PolicyApprovalRequest {
        policy: TrafficPolicy,
        change_type: PolicyChangeType,
        risk_assessment: RiskAssessment,
        requestor: AgentId,
        timestamp: DateTime<Utc>,
    },

    // To L5 Remediation
    MeshAlert {
        alert_type: MeshAlertType,
        service_id: ServiceId,
        severity: Severity,
        details: String,
        suggested_action: Option<MeshRemediationAction>,
        timestamp: DateTime<Utc>,
    },

    // To L6 Integration
    MeshTopologyExport {
        topology: MeshTopology,
        policies: Vec<TrafficPolicy>,
        timestamp: DateTime<Utc>,
    },
}

/// Types of mesh events for L3 learning
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MeshEventType {
    PolicyApplied,
    PolicyRemoved,
    CertificateRotated,
    ConnectionPoolExhausted,
    OutlierEjected,
    OutlierRestored,
    RateLimitTriggered,
    RetryExhausted,
    TimeoutOccurred,
}

/// Mesh alert types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MeshAlertType {
    CertificateExpiring,
    HighErrorRate,
    LatencyDegradation,
    ConnectionPoolSaturation,
    PolicyConflict,
    MtlsFailure,
}
```

#### Outbound Flow Targets

| Target | Message Type | Trigger Condition | Priority |
|--------|-------------|-------------------|----------|
| L1 Metrics (M04) | MeshMetrics | Periodic collection | Normal |
| L1 State (M05) | StatePersist | State change | High |
| M10 Traffic | TrafficPolicyUpdate | Policy change | High |
| M11 Load Balancer | LoadBalancingUpdate | LB config change | High |
| M12 Circuit Breaker | CircuitBreakerPolicy | CB config change | High |
| L3 Learning | MeshEvent | Significant events | Normal |
| L3 Learning | TrafficData | Continuous stream | Low |
| L4 Consensus | PolicyApprovalRequest | High-impact policy | Critical |
| L5 Remediation | MeshAlert | Threshold breached | Critical |
| L6 Integration | MeshTopologyExport | Periodic/on-demand | Normal |

#### Outbound Sequence Diagram

```
                           +-------------------+
                           |  M09 MESH         |
                           |  CONTROLLER       |
                           +--------+----------+
                                    |
       +----------------------------+----------------------------+
       |              |             |             |              |
       v              v             v             v              v
  +---------+   +---------+   +---------+   +---------+    +---------+
  |L1:Metrics|  |M10:Traffic| |M11:LB   |   |M12:CB   |    |L3:Learn |
  +---------+   +---------+   +---------+   +---------+    +---------+
       |              |             |             |              |
       |              |             |             |              |
       +----------------------------+----------------------------+
                                    |
       +----------------------------+----------------------------+
       |              |             |             |              |
       v              v             v             v              v
  +---------+   +---------+   +---------+   +---------+    +---------+
  |L1:State |   |L4:Consens|  |L5:Remed |   |L6:Integr|   |Sidecars |
  +---------+   +---------+   +---------+   +---------+    +---------+
```

### Cross-Module Dependencies

#### Bidirectional Dependency Matrix

| Module | Reads From M09 | Writes To M09 | Sync/Async | Error Path |
|--------|---------------|---------------|------------|------------|
| M02 Config | - | ConfigUpdate | Async | Use defaults |
| M04 Metrics | MeshMetrics | - | Async | Skip metrics |
| M05 State | StatePersist | StateRestored | Async | Start fresh |
| M07 Health | - | ServiceHealthUpdate | Async | Use cached |
| M08 Discovery | - | TopologyUpdate | Async | Stale topology |
| M10 Traffic | TrafficPolicyUpdate | - | Async | Static routes |
| M11 Load Balancer | LoadBalancingUpdate | - | Async | Default config |
| M12 Circuit Breaker | CircuitBreakerPolicy | - | Async | Default thresholds |
| L3 Learning | MeshEvent, TrafficData | PolicyRecommendation, AnomalyPattern | Async | Ignore recommendations |
| L4 Consensus | PolicyApprovalRequest | PolicyConsensusResult | Async | Auto-approve low-risk |
| L5 Remediation | MeshAlert | RemediationAction | Async | Alert only |

#### Communication Patterns

```rust
/// Communication patterns for M09
pub struct MeshControllerComms {
    // Synchronous queries
    sync_queries: SyncQueryHandler,

    // Asynchronous events
    async_events: AsyncEventEmitter,

    // Streaming data
    streaming: StreamManager,

    // Control plane distribution
    control_plane: ControlPlaneDistributor,
}

impl MeshControllerComms {
    /// Synchronous: Query effective policy
    pub fn get_effective_policy(&self, service_id: &ServiceId) -> TrafficPolicy {
        self.policy_manager.compute_effective(service_id)
    }

    /// Asynchronous: Emit mesh events
    pub async fn emit_event(&self, event: MeshControllerOutbound) {
        self.async_events.publish(event).await;
    }

    /// Streaming: Continuous traffic data
    pub fn traffic_data_stream(&self) -> impl Stream<Item = TrafficData> {
        self.streaming.subscribe_traffic()
    }

    /// Control plane: Push config to sidecars
    pub async fn push_sidecar_config(&self, config: SidecarConfig) {
        self.control_plane.distribute(config).await;
    }
}
```

#### Error Propagation Paths

```
M09 Mesh Error
       |
       +---> [Log to M03 Logging] ---> Structured log entry
       |
       +---> [Encode via M01 Error Taxonomy] ---> 11D Error Vector
       |            |
       |            +---> [Send to L3 Learning] ---> Pattern recognition
       |
       +---> [Alert to L5 Remediation] ---> Trigger mesh remediation
       |
       +---> [Notify L4 Consensus] ---> If policy-related failure
```

### Contextual Flow: Policy Application Lifecycle

#### Policy State Machine

```
                                 +-------------------+
                                 |     CREATED       |
                                 |                   |
                                 | Policy defined    |
                                 | but not active    |
                                 +--------+----------+
                                          |
                      Validation passed   | Validation failed
                      +-------------------+-------------------+
                      |                                       |
                      v                                       v
           +-------------------+                   +-------------------+
           |    VALIDATING     |                   |    REJECTED       |
           |                   |                   |                   |
           | Checking policy   |                   | Policy invalid    |
           | constraints       |                   +-------------------+
           +--------+----------+
                    |
                    | Low risk: auto-approve
                    | High risk: require consensus
                    +-------------------+
                    |                   |
                    v                   v
         +-------------------+ +-------------------+
         |  PENDING_APPROVAL | |    APPROVED       |
         |                   | |                   |
         | Awaiting L4       | | Ready for         |
         | consensus         | | distribution      |
         +--------+----------+ +--------+----------+
                  |                     |
                  | Consensus reached   |
                  +----------+----------+
                             |
                             v
                  +-------------------+
                  |   DISTRIBUTING    |
                  |                   |
                  | Pushing to        |
                  | sidecars          |
                  +--------+----------+
                           |
                           | All sidecars confirmed
                           v
                  +-------------------+
                  |      ACTIVE       |<----+
                  |                   |     |
                  | Policy enforced   |     | Policy updated
                  | in data plane     |-----+
                  +--------+----------+
                           |
                           | Policy removed
                           v
                  +-------------------+
                  |    DEPRECATED     |
                  |                   |
                  | Policy disabled   |
                  | but kept in       |
                  | history           |
                  +-------------------+
```

#### Data Lifecycle Within Module

```rust
/// Mesh policy application lifecycle
impl ServiceMeshController {
    /// Complete policy lifecycle
    async fn apply_policy_lifecycle(&mut self, policy: TrafficPolicy) -> Result<PolicyId, MeshError> {
        // 1. VALIDATE: Check policy constraints
        let validation = self.policy_manager.validate(&policy)?;

        // 2. RISK ASSESS: Determine if consensus needed
        let risk = self.assess_policy_risk(&policy);

        // 3. APPROVAL: Get consensus for high-risk policies
        if risk.level >= RiskLevel::High {
            let approval = self.request_consensus(&policy).await?;
            if !approval.approved {
                return Err(MeshError::PolicyRejected);
            }
        }

        // 4. STORE: Persist policy
        let policy_id = self.policy_manager.create(policy.clone()).await?;

        // 5. MERGE: Compute effective policies
        let affected_services = self.find_affected_services(&policy);
        for service_id in &affected_services {
            let effective = self.policy_manager.compute_effective(service_id);
            self.effective_cache.update(service_id, effective);
        }

        // 6. DISTRIBUTE: Push to data plane
        self.distribute_to_sidecars(&policy).await?;

        // 7. NOTIFY: Inform downstream modules
        self.notify_policy_change(&policy, &affected_services).await;

        Ok(policy_id)
    }
}
```

---

## Metrics

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `me_m09_requests_total` | Counter | source, dest, status | Total mesh requests |
| `me_m09_request_duration_ms` | Histogram | source, dest | Request latency |
| `me_m09_active_connections` | Gauge | service | Active connection count |
| `me_m09_connection_pool_usage` | Gauge | service | Connection pool utilization |
| `me_m09_retries_total` | Counter | service, result | Retry attempts |
| `me_m09_outlier_ejections` | Counter | service | Outlier ejection count |
| `me_m09_rate_limit_exceeded` | Counter | service | Rate limit violations |
| `me_m09_policies_active` | Gauge | - | Active policy count |
| `me_m09_mtls_connections` | Gauge | service | mTLS connection count |
| `me_m09_certificate_expiry_days` | Gauge | workload | Days until cert expiry |
| `me_m09_sidecar_config_pushes` | Counter | result | Config push count |

---

## Error Codes

| Code | Name | Severity | Description | Recovery |
|------|------|----------|-------------|----------|
| E9001 | PolicyValidationFailed | Error | Policy constraints violated | Fix policy config |
| E9002 | PolicyConflict | Warning | Conflicting policies detected | Resolve conflict |
| E9003 | SidecarPushFailed | Error | Cannot push config to sidecar | Retry with backoff |
| E9004 | CertificateIssuanceFailed | Critical | Cannot issue certificate | Check CA config |
| E9005 | CertificateRotationFailed | Critical | Cert rotation failed | Manual intervention |
| E9006 | ConnectionPoolExhausted | Warning | No available connections | Increase pool size |
| E9007 | OutlierThresholdExceeded | Warning | Too many outlier ejections | Review health |
| E9008 | MtlsHandshakeFailed | Error | mTLS handshake error | Check certificates |
| E9009 | ConsensusTimeout | Warning | Policy consensus timed out | Auto-reject or retry |
| E9010 | TopologyInconsistent | Error | Mesh topology inconsistent | Resync from M08 |

---

## Navigation

| Direction | Link |
|-----------|------|
| Up | [INDEX.md](../INDEX.md) |
| Layer | [L02_SERVICES.md](../layers/L02_SERVICES.md) |
| Previous | [M08_SERVICE_DISCOVERY.md](M08_SERVICE_DISCOVERY.md) |
| Next | [M10_TRAFFIC_MANAGER.md](M10_TRAFFIC_MANAGER.md) |
| Related | [M12_CIRCUIT_BREAKER.md](M12_CIRCUIT_BREAKER.md) |
| API | [REST_API.md](../integration/REST_API.md) |

---

*[Back to Index](../INDEX.md) | [Layer: L02 Services](../layers/L02_SERVICES.md)*
