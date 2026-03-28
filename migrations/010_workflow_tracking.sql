-- ============================================================================
-- Migration: 010_workflow_tracking.sql
-- Purpose: Workflow orchestration, task tracking, and job management
-- Database: workflow_tracking.db
-- Version: 1.0.0
--
-- CROSS-DATABASE INTEGRATION:
-- This database integrates with all other Maintenance Engine databases:
--
-- | Database               | Integration Tables                    | Purpose                    |
-- |------------------------|---------------------------------------|----------------------------|
-- | service_tracking.db    | service_workflow_associations         | Service-workflow mapping   |
-- |                        | service_remediation_history           | Remediation tracking       |
-- | system_synergy.db      | workflow_synergy_impact               | Synergy measurements       |
-- |                        | integration_tasks                     | Integration operations     |
-- | hebbian_pulse.db       | workflow_learning_events              | Learning signals           |
-- |                        | pathway_workflow_mapping              | Pathway associations       |
-- | consensus_tracking.db  | workflow_approval_requests            | L2/L3 approvals            |
-- |                        | consensus_workflow_history            | Consensus decisions        |
-- | episodic_memory.db     | workflow_episodes                     | NAM-06 episodes            |
-- |                        | workflow_step_events                  | Step recording             |
-- |                        | workflow_pattern_discoveries          | Pattern learning           |
-- | tensor_memory.db       | workflow_tensor_snapshots             | 12D tensor checkpoints     |
-- |                        | workflow_tensor_deltas                | State changes              |
-- | performance_metrics.db | workflow_performance                  | Performance tracking       |
-- |                        | workflow_step_performance             | Step-level metrics         |
-- |                        | workflow_performance_baselines        | Baseline comparison        |
-- | flow_state.db          | workflow_state_machines               | State machine control      |
-- |                        | workflow_transitions                  | Workflow-driven transitions|
-- |                        | flow_orchestration_workflows          | Orchestration mapping      |
-- | security_events.db     | workflow_security_events              | Security audit             |
-- |                        | workflow_access_control               | Access management          |
-- |                        | workflow_audit_trail                  | Compliance audit           |
--
-- ============================================================================

--------------------------------------------------------------------------------
-- SCHEMA VERSION
--------------------------------------------------------------------------------
INSERT INTO schema_versions (version, migration_name, checksum)
VALUES ('010', 'workflow_tracking', 'sha256:010_workflow_tracking');

--------------------------------------------------------------------------------
-- WORKFLOW DEFINITIONS
--------------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS workflow_definitions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    workflow_id TEXT NOT NULL UNIQUE,
    workflow_name TEXT NOT NULL,
    description TEXT,
    version TEXT NOT NULL DEFAULT '1.0.0',

    -- Workflow Type
    workflow_type TEXT NOT NULL CHECK (workflow_type IN (
        'remediation', 'deployment', 'scaling', 'maintenance',
        'backup', 'recovery', 'migration', 'custom'
    )),

    -- Execution Settings
    execution_mode TEXT NOT NULL DEFAULT 'sequential' CHECK (execution_mode IN (
        'sequential', 'parallel', 'dag', 'conditional'
    )),
    max_concurrent_tasks INTEGER DEFAULT 5,
    timeout_seconds INTEGER DEFAULT 3600,
    retry_policy TEXT DEFAULT 'exponential',  -- JSON

    -- Trigger Configuration
    trigger_type TEXT NOT NULL CHECK (trigger_type IN (
        'manual', 'scheduled', 'event', 'condition', 'api'
    )),
    trigger_config TEXT,  -- JSON: cron, event patterns, conditions

    -- Authorization
    required_approval_tier TEXT CHECK (required_approval_tier IN ('L0', 'L1', 'L2', 'L3')),
    allowed_agents TEXT,  -- JSON array of agent_ids

    -- Status
    is_enabled INTEGER NOT NULL DEFAULT 1,
    is_template INTEGER NOT NULL DEFAULT 0,

    -- Metadata
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    created_by TEXT,
    tags TEXT  -- JSON array
);

--------------------------------------------------------------------------------
-- WORKFLOW STEPS
--------------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS workflow_steps (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    step_id TEXT NOT NULL UNIQUE,
    workflow_id TEXT NOT NULL,
    step_name TEXT NOT NULL,
    description TEXT,

    -- Step Position
    step_order INTEGER NOT NULL,
    parent_step_id TEXT,  -- For nested/conditional steps

    -- Step Type
    step_type TEXT NOT NULL CHECK (step_type IN (
        'action', 'condition', 'loop', 'parallel', 'wait',
        'approval', 'notification', 'subworkflow', 'checkpoint'
    )),

    -- Action Configuration
    action_type TEXT,  -- restart, scale, deploy, execute, etc.
    action_config TEXT NOT NULL,  -- JSON: parameters, targets

    -- Conditional Logic
    condition_expression TEXT,  -- JSON: for condition/loop steps
    on_success_step TEXT,
    on_failure_step TEXT,

    -- Execution Settings
    timeout_seconds INTEGER DEFAULT 300,
    retry_count INTEGER DEFAULT 3,
    retry_delay_seconds INTEGER DEFAULT 10,
    continue_on_failure INTEGER DEFAULT 0,

    -- Dependencies
    depends_on TEXT,  -- JSON array of step_ids

    -- Metadata
    created_at TEXT NOT NULL DEFAULT (datetime('now')),

    UNIQUE(workflow_id, step_order),
    FOREIGN KEY (workflow_id) REFERENCES workflow_definitions(workflow_id),
    FOREIGN KEY (parent_step_id) REFERENCES workflow_steps(step_id)
);

--------------------------------------------------------------------------------
-- WORKFLOW INSTANCES (Executions)
--------------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS workflow_instances (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    instance_id TEXT NOT NULL UNIQUE,
    workflow_id TEXT NOT NULL,

    -- Execution Context
    trigger_source TEXT NOT NULL,  -- manual, scheduled, event, api
    trigger_event_id TEXT,  -- Reference to triggering event
    triggered_by TEXT,  -- agent_id or user_id

    -- Input/Output
    input_parameters TEXT,  -- JSON
    output_results TEXT,    -- JSON
    context_data TEXT,      -- JSON: shared data across steps

    -- Status
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN (
        'pending', 'queued', 'running', 'paused', 'waiting_approval',
        'completed', 'failed', 'cancelled', 'timeout', 'rolled_back'
    )),

    -- Progress
    current_step_id TEXT,
    current_step_order INTEGER DEFAULT 0,
    total_steps INTEGER NOT NULL,
    completed_steps INTEGER DEFAULT 0,
    failed_steps INTEGER DEFAULT 0,
    skipped_steps INTEGER DEFAULT 0,
    progress_percent REAL GENERATED ALWAYS AS (
        CASE WHEN total_steps > 0
        THEN (completed_steps * 100.0 / total_steps)
        ELSE 0 END
    ) STORED,

    -- Timing
    scheduled_at TEXT,
    started_at TEXT,
    completed_at TEXT,
    duration_ms INTEGER,

    -- Error Handling
    error_message TEXT,
    error_step_id TEXT,
    retry_attempt INTEGER DEFAULT 0,
    max_retries INTEGER DEFAULT 3,

    -- Approval Tracking
    approval_status TEXT CHECK (approval_status IN (
        'not_required', 'pending', 'approved', 'rejected'
    )),
    approved_by TEXT,
    approved_at TEXT,

    -- Metadata
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),

    FOREIGN KEY (workflow_id) REFERENCES workflow_definitions(workflow_id),
    FOREIGN KEY (current_step_id) REFERENCES workflow_steps(step_id)
);

--------------------------------------------------------------------------------
-- STEP EXECUTIONS
--------------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS step_executions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    execution_id TEXT NOT NULL UNIQUE,
    instance_id TEXT NOT NULL,
    step_id TEXT NOT NULL,

    -- Execution Details
    attempt_number INTEGER NOT NULL DEFAULT 1,

    -- Status
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN (
        'pending', 'running', 'completed', 'failed', 'skipped',
        'cancelled', 'timeout', 'waiting'
    )),

    -- Input/Output
    input_data TEXT,   -- JSON
    output_data TEXT,  -- JSON

    -- Timing
    started_at TEXT,
    completed_at TEXT,
    duration_ms INTEGER,

    -- Results
    exit_code INTEGER,
    success INTEGER,
    error_message TEXT,
    error_details TEXT,  -- JSON: stack trace, context

    -- Agent Assignment
    assigned_agent TEXT,
    executed_by TEXT,

    -- Logging
    log_output TEXT,
    artifacts TEXT,  -- JSON: paths to generated files

    -- Metadata
    created_at TEXT NOT NULL DEFAULT (datetime('now')),

    UNIQUE(instance_id, step_id, attempt_number),
    FOREIGN KEY (instance_id) REFERENCES workflow_instances(instance_id),
    FOREIGN KEY (step_id) REFERENCES workflow_steps(step_id)
);

--------------------------------------------------------------------------------
-- TASK QUEUE
--------------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS task_queue (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    task_id TEXT NOT NULL UNIQUE,

    -- Task Definition
    task_type TEXT NOT NULL CHECK (task_type IN (
        'workflow_step', 'remediation', 'health_check', 'metric_collection',
        'learning_cycle', 'consensus_vote', 'notification', 'cleanup', 'custom'
    )),
    task_name TEXT NOT NULL,
    description TEXT,

    -- Execution Context
    workflow_instance_id TEXT,
    step_execution_id TEXT,

    -- Priority and Scheduling
    priority INTEGER NOT NULL DEFAULT 5 CHECK (priority BETWEEN 1 AND 10),
    scheduled_at TEXT,
    deadline_at TEXT,

    -- Status
    status TEXT NOT NULL DEFAULT 'queued' CHECK (status IN (
        'queued', 'assigned', 'running', 'completed', 'failed',
        'cancelled', 'timeout', 'retry_pending'
    )),

    -- Assignment
    assigned_agent TEXT,
    assigned_at TEXT,

    -- Payload
    payload TEXT NOT NULL,  -- JSON: task-specific data
    result TEXT,            -- JSON: execution result

    -- Retry Logic
    attempt_count INTEGER DEFAULT 0,
    max_attempts INTEGER DEFAULT 3,
    next_retry_at TEXT,

    -- Timing
    started_at TEXT,
    completed_at TEXT,
    duration_ms INTEGER,

    -- Error Tracking
    last_error TEXT,
    error_count INTEGER DEFAULT 0,

    -- Metadata
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    created_by TEXT,

    FOREIGN KEY (workflow_instance_id) REFERENCES workflow_instances(instance_id),
    FOREIGN KEY (step_execution_id) REFERENCES step_executions(execution_id)
);

--------------------------------------------------------------------------------
-- SCHEDULED JOBS
--------------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS scheduled_jobs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    job_id TEXT NOT NULL UNIQUE,
    job_name TEXT NOT NULL,
    description TEXT,

    -- Schedule Configuration
    schedule_type TEXT NOT NULL CHECK (schedule_type IN (
        'cron', 'interval', 'fixed_time', 'one_time'
    )),
    cron_expression TEXT,
    interval_seconds INTEGER,
    fixed_time TEXT,
    timezone TEXT DEFAULT 'UTC',

    -- Job Configuration
    job_type TEXT NOT NULL CHECK (job_type IN (
        'workflow', 'task', 'health_check', 'cleanup',
        'backup', 'report', 'custom'
    )),
    job_config TEXT NOT NULL,  -- JSON: workflow_id, task definition, etc.

    -- Execution Control
    is_enabled INTEGER NOT NULL DEFAULT 1,
    max_concurrent INTEGER DEFAULT 1,
    catch_up_missed INTEGER DEFAULT 0,

    -- Timing
    last_run_at TEXT,
    next_run_at TEXT,
    last_run_duration_ms INTEGER,
    last_run_status TEXT,

    -- Statistics
    total_runs INTEGER DEFAULT 0,
    successful_runs INTEGER DEFAULT 0,
    failed_runs INTEGER DEFAULT 0,

    -- Metadata
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    created_by TEXT
);

--------------------------------------------------------------------------------
-- JOB HISTORY
--------------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS job_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    history_id TEXT NOT NULL UNIQUE,
    job_id TEXT NOT NULL,

    -- Execution
    scheduled_time TEXT NOT NULL,
    started_at TEXT,
    completed_at TEXT,
    duration_ms INTEGER,

    -- Status
    status TEXT NOT NULL CHECK (status IN (
        'scheduled', 'running', 'completed', 'failed', 'skipped', 'cancelled'
    )),

    -- Results
    result TEXT,  -- JSON
    error_message TEXT,

    -- Instance Reference
    workflow_instance_id TEXT,
    task_id TEXT,

    -- Metadata
    created_at TEXT NOT NULL DEFAULT (datetime('now')),

    FOREIGN KEY (job_id) REFERENCES scheduled_jobs(job_id),
    FOREIGN KEY (workflow_instance_id) REFERENCES workflow_instances(instance_id),
    FOREIGN KEY (task_id) REFERENCES task_queue(task_id)
);

--------------------------------------------------------------------------------
-- WORKFLOW VARIABLES
--------------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS workflow_variables (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    variable_id TEXT NOT NULL UNIQUE,

    -- Scope
    scope TEXT NOT NULL CHECK (scope IN (
        'global', 'workflow', 'instance', 'step'
    )),
    scope_id TEXT,  -- workflow_id, instance_id, or step_id

    -- Variable Definition
    variable_name TEXT NOT NULL,
    variable_type TEXT NOT NULL CHECK (variable_type IN (
        'string', 'number', 'boolean', 'json', 'secret'
    )),
    variable_value TEXT,
    default_value TEXT,

    -- Constraints
    is_required INTEGER DEFAULT 0,
    is_readonly INTEGER DEFAULT 0,
    validation_rule TEXT,  -- JSON: regex, range, enum

    -- Metadata
    description TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),

    UNIQUE(scope, scope_id, variable_name)
);

--------------------------------------------------------------------------------
-- WORKFLOW LOCKS
--------------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS workflow_locks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    lock_id TEXT NOT NULL UNIQUE,

    -- Lock Target
    resource_type TEXT NOT NULL CHECK (resource_type IN (
        'workflow', 'service', 'database', 'file', 'custom'
    )),
    resource_id TEXT NOT NULL,

    -- Lock Details
    lock_type TEXT NOT NULL CHECK (lock_type IN (
        'exclusive', 'shared', 'advisory'
    )),
    holder_type TEXT NOT NULL,  -- workflow_instance, agent, user
    holder_id TEXT NOT NULL,

    -- Timing
    acquired_at TEXT NOT NULL DEFAULT (datetime('now')),
    expires_at TEXT,
    renewed_at TEXT,

    -- Status
    is_active INTEGER NOT NULL DEFAULT 1,
    release_reason TEXT,

    UNIQUE(resource_type, resource_id, lock_type)
);

--------------------------------------------------------------------------------
-- WORKFLOW EVENTS
--------------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS workflow_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    event_id TEXT NOT NULL UNIQUE,

    -- Event Source
    source_type TEXT NOT NULL CHECK (source_type IN (
        'workflow', 'step', 'task', 'job', 'system', 'external'
    )),
    source_id TEXT NOT NULL,

    -- Event Details
    event_type TEXT NOT NULL CHECK (event_type IN (
        'started', 'completed', 'failed', 'paused', 'resumed',
        'cancelled', 'timeout', 'retry', 'approval_requested',
        'approval_granted', 'approval_denied', 'error', 'warning',
        'state_change', 'progress', 'custom'
    )),
    severity TEXT NOT NULL DEFAULT 'info' CHECK (severity IN (
        'debug', 'info', 'warning', 'error', 'critical'
    )),

    -- Event Data
    message TEXT NOT NULL,
    details TEXT,  -- JSON

    -- Context
    workflow_id TEXT,
    instance_id TEXT,
    step_id TEXT,

    -- Metadata
    timestamp TEXT NOT NULL DEFAULT (datetime('now')),
    actor TEXT,  -- agent_id, user_id
    correlation_id TEXT,

    FOREIGN KEY (workflow_id) REFERENCES workflow_definitions(workflow_id),
    FOREIGN KEY (instance_id) REFERENCES workflow_instances(instance_id),
    FOREIGN KEY (step_id) REFERENCES workflow_steps(step_id)
);

--------------------------------------------------------------------------------
-- WORKFLOW TEMPLATES
--------------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS workflow_templates (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    template_id TEXT NOT NULL UNIQUE,
    template_name TEXT NOT NULL,
    description TEXT,
    category TEXT NOT NULL,

    -- Template Definition
    workflow_definition TEXT NOT NULL,  -- JSON: full workflow structure

    -- Parameters
    parameter_schema TEXT,  -- JSON Schema for required inputs
    default_parameters TEXT,  -- JSON

    -- Usage
    is_public INTEGER NOT NULL DEFAULT 1,
    usage_count INTEGER DEFAULT 0,

    -- Versioning
    version TEXT NOT NULL DEFAULT '1.0.0',
    previous_version_id TEXT,

    -- Metadata
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    created_by TEXT,
    tags TEXT  -- JSON array
);

--------------------------------------------------------------------------------
-- INDEXES
--------------------------------------------------------------------------------

-- Workflow Definitions
CREATE INDEX IF NOT EXISTS idx_workflow_defs_type ON workflow_definitions(workflow_type);
CREATE INDEX IF NOT EXISTS idx_workflow_defs_enabled ON workflow_definitions(is_enabled);
CREATE INDEX IF NOT EXISTS idx_workflow_defs_trigger ON workflow_definitions(trigger_type);

-- Workflow Steps
CREATE INDEX IF NOT EXISTS idx_workflow_steps_workflow ON workflow_steps(workflow_id);
CREATE INDEX IF NOT EXISTS idx_workflow_steps_order ON workflow_steps(workflow_id, step_order);
CREATE INDEX IF NOT EXISTS idx_workflow_steps_type ON workflow_steps(step_type);

-- Workflow Instances
CREATE INDEX IF NOT EXISTS idx_workflow_instances_workflow ON workflow_instances(workflow_id);
CREATE INDEX IF NOT EXISTS idx_workflow_instances_status ON workflow_instances(status);
CREATE INDEX IF NOT EXISTS idx_workflow_instances_trigger ON workflow_instances(triggered_by);
CREATE INDEX IF NOT EXISTS idx_workflow_instances_created ON workflow_instances(created_at);
CREATE INDEX IF NOT EXISTS idx_workflow_instances_running ON workflow_instances(status)
    WHERE status IN ('running', 'paused', 'waiting_approval');

-- Step Executions
CREATE INDEX IF NOT EXISTS idx_step_executions_instance ON step_executions(instance_id);
CREATE INDEX IF NOT EXISTS idx_step_executions_step ON step_executions(step_id);
CREATE INDEX IF NOT EXISTS idx_step_executions_status ON step_executions(status);

-- Task Queue
CREATE INDEX IF NOT EXISTS idx_task_queue_status ON task_queue(status);
CREATE INDEX IF NOT EXISTS idx_task_queue_priority ON task_queue(priority DESC, created_at);
CREATE INDEX IF NOT EXISTS idx_task_queue_scheduled ON task_queue(scheduled_at);
CREATE INDEX IF NOT EXISTS idx_task_queue_assigned ON task_queue(assigned_agent);
CREATE INDEX IF NOT EXISTS idx_task_queue_pending ON task_queue(status, priority DESC)
    WHERE status = 'queued';

-- Scheduled Jobs
CREATE INDEX IF NOT EXISTS idx_scheduled_jobs_enabled ON scheduled_jobs(is_enabled);
CREATE INDEX IF NOT EXISTS idx_scheduled_jobs_next_run ON scheduled_jobs(next_run_at);
CREATE INDEX IF NOT EXISTS idx_scheduled_jobs_type ON scheduled_jobs(job_type);

-- Job History
CREATE INDEX IF NOT EXISTS idx_job_history_job ON job_history(job_id);
CREATE INDEX IF NOT EXISTS idx_job_history_status ON job_history(status);
CREATE INDEX IF NOT EXISTS idx_job_history_scheduled ON job_history(scheduled_time);

-- Workflow Variables
CREATE INDEX IF NOT EXISTS idx_workflow_vars_scope ON workflow_variables(scope, scope_id);

-- Workflow Locks
CREATE INDEX IF NOT EXISTS idx_workflow_locks_resource ON workflow_locks(resource_type, resource_id);
CREATE INDEX IF NOT EXISTS idx_workflow_locks_active ON workflow_locks(is_active) WHERE is_active = 1;
CREATE INDEX IF NOT EXISTS idx_workflow_locks_expires ON workflow_locks(expires_at);

-- Workflow Events
CREATE INDEX IF NOT EXISTS idx_workflow_events_source ON workflow_events(source_type, source_id);
CREATE INDEX IF NOT EXISTS idx_workflow_events_type ON workflow_events(event_type);
CREATE INDEX IF NOT EXISTS idx_workflow_events_instance ON workflow_events(instance_id);
CREATE INDEX IF NOT EXISTS idx_workflow_events_timestamp ON workflow_events(timestamp);
CREATE INDEX IF NOT EXISTS idx_workflow_events_correlation ON workflow_events(correlation_id);

--------------------------------------------------------------------------------
-- VIEWS
--------------------------------------------------------------------------------

-- Active Workflows Overview
CREATE VIEW IF NOT EXISTS v_active_workflows AS
SELECT
    wi.instance_id,
    wd.workflow_name,
    wd.workflow_type,
    wi.status,
    wi.progress_percent,
    wi.current_step_order || '/' || wi.total_steps AS progress,
    wi.triggered_by,
    wi.started_at,
    (julianday('now') - julianday(wi.started_at)) * 86400000 AS running_ms,
    wi.approval_status
FROM workflow_instances wi
JOIN workflow_definitions wd ON wi.workflow_id = wd.workflow_id
WHERE wi.status IN ('running', 'paused', 'waiting_approval')
ORDER BY wi.started_at DESC;

-- Task Queue Summary
CREATE VIEW IF NOT EXISTS v_task_queue_summary AS
SELECT
    status,
    task_type,
    COUNT(*) AS task_count,
    AVG(priority) AS avg_priority,
    MIN(created_at) AS oldest_task,
    SUM(CASE WHEN deadline_at < datetime('now') THEN 1 ELSE 0 END) AS overdue_count
FROM task_queue
WHERE status IN ('queued', 'assigned', 'running', 'retry_pending')
GROUP BY status, task_type
ORDER BY status, task_type;

-- Pending Tasks by Priority
CREATE VIEW IF NOT EXISTS v_pending_tasks AS
SELECT
    task_id,
    task_type,
    task_name,
    priority,
    scheduled_at,
    deadline_at,
    attempt_count,
    workflow_instance_id,
    created_at,
    CASE
        WHEN deadline_at < datetime('now') THEN 'OVERDUE'
        WHEN deadline_at < datetime('now', '+1 hour') THEN 'URGENT'
        ELSE 'NORMAL'
    END AS urgency
FROM task_queue
WHERE status = 'queued'
ORDER BY
    CASE WHEN deadline_at < datetime('now') THEN 0 ELSE 1 END,
    priority DESC,
    created_at;

-- Scheduled Jobs Due
CREATE VIEW IF NOT EXISTS v_scheduled_jobs_due AS
SELECT
    job_id,
    job_name,
    job_type,
    schedule_type,
    next_run_at,
    last_run_status,
    total_runs,
    successful_runs,
    failed_runs,
    CASE
        WHEN failed_runs > successful_runs * 0.1 THEN 'DEGRADED'
        ELSE 'HEALTHY'
    END AS job_health
FROM scheduled_jobs
WHERE is_enabled = 1
AND next_run_at <= datetime('now', '+5 minutes')
ORDER BY next_run_at;

-- Workflow Execution Stats
CREATE VIEW IF NOT EXISTS v_workflow_stats AS
SELECT
    wd.workflow_id,
    wd.workflow_name,
    wd.workflow_type,
    COUNT(wi.instance_id) AS total_executions,
    SUM(CASE WHEN wi.status = 'completed' THEN 1 ELSE 0 END) AS successful,
    SUM(CASE WHEN wi.status = 'failed' THEN 1 ELSE 0 END) AS failed,
    AVG(wi.duration_ms) AS avg_duration_ms,
    MAX(wi.completed_at) AS last_execution
FROM workflow_definitions wd
LEFT JOIN workflow_instances wi ON wd.workflow_id = wi.workflow_id
GROUP BY wd.workflow_id, wd.workflow_name, wd.workflow_type;

-- Recent Workflow Events
CREATE VIEW IF NOT EXISTS v_recent_workflow_events AS
SELECT
    we.event_id,
    we.event_type,
    we.severity,
    we.message,
    wd.workflow_name,
    wi.instance_id,
    we.timestamp,
    we.actor
FROM workflow_events we
LEFT JOIN workflow_instances wi ON we.instance_id = wi.instance_id
LEFT JOIN workflow_definitions wd ON we.workflow_id = wd.workflow_id
WHERE we.timestamp >= datetime('now', '-24 hours')
ORDER BY we.timestamp DESC
LIMIT 100;

-- Lock Status
CREATE VIEW IF NOT EXISTS v_active_locks AS
SELECT
    lock_id,
    resource_type,
    resource_id,
    lock_type,
    holder_type,
    holder_id,
    acquired_at,
    expires_at,
    CASE
        WHEN expires_at IS NOT NULL AND expires_at < datetime('now') THEN 'EXPIRED'
        ELSE 'ACTIVE'
    END AS lock_status,
    (julianday('now') - julianday(acquired_at)) * 86400 AS held_seconds
FROM workflow_locks
WHERE is_active = 1
ORDER BY acquired_at;

--------------------------------------------------------------------------------
-- TRIGGERS
--------------------------------------------------------------------------------

-- Update workflow_instances.updated_at
CREATE TRIGGER IF NOT EXISTS trg_workflow_instance_update
AFTER UPDATE ON workflow_instances
BEGIN
    UPDATE workflow_instances
    SET updated_at = datetime('now')
    WHERE instance_id = NEW.instance_id;
END;

-- Calculate duration on completion
CREATE TRIGGER IF NOT EXISTS trg_workflow_instance_completed
AFTER UPDATE OF status ON workflow_instances
WHEN NEW.status IN ('completed', 'failed', 'cancelled', 'timeout')
AND OLD.status NOT IN ('completed', 'failed', 'cancelled', 'timeout')
BEGIN
    UPDATE workflow_instances
    SET completed_at = datetime('now'),
        duration_ms = (julianday('now') - julianday(started_at)) * 86400000
    WHERE instance_id = NEW.instance_id;
END;

-- Log workflow state changes
CREATE TRIGGER IF NOT EXISTS trg_workflow_state_change
AFTER UPDATE OF status ON workflow_instances
WHEN OLD.status != NEW.status
BEGIN
    INSERT INTO workflow_events (
        event_id, source_type, source_id, event_type, severity,
        message, details, workflow_id, instance_id
    ) VALUES (
        lower(hex(randomblob(16))),
        'workflow',
        NEW.instance_id,
        'state_change',
        CASE NEW.status
            WHEN 'failed' THEN 'error'
            WHEN 'timeout' THEN 'warning'
            ELSE 'info'
        END,
        'Workflow status changed from ' || OLD.status || ' to ' || NEW.status,
        json_object('old_status', OLD.status, 'new_status', NEW.status),
        NEW.workflow_id,
        NEW.instance_id
    );
END;

-- Update task_queue.updated_at
CREATE TRIGGER IF NOT EXISTS trg_task_queue_update
AFTER UPDATE ON task_queue
BEGIN
    UPDATE task_queue
    SET updated_at = datetime('now')
    WHERE task_id = NEW.task_id;
END;

-- Calculate step duration on completion
CREATE TRIGGER IF NOT EXISTS trg_step_execution_completed
AFTER UPDATE OF status ON step_executions
WHEN NEW.status IN ('completed', 'failed', 'skipped', 'cancelled', 'timeout')
AND OLD.status NOT IN ('completed', 'failed', 'skipped', 'cancelled', 'timeout')
BEGIN
    UPDATE step_executions
    SET completed_at = datetime('now'),
        duration_ms = (julianday('now') - julianday(started_at)) * 86400000
    WHERE execution_id = NEW.execution_id;
END;

-- Update workflow progress on step completion
CREATE TRIGGER IF NOT EXISTS trg_update_workflow_progress
AFTER UPDATE OF status ON step_executions
WHEN NEW.status IN ('completed', 'failed', 'skipped')
BEGIN
    UPDATE workflow_instances
    SET completed_steps = (
            SELECT COUNT(*) FROM step_executions
            WHERE instance_id = NEW.instance_id AND status = 'completed'
        ),
        failed_steps = (
            SELECT COUNT(*) FROM step_executions
            WHERE instance_id = NEW.instance_id AND status = 'failed'
        ),
        skipped_steps = (
            SELECT COUNT(*) FROM step_executions
            WHERE instance_id = NEW.instance_id AND status = 'skipped'
        )
    WHERE instance_id = NEW.instance_id;
END;

-- Auto-release expired locks
CREATE TRIGGER IF NOT EXISTS trg_check_lock_expiry
AFTER INSERT ON workflow_locks
BEGIN
    UPDATE workflow_locks
    SET is_active = 0,
        release_reason = 'expired'
    WHERE expires_at < datetime('now')
    AND is_active = 1;
END;

-- Update job statistics after history entry
CREATE TRIGGER IF NOT EXISTS trg_update_job_stats
AFTER INSERT ON job_history
BEGIN
    UPDATE scheduled_jobs
    SET total_runs = total_runs + 1,
        successful_runs = successful_runs + CASE WHEN NEW.status = 'completed' THEN 1 ELSE 0 END,
        failed_runs = failed_runs + CASE WHEN NEW.status = 'failed' THEN 1 ELSE 0 END,
        last_run_at = NEW.started_at,
        last_run_status = NEW.status,
        last_run_duration_ms = NEW.duration_ms
    WHERE job_id = NEW.job_id;
END;

