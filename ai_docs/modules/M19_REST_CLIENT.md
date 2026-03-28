# Module M19: REST Client

> **M19_REST_CLIENT** | HTTP/REST Communication | Layer: L4 Integration | [Back to Index](INDEX.md)

---

## Navigation

| Direction | Link |
|-----------|------|
| Up | [INDEX.md](INDEX.md) |
| Layer | [L04_INTEGRATION.md](../layers/L04_INTEGRATION.md) |
| Related | [M20_GRPC_CLIENT.md](M20_GRPC_CLIENT.md) |
| Related | [M21_WEBSOCKET_CLIENT.md](M21_WEBSOCKET_CLIENT.md) |
| Related | [M24_BRIDGE_MANAGER.md](M24_BRIDGE_MANAGER.md) |
| L2 Services | [L02_SERVICES.md](../layers/L02_SERVICES.md) |

---

## Module Specification

### Overview

The REST Client module provides HTTP/REST communication capabilities for the Maintenance Engine. It enables synchronous request-response communication with external ULTRAPLATE services using the REST protocol with configurable timeouts, retries, and health checking.

### Module Properties

| Property | Value |
|----------|-------|
| Module ID | M19 |
| Module Name | REST Client |
| Layer | L4 (Integration) |
| Protocol | HTTP/REST |
| Version | 1.0.0 |
| Dependencies | M07 (Health Monitor), M12 (Circuit Breaker) |
| Dependents | M24 (Bridge Manager), L3 (Core Logic) |

---

## Core Types

### WireProtocol Enumeration

```rust
pub enum WireProtocol {
    /// REST/HTTP
    Rest,
    /// gRPC
    Grpc,
    /// WebSocket
    WebSocket,
    /// Unix Domain Socket
    Ipc,
}
```

**Default Timeouts by Protocol:**
- REST: 5000 ms
- gRPC: 3000 ms
- WebSocket: 10000 ms
- IPC: 1000 ms

### ServiceEndpoint Structure

The ServiceEndpoint defines a complete service configuration for communication:

```rust
pub struct ServiceEndpoint {
    /// Service ID (e.g., "synthex", "san-k7")
    pub service_id: String,
    /// Host address (hostname or IP)
    pub host: String,
    /// Port number (8080-10001)
    pub port: u16,
    /// Protocol (REST, gRPC, WebSocket, IPC)
    pub protocol: WireProtocol,
    /// Health endpoint path (default: "/api/health")
    pub health_path: String,
    /// Base path for API (default: "/api")
    pub base_path: String,
    /// Request timeout in milliseconds
    pub timeout_ms: u64,
    /// Enable automatic retry on failure
    pub retry_enabled: bool,
    /// Maximum retry attempts
    pub max_retries: u32,
}
```

**URL Construction:**
- Full URL: `http://host:port/base_path/endpoint`
- Health URL: `http://host:port/health_path`

---

## 12 ULTRAPLATE Services

REST is the primary protocol for Tier 1-3 services:

| Service | Port | Protocol | Tier | Timeout | Retries |
|---------|------|----------|------|---------|---------|
| SYNTHEX | 8090 | REST | 1 | 10000ms | 3 |
| SAN-K7 | 8100 | REST | 1 | 10000ms | 3 |
| NAIS | 8101 | REST | 2 | 50000ms | 3 |
| CodeSynthor V7 | 8110 | REST | 2 | 50000ms | 3 |
| DevOps Engine | 8081 | REST | 2 | 50000ms | 3 |
| Tool Library | 8105 | REST | 3 | 100000ms | 3 |
| Library Agent | 8083 | REST | 3 | 100000ms | 3 |
| CCM | 8104 | REST | 3 | 100000ms | 3 |
| Bash Engine | 8102 | REST | 5 | 500000ms | 2 |
| Tool Maker | 8103 | REST | 5 | 500000ms | 2 |

---

## API

### ServiceEndpoint Methods

#### Create New Endpoint

```rust
pub fn new(service_id: impl Into<String>,
           host: impl Into<String>,
           port: u16) -> Self
```

Creates a default REST endpoint with:
- Protocol: REST
- Health path: `/api/health`
- Base path: `/api`
- Timeout: 5000 ms
- Retries: 3 (enabled)

#### Get Full URL

```rust
pub fn url(&self, path: &str) -> String
```

Constructs full URL: `http://host:port/base_path/path`

#### Get Health Check URL

```rust
pub fn health_url(&self) -> String
```

Constructs health URL: `http://host:port/health_path`

### Default Endpoints

```rust
pub fn default_endpoints() -> Vec<ServiceEndpoint>
```

Returns pre-configured endpoints for all 10 ULTRAPLATE services.

---

## Wire Weight Matrix

Service-to-service communication weights define request importance and latency SLOs:

```rust
pub struct WireWeight {
    /// Source service ID
    pub source: String,
    /// Target service ID
    pub target: String,
    /// Weight multiplier for prioritization (0.0-2.0)
    pub weight: f64,
    /// Latency Service Level Objective in ms
    pub latency_slo_ms: u64,
    /// Error budget (fraction of requests allowed to fail)
    pub error_budget: f64,
}
```

**Default Wire Weights (Maintenance Engine perspective):**

| Source | Target | Weight | Latency SLO | Error Budget |
|--------|--------|--------|-------------|--------------|
| maintenance-engine | synthex | 1.5 | 10ms | 0.1% |
| maintenance-engine | san-k7 | 1.5 | 10ms | 0.1% |
| maintenance-engine | nais | 1.3 | 50ms | 0.5% |
| maintenance-engine | codesynthor-v7 | 1.3 | 50ms | 0.5% |
| maintenance-engine | devops-engine | 1.3 | 50ms | 0.5% |
| maintenance-engine | tool-library | 1.2 | 100ms | 1.0% |
| maintenance-engine | ccm | 1.2 | 100ms | 1.0% |
| maintenance-engine | library-agent | 1.2 | 100ms | 1.0% |
| maintenance-engine | bash-engine | 1.0 | 500ms | 2.0% |
| maintenance-engine | tool-maker | 1.0 | 500ms | 2.0% |

---

## Communication Patterns

### Health Check Pattern

Standard health endpoint that all services expose:

```
GET /api/health
Response: {"status":"healthy","timestamp":"..."}
```

### Status Endpoint

Service status information:

```
GET /api/status
Response: {"status":"operational","uptime_seconds":N,"version":"X.X.X"}
```

### API Path Construction

All REST endpoints follow RESTful conventions:
- Base: `/api`
- Resources: `/api/resource`
- Sub-resources: `/api/resource/{id}/sub`
- Health checks: Separate `/health` or `/api/health` path

---

## Configuration

### Endpoint Configuration Example

```rust
ServiceEndpoint {
    service_id: "synthex".into(),
    host: "localhost".into(),
    port: 8090,
    protocol: WireProtocol::Rest,
    health_path: "/api/health".into(),
    base_path: "/api".into(),
    timeout_ms: 10000,
    retry_enabled: true,
    max_retries: 3,
}
```

### Customization Options

Endpoints can be customized per service:
- Different timeouts for slow services (Tool Library: 100s)
- Different retry counts for execution services (Bash Engine: 2 retries)
- Custom health paths per service
- Custom base paths (SAN-K7: empty base path)

---

## Related Modules

| Module | Relationship |
|--------|--------------|
| [M20_GRPC_CLIENT](M20_GRPC_CLIENT.md) | Alternative protocol (binary) |
| [M21_WEBSOCKET_CLIENT](M21_WEBSOCKET_CLIENT.md) | Streaming upgrade path |
| [M22_IPC_MANAGER](M22_IPC_MANAGER.md) | Local IPC alternative |
| [M24_BRIDGE_MANAGER](M24_BRIDGE_MANAGER.md) | Orchestrates REST communication |
| [M07_HEALTH_MONITOR](M07_HEALTH_MONITOR.md) | Uses health endpoints |
| [M12_CIRCUIT_BREAKER](M12_CIRCUIT_BREAKER.md) | Protects REST calls |

---

## Testing

### Unit Tests

```rust
#[test]
fn test_endpoint_url() {
    let endpoint = ServiceEndpoint::new("test", "localhost", 8080);
    assert_eq!(endpoint.url("/status"), "http://localhost:8080/api/status");
    assert_eq!(endpoint.health_url(), "http://localhost:8080/api/health");
}

#[test]
fn test_default_endpoints() {
    let endpoints = default_endpoints();
    assert!(endpoints.len() >= 10);
    assert!(endpoints.iter().any(|e| e.service_id == "synthex"));
}
```

---

## Source Code

- **Location:** `/home/louranicas/claude-code-workspace/the_maintenance_engine/src/m4_integration/mod.rs`
- **Type:** Module (Rust)
- **Status:** Stable

---

*The Maintenance Engine v1.0.0 | Module M19: REST Client*
*[Back to Index](INDEX.md)*
