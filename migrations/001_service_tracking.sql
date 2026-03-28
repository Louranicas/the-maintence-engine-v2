-- ============================================================================
-- Migration: 001_service_tracking.sql
-- SYNTHEX Database Migration - Service Tracking Module
-- Version: 1.0.0
-- Pattern: NAM @0.A Agent Registry Integration
-- ============================================================================

-- Schema Version Tracking
CREATE TABLE IF NOT EXISTS schema_versions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    version TEXT NOT NULL UNIQUE,
    migration_name TEXT NOT NULL,
    applied_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    checksum TEXT,
    execution_time_ms INTEGER,
    rollback_sql TEXT
);

INSERT INTO schema_versions (version, migration_name, checksum)
VALUES ('001', 'service_tracking', 'sha256:001_service_tracking');

-- ============================================================================
-- CORE SERVICES TABLE
-- Tracks all managed services with real-time health metrics
-- ============================================================================
CREATE TABLE IF NOT EXISTS services (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,
    display_name TEXT,
    description TEXT,

    -- Process Information
    status TEXT NOT NULL DEFAULT 'unknown'
        CHECK(status IN ('running', 'stopped', 'starting', 'stopping', 'crashed', 'unknown', 'degraded')),
    pid INTEGER,
    port INTEGER,
    host TEXT DEFAULT 'localhost',

    -- Health Metrics
    health_status TEXT DEFAULT 'unknown'
        CHECK(health_status IN ('healthy', 'unhealthy', 'degraded', 'unknown', 'pending')),
    health_check_url TEXT,
    last_health_check DATETIME,
    health_check_interval_ms INTEGER DEFAULT 30000,
    consecutive_failures INTEGER DEFAULT 0,

    -- Resource Metrics
    cpu_percent REAL DEFAULT 0.0 CHECK(cpu_percent >= 0.0 AND cpu_percent <= 100.0),
    memory_mb REAL DEFAULT 0.0 CHECK(memory_mb >= 0.0),
    memory_percent REAL DEFAULT 0.0 CHECK(memory_percent >= 0.0 AND memory_percent <= 100.0),
    disk_io_read_kb REAL DEFAULT 0.0,
    disk_io_write_kb REAL DEFAULT 0.0,
    network_rx_kb REAL DEFAULT 0.0,
    network_tx_kb REAL DEFAULT 0.0,
    open_file_handles INTEGER DEFAULT 0,
    thread_count INTEGER DEFAULT 0,

    -- Lifecycle Metrics
    restart_count INTEGER DEFAULT 0,
    uptime_seconds INTEGER DEFAULT 0,
    last_restart DATETIME,
    start_time DATETIME,
    stop_time DATETIME,

    -- Configuration
    config_path TEXT,
    log_path TEXT,
    working_directory TEXT,
    environment_vars TEXT, -- JSON blob
    startup_command TEXT,
    shutdown_command TEXT,
    restart_policy TEXT DEFAULT 'on-failure'
        CHECK(restart_policy IN ('never', 'always', 'on-failure', 'unless-stopped')),
    max_restarts INTEGER DEFAULT 5,
    restart_window_seconds INTEGER DEFAULT 300,

    -- Generated Columns
    is_healthy INTEGER GENERATED ALWAYS AS (
        CASE WHEN health_status = 'healthy' AND status = 'running' THEN 1 ELSE 0 END
    ) STORED,
    resource_pressure REAL GENERATED ALWAYS AS (
        (cpu_percent * 0.4) + (memory_percent * 0.6)
    ) STORED,
    needs_attention INTEGER GENERATED ALWAYS AS (
        CASE WHEN consecutive_failures > 2 OR restart_count > max_restarts THEN 1 ELSE 0 END
    ) STORED,

    -- Metadata
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    version TEXT DEFAULT '1.0.0',
    tags TEXT -- JSON array
);

-- ============================================================================
-- SERVICE DEPENDENCIES TABLE
-- Tracks inter-service dependencies for orchestration
-- ============================================================================
CREATE TABLE IF NOT EXISTS service_dependencies (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    service_id INTEGER NOT NULL,
    dependency_id INTEGER NOT NULL,
    dependency_type TEXT NOT NULL DEFAULT 'required'
        CHECK(dependency_type IN ('required', 'optional', 'soft', 'hard', 'startup', 'runtime')),
    resolved INTEGER DEFAULT 0,
    resolution_time_ms INTEGER,

    -- Dependency Behavior
    wait_timeout_ms INTEGER DEFAULT 30000,
    retry_count INTEGER DEFAULT 3,
    retry_delay_ms INTEGER DEFAULT 1000,
    failure_action TEXT DEFAULT 'warn'
        CHECK(failure_action IN ('ignore', 'warn', 'block', 'cascade_stop')),

    -- Health Propagation
    propagate_health INTEGER DEFAULT 1,
    health_weight REAL DEFAULT 1.0 CHECK(health_weight >= 0.0 AND health_weight <= 1.0),

    -- Metadata
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    notes TEXT,

    FOREIGN KEY (service_id) REFERENCES services(id) ON DELETE CASCADE,
    FOREIGN KEY (dependency_id) REFERENCES services(id) ON DELETE CASCADE,
    UNIQUE(service_id, dependency_id)
);

-- ============================================================================
-- SERVICE EVENTS TABLE
-- Captures all service lifecycle and operational events
-- ============================================================================
CREATE TABLE IF NOT EXISTS service_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    service_id INTEGER NOT NULL,
    event_type TEXT NOT NULL
        CHECK(event_type IN (
            'started', 'stopped', 'crashed', 'restarted', 'health_check_passed',
            'health_check_failed', 'config_changed', 'resource_warning',
            'resource_critical', 'dependency_resolved', 'dependency_failed',
            'scaling_up', 'scaling_down', 'maintenance_started', 'maintenance_ended',
            'alert_triggered', 'alert_resolved', 'custom'
        )),
    event_data TEXT, -- JSON blob with event-specific data
    severity TEXT NOT NULL DEFAULT 'info'
        CHECK(severity IN ('debug', 'info', 'warning', 'error', 'critical', 'emergency')),

    -- Event Context
    source TEXT DEFAULT 'system',
    correlation_id TEXT,
    parent_event_id INTEGER,

    -- Timing
    timestamp DATETIME DEFAULT CURRENT_TIMESTAMP,
    duration_ms INTEGER,

    -- Processing Status
    acknowledged INTEGER DEFAULT 0,
    acknowledged_by TEXT,
    acknowledged_at DATETIME,
    processed INTEGER DEFAULT 0,
    processed_at DATETIME,

    -- Generated Columns
    is_critical INTEGER GENERATED ALWAYS AS (
        CASE WHEN severity IN ('critical', 'emergency') THEN 1 ELSE 0 END
    ) STORED,

    FOREIGN KEY (service_id) REFERENCES services(id) ON DELETE CASCADE,
    FOREIGN KEY (parent_event_id) REFERENCES service_events(id)
);

-- ============================================================================
-- SERVICE REGISTRY TABLE
-- Central registry for service discovery and routing
-- ============================================================================
CREATE TABLE IF NOT EXISTS service_registry (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    service_id INTEGER NOT NULL,
    service_type TEXT NOT NULL
        CHECK(service_type IN (
            'api', 'grpc', 'websocket', 'worker', 'scheduler', 'database',
            'cache', 'queue', 'gateway', 'proxy', 'monitor', 'agent', 'custom'
        )),

    -- Endpoint Configuration
    endpoint TEXT NOT NULL,
    protocol TEXT NOT NULL DEFAULT 'http'
        CHECK(protocol IN ('http', 'https', 'grpc', 'grpcs', 'ws', 'wss', 'tcp', 'udp', 'unix')),
    port INTEGER NOT NULL CHECK(port > 0 AND port < 65536),
    path_prefix TEXT DEFAULT '/',

    -- Discovery Metadata
    metadata TEXT, -- JSON blob
    capabilities TEXT, -- JSON array of capability strings
    version TEXT DEFAULT '1.0.0',
    api_version TEXT,

    -- Load Balancing
    weight INTEGER DEFAULT 100 CHECK(weight >= 0 AND weight <= 1000),
    priority INTEGER DEFAULT 0,
    zone TEXT DEFAULT 'default',
    region TEXT,

    -- Health Configuration
    health_endpoint TEXT DEFAULT '/health',
    readiness_endpoint TEXT DEFAULT '/ready',
    liveness_endpoint TEXT DEFAULT '/live',

    -- TLS Configuration
    tls_enabled INTEGER DEFAULT 0,
    tls_cert_path TEXT,
    tls_key_path TEXT,
    tls_ca_path TEXT,
    mtls_required INTEGER DEFAULT 0,

    -- Registration Status
    registered INTEGER DEFAULT 1,
    registration_time DATETIME DEFAULT CURRENT_TIMESTAMP,
    deregistration_time DATETIME,
    ttl_seconds INTEGER DEFAULT 60,
    last_heartbeat DATETIME DEFAULT CURRENT_TIMESTAMP,

    -- Generated Columns
    full_endpoint TEXT GENERATED ALWAYS AS (
        protocol || '://' || endpoint || ':' || port || path_prefix
    ) STORED,
    is_active INTEGER GENERATED ALWAYS AS (
        CASE WHEN registered = 1 AND
             (julianday('now') - julianday(last_heartbeat)) * 86400 < ttl_seconds
        THEN 1 ELSE 0 END
    ) STORED,

    FOREIGN KEY (service_id) REFERENCES services(id) ON DELETE CASCADE,
    UNIQUE(service_id, endpoint, port)
);

-- ============================================================================
-- AGENT REGISTRY TABLE
-- NAM @0.A Pattern - Agent orchestration and capability tracking
-- ============================================================================
CREATE TABLE IF NOT EXISTS agent_registry (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    agent_id TEXT NOT NULL UNIQUE,
    agent_type TEXT NOT NULL
        CHECK(agent_type IN (
            'orchestrator', 'executor', 'monitor', 'analyzer', 'optimizer',
            'validator', 'scheduler', 'communicator', 'learner', 'custom'
        )),

    -- Tiered Architecture (NAM Pattern)
    tier INTEGER NOT NULL DEFAULT 1 CHECK(tier >= 1 AND tier <= 6),
    tier_name TEXT GENERATED ALWAYS AS (
        CASE tier
            WHEN 1 THEN 'foundation'
            WHEN 2 THEN 'operational'
            WHEN 3 THEN 'tactical'
            WHEN 4 THEN 'strategic'
            WHEN 5 THEN 'cognitive'
            WHEN 6 THEN 'meta'
        END
    ) STORED,

    -- Load Balancing
    weight REAL DEFAULT 1.0 CHECK(weight >= 0.0 AND weight <= 10.0),
    current_load REAL DEFAULT 0.0 CHECK(current_load >= 0.0 AND current_load <= 100.0),
    max_concurrent_tasks INTEGER DEFAULT 10,
    active_tasks INTEGER DEFAULT 0,

    -- Capabilities (JSON array)
    capabilities TEXT NOT NULL DEFAULT '[]',
    supported_actions TEXT DEFAULT '[]',
    input_schemas TEXT, -- JSON schema definitions
    output_schemas TEXT,

    -- Communication
    communication_protocol TEXT DEFAULT 'grpc'
        CHECK(communication_protocol IN ('grpc', 'http', 'websocket', 'unix_socket', 'memory')),
    endpoint TEXT,
    queue_name TEXT,

    -- State Management
    status TEXT DEFAULT 'inactive'
        CHECK(status IN ('active', 'inactive', 'busy', 'error', 'maintenance', 'draining')),
    last_activity DATETIME,
    last_heartbeat DATETIME,
    heartbeat_interval_ms INTEGER DEFAULT 5000,

    -- Performance Metrics
    total_tasks_completed INTEGER DEFAULT 0,
    total_tasks_failed INTEGER DEFAULT 0,
    avg_task_duration_ms REAL DEFAULT 0.0,
    success_rate REAL GENERATED ALWAYS AS (
        CASE WHEN total_tasks_completed + total_tasks_failed > 0
        THEN CAST(total_tasks_completed AS REAL) / (total_tasks_completed + total_tasks_failed) * 100
        ELSE 0.0 END
    ) STORED,

    -- Availability Score (Generated)
    availability_score REAL GENERATED ALWAYS AS (
        CASE WHEN status = 'active' AND current_load < 80
        THEN (100 - current_load) * weight * (success_rate / 100)
        ELSE 0.0 END
    ) STORED,

    -- Metadata
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    version TEXT DEFAULT '1.0.0',
    metadata TEXT -- JSON blob for custom data
);

-- ============================================================================
-- INDEXES FOR PERFORMANCE
-- ============================================================================

-- Services Indexes
CREATE INDEX IF NOT EXISTS idx_services_status ON services(status);
CREATE INDEX IF NOT EXISTS idx_services_health ON services(health_status);
CREATE INDEX IF NOT EXISTS idx_services_needs_attention ON services(needs_attention) WHERE needs_attention = 1;
CREATE INDEX IF NOT EXISTS idx_services_is_healthy ON services(is_healthy);
CREATE INDEX IF NOT EXISTS idx_services_name ON services(name);
CREATE INDEX IF NOT EXISTS idx_services_port ON services(port) WHERE port IS NOT NULL;

-- Service Dependencies Indexes
CREATE INDEX IF NOT EXISTS idx_deps_service ON service_dependencies(service_id);
CREATE INDEX IF NOT EXISTS idx_deps_dependency ON service_dependencies(dependency_id);
CREATE INDEX IF NOT EXISTS idx_deps_resolved ON service_dependencies(resolved);
CREATE INDEX IF NOT EXISTS idx_deps_type ON service_dependencies(dependency_type);

-- Service Events Indexes
CREATE INDEX IF NOT EXISTS idx_events_service ON service_events(service_id);
CREATE INDEX IF NOT EXISTS idx_events_timestamp ON service_events(timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_events_type ON service_events(event_type);
CREATE INDEX IF NOT EXISTS idx_events_severity ON service_events(severity);
CREATE INDEX IF NOT EXISTS idx_events_critical ON service_events(is_critical) WHERE is_critical = 1;
CREATE INDEX IF NOT EXISTS idx_events_unprocessed ON service_events(processed) WHERE processed = 0;
CREATE INDEX IF NOT EXISTS idx_events_correlation ON service_events(correlation_id) WHERE correlation_id IS NOT NULL;

-- Service Registry Indexes
CREATE INDEX IF NOT EXISTS idx_registry_service ON service_registry(service_id);
CREATE INDEX IF NOT EXISTS idx_registry_type ON service_registry(service_type);
CREATE INDEX IF NOT EXISTS idx_registry_active ON service_registry(is_active) WHERE is_active = 1;
CREATE INDEX IF NOT EXISTS idx_registry_zone ON service_registry(zone);
CREATE INDEX IF NOT EXISTS idx_registry_endpoint ON service_registry(endpoint, port);

-- Agent Registry Indexes
CREATE INDEX IF NOT EXISTS idx_agents_type ON agent_registry(agent_type);
CREATE INDEX IF NOT EXISTS idx_agents_tier ON agent_registry(tier);
CREATE INDEX IF NOT EXISTS idx_agents_status ON agent_registry(status);
CREATE INDEX IF NOT EXISTS idx_agents_availability ON agent_registry(availability_score DESC);
CREATE INDEX IF NOT EXISTS idx_agents_active ON agent_registry(status) WHERE status = 'active';

-- ============================================================================
-- TRIGGERS FOR AUTOMATIC UPDATES
-- ============================================================================

-- Update services.updated_at on modification
CREATE TRIGGER IF NOT EXISTS trg_services_updated_at
AFTER UPDATE ON services
BEGIN
    UPDATE services SET updated_at = CURRENT_TIMESTAMP WHERE id = NEW.id;
END;

-- Update service_dependencies.updated_at on modification
CREATE TRIGGER IF NOT EXISTS trg_deps_updated_at
AFTER UPDATE ON service_dependencies
BEGIN
    UPDATE service_dependencies SET updated_at = CURRENT_TIMESTAMP WHERE id = NEW.id;
END;

-- Update agent_registry.updated_at on modification
CREATE TRIGGER IF NOT EXISTS trg_agents_updated_at
AFTER UPDATE ON agent_registry
BEGIN
    UPDATE agent_registry SET updated_at = CURRENT_TIMESTAMP WHERE id = NEW.id;
END;

-- Auto-create event when service status changes
CREATE TRIGGER IF NOT EXISTS trg_service_status_changed
AFTER UPDATE OF status ON services
WHEN OLD.status != NEW.status
BEGIN
    INSERT INTO service_events (service_id, event_type, event_data, severity)
    VALUES (
        NEW.id,
        CASE NEW.status
            WHEN 'running' THEN 'started'
            WHEN 'stopped' THEN 'stopped'
            WHEN 'crashed' THEN 'crashed'
            ELSE 'custom'
        END,
        json_object('old_status', OLD.status, 'new_status', NEW.status),
        CASE NEW.status
            WHEN 'crashed' THEN 'error'
            WHEN 'degraded' THEN 'warning'
            ELSE 'info'
        END
    );
END;

-- Auto-increment restart_count when service restarts
CREATE TRIGGER IF NOT EXISTS trg_service_restarted
AFTER UPDATE OF status ON services
WHEN OLD.status IN ('stopped', 'crashed') AND NEW.status = 'running'
BEGIN
    UPDATE services SET
        restart_count = restart_count + 1,
        last_restart = CURRENT_TIMESTAMP,
        start_time = CURRENT_TIMESTAMP
    WHERE id = NEW.id;
END;

-- ============================================================================
-- WORKFLOW INTEGRATION TABLES (Cross-DB References)
-- ============================================================================

-- Service-Workflow Association (tracks which workflows affect services)
CREATE TABLE IF NOT EXISTS service_workflow_associations (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    service_id INTEGER NOT NULL,
    workflow_id TEXT NOT NULL,  -- References workflow_tracking.workflow_definitions
    association_type TEXT NOT NULL
        CHECK(association_type IN ('target', 'dependency', 'affected', 'owner')),
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,

    FOREIGN KEY (service_id) REFERENCES services(id) ON DELETE CASCADE,
    UNIQUE(service_id, workflow_id, association_type)
);

-- Service Remediation History (links to workflow executions)
CREATE TABLE IF NOT EXISTS service_remediation_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    service_id INTEGER NOT NULL,
    workflow_instance_id TEXT NOT NULL,  -- References workflow_tracking.workflow_instances
    remediation_type TEXT NOT NULL,
    trigger_reason TEXT,
    outcome TEXT CHECK(outcome IN ('success', 'failure', 'partial', 'rolled_back')),
    started_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    completed_at DATETIME,

    FOREIGN KEY (service_id) REFERENCES services(id) ON DELETE CASCADE
);

-- Indexes for workflow integration
CREATE INDEX IF NOT EXISTS idx_service_workflow_assoc_service ON service_workflow_associations(service_id);
CREATE INDEX IF NOT EXISTS idx_service_workflow_assoc_workflow ON service_workflow_associations(workflow_id);
CREATE INDEX IF NOT EXISTS idx_service_remediation_service ON service_remediation_history(service_id);
CREATE INDEX IF NOT EXISTS idx_service_remediation_workflow ON service_remediation_history(workflow_instance_id);

-- ============================================================================
-- CROSS-DATABASE VIEWS
-- ============================================================================

-- Unified system health view (referenced by workflow decisions)
CREATE VIEW IF NOT EXISTS v_system_health AS
SELECT
    s.id AS service_id,
    s.name,
    s.status,
    s.health_status,
    s.is_healthy,
    s.resource_pressure,
    s.needs_attention,
    s.restart_count,
    s.uptime_seconds,
    (SELECT COUNT(*) FROM service_dependencies sd
     WHERE sd.service_id = s.id AND sd.resolved = 0) AS unresolved_deps,
    (SELECT COUNT(*) FROM service_events se
     WHERE se.service_id = s.id
     AND se.severity IN ('critical', 'emergency')
     AND se.timestamp > datetime('now', '-1 hour')) AS recent_critical_events
FROM services s;

-- Service overview for dashboard
CREATE VIEW IF NOT EXISTS v_service_overview AS
SELECT
    s.id,
    s.name,
    s.display_name,
    s.status,
    s.health_status,
    s.port,
    s.cpu_percent,
    s.memory_percent,
    s.resource_pressure,
    s.restart_count,
    s.uptime_seconds,
    s.is_healthy,
    s.needs_attention,
    s.updated_at,
    (SELECT COUNT(*) FROM service_dependencies sd WHERE sd.service_id = s.id) AS dependency_count,
    (SELECT COUNT(*) FROM service_workflow_associations swa WHERE swa.service_id = s.id) AS active_workflows
FROM services s
ORDER BY s.needs_attention DESC, s.resource_pressure DESC;

-- Services pending remediation
CREATE VIEW IF NOT EXISTS v_services_pending_remediation AS
SELECT
    s.id,
    s.name,
    s.status,
    s.health_status,
    s.consecutive_failures,
    s.restart_count,
    s.max_restarts,
    CASE
        WHEN s.restart_count >= s.max_restarts THEN 'restart_limit_exceeded'
        WHEN s.consecutive_failures > 5 THEN 'consecutive_failures'
        WHEN s.health_status = 'unhealthy' AND s.status = 'running' THEN 'unhealthy_running'
        WHEN s.status = 'crashed' THEN 'crashed'
        ELSE 'monitoring'
    END AS remediation_reason,
    (SELECT MAX(srh.completed_at) FROM service_remediation_history srh
     WHERE srh.service_id = s.id) AS last_remediation
FROM services s
WHERE s.needs_attention = 1 OR s.status = 'crashed' OR s.health_status = 'unhealthy';

-- Dependency graph for workflow orchestration
CREATE VIEW IF NOT EXISTS v_dependency_graph AS
SELECT
    s1.name AS service_name,
    s2.name AS depends_on,
    sd.dependency_type,
    sd.resolved,
    sd.health_weight,
    sd.failure_action,
    s2.is_healthy AS dependency_healthy
FROM service_dependencies sd
JOIN services s1 ON sd.service_id = s1.id
JOIN services s2 ON sd.dependency_id = s2.id
ORDER BY s1.name, sd.dependency_type;

-- ============================================================================
-- WORKFLOW INTEGRATION TRIGGERS
-- ============================================================================

-- Auto-create remediation history entry when service crashes
CREATE TRIGGER IF NOT EXISTS trg_service_crashed_remediation
AFTER UPDATE OF status ON services
WHEN NEW.status = 'crashed' AND OLD.status != 'crashed'
BEGIN
    INSERT INTO service_remediation_history (service_id, workflow_instance_id, remediation_type, trigger_reason)
    SELECT
        NEW.id,
        'pending_' || lower(hex(randomblob(8))),
        'auto_restart',
        'Service crashed from status: ' || OLD.status;
END;

-- Track health status changes for learning integration
CREATE TRIGGER IF NOT EXISTS trg_service_health_changed
AFTER UPDATE OF health_status ON services
WHEN OLD.health_status != NEW.health_status
BEGIN
    INSERT INTO service_events (service_id, event_type, event_data, severity)
    VALUES (
        NEW.id,
        'health_check_' || CASE WHEN NEW.health_status = 'healthy' THEN 'passed' ELSE 'failed' END,
        json_object(
            'old_health', OLD.health_status,
            'new_health', NEW.health_status,
            'consecutive_failures', NEW.consecutive_failures
        ),
        CASE NEW.health_status
            WHEN 'healthy' THEN 'info'
            WHEN 'degraded' THEN 'warning'
            WHEN 'unhealthy' THEN 'error'
            ELSE 'warning'
        END
    );
END;

-- ============================================================================
-- End of Migration 001_service_tracking.sql
-- ============================================================================
