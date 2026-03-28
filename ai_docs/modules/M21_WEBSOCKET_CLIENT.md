# Module M21: WebSocket Client

> **M21_WEBSOCKET_CLIENT** | Real-time Streaming Communication | Layer: L4 Integration | [Back to Index](INDEX.md)

---

## Navigation

| Direction | Link |
|-----------|------|
| Up | [INDEX.md](INDEX.md) |
| Layer | [L04_INTEGRATION.md](../layers/L04_INTEGRATION.md) |
| Related | [M19_REST_CLIENT.md](M19_REST_CLIENT.md) |
| Related | [M20_GRPC_CLIENT.md](M20_GRPC_CLIENT.md) |
| Related | [M24_BRIDGE_MANAGER.md](M24_BRIDGE_MANAGER.md) |
| L2 Services | [L02_SERVICES.md](../layers/L02_SERVICES.md) |

---

## Module Specification

### Overview

The WebSocket Client module provides real-time, full-duplex streaming communication for the Maintenance Engine. It enables bidirectional message exchange over persistent connections, supporting continuous event streams, metrics aggregation, and asynchronous notifications from ULTRAPLATE services.

### Module Properties

| Property | Value |
|----------|-------|
| Module ID | M21 |
| Module Name | WebSocket Client |
| Layer | L4 (Integration) |
| Protocol | WebSocket (WS/WSS) |
| Version | 1.0.0 |
| Dependencies | M07 (Health Monitor), M12 (Circuit Breaker) |
| Dependents | M24 (Bridge Manager), M23 (Event Bus), L3 (Core Logic) |

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
- WebSocket: 10000 ms (longest, for persistent connections)
- IPC: 1000 ms

### ServiceEndpoint Structure

The ServiceEndpoint defines WebSocket service configuration:

```rust
pub struct ServiceEndpoint {
    /// Service ID (e.g., "synthex", "codesynthor-v7")
    pub service_id: String,
    /// Host address (hostname or IP)
    pub host: String,
    /// Port number (WebSocket port, typically 8091 for SYNTHEX)
    pub port: u16,
    /// Protocol: WireProtocol::WebSocket
    pub protocol: WireProtocol,
    /// Health endpoint path
    pub health_path: String,
    /// Base path for WebSocket endpoints
    pub base_path: String,
    /// Request timeout in milliseconds (10000ms for WebSocket)
    pub timeout_ms: u64,
    /// Enable automatic reconnection
    pub retry_enabled: bool,
    /// Maximum retry attempts (connection attempts)
    pub max_retries: u32,
}
```

**URL Construction:** `ws://host:port/base_path/endpoint`

---

## 12 ULTRAPLATE Services

WebSocket is supported by streaming-capable services:

| Service | WS Port | HTTP Port | Protocol | Tier | Timeout | Notes |
|---------|---------|-----------|----------|------|---------|-------|
| SYNTHEX | 8091 | 8090 | REST/WS | 1 | 10000ms | Full streaming support |
| CodeSynthor V7 | 8110 | 8110 | REST/WS | 2 | 50000ms | Code streaming events |
| DevOps Engine | 8081 | 8081 | REST | 2 | 50000ms | REST preferred |

**Note:** SYNTHEX and CodeSynthor V7 support WebSocket for real-time event streaming. Most other services are REST-only.

---

## Protocol Characteristics

### WebSocket Advantages

- **Persistent Connection:** Eliminates connection overhead
- **Full-Duplex:** Simultaneous bidirectional communication
- **Low Latency:** No polling overhead
- **Default Timeout:** 10000ms (accommodates persistent connections)
- **Event Streaming:** Natural for continuous data flows

### WebSocket Use Cases

1. **Event Streaming:** Real-time event delivery
2. **Metrics Collection:** Continuous metric updates
3. **Log Streaming:** Live log tailing
4. **Bidirectional Commands:** Request-response with context persistence
5. **Long-Polling Alternative:** True push capability

---

## Communication Patterns

### WebSocket Connection Lifecycle

```
1. UPGRADE: HTTP GET with Upgrade header
   GET /api/events HTTP/1.1
   Upgrade: websocket
   Connection: Upgrade

2. HANDSHAKE: Server accepts upgrade
   HTTP/1.1 101 Switching Protocols

3. ACTIVE: Full-duplex frame exchange

4. CLOSE: Graceful connection termination
```

### Message Formats

**Text Frames (JSON):**
```json
{
  "type": "event",
  "service": "synthex",
  "event": "health_check",
  "timestamp": "2026-01-28T12:00:00Z",
  "data": {...}
}
```

**Binary Frames (MessagePack/Binary):**
- Efficient for high-frequency metrics
- Reduces bandwidth vs JSON

---

## API

### ServiceEndpoint Methods

#### Create WebSocket Endpoint

```rust
pub fn new(service_id: impl Into<String>,
           host: impl Into<String>,
           port: u16) -> Self
```

To configure as WebSocket:
- Set `protocol: WireProtocol::WebSocket`
- Set `timeout_ms: 10000`
- Set `base_path: "/api"` or custom path

#### Get URL

```rust
pub fn url(&self, path: &str) -> String
```

Constructs WebSocket URL: `ws://host:port/base_path/path`

#### Get Health URL

```rust
pub fn health_url(&self) -> String
```

Returns HTTP health check URL (not WebSocket).

---

## Wire Weight Matrix

WebSocket communication weights (streaming focus):

```rust
pub struct WireWeight {
    /// Source service ID
    pub source: String,
    /// Target service ID (typically "synthex")
    pub target: String,
    /// Weight multiplier for prioritization
    pub weight: f64,
    /// Latency Service Level Objective in ms
    pub latency_slo_ms: u64,
    /// Error budget (lower for streaming reliability)
    pub error_budget: f64,
}
```

**WebSocket Characteristics:**

| Dimension | Value | Notes |
|-----------|-------|-------|
| Weight | 1.5 | High priority for real-time |
| Latency SLO | 10ms | Per-frame latency |
| Error Budget | 0.1% | Strict for streaming |
| Timeout | 10000ms | Connection timeout |
| Retries | 3 | Reconnection attempts |

---

## Configuration

### WebSocket Endpoint Configuration Example

```rust
ServiceEndpoint {
    service_id: "synthex".into(),
    host: "localhost".into(),
    port: 8091,
    protocol: WireProtocol::WebSocket,
    health_path: "/api/health".into(),
    base_path: "/api".into(),
    timeout_ms: 10000,
    retry_enabled: true,
    max_retries: 3,
}
```

### Stream Configuration Options

- **Timeout:** 10000ms (accommodates persistent connections)
- **Retries:** 3 (connection reconnection attempts)
- **Health Check:** HTTP endpoint for connection validation
- **Message Size:** Configurable per stream
- **Compression:** Optional per-message deflate

---

## Event Types

### SYNTHEX Event Streams

Common event types from SYNTHEX WebSocket:

1. **Health Events:** Service health status changes
2. **Metrics Events:** Real-time metric updates
3. **Log Events:** Structured log entries
4. **Error Events:** Critical error notifications
5. **State Events:** Service state transitions

### CodeSynthor V7 Event Streams

1. **Analysis Events:** Code analysis results
2. **Compilation Events:** Build status updates
3. **Diagnostic Events:** Static analysis findings
4. **Progress Events:** Long-running task progress

---

## Related Modules

| Module | Relationship |
|--------|--------------|
| [M19_REST_CLIENT](M19_REST_CLIENT.md) | HTTP upgrade path to WebSocket |
| [M20_GRPC_CLIENT](M20_GRPC_CLIENT.md) | gRPC streaming alternative |
| [M22_IPC_MANAGER](M22_IPC_MANAGER.md) | Local streaming alternative |
| [M23_EVENT_BUS](M23_EVENT_BUS.md) | Internal event distribution |
| [M24_BRIDGE_MANAGER](M24_BRIDGE_MANAGER.md) | Orchestrates WebSocket connections |
| [M07_HEALTH_MONITOR](M07_HEALTH_MONITOR.md) | Connection health monitoring |
| [M12_CIRCUIT_BREAKER](M12_CIRCUIT_BREAKER.md) | Protects WebSocket streams |

---

## Performance Characteristics

### Comparison with REST

| Metric | WebSocket | REST |
|--------|-----------|------|
| Default Timeout | 10000ms | 5000ms |
| Connection Type | Persistent | Per-request |
| Streaming | Native | Polling |
| Overhead | Single handshake | Per-request headers |
| Latency | <10ms per frame | 50-100ms per request |
| Throughput | Continuous | Request-limited |

---

## Connection Management

### Reconnection Strategy

- **Exponential Backoff:** 1s, 2s, 4s, 8s delays
- **Max Backoff:** 60s
- **Jitter:** Random 0-100ms added
- **Max Retries:** 3 (configurable)

### Health Monitoring

- **Ping/Pong:** Keep-alive every 30s
- **Timeout Detection:** 10s without response
- **Auto-reconnect:** On timeout or close
- **Connection Pooling:** Multiple streams supported

---

## Security

### WebSocket Security Features

- **TLS/WSS:** Encryption support (future)
- **Authentication:** Bearer token in handshake
- **Message Validation:** Schema validation
- **Rate Limiting:** Per-stream rate limiting
- **Access Control:** Service-level ACLs

---

## Future Enhancements

- WSS (Secure WebSocket) with TLS
- Message compression (per-message-deflate)
- Stream multiplexing
- Automatic fallback to polling
- Client library SDKs
- Connection pooling optimization

---

## Source Code

- **Location:** `/home/louranicas/claude-code-workspace/the_maintenance_engine/src/m4_integration/mod.rs`
- **Type:** Module (Rust)
- **Protocol:** WebSocket (RFC 6455)
- **Status:** Stable

---

*The Maintenance Engine v1.0.0 | Module M21: WebSocket Client*
*[Back to Index](INDEX.md)*
