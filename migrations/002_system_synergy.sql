-- ============================================================================
-- Migration: 002_system_synergy.sql
-- SYNTHEX Database Migration - System Synergy Module
-- Version: 1.0.0
-- Pattern: Cross-system integration tracking and synergy metrics
-- ============================================================================

INSERT INTO schema_versions (version, migration_name, checksum)
VALUES ('002', 'system_synergy', 'sha256:002_system_synergy');

-- ============================================================================
-- SYSTEM CONNECTIONS TABLE
-- Tracks connections between different systems with synergy scores
-- ============================================================================
CREATE TABLE IF NOT EXISTS system_connections (
    id INTEGER PRIMARY KEY AUTOINCREMENT,

    -- Connection Endpoints
    source_system TEXT NOT NULL,
    target_system TEXT NOT NULL,
    connection_name TEXT,

    -- Connection Type
    connection_type TEXT NOT NULL DEFAULT 'sync'
        CHECK(connection_type IN (
            'sync', 'async', 'streaming', 'batch', 'event', 'rpc',
            'pubsub', 'request_reply', 'fire_forget', 'bidirectional'
        )),

    -- Synergy Metrics
    synergy_score REAL DEFAULT 0.0 CHECK(synergy_score >= 0.0 AND synergy_score <= 100.0),
    synergy_trend TEXT DEFAULT 'stable'
        CHECK(synergy_trend IN ('improving', 'stable', 'degrading', 'unknown')),

    -- Performance Metrics
    avg_latency_ms REAL DEFAULT 0.0,
    p50_latency_ms REAL DEFAULT 0.0,
    p95_latency_ms REAL DEFAULT 0.0,
    p99_latency_ms REAL DEFAULT 0.0,
    max_latency_ms REAL DEFAULT 0.0,

    -- Throughput Metrics
    requests_per_second REAL DEFAULT 0.0,
    bytes_per_second REAL DEFAULT 0.0,
    peak_rps REAL DEFAULT 0.0,

    -- Reliability Metrics
    success_rate REAL DEFAULT 100.0 CHECK(success_rate >= 0.0 AND success_rate <= 100.0),
    error_rate REAL DEFAULT 0.0 CHECK(error_rate >= 0.0 AND error_rate <= 100.0),
    timeout_rate REAL DEFAULT 0.0 CHECK(timeout_rate >= 0.0 AND timeout_rate <= 100.0),
    retry_rate REAL DEFAULT 0.0,

    -- Circuit Breaker
    circuit_state TEXT DEFAULT 'closed'
        CHECK(circuit_state IN ('closed', 'open', 'half_open')),
    circuit_failure_threshold INTEGER DEFAULT 5,
    circuit_recovery_timeout_ms INTEGER DEFAULT 30000,
    last_circuit_trip DATETIME,

    -- Connection Configuration
    timeout_ms INTEGER DEFAULT 5000,
    retry_count INTEGER DEFAULT 3,
    retry_backoff_ms INTEGER DEFAULT 1000,
    max_connections INTEGER DEFAULT 100,
    keepalive_enabled INTEGER DEFAULT 1,
    compression_enabled INTEGER DEFAULT 0,

    -- Health Assessment (Generated)
    health_score REAL GENERATED ALWAYS AS (
        (success_rate * 0.4) +
        ((100 - LEAST(error_rate * 10, 100)) * 0.3) +
        ((100 - LEAST(avg_latency_ms / 10, 100)) * 0.3)
    ) STORED,
    is_healthy INTEGER GENERATED ALWAYS AS (
        CASE WHEN success_rate > 95 AND circuit_state = 'closed' THEN 1 ELSE 0 END
    ) STORED,

    -- Metadata
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    last_activity DATETIME DEFAULT CURRENT_TIMESTAMP,
    metadata TEXT, -- JSON blob

    UNIQUE(source_system, target_system, connection_type)
);

-- ============================================================================
-- INTEGRATION EVENTS TABLE
-- Captures cross-system integration events for observability
-- ============================================================================
CREATE TABLE IF NOT EXISTS integration_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,

    -- Event Identification
    event_id TEXT NOT NULL UNIQUE DEFAULT (lower(hex(randomblob(16)))),
    trace_id TEXT,
    span_id TEXT,
    parent_span_id TEXT,

    -- Source and Target
    source TEXT NOT NULL,
    target TEXT NOT NULL,

    -- Event Classification
    event_category TEXT NOT NULL
        CHECK(event_category IN (
            'request', 'response', 'error', 'timeout', 'retry',
            'circuit_break', 'fallback', 'cache_hit', 'cache_miss',
            'rate_limit', 'auth_failure', 'validation_error',
            'transformation', 'routing', 'custom'
        )),
    event_subcategory TEXT,

    -- Event Data
    event_data TEXT, -- JSON blob
    request_data TEXT, -- JSON blob (sanitized)
    response_data TEXT, -- JSON blob (sanitized)
    error_message TEXT,
    error_code TEXT,
    stack_trace TEXT,

    -- Timing
    timestamp DATETIME DEFAULT CURRENT_TIMESTAMP,
    latency_ms REAL,
    queue_time_ms REAL,
    processing_time_ms REAL,

    -- Request Metadata
    http_method TEXT,
    http_status INTEGER,
    endpoint TEXT,
    content_type TEXT,
    content_length INTEGER,

    -- Processing State
    processed INTEGER DEFAULT 0,
    processed_at DATETIME,
    acknowledged INTEGER DEFAULT 0,
    acknowledged_by TEXT,

    -- Severity
    severity TEXT DEFAULT 'info'
        CHECK(severity IN ('debug', 'info', 'warning', 'error', 'critical')),

    -- Generated Columns
    is_error INTEGER GENERATED ALWAYS AS (
        CASE WHEN event_category IN ('error', 'timeout', 'circuit_break', 'auth_failure') THEN 1 ELSE 0 END
    ) STORED,
    is_slow INTEGER GENERATED ALWAYS AS (
        CASE WHEN latency_ms > 1000 THEN 1 ELSE 0 END
    ) STORED
);

-- ============================================================================
-- SYNERGY METRICS TABLE
-- Aggregated synergy measurements between system pairs
-- ============================================================================
CREATE TABLE IF NOT EXISTS synergy_metrics (
    id INTEGER PRIMARY KEY AUTOINCREMENT,

    -- System Pair
    system_pair TEXT NOT NULL, -- Format: "system_a:system_b"
    source_system TEXT NOT NULL,
    target_system TEXT NOT NULL,

    -- Core Synergy Score
    synergy_score REAL NOT NULL DEFAULT 0.0 CHECK(synergy_score >= 0.0 AND synergy_score <= 100.0),
    synergy_grade TEXT GENERATED ALWAYS AS (
        CASE
            WHEN synergy_score >= 95 THEN 'A+'
            WHEN synergy_score >= 90 THEN 'A'
            WHEN synergy_score >= 85 THEN 'B+'
            WHEN synergy_score >= 80 THEN 'B'
            WHEN synergy_score >= 75 THEN 'C+'
            WHEN synergy_score >= 70 THEN 'C'
            WHEN synergy_score >= 60 THEN 'D'
            ELSE 'F'
        END
    ) STORED,

    -- Interaction Statistics
    interaction_count INTEGER DEFAULT 0,
    success_count INTEGER DEFAULT 0,
    failure_count INTEGER DEFAULT 0,
    success_rate REAL GENERATED ALWAYS AS (
        CASE WHEN interaction_count > 0
        THEN CAST(success_count AS REAL) / interaction_count * 100
        ELSE 0.0 END
    ) STORED,

    -- Measurement Window
    measurement_window TEXT NOT NULL DEFAULT '1h'
        CHECK(measurement_window IN ('1m', '5m', '15m', '1h', '6h', '1d', '7d', '30d')),
    window_start DATETIME NOT NULL,
    window_end DATETIME NOT NULL,

    -- Component Scores
    latency_score REAL DEFAULT 0.0,
    reliability_score REAL DEFAULT 0.0,
    throughput_score REAL DEFAULT 0.0,
    consistency_score REAL DEFAULT 0.0,

    -- Trend Analysis
    previous_score REAL,
    score_delta REAL GENERATED ALWAYS AS (synergy_score - COALESCE(previous_score, synergy_score)) STORED,
    trend_direction TEXT GENERATED ALWAYS AS (
        CASE
            WHEN synergy_score - COALESCE(previous_score, synergy_score) > 2 THEN 'up'
            WHEN synergy_score - COALESCE(previous_score, synergy_score) < -2 THEN 'down'
            ELSE 'stable'
        END
    ) STORED,

    -- Anomaly Detection
    baseline_score REAL,
    standard_deviation REAL,
    is_anomaly INTEGER GENERATED ALWAYS AS (
        CASE WHEN baseline_score IS NOT NULL AND standard_deviation IS NOT NULL
             AND ABS(synergy_score - baseline_score) > (2 * standard_deviation)
        THEN 1 ELSE 0 END
    ) STORED,

    -- Metadata
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    metadata TEXT, -- JSON blob

    UNIQUE(system_pair, measurement_window, window_start)
);

-- ============================================================================
-- DATA FLOWS TABLE
-- Tracks data movement between systems
-- ============================================================================
CREATE TABLE IF NOT EXISTS data_flows (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    flow_id TEXT NOT NULL UNIQUE DEFAULT (lower(hex(randomblob(16)))),

    -- Flow Endpoints
    source TEXT NOT NULL,
    target TEXT NOT NULL,
    flow_name TEXT,

    -- Flow Classification
    flow_type TEXT NOT NULL DEFAULT 'sync'
        CHECK(flow_type IN (
            'sync', 'async', 'streaming', 'batch', 'etl',
            'cdc', 'replication', 'backup', 'archive', 'custom'
        )),
    data_type TEXT DEFAULT 'mixed'
        CHECK(data_type IN ('json', 'binary', 'text', 'avro', 'protobuf', 'parquet', 'mixed')),

    -- Volume Metrics
    data_size_kb REAL DEFAULT 0.0,
    record_count INTEGER DEFAULT 0,
    message_count INTEGER DEFAULT 0,

    -- Bandwidth Metrics
    bandwidth_mbps REAL DEFAULT 0.0,
    peak_bandwidth_mbps REAL DEFAULT 0.0,
    avg_bandwidth_mbps REAL DEFAULT 0.0,

    -- Throughput
    records_per_second REAL DEFAULT 0.0,
    messages_per_second REAL DEFAULT 0.0,
    bytes_per_second REAL DEFAULT 0.0,

    -- Quality Metrics
    compression_ratio REAL DEFAULT 1.0,
    error_rate REAL DEFAULT 0.0,
    retry_rate REAL DEFAULT 0.0,
    duplicate_rate REAL DEFAULT 0.0,

    -- Flow State
    status TEXT DEFAULT 'active'
        CHECK(status IN ('active', 'paused', 'stopped', 'error', 'backpressure')),
    last_transfer DATETIME,
    last_success DATETIME,
    last_failure DATETIME,

    -- Backpressure Management
    backpressure_enabled INTEGER DEFAULT 1,
    queue_depth INTEGER DEFAULT 0,
    max_queue_depth INTEGER DEFAULT 10000,
    backpressure_threshold REAL DEFAULT 0.8,
    is_backpressured INTEGER GENERATED ALWAYS AS (
        CASE WHEN queue_depth > (max_queue_depth * backpressure_threshold) THEN 1 ELSE 0 END
    ) STORED,

    -- Data Quality Score (Generated)
    quality_score REAL GENERATED ALWAYS AS (
        (100 - (error_rate * 10)) * (1 - duplicate_rate) * (1 - retry_rate * 0.5)
    ) STORED,

    -- Metadata
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    metadata TEXT -- JSON blob
);

-- ============================================================================
-- INDEXES FOR PERFORMANCE
-- ============================================================================

-- System Connections Indexes
CREATE INDEX IF NOT EXISTS idx_conn_source ON system_connections(source_system);
CREATE INDEX IF NOT EXISTS idx_conn_target ON system_connections(target_system);
CREATE INDEX IF NOT EXISTS idx_conn_type ON system_connections(connection_type);
CREATE INDEX IF NOT EXISTS idx_conn_synergy ON system_connections(synergy_score DESC);
CREATE INDEX IF NOT EXISTS idx_conn_health ON system_connections(is_healthy);
CREATE INDEX IF NOT EXISTS idx_conn_circuit ON system_connections(circuit_state) WHERE circuit_state != 'closed';
CREATE INDEX IF NOT EXISTS idx_conn_activity ON system_connections(last_activity DESC);

-- Integration Events Indexes
CREATE INDEX IF NOT EXISTS idx_int_events_source ON integration_events(source);
CREATE INDEX IF NOT EXISTS idx_int_events_target ON integration_events(target);
CREATE INDEX IF NOT EXISTS idx_int_events_timestamp ON integration_events(timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_int_events_category ON integration_events(event_category);
CREATE INDEX IF NOT EXISTS idx_int_events_trace ON integration_events(trace_id) WHERE trace_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_int_events_errors ON integration_events(is_error) WHERE is_error = 1;
CREATE INDEX IF NOT EXISTS idx_int_events_slow ON integration_events(is_slow) WHERE is_slow = 1;
CREATE INDEX IF NOT EXISTS idx_int_events_unprocessed ON integration_events(processed) WHERE processed = 0;

-- Synergy Metrics Indexes
CREATE INDEX IF NOT EXISTS idx_synergy_pair ON synergy_metrics(system_pair);
CREATE INDEX IF NOT EXISTS idx_synergy_window ON synergy_metrics(measurement_window);
CREATE INDEX IF NOT EXISTS idx_synergy_score ON synergy_metrics(synergy_score DESC);
CREATE INDEX IF NOT EXISTS idx_synergy_anomaly ON synergy_metrics(is_anomaly) WHERE is_anomaly = 1;
CREATE INDEX IF NOT EXISTS idx_synergy_time ON synergy_metrics(window_start, window_end);

-- Data Flows Indexes
CREATE INDEX IF NOT EXISTS idx_flows_source ON data_flows(source);
CREATE INDEX IF NOT EXISTS idx_flows_target ON data_flows(target);
CREATE INDEX IF NOT EXISTS idx_flows_type ON data_flows(flow_type);
CREATE INDEX IF NOT EXISTS idx_flows_status ON data_flows(status);
CREATE INDEX IF NOT EXISTS idx_flows_backpressure ON data_flows(is_backpressured) WHERE is_backpressured = 1;
CREATE INDEX IF NOT EXISTS idx_flows_quality ON data_flows(quality_score);

-- ============================================================================
-- TRIGGERS FOR AUTOMATIC UPDATES
-- ============================================================================

-- Update system_connections.updated_at on modification
CREATE TRIGGER IF NOT EXISTS trg_conn_updated_at
AFTER UPDATE ON system_connections
BEGIN
    UPDATE system_connections SET updated_at = CURRENT_TIMESTAMP WHERE id = NEW.id;
END;

-- Update data_flows.updated_at on modification
CREATE TRIGGER IF NOT EXISTS trg_flows_updated_at
AFTER UPDATE ON data_flows
BEGIN
    UPDATE data_flows SET updated_at = CURRENT_TIMESTAMP WHERE id = NEW.id;
END;

-- Auto-create integration event on circuit state change
CREATE TRIGGER IF NOT EXISTS trg_circuit_state_changed
AFTER UPDATE OF circuit_state ON system_connections
WHEN OLD.circuit_state != NEW.circuit_state
BEGIN
    INSERT INTO integration_events (source, target, event_category, event_data, severity)
    VALUES (
        NEW.source_system,
        NEW.target_system,
        'circuit_break',
        json_object('old_state', OLD.circuit_state, 'new_state', NEW.circuit_state),
        CASE NEW.circuit_state
            WHEN 'open' THEN 'error'
            WHEN 'half_open' THEN 'warning'
            ELSE 'info'
        END
    );
END;

-- ============================================================================
-- WORKFLOW INTEGRATION TABLES
-- ============================================================================

-- Workflow Synergy Impact (tracks how workflows affect system synergy)
CREATE TABLE IF NOT EXISTS workflow_synergy_impact (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    workflow_instance_id TEXT NOT NULL,  -- References workflow_tracking.workflow_instances
    connection_id INTEGER NOT NULL,

    -- Synergy measurements
    synergy_before REAL NOT NULL,
    synergy_after REAL,
    synergy_delta REAL GENERATED ALWAYS AS (COALESCE(synergy_after, synergy_before) - synergy_before) STORED,

    -- Impact assessment
    impact_type TEXT CHECK(impact_type IN ('positive', 'negative', 'neutral', 'unknown')),
    impact_magnitude REAL CHECK(impact_magnitude >= 0.0 AND impact_magnitude <= 100.0),

    -- Timing
    measured_at DATETIME DEFAULT CURRENT_TIMESTAMP,

    FOREIGN KEY (connection_id) REFERENCES system_connections(id) ON DELETE CASCADE
);

-- System Integration Tasks (workflow-driven integration operations)
CREATE TABLE IF NOT EXISTS integration_tasks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    task_id TEXT NOT NULL UNIQUE DEFAULT (lower(hex(randomblob(16)))),
    workflow_instance_id TEXT,  -- References workflow_tracking.workflow_instances

    -- Task details
    source_system TEXT NOT NULL,
    target_system TEXT NOT NULL,
    operation_type TEXT NOT NULL
        CHECK(operation_type IN ('connect', 'disconnect', 'reconfigure', 'health_check', 'sync', 'migrate')),

    -- Status
    status TEXT DEFAULT 'pending'
        CHECK(status IN ('pending', 'running', 'completed', 'failed', 'rolled_back')),

    -- Configuration
    config TEXT,  -- JSON

    -- Results
    result TEXT,  -- JSON
    error_message TEXT,

    -- Timing
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    started_at DATETIME,
    completed_at DATETIME
);

-- Indexes for workflow integration
CREATE INDEX IF NOT EXISTS idx_workflow_synergy_workflow ON workflow_synergy_impact(workflow_instance_id);
CREATE INDEX IF NOT EXISTS idx_workflow_synergy_connection ON workflow_synergy_impact(connection_id);
CREATE INDEX IF NOT EXISTS idx_integration_tasks_workflow ON integration_tasks(workflow_instance_id);
CREATE INDEX IF NOT EXISTS idx_integration_tasks_status ON integration_tasks(status);

-- ============================================================================
-- CROSS-DATABASE VIEWS
-- ============================================================================

-- System synergy overview for workflow decisions
CREATE VIEW IF NOT EXISTS v_synergy_overview AS
SELECT
    sc.source_system,
    sc.target_system,
    sc.connection_type,
    sc.synergy_score,
    sc.health_score,
    sc.is_healthy,
    sc.circuit_state,
    sc.avg_latency_ms,
    sc.success_rate,
    sc.error_rate,
    (SELECT COUNT(*) FROM workflow_synergy_impact wsi
     WHERE wsi.connection_id = sc.id
     AND wsi.measured_at > datetime('now', '-24 hours')) AS recent_workflow_impacts
FROM system_connections sc
ORDER BY sc.synergy_score DESC;

-- Cross-system health matrix
CREATE VIEW IF NOT EXISTS v_system_health_matrix AS
SELECT
    sc.source_system,
    sc.target_system,
    sc.synergy_score,
    sc.circuit_state,
    CASE
        WHEN sc.circuit_state = 'open' THEN 'CRITICAL'
        WHEN sc.synergy_score < 50 THEN 'DEGRADED'
        WHEN sc.synergy_score < 75 THEN 'WARNING'
        ELSE 'HEALTHY'
    END AS connection_status,
    (SELECT sm.synergy_score FROM synergy_metrics sm
     WHERE sm.source_system = sc.source_system
     AND sm.target_system = sc.target_system
     ORDER BY sm.window_start DESC LIMIT 1) AS latest_metric_score
FROM system_connections sc
WHERE sc.is_healthy IS NOT NULL;

-- Synergy trends for learning integration
CREATE VIEW IF NOT EXISTS v_synergy_trends AS
SELECT
    sm.system_pair,
    sm.measurement_window,
    sm.synergy_score,
    sm.previous_score,
    sm.score_delta,
    sm.trend_direction,
    sm.is_anomaly,
    sm.window_start,
    sm.window_end
FROM synergy_metrics sm
WHERE sm.window_start > datetime('now', '-7 days')
ORDER BY sm.system_pair, sm.window_start DESC;

-- ============================================================================
-- WORKFLOW INTEGRATION TRIGGERS
-- ============================================================================

-- Track synergy changes for workflow learning
CREATE TRIGGER IF NOT EXISTS trg_synergy_change_track
AFTER UPDATE OF synergy_score ON system_connections
WHEN ABS(OLD.synergy_score - NEW.synergy_score) > 5.0
BEGIN
    INSERT INTO integration_events (
        source, target, event_category, event_data, severity
    )
    VALUES (
        NEW.source_system,
        NEW.target_system,
        CASE
            WHEN NEW.synergy_score > OLD.synergy_score THEN 'custom'
            ELSE 'error'
        END,
        json_object(
            'event_type', 'synergy_change',
            'old_score', OLD.synergy_score,
            'new_score', NEW.synergy_score,
            'delta', NEW.synergy_score - OLD.synergy_score
        ),
        CASE
            WHEN NEW.synergy_score < 50 THEN 'error'
            WHEN NEW.synergy_score < 75 THEN 'warning'
            ELSE 'info'
        END
    );
END;

-- ============================================================================
-- End of Migration 002_system_synergy.sql
-- ============================================================================
