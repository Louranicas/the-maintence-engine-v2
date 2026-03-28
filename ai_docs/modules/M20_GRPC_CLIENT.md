# Module M20: gRPC Client

> **M20_GRPC_CLIENT** | gRPC Communication | Layer: L4 Integration | [Back to Index](INDEX.md)

---

## Navigation

| Direction | Link |
|-----------|------|
| Up | [INDEX.md](INDEX.md) |
| Layer | [L04_INTEGRATION.md](../layers/L04_INTEGRATION.md) |
| Related | [M19_REST_CLIENT.md](M19_REST_CLIENT.md) |
| Related | [M21_WEBSOCKET_CLIENT.md](M21_WEBSOCKET_CLIENT.md) |
| Related | [M24_BRIDGE_MANAGER.md](M24_BRIDGE_MANAGER.md) |
| L2 Services | [L02_SERVICES.md](../layers/L02_SERVICES.md) |

---

## Module Specification

### Overview

The gRPC Client module provides high-performance binary RPC communication for the Maintenance Engine. It enables low-latency, bidirectional streaming communication with services supporting the gRPC protocol using Protocol Buffers for serialization.

### Module Properties

| Property | Value |
|----------|-------|
| Module ID | M20 |
| Module Name | gRPC Client |
| Layer | L4 (Integration) |
| Protocol | gRPC (HTTP/2) |
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
- gRPC: 3000 ms (fastest)
- WebSocket: 10000 ms
- IPC: 1000 ms

### ServiceEndpoint Structure

The ServiceEndpoint defines gRPC service configuration:

```rust
pub struct ServiceEndpoint {
    /// Service ID (e.g., "tool-maker")
    pub service_id: String,
    /// Host address (hostname or IP)
    pub host: String,
    /// Port number (gRPC services use specific ports)
    pub port: u16,
    /// Protocol: WireProtocol::Grpc
    pub protocol: WireProtocol,
    /// Health endpoint path (gRPC health check service)
    pub health_path: String,
    /// Base path for gRPC service (typically empty)
    pub base_path: String,
    /// Request timeout in milliseconds (typically 3000ms for gRPC)
    pub timeout_ms: u64,
    /// Enable automatic retry on failure
    pub retry_enabled: bool,
    /// Maximum retry attempts
    pub max_retries: u32,
}
```

---

## 12 ULTRAPLATE Services

gRPC is supported by execution tier services:

| Service | Port | Protocol | Tier | Timeout | Retries | Notes |
|---------|------|----------|------|---------|---------|-------|
| Tool Maker | 8103 | gRPC | 5 | 3000ms | 2 | Primary gRPC consumer |
| Bash Engine | 8102 | REST/IPC | 5 | 500000ms | 2 | REST preferred |
| CodeSynthor V7 | 8110 | REST | 2 | 50000ms | 3 | REST preferred |

**Note:** Most ULTRAPLATE services prefer REST for compatibility. gRPC is available for specialized high-performance scenarios.

---

## Protocol Characteristics

### gRPC Advantages

- **Low Latency:** Binary Protocol Buffers vs JSON serialization
- **Bidirectional Streaming:** Full duplex communication support
- **HTTP/2:** Multiplexing and connection reuse
- **Type Safety:** Schema-driven interface definition
- **Default Timeout:** 3000ms (fastest)

### gRPC Considerations

- Requires service implementation with Protocol Buffers
- Not all ULTRAPLATE services support gRPC
- Tool Maker (port 8103) is the primary gRPC consumer
- Client library dependencies required

---

## Communication Patterns

### gRPC Service Health Check

Standard gRPC Health Checking Protocol:

```
Service: grpc.health.v1.Health
Method: Check
Request: {service: "service_name"}
Response: {status: SERVING|NOT_SERVING}
```

### Streaming Patterns

gRPC supports multiple communication patterns:

1. **Unary RPC:** Single request-response
2. **Server Streaming:** Single request, stream responses
3. **Client Streaming:** Stream requests, single response
4. **Bidirectional Streaming:** Stream both directions

---

## API

### ServiceEndpoint Methods

#### Create gRPC Endpoint

```rust
pub fn new(service_id: impl Into<String>,
           host: impl Into<String>,
           port: u16) -> Self
```

Creates endpoint with:
- Protocol: Must be set to WireProtocol::Grpc
- Timeout: 3000 ms (recommended for gRPC)
- Retries: 2-3 (configurable)

#### Get URL

```rust
pub fn url(&self, path: &str) -> String
```

For gRPC, constructs: `http://host:port/service_name`

#### Get Health URL

```rust
pub fn health_url(&self) -> String
```

Accesses gRPC health check endpoint.

---

## Wire Weight Matrix

gRPC communication weights (Tool Maker focus):

```rust
pub struct WireWeight {
    /// Source service ID
    pub source: String,
    /// Target service ID (typically "tool-maker")
    pub target: String,
    /// Weight multiplier for prioritization
    pub weight: f64,
    /// Latency Service Level Objective in ms
    pub latency_slo_ms: u64,
    /// Error budget
    pub error_budget: f64,
}
```

**Tool Maker gRPC Characteristics:**

| Dimension | Value | Notes |
|-----------|-------|-------|
| Weight | 1.0 | Standard priority |
| Latency SLO | 500ms | Execution service |
| Error Budget | 2.0% | 2% acceptable failures |
| Timeout | 3000ms | 6x SLO margin |
| Retries | 2 | Limited retry budget |

---

## Configuration

### gRPC Endpoint Configuration Example

```rust
ServiceEndpoint {
    service_id: "tool-maker".into(),
    host: "localhost".into(),
    port: 8103,
    protocol: WireProtocol::Grpc,
    health_path: "/grpc.health.v1.Health/Check".into(),
    base_path: "".into(),
    timeout_ms: 3000,
    retry_enabled: true,
    max_retries: 2,
}
```

### Customization Options

- **Timeout:** Default 3000ms, adjustable per operation
- **Retries:** 0-3 range, typically 2 for execution services
- **Health Check:** Uses gRPC Health Checking Protocol
- **TLS:** Can be configured per endpoint (future enhancement)

---

## Related Modules

| Module | Relationship |
|--------|--------------|
| [M19_REST_CLIENT](M19_REST_CLIENT.md) | Alternative protocol (JSON) |
| [M21_WEBSOCKET_CLIENT](M21_WEBSOCKET_CLIENT.md) | Streaming via WebSocket |
| [M22_IPC_MANAGER](M22_IPC_MANAGER.md) | Local IPC alternative |
| [M24_BRIDGE_MANAGER](M24_BRIDGE_MANAGER.md) | Orchestrates gRPC communication |
| [M07_HEALTH_MONITOR](M07_HEALTH_MONITOR.md) | Health check integration |
| [M12_CIRCUIT_BREAKER](M12_CIRCUIT_BREAKER.md) | Protects gRPC calls |

---

## Performance Characteristics

### Comparison with REST

| Metric | gRPC | REST |
|--------|------|------|
| Default Timeout | 3000ms | 5000ms |
| Serialization | Protocol Buffers | JSON |
| Streaming | Native bidirectional | Polling/WebSocket |
| Multiplexing | HTTP/2 native | Not native |
| Payload Size | ~1-5 KB | ~5-20 KB |

---

## Future Enhancements

- TLS/SSL mutual authentication
- Streaming metric collection
- Server push capabilities
- Connection pooling and management
- Load balancing across multiple instances

---

## Source Code

- **Location:** `/home/louranicas/claude-code-workspace/the_maintenance_engine/src/m4_integration/mod.rs`
- **Type:** Module (Rust)
- **Protocol:** gRPC with HTTP/2
- **Status:** Stable

---

*The Maintenance Engine v1.0.0 | Module M20: gRPC Client*
*[Back to Index](INDEX.md)*
