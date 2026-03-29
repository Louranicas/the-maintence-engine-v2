-- Migration: 006_tensor_memory.sql
-- Purpose: 12D tensor storage and retrieval
-- Database: tensor_memory.db

--------------------------------------------------------------------------------
-- TENSOR STORAGE TABLE
--------------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS tensor_snapshots (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    snapshot_id TEXT NOT NULL UNIQUE,
    service_id TEXT NOT NULL,
    timestamp TEXT NOT NULL DEFAULT (datetime('now')),

    -- 12D Tensor Dimensions
    d0_service_id INTEGER NOT NULL,
    d1_port INTEGER NOT NULL,
    d2_tier INTEGER NOT NULL CHECK (d2_tier BETWEEN 1 AND 5),
    d3_deps INTEGER NOT NULL DEFAULT 0,
    d4_agents INTEGER NOT NULL DEFAULT 0,
    d5_protocol INTEGER NOT NULL CHECK (d5_protocol BETWEEN 0 AND 3),
    d6_health REAL NOT NULL CHECK (d6_health BETWEEN 0.0 AND 1.0),
    d7_uptime INTEGER NOT NULL DEFAULT 0,
    d8_synergy REAL NOT NULL CHECK (d8_synergy BETWEEN 0.0 AND 1.0),
    d9_latency REAL NOT NULL DEFAULT 0.0,
    d10_error_rate REAL NOT NULL CHECK (d10_error_rate BETWEEN 0.0 AND 1.0),
    d11_temporal REAL NOT NULL DEFAULT 0.0
    -- NOTE: magnitude computed in Rust (SQLite lacks sqrt() without math extension)
);

--------------------------------------------------------------------------------
-- TENSOR OPERATIONS TABLE
--------------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS tensor_operations (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    operation_id TEXT NOT NULL UNIQUE,
    operation_type TEXT NOT NULL CHECK (operation_type IN (
        'encode', 'decode', 'normalize', 'transform',
        'similarity', 'cluster', 'reduce', 'expand'
    )),
    input_snapshot_id TEXT NOT NULL,
    output_snapshot_id TEXT,
    parameters TEXT,  -- JSON
    result TEXT,      -- JSON
    duration_ms INTEGER,
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'running', 'completed', 'failed')),
    error_message TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    completed_at TEXT,

    FOREIGN KEY (input_snapshot_id) REFERENCES tensor_snapshots(snapshot_id),
    FOREIGN KEY (output_snapshot_id) REFERENCES tensor_snapshots(snapshot_id)
);

--------------------------------------------------------------------------------
-- TENSOR SIMILARITY CACHE
--------------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS tensor_similarities (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    snapshot_a TEXT NOT NULL,
    snapshot_b TEXT NOT NULL,
    cosine_similarity REAL NOT NULL CHECK (cosine_similarity BETWEEN -1.0 AND 1.0),
    euclidean_distance REAL NOT NULL,
    manhattan_distance REAL NOT NULL,
    computed_at TEXT NOT NULL DEFAULT (datetime('now')),

    UNIQUE(snapshot_a, snapshot_b),
    FOREIGN KEY (snapshot_a) REFERENCES tensor_snapshots(snapshot_id),
    FOREIGN KEY (snapshot_b) REFERENCES tensor_snapshots(snapshot_id)
);

--------------------------------------------------------------------------------
-- TENSOR CLUSTERS
--------------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS tensor_clusters (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    cluster_id TEXT NOT NULL UNIQUE,
    cluster_name TEXT NOT NULL,
    centroid_d0 REAL NOT NULL,
    centroid_d1 REAL NOT NULL,
    centroid_d2 REAL NOT NULL,
    centroid_d3 REAL NOT NULL,
    centroid_d4 REAL NOT NULL,
    centroid_d5 REAL NOT NULL,
    centroid_d6 REAL NOT NULL,
    centroid_d7 REAL NOT NULL,
    centroid_d8 REAL NOT NULL,
    centroid_d9 REAL NOT NULL,
    centroid_d10 REAL NOT NULL,
    centroid_d11 REAL NOT NULL,
    member_count INTEGER NOT NULL DEFAULT 0,
    intra_cluster_distance REAL NOT NULL DEFAULT 0.0,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS tensor_cluster_members (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    cluster_id TEXT NOT NULL,
    snapshot_id TEXT NOT NULL,
    distance_to_centroid REAL NOT NULL,
    assigned_at TEXT NOT NULL DEFAULT (datetime('now')),

    UNIQUE(cluster_id, snapshot_id),
    FOREIGN KEY (cluster_id) REFERENCES tensor_clusters(cluster_id),
    FOREIGN KEY (snapshot_id) REFERENCES tensor_snapshots(snapshot_id)
);

--------------------------------------------------------------------------------
-- TEMPORAL TENSOR SEQUENCES
--------------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS tensor_sequences (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    sequence_id TEXT NOT NULL UNIQUE,
    service_id TEXT NOT NULL,
    start_time TEXT NOT NULL,
    end_time TEXT,
    snapshot_count INTEGER NOT NULL DEFAULT 0,
    trend_d6 REAL,  -- health trend
    trend_d8 REAL,  -- synergy trend
    trend_d9 REAL,  -- latency trend
    trend_d10 REAL, -- error_rate trend
    is_active INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS tensor_sequence_points (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    sequence_id TEXT NOT NULL,
    snapshot_id TEXT NOT NULL,
    sequence_index INTEGER NOT NULL,
    delta_from_previous TEXT,  -- JSON of dimension deltas

    UNIQUE(sequence_id, sequence_index),
    FOREIGN KEY (sequence_id) REFERENCES tensor_sequences(sequence_id),
    FOREIGN KEY (snapshot_id) REFERENCES tensor_snapshots(snapshot_id)
);

--------------------------------------------------------------------------------
-- INDEXES
--------------------------------------------------------------------------------
CREATE INDEX IF NOT EXISTS idx_tensor_snapshots_service ON tensor_snapshots(service_id);
CREATE INDEX IF NOT EXISTS idx_tensor_snapshots_timestamp ON tensor_snapshots(timestamp);
CREATE INDEX IF NOT EXISTS idx_tensor_snapshots_health ON tensor_snapshots(d6_health);
CREATE INDEX IF NOT EXISTS idx_tensor_snapshots_synergy ON tensor_snapshots(d8_synergy);
CREATE INDEX IF NOT EXISTS idx_tensor_operations_status ON tensor_operations(status);
CREATE INDEX IF NOT EXISTS idx_tensor_operations_type ON tensor_operations(operation_type);
CREATE INDEX IF NOT EXISTS idx_tensor_similarities_a ON tensor_similarities(snapshot_a);
CREATE INDEX IF NOT EXISTS idx_tensor_similarities_b ON tensor_similarities(snapshot_b);
CREATE INDEX IF NOT EXISTS idx_tensor_cluster_members_cluster ON tensor_cluster_members(cluster_id);
CREATE INDEX IF NOT EXISTS idx_tensor_sequences_service ON tensor_sequences(service_id);
CREATE INDEX IF NOT EXISTS idx_tensor_sequences_active ON tensor_sequences(is_active);

--------------------------------------------------------------------------------
-- VIEWS
--------------------------------------------------------------------------------
CREATE VIEW IF NOT EXISTS v_tensor_health_summary AS
SELECT
    service_id,
    COUNT(*) AS snapshot_count,
    AVG(d6_health) AS avg_health,
    MIN(d6_health) AS min_health,
    MAX(d6_health) AS max_health,
    AVG(d8_synergy) AS avg_synergy,
    AVG(d9_latency) AS avg_latency,
    AVG(d10_error_rate) AS avg_error_rate
FROM tensor_snapshots
WHERE timestamp >= datetime('now', '-24 hours')
GROUP BY service_id;

CREATE VIEW IF NOT EXISTS v_tensor_latest AS
SELECT ts.*
FROM tensor_snapshots ts
INNER JOIN (
    SELECT service_id, MAX(timestamp) AS max_ts
    FROM tensor_snapshots
    GROUP BY service_id
) latest ON ts.service_id = latest.service_id AND ts.timestamp = latest.max_ts;

CREATE VIEW IF NOT EXISTS v_tensor_anomalies AS
SELECT
    snapshot_id,
    service_id,
    timestamp,
    d6_health,
    d8_synergy,
    d9_latency,
    d10_error_rate,
    CASE
        WHEN d6_health < 0.5 THEN 'LOW_HEALTH'
        WHEN d8_synergy < 0.3 THEN 'LOW_SYNERGY'
        WHEN d9_latency > 1000 THEN 'HIGH_LATENCY'
        WHEN d10_error_rate > 0.1 THEN 'HIGH_ERROR_RATE'
    END AS anomaly_type
FROM tensor_snapshots
WHERE d6_health < 0.5 OR d8_synergy < 0.3 OR d9_latency > 1000 OR d10_error_rate > 0.1;

--------------------------------------------------------------------------------
-- TRIGGERS
--------------------------------------------------------------------------------
CREATE TRIGGER IF NOT EXISTS trg_tensor_snapshot_sequence
AFTER INSERT ON tensor_snapshots
BEGIN
    UPDATE tensor_sequences
    SET snapshot_count = snapshot_count + 1,
        updated_at = datetime('now')
    WHERE service_id = NEW.service_id AND is_active = 1;
END;

CREATE TRIGGER IF NOT EXISTS trg_tensor_cluster_member_count
AFTER INSERT ON tensor_cluster_members
BEGIN
    UPDATE tensor_clusters
    SET member_count = member_count + 1,
        updated_at = datetime('now')
    WHERE cluster_id = NEW.cluster_id;
END;

CREATE TRIGGER IF NOT EXISTS trg_tensor_cluster_member_remove
AFTER DELETE ON tensor_cluster_members
BEGIN
    UPDATE tensor_clusters
    SET member_count = member_count - 1,
        updated_at = datetime('now')
    WHERE cluster_id = OLD.cluster_id;
END;

--------------------------------------------------------------------------------
-- WORKFLOW INTEGRATION TABLES
--------------------------------------------------------------------------------

-- Workflow Tensor Snapshots (12D tensor state at workflow checkpoints)
CREATE TABLE IF NOT EXISTS workflow_tensor_snapshots (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    snapshot_id TEXT NOT NULL UNIQUE DEFAULT (lower(hex(randomblob(16)))),

    -- Workflow reference
    workflow_instance_id TEXT NOT NULL,  -- References workflow_tracking.workflow_instances
    step_id TEXT,  -- References workflow_tracking.workflow_steps
    checkpoint_type TEXT NOT NULL CHECK (checkpoint_type IN (
        'start', 'step_complete', 'checkpoint', 'error', 'end'
    )),

    -- Service context
    service_id TEXT NOT NULL,
    tensor_snapshot_id TEXT NOT NULL,  -- References tensor_snapshots

    -- Metadata
    created_at TEXT NOT NULL DEFAULT (datetime('now')),

    FOREIGN KEY (tensor_snapshot_id) REFERENCES tensor_snapshots(snapshot_id)
);

-- Workflow Tensor Deltas (changes during workflow execution)
CREATE TABLE IF NOT EXISTS workflow_tensor_deltas (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    delta_id TEXT NOT NULL UNIQUE DEFAULT (lower(hex(randomblob(16)))),

    -- Workflow reference
    workflow_instance_id TEXT NOT NULL,

    -- Snapshot references
    before_snapshot_id TEXT NOT NULL,
    after_snapshot_id TEXT NOT NULL,

    -- Delta values (12D)
    delta_d0 REAL DEFAULT 0,
    delta_d1 REAL DEFAULT 0,
    delta_d2 REAL DEFAULT 0,
    delta_d3 REAL DEFAULT 0,
    delta_d4 REAL DEFAULT 0,
    delta_d5 REAL DEFAULT 0,
    delta_d6 REAL DEFAULT 0,  -- health change
    delta_d7 REAL DEFAULT 0,  -- uptime change
    delta_d8 REAL DEFAULT 0,  -- synergy change
    delta_d9 REAL DEFAULT 0,  -- latency change
    delta_d10 REAL DEFAULT 0, -- error_rate change
    delta_d11 REAL DEFAULT 0, -- temporal change
    -- NOTE: delta_magnitude computed in Rust (SQLite lacks sqrt())

    -- Impact assessment
    impact_category TEXT CHECK (impact_category IN ('positive', 'negative', 'neutral', 'mixed')),

    -- Timing
    measured_at TEXT NOT NULL DEFAULT (datetime('now')),

    FOREIGN KEY (before_snapshot_id) REFERENCES tensor_snapshots(snapshot_id),
    FOREIGN KEY (after_snapshot_id) REFERENCES tensor_snapshots(snapshot_id)
);

-- Indexes for workflow integration
CREATE INDEX IF NOT EXISTS idx_workflow_tensor_workflow ON workflow_tensor_snapshots(workflow_instance_id);
CREATE INDEX IF NOT EXISTS idx_workflow_tensor_service ON workflow_tensor_snapshots(service_id);
CREATE INDEX IF NOT EXISTS idx_workflow_tensor_checkpoint ON workflow_tensor_snapshots(checkpoint_type);
CREATE INDEX IF NOT EXISTS idx_workflow_deltas_workflow ON workflow_tensor_deltas(workflow_instance_id);
CREATE INDEX IF NOT EXISTS idx_workflow_deltas_magnitude ON workflow_tensor_deltas(delta_magnitude DESC);

--------------------------------------------------------------------------------
-- CROSS-DATABASE VIEWS
--------------------------------------------------------------------------------

-- Workflow tensor impact summary
CREATE VIEW IF NOT EXISTS v_workflow_tensor_impact AS
SELECT
    wtd.workflow_instance_id,
    COUNT(*) AS delta_count,
    AVG(wtd.delta_magnitude) AS avg_delta_magnitude,
    MAX(wtd.delta_magnitude) AS max_delta_magnitude,
    AVG(wtd.delta_d6) AS avg_health_change,
    AVG(wtd.delta_d8) AS avg_synergy_change,
    AVG(wtd.delta_d9) AS avg_latency_change,
    AVG(wtd.delta_d10) AS avg_error_rate_change,
    SUM(CASE WHEN wtd.impact_category = 'positive' THEN 1 ELSE 0 END) AS positive_impacts,
    SUM(CASE WHEN wtd.impact_category = 'negative' THEN 1 ELSE 0 END) AS negative_impacts
FROM workflow_tensor_deltas wtd
GROUP BY wtd.workflow_instance_id;

-- Tensor state at workflow checkpoints
CREATE VIEW IF NOT EXISTS v_workflow_checkpoint_tensors AS
SELECT
    wts.workflow_instance_id,
    wts.checkpoint_type,
    wts.service_id,
    ts.d6_health,
    ts.d8_synergy,
    ts.d9_latency,
    ts.d10_error_rate,
    ts.magnitude,
    wts.created_at
FROM workflow_tensor_snapshots wts
JOIN tensor_snapshots ts ON wts.tensor_snapshot_id = ts.snapshot_id
ORDER BY wts.workflow_instance_id, wts.created_at;

-- Anomalous workflow tensor changes
CREATE VIEW IF NOT EXISTS v_workflow_tensor_anomalies AS
SELECT
    wtd.workflow_instance_id,
    wtd.delta_magnitude,
    wtd.delta_d6 AS health_change,
    wtd.delta_d8 AS synergy_change,
    wtd.delta_d9 AS latency_change,
    wtd.delta_d10 AS error_change,
    wtd.impact_category,
    wtd.measured_at,
    CASE
        WHEN wtd.delta_d6 < -0.2 THEN 'HEALTH_DROP'
        WHEN wtd.delta_d8 < -0.2 THEN 'SYNERGY_DROP'
        WHEN wtd.delta_d9 > 500 THEN 'LATENCY_SPIKE'
        WHEN wtd.delta_d10 > 0.1 THEN 'ERROR_SPIKE'
        ELSE 'OTHER'
    END AS anomaly_type
FROM workflow_tensor_deltas wtd
WHERE wtd.delta_magnitude > 1.0  -- Significant change threshold
OR wtd.delta_d6 < -0.2 OR wtd.delta_d8 < -0.2
OR wtd.delta_d9 > 500 OR wtd.delta_d10 > 0.1;

--------------------------------------------------------------------------------
-- WORKFLOW INTEGRATION TRIGGERS
--------------------------------------------------------------------------------

-- Calculate tensor delta after workflow snapshot
CREATE TRIGGER IF NOT EXISTS trg_calculate_workflow_tensor_delta
AFTER INSERT ON workflow_tensor_snapshots
WHEN NEW.checkpoint_type IN ('step_complete', 'checkpoint', 'end')
BEGIN
    INSERT INTO workflow_tensor_deltas (
        workflow_instance_id, before_snapshot_id, after_snapshot_id,
        delta_d0, delta_d1, delta_d2, delta_d3, delta_d4, delta_d5,
        delta_d6, delta_d7, delta_d8, delta_d9, delta_d10, delta_d11,
        impact_category
    )
    SELECT
        NEW.workflow_instance_id,
        prev.tensor_snapshot_id,
        NEW.tensor_snapshot_id,
        curr.d0_service_id - prev_ts.d0_service_id,
        curr.d1_port - prev_ts.d1_port,
        curr.d2_tier - prev_ts.d2_tier,
        curr.d3_deps - prev_ts.d3_deps,
        curr.d4_agents - prev_ts.d4_agents,
        curr.d5_protocol - prev_ts.d5_protocol,
        curr.d6_health - prev_ts.d6_health,
        curr.d7_uptime - prev_ts.d7_uptime,
        curr.d8_synergy - prev_ts.d8_synergy,
        curr.d9_latency - prev_ts.d9_latency,
        curr.d10_error_rate - prev_ts.d10_error_rate,
        curr.d11_temporal - prev_ts.d11_temporal,
        CASE
            WHEN curr.d6_health > prev_ts.d6_health AND curr.d10_error_rate <= prev_ts.d10_error_rate THEN 'positive'
            WHEN curr.d6_health < prev_ts.d6_health OR curr.d10_error_rate > prev_ts.d10_error_rate THEN 'negative'
            ELSE 'neutral'
        END
    FROM workflow_tensor_snapshots prev
    JOIN tensor_snapshots prev_ts ON prev.tensor_snapshot_id = prev_ts.snapshot_id
    JOIN tensor_snapshots curr ON NEW.tensor_snapshot_id = curr.snapshot_id
    WHERE prev.workflow_instance_id = NEW.workflow_instance_id
    AND prev.service_id = NEW.service_id
    AND prev.id = (
        SELECT MAX(id) FROM workflow_tensor_snapshots
        WHERE workflow_instance_id = NEW.workflow_instance_id
        AND service_id = NEW.service_id
        AND id < NEW.id
    );
END;
