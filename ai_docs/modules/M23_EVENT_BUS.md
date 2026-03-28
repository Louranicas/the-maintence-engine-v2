# Module M23: Event Bus

> **M23_EVENT_BUS** | Event Distribution System | Layer: L4 Integration | [Back to Index](INDEX.md)

---

## Navigation

| Direction | Link |
|-----------|------|
| Up | [INDEX.md](INDEX.md) |
| Layer | [L04_INTEGRATION.md](../layers/L04_INTEGRATION.md) |
| Related | [M21_WEBSOCKET_CLIENT.md](M21_WEBSOCKET_CLIENT.md) |
| Related | [M24_BRIDGE_MANAGER.md](M24_BRIDGE_MANAGER.md) |
| Related | [M23_EVENT_BUS.md](M23_EVENT_BUS.md) |
| L3 Core Logic | [L03_CORE_LOGIC.md](../layers/L03_CORE_LOGIC.md) |
| L5 Learning | [L05_LEARNING.md](../layers/L05_LEARNING.md) |

---

## Module Specification

### Overview

The Event Bus module provides distributed event distribution and pub-sub messaging for the Maintenance Engine. It enables loose coupling between services through a central event distribution system, supporting multiple event sources and subscribers with guaranteed delivery semantics.

### Module Properties

| Property | Value |
|----------|-------|
| Module ID | M23 |
| Module Name | Event Bus |
| Layer | L4 (Integration) |
| Pattern | Pub-Sub / Event Distribution |
| Version | 1.0.0 |
| Dependencies | M07 (Health Monitor), M12 (Circuit Breaker), M21 (WebSocket) |
| Dependents | M24 (Bridge Manager), L3 (Core Logic), L5 (Learning) |

---

## Core Concepts

### Event Bus Architecture

The Event Bus coordinates asynchronous communication across the Maintenance Engine:

```
┌─────────────────────────────────────────────────────────┐
│                      EVENT BUS                          │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  │
│  │  Publisher 1 │  │  Publisher 2 │  │  Publisher N │  │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘  │
│         │                 │                  │           │
│         └─────────────────┼──────────────────┘           │
│                           │                              │
│                    ┌──────▼──────┐                       │
│                    │ Event Queue  │                       │
│                    │ (Topic-based)│                       │
│                    └──────┬───────┘                       │
│                           │                              │
│         ┌─────────────────┼──────────────────┐           │
│         │                 │                  │           │
│    ┌────▼────┐      ┌────▼────┐      ┌────▼────┐        │
│    │Sub 1    │      │Sub 2    │      │Sub N    │        │
│    │(Topic A)│      │(Topic B)│      │(Topic C)│        │
│    └─────────┘      └─────────┘      └─────────┘        │
└─────────────────────────────────────────────────────────┘
```

---

## Event Types and Topics

### Core Event Topics

Events are organized by topic for selective subscription:

#### Health Events (Topic: `health.*`)

```rust
{
  "type": "health_check",
  "service": "synthex",
  "status": "healthy|degraded|unhealthy",
  "score": 0.95,
  "timestamp": "2026-01-28T12:00:00Z"
}
```

#### Service Events (Topic: `service.*`)

```rust
{
  "type": "service_started|service_stopped|service_restarted",
  "service": "san-k7",
  "details": {...},
  "timestamp": "2026-01-28T12:00:00Z"
}
```

#### Learning Events (Topic: `learning.*`)

```rust
{
  "type": "pathway_strengthened|pathway_weakened",
  "source": "synthex",
  "target": "codesynthor",
  "weight": 0.85,
  "timestamp": "2026-01-28T12:00:00Z"
}
```

#### Remediation Events (Topic: `remediation.*`)

```rust
{
  "type": "remediation_attempted|remediation_success|remediation_failure",
  "issue": "service_unhealthy",
  "service": "bash-engine",
  "action": "restart",
  "timestamp": "2026-01-28T12:00:00Z"
}
```

#### Consensus Events (Topic: `consensus.*`)

```rust
{
  "type": "vote_cast|vote_collected|consensus_reached",
  "round": 42,
  "decision": "restart_service",
  "voters": 27,
  "timestamp": "2026-01-28T12:00:00Z"
}
```

---

## Publisher-Subscriber Model

### Event Publishers

Any module can publish events to the bus:

1. **M07 Health Monitor** → Health events
2. **M12 Circuit Breaker** → Circuit state events
3. **M13 Remediation Engine** → Remediation attempt/result events
4. **M25 Hebbian Manager** → Learning pathway events
5. **M31 PBFT Manager** → Consensus events
6. **External Services** → Application events via WebSocket

### Event Subscribers

Modules subscribe to relevant topics:

| Subscriber | Topics | Purpose |
|------------|--------|---------|
| M13 (Remediation) | health.*, service.* | Detect issues for remediation |
| M14 (Escalation) | remediation.* | Track escalations |
| M25 (Learning) | service.*, remediation.* | Learn from actions |
| M31 (Consensus) | remediation.*, escalation.* | Vote on critical actions |
| M24 (Bridge Manager) | all | Distribute to external services |

---

## Event Distribution Guarantees

### Delivery Semantics

- **At-Least-Once:** Events guaranteed to reach subscribers
- **In-Order:** Events delivered in publication order per topic
- **Lossy on Failure:** Best-effort if subscribers unreachable
- **Queue Buffering:** Events buffered if subscribers temporarily unavailable

### Guarantees Implementation

```rust
pub struct Event {
    /// Unique event ID for deduplication
    pub event_id: String,
    /// Event type (e.g., "health_check", "service_started")
    pub event_type: String,
    /// Topic for filtering (e.g., "health.synthex")
    pub topic: String,
    /// Payload (JSON)
    pub payload: serde_json::Value,
    /// Publication timestamp
    pub timestamp: SystemTime,
    /// Source service
    pub source: String,
    /// Delivery attempts
    pub retry_count: u32,
    /// TTL in milliseconds
    pub ttl_ms: u64,
}
```

---

## API

### Publishing Events

```rust
/// Publish an event to the bus
pub fn publish(event: Event) -> Result<()>

/// Publish with specific topic
pub fn publish_to_topic(topic: &str, payload: serde_json::Value) -> Result<()>

/// Broadcast to all subscribers
pub fn broadcast(event: Event) -> Result<usize>  // Returns subscriber count
```

### Subscribing to Events

```rust
/// Subscribe to events matching pattern
pub fn subscribe(topic_pattern: &str) -> Result<EventReceiver>

/// Subscribe to multiple topics
pub fn subscribe_multi(topics: Vec<&str>) -> Result<EventReceiver>

/// One-time event consumption
pub fn wait_for(topic_pattern: &str, timeout_ms: u64) -> Result<Event>
```

### Topic Patterns

- `health.*` - All health events
- `health.synthex` - SYNTHEX health events only
- `service.*` - All service events
- `learning.pathway` - Pathway learning events
- `*` - All events (subscription sink)

---

## Wire Weight Matrix

Event distribution weights:

```rust
pub struct EventWeight {
    /// Topic pattern
    pub topic: String,
    /// Priority (higher = more important)
    pub priority: u8,
    /// Guaranteed delivery SLO
    pub slo_ms: u64,
    /// Acceptable loss rate
    pub loss_budget: f64,
}
```

**Default Event Weights:**

| Topic | Priority | SLO | Loss Budget | Use Case |
|-------|----------|-----|-------------|----------|
| health.* | 9 | 100ms | 0.1% | Critical monitoring |
| service.* | 8 | 500ms | 0.5% | Service lifecycle |
| remediation.* | 8 | 1000ms | 1.0% | Action tracking |
| learning.* | 5 | 5000ms | 5.0% | Async learning |
| consensus.* | 9 | 10000ms | 0.01% | Critical decisions |

---

## Configuration

### Event Bus Configuration Example

```toml
[event_bus]
# Queue capacity (events)
queue_size = 10000

# Default TTL for events (milliseconds)
default_ttl_ms = 60000

# Subscription timeout
subscription_timeout_ms = 5000

# Retry policy
max_retries = 3
retry_backoff_ms = 1000

# Monitoring
enable_metrics = true
event_sampling_rate = 0.1  # Sample 10% of events for metrics
```

---

## Integration Points

### From M21 (WebSocket)

WebSocket streams publish events to the bus:

```
SYNTHEX WebSocket → health events → Event Bus → L3 Modules
```

### To M24 (Bridge Manager)

Event Bus distributes events to external services:

```
Event Bus → M24 Bridge Manager → External Services (via REST/WS)
```

### Internal Subscribers

L3 modules subscribe to relevant events:

```
Event Bus → M13 (Remediation)
Event Bus → M14 (Escalation)
Event Bus → M25 (Learning)
Event Bus → M31 (Consensus)
```

---

## Related Modules

| Module | Relationship |
|--------|--------------|
| [M21_WEBSOCKET_CLIENT](M21_WEBSOCKET_CLIENT.md) | Event source |
| [M24_BRIDGE_MANAGER](M24_BRIDGE_MANAGER.md) | Event distributor |
| [M13_PIPELINE_MANAGER](../M13_PIPELINE_MANAGER.md) | Event consumer |
| [M25_HEBBIAN_MANAGER](../M25_HEBBIAN_MANAGER.md) | Learning event consumer |
| [M31_PBFT_MANAGER](../M31_PBFT_MANAGER.md) | Consensus event consumer |
| [M07_HEALTH_MONITOR](M07_HEALTH_MONITOR.md) | Event publisher |
| [M12_CIRCUIT_BREAKER](M12_CIRCUIT_BREAKER.md) | Event publisher |

---

## Performance Characteristics

### Throughput

- **Messages/Second:** 10,000+ events/sec per topic
- **Latency:** <100ms p99 for health events
- **Subscribers:** Scales to 100+ subscribers per topic
- **Memory:** ~1KB per event in queue

### Scalability

- **Queue Size:** Configurable (default 10,000)
- **Topics:** Unlimited dynamic topics
- **Subscribers:** Scales linearly per topic
- **Network:** Can bridge to multiple clusters

---

## Monitoring and Observability

### Event Bus Metrics

- **Events published:** Total events per topic
- **Events delivered:** Successful deliveries per topic
- **Events dropped:** Dropped due to queue full
- **Subscriber count:** Active subscribers per topic
- **Event latency:** P50, P95, P99 delivery latency
- **Queue depth:** Current queue size and peak

### Health Checks

```sql
SELECT
  topic,
  COUNT(*) as event_count,
  AVG(latency_ms) as avg_latency,
  COUNT(DISTINCT subscriber_id) as subscriber_count
FROM event_metrics
GROUP BY topic
ORDER BY event_count DESC;
```

---

## Security Considerations

### Access Control

- Topic-level ACLs
- Publisher authentication
- Subscriber verification
- Event payload validation

### Data Protection

- Events contain no secrets
- Sensitive data filtered before publication
- Event retention policies
- Audit logging of critical events

---

## Future Enhancements

- Persistent event log (event sourcing)
- Cross-cluster event federation
- Dead-letter queue for failed events
- Event replay capabilities
- Event schema registry
- Filtering and transformation rules
- Event analytics and aggregation

---

## Source Code

- **Location:** `/home/louranicas/claude-code-workspace/the_maintenance_engine/src/m4_integration/mod.rs`
- **Type:** Module (Rust)
- **Pattern:** Pub-Sub / Event Bus
- **Status:** Stable

---

*The Maintenance Engine v1.0.0 | Module M23: Event Bus*
*[Back to Index](INDEX.md)*
