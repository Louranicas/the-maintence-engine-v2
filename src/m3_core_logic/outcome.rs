//! # M17: Outcome Recorder
//!
//! Records and aggregates remediation outcomes for the Maintenance Engine.
//! Tracks success rates, effectiveness, and duration metrics per service
//! to drive Hebbian pathway weight adjustments and adaptive learning.
//!
//! ## Layer: L3 (Core Logic)
//!
//! ## Responsibilities
//!
//! - Record individual remediation outcomes with full context
//! - Maintain per-service aggregate statistics
//! - Calculate pathway deltas for Hebbian weight updates
//! - Provide effectiveness and trend analysis
//!
//! ## Capacity Limits
//!
//! | Collection | Cap | Eviction |
//! |------------|-----|----------|
//! | outcomes | 1000 | Oldest first |
//! | `service_outcomes` | Unbounded keys | Indices pruned on eviction |
//! | `aggregate_stats` | Unbounded keys | Never evicted |
//!
//! ## STDP Integration
//!
//! The `calculate_pathway_delta` method returns weight changes compatible
//! with the Hebbian STDP layer:
//! - Positive delta (LTP) for successful remediations
//! - Negative delta (LTD) for failed remediations
//! - Magnitude scaled by actual effectiveness
//!
//! ## Related Documentation
//! - [Layer Specification](../../ai_docs/layers/L03_CORE_LOGIC.md)
//! - [STDP Specification](../../ai_specs/STDP_SPEC.md)

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use uuid::Uuid;

use crate::m3_core_logic::{IssueType, RemediationAction, Severity};
use crate::{Error, Result};

/// STDP Long-Term Potentiation rate used for pathway delta calculation.
const LTP_RATE: f64 = 0.1;

/// STDP Long-Term Depression rate used for pathway delta calculation.
const LTD_RATE: f64 = 0.05;

/// Maximum number of outcome records stored before oldest are evicted.
const OUTCOME_CAP: usize = 1000;

/// A single recorded remediation outcome with full context.
///
/// Captures every dimension of a remediation attempt including the action
/// taken, whether it succeeded, how long it took, and how effective it was
/// at resolving the underlying issue.
#[derive(Clone, Debug)]
pub struct OutcomeRecord {
    /// Unique identifier for this outcome record (UUID v4).
    pub id: String,
    /// The remediation request ID that triggered this outcome.
    pub request_id: String,
    /// The service this outcome pertains to.
    pub service_id: String,
    /// The type of issue that was being remediated.
    pub issue_type: IssueType,
    /// The severity of the issue at decision time.
    pub severity: Severity,
    /// The remediation action that was executed.
    pub action_taken: RemediationAction,
    /// Whether the remediation action completed without error.
    pub success: bool,
    /// Wall-clock duration of the remediation in milliseconds.
    pub duration_ms: u64,
    /// The confidence score at the time the decision was made (0.0-1.0).
    pub confidence_at_decision: f64,
    /// How effectively the action resolved the issue (0.0-1.0).
    ///
    /// - 1.0 = fully resolved
    /// - 0.5 = partially resolved
    /// - 0.0 = no improvement
    pub actual_effectiveness: f64,
    /// Any side effects observed after the remediation.
    pub side_effects: Vec<String>,
    /// Timestamp when the outcome was recorded.
    pub timestamp: DateTime<Utc>,
}

/// Aggregate statistics for a single service's remediation history.
///
/// Updated incrementally each time a new outcome is recorded for the service.
#[derive(Clone, Debug)]
pub struct AggregateOutcome {
    /// The service these aggregates pertain to.
    pub service_id: String,
    /// Total number of outcomes recorded for this service.
    pub total_outcomes: u64,
    /// Number of successful outcomes for this service.
    pub successful_outcomes: u64,
    /// Running average duration in milliseconds across all outcomes.
    pub avg_duration_ms: f64,
    /// Running average effectiveness across all outcomes.
    pub avg_effectiveness: f64,
    /// Running average confidence at decision across all outcomes.
    pub avg_confidence: f64,
    /// The action with the highest average effectiveness, if any.
    pub best_action: Option<RemediationAction>,
    /// The action with the lowest average effectiveness, if any.
    pub worst_action: Option<RemediationAction>,
    /// Timestamp of the most recent aggregate update.
    pub last_updated: DateTime<Utc>,
}

/// Records, stores, and analyzes remediation outcomes.
///
/// Thread-safe via `parking_lot::RwLock` on all interior collections.
/// Maintains a capped ring of individual outcomes (max 1000), per-service
/// index mappings, and incrementally updated aggregate statistics.
///
/// # Example
///
/// ```rust,no_run
/// use maintenance_engine::m3_core_logic::outcome::OutcomeRecorder;
/// use maintenance_engine::m3_core_logic::{IssueType, Severity, RemediationAction};
///
/// let recorder = OutcomeRecorder::new();
/// let action = RemediationAction::CacheCleanup {
///     service_id: "synthex".into(),
///     threshold_percent: 80,
/// };
/// let outcome = recorder.record(
///     "synthex", "req-001", IssueType::MemoryPressure,
///     Severity::Medium, action, true, 150, 0.85, 0.92,
/// );
/// ```
pub struct OutcomeRecorder {
    /// All outcome records, capped at `OUTCOME_CAP`.
    outcomes: RwLock<Vec<OutcomeRecord>>,
    /// Maps `service_id` -> indices into the `outcomes` vec.
    service_outcomes: RwLock<HashMap<String, Vec<usize>>>,
    /// Per-service aggregate statistics.
    aggregate_stats: RwLock<HashMap<String, AggregateOutcome>>,
}

impl OutcomeRecorder {
    /// Create a new, empty `OutcomeRecorder`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            outcomes: RwLock::new(Vec::new()),
            service_outcomes: RwLock::new(HashMap::new()),
            aggregate_stats: RwLock::new(HashMap::new()),
        }
    }

    /// Record a new remediation outcome.
    ///
    /// Creates an `OutcomeRecord`, appends it to the outcomes list (evicting
    /// the oldest record if at capacity), updates service indices, and
    /// recalculates aggregate statistics for the service.
    ///
    /// # Arguments
    ///
    /// * `service_id` - The service that was remediated
    /// * `request_id` - The remediation request identifier
    /// * `issue_type` - The classification of the issue
    /// * `severity` - The severity level at decision time
    /// * `action_taken` - The remediation action that was executed
    /// * `success` - Whether the action completed successfully
    /// * `duration_ms` - Wall-clock duration in milliseconds
    /// * `confidence` - Confidence score at decision time (0.0-1.0)
    /// * `effectiveness` - How well the action fixed the issue (0.0-1.0)
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if `confidence` or `effectiveness` is outside [0.0, 1.0].
    #[allow(clippy::too_many_arguments)]
    pub fn record(
        &self,
        service_id: &str,
        request_id: &str,
        issue_type: IssueType,
        severity: Severity,
        action_taken: RemediationAction,
        success: bool,
        duration_ms: u64,
        confidence: f64,
        effectiveness: f64,
    ) -> Result<OutcomeRecord> {
        if !(0.0..=1.0).contains(&confidence) {
            return Err(Error::Validation(format!(
                "confidence must be in [0.0, 1.0], got {confidence}"
            )));
        }
        if !(0.0..=1.0).contains(&effectiveness) {
            return Err(Error::Validation(format!(
                "effectiveness must be in [0.0, 1.0], got {effectiveness}"
            )));
        }

        let record = OutcomeRecord {
            id: Uuid::new_v4().to_string(),
            request_id: request_id.to_owned(),
            service_id: service_id.to_owned(),
            issue_type,
            severity,
            action_taken,
            success,
            duration_ms,
            confidence_at_decision: confidence,
            actual_effectiveness: effectiveness,
            side_effects: Vec::new(),
            timestamp: Utc::now(),
        };

        let result = record.clone();

        // Insert into outcomes vec, enforcing cap
        let idx = {
            let mut outcomes = self.outcomes.write();
            if outcomes.len() >= OUTCOME_CAP {
                // Evict oldest record and rebuild service indices
                outcomes.remove(0);
                // Rebuild service_outcomes indices after shift
                let mut svc_map = self.service_outcomes.write();
                Self::rebuild_service_indices(&outcomes, &mut svc_map);
            }
            let idx = outcomes.len();
            outcomes.push(record);
            idx
        };

        // Update service index
        {
            let mut svc_map = self.service_outcomes.write();
            svc_map
                .entry(service_id.to_owned())
                .or_default()
                .push(idx);
        }

        // Update aggregate statistics
        self.update_aggregates(service_id);

        Ok(result)
    }

    /// Retrieve a single outcome record by its ID.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if no outcome with the given ID is found.
    pub fn get_outcome(&self, id: &str) -> Result<OutcomeRecord> {
        let outcomes = self.outcomes.read();
        outcomes
            .iter()
            .find(|o| o.id == id)
            .cloned()
            .ok_or_else(|| Error::Validation(format!("Outcome not found: {id}")))
    }

    /// Retrieve all outcome records for a given service.
    ///
    /// Returns an empty vector if no outcomes exist for the service.
    #[must_use]
    pub fn get_service_outcomes(&self, service_id: &str) -> Vec<OutcomeRecord> {
        let outcomes = self.outcomes.read();
        let svc_map = self.service_outcomes.read();

        svc_map
            .get(service_id)
            .map(|indices| {
                indices
                    .iter()
                    .filter_map(|&idx| outcomes.get(idx).cloned())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Retrieve aggregate statistics for a given service.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if no aggregates exist for the service.
    pub fn get_aggregate(&self, service_id: &str) -> Result<AggregateOutcome> {
        let stats = self.aggregate_stats.read();
        stats
            .get(service_id)
            .cloned()
            .ok_or_else(|| Error::Validation(format!("No aggregates for service: {service_id}")))
    }

    /// Calculate the average effectiveness for a specific action on a service.
    ///
    /// Returns 0.0 if no matching outcomes are found.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn get_effectiveness(&self, service_id: &str, action: &RemediationAction) -> f64 {
        let service_outcomes = self.get_service_outcomes(service_id);
        let matching: Vec<&OutcomeRecord> = service_outcomes
            .iter()
            .filter(|o| &o.action_taken == action)
            .collect();

        if matching.is_empty() {
            return 0.0;
        }

        let sum: f64 = matching.iter().map(|o| o.actual_effectiveness).sum();
        sum / matching.len() as f64
    }

    /// Calculate the overall success rate for a specific remediation action
    /// across all services.
    ///
    /// Returns 0.0 if no matching outcomes are found.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn get_action_success_rate(&self, action: &RemediationAction) -> f64 {
        let outcomes = self.outcomes.read();
        let (total, successes) = outcomes
            .iter()
            .filter(|o| &o.action_taken == action)
            .fold((0usize, 0usize), |(t, s), o| {
                (t + 1, s + usize::from(o.success))
            });
        drop(outcomes);

        if total == 0 {
            return 0.0;
        }

        successes as f64 / total as f64
    }

    /// Calculate the Hebbian pathway weight delta for a remediation outcome.
    ///
    /// Returns a weight change value compatible with the STDP learning layer:
    /// - Positive (LTP): `+LTP_RATE * effectiveness` for successful actions
    /// - Negative (LTD): `-LTD_RATE * (1.0 - effectiveness)` for failed actions
    ///
    /// The delta magnitude is scaled by the actual effectiveness to provide
    /// proportional reinforcement or correction.
    ///
    /// # Arguments
    ///
    /// * `service_id` - The service to calculate the delta for
    /// * `action` - The remediation action to evaluate
    /// * `success` - Whether the most recent attempt succeeded
    #[must_use]
    pub fn calculate_pathway_delta(
        &self,
        service_id: &str,
        action: &RemediationAction,
        success: bool,
    ) -> f64 {
        let effectiveness = self.get_effectiveness(service_id, action);

        if success {
            // LTP: strengthen pathway proportional to effectiveness
            LTP_RATE * effectiveness
        } else {
            // LTD: weaken pathway proportional to ineffectiveness
            -LTD_RATE * (1.0 - effectiveness)
        }
    }

    /// Retrieve the effectiveness trend for a service over the last `n` outcomes.
    ///
    /// Returns a vector of effectiveness values ordered from oldest to newest.
    /// If fewer than `n` outcomes exist, returns all available.
    #[must_use]
    pub fn get_trend(&self, service_id: &str, last_n: usize) -> Vec<f64> {
        let service_outcomes = self.get_service_outcomes(service_id);

        let start = service_outcomes.len().saturating_sub(last_n);
        service_outcomes[start..]
            .iter()
            .map(|o| o.actual_effectiveness)
            .collect()
    }

    /// Return the total number of outcomes currently stored.
    #[must_use]
    pub fn total_outcomes(&self) -> usize {
        self.outcomes.read().len()
    }

    /// Calculate the overall success rate across all recorded outcomes.
    ///
    /// Returns 0.0 if no outcomes have been recorded.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn overall_success_rate(&self) -> f64 {
        let outcomes = self.outcomes.read();
        if outcomes.is_empty() {
            return 0.0;
        }

        let successes = outcomes.iter().filter(|o| o.success).count();
        successes as f64 / outcomes.len() as f64
    }

    /// Rebuild the `service_outcomes` index from scratch after an eviction.
    ///
    /// Called when the oldest outcome is removed and all indices shift down.
    fn rebuild_service_indices(
        outcomes: &[OutcomeRecord],
        svc_map: &mut HashMap<String, Vec<usize>>,
    ) {
        svc_map.clear();
        for (idx, record) in outcomes.iter().enumerate() {
            svc_map
                .entry(record.service_id.clone())
                .or_default()
                .push(idx);
        }
    }

    /// Incrementally update aggregate statistics for a service.
    ///
    /// Scans all current outcomes for the service and recomputes
    /// totals, averages, and best/worst actions.
    #[allow(clippy::cast_precision_loss)]
    fn update_aggregates(&self, service_id: &str) {
        let service_outcomes = self.get_service_outcomes(service_id);
        if service_outcomes.is_empty() {
            return;
        }

        let total = service_outcomes.len() as u64;
        let successful = service_outcomes.iter().filter(|o| o.success).count() as u64;

        let avg_duration = service_outcomes.iter().map(|o| o.duration_ms as f64).sum::<f64>()
            / total as f64;
        let avg_effectiveness =
            service_outcomes.iter().map(|o| o.actual_effectiveness).sum::<f64>() / total as f64;
        let avg_confidence = service_outcomes
            .iter()
            .map(|o| o.confidence_at_decision)
            .sum::<f64>()
            / total as f64;

        // Calculate per-action effectiveness to find best and worst
        let mut action_effectiveness: HashMap<String, (f64, u64, RemediationAction)> =
            HashMap::new();
        for outcome in &service_outcomes {
            let key = format!("{:?}", outcome.action_taken);
            let entry = action_effectiveness
                .entry(key)
                .or_insert_with(|| (0.0, 0, outcome.action_taken.clone()));
            entry.0 += outcome.actual_effectiveness;
            entry.1 += 1;
        }

        let best_action = action_effectiveness
            .values()
            .max_by(|a, b| {
                let avg_a = a.0 / a.1 as f64;
                let avg_b = b.0 / b.1 as f64;
                avg_a
                    .partial_cmp(&avg_b)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|entry| entry.2.clone());

        let worst_action = action_effectiveness
            .values()
            .min_by(|a, b| {
                let avg_a = a.0 / a.1 as f64;
                let avg_b = b.0 / b.1 as f64;
                avg_a
                    .partial_cmp(&avg_b)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|entry| entry.2.clone());

        let aggregate = AggregateOutcome {
            service_id: service_id.to_owned(),
            total_outcomes: total,
            successful_outcomes: successful,
            avg_duration_ms: avg_duration,
            avg_effectiveness,
            avg_confidence,
            best_action,
            worst_action,
            last_updated: Utc::now(),
        };

        let mut stats = self.aggregate_stats.write();
        stats.insert(service_id.to_owned(), aggregate);
    }
}

impl Default for OutcomeRecorder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to create a simple `RemediationAction` for testing.
    fn test_action() -> RemediationAction {
        RemediationAction::CacheCleanup {
            service_id: "test-svc".into(),
            threshold_percent: 80,
        }
    }

    /// Helper to create a different action for comparison tests.
    fn alt_action() -> RemediationAction {
        RemediationAction::RetryWithBackoff {
            max_retries: 3,
            initial_delay_ms: 100,
        }
    }

    #[test]
    fn test_record_outcome() {
        let recorder = OutcomeRecorder::new();
        let result = recorder.record(
            "synthex",
            "req-001",
            IssueType::MemoryPressure,
            Severity::Medium,
            test_action(),
            true,
            150,
            0.85,
            0.92,
        );

        assert!(result.is_ok());
        let outcome = result.ok().unwrap_or_else(|| {
            // This branch is unreachable due to the assert above,
            // but satisfies the no-unwrap rule
            OutcomeRecord {
                id: String::new(),
                request_id: String::new(),
                service_id: String::new(),
                issue_type: IssueType::HealthFailure,
                severity: Severity::Low,
                action_taken: test_action(),
                success: false,
                duration_ms: 0,
                confidence_at_decision: 0.0,
                actual_effectiveness: 0.0,
                side_effects: Vec::new(),
                timestamp: Utc::now(),
            }
        });
        assert_eq!(outcome.service_id, "synthex");
        assert_eq!(outcome.request_id, "req-001");
        assert!(outcome.success);
        assert!((outcome.actual_effectiveness - 0.92).abs() < f64::EPSILON);
        assert_eq!(recorder.total_outcomes(), 1);
    }

    #[test]
    fn test_get_service_outcomes() {
        let recorder = OutcomeRecorder::new();

        // Record outcomes for two different services
        let _ = recorder.record(
            "synthex", "req-001", IssueType::MemoryPressure,
            Severity::Medium, test_action(), true, 100, 0.8, 0.9,
        );
        let _ = recorder.record(
            "synthex", "req-002", IssueType::LatencySpike,
            Severity::Low, alt_action(), false, 200, 0.6, 0.3,
        );
        let _ = recorder.record(
            "nais", "req-003", IssueType::HealthFailure,
            Severity::High, test_action(), true, 50, 0.9, 0.95,
        );

        let synthex_outcomes = recorder.get_service_outcomes("synthex");
        assert_eq!(synthex_outcomes.len(), 2);

        let nais_outcomes = recorder.get_service_outcomes("nais");
        assert_eq!(nais_outcomes.len(), 1);

        let empty_outcomes = recorder.get_service_outcomes("nonexistent");
        assert!(empty_outcomes.is_empty());
    }

    #[test]
    fn test_aggregate_stats() {
        let recorder = OutcomeRecorder::new();

        let _ = recorder.record(
            "synthex", "req-001", IssueType::MemoryPressure,
            Severity::Medium, test_action(), true, 100, 0.8, 0.9,
        );
        let _ = recorder.record(
            "synthex", "req-002", IssueType::LatencySpike,
            Severity::Low, test_action(), true, 200, 0.7, 0.8,
        );
        let _ = recorder.record(
            "synthex", "req-003", IssueType::ErrorRateHigh,
            Severity::High, test_action(), false, 300, 0.5, 0.2,
        );

        let aggregate = recorder.get_aggregate("synthex");
        assert!(aggregate.is_ok());

        let agg = aggregate.ok().unwrap_or_else(|| AggregateOutcome {
            service_id: String::new(),
            total_outcomes: 0,
            successful_outcomes: 0,
            avg_duration_ms: 0.0,
            avg_effectiveness: 0.0,
            avg_confidence: 0.0,
            best_action: None,
            worst_action: None,
            last_updated: Utc::now(),
        });

        assert_eq!(agg.total_outcomes, 3);
        assert_eq!(agg.successful_outcomes, 2);
        assert!((agg.avg_duration_ms - 200.0).abs() < f64::EPSILON);

        // avg effectiveness = (0.9 + 0.8 + 0.2) / 3 = 1.9/3 ~ 0.6333
        let expected_eff = (0.9 + 0.8 + 0.2) / 3.0;
        assert!((agg.avg_effectiveness - expected_eff).abs() < 0.001);

        // No aggregates for unknown service
        let missing = recorder.get_aggregate("unknown");
        assert!(missing.is_err());
    }

    #[test]
    fn test_effectiveness_calculation() {
        let recorder = OutcomeRecorder::new();
        let action = test_action();

        let _ = recorder.record(
            "synthex", "req-001", IssueType::MemoryPressure,
            Severity::Medium, action.clone(), true, 100, 0.8, 0.9,
        );
        let _ = recorder.record(
            "synthex", "req-002", IssueType::MemoryPressure,
            Severity::Medium, action.clone(), true, 100, 0.8, 0.7,
        );

        let eff = recorder.get_effectiveness("synthex", &action);
        assert!((eff - 0.8).abs() < f64::EPSILON); // (0.9 + 0.7) / 2 = 0.8

        // No matching outcomes
        let eff_none = recorder.get_effectiveness("synthex", &alt_action());
        assert!((eff_none - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_action_success_rate() {
        let recorder = OutcomeRecorder::new();
        let action = test_action();

        let _ = recorder.record(
            "synthex", "req-001", IssueType::MemoryPressure,
            Severity::Medium, action.clone(), true, 100, 0.8, 0.9,
        );
        let _ = recorder.record(
            "nais", "req-002", IssueType::MemoryPressure,
            Severity::Medium, action.clone(), true, 100, 0.8, 0.8,
        );
        let _ = recorder.record(
            "sank7", "req-003", IssueType::MemoryPressure,
            Severity::Medium, action.clone(), false, 100, 0.8, 0.1,
        );

        let rate = recorder.get_action_success_rate(&action);
        // 2 successes out of 3 = 0.6666...
        assert!((rate - 2.0 / 3.0).abs() < 0.001);

        // No matching action
        let rate_none = recorder.get_action_success_rate(&alt_action());
        assert!((rate_none - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_pathway_delta_positive() {
        let recorder = OutcomeRecorder::new();
        let action = test_action();

        let _ = recorder.record(
            "synthex", "req-001", IssueType::MemoryPressure,
            Severity::Medium, action.clone(), true, 100, 0.8, 0.9,
        );

        let delta = recorder.calculate_pathway_delta("synthex", &action, true);
        // LTP: 0.1 * 0.9 = 0.09
        assert!(delta > 0.0);
        assert!((delta - LTP_RATE * 0.9).abs() < f64::EPSILON);
    }

    #[test]
    fn test_pathway_delta_negative() {
        let recorder = OutcomeRecorder::new();
        let action = test_action();

        let _ = recorder.record(
            "synthex", "req-001", IssueType::MemoryPressure,
            Severity::Medium, action.clone(), false, 100, 0.8, 0.3,
        );

        let delta = recorder.calculate_pathway_delta("synthex", &action, false);
        // LTD: -0.05 * (1.0 - 0.3) = -0.05 * 0.7 = -0.035
        assert!(delta < 0.0);
        assert!((delta - (-LTD_RATE * 0.7)).abs() < f64::EPSILON);
    }

    #[test]
    fn test_trend_analysis() {
        let recorder = OutcomeRecorder::new();
        let action = test_action();

        // Record 5 outcomes with varying effectiveness
        let effectiveness_values = [0.3, 0.5, 0.6, 0.8, 0.9];
        for (i, &eff) in effectiveness_values.iter().enumerate() {
            let _ = recorder.record(
                "synthex",
                &format!("req-{i}"),
                IssueType::MemoryPressure,
                Severity::Medium,
                action.clone(),
                true,
                100,
                0.8,
                eff,
            );
        }

        // Get last 3
        let trend = recorder.get_trend("synthex", 3);
        assert_eq!(trend.len(), 3);
        assert!((trend[0] - 0.6).abs() < f64::EPSILON);
        assert!((trend[1] - 0.8).abs() < f64::EPSILON);
        assert!((trend[2] - 0.9).abs() < f64::EPSILON);

        // Get all 5
        let full_trend = recorder.get_trend("synthex", 10);
        assert_eq!(full_trend.len(), 5);

        // Empty service
        let empty_trend = recorder.get_trend("unknown", 5);
        assert!(empty_trend.is_empty());
    }

    #[test]
    fn test_outcome_cap() {
        let recorder = OutcomeRecorder::new();
        let action = test_action();

        // Fill beyond capacity
        for i in 0..1050 {
            let _ = recorder.record(
                "synthex",
                &format!("req-{i}"),
                IssueType::MemoryPressure,
                Severity::Medium,
                action.clone(),
                true,
                100,
                0.8,
                0.9,
            );
        }

        // Should be capped at OUTCOME_CAP
        assert_eq!(recorder.total_outcomes(), OUTCOME_CAP);

        // Service outcomes should still be consistent
        let svc = recorder.get_service_outcomes("synthex");
        assert_eq!(svc.len(), OUTCOME_CAP);
    }

    #[test]
    fn test_overall_success_rate() {
        let recorder = OutcomeRecorder::new();

        // Empty recorder
        assert!((recorder.overall_success_rate() - 0.0).abs() < f64::EPSILON);

        let _ = recorder.record(
            "synthex", "req-001", IssueType::MemoryPressure,
            Severity::Medium, test_action(), true, 100, 0.8, 0.9,
        );
        let _ = recorder.record(
            "synthex", "req-002", IssueType::LatencySpike,
            Severity::Low, test_action(), false, 200, 0.6, 0.2,
        );
        let _ = recorder.record(
            "nais", "req-003", IssueType::HealthFailure,
            Severity::High, test_action(), true, 50, 0.9, 0.95,
        );
        let _ = recorder.record(
            "nais", "req-004", IssueType::Crash,
            Severity::Critical, alt_action(), false, 500, 0.4, 0.1,
        );

        // 2 successes out of 4 = 0.5
        let rate = recorder.overall_success_rate();
        assert!((rate - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_validation_errors() {
        let recorder = OutcomeRecorder::new();

        // Invalid confidence
        let result = recorder.record(
            "synthex", "req-001", IssueType::MemoryPressure,
            Severity::Medium, test_action(), true, 100, 1.5, 0.9,
        );
        assert!(result.is_err());

        // Invalid effectiveness
        let result = recorder.record(
            "synthex", "req-002", IssueType::MemoryPressure,
            Severity::Medium, test_action(), true, 100, 0.8, -0.1,
        );
        assert!(result.is_err());

        // Negative confidence
        let result = recorder.record(
            "synthex", "req-003", IssueType::MemoryPressure,
            Severity::Medium, test_action(), true, 100, -0.5, 0.9,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_get_outcome_by_id() {
        let recorder = OutcomeRecorder::new();
        let result = recorder.record(
            "synthex", "req-001", IssueType::MemoryPressure,
            Severity::Medium, test_action(), true, 150, 0.85, 0.92,
        );

        assert!(result.is_ok());
        if let Ok(recorded) = result {
            let fetched = recorder.get_outcome(&recorded.id);
            assert!(fetched.is_ok());
            if let Ok(found) = fetched {
                assert_eq!(found.request_id, "req-001");
                assert_eq!(found.service_id, "synthex");
            }
        }

        // Non-existent ID
        let missing = recorder.get_outcome("nonexistent-id");
        assert!(missing.is_err());
    }

    #[test]
    fn test_multiple_actions_best_worst() {
        let recorder = OutcomeRecorder::new();

        // Record good effectiveness for cache cleanup
        let _ = recorder.record(
            "synthex", "req-001", IssueType::MemoryPressure,
            Severity::Medium, test_action(), true, 100, 0.8, 0.95,
        );
        let _ = recorder.record(
            "synthex", "req-002", IssueType::MemoryPressure,
            Severity::Medium, test_action(), true, 100, 0.8, 0.90,
        );

        // Record poor effectiveness for retry
        let _ = recorder.record(
            "synthex", "req-003", IssueType::Timeout,
            Severity::Low, alt_action(), false, 500, 0.5, 0.1,
        );
        let _ = recorder.record(
            "synthex", "req-004", IssueType::Timeout,
            Severity::Low, alt_action(), false, 500, 0.5, 0.2,
        );

        let agg = recorder.get_aggregate("synthex");
        assert!(agg.is_ok());
        if let Ok(stats) = agg {
            assert!(stats.best_action.is_some());
            assert!(stats.worst_action.is_some());
            // Best action should be cache cleanup (avg 0.925)
            // Worst action should be retry (avg 0.15)
            assert_eq!(stats.best_action, Some(test_action()));
            assert_eq!(stats.worst_action, Some(alt_action()));
        }
    }
}
