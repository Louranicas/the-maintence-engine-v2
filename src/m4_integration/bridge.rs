//! # M24: Bridge Manager
//!
//! Service bridge coordination for the Maintenance Engine.
//! Manages inter-service bridges, tracks bridge health and request metrics,
//! computes synergy scores between services, and monitors bridge status.
//!
//! ## Layer: L4 (Integration)
//! ## Dependencies: M01 (Error), M04 (mod.rs types)
//!
//! ## Features
//!
//! - Bridge registration and lifecycle management
//! - Per-bridge health scoring and status transitions
//! - Request counting with error tracking
//! - Cross-service synergy score computation
//! - Active/failed bridge enumeration
//! - Wire weight integration from default topology
//!
//! ## Bridge Status Model
//!
//! | Status | Health Score | Condition |
//! |--------|-------------|-----------|
//! | Active | >= 0.7 | Normal operation |
//! | Degraded | [0.3, 0.7) | Performance issues |
//! | Failed | < 0.3 | Critical failure |
//! | Disconnected | N/A | Manually disconnected |
//!
//! ## Related Documentation
//! - [Layer Specification](../../ai_docs/layers/L04_INTEGRATION.md)

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use uuid::Uuid;

use super::{default_wire_weights, WireProtocol, WireWeight};
use crate::{Error, Result};

/// Operational status of a service bridge.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BridgeStatus {
    /// Bridge is fully operational.
    Active,
    /// Bridge is experiencing degraded performance.
    Degraded,
    /// Bridge has failed and is not processing requests.
    Failed,
    /// Bridge has been manually disconnected.
    Disconnected,
}

impl std::fmt::Display for BridgeStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Active => write!(f, "Active"),
            Self::Degraded => write!(f, "Degraded"),
            Self::Failed => write!(f, "Failed"),
            Self::Disconnected => write!(f, "Disconnected"),
        }
    }
}

impl BridgeStatus {
    /// Derive the bridge status from a health score.
    ///
    /// - `>= 0.7` => Active
    /// - `[0.3, 0.7)` => Degraded
    /// - `< 0.3` => Failed
    #[must_use]
    pub fn from_health_score(score: f64) -> Self {
        if score >= 0.7 {
            Self::Active
        } else if score >= 0.3 {
            Self::Degraded
        } else {
            Self::Failed
        }
    }
}

/// A bridge connecting two services in the ULTRAPLATE mesh.
#[derive(Clone, Debug)]
pub struct ServiceBridge {
    /// Unique bridge identifier (UUID v4).
    pub bridge_id: String,
    /// Source service identifier.
    pub source_service: String,
    /// Target service identifier.
    pub target_service: String,
    /// Wire protocol used by this bridge.
    pub protocol: WireProtocol,
    /// Current operational status.
    pub status: BridgeStatus,
    /// Health score in range [0.0, 1.0].
    pub health_score: f64,
    /// Average latency in milliseconds.
    pub latency_ms: f64,
    /// Total number of requests processed.
    pub request_count: u64,
    /// Total number of failed requests.
    pub error_count: u64,
    /// Timestamp when the bridge was created.
    pub created_at: DateTime<Utc>,
    /// Timestamp of the most recent request, if any.
    pub last_active: Option<DateTime<Utc>>,
}

impl ServiceBridge {
    /// Calculate the error rate for this bridge.
    ///
    /// Returns `0.0` if no requests have been made.
    #[must_use]
    pub fn error_rate(&self) -> f64 {
        if self.request_count == 0 {
            return 0.0;
        }
        #[allow(clippy::cast_precision_loss)]
        {
            self.error_count as f64 / self.request_count as f64
        }
    }
}

/// Manages service-to-service bridges, synergy tracking, and wire weight topology.
///
/// The `BridgeManager` is the central coordination point for all inter-service
/// communication bridges in the Maintenance Engine.
pub struct BridgeManager {
    /// Registered bridges, keyed by bridge ID.
    bridges: RwLock<HashMap<String, ServiceBridge>>,
    /// Synergy scores between service pairs (source, target) -> score.
    synergy_scores: RwLock<HashMap<(String, String), f64>>,
    /// Default wire weight topology.
    wire_weights: Vec<WireWeight>,
}

impl BridgeManager {
    /// Create a new `BridgeManager` loaded with the default wire weight topology.
    ///
    /// Initial synergy scores are derived from the wire weights.
    #[must_use]
    pub fn new() -> Self {
        let weights = default_wire_weights();

        let mut synergy = HashMap::new();
        for w in &weights {
            // Normalize weight to [0, 1] range as initial synergy
            // Weights range from 1.0 to 1.5, so map to approximately [0.6, 1.0]
            let score = (w.weight / 1.5).min(1.0);
            synergy.insert((w.source.clone(), w.target.clone()), score);
        }

        Self {
            bridges: RwLock::new(HashMap::new()),
            synergy_scores: RwLock::new(synergy),
            wire_weights: weights,
        }
    }

    /// Register a new bridge between two services.
    ///
    /// Returns the generated bridge ID on success.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] if `source` or `target` is empty.
    pub fn register_bridge(
        &self,
        source: &str,
        target: &str,
        protocol: WireProtocol,
    ) -> Result<String> {
        if source.is_empty() {
            return Err(Error::Validation(
                "bridge source service must not be empty".into(),
            ));
        }
        if target.is_empty() {
            return Err(Error::Validation(
                "bridge target service must not be empty".into(),
            ));
        }

        let bridge_id = Uuid::new_v4().to_string();
        let bridge = ServiceBridge {
            bridge_id: bridge_id.clone(),
            source_service: source.into(),
            target_service: target.into(),
            protocol,
            status: BridgeStatus::Active,
            health_score: 1.0,
            latency_ms: 0.0,
            request_count: 0,
            error_count: 0,
            created_at: Utc::now(),
            last_active: None,
        };

        self.bridges.write().insert(bridge_id.clone(), bridge);

        Ok(bridge_id)
    }

    /// Remove a bridge by its ID.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ServiceNotFound`] if the bridge ID does not exist.
    pub fn deregister_bridge(&self, bridge_id: &str) -> Result<()> {
        if self.bridges.write().remove(bridge_id).is_none() {
            return Err(Error::ServiceNotFound(format!(
                "bridge '{bridge_id}' not found"
            )));
        }
        Ok(())
    }

    /// Update the health score and derived status for a bridge.
    ///
    /// The health score is clamped to [0.0, 1.0] and the bridge status is
    /// automatically derived from the score.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ServiceNotFound`] if the bridge ID does not exist.
    #[allow(clippy::significant_drop_tightening)]
    pub fn update_health(&self, bridge_id: &str, health_score: f64) -> Result<()> {
        let mut bridges = self.bridges.write();
        let bridge = bridges
            .get_mut(bridge_id)
            .ok_or_else(|| Error::ServiceNotFound(format!("bridge '{bridge_id}' not found")))?;

        let clamped = health_score.clamp(0.0, 1.0);
        bridge.health_score = clamped;
        bridge.status = BridgeStatus::from_health_score(clamped);

        Ok(())
    }

    /// Record a request on a bridge, updating latency and error counters.
    ///
    /// The latency is tracked as a running average across all requests.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ServiceNotFound`] if the bridge ID does not exist.
    #[allow(clippy::significant_drop_tightening)]
    pub fn record_request(
        &self,
        bridge_id: &str,
        latency_ms: f64,
        success: bool,
    ) -> Result<()> {
        let mut bridges = self.bridges.write();
        let bridge = bridges
            .get_mut(bridge_id)
            .ok_or_else(|| Error::ServiceNotFound(format!("bridge '{bridge_id}' not found")))?;

        // Update running average latency
        let new_count = bridge.request_count + 1;
        #[allow(clippy::cast_precision_loss)]
        {
            let prev_total = bridge.latency_ms * bridge.request_count as f64;
            bridge.latency_ms = (prev_total + latency_ms) / new_count as f64;
        }

        bridge.request_count = new_count;
        if !success {
            bridge.error_count += 1;
        }
        bridge.last_active = Some(Utc::now());

        Ok(())
    }

    /// Retrieve a clone of a bridge by its ID.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ServiceNotFound`] if the bridge ID does not exist.
    pub fn get_bridge(&self, bridge_id: &str) -> Result<ServiceBridge> {
        self.bridges
            .read()
            .get(bridge_id)
            .cloned()
            .ok_or_else(|| Error::ServiceNotFound(format!("bridge '{bridge_id}' not found")))
    }

    /// Retrieve all bridges that involve a given service (as source or target).
    #[must_use]
    pub fn get_bridges_for_service(&self, service_id: &str) -> Vec<ServiceBridge> {
        self.bridges
            .read()
            .values()
            .filter(|b| b.source_service == service_id || b.target_service == service_id)
            .cloned()
            .collect()
    }

    /// Get the synergy score between two services.
    ///
    /// Returns `0.0` if no synergy score has been recorded for the pair.
    #[must_use]
    pub fn get_synergy(&self, source: &str, target: &str) -> f64 {
        self.synergy_scores
            .read()
            .get(&(source.into(), target.into()))
            .copied()
            .unwrap_or(0.0)
    }

    /// Update the synergy score between two services.
    ///
    /// The score is clamped to [0.0, 1.0].
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] if `source` or `target` is empty.
    pub fn update_synergy(
        &self,
        source: &str,
        target: &str,
        score: f64,
    ) -> Result<()> {
        if source.is_empty() {
            return Err(Error::Validation("source must not be empty".into()));
        }
        if target.is_empty() {
            return Err(Error::Validation("target must not be empty".into()));
        }

        let clamped = score.clamp(0.0, 1.0);
        self.synergy_scores
            .write()
            .insert((source.into(), target.into()), clamped);
        Ok(())
    }

    /// Retrieve all bridges with [`BridgeStatus::Active`].
    #[must_use]
    pub fn get_active_bridges(&self) -> Vec<ServiceBridge> {
        self.bridges
            .read()
            .values()
            .filter(|b| b.status == BridgeStatus::Active)
            .cloned()
            .collect()
    }

    /// Retrieve all bridges with [`BridgeStatus::Failed`].
    #[must_use]
    pub fn get_failed_bridges(&self) -> Vec<ServiceBridge> {
        self.bridges
            .read()
            .values()
            .filter(|b| b.status == BridgeStatus::Failed)
            .cloned()
            .collect()
    }

    /// Return the total number of registered bridges.
    #[must_use]
    pub fn bridge_count(&self) -> usize {
        self.bridges.read().len()
    }

    /// Compute the overall synergy score as the average of all recorded synergy values.
    ///
    /// Returns `0.0` if no synergy scores exist.
    #[must_use]
    pub fn overall_synergy(&self) -> f64 {
        let synergy = self.synergy_scores.read();
        if synergy.is_empty() {
            return 0.0;
        }

        let total: f64 = synergy.values().sum();
        #[allow(clippy::cast_precision_loss)]
        {
            total / synergy.len() as f64
        }
    }

    /// Retrieve the loaded wire weights.
    #[must_use]
    pub fn wire_weights(&self) -> &[WireWeight] {
        &self.wire_weights
    }

    /// Get the wire weight for a specific source-target pair, if one exists.
    #[must_use]
    pub fn get_wire_weight(&self, source: &str, target: &str) -> Option<&WireWeight> {
        self.wire_weights
            .iter()
            .find(|w| w.source == source && w.target == target)
    }
}

impl Default for BridgeManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_bridge() {
        let mgr = BridgeManager::new();
        let result = mgr.register_bridge("synthex", "nais", WireProtocol::Rest);
        assert!(result.is_ok());

        let bridge_id = result.unwrap_or_default();
        assert!(!bridge_id.is_empty());

        let bridge = mgr.get_bridge(&bridge_id);
        assert!(bridge.is_ok());
        let bridge = bridge.unwrap_or_else(|_| ServiceBridge {
            bridge_id: String::new(),
            source_service: String::new(),
            target_service: String::new(),
            protocol: WireProtocol::Rest,
            status: BridgeStatus::Disconnected,
            health_score: 0.0,
            latency_ms: 0.0,
            request_count: 0,
            error_count: 0,
            created_at: Utc::now(),
            last_active: None,
        });
        assert_eq!(bridge.source_service, "synthex");
        assert_eq!(bridge.target_service, "nais");
        assert_eq!(bridge.status, BridgeStatus::Active);
        assert!((bridge.health_score - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_register_bridge_empty_source() {
        let mgr = BridgeManager::new();
        assert!(
            mgr.register_bridge("", "nais", WireProtocol::Rest)
                .is_err()
        );
    }

    #[test]
    fn test_register_bridge_empty_target() {
        let mgr = BridgeManager::new();
        assert!(
            mgr.register_bridge("synthex", "", WireProtocol::Rest)
                .is_err()
        );
    }

    #[test]
    fn test_deregister_bridge() {
        let mgr = BridgeManager::new();
        let bridge_id = mgr
            .register_bridge("synthex", "nais", WireProtocol::Rest)
            .unwrap_or_default();

        assert_eq!(mgr.bridge_count(), 1);
        assert!(mgr.deregister_bridge(&bridge_id).is_ok());
        assert_eq!(mgr.bridge_count(), 0);

        // Deregistering again should fail
        assert!(mgr.deregister_bridge(&bridge_id).is_err());
    }

    #[test]
    fn test_update_health() {
        let mgr = BridgeManager::new();
        let bridge_id = mgr
            .register_bridge("synthex", "nais", WireProtocol::Rest)
            .unwrap_or_default();

        // Active -> Degraded
        assert!(mgr.update_health(&bridge_id, 0.5).is_ok());
        let bridge = mgr.get_bridge(&bridge_id).unwrap_or_else(|_| ServiceBridge {
            bridge_id: String::new(),
            source_service: String::new(),
            target_service: String::new(),
            protocol: WireProtocol::Rest,
            status: BridgeStatus::Disconnected,
            health_score: 0.0,
            latency_ms: 0.0,
            request_count: 0,
            error_count: 0,
            created_at: Utc::now(),
            last_active: None,
        });
        assert_eq!(bridge.status, BridgeStatus::Degraded);
        assert!((bridge.health_score - 0.5).abs() < f64::EPSILON);

        // Degraded -> Failed
        assert!(mgr.update_health(&bridge_id, 0.1).is_ok());
        let bridge = mgr.get_bridge(&bridge_id).unwrap_or_else(|_| ServiceBridge {
            bridge_id: String::new(),
            source_service: String::new(),
            target_service: String::new(),
            protocol: WireProtocol::Rest,
            status: BridgeStatus::Disconnected,
            health_score: 0.0,
            latency_ms: 0.0,
            request_count: 0,
            error_count: 0,
            created_at: Utc::now(),
            last_active: None,
        });
        assert_eq!(bridge.status, BridgeStatus::Failed);

        // Clamping above 1.0
        assert!(mgr.update_health(&bridge_id, 2.0).is_ok());
        let bridge = mgr.get_bridge(&bridge_id).unwrap_or_else(|_| ServiceBridge {
            bridge_id: String::new(),
            source_service: String::new(),
            target_service: String::new(),
            protocol: WireProtocol::Rest,
            status: BridgeStatus::Disconnected,
            health_score: 0.0,
            latency_ms: 0.0,
            request_count: 0,
            error_count: 0,
            created_at: Utc::now(),
            last_active: None,
        });
        assert!((bridge.health_score - 1.0).abs() < f64::EPSILON);
        assert_eq!(bridge.status, BridgeStatus::Active);
    }

    #[test]
    fn test_update_health_nonexistent() {
        let mgr = BridgeManager::new();
        assert!(mgr.update_health("nonexistent", 0.5).is_err());
    }

    #[test]
    fn test_record_request() {
        let mgr = BridgeManager::new();
        let bridge_id = mgr
            .register_bridge("synthex", "nais", WireProtocol::Rest)
            .unwrap_or_default();

        // Record successful requests
        assert!(mgr.record_request(&bridge_id, 10.0, true).is_ok());
        assert!(mgr.record_request(&bridge_id, 20.0, true).is_ok());
        assert!(mgr.record_request(&bridge_id, 30.0, false).is_ok());

        let bridge = mgr.get_bridge(&bridge_id).unwrap_or_else(|_| ServiceBridge {
            bridge_id: String::new(),
            source_service: String::new(),
            target_service: String::new(),
            protocol: WireProtocol::Rest,
            status: BridgeStatus::Disconnected,
            health_score: 0.0,
            latency_ms: 0.0,
            request_count: 0,
            error_count: 0,
            created_at: Utc::now(),
            last_active: None,
        });
        assert_eq!(bridge.request_count, 3);
        assert_eq!(bridge.error_count, 1);
        // Average latency: (10 + 20 + 30) / 3 = 20
        assert!((bridge.latency_ms - 20.0).abs() < f64::EPSILON);
        assert!(bridge.last_active.is_some());
    }

    #[test]
    fn test_record_request_nonexistent() {
        let mgr = BridgeManager::new();
        assert!(mgr.record_request("nonexistent", 10.0, true).is_err());
    }

    #[test]
    fn test_bridges_for_service() {
        let mgr = BridgeManager::new();
        let _b1 = mgr.register_bridge("synthex", "nais", WireProtocol::Rest);
        let _b2 = mgr.register_bridge("synthex", "san-k7", WireProtocol::Rest);
        let _b3 = mgr.register_bridge("nais", "ccm", WireProtocol::Rest);

        let synthex_bridges = mgr.get_bridges_for_service("synthex");
        assert_eq!(synthex_bridges.len(), 2);

        let nais_bridges = mgr.get_bridges_for_service("nais");
        assert_eq!(nais_bridges.len(), 2); // as source AND as target

        let ccm_bridges = mgr.get_bridges_for_service("ccm");
        assert_eq!(ccm_bridges.len(), 1);

        let empty = mgr.get_bridges_for_service("nonexistent");
        assert!(empty.is_empty());
    }

    #[test]
    fn test_synergy_tracking() {
        let mgr = BridgeManager::new();

        // Default synergies from wire weights
        let synthex_synergy = mgr.get_synergy("maintenance-engine", "synthex");
        assert!(
            synthex_synergy > 0.0,
            "expected positive synergy from wire weights, got {synthex_synergy}"
        );

        // Update synergy
        assert!(mgr.update_synergy("synthex", "nais", 0.85).is_ok());
        assert!((mgr.get_synergy("synthex", "nais") - 0.85).abs() < f64::EPSILON);

        // Clamping
        assert!(mgr.update_synergy("a", "b", 1.5).is_ok());
        assert!((mgr.get_synergy("a", "b") - 1.0).abs() < f64::EPSILON);

        // Nonexistent pair
        assert!((mgr.get_synergy("x", "y")).abs() < f64::EPSILON);
    }

    #[test]
    fn test_synergy_validation() {
        let mgr = BridgeManager::new();
        assert!(mgr.update_synergy("", "target", 0.5).is_err());
        assert!(mgr.update_synergy("source", "", 0.5).is_err());
    }

    #[test]
    fn test_active_bridges() {
        let mgr = BridgeManager::new();
        let b1 = mgr
            .register_bridge("synthex", "nais", WireProtocol::Rest)
            .unwrap_or_default();
        let _b2 = mgr.register_bridge("synthex", "san-k7", WireProtocol::Rest);

        // All start as active
        assert_eq!(mgr.get_active_bridges().len(), 2);

        // Degrade one
        let _r = mgr.update_health(&b1, 0.2);
        assert_eq!(mgr.get_active_bridges().len(), 1);
        assert_eq!(mgr.get_failed_bridges().len(), 1);
    }

    #[test]
    fn test_overall_synergy() {
        let mgr = BridgeManager::new();

        // Should have initial synergy from wire weights
        let overall = mgr.overall_synergy();
        assert!(
            overall > 0.0,
            "expected positive overall synergy, got {overall}"
        );
        assert!(
            overall <= 1.0,
            "expected overall synergy <= 1.0, got {overall}"
        );
    }

    #[test]
    fn test_bridge_count() {
        let mgr = BridgeManager::new();
        assert_eq!(mgr.bridge_count(), 0);

        let _b1 = mgr.register_bridge("a", "b", WireProtocol::Rest);
        assert_eq!(mgr.bridge_count(), 1);

        let _b2 = mgr.register_bridge("c", "d", WireProtocol::Grpc);
        assert_eq!(mgr.bridge_count(), 2);
    }

    #[test]
    fn test_bridge_status_from_health() {
        assert_eq!(BridgeStatus::from_health_score(1.0), BridgeStatus::Active);
        assert_eq!(BridgeStatus::from_health_score(0.7), BridgeStatus::Active);
        assert_eq!(BridgeStatus::from_health_score(0.69), BridgeStatus::Degraded);
        assert_eq!(BridgeStatus::from_health_score(0.3), BridgeStatus::Degraded);
        assert_eq!(BridgeStatus::from_health_score(0.29), BridgeStatus::Failed);
        assert_eq!(BridgeStatus::from_health_score(0.0), BridgeStatus::Failed);
    }

    #[test]
    fn test_bridge_error_rate() {
        let bridge = ServiceBridge {
            bridge_id: "test".into(),
            source_service: "a".into(),
            target_service: "b".into(),
            protocol: WireProtocol::Rest,
            status: BridgeStatus::Active,
            health_score: 1.0,
            latency_ms: 10.0,
            request_count: 100,
            error_count: 5,
            created_at: Utc::now(),
            last_active: None,
        };
        assert!((bridge.error_rate() - 0.05).abs() < f64::EPSILON);

        let empty_bridge = ServiceBridge {
            bridge_id: "test2".into(),
            source_service: "a".into(),
            target_service: "b".into(),
            protocol: WireProtocol::Rest,
            status: BridgeStatus::Active,
            health_score: 1.0,
            latency_ms: 0.0,
            request_count: 0,
            error_count: 0,
            created_at: Utc::now(),
            last_active: None,
        };
        assert!((empty_bridge.error_rate()).abs() < f64::EPSILON);
    }

    #[test]
    fn test_wire_weights_loaded() {
        let mgr = BridgeManager::new();
        assert!(
            !mgr.wire_weights().is_empty(),
            "wire weights should be loaded from defaults"
        );

        let synthex_weight = mgr.get_wire_weight("maintenance-engine", "synthex");
        assert!(synthex_weight.is_some());
    }

    #[test]
    fn test_bridge_status_display() {
        assert_eq!(format!("{}", BridgeStatus::Active), "Active");
        assert_eq!(format!("{}", BridgeStatus::Degraded), "Degraded");
        assert_eq!(format!("{}", BridgeStatus::Failed), "Failed");
        assert_eq!(format!("{}", BridgeStatus::Disconnected), "Disconnected");
    }
}
