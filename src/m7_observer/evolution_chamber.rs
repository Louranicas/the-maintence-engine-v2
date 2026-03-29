//! # M39: Evolution Chamber
//!
//! RALPH 5-phase meta-learning loop for system parameter evolution.
//! Manages the full mutation lifecycle: proposal, application,
//! verification, and acceptance or rollback, driven by fitness
//! evaluation from the Fitness Evaluator utility.
//!
//! ## Layer: L7 (Observer)
//! ## Lock Order: 4 (after `EmergenceDetector`)
//! ## Dependencies: M01 (Error), Fitness Evaluator (utility)
//!
//! ## RALPH Phases
//!
//! | Phase | Purpose |
//! |-------|---------|
//! | Recognize | Identify parameters drifting from targets |
//! | Analyze | Compute deltas and rank candidates |
//! | Learn | Extract patterns from mutation history |
//! | Propose | Generate bounded mutations |
//! | Harvest | Accept beneficial mutations, rollback harmful ones |
//!
//! ## Related Documentation
//! - [Evolution Chamber Spec](../../ai_specs/evolution_chamber_ai_specs/EVOLUTION_CHAMBER_SPEC.md)
//! - [RALPH Spec](../../ai_specs/evolution_chamber_ai_specs/RALPH_SPEC.md)
//! - [Layer Specification](../../ai_docs/layers/L07_OBSERVER.md)

use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{Error, Result};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Default maximum concurrent mutations.
const DEFAULT_MAX_CONCURRENT_MUTATIONS: u32 = 3;

/// Default mutation verification timeout in milliseconds.
const DEFAULT_MUTATION_VERIFICATION_MS: u64 = 30_000;

/// Default fitness snapshot history capacity for M39.
/// Distinct from `FitnessConfig.history_capacity` (200), which sizes the
/// `FitnessEvaluator`'s internal `FitnessReport` buffer.
const DEFAULT_FITNESS_HISTORY_CAPACITY: usize = 500;

/// Default mutation history capacity.
const DEFAULT_MUTATION_HISTORY_CAPACITY: usize = 1000;

/// Default auto-apply fitness delta threshold.
const DEFAULT_AUTO_APPLY_THRESHOLD: f64 = 0.10;

/// Default rollback fitness delta threshold.
const DEFAULT_ROLLBACK_THRESHOLD: f64 = -0.02;

/// Default minimum generation interval in milliseconds.
const DEFAULT_MIN_GENERATION_INTERVAL_MS: u64 = 60_000;

/// Default maximum mutation delta magnitude.
const DEFAULT_MAX_MUTATION_DELTA: f64 = 0.20;

/// Default evolution tick interval in milliseconds (15s for V2, was 60s in V1).
/// Used by the observer tick configuration to set V2's faster iteration rate.
pub const DEFAULT_EVOLUTION_TICK_MS: u64 = 15_000;

/// Convergence detection: pause when fitness variance below this for 50 gens.
const CONVERGENCE_VARIANCE_THRESHOLD: f64 = 0.001;

/// Convergence detection: number of generations to check.
const CONVERGENCE_WINDOW: usize = 50;

// ---------------------------------------------------------------------------
// V2 Enhanced Enums
// ---------------------------------------------------------------------------

/// R19: Evolution strategy selection based on system state.
///
/// The chamber selects a strategy each tick based on fitness trajectory,
/// layer health, and field coherence. This replaces V1's single-mode
/// mutation approach with adaptive multi-strategy evolution.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum EvolutionStrategy {
    /// Single parameter, conservative delta (fitness > 0.7, stable)
    Conservative,
    /// Multi-parameter, larger delta, higher rollback tolerance (fitness < 0.5)
    Exploratory,
    /// Target known structural deficits (layer health < 0.5)
    StructuralRepair,
    /// Narrow search around current optimum (fitness > 0.8, variance < 0.01)
    Convergence,
    /// Field-driven adaptation (`|r_delta|` > 0.05)
    Morphogenic,
}

impl std::fmt::Display for EvolutionStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Conservative => f.write_str("Conservative"),
            Self::Exploratory => f.write_str("Exploratory"),
            Self::StructuralRepair => f.write_str("StructuralRepair"),
            Self::Convergence => f.write_str("Convergence"),
            Self::Morphogenic => f.write_str("Morphogenic"),
        }
    }
}

/// R19: Source of a mutation hint from the 4-source Learn phase.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum HintSource {
    /// Source 1: Urgent system event (emergence detection)
    Emergence(String),
    /// Source 2: Weakest fitness dimension (tensor analysis)
    DimensionAnalysis(String),
    /// Source 3: Historical correlation pathway
    EstablishedPathway(String),
    /// Source 4 (NEW): Code bug, not tunable — layer health < 0.5
    StructuralDeficit(String),
}

impl std::fmt::Display for HintSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Emergence(s) => write!(f, "emergence:{s}"),
            Self::DimensionAnalysis(s) => write!(f, "dimension:{s}"),
            Self::EstablishedPathway(s) => write!(f, "pathway:{s}"),
            Self::StructuralDeficit(s) => write!(f, "structural:{s}"),
        }
    }
}

/// R19: Hint from the Learn phase guiding mutation selection.
///
/// Instead of blind round-robin parameter selection (V1), hints
/// direct the Propose phase toward parameters that address actual
/// observed problems.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MutationHint {
    /// Parameter name to target.
    pub parameter: String,
    /// Which Learn source generated this hint.
    pub source: HintSource,
    /// Confidence in this hint (0.0-1.0).
    pub confidence: f64,
    /// Human-readable reason for the hint.
    pub reason: String,
}

/// R19: Gate decision from N05 Evolution Gate.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum GateDecision {
    /// Mutation passed field coherence test.
    Accept {
        /// Field r after shadow test.
        r_after: f64,
        /// Confidence in acceptance.
        confidence: f64,
    },
    /// Mutation failed field coherence test.
    Reject {
        /// Reason for rejection.
        reason: String,
        /// r delta observed during test.
        r_delta: f64,
    },
    /// Escalate to PBFT consensus for fleet-wide decision.
    DeferToConsensus {
        /// PBFT proposal ID.
        proposal_id: String,
    },
}

// ---------------------------------------------------------------------------
// Enums
// ---------------------------------------------------------------------------

/// Status of a mutation through its lifecycle.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum MutationStatus {
    /// Mutation has been proposed but not yet applied.
    Proposed,
    /// Mutation has been applied and is being verified.
    Verifying,
    /// Mutation was verified and accepted (fitness improved).
    Accepted,
    /// Mutation was rolled back (fitness regressed).
    RolledBack,
    /// Mutation failed during application or verification.
    Failed,
}

/// A phase in the RALPH meta-learning loop.
///
/// The five phases cycle in order: Recognize -> Analyze -> Learn ->
/// Propose -> Harvest, then back to Recognize.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum RalphPhase {
    /// Identify parameters drifting from targets.
    Recognize,
    /// Compute deltas and rank candidates.
    Analyze,
    /// Extract patterns from mutation history.
    Learn,
    /// Generate bounded mutations.
    Propose,
    /// Accept beneficial mutations, rollback harmful ones.
    Harvest,
}

impl RalphPhase {
    /// Returns the next phase in the RALPH cycle.
    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Self::Recognize => Self::Analyze,
            Self::Analyze => Self::Learn,
            Self::Learn => Self::Propose,
            Self::Propose => Self::Harvest,
            Self::Harvest => Self::Recognize,
        }
    }

    /// Returns the zero-indexed ordinal of this phase (0-4).
    #[must_use]
    pub const fn ordinal(self) -> u8 {
        match self {
            Self::Recognize => 0,
            Self::Analyze => 1,
            Self::Learn => 2,
            Self::Propose => 3,
            Self::Harvest => 4,
        }
    }

    /// Returns the phase name as a static string.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Recognize => "Recognize",
            Self::Analyze => "Analyze",
            Self::Learn => "Learn",
            Self::Propose => "Propose",
            Self::Harvest => "Harvest",
        }
    }
}

impl std::fmt::Display for RalphPhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

impl std::fmt::Display for MutationStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Proposed => f.write_str("Proposed"),
            Self::Verifying => f.write_str("Verifying"),
            Self::Accepted => f.write_str("Accepted"),
            Self::RolledBack => f.write_str("RolledBack"),
            Self::Failed => f.write_str("Failed"),
        }
    }
}

// ---------------------------------------------------------------------------
// Data structures
// ---------------------------------------------------------------------------

/// A completed mutation record with before/after fitness measurements.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MutationRecord {
    /// Unique mutation ID (UUID v4).
    pub id: String,
    /// Generation in which this mutation was proposed.
    pub generation: u64,
    /// RALPH phase during which this mutation was created.
    pub source_phase: RalphPhase,
    /// Name of the parameter being mutated.
    pub target_parameter: String,
    /// Original parameter value before mutation.
    pub original_value: f64,
    /// Mutated parameter value.
    pub mutated_value: f64,
    /// Signed delta (`mutated_value - original_value`).
    pub delta: f64,
    /// Fitness score before the mutation was applied.
    pub fitness_before: f64,
    /// Fitness score after verification (0.0 if not yet verified).
    pub fitness_after: f64,
    /// Whether the mutation was applied to the live system.
    pub applied: bool,
    /// Whether the mutation was subsequently rolled back.
    pub rolled_back: bool,
    /// Timestamp of record creation.
    pub timestamp: DateTime<Utc>,
    /// Verification latency in milliseconds (0 if not verified).
    pub verification_ms: u64,
}

/// An in-flight mutation that has been proposed or is being verified.
#[derive(Clone, Debug)]
pub struct ActiveMutation {
    /// Unique mutation ID (UUID v4).
    pub id: String,
    /// Generation in which this mutation was created.
    pub generation: u64,
    /// Parameter targeted by this mutation.
    pub target_parameter: String,
    /// Original parameter value.
    pub original_value: f64,
    /// Value applied by the mutation.
    pub applied_value: f64,
    /// Fitness score at the time this mutation was proposed.
    pub fitness_at_proposal: f64,
    /// Timestamp when the mutation was applied.
    pub applied_at: DateTime<Utc>,
    /// Deadline by which verification must complete.
    pub verification_deadline: DateTime<Utc>,
    /// Current status of the mutation.
    pub status: MutationStatus,
}

/// Mutable state of the RALPH meta-learning loop.
#[derive(Clone, Debug)]
pub struct RalphState {
    /// Current active phase.
    pub current_phase: RalphPhase,
    /// Monotonically increasing cycle number.
    pub cycle_number: u64,
    /// When the current cycle was started (`None` if not started).
    pub cycle_started_at: Option<DateTime<Utc>>,
    /// When the current cycle was completed (`None` if still running).
    pub cycle_completed_at: Option<DateTime<Utc>>,
    /// Number of mutations proposed in the current cycle.
    pub mutations_proposed: u32,
    /// Number of mutations applied in the current cycle.
    pub mutations_applied: u32,
    /// Whether the RALPH loop is paused.
    pub paused: bool,
}

/// A point-in-time fitness measurement with tensor state.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FitnessSnapshot {
    /// Snapshot timestamp.
    pub timestamp: DateTime<Utc>,
    /// Overall fitness score.
    pub fitness: f64,
    /// Full 12D tensor state at the time of measurement.
    pub tensor: [f64; 12],
    /// Generation number, if during an active evolution cycle.
    pub generation: Option<u64>,
}

/// Configuration for the Evolution Chamber.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EvolutionChamberConfig {
    /// Maximum number of mutations active simultaneously.
    pub max_concurrent_mutations: u32,
    /// Verification timeout in milliseconds per mutation.
    pub mutation_verification_ms: u64,
    /// M39's internal `FitnessSnapshot` buffer capacity (default 500).
    /// Distinct from `FitnessConfig.history_capacity` (200).
    pub fitness_history_capacity: usize,
    /// Maximum retained mutation history records.
    pub mutation_history_capacity: usize,
    /// Fitness delta at or above which a mutation is auto-applied.
    pub auto_apply_threshold: f64,
    /// Fitness delta at or below which a mutation is auto-rolled-back.
    pub rollback_threshold: f64,
    /// Minimum milliseconds between generation advances.
    pub min_generation_interval_ms: u64,
    /// Maximum absolute delta magnitude for a single mutation.
    pub max_mutation_delta: f64,
}

impl Default for EvolutionChamberConfig {
    fn default() -> Self {
        Self {
            max_concurrent_mutations: DEFAULT_MAX_CONCURRENT_MUTATIONS,
            mutation_verification_ms: DEFAULT_MUTATION_VERIFICATION_MS,
            fitness_history_capacity: DEFAULT_FITNESS_HISTORY_CAPACITY,
            mutation_history_capacity: DEFAULT_MUTATION_HISTORY_CAPACITY,
            auto_apply_threshold: DEFAULT_AUTO_APPLY_THRESHOLD,
            rollback_threshold: DEFAULT_ROLLBACK_THRESHOLD,
            min_generation_interval_ms: DEFAULT_MIN_GENERATION_INTERVAL_MS,
            max_mutation_delta: DEFAULT_MAX_MUTATION_DELTA,
        }
    }
}

/// Aggregate statistics for the Evolution Chamber.
#[derive(Clone, Debug, Default)]
pub struct ChamberStats {
    /// Total mutations proposed since creation.
    pub total_mutations_proposed: u64,
    /// Total mutations successfully applied.
    pub total_mutations_applied: u64,
    /// Total mutations rolled back.
    pub total_mutations_rolled_back: u64,
    /// Total RALPH cycles completed.
    pub total_ralph_cycles: u64,
    /// Current generation number.
    pub current_generation: u64,
    /// Current RALPH phase, if a cycle is active.
    pub current_phase: Option<RalphPhase>,
}

// ---------------------------------------------------------------------------
// EvolutionChamber
// ---------------------------------------------------------------------------

/// M39 Evolution Chamber: RALPH 5-phase meta-learning loop.
///
/// Manages system parameter evolution through bounded mutations,
/// fitness-based verification, and automatic accept/rollback decisions.
///
/// # Thread Safety
///
/// All mutable state is protected by `parking_lot::RwLock`.
///
/// # Lock Order
///
/// Lock order 4 (after `EmergenceDetector`).
pub struct EvolutionChamber {
    /// Current generation number (monotonically increasing).
    generation: RwLock<u64>,
    /// In-flight mutations awaiting verification.
    active_mutations: RwLock<Vec<ActiveMutation>>,
    /// Historical mutation records (bounded ring buffer).
    mutation_history: RwLock<Vec<MutationRecord>>,
    /// RALPH meta-learning loop state.
    ralph_state: RwLock<RalphState>,
    /// Fitness snapshot history (bounded ring buffer).
    fitness_snapshots: RwLock<Vec<FitnessSnapshot>>,
    /// Immutable configuration.
    config: EvolutionChamberConfig,
    /// Aggregate statistics.
    stats: RwLock<ChamberStats>,
}

impl EvolutionChamber {
    /// Creates a new `EvolutionChamber` with default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self::with_config(EvolutionChamberConfig::default())
    }

    /// Creates a new `EvolutionChamber` with the given configuration.
    #[must_use]
    pub fn with_config(config: EvolutionChamberConfig) -> Self {
        Self {
            generation: RwLock::new(0),
            active_mutations: RwLock::new(Vec::new()),
            mutation_history: RwLock::new(Vec::with_capacity(
                config.mutation_history_capacity.min(1024),
            )),
            ralph_state: RwLock::new(RalphState {
                current_phase: RalphPhase::Recognize,
                cycle_number: 0,
                cycle_started_at: None,
                cycle_completed_at: None,
                mutations_proposed: 0,
                mutations_applied: 0,
                paused: false,
            }),
            fitness_snapshots: RwLock::new(Vec::with_capacity(
                config.fitness_history_capacity.min(1024),
            )),
            stats: RwLock::new(ChamberStats::default()),
            config,
        }
    }

    /// Validates an `EvolutionChamberConfig` for internal consistency.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if any configuration parameter is
    /// out of its acceptable range.
    pub fn validate_config(config: &EvolutionChamberConfig) -> Result<()> {
        if config.max_concurrent_mutations == 0 {
            return Err(Error::Validation(
                "max_concurrent_mutations must be > 0".into(),
            ));
        }
        if config.mutation_verification_ms == 0 {
            return Err(Error::Validation(
                "mutation_verification_ms must be > 0".into(),
            ));
        }
        if config.fitness_history_capacity == 0 {
            return Err(Error::Validation(
                "fitness_history_capacity must be > 0".into(),
            ));
        }
        if config.mutation_history_capacity == 0 {
            return Err(Error::Validation(
                "mutation_history_capacity must be > 0".into(),
            ));
        }
        if config.auto_apply_threshold < 0.0 || config.auto_apply_threshold > 1.0 {
            return Err(Error::Validation(
                "auto_apply_threshold must be in [0.0, 1.0]".into(),
            ));
        }
        if config.rollback_threshold > 0.0 {
            return Err(Error::Validation(
                "rollback_threshold must be <= 0.0".into(),
            ));
        }
        if config.max_mutation_delta <= 0.0 || config.max_mutation_delta > 1.0 {
            return Err(Error::Validation(
                "max_mutation_delta must be in (0.0, 1.0]".into(),
            ));
        }
        if config.min_generation_interval_ms == 0 {
            return Err(Error::Validation(
                "min_generation_interval_ms must be > 0".into(),
            ));
        }
        Ok(())
    }

    // ----- Mutation lifecycle -----

    /// Proposes a new mutation targeting a named parameter.
    ///
    /// The mutation is recorded but **not** applied until
    /// [`apply_mutation`](Self::apply_mutation) is called.
    ///
    /// # Errors
    ///
    /// - `Error::Validation` if the target parameter name is empty.
    /// - `Error::Validation` if the delta exceeds `max_mutation_delta`.
    /// - `Error::Validation` if the concurrent mutation limit is reached.
    pub fn propose_mutation(
        &self,
        target: &str,
        original: f64,
        mutated: f64,
        fitness_before: f64,
    ) -> Result<MutationRecord> {
        if target.is_empty() {
            return Err(Error::Validation(
                "target parameter name must not be empty".into(),
            ));
        }

        let delta = mutated - original;
        if delta.abs() > self.config.max_mutation_delta {
            return Err(Error::Validation(format!(
                "mutation delta {delta:.6} exceeds max_mutation_delta {}",
                self.config.max_mutation_delta
            )));
        }

        #[allow(clippy::cast_possible_truncation)]
        let active_count = self.active_mutations.read().len() as u32;
        if active_count >= self.config.max_concurrent_mutations {
            return Err(Error::Validation(format!(
                "concurrent mutation limit reached ({}/{})",
                active_count, self.config.max_concurrent_mutations
            )));
        }

        let generation = *self.generation.read();
        let phase = self.ralph_state.read().current_phase;
        let now = Utc::now();

        let record = MutationRecord {
            id: Uuid::new_v4().to_string(),
            generation,
            source_phase: phase,
            target_parameter: target.to_string(),
            original_value: original,
            mutated_value: mutated,
            delta,
            fitness_before,
            fitness_after: 0.0,
            applied: false,
            rolled_back: false,
            timestamp: now,
            verification_ms: 0,
        };

        // Register as active mutation
        {
            let verification_deadline = now
                + chrono::Duration::milliseconds(
                    i64::try_from(self.config.mutation_verification_ms).unwrap_or(30_000),
                );

            let active = ActiveMutation {
                id: record.id.clone(),
                generation,
                target_parameter: target.to_string(),
                original_value: original,
                applied_value: mutated,
                fitness_at_proposal: fitness_before,
                applied_at: now,
                verification_deadline,
                status: MutationStatus::Proposed,
            };
            self.active_mutations.write().push(active);
        }

        // Update stats
        {
            let mut stats = self.stats.write();
            stats.total_mutations_proposed += 1;
        }

        // Update RALPH state
        {
            let mut rs = self.ralph_state.write();
            rs.mutations_proposed += 1;
        }

        Ok(record)
    }

    /// Applies a previously proposed mutation, transitioning it to
    /// the `Verifying` state.
    ///
    /// # Errors
    ///
    /// - `Error::Validation` if the mutation ID is not found or is not
    ///   in the `Proposed` state.
    pub fn apply_mutation(&self, mutation_id: &str) -> Result<()> {
        let mut active = self.active_mutations.write();
        let mutation = active
            .iter_mut()
            .find(|m| m.id == mutation_id)
            .ok_or_else(|| {
                Error::Validation(format!("active mutation '{mutation_id}' not found"))
            })?;

        if mutation.status != MutationStatus::Proposed {
            return Err(Error::Validation(format!(
                "mutation '{mutation_id}' is in state {} (expected Proposed)",
                mutation.status
            )));
        }

        mutation.status = MutationStatus::Verifying;
        mutation.applied_at = Utc::now();

        // Update stats
        drop(active);
        {
            let mut stats = self.stats.write();
            stats.total_mutations_applied += 1;
        }
        {
            let mut rs = self.ralph_state.write();
            rs.mutations_applied += 1;
        }

        Ok(())
    }

    /// Verifies a mutation against a post-application fitness score.
    ///
    /// The mutation is moved from `active_mutations` into
    /// `mutation_history` with the `Accepted` status.
    ///
    /// # Errors
    ///
    /// - `Error::Validation` if the mutation ID is not found or is not
    ///   in the `Verifying` state.
    pub fn verify_mutation(
        &self,
        mutation_id: &str,
        fitness_after: f64,
    ) -> Result<MutationRecord> {
        let now = Utc::now();

        let removed = {
            let mut active = self.active_mutations.write();
            let idx = active
                .iter()
                .position(|m| m.id == mutation_id)
                .ok_or_else(|| {
                    Error::Validation(format!("active mutation '{mutation_id}' not found"))
                })?;

            if active[idx].status != MutationStatus::Verifying {
                return Err(Error::Validation(format!(
                    "mutation '{mutation_id}' is in state {} (expected Verifying)",
                    active[idx].status
                )));
            }

            active.remove(idx)
        };

        let verification_ms = {
            let elapsed = now - removed.applied_at;
            u64::try_from(elapsed.num_milliseconds().max(0)).unwrap_or(0)
        };

        let record = MutationRecord {
            id: removed.id,
            generation: removed.generation,
            source_phase: self.ralph_state.read().current_phase,
            target_parameter: removed.target_parameter,
            original_value: removed.original_value,
            mutated_value: removed.applied_value,
            delta: removed.applied_value - removed.original_value,
            fitness_before: removed.fitness_at_proposal,
            fitness_after,
            applied: true,
            rolled_back: false,
            timestamp: now,
            verification_ms,
        };

        self.push_mutation_history(record.clone());

        Ok(record)
    }

    /// Rolls back a mutation, returning the parameter to its original
    /// value.
    ///
    /// The mutation is moved from `active_mutations` into
    /// `mutation_history` with the `RolledBack` status.
    ///
    /// # Errors
    ///
    /// - `Error::Validation` if the mutation ID is not found among
    ///   active mutations.
    pub fn rollback_mutation(&self, mutation_id: &str) -> Result<MutationRecord> {
        let now = Utc::now();

        let removed = {
            let mut active = self.active_mutations.write();
            let idx = active
                .iter()
                .position(|m| m.id == mutation_id)
                .ok_or_else(|| {
                    Error::Validation(format!("active mutation '{mutation_id}' not found"))
                })?;

            active.remove(idx)
        };

        let verification_ms = {
            let elapsed = now - removed.applied_at;
            u64::try_from(elapsed.num_milliseconds().max(0)).unwrap_or(0)
        };

        let record = MutationRecord {
            id: removed.id,
            generation: removed.generation,
            source_phase: self.ralph_state.read().current_phase,
            target_parameter: removed.target_parameter,
            original_value: removed.original_value,
            mutated_value: removed.applied_value,
            delta: removed.applied_value - removed.original_value,
            fitness_before: removed.fitness_at_proposal,
            fitness_after: removed.fitness_at_proposal,
            applied: true,
            rolled_back: true,
            timestamp: now,
            verification_ms,
        };

        // Update stats
        {
            let mut stats = self.stats.write();
            stats.total_mutations_rolled_back += 1;
        }

        self.push_mutation_history(record.clone());

        Ok(record)
    }

    /// Pushes a mutation record into bounded history.
    fn push_mutation_history(&self, record: MutationRecord) {
        let mut history = self.mutation_history.write();
        if history.len() >= self.config.mutation_history_capacity {
            history.remove(0);
        }
        history.push(record);
    }

    // ----- RALPH cycle management -----

    /// Advances the RALPH phase to the next in the cycle.
    ///
    /// # Errors
    ///
    /// - `Error::Validation` if the RALPH loop is paused.
    /// - `Error::Validation` if no cycle is currently started.
    pub fn advance_phase(&self) -> Result<RalphPhase> {
        let mut rs = self.ralph_state.write();
        if rs.paused {
            return Err(Error::Validation(
                "cannot advance phase: RALPH loop is paused".into(),
            ));
        }
        if rs.cycle_started_at.is_none() {
            return Err(Error::Validation(
                "cannot advance phase: no active cycle (call start_cycle first)".into(),
            ));
        }
        let next = rs.current_phase.next();
        rs.current_phase = next;
        drop(rs);
        Ok(next)
    }

    /// Starts a new RALPH cycle, resetting per-cycle counters and
    /// incrementing the cycle number.
    ///
    /// # Errors
    ///
    /// - `Error::Validation` if a cycle is already in progress (started
    ///   but not completed).
    /// - `Error::Validation` if the loop is paused.
    pub fn start_cycle(&self) -> Result<u64> {
        let mut rs = self.ralph_state.write();
        if rs.paused {
            return Err(Error::Validation(
                "cannot start cycle: RALPH loop is paused".into(),
            ));
        }
        if rs.cycle_started_at.is_some() && rs.cycle_completed_at.is_none() {
            return Err(Error::Validation(
                "cycle already in progress; complete it before starting a new one".into(),
            ));
        }
        rs.cycle_number += 1;
        rs.current_phase = RalphPhase::Recognize;
        rs.cycle_started_at = Some(Utc::now());
        rs.cycle_completed_at = None;
        rs.mutations_proposed = 0;
        rs.mutations_applied = 0;

        let cycle = rs.cycle_number;
        drop(rs);

        // Advance generation
        {
            let mut gen = self.generation.write();
            *gen += 1;
        }

        // Update stats
        {
            let mut stats = self.stats.write();
            stats.current_generation = *self.generation.read();
            stats.current_phase = Some(RalphPhase::Recognize);
        }

        Ok(cycle)
    }

    /// Completes the current RALPH cycle.
    ///
    /// # Errors
    ///
    /// - `Error::Validation` if no cycle is currently started.
    pub fn complete_cycle(&self) -> Result<()> {
        let mut rs = self.ralph_state.write();
        if rs.cycle_started_at.is_none() {
            return Err(Error::Validation(
                "no cycle in progress to complete".into(),
            ));
        }
        rs.cycle_completed_at = Some(Utc::now());

        drop(rs);

        {
            let mut stats = self.stats.write();
            stats.total_ralph_cycles += 1;
            stats.current_phase = None;
        }

        Ok(())
    }

    /// Pauses the RALPH meta-learning loop.
    ///
    /// While paused, `advance_phase` and `start_cycle` will return
    /// errors.
    pub fn pause(&self) {
        self.ralph_state.write().paused = true;
    }

    /// Resumes the RALPH meta-learning loop after a pause.
    pub fn resume(&self) {
        self.ralph_state.write().paused = false;
    }

    // ----- Fitness tracking -----

    /// Records a fitness snapshot with its associated 12D tensor.
    ///
    /// The snapshot is stored in the bounded fitness history buffer.
    /// Returns the recorded snapshot.
    pub fn record_fitness(&self, fitness: f64, tensor: [f64; 12]) -> FitnessSnapshot {
        let generation = *self.generation.read();
        let snapshot = FitnessSnapshot {
            timestamp: Utc::now(),
            fitness,
            tensor,
            generation: if generation > 0 {
                Some(generation)
            } else {
                None
            },
        };

        {
            let mut snaps = self.fitness_snapshots.write();
            if snaps.len() >= self.config.fitness_history_capacity {
                snaps.remove(0);
            }
            snaps.push(snapshot.clone());
        }

        snapshot
    }

    // ----- Accessors (pure / read-only) -----

    /// Returns the current generation number.
    #[must_use]
    pub fn generation(&self) -> u64 {
        *self.generation.read()
    }

    /// Returns a clone of the current RALPH state.
    #[must_use]
    pub fn ralph_state(&self) -> RalphState {
        self.ralph_state.read().clone()
    }

    /// Returns the number of currently active (in-flight) mutations.
    #[must_use]
    pub fn active_mutation_count(&self) -> usize {
        self.active_mutations.read().len()
    }

    /// Returns a clone of all currently active mutations.
    #[must_use]
    pub fn active_mutations(&self) -> Vec<ActiveMutation> {
        self.active_mutations.read().clone()
    }

    /// Looks up an active mutation by ID.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if the mutation is not found.
    pub fn get_active_mutation(&self, id: &str) -> Result<ActiveMutation> {
        self.active_mutations
            .read()
            .iter()
            .find(|m| m.id == id)
            .cloned()
            .ok_or_else(|| Error::Validation(format!("active mutation '{id}' not found")))
    }

    /// Sets the generation counter (used for cognitive state restoration).
    pub fn set_generation(&self, gen: u64) {
        *self.generation.write() = gen;
    }

    /// Looks up a mutation record by ID in the history.
    #[must_use]
    pub fn get_mutation(&self, id: &str) -> Option<MutationRecord> {
        self.mutation_history
            .read()
            .iter()
            .find(|m| m.id == id)
            .cloned()
    }

    /// Returns the most recent `n` mutation records (newest last).
    #[must_use]
    pub fn recent_mutations(&self, n: usize) -> Vec<MutationRecord> {
        let history = self.mutation_history.read();
        let start = history.len().saturating_sub(n);
        history[start..].to_vec()
    }

    /// Returns the most recent `n` fitness snapshots (newest last).
    #[must_use]
    pub fn fitness_history(&self, n: usize) -> Vec<FitnessSnapshot> {
        let snaps = self.fitness_snapshots.read();
        let start = snaps.len().saturating_sub(n);
        snaps[start..].to_vec()
    }

    /// Returns a snapshot of aggregate chamber statistics.
    #[must_use]
    pub fn stats(&self) -> ChamberStats {
        let mut s = self.stats.read().clone();
        s.current_generation = *self.generation.read();
        s.current_phase = {
            let rs = self.ralph_state.read();
            if rs.cycle_started_at.is_some() && rs.cycle_completed_at.is_none() {
                Some(rs.current_phase)
            } else {
                None
            }
        };
        s
    }

    /// Returns whether the given fitness delta meets or exceeds the
    /// auto-apply threshold.
    #[must_use]
    pub fn should_auto_apply(&self, fitness_delta: f64) -> bool {
        fitness_delta >= self.config.auto_apply_threshold
    }

    /// Returns whether the given fitness delta meets or exceeds the
    /// rollback threshold (i.e., the delta is sufficiently negative).
    #[must_use]
    pub fn should_rollback(&self, fitness_delta: f64) -> bool {
        fitness_delta <= self.config.rollback_threshold
    }

    /// Clears all state: active mutations, history, fitness snapshots,
    /// and statistics. Resets generation to 0 and RALPH state to
    /// initial values.
    pub fn clear(&self) {
        self.active_mutations.write().clear();
        self.mutation_history.write().clear();
        self.fitness_snapshots.write().clear();
        *self.generation.write() = 0;
        *self.ralph_state.write() = RalphState {
            current_phase: RalphPhase::Recognize,
            cycle_number: 0,
            cycle_started_at: None,
            cycle_completed_at: None,
            mutations_proposed: 0,
            mutations_applied: 0,
            paused: false,
        };
        *self.stats.write() = ChamberStats::default();
    }

    /// Returns the immutable configuration.
    #[must_use]
    pub const fn config(&self) -> &EvolutionChamberConfig {
        &self.config
    }

    // ----- V2 Enhanced Methods (R19) -----

    /// R19: Select evolution strategy based on current system state.
    ///
    /// Examines fitness level, trajectory, layer health, and field
    /// coherence to choose the most appropriate mutation strategy.
    #[must_use]
    pub fn select_strategy(
        &self,
        fitness: f64,
        layer_health: &[f64],
        _r: f64,
        r_delta: f64,
    ) -> EvolutionStrategy {
        // Morphogenic: field-driven adaptation takes priority
        if r_delta.abs() > 0.05 {
            return EvolutionStrategy::Morphogenic;
        }

        // Structural repair: any layer below 0.5 indicates a code bug
        if layer_health.iter().skip(1).any(|&h| h < 0.5) {
            return EvolutionStrategy::StructuralRepair;
        }

        // Convergence: high fitness + low variance = narrow search
        if fitness > 0.8 {
            // Collect fitness values under lock, then drop lock before computation
            let recent: Vec<f64> = {
                let snaps = self.fitness_snapshots.read();
                if snaps.len() >= CONVERGENCE_WINDOW {
                    snaps
                        .iter()
                        .rev()
                        .take(CONVERGENCE_WINDOW)
                        .map(|s| s.fitness)
                        .collect()
                } else {
                    Vec::new()
                }
            };
            if !recent.is_empty() {
                #[allow(clippy::cast_precision_loss)]
                let count = recent.len() as f64;
                let mean = recent.iter().sum::<f64>() / count;
                let variance =
                    recent.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / count;
                if variance < CONVERGENCE_VARIANCE_THRESHOLD {
                    return EvolutionStrategy::Convergence;
                }
            }
        }

        // Exploratory: low fitness needs aggressive exploration
        if fitness < 0.5 {
            return EvolutionStrategy::Exploratory;
        }

        // Default: conservative single-parameter tuning
        EvolutionStrategy::Conservative
    }

    /// R19: 4-source Learn phase producing a mutation hint.
    ///
    /// Queries four sources in priority order:
    /// 1. Emergence events (urgent system signals)
    /// 2. Dimension analysis (weakest fitness tensor dimension)
    /// 3. Established pathways (historical correlations)
    /// 4. Structural deficit (layer health below 0.5)
    ///
    /// Returns `None` if no source produces a hint.
    #[must_use]
    pub fn learn_with_hints(
        &self,
        emergence_count: u64,
        weakest_dimension: Option<(usize, f64)>,
        layer_health: &[f64],
    ) -> Option<MutationHint> {
        // Source 1: Emergence events
        if emergence_count > 100 {
            return Some(MutationHint {
                parameter: "emergence_detector.min_confidence".to_string(),
                source: HintSource::Emergence(format!("{emergence_count} events")),
                confidence: 0.8,
                reason: format!("High emergence count ({emergence_count}) suggests detection threshold too sensitive"),
            });
        }

        // Source 2: Weakest fitness dimension
        if let Some((dim_idx, dim_val)) = weakest_dimension {
            if dim_val < 0.3 {
                let param = match dim_idx {
                    6 => "health_monitor.poll_interval_ms",
                    9 => "circuit_breaker.latency_threshold_ms",
                    10 => "service_registry.max_error_rate",
                    _ => "emergence_detector.min_confidence",
                };
                return Some(MutationHint {
                    parameter: param.to_string(),
                    source: HintSource::DimensionAnalysis(format!("D{dim_idx}={dim_val:.3}")),
                    confidence: 0.7,
                    reason: format!("Dimension D{dim_idx} at {dim_val:.3} is the weakest tensor component"),
                });
            }
        }

        // Source 3: Established pathway (use mutation success rate as proxy)
        let history = self.mutation_history.read();
        let recent_successes: Vec<_> = history
            .iter()
            .rev()
            .take(10)
            .filter(|m| !m.rolled_back && m.applied)
            .collect();
        if let Some(best) = recent_successes.first() {
            return Some(MutationHint {
                parameter: best.target_parameter.clone(),
                source: HintSource::EstablishedPathway(format!(
                    "successful mutation gen {}",
                    best.generation
                )),
                confidence: 0.6,
                reason: format!(
                    "Parameter '{}' was successfully mutated in generation {}",
                    best.target_parameter, best.generation
                ),
            });
        }
        drop(history);

        // Source 4: Structural deficit
        for (idx, &score) in layer_health.iter().enumerate().skip(1) {
            if score < 0.5 {
                return Some(MutationHint {
                    parameter: format!("L{}_structural", idx + 1),
                    source: HintSource::StructuralDeficit(format!(
                        "L{} health={score:.2}",
                        idx + 1
                    )),
                    confidence: 0.99,
                    reason: format!(
                        "Layer {} below 0.5 — structural deficit, not tunable parameter",
                        idx + 1
                    ),
                });
            }
        }

        None
    }

    /// R19: Check for convergence (fitness variance below threshold).
    ///
    /// Returns `true` if the last `CONVERGENCE_WINDOW` snapshots have
    /// fitness variance below `CONVERGENCE_VARIANCE_THRESHOLD`, indicating
    /// the chamber should pause and avoid wasted cycles.
    #[must_use]
    pub fn is_converged(&self) -> bool {
        let snaps = self.fitness_snapshots.read();
        if snaps.len() < CONVERGENCE_WINDOW {
            return false;
        }
        let recent: Vec<f64> = snaps
            .iter()
            .rev()
            .take(CONVERGENCE_WINDOW)
            .map(|s| s.fitness)
            .collect();
        drop(snaps); // early drop before computation
        #[allow(clippy::cast_precision_loss)]
        let count = recent.len() as f64;
        let mean = recent.iter().sum::<f64>() / count;
        let variance = recent.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / count;
        variance < CONVERGENCE_VARIANCE_THRESHOLD
    }

    /// R19: Record a complete mutation outcome with V2 fields.
    ///
    /// Unlike V1 which left `fitness_after` at 0.0 and all hint fields
    /// as None, V2 mandates recording the full outcome including strategy,
    /// hint source, and field coherence delta.
    pub fn record_v2_outcome(
        &self,
        mutation_id: &str,
        fitness_after: f64,
        strategy: &EvolutionStrategy,
        hint: Option<&MutationHint>,
    ) {
        let mut history = self.mutation_history.write();
        if let Some(record) = history.iter_mut().find(|m| m.id == mutation_id) {
            record.fitness_after = fitness_after;
            // V2: We can't add new fields to the existing MutationRecord struct
            // without breaking V1 compatibility, so we log the V2 metadata
            // alongside the record for now. Full V2 MutationRecord migration
            // will be in a future sprint when the database schema is updated.
            let hint_str = hint.map_or_else(
                || "none".to_string(),
                |h| h.source.to_string(),
            );
            tracing::info!(
                mutation_id,
                fitness_after,
                strategy = %strategy,
                hint_source = hint_str,
                "V2 mutation outcome recorded"
            );
        }
    }

    /// Verify or rollback a mutation based on current fitness.
    ///
    /// Combines V1's separate verify/rollback into a single decision
    /// point. Used by the RALPH loop in `ralph_process_mutations`.
    ///
    /// # Errors
    ///
    /// Returns `Error` if the mutation is not found or in wrong state.
    pub fn verify_or_rollback(
        &self,
        mutation_id: &str,
        current_fitness: f64,
    ) -> Result<MutationRecord> {
        let fitness_at_proposal = {
            let active = self.active_mutations.read();
            active
                .iter()
                .find(|m| m.id == mutation_id)
                .map(|m| m.fitness_at_proposal)
                .ok_or_else(|| {
                    Error::Validation(format!("active mutation '{mutation_id}' not found"))
                })?
        };

        let delta = current_fitness - fitness_at_proposal;

        if self.should_auto_apply(delta) || delta >= 0.0 {
            self.verify_mutation(mutation_id, current_fitness)
        } else if self.should_rollback(delta) {
            self.rollback_mutation(mutation_id)
        } else {
            // Marginal delta — verify anyway (V2 is more accepting)
            self.verify_mutation(mutation_id, current_fitness)
        }
    }
}

impl Default for EvolutionChamber {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests (50 total)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_chamber() -> EvolutionChamber {
        EvolutionChamber::new()
    }

    fn make_chamber_with_config(max_mutations: u32) -> EvolutionChamber {
        EvolutionChamber::with_config(EvolutionChamberConfig {
            max_concurrent_mutations: max_mutations,
            ..EvolutionChamberConfig::default()
        })
    }

    /// Helper: propose, apply, and return the mutation ID.
    fn propose_and_apply(
        chamber: &EvolutionChamber,
        target: &str,
        original: f64,
        mutated: f64,
    ) -> String {
        let rec = chamber
            .propose_mutation(target, original, mutated, 0.80)
            .unwrap_or_else(|_| unreachable!());
        let id = rec.id.clone();
        chamber
            .apply_mutation(&id)
            .unwrap_or_else(|_| unreachable!());
        id
    }

    // -----------------------------------------------------------------------
    // 1. Construction & defaults
    // -----------------------------------------------------------------------

    #[test]
    fn test_01_new_chamber_initial_state() {
        let c = make_chamber();
        assert_eq!(c.generation(), 0);
        assert_eq!(c.active_mutation_count(), 0);
        assert!(c.recent_mutations(10).is_empty());
        assert!(c.fitness_history(10).is_empty());
    }

    #[test]
    fn test_02_default_and_with_config() {
        let c = EvolutionChamber::default();
        assert_eq!(c.generation(), 0);
        assert_eq!(
            c.config().max_concurrent_mutations,
            DEFAULT_MAX_CONCURRENT_MUTATIONS
        );

        let cfg = EvolutionChamberConfig {
            max_concurrent_mutations: 7,
            fitness_history_capacity: 100,
            ..EvolutionChamberConfig::default()
        };
        let c2 = EvolutionChamber::with_config(cfg);
        assert_eq!(c2.config().max_concurrent_mutations, 7);
        assert_eq!(c2.config().fitness_history_capacity, 100);
    }

    // -----------------------------------------------------------------------
    // 2. Config validation
    // -----------------------------------------------------------------------

    #[test]
    fn test_03_validate_config_default_passes() {
        assert!(EvolutionChamber::validate_config(&EvolutionChamberConfig::default()).is_ok());
    }

    #[test]
    fn test_04_validate_config_rejects_zero_concurrent() {
        let mut cfg = EvolutionChamberConfig::default();
        cfg.max_concurrent_mutations = 0;
        assert!(EvolutionChamber::validate_config(&cfg).is_err());
    }

    #[test]
    fn test_05_validate_config_rejects_zero_verification_ms() {
        let mut cfg = EvolutionChamberConfig::default();
        cfg.mutation_verification_ms = 0;
        assert!(EvolutionChamber::validate_config(&cfg).is_err());
    }

    #[test]
    fn test_06_validate_config_rejects_zero_capacities() {
        let mut cfg = EvolutionChamberConfig::default();
        cfg.fitness_history_capacity = 0;
        assert!(EvolutionChamber::validate_config(&cfg).is_err());

        let mut cfg2 = EvolutionChamberConfig::default();
        cfg2.mutation_history_capacity = 0;
        assert!(EvolutionChamber::validate_config(&cfg2).is_err());
    }

    #[test]
    fn test_07_validate_config_rejects_bad_thresholds() {
        let mut cfg = EvolutionChamberConfig::default();
        cfg.auto_apply_threshold = 1.5;
        assert!(EvolutionChamber::validate_config(&cfg).is_err());

        let mut cfg2 = EvolutionChamberConfig::default();
        cfg2.rollback_threshold = 0.01;
        assert!(EvolutionChamber::validate_config(&cfg2).is_err());
    }

    #[test]
    fn test_08_validate_config_rejects_bad_delta_and_interval() {
        let mut cfg = EvolutionChamberConfig::default();
        cfg.max_mutation_delta = 0.0;
        assert!(EvolutionChamber::validate_config(&cfg).is_err());

        let mut cfg2 = EvolutionChamberConfig::default();
        cfg2.min_generation_interval_ms = 0;
        assert!(EvolutionChamber::validate_config(&cfg2).is_err());
    }

    // -----------------------------------------------------------------------
    // 3. Mutation proposal
    // -----------------------------------------------------------------------

    #[test]
    fn test_09_propose_mutation_creates_record() {
        let c = make_chamber();
        let rec = c.propose_mutation("ltp_rate", 0.10, 0.12, 0.85);
        assert!(rec.is_ok());
        let rec = rec.unwrap_or_else(|_| unreachable!());
        assert_eq!(rec.target_parameter, "ltp_rate");
        assert!((rec.original_value - 0.10).abs() < 1e-10);
        assert!((rec.mutated_value - 0.12).abs() < 1e-10);
        assert!((rec.delta - 0.02).abs() < 1e-10);
        assert!(!rec.applied);
        assert!(!rec.rolled_back);
        // UUID format check
        assert!(rec.id.contains('-'));
        assert_eq!(rec.id.len(), 36);
    }

    #[test]
    fn test_10_propose_mutation_creates_active_entry() {
        let c = make_chamber();
        let _rec = c
            .propose_mutation("decay_rate", 0.001, 0.002, 0.80)
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(c.active_mutation_count(), 1);
    }

    #[test]
    fn test_11_propose_empty_target_fails() {
        let c = make_chamber();
        assert!(c.propose_mutation("", 0.1, 0.2, 0.8).is_err());
    }

    #[test]
    fn test_12_propose_exceeding_delta_fails() {
        let c = make_chamber();
        // default max_mutation_delta = 0.20; delta = 0.5 - 0.0 = 0.5
        assert!(c.propose_mutation("param", 0.0, 0.5, 0.8).is_err());
    }

    #[test]
    fn test_13_propose_exceeding_concurrent_limit_fails() {
        let c = make_chamber_with_config(2);
        let _r1 = c.propose_mutation("p1", 0.1, 0.12, 0.8);
        let _r2 = c.propose_mutation("p2", 0.2, 0.22, 0.8);
        assert!(c.propose_mutation("p3", 0.3, 0.32, 0.8).is_err());
    }

    // -----------------------------------------------------------------------
    // 4. Mutation application
    // -----------------------------------------------------------------------

    #[test]
    fn test_14_apply_mutation_transitions_to_verifying() {
        let c = make_chamber();
        let rec = c
            .propose_mutation("ltp_rate", 0.1, 0.12, 0.8)
            .unwrap_or_else(|_| unreachable!());
        assert!(c.apply_mutation(&rec.id).is_ok());
        assert_eq!(c.active_mutation_count(), 1);
    }

    #[test]
    fn test_15_apply_nonexistent_mutation_fails() {
        let c = make_chamber();
        assert!(c.apply_mutation("nonexistent-id").is_err());
    }

    #[test]
    fn test_16_apply_already_verifying_mutation_fails() {
        let c = make_chamber();
        let rec = c
            .propose_mutation("param", 0.1, 0.12, 0.8)
            .unwrap_or_else(|_| unreachable!());
        c.apply_mutation(&rec.id)
            .unwrap_or_else(|_| unreachable!());
        assert!(c.apply_mutation(&rec.id).is_err());
    }

    // -----------------------------------------------------------------------
    // 5. Mutation verification
    // -----------------------------------------------------------------------

    #[test]
    fn test_17_verify_mutation_moves_to_history() {
        let c = make_chamber();
        let id = propose_and_apply(&c, "ltp_rate", 0.10, 0.12);
        let result = c.verify_mutation(&id, 0.90);
        assert!(result.is_ok());
        let record = result.unwrap_or_else(|_| unreachable!());
        assert!(record.applied);
        assert!(!record.rolled_back);
        assert!((record.fitness_after - 0.90).abs() < 1e-10);
        assert_eq!(c.active_mutation_count(), 0);
        assert!(c.get_mutation(&id).is_some());
    }

    #[test]
    fn test_18_verify_nonexistent_mutation_fails() {
        let c = make_chamber();
        assert!(c.verify_mutation("nonexistent-id", 0.9).is_err());
    }

    #[test]
    fn test_19_verify_proposed_not_applied_fails() {
        let c = make_chamber();
        let rec = c
            .propose_mutation("param", 0.1, 0.12, 0.8)
            .unwrap_or_else(|_| unreachable!());
        assert!(c.verify_mutation(&rec.id, 0.9).is_err());
    }

    // -----------------------------------------------------------------------
    // 6. Mutation rollback
    // -----------------------------------------------------------------------

    #[test]
    fn test_20_rollback_applied_mutation() {
        let c = make_chamber();
        let rec = c
            .propose_mutation("ltp_rate", 0.10, 0.12, 0.8)
            .unwrap_or_else(|_| unreachable!());
        let id = rec.id.clone();
        c.apply_mutation(&id).unwrap_or_else(|_| unreachable!());
        let result = c.rollback_mutation(&id);
        assert!(result.is_ok());
        let record = result.unwrap_or_else(|_| unreachable!());
        assert!(record.rolled_back);
        assert!(record.applied);
        assert_eq!(c.active_mutation_count(), 0);
    }

    #[test]
    fn test_21_rollback_nonexistent_mutation_fails() {
        let c = make_chamber();
        assert!(c.rollback_mutation("nonexistent-id").is_err());
    }

    #[test]
    fn test_22_rollback_proposed_mutation_succeeds() {
        let c = make_chamber();
        let rec = c
            .propose_mutation("param", 0.1, 0.12, 0.8)
            .unwrap_or_else(|_| unreachable!());
        assert!(c.rollback_mutation(&rec.id).is_ok());
        assert_eq!(c.active_mutation_count(), 0);
    }

    // -----------------------------------------------------------------------
    // 7. RALPH phase transitions
    // -----------------------------------------------------------------------

    #[test]
    fn test_23_initial_phase_is_recognize() {
        let c = make_chamber();
        assert_eq!(c.ralph_state().current_phase, RalphPhase::Recognize);
    }

    #[test]
    fn test_24_advance_phase_full_cycle_wraps() {
        let c = make_chamber();
        let _cycle = c.start_cycle().unwrap_or_else(|_| unreachable!());

        let expected = [
            RalphPhase::Analyze,
            RalphPhase::Learn,
            RalphPhase::Propose,
            RalphPhase::Harvest,
            RalphPhase::Recognize,
        ];
        for &phase in &expected {
            let p = c.advance_phase().unwrap_or_else(|_| unreachable!());
            assert_eq!(p, phase);
        }
    }

    #[test]
    fn test_25_advance_phase_without_cycle_fails() {
        let c = make_chamber();
        assert!(c.advance_phase().is_err());
    }

    #[test]
    fn test_26_advance_phase_while_paused_fails() {
        let c = make_chamber();
        let _cycle = c.start_cycle().unwrap_or_else(|_| unreachable!());
        c.pause();
        assert!(c.advance_phase().is_err());
    }

    #[test]
    fn test_27_ralph_phase_ordinals_and_names() {
        let phases = [
            (RalphPhase::Recognize, 0, "Recognize"),
            (RalphPhase::Analyze, 1, "Analyze"),
            (RalphPhase::Learn, 2, "Learn"),
            (RalphPhase::Propose, 3, "Propose"),
            (RalphPhase::Harvest, 4, "Harvest"),
        ];
        for (phase, ordinal, name) in phases {
            assert_eq!(phase.ordinal(), ordinal);
            assert_eq!(phase.name(), name);
            assert_eq!(phase.to_string(), name);
        }
    }

    // -----------------------------------------------------------------------
    // 8. Cycle management
    // -----------------------------------------------------------------------

    #[test]
    fn test_28_start_cycle_increments_generation() {
        let c = make_chamber();
        let cycle = c.start_cycle().unwrap_or_else(|_| unreachable!());
        assert_eq!(cycle, 1);
        assert_eq!(c.generation(), 1);
    }

    #[test]
    fn test_29_start_cycle_while_active_fails() {
        let c = make_chamber();
        let _c1 = c.start_cycle().unwrap_or_else(|_| unreachable!());
        assert!(c.start_cycle().is_err());
    }

    #[test]
    fn test_30_complete_then_start_new_cycle() {
        let c = make_chamber();
        let _c1 = c.start_cycle().unwrap_or_else(|_| unreachable!());
        c.complete_cycle().unwrap_or_else(|_| unreachable!());
        let c2 = c.start_cycle().unwrap_or_else(|_| unreachable!());
        assert_eq!(c2, 2);
        assert_eq!(c.generation(), 2);
    }

    #[test]
    fn test_31_complete_without_start_fails() {
        let c = make_chamber();
        assert!(c.complete_cycle().is_err());
    }

    #[test]
    fn test_32_start_cycle_while_paused_fails() {
        let c = make_chamber();
        c.pause();
        assert!(c.start_cycle().is_err());
    }

    // -----------------------------------------------------------------------
    // 9. Pause / Resume
    // -----------------------------------------------------------------------

    #[test]
    fn test_33_pause_and_resume() {
        let c = make_chamber();
        c.pause();
        assert!(c.ralph_state().paused);
        c.resume();
        assert!(!c.ralph_state().paused);
    }

    // -----------------------------------------------------------------------
    // 10. Fitness history
    // -----------------------------------------------------------------------

    #[test]
    fn test_34_record_fitness_stores_snapshot() {
        let c = make_chamber();
        let snap = c.record_fitness(0.85, [0.5; 12]);
        assert!((snap.fitness - 0.85).abs() < 1e-10);
        assert_eq!(c.fitness_history(10).len(), 1);
    }

    #[test]
    fn test_35_fitness_history_capacity_enforced() {
        let cfg = EvolutionChamberConfig {
            fitness_history_capacity: 5,
            ..EvolutionChamberConfig::default()
        };
        let c = EvolutionChamber::with_config(cfg);
        for i in 0..10 {
            #[allow(clippy::cast_precision_loss)]
            let fitness = i as f64 * 0.1;
            c.record_fitness(fitness, [0.5; 12]);
        }
        let history = c.fitness_history(100);
        assert_eq!(history.len(), 5);
        // Oldest entries evicted; first surviving is generation index 5 (0.5)
        assert!((history[0].fitness - 0.5).abs() < 1e-10);
    }

    #[test]
    fn test_36_fitness_snapshot_generation_tracking() {
        let c = make_chamber();
        // Before any cycle, generation is 0 -> snapshot has None
        let snap0 = c.record_fitness(0.9, [1.0; 12]);
        assert_eq!(snap0.generation, None);

        // Start a cycle -> generation becomes 1
        let _cycle = c.start_cycle().unwrap_or_else(|_| unreachable!());
        let snap1 = c.record_fitness(0.9, [1.0; 12]);
        assert_eq!(snap1.generation, Some(1));
    }

    // -----------------------------------------------------------------------
    // 11. Auto-apply and rollback thresholds
    // -----------------------------------------------------------------------

    #[test]
    fn test_37_should_auto_apply_boundary() {
        let c = make_chamber();
        // At threshold
        assert!(c.should_auto_apply(0.10));
        // Above threshold
        assert!(c.should_auto_apply(0.15));
        // Below threshold
        assert!(!c.should_auto_apply(0.05));
        assert!(!c.should_auto_apply(-0.01));
    }

    #[test]
    fn test_38_should_rollback_boundary() {
        let c = make_chamber();
        // At threshold
        assert!(c.should_rollback(-0.02));
        // Below threshold
        assert!(c.should_rollback(-0.05));
        // Above threshold (not rolled back)
        assert!(!c.should_rollback(0.0));
        assert!(!c.should_rollback(-0.01));
    }

    // -----------------------------------------------------------------------
    // 12. Statistics
    // -----------------------------------------------------------------------

    #[test]
    fn test_39_stats_track_propose_apply_rollback() {
        let c = make_chamber();
        let _cycle = c.start_cycle().unwrap_or_else(|_| unreachable!());

        let rec = c
            .propose_mutation("ltp_rate", 0.10, 0.12, 0.8)
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(c.stats().total_mutations_proposed, 1);

        c.apply_mutation(&rec.id)
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(c.stats().total_mutations_applied, 1);

        c.rollback_mutation(&rec.id)
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(c.stats().total_mutations_rolled_back, 1);
    }

    #[test]
    fn test_40_stats_current_phase_tracks_active_cycle() {
        let c = make_chamber();
        assert!(c.stats().current_phase.is_none());

        let _cycle = c.start_cycle().unwrap_or_else(|_| unreachable!());
        assert_eq!(c.stats().current_phase, Some(RalphPhase::Recognize));

        c.complete_cycle().unwrap_or_else(|_| unreachable!());
        assert!(c.stats().current_phase.is_none());
    }

    // -----------------------------------------------------------------------
    // 13. Clear
    // -----------------------------------------------------------------------

    #[test]
    fn test_41_clear_resets_all_state() {
        let c = make_chamber();
        let _cycle = c.start_cycle().unwrap_or_else(|_| unreachable!());
        c.record_fitness(0.85, [0.5; 12]);
        let _rec = c.propose_mutation("param", 0.1, 0.12, 0.8);
        c.complete_cycle().unwrap_or_else(|_| unreachable!());

        c.clear();

        assert_eq!(c.generation(), 0);
        assert_eq!(c.active_mutation_count(), 0);
        assert!(c.recent_mutations(10).is_empty());
        assert!(c.fitness_history(10).is_empty());
        assert_eq!(c.stats().total_mutations_proposed, 0);
        assert_eq!(c.stats().total_ralph_cycles, 0);
        assert!(!c.ralph_state().paused);
    }

    // -----------------------------------------------------------------------
    // 14. Mutation history capacity
    // -----------------------------------------------------------------------

    #[test]
    fn test_42_mutation_history_capacity_enforced() {
        let cfg = EvolutionChamberConfig {
            mutation_history_capacity: 3,
            ..EvolutionChamberConfig::default()
        };
        let c = EvolutionChamber::with_config(cfg);

        for i in 0..5 {
            let id = propose_and_apply(&c, &format!("p{i}"), 0.1, 0.12);
            c.verify_mutation(&id, 0.9)
                .unwrap_or_else(|_| unreachable!());
        }

        assert_eq!(c.recent_mutations(100).len(), 3);
    }

    // -----------------------------------------------------------------------
    // 15. Thread safety
    // -----------------------------------------------------------------------

    #[test]
    fn test_43_concurrent_propose_mutations() {
        use std::sync::Arc;
        use std::thread;

        let c = Arc::new(EvolutionChamber::with_config(EvolutionChamberConfig {
            max_concurrent_mutations: 100,
            ..EvolutionChamberConfig::default()
        }));
        let mut handles = Vec::new();

        for t in 0..4 {
            let c_clone = Arc::clone(&c);
            handles.push(thread::spawn(move || {
                for i in 0..5 {
                    let _r = c_clone.propose_mutation(
                        &format!("t{t}_p{i}"),
                        0.1,
                        0.12,
                        0.8,
                    );
                }
            }));
        }

        for handle in handles {
            let _r = handle.join();
        }

        assert_eq!(c.stats().total_mutations_proposed, 20);
        assert_eq!(c.active_mutation_count(), 20);
    }

    #[test]
    fn test_44_concurrent_fitness_recording() {
        use std::sync::Arc;
        use std::thread;

        let c = Arc::new(make_chamber());
        let mut handles = Vec::new();

        for _ in 0..4 {
            let c_clone = Arc::clone(&c);
            handles.push(thread::spawn(move || {
                for _ in 0..10 {
                    c_clone.record_fitness(0.85, [0.5; 12]);
                }
            }));
        }

        for handle in handles {
            let _r = handle.join();
        }

        assert_eq!(c.fitness_history(1000).len(), 40);
    }

    // -----------------------------------------------------------------------
    // 16. Mutation delta boundaries
    // -----------------------------------------------------------------------

    #[test]
    fn test_45_exact_max_delta_is_allowed() {
        let c = make_chamber();
        // max_mutation_delta default = 0.20
        assert!(c.propose_mutation("param", 0.50, 0.70, 0.8).is_ok());
    }

    #[test]
    fn test_46_negative_delta_within_bounds() {
        let c = make_chamber();
        let rec = c
            .propose_mutation("param", 0.50, 0.35, 0.8)
            .unwrap_or_else(|_| unreachable!());
        assert!(rec.delta < 0.0);
    }

    // -----------------------------------------------------------------------
    // 17. Generation numbering
    // -----------------------------------------------------------------------

    #[test]
    fn test_47_multiple_cycles_advance_generation() {
        let c = make_chamber();
        for expected in 1..=5 {
            let cycle = c.start_cycle().unwrap_or_else(|_| unreachable!());
            assert_eq!(cycle, expected);
            assert_eq!(c.generation(), expected);
            c.complete_cycle().unwrap_or_else(|_| unreachable!());
        }
    }

    // -----------------------------------------------------------------------
    // 18. Display implementations
    // -----------------------------------------------------------------------

    #[test]
    fn test_48_mutation_status_display() {
        assert_eq!(MutationStatus::Proposed.to_string(), "Proposed");
        assert_eq!(MutationStatus::Verifying.to_string(), "Verifying");
        assert_eq!(MutationStatus::Accepted.to_string(), "Accepted");
        assert_eq!(MutationStatus::RolledBack.to_string(), "RolledBack");
        assert_eq!(MutationStatus::Failed.to_string(), "Failed");
    }

    // -----------------------------------------------------------------------
    // 19. History lookup
    // -----------------------------------------------------------------------

    #[test]
    fn test_49_get_mutation_by_id_and_not_found() {
        let c = make_chamber();
        assert!(c.get_mutation("nonexistent").is_none());

        let id = propose_and_apply(&c, "param", 0.1, 0.12);
        c.verify_mutation(&id, 0.9)
            .unwrap_or_else(|_| unreachable!());
        let found = c.get_mutation(&id);
        assert!(found.is_some());
        let found = found.unwrap_or_else(|| unreachable!());
        assert_eq!(found.target_parameter, "param");
    }

    // -----------------------------------------------------------------------
    // 20. RALPH state after cycle operations
    // -----------------------------------------------------------------------

    #[test]
    fn test_50_ralph_state_reflects_cycle_lifecycle() {
        let c = make_chamber();

        // Before cycle: no start/complete timestamps
        let rs0 = c.ralph_state();
        assert!(rs0.cycle_started_at.is_none());
        assert!(rs0.cycle_completed_at.is_none());

        // After start
        let _cycle = c.start_cycle().unwrap_or_else(|_| unreachable!());
        let rs1 = c.ralph_state();
        assert!(rs1.cycle_started_at.is_some());
        assert!(rs1.cycle_completed_at.is_none());
        assert_eq!(rs1.cycle_number, 1);
        assert_eq!(rs1.current_phase, RalphPhase::Recognize);

        // After complete
        c.complete_cycle().unwrap_or_else(|_| unreachable!());
        let rs2 = c.ralph_state();
        assert!(rs2.cycle_completed_at.is_some());
    }
}
