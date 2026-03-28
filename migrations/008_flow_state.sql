-- Migration: 008_flow_state.sql
-- Purpose: Flow state transitions and state machine tracking
-- Database: flow_state.db

--------------------------------------------------------------------------------
-- FLOW STATE MACHINES
--------------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS state_machines (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    machine_id TEXT NOT NULL UNIQUE,
    machine_name TEXT NOT NULL,
    service_id TEXT NOT NULL,
    current_state TEXT NOT NULL,
    previous_state TEXT,
    initial_state TEXT NOT NULL,
    terminal_states TEXT NOT NULL,  -- JSON array of terminal states
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),

    FOREIGN KEY (service_id) REFERENCES services(id)
);

--------------------------------------------------------------------------------
-- STATE DEFINITIONS
--------------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS state_definitions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    state_id TEXT NOT NULL UNIQUE,
    machine_id TEXT NOT NULL,
    state_name TEXT NOT NULL,
    state_type TEXT NOT NULL CHECK (state_type IN ('initial', 'normal', 'terminal', 'error')),
    description TEXT,
    entry_actions TEXT,   -- JSON array of actions on entry
    exit_actions TEXT,    -- JSON array of actions on exit
    timeout_seconds INTEGER,
    timeout_transition_to TEXT,  -- state to transition to on timeout

    UNIQUE(machine_id, state_name),
    FOREIGN KEY (machine_id) REFERENCES state_machines(machine_id)
);

--------------------------------------------------------------------------------
-- STATE TRANSITIONS
--------------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS transition_definitions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    transition_id TEXT NOT NULL UNIQUE,
    machine_id TEXT NOT NULL,
    from_state TEXT NOT NULL,
    to_state TEXT NOT NULL,
    trigger_event TEXT NOT NULL,
    guard_condition TEXT,  -- JSON expression for guard
    actions TEXT,          -- JSON array of transition actions
    priority INTEGER NOT NULL DEFAULT 0,
    is_enabled INTEGER NOT NULL DEFAULT 1,

    UNIQUE(machine_id, from_state, trigger_event),
    FOREIGN KEY (machine_id) REFERENCES state_machines(machine_id)
);

--------------------------------------------------------------------------------
-- FLOW STATE HISTORY
--------------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS state_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    history_id TEXT NOT NULL UNIQUE,
    machine_id TEXT NOT NULL,
    from_state TEXT NOT NULL,
    to_state TEXT NOT NULL,
    trigger_event TEXT NOT NULL,
    transition_id TEXT,
    triggered_by TEXT,  -- agent or system that triggered
    metadata TEXT,      -- JSON
    timestamp TEXT NOT NULL DEFAULT (datetime('now')),
    duration_in_previous_ms INTEGER,

    FOREIGN KEY (machine_id) REFERENCES state_machines(machine_id),
    FOREIGN KEY (transition_id) REFERENCES transition_definitions(transition_id)
);

--------------------------------------------------------------------------------
-- ACTIVE TIMERS
--------------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS state_timers (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    timer_id TEXT NOT NULL UNIQUE,
    machine_id TEXT NOT NULL,
    state_name TEXT NOT NULL,
    timer_type TEXT NOT NULL CHECK (timer_type IN ('timeout', 'delay', 'periodic')),
    duration_seconds INTEGER NOT NULL,
    started_at TEXT NOT NULL DEFAULT (datetime('now')),
    expires_at TEXT NOT NULL,
    action_on_expire TEXT,  -- JSON
    is_active INTEGER NOT NULL DEFAULT 1,
    fired_at TEXT,

    FOREIGN KEY (machine_id) REFERENCES state_machines(machine_id)
);

--------------------------------------------------------------------------------
-- FLOW EVENTS
--------------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS flow_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    event_id TEXT NOT NULL UNIQUE,
    event_type TEXT NOT NULL,
    source TEXT NOT NULL,
    target_machine TEXT,
    payload TEXT,  -- JSON
    priority INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    processed_at TEXT,
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'processing', 'processed', 'failed', 'dropped')),
    error_message TEXT
);

--------------------------------------------------------------------------------
-- STATE SNAPSHOTS (for recovery)
--------------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS state_snapshots (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    snapshot_id TEXT NOT NULL UNIQUE,
    machine_id TEXT NOT NULL,
    state_name TEXT NOT NULL,
    machine_context TEXT,  -- JSON of any context data
    pending_timers TEXT,   -- JSON array of active timers
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    is_valid INTEGER NOT NULL DEFAULT 1,

    FOREIGN KEY (machine_id) REFERENCES state_machines(machine_id)
);

--------------------------------------------------------------------------------
-- FLOW ORCHESTRATION
--------------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS flow_orchestrations (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    orchestration_id TEXT NOT NULL UNIQUE,
    orchestration_name TEXT NOT NULL,
    description TEXT,
    participant_machines TEXT NOT NULL,  -- JSON array of machine_ids
    coordination_type TEXT NOT NULL CHECK (coordination_type IN ('sequential', 'parallel', 'choreography', 'saga')),
    current_phase TEXT,
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'running', 'completed', 'failed', 'compensating')),
    started_at TEXT,
    completed_at TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS orchestration_steps (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    step_id TEXT NOT NULL UNIQUE,
    orchestration_id TEXT NOT NULL,
    step_index INTEGER NOT NULL,
    machine_id TEXT NOT NULL,
    expected_state TEXT NOT NULL,
    compensation_action TEXT,  -- JSON for saga rollback
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'running', 'completed', 'failed', 'compensated')),
    started_at TEXT,
    completed_at TEXT,

    UNIQUE(orchestration_id, step_index),
    FOREIGN KEY (orchestration_id) REFERENCES flow_orchestrations(orchestration_id),
    FOREIGN KEY (machine_id) REFERENCES state_machines(machine_id)
);

--------------------------------------------------------------------------------
-- STATE MACHINE METRICS
--------------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS state_metrics (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    machine_id TEXT NOT NULL,
    state_name TEXT NOT NULL,
    period_start TEXT NOT NULL,
    period_end TEXT NOT NULL,
    entry_count INTEGER NOT NULL DEFAULT 0,
    total_time_ms INTEGER NOT NULL DEFAULT 0,
    avg_duration_ms REAL,
    min_duration_ms INTEGER,
    max_duration_ms INTEGER,
    timeout_count INTEGER NOT NULL DEFAULT 0,

    UNIQUE(machine_id, state_name, period_start),
    FOREIGN KEY (machine_id) REFERENCES state_machines(machine_id)
);

--------------------------------------------------------------------------------
-- INDEXES
--------------------------------------------------------------------------------
CREATE INDEX IF NOT EXISTS idx_state_machines_service ON state_machines(service_id);
CREATE INDEX IF NOT EXISTS idx_state_machines_current ON state_machines(current_state);
CREATE INDEX IF NOT EXISTS idx_state_definitions_machine ON state_definitions(machine_id);
CREATE INDEX IF NOT EXISTS idx_transition_defs_machine ON transition_definitions(machine_id);
CREATE INDEX IF NOT EXISTS idx_transition_defs_from ON transition_definitions(from_state);
CREATE INDEX IF NOT EXISTS idx_state_history_machine ON state_history(machine_id);
CREATE INDEX IF NOT EXISTS idx_state_history_timestamp ON state_history(timestamp);
CREATE INDEX IF NOT EXISTS idx_state_timers_active ON state_timers(is_active) WHERE is_active = 1;
CREATE INDEX IF NOT EXISTS idx_state_timers_expires ON state_timers(expires_at);
CREATE INDEX IF NOT EXISTS idx_flow_events_status ON flow_events(status);
CREATE INDEX IF NOT EXISTS idx_flow_events_target ON flow_events(target_machine);
CREATE INDEX IF NOT EXISTS idx_orchestrations_status ON flow_orchestrations(status);

--------------------------------------------------------------------------------
-- VIEWS
--------------------------------------------------------------------------------
CREATE VIEW IF NOT EXISTS v_active_state_machines AS
SELECT
    sm.machine_id,
    sm.machine_name,
    sm.service_id,
    sm.current_state,
    sd.state_type,
    sm.updated_at,
    (julianday('now') - julianday(sm.updated_at)) * 86400000 AS ms_in_current_state,
    (SELECT COUNT(*) FROM state_history sh WHERE sh.machine_id = sm.machine_id) AS total_transitions
FROM state_machines sm
LEFT JOIN state_definitions sd ON sm.machine_id = sd.machine_id AND sm.current_state = sd.state_name
WHERE sm.current_state NOT IN (
    SELECT json_each.value FROM state_machines sm2, json_each(sm2.terminal_states)
    WHERE sm2.machine_id = sm.machine_id
);

CREATE VIEW IF NOT EXISTS v_pending_timers AS
SELECT
    st.timer_id,
    st.machine_id,
    sm.machine_name,
    st.state_name,
    st.timer_type,
    st.expires_at,
    (julianday(st.expires_at) - julianday('now')) * 86400 AS seconds_remaining,
    st.action_on_expire
FROM state_timers st
JOIN state_machines sm ON st.machine_id = sm.machine_id
WHERE st.is_active = 1
AND st.expires_at > datetime('now')
ORDER BY st.expires_at;

CREATE VIEW IF NOT EXISTS v_recent_transitions AS
SELECT
    sh.history_id,
    sh.machine_id,
    sm.machine_name,
    sh.from_state,
    sh.to_state,
    sh.trigger_event,
    sh.triggered_by,
    sh.timestamp,
    sh.duration_in_previous_ms
FROM state_history sh
JOIN state_machines sm ON sh.machine_id = sm.machine_id
WHERE sh.timestamp >= datetime('now', '-1 hour')
ORDER BY sh.timestamp DESC;

CREATE VIEW IF NOT EXISTS v_state_distribution AS
SELECT
    sm.current_state,
    sd.state_type,
    COUNT(*) AS machine_count,
    GROUP_CONCAT(sm.machine_name, ', ') AS machines
FROM state_machines sm
LEFT JOIN state_definitions sd ON sm.machine_id = sd.machine_id AND sm.current_state = sd.state_name
GROUP BY sm.current_state, sd.state_type
ORDER BY machine_count DESC;

--------------------------------------------------------------------------------
-- TRIGGERS
--------------------------------------------------------------------------------
CREATE TRIGGER IF NOT EXISTS trg_state_machine_update
AFTER UPDATE OF current_state ON state_machines
BEGIN
    UPDATE state_machines SET
        previous_state = OLD.current_state,
        updated_at = datetime('now')
    WHERE machine_id = NEW.machine_id;
END;

CREATE TRIGGER IF NOT EXISTS trg_log_state_transition
AFTER UPDATE OF current_state ON state_machines
WHEN OLD.current_state != NEW.current_state
BEGIN
    INSERT INTO state_history (
        history_id,
        machine_id,
        from_state,
        to_state,
        trigger_event,
        triggered_by,
        duration_in_previous_ms
    ) VALUES (
        lower(hex(randomblob(16))),
        NEW.machine_id,
        OLD.current_state,
        NEW.current_state,
        'state_update',
        'system',
        (julianday('now') - julianday(OLD.updated_at)) * 86400000
    );
END;

CREATE TRIGGER IF NOT EXISTS trg_deactivate_timer_on_state_exit
AFTER UPDATE OF current_state ON state_machines
WHEN OLD.current_state != NEW.current_state
BEGIN
    UPDATE state_timers SET
        is_active = 0,
        fired_at = datetime('now')
    WHERE machine_id = NEW.machine_id
    AND state_name = OLD.current_state
    AND is_active = 1;
END;

--------------------------------------------------------------------------------
-- WORKFLOW INTEGRATION TABLES
--------------------------------------------------------------------------------

-- Workflow State Machine Mapping
CREATE TABLE IF NOT EXISTS workflow_state_machines (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    mapping_id TEXT NOT NULL UNIQUE DEFAULT (lower(hex(randomblob(16)))),

    -- References
    workflow_instance_id TEXT NOT NULL,  -- References workflow_tracking.workflow_instances
    machine_id TEXT NOT NULL,

    -- Mapping type
    mapping_type TEXT NOT NULL CHECK (mapping_type IN (
        'driver', 'observer', 'controlled', 'synchronized'
    )),

    -- State synchronization
    sync_states INTEGER DEFAULT 1,  -- Whether to sync state changes

    -- Metadata
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    deactivated_at TEXT,

    FOREIGN KEY (machine_id) REFERENCES state_machines(machine_id),
    UNIQUE(workflow_instance_id, machine_id)
);

-- Workflow-Driven Transitions
CREATE TABLE IF NOT EXISTS workflow_transitions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    transition_id TEXT NOT NULL UNIQUE DEFAULT (lower(hex(randomblob(16)))),

    -- References
    workflow_instance_id TEXT NOT NULL,
    step_execution_id TEXT,  -- References workflow_tracking.step_executions
    machine_id TEXT NOT NULL,

    -- Transition details
    from_state TEXT NOT NULL,
    to_state TEXT NOT NULL,
    trigger_event TEXT NOT NULL,

    -- Outcome
    success INTEGER NOT NULL,
    error_message TEXT,

    -- Timing
    requested_at TEXT NOT NULL DEFAULT (datetime('now')),
    completed_at TEXT,
    duration_ms INTEGER,

    FOREIGN KEY (machine_id) REFERENCES state_machines(machine_id)
);

-- Flow Orchestration Workflows
CREATE TABLE IF NOT EXISTS flow_orchestration_workflows (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    mapping_id TEXT NOT NULL UNIQUE DEFAULT (lower(hex(randomblob(16)))),

    -- References
    orchestration_id TEXT NOT NULL,
    workflow_instance_id TEXT NOT NULL,

    -- Role
    role TEXT NOT NULL CHECK (role IN ('initiator', 'participant', 'observer')),

    -- Status
    is_active INTEGER DEFAULT 1,

    -- Metadata
    created_at TEXT NOT NULL DEFAULT (datetime('now')),

    FOREIGN KEY (orchestration_id) REFERENCES flow_orchestrations(orchestration_id)
);

-- Indexes for workflow integration
CREATE INDEX IF NOT EXISTS idx_workflow_state_machines_workflow ON workflow_state_machines(workflow_instance_id);
CREATE INDEX IF NOT EXISTS idx_workflow_state_machines_machine ON workflow_state_machines(machine_id);
CREATE INDEX IF NOT EXISTS idx_workflow_transitions_workflow ON workflow_transitions(workflow_instance_id);
CREATE INDEX IF NOT EXISTS idx_workflow_transitions_machine ON workflow_transitions(machine_id);
CREATE INDEX IF NOT EXISTS idx_flow_orch_workflows_orch ON flow_orchestration_workflows(orchestration_id);
CREATE INDEX IF NOT EXISTS idx_flow_orch_workflows_workflow ON flow_orchestration_workflows(workflow_instance_id);

--------------------------------------------------------------------------------
-- CROSS-DATABASE VIEWS
--------------------------------------------------------------------------------

-- Workflow state machine status
CREATE VIEW IF NOT EXISTS v_workflow_state_machine_status AS
SELECT
    wsm.workflow_instance_id,
    sm.machine_id,
    sm.machine_name,
    sm.current_state,
    sm.previous_state,
    sd.state_type,
    wsm.mapping_type,
    (julianday('now') - julianday(sm.updated_at)) * 86400000 AS ms_in_state,
    (SELECT COUNT(*) FROM workflow_transitions wt
     WHERE wt.workflow_instance_id = wsm.workflow_instance_id
     AND wt.machine_id = sm.machine_id) AS workflow_transitions
FROM workflow_state_machines wsm
JOIN state_machines sm ON wsm.machine_id = sm.machine_id
LEFT JOIN state_definitions sd ON sm.machine_id = sd.machine_id AND sm.current_state = sd.state_name
WHERE wsm.deactivated_at IS NULL
ORDER BY wsm.workflow_instance_id, sm.machine_name;

-- Workflow transition history
CREATE VIEW IF NOT EXISTS v_workflow_transition_history AS
SELECT
    wt.workflow_instance_id,
    wt.machine_id,
    sm.machine_name,
    wt.from_state,
    wt.to_state,
    wt.trigger_event,
    wt.success,
    wt.error_message,
    wt.requested_at,
    wt.duration_ms,
    wt.step_execution_id
FROM workflow_transitions wt
JOIN state_machines sm ON wt.machine_id = sm.machine_id
ORDER BY wt.requested_at DESC;

-- Active flow orchestrations with workflows
CREATE VIEW IF NOT EXISTS v_active_orchestrations_with_workflows AS
SELECT
    fo.orchestration_id,
    fo.orchestration_name,
    fo.coordination_type,
    fo.current_phase,
    fo.status AS orchestration_status,
    fow.workflow_instance_id,
    fow.role AS workflow_role,
    (SELECT COUNT(*) FROM orchestration_steps os
     WHERE os.orchestration_id = fo.orchestration_id
     AND os.status = 'completed') AS completed_steps,
    (SELECT COUNT(*) FROM orchestration_steps os
     WHERE os.orchestration_id = fo.orchestration_id) AS total_steps
FROM flow_orchestrations fo
JOIN flow_orchestration_workflows fow ON fo.orchestration_id = fow.orchestration_id
WHERE fo.status IN ('pending', 'running')
AND fow.is_active = 1
ORDER BY fo.started_at DESC;

-- State machine workflow impact
CREATE VIEW IF NOT EXISTS v_state_machine_workflow_impact AS
SELECT
    sm.machine_id,
    sm.machine_name,
    sm.service_id,
    COUNT(DISTINCT wsm.workflow_instance_id) AS active_workflows,
    COUNT(wt.id) AS total_workflow_transitions,
    SUM(CASE WHEN wt.success = 1 THEN 1 ELSE 0 END) AS successful_transitions,
    SUM(CASE WHEN wt.success = 0 THEN 1 ELSE 0 END) AS failed_transitions,
    AVG(wt.duration_ms) AS avg_transition_duration_ms
FROM state_machines sm
LEFT JOIN workflow_state_machines wsm ON sm.machine_id = wsm.machine_id AND wsm.deactivated_at IS NULL
LEFT JOIN workflow_transitions wt ON sm.machine_id = wt.machine_id
WHERE wt.requested_at >= datetime('now', '-24 hours') OR wt.requested_at IS NULL
GROUP BY sm.machine_id, sm.machine_name, sm.service_id;

--------------------------------------------------------------------------------
-- WORKFLOW INTEGRATION TRIGGERS
--------------------------------------------------------------------------------

-- Log workflow-driven state transitions
CREATE TRIGGER IF NOT EXISTS trg_log_workflow_transition
AFTER INSERT ON workflow_transitions
WHEN NEW.success = 1
BEGIN
    INSERT INTO state_history (
        history_id,
        machine_id,
        from_state,
        to_state,
        trigger_event,
        triggered_by,
        metadata,
        duration_in_previous_ms
    ) VALUES (
        lower(hex(randomblob(16))),
        NEW.machine_id,
        NEW.from_state,
        NEW.to_state,
        NEW.trigger_event,
        'workflow:' || NEW.workflow_instance_id,
        json_object('step_execution_id', NEW.step_execution_id),
        NEW.duration_ms
    );

    -- Update state machine
    UPDATE state_machines
    SET current_state = NEW.to_state,
        previous_state = NEW.from_state
    WHERE machine_id = NEW.machine_id;
END;

-- Create flow event for workflow transitions
CREATE TRIGGER IF NOT EXISTS trg_workflow_flow_event
AFTER INSERT ON workflow_transitions
BEGIN
    INSERT INTO flow_events (
        event_id,
        event_type,
        source,
        target_machine,
        payload,
        priority,
        status
    ) VALUES (
        lower(hex(randomblob(16))),
        'workflow_transition',
        'workflow:' || NEW.workflow_instance_id,
        NEW.machine_id,
        json_object(
            'from_state', NEW.from_state,
            'to_state', NEW.to_state,
            'trigger', NEW.trigger_event,
            'success', NEW.success
        ),
        5,
        'processed'
    );
END;

-- Complete workflow transition timing
CREATE TRIGGER IF NOT EXISTS trg_complete_workflow_transition
AFTER UPDATE OF to_state ON state_machines
BEGIN
    UPDATE workflow_transitions
    SET completed_at = datetime('now'),
        duration_ms = (julianday('now') - julianday(requested_at)) * 86400000
    WHERE machine_id = NEW.machine_id
    AND to_state = NEW.current_state
    AND completed_at IS NULL;
END;
