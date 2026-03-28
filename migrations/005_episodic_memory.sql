-- ============================================================================
-- Migration: 005_episodic_memory.sql
-- SYNTHEX Database Migration - Episodic Memory Module (NAM-06)
-- Version: 1.0.0
-- Pattern: NAM-06 Episodic Memory for experience-based learning
-- ============================================================================

INSERT INTO schema_versions (version, migration_name, checksum)
VALUES ('005', 'episodic_memory', 'sha256:005_episodic_memory');

-- ============================================================================
-- EPISODES TABLE
-- Core episodic memory storage - each episode represents a complete experience
-- ============================================================================
CREATE TABLE IF NOT EXISTS episodes (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    episode_id TEXT NOT NULL UNIQUE DEFAULT (lower(hex(randomblob(16)))),

    -- Episode Classification
    episode_type TEXT NOT NULL DEFAULT 'operational'
        CHECK(episode_type IN (
            'operational', 'incident', 'success', 'failure', 'learning',
            'interaction', 'decision', 'discovery', 'maintenance', 'custom'
        )),
    episode_name TEXT,

    -- Temporal Boundaries
    start_timestamp DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    end_timestamp DATETIME,
    duration_ms INTEGER GENERATED ALWAYS AS (
        CASE WHEN end_timestamp IS NOT NULL
        THEN CAST((julianday(end_timestamp) - julianday(start_timestamp)) * 86400000 AS INTEGER)
        ELSE NULL END
    ) STORED,

    -- Episode Triggers
    trigger_event TEXT NOT NULL,
    trigger_type TEXT DEFAULT 'external'
        CHECK(trigger_type IN ('external', 'internal', 'scheduled', 'threshold', 'cascade', 'manual')),
    trigger_source TEXT,
    trigger_data TEXT, -- JSON blob

    -- Episode Resolution
    resolution_event TEXT,
    resolution_type TEXT
        CHECK(resolution_type IN ('success', 'failure', 'partial', 'timeout', 'cancelled', 'ongoing')),
    resolution_data TEXT, -- JSON blob

    -- Outcome Assessment
    outcome TEXT
        CHECK(outcome IN ('positive', 'negative', 'neutral', 'mixed', 'unknown')),
    outcome_score REAL CHECK(outcome_score >= -1.0 AND outcome_score <= 1.0),
    outcome_confidence REAL DEFAULT 0.5 CHECK(outcome_confidence >= 0.0 AND outcome_confidence <= 1.0),

    -- Context
    context TEXT, -- JSON blob with environmental context
    participants TEXT, -- JSON array of involved agents/systems
    affected_systems TEXT, -- JSON array

    -- Tensor Signature (NAM-06 Pattern)
    tensor_signature TEXT, -- Encoded tensor representation for similarity matching
    embedding_vector TEXT, -- JSON array of floats for vector similarity search
    embedding_model TEXT DEFAULT 'default',

    -- Memory Attributes
    salience REAL DEFAULT 0.5 CHECK(salience >= 0.0 AND salience <= 1.0),
    emotional_valence REAL DEFAULT 0.0 CHECK(emotional_valence >= -1.0 AND emotional_valence <= 1.0),
    novelty_score REAL DEFAULT 0.5 CHECK(novelty_score >= 0.0 AND novelty_score <= 1.0),

    -- Retrieval Statistics
    retrieval_count INTEGER DEFAULT 0,
    last_retrieved DATETIME,
    avg_retrieval_relevance REAL DEFAULT 0.0,

    -- Memory Consolidation Status
    consolidation_status TEXT DEFAULT 'working'
        CHECK(consolidation_status IN ('sensory', 'working', 'short_term', 'long_term', 'permanent')),
    consolidation_strength REAL DEFAULT 0.0,
    decay_rate REAL DEFAULT 0.01,

    -- Calculated Fields
    is_complete INTEGER GENERATED ALWAYS AS (
        CASE WHEN end_timestamp IS NOT NULL THEN 1 ELSE 0 END
    ) STORED,
    memory_strength REAL GENERATED ALWAYS AS (
        salience * (1 + (retrieval_count * 0.1)) * (1 - decay_rate)
    ) STORED,
    is_significant INTEGER GENERATED ALWAYS AS (
        CASE WHEN salience > 0.7 OR ABS(emotional_valence) > 0.5 OR novelty_score > 0.7 THEN 1 ELSE 0 END
    ) STORED,

    -- Metadata
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    metadata TEXT -- JSON blob
);

-- ============================================================================
-- EPISODE LINKS TABLE
-- Connects related episodes for associative memory
-- ============================================================================
CREATE TABLE IF NOT EXISTS episode_links (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    link_id TEXT NOT NULL UNIQUE DEFAULT (lower(hex(randomblob(16)))),

    -- Linked Episodes
    source_episode_id INTEGER NOT NULL,
    target_episode_id INTEGER NOT NULL,

    -- Link Classification
    link_type TEXT NOT NULL
        CHECK(link_type IN (
            'causal', 'temporal', 'similarity', 'contrast', 'elaboration',
            'generalization', 'specialization', 'prerequisite', 'consequence',
            'analogy', 'contradiction', 'reinforcement', 'custom'
        )),
    link_subtype TEXT,

    -- Link Strength (Associative Strength)
    strength REAL NOT NULL DEFAULT 0.5 CHECK(strength >= 0.0 AND strength <= 1.0),
    initial_strength REAL DEFAULT 0.5,

    -- Bidirectionality
    is_bidirectional INTEGER DEFAULT 0,
    reverse_strength REAL CHECK(reverse_strength IS NULL OR (reverse_strength >= 0.0 AND reverse_strength <= 1.0)),

    -- Temporal Relationship
    temporal_distance_ms INTEGER, -- Time between episodes
    temporal_order TEXT
        CHECK(temporal_order IN ('before', 'after', 'concurrent', 'overlapping')),

    -- Similarity Metrics (for similarity links)
    cosine_similarity REAL CHECK(cosine_similarity IS NULL OR (cosine_similarity >= -1.0 AND cosine_similarity <= 1.0)),
    jaccard_similarity REAL CHECK(jaccard_similarity IS NULL OR (jaccard_similarity >= 0.0 AND jaccard_similarity <= 1.0)),

    -- Link Confidence
    confidence REAL DEFAULT 0.5 CHECK(confidence >= 0.0 AND confidence <= 1.0),
    evidence_count INTEGER DEFAULT 1,

    -- Usage Statistics
    traversal_count INTEGER DEFAULT 0,
    last_traversed DATETIME,
    successful_retrievals INTEGER DEFAULT 0,

    -- Link State
    state TEXT DEFAULT 'active'
        CHECK(state IN ('active', 'dormant', 'strengthening', 'weakening', 'pruned')),

    -- Calculated Fields
    effective_strength REAL GENERATED ALWAYS AS (strength * confidence) STORED,
    is_strong_link INTEGER GENERATED ALWAYS AS (
        CASE WHEN strength > 0.7 AND confidence > 0.7 THEN 1 ELSE 0 END
    ) STORED,

    -- Metadata
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    discovered_by TEXT, -- Agent that discovered this link
    metadata TEXT,

    FOREIGN KEY (source_episode_id) REFERENCES episodes(id) ON DELETE CASCADE,
    FOREIGN KEY (target_episode_id) REFERENCES episodes(id) ON DELETE CASCADE,
    UNIQUE(source_episode_id, target_episode_id, link_type)
);

-- ============================================================================
-- EPISODE DETAILS TABLE
-- Stores detailed event sequences within episodes
-- ============================================================================
CREATE TABLE IF NOT EXISTS episode_details (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    episode_id INTEGER NOT NULL,

    -- Event Sequence
    sequence_number INTEGER NOT NULL,
    event_type TEXT NOT NULL,
    event_data TEXT, -- JSON blob

    -- Timing
    timestamp DATETIME DEFAULT CURRENT_TIMESTAMP,
    relative_time_ms INTEGER, -- Time from episode start
    duration_ms INTEGER,

    -- Importance
    importance REAL DEFAULT 0.5 CHECK(importance >= 0.0 AND importance <= 1.0),
    is_key_event INTEGER DEFAULT 0,

    FOREIGN KEY (episode_id) REFERENCES episodes(id) ON DELETE CASCADE
);

-- ============================================================================
-- INDEXES FOR PERFORMANCE
-- ============================================================================

-- Episodes Indexes
CREATE INDEX IF NOT EXISTS idx_episodes_type ON episodes(episode_type);
CREATE INDEX IF NOT EXISTS idx_episodes_start ON episodes(start_timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_episodes_end ON episodes(end_timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_episodes_outcome ON episodes(outcome);
CREATE INDEX IF NOT EXISTS idx_episodes_trigger ON episodes(trigger_type);
CREATE INDEX IF NOT EXISTS idx_episodes_consolidation ON episodes(consolidation_status);
CREATE INDEX IF NOT EXISTS idx_episodes_strength ON episodes(memory_strength DESC);
CREATE INDEX IF NOT EXISTS idx_episodes_significant ON episodes(is_significant) WHERE is_significant = 1;
CREATE INDEX IF NOT EXISTS idx_episodes_complete ON episodes(is_complete);
CREATE INDEX IF NOT EXISTS idx_episodes_salience ON episodes(salience DESC);
CREATE INDEX IF NOT EXISTS idx_episodes_retrieval ON episodes(retrieval_count DESC);

-- Episode Links Indexes
CREATE INDEX IF NOT EXISTS idx_links_source ON episode_links(source_episode_id);
CREATE INDEX IF NOT EXISTS idx_links_target ON episode_links(target_episode_id);
CREATE INDEX IF NOT EXISTS idx_links_type ON episode_links(link_type);
CREATE INDEX IF NOT EXISTS idx_links_strength ON episode_links(strength DESC);
CREATE INDEX IF NOT EXISTS idx_links_effective ON episode_links(effective_strength DESC);
CREATE INDEX IF NOT EXISTS idx_links_strong ON episode_links(is_strong_link) WHERE is_strong_link = 1;
CREATE INDEX IF NOT EXISTS idx_links_state ON episode_links(state);
CREATE INDEX IF NOT EXISTS idx_links_traversal ON episode_links(traversal_count DESC);

-- Episode Details Indexes
CREATE INDEX IF NOT EXISTS idx_details_episode ON episode_details(episode_id);
CREATE INDEX IF NOT EXISTS idx_details_sequence ON episode_details(episode_id, sequence_number);
CREATE INDEX IF NOT EXISTS idx_details_key ON episode_details(is_key_event) WHERE is_key_event = 1;

-- ============================================================================
-- TRIGGERS FOR AUTOMATIC UPDATES
-- ============================================================================

-- Update episodes.updated_at on modification
CREATE TRIGGER IF NOT EXISTS trg_episodes_updated_at
AFTER UPDATE ON episodes
BEGIN
    UPDATE episodes SET updated_at = CURRENT_TIMESTAMP WHERE id = NEW.id;
END;

-- Update episode_links.updated_at on modification
CREATE TRIGGER IF NOT EXISTS trg_links_updated_at
AFTER UPDATE ON episode_links
BEGIN
    UPDATE episode_links SET updated_at = CURRENT_TIMESTAMP WHERE id = NEW.id;
END;

-- Auto-update retrieval statistics when episode is accessed
CREATE TRIGGER IF NOT EXISTS trg_episode_retrieved
AFTER UPDATE OF retrieval_count ON episodes
BEGIN
    UPDATE episodes SET last_retrieved = CURRENT_TIMESTAMP WHERE id = NEW.id;
END;

-- Auto-update link traversal statistics
CREATE TRIGGER IF NOT EXISTS trg_link_traversed
AFTER UPDATE OF traversal_count ON episode_links
BEGIN
    UPDATE episode_links SET last_traversed = CURRENT_TIMESTAMP WHERE id = NEW.id;
END;

-- Create reverse link for bidirectional links
CREATE TRIGGER IF NOT EXISTS trg_create_reverse_link
AFTER INSERT ON episode_links
WHEN NEW.is_bidirectional = 1
BEGIN
    INSERT OR IGNORE INTO episode_links (
        source_episode_id, target_episode_id, link_type, link_subtype,
        strength, is_bidirectional, temporal_order, confidence, discovered_by
    )
    VALUES (
        NEW.target_episode_id,
        NEW.source_episode_id,
        NEW.link_type,
        NEW.link_subtype,
        COALESCE(NEW.reverse_strength, NEW.strength),
        1,
        CASE NEW.temporal_order
            WHEN 'before' THEN 'after'
            WHEN 'after' THEN 'before'
            ELSE NEW.temporal_order
        END,
        NEW.confidence,
        NEW.discovered_by
    );
END;

-- ============================================================================
-- WORKFLOW INTEGRATION TABLES
-- ============================================================================

-- Workflow Episodes (NAM-06 compliant episode recording for workflows)
CREATE TABLE IF NOT EXISTS workflow_episodes (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    episode_id INTEGER NOT NULL,  -- References episodes table

    -- Workflow reference
    workflow_instance_id TEXT NOT NULL,  -- References workflow_tracking.workflow_instances
    workflow_type TEXT NOT NULL,

    -- Episode metadata
    step_count INTEGER DEFAULT 0,
    successful_steps INTEGER DEFAULT 0,
    failed_steps INTEGER DEFAULT 0,

    -- Learning signals
    primary_learning_signal TEXT CHECK(primary_learning_signal IN ('positive', 'negative', 'neutral')),
    learning_weight REAL DEFAULT 1.0,

    FOREIGN KEY (episode_id) REFERENCES episodes(id) ON DELETE CASCADE
);

-- Workflow Step Events (detailed step-by-step recording)
CREATE TABLE IF NOT EXISTS workflow_step_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    episode_id INTEGER NOT NULL,
    step_execution_id TEXT NOT NULL,  -- References workflow_tracking.step_executions

    -- Step details
    step_name TEXT NOT NULL,
    step_type TEXT NOT NULL,
    step_order INTEGER NOT NULL,

    -- Outcome
    outcome TEXT NOT NULL CHECK(outcome IN ('success', 'failure', 'skipped', 'timeout')),
    duration_ms INTEGER,
    error_message TEXT,

    -- Importance for learning
    importance REAL DEFAULT 0.5,
    is_key_event INTEGER DEFAULT 0,

    -- Timing
    timestamp DATETIME DEFAULT CURRENT_TIMESTAMP,

    FOREIGN KEY (episode_id) REFERENCES episodes(id) ON DELETE CASCADE
);

-- Workflow Pattern Discoveries (patterns learned from workflow episodes)
CREATE TABLE IF NOT EXISTS workflow_pattern_discoveries (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    pattern_id TEXT NOT NULL UNIQUE DEFAULT (lower(hex(randomblob(16)))),

    -- Pattern details
    pattern_name TEXT NOT NULL,
    pattern_type TEXT NOT NULL CHECK(pattern_type IN (
        'success_sequence', 'failure_sequence', 'recovery_pattern',
        'optimization_opportunity', 'anti_pattern', 'dependency_chain'
    )),
    pattern_signature TEXT NOT NULL,

    -- Discovery context
    discovered_from_episodes TEXT,  -- JSON array of episode_ids
    discovery_confidence REAL DEFAULT 0.5,

    -- Workflow application
    applicable_workflow_types TEXT,  -- JSON array
    recommendation TEXT,

    -- Validation
    validation_count INTEGER DEFAULT 0,
    success_rate REAL DEFAULT 0.0,

    -- Metadata
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Indexes for workflow integration
CREATE INDEX IF NOT EXISTS idx_workflow_episodes_instance ON workflow_episodes(workflow_instance_id);
CREATE INDEX IF NOT EXISTS idx_workflow_episodes_type ON workflow_episodes(workflow_type);
CREATE INDEX IF NOT EXISTS idx_workflow_step_events_episode ON workflow_step_events(episode_id);
CREATE INDEX IF NOT EXISTS idx_workflow_step_events_execution ON workflow_step_events(step_execution_id);
CREATE INDEX IF NOT EXISTS idx_workflow_patterns_type ON workflow_pattern_discoveries(pattern_type);

-- ============================================================================
-- CROSS-DATABASE VIEWS
-- ============================================================================

-- Workflow episode summary for learning
CREATE VIEW IF NOT EXISTS v_workflow_episode_summary AS
SELECT
    we.workflow_instance_id,
    we.workflow_type,
    e.episode_type,
    e.outcome,
    e.outcome_score,
    e.salience,
    e.memory_strength,
    e.consolidation_status,
    we.step_count,
    we.successful_steps,
    we.failed_steps,
    CAST(we.successful_steps AS REAL) / NULLIF(we.step_count, 0) AS success_rate,
    e.start_timestamp,
    e.end_timestamp,
    e.duration_ms
FROM workflow_episodes we
JOIN episodes e ON we.episode_id = e.id
ORDER BY e.memory_strength DESC;

-- Similar workflow episodes for pattern matching
CREATE VIEW IF NOT EXISTS v_similar_workflow_episodes AS
SELECT
    we1.workflow_instance_id AS source_workflow,
    we2.workflow_instance_id AS similar_workflow,
    we1.workflow_type,
    el.link_type,
    el.strength AS similarity_strength,
    el.cosine_similarity,
    e1.outcome AS source_outcome,
    e2.outcome AS similar_outcome
FROM workflow_episodes we1
JOIN episode_links el ON el.source_episode_id = we1.episode_id
JOIN workflow_episodes we2 ON we2.episode_id = el.target_episode_id
JOIN episodes e1 ON we1.episode_id = e1.id
JOIN episodes e2 ON we2.episode_id = e2.id
WHERE el.link_type = 'similarity' AND el.strength > 0.7
ORDER BY el.strength DESC;

-- High-salience workflow events for learning
CREATE VIEW IF NOT EXISTS v_high_salience_workflow_events AS
SELECT
    wse.step_execution_id,
    wse.step_name,
    wse.step_type,
    wse.outcome,
    wse.importance,
    wse.duration_ms,
    e.episode_id,
    we.workflow_instance_id,
    we.workflow_type,
    e.salience AS episode_salience,
    e.outcome AS episode_outcome
FROM workflow_step_events wse
JOIN episodes e ON wse.episode_id = e.id
JOIN workflow_episodes we ON we.episode_id = e.id
WHERE wse.is_key_event = 1 OR wse.importance > 0.7
ORDER BY e.salience DESC, wse.importance DESC;

-- Workflow pattern effectiveness
CREATE VIEW IF NOT EXISTS v_workflow_pattern_effectiveness AS
SELECT
    wpd.pattern_id,
    wpd.pattern_name,
    wpd.pattern_type,
    wpd.discovery_confidence,
    wpd.validation_count,
    wpd.success_rate,
    wpd.applicable_workflow_types,
    wpd.recommendation,
    wpd.created_at
FROM workflow_pattern_discoveries wpd
WHERE wpd.validation_count >= 3 AND wpd.success_rate > 0.7
ORDER BY wpd.success_rate DESC, wpd.validation_count DESC;

-- ============================================================================
-- WORKFLOW INTEGRATION TRIGGERS
-- ============================================================================

-- Auto-create episode when workflow instance is created
CREATE TRIGGER IF NOT EXISTS trg_create_workflow_episode
AFTER INSERT ON workflow_episodes
BEGIN
    -- Create episode link to recent similar workflow episodes
    INSERT INTO episode_links (source_episode_id, target_episode_id, link_type, strength, confidence)
    SELECT
        NEW.episode_id,
        we2.episode_id,
        'similarity',
        0.5,
        0.5
    FROM workflow_episodes we2
    WHERE we2.workflow_type = NEW.workflow_type
    AND we2.episode_id != NEW.episode_id
    AND we2.id IN (
        SELECT id FROM workflow_episodes
        WHERE workflow_type = NEW.workflow_type
        ORDER BY id DESC LIMIT 5
    );
END;

-- Update episode on workflow completion
CREATE TRIGGER IF NOT EXISTS trg_complete_workflow_episode
AFTER UPDATE OF successful_steps, failed_steps ON workflow_episodes
BEGIN
    UPDATE episodes
    SET
        outcome = CASE
            WHEN NEW.failed_steps = 0 THEN 'positive'
            WHEN NEW.successful_steps = 0 THEN 'negative'
            ELSE 'mixed'
        END,
        outcome_score = (CAST(NEW.successful_steps AS REAL) / NULLIF(NEW.step_count, 0)) * 2 - 1,
        salience = CASE
            WHEN NEW.failed_steps > 0 THEN MIN(0.5 + (NEW.failed_steps * 0.1), 1.0)
            ELSE 0.5
        END
    WHERE id = NEW.episode_id;
END;

-- Mark key events for significant step outcomes
CREATE TRIGGER IF NOT EXISTS trg_mark_key_workflow_events
AFTER INSERT ON workflow_step_events
WHEN NEW.outcome IN ('failure', 'timeout') OR NEW.duration_ms > 60000
BEGIN
    UPDATE workflow_step_events
    SET is_key_event = 1,
        importance = CASE
            WHEN NEW.outcome = 'failure' THEN 0.9
            WHEN NEW.outcome = 'timeout' THEN 0.8
            ELSE 0.7
        END
    WHERE id = NEW.id;
END;

-- Update pattern discovery on modification
CREATE TRIGGER IF NOT EXISTS trg_workflow_pattern_updated
AFTER UPDATE ON workflow_pattern_discoveries
BEGIN
    UPDATE workflow_pattern_discoveries SET updated_at = CURRENT_TIMESTAMP WHERE id = NEW.id;
END;

-- ============================================================================
-- End of Migration 005_episodic_memory.sql
-- ============================================================================
