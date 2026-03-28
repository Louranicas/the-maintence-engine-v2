//! # M04: Metrics Collector
//!
//! Real-time metrics collection and aggregation for the Maintenance Engine.
//! Supports Prometheus-compatible metrics with counters, gauges, and histograms.
//!
//! ## Layer: L1 (Foundation)
//! ## Dependencies: M01 (Error Taxonomy)
//! ## Tests: 27+ tests
//!
//! ## 12D Tensor Encoding
//! ```text
//! [4/36, 0.0, 1/6, 1, 0, 0.5, health, uptime, synergy, latency, error_rate, temporal]
//! ```
//!
//! ## Metric Types
//!
//! | Type | Description | Use Case |
//! |------|-------------|----------|
//! | Counter | Monotonically increasing | `requests_total`, `errors_total` |
//! | Gauge | Can increase or decrease | `active_connections`, `health_score` |
//! | Histogram | Value distribution | `request_duration_seconds` |
//!
//! ## Related Documentation
//! - [Module Specification](../../ai_docs/modules/M04_METRICS_COLLECTOR.md)
//! - [Observability Guide](../../ai_docs/diagnostics/OBSERVABILITY_GUIDE.md)

// Allow intentional fixed-point arithmetic casts
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_possible_wrap)]
// Allow format! in push_str for clarity
#![allow(clippy::format_push_string)]

use crate::{Error, Result};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

// ============================================================================
// CORE DATA STRUCTURES
// ============================================================================

/// Label key-value pairs for metric dimensions
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Labels {
    inner: Vec<(String, String)>,
}

impl Labels {
    /// Create new empty labels
    #[must_use]
    pub const fn new() -> Self {
        Self { inner: Vec::new() }
    }

    /// Add a label with builder pattern
    #[must_use]
    pub fn with<K: Into<String>, V: Into<String>>(mut self, key: K, value: V) -> Self {
        self.inner.push((key.into(), value.into()));
        self.inner.sort_by(|a, b| a.0.cmp(&b.0)); // Keep sorted for consistent hashing
        self
    }

    /// Add service label
    #[must_use]
    pub fn service<S: Into<String>>(self, service: S) -> Self {
        self.with("service", service)
    }

    /// Add layer label
    #[must_use]
    pub fn layer<L: Into<String>>(self, layer: L) -> Self {
        self.with("layer", layer)
    }

    /// Add module label
    #[must_use]
    pub fn module<M: Into<String>>(self, module: M) -> Self {
        self.with("module", module)
    }

    /// Add tier label
    #[must_use]
    pub fn tier<T: Into<String>>(self, tier: T) -> Self {
        self.with("tier", tier)
    }

    /// Add status label
    #[must_use]
    pub fn status<S: Into<String>>(self, status: S) -> Self {
        self.with("status", status)
    }

    /// Add agent identity label (NAM R5).
    #[must_use]
    pub fn agent<A: Into<String>>(self, agent_id: A) -> Self {
        self.with("agent", agent_id)
    }

    /// Create labels from a slice of tuples
    #[must_use]
    pub fn from_pairs(pairs: &[(&str, &str)]) -> Self {
        let mut labels = Self::new();
        for (k, v) in pairs {
            labels.inner.push(((*k).to_string(), (*v).to_string()));
        }
        labels.inner.sort_by(|a, b| a.0.cmp(&b.0));
        labels
    }

    /// Format labels for Prometheus output
    fn prometheus_format(&self) -> String {
        if self.inner.is_empty() {
            return String::new();
        }
        let pairs: Vec<String> = self
            .inner
            .iter()
            .map(|(k, v)| format!("{}=\"{}\"", k, escape_label_value(v)))
            .collect();
        format!("{{{}}}", pairs.join(","))
    }

    /// Check if labels are empty
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

/// Escape special characters in label values
fn escape_label_value(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}

// ============================================================================
// COUNTER METRIC
// ============================================================================

/// Counter metric (monotonically increasing value)
#[derive(Debug)]
pub struct Counter {
    /// Metric name
    name: String,
    /// Metric help text
    help: String,
    /// Label names for this counter (stored for validation and introspection)
    #[allow(dead_code)]
    label_names: Vec<String>,
    /// Counter values per label combination
    values: RwLock<HashMap<Labels, AtomicU64>>,
}

impl Counter {
    /// Create a new counter metric
    fn new(name: &str, help: &str, label_names: &[&str]) -> Self {
        Self {
            name: name.to_string(),
            help: help.to_string(),
            label_names: label_names.iter().map(|s| (*s).to_string()).collect(),
            values: RwLock::new(HashMap::new()),
        }
    }

    /// Increment counter by 1
    pub fn inc(&self, labels: &Labels) {
        self.inc_by(labels, 1);
    }

    /// Increment counter by a specific value
    pub fn inc_by(&self, labels: &Labels, value: u64) {
        let values = self.values.read();
        if let Some(counter) = values.get(labels) {
            counter.fetch_add(value, Ordering::Relaxed);
            return;
        }
        drop(values);

        let mut values = self.values.write();
        values
            .entry(labels.clone())
            .or_insert_with(|| AtomicU64::new(0))
            .fetch_add(value, Ordering::Relaxed);
    }

    /// Get current counter value
    #[must_use]
    pub fn get(&self, labels: &Labels) -> u64 {
        self.values
            .read()
            .get(labels)
            .map_or(0, |c| c.load(Ordering::Relaxed))
    }

    /// Reset counter (use with caution - typically counters should not be reset)
    pub fn reset(&self, labels: &Labels) {
        let values = self.values.read();
        if let Some(counter) = values.get(labels) {
            counter.store(0, Ordering::Relaxed);
        }
    }

    /// Export to Prometheus format
    fn export_prometheus(&self) -> String {
        let mut output = String::new();
        output.push_str(&format!("# HELP {} {}\n", self.name, self.help));
        output.push_str(&format!("# TYPE {} counter\n", self.name));

        {
            let values = self.values.read();
            for (labels, value) in values.iter() {
                output.push_str(&format!(
                    "{}{} {}\n",
                    self.name,
                    labels.prometheus_format(),
                    value.load(Ordering::Relaxed)
                ));
            }
        }
        output
    }
}

// ============================================================================
// GAUGE METRIC
// ============================================================================

/// Gauge metric (can increase or decrease)
/// Uses fixed-point representation for thread-safe f64 operations
#[derive(Debug)]
pub struct Gauge {
    /// Metric name
    name: String,
    /// Metric help text
    help: String,
    /// Label names for this gauge (stored for validation and introspection)
    #[allow(dead_code)]
    label_names: Vec<String>,
    /// Gauge values per label combination (stored as fixed-point i64: value * `1_000_000`)
    values: RwLock<HashMap<Labels, AtomicU64>>,
}

/// Fixed-point scaling factor for gauge values (6 decimal places)
const GAUGE_SCALE: f64 = 1_000_000.0;

impl Gauge {
    /// Create a new gauge metric
    fn new(name: &str, help: &str, label_names: &[&str]) -> Self {
        Self {
            name: name.to_string(),
            help: help.to_string(),
            label_names: label_names.iter().map(|s| (*s).to_string()).collect(),
            values: RwLock::new(HashMap::new()),
        }
    }

    /// Set gauge to a specific value
    pub fn set(&self, labels: &Labels, value: f64) {
        let scaled = (value * GAUGE_SCALE) as u64;
        let values = self.values.read();
        if let Some(gauge) = values.get(labels) {
            gauge.store(scaled, Ordering::Relaxed);
            return;
        }
        drop(values);

        let mut values = self.values.write();
        values
            .entry(labels.clone())
            .or_insert_with(|| AtomicU64::new(0))
            .store(scaled, Ordering::Relaxed);
    }

    /// Increment gauge by 1
    pub fn inc(&self, labels: &Labels) {
        self.add(labels, 1.0);
    }

    /// Decrement gauge by 1
    pub fn dec(&self, labels: &Labels) {
        self.add(labels, -1.0);
    }

    /// Add to gauge (can be negative)
    pub fn add(&self, labels: &Labels, delta: f64) {
        let scaled_delta = (delta * GAUGE_SCALE) as i64;
        let values = self.values.read();
        if let Some(gauge) = values.get(labels) {
            // Use wrapping add to handle the conversion properly
            let current = gauge.load(Ordering::Relaxed) as i64;
            let new_val = current.saturating_add(scaled_delta).max(0) as u64;
            gauge.store(new_val, Ordering::Relaxed);
            return;
        }
        drop(values);

        let mut values = self.values.write();
        let initial = if delta >= 0.0 {
            (delta * GAUGE_SCALE) as u64
        } else {
            0
        };
        values
            .entry(labels.clone())
            .or_insert_with(|| AtomicU64::new(initial));
    }

    /// Get current gauge value
    #[must_use]
    pub fn get(&self, labels: &Labels) -> f64 {
        self.values
            .read()
            .get(labels)
            .map_or(0.0, |g| g.load(Ordering::Relaxed) as f64 / GAUGE_SCALE)
    }

    /// Export to Prometheus format
    fn export_prometheus(&self) -> String {
        let mut output = String::new();
        output.push_str(&format!("# HELP {} {}\n", self.name, self.help));
        output.push_str(&format!("# TYPE {} gauge\n", self.name));

        {
            let values = self.values.read();
            for (labels, value) in values.iter() {
                let float_val = value.load(Ordering::Relaxed) as f64 / GAUGE_SCALE;
                output.push_str(&format!(
                    "{}{} {}\n",
                    self.name,
                    labels.prometheus_format(),
                    float_val
                ));
            }
        }
        output
    }
}

// ============================================================================
// HISTOGRAM METRIC
// ============================================================================

/// Histogram bucket data
#[derive(Debug)]
struct HistogramBucket {
    /// Bucket upper bound (le = less than or equal)
    le: f64,
    /// Count of observations in this bucket
    count: AtomicU64,
}

/// Histogram data for a single label combination
#[derive(Debug)]
struct HistogramData {
    /// Bucket counts (cumulative, includes +Inf)
    buckets: Vec<HistogramBucket>,
    /// Sum of all observed values (fixed-point)
    sum: AtomicU64,
    /// Total count of observations
    count: AtomicU64,
}

impl HistogramData {
    fn new(bucket_bounds: &[f64]) -> Self {
        let mut buckets: Vec<HistogramBucket> = bucket_bounds
            .iter()
            .map(|&le| HistogramBucket {
                le,
                count: AtomicU64::new(0),
            })
            .collect();
        // Add +Inf bucket
        buckets.push(HistogramBucket {
            le: f64::INFINITY,
            count: AtomicU64::new(0),
        });
        Self {
            buckets,
            sum: AtomicU64::new(0),
            count: AtomicU64::new(0),
        }
    }

    fn observe(&self, value: f64) {
        // Update buckets (cumulative)
        for bucket in &self.buckets {
            if value <= bucket.le {
                bucket.count.fetch_add(1, Ordering::Relaxed);
            }
        }
        // Update sum (fixed-point)
        let scaled_value = (value * GAUGE_SCALE) as u64;
        self.sum.fetch_add(scaled_value, Ordering::Relaxed);
        // Update count
        self.count.fetch_add(1, Ordering::Relaxed);
    }
}

/// Histogram metric (value distribution tracking)
#[derive(Debug)]
pub struct Histogram {
    /// Metric name
    name: String,
    /// Metric help text
    help: String,
    /// Label names for this histogram (stored for validation and introspection)
    #[allow(dead_code)]
    label_names: Vec<String>,
    /// Bucket boundaries
    bucket_bounds: Vec<f64>,
    /// Histogram data per label combination
    values: RwLock<HashMap<Labels, Arc<HistogramData>>>,
}

impl Histogram {
    /// Create a new histogram metric with specified buckets
    fn new(name: &str, help: &str, label_names: &[&str], buckets: &[f64]) -> Self {
        let mut sorted_buckets = buckets.to_vec();
        sorted_buckets.sort_by(f64::total_cmp);
        Self {
            name: name.to_string(),
            help: help.to_string(),
            label_names: label_names.iter().map(|s| (*s).to_string()).collect(),
            bucket_bounds: sorted_buckets,
            values: RwLock::new(HashMap::new()),
        }
    }

    /// Observe a value
    pub fn observe(&self, labels: &Labels, value: f64) {
        {
            let values = self.values.read();
            if let Some(data) = values.get(labels) {
                data.observe(value);
                return;
            }
        }

        let data = Arc::clone(
            self.values
                .write()
                .entry(labels.clone())
                .or_insert_with(|| Arc::new(HistogramData::new(&self.bucket_bounds))),
        );
        data.observe(value);
    }

    /// Get the sum of all observations
    #[must_use]
    pub fn get_sum(&self, labels: &Labels) -> f64 {
        self.values
            .read()
            .get(labels)
            .map_or(0.0, |d| d.sum.load(Ordering::Relaxed) as f64 / GAUGE_SCALE)
    }

    /// Get the count of all observations
    #[must_use]
    pub fn get_count(&self, labels: &Labels) -> u64 {
        self.values
            .read()
            .get(labels)
            .map_or(0, |d| d.count.load(Ordering::Relaxed))
    }

    /// Get bucket counts
    #[must_use]
    pub fn get_buckets(&self, labels: &Labels) -> Vec<(f64, u64)> {
        self.values.read().get(labels).map_or_else(Vec::new, |d| {
            d.buckets
                .iter()
                .map(|b| (b.le, b.count.load(Ordering::Relaxed)))
                .collect()
        })
    }

    /// Export to Prometheus format
    fn export_prometheus(&self) -> String {
        let mut output = String::new();
        output.push_str(&format!("# HELP {} {}\n", self.name, self.help));
        output.push_str(&format!("# TYPE {} histogram\n", self.name));

        {
            let values = self.values.read();
            for (labels, data) in values.iter() {
                // Export buckets
                for bucket in &data.buckets {
                    let le_str = if bucket.le.is_infinite() {
                        "+Inf".to_string()
                    } else {
                        format!("{}", bucket.le)
                    };
                    let bucket_labels = labels.clone().with("le", le_str);
                    output.push_str(&format!(
                        "{}_bucket{} {}\n",
                        self.name,
                        bucket_labels.prometheus_format(),
                        bucket.count.load(Ordering::Relaxed)
                    ));
                }
                // Export sum
                output.push_str(&format!(
                    "{}_sum{} {}\n",
                    self.name,
                    labels.prometheus_format(),
                    data.sum.load(Ordering::Relaxed) as f64 / GAUGE_SCALE
                ));
                // Export count
                output.push_str(&format!(
                    "{}_count{} {}\n",
                    self.name,
                    labels.prometheus_format(),
                    data.count.load(Ordering::Relaxed)
                ));
            }
        }
        output
    }
}

// ============================================================================
// METRICS REGISTRY
// ============================================================================

/// Central metrics registry for all metric types
#[derive(Debug)]
pub struct MetricsRegistry {
    /// Registered counters
    counters: RwLock<HashMap<String, Arc<Counter>>>,
    /// Registered gauges
    gauges: RwLock<HashMap<String, Arc<Gauge>>>,
    /// Registered histograms
    histograms: RwLock<HashMap<String, Arc<Histogram>>>,
    /// Registry name/prefix
    prefix: String,
}

impl Default for MetricsRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl MetricsRegistry {
    /// Create a new empty registry
    #[must_use]
    pub fn new() -> Self {
        Self {
            counters: RwLock::new(HashMap::new()),
            gauges: RwLock::new(HashMap::new()),
            histograms: RwLock::new(HashMap::new()),
            prefix: String::new(),
        }
    }

    /// Create a new registry with a prefix
    #[must_use]
    pub fn with_prefix(prefix: &str) -> Self {
        Self {
            counters: RwLock::new(HashMap::new()),
            gauges: RwLock::new(HashMap::new()),
            histograms: RwLock::new(HashMap::new()),
            prefix: prefix.to_string(),
        }
    }

    /// Get the full metric name with prefix
    fn full_name(&self, name: &str) -> String {
        if self.prefix.is_empty() {
            name.to_string()
        } else {
            format!("{}_{}", self.prefix, name)
        }
    }

    /// Register a new counter metric
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The metric name is invalid
    /// - A counter with the same name is already registered
    pub fn register_counter(
        &self,
        name: &str,
        help: &str,
        labels: &[&str],
    ) -> Result<Arc<Counter>> {
        let full_name = self.full_name(name);
        validate_metric_name(&full_name)?;

        let counter = Arc::new(Counter::new(&full_name, help, labels));
        let mut counters = self.counters.write();
        if counters.contains_key(&full_name) {
            return Err(Error::Validation(format!(
                "Counter '{full_name}' already registered"
            )));
        }
        counters.insert(full_name, Arc::clone(&counter));
        drop(counters);
        Ok(counter)
    }

    /// Get an existing counter by name
    #[must_use]
    pub fn get_counter(&self, name: &str) -> Option<Arc<Counter>> {
        let full_name = self.full_name(name);
        self.counters.read().get(&full_name).cloned()
    }

    /// Register a new gauge metric
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The metric name is invalid
    /// - A gauge with the same name is already registered
    pub fn register_gauge(&self, name: &str, help: &str, labels: &[&str]) -> Result<Arc<Gauge>> {
        let full_name = self.full_name(name);
        validate_metric_name(&full_name)?;

        let gauge = Arc::new(Gauge::new(&full_name, help, labels));
        let mut gauges = self.gauges.write();
        if gauges.contains_key(&full_name) {
            return Err(Error::Validation(format!(
                "Gauge '{full_name}' already registered"
            )));
        }
        gauges.insert(full_name, Arc::clone(&gauge));
        drop(gauges);
        Ok(gauge)
    }

    /// Get an existing gauge by name
    #[must_use]
    pub fn get_gauge(&self, name: &str) -> Option<Arc<Gauge>> {
        let full_name = self.full_name(name);
        self.gauges.read().get(&full_name).cloned()
    }

    /// Register a new histogram metric with custom buckets
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The metric name is invalid
    /// - A histogram with the same name is already registered
    pub fn register_histogram(
        &self,
        name: &str,
        help: &str,
        labels: &[&str],
        buckets: &[f64],
    ) -> Result<Arc<Histogram>> {
        let full_name = self.full_name(name);
        validate_metric_name(&full_name)?;

        let histogram = Arc::new(Histogram::new(&full_name, help, labels, buckets));
        let mut histograms = self.histograms.write();
        if histograms.contains_key(&full_name) {
            return Err(Error::Validation(format!(
                "Histogram '{full_name}' already registered"
            )));
        }
        histograms.insert(full_name, Arc::clone(&histogram));
        drop(histograms);
        Ok(histogram)
    }

    /// Register a histogram with default latency buckets
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The metric name is invalid
    /// - A histogram with the same name is already registered
    pub fn register_histogram_default(
        &self,
        name: &str,
        help: &str,
        labels: &[&str],
    ) -> Result<Arc<Histogram>> {
        self.register_histogram(name, help, labels, &DEFAULT_LATENCY_BUCKETS)
    }

    /// Get an existing histogram by name
    #[must_use]
    pub fn get_histogram(&self, name: &str) -> Option<Arc<Histogram>> {
        let full_name = self.full_name(name);
        self.histograms.read().get(&full_name).cloned()
    }

    /// Export all metrics in Prometheus text format
    #[must_use]
    pub fn export_metrics(&self) -> String {
        let mut output = String::new();

        // Export counters
        for counter in self.counters.read().values() {
            output.push_str(&counter.export_prometheus());
        }

        // Export gauges
        for gauge in self.gauges.read().values() {
            output.push_str(&gauge.export_prometheus());
        }

        // Export histograms
        for histogram in self.histograms.read().values() {
            output.push_str(&histogram.export_prometheus());
        }

        output
    }

    /// Get total number of registered metrics
    #[must_use]
    pub fn metric_count(&self) -> usize {
        self.counters.read().len() + self.gauges.read().len() + self.histograms.read().len()
    }

    /// List all registered metric names
    #[must_use]
    pub fn list_metrics(&self) -> Vec<String> {
        let mut names = Vec::new();
        names.extend(self.counters.read().keys().cloned());
        names.extend(self.gauges.read().keys().cloned());
        names.extend(self.histograms.read().keys().cloned());
        names.sort();
        names
    }
}

// ============================================================================
// METRIC RECORDER TRAIT
// ============================================================================

/// Trait for types that can record metrics.
///
/// Enables dependency inversion -- upper layers can accept `&dyn MetricRecorder`
/// instead of depending directly on [`MetricsRegistry`].
pub trait MetricRecorder: Send + Sync {
    /// Increment a counter by 1.
    ///
    /// # Errors
    ///
    /// Returns an error if the named counter is not registered.
    fn increment_counter(&self, name: &str, labels: &Labels) -> crate::Result<()>;

    /// Set a gauge value.
    ///
    /// # Errors
    ///
    /// Returns an error if the named gauge is not registered.
    fn set_gauge(&self, name: &str, value: f64, labels: &Labels) -> crate::Result<()>;

    /// Observe a histogram value.
    ///
    /// # Errors
    ///
    /// Returns an error if the named histogram is not registered.
    fn observe_histogram(&self, name: &str, value: f64, labels: &Labels) -> crate::Result<()>;

    /// Take a point-in-time snapshot of all metrics.
    ///
    /// # Errors
    ///
    /// Returns an error if snapshot collection fails.
    fn snapshot(&self) -> crate::Result<MetricSnapshot>;
}

impl MetricRecorder for MetricsRegistry {
    fn increment_counter(&self, name: &str, labels: &Labels) -> crate::Result<()> {
        let counter = self
            .get_counter(name)
            .ok_or_else(|| Error::Validation(format!("Counter '{name}' not found")))?;
        counter.inc(labels);
        Ok(())
    }

    fn set_gauge(&self, name: &str, value: f64, labels: &Labels) -> crate::Result<()> {
        let gauge = self
            .get_gauge(name)
            .ok_or_else(|| Error::Validation(format!("Gauge '{name}' not found")))?;
        gauge.set(labels, value);
        Ok(())
    }

    fn observe_histogram(&self, name: &str, value: f64, labels: &Labels) -> crate::Result<()> {
        let histogram = self
            .get_histogram(name)
            .ok_or_else(|| Error::Validation(format!("Histogram '{name}' not found")))?;
        histogram.observe(labels, value);
        Ok(())
    }

    fn snapshot(&self) -> crate::Result<MetricSnapshot> {
        Ok(Self::snapshot(self))
    }
}

// ============================================================================
// DEFAULT BUCKETS
// ============================================================================

/// Default latency buckets for request duration histograms (in seconds)
pub const DEFAULT_LATENCY_BUCKETS: [f64; 11] = [
    0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
];

/// Default size buckets (in bytes)
pub const DEFAULT_SIZE_BUCKETS: [f64; 6] = [
    100.0, 1_000.0, 10_000.0, 100_000.0, 1_000_000.0, 10_000_000.0,
];

// ============================================================================
// VALIDATION
// ============================================================================

/// Validate a metric name follows Prometheus naming conventions
fn validate_metric_name(name: &str) -> Result<()> {
    if name.is_empty() {
        return Err(Error::Validation("Metric name cannot be empty".to_string()));
    }

    // Must match [a-zA-Z_:][a-zA-Z0-9_:]*
    let first_char = name.chars().next();
    if let Some(c) = first_char {
        if !c.is_ascii_alphabetic() && c != '_' && c != ':' {
            return Err(Error::Validation(format!(
                "Metric name '{name}' must start with [a-zA-Z_:]"
            )));
        }
    }

    for c in name.chars() {
        if !c.is_ascii_alphanumeric() && c != '_' && c != ':' {
            return Err(Error::Validation(format!(
                "Metric name '{name}' contains invalid character '{c}'"
            )));
        }
    }

    Ok(())
}

// ============================================================================
// CONVENIENCE FUNCTIONS
// ============================================================================

/// Create a new metrics registry
#[must_use]
pub fn create_registry() -> MetricsRegistry {
    MetricsRegistry::new()
}

/// Create a new metrics registry with the Maintenance Engine prefix
#[must_use]
pub fn create_maintenance_registry() -> MetricsRegistry {
    MetricsRegistry::with_prefix("maintenance")
}

/// Increment a counter by name
///
/// # Errors
///
/// Returns an error if the counter is not found in the registry
pub fn increment_counter(
    registry: &MetricsRegistry,
    name: &str,
    labels: &[(&str, &str)],
) -> Result<()> {
    let counter = registry
        .get_counter(name)
        .ok_or_else(|| Error::Validation(format!("Counter '{name}' not found")))?;
    counter.inc(&Labels::from_pairs(labels));
    Ok(())
}

/// Set a gauge value by name
///
/// # Errors
///
/// Returns an error if the gauge is not found in the registry
pub fn set_gauge(
    registry: &MetricsRegistry,
    name: &str,
    value: f64,
    labels: &[(&str, &str)],
) -> Result<()> {
    let gauge = registry
        .get_gauge(name)
        .ok_or_else(|| Error::Validation(format!("Gauge '{name}' not found")))?;
    gauge.set(&Labels::from_pairs(labels), value);
    Ok(())
}

/// Observe a histogram value by name
///
/// # Errors
///
/// Returns an error if the histogram is not found in the registry
pub fn observe_histogram(
    registry: &MetricsRegistry,
    name: &str,
    value: f64,
    labels: &[(&str, &str)],
) -> Result<()> {
    let histogram = registry
        .get_histogram(name)
        .ok_or_else(|| Error::Validation(format!("Histogram '{name}' not found")))?;
    histogram.observe(&Labels::from_pairs(labels), value);
    Ok(())
}

/// Export all metrics from a registry
#[must_use]
pub fn export_metrics(registry: &MetricsRegistry) -> String {
    registry.export_metrics()
}

// ============================================================================
// DEFAULT MAINTENANCE ENGINE METRICS
// ============================================================================

/// Register default metrics for the Maintenance Engine
///
/// # Errors
///
/// Returns an error if any metric registration fails
pub fn register_default_metrics(registry: &MetricsRegistry) -> Result<()> {
    // Counters
    registry.register_counter(
        "requests_total",
        "Total number of maintenance requests processed",
        &["service", "endpoint", "method", "status"],
    )?;

    registry.register_counter(
        "errors_total",
        "Total number of errors encountered",
        &["service", "error_type", "severity"],
    )?;

    registry.register_counter(
        "consensus_votes_total",
        "Total PBFT consensus votes cast",
        &["node_id", "vote_type", "result"],
    )?;

    // Gauges
    registry.register_gauge(
        "active_connections",
        "Current number of active connections",
        &["service", "protocol"],
    )?;

    registry.register_gauge(
        "pathway_strength",
        "Current strength of neural pathways (0-1)",
        &["pathway_id", "source", "target"],
    )?;

    registry.register_gauge(
        "agent_status",
        "Current status of ULTRAPLATE agents (0=down, 1=degraded, 2=healthy)",
        &["agent_id", "role"],
    )?;

    registry.register_gauge(
        "health_score",
        "System health composite score (0-1)",
        &["component"],
    )?;

    registry.register_gauge(
        "synergy_score",
        "Module synergy score (0-1)",
        &["source", "target"],
    )?;

    // Histograms
    registry.register_histogram_default(
        "request_duration_seconds",
        "Request latency distribution",
        &["service", "endpoint"],
    )?;

    registry.register_histogram(
        "remediation_latency_seconds",
        "Time to complete remediation actions",
        &["action_type", "module"],
        &[0.5, 1.0, 2.5, 5.0, 10.0, 30.0, 60.0, 120.0, 300.0],
    )?;

    Ok(())
}

// ============================================================================
// METRIC SNAPSHOT
// ============================================================================

/// Point-in-time snapshot of metrics
#[derive(Clone, Debug, Default)]
pub struct MetricSnapshot {
    /// Timestamp (Unix epoch milliseconds)
    pub timestamp: u64,
    /// Counter values
    pub counters: HashMap<String, HashMap<String, u64>>,
    /// Gauge values
    pub gauges: HashMap<String, HashMap<String, f64>>,
    /// Histogram summaries
    pub histograms: HashMap<String, HistogramSummary>,
}

/// Histogram summary for snapshots
#[derive(Clone, Debug, Default)]
pub struct HistogramSummary {
    /// Total count of observations
    pub count: u64,
    /// Sum of all observations
    pub sum: f64,
    /// P50 estimate
    pub p50: f64,
    /// P95 estimate
    pub p95: f64,
    /// P99 estimate
    pub p99: f64,
}

// ============================================================================
// NAM: MetricDelta (R2 HebbianRouting — STDP timing correlation)
// ============================================================================

/// Delta between two metric snapshots (NAM R2 — STDP timing correlation).
///
/// Enables Hebbian learning by capturing the difference between two
/// measurement points, including timing information for STDP windows.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MetricDelta {
    /// Counter value changes (positive only for monotonic counters).
    pub counter_deltas: HashMap<String, u64>,
    /// Gauge value changes (can be negative).
    pub gauge_deltas: HashMap<String, f64>,
    /// Time elapsed between snapshots in milliseconds.
    pub duration_between: u64,
}

/// Compute the delta between two metric snapshots (NAM R2).
///
/// Returns the difference in counters and gauges between `prev` and `next`.
#[must_use]
pub fn snapshot_delta(prev: &MetricSnapshot, next: &MetricSnapshot) -> MetricDelta {
    let mut counter_deltas = HashMap::new();
    for (name, next_values) in &next.counters {
        if let Some(prev_values) = prev.counters.get(name) {
            for (labels, &next_val) in next_values {
                let prev_val = prev_values.get(labels).copied().unwrap_or(0);
                let delta = next_val.saturating_sub(prev_val);
                if delta > 0 {
                    counter_deltas.insert(format!("{name}{labels}"), delta);
                }
            }
        } else {
            // All new counter values are deltas from zero
            for (labels, &val) in next_values {
                if val > 0 {
                    counter_deltas.insert(format!("{name}{labels}"), val);
                }
            }
        }
    }

    let mut gauge_deltas = HashMap::new();
    for (name, next_values) in &next.gauges {
        if let Some(prev_values) = prev.gauges.get(name) {
            for (labels, &next_val) in next_values {
                let prev_val = prev_values.get(labels).copied().unwrap_or(0.0);
                let delta = next_val - prev_val;
                if delta.abs() > f64::EPSILON {
                    gauge_deltas.insert(format!("{name}{labels}"), delta);
                }
            }
        }
    }

    let duration_between = next.timestamp.saturating_sub(prev.timestamp);

    MetricDelta {
        counter_deltas,
        gauge_deltas,
        duration_between,
    }
}

impl MetricSnapshot {
    /// Encode this snapshot as a 12D tensor (NAM R4).
    ///
    /// D6 = average gauge values (health proxy), D10 = normalized error counters.
    #[must_use]
    pub fn to_tensor(&self) -> crate::Tensor12D {
        // Average all gauge values as a health proxy
        let gauge_sum: f64 = self
            .gauges
            .values()
            .flat_map(|m| m.values())
            .copied()
            .sum();
        let gauge_count = self
            .gauges
            .values()
            .map(HashMap::len)
            .sum::<usize>()
            .max(1);
        let avg_gauge = (gauge_sum / gauge_count as f64).clamp(0.0, 1.0);

        // Sum all counter values as error rate proxy (normalized to [0, 1])
        let counter_sum: u64 = self
            .counters
            .values()
            .flat_map(|m| m.values())
            .copied()
            .sum();
        let error_rate = (counter_sum as f64 / 10_000.0).clamp(0.0, 1.0);

        let mut tensor = crate::Tensor12D {
            service_id: 0.0,
            port: 0.0,
            tier: 1.0 / 6.0,
            dependency_count: 0.0,
            agent_count: 0.0,
            protocol: 0.5,
            health_score: avg_gauge,
            uptime: 0.0,
            synergy: 0.0,
            latency: 0.0,
            error_rate,
            temporal_context: 0.0,
        };
        tensor.clamp_normalize();
        tensor
    }
}

/// Get current timestamp in milliseconds
fn current_timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

impl MetricsRegistry {
    /// Create a point-in-time snapshot of all metrics
    #[must_use]
    pub fn snapshot(&self) -> MetricSnapshot {
        let mut snapshot = MetricSnapshot {
            timestamp: current_timestamp_ms(),
            ..Default::default()
        };

        // Snapshot counters
        for (name, counter) in self.counters.read().iter() {
            let mut counter_values = HashMap::new();
            for (labels, value) in counter.values.read().iter() {
                counter_values.insert(
                    labels.prometheus_format(),
                    value.load(Ordering::Relaxed),
                );
            }
            snapshot.counters.insert(name.clone(), counter_values);
        }

        // Snapshot gauges
        for (name, gauge) in self.gauges.read().iter() {
            let mut gauge_values = HashMap::new();
            for (labels, value) in gauge.values.read().iter() {
                gauge_values.insert(
                    labels.prometheus_format(),
                    value.load(Ordering::Relaxed) as f64 / GAUGE_SCALE,
                );
            }
            snapshot.gauges.insert(name.clone(), gauge_values);
        }

        // Snapshot histograms (simplified summary)
        for (name, histogram) in self.histograms.read().iter() {
            for (labels, data) in histogram.values.read().iter() {
                let summary = HistogramSummary {
                    count: data.count.load(Ordering::Relaxed),
                    sum: data.sum.load(Ordering::Relaxed) as f64 / GAUGE_SCALE,
                    p50: 0.0, // Would need interpolation logic
                    p95: 0.0,
                    p99: 0.0,
                };
                snapshot
                    .histograms
                    .insert(format!("{}{}", name, labels.prometheus_format()), summary);
            }
        }

        snapshot
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ====================================================================
    // Labels tests
    // ====================================================================

    #[test]
    fn test_labels_builder() {
        let labels = Labels::new()
            .service("synthex")
            .layer("L1")
            .module("M04")
            .status("healthy");

        let formatted = labels.prometheus_format();
        assert!(formatted.contains("service=\"synthex\""));
        assert!(formatted.contains("layer=\"L1\""));
        assert!(formatted.contains("module=\"M04\""));
        assert!(formatted.contains("status=\"healthy\""));
    }

    #[test]
    fn test_labels_from_pairs() {
        let labels = Labels::from_pairs(&[("method", "GET"), ("endpoint", "/health")]);
        let formatted = labels.prometheus_format();
        assert!(formatted.contains("method=\"GET\""));
        assert!(formatted.contains("endpoint=\"/health\""));
    }

    #[test]
    fn test_labels_empty() {
        let labels = Labels::new();
        assert!(labels.is_empty());
        assert_eq!(labels.prometheus_format(), "");
    }

    #[test]
    fn test_labels_special_characters() {
        let labels = Labels::new()
            .with("path", "/api/v1/test")
            .with("msg", "Line1\nLine2")
            .with("quote", "Say \"hello\"");

        let formatted = labels.prometheus_format();
        assert!(formatted.contains(r#"msg="Line1\nLine2""#));
        assert!(formatted.contains(r#"quote="Say \"hello\"""#));
    }

    #[test]
    fn test_labels_builder_chain_preserves_sort_order() {
        let labels = Labels::new()
            .with("z_key", "last")
            .with("a_key", "first")
            .with("m_key", "middle");

        let formatted = labels.prometheus_format();
        // Keys should be sorted alphabetically
        let a_pos = formatted.find("a_key");
        let m_pos = formatted.find("m_key");
        let z_pos = formatted.find("z_key");
        assert!(a_pos.is_some());
        assert!(m_pos.is_some());
        assert!(z_pos.is_some());
        if let (Some(a), Some(m), Some(z)) = (a_pos, m_pos, z_pos) {
            assert!(a < m);
            assert!(m < z);
        }
    }

    #[test]
    fn test_labels_from_pairs_empty() {
        let labels = Labels::from_pairs(&[]);
        assert!(labels.is_empty());
        assert_eq!(labels.prometheus_format(), "");
    }

    // ====================================================================
    // Counter tests
    // ====================================================================

    #[test]
    fn test_counter_inc() -> crate::Result<()> {
        let registry = create_registry();
        let counter = registry.register_counter("test_inc", "Test inc", &["label"])?;
        let labels = Labels::new().with("label", "value");

        assert_eq!(counter.get(&labels), 0);
        counter.inc(&labels);
        assert_eq!(counter.get(&labels), 1);
        counter.inc(&labels);
        assert_eq!(counter.get(&labels), 2);
        Ok(())
    }

    #[test]
    fn test_counter_inc_by() -> crate::Result<()> {
        let registry = create_registry();
        let counter = registry.register_counter("test_inc_by", "Test inc_by", &["label"])?;
        let labels = Labels::new().with("label", "value");

        counter.inc_by(&labels, 10);
        assert_eq!(counter.get(&labels), 10);
        counter.inc_by(&labels, 5);
        assert_eq!(counter.get(&labels), 15);
        Ok(())
    }

    #[test]
    fn test_counter_get_unregistered_labels() -> crate::Result<()> {
        let registry = create_registry();
        let counter = registry.register_counter("test_get", "Test get", &["label"])?;
        let labels = Labels::new().with("label", "nonexistent");

        // Getting a counter that has never been incremented returns 0
        assert_eq!(counter.get(&labels), 0);
        Ok(())
    }

    #[test]
    fn test_counter_reset() -> crate::Result<()> {
        let registry = create_registry();
        let counter = registry.register_counter("test_reset", "Test reset", &["label"])?;
        let labels = Labels::new().with("label", "value");

        counter.inc_by(&labels, 42);
        assert_eq!(counter.get(&labels), 42);
        counter.reset(&labels);
        assert_eq!(counter.get(&labels), 0);
        Ok(())
    }

    #[test]
    fn test_counter_overflow_behavior() -> crate::Result<()> {
        let registry = create_registry();
        let counter = registry.register_counter("test_overflow", "Test overflow", &[])?;
        let labels = Labels::new();

        // Increment by a large value near max
        counter.inc_by(&labels, u64::MAX - 1);
        assert_eq!(counter.get(&labels), u64::MAX - 1);

        // Wrapping addition: (u64::MAX - 1) + 2 wraps around
        counter.inc_by(&labels, 2);
        // AtomicU64::fetch_add wraps, so the result wraps to 0
        assert_eq!(counter.get(&labels), 0);
        Ok(())
    }

    // ====================================================================
    // Gauge tests
    // ====================================================================

    #[test]
    fn test_gauge_set() -> crate::Result<()> {
        let registry = create_registry();
        let gauge = registry.register_gauge("test_gauge_set", "Test set", &["label"])?;
        let labels = Labels::new().with("label", "value");

        assert!((gauge.get(&labels) - 0.0).abs() < f64::EPSILON);
        gauge.set(&labels, 0.95);
        assert!((gauge.get(&labels) - 0.95).abs() < 0.001);
        Ok(())
    }

    #[test]
    fn test_gauge_inc_dec() -> crate::Result<()> {
        let registry = create_registry();
        let gauge = registry.register_gauge("test_gauge_id", "Test inc/dec", &["label"])?;
        let labels = Labels::new().with("label", "value");

        gauge.set(&labels, 5.0);
        gauge.inc(&labels);
        assert!((gauge.get(&labels) - 6.0).abs() < 0.001);
        gauge.dec(&labels);
        assert!((gauge.get(&labels) - 5.0).abs() < 0.001);
        Ok(())
    }

    #[test]
    fn test_gauge_negative_values() -> crate::Result<()> {
        let registry = create_registry();
        let gauge = registry.register_gauge("test_gauge_neg", "Test negative", &[])?;
        let labels = Labels::new();

        // Start at 2.0 and subtract 3.0 -- should clamp to 0
        gauge.set(&labels, 2.0);
        gauge.add(&labels, -3.0);
        // Internal representation clamps to 0 for u64 storage
        assert!((gauge.get(&labels) - 0.0).abs() < 0.001);
        Ok(())
    }

    #[test]
    fn test_gauge_multiple_labels() -> crate::Result<()> {
        let registry = create_registry();
        let gauge = registry.register_gauge("test_gauge_ml", "Test multiple labels", &["a", "b"])?;
        let labels_a = Labels::new().with("a", "1").with("b", "2");
        let labels_b = Labels::new().with("a", "3").with("b", "4");

        gauge.set(&labels_a, 1.0);
        gauge.set(&labels_b, 2.0);
        assert!((gauge.get(&labels_a) - 1.0).abs() < 0.001);
        assert!((gauge.get(&labels_b) - 2.0).abs() < 0.001);
        Ok(())
    }

    // ====================================================================
    // Histogram tests
    // ====================================================================

    #[test]
    fn test_histogram_observe() -> crate::Result<()> {
        let registry = create_registry();
        let histogram = registry.register_histogram(
            "test_histo_obs",
            "Test observe",
            &["endpoint"],
            &[0.1, 0.5, 1.0, 5.0],
        )?;
        let labels = Labels::new().with("endpoint", "/api");

        histogram.observe(&labels, 0.05);
        histogram.observe(&labels, 0.3);
        histogram.observe(&labels, 0.8);
        histogram.observe(&labels, 2.0);

        assert_eq!(histogram.get_count(&labels), 4);
        assert!((histogram.get_sum(&labels) - 3.15).abs() < 0.01);
        Ok(())
    }

    #[test]
    fn test_histogram_bucket_boundaries() -> crate::Result<()> {
        let registry = create_registry();
        let histogram = registry.register_histogram(
            "test_histo_bkt",
            "Test buckets",
            &[],
            &[1.0, 5.0, 10.0],
        )?;
        let labels = Labels::new();

        histogram.observe(&labels, 0.5); // le=1.0, le=5.0, le=10.0, le=+Inf
        histogram.observe(&labels, 3.0); // le=5.0, le=10.0, le=+Inf
        histogram.observe(&labels, 7.0); // le=10.0, le=+Inf
        histogram.observe(&labels, 15.0); // le=+Inf only

        let buckets = histogram.get_buckets(&labels);
        assert_eq!(buckets.len(), 4); // 3 explicit + +Inf
        // le=1.0: 1 observation
        assert_eq!(buckets[0].1, 1);
        // le=5.0: 2 observations
        assert_eq!(buckets[1].1, 2);
        // le=10.0: 3 observations
        assert_eq!(buckets[2].1, 3);
        // le=+Inf: 4 observations
        assert_eq!(buckets[3].1, 4);
        Ok(())
    }

    #[test]
    fn test_histogram_summary_statistics() -> crate::Result<()> {
        let registry = create_registry();
        let histogram = registry.register_histogram(
            "test_histo_sum",
            "Test summary",
            &[],
            &[0.1, 0.5, 1.0],
        )?;
        let labels = Labels::new();

        histogram.observe(&labels, 0.2);
        histogram.observe(&labels, 0.4);

        assert_eq!(histogram.get_count(&labels), 2);
        assert!((histogram.get_sum(&labels) - 0.6).abs() < 0.001);

        // Unobserved labels return zero
        let empty_labels = Labels::new().with("x", "y");
        assert_eq!(histogram.get_count(&empty_labels), 0);
        assert!((histogram.get_sum(&empty_labels) - 0.0).abs() < f64::EPSILON);
        Ok(())
    }

    #[test]
    fn test_histogram_unsorted_buckets() -> crate::Result<()> {
        let registry = create_registry();
        // Provide buckets out of order -- they should be sorted by total_cmp
        let histogram = registry.register_histogram(
            "test_histo_unsorted",
            "Test unsorted",
            &[],
            &[10.0, 1.0, 5.0],
        )?;
        let labels = Labels::new();

        histogram.observe(&labels, 3.0);
        let buckets = histogram.get_buckets(&labels);
        // Sorted: 1.0, 5.0, 10.0, +Inf
        assert_eq!(buckets.len(), 4);
        assert!((buckets[0].0 - 1.0).abs() < f64::EPSILON);
        assert!((buckets[1].0 - 5.0).abs() < f64::EPSILON);
        assert!((buckets[2].0 - 10.0).abs() < f64::EPSILON);
        assert!(buckets[3].0.is_infinite());
        Ok(())
    }

    // ====================================================================
    // Prometheus export tests
    // ====================================================================

    #[test]
    fn test_prometheus_counter_export() -> crate::Result<()> {
        let registry = create_registry();
        let counter = registry.register_counter("requests_total", "Total requests", &["status"])?;
        counter.inc(&Labels::new().with("status", "200"));
        counter.inc(&Labels::new().with("status", "200"));

        let output = registry.export_metrics();
        assert!(output.contains("# HELP requests_total Total requests"));
        assert!(output.contains("# TYPE requests_total counter"));
        assert!(output.contains("requests_total{status=\"200\"} 2"));
        Ok(())
    }

    #[test]
    fn test_prometheus_gauge_export() -> crate::Result<()> {
        let registry = create_registry();
        let gauge = registry.register_gauge("active_connections", "Active connections", &[])?;
        gauge.set(&Labels::new(), 42.0);

        let output = registry.export_metrics();
        assert!(output.contains("# HELP active_connections Active connections"));
        assert!(output.contains("# TYPE active_connections gauge"));
        assert!(output.contains("active_connections 42"));
        Ok(())
    }

    #[test]
    fn test_prometheus_histogram_export() -> crate::Result<()> {
        let registry = create_registry();
        let histogram = registry.register_histogram(
            "duration_seconds",
            "Duration",
            &[],
            &[0.1, 0.5],
        )?;
        histogram.observe(&Labels::new(), 0.3);

        let output = registry.export_metrics();
        assert!(output.contains("# HELP duration_seconds Duration"));
        assert!(output.contains("# TYPE duration_seconds histogram"));
        assert!(output.contains("duration_seconds_bucket"));
        assert!(output.contains("duration_seconds_sum"));
        assert!(output.contains("duration_seconds_count"));
        Ok(())
    }

    // ====================================================================
    // MetricRecorder trait tests
    // ====================================================================

    #[test]
    fn test_metric_recorder_increment_counter() -> crate::Result<()> {
        let registry = create_registry();
        registry.register_counter("recorder_counter", "Recorder test", &["label"])?;

        let recorder: &dyn MetricRecorder = &registry;
        let labels = Labels::new().with("label", "value");

        recorder.increment_counter("recorder_counter", &labels)?;
        recorder.increment_counter("recorder_counter", &labels)?;

        if let Some(counter) = registry.get_counter("recorder_counter") {
            assert_eq!(counter.get(&labels), 2);
        }
        Ok(())
    }

    #[test]
    fn test_metric_recorder_set_gauge() -> crate::Result<()> {
        let registry = create_registry();
        registry.register_gauge("recorder_gauge", "Recorder test", &[])?;

        let recorder: &dyn MetricRecorder = &registry;
        let labels = Labels::new();

        recorder.set_gauge("recorder_gauge", 0.75, &labels)?;

        if let Some(gauge) = registry.get_gauge("recorder_gauge") {
            assert!((gauge.get(&labels) - 0.75).abs() < 0.001);
        }
        Ok(())
    }

    #[test]
    fn test_metric_recorder_observe_histogram() -> crate::Result<()> {
        let registry = create_registry();
        registry.register_histogram(
            "recorder_histogram",
            "Recorder test",
            &[],
            &[0.1, 1.0],
        )?;

        let recorder: &dyn MetricRecorder = &registry;
        let labels = Labels::new();

        recorder.observe_histogram("recorder_histogram", 0.5, &labels)?;

        if let Some(histogram) = registry.get_histogram("recorder_histogram") {
            assert_eq!(histogram.get_count(&labels), 1);
        }
        Ok(())
    }

    #[test]
    fn test_metric_recorder_not_found_errors() {
        let registry = create_registry();
        let recorder: &dyn MetricRecorder = &registry;
        let labels = Labels::new();

        assert!(recorder.increment_counter("nonexistent", &labels).is_err());
        assert!(recorder.set_gauge("nonexistent", 1.0, &labels).is_err());
        assert!(recorder.observe_histogram("nonexistent", 1.0, &labels).is_err());
    }

    #[test]
    fn test_metric_recorder_snapshot() -> crate::Result<()> {
        let registry = create_registry();
        registry.register_counter("snap_counter", "Snapshot test", &[])?;

        let recorder: &dyn MetricRecorder = &registry;
        let labels = Labels::new();
        recorder.increment_counter("snap_counter", &labels)?;

        let snapshot = recorder.snapshot()?;
        assert!(snapshot.timestamp > 0);
        assert!(!snapshot.counters.is_empty());
        Ok(())
    }

    // ====================================================================
    // MetricsRegistry tests
    // ====================================================================

    #[test]
    fn test_registry_list_metrics() -> crate::Result<()> {
        let registry = create_registry();
        registry.register_counter("alpha_counter", "Alpha", &[])?;
        registry.register_gauge("beta_gauge", "Beta", &[])?;
        registry.register_histogram("gamma_histogram", "Gamma", &[], &[1.0])?;

        let names = registry.list_metrics();
        assert_eq!(names.len(), 3);
        // Should be sorted alphabetically
        assert_eq!(names[0], "alpha_counter");
        assert_eq!(names[1], "beta_gauge");
        assert_eq!(names[2], "gamma_histogram");
        Ok(())
    }

    #[test]
    fn test_registry_metric_count() -> crate::Result<()> {
        let registry = create_registry();
        assert_eq!(registry.metric_count(), 0);

        registry.register_counter("c1", "Counter 1", &[])?;
        assert_eq!(registry.metric_count(), 1);

        registry.register_gauge("g1", "Gauge 1", &[])?;
        assert_eq!(registry.metric_count(), 2);

        registry.register_histogram("h1", "Histogram 1", &[], &[1.0])?;
        assert_eq!(registry.metric_count(), 3);
        Ok(())
    }

    // ====================================================================
    // Validation tests
    // ====================================================================

    #[test]
    fn test_metric_validation() {
        assert!(validate_metric_name("valid_metric").is_ok());
        assert!(validate_metric_name("valid:metric").is_ok());
        assert!(validate_metric_name("_valid_metric").is_ok());
        assert!(validate_metric_name("Valid123").is_ok());

        assert!(validate_metric_name("").is_err());
        assert!(validate_metric_name("123invalid").is_err());
        assert!(validate_metric_name("invalid-metric").is_err());
        assert!(validate_metric_name("invalid.metric").is_err());
    }

    // ====================================================================
    // Registry with prefix
    // ====================================================================

    #[test]
    fn test_registry_with_prefix() -> crate::Result<()> {
        let registry = MetricsRegistry::with_prefix("maintenance");
        registry.register_counter("requests_total", "Total requests", &[])?;

        let output = registry.export_metrics();
        assert!(output.contains("maintenance_requests_total"));
        Ok(())
    }

    // ====================================================================
    // Default metrics registration
    // ====================================================================

    #[test]
    fn test_default_metrics_registration() {
        let registry = create_maintenance_registry();
        let result = register_default_metrics(&registry);
        assert!(result.is_ok());

        // Verify key metrics are registered
        assert!(registry.get_counter("requests_total").is_some());
        assert!(registry.get_counter("errors_total").is_some());
        assert!(registry.get_counter("consensus_votes_total").is_some());
        assert!(registry.get_gauge("active_connections").is_some());
        assert!(registry.get_gauge("pathway_strength").is_some());
        assert!(registry.get_gauge("health_score").is_some());
        assert!(registry.get_histogram("request_duration_seconds").is_some());
        assert!(registry.get_histogram("remediation_latency_seconds").is_some());
    }

    // ====================================================================
    // Snapshot
    // ====================================================================

    #[test]
    fn test_metric_snapshot() -> crate::Result<()> {
        let registry = create_registry();
        registry.register_counter("test_counter", "Test", &[])?;
        registry.register_gauge("test_gauge", "Test", &[])?;

        if let Some(counter) = registry.get_counter("test_counter") {
            counter.inc(&Labels::new());
        }
        if let Some(gauge) = registry.get_gauge("test_gauge") {
            gauge.set(&Labels::new(), 0.5);
        }

        let snapshot = registry.snapshot();
        assert!(snapshot.timestamp > 0);
        assert!(!snapshot.counters.is_empty());
        assert!(!snapshot.gauges.is_empty());
        Ok(())
    }

    // ====================================================================
    // Convenience functions
    // ====================================================================

    #[test]
    fn test_convenience_functions() -> crate::Result<()> {
        let registry = create_registry();
        registry.register_counter("test_counter", "Test", &["label"])?;
        registry.register_gauge("test_gauge", "Test", &["label"])?;
        registry.register_histogram_default("test_histogram", "Test", &["label"])?;

        // Test convenience functions
        increment_counter(&registry, "test_counter", &[("label", "value")])?;
        set_gauge(&registry, "test_gauge", 0.75, &[("label", "value")])?;
        observe_histogram(&registry, "test_histogram", 0.123, &[("label", "value")])?;

        // Verify values
        if let Some(counter) = registry.get_counter("test_counter") {
            assert_eq!(counter.get(&Labels::new().with("label", "value")), 1);
        }
        if let Some(gauge) = registry.get_gauge("test_gauge") {
            assert!((gauge.get(&Labels::new().with("label", "value")) - 0.75).abs() < 0.001);
        }
        Ok(())
    }

    // ====================================================================
    // Duplicate registration
    // ====================================================================

    #[test]
    fn test_duplicate_metric_registration() -> crate::Result<()> {
        let registry = create_registry();
        registry.register_counter("duplicate", "First", &[])?;

        let result2 = registry.register_counter("duplicate", "Second", &[]);
        assert!(result2.is_err());
        Ok(())
    }

    // ====================================================================
    // Thread safety
    // ====================================================================

    #[test]
    fn test_thread_safety() -> crate::Result<()> {
        use std::thread;

        let registry = Arc::new(create_registry());
        registry.register_counter("concurrent_counter", "Test", &["thread"])?;

        let mut handles = vec![];

        for i in 0..4 {
            let r = Arc::clone(&registry);
            handles.push(thread::spawn(move || {
                if let Some(counter) = r.get_counter("concurrent_counter") {
                    for _ in 0..100 {
                        counter.inc(&Labels::new().with("thread", i.to_string()));
                    }
                }
            }));
        }

        for handle in handles {
            handle.join().ok();
        }

        // Each thread incremented 100 times, 4 threads = 400 total (split across labels)
        if let Some(counter) = registry.get_counter("concurrent_counter") {
            let mut total = 0u64;
            for i in 0..4 {
                total += counter.get(&Labels::new().with("thread", i.to_string()));
            }
            assert_eq!(total, 400);
        }
        Ok(())
    }

    // ====================================================================
    // NAM: Labels::agent() tests
    // ====================================================================

    #[test]
    fn test_labels_agent_adds_key() {
        let labels = Labels::new().agent("@0.A");
        let debug = format!("{labels:?}");
        assert!(debug.contains("agent"));
        assert!(debug.contains("@0.A"));
    }

    #[test]
    fn test_labels_agent_chains_with_service() {
        let labels = Labels::new().service("me").agent("bot-1");
        let debug = format!("{labels:?}");
        assert!(debug.contains("agent"));
        assert!(debug.contains("service"));
    }

    #[test]
    fn test_labels_agent_sorts_correctly() {
        // "agent" < "service" alphabetically
        let labels = Labels::new().service("me").agent("bot");
        let prom = labels.prometheus_format();
        // agent should appear before service
        let agent_pos = prom.find("agent").expect("agent label");
        let service_pos = prom.find("service").expect("service label");
        assert!(
            agent_pos < service_pos,
            "agent should sort before service"
        );
    }

    // ====================================================================
    // NAM: snapshot_delta() tests
    // ====================================================================

    #[test]
    fn test_snapshot_delta_empty_snapshots() {
        let prev = MetricSnapshot::default();
        let next = MetricSnapshot::default();
        let delta = snapshot_delta(&prev, &next);
        assert!(delta.counter_deltas.is_empty());
        assert!(delta.gauge_deltas.is_empty());
        assert_eq!(delta.duration_between, 0);
    }

    #[test]
    fn test_snapshot_delta_counter_changes() {
        let mut prev = MetricSnapshot::default();
        prev.timestamp = 1000;
        let mut prev_values = HashMap::new();
        prev_values.insert(String::new(), 10);
        prev.counters.insert("requests".to_string(), prev_values);

        let mut next = MetricSnapshot::default();
        next.timestamp = 2000;
        let mut next_values = HashMap::new();
        next_values.insert(String::new(), 25);
        next.counters.insert("requests".to_string(), next_values);

        let delta = snapshot_delta(&prev, &next);
        assert_eq!(delta.counter_deltas.get("requests"), Some(&15));
        assert_eq!(delta.duration_between, 1000);
    }

    #[test]
    fn test_snapshot_delta_gauge_changes() {
        let mut prev = MetricSnapshot::default();
        let mut prev_gauges = HashMap::new();
        prev_gauges.insert(String::new(), 0.5);
        prev.gauges.insert("health".to_string(), prev_gauges);

        let mut next = MetricSnapshot::default();
        let mut next_gauges = HashMap::new();
        next_gauges.insert(String::new(), 0.9);
        next.gauges.insert("health".to_string(), next_gauges);

        let delta = snapshot_delta(&prev, &next);
        let health_delta = delta.gauge_deltas.get("health").copied().unwrap_or(0.0);
        assert!((health_delta - 0.4).abs() < 0.001);
    }

    #[test]
    fn test_snapshot_delta_new_counter() {
        let prev = MetricSnapshot::default();
        let mut next = MetricSnapshot::default();
        let mut next_values = HashMap::new();
        next_values.insert(String::new(), 5);
        next.counters.insert("new_counter".to_string(), next_values);

        let delta = snapshot_delta(&prev, &next);
        assert_eq!(delta.counter_deltas.get("new_counter"), Some(&5));
    }

    // ====================================================================
    // NAM: MetricDelta tests
    // ====================================================================

    #[test]
    fn test_metric_delta_default() {
        let delta = MetricDelta::default();
        assert!(delta.counter_deltas.is_empty());
        assert!(delta.gauge_deltas.is_empty());
        assert_eq!(delta.duration_between, 0);
    }

    // ====================================================================
    // NAM: MetricSnapshot::to_tensor() tests
    // ====================================================================

    #[test]
    fn test_metric_snapshot_to_tensor_valid() {
        let snapshot = MetricSnapshot::default();
        let tensor = snapshot.to_tensor();
        assert!(tensor.validate().is_ok());
    }

    #[test]
    fn test_metric_snapshot_to_tensor_with_gauges() {
        let mut snapshot = MetricSnapshot::default();
        let mut gauge_values = HashMap::new();
        gauge_values.insert(String::new(), 0.8);
        snapshot.gauges.insert("health".to_string(), gauge_values);

        let tensor = snapshot.to_tensor();
        assert!(tensor.validate().is_ok());
        assert!((tensor.health_score - 0.8).abs() < 0.001);
    }

    // ====================================================================
    // Edge case tests — empty label names, histogram no obs, delta no prior
    // ====================================================================

    #[test]
    fn test_labels_with_empty_key_and_value() {
        let labels = Labels::new().with("", "");
        let formatted = labels.prometheus_format();
        // Empty key/value still formatted as a label pair
        assert!(formatted.contains("=\"\""));
        assert!(!labels.is_empty());
    }

    #[test]
    fn test_histogram_no_observations_returns_zero() -> crate::Result<()> {
        let registry = create_registry();
        let histogram = registry.register_histogram(
            "test_no_obs",
            "No observations",
            &["endpoint"],
            &[0.1, 0.5, 1.0],
        )?;
        let labels = Labels::new().with("endpoint", "/test");

        // No observations recorded — count and sum should be zero
        assert_eq!(histogram.get_count(&labels), 0);
        assert!((histogram.get_sum(&labels) - 0.0).abs() < f64::EPSILON);

        // get_buckets returns empty vec for unseen labels
        let buckets = histogram.get_buckets(&labels);
        assert!(buckets.is_empty());
        Ok(())
    }

    #[test]
    fn test_snapshot_delta_with_no_prior_snapshot() {
        // prev has no counters at all; next has some — all treated as new deltas
        let prev = MetricSnapshot {
            timestamp: 100,
            ..Default::default()
        };
        let mut next = MetricSnapshot {
            timestamp: 200,
            ..Default::default()
        };
        let mut counter_vals = HashMap::new();
        counter_vals.insert(String::new(), 42);
        next.counters
            .insert("brand_new_counter".to_string(), counter_vals);

        let mut gauge_vals = HashMap::new();
        gauge_vals.insert(String::new(), 0.75);
        next.gauges
            .insert("brand_new_gauge".to_string(), gauge_vals);

        let delta = snapshot_delta(&prev, &next);
        // Counter delta should be 42 (all new)
        assert_eq!(delta.counter_deltas.get("brand_new_counter"), Some(&42));
        // Gauge delta: prev has no matching gauge, so gauge_deltas should be empty
        // (the function only computes gauge deltas when both prev and next have the key)
        assert!(delta.gauge_deltas.is_empty());
        assert_eq!(delta.duration_between, 100);
    }

    #[test]
    fn test_concurrent_gauge_updates() -> crate::Result<()> {
        use std::thread;

        let registry = Arc::new(create_registry());
        registry.register_gauge("concurrent_gauge", "Test concurrent gauge", &["worker"])?;

        let mut handles = vec![];
        for i in 0..4 {
            let r = Arc::clone(&registry);
            handles.push(thread::spawn(move || {
                if let Some(gauge) = r.get_gauge("concurrent_gauge") {
                    let label = Labels::new().with("worker", i.to_string());
                    gauge.set(&label, f64::from(i) * 10.0);
                    gauge.inc(&label);
                    gauge.dec(&label);
                    // Final value should be i * 10.0
                }
            }));
        }

        for handle in handles {
            handle.join().ok();
        }

        if let Some(gauge) = registry.get_gauge("concurrent_gauge") {
            for i in 0..4 {
                let label = Labels::new().with("worker", i.to_string());
                let val = gauge.get(&label);
                assert!(
                    (val - f64::from(i) * 10.0).abs() < 0.01,
                    "Worker {i} gauge should be {} but was {val}",
                    f64::from(i) * 10.0
                );
            }
        }
        Ok(())
    }

    #[test]
    fn test_registry_duplicate_gauge_and_histogram_errors() -> crate::Result<()> {
        let registry = create_registry();
        registry.register_gauge("dup_gauge", "First", &[])?;
        let result = registry.register_gauge("dup_gauge", "Second", &[]);
        assert!(result.is_err());

        registry.register_histogram("dup_hist", "First", &[], &[1.0])?;
        let result = registry.register_histogram("dup_hist", "Second", &[], &[1.0]);
        assert!(result.is_err());
        Ok(())
    }
}
