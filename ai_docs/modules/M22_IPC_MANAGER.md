# Module M22: IPC Manager

> **M22_IPC_MANAGER** | Inter-Process Communication | Layer: L4 Integration | [Back to Index](INDEX.md)

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

The IPC Manager module provides ultra-low-latency inter-process communication for the Maintenance Engine. It enables local process-to-process communication using Unix Domain Sockets, offering the fastest communication pathway for co-located services with minimal overhead and no network stack traversal.

### Module Properties

| Property | Value |
|----------|-------|
| Module ID | M22 |
| Module Name | IPC Manager |
| Layer | L4 (Integration) |
| Protocol | Unix Domain Sockets (IPC) |
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
- IPC: 1000 ms (fastest)

### ServiceEndpoint Structure

The ServiceEndpoint defines IPC service configuration:

```rust
pub struct ServiceEndpoint {
    /// Service ID (e.g., "bash-engine", "san-k7")
    pub service_id: String,
    /// Socket path (e.g., "/var/run/maintenance/bash-engine.sock")
    pub host: String,
    /// Ignored for IPC, can be 0
    pub port: u16,
    /// Protocol: WireProtocol::Ipc
    pub protocol: WireProtocol,
    /// Health check command/path
    pub health_path: String,
    /// Base path (typically empty for IPC)
    pub base_path: String,
    /// Request timeout in milliseconds (1000ms for IPC)
    pub timeout_ms: u64,
    /// Enable automatic retry on failure
    pub retry_enabled: bool,
    /// Maximum retry attempts
    pub max_retries: u32,
}
```

**Socket Path Convention:** `/var/run/maintenance/{service_id}.sock`

---

## 12 ULTRAPLATE Services

IPC is available for local process integration:

| Service | Socket Path | Protocol | Tier | Timeout | Use Case |
|---------|-------------|----------|------|---------|----------|
| SAN-K7 | /var/run/maintenance/san-k7.sock | REST/IPC | 1 | 1000ms | Orchestrator queries |
| Bash Engine | /var/run/maintenance/bash-engine.sock | REST/IPC | 5 | 1000ms | Fast command execution |
| Maintenance Engine | /var/run/maintenance/maintenance-engine.sock | Local | - | 0ms | Self-reference |

**Note:** IPC is fastest for local services but only works when services run on the same physical machine.

---

## Protocol Characteristics

### IPC Advantages

- **Lowest Latency:** 1000ms timeout (no network stack)
- **Zero Network Overhead:** Direct kernel communication
- **File Permissions:** File system security model
- **No Address Translation:** Direct process addressing
- **Default Timeout:** 1000ms (fastest)

### IPC Use Cases

1. **Local Orchestration:** SAN-K7 to Maintenance Engine
2. **Command Execution:** Quick bash command runs
3. **State Synchronization:** Fast local state updates
4. **Health Checks:** Immediate local health status
5. **Metrics Collection:** Real-time local metrics

---

## Communication Patterns

### Unix Domain Socket Connection

```
1. Client creates socket: /var/run/maintenance/service.sock
2. Connects to listening service
3. Sends request over socket
4. Receives response
5. Closes socket or maintains persistent connection
```

### Message Format

IPC messages can be:
- **Binary:** Raw bytes (fastest)
- **Text:** JSON or plain text
- **Structured:** Protocol Buffers or MessagePack

---

## API

### ServiceEndpoint Methods

#### Create IPC Endpoint

```rust
pub fn new(service_id: impl Into<String>,
           host: impl Into<String>,
           port: u16) -> Self
```

To configure as IPC:
- Set `protocol: WireProtocol::Ipc`
- Set `host` to socket path: `/var/run/maintenance/{service_id}.sock`
- Set `port: 0` (ignored for IPC)
- Set `timeout_ms: 1000`

#### Get URL

```rust
pub fn url(&self, path: &str) -> String
```

For IPC, returns socket path: `unix:///var/run/maintenance/service.sock{path}`

#### Get Health URL

```rust
pub fn health_url(&self) -> String
```

Returns health check path for IPC service.

---

## Wire Weight Matrix

IPC communication weights (local optimization):

```rust
pub struct WireWeight {
    /// Source service ID
    pub source: String,
    /// Target service ID (local services)
    pub target: String,
    /// Weight multiplier (high for IPC)
    pub weight: f64,
    /// Latency Service Level Objective in ms
    pub latency_slo_ms: u64,
    /// Error budget
    pub error_budget: f64,
}
```

**IPC Characteristics:**

| Dimension | Value | Notes |
|-----------|-------|-------|
| Weight | 2.0 | Highest priority (local only) |
| Latency SLO | 1ms | Sub-millisecond |
| Error Budget | 0.01% | Strict (local should be reliable) |
| Timeout | 1000ms | 1000x SLO margin |
| Retries | 3 | Immediate local retries |

---

## Configuration

### IPC Endpoint Configuration Example

```rust
ServiceEndpoint {
    service_id: "bash-engine".into(),
    host: "/var/run/maintenance/bash-engine.sock".into(),
    port: 0,
    protocol: WireProtocol::Ipc,
    health_path: "/health".into(),
    base_path: "".into(),
    timeout_ms: 1000,
    retry_enabled: true,
    max_retries: 3,
}
```

### Socket Directory Structure

```
/var/run/maintenance/
├── maintenance-engine.sock
├── bash-engine.sock
├── san-k7.sock
├── synthex.sock
└── codesynthor-v7.sock
```

### Permission Model

- **Owner:** Service user
- **Permissions:** 0o600 (owner read/write)
- **Security:** File system ACLs
- **Cleanup:** Automatic on service shutdown

---

## Advantages Over Network Protocols

### Performance Comparison

| Metric | IPC | gRPC | REST | WebSocket |
|--------|-----|------|------|-----------|
| Latency | <1ms | 3-5ms | 5-20ms | 10-50ms |
| Overhead | Minimal | Protocol headers | HTTP headers | Frame headers |
| Throughput | Limited by I/O | ~1000 req/s | ~100 req/s | Continuous |
| Connection | Stateless | Persistent | Per-request | Persistent |
| CPU Usage | Minimal | Low | Moderate | Moderate |

---

## Security Model

### File-Based Security

- **Ownership:** Process owner verification
- **Permissions:** Unix file permissions (0o600)
- **Isolation:** Only local processes
- **No Network:** Cannot be accessed remotely
- **Kernel Isolation:** Protected by OS security

### Best Practices

1. **Socket Cleanup:** Remove stale sockets on startup
2. **Permission Control:** Restrict to necessary processes
3. **Path Validation:** Prevent path traversal
4. **Error Handling:** Handle socket errors gracefully
5. **Resource Limits:** Prevent socket exhaustion

---

## Related Modules

| Module | Relationship |
|--------|--------------|
| [M19_REST_CLIENT](M19_REST_CLIENT.md) | Network-based alternative |
| [M20_GRPC_CLIENT](M20_GRPC_CLIENT.md) | gRPC alternative |
| [M21_WEBSOCKET_CLIENT](M21_WEBSOCKET_CLIENT.md) | Streaming alternative |
| [M24_BRIDGE_MANAGER](M24_BRIDGE_MANAGER.md) | Orchestrates IPC communication |
| [M07_HEALTH_MONITOR](M07_HEALTH_MONITOR.md) | Uses IPC for health checks |
| [M12_CIRCUIT_BREAKER](M12_CIRCUIT_BREAKER.md) | Protects IPC calls |

---

## Platform Compatibility

### Linux

- **Support:** Full support for Unix Domain Sockets
- **Path:** `/var/run/maintenance/` directory
- **Permissions:** POSIX permissions

### macOS

- **Support:** Full support for Unix Domain Sockets
- **Path:** `/var/run/maintenance/` (or `/tmp/`) directory
- **Permissions:** POSIX permissions

### Windows

- **Support:** Named Pipes (Windows IPC equivalent)
- **Path:** `\\.\pipe\maintenance\service_name`
- **Permissions:** Windows ACLs

---

## Performance Optimization

### Connection Pooling

- Keep socket open for multiple requests
- Reduce connection overhead
- Maintain socket state across calls

### Message Batching

- Group multiple operations in single request
- Reduce round-trip overhead
- Improve throughput

### Binary Protocols

- Use MessagePack or Protocol Buffers
- Reduce serialization overhead
- Faster deserialization

---

## Future Enhancements

- Connection pooling management
- Binary protocol support (MessagePack)
- Abstract sockets (Linux-specific optimization)
- Socket statistics and monitoring
- Hot-reload support for socket paths
- Performance profiling per socket

---

## Source Code

- **Location:** `/home/louranicas/claude-code-workspace/the_maintenance_engine/src/m4_integration/mod.rs`
- **Type:** Module (Rust)
- **Protocol:** Unix Domain Sockets (POSIX)
- **Status:** Stable

---

*The Maintenance Engine v1.0.0 | Module M22: IPC Manager*
*[Back to Index](INDEX.md)*
