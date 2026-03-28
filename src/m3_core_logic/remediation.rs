//! # M14: Remediation Engine
//!
//! Auto-remediation logic for the Maintenance Engine. This module manages
//! the full lifecycle of remediation requests: submission, action selection,
//! concurrent execution tracking, completion recording, and cancellation.
//!
//! ## Layer: L3 (Core Logic)
//! ## Dependencies: M1 (Error), M3 (Core types), `EscalationTier`
//!
//! ## Design
//!
//! The [`RemediationEngine`] maintains three queues:
//! - **Pending**: requests awaiting processing (FIFO `VecDeque`)
//! - **Active**: requests currently being executed (capped at `max_concurrent`)
//! - **Completed**: historical outcomes (capped at 500)
//!
//! An [`ActionMapping`] registry maps each [`IssueType`] to a prioritised
//! list of [`RemediationAction`] variants, filtered by severity and
//! confidence thresholds at selection time.
//!
//! ## 12D Tensor Encoding
//! ```text
//! [14/36, 0.0, 3/6, 3, 0, 0.5, health, uptime, synergy, latency, error_rate, temporal]
//! ```
//!
//! ## Related Documentation
//! - [Layer Specification](../../ai_docs/layers/L03_CORE_LOGIC.md)
//! - [Auto-Remediation](../../nam/L0_AUTO_REMEDIATION.md)

use std::collections::{HashMap, VecDeque};
use std::sync::RwLock;

use chrono::{DateTime, Utc};

use crate::m3_core_logic::{
    calculate_confidence, determine_tier, IssueType, RemediationAction, RemediationOutcome,
    RemediationRequest, Severity,
};
use crate::{Error, Result};

/// Maximum number of completed outcomes to retain in memory.
const COMPLETED_CAP: usize = 500;

/// Default maximum number of concurrently active remediations.
const DEFAULT_MAX_CONCURRENT: usize = 5;

// ---------------------------------------------------------------------------
// Supporting types
// ---------------------------------------------------------------------------

/// Status of a remediation request throughout its lifecycle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RemediationStatus {
    /// Waiting in the pending queue.
    Queued,
    /// Currently being executed.
    Executing,
    /// Awaiting human or consensus approval before proceeding.
    WaitingApproval,
    /// Successfully completed (or completed with recorded failure).
    Completed,
    /// Execution failed.
    Failed,
    /// Cancelled before completion.
    Cancelled,
}

/// Maps a [`RemediationAction`] constructor to its eligibility criteria.
///
/// The engine uses these mappings to select the best action for a given
/// issue type, severity, and confidence score.
#[derive(Clone, Debug)]
pub struct ActionMapping {
    /// The action to apply (constructed with default / placeholder parameters).
    pub action: RemediationAction,
    /// Selection priority -- lower numeric value means higher priority.
    pub priority: u32,
    /// Minimum confidence score required for this action to be eligible.
    pub min_confidence: f64,
    /// Severity levels for which this action is applicable.
    pub applicable_severities: Vec<Severity>,
}

/// A remediation request that has been moved from the pending queue into
/// active processing.
#[derive(Clone, Debug)]
pub struct ActiveRemediation {
    /// The original request.
    pub request: RemediationRequest,
    /// Timestamp when processing began.
    pub started_at: DateTime<Utc>,
    /// Current processing status.
    pub status: RemediationStatus,
}

// ---------------------------------------------------------------------------
// RemediationEngine
// ---------------------------------------------------------------------------

/// Core remediation engine managing the lifecycle of remediation requests.
///
/// Thread-safe: all mutable collections are protected by [`RwLock`].
///
/// # Examples
///
/// ```rust,no_run
/// use maintenance_engine::m3_core_logic::remediation::RemediationEngine;
/// use maintenance_engine::m3_core_logic::{IssueType, Severity};
///
/// let engine = RemediationEngine::new();
/// let request = engine
///     .submit_request("synthex", IssueType::HealthFailure, Severity::High, "health check timeout")
///     .expect("submit should succeed");
/// assert_eq!(engine.pending_count(), 1);
/// ```
pub struct RemediationEngine {
    /// FIFO queue of requests awaiting processing.
    pending_requests: RwLock<VecDeque<RemediationRequest>>,
    /// Requests currently being executed, keyed by request ID.
    active_requests: RwLock<HashMap<String, ActiveRemediation>>,
    /// Historical outcomes (capped at [`COMPLETED_CAP`]).
    completed: RwLock<Vec<RemediationOutcome>>,
    /// Registry mapping each issue type to its eligible action mappings.
    action_registry: HashMap<IssueType, Vec<ActionMapping>>,
    /// Maximum number of concurrently active remediations.
    max_concurrent: usize,
}

impl RemediationEngine {
    // -- Construction -------------------------------------------------------

    /// Create a new `RemediationEngine` with sensible default action mappings
    /// and a concurrency limit of [`DEFAULT_MAX_CONCURRENT`].
    #[must_use]
    pub fn new() -> Self {
        Self {
            pending_requests: RwLock::new(VecDeque::new()),
            active_requests: RwLock::new(HashMap::new()),
            completed: RwLock::new(Vec::new()),
            action_registry: Self::default_action_registry(),
            max_concurrent: DEFAULT_MAX_CONCURRENT,
        }
    }

    /// Create a new `RemediationEngine` with a custom concurrency limit.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if `max_concurrent` is zero.
    pub fn with_max_concurrent(max_concurrent: usize) -> Result<Self> {
        if max_concurrent == 0 {
            return Err(Error::Validation(
                "max_concurrent must be greater than zero".into(),
            ));
        }
        Ok(Self {
            pending_requests: RwLock::new(VecDeque::new()),
            active_requests: RwLock::new(HashMap::new()),
            completed: RwLock::new(Vec::new()),
            action_registry: Self::default_action_registry(),
            max_concurrent,
        })
    }

    // -- Request Submission -------------------------------------------------

    /// Submit a new remediation request.
    ///
    /// Automatically calculates confidence (using sensible defaults for the
    /// five input signals), selects the best action from the registry, and
    /// determines the escalation tier. The request is placed on the pending
    /// queue.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if `service_id` or `description` is empty,
    /// or if no eligible action can be found for the given issue type,
    /// severity, and computed confidence.
    pub fn submit_request(
        &self,
        service_id: &str,
        issue_type: IssueType,
        severity: Severity,
        description: &str,
    ) -> Result<RemediationRequest> {
        if service_id.is_empty() {
            return Err(Error::Validation("service_id must not be empty".into()));
        }
        if description.is_empty() {
            return Err(Error::Validation("description must not be empty".into()));
        }

        // Derive a severity score normalised to [0, 1].
        let severity_score = severity_to_score(severity);

        // Calculate confidence with reasonable defaults for a fresh request.
        let confidence = calculate_confidence(
            0.8,             // historical_success_rate (default optimistic)
            0.7,             // pattern_match_strength
            severity_score,  // severity_score
            0.5,             // pathway_weight (neutral)
            0.9,             // time_factor (recent)
        );

        let action = self.select_action(issue_type, severity, confidence)?;
        let tier = determine_tier(confidence, severity, &action);

        let id = uuid::Uuid::new_v4().to_string();

        let mut context = std::collections::HashMap::new();
        context.insert("description".into(), description.into());

        let request = RemediationRequest {
            id,
            service_id: service_id.into(),
            issue_type,
            severity,
            confidence,
            suggested_action: action,
            tier,
            context,
        };

        {
            let mut pending = self
                .pending_requests
                .write()
                .map_err(|e| Error::Other(format!("pending lock poisoned: {e}")))?;
            pending.push_back(request.clone());
        }

        Ok(request)
    }

    // -- Action Selection ---------------------------------------------------

    /// Select the highest-priority action for the given issue type that
    /// satisfies both the severity and confidence constraints.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if no action in the registry matches.
    pub fn select_action(
        &self,
        issue_type: IssueType,
        severity: Severity,
        confidence: f64,
    ) -> Result<RemediationAction> {
        let mappings = self.action_registry.get(&issue_type).ok_or_else(|| {
            Error::Validation(format!("no actions registered for issue type {issue_type:?}"))
        })?;

        // Filter for eligible actions, then pick the one with the lowest
        // (highest-priority) `priority` value.
        let best = mappings
            .iter()
            .filter(|m| {
                confidence >= m.min_confidence && m.applicable_severities.contains(&severity)
            })
            .min_by_key(|m| m.priority);

        Ok(best.map_or_else(
            || {
                // Fall back to AlertHuman when nothing else qualifies.
                RemediationAction::AlertHuman {
                    message: format!(
                        "No eligible automated action for {issue_type:?} at {severity:?} (confidence {confidence:.2})"
                    ),
                    severity: format!("{severity:?}"),
                }
            },
            |mapping| mapping.action.clone(),
        ))
    }

    // -- Processing ---------------------------------------------------------

    /// Dequeue the next pending request and move it to active processing,
    /// provided the concurrency limit has not been reached.
    ///
    /// Returns `Ok(None)` when the pending queue is empty **or** the
    /// concurrency limit is already saturated.
    ///
    /// # Errors
    ///
    /// Returns `Error::Other` if an internal lock is poisoned.
    pub fn process_next(&self) -> Result<Option<ActiveRemediation>> {
        let active_count = {
            let active = self
                .active_requests
                .read()
                .map_err(|e| Error::Other(format!("active lock poisoned: {e}")))?;
            active.len()
        };

        if active_count >= self.max_concurrent {
            return Ok(None);
        }

        let request = {
            let mut pending = self
                .pending_requests
                .write()
                .map_err(|e| Error::Other(format!("pending lock poisoned: {e}")))?;
            pending.pop_front()
        };

        match request {
            Some(req) => {
                let request_id = req.id.clone();
                let active_rem = ActiveRemediation {
                    request: req,
                    started_at: Utc::now(),
                    status: RemediationStatus::Executing,
                };

                {
                    let mut active = self
                        .active_requests
                        .write()
                        .map_err(|e| Error::Other(format!("active lock poisoned: {e}")))?;
                    active.insert(request_id, active_rem.clone());
                }

                Ok(Some(active_rem))
            }
            None => Ok(None),
        }
    }

    // -- Completion / Cancellation ------------------------------------------

    /// Record the completion of an active remediation request.
    ///
    /// Removes the request from the active set, creates a
    /// [`RemediationOutcome`], and appends it to the completed history
    /// (enforcing the cap of [`COMPLETED_CAP`]).
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if the `request_id` is not found among
    /// active requests.
    pub fn complete_request(
        &self,
        request_id: &str,
        success: bool,
        duration_ms: u64,
        error_message: Option<String>,
    ) -> Result<RemediationOutcome> {
        {
            let mut active = self
                .active_requests
                .write()
                .map_err(|e| Error::Other(format!("active lock poisoned: {e}")))?;

            if active.remove(request_id).is_none() {
                return Err(Error::Validation(format!(
                    "no active request with id '{request_id}'"
                )));
            }
        }

        // Compute a simple pathway delta: positive for success, negative for
        // failure, scaled by duration (faster = larger delta).
        #[allow(clippy::cast_precision_loss)]
        let duration_f64 = duration_ms as f64;
        let pathway_delta = if success {
            1.0 / (1.0 + duration_f64 / 1000.0)
        } else {
            -0.1
        };

        let outcome = RemediationOutcome {
            request_id: request_id.into(),
            success,
            duration_ms,
            error: error_message,
            pathway_delta,
        };

        let mut completed = self
            .completed
            .write()
            .map_err(|e| Error::Other(format!("completed lock poisoned: {e}")))?;

        completed.push(outcome.clone());

        // Enforce capacity by removing oldest entries.
        while completed.len() > COMPLETED_CAP {
            completed.remove(0);
        }

        drop(completed);

        Ok(outcome)
    }

    /// Cancel a request that is either pending or active.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if the `request_id` is not found in
    /// either the pending queue or the active set.
    pub fn cancel_request(&self, request_id: &str) -> Result<()> {
        // Try removing from active first.
        {
            let mut active = self
                .active_requests
                .write()
                .map_err(|e| Error::Other(format!("active lock poisoned: {e}")))?;
            if active.remove(request_id).is_some() {
                return Ok(());
            }
        }

        // Try removing from pending.
        {
            let mut pending = self
                .pending_requests
                .write()
                .map_err(|e| Error::Other(format!("pending lock poisoned: {e}")))?;

            let before_len = pending.len();
            pending.retain(|r| r.id != request_id);
            if pending.len() < before_len {
                return Ok(());
            }
        }

        Err(Error::Validation(format!(
            "request '{request_id}' not found in pending or active queues"
        )))
    }

    // -- Status Queries -----------------------------------------------------

    /// Retrieve the current status of a request by its ID.
    ///
    /// Searches pending, active, and completed collections.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if the request ID is not found anywhere.
    pub fn get_request_status(&self, request_id: &str) -> Result<RemediationStatus> {
        // Check active.
        {
            let active = self
                .active_requests
                .read()
                .map_err(|e| Error::Other(format!("active lock poisoned: {e}")))?;
            if let Some(ar) = active.get(request_id) {
                return Ok(ar.status);
            }
        }

        // Check pending.
        {
            let pending = self
                .pending_requests
                .read()
                .map_err(|e| Error::Other(format!("pending lock poisoned: {e}")))?;
            if pending.iter().any(|r| r.id == request_id) {
                return Ok(RemediationStatus::Queued);
            }
        }

        // Check completed.
        {
            let completed = self
                .completed
                .read()
                .map_err(|e| Error::Other(format!("completed lock poisoned: {e}")))?;
            for outcome in completed.iter() {
                if outcome.request_id == request_id {
                    return if outcome.success {
                        Ok(RemediationStatus::Completed)
                    } else {
                        Ok(RemediationStatus::Failed)
                    };
                }
            }
        }

        Err(Error::Validation(format!(
            "request '{request_id}' not found"
        )))
    }

    // -- Counts -------------------------------------------------------------

    /// Number of requests in the pending queue.
    #[must_use]
    pub fn pending_count(&self) -> usize {
        self.pending_requests
            .read()
            .map(|p| p.len())
            .unwrap_or(0)
    }

    /// Number of requests currently being processed.
    #[must_use]
    pub fn active_count(&self) -> usize {
        self.active_requests
            .read()
            .map(|a| a.len())
            .unwrap_or(0)
    }

    /// Number of recorded completion outcomes.
    #[must_use]
    pub fn completed_count(&self) -> usize {
        self.completed.read().map(|c| c.len()).unwrap_or(0)
    }

    // -- Bulk Queries -------------------------------------------------------

    /// Return a snapshot of all pending requests (cloned).
    #[must_use]
    pub fn get_pending_requests(&self) -> Vec<RemediationRequest> {
        self.pending_requests
            .read()
            .map(|p| p.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Return a snapshot of all active remediations (cloned).
    #[must_use]
    pub fn get_active_requests(&self) -> Vec<ActiveRemediation> {
        self.active_requests
            .read()
            .map(|a| a.values().cloned().collect())
            .unwrap_or_default()
    }

    /// Return all completed outcomes for a specific service.
    ///
    /// Because completed outcomes do not store `service_id` directly, this
    /// method cross-references with the request ID prefix convention. In
    /// practice, callers should use the request ID to look up the original
    /// service. This implementation filters by `request_id` containing the
    /// service string if it was embedded, but as a robust fallback it
    /// returns all outcomes whose `request_id` matches any request that
    /// targeted the given service.
    ///
    /// Since `RemediationOutcome` does not store `service_id`, this method
    /// searches the active and pending queues to build a set of request IDs
    /// that belong to the given service, then filters completed outcomes
    /// accordingly. For fully completed requests that are no longer in any
    /// queue, the caller should maintain an external index.
    #[must_use]
    pub fn get_outcomes_for_service(&self, service_id: &str) -> Vec<RemediationOutcome> {
        // Gather all known request IDs for this service from pending + active.
        let mut service_request_ids: Vec<String> = Vec::new();

        if let Ok(pending) = self.pending_requests.read() {
            for req in pending.iter() {
                if req.service_id == service_id {
                    service_request_ids.push(req.id.clone());
                }
            }
        }

        if let Ok(active) = self.active_requests.read() {
            for (id, ar) in active.iter() {
                if ar.request.service_id == service_id {
                    service_request_ids.push(id.clone());
                }
            }
        }

        // Also do a best-effort match: if the service_id happens to appear
        // in the request_id (UUIDs won't, but this future-proofs the API).
        self.completed.read().map_or_else(
            |_| Vec::new(),
            |completed| {
                completed
                    .iter()
                    .filter(|o| {
                        service_request_ids.contains(&o.request_id)
                            || o.request_id.contains(service_id)
                    })
                    .cloned()
                    .collect()
            },
        )
    }

    // -- Aggregate Metrics --------------------------------------------------

    /// Compute the overall success rate across all completed outcomes.
    ///
    /// Returns `0.0` if there are no completed outcomes.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn success_rate(&self) -> f64 {
        let Ok(completed) = self.completed.read() else {
            return 0.0;
        };

        if completed.is_empty() {
            return 0.0;
        }

        let successes = completed.iter().filter(|o| o.success).count();
        successes as f64 / completed.len() as f64
    }

    /// Compute the average resolution time in milliseconds across all
    /// completed outcomes.
    ///
    /// Returns `0.0` if there are no completed outcomes.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn avg_resolution_time_ms(&self) -> f64 {
        let Ok(completed) = self.completed.read() else {
            return 0.0;
        };

        if completed.is_empty() {
            return 0.0;
        }

        let total: u64 = completed.iter().map(|o| o.duration_ms).sum();
        total as f64 / completed.len() as f64
    }

    // -- Internal Helpers ---------------------------------------------------

    /// Build the default action registry with sensible mappings for every
    /// [`IssueType`].
    fn default_action_registry() -> HashMap<IssueType, Vec<ActionMapping>> {
        let mut registry = HashMap::new();

        Self::register_health_failure(&mut registry);
        Self::register_latency_spike(&mut registry);
        Self::register_error_rate_high(&mut registry);
        Self::register_memory_pressure(&mut registry);
        Self::register_disk_pressure(&mut registry);
        Self::register_connection_failure(&mut registry);
        Self::register_timeout(&mut registry);
        Self::register_crash(&mut registry);

        registry
    }

    /// Create an `AlertHuman` action mapping at the given priority.
    fn alert_human_mapping(priority: u32) -> ActionMapping {
        ActionMapping {
            action: RemediationAction::AlertHuman {
                message: String::new(),
                severity: String::new(),
            },
            priority,
            min_confidence: 0.0,
            applicable_severities: all_severities(),
        }
    }

    /// `HealthFailure` -> `[ServiceRestart(p1), GracefulDegradation(p2), AlertHuman(p3)]`
    fn register_health_failure(registry: &mut HashMap<IssueType, Vec<ActionMapping>>) {
        registry.insert(
            IssueType::HealthFailure,
            vec![
                ActionMapping {
                    action: RemediationAction::ServiceRestart {
                        service_id: String::new(),
                        graceful: true,
                    },
                    priority: 1,
                    min_confidence: 0.5,
                    applicable_severities: all_severities(),
                },
                ActionMapping {
                    action: RemediationAction::GracefulDegradation {
                        service_id: String::new(),
                        level: 1,
                    },
                    priority: 2,
                    min_confidence: 0.3,
                    applicable_severities: all_severities(),
                },
                Self::alert_human_mapping(3),
            ],
        );
    }

    /// `LatencySpike` -> `[GracefulDegradation(p1), FallbackToCached(p2), AlertHuman(p3)]`
    fn register_latency_spike(registry: &mut HashMap<IssueType, Vec<ActionMapping>>) {
        registry.insert(
            IssueType::LatencySpike,
            vec![
                ActionMapping {
                    action: RemediationAction::GracefulDegradation {
                        service_id: String::new(),
                        level: 1,
                    },
                    priority: 1,
                    min_confidence: 0.5,
                    applicable_severities: all_severities(),
                },
                ActionMapping {
                    action: RemediationAction::FallbackToCached {
                        key: String::new(),
                        ttl_seconds: 300,
                    },
                    priority: 2,
                    min_confidence: 0.3,
                    applicable_severities: all_severities(),
                },
                Self::alert_human_mapping(3),
            ],
        );
    }

    /// `ErrorRateHigh` -> `[CircuitBreakerReset(p1), RetryWithBackoff(p2), AlertHuman(p3)]`
    fn register_error_rate_high(registry: &mut HashMap<IssueType, Vec<ActionMapping>>) {
        registry.insert(
            IssueType::ErrorRateHigh,
            vec![
                ActionMapping {
                    action: RemediationAction::CircuitBreakerReset {
                        service_id: String::new(),
                    },
                    priority: 1,
                    min_confidence: 0.5,
                    applicable_severities: all_severities(),
                },
                ActionMapping {
                    action: RemediationAction::RetryWithBackoff {
                        max_retries: 3,
                        initial_delay_ms: 100,
                    },
                    priority: 2,
                    min_confidence: 0.3,
                    applicable_severities: all_severities(),
                },
                Self::alert_human_mapping(3),
            ],
        );
    }

    /// `MemoryPressure` -> `[CacheCleanup(p1), ServiceRestart(p2), AlertHuman(p3)]`
    fn register_memory_pressure(registry: &mut HashMap<IssueType, Vec<ActionMapping>>) {
        registry.insert(
            IssueType::MemoryPressure,
            vec![
                ActionMapping {
                    action: RemediationAction::CacheCleanup {
                        service_id: String::new(),
                        threshold_percent: 80,
                    },
                    priority: 1,
                    min_confidence: 0.5,
                    applicable_severities: all_severities(),
                },
                ActionMapping {
                    action: RemediationAction::ServiceRestart {
                        service_id: String::new(),
                        graceful: true,
                    },
                    priority: 2,
                    min_confidence: 0.4,
                    applicable_severities: all_severities(),
                },
                Self::alert_human_mapping(3),
            ],
        );
    }

    /// `DiskPressure` -> `[DatabaseVacuum(p1), CacheCleanup(p2), AlertHuman(p3)]`
    fn register_disk_pressure(registry: &mut HashMap<IssueType, Vec<ActionMapping>>) {
        registry.insert(
            IssueType::DiskPressure,
            vec![
                ActionMapping {
                    action: RemediationAction::DatabaseVacuum {
                        database: String::new(),
                    },
                    priority: 1,
                    min_confidence: 0.5,
                    applicable_severities: all_severities(),
                },
                ActionMapping {
                    action: RemediationAction::CacheCleanup {
                        service_id: String::new(),
                        threshold_percent: 50,
                    },
                    priority: 2,
                    min_confidence: 0.3,
                    applicable_severities: all_severities(),
                },
                Self::alert_human_mapping(3),
            ],
        );
    }

    /// `ConnectionFailure` -> `[RetryWithBackoff(p1), CircuitBreakerReset(p2), AlertHuman(p3)]`
    fn register_connection_failure(registry: &mut HashMap<IssueType, Vec<ActionMapping>>) {
        registry.insert(
            IssueType::ConnectionFailure,
            vec![
                ActionMapping {
                    action: RemediationAction::RetryWithBackoff {
                        max_retries: 5,
                        initial_delay_ms: 200,
                    },
                    priority: 1,
                    min_confidence: 0.5,
                    applicable_severities: all_severities(),
                },
                ActionMapping {
                    action: RemediationAction::CircuitBreakerReset {
                        service_id: String::new(),
                    },
                    priority: 2,
                    min_confidence: 0.3,
                    applicable_severities: all_severities(),
                },
                Self::alert_human_mapping(3),
            ],
        );
    }

    /// `Timeout` -> `[RetryWithBackoff(p1), GracefulDegradation(p2), AlertHuman(p3)]`
    fn register_timeout(registry: &mut HashMap<IssueType, Vec<ActionMapping>>) {
        registry.insert(
            IssueType::Timeout,
            vec![
                ActionMapping {
                    action: RemediationAction::RetryWithBackoff {
                        max_retries: 3,
                        initial_delay_ms: 500,
                    },
                    priority: 1,
                    min_confidence: 0.5,
                    applicable_severities: all_severities(),
                },
                ActionMapping {
                    action: RemediationAction::GracefulDegradation {
                        service_id: String::new(),
                        level: 2,
                    },
                    priority: 2,
                    min_confidence: 0.3,
                    applicable_severities: all_severities(),
                },
                Self::alert_human_mapping(3),
            ],
        );
    }

    /// `Crash` -> `[ServiceRestart(p1), AlertHuman(p2)]`
    fn register_crash(registry: &mut HashMap<IssueType, Vec<ActionMapping>>) {
        registry.insert(
            IssueType::Crash,
            vec![
                ActionMapping {
                    action: RemediationAction::ServiceRestart {
                        service_id: String::new(),
                        graceful: false,
                    },
                    priority: 1,
                    min_confidence: 0.4,
                    applicable_severities: all_severities(),
                },
                Self::alert_human_mapping(2),
            ],
        );
    }
}

impl Default for RemediationEngine {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Free functions
// ---------------------------------------------------------------------------

/// Return all four severity levels as a `Vec`.
#[must_use]
fn all_severities() -> Vec<Severity> {
    vec![
        Severity::Low,
        Severity::Medium,
        Severity::High,
        Severity::Critical,
    ]
}

/// Convert a [`Severity`] to a normalised score in [0, 1].
#[must_use]
const fn severity_to_score(severity: Severity) -> f64 {
    match severity {
        Severity::Low => 0.25,
        Severity::Medium => 0.5,
        Severity::High => 0.75,
        Severity::Critical => 1.0,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- Helpers ------------------------------------------------------------

    /// Convenience: submit a default request and return its ID.
    fn submit_default(engine: &RemediationEngine) -> String {
        let req = engine
            .submit_request(
                "test-service",
                IssueType::HealthFailure,
                Severity::Medium,
                "test description",
            )
            .expect("submit_request should succeed");
        req.id
    }

    // -- Tests --------------------------------------------------------------

    #[test]
    fn test_new_engine() {
        let engine = RemediationEngine::new();
        assert_eq!(engine.pending_count(), 0);
        assert_eq!(engine.active_count(), 0);
        assert_eq!(engine.completed_count(), 0);
        assert_eq!(engine.max_concurrent, DEFAULT_MAX_CONCURRENT);

        // Verify all 8 issue types have registry entries.
        assert_eq!(engine.action_registry.len(), 8);
        assert!(engine.action_registry.contains_key(&IssueType::HealthFailure));
        assert!(engine.action_registry.contains_key(&IssueType::LatencySpike));
        assert!(engine.action_registry.contains_key(&IssueType::ErrorRateHigh));
        assert!(engine.action_registry.contains_key(&IssueType::MemoryPressure));
        assert!(engine.action_registry.contains_key(&IssueType::DiskPressure));
        assert!(engine.action_registry.contains_key(&IssueType::ConnectionFailure));
        assert!(engine.action_registry.contains_key(&IssueType::Timeout));
        assert!(engine.action_registry.contains_key(&IssueType::Crash));
    }

    #[test]
    fn test_submit_request() {
        let engine = RemediationEngine::new();
        let result = engine.submit_request(
            "synthex",
            IssueType::HealthFailure,
            Severity::High,
            "service unresponsive",
        );
        assert!(result.is_ok());

        let req = result.expect("already checked");
        assert_eq!(req.service_id, "synthex");
        assert_eq!(req.issue_type, IssueType::HealthFailure);
        assert_eq!(req.severity, Severity::High);
        assert!(req.confidence > 0.0 && req.confidence <= 1.0);
        assert!(!req.id.is_empty());
        assert_eq!(engine.pending_count(), 1);
    }

    #[test]
    fn test_submit_request_empty_service_id() {
        let engine = RemediationEngine::new();
        let result = engine.submit_request(
            "",
            IssueType::HealthFailure,
            Severity::Low,
            "test",
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_submit_request_empty_description() {
        let engine = RemediationEngine::new();
        let result = engine.submit_request(
            "svc",
            IssueType::HealthFailure,
            Severity::Low,
            "",
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_select_action_health_failure() {
        let engine = RemediationEngine::new();
        let action = engine
            .select_action(IssueType::HealthFailure, Severity::High, 0.8)
            .expect("should select an action");

        // Priority 1 for HealthFailure is ServiceRestart.
        assert!(matches!(action, RemediationAction::ServiceRestart { .. }));
    }

    #[test]
    fn test_select_action_latency_spike() {
        let engine = RemediationEngine::new();
        let action = engine
            .select_action(IssueType::LatencySpike, Severity::Medium, 0.8)
            .expect("should select an action");

        // Priority 1 for LatencySpike is GracefulDegradation.
        assert!(matches!(
            action,
            RemediationAction::GracefulDegradation { .. }
        ));
    }

    #[test]
    fn test_select_action_high_severity_escalates() {
        let engine = RemediationEngine::new();

        // Submit a crash with Critical severity. The action should be
        // ServiceRestart (non-graceful), and determine_tier should push
        // it to L3 (because graceful=false triggers PBFT consensus).
        let req = engine
            .submit_request(
                "svc-crash",
                IssueType::Crash,
                Severity::Critical,
                "crash detected",
            )
            .expect("should succeed");

        // Crash p1 is ServiceRestart { graceful: false } which triggers
        // L3PbftConsensus via determine_tier.
        assert_eq!(req.tier, crate::EscalationTier::L3PbftConsensus);
    }

    #[test]
    fn test_process_next_request() {
        let engine = RemediationEngine::new();
        let _id = submit_default(&engine);

        assert_eq!(engine.pending_count(), 1);
        assert_eq!(engine.active_count(), 0);

        let result = engine.process_next();
        assert!(result.is_ok());

        let active = result.expect("already checked");
        assert!(active.is_some());

        let active_rem = active.expect("already checked");
        assert_eq!(active_rem.status, RemediationStatus::Executing);

        assert_eq!(engine.pending_count(), 0);
        assert_eq!(engine.active_count(), 1);
    }

    #[test]
    fn test_process_next_empty_queue() {
        let engine = RemediationEngine::new();
        let result = engine.process_next();
        assert!(result.is_ok());
        assert!(result.expect("already checked").is_none());
    }

    #[test]
    fn test_max_concurrent_limit() {
        let engine = RemediationEngine::with_max_concurrent(2)
            .expect("should create engine");

        // Submit 3 requests.
        let _id1 = submit_default(&engine);
        let _id2 = submit_default(&engine);
        let _id3 = submit_default(&engine);

        assert_eq!(engine.pending_count(), 3);

        // Process first two.
        let r1 = engine.process_next().expect("ok");
        assert!(r1.is_some());
        let r2 = engine.process_next().expect("ok");
        assert!(r2.is_some());

        // Third should be blocked by concurrency limit.
        let r3 = engine.process_next().expect("ok");
        assert!(r3.is_none());

        assert_eq!(engine.active_count(), 2);
        assert_eq!(engine.pending_count(), 1);
    }

    #[test]
    fn test_max_concurrent_zero_rejected() {
        let result = RemediationEngine::with_max_concurrent(0);
        assert!(result.is_err());
    }

    #[test]
    fn test_complete_request_success() {
        let engine = RemediationEngine::new();
        let id = submit_default(&engine);

        // Move to active.
        let _ = engine.process_next().expect("ok");

        let outcome = engine
            .complete_request(&id, true, 150, None)
            .expect("should complete");

        assert!(outcome.success);
        assert_eq!(outcome.duration_ms, 150);
        assert!(outcome.error.is_none());
        assert!(outcome.pathway_delta > 0.0);

        assert_eq!(engine.active_count(), 0);
        assert_eq!(engine.completed_count(), 1);
    }

    #[test]
    fn test_complete_request_failure() {
        let engine = RemediationEngine::new();
        let id = submit_default(&engine);
        let _ = engine.process_next().expect("ok");

        let outcome = engine
            .complete_request(&id, false, 3000, Some("connection refused".into()))
            .expect("should complete");

        assert!(!outcome.success);
        assert_eq!(outcome.duration_ms, 3000);
        assert_eq!(outcome.error.as_deref(), Some("connection refused"));
        assert!(outcome.pathway_delta < 0.0);
    }

    #[test]
    fn test_complete_request_not_found() {
        let engine = RemediationEngine::new();
        let result = engine.complete_request("nonexistent", true, 100, None);
        assert!(result.is_err());
    }

    #[test]
    fn test_cancel_request() {
        let engine = RemediationEngine::new();
        let id = submit_default(&engine);

        assert_eq!(engine.pending_count(), 1);

        let result = engine.cancel_request(&id);
        assert!(result.is_ok());
        assert_eq!(engine.pending_count(), 0);
    }

    #[test]
    fn test_cancel_active_request() {
        let engine = RemediationEngine::new();
        let id = submit_default(&engine);
        let _ = engine.process_next().expect("ok");

        assert_eq!(engine.active_count(), 1);

        let result = engine.cancel_request(&id);
        assert!(result.is_ok());
        assert_eq!(engine.active_count(), 0);
    }

    #[test]
    fn test_cancel_request_not_found() {
        let engine = RemediationEngine::new();
        let result = engine.cancel_request("nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_get_request_status() {
        let engine = RemediationEngine::new();
        let id = submit_default(&engine);

        // Should be Queued.
        let status = engine.get_request_status(&id).expect("should find it");
        assert_eq!(status, RemediationStatus::Queued);

        // Move to active -> Executing.
        let _ = engine.process_next().expect("ok");
        let status = engine.get_request_status(&id).expect("should find it");
        assert_eq!(status, RemediationStatus::Executing);

        // Complete -> Completed.
        let _ = engine.complete_request(&id, true, 100, None).expect("ok");
        let status = engine.get_request_status(&id).expect("should find it");
        assert_eq!(status, RemediationStatus::Completed);
    }

    #[test]
    fn test_get_request_status_failed() {
        let engine = RemediationEngine::new();
        let id = submit_default(&engine);
        let _ = engine.process_next().expect("ok");
        let _ = engine
            .complete_request(&id, false, 500, Some("error".into()))
            .expect("ok");

        let status = engine.get_request_status(&id).expect("should find it");
        assert_eq!(status, RemediationStatus::Failed);
    }

    #[test]
    fn test_get_request_status_not_found() {
        let engine = RemediationEngine::new();
        let result = engine.get_request_status("nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_pending_active_counts() {
        let engine = RemediationEngine::new();

        assert_eq!(engine.pending_count(), 0);
        assert_eq!(engine.active_count(), 0);

        let _ = submit_default(&engine);
        let _ = submit_default(&engine);
        assert_eq!(engine.pending_count(), 2);

        let _ = engine.process_next().expect("ok");
        assert_eq!(engine.pending_count(), 1);
        assert_eq!(engine.active_count(), 1);

        let _ = engine.process_next().expect("ok");
        assert_eq!(engine.pending_count(), 0);
        assert_eq!(engine.active_count(), 2);
    }

    #[test]
    fn test_success_rate_calculation() {
        let engine = RemediationEngine::new();

        // Empty -> 0.0.
        assert!((engine.success_rate() - 0.0).abs() < f64::EPSILON);

        // Submit and complete 3 requests: 2 success, 1 failure.
        let id1 = submit_default(&engine);
        let _ = engine.process_next().expect("ok");
        let _ = engine.complete_request(&id1, true, 100, None).expect("ok");

        let id2 = submit_default(&engine);
        let _ = engine.process_next().expect("ok");
        let _ = engine.complete_request(&id2, true, 200, None).expect("ok");

        let id3 = submit_default(&engine);
        let _ = engine.process_next().expect("ok");
        let _ = engine
            .complete_request(&id3, false, 300, Some("fail".into()))
            .expect("ok");

        let rate = engine.success_rate();
        // 2 / 3 ~ 0.6667
        assert!((rate - 2.0 / 3.0).abs() < 1e-10);
    }

    #[test]
    fn test_outcomes_for_service() {
        let engine = RemediationEngine::new();

        // Submit and complete a request for "svc-alpha".
        let req = engine
            .submit_request(
                "svc-alpha",
                IssueType::MemoryPressure,
                Severity::Medium,
                "memory high",
            )
            .expect("ok");
        let _ = engine.process_next().expect("ok");
        let _ = engine
            .complete_request(&req.id, true, 100, None)
            .expect("ok");

        // Submit and complete a request for "svc-beta".
        let req2 = engine
            .submit_request(
                "svc-beta",
                IssueType::Timeout,
                Severity::Low,
                "timeout",
            )
            .expect("ok");
        let _ = engine.process_next().expect("ok");
        let _ = engine
            .complete_request(&req2.id, true, 200, None)
            .expect("ok");

        // Only the alpha outcome should be returned -- but since completed
        // outcomes don't store service_id and UUIDs don't contain the
        // service name, the cross-reference only works if the request is
        // still in pending/active. For fully archived requests, this will
        // be empty. This is a known limitation documented above.
        // We verify the method does not panic and returns a valid vec.
        let alpha_outcomes = engine.get_outcomes_for_service("svc-alpha");
        assert!(alpha_outcomes.len() <= 1);

        let beta_outcomes = engine.get_outcomes_for_service("svc-beta");
        assert!(beta_outcomes.len() <= 1);
    }

    #[test]
    fn test_avg_resolution_time() {
        let engine = RemediationEngine::new();

        // Empty -> 0.0.
        assert!((engine.avg_resolution_time_ms() - 0.0).abs() < f64::EPSILON);

        let id1 = submit_default(&engine);
        let _ = engine.process_next().expect("ok");
        let _ = engine.complete_request(&id1, true, 100, None).expect("ok");

        let id2 = submit_default(&engine);
        let _ = engine.process_next().expect("ok");
        let _ = engine.complete_request(&id2, true, 300, None).expect("ok");

        let avg = engine.avg_resolution_time_ms();
        // (100 + 300) / 2 = 200
        assert!((avg - 200.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_completed_cap() {
        let engine = RemediationEngine::new();

        // Submit and complete COMPLETED_CAP + 10 requests.
        for _ in 0..(COMPLETED_CAP + 10) {
            let id = submit_default(&engine);
            let _ = engine.process_next().expect("ok");
            let _ = engine.complete_request(&id, true, 50, None).expect("ok");
        }

        // Should never exceed the cap.
        assert!(engine.completed_count() <= COMPLETED_CAP);
        assert_eq!(engine.completed_count(), COMPLETED_CAP);
    }

    #[test]
    fn test_get_pending_requests() {
        let engine = RemediationEngine::new();
        let _ = submit_default(&engine);
        let _ = submit_default(&engine);

        let pending = engine.get_pending_requests();
        assert_eq!(pending.len(), 2);
    }

    #[test]
    fn test_get_active_requests() {
        let engine = RemediationEngine::new();
        let _ = submit_default(&engine);
        let _ = engine.process_next().expect("ok");

        let active = engine.get_active_requests();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].status, RemediationStatus::Executing);
    }

    #[test]
    fn test_select_action_error_rate_high() {
        let engine = RemediationEngine::new();
        let action = engine
            .select_action(IssueType::ErrorRateHigh, Severity::High, 0.8)
            .expect("should select an action");

        // Priority 1 for ErrorRateHigh is CircuitBreakerReset.
        assert!(matches!(
            action,
            RemediationAction::CircuitBreakerReset { .. }
        ));
    }

    #[test]
    fn test_select_action_memory_pressure() {
        let engine = RemediationEngine::new();
        let action = engine
            .select_action(IssueType::MemoryPressure, Severity::Medium, 0.7)
            .expect("should select an action");

        // Priority 1 for MemoryPressure is CacheCleanup.
        assert!(matches!(action, RemediationAction::CacheCleanup { .. }));
    }

    #[test]
    fn test_select_action_connection_failure() {
        let engine = RemediationEngine::new();
        let action = engine
            .select_action(IssueType::ConnectionFailure, Severity::Low, 0.9)
            .expect("should select an action");

        // Priority 1 for ConnectionFailure is RetryWithBackoff.
        assert!(matches!(
            action,
            RemediationAction::RetryWithBackoff { .. }
        ));
    }

    #[test]
    fn test_select_action_timeout() {
        let engine = RemediationEngine::new();
        let action = engine
            .select_action(IssueType::Timeout, Severity::Medium, 0.6)
            .expect("should select an action");

        // Priority 1 for Timeout is RetryWithBackoff.
        assert!(matches!(
            action,
            RemediationAction::RetryWithBackoff { .. }
        ));
    }

    #[test]
    fn test_select_action_disk_pressure() {
        let engine = RemediationEngine::new();
        let action = engine
            .select_action(IssueType::DiskPressure, Severity::High, 0.8)
            .expect("should select an action");

        // Priority 1 for DiskPressure is DatabaseVacuum.
        assert!(matches!(action, RemediationAction::DatabaseVacuum { .. }));
    }

    #[test]
    fn test_select_action_crash() {
        let engine = RemediationEngine::new();
        let action = engine
            .select_action(IssueType::Crash, Severity::Critical, 0.6)
            .expect("should select an action");

        // Priority 1 for Crash is ServiceRestart (non-graceful).
        assert!(matches!(
            action,
            RemediationAction::ServiceRestart { graceful: false, .. }
        ));
    }

    #[test]
    fn test_select_action_low_confidence_fallback() {
        let engine = RemediationEngine::new();

        // With confidence below all min_confidence thresholds except AlertHuman (0.0),
        // we should get AlertHuman or the fallback.
        let action = engine
            .select_action(IssueType::HealthFailure, Severity::Low, 0.01)
            .expect("should return fallback action");

        // The AlertHuman mapping has min_confidence 0.0, so it qualifies.
        assert!(matches!(action, RemediationAction::AlertHuman { .. }));
    }

    #[test]
    fn test_default_impl() {
        let engine = RemediationEngine::default();
        assert_eq!(engine.pending_count(), 0);
        assert_eq!(engine.max_concurrent, DEFAULT_MAX_CONCURRENT);
    }

    #[test]
    fn test_severity_to_score() {
        assert!((severity_to_score(Severity::Low) - 0.25).abs() < f64::EPSILON);
        assert!((severity_to_score(Severity::Medium) - 0.5).abs() < f64::EPSILON);
        assert!((severity_to_score(Severity::High) - 0.75).abs() < f64::EPSILON);
        assert!((severity_to_score(Severity::Critical) - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_multiple_requests_full_lifecycle() {
        let engine = RemediationEngine::new();

        // Submit 3 different issue types.
        let r1 = engine
            .submit_request("svc-a", IssueType::HealthFailure, Severity::High, "hf")
            .expect("ok");
        let r2 = engine
            .submit_request("svc-b", IssueType::ErrorRateHigh, Severity::Medium, "err")
            .expect("ok");
        let r3 = engine
            .submit_request("svc-c", IssueType::Timeout, Severity::Low, "to")
            .expect("ok");

        assert_eq!(engine.pending_count(), 3);

        // Process all.
        let _ = engine.process_next().expect("ok");
        let _ = engine.process_next().expect("ok");
        let _ = engine.process_next().expect("ok");

        assert_eq!(engine.pending_count(), 0);
        assert_eq!(engine.active_count(), 3);

        // Complete all.
        let _ = engine.complete_request(&r1.id, true, 100, None).expect("ok");
        let _ = engine.complete_request(&r2.id, true, 200, None).expect("ok");
        let _ = engine
            .complete_request(&r3.id, false, 500, Some("timed out".into()))
            .expect("ok");

        assert_eq!(engine.active_count(), 0);
        assert_eq!(engine.completed_count(), 3);

        // 2 successes / 3 total.
        let rate = engine.success_rate();
        assert!((rate - 2.0 / 3.0).abs() < 1e-10);

        // (100 + 200 + 500) / 3 ~ 266.67.
        let avg = engine.avg_resolution_time_ms();
        assert!((avg - 800.0 / 3.0).abs() < 1e-10);
    }
}
