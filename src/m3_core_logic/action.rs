//! # M16: Action Executor
//!
//! Dispatches, executes, and tracks remediation actions across the
//! ULTRAPLATE service fleet. Integrates with the escalation tier system
//! to enforce approval gates before execution and supports checkpoint-based
//! rollback for reversible operations.
//!
//! ## Layer: L3 (Core Logic)
//! ## Dependencies: M01 (Error), M14 (Remediation Engine), M15 (Confidence Calculator)
//! ## Tests: 14+
//!
//! ## 12D Tensor Encoding
//! ```text
//! [16/36, 0.0, 3/6, deps, agents, protocol, health, uptime, synergy, latency, error_rate, temporal]
//! ```
//!
//! ## Pipeline Integration
//!
//! | Pipeline | Role |
//! |----------|------|
//! | PL-REMEDIATE-001 | Primary remediation execution |
//! | PL-HEALTH-001 | Post-action health verification |
//!
//! ## Escalation Tier Enforcement
//!
//! | Tier | Dispatch Behaviour |
//! |------|-------------------|
//! | L0 `AutoExecute` | Dispatched with `Approved` status, ready to execute |
//! | L1 `NotifyHuman` | Dispatched with `Approved` status, human notified |
//! | L2 `RequireApproval` | Dispatched with `Pending` status, requires `approve()` |
//! | L3 `PbftConsensus` | Dispatched with `Pending` status, requires `approve()` |
//!
//! ## Related Documentation
//! - [Layer Specification](../../ai_docs/layers/L03_CORE_LOGIC.md)
//! - [Escalation Spec](../../ai_specs/ESCALATION_SPEC.md)

use std::collections::HashMap;
use std::sync::RwLock;

use chrono::{DateTime, Utc};

use super::{RemediationAction, RemediationRequest};
use crate::{EscalationTier, Error, Result};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Default maximum number of concurrently active action executions.
const DEFAULT_MAX_CONCURRENT: usize = 10;

/// Maximum number of completed executions retained in history.
const COMPLETED_CAP: usize = 500;

// ---------------------------------------------------------------------------
// ActionStatus
// ---------------------------------------------------------------------------

/// Status of an action execution through its lifecycle.
///
/// State transitions:
/// ```text
/// Pending --> Approved --> Executing --> Completed
///         |            |            \-> Failed
///         |            \-> RollingBack --> Completed | Failed
///         \-> Rejected
///         \-> TimedOut
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ActionStatus {
    /// Waiting for approval (L2/L3 tier actions).
    Pending,
    /// Approved and ready for execution.
    Approved,
    /// Currently executing the remediation action.
    Executing,
    /// Rolling back a previously executed action via checkpoint.
    RollingBack,
    /// Execution completed (check [`ActionResult::success`] for outcome).
    Completed,
    /// Execution failed with an error.
    Failed,
    /// Rejected by a human operator or consensus vote.
    Rejected,
    /// Execution exceeded its time budget.
    TimedOut,
}

// ---------------------------------------------------------------------------
// ActionResult
// ---------------------------------------------------------------------------

/// Outcome produced by a completed (or failed) action execution.
#[derive(Clone, Debug)]
pub struct ActionResult {
    /// Whether the action achieved its intended effect.
    pub success: bool,
    /// Human-readable description of the outcome.
    pub message: String,
    /// Metric keys that were affected by this action.
    pub metrics_affected: Vec<String>,
    /// Side effects observed during execution.
    pub side_effects: Vec<String>,
}

impl ActionResult {
    /// Create a successful result with the given message.
    #[must_use]
    pub fn success(message: impl Into<String>) -> Self {
        Self {
            success: true,
            message: message.into(),
            metrics_affected: Vec::new(),
            side_effects: Vec::new(),
        }
    }

    /// Create a failure result with the given message.
    #[must_use]
    pub fn failure(message: impl Into<String>) -> Self {
        Self {
            success: false,
            message: message.into(),
            metrics_affected: Vec::new(),
            side_effects: Vec::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// ActionExecution
// ---------------------------------------------------------------------------

/// A single execution of a [`RemediationAction`] against a service.
///
/// Tracks the full lifecycle from dispatch through completion, including
/// timing information, result details, and optional rollback linkage.
#[derive(Clone, Debug)]
pub struct ActionExecution {
    /// Unique identifier for this execution (UUID v4).
    pub execution_id: String,
    /// The originating [`RemediationRequest`] ID.
    pub request_id: String,
    /// Target service identifier.
    pub service_id: String,
    /// The remediation action being executed.
    pub action: RemediationAction,
    /// Escalation tier governing this execution.
    pub tier: EscalationTier,
    /// Current status in the execution lifecycle.
    pub status: ActionStatus,
    /// Timestamp when the execution was created.
    pub started_at: DateTime<Utc>,
    /// Timestamp when the execution reached a terminal state.
    pub completed_at: Option<DateTime<Utc>>,
    /// Wall-clock duration in milliseconds (set on completion).
    pub duration_ms: Option<u64>,
    /// Outcome of the execution (set on completion or failure).
    pub result: Option<ActionResult>,
    /// Linked rollback checkpoint ID, if a checkpoint was saved.
    pub rollback_id: Option<String>,
}

// ---------------------------------------------------------------------------
// RollbackInfo
// ---------------------------------------------------------------------------

/// Checkpoint information enabling rollback of a completed action.
#[derive(Clone, Debug)]
pub struct RollbackInfo {
    /// Unique rollback identifier (UUID v4).
    pub rollback_id: String,
    /// The execution this rollback is associated with.
    pub execution_id: String,
    /// Serialised description of the pre-execution state.
    pub checkpoint: String,
    /// Timestamp when the checkpoint was created.
    pub created_at: DateTime<Utc>,
    /// Whether the rollback has been executed.
    pub executed: bool,
}

// ---------------------------------------------------------------------------
// ActionExecutor
// ---------------------------------------------------------------------------

/// Orchestrates the dispatch, approval, execution, and rollback of
/// remediation actions across the ULTRAPLATE service fleet.
///
/// Thread-safe via interior [`RwLock`] guards on all mutable state.
///
/// # Construction
///
/// ```rust
/// use maintenance_engine::m3_core_logic::action::ActionExecutor;
///
/// let executor = ActionExecutor::new();
/// assert!(executor.can_accept_more());
/// ```
pub struct ActionExecutor {
    /// Currently active (non-terminal) executions, keyed by execution ID.
    active_actions: RwLock<HashMap<String, ActionExecution>>,
    /// Completed executions retained for historical analysis (capped at [`COMPLETED_CAP`]).
    completed_actions: RwLock<Vec<ActionExecution>>,
    /// Maximum number of concurrently active executions.
    max_concurrent: usize,
    /// Rollback checkpoints keyed by rollback ID.
    rollback_registry: RwLock<HashMap<String, RollbackInfo>>,
}

impl ActionExecutor {
    /// Create a new `ActionExecutor` with default settings.
    ///
    /// Uses [`DEFAULT_MAX_CONCURRENT`] (10) as the concurrency limit.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use maintenance_engine::m3_core_logic::action::ActionExecutor;
    ///
    /// let executor = ActionExecutor::new();
    /// assert_eq!(executor.get_active_count(), 0);
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self {
            active_actions: RwLock::new(HashMap::new()),
            completed_actions: RwLock::new(Vec::new()),
            max_concurrent: DEFAULT_MAX_CONCURRENT,
            rollback_registry: RwLock::new(HashMap::new()),
        }
    }

    /// Create a new `ActionExecutor` with a custom concurrency limit.
    ///
    /// A `max_concurrent` of zero is treated as 1 to prevent deadlock.
    #[must_use]
    pub fn with_max_concurrent(max_concurrent: usize) -> Self {
        Self {
            active_actions: RwLock::new(HashMap::new()),
            completed_actions: RwLock::new(Vec::new()),
            max_concurrent: if max_concurrent == 0 { 1 } else { max_concurrent },
            rollback_registry: RwLock::new(HashMap::new()),
        }
    }

    // -----------------------------------------------------------------------
    // Dispatch & Approval
    // -----------------------------------------------------------------------

    /// Dispatch a new action execution from a [`RemediationRequest`].
    ///
    /// - L0 and L1 tier requests are dispatched with [`ActionStatus::Approved`].
    /// - L2 and L3 tier requests are dispatched with [`ActionStatus::Pending`]
    ///   and must be explicitly approved via [`approve`](Self::approve).
    ///
    /// # Errors
    ///
    /// Returns [`Error::Pipeline`] if the executor has reached its concurrency
    /// limit ([`can_accept_more`](Self::can_accept_more) returns `false`).
    pub fn dispatch(&self, request: &RemediationRequest) -> Result<ActionExecution> {
        if !self.can_accept_more() {
            return Err(Error::Pipeline(format!(
                "Action executor at capacity ({}/{})",
                self.get_active_count(),
                self.max_concurrent,
            )));
        }

        let initial_status = match request.tier {
            EscalationTier::L0AutoExecute | EscalationTier::L1NotifyHuman => ActionStatus::Approved,
            EscalationTier::L2RequireApproval | EscalationTier::L3PbftConsensus => {
                ActionStatus::Pending
            }
        };

        let execution = ActionExecution {
            execution_id: generate_uuid(),
            request_id: request.id.clone(),
            service_id: request.service_id.clone(),
            action: request.suggested_action.clone(),
            tier: request.tier,
            status: initial_status,
            started_at: Utc::now(),
            completed_at: None,
            duration_ms: None,
            result: None,
            rollback_id: None,
        };

        {
            let mut active = self
                .active_actions
                .write()
                .map_err(|e| Error::Other(format!("RwLock poisoned: {e}")))?;
            active.insert(execution.execution_id.clone(), execution.clone());
        }

        Ok(execution)
    }

    /// Approve a pending execution, advancing it to [`ActionStatus::Approved`].
    ///
    /// Only executions in [`ActionStatus::Pending`] can be approved.
    ///
    /// # Errors
    ///
    /// - [`Error::Validation`] if the execution is not in `Pending` status.
    /// - [`Error::Pipeline`] if the execution ID is not found.
    pub fn approve(&self, execution_id: &str) -> Result<ActionExecution> {
        let mut active = self
            .active_actions
            .write()
            .map_err(|e| Error::Other(format!("RwLock poisoned: {e}")))?;

        let execution = active
            .get_mut(execution_id)
            .ok_or_else(|| Error::Pipeline(format!("Execution not found: {execution_id}")))?;

        if execution.status != ActionStatus::Pending {
            return Err(Error::Validation(format!(
                "Cannot approve execution in {:?} status",
                execution.status,
            )));
        }

        execution.status = ActionStatus::Approved;
        let snapshot = execution.clone();
        drop(active);
        Ok(snapshot)
    }

    /// Reject a pending execution with the given reason.
    ///
    /// The execution is moved to [`ActionStatus::Rejected`] and archived
    /// into the completed-actions history.
    ///
    /// # Errors
    ///
    /// - [`Error::Validation`] if the execution is not in `Pending` status.
    /// - [`Error::Pipeline`] if the execution ID is not found.
    pub fn reject(&self, execution_id: &str, reason: &str) -> Result<ActionExecution> {
        let execution = {
            let mut active = self
                .active_actions
                .write()
                .map_err(|e| Error::Other(format!("RwLock poisoned: {e}")))?;

            let mut execution = active
                .remove(execution_id)
                .ok_or_else(|| {
                    Error::Pipeline(format!("Execution not found: {execution_id}"))
                })?;

            if execution.status != ActionStatus::Pending {
                active.insert(execution.execution_id.clone(), execution);
                return Err(Error::Validation(
                    "Cannot reject execution that is not in Pending status".into(),
                ));
            }
            drop(active);

            execution.status = ActionStatus::Rejected;
            execution.completed_at = Some(Utc::now());
            execution.duration_ms = compute_duration_ms(
                execution.started_at,
                execution.completed_at,
            );
            execution.result = Some(ActionResult::failure(format!("Rejected: {reason}")));
            execution
        };

        self.archive_completed(execution.clone())?;
        Ok(execution)
    }

    // -----------------------------------------------------------------------
    // Execution
    // -----------------------------------------------------------------------

    /// Execute an approved action, transitioning it through `Executing`
    /// to either `Completed` or `Failed`.
    ///
    /// At this layer (L3 Core Logic) execution is simulated: the method
    /// produces a deterministic [`ActionResult`] based on the
    /// [`RemediationAction`] variant. Real execution is deferred to
    /// L4 Integration bridges.
    ///
    /// # Errors
    ///
    /// - [`Error::Pipeline`] if the execution ID is not found.
    /// - [`Error::Validation`] if the execution is not in `Approved` status.
    #[allow(clippy::significant_drop_tightening)]
    pub fn execute(&self, execution_id: &str) -> Result<ActionExecution> {
        {
            let mut active = self
                .active_actions
                .write()
                .map_err(|e| Error::Other(format!("RwLock poisoned: {e}")))?;

            let execution = active
                .get_mut(execution_id)
                .ok_or_else(|| {
                    Error::Pipeline(format!("Execution not found: {execution_id}"))
                })?;

            if execution.status != ActionStatus::Approved {
                return Err(Error::Validation(format!(
                    "Cannot execute action in {:?} status; must be Approved",
                    execution.status,
                )));
            }

            execution.status = ActionStatus::Executing;
        }

        let result = self.simulate_action(execution_id)?;

        if result.success {
            self.complete(execution_id, true, &result.message)
        } else {
            self.fail(execution_id, &result.message)
        }
    }

    /// Mark an executing action as successfully completed.
    ///
    /// # Errors
    ///
    /// - [`Error::Pipeline`] if the execution ID is not found.
    /// - [`Error::Validation`] if the execution is not in `Executing`
    ///   or `RollingBack` status.
    #[allow(clippy::significant_drop_tightening)]
    pub fn complete(
        &self,
        execution_id: &str,
        success: bool,
        message: &str,
    ) -> Result<ActionExecution> {
        let execution = {
            let mut active = self
                .active_actions
                .write()
                .map_err(|e| Error::Other(format!("RwLock poisoned: {e}")))?;

            let mut execution = active.remove(execution_id).ok_or_else(|| {
                Error::Pipeline(format!("Execution not found: {execution_id}"))
            })?;

            if execution.status != ActionStatus::Executing
                && execution.status != ActionStatus::RollingBack
            {
                active.insert(execution.execution_id.clone(), execution);
                return Err(Error::Validation(
                    "Cannot complete action that is not in Executing or RollingBack status"
                        .into(),
                ));
            }
            drop(active);

            execution.status = if success {
                ActionStatus::Completed
            } else {
                ActionStatus::Failed
            };
            execution.completed_at = Some(Utc::now());
            execution.duration_ms = compute_duration_ms(
                execution.started_at,
                execution.completed_at,
            );
            execution.result = Some(if success {
                ActionResult::success(message)
            } else {
                ActionResult::failure(message)
            });
            execution
        };

        self.archive_completed(execution.clone())?;
        Ok(execution)
    }

    /// Mark an executing action as failed with the given error message.
    ///
    /// # Errors
    ///
    /// - [`Error::Pipeline`] if the execution ID is not found.
    /// - [`Error::Validation`] if the execution is not in `Executing`
    ///   or `RollingBack` status.
    #[allow(clippy::significant_drop_tightening)]
    pub fn fail(&self, execution_id: &str, error: &str) -> Result<ActionExecution> {
        let execution = {
            let mut active = self
                .active_actions
                .write()
                .map_err(|e| Error::Other(format!("RwLock poisoned: {e}")))?;

            let mut execution = active.remove(execution_id).ok_or_else(|| {
                Error::Pipeline(format!("Execution not found: {execution_id}"))
            })?;

            if execution.status != ActionStatus::Executing
                && execution.status != ActionStatus::RollingBack
            {
                active.insert(execution.execution_id.clone(), execution);
                return Err(Error::Validation(
                    "Cannot fail action that is not in Executing or RollingBack status"
                        .into(),
                ));
            }
            drop(active);

            execution.status = ActionStatus::Failed;
            execution.completed_at = Some(Utc::now());
            execution.duration_ms = compute_duration_ms(
                execution.started_at,
                execution.completed_at,
            );
            execution.result = Some(ActionResult::failure(error));
            execution
        };

        self.archive_completed(execution.clone())?;
        Ok(execution)
    }

    // -----------------------------------------------------------------------
    // Checkpoints & Rollback
    // -----------------------------------------------------------------------

    /// Save a rollback checkpoint for an active execution.
    ///
    /// Returns the generated rollback ID which can later be passed to
    /// [`rollback`](Self::rollback).
    ///
    /// # Errors
    ///
    /// - [`Error::Pipeline`] if the execution ID is not found among active actions.
    #[allow(clippy::significant_drop_tightening)]
    pub fn save_checkpoint(
        &self,
        execution_id: &str,
        checkpoint: &str,
    ) -> Result<String> {
        let rollback_id = generate_uuid();

        {
            let mut active = self
                .active_actions
                .write()
                .map_err(|e| Error::Other(format!("RwLock poisoned: {e}")))?;

            let execution = active.get_mut(execution_id).ok_or_else(|| {
                Error::Pipeline(format!("Execution not found: {execution_id}"))
            })?;

            execution.rollback_id = Some(rollback_id.clone());
        }

        let info = RollbackInfo {
            rollback_id: rollback_id.clone(),
            execution_id: execution_id.to_owned(),
            checkpoint: checkpoint.to_owned(),
            created_at: Utc::now(),
            executed: false,
        };

        {
            let mut registry = self
                .rollback_registry
                .write()
                .map_err(|e| Error::Other(format!("RwLock poisoned: {e}")))?;
            registry.insert(rollback_id.clone(), info);
        }

        Ok(rollback_id)
    }

    /// Execute a rollback using a previously saved checkpoint.
    ///
    /// This marks the rollback as executed. Actual state restoration is
    /// delegated to L4 Integration bridges.
    ///
    /// # Errors
    ///
    /// - [`Error::Pipeline`] if the rollback ID is not found.
    /// - [`Error::Validation`] if the rollback has already been executed.
    #[allow(clippy::significant_drop_tightening)]
    pub fn rollback(&self, rollback_id: &str) -> Result<()> {
        let execution_id = {
            let mut registry = self
                .rollback_registry
                .write()
                .map_err(|e| Error::Other(format!("RwLock poisoned: {e}")))?;

            let info = registry.get_mut(rollback_id).ok_or_else(|| {
                Error::Pipeline(format!("Rollback not found: {rollback_id}"))
            })?;

            if info.executed {
                return Err(Error::Validation(format!(
                    "Rollback {rollback_id} has already been executed"
                )));
            }

            info.executed = true;
            info.execution_id.clone()
        };

        let mut active = self
            .active_actions
            .write()
            .map_err(|e| Error::Other(format!("RwLock poisoned: {e}")))?;

        if let Some(execution) = active.get_mut(&execution_id) {
            execution.status = ActionStatus::RollingBack;
        }
        drop(active);

        Ok(())
    }

    // -----------------------------------------------------------------------
    // Queries
    // -----------------------------------------------------------------------

    /// Retrieve a clone of an active execution by its ID.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Pipeline`] if the execution ID is not found
    /// among active actions.
    pub fn get_execution(&self, execution_id: &str) -> Result<ActionExecution> {
        let active = self
            .active_actions
            .read()
            .map_err(|e| Error::Other(format!("RwLock poisoned: {e}")))?;

        let result = active
            .get(execution_id)
            .cloned()
            .ok_or_else(|| Error::Pipeline(format!("Execution not found: {execution_id}")));
        drop(active);
        result
    }

    /// Return the number of currently active (non-terminal) executions.
    #[must_use]
    pub fn get_active_count(&self) -> usize {
        self.active_actions
            .read()
            .map_or(0, |guard| guard.len())
    }

    /// Return the number of completed executions in the history buffer.
    #[must_use]
    pub fn get_completed_count(&self) -> usize {
        self.completed_actions
            .read()
            .map_or(0, |guard| guard.len())
    }

    /// Check whether the executor can accept additional action dispatches.
    #[must_use]
    pub fn can_accept_more(&self) -> bool {
        self.get_active_count() < self.max_concurrent
    }

    /// Retrieve all active executions targeting a given service.
    ///
    /// Returns an empty vector if no executions match or if the lock
    /// cannot be acquired.
    #[must_use]
    pub fn get_executions_for_service(&self, service_id: &str) -> Vec<ActionExecution> {
        self.active_actions
            .read()
            .map_or_else(|_| Vec::new(), |guard| {
                guard
                    .values()
                    .filter(|e| e.service_id == service_id)
                    .cloned()
                    .collect()
            })
    }

    /// Calculate the historical success rate from completed executions.
    ///
    /// Returns `0.0` if no completed executions exist.
    #[must_use]
    pub fn get_success_rate(&self) -> f64 {
        let Ok(completed) = self.completed_actions.read() else {
            return 0.0;
        };

        if completed.is_empty() {
            return 0.0;
        }

        let total = completed.len();
        let successes = completed
            .iter()
            .filter(|e| e.status == ActionStatus::Completed)
            .count();
        drop(completed);

        #[allow(clippy::cast_precision_loss)]
        let rate = successes as f64 / total as f64;
        rate
    }

    /// Clear all completed executions from the history buffer.
    ///
    /// Returns the number of entries removed.
    #[must_use]
    pub fn clear_completed(&self) -> usize {
        self.completed_actions.write().map_or(0, |mut guard| {
            let count = guard.len();
            guard.clear();
            count
        })
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    /// Archive a terminal execution into the completed-actions buffer,
    /// enforcing the [`COMPLETED_CAP`] limit by discarding the oldest
    /// entries.
    fn archive_completed(&self, execution: ActionExecution) -> Result<()> {
        let mut completed = self
            .completed_actions
            .write()
            .map_err(|e| Error::Other(format!("RwLock poisoned: {e}")))?;

        completed.push(execution);

        if completed.len() > COMPLETED_CAP {
            let excess = completed.len() - COMPLETED_CAP;
            completed.drain(..excess);
        }
        drop(completed);

        Ok(())
    }

    /// Simulate action execution based on the [`RemediationAction`] variant.
    ///
    /// At this layer all simulations succeed. Real execution with potential
    /// failure modes is handled by L4 Integration bridges.
    fn simulate_action(&self, execution_id: &str) -> Result<ActionResult> {
        let active = self
            .active_actions
            .read()
            .map_err(|e| Error::Other(format!("RwLock poisoned: {e}")))?;

        let execution = active
            .get(execution_id)
            .ok_or_else(|| Error::Pipeline(format!("Execution not found: {execution_id}")))?;

        let result = build_simulated_result(&execution.action);
        drop(active);

        Ok(result)
    }
}

impl Default for ActionExecutor {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Generate a UUID v4 string.
fn generate_uuid() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// Compute the elapsed milliseconds between `started` and `completed`.
///
/// Returns `None` if `completed` is `None` or if the duration cannot
/// be represented as a `u64`.
fn compute_duration_ms(
    started: DateTime<Utc>,
    completed: Option<DateTime<Utc>>,
) -> Option<u64> {
    completed
        .map(|c| c.signed_duration_since(started))
        .and_then(|d| u64::try_from(d.num_milliseconds()).ok())
}

/// Build a simulated [`ActionResult`] for a given [`RemediationAction`].
///
/// Extracted from the executor to keep method line counts within clippy
/// limits and to allow reuse.
fn build_simulated_result(action: &RemediationAction) -> ActionResult {
    match action {
        RemediationAction::RetryWithBackoff {
            max_retries,
            initial_delay_ms,
        } => {
            let mut r = ActionResult::success(format!(
                "Retry with backoff completed: max_retries={max_retries}, \
                 initial_delay={initial_delay_ms}ms"
            ));
            r.metrics_affected.push("error_rate".into());
            r.metrics_affected.push("request_latency".into());
            r
        }
        RemediationAction::CircuitBreakerReset { service_id } => {
            let mut r = ActionResult::success(format!(
                "Circuit breaker reset for service {service_id}"
            ));
            r.metrics_affected.push("circuit_state".into());
            r.side_effects
                .push(format!("Traffic resumed to {service_id}"));
            r
        }
        RemediationAction::ServiceRestart {
            service_id,
            graceful,
        } => {
            let mode = if *graceful { "graceful" } else { "forced" };
            let mut r = ActionResult::success(format!(
                "Service {service_id} restarted ({mode})"
            ));
            r.metrics_affected.push("uptime".into());
            r.metrics_affected.push("health_score".into());
            r.side_effects
                .push(format!("Temporary unavailability of {service_id}"));
            if !graceful {
                r.side_effects
                    .push("In-flight requests may have been dropped".into());
            }
            r
        }
        RemediationAction::GracefulDegradation { service_id, level } => {
            let mut r = ActionResult::success(format!(
                "Service {service_id} degraded to level {level}"
            ));
            r.metrics_affected.push("throughput".into());
            r.metrics_affected.push("feature_availability".into());
            r.side_effects
                .push(format!("Reduced functionality on {service_id}"));
            r
        }
        RemediationAction::FallbackToCached { key, ttl_seconds } => {
            let mut r = ActionResult::success(format!(
                "Fallback to cached data: key={key}, ttl={ttl_seconds}s"
            ));
            r.metrics_affected.push("data_freshness".into());
            r.side_effects.push("Serving stale data".into());
            r
        }
        RemediationAction::CacheCleanup {
            service_id,
            threshold_percent,
        } => {
            let mut r = ActionResult::success(format!(
                "Cache cleanup on {service_id}: cleared {threshold_percent}%"
            ));
            r.metrics_affected.push("memory_usage".into());
            r.metrics_affected.push("cache_hit_rate".into());
            r.side_effects
                .push("Cache miss rate will temporarily increase".into());
            r
        }
        RemediationAction::SessionRotation { session_id } => {
            let mut r = ActionResult::success(format!(
                "Session {session_id} rotated successfully"
            ));
            r.metrics_affected.push("security_posture".into());
            r.side_effects.push(
                "Active sessions using old credentials will need re-auth".into(),
            );
            r
        }
        RemediationAction::DatabaseVacuum { database } => {
            let mut r = ActionResult::success(format!(
                "Database vacuum completed on {database}"
            ));
            r.metrics_affected.push("disk_usage".into());
            r.metrics_affected.push("query_latency".into());
            r.side_effects
                .push(format!("Write lock held on {database} during vacuum"));
            r
        }
        RemediationAction::AlertHuman { message, severity } => {
            let mut r = ActionResult::success(format!(
                "Human alert dispatched: [{severity}] {message}"
            ));
            r.metrics_affected.push("alert_count".into());
            r
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::m3_core_logic::{IssueType, Severity};

    /// Helper to build a [`RemediationRequest`] for testing.
    fn make_request(
        tier: EscalationTier,
        action: RemediationAction,
        service_id: &str,
    ) -> RemediationRequest {
        RemediationRequest {
            id: generate_uuid(),
            service_id: service_id.into(),
            issue_type: IssueType::HealthFailure,
            severity: Severity::Medium,
            confidence: 0.95,
            suggested_action: action,
            tier,
            context: HashMap::new(),
        }
    }

    /// Helper to build a simple L0 cache-cleanup request.
    fn make_l0_request(service_id: &str) -> RemediationRequest {
        make_request(
            EscalationTier::L0AutoExecute,
            RemediationAction::CacheCleanup {
                service_id: service_id.into(),
                threshold_percent: 80,
            },
            service_id,
        )
    }

    // -----------------------------------------------------------------------
    // 1. test_new_executor
    // -----------------------------------------------------------------------

    #[test]
    fn test_new_executor() {
        let executor = ActionExecutor::new();
        assert_eq!(executor.get_active_count(), 0);
        assert_eq!(executor.get_completed_count(), 0);
        assert!(executor.can_accept_more());
        assert!((executor.get_success_rate() - 0.0).abs() < f64::EPSILON);
    }

    // -----------------------------------------------------------------------
    // 2. test_dispatch_l0_auto
    // -----------------------------------------------------------------------

    #[test]
    fn test_dispatch_l0_auto() {
        let executor = ActionExecutor::new();
        let request = make_l0_request("synthex");
        let execution = executor.dispatch(&request);
        assert!(execution.is_ok());
        let execution = match execution {
            Ok(e) => e,
            Err(_) => return,
        };
        assert_eq!(execution.status, ActionStatus::Approved);
        assert_eq!(execution.service_id, "synthex");
        assert_eq!(execution.tier, EscalationTier::L0AutoExecute);
        assert_eq!(executor.get_active_count(), 1);
    }

    // -----------------------------------------------------------------------
    // 3. test_dispatch_l2_needs_approval
    // -----------------------------------------------------------------------

    #[test]
    fn test_dispatch_l2_needs_approval() {
        let executor = ActionExecutor::new();
        let request = make_request(
            EscalationTier::L2RequireApproval,
            RemediationAction::ServiceRestart {
                service_id: "nais".into(),
                graceful: true,
            },
            "nais",
        );

        let result = executor.dispatch(&request);
        assert!(result.is_ok());
        let execution = match result {
            Ok(e) => e,
            Err(_) => return,
        };
        assert_eq!(execution.status, ActionStatus::Pending);
        assert_eq!(execution.tier, EscalationTier::L2RequireApproval);
    }

    // -----------------------------------------------------------------------
    // 4. test_execute_action
    // -----------------------------------------------------------------------

    #[test]
    fn test_execute_action() {
        let executor = ActionExecutor::new();
        let request = make_l0_request("synthex");

        let dispatched = match executor.dispatch(&request) {
            Ok(e) => e,
            Err(_) => return,
        };

        let result = executor.execute(&dispatched.execution_id);
        assert!(result.is_ok());

        let completed = match result {
            Ok(e) => e,
            Err(_) => return,
        };

        assert_eq!(completed.status, ActionStatus::Completed);
        assert!(completed.completed_at.is_some());
        assert!(completed.duration_ms.is_some());
        assert!(completed.result.is_some());

        let action_result = match &completed.result {
            Some(r) => r,
            None => return,
        };
        assert!(action_result.success);
        assert!(!action_result.message.is_empty());

        assert_eq!(executor.get_active_count(), 0);
        assert_eq!(executor.get_completed_count(), 1);
    }

    // -----------------------------------------------------------------------
    // 5. test_complete_success
    // -----------------------------------------------------------------------

    #[test]
    fn test_complete_success() {
        let executor = ActionExecutor::new();
        let request = make_l0_request("san-k7");

        let dispatched = match executor.dispatch(&request) {
            Ok(e) => e,
            Err(_) => return,
        };

        {
            let mut active = match executor.active_actions.write() {
                Ok(g) => g,
                Err(_) => return,
            };
            if let Some(exec) = active.get_mut(&dispatched.execution_id) {
                exec.status = ActionStatus::Executing;
            }
        }

        let result = executor.complete(&dispatched.execution_id, true, "All good");
        assert!(result.is_ok());
        let completed = match result {
            Ok(e) => e,
            Err(_) => return,
        };

        assert_eq!(completed.status, ActionStatus::Completed);
        let action_result = match &completed.result {
            Some(r) => r,
            None => return,
        };
        assert!(action_result.success);
        assert_eq!(action_result.message, "All good");
    }

    // -----------------------------------------------------------------------
    // 6. test_complete_failure
    // -----------------------------------------------------------------------

    #[test]
    fn test_complete_failure() {
        let executor = ActionExecutor::new();
        let request = make_l0_request("nais");

        let dispatched = match executor.dispatch(&request) {
            Ok(e) => e,
            Err(_) => return,
        };

        {
            let mut active = match executor.active_actions.write() {
                Ok(g) => g,
                Err(_) => return,
            };
            if let Some(exec) = active.get_mut(&dispatched.execution_id) {
                exec.status = ActionStatus::Executing;
            }
        }

        let result = executor.fail(&dispatched.execution_id, "Connection refused");
        assert!(result.is_ok());
        let failed = match result {
            Ok(e) => e,
            Err(_) => return,
        };

        assert_eq!(failed.status, ActionStatus::Failed);
        let action_result = match &failed.result {
            Some(r) => r,
            None => return,
        };
        assert!(!action_result.success);
        assert!(action_result.message.contains("Connection refused"));
    }

    // -----------------------------------------------------------------------
    // 7. test_max_concurrent_limit
    // -----------------------------------------------------------------------

    #[test]
    fn test_max_concurrent_limit() {
        let executor = ActionExecutor::with_max_concurrent(2);

        let r1 = make_l0_request("svc-1");
        let r2 = make_l0_request("svc-2");
        let r3 = make_l0_request("svc-3");

        assert!(executor.dispatch(&r1).is_ok());
        assert!(executor.dispatch(&r2).is_ok());

        let result = executor.dispatch(&r3);
        assert!(result.is_err());
        assert_eq!(executor.get_active_count(), 2);
    }

    // -----------------------------------------------------------------------
    // 8. test_save_and_rollback_checkpoint
    // -----------------------------------------------------------------------

    #[test]
    fn test_save_and_rollback_checkpoint() {
        let executor = ActionExecutor::new();
        let request = make_l0_request("synthex");

        let dispatched = match executor.dispatch(&request) {
            Ok(e) => e,
            Err(_) => return,
        };

        let rollback_id = match executor.save_checkpoint(
            &dispatched.execution_id,
            r#"{"state":"healthy","connections":42}"#,
        ) {
            Ok(id) => id,
            Err(_) => return,
        };

        let exec = match executor.get_execution(&dispatched.execution_id) {
            Ok(e) => e,
            Err(_) => return,
        };
        assert_eq!(exec.rollback_id.as_deref(), Some(rollback_id.as_str()));

        let rollback_result = executor.rollback(&rollback_id);
        assert!(rollback_result.is_ok());

        let exec_after = match executor.get_execution(&dispatched.execution_id) {
            Ok(e) => e,
            Err(_) => return,
        };
        assert_eq!(exec_after.status, ActionStatus::RollingBack);

        let double_rollback = executor.rollback(&rollback_id);
        assert!(double_rollback.is_err());
    }

    // -----------------------------------------------------------------------
    // 9. test_approve_execution
    // -----------------------------------------------------------------------

    #[test]
    fn test_approve_execution() {
        let executor = ActionExecutor::new();
        let request = make_request(
            EscalationTier::L2RequireApproval,
            RemediationAction::GracefulDegradation {
                service_id: "codesynthor".into(),
                level: 3,
            },
            "codesynthor",
        );

        let dispatched = match executor.dispatch(&request) {
            Ok(e) => e,
            Err(_) => return,
        };
        assert_eq!(dispatched.status, ActionStatus::Pending);

        let approved = match executor.approve(&dispatched.execution_id) {
            Ok(e) => e,
            Err(_) => return,
        };
        assert_eq!(approved.status, ActionStatus::Approved);

        let double_approve = executor.approve(&dispatched.execution_id);
        assert!(double_approve.is_err());
    }

    // -----------------------------------------------------------------------
    // 10. test_reject_execution
    // -----------------------------------------------------------------------

    #[test]
    fn test_reject_execution() {
        let executor = ActionExecutor::new();
        let request = make_request(
            EscalationTier::L3PbftConsensus,
            RemediationAction::DatabaseVacuum {
                database: "hebbian_pulse.db".into(),
            },
            "database",
        );

        let dispatched = match executor.dispatch(&request) {
            Ok(e) => e,
            Err(_) => return,
        };
        assert_eq!(dispatched.status, ActionStatus::Pending);

        let rejected = match executor.reject(
            &dispatched.execution_id,
            "Insufficient quorum",
        ) {
            Ok(e) => e,
            Err(_) => return,
        };

        assert_eq!(rejected.status, ActionStatus::Rejected);
        assert!(rejected.result.is_some());
        let action_result = match &rejected.result {
            Some(r) => r,
            None => return,
        };
        assert!(!action_result.success);
        assert!(action_result.message.contains("Insufficient quorum"));

        assert_eq!(executor.get_active_count(), 0);
        assert_eq!(executor.get_completed_count(), 1);
    }

    // -----------------------------------------------------------------------
    // 11. test_get_executions_for_service
    // -----------------------------------------------------------------------

    #[test]
    fn test_get_executions_for_service() {
        let executor = ActionExecutor::new();

        let r1 = make_l0_request("synthex");
        let r2 = make_l0_request("synthex");
        let r3 = make_l0_request("nais");

        assert!(executor.dispatch(&r1).is_ok());
        assert!(executor.dispatch(&r2).is_ok());
        assert!(executor.dispatch(&r3).is_ok());

        let synthex_execs = executor.get_executions_for_service("synthex");
        assert_eq!(synthex_execs.len(), 2);
        for exec in &synthex_execs {
            assert_eq!(exec.service_id, "synthex");
        }

        let nais_execs = executor.get_executions_for_service("nais");
        assert_eq!(nais_execs.len(), 1);

        let empty_execs = executor.get_executions_for_service("nonexistent");
        assert!(empty_execs.is_empty());
    }

    // -----------------------------------------------------------------------
    // 12. test_success_rate
    // -----------------------------------------------------------------------

    #[test]
    fn test_success_rate() {
        let executor = ActionExecutor::new();

        assert!((executor.get_success_rate() - 0.0).abs() < f64::EPSILON);

        for i in 0..3 {
            let request = make_l0_request(&format!("svc-{i}"));
            let dispatched = match executor.dispatch(&request) {
                Ok(e) => e,
                Err(_) => return,
            };
            assert!(executor.execute(&dispatched.execution_id).is_ok());
        }

        assert!((executor.get_success_rate() - 1.0).abs() < f64::EPSILON);

        let rejected_req = make_request(
            EscalationTier::L2RequireApproval,
            RemediationAction::CacheCleanup {
                service_id: "svc-fail".into(),
                threshold_percent: 50,
            },
            "svc-fail",
        );
        let dispatched = match executor.dispatch(&rejected_req) {
            Ok(e) => e,
            Err(_) => return,
        };
        assert!(executor
            .reject(&dispatched.execution_id, "test rejection")
            .is_ok());

        // 3 completed + 1 rejected = 4 total, 3 successful -> 0.75
        assert!((executor.get_success_rate() - 0.75).abs() < f64::EPSILON);
    }

    // -----------------------------------------------------------------------
    // 13. test_completed_cap
    // -----------------------------------------------------------------------

    #[test]
    fn test_completed_cap() {
        let executor = ActionExecutor::with_max_concurrent(600);

        for i in 0..550 {
            let request = make_l0_request(&format!("svc-{i}"));
            let dispatched = match executor.dispatch(&request) {
                Ok(e) => e,
                Err(_) => continue,
            };
            let _ = executor.execute(&dispatched.execution_id);
        }

        assert!(executor.get_completed_count() <= COMPLETED_CAP);
        assert_eq!(executor.get_completed_count(), COMPLETED_CAP);
    }

    // -----------------------------------------------------------------------
    // 14. test_can_accept_more
    // -----------------------------------------------------------------------

    #[test]
    fn test_can_accept_more() {
        let executor = ActionExecutor::with_max_concurrent(3);

        assert!(executor.can_accept_more());

        let r1 = make_l0_request("a");
        let r2 = make_l0_request("b");
        let r3 = make_l0_request("c");

        assert!(executor.dispatch(&r1).is_ok());
        assert!(executor.can_accept_more());

        assert!(executor.dispatch(&r2).is_ok());
        assert!(executor.can_accept_more());

        let d3 = match executor.dispatch(&r3) {
            Ok(e) => e,
            Err(_) => return,
        };
        assert!(!executor.can_accept_more());

        assert!(executor.execute(&d3.execution_id).is_ok());
        assert!(executor.can_accept_more());
    }

    // -----------------------------------------------------------------------
    // 15. test_clear_completed
    // -----------------------------------------------------------------------

    #[test]
    fn test_clear_completed() {
        let executor = ActionExecutor::new();

        for i in 0..5 {
            let request = make_l0_request(&format!("svc-{i}"));
            let dispatched = match executor.dispatch(&request) {
                Ok(e) => e,
                Err(_) => continue,
            };
            let _ = executor.execute(&dispatched.execution_id);
        }

        assert_eq!(executor.get_completed_count(), 5);

        let cleared = executor.clear_completed();
        assert_eq!(cleared, 5);
        assert_eq!(executor.get_completed_count(), 0);
    }

    // -----------------------------------------------------------------------
    // 16. test_execute_requires_approved_status
    // -----------------------------------------------------------------------

    #[test]
    fn test_execute_requires_approved_status() {
        let executor = ActionExecutor::new();
        let request = make_request(
            EscalationTier::L2RequireApproval,
            RemediationAction::SessionRotation {
                session_id: "sess-42".into(),
            },
            "auth-service",
        );

        let dispatched = match executor.dispatch(&request) {
            Ok(e) => e,
            Err(_) => return,
        };

        let result = executor.execute(&dispatched.execution_id);
        assert!(result.is_err());

        assert!(executor.approve(&dispatched.execution_id).is_ok());
        let executed = executor.execute(&dispatched.execution_id);
        assert!(executed.is_ok());
    }

    // -----------------------------------------------------------------------
    // 17. test_l1_dispatches_as_approved
    // -----------------------------------------------------------------------

    #[test]
    fn test_l1_dispatches_as_approved() {
        let executor = ActionExecutor::new();
        let request = make_request(
            EscalationTier::L1NotifyHuman,
            RemediationAction::RetryWithBackoff {
                max_retries: 3,
                initial_delay_ms: 100,
            },
            "api-gateway",
        );

        let dispatched = match executor.dispatch(&request) {
            Ok(e) => e,
            Err(_) => return,
        };

        assert_eq!(dispatched.status, ActionStatus::Approved);
    }

    // -----------------------------------------------------------------------
    // 18. test_default_implementation
    // -----------------------------------------------------------------------

    #[test]
    fn test_default_implementation() {
        let executor = ActionExecutor::default();
        assert_eq!(executor.get_active_count(), 0);
        assert!(executor.can_accept_more());
    }

    // -----------------------------------------------------------------------
    // 19. test_dispatch_l3_needs_approval
    // -----------------------------------------------------------------------
    #[test]
    fn test_dispatch_l3_needs_approval() {
        let executor = ActionExecutor::new();
        let request = make_request(
            EscalationTier::L3PbftConsensus,
            RemediationAction::DatabaseVacuum {
                database: "test.db".into(),
            },
            "db-svc",
        );
        let result = executor.dispatch(&request);
        assert!(result.is_ok());
        if let Ok(e) = result {
            assert_eq!(e.status, ActionStatus::Pending);
            assert_eq!(e.tier, EscalationTier::L3PbftConsensus);
        }
    }

    // -----------------------------------------------------------------------
    // 20. test_execute_pending_fails
    // -----------------------------------------------------------------------
    #[test]
    fn test_execute_pending_fails() {
        let executor = ActionExecutor::new();
        let request = make_request(
            EscalationTier::L2RequireApproval,
            RemediationAction::CacheCleanup {
                service_id: "svc".into(),
                threshold_percent: 50,
            },
            "svc",
        );
        let dispatched = executor.dispatch(&request);
        assert!(dispatched.is_ok());
        if let Ok(e) = dispatched {
            // Trying to execute a Pending action should fail
            let result = executor.execute(&e.execution_id);
            assert!(result.is_err());
        }
    }

    // -----------------------------------------------------------------------
    // 21. test_reject_non_pending_fails
    // -----------------------------------------------------------------------
    #[test]
    fn test_reject_non_pending_fails() {
        let executor = ActionExecutor::new();
        let request = make_l0_request("svc");
        let dispatched = executor.dispatch(&request);
        assert!(dispatched.is_ok());
        if let Ok(e) = dispatched {
            // L0 dispatches as Approved, reject should fail
            let result = executor.reject(&e.execution_id, "test");
            assert!(result.is_err());
        }
    }

    // -----------------------------------------------------------------------
    // 22. test_approve_nonexistent_fails
    // -----------------------------------------------------------------------
    #[test]
    fn test_approve_nonexistent_fails() {
        let executor = ActionExecutor::new();
        let result = executor.approve("nonexistent-id");
        assert!(result.is_err());
    }

    // -----------------------------------------------------------------------
    // 23. test_reject_nonexistent_fails
    // -----------------------------------------------------------------------
    #[test]
    fn test_reject_nonexistent_fails() {
        let executor = ActionExecutor::new();
        let result = executor.reject("nonexistent-id", "reason");
        assert!(result.is_err());
    }

    // -----------------------------------------------------------------------
    // 24. test_complete_nonexistent_fails
    // -----------------------------------------------------------------------
    #[test]
    fn test_complete_nonexistent_fails() {
        let executor = ActionExecutor::new();
        let result = executor.complete("nonexistent-id", true, "ok");
        assert!(result.is_err());
    }

    // -----------------------------------------------------------------------
    // 25. test_fail_nonexistent_fails
    // -----------------------------------------------------------------------
    #[test]
    fn test_fail_nonexistent_fails() {
        let executor = ActionExecutor::new();
        let result = executor.fail("nonexistent-id", "error");
        assert!(result.is_err());
    }

    // -----------------------------------------------------------------------
    // 26. test_save_checkpoint_nonexistent_fails
    // -----------------------------------------------------------------------
    #[test]
    fn test_save_checkpoint_nonexistent_fails() {
        let executor = ActionExecutor::new();
        let result = executor.save_checkpoint("nonexistent", "data");
        assert!(result.is_err());
    }

    // -----------------------------------------------------------------------
    // 27. test_rollback_nonexistent_fails
    // -----------------------------------------------------------------------
    #[test]
    fn test_rollback_nonexistent_fails() {
        let executor = ActionExecutor::new();
        let result = executor.rollback("nonexistent-rollback-id");
        assert!(result.is_err());
    }

    // -----------------------------------------------------------------------
    // 28. test_get_execution_nonexistent_fails
    // -----------------------------------------------------------------------
    #[test]
    fn test_get_execution_nonexistent_fails() {
        let executor = ActionExecutor::new();
        let result = executor.get_execution("nonexistent");
        assert!(result.is_err());
    }

    // -----------------------------------------------------------------------
    // 29. test_with_max_concurrent_zero_becomes_one
    // -----------------------------------------------------------------------
    #[test]
    fn test_with_max_concurrent_zero_becomes_one() {
        let executor = ActionExecutor::with_max_concurrent(0);
        assert!(executor.can_accept_more());

        let r = make_l0_request("svc");
        assert!(executor.dispatch(&r).is_ok());
        assert!(!executor.can_accept_more());
    }

    // -----------------------------------------------------------------------
    // 30. test_action_result_success_helper
    // -----------------------------------------------------------------------
    #[test]
    fn test_action_result_success_helper() {
        let result = ActionResult::success("all good");
        assert!(result.success);
        assert_eq!(result.message, "all good");
        assert!(result.metrics_affected.is_empty());
        assert!(result.side_effects.is_empty());
    }

    // -----------------------------------------------------------------------
    // 31. test_action_result_failure_helper
    // -----------------------------------------------------------------------
    #[test]
    fn test_action_result_failure_helper() {
        let result = ActionResult::failure("connection refused");
        assert!(!result.success);
        assert_eq!(result.message, "connection refused");
        assert!(result.metrics_affected.is_empty());
        assert!(result.side_effects.is_empty());
    }

    // -----------------------------------------------------------------------
    // 32. test_dispatch_populates_fields
    // -----------------------------------------------------------------------
    #[test]
    fn test_dispatch_populates_fields() {
        let executor = ActionExecutor::new();
        let request = make_l0_request("synthex");
        let result = executor.dispatch(&request);
        assert!(result.is_ok());
        if let Ok(e) = result {
            assert_eq!(e.request_id, request.id);
            assert_eq!(e.service_id, "synthex");
            assert!(e.completed_at.is_none());
            assert!(e.duration_ms.is_none());
            assert!(e.result.is_none());
            assert!(e.rollback_id.is_none());
            assert!(!e.execution_id.is_empty());
        }
    }

    // -----------------------------------------------------------------------
    // 33. test_execute_all_action_types
    // -----------------------------------------------------------------------
    #[test]
    fn test_execute_all_action_types() {
        let actions: Vec<RemediationAction> = vec![
            RemediationAction::RetryWithBackoff {
                max_retries: 3,
                initial_delay_ms: 100,
            },
            RemediationAction::CircuitBreakerReset {
                service_id: "svc".into(),
            },
            RemediationAction::ServiceRestart {
                service_id: "svc".into(),
                graceful: true,
            },
            RemediationAction::GracefulDegradation {
                service_id: "svc".into(),
                level: 2,
            },
            RemediationAction::FallbackToCached {
                key: "k".into(),
                ttl_seconds: 60,
            },
            RemediationAction::CacheCleanup {
                service_id: "svc".into(),
                threshold_percent: 50,
            },
            RemediationAction::SessionRotation {
                session_id: "s1".into(),
            },
            RemediationAction::DatabaseVacuum {
                database: "db".into(),
            },
            RemediationAction::AlertHuman {
                message: "help".into(),
                severity: "High".into(),
            },
        ];

        for action in actions {
            let executor = ActionExecutor::new();
            let request = make_request(EscalationTier::L0AutoExecute, action, "svc");
            let dispatched = executor.dispatch(&request);
            assert!(dispatched.is_ok());
            if let Ok(e) = dispatched {
                let result = executor.execute(&e.execution_id);
                assert!(result.is_ok());
                if let Ok(completed) = result {
                    assert_eq!(completed.status, ActionStatus::Completed);
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // 34. test_complete_not_executing_fails
    // -----------------------------------------------------------------------
    #[test]
    fn test_complete_not_executing_fails() {
        let executor = ActionExecutor::new();
        let request = make_l0_request("svc");
        let dispatched = executor.dispatch(&request);
        assert!(dispatched.is_ok());
        if let Ok(e) = dispatched {
            // Approved but not Executing -> complete should fail
            let result = executor.complete(&e.execution_id, true, "ok");
            assert!(result.is_err());
        }
    }

    // -----------------------------------------------------------------------
    // 35. test_fail_not_executing_fails
    // -----------------------------------------------------------------------
    #[test]
    fn test_fail_not_executing_fails() {
        let executor = ActionExecutor::new();
        let request = make_l0_request("svc");
        let dispatched = executor.dispatch(&request);
        assert!(dispatched.is_ok());
        if let Ok(e) = dispatched {
            let result = executor.fail(&e.execution_id, "err");
            assert!(result.is_err());
        }
    }

    // -----------------------------------------------------------------------
    // 36. test_rejected_goes_to_completed_count
    // -----------------------------------------------------------------------
    #[test]
    fn test_rejected_goes_to_completed_count() {
        let executor = ActionExecutor::new();
        let request = make_request(
            EscalationTier::L2RequireApproval,
            RemediationAction::CacheCleanup {
                service_id: "svc".into(),
                threshold_percent: 50,
            },
            "svc",
        );
        let dispatched = executor.dispatch(&request);
        assert!(dispatched.is_ok());
        if let Ok(e) = dispatched {
            let _ = executor.reject(&e.execution_id, "no quorum");
            assert_eq!(executor.get_completed_count(), 1);
            assert_eq!(executor.get_active_count(), 0);
        }
    }

    // -----------------------------------------------------------------------
    // 37. test_success_rate_with_mix
    // -----------------------------------------------------------------------
    #[test]
    fn test_success_rate_with_mix() {
        let executor = ActionExecutor::new();

        // 2 successes
        for _ in 0..2 {
            let r = make_l0_request("svc");
            let d = executor.dispatch(&r);
            if let Ok(e) = d {
                let _ = executor.execute(&e.execution_id);
            }
        }

        // 2 rejections (count as non-success)
        for _ in 0..2 {
            let r = make_request(
                EscalationTier::L2RequireApproval,
                RemediationAction::CacheCleanup {
                    service_id: "svc".into(),
                    threshold_percent: 50,
                },
                "svc",
            );
            let d = executor.dispatch(&r);
            if let Ok(e) = d {
                let _ = executor.reject(&e.execution_id, "test");
            }
        }

        // 2 completed / 4 total = 0.5
        assert!((executor.get_success_rate() - 0.5).abs() < f64::EPSILON);
    }

    // -----------------------------------------------------------------------
    // 38. test_get_executions_for_nonexistent_service
    // -----------------------------------------------------------------------
    #[test]
    fn test_get_executions_for_nonexistent_service() {
        let executor = ActionExecutor::new();
        let result = executor.get_executions_for_service("doesnt-exist");
        assert!(result.is_empty());
    }

    // -----------------------------------------------------------------------
    // 39. test_clear_completed_empty
    // -----------------------------------------------------------------------
    #[test]
    fn test_clear_completed_empty() {
        let executor = ActionExecutor::new();
        let cleared = executor.clear_completed();
        assert_eq!(cleared, 0);
    }

    // -----------------------------------------------------------------------
    // 40. test_execution_id_is_unique
    // -----------------------------------------------------------------------
    #[test]
    fn test_execution_id_is_unique() {
        let executor = ActionExecutor::new();
        let r1 = make_l0_request("svc");
        let r2 = make_l0_request("svc");

        let d1 = executor.dispatch(&r1);
        let d2 = executor.dispatch(&r2);

        if let (Ok(e1), Ok(e2)) = (d1, d2) {
            assert_ne!(e1.execution_id, e2.execution_id);
        }
    }

    // -----------------------------------------------------------------------
    // 41. test_approve_then_execute_full_lifecycle
    // -----------------------------------------------------------------------
    #[test]
    fn test_approve_then_execute_full_lifecycle() {
        let executor = ActionExecutor::new();
        let request = make_request(
            EscalationTier::L2RequireApproval,
            RemediationAction::RetryWithBackoff {
                max_retries: 5,
                initial_delay_ms: 200,
            },
            "gateway",
        );

        let dispatched = executor.dispatch(&request);
        assert!(dispatched.is_ok());
        if let Ok(e) = dispatched {
            assert_eq!(e.status, ActionStatus::Pending);
            assert_eq!(executor.get_active_count(), 1);

            let approved = executor.approve(&e.execution_id);
            assert!(approved.is_ok());

            let executed = executor.execute(&e.execution_id);
            assert!(executed.is_ok());
            if let Ok(done) = executed {
                assert_eq!(done.status, ActionStatus::Completed);
                assert!(done.result.is_some());
            }
            assert_eq!(executor.get_active_count(), 0);
            assert_eq!(executor.get_completed_count(), 1);
        }
    }

    // -----------------------------------------------------------------------
    // 42. test_rollback_sets_rolling_back_status
    // -----------------------------------------------------------------------
    #[test]
    fn test_rollback_sets_rolling_back_status() {
        let executor = ActionExecutor::new();
        let request = make_l0_request("svc");
        let dispatched = executor.dispatch(&request);
        assert!(dispatched.is_ok());
        if let Ok(e) = dispatched {
            let rb_id = executor.save_checkpoint(&e.execution_id, "state-data");
            assert!(rb_id.is_ok());
            if let Ok(rid) = rb_id {
                let _ = executor.rollback(&rid);
                let execution = executor.get_execution(&e.execution_id);
                assert!(execution.is_ok());
                if let Ok(ex) = execution {
                    assert_eq!(ex.status, ActionStatus::RollingBack);
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // 43. test_complete_rolling_back_succeeds
    // -----------------------------------------------------------------------
    #[test]
    fn test_complete_rolling_back_succeeds() {
        let executor = ActionExecutor::new();
        let request = make_l0_request("svc");
        let dispatched = executor.dispatch(&request);
        assert!(dispatched.is_ok());
        if let Ok(e) = dispatched {
            let rb_id = executor.save_checkpoint(&e.execution_id, "data");
            if let Ok(rid) = rb_id {
                let _ = executor.rollback(&rid);
                // Now in RollingBack status, complete should work
                let result = executor.complete(&e.execution_id, true, "rollback done");
                assert!(result.is_ok());
                if let Ok(completed) = result {
                    assert_eq!(completed.status, ActionStatus::Completed);
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // 44. test_dispatched_has_correct_action
    // -----------------------------------------------------------------------
    #[test]
    fn test_dispatched_has_correct_action() {
        let executor = ActionExecutor::new();
        let action = RemediationAction::SessionRotation {
            session_id: "sess-99".into(),
        };
        let request = make_request(EscalationTier::L0AutoExecute, action.clone(), "auth");
        let dispatched = executor.dispatch(&request);
        assert!(dispatched.is_ok());
        if let Ok(e) = dispatched {
            assert!(matches!(
                e.action,
                RemediationAction::SessionRotation { .. }
            ));
        }
    }

    // -----------------------------------------------------------------------
    // 45. test_simulated_result_has_metrics
    // -----------------------------------------------------------------------
    #[test]
    fn test_simulated_result_has_metrics() {
        // Test the build_simulated_result function directly since execute()
        // passes through complete() which builds a fresh ActionResult.
        let result = build_simulated_result(&RemediationAction::ServiceRestart {
            service_id: "svc".into(),
            graceful: false,
        });
        assert!(result.success);
        assert!(!result.metrics_affected.is_empty());
        assert!(!result.side_effects.is_empty());
    }

    // -----------------------------------------------------------------------
    // 46. test_graceful_restart_has_side_effects
    // -----------------------------------------------------------------------
    #[test]
    fn test_graceful_restart_has_side_effects() {
        let result = build_simulated_result(&RemediationAction::ServiceRestart {
            service_id: "svc".into(),
            graceful: true,
        });
        assert!(result.success);
        assert!(!result.side_effects.is_empty());
        // Graceful should not mention dropped requests
        let has_dropped = result
            .side_effects
            .iter()
            .any(|s| s.contains("dropped"));
        assert!(!has_dropped);
    }

    // -----------------------------------------------------------------------
    // 47. test_forced_restart_mentions_dropped
    // -----------------------------------------------------------------------
    #[test]
    fn test_forced_restart_mentions_dropped() {
        let result = build_simulated_result(&RemediationAction::ServiceRestart {
            service_id: "svc".into(),
            graceful: false,
        });
        assert!(result.success);
        let has_dropped = result
            .side_effects
            .iter()
            .any(|s| s.contains("dropped"));
        assert!(has_dropped);
    }

    // -----------------------------------------------------------------------
    // 48. test_cache_cleanup_result
    // -----------------------------------------------------------------------
    #[test]
    fn test_cache_cleanup_result() {
        let result = build_simulated_result(&RemediationAction::CacheCleanup {
            service_id: "svc".into(),
            threshold_percent: 75,
        });
        assert!(result.success);
        assert!(result.message.contains("75"));
        assert!(result.metrics_affected.contains(&"memory_usage".to_owned()));
    }

    // -----------------------------------------------------------------------
    // 49. test_alert_human_result
    // -----------------------------------------------------------------------
    #[test]
    fn test_alert_human_result() {
        let result = build_simulated_result(&RemediationAction::AlertHuman {
            message: "disk full".into(),
            severity: "Critical".into(),
        });
        assert!(result.success);
        assert!(result.message.contains("disk full"));
        assert!(result.message.contains("Critical"));
    }

    // -----------------------------------------------------------------------
    // 50. test_compute_duration_ms_function
    // -----------------------------------------------------------------------
    #[test]
    fn test_compute_duration_ms_function() {
        let start = Utc::now();
        // None completed -> None duration
        let result = compute_duration_ms(start, None);
        assert!(result.is_none());

        // Same timestamp -> 0ms
        let result = compute_duration_ms(start, Some(start));
        assert!(result.is_some());
        if let Some(d) = result {
            assert_eq!(d, 0);
        }
    }
}
