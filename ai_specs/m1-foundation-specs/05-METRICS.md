# M04 Metrics — metrics.rs

> **File:** `src/m1_foundation/metrics.rs` | **LOC:** ~1,280 | **Tests:** 12
> **Role:** Prometheus-compatible metrics (Counter, Gauge, Histogram) with label-based indexing

---

## MetricRecorder Trait

```rust
pub trait MetricRecorder: Send + Sync {
    fn increment_counter(&self, name: &str, labels: &Labels) -> Result<()>;
    fn set_gauge(&self, name: &str, value: f64, labels: &Labels) -> Result<()>;
    fn observe_histogram(&self, name: &str, value: f64, labels: &Labels) -> Result<()>;
    fn snapshot(&self) -> Result<MetricSnapshot>;
}
```

Concrete implementor: `MetricsRegistry`

---

## Labels (Fluent Builder)

```rust
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Labels { inner: Vec<(String, String)> }  // sorted for consistent hashing
```

```rust
Labels::new()
    .service("synthex")
    .layer("L1")
    .module("M04")
    .tier("1")
    .status("healthy")
    .agent("@0.A")           // NAM R5
    .with("custom", "value") // generic key-value
```

All methods are `#[must_use]`. `const fn new()`, `const fn is_empty()`.

---

## Metric Types

### Counter
```rust
pub fn inc(&self, labels: &Labels)
pub fn inc_by(&self, labels: &Labels, value: u64)
pub fn get(&self, labels: &Labels) -> u64           // #[must_use]
pub fn reset(&self, labels: &Labels)
```

### Gauge
```rust
pub fn set(&self, labels: &Labels, value: f64)
pub fn inc(&self, labels: &Labels)
pub fn dec(&self, labels: &Labels)
pub fn add(&self, labels: &Labels, delta: f64)
pub fn get(&self, labels: &Labels) -> f64           // #[must_use]
```

### Histogram
```rust
pub fn observe(&self, labels: &Labels, value: f64)
pub fn get_sum(&self, labels: &Labels) -> f64       // #[must_use]
pub fn get_count(&self, labels: &Labels) -> u64     // #[must_use]
pub fn get_buckets(&self, labels: &Labels) -> Vec<(f64, u64)>  // #[must_use]
```

---

## MetricsRegistry

```rust
pub fn new() -> Self
pub fn with_prefix(prefix: &str) -> Self
pub fn register_counter(name, help, labels) -> Result<Arc<Counter>>
pub fn register_gauge(name, help, labels) -> Result<Arc<Gauge>>
pub fn register_histogram(name, help, labels, buckets) -> Result<Arc<Histogram>>
pub fn register_histogram_default(name, help, labels) -> Result<Arc<Histogram>>
pub fn get_counter(name) -> Option<Arc<Counter>>
pub fn get_gauge(name) -> Option<Arc<Gauge>>
pub fn get_histogram(name) -> Option<Arc<Histogram>>
pub fn export_metrics(&self) -> String              // Prometheus text format
pub fn metric_count(&self) -> usize
pub fn list_metrics(&self) -> Vec<String>
pub fn snapshot(&self) -> MetricSnapshot
```

**Name validation:** must match `[a-zA-Z_:][a-zA-Z0-9_:]*`

---

## Concurrency Model

```rust
Counter.values:    RwLock<HashMap<Labels, AtomicU64>>
Gauge.values:      RwLock<HashMap<Labels, AtomicU64>>     // fixed-point scaling (GAUGE_SCALE=1e6)
Histogram.values:  RwLock<HashMap<Labels, Arc<HistogramData>>>

MetricsRegistry.counters:   RwLock<HashMap<String, Arc<Counter>>>
MetricsRegistry.gauges:     RwLock<HashMap<String, Arc<Gauge>>>
MetricsRegistry.histograms: RwLock<HashMap<String, Arc<Histogram>>>
```

**Lock upgrade pattern:**
1. Take read lock → if key found → operate atomically on AtomicU64 → drop lock
2. If not found → drop read lock → take write lock → insert entry → operate

All atomic operations use `Ordering::Relaxed`.

---

## Tensor Contribution

```rust
impl MetricSnapshot {
    pub fn to_tensor(&self) -> Tensor12D
    // D2 = 1.0/6.0 (L1 tier)
    // D6 = average gauge value (health proxy)
    // D10 = normalized error rate
}
```

---

## Constants

```rust
pub const DEFAULT_LATENCY_BUCKETS: [f64; 11] = [0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0];
pub const DEFAULT_SIZE_BUCKETS: [f64; 6] = [100.0, 1_000.0, 10_000.0, 100_000.0, 1_000_000.0, 10_000_000.0];
```

---

## Free Functions

```rust
pub fn create_registry() -> MetricsRegistry
pub fn create_maintenance_registry() -> MetricsRegistry  // pre-configured for ME
pub fn increment_counter(registry, name, labels) -> Result<()>
pub fn set_gauge(registry, name, value, labels) -> Result<()>
pub fn observe_histogram(registry, name, value, labels) -> Result<()>
pub fn export_metrics(registry) -> String
pub fn register_default_metrics(registry) -> Result<()>
pub fn snapshot_delta(prev, next) -> MetricDelta
```

---

## Clippy Allowances

- `cast_possible_truncation`, `cast_sign_loss`, `cast_precision_loss`, `cast_possible_wrap` — fixed-point arithmetic for gauge f64 storage
- `format_push_string` — Prometheus text format export

---

*M04 Metrics Spec v1.0 | 2026-03-01*
