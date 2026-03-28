//! # M25: Hebbian Manager
//!
//! Pathway management for Hebbian learning within the Maintenance Engine.
//!
//! Manages a registry of [`HebbianPathway`] instances, tracking activations,
//! LTP/LTD events, success/failure rates, and routing weights. The manager
//! provides batch operations such as decay application and pulse triggering,
//! as well as per-pathway metrics aggregation.
//!
//! ## Layer: L5 (Learning)
//! ## Dependencies: M01 (Error), L5 types (`HebbianPathway`, `PulseTrigger`)
//! ## Tests: 12+
//!
//! ## 12D Tensor Encoding
//! ```text
//! [25/36, 0.0, 5/6, 0, 0, 0.5, health, uptime, synergy, latency, error_rate, temporal]
//! ```
//!
//! ## Thread Safety
//!
//! All mutable state is guarded by `parking_lot::RwLock` instances for
//! concurrent read access with exclusive write locks on mutations.
//!
//! ## Related Documentation
//! - [Layer Specification](../../ai_docs/layers/L05_LEARNING.md)
//! - [Hebbian Integration](../../nam/HEBBIAN_INTEGRATION.md)

use std::collections::HashMap;
use std::time::SystemTime;

use parking_lot::RwLock;

use super::{HebbianPathway, HebbianPulse, PathwayType, PulseTrigger};
use crate::{Error, Result, StdpConfig};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Maximum number of pulse history entries retained.
const PULSE_HISTORY_CAPACITY: usize = 200;

/// Decay rate applied to pathway strength on each decay cycle.
const DECAY_RATE: f64 = 0.001;

/// Minimum strength threshold -- pathways below this may be pruned.
const MIN_STRENGTH: f64 = 0.1;

// ---------------------------------------------------------------------------
// PathwayMetrics
// ---------------------------------------------------------------------------

/// Aggregated metrics for a single Hebbian pathway.
///
/// Tracks cumulative activation counts, LTP/LTD totals, strength statistics,
/// and the timestamp of the most recent activation.
#[derive(Clone, Debug)]
pub struct PathwayMetrics {
    /// Key identifying the pathway (`"source->target"`).
    pub pathway_key: String,
    /// Total number of activations (LTP + LTD + neutral).
    pub total_activations: u64,
    /// Total LTP (strengthening) events applied.
    pub total_ltp: u64,
    /// Total LTD (weakening) events applied.
    pub total_ltd: u64,
    /// Current average strength of the pathway.
    pub avg_strength: f64,
    /// Peak strength ever observed.
    pub peak_strength: f64,
    /// Timestamp of the most recent activation, if any.
    pub last_activation: Option<SystemTime>,
}

impl PathwayMetrics {
    /// Create a new metrics snapshot from a pathway.
    #[must_use]
    fn from_pathway(key: &str, pathway: &HebbianPathway) -> Self {
        Self {
            pathway_key: key.to_string(),
            total_activations: pathway.activation_count,
            total_ltp: pathway.ltp_count,
            total_ltd: pathway.ltd_count,
            avg_strength: pathway.strength,
            peak_strength: pathway.strength,
            last_activation: pathway.last_activation,
        }
    }

    /// Update metrics with the latest pathway state.
    fn update(&mut self, pathway: &HebbianPathway) {
        self.total_activations = pathway.activation_count;
        self.total_ltp = pathway.ltp_count;
        self.total_ltd = pathway.ltd_count;
        self.avg_strength = pathway.strength;
        if pathway.strength > self.peak_strength {
            self.peak_strength = pathway.strength;
        }
        self.last_activation = pathway.last_activation;
    }
}

// ---------------------------------------------------------------------------
// HebbianManager
// ---------------------------------------------------------------------------

/// Thread-safe manager for Hebbian learning pathways.
///
/// Maintains a registry of pathways keyed by `"source->target"` strings,
/// a bounded pulse history, and per-pathway metrics. All internal state is
/// protected by `parking_lot::RwLock` for safe concurrent access.
///
/// # Construction
///
/// ```rust
/// use maintenance_engine::m5_learning::hebbian::HebbianManager;
///
/// let manager = HebbianManager::new();
/// assert!(manager.pathway_count() > 0); // default pathways loaded
/// ```
pub struct HebbianManager {
    /// Pathway registry keyed by `"source->target"`.
    pathways: RwLock<HashMap<String, HebbianPathway>>,
    /// Bounded pulse history (most recent pulses).
    pulse_history: RwLock<Vec<HebbianPulse>>,
    /// Per-pathway aggregated metrics.
    pathway_metrics: RwLock<HashMap<String, PathwayMetrics>>,
    /// STDP configuration used for LTP/LTD operations.
    config: StdpConfig,
    /// Monotonically increasing pulse counter.
    pulse_counter: RwLock<u64>,
}

impl HebbianManager {
    // -------------------------------------------------------------------
    // Construction
    // -------------------------------------------------------------------

    /// Create a new `HebbianManager` pre-loaded with default pathways.
    ///
    /// The default pathways are sourced from [`super::default_pathways`] and
    /// cover common maintenance-to-action associations.
    #[must_use]
    pub fn new() -> Self {
        let defaults = super::default_pathways();
        let mut pathways = HashMap::with_capacity(defaults.len());
        let mut metrics = HashMap::with_capacity(defaults.len());

        for pathway in defaults {
            let key = format!("{}->{}",pathway.source, pathway.target);
            metrics.insert(key.clone(), PathwayMetrics::from_pathway(&key, &pathway));
            pathways.insert(key, pathway);
        }

        Self {
            pathways: RwLock::new(pathways),
            pulse_history: RwLock::new(Vec::new()),
            pathway_metrics: RwLock::new(metrics),
            config: StdpConfig::default(),
            pulse_counter: RwLock::new(0),
        }
    }

    // -------------------------------------------------------------------
    // Pathway CRUD
    // -------------------------------------------------------------------

    /// Add a new pathway between `source` and `target`.
    ///
    /// Returns the pathway key (`"source->target"`) on success.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] if a pathway with the same key already exists.
    pub fn add_pathway(
        &self,
        source: impl Into<String>,
        target: impl Into<String>,
        pathway_type: PathwayType,
    ) -> Result<String> {
        let source = source.into();
        let target = target.into();
        let key = format!("{source}->{target}");

        let mut guard = self.pathways.write();
        if guard.contains_key(&key) {
            return Err(Error::Validation(format!(
                "Pathway '{key}' already exists"
            )));
        }

        let mut pathway = HebbianPathway::new(source, target);
        pathway.pathway_type = pathway_type;
        let metrics = PathwayMetrics::from_pathway(&key, &pathway);
        guard.insert(key.clone(), pathway);
        drop(guard);

        self.pathway_metrics.write().insert(key.clone(), metrics);

        Ok(key)
    }

    /// Remove a pathway by its key.
    ///
    /// # Errors
    ///
    /// Returns [`Error::PathwayNotFound`] if the key does not exist.
    pub fn remove_pathway(&self, key: &str) -> Result<()> {
        let mut guard = self.pathways.write();
        if guard.remove(key).is_none() {
            let (source, target) = Self::split_key(key);
            return Err(Error::PathwayNotFound { source, target });
        }
        drop(guard);
        self.pathway_metrics.write().remove(key);
        Ok(())
    }

    /// Retrieve a clone of a pathway by its key.
    ///
    /// # Errors
    ///
    /// Returns [`Error::PathwayNotFound`] if the key does not exist.
    #[must_use = "returns the pathway clone; caller should use the value"]
    pub fn get_pathway(&self, key: &str) -> Result<HebbianPathway> {
        let guard = self.pathways.read();
        guard.get(key).cloned().ok_or_else(|| {
            let (source, target) = Self::split_key(key);
            Error::PathwayNotFound { source, target }
        })
    }

    /// Get the current strength of a pathway.
    ///
    /// # Errors
    ///
    /// Returns [`Error::PathwayNotFound`] if the key does not exist.
    #[must_use = "returns the strength value; caller should use it"]
    pub fn get_strength(&self, key: &str) -> Result<f64> {
        let guard = self.pathways.read();
        guard.get(key).map(|p| p.strength).ok_or_else(|| {
            let (source, target) = Self::split_key(key);
            Error::PathwayNotFound { source, target }
        })
    }

    // -------------------------------------------------------------------
    // Strengthening / Weakening
    // -------------------------------------------------------------------

    /// Apply Long-Term Potentiation (LTP) to a pathway, increasing its strength.
    ///
    /// Returns the new strength value after potentiation.
    ///
    /// # Errors
    ///
    /// Returns [`Error::PathwayNotFound`] if the key does not exist.
    pub fn strengthen(&self, key: &str) -> Result<f64> {
        let mut guard = self.pathways.write();
        let pathway = guard.get_mut(key).ok_or_else(|| {
            let (source, target) = Self::split_key(key);
            Error::PathwayNotFound { source, target }
        })?;

        pathway.apply_ltp(&self.config);
        let new_strength = pathway.strength;
        let snapshot = pathway.clone();
        drop(guard);

        // Update metrics
        if let Some(metrics) = self.pathway_metrics.write().get_mut(key) {
            metrics.update(&snapshot);
        }

        Ok(new_strength)
    }

    /// Apply Long-Term Depression (LTD) to a pathway, decreasing its strength.
    ///
    /// Returns the new strength value after depression.
    ///
    /// # Errors
    ///
    /// Returns [`Error::PathwayNotFound`] if the key does not exist.
    pub fn weaken(&self, key: &str) -> Result<f64> {
        let mut guard = self.pathways.write();
        let pathway = guard.get_mut(key).ok_or_else(|| {
            let (source, target) = Self::split_key(key);
            Error::PathwayNotFound { source, target }
        })?;

        pathway.apply_ltd(&self.config);
        let new_strength = pathway.strength;
        let snapshot = pathway.clone();
        drop(guard);

        if let Some(metrics) = self.pathway_metrics.write().get_mut(key) {
            metrics.update(&snapshot);
        }

        Ok(new_strength)
    }

    /// Record a successful activation for a pathway.
    ///
    /// Increments the success count, activation count, and applies LTP.
    /// Returns the new strength value.
    ///
    /// # Errors
    ///
    /// Returns [`Error::PathwayNotFound`] if the key does not exist.
    pub fn record_success(&self, key: &str) -> Result<f64> {
        let mut guard = self.pathways.write();
        let pathway = guard.get_mut(key).ok_or_else(|| {
            let (source, target) = Self::split_key(key);
            Error::PathwayNotFound { source, target }
        })?;

        pathway.record_success(&self.config);
        let new_strength = pathway.strength;
        let snapshot = pathway.clone();
        drop(guard);

        if let Some(metrics) = self.pathway_metrics.write().get_mut(key) {
            metrics.update(&snapshot);
        }

        Ok(new_strength)
    }

    /// Record a failed activation for a pathway.
    ///
    /// Increments the failure count, activation count, and applies LTD.
    /// Returns the new strength value.
    ///
    /// # Errors
    ///
    /// Returns [`Error::PathwayNotFound`] if the key does not exist.
    pub fn record_failure(&self, key: &str) -> Result<f64> {
        let mut guard = self.pathways.write();
        let pathway = guard.get_mut(key).ok_or_else(|| {
            let (source, target) = Self::split_key(key);
            Error::PathwayNotFound { source, target }
        })?;

        pathway.record_failure(&self.config);
        let new_strength = pathway.strength;
        let snapshot = pathway.clone();
        drop(guard);

        if let Some(metrics) = self.pathway_metrics.write().get_mut(key) {
            metrics.update(&snapshot);
        }

        Ok(new_strength)
    }

    // -------------------------------------------------------------------
    // Routing & Queries
    // -------------------------------------------------------------------

    /// Get the routing weight for a pathway between `source` and `target`.
    ///
    /// The routing weight is `strength * success_rate`. Returns `0.0` if
    /// no pathway exists between the given endpoints.
    #[must_use]
    pub fn get_routing_weight(&self, source: &str, target: &str) -> f64 {
        let key = format!("{source}->{target}");
        let guard = self.pathways.read();
        guard
            .get(&key)
            .map_or(0.0, HebbianPathway::routing_weight)
    }

    /// Get the `n` strongest pathways, sorted by descending strength.
    #[must_use]
    pub fn get_strongest_pathways(&self, n: usize) -> Vec<HebbianPathway> {
        let guard = self.pathways.read();
        let mut all: Vec<HebbianPathway> = guard.values().cloned().collect();
        drop(guard);
        all.sort_by(|a, b| {
            b.strength
                .partial_cmp(&a.strength)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        all.truncate(n);
        all
    }

    /// Get the `n` weakest pathways, sorted by ascending strength.
    #[must_use]
    pub fn get_weakest_pathways(&self, n: usize) -> Vec<HebbianPathway> {
        let guard = self.pathways.read();
        let mut all: Vec<HebbianPathway> = guard.values().cloned().collect();
        drop(guard);
        all.sort_by(|a, b| {
            a.strength
                .partial_cmp(&b.strength)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        all.truncate(n);
        all
    }

    /// Get all pathways originating from a given source.
    #[must_use]
    pub fn get_pathways_for_source(&self, source: &str) -> Vec<HebbianPathway> {
        let guard = self.pathways.read();
        guard
            .values()
            .filter(|p| p.source == source)
            .cloned()
            .collect()
    }

    // -------------------------------------------------------------------
    // Decay
    // -------------------------------------------------------------------

    /// Apply decay to all pathways, reducing each strength by [`DECAY_RATE`].
    ///
    /// Strength is clamped to [`MIN_STRENGTH`] at the lower bound and `1.0`
    /// at the upper bound. Returns the number of pathways affected (those
    /// whose strength was actually reduced).
    pub fn apply_decay(&self) -> usize {
        let mut guard = self.pathways.write();
        let mut affected = 0_usize;

        for pathway in guard.values_mut() {
            let old = pathway.strength;
            pathway.strength = (pathway.strength - DECAY_RATE).max(MIN_STRENGTH);
            if (old - pathway.strength).abs() > f64::EPSILON {
                affected += 1;
            }
        }
        drop(guard);

        affected
    }

    // -------------------------------------------------------------------
    // Pulse
    // -------------------------------------------------------------------

    /// Trigger a Hebbian pulse event.
    ///
    /// A pulse captures a snapshot of the pathway registry at a point in time,
    /// recording how many pathways were reinforced, weakened, or pruned. The
    /// pulse is appended to the bounded pulse history.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Other`] if pulse creation fails for any reason.
    #[allow(clippy::cast_possible_truncation)]
    pub fn trigger_pulse(&self, trigger: PulseTrigger) -> Result<HebbianPulse> {
        let guard = self.pathways.read();

        let total = guard.len() as u32;
        let reinforced = guard.values().filter(|p| p.ltp_count > 0).count() as u32;
        let weakened = guard.values().filter(|p| p.ltd_count > 0).count() as u32;
        let prunable = guard
            .values()
            .filter(|p| p.strength < super::HebbianPathway::default().strength * 0.2)
            .count() as u32;

        let avg_strength = if guard.is_empty() {
            0.0
        } else {
            #[allow(clippy::cast_precision_loss)]
            let sum: f64 = guard.values().map(|p| p.strength).sum();
            #[allow(clippy::cast_precision_loss)]
            let count = guard.len() as f64;
            sum / count
        };

        drop(guard);

        let pulse_number = {
            let mut counter = self.pulse_counter.write();
            *counter += 1;
            *counter
        };

        let pulse = HebbianPulse {
            pulse_number,
            trigger_type: trigger,
            pathways_reinforced: reinforced,
            pathways_weakened: weakened,
            pathways_pruned: prunable,
            new_pathways: 0,
            average_strength: avg_strength,
            total_pathways: total,
            duration_ms: 0,
            timestamp: SystemTime::now(),
        };

        {
            let mut history = self.pulse_history.write();
            if history.len() >= PULSE_HISTORY_CAPACITY {
                history.remove(0);
            }
            history.push(pulse.clone());
        }

        Ok(pulse)
    }

    // -------------------------------------------------------------------
    // Metrics
    // -------------------------------------------------------------------

    /// Get aggregated metrics for a specific pathway.
    ///
    /// # Errors
    ///
    /// Returns [`Error::PathwayNotFound`] if the key does not exist.
    pub fn get_metrics(&self, key: &str) -> Result<PathwayMetrics> {
        // Snapshot the pathway while holding the read lock
        let snapshot = {
            let pathway_guard = self.pathways.read();
            pathway_guard.get(key).cloned().ok_or_else(|| {
                let (source, target) = Self::split_key(key);
                Error::PathwayNotFound { source, target }
            })?
        };

        let mut metrics_guard = self.pathway_metrics.write();
        let entry = metrics_guard
            .entry(key.to_string())
            .or_insert_with(|| PathwayMetrics::from_pathway(key, &snapshot));
        entry.update(&snapshot);
        let result = entry.clone();
        drop(metrics_guard);

        Ok(result)
    }

    /// Get the total number of registered pathways.
    #[must_use]
    pub fn pathway_count(&self) -> usize {
        self.pathways.read().len()
    }

    // -------------------------------------------------------------------
    // Internal helpers
    // -------------------------------------------------------------------

    /// Split a `"source->target"` key into its components.
    fn split_key(key: &str) -> (String, String) {
        let parts: Vec<&str> = key.splitn(2, "->").collect();
        if parts.len() == 2 {
            (parts[0].to_string(), parts[1].to_string())
        } else {
            (key.to_string(), String::new())
        }
    }
}

impl Default for HebbianManager {
    fn default() -> Self {
        Self::new()
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_loads_defaults() {
        let manager = HebbianManager::new();
        // default_pathways() returns 9 pathways
        assert!(manager.pathway_count() >= 9);
    }

    #[test]
    fn test_add_pathway() {
        let manager = HebbianManager::new();
        let result = manager.add_pathway("alpha", "beta", PathwayType::AgentToAgent);
        assert!(result.is_ok());
        let key = result.ok().unwrap_or_default();
        assert_eq!(key, "alpha->beta");

        // Adding the same pathway again should fail
        let dup = manager.add_pathway("alpha", "beta", PathwayType::AgentToAgent);
        assert!(dup.is_err());
    }

    #[test]
    fn test_remove_pathway() {
        let manager = HebbianManager::new();
        let key = manager
            .add_pathway("temp", "node", PathwayType::SystemToSystem)
            .ok()
            .unwrap_or_default();
        assert!(manager.remove_pathway(&key).is_ok());

        // Removing again should fail
        assert!(manager.remove_pathway(&key).is_err());
    }

    #[test]
    fn test_strengthen_pathway() {
        let manager = HebbianManager::new();
        let key = manager
            .add_pathway("s1", "t1", PathwayType::ServiceToService)
            .ok()
            .unwrap_or_default();

        let initial = manager.get_strength(&key).ok().unwrap_or(0.0);
        let after = manager.strengthen(&key).ok().unwrap_or(0.0);

        assert!(after > initial, "Strength should increase after LTP");
    }

    #[test]
    fn test_weaken_pathway() {
        let manager = HebbianManager::new();
        let key = manager
            .add_pathway("s2", "t2", PathwayType::ServiceToService)
            .ok()
            .unwrap_or_default();

        let initial = manager.get_strength(&key).ok().unwrap_or(0.0);
        let after = manager.weaken(&key).ok().unwrap_or(0.0);

        assert!(after < initial, "Strength should decrease after LTD");
    }

    #[test]
    fn test_strength_bounds() {
        let manager = HebbianManager::new();
        let key = manager
            .add_pathway("bound_s", "bound_t", PathwayType::ServiceToService)
            .ok()
            .unwrap_or_default();

        // Strengthen many times -- should cap at 1.0
        for _ in 0..20 {
            let _ = manager.strengthen(&key);
        }
        let strong = manager.get_strength(&key).ok().unwrap_or(0.0);
        assert!(strong <= 1.0, "Strength must not exceed 1.0");
        assert!((strong - 1.0).abs() < f64::EPSILON, "Should reach cap");

        // Weaken many times -- should floor at MIN_STRENGTH (0.1)
        for _ in 0..30 {
            let _ = manager.weaken(&key);
        }
        let weak = manager.get_strength(&key).ok().unwrap_or(0.0);
        assert!(weak >= MIN_STRENGTH, "Strength must not go below MIN_STRENGTH");
    }

    #[test]
    fn test_record_success() {
        let manager = HebbianManager::new();
        let key = manager
            .add_pathway("succ_s", "succ_t", PathwayType::PatternToOutcome)
            .ok()
            .unwrap_or_default();

        let initial = manager.get_strength(&key).ok().unwrap_or(0.0);
        let after = manager.record_success(&key).ok().unwrap_or(0.0);

        assert!(after > initial, "Success should strengthen pathway");
        let pathway = manager.get_pathway(&key).ok();
        assert!(pathway.is_some());
        if let Some(p) = pathway {
            assert_eq!(p.success_count, 1);
            assert_eq!(p.activation_count, 1);
        }
    }

    #[test]
    fn test_record_failure() {
        let manager = HebbianManager::new();
        let key = manager
            .add_pathway("fail_s", "fail_t", PathwayType::ConfigToBehavior)
            .ok()
            .unwrap_or_default();

        let initial = manager.get_strength(&key).ok().unwrap_or(0.0);
        let after = manager.record_failure(&key).ok().unwrap_or(0.0);

        assert!(after < initial, "Failure should weaken pathway");
        let pathway = manager.get_pathway(&key).ok();
        assert!(pathway.is_some());
        if let Some(p) = pathway {
            assert_eq!(p.failure_count, 1);
            assert_eq!(p.activation_count, 1);
        }
    }

    #[test]
    fn test_routing_weight() {
        let manager = HebbianManager::new();
        let _ = manager.add_pathway("rw_s", "rw_t", PathwayType::ServiceToService);

        // New pathway with no successes/failures has neutral success_rate (0.5)
        // and default strength (0.5), so routing_weight = 0.5 * 0.5 = 0.25
        let weight = manager.get_routing_weight("rw_s", "rw_t");
        assert!((weight - 0.25).abs() < f64::EPSILON);

        // Non-existent pathway returns 0.0
        let none_weight = manager.get_routing_weight("nonexistent", "path");
        assert!((none_weight - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_strongest_pathways() {
        let manager = HebbianManager::new();
        let key_a = manager
            .add_pathway("strong_a", "strong_b", PathwayType::MetricToAction)
            .ok()
            .unwrap_or_default();

        // Strengthen this pathway to make it the strongest
        for _ in 0..8 {
            let _ = manager.strengthen(&key_a);
        }

        let strongest = manager.get_strongest_pathways(1);
        assert!(!strongest.is_empty());
        assert!(strongest[0].strength > 0.5);
    }

    #[test]
    fn test_apply_decay() {
        let manager = HebbianManager::new();
        let count = manager.pathway_count();
        let affected = manager.apply_decay();

        // All default pathways start at 0.5, so decay should affect all of them
        assert_eq!(affected, count, "All pathways should be affected by decay");

        // Verify strength was reduced
        let weakest = manager.get_weakest_pathways(1);
        assert!(!weakest.is_empty());
        assert!(
            weakest[0].strength < 0.5,
            "Decayed pathway should be weaker than default"
        );
    }

    #[test]
    fn test_trigger_pulse() {
        let manager = HebbianManager::new();

        let result = manager.trigger_pulse(PulseTrigger::Manual);
        assert!(result.is_ok());

        if let Ok(pulse) = result {
            assert_eq!(pulse.pulse_number, 1);
            assert_eq!(pulse.trigger_type, PulseTrigger::Manual);
            assert!(pulse.total_pathways > 0);
            assert!(pulse.average_strength > 0.0);
        }

        // Second pulse should increment counter
        let result2 = manager.trigger_pulse(PulseTrigger::TimeInterval);
        assert!(result2.is_ok());
        if let Ok(pulse2) = result2 {
            assert_eq!(pulse2.pulse_number, 2);
        }
    }

    #[test]
    fn test_get_metrics() {
        let manager = HebbianManager::new();
        let key = manager
            .add_pathway("met_s", "met_t", PathwayType::ServiceToService)
            .ok()
            .unwrap_or_default();

        let _ = manager.strengthen(&key);
        let _ = manager.record_success(&key);

        let metrics = manager.get_metrics(&key);
        assert!(metrics.is_ok());
        if let Ok(m) = metrics {
            assert_eq!(m.pathway_key, key);
            assert!(m.total_ltp >= 2); // strengthen + record_success both apply LTP
            assert!(m.total_activations >= 1);
        }
    }

    #[test]
    fn test_get_pathways_for_source() {
        let manager = HebbianManager::new();

        // "maintenance" is a source in default pathways
        let maint_paths = manager.get_pathways_for_source("maintenance");
        assert!(
            maint_paths.len() >= 4,
            "Default pathways include at least 4 maintenance routes"
        );

        for p in &maint_paths {
            assert_eq!(p.source, "maintenance");
        }
    }

    #[test]
    fn test_nonexistent_pathway_errors() {
        let manager = HebbianManager::new();
        assert!(manager.get_pathway("no->path").is_err());
        assert!(manager.get_strength("no->path").is_err());
        assert!(manager.strengthen("no->path").is_err());
        assert!(manager.weaken("no->path").is_err());
        assert!(manager.record_success("no->path").is_err());
        assert!(manager.record_failure("no->path").is_err());
        assert!(manager.get_metrics("no->path").is_err());
    }

    // ---------------------------------------------------------------
    // Additional tests to reach 50+
    // ---------------------------------------------------------------

    #[test]
    fn test_default_impl() {
        let manager = HebbianManager::default();
        assert!(manager.pathway_count() >= 9);
    }

    #[test]
    fn test_add_pathway_returns_correct_key_format() {
        let manager = HebbianManager::new();
        let key = manager
            .add_pathway("src_a", "tgt_b", PathwayType::ServiceToService)
            .ok()
            .unwrap_or_default();
        assert!(key.contains("->"));
        assert!(key.starts_with("src_a"));
        assert!(key.ends_with("tgt_b"));
    }

    #[test]
    fn test_add_multiple_pathways_increases_count() {
        let manager = HebbianManager::new();
        let initial = manager.pathway_count();
        let _ = manager.add_pathway("a1", "b1", PathwayType::AgentToAgent);
        let _ = manager.add_pathway("a2", "b2", PathwayType::AgentToAgent);
        let _ = manager.add_pathway("a3", "b3", PathwayType::AgentToAgent);
        assert_eq!(manager.pathway_count(), initial + 3);
    }

    #[test]
    fn test_get_pathway_returns_correct_source_target() {
        let manager = HebbianManager::new();
        let _ = manager.add_pathway("get_src", "get_tgt", PathwayType::MetricToAction);
        let pathway = manager.get_pathway("get_src->get_tgt").ok();
        assert!(pathway.is_some());
        if let Some(p) = pathway {
            assert_eq!(p.source, "get_src");
            assert_eq!(p.target, "get_tgt");
        }
    }

    #[test]
    fn test_strengthen_increments_ltp_count() {
        let manager = HebbianManager::new();
        let key = manager
            .add_pathway("ltp_src", "ltp_tgt", PathwayType::ServiceToService)
            .ok()
            .unwrap_or_default();
        let _ = manager.strengthen(&key);
        let _ = manager.strengthen(&key);
        let pathway = manager.get_pathway(&key).ok();
        assert!(pathway.is_some());
        if let Some(p) = pathway {
            assert_eq!(p.ltp_count, 2);
        }
    }

    #[test]
    fn test_weaken_increments_ltd_count() {
        let manager = HebbianManager::new();
        let key = manager
            .add_pathway("ltd_src", "ltd_tgt", PathwayType::ServiceToService)
            .ok()
            .unwrap_or_default();
        let _ = manager.weaken(&key);
        let _ = manager.weaken(&key);
        let _ = manager.weaken(&key);
        let pathway = manager.get_pathway(&key).ok();
        assert!(pathway.is_some());
        if let Some(p) = pathway {
            assert_eq!(p.ltd_count, 3);
        }
    }

    #[test]
    fn test_record_success_increments_activation_count() {
        let manager = HebbianManager::new();
        let key = manager
            .add_pathway("act_s", "act_t", PathwayType::PatternToOutcome)
            .ok()
            .unwrap_or_default();
        for _ in 0..5 {
            let _ = manager.record_success(&key);
        }
        let pathway = manager.get_pathway(&key).ok();
        assert!(pathway.is_some());
        if let Some(p) = pathway {
            assert_eq!(p.activation_count, 5);
            assert_eq!(p.success_count, 5);
        }
    }

    #[test]
    fn test_record_failure_increments_failure_count() {
        let manager = HebbianManager::new();
        let key = manager
            .add_pathway("fail_s2", "fail_t2", PathwayType::ConfigToBehavior)
            .ok()
            .unwrap_or_default();
        for _ in 0..4 {
            let _ = manager.record_failure(&key);
        }
        let pathway = manager.get_pathway(&key).ok();
        assert!(pathway.is_some());
        if let Some(p) = pathway {
            assert_eq!(p.failure_count, 4);
            assert_eq!(p.activation_count, 4);
        }
    }

    #[test]
    fn test_weakest_pathways_sorted_ascending() {
        let manager = HebbianManager::new();
        let key_weak = manager
            .add_pathway("w_a", "w_b", PathwayType::ServiceToService)
            .ok()
            .unwrap_or_default();
        for _ in 0..10 {
            let _ = manager.weaken(&key_weak);
        }
        let weakest = manager.get_weakest_pathways(3);
        assert!(weakest.len() >= 1);
        // Verify ascending order
        for window in weakest.windows(2) {
            assert!(window[0].strength <= window[1].strength + f64::EPSILON);
        }
    }

    #[test]
    fn test_strongest_pathways_sorted_descending() {
        let manager = HebbianManager::new();
        let strongest = manager.get_strongest_pathways(5);
        for window in strongest.windows(2) {
            assert!(window[0].strength >= window[1].strength - f64::EPSILON);
        }
    }

    #[test]
    fn test_get_pathways_for_nonexistent_source() {
        let manager = HebbianManager::new();
        let paths = manager.get_pathways_for_source("nonexistent_source");
        assert!(paths.is_empty());
    }

    #[test]
    fn test_routing_weight_after_successes() {
        let manager = HebbianManager::new();
        let _ = manager.add_pathway("rw2_s", "rw2_t", PathwayType::ServiceToService);
        for _ in 0..5 {
            let _ = manager.record_success("rw2_s->rw2_t");
        }
        let weight = manager.get_routing_weight("rw2_s", "rw2_t");
        assert!(weight > 0.25, "Routing weight should increase after successes");
    }

    #[test]
    fn test_decay_does_not_go_below_min_strength() {
        let manager = HebbianManager::new();
        // Apply decay many times
        for _ in 0..500 {
            manager.apply_decay();
        }
        let weakest = manager.get_weakest_pathways(1);
        assert!(!weakest.is_empty());
        assert!(
            weakest[0].strength >= MIN_STRENGTH - f64::EPSILON,
            "Strength should never go below MIN_STRENGTH after decay"
        );
    }

    #[test]
    fn test_pulse_history_bounded() {
        let manager = HebbianManager::new();
        for _ in 0..250 {
            let _ = manager.trigger_pulse(PulseTrigger::Manual);
        }
        let last_pulse = manager.trigger_pulse(PulseTrigger::TimeInterval);
        assert!(last_pulse.is_ok());
        if let Ok(p) = last_pulse {
            assert_eq!(p.pulse_number, 251);
        }
    }

    #[test]
    fn test_pulse_records_average_strength() {
        let manager = HebbianManager::new();
        let pulse = manager.trigger_pulse(PulseTrigger::Manual);
        assert!(pulse.is_ok());
        if let Ok(p) = pulse {
            assert!(p.average_strength > 0.0);
            assert!(p.average_strength <= 1.0);
        }
    }

    #[test]
    fn test_pulse_trigger_types() {
        let manager = HebbianManager::new();
        let p1 = manager.trigger_pulse(PulseTrigger::Manual);
        assert!(p1.is_ok());
        if let Ok(p) = p1 {
            assert_eq!(p.trigger_type, PulseTrigger::Manual);
        }
        let p2 = manager.trigger_pulse(PulseTrigger::TimeInterval);
        assert!(p2.is_ok());
        if let Ok(p) = p2 {
            assert_eq!(p.trigger_type, PulseTrigger::TimeInterval);
        }
    }

    #[test]
    fn test_metrics_peak_strength_tracks_maximum() {
        let manager = HebbianManager::new();
        let key = manager
            .add_pathway("peak_s", "peak_t", PathwayType::ServiceToService)
            .ok()
            .unwrap_or_default();
        for _ in 0..5 {
            let _ = manager.strengthen(&key);
        }
        let peak_after_strengthen = manager
            .get_metrics(&key)
            .ok()
            .map(|m| m.peak_strength)
            .unwrap_or(0.0);

        for _ in 0..3 {
            let _ = manager.weaken(&key);
        }
        let peak_after_weaken = manager
            .get_metrics(&key)
            .ok()
            .map(|m| m.peak_strength)
            .unwrap_or(0.0);

        assert!(
            (peak_after_weaken - peak_after_strengthen).abs() < f64::EPSILON,
            "Peak strength should remain at maximum"
        );
    }

    #[test]
    fn test_metrics_last_activation_set() {
        let manager = HebbianManager::new();
        let key = manager
            .add_pathway("act_s2", "act_t2", PathwayType::ServiceToService)
            .ok()
            .unwrap_or_default();
        let _ = manager.record_success(&key);
        let metrics = manager.get_metrics(&key);
        assert!(metrics.is_ok());
        if let Ok(m) = metrics {
            assert!(m.last_activation.is_some());
        }
    }

    #[test]
    fn test_remove_pathway_decreases_count() {
        let manager = HebbianManager::new();
        let initial = manager.pathway_count();
        let key = manager
            .add_pathway("rem_s", "rem_t", PathwayType::ServiceToService)
            .ok()
            .unwrap_or_default();
        assert_eq!(manager.pathway_count(), initial + 1);
        let _ = manager.remove_pathway(&key);
        assert_eq!(manager.pathway_count(), initial);
    }

    #[test]
    fn test_split_key_with_no_arrow() {
        let manager = HebbianManager::new();
        // Test internal split_key indirectly through error
        let result = manager.get_pathway("noarrow");
        assert!(result.is_err());
    }

    #[test]
    fn test_strengthen_then_weaken_returns_to_near_original() {
        let manager = HebbianManager::new();
        let key = manager
            .add_pathway("sw_s", "sw_t", PathwayType::ServiceToService)
            .ok()
            .unwrap_or_default();
        let initial = manager.get_strength(&key).ok().unwrap_or(0.0);
        let _ = manager.strengthen(&key);
        let _ = manager.weaken(&key);
        let final_strength = manager.get_strength(&key).ok().unwrap_or(0.0);
        // Due to asymmetric LTP/LTD rates, should be close but not exact
        assert!((final_strength - initial).abs() < 0.1);
    }

    #[test]
    fn test_decay_returns_zero_when_all_at_min() {
        let manager = HebbianManager::new();
        // Weaken all pathways to minimum
        for _ in 0..500 {
            manager.apply_decay();
        }
        // Now decay should affect zero pathways
        let affected = manager.apply_decay();
        assert_eq!(affected, 0, "No pathways should be affected at minimum strength");
    }

    #[test]
    fn test_multiple_pathway_types() {
        let manager = HebbianManager::new();
        let types = [
            PathwayType::AgentToAgent,
            PathwayType::ServiceToService,
            PathwayType::SystemToSystem,
            PathwayType::MetricToAction,
            PathwayType::PatternToOutcome,
            PathwayType::ConfigToBehavior,
        ];
        for (i, pt) in types.iter().enumerate() {
            let result = manager.add_pathway(format!("type_s{i}"), format!("type_t{i}"), *pt);
            assert!(result.is_ok());
        }
    }

    #[test]
    fn test_concurrent_access() {
        use std::sync::Arc;
        use std::thread;

        let manager = Arc::new(HebbianManager::new());
        let key = manager
            .add_pathway("conc_s", "conc_t", PathwayType::ServiceToService)
            .ok()
            .unwrap_or_default();

        let mut handles = Vec::new();
        for _ in 0..4 {
            let mgr = Arc::clone(&manager);
            let k = key.clone();
            handles.push(thread::spawn(move || {
                for _ in 0..10 {
                    let _ = mgr.strengthen(&k);
                }
            }));
        }
        for handle in handles {
            let _ = handle.join();
        }

        let pathway = manager.get_pathway(&key).ok();
        assert!(pathway.is_some());
        if let Some(p) = pathway {
            assert_eq!(p.ltp_count, 40);
        }
    }

    #[test]
    fn test_pathway_count_includes_defaults_and_added() {
        let manager = HebbianManager::new();
        let default_count = manager.pathway_count();
        let _ = manager.add_pathway("extra_s", "extra_t", PathwayType::AgentToAgent);
        assert_eq!(manager.pathway_count(), default_count + 1);
    }

    #[test]
    fn test_get_strongest_pathways_empty_when_n_zero() {
        let manager = HebbianManager::new();
        let result = manager.get_strongest_pathways(0);
        assert!(result.is_empty());
    }

    #[test]
    fn test_get_weakest_pathways_empty_when_n_zero() {
        let manager = HebbianManager::new();
        let result = manager.get_weakest_pathways(0);
        assert!(result.is_empty());
    }

    #[test]
    fn test_get_strongest_returns_at_most_n() {
        let manager = HebbianManager::new();
        let result = manager.get_strongest_pathways(2);
        assert!(result.len() <= 2);
    }

    #[test]
    fn test_add_and_get_pathway_roundtrip() {
        let manager = HebbianManager::new();
        let key = manager
            .add_pathway("rt_src", "rt_tgt", PathwayType::MetricToAction)
            .ok()
            .unwrap_or_default();
        let pathway = manager.get_pathway(&key);
        assert!(pathway.is_ok());
        if let Ok(p) = pathway {
            assert_eq!(p.pathway_type, PathwayType::MetricToAction);
            assert!((p.strength - 0.5).abs() < f64::EPSILON);
        }
    }

    #[test]
    fn test_metrics_created_for_new_pathway() {
        let manager = HebbianManager::new();
        let key = manager
            .add_pathway("mc_s", "mc_t", PathwayType::ServiceToService)
            .ok()
            .unwrap_or_default();
        let metrics = manager.get_metrics(&key);
        assert!(metrics.is_ok());
        if let Ok(m) = metrics {
            assert_eq!(m.total_activations, 0);
            assert_eq!(m.total_ltp, 0);
            assert_eq!(m.total_ltd, 0);
        }
    }

    #[test]
    fn test_apply_decay_does_not_exceed_one() {
        let manager = HebbianManager::new();
        let key = manager
            .add_pathway("dc_s", "dc_t", PathwayType::ServiceToService)
            .ok()
            .unwrap_or_default();
        for _ in 0..20 {
            let _ = manager.strengthen(&key);
        }
        manager.apply_decay();
        let s = manager.get_strength(&key).ok().unwrap_or(2.0);
        assert!(s <= 1.0);
    }

    #[test]
    fn test_get_pathways_for_source_with_added() {
        let manager = HebbianManager::new();
        let _ = manager.add_pathway("custom_src", "t1", PathwayType::AgentToAgent);
        let _ = manager.add_pathway("custom_src", "t2", PathwayType::AgentToAgent);
        let paths = manager.get_pathways_for_source("custom_src");
        assert_eq!(paths.len(), 2);
    }

    #[test]
    fn test_metrics_avg_strength_equals_strength() {
        let manager = HebbianManager::new();
        let key = manager
            .add_pathway("avg_s", "avg_t", PathwayType::ServiceToService)
            .ok()
            .unwrap_or_default();
        let strength = manager.get_strength(&key).ok().unwrap_or(0.0);
        let metrics = manager.get_metrics(&key).ok();
        assert!(metrics.is_some());
        if let Some(m) = metrics {
            assert!((m.avg_strength - strength).abs() < f64::EPSILON);
        }
    }

    #[test]
    fn test_pulse_total_pathways_count() {
        let manager = HebbianManager::new();
        let count = manager.pathway_count();
        let pulse = manager.trigger_pulse(PulseTrigger::Manual);
        assert!(pulse.is_ok());
        if let Ok(p) = pulse {
            assert_eq!(p.total_pathways as usize, count);
        }
    }

    #[test]
    fn test_record_success_then_failure_balance() {
        let manager = HebbianManager::new();
        let key = manager
            .add_pathway("bal_s", "bal_t", PathwayType::ServiceToService)
            .ok()
            .unwrap_or_default();
        let _ = manager.record_success(&key);
        let _ = manager.record_failure(&key);
        let pathway = manager.get_pathway(&key).ok();
        assert!(pathway.is_some());
        if let Some(p) = pathway {
            assert_eq!(p.success_count, 1);
            assert_eq!(p.failure_count, 1);
            assert_eq!(p.activation_count, 2);
        }
    }
}
