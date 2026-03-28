//! # Layer 1 Foundation Integration Tests
//!
//! Comprehensive integration tests for the M1-M6 Foundation modules:
//! - M01: Error Taxonomy
//! - M02: Configuration Manager
//! - M03: Logging System
//! - M04: Metrics Collector
//! - M05: State Persistence
//! - M06: Resource Manager
//!
//! These tests validate cross-module interactions, public API contracts,
//! and boundary conditions from an external-crate perspective.

mod common;

use std::collections::HashSet;

// ---------------------------------------------------------------------------
// M01: Error Taxonomy imports
// ---------------------------------------------------------------------------
use maintenance_engine::m1_foundation::Error;

// ---------------------------------------------------------------------------
// M02: Configuration imports
// ---------------------------------------------------------------------------
use maintenance_engine::m1_foundation::config::{Config, ConfigBuilder, ConfigManager};

// ---------------------------------------------------------------------------
// M03: Logging imports
// ---------------------------------------------------------------------------
use maintenance_engine::m1_foundation::logging::{
    generate_correlation_id, generate_short_correlation_id, LogConfig, LogContext, LogFormat,
    LogLevel,
};

// ---------------------------------------------------------------------------
// M04: Metrics imports
// ---------------------------------------------------------------------------
use maintenance_engine::m1_foundation::metrics::{
    create_maintenance_registry, create_registry, export_metrics, increment_counter, Labels,
    MetricsRegistry, observe_histogram, register_default_metrics, set_gauge,
    DEFAULT_LATENCY_BUCKETS, DEFAULT_SIZE_BUCKETS,
};

// ---------------------------------------------------------------------------
// M05: State Persistence imports
// ---------------------------------------------------------------------------
use maintenance_engine::m1_foundation::state::{DatabaseConfig, DatabaseType, QueryBuilder};

// ---------------------------------------------------------------------------
// M06: Resource Manager imports
// ---------------------------------------------------------------------------
use maintenance_engine::m1_foundation::resources::{
    check_limits, format_alerts, format_resources, ProcessInfo, ResourceAlert, ResourceLimits,
    ResourceManager, SystemResources,
};

// ===========================================================================
// Constants
// ===========================================================================

/// Number of database types exposed by the state module.
const DATABASE_TYPE_COUNT: usize = 11;

/// Default REST API port.
const DEFAULT_PORT: u16 = 8080;

/// Default gRPC port.
const DEFAULT_GRPC_PORT: u16 = 8081;

/// Default WebSocket port.
const DEFAULT_WS_PORT: u16 = 8082;

/// Correlation ID length (UUID v4 with hyphens).
const CORRELATION_ID_LEN: usize = 36;

/// Short correlation ID length.
const SHORT_CORRELATION_ID_LEN: usize = 8;

/// Default resource limit for CPU percentage.
const DEFAULT_MAX_CPU: f64 = 80.0;

/// Default resource limit for memory percentage.
const DEFAULT_MAX_MEMORY: f64 = 85.0;

/// Default resource limit for disk percentage.
const DEFAULT_MAX_DISK: f64 = 90.0;

/// Default resource limit for open files.
const DEFAULT_MAX_OPEN_FILES: u32 = 1000;

/// Float comparison tolerance.
const FLOAT_TOLERANCE: f64 = 1e-6;

// ===========================================================================
// 1. Error Taxonomy (M01)
// ===========================================================================

#[test]
fn error_config_display_contains_message() {
    let err = Error::Config("bad port".into());
    let display = err.to_string();
    assert!(display.contains("Configuration error"), "display was: {display}");
    assert!(display.contains("bad port"), "display was: {display}");
}

#[test]
fn error_database_display_contains_message() {
    let err = Error::Database("connection lost".into());
    let display = err.to_string();
    assert!(display.contains("Database error"), "display was: {display}");
    assert!(display.contains("connection lost"), "display was: {display}");
}

#[test]
fn error_network_display_contains_target_and_message() {
    let err = Error::Network {
        target: "synthex".into(),
        message: "timeout".into(),
    };
    let display = err.to_string();
    assert!(display.contains("synthex"), "display was: {display}");
    assert!(display.contains("timeout"), "display was: {display}");
}

#[test]
fn error_circuit_open_display_includes_service_and_retry() {
    let err = Error::CircuitOpen {
        service_id: "nais".into(),
        retry_after_ms: 5000,
    };
    let display = err.to_string();
    assert!(display.contains("Circuit breaker open"), "display was: {display}");
    assert!(display.contains("nais"), "display was: {display}");
    assert!(display.contains("5000"), "display was: {display}");
}

#[test]
fn error_consensus_quorum_display_shows_fraction() {
    let err = Error::ConsensusQuorum {
        required: 27,
        received: 18,
    };
    let display = err.to_string();
    assert!(display.contains("18/27"), "display was: {display}");
}

#[test]
fn error_view_change_display_shows_views() {
    let err = Error::ViewChange {
        current_view: 3,
        new_view: 4,
    };
    let display = err.to_string();
    assert!(display.contains('3'), "display was: {display}");
    assert!(display.contains('4'), "display was: {display}");
}

#[test]
fn error_pathway_not_found_display_shows_source_target() {
    let err = Error::PathwayNotFound {
        source: "M01".into(),
        target: "M06".into(),
    };
    let display = err.to_string();
    assert!(display.contains("M01"), "display was: {display}");
    assert!(display.contains("M06"), "display was: {display}");
}

#[test]
fn error_tensor_validation_display_shows_dimension_and_value() {
    let err = Error::TensorValidation {
        dimension: 6,
        value: 1.5,
    };
    let display = err.to_string();
    assert!(display.contains("dimension 6"), "display was: {display}");
    assert!(display.contains("1.5"), "display was: {display}");
}

#[test]
fn error_service_not_found_display() {
    let err = Error::ServiceNotFound("ghost-service".into());
    let display = err.to_string();
    assert!(display.contains("ghost-service"), "display was: {display}");
}

#[test]
fn error_health_check_failed_display() {
    let err = Error::HealthCheckFailed {
        service_id: "san-k7".into(),
        reason: "no heartbeat".into(),
    };
    let display = err.to_string();
    assert!(display.contains("san-k7"), "display was: {display}");
    assert!(display.contains("no heartbeat"), "display was: {display}");
}

#[test]
fn error_escalation_required_display() {
    let err = Error::EscalationRequired {
        from_tier: "L0".into(),
        to_tier: "L2".into(),
        reason: "low confidence".into(),
    };
    let display = err.to_string();
    assert!(display.contains("L0"), "display was: {display}");
    assert!(display.contains("L2"), "display was: {display}");
    assert!(display.contains("low confidence"), "display was: {display}");
}

#[test]
fn error_timeout_display() {
    let err = Error::Timeout {
        operation: "health_check".into(),
        timeout_ms: 3000,
    };
    let display = err.to_string();
    assert!(display.contains("health_check"), "display was: {display}");
    assert!(display.contains("3000"), "display was: {display}");
}

#[test]
fn error_pipeline_and_validation_and_other_display() {
    let pipeline = Error::Pipeline("stage failed".into());
    assert!(pipeline.to_string().contains("Pipeline error"));

    let validation = Error::Validation("field missing".into());
    assert!(validation.to_string().contains("Validation error"));

    let other = Error::Other("unexpected".into());
    assert!(other.to_string().contains("unexpected"));
}

#[test]
fn error_io_conversion_roundtrip() {
    let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file gone");
    let engine_err: Error = io_err.into();
    let display = engine_err.to_string();
    assert!(display.contains("IO error"), "display was: {display}");
    assert!(display.contains("file gone"), "display was: {display}");

    // Verify std::error::Error::source returns the inner io error
    let source = std::error::Error::source(&engine_err);
    assert!(source.is_some(), "IO error variant should expose source");
}

#[test]
fn error_non_io_variants_have_no_source() {
    let err = Error::Config("test".into());
    assert!(
        std::error::Error::source(&err).is_none(),
        "Config variant should not expose a source"
    );
}

#[test]
fn error_debug_impl_produces_non_empty_string() {
    let err = Error::ConsensusQuorum {
        required: 27,
        received: 20,
    };
    let debug_str = format!("{err:?}");
    assert!(!debug_str.is_empty(), "Debug output should not be empty");
    assert!(debug_str.contains("ConsensusQuorum"), "debug was: {debug_str}");
}

// ===========================================================================
// 2. Configuration (M02)
// ===========================================================================

#[test]
fn config_defaults_returns_expected_values() {
    let config = Config::defaults();
    assert_eq!(config.host, "0.0.0.0");
    assert_eq!(config.port, DEFAULT_PORT);
    assert_eq!(config.grpc_port, DEFAULT_GRPC_PORT);
    assert_eq!(config.ws_port, DEFAULT_WS_PORT);
    assert_eq!(config.database_path, "data/maintenance.db");
    assert_eq!(config.log_level, "info");
}

#[test]
fn config_builder_skip_files_and_env_produces_defaults() {
    let result = ConfigBuilder::new().skip_files().skip_env().build();
    let Ok(config) = result else {
        panic!("build failed");
    };
    assert_eq!(config.port, DEFAULT_PORT);
}

#[test]
fn config_builder_custom_values_applied() {
    let result = ConfigBuilder::new()
        .skip_files()
        .skip_env()
        .host("10.0.0.1")
        .port(9090)
        .grpc_port(9091)
        .ws_port(9092)
        .database_path("/tmp/test.db")
        .log_level("debug")
        .build();
    let Ok(config) = result else {
        panic!("build failed");
    };
    assert_eq!(config.host, "10.0.0.1");
    assert_eq!(config.port, 9090);
    assert_eq!(config.grpc_port, 9091);
    assert_eq!(config.ws_port, 9092);
    assert_eq!(config.database_path, "/tmp/test.db");
    assert_eq!(config.log_level, "debug");
}

#[test]
fn config_validation_rejects_zero_port() {
    let config = Config {
        port: 0,
        ..Config::defaults()
    };
    let result = config.validate();
    assert!(result.is_err(), "zero port should fail validation");
}

#[test]
fn config_validation_rejects_port_conflict() {
    let config = Config {
        port: 8080,
        grpc_port: 8080,
        ..Config::defaults()
    };
    let result = config.validate();
    assert!(result.is_err(), "duplicate ports should fail validation");
}

#[test]
fn config_validation_rejects_invalid_log_level() {
    let config = Config {
        log_level: "CRITICAL".into(),
        ..Config::defaults()
    };
    let result = config.validate();
    assert!(result.is_err(), "invalid log level should fail validation");
}

#[test]
fn config_validation_rejects_empty_host() {
    let config = Config {
        host: String::new(),
        ..Config::defaults()
    };
    let result = config.validate();
    assert!(result.is_err(), "empty host should fail validation");
}

#[test]
fn config_serialization_json_roundtrip() {
    let original = Config::defaults();
    let Ok(json_str) = serde_json::to_string(&original) else {
        panic!("serialization failed");
    };
    let Ok(restored) = serde_json::from_str::<Config>(&json_str) else {
        panic!("deserialization failed");
    };
    assert_eq!(original, restored);
}

#[test]
fn config_manager_from_config_returns_same_values() {
    let config = Config::defaults();
    let manager = ConfigManager::from_config(config.clone());
    let retrieved = manager.get();
    assert_eq!(retrieved.port, config.port);
    assert_eq!(retrieved.host, config.host);
    assert_eq!(retrieved.log_level, config.log_level);
}

#[test]
fn config_manager_reload_request_flag_lifecycle() {
    let manager = ConfigManager::from_config(Config::defaults());
    assert!(!manager.reload_requested());
    manager.request_reload();
    assert!(manager.reload_requested());
    manager.clear_reload_request();
    assert!(!manager.reload_requested());
}

#[test]
fn config_manager_validate_returns_valid_for_defaults() {
    let manager = ConfigManager::from_config(Config::defaults());
    let result = manager.validate();
    assert!(result.valid, "default config should validate successfully");
    assert!(result.errors.is_empty());
}

// ===========================================================================
// 3. Metrics (M04)
// ===========================================================================

#[test]
fn metrics_registry_creation_empty() {
    let registry = create_registry();
    assert_eq!(registry.metric_count(), 0);
    assert!(registry.list_metrics().is_empty());
}

#[test]
fn metrics_counter_increment_and_read() {
    let registry = create_registry();
    let counter_result = registry.register_counter(
        "test_requests_total",
        "Total test requests",
        &["method"],
    );
    let Ok(counter) = counter_result else {
        panic!("register failed");
    };

    let labels = Labels::new().with("method", "GET");
    assert_eq!(counter.get(&labels), 0);

    counter.inc(&labels);
    assert_eq!(counter.get(&labels), 1);

    counter.inc_by(&labels, 9);
    assert_eq!(counter.get(&labels), 10);
}

#[test]
fn metrics_gauge_set_and_read() {
    let registry = create_registry();
    let gauge_result = registry.register_gauge(
        "test_health_score",
        "Test health score",
        &["component"],
    );
    let Ok(gauge) = gauge_result else {
        panic!("register failed");
    };

    let labels = Labels::new().with("component", "cpu");
    assert!((gauge.get(&labels)).abs() < FLOAT_TOLERANCE);

    gauge.set(&labels, 0.95);
    assert!((gauge.get(&labels) - 0.95).abs() < 0.001);
}

#[test]
fn metrics_histogram_observe_and_read() {
    let registry = create_registry();
    let histo_result = registry.register_histogram(
        "test_duration_seconds",
        "Test request duration",
        &["endpoint"],
        &[0.01, 0.05, 0.1, 0.5, 1.0],
    );
    let Ok(histo) = histo_result else {
        panic!("register failed");
    };

    let labels = Labels::new().with("endpoint", "/health");
    histo.observe(&labels, 0.03);
    histo.observe(&labels, 0.08);
    histo.observe(&labels, 0.75);

    assert_eq!(histo.get_count(&labels), 3);
    // Sum should be approximately 0.03 + 0.08 + 0.75 = 0.86
    assert!((histo.get_sum(&labels) - 0.86).abs() < 0.01);

    let buckets = histo.get_buckets(&labels);
    assert!(!buckets.is_empty(), "histogram should have buckets after observation");
}

#[test]
fn metrics_export_contains_help_and_type() {
    let registry = create_registry();
    let _ = registry.register_counter("export_test_total", "Export test counter", &[]);
    let _ = registry.register_gauge("export_test_gauge", "Export test gauge", &[]);

    let output = export_metrics(&registry);
    assert!(output.contains("# HELP export_test_total"), "output: {output}");
    assert!(output.contains("# TYPE export_test_total counter"), "output: {output}");
    assert!(output.contains("# HELP export_test_gauge"), "output: {output}");
    assert!(output.contains("# TYPE export_test_gauge gauge"), "output: {output}");
}

#[test]
fn metrics_convenience_increment_counter() {
    let registry = create_registry();
    let _ = registry.register_counter("conv_counter", "Test", &["k"]);
    let result = increment_counter(&registry, "conv_counter", &[("k", "v")]);
    assert!(result.is_ok(), "increment_counter failed: {result:?}");

    let counter = registry.get_counter("conv_counter");
    assert!(counter.is_some());
    if let Some(c) = counter {
        assert_eq!(c.get(&Labels::new().with("k", "v")), 1);
    }
}

#[test]
fn metrics_convenience_set_gauge() {
    let registry = create_registry();
    let _ = registry.register_gauge("conv_gauge", "Test", &["k"]);
    let result = set_gauge(&registry, "conv_gauge", 42.5, &[("k", "v")]);
    assert!(result.is_ok(), "set_gauge failed: {result:?}");

    let gauge = registry.get_gauge("conv_gauge");
    assert!(gauge.is_some());
    if let Some(g) = gauge {
        assert!((g.get(&Labels::new().with("k", "v")) - 42.5).abs() < 0.01);
    }
}

#[test]
fn metrics_convenience_observe_histogram() {
    let registry = create_registry();
    let _ = registry.register_histogram(
        "conv_hist",
        "Test",
        &["k"],
        &DEFAULT_LATENCY_BUCKETS,
    );
    let result = observe_histogram(&registry, "conv_hist", 0.123, &[("k", "v")]);
    assert!(result.is_ok(), "observe_histogram failed: {result:?}");
}

#[test]
fn metrics_convenience_functions_error_on_missing_metric() {
    let registry = create_registry();
    assert!(increment_counter(&registry, "nonexistent", &[]).is_err());
    assert!(set_gauge(&registry, "nonexistent", 1.0, &[]).is_err());
    assert!(observe_histogram(&registry, "nonexistent", 1.0, &[]).is_err());
}

#[test]
fn metrics_duplicate_registration_returns_error() {
    let registry = create_registry();
    let first = registry.register_counter("dup_metric", "First", &[]);
    assert!(first.is_ok());
    let second = registry.register_counter("dup_metric", "Second", &[]);
    assert!(second.is_err(), "duplicate registration should fail");
}

#[test]
fn metrics_registry_with_prefix_prepends_name() {
    let registry = MetricsRegistry::with_prefix("me");
    let _ = registry.register_counter("requests_total", "Total requests", &[]);
    let names = registry.list_metrics();
    assert!(
        names.iter().any(|n| n.starts_with("me_")),
        "prefixed name not found in: {names:?}"
    );
}

#[test]
fn metrics_maintenance_registry_registers_defaults() {
    let registry = create_maintenance_registry();
    let result = register_default_metrics(&registry);
    assert!(result.is_ok(), "default registration failed: {result:?}");

    // Spot-check a few key metrics
    assert!(registry.get_counter("requests_total").is_some());
    assert!(registry.get_counter("errors_total").is_some());
    assert!(registry.get_gauge("health_score").is_some());
    assert!(registry.get_histogram("request_duration_seconds").is_some());
    assert!(registry.metric_count() > 0);
}

#[test]
fn metrics_labels_builder_chain() {
    let labels = Labels::new()
        .service("maintenance-engine")
        .layer("L1")
        .module("M04")
        .tier("1")
        .status("healthy");
    assert!(!labels.is_empty());
}

#[test]
fn metrics_labels_from_pairs() {
    let labels = Labels::from_pairs(&[("method", "POST"), ("path", "/api/health")]);
    assert!(!labels.is_empty());
}

#[test]
fn metrics_snapshot_captures_counter_and_gauge() {
    let registry = create_registry();
    let _ = registry.register_counter("snap_counter", "Test", &[]);
    let _ = registry.register_gauge("snap_gauge", "Test", &[]);

    if let Some(c) = registry.get_counter("snap_counter") {
        c.inc(&Labels::new());
        c.inc(&Labels::new());
    }
    if let Some(g) = registry.get_gauge("snap_gauge") {
        g.set(&Labels::new(), 0.75);
    }

    let snapshot = registry.snapshot();
    assert!(snapshot.timestamp > 0, "timestamp should be positive");
    assert!(!snapshot.counters.is_empty(), "counters snapshot empty");
    assert!(!snapshot.gauges.is_empty(), "gauges snapshot empty");
}

#[test]
fn metrics_default_bucket_constants_are_sorted() {
    for window in DEFAULT_LATENCY_BUCKETS.windows(2) {
        assert!(
            window[0] < window[1],
            "latency buckets not sorted: {} >= {}",
            window[0],
            window[1]
        );
    }
    for window in DEFAULT_SIZE_BUCKETS.windows(2) {
        assert!(
            window[0] < window[1],
            "size buckets not sorted: {} >= {}",
            window[0],
            window[1]
        );
    }
}

// ===========================================================================
// 4. State Persistence (M05)
// ===========================================================================

#[test]
fn state_database_type_all_returns_eleven_variants() {
    let all = DatabaseType::all();
    assert_eq!(all.len(), DATABASE_TYPE_COUNT);
}

#[test]
fn state_database_type_filenames_end_with_db() {
    for db_type in &DatabaseType::all() {
        let filename = db_type.filename();
        let ext = std::path::Path::new(filename)
            .extension()
            .and_then(std::ffi::OsStr::to_str);
        assert_eq!(
            ext,
            Some("db"),
            "{db_type:?} filename does not end with .db: {filename}"
        );
    }
}

#[test]
fn state_database_type_filenames_are_unique() {
    let filenames: HashSet<&str> = DatabaseType::all().iter().map(DatabaseType::filename).collect();
    assert_eq!(filenames.len(), DATABASE_TYPE_COUNT, "duplicate filenames detected");
}

#[test]
fn state_database_type_migration_numbers_are_unique() {
    let numbers: HashSet<u32> = DatabaseType::all().iter().map(DatabaseType::migration_number).collect();
    assert_eq!(numbers.len(), DATABASE_TYPE_COUNT, "duplicate migration numbers");
}

#[test]
fn state_database_type_display_matches_filename() {
    for db_type in &DatabaseType::all() {
        assert_eq!(db_type.to_string(), db_type.filename());
    }
}

#[test]
fn state_database_type_specific_filenames() {
    assert_eq!(DatabaseType::ServiceTracking.filename(), "service_tracking.db");
    assert_eq!(DatabaseType::SystemSynergy.filename(), "system_synergy.db");
    assert_eq!(DatabaseType::HebbianPulse.filename(), "hebbian_pulse.db");
    assert_eq!(DatabaseType::ConsensusTracking.filename(), "consensus_tracking.db");
    assert_eq!(DatabaseType::EpisodicMemory.filename(), "episodic_memory.db");
    assert_eq!(DatabaseType::TensorMemory.filename(), "tensor_memory.db");
    assert_eq!(DatabaseType::PerformanceMetrics.filename(), "performance_metrics.db");
    assert_eq!(DatabaseType::FlowState.filename(), "flow_state.db");
    assert_eq!(DatabaseType::SecurityEvents.filename(), "security_events.db");
    assert_eq!(DatabaseType::WorkflowTracking.filename(), "workflow_tracking.db");
    assert_eq!(DatabaseType::EvolutionTracking.filename(), "evolution_tracking.db");
}

#[test]
fn state_database_config_defaults() {
    let config = DatabaseConfig::default();
    assert_eq!(config.path, "data/maintenance.db");
    assert_eq!(config.max_connections, 10);
    assert_eq!(config.min_connections, 2);
    assert_eq!(config.acquire_timeout_secs, 30);
    assert!(config.wal_mode);
    assert!(config.create_if_missing);
}

#[test]
fn state_database_config_builder_pattern() {
    let config = DatabaseConfig::new("/tmp/test.db")
        .with_max_connections(25)
        .with_min_connections(5)
        .with_acquire_timeout(120)
        .with_wal_mode(false);

    assert_eq!(config.path, "/tmp/test.db");
    assert_eq!(config.max_connections, 25);
    assert_eq!(config.min_connections, 5);
    assert_eq!(config.acquire_timeout_secs, 120);
    assert!(!config.wal_mode);
}

#[test]
fn state_query_builder_select() {
    let query = QueryBuilder::select(&["id", "name", "status"])
        .from("services")
        .where_eq("status", "running")
        .and_eq("tier", "1")
        .order_by("name", "ASC")
        .limit(10)
        .offset(5);

    let built = query.build();
    assert!(built.contains("SELECT id, name, status"), "built: {built}");
    assert!(built.contains("FROM services"), "built: {built}");
    assert!(built.contains("WHERE status = ?"), "built: {built}");
    assert!(built.contains("AND tier = ?"), "built: {built}");
    assert!(built.contains("ORDER BY name ASC"), "built: {built}");
    assert!(built.contains("LIMIT 10"), "built: {built}");
    assert!(built.contains("OFFSET 5"), "built: {built}");
    assert_eq!(query.params(), vec!["running", "1"]);
}

#[test]
fn state_query_builder_insert() {
    let query = QueryBuilder::insert_into("services", &["id", "name", "port"])
        .values(&["synthex", "SYNTHEX Engine", "8090"]);

    let built = query.build();
    assert!(built.contains("INSERT INTO services"), "built: {built}");
    assert!(built.contains("VALUES (?, ?, ?)"), "built: {built}");
    assert_eq!(query.params().len(), 3);
}

#[test]
fn state_query_builder_update() {
    let query = QueryBuilder::update("services")
        .set("status", "stopped")
        .set("health_score", "0.0")
        .where_eq("id", "synthex");

    let built = query.build();
    assert!(built.contains("UPDATE services"), "built: {built}");
    assert!(built.contains("SET status = ?"), "built: {built}");
    assert!(built.contains(", health_score = ?"), "built: {built}");
    assert!(built.contains("WHERE id = ?"), "built: {built}");
    assert_eq!(query.params().len(), 3);
}

#[test]
fn state_query_builder_delete() {
    let query = QueryBuilder::delete_from("services").where_eq("status", "inactive");

    let built = query.build();
    assert!(built.contains("DELETE FROM services"), "built: {built}");
    assert!(built.contains("WHERE status = ?"), "built: {built}");
    assert_eq!(query.params(), vec!["inactive"]);
}

#[test]
fn state_query_builder_or_condition() {
    let query = QueryBuilder::select(&["*"])
        .from("events")
        .where_eq("type", "error")
        .or_eq("type", "warning");

    let built = query.build();
    assert!(built.contains("OR type = ?"), "built: {built}");
    assert_eq!(query.params().len(), 2);
}

// ===========================================================================
// 5. Resources (M06)
// ===========================================================================

#[test]
fn resources_limits_default_values() {
    let limits = ResourceLimits::default();
    assert!((limits.max_cpu_percent - DEFAULT_MAX_CPU).abs() < FLOAT_TOLERANCE);
    assert!((limits.max_memory_percent - DEFAULT_MAX_MEMORY).abs() < FLOAT_TOLERANCE);
    assert!((limits.max_disk_percent - DEFAULT_MAX_DISK).abs() < FLOAT_TOLERANCE);
    assert_eq!(limits.max_open_files, DEFAULT_MAX_OPEN_FILES);
}

#[test]
fn resources_limits_validation_accepts_valid() {
    let limits = ResourceLimits::new(50.0, 60.0, 70.0, 500);
    assert!(limits.validate().is_ok());
}

#[test]
fn resources_limits_validation_rejects_cpu_over_100() {
    let limits = ResourceLimits::new(101.0, 60.0, 70.0, 500);
    assert!(limits.validate().is_err());
}

#[test]
fn resources_limits_validation_rejects_negative_memory() {
    let limits = ResourceLimits::new(50.0, -1.0, 70.0, 500);
    assert!(limits.validate().is_err());
}

#[test]
fn resources_check_limits_no_alerts_within_bounds() {
    let resources = SystemResources {
        cpu_percent: 50.0,
        memory_percent: 60.0,
        disk_percent: 70.0,
        open_files: 200,
        ..SystemResources::default()
    };
    let limits = ResourceLimits::default();
    let alerts = check_limits(&resources, &limits);
    assert!(alerts.is_empty(), "expected no alerts, got: {alerts:?}");
}

#[test]
fn resources_check_limits_cpu_alert_when_exceeded() {
    let resources = SystemResources {
        cpu_percent: 95.0,
        memory_percent: 50.0,
        disk_percent: 50.0,
        open_files: 100,
        ..SystemResources::default()
    };
    let limits = ResourceLimits::default();
    let alerts = check_limits(&resources, &limits);
    assert_eq!(alerts.len(), 1);
    assert!(matches!(alerts[0], ResourceAlert::CpuHigh { .. }));
}

#[test]
fn resources_check_limits_all_resources_exceeded() {
    let resources = SystemResources {
        cpu_percent: 99.0,
        memory_percent: 95.0,
        disk_percent: 98.0,
        open_files: 5000,
        ..SystemResources::default()
    };
    let limits = ResourceLimits::default();
    let alerts = check_limits(&resources, &limits);
    assert_eq!(alerts.len(), 4, "expected 4 alerts, got: {alerts:?}");
}

#[test]
fn resources_alert_display_formats() {
    let cpu = ResourceAlert::CpuHigh {
        current: 90.5,
        threshold: 80.0,
    };
    assert!(cpu.to_string().contains("CPU"));

    let mem = ResourceAlert::MemoryHigh {
        current: 92.0,
        threshold: 85.0,
    };
    assert!(mem.to_string().contains("Memory"));

    let disk = ResourceAlert::DiskHigh {
        current: 95.0,
        threshold: 90.0,
    };
    assert!(disk.to_string().contains("Disk"));

    let files = ResourceAlert::OpenFilesHigh {
        current: 1500,
        threshold: 1000,
    };
    assert!(files.to_string().contains("Open files"));
}

#[test]
fn resources_format_resources_contains_all_sections() {
    let resources = SystemResources {
        cpu_percent: 42.0,
        memory_used_mb: 4096,
        memory_total_mb: 16384,
        memory_percent: 25.0,
        disk_used_mb: 100_000,
        disk_total_mb: 500_000,
        disk_percent: 20.0,
        open_files: 150,
        thread_count: 8,
        ..SystemResources::default()
    };
    let formatted = format_resources(&resources);
    assert!(formatted.contains("CPU"), "formatted: {formatted}");
    assert!(formatted.contains("Memory"), "formatted: {formatted}");
    assert!(formatted.contains("Disk"), "formatted: {formatted}");
    assert!(formatted.contains("Open Files"), "formatted: {formatted}");
    assert!(formatted.contains("Thread Count"), "formatted: {formatted}");
}

#[test]
fn resources_format_alerts_empty_returns_no_alerts_message() {
    let alerts: Vec<ResourceAlert> = vec![];
    let formatted = format_alerts(&alerts);
    assert!(formatted.contains("No resource alerts"), "formatted: {formatted}");
}

#[test]
fn resources_format_alerts_multiple_numbered() {
    let alerts = vec![
        ResourceAlert::CpuHigh {
            current: 90.0,
            threshold: 80.0,
        },
        ResourceAlert::MemoryHigh {
            current: 95.0,
            threshold: 85.0,
        },
    ];
    let formatted = format_alerts(&alerts);
    assert!(formatted.contains("1."), "formatted: {formatted}");
    assert!(formatted.contains("2."), "formatted: {formatted}");
}

#[test]
fn resources_manager_new_is_healthy_with_max_score() {
    let manager = ResourceManager::new();
    assert!(manager.is_healthy());
    assert!((manager.health_score() - 1.0).abs() < FLOAT_TOLERANCE);
}

#[test]
fn resources_manager_with_custom_limits() {
    let limits = ResourceLimits::new(60.0, 70.0, 75.0, 500);
    let manager = ResourceManager::with_limits(limits);
    assert!((manager.limits().max_cpu_percent - 60.0).abs() < FLOAT_TOLERANCE);
    assert!((manager.limits().max_memory_percent - 70.0).abs() < FLOAT_TOLERANCE);
}

#[test]
fn resources_manager_set_limits_validates() {
    let mut manager = ResourceManager::new();

    let valid = ResourceLimits::new(50.0, 60.0, 70.0, 400);
    assert!(manager.set_limits(valid).is_ok());

    let invalid = ResourceLimits::new(200.0, 60.0, 70.0, 400);
    assert!(manager.set_limits(invalid).is_err());
}

#[test]
fn resources_manager_utilization_summary_empty_without_snapshot() {
    let manager = ResourceManager::new();
    let summary = manager.utilization_summary();
    assert!(summary.is_empty());
}

#[test]
fn resources_process_info_default_values() {
    let info = ProcessInfo::default();
    assert_eq!(info.pid, 0);
    assert_eq!(info.thread_count, 1);
    assert_eq!(info.open_files, 0);
    assert_eq!(info.virtual_memory_bytes, 0);
    assert_eq!(info.resident_memory_bytes, 0);
    assert!(info.start_time.is_none());
}

// ===========================================================================
// 6. Logging (M03)
// ===========================================================================

#[test]
fn logging_correlation_id_is_uuid_length() {
    let id = generate_correlation_id();
    assert_eq!(id.len(), CORRELATION_ID_LEN, "id: {id}");
}

#[test]
fn logging_correlation_id_uniqueness() {
    let mut ids = HashSet::new();
    for _ in 0..100 {
        let id = generate_correlation_id();
        assert!(ids.insert(id), "duplicate correlation ID generated");
    }
    assert_eq!(ids.len(), 100);
}

#[test]
fn logging_short_correlation_id_length() {
    let id = generate_short_correlation_id();
    assert_eq!(id.len(), SHORT_CORRELATION_ID_LEN, "id: {id}");
}

#[test]
fn logging_log_context_new_has_correlation_id() {
    let ctx = LogContext::new();
    assert!(!ctx.correlation_id.is_empty());
    assert!(ctx.service_id.is_none());
    assert!(ctx.layer.is_none());
    assert!(ctx.module.is_none());
}

#[test]
fn logging_log_context_with_context_populates_fields() {
    let ctx = LogContext::with_context("maintenance-engine", "L1", "M03");
    assert!(!ctx.correlation_id.is_empty());
    assert_eq!(ctx.service_id.as_deref(), Some("maintenance-engine"));
    assert_eq!(ctx.layer.as_deref(), Some("L1"));
    assert_eq!(ctx.module.as_deref(), Some("M03"));
}

#[test]
fn logging_log_context_child_inherits_correlation_id() {
    let parent = LogContext::with_context("svc", "L1", "M01");
    let child = parent.child();
    assert_eq!(child.correlation_id, parent.correlation_id);
    assert_eq!(child.service_id, parent.service_id);
}

#[test]
fn logging_log_context_with_module_preserves_correlation() {
    let ctx = LogContext::with_context("svc", "L1", "M01");
    let derived = ctx.with_module("M06");
    assert_eq!(derived.correlation_id, ctx.correlation_id);
    assert_eq!(derived.module.as_deref(), Some("M06"));
    assert_eq!(derived.layer.as_deref(), Some("L1"));
}

#[test]
fn logging_log_context_with_layer_preserves_correlation() {
    let ctx = LogContext::with_context("svc", "L1", "M01");
    let derived = ctx.with_layer("L2");
    assert_eq!(derived.correlation_id, ctx.correlation_id);
    assert_eq!(derived.layer.as_deref(), Some("L2"));
    assert_eq!(derived.module.as_deref(), Some("M01"));
}

#[test]
fn logging_log_context_display_format() {
    let ctx = LogContext::with_context("me", "L1", "M03");
    let display = ctx.to_string();
    assert!(display.contains("corr_id="), "display: {display}");
    assert!(display.contains("service=me"), "display: {display}");
    assert!(display.contains("layer=L1"), "display: {display}");
    assert!(display.contains("module=M03"), "display: {display}");
}

#[test]
fn logging_log_format_parse_all_variants() {
    assert!(matches!(LogFormat::parse_str("json"), Ok(LogFormat::Json)));
    assert!(matches!(LogFormat::parse_str("JSON"), Ok(LogFormat::Json)));
    assert!(matches!(LogFormat::parse_str("pretty"), Ok(LogFormat::Pretty)));
    assert!(matches!(LogFormat::parse_str("compact"), Ok(LogFormat::Compact)));
    assert!(LogFormat::parse_str("xml").is_err());
}

#[test]
fn logging_log_format_display_roundtrip() {
    let formats = [LogFormat::Json, LogFormat::Pretty, LogFormat::Compact];
    for fmt in &formats {
        let s = fmt.to_string();
        let parsed = LogFormat::parse_str(&s);
        assert!(parsed.is_ok(), "roundtrip failed for {fmt:?}");
        if let Ok(p) = parsed {
            assert_eq!(*fmt, p, "roundtrip mismatch for {s}");
        }
    }
}

#[test]
fn logging_log_level_parse_all_variants() {
    assert!(matches!(LogLevel::parse_str("trace"), Ok(LogLevel::Trace)));
    assert!(matches!(LogLevel::parse_str("debug"), Ok(LogLevel::Debug)));
    assert!(matches!(LogLevel::parse_str("info"), Ok(LogLevel::Info)));
    assert!(matches!(LogLevel::parse_str("INFO"), Ok(LogLevel::Info)));
    assert!(matches!(LogLevel::parse_str("warn"), Ok(LogLevel::Warn)));
    assert!(matches!(LogLevel::parse_str("warning"), Ok(LogLevel::Warn)));
    assert!(matches!(LogLevel::parse_str("error"), Ok(LogLevel::Error)));
    assert!(LogLevel::parse_str("fatal").is_err());
}

#[test]
fn logging_log_level_ordering() {
    assert!(LogLevel::Trace < LogLevel::Debug);
    assert!(LogLevel::Debug < LogLevel::Info);
    assert!(LogLevel::Info < LogLevel::Warn);
    assert!(LogLevel::Warn < LogLevel::Error);
}

#[test]
fn logging_log_config_default_values() {
    let config = LogConfig::default();
    assert_eq!(config.level, "info");
    assert_eq!(config.format, LogFormat::Pretty);
    assert!(config.include_timestamps);
    assert!(config.include_targets);
    assert!(!config.include_file_line);
    assert!(!config.include_thread_ids);
    assert!(!config.include_span_events);
}

#[test]
fn logging_log_config_development_preset() {
    let config = LogConfig::development();
    assert_eq!(config.level, "debug");
    assert_eq!(config.format, LogFormat::Pretty);
    assert!(config.include_file_line);
}

#[test]
fn logging_log_config_production_preset() {
    let config = LogConfig::production();
    assert_eq!(config.level, "info");
    assert_eq!(config.format, LogFormat::Json);
    assert!(config.include_thread_ids);
    assert!(!config.include_file_line);
}

#[test]
fn logging_log_config_validate_accepts_valid() {
    let config = LogConfig::default();
    assert!(config.validate().is_ok());
}

#[test]
fn logging_log_config_validate_rejects_invalid_level() {
    let config = LogConfig {
        level: "INVALID".to_string(),
        ..LogConfig::default()
    };
    assert!(config.validate().is_err());
}

// ===========================================================================
// 7. Cross-Module Integration
// ===========================================================================

#[test]
fn cross_config_affects_resource_limits_concept() {
    // Demonstrate that configuration values can drive resource limit decisions.
    // In a real deployment, config would set thresholds; here we verify the
    // pattern works across module boundaries.
    let config = ConfigBuilder::new()
        .skip_files()
        .skip_env()
        .log_level("warn")
        .build();
    let Ok(config) = config else {
        panic!("config build failed");
    };

    // Resource limits could be driven by config; verify both are accessible
    let limits = ResourceLimits::new(
        DEFAULT_MAX_CPU,
        DEFAULT_MAX_MEMORY,
        DEFAULT_MAX_DISK,
        DEFAULT_MAX_OPEN_FILES,
    );
    assert!(limits.validate().is_ok());

    // Config log level valid
    assert_eq!(config.log_level, "warn");
}

#[test]
fn cross_metrics_and_resource_monitoring() {
    // Verify metrics can record resource states
    let registry = create_registry();
    let gauge_result = registry.register_gauge(
        "system_cpu_percent",
        "CPU usage percentage",
        &["host"],
    );
    let Ok(gauge) = gauge_result else {
        panic!("register failed");
    };

    let resources = SystemResources {
        cpu_percent: 55.0,
        memory_percent: 65.0,
        ..SystemResources::default()
    };

    // Record resource metrics
    let labels = Labels::new().with("host", "localhost");
    gauge.set(&labels, resources.cpu_percent);
    assert!((gauge.get(&labels) - 55.0).abs() < 0.01);
}

#[test]
fn cross_logging_context_with_config_service() {
    let config = Config::defaults();
    // Use config host in a log context
    let ctx = LogContext::with_context(&config.host, "L1", "M02");
    assert_eq!(ctx.service_id.as_deref(), Some("0.0.0.0"));
    assert!(!ctx.correlation_id.is_empty());
}

#[test]
fn cross_error_taxonomy_used_in_config_validation() {
    // Config validation returns Error::Validation
    let config = Config {
        port: 0,
        ..Config::defaults()
    };
    let result = config.validate();
    match result {
        Err(Error::Validation(msg)) => {
            assert!(msg.contains("port cannot be zero"), "msg: {msg}");
        }
        Ok(()) => panic!("expected validation error for zero port"),
        Err(other) => panic!("expected Validation error, got: {other}"),
    }
}

#[test]
fn cross_error_taxonomy_used_in_resource_validation() {
    let limits = ResourceLimits::new(-10.0, 50.0, 50.0, 100);
    let result = limits.validate();
    match result {
        Err(Error::Validation(msg)) => {
            assert!(msg.contains("max_cpu_percent"), "msg: {msg}");
        }
        Ok(()) => panic!("expected validation error for negative CPU limit"),
        Err(other) => panic!("expected Validation error, got: {other}"),
    }
}

#[test]
fn cross_database_types_cover_all_engine_domains() {
    // Verify the 11 database types map to distinct domains
    let all = DatabaseType::all();
    let filenames: Vec<&str> = all.iter().map(DatabaseType::filename).collect();

    // Spot-check domain coverage
    assert!(filenames.contains(&"service_tracking.db"), "missing service_tracking");
    assert!(filenames.contains(&"hebbian_pulse.db"), "missing hebbian_pulse");
    assert!(filenames.contains(&"consensus_tracking.db"), "missing consensus_tracking");
    assert!(filenames.contains(&"tensor_memory.db"), "missing tensor_memory");
    assert!(filenames.contains(&"security_events.db"), "missing security_events");
    assert!(filenames.contains(&"evolution_tracking.db"), "missing evolution_tracking");
}
