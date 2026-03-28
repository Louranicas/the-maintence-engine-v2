//! # N02: Intent Router
//!
//! 12D [`IntentTensor`] to service routing decisions.
//!
//! ## Layer: L8 (Nexus Integration)
//! ## Module: N02
//! ## Dependencies: L1 (Error, Timestamp)
//!
//! ## Trait
//!
//! | Trait | Purpose |
//! |-------|---------|
//! | [`IntentRouter`] | Route intents to services via weighted dot product |
//!
//! All methods are `&self` (C2) with interior mutability via `RwLock`.
//! Methods returning data through `RwLock` return OWNED types (C7).
//!
//! ## Routing Algorithm
//!
//! For each active service, compute:
//! ```text
//! score = sum(intent.dimensions[i] * service.dimension_weights[i]) * service.capacity
//! ```
//!
//! Return the highest-scoring service. If the best score is below
//! [`IntentRouterConfig::min_score_threshold`], return an error.
//!
//! ## 12D Tensor Integration
//!
//! The [`IntentTensor`] carries 12 dimensions matching the ME V2 tensor
//! encoding: `service_id`, `port`, `tier`, `deps`, `agents`, `protocol`, `health`,
//! `uptime`, `synergy`, `latency`, `error_rate`, `temporal_context`.
//!
//! ## Related Documentation
//! - [Nexus Specs](../../ai_specs/nexus-specs/)

use std::collections::{HashMap, VecDeque};
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};

use parking_lot::RwLock;

use crate::m1_foundation::shared_types::Timestamp;
use crate::{Error, Result};

/// Number of dimensions in the intent tensor.
const DIMENSION_COUNT: usize = 12;

/// Maximum number of alternative routes to track per decision.
const MAX_ALTERNATIVES: usize = 3;

// ============================================================================
// IntentTensor
// ============================================================================

/// 12-dimensional intent vector for routing decisions.
///
/// Each dimension is clamped to `[0.0, 1.0]` at construction.
/// The source field identifies the originating module or service.
#[derive(Clone, Debug)]
pub struct IntentTensor {
    /// 12 dimensions matching the ME V2 tensor encoding.
    dimensions: [f64; DIMENSION_COUNT],
    /// Originating module or service.
    source: String,
    /// Routing priority in `[0.0, 1.0]`.
    priority: f64,
    /// When this intent was created.
    timestamp: Timestamp,
}

impl IntentTensor {
    /// Create a new intent tensor with validated dimensions.
    ///
    /// All dimensions are clamped to `[0.0, 1.0]`. Priority is clamped
    /// to `[0.0, 1.0]`.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if any dimension is NaN or infinite.
    pub fn new(
        dimensions: [f64; DIMENSION_COUNT],
        source: impl Into<String>,
        priority: f64,
    ) -> Result<Self> {
        for (i, &val) in dimensions.iter().enumerate() {
            if val.is_nan() || val.is_infinite() {
                return Err(Error::Validation(format!(
                    "IntentTensor dimension {i} is not finite: {val}"
                )));
            }
        }
        if priority.is_nan() || priority.is_infinite() {
            return Err(Error::Validation(format!(
                "IntentTensor priority is not finite: {priority}"
            )));
        }
        let mut clamped = [0.0_f64; DIMENSION_COUNT];
        for (i, &val) in dimensions.iter().enumerate() {
            clamped[i] = val.clamp(0.0, 1.0);
        }
        Ok(Self {
            dimensions: clamped,
            source: source.into(),
            priority: priority.clamp(0.0, 1.0),
            timestamp: Timestamp::now(),
        })
    }

    /// Access the 12 dimensions.
    #[must_use]
    pub const fn dimensions(&self) -> &[f64; DIMENSION_COUNT] {
        &self.dimensions
    }

    /// Access the source identifier.
    #[must_use]
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Access the priority value.
    #[must_use]
    pub const fn priority(&self) -> f64 {
        self.priority
    }

    /// Access the creation timestamp.
    #[must_use]
    pub const fn timestamp(&self) -> Timestamp {
        self.timestamp
    }
}

// ============================================================================
// ServiceAffinity
// ============================================================================

/// Describes a service's affinity profile for intent routing.
///
/// The dimension weights indicate how strongly a service matches
/// each of the 12 tensor dimensions. Capacity gates the final score.
#[derive(Clone, Debug)]
pub struct ServiceAffinity {
    /// Unique service identifier.
    service_id: String,
    /// Per-dimension routing weights in `[0.0, 1.0]`.
    dimension_weights: [f64; DIMENSION_COUNT],
    /// Available capacity in `[0.0, 1.0]`.
    capacity: f64,
    /// Whether this service is currently accepting intents.
    active: bool,
}

impl ServiceAffinity {
    /// Create a new service affinity.
    ///
    /// Weights and capacity are clamped to `[0.0, 1.0]`.
    #[must_use]
    pub fn new(
        service_id: impl Into<String>,
        dimension_weights: [f64; DIMENSION_COUNT],
        capacity: f64,
    ) -> Self {
        let mut clamped_weights = [0.0_f64; DIMENSION_COUNT];
        for (i, &w) in dimension_weights.iter().enumerate() {
            clamped_weights[i] = if w.is_nan() || w.is_infinite() {
                0.0
            } else {
                w.clamp(0.0, 1.0)
            };
        }
        let safe_capacity = if capacity.is_nan() || capacity.is_infinite() {
            0.0
        } else {
            capacity.clamp(0.0, 1.0)
        };
        Self {
            service_id: service_id.into(),
            dimension_weights: clamped_weights,
            capacity: safe_capacity,
            active: true,
        }
    }

    /// Access the service ID.
    #[must_use]
    pub fn service_id(&self) -> &str {
        &self.service_id
    }

    /// Access the dimension weights.
    #[must_use]
    pub const fn dimension_weights(&self) -> &[f64; DIMENSION_COUNT] {
        &self.dimension_weights
    }

    /// Access the capacity.
    #[must_use]
    pub const fn capacity(&self) -> f64 {
        self.capacity
    }

    /// Check if the service is active.
    #[must_use]
    pub const fn active(&self) -> bool {
        self.active
    }

    /// Set the active flag.
    pub const fn set_active(&mut self, active: bool) {
        self.active = active;
    }
}

// ============================================================================
// RoutingDecision
// ============================================================================

/// A routing decision: maps an intent to a target service.
#[derive(Clone, Debug)]
pub struct RoutingDecision {
    /// Unique identifier for this decision (UUID v4 hex string).
    intent_id: String,
    /// The service selected for this intent.
    target_service: String,
    /// Weighted dot-product score for the selected service.
    score: f64,
    /// Confidence derived from the score relative to alternatives.
    confidence: f64,
    /// Top alternative routes: `(service_id, score)`.
    alternatives: Vec<(String, f64)>,
    /// When the decision was made.
    timestamp: Timestamp,
}

impl RoutingDecision {
    /// Access the intent ID.
    #[must_use]
    pub fn intent_id(&self) -> &str {
        &self.intent_id
    }

    /// Access the target service.
    #[must_use]
    pub fn target_service(&self) -> &str {
        &self.target_service
    }

    /// Access the routing score.
    #[must_use]
    pub const fn score(&self) -> f64 {
        self.score
    }

    /// Access the confidence level.
    #[must_use]
    pub const fn confidence(&self) -> f64 {
        self.confidence
    }

    /// Access the alternative routes.
    #[must_use]
    pub fn alternatives(&self) -> &[(String, f64)] {
        &self.alternatives
    }

    /// Access the decision timestamp.
    #[must_use]
    pub const fn timestamp(&self) -> Timestamp {
        self.timestamp
    }
}

// ============================================================================
// RoutingStats
// ============================================================================

/// Snapshot of routing engine statistics.
#[derive(Clone, Debug)]
pub struct RoutingStats {
    /// Total intents routed since creation or last reset.
    pub total_routed: u64,
    /// Average score across all decisions.
    pub avg_score: f64,
    /// Number of registered services.
    pub service_count: usize,
    /// Number of decisions in the log.
    pub decisions_logged: usize,
}

// ============================================================================
// IntentRouterConfig
// ============================================================================

/// Configuration for the [`IntentRouterCore`].
#[derive(Clone, Debug)]
pub struct IntentRouterConfig {
    /// Maximum number of registered services.
    max_services: usize,
    /// Decision log ring-buffer capacity.
    decision_log_capacity: usize,
    /// Minimum score to accept a routing decision.
    min_score_threshold: f64,
}

impl IntentRouterConfig {
    /// Create a new configuration with explicit values.
    #[must_use]
    pub fn new(max_services: usize, decision_log_capacity: usize, min_score_threshold: f64) -> Self {
        Self {
            max_services: max_services.max(1),
            decision_log_capacity: decision_log_capacity.max(1),
            min_score_threshold: min_score_threshold.clamp(0.0, 1.0),
        }
    }

    /// Maximum number of registered services.
    #[must_use]
    pub const fn max_services(&self) -> usize {
        self.max_services
    }

    /// Decision log ring-buffer capacity.
    #[must_use]
    pub const fn decision_log_capacity(&self) -> usize {
        self.decision_log_capacity
    }

    /// Minimum score threshold.
    #[must_use]
    pub const fn min_score_threshold(&self) -> f64 {
        self.min_score_threshold
    }
}

impl Default for IntentRouterConfig {
    fn default() -> Self {
        Self {
            max_services: 100,
            decision_log_capacity: 500,
            min_score_threshold: 0.1,
        }
    }
}

// ============================================================================
// IntentRouter (trait)
// ============================================================================

/// Trait for routing 12D intents to services.
///
/// All methods are `&self` (C2). State mutation uses interior mutability.
/// Methods returning data through `RwLock` return owned types (C7).
pub trait IntentRouter: Send + Sync + fmt::Debug {
    /// Route an intent to the best-matching service.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if no service scores above the threshold.
    fn route(&self, intent: &IntentTensor) -> Result<RoutingDecision>;

    /// Register a service with its affinity profile.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if the maximum number of services is reached,
    /// or if a service with the same ID is already registered.
    fn register_service(&self, service_id: &str, affinity: ServiceAffinity) -> Result<()>;

    /// Remove a service from the routing table.
    ///
    /// # Errors
    ///
    /// Returns `Error::ServiceNotFound` if no service is registered with this ID.
    fn deregister_service(&self, service_id: &str) -> Result<()>;

    /// Get a snapshot of routing statistics.
    fn routing_stats(&self) -> RoutingStats;

    /// Get the number of registered services.
    fn service_count(&self) -> usize;

    /// Get the most recent routing decisions.
    fn recent_decisions(&self, limit: usize) -> Vec<RoutingDecision>;

    /// Update the affinity profile for an existing service.
    ///
    /// # Errors
    ///
    /// Returns `Error::ServiceNotFound` if no service is registered with this ID.
    fn update_affinity(&self, service_id: &str, affinity: ServiceAffinity) -> Result<()>;

    /// Return the service ID of the best match without logging a decision.
    fn best_match(&self, intent: &IntentTensor) -> Option<String>;

    /// Clear all services and decisions.
    fn reset(&self);
}

// ============================================================================
// IntentRouterCore
// ============================================================================

/// Core implementation of [`IntentRouter`].
///
/// Uses `parking_lot::RwLock` for interior mutability and
/// `AtomicU64` for lock-free counters.
pub struct IntentRouterCore {
    /// Registered services keyed by service ID.
    services: RwLock<HashMap<String, ServiceAffinity>>,
    /// Ring-buffer of recent routing decisions.
    decisions: RwLock<VecDeque<RoutingDecision>>,
    /// Configuration.
    config: IntentRouterConfig,
    /// Total number of intents routed.
    total_routed: AtomicU64,
    /// Running sum of scores for average calculation.
    score_sum: RwLock<f64>,
}

impl IntentRouterCore {
    /// Create a new intent router with the given configuration.
    #[must_use]
    pub fn new(config: IntentRouterConfig) -> Self {
        Self {
            services: RwLock::new(HashMap::new()),
            decisions: RwLock::new(VecDeque::new()),
            config,
            total_routed: AtomicU64::new(0),
            score_sum: RwLock::new(0.0),
        }
    }

    /// Create a new intent router with default configuration.
    #[must_use]
    pub fn with_defaults() -> Self {
        Self::new(IntentRouterConfig::default())
    }

    /// Access the configuration.
    #[must_use]
    pub const fn config(&self) -> &IntentRouterConfig {
        &self.config
    }
}

impl Default for IntentRouterCore {
    fn default() -> Self {
        Self::with_defaults()
    }
}

impl fmt::Debug for IntentRouterCore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let svc_count = self.services.read().len();
        let dec_count = self.decisions.read().len();
        f.debug_struct("IntentRouterCore")
            .field("services", &svc_count)
            .field("decisions", &dec_count)
            .field("config", &self.config)
            .field("total_routed", &self.total_routed.load(Ordering::Relaxed))
            .field("score_sum", &*self.score_sum.read())
            .finish()
    }
}

/// Compute the weighted dot product between an intent and a service affinity.
///
/// `score = sum(intent[i] * weights[i]) * capacity`
fn compute_score(intent: &IntentTensor, affinity: &ServiceAffinity) -> f64 {
    let mut dot = 0.0_f64;
    for i in 0..DIMENSION_COUNT {
        dot = intent.dimensions[i].mul_add(affinity.dimension_weights[i], dot);
    }
    dot * affinity.capacity
}

/// Compute confidence from the best score and the runner-up score.
///
/// Returns 1.0 when there is a single service, otherwise the margin
/// between best and second-best relative to the best.
fn compute_confidence(best_score: f64, second_score: f64) -> f64 {
    if best_score <= 0.0 {
        return 0.0;
    }
    ((best_score - second_score) / best_score).clamp(0.0, 1.0)
}

/// Generate a deterministic hex ID from the intent source and timestamp.
fn generate_intent_id(intent: &IntentTensor) -> String {
    let hash = intent
        .source
        .bytes()
        .fold(intent.timestamp.ticks(), |acc, b| {
            acc.wrapping_mul(31).wrapping_add(u64::from(b))
        });
    format!("{hash:016x}")
}

impl IntentRouter for IntentRouterCore {
    fn route(&self, intent: &IntentTensor) -> Result<RoutingDecision> {
        let services = self.services.read();
        let mut scored: Vec<(String, f64)> = services
            .values()
            .filter(|a| a.active)
            .map(|a| (a.service_id.clone(), compute_score(intent, a)))
            .collect();
        drop(services);

        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        let (target_service, best_score) = scored
            .first()
            .ok_or_else(|| Error::Validation("No active services registered".to_string()))?
            .clone();

        if best_score < self.config.min_score_threshold {
            return Err(Error::Validation(format!(
                "Best score {best_score:.4} is below threshold {}",
                self.config.min_score_threshold
            )));
        }

        let second_score = scored.get(1).map_or(0.0, |s| s.1);
        let confidence = compute_confidence(best_score, second_score);

        let alternatives: Vec<(String, f64)> = scored
            .iter()
            .skip(1)
            .take(MAX_ALTERNATIVES)
            .cloned()
            .collect();

        let decision = RoutingDecision {
            intent_id: generate_intent_id(intent),
            target_service,
            score: best_score,
            confidence,
            alternatives,
            timestamp: Timestamp::now(),
        };

        // Log the decision in the ring buffer.
        {
            let mut decisions = self.decisions.write();
            if decisions.len() >= self.config.decision_log_capacity {
                decisions.pop_front();
            }
            decisions.push_back(decision.clone());
        }

        // Update counters.
        self.total_routed.fetch_add(1, Ordering::Relaxed);
        {
            let mut sum = self.score_sum.write();
            *sum += best_score;
        }

        Ok(decision)
    }

    fn register_service(&self, service_id: &str, affinity: ServiceAffinity) -> Result<()> {
        let mut services = self.services.write();
        if services.len() >= self.config.max_services {
            return Err(Error::Validation(format!(
                "Maximum service count {} reached",
                self.config.max_services
            )));
        }
        if services.contains_key(service_id) {
            return Err(Error::Validation(format!(
                "Service '{service_id}' is already registered"
            )));
        }
        services.insert(service_id.to_string(), affinity);
        drop(services);
        Ok(())
    }

    fn deregister_service(&self, service_id: &str) -> Result<()> {
        let mut services = self.services.write();
        let removed = services.remove(service_id).is_some();
        drop(services);
        if !removed {
            return Err(Error::ServiceNotFound(service_id.to_string()));
        }
        Ok(())
    }

    #[allow(clippy::cast_precision_loss)]
    fn routing_stats(&self) -> RoutingStats {
        let total = self.total_routed.load(Ordering::Relaxed);
        let sum = *self.score_sum.read();
        let avg = if total > 0 {
            sum / total as f64
        } else {
            0.0
        };
        let service_count = self.services.read().len();
        let decisions_logged = self.decisions.read().len();
        RoutingStats {
            total_routed: total,
            avg_score: avg,
            service_count,
            decisions_logged,
        }
    }

    fn service_count(&self) -> usize {
        self.services.read().len()
    }

    fn recent_decisions(&self, limit: usize) -> Vec<RoutingDecision> {
        self.decisions.read().iter().rev().take(limit).cloned().collect()
    }

    fn update_affinity(&self, service_id: &str, affinity: ServiceAffinity) -> Result<()> {
        let mut services = self.services.write();
        if !services.contains_key(service_id) {
            drop(services);
            return Err(Error::ServiceNotFound(service_id.to_string()));
        }
        services.insert(service_id.to_string(), affinity);
        drop(services);
        Ok(())
    }

    fn best_match(&self, intent: &IntentTensor) -> Option<String> {
        let services = self.services.read();
        services
            .values()
            .filter(|a| a.active)
            .map(|a| (a.service_id.clone(), compute_score(intent, a)))
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
            .filter(|(_, score)| *score >= self.config.min_score_threshold)
            .map(|(id, _)| id)
    }

    fn reset(&self) {
        self.services.write().clear();
        self.decisions.write().clear();
        self.total_routed.store(0, Ordering::Relaxed);
        *self.score_sum.write() = 0.0;
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ---- Helpers ----

    /// Create a simple intent with uniform dimensions.
    fn uniform_intent(val: f64) -> IntentTensor {
        IntentTensor::new([val; DIMENSION_COUNT], "test", 0.5).unwrap_or_else(|_| {
            IntentTensor {
                dimensions: [0.0; DIMENSION_COUNT],
                source: "test".into(),
                priority: 0.5,
                timestamp: Timestamp::now(),
            }
        })
    }

    /// Create a service affinity with uniform weights.
    fn uniform_affinity(id: &str, weight: f64, capacity: f64) -> ServiceAffinity {
        ServiceAffinity::new(id, [weight; DIMENSION_COUNT], capacity)
    }

    fn make_router() -> IntentRouterCore {
        IntentRouterCore::with_defaults()
    }

    fn make_router_with_config(max: usize, log_cap: usize, threshold: f64) -> IntentRouterCore {
        IntentRouterCore::new(IntentRouterConfig::new(max, log_cap, threshold))
    }

    // ---- IntentTensor Construction ----

    #[test]
    fn test_intent_tensor_new_valid() {
        let intent = IntentTensor::new([0.5; DIMENSION_COUNT], "source", 0.8);
        assert!(intent.is_ok());
        let intent = intent.unwrap_or_else(|_| unreachable!());
        assert_eq!(intent.source(), "source");
        assert!((intent.priority() - 0.8).abs() < f64::EPSILON);
    }

    #[test]
    fn test_intent_tensor_clamps_dimensions() {
        let dims = [1.5, -0.5, 0.0, 1.0, 0.5, 0.3, 0.7, 0.9, 0.1, 0.2, 0.4, 0.6];
        let intent = IntentTensor::new(dims, "src", 0.5)
            .unwrap_or_else(|_| unreachable!());
        assert!((intent.dimensions()[0] - 1.0).abs() < f64::EPSILON);
        assert!(intent.dimensions()[1].abs() < f64::EPSILON);
    }

    #[test]
    fn test_intent_tensor_clamps_priority() {
        let intent = IntentTensor::new([0.5; DIMENSION_COUNT], "src", 2.0)
            .unwrap_or_else(|_| unreachable!());
        assert!((intent.priority() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_intent_tensor_rejects_nan() {
        let mut dims = [0.5; DIMENSION_COUNT];
        dims[3] = f64::NAN;
        assert!(IntentTensor::new(dims, "src", 0.5).is_err());
    }

    #[test]
    fn test_intent_tensor_rejects_infinite() {
        let mut dims = [0.5; DIMENSION_COUNT];
        dims[7] = f64::INFINITY;
        assert!(IntentTensor::new(dims, "src", 0.5).is_err());
    }

    #[test]
    fn test_intent_tensor_rejects_nan_priority() {
        assert!(IntentTensor::new([0.5; DIMENSION_COUNT], "src", f64::NAN).is_err());
    }

    #[test]
    fn test_intent_tensor_timestamp_advances() {
        let a = IntentTensor::new([0.5; DIMENSION_COUNT], "a", 0.5)
            .unwrap_or_else(|_| unreachable!());
        let b = IntentTensor::new([0.5; DIMENSION_COUNT], "b", 0.5)
            .unwrap_or_else(|_| unreachable!());
        assert!(b.timestamp() > a.timestamp());
    }

    #[test]
    fn test_intent_tensor_clone() {
        let intent = IntentTensor::new([0.3; DIMENSION_COUNT], "c", 0.7)
            .unwrap_or_else(|_| unreachable!());
        let clone = intent.clone();
        assert_eq!(clone.source(), intent.source());
        assert!((clone.priority() - intent.priority()).abs() < f64::EPSILON);
    }

    // ---- ServiceAffinity Construction ----

    #[test]
    fn test_service_affinity_new() {
        let aff = ServiceAffinity::new("svc-1", [0.8; DIMENSION_COUNT], 0.9);
        assert_eq!(aff.service_id(), "svc-1");
        assert!((aff.capacity() - 0.9).abs() < f64::EPSILON);
        assert!(aff.active());
    }

    #[test]
    fn test_service_affinity_clamps_weights() {
        let mut weights = [0.5; DIMENSION_COUNT];
        weights[0] = 2.0;
        weights[1] = -1.0;
        let aff = ServiceAffinity::new("svc", weights, 0.5);
        assert!((aff.dimension_weights()[0] - 1.0).abs() < f64::EPSILON);
        assert!(aff.dimension_weights()[1].abs() < f64::EPSILON);
    }

    #[test]
    fn test_service_affinity_clamps_capacity() {
        let aff = ServiceAffinity::new("svc", [0.5; DIMENSION_COUNT], 1.5);
        assert!((aff.capacity() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_service_affinity_nan_weight_becomes_zero() {
        let mut weights = [0.5; DIMENSION_COUNT];
        weights[5] = f64::NAN;
        let aff = ServiceAffinity::new("svc", weights, 0.5);
        assert!(aff.dimension_weights()[5].abs() < f64::EPSILON);
    }

    #[test]
    fn test_service_affinity_nan_capacity_becomes_zero() {
        let aff = ServiceAffinity::new("svc", [0.5; DIMENSION_COUNT], f64::NAN);
        assert!(aff.capacity().abs() < f64::EPSILON);
    }

    #[test]
    fn test_service_affinity_set_active() {
        let mut aff = ServiceAffinity::new("svc", [0.5; DIMENSION_COUNT], 0.5);
        assert!(aff.active());
        aff.set_active(false);
        assert!(!aff.active());
    }

    // ---- IntentRouterConfig ----

    #[test]
    fn test_config_default() {
        let cfg = IntentRouterConfig::default();
        assert_eq!(cfg.max_services(), 100);
        assert_eq!(cfg.decision_log_capacity(), 500);
        assert!((cfg.min_score_threshold() - 0.1).abs() < f64::EPSILON);
    }

    #[test]
    fn test_config_custom() {
        let cfg = IntentRouterConfig::new(50, 200, 0.3);
        assert_eq!(cfg.max_services(), 50);
        assert_eq!(cfg.decision_log_capacity(), 200);
        assert!((cfg.min_score_threshold() - 0.3).abs() < f64::EPSILON);
    }

    #[test]
    fn test_config_clamps_threshold() {
        let cfg = IntentRouterConfig::new(10, 10, 2.0);
        assert!((cfg.min_score_threshold() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_config_enforces_minimum_values() {
        let cfg = IntentRouterConfig::new(0, 0, -1.0);
        assert_eq!(cfg.max_services(), 1);
        assert_eq!(cfg.decision_log_capacity(), 1);
        assert!(cfg.min_score_threshold().abs() < f64::EPSILON);
    }

    // ---- IntentRouterCore Construction ----

    #[test]
    fn test_router_new_empty() {
        let router = make_router();
        assert_eq!(router.service_count(), 0);
        let stats = router.routing_stats();
        assert_eq!(stats.total_routed, 0);
        assert!(stats.avg_score.abs() < f64::EPSILON);
    }

    #[test]
    fn test_router_default() {
        let router = IntentRouterCore::default();
        assert_eq!(router.service_count(), 0);
    }

    #[test]
    fn test_router_debug() {
        let router = make_router();
        let debug = format!("{router:?}");
        assert!(debug.contains("IntentRouterCore"));
    }

    // ---- Registration ----

    #[test]
    fn test_register_service() {
        let router = make_router();
        let aff = uniform_affinity("svc-1", 0.5, 1.0);
        assert!(router.register_service("svc-1", aff).is_ok());
        assert_eq!(router.service_count(), 1);
    }

    #[test]
    fn test_register_duplicate_fails() {
        let router = make_router();
        let aff = uniform_affinity("svc-1", 0.5, 1.0);
        router.register_service("svc-1", aff.clone()).ok();
        assert!(router.register_service("svc-1", aff).is_err());
    }

    #[test]
    fn test_register_exceeds_max_fails() {
        let router = make_router_with_config(2, 10, 0.1);
        router
            .register_service("a", uniform_affinity("a", 0.5, 1.0))
            .ok();
        router
            .register_service("b", uniform_affinity("b", 0.5, 1.0))
            .ok();
        assert!(router
            .register_service("c", uniform_affinity("c", 0.5, 1.0))
            .is_err());
    }

    #[test]
    fn test_deregister_service() {
        let router = make_router();
        router
            .register_service("svc-1", uniform_affinity("svc-1", 0.5, 1.0))
            .ok();
        assert!(router.deregister_service("svc-1").is_ok());
        assert_eq!(router.service_count(), 0);
    }

    #[test]
    fn test_deregister_nonexistent_fails() {
        let router = make_router();
        assert!(router.deregister_service("nope").is_err());
    }

    #[test]
    fn test_register_multiple_services() {
        let router = make_router();
        for i in 0..10 {
            let id = format!("svc-{i}");
            router
                .register_service(&id, uniform_affinity(&id, 0.5, 1.0))
                .ok();
        }
        assert_eq!(router.service_count(), 10);
    }

    // ---- Update Affinity ----

    #[test]
    fn test_update_affinity() {
        let router = make_router();
        router
            .register_service("svc", uniform_affinity("svc", 0.3, 0.5))
            .ok();
        let new_aff = uniform_affinity("svc", 0.9, 1.0);
        assert!(router.update_affinity("svc", new_aff).is_ok());
    }

    #[test]
    fn test_update_affinity_nonexistent_fails() {
        let router = make_router();
        assert!(router
            .update_affinity("nope", uniform_affinity("nope", 0.5, 1.0))
            .is_err());
    }

    // ---- Routing ----

    #[test]
    fn test_route_single_service() {
        let router = make_router();
        router
            .register_service("svc", uniform_affinity("svc", 0.5, 1.0))
            .ok();
        let intent = uniform_intent(0.5);
        let decision = router.route(&intent);
        assert!(decision.is_ok());
        let decision = decision.unwrap_or_else(|_| unreachable!());
        assert_eq!(decision.target_service(), "svc");
        assert!(decision.score() > 0.0);
    }

    #[test]
    fn test_route_selects_highest_scorer() {
        let router = make_router();
        router
            .register_service("low", uniform_affinity("low", 0.1, 0.5))
            .ok();
        router
            .register_service("high", uniform_affinity("high", 0.9, 1.0))
            .ok();
        let intent = uniform_intent(0.8);
        let decision = router.route(&intent).unwrap_or_else(|_| unreachable!());
        assert_eq!(decision.target_service(), "high");
    }

    #[test]
    fn test_route_no_active_services_fails() {
        let router = make_router();
        let intent = uniform_intent(0.5);
        assert!(router.route(&intent).is_err());
    }

    #[test]
    fn test_route_all_inactive_fails() {
        let router = make_router();
        let mut aff = uniform_affinity("svc", 0.5, 1.0);
        aff.set_active(false);
        router.register_service("svc", aff).ok();
        let intent = uniform_intent(0.5);
        assert!(router.route(&intent).is_err());
    }

    #[test]
    fn test_route_below_threshold_fails() {
        let router = make_router_with_config(100, 500, 0.9);
        router
            .register_service("svc", uniform_affinity("svc", 0.01, 0.01))
            .ok();
        let intent = uniform_intent(0.01);
        assert!(router.route(&intent).is_err());
    }

    #[test]
    fn test_route_records_alternatives() {
        let router = make_router();
        for i in 0..5 {
            let id = format!("svc-{i}");
            let weight = 0.1 + f64::from(i) * 0.2;
            router
                .register_service(&id, uniform_affinity(&id, weight, 1.0))
                .ok();
        }
        let intent = uniform_intent(0.5);
        let decision = router.route(&intent).unwrap_or_else(|_| unreachable!());
        assert!(decision.alternatives().len() <= MAX_ALTERNATIVES);
    }

    #[test]
    fn test_route_confidence_single_service() {
        let router = make_router();
        router
            .register_service("only", uniform_affinity("only", 0.5, 1.0))
            .ok();
        let intent = uniform_intent(0.5);
        let decision = router.route(&intent).unwrap_or_else(|_| unreachable!());
        // Single service: confidence = 1.0 (full margin).
        assert!((decision.confidence() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_route_confidence_two_equal_services() {
        let router = make_router();
        router
            .register_service("a", uniform_affinity("a", 0.5, 1.0))
            .ok();
        router
            .register_service("b", uniform_affinity("b", 0.5, 1.0))
            .ok();
        let intent = uniform_intent(0.5);
        let decision = router.route(&intent).unwrap_or_else(|_| unreachable!());
        // Equal scores: confidence = 0.0.
        assert!(decision.confidence().abs() < f64::EPSILON);
    }

    // ---- Decision Logging ----

    #[test]
    fn test_recent_decisions_empty() {
        let router = make_router();
        assert!(router.recent_decisions(10).is_empty());
    }

    #[test]
    fn test_recent_decisions_returns_newest_first() {
        let router = make_router();
        router
            .register_service("svc", uniform_affinity("svc", 0.5, 1.0))
            .ok();
        for _ in 0..3 {
            router.route(&uniform_intent(0.5)).ok();
        }
        let recent = router.recent_decisions(10);
        assert_eq!(recent.len(), 3);
        assert!(recent[0].timestamp() >= recent[1].timestamp());
    }

    #[test]
    fn test_decision_log_ring_buffer() {
        let router = make_router_with_config(100, 3, 0.0);
        router
            .register_service("svc", uniform_affinity("svc", 0.5, 1.0))
            .ok();
        for _ in 0..5 {
            router.route(&uniform_intent(0.5)).ok();
        }
        let stats = router.routing_stats();
        assert_eq!(stats.decisions_logged, 3);
        assert_eq!(stats.total_routed, 5);
    }

    #[test]
    fn test_recent_decisions_limit() {
        let router = make_router();
        router
            .register_service("svc", uniform_affinity("svc", 0.5, 1.0))
            .ok();
        for _ in 0..10 {
            router.route(&uniform_intent(0.5)).ok();
        }
        let recent = router.recent_decisions(3);
        assert_eq!(recent.len(), 3);
    }

    // ---- Statistics ----

    #[test]
    fn test_routing_stats_initial() {
        let router = make_router();
        let stats = router.routing_stats();
        assert_eq!(stats.total_routed, 0);
        assert!(stats.avg_score.abs() < f64::EPSILON);
        assert_eq!(stats.service_count, 0);
        assert_eq!(stats.decisions_logged, 0);
    }

    #[test]
    fn test_routing_stats_after_routes() {
        let router = make_router();
        router
            .register_service("svc", uniform_affinity("svc", 0.5, 1.0))
            .ok();
        router.route(&uniform_intent(0.5)).ok();
        router.route(&uniform_intent(0.5)).ok();
        let stats = router.routing_stats();
        assert_eq!(stats.total_routed, 2);
        assert!(stats.avg_score > 0.0);
        assert_eq!(stats.service_count, 1);
        assert_eq!(stats.decisions_logged, 2);
    }

    // ---- Best Match ----

    #[test]
    fn test_best_match_empty() {
        let router = make_router();
        assert!(router.best_match(&uniform_intent(0.5)).is_none());
    }

    #[test]
    fn test_best_match_selects_highest() {
        let router = make_router();
        router
            .register_service("low", uniform_affinity("low", 0.1, 0.5))
            .ok();
        router
            .register_service("high", uniform_affinity("high", 0.9, 1.0))
            .ok();
        let best = router.best_match(&uniform_intent(0.8));
        assert_eq!(best.as_deref(), Some("high"));
    }

    #[test]
    fn test_best_match_below_threshold_returns_none() {
        let router = make_router_with_config(100, 500, 0.9);
        router
            .register_service("svc", uniform_affinity("svc", 0.01, 0.01))
            .ok();
        assert!(router.best_match(&uniform_intent(0.01)).is_none());
    }

    #[test]
    fn test_best_match_does_not_log_decision() {
        let router = make_router();
        router
            .register_service("svc", uniform_affinity("svc", 0.5, 1.0))
            .ok();
        router.best_match(&uniform_intent(0.5));
        assert_eq!(router.routing_stats().total_routed, 0);
    }

    #[test]
    fn test_best_match_ignores_inactive() {
        let router = make_router();
        let mut aff = uniform_affinity("inactive", 0.9, 1.0);
        aff.set_active(false);
        router.register_service("inactive", aff).ok();
        router
            .register_service("active", uniform_affinity("active", 0.3, 0.5))
            .ok();
        let best = router.best_match(&uniform_intent(0.5));
        assert_eq!(best.as_deref(), Some("active"));
    }

    // ---- Reset ----

    #[test]
    fn test_reset_clears_everything() {
        let router = make_router();
        router
            .register_service("svc", uniform_affinity("svc", 0.5, 1.0))
            .ok();
        router.route(&uniform_intent(0.5)).ok();
        router.reset();
        assert_eq!(router.service_count(), 0);
        assert_eq!(router.routing_stats().total_routed, 0);
        assert!(router.recent_decisions(10).is_empty());
    }

    // ---- Score Computation ----

    #[test]
    fn test_compute_score_zero_intent() {
        let intent = uniform_intent(0.0);
        let affinity = uniform_affinity("svc", 1.0, 1.0);
        let score = compute_score(&intent, &affinity);
        assert!(score.abs() < f64::EPSILON);
    }

    #[test]
    fn test_compute_score_zero_weights() {
        let intent = uniform_intent(1.0);
        let affinity = uniform_affinity("svc", 0.0, 1.0);
        let score = compute_score(&intent, &affinity);
        assert!(score.abs() < f64::EPSILON);
    }

    #[test]
    fn test_compute_score_full_match() {
        let intent = uniform_intent(1.0);
        let affinity = uniform_affinity("svc", 1.0, 1.0);
        let score = compute_score(&intent, &affinity);
        // 12 * (1.0 * 1.0) * 1.0 = 12.0
        assert!((score - 12.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_compute_score_capacity_scales() {
        let intent = uniform_intent(1.0);
        let full = uniform_affinity("full", 1.0, 1.0);
        let half = uniform_affinity("half", 1.0, 0.5);
        let full_score = compute_score(&intent, &full);
        let half_score = compute_score(&intent, &half);
        assert!((full_score - 2.0 * half_score).abs() < f64::EPSILON);
    }

    #[test]
    fn test_compute_score_selective_weights() {
        let mut dims = [0.0; DIMENSION_COUNT];
        dims[6] = 1.0; // health_score
        let intent = IntentTensor::new(dims, "test", 0.5)
            .unwrap_or_else(|_| unreachable!());
        let mut weights = [0.0; DIMENSION_COUNT];
        weights[6] = 1.0;
        let affinity = ServiceAffinity::new("svc", weights, 1.0);
        let score = compute_score(&intent, &affinity);
        assert!((score - 1.0).abs() < f64::EPSILON);
    }

    // ---- Confidence Computation ----

    #[test]
    fn test_confidence_zero_best() {
        assert!(compute_confidence(0.0, 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_confidence_equal_scores() {
        assert!(compute_confidence(5.0, 5.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_confidence_large_margin() {
        assert!((compute_confidence(10.0, 0.0) - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_confidence_partial_margin() {
        let c = compute_confidence(10.0, 5.0);
        assert!((c - 0.5).abs() < f64::EPSILON);
    }

    // ---- RoutingDecision Accessors ----

    #[test]
    fn test_routing_decision_accessors() {
        let router = make_router();
        router
            .register_service("svc", uniform_affinity("svc", 0.5, 1.0))
            .ok();
        let decision = router.route(&uniform_intent(0.5)).unwrap_or_else(|_| unreachable!());
        assert!(!decision.intent_id().is_empty());
        assert_eq!(decision.target_service(), "svc");
        assert!(decision.score() > 0.0);
        assert!(decision.confidence() >= 0.0);
        assert!(decision.alternatives().is_empty()); // single service
    }

    // ---- Edge Cases ----

    #[test]
    fn test_route_many_services() {
        let router = make_router();
        for i in 0..50 {
            let id = format!("svc-{i}");
            let weight = f64::from(i) / 50.0;
            router
                .register_service(&id, uniform_affinity(&id, weight, 1.0))
                .ok();
        }
        let decision = router.route(&uniform_intent(0.5)).unwrap_or_else(|_| unreachable!());
        assert_eq!(decision.target_service(), "svc-49");
    }

    #[test]
    fn test_route_zero_capacity_service_loses() {
        let router = make_router();
        router
            .register_service("zero", uniform_affinity("zero", 1.0, 0.0))
            .ok();
        router
            .register_service("half", uniform_affinity("half", 0.5, 0.5))
            .ok();
        let decision = router.route(&uniform_intent(0.5)).unwrap_or_else(|_| unreachable!());
        assert_eq!(decision.target_service(), "half");
    }

    #[test]
    fn test_intent_id_deterministic_for_same_input() {
        let a = IntentTensor {
            dimensions: [0.5; DIMENSION_COUNT],
            source: "test".into(),
            priority: 0.5,
            timestamp: Timestamp::from_raw(42),
        };
        let b = IntentTensor {
            dimensions: [0.5; DIMENSION_COUNT],
            source: "test".into(),
            priority: 0.5,
            timestamp: Timestamp::from_raw(42),
        };
        assert_eq!(generate_intent_id(&a), generate_intent_id(&b));
    }

    #[test]
    fn test_intent_id_varies_with_source() {
        let a = IntentTensor {
            dimensions: [0.5; DIMENSION_COUNT],
            source: "alpha".into(),
            priority: 0.5,
            timestamp: Timestamp::from_raw(42),
        };
        let b = IntentTensor {
            dimensions: [0.5; DIMENSION_COUNT],
            source: "beta".into(),
            priority: 0.5,
            timestamp: Timestamp::from_raw(42),
        };
        assert_ne!(generate_intent_id(&a), generate_intent_id(&b));
    }

    #[test]
    fn test_concurrent_service_count() {
        let router = make_router();
        router
            .register_service("a", uniform_affinity("a", 0.5, 1.0))
            .ok();
        router
            .register_service("b", uniform_affinity("b", 0.5, 1.0))
            .ok();
        assert_eq!(router.service_count(), 2);
        router.deregister_service("a").ok();
        assert_eq!(router.service_count(), 1);
    }

    #[test]
    fn test_stats_avg_score_accuracy() {
        let router = make_router();
        router
            .register_service("svc", uniform_affinity("svc", 1.0, 1.0))
            .ok();
        // route 3 identical intents — scores should be identical
        for _ in 0..3 {
            router.route(&uniform_intent(0.5)).ok();
        }
        let stats = router.routing_stats();
        // expected score = sum(0.5 * 1.0 for 12 dims) * 1.0 = 6.0
        assert!((stats.avg_score - 6.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_routing_stats_clone() {
        let stats = RoutingStats {
            total_routed: 10,
            avg_score: 0.5,
            service_count: 3,
            decisions_logged: 7,
        };
        let cloned = stats.clone();
        assert_eq!(cloned.total_routed, 10);
    }

    #[test]
    fn test_routing_decision_clone() {
        let router = make_router();
        router
            .register_service("svc", uniform_affinity("svc", 0.5, 1.0))
            .ok();
        let decision = router.route(&uniform_intent(0.5)).unwrap_or_else(|_| unreachable!());
        let cloned = decision.clone();
        assert_eq!(cloned.target_service(), decision.target_service());
        assert!((cloned.score() - decision.score()).abs() < f64::EPSILON);
    }

    #[test]
    fn test_config_clone() {
        let cfg = IntentRouterConfig::new(42, 84, 0.42);
        let cloned = cfg.clone();
        assert_eq!(cloned.max_services(), 42);
    }
}
