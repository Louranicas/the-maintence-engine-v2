//! # M56: Checkpoint Manager
//!
//! Manages cognitive state checkpoints for the Maintenance Engine's evolution
//! and consensus systems. Provides save, load, list, and prune operations
//! for [`CognitiveSnapshot`] instances, enabling rollback and recovery
//! during RALPH evolution cycles and PBFT consensus rounds.
//!
//! ## Layer: L6 (Consensus)
//!
//! ## Key Types
//!
//! | Type | Purpose |
//! |------|---------|
//! | [`CognitiveSnapshot`] | Full cognitive state capture (12D tensor, fitness, evolution counters) |
//! | [`CheckpointSummary`] | Lightweight reference for listing checkpoints |
//! | [`CheckpointConfig`] | Retention and pruning configuration |
//! | [`InMemoryCheckpointManager`] | Thread-safe in-memory implementation |
//!
//! ## Thread Safety
//!
//! Uses `std::sync::RwLock` (L6 convention) with explicit lock-poisoning
//! error handling via `Error::Other`.
//!
//! ## Related Documentation
//! - [Module Specification](../../ai_docs/modules/M56_CHECKPOINT_MANAGER.md)
//! - [Evolution Chamber](../m7_observer/evolution_chamber.rs)

use std::fmt;
use std::sync::RwLock;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::m1_foundation::shared_types::Timestamp;
use crate::{Error, Result};

/// Maximum number of fitness history entries retained per snapshot.
const MAX_FITNESS_HISTORY: usize = 50;

// ============================================================================
// CognitiveSnapshot
// ============================================================================

/// Full cognitive state capture for checkpoint persistence.
///
/// Contains all essential state required to restore the Maintenance Engine's
/// cognitive systems to a previous point: evolution generation, fitness tensor,
/// mutation statistics, and phase information.
///
/// # Serialization
///
/// The `saved_at` field stores a [`Timestamp`] as its raw `u64` tick value
/// for serde compatibility (Timestamp does not derive Serialize/Deserialize).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CognitiveSnapshot {
    /// Unique checkpoint identifier (UUID v4).
    pub id: String,
    /// RALPH evolution generation number.
    pub generation: u64,
    /// Current composite fitness score (0.0..=1.0).
    pub fitness: f64,
    /// Recent fitness history (newest last, max 50 entries).
    pub fitness_history: Vec<f64>,
    /// Total mutations attempted.
    pub mutation_count: u64,
    /// Total mutations accepted.
    pub accepted_count: u64,
    /// Total mutations rolled back.
    pub rolled_back_count: u64,
    /// Current evolution cycle number.
    pub cycle_number: u64,
    /// Current evolution phase name.
    pub current_phase: String,
    /// Whether evolution is paused.
    pub paused: bool,
    /// 12D tensor snapshot.
    pub tensor_snapshot: [f64; 12],
    /// Timestamp tick value when this snapshot was saved.
    saved_at_ticks: u64,
}

impl CognitiveSnapshot {
    /// Returns the save timestamp as a [`Timestamp`].
    #[must_use]
    pub const fn saved_at(&self) -> Timestamp {
        Timestamp::from_raw(self.saved_at_ticks)
    }
}

// ============================================================================
// CheckpointSummary
// ============================================================================

/// Lightweight summary of a checkpoint for listing operations.
///
/// Contains only the identifying metadata without the full tensor
/// or fitness history, suitable for index/catalog displays.
#[derive(Clone, Debug)]
pub struct CheckpointSummary {
    /// Unique checkpoint identifier.
    pub id: String,
    /// RALPH evolution generation number.
    pub generation: u64,
    /// Composite fitness score at time of save.
    pub fitness: f64,
    /// Timestamp tick value when saved.
    saved_at_ticks: u64,
}

impl CheckpointSummary {
    /// Returns the save timestamp as a [`Timestamp`].
    #[must_use]
    pub const fn saved_at(&self) -> Timestamp {
        Timestamp::from_raw(self.saved_at_ticks)
    }
}

// ============================================================================
// CheckpointConfig
// ============================================================================

/// Configuration for checkpoint retention and pruning behaviour.
///
/// # Defaults
///
/// | Field | Default |
/// |-------|---------|
/// | `max_retained` | 20 |
/// | `min_generation_gap` | 5 |
/// | `auto_prune` | true |
#[derive(Clone, Debug)]
pub struct CheckpointConfig {
    /// Maximum number of checkpoints to retain.
    pub max_retained: usize,
    /// Minimum generation gap between consecutive checkpoints.
    /// A save is skipped (returns `Ok` with existing ID) when the gap is too small.
    pub min_generation_gap: u64,
    /// Whether to automatically prune old checkpoints after each save.
    pub auto_prune: bool,
}

impl Default for CheckpointConfig {
    fn default() -> Self {
        Self {
            max_retained: 20,
            min_generation_gap: 5,
            auto_prune: true,
        }
    }
}

impl CheckpointConfig {
    /// Validate configuration parameters.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if `max_retained` is zero.
    pub fn validate(&self) -> Result<()> {
        if self.max_retained == 0 {
            return Err(Error::Validation(
                "max_retained must be greater than 0".into(),
            ));
        }
        Ok(())
    }
}

// ============================================================================
// CheckpointManager trait
// ============================================================================

/// Trait for managing cognitive state checkpoints.
///
/// All methods take `&self` and use interior mutability (L6 convention).
/// Implementations must be `Send + Sync` for cross-thread access.
pub trait CheckpointManager: Send + Sync + fmt::Debug {
    /// Save a cognitive snapshot as a checkpoint.
    ///
    /// Generates a UUID, validates generation gap against the most recent
    /// checkpoint, truncates fitness history to 50 entries, and optionally
    /// auto-prunes old checkpoints.
    ///
    /// # Errors
    ///
    /// Returns `Error::Other` if an internal lock is poisoned.
    fn save(&self, snapshot: CognitiveSnapshot) -> Result<String>;

    /// Load the most recently saved checkpoint.
    ///
    /// # Errors
    ///
    /// Returns `Error::Other` if an internal lock is poisoned.
    fn load_latest(&self) -> Result<Option<CognitiveSnapshot>>;

    /// Load a checkpoint by exact generation number.
    ///
    /// # Errors
    ///
    /// Returns `Error::Other` if an internal lock is poisoned.
    fn load_by_generation(&self, generation: u64) -> Result<Option<CognitiveSnapshot>>;

    /// List checkpoint summaries, newest first.
    ///
    /// Returns at most `limit` entries.
    fn list_checkpoints(&self, limit: usize) -> Vec<CheckpointSummary>;

    /// Remove the oldest checkpoints, keeping `keep` most recent.
    ///
    /// # Errors
    ///
    /// Returns `Error::Other` if an internal lock is poisoned.
    /// Returns `Error::Validation` if `keep` is zero.
    fn prune_old(&self, keep: usize) -> Result<usize>;

    /// Return the total number of stored checkpoints.
    fn checkpoint_count(&self) -> usize;
}

// ============================================================================
// InMemoryCheckpointManager
// ============================================================================

/// In-memory checkpoint manager using `std::sync::RwLock` (L6 convention).
///
/// Stores checkpoints in a `Vec` ordered by insertion time (oldest first).
/// Thread-safe for concurrent read/write access across consensus agents.
pub struct InMemoryCheckpointManager {
    /// Ordered checkpoint storage (oldest first).
    checkpoints: RwLock<Vec<CognitiveSnapshot>>,
    /// Retention and pruning configuration.
    config: CheckpointConfig,
}

impl fmt::Debug for InMemoryCheckpointManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let count = self
            .checkpoints
            .read()
            .map(|g| g.len())
            .unwrap_or(0);
        f.debug_struct("InMemoryCheckpointManager")
            .field("checkpoint_count", &count)
            .field("config", &self.config)
            .finish()
    }
}

impl InMemoryCheckpointManager {
    /// Create a new checkpoint manager with the given configuration.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if the config is invalid (e.g. `max_retained == 0`).
    pub fn new(config: CheckpointConfig) -> Result<Self> {
        config.validate()?;
        Ok(Self {
            checkpoints: RwLock::new(Vec::new()),
            config,
        })
    }

    /// Create a checkpoint manager with default configuration.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if defaults are somehow invalid (should not happen).
    pub fn with_defaults() -> Result<Self> {
        Self::new(CheckpointConfig::default())
    }

    /// Read-lock helper that converts lock poisoning to `Error::Other`.
    fn read_lock(&self) -> Result<std::sync::RwLockReadGuard<'_, Vec<CognitiveSnapshot>>> {
        self.checkpoints
            .read()
            .map_err(|e| Error::Other(format!("Lock poisoned: {e}")))
    }

    /// Write-lock helper that converts lock poisoning to `Error::Other`.
    fn write_lock(&self) -> Result<std::sync::RwLockWriteGuard<'_, Vec<CognitiveSnapshot>>> {
        self.checkpoints
            .write()
            .map_err(|e| Error::Other(format!("Lock poisoned: {e}")))
    }
}

impl CheckpointManager for InMemoryCheckpointManager {
    fn save(&self, mut snapshot: CognitiveSnapshot) -> Result<String> {
        // Assign a UUID
        snapshot.id = Uuid::new_v4().to_string();

        // Record save timestamp
        snapshot.saved_at_ticks = Timestamp::now().ticks();

        // Truncate fitness history to MAX_FITNESS_HISTORY
        if snapshot.fitness_history.len() > MAX_FITNESS_HISTORY {
            let start = snapshot.fitness_history.len() - MAX_FITNESS_HISTORY;
            snapshot.fitness_history = snapshot.fitness_history[start..].to_vec();
        }

        let mut checkpoints = self.write_lock()?;

        // Enforce minimum generation gap
        if let Some(last) = checkpoints.last() {
            if snapshot.generation.saturating_sub(last.generation) < self.config.min_generation_gap {
                return Ok(last.id.clone());
            }
        }

        let id = snapshot.id.clone();
        checkpoints.push(snapshot);

        // Auto-prune if enabled
        if self.config.auto_prune && checkpoints.len() > self.config.max_retained {
            let to_remove = checkpoints.len() - self.config.max_retained;
            checkpoints.drain(..to_remove);
        }

        drop(checkpoints);
        Ok(id)
    }

    fn load_latest(&self) -> Result<Option<CognitiveSnapshot>> {
        let checkpoints = self.read_lock()?;
        Ok(checkpoints.last().cloned())
    }

    fn load_by_generation(&self, generation: u64) -> Result<Option<CognitiveSnapshot>> {
        let checkpoints = self.read_lock()?;
        Ok(checkpoints
            .iter()
            .find(|cp| cp.generation == generation)
            .cloned())
    }

    fn list_checkpoints(&self, limit: usize) -> Vec<CheckpointSummary> {
        let Ok(checkpoints) = self.read_lock() else {
            return Vec::new();
        };

        let len = checkpoints.len();
        let start = len.saturating_sub(limit);

        checkpoints[start..]
            .iter()
            .rev()
            .map(|cp| CheckpointSummary {
                id: cp.id.clone(),
                generation: cp.generation,
                fitness: cp.fitness,
                saved_at_ticks: cp.saved_at_ticks,
            })
            .collect()
    }

    fn prune_old(&self, keep: usize) -> Result<usize> {
        if keep == 0 {
            return Err(Error::Validation("keep must be greater than 0".into()));
        }

        let mut checkpoints = self.write_lock()?;
        let len = checkpoints.len();

        if len <= keep {
            return Ok(0);
        }

        let to_remove = len - keep;
        checkpoints.drain(..to_remove);
        drop(checkpoints);
        Ok(to_remove)
    }

    fn checkpoint_count(&self) -> usize {
        self.checkpoints
            .read()
            .map(|g| g.len())
            .unwrap_or(0)
    }
}

/// Create a default [`CognitiveSnapshot`] for testing or initialisation.
///
/// All counters start at zero, fitness at 0.5, tensor dimensions at 0.0,
/// phase set to "Init", and generation set to the given value.
#[must_use]
pub fn default_snapshot(generation: u64) -> CognitiveSnapshot {
    CognitiveSnapshot {
        id: String::new(),
        generation,
        fitness: 0.5,
        fitness_history: Vec::new(),
        mutation_count: 0,
        accepted_count: 0,
        rolled_back_count: 0,
        cycle_number: 0,
        current_phase: "Init".into(),
        paused: false,
        tensor_snapshot: [0.0; 12],
        saved_at_ticks: 0,
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // --- Helper ---

    fn make_manager() -> InMemoryCheckpointManager {
        InMemoryCheckpointManager::new(CheckpointConfig {
            max_retained: 20,
            min_generation_gap: 5,
            auto_prune: true,
        })
        .ok()
        .unwrap_or_else(|| unreachable!())
    }

    fn make_manager_no_gap() -> InMemoryCheckpointManager {
        InMemoryCheckpointManager::new(CheckpointConfig {
            max_retained: 20,
            min_generation_gap: 0,
            auto_prune: false,
        })
        .ok()
        .unwrap_or_else(|| unreachable!())
    }

    fn snapshot_gen(gen: u64) -> CognitiveSnapshot {
        default_snapshot(gen)
    }

    fn snapshot_with_fitness(gen: u64, fitness: f64) -> CognitiveSnapshot {
        let mut s = default_snapshot(gen);
        s.fitness = fitness;
        s
    }

    // --- Config validation tests ---

    #[test]
    fn config_default_is_valid() {
        let cfg = CheckpointConfig::default();
        assert!(cfg.validate().is_ok());
        assert_eq!(cfg.max_retained, 20);
        assert_eq!(cfg.min_generation_gap, 5);
        assert!(cfg.auto_prune);
    }

    #[test]
    fn config_max_retained_zero_rejected() {
        let cfg = CheckpointConfig {
            max_retained: 0,
            ..CheckpointConfig::default()
        };
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn config_max_retained_one_accepted() {
        let cfg = CheckpointConfig {
            max_retained: 1,
            ..CheckpointConfig::default()
        };
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn new_with_invalid_config_fails() {
        let result = InMemoryCheckpointManager::new(CheckpointConfig {
            max_retained: 0,
            ..CheckpointConfig::default()
        });
        assert!(result.is_err());
    }

    #[test]
    fn with_defaults_succeeds() {
        let mgr = InMemoryCheckpointManager::with_defaults();
        assert!(mgr.is_ok());
    }

    // --- Save + Load roundtrip ---

    #[test]
    fn save_and_load_latest_roundtrip() {
        let mgr = make_manager();
        let snap = snapshot_gen(10);
        let id = mgr.save(snap).ok().unwrap_or_default();
        assert!(!id.is_empty());

        let loaded = mgr.load_latest().ok().flatten();
        assert!(loaded.is_some());
        let loaded = loaded.unwrap_or_else(|| unreachable!());
        assert_eq!(loaded.id, id);
        assert_eq!(loaded.generation, 10);
    }

    #[test]
    fn save_assigns_uuid() {
        let mgr = make_manager();
        let id = mgr.save(snapshot_gen(10)).ok().unwrap_or_default();
        // UUIDs are 36 chars (8-4-4-4-12)
        assert_eq!(id.len(), 36);
        assert_eq!(id.chars().filter(|c| *c == '-').count(), 4);
    }

    #[test]
    fn save_assigns_timestamp() {
        let mgr = make_manager();
        let _id = mgr.save(snapshot_gen(10)).ok();
        let loaded = mgr.load_latest().ok().flatten();
        assert!(loaded.is_some());
        let snap = loaded.unwrap_or_else(|| unreachable!());
        assert!(snap.saved_at().ticks() > 0);
    }

    #[test]
    fn load_latest_empty_returns_none() {
        let mgr = make_manager();
        let result = mgr.load_latest();
        assert!(result.is_ok());
        assert!(result.ok().flatten().is_none());
    }

    #[test]
    fn load_latest_returns_most_recent() {
        let mgr = make_manager();
        let _id1 = mgr.save(snapshot_gen(10)).ok();
        let id2 = mgr.save(snapshot_gen(20)).ok().unwrap_or_default();
        let loaded = mgr.load_latest().ok().flatten();
        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap_or_else(|| unreachable!()).id, id2);
    }

    // --- Load by generation ---

    #[test]
    fn load_by_generation_hit() {
        let mgr = make_manager();
        let _id = mgr.save(snapshot_gen(10)).ok();
        let _id = mgr.save(snapshot_gen(20)).ok();

        let found = mgr.load_by_generation(10).ok().flatten();
        assert!(found.is_some());
        assert_eq!(found.unwrap_or_else(|| unreachable!()).generation, 10);
    }

    #[test]
    fn load_by_generation_miss() {
        let mgr = make_manager();
        let _id = mgr.save(snapshot_gen(10)).ok();

        let found = mgr.load_by_generation(99).ok().flatten();
        assert!(found.is_none());
    }

    #[test]
    fn load_by_generation_empty() {
        let mgr = make_manager();
        let found = mgr.load_by_generation(0).ok().flatten();
        assert!(found.is_none());
    }

    // --- list_checkpoints ---

    #[test]
    fn list_checkpoints_newest_first() {
        let mgr = make_manager();
        let _id = mgr.save(snapshot_gen(10)).ok();
        let _id = mgr.save(snapshot_gen(20)).ok();
        let _id = mgr.save(snapshot_gen(30)).ok();

        let summaries = mgr.list_checkpoints(10);
        assert_eq!(summaries.len(), 3);
        assert_eq!(summaries[0].generation, 30);
        assert_eq!(summaries[1].generation, 20);
        assert_eq!(summaries[2].generation, 10);
    }

    #[test]
    fn list_checkpoints_respects_limit() {
        let mgr = make_manager();
        let _id = mgr.save(snapshot_gen(10)).ok();
        let _id = mgr.save(snapshot_gen(20)).ok();
        let _id = mgr.save(snapshot_gen(30)).ok();

        let summaries = mgr.list_checkpoints(2);
        assert_eq!(summaries.len(), 2);
        assert_eq!(summaries[0].generation, 30);
        assert_eq!(summaries[1].generation, 20);
    }

    #[test]
    fn list_checkpoints_empty() {
        let mgr = make_manager();
        let summaries = mgr.list_checkpoints(10);
        assert!(summaries.is_empty());
    }

    #[test]
    fn list_checkpoints_limit_zero() {
        let mgr = make_manager();
        let _id = mgr.save(snapshot_gen(10)).ok();
        let summaries = mgr.list_checkpoints(0);
        assert!(summaries.is_empty());
    }

    #[test]
    fn list_checkpoints_limit_exceeds_count() {
        let mgr = make_manager();
        let _id = mgr.save(snapshot_gen(10)).ok();
        let summaries = mgr.list_checkpoints(100);
        assert_eq!(summaries.len(), 1);
    }

    #[test]
    fn list_checkpoint_summary_fields() {
        let mgr = make_manager();
        let snap = snapshot_with_fitness(10, 0.85);
        let id = mgr.save(snap).ok().unwrap_or_default();

        let summaries = mgr.list_checkpoints(1);
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].id, id);
        assert_eq!(summaries[0].generation, 10);
        assert!((summaries[0].fitness - 0.85).abs() < f64::EPSILON);
        assert!(summaries[0].saved_at().ticks() > 0);
    }

    // --- prune_old ---

    #[test]
    fn prune_old_removes_oldest() {
        let mgr = make_manager_no_gap();
        for gen in 0..10 {
            let _id = mgr.save(snapshot_gen(gen)).ok();
        }
        assert_eq!(mgr.checkpoint_count(), 10);

        let removed = mgr.prune_old(3).ok().unwrap_or(0);
        assert_eq!(removed, 7);
        assert_eq!(mgr.checkpoint_count(), 3);

        // Remaining should be newest 3
        let summaries = mgr.list_checkpoints(10);
        assert_eq!(summaries[0].generation, 9);
        assert_eq!(summaries[1].generation, 8);
        assert_eq!(summaries[2].generation, 7);
    }

    #[test]
    fn prune_old_nothing_to_remove() {
        let mgr = make_manager();
        let _id = mgr.save(snapshot_gen(10)).ok();

        let removed = mgr.prune_old(5).ok().unwrap_or(0);
        assert_eq!(removed, 0);
        assert_eq!(mgr.checkpoint_count(), 1);
    }

    #[test]
    fn prune_old_keep_zero_rejected() {
        let mgr = make_manager();
        let result = mgr.prune_old(0);
        assert!(result.is_err());
    }

    #[test]
    fn prune_old_keep_equals_count() {
        let mgr = make_manager_no_gap();
        for gen in 0..5 {
            let _id = mgr.save(snapshot_gen(gen)).ok();
        }
        let removed = mgr.prune_old(5).ok().unwrap_or(99);
        assert_eq!(removed, 0);
        assert_eq!(mgr.checkpoint_count(), 5);
    }

    // --- checkpoint_count ---

    #[test]
    fn checkpoint_count_empty() {
        let mgr = make_manager();
        assert_eq!(mgr.checkpoint_count(), 0);
    }

    #[test]
    fn checkpoint_count_after_saves() {
        let mgr = make_manager_no_gap();
        let _id = mgr.save(snapshot_gen(0)).ok();
        let _id = mgr.save(snapshot_gen(1)).ok();
        let _id = mgr.save(snapshot_gen(2)).ok();
        assert_eq!(mgr.checkpoint_count(), 3);
    }

    // --- min_generation_gap enforcement ---

    #[test]
    fn min_generation_gap_skips_close_generations() {
        let mgr = make_manager(); // gap = 5
        let id1 = mgr.save(snapshot_gen(10)).ok().unwrap_or_default();

        // Generation 12 is only 2 apart — should return existing ID
        let id2 = mgr.save(snapshot_gen(12)).ok().unwrap_or_default();
        assert_eq!(id1, id2);
        assert_eq!(mgr.checkpoint_count(), 1);
    }

    #[test]
    fn min_generation_gap_allows_sufficient_gap() {
        let mgr = make_manager(); // gap = 5
        let id1 = mgr.save(snapshot_gen(10)).ok().unwrap_or_default();
        let id2 = mgr.save(snapshot_gen(15)).ok().unwrap_or_default();
        assert_ne!(id1, id2);
        assert_eq!(mgr.checkpoint_count(), 2);
    }

    #[test]
    fn min_generation_gap_exact_boundary() {
        let mgr = make_manager(); // gap = 5
        let id1 = mgr.save(snapshot_gen(10)).ok().unwrap_or_default();
        // Gap is exactly 5 — should be accepted
        let id2 = mgr.save(snapshot_gen(15)).ok().unwrap_or_default();
        assert_ne!(id1, id2);
    }

    #[test]
    fn min_generation_gap_just_below() {
        let mgr = make_manager(); // gap = 5
        let id1 = mgr.save(snapshot_gen(10)).ok().unwrap_or_default();
        // Gap is 4 — should be rejected
        let id2 = mgr.save(snapshot_gen(14)).ok().unwrap_or_default();
        assert_eq!(id1, id2);
        assert_eq!(mgr.checkpoint_count(), 1);
    }

    #[test]
    fn min_generation_gap_zero_allows_all() {
        let mgr = make_manager_no_gap(); // gap = 0
        let id1 = mgr.save(snapshot_gen(10)).ok().unwrap_or_default();
        let id2 = mgr.save(snapshot_gen(11)).ok().unwrap_or_default();
        assert_ne!(id1, id2);
        assert_eq!(mgr.checkpoint_count(), 2);
    }

    #[test]
    fn min_generation_gap_first_save_always_accepted() {
        let mgr = make_manager(); // gap = 5
        let result = mgr.save(snapshot_gen(1));
        assert!(result.is_ok());
        assert_eq!(mgr.checkpoint_count(), 1);
    }

    // --- fitness_history truncation ---

    #[test]
    fn fitness_history_truncated_to_50() {
        let mgr = make_manager();
        let mut snap = snapshot_gen(10);
        snap.fitness_history = (0..100).map(|i| f64::from(i) / 100.0).collect();
        assert_eq!(snap.fitness_history.len(), 100);

        let _id = mgr.save(snap).ok();
        let loaded = mgr.load_latest().ok().flatten();
        assert!(loaded.is_some());
        let loaded = loaded.unwrap_or_else(|| unreachable!());
        assert_eq!(loaded.fitness_history.len(), MAX_FITNESS_HISTORY);
        // Should keep the newest 50 (indices 50..100)
        assert!((loaded.fitness_history[0] - 0.50).abs() < f64::EPSILON);
    }

    #[test]
    fn fitness_history_under_limit_unchanged() {
        let mgr = make_manager();
        let mut snap = snapshot_gen(10);
        snap.fitness_history = vec![0.1, 0.2, 0.3];

        let _id = mgr.save(snap).ok();
        let loaded = mgr.load_latest().ok().flatten();
        assert!(loaded.is_some());
        assert_eq!(
            loaded.unwrap_or_else(|| unreachable!()).fitness_history.len(),
            3
        );
    }

    #[test]
    fn fitness_history_exactly_50_unchanged() {
        let mgr = make_manager();
        let mut snap = snapshot_gen(10);
        snap.fitness_history = (0..50).map(|i| f64::from(i) / 50.0).collect();

        let _id = mgr.save(snap).ok();
        let loaded = mgr.load_latest().ok().flatten();
        assert!(loaded.is_some());
        assert_eq!(
            loaded.unwrap_or_else(|| unreachable!()).fitness_history.len(),
            50
        );
    }

    // --- auto_prune ---

    #[test]
    fn auto_prune_trims_excess() {
        let mgr = InMemoryCheckpointManager::new(CheckpointConfig {
            max_retained: 3,
            min_generation_gap: 0,
            auto_prune: true,
        })
        .ok()
        .unwrap_or_else(|| unreachable!());

        for gen in 0..6 {
            let _id = mgr.save(snapshot_gen(gen)).ok();
        }

        // With auto_prune and max_retained=3, only 3 should remain
        assert_eq!(mgr.checkpoint_count(), 3);

        // The newest 3 should remain
        let summaries = mgr.list_checkpoints(10);
        assert_eq!(summaries[0].generation, 5);
        assert_eq!(summaries[1].generation, 4);
        assert_eq!(summaries[2].generation, 3);
    }

    #[test]
    fn auto_prune_disabled_keeps_all() {
        let mgr = InMemoryCheckpointManager::new(CheckpointConfig {
            max_retained: 3,
            min_generation_gap: 0,
            auto_prune: false,
        })
        .ok()
        .unwrap_or_else(|| unreachable!());

        for gen in 0..6 {
            let _id = mgr.save(snapshot_gen(gen)).ok();
        }

        // Without auto_prune, all 6 should remain
        assert_eq!(mgr.checkpoint_count(), 6);
    }

    // --- Concurrent access ---

    #[test]
    fn concurrent_saves_are_safe() {
        use std::sync::Arc;
        use std::thread;

        let mgr = Arc::new(
            InMemoryCheckpointManager::new(CheckpointConfig {
                max_retained: 200,
                min_generation_gap: 0,
                auto_prune: false,
            })
            .ok()
            .unwrap_or_else(|| unreachable!()),
        );

        let handles: Vec<_> = (0u64..10)
            .map(|i| {
                let mgr = Arc::clone(&mgr);
                thread::spawn(move || {
                    let gen = i * 100;
                    mgr.save(snapshot_gen(gen)).ok()
                })
            })
            .collect();

        for handle in handles {
            let _ = handle.join();
        }

        assert_eq!(mgr.checkpoint_count(), 10);
    }

    #[test]
    fn concurrent_reads_are_safe() {
        use std::sync::Arc;
        use std::thread;

        let mgr = Arc::new(make_manager_no_gap());
        for gen in 0..5 {
            let _id = mgr.save(snapshot_gen(gen)).ok();
        }

        let handles: Vec<_> = (0..10)
            .map(|_| {
                let mgr = Arc::clone(&mgr);
                thread::spawn(move || mgr.load_latest().ok().flatten().is_some())
            })
            .collect();

        for handle in handles {
            let result = handle.join();
            assert!(result.is_ok());
            assert!(result.ok().unwrap_or(false));
        }
    }

    // --- Large generation numbers ---

    #[test]
    fn large_generation_numbers() {
        let mgr = make_manager_no_gap();
        let big = u64::MAX - 1;
        let id = mgr.save(snapshot_gen(big)).ok().unwrap_or_default();
        assert!(!id.is_empty());

        let loaded = mgr.load_by_generation(big).ok().flatten();
        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap_or_else(|| unreachable!()).generation, big);
    }

    #[test]
    fn generation_zero() {
        let mgr = make_manager_no_gap();
        let id = mgr.save(snapshot_gen(0)).ok().unwrap_or_default();
        assert!(!id.is_empty());

        let loaded = mgr.load_by_generation(0).ok().flatten();
        assert!(loaded.is_some());
    }

    // --- Serialization roundtrip ---

    #[test]
    fn serde_json_roundtrip() {
        let mgr = make_manager();
        let mut snap = snapshot_with_fitness(100, 0.92);
        snap.mutation_count = 42;
        snap.accepted_count = 30;
        snap.rolled_back_count = 12;
        snap.cycle_number = 7;
        snap.current_phase = "Evaluate".into();
        snap.paused = true;
        snap.tensor_snapshot = [0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0, 0.0, 0.5];

        let id = mgr.save(snap).ok().unwrap_or_default();
        let loaded = mgr.load_latest().ok().flatten();
        assert!(loaded.is_some());
        let loaded = loaded.unwrap_or_else(|| unreachable!());

        let json = serde_json::to_string(&loaded);
        assert!(json.is_ok());
        let json = json.ok().unwrap_or_default();

        let deserialized: std::result::Result<CognitiveSnapshot, _> = serde_json::from_str(&json);
        assert!(deserialized.is_ok());
        let deserialized = deserialized.ok().unwrap_or_else(|| unreachable!());

        assert_eq!(deserialized.id, id);
        assert_eq!(deserialized.generation, 100);
        assert!((deserialized.fitness - 0.92).abs() < f64::EPSILON);
        assert_eq!(deserialized.mutation_count, 42);
        assert_eq!(deserialized.accepted_count, 30);
        assert_eq!(deserialized.rolled_back_count, 12);
        assert_eq!(deserialized.cycle_number, 7);
        assert_eq!(deserialized.current_phase, "Evaluate");
        assert!(deserialized.paused);
        assert!((deserialized.tensor_snapshot[0] - 0.1).abs() < f64::EPSILON);
    }

    #[test]
    fn serde_json_empty_snapshot() {
        let snap = default_snapshot(0);
        let json = serde_json::to_string(&snap);
        assert!(json.is_ok());
        let deserialized: std::result::Result<CognitiveSnapshot, _> =
            serde_json::from_str(&json.ok().unwrap_or_default());
        assert!(deserialized.is_ok());
    }

    // --- Snapshot field preservation ---

    #[test]
    fn all_fields_preserved_on_save_load() {
        let mgr = make_manager();
        let mut snap = default_snapshot(50);
        snap.fitness = 0.77;
        snap.fitness_history = vec![0.5, 0.6, 0.7, 0.77];
        snap.mutation_count = 100;
        snap.accepted_count = 80;
        snap.rolled_back_count = 20;
        snap.cycle_number = 15;
        snap.current_phase = "Mutate".into();
        snap.paused = true;
        snap.tensor_snapshot = [1.0; 12];

        let _id = mgr.save(snap).ok();
        let loaded = mgr.load_latest().ok().flatten();
        assert!(loaded.is_some());
        let loaded = loaded.unwrap_or_else(|| unreachable!());

        assert_eq!(loaded.generation, 50);
        assert!((loaded.fitness - 0.77).abs() < f64::EPSILON);
        assert_eq!(loaded.fitness_history, vec![0.5, 0.6, 0.7, 0.77]);
        assert_eq!(loaded.mutation_count, 100);
        assert_eq!(loaded.accepted_count, 80);
        assert_eq!(loaded.rolled_back_count, 20);
        assert_eq!(loaded.cycle_number, 15);
        assert_eq!(loaded.current_phase, "Mutate");
        assert!(loaded.paused);
        assert!((loaded.tensor_snapshot[11] - 1.0).abs() < f64::EPSILON);
    }

    // --- Debug impl ---

    #[test]
    fn debug_impl_works() {
        let mgr = make_manager();
        let debug_str = format!("{mgr:?}");
        assert!(debug_str.contains("InMemoryCheckpointManager"));
        assert!(debug_str.contains("checkpoint_count"));
    }

    // --- Trait object safety ---

    #[test]
    fn trait_object_usable() {
        let mgr = make_manager();
        let trait_obj: &dyn CheckpointManager = &mgr;
        assert_eq!(trait_obj.checkpoint_count(), 0);
        let _ = trait_obj.save(snapshot_gen(10));
        assert_eq!(trait_obj.checkpoint_count(), 1);
    }

    // --- Edge cases ---

    #[test]
    fn save_same_generation_twice_respects_gap() {
        let mgr = make_manager(); // gap = 5
        let id1 = mgr.save(snapshot_gen(10)).ok().unwrap_or_default();
        let id2 = mgr.save(snapshot_gen(10)).ok().unwrap_or_default();
        // Same generation means gap = 0 < 5, so second save returns first ID
        assert_eq!(id1, id2);
        assert_eq!(mgr.checkpoint_count(), 1);
    }

    #[test]
    fn prune_on_empty_returns_zero() {
        let mgr = make_manager();
        let removed = mgr.prune_old(5).ok().unwrap_or(99);
        assert_eq!(removed, 0);
    }

    #[test]
    fn multiple_prune_calls() {
        let mgr = make_manager_no_gap();
        for gen in 0..10 {
            let _id = mgr.save(snapshot_gen(gen)).ok();
        }

        let r1 = mgr.prune_old(7).ok().unwrap_or(0);
        assert_eq!(r1, 3);
        assert_eq!(mgr.checkpoint_count(), 7);

        let r2 = mgr.prune_old(3).ok().unwrap_or(0);
        assert_eq!(r2, 4);
        assert_eq!(mgr.checkpoint_count(), 3);
    }

    #[test]
    fn default_snapshot_helper() {
        let snap = default_snapshot(42);
        assert_eq!(snap.generation, 42);
        assert!((snap.fitness - 0.5).abs() < f64::EPSILON);
        assert!(snap.fitness_history.is_empty());
        assert_eq!(snap.mutation_count, 0);
        assert!(!snap.paused);
        assert_eq!(snap.tensor_snapshot, [0.0; 12]);
    }

    #[test]
    fn saved_at_timestamp_monotonic() {
        let mgr = make_manager_no_gap();
        let _id1 = mgr.save(snapshot_gen(0)).ok();
        let _id2 = mgr.save(snapshot_gen(1)).ok();

        let summaries = mgr.list_checkpoints(10);
        // Newest first
        assert!(summaries[0].saved_at().ticks() > summaries[1].saved_at().ticks());
    }

    #[test]
    fn snapshot_saved_at_accessor() {
        let mut snap = default_snapshot(0);
        snap.saved_at_ticks = 42;
        assert_eq!(snap.saved_at().ticks(), 42);
    }

    #[test]
    fn summary_saved_at_accessor() {
        let summary = CheckpointSummary {
            id: "test".into(),
            generation: 1,
            fitness: 0.5,
            saved_at_ticks: 99,
        };
        assert_eq!(summary.saved_at().ticks(), 99);
    }

    #[test]
    fn auto_prune_with_max_retained_one() {
        let mgr = InMemoryCheckpointManager::new(CheckpointConfig {
            max_retained: 1,
            min_generation_gap: 0,
            auto_prune: true,
        })
        .ok()
        .unwrap_or_else(|| unreachable!());

        for gen in 0..5 {
            let _id = mgr.save(snapshot_gen(gen)).ok();
        }

        assert_eq!(mgr.checkpoint_count(), 1);
        let loaded = mgr.load_latest().ok().flatten();
        assert_eq!(
            loaded.unwrap_or_else(|| unreachable!()).generation,
            4
        );
    }

    #[test]
    fn load_by_generation_after_prune() {
        let mgr = make_manager_no_gap();
        for gen in 0..10 {
            let _id = mgr.save(snapshot_gen(gen)).ok();
        }
        let _ = mgr.prune_old(3);

        // Pruned generations should not be found
        assert!(mgr.load_by_generation(0).ok().flatten().is_none());
        assert!(mgr.load_by_generation(5).ok().flatten().is_none());

        // Remaining generations should still be found
        assert!(mgr.load_by_generation(7).ok().flatten().is_some());
        assert!(mgr.load_by_generation(8).ok().flatten().is_some());
        assert!(mgr.load_by_generation(9).ok().flatten().is_some());
    }

    #[test]
    fn fitness_values_preserved_in_summary() {
        let mgr = make_manager_no_gap();
        let _id = mgr.save(snapshot_with_fitness(0, 0.1)).ok();
        let _id = mgr.save(snapshot_with_fitness(1, 0.9)).ok();

        let summaries = mgr.list_checkpoints(10);
        assert!((summaries[0].fitness - 0.9).abs() < f64::EPSILON);
        assert!((summaries[1].fitness - 0.1).abs() < f64::EPSILON);
    }

    #[test]
    fn prune_old_keep_one() {
        let mgr = make_manager_no_gap();
        for gen in 0..5 {
            let _id = mgr.save(snapshot_gen(gen)).ok();
        }
        let removed = mgr.prune_old(1).ok().unwrap_or(0);
        assert_eq!(removed, 4);
        assert_eq!(mgr.checkpoint_count(), 1);
        assert_eq!(
            mgr.load_latest()
                .ok()
                .flatten()
                .unwrap_or_else(|| unreachable!())
                .generation,
            4
        );
    }
}
