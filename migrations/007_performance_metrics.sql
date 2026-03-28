-- Migration: 007_performance_metrics.sql
-- Purpose: Performance tracking and SLO monitoring
-- Database: performance_metrics.db

--------------------------------------------------------------------------------
-- SERVICE PERFORMANCE METRICS
--------------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS performance_samples (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    sample_id TEXT NOT NULL UNIQUE,
    service_id TEXT NOT NULL,
    timestamp TEXT NOT NULL DEFAULT (datetime('now')),

    -- Resource metrics
    cpu_percent REAL NOT NULL CHECK (cpu_percent BETWEEN 0.0 AND 100.0),
    memory_mb REAL NOT NULL CHECK (memory_mb >= 0),
    memory_percent REAL CHECK (memory_percent BETWEEN 0.0 AND 100.0),
    disk_io_read_mb REAL DEFAULT 0,
    disk_io_write_mb REAL DEFAULT 0,
    network_rx_mb REAL DEFAULT 0,
    network_tx_mb REAL DEFAULT 0,

    -- Latency metrics (milliseconds)
    avg_response_ms REAL,
    p50_latency_ms REAL,
    p95_latency_ms REAL,
    p99_latency_ms REAL,
    max_latency_ms REAL,

    -- Throughput metrics
    requests_per_second REAL DEFAULT 0,
    active_connections INTEGER DEFAULT 0,
    queue_depth INTEGER DEFAULT 0,

    -- Error metrics
    error_count INTEGER DEFAULT 0,
    timeout_count INTEGER DEFAULT 0,

    FOREIGN KEY (service_id) REFERENCES services(id)
);

--------------------------------------------------------------------------------
-- SLO DEFINITIONS
--------------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS slo_definitions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    slo_id TEXT NOT NULL UNIQUE,
    service_id TEXT NOT NULL,
    metric_name TEXT NOT NULL,
    target_value REAL NOT NULL,
    comparison TEXT NOT NULL CHECK (comparison IN ('lt', 'lte', 'gt', 'gte', 'eq')),
    time_window_minutes INTEGER NOT NULL DEFAULT 60,
    burn_rate_threshold REAL NOT NULL DEFAULT 1.0,
    is_active INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),

    UNIQUE(service_id, metric_name),
    FOREIGN KEY (service_id) REFERENCES services(id)
);

--------------------------------------------------------------------------------
-- SLO VIOLATIONS
--------------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS slo_violations (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    violation_id TEXT NOT NULL UNIQUE,
    slo_id TEXT NOT NULL,
    service_id TEXT NOT NULL,
    detected_at TEXT NOT NULL DEFAULT (datetime('now')),
    resolved_at TEXT,
    actual_value REAL NOT NULL,
    target_value REAL NOT NULL,
    duration_minutes INTEGER,
    severity TEXT NOT NULL CHECK (severity IN ('warning', 'critical', 'emergency')),
    burn_rate REAL,
    is_resolved INTEGER NOT NULL DEFAULT 0,

    FOREIGN KEY (slo_id) REFERENCES slo_definitions(slo_id),
    FOREIGN KEY (service_id) REFERENCES services(id)
);

--------------------------------------------------------------------------------
-- ERROR BUDGET TRACKING
--------------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS error_budgets (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    budget_id TEXT NOT NULL UNIQUE,
    service_id TEXT NOT NULL,
    period_start TEXT NOT NULL,
    period_end TEXT NOT NULL,
    total_budget_minutes REAL NOT NULL,
    consumed_minutes REAL NOT NULL DEFAULT 0,
    remaining_minutes REAL GENERATED ALWAYS AS (total_budget_minutes - consumed_minutes) STORED,
    budget_percent_remaining REAL GENERATED ALWAYS AS (
        CASE
            WHEN total_budget_minutes > 0
            THEN ((total_budget_minutes - consumed_minutes) / total_budget_minutes) * 100
            ELSE 0
        END
    ) STORED,
    is_exhausted INTEGER GENERATED ALWAYS AS (consumed_minutes >= total_budget_minutes) STORED,

    UNIQUE(service_id, period_start),
    FOREIGN KEY (service_id) REFERENCES services(id)
);

--------------------------------------------------------------------------------
-- PERFORMANCE BASELINES
--------------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS performance_baselines (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    baseline_id TEXT NOT NULL UNIQUE,
    service_id TEXT NOT NULL,
    metric_name TEXT NOT NULL,
    baseline_type TEXT NOT NULL CHECK (baseline_type IN ('daily', 'weekly', 'monthly')),

    avg_value REAL NOT NULL,
    std_dev REAL NOT NULL,
    min_value REAL NOT NULL,
    max_value REAL NOT NULL,
    p50_value REAL,
    p95_value REAL,
    p99_value REAL,

    sample_count INTEGER NOT NULL,
    computed_at TEXT NOT NULL DEFAULT (datetime('now')),
    valid_until TEXT,

    UNIQUE(service_id, metric_name, baseline_type),
    FOREIGN KEY (service_id) REFERENCES services(id)
);

--------------------------------------------------------------------------------
-- PERFORMANCE ANOMALIES
--------------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS performance_anomalies (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    anomaly_id TEXT NOT NULL UNIQUE,
    service_id TEXT NOT NULL,
    metric_name TEXT NOT NULL,
    detected_at TEXT NOT NULL DEFAULT (datetime('now')),
    actual_value REAL NOT NULL,
    expected_value REAL NOT NULL,
    deviation_sigma REAL NOT NULL,  -- standard deviations from baseline
    anomaly_type TEXT NOT NULL CHECK (anomaly_type IN ('spike', 'drop', 'drift', 'oscillation')),
    severity TEXT NOT NULL CHECK (severity IN ('low', 'medium', 'high', 'critical')),
    is_investigated INTEGER NOT NULL DEFAULT 0,
    investigation_notes TEXT,

    FOREIGN KEY (service_id) REFERENCES services(id)
);

--------------------------------------------------------------------------------
-- PIPELINE PERFORMANCE
--------------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS pipeline_performance (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    execution_id TEXT NOT NULL UNIQUE,
    pipeline_id TEXT NOT NULL,
    started_at TEXT NOT NULL DEFAULT (datetime('now')),
    completed_at TEXT,
    duration_ms INTEGER,
    status TEXT NOT NULL CHECK (status IN ('running', 'completed', 'failed', 'timeout')),

    -- Stage metrics (JSON array)
    stage_durations TEXT,  -- JSON: [{"stage": "name", "duration_ms": 123}, ...]

    -- Resource usage
    peak_memory_mb REAL,
    cpu_seconds REAL,

    -- Results
    items_processed INTEGER DEFAULT 0,
    items_failed INTEGER DEFAULT 0,
    error_message TEXT,

    FOREIGN KEY (pipeline_id) REFERENCES pipelines(id)
);

--------------------------------------------------------------------------------
-- AGGREGATED METRICS (hourly rollups)
--------------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS metrics_hourly (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    service_id TEXT NOT NULL,
    hour TEXT NOT NULL,  -- ISO format: YYYY-MM-DDTHH:00:00

    -- Aggregated resource metrics
    avg_cpu REAL,
    max_cpu REAL,
    avg_memory_mb REAL,
    max_memory_mb REAL,

    -- Aggregated latency
    avg_latency_ms REAL,
    p95_latency_ms REAL,
    p99_latency_ms REAL,

    -- Aggregated throughput
    total_requests INTEGER,
    avg_rps REAL,
    max_rps REAL,

    -- Aggregated errors
    total_errors INTEGER,
    error_rate REAL,

    -- Availability
    uptime_percent REAL,

    sample_count INTEGER NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),

    UNIQUE(service_id, hour),
    FOREIGN KEY (service_id) REFERENCES services(id)
);

--------------------------------------------------------------------------------
-- INDEXES
--------------------------------------------------------------------------------
CREATE INDEX IF NOT EXISTS idx_perf_samples_service ON performance_samples(service_id);
CREATE INDEX IF NOT EXISTS idx_perf_samples_timestamp ON performance_samples(timestamp);
CREATE INDEX IF NOT EXISTS idx_perf_samples_service_time ON performance_samples(service_id, timestamp);
CREATE INDEX IF NOT EXISTS idx_slo_violations_service ON slo_violations(service_id);
CREATE INDEX IF NOT EXISTS idx_slo_violations_unresolved ON slo_violations(is_resolved) WHERE is_resolved = 0;
CREATE INDEX IF NOT EXISTS idx_error_budgets_service ON error_budgets(service_id);
CREATE INDEX IF NOT EXISTS idx_perf_anomalies_service ON performance_anomalies(service_id);
CREATE INDEX IF NOT EXISTS idx_perf_anomalies_severity ON performance_anomalies(severity);
CREATE INDEX IF NOT EXISTS idx_pipeline_perf_pipeline ON pipeline_performance(pipeline_id);
CREATE INDEX IF NOT EXISTS idx_pipeline_perf_status ON pipeline_performance(status);
CREATE INDEX IF NOT EXISTS idx_metrics_hourly_service_hour ON metrics_hourly(service_id, hour);

--------------------------------------------------------------------------------
-- VIEWS
--------------------------------------------------------------------------------
CREATE VIEW IF NOT EXISTS v_slo_status AS
SELECT
    sd.slo_id,
    sd.service_id,
    sd.metric_name,
    sd.target_value,
    sd.comparison,
    COALESCE(
        (SELECT COUNT(*) FROM slo_violations sv
         WHERE sv.slo_id = sd.slo_id
         AND sv.detected_at >= datetime('now', '-24 hours')),
        0
    ) AS violations_24h,
    CASE
        WHEN EXISTS (
            SELECT 1 FROM slo_violations sv
            WHERE sv.slo_id = sd.slo_id AND sv.is_resolved = 0
        ) THEN 'VIOLATED'
        ELSE 'HEALTHY'
    END AS current_status
FROM slo_definitions sd
WHERE sd.is_active = 1;

CREATE VIEW IF NOT EXISTS v_service_health_latest AS
SELECT
    ps.service_id,
    ps.timestamp,
    ps.cpu_percent,
    ps.memory_mb,
    ps.avg_response_ms,
    ps.p99_latency_ms,
    ps.requests_per_second,
    ps.error_count,
    CASE
        WHEN ps.cpu_percent > 90 THEN 'CRITICAL'
        WHEN ps.cpu_percent > 75 THEN 'WARNING'
        WHEN ps.p99_latency_ms > 1000 THEN 'WARNING'
        WHEN ps.error_count > 10 THEN 'WARNING'
        ELSE 'HEALTHY'
    END AS health_status
FROM performance_samples ps
INNER JOIN (
    SELECT service_id, MAX(timestamp) AS max_ts
    FROM performance_samples
    GROUP BY service_id
) latest ON ps.service_id = latest.service_id AND ps.timestamp = latest.max_ts;

CREATE VIEW IF NOT EXISTS v_error_budget_status AS
SELECT
    eb.service_id,
    eb.period_start,
    eb.period_end,
    eb.total_budget_minutes,
    eb.consumed_minutes,
    eb.remaining_minutes,
    eb.budget_percent_remaining,
    CASE
        WHEN eb.is_exhausted = 1 THEN 'EXHAUSTED'
        WHEN eb.budget_percent_remaining < 10 THEN 'CRITICAL'
        WHEN eb.budget_percent_remaining < 25 THEN 'LOW'
        ELSE 'HEALTHY'
    END AS budget_status
FROM error_budgets eb
WHERE eb.period_end >= datetime('now');

--------------------------------------------------------------------------------
-- TRIGGERS
--------------------------------------------------------------------------------
CREATE TRIGGER IF NOT EXISTS trg_slo_violation_check
AFTER INSERT ON performance_samples
BEGIN
    INSERT INTO slo_violations (violation_id, slo_id, service_id, actual_value, target_value, severity)
    SELECT
        lower(hex(randomblob(16))),
        sd.slo_id,
        NEW.service_id,
        CASE sd.metric_name
            WHEN 'latency_p99' THEN NEW.p99_latency_ms
            WHEN 'cpu_percent' THEN NEW.cpu_percent
            WHEN 'error_rate' THEN CAST(NEW.error_count AS REAL) / NULLIF(NEW.requests_per_second, 0)
        END,
        sd.target_value,
        CASE
            WHEN sd.metric_name = 'latency_p99' AND NEW.p99_latency_ms > sd.target_value * 2 THEN 'critical'
            WHEN sd.metric_name = 'cpu_percent' AND NEW.cpu_percent > sd.target_value * 1.5 THEN 'critical'
            ELSE 'warning'
        END
    FROM slo_definitions sd
    WHERE sd.service_id = NEW.service_id
    AND sd.is_active = 1
    AND (
        (sd.metric_name = 'latency_p99' AND sd.comparison = 'lt' AND NEW.p99_latency_ms >= sd.target_value)
        OR (sd.metric_name = 'cpu_percent' AND sd.comparison = 'lt' AND NEW.cpu_percent >= sd.target_value)
    );
END;

--------------------------------------------------------------------------------
-- WORKFLOW INTEGRATION TABLES
--------------------------------------------------------------------------------

-- Workflow Performance Tracking
CREATE TABLE IF NOT EXISTS workflow_performance (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    performance_id TEXT NOT NULL UNIQUE DEFAULT (lower(hex(randomblob(16)))),

    -- Workflow reference
    workflow_instance_id TEXT NOT NULL,  -- References workflow_tracking.workflow_instances
    workflow_type TEXT NOT NULL,

    -- Overall metrics
    total_duration_ms INTEGER,
    step_count INTEGER NOT NULL DEFAULT 0,
    avg_step_duration_ms REAL,
    max_step_duration_ms INTEGER,

    -- Resource usage
    peak_cpu_percent REAL,
    avg_cpu_percent REAL,
    peak_memory_mb REAL,
    avg_memory_mb REAL,

    -- Throughput
    operations_per_second REAL,
    data_processed_mb REAL,

    -- Success metrics
    success_rate REAL,
    error_count INTEGER DEFAULT 0,
    retry_count INTEGER DEFAULT 0,

    -- SLO compliance
    slo_violations_during INTEGER DEFAULT 0,
    met_deadline INTEGER,

    -- Timing
    started_at TEXT NOT NULL,
    completed_at TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Workflow Step Performance
CREATE TABLE IF NOT EXISTS workflow_step_performance (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    step_perf_id TEXT NOT NULL UNIQUE DEFAULT (lower(hex(randomblob(16)))),

    -- References
    workflow_performance_id TEXT NOT NULL,
    step_execution_id TEXT NOT NULL,  -- References workflow_tracking.step_executions

    -- Step identification
    step_name TEXT NOT NULL,
    step_order INTEGER NOT NULL,

    -- Performance metrics
    duration_ms INTEGER,
    queue_time_ms INTEGER,  -- Time waiting in queue
    execution_time_ms INTEGER,

    -- Resource consumption
    cpu_usage_percent REAL,
    memory_usage_mb REAL,

    -- Status
    status TEXT NOT NULL,
    retries INTEGER DEFAULT 0,

    -- Baseline comparison
    baseline_duration_ms REAL,
    duration_deviation_percent REAL,

    FOREIGN KEY (workflow_performance_id) REFERENCES workflow_performance(performance_id)
);

-- Workflow Performance Baselines
CREATE TABLE IF NOT EXISTS workflow_performance_baselines (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    baseline_id TEXT NOT NULL UNIQUE DEFAULT (lower(hex(randomblob(16)))),

    -- Scope
    workflow_type TEXT NOT NULL,
    step_name TEXT,  -- NULL means workflow-level baseline

    -- Baseline metrics
    avg_duration_ms REAL NOT NULL,
    std_dev_duration_ms REAL NOT NULL,
    p50_duration_ms REAL,
    p95_duration_ms REAL,
    p99_duration_ms REAL,

    -- Resource baselines
    avg_cpu_percent REAL,
    avg_memory_mb REAL,

    -- Sample info
    sample_count INTEGER NOT NULL,
    computed_at TEXT NOT NULL DEFAULT (datetime('now')),
    valid_until TEXT,

    UNIQUE(workflow_type, step_name)
);

-- Indexes for workflow performance
CREATE INDEX IF NOT EXISTS idx_workflow_perf_instance ON workflow_performance(workflow_instance_id);
CREATE INDEX IF NOT EXISTS idx_workflow_perf_type ON workflow_performance(workflow_type);
CREATE INDEX IF NOT EXISTS idx_workflow_perf_started ON workflow_performance(started_at);
CREATE INDEX IF NOT EXISTS idx_workflow_step_perf_workflow ON workflow_step_performance(workflow_performance_id);
CREATE INDEX IF NOT EXISTS idx_workflow_step_perf_step ON workflow_step_performance(step_name);
CREATE INDEX IF NOT EXISTS idx_workflow_baselines_type ON workflow_performance_baselines(workflow_type);

--------------------------------------------------------------------------------
-- CROSS-DATABASE VIEWS
--------------------------------------------------------------------------------

-- Workflow performance summary
CREATE VIEW IF NOT EXISTS v_workflow_performance_summary AS
SELECT
    wp.workflow_type,
    COUNT(*) AS total_executions,
    AVG(wp.total_duration_ms) AS avg_duration_ms,
    MIN(wp.total_duration_ms) AS min_duration_ms,
    MAX(wp.total_duration_ms) AS max_duration_ms,
    AVG(wp.success_rate) AS avg_success_rate,
    AVG(wp.peak_cpu_percent) AS avg_peak_cpu,
    AVG(wp.peak_memory_mb) AS avg_peak_memory,
    SUM(wp.slo_violations_during) AS total_slo_violations,
    SUM(CASE WHEN wp.met_deadline = 1 THEN 1 ELSE 0 END) * 100.0 / COUNT(*) AS deadline_compliance_pct
FROM workflow_performance wp
WHERE wp.started_at >= datetime('now', '-7 days')
GROUP BY wp.workflow_type
ORDER BY total_executions DESC;

-- Slow workflow steps
CREATE VIEW IF NOT EXISTS v_slow_workflow_steps AS
SELECT
    wsp.step_name,
    wp.workflow_type,
    wsp.duration_ms,
    wsp.baseline_duration_ms,
    wsp.duration_deviation_percent,
    wsp.cpu_usage_percent,
    wsp.memory_usage_mb,
    wsp.status,
    wsp.retries,
    wp.workflow_instance_id
FROM workflow_step_performance wsp
JOIN workflow_performance wp ON wsp.workflow_performance_id = wp.performance_id
WHERE wsp.duration_deviation_percent > 50
OR wsp.duration_ms > 60000
ORDER BY wsp.duration_deviation_percent DESC;

-- Workflow SLO impact
CREATE VIEW IF NOT EXISTS v_workflow_slo_impact AS
SELECT
    wp.workflow_instance_id,
    wp.workflow_type,
    wp.started_at,
    wp.completed_at,
    wp.slo_violations_during,
    (SELECT COUNT(*) FROM slo_violations sv
     WHERE sv.detected_at BETWEEN wp.started_at AND COALESCE(wp.completed_at, datetime('now'))
    ) AS concurrent_violations,
    (SELECT GROUP_CONCAT(DISTINCT sv.service_id) FROM slo_violations sv
     WHERE sv.detected_at BETWEEN wp.started_at AND COALESCE(wp.completed_at, datetime('now'))
    ) AS affected_services
FROM workflow_performance wp
WHERE wp.slo_violations_during > 0
ORDER BY wp.started_at DESC;

-- Performance anomaly detection for workflows
CREATE VIEW IF NOT EXISTS v_workflow_performance_anomalies AS
SELECT
    wp.workflow_instance_id,
    wp.workflow_type,
    wp.total_duration_ms,
    wpb.avg_duration_ms AS baseline_duration,
    wpb.std_dev_duration_ms,
    (wp.total_duration_ms - wpb.avg_duration_ms) / NULLIF(wpb.std_dev_duration_ms, 0) AS z_score,
    CASE
        WHEN ABS((wp.total_duration_ms - wpb.avg_duration_ms) / NULLIF(wpb.std_dev_duration_ms, 0)) > 3 THEN 'CRITICAL'
        WHEN ABS((wp.total_duration_ms - wpb.avg_duration_ms) / NULLIF(wpb.std_dev_duration_ms, 0)) > 2 THEN 'WARNING'
        ELSE 'NORMAL'
    END AS anomaly_severity,
    wp.error_count,
    wp.started_at
FROM workflow_performance wp
JOIN workflow_performance_baselines wpb ON wp.workflow_type = wpb.workflow_type AND wpb.step_name IS NULL
WHERE ABS((wp.total_duration_ms - wpb.avg_duration_ms) / NULLIF(wpb.std_dev_duration_ms, 0)) > 2
ORDER BY z_score DESC;

--------------------------------------------------------------------------------
-- WORKFLOW INTEGRATION TRIGGERS
--------------------------------------------------------------------------------

-- Calculate step deviation from baseline
CREATE TRIGGER IF NOT EXISTS trg_workflow_step_deviation
AFTER INSERT ON workflow_step_performance
BEGIN
    UPDATE workflow_step_performance
    SET baseline_duration_ms = (
            SELECT wpb.avg_duration_ms FROM workflow_performance_baselines wpb
            JOIN workflow_performance wp ON NEW.workflow_performance_id = wp.performance_id
            WHERE wpb.workflow_type = wp.workflow_type
            AND wpb.step_name = NEW.step_name
        ),
        duration_deviation_percent = CASE
            WHEN (SELECT wpb.avg_duration_ms FROM workflow_performance_baselines wpb
                  JOIN workflow_performance wp ON NEW.workflow_performance_id = wp.performance_id
                  WHERE wpb.workflow_type = wp.workflow_type AND wpb.step_name = NEW.step_name) > 0
            THEN ((NEW.duration_ms - (SELECT wpb.avg_duration_ms FROM workflow_performance_baselines wpb
                  JOIN workflow_performance wp ON NEW.workflow_performance_id = wp.performance_id
                  WHERE wpb.workflow_type = wp.workflow_type AND wpb.step_name = NEW.step_name))
                  / (SELECT wpb.avg_duration_ms FROM workflow_performance_baselines wpb
                  JOIN workflow_performance wp ON NEW.workflow_performance_id = wp.performance_id
                  WHERE wpb.workflow_type = wp.workflow_type AND wpb.step_name = NEW.step_name)) * 100
            ELSE 0
        END
    WHERE step_perf_id = NEW.step_perf_id;
END;

-- Track SLO violations during workflow
CREATE TRIGGER IF NOT EXISTS trg_workflow_slo_violation_count
AFTER INSERT ON slo_violations
BEGIN
    UPDATE workflow_performance
    SET slo_violations_during = slo_violations_during + 1
    WHERE started_at <= NEW.detected_at
    AND (completed_at IS NULL OR completed_at >= NEW.detected_at)
    AND workflow_instance_id IN (
        SELECT workflow_instance_id FROM workflow_performance
        WHERE started_at <= NEW.detected_at
        AND (completed_at IS NULL OR completed_at >= NEW.detected_at)
    );
END;
