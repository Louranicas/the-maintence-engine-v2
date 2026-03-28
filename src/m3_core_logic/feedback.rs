//! # M18: Feedback Loop
//!
//! Closed-loop feedback system for the Maintenance Engine's adaptive learning.
//! Generates feedback signals from remediation outcomes, calibrates confidence
//! predictions, and produces pathway learning recommendations.
//!
//! ## Layer: L3 (Core Logic)
//!
//! ## Responsibilities
//!
//! - Generate feedback signals (LTP/LTD) from remediation outcomes
//! - Calibrate confidence predictions against actual results
//! - Produce learning recommendations for Hebbian pathway adjustments
//! - Track signal history for trend analysis
//!
//! ## Signal Types
//!
//! | Type | Trigger | Effect |
//! |------|---------|--------|
//! | Reinforcement | High effectiveness success | Strengthen pathway (LTP) |
//! | Correction | Low effectiveness or failure | Weaken pathway (LTD) |
//! | Exploration | Novel or untried action | Small positive bias |
//! | Calibration | Confidence offset detected | Adjust thresholds |
//!
//! ## Capacity Limits
//!
//! | Collection | Cap | Eviction |
//! |------------|-----|----------|
//! | signals | 500 | Oldest first |
//! | `calibration_data.predicted_confidences` | 50 per service | FIFO |
//! | `calibration_data.actual_outcomes` | 50 per service | FIFO |
//! | `learning_recommendations` | Unbounded | Manual clear |
//!
//! ## STDP Integration
//!
//! Signal strength maps directly to STDP weight changes:
//! - Positive strength -> Long-Term Potentiation (LTP)
//! - Negative strength -> Long-Term Depression (LTD)
//! - Magnitude proportional to effectiveness
//!
//! ## Related Documentation
//! - [Layer Specification](../../ai_docs/layers/L03_CORE_LOGIC.md)
//! - [STDP Specification](../../ai_specs/STDP_SPEC.md)

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use uuid::Uuid;

use crate::{Error, Result};

/// Maximum number of feedback signals stored before oldest are evicted.
const SIGNAL_CAP: usize = 500;

/// Maximum number of calibration data points per service.
const CALIBRATION_CAP: usize = 50;

/// Effectiveness threshold above which a success is considered reinforcement.
const REINFORCEMENT_THRESHOLD: f64 = 0.7;

/// Minimum number of calibration data points required before computing offset.
const MIN_CALIBRATION_SAMPLES: usize = 3;

/// Classification of the feedback signal type.
///
/// Determines how the Hebbian learning layer should interpret the signal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SignalType {
    /// Reinforcement signal: high-effectiveness success that should strengthen
    /// the associated pathway (LTP).
    Reinforcement,
    /// Correction signal: failure or low effectiveness that should weaken
    /// the associated pathway (LTD).
    Correction,
    /// Exploration signal: the action was novel or untried, providing a small
    /// positive bias to encourage diversity.
    Exploration,
    /// Calibration signal: the confidence prediction was significantly off,
    /// indicating threshold adjustments are needed.
    Calibration,
}

/// A single feedback signal generated from a remediation outcome.
///
/// Feedback signals encode the direction and magnitude of weight changes
/// for Hebbian pathways. Positive strength values indicate potentiation (LTP)
/// while negative values indicate depression (LTD).
#[derive(Clone, Debug)]
pub struct FeedbackSignal {
    /// Unique identifier for this signal (UUID v4).
    pub id: String,
    /// The service this signal pertains to.
    pub service_id: String,
    /// The outcome record ID that generated this signal.
    pub outcome_id: String,
    /// The type of feedback signal.
    pub signal_type: SignalType,
    /// Signal strength in the range [-1.0, 1.0].
    ///
    /// - Positive values indicate Long-Term Potentiation (LTP)
    /// - Negative values indicate Long-Term Depression (LTD)
    /// - Magnitude reflects confidence in the feedback
    pub strength: f64,
    /// The pathway key in "source->target" format identifying which
    /// Hebbian pathway this signal applies to.
    pub pathway_key: String,
    /// Timestamp when the signal was generated.
    pub timestamp: DateTime<Utc>,
}

/// Calibration data for a single service's confidence predictions.
///
/// Tracks predicted confidence values alongside actual binary outcomes
/// to compute a calibration offset that can be applied to future predictions.
#[derive(Clone, Debug)]
pub struct CalibrationEntry {
    /// The service this calibration data pertains to.
    pub service_id: String,
    /// History of predicted confidence values (capped at [`CALIBRATION_CAP`]).
    pub predicted_confidences: Vec<f64>,
    /// History of actual binary outcomes parallel to `predicted_confidences`
    /// (capped at [`CALIBRATION_CAP`]).
    pub actual_outcomes: Vec<bool>,
    /// The computed calibration offset.
    ///
    /// - Negative offset: system is overconfident (predictions too high)
    /// - Positive offset: system is underconfident (predictions too low)
    /// - Zero: well-calibrated
    pub calibration_offset: f64,
    /// Timestamp of the most recent calibration update.
    pub last_updated: DateTime<Utc>,
}

/// A recommendation for adjusting Hebbian pathway weights or thresholds.
///
/// Generated by analyzing accumulated feedback signals to identify
/// pathways that should be strengthened, weakened, created, or pruned.
#[derive(Clone, Debug)]
pub struct LearningRecommendation {
    /// Unique identifier for this recommendation (UUID v4).
    pub id: String,
    /// The type of learning adjustment recommended.
    pub recommendation_type: RecommendationType,
    /// The pathway key in "source->target" format.
    pub pathway_key: String,
    /// The suggested weight delta to apply.
    pub suggested_delta: f64,
    /// Human-readable explanation of why this recommendation was made.
    pub reason: String,
    /// Timestamp when the recommendation was generated.
    pub timestamp: DateTime<Utc>,
}

/// Classification of learning recommendation types.
///
/// Each type maps to a specific Hebbian pathway operation in the L5
/// learning layer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecommendationType {
    /// Increase the weight of an existing pathway.
    StrengthenPathway,
    /// Decrease the weight of an existing pathway.
    WeakenPathway,
    /// Create a new pathway between nodes.
    CreatePathway,
    /// Remove an underperforming pathway entirely.
    PrunePathway,
    /// Adjust the activation threshold for a pathway.
    AdjustThreshold,
}

/// Closed-loop feedback system for adaptive learning.
///
/// The `FeedbackLoop` receives remediation outcomes, generates feedback signals
/// for the Hebbian learning layer, calibrates confidence predictions, and
/// produces actionable learning recommendations.
///
/// Thread-safe via `parking_lot::RwLock` on all interior collections.
///
/// # Example
///
/// ```rust,no_run
/// use maintenance_engine::m3_core_logic::feedback::FeedbackLoop;
///
/// let feedback = FeedbackLoop::new();
/// let signal = feedback.generate_signal(
///     "synthex", "outcome-001", true, 0.9, 0.85, "remediation->cache_cleanup",
/// );
/// ```
pub struct FeedbackLoop {
    /// Accumulated feedback signals, capped at [`SIGNAL_CAP`].
    signals: RwLock<Vec<FeedbackSignal>>,
    /// Per-service calibration data tracking prediction accuracy.
    calibration_data: RwLock<HashMap<String, CalibrationEntry>>,
    /// Generated learning recommendations awaiting consumption.
    learning_recommendations: RwLock<Vec<LearningRecommendation>>,
}

impl FeedbackLoop {
    /// Create a new, empty `FeedbackLoop`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            signals: RwLock::new(Vec::new()),
            calibration_data: RwLock::new(HashMap::new()),
            learning_recommendations: RwLock::new(Vec::new()),
        }
    }

    /// Generate a feedback signal from a remediation outcome.
    ///
    /// The signal strength and type are determined by the outcome:
    /// - Success with high effectiveness -> Reinforcement, strength = +effectiveness
    /// - Success with low effectiveness -> Exploration, strength = +effectiveness * 0.5
    /// - Failure -> Correction, strength = -effectiveness (or -0.1 minimum)
    ///
    /// # Arguments
    ///
    /// * `service_id` - The service the outcome pertains to
    /// * `outcome_id` - The outcome record ID
    /// * `action_success` - Whether the remediation action succeeded
    /// * `effectiveness` - How effective the action was (0.0-1.0)
    /// * `confidence_at_decision` - The confidence score when the decision was made
    /// * `pathway_key` - The Hebbian pathway key in "source->target" format
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if `effectiveness` or `confidence_at_decision`
    /// is outside [0.0, 1.0].
    pub fn generate_signal(
        &self,
        service_id: &str,
        outcome_id: &str,
        action_success: bool,
        effectiveness: f64,
        confidence_at_decision: f64,
        pathway_key: &str,
    ) -> Result<FeedbackSignal> {
        if !(0.0..=1.0).contains(&effectiveness) {
            return Err(Error::Validation(format!(
                "effectiveness must be in [0.0, 1.0], got {effectiveness}"
            )));
        }
        if !(0.0..=1.0).contains(&confidence_at_decision) {
            return Err(Error::Validation(format!(
                "confidence_at_decision must be in [0.0, 1.0], got {confidence_at_decision}"
            )));
        }

        let (signal_type, strength) = if action_success {
            if effectiveness >= REINFORCEMENT_THRESHOLD {
                // Strong success -> reinforce pathway
                (SignalType::Reinforcement, effectiveness)
            } else {
                // Weak success -> exploration signal with reduced strength
                (SignalType::Exploration, effectiveness * 0.5)
            }
        } else {
            // Failure -> correction signal with negative strength
            let neg_strength = if effectiveness > 0.0 {
                -effectiveness
            } else {
                // Even total failures get some negative signal
                -0.1
            };
            (SignalType::Correction, neg_strength)
        };

        // Clamp strength to [-1.0, 1.0]
        let strength = strength.clamp(-1.0, 1.0);

        // Check if calibration is significantly off -> override to Calibration type
        let calibration_offset = self.get_calibration_offset(service_id);
        let signal_type = if calibration_offset.abs() > 0.2 && action_success {
            SignalType::Calibration
        } else {
            signal_type
        };

        let signal = FeedbackSignal {
            id: Uuid::new_v4().to_string(),
            service_id: service_id.to_owned(),
            outcome_id: outcome_id.to_owned(),
            signal_type,
            strength,
            pathway_key: pathway_key.to_owned(),
            timestamp: Utc::now(),
        };

        let result = signal.clone();

        // Store signal, enforcing cap
        {
            let mut signals = self.signals.write();
            if signals.len() >= SIGNAL_CAP {
                signals.remove(0);
            }
            signals.push(signal);
        }

        Ok(result)
    }

    /// Record a calibration data point and return the updated calibration offset.
    ///
    /// Compares the predicted confidence with the actual outcome to track
    /// whether the system is over- or under-confident for this service.
    ///
    /// The offset is calculated as:
    /// `offset = actual_success_rate - avg_predicted_confidence`
    ///
    /// - Negative offset: system is overconfident
    /// - Positive offset: system is underconfident
    ///
    /// # Arguments
    ///
    /// * `service_id` - The service to record calibration for
    /// * `predicted_confidence` - The confidence value the system predicted
    /// * `actual_success` - Whether the action actually succeeded
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if `predicted_confidence` is outside [0.0, 1.0].
    #[allow(clippy::significant_drop_tightening, clippy::cast_precision_loss)]
    pub fn record_calibration(
        &self,
        service_id: &str,
        predicted_confidence: f64,
        actual_success: bool,
    ) -> Result<f64> {
        if !(0.0..=1.0).contains(&predicted_confidence) {
            return Err(Error::Validation(format!(
                "predicted_confidence must be in [0.0, 1.0], got {predicted_confidence}"
            )));
        }

        let mut data = self.calibration_data.write();
        let entry = data
            .entry(service_id.to_owned())
            .or_insert_with(|| CalibrationEntry {
                service_id: service_id.to_owned(),
                predicted_confidences: Vec::new(),
                actual_outcomes: Vec::new(),
                calibration_offset: 0.0,
                last_updated: Utc::now(),
            });

        // Add data point, enforcing cap
        if entry.predicted_confidences.len() >= CALIBRATION_CAP {
            entry.predicted_confidences.remove(0);
            entry.actual_outcomes.remove(0);
        }
        entry.predicted_confidences.push(predicted_confidence);
        entry.actual_outcomes.push(actual_success);
        entry.last_updated = Utc::now();

        // Calculate offset only with sufficient data
        if entry.predicted_confidences.len() >= MIN_CALIBRATION_SAMPLES {
            let avg_predicted: f64 =
                entry.predicted_confidences.iter().sum::<f64>()
                    / entry.predicted_confidences.len() as f64;

            let actual_success_rate = entry
                .actual_outcomes
                .iter()
                .filter(|&&s| s)
                .count() as f64
                / entry.actual_outcomes.len() as f64;

            entry.calibration_offset = actual_success_rate - avg_predicted;
        }

        Ok(entry.calibration_offset)
    }

    /// Retrieve the current calibration offset for a service.
    ///
    /// Returns 0.0 if no calibration data exists for the service.
    #[must_use]
    pub fn get_calibration_offset(&self, service_id: &str) -> f64 {
        let data = self.calibration_data.read();
        data.get(service_id)
            .map_or(0.0, |entry| entry.calibration_offset)
    }

    /// Generate learning recommendations based on accumulated signals for a service.
    ///
    /// Analyzes all signals for the service, groups them by pathway, and produces
    /// recommendations:
    /// - Pathways with consistently positive signals -> `StrengthenPathway`
    /// - Pathways with consistently negative signals -> `WeakenPathway`
    /// - Pathways with very negative signals and many corrections -> `PrunePathway`
    /// - Significant calibration offset -> `AdjustThreshold`
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if the `service_id` is empty.
    #[allow(clippy::cast_precision_loss)]
    pub fn generate_recommendations(
        &self,
        service_id: &str,
    ) -> Result<Vec<LearningRecommendation>> {
        if service_id.is_empty() {
            return Err(Error::Validation(
                "service_id must not be empty".to_owned(),
            ));
        }

        let service_signals = self.get_signals_for_service(service_id);
        if service_signals.is_empty() {
            return Ok(Vec::new());
        }

        // Group signals by pathway_key
        let mut pathway_signals: HashMap<String, Vec<&FeedbackSignal>> = HashMap::new();
        for signal in &service_signals {
            pathway_signals
                .entry(signal.pathway_key.clone())
                .or_default()
                .push(signal);
        }

        let mut recommendations = Vec::new();

        for (pathway_key, signals) in &pathway_signals {
            let total = signals.len() as f64;
            let avg_strength: f64 = signals.iter().map(|s| s.strength).sum::<f64>() / total;

            let correction_count = signals
                .iter()
                .filter(|s| s.signal_type == SignalType::Correction)
                .count();
            let reinforcement_count = signals
                .iter()
                .filter(|s| s.signal_type == SignalType::Reinforcement)
                .count();

            if avg_strength > 0.3 && reinforcement_count > correction_count {
                // Consistently positive -> strengthen
                recommendations.push(LearningRecommendation {
                    id: Uuid::new_v4().to_string(),
                    recommendation_type: RecommendationType::StrengthenPathway,
                    pathway_key: pathway_key.clone(),
                    suggested_delta: avg_strength * 0.1, // Scale by LTP rate
                    reason: format!(
                        "Pathway has avg strength {avg_strength:.3} with {reinforcement_count} reinforcements vs {correction_count} corrections"
                    ),
                    timestamp: Utc::now(),
                });
            } else if avg_strength < -0.3 && correction_count > reinforcement_count {
                if avg_strength < -0.7 && correction_count >= 3 {
                    // Very negative with many corrections -> prune
                    recommendations.push(LearningRecommendation {
                        id: Uuid::new_v4().to_string(),
                        recommendation_type: RecommendationType::PrunePathway,
                        pathway_key: pathway_key.clone(),
                        suggested_delta: avg_strength,
                        reason: format!(
                            "Pathway has avg strength {avg_strength:.3} with {correction_count} corrections; recommend pruning"
                        ),
                        timestamp: Utc::now(),
                    });
                } else {
                    // Moderately negative -> weaken
                    recommendations.push(LearningRecommendation {
                        id: Uuid::new_v4().to_string(),
                        recommendation_type: RecommendationType::WeakenPathway,
                        pathway_key: pathway_key.clone(),
                        suggested_delta: avg_strength * 0.05, // Scale by LTD rate
                        reason: format!(
                            "Pathway has avg strength {avg_strength:.3} with {correction_count} corrections vs {reinforcement_count} reinforcements"
                        ),
                        timestamp: Utc::now(),
                    });
                }
            }
        }

        // Check calibration offset for threshold adjustment
        let cal_offset = self.get_calibration_offset(service_id);
        if cal_offset.abs() > 0.15 {
            recommendations.push(LearningRecommendation {
                id: Uuid::new_v4().to_string(),
                recommendation_type: RecommendationType::AdjustThreshold,
                pathway_key: format!("calibration->{service_id}"),
                suggested_delta: cal_offset,
                reason: format!(
                    "Calibration offset of {cal_offset:.3} detected for service {service_id}; \
                     {}",
                    if cal_offset > 0.0 {
                        "system is underconfident"
                    } else {
                        "system is overconfident"
                    }
                ),
                timestamp: Utc::now(),
            });
        }

        // Store recommendations
        {
            let mut recs = self.learning_recommendations.write();
            for rec in &recommendations {
                recs.push(rec.clone());
            }
        }

        Ok(recommendations)
    }

    /// Retrieve all feedback signals for a specific service.
    ///
    /// Returns an empty vector if no signals exist for the service.
    #[must_use]
    pub fn get_signals_for_service(&self, service_id: &str) -> Vec<FeedbackSignal> {
        let signals = self.signals.read();
        signals
            .iter()
            .filter(|s| s.service_id == service_id)
            .cloned()
            .collect()
    }

    /// Retrieve the most recent `n` feedback signals across all services.
    ///
    /// Returns signals ordered from oldest to newest. If fewer than `n`
    /// signals exist, returns all available.
    #[must_use]
    pub fn get_recent_signals(&self, n: usize) -> Vec<FeedbackSignal> {
        let signals = self.signals.read();
        let start = signals.len().saturating_sub(n);
        signals[start..].to_vec()
    }

    /// Retrieve all accumulated learning recommendations.
    #[must_use]
    pub fn get_recommendations(&self) -> Vec<LearningRecommendation> {
        let recs = self.learning_recommendations.read();
        recs.clone()
    }

    /// Return the total number of feedback signals currently stored.
    #[must_use]
    pub fn signal_count(&self) -> usize {
        self.signals.read().len()
    }

    /// Remove all signals older than the specified timestamp.
    ///
    /// Returns the number of signals removed.
    pub fn clear_old_signals(&self, older_than: DateTime<Utc>) -> usize {
        let mut signals = self.signals.write();
        let original_len = signals.len();
        signals.retain(|s| s.timestamp >= older_than);
        original_len - signals.len()
    }
}

impl Default for FeedbackLoop {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_signal_success() {
        let feedback = FeedbackLoop::new();
        let result = feedback.generate_signal(
            "synthex",
            "outcome-001",
            true,
            0.9,
            0.85,
            "remediation->cache_cleanup",
        );

        assert!(result.is_ok());
        if let Ok(signal) = result {
            assert_eq!(signal.service_id, "synthex");
            assert_eq!(signal.outcome_id, "outcome-001");
            assert_eq!(signal.signal_type, SignalType::Reinforcement);
            assert!(signal.strength > 0.0);
            assert!((signal.strength - 0.9).abs() < f64::EPSILON);
            assert_eq!(signal.pathway_key, "remediation->cache_cleanup");
        }

        assert_eq!(feedback.signal_count(), 1);
    }

    #[test]
    fn test_generate_signal_failure() {
        let feedback = FeedbackLoop::new();
        let result = feedback.generate_signal(
            "nais",
            "outcome-002",
            false,
            0.4,
            0.7,
            "remediation->service_restart",
        );

        assert!(result.is_ok());
        if let Ok(signal) = result {
            assert_eq!(signal.service_id, "nais");
            assert_eq!(signal.signal_type, SignalType::Correction);
            assert!(signal.strength < 0.0);
            assert!((signal.strength - (-0.4)).abs() < f64::EPSILON);
        }
    }

    #[test]
    fn test_signal_strength_range() {
        let feedback = FeedbackLoop::new();

        // Test maximum positive strength
        let result = feedback.generate_signal(
            "test", "o1", true, 1.0, 0.5, "a->b",
        );
        assert!(result.is_ok());
        if let Ok(signal) = result {
            assert!(signal.strength >= -1.0 && signal.strength <= 1.0);
        }

        // Test maximum negative strength
        let result = feedback.generate_signal(
            "test", "o2", false, 1.0, 0.5, "a->b",
        );
        assert!(result.is_ok());
        if let Ok(signal) = result {
            assert!(signal.strength >= -1.0 && signal.strength <= 1.0);
            assert!((signal.strength - (-1.0)).abs() < f64::EPSILON);
        }

        // Test zero effectiveness failure
        let result = feedback.generate_signal(
            "test", "o3", false, 0.0, 0.5, "a->b",
        );
        assert!(result.is_ok());
        if let Ok(signal) = result {
            assert!(signal.strength >= -1.0 && signal.strength <= 1.0);
            assert!((signal.strength - (-0.1)).abs() < f64::EPSILON);
        }

        // Test weak success -> exploration
        let result = feedback.generate_signal(
            "test", "o4", true, 0.3, 0.5, "a->b",
        );
        assert!(result.is_ok());
        if let Ok(signal) = result {
            assert_eq!(signal.signal_type, SignalType::Exploration);
            assert!((signal.strength - 0.15).abs() < f64::EPSILON);
        }

        // Invalid effectiveness
        let result = feedback.generate_signal(
            "test", "o5", true, 1.5, 0.5, "a->b",
        );
        assert!(result.is_err());

        // Invalid confidence
        let result = feedback.generate_signal(
            "test", "o6", true, 0.5, -0.1, "a->b",
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_record_calibration() {
        let feedback = FeedbackLoop::new();

        // Record a few well-calibrated predictions
        let offset = feedback.record_calibration("synthex", 0.8, true);
        assert!(offset.is_ok());
        // Not enough samples yet (< MIN_CALIBRATION_SAMPLES)
        if let Ok(off) = offset {
            assert!((off - 0.0).abs() < f64::EPSILON);
        }

        let _ = feedback.record_calibration("synthex", 0.7, true);
        let offset = feedback.record_calibration("synthex", 0.9, true);
        assert!(offset.is_ok());

        // Now we have 3 samples: predicted avg = (0.8+0.7+0.9)/3 = 0.8
        // actual success rate = 3/3 = 1.0
        // offset = 1.0 - 0.8 = 0.2 (underconfident)
        if let Ok(off) = offset {
            assert!((off - 0.2).abs() < 0.001);
        }
    }

    #[test]
    fn test_calibration_overconfident() {
        let feedback = FeedbackLoop::new();

        // Predict high confidence but mostly fail
        let _ = feedback.record_calibration("overconf", 0.9, false);
        let _ = feedback.record_calibration("overconf", 0.85, false);
        let _ = feedback.record_calibration("overconf", 0.9, true);
        let _ = feedback.record_calibration("overconf", 0.88, false);
        let offset = feedback.record_calibration("overconf", 0.9, false);

        assert!(offset.is_ok());
        if let Ok(off) = offset {
            // avg predicted ~ 0.886, actual success = 1/5 = 0.2
            // offset = 0.2 - 0.886 = -0.686 (overconfident)
            assert!(off < 0.0, "Overconfident system should have negative offset");
        }
    }

    #[test]
    fn test_calibration_underconfident() {
        let feedback = FeedbackLoop::new();

        // Predict low confidence but mostly succeed
        let _ = feedback.record_calibration("underconf", 0.3, true);
        let _ = feedback.record_calibration("underconf", 0.25, true);
        let _ = feedback.record_calibration("underconf", 0.35, true);
        let _ = feedback.record_calibration("underconf", 0.3, false);
        let offset = feedback.record_calibration("underconf", 0.28, true);

        assert!(offset.is_ok());
        if let Ok(off) = offset {
            // avg predicted ~ 0.296, actual success = 4/5 = 0.8
            // offset = 0.8 - 0.296 = 0.504 (underconfident)
            assert!(off > 0.0, "Underconfident system should have positive offset");
        }
    }

    #[test]
    fn test_generate_recommendations() {
        let feedback = FeedbackLoop::new();

        // Add strongly positive signals for one pathway
        for i in 0..5 {
            let _ = feedback.generate_signal(
                "synthex",
                &format!("o-pos-{i}"),
                true,
                0.9,
                0.8,
                "remediation->cache_cleanup",
            );
        }

        // Add strongly negative signals for another pathway
        for i in 0..5 {
            let _ = feedback.generate_signal(
                "synthex",
                &format!("o-neg-{i}"),
                false,
                0.8,
                0.6,
                "remediation->service_restart",
            );
        }

        let recs = feedback.generate_recommendations("synthex");
        assert!(recs.is_ok());

        if let Ok(recommendations) = recs {
            // Should have at least one strengthen and one weaken/prune
            assert!(!recommendations.is_empty());

            let has_strengthen = recommendations
                .iter()
                .any(|r| r.recommendation_type == RecommendationType::StrengthenPathway);
            assert!(has_strengthen, "Should recommend strengthening positive pathway");

            let has_weaken_or_prune = recommendations.iter().any(|r| {
                r.recommendation_type == RecommendationType::WeakenPathway
                    || r.recommendation_type == RecommendationType::PrunePathway
            });
            assert!(
                has_weaken_or_prune,
                "Should recommend weakening/pruning negative pathway"
            );
        }
    }

    #[test]
    fn test_signals_for_service() {
        let feedback = FeedbackLoop::new();

        let _ = feedback.generate_signal("synthex", "o1", true, 0.9, 0.8, "a->b");
        let _ = feedback.generate_signal("synthex", "o2", true, 0.8, 0.7, "a->c");
        let _ = feedback.generate_signal("nais", "o3", false, 0.5, 0.6, "d->e");

        let synthex_signals = feedback.get_signals_for_service("synthex");
        assert_eq!(synthex_signals.len(), 2);

        let nais_signals = feedback.get_signals_for_service("nais");
        assert_eq!(nais_signals.len(), 1);

        let empty_signals = feedback.get_signals_for_service("nonexistent");
        assert!(empty_signals.is_empty());
    }

    #[test]
    fn test_recent_signals() {
        let feedback = FeedbackLoop::new();

        for i in 0..10 {
            let _ = feedback.generate_signal(
                "test",
                &format!("o-{i}"),
                true,
                0.8,
                0.7,
                "a->b",
            );
        }

        // Get last 3
        let recent = feedback.get_recent_signals(3);
        assert_eq!(recent.len(), 3);

        // Get more than available
        let all = feedback.get_recent_signals(20);
        assert_eq!(all.len(), 10);

        // Get 0
        let none = feedback.get_recent_signals(0);
        assert!(none.is_empty());
    }

    #[test]
    fn test_signal_cap() {
        let feedback = FeedbackLoop::new();

        // Fill beyond capacity
        for i in 0..550 {
            let _ = feedback.generate_signal(
                "test",
                &format!("o-{i}"),
                true,
                0.8,
                0.7,
                "a->b",
            );
        }

        assert_eq!(feedback.signal_count(), SIGNAL_CAP);
    }

    #[test]
    fn test_clear_old_signals() {
        let feedback = FeedbackLoop::new();

        // Generate some signals
        for i in 0..5 {
            let _ = feedback.generate_signal(
                "test",
                &format!("o-{i}"),
                true,
                0.8,
                0.7,
                "a->b",
            );
        }

        assert_eq!(feedback.signal_count(), 5);

        // Clear signals older than a future timestamp (should clear all)
        let future = Utc::now() + chrono::Duration::hours(1);
        let removed = feedback.clear_old_signals(future);
        assert_eq!(removed, 5);
        assert_eq!(feedback.signal_count(), 0);
    }

    #[test]
    fn test_calibration_cap() {
        let feedback = FeedbackLoop::new();

        // Fill beyond calibration cap
        for i in 0..60 {
            let conf = (i as f64 % 10.0) / 10.0;
            let success = i % 2 == 0;
            let _ = feedback.record_calibration("test-svc", conf, success);
        }

        // Verify the data is capped
        let data = feedback.calibration_data.read();
        if let Some(entry) = data.get("test-svc") {
            assert_eq!(entry.predicted_confidences.len(), CALIBRATION_CAP);
            assert_eq!(entry.actual_outcomes.len(), CALIBRATION_CAP);
        }
    }

    #[test]
    fn test_get_recommendations_stored() {
        let feedback = FeedbackLoop::new();

        // Add signals that will trigger recommendations
        for i in 0..5 {
            let _ = feedback.generate_signal(
                "synthex",
                &format!("o-{i}"),
                true,
                0.95,
                0.8,
                "x->y",
            );
        }

        let _ = feedback.generate_recommendations("synthex");
        let stored = feedback.get_recommendations();
        assert!(!stored.is_empty());
    }

    #[test]
    fn test_empty_service_recommendation_error() {
        let feedback = FeedbackLoop::new();
        let result = feedback.generate_recommendations("");
        assert!(result.is_err());
    }

    #[test]
    fn test_no_signals_empty_recommendations() {
        let feedback = FeedbackLoop::new();
        let result = feedback.generate_recommendations("no-data-service");
        assert!(result.is_ok());
        if let Ok(recs) = result {
            assert!(recs.is_empty());
        }
    }

    #[test]
    fn test_default_impl() {
        let feedback = FeedbackLoop::default();
        assert_eq!(feedback.signal_count(), 0);
    }

    #[test]
    fn test_calibration_validation() {
        let feedback = FeedbackLoop::new();

        // Invalid confidence
        let result = feedback.record_calibration("test", 1.5, true);
        assert!(result.is_err());

        // Negative confidence
        let result = feedback.record_calibration("test", -0.1, true);
        assert!(result.is_err());
    }

    // -----------------------------------------------------------------------
    // 18. test_signal_has_uuid_id
    // -----------------------------------------------------------------------
    #[test]
    fn test_signal_has_uuid_id() {
        let feedback = FeedbackLoop::new();
        let result = feedback.generate_signal("svc", "o1", true, 0.9, 0.8, "a->b");
        assert!(result.is_ok());
        if let Ok(s) = result {
            assert!(!s.id.is_empty());
            assert_eq!(s.id.len(), 36);
        }
    }

    // -----------------------------------------------------------------------
    // 19. test_signal_timestamp
    // -----------------------------------------------------------------------
    #[test]
    fn test_signal_timestamp() {
        let feedback = FeedbackLoop::new();
        let before = Utc::now();
        let result = feedback.generate_signal("svc", "o1", true, 0.9, 0.8, "a->b");
        let after = Utc::now();
        assert!(result.is_ok());
        if let Ok(s) = result {
            assert!(s.timestamp >= before);
            assert!(s.timestamp <= after);
        }
    }

    // -----------------------------------------------------------------------
    // 20. test_reinforcement_signal_threshold
    // -----------------------------------------------------------------------
    #[test]
    fn test_reinforcement_signal_threshold() {
        let feedback = FeedbackLoop::new();
        // Exactly at threshold
        let result = feedback.generate_signal("svc", "o1", true, 0.7, 0.8, "a->b");
        assert!(result.is_ok());
        if let Ok(s) = result {
            assert_eq!(s.signal_type, SignalType::Reinforcement);
        }
    }

    // -----------------------------------------------------------------------
    // 21. test_exploration_signal_below_threshold
    // -----------------------------------------------------------------------
    #[test]
    fn test_exploration_signal_below_threshold() {
        let feedback = FeedbackLoop::new();
        // Just below threshold
        let result = feedback.generate_signal("svc", "o1", true, 0.69, 0.8, "a->b");
        assert!(result.is_ok());
        if let Ok(s) = result {
            assert_eq!(s.signal_type, SignalType::Exploration);
            // strength = 0.69 * 0.5 = 0.345
            assert!((s.strength - 0.69 * 0.5).abs() < f64::EPSILON);
        }
    }

    // -----------------------------------------------------------------------
    // 22. test_correction_with_nonzero_effectiveness
    // -----------------------------------------------------------------------
    #[test]
    fn test_correction_with_nonzero_effectiveness() {
        let feedback = FeedbackLoop::new();
        let result = feedback.generate_signal("svc", "o1", false, 0.5, 0.8, "a->b");
        assert!(result.is_ok());
        if let Ok(s) = result {
            assert_eq!(s.signal_type, SignalType::Correction);
            assert!((s.strength - (-0.5)).abs() < f64::EPSILON);
        }
    }

    // -----------------------------------------------------------------------
    // 23. test_calibration_offset_no_data
    // -----------------------------------------------------------------------
    #[test]
    fn test_calibration_offset_no_data() {
        let feedback = FeedbackLoop::new();
        let offset = feedback.get_calibration_offset("nonexistent");
        assert!((offset - 0.0).abs() < f64::EPSILON);
    }

    // -----------------------------------------------------------------------
    // 24. test_calibration_insufficient_samples
    // -----------------------------------------------------------------------
    #[test]
    fn test_calibration_insufficient_samples() {
        let feedback = FeedbackLoop::new();
        let _ = feedback.record_calibration("svc", 0.8, true);
        let offset = feedback.record_calibration("svc", 0.7, true);
        assert!(offset.is_ok());
        // Only 2 samples < MIN_CALIBRATION_SAMPLES(3), offset should be 0.0
        if let Ok(off) = offset {
            assert!((off - 0.0).abs() < f64::EPSILON);
        }
    }

    // -----------------------------------------------------------------------
    // 25. test_calibration_boundary_zero
    // -----------------------------------------------------------------------
    #[test]
    fn test_calibration_boundary_zero() {
        let feedback = FeedbackLoop::new();
        let result = feedback.record_calibration("svc", 0.0, false);
        assert!(result.is_ok());
    }

    // -----------------------------------------------------------------------
    // 26. test_calibration_boundary_one
    // -----------------------------------------------------------------------
    #[test]
    fn test_calibration_boundary_one() {
        let feedback = FeedbackLoop::new();
        let result = feedback.record_calibration("svc", 1.0, true);
        assert!(result.is_ok());
    }

    // -----------------------------------------------------------------------
    // 27. test_generate_signal_validation_effectiveness
    // -----------------------------------------------------------------------
    #[test]
    fn test_generate_signal_validation_effectiveness() {
        let feedback = FeedbackLoop::new();
        // Negative effectiveness
        let result = feedback.generate_signal("svc", "o1", true, -0.5, 0.8, "a->b");
        assert!(result.is_err());

        // Over 1.0 effectiveness
        let result = feedback.generate_signal("svc", "o2", true, 2.0, 0.8, "a->b");
        assert!(result.is_err());
    }

    // -----------------------------------------------------------------------
    // 28. test_generate_signal_validation_confidence
    // -----------------------------------------------------------------------
    #[test]
    fn test_generate_signal_validation_confidence() {
        let feedback = FeedbackLoop::new();
        // Negative confidence
        let result = feedback.generate_signal("svc", "o1", true, 0.5, -0.1, "a->b");
        assert!(result.is_err());

        // Over 1.0 confidence
        let result = feedback.generate_signal("svc", "o2", true, 0.5, 1.5, "a->b");
        assert!(result.is_err());
    }

    // -----------------------------------------------------------------------
    // 29. test_recent_signals_zero_count
    // -----------------------------------------------------------------------
    #[test]
    fn test_recent_signals_zero_count() {
        let feedback = FeedbackLoop::new();
        let _ = feedback.generate_signal("svc", "o1", true, 0.9, 0.8, "a->b");
        let recent = feedback.get_recent_signals(0);
        assert!(recent.is_empty());
    }

    // -----------------------------------------------------------------------
    // 30. test_clear_old_signals_preserves_recent
    // -----------------------------------------------------------------------
    #[test]
    fn test_clear_old_signals_preserves_recent() {
        let feedback = FeedbackLoop::new();
        for i in 0..5 {
            let _ = feedback.generate_signal(
                "svc", &format!("o{i}"), true, 0.8, 0.7, "a->b",
            );
        }

        // Clear signals older than a past timestamp (should clear nothing)
        let past = Utc::now() - chrono::Duration::hours(1);
        let removed = feedback.clear_old_signals(past);
        assert_eq!(removed, 0);
        assert_eq!(feedback.signal_count(), 5);
    }

    // -----------------------------------------------------------------------
    // 31. test_recommendation_empty_service_error
    // -----------------------------------------------------------------------
    #[test]
    fn test_recommendation_empty_service_error() {
        let feedback = FeedbackLoop::new();
        let result = feedback.generate_recommendations("");
        assert!(result.is_err());
    }

    // -----------------------------------------------------------------------
    // 32. test_recommendation_no_signals_empty
    // -----------------------------------------------------------------------
    #[test]
    fn test_recommendation_no_signals_empty() {
        let feedback = FeedbackLoop::new();
        let result = feedback.generate_recommendations("no-data");
        assert!(result.is_ok());
        if let Ok(recs) = result {
            assert!(recs.is_empty());
        }
    }

    // -----------------------------------------------------------------------
    // 33. test_recommendation_strengthen_pathway
    // -----------------------------------------------------------------------
    #[test]
    fn test_recommendation_strengthen_pathway() {
        let feedback = FeedbackLoop::new();
        // 10 high-effectiveness successes -> should recommend StrengthenPathway
        for i in 0..10 {
            let _ = feedback.generate_signal(
                "svc", &format!("o{i}"), true, 0.95, 0.8, "p->q",
            );
        }
        let recs = feedback.generate_recommendations("svc");
        assert!(recs.is_ok());
        if let Ok(r) = recs {
            let has_strengthen = r
                .iter()
                .any(|rec| rec.recommendation_type == RecommendationType::StrengthenPathway);
            assert!(has_strengthen);
        }
    }

    // -----------------------------------------------------------------------
    // 34. test_recommendation_weaken_pathway
    // -----------------------------------------------------------------------
    #[test]
    fn test_recommendation_weaken_pathway() {
        let feedback = FeedbackLoop::new();
        // Moderate negative signals -> should recommend WeakenPathway
        for i in 0..5 {
            let _ = feedback.generate_signal(
                "svc", &format!("o{i}"), false, 0.5, 0.6, "weak->path",
            );
        }
        let recs = feedback.generate_recommendations("svc");
        assert!(recs.is_ok());
        if let Ok(r) = recs {
            let has_weaken = r
                .iter()
                .any(|rec| rec.recommendation_type == RecommendationType::WeakenPathway
                    || rec.recommendation_type == RecommendationType::PrunePathway);
            assert!(has_weaken);
        }
    }

    // -----------------------------------------------------------------------
    // 35. test_recommendation_adjust_threshold
    // -----------------------------------------------------------------------
    #[test]
    fn test_recommendation_adjust_threshold() {
        let feedback = FeedbackLoop::new();
        // Create large calibration offset
        for _ in 0..5 {
            let _ = feedback.record_calibration("svc-thresh", 0.9, false);
        }

        // Need a signal for the service to trigger recommendation generation
        let _ = feedback.generate_signal(
            "svc-thresh", "o1", true, 0.9, 0.8, "x->y",
        );

        let recs = feedback.generate_recommendations("svc-thresh");
        assert!(recs.is_ok());
        if let Ok(r) = recs {
            let has_threshold = r
                .iter()
                .any(|rec| rec.recommendation_type == RecommendationType::AdjustThreshold);
            assert!(has_threshold);
        }
    }

    // -----------------------------------------------------------------------
    // 36. test_recommendation_has_reason
    // -----------------------------------------------------------------------
    #[test]
    fn test_recommendation_has_reason() {
        let feedback = FeedbackLoop::new();
        for i in 0..5 {
            let _ = feedback.generate_signal(
                "svc", &format!("o{i}"), true, 0.95, 0.8, "a->b",
            );
        }
        let recs = feedback.generate_recommendations("svc");
        assert!(recs.is_ok());
        if let Ok(r) = recs {
            for rec in &r {
                assert!(!rec.reason.is_empty());
                assert!(!rec.pathway_key.is_empty());
                assert!(!rec.id.is_empty());
            }
        }
    }

    // -----------------------------------------------------------------------
    // 37. test_signals_for_multiple_services
    // -----------------------------------------------------------------------
    #[test]
    fn test_signals_for_multiple_services() {
        let feedback = FeedbackLoop::new();
        for i in 0..3 {
            let _ = feedback.generate_signal(
                "svc-a", &format!("oa{i}"), true, 0.9, 0.8, "a->b",
            );
        }
        for i in 0..2 {
            let _ = feedback.generate_signal(
                "svc-b", &format!("ob{i}"), false, 0.3, 0.5, "c->d",
            );
        }

        assert_eq!(feedback.get_signals_for_service("svc-a").len(), 3);
        assert_eq!(feedback.get_signals_for_service("svc-b").len(), 2);
        assert_eq!(feedback.signal_count(), 5);
    }

    // -----------------------------------------------------------------------
    // 38. test_calibration_signal_type_override
    // -----------------------------------------------------------------------
    #[test]
    fn test_calibration_signal_type_override() {
        let feedback = FeedbackLoop::new();
        // Create large calibration offset (> 0.2)
        for _ in 0..5 {
            let _ = feedback.record_calibration("svc-cal", 0.9, false);
        }

        // Now generate a success signal - should be overridden to Calibration
        let result = feedback.generate_signal(
            "svc-cal", "o1", true, 0.95, 0.8, "a->b",
        );
        assert!(result.is_ok());
        if let Ok(s) = result {
            assert_eq!(s.signal_type, SignalType::Calibration);
        }
    }

    // -----------------------------------------------------------------------
    // 39. test_get_recommendations_accumulates
    // -----------------------------------------------------------------------
    #[test]
    fn test_get_recommendations_accumulates() {
        let feedback = FeedbackLoop::new();

        // First batch
        for i in 0..5 {
            let _ = feedback.generate_signal(
                "svc", &format!("o{i}"), true, 0.95, 0.8, "x->y",
            );
        }
        let _ = feedback.generate_recommendations("svc");
        let count1 = feedback.get_recommendations().len();

        // Second batch
        for i in 5..10 {
            let _ = feedback.generate_signal(
                "svc", &format!("o{i}"), true, 0.95, 0.8, "x->y",
            );
        }
        let _ = feedback.generate_recommendations("svc");
        let count2 = feedback.get_recommendations().len();

        assert!(count2 >= count1);
    }

    // -----------------------------------------------------------------------
    // 40. test_signal_pathway_key_preserved
    // -----------------------------------------------------------------------
    #[test]
    fn test_signal_pathway_key_preserved() {
        let feedback = FeedbackLoop::new();
        let result = feedback.generate_signal(
            "svc", "o1", true, 0.9, 0.8, "source->target",
        );
        assert!(result.is_ok());
        if let Ok(s) = result {
            assert_eq!(s.pathway_key, "source->target");
        }
    }

    // -----------------------------------------------------------------------
    // 41. test_multiple_pathways_independent_recommendations
    // -----------------------------------------------------------------------
    #[test]
    fn test_multiple_pathways_independent_recommendations() {
        let feedback = FeedbackLoop::new();
        // Positive signals on pathway A
        for i in 0..5 {
            let _ = feedback.generate_signal(
                "svc", &format!("oA{i}"), true, 0.95, 0.8, "pathA",
            );
        }
        // Negative signals on pathway B
        for i in 0..5 {
            let _ = feedback.generate_signal(
                "svc", &format!("oB{i}"), false, 0.6, 0.5, "pathB",
            );
        }

        let recs = feedback.generate_recommendations("svc");
        assert!(recs.is_ok());
        if let Ok(r) = recs {
            let path_a_rec = r.iter().find(|rec| rec.pathway_key == "pathA");
            let path_b_rec = r.iter().find(|rec| rec.pathway_key == "pathB");

            if let Some(a) = path_a_rec {
                assert_eq!(a.recommendation_type, RecommendationType::StrengthenPathway);
            }
            if let Some(b) = path_b_rec {
                assert!(
                    b.recommendation_type == RecommendationType::WeakenPathway
                        || b.recommendation_type == RecommendationType::PrunePathway
                );
            }
        }
    }

    // -----------------------------------------------------------------------
    // 42. test_calibration_data_multiple_services
    // -----------------------------------------------------------------------
    #[test]
    fn test_calibration_data_multiple_services() {
        let feedback = FeedbackLoop::new();

        for _ in 0..5 {
            let _ = feedback.record_calibration("svc-a", 0.8, true);
        }
        for _ in 0..5 {
            let _ = feedback.record_calibration("svc-b", 0.3, false);
        }

        let offset_a = feedback.get_calibration_offset("svc-a");
        let offset_b = feedback.get_calibration_offset("svc-b");

        assert!(offset_a > 0.0); // under-confident (predicted 0.8, actual 1.0)
        assert!(offset_b < 0.0); // over-confident (predicted 0.3, actual 0.0)
    }

    // -----------------------------------------------------------------------
    // 43. test_signal_count_after_clear
    // -----------------------------------------------------------------------
    #[test]
    fn test_signal_count_after_clear() {
        let feedback = FeedbackLoop::new();
        for i in 0..10 {
            let _ = feedback.generate_signal(
                "svc", &format!("o{i}"), true, 0.8, 0.7, "a->b",
            );
        }
        assert_eq!(feedback.signal_count(), 10);

        let future = Utc::now() + chrono::Duration::hours(1);
        let removed = feedback.clear_old_signals(future);
        assert_eq!(removed, 10);
        assert_eq!(feedback.signal_count(), 0);
    }

    // -----------------------------------------------------------------------
    // 44. test_signal_strength_zero_effectiveness_success
    // -----------------------------------------------------------------------
    #[test]
    fn test_signal_strength_zero_effectiveness_success() {
        let feedback = FeedbackLoop::new();
        let result = feedback.generate_signal("svc", "o1", true, 0.0, 0.5, "a->b");
        assert!(result.is_ok());
        if let Ok(s) = result {
            // 0.0 < 0.7 threshold -> Exploration, strength = 0.0 * 0.5 = 0.0
            assert_eq!(s.signal_type, SignalType::Exploration);
            assert!((s.strength - 0.0).abs() < f64::EPSILON);
        }
    }

    // -----------------------------------------------------------------------
    // 45. test_recommendation_suggested_delta_sign
    // -----------------------------------------------------------------------
    #[test]
    fn test_recommendation_suggested_delta_sign() {
        let feedback = FeedbackLoop::new();
        for i in 0..8 {
            let _ = feedback.generate_signal(
                "svc", &format!("o{i}"), true, 0.95, 0.8, "strong->path",
            );
        }
        let recs = feedback.generate_recommendations("svc");
        assert!(recs.is_ok());
        if let Ok(r) = recs {
            for rec in &r {
                if rec.recommendation_type == RecommendationType::StrengthenPathway {
                    assert!(rec.suggested_delta > 0.0);
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // 46. test_recent_signals_ordering
    // -----------------------------------------------------------------------
    #[test]
    fn test_recent_signals_ordering() {
        let feedback = FeedbackLoop::new();
        for i in 0..5 {
            let _ = feedback.generate_signal(
                "svc", &format!("o-{i}"), true, 0.8, 0.7, "a->b",
            );
        }

        let recent = feedback.get_recent_signals(5);
        assert_eq!(recent.len(), 5);
        // Ordered oldest to newest
        for i in 0..4 {
            assert!(recent[i].timestamp <= recent[i + 1].timestamp);
        }
    }

    // -----------------------------------------------------------------------
    // 47. test_calibration_offset_perfectly_calibrated
    // -----------------------------------------------------------------------
    #[test]
    fn test_calibration_offset_perfectly_calibrated() {
        let feedback = FeedbackLoop::new();
        // Predict 0.5, succeed 50% of the time
        let _ = feedback.record_calibration("svc", 0.5, true);
        let _ = feedback.record_calibration("svc", 0.5, false);
        let _ = feedback.record_calibration("svc", 0.5, true);
        let _ = feedback.record_calibration("svc", 0.5, false);

        let offset = feedback.get_calibration_offset("svc");
        // actual = 2/4 = 0.5, predicted avg = 0.5, offset = 0.0
        assert!(offset.abs() < f64::EPSILON);
    }

    // -----------------------------------------------------------------------
    // 48. test_prune_pathway_recommendation
    // -----------------------------------------------------------------------
    #[test]
    fn test_prune_pathway_recommendation() {
        let feedback = FeedbackLoop::new();
        // Very negative signals with high failure effectiveness
        for i in 0..10 {
            let _ = feedback.generate_signal(
                "svc", &format!("o{i}"), false, 0.9, 0.5, "bad->path",
            );
        }
        let recs = feedback.generate_recommendations("svc");
        assert!(recs.is_ok());
        if let Ok(r) = recs {
            let has_prune = r
                .iter()
                .any(|rec| rec.recommendation_type == RecommendationType::PrunePathway);
            assert!(has_prune);
        }
    }

    // -----------------------------------------------------------------------
    // 49. test_mixed_signals_no_recommendation
    // -----------------------------------------------------------------------
    #[test]
    fn test_mixed_signals_no_recommendation() {
        let feedback = FeedbackLoop::new();
        // Equal positive and negative signals -> avg near 0, no strong recommendation
        for i in 0..5 {
            let _ = feedback.generate_signal(
                "svc", &format!("pos{i}"), true, 0.8, 0.7, "mixed->path",
            );
        }
        for i in 0..5 {
            let _ = feedback.generate_signal(
                "svc", &format!("neg{i}"), false, 0.8, 0.7, "mixed->path",
            );
        }
        let recs = feedback.generate_recommendations("svc");
        assert!(recs.is_ok());
        // With mixed signals, avg strength is near 0, so no strengthen/weaken
        // for that specific pathway (might get calibration rec though)
        if let Ok(r) = recs {
            let mixed_path_rec = r
                .iter()
                .filter(|rec| rec.pathway_key == "mixed->path")
                .count();
            assert_eq!(mixed_path_rec, 0);
        }
    }

    // -----------------------------------------------------------------------
    // 50. test_feedback_loop_new_empty_state
    // -----------------------------------------------------------------------
    #[test]
    fn test_feedback_loop_new_empty_state() {
        let feedback = FeedbackLoop::new();
        assert_eq!(feedback.signal_count(), 0);
        assert!(feedback.get_recommendations().is_empty());
        assert!(feedback.get_signals_for_service("any").is_empty());
        assert!(feedback.get_recent_signals(10).is_empty());
        assert!((feedback.get_calibration_offset("any") - 0.0).abs() < f64::EPSILON);
    }
}
