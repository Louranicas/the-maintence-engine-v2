-- ============================================================================
-- Migration: 003_hebbian_pulse.sql
-- SYNTHEX Database Migration - Hebbian Pulse Learning Module
-- Version: 1.0.0
-- Pattern: Neural pathway strengthening via Hebbian learning principles
-- Implements: LTP, LTD, STDP (Spike-Timing Dependent Plasticity)
-- ============================================================================

INSERT INTO schema_versions (version, migration_name, checksum)
VALUES ('003', 'hebbian_pulse', 'sha256:003_hebbian_pulse');

-- ============================================================================
-- NEURAL PATHWAYS TABLE
-- Core structure for tracking learned connections between entities
-- ============================================================================
CREATE TABLE IF NOT EXISTS neural_pathways (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    pathway_id TEXT NOT NULL UNIQUE DEFAULT (lower(hex(randomblob(16)))),

    -- Pathway Identification
    pathway_type TEXT NOT NULL
        CHECK(pathway_type IN (
            'associative', 'causal', 'temporal', 'semantic', 'procedural',
            'contextual', 'hierarchical', 'lateral', 'feedback', 'custom'
        )),
    pathway_name TEXT,

    -- Connection Endpoints
    source_id TEXT NOT NULL,
    source_type TEXT NOT NULL,
    target_id TEXT NOT NULL,
    target_type TEXT NOT NULL,

    -- Connection Strength (Core Hebbian Metric)
    strength REAL NOT NULL DEFAULT 0.5 CHECK(strength >= 0.0 AND strength <= 1.0),
    initial_strength REAL DEFAULT 0.5,
    peak_strength REAL DEFAULT 0.5,
    min_strength REAL DEFAULT 0.5,

    -- Long-Term Potentiation (LTP)
    ltp REAL DEFAULT 0.0 CHECK(ltp >= 0.0),
    ltp_threshold REAL DEFAULT 0.7,
    ltp_rate REAL DEFAULT 0.1,
    ltp_decay_rate REAL DEFAULT 0.01,
    last_ltp_event DATETIME,
    ltp_event_count INTEGER DEFAULT 0,

    -- Long-Term Depression (LTD)
    ltd REAL DEFAULT 0.0 CHECK(ltd >= 0.0),
    ltd_threshold REAL DEFAULT 0.3,
    ltd_rate REAL DEFAULT 0.05,
    ltd_decay_rate REAL DEFAULT 0.005,
    last_ltd_event DATETIME,
    ltd_event_count INTEGER DEFAULT 0,

    -- Spike-Timing Dependent Plasticity (STDP)
    stdp_delta REAL DEFAULT 0.0,
    stdp_window_ms INTEGER DEFAULT 100,
    stdp_positive_rate REAL DEFAULT 0.1,
    stdp_negative_rate REAL DEFAULT 0.05,
    last_source_activation DATETIME,
    last_target_activation DATETIME,

    -- Activation Statistics
    activation_count INTEGER DEFAULT 0,
    co_activation_count INTEGER DEFAULT 0,
    last_activation DATETIME,
    avg_activation_interval_ms REAL,

    -- Pathway State
    state TEXT DEFAULT 'active'
        CHECK(state IN ('active', 'dormant', 'potentiated', 'depressed', 'pruned', 'consolidating')),
    consolidation_level INTEGER DEFAULT 0 CHECK(consolidation_level >= 0 AND consolidation_level <= 3),

    -- Calculated Fields
    net_plasticity REAL GENERATED ALWAYS AS (ltp - ltd) STORED,
    strength_trend TEXT GENERATED ALWAYS AS (
        CASE
            WHEN ltp - ltd > 0.1 THEN 'strengthening'
            WHEN ltp - ltd < -0.1 THEN 'weakening'
            ELSE 'stable'
        END
    ) STORED,
    is_strong INTEGER GENERATED ALWAYS AS (CASE WHEN strength > 0.7 THEN 1 ELSE 0 END) STORED,
    is_weak INTEGER GENERATED ALWAYS AS (CASE WHEN strength < 0.3 THEN 1 ELSE 0 END) STORED,

    -- Metadata
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    metadata TEXT -- JSON blob
);

-- ============================================================================
-- HEBBIAN PULSES TABLE
-- Records each Hebbian pulse event that triggers pathway updates
-- ============================================================================
CREATE TABLE IF NOT EXISTS hebbian_pulses (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    pulse_id TEXT NOT NULL UNIQUE DEFAULT (lower(hex(randomblob(16)))),

    -- Pulse Identification
    pulse_number INTEGER NOT NULL,
    pulse_name TEXT,

    -- Trigger Information
    trigger_type TEXT NOT NULL
        CHECK(trigger_type IN (
            'scheduled', 'event_driven', 'threshold', 'manual',
            'consolidation', 'learning', 'decay', 'prune', 'custom'
        )),
    trigger_event TEXT,
    trigger_source TEXT,

    -- Pulse Scope
    scope TEXT DEFAULT 'global'
        CHECK(scope IN ('global', 'local', 'pathway', 'cluster', 'layer')),
    scope_filter TEXT, -- JSON filter criteria

    -- Pathway Updates
    pathways_evaluated INTEGER DEFAULT 0,
    pathways_reinforced INTEGER DEFAULT 0,
    pathways_weakened INTEGER DEFAULT 0,
    pathways_pruned INTEGER DEFAULT 0,
    pathways_created INTEGER DEFAULT 0,

    -- Strength Changes
    total_ltp_applied REAL DEFAULT 0.0,
    total_ltd_applied REAL DEFAULT 0.0,
    avg_strength_change REAL DEFAULT 0.0,
    max_strength_increase REAL DEFAULT 0.0,
    max_strength_decrease REAL DEFAULT 0.0,

    -- STDP Statistics
    stdp_positive_updates INTEGER DEFAULT 0,
    stdp_negative_updates INTEGER DEFAULT 0,
    avg_stdp_delta REAL DEFAULT 0.0,

    -- Timing
    timestamp DATETIME DEFAULT CURRENT_TIMESTAMP,
    duration_ms INTEGER,
    start_time DATETIME,
    end_time DATETIME,

    -- Pulse Parameters
    learning_rate REAL DEFAULT 0.1,
    decay_factor REAL DEFAULT 0.99,
    prune_threshold REAL DEFAULT 0.1,
    reinforce_threshold REAL DEFAULT 0.8,

    -- Outcomes (Generated)
    net_reinforcement INTEGER GENERATED ALWAYS AS (pathways_reinforced - pathways_weakened) STORED,
    reinforcement_ratio REAL GENERATED ALWAYS AS (
        CASE WHEN pathways_weakened > 0
        THEN CAST(pathways_reinforced AS REAL) / pathways_weakened
        ELSE pathways_reinforced END
    ) STORED,
    effectiveness_score REAL GENERATED ALWAYS AS (
        CASE WHEN pathways_evaluated > 0
        THEN (CAST(pathways_reinforced + pathways_weakened AS REAL) / pathways_evaluated) * 100
        ELSE 0.0 END
    ) STORED,

    -- Metadata
    metadata TEXT -- JSON blob
);

-- ============================================================================
-- PATTERN REINFORCEMENT TABLE
-- Tracks reinforcement of recognized patterns
-- ============================================================================
CREATE TABLE IF NOT EXISTS pattern_reinforcement (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    pattern_id TEXT NOT NULL,

    -- Pattern Classification
    pattern_type TEXT NOT NULL
        CHECK(pattern_type IN (
            'behavioral', 'temporal', 'sequential', 'structural',
            'anomaly', 'correlation', 'causation', 'cluster', 'custom'
        )),
    pattern_name TEXT,
    pattern_signature TEXT, -- Hash or signature of the pattern

    -- Reinforcement Metrics
    reinforcement_strength REAL DEFAULT 0.0 CHECK(reinforcement_strength >= -1.0 AND reinforcement_strength <= 1.0),
    reinforcement_count INTEGER DEFAULT 0,
    last_reinforcement DATETIME,

    -- Confidence Scoring
    confidence_score REAL DEFAULT 0.5 CHECK(confidence_score >= 0.0 AND confidence_score <= 1.0),
    confidence_samples INTEGER DEFAULT 0,
    confidence_variance REAL DEFAULT 0.0,

    -- Pattern Statistics
    occurrence_count INTEGER DEFAULT 0,
    first_occurrence DATETIME,
    last_occurrence DATETIME,
    avg_occurrence_interval_ms REAL,

    -- Associated Pathways
    primary_pathway_id INTEGER,
    pathway_count INTEGER DEFAULT 0,

    -- Pattern State
    state TEXT DEFAULT 'emerging'
        CHECK(state IN ('emerging', 'established', 'strong', 'decaying', 'dormant', 'extinct')),
    maturity_level INTEGER DEFAULT 0 CHECK(maturity_level >= 0 AND maturity_level <= 5),

    -- Calculated Fields
    strength_grade TEXT GENERATED ALWAYS AS (
        CASE
            WHEN reinforcement_strength > 0.8 THEN 'A'
            WHEN reinforcement_strength > 0.6 THEN 'B'
            WHEN reinforcement_strength > 0.4 THEN 'C'
            WHEN reinforcement_strength > 0.2 THEN 'D'
            ELSE 'F'
        END
    ) STORED,
    is_reliable INTEGER GENERATED ALWAYS AS (
        CASE WHEN confidence_score > 0.7 AND confidence_samples > 10 THEN 1 ELSE 0 END
    ) STORED,

    -- Metadata
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    metadata TEXT, -- JSON blob with pattern details

    FOREIGN KEY (primary_pathway_id) REFERENCES neural_pathways(id)
);

-- ============================================================================
-- MEMORY CONSOLIDATION TABLE
-- Tracks memory consolidation from working to long-term storage
-- ============================================================================
CREATE TABLE IF NOT EXISTS memory_consolidation (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    consolidation_id TEXT NOT NULL UNIQUE DEFAULT (lower(hex(randomblob(16)))),

    -- Entity Being Consolidated
    entity_id TEXT NOT NULL,
    entity_type TEXT NOT NULL
        CHECK(entity_type IN (
            'pathway', 'pattern', 'episode', 'fact', 'skill',
            'procedure', 'context', 'association', 'custom'
        )),

    -- Memory Layers
    from_layer TEXT NOT NULL
        CHECK(from_layer IN ('sensory', 'working', 'short_term', 'intermediate', 'long_term', 'permanent')),
    to_layer TEXT NOT NULL
        CHECK(to_layer IN ('sensory', 'working', 'short_term', 'intermediate', 'long_term', 'permanent')),

    -- Consolidation Type
    consolidation_type TEXT NOT NULL DEFAULT 'standard'
        CHECK(consolidation_type IN (
            'standard', 'rapid', 'sleep', 'rehearsal', 'emotional',
            'semantic', 'episodic', 'procedural', 'forced'
        )),

    -- Consolidation State
    status TEXT DEFAULT 'pending'
        CHECK(status IN ('pending', 'in_progress', 'completed', 'failed', 'reverted')),
    progress_percent REAL DEFAULT 0.0 CHECK(progress_percent >= 0.0 AND progress_percent <= 100.0),

    -- Timing
    initiated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    started_at DATETIME,
    completed_at DATETIME,
    duration_ms INTEGER,

    -- Consolidation Metrics
    rehearsal_count INTEGER DEFAULT 0,
    strength_before REAL,
    strength_after REAL,
    stability_score REAL DEFAULT 0.0,

    -- Resource Usage
    processing_cycles INTEGER DEFAULT 0,
    memory_allocated_kb REAL DEFAULT 0.0,

    -- Triggering Conditions
    trigger_reason TEXT,
    trigger_threshold REAL,
    trigger_pulse_id TEXT,

    -- Calculated Fields
    strength_change REAL GENERATED ALWAYS AS (
        COALESCE(strength_after, 0) - COALESCE(strength_before, 0)
    ) STORED,
    layer_jump INTEGER GENERATED ALWAYS AS (
        CASE to_layer
            WHEN 'permanent' THEN 5
            WHEN 'long_term' THEN 4
            WHEN 'intermediate' THEN 3
            WHEN 'short_term' THEN 2
            WHEN 'working' THEN 1
            ELSE 0
        END -
        CASE from_layer
            WHEN 'permanent' THEN 5
            WHEN 'long_term' THEN 4
            WHEN 'intermediate' THEN 3
            WHEN 'short_term' THEN 2
            WHEN 'working' THEN 1
            ELSE 0
        END
    ) STORED,

    -- Metadata
    metadata TEXT -- JSON blob
);

-- ============================================================================
-- PATHWAY ACTIVATIONS TABLE
-- High-frequency table for tracking individual pathway activations
-- ============================================================================
CREATE TABLE IF NOT EXISTS pathway_activations (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    pathway_id INTEGER NOT NULL,

    -- Activation Details
    activation_strength REAL NOT NULL CHECK(activation_strength >= 0.0 AND activation_strength <= 1.0),
    activation_type TEXT DEFAULT 'direct'
        CHECK(activation_type IN ('direct', 'indirect', 'cascade', 'spontaneous', 'external')),

    -- Timing
    timestamp DATETIME DEFAULT CURRENT_TIMESTAMP,
    duration_ms INTEGER,

    -- Context
    trigger_source TEXT,
    correlation_id TEXT,

    -- STDP Calculation Support
    pre_synaptic_time DATETIME,
    post_synaptic_time DATETIME,
    timing_delta_ms INTEGER,

    FOREIGN KEY (pathway_id) REFERENCES neural_pathways(id) ON DELETE CASCADE
);

-- ============================================================================
-- INDEXES FOR PERFORMANCE
-- ============================================================================

-- Neural Pathways Indexes
CREATE INDEX IF NOT EXISTS idx_pathways_type ON neural_pathways(pathway_type);
CREATE INDEX IF NOT EXISTS idx_pathways_source ON neural_pathways(source_id, source_type);
CREATE INDEX IF NOT EXISTS idx_pathways_target ON neural_pathways(target_id, target_type);
CREATE INDEX IF NOT EXISTS idx_pathways_strength ON neural_pathways(strength DESC);
CREATE INDEX IF NOT EXISTS idx_pathways_strong ON neural_pathways(is_strong) WHERE is_strong = 1;
CREATE INDEX IF NOT EXISTS idx_pathways_weak ON neural_pathways(is_weak) WHERE is_weak = 1;
CREATE INDEX IF NOT EXISTS idx_pathways_state ON neural_pathways(state);
CREATE INDEX IF NOT EXISTS idx_pathways_consolidation ON neural_pathways(consolidation_level);
CREATE INDEX IF NOT EXISTS idx_pathways_activation ON neural_pathways(last_activation DESC);
CREATE INDEX IF NOT EXISTS idx_pathways_plasticity ON neural_pathways(net_plasticity DESC);

-- Hebbian Pulses Indexes
CREATE INDEX IF NOT EXISTS idx_pulses_number ON hebbian_pulses(pulse_number);
CREATE INDEX IF NOT EXISTS idx_pulses_trigger ON hebbian_pulses(trigger_type);
CREATE INDEX IF NOT EXISTS idx_pulses_timestamp ON hebbian_pulses(timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_pulses_scope ON hebbian_pulses(scope);
CREATE INDEX IF NOT EXISTS idx_pulses_effectiveness ON hebbian_pulses(effectiveness_score DESC);

-- Pattern Reinforcement Indexes
CREATE INDEX IF NOT EXISTS idx_patterns_type ON pattern_reinforcement(pattern_type);
CREATE INDEX IF NOT EXISTS idx_patterns_strength ON pattern_reinforcement(reinforcement_strength DESC);
CREATE INDEX IF NOT EXISTS idx_patterns_confidence ON pattern_reinforcement(confidence_score DESC);
CREATE INDEX IF NOT EXISTS idx_patterns_state ON pattern_reinforcement(state);
CREATE INDEX IF NOT EXISTS idx_patterns_reliable ON pattern_reinforcement(is_reliable) WHERE is_reliable = 1;
CREATE INDEX IF NOT EXISTS idx_patterns_signature ON pattern_reinforcement(pattern_signature);

-- Memory Consolidation Indexes
CREATE INDEX IF NOT EXISTS idx_consolidation_entity ON memory_consolidation(entity_id, entity_type);
CREATE INDEX IF NOT EXISTS idx_consolidation_layers ON memory_consolidation(from_layer, to_layer);
CREATE INDEX IF NOT EXISTS idx_consolidation_status ON memory_consolidation(status);
CREATE INDEX IF NOT EXISTS idx_consolidation_pending ON memory_consolidation(status) WHERE status = 'pending';
CREATE INDEX IF NOT EXISTS idx_consolidation_type ON memory_consolidation(consolidation_type);

-- Pathway Activations Indexes
CREATE INDEX IF NOT EXISTS idx_activations_pathway ON pathway_activations(pathway_id);
CREATE INDEX IF NOT EXISTS idx_activations_timestamp ON pathway_activations(timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_activations_correlation ON pathway_activations(correlation_id) WHERE correlation_id IS NOT NULL;

-- ============================================================================
-- TRIGGERS FOR AUTOMATIC UPDATES
-- ============================================================================

-- Update neural_pathways.updated_at on modification
CREATE TRIGGER IF NOT EXISTS trg_pathways_updated_at
AFTER UPDATE ON neural_pathways
BEGIN
    UPDATE neural_pathways SET updated_at = CURRENT_TIMESTAMP WHERE id = NEW.id;
END;

-- Update pattern_reinforcement.updated_at on modification
CREATE TRIGGER IF NOT EXISTS trg_patterns_updated_at
AFTER UPDATE ON pattern_reinforcement
BEGIN
    UPDATE pattern_reinforcement SET updated_at = CURRENT_TIMESTAMP WHERE id = NEW.id;
END;

-- Auto-update pathway activation statistics
CREATE TRIGGER IF NOT EXISTS trg_pathway_activated
AFTER INSERT ON pathway_activations
BEGIN
    UPDATE neural_pathways SET
        activation_count = activation_count + 1,
        last_activation = NEW.timestamp
    WHERE id = NEW.pathway_id;
END;

-- Auto-update peak/min strength
CREATE TRIGGER IF NOT EXISTS trg_pathway_strength_changed
AFTER UPDATE OF strength ON neural_pathways
BEGIN
    UPDATE neural_pathways SET
        peak_strength = MAX(peak_strength, NEW.strength),
        min_strength = MIN(min_strength, NEW.strength)
    WHERE id = NEW.id;
END;

-- Auto-update pattern occurrence on reinforcement
CREATE TRIGGER IF NOT EXISTS trg_pattern_reinforced
AFTER UPDATE OF reinforcement_strength ON pattern_reinforcement
WHEN NEW.reinforcement_strength != OLD.reinforcement_strength
BEGIN
    UPDATE pattern_reinforcement SET
        reinforcement_count = reinforcement_count + 1,
        last_reinforcement = CURRENT_TIMESTAMP,
        occurrence_count = occurrence_count + 1,
        last_occurrence = CURRENT_TIMESTAMP
    WHERE id = NEW.id;
END;

-- ============================================================================
-- WORKFLOW INTEGRATION TABLES
-- ============================================================================

-- Workflow Learning Events (tracks learning signals from workflows)
CREATE TABLE IF NOT EXISTS workflow_learning_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    event_id TEXT NOT NULL UNIQUE DEFAULT (lower(hex(randomblob(16)))),

    -- Workflow reference
    workflow_instance_id TEXT NOT NULL,  -- References workflow_tracking.workflow_instances
    workflow_type TEXT NOT NULL,

    -- Learning signal
    signal_type TEXT NOT NULL
        CHECK(signal_type IN ('success', 'failure', 'timeout', 'partial', 'rollback')),
    signal_strength REAL NOT NULL DEFAULT 1.0 CHECK(signal_strength >= 0.0 AND signal_strength <= 1.0),

    -- Context
    affected_pathways TEXT,  -- JSON array of pathway_ids
    affected_patterns TEXT,  -- JSON array of pattern_ids

    -- Outcome assessment
    learning_applied INTEGER DEFAULT 0,
    ltp_delta REAL DEFAULT 0.0,
    ltd_delta REAL DEFAULT 0.0,

    -- Timing
    timestamp DATETIME DEFAULT CURRENT_TIMESTAMP,
    processed_at DATETIME
);

-- Pathway-Workflow Associations (maps pathways to workflow types)
CREATE TABLE IF NOT EXISTS pathway_workflow_mapping (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    pathway_id INTEGER NOT NULL,
    workflow_type TEXT NOT NULL,

    -- Association strength
    relevance_score REAL DEFAULT 0.5 CHECK(relevance_score >= 0.0 AND relevance_score <= 1.0),
    activation_count INTEGER DEFAULT 0,
    success_count INTEGER DEFAULT 0,

    -- Metadata
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,

    FOREIGN KEY (pathway_id) REFERENCES neural_pathways(id) ON DELETE CASCADE,
    UNIQUE(pathway_id, workflow_type)
);

-- Indexes for workflow integration
CREATE INDEX IF NOT EXISTS idx_workflow_learning_workflow ON workflow_learning_events(workflow_instance_id);
CREATE INDEX IF NOT EXISTS idx_workflow_learning_type ON workflow_learning_events(signal_type);
CREATE INDEX IF NOT EXISTS idx_workflow_learning_unprocessed ON workflow_learning_events(learning_applied) WHERE learning_applied = 0;
CREATE INDEX IF NOT EXISTS idx_pathway_workflow_pathway ON pathway_workflow_mapping(pathway_id);
CREATE INDEX IF NOT EXISTS idx_pathway_workflow_type ON pathway_workflow_mapping(workflow_type);

-- ============================================================================
-- CROSS-DATABASE VIEWS
-- ============================================================================

-- Strong pathways for workflow routing (STDP-informed)
CREATE VIEW IF NOT EXISTS v_strong_pathways_for_routing AS
SELECT
    np.pathway_id,
    np.pathway_type,
    np.pathway_name,
    np.source_id,
    np.source_type,
    np.target_id,
    np.target_type,
    np.strength,
    np.net_plasticity,
    np.strength_trend,
    np.activation_count,
    np.consolidation_level,
    (SELECT COUNT(*) FROM pathway_workflow_mapping pwm
     WHERE pwm.pathway_id = np.id) AS workflow_associations
FROM neural_pathways np
WHERE np.is_strong = 1 AND np.state = 'active'
ORDER BY np.strength DESC, np.activation_count DESC;

-- Learning pulse summary for workflow feedback
CREATE VIEW IF NOT EXISTS v_learning_pulse_summary AS
SELECT
    hp.pulse_number,
    hp.trigger_type,
    hp.scope,
    hp.pathways_evaluated,
    hp.pathways_reinforced,
    hp.pathways_weakened,
    hp.pathways_pruned,
    hp.net_reinforcement,
    hp.effectiveness_score,
    hp.timestamp,
    (SELECT COUNT(*) FROM workflow_learning_events wle
     WHERE wle.timestamp BETWEEN hp.start_time AND COALESCE(hp.end_time, datetime('now'))
    ) AS workflow_signals_during_pulse
FROM hebbian_pulses hp
WHERE hp.timestamp > datetime('now', '-24 hours')
ORDER BY hp.timestamp DESC;

-- Workflow learning effectiveness
CREATE VIEW IF NOT EXISTS v_workflow_learning_effectiveness AS
SELECT
    wle.workflow_type,
    wle.signal_type,
    COUNT(*) AS event_count,
    AVG(wle.signal_strength) AS avg_signal_strength,
    AVG(wle.ltp_delta) AS avg_ltp_delta,
    AVG(wle.ltd_delta) AS avg_ltd_delta,
    SUM(CASE WHEN wle.learning_applied = 1 THEN 1 ELSE 0 END) AS applied_count
FROM workflow_learning_events wle
WHERE wle.timestamp > datetime('now', '-7 days')
GROUP BY wle.workflow_type, wle.signal_type
ORDER BY event_count DESC;

-- Pattern recommendations for workflows
CREATE VIEW IF NOT EXISTS v_pattern_recommendations AS
SELECT
    pr.pattern_id,
    pr.pattern_type,
    pr.pattern_name,
    pr.reinforcement_strength,
    pr.confidence_score,
    pr.state,
    pr.is_reliable,
    (SELECT pwm.workflow_type FROM pathway_workflow_mapping pwm
     JOIN neural_pathways np ON pwm.pathway_id = np.id
     WHERE np.id = pr.primary_pathway_id
     LIMIT 1) AS associated_workflow_type
FROM pattern_reinforcement pr
WHERE pr.is_reliable = 1 AND pr.state IN ('established', 'strong')
ORDER BY pr.reinforcement_strength DESC;

-- ============================================================================
-- WORKFLOW INTEGRATION TRIGGERS
-- ============================================================================

-- Auto-create learning event when pathway is activated by workflow
CREATE TRIGGER IF NOT EXISTS trg_pathway_workflow_activation
AFTER INSERT ON pathway_activations
WHEN NEW.trigger_source LIKE 'workflow:%'
BEGIN
    INSERT OR IGNORE INTO pathway_workflow_mapping (pathway_id, workflow_type)
    VALUES (
        NEW.pathway_id,
        SUBSTR(NEW.trigger_source, 10)  -- Extract workflow type from 'workflow:type'
    );

    UPDATE pathway_workflow_mapping
    SET activation_count = activation_count + 1,
        updated_at = CURRENT_TIMESTAMP
    WHERE pathway_id = NEW.pathway_id
    AND workflow_type = SUBSTR(NEW.trigger_source, 10);
END;

-- Update pathway_workflow_mapping.updated_at on modification
CREATE TRIGGER IF NOT EXISTS trg_pathway_workflow_updated_at
AFTER UPDATE ON pathway_workflow_mapping
BEGIN
    UPDATE pathway_workflow_mapping SET updated_at = CURRENT_TIMESTAMP WHERE id = NEW.id;
END;

-- Apply STDP-based learning from workflow outcomes
CREATE TRIGGER IF NOT EXISTS trg_workflow_learning_apply
AFTER INSERT ON workflow_learning_events
BEGIN
    -- Update associated pathways based on workflow outcome
    UPDATE neural_pathways
    SET
        ltp = CASE
            WHEN NEW.signal_type = 'success' THEN MIN(ltp + (NEW.signal_strength * ltp_rate), 1.0)
            ELSE ltp
        END,
        ltd = CASE
            WHEN NEW.signal_type IN ('failure', 'rollback') THEN MIN(ltd + (NEW.signal_strength * ltd_rate), 1.0)
            ELSE ltd
        END,
        strength = CASE
            WHEN NEW.signal_type = 'success' THEN MIN(strength + (NEW.signal_strength * 0.05), 1.0)
            WHEN NEW.signal_type IN ('failure', 'rollback') THEN MAX(strength - (NEW.signal_strength * 0.03), 0.0)
            ELSE strength
        END
    WHERE pathway_id IN (
        SELECT pwm.pathway_id FROM pathway_workflow_mapping pwm
        WHERE pwm.workflow_type = NEW.workflow_type
    );
END;

-- ============================================================================
-- End of Migration 003_hebbian_pulse.sql
-- ============================================================================
