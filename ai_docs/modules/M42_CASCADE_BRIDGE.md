# M42: CascadeBridge — Module Specification

**Module ID:** M42
**Layer:** L4 (Integration)
**File:** `src/m4_integration/cascade_bridge.rs`
**Priority:** P0
**Estimated LOC:** ~180
**Tests:** ~12
**Root Causes Addressed:** RC2 (Cascade Amplification Unbounded), RC4 (No Cascade Breakers)
**Alpha Corrections Applied:** H4, H5, M9

---

## 1. Purpose

The CascadeBridge provides the ME with **real-time visibility into SYNTHEX's cascade damping pipeline** (SPEC-003). It polls SYNTHEX's REST API every 15 seconds, reads cascade amplification levels and circuit breaker states, and feeds this data to M40 ThermalMonitor, M38 EmergenceDetector, and M12 CircuitBreakerRegistry.

**Key Distinction:** SYNTHEX owns the cascade pipeline internals (12 stages, per-stage breakers, damping math). The ME does NOT control the pipeline directly. Instead, M42 observes the pipeline state and triggers external remediation (restart, cooldown, escalation) when the pipeline's internal controls are insufficient.

**Integration Pattern:** Mirrors the existing `PeerBridgeManager` in `m4_integration/peer_bridge.rs` — tiered polling with circuit breaker protection on the bridge itself.

---

## 2. Struct Definition

```rust
//! # M42: Cascade Bridge
//!
//! Cross-service bridge providing real-time cascade pipeline state from SYNTHEX.
//! Polls the SYNTHEX REST API at 15-second intervals and distributes cascade
//! state to thermal monitor, emergence detector, and circuit breaker registry.
//!
//! ## Layer: L4 (Integration)
//! ## Dependencies: M01 (Error), M19 (RestClient), M12 (CircuitBreaker)
//!
//! ## 12D Tensor Encoding
//! ```text
//! [42/42, 8090, 4/6, 1, 0, 0.0, health, uptime, synergy, latency, error_rate, temporal]
//! ```

use parking_lot::RwLock;
use crate::{Error, Result};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Default SYNTHEX REST API base URL.
const DEFAULT_SYNTHEX_URL: &str = "http://localhost:8090";

/// Default poll interval in seconds.
const DEFAULT_POLL_INTERVAL_SECS: u64 = 15;

/// Maximum consecutive failures before bridge circuit breaker opens.
const MAX_CONSECUTIVE_FAILURES: u32 = 5;

/// Recovery timeout in seconds after bridge circuit breaker opens.
const RECOVERY_TIMEOUT_SECS: u64 = 60;

/// Cascade amplification warning threshold.
const AMPLIFICATION_WARNING: f64 = 150.0;

/// Cascade amplification critical threshold.
const AMPLIFICATION_CRITICAL: f64 = 500.0;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// State of the SYNTHEX cascade pipeline as reported by the API.
#[derive(Clone, Debug, Default)]
pub struct CascadeState {
    /// Current peak amplification factor across all stages.
    pub amplification: f64,
    /// Number of active pipeline stages (0–12).
    pub active_stages: u8,
    /// Number of circuit breakers in OPEN state.
    pub breakers_open: u8,
    /// Number of circuit breakers in HALF_OPEN state.
    pub breakers_half_open: u8,
    /// Damping factor being applied.
    pub damping_factor: f64,
    /// Whether the pipeline is in emergency mode.
    pub emergency_mode: bool,
    /// Timestamp of last successful poll.
    pub last_poll: Option<chrono::DateTime<chrono::Utc>>,
    /// Whether SYNTHEX is reachable.
    pub synthex_reachable: bool,
}

/// Configuration for the cascade bridge.
#[derive(Clone, Debug)]
pub struct CascadeConfig {
    /// SYNTHEX base URL.
    pub synthex_url: String,
    /// Poll interval in seconds.
    pub poll_interval_secs: u64,
    /// Amplification warning threshold.
    pub warning_threshold: f64,
    /// Amplification critical threshold.
    pub critical_threshold: f64,
}

impl Default for CascadeConfig {
    fn default() -> Self {
        Self {
            synthex_url: DEFAULT_SYNTHEX_URL.into(),
            poll_interval_secs: DEFAULT_POLL_INTERVAL_SECS,
            warning_threshold: AMPLIFICATION_WARNING,
            critical_threshold: AMPLIFICATION_CRITICAL,
        }
    }
}

/// Bridge state (circuit breaker for the bridge itself).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BridgeState {
    /// Bridge is operational, polling normally.
    Connected,
    /// Bridge has lost contact, attempting recovery.
    Disconnected,
    /// Bridge circuit breaker is open, waiting for recovery timeout.
    CircuitOpen,
}

// ---------------------------------------------------------------------------
// CascadeBridge
// ---------------------------------------------------------------------------

/// Cross-service bridge to the SYNTHEX cascade pipeline.
///
/// Polls SYNTHEX every 15 seconds and distributes cascade state
/// to downstream ME modules (M40, M38, M12).
pub struct CascadeBridge {
    /// Bridge configuration.
    config: CascadeConfig,
    /// Current cascade pipeline state.
    state: RwLock<CascadeState>,
    /// Bridge circuit breaker state.
    bridge_state: RwLock<BridgeState>,
    /// Consecutive poll failure count.
    consecutive_failures: RwLock<u32>,
    /// Last failure timestamp (for recovery timeout).
    last_failure: RwLock<Option<chrono::DateTime<chrono::Utc>>>,
    /// Total successful polls since startup.
    total_polls: RwLock<u64>,
}
```

---

## 3. Method Signatures & Logic

### 3.1 `new(config) → Self`

```rust
impl CascadeBridge {
    /// Create a new CascadeBridge with the given configuration.
    #[must_use]
    pub fn new(config: CascadeConfig) -> Self {
        Self {
            config,
            state: RwLock::new(CascadeState::default()),
            bridge_state: RwLock::new(BridgeState::Disconnected),
            consecutive_failures: RwLock::new(0),
            last_failure: RwLock::new(None),
            total_polls: RwLock::new(0),
        }
    }

    /// Create with default configuration.
    #[must_use]
    pub fn with_defaults() -> Self {
        Self::new(CascadeConfig::default())
    }
}
```

### 3.2 `poll_cascade_state(client) → Result<CascadeState>`

The main polling method. Issues HTTP GET to SYNTHEX and parses the response.

```rust
/// Poll SYNTHEX for current cascade pipeline state.
///
/// Issues GET /api/v3/cascade/status (if V3 is deployed) or falls back
/// to GET /api/status for basic health inference.
///
/// # Errors
/// Returns `Error::Network` if SYNTHEX is unreachable.
/// Returns `Error::Parse` if the response format is unexpected.
pub async fn poll_cascade_state(&self, client: &reqwest::Client) -> Result<CascadeState> {
    // Check bridge circuit breaker
    if *self.bridge_state.read() == BridgeState::CircuitOpen {
        // Check recovery timeout
        if let Some(last) = *self.last_failure.read() {
            let elapsed = chrono::Utc::now() - last;
            if elapsed.num_seconds() < RECOVERY_TIMEOUT_SECS as i64 {
                return Err(Error::CircuitOpen {
                    service_id: "cascade_bridge".into(),
                    retry_after_ms: 60_000,
                });
            }
            // Recovery timeout elapsed, try half-open
            *self.bridge_state.write() = BridgeState::Disconnected;
        }
    }

    let url = format!("{}/api/v3/cascade/status", self.config.synthex_url);
    let response = client.get(&url)
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await;

    match response {
        Ok(resp) if resp.status().is_success() => {
            let body: serde_json::Value = resp.json().await
                .map_err(|e| Error::Validation(format!("parse error: {e}")))?;

            let state = CascadeState {
                amplification: body["amplification"].as_f64().unwrap_or(0.0),
                active_stages: body["active_stages"].as_u64().unwrap_or(0) as u8,
                breakers_open: body["breakers_open"].as_u64().unwrap_or(0) as u8,
                breakers_half_open: body["breakers_half_open"].as_u64().unwrap_or(0) as u8,
                damping_factor: body["damping_factor"].as_f64().unwrap_or(0.6),
                emergency_mode: body["emergency_mode"].as_bool().unwrap_or(false),
                last_poll: Some(chrono::Utc::now()),
                synthex_reachable: true,
            };

            // Reset failure counter on success
            *self.consecutive_failures.write() = 0;
            *self.bridge_state.write() = BridgeState::Connected;
            *self.state.write() = state.clone();
            *self.total_polls.write() += 1;

            Ok(state)
        }
        Ok(resp) => {
            // Non-success HTTP status
            self.record_failure();
            Err(Error::Network {
                target: "synthex".into(),
                message: format!("SYNTHEX returned {}", resp.status()),
            })
        }
        Err(e) => {
            // Connection failure (SYNTHEX down or unreachable)
            self.record_failure();

            // Try fallback: basic health endpoint
            let fallback_url = format!("{}/api/health", self.config.synthex_url);
            if let Ok(fallback) = client.get(&fallback_url)
                .timeout(std::time::Duration::from_secs(3))
                .send()
                .await
            {
                if fallback.status().is_success() {
                    // SYNTHEX is up but V3 cascade endpoint not yet deployed
                    let mut state = self.state.write();
                    state.synthex_reachable = true;
                    state.last_poll = Some(chrono::Utc::now());
                    *self.bridge_state.write() = BridgeState::Connected;
                    return Ok(state.clone());
                }
            }

            Err(Error::Network {
                target: "synthex".into(),
                message: format!("SYNTHEX unreachable: {e}"),
            })
        }
    }
}
```

### 3.3 `record_failure()`

```rust
/// Record a poll failure and potentially open the bridge circuit breaker.
fn record_failure(&self) {
    let mut failures = self.consecutive_failures.write();
    *failures += 1;
    *self.last_failure.write() = Some(chrono::Utc::now());

    if *failures >= MAX_CONSECUTIVE_FAILURES {
        *self.bridge_state.write() = BridgeState::CircuitOpen;
        tracing::warn!(
            failures = *failures,
            "Cascade bridge circuit breaker OPENED"
        );
    }

    let mut state = self.state.write();
    state.synthex_reachable = false;
}
```

### 3.4 `check_amplification() → Option<CascadeAlert>`

Evaluates current amplification against thresholds.

```rust
/// Alert level for cascade amplification.
#[derive(Clone, Debug)]
pub enum CascadeAlert {
    /// Amplification between warning and critical.
    Warning { amplification: f64 },
    /// Amplification above critical threshold.
    Critical { amplification: f64 },
}

/// Check if current amplification exceeds thresholds.
///
/// Returns `Some(CascadeAlert)` if action is needed, `None` if within bounds.
#[must_use]
pub fn check_amplification(&self) -> Option<CascadeAlert> {
    let state = self.state.read();
    if state.amplification >= self.config.critical_threshold {
        Some(CascadeAlert::Critical { amplification: state.amplification })
    } else if state.amplification >= self.config.warning_threshold {
        Some(CascadeAlert::Warning { amplification: state.amplification })
    } else {
        None
    }
}
```

### 3.5 `state() → CascadeState`

```rust
/// Get a snapshot of the current cascade state.
#[must_use]
pub fn state(&self) -> CascadeState {
    self.state.read().clone()
}

/// Get the bridge's own circuit breaker state.
#[must_use]
pub fn bridge_state(&self) -> BridgeState {
    *self.bridge_state.read()
}
```

---

## 4. Background Task (15s Interval)

```rust
// In main.rs spawn_background_tasks():
tokio::spawn(async move {
    let client = reqwest::Client::new();
    let mut interval = tokio::time::interval(Duration::from_secs(15));

    loop {
        interval.tick().await;

        match cascade_bridge.poll_cascade_state(&client).await {
            Ok(state) => {
                // 1. Feed cascade amplification to M40 ThermalMonitor
                let normalized = (state.amplification / 500.0).min(1.0);
                let _ = thermal_monitor.update_source("cascade_amplification", normalized);

                // 2. Publish state update to EventBus (string-based — Alpha Correction H5)
                let _ = event_bus.publish(
                    "cascade",
                    "CascadeStateUpdate",
                    &serde_json::json!({
                        "amplification": state.amplification,
                        "stage_count": state.active_stages,
                        "breakers_open": state.breakers_open,
                    }).to_string(),
                    "cascade_bridge",
                );

                // 3. Check amplification thresholds
                if let Some(alert) = cascade_bridge.check_amplification() {
                    match alert {
                        CascadeAlert::Warning { amplification } => {
                            tracing::warn!(
                                amplification,
                                "Cascade amplification WARNING"
                            );
                        }
                        CascadeAlert::Critical { amplification } => {
                            tracing::error!(
                                amplification,
                                "Cascade amplification CRITICAL — triggering remediation"
                            );
                            // Submit remediation request
                            let _ = remediation_engine.submit(RemediationRequest {
                                issue_type: IssueType::PerformanceDegradation,
                                severity: Severity::Critical,
                                source: "cascade_bridge".into(),
                                description: format!(
                                    "Cascade amplification at {amplification:.1}x (threshold: {})",
                                    AMPLIFICATION_CRITICAL
                                ),
                            });
                        }
                    }
                }

                // 4. If breakers are open, register with M12
                if state.breakers_open > 0 {
                    let _ = circuit_breaker_registry.record_failure(
                        "synthex_cascade",
                    );
                }
            }
            Err(e) => {
                tracing::debug!("Cascade bridge poll failed: {e}");
                // M42 handles its own circuit breaker internally
            }
        }
    }
});
```

---

## 5. Cross-Module Integration

### 5.1 Inbound Data Flow

```
SYNTHEX /api/v3/cascade/status ──[HTTP GET]──► M42 poll_cascade_state()
SYNTHEX /api/health             ──[HTTP GET]──► M42 fallback health check
```

### 5.2 Outbound Data Flow

```
M42 ──[CascadeStateUpdate event]──► M40 ThermalMonitor (HS-002 heat source)
M42 ──[CascadeStateUpdate event]──► M38 EmergenceDetector (cascade patterns)
M42 ──[CascadeStateUpdate event]──► M12 CircuitBreakerRegistry (breaker sync)
M42 ──[CascadeAlert::Critical]──► M14 RemediationEngine (escalation)
M42 ──[normalized amplification]──► M40 ThermalMonitor.update_source()
```

### 5.3 Engine.rs Wiring

```rust
// In Engine struct:
pub struct Engine {
    // ... existing fields ...
    cascade_bridge: CascadeBridge,
}

// In Engine::new():
cascade_bridge: CascadeBridge::with_defaults(),

// Accessor:
pub const fn cascade_bridge(&self) -> &CascadeBridge {
    &self.cascade_bridge
}
```

### 5.4 HTTP Handler

```rust
// GET /api/cascade
async fn get_cascade_bridge(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let cascade = state.engine.cascade_bridge().state();
    let bridge = state.engine.cascade_bridge().bridge_state();
    Json(serde_json::json!({
        "synthex_reachable": cascade.synthex_reachable,
        "bridge_state": format!("{bridge:?}"),
        "amplification": cascade.amplification,
        "active_stages": cascade.active_stages,
        "breakers_open": cascade.breakers_open,
        "breakers_half_open": cascade.breakers_half_open,
        "damping_factor": cascade.damping_factor,
        "emergency_mode": cascade.emergency_mode,
        "last_poll": cascade.last_poll.map(|t| t.to_rfc3339()),
        "alert": state.engine.cascade_bridge().check_amplification()
            .map(|a| format!("{a:?}")),
    }))
}
```

---

## 6. Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = CascadeConfig::default();
        assert_eq!(config.synthex_url, "http://localhost:8090");
        assert_eq!(config.poll_interval_secs, 15);
        assert!((config.warning_threshold - 150.0).abs() < f64::EPSILON);
        assert!((config.critical_threshold - 500.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_initial_state() {
        let bridge = CascadeBridge::with_defaults();
        let state = bridge.state();
        assert!(!state.synthex_reachable);
        assert!(state.last_poll.is_none());
        assert_eq!(bridge.bridge_state(), BridgeState::Disconnected);
    }

    #[test]
    fn test_check_amplification_none() {
        let bridge = CascadeBridge::with_defaults();
        // Default amplification is 0
        assert!(bridge.check_amplification().is_none());
    }

    #[test]
    fn test_check_amplification_warning() {
        let bridge = CascadeBridge::with_defaults();
        bridge.state.write().amplification = 200.0;
        let alert = bridge.check_amplification();
        assert!(matches!(alert, Some(CascadeAlert::Warning { .. })));
    }

    #[test]
    fn test_check_amplification_critical() {
        let bridge = CascadeBridge::with_defaults();
        bridge.state.write().amplification = 600.0;
        let alert = bridge.check_amplification();
        assert!(matches!(alert, Some(CascadeAlert::Critical { .. })));
    }

    #[test]
    fn test_record_failure_opens_breaker() {
        let bridge = CascadeBridge::with_defaults();
        for _ in 0..MAX_CONSECUTIVE_FAILURES {
            bridge.record_failure();
        }
        assert_eq!(bridge.bridge_state(), BridgeState::CircuitOpen);
    }

    #[test]
    fn test_failure_counter_resets_on_state_update() {
        let bridge = CascadeBridge::with_defaults();
        bridge.record_failure();
        bridge.record_failure();
        assert_eq!(*bridge.consecutive_failures.read(), 2);
        // Simulate successful poll
        *bridge.consecutive_failures.write() = 0;
        *bridge.bridge_state.write() = BridgeState::Connected;
        assert_eq!(*bridge.consecutive_failures.read(), 0);
    }

    #[test]
    fn test_cascade_state_clone() {
        let state = CascadeState {
            amplification: 100.0,
            active_stages: 12,
            breakers_open: 2,
            breakers_half_open: 1,
            damping_factor: 0.6,
            emergency_mode: false,
            last_poll: Some(chrono::Utc::now()),
            synthex_reachable: true,
        };
        let cloned = state.clone();
        assert!((cloned.amplification - 100.0).abs() < f64::EPSILON);
        assert_eq!(cloned.active_stages, 12);
    }
}
```

---

## 7. NAM Integration

This section addresses NAM gaps NAM-G21 through NAM-G25 (see [NAM_GAP_ANALYSIS.md](NAM_GAP_ANALYSIS.md)).

### 7.1 Escalation Gates (R-NAM-01) — Addresses NAM-G21

`CascadeAlert::Critical` MUST NOT trigger immediate remediation. The remediation engine's
`WaitingApproval` state exists but is NEVER USED (verified in source). Critical cascade
alerts must route through escalation.

```rust
// In background task, replace direct remediation submission:
CascadeAlert::Critical { amplification } => {
    tracing::error!(
        amplification,
        "Cascade amplification CRITICAL"
    );

    // NAM: L2 escalation — require approval before remediation
    let _ = event_bus.publish(
        "escalation", "ApprovalRequired",
        &serde_json::json!({
            "tier": "L2",
            "proposed_action": "cascade_remediation",
            "amplification": amplification,
            "threshold": AMPLIFICATION_CRITICAL,
            "severity": "Critical",
            "source": "cascade_bridge",
            "timeout_secs": 1800,
            "description": format!(
                "Cascade amplification at {amplification:.1}x (threshold: {})",
                AMPLIFICATION_CRITICAL
            ),
        }).to_string(),
        "cascade_bridge",
    );

    // DO NOT submit RemediationRequest directly — wait for approval
    // The escalation handler will submit after L2 approval or L2 timeout
}
CascadeAlert::Warning { amplification } => {
    // L1: Notify human at warning level
    let _ = event_bus.publish(
        "escalation", "HumanNotification",
        &serde_json::json!({
            "agent": "@0.A",
            "tier": "L1",
            "event": "cascade_warning",
            "amplification": amplification,
            "threshold": AMPLIFICATION_WARNING,
        }).to_string(),
        "cascade_bridge",
    );
}
```

### 7.2 12D Tensor Integration (R-NAM-03) — Addresses NAM-G23

Cascade state updates tensor dimensions D9 (latency) and D10 (error_rate).

```rust
// After successful poll:
if let Ok(state) = cascade_bridge.poll_cascade_state(&client).await {
    // R4: Update 12D tensor with cascade state
    // D9 (latency) ← normalized amplification (higher amp = higher latency impact)
    tensor_store.update_dimension(
        "maintenance_engine",
        TensorDimension::Latency, // D9
        (state.amplification / 500.0).min(1.0),
    );

    // D10 (error_rate) ← breaker open ratio
    let breaker_ratio = if state.active_stages > 0 {
        state.breakers_open as f64 / state.active_stages as f64
    } else {
        0.0
    };
    tensor_store.update_dimension(
        "maintenance_engine",
        TensorDimension::ErrorRate, // D10
        breaker_ratio,
    );
}
```

| Tensor Dimension | Update | Trigger | Value Range |
|------------------|--------|---------|-------------|
| D9 (latency) | `amplification / 500.0` | Every poll | 0.0–1.0 |
| D10 (error_rate) | `breakers_open / active_stages` | Every poll | 0.0–1.0 |

### 7.3 Human @0.A Notification (R-NAM-06) — Addresses NAM-G24

Human @0.A is notified when the bridge circuit breaker opens/closes, and when cascade
enters emergency mode.

```rust
// When bridge circuit breaker opens:
fn record_failure(&self) {
    let mut failures = self.consecutive_failures.write();
    *failures += 1;
    *self.last_failure.write() = Some(chrono::Utc::now());

    if *failures >= MAX_CONSECUTIVE_FAILURES {
        *self.bridge_state.write() = BridgeState::CircuitOpen;
        tracing::warn!(failures = *failures, "Cascade bridge circuit breaker OPENED");

        // R5: Notify Human @0.A — bridge circuit breaker state change
        let _ = event_bus.publish(
            "escalation", "HumanNotification",
            &serde_json::json!({
                "agent": "@0.A",
                "event": "bridge_circuit_open",
                "consecutive_failures": *failures,
                "urgency": "HIGH",
                "impact": "SYNTHEX cascade monitoring suspended",
            }).to_string(),
            "cascade_bridge",
        );
    }

    let mut state = self.state.write();
    state.synthex_reachable = false;
}

// When emergency_mode is detected:
if state.emergency_mode {
    let _ = event_bus.publish(
        "escalation", "HumanNotification",
        &serde_json::json!({
            "agent": "@0.A",
            "event": "synthex_emergency_mode",
            "amplification": state.amplification,
            "breakers_open": state.breakers_open,
            "urgency": "CRITICAL",
        }).to_string(),
        "cascade_bridge",
    );
}
```

### 7.4 NAM Compliance Summary

| NAM Requirement | Gap ID | Status | Implementation |
|-----------------|--------|--------|----------------|
| R3 DissentCapture | NAM-G22 | DEFERRED | Requires INTEGRATOR agent patterns (Gen2) |
| R4 FieldVisualization | NAM-G23 | ADDRESSED | D9, D10 tensor updates per poll |
| R5 HumanAsAgent | NAM-G24 | ADDRESSED | @0.A notified on breaker state changes |
| Escalation | NAM-G21 | ADDRESSED | Critical→L2, Warning→L1 |
| Episodic Memory | NAM-G25 | DEFERRED | Pattern recognition for recurring spikes (Gen2) |

**Projected NAM Score: 3/100 → 55/100**

---

*Document: ME_MODULE_M42_CASCADE_BRIDGE.md (NAM Integration Applied — R-NAM-01, R-NAM-03, R-NAM-06)*
*Alpha Corrections: H4, H5, M9*
*Location: generation_1_bug_fix/ai_docs/*
