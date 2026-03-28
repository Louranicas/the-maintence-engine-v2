# Layer 2: Services

> **L02_SERVICES** | Service Integration Layer | [Back to Index](../INDEX.md)

---

## Navigation

| Direction | Link |
|-----------|------|
| Up | [INDEX.md](../INDEX.md) |
| Previous | [L01_FOUNDATION.md](L01_FOUNDATION.md) |
| Next | [L03_CORE_LOGIC.md](L03_CORE_LOGIC.md) |
| Down (API) | [REST_API.md](../integration/REST_API.md) |

---

## Layer Overview

The Services Layer (L2) provides health monitoring, service discovery, and service mesh integration. It acts as the nervous system of the engine, continuously monitoring the health of all components and managing service-to-service communication.

### Layer Properties

| Property | Value |
|----------|-------|
| Layer ID | L2 |
| Layer Name | Services |
| Dependencies | L1 (Foundation) |
| Dependents | L3-L6 |
| Primary Functions | Health Monitoring, Service Discovery |
| Protocol Support | gRPC, HTTP/2, TCP |

---

## Architecture

```
+------------------------------------------------------------------+
|                      L2: Services Layer                           |
+------------------------------------------------------------------+
|                                                                  |
|  +---------------------------+  +---------------------------+    |
|  |     Health Monitor        |  |    Service Discovery      |    |
|  |                           |  |                           |    |
|  |  - Active probes          |  |  - Service registry       |    |
|  |  - Passive observation    |  |  - DNS resolution         |    |
|  |  - Dependency tracking    |  |  - Load balancing         |    |
|  |  - Anomaly detection      |  |  - Circuit breaking       |    |
|  +-------------+-------------+  +-------------+-------------+    |
|                |                              |                  |
|                +------------+-----------------+                  |
|                             |                                    |
|  +---------------------------+---------------------------+       |
|  |              Service Mesh Controller                  |       |
|  |                                                       |       |
|  |  - Traffic management    - mTLS enforcement           |       |
|  |  - Rate limiting         - Retry policies             |       |
|  |  - Timeout management    - Observability              |       |
|  +-------------------------------------------------------+       |
|                                                                  |
+------------------------------------------------------------------+
        |                    |                    |
        v                    v                    v
   [L1 Foundation]    [Internal Services]   [External Services]
```

---

## Health Monitoring

### Health Check Types

| Type | Interval | Timeout | Description |
|------|----------|---------|-------------|
| Liveness | 10s | 5s | Is the service running? |
| Readiness | 5s | 3s | Is the service ready for traffic? |
| Startup | 1s | 30s | Has the service started successfully? |
| Deep | 60s | 10s | Full dependency health check |

### Health States

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum HealthState {
    Healthy,           // All checks passing
    Degraded,          // Some checks failing, service operational
    Unhealthy,         // Critical checks failing
    Unknown,           // Unable to determine health
    Starting,          // Service starting up
    ShuttingDown,      // Service shutting down
}
```

### Health Check API

```rust
pub struct HealthMonitor {
    pub async fn check(&self, service: &ServiceId) -> HealthResult;
    pub async fn check_all(&self) -> HashMap<ServiceId, HealthResult>;
    pub fn register_probe(&mut self, probe: HealthProbe);
    pub fn set_check_interval(&mut self, service: &ServiceId, interval: Duration);
    pub fn subscribe(&self) -> HealthEventStream;
}

pub struct HealthResult {
    pub state: HealthState,
    pub checks: Vec<CheckResult>,
    pub dependencies: Vec<DependencyHealth>,
    pub latency_ms: u64,
    pub timestamp: DateTime<Utc>,
}
```

### Health Probe Configuration

```toml
[layer.L2.health]
default_interval_ms = 5000
default_timeout_ms = 3000
failure_threshold = 3
success_threshold = 2

[[layer.L2.health.probes]]
name = "database"
type = "tcp"
target = "localhost:5432"
interval_ms = 5000

[[layer.L2.health.probes]]
name = "api_server"
type = "http"
target = "http://localhost:8080/health"
method = "GET"
expected_status = 200

[[layer.L2.health.probes]]
name = "consensus"
type = "grpc"
target = "localhost:9090"
service = "consensus.HealthService"
```

---

## Service Discovery

### Service Registry

```rust
pub struct ServiceRegistry {
    pub async fn register(&mut self, service: ServiceDefinition) -> Result<ServiceId>;
    pub async fn deregister(&mut self, id: &ServiceId) -> Result<()>;
    pub async fn discover(&self, query: ServiceQuery) -> Vec<ServiceEndpoint>;
    pub async fn watch(&self, query: ServiceQuery) -> ServiceWatcher;
    pub fn get_endpoints(&self, service: &str) -> Vec<Endpoint>;
}

pub struct ServiceDefinition {
    pub name: String,
    pub version: String,
    pub endpoints: Vec<Endpoint>,
    pub metadata: HashMap<String, String>,
    pub health_check: HealthCheckConfig,
    pub tags: Vec<String>,
}
```

### Discovery Mechanisms

| Mechanism | Use Case | Configuration |
|-----------|----------|---------------|
| Static | Known endpoints | Config file |
| DNS | DNS-based discovery | DNS server |
| Consul | Service mesh | Consul agent |
| Kubernetes | K8s environments | In-cluster config |
| Eureka | Spring ecosystem | Eureka server |

### Load Balancing Strategies

```rust
pub enum LoadBalancingStrategy {
    RoundRobin,
    LeastConnections,
    WeightedRoundRobin { weights: HashMap<ServiceId, u32> },
    Random,
    ConsistentHash { key: String },
    HealthAware { prefer_healthy: bool },
}
```

### Discovery Configuration

```toml
[layer.L2.discovery]
mechanism = "consul"
refresh_interval_ms = 30000
cache_ttl_ms = 60000

[layer.L2.discovery.consul]
address = "localhost:8500"
datacenter = "dc1"
token = "${CONSUL_TOKEN}"

[layer.L2.discovery.load_balancing]
strategy = "health_aware"
prefer_healthy = true
health_weight = 0.7
```

---

## Service Mesh Controller

### Traffic Management

```rust
pub struct TrafficManager {
    pub fn route(&self, request: &Request) -> RoutingDecision;
    pub fn apply_retry_policy(&mut self, service: &str, policy: RetryPolicy);
    pub fn apply_timeout(&mut self, service: &str, timeout: Duration);
    pub fn set_rate_limit(&mut self, service: &str, limit: RateLimit);
    pub fn circuit_breaker(&self, service: &str) -> &CircuitBreaker;
}

pub struct RetryPolicy {
    pub max_retries: u32,
    pub backoff: BackoffStrategy,
    pub retryable_codes: Vec<StatusCode>,
    pub per_try_timeout: Duration,
}
```

### mTLS Configuration

```toml
[layer.L2.mesh.mtls]
enabled = true
cert_path = "/etc/certs/service.crt"
key_path = "/etc/certs/service.key"
ca_path = "/etc/certs/ca.crt"
verify_client = true
min_tls_version = "1.3"
```

### Rate Limiting

```rust
pub struct RateLimit {
    pub requests_per_second: u32,
    pub burst_size: u32,
    pub key_func: RateLimitKey,
}

pub enum RateLimitKey {
    Global,
    PerIp,
    PerUser,
    PerEndpoint,
    Custom(Box<dyn Fn(&Request) -> String>),
}
```

---

## Inter-Layer Communication

### Events Published to L3 (Learning)

```rust
pub enum L2Event {
    ServiceHealthChanged { service: ServiceId, old: HealthState, new: HealthState },
    ServiceDiscovered { service: ServiceDefinition },
    ServiceLost { service: ServiceId, reason: String },
    LatencyAnomaly { service: ServiceId, latency_ms: u64, threshold_ms: u64 },
    CircuitOpened { service: ServiceId, failures: u32 },
    CircuitClosed { service: ServiceId, recovery_time_ms: u64 },
}
```

### Requests to L1 (Foundation)

- Error classification for health check failures
- Metric reporting for service health
- Configuration for probe settings
- State persistence for service registry

---

## Monitoring and Observability

### Metrics

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `me_l2_health_check_duration_ms` | Histogram | service, type | Health check latency |
| `me_l2_health_state` | Gauge | service | Current health state (0-5) |
| `me_l2_discovery_cache_hits` | Counter | mechanism | Discovery cache hit count |
| `me_l2_circuit_state` | Gauge | service | Circuit breaker state |
| `me_l2_active_services` | Gauge | - | Number of registered services |

### Alerting Rules

```yaml
groups:
  - name: L2_Services
    rules:
      - alert: ServiceUnhealthy
        expr: me_l2_health_state{state="unhealthy"} > 0
        for: 2m
        labels:
          severity: critical
        annotations:
          summary: "Service {{ $labels.service }} is unhealthy"

      - alert: HighHealthCheckLatency
        expr: me_l2_health_check_duration_ms > 3000
        for: 5m
        labels:
          severity: warning
```

---

## Configuration

```toml
[layer.L2]
enabled = true
startup_order = 2

[layer.L2.health]
default_interval_ms = 5000
default_timeout_ms = 3000
failure_threshold = 3
success_threshold = 2
parallel_checks = true
max_concurrent_checks = 50

[layer.L2.discovery]
mechanism = "consul"
refresh_interval_ms = 30000
cache_enabled = true
cache_ttl_ms = 60000

[layer.L2.mesh]
mtls_enabled = true
rate_limiting_enabled = true
circuit_breaker_enabled = true
default_timeout_ms = 5000
default_retry_count = 3
```

---

## Navigation

| Direction | Link |
|-----------|------|
| Up | [INDEX.md](../INDEX.md) |
| Previous | [L01_FOUNDATION.md](L01_FOUNDATION.md) |
| Next | [L03_CORE_LOGIC.md](L03_CORE_LOGIC.md) |
| API Reference | [REST_API.md](../integration/REST_API.md) |
| Pattern | [PATTERN_001_CIRCUIT_BREAKER.md](../patterns/PATTERN_001_CIRCUIT_BREAKER.md) |

---

*[Back to Index](../INDEX.md) | [Previous: L01 Foundation](L01_FOUNDATION.md) | [Next: L03 Core Logic](L03_CORE_LOGIC.md)*
