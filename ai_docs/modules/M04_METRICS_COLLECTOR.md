# Module M04: Metrics Collector

> **M04_METRICS_COLLECTOR** | Real-time Metrics | Layer: L1 Foundation | [Back to Index](../INDEX.md)

---

## Navigation

| Direction | Link |
|-----------|------|
| Up | [INDEX.md](../INDEX.md) |
| Layer | [L01_FOUNDATION.md](../layers/L01_FOUNDATION.md) |
| Previous | [M03_LOGGING_SYSTEM.md](M03_LOGGING_SYSTEM.md) |
| Next | [M05_STATE_PERSISTENCE.md](M05_STATE_PERSISTENCE.md) |
| Related | [M06_RESOURCE_MANAGER.md](M06_RESOURCE_MANAGER.md), [L02_SERVICES.md](../layers/L02_SERVICES.md) |

---

## Module Overview

The Metrics Collector (M04) provides real-time metrics collection, aggregation, and export for all layers and modules of the Maintenance Engine. It supports Prometheus-compatible metrics, custom dimensions, aggregation windows, and integration with the 12D tensor encoding system.

### Module Properties

| Property | Value |
|----------|-------|
| Module ID | M04 |
| Module Name | Metrics Collector |
| Layer | L1 (Foundation) |
| Version | 1.0 |
| Dependencies | M02 (Configuration), M03 (Logging) |
| Dependents | M06 (Resource), L2-L6 (All Layers), External (Prometheus) |
| Criticality | High |
| Startup Order | 4 (after M03) |

---

## Architecture Diagram

```
+------------------------------------------------------------------+
|                     M04: Metrics Collector                        |
+------------------------------------------------------------------+
|                                                                    |
|  +----------------+     +------------------+     +---------------+ |
|  | Metric         |     |   Aggregator     |     |   Exporter    | |
|  | Registry       |---->|   (Windows)      |---->|   (Prometheus)| |
|  +----------------+     +------------------+     +---------------+ |
|         ^                       |                       |         |
|         |                       v                       v         |
|  +----------------+     +------------------+     +---------------+ |
|  | Metric         |     |   12D Tensor     |     |   HTTP        | |
|  | Collectors     |     |   Encoder        |     |   Endpoint    | |
|  +----------------+     +------------------+     +---------------+ |
|         ^                       |                       |         |
|         |                       v                       v         |
|  +----------------+     +------------------+     +---------------+ |
|  | L1-L6 Modules  |     |   Time-Series    |     |   Push        | |
|  | (Sources)      |     |   Storage        |     |   Gateway     | |
|  +----------------+     +------------------+     +---------------+ |
|                                                                    |
+------------------------------------------------------------------+
```

---

## Core Data Structures

### Metric Types

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::sync::Arc;
use parking_lot::RwLock;

/// Metric label key-value pairs
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Labels(HashMap<String, String>);

impl Labels {
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    pub fn with<K: Into<String>, V: Into<String>>(mut self, key: K, value: V) -> Self {
        self.0.insert(key.into(), value.into());
        self
    }

    pub fn layer(self, layer: &str) -> Self {
        self.with("layer", layer)
    }

    pub fn module(self, module: &str) -> Self {
        self.with("module", module)
    }

    pub fn service(self, service: &str) -> Self {
        self.with("service", service)
    }
}

/// Counter metric (monotonically increasing)
#[derive(Debug)]
pub struct Counter {
    /// Metric name
    name: String,
    /// Metric help text
    help: String,
    /// Label values -> counter value
    values: RwLock<HashMap<Labels, AtomicU64>>,
}

impl Counter {
    /// Increment counter by 1
    pub fn inc(&self, labels: &Labels);

    /// Increment counter by value
    pub fn inc_by(&self, labels: &Labels, value: u64);

    /// Get current value
    pub fn get(&self, labels: &Labels) -> u64;

    /// Reset counter (use with caution)
    pub fn reset(&self, labels: &Labels);
}

/// Gauge metric (can increase or decrease)
#[derive(Debug)]
pub struct Gauge {
    /// Metric name
    name: String,
    /// Metric help text
    help: String,
    /// Label values -> gauge value
    values: RwLock<HashMap<Labels, AtomicI64>>,
}

impl Gauge {
    /// Set gauge to value
    pub fn set(&self, labels: &Labels, value: i64);

    /// Increment gauge
    pub fn inc(&self, labels: &Labels);

    /// Decrement gauge
    pub fn dec(&self, labels: &Labels);

    /// Add to gauge
    pub fn add(&self, labels: &Labels, value: i64);

    /// Subtract from gauge
    pub fn sub(&self, labels: &Labels, value: i64);

    /// Get current value
    pub fn get(&self, labels: &Labels) -> i64;
}

/// Histogram metric (value distribution)
#[derive(Debug)]
pub struct Histogram {
    /// Metric name
    name: String,
    /// Metric help text
    help: String,
    /// Bucket boundaries
    buckets: Vec<f64>,
    /// Label values -> histogram data
    values: RwLock<HashMap<Labels, HistogramData>>,
}

/// Histogram internal data
#[derive(Debug, Clone)]
pub struct HistogramData {
    /// Bucket counts (includes +Inf)
    pub bucket_counts: Vec<AtomicU64>,
    /// Sum of all observed values
    pub sum: AtomicI64,  // Stored as fixed-point
    /// Count of observations
    pub count: AtomicU64,
}

impl Histogram {
    /// Observe a value
    pub fn observe(&self, labels: &Labels, value: f64);

    /// Get bucket counts
    pub fn get_buckets(&self, labels: &Labels) -> Vec<u64>;

    /// Get sum of observations
    pub fn get_sum(&self, labels: &Labels) -> f64;

    /// Get count of observations
    pub fn get_count(&self, labels: &Labels) -> u64;

    /// Get quantile estimate (linear interpolation)
    pub fn quantile(&self, labels: &Labels, q: f64) -> f64;
}

/// Summary metric (quantile estimates)
#[derive(Debug)]
pub struct Summary {
    /// Metric name
    name: String,
    /// Metric help text
    help: String,
    /// Quantiles to track
    quantiles: Vec<f64>,
    /// Max age of observations
    max_age: std::time::Duration,
    /// Label values -> summary data
    values: RwLock<HashMap<Labels, SummaryData>>,
}

/// Summary internal data
#[derive(Debug)]
pub struct SummaryData {
    /// Quantile values (rolling window)
    quantile_values: Vec<f64>,
    /// Sum of observations
    sum: f64,
    /// Count of observations
    count: u64,
    /// Last update time
    last_update: DateTime<Utc>,
}

/// Metric metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricMetadata {
    /// Metric name
    pub name: String,
    /// Metric type
    pub metric_type: MetricType,
    /// Help text
    pub help: String,
    /// Unit (optional)
    pub unit: Option<String>,
    /// Layer that owns this metric
    pub layer: String,
    /// Module that owns this metric
    pub module: String,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
}

/// Metric types
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum MetricType {
    Counter,
    Gauge,
    Histogram,
    Summary,
}

/// 12D Tensor metric (special for Maintenance Engine)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TensorMetric {
    /// Metric name
    pub name: String,
    /// 12-dimensional values
    pub dimensions: [f64; 12],
    /// Labels
    pub labels: Labels,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
}

/// Aggregated metric snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricSnapshot {
    /// Snapshot timestamp
    pub timestamp: DateTime<Utc>,
    /// Counter values
    pub counters: HashMap<String, HashMap<Labels, u64>>,
    /// Gauge values
    pub gauges: HashMap<String, HashMap<Labels, i64>>,
    /// Histogram summaries
    pub histograms: HashMap<String, HashMap<Labels, HistogramSummary>>,
    /// Tensor metrics
    pub tensors: Vec<TensorMetric>,
}

/// Histogram summary for export
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistogramSummary {
    pub count: u64,
    pub sum: f64,
    pub buckets: Vec<(f64, u64)>,  // (le, count)
    pub quantiles: HashMap<String, f64>,  // "p50", "p95", etc.
}
```

### Metrics Collector State

```rust
/// Main metrics collector
pub struct MetricsCollector {
    /// Metric registry
    registry: Arc<MetricRegistry>,
    /// Configuration
    config: MetricsConfig,
    /// Aggregation windows
    aggregator: Arc<MetricAggregator>,
    /// 12D Tensor encoder
    tensor_encoder: Arc<TensorEncoder>,
    /// Time-series storage
    storage: Arc<dyn MetricStorage>,
    /// Export endpoints
    exporters: Vec<Arc<dyn MetricExporter>>,
    /// Shutdown signal
    shutdown: tokio::sync::watch::Sender<bool>,
}

/// Metric registry
pub struct MetricRegistry {
    /// Registered counters
    counters: RwLock<HashMap<String, Arc<Counter>>>,
    /// Registered gauges
    gauges: RwLock<HashMap<String, Arc<Gauge>>>,
    /// Registered histograms
    histograms: RwLock<HashMap<String, Arc<Histogram>>>,
    /// Registered summaries
    summaries: RwLock<HashMap<String, Arc<Summary>>>,
    /// Metric metadata
    metadata: RwLock<HashMap<String, MetricMetadata>>,
}

/// Metric aggregator for time windows
pub struct MetricAggregator {
    /// Aggregation windows (1m, 5m, 15m, 1h)
    windows: Vec<AggregationWindow>,
    /// Current aggregations
    aggregations: RwLock<HashMap<String, Vec<AggregatedValue>>>,
}

/// Aggregation window definition
#[derive(Debug, Clone)]
pub struct AggregationWindow {
    /// Window name
    pub name: String,
    /// Window duration
    pub duration: std::time::Duration,
    /// Aggregation function
    pub function: AggregationFunction,
    /// Retention period
    pub retention: std::time::Duration,
}

/// Aggregation functions
#[derive(Debug, Clone, Copy)]
pub enum AggregationFunction {
    Sum,
    Avg,
    Min,
    Max,
    Count,
    Rate,
    P50,
    P95,
    P99,
}

/// Metrics configuration
#[derive(Debug, Clone)]
pub struct MetricsConfig {
    /// Export interval (ms)
    pub export_interval_ms: u64,
    /// Retention period (hours)
    pub retention_hours: u64,
    /// Prometheus HTTP port
    pub prometheus_port: u16,
    /// Enable push gateway
    pub push_gateway_enabled: bool,
    /// Push gateway URL
    pub push_gateway_url: Option<String>,
    /// Default histogram buckets
    pub default_buckets: Vec<f64>,
    /// Enable 12D tensor metrics
    pub tensor_metrics_enabled: bool,
    /// Max metrics per module
    pub max_metrics_per_module: usize,
}

/// Trait for metric exporters
#[async_trait::async_trait]
pub trait MetricExporter: Send + Sync {
    /// Export metrics
    async fn export(&self, snapshot: &MetricSnapshot) -> Result<(), MetricError>;

    /// Exporter name
    fn name(&self) -> &str;

    /// Health check
    async fn health_check(&self) -> bool;
}

/// Trait for metric storage
#[async_trait::async_trait]
pub trait MetricStorage: Send + Sync {
    /// Store metric snapshot
    async fn store(&self, snapshot: &MetricSnapshot) -> Result<(), MetricError>;

    /// Query metrics
    async fn query(&self, query: &MetricQuery) -> Result<Vec<MetricSnapshot>, MetricError>;

    /// Cleanup old metrics
    async fn cleanup(&self, retention: std::time::Duration) -> Result<u64, MetricError>;
}

/// Metric query
#[derive(Debug, Clone)]
pub struct MetricQuery {
    /// Metric name pattern
    pub name: String,
    /// Label filters
    pub labels: Labels,
    /// Start time
    pub start: DateTime<Utc>,
    /// End time
    pub end: DateTime<Utc>,
    /// Step (for range queries)
    pub step: Option<std::time::Duration>,
    /// Aggregation function
    pub aggregation: Option<AggregationFunction>,
}
```

---

## Public API

```rust
impl MetricsCollector {
    /// Create new metrics collector
    pub fn new(config: MetricsConfig) -> Result<Self, MetricError>;

    /// Register a counter metric
    pub fn counter(&self, name: &str, help: &str) -> Arc<Counter>;

    /// Register a gauge metric
    pub fn gauge(&self, name: &str, help: &str) -> Arc<Gauge>;

    /// Register a histogram metric
    pub fn histogram(&self, name: &str, help: &str, buckets: &[f64]) -> Arc<Histogram>;

    /// Register a histogram with default buckets
    pub fn histogram_default(&self, name: &str, help: &str) -> Arc<Histogram>;

    /// Register a summary metric
    pub fn summary(&self, name: &str, help: &str, quantiles: &[f64]) -> Arc<Summary>;

    /// Record 12D tensor metric
    pub fn record_tensor(&self, name: &str, dimensions: [f64; 12], labels: Labels);

    /// Get current metric snapshot
    pub fn snapshot(&self) -> MetricSnapshot;

    /// Export metrics in Prometheus format
    pub fn export_prometheus(&self) -> String;

    /// Export metrics as JSON
    pub fn export_json(&self) -> String;

    /// Query metrics
    pub async fn query(&self, query: MetricQuery) -> Result<Vec<MetricSnapshot>, MetricError>;

    /// Get aggregated value
    pub fn aggregated(&self, name: &str, window: &str, function: AggregationFunction) -> Option<f64>;

    /// Start HTTP server for Prometheus scraping
    pub async fn start_http_server(&self) -> Result<(), MetricError>;

    /// Start push gateway export
    pub async fn start_push_gateway(&self) -> Result<(), MetricError>;

    /// Shutdown collector
    pub async fn shutdown(&mut self) -> Result<(), MetricError>;

    /// Get registry for advanced operations
    pub fn registry(&self) -> Arc<MetricRegistry>;

    /// Health check
    pub fn health(&self) -> MetricHealth;
}

impl MetricRegistry {
    /// Get counter by name
    pub fn get_counter(&self, name: &str) -> Option<Arc<Counter>>;

    /// Get gauge by name
    pub fn get_gauge(&self, name: &str) -> Option<Arc<Gauge>>;

    /// Get histogram by name
    pub fn get_histogram(&self, name: &str) -> Option<Arc<Histogram>>;

    /// List all metrics
    pub fn list(&self) -> Vec<MetricMetadata>;

    /// Unregister metric
    pub fn unregister(&self, name: &str) -> bool;
}
```

---

## Configuration

```toml
[foundation.metrics]
version = "1.0"

[foundation.metrics.collection]
# Export interval for aggregations
export_interval_ms = 15000
# Metric retention
retention_hours = 168
# Max metrics per module
max_metrics_per_module = 100

[foundation.metrics.prometheus]
# Enable Prometheus HTTP endpoint
enabled = true
# HTTP port
port = 9090
# Path
path = "/metrics"

[foundation.metrics.push_gateway]
# Enable push gateway
enabled = false
# Gateway URL
url = "http://prometheus-pushgateway:9091"
# Push interval (ms)
interval_ms = 60000
# Job name
job = "maintenance_engine"

[foundation.metrics.buckets]
# Default histogram buckets (ms)
default = [1, 5, 10, 25, 50, 100, 250, 500, 1000, 2500, 5000, 10000]
# Latency buckets (ms)
latency = [0.5, 1, 2.5, 5, 10, 25, 50, 100, 250, 500, 1000]
# Size buckets (bytes)
size = [100, 1000, 10000, 100000, 1000000, 10000000]

[foundation.metrics.aggregation]
# Aggregation windows
windows = ["1m", "5m", "15m", "1h"]
# Default aggregation functions
functions = ["avg", "max", "p95"]

[foundation.metrics.tensor]
# Enable 12D tensor metrics
enabled = true
# Tensor metric prefix
prefix = "me_tensor"
# Tensor retention (shorter)
retention_hours = 24

[foundation.metrics.storage]
# Storage backend
backend = "memory"  # or "sqlite", "prometheus"
# Memory storage max entries
max_entries = 100000
# SQLite path (if using SQLite)
sqlite_path = "data/performance_metrics.db"
```

---

## Bi-Directional Data Flow

### Inbound Data Flow

```
+------------------------------------------------------------------+
|                     INBOUND DATA SOURCES                          |
+------------------------------------------------------------------+
|                                                                    |
|  +-------------+   +-------------+   +-------------+              |
|  | All Layers  |   |    M02      |   |    M03      |              |
|  | L1-L6       |   |   Config    |   |   Logging   |              |
|  | (Metrics)   |   |   Manager   |   |   System    |              |
|  +------+------+   +------+------+   +------+------+              |
|         |                 |                 |                      |
|         v                 v                 v                      |
|    Metric Values     Config Updates    Log Volume                  |
|    (inc/observe)     (intervals, etc)  Metrics                     |
|         |                 |                 |                      |
|         +--------+--------+--------+--------+                      |
|                  |                 |                               |
|                  v                 v                               |
|         +------------------+  +------------------+                 |
|         |  Metric Registry |  | Config Updater   |                 |
|         +------------------+  +------------------+                 |
|                                                                    |
+------------------------------------------------------------------+
```

#### Inbound Message Types

```rust
/// Messages received by Metrics Collector
#[derive(Debug, Clone)]
pub enum MetricInboundMessage {
    /// Counter increment
    CounterIncrement {
        name: String,
        labels: Labels,
        value: u64,
    },

    /// Gauge set
    GaugeSet {
        name: String,
        labels: Labels,
        value: i64,
    },

    /// Histogram observation
    HistogramObserve {
        name: String,
        labels: Labels,
        value: f64,
    },

    /// 12D Tensor metric
    TensorRecord {
        name: String,
        dimensions: [f64; 12],
        labels: Labels,
        timestamp: DateTime<Utc>,
    },

    /// Configuration update from M02
    ConfigUpdate {
        config: MetricsConfig,
    },

    /// Log volume update from M03
    LogVolumeUpdate {
        total_logs: u64,
        by_level: HashMap<String, u64>,
        by_module: HashMap<String, u64>,
    },

    /// Resource metrics from M06
    ResourceMetrics {
        memory_used: u64,
        memory_total: u64,
        connections_active: u64,
        connections_max: u64,
        file_descriptors: u64,
    },

    /// Query request
    QueryRequest {
        query: MetricQuery,
        response_channel: tokio::sync::oneshot::Sender<Result<Vec<MetricSnapshot>, MetricError>>,
    },

    /// Export request
    ExportRequest {
        format: ExportFormat,
        response_channel: tokio::sync::oneshot::Sender<String>,
    },
}

/// Export formats
#[derive(Debug, Clone)]
pub enum ExportFormat {
    Prometheus,
    Json,
    OpenMetrics,
}
```

#### Inbound Sequence Diagram

```
                    INBOUND FLOW

    L1-L6           M03 Logging        M06 Resource
    Modules         System             Manager
        |               |                   |
        | counter.inc() | LogVolumeUpdate   | ResourceMetrics
        | gauge.set()   | (periodic)        | (periodic)
        | histogram     |                   |
        | .observe()    |                   |
        v               v                   v
    +----------------------------------------------+
    |            Metrics Collector Inbox            |
    +----------------------------------------------+
                        |
                        v
              +------------------+
              |  Metric Registry |
              |  Update          |
              +------------------+
                        |
         +--------------+--------------+
         |              |              |
         v              v              v
    +----------+  +-----------+  +-----------+
    | Counter  |  |   Gauge   |  | Histogram |
    | Storage  |  |  Storage  |  |  Storage  |
    +----------+  +-----------+  +-----------+
         |              |              |
         +--------------+--------------+
                        |
                        v
              +------------------+
              |   Aggregator     |
              |   (Time Windows) |
              +------------------+
```

### Outbound Data Flow

```
+------------------------------------------------------------------+
|                     OUTBOUND DATA TARGETS                         |
+------------------------------------------------------------------+
|                                                                    |
|         +------------------+                                       |
|         | Metrics Collector|                                       |
|         |  (Source)        |                                       |
|         +--------+---------+                                       |
|                  |                                                 |
|    +-------------+-------------+-------------+-------------+       |
|    |             |             |             |             |       |
|    v             v             v             v             v       |
| +------+   +------+   +------+   +------+   +------+              |
| | M05  |   | M06  |   | Prom |   | Push |   | L2-6 |              |
| |State |   | Res  |   | HTTP |   | Gate |   |Query |              |
| +------+   +------+   +------+   +------+   +------+              |
|    |             |             |             |             |       |
|    v             v             v             v             v       |
| Metric       Resource    /metrics     Push        Query            |
| Snapshots    Alerts      Scrape       Export      Results          |
|                                                                    |
+------------------------------------------------------------------+
```

#### Outbound Message Types

```rust
/// Messages sent by Metrics Collector
#[derive(Debug, Clone)]
pub enum MetricOutboundMessage {
    /// Metric snapshot for persistence
    MetricSnapshot {
        snapshot: MetricSnapshot,
        checkpoint_id: String,
    },

    /// Resource alert to M06
    ResourceAlert {
        alert_type: ResourceAlertType,
        metric_name: String,
        current_value: f64,
        threshold: f64,
        labels: Labels,
    },

    /// Prometheus export response
    PrometheusExport {
        metrics: String,
        timestamp: DateTime<Utc>,
    },

    /// Query response to requesting module
    QueryResponse {
        query_id: String,
        results: Vec<MetricSnapshot>,
        execution_time_ms: u64,
    },

    /// Health status update
    HealthUpdate {
        healthy: bool,
        metrics_count: usize,
        storage_size: u64,
        last_export: DateTime<Utc>,
    },

    /// Aggregation complete notification
    AggregationComplete {
        window: String,
        metrics_aggregated: usize,
        timestamp: DateTime<Utc>,
    },
}

/// Resource alert types
#[derive(Debug, Clone)]
pub enum ResourceAlertType {
    MemoryHigh,
    ConnectionsHigh,
    LatencyHigh,
    ErrorRateHigh,
    DiskSpaceLow,
    CustomThreshold,
}
```

#### Outbound Sequence Diagram

```
                    OUTBOUND FLOW

                 Metrics Collector
                        |
                        | Aggregation Complete
                        v
              +------------------+
              |   Export Engine  |
              +------------------+
                        |
         +--------------+--------------+--------------+
         |              |              |              |
         v              v              v              v
    +----------+  +-----------+  +-----------+  +-----------+
    | Prometheus|  |   M05     |  |   M06     |  |   Push    |
    | HTTP      |  |  State    |  | Resource  |  |  Gateway  |
    +----------+  +-----------+  +-----------+  +-----------+
         |              |              |              |
         v              v              v              v
    Prometheus     Snapshot       Resource        Remote
    Scrape         Persisted      Alerts          Export


    Alert Path:

              +------------------+
              |  Threshold Check |
              +--------+---------+
                       |
                       | Threshold Exceeded
                       v
              +------------------+
              |      M06         |
              | Resource Manager |
              +------------------+
                       |
                       v
              +------------------+
              |   Alert Action   |
              +------------------+
```

### Cross-Module Dependencies

```
+------------------------------------------------------------------+
|            BI-DIRECTIONAL DEPENDENCY MATRIX (M04)                 |
+------------------------------------------------------------------+
|                                                                    |
|  Module    | M04 Reads From        | M04 Writes To                |
|  ----------|------------------------|-----------------------------  |
|  M01       | Error metrics via     | -                            |
|            | logging correlation   |                              |
|  ----------|------------------------|-----------------------------  |
|  M02       | Metrics configuration | -                            |
|            | (intervals, buckets)  |                              |
|  ----------|------------------------|-----------------------------  |
|  M03       | Log volume metrics    | Metrics for logging          |
|            |                        | subsystem                    |
|  ----------|------------------------|-----------------------------  |
|  M05       | -                      | Metric snapshots for         |
|            |                        | persistence                  |
|  ----------|------------------------|-----------------------------  |
|  M06       | Resource metrics      | Resource alerts,             |
|            | (memory, connections) | utilization reports          |
|  ----------|------------------------|-----------------------------  |
|  L2-L6     | Module metrics        | Query results,               |
|            | (counters, gauges)    | health reports               |
|  ----------|------------------------|-----------------------------  |
|  External  | -                      | Prometheus scrape,           |
|            |                        | Push gateway                 |
|                                                                    |
+------------------------------------------------------------------+

Communication Patterns:
+------------------------------------------------------------------+
|  Pattern          | Source  | Target  | Type        | Frequency  |
|  -----------------|---------|---------|-------------|----------- |
|  Metric Record    | L1-L6   | M04     | Async       | Continuous |
|  Config Update    | M02     | M04     | Event       | On change  |
|  Log Volume       | M03     | M04     | Async       | Periodic   |
|  Resource Metrics | M06     | M04     | Async       | Periodic   |
|  Prometheus       | Prom    | M04     | HTTP GET    | 15s        |
|  Snapshot Export  | M04     | M05     | Async       | Periodic   |
|  Resource Alert   | M04     | M06     | Async       | On thresh  |
+------------------------------------------------------------------+

Error Propagation:
+------------------------------------------------------------------+
|  Error Source     | Propagates To       | Action                  |
|  -----------------|---------------------|------------------------ |
|  Storage Full     | M03 (log)           | Drop old + Alert        |
|  Export Failure   | M03 (log), M06      | Retry + Buffer          |
|  Query Timeout    | Requester           | Return partial          |
|  Invalid Metric   | M03 (log)           | Reject + Log            |
+------------------------------------------------------------------+
```

### Contextual Flow

```
+------------------------------------------------------------------+
|                   METRIC DATA LIFECYCLE                            |
+------------------------------------------------------------------+
|                                                                    |
|  1. RECORD PHASE                                                   |
|     +------------------+                                           |
|     |  counter.inc()   |  (Any module/layer)                      |
|     |  gauge.set()     |                                           |
|     |  histogram       |                                           |
|     |  .observe()      |                                           |
|     +--------+---------+                                           |
|              |                                                     |
|  2. STORE PHASE                                                    |
|              v                                                     |
|     +------------------+                                           |
|     |  Metric Registry |  (Atomic update)                         |
|     +--------+---------+                                           |
|              |                                                     |
|  3. AGGREGATE PHASE (Periodic)                                     |
|              v                                                     |
|     +------------------+                                           |
|     |   Aggregator     |  (1m, 5m, 15m, 1h windows)               |
|     +--------+---------+                                           |
|              |                                                     |
|              +--------+--------+                                   |
|              |                 |                                   |
|              v                 v                                   |
|     +------------------+ +------------------+                      |
|     | Time-Series      | |  12D Tensor      |                      |
|     | Storage          | |  Encoding        |                      |
|     +--------+---------+ +------------------+                      |
|              |                                                     |
|  4. EXPORT PHASE                                                   |
|              v                                                     |
|     +------------------+                                           |
|     |  Export Engine   |                                           |
|     +--------+---------+                                           |
|              |                                                     |
|              +-------+-------+-------+                             |
|              |       |       |       |                             |
|              v       v       v       v                             |
|           /metrics  Push   M05      Alerts                         |
|           Scrape    Gate   State    to M06                         |
|                                                                    |
|  5. CLEANUP PHASE (Periodic)                                       |
|     +------------------+                                           |
|     |   Retention      |  (Delete old data)                       |
|     |   Policy         |                                           |
|     +------------------+                                           |
|                                                                    |
+------------------------------------------------------------------+
```

#### 12D Tensor Integration

```
+------------------------------------------------------------------+
|              12D TENSOR METRIC ENCODING                           |
+------------------------------------------------------------------+
|                                                                    |
|  Service Metrics -> 12D Tensor:                                    |
|                                                                    |
|  D0:  service_id      (service identifier hash)                    |
|  D1:  port            (normalized: port/65535)                     |
|  D2:  tier            (service tier: 1-5 normalized)               |
|  D3:  dependencies    (dependency count normalized)                |
|  D4:  agents          (agent count normalized)                     |
|  D5:  protocol        (protocol type encoded)                      |
|  D6:  health_score    (0.0 - 1.0)                                  |
|  D7:  uptime          (uptime % normalized)                        |
|  D8:  synergy_score   (cross-system synergy)                       |
|  D9:  latency         (latency normalized by SLO)                  |
|  D10: error_rate      (errors / total normalized)                  |
|  D11: temporal        (time-of-day encoded)                        |
|                                                                    |
|  Example Tensor:                                                   |
|  [0.23, 0.12, 0.2, 0.3, 0.4, 0.5, 0.95, 0.99, 0.92, 0.1, 0.02, 0.5]|
|  (SYNTHEX service with high health and low latency)                |
|                                                                    |
+------------------------------------------------------------------+
```

---

## Metrics

| Metric | Type | Description |
|--------|------|-------------|
| `me_m04_metrics_total` | Counter | Total metrics registered |
| `me_m04_observations_total` | Counter | Total observations recorded |
| `me_m04_exports_total` | Counter | Total export operations |
| `me_m04_export_duration_ms` | Histogram | Export latency |
| `me_m04_queries_total` | Counter | Total query operations |
| `me_m04_query_duration_ms` | Histogram | Query latency |
| `me_m04_storage_size_bytes` | Gauge | Metric storage size |
| `me_m04_aggregation_duration_ms` | Histogram | Aggregation latency |
| `me_m04_alerts_triggered_total` | Counter | Alerts triggered |
| `me_m04_prometheus_scrapes_total` | Counter | Prometheus scrapes |

---

## Error Codes

| Code | Name | Description | Severity | Recovery |
|------|------|-------------|----------|----------|
| E4001 | METRIC_REGISTRATION_FAILED | Failed to register metric | Error | Log + Skip |
| E4002 | METRIC_STORAGE_FULL | Metric storage at capacity | Warning | Evict old |
| E4003 | METRIC_EXPORT_FAILED | Export operation failed | Warning | Retry |
| E4004 | METRIC_QUERY_TIMEOUT | Query exceeded timeout | Warning | Return partial |
| E4005 | METRIC_INVALID_NAME | Invalid metric name | Error | Reject |
| E4006 | METRIC_INVALID_LABELS | Invalid label name/value | Error | Reject |
| E4007 | METRIC_AGGREGATION_FAILED | Aggregation failed | Warning | Skip window |
| E4008 | METRIC_PROMETHEUS_ERROR | Prometheus endpoint error | Warning | Retry |
| E4009 | METRIC_PUSH_GATEWAY_ERROR | Push gateway error | Warning | Buffer + Retry |
| E4010 | METRIC_TENSOR_ENCODING_ERROR | 12D encoding failed | Warning | Skip tensor |

---

## Navigation

| Direction | Link |
|-----------|------|
| Up | [INDEX.md](../INDEX.md) |
| Layer | [L01_FOUNDATION.md](../layers/L01_FOUNDATION.md) |
| Previous | [M03_LOGGING_SYSTEM.md](M03_LOGGING_SYSTEM.md) |
| Next | [M05_STATE_PERSISTENCE.md](M05_STATE_PERSISTENCE.md) |
| Related | [M06_RESOURCE_MANAGER.md](M06_RESOURCE_MANAGER.md) |

---

*[Back to Index](../INDEX.md) | [Layer: L01 Foundation](../layers/L01_FOUNDATION.md)*
