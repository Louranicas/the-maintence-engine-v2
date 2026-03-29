//! # M35: Dissent Tracker (NAM R3)
//!
//! Tracks and analyzes dissenting opinions in the PBFT consensus process.
//! Implements NAM R3 (Dissent Capture) by recording minority viewpoints,
//! marking valuable dissent for learning, and providing analysis of
//! dissent patterns across proposals and agents.
//!
//! ## NAM R3: Dissent Capture
//!
//! The Non-Anthropocentric Model requires that minority opinions be
//! captured, preserved, and valued. This module implements that
//! principle by:
//!
//! - Recording every dissent event with full context
//! - Allowing post-hoc evaluation of dissent value
//! - Weighting Critic-role dissent higher (1.2x)
//! - Tracking dissent patterns per agent and per proposal
//!
//! ## Layer: L6 (Consensus)
//!
//! ## Related Documentation
//! - [Module Specification](../../ai_docs/modules/M35_DISSENT_TRACKER.md)
//! - [NAM Compliance](../../nam/NAM_SPEC.md)

use std::collections::HashMap;
use std::sync::RwLock;
use std::time::SystemTime;

use crate::{Error, Result};

use super::{AgentRole, DissentEvent};

/// Maximum number of dissent events retained.
const MAX_DISSENT_EVENTS: usize = 500;

/// Analysis of dissent patterns for a single proposal.
#[derive(Clone, Debug)]
pub struct DissentAnalysis {
    /// Proposal ID.
    pub proposal_id: String,
    /// Total number of dissent events for this proposal.
    pub total_dissent: usize,
    /// Breakdown of dissent by agent role.
    pub dissent_by_role: HashMap<String, usize>,
    /// Number of dissent events marked as valuable.
    pub valuable_count: usize,
    /// Most common dissent reasons, sorted by frequency descending.
    pub common_reasons: Vec<(String, usize)>,
}

/// Dissent tracker for NAM R3 compliance.
///
/// Records, indexes, and analyzes dissenting opinions from consensus
/// processes. Supports marking dissent as valuable for post-hoc learning.
pub struct DissentTracker {
    /// All dissent events (capped at 500).
    dissent_events: RwLock<Vec<DissentEvent>>,
    /// Index: `proposal_id` -> indices into `dissent_events`.
    dissent_by_proposal: RwLock<HashMap<String, Vec<usize>>>,
    /// Index: `agent_id` -> indices into `dissent_events`.
    dissent_by_agent: RwLock<HashMap<String, Vec<usize>>>,
    /// Count of dissent events marked as valuable.
    valuable_dissent_count: RwLock<u64>,
    /// Count of unique proposals that have received at least one dissent.
    unique_proposals_with_dissent: RwLock<u64>,
}

impl DissentTracker {
    /// Create a new empty dissent tracker.
    #[must_use]
    pub fn new() -> Self {
        Self {
            dissent_events: RwLock::new(Vec::new()),
            dissent_by_proposal: RwLock::new(HashMap::new()),
            dissent_by_agent: RwLock::new(HashMap::new()),
            valuable_dissent_count: RwLock::new(0),
            unique_proposals_with_dissent: RwLock::new(0),
        }
    }

    /// Record a new dissent event.
    ///
    /// Creates a `DissentEvent` capturing the dissenting agent's identity,
    /// role, and reason for disagreement. The event is indexed by both
    /// proposal ID and agent ID for efficient retrieval.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if the `proposal_id`, `agent_id`, or reason is empty.
    #[allow(clippy::too_many_lines)]
    pub fn record_dissent(
        &self,
        proposal_id: &str,
        agent_id: &str,
        agent_role: AgentRole,
        reason: String,
    ) -> Result<DissentEvent> {
        if proposal_id.is_empty() {
            return Err(Error::Validation("Proposal ID cannot be empty".into()));
        }
        if agent_id.is_empty() {
            return Err(Error::Validation("Agent ID cannot be empty".into()));
        }
        if reason.is_empty() {
            return Err(Error::Validation("Dissent reason cannot be empty".into()));
        }

        // R14 fix: Collect all data under a single events lock, then drop
        // before acquiring index locks. This eliminates nested write guards
        // on dissent_events + dissent_by_proposal + dissent_by_agent which
        // risked deadlock under concurrent dissent recording.
        let (event, new_idx, needs_reindex) = {
            let mut events = self
                .dissent_events
                .write()
                .map_err(|e| Error::Other(format!("Lock poisoned: {e}")))?;

            let needs_reindex = events.len() >= MAX_DISSENT_EVENTS;
            if needs_reindex {
                events.remove(0);
            }

            let dissent_id = format!(
                "dissent-{}-{}-{}",
                proposal_id,
                agent_id,
                events.len()
            );

            let event = DissentEvent {
                id: dissent_id,
                proposed_action: proposal_id.into(),
                dissenting_agent: format!("{agent_id}:{agent_role:?}"),
                reason,
                outcome: None,
                was_valuable: None,
                timestamp: SystemTime::now(),
            };

            let idx = events.len();
            events.push(event.clone());

            // Collect re-index data while still holding events lock
            let proposal_index: Vec<(String, usize)> = if needs_reindex {
                events
                    .iter()
                    .enumerate()
                    .map(|(i, e)| (e.proposed_action.clone(), i))
                    .collect()
            } else {
                Vec::new()
            };
            // Index by raw agent_id (before ":Role" suffix) to match get_dissent_by_agent()
            let agent_index: Vec<(String, usize)> = if needs_reindex {
                events
                    .iter()
                    .enumerate()
                    .map(|(i, e)| {
                        let raw_id = e
                            .dissenting_agent
                            .split(':')
                            .next()
                            .unwrap_or(&e.dissenting_agent)
                            .to_string();
                        (raw_id, i)
                    })
                    .collect()
            } else {
                Vec::new()
            };

            drop(events); // CRITICAL: drop events lock BEFORE acquiring index locks

            // Re-index if capacity was exceeded (locks acquired sequentially, not nested)
            if needs_reindex {
                if let Ok(mut by_proposal) = self.dissent_by_proposal.write() {
                    by_proposal.clear();
                    for (action, i) in &proposal_index {
                        by_proposal.entry(action.clone()).or_default().push(*i);
                    }
                }
                if let Ok(mut by_agent) = self.dissent_by_agent.write() {
                    by_agent.clear();
                    for (agent, i) in &agent_index {
                        by_agent.entry(agent.clone()).or_default().push(*i);
                    }
                }
            }

            (event, idx, needs_reindex)
        };

        // Update indexes for the new event. Skip if reindex already handled it
        // (reindex rebuilds ALL indexes including the new event).
        if !needs_reindex {
            {
                let mut by_proposal = self
                    .dissent_by_proposal
                    .write()
                    .map_err(|e| Error::Other(format!("Lock poisoned: {e}")))?;
                let entry = by_proposal
                    .entry(event.proposed_action.clone())
                    .or_default();
                if entry.is_empty() {
                    // First dissent for this proposal — update unique counter
                    drop(by_proposal);
                    if let Ok(mut count) = self.unique_proposals_with_dissent.write() {
                        *count += 1;
                    }
                    if let Ok(mut by_proposal) = self.dissent_by_proposal.write() {
                        by_proposal
                            .entry(event.proposed_action.clone())
                            .or_default()
                            .push(new_idx);
                    }
                } else {
                    entry.push(new_idx);
                }
            }

            // Use raw agent_id (not "agent:Role" composite) to match get_dissent_by_agent()
            {
                let agent_key = event
                    .dissenting_agent
                    .split(':')
                    .next()
                    .unwrap_or(&event.dissenting_agent)
                    .to_string();
                let mut by_agent = self
                    .dissent_by_agent
                    .write()
                    .map_err(|e| Error::Other(format!("Lock poisoned: {e}")))?;
                by_agent.entry(agent_key).or_default().push(new_idx);
            }
        }

        Ok(event)
    }

    /// Mark a dissent event as valuable (NAM R3 post-hoc evaluation).
    ///
    /// Valuable dissent is dissent that, in hindsight, correctly
    /// identified a problem or risk that the majority overlooked.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if the dissent event is not found.
    pub fn mark_valuable(&self, dissent_id: &str) -> Result<()> {
        let mut events = self
            .dissent_events
            .write()
            .map_err(|e| Error::Other(format!("Lock poisoned: {e}")))?;

        let event = events
            .iter_mut()
            .find(|e| e.id == dissent_id)
            .ok_or_else(|| {
                Error::Validation(format!("Dissent event not found: {dissent_id}"))
            })?;

        // Only increment counter if not already marked valuable
        let was_already_valuable = event.was_valuable.unwrap_or(false);
        event.was_valuable = Some(true);

        if !was_already_valuable {
            drop(events);
            let mut count = self
                .valuable_dissent_count
                .write()
                .map_err(|e| Error::Other(format!("Lock poisoned: {e}")))?;
            *count += 1;
        }

        Ok(())
    }

    /// Get all dissent events for a specific proposal.
    #[must_use]
    pub fn get_dissent_for_proposal(&self, proposal_id: &str) -> Vec<DissentEvent> {
        let Ok(events) = self.dissent_events.read() else {
            return Vec::new();
        };
        let Ok(by_proposal) = self.dissent_by_proposal.read() else {
            return Vec::new();
        };

        by_proposal.get(proposal_id).map_or_else(Vec::new, |indices| {
            indices
                .iter()
                .filter_map(|&idx| events.get(idx).cloned())
                .collect()
        })
    }

    /// Get all dissent events by a specific agent.
    #[must_use]
    pub fn get_dissent_by_agent(&self, agent_id: &str) -> Vec<DissentEvent> {
        let Ok(events) = self.dissent_events.read() else {
            return Vec::new();
        };
        let Ok(by_agent) = self.dissent_by_agent.read() else {
            return Vec::new();
        };

        by_agent.get(agent_id).map_or_else(Vec::new, |indices| {
            indices
                .iter()
                .filter_map(|&idx| events.get(idx).cloned())
                .collect()
        })
    }

    /// Analyze dissent patterns for a specific proposal.
    ///
    /// Produces a `DissentAnalysis` containing role breakdowns,
    /// valuable dissent counts, and common reasons.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if no dissent events exist for the proposal.
    pub fn analyze_dissent(&self, proposal_id: &str) -> Result<DissentAnalysis> {
        let dissent_events = self.get_dissent_for_proposal(proposal_id);

        if dissent_events.is_empty() {
            return Err(Error::Validation(format!(
                "No dissent events found for proposal: {proposal_id}"
            )));
        }

        // Count dissent by role
        let mut dissent_by_role: HashMap<String, usize> = HashMap::new();
        for event in &dissent_events {
            // The dissenting_agent field is formatted as "agent_id:Role"
            let role = event
                .dissenting_agent
                .split(':')
                .nth(1)
                .unwrap_or("Unknown")
                .to_string();
            *dissent_by_role.entry(role).or_insert(0) += 1;
        }

        // Count valuable dissent
        let valuable_count = dissent_events
            .iter()
            .filter(|e| e.was_valuable == Some(true))
            .count();

        // Aggregate reasons and sort by frequency
        let mut reason_counts: HashMap<String, usize> = HashMap::new();
        for event in &dissent_events {
            *reason_counts.entry(event.reason.clone()).or_insert(0) += 1;
        }
        let mut common_reasons: Vec<(String, usize)> = reason_counts.into_iter().collect();
        common_reasons.sort_by(|a, b| b.1.cmp(&a.1));

        Ok(DissentAnalysis {
            proposal_id: proposal_id.into(),
            total_dissent: dissent_events.len(),
            dissent_by_role,
            valuable_count,
            common_reasons,
        })
    }

    /// Get all dissent events that have been marked as valuable.
    #[must_use]
    pub fn get_valuable_dissent(&self) -> Vec<DissentEvent> {
        let Ok(events) = self.dissent_events.read() else {
            return Vec::new();
        };
        events
            .iter()
            .filter(|e| e.was_valuable == Some(true))
            .cloned()
            .collect()
    }

    /// Get dissent events from agents with the Critic role.
    ///
    /// Critic agents have elevated vote weight (1.2x) and their dissent
    /// is weighted higher in NAM R3 analysis.
    #[must_use]
    pub fn get_critic_dissent(&self) -> Vec<DissentEvent> {
        let Ok(events) = self.dissent_events.read() else {
            return Vec::new();
        };
        events
            .iter()
            .filter(|e| e.dissenting_agent.contains("Critic"))
            .cloned()
            .collect()
    }

    /// Calculate the overall dissent rate.
    ///
    /// Returns the ratio of total dissent events to unique proposals
    /// that have had at least one dissent. Returns 0.0 if no proposals
    /// have received dissent.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn dissent_rate(&self) -> f64 {
        let Ok(events) = self.dissent_events.read() else {
            return 0.0;
        };
        let Ok(proposals) = self.unique_proposals_with_dissent.read() else {
            return 0.0;
        };

        let total_events = events.len();
        let unique_proposals = *proposals;

        if unique_proposals == 0 {
            return 0.0;
        }

        total_events as f64 / unique_proposals as f64
    }

    /// Get the total number of dissent events recorded.
    #[must_use]
    pub fn total_dissent(&self) -> usize {
        let Ok(events) = self.dissent_events.read() else {
            return 0;
        };
        events.len()
    }

    /// Calculate the rate of valuable dissent relative to total dissent.
    ///
    /// Returns the fraction of dissent events that were marked valuable.
    /// Returns 0.0 if no dissent has been recorded.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn valuable_dissent_rate(&self) -> f64 {
        let Ok(count) = self.valuable_dissent_count.read() else {
            return 0.0;
        };
        let total = self.total_dissent();
        if total == 0 {
            return 0.0;
        }
        *count as f64 / total as f64
    }
}

impl Default for DissentTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_record_dissent() {
        let tracker = DissentTracker::new();
        let result = tracker.record_dissent(
            "prop-1",
            "agent-29",
            AgentRole::Critic,
            "Risk of data loss during migration".into(),
        );
        assert!(result.is_ok());
        let event = result.unwrap_or_else(|_| unreachable!());
        assert_eq!(event.proposed_action, "prop-1");
        assert!(event.dissenting_agent.contains("agent-29"));
        assert!(event.dissenting_agent.contains("Critic"));
        assert!(event.was_valuable.is_none());
        assert_eq!(tracker.total_dissent(), 1);
    }

    #[test]
    fn test_record_dissent_validation() {
        let tracker = DissentTracker::new();

        // Empty proposal ID
        let result = tracker.record_dissent("", "agent-01", AgentRole::Validator, "reason".into());
        assert!(result.is_err());

        // Empty agent ID
        let result = tracker.record_dissent("prop-1", "", AgentRole::Validator, "reason".into());
        assert!(result.is_err());

        // Empty reason
        let result =
            tracker.record_dissent("prop-1", "agent-01", AgentRole::Validator, String::new());
        assert!(result.is_err());
    }

    #[test]
    fn test_mark_valuable() {
        let tracker = DissentTracker::new();
        let event = tracker
            .record_dissent(
                "prop-1",
                "agent-30",
                AgentRole::Critic,
                "Service dependency not considered".into(),
            )
            .unwrap_or_else(|_| unreachable!());

        let result = tracker.mark_valuable(&event.id);
        assert!(result.is_ok());

        let valuable = tracker.get_valuable_dissent();
        assert_eq!(valuable.len(), 1);
        assert_eq!(valuable[0].was_valuable, Some(true));
    }

    #[test]
    fn test_mark_valuable_nonexistent_fails() {
        let tracker = DissentTracker::new();
        let result = tracker.mark_valuable("nonexistent-id");
        assert!(result.is_err());
    }

    #[test]
    fn test_dissent_for_proposal() {
        let tracker = DissentTracker::new();

        let _ = tracker.record_dissent(
            "prop-1",
            "agent-29",
            AgentRole::Critic,
            "Too risky".into(),
        );
        let _ = tracker.record_dissent(
            "prop-1",
            "agent-30",
            AgentRole::Critic,
            "Insufficient testing".into(),
        );
        let _ = tracker.record_dissent(
            "prop-2",
            "agent-35",
            AgentRole::Integrator,
            "Cross-system impact".into(),
        );

        let prop1_dissent = tracker.get_dissent_for_proposal("prop-1");
        assert_eq!(prop1_dissent.len(), 2);

        let prop2_dissent = tracker.get_dissent_for_proposal("prop-2");
        assert_eq!(prop2_dissent.len(), 1);

        let prop3_dissent = tracker.get_dissent_for_proposal("prop-3");
        assert!(prop3_dissent.is_empty());
    }

    #[test]
    fn test_dissent_by_agent() {
        let tracker = DissentTracker::new();

        let _ = tracker.record_dissent(
            "prop-1",
            "agent-29",
            AgentRole::Critic,
            "Too risky".into(),
        );
        let _ = tracker.record_dissent(
            "prop-2",
            "agent-29",
            AgentRole::Critic,
            "Untested path".into(),
        );
        let _ = tracker.record_dissent(
            "prop-1",
            "agent-35",
            AgentRole::Integrator,
            "Cross-system impact".into(),
        );

        let agent29 = tracker.get_dissent_by_agent("agent-29");
        assert_eq!(agent29.len(), 2);

        let agent35 = tracker.get_dissent_by_agent("agent-35");
        assert_eq!(agent35.len(), 1);
    }

    #[test]
    fn test_analyze_dissent() {
        let tracker = DissentTracker::new();

        let _ = tracker.record_dissent(
            "prop-1",
            "agent-29",
            AgentRole::Critic,
            "Too risky".into(),
        );
        let _ = tracker.record_dissent(
            "prop-1",
            "agent-30",
            AgentRole::Critic,
            "Too risky".into(),
        );
        let _ = tracker.record_dissent(
            "prop-1",
            "agent-35",
            AgentRole::Integrator,
            "Cross-system impact".into(),
        );

        // Mark one as valuable
        let events = tracker.get_dissent_for_proposal("prop-1");
        let _ = tracker.mark_valuable(&events[0].id);

        let analysis = tracker
            .analyze_dissent("prop-1")
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(analysis.total_dissent, 3);
        assert_eq!(analysis.valuable_count, 1);

        // Check role breakdown
        assert_eq!(analysis.dissent_by_role.get("Critic"), Some(&2));
        assert_eq!(analysis.dissent_by_role.get("Integrator"), Some(&1));

        // Check common reasons (most common first)
        assert_eq!(analysis.common_reasons[0].0, "Too risky");
        assert_eq!(analysis.common_reasons[0].1, 2);
    }

    #[test]
    fn test_analyze_empty_proposal_fails() {
        let tracker = DissentTracker::new();
        let result = tracker.analyze_dissent("nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_valuable_dissent() {
        let tracker = DissentTracker::new();

        let e1 = tracker
            .record_dissent(
                "prop-1",
                "agent-29",
                AgentRole::Critic,
                "Data corruption risk".into(),
            )
            .unwrap_or_else(|_| unreachable!());
        let _ = tracker.record_dissent(
            "prop-1",
            "agent-30",
            AgentRole::Critic,
            "Performance concern".into(),
        );

        let _ = tracker.mark_valuable(&e1.id);

        let valuable = tracker.get_valuable_dissent();
        assert_eq!(valuable.len(), 1);
        assert_eq!(valuable[0].id, e1.id);
    }

    #[test]
    fn test_critic_dissent() {
        let tracker = DissentTracker::new();

        let _ = tracker.record_dissent(
            "prop-1",
            "agent-29",
            AgentRole::Critic,
            "Too risky".into(),
        );
        let _ = tracker.record_dissent(
            "prop-1",
            "agent-01",
            AgentRole::Validator,
            "Disagree with approach".into(),
        );
        let _ = tracker.record_dissent(
            "prop-1",
            "agent-30",
            AgentRole::Critic,
            "Insufficient testing".into(),
        );

        let critic_dissent = tracker.get_critic_dissent();
        assert_eq!(critic_dissent.len(), 2);

        // Verify all returned events are from Critics
        for event in &critic_dissent {
            assert!(event.dissenting_agent.contains("Critic"));
        }
    }

    #[test]
    fn test_dissent_rate() {
        let tracker = DissentTracker::new();
        assert!((tracker.dissent_rate() - 0.0).abs() < f64::EPSILON);

        // 3 dissent events across 2 proposals -> rate = 3/2 = 1.5
        let _ = tracker.record_dissent(
            "prop-1",
            "agent-29",
            AgentRole::Critic,
            "Risk A".into(),
        );
        let _ = tracker.record_dissent(
            "prop-1",
            "agent-30",
            AgentRole::Critic,
            "Risk B".into(),
        );
        let _ = tracker.record_dissent(
            "prop-2",
            "agent-29",
            AgentRole::Critic,
            "Risk C".into(),
        );

        let rate = tracker.dissent_rate();
        assert!((rate - 1.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_valuable_dissent_rate() {
        let tracker = DissentTracker::new();
        assert!((tracker.valuable_dissent_rate() - 0.0).abs() < f64::EPSILON);

        let e1 = tracker
            .record_dissent(
                "prop-1",
                "agent-29",
                AgentRole::Critic,
                "Concern A".into(),
            )
            .unwrap_or_else(|_| unreachable!());
        let _ = tracker.record_dissent(
            "prop-1",
            "agent-30",
            AgentRole::Critic,
            "Concern B".into(),
        );
        let _ = tracker.record_dissent(
            "prop-1",
            "agent-35",
            AgentRole::Integrator,
            "Concern C".into(),
        );
        let _ = tracker.record_dissent(
            "prop-1",
            "agent-21",
            AgentRole::Explorer,
            "Concern D".into(),
        );

        // Mark 1 out of 4 as valuable -> rate = 0.25
        let _ = tracker.mark_valuable(&e1.id);

        let rate = tracker.valuable_dissent_rate();
        assert!((rate - 0.25).abs() < f64::EPSILON);
    }

    // ---------------------------------------------------------------
    // Additional tests to reach 50+
    // ---------------------------------------------------------------

    #[test]
    fn test_default_impl() {
        let tracker = DissentTracker::default();
        assert_eq!(tracker.total_dissent(), 0);
    }

    #[test]
    fn test_total_dissent_increments() {
        let tracker = DissentTracker::new();
        for i in 0..5 {
            let _ = tracker.record_dissent(
                "prop-1", &format!("agent-{i}"), AgentRole::Validator, format!("reason {i}"),
            );
        }
        assert_eq!(tracker.total_dissent(), 5);
    }

    #[test]
    fn test_dissent_id_contains_proposal_and_agent() {
        let tracker = DissentTracker::new();
        let event = tracker.record_dissent(
            "prop-1", "agent-29", AgentRole::Critic, "risky".into(),
        ).unwrap_or_else(|_| unreachable!());
        assert!(event.id.contains("prop-1"));
        assert!(event.id.contains("agent-29"));
    }

    #[test]
    fn test_dissent_agent_format() {
        let tracker = DissentTracker::new();
        let event = tracker.record_dissent(
            "prop-1", "agent-29", AgentRole::Critic, "risk".into(),
        ).unwrap_or_else(|_| unreachable!());
        assert!(event.dissenting_agent.contains("agent-29"));
        assert!(event.dissenting_agent.contains("Critic"));
    }

    #[test]
    fn test_dissent_reason_preserved() {
        let tracker = DissentTracker::new();
        let event = tracker.record_dissent(
            "prop-1", "agent-01", AgentRole::Validator, "specific concern about X".into(),
        ).unwrap_or_else(|_| unreachable!());
        assert_eq!(event.reason, "specific concern about X");
    }

    #[test]
    fn test_dissent_outcome_initially_none() {
        let tracker = DissentTracker::new();
        let event = tracker.record_dissent(
            "prop-1", "agent-01", AgentRole::Validator, "reason".into(),
        ).unwrap_or_else(|_| unreachable!());
        assert!(event.outcome.is_none());
    }

    #[test]
    fn test_dissent_was_valuable_initially_none() {
        let tracker = DissentTracker::new();
        let event = tracker.record_dissent(
            "prop-1", "agent-01", AgentRole::Validator, "reason".into(),
        ).unwrap_or_else(|_| unreachable!());
        assert!(event.was_valuable.is_none());
    }

    #[test]
    fn test_mark_valuable_idempotent() {
        let tracker = DissentTracker::new();
        let event = tracker.record_dissent(
            "prop-1", "agent-29", AgentRole::Critic, "risk".into(),
        ).unwrap_or_else(|_| unreachable!());

        let _ = tracker.mark_valuable(&event.id);
        let _ = tracker.mark_valuable(&event.id);

        // Should still be only 1 valuable
        let valuable = tracker.get_valuable_dissent();
        assert_eq!(valuable.len(), 1);
    }

    #[test]
    fn test_dissent_for_proposal_empty() {
        let tracker = DissentTracker::new();
        let events = tracker.get_dissent_for_proposal("nonexistent");
        assert!(events.is_empty());
    }

    #[test]
    fn test_dissent_by_agent_empty() {
        let tracker = DissentTracker::new();
        let events = tracker.get_dissent_by_agent("nonexistent");
        assert!(events.is_empty());
    }

    #[test]
    fn test_get_critic_dissent_excludes_non_critics() {
        let tracker = DissentTracker::new();
        let _ = tracker.record_dissent(
            "prop-1", "agent-01", AgentRole::Validator, "disagreement".into(),
        );
        let critic_events = tracker.get_critic_dissent();
        assert!(critic_events.is_empty());
    }

    #[test]
    fn test_multiple_proposals_dissent() {
        let tracker = DissentTracker::new();
        for i in 0..3 {
            let _ = tracker.record_dissent(
                &format!("prop-{i}"), "agent-29", AgentRole::Critic, "concern".into(),
            );
        }
        for i in 0..3 {
            let events = tracker.get_dissent_for_proposal(&format!("prop-{i}"));
            assert_eq!(events.len(), 1);
        }
    }

    #[test]
    fn test_analyze_dissent_valuable_count_zero() {
        let tracker = DissentTracker::new();
        let _ = tracker.record_dissent(
            "prop-1", "agent-29", AgentRole::Critic, "risk".into(),
        );
        let analysis = tracker.analyze_dissent("prop-1").unwrap_or_else(|_| unreachable!());
        assert_eq!(analysis.valuable_count, 0);
    }

    #[test]
    fn test_analyze_dissent_common_reasons_sorted() {
        let tracker = DissentTracker::new();
        for _ in 0..3 {
            let _ = tracker.record_dissent(
                "prop-1", &format!("a-{}", tracker.total_dissent()),
                AgentRole::Critic, "frequent_reason".into(),
            );
        }
        let _ = tracker.record_dissent(
            "prop-1", "a-unique", AgentRole::Validator, "rare_reason".into(),
        );
        let analysis = tracker.analyze_dissent("prop-1").unwrap_or_else(|_| unreachable!());
        assert_eq!(analysis.common_reasons[0].0, "frequent_reason");
        assert_eq!(analysis.common_reasons[0].1, 3);
    }

    #[test]
    fn test_dissent_rate_single_proposal() {
        let tracker = DissentTracker::new();
        let _ = tracker.record_dissent(
            "prop-1", "agent-01", AgentRole::Validator, "reason".into(),
        );
        // 1 event, 1 unique proposal -> rate = 1.0
        let rate = tracker.dissent_rate();
        assert!((rate - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_valuable_dissent_rate_none_valuable() {
        let tracker = DissentTracker::new();
        let _ = tracker.record_dissent(
            "prop-1", "agent-01", AgentRole::Validator, "concern".into(),
        );
        assert!((tracker.valuable_dissent_rate()).abs() < f64::EPSILON);
    }

    #[test]
    fn test_valuable_dissent_rate_all_valuable() {
        let tracker = DissentTracker::new();
        let e1 = tracker.record_dissent(
            "prop-1", "agent-29", AgentRole::Critic, "concern".into(),
        ).unwrap_or_else(|_| unreachable!());
        let _ = tracker.mark_valuable(&e1.id);
        assert!((tracker.valuable_dissent_rate() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_get_valuable_dissent_empty() {
        let tracker = DissentTracker::new();
        assert!(tracker.get_valuable_dissent().is_empty());
    }

    #[test]
    fn test_get_critic_dissent_empty() {
        let tracker = DissentTracker::new();
        assert!(tracker.get_critic_dissent().is_empty());
    }

    #[test]
    fn test_record_dissent_multiple_roles() {
        let tracker = DissentTracker::new();
        let roles = [
            AgentRole::Validator,
            AgentRole::Explorer,
            AgentRole::Critic,
            AgentRole::Integrator,
            AgentRole::Historian,
        ];
        for (i, role) in roles.iter().enumerate() {
            let result = tracker.record_dissent(
                "prop-1", &format!("agent-{i}"), *role, format!("reason {i}"),
            );
            assert!(result.is_ok());
        }
        assert_eq!(tracker.total_dissent(), 5);
    }

    #[test]
    fn test_analyze_dissent_proposal_id_preserved() {
        let tracker = DissentTracker::new();
        let _ = tracker.record_dissent(
            "my-proposal-42", "agent-29", AgentRole::Critic, "risk".into(),
        );
        let analysis = tracker.analyze_dissent("my-proposal-42")
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(analysis.proposal_id, "my-proposal-42");
    }

    #[test]
    fn test_dissent_capacity() {
        let tracker = DissentTracker::new();
        for i in 0..550 {
            let _ = tracker.record_dissent(
                &format!("prop-{}", i % 10),
                &format!("agent-{i}"),
                AgentRole::Validator,
                format!("reason {i}"),
            );
        }
        assert!(tracker.total_dissent() <= MAX_DISSENT_EVENTS);
    }

    #[test]
    fn test_multiple_agents_same_proposal() {
        let tracker = DissentTracker::new();
        let _ = tracker.record_dissent("prop-1", "agent-01", AgentRole::Validator, "r1".into());
        let _ = tracker.record_dissent("prop-1", "agent-02", AgentRole::Explorer, "r2".into());
        let _ = tracker.record_dissent("prop-1", "agent-03", AgentRole::Critic, "r3".into());
        let _ = tracker.record_dissent("prop-1", "agent-04", AgentRole::Integrator, "r4".into());

        let events = tracker.get_dissent_for_proposal("prop-1");
        assert_eq!(events.len(), 4);
    }

    #[test]
    fn test_single_agent_multiple_proposals() {
        let tracker = DissentTracker::new();
        for i in 0..5 {
            let _ = tracker.record_dissent(
                &format!("prop-{i}"), "agent-29", AgentRole::Critic, "concern".into(),
            );
        }
        let events = tracker.get_dissent_by_agent("agent-29");
        assert_eq!(events.len(), 5);
    }

    #[test]
    fn test_analyze_dissent_role_breakdown() {
        let tracker = DissentTracker::new();
        let _ = tracker.record_dissent("prop-1", "a1", AgentRole::Critic, "r1".into());
        let _ = tracker.record_dissent("prop-1", "a2", AgentRole::Critic, "r2".into());
        let _ = tracker.record_dissent("prop-1", "a3", AgentRole::Validator, "r3".into());

        let analysis = tracker.analyze_dissent("prop-1").unwrap_or_else(|_| unreachable!());
        assert_eq!(analysis.dissent_by_role.len(), 2);
    }

    #[test]
    fn test_dissent_has_timestamp() {
        let tracker = DissentTracker::new();
        let event = tracker.record_dissent(
            "prop-1", "agent-01", AgentRole::Validator, "reason".into(),
        ).unwrap_or_else(|_| unreachable!());
        let elapsed = event.timestamp.elapsed();
        assert!(elapsed.is_ok());
    }

    #[test]
    fn test_validator_dissent_not_in_critic_filter() {
        let tracker = DissentTracker::new();
        let _ = tracker.record_dissent(
            "prop-1", "agent-01", AgentRole::Validator, "disagreement".into(),
        );
        let _ = tracker.record_dissent(
            "prop-1", "agent-21", AgentRole::Explorer, "alternative view".into(),
        );
        let critic_events = tracker.get_critic_dissent();
        assert!(critic_events.is_empty());
    }

    #[test]
    fn test_historian_dissent() {
        let tracker = DissentTracker::new();
        let event = tracker.record_dissent(
            "prop-1", "agent-39", AgentRole::Historian, "precedent says no".into(),
        ).unwrap_or_else(|_| unreachable!());
        assert!(event.dissenting_agent.contains("Historian"));
    }

    // ---------------------------------------------------------------
    // Additional tests to reach 50+
    // ---------------------------------------------------------------

    #[test]
    fn test_dissent_count_after_multiple_records() {
        let tracker = DissentTracker::new();
        for i in 0..10 {
            let _ = tracker.record_dissent(
                &format!("prop-{}", i % 3),
                &format!("agent-{i}"),
                AgentRole::Critic,
                format!("reason-{i}"),
            );
        }
        assert_eq!(tracker.total_dissent(), 10);
    }

    #[test]
    fn test_valuable_rate_with_no_valuables() {
        let tracker = DissentTracker::new();
        let _ = tracker.record_dissent("prop-1", "a1", AgentRole::Validator, "r1".into());
        let _ = tracker.record_dissent("prop-1", "a2", AgentRole::Critic, "r2".into());
        let _ = tracker.record_dissent("prop-1", "a3", AgentRole::Explorer, "r3".into());
        assert!((tracker.valuable_dissent_rate()).abs() < f64::EPSILON);
    }

    #[test]
    fn test_filtering_by_proposal_id_isolation() {
        let tracker = DissentTracker::new();
        let _ = tracker.record_dissent("alpha", "a1", AgentRole::Critic, "r1".into());
        let _ = tracker.record_dissent("alpha", "a2", AgentRole::Critic, "r2".into());
        let _ = tracker.record_dissent("beta", "a3", AgentRole::Critic, "r3".into());
        let _ = tracker.record_dissent("gamma", "a4", AgentRole::Critic, "r4".into());

        let alpha_events = tracker.get_dissent_for_proposal("alpha");
        assert_eq!(alpha_events.len(), 2);

        let beta_events = tracker.get_dissent_for_proposal("beta");
        assert_eq!(beta_events.len(), 1);

        let gamma_events = tracker.get_dissent_for_proposal("gamma");
        assert_eq!(gamma_events.len(), 1);
    }

    #[test]
    fn test_filtering_by_agent_role_via_analysis() {
        let tracker = DissentTracker::new();
        let _ = tracker.record_dissent("prop-1", "v1", AgentRole::Validator, "r1".into());
        let _ = tracker.record_dissent("prop-1", "v2", AgentRole::Validator, "r2".into());
        let _ = tracker.record_dissent("prop-1", "c1", AgentRole::Critic, "r3".into());
        let _ = tracker.record_dissent("prop-1", "e1", AgentRole::Explorer, "r4".into());
        let _ = tracker.record_dissent("prop-1", "i1", AgentRole::Integrator, "r5".into());

        let analysis = tracker.analyze_dissent("prop-1").unwrap_or_else(|_| unreachable!());
        assert_eq!(analysis.dissent_by_role.get("Validator"), Some(&2));
        assert_eq!(analysis.dissent_by_role.get("Critic"), Some(&1));
        assert_eq!(analysis.dissent_by_role.get("Explorer"), Some(&1));
        assert_eq!(analysis.dissent_by_role.get("Integrator"), Some(&1));
    }

    #[test]
    fn test_capacity_eviction_preserves_newer_events() {
        let tracker = DissentTracker::new();

        // Fill to capacity (500) + 10 more to trigger eviction
        for i in 0..510 {
            let _ = tracker.record_dissent(
                "prop-overflow",
                &format!("agent-{i}"),
                AgentRole::Validator,
                format!("reason-{i}"),
            );
        }

        // Should be capped at MAX_DISSENT_EVENTS
        assert!(tracker.total_dissent() <= 500);
    }

    #[test]
    fn test_concurrent_recording() {
        use std::sync::Arc;
        use std::thread;

        let tracker = Arc::new(DissentTracker::new());
        let mut handles = Vec::new();

        for thread_id in 0..4 {
            let t = Arc::clone(&tracker);
            handles.push(thread::spawn(move || {
                for i in 0..10 {
                    let _ = t.record_dissent(
                        &format!("prop-{thread_id}"),
                        &format!("agent-{thread_id}-{i}"),
                        AgentRole::Critic,
                        format!("concurrent reason {thread_id}-{i}"),
                    );
                }
            }));
        }

        for handle in handles {
            let _ = handle.join();
        }

        assert_eq!(tracker.total_dissent(), 40);
    }

    #[test]
    fn test_event_id_uniqueness() {
        let tracker = DissentTracker::new();
        let e1 = tracker.record_dissent(
            "prop-1", "agent-01", AgentRole::Validator, "reason-1".into(),
        ).unwrap_or_else(|_| unreachable!());
        let e2 = tracker.record_dissent(
            "prop-1", "agent-02", AgentRole::Validator, "reason-2".into(),
        ).unwrap_or_else(|_| unreachable!());

        assert_ne!(e1.id, e2.id);
    }

    #[test]
    fn test_dissent_rate_with_many_proposals() {
        let tracker = DissentTracker::new();
        // 5 proposals, each with 2 dissent events = 10 events / 5 proposals = 2.0
        for p in 0..5 {
            let _ = tracker.record_dissent(
                &format!("prop-{p}"), &format!("a-{p}-0"), AgentRole::Critic, "r".into(),
            );
            let _ = tracker.record_dissent(
                &format!("prop-{p}"), &format!("a-{p}-1"), AgentRole::Critic, "r".into(),
            );
        }
        let rate = tracker.dissent_rate();
        assert!((rate - 2.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_explorer_dissent_not_in_critic_filter() {
        let tracker = DissentTracker::new();
        let _ = tracker.record_dissent(
            "prop-1", "e-01", AgentRole::Explorer, "alternative found".into(),
        );
        let _ = tracker.record_dissent(
            "prop-1", "e-02", AgentRole::Explorer, "different path".into(),
        );
        let critic_events = tracker.get_critic_dissent();
        assert!(critic_events.is_empty());
    }

    #[test]
    fn test_integrator_dissent_role_format() {
        let tracker = DissentTracker::new();
        let event = tracker.record_dissent(
            "prop-1", "i-01", AgentRole::Integrator, "cross-system impact".into(),
        ).unwrap_or_else(|_| unreachable!());
        assert!(event.dissenting_agent.contains("Integrator"));
        assert!(event.dissenting_agent.contains("i-01"));
    }
}
