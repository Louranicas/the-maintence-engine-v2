-- ============================================================================
-- Migration: 011_evolution_tracking.sql
-- Purpose: L7 Observer Layer evolution tracking, mutation management,
--          fitness history, and emergence logging
-- Database: evolution_tracking.db
-- Version: 1.0.0
--
-- CROSS-DATABASE INTEGRATION:
-- This database supports the L7 Observer Layer (M37-M39) and integrates
-- with existing Maintenance Engine databases:
--
-- | Database               | Integration Point                     | Purpose                    |
-- |------------------------|---------------------------------------|----------------------------|
-- | service_tracking.db    | mutations.target_module               | Service-level mutations    |
-- | hebbian_pulse.db       | mutations.target_parameter            | Pathway weight tuning      |
-- | tensor_memory.db       | fitness_history.dimension_scores      | 12D tensor fitness         |
-- | performance_metrics.db | fitness_history.overall_fitness       | Performance correlation    |
-- | consensus_tracking.db  | generations.phase                     | RALPH loop consensus       |
-- | flow_state.db          | correlations.source_layer             | Cross-layer state tracking |
--
-- L7 MODULES:
-- | Module | Usage                                                |
-- |--------|------------------------------------------------------|
-- | M37    | correlations (Log Correlator cross-layer events)     |
-- | M38    | emergence_log (Emergence Detector behaviors)         |
-- | M39    | mutations, fitness_history, generations (Evolution)  |
--
-- ============================================================================

--------------------------------------------------------------------------------
-- SCHEMA VERSION
--------------------------------------------------------------------------------
INSERT INTO schema_versions (version, migration_name, checksum)
VALUES ('011', 'evolution_tracking', 'sha256:011_evolution_tracking');

--------------------------------------------------------------------------------
-- MUTATIONS
-- Tracks all generated mutations from the RALPH loop (M39 Evolution Chamber).
-- Each mutation targets a specific module parameter and records its lifecycle
-- from proposal through verification or rollback.
--------------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS mutations (
    id INTEGER PRIMARY KEY,
    generation INTEGER NOT NULL,
    mutation_type TEXT NOT NULL CHECK (mutation_type IN (
        'parameter', 'threshold', 'weight', 'topology'
    )),
    target_module TEXT NOT NULL,
    target_parameter TEXT NOT NULL,
    old_value REAL NOT NULL,
    new_value REAL NOT NULL,
    delta REAL NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN (
        'pending', 'applied', 'verified', 'rolled_back', 'rejected'
    )),
    fitness_before REAL,
    fitness_after REAL,
    applied_at TEXT,
    verified_at TEXT,
    rolled_back_at TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

--------------------------------------------------------------------------------
-- FITNESS HISTORY
-- Tracks fitness evaluations over time using 12D tensor scoring.
-- Each entry captures the full dimensional breakdown, weighted contributions,
-- trend direction, and stability assessment.
--------------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS fitness_history (
    id INTEGER PRIMARY KEY,
    overall_fitness REAL NOT NULL,
    dimension_scores TEXT NOT NULL,
    weighted_scores TEXT NOT NULL,
    trend TEXT NOT NULL CHECK (trend IN (
        'Improving', 'Declining', 'Stable', 'Volatile'
    )),
    weakest_dimension INTEGER NOT NULL,
    strongest_dimension INTEGER NOT NULL,
    stability REAL NOT NULL,
    evaluated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

--------------------------------------------------------------------------------
-- GENERATIONS
-- Tracks RALPH loop generations (Recognize, Analyze, Learn, Plan, Harmonize).
-- Each generation records mutation statistics and overall fitness impact.
--------------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS generations (
    id INTEGER PRIMARY KEY,
    generation_number INTEGER NOT NULL UNIQUE,
    phase TEXT NOT NULL CHECK (phase IN (
        'Recognize', 'Analyze', 'Learn', 'Plan', 'Harmonize'
    )),
    mutations_generated INTEGER NOT NULL DEFAULT 0,
    mutations_applied INTEGER NOT NULL DEFAULT 0,
    mutations_rolled_back INTEGER NOT NULL DEFAULT 0,
    fitness_delta REAL,
    started_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    completed_at TEXT
);

--------------------------------------------------------------------------------
-- EMERGENCE LOG
-- Tracks detected emergent behaviors from the Emergence Detector (M38).
-- Records cascade failures, synergy amplifications, resonance cycles, and
-- other cross-layer emergent phenomena.
--------------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS emergence_log (
    id INTEGER PRIMARY KEY,
    emergence_type TEXT NOT NULL CHECK (emergence_type IN (
        'CascadingFailure', 'SynergyAmplification', 'ResonanceCycle',
        'AdaptiveConvergence', 'CrossLayerAmplification', 'EmergentRecovery',
        'SystemPhaseLock'
    )),
    source_module TEXT NOT NULL,
    affected_modules TEXT NOT NULL,
    confidence REAL NOT NULL,
    severity TEXT NOT NULL CHECK (severity IN (
        'Low', 'Medium', 'High', 'Critical'
    )),
    acknowledged INTEGER NOT NULL DEFAULT 0,
    description TEXT,
    detected_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    acknowledged_at TEXT
);

--------------------------------------------------------------------------------
-- CORRELATIONS
-- Tracks cross-layer event correlations from the Log Correlator (M37).
-- Links events across different layers within temporal windows to discover
-- causal chains, resonance patterns, and cascading effects.
--------------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS correlations (
    id INTEGER PRIMARY KEY,
    correlation_type TEXT NOT NULL CHECK (correlation_type IN (
        'Temporal', 'Causal', 'Resonance', 'Cascade', 'Periodic'
    )),
    source_layer TEXT NOT NULL,
    target_layer TEXT NOT NULL,
    confidence REAL NOT NULL,
    window_start TEXT NOT NULL,
    window_end TEXT NOT NULL,
    event_count INTEGER NOT NULL,
    description TEXT,
    detected_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

--------------------------------------------------------------------------------
-- INDEXES
--------------------------------------------------------------------------------

-- Mutations indexes
CREATE INDEX IF NOT EXISTS idx_mutations_status
    ON mutations(status);
CREATE INDEX IF NOT EXISTS idx_mutations_generation
    ON mutations(generation);
CREATE INDEX IF NOT EXISTS idx_mutations_target
    ON mutations(target_module);

-- Fitness history indexes
CREATE INDEX IF NOT EXISTS idx_fitness_evaluated_at
    ON fitness_history(evaluated_at);

-- Emergence log indexes
CREATE INDEX IF NOT EXISTS idx_emergence_type
    ON emergence_log(emergence_type);
CREATE INDEX IF NOT EXISTS idx_emergence_detected_at
    ON emergence_log(detected_at);

-- Correlations indexes
CREATE INDEX IF NOT EXISTS idx_correlations_type
    ON correlations(correlation_type);
CREATE INDEX IF NOT EXISTS idx_correlations_detected_at
    ON correlations(detected_at);

--------------------------------------------------------------------------------
-- VIEWS
--------------------------------------------------------------------------------

-- Active mutations: pending or applied (not yet verified/rolled back/rejected)
CREATE VIEW IF NOT EXISTS v_active_mutations AS
SELECT
    id,
    generation,
    mutation_type,
    target_module,
    target_parameter,
    old_value,
    new_value,
    delta,
    status,
    fitness_before,
    fitness_after,
    applied_at,
    created_at
FROM mutations
WHERE status IN ('pending', 'applied')
ORDER BY created_at DESC;

-- Fitness trend: last 10 fitness evaluations with trend direction
CREATE VIEW IF NOT EXISTS v_fitness_trend AS
SELECT
    id,
    overall_fitness,
    dimension_scores,
    weighted_scores,
    trend,
    weakest_dimension,
    strongest_dimension,
    stability,
    evaluated_at
FROM fitness_history
ORDER BY evaluated_at DESC
LIMIT 10;

-- Latest generation: most recent RALPH loop generation
CREATE VIEW IF NOT EXISTS v_latest_generation AS
SELECT
    id,
    generation_number,
    phase,
    mutations_generated,
    mutations_applied,
    mutations_rolled_back,
    fitness_delta,
    started_at,
    completed_at
FROM generations
ORDER BY generation_number DESC
LIMIT 1;
