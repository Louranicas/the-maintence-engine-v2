# L4: Integration Layer Specification

> Target: ~7,500 LOC | 8 modules (M19-M24c) | 350+ tests

---

## Layer Purpose

The Integration layer provides communication protocols (REST, gRPC, WebSocket, IPC), event distribution, bridge management, peer-to-peer synergy tracking, and Tool Library registration. It connects ME V2 to the 13 ULTRAPLATE services.

---

## Module Specifications

### M19: REST Client (`rest.rs`)

**Purpose:** HTTP/REST communication with ULTRAPLATE services for health polling and API calls.

**Target:** ~600 LOC, 50+ tests

**Key Traits:**
```rust
pub trait RestClient: Send + Sync {
    fn get(&self, url: &str, timeout: Duration) -> Result<RestResponse>;
    fn post(&self, url: &str, body: &[u8], timeout: Duration) -> Result<RestResponse>;
    fn health_check(&self, service: &ServiceId) -> Result<HealthStatus>;
    fn batch_health(&self, services: &[ServiceId]) -> Result<Vec<(ServiceId, HealthStatus)>>;
}
```

**Tensor Contribution:** D5 (protocol = 0 for REST), D9 (latency)

**Integration:** Health polling for all services at tier-based intervals

---

### M20: gRPC Client (`grpc.rs`)

**Purpose:** Binary RPC communication for high-throughput service calls.

**Target:** ~1,200 LOC, 50+ tests

**Key Traits:**
```rust
pub trait GrpcClient: Send + Sync {
    fn call(&self, service: &ServiceId, method: &str, payload: &[u8], timeout: Duration) -> Result<Vec<u8>>;
    fn stream(&self, service: &ServiceId, method: &str) -> Result<GrpcStream>;
    fn connection_pool_status(&self) -> Result<PoolStatus>;
}
```

**Tensor Contribution:** D5 (protocol = 1 for gRPC)

---

### M21: WebSocket Client (`websocket.rs`)

**Purpose:** Real-time bidirectional communication for event streaming and field observation.

**Target:** ~1,000 LOC, 50+ tests

**Key Traits:**
```rust
pub trait WebSocketClient: Send + Sync {
    fn connect(&self, url: &str) -> Result<WsConnection>;
    fn subscribe(&self, conn: &WsConnection, topic: &str) -> Result<Subscription>;
    fn send(&self, conn: &WsConnection, message: &[u8]) -> Result<()>;
    fn field_stream(&self) -> Result<Subscription>; // SVF field evolution
}
```

**Tensor Contribution:** D5 (protocol = 2 for WebSocket)

**Integration:** Connects to SVF `/ws/field-evolution` for Kuramoto r-tracking (N01)

---

### M22: IPC Manager (`ipc.rs`)

**Purpose:** Local inter-process communication for co-located services.

**Target:** ~950 LOC, 50+ tests

**Key Traits:**
```rust
pub trait IpcManager: Send + Sync {
    fn connect(&self, path: &str) -> Result<IpcConnection>;
    fn send(&self, conn: &IpcConnection, message: &[u8]) -> Result<()>;
    fn receive(&self, conn: &IpcConnection, timeout: Duration) -> Result<Vec<u8>>;
}
```

**Tensor Contribution:** D5 (protocol = 3 for IPC)

---

### M23: Event Bus (`event_bus.rs`)

**Purpose:** Internal publish/subscribe event distribution with bounded channels.

**Target:** ~700 LOC, 50+ tests

**Key Types:**
```rust
pub struct EventBus { inner: RwLock<EventBusInner> }

pub struct EventSubscription {
    id: SubscriptionId,
    topic: EventTopic,
    filter: Option<EventFilter>,
}
```

**Key Traits:**
```rust
pub trait EventBusOps: Send + Sync {
    fn publish(&self, event: Event) -> Result<()>;
    fn subscribe(&self, topic: EventTopic) -> Result<EventSubscription>;
    fn unsubscribe(&self, id: &SubscriptionId) -> Result<()>;
    fn pending_count(&self) -> usize;
}
```

**Capacity:** Bounded channels (configurable, default 1024)

---

### M24: Bridge Manager (`bridge.rs`)

**Purpose:** Manage cross-service bridge connections and synergy tracking.

**Target:** ~750 LOC, 50+ tests

**Key Types:**
```rust
pub struct BridgeManager { inner: RwLock<BridgeManagerInner> }

pub struct Bridge {
    source: ServiceId,
    target: ServiceId,
    protocol: Protocol,
    status: BridgeStatus,
    synergy_score: f64,
    last_heartbeat: Timestamp,
}
```

**Tensor Contribution:** D8 (synergy score)

---

### M24b: Peer Bridge (`peer_bridge.rs`)

**Purpose:** Active peer-to-peer bridge communication with tiered health polling.

**Target:** ~600 LOC, 50+ tests

**Tier-based polling intervals:**
- Tier 1 (SYNTHEX, SAN-K7): 5s
- Tier 2 (NAIS, CodeSynthor, DevOps): 10s
- Tier 3 (Tool Library, CCM): 30s
- Tier 4+ (Prometheus, Architect): 60s

---

### M24c: Tool Registrar (`tool_registrar.rs`)

**Purpose:** Register ME V2 tools with the Tool Library service.

**Target:** ~1,200 LOC, 50+ tests

**15 Tool Definitions:**
- health_check, health_batch, service_status
- remediation_execute, remediation_rollback
- hebbian_query, stdp_status
- consensus_propose, consensus_status
- observer_report, emergence_detect
- tensor_encode, tensor_query
- evolution_evaluate, evolution_status

---

## Layer Coordinator (`mod.rs`)

**Target:** ~500 LOC, 20+ tests

**Provides:**
- `IntegrationLayer` aggregate struct
- Builder pattern with protocol clients
- `connect_all_services()` — batch connection establishment
- C11: All L4 operations wrapped with Nexus field capture
- C12: STDP co-activation on cross-service calls

---

## Design Constraints

- C1: Imports from L1, L2, L3 only
- C2: All trait methods `&self`
- C3: `TensorContributor` on M19-M24
- C4: Zero unsafe/unwrap/expect
- C11: Pre/post r-capture on all external calls
- C12: STDP increment (+0.05) on each service interaction

---

## Test Strategy

- Unit tests per module: 50+ each
- Integration: `tests/l4_integration_layer.rs`
- Mock services for protocol testing
- Property: synergy scores in [0.0, 1.0], bridge status FSM correctness
