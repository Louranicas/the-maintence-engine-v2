# Layer 1: Foundation

> **L01_FOUNDATION** | Modules M1-M6 | [Back to Index](../INDEX.md)

---

## Navigation

| Direction | Link |
|-----------|------|
| Up | [INDEX.md](../INDEX.md) |
| Previous | [QUICKSTART.md](../QUICKSTART.md) |
| Next | [L02_SERVICES.md](L02_SERVICES.md) |
| Related Module | [M01_ERROR_TAXONOMY.md](../modules/M01_ERROR_TAXONOMY.md) |

---

## Layer Overview

The Foundation Layer (L1) provides the core infrastructure upon which all other layers depend. It implements six foundational modules (M1-M6) that handle error classification, configuration management, logging, metrics, state persistence, and resource management.

### Layer Properties

| Property | Value |
|----------|-------|
| Layer ID | L1 |
| Layer Name | Foundation |
| Module Count | 6 |
| Dependencies | None (base layer) |
| Dependents | L2-L6 |
| Criticality | Critical |

---

## Module Architecture

```
+------------------------------------------------------------------+
|                      L1: Foundation Layer                         |
+------------------------------------------------------------------+
|                                                                  |
|  +-------------+  +-------------+  +-------------+               |
|  |     M1      |  |     M2      |  |     M3      |               |
|  |   Error     |  |   Config    |  |   Logging   |               |
|  |  Taxonomy   |  |  Manager    |  |   System    |               |
|  +------+------+  +------+------+  +------+------+               |
|         |                |                |                      |
|         +--------+-------+--------+-------+                      |
|                  |                |                              |
|  +-------------+ | +-------------+ | +-------------+             |
|  |     M4      | | |     M5      | | |     M6      |             |
|  |   Metrics   |-+-|    State    |-+-|  Resource   |             |
|  |  Collector  |   | Persistence |   |   Manager   |             |
|  +-------------+   +-------------+   +-------------+             |
|                                                                  |
+------------------------------------------------------------------+
```

---

## M1: Error Taxonomy

The Error Taxonomy module provides a comprehensive 11-dimensional tensor encoding for error classification. See [M01_ERROR_TAXONOMY.md](../modules/M01_ERROR_TAXONOMY.md) for detailed specification.

### Key Features
- 11D tensor encoding for error vectors
- Hierarchical error categorization
- Semantic similarity computation
- Error pattern recognition

### API

```rust
pub struct ErrorTaxonomy {
    pub fn classify(&self, error: &dyn Error) -> ErrorVector;
    pub fn similarity(&self, a: &ErrorVector, b: &ErrorVector) -> f64;
    pub fn categorize(&self, vector: &ErrorVector) -> ErrorCategory;
    pub fn encode(&self, error: &dyn Error) -> [f32; 11];
}
```

### Error Categories

| Category | Code Range | Description |
|----------|-----------|-------------|
| Infrastructure | E1000-E1999 | Hardware, network, storage |
| Application | E2000-E2999 | Process, memory, runtime |
| Data | E3000-E3999 | Corruption, consistency |
| Security | E4000-E4999 | Auth, access, encryption |
| Performance | E5000-E5999 | Latency, throughput |

---

## M2: Configuration Manager

Centralized configuration management with hot-reload capability.

### Features
- TOML/YAML/JSON configuration formats
- Environment variable interpolation
- Hot-reload without restart
- Configuration validation
- Secret management integration

### API

```rust
pub struct ConfigManager {
    pub fn load(&mut self, path: &Path) -> Result<()>;
    pub fn get<T: DeserializeOwned>(&self, key: &str) -> Option<T>;
    pub fn set<T: Serialize>(&mut self, key: &str, value: T) -> Result<()>;
    pub fn watch(&self) -> ConfigWatcher;
    pub fn validate(&self) -> ValidationResult;
}
```

### Configuration Schema

```toml
[foundation.config]
version = "1.0"
reload_interval_ms = 5000
validation_strict = true

[foundation.config.sources]
primary = "/etc/maintenance-engine/config.toml"
override = "/etc/maintenance-engine/config.d/"
environment_prefix = "ME_"
```

---

## M3: Logging System

Structured logging with multi-destination output and log correlation.

### Features
- Structured JSON logging
- Log correlation IDs
- Multi-destination output (stdout, file, syslog)
- Log level filtering per module
- Async log writing

### API

```rust
pub struct Logger {
    pub fn trace(&self, message: &str, fields: &LogFields);
    pub fn debug(&self, message: &str, fields: &LogFields);
    pub fn info(&self, message: &str, fields: &LogFields);
    pub fn warn(&self, message: &str, fields: &LogFields);
    pub fn error(&self, message: &str, fields: &LogFields);
    pub fn with_correlation(&self, id: CorrelationId) -> Logger;
}
```

### Log Format

```json
{
  "timestamp": "2026-01-28T12:00:00.000Z",
  "level": "INFO",
  "layer": "L1",
  "module": "M3",
  "correlation_id": "corr-abc123",
  "message": "Configuration loaded successfully",
  "fields": {
    "config_path": "/etc/maintenance-engine/config.toml",
    "keys_loaded": 47
  }
}
```

---

## M4: Metrics Collector

Real-time metrics collection and aggregation.

### Features
- Counter, gauge, histogram metrics
- Prometheus-compatible export
- Custom metric dimensions
- Aggregation windows
- Metric retention policies

### API

```rust
pub struct MetricsCollector {
    pub fn counter(&self, name: &str, labels: &Labels) -> Counter;
    pub fn gauge(&self, name: &str, labels: &Labels) -> Gauge;
    pub fn histogram(&self, name: &str, buckets: &[f64]) -> Histogram;
    pub fn export(&self) -> PrometheusMetrics;
    pub fn snapshot(&self) -> MetricsSnapshot;
}
```

### Core Metrics

| Metric | Type | Description |
|--------|------|-------------|
| `me_errors_total` | Counter | Total errors by category |
| `me_layer_health` | Gauge | Layer health score (0-1) |
| `me_request_duration_ms` | Histogram | Request latency distribution |
| `me_active_connections` | Gauge | Current active connections |

---

## M5: State Persistence

Durable state storage with transactional guarantees.

### Features
- PostgreSQL backend
- Transactional writes
- Point-in-time recovery
- State snapshots
- Migration management

### API

```rust
pub struct StatePersistence {
    pub async fn save<T: Serialize>(&self, key: &str, state: &T) -> Result<()>;
    pub async fn load<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>>;
    pub async fn transaction<F, R>(&self, f: F) -> Result<R>
    where F: FnOnce(&Transaction) -> Result<R>;
    pub async fn snapshot(&self) -> Result<Snapshot>;
}
```

### State Schema

```sql
CREATE TABLE me_state (
    key VARCHAR(255) PRIMARY KEY,
    value JSONB NOT NULL,
    version BIGINT NOT NULL DEFAULT 1,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_me_state_updated ON me_state(updated_at);
```

---

## M6: Resource Manager

Resource allocation and lifecycle management.

### Features
- Connection pool management
- Memory budgets
- CPU affinity
- File descriptor limits
- Graceful resource release

### API

```rust
pub struct ResourceManager {
    pub fn allocate(&mut self, request: ResourceRequest) -> Result<ResourceHandle>;
    pub fn release(&mut self, handle: ResourceHandle) -> Result<()>;
    pub fn status(&self) -> ResourceStatus;
    pub fn set_budget(&mut self, resource: ResourceType, limit: usize);
    pub fn cleanup(&mut self) -> CleanupReport;
}
```

### Resource Limits

| Resource | Default Limit | Configurable |
|----------|---------------|--------------|
| DB Connections | 100 | Yes |
| Memory (MB) | 2048 | Yes |
| File Descriptors | 10000 | Yes |
| Worker Threads | 8 | Yes |

---

## Inter-Module Communication

```
M1 (Error) <---> M3 (Logging)     : Error events logged
M2 (Config) <---> M3 (Logging)    : Log level configuration
M2 (Config) <---> M5 (State)      : Config persistence
M3 (Logging) <---> M4 (Metrics)   : Log volume metrics
M4 (Metrics) <---> M5 (State)     : Metric persistence
M6 (Resource) <---> M4 (Metrics)  : Resource utilization metrics
```

---

## Layer Health Check

```rust
pub struct L1HealthCheck {
    pub async fn check(&self) -> L1HealthStatus {
        L1HealthStatus {
            m1_error_taxonomy: self.check_m1().await,
            m2_config_manager: self.check_m2().await,
            m3_logging_system: self.check_m3().await,
            m4_metrics_collector: self.check_m4().await,
            m5_state_persistence: self.check_m5().await,
            m6_resource_manager: self.check_m6().await,
        }
    }
}
```

---

## Configuration

```toml
[layer.L1]
enabled = true
startup_order = 1

[layer.L1.M1]
tensor_dimensions = 11
similarity_threshold = 0.85

[layer.L1.M2]
config_path = "/etc/maintenance-engine/config.toml"
hot_reload = true

[layer.L1.M3]
level = "info"
format = "json"
outputs = ["stdout", "file"]

[layer.L1.M4]
export_interval_ms = 15000
retention_hours = 168

[layer.L1.M5]
database_url = "postgres://localhost/maintenance_engine"
pool_size = 20

[layer.L1.M6]
max_memory_mb = 2048
max_connections = 100
```

---

## Navigation

| Direction | Link |
|-----------|------|
| Up | [INDEX.md](../INDEX.md) |
| Previous | [QUICKSTART.md](../QUICKSTART.md) |
| Next | [L02_SERVICES.md](L02_SERVICES.md) |
| Related Module | [M01_ERROR_TAXONOMY.md](../modules/M01_ERROR_TAXONOMY.md) |
| Pattern | [PATTERN_001_CIRCUIT_BREAKER.md](../patterns/PATTERN_001_CIRCUIT_BREAKER.md) |

---

*[Back to Index](../INDEX.md) | [Next: L02 Services](L02_SERVICES.md)*
