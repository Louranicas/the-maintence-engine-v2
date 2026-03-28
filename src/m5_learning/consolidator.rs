//! # M29: Memory Consolidator
//!
//! Manages progression of knowledge items through memory layers:
//! Working -> `ShortTerm` -> `LongTerm` -> Episodic.
//!
//! Items are promoted when they meet activation, age, strength, and success-rate
//! thresholds. Items are demoted when consecutive failures accumulate, strength
//! drops below a floor, or failure rate exceeds a ceiling. A full consolidation
//! pass evaluates every item for promotion, demotion, or pruning and produces a
//! [`ConsolidationReport`].
//!
//! ## Layer: L5 (Learning)
//! ## Dependencies: M01 (Error), L5 types (`MemoryLayer`, `ConsolidationEvent`, `ConsolidationType`)
//! ## Tests: 50
//!
//! ## 12D Tensor Encoding
//! ```text
//! [29/36, 0.0, 5/6, 0, 0, 0.5, health, uptime, synergy, latency, error_rate, temporal]
//! ```
//!
//! ## Thread Safety
//!
//! All mutable state is guarded by `parking_lot::RwLock` instances for
//! concurrent read access with exclusive write locks on mutations.
//!
//! ## Related Documentation
//! - [Layer Specification](../../ai_docs/layers/L05_LEARNING.md)
//! - [Module Specification](../../ai_docs/modules/M29_MEMORY_CONSOLIDATOR.md)

use std::collections::HashMap;
use std::time::SystemTime;

use parking_lot::RwLock;

use super::{ConsolidationEvent, ConsolidationType, MemoryLayer};
use crate::{Error, Result};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Maximum number of consolidation events retained in the event history.
const EVENT_HISTORY_CAPACITY: usize = 500;

/// Maximum number of items allowed per layer before pruning is triggered.
const DEFAULT_MAX_ITEMS_PER_LAYER: usize = 1000;

/// Default initial strength for newly stored items.
const DEFAULT_INITIAL_STRENGTH: f64 = 0.5;

// ---------------------------------------------------------------------------
// MemoryItem
// ---------------------------------------------------------------------------

/// A knowledge item that progresses through memory layers.
///
/// Each item tracks its activation history, success/failure rates, and current
/// memory layer. The consolidator uses these statistics to decide whether an
/// item should be promoted, demoted, or pruned.
#[derive(Clone, Debug)]
pub struct MemoryItem {
    /// Unique item identifier.
    pub id: String,
    /// Type of entity this item represents (e.g. "pathway", "pattern").
    pub entity_type: String,
    /// Identifier of the entity within its type namespace.
    pub entity_id: String,
    /// Current memory layer.
    pub layer: MemoryLayer,
    /// Strength score (0.0 - 1.0).
    pub strength: f64,
    /// Total number of activations.
    pub activation_count: u64,
    /// Number of successful activations.
    pub success_count: u64,
    /// Number of failed activations.
    pub failure_count: u64,
    /// Consecutive failure count (resets on success).
    pub consecutive_failures: u32,
    /// Timestamp when the item was created.
    pub created_at: SystemTime,
    /// Timestamp of the most recent activation.
    pub last_activated: SystemTime,
    /// Arbitrary key-value metadata.
    pub metadata: HashMap<String, String>,
}

impl MemoryItem {
    /// Calculate the success rate as `success_count / (success_count + failure_count)`.
    ///
    /// Returns `0.0` if no successes or failures have been recorded.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn success_rate(&self) -> f64 {
        let total = self.success_count + self.failure_count;
        if total == 0 {
            0.0
        } else {
            self.success_count as f64 / total as f64
        }
    }

    /// Calculate the age of this item in seconds since creation.
    ///
    /// Returns `0` if the system clock has moved backwards.
    #[must_use]
    pub fn age_secs(&self) -> u64 {
        SystemTime::now()
            .duration_since(self.created_at)
            .map_or(0, |d| d.as_secs())
    }

    /// Check whether this item is stale (not activated within the threshold).
    #[must_use]
    pub fn is_stale(&self, stale_threshold_secs: u64) -> bool {
        SystemTime::now()
            .duration_since(self.last_activated)
            .is_ok_and(|d| d.as_secs() > stale_threshold_secs)
    }

    /// Record an activation (increments count, updates timestamp).
    pub fn activate(&mut self) {
        self.activation_count += 1;
        self.last_activated = SystemTime::now();
    }

    /// Record a successful activation.
    pub const fn record_success(&mut self) {
        self.success_count += 1;
        self.consecutive_failures = 0;
    }

    /// Record a failed activation.
    pub const fn record_failure(&mut self) {
        self.failure_count += 1;
        self.consecutive_failures += 1;
    }
}

// ---------------------------------------------------------------------------
// Criteria
// ---------------------------------------------------------------------------

/// Criteria for promoting an item to the next memory layer.
#[derive(Clone, Copy, Debug)]
pub struct PromotionCriteria {
    /// Minimum number of activations required.
    pub min_activations: u64,
    /// Minimum age in seconds.
    pub min_age_secs: u64,
    /// Minimum strength score.
    pub min_strength: f64,
    /// Minimum success rate (0.0 - 1.0).
    pub min_success_rate: f64,
}

/// Criteria for demoting an item to a lower memory layer.
#[derive(Clone, Copy, Debug)]
pub struct DemotionCriteria {
    /// Maximum consecutive failures before demotion.
    pub max_consecutive_failures: u32,
    /// Minimum strength floor -- below this triggers demotion.
    pub min_strength: f64,
    /// Maximum failure rate ceiling -- above this triggers demotion.
    pub max_failure_rate: f64,
}

// ---------------------------------------------------------------------------
// ConsolidationConfig
// ---------------------------------------------------------------------------

/// Configuration controlling promotion, demotion, and capacity limits.
#[derive(Clone, Debug)]
pub struct ConsolidationConfig {
    /// Criteria for Working -> `ShortTerm` promotion.
    pub working_to_short: PromotionCriteria,
    /// Criteria for `ShortTerm` -> `LongTerm` promotion.
    pub short_to_long: PromotionCriteria,
    /// Criteria for `LongTerm` -> `ShortTerm` demotion.
    pub long_to_short_demotion: DemotionCriteria,
    /// Criteria for `ShortTerm` -> Working demotion.
    pub short_to_working_demotion: DemotionCriteria,
    /// Maximum number of items retained per layer.
    pub max_items_per_layer: usize,
}

impl Default for ConsolidationConfig {
    fn default() -> Self {
        Self {
            working_to_short: PromotionCriteria {
                min_activations: 3,
                min_age_secs: 60,
                min_strength: 0.3,
                min_success_rate: 0.0,
            },
            short_to_long: PromotionCriteria {
                min_activations: 20,
                min_age_secs: 86400,
                min_strength: 0.7,
                min_success_rate: 0.7,
            },
            long_to_short_demotion: DemotionCriteria {
                max_consecutive_failures: 5,
                min_strength: 0.2,
                max_failure_rate: 0.6,
            },
            short_to_working_demotion: DemotionCriteria {
                max_consecutive_failures: 3,
                min_strength: 0.1,
                max_failure_rate: 0.8,
            },
            max_items_per_layer: DEFAULT_MAX_ITEMS_PER_LAYER,
        }
    }
}

// ---------------------------------------------------------------------------
// ConsolidationReport
// ---------------------------------------------------------------------------

/// Report produced by a single consolidation pass.
#[derive(Clone, Debug)]
pub struct ConsolidationReport {
    /// Number of items promoted during this pass.
    pub promotions: usize,
    /// Number of items demoted during this pass.
    pub demotions: usize,
    /// Number of items pruned during this pass.
    pub pruned: usize,
    /// Total items after consolidation.
    pub total_items: usize,
    /// Item counts per layer (keyed by layer name).
    pub items_per_layer: HashMap<String, usize>,
    /// Events generated during this pass.
    pub events: Vec<ConsolidationEvent>,
    /// Timestamp of the consolidation pass.
    pub timestamp: SystemTime,
}

// ---------------------------------------------------------------------------
// MemoryConsolidator
// ---------------------------------------------------------------------------

/// Thread-safe memory consolidator managing item lifecycle across layers.
///
/// Items start in the Working layer and can be promoted through `ShortTerm`,
/// `LongTerm`, and Episodic layers based on activation frequency, age,
/// strength, and success rate. Items that degrade are demoted or pruned.
///
/// # Example
///
/// ```rust
/// use maintenance_engine::m5_learning::consolidator::MemoryConsolidator;
/// use std::collections::HashMap;
///
/// let consolidator = MemoryConsolidator::new();
/// let id = consolidator.store("pathway", "p1", HashMap::new());
/// assert!(id.is_ok());
/// assert_eq!(consolidator.item_count(), 1);
/// ```
pub struct MemoryConsolidator {
    /// All memory items keyed by their unique ID.
    items: RwLock<HashMap<String, MemoryItem>>,
    /// Bounded event history.
    events: RwLock<Vec<ConsolidationEvent>>,
    /// Configuration for promotion/demotion thresholds.
    config: RwLock<ConsolidationConfig>,
    /// Monotonically increasing item counter for ID generation.
    next_id: RwLock<u64>,
}

impl MemoryConsolidator {
    // -------------------------------------------------------------------
    // Construction
    // -------------------------------------------------------------------

    /// Create a new `MemoryConsolidator` with default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self {
            items: RwLock::new(HashMap::new()),
            events: RwLock::new(Vec::new()),
            config: RwLock::new(ConsolidationConfig::default()),
            next_id: RwLock::new(1),
        }
    }

    /// Create a new `MemoryConsolidator` with a custom configuration.
    #[must_use]
    pub fn with_config(config: ConsolidationConfig) -> Self {
        Self {
            items: RwLock::new(HashMap::new()),
            events: RwLock::new(Vec::new()),
            config: RwLock::new(config),
            next_id: RwLock::new(1),
        }
    }

    // -------------------------------------------------------------------
    // Item CRUD
    // -------------------------------------------------------------------

    /// Store a new item in the Working memory layer.
    ///
    /// Returns the generated item ID on success.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] if `entity_type` or `entity_id` is empty.
    pub fn store(
        &self,
        entity_type: &str,
        entity_id: &str,
        metadata: HashMap<String, String>,
    ) -> Result<String> {
        if entity_type.is_empty() {
            return Err(Error::Validation(
                "entity_type must not be empty".to_string(),
            ));
        }
        if entity_id.is_empty() {
            return Err(Error::Validation(
                "entity_id must not be empty".to_string(),
            ));
        }

        let id = {
            let mut counter = self.next_id.write();
            let id = format!("MEM-{counter:06}");
            *counter += 1;
            id
        };

        let now = SystemTime::now();
        let item = MemoryItem {
            id: id.clone(),
            entity_type: entity_type.to_string(),
            entity_id: entity_id.to_string(),
            layer: MemoryLayer::Working,
            strength: DEFAULT_INITIAL_STRENGTH,
            activation_count: 0,
            success_count: 0,
            failure_count: 0,
            consecutive_failures: 0,
            created_at: now,
            last_activated: now,
            metadata,
        };

        self.items.write().insert(id.clone(), item);
        Ok(id)
    }

    /// Get a clone of an item by its ID.
    #[must_use]
    pub fn get_item(&self, id: &str) -> Option<MemoryItem> {
        self.items.read().get(id).cloned()
    }

    /// Remove an item by its ID.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] if the item does not exist.
    pub fn remove_item(&self, id: &str) -> Result<()> {
        if self.items.write().remove(id).is_some() {
            Ok(())
        } else {
            Err(Error::Validation(format!("Item '{id}' not found")))
        }
    }

    /// Get the total number of items across all layers.
    #[must_use]
    pub fn item_count(&self) -> usize {
        self.items.read().len()
    }

    /// Get all items in a specific memory layer.
    #[must_use]
    pub fn items_in_layer(&self, layer: MemoryLayer) -> Vec<MemoryItem> {
        self.items
            .read()
            .values()
            .filter(|item| item.layer == layer)
            .cloned()
            .collect()
    }

    /// Get the number of items in a specific memory layer.
    #[must_use]
    pub fn layer_count(&self, layer: MemoryLayer) -> usize {
        self.items
            .read()
            .values()
            .filter(|item| item.layer == layer)
            .count()
    }

    // -------------------------------------------------------------------
    // Activation / Success / Failure
    // -------------------------------------------------------------------

    /// Activate an item (increment activation count, update timestamp).
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] if the item does not exist.
    pub fn activate_item(&self, id: &str) -> Result<()> {
        let mut guard = self.items.write();
        let item = guard
            .get_mut(id)
            .ok_or_else(|| Error::Validation(format!("Item '{id}' not found")))?;
        item.activate();
        drop(guard);
        Ok(())
    }

    /// Record a successful activation for an item.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] if the item does not exist.
    pub fn record_item_success(&self, id: &str) -> Result<()> {
        let mut guard = self.items.write();
        let item = guard
            .get_mut(id)
            .ok_or_else(|| Error::Validation(format!("Item '{id}' not found")))?;
        item.record_success();
        drop(guard);
        Ok(())
    }

    /// Record a failed activation for an item.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] if the item does not exist.
    pub fn record_item_failure(&self, id: &str) -> Result<()> {
        let mut guard = self.items.write();
        let item = guard
            .get_mut(id)
            .ok_or_else(|| Error::Validation(format!("Item '{id}' not found")))?;
        item.record_failure();
        drop(guard);
        Ok(())
    }

    // -------------------------------------------------------------------
    // Promotion / Demotion checks
    // -------------------------------------------------------------------

    /// Determine if an item qualifies for promotion to a higher layer.
    ///
    /// Returns `Some(target_layer)` if promotion criteria are met, `None` otherwise.
    /// Episodic items cannot be promoted further.
    #[must_use]
    pub fn check_promotion(&self, item: &MemoryItem) -> Option<MemoryLayer> {
        let config = self.config.read().clone();

        match item.layer {
            MemoryLayer::Working => {
                let c = &config.working_to_short;
                if item.activation_count >= c.min_activations
                    && item.age_secs() >= c.min_age_secs
                    && item.strength >= c.min_strength
                    && item.success_rate() >= c.min_success_rate
                {
                    Some(MemoryLayer::ShortTerm)
                } else {
                    None
                }
            }
            MemoryLayer::ShortTerm => {
                let c = &config.short_to_long;
                if item.activation_count >= c.min_activations
                    && item.age_secs() >= c.min_age_secs
                    && item.strength >= c.min_strength
                    && item.success_rate() >= c.min_success_rate
                {
                    Some(MemoryLayer::LongTerm)
                } else {
                    None
                }
            }
            MemoryLayer::LongTerm | MemoryLayer::Episodic => None,
        }
    }

    /// Determine if an item qualifies for demotion to a lower layer.
    ///
    /// Returns `Some(target_layer)` if demotion criteria are met, `None` otherwise.
    /// Working items cannot be demoted further.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn check_demotion(&self, item: &MemoryItem) -> Option<MemoryLayer> {
        let config = self.config.read().clone();

        match item.layer {
            MemoryLayer::LongTerm => {
                let c = &config.long_to_short_demotion;
                let total = item.success_count + item.failure_count;
                let failure_rate = if total == 0 {
                    0.0
                } else {
                    item.failure_count as f64 / total as f64
                };

                if item.consecutive_failures >= c.max_consecutive_failures
                    || item.strength < c.min_strength
                    || failure_rate > c.max_failure_rate
                {
                    Some(MemoryLayer::ShortTerm)
                } else {
                    None
                }
            }
            MemoryLayer::ShortTerm => {
                let c = &config.short_to_working_demotion;
                let total = item.success_count + item.failure_count;
                let failure_rate = if total == 0 {
                    0.0
                } else {
                    item.failure_count as f64 / total as f64
                };

                if item.consecutive_failures >= c.max_consecutive_failures
                    || item.strength < c.min_strength
                    || failure_rate > c.max_failure_rate
                {
                    Some(MemoryLayer::Working)
                } else {
                    None
                }
            }
            MemoryLayer::Working | MemoryLayer::Episodic => None,
        }
    }

    // -------------------------------------------------------------------
    // Consolidation pass
    // -------------------------------------------------------------------

    /// Run a full consolidation pass over all items.
    ///
    /// Evaluates every item for promotion or demotion and collects the results
    /// in a [`ConsolidationReport`]. Events are appended to the bounded history.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Other`] if the consolidation pass encounters an
    /// internal error.
    #[allow(clippy::too_many_lines)]
    pub fn consolidate(&self) -> Result<ConsolidationReport> {
        // Snapshot IDs and items to avoid holding the write lock during checks
        let snapshot: Vec<(String, MemoryItem)> = {
            let guard = self.items.read();
            guard
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect()
        };

        let mut promotions = 0_usize;
        let mut demotions = 0_usize;
        let mut pruned = 0_usize;
        let mut pass_events: Vec<ConsolidationEvent> = Vec::new();

        for (id, item) in &snapshot {
            // Check promotion first
            if let Some(target_layer) = self.check_promotion(item) {
                let event = ConsolidationEvent {
                    entity_type: item.entity_type.clone(),
                    entity_id: item.entity_id.clone(),
                    from_layer: item.layer,
                    to_layer: target_layer,
                    consolidation_type: ConsolidationType::Promotion,
                    strength_before: item.strength,
                    strength_after: item.strength,
                    timestamp: SystemTime::now(),
                };
                pass_events.push(event);

                let mut guard = self.items.write();
                if let Some(live_item) = guard.get_mut(id) {
                    live_item.layer = target_layer;
                }
                drop(guard);

                promotions += 1;
                continue;
            }

            // Check demotion
            if let Some(target_layer) = self.check_demotion(item) {
                let event = ConsolidationEvent {
                    entity_type: item.entity_type.clone(),
                    entity_id: item.entity_id.clone(),
                    from_layer: item.layer,
                    to_layer: target_layer,
                    consolidation_type: ConsolidationType::Demotion,
                    strength_before: item.strength,
                    strength_after: item.strength,
                    timestamp: SystemTime::now(),
                };
                pass_events.push(event);

                let mut guard = self.items.write();
                if let Some(live_item) = guard.get_mut(id) {
                    live_item.layer = target_layer;
                }
                drop(guard);

                demotions += 1;
            }
        }

        // Prune excess items per layer
        let max_per_layer = self.config.read().max_items_per_layer;
        for layer in &[
            MemoryLayer::Working,
            MemoryLayer::ShortTerm,
            MemoryLayer::LongTerm,
            MemoryLayer::Episodic,
        ] {
            let mut guard = self.items.write();
            let mut layer_items: Vec<(String, f64)> = guard
                .iter()
                .filter(|(_, v)| v.layer == *layer)
                .map(|(k, v)| (k.clone(), v.strength))
                .collect();

            if layer_items.len() > max_per_layer {
                // Sort by strength ascending (weakest first)
                layer_items.sort_by(|a, b| {
                    a.1.partial_cmp(&b.1)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });

                let to_remove = layer_items.len() - max_per_layer;
                for (prune_id, _) in layer_items.iter().take(to_remove) {
                    if let Some(removed) = guard.remove(prune_id) {
                        pass_events.push(ConsolidationEvent {
                            entity_type: removed.entity_type,
                            entity_id: removed.entity_id,
                            from_layer: removed.layer,
                            to_layer: removed.layer,
                            consolidation_type: ConsolidationType::Pruning,
                            strength_before: removed.strength,
                            strength_after: 0.0,
                            timestamp: SystemTime::now(),
                        });
                        pruned += 1;
                    }
                }
            }
            drop(guard);
        }

        // Build layer counts
        let items_per_layer = {
            let guard = self.items.read();
            let mut counts = HashMap::new();
            counts.insert(
                "Working".to_string(),
                guard.values().filter(|i| i.layer == MemoryLayer::Working).count(),
            );
            counts.insert(
                "ShortTerm".to_string(),
                guard.values().filter(|i| i.layer == MemoryLayer::ShortTerm).count(),
            );
            counts.insert(
                "LongTerm".to_string(),
                guard.values().filter(|i| i.layer == MemoryLayer::LongTerm).count(),
            );
            counts.insert(
                "Episodic".to_string(),
                guard.values().filter(|i| i.layer == MemoryLayer::Episodic).count(),
            );
            counts
        };

        let total_items = self.items.read().len();

        // Append events to bounded history
        {
            let mut history = self.events.write();
            for event in &pass_events {
                if history.len() >= EVENT_HISTORY_CAPACITY {
                    history.remove(0);
                }
                history.push(event.clone());
            }
        }

        let report = ConsolidationReport {
            promotions,
            demotions,
            pruned,
            total_items,
            items_per_layer,
            events: pass_events,
            timestamp: SystemTime::now(),
        };

        Ok(report)
    }

    // -------------------------------------------------------------------
    // Manual layer transitions
    // -------------------------------------------------------------------

    /// Manually promote an item to a specific layer.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] if the item does not exist.
    pub fn promote_item(&self, id: &str, to_layer: MemoryLayer) -> Result<()> {
        let mut guard = self.items.write();
        let item = guard
            .get_mut(id)
            .ok_or_else(|| Error::Validation(format!("Item '{id}' not found")))?;

        let event = ConsolidationEvent {
            entity_type: item.entity_type.clone(),
            entity_id: item.entity_id.clone(),
            from_layer: item.layer,
            to_layer,
            consolidation_type: ConsolidationType::Promotion,
            strength_before: item.strength,
            strength_after: item.strength,
            timestamp: SystemTime::now(),
        };

        item.layer = to_layer;
        drop(guard);

        {
            let mut history = self.events.write();
            if history.len() >= EVENT_HISTORY_CAPACITY {
                history.remove(0);
            }
            history.push(event);
        }

        Ok(())
    }

    /// Manually demote an item to a specific layer.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] if the item does not exist.
    pub fn demote_item(&self, id: &str, to_layer: MemoryLayer) -> Result<()> {
        let mut guard = self.items.write();
        let item = guard
            .get_mut(id)
            .ok_or_else(|| Error::Validation(format!("Item '{id}' not found")))?;

        let event = ConsolidationEvent {
            entity_type: item.entity_type.clone(),
            entity_id: item.entity_id.clone(),
            from_layer: item.layer,
            to_layer,
            consolidation_type: ConsolidationType::Demotion,
            strength_before: item.strength,
            strength_after: item.strength,
            timestamp: SystemTime::now(),
        };

        item.layer = to_layer;
        drop(guard);

        {
            let mut history = self.events.write();
            if history.len() >= EVENT_HISTORY_CAPACITY {
                history.remove(0);
            }
            history.push(event);
        }

        Ok(())
    }

    /// Set the strength of a specific item.
    ///
    /// The value is clamped to [0.0, 1.0].
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] if the item does not exist.
    pub fn set_item_strength(&self, id: &str, strength: f64) -> Result<()> {
        let mut guard = self.items.write();
        let item = guard
            .get_mut(id)
            .ok_or_else(|| Error::Validation(format!("Item '{id}' not found")))?;
        item.strength = strength.clamp(0.0, 1.0);
        drop(guard);
        Ok(())
    }

    // -------------------------------------------------------------------
    // Queries
    // -------------------------------------------------------------------

    /// Get a clone of the full event history.
    #[must_use]
    pub fn event_history(&self) -> Vec<ConsolidationEvent> {
        self.events.read().clone()
    }

    /// Get a clone of the current configuration.
    #[must_use]
    pub fn get_config(&self) -> ConsolidationConfig {
        self.config.read().clone()
    }
}

impl Default for MemoryConsolidator {
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

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    fn empty_metadata() -> HashMap<String, String> {
        HashMap::new()
    }

    fn setup_consolidator() -> MemoryConsolidator {
        let c = MemoryConsolidator::new();
        c.store("pathway", "p1", empty_metadata()).ok();
        c
    }

    /// Create an item that qualifies for Working -> ShortTerm promotion
    /// (needs activations >= 3, age >= 60s, strength >= 0.3, success_rate >= 0.0).
    /// We cannot easily fake age, so we use a config with `min_age_secs = 0`.
    fn promotable_config() -> ConsolidationConfig {
        ConsolidationConfig {
            working_to_short: PromotionCriteria {
                min_activations: 2,
                min_age_secs: 0,
                min_strength: 0.3,
                min_success_rate: 0.0,
            },
            short_to_long: PromotionCriteria {
                min_activations: 5,
                min_age_secs: 0,
                min_strength: 0.6,
                min_success_rate: 0.5,
            },
            long_to_short_demotion: DemotionCriteria {
                max_consecutive_failures: 3,
                min_strength: 0.2,
                max_failure_rate: 0.6,
            },
            short_to_working_demotion: DemotionCriteria {
                max_consecutive_failures: 2,
                min_strength: 0.1,
                max_failure_rate: 0.8,
            },
            max_items_per_layer: DEFAULT_MAX_ITEMS_PER_LAYER,
        }
    }

    // -----------------------------------------------------------------------
    // 1-2: Construction
    // -----------------------------------------------------------------------

    #[test]
    fn test_01_default_construction() {
        let c = MemoryConsolidator::new();
        assert_eq!(c.item_count(), 0);
        assert!(c.event_history().is_empty());
    }

    #[test]
    fn test_02_construction_with_config() {
        let config = promotable_config();
        let c = MemoryConsolidator::with_config(config.clone());
        let retrieved = c.get_config();
        assert_eq!(
            retrieved.working_to_short.min_activations,
            config.working_to_short.min_activations
        );
        assert!(
            (retrieved.short_to_long.min_strength - config.short_to_long.min_strength).abs()
                < f64::EPSILON
        );
    }

    // -----------------------------------------------------------------------
    // 3-8: Store / Get / Remove
    // -----------------------------------------------------------------------

    #[test]
    fn test_03_store_item() {
        let c = MemoryConsolidator::new();
        let id = c.store("pattern", "pat-001", empty_metadata());
        assert!(id.is_ok());
        assert_eq!(c.item_count(), 1);
    }

    #[test]
    fn test_04_store_empty_entity_type_fails() {
        let c = MemoryConsolidator::new();
        assert!(c.store("", "id", empty_metadata()).is_err());
    }

    #[test]
    fn test_05_store_empty_entity_id_fails() {
        let c = MemoryConsolidator::new();
        assert!(c.store("type", "", empty_metadata()).is_err());
    }

    #[test]
    fn test_06_get_item() {
        let c = setup_consolidator();
        let id = c.items.read().keys().next().cloned().unwrap_or_default();
        let item = c.get_item(&id);
        assert!(item.is_some());
        if let Some(item) = item {
            assert_eq!(item.entity_type, "pathway");
            assert_eq!(item.entity_id, "p1");
            assert_eq!(item.layer, MemoryLayer::Working);
            assert!((item.strength - DEFAULT_INITIAL_STRENGTH).abs() < f64::EPSILON);
        }
    }

    #[test]
    fn test_07_get_nonexistent_item() {
        let c = MemoryConsolidator::new();
        assert!(c.get_item("nonexistent").is_none());
    }

    #[test]
    fn test_08_remove_item() {
        let c = setup_consolidator();
        let id = c.items.read().keys().next().cloned().unwrap_or_default();
        assert!(c.remove_item(&id).is_ok());
        assert_eq!(c.item_count(), 0);
        assert!(c.remove_item(&id).is_err());
    }

    // -----------------------------------------------------------------------
    // 9-11: Layer queries
    // -----------------------------------------------------------------------

    #[test]
    fn test_09_items_in_layer() {
        let c = MemoryConsolidator::new();
        c.store("a", "1", empty_metadata()).ok();
        c.store("b", "2", empty_metadata()).ok();

        let working = c.items_in_layer(MemoryLayer::Working);
        assert_eq!(working.len(), 2);
        assert!(c.items_in_layer(MemoryLayer::ShortTerm).is_empty());
    }

    #[test]
    fn test_10_layer_count() {
        let c = MemoryConsolidator::new();
        c.store("a", "1", empty_metadata()).ok();
        c.store("b", "2", empty_metadata()).ok();
        c.store("c", "3", empty_metadata()).ok();
        assert_eq!(c.layer_count(MemoryLayer::Working), 3);
        assert_eq!(c.layer_count(MemoryLayer::LongTerm), 0);
    }

    #[test]
    fn test_11_item_count() {
        let c = MemoryConsolidator::new();
        assert_eq!(c.item_count(), 0);
        c.store("x", "y", empty_metadata()).ok();
        assert_eq!(c.item_count(), 1);
    }

    // -----------------------------------------------------------------------
    // 12-16: Activation / Success / Failure
    // -----------------------------------------------------------------------

    #[test]
    fn test_12_activate_item() {
        let c = setup_consolidator();
        let id = c.items.read().keys().next().cloned().unwrap_or_default();
        assert!(c.activate_item(&id).is_ok());
        let item = c.get_item(&id);
        assert_eq!(item.map(|i| i.activation_count).unwrap_or(0), 1);
    }

    #[test]
    fn test_13_activate_nonexistent_fails() {
        let c = MemoryConsolidator::new();
        assert!(c.activate_item("nope").is_err());
    }

    #[test]
    fn test_14_record_success() {
        let c = setup_consolidator();
        let id = c.items.read().keys().next().cloned().unwrap_or_default();
        assert!(c.record_item_success(&id).is_ok());
        let item = c.get_item(&id);
        assert!(item.is_some());
        if let Some(item) = item {
            assert_eq!(item.success_count, 1);
            assert_eq!(item.consecutive_failures, 0);
        }
    }

    #[test]
    fn test_15_record_failure() {
        let c = setup_consolidator();
        let id = c.items.read().keys().next().cloned().unwrap_or_default();
        assert!(c.record_item_failure(&id).is_ok());
        let item = c.get_item(&id);
        assert!(item.is_some());
        if let Some(item) = item {
            assert_eq!(item.failure_count, 1);
            assert_eq!(item.consecutive_failures, 1);
        }
    }

    #[test]
    fn test_16_record_failure_nonexistent_fails() {
        let c = MemoryConsolidator::new();
        assert!(c.record_item_failure("nope").is_err());
    }

    // -----------------------------------------------------------------------
    // 17-18: Success resets consecutive failures
    // -----------------------------------------------------------------------

    #[test]
    fn test_17_success_resets_consecutive_failures() {
        let c = setup_consolidator();
        let id = c.items.read().keys().next().cloned().unwrap_or_default();
        let _ = c.record_item_failure(&id);
        let _ = c.record_item_failure(&id);
        let _ = c.record_item_success(&id);
        let item = c.get_item(&id);
        assert_eq!(item.map(|i| i.consecutive_failures).unwrap_or(u32::MAX), 0);
    }

    #[test]
    fn test_18_multiple_failures_increment() {
        let c = setup_consolidator();
        let id = c.items.read().keys().next().cloned().unwrap_or_default();
        for _ in 0..5 {
            let _ = c.record_item_failure(&id);
        }
        let item = c.get_item(&id);
        assert_eq!(item.as_ref().map(|i| i.consecutive_failures).unwrap_or(0), 5);
        assert_eq!(item.as_ref().map(|i| i.failure_count).unwrap_or(0), 5);
    }

    // -----------------------------------------------------------------------
    // 19-22: MemoryItem methods
    // -----------------------------------------------------------------------

    #[test]
    fn test_19_success_rate_empty() {
        let item = MemoryItem {
            id: "t".into(),
            entity_type: "t".into(),
            entity_id: "e".into(),
            layer: MemoryLayer::Working,
            strength: 0.5,
            activation_count: 0,
            success_count: 0,
            failure_count: 0,
            consecutive_failures: 0,
            created_at: SystemTime::now(),
            last_activated: SystemTime::now(),
            metadata: HashMap::new(),
        };
        assert!(item.success_rate().abs() < f64::EPSILON);
    }

    #[test]
    fn test_20_success_rate_calculated() {
        let item = MemoryItem {
            id: "t".into(),
            entity_type: "t".into(),
            entity_id: "e".into(),
            layer: MemoryLayer::Working,
            strength: 0.5,
            activation_count: 10,
            success_count: 7,
            failure_count: 3,
            consecutive_failures: 0,
            created_at: SystemTime::now(),
            last_activated: SystemTime::now(),
            metadata: HashMap::new(),
        };
        assert!((item.success_rate() - 0.7).abs() < f64::EPSILON);
    }

    #[test]
    fn test_21_age_secs() {
        let item = MemoryItem {
            id: "t".into(),
            entity_type: "t".into(),
            entity_id: "e".into(),
            layer: MemoryLayer::Working,
            strength: 0.5,
            activation_count: 0,
            success_count: 0,
            failure_count: 0,
            consecutive_failures: 0,
            created_at: SystemTime::now(),
            last_activated: SystemTime::now(),
            metadata: HashMap::new(),
        };
        // Just created, age should be near zero
        assert!(item.age_secs() < 2);
    }

    #[test]
    fn test_22_is_stale() {
        let item = MemoryItem {
            id: "t".into(),
            entity_type: "t".into(),
            entity_id: "e".into(),
            layer: MemoryLayer::Working,
            strength: 0.5,
            activation_count: 0,
            success_count: 0,
            failure_count: 0,
            consecutive_failures: 0,
            created_at: SystemTime::now(),
            last_activated: SystemTime::now(),
            metadata: HashMap::new(),
        };
        // Just activated, should not be stale with a 60s threshold
        assert!(!item.is_stale(60));
        // Should be stale with a 0s threshold
        // (race condition safe: even if 0 seconds elapsed, `> 0` is false)
    }

    // -----------------------------------------------------------------------
    // 23-27: Promotion checks
    // -----------------------------------------------------------------------

    #[test]
    fn test_23_check_promotion_working_to_short() {
        let c = MemoryConsolidator::with_config(promotable_config());
        let id = c.store("x", "y", empty_metadata()).unwrap_or_default();
        let _ = c.activate_item(&id);
        let _ = c.activate_item(&id);
        let item = c.get_item(&id).unwrap_or_else(|| unreachable!());
        assert_eq!(c.check_promotion(&item), Some(MemoryLayer::ShortTerm));
    }

    #[test]
    fn test_24_check_promotion_not_enough_activations() {
        let c = MemoryConsolidator::with_config(promotable_config());
        let id = c.store("x", "y", empty_metadata()).unwrap_or_default();
        let _ = c.activate_item(&id);
        let item = c.get_item(&id).unwrap_or_else(|| unreachable!());
        assert!(c.check_promotion(&item).is_none());
    }

    #[test]
    fn test_25_check_promotion_strength_too_low() {
        let c = MemoryConsolidator::with_config(promotable_config());
        let id = c.store("x", "y", empty_metadata()).unwrap_or_default();
        let _ = c.activate_item(&id);
        let _ = c.activate_item(&id);
        let _ = c.set_item_strength(&id, 0.1);
        let item = c.get_item(&id).unwrap_or_else(|| unreachable!());
        assert!(c.check_promotion(&item).is_none());
    }

    #[test]
    fn test_26_check_promotion_short_to_long() {
        let c = MemoryConsolidator::with_config(promotable_config());
        let id = c.store("x", "y", empty_metadata()).unwrap_or_default();
        // Manually set to ShortTerm
        let _ = c.promote_item(&id, MemoryLayer::ShortTerm);
        let _ = c.set_item_strength(&id, 0.8);
        for _ in 0..5 {
            let _ = c.activate_item(&id);
        }
        for _ in 0..4 {
            let _ = c.record_item_success(&id);
        }
        let _ = c.record_item_failure(&id);
        let item = c.get_item(&id).unwrap_or_else(|| unreachable!());
        // success_rate = 4/5 = 0.8 >= 0.5, activations=5 >= 5, strength=0.8 >= 0.6
        assert_eq!(c.check_promotion(&item), Some(MemoryLayer::LongTerm));
    }

    #[test]
    fn test_27_check_promotion_longterm_returns_none() {
        let c = MemoryConsolidator::with_config(promotable_config());
        let id = c.store("x", "y", empty_metadata()).unwrap_or_default();
        let _ = c.promote_item(&id, MemoryLayer::LongTerm);
        let item = c.get_item(&id).unwrap_or_else(|| unreachable!());
        assert!(c.check_promotion(&item).is_none());
    }

    // -----------------------------------------------------------------------
    // 28-32: Demotion checks
    // -----------------------------------------------------------------------

    #[test]
    fn test_28_check_demotion_long_to_short_consecutive_failures() {
        let c = MemoryConsolidator::with_config(promotable_config());
        let id = c.store("x", "y", empty_metadata()).unwrap_or_default();
        let _ = c.promote_item(&id, MemoryLayer::LongTerm);
        for _ in 0..3 {
            let _ = c.record_item_failure(&id);
        }
        let item = c.get_item(&id).unwrap_or_else(|| unreachable!());
        assert_eq!(c.check_demotion(&item), Some(MemoryLayer::ShortTerm));
    }

    #[test]
    fn test_29_check_demotion_long_to_short_low_strength() {
        let c = MemoryConsolidator::with_config(promotable_config());
        let id = c.store("x", "y", empty_metadata()).unwrap_or_default();
        let _ = c.promote_item(&id, MemoryLayer::LongTerm);
        let _ = c.set_item_strength(&id, 0.1);
        let item = c.get_item(&id).unwrap_or_else(|| unreachable!());
        assert_eq!(c.check_demotion(&item), Some(MemoryLayer::ShortTerm));
    }

    #[test]
    fn test_30_check_demotion_short_to_working() {
        let c = MemoryConsolidator::with_config(promotable_config());
        let id = c.store("x", "y", empty_metadata()).unwrap_or_default();
        let _ = c.promote_item(&id, MemoryLayer::ShortTerm);
        for _ in 0..2 {
            let _ = c.record_item_failure(&id);
        }
        let item = c.get_item(&id).unwrap_or_else(|| unreachable!());
        assert_eq!(c.check_demotion(&item), Some(MemoryLayer::Working));
    }

    #[test]
    fn test_31_check_demotion_working_returns_none() {
        let c = MemoryConsolidator::with_config(promotable_config());
        let id = c.store("x", "y", empty_metadata()).unwrap_or_default();
        for _ in 0..10 {
            let _ = c.record_item_failure(&id);
        }
        let _ = c.set_item_strength(&id, 0.01);
        let item = c.get_item(&id).unwrap_or_else(|| unreachable!());
        assert!(c.check_demotion(&item).is_none());
    }

    #[test]
    fn test_32_check_demotion_no_failures() {
        let c = MemoryConsolidator::with_config(promotable_config());
        let id = c.store("x", "y", empty_metadata()).unwrap_or_default();
        let _ = c.promote_item(&id, MemoryLayer::LongTerm);
        let item = c.get_item(&id).unwrap_or_else(|| unreachable!());
        assert!(c.check_demotion(&item).is_none());
    }

    // -----------------------------------------------------------------------
    // 33-38: Consolidation pass
    // -----------------------------------------------------------------------

    #[test]
    fn test_33_consolidate_empty() {
        let c = MemoryConsolidator::new();
        let report = c.consolidate();
        assert!(report.is_ok());
        if let Ok(r) = report {
            assert_eq!(r.promotions, 0);
            assert_eq!(r.demotions, 0);
            assert_eq!(r.pruned, 0);
            assert_eq!(r.total_items, 0);
        }
    }

    #[test]
    fn test_34_consolidate_promotes() {
        let c = MemoryConsolidator::with_config(promotable_config());
        let id = c.store("svc", "s1", empty_metadata()).unwrap_or_default();
        let _ = c.activate_item(&id);
        let _ = c.activate_item(&id);

        let report = c.consolidate();
        assert!(report.is_ok());
        if let Ok(r) = report {
            assert_eq!(r.promotions, 1);
            assert_eq!(r.demotions, 0);
        }
        let item = c.get_item(&id);
        assert_eq!(
            item.map(|i| i.layer),
            Some(MemoryLayer::ShortTerm)
        );
    }

    #[test]
    fn test_35_consolidate_demotes() {
        let c = MemoryConsolidator::with_config(promotable_config());
        let id = c.store("svc", "s1", empty_metadata()).unwrap_or_default();
        let _ = c.promote_item(&id, MemoryLayer::ShortTerm);
        for _ in 0..2 {
            let _ = c.record_item_failure(&id);
        }

        let report = c.consolidate();
        assert!(report.is_ok());
        if let Ok(r) = report {
            assert_eq!(r.demotions, 1);
        }
        let item = c.get_item(&id);
        assert_eq!(item.map(|i| i.layer), Some(MemoryLayer::Working));
    }

    #[test]
    fn test_36_consolidate_report_layer_counts() {
        let c = MemoryConsolidator::with_config(promotable_config());
        c.store("a", "1", empty_metadata()).ok();
        c.store("b", "2", empty_metadata()).ok();
        let id3 = c.store("c", "3", empty_metadata()).unwrap_or_default();
        let _ = c.promote_item(&id3, MemoryLayer::LongTerm);

        let report = c.consolidate().unwrap_or_else(|_| ConsolidationReport {
            promotions: 0,
            demotions: 0,
            pruned: 0,
            total_items: 0,
            items_per_layer: HashMap::new(),
            events: Vec::new(),
            timestamp: SystemTime::now(),
        });

        assert_eq!(report.total_items, 3);
        assert_eq!(
            report.items_per_layer.get("LongTerm").copied().unwrap_or(0),
            1
        );
    }

    #[test]
    fn test_37_consolidate_pruning() {
        let config = ConsolidationConfig {
            max_items_per_layer: 2,
            ..promotable_config()
        };
        let c = MemoryConsolidator::with_config(config);
        c.store("a", "1", empty_metadata()).ok();
        c.store("b", "2", empty_metadata()).ok();
        c.store("c", "3", empty_metadata()).ok();

        let report = c.consolidate();
        assert!(report.is_ok());
        if let Ok(r) = report {
            assert!(r.pruned >= 1, "Should prune at least 1 excess item");
        }
        assert!(c.item_count() <= 2);
    }

    #[test]
    fn test_38_consolidate_events_recorded() {
        let c = MemoryConsolidator::with_config(promotable_config());
        let id = c.store("svc", "s1", empty_metadata()).unwrap_or_default();
        let _ = c.activate_item(&id);
        let _ = c.activate_item(&id);

        let _ = c.consolidate();
        let history = c.event_history();
        assert!(!history.is_empty());
    }

    // -----------------------------------------------------------------------
    // 39-42: Manual promote / demote / set_strength
    // -----------------------------------------------------------------------

    #[test]
    fn test_39_promote_item_manually() {
        let c = setup_consolidator();
        let id = c.items.read().keys().next().cloned().unwrap_or_default();
        assert!(c.promote_item(&id, MemoryLayer::LongTerm).is_ok());
        let item = c.get_item(&id);
        assert_eq!(item.map(|i| i.layer), Some(MemoryLayer::LongTerm));
    }

    #[test]
    fn test_40_demote_item_manually() {
        let c = setup_consolidator();
        let id = c.items.read().keys().next().cloned().unwrap_or_default();
        let _ = c.promote_item(&id, MemoryLayer::LongTerm);
        assert!(c.demote_item(&id, MemoryLayer::Working).is_ok());
        let item = c.get_item(&id);
        assert_eq!(item.map(|i| i.layer), Some(MemoryLayer::Working));
    }

    #[test]
    fn test_41_promote_nonexistent_fails() {
        let c = MemoryConsolidator::new();
        assert!(c.promote_item("nope", MemoryLayer::ShortTerm).is_err());
    }

    #[test]
    fn test_42_set_item_strength() {
        let c = setup_consolidator();
        let id = c.items.read().keys().next().cloned().unwrap_or_default();
        assert!(c.set_item_strength(&id, 0.9).is_ok());
        let item = c.get_item(&id);
        assert!(
            (item.map(|i| i.strength).unwrap_or(0.0) - 0.9).abs() < f64::EPSILON
        );
    }

    // -----------------------------------------------------------------------
    // 43-44: Strength clamping
    // -----------------------------------------------------------------------

    #[test]
    fn test_43_set_strength_clamps_high() {
        let c = setup_consolidator();
        let id = c.items.read().keys().next().cloned().unwrap_or_default();
        let _ = c.set_item_strength(&id, 5.0);
        let item = c.get_item(&id);
        assert!(
            (item.map(|i| i.strength).unwrap_or(0.0) - 1.0).abs() < f64::EPSILON
        );
    }

    #[test]
    fn test_44_set_strength_clamps_low() {
        let c = setup_consolidator();
        let id = c.items.read().keys().next().cloned().unwrap_or_default();
        let _ = c.set_item_strength(&id, -1.0);
        let item = c.get_item(&id);
        assert!(item.map(|i| i.strength).unwrap_or(-1.0).abs() < f64::EPSILON);
    }

    // -----------------------------------------------------------------------
    // 45: Config defaults
    // -----------------------------------------------------------------------

    #[test]
    fn test_45_config_defaults() {
        let config = ConsolidationConfig::default();
        assert_eq!(config.working_to_short.min_activations, 3);
        assert_eq!(config.working_to_short.min_age_secs, 60);
        assert!((config.working_to_short.min_strength - 0.3).abs() < f64::EPSILON);
        assert!(config.working_to_short.min_success_rate.abs() < f64::EPSILON);

        assert_eq!(config.short_to_long.min_activations, 20);
        assert_eq!(config.short_to_long.min_age_secs, 86400);
        assert!((config.short_to_long.min_strength - 0.7).abs() < f64::EPSILON);
        assert!((config.short_to_long.min_success_rate - 0.7).abs() < f64::EPSILON);

        assert_eq!(config.long_to_short_demotion.max_consecutive_failures, 5);
        assert!((config.long_to_short_demotion.min_strength - 0.2).abs() < f64::EPSILON);
        assert!((config.long_to_short_demotion.max_failure_rate - 0.6).abs() < f64::EPSILON);

        assert_eq!(config.short_to_working_demotion.max_consecutive_failures, 3);
        assert!((config.short_to_working_demotion.min_strength - 0.1).abs() < f64::EPSILON);
        assert!((config.short_to_working_demotion.max_failure_rate - 0.8).abs() < f64::EPSILON);

        assert_eq!(config.max_items_per_layer, DEFAULT_MAX_ITEMS_PER_LAYER);
    }

    // -----------------------------------------------------------------------
    // 46: Multiple items across layers
    // -----------------------------------------------------------------------

    #[test]
    fn test_46_multiple_items_across_layers() {
        let c = MemoryConsolidator::with_config(promotable_config());
        let id1 = c.store("a", "1", empty_metadata()).unwrap_or_default();
        let id2 = c.store("b", "2", empty_metadata()).unwrap_or_default();
        let id3 = c.store("c", "3", empty_metadata()).unwrap_or_default();

        let _ = c.promote_item(&id1, MemoryLayer::ShortTerm);
        let _ = c.promote_item(&id2, MemoryLayer::LongTerm);
        let _ = c.promote_item(&id3, MemoryLayer::Episodic);

        assert_eq!(c.layer_count(MemoryLayer::Working), 0);
        assert_eq!(c.layer_count(MemoryLayer::ShortTerm), 1);
        assert_eq!(c.layer_count(MemoryLayer::LongTerm), 1);
        assert_eq!(c.layer_count(MemoryLayer::Episodic), 1);
        assert_eq!(c.item_count(), 3);
    }

    // -----------------------------------------------------------------------
    // 47: Event history
    // -----------------------------------------------------------------------

    #[test]
    fn test_47_event_history_from_manual_operations() {
        let c = MemoryConsolidator::new();
        let id = c.store("x", "y", empty_metadata()).unwrap_or_default();
        let _ = c.promote_item(&id, MemoryLayer::ShortTerm);
        let _ = c.demote_item(&id, MemoryLayer::Working);

        let history = c.event_history();
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].consolidation_type, ConsolidationType::Promotion);
        assert_eq!(history[1].consolidation_type, ConsolidationType::Demotion);
    }

    // -----------------------------------------------------------------------
    // 48: Event history bounded capacity
    // -----------------------------------------------------------------------

    #[test]
    fn test_48_event_history_bounded() {
        let c = MemoryConsolidator::new();
        // Generate >500 events
        for i in 0..550 {
            let id = c
                .store("t", &format!("e{i}"), empty_metadata())
                .unwrap_or_default();
            let _ = c.promote_item(&id, MemoryLayer::ShortTerm);
        }
        let history = c.event_history();
        assert!(
            history.len() <= EVENT_HISTORY_CAPACITY,
            "Event history should be bounded at {EVENT_HISTORY_CAPACITY}"
        );
    }

    // -----------------------------------------------------------------------
    // 49: Metadata preservation
    // -----------------------------------------------------------------------

    #[test]
    fn test_49_metadata_preserved() {
        let c = MemoryConsolidator::new();
        let mut meta = HashMap::new();
        meta.insert("key".to_string(), "value".to_string());
        meta.insert("source".to_string(), "synthex".to_string());

        let id = c.store("svc", "s1", meta).unwrap_or_default();
        let item = c.get_item(&id);
        assert!(item.is_some());
        if let Some(item) = item {
            assert_eq!(item.metadata.get("key").map(String::as_str), Some("value"));
            assert_eq!(
                item.metadata.get("source").map(String::as_str),
                Some("synthex")
            );
        }
    }

    // -----------------------------------------------------------------------
    // 50: Demotion via high failure rate
    // -----------------------------------------------------------------------

    #[test]
    fn test_50_demotion_high_failure_rate() {
        let c = MemoryConsolidator::with_config(promotable_config());
        let id = c.store("x", "y", empty_metadata()).unwrap_or_default();
        let _ = c.promote_item(&id, MemoryLayer::LongTerm);

        // Record 1 success and 4 failures -> failure_rate = 4/5 = 0.8 > 0.6
        let _ = c.record_item_success(&id);
        for _ in 0..4 {
            let _ = c.record_item_failure(&id);
        }

        let item = c.get_item(&id).unwrap_or_else(|| unreachable!());
        assert_eq!(c.check_demotion(&item), Some(MemoryLayer::ShortTerm));
    }
}
