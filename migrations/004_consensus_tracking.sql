-- ============================================================================
-- Migration: 004_consensus_tracking.sql
-- SYNTHEX Database Migration - Consensus Tracking Module
-- Version: 1.0.0
-- Pattern: PBFT-inspired consensus for multi-agent coordination
-- ============================================================================

INSERT INTO schema_versions (version, migration_name, checksum)
VALUES ('004', 'consensus_tracking', 'sha256:004_consensus_tracking');

-- ============================================================================
-- CONSENSUS PROPOSALS TABLE
-- Tracks all proposals submitted for consensus
-- ============================================================================
CREATE TABLE IF NOT EXISTS consensus_proposals (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    proposal_id TEXT NOT NULL UNIQUE DEFAULT (lower(hex(randomblob(16)))),

    -- PBFT View and Sequence
    view_number INTEGER NOT NULL DEFAULT 0,
    sequence_number INTEGER NOT NULL,

    -- Proposal Content
    action_type TEXT NOT NULL
        CHECK(action_type IN (
            'config_change', 'state_update', 'resource_allocation', 'agent_action',
            'emergency_response', 'maintenance', 'scaling', 'deployment',
            'rollback', 'recovery', 'custom'
        )),
    action_data TEXT NOT NULL, -- JSON blob with action details
    action_hash TEXT, -- SHA256 of action_data for verification

    -- Proposer Information
    proposer_agent TEXT NOT NULL,
    proposer_tier INTEGER,
    proposer_weight REAL DEFAULT 1.0,

    -- Proposal State
    status TEXT DEFAULT 'pending'
        CHECK(status IN (
            'pending', 'pre_prepare', 'prepare', 'commit',
            'executed', 'rejected', 'timeout', 'cancelled'
        )),
    phase TEXT DEFAULT 'pre_prepare'
        CHECK(phase IN ('pre_prepare', 'prepare', 'commit', 'reply')),

    -- Timing
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    pre_prepare_at DATETIME,
    prepare_at DATETIME,
    commit_at DATETIME,
    executed_at DATETIME,

    -- Timeout Configuration
    timeout_ms INTEGER DEFAULT 30000,
    deadline DATETIME,

    -- Quorum Requirements
    required_quorum REAL DEFAULT 0.67,
    total_voters INTEGER DEFAULT 0,
    quorum_size INTEGER GENERATED ALWAYS AS (
        CAST(CEIL(total_voters * required_quorum) AS INTEGER)
    ) STORED,

    -- Priority and Urgency
    priority INTEGER DEFAULT 5 CHECK(priority >= 1 AND priority <= 10),
    is_urgent INTEGER DEFAULT 0,

    -- Execution Details
    execution_result TEXT,
    execution_error TEXT,
    execution_duration_ms INTEGER,

    -- Calculated Fields
    is_expired INTEGER GENERATED ALWAYS AS (
        CASE WHEN deadline IS NOT NULL AND deadline < CURRENT_TIMESTAMP
             AND status NOT IN ('executed', 'rejected', 'cancelled')
        THEN 1 ELSE 0 END
    ) STORED,

    -- Metadata
    metadata TEXT, -- JSON blob

    UNIQUE(view_number, sequence_number)
);

-- ============================================================================
-- CONSENSUS VOTES TABLE
-- Records individual votes from agents
-- ============================================================================
CREATE TABLE IF NOT EXISTS consensus_votes (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    vote_id TEXT NOT NULL UNIQUE DEFAULT (lower(hex(randomblob(16)))),

    -- Vote Target
    proposal_id INTEGER NOT NULL,

    -- Voter Information
    agent_id TEXT NOT NULL,
    agent_tier INTEGER,
    agent_weight REAL DEFAULT 1.0,

    -- Vote Details
    vote TEXT NOT NULL
        CHECK(vote IN ('approve', 'reject', 'abstain')),
    vote_weight REAL DEFAULT 1.0,

    -- PBFT Phase
    phase TEXT NOT NULL
        CHECK(phase IN ('pre_prepare', 'prepare', 'commit')),

    -- Timing
    timestamp DATETIME DEFAULT CURRENT_TIMESTAMP,
    received_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    processing_time_ms INTEGER,

    -- Vote Justification
    reason TEXT,
    confidence REAL DEFAULT 1.0 CHECK(confidence >= 0.0 AND confidence <= 1.0),

    -- Cryptographic Verification (optional)
    signature TEXT,
    public_key_id TEXT,
    verified INTEGER DEFAULT 0,

    -- Calculated Fields
    effective_weight REAL GENERATED ALWAYS AS (vote_weight * confidence) STORED,

    -- Metadata
    metadata TEXT,

    FOREIGN KEY (proposal_id) REFERENCES consensus_proposals(id) ON DELETE CASCADE,
    UNIQUE(proposal_id, agent_id, phase)
);

-- ============================================================================
-- CONSENSUS OUTCOMES TABLE
-- Final outcome of each consensus round
-- ============================================================================
CREATE TABLE IF NOT EXISTS consensus_outcomes (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    outcome_id TEXT NOT NULL UNIQUE DEFAULT (lower(hex(randomblob(16)))),

    -- Reference to Proposal
    proposal_id INTEGER NOT NULL UNIQUE,

    -- Quorum Results
    quorum_reached INTEGER NOT NULL DEFAULT 0,
    total_votes INTEGER DEFAULT 0,
    votes_for INTEGER DEFAULT 0,
    votes_against INTEGER DEFAULT 0,
    votes_abstain INTEGER DEFAULT 0,

    -- Weighted Voting Results
    weighted_votes_for REAL DEFAULT 0.0,
    weighted_votes_against REAL DEFAULT 0.0,
    weighted_total REAL DEFAULT 0.0,
    approval_percentage REAL GENERATED ALWAYS AS (
        CASE WHEN weighted_total > 0
        THEN (weighted_votes_for / weighted_total) * 100
        ELSE 0.0 END
    ) STORED,

    -- Outcome
    decision TEXT NOT NULL
        CHECK(decision IN ('approved', 'rejected', 'timeout', 'cancelled', 'deferred')),
    decision_timestamp DATETIME DEFAULT CURRENT_TIMESTAMP,

    -- Execution Status
    execution_status TEXT DEFAULT 'pending'
        CHECK(execution_status IN ('pending', 'executing', 'completed', 'failed', 'skipped', 'rolled_back')),
    execution_started_at DATETIME,
    execution_completed_at DATETIME,
    execution_attempts INTEGER DEFAULT 0,

    -- Execution Results
    execution_result TEXT, -- JSON blob
    execution_error TEXT,
    rollback_performed INTEGER DEFAULT 0,
    rollback_at DATETIME,

    -- Consensus Quality Metrics
    consensus_strength REAL DEFAULT 0.0, -- How strong the agreement was
    consensus_time_ms INTEGER, -- Time to reach consensus
    rounds_required INTEGER DEFAULT 1,

    -- Calculated Fields
    participation_rate REAL GENERATED ALWAYS AS (
        CASE WHEN total_votes > 0
        THEN CAST(total_votes AS REAL) / NULLIF(
            (SELECT total_voters FROM consensus_proposals WHERE id = proposal_id), 0
        ) * 100
        ELSE 0.0 END
    ) STORED,

    -- Metadata
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    metadata TEXT,

    FOREIGN KEY (proposal_id) REFERENCES consensus_proposals(id)
);

-- ============================================================================
-- DISSENT EVENTS TABLE
-- Tracks disagreements and dissenting opinions for learning
-- ============================================================================
CREATE TABLE IF NOT EXISTS dissent_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    dissent_id TEXT NOT NULL UNIQUE DEFAULT (lower(hex(randomblob(16)))),

    -- Reference to Proposal/Vote
    proposal_id INTEGER NOT NULL,
    vote_id INTEGER,

    -- Dissenting Agent
    dissenting_agent TEXT NOT NULL,
    agent_tier INTEGER,
    agent_weight REAL DEFAULT 1.0,

    -- Proposed Action Context
    proposed_action TEXT NOT NULL,
    action_category TEXT,

    -- Dissent Details
    dissent_reason TEXT NOT NULL,
    dissent_category TEXT
        CHECK(dissent_category IN (
            'safety', 'performance', 'resource', 'timing', 'policy',
            'priority', 'dependency', 'risk', 'ethical', 'technical', 'other'
        )),
    severity TEXT DEFAULT 'medium'
        CHECK(severity IN ('low', 'medium', 'high', 'critical')),

    -- Alternative Proposal
    alternative_action TEXT,
    alternative_rationale TEXT,

    -- Dissent Outcome
    outcome TEXT
        CHECK(outcome IN (
            'overruled', 'accepted', 'compromised', 'deferred',
            'escalated', 'withdrawn', 'pending'
        )),
    outcome_reason TEXT,
    outcome_timestamp DATETIME,

    -- Resolution
    resolution_agent TEXT,
    resolution_notes TEXT,

    -- Learning Metrics
    was_correct INTEGER, -- Post-hoc evaluation: was dissent justified?
    correctness_evidence TEXT,
    learning_incorporated INTEGER DEFAULT 0,

    -- Calculated Fields
    is_high_severity INTEGER GENERATED ALWAYS AS (
        CASE WHEN severity IN ('high', 'critical') THEN 1 ELSE 0 END
    ) STORED,

    -- Metadata
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    metadata TEXT,

    FOREIGN KEY (proposal_id) REFERENCES consensus_proposals(id),
    FOREIGN KEY (vote_id) REFERENCES consensus_votes(id)
);

-- ============================================================================
-- CONSENSUS ROUNDS TABLE
-- Tracks multiple rounds for proposals requiring re-voting
-- ============================================================================
CREATE TABLE IF NOT EXISTS consensus_rounds (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    round_id TEXT NOT NULL UNIQUE DEFAULT (lower(hex(randomblob(16)))),

    -- Reference
    proposal_id INTEGER NOT NULL,
    round_number INTEGER NOT NULL DEFAULT 1,

    -- Round State
    status TEXT DEFAULT 'active'
        CHECK(status IN ('active', 'completed', 'timeout', 'superseded')),

    -- Timing
    started_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    ended_at DATETIME,
    duration_ms INTEGER,

    -- Round Results
    votes_collected INTEGER DEFAULT 0,
    approval_rate REAL DEFAULT 0.0,

    -- Round Outcome
    outcome TEXT
        CHECK(outcome IN ('success', 'failure', 'timeout', 'retry')),
    retry_reason TEXT,

    FOREIGN KEY (proposal_id) REFERENCES consensus_proposals(id) ON DELETE CASCADE,
    UNIQUE(proposal_id, round_number)
);

-- ============================================================================
-- INDEXES FOR PERFORMANCE
-- ============================================================================

-- Consensus Proposals Indexes
CREATE INDEX IF NOT EXISTS idx_proposals_view_seq ON consensus_proposals(view_number, sequence_number);
CREATE INDEX IF NOT EXISTS idx_proposals_status ON consensus_proposals(status);
CREATE INDEX IF NOT EXISTS idx_proposals_proposer ON consensus_proposals(proposer_agent);
CREATE INDEX IF NOT EXISTS idx_proposals_action ON consensus_proposals(action_type);
CREATE INDEX IF NOT EXISTS idx_proposals_created ON consensus_proposals(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_proposals_pending ON consensus_proposals(status) WHERE status IN ('pending', 'pre_prepare', 'prepare', 'commit');
CREATE INDEX IF NOT EXISTS idx_proposals_expired ON consensus_proposals(is_expired) WHERE is_expired = 1;
CREATE INDEX IF NOT EXISTS idx_proposals_priority ON consensus_proposals(priority DESC, is_urgent DESC);

-- Consensus Votes Indexes
CREATE INDEX IF NOT EXISTS idx_votes_proposal ON consensus_votes(proposal_id);
CREATE INDEX IF NOT EXISTS idx_votes_agent ON consensus_votes(agent_id);
CREATE INDEX IF NOT EXISTS idx_votes_phase ON consensus_votes(phase);
CREATE INDEX IF NOT EXISTS idx_votes_vote ON consensus_votes(vote);
CREATE INDEX IF NOT EXISTS idx_votes_timestamp ON consensus_votes(timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_votes_unverified ON consensus_votes(verified) WHERE verified = 0;

-- Consensus Outcomes Indexes
CREATE INDEX IF NOT EXISTS idx_outcomes_proposal ON consensus_outcomes(proposal_id);
CREATE INDEX IF NOT EXISTS idx_outcomes_decision ON consensus_outcomes(decision);
CREATE INDEX IF NOT EXISTS idx_outcomes_execution ON consensus_outcomes(execution_status);
CREATE INDEX IF NOT EXISTS idx_outcomes_timestamp ON consensus_outcomes(decision_timestamp DESC);

-- Dissent Events Indexes
CREATE INDEX IF NOT EXISTS idx_dissent_proposal ON dissent_events(proposal_id);
CREATE INDEX IF NOT EXISTS idx_dissent_agent ON dissent_events(dissenting_agent);
CREATE INDEX IF NOT EXISTS idx_dissent_category ON dissent_events(dissent_category);
CREATE INDEX IF NOT EXISTS idx_dissent_severity ON dissent_events(severity);
CREATE INDEX IF NOT EXISTS idx_dissent_outcome ON dissent_events(outcome);
CREATE INDEX IF NOT EXISTS idx_dissent_high_severity ON dissent_events(is_high_severity) WHERE is_high_severity = 1;
CREATE INDEX IF NOT EXISTS idx_dissent_pending ON dissent_events(outcome) WHERE outcome = 'pending';

-- Consensus Rounds Indexes
CREATE INDEX IF NOT EXISTS idx_rounds_proposal ON consensus_rounds(proposal_id);
CREATE INDEX IF NOT EXISTS idx_rounds_status ON consensus_rounds(status);

-- ============================================================================
-- TRIGGERS FOR AUTOMATIC UPDATES
-- ============================================================================

-- Update dissent_events.updated_at on modification
CREATE TRIGGER IF NOT EXISTS trg_dissent_updated_at
AFTER UPDATE ON dissent_events
BEGIN
    UPDATE dissent_events SET updated_at = CURRENT_TIMESTAMP WHERE id = NEW.id;
END;

-- Auto-create dissent event for reject votes with reasons
CREATE TRIGGER IF NOT EXISTS trg_create_dissent_on_reject
AFTER INSERT ON consensus_votes
WHEN NEW.vote = 'reject' AND NEW.reason IS NOT NULL
BEGIN
    INSERT INTO dissent_events (
        proposal_id, vote_id, dissenting_agent, agent_tier, agent_weight,
        proposed_action, dissent_reason, outcome
    )
    SELECT
        NEW.proposal_id,
        NEW.id,
        NEW.agent_id,
        NEW.agent_tier,
        NEW.agent_weight,
        cp.action_type,
        NEW.reason,
        'pending'
    FROM consensus_proposals cp
    WHERE cp.id = NEW.proposal_id;
END;

-- Auto-update proposal status based on votes
CREATE TRIGGER IF NOT EXISTS trg_check_quorum_on_vote
AFTER INSERT ON consensus_votes
WHEN NEW.phase = 'commit'
BEGIN
    UPDATE consensus_proposals
    SET status = 'commit'
    WHERE id = NEW.proposal_id
    AND status = 'prepare'
    AND (
        SELECT COUNT(*) FROM consensus_votes
        WHERE proposal_id = NEW.proposal_id
        AND phase = 'commit'
        AND vote = 'approve'
    ) >= quorum_size;
END;

-- Auto-set deadline on proposal creation
CREATE TRIGGER IF NOT EXISTS trg_set_proposal_deadline
AFTER INSERT ON consensus_proposals
BEGIN
    UPDATE consensus_proposals
    SET deadline = datetime(created_at, '+' || (timeout_ms / 1000) || ' seconds')
    WHERE id = NEW.id AND deadline IS NULL;
END;

-- ============================================================================
-- WORKFLOW INTEGRATION TABLES
-- ============================================================================

-- Workflow Approval Requests (L2/L3 escalations requiring PBFT)
CREATE TABLE IF NOT EXISTS workflow_approval_requests (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    request_id TEXT NOT NULL UNIQUE DEFAULT (lower(hex(randomblob(16)))),

    -- Workflow reference
    workflow_instance_id TEXT NOT NULL,  -- References workflow_tracking.workflow_instances
    step_id TEXT,  -- References workflow_tracking.workflow_steps

    -- Approval tier
    approval_tier TEXT NOT NULL CHECK(approval_tier IN ('L1', 'L2', 'L3')),

    -- Consensus proposal link
    proposal_id INTEGER,  -- Links to consensus_proposals for L3

    -- Status
    status TEXT DEFAULT 'pending'
        CHECK(status IN ('pending', 'voting', 'approved', 'rejected', 'timeout', 'cancelled')),

    -- Timing
    requested_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    deadline_at DATETIME,
    resolved_at DATETIME,

    -- Resolution
    resolved_by TEXT,  -- agent_id or 'consensus'
    resolution_reason TEXT,

    FOREIGN KEY (proposal_id) REFERENCES consensus_proposals(id)
);

-- Consensus-Workflow History (tracks all consensus decisions affecting workflows)
CREATE TABLE IF NOT EXISTS consensus_workflow_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    history_id TEXT NOT NULL UNIQUE DEFAULT (lower(hex(randomblob(16)))),

    -- References
    proposal_id INTEGER NOT NULL,
    outcome_id INTEGER,
    workflow_instance_id TEXT NOT NULL,

    -- Decision details
    action_requested TEXT NOT NULL,
    decision TEXT NOT NULL CHECK(decision IN ('approved', 'rejected', 'timeout')),
    quorum_reached INTEGER NOT NULL,
    approval_percentage REAL,

    -- Impact
    workflow_action_taken TEXT,  -- What the workflow did after decision
    rollback_required INTEGER DEFAULT 0,

    -- Timing
    decision_timestamp DATETIME DEFAULT CURRENT_TIMESTAMP,

    FOREIGN KEY (proposal_id) REFERENCES consensus_proposals(id),
    FOREIGN KEY (outcome_id) REFERENCES consensus_outcomes(id)
);

-- Indexes for workflow integration
CREATE INDEX IF NOT EXISTS idx_workflow_approval_workflow ON workflow_approval_requests(workflow_instance_id);
CREATE INDEX IF NOT EXISTS idx_workflow_approval_status ON workflow_approval_requests(status);
CREATE INDEX IF NOT EXISTS idx_workflow_approval_pending ON workflow_approval_requests(status) WHERE status = 'pending';
CREATE INDEX IF NOT EXISTS idx_consensus_workflow_proposal ON consensus_workflow_history(proposal_id);
CREATE INDEX IF NOT EXISTS idx_consensus_workflow_instance ON consensus_workflow_history(workflow_instance_id);

-- ============================================================================
-- CROSS-DATABASE VIEWS
-- ============================================================================

-- Pending approvals summary for workflow engine
CREATE VIEW IF NOT EXISTS v_pending_approvals AS
SELECT
    war.request_id,
    war.workflow_instance_id,
    war.approval_tier,
    war.status,
    war.requested_at,
    war.deadline_at,
    (julianday(war.deadline_at) - julianday('now')) * 86400 AS seconds_remaining,
    cp.action_type,
    cp.proposer_agent,
    cp.priority,
    cp.is_urgent,
    (SELECT COUNT(*) FROM consensus_votes cv
     WHERE cv.proposal_id = war.proposal_id
     AND cv.phase = 'prepare') AS prepare_votes,
    (SELECT COUNT(*) FROM consensus_votes cv
     WHERE cv.proposal_id = war.proposal_id
     AND cv.phase = 'commit') AS commit_votes
FROM workflow_approval_requests war
LEFT JOIN consensus_proposals cp ON war.proposal_id = cp.id
WHERE war.status IN ('pending', 'voting')
ORDER BY war.deadline_at;

-- Consensus effectiveness by workflow type
CREATE VIEW IF NOT EXISTS v_consensus_workflow_effectiveness AS
SELECT
    cwh.action_requested,
    COUNT(*) AS total_decisions,
    SUM(CASE WHEN cwh.decision = 'approved' THEN 1 ELSE 0 END) AS approved_count,
    SUM(CASE WHEN cwh.decision = 'rejected' THEN 1 ELSE 0 END) AS rejected_count,
    SUM(CASE WHEN cwh.decision = 'timeout' THEN 1 ELSE 0 END) AS timeout_count,
    AVG(cwh.approval_percentage) AS avg_approval_pct,
    SUM(CASE WHEN cwh.rollback_required = 1 THEN 1 ELSE 0 END) AS rollback_count
FROM consensus_workflow_history cwh
WHERE cwh.decision_timestamp > datetime('now', '-30 days')
GROUP BY cwh.action_requested
ORDER BY total_decisions DESC;

-- Dissent patterns for workflow learning
CREATE VIEW IF NOT EXISTS v_workflow_dissent_patterns AS
SELECT
    de.dissent_category,
    de.severity,
    cwh.action_requested AS workflow_action,
    COUNT(*) AS dissent_count,
    AVG(CASE WHEN de.was_correct = 1 THEN 1.0 ELSE 0.0 END) AS avg_correctness,
    SUM(CASE WHEN de.learning_incorporated = 1 THEN 1 ELSE 0 END) AS learning_applied
FROM dissent_events de
JOIN consensus_proposals cp ON de.proposal_id = cp.id
JOIN consensus_workflow_history cwh ON cwh.proposal_id = cp.id
WHERE de.created_at > datetime('now', '-30 days')
GROUP BY de.dissent_category, de.severity, cwh.action_requested
ORDER BY dissent_count DESC;

-- Active consensus rounds for dashboard
CREATE VIEW IF NOT EXISTS v_active_consensus AS
SELECT
    cp.proposal_id,
    cp.action_type,
    cp.status,
    cp.phase,
    cp.priority,
    cp.is_urgent,
    cp.created_at,
    cp.deadline,
    cp.quorum_size,
    (SELECT COUNT(*) FROM consensus_votes cv WHERE cv.proposal_id = cp.id AND cv.vote = 'approve') AS approve_votes,
    (SELECT COUNT(*) FROM consensus_votes cv WHERE cv.proposal_id = cp.id AND cv.vote = 'reject') AS reject_votes,
    war.workflow_instance_id,
    war.approval_tier
FROM consensus_proposals cp
LEFT JOIN workflow_approval_requests war ON war.proposal_id = cp.id
WHERE cp.status IN ('pending', 'pre_prepare', 'prepare', 'commit')
ORDER BY cp.is_urgent DESC, cp.priority DESC, cp.created_at;

-- ============================================================================
-- WORKFLOW INTEGRATION TRIGGERS
-- ============================================================================

-- Auto-create workflow approval request when L3 proposal is created
CREATE TRIGGER IF NOT EXISTS trg_create_workflow_approval_on_proposal
AFTER INSERT ON consensus_proposals
WHEN NEW.action_type IN ('emergency_response', 'maintenance', 'rollback', 'recovery')
BEGIN
    INSERT INTO workflow_approval_requests (
        workflow_instance_id, approval_tier, proposal_id, deadline_at
    )
    VALUES (
        json_extract(NEW.action_data, '$.workflow_instance_id'),
        'L3',
        NEW.id,
        NEW.deadline
    );
END;

-- Update workflow approval status when consensus outcome is reached
CREATE TRIGGER IF NOT EXISTS trg_update_workflow_approval_on_outcome
AFTER INSERT ON consensus_outcomes
BEGIN
    UPDATE workflow_approval_requests
    SET status = CASE NEW.decision
            WHEN 'approved' THEN 'approved'
            WHEN 'rejected' THEN 'rejected'
            ELSE 'timeout'
        END,
        resolved_at = CURRENT_TIMESTAMP,
        resolved_by = 'consensus'
    WHERE proposal_id = NEW.proposal_id;

    INSERT INTO consensus_workflow_history (
        proposal_id, outcome_id, workflow_instance_id,
        action_requested, decision, quorum_reached, approval_percentage
    )
    SELECT
        NEW.proposal_id,
        NEW.id,
        war.workflow_instance_id,
        cp.action_type,
        NEW.decision,
        NEW.quorum_reached,
        NEW.approval_percentage
    FROM workflow_approval_requests war
    JOIN consensus_proposals cp ON war.proposal_id = cp.id
    WHERE war.proposal_id = NEW.proposal_id;
END;

-- ============================================================================
-- End of Migration 004_consensus_tracking.sql
-- ============================================================================
