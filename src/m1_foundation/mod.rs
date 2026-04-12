//! # Layer 1: Foundation
//!
//! Core foundation modules providing error handling, configuration,
//! logging, metrics, state persistence, and resource management infrastructure.
//!
//! ## Modules
//!
//! | Module | Name | Purpose |
//! |--------|------|---------|
//! | M00 | Shared Types | Pure vocabulary types (zero logic/IO) |
//! | M1 | Error Taxonomy | Unified error types |
//! | M2 | Configuration | Config management |
//! | M3 | Logging | Structured logging |
//! | M4 | Metrics | Prometheus metrics |
//! | M5 | State Persistence | `SQLite` database operations |
//! | M6 | Resource Manager | System resource monitoring |
//! | M7 | Signal Bus | Typed synchronous signal bus |
//! | M8 | Tensor Registry | Coverage-aware tensor composition |
//!
//! ## 12D Tensor Encoding
//!
//! Layer 1 modules encode as:
//! ```text
//! [module_id/36, 0.0, 1/6, 0, 0, 0.5, health, uptime, synergy, latency, error_rate, temporal]
//! ```
//!
//! ## Traits (Dependency Inversion)
//!
//! | Trait | Module | Purpose |
//! |-------|--------|---------|
//! | [`ErrorClassifier`] | M1 | Classify errors by retryability, transience, severity |
//! | [`ConfigProvider`] | M2 | Abstract configuration access |
//! | [`CorrelationProvider`] | M3 | Distributed tracing context propagation |
//! | [`MetricRecorder`] | M4 | Record counters, gauges, histograms |
//! | [`StateStore`] | M5 | Abstract database pool access |
//! | [`ResourceCollector`] | M6 | Collect and report system resources |
//! | [`SignalSubscriber`] | M7 | Receive health/learning/dissent signals |
//! | [`TensorContributor`] | M8 | Contribute tensor data with coverage |
//!
//! ## PAI (Publicly Accessible Interface)
//!
//! All public types from M00–M08 plus NAM primitives are re-exported through
//! this module (~125 items). Every `pub` item is accessible via
//! `crate::m1_foundation::ItemName` — no submodule import paths required.
//!
//! ## Related Documentation
//! - [Layer Specification](../../ai_docs/layers/L01_FOUNDATION.md)
//! - [Error Handling Pattern](../../ai_docs/patterns/PATTERN_001_ERROR_HANDLING.md)
//! - [PAI Audit](../../ai_docs/code-base-overview/m1-foundation-overview/L1_PAI_AUDIT.md)

mod error;
pub mod config;
pub mod logging;
pub mod metrics;
pub mod nam;
pub mod resources;
pub mod shared_types;
pub mod signals;
pub mod state;
pub mod tensor_registry;
pub mod self_model;

// Re-export NAM foundation primitives
pub use nam::{
    AgentOrigin, Confidence, Dissent, LearningSignal, Outcome, HUMAN_AGENT_TAG, LAYER_ID,
    MODULE_COUNT,
};

pub use error::{AnnotatedError, Error, ErrorClassifier, Result, Severity};

// Re-export commonly used config types at the module level
pub use config::{
    Config, ConfigBuilder, ConfigChangeEvent, ConfigManager, ConfigProvider, ValidationError,
    ValidationResult, ValidationWarning,
};

// Re-export metrics types
pub use metrics::{
    create_maintenance_registry, create_registry, export_metrics, increment_counter,
    observe_histogram, register_default_metrics, set_gauge, snapshot_delta, Counter, Gauge,
    Histogram, HistogramSummary, Labels, MetricDelta, MetricRecorder, MetricSnapshot,
    MetricsRegistry, DEFAULT_LATENCY_BUCKETS, DEFAULT_SIZE_BUCKETS,
};

// Re-export resource types
pub use resources::{
    check_limits, collect_resources, compute_health_score, format_alerts, format_resources,
    get_process_info, AdaptiveResourceLimits, ProcessInfo, ResourceAlert, ResourceCollector,
    ResourceLimits, ResourceManager, SystemResources,
};

// Re-export state persistence types (M05: State Persistence)
pub use state::{
    begin_transaction, connect, count, delete, execute, exists, fetch_all, fetch_one,
    fetch_optional, load, run_migrations, save, save_versioned, save_with_provenance,
    DatabaseConfig, DatabasePool, DatabaseType, PoolStats, QueryBuilder, StatePersistence,
    StatePersistenceBuilder, StateStore, Transaction,
};

// Re-export logging types (M03: Logging System)
pub use logging::{
    generate_correlation_id, generate_short_correlation_id, init_logging, is_logging_initialized,
    try_init_logging, with_context, with_context_async, CorrelationProvider, LogConfig, LogContext,
    LogFormat, LogLevel,
};

// Re-export shared types (M00: Shared Types)
pub use shared_types::{AgentId, CoverageBitmap, DimensionIndex, HealthReport, ModuleId, Timestamp};

/// Frozen identity anchor for 12D tensor dimension D1 (port).
///
/// Computed as `8080.0 / 65535.0 ≈ 0.1232`. This is **NOT** the live bind
/// port — `MEv2` binds on :8180 per `devenv.toml` since Session 081 retired V1.
/// D1 is a normalized port-identity hash that seeds `RALPH`'s fitness
/// comparisons and persists across 18,310+ rows in `tensor_memory.db`.
///
/// **Do not rebaseline.** Re-computing against :8180 would orphan every
/// historical fitness delta and bias mutation-effectiveness scoring at the
/// cutover generation. If a live-port dimension is ever needed, add a new
/// dim rather than modifying this anchor.
///
/// History: frozen Session 097 following Circle-of-Experts deliberation
/// (`RALPH` `Historian` + `Test` `Archeologist` + `Safety` `Auditor` + `MEv2`
/// `Architect` + `Runtime` `Operator` voted freeze, 5/6).
pub const ME_IDENTITY_PORT_ANCHOR: f64 = 8080.0 / 65535.0;

// Re-export signal types (M07: Signal Bus)
pub use signals::{
    DissentEvent, HealthSignal, LearningEvent, Signal, SignalBus, SignalBusConfig, SignalBusStats,
    SignalContext, SignalSubscriber,
};

// Re-export tensor registry types (M08: Tensor Registry)
pub use tensor_registry::{
    ComposedTensor, ContributedTensor, ContributorInventoryEntry, ContributorKind,
    TensorContributor, TensorDimension, TensorRegistry,
};

// Re-export self-model types (M48: Self Model)
pub use self_model::{
    ArchitectureDescriptor, CapabilityEntry, CapabilityStatus, CapabilitySummary,
    LayerStatusEntry, RuntimeSnapshot, SelfModel, SelfModelConfig, SelfModelConfigBuilder,
    SelfModelHealth, SelfModelProvider,
};

// ============================================================================
// Foundation Self-Model
// ============================================================================

/// Aggregate status of the L1 Foundation layer.
///
/// Provides a single-point summary of all foundation subsystem health,
/// including a composed 12D tensor encoding the layer state.
#[derive(Debug, Clone, PartialEq)]
pub struct FoundationStatus {
    /// Layer identifier (always "L1")
    pub layer_id: &'static str,
    /// Number of foundation modules (always 6)
    pub module_count: u8,
    /// Whether the logging subsystem has been initialized
    pub logging_initialized: bool,
    /// Whether the current configuration passes validation
    pub config_valid: bool,
    /// Number of metrics currently registered
    pub metrics_count: usize,
    /// Whether system resources are within healthy limits
    pub resources_healthy: bool,
    /// Overall foundation health score (0.0–1.0)
    pub health_score: f64,
    /// Composed 12D tensor representing foundation state
    pub tensor: crate::Tensor12D,
}

impl Default for FoundationStatus {
    fn default() -> Self {
        Self {
            layer_id: LAYER_ID,
            module_count: MODULE_COUNT,
            logging_initialized: false,
            config_valid: true,
            metrics_count: 0,
            resources_healthy: true,
            health_score: 1.0,
            tensor: crate::Tensor12D::default(),
        }
    }
}

/// Build a composed foundation tensor from individual subsystem tensors.
///
/// Averages the per-dimension values from up to three source tensors
/// (config, resources, metrics) to produce a single foundation-level tensor.
/// All dimensions are clamped to [0.0, 1.0] before returning.
#[must_use]
pub fn build_foundation_tensor(
    config_tensor: &crate::Tensor12D,
    resources_tensor: &crate::Tensor12D,
    metrics_tensor: &crate::Tensor12D,
) -> crate::Tensor12D {
    let c = config_tensor.to_array();
    let r = resources_tensor.to_array();
    let m = metrics_tensor.to_array();
    let mut dims = [0.0f64; 12];
    for i in 0..12 {
        dims[i] = (c[i] + r[i] + m[i]) / 3.0;
    }
    let mut tensor = crate::Tensor12D::new(dims);
    tensor.clamp_normalize();
    tensor
}

// ============================================================================
// Layer Integration Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::sync::Arc;

    // ========================================================================
    // Group 1: Trait Importability & Object Safety (6 tests)
    // ========================================================================

    #[test]
    fn test_all_six_traits_importable() {
        fn _assert_traits_exist<
            A: ErrorClassifier,
            B: ConfigProvider,
            C: CorrelationProvider,
            D: MetricRecorder,
            E: StateStore,
            F: ResourceCollector,
        >() {
        }
    }

    #[test]
    fn test_trait_object_config_provider() {
        fn _assert_object_safe(_: &dyn ConfigProvider) {}
        fn _assert_arc_works(cm: ConfigManager) {
            let _provider: Arc<dyn ConfigProvider> = Arc::new(cm);
        }
    }

    #[test]
    fn test_trait_object_metric_recorder() {
        let registry = MetricsRegistry::new();
        let _recorder: Arc<dyn MetricRecorder> = Arc::new(registry);
    }

    #[test]
    fn test_trait_object_state_store() {
        fn _assert_object_safe(_: &dyn StateStore) {}
    }

    #[test]
    fn test_trait_object_resource_collector() {
        let manager = ResourceManager::new();
        let _collector: Arc<dyn ResourceCollector> = Arc::new(manager);
    }

    #[test]
    fn test_trait_object_correlation_provider() {
        let ctx = LogContext::with_context("svc", "L1", "M03");
        let _provider: Box<dyn CorrelationProvider> = Box::new(ctx);
    }

    // ========================================================================
    // Group 2: ErrorClassifier Integration (8 tests)
    // ========================================================================

    #[test]
    fn test_error_classifier_config() {
        let err = Error::Config("bad key".to_string());
        assert!(!err.is_retryable());
        assert!(!err.is_transient());
        assert_eq!(err.severity(), Severity::Low);
    }

    #[test]
    fn test_error_classifier_database() {
        // Generic database error — not retryable unless "locked"/"busy"
        let err = Error::Database("connection lost".to_string());
        assert!(!err.is_retryable());
        assert!(!err.is_transient());
        assert_eq!(err.severity(), Severity::Medium);

        // Database "locked" — retryable
        let err_locked = Error::Database("database is locked".to_string());
        assert!(err_locked.is_retryable());
    }

    #[test]
    fn test_error_classifier_network_retryable() {
        let err = Error::Network {
            target: "synthex".to_string(),
            message: "timeout".to_string(),
        };
        assert!(err.is_retryable());
        assert!(err.is_transient());
        assert_eq!(err.severity(), Severity::Medium);
    }

    #[test]
    fn test_error_classifier_circuit_open() {
        let err = Error::CircuitOpen {
            service_id: "san-k7".to_string(),
            retry_after_ms: 5000,
        };
        assert!(err.is_retryable());
        assert!(err.is_transient());
        assert_eq!(err.severity(), Severity::Medium);
    }

    #[test]
    fn test_error_classifier_consensus_quorum() {
        let err = Error::ConsensusQuorum {
            required: 27,
            received: 15,
        };
        assert!(err.is_retryable());
        assert!(err.is_transient());
        assert_eq!(err.severity(), Severity::High);
    }

    #[test]
    fn test_error_classifier_timeout() {
        let err = Error::Timeout {
            operation: "health_check".to_string(),
            timeout_ms: 3000,
        };
        assert!(err.is_retryable());
        assert!(err.is_transient());
        assert_eq!(err.severity(), Severity::Medium);
    }

    #[test]
    fn test_error_classifier_validation() {
        let err = Error::Validation("invalid port".to_string());
        assert!(!err.is_retryable());
        assert!(!err.is_transient());
        assert_eq!(err.severity(), Severity::Low);
    }

    #[test]
    fn test_severity_ordering() {
        assert!(Severity::Low < Severity::Medium);
        assert!(Severity::Medium < Severity::High);
        assert!(Severity::High < Severity::Critical);
        assert!(Severity::Low < Severity::Critical);
    }

    // ========================================================================
    // Group 3: Config Integration (7 tests)
    // ========================================================================

    #[test]
    fn test_config_feeds_into_log_config() {
        let config = Config::default();
        let log_config = LogConfig::default();
        assert_eq!(config.log_level, log_config.level);
    }

    #[test]
    fn test_config_default_all_fields() {
        let config = Config::default();
        assert_eq!(config.host, "0.0.0.0");
        assert_eq!(config.port, 8180);
        assert_eq!(config.grpc_port, 8081);
        assert_eq!(config.ws_port, 8082);
        assert_eq!(config.database_path, "data/maintenance.db");
        assert_eq!(config.log_level, "info");
    }

    #[test]
    fn test_config_builder_chain() -> Result<()> {
        let config = ConfigBuilder::new()
            .host("127.0.0.1")
            .port(9090)
            .grpc_port(9091)
            .ws_port(9092)
            .skip_files()
            .skip_env()
            .build()?;
        assert_eq!(config.host, "127.0.0.1");
        assert_eq!(config.port, 9090);
        assert_eq!(config.grpc_port, 9091);
        assert_eq!(config.ws_port, 9092);
        Ok(())
    }

    #[test]
    fn test_config_builder_default_matches_config_default() -> Result<()> {
        let from_builder = ConfigBuilder::new().skip_files().skip_env().build()?;
        let from_default = Config::default();
        assert_eq!(from_builder, from_default);
        Ok(())
    }

    #[test]
    fn test_config_clone_equality() {
        let config = Config::default();
        let cloned = config.clone();
        assert_eq!(config, cloned);
    }

    #[test]
    fn test_config_debug_output() {
        let config = Config::default();
        let debug = format!("{config:?}");
        assert!(debug.contains("8180"));
        assert!(debug.contains("0.0.0.0"));
    }

    #[test]
    fn test_config_port_matches_database_path() {
        let config = Config::default();
        let db_config = DatabaseConfig::default();
        assert_eq!(config.database_path, db_config.path);
    }

    #[test]
    fn test_validation_result_success() {
        let result = ValidationResult::success();
        assert!(result.valid);
        assert!(result.errors.is_empty());
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn test_validation_result_failure() {
        let errors = vec![ValidationError {
            key: "port".to_string(),
            code: "E001".to_string(),
            message: "port cannot be zero".to_string(),
        }];
        let result = ValidationResult::failure(errors);
        assert!(!result.valid);
        assert_eq!(result.errors.len(), 1);
        assert_eq!(result.errors[0].key, "port");
        assert_eq!(result.errors[0].code, "E001");
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn test_validation_error_fields() {
        let err = ValidationError {
            key: "host".to_string(),
            code: "E002".to_string(),
            message: "host cannot be empty".to_string(),
        };
        assert_eq!(err.key, "host");
        assert_eq!(err.code, "E002");
        assert_eq!(err.message, "host cannot be empty");
    }

    #[test]
    fn test_validation_warning_fields() {
        let warning = ValidationWarning {
            key: "log_level".to_string(),
            code: "W001".to_string(),
            message: "trace level is verbose for production".to_string(),
        };
        assert_eq!(warning.key, "log_level");
        assert_eq!(warning.code, "W001");
        assert!(warning.message.contains("verbose"));
    }

    #[test]
    fn test_validation_result_with_warnings() {
        let mut result = ValidationResult::success();
        result.warnings.push(ValidationWarning {
            key: "ws_port".to_string(),
            code: "W002".to_string(),
            message: "ws_port is same as default".to_string(),
        });
        assert!(result.valid);
        assert_eq!(result.warnings.len(), 1);
    }

    #[test]
    fn test_validation_result_clone() {
        let errors = vec![ValidationError {
            key: "port".to_string(),
            code: "E001".to_string(),
            message: "invalid".to_string(),
        }];
        let result = ValidationResult::failure(errors);
        let cloned = result.clone();
        assert_eq!(cloned.valid, result.valid);
        assert_eq!(cloned.errors.len(), result.errors.len());
    }

    #[test]
    fn test_config_change_event_fields() {
        let event = ConfigChangeEvent {
            change_id: "chg-001".to_string(),
            timestamp: chrono::Utc::now(),
            changed_keys: vec!["port".to_string(), "host".to_string()],
            previous: {
                let mut m = std::collections::HashMap::new();
                m.insert("port".to_string(), "8180".to_string());
                m
            },
            new: {
                let mut m = std::collections::HashMap::new();
                m.insert("port".to_string(), "9090".to_string());
                m
            },
            requested_by: None,
        };
        assert_eq!(event.change_id, "chg-001");
        assert_eq!(event.changed_keys.len(), 2);
        assert!(event.changed_keys.contains(&"port".to_string()));
        assert_eq!(event.previous.get("port").map(String::as_str), Some("8180"));
        assert_eq!(event.new.get("port").map(String::as_str), Some("9090"));
    }

    #[test]
    fn test_config_change_event_clone() {
        let event = ConfigChangeEvent {
            change_id: "chg-002".to_string(),
            timestamp: chrono::Utc::now(),
            changed_keys: vec!["log_level".to_string()],
            previous: std::collections::HashMap::new(),
            new: std::collections::HashMap::new(),
            requested_by: None,
        };
        let cloned = event.clone();
        assert_eq!(cloned.change_id, event.change_id);
        assert_eq!(cloned.changed_keys, event.changed_keys);
    }

    // ========================================================================
    // Group 4: Logging Integration (8 tests)
    // ========================================================================

    #[test]
    fn test_log_context_new_has_correlation_id() {
        let ctx = LogContext::new();
        assert!(!ctx.correlation_id.is_empty());
        assert!(ctx.service_id.is_none());
        assert!(ctx.layer.is_none());
        assert!(ctx.module.is_none());
    }

    #[test]
    fn test_log_context_with_context_populates_fields() {
        let ctx = LogContext::with_context("maintenance-engine", "L1", "M03");
        assert!(!ctx.correlation_id.is_empty());
        assert_eq!(ctx.service_id.as_deref(), Some("maintenance-engine"));
        assert_eq!(ctx.layer.as_deref(), Some("L1"));
        assert_eq!(ctx.module.as_deref(), Some("M03"));
    }

    #[test]
    fn test_correlation_id_uniqueness() {
        let ids: HashSet<String> = (0..100).map(|_| generate_correlation_id()).collect();
        assert_eq!(ids.len(), 100, "All 100 correlation IDs must be unique");
    }

    #[test]
    fn test_short_correlation_id_length() {
        let id = generate_short_correlation_id();
        assert!(!id.is_empty());
        assert!(id.len() <= 12, "Short ID should be compact");
    }

    #[test]
    fn test_log_level_ordering() {
        assert!(LogLevel::Trace < LogLevel::Debug);
        assert!(LogLevel::Debug < LogLevel::Info);
        assert!(LogLevel::Info < LogLevel::Warn);
        assert!(LogLevel::Warn < LogLevel::Error);
    }

    #[test]
    fn test_log_level_parse_all_variants() -> Result<()> {
        assert_eq!(LogLevel::parse_str("trace")?, LogLevel::Trace);
        assert_eq!(LogLevel::parse_str("debug")?, LogLevel::Debug);
        assert_eq!(LogLevel::parse_str("info")?, LogLevel::Info);
        assert_eq!(LogLevel::parse_str("warn")?, LogLevel::Warn);
        assert_eq!(LogLevel::parse_str("warning")?, LogLevel::Warn);
        assert_eq!(LogLevel::parse_str("error")?, LogLevel::Error);
        assert!(LogLevel::parse_str("invalid").is_err());
        Ok(())
    }

    #[test]
    fn test_log_format_parse_and_display() -> Result<()> {
        assert_eq!(LogFormat::parse_str("json")?, LogFormat::Json);
        assert_eq!(LogFormat::parse_str("pretty")?, LogFormat::Pretty);
        assert_eq!(LogFormat::parse_str("compact")?, LogFormat::Compact);
        assert_eq!(format!("{}", LogFormat::Json), "json");
        assert_eq!(format!("{}", LogFormat::Pretty), "pretty");
        assert_eq!(format!("{}", LogFormat::Compact), "compact");
        assert!(LogFormat::parse_str("xml").is_err());
        Ok(())
    }

    #[test]
    fn test_log_config_default_values() {
        let config = LogConfig::default();
        assert_eq!(config.level, "info");
        assert_eq!(config.format, LogFormat::Pretty);
        assert!(config.include_timestamps);
        assert!(config.include_targets);
        assert!(!config.include_file_line);
        assert!(!config.include_thread_ids);
        assert!(!config.include_span_events);
    }

    // ========================================================================
    // Group 5: Metrics Integration (8 tests)
    // ========================================================================

    #[test]
    fn test_metrics_registry_empty() {
        let registry = MetricsRegistry::new();
        assert_eq!(registry.metric_count(), 0);
        assert!(registry.list_metrics().is_empty());
    }

    #[test]
    fn test_metrics_counter_lifecycle() -> Result<()> {
        let registry = MetricsRegistry::new();
        let counter = registry.register_counter("test_total", "A test counter", &["service"])?;
        let labels = Labels::new().service("me");
        counter.inc(&labels);
        counter.inc(&labels);
        assert_eq!(counter.get(&labels), 2);
        counter.reset(&labels);
        assert_eq!(counter.get(&labels), 0);
        Ok(())
    }

    #[test]
    fn test_metrics_gauge_lifecycle() -> Result<()> {
        let registry = MetricsRegistry::new();
        let gauge = registry.register_gauge("health_score", "Health score", &["service"])?;
        let labels = Labels::new().service("me");
        gauge.set(&labels, 0.95);
        let val = gauge.get(&labels);
        assert!((val - 0.95).abs() < 0.001);
        Ok(())
    }

    #[test]
    fn test_metrics_histogram_lifecycle() -> Result<()> {
        let registry = MetricsRegistry::new();
        let histogram = registry.register_histogram(
            "request_duration",
            "Request duration",
            &["service"],
            &DEFAULT_LATENCY_BUCKETS,
        )?;
        let labels = Labels::new().service("me");
        histogram.observe(&labels, 0.05);
        histogram.observe(&labels, 0.15);
        histogram.observe(&labels, 1.5);
        assert_eq!(registry.metric_count(), 1);
        Ok(())
    }

    #[test]
    fn test_metrics_export_contains_type_lines() -> Result<()> {
        let registry = MetricsRegistry::new();
        registry.register_counter("my_counter", "Help text", &[])?;
        let output = registry.export_metrics();
        assert!(output.contains("# HELP my_counter Help text"));
        assert!(output.contains("# TYPE my_counter counter"));
        Ok(())
    }

    #[test]
    fn test_labels_builder_chain() {
        let labels = Labels::new()
            .service("maintenance-engine")
            .layer("L1")
            .module("M04")
            .tier("2")
            .status("healthy");
        let debug = format!("{labels:?}");
        assert!(debug.contains("maintenance-engine"));
        assert!(debug.contains("L1"));
    }

    #[test]
    fn test_labels_from_pairs() {
        let labels = Labels::from_pairs(&[("service", "me"), ("layer", "L1")]);
        let empty = Labels::from_pairs(&[]);
        assert_ne!(labels, empty);
    }

    #[test]
    fn test_metric_snapshot_default() {
        let snapshot = MetricSnapshot::default();
        assert_eq!(snapshot.timestamp, 0);
        assert!(snapshot.counters.is_empty());
        assert!(snapshot.gauges.is_empty());
        assert!(snapshot.histograms.is_empty());
    }

    #[test]
    fn test_histogram_summary_default() {
        let summary = HistogramSummary::default();
        assert_eq!(summary.count, 0);
        assert!((summary.sum).abs() < f64::EPSILON);
        assert!((summary.p50).abs() < f64::EPSILON);
        assert!((summary.p95).abs() < f64::EPSILON);
        assert!((summary.p99).abs() < f64::EPSILON);
    }

    #[test]
    fn test_histogram_summary_fields() {
        let summary = HistogramSummary {
            count: 100,
            sum: 42.5,
            p50: 0.3,
            p95: 0.8,
            p99: 1.2,
        };
        assert_eq!(summary.count, 100);
        assert!((summary.sum - 42.5).abs() < f64::EPSILON);
        assert!((summary.p50 - 0.3).abs() < f64::EPSILON);
        assert!((summary.p95 - 0.8).abs() < f64::EPSILON);
        assert!((summary.p99 - 1.2).abs() < f64::EPSILON);
    }

    #[test]
    fn test_histogram_summary_clone() {
        let summary = HistogramSummary {
            count: 50,
            sum: 25.0,
            p50: 0.4,
            p95: 0.9,
            p99: 1.5,
        };
        let cloned = summary.clone();
        assert_eq!(cloned.count, summary.count);
        assert!((cloned.sum - summary.sum).abs() < f64::EPSILON);
    }

    #[test]
    fn test_histogram_summary_in_metric_snapshot() {
        let mut snapshot = MetricSnapshot::default();
        snapshot.histograms.insert(
            "request_duration".to_string(),
            HistogramSummary {
                count: 200,
                sum: 100.0,
                p50: 0.35,
                p95: 0.85,
                p99: 1.4,
            },
        );
        assert_eq!(snapshot.histograms.len(), 1);
        let summary = &snapshot.histograms["request_duration"];
        assert_eq!(summary.count, 200);
        assert!((summary.p50 - 0.35).abs() < f64::EPSILON);
    }

    // ========================================================================
    // Group 6: State/Persistence Integration (7 tests)
    // ========================================================================

    #[test]
    fn test_database_config_default() {
        let config = DatabaseConfig::default();
        assert_eq!(config.path, "data/maintenance.db");
        assert_eq!(config.max_connections, 10);
        assert_eq!(config.min_connections, 2);
        assert_eq!(config.acquire_timeout_secs, 30);
        assert!(config.wal_mode);
        assert!(config.create_if_missing);
    }

    #[test]
    fn test_database_config_builder_chain() {
        let config = DatabaseConfig::new("data/test.db")
            .with_max_connections(20)
            .with_min_connections(5)
            .with_acquire_timeout(60)
            .with_wal_mode(false);
        assert_eq!(config.path, "data/test.db");
        assert_eq!(config.max_connections, 20);
        assert_eq!(config.min_connections, 5);
        assert_eq!(config.acquire_timeout_secs, 60);
        assert!(!config.wal_mode);
    }

    #[test]
    fn test_database_type_all_eleven_variants() {
        let all = DatabaseType::all();
        assert_eq!(all.len(), 11);
    }

    #[test]
    fn test_database_type_filenames_unique() {
        let all = DatabaseType::all();
        let filenames: HashSet<&str> = all.iter().map(DatabaseType::filename).collect();
        assert_eq!(filenames.len(), 11, "All filenames must be unique");
    }

    #[test]
    fn test_database_type_display_matches_filename() {
        for db_type in DatabaseType::all() {
            assert_eq!(format!("{db_type}"), db_type.filename());
        }
    }

    #[test]
    fn test_database_type_migration_numbers_sequential() {
        let all = DatabaseType::all();
        for (i, db_type) in all.iter().enumerate() {
            assert_eq!(
                db_type.migration_number() as usize,
                i + 1,
                "Migration number for {:?} should be {}",
                db_type,
                i + 1
            );
        }
    }

    #[test]
    fn test_query_builder_select_from_where() {
        let qb = QueryBuilder::select(&["id", "name"])
            .from("services")
            .where_eq("status", "running")
            .order_by("name", "ASC")
            .limit(10);
        let sql = qb.build();
        let params = qb.params();
        assert!(sql.contains("SELECT id, name"));
        assert!(sql.contains("FROM services"));
        assert!(sql.contains("WHERE status = ?"));
        assert!(sql.contains("ORDER BY name ASC"));
        assert!(sql.contains("LIMIT 10"));
        assert_eq!(params.len(), 1);
        assert_eq!(params[0], "running");
    }

    // ========================================================================
    // Group 7: Resources Integration (8 tests)
    // ========================================================================

    #[test]
    fn test_resource_limits_default() {
        let limits = ResourceLimits::default();
        assert!((limits.max_cpu_percent - 80.0).abs() < f64::EPSILON);
        assert!((limits.max_memory_percent - 85.0).abs() < f64::EPSILON);
        assert!((limits.max_disk_percent - 90.0).abs() < f64::EPSILON);
        assert_eq!(limits.max_open_files, 1000);
    }

    #[test]
    fn test_resource_limits_custom() {
        let limits = ResourceLimits::new(95.0, 95.0, 95.0, 5000);
        assert!((limits.max_cpu_percent - 95.0).abs() < f64::EPSILON);
        assert!((limits.max_memory_percent - 95.0).abs() < f64::EPSILON);
        assert_eq!(limits.max_open_files, 5000);
    }

    #[test]
    fn test_resource_limits_validation_ok() -> Result<()> {
        let limits = ResourceLimits::new(80.0, 85.0, 90.0, 1000);
        limits.validate()?;
        Ok(())
    }

    #[test]
    fn test_resource_limits_validation_rejects_out_of_range() {
        let limits = ResourceLimits::new(150.0, 85.0, 90.0, 1000);
        assert!(limits.validate().is_err());

        let limits = ResourceLimits::new(80.0, -5.0, 90.0, 1000);
        assert!(limits.validate().is_err());
    }

    #[test]
    fn test_resource_manager_new_healthy() {
        let manager = ResourceManager::new();
        assert!((manager.health_score() - 1.0).abs() < f64::EPSILON);
        assert!(manager.is_healthy());
        assert!(manager.last_snapshot().is_none());
        assert!(manager.alert_history().is_empty());
    }

    #[test]
    fn test_resource_manager_with_custom_limits() {
        let limits = ResourceLimits::new(50.0, 60.0, 70.0, 500);
        let manager = ResourceManager::with_limits(limits);
        let mgr_limits = manager.limits();
        assert!((mgr_limits.max_cpu_percent - 50.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_resource_alert_display_all_variants() {
        let alerts = vec![
            ResourceAlert::CpuHigh {
                current: 95.0,
                threshold: 80.0,
            },
            ResourceAlert::MemoryHigh {
                current: 90.0,
                threshold: 85.0,
            },
            ResourceAlert::DiskHigh {
                current: 95.0,
                threshold: 90.0,
            },
            ResourceAlert::OpenFilesHigh {
                current: 1500,
                threshold: 1000,
            },
        ];
        for alert in &alerts {
            let display = format!("{alert}");
            assert!(!display.is_empty());
        }
        assert!(format!("{}", alerts[0]).contains("CPU"));
        assert!(format!("{}", alerts[1]).contains("Memory"));
        assert!(format!("{}", alerts[2]).contains("Disk"));
        assert!(format!("{}", alerts[3]).contains("Open files"));
    }

    #[test]
    fn test_check_limits_no_alerts_when_under_threshold() {
        let resources = SystemResources {
            cpu_percent: 50.0,
            memory_percent: 60.0,
            disk_percent: 70.0,
            open_files: 500,
            ..SystemResources::default()
        };
        let limits = ResourceLimits::default();
        let alerts = check_limits(&resources, &limits);
        assert!(alerts.is_empty());
    }

    // ========================================================================
    // Group 8: Cross-Module & Re-export Integration (8 tests)
    // ========================================================================

    #[test]
    fn test_type_re_export_completeness() {
        let _: Error = Error::Other("test".to_string());
        let _: Severity = Severity::Low;
        let _: Config = Config::default();
        let _: LogContext = LogContext::new();
        let _: LogConfig = LogConfig::default();
        let _: LogLevel = LogLevel::Info;
        let _: LogFormat = LogFormat::Pretty;
        let _: Labels = Labels::new();
        let _: MetricsRegistry = MetricsRegistry::new();
        let _: MetricSnapshot = MetricSnapshot::default();
        let _: HistogramSummary = HistogramSummary::default();
        let _: ResourceLimits = ResourceLimits::default();
        let _: ResourceManager = ResourceManager::new();
        let _: SystemResources = SystemResources::default();
        let _: ProcessInfo = ProcessInfo::default();
        let _: DatabaseConfig = DatabaseConfig::default();
        let _: QueryBuilder = QueryBuilder::new("SELECT 1");
        let _: PoolStats = PoolStats { size: 0, idle: 0 };
        let _: ValidationResult = ValidationResult::success();
        let _: ValidationError = ValidationError {
            key: String::new(),
            code: String::new(),
            message: String::new(),
        };
        let _: ValidationWarning = ValidationWarning {
            key: String::new(),
            code: String::new(),
            message: String::new(),
        };
    }

    #[test]
    fn test_default_construction_sensible() {
        let config = Config::default();
        assert_eq!(config.port, 8180);

        let log_config = LogConfig::default();
        assert_eq!(log_config.level, "info");

        let limits = ResourceLimits::default();
        assert!((limits.max_cpu_percent - 80.0).abs() < f64::EPSILON);

        let manager = ResourceManager::new();
        assert!((manager.health_score() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_error_result_type_alias() {
        fn returns_ok() -> Result<u32> {
            Ok(42)
        }
        fn returns_err() -> Result<u32> {
            Err(Error::Other("fail".to_string()))
        }
        assert_eq!(returns_ok().ok(), Some(42));
        assert!(returns_err().is_err());
    }

    #[test]
    fn test_error_from_string_conversion() {
        let err: Error = Error::from("something went wrong".to_string());
        let display = format!("{err}");
        assert!(display.contains("something went wrong"));
    }

    #[test]
    fn test_error_from_io_error() {
        // NotFound is NOT retryable
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file missing");
        let err = Error::from(io_err);
        assert!(matches!(err, Error::Io(_)));
        assert!(!err.is_retryable());
        assert_eq!(err.severity(), Severity::Medium);

        // ConnectionRefused IS retryable
        let io_err2 =
            std::io::Error::new(std::io::ErrorKind::ConnectionRefused, "refused");
        let err2 = Error::from(io_err2);
        assert!(err2.is_retryable());
    }

    #[test]
    fn test_error_display_all_simple_variants() {
        let errors = vec![
            Error::Config("bad config".to_string()),
            Error::Database("db down".to_string()),
            Error::Pipeline("pipe broke".to_string()),
            Error::Validation("invalid".to_string()),
            Error::Other("unknown".to_string()),
            Error::ServiceNotFound("ghost".to_string()),
        ];
        for err in &errors {
            let display = format!("{err}");
            assert!(!display.is_empty(), "Display should not be empty for {err:?}");
        }
    }

    #[test]
    fn test_error_display_struct_variants() {
        let errors: Vec<Error> = vec![
            Error::Network {
                target: "synthex".to_string(),
                message: "refused".to_string(),
            },
            Error::CircuitOpen {
                service_id: "san-k7".to_string(),
                retry_after_ms: 5000,
            },
            Error::ConsensusQuorum {
                required: 27,
                received: 15,
            },
            Error::ViewChange {
                current_view: 1,
                new_view: 2,
            },
            Error::PathwayNotFound {
                source: "M25".to_string(),
                target: "M26".to_string(),
            },
            Error::TensorValidation {
                dimension: 3,
                value: -1.0,
            },
            Error::HealthCheckFailed {
                service_id: "me".to_string(),
                reason: "timeout".to_string(),
            },
            Error::EscalationRequired {
                from_tier: "L0".to_string(),
                to_tier: "L3".to_string(),
                reason: "critical".to_string(),
            },
            Error::Timeout {
                operation: "query".to_string(),
                timeout_ms: 3000,
            },
        ];
        for err in &errors {
            let display = format!("{err}");
            assert!(!display.is_empty(), "Display should not be empty for {err:?}");
        }
    }

    #[test]
    fn test_standalone_function_re_exports() {
        // Verify standalone functions are callable through re-exports
        let _id = generate_correlation_id();
        let _short = generate_short_correlation_id();
        let _initialized = is_logging_initialized();

        // Resource functions
        let resources = SystemResources::default();
        let limits = ResourceLimits::default();
        let _alerts = check_limits(&resources, &limits);
        let _formatted = format_resources(&resources);
        let _alert_text = format_alerts(&[]);
        let _score = compute_health_score(Some(&resources), &limits);

        // Metric functions
        let registry = create_registry();
        assert_eq!(registry.metric_count(), 0);
        let _export = export_metrics(&registry);

        // State query builder functions
        let qb = QueryBuilder::new("SELECT 1");
        let sql = qb.build();
        assert!(sql.contains("SELECT"));
    }

    // ========================================================================
    // Group 9: Constants & Bucket Re-exports (3 tests)
    // ========================================================================

    #[test]
    fn test_default_latency_buckets_sorted() {
        for window in DEFAULT_LATENCY_BUCKETS.windows(2) {
            assert!(
                window[0] < window[1],
                "Latency buckets must be sorted: {} >= {}",
                window[0],
                window[1]
            );
        }
    }

    #[test]
    fn test_default_size_buckets_sorted() {
        for window in DEFAULT_SIZE_BUCKETS.windows(2) {
            assert!(
                window[0] < window[1],
                "Size buckets must be sorted: {} >= {}",
                window[0],
                window[1]
            );
        }
    }

    #[test]
    fn test_create_maintenance_registry_prefixed() -> Result<()> {
        let registry = create_maintenance_registry();
        let counter =
            registry.register_counter("test_counter", "A test", &[])?;
        let labels = Labels::new();
        counter.inc(&labels);
        let exported = registry.export_metrics();
        assert!(
            exported.contains("maintenance_test_counter"),
            "Maintenance registry should prefix metrics with 'maintenance_'"
        );
        Ok(())
    }

    // ========================================================================
    // Group 10: NAM Type Re-export Completeness (12 tests)
    // ========================================================================

    #[test]
    fn test_nam_types_importable() {
        let _origin: AgentOrigin = AgentOrigin::System;
        let _confidence: Confidence = Confidence::certain();
        let _outcome: Outcome = Outcome::Success;
        let _signal: LearningSignal = LearningSignal::success("test");
        let _dissent: Dissent = Dissent::new(
            AgentOrigin::System,
            "target",
            "reasoning",
        );
    }

    #[test]
    fn test_nam_constants_accessible() {
        assert_eq!(HUMAN_AGENT_TAG, "@0.A");
        assert_eq!(LAYER_ID, "L1");
        assert_eq!(MODULE_COUNT, 9);
    }

    #[test]
    fn test_agent_origin_human_with_tag() {
        let origin = AgentOrigin::Human {
            tag: HUMAN_AGENT_TAG.to_string(),
        };
        let display = format!("{origin}");
        assert!(display.contains("@0.A"));
    }

    #[test]
    fn test_agent_origin_uses_crate_agent_role() {
        let origin = AgentOrigin::agent("a-001", crate::AgentRole::Validator);
        assert!(matches!(origin, AgentOrigin::Agent { .. }));
    }

    #[test]
    fn test_annotated_error_re_exported() {
        let err = Error::Config("bad".to_string());
        let annotated = AnnotatedError::new(err);
        assert!(annotated.origin.is_none());
        assert_eq!(annotated.confidence, Confidence::certain());
    }

    #[test]
    fn test_annotated_error_with_origin() {
        let err = Error::Other("fail".to_string());
        let annotated = AnnotatedError::new(err)
            .with_origin(AgentOrigin::human());
        assert!(annotated.origin.is_some());
    }

    #[test]
    fn test_metric_delta_re_exported() {
        let delta = MetricDelta::default();
        assert!(delta.counter_deltas.is_empty());
        assert!(delta.gauge_deltas.is_empty());
        assert_eq!(delta.duration_between, 0);
    }

    #[test]
    fn test_snapshot_delta_re_exported() {
        let prev = MetricSnapshot::default();
        let next = MetricSnapshot::default();
        let delta = snapshot_delta(&prev, &next);
        assert!(delta.counter_deltas.is_empty());
    }

    #[test]
    fn test_adaptive_resource_limits_re_exported() {
        let adaptive = AdaptiveResourceLimits {
            base: ResourceLimits::default(),
            pathway_strength: 0.5,
        };
        let effective = adaptive.effective_limits();
        // With 0.5 pathway_strength, thresholds should be relaxed slightly
        assert!(effective.max_cpu_percent >= 80.0);
    }

    #[test]
    fn test_foundation_status_default() {
        let status = FoundationStatus::default();
        assert_eq!(status.layer_id, "L1");
        assert_eq!(status.module_count, 9);
        assert!(!status.logging_initialized);
        assert!(status.config_valid);
        assert_eq!(status.metrics_count, 0);
        assert!(status.resources_healthy);
        assert!((status.health_score - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_foundation_status_tensor_valid() {
        let status = FoundationStatus::default();
        assert!(status.tensor.validate().is_ok());
    }

    #[test]
    fn test_build_foundation_tensor_defaults() {
        let config_t = crate::Tensor12D::default();
        let resources_t = crate::Tensor12D::default();
        let metrics_t = crate::Tensor12D::default();
        let composed = build_foundation_tensor(&config_t, &resources_t, &metrics_t);
        assert!(composed.validate().is_ok());
        // All zeros averaged = all zeros
        for &dim in &composed.to_array() {
            assert!(dim.abs() < f64::EPSILON);
        }
    }

    #[test]
    fn test_build_foundation_tensor_averages() {
        let config_t = crate::Tensor12D::new([0.3; 12]);
        let resources_t = crate::Tensor12D::new([0.6; 12]);
        let metrics_t = crate::Tensor12D::new([0.9; 12]);
        let composed = build_foundation_tensor(&config_t, &resources_t, &metrics_t);
        assert!(composed.validate().is_ok());
        // Average of 0.3, 0.6, 0.9 = 0.6
        for &dim in &composed.to_array() {
            assert!((dim - 0.6).abs() < 1e-10);
        }
    }

    #[test]
    fn test_build_foundation_tensor_clamps() {
        // Values that average above 1.0 should be clamped
        let high = crate::Tensor12D::new([1.0; 12]);
        let higher = crate::Tensor12D::new([1.0; 12]);
        let highest = crate::Tensor12D::new([1.0; 12]);
        let composed = build_foundation_tensor(&high, &higher, &highest);
        assert!(composed.validate().is_ok());
        for &dim in &composed.to_array() {
            assert!(dim <= 1.0);
            assert!(dim >= 0.0);
        }
    }

    #[test]
    fn test_tensor_encoding_round_trip_config() {
        let config = Config::default();
        let tensor = config.to_tensor();
        assert!(tensor.validate().is_ok());
        // Config::to_tensor().port tracks config.port live; D1 in engine.rs is
        // frozen at ME_IDENTITY_PORT_ANCHOR. Different tensors, different purposes.
        assert!(tensor.port > 0.0);
        assert!(tensor.port < 1.0);
    }

    #[test]
    fn test_tensor_encoding_round_trip_resources() {
        let resources = SystemResources::default();
        let tensor = resources.to_tensor();
        assert!(tensor.validate().is_ok());
    }

    #[test]
    fn test_tensor_compose_config_and_resources() {
        let config = Config::default();
        let resources = SystemResources::default();
        let metrics = MetricSnapshot::default();
        let composed = build_foundation_tensor(
            &config.to_tensor(),
            &resources.to_tensor(),
            &metrics.to_tensor(),
        );
        assert!(composed.validate().is_ok());
    }
}
