//! # M46: Peer Bridge Manager
//!
//! Active health polling and communication bridge for ULTRAPLATE peer services.
//!
//! ## Layer: L4 (Integration)
//!
//! ## Features
//!
//! - Tiered health polling (15s/30s/60s intervals by service tier)
//! - Per-peer circuit breaker (opens at 5 consecutive failures, resets after 30s)
//! - Synergy computation across the mesh
//! - Event forwarding to SYNTHEX
//! - Service registration with SYNTHEX and SAN-K7
//!
//! ## Thread Safety
//!
//! All mutable state is protected by `RwLock` from `parking_lot`. The
//! manager requires only `&self` for all operations.

#![allow(clippy::missing_errors_doc)]
#![allow(clippy::module_name_repetitions)]

use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crate::{Error, Result};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Maximum consecutive failures before opening the circuit breaker.
const CIRCUIT_OPEN_THRESHOLD: u32 = 5;

/// Duration (seconds) a circuit stays open before allowing a retry.
const CIRCUIT_RESET_SECONDS: i64 = 30;

/// Default HTTP timeout for health polls.
const DEFAULT_POLL_TIMEOUT: Duration = Duration::from_secs(5);

// ---------------------------------------------------------------------------
// Peer Configuration
// ---------------------------------------------------------------------------

/// Configuration for a single peer service.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PeerConfig {
    /// Unique service identifier.
    pub service_id: String,
    /// Hostname or IP address.
    pub host: String,
    /// Port number.
    pub port: u16,
    /// Health endpoint path (e.g., `/api/health` or `/health`).
    pub health_path: String,
    /// Service tier (1=highest priority, 5=lowest).
    pub tier: u8,
    /// Weight multiplier for synergy computation.
    pub weight: f64,
    /// Polling interval in seconds.
    pub poll_interval_secs: u64,
}

impl PeerConfig {
    /// Build the full health URL for this peer.
    #[must_use]
    pub fn health_url(&self) -> String {
        format!("http://{}:{}{}", self.host, self.port, self.health_path)
    }
}

/// Per-peer health tracking state.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PeerHealthState {
    /// Service identifier.
    pub service_id: String,
    /// Whether the service is currently reachable.
    pub reachable: bool,
    /// Last known health score [0.0, 1.0].
    pub health_score: f64,
    /// Number of consecutive poll failures.
    pub consecutive_failures: u32,
    /// Average response latency in milliseconds.
    pub avg_latency_ms: f64,
    /// Computed synergy score [0.0, 1.0].
    pub synergy_score: f64,
    /// Whether the circuit breaker is open (blocking polls).
    pub circuit_open: bool,
    /// When the circuit was opened (for reset timing).
    pub circuit_opened_at: Option<DateTime<Utc>>,
    /// Service version string (if returned by health endpoint).
    pub version: Option<String>,
    /// Total successful polls.
    pub total_successes: u64,
    /// Total failed polls.
    pub total_failures: u64,
    /// Timestamp of last successful poll.
    pub last_success: Option<DateTime<Utc>>,
    /// Timestamp of last poll attempt.
    pub last_poll: Option<DateTime<Utc>>,
}

impl PeerHealthState {
    /// Create a new health state for a peer.
    #[must_use]
    fn new(service_id: &str) -> Self {
        Self {
            service_id: service_id.to_string(),
            reachable: false,
            health_score: 0.0,
            consecutive_failures: 0,
            avg_latency_ms: 0.0,
            synergy_score: 0.0,
            circuit_open: false,
            circuit_opened_at: None,
            version: None,
            total_successes: 0,
            total_failures: 0,
            last_success: None,
            last_poll: None,
        }
    }

    /// Compute synergy for this peer.
    ///
    /// Formula: `0.4 * success_rate + 0.3 * health + 0.2 * (1 - norm_latency) + 0.1 * recency`
    #[allow(clippy::cast_precision_loss)]
    fn compute_synergy(&mut self) {
        let total = self.total_successes + self.total_failures;
        let success_rate = if total == 0 {
            0.0
        } else {
            self.total_successes as f64 / total as f64
        };

        // Normalize latency: 0ms = 1.0, 2000ms = 0.0
        let norm_latency = (self.avg_latency_ms / 2000.0).clamp(0.0, 1.0);

        // Recency: 1.0 if polled within 60s, 0.0 if never
        let recency = self.last_success.map_or(0.0, |ts| {
            let age_secs = (Utc::now() - ts).num_seconds();
            if age_secs <= 60 {
                1.0
            } else {
                (1.0 - (age_secs as f64 / 300.0)).clamp(0.0, 1.0)
            }
        });

        // FMA chain: 0.4*success + 0.3*health + 0.2*(1-latency) + 0.1*recency
        self.synergy_score = 0.4f64.mul_add(
            success_rate,
            0.3f64.mul_add(
                self.health_score,
                0.2f64.mul_add(1.0 - norm_latency, 0.1 * recency),
            ),
        );
    }

    /// Record a successful poll.
    fn record_success(&mut self, latency_ms: f64, health: f64, version: Option<String>) {
        self.reachable = true;
        self.health_score = health;
        self.consecutive_failures = 0;
        self.total_successes += 1;
        self.last_success = Some(Utc::now());
        self.last_poll = Some(Utc::now());
        self.version = version;

        // Exponential moving average for latency
        if self.avg_latency_ms <= f64::EPSILON {
            self.avg_latency_ms = latency_ms;
        } else {
            self.avg_latency_ms = 0.8f64.mul_add(self.avg_latency_ms, 0.2 * latency_ms);
        }

        // Close circuit on success
        if self.circuit_open {
            self.circuit_open = false;
            self.circuit_opened_at = None;
        }

        self.compute_synergy();
    }

    /// Record a failed poll.
    fn record_failure(&mut self) {
        self.reachable = false;
        self.consecutive_failures += 1;
        self.total_failures += 1;
        self.last_poll = Some(Utc::now());

        // Open circuit at threshold
        if self.consecutive_failures >= CIRCUIT_OPEN_THRESHOLD && !self.circuit_open {
            self.circuit_open = true;
            self.circuit_opened_at = Some(Utc::now());
        }

        self.compute_synergy();
    }

    /// Check if the circuit should be reset (half-open after timeout).
    fn should_retry(&self) -> bool {
        if !self.circuit_open {
            return true;
        }
        self.circuit_opened_at.is_none_or(|opened| (Utc::now() - opened).num_seconds() >= CIRCUIT_RESET_SECONDS)
    }
}

/// Mesh-level health summary.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MeshHealthSummary {
    /// Total configured peers.
    pub total_peers: usize,
    /// Currently reachable peers.
    pub reachable_peers: usize,
    /// Peers with open circuit breakers.
    pub circuit_open_count: usize,
    /// Overall mesh synergy [0.0, 1.0].
    pub mesh_synergy: f64,
    /// Per-peer health states.
    pub peers: Vec<PeerHealthState>,
}

// ---------------------------------------------------------------------------
// Health response parsing
// ---------------------------------------------------------------------------

/// Minimal health response from a peer service.
#[derive(Deserialize)]
struct HealthResponse {
    #[serde(default = "default_healthy")]
    status: String,
    #[serde(default)]
    version: Option<String>,
}

fn default_healthy() -> String {
    "healthy".to_string()
}

// ---------------------------------------------------------------------------
// PeerBridgeManager
// ---------------------------------------------------------------------------

/// Active bridge communication manager for ULTRAPLATE peer services.
///
/// Maintains per-peer health state, performs tiered polling, and computes
/// mesh synergy scores. All operations are non-blocking and fail-silent.
pub struct PeerBridgeManager {
    /// HTTP client for polling.
    http_client: reqwest::Client,
    /// Per-peer health states.
    peer_states: RwLock<HashMap<String, PeerHealthState>>,
    /// Peer configurations.
    peer_configs: RwLock<HashMap<String, PeerConfig>>,
    /// Overall mesh synergy.
    mesh_synergy: RwLock<f64>,
    /// Whether polling is active.
    polling_active: AtomicBool,
}

impl PeerBridgeManager {
    /// Create a new `PeerBridgeManager` with default ULTRAPLATE peers.
    ///
    /// # Errors
    ///
    /// Returns an error if the HTTP client cannot be created.
    pub fn new() -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(DEFAULT_POLL_TIMEOUT)
            .build()
            .map_err(|e| Error::Network {
                target: "peer_bridge".into(),
                message: format!("Failed to create HTTP client: {e}"),
            })?;

        let configs = default_peer_configs();
        let mut states = HashMap::new();
        for config in &configs {
            states.insert(config.service_id.clone(), PeerHealthState::new(&config.service_id));
        }

        let config_map: HashMap<String, PeerConfig> = configs
            .into_iter()
            .map(|c| (c.service_id.clone(), c))
            .collect();

        Ok(Self {
            http_client: client,
            peer_states: RwLock::new(states),
            peer_configs: RwLock::new(config_map),
            mesh_synergy: RwLock::new(0.0),
            polling_active: AtomicBool::new(false),
        })
    }

    /// Create with custom peer configurations.
    ///
    /// # Errors
    ///
    /// Returns an error if the HTTP client cannot be created.
    pub fn with_configs(configs: Vec<PeerConfig>) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(DEFAULT_POLL_TIMEOUT)
            .build()
            .map_err(|e| Error::Network {
                target: "peer_bridge".into(),
                message: format!("Failed to create HTTP client: {e}"),
            })?;

        let mut states = HashMap::new();
        for config in &configs {
            states.insert(config.service_id.clone(), PeerHealthState::new(&config.service_id));
        }

        let config_map: HashMap<String, PeerConfig> = configs
            .into_iter()
            .map(|c| (c.service_id.clone(), c))
            .collect();

        Ok(Self {
            http_client: client,
            peer_states: RwLock::new(states),
            peer_configs: RwLock::new(config_map),
            mesh_synergy: RwLock::new(0.0),
            polling_active: AtomicBool::new(false),
        })
    }

    /// Check whether polling is active.
    #[must_use]
    pub fn is_polling(&self) -> bool {
        self.polling_active.load(Ordering::Relaxed)
    }

    /// Set the polling active flag.
    pub fn set_polling(&self, active: bool) {
        self.polling_active.store(active, Ordering::Relaxed);
    }

    /// Get the total number of configured peers.
    #[must_use]
    pub fn peer_count(&self) -> usize {
        self.peer_configs.read().len()
    }

    /// Get the number of currently reachable peers.
    #[must_use]
    pub fn reachable_count(&self) -> usize {
        self.peer_states.read().values().filter(|s| s.reachable).count()
    }

    /// Get the current mesh synergy score.
    #[must_use]
    pub fn mesh_synergy(&self) -> f64 {
        *self.mesh_synergy.read()
    }

    /// Get a snapshot of all peer health states.
    #[must_use]
    pub fn all_states(&self) -> Vec<PeerHealthState> {
        self.peer_states.read().values().cloned().collect()
    }

    /// Get the health state for a specific peer.
    #[must_use]
    pub fn peer_state(&self, service_id: &str) -> Option<PeerHealthState> {
        self.peer_states.read().get(service_id).cloned()
    }

    /// Get a mesh health summary.
    #[must_use]
    pub fn mesh_summary(&self) -> MeshHealthSummary {
        let peers: Vec<PeerHealthState> = self.peer_states.read().values().cloned().collect();
        let reachable = peers.iter().filter(|p| p.reachable).count();
        let circuit_open = peers.iter().filter(|p| p.circuit_open).count();

        MeshHealthSummary {
            total_peers: peers.len(),
            reachable_peers: reachable,
            circuit_open_count: circuit_open,
            mesh_synergy: *self.mesh_synergy.read(),
            peers,
        }
    }

    /// Poll a single peer's health endpoint.
    ///
    /// Updates the peer's health state based on the response.
    pub async fn poll_peer(&self, service_id: &str) -> Result<bool> {
        let url = self.peer_configs.read()
            .get(service_id)
            .ok_or_else(|| Error::ServiceNotFound(service_id.to_string()))?
            .health_url();

        // Check circuit breaker
        {
            let states = self.peer_states.read();
            if let Some(state) = states.get(service_id) {
                if !state.should_retry() {
                    return Ok(false);
                }
            }
        }

        let start = std::time::Instant::now();
        let response = self.http_client.get(&url).send().await;
        let latency_ms = start.elapsed().as_secs_f64() * 1000.0;

        match response {
            Ok(resp) if resp.status().is_success() => {
                let body: std::result::Result<HealthResponse, _> = resp.json().await;
                let (health, version) = body.map_or((1.0, None), |h| {
                    let score = if h.status == "healthy" { 1.0 } else { 0.5 };
                    (score, h.version)
                });

                let mut states = self.peer_states.write();
                if let Some(state) = states.get_mut(service_id) {
                    state.record_success(latency_ms, health, version);
                }
                drop(states);
                self.recompute_mesh_synergy();
                Ok(true)
            }
            Ok(_) | Err(_) => {
                let mut states = self.peer_states.write();
                if let Some(state) = states.get_mut(service_id) {
                    state.record_failure();
                }
                drop(states);
                self.recompute_mesh_synergy();
                Ok(false)
            }
        }
    }

    /// Poll all peers for a specific tier.
    pub async fn poll_tier(&self, tier: u8) {
        let service_ids: Vec<String> = {
            let configs = self.peer_configs.read();
            configs
                .values()
                .filter(|c| c.tier == tier)
                .map(|c| c.service_id.clone())
                .collect()
        };

        for service_id in service_ids {
            let _ = self.poll_peer(&service_id).await;
        }
    }

    /// Poll all configured peers.
    pub async fn poll_all(&self) {
        let service_ids: Vec<String> = {
            let configs = self.peer_configs.read();
            configs.keys().cloned().collect()
        };

        for service_id in service_ids {
            let _ = self.poll_peer(&service_id).await;
        }
    }

    /// Get peers grouped by tier for tiered polling.
    #[must_use]
    pub fn peers_by_tier(&self) -> HashMap<u8, Vec<String>> {
        let mut by_tier: HashMap<u8, Vec<String>> = HashMap::new();
        for config in self.peer_configs.read().values() {
            by_tier
                .entry(config.tier)
                .or_default()
                .push(config.service_id.clone());
        }
        by_tier
    }

    /// Get the polling interval for a given tier.
    #[must_use]
    pub const fn tier_interval_secs(tier: u8) -> u64 {
        match tier {
            1 => 15,
            2 | 3 => 30,
            _ => 60,
        }
    }

    /// Forward an event to SYNTHEX via POST.
    ///
    /// # Errors
    ///
    /// Returns network errors if the request fails.
    pub async fn forward_event(&self, event_json: &str) -> Result<bool> {
        let url = self.peer_configs.read()
            .get("synthex")
            .map_or_else(
                || "http://localhost:8090/api/events".to_string(),
                |c| format!("http://{}:{}/api/events", c.host, c.port),
            );

        let result = self.http_client
            .post(&url)
            .header("Content-Type", "application/json")
            .body(event_json.to_string())
            .send()
            .await;

        match result {
            Ok(resp) if resp.status().is_success() => Ok(true),
            Ok(_) | Err(_) => Ok(false),
        }
    }

    /// Register this service with SYNTHEX and SAN-K7.
    ///
    /// Best-effort: logs and swallows failures.
    pub async fn register_self(&self) {
        let registration = serde_json::json!({
            "service_id": "maintenance-engine",
            "port": 8080,
            "tier": 1,
            "weight": 1.5,
            "capabilities": [
                "health_monitoring",
                "auto_remediation",
                "hebbian_learning",
                "pbft_consensus",
                "tensor_encoding",
                "observer_layer"
            ],
            "version": "1.0.0",
        });

        let body = registration.to_string();

        // Register with SYNTHEX
        let _ = self.http_client
            .post("http://localhost:8090/api/services/register")
            .header("Content-Type", "application/json")
            .body(body.clone())
            .send()
            .await;

        // Register with SAN-K7
        let _ = self.http_client
            .post("http://localhost:8100/services/register")
            .header("Content-Type", "application/json")
            .body(body)
            .send()
            .await;
    }

    // ---------------------------------------------------------------
    // Internal helpers
    // ---------------------------------------------------------------

    /// Recompute the overall mesh synergy from all peer synergy scores.
    fn recompute_mesh_synergy(&self) {
        let states = self.peer_states.read();
        let configs = self.peer_configs.read();

        if states.is_empty() {
            *self.mesh_synergy.write() = 0.0;
            return;
        }

        let mut weighted_sum = 0.0;
        let mut weight_total = 0.0;

        for (id, state) in states.iter() {
            let weight = configs.get(id).map_or(1.0, |c| c.weight);
            weighted_sum = weight.mul_add(state.synergy_score, weighted_sum);
            weight_total += weight;
        }

        let synergy = if weight_total > f64::EPSILON {
            weighted_sum / weight_total
        } else {
            0.0
        };

        drop(states);
        drop(configs);
        *self.mesh_synergy.write() = synergy;
    }
}

// ---------------------------------------------------------------------------
// Default Peer Configs
// ---------------------------------------------------------------------------

/// Default ULTRAPLATE peer configurations.
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn default_peer_configs() -> Vec<PeerConfig> {
    vec![
        // Tier 1: Core (15s polling)
        PeerConfig {
            service_id: "synthex".into(),
            host: "localhost".into(),
            port: 8090,
            health_path: "/api/health".into(),
            tier: 1,
            weight: 1.5,
            poll_interval_secs: 15,
        },
        PeerConfig {
            service_id: "san-k7".into(),
            host: "localhost".into(),
            port: 8100,
            health_path: "/health".into(),
            tier: 1,
            weight: 1.5,
            poll_interval_secs: 15,
        },
        // Tier 2: Intelligence (30s polling)
        PeerConfig {
            service_id: "nais".into(),
            host: "localhost".into(),
            port: 8101,
            health_path: "/health".into(),
            tier: 2,
            weight: 1.3,
            poll_interval_secs: 30,
        },
        PeerConfig {
            service_id: "codesynthor-v7".into(),
            host: "localhost".into(),
            port: 8110,
            health_path: "/health".into(),
            tier: 2,
            weight: 1.3,
            poll_interval_secs: 30,
        },
        PeerConfig {
            service_id: "devops-engine".into(),
            host: "localhost".into(),
            port: 8081,
            health_path: "/health".into(),
            tier: 2,
            weight: 1.3,
            poll_interval_secs: 30,
        },
        // Tier 3: Integration (30s polling)
        PeerConfig {
            service_id: "tool-library".into(),
            host: "localhost".into(),
            port: 8105,
            health_path: "/health".into(),
            tier: 3,
            weight: 1.2,
            poll_interval_secs: 30,
        },
        PeerConfig {
            service_id: "ccm".into(),
            host: "localhost".into(),
            port: 8104,
            health_path: "/health".into(),
            tier: 3,
            weight: 1.2,
            poll_interval_secs: 30,
        },
        // library-agent (8083) removed: disabled in devenv, was dragging fitness tensor
        // Tier 4: Orchestration (60s polling)
        PeerConfig {
            service_id: "prometheus-swarm".into(),
            host: "localhost".into(),
            port: 10001,
            health_path: "/health".into(),
            tier: 4,
            weight: 1.1,
            poll_interval_secs: 60,
        },
        PeerConfig {
            service_id: "architect-agent".into(),
            host: "localhost".into(),
            port: 9001,
            health_path: "/health".into(),
            tier: 4,
            weight: 1.1,
            poll_interval_secs: 60,
        },
        // Tier 5: Execution (60s polling)
        PeerConfig {
            service_id: "bash-engine".into(),
            host: "localhost".into(),
            port: 8102,
            health_path: "/health".into(),
            tier: 5,
            weight: 1.0,
            poll_interval_secs: 60,
        },
        PeerConfig {
            service_id: "tool-maker".into(),
            host: "localhost".into(),
            port: 8103,
            health_path: "/health".into(),
            tier: 5,
            weight: 1.0,
            poll_interval_secs: 60,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Construction Tests ────────────────────────────────────────

    #[test]
    fn test_new_creates_manager() {
        let manager = PeerBridgeManager::new();
        assert!(manager.is_ok());
    }

    #[test]
    fn test_default_peer_count() {
        let manager = PeerBridgeManager::new().expect("create manager");
        assert_eq!(manager.peer_count(), 11); // library-agent removed
    }

    #[test]
    fn test_initial_reachable_count_is_zero() {
        let manager = PeerBridgeManager::new().expect("create manager");
        assert_eq!(manager.reachable_count(), 0);
    }

    #[test]
    fn test_initial_mesh_synergy_is_zero() {
        let manager = PeerBridgeManager::new().expect("create manager");
        assert!(manager.mesh_synergy().abs() < f64::EPSILON);
    }

    #[test]
    fn test_polling_initially_inactive() {
        let manager = PeerBridgeManager::new().expect("create manager");
        assert!(!manager.is_polling());
    }

    // ── Custom Config Tests ───────────────────────────────────────

    #[test]
    fn test_with_empty_configs() {
        let manager = PeerBridgeManager::with_configs(vec![]);
        assert!(manager.is_ok());
        assert_eq!(manager.expect("create").peer_count(), 0);
    }

    #[test]
    fn test_with_single_config() {
        let configs = vec![PeerConfig {
            service_id: "test-svc".into(),
            host: "localhost".into(),
            port: 9999,
            health_path: "/health".into(),
            tier: 1,
            weight: 1.0,
            poll_interval_secs: 15,
        }];
        let manager = PeerBridgeManager::with_configs(configs).expect("create");
        assert_eq!(manager.peer_count(), 1);
    }

    // ── Peer Config Tests ─────────────────────────────────────────

    #[test]
    fn test_peer_config_health_url() {
        let config = PeerConfig {
            service_id: "test".into(),
            host: "localhost".into(),
            port: 8080,
            health_path: "/api/health".into(),
            tier: 1,
            weight: 1.0,
            poll_interval_secs: 15,
        };
        assert_eq!(config.health_url(), "http://localhost:8080/api/health");
    }

    #[test]
    fn test_default_peer_configs_has_11() {
        let configs = default_peer_configs();
        assert_eq!(configs.len(), 11); // library-agent removed
    }

    #[test]
    fn test_default_configs_include_synthex() {
        let configs = default_peer_configs();
        assert!(configs.iter().any(|c| c.service_id == "synthex"));
    }

    #[test]
    fn test_default_configs_include_san_k7() {
        let configs = default_peer_configs();
        assert!(configs.iter().any(|c| c.service_id == "san-k7"));
    }

    #[test]
    fn test_tier_1_configs_have_15s_interval() {
        let configs = default_peer_configs();
        for config in configs.iter().filter(|c| c.tier == 1) {
            assert_eq!(config.poll_interval_secs, 15);
        }
    }

    #[test]
    fn test_tier_5_configs_have_60s_interval() {
        let configs = default_peer_configs();
        for config in configs.iter().filter(|c| c.tier == 5) {
            assert_eq!(config.poll_interval_secs, 60);
        }
    }

    // ── PeerHealthState Tests ─────────────────────────────────────

    #[test]
    fn test_new_health_state_is_unreachable() {
        let state = PeerHealthState::new("test");
        assert!(!state.reachable);
        assert!(state.health_score.abs() < f64::EPSILON);
    }

    #[test]
    fn test_record_success_marks_reachable() {
        let mut state = PeerHealthState::new("test");
        state.record_success(10.0, 1.0, None);
        assert!(state.reachable);
        assert!((state.health_score - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_record_success_resets_failures() {
        let mut state = PeerHealthState::new("test");
        state.record_failure();
        state.record_failure();
        assert_eq!(state.consecutive_failures, 2);
        state.record_success(10.0, 1.0, None);
        assert_eq!(state.consecutive_failures, 0);
    }

    #[test]
    fn test_record_failure_increments_count() {
        let mut state = PeerHealthState::new("test");
        state.record_failure();
        assert_eq!(state.consecutive_failures, 1);
        state.record_failure();
        assert_eq!(state.consecutive_failures, 2);
    }

    #[test]
    fn test_circuit_opens_at_threshold() {
        let mut state = PeerHealthState::new("test");
        for _ in 0..CIRCUIT_OPEN_THRESHOLD {
            state.record_failure();
        }
        assert!(state.circuit_open);
        assert!(state.circuit_opened_at.is_some());
    }

    #[test]
    fn test_circuit_does_not_open_below_threshold() {
        let mut state = PeerHealthState::new("test");
        for _ in 0..(CIRCUIT_OPEN_THRESHOLD - 1) {
            state.record_failure();
        }
        assert!(!state.circuit_open);
    }

    #[test]
    fn test_success_closes_circuit() {
        let mut state = PeerHealthState::new("test");
        for _ in 0..CIRCUIT_OPEN_THRESHOLD {
            state.record_failure();
        }
        assert!(state.circuit_open);
        state.record_success(10.0, 1.0, None);
        assert!(!state.circuit_open);
    }

    #[test]
    fn test_should_retry_when_circuit_closed() {
        let state = PeerHealthState::new("test");
        assert!(state.should_retry());
    }

    #[test]
    fn test_should_not_retry_when_circuit_just_opened() {
        let mut state = PeerHealthState::new("test");
        for _ in 0..CIRCUIT_OPEN_THRESHOLD {
            state.record_failure();
        }
        assert!(!state.should_retry());
    }

    #[test]
    fn test_synergy_increases_with_success() {
        let mut state = PeerHealthState::new("test");
        state.record_success(10.0, 1.0, None);
        assert!(state.synergy_score > 0.0);
    }

    #[test]
    fn test_synergy_stays_in_range() {
        let mut state = PeerHealthState::new("test");
        for _ in 0..100 {
            state.record_success(5.0, 1.0, None);
        }
        assert!(state.synergy_score >= 0.0);
        assert!(state.synergy_score <= 1.0);
    }

    #[test]
    fn test_latency_ema_after_multiple_successes() {
        let mut state = PeerHealthState::new("test");
        state.record_success(100.0, 1.0, None);
        assert!((state.avg_latency_ms - 100.0).abs() < f64::EPSILON);
        state.record_success(50.0, 1.0, None);
        // EMA: 0.8 * 100 + 0.2 * 50 = 90
        assert!((state.avg_latency_ms - 90.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_version_stored_on_success() {
        let mut state = PeerHealthState::new("test");
        state.record_success(10.0, 1.0, Some("1.2.3".into()));
        assert_eq!(state.version.as_deref(), Some("1.2.3"));
    }

    #[test]
    fn test_total_counters() {
        let mut state = PeerHealthState::new("test");
        state.record_success(10.0, 1.0, None);
        state.record_success(10.0, 1.0, None);
        state.record_failure();
        assert_eq!(state.total_successes, 2);
        assert_eq!(state.total_failures, 1);
    }

    // ── Manager State Tests ───────────────────────────────────────

    #[test]
    fn test_all_states_returns_all_peers() {
        let manager = PeerBridgeManager::new().expect("create manager");
        let states = manager.all_states();
        assert_eq!(states.len(), 11); // library-agent removed
    }

    #[test]
    fn test_peer_state_returns_known_peer() {
        let manager = PeerBridgeManager::new().expect("create manager");
        let state = manager.peer_state("synthex");
        assert!(state.is_some());
    }

    #[test]
    fn test_peer_state_returns_none_for_unknown() {
        let manager = PeerBridgeManager::new().expect("create manager");
        let state = manager.peer_state("nonexistent");
        assert!(state.is_none());
    }

    #[test]
    fn test_mesh_summary_structure() {
        let manager = PeerBridgeManager::new().expect("create manager");
        let summary = manager.mesh_summary();
        assert_eq!(summary.total_peers, 11); // library-agent removed
        assert_eq!(summary.reachable_peers, 0);
        assert_eq!(summary.circuit_open_count, 0);
    }

    #[test]
    fn test_set_polling_flag() {
        let manager = PeerBridgeManager::new().expect("create manager");
        assert!(!manager.is_polling());
        manager.set_polling(true);
        assert!(manager.is_polling());
        manager.set_polling(false);
        assert!(!manager.is_polling());
    }

    // ── Tier Tests ────────────────────────────────────────────────

    #[test]
    fn test_peers_by_tier() {
        let manager = PeerBridgeManager::new().expect("create manager");
        let by_tier = manager.peers_by_tier();
        assert!(by_tier.contains_key(&1));
        assert!(by_tier.contains_key(&2));
        assert_eq!(by_tier.get(&1).map_or(0, Vec::len), 2);
    }

    #[test]
    fn test_tier_interval_secs() {
        assert_eq!(PeerBridgeManager::tier_interval_secs(1), 15);
        assert_eq!(PeerBridgeManager::tier_interval_secs(2), 30);
        assert_eq!(PeerBridgeManager::tier_interval_secs(3), 30);
        assert_eq!(PeerBridgeManager::tier_interval_secs(4), 60);
        assert_eq!(PeerBridgeManager::tier_interval_secs(5), 60);
    }

    // ── Mesh Synergy Tests ────────────────────────────────────────

    #[test]
    fn test_recompute_mesh_synergy_empty() {
        let manager = PeerBridgeManager::with_configs(vec![]).expect("create");
        manager.recompute_mesh_synergy();
        assert!(manager.mesh_synergy().abs() < f64::EPSILON);
    }

    #[test]
    fn test_recompute_mesh_synergy_with_success() {
        let configs = vec![PeerConfig {
            service_id: "test".into(),
            host: "localhost".into(),
            port: 9999,
            health_path: "/health".into(),
            tier: 1,
            weight: 1.0,
            poll_interval_secs: 15,
        }];
        let manager = PeerBridgeManager::with_configs(configs).expect("create");

        // Simulate a success
        {
            let mut states = manager.peer_states.write();
            if let Some(state) = states.get_mut("test") {
                state.record_success(10.0, 1.0, None);
            }
        }
        manager.recompute_mesh_synergy();
        assert!(manager.mesh_synergy() > 0.0);
    }

    // ── Serialization Tests ───────────────────────────────────────

    #[test]
    fn test_peer_health_state_serialization() {
        let state = PeerHealthState::new("test-svc");
        let json = serde_json::to_string(&state);
        assert!(json.is_ok());
    }

    #[test]
    fn test_mesh_summary_serialization() {
        let manager = PeerBridgeManager::new().expect("create manager");
        let summary = manager.mesh_summary();
        let json = serde_json::to_string(&summary);
        assert!(json.is_ok());
    }

    #[test]
    fn test_peer_config_serialization() {
        let config = PeerConfig {
            service_id: "test".into(),
            host: "localhost".into(),
            port: 8080,
            health_path: "/health".into(),
            tier: 1,
            weight: 1.0,
            poll_interval_secs: 15,
        };
        let json = serde_json::to_string(&config);
        assert!(json.is_ok());
        let parsed: std::result::Result<PeerConfig, _> =
            serde_json::from_str(&json.expect("serialize"));
        assert!(parsed.is_ok());
    }

    // ── Async Tests ───────────────────────────────────────────────

    #[tokio::test]
    async fn test_poll_unknown_peer_returns_error() {
        let manager = PeerBridgeManager::new().expect("create manager");
        let result = manager.poll_peer("nonexistent").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_poll_unreachable_peer_records_failure() {
        let configs = vec![PeerConfig {
            service_id: "unreachable".into(),
            host: "127.0.0.1".into(),
            port: 1, // Port 1 should not be listening
            health_path: "/health".into(),
            tier: 5,
            weight: 1.0,
            poll_interval_secs: 60,
        }];
        let manager = PeerBridgeManager::with_configs(configs).expect("create");
        let result = manager.poll_peer("unreachable").await;
        assert!(result.is_ok());
        assert!(!result.expect("poll result"));

        let state = manager.peer_state("unreachable");
        assert!(state.is_some());
        let state = state.expect("state exists");
        assert!(!state.reachable);
        assert_eq!(state.consecutive_failures, 1);
    }

    #[tokio::test]
    async fn test_poll_all_with_no_peers() {
        let manager = PeerBridgeManager::with_configs(vec![]).expect("create");
        manager.poll_all().await;
        // Should complete without error
    }

    #[tokio::test]
    async fn test_forward_event_to_unreachable() {
        let manager = PeerBridgeManager::with_configs(vec![]).expect("create");
        let result = manager.forward_event(r#"{"type":"test"}"#).await;
        assert!(result.is_ok());
        assert!(!result.expect("forward result"));
    }

    // --- Additional tests to reach 50+ ---

    #[test]
    fn test_tier_interval_secs_unknown_tier() {
        // Tiers > 3 should default to 60
        assert_eq!(PeerBridgeManager::tier_interval_secs(6), 60);
        assert_eq!(PeerBridgeManager::tier_interval_secs(0), 60);
        assert_eq!(PeerBridgeManager::tier_interval_secs(255), 60);
    }

    #[test]
    fn test_peers_by_tier_tier_5_has_entries() {
        let manager = PeerBridgeManager::new().expect("create manager");
        let by_tier = manager.peers_by_tier();
        assert!(by_tier.contains_key(&5));
        assert!(!by_tier.get(&5).map_or(true, Vec::is_empty));
    }

    #[test]
    fn test_default_configs_cover_all_tiers() {
        let configs = default_peer_configs();
        let tiers: std::collections::HashSet<u8> = configs.iter().map(|c| c.tier).collect();
        assert!(tiers.contains(&1));
        assert!(tiers.contains(&2));
        assert!(tiers.contains(&3));
        assert!(tiers.contains(&4));
        assert!(tiers.contains(&5));
    }

    #[test]
    fn test_peer_health_state_initial_counters() {
        let state = PeerHealthState::new("test");
        assert_eq!(state.total_successes, 0);
        assert_eq!(state.total_failures, 0);
        assert!(state.last_success.is_none());
        assert!(state.last_poll.is_none());
        assert!(state.version.is_none());
    }

    #[test]
    fn test_synergy_zero_with_no_data() {
        let state = PeerHealthState::new("test");
        assert!(state.synergy_score.abs() < f64::EPSILON);
    }

    #[test]
    fn test_record_failure_marks_unreachable() {
        let mut state = PeerHealthState::new("test");
        state.record_success(10.0, 1.0, None); // first make reachable
        assert!(state.reachable);
        state.record_failure();
        assert!(!state.reachable);
    }

    #[test]
    fn test_record_failure_sets_last_poll() {
        let mut state = PeerHealthState::new("test");
        state.record_failure();
        assert!(state.last_poll.is_some());
    }

    #[test]
    fn test_record_success_sets_last_success_and_last_poll() {
        let mut state = PeerHealthState::new("test");
        state.record_success(10.0, 0.9, None);
        assert!(state.last_success.is_some());
        assert!(state.last_poll.is_some());
    }

    #[test]
    fn test_circuit_opened_at_set_when_threshold_reached() {
        let mut state = PeerHealthState::new("test");
        for _ in 0..CIRCUIT_OPEN_THRESHOLD {
            state.record_failure();
        }
        assert!(state.circuit_opened_at.is_some());
    }

    #[test]
    fn test_success_clears_circuit_opened_at() {
        let mut state = PeerHealthState::new("test");
        for _ in 0..CIRCUIT_OPEN_THRESHOLD {
            state.record_failure();
        }
        assert!(state.circuit_opened_at.is_some());
        state.record_success(10.0, 1.0, None);
        assert!(state.circuit_opened_at.is_none());
    }

    #[test]
    fn test_peer_config_all_weights_positive() {
        let configs = default_peer_configs();
        for config in &configs {
            assert!(config.weight > 0.0, "weight for {} should be positive", config.service_id);
        }
    }

    #[test]
    fn test_peer_config_all_ports_nonzero() {
        let configs = default_peer_configs();
        for config in &configs {
            assert!(config.port > 0, "port for {} should be nonzero", config.service_id);
        }
    }

    #[test]
    fn test_mesh_summary_serialization_roundtrip() {
        let manager = PeerBridgeManager::new().expect("create manager");
        let summary = manager.mesh_summary();
        let json = serde_json::to_string(&summary).unwrap_or_default();
        let parsed: std::result::Result<MeshHealthSummary, _> = serde_json::from_str(&json);
        assert!(parsed.is_ok());
    }

    #[tokio::test]
    async fn test_poll_tier_with_no_matching_peers() {
        let manager = PeerBridgeManager::with_configs(vec![]).expect("create");
        manager.poll_tier(1).await;
        // Should complete without error
    }
}
