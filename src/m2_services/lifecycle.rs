//! # M11: Lifecycle Manager
//!
//! Service lifecycle management for the ULTRAPLATE Developer Environment.
//!
//! Tracks the lifecycle of every registered service through a finite state
//! machine with validated transitions, restart counting with exponential
//! backoff, and a full transition history. Thread-safe via interior
//! mutability (`parking_lot::RwLock`).
//!
//! ## Layer: L2 (Services)
//! ## Module: M11
//! ## Dependencies: L1 (Error, Timestamp, `ModuleId`, `SignalBus`, `TensorContributor`)
//!
//! ## Trait: [`LifecycleOps`]
//!
//! All methods are `&self` (C2) with interior mutability via `RwLock`.
//! Methods returning data through `RwLock` return OWNED types (C7).
//!
//! ## Valid State Transitions
//!
//! | From | To | Trigger |
//! |------|----|---------|
//! | Stopped | Starting | `start_service` |
//! | Starting | Running | `mark_running` |
//! | Starting | Failed | `mark_failed` |
//! | Running | Stopping | `stop_service` |
//! | Running | Failed | `mark_failed` |
//! | Stopping | Stopped | `mark_stopped` |
//! | Failed | Starting | `start_service` / `restart_service` |
//!
//! ## 12D Tensor Contribution (C3)
//!
//! | Dimension | Value |
//! |-----------|-------|
//! | D6 (`health_score`) | fraction of services in Running state |
//! | D7 (uptime) | proxy: `1.0 - avg(restart_count / max_restarts)` |
//!
//! ## Signal Emission (C6)
//!
//! `mark_failed()` emits degradation signal, `mark_running()` emits
//! improvement signal. `restart_service()` emits recovery signal.
//!
//! ## Related Documentation
//! - [Layer Specification](../../ai_docs/layers/L02_SERVICES.md)
//! - [Service Registry](../../service_registry/SERVICE_REGISTRY.md)

use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use parking_lot::RwLock;

use super::{RestartConfig, ServiceStatus, ServiceTier};
use crate::m1_foundation::shared_types::{CoverageBitmap, DimensionIndex, ModuleId, Timestamp};
use crate::m1_foundation::signals::{HealthSignal, SignalBus};
use crate::m1_foundation::tensor_registry::{ContributedTensor, ContributorKind, TensorContributor};
use crate::m1_foundation::MetricsRegistry;
use crate::{Error, Result, Tensor12D};

// ============================================================================
// Constants
// ============================================================================

/// Maximum transition history entries per service.
const DEFAULT_MAX_HISTORY: usize = 100;

// ============================================================================
// LifecycleTransition
// ============================================================================

/// A single recorded state transition for a service.
///
/// Every successful lifecycle operation produces one of these entries,
/// stored in chronological order inside the owning [`LifecycleEntry`].
#[derive(Clone, Debug)]
pub struct LifecycleTransition {
    /// State before the transition.
    pub from: ServiceStatus,
    /// State after the transition.
    pub to: ServiceStatus,
    /// Human-readable reason for the transition.
    pub reason: String,
    /// Timestamp at which the transition occurred (C5: Timestamp, not chrono).
    pub timestamp: Timestamp,
}

impl fmt::Display for LifecycleTransition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} -> {}: {}", self.from, self.to, self.reason)
    }
}

// ============================================================================
// LifecycleAction
// ============================================================================

/// High-level lifecycle commands that can be issued to the manager.
///
/// These are used as declarative intents; the manager validates feasibility
/// and applies the appropriate state transitions.
#[derive(Clone, Debug)]
pub enum LifecycleAction {
    /// Request to start a service.
    Start {
        /// Target service identifier.
        service_id: String,
    },
    /// Request to stop a service.
    Stop {
        /// Target service identifier.
        service_id: String,
        /// Whether to attempt a graceful shutdown.
        graceful: bool,
    },
    /// Request to restart a service (stop then start).
    Restart {
        /// Target service identifier.
        service_id: String,
        /// Reason for the restart.
        reason: String,
    },
    /// Request a health probe for a service.
    HealthCheck {
        /// Target service identifier.
        service_id: String,
    },
}

impl fmt::Display for LifecycleAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Start { service_id } => write!(f, "Start({service_id})"),
            Self::Stop {
                service_id,
                graceful,
            } => write!(f, "Stop({service_id}, graceful={graceful})"),
            Self::Restart {
                service_id,
                reason,
            } => write!(f, "Restart({service_id}, {reason})"),
            Self::HealthCheck { service_id } => write!(f, "HealthCheck({service_id})"),
        }
    }
}

// ============================================================================
// LifecycleEntry
// ============================================================================

/// Per-service lifecycle tracking state.
///
/// Each registered service has exactly one `LifecycleEntry` that records its
/// current and previous status, a complete transition history, and restart
/// bookkeeping (count, backoff).
///
/// # Construction
///
/// Use [`LifecycleEntryBuilder`] for ergonomic construction:
///
/// ```rust
/// use maintenance_engine::m2_services::lifecycle::LifecycleEntryBuilder;
/// use maintenance_engine::m2_services::{RestartConfig, ServiceTier};
///
/// let entry = LifecycleEntryBuilder::new("synthex", "SYNTHEX Engine", ServiceTier::Tier1)
///     .max_restarts(10)
///     .initial_backoff(std::time::Duration::from_secs(2))
///     .build();
///
/// assert_eq!(entry.service_id, "synthex");
/// assert_eq!(entry.config.max_restarts, 10);
/// ```
#[derive(Clone, Debug)]
pub struct LifecycleEntry {
    /// Unique service identifier.
    pub service_id: String,
    /// Human-readable service name.
    pub name: String,
    /// Service tier for priority weighting.
    pub tier: ServiceTier,
    /// Current lifecycle state.
    pub current_state: ServiceStatus,
    /// State immediately before the last transition, if any.
    pub previous_state: Option<ServiceStatus>,
    /// Ordered list of all transitions (oldest first).
    pub transition_history: Vec<LifecycleTransition>,
    /// Number of restarts performed since the last manual reset.
    pub restart_count: u32,
    /// Restart configuration (max restarts, backoff settings).
    pub config: RestartConfig,
    /// Current backoff duration (doubles with each restart, capped at max).
    pub current_backoff: Duration,
    /// Timestamp when this entry was created (C5: Timestamp, not chrono).
    pub created_at: Timestamp,
    /// Timestamp of the most recent state transition.
    pub last_transition: Timestamp,
}

impl fmt::Display for LifecycleEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} ({}): {} [restarts: {}/{}]",
            self.service_id, self.tier, self.current_state, self.restart_count,
            self.config.max_restarts
        )
    }
}

// ============================================================================
// LifecycleEntryBuilder
// ============================================================================

/// Builder for [`LifecycleEntry`] with sensible defaults.
///
/// Required fields: `service_id`, `name`, `tier`.
///
/// # Examples
///
/// ```rust
/// use maintenance_engine::m2_services::lifecycle::LifecycleEntryBuilder;
/// use maintenance_engine::m2_services::ServiceTier;
///
/// let entry = LifecycleEntryBuilder::new("nais", "NAIS", ServiceTier::Tier2)
///     .build();
///
/// assert_eq!(entry.service_id, "nais");
/// assert_eq!(entry.config.max_restarts, 5);
/// ```
#[derive(Clone, Debug)]
pub struct LifecycleEntryBuilder {
    service_id: String,
    name: String,
    tier: ServiceTier,
    config: RestartConfig,
}

impl LifecycleEntryBuilder {
    /// Create a new builder with the required fields.
    #[must_use]
    pub fn new(
        service_id: impl Into<String>,
        name: impl Into<String>,
        tier: ServiceTier,
    ) -> Self {
        Self {
            service_id: service_id.into(),
            name: name.into(),
            tier,
            config: RestartConfig::default(),
        }
    }

    /// Override the full restart configuration.
    #[must_use]
    pub const fn restart_config(mut self, config: RestartConfig) -> Self {
        self.config = config;
        self
    }

    /// Override the maximum number of allowed restarts.
    #[must_use]
    pub const fn max_restarts(mut self, max: u32) -> Self {
        self.config.max_restarts = max;
        self
    }

    /// Override the initial restart backoff duration (C8: Duration, not ms).
    #[must_use]
    pub const fn initial_backoff(mut self, backoff: Duration) -> Self {
        self.config.initial_backoff = backoff;
        self
    }

    /// Override the maximum backoff duration (cap for exponential growth).
    #[must_use]
    pub const fn max_backoff(mut self, max: Duration) -> Self {
        self.config.max_backoff = max;
        self
    }

    /// Consume the builder and produce a [`LifecycleEntry`].
    ///
    /// The entry starts in [`ServiceStatus::Stopped`] with no transition
    /// history and the initial backoff from the restart configuration.
    #[must_use]
    pub fn build(self) -> LifecycleEntry {
        let now = Timestamp::now();
        LifecycleEntry {
            service_id: self.service_id,
            name: self.name,
            tier: self.tier,
            current_state: ServiceStatus::Stopped,
            previous_state: None,
            transition_history: Vec::new(),
            restart_count: 0,
            current_backoff: self.config.initial_backoff,
            config: self.config,
            created_at: now,
            last_transition: now,
        }
    }
}

// ============================================================================
// Transition validation
// ============================================================================

/// Check whether a state transition is permitted by the lifecycle FSM.
///
/// Returns `true` if the transition from `from` to `to` is valid.
#[must_use]
pub const fn is_valid_transition(from: ServiceStatus, to: ServiceStatus) -> bool {
    matches!(
        (from, to),
        (
            ServiceStatus::Stopped | ServiceStatus::Failed,
            ServiceStatus::Starting,
        ) | (
            ServiceStatus::Starting,
            ServiceStatus::Running | ServiceStatus::Failed,
        ) | (
            ServiceStatus::Running,
            ServiceStatus::Stopping | ServiceStatus::Failed,
        ) | (ServiceStatus::Stopping, ServiceStatus::Stopped)
    )
}

/// Health score for a service status (used for signal emission and tensor).
const fn status_health_score(status: ServiceStatus) -> f64 {
    match status {
        ServiceStatus::Running => 1.0,
        ServiceStatus::Starting | ServiceStatus::Stopping => 0.5,
        ServiceStatus::Stopped | ServiceStatus::Failed => 0.0,
    }
}

/// Push a transition onto an entry without validation (caller guarantees validity).
fn record_transition(
    entry: &mut LifecycleEntry,
    to: ServiceStatus,
    reason: &str,
    ts: Timestamp,
) {
    entry.transition_history.push(LifecycleTransition {
        from: entry.current_state,
        to,
        reason: reason.to_owned(),
        timestamp: ts,
    });
    entry.previous_state = Some(entry.current_state);
    entry.current_state = to;
    entry.last_transition = ts;
}

// ============================================================================
// LifecycleOps (trait)
// ============================================================================

/// Trait for service lifecycle operations.
///
/// All methods are `&self` (C2 constraint) with interior mutability via
/// `parking_lot::RwLock`. Methods returning data through `RwLock` return
/// owned types, not references (C7 constraint).
pub trait LifecycleOps: Send + Sync + fmt::Debug {
    /// Register a service for lifecycle tracking.
    ///
    /// The service starts in [`ServiceStatus::Stopped`] state.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] if a service with the same ID is
    /// already registered.
    fn register(
        &self,
        service_id: &str,
        name: &str,
        tier: ServiceTier,
        config: RestartConfig,
    ) -> Result<()>;

    /// Remove a service from lifecycle tracking.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ServiceNotFound`] if the service is not registered.
    fn deregister(&self, service_id: &str) -> Result<()>;

    /// Start a service (Stopped | Failed -> Starting).
    ///
    /// # Errors
    ///
    /// Returns [`Error::ServiceNotFound`] or [`Error::Validation`] if the
    /// transition is not permitted.
    fn start_service(&self, service_id: &str) -> Result<()>;

    /// Mark a service as running (Starting -> Running).
    ///
    /// # Errors
    ///
    /// Returns [`Error::ServiceNotFound`] or [`Error::Validation`] if the
    /// service is not in Starting state.
    fn mark_running(&self, service_id: &str) -> Result<()>;

    /// Mark a service as failed (Starting | Running -> Failed).
    ///
    /// # Errors
    ///
    /// Returns [`Error::ServiceNotFound`] or [`Error::Validation`] if the
    /// transition is not permitted.
    fn mark_failed(&self, service_id: &str) -> Result<()>;

    /// Stop a service (Running -> Stopping).
    ///
    /// # Errors
    ///
    /// Returns [`Error::ServiceNotFound`] or [`Error::Validation`] if the
    /// service is not in Running state.
    fn stop_service(&self, service_id: &str) -> Result<()>;

    /// Mark a service as stopped (Stopping -> Stopped).
    ///
    /// # Errors
    ///
    /// Returns [`Error::ServiceNotFound`] or [`Error::Validation`] if the
    /// service is not in Stopping state.
    fn mark_stopped(&self, service_id: &str) -> Result<()>;

    /// Restart a service (compound: stop if running, then start).
    ///
    /// Returns the backoff [`Duration`] to wait before the service should
    /// be marked as running. Increments the restart counter and doubles
    /// the backoff (capped at `max_backoff`).
    ///
    /// # Errors
    ///
    /// Returns [`Error::ServiceNotFound`], [`Error::Validation`] if the
    /// service cannot be restarted (wrong state or restart limit reached).
    fn restart_service(&self, service_id: &str) -> Result<Duration>;

    /// Get the current lifecycle status of a service.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ServiceNotFound`] if the service is not registered.
    fn get_status(&self, service_id: &str) -> Result<ServiceStatus>;

    /// Get a full clone of the lifecycle entry for a service (C7: owned).
    ///
    /// # Errors
    ///
    /// Returns [`Error::ServiceNotFound`] if the service is not registered.
    fn get_entry(&self, service_id: &str) -> Result<LifecycleEntry>;

    /// Get the transition history for a service (C7: owned).
    ///
    /// Results are ordered oldest-first.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ServiceNotFound`] if the service is not registered.
    fn get_history(&self, service_id: &str) -> Result<Vec<LifecycleTransition>>;

    /// Check if a service can be restarted (`restart_count` < `max_restarts`).
    ///
    /// # Errors
    ///
    /// Returns [`Error::ServiceNotFound`] if the service is not registered.
    fn can_restart(&self, service_id: &str) -> Result<bool>;

    /// Get the current exponential backoff duration (C8: Duration).
    ///
    /// # Errors
    ///
    /// Returns [`Error::ServiceNotFound`] if the service is not registered.
    fn get_restart_backoff(&self, service_id: &str) -> Result<Duration>;

    /// Check if a service is registered.
    fn is_registered(&self, service_id: &str) -> bool;

    /// Number of tracked services.
    fn service_count(&self) -> usize;

    /// Get IDs of all services in Running state.
    fn get_all_running(&self) -> Vec<String>;

    /// Get IDs of all services in Failed state.
    fn get_all_failed(&self) -> Vec<String>;

    /// Reset restart counter and backoff to initial values.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ServiceNotFound`] if the service is not registered.
    fn reset_restart_count(&self, service_id: &str) -> Result<()>;
}

// ============================================================================
// LifecycleManager
// ============================================================================

/// Central lifecycle manager for all ULTRAPLATE services.
///
/// Holds per-service [`LifecycleEntry`] records behind a
/// [`parking_lot::RwLock`] for safe concurrent access. All mutating
/// operations validate the requested transition against the lifecycle FSM.
///
/// # Examples
///
/// ```rust
/// use maintenance_engine::m2_services::lifecycle::{LifecycleManager, LifecycleOps};
/// use maintenance_engine::m2_services::{RestartConfig, ServiceStatus, ServiceTier};
///
/// let manager = LifecycleManager::new();
/// manager.register("synthex", "SYNTHEX Engine", ServiceTier::Tier1, RestartConfig::default())
///     .ok();
///
/// manager.start_service("synthex").ok();
/// manager.mark_running("synthex").ok();
///
/// let status = manager.get_status("synthex");
/// assert_eq!(status.ok(), Some(ServiceStatus::Running));
/// ```
pub struct LifecycleManager {
    /// Per-service lifecycle entries, keyed by service ID.
    entries: RwLock<HashMap<String, LifecycleEntry>>,
    /// Optional signal bus for health transition events (C6).
    signal_bus: Option<Arc<SignalBus>>,
    /// Optional metrics registry for recording counters/gauges.
    #[allow(dead_code)]
    metrics: Option<Arc<MetricsRegistry>>,
}

impl fmt::Debug for LifecycleManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let entries = self.entries.read();
        let count = entries.len();
        drop(entries);
        f.debug_struct("LifecycleManager")
            .field("service_count", &count)
            .field("has_signal_bus", &self.signal_bus.is_some())
            .finish_non_exhaustive()
    }
}

impl Default for LifecycleManager {
    fn default() -> Self {
        Self::new()
    }
}

impl LifecycleManager {
    /// Create a new, empty lifecycle manager with no signal bus or metrics.
    #[must_use]
    pub fn new() -> Self {
        Self {
            entries: RwLock::new(HashMap::new()),
            signal_bus: None,
            metrics: None,
        }
    }

    /// Create a lifecycle manager with a signal bus for health event emission.
    #[must_use]
    pub fn with_signal_bus(bus: Arc<SignalBus>) -> Self {
        Self {
            entries: RwLock::new(HashMap::new()),
            signal_bus: Some(bus),
            metrics: None,
        }
    }

    /// Create a lifecycle manager with signal bus and metrics registry.
    #[must_use]
    pub fn with_signal_bus_and_metrics(
        bus: Arc<SignalBus>,
        metrics: Arc<MetricsRegistry>,
    ) -> Self {
        Self {
            entries: RwLock::new(HashMap::new()),
            signal_bus: Some(bus),
            metrics: Some(metrics),
        }
    }

    /// Apply a validated state transition, emitting a signal if health changes.
    ///
    /// Acquires write lock, validates transition, updates entry, releases lock,
    /// then emits signal outside the lock scope.
    fn apply_transition(
        &self,
        service_id: &str,
        to: ServiceStatus,
        reason: &str,
    ) -> Result<()> {
        let signal_data = {
            let mut entries = self.entries.write();
            let entry = entries
                .get_mut(service_id)
                .ok_or_else(|| Error::ServiceNotFound(service_id.to_owned()))?;

            let from = entry.current_state;
            if !is_valid_transition(from, to) {
                return Err(Error::Validation(format!(
                    "Invalid lifecycle transition for '{service_id}': {from} -> {to}"
                )));
            }

            let now = Timestamp::now();
            record_transition(entry, to, reason, now);

            // Trim history if exceeded
            if entry.transition_history.len() > DEFAULT_MAX_HISTORY {
                let excess = entry.transition_history.len() - DEFAULT_MAX_HISTORY;
                entry.transition_history.drain(..excess);
            }

            let from_score = status_health_score(from);
            let to_score = status_health_score(to);

            let result = if (from_score - to_score).abs() > f64::EPSILON {
                Some((from_score, to_score, now, reason.to_owned()))
            } else {
                None
            };
            drop(entries);
            result
        };

        if let (Some(bus), Some((from_score, to_score, ts, reason_str))) =
            (&self.signal_bus, signal_data)
        {
            bus.emit_health(&HealthSignal {
                module_id: ModuleId::M11,
                previous_health: from_score,
                current_health: to_score,
                timestamp: ts,
                reason: reason_str,
            });
        }

        Ok(())
    }
}

// ============================================================================
// LifecycleOps implementation
// ============================================================================

impl LifecycleOps for LifecycleManager {
    fn register(
        &self,
        service_id: &str,
        name: &str,
        tier: ServiceTier,
        config: RestartConfig,
    ) -> Result<()> {
        let mut entries = self.entries.write();
        if entries.contains_key(service_id) {
            return Err(Error::Validation(format!(
                "Service '{service_id}' is already registered in the lifecycle manager"
            )));
        }

        let entry = LifecycleEntryBuilder::new(service_id, name, tier)
            .restart_config(config)
            .build();
        entries.insert(service_id.to_owned(), entry);
        drop(entries);
        Ok(())
    }

    fn deregister(&self, service_id: &str) -> Result<()> {
        let mut entries = self.entries.write();
        entries
            .remove(service_id)
            .map(|_| ())
            .ok_or_else(|| Error::ServiceNotFound(service_id.to_owned()))
    }

    fn start_service(&self, service_id: &str) -> Result<()> {
        self.apply_transition(service_id, ServiceStatus::Starting, "start requested")
    }

    fn mark_running(&self, service_id: &str) -> Result<()> {
        self.apply_transition(service_id, ServiceStatus::Running, "service ready")
    }

    fn mark_failed(&self, service_id: &str) -> Result<()> {
        self.apply_transition(service_id, ServiceStatus::Failed, "service failure")
    }

    fn stop_service(&self, service_id: &str) -> Result<()> {
        self.apply_transition(service_id, ServiceStatus::Stopping, "stop requested")
    }

    fn mark_stopped(&self, service_id: &str) -> Result<()> {
        self.apply_transition(service_id, ServiceStatus::Stopped, "service stopped")
    }

    fn restart_service(&self, service_id: &str) -> Result<Duration> {
        let (backoff, signal_data) = {
            let mut entries = self.entries.write();
            let entry = entries
                .get_mut(service_id)
                .ok_or_else(|| Error::ServiceNotFound(service_id.to_owned()))?;

            // Check eligibility
            if entry.restart_count >= entry.config.max_restarts {
                return Err(Error::Validation(format!(
                    "Service '{}' has reached restart limit ({}/{})",
                    service_id, entry.restart_count, entry.config.max_restarts
                )));
            }

            let now = Timestamp::now();
            let from_state = entry.current_state;
            let from_score = status_health_score(from_state);

            match entry.current_state {
                ServiceStatus::Running => {
                    record_transition(entry, ServiceStatus::Stopping, "restart: stopping", now);
                    record_transition(entry, ServiceStatus::Stopped, "restart: stopped", now);
                    record_transition(entry, ServiceStatus::Starting, "restart: restarting", now);
                }
                ServiceStatus::Failed => {
                    record_transition(
                        entry,
                        ServiceStatus::Starting,
                        "restart: recovering from failure",
                        now,
                    );
                }
                other => {
                    return Err(Error::Validation(format!(
                        "Cannot restart service '{service_id}' in {other} state"
                    )));
                }
            }

            // Capture current backoff BEFORE doubling (this is the wait time)
            let backoff = entry.current_backoff;

            // Increment restart counter
            entry.restart_count = entry.restart_count.saturating_add(1);

            // Double backoff for next restart (capped at max)
            entry.current_backoff = entry
                .current_backoff
                .checked_mul(2)
                .unwrap_or(entry.config.max_backoff)
                .min(entry.config.max_backoff);

            // Trim history
            if entry.transition_history.len() > DEFAULT_MAX_HISTORY {
                let excess = entry.transition_history.len() - DEFAULT_MAX_HISTORY;
                entry.transition_history.drain(..excess);
            }

            let to_score = status_health_score(ServiceStatus::Starting);
            let sig = if (from_score - to_score).abs() > f64::EPSILON {
                Some((from_score, to_score, now))
            } else {
                None
            };

            let result = (backoff, sig);
            drop(entries);
            result
        };

        if let (Some(bus), Some((from_score, to_score, ts))) = (&self.signal_bus, signal_data) {
            bus.emit_health(&HealthSignal {
                module_id: ModuleId::M11,
                previous_health: from_score,
                current_health: to_score,
                timestamp: ts,
                reason: format!("Service '{service_id}' restarted"),
            });
        }

        Ok(backoff)
    }

    fn get_status(&self, service_id: &str) -> Result<ServiceStatus> {
        let entries = self.entries.read();
        entries
            .get(service_id)
            .map(|e| e.current_state)
            .ok_or_else(|| Error::ServiceNotFound(service_id.to_owned()))
    }

    fn get_entry(&self, service_id: &str) -> Result<LifecycleEntry> {
        let entries = self.entries.read();
        entries
            .get(service_id)
            .cloned()
            .ok_or_else(|| Error::ServiceNotFound(service_id.to_owned()))
    }

    fn get_history(&self, service_id: &str) -> Result<Vec<LifecycleTransition>> {
        let entries = self.entries.read();
        entries
            .get(service_id)
            .map(|e| e.transition_history.clone())
            .ok_or_else(|| Error::ServiceNotFound(service_id.to_owned()))
    }

    fn can_restart(&self, service_id: &str) -> Result<bool> {
        let entries = self.entries.read();
        let entry = entries
            .get(service_id)
            .ok_or_else(|| Error::ServiceNotFound(service_id.to_owned()))?;
        let result = entry.restart_count < entry.config.max_restarts;
        drop(entries);
        Ok(result)
    }

    fn get_restart_backoff(&self, service_id: &str) -> Result<Duration> {
        let entries = self.entries.read();
        let entry = entries
            .get(service_id)
            .ok_or_else(|| Error::ServiceNotFound(service_id.to_owned()))?;
        let backoff = entry.current_backoff;
        drop(entries);
        Ok(backoff)
    }

    fn is_registered(&self, service_id: &str) -> bool {
        self.entries.read().contains_key(service_id)
    }

    fn service_count(&self) -> usize {
        self.entries.read().len()
    }

    fn get_all_running(&self) -> Vec<String> {
        let entries = self.entries.read();
        entries
            .iter()
            .filter(|(_, e)| e.current_state == ServiceStatus::Running)
            .map(|(id, _)| id.clone())
            .collect()
    }

    fn get_all_failed(&self) -> Vec<String> {
        let entries = self.entries.read();
        entries
            .iter()
            .filter(|(_, e)| e.current_state == ServiceStatus::Failed)
            .map(|(id, _)| id.clone())
            .collect()
    }

    fn reset_restart_count(&self, service_id: &str) -> Result<()> {
        let mut entries = self.entries.write();
        let entry = entries
            .get_mut(service_id)
            .ok_or_else(|| Error::ServiceNotFound(service_id.to_owned()))?;
        entry.restart_count = 0;
        entry.current_backoff = entry.config.initial_backoff;
        drop(entries);
        Ok(())
    }
}

// ============================================================================
// TensorContributor (C3)
// ============================================================================

impl TensorContributor for LifecycleManager {
    /// Contribute D6 (% running) and D7 (uptime proxy) to the 12D tensor.
    #[allow(clippy::cast_precision_loss)]
    fn contribute(&self) -> ContributedTensor {
        let entries = self.entries.read();
        let total = entries.len();

        let coverage = CoverageBitmap::EMPTY
            .with_dimension(DimensionIndex::HealthScore)
            .with_dimension(DimensionIndex::Uptime);

        if total == 0 {
            drop(entries);
            return ContributedTensor {
                tensor: Tensor12D::default(),
                coverage,
                kind: ContributorKind::Snapshot,
            };
        }

        // D6: fraction of services in Running state
        let running = entries
            .values()
            .filter(|e| e.current_state == ServiceStatus::Running)
            .count();
        let running_ratio = running as f64 / total as f64;

        // D7: uptime proxy = 1.0 - avg(restart_count / max_restarts)
        let restart_ratios: f64 = entries
            .values()
            .map(|e| {
                if e.config.max_restarts == 0 {
                    0.0
                } else {
                    f64::from(e.restart_count) / f64::from(e.config.max_restarts)
                }
            })
            .sum();
        let avg_restart_ratio = restart_ratios / total as f64;
        let uptime_proxy = (1.0 - avg_restart_ratio).clamp(0.0, 1.0);
        drop(entries);

        let mut dims = [0.0f64; 12];
        dims[DimensionIndex::HealthScore as usize] = running_ratio;
        dims[DimensionIndex::Uptime as usize] = uptime_proxy;

        ContributedTensor {
            tensor: Tensor12D::new(dims),
            coverage,
            kind: ContributorKind::Snapshot,
        }
    }

    fn contributor_kind(&self) -> ContributorKind {
        ContributorKind::Snapshot
    }

    fn module_id(&self) -> &'static str {
        "M11"
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // Helper: create a manager with a single service registered.
    fn setup_manager() -> LifecycleManager {
        let mgr = LifecycleManager::new();
        mgr.register("svc", "Test Service", ServiceTier::Tier3, RestartConfig::default())
            .ok();
        mgr
    }

    // Helper: create a manager with a service in Running state.
    fn setup_running() -> LifecycleManager {
        let mgr = setup_manager();
        mgr.start_service("svc").ok();
        mgr.mark_running("svc").ok();
        mgr
    }

    // ================================================================
    // [COMPILE] — trait object safety, Send+Sync assertions
    // ================================================================

    #[test]
    fn test_lifecycle_manager_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<LifecycleManager>();
    }

    #[test]
    fn test_lifecycle_ops_trait_object_safe() {
        fn assert_object_safe(_: &dyn LifecycleOps) {}
        let mgr = LifecycleManager::new();
        assert_object_safe(&mgr);
    }

    #[test]
    fn test_tensor_contributor_implemented() {
        fn assert_tensor_contributor(_: &dyn TensorContributor) {}
        let mgr = LifecycleManager::new();
        assert_tensor_contributor(&mgr);
    }

    // ================================================================
    // [BASIC] — happy path, construction, defaults
    // ================================================================

    #[test]
    fn test_new_manager_empty() {
        let mgr = LifecycleManager::new();
        assert_eq!(mgr.service_count(), 0);
        assert!(mgr.get_all_running().is_empty());
        assert!(mgr.get_all_failed().is_empty());
    }

    #[test]
    fn test_register_service() {
        let mgr = LifecycleManager::new();
        let result =
            mgr.register("synthex", "SYNTHEX Engine", ServiceTier::Tier1, RestartConfig::default());
        assert!(result.is_ok());
        assert_eq!(mgr.service_count(), 1);
        assert!(mgr.is_registered("synthex"));

        let status = mgr.get_status("synthex");
        assert!(status.is_ok());
        assert_eq!(status.ok(), Some(ServiceStatus::Stopped));
    }

    #[test]
    fn test_register_with_custom_config() {
        let mgr = LifecycleManager::new();
        let config = RestartConfig::new(10, Duration::from_secs(2), Duration::from_secs(60));
        mgr.register("svc", "Svc", ServiceTier::Tier2, config).ok();

        let entry = mgr.get_entry("svc");
        assert!(entry.is_ok());
        if let Ok(e) = entry {
            assert_eq!(e.config.max_restarts, 10);
            assert_eq!(e.config.initial_backoff, Duration::from_secs(2));
            assert_eq!(e.config.max_backoff, Duration::from_secs(60));
            assert_eq!(e.current_backoff, Duration::from_secs(2));
        }
    }

    #[test]
    fn test_deregister_service() {
        let mgr = setup_manager();
        assert!(mgr.is_registered("svc"));
        let result = mgr.deregister("svc");
        assert!(result.is_ok());
        assert!(!mgr.is_registered("svc"));
        assert_eq!(mgr.service_count(), 0);
    }

    #[test]
    fn test_start_service_from_stopped() {
        let mgr = setup_manager();
        let result = mgr.start_service("svc");
        assert!(result.is_ok());
        assert_eq!(mgr.get_status("svc").ok(), Some(ServiceStatus::Starting));
    }

    #[test]
    fn test_mark_running() {
        let mgr = setup_manager();
        mgr.start_service("svc").ok();
        let result = mgr.mark_running("svc");
        assert!(result.is_ok());
        assert_eq!(mgr.get_status("svc").ok(), Some(ServiceStatus::Running));
    }

    #[test]
    fn test_stop_service() {
        let mgr = setup_running();
        let result = mgr.stop_service("svc");
        assert!(result.is_ok());
        assert_eq!(mgr.get_status("svc").ok(), Some(ServiceStatus::Stopping));
    }

    #[test]
    fn test_mark_stopped() {
        let mgr = setup_running();
        mgr.stop_service("svc").ok();
        let result = mgr.mark_stopped("svc");
        assert!(result.is_ok());
        assert_eq!(mgr.get_status("svc").ok(), Some(ServiceStatus::Stopped));
    }

    #[test]
    fn test_mark_failed_from_starting() {
        let mgr = setup_manager();
        mgr.start_service("svc").ok();
        let result = mgr.mark_failed("svc");
        assert!(result.is_ok());
        assert_eq!(mgr.get_status("svc").ok(), Some(ServiceStatus::Failed));
    }

    #[test]
    fn test_mark_failed_from_running() {
        let mgr = setup_running();
        let result = mgr.mark_failed("svc");
        assert!(result.is_ok());
        assert_eq!(mgr.get_status("svc").ok(), Some(ServiceStatus::Failed));
    }

    #[test]
    fn test_get_status() {
        let mgr = setup_manager();
        let status = mgr.get_status("svc");
        assert!(status.is_ok());
        assert_eq!(status.ok(), Some(ServiceStatus::Stopped));
    }

    #[test]
    fn test_get_entry_clone() {
        let mgr = setup_manager();
        let entry = mgr.get_entry("svc");
        assert!(entry.is_ok());
        if let Ok(e) = entry {
            assert_eq!(e.service_id, "svc");
            assert_eq!(e.name, "Test Service");
            assert_eq!(e.tier, ServiceTier::Tier3);
            assert_eq!(e.current_state, ServiceStatus::Stopped);
            assert!(e.previous_state.is_none());
            assert!(e.transition_history.is_empty());
            assert_eq!(e.restart_count, 0);
        }
    }

    #[test]
    fn test_service_count() {
        let mgr = LifecycleManager::new();
        assert_eq!(mgr.service_count(), 0);
        mgr.register("a", "A", ServiceTier::Tier1, RestartConfig::default())
            .ok();
        assert_eq!(mgr.service_count(), 1);
        mgr.register("b", "B", ServiceTier::Tier2, RestartConfig::default())
            .ok();
        assert_eq!(mgr.service_count(), 2);
    }

    #[test]
    fn test_lifecycle_entry_builder_defaults() {
        let entry = LifecycleEntryBuilder::new("svc", "Service", ServiceTier::Tier5).build();
        assert_eq!(entry.service_id, "svc");
        assert_eq!(entry.name, "Service");
        assert_eq!(entry.tier, ServiceTier::Tier5);
        assert_eq!(entry.current_state, ServiceStatus::Stopped);
        assert_eq!(entry.config.max_restarts, 5);
        assert_eq!(entry.config.initial_backoff, Duration::from_secs(1));
        assert_eq!(entry.config.max_backoff, Duration::from_secs(30));
        assert_eq!(entry.current_backoff, Duration::from_secs(1));
    }

    #[test]
    fn test_lifecycle_entry_builder_custom() {
        let entry = LifecycleEntryBuilder::new("svc", "Custom", ServiceTier::Tier1)
            .max_restarts(10)
            .initial_backoff(Duration::from_millis(500))
            .max_backoff(Duration::from_secs(120))
            .build();
        assert_eq!(entry.config.max_restarts, 10);
        assert_eq!(entry.config.initial_backoff, Duration::from_millis(500));
        assert_eq!(entry.config.max_backoff, Duration::from_secs(120));
        assert_eq!(entry.current_backoff, Duration::from_millis(500));
    }

    // ================================================================
    // [INVARIANT] — FSM transitions
    // ================================================================

    #[test]
    fn test_valid_transition_stopped_to_starting() {
        assert!(is_valid_transition(
            ServiceStatus::Stopped,
            ServiceStatus::Starting
        ));
    }

    #[test]
    fn test_valid_transition_starting_to_running() {
        assert!(is_valid_transition(
            ServiceStatus::Starting,
            ServiceStatus::Running
        ));
    }

    #[test]
    fn test_valid_transition_starting_to_failed() {
        assert!(is_valid_transition(
            ServiceStatus::Starting,
            ServiceStatus::Failed
        ));
    }

    #[test]
    fn test_valid_transition_running_to_stopping() {
        assert!(is_valid_transition(
            ServiceStatus::Running,
            ServiceStatus::Stopping
        ));
    }

    #[test]
    fn test_valid_transition_running_to_failed() {
        assert!(is_valid_transition(
            ServiceStatus::Running,
            ServiceStatus::Failed
        ));
    }

    #[test]
    fn test_valid_transition_stopping_to_stopped() {
        assert!(is_valid_transition(
            ServiceStatus::Stopping,
            ServiceStatus::Stopped
        ));
    }

    #[test]
    fn test_valid_transition_failed_to_starting() {
        assert!(is_valid_transition(
            ServiceStatus::Failed,
            ServiceStatus::Starting
        ));
    }

    #[test]
    fn test_invalid_transition_stopped_to_running() {
        assert!(!is_valid_transition(
            ServiceStatus::Stopped,
            ServiceStatus::Running
        ));
    }

    #[test]
    fn test_invalid_transition_stopped_to_stopping() {
        assert!(!is_valid_transition(
            ServiceStatus::Stopped,
            ServiceStatus::Stopping
        ));
    }

    #[test]
    fn test_invalid_transition_stopped_to_failed() {
        assert!(!is_valid_transition(
            ServiceStatus::Stopped,
            ServiceStatus::Failed
        ));
    }

    #[test]
    fn test_invalid_transition_running_to_starting() {
        assert!(!is_valid_transition(
            ServiceStatus::Running,
            ServiceStatus::Starting
        ));
    }

    #[test]
    fn test_invalid_transition_starting_to_stopped() {
        assert!(!is_valid_transition(
            ServiceStatus::Starting,
            ServiceStatus::Stopped
        ));
    }

    #[test]
    fn test_invalid_transition_stopping_to_starting() {
        assert!(!is_valid_transition(
            ServiceStatus::Stopping,
            ServiceStatus::Starting
        ));
    }

    // ================================================================
    // [BOUNDARY] — edge cases
    // ================================================================

    #[test]
    fn test_empty_manager_queries() {
        let mgr = LifecycleManager::new();
        assert_eq!(mgr.service_count(), 0);
        assert!(mgr.get_all_running().is_empty());
        assert!(mgr.get_all_failed().is_empty());
        assert!(!mgr.is_registered("any"));
    }

    #[test]
    fn test_restart_at_exact_max() {
        let config = RestartConfig::new(2, Duration::from_millis(100), Duration::from_secs(10));
        let mgr = LifecycleManager::new();
        mgr.register("svc", "Svc", ServiceTier::Tier5, config).ok();

        // First restart (count 0 -> 1)
        mgr.start_service("svc").ok();
        mgr.mark_running("svc").ok();
        let r1 = mgr.restart_service("svc");
        assert!(r1.is_ok());

        // Second restart (count 1 -> 2)
        mgr.mark_running("svc").ok();
        let r2 = mgr.restart_service("svc");
        assert!(r2.is_ok());

        // Third restart should fail (count == max_restarts == 2)
        mgr.mark_running("svc").ok();
        let r3 = mgr.restart_service("svc");
        assert!(r3.is_err());
    }

    #[test]
    fn test_restart_over_max_fails() {
        let config = RestartConfig::new(1, Duration::from_millis(100), Duration::from_secs(10));
        let mgr = LifecycleManager::new();
        mgr.register("svc", "Svc", ServiceTier::Tier5, config).ok();

        mgr.start_service("svc").ok();
        mgr.mark_running("svc").ok();
        mgr.restart_service("svc").ok();

        // restart_count is now 1 == max_restarts
        let result = mgr.can_restart("svc");
        assert!(result.is_ok());
        assert_eq!(result.ok(), Some(false));
    }

    #[test]
    fn test_backoff_caps_at_max() {
        let config = RestartConfig::new(10, Duration::from_secs(1), Duration::from_secs(5));
        let mgr = LifecycleManager::new();
        mgr.register("svc", "Svc", ServiceTier::Tier5, config).ok();

        // Restart several times: 1s -> 2s -> 4s -> 5s (capped) -> 5s
        for _ in 0..5 {
            mgr.start_service("svc").ok();
            mgr.mark_running("svc").ok();
            mgr.restart_service("svc").ok();
        }

        let backoff = mgr.get_restart_backoff("svc");
        assert!(backoff.is_ok());
        if let Ok(b) = backoff {
            assert!(b <= Duration::from_secs(5));
        }
    }

    #[test]
    fn test_zero_max_restarts() {
        let config = RestartConfig::new(0, Duration::from_secs(1), Duration::from_secs(30));
        let mgr = LifecycleManager::new();
        mgr.register("svc", "Svc", ServiceTier::Tier5, config).ok();

        mgr.start_service("svc").ok();
        mgr.mark_running("svc").ok();

        // Cannot restart at all
        let result = mgr.restart_service("svc");
        assert!(result.is_err());
        assert_eq!(mgr.can_restart("svc").ok(), Some(false));
    }

    #[test]
    fn test_single_service_full_cycle() {
        let mgr = setup_manager();

        // Stopped -> Starting -> Running -> Stopping -> Stopped
        mgr.start_service("svc").ok();
        assert_eq!(mgr.get_status("svc").ok(), Some(ServiceStatus::Starting));

        mgr.mark_running("svc").ok();
        assert_eq!(mgr.get_status("svc").ok(), Some(ServiceStatus::Running));

        mgr.stop_service("svc").ok();
        assert_eq!(mgr.get_status("svc").ok(), Some(ServiceStatus::Stopping));

        mgr.mark_stopped("svc").ok();
        assert_eq!(mgr.get_status("svc").ok(), Some(ServiceStatus::Stopped));

        let history = mgr.get_history("svc");
        assert!(history.is_ok());
        if let Ok(h) = history {
            assert_eq!(h.len(), 4);
        }
    }

    #[test]
    fn test_many_services() {
        let mgr = LifecycleManager::new();
        for i in 0..20 {
            let id = format!("svc-{i}");
            mgr.register(&id, &id, ServiceTier::Tier3, RestartConfig::default())
                .ok();
        }
        assert_eq!(mgr.service_count(), 20);
    }

    #[test]
    fn test_history_ordering() {
        let mgr = setup_manager();
        mgr.start_service("svc").ok();
        mgr.mark_running("svc").ok();
        mgr.stop_service("svc").ok();
        mgr.mark_stopped("svc").ok();

        let history = mgr.get_history("svc");
        assert!(history.is_ok());
        if let Ok(h) = history {
            assert_eq!(h.len(), 4);
            assert_eq!(h[0].from, ServiceStatus::Stopped);
            assert_eq!(h[0].to, ServiceStatus::Starting);
            assert_eq!(h[1].from, ServiceStatus::Starting);
            assert_eq!(h[1].to, ServiceStatus::Running);
            assert_eq!(h[2].from, ServiceStatus::Running);
            assert_eq!(h[2].to, ServiceStatus::Stopping);
            assert_eq!(h[3].from, ServiceStatus::Stopping);
            assert_eq!(h[3].to, ServiceStatus::Stopped);
        }
    }

    // ================================================================
    // [PROPERTY] — health score, tensor, backoff properties
    // ================================================================

    #[test]
    fn test_tensor_dims_in_unit_interval() {
        let mgr = LifecycleManager::new();
        for i in 0..5 {
            let id = format!("svc-{i}");
            mgr.register(&id, &id, ServiceTier::Tier3, RestartConfig::default())
                .ok();
            if i < 3 {
                mgr.start_service(&id).ok();
                mgr.mark_running(&id).ok();
            }
        }

        let tensor = mgr.contribute();
        let arr = tensor.tensor.to_array();
        for (idx, val) in arr.iter().enumerate() {
            assert!(
                (0.0..=1.0).contains(val),
                "Dimension {idx} out of range: {val}"
            );
        }
    }

    #[test]
    fn test_tensor_coverage_matches_populated_dims() {
        let mgr = setup_running();
        let tensor = mgr.contribute();
        assert!(tensor.coverage.is_covered(DimensionIndex::HealthScore));
        assert!(tensor.coverage.is_covered(DimensionIndex::Uptime));
        assert_eq!(tensor.coverage.count(), 2);
    }

    #[test]
    fn test_running_ratio_in_tensor() {
        let mgr = LifecycleManager::new();
        for i in 0..4 {
            let id = format!("svc-{i}");
            mgr.register(&id, &id, ServiceTier::Tier3, RestartConfig::default())
                .ok();
        }
        // Start 2 of 4
        for i in 0..2 {
            let id = format!("svc-{i}");
            mgr.start_service(&id).ok();
            mgr.mark_running(&id).ok();
        }

        let tensor = mgr.contribute();
        let d6 = tensor.tensor.to_array()[DimensionIndex::HealthScore as usize];
        assert!((d6 - 0.5).abs() < f64::EPSILON, "Expected 0.5, got {d6}");
    }

    #[test]
    fn test_backoff_doubles_each_restart() {
        let config = RestartConfig::new(10, Duration::from_secs(1), Duration::from_secs(120));
        let mgr = LifecycleManager::new();
        mgr.register("svc", "Svc", ServiceTier::Tier5, config).ok();

        let b0 = mgr.get_restart_backoff("svc").ok();
        assert_eq!(b0, Some(Duration::from_secs(1)));

        mgr.start_service("svc").ok();
        mgr.mark_running("svc").ok();
        let returned_backoff = mgr.restart_service("svc");
        assert!(returned_backoff.is_ok());
        // Returned backoff was 1s (pre-doubling)
        assert_eq!(returned_backoff.ok(), Some(Duration::from_secs(1)));

        // After restart: backoff doubled to 2s
        let b1 = mgr.get_restart_backoff("svc").ok();
        assert_eq!(b1, Some(Duration::from_secs(2)));

        // Second restart
        mgr.mark_running("svc").ok();
        let returned_2 = mgr.restart_service("svc");
        assert_eq!(returned_2.ok(), Some(Duration::from_secs(2)));
        let b2 = mgr.get_restart_backoff("svc").ok();
        assert_eq!(b2, Some(Duration::from_secs(4)));
    }

    #[test]
    fn test_uptime_proxy_decreases_with_restarts() {
        let config = RestartConfig::new(10, Duration::from_millis(100), Duration::from_secs(30));
        let mgr = LifecycleManager::new();
        mgr.register("svc", "Svc", ServiceTier::Tier3, config).ok();

        let t0 = mgr.contribute();
        let d7_before = t0.tensor.to_array()[DimensionIndex::Uptime as usize];

        // Restart a few times
        for _ in 0..3 {
            mgr.start_service("svc").ok();
            mgr.mark_running("svc").ok();
            mgr.restart_service("svc").ok();
        }

        let t1 = mgr.contribute();
        let d7_after = t1.tensor.to_array()[DimensionIndex::Uptime as usize];

        assert!(
            d7_after < d7_before,
            "Uptime proxy should decrease: before={d7_before}, after={d7_after}"
        );
    }

    #[test]
    fn test_status_health_score_values() {
        assert!((status_health_score(ServiceStatus::Running) - 1.0).abs() < f64::EPSILON);
        assert!((status_health_score(ServiceStatus::Starting) - 0.5).abs() < f64::EPSILON);
        assert!((status_health_score(ServiceStatus::Stopping) - 0.5).abs() < f64::EPSILON);
        assert!((status_health_score(ServiceStatus::Stopped) - 0.0).abs() < f64::EPSILON);
        assert!((status_health_score(ServiceStatus::Failed) - 0.0).abs() < f64::EPSILON);
    }

    // ================================================================
    // [NEGATIVE] — error paths
    // ================================================================

    #[test]
    fn test_start_unknown_service() {
        let mgr = LifecycleManager::new();
        assert!(mgr.start_service("ghost").is_err());
    }

    #[test]
    fn test_stop_unknown_service() {
        let mgr = LifecycleManager::new();
        assert!(mgr.stop_service("ghost").is_err());
    }

    #[test]
    fn test_mark_running_unknown() {
        let mgr = LifecycleManager::new();
        assert!(mgr.mark_running("ghost").is_err());
    }

    #[test]
    fn test_mark_failed_unknown() {
        let mgr = LifecycleManager::new();
        assert!(mgr.mark_failed("ghost").is_err());
    }

    #[test]
    fn test_deregister_unknown() {
        let mgr = LifecycleManager::new();
        assert!(mgr.deregister("ghost").is_err());
    }

    #[test]
    fn test_get_status_unknown() {
        let mgr = LifecycleManager::new();
        assert!(mgr.get_status("ghost").is_err());
    }

    #[test]
    fn test_get_history_unknown() {
        let mgr = LifecycleManager::new();
        assert!(mgr.get_history("ghost").is_err());
    }

    #[test]
    fn test_can_restart_unknown() {
        let mgr = LifecycleManager::new();
        assert!(mgr.can_restart("ghost").is_err());
    }

    #[test]
    fn test_duplicate_register_fails() {
        let mgr = setup_manager();
        let result =
            mgr.register("svc", "Duplicate", ServiceTier::Tier1, RestartConfig::default());
        assert!(result.is_err());
        assert_eq!(mgr.service_count(), 1);
    }

    #[test]
    fn test_start_already_running() {
        let mgr = setup_running();
        // Running -> Starting is not a valid transition
        let result = mgr.start_service("svc");
        assert!(result.is_err());
    }

    #[test]
    fn test_stop_when_stopped() {
        let mgr = setup_manager();
        // Stopped -> Stopping is not valid
        let result = mgr.stop_service("svc");
        assert!(result.is_err());
    }

    #[test]
    fn test_mark_stopped_when_running() {
        let mgr = setup_running();
        // Running -> Stopped is not valid (must go through Stopping)
        let result = mgr.mark_stopped("svc");
        assert!(result.is_err());
    }

    // ================================================================
    // [INTEGRATION] — cross-module, compound operations
    // ================================================================

    #[test]
    fn test_full_lifecycle_happy_path() {
        let mgr = LifecycleManager::new();
        mgr.register("synthex", "SYNTHEX Engine", ServiceTier::Tier1, RestartConfig::default())
            .ok();

        // Full cycle: start -> run -> stop -> stopped
        mgr.start_service("synthex").ok();
        mgr.mark_running("synthex").ok();
        mgr.stop_service("synthex").ok();
        mgr.mark_stopped("synthex").ok();

        assert_eq!(
            mgr.get_status("synthex").ok(),
            Some(ServiceStatus::Stopped)
        );

        let entry = mgr.get_entry("synthex");
        assert!(entry.is_ok());
        if let Ok(e) = entry {
            assert_eq!(e.transition_history.len(), 4);
            assert_eq!(e.restart_count, 0);
        }
    }

    #[test]
    fn test_restart_cycle_with_backoff() {
        let config = RestartConfig::new(5, Duration::from_secs(1), Duration::from_secs(30));
        let mgr = LifecycleManager::new();
        mgr.register("svc", "Test", ServiceTier::Tier4, config).ok();

        mgr.start_service("svc").ok();
        mgr.mark_running("svc").ok();

        // First restart: returns initial backoff (1s)
        let b1 = mgr.restart_service("svc");
        assert!(b1.is_ok());
        assert_eq!(b1.ok(), Some(Duration::from_secs(1)));
        assert_eq!(mgr.get_status("svc").ok(), Some(ServiceStatus::Starting));

        // After restart, 3 transitions recorded (stop, stopped, start)
        let entry = mgr.get_entry("svc");
        assert!(entry.is_ok());
        if let Ok(e) = entry {
            // 2 (start+running) + 3 (restart) = 5
            assert_eq!(e.transition_history.len(), 5);
            assert_eq!(e.restart_count, 1);
        }
    }

    #[test]
    fn test_restart_from_failed() {
        let mgr = setup_manager();
        mgr.start_service("svc").ok();
        mgr.mark_failed("svc").ok();

        // Restart from Failed state (single transition: Failed -> Starting)
        let result = mgr.restart_service("svc");
        assert!(result.is_ok());
        assert_eq!(mgr.get_status("svc").ok(), Some(ServiceStatus::Starting));

        let entry = mgr.get_entry("svc");
        assert!(entry.is_ok());
        if let Ok(e) = entry {
            assert_eq!(e.restart_count, 1);
        }
    }

    #[test]
    fn test_multiple_restarts_increment() {
        let config = RestartConfig::new(10, Duration::from_millis(100), Duration::from_secs(30));
        let mgr = LifecycleManager::new();
        mgr.register("svc", "Svc", ServiceTier::Tier5, config).ok();

        for i in 0u32..5 {
            mgr.start_service("svc").ok();
            mgr.mark_running("svc").ok();
            mgr.restart_service("svc").ok();

            let entry = mgr.get_entry("svc");
            assert!(entry.is_ok());
            if let Ok(e) = entry {
                assert_eq!(e.restart_count, i + 1);
            }
        }
    }

    #[test]
    fn test_reset_restart_count() {
        let mgr = setup_running();
        mgr.restart_service("svc").ok();

        let entry = mgr.get_entry("svc");
        assert!(entry.is_ok());
        if let Ok(e) = entry {
            assert_eq!(e.restart_count, 1);
            assert!(e.current_backoff > Duration::from_secs(1));
        }

        // Reset
        let result = mgr.reset_restart_count("svc");
        assert!(result.is_ok());

        let entry = mgr.get_entry("svc");
        assert!(entry.is_ok());
        if let Ok(e) = entry {
            assert_eq!(e.restart_count, 0);
            assert_eq!(e.current_backoff, e.config.initial_backoff);
        }
    }

    #[test]
    fn test_reset_restart_count_unknown() {
        let mgr = LifecycleManager::new();
        assert!(mgr.reset_restart_count("ghost").is_err());
    }

    #[test]
    fn test_signal_emission_on_failure() {
        let bus = Arc::new(SignalBus::new());
        let mgr = LifecycleManager::with_signal_bus(bus.clone());
        mgr.register("svc", "Test", ServiceTier::Tier3, RestartConfig::default())
            .ok();

        mgr.start_service("svc").ok();
        mgr.mark_running("svc").ok();

        // Running -> Failed should emit signal (health 1.0 -> 0.0)
        mgr.mark_failed("svc").ok();

        let stats = bus.stats();
        assert!(
            stats.health_emitted > 0,
            "Expected health signal emission on failure"
        );
    }

    #[test]
    fn test_signal_emission_on_recovery() {
        let bus = Arc::new(SignalBus::new());
        let mgr = LifecycleManager::with_signal_bus(bus.clone());
        mgr.register("svc", "Test", ServiceTier::Tier3, RestartConfig::default())
            .ok();

        // Stopped -> Starting emits signal (0.0 -> 0.5)
        mgr.start_service("svc").ok();
        // Starting -> Running emits signal (0.5 -> 1.0)
        mgr.mark_running("svc").ok();

        let stats = bus.stats();
        assert!(
            stats.health_emitted >= 2,
            "Expected at least 2 health signals, got {}",
            stats.health_emitted
        );
    }

    #[test]
    fn test_signal_bus_none_no_panic() {
        let mgr = LifecycleManager::new(); // no signal bus
        mgr.register("svc", "Test", ServiceTier::Tier3, RestartConfig::default())
            .ok();
        mgr.start_service("svc").ok();
        mgr.mark_running("svc").ok();
        mgr.mark_failed("svc").ok(); // should not panic without bus
    }

    #[test]
    fn test_get_all_running_multiple() {
        let mgr = LifecycleManager::new();
        mgr.register("a", "A", ServiceTier::Tier1, RestartConfig::default())
            .ok();
        mgr.register("b", "B", ServiceTier::Tier2, RestartConfig::default())
            .ok();
        mgr.register("c", "C", ServiceTier::Tier3, RestartConfig::default())
            .ok();

        // Start a and b, leave c stopped
        for id in &["a", "b"] {
            mgr.start_service(id).ok();
            mgr.mark_running(id).ok();
        }

        let running = mgr.get_all_running();
        assert_eq!(running.len(), 2);
        assert!(running.contains(&"a".to_owned()));
        assert!(running.contains(&"b".to_owned()));
        assert!(!running.contains(&"c".to_owned()));
    }

    #[test]
    fn test_get_all_failed_multiple() {
        let mgr = LifecycleManager::new();
        mgr.register("ok", "OK", ServiceTier::Tier1, RestartConfig::default())
            .ok();
        mgr.register("bad", "BAD", ServiceTier::Tier2, RestartConfig::default())
            .ok();

        // "ok" runs normally
        mgr.start_service("ok").ok();
        mgr.mark_running("ok").ok();

        // "bad" fails during startup
        mgr.start_service("bad").ok();
        mgr.mark_failed("bad").ok();

        let failed = mgr.get_all_failed();
        assert_eq!(failed.len(), 1);
        assert!(failed.contains(&"bad".to_owned()));
    }

    #[test]
    fn test_lifecycle_action_display() {
        let start = LifecycleAction::Start {
            service_id: "synthex".to_owned(),
        };
        assert_eq!(start.to_string(), "Start(synthex)");

        let stop = LifecycleAction::Stop {
            service_id: "nais".to_owned(),
            graceful: true,
        };
        assert_eq!(stop.to_string(), "Stop(nais, graceful=true)");

        let restart = LifecycleAction::Restart {
            service_id: "svc".to_owned(),
            reason: "config change".to_owned(),
        };
        assert!(restart.to_string().contains("Restart"));

        let health = LifecycleAction::HealthCheck {
            service_id: "svc".to_owned(),
        };
        assert!(health.to_string().contains("HealthCheck"));
    }

    #[test]
    fn test_lifecycle_transition_display() {
        let t = LifecycleTransition {
            from: ServiceStatus::Stopped,
            to: ServiceStatus::Starting,
            reason: "boot".to_owned(),
            timestamp: Timestamp::now(),
        };
        let s = t.to_string();
        assert!(s.contains("stopped"));
        assert!(s.contains("starting"));
        assert!(s.contains("boot"));
    }

    #[test]
    fn test_lifecycle_entry_display() {
        let entry = LifecycleEntryBuilder::new("synthex", "SYNTHEX", ServiceTier::Tier1).build();
        let s = entry.to_string();
        assert!(s.contains("synthex"));
        assert!(s.contains("Tier1"));
        assert!(s.contains("stopped"));
    }

    #[test]
    fn test_default_lifecycle_manager() {
        let mgr = LifecycleManager::default();
        assert_eq!(mgr.service_count(), 0);
    }

    #[test]
    fn test_restart_from_stopped_fails() {
        let mgr = setup_manager();
        // Stopped is not a valid state for restart
        let result = mgr.restart_service("svc");
        assert!(result.is_err());
    }

    #[test]
    fn test_restart_from_starting_fails() {
        let mgr = setup_manager();
        mgr.start_service("svc").ok();
        // Starting is not a valid state for restart
        let result = mgr.restart_service("svc");
        assert!(result.is_err());
    }

    #[test]
    fn test_tensor_empty_manager() {
        let mgr = LifecycleManager::new();
        let tensor = mgr.contribute();
        let arr = tensor.tensor.to_array();
        // With no services, all dims should be 0.0
        for val in &arr {
            assert!((*val - 0.0).abs() < f64::EPSILON);
        }
    }
}
