# Module M24: Bridge Manager

> **M24_BRIDGE_MANAGER** | Service Bridge Coordination | Layer: L4 Integration | [Back to Index](INDEX.md)

---

## Navigation

| Direction | Link |
|-----------|------|
| Up | [INDEX.md](INDEX.md) |
| Layer | [L04_INTEGRATION.md](../layers/L04_INTEGRATION.md) |
| Related | [M19_REST_CLIENT.md](M19_REST_CLIENT.md) |
| Related | [M20_GRPC_CLIENT.md](M20_GRPC_CLIENT.md) |
| Related | [M21_WEBSOCKET_CLIENT.md](M21_WEBSOCKET_CLIENT.md) |
| Related | [M22_IPC_MANAGER.md](M22_IPC_MANAGER.md) |
| Related | [M23_EVENT_BUS.md](M23_EVENT_BUS.md) |
| L3 Core Logic | [L03_CORE_LOGIC.md](../layers/L03_CORE_LOGIC.md) |

---

## Module Specification

### Overview

The Bridge Manager module orchestrates communication between the Maintenance Engine and the 12 ULTRAPLATE services. It serves as the central coordinator for all external service integration, managing protocol selection, connection pooling, error handling, and circuit breaking for all service bridges.

### Module Properties

| Property | Value |
|----------|-------|
| Module ID | M24 |
| Module Name | Bridge Manager |
| Layer | L4 (Integration) |
| Role | Integration Orchestrator |
| Version | 1.0.0 |
| Dependencies | M19-M23 (all integration modules) |
| Dependents | M07 (Health Monitor), L3 (Core Logic), L5 (Learning) |

---

## Architecture Overview

### Service Bridge Topology

```
┌──────────────────────────────────────────────────────────────┐
│              MAINTENANCE ENGINE (M24 Bridge Manager)          │
│                                                               │
│  ┌─────────────────────────────────────────────────────────┐ │
│  │           BRIDGE COORDINATION LAYER                      │ │
│  │                                                           │ │
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  │ │
│  │  │ REST Client  │  │ gRPC Client  │  │WebSocket Cli │  │ │
│  │  │   (M19)      │  │   (M20)      │  │   (M21)      │  │ │
│  │  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘  │ │
│  │         │                 │                  │           │ │
│  │  ┌──────────────┐  ┌──────────────┐                     │ │
│  │  │ IPC Manager  │  │  Event Bus   │                     │ │
│  │  │   (M22)      │  │   (M23)      │                     │ │
│  │  └──────┬───────┘  └──────┬───────┘                     │ │
│  │         │                 │                              │ │
│  │         └─────────────────┼──────────────────────────┐   │ │
│  │                           │                          │   │ │
│  │                    ┌──────▼─────────┐               │   │ │
│  │                    │ BRIDGE MANAGER │               │   │ │
│  │                    │ ORCHESTRATOR   │               │   │ │
│  │                    └──────┬─────────┘               │   │ │
│  │                           │                         │   │ │
│  └───────────────────────────┼─────────────────────────┘   │ │
│                              │                              │ │
│  ┌───────────────────────────┼──────────────────────────┐   │ │
│  │                           │                          │   │ │
│  │  EXTERNAL ULTRAPLATE SERVICES                       │   │ │
│  │                                                      │   │ │
│  │  ┌──────────┐  ┌──────────┐  ┌──────────┐          │   │ │
│  │  │SYNTHEX   │  │  SAN-K7  │  │   NAIS   │  ...     │   │ │
│  │  │ (8090)   │  │  (8100)  │  │  (8101)  │          │   │ │
│  │  └──────────┘  └──────────┘  └──────────┘          │   │ │
│  │                                                      │   │ │
│  └──────────────────────────────────────────────────────┘   │ │
└──────────────────────────────────────────────────────────────┘
```

---

## Service Registry and Endpoints

### Default Endpoints Configuration

The Bridge Manager maintains a registry of all 12 ULTRAPLATE services:

```rust
pub fn default_endpoints() -> Vec<ServiceEndpoint>
```

**Tier 1: Core Services**

| Service | Port | Protocol | Timeout | Retries |
|---------|------|----------|---------|---------|
| SYNTHEX | 8090 | REST | 10000ms | 3 |
| SAN-K7 | 8100 | REST | 10000ms | 3 |

**Tier 2: Intelligence Services**

| Service | Port | Protocol | Timeout | Retries |
|---------|------|----------|---------|---------|
| NAIS | 8101 | REST | 50000ms | 3 |
| CodeSynthor V7 | 8110 | REST | 50000ms | 3 |
| DevOps Engine | 8081 | REST | 50000ms | 3 |

**Tier 3: Integration Services**

| Service | Port | Protocol | Timeout | Retries |
|---------|------|----------|---------|---------|
| Tool Library | 8105 | REST | 100000ms | 3 |
| Library Agent | 8083 | REST | 100000ms | 3 |
| CCM | 8104 | REST | 100000ms | 3 |

**Tier 5: Execution Services**

| Service | Port | Protocol | Timeout | Retries |
|---------|------|----------|---------|---------|
| Bash Engine | 8102 | REST | 500000ms | 2 |
| Tool Maker | 8103 | gRPC | 3000ms | 2 |

---

## Core Components

### Bridge Coordinator

Manages overall bridge orchestration:

```rust
pub struct BridgeManager {
    /// Service endpoint registry
    endpoints: HashMap<String, ServiceEndpoint>,

    /// Active connections cache
    connections: HashMap<String, ServiceConnection>,

    /// Wire weight matrix
    weights: Vec<WireWeight>,

    /// Health status cache
    health_cache: HashMap<String, HealthStatus>,

    /// Event bus for notifications
    event_bus: EventBus,
}
```

### Protocol Selector

Automatically selects optimal protocol per service:

```rust
pub fn select_protocol(service_id: &str) -> WireProtocol {
    match service_id {
        "synthex" | "san-k7" => WireProtocol::Rest,
        "tool-maker" => WireProtocol::Grpc,
        "bash-engine" => WireProtocol::Rest,
        _ => WireProtocol::Rest,
    }
}
```

### Connection Pool Manager

Maintains persistent connections:

- **REST:** Pooled HTTP clients per host
- **gRPC:** Channel pooling with load balancing
- **WebSocket:** Stream multiplexing
- **IPC:** Socket file descriptors

---

## Wire Weight Matrix

Determines routing priority and SLOs for all service pairs:

```rust
pub struct WireWeight {
    pub source: String,
    pub target: String,
    pub weight: f64,
    pub latency_slo_ms: u64,
    pub error_budget: f64,
}
```

**Weight Ranges:**

| Weight | Priority | Use Case |
|--------|----------|----------|
| 1.5 | High | Tier 1 services (critical) |
| 1.3 | High | Tier 2 services (important) |
| 1.2 | Medium | Tier 3 services (utility) |
| 1.1 | Medium | Tier 4 services (background) |
| 1.0 | Normal | Tier 5 services (execution) |

---

## Bridge Operations

### Health Check Orchestration

```rust
pub async fn check_all_health() -> HealthReport {
    // Parallel health checks for all services
    let results = futures::future::join_all(
        endpoints.iter().map(|ep| check_service_health(ep))
    ).await;

    aggregate_health_report(results)
}
```

**Health Check Details:**

| Service | Endpoint | Timeout | Frequency |
|---------|----------|---------|-----------|
| SYNTHEX | /api/health | 5000ms | Every 10s |
| SAN-K7 | /health | 5000ms | Every 10s |
| NAIS | /health | 5000ms | Every 30s |
| All Others | Service-specific | Varies | Per config |

### Service Discovery

Maintains dynamic service registry with health status:

```
Bridge Manager
├── ServiceRegistry
│   ├── service_id: String
│   ├── endpoint: ServiceEndpoint
│   ├── status: HealthStatus
│   ├── last_check: SystemTime
│   └── weight: f64
```

### Error Handling and Fallback

```rust
pub async fn call_service(
    service_id: &str,
    operation: &Operation
) -> Result<Response> {
    // 1. Select primary protocol
    let protocol = select_protocol(service_id);

    // 2. Route through appropriate client
    let result = match protocol {
        WireProtocol::Rest => rest_client.call(service_id, operation).await,
        WireProtocol::Grpc => grpc_client.call(service_id, operation).await,
        WireProtocol::WebSocket => ws_client.call(service_id, operation).await,
        WireProtocol::Ipc => ipc_manager.call(service_id, operation).await,
    };

    // 3. Handle failures with circuit breaker
    match result {
        Ok(response) => {
            circuit_breaker.record_success(service_id);
            Ok(response)
        }
        Err(e) => {
            circuit_breaker.record_failure(service_id);
            self.handle_service_error(service_id, e).await
        }
    }
}
```

---

## Circuit Breaker Integration

Bridge Manager integrates with M12 Circuit Breaker:

```
Service Call
    │
    ├─► Check Circuit State
    │   ├─► CLOSED: Allow call (M19-M22)
    │   ├─► OPEN: Fast fail (no call)
    │   └─► HALF_OPEN: Allow probe call
    │
    └─► Record Result
        ├─► Success: Circuit may close
        └─► Failure: Circuit may open
```

**Circuit Breaker Configuration Per Service:**

| Service | Failure Threshold | Recovery Time | Probe Calls |
|---------|------------------|---------------|-------------|
| SYNTHEX | 5 failures | 60s | 3 |
| SAN-K7 | 5 failures | 60s | 3 |
| Bash Engine | 3 failures | 30s | 2 |
| Tool Maker | 3 failures | 30s | 2 |

---

## Event Bus Integration

Bridge Manager publishes events for all operations:

### Published Events

```
Event Topic: bridge.call_initiated
{
  "service": "synthex",
  "operation": "health_check",
  "timestamp": "2026-01-28T12:00:00Z"
}

Event Topic: bridge.call_success
{
  "service": "synthex",
  "operation": "health_check",
  "duration_ms": 45,
  "timestamp": "2026-01-28T12:00:00Z"
}

Event Topic: bridge.call_failed
{
  "service": "bash-engine",
  "operation": "execute",
  "error": "Timeout after 500000ms",
  "timestamp": "2026-01-28T12:00:00Z"
}
```

---

## API

### Service Health Check

```rust
pub async fn get_service_health(&self, service_id: &str) -> Result<HealthStatus>

pub async fn get_all_health(&self) -> Result<HashMap<String, HealthStatus>>

pub fn get_cached_health(&self, service_id: &str) -> Option<HealthStatus>
```

### Service Information

```rust
pub fn get_endpoint(&self, service_id: &str) -> Option<&ServiceEndpoint>

pub fn get_endpoints(&self) -> Vec<&ServiceEndpoint>

pub fn get_weight(&self, source: &str, target: &str) -> Option<f64>
```

### Bridge Operations

```rust
pub async fn call_service<T: Serialize>(
    &self,
    service_id: &str,
    path: &str,
    payload: &T
) -> Result<serde_json::Value>

pub async fn stream_service(
    &self,
    service_id: &str,
    path: &str
) -> Result<EventStream>
```

---

## Configuration

### Bridge Manager Configuration

```toml
[bridge_manager]
# Service health check interval (milliseconds)
health_check_interval_ms = 10000

# Connection pool sizes
rest_pool_size = 50
grpc_channel_pool_size = 10

# Cache settings
health_cache_ttl_ms = 5000
endpoint_cache_ttl_ms = 60000

# Retry policy
max_retries = 3
retry_backoff_ms = 1000

# Monitoring
enable_metrics = true
event_publishing = true
```

---

## Monitoring and Observability

### Metrics Collected

- **Call count** per service
- **Success rate** per service
- **Average latency** per service
- **Circuit breaker state** per service
- **Connection pool usage** per protocol
- **Event publication rate**

### Health Dashboard

```
Service Health Summary:
├── SYNTHEX (8090)     ✓ HEALTHY  (99.8% uptime)
├── SAN-K7 (8100)      ✓ HEALTHY  (99.7% uptime)
├── NAIS (8101)        ✓ HEALTHY  (98.5% uptime)
├── CodeSynthor (8110) ✓ HEALTHY  (99.2% uptime)
├── DevOps (8081)      ⚠ DEGRADED (95.3% uptime)
├── Tool Library (8105) ✓ HEALTHY  (99.1% uptime)
├── Bash Engine (8102) ✓ HEALTHY  (98.9% uptime)
└── Tool Maker (8103)  ✓ HEALTHY  (99.4% uptime)
```

---

## Related Modules

| Module | Relationship |
|--------|--------------|
| [M19_REST_CLIENT](M19_REST_CLIENT.md) | HTTP protocol layer |
| [M20_GRPC_CLIENT](M20_GRPC_CLIENT.md) | gRPC protocol layer |
| [M21_WEBSOCKET_CLIENT](M21_WEBSOCKET_CLIENT.md) | WebSocket protocol layer |
| [M22_IPC_MANAGER](M22_IPC_MANAGER.md) | IPC protocol layer |
| [M23_EVENT_BUS](M23_EVENT_BUS.md) | Event distribution |
| [M07_HEALTH_MONITOR](M07_HEALTH_MONITOR.md) | Health integration |
| [M12_CIRCUIT_BREAKER](M12_CIRCUIT_BREAKER.md) | Failure protection |

---

## Load Balancing Strategy

Bridge Manager distributes load across service instances:

### Protocol-Specific Strategies

**REST:** Round-robin load balancing
```
Request 1 → Instance 1
Request 2 → Instance 2
Request 3 → Instance 1
```

**gRPC:** Connection pooling with automatic multiplexing
```
Multiple requests → Shared gRPC channel → Multiplexed
```

**WebSocket:** Stream affinity
```
Subscription 1 → Stream A
Subscription 2 → Stream B
```

---

## Security Considerations

### Authentication

- **Service Tokens:** 24-hour expiry
- **Mutual TLS:** For sensitive operations
- **Bearer Tokens:** In HTTP headers

### Rate Limiting

Per service rate limits to prevent overwhelming backends:

| Tier | Rate | Burst |
|------|------|-------|
| 1 | 1000 req/min | 2000 |
| 2 | 800 req/min | 1600 |
| 3 | 600 req/min | 1200 |
| 4 | 400 req/min | 800 |
| 5 | 200 req/min | 400 |

---

## Future Enhancements

- Multi-region bridge federation
- Service mesh integration (Istio/Linkerd)
- Advanced load balancing (canary, weighted)
- Request tracing (OpenTelemetry)
- Custom protocol adapters
- Service discovery integration (Consul, Eureka)
- Automatic failover to backup regions

---

## Source Code

- **Location:** `/home/louranicas/claude-code-workspace/the_maintenance_engine/src/m4_integration/mod.rs`
- **Type:** Module (Rust)
- **Role:** Integration Orchestrator
- **Status:** Stable

---

*The Maintenance Engine v1.0.0 | Module M24: Bridge Manager*
*[Back to Index](INDEX.md)*
