//! # M50: Approval Workflow
//!
//! Manages the full lifecycle of approval requests within the Maintenance Engine.
//! Integrates with the NAM escalation tier system to route L2 requests through
//! human approval and L3 requests through PBFT consensus.
//!
//! ## Layer: L3 (Core Logic)
//! ## Dependencies: M01 (Error), `EscalationTier`, M00 (Timestamp)
//!
//! ## Design
//!
//! The [`ApprovalManager`] maintains a thread-safe registry of pending approval
//! requests, each tracked through its lifecycle from submission to decision
//! (approved, rejected, expired, or escalated). An audit log records every
//! decision for post-hoc analysis and Hebbian pathway feedback.
//!
//! ## Escalation Tier Enforcement
//!
//! | Tier | Decision Path |
//! |------|---------------|
//! | L2 `RequireApproval` | Human agent decides via [`decide_human`] |
//! | L3 `PbftConsensus` | Consensus outcome via [`decide_pbft`] |
//!
//! ## NAM-R5 Compliance
//!
//! Only the human agent `@0.A` may issue human decisions, enforcing the
//! Human-as-Agent integration at Tier 0.
//!
//! ## 12D Tensor Encoding
//! ```text
//! [50/56, 0.0, 3/6, 3, 0, 0.5, health, uptime, synergy, latency, error_rate, temporal]
//! ```
//!
//! ## Related Documentation
//! - [Layer Specification](../../ai_docs/layers/L03_CORE_LOGIC.md)
//! - [Escalation Spec](../../ai_specs/ESCALATION_SPEC.md)

use std::collections::HashMap;
use std::fmt;

use parking_lot::RwLock;
use uuid::Uuid;

use crate::m1_foundation::shared_types::Timestamp;
use crate::{EscalationTier, Error, Result};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Default maximum number of pending approval requests.
const DEFAULT_MAX_PENDING: usize = 100;

/// Default timeout in ticks for L2 requests.
const DEFAULT_L2_TIMEOUT: u64 = 1800;

/// Default timeout in ticks for L3 requests.
const DEFAULT_L3_TIMEOUT: u64 = 3600;

/// Maximum number of audit log entries before oldest are evicted.
const AUDIT_LOG_CAP: usize = 200;

/// The only agent identifier permitted for human decisions (NAM-R5).
const HUMAN_AGENT_ID: &str = "@0.A";

// ---------------------------------------------------------------------------
// ApprovalDecision
// ---------------------------------------------------------------------------

/// Decision that a human agent can render on a pending approval request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApprovalDecision {
    /// The request is approved for execution.
    Approved,
    /// The request is rejected and will not be executed.
    Rejected,
    /// The decision is deferred with a reason; the request remains pending.
    Deferred {
        /// Explanation for why the decision was deferred.
        reason: String,
    },
}

// ---------------------------------------------------------------------------
// ApprovalStatus
// ---------------------------------------------------------------------------

/// Current status of an approval request in its lifecycle.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApprovalStatus {
    /// Awaiting a decision.
    Pending,
    /// Approved by a human agent or PBFT consensus.
    Approved {
        /// The agent or process that approved the request.
        by: String,
        /// Timestamp of the approval decision.
        at: Timestamp,
    },
    /// Rejected by a human agent or PBFT consensus.
    Rejected {
        /// The agent or process that rejected the request.
        by: String,
        /// Reason for rejection.
        reason: String,
        /// Timestamp of the rejection decision.
        at: Timestamp,
    },
    /// The request timed out before a decision was made.
    Expired,
    /// Escalated from L2 human approval to L3 PBFT consensus.
    Escalated {
        /// The PBFT proposal ID that this request was escalated to.
        to_pbft: String,
    },
}

// ---------------------------------------------------------------------------
// ApprovalRequest
// ---------------------------------------------------------------------------

/// A request for approval of a maintenance action.
///
/// Contains all context needed for a human agent or PBFT consensus to
/// make an informed decision about whether the proposed action should proceed.
#[derive(Clone, Debug)]
pub struct ApprovalRequest {
    /// Unique identifier for this request (UUID v4).
    pub id: String,
    /// Human-readable description of the proposed action.
    pub action_description: String,
    /// Escalation tier governing the approval path.
    pub tier: EscalationTier,
    /// Identifier of the module or service that submitted this request.
    pub requester: String,
    /// Confidence score of the proposed action (0.0-1.0).
    pub confidence: f64,
    /// Severity level of the underlying issue.
    pub severity: String,
    /// Timestamp when the request was submitted.
    pub submitted_at: Timestamp,
    /// Number of ticks before the request expires.
    pub timeout_secs: u64,
}

// ---------------------------------------------------------------------------
// ConsensusOutcome
// ---------------------------------------------------------------------------

/// Outcome of a PBFT consensus vote on an approval request.
#[derive(Clone, Debug)]
pub struct ConsensusOutcome {
    /// The PBFT proposal identifier.
    pub proposal_id: String,
    /// Whether the quorum threshold was reached.
    pub quorum_reached: bool,
    /// Number of votes in favour.
    pub votes_for: u32,
    /// Number of votes against.
    pub votes_against: u32,
}

// ---------------------------------------------------------------------------
// ApprovalAuditEntry
// ---------------------------------------------------------------------------

/// A single entry in the approval audit log.
///
/// Records every decision made on an approval request for post-hoc analysis,
/// compliance reporting, and Hebbian pathway feedback.
#[derive(Clone, Debug)]
pub struct ApprovalAuditEntry {
    /// The approval request ID this entry pertains to.
    pub request_id: String,
    /// Description of the action that was requested.
    pub action: String,
    /// The decision that was rendered.
    pub decision: String,
    /// The agent or process that made the decision.
    pub decided_by: String,
    /// Timestamp when the decision was recorded.
    pub timestamp: Timestamp,
}

// ---------------------------------------------------------------------------
// ApprovalConfig
// ---------------------------------------------------------------------------

/// Configuration for the approval workflow system.
#[derive(Clone, Debug)]
pub struct ApprovalConfig {
    /// Maximum number of pending approval requests.
    pub max_pending: usize,
    /// Timeout in ticks for L2 (human approval) requests.
    pub l2_timeout_secs: u64,
    /// Timeout in ticks for L3 (PBFT consensus) requests.
    pub l3_timeout_secs: u64,
}

impl Default for ApprovalConfig {
    fn default() -> Self {
        Self {
            max_pending: DEFAULT_MAX_PENDING,
            l2_timeout_secs: DEFAULT_L2_TIMEOUT,
            l3_timeout_secs: DEFAULT_L3_TIMEOUT,
        }
    }
}

// ---------------------------------------------------------------------------
// ApprovalWorkflow trait
// ---------------------------------------------------------------------------

/// Trait defining the approval workflow contract.
///
/// All methods take `&self` with interior mutability via [`RwLock`] to satisfy
/// the `Send + Sync` requirement.
pub trait ApprovalWorkflow: Send + Sync + fmt::Debug {
    /// Submit a new approval request.
    ///
    /// Returns the generated request ID on success.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] if required fields are empty or the
    /// pending request cap has been reached.
    fn submit(&self, request: ApprovalRequest) -> Result<String>;

    /// Record a human decision on a pending L2 request.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] if:
    /// - The request ID is not found or not in `Pending` status
    /// - The request tier is L3 (must use [`decide_pbft`](Self::decide_pbft))
    /// - The agent is not `@0.A` (NAM-R5 enforcement)
    fn decide_human(&self, request_id: &str, decision: ApprovalDecision, agent: &str)
        -> Result<()>;

    /// Record a PBFT consensus outcome on a pending L3 request.
    ///
    /// Maps `quorum_reached == true` to [`ApprovalStatus::Approved`] and
    /// `quorum_reached == false` to [`ApprovalStatus::Rejected`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] if the request ID is not found or not
    /// in `Pending` status.
    fn decide_pbft(&self, request_id: &str, outcome: ConsensusOutcome) -> Result<()>;

    /// Return all currently pending approval requests.
    fn get_pending(&self) -> Vec<ApprovalRequest>;

    /// Get the current status of an approval request.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] if the request ID is not found.
    fn get_status(&self, request_id: &str) -> Result<ApprovalStatus>;

    /// Expire all timed-out requests and return their IDs.
    fn expire_timed_out(&self) -> Vec<String>;

    /// Return the total number of tracked approval requests (all statuses).
    fn approval_count(&self) -> usize;
}

// ---------------------------------------------------------------------------
// ApprovalManager
// ---------------------------------------------------------------------------

/// Thread-safe implementation of [`ApprovalWorkflow`].
///
/// Stores pending requests with their statuses and maintains a capped audit
/// log of all decisions.
///
/// # Construction
///
/// ```rust
/// use maintenance_engine_v2::m3_core_logic::approval::{ApprovalManager, ApprovalConfig};
///
/// let manager = ApprovalManager::new();
/// assert_eq!(manager.approval_count(), 0);
///
/// let config = ApprovalConfig { max_pending: 50, ..ApprovalConfig::default() };
/// let manager = ApprovalManager::with_config(config);
/// ```
pub struct ApprovalManager {
    /// Pending and resolved requests, keyed by request ID.
    pending: RwLock<HashMap<String, (ApprovalRequest, ApprovalStatus)>>,
    /// Capped audit log of all decisions.
    audit_log: RwLock<Vec<ApprovalAuditEntry>>,
    /// Workflow configuration.
    config: ApprovalConfig,
}

impl fmt::Debug for ApprovalManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let count = self.pending.read().len();
        let audit_len = self.audit_log.read().len();
        f.debug_struct("ApprovalManager")
            .field("pending_count", &count)
            .field("audit_log_len", &audit_len)
            .field("config", &self.config)
            .finish()
    }
}

impl ApprovalManager {
    /// Create a new `ApprovalManager` with default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self {
            pending: RwLock::new(HashMap::new()),
            audit_log: RwLock::new(Vec::new()),
            config: ApprovalConfig::default(),
        }
    }

    /// Create a new `ApprovalManager` with custom configuration.
    #[must_use]
    pub fn with_config(config: ApprovalConfig) -> Self {
        Self {
            pending: RwLock::new(HashMap::new()),
            audit_log: RwLock::new(Vec::new()),
            config,
        }
    }

    /// Append an entry to the audit log, evicting the oldest if at capacity.
    fn record_audit(&self, entry: ApprovalAuditEntry) {
        let mut log = self.audit_log.write();
        if log.len() >= AUDIT_LOG_CAP {
            log.remove(0);
        }
        log.push(entry);
    }

    /// Return a snapshot of the audit log.
    #[must_use]
    pub fn get_audit_log(&self) -> Vec<ApprovalAuditEntry> {
        self.audit_log.read().clone()
    }

    /// Determine the timeout for a given escalation tier.
    const fn timeout_for_tier(&self, tier: EscalationTier) -> u64 {
        match tier {
            EscalationTier::L3PbftConsensus => self.config.l3_timeout_secs,
            _ => self.config.l2_timeout_secs,
        }
    }
}

impl Default for ApprovalManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ApprovalWorkflow for ApprovalManager {
    fn submit(&self, request: ApprovalRequest) -> Result<String> {
        // Validate non-empty fields
        if request.action_description.is_empty() {
            return Err(Error::Validation(
                "action_description must not be empty".into(),
            ));
        }
        if request.requester.is_empty() {
            return Err(Error::Validation("requester must not be empty".into()));
        }
        if request.severity.is_empty() {
            return Err(Error::Validation("severity must not be empty".into()));
        }

        // Generate UUID if not provided
        let id = if request.id.is_empty() {
            Uuid::new_v4().to_string()
        } else {
            request.id.clone()
        };

        // Compute timeout before constructing the stored request
        let timeout = if request.timeout_secs == 0 {
            self.timeout_for_tier(request.tier)
        } else {
            request.timeout_secs
        };

        let stored_request = ApprovalRequest {
            id: id.clone(),
            action_description: request.action_description,
            tier: request.tier,
            requester: request.requester,
            confidence: request.confidence,
            severity: request.severity,
            submitted_at: request.submitted_at,
            timeout_secs: timeout,
        };

        // Check max_pending cap and insert under a single lock acquisition
        let mut pending = self.pending.write();
        let pending_count = pending
            .values()
            .filter(|(_, status)| matches!(status, ApprovalStatus::Pending))
            .count();
        if pending_count >= self.config.max_pending {
            return Err(Error::Validation(format!(
                "maximum pending requests ({}) reached",
                self.config.max_pending
            )));
        }

        pending.insert(id.clone(), (stored_request, ApprovalStatus::Pending));
        drop(pending);

        Ok(id)
    }

    fn decide_human(
        &self,
        request_id: &str,
        decision: ApprovalDecision,
        agent: &str,
    ) -> Result<()> {
        // NAM-R5: only @0.A may render human decisions
        if agent != HUMAN_AGENT_ID {
            return Err(Error::Validation(format!(
                "only agent '{HUMAN_AGENT_ID}' may render human decisions, got '{agent}'"
            )));
        }

        let mut pending = self.pending.write();
        let (request, status) = pending.get_mut(request_id).ok_or_else(|| {
            Error::Validation(format!("approval request '{request_id}' not found"))
        })?;

        // Must be Pending
        if !matches!(status, ApprovalStatus::Pending) {
            return Err(Error::Validation(format!(
                "approval request '{request_id}' is not in Pending status"
            )));
        }

        // L3 requests must use decide_pbft
        if matches!(request.tier, EscalationTier::L3PbftConsensus) {
            return Err(Error::Validation(
                "L3 PBFT requests must use decide_pbft, not decide_human".into(),
            ));
        }

        let now = Timestamp::now();
        let action_desc = request.action_description.clone();

        match decision {
            ApprovalDecision::Approved => {
                *status = ApprovalStatus::Approved {
                    by: agent.to_owned(),
                    at: now,
                };
                drop(pending);
                self.record_audit(ApprovalAuditEntry {
                    request_id: request_id.to_owned(),
                    action: action_desc,
                    decision: "Approved".to_owned(),
                    decided_by: agent.to_owned(),
                    timestamp: now,
                });
            }
            ApprovalDecision::Rejected => {
                *status = ApprovalStatus::Rejected {
                    by: agent.to_owned(),
                    reason: "Rejected by human agent".to_owned(),
                    at: now,
                };
                drop(pending);
                self.record_audit(ApprovalAuditEntry {
                    request_id: request_id.to_owned(),
                    action: action_desc,
                    decision: "Rejected".to_owned(),
                    decided_by: agent.to_owned(),
                    timestamp: now,
                });
            }
            ApprovalDecision::Deferred { reason } => {
                // Deferred keeps Pending status; just audit it
                drop(pending);
                self.record_audit(ApprovalAuditEntry {
                    request_id: request_id.to_owned(),
                    action: action_desc,
                    decision: format!("Deferred: {reason}"),
                    decided_by: agent.to_owned(),
                    timestamp: now,
                });
            }
        }

        Ok(())
    }

    fn decide_pbft(&self, request_id: &str, outcome: ConsensusOutcome) -> Result<()> {
        let mut pending = self.pending.write();
        let (request, status) = pending.get_mut(request_id).ok_or_else(|| {
            Error::Validation(format!("approval request '{request_id}' not found"))
        })?;

        if !matches!(status, ApprovalStatus::Pending) {
            return Err(Error::Validation(format!(
                "approval request '{request_id}' is not in Pending status"
            )));
        }

        let now = Timestamp::now();
        let action_desc = request.action_description.clone();
        let decided_by = format!(
            "PBFT(for={},against={})",
            outcome.votes_for, outcome.votes_against
        );

        if outcome.quorum_reached {
            *status = ApprovalStatus::Approved {
                by: decided_by.clone(),
                at: now,
            };
        } else {
            *status = ApprovalStatus::Rejected {
                by: decided_by.clone(),
                reason: format!(
                    "PBFT quorum not reached: {} for, {} against",
                    outcome.votes_for, outcome.votes_against
                ),
                at: now,
            };
        }

        drop(pending);
        self.record_audit(ApprovalAuditEntry {
            request_id: request_id.to_owned(),
            action: action_desc,
            decision: if outcome.quorum_reached {
                "Approved (PBFT)".to_owned()
            } else {
                "Rejected (PBFT)".to_owned()
            },
            decided_by,
            timestamp: now,
        });

        Ok(())
    }

    fn get_pending(&self) -> Vec<ApprovalRequest> {
        self.pending
            .read()
            .values()
            .filter(|(_, status)| matches!(status, ApprovalStatus::Pending))
            .map(|(req, _)| req.clone())
            .collect()
    }

    fn get_status(&self, request_id: &str) -> Result<ApprovalStatus> {
        self.pending
            .read()
            .get(request_id)
            .map(|(_, status)| status.clone())
            .ok_or_else(|| {
                Error::Validation(format!("approval request '{request_id}' not found"))
            })
    }

    fn expire_timed_out(&self) -> Vec<String> {
        let now = Timestamp::now();
        let mut expired_ids = Vec::new();

        let mut pending = self.pending.write();
        for (id, (request, status)) in pending.iter_mut() {
            if matches!(status, ApprovalStatus::Pending) {
                let elapsed = now.elapsed_since(request.submitted_at);
                if elapsed > request.timeout_secs {
                    *status = ApprovalStatus::Expired;
                    expired_ids.push(id.clone());
                }
            }
        }

        // Drop the write guard before recording audit entries
        let expired_actions: Vec<(String, String)> = expired_ids
            .iter()
            .filter_map(|id| {
                pending
                    .get(id)
                    .map(|(req, _)| (id.clone(), req.action_description.clone()))
            })
            .collect();
        drop(pending);

        for (id, action) in expired_actions {
            self.record_audit(ApprovalAuditEntry {
                request_id: id,
                action,
                decision: "Expired".to_owned(),
                decided_by: "system".to_owned(),
                timestamp: now,
            });
        }

        expired_ids
    }

    fn approval_count(&self) -> usize {
        self.pending.read().len()
    }
}

// ===========================================================================
// Helper: create a test request
// ===========================================================================

/// Create an [`ApprovalRequest`] with the given tier and reasonable defaults.
///
/// Intended for testing; generates a UUID and sets the submitted timestamp
/// to [`Timestamp::now()`].
#[must_use]
pub fn make_request(
    action_description: impl Into<String>,
    tier: EscalationTier,
    requester: impl Into<String>,
) -> ApprovalRequest {
    ApprovalRequest {
        id: Uuid::new_v4().to_string(),
        action_description: action_description.into(),
        tier,
        requester: requester.into(),
        confidence: 0.5,
        severity: "medium".to_owned(),
        submitted_at: Timestamp::now(),
        timeout_secs: match tier {
            EscalationTier::L3PbftConsensus => DEFAULT_L3_TIMEOUT,
            _ => DEFAULT_L2_TIMEOUT,
        },
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

    fn l2_request(desc: &str) -> ApprovalRequest {
        make_request(desc, EscalationTier::L2RequireApproval, "test-module")
    }

    fn l3_request(desc: &str) -> ApprovalRequest {
        make_request(desc, EscalationTier::L3PbftConsensus, "test-module")
    }

    fn quorum_reached() -> ConsensusOutcome {
        ConsensusOutcome {
            proposal_id: Uuid::new_v4().to_string(),
            quorum_reached: true,
            votes_for: 27,
            votes_against: 13,
        }
    }

    fn quorum_failed() -> ConsensusOutcome {
        ConsensusOutcome {
            proposal_id: Uuid::new_v4().to_string(),
            quorum_reached: false,
            votes_for: 10,
            votes_against: 30,
        }
    }

    // -----------------------------------------------------------------------
    // submit tests
    // -----------------------------------------------------------------------

    #[test]
    fn submit_returns_id() {
        let mgr = ApprovalManager::new();
        let req = l2_request("restart service-a");
        let id = mgr.submit(req).ok();
        assert!(id.is_some());
    }

    #[test]
    fn submit_increments_count() {
        let mgr = ApprovalManager::new();
        assert_eq!(mgr.approval_count(), 0);
        let _ = mgr.submit(l2_request("action-1"));
        assert_eq!(mgr.approval_count(), 1);
        let _ = mgr.submit(l2_request("action-2"));
        assert_eq!(mgr.approval_count(), 2);
    }

    #[test]
    fn submit_empty_description_fails() {
        let mgr = ApprovalManager::new();
        let mut req = l2_request("x");
        req.action_description = String::new();
        let result = mgr.submit(req);
        assert!(result.is_err());
    }

    #[test]
    fn submit_empty_requester_fails() {
        let mgr = ApprovalManager::new();
        let mut req = l2_request("restart");
        req.requester = String::new();
        let result = mgr.submit(req);
        assert!(result.is_err());
    }

    #[test]
    fn submit_empty_severity_fails() {
        let mgr = ApprovalManager::new();
        let mut req = l2_request("restart");
        req.severity = String::new();
        let result = mgr.submit(req);
        assert!(result.is_err());
    }

    #[test]
    fn submit_max_pending_cap_enforced() {
        let config = ApprovalConfig {
            max_pending: 3,
            ..ApprovalConfig::default()
        };
        let mgr = ApprovalManager::with_config(config);

        for i in 0..3 {
            let result = mgr.submit(l2_request(&format!("action-{i}")));
            assert!(result.is_ok(), "submit {i} should succeed");
        }

        let result = mgr.submit(l2_request("action-overflow"));
        assert!(result.is_err(), "4th submit should fail");
    }

    #[test]
    fn submit_preserves_provided_id() {
        let mgr = ApprovalManager::new();
        let mut req = l2_request("restart");
        req.id = "custom-id-42".to_owned();
        let id = mgr.submit(req);
        assert_eq!(id.as_deref().ok(), Some("custom-id-42"));
    }

    #[test]
    fn submit_generates_uuid_when_id_empty() {
        let mgr = ApprovalManager::new();
        let mut req = l2_request("restart");
        req.id = String::new();
        let id = mgr.submit(req);
        assert!(id.is_ok());
        let id_str = id.ok();
        assert!(id_str.is_some());
        // UUID v4 format: 8-4-4-4-12
        let id_val = id_str.unwrap_or_default();
        assert_eq!(id_val.len(), 36);
    }

    // -----------------------------------------------------------------------
    // get_status tests
    // -----------------------------------------------------------------------

    #[test]
    fn get_status_pending_after_submit() {
        let mgr = ApprovalManager::new();
        let req = l2_request("restart");
        let id = mgr.submit(req);
        let id = id.ok().unwrap_or_default();
        let status = mgr.get_status(&id);
        assert!(matches!(status, Ok(ApprovalStatus::Pending)));
    }

    #[test]
    fn get_status_not_found() {
        let mgr = ApprovalManager::new();
        let result = mgr.get_status("nonexistent-id");
        assert!(result.is_err());
    }

    // -----------------------------------------------------------------------
    // submit + get_status roundtrip
    // -----------------------------------------------------------------------

    #[test]
    fn submit_get_status_roundtrip() {
        let mgr = ApprovalManager::new();
        let req = l2_request("vacuum database");
        let id = mgr.submit(req);
        assert!(id.is_ok());
        let id = id.ok().unwrap_or_default();
        let status = mgr.get_status(&id);
        assert!(status.is_ok());
        assert!(matches!(status, Ok(ApprovalStatus::Pending)));
    }

    // -----------------------------------------------------------------------
    // decide_human tests
    // -----------------------------------------------------------------------

    #[test]
    fn decide_human_approve_l2() {
        let mgr = ApprovalManager::new();
        let id = mgr.submit(l2_request("restart service")).ok().unwrap_or_default();
        let result = mgr.decide_human(&id, ApprovalDecision::Approved, HUMAN_AGENT_ID);
        assert!(result.is_ok());
        let status = mgr.get_status(&id).ok();
        assert!(matches!(status, Some(ApprovalStatus::Approved { .. })));
    }

    #[test]
    fn decide_human_reject_l2() {
        let mgr = ApprovalManager::new();
        let id = mgr.submit(l2_request("restart service")).ok().unwrap_or_default();
        let result = mgr.decide_human(&id, ApprovalDecision::Rejected, HUMAN_AGENT_ID);
        assert!(result.is_ok());
        let status = mgr.get_status(&id).ok();
        assert!(matches!(status, Some(ApprovalStatus::Rejected { .. })));
    }

    #[test]
    fn decide_human_defer_keeps_pending() {
        let mgr = ApprovalManager::new();
        let id = mgr.submit(l2_request("restart service")).ok().unwrap_or_default();
        let result = mgr.decide_human(
            &id,
            ApprovalDecision::Deferred {
                reason: "need more info".into(),
            },
            HUMAN_AGENT_ID,
        );
        assert!(result.is_ok());
        let status = mgr.get_status(&id).ok();
        assert!(matches!(status, Some(ApprovalStatus::Pending)));
    }

    #[test]
    fn decide_human_on_l3_fails() {
        let mgr = ApprovalManager::new();
        let id = mgr.submit(l3_request("kill process")).ok().unwrap_or_default();
        let result = mgr.decide_human(&id, ApprovalDecision::Approved, HUMAN_AGENT_ID);
        assert!(result.is_err());
    }

    #[test]
    fn decide_human_non_0a_agent_rejected() {
        let mgr = ApprovalManager::new();
        let id = mgr.submit(l2_request("restart service")).ok().unwrap_or_default();
        let result = mgr.decide_human(&id, ApprovalDecision::Approved, "rogue-agent");
        assert!(result.is_err());
    }

    #[test]
    fn decide_human_not_found() {
        let mgr = ApprovalManager::new();
        let result = mgr.decide_human("bad-id", ApprovalDecision::Approved, HUMAN_AGENT_ID);
        assert!(result.is_err());
    }

    #[test]
    fn decide_human_already_decided_fails() {
        let mgr = ApprovalManager::new();
        let id = mgr.submit(l2_request("restart")).ok().unwrap_or_default();
        let _ = mgr.decide_human(&id, ApprovalDecision::Approved, HUMAN_AGENT_ID);
        // Second decision should fail (no longer Pending)
        let result = mgr.decide_human(&id, ApprovalDecision::Rejected, HUMAN_AGENT_ID);
        assert!(result.is_err());
    }

    // -----------------------------------------------------------------------
    // decide_pbft tests
    // -----------------------------------------------------------------------

    #[test]
    fn decide_pbft_quorum_reached_approves() {
        let mgr = ApprovalManager::new();
        let id = mgr.submit(l3_request("database migration")).ok().unwrap_or_default();
        let result = mgr.decide_pbft(&id, quorum_reached());
        assert!(result.is_ok());
        let status = mgr.get_status(&id).ok();
        assert!(matches!(status, Some(ApprovalStatus::Approved { .. })));
    }

    #[test]
    fn decide_pbft_quorum_failed_rejects() {
        let mgr = ApprovalManager::new();
        let id = mgr.submit(l3_request("database migration")).ok().unwrap_or_default();
        let result = mgr.decide_pbft(&id, quorum_failed());
        assert!(result.is_ok());
        let status = mgr.get_status(&id).ok();
        assert!(matches!(status, Some(ApprovalStatus::Rejected { .. })));
    }

    #[test]
    fn decide_pbft_not_found() {
        let mgr = ApprovalManager::new();
        let result = mgr.decide_pbft("nonexistent", quorum_reached());
        assert!(result.is_err());
    }

    #[test]
    fn decide_pbft_already_decided_fails() {
        let mgr = ApprovalManager::new();
        let id = mgr.submit(l3_request("migration")).ok().unwrap_or_default();
        let _ = mgr.decide_pbft(&id, quorum_reached());
        let result = mgr.decide_pbft(&id, quorum_failed());
        assert!(result.is_err());
    }

    #[test]
    fn decide_pbft_on_l2_still_works() {
        // PBFT can decide on any tier (it's the higher-authority path)
        let mgr = ApprovalManager::new();
        let id = mgr.submit(l2_request("restart")).ok().unwrap_or_default();
        let result = mgr.decide_pbft(&id, quorum_reached());
        assert!(result.is_ok());
    }

    // -----------------------------------------------------------------------
    // get_pending tests
    // -----------------------------------------------------------------------

    #[test]
    fn get_pending_returns_only_pending() {
        let mgr = ApprovalManager::new();
        let id1 = mgr.submit(l2_request("action-1")).ok().unwrap_or_default();
        let _ = mgr.submit(l2_request("action-2"));
        // Approve id1
        let _ = mgr.decide_human(&id1, ApprovalDecision::Approved, HUMAN_AGENT_ID);

        let pending = mgr.get_pending();
        assert_eq!(pending.len(), 1);
        assert_ne!(pending[0].id, id1);
    }

    #[test]
    fn get_pending_empty_initially() {
        let mgr = ApprovalManager::new();
        assert!(mgr.get_pending().is_empty());
    }

    // -----------------------------------------------------------------------
    // expire_timed_out tests
    // -----------------------------------------------------------------------

    #[test]
    fn expire_timed_out_expires_old_requests() {
        let mgr = ApprovalManager::new();
        // Create a request with a submitted_at far in the past and very small timeout
        let mut req = l2_request("old action");
        req.submitted_at = Timestamp::from_raw(0);
        req.timeout_secs = 1; // 1 tick timeout
        let id = mgr.submit(req).ok().unwrap_or_default();

        // Force enough ticks so now() > submitted_at + timeout_secs
        // Timestamp::now() auto-increments, so after several calls it will exceed 1
        let _ = Timestamp::now();
        let _ = Timestamp::now();

        let expired = mgr.expire_timed_out();
        assert!(expired.contains(&id));

        let status = mgr.get_status(&id).ok();
        assert!(matches!(status, Some(ApprovalStatus::Expired)));
    }

    #[test]
    fn expire_timed_out_does_not_expire_fresh_requests() {
        let mgr = ApprovalManager::new();
        let req = l2_request("fresh action");
        let id = mgr.submit(req).ok().unwrap_or_default();

        let expired = mgr.expire_timed_out();
        assert!(!expired.contains(&id));

        let status = mgr.get_status(&id).ok();
        assert!(matches!(status, Some(ApprovalStatus::Pending)));
    }

    #[test]
    fn expire_does_not_touch_already_decided() {
        let mgr = ApprovalManager::new();
        let mut req = l2_request("decided action");
        req.submitted_at = Timestamp::from_raw(0);
        req.timeout_secs = 1;
        let id = mgr.submit(req).ok().unwrap_or_default();

        // Approve before expiration check
        let _ = mgr.decide_human(&id, ApprovalDecision::Approved, HUMAN_AGENT_ID);

        let _ = Timestamp::now();
        let _ = Timestamp::now();

        let expired = mgr.expire_timed_out();
        assert!(!expired.contains(&id));

        let status = mgr.get_status(&id).ok();
        assert!(matches!(status, Some(ApprovalStatus::Approved { .. })));
    }

    // -----------------------------------------------------------------------
    // approval_count tests
    // -----------------------------------------------------------------------

    #[test]
    fn approval_count_includes_all_statuses() {
        let mgr = ApprovalManager::new();
        let id1 = mgr.submit(l2_request("a1")).ok().unwrap_or_default();
        let _ = mgr.submit(l2_request("a2"));

        let _ = mgr.decide_human(&id1, ApprovalDecision::Approved, HUMAN_AGENT_ID);

        // Count includes both the approved and the still-pending
        assert_eq!(mgr.approval_count(), 2);
    }

    #[test]
    fn approval_count_zero_initially() {
        let mgr = ApprovalManager::new();
        assert_eq!(mgr.approval_count(), 0);
    }

    // -----------------------------------------------------------------------
    // audit log tests
    // -----------------------------------------------------------------------

    #[test]
    fn audit_log_records_human_decisions() {
        let mgr = ApprovalManager::new();
        let id = mgr.submit(l2_request("test action")).ok().unwrap_or_default();
        let _ = mgr.decide_human(&id, ApprovalDecision::Approved, HUMAN_AGENT_ID);

        let log = mgr.get_audit_log();
        assert_eq!(log.len(), 1);
        assert_eq!(log[0].request_id, id);
        assert_eq!(log[0].decision, "Approved");
        assert_eq!(log[0].decided_by, HUMAN_AGENT_ID);
    }

    #[test]
    fn audit_log_records_pbft_decisions() {
        let mgr = ApprovalManager::new();
        let id = mgr.submit(l3_request("migration")).ok().unwrap_or_default();
        let _ = mgr.decide_pbft(&id, quorum_reached());

        let log = mgr.get_audit_log();
        assert_eq!(log.len(), 1);
        assert_eq!(log[0].decision, "Approved (PBFT)");
    }

    #[test]
    fn audit_log_records_rejection() {
        let mgr = ApprovalManager::new();
        let id = mgr.submit(l2_request("test")).ok().unwrap_or_default();
        let _ = mgr.decide_human(&id, ApprovalDecision::Rejected, HUMAN_AGENT_ID);

        let log = mgr.get_audit_log();
        assert_eq!(log.len(), 1);
        assert_eq!(log[0].decision, "Rejected");
    }

    #[test]
    fn audit_log_records_deferred() {
        let mgr = ApprovalManager::new();
        let id = mgr.submit(l2_request("test")).ok().unwrap_or_default();
        let _ = mgr.decide_human(
            &id,
            ApprovalDecision::Deferred {
                reason: "investigating".into(),
            },
            HUMAN_AGENT_ID,
        );

        let log = mgr.get_audit_log();
        assert_eq!(log.len(), 1);
        assert!(log[0].decision.starts_with("Deferred:"));
    }

    #[test]
    fn audit_log_records_expiration() {
        let mgr = ApprovalManager::new();
        let mut req = l2_request("expiring");
        req.submitted_at = Timestamp::from_raw(0);
        req.timeout_secs = 1;
        let _ = mgr.submit(req);

        let _ = Timestamp::now();
        let _ = Timestamp::now();

        let _ = mgr.expire_timed_out();

        let log = mgr.get_audit_log();
        assert!(!log.is_empty());
        assert_eq!(log.last().map(|e| e.decision.as_str()), Some("Expired"));
    }

    #[test]
    fn audit_log_capped_at_200() {
        let mgr = ApprovalManager::new();
        // Submit and decide 210 requests to overflow the audit log
        for i in 0..210 {
            let req = l2_request(&format!("action-{i}"));
            if let Ok(id) = mgr.submit(req) {
                let _ = mgr.decide_human(&id, ApprovalDecision::Approved, HUMAN_AGENT_ID);
            }
        }

        let log = mgr.get_audit_log();
        assert!(log.len() <= AUDIT_LOG_CAP);
    }

    // -----------------------------------------------------------------------
    // Config tests
    // -----------------------------------------------------------------------

    #[test]
    fn default_config_values() {
        let config = ApprovalConfig::default();
        assert_eq!(config.max_pending, DEFAULT_MAX_PENDING);
        assert_eq!(config.l2_timeout_secs, DEFAULT_L2_TIMEOUT);
        assert_eq!(config.l3_timeout_secs, DEFAULT_L3_TIMEOUT);
    }

    #[test]
    fn custom_config() {
        let config = ApprovalConfig {
            max_pending: 50,
            l2_timeout_secs: 900,
            l3_timeout_secs: 7200,
        };
        let mgr = ApprovalManager::with_config(config);
        // Config is internal, test via behavior
        for i in 0..50 {
            let result = mgr.submit(l2_request(&format!("req-{i}")));
            assert!(result.is_ok());
        }
        let overflow = mgr.submit(l2_request("overflow"));
        assert!(overflow.is_err());
    }

    // -----------------------------------------------------------------------
    // Concurrent access tests
    // -----------------------------------------------------------------------

    #[test]
    fn concurrent_submits() {
        use std::sync::Arc;
        use std::thread;

        let mgr = Arc::new(ApprovalManager::new());
        let mut handles = Vec::new();

        for i in 0..10 {
            let mgr_clone = Arc::clone(&mgr);
            handles.push(thread::spawn(move || {
                mgr_clone
                    .submit(l2_request(&format!("concurrent-{i}")))
                    .is_ok()
            }));
        }

        let results: Vec<bool> = handles.into_iter().filter_map(|h| h.join().ok()).collect();
        assert_eq!(results.len(), 10);
        assert!(results.iter().all(|&r| r));
        assert_eq!(mgr.approval_count(), 10);
    }

    #[test]
    fn concurrent_submit_and_decide() {
        use std::sync::Arc;
        use std::thread;

        let mgr = Arc::new(ApprovalManager::new());

        // Submit first
        let id = mgr.submit(l2_request("concurrent-action")).ok().unwrap_or_default();

        // Concurrent readers and one writer
        let mgr1 = Arc::clone(&mgr);
        let id1 = id.clone();
        let writer = thread::spawn(move || {
            mgr1.decide_human(&id1, ApprovalDecision::Approved, HUMAN_AGENT_ID)
        });

        let mgr2 = Arc::clone(&mgr);
        let id2 = id.clone();
        let reader = thread::spawn(move || mgr2.get_status(&id2));

        let write_result = writer.join();
        let read_result = reader.join();

        assert!(write_result.is_ok());
        assert!(read_result.is_ok());
    }

    // -----------------------------------------------------------------------
    // Debug impl tests
    // -----------------------------------------------------------------------

    #[test]
    fn debug_impl() {
        let mgr = ApprovalManager::new();
        let debug_str = format!("{mgr:?}");
        assert!(debug_str.contains("ApprovalManager"));
        assert!(debug_str.contains("pending_count"));
    }

    // -----------------------------------------------------------------------
    // Default impl tests
    // -----------------------------------------------------------------------

    #[test]
    fn default_impl() {
        let mgr = ApprovalManager::default();
        assert_eq!(mgr.approval_count(), 0);
    }

    // -----------------------------------------------------------------------
    // make_request helper tests
    // -----------------------------------------------------------------------

    #[test]
    fn make_request_l2_default_timeout() {
        let req = make_request("test", EscalationTier::L2RequireApproval, "mod");
        assert_eq!(req.timeout_secs, DEFAULT_L2_TIMEOUT);
    }

    #[test]
    fn make_request_l3_default_timeout() {
        let req = make_request("test", EscalationTier::L3PbftConsensus, "mod");
        assert_eq!(req.timeout_secs, DEFAULT_L3_TIMEOUT);
    }

    #[test]
    fn make_request_sets_uuid() {
        let req = make_request("test", EscalationTier::L2RequireApproval, "mod");
        assert_eq!(req.id.len(), 36); // UUID v4 format
    }

    // -----------------------------------------------------------------------
    // EscalationTier interaction tests
    // -----------------------------------------------------------------------

    #[test]
    fn l0_request_can_be_submitted() {
        let mgr = ApprovalManager::new();
        let req = make_request("auto-exec", EscalationTier::L0AutoExecute, "system");
        let result = mgr.submit(req);
        assert!(result.is_ok());
    }

    #[test]
    fn l1_request_can_be_submitted() {
        let mgr = ApprovalManager::new();
        let req = make_request("notify", EscalationTier::L1NotifyHuman, "system");
        let result = mgr.submit(req);
        assert!(result.is_ok());
    }

    #[test]
    fn l1_request_can_be_human_decided() {
        let mgr = ApprovalManager::new();
        let req = make_request("notify", EscalationTier::L1NotifyHuman, "system");
        let id = mgr.submit(req).ok().unwrap_or_default();
        let result = mgr.decide_human(&id, ApprovalDecision::Approved, HUMAN_AGENT_ID);
        assert!(result.is_ok());
    }

    // -----------------------------------------------------------------------
    // Edge case tests
    // -----------------------------------------------------------------------

    #[test]
    fn decide_on_expired_request_fails() {
        let mgr = ApprovalManager::new();
        let mut req = l2_request("will-expire");
        req.submitted_at = Timestamp::from_raw(0);
        req.timeout_secs = 1;
        let id = mgr.submit(req).ok().unwrap_or_default();

        let _ = Timestamp::now();
        let _ = Timestamp::now();

        let _ = mgr.expire_timed_out();

        // Try to decide on expired
        let result = mgr.decide_human(&id, ApprovalDecision::Approved, HUMAN_AGENT_ID);
        assert!(result.is_err());
    }

    #[test]
    fn multiple_submits_different_tiers() {
        let mgr = ApprovalManager::new();
        let _ = mgr.submit(make_request("a", EscalationTier::L0AutoExecute, "m1"));
        let _ = mgr.submit(make_request("b", EscalationTier::L1NotifyHuman, "m2"));
        let _ = mgr.submit(make_request("c", EscalationTier::L2RequireApproval, "m3"));
        let _ = mgr.submit(make_request("d", EscalationTier::L3PbftConsensus, "m4"));
        assert_eq!(mgr.approval_count(), 4);
        assert_eq!(mgr.get_pending().len(), 4);
    }

    #[test]
    fn pbft_rejection_includes_vote_counts() {
        let mgr = ApprovalManager::new();
        let id = mgr.submit(l3_request("migration")).ok().unwrap_or_default();
        let outcome = ConsensusOutcome {
            proposal_id: "p-1".into(),
            quorum_reached: false,
            votes_for: 5,
            votes_against: 35,
        };
        let _ = mgr.decide_pbft(&id, outcome);

        let status = mgr.get_status(&id).ok();
        if let Some(ApprovalStatus::Rejected { reason, .. }) = status {
            assert!(reason.contains('5'));
            assert!(reason.contains("35"));
        } else {
            panic!("expected Rejected status");
        }
    }

    #[test]
    fn confidence_preserved_in_request() {
        let mgr = ApprovalManager::new();
        let mut req = l2_request("test");
        req.confidence = 0.87;
        let id = mgr.submit(req).ok().unwrap_or_default();

        let pending = mgr.get_pending();
        let found = pending.iter().find(|r| r.id == id);
        assert!(found.is_some());
        let delta = (found.map_or(0.0, |r| r.confidence) - 0.87).abs();
        assert!(delta < f64::EPSILON);
    }

    #[test]
    fn severity_preserved_in_request() {
        let mgr = ApprovalManager::new();
        let mut req = l2_request("test");
        req.severity = "critical".to_owned();
        let id = mgr.submit(req).ok().unwrap_or_default();

        let pending = mgr.get_pending();
        let found = pending.iter().find(|r| r.id == id);
        assert_eq!(found.map(|r| r.severity.as_str()), Some("critical"));
    }

    #[test]
    fn audit_log_tracks_action_description() {
        let mgr = ApprovalManager::new();
        let id = mgr
            .submit(l2_request("vacuum remediation_log.db"))
            .ok()
            .unwrap_or_default();
        let _ = mgr.decide_human(&id, ApprovalDecision::Approved, HUMAN_AGENT_ID);

        let log = mgr.get_audit_log();
        assert_eq!(log[0].action, "vacuum remediation_log.db");
    }
}
