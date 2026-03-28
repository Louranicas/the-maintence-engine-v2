# Layer 4: Integration

> **L04_INTEGRATION** | Service Bridge Layer | [Back to Index](../INDEX.md)

---

## Navigation

| Direction | Link |
|-----------|------|
| Up | [INDEX.md](../INDEX.md) |
| Previous | [L03_CORE_LOGIC.md](L03_CORE_LOGIC.md) |
| Next | [L05_LEARNING.md](L05_LEARNING.md) |
| Related | [Service Registry](../../service_registry/SERVICE_REGISTRY.md) |

---

## Layer Overview

The Integration Layer (L4) provides connectivity to all 12 ULTRAPLATE services through wire protocols, service endpoints, and bridge adapters. It acts as the interface between the Maintenance Engine's internal logic and external services, enabling cross-system coordination and monitoring.

### Layer Properties

| Property | Value |
|----------|-------|
| Layer ID | L4 |
| Layer Name | Integration |
| Source Directory | `src/m4_integration/` |
| Dependencies | L1, L2, L3 |
| Dependents | L5, L6 |
| Modules | M19-M24 |
| Service Bridges | 12 ULTRAPLATE services |
| Protocols | REST, gRPC, WebSocket, IPC |

---

## Architecture

```
+------------------------------------------------------------------+
|                      L4: Integration Layer                         |
+------------------------------------------------------------------+
|                                                                  |
|  +---------------------------+  +---------------------------+    |
|  |     Wire Protocol         |  |    Service Endpoints      |    |
|  |                           |  |                           |    |
|  |  - REST client            |  |  - SYNTHEX (8090/8091)    |    |
|  |  - gRPC client            |  |  - SAN-K7 (8100)          |    |
|  |  - WebSocket client       |  |  - NAIS (8101)            |    |
|  |  - IPC channels           |  |  - Tool Library (8105)    |    |
|  +-------------+-------------+  +-------------+-------------+    |
|                |                              |                  |
|                +------------+-----------------+                  |
|                             |                                    |
|  +-------------------------------------------------------+       |
|  |                  Service Bridge Hub                    |       |
|  |                                                        |       |
|  |  +--------+  +--------+  +--------+  +--------+        |       |
|  |  |SYNTHEX |  | SAN-K7 |  |  NAIS  |  |CodeSyn.|        |       |
|  |  +--------+  +--------+  +--------+  +--------+        |       |
|  |  +--------+  +--------+  +--------+  +--------+        |       |
|  |  |DevOps  |  |ToolLib |  |LibAgent|  |  CCM   |        |       |
|  |  +--------+  +--------+  +--------+  +--------+        |       |
|  |  +--------+  +--------+  +--------+  +--------+        |       |
|  |  |Prometh.|  |Archit. |  | Bash   |  |ToolMkr |        |       |
|  |  +--------+  +--------+  +--------+  +--------+        |       |
|  +-------------------------------------------------------+       |
|                                                                  |
+------------------------------------------------------------------+
```

---

## Module Reference (M19-M24)

| Module | File | Purpose |
|--------|------|---------|
| M19 | `synthex.rs` | SYNTHEX bridge (8090/8091) |
| M20 | `sank7.rs` | SAN-K7 Orchestrator bridge (8100) |
| M21 | `nais.rs` | NAIS Intelligence bridge (8101) |
| M22 | `tool_library.rs` | Tool Library bridge (8105) |
| M23 | `codesynthor.rs` | CodeSynthor V7 bridge (8110) |
| M24 | `devops.rs` | DevOps Engine bridge (8081) |

---

## Core Types

### WireProtocol

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum WireProtocol {
    /// REST over HTTP/HTTPS
    Rest { base_url: String, timeout_ms: u64 },

    /// gRPC with protocol buffers
    Grpc { endpoint: String, tls: bool },

    /// WebSocket for real-time
    WebSocket { url: String, reconnect: bool },

    /// Inter-process communication
    Ipc { socket_path: String },
}
```

### ServiceEndpoint

```rust
#[derive(Debug, Clone)]
pub struct ServiceEndpoint {
    /// Service identifier
    pub service_id: ServiceId,

    /// Service name
    pub name: String,

    /// Primary port
    pub port: u16,

    /// Secondary port (if any)
    pub secondary_port: Option<u16>,

    /// Wire protocol
    pub protocol: WireProtocol,

    /// Service tier (1-5)
    pub tier: u8,

    /// Connection weight for load balancing
    pub weight: f64,

    /// Health check endpoint
    pub health_endpoint: String,

    /// Current connection status
    pub status: ConnectionStatus,
}
```

### WireWeight

```rust
#[derive(Debug, Clone)]
pub struct WireWeight {
    /// Source service
    pub from: ServiceId,

    /// Target service
    pub to: ServiceId,

    /// Connection strength [0.0, 1.0]
    pub strength: f64,

    /// Message count
    pub messages: u64,

    /// Average latency in ms
    pub avg_latency_ms: f64,

    /// Error rate [0.0, 1.0]
    pub error_rate: f64,
}
```

---

## 12 ULTRAPLATE Services

| # | Service | Port | Protocol | Tier | Weight | Status |
|---|---------|------|----------|------|--------|--------|
| 1 | SYNTHEX | 8090/8091 | REST+WS | 1 | 1.5 | Active |
| 2 | SAN-K7 Orchestrator | 8100 | REST | 1 | 1.5 | Active |
| 3 | NAIS | 8101 | REST | 2 | 1.3 | Active |
| 4 | CodeSynthor V7 | 8110 | REST | 2 | 1.3 | Active |
| 5 | DevOps Engine | 8081 | REST | 2 | 1.3 | Active |
| 6 | Tool Library | 8105 | REST | 3 | 1.2 | Active |
| 7 | Library Agent | 8083 | REST | 3 | 1.2 | Stopped |
| 8 | CCM | 8104 | REST | 3 | 1.2 | Active |
| 9 | Prometheus Swarm | 10001+ | gRPC | 4 | 1.1 | Active |
| 10 | Architect Agent | 9001+ | REST | 4 | 1.1 | Active |
| 11 | Bash Engine | 8102 | IPC | 5 | 1.0 | Active |
| 12 | Tool Maker | 8103 | gRPC | 5 | 1.0 | Active |

---

## Service Bridges

### SYNTHEX Bridge (M19)

```rust
pub struct SynthexBridge {
    /// REST endpoint (8090)
    rest_endpoint: ServiceEndpoint,

    /// WebSocket endpoint (8091)
    ws_endpoint: ServiceEndpoint,

    /// Active WebSocket connection
    ws_connection: Option<WebSocketConnection>,
}

impl SynthexBridge {
    /// Check SYNTHEX health
    pub async fn health(&self) -> Result<HealthStatus>;

    /// Get SYNTHEX status
    pub async fn status(&self) -> Result<SynthexStatus>;

    /// Submit pattern for analysis
    pub async fn submit_pattern(&self, pattern: Pattern) -> Result<PatternId>;

    /// Subscribe to real-time events
    pub async fn subscribe(&mut self, topics: Vec<Topic>) -> Result<EventStream>;

    /// Query pattern results
    pub async fn query(&self, query: PatternQuery) -> Result<Vec<PatternResult>>;
}
```

**Endpoints:**
```
GET  /api/health              - Health check
GET  /api/status              - System status
POST /api/patterns            - Submit pattern
GET  /api/patterns/{id}       - Get pattern result
WS   /ws/events               - Real-time events
```

### SAN-K7 Bridge (M20)

```rust
pub struct SanK7Bridge {
    endpoint: ServiceEndpoint,
}

impl SanK7Bridge {
    /// Check SAN-K7 health
    pub async fn health(&self) -> Result<HealthStatus>;

    /// Get module status (M1-M55)
    pub async fn module_status(&self, module_id: &str) -> Result<ModuleStatus>;

    /// Get all healthy modules count
    pub async fn healthy_modules(&self) -> Result<u32>;

    /// Trigger orchestration action
    pub async fn orchestrate(&self, action: OrchestrationAction) -> Result<ActionResult>;
}
```

**Endpoints:**
```
GET  /health                  - Health check
GET  /modules                 - List all modules
GET  /modules/{id}/status     - Module status
POST /orchestrate             - Trigger action
```

### NAIS Bridge (M21)

```rust
pub struct NaisBridge {
    endpoint: ServiceEndpoint,
}

impl NaisBridge {
    /// Check NAIS health
    pub async fn health(&self) -> Result<HealthStatus>;

    /// Get intelligence analysis
    pub async fn analyze(&self, data: AnalysisRequest) -> Result<AnalysisResult>;

    /// Get adaptive recommendation
    pub async fn recommend(&self, context: Context) -> Result<Recommendation>;

    /// Report learning outcome
    pub async fn report_outcome(&self, outcome: LearningOutcome) -> Result<()>;
}
```

### Tool Library Bridge (M22)

```rust
pub struct ToolLibraryBridge {
    endpoint: ServiceEndpoint,
}

impl ToolLibraryBridge {
    /// List all tools
    pub async fn list_tools(&self) -> Result<Vec<Tool>>;

    /// Get tool details
    pub async fn get_tool(&self, tool_id: &str) -> Result<Tool>;

    /// Search tools by capability
    pub async fn search(&self, query: ToolQuery) -> Result<Vec<Tool>>;

    /// Execute tool
    pub async fn execute(&self, tool_id: &str, params: ToolParams) -> Result<ToolResult>;
}
```

**Endpoints:**
```
GET  /tools                   - List all tools
GET  /tools/{id}              - Get tool details
GET  /tools/search?q=...      - Search tools
POST /tools/{id}/execute      - Execute tool
```

### CodeSynthor Bridge (M23)

```rust
pub struct CodeSynthorBridge {
    endpoint: ServiceEndpoint,
}

impl CodeSynthorBridge {
    /// Check health
    pub async fn health(&self) -> Result<HealthStatus>;

    /// Get module status (M1-M62)
    pub async fn module_status(&self, module_id: &str) -> Result<ModuleStatus>;

    /// Generate code artifact
    pub async fn generate(&self, request: GenerateRequest) -> Result<GenerateResult>;

    /// Analyze code quality
    pub async fn analyze(&self, code: &str) -> Result<QualityReport>;
}
```

### DevOps Engine Bridge (M24)

```rust
pub struct DevOpsBridge {
    endpoint: ServiceEndpoint,
}

impl DevOpsBridge {
    /// Check health
    pub async fn health(&self) -> Result<HealthStatus>;

    /// Get Hebbian pulse state
    pub async fn hebbian_state(&self) -> Result<HebbianState>;

    /// Report pathway activation
    pub async fn report_activation(&self, pathway: PathwayActivation) -> Result<()>;

    /// Get system metrics
    pub async fn metrics(&self) -> Result<SystemMetrics>;
}
```

---

## Wire Protocol Manager

### Protocol API

```rust
pub struct WireProtocolManager {
    /// Register a service endpoint
    pub fn register(&mut self, endpoint: ServiceEndpoint) -> Result<()>;

    /// Deregister a service
    pub fn deregister(&mut self, service_id: &ServiceId) -> Result<()>;

    /// Get endpoint for service
    pub fn get_endpoint(&self, service_id: &ServiceId) -> Option<&ServiceEndpoint>;

    /// Get all endpoints by protocol
    pub fn by_protocol(&self, protocol: &WireProtocol) -> Vec<&ServiceEndpoint>;

    /// Check connectivity to service
    pub async fn ping(&self, service_id: &ServiceId) -> Result<Duration>;

    /// Get wire weights between services
    pub fn wire_weights(&self) -> Vec<&WireWeight>;
}
```

### Connection Pool

```rust
pub struct ConnectionPool {
    /// Maximum connections per service
    max_connections: usize,

    /// Connection timeout
    connect_timeout: Duration,

    /// Idle timeout
    idle_timeout: Duration,

    /// Active connections
    connections: HashMap<ServiceId, Vec<Connection>>,
}

impl ConnectionPool {
    /// Get connection to service
    pub async fn get(&self, service_id: &ServiceId) -> Result<Connection>;

    /// Return connection to pool
    pub fn release(&self, conn: Connection);

    /// Close all connections to service
    pub async fn close_all(&self, service_id: &ServiceId);
}
```

---

## Bridge Hub

### Hub API

```rust
pub struct BridgeHub {
    synthex: SynthexBridge,
    sank7: SanK7Bridge,
    nais: NaisBridge,
    tool_library: ToolLibraryBridge,
    codesynthor: CodeSynthorBridge,
    devops: DevOpsBridge,
    // ... other bridges
}

impl BridgeHub {
    /// Check all service health
    pub async fn health_check_all(&self) -> HashMap<ServiceId, HealthStatus>;

    /// Get service by ID
    pub fn get_bridge(&self, service_id: &ServiceId) -> Option<&dyn ServiceBridge>;

    /// Broadcast message to all services
    pub async fn broadcast(&self, message: BroadcastMessage) -> Vec<BroadcastResult>;

    /// Get aggregate synergy score
    pub fn synergy_score(&self) -> f64;
}
```

---

## 12D Tensor Integration

The Integration Layer encodes service state into 12D tensors for neural processing:

```rust
impl ServiceEndpoint {
    /// Convert endpoint state to 12D tensor
    pub fn to_tensor(&self) -> Tensor12D {
        Tensor12D {
            service_id: self.normalize_service_id(),
            port: self.port as f64 / 65535.0,
            tier: self.tier as f64 / 5.0,
            deps: (self.dependencies.len() as f64 + 1.0).ln(),
            agents: self.assigned_agents as f64 / 40.0,
            protocol: self.protocol.to_normalized(),
            health: self.health_score,
            uptime: self.uptime_ratio,
            synergy: self.synergy_score,
            latency: 1.0 - (self.avg_latency_ms / 2000.0).min(1.0),
            error_rate: self.error_rate,
            temporal: self.temporal_context(),
        }
    }
}
```

---

## Inter-Layer Communication

### Events from L3 (Core Logic)

```rust
pub enum L3InputEvent {
    RemediationStarted { action: RemediationAction },
    RemediationCompleted { result: RemediationResult },
    PipelineTriggered { pipeline_id: PipelineId },
}
```

### Events to L5 (Learning)

```rust
pub enum L4OutputEvent {
    ServiceStateChanged { service: ServiceId, tensor: Tensor12D },
    BridgeConnected { service: ServiceId, latency_ms: u64 },
    BridgeDisconnected { service: ServiceId, reason: String },
    SynergyUpdated { from: ServiceId, to: ServiceId, score: f64 },
}
```

---

## Metrics

| Metric | Type | Description |
|--------|------|-------------|
| `me_l4_bridge_requests_total` | Counter | Bridge requests by service |
| `me_l4_bridge_latency_ms` | Histogram | Request latency by service |
| `me_l4_bridge_errors_total` | Counter | Bridge errors by service |
| `me_l4_connections_active` | Gauge | Active connections per service |
| `me_l4_synergy_score` | Gauge | Cross-service synergy score |
| `me_l4_wire_weights` | Gauge | Wire weights between services |

---

## Configuration

```toml
[layer.L4]
enabled = true
startup_order = 4

[layer.L4.wire]
connect_timeout_ms = 5000
request_timeout_ms = 30000
max_retries = 3
retry_backoff_ms = 1000

[layer.L4.pool]
max_connections_per_service = 10
idle_timeout_ms = 60000
health_check_interval_ms = 30000

[layer.L4.bridges.synthex]
rest_url = "http://localhost:8090"
ws_url = "ws://localhost:8091"
enabled = true

[layer.L4.bridges.sank7]
url = "http://localhost:8100"
enabled = true

[layer.L4.bridges.nais]
url = "http://localhost:8101"
enabled = true

[layer.L4.bridges.tool_library]
url = "http://localhost:8105"
enabled = true

[layer.L4.bridges.codesynthor]
url = "http://localhost:8110"
enabled = true

[layer.L4.bridges.devops]
url = "http://localhost:8081"
enabled = true
```

---

## CLI Commands

```bash
# View all bridges status
./maintenance-engine bridge status

# Check specific service
./maintenance-engine bridge health --service synthex

# Ping service
./maintenance-engine bridge ping --service sank7

# View wire weights
./maintenance-engine bridge weights

# View synergy matrix
./maintenance-engine bridge synergy

# Test connectivity to all services
./maintenance-engine bridge test-all
```

---

## Navigation

| Direction | Link |
|-----------|------|
| Up | [INDEX.md](../INDEX.md) |
| Previous | [L03_CORE_LOGIC.md](L03_CORE_LOGIC.md) |
| Next | [L05_LEARNING.md](L05_LEARNING.md) |
| Service Registry | [SERVICE_REGISTRY.md](../../service_registry/SERVICE_REGISTRY.md) |
| API | [REST_API.md](../integration/REST_API.md) |

---

*[Back to Index](../INDEX.md) | [Previous: L03 Core Logic](L03_CORE_LOGIC.md) | [Next: L05 Learning](L05_LEARNING.md)*
