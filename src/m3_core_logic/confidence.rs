//! # M15: Confidence Calculator
//!
//! Calculates and calibrates confidence scores for remediation actions.
//!
//! The confidence calculator integrates multiple signals to produce a calibrated
//! confidence score for proposed remediation actions:
//!
//! - **Historical success rate**: How often similar actions succeeded for a service.
//! - **Pattern match strength**: How closely the current issue matches known patterns.
//! - **Severity score**: Normalized severity of the detected issue.
//! - **Pathway weight**: Hebbian pathway weight from the learning layer.
//! - **Time factor**: Recency-weighted decay so recent outcomes dominate.
//!
//! The raw confidence produced by [`super::calculate_confidence`] is then
//! *calibrated* against historical accuracy to correct for systematic over-
//! or under-confidence.
//!
//! ## Layer: L3 (Core Logic)
//! ## Dependencies: M1 (Error), M3 Core Logic types
//!
//! ## 12D Tensor Encoding
//! ```text
//! [15/36, 0.0, 3/6, 2, 0, 0.5, health, uptime, synergy, latency, error_rate, temporal]
//! ```
//!
//! ## Calibration Model
//!
//! If the calculator has been historically over-confident (predicted high
//! confidence but actions failed), the calibration offset becomes negative,
//! reducing future scores. Conversely, if under-confident, the offset
//! becomes positive. The offset is bounded to `[-0.2, 0.2]` to prevent
//! runaway correction.
//!
//! ## Related Documentation
//! - [Module Specification](../../ai_docs/modules/M15_CONFIDENCE.md)
//! - [Auto-Remediation](../../nam/L0_AUTO_REMEDIATION.md)

use std::collections::{HashMap, VecDeque};

use chrono::{DateTime, Utc};
use parking_lot::RwLock;

use super::{calculate_confidence, IssueType, Severity};
use crate::{Error, Result};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Maximum number of action records retained per service.
const ACTION_HISTORY_CAPACITY: usize = 100;

/// Default confidence returned when no historical data is available.
const DEFAULT_NO_HISTORY_CONFIDENCE: f64 = 0.5;

/// Maximum absolute value of the calibration offset.
const MAX_CALIBRATION_OFFSET: f64 = 0.2;

/// Number of hours after which the time factor contribution is halved.
const TIME_FACTOR_HALF_LIFE_HOURS: f64 = 24.0;

// ---------------------------------------------------------------------------
// Data structures
// ---------------------------------------------------------------------------

/// Record of a single remediation action and its outcome.
///
/// Stored in a bounded deque within [`ServiceHistory`] to provide the
/// data needed for calibration and historical success rate calculations.
#[derive(Clone, Debug)]
pub struct ActionRecord {
    /// The type of action performed (e.g. "retry", "restart").
    pub action_type: String,
    /// The issue type that triggered this action.
    pub issue_type: String,
    /// Whether the action was successful.
    pub success: bool,
    /// The confidence score at the time the decision was made.
    pub confidence_at_decision: f64,
    /// When this action was recorded.
    pub timestamp: DateTime<Utc>,
}

/// Historical data for a single service, tracking action outcomes
/// and per-issue-type success rates.
#[derive(Clone, Debug)]
pub struct ServiceHistory {
    /// Unique service identifier.
    pub service_id: String,
    /// Total number of actions taken for this service.
    pub total_actions: u64,
    /// Number of actions that succeeded.
    pub successful_actions: u64,
    /// Bounded ring buffer of recent action records.
    pub action_history: VecDeque<ActionRecord>,
    /// Per-issue-type success tracking: `(successful, total)`.
    pub issue_type_success: HashMap<String, (u64, u64)>,
}

impl ServiceHistory {
    /// Create a new, empty service history.
    fn new(service_id: String) -> Self {
        Self {
            service_id,
            total_actions: 0,
            successful_actions: 0,
            action_history: VecDeque::with_capacity(ACTION_HISTORY_CAPACITY),
            issue_type_success: HashMap::new(),
        }
    }
}

/// All factors contributing to a confidence calculation, including
/// both raw and calibrated scores.
///
/// Returned by [`ConfidenceCalculator::calculate`] so callers can
/// inspect the individual components.
#[derive(Clone, Debug)]
pub struct ConfidenceFactors {
    /// Historical success rate for the service (0.0 - 1.0).
    pub historical_success_rate: f64,
    /// Pattern match strength from the pattern cache (0.0 - 1.0).
    pub pattern_match_strength: f64,
    /// Normalized severity score (0.0 - 1.0).
    pub severity_score: f64,
    /// Hebbian pathway weight (0.0 - 1.0).
    pub pathway_weight: f64,
    /// Time-decay factor based on action recency (0.0 - 1.0).
    pub time_factor: f64,
    /// Raw confidence before calibration.
    pub raw_confidence: f64,
    /// Calibrated confidence after adjusting for historical accuracy.
    pub calibrated_confidence: f64,
}

// ---------------------------------------------------------------------------
// ConfidenceCalculator
// ---------------------------------------------------------------------------

/// Thread-safe confidence calculator that integrates historical data,
/// pattern matching, and Hebbian pathway weights to produce calibrated
/// confidence scores for remediation actions.
///
/// # Thread Safety
///
/// All mutable state is guarded by `parking_lot::RwLock` instances,
/// allowing concurrent readers with exclusive writers. The calculator
/// can be shared across threads via `Arc<ConfidenceCalculator>`.
///
/// # Examples
///
/// ```
/// use maintenance_engine::m3_core_logic::confidence::ConfidenceCalculator;
/// use maintenance_engine::m3_core_logic::{IssueType, Severity};
///
/// let calc = ConfidenceCalculator::new();
///
/// // With no history, we get a moderate confidence
/// let factors = calc.calculate("svc-1", IssueType::HealthFailure, Severity::Medium);
/// assert!(factors.is_ok());
/// ```
pub struct ConfidenceCalculator {
    /// Per-service historical action data.
    historical_data: RwLock<HashMap<String, ServiceHistory>>,
    /// Cached pattern match strengths, keyed by pattern identifier.
    pattern_cache: RwLock<HashMap<String, f64>>,
    /// Hebbian pathway weights, keyed by pathway identifier.
    pathway_weights: RwLock<HashMap<String, f64>>,
}

impl ConfidenceCalculator {
    /// Create a new, empty confidence calculator with no historical data.
    ///
    /// # Examples
    ///
    /// ```
    /// use maintenance_engine::m3_core_logic::confidence::ConfidenceCalculator;
    ///
    /// let calc = ConfidenceCalculator::new();
    /// assert_eq!(calc.service_count(), 0);
    /// assert_eq!(calc.total_records(), 0);
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self {
            historical_data: RwLock::new(HashMap::new()),
            pattern_cache: RwLock::new(HashMap::new()),
            pathway_weights: RwLock::new(HashMap::new()),
        }
    }

    /// Calculate a calibrated confidence score for a proposed remediation
    /// action on the specified service.
    ///
    /// Gathers historical success rate, pattern match strength, pathway
    /// weight, and time factor, then calls the core
    /// [`super::calculate_confidence`] function and applies calibration.
    ///
    /// # Arguments
    ///
    /// * `service_id` - Target service identifier.
    /// * `issue_type` - Classification of the detected issue.
    /// * `severity` - Severity level of the issue.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] if the `service_id` is empty.
    pub fn calculate(
        &self,
        service_id: &str,
        issue_type: IssueType,
        severity: Severity,
    ) -> Result<ConfidenceFactors> {
        if service_id.is_empty() {
            return Err(Error::Validation(
                "service_id must not be empty".into(),
            ));
        }

        let historical_success_rate = self.get_historical_success_rate(service_id);
        let pattern_match_strength = self.get_pattern_strength(service_id, issue_type);
        let severity_score = severity.score();
        let pathway_weight = self.get_pathway_weight_for_service(service_id);
        let time_factor = self.calculate_time_factor(service_id);

        let raw_confidence = calculate_confidence(
            historical_success_rate,
            pattern_match_strength,
            severity_score,
            pathway_weight,
            time_factor,
        );

        let calibrated_confidence = self.calibrate(raw_confidence, service_id);

        Ok(ConfidenceFactors {
            historical_success_rate,
            pattern_match_strength,
            severity_score,
            pathway_weight,
            time_factor,
            raw_confidence,
            calibrated_confidence,
        })
    }

    /// Record the outcome of a remediation action for future calibration.
    ///
    /// Updates the service's historical data with the new outcome, maintains
    /// the bounded action history, and refreshes per-issue-type counters.
    ///
    /// # Arguments
    ///
    /// * `service_id` - The service the action was taken on.
    /// * `issue_type` - The issue type that triggered the action.
    /// * `action_type` - A label for the action taken (e.g. "restart").
    /// * `success` - Whether the action succeeded.
    /// * `confidence_at_decision` - The confidence score at the time of decision.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] if `service_id` or `action_type` is empty.
    #[allow(clippy::significant_drop_tightening)]
    pub fn record_outcome(
        &self,
        service_id: &str,
        issue_type: &str,
        action_type: &str,
        success: bool,
        confidence_at_decision: f64,
    ) -> Result<()> {
        if service_id.is_empty() {
            return Err(Error::Validation(
                "service_id must not be empty".into(),
            ));
        }
        if action_type.is_empty() {
            return Err(Error::Validation(
                "action_type must not be empty".into(),
            ));
        }

        let record = ActionRecord {
            action_type: action_type.to_owned(),
            issue_type: issue_type.to_owned(),
            success,
            confidence_at_decision,
            timestamp: Utc::now(),
        };

        {
            let mut guard = self.historical_data.write();
            let history = guard
                .entry(service_id.to_owned())
                .or_insert_with(|| ServiceHistory::new(service_id.to_owned()));

            history.total_actions = history.total_actions.saturating_add(1);
            if success {
                history.successful_actions = history.successful_actions.saturating_add(1);
            }

            // Maintain bounded history
            if history.action_history.len() >= ACTION_HISTORY_CAPACITY {
                history.action_history.pop_front();
            }
            history.action_history.push_back(record);

            // Update per-issue-type counters
            let entry = history
                .issue_type_success
                .entry(issue_type.to_owned())
                .or_insert((0, 0));
            if success {
                entry.0 = entry.0.saturating_add(1);
            }
            entry.1 = entry.1.saturating_add(1);
        }

        Ok(())
    }

    /// Get the overall historical success rate for a service.
    ///
    /// Returns `DEFAULT_NO_HISTORY_CONFIDENCE` (0.5) when no history exists.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn get_historical_success_rate(&self, service_id: &str) -> f64 {
        let guard = self.historical_data.read();
        let result = guard.get(service_id).map_or(
            DEFAULT_NO_HISTORY_CONFIDENCE,
            |h| {
                if h.total_actions == 0 {
                    DEFAULT_NO_HISTORY_CONFIDENCE
                } else {
                    h.successful_actions as f64 / h.total_actions as f64
                }
            },
        );
        drop(guard);
        result
    }

    /// Get the success rate for a specific issue type on a specific service.
    ///
    /// Returns `DEFAULT_NO_HISTORY_CONFIDENCE` (0.5) when no history exists
    /// for the given combination.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn get_issue_success_rate(&self, service_id: &str, issue_type: &str) -> f64 {
        let guard = self.historical_data.read();
        let result = guard.get(service_id).map_or(
            DEFAULT_NO_HISTORY_CONFIDENCE,
            |h| {
                h.issue_type_success.get(issue_type).map_or(
                    DEFAULT_NO_HISTORY_CONFIDENCE,
                    |&(success, total)| {
                        if total == 0 {
                            DEFAULT_NO_HISTORY_CONFIDENCE
                        } else {
                            success as f64 / total as f64
                        }
                    },
                )
            },
        );
        drop(guard);
        result
    }

    /// Update a pattern match strength in the cache.
    ///
    /// # Arguments
    ///
    /// * `pattern_key` - Unique identifier for the pattern.
    /// * `strength` - Match strength (clamped to [0.0, 1.0]).
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] if `pattern_key` is empty.
    pub fn update_pattern_strength(
        &self,
        pattern_key: &str,
        strength: f64,
    ) -> Result<()> {
        if pattern_key.is_empty() {
            return Err(Error::Validation(
                "pattern_key must not be empty".into(),
            ));
        }

        let clamped = strength.clamp(0.0, 1.0);
        {
            let mut guard = self.pattern_cache.write();
            guard.insert(pattern_key.to_owned(), clamped);
        }
        Ok(())
    }

    /// Update a Hebbian pathway weight.
    ///
    /// # Arguments
    ///
    /// * `pathway_key` - Unique identifier for the pathway.
    /// * `weight` - Pathway weight (clamped to [0.0, 1.0]).
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] if `pathway_key` is empty.
    pub fn update_pathway_weight(
        &self,
        pathway_key: &str,
        weight: f64,
    ) -> Result<()> {
        if pathway_key.is_empty() {
            return Err(Error::Validation(
                "pathway_key must not be empty".into(),
            ));
        }

        let clamped = weight.clamp(0.0, 1.0);
        {
            let mut guard = self.pathway_weights.write();
            guard.insert(pathway_key.to_owned(), clamped);
        }
        Ok(())
    }

    /// Apply calibration to a raw confidence score based on historical
    /// accuracy for the given service.
    ///
    /// If the calculator has been over-confident (high predicted confidence
    /// but low actual success), the offset is negative, reducing the score.
    /// If under-confident, the offset is positive.
    ///
    /// The result is always clamped to `[0.0, 1.0]`.
    #[must_use]
    pub fn calibrate(&self, raw_confidence: f64, service_id: &str) -> f64 {
        let offset = self.get_calibration_offset(service_id);
        (raw_confidence + offset).clamp(0.0, 1.0)
    }

    /// Calculate a time decay factor for a service based on the recency
    /// of its action history.
    ///
    /// Uses exponential decay: actions within the last hour contribute
    /// most, with a half-life of [`TIME_FACTOR_HALF_LIFE_HOURS`] (24h).
    ///
    /// Returns `DEFAULT_NO_HISTORY_CONFIDENCE` (0.5) when no history exists.
    #[must_use]
    #[allow(clippy::cast_precision_loss, clippy::significant_drop_tightening)]
    pub fn calculate_time_factor(&self, service_id: &str) -> f64 {
        let guard = self.historical_data.read();
        let Some(history) = guard.get(service_id) else {
            return DEFAULT_NO_HISTORY_CONFIDENCE;
        };

        if history.action_history.is_empty() {
            return DEFAULT_NO_HISTORY_CONFIDENCE;
        }

        let now = Utc::now();
        let mut weighted_sum = 0.0_f64;
        let mut weight_total = 0.0_f64;

        for record in &history.action_history {
            let hours_ago = now
                .signed_duration_since(record.timestamp)
                .num_seconds()
                .max(0) as f64
                / 3600.0;

            // Exponential decay: weight = 2^(-hours_ago / half_life)
            let decay = (-hours_ago / TIME_FACTOR_HALF_LIFE_HOURS).exp2();
            let value = if record.success { 1.0 } else { 0.0 };
            weighted_sum = decay.mul_add(value, weighted_sum);
            weight_total += decay;
        }

        if weight_total > 0.0 {
            (weighted_sum / weight_total).clamp(0.0, 1.0)
        } else {
            DEFAULT_NO_HISTORY_CONFIDENCE
        }
    }

    /// Compute the calibration offset for a service.
    ///
    /// Compares the average predicted confidence against the actual success
    /// rate. The difference, clamped to `[-MAX_CALIBRATION_OFFSET, MAX_CALIBRATION_OFFSET]`,
    /// is returned as the offset to add to raw confidence.
    ///
    /// Returns `0.0` when no history exists (no correction needed).
    #[must_use]
    #[allow(clippy::cast_precision_loss, clippy::significant_drop_tightening)]
    pub fn get_calibration_offset(&self, service_id: &str) -> f64 {
        let guard = self.historical_data.read();
        let Some(history) = guard.get(service_id) else {
            return 0.0;
        };

        if history.action_history.is_empty() {
            return 0.0;
        }

        let count = history.action_history.len() as f64;
        let avg_predicted: f64 = history
            .action_history
            .iter()
            .map(|r| r.confidence_at_decision)
            .sum::<f64>()
            / count;

        let actual_rate = if history.total_actions == 0 {
            DEFAULT_NO_HISTORY_CONFIDENCE
        } else {
            history.successful_actions as f64 / history.total_actions as f64
        };

        // If avg_predicted > actual_rate, we were over-confident -> negative offset
        // If avg_predicted < actual_rate, we were under-confident -> positive offset
        let offset = actual_rate - avg_predicted;
        offset.clamp(-MAX_CALIBRATION_OFFSET, MAX_CALIBRATION_OFFSET)
    }

    /// Return the number of services that have historical data.
    #[must_use]
    pub fn service_count(&self) -> usize {
        let guard = self.historical_data.read();
        let count = guard.len();
        drop(guard);
        count
    }

    /// Return the total number of action records across all services.
    #[must_use]
    pub fn total_records(&self) -> u64 {
        let guard = self.historical_data.read();
        let total = guard.values().map(|h| h.total_actions).sum();
        drop(guard);
        total
    }

    // ------------------------------------------------------------------
    // Private helpers
    // ------------------------------------------------------------------

    /// Look up the pattern match strength for a service + issue type
    /// combination. Falls back to a service-only key, then to a default.
    #[allow(clippy::significant_drop_tightening)]
    fn get_pattern_strength(&self, service_id: &str, issue_type: IssueType) -> f64 {
        let guard = self.pattern_cache.read();

        // Try specific key: "service_id:issue_type"
        let specific_key = format!("{service_id}:{}", issue_type.as_str());
        if let Some(&strength) = guard.get(&specific_key) {
            return strength;
        }

        // Try issue-type-only key
        if let Some(&strength) = guard.get(issue_type.as_str()) {
            return strength;
        }

        // Try service-only key
        if let Some(&strength) = guard.get(service_id) {
            return strength;
        }

        // No pattern data available
        DEFAULT_NO_HISTORY_CONFIDENCE
    }

    /// Look up the pathway weight for a service. Falls back to a default.
    fn get_pathway_weight_for_service(&self, service_id: &str) -> f64 {
        let guard = self.pathway_weights.read();
        let result = guard
            .get(service_id)
            .copied()
            .unwrap_or(DEFAULT_NO_HISTORY_CONFIDENCE);
        drop(guard);
        result
    }
}

impl Default for ConfidenceCalculator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: create a fresh calculator.
    fn make_calculator() -> ConfidenceCalculator {
        ConfidenceCalculator::new()
    }

    /// Helper: record several outcomes for a service.
    fn seed_history(
        calc: &ConfidenceCalculator,
        service_id: &str,
        successes: u64,
        failures: u64,
        confidence: f64,
    ) {
        for _ in 0..successes {
            let _ = calc.record_outcome(
                service_id,
                "health_failure",
                "restart",
                true,
                confidence,
            );
        }
        for _ in 0..failures {
            let _ = calc.record_outcome(
                service_id,
                "health_failure",
                "restart",
                false,
                confidence,
            );
        }
    }

    // ------------------------------------------------------------------
    // 1. test_new_calculator
    // ------------------------------------------------------------------
    #[test]
    fn test_new_calculator() {
        let calc = make_calculator();
        assert_eq!(calc.service_count(), 0);
        assert_eq!(calc.total_records(), 0);
    }

    // ------------------------------------------------------------------
    // 2. test_calculate_no_history
    // ------------------------------------------------------------------
    #[test]
    fn test_calculate_no_history() {
        let calc = make_calculator();
        let factors = calc.calculate("svc-new", IssueType::HealthFailure, Severity::Medium);
        assert!(factors.is_ok());

        let f = factors.ok().unwrap_or_else(|| ConfidenceFactors {
            historical_success_rate: 0.0,
            pattern_match_strength: 0.0,
            severity_score: 0.0,
            pathway_weight: 0.0,
            time_factor: 0.0,
            raw_confidence: 0.0,
            calibrated_confidence: 0.0,
        });

        // With all defaults at 0.5, severity Medium = 0.5
        // confidence = 0.3*0.5 + 0.25*0.5 + 0.2*0.5 + 0.15*0.5 + 0.1*0.5
        //            = 0.15 + 0.125 + 0.1 + 0.075 + 0.05 = 0.5
        assert!(
            (f.calibrated_confidence - 0.5).abs() < 0.01,
            "Expected ~0.5 for no-history, got {}",
            f.calibrated_confidence
        );
        assert!(
            (f.historical_success_rate - 0.5).abs() < f64::EPSILON,
            "Default historical rate should be 0.5"
        );
    }

    // ------------------------------------------------------------------
    // 3. test_calculate_with_history
    // ------------------------------------------------------------------
    #[test]
    fn test_calculate_with_history() {
        let calc = make_calculator();

        // Seed with 9 successes, 1 failure at confidence 0.9
        seed_history(&calc, "svc-good", 9, 1, 0.9);

        let factors = calc.calculate("svc-good", IssueType::HealthFailure, Severity::Low);
        assert!(factors.is_ok());

        let f = factors.ok().unwrap_or_else(|| ConfidenceFactors {
            historical_success_rate: 0.0,
            pattern_match_strength: 0.0,
            severity_score: 0.0,
            pathway_weight: 0.0,
            time_factor: 0.0,
            raw_confidence: 0.0,
            calibrated_confidence: 0.0,
        });

        // Historical success rate should be 0.9
        assert!(
            (f.historical_success_rate - 0.9).abs() < f64::EPSILON,
            "Expected 0.9, got {}",
            f.historical_success_rate
        );

        // Confidence should be higher than no-history case
        assert!(
            f.calibrated_confidence > 0.4,
            "Expected high confidence, got {}",
            f.calibrated_confidence
        );
    }

    // ------------------------------------------------------------------
    // 4. test_record_outcome
    // ------------------------------------------------------------------
    #[test]
    fn test_record_outcome() {
        let calc = make_calculator();

        let result = calc.record_outcome(
            "svc-1",
            "health_failure",
            "restart",
            true,
            0.85,
        );
        assert!(result.is_ok());
        assert_eq!(calc.service_count(), 1);
        assert_eq!(calc.total_records(), 1);

        // Record another for same service
        let result = calc.record_outcome(
            "svc-1",
            "latency_spike",
            "cache_cleanup",
            false,
            0.7,
        );
        assert!(result.is_ok());
        assert_eq!(calc.service_count(), 1);
        assert_eq!(calc.total_records(), 2);

        // Record for a different service
        let result = calc.record_outcome(
            "svc-2",
            "crash",
            "restart",
            true,
            0.95,
        );
        assert!(result.is_ok());
        assert_eq!(calc.service_count(), 2);
        assert_eq!(calc.total_records(), 3);

        // Validation: empty service_id
        let result = calc.record_outcome("", "crash", "restart", true, 0.9);
        assert!(result.is_err());

        // Validation: empty action_type
        let result = calc.record_outcome("svc-1", "crash", "", true, 0.9);
        assert!(result.is_err());
    }

    // ------------------------------------------------------------------
    // 5. test_historical_success_rate
    // ------------------------------------------------------------------
    #[test]
    fn test_historical_success_rate() {
        let calc = make_calculator();

        // No history -> default 0.5
        let rate = calc.get_historical_success_rate("unknown");
        assert!((rate - 0.5).abs() < f64::EPSILON);

        // 8 successes, 2 failures -> 0.8
        seed_history(&calc, "svc-tested", 8, 2, 0.8);
        let rate = calc.get_historical_success_rate("svc-tested");
        assert!(
            (rate - 0.8).abs() < f64::EPSILON,
            "Expected 0.8, got {rate}"
        );

        // All failures -> 0.0
        seed_history(&calc, "svc-bad", 0, 5, 0.5);
        let rate = calc.get_historical_success_rate("svc-bad");
        assert!(
            (rate - 0.0).abs() < f64::EPSILON,
            "Expected 0.0, got {rate}"
        );
    }

    // ------------------------------------------------------------------
    // 6. test_issue_success_rate
    // ------------------------------------------------------------------
    #[test]
    fn test_issue_success_rate() {
        let calc = make_calculator();

        // No history -> 0.5
        let rate = calc.get_issue_success_rate("svc-1", "health_failure");
        assert!((rate - 0.5).abs() < f64::EPSILON);

        // Record some specific issue type outcomes
        let _ = calc.record_outcome("svc-1", "health_failure", "restart", true, 0.8);
        let _ = calc.record_outcome("svc-1", "health_failure", "restart", true, 0.8);
        let _ = calc.record_outcome("svc-1", "health_failure", "restart", false, 0.8);
        let _ = calc.record_outcome("svc-1", "latency_spike", "cache", false, 0.6);

        // health_failure: 2 success, 3 total -> 2/3
        let rate = calc.get_issue_success_rate("svc-1", "health_failure");
        let expected = 2.0 / 3.0;
        assert!(
            (rate - expected).abs() < f64::EPSILON,
            "Expected {expected}, got {rate}"
        );

        // latency_spike: 0 success, 1 total -> 0.0
        let rate = calc.get_issue_success_rate("svc-1", "latency_spike");
        assert!(
            (rate - 0.0).abs() < f64::EPSILON,
            "Expected 0.0, got {rate}"
        );

        // Unknown issue type for existing service -> 0.5
        let rate = calc.get_issue_success_rate("svc-1", "crash");
        assert!((rate - 0.5).abs() < f64::EPSILON);
    }

    // ------------------------------------------------------------------
    // 7. test_calibration_overconfident
    // ------------------------------------------------------------------
    #[test]
    fn test_calibration_overconfident() {
        let calc = make_calculator();

        // Record outcomes where we were very confident (0.9) but only 50% succeeded
        for _ in 0..5 {
            let _ = calc.record_outcome("svc-over", "crash", "restart", true, 0.9);
        }
        for _ in 0..5 {
            let _ = calc.record_outcome("svc-over", "crash", "restart", false, 0.9);
        }

        // avg_predicted = 0.9, actual = 0.5 -> offset = 0.5 - 0.9 = -0.4
        // clamped to -0.2
        let offset = calc.get_calibration_offset("svc-over");
        assert!(
            offset < 0.0,
            "Over-confident service should have negative offset, got {offset}"
        );
        assert!(
            (offset - (-MAX_CALIBRATION_OFFSET)).abs() < f64::EPSILON,
            "Offset should be clamped to -{MAX_CALIBRATION_OFFSET}, got {offset}"
        );

        // Calibration should reduce the raw confidence
        let calibrated = calc.calibrate(0.8, "svc-over");
        assert!(
            calibrated < 0.8,
            "Calibrated confidence should be lower than raw for over-confident service"
        );
    }

    // ------------------------------------------------------------------
    // 8. test_calibration_underconfident
    // ------------------------------------------------------------------
    #[test]
    fn test_calibration_underconfident() {
        let calc = make_calculator();

        // Record outcomes where we were not confident (0.3) but everything succeeded
        for _ in 0..10 {
            let _ = calc.record_outcome("svc-under", "crash", "restart", true, 0.3);
        }

        // avg_predicted = 0.3, actual = 1.0 -> offset = 1.0 - 0.3 = 0.7
        // clamped to +0.2
        let offset = calc.get_calibration_offset("svc-under");
        assert!(
            offset > 0.0,
            "Under-confident service should have positive offset, got {offset}"
        );
        assert!(
            (offset - MAX_CALIBRATION_OFFSET).abs() < f64::EPSILON,
            "Offset should be clamped to +{MAX_CALIBRATION_OFFSET}, got {offset}"
        );

        // Calibration should increase the raw confidence
        let calibrated = calc.calibrate(0.5, "svc-under");
        assert!(
            calibrated > 0.5,
            "Calibrated confidence should be higher than raw for under-confident service"
        );
    }

    // ------------------------------------------------------------------
    // 9. test_time_factor_recent
    // ------------------------------------------------------------------
    #[test]
    fn test_time_factor_recent() {
        let calc = make_calculator();

        // No history -> 0.5
        let tf = calc.calculate_time_factor("no-history");
        assert!(
            (tf - 0.5).abs() < f64::EPSILON,
            "No-history time factor should be 0.5, got {tf}"
        );

        // Record successful actions just now (timestamp = now)
        for _ in 0..5 {
            let _ = calc.record_outcome("svc-recent", "crash", "restart", true, 0.8);
        }

        // All records are very recent -> time factor should be close to 1.0
        let tf = calc.calculate_time_factor("svc-recent");
        assert!(
            tf > 0.9,
            "Recent successful actions should give time factor near 1.0, got {tf}"
        );

        // Record some failures to bring it down
        for _ in 0..5 {
            let _ = calc.record_outcome("svc-mixed", "crash", "restart", true, 0.8);
        }
        for _ in 0..5 {
            let _ = calc.record_outcome("svc-mixed", "crash", "restart", false, 0.8);
        }

        let tf = calc.calculate_time_factor("svc-mixed");
        // All records are recent with 50/50 mix -> should be near 0.5
        assert!(
            (tf - 0.5).abs() < 0.1,
            "Mixed recent outcomes should give ~0.5 time factor, got {tf}"
        );
    }

    // ------------------------------------------------------------------
    // 10. test_pattern_strength_update
    // ------------------------------------------------------------------
    #[test]
    fn test_pattern_strength_update() {
        let calc = make_calculator();

        // Update a pattern
        let result = calc.update_pattern_strength("svc-1:health_failure", 0.85);
        assert!(result.is_ok());

        // It should be used in calculation
        let factors = calc.calculate("svc-1", IssueType::HealthFailure, Severity::Medium);
        assert!(factors.is_ok());
        let f = factors.ok().unwrap_or_else(|| ConfidenceFactors {
            historical_success_rate: 0.0,
            pattern_match_strength: 0.0,
            severity_score: 0.0,
            pathway_weight: 0.0,
            time_factor: 0.0,
            raw_confidence: 0.0,
            calibrated_confidence: 0.0,
        });
        assert!(
            (f.pattern_match_strength - 0.85).abs() < f64::EPSILON,
            "Pattern strength should be 0.85, got {}",
            f.pattern_match_strength
        );

        // Values are clamped
        let result = calc.update_pattern_strength("key", 1.5);
        assert!(result.is_ok());
        {
            let guard = calc.pattern_cache.read();
            let val = guard.get("key").copied().unwrap_or(0.0);
            assert!(
                (val - 1.0).abs() < f64::EPSILON,
                "Should be clamped to 1.0, got {val}"
            );
        }

        // Validation: empty key
        let result = calc.update_pattern_strength("", 0.5);
        assert!(result.is_err());
    }

    // ------------------------------------------------------------------
    // 11. test_pathway_weight_update
    // ------------------------------------------------------------------
    #[test]
    fn test_pathway_weight_update() {
        let calc = make_calculator();

        // Update a pathway weight
        let result = calc.update_pathway_weight("svc-1", 0.75);
        assert!(result.is_ok());

        // It should be used in calculation
        let factors = calc.calculate("svc-1", IssueType::Crash, Severity::High);
        assert!(factors.is_ok());
        let f = factors.ok().unwrap_or_else(|| ConfidenceFactors {
            historical_success_rate: 0.0,
            pattern_match_strength: 0.0,
            severity_score: 0.0,
            pathway_weight: 0.0,
            time_factor: 0.0,
            raw_confidence: 0.0,
            calibrated_confidence: 0.0,
        });
        assert!(
            (f.pathway_weight - 0.75).abs() < f64::EPSILON,
            "Pathway weight should be 0.75, got {}",
            f.pathway_weight
        );

        // Values are clamped to [0.0, 1.0]
        let result = calc.update_pathway_weight("key", -0.5);
        assert!(result.is_ok());
        {
            let guard = calc.pathway_weights.read();
            let val = guard.get("key").copied().unwrap_or(1.0);
            assert!(
                val.abs() < f64::EPSILON,
                "Should be clamped to 0.0, got {val}"
            );
        }

        // Validation: empty key
        let result = calc.update_pathway_weight("", 0.5);
        assert!(result.is_err());
    }

    // ------------------------------------------------------------------
    // 12. test_action_history_cap
    // ------------------------------------------------------------------
    #[test]
    fn test_action_history_cap() {
        let calc = make_calculator();

        // Fill beyond capacity
        for i in 0..ACTION_HISTORY_CAPACITY + 50 {
            let _ = calc.record_outcome(
                "svc-cap",
                "health_failure",
                &format!("action-{i}"),
                true,
                0.8,
            );
        }

        let guard = calc.historical_data.read();
        let history = guard.get("svc-cap");
        assert!(history.is_some());

        let fallback = ServiceHistory {
            service_id: String::new(),
            total_actions: 0,
            successful_actions: 0,
            action_history: VecDeque::new(),
            issue_type_success: HashMap::new(),
        };
        let h = history.unwrap_or(&fallback);

        // Ring buffer should not exceed capacity
        assert!(
            h.action_history.len() <= ACTION_HISTORY_CAPACITY,
            "History length {} exceeds cap {}",
            h.action_history.len(),
            ACTION_HISTORY_CAPACITY
        );
        assert_eq!(
            h.action_history.len(),
            ACTION_HISTORY_CAPACITY,
            "History should be exactly at capacity"
        );

        // But total_actions should reflect all recorded outcomes
        #[allow(clippy::cast_possible_truncation)]
        let expected_total = (ACTION_HISTORY_CAPACITY + 50) as u64;
        assert_eq!(
            h.total_actions, expected_total,
            "total_actions should count all outcomes, not just buffered ones"
        );
    }

    // ------------------------------------------------------------------
    // 13. test_calculate_empty_service_id_fails
    // ------------------------------------------------------------------
    #[test]
    fn test_calculate_empty_service_id_fails() {
        let calc = make_calculator();
        let result = calc.calculate("", IssueType::Crash, Severity::Critical);
        assert!(result.is_err());
    }

    // ------------------------------------------------------------------
    // 14. test_severity_scores
    // ------------------------------------------------------------------
    #[test]
    fn test_severity_scores() {
        assert!((Severity::Low.score() - 0.25).abs() < f64::EPSILON);
        assert!((Severity::Medium.score() - 0.5).abs() < f64::EPSILON);
        assert!((Severity::High.score() - 0.75).abs() < f64::EPSILON);
        assert!((Severity::Critical.score() - 1.0).abs() < f64::EPSILON);
    }

    // ------------------------------------------------------------------
    // 15. test_default_impl
    // ------------------------------------------------------------------
    #[test]
    fn test_default_impl() {
        let calc = ConfidenceCalculator::default();
        assert_eq!(calc.service_count(), 0);
        assert_eq!(calc.total_records(), 0);
    }

    // ------------------------------------------------------------------
    // 16. test_calibration_no_history_zero_offset
    // ------------------------------------------------------------------
    #[test]
    fn test_calibration_no_history_zero_offset() {
        let calc = make_calculator();
        let offset = calc.get_calibration_offset("nonexistent");
        assert!(
            offset.abs() < f64::EPSILON,
            "No-history offset should be 0.0, got {offset}"
        );

        // Calibration should not change the raw value
        let calibrated = calc.calibrate(0.73, "nonexistent");
        assert!(
            (calibrated - 0.73).abs() < f64::EPSILON,
            "No-history calibration should be identity"
        );
    }

    // ------------------------------------------------------------------
    // 17. test_confidence_clamped_to_bounds
    // ------------------------------------------------------------------
    #[test]
    fn test_confidence_clamped_to_bounds() {
        let calc = make_calculator();

        // Create a massively under-confident service
        for _ in 0..20 {
            let _ = calc.record_outcome("svc-clamp", "crash", "restart", true, 0.1);
        }

        // Calibrate a very high raw confidence
        let calibrated = calc.calibrate(0.99, "svc-clamp");
        assert!(
            calibrated <= 1.0,
            "Calibrated confidence must not exceed 1.0, got {calibrated}"
        );
        assert!(
            calibrated >= 0.0,
            "Calibrated confidence must not be negative, got {calibrated}"
        );

        // Create a massively over-confident service
        for _ in 0..20 {
            let _ = calc.record_outcome("svc-clamp-low", "crash", "restart", false, 0.95);
        }

        // Calibrate a very low raw confidence
        let calibrated = calc.calibrate(0.05, "svc-clamp-low");
        assert!(
            calibrated >= 0.0,
            "Calibrated confidence must not be negative, got {calibrated}"
        );
    }

    // ------------------------------------------------------------------
    // 19. test_calculate_all_issue_types
    // ------------------------------------------------------------------
    #[test]
    fn test_calculate_all_issue_types() {
        let calc = make_calculator();
        let issue_types = [
            IssueType::HealthFailure,
            IssueType::LatencySpike,
            IssueType::ErrorRateHigh,
            IssueType::MemoryPressure,
            IssueType::DiskPressure,
            IssueType::ConnectionFailure,
            IssueType::Timeout,
            IssueType::Crash,
        ];

        for issue in &issue_types {
            let result = calc.calculate("svc-all", *issue, Severity::Medium);
            assert!(result.is_ok());
            if let Ok(f) = result {
                assert!(f.calibrated_confidence >= 0.0);
                assert!(f.calibrated_confidence <= 1.0);
            }
        }
    }

    // ------------------------------------------------------------------
    // 20. test_calculate_all_severities
    // ------------------------------------------------------------------
    #[test]
    fn test_calculate_all_severities() {
        let calc = make_calculator();
        let severities = [
            Severity::Low,
            Severity::Medium,
            Severity::High,
            Severity::Critical,
        ];

        for sev in &severities {
            let result = calc.calculate("svc-sev", IssueType::Crash, *sev);
            assert!(result.is_ok());
            if let Ok(f) = result {
                assert!((f.severity_score - sev.score()).abs() < f64::EPSILON);
            }
        }
    }

    // ------------------------------------------------------------------
    // 21. test_higher_severity_changes_confidence
    // ------------------------------------------------------------------
    #[test]
    fn test_higher_severity_changes_confidence() {
        let calc = make_calculator();

        let low = calc.calculate("svc", IssueType::Crash, Severity::Low);
        let critical = calc.calculate("svc", IssueType::Crash, Severity::Critical);

        assert!(low.is_ok());
        assert!(critical.is_ok());

        if let (Ok(l), Ok(c)) = (low, critical) {
            // Higher severity should yield different confidence
            assert!((l.severity_score - c.severity_score).abs() > 0.1);
        }
    }

    // ------------------------------------------------------------------
    // 22. test_record_outcome_updates_counters
    // ------------------------------------------------------------------
    #[test]
    fn test_record_outcome_updates_counters() {
        let calc = make_calculator();

        let _ = calc.record_outcome("svc-count", "crash", "restart", true, 0.8);
        let _ = calc.record_outcome("svc-count", "crash", "restart", false, 0.8);
        let _ = calc.record_outcome("svc-count", "crash", "restart", true, 0.8);

        assert_eq!(calc.total_records(), 3);
        let rate = calc.get_historical_success_rate("svc-count");
        assert!((rate - 2.0 / 3.0).abs() < f64::EPSILON);
    }

    // ------------------------------------------------------------------
    // 23. test_all_successes_success_rate
    // ------------------------------------------------------------------
    #[test]
    fn test_all_successes_success_rate() {
        let calc = make_calculator();
        seed_history(&calc, "svc-perfect", 10, 0, 0.8);
        let rate = calc.get_historical_success_rate("svc-perfect");
        assert!((rate - 1.0).abs() < f64::EPSILON);
    }

    // ------------------------------------------------------------------
    // 24. test_pattern_strength_clamped_negative
    // ------------------------------------------------------------------
    #[test]
    fn test_pattern_strength_clamped_negative() {
        let calc = make_calculator();
        let result = calc.update_pattern_strength("key-neg", -0.5);
        assert!(result.is_ok());
        let guard = calc.pattern_cache.read();
        let val = guard.get("key-neg").copied().unwrap_or(1.0);
        assert!(val.abs() < f64::EPSILON);
    }

    // ------------------------------------------------------------------
    // 25. test_pathway_weight_clamped_high
    // ------------------------------------------------------------------
    #[test]
    fn test_pathway_weight_clamped_high() {
        let calc = make_calculator();
        let result = calc.update_pathway_weight("key-high", 2.0);
        assert!(result.is_ok());
        let guard = calc.pathway_weights.read();
        let val = guard.get("key-high").copied().unwrap_or(0.0);
        assert!((val - 1.0).abs() < f64::EPSILON);
    }

    // ------------------------------------------------------------------
    // 26. test_multiple_services_independent
    // ------------------------------------------------------------------
    #[test]
    fn test_multiple_services_independent() {
        let calc = make_calculator();
        seed_history(&calc, "svc-a", 10, 0, 0.9);
        seed_history(&calc, "svc-b", 0, 10, 0.9);

        let rate_a = calc.get_historical_success_rate("svc-a");
        let rate_b = calc.get_historical_success_rate("svc-b");

        assert!((rate_a - 1.0).abs() < f64::EPSILON);
        assert!((rate_b - 0.0).abs() < f64::EPSILON);
    }

    // ------------------------------------------------------------------
    // 27. test_calibration_perfectly_calibrated
    // ------------------------------------------------------------------
    #[test]
    fn test_calibration_perfectly_calibrated() {
        let calc = make_calculator();
        // 50% confidence, 50% success -> offset should be near 0
        for _ in 0..5 {
            let _ = calc.record_outcome("svc-cal", "crash", "restart", true, 0.5);
        }
        for _ in 0..5 {
            let _ = calc.record_outcome("svc-cal", "crash", "restart", false, 0.5);
        }
        let offset = calc.get_calibration_offset("svc-cal");
        assert!(offset.abs() < f64::EPSILON);
    }

    // ------------------------------------------------------------------
    // 28. test_calibrate_clamps_result
    // ------------------------------------------------------------------
    #[test]
    fn test_calibrate_clamps_result() {
        let calc = make_calculator();
        // With no history, calibrate should be identity
        let result = calc.calibrate(0.5, "nonexistent");
        assert!((result - 0.5).abs() < f64::EPSILON);

        // Calibrate extreme values
        let result = calc.calibrate(1.0, "nonexistent");
        assert!(result <= 1.0);
        assert!(result >= 0.0);

        let result = calc.calibrate(0.0, "nonexistent");
        assert!(result <= 1.0);
        assert!(result >= 0.0);
    }

    // ------------------------------------------------------------------
    // 29. test_time_factor_no_actions_returns_default
    // ------------------------------------------------------------------
    #[test]
    fn test_time_factor_no_actions_returns_default() {
        let calc = make_calculator();
        let tf = calc.calculate_time_factor("nonexistent");
        assert!((tf - DEFAULT_NO_HISTORY_CONFIDENCE).abs() < f64::EPSILON);
    }

    // ------------------------------------------------------------------
    // 30. test_calculate_factors_all_populated
    // ------------------------------------------------------------------
    #[test]
    fn test_calculate_factors_all_populated() {
        let calc = make_calculator();
        let _ = calc.update_pattern_strength("svc-full:health_failure", 0.8);
        let _ = calc.update_pathway_weight("svc-full", 0.6);
        seed_history(&calc, "svc-full", 8, 2, 0.7);

        let result = calc.calculate("svc-full", IssueType::HealthFailure, Severity::High);
        assert!(result.is_ok());
        if let Ok(f) = result {
            assert!((f.pattern_match_strength - 0.8).abs() < f64::EPSILON);
            assert!((f.pathway_weight - 0.6).abs() < f64::EPSILON);
            assert!((f.historical_success_rate - 0.8).abs() < f64::EPSILON);
            assert!((f.severity_score - 0.75).abs() < f64::EPSILON);
            assert!(f.raw_confidence > 0.0);
            assert!(f.raw_confidence <= 1.0);
            assert!(f.calibrated_confidence >= 0.0);
            assert!(f.calibrated_confidence <= 1.0);
        }
    }

    // ------------------------------------------------------------------
    // 31. test_issue_success_rate_multiple_types
    // ------------------------------------------------------------------
    #[test]
    fn test_issue_success_rate_multiple_types() {
        let calc = make_calculator();
        let _ = calc.record_outcome("svc-multi", "health_failure", "restart", true, 0.8);
        let _ = calc.record_outcome("svc-multi", "health_failure", "restart", true, 0.8);
        let _ = calc.record_outcome("svc-multi", "crash", "restart", false, 0.7);
        let _ = calc.record_outcome("svc-multi", "crash", "restart", false, 0.7);
        let _ = calc.record_outcome("svc-multi", "crash", "restart", true, 0.7);

        let hf_rate = calc.get_issue_success_rate("svc-multi", "health_failure");
        assert!((hf_rate - 1.0).abs() < f64::EPSILON); // 2/2

        let crash_rate = calc.get_issue_success_rate("svc-multi", "crash");
        assert!((crash_rate - 1.0 / 3.0).abs() < f64::EPSILON); // 1/3
    }

    // ------------------------------------------------------------------
    // 32. test_service_count_tracks_unique_services
    // ------------------------------------------------------------------
    #[test]
    fn test_service_count_tracks_unique_services() {
        let calc = make_calculator();
        assert_eq!(calc.service_count(), 0);

        let _ = calc.record_outcome("svc-1", "crash", "restart", true, 0.8);
        assert_eq!(calc.service_count(), 1);

        let _ = calc.record_outcome("svc-1", "crash", "restart", true, 0.8);
        assert_eq!(calc.service_count(), 1); // Same service, no increase

        let _ = calc.record_outcome("svc-2", "crash", "restart", true, 0.8);
        assert_eq!(calc.service_count(), 2);

        let _ = calc.record_outcome("svc-3", "crash", "restart", true, 0.8);
        assert_eq!(calc.service_count(), 3);
    }

    // ------------------------------------------------------------------
    // 33. test_raw_confidence_formula
    // ------------------------------------------------------------------
    #[test]
    fn test_raw_confidence_formula() {
        let calc = make_calculator();
        // All inputs at 1.0:
        // 0.3*1.0 + 0.25*1.0 + 0.2*1.0 + 0.15*1.0 + 0.1*1.0 = 1.0
        let _ = calc.update_pattern_strength("svc-max:health_failure", 1.0);
        let _ = calc.update_pathway_weight("svc-max", 1.0);
        seed_history(&calc, "svc-max", 100, 0, 1.0);

        let result = calc.calculate("svc-max", IssueType::HealthFailure, Severity::Critical);
        assert!(result.is_ok());
        if let Ok(f) = result {
            // time_factor for all-success recent should be near 1.0
            assert!(f.raw_confidence > 0.9);
        }
    }

    // ------------------------------------------------------------------
    // 34. test_confidence_zero_inputs
    // ------------------------------------------------------------------
    #[test]
    fn test_confidence_zero_inputs() {
        // Direct call to calculate_confidence with all zeros
        let result = calculate_confidence(0.0, 0.0, 0.0, 0.0, 0.0);
        assert!((result - 0.0).abs() < f64::EPSILON);
    }

    // ------------------------------------------------------------------
    // 35. test_confidence_half_inputs
    // ------------------------------------------------------------------
    #[test]
    fn test_confidence_half_inputs() {
        let result = calculate_confidence(0.5, 0.5, 0.5, 0.5, 0.5);
        assert!((result - 0.5).abs() < f64::EPSILON);
    }

    // ------------------------------------------------------------------
    // 36. test_confidence_all_ones
    // ------------------------------------------------------------------
    #[test]
    fn test_confidence_all_ones() {
        let result = calculate_confidence(1.0, 1.0, 1.0, 1.0, 1.0);
        assert!((result - 1.0).abs() < f64::EPSILON);
    }

    // ------------------------------------------------------------------
    // 37. test_pattern_strength_overwrite
    // ------------------------------------------------------------------
    #[test]
    fn test_pattern_strength_overwrite() {
        let calc = make_calculator();
        let _ = calc.update_pattern_strength("key", 0.3);
        let _ = calc.update_pattern_strength("key", 0.8);

        let guard = calc.pattern_cache.read();
        let val = guard.get("key").copied().unwrap_or(0.0);
        assert!((val - 0.8).abs() < f64::EPSILON);
    }

    // ------------------------------------------------------------------
    // 38. test_pathway_weight_overwrite
    // ------------------------------------------------------------------
    #[test]
    fn test_pathway_weight_overwrite() {
        let calc = make_calculator();
        let _ = calc.update_pathway_weight("pw-key", 0.2);
        let _ = calc.update_pathway_weight("pw-key", 0.9);

        let guard = calc.pathway_weights.read();
        let val = guard.get("pw-key").copied().unwrap_or(0.0);
        assert!((val - 0.9).abs() < f64::EPSILON);
    }

    // ------------------------------------------------------------------
    // 39. test_calibration_offset_bounded
    // ------------------------------------------------------------------
    #[test]
    fn test_calibration_offset_bounded() {
        let calc = make_calculator();
        // Extreme over-confidence: predicted 1.0, actual 0% success
        for _ in 0..20 {
            let _ = calc.record_outcome("svc-bound", "crash", "restart", false, 1.0);
        }
        let offset = calc.get_calibration_offset("svc-bound");
        assert!(offset >= -MAX_CALIBRATION_OFFSET);
        assert!(offset <= MAX_CALIBRATION_OFFSET);
    }

    // ------------------------------------------------------------------
    // 40. test_get_pattern_strength_service_only_fallback
    // ------------------------------------------------------------------
    #[test]
    fn test_get_pattern_strength_service_only_fallback() {
        let calc = make_calculator();
        let _ = calc.update_pattern_strength("svc-fb-only", 0.65);

        // When calculating for svc-fb-only with a crash issue (no specific key),
        // it should fall back to the service-only key
        let factors = calc.calculate("svc-fb-only", IssueType::Crash, Severity::Medium);
        assert!(factors.is_ok());
        if let Ok(f) = factors {
            assert!((f.pattern_match_strength - 0.65).abs() < f64::EPSILON);
        }
    }

    // ------------------------------------------------------------------
    // 41. test_get_pathway_weight_default
    // ------------------------------------------------------------------
    #[test]
    fn test_get_pathway_weight_default() {
        let calc = make_calculator();
        // No pathway weight set -> should use default
        let factors = calc.calculate("svc-no-pw", IssueType::Crash, Severity::Low);
        assert!(factors.is_ok());
        if let Ok(f) = factors {
            assert!((f.pathway_weight - DEFAULT_NO_HISTORY_CONFIDENCE).abs() < f64::EPSILON);
        }
    }

    // ------------------------------------------------------------------
    // 42. test_total_records_multiple_services
    // ------------------------------------------------------------------
    #[test]
    fn test_total_records_multiple_services() {
        let calc = make_calculator();
        for i in 0..5 {
            let _ = calc.record_outcome(&format!("svc-{i}"), "crash", "restart", true, 0.8);
        }
        assert_eq!(calc.total_records(), 5);
        assert_eq!(calc.service_count(), 5);
    }

    // ------------------------------------------------------------------
    // 43. test_time_factor_all_failures_near_zero
    // ------------------------------------------------------------------
    #[test]
    fn test_time_factor_all_failures_near_zero() {
        let calc = make_calculator();
        for _ in 0..10 {
            let _ = calc.record_outcome("svc-fail-tf", "crash", "restart", false, 0.8);
        }
        let tf = calc.calculate_time_factor("svc-fail-tf");
        // All recent failures -> time factor should be near 0.0
        assert!(tf < 0.1);
    }

    // ------------------------------------------------------------------
    // 44. test_record_outcome_issue_type_counters
    // ------------------------------------------------------------------
    #[test]
    fn test_record_outcome_issue_type_counters() {
        let calc = make_calculator();
        let _ = calc.record_outcome("svc-it", "health_failure", "restart", true, 0.8);
        let _ = calc.record_outcome("svc-it", "health_failure", "restart", false, 0.7);

        let rate = calc.get_issue_success_rate("svc-it", "health_failure");
        assert!((rate - 0.5).abs() < f64::EPSILON);
    }

    // ------------------------------------------------------------------
    // 45. test_calibrate_applies_offset
    // ------------------------------------------------------------------
    #[test]
    fn test_calibrate_applies_offset() {
        let calc = make_calculator();
        // Create under-confident scenario: predicted 0.3, all succeed
        for _ in 0..10 {
            let _ = calc.record_outcome("svc-apply", "crash", "restart", true, 0.3);
        }
        let offset = calc.get_calibration_offset("svc-apply");
        assert!(offset > 0.0); // under-confident -> positive

        let calibrated = calc.calibrate(0.5, "svc-apply");
        assert!(calibrated > 0.5); // should be boosted
    }

    // ------------------------------------------------------------------
    // 46. test_calculate_returns_severity_score
    // ------------------------------------------------------------------
    #[test]
    fn test_calculate_returns_severity_score() {
        let calc = make_calculator();
        let result = calc.calculate("svc", IssueType::Crash, Severity::Critical);
        assert!(result.is_ok());
        if let Ok(f) = result {
            assert!((f.severity_score - 1.0).abs() < f64::EPSILON);
        }
    }

    // ------------------------------------------------------------------
    // 47. test_pattern_cache_multiple_keys
    // ------------------------------------------------------------------
    #[test]
    fn test_pattern_cache_multiple_keys() {
        let calc = make_calculator();
        let _ = calc.update_pattern_strength("k1", 0.1);
        let _ = calc.update_pattern_strength("k2", 0.2);
        let _ = calc.update_pattern_strength("k3", 0.3);

        let guard = calc.pattern_cache.read();
        assert_eq!(guard.len(), 3);
    }

    // ------------------------------------------------------------------
    // 48. test_pathway_weights_multiple_keys
    // ------------------------------------------------------------------
    #[test]
    fn test_pathway_weights_multiple_keys() {
        let calc = make_calculator();
        let _ = calc.update_pathway_weight("pw1", 0.1);
        let _ = calc.update_pathway_weight("pw2", 0.2);

        let guard = calc.pathway_weights.read();
        assert_eq!(guard.len(), 2);
    }

    // ------------------------------------------------------------------
    // 49. test_history_maintained_per_service
    // ------------------------------------------------------------------
    #[test]
    fn test_history_maintained_per_service() {
        let calc = make_calculator();
        seed_history(&calc, "svc-a", 5, 5, 0.5);
        seed_history(&calc, "svc-b", 3, 7, 0.5);

        let rate_a = calc.get_historical_success_rate("svc-a");
        let rate_b = calc.get_historical_success_rate("svc-b");

        assert!((rate_a - 0.5).abs() < f64::EPSILON);
        assert!((rate_b - 0.3).abs() < f64::EPSILON);
    }

    // ------------------------------------------------------------------
    // 50. test_confidence_formula_weighted_correctly
    // ------------------------------------------------------------------
    #[test]
    fn test_confidence_formula_weighted_correctly() {
        // historical=1.0, rest=0.0 -> result = 0.3
        let r1 = calculate_confidence(1.0, 0.0, 0.0, 0.0, 0.0);
        assert!((r1 - 0.3).abs() < f64::EPSILON);

        // pattern=1.0, rest=0.0 -> result = 0.25
        let r2 = calculate_confidence(0.0, 1.0, 0.0, 0.0, 0.0);
        assert!((r2 - 0.25).abs() < f64::EPSILON);

        // severity=1.0, rest=0.0 -> result = 0.2
        let r3 = calculate_confidence(0.0, 0.0, 1.0, 0.0, 0.0);
        assert!((r3 - 0.2).abs() < f64::EPSILON);

        // pathway=1.0, rest=0.0 -> result = 0.15
        let r4 = calculate_confidence(0.0, 0.0, 0.0, 1.0, 0.0);
        assert!((r4 - 0.15).abs() < f64::EPSILON);

        // time=1.0, rest=0.0 -> result = 0.1
        let r5 = calculate_confidence(0.0, 0.0, 0.0, 0.0, 1.0);
        assert!((r5 - 0.1).abs() < f64::EPSILON);
    }

    // ------------------------------------------------------------------
    // 18. test_pattern_key_fallback_hierarchy
    // ------------------------------------------------------------------
    #[test]
    fn test_pattern_key_fallback_hierarchy() {
        let calc = make_calculator();

        // Set up fallback hierarchy
        let _ = calc.update_pattern_strength("health_failure", 0.6);
        let _ = calc.update_pattern_strength("svc-fb", 0.7);
        let _ = calc.update_pattern_strength("svc-fb:health_failure", 0.9);

        // Most specific key should win
        let factors = calc.calculate("svc-fb", IssueType::HealthFailure, Severity::Low);
        assert!(factors.is_ok());
        let f = factors.ok().unwrap_or_else(|| ConfidenceFactors {
            historical_success_rate: 0.0,
            pattern_match_strength: 0.0,
            severity_score: 0.0,
            pathway_weight: 0.0,
            time_factor: 0.0,
            raw_confidence: 0.0,
            calibrated_confidence: 0.0,
        });
        assert!(
            (f.pattern_match_strength - 0.9).abs() < f64::EPSILON,
            "Should use specific key, got {}",
            f.pattern_match_strength
        );

        // Issue-type-only fallback for a different service
        let factors = calc.calculate("svc-other", IssueType::HealthFailure, Severity::Low);
        assert!(factors.is_ok());
        let f = factors.ok().unwrap_or_else(|| ConfidenceFactors {
            historical_success_rate: 0.0,
            pattern_match_strength: 0.0,
            severity_score: 0.0,
            pathway_weight: 0.0,
            time_factor: 0.0,
            raw_confidence: 0.0,
            calibrated_confidence: 0.0,
        });
        assert!(
            (f.pattern_match_strength - 0.6).abs() < f64::EPSILON,
            "Should fall back to issue-type key, got {}",
            f.pattern_match_strength
        );
    }
}
