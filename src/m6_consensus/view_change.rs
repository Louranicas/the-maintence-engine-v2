//! # M34: View Change Handler
//!
//! PBFT leader election and view change protocol implementation.
//! Manages deterministic primary selection, view change request collection,
//! execution, and history tracking for the CVA-NAM agent fleet.
//!
//! ## Layer: L6 (Consensus)
//!
//! ## View Change Lifecycle
//!
//! ```text
//! Idle -> Requested -> Collecting -> Executing -> Complete
//!                                             \-> Failed
//! ```
//!
//! ## Primary Selection
//!
//! The primary (leader) for view `v` is determined by:
//! ```text
//! primary_index = v % agent_count
//! ```
//!
//! This ensures deterministic, round-robin leader rotation across views.
//!
//! ## Related Documentation
//! - [Module Specification](../../ai_docs/modules/M34_VIEW_CHANGE_HANDLER.md)
//! - [PBFT Consensus](../../nam/PBFT_CONSENSUS.md)

use std::time::SystemTime;

use parking_lot::RwLock;

use crate::{Error, Result};

use super::ConsensusAction;

/// Maximum number of view change history records retained.
const MAX_VIEW_HISTORY: usize = 200;

/// Maximum number of pending view change requests.
const MAX_PENDING_REQUESTS: usize = 100;

/// Base timeout in milliseconds for `ServiceTermination`.
const BASE_TIMEOUT_SERVICE_TERMINATION_MS: u64 = 60_000;
/// Base timeout in milliseconds for `DatabaseMigration`.
const BASE_TIMEOUT_DATABASE_MIGRATION_MS: u64 = 300_000;
/// Base timeout in milliseconds for `CredentialRotation`.
const BASE_TIMEOUT_CREDENTIAL_ROTATION_MS: u64 = 120_000;
/// Base timeout in milliseconds for `CascadeRestart`.
const BASE_TIMEOUT_CASCADE_RESTART_MS: u64 = 180_000;
/// Base timeout in milliseconds for `ConfigRollback`.
const BASE_TIMEOUT_CONFIG_ROLLBACK_MS: u64 = 90_000;

/// Reason for initiating a view change.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ViewChangeReason {
    /// The proposal phase timed out waiting for the primary.
    ProposalTimeout,
    /// The prepare phase timed out before quorum.
    PrepareTimeout,
    /// The commit phase timed out before quorum.
    CommitTimeout,
    /// Quorum could not be reached for the current view.
    QuorumFailure,
    /// The primary proposed an invalid or malformed proposal.
    InvalidProposal,
    /// Enhanced consensus (Critic + Integrator) could not be achieved.
    EnhancedConsensusFail,
    /// The execution phase of a committed proposal failed.
    ExecutionFailure,
    /// A manual (operator-initiated) view change trigger.
    ManualTrigger,
}

/// A request to initiate a view change.
#[derive(Clone, Debug)]
pub struct ViewChangeRequest {
    /// Unique request identifier.
    pub id: String,
    /// The view number being moved away from.
    pub from_view: u64,
    /// The view number being moved to.
    pub to_view: u64,
    /// Reason for the view change.
    pub reason: ViewChangeReason,
    /// Agent or operator that requested the change.
    pub requester: String,
    /// When the request was created.
    pub timestamp: SystemTime,
}

/// State of the view change process.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewChangeState {
    /// No view change in progress.
    Idle,
    /// A view change has been requested.
    Requested,
    /// Collecting acknowledgements from agents.
    Collecting,
    /// Executing the view transition.
    Executing,
    /// View change completed successfully.
    Complete,
    /// View change failed.
    Failed,
}

/// Current view state of the consensus protocol.
#[derive(Clone, Debug)]
pub struct ViewState {
    /// Current view number.
    pub view_number: u64,
    /// ID of the current primary (leader).
    pub primary_id: String,
    /// Current view change state.
    pub state: ViewChangeState,
    /// When the current view change started (if any).
    pub started_at: Option<SystemTime>,
    /// When the current view change completed (if any).
    pub completed_at: Option<SystemTime>,
    /// Total number of view changes performed.
    pub change_count: u64,
}

/// Historical record of a completed view change.
#[derive(Clone, Debug)]
pub struct ViewChangeRecord {
    /// View number transitioned from.
    pub from_view: u64,
    /// View number transitioned to.
    pub to_view: u64,
    /// Reason for the view change.
    pub reason: ViewChangeReason,
    /// ID of the previous primary.
    pub old_primary: String,
    /// ID of the new primary.
    pub new_primary: String,
    /// Duration of the view change in milliseconds.
    pub duration_ms: u64,
    /// Whether the view change succeeded.
    pub success: bool,
    /// When the view change occurred.
    pub timestamp: SystemTime,
}

/// PBFT view change handler.
///
/// Manages leader election via deterministic primary selection,
/// view change request collection, execution, and history tracking.
/// All view changes follow the round-robin `view_number % agent_count`
/// primary selection algorithm.
pub struct ViewChangeHandler {
    /// Current view state, protected by a read-write lock.
    current_view: RwLock<ViewState>,
    /// History of completed view changes (capped at [`MAX_VIEW_HISTORY`]).
    view_history: RwLock<Vec<ViewChangeRecord>>,
    /// Pending view change requests (capped at [`MAX_PENDING_REQUESTS`]).
    pending_requests: RwLock<Vec<ViewChangeRequest>>,
    /// Fleet agent IDs for deterministic leader selection.
    agent_ids: Vec<String>,
    /// Timeout multiplier applied to base action timeouts.
    timeout_multiplier: RwLock<f64>,
}

impl ViewChangeHandler {
    /// Create a new view change handler with the given agent fleet.
    ///
    /// Initializes at view 0 with the first agent as primary.
    /// If the agent list is empty, the primary is set to `"unknown"`.
    #[must_use]
    pub fn new(agent_ids: Vec<String>) -> Self {
        let primary = agent_ids.first().cloned().unwrap_or_else(|| "unknown".into());
        Self {
            current_view: RwLock::new(ViewState {
                view_number: 0,
                primary_id: primary,
                state: ViewChangeState::Idle,
                started_at: None,
                completed_at: None,
                change_count: 0,
            }),
            view_history: RwLock::new(Vec::new()),
            pending_requests: RwLock::new(Vec::new()),
            agent_ids,
            timeout_multiplier: RwLock::new(1.0),
        }
    }

    /// Get the current view number.
    #[must_use]
    pub fn current_view(&self) -> u64 {
        self.current_view.read().view_number
    }

    /// Get the ID of the current primary (leader).
    ///
    /// # Errors
    ///
    /// Never returns an error. The `Result` return type is retained for
    /// API stability; `parking_lot::RwLock` does not poison, so the
    /// previous poison-error path is unreachable.
    pub fn current_primary(&self) -> Result<String> {
        Ok(self.current_view.read().primary_id.clone())
    }

    /// Deterministic primary selection for a given view number.
    ///
    /// Returns the index into the agent list: `view_number % agent_count`.
    /// If `agent_count` is zero, returns 0.
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub const fn select_primary(view_number: u64, agent_count: usize) -> usize {
        if agent_count == 0 {
            return 0;
        }
        (view_number % agent_count as u64) as usize
    }

    /// Request a view change for the given reason.
    ///
    /// Creates a request to transition from the current view to the next.
    /// The request is added to the pending queue and the state transitions
    /// to `Requested` (if currently `Idle`).
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if the requester is empty. Lock-poison
    /// errors are no longer possible under `parking_lot::RwLock`.
    pub fn request_view_change(
        &self,
        reason: ViewChangeReason,
        requester: &str,
    ) -> Result<String> {
        if requester.is_empty() {
            return Err(Error::Validation("Requester cannot be empty".into()));
        }

        let mut view_guard = self.current_view.write();

        let from_view = view_guard.view_number;
        let to_view = from_view + 1;
        let request_id = format!("vcr-{from_view}-{to_view}-{requester}");

        let request = ViewChangeRequest {
            id: request_id.clone(),
            from_view,
            to_view,
            reason,
            requester: requester.into(),
            timestamp: SystemTime::now(),
        };

        // Transition to Requested if Idle
        if view_guard.state == ViewChangeState::Idle {
            view_guard.state = ViewChangeState::Requested;
            view_guard.started_at = Some(SystemTime::now());
        }

        drop(view_guard);

        let mut pending = self.pending_requests.write();
        if pending.len() >= MAX_PENDING_REQUESTS {
            pending.remove(0);
        }
        pending.push(request);
        drop(pending);

        Ok(request_id)
    }

    /// Execute the pending view change.
    ///
    /// Increments the view number, selects a new primary using
    /// deterministic rotation, records the change in history, and
    /// resets pending requests.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if no view change is in progress
    /// (state must be `Requested`, `Collecting`, or `Executing`).
    /// Lock-poison errors are no longer possible under
    /// `parking_lot::RwLock`.
    #[allow(clippy::cast_possible_truncation)]
    pub fn execute_view_change(&self) -> Result<ViewChangeRecord> {
        let mut view_guard = self.current_view.write();

        // Must be in a changeable state
        match view_guard.state {
            ViewChangeState::Requested
            | ViewChangeState::Collecting
            | ViewChangeState::Executing => {}
            other => {
                return Err(Error::Validation(format!(
                    "Cannot execute view change in state: {other:?}"
                )));
            }
        }

        view_guard.state = ViewChangeState::Executing;

        let old_view = view_guard.view_number;
        let new_view = old_view + 1;
        let old_primary = view_guard.primary_id.clone();

        // Determine the reason from the first pending request
        let reason = {
            let pending = self.pending_requests.read();
            pending
                .first()
                .map_or(ViewChangeReason::ManualTrigger, |r| r.reason.clone())
        };

        // Select new primary
        let new_primary_index = Self::select_primary(new_view, self.agent_ids.len());
        let new_primary = self
            .agent_ids
            .get(new_primary_index)
            .cloned()
            .unwrap_or_else(|| "unknown".into());

        // Calculate duration
        let duration_ms = view_guard.started_at.map_or(0, |started| {
            SystemTime::now()
                .duration_since(started)
                .map_or(0, |d| d.as_millis().min(u128::from(u64::MAX)) as u64)
        });

        // Update view state
        view_guard.view_number = new_view;
        view_guard.primary_id.clone_from(&new_primary);
        view_guard.state = ViewChangeState::Idle;
        view_guard.started_at = None;
        view_guard.completed_at = Some(SystemTime::now());
        view_guard.change_count += 1;

        drop(view_guard);

        let record = ViewChangeRecord {
            from_view: old_view,
            to_view: new_view,
            reason,
            old_primary,
            new_primary,
            duration_ms,
            success: true,
            timestamp: SystemTime::now(),
        };

        // Store in history
        {
            let mut history = self.view_history.write();
            if history.len() >= MAX_VIEW_HISTORY {
                history.remove(0);
            }
            history.push(record.clone());
        }

        // Clear pending requests
        {
            let mut pending = self.pending_requests.write();
            pending.clear();
        }

        Ok(record)
    }

    /// Cancel a pending view change and reset to `Idle`.
    ///
    /// Records a failed view change in history if the state was
    /// `Requested`, `Collecting`, or `Executing`.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if no view change is in progress.
    /// Lock-poison errors are no longer possible under
    /// `parking_lot::RwLock`.
    #[allow(clippy::cast_possible_truncation)]
    pub fn cancel_view_change(&self) -> Result<()> {
        let mut view_guard = self.current_view.write();

        match view_guard.state {
            ViewChangeState::Requested
            | ViewChangeState::Collecting
            | ViewChangeState::Executing => {}
            other => {
                return Err(Error::Validation(format!(
                    "No view change in progress (state: {other:?})"
                )));
            }
        }

        // Record failed view change
        let from_view = view_guard.view_number;
        let to_view = from_view + 1;
        let old_primary = view_guard.primary_id.clone();

        let duration_ms = view_guard.started_at.map_or(0, |started| {
            SystemTime::now()
                .duration_since(started)
                .map_or(0, |d| d.as_millis().min(u128::from(u64::MAX)) as u64)
        });

        let record = ViewChangeRecord {
            from_view,
            to_view,
            reason: ViewChangeReason::ManualTrigger,
            old_primary: old_primary.clone(),
            new_primary: old_primary,
            duration_ms,
            success: false,
            timestamp: SystemTime::now(),
        };

        view_guard.state = ViewChangeState::Idle;
        view_guard.started_at = None;
        drop(view_guard);

        {
            let mut history = self.view_history.write();
            if history.len() >= MAX_VIEW_HISTORY {
                history.remove(0);
            }
            history.push(record);
        }

        // Clear pending requests
        {
            let mut pending = self.pending_requests.write();
            pending.clear();
        }

        Ok(())
    }

    /// Get a snapshot of the current view state.
    ///
    /// # Errors
    ///
    /// Never returns an error. The `Result` return type is retained for
    /// API stability; `parking_lot::RwLock` does not poison.
    pub fn view_state(&self) -> Result<ViewState> {
        Ok(self.current_view.read().clone())
    }

    /// Get the number of pending view change requests.
    #[must_use]
    pub fn pending_request_count(&self) -> usize {
        self.pending_requests.read().len()
    }

    /// Get a clone of the view change history.
    #[must_use]
    pub fn view_history(&self) -> Vec<ViewChangeRecord> {
        self.view_history.read().clone()
    }

    /// Get the total number of view changes that have been performed.
    #[must_use]
    pub fn change_count(&self) -> u64 {
        self.current_view.read().change_count
    }

    /// Calculate the timeout for a given consensus action.
    ///
    /// Applies the current timeout multiplier to the base timeout for
    /// the specified action type.
    #[must_use]
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss
    )]
    pub fn timeout_for_action(&self, action: &ConsensusAction) -> u64 {
        let base = match action {
            ConsensusAction::ServiceTermination => BASE_TIMEOUT_SERVICE_TERMINATION_MS,
            ConsensusAction::DatabaseMigration => BASE_TIMEOUT_DATABASE_MIGRATION_MS,
            ConsensusAction::CredentialRotation => BASE_TIMEOUT_CREDENTIAL_ROTATION_MS,
            ConsensusAction::CascadeRestart => BASE_TIMEOUT_CASCADE_RESTART_MS,
            ConsensusAction::ConfigRollback => BASE_TIMEOUT_CONFIG_ROLLBACK_MS,
        };
        let multiplier = *self.timeout_multiplier.read();
        let result = (base as f64) * multiplier;
        // Clamp to valid u64 range
        if result <= 0.0 {
            0
        } else if result >= u64::MAX as f64 {
            u64::MAX
        } else {
            result as u64
        }
    }

    /// Set the timeout multiplier.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if the multiplier is not positive or is non-finite.
    /// Lock-poison errors are no longer possible under
    /// `parking_lot::RwLock`.
    pub fn set_timeout_multiplier(&self, multiplier: f64) -> Result<()> {
        if multiplier <= 0.0 || !multiplier.is_finite() {
            return Err(Error::Validation(format!(
                "Timeout multiplier must be positive and finite, got: {multiplier}"
            )));
        }
        *self.timeout_multiplier.write() = multiplier;
        Ok(())
    }

    /// Get the current timeout multiplier.
    #[must_use]
    pub fn timeout_multiplier(&self) -> f64 {
        *self.timeout_multiplier.read()
    }

    /// Check whether a view change is currently in progress.
    #[must_use]
    pub fn is_view_change_in_progress(&self) -> bool {
        matches!(
            self.current_view.read().state,
            ViewChangeState::Requested
                | ViewChangeState::Collecting
                | ViewChangeState::Executing
        )
    }

    /// Calculate the success rate of view changes from history.
    ///
    /// Returns 0.0 if no view changes have been recorded.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn success_rate(&self) -> f64 {
        let history = self.view_history.read();
        if history.is_empty() {
            return 0.0;
        }
        let success_count = history.iter().filter(|r| r.success).count();
        success_count as f64 / history.len() as f64
    }

    /// Calculate the average duration (in milliseconds) of view changes
    /// from history.
    ///
    /// Returns 0.0 if no view changes have been recorded.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn average_duration_ms(&self) -> f64 {
        let history = self.view_history.read();
        if history.is_empty() {
            return 0.0;
        }
        let total: u64 = history.iter().map(|r| r.duration_ms).sum();
        total as f64 / history.len() as f64
    }

    /// Get the primary agent ID for a given view number.
    ///
    /// Uses deterministic selection from the agent fleet.
    #[must_use]
    pub fn primary_for_view(&self, view_number: u64) -> String {
        let index = Self::select_primary(view_number, self.agent_ids.len());
        self.agent_ids
            .get(index)
            .cloned()
            .unwrap_or_else(|| "unknown".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to create a fleet of `n` agents with IDs `agent-00`, `agent-01`, etc.
    fn make_fleet(n: usize) -> Vec<String> {
        (0..n).map(|i| format!("agent-{i:02}")).collect()
    }

    /// Helper to create a standard 5-agent fleet for most tests.
    fn default_fleet() -> Vec<String> {
        make_fleet(5)
    }

    // ---------------------------------------------------------------
    // Initial state tests
    // ---------------------------------------------------------------

    #[test]
    fn test_initial_view_is_zero() {
        let handler = ViewChangeHandler::new(default_fleet());
        assert_eq!(handler.current_view(), 0);
    }

    #[test]
    fn test_initial_primary_is_first_agent() {
        let handler = ViewChangeHandler::new(default_fleet());
        let primary = handler.current_primary().unwrap_or_else(|_| unreachable!());
        assert_eq!(primary, "agent-00");
    }

    #[test]
    fn test_initial_state_is_idle() {
        let handler = ViewChangeHandler::new(default_fleet());
        let state = handler.view_state().unwrap_or_else(|_| unreachable!());
        assert_eq!(state.state, ViewChangeState::Idle);
        assert!(state.started_at.is_none());
        assert!(state.completed_at.is_none());
        assert_eq!(state.change_count, 0);
    }

    #[test]
    fn test_initial_no_pending_requests() {
        let handler = ViewChangeHandler::new(default_fleet());
        assert_eq!(handler.pending_request_count(), 0);
    }

    #[test]
    fn test_initial_empty_history() {
        let handler = ViewChangeHandler::new(default_fleet());
        assert!(handler.view_history().is_empty());
    }

    // ---------------------------------------------------------------
    // Primary selection tests
    // ---------------------------------------------------------------

    #[test]
    fn test_select_primary_view_zero() {
        assert_eq!(ViewChangeHandler::select_primary(0, 5), 0);
    }

    #[test]
    fn test_select_primary_sequential() {
        assert_eq!(ViewChangeHandler::select_primary(1, 5), 1);
        assert_eq!(ViewChangeHandler::select_primary(2, 5), 2);
        assert_eq!(ViewChangeHandler::select_primary(3, 5), 3);
        assert_eq!(ViewChangeHandler::select_primary(4, 5), 4);
    }

    #[test]
    fn test_select_primary_wraps_around() {
        assert_eq!(ViewChangeHandler::select_primary(5, 5), 0);
        assert_eq!(ViewChangeHandler::select_primary(6, 5), 1);
        assert_eq!(ViewChangeHandler::select_primary(10, 5), 0);
        assert_eq!(ViewChangeHandler::select_primary(13, 5), 3);
    }

    #[test]
    fn test_select_primary_zero_agents() {
        assert_eq!(ViewChangeHandler::select_primary(0, 0), 0);
        assert_eq!(ViewChangeHandler::select_primary(99, 0), 0);
    }

    #[test]
    fn test_select_primary_single_agent() {
        assert_eq!(ViewChangeHandler::select_primary(0, 1), 0);
        assert_eq!(ViewChangeHandler::select_primary(1, 1), 0);
        assert_eq!(ViewChangeHandler::select_primary(100, 1), 0);
    }

    #[test]
    fn test_select_primary_large_view_number() {
        let result = ViewChangeHandler::select_primary(1_000_000, 40);
        assert!(result < 40);
        assert_eq!(result, 0); // 1_000_000 % 40 == 0
    }

    // ---------------------------------------------------------------
    // View change request tests
    // ---------------------------------------------------------------

    #[test]
    fn test_request_view_change_success() {
        let handler = ViewChangeHandler::new(default_fleet());
        let result = handler.request_view_change(ViewChangeReason::ProposalTimeout, "agent-01");
        assert!(result.is_ok());
        let id = result.unwrap_or_else(|_| unreachable!());
        assert!(id.contains("vcr-0-1"));
    }

    #[test]
    fn test_request_sets_state_to_requested() {
        let handler = ViewChangeHandler::new(default_fleet());
        let _ = handler.request_view_change(ViewChangeReason::PrepareTimeout, "agent-02");
        assert!(handler.is_view_change_in_progress());
        let state = handler.view_state().unwrap_or_else(|_| unreachable!());
        assert_eq!(state.state, ViewChangeState::Requested);
        assert!(state.started_at.is_some());
    }

    #[test]
    fn test_request_increments_pending_count() {
        let handler = ViewChangeHandler::new(default_fleet());
        let _ = handler.request_view_change(ViewChangeReason::CommitTimeout, "agent-01");
        assert_eq!(handler.pending_request_count(), 1);
        let _ = handler.request_view_change(ViewChangeReason::QuorumFailure, "agent-02");
        assert_eq!(handler.pending_request_count(), 2);
    }

    #[test]
    fn test_request_empty_requester_fails() {
        let handler = ViewChangeHandler::new(default_fleet());
        let result = handler.request_view_change(ViewChangeReason::ManualTrigger, "");
        assert!(result.is_err());
    }

    // ---------------------------------------------------------------
    // View change execution tests
    // ---------------------------------------------------------------

    #[test]
    fn test_execute_view_change_success() {
        let handler = ViewChangeHandler::new(default_fleet());
        let _ = handler.request_view_change(ViewChangeReason::ProposalTimeout, "agent-01");
        let result = handler.execute_view_change();
        assert!(result.is_ok());
        let record = result.unwrap_or_else(|_| unreachable!());
        assert_eq!(record.from_view, 0);
        assert_eq!(record.to_view, 1);
        assert!(record.success);
        assert_eq!(record.old_primary, "agent-00");
        assert_eq!(record.new_primary, "agent-01");
    }

    #[test]
    fn test_execute_increments_view() {
        let handler = ViewChangeHandler::new(default_fleet());
        let _ = handler.request_view_change(ViewChangeReason::ProposalTimeout, "agent-01");
        let _ = handler.execute_view_change();
        assert_eq!(handler.current_view(), 1);
    }

    #[test]
    fn test_execute_updates_primary() {
        let handler = ViewChangeHandler::new(default_fleet());
        let _ = handler.request_view_change(ViewChangeReason::ProposalTimeout, "agent-01");
        let _ = handler.execute_view_change();
        let primary = handler.current_primary().unwrap_or_else(|_| unreachable!());
        assert_eq!(primary, "agent-01");
    }

    #[test]
    fn test_execute_resets_state_to_idle() {
        let handler = ViewChangeHandler::new(default_fleet());
        let _ = handler.request_view_change(ViewChangeReason::ProposalTimeout, "agent-01");
        let _ = handler.execute_view_change();
        assert!(!handler.is_view_change_in_progress());
        let state = handler.view_state().unwrap_or_else(|_| unreachable!());
        assert_eq!(state.state, ViewChangeState::Idle);
    }

    #[test]
    fn test_execute_clears_pending_requests() {
        let handler = ViewChangeHandler::new(default_fleet());
        let _ = handler.request_view_change(ViewChangeReason::ProposalTimeout, "agent-01");
        let _ = handler.request_view_change(ViewChangeReason::PrepareTimeout, "agent-02");
        let _ = handler.execute_view_change();
        assert_eq!(handler.pending_request_count(), 0);
    }

    #[test]
    fn test_execute_without_request_fails() {
        let handler = ViewChangeHandler::new(default_fleet());
        let result = handler.execute_view_change();
        assert!(result.is_err());
    }

    #[test]
    fn test_execute_records_in_history() {
        let handler = ViewChangeHandler::new(default_fleet());
        let _ = handler.request_view_change(ViewChangeReason::ProposalTimeout, "agent-01");
        let _ = handler.execute_view_change();
        let history = handler.view_history();
        assert_eq!(history.len(), 1);
        assert!(history[0].success);
    }

    #[test]
    fn test_execute_increments_change_count() {
        let handler = ViewChangeHandler::new(default_fleet());
        let _ = handler.request_view_change(ViewChangeReason::ProposalTimeout, "agent-01");
        let _ = handler.execute_view_change();
        assert_eq!(handler.change_count(), 1);
    }

    // ---------------------------------------------------------------
    // Cancel view change tests
    // ---------------------------------------------------------------

    #[test]
    fn test_cancel_view_change_success() {
        let handler = ViewChangeHandler::new(default_fleet());
        let _ = handler.request_view_change(ViewChangeReason::QuorumFailure, "agent-01");
        let result = handler.cancel_view_change();
        assert!(result.is_ok());
        assert!(!handler.is_view_change_in_progress());
    }

    #[test]
    fn test_cancel_records_failed_in_history() {
        let handler = ViewChangeHandler::new(default_fleet());
        let _ = handler.request_view_change(ViewChangeReason::QuorumFailure, "agent-01");
        let _ = handler.cancel_view_change();
        let history = handler.view_history();
        assert_eq!(history.len(), 1);
        assert!(!history[0].success);
    }

    #[test]
    fn test_cancel_preserves_view_number() {
        let handler = ViewChangeHandler::new(default_fleet());
        let _ = handler.request_view_change(ViewChangeReason::QuorumFailure, "agent-01");
        let _ = handler.cancel_view_change();
        assert_eq!(handler.current_view(), 0);
    }

    #[test]
    fn test_cancel_clears_pending() {
        let handler = ViewChangeHandler::new(default_fleet());
        let _ = handler.request_view_change(ViewChangeReason::QuorumFailure, "agent-01");
        let _ = handler.cancel_view_change();
        assert_eq!(handler.pending_request_count(), 0);
    }

    #[test]
    fn test_cancel_without_request_fails() {
        let handler = ViewChangeHandler::new(default_fleet());
        let result = handler.cancel_view_change();
        assert!(result.is_err());
    }

    // ---------------------------------------------------------------
    // Multiple consecutive view change tests
    // ---------------------------------------------------------------

    #[test]
    fn test_multiple_consecutive_view_changes() {
        let handler = ViewChangeHandler::new(default_fleet());

        for i in 0u64..5 {
            let _ = handler.request_view_change(ViewChangeReason::ProposalTimeout, "agent-01");
            let record = handler.execute_view_change().unwrap_or_else(|_| unreachable!());
            assert_eq!(record.from_view, i);
            assert_eq!(record.to_view, i + 1);
        }

        assert_eq!(handler.current_view(), 5);
        assert_eq!(handler.change_count(), 5);
    }

    #[test]
    fn test_primary_rotates_through_fleet() {
        let fleet = make_fleet(3);
        let handler = ViewChangeHandler::new(fleet);

        // View 0: agent-00 (initial)
        assert_eq!(
            handler.current_primary().unwrap_or_else(|_| unreachable!()),
            "agent-00"
        );

        // View 1: agent-01
        let _ = handler.request_view_change(ViewChangeReason::ProposalTimeout, "agent-00");
        let _ = handler.execute_view_change();
        assert_eq!(
            handler.current_primary().unwrap_or_else(|_| unreachable!()),
            "agent-01"
        );

        // View 2: agent-02
        let _ = handler.request_view_change(ViewChangeReason::ProposalTimeout, "agent-01");
        let _ = handler.execute_view_change();
        assert_eq!(
            handler.current_primary().unwrap_or_else(|_| unreachable!()),
            "agent-02"
        );

        // View 3: agent-00 (wraps around)
        let _ = handler.request_view_change(ViewChangeReason::ProposalTimeout, "agent-02");
        let _ = handler.execute_view_change();
        assert_eq!(
            handler.current_primary().unwrap_or_else(|_| unreachable!()),
            "agent-00"
        );
    }

    #[test]
    fn test_history_grows_with_changes() {
        let handler = ViewChangeHandler::new(default_fleet());
        for _ in 0..3 {
            let _ = handler.request_view_change(ViewChangeReason::ProposalTimeout, "agent-01");
            let _ = handler.execute_view_change();
        }
        assert_eq!(handler.view_history().len(), 3);
    }

    // ---------------------------------------------------------------
    // Timeout calculation tests
    // ---------------------------------------------------------------

    #[test]
    fn test_timeout_service_termination() {
        let handler = ViewChangeHandler::new(default_fleet());
        let timeout = handler.timeout_for_action(&ConsensusAction::ServiceTermination);
        assert_eq!(timeout, 60_000);
    }

    #[test]
    fn test_timeout_database_migration() {
        let handler = ViewChangeHandler::new(default_fleet());
        let timeout = handler.timeout_for_action(&ConsensusAction::DatabaseMigration);
        assert_eq!(timeout, 300_000);
    }

    #[test]
    fn test_timeout_with_multiplier() {
        let handler = ViewChangeHandler::new(default_fleet());
        let _ = handler.set_timeout_multiplier(2.0);
        let timeout = handler.timeout_for_action(&ConsensusAction::ServiceTermination);
        assert_eq!(timeout, 120_000);
    }

    #[test]
    fn test_set_timeout_multiplier_success() {
        let handler = ViewChangeHandler::new(default_fleet());
        let result = handler.set_timeout_multiplier(1.5);
        assert!(result.is_ok());
        assert!((handler.timeout_multiplier() - 1.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_set_timeout_multiplier_zero_fails() {
        let handler = ViewChangeHandler::new(default_fleet());
        let result = handler.set_timeout_multiplier(0.0);
        assert!(result.is_err());
    }

    #[test]
    fn test_set_timeout_multiplier_negative_fails() {
        let handler = ViewChangeHandler::new(default_fleet());
        let result = handler.set_timeout_multiplier(-1.0);
        assert!(result.is_err());
    }

    #[test]
    fn test_set_timeout_multiplier_nan_fails() {
        let handler = ViewChangeHandler::new(default_fleet());
        let result = handler.set_timeout_multiplier(f64::NAN);
        assert!(result.is_err());
    }

    #[test]
    fn test_set_timeout_multiplier_infinity_fails() {
        let handler = ViewChangeHandler::new(default_fleet());
        let result = handler.set_timeout_multiplier(f64::INFINITY);
        assert!(result.is_err());
    }

    #[test]
    fn test_default_timeout_multiplier() {
        let handler = ViewChangeHandler::new(default_fleet());
        assert!((handler.timeout_multiplier() - 1.0).abs() < f64::EPSILON);
    }

    // ---------------------------------------------------------------
    // Success rate and duration tracking tests
    // ---------------------------------------------------------------

    #[test]
    fn test_success_rate_no_history() {
        let handler = ViewChangeHandler::new(default_fleet());
        assert!((handler.success_rate() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_success_rate_all_successful() {
        let handler = ViewChangeHandler::new(default_fleet());
        for _ in 0..3 {
            let _ = handler.request_view_change(ViewChangeReason::ProposalTimeout, "agent-01");
            let _ = handler.execute_view_change();
        }
        assert!((handler.success_rate() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_success_rate_mixed() {
        let handler = ViewChangeHandler::new(default_fleet());
        // 1 success
        let _ = handler.request_view_change(ViewChangeReason::ProposalTimeout, "agent-01");
        let _ = handler.execute_view_change();
        // 1 failure (cancel)
        let _ = handler.request_view_change(ViewChangeReason::QuorumFailure, "agent-01");
        let _ = handler.cancel_view_change();
        // success rate = 1/2 = 0.5
        assert!((handler.success_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_average_duration_no_history() {
        let handler = ViewChangeHandler::new(default_fleet());
        assert!((handler.average_duration_ms() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_average_duration_with_history() {
        let handler = ViewChangeHandler::new(default_fleet());
        let _ = handler.request_view_change(ViewChangeReason::ProposalTimeout, "agent-01");
        let _ = handler.execute_view_change();
        // Duration should be >= 0 (near-instant in tests)
        let avg = handler.average_duration_ms();
        assert!(avg >= 0.0);
    }

    // ---------------------------------------------------------------
    // `primary_for_view` tests
    // ---------------------------------------------------------------

    #[test]
    fn test_primary_for_view_zero() {
        let handler = ViewChangeHandler::new(default_fleet());
        assert_eq!(handler.primary_for_view(0), "agent-00");
    }

    #[test]
    fn test_primary_for_view_wraps() {
        let handler = ViewChangeHandler::new(make_fleet(3));
        assert_eq!(handler.primary_for_view(0), "agent-00");
        assert_eq!(handler.primary_for_view(1), "agent-01");
        assert_eq!(handler.primary_for_view(2), "agent-02");
        assert_eq!(handler.primary_for_view(3), "agent-00");
        assert_eq!(handler.primary_for_view(4), "agent-01");
    }

    // ---------------------------------------------------------------
    // Edge case tests
    // ---------------------------------------------------------------

    #[test]
    fn test_empty_fleet_initial_primary() {
        let handler = ViewChangeHandler::new(Vec::new());
        let primary = handler.current_primary().unwrap_or_else(|_| unreachable!());
        assert_eq!(primary, "unknown");
    }

    #[test]
    fn test_view_change_reason_preserved_in_record() {
        let handler = ViewChangeHandler::new(default_fleet());
        let _ =
            handler.request_view_change(ViewChangeReason::EnhancedConsensusFail, "agent-01");
        let record = handler.execute_view_change().unwrap_or_else(|_| unreachable!());
        assert_eq!(record.reason, ViewChangeReason::EnhancedConsensusFail);
    }

    #[test]
    fn test_view_state_completed_at_after_change() {
        let handler = ViewChangeHandler::new(default_fleet());
        let _ = handler.request_view_change(ViewChangeReason::ProposalTimeout, "agent-01");
        let _ = handler.execute_view_change();
        let state = handler.view_state().unwrap_or_else(|_| unreachable!());
        assert!(state.completed_at.is_some());
    }

    // ---------------------------------------------------------------
    // F-06: parking_lot::RwLock migration — poisoning-resistance tests
    //
    // Regression guard for Session 099 bug-hunt finding F-06:
    // Prior to the fix, view_change.rs used std::sync::RwLock and read
    // accessors swallowed PoisonError via .map_or(default). A single
    // panic while holding a write lock would permanently corrupt the
    // PBFT audit trail — `pending_request_count` would return 0 forever,
    // `view_history` would return empty, `success_rate` would return 0.0.
    //
    // parking_lot::RwLock does not implement lock poisoning, so a panic
    // holding the write lock releases the lock cleanly on unwind and
    // subsequent readers observe the state as it was at panic time.
    // ---------------------------------------------------------------

    #[test]
    fn f06_reads_survive_writer_panic() {
        // Seed history with one completed view change so readers have data
        // to return. Then spawn a thread that grabs the write guard and
        // panics; parking_lot releases the lock on unwind (no poisoning).
        use std::sync::Arc;
        use std::thread;

        let handler = Arc::new(ViewChangeHandler::new(default_fleet()));
        let _ = handler.request_view_change(ViewChangeReason::ProposalTimeout, "agent-01");
        let _ = handler.execute_view_change();
        assert_eq!(handler.view_history().len(), 1);
        assert!((handler.success_rate() - 1.0).abs() < f64::EPSILON);

        // Spawn a thread that panics mid-write. Catch the panic so the
        // test process survives; the lock is released on unwind.
        let panicker = {
            let h = Arc::clone(&handler);
            thread::spawn(move || {
                // This grabs the write lock, simulates mutation work,
                // then panics. With std::sync::RwLock this would poison
                // the lock; with parking_lot it releases cleanly.
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let mut guard = h.current_view.write();
                    guard.change_count += 100; // partial mutation visible post-panic
                    panic!("simulated writer panic");
                }));
                assert!(result.is_err(), "expected inner panic");
            })
        };
        panicker
            .join()
            .unwrap_or_else(|_| unreachable!("outer thread should not panic"));

        // Readers must still work AND observe the partial mutation from the
        // panicking writer (parking_lot does not roll back, it just releases).
        // This asserts the PBFT audit history survives a writer panic.
        assert_eq!(handler.view_history().len(), 1, "history preserved");
        assert!(
            (handler.success_rate() - 1.0).abs() < f64::EPSILON,
            "success_rate still accurate post-panic"
        );
        assert_eq!(
            handler.pending_request_count(),
            0,
            "pending count accurate post-panic"
        );
        // change_count reflects both the legitimate execute_view_change (+1)
        // and the panicking writer's partial mutation (+100).
        assert_eq!(handler.change_count(), 101, "partial write observable");
        // Subsequent writes continue to work — lock is not poisoned.
        assert!(
            handler
                .request_view_change(ViewChangeReason::QuorumFailure, "agent-02")
                .is_ok(),
            "writes succeed after poison-free recovery"
        );
    }

    #[test]
    fn f06_concurrent_readers_during_writer_panic_do_not_deadlock() {
        // Ensure that a panicking writer does not leave the lock in a state
        // that blocks concurrent readers. Spawns 4 reader threads and 1
        // panicking writer; all reads complete successfully.
        use std::sync::Arc;
        use std::thread;
        use std::time::Duration;

        let handler = Arc::new(ViewChangeHandler::new(default_fleet()));

        let mut reader_handles = Vec::with_capacity(4);
        for _ in 0..4 {
            let h = Arc::clone(&handler);
            reader_handles.push(thread::spawn(move || {
                // Spin on reads for a short window.
                let start = std::time::Instant::now();
                let mut reads = 0u32;
                while start.elapsed() < Duration::from_millis(50) {
                    let _ = h.pending_request_count();
                    let _ = h.view_history();
                    let _ = h.success_rate();
                    let _ = h.current_view();
                    reads += 1;
                }
                reads
            }));
        }

        // Writer that panics mid-way.
        let writer = {
            let h = Arc::clone(&handler);
            thread::spawn(move || {
                let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let _guard = h.current_view.write();
                    // Short hold then panic.
                    std::thread::sleep(Duration::from_millis(5));
                    panic!("mid-write panic");
                }));
            })
        };

        writer.join().unwrap_or_else(|_| unreachable!());
        for h in reader_handles {
            let reads = h.join().unwrap_or_else(|_| unreachable!());
            assert!(reads > 0, "reader made progress despite writer panic");
        }

        // Lock is still usable.
        assert_eq!(handler.current_view(), 0);
    }

    #[test]
    fn f06_parking_lot_does_not_poison_on_simple_panic() {
        // Minimal regression guard: a single in-thread panic inside a
        // write-lock scope does NOT poison the lock, and subsequent writes
        // see the pre-panic state plus any partial mutation.
        let handler = ViewChangeHandler::new(default_fleet());

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut guard = handler.current_view.write();
            guard.change_count = 42;
            panic!("oops");
        }));
        assert!(result.is_err(), "inner panic caught");

        // Lock is released (parking_lot), and change_count reflects the
        // write that occurred before the panic.
        assert_eq!(handler.change_count(), 42);
        // New writes proceed normally.
        let _ = handler.request_view_change(ViewChangeReason::ProposalTimeout, "agent-01");
        assert_eq!(handler.pending_request_count(), 1);
    }
}
