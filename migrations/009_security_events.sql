-- Migration: 009_security_events.sql
-- Purpose: Security monitoring and event tracking
-- Database: security_events.db

--------------------------------------------------------------------------------
-- SECURITY EVENTS TABLE
--------------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS security_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    event_id TEXT NOT NULL UNIQUE,
    event_type TEXT NOT NULL CHECK (event_type IN (
        'auth_success', 'auth_failure', 'auth_lockout',
        'access_granted', 'access_denied', 'privilege_escalation',
        'config_change', 'service_start', 'service_stop',
        'anomaly_detected', 'rate_limit', 'input_validation',
        'encryption_failure', 'certificate_expiry', 'audit_failure'
    )),
    severity TEXT NOT NULL CHECK (severity IN ('info', 'low', 'medium', 'high', 'critical')),
    source_service TEXT NOT NULL,
    source_ip TEXT,
    source_agent TEXT,
    target_resource TEXT,
    action TEXT NOT NULL,
    outcome TEXT NOT NULL CHECK (outcome IN ('success', 'failure', 'blocked', 'pending')),
    description TEXT,
    metadata TEXT,  -- JSON
    timestamp TEXT NOT NULL DEFAULT (datetime('now')),
    processed INTEGER NOT NULL DEFAULT 0,
    acknowledged INTEGER NOT NULL DEFAULT 0,
    acknowledged_by TEXT,
    acknowledged_at TEXT
);

--------------------------------------------------------------------------------
-- SECURITY ALERTS
--------------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS security_alerts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    alert_id TEXT NOT NULL UNIQUE,
    alert_type TEXT NOT NULL CHECK (alert_type IN (
        'brute_force', 'anomalous_access', 'privilege_abuse',
        'data_exfiltration', 'config_tampering', 'service_compromise',
        'certificate_issue', 'encryption_weakness', 'compliance_violation'
    )),
    severity TEXT NOT NULL CHECK (severity IN ('low', 'medium', 'high', 'critical')),
    title TEXT NOT NULL,
    description TEXT NOT NULL,
    related_events TEXT,  -- JSON array of event_ids
    source_service TEXT NOT NULL,
    affected_services TEXT,  -- JSON array
    recommended_actions TEXT,  -- JSON array
    status TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open', 'investigating', 'mitigated', 'resolved', 'false_positive')),
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    resolved_at TEXT,
    assigned_to TEXT,
    resolution_notes TEXT
);

--------------------------------------------------------------------------------
-- ACCESS CONTROL AUDIT
--------------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS access_audit (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    audit_id TEXT NOT NULL UNIQUE,
    principal_type TEXT NOT NULL CHECK (principal_type IN ('user', 'agent', 'service', 'system')),
    principal_id TEXT NOT NULL,
    resource_type TEXT NOT NULL,
    resource_id TEXT NOT NULL,
    action TEXT NOT NULL CHECK (action IN ('read', 'write', 'delete', 'execute', 'admin')),
    permission_level TEXT NOT NULL CHECK (permission_level IN ('none', 'read', 'write', 'admin')),
    decision TEXT NOT NULL CHECK (decision IN ('allow', 'deny')),
    decision_reason TEXT,
    timestamp TEXT NOT NULL DEFAULT (datetime('now')),
    request_context TEXT  -- JSON
);

--------------------------------------------------------------------------------
-- SECURITY POLICIES
--------------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS security_policies (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    policy_id TEXT NOT NULL UNIQUE,
    policy_name TEXT NOT NULL,
    policy_type TEXT NOT NULL CHECK (policy_type IN (
        'access_control', 'rate_limiting', 'encryption',
        'authentication', 'audit', 'compliance'
    )),
    scope TEXT NOT NULL CHECK (scope IN ('global', 'service', 'resource')),
    scope_target TEXT,  -- service_id or resource pattern if not global
    rules TEXT NOT NULL,  -- JSON
    is_enabled INTEGER NOT NULL DEFAULT 1,
    enforcement_level TEXT NOT NULL CHECK (enforcement_level IN ('monitor', 'warn', 'enforce')),
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    created_by TEXT,
    version INTEGER NOT NULL DEFAULT 1
);

--------------------------------------------------------------------------------
-- RATE LIMITING
--------------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS rate_limit_buckets (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    bucket_id TEXT NOT NULL UNIQUE,
    identifier TEXT NOT NULL,  -- IP, service_id, agent_id, etc.
    identifier_type TEXT NOT NULL CHECK (identifier_type IN ('ip', 'service', 'agent', 'api_key')),
    resource TEXT NOT NULL,
    window_start TEXT NOT NULL,
    window_seconds INTEGER NOT NULL,
    request_count INTEGER NOT NULL DEFAULT 0,
    limit_value INTEGER NOT NULL,
    is_blocked INTEGER NOT NULL DEFAULT 0,
    blocked_until TEXT,

    UNIQUE(identifier, identifier_type, resource, window_start)
);

--------------------------------------------------------------------------------
-- CERTIFICATE TRACKING
--------------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS certificate_inventory (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    cert_id TEXT NOT NULL UNIQUE,
    service_id TEXT NOT NULL,
    cert_type TEXT NOT NULL CHECK (cert_type IN ('tls', 'mtls', 'signing', 'encryption')),
    subject TEXT NOT NULL,
    issuer TEXT NOT NULL,
    serial_number TEXT NOT NULL,
    fingerprint_sha256 TEXT NOT NULL,
    not_before TEXT NOT NULL,
    not_after TEXT NOT NULL,
    days_until_expiry INTEGER GENERATED ALWAYS AS (
        julianday(not_after) - julianday('now')
    ) STORED,
    key_algorithm TEXT,
    key_size INTEGER,
    is_active INTEGER NOT NULL DEFAULT 1,
    last_rotation TEXT,
    auto_renew INTEGER NOT NULL DEFAULT 0,

    FOREIGN KEY (service_id) REFERENCES services(id)
);

--------------------------------------------------------------------------------
-- THREAT INTELLIGENCE
--------------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS threat_indicators (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    indicator_id TEXT NOT NULL UNIQUE,
    indicator_type TEXT NOT NULL CHECK (indicator_type IN ('ip', 'domain', 'hash', 'pattern', 'behavior')),
    indicator_value TEXT NOT NULL,
    threat_type TEXT NOT NULL CHECK (threat_type IN (
        'malware', 'phishing', 'brute_force', 'dos',
        'data_theft', 'privilege_escalation', 'lateral_movement'
    )),
    severity TEXT NOT NULL CHECK (severity IN ('low', 'medium', 'high', 'critical')),
    confidence REAL NOT NULL CHECK (confidence BETWEEN 0.0 AND 1.0),
    source TEXT NOT NULL,
    first_seen TEXT NOT NULL DEFAULT (datetime('now')),
    last_seen TEXT NOT NULL DEFAULT (datetime('now')),
    expiry TEXT,
    is_active INTEGER NOT NULL DEFAULT 1,
    hit_count INTEGER NOT NULL DEFAULT 0,
    metadata TEXT  -- JSON
);

--------------------------------------------------------------------------------
-- COMPLIANCE TRACKING
--------------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS compliance_checks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    check_id TEXT NOT NULL UNIQUE,
    framework TEXT NOT NULL,  -- e.g., 'NAM', 'SOC2', 'GDPR'
    requirement_id TEXT NOT NULL,
    requirement_name TEXT NOT NULL,
    check_type TEXT NOT NULL CHECK (check_type IN ('automated', 'manual', 'hybrid')),
    service_id TEXT,
    last_check TEXT,
    next_check TEXT,
    status TEXT NOT NULL CHECK (status IN ('pass', 'fail', 'partial', 'not_applicable', 'pending')),
    evidence TEXT,  -- JSON
    remediation_status TEXT CHECK (remediation_status IN ('not_required', 'pending', 'in_progress', 'completed')),
    notes TEXT
);

--------------------------------------------------------------------------------
-- INCIDENT RESPONSE
--------------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS security_incidents (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    incident_id TEXT NOT NULL UNIQUE,
    incident_type TEXT NOT NULL,
    severity TEXT NOT NULL CHECK (severity IN ('low', 'medium', 'high', 'critical')),
    title TEXT NOT NULL,
    description TEXT NOT NULL,
    affected_services TEXT,  -- JSON array
    affected_agents TEXT,    -- JSON array
    root_cause TEXT,
    timeline TEXT,  -- JSON array of {timestamp, action, actor}
    status TEXT NOT NULL DEFAULT 'detected' CHECK (status IN (
        'detected', 'triaging', 'contained', 'eradicating', 'recovering', 'resolved', 'post_mortem'
    )),
    detected_at TEXT NOT NULL DEFAULT (datetime('now')),
    contained_at TEXT,
    resolved_at TEXT,
    lead_responder TEXT,
    related_alerts TEXT,  -- JSON array of alert_ids
    lessons_learned TEXT,
    preventive_measures TEXT  -- JSON array
);

--------------------------------------------------------------------------------
-- INDEXES
--------------------------------------------------------------------------------
CREATE INDEX IF NOT EXISTS idx_security_events_type ON security_events(event_type);
CREATE INDEX IF NOT EXISTS idx_security_events_severity ON security_events(severity);
CREATE INDEX IF NOT EXISTS idx_security_events_source ON security_events(source_service);
CREATE INDEX IF NOT EXISTS idx_security_events_timestamp ON security_events(timestamp);
CREATE INDEX IF NOT EXISTS idx_security_events_unprocessed ON security_events(processed) WHERE processed = 0;
CREATE INDEX IF NOT EXISTS idx_security_alerts_status ON security_alerts(status);
CREATE INDEX IF NOT EXISTS idx_security_alerts_severity ON security_alerts(severity);
CREATE INDEX IF NOT EXISTS idx_access_audit_principal ON access_audit(principal_id);
CREATE INDEX IF NOT EXISTS idx_access_audit_resource ON access_audit(resource_id);
CREATE INDEX IF NOT EXISTS idx_access_audit_timestamp ON access_audit(timestamp);
CREATE INDEX IF NOT EXISTS idx_rate_limits_identifier ON rate_limit_buckets(identifier, identifier_type);
CREATE INDEX IF NOT EXISTS idx_rate_limits_blocked ON rate_limit_buckets(is_blocked) WHERE is_blocked = 1;
CREATE INDEX IF NOT EXISTS idx_certs_expiry ON certificate_inventory(days_until_expiry);
CREATE INDEX IF NOT EXISTS idx_certs_service ON certificate_inventory(service_id);
CREATE INDEX IF NOT EXISTS idx_threat_indicators_active ON threat_indicators(is_active) WHERE is_active = 1;
CREATE INDEX IF NOT EXISTS idx_threat_indicators_type ON threat_indicators(indicator_type);
CREATE INDEX IF NOT EXISTS idx_compliance_status ON compliance_checks(status);
CREATE INDEX IF NOT EXISTS idx_incidents_status ON security_incidents(status);
CREATE INDEX IF NOT EXISTS idx_incidents_severity ON security_incidents(severity);

--------------------------------------------------------------------------------
-- VIEWS
--------------------------------------------------------------------------------
CREATE VIEW IF NOT EXISTS v_active_alerts AS
SELECT
    alert_id,
    alert_type,
    severity,
    title,
    source_service,
    status,
    created_at,
    assigned_to,
    (julianday('now') - julianday(created_at)) * 24 AS hours_open
FROM security_alerts
WHERE status NOT IN ('resolved', 'false_positive')
ORDER BY
    CASE severity
        WHEN 'critical' THEN 1
        WHEN 'high' THEN 2
        WHEN 'medium' THEN 3
        WHEN 'low' THEN 4
    END,
    created_at;

CREATE VIEW IF NOT EXISTS v_expiring_certificates AS
SELECT
    cert_id,
    service_id,
    cert_type,
    subject,
    not_after,
    days_until_expiry,
    CASE
        WHEN days_until_expiry <= 7 THEN 'critical'
        WHEN days_until_expiry <= 30 THEN 'warning'
        WHEN days_until_expiry <= 90 THEN 'notice'
        ELSE 'ok'
    END AS urgency
FROM certificate_inventory
WHERE is_active = 1
AND days_until_expiry <= 90
ORDER BY days_until_expiry;

CREATE VIEW IF NOT EXISTS v_security_summary AS
SELECT
    (SELECT COUNT(*) FROM security_events WHERE timestamp >= datetime('now', '-24 hours')) AS events_24h,
    (SELECT COUNT(*) FROM security_events WHERE severity IN ('high', 'critical') AND timestamp >= datetime('now', '-24 hours')) AS high_severity_24h,
    (SELECT COUNT(*) FROM security_alerts WHERE status NOT IN ('resolved', 'false_positive')) AS open_alerts,
    (SELECT COUNT(*) FROM security_alerts WHERE severity = 'critical' AND status NOT IN ('resolved', 'false_positive')) AS critical_alerts,
    (SELECT COUNT(*) FROM certificate_inventory WHERE is_active = 1 AND days_until_expiry <= 30) AS certs_expiring_30d,
    (SELECT COUNT(*) FROM security_incidents WHERE status NOT IN ('resolved', 'post_mortem')) AS active_incidents,
    (SELECT COUNT(*) FROM compliance_checks WHERE status = 'fail') AS failed_compliance_checks;

CREATE VIEW IF NOT EXISTS v_auth_failures_by_source AS
SELECT
    source_ip,
    source_service,
    COUNT(*) AS failure_count,
    MIN(timestamp) AS first_failure,
    MAX(timestamp) AS last_failure
FROM security_events
WHERE event_type = 'auth_failure'
AND timestamp >= datetime('now', '-1 hour')
GROUP BY source_ip, source_service
HAVING COUNT(*) >= 3
ORDER BY failure_count DESC;

--------------------------------------------------------------------------------
-- TRIGGERS
--------------------------------------------------------------------------------
CREATE TRIGGER IF NOT EXISTS trg_alert_on_critical_event
AFTER INSERT ON security_events
WHEN NEW.severity = 'critical'
BEGIN
    INSERT INTO security_alerts (
        alert_id,
        alert_type,
        severity,
        title,
        description,
        related_events,
        source_service,
        status
    ) VALUES (
        lower(hex(randomblob(16))),
        CASE NEW.event_type
            WHEN 'auth_failure' THEN 'brute_force'
            WHEN 'privilege_escalation' THEN 'privilege_abuse'
            WHEN 'config_change' THEN 'config_tampering'
            ELSE 'anomalous_access'
        END,
        'critical',
        'Critical security event: ' || NEW.event_type,
        NEW.description,
        json_array(NEW.event_id),
        NEW.source_service,
        'open'
    );
END;

CREATE TRIGGER IF NOT EXISTS trg_update_threat_indicator_hit
AFTER INSERT ON security_events
BEGIN
    UPDATE threat_indicators
    SET hit_count = hit_count + 1,
        last_seen = datetime('now')
    WHERE is_active = 1
    AND (
        (indicator_type = 'ip' AND indicator_value = NEW.source_ip)
        OR (indicator_type = 'pattern' AND NEW.description LIKE '%' || indicator_value || '%')
    );
END;

CREATE TRIGGER IF NOT EXISTS trg_alert_status_update
AFTER UPDATE OF status ON security_alerts
WHEN NEW.status IN ('resolved', 'false_positive')
BEGIN
    UPDATE security_alerts
    SET resolved_at = datetime('now'),
        updated_at = datetime('now')
    WHERE alert_id = NEW.alert_id;
END;

CREATE TRIGGER IF NOT EXISTS trg_compliance_remediation_tracking
AFTER UPDATE OF status ON compliance_checks
WHEN OLD.status = 'pass' AND NEW.status = 'fail'
BEGIN
    UPDATE compliance_checks
    SET remediation_status = 'pending'
    WHERE check_id = NEW.check_id;
END;

--------------------------------------------------------------------------------
-- WORKFLOW INTEGRATION TABLES
--------------------------------------------------------------------------------

-- Workflow Security Events
CREATE TABLE IF NOT EXISTS workflow_security_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    event_id TEXT NOT NULL UNIQUE DEFAULT (lower(hex(randomblob(16)))),

    -- Workflow reference
    workflow_instance_id TEXT NOT NULL,  -- References workflow_tracking.workflow_instances
    step_execution_id TEXT,  -- References workflow_tracking.step_executions

    -- Security event type
    security_event_type TEXT NOT NULL CHECK (security_event_type IN (
        'workflow_started', 'workflow_completed', 'workflow_failed',
        'step_executed', 'approval_requested', 'approval_granted', 'approval_denied',
        'privilege_used', 'resource_accessed', 'config_modified',
        'sensitive_action', 'escalation', 'rollback'
    )),

    -- Context
    actor TEXT NOT NULL,  -- agent_id or user_id
    actor_type TEXT NOT NULL CHECK (actor_type IN ('user', 'agent', 'system', 'workflow')),
    target_resource TEXT,
    action_performed TEXT NOT NULL,

    -- Risk assessment
    risk_level TEXT NOT NULL DEFAULT 'low' CHECK (risk_level IN ('low', 'medium', 'high', 'critical')),

    -- Details
    details TEXT,  -- JSON
    ip_address TEXT,

    -- Timestamp
    timestamp TEXT NOT NULL DEFAULT (datetime('now')),

    -- Audit flag
    requires_audit INTEGER DEFAULT 0
);

-- Workflow Access Control
CREATE TABLE IF NOT EXISTS workflow_access_control (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    access_id TEXT NOT NULL UNIQUE DEFAULT (lower(hex(randomblob(16)))),

    -- Workflow reference
    workflow_id TEXT NOT NULL,  -- References workflow_tracking.workflow_definitions

    -- Access rules
    principal_type TEXT NOT NULL CHECK (principal_type IN ('user', 'agent', 'role', 'group')),
    principal_id TEXT NOT NULL,

    -- Permissions
    can_execute INTEGER DEFAULT 0,
    can_approve INTEGER DEFAULT 0,
    can_cancel INTEGER DEFAULT 0,
    can_view INTEGER DEFAULT 1,
    can_modify INTEGER DEFAULT 0,

    -- Constraints
    require_approval_above_tier TEXT,  -- Requires approval for actions above this tier
    max_concurrent INTEGER,  -- Max concurrent executions allowed

    -- Validity
    valid_from TEXT DEFAULT (datetime('now')),
    valid_until TEXT,
    is_active INTEGER DEFAULT 1,

    -- Metadata
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    created_by TEXT,

    UNIQUE(workflow_id, principal_type, principal_id)
);

-- Workflow Audit Trail
CREATE TABLE IF NOT EXISTS workflow_audit_trail (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    audit_id TEXT NOT NULL UNIQUE DEFAULT (lower(hex(randomblob(16)))),

    -- References
    workflow_instance_id TEXT NOT NULL,
    step_id TEXT,

    -- Audit details
    action TEXT NOT NULL,
    action_category TEXT NOT NULL CHECK (action_category IN (
        'lifecycle', 'data', 'config', 'security', 'compliance'
    )),

    -- Actor
    performed_by TEXT NOT NULL,
    performed_by_type TEXT NOT NULL,

    -- Before/After
    before_state TEXT,  -- JSON
    after_state TEXT,   -- JSON

    -- Context
    reason TEXT,
    approval_id TEXT,  -- If action required approval

    -- Compliance
    compliance_relevant INTEGER DEFAULT 0,
    compliance_frameworks TEXT,  -- JSON array of applicable frameworks

    -- Timestamp
    timestamp TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Indexes for workflow security
CREATE INDEX IF NOT EXISTS idx_workflow_sec_events_workflow ON workflow_security_events(workflow_instance_id);
CREATE INDEX IF NOT EXISTS idx_workflow_sec_events_type ON workflow_security_events(security_event_type);
CREATE INDEX IF NOT EXISTS idx_workflow_sec_events_risk ON workflow_security_events(risk_level);
CREATE INDEX IF NOT EXISTS idx_workflow_sec_events_timestamp ON workflow_security_events(timestamp);
CREATE INDEX IF NOT EXISTS idx_workflow_sec_events_audit ON workflow_security_events(requires_audit) WHERE requires_audit = 1;
CREATE INDEX IF NOT EXISTS idx_workflow_access_workflow ON workflow_access_control(workflow_id);
CREATE INDEX IF NOT EXISTS idx_workflow_access_principal ON workflow_access_control(principal_type, principal_id);
CREATE INDEX IF NOT EXISTS idx_workflow_audit_workflow ON workflow_audit_trail(workflow_instance_id);
CREATE INDEX IF NOT EXISTS idx_workflow_audit_action ON workflow_audit_trail(action_category);
CREATE INDEX IF NOT EXISTS idx_workflow_audit_compliance ON workflow_audit_trail(compliance_relevant) WHERE compliance_relevant = 1;

--------------------------------------------------------------------------------
-- CROSS-DATABASE VIEWS
--------------------------------------------------------------------------------

-- Workflow security overview
CREATE VIEW IF NOT EXISTS v_workflow_security_overview AS
SELECT
    wse.workflow_instance_id,
    wse.security_event_type,
    wse.risk_level,
    wse.actor,
    wse.actor_type,
    wse.action_performed,
    wse.target_resource,
    wse.timestamp,
    (SELECT COUNT(*) FROM workflow_security_events wse2
     WHERE wse2.workflow_instance_id = wse.workflow_instance_id
     AND wse2.risk_level IN ('high', 'critical')) AS high_risk_events
FROM workflow_security_events wse
WHERE wse.timestamp >= datetime('now', '-24 hours')
ORDER BY wse.timestamp DESC;

-- High-risk workflow activities
CREATE VIEW IF NOT EXISTS v_high_risk_workflow_activities AS
SELECT
    wse.workflow_instance_id,
    wse.security_event_type,
    wse.actor,
    wse.action_performed,
    wse.risk_level,
    wse.details,
    wse.timestamp,
    (SELECT wat.reason FROM workflow_audit_trail wat
     WHERE wat.workflow_instance_id = wse.workflow_instance_id
     ORDER BY wat.timestamp DESC LIMIT 1) AS latest_audit_reason
FROM workflow_security_events wse
WHERE wse.risk_level IN ('high', 'critical')
ORDER BY wse.timestamp DESC;

-- Workflow access audit
CREATE VIEW IF NOT EXISTS v_workflow_access_audit AS
SELECT
    wac.workflow_id,
    wac.principal_type,
    wac.principal_id,
    wac.can_execute,
    wac.can_approve,
    wac.can_cancel,
    wac.can_modify,
    wac.require_approval_above_tier,
    wac.is_active,
    (SELECT COUNT(*) FROM workflow_security_events wse
     WHERE wse.actor = wac.principal_id
     AND wse.timestamp >= datetime('now', '-7 days')) AS recent_activities
FROM workflow_access_control wac
WHERE wac.is_active = 1
ORDER BY wac.workflow_id, wac.principal_type;

-- Workflow compliance status
CREATE VIEW IF NOT EXISTS v_workflow_compliance_status AS
SELECT
    wat.workflow_instance_id,
    wat.action_category,
    COUNT(*) AS audit_entries,
    SUM(wat.compliance_relevant) AS compliance_relevant_count,
    (SELECT GROUP_CONCAT(DISTINCT jf.value) FROM workflow_audit_trail wat2,
     json_each(wat2.compliance_frameworks) jf
     WHERE wat2.workflow_instance_id = wat.workflow_instance_id
     AND wat2.compliance_relevant = 1) AS applicable_frameworks
FROM workflow_audit_trail wat
WHERE wat.timestamp >= datetime('now', '-30 days')
GROUP BY wat.workflow_instance_id, wat.action_category;

-- Pending security audits
CREATE VIEW IF NOT EXISTS v_pending_workflow_audits AS
SELECT
    wse.event_id,
    wse.workflow_instance_id,
    wse.security_event_type,
    wse.risk_level,
    wse.actor,
    wse.action_performed,
    wse.timestamp,
    (julianday('now') - julianday(wse.timestamp)) * 24 AS hours_pending
FROM workflow_security_events wse
WHERE wse.requires_audit = 1
AND NOT EXISTS (
    SELECT 1 FROM workflow_audit_trail wat
    WHERE wat.workflow_instance_id = wse.workflow_instance_id
    AND wat.timestamp > wse.timestamp
)
ORDER BY wse.timestamp;

--------------------------------------------------------------------------------
-- WORKFLOW INTEGRATION TRIGGERS
--------------------------------------------------------------------------------

-- Create security event on workflow start
CREATE TRIGGER IF NOT EXISTS trg_workflow_start_security_event
AFTER INSERT ON workflow_security_events
WHEN NEW.security_event_type = 'workflow_started'
BEGIN
    -- Check if this requires elevated privileges
    UPDATE workflow_security_events
    SET requires_audit = 1,
        risk_level = 'high'
    WHERE event_id = NEW.event_id
    AND EXISTS (
        SELECT 1 FROM workflow_access_control wac
        WHERE wac.principal_id = NEW.actor
        AND wac.require_approval_above_tier IS NOT NULL
    );
END;

-- Auto-create audit trail for high-risk events
CREATE TRIGGER IF NOT EXISTS trg_audit_high_risk_workflow_events
AFTER INSERT ON workflow_security_events
WHEN NEW.risk_level IN ('high', 'critical')
BEGIN
    INSERT INTO workflow_audit_trail (
        workflow_instance_id, step_id, action, action_category,
        performed_by, performed_by_type, after_state, compliance_relevant
    )
    VALUES (
        NEW.workflow_instance_id,
        NEW.step_execution_id,
        NEW.action_performed,
        'security',
        NEW.actor,
        NEW.actor_type,
        NEW.details,
        1
    );

    -- Also create a security alert for critical events
    INSERT INTO security_alerts (
        alert_id, alert_type, severity, title, description,
        source_service, status
    )
    SELECT
        lower(hex(randomblob(16))),
        'anomalous_access',
        CASE NEW.risk_level WHEN 'critical' THEN 'critical' ELSE 'high' END,
        'High-risk workflow activity: ' || NEW.security_event_type,
        'Workflow ' || NEW.workflow_instance_id || ' performed ' || NEW.action_performed,
        'workflow_engine',
        'open'
    WHERE NEW.risk_level = 'critical';
END;

-- Track workflow access attempts
CREATE TRIGGER IF NOT EXISTS trg_track_workflow_access
AFTER INSERT ON workflow_security_events
WHEN NEW.security_event_type IN ('privilege_used', 'resource_accessed')
BEGIN
    INSERT INTO access_audit (
        audit_id, principal_type, principal_id,
        resource_type, resource_id, action, permission_level,
        decision, decision_reason, request_context
    )
    VALUES (
        lower(hex(randomblob(16))),
        NEW.actor_type,
        NEW.actor,
        'workflow',
        NEW.workflow_instance_id,
        NEW.action_performed,
        CASE NEW.risk_level
            WHEN 'critical' THEN 'admin'
            WHEN 'high' THEN 'write'
            ELSE 'read'
        END,
        'allow',
        'Workflow execution',
        NEW.details
    );
END;
