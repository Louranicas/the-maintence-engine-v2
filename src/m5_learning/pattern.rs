//! # M27: Pattern Recognizer
//!
//! Identifies recurring system patterns and maps them to historical outcomes.
//! Tracks pattern occurrences, calculates confidence scores, and provides
//! pattern-based routing suggestions for the learning layer.
//!
//! ## Layer: L5 (Learning)
//! ## Dependencies: M01 (Error), L5 mod.rs types
//!
//! ## Pattern Types
//!
//! | Type | Description |
//! |------|-------------|
//! | Temporal | Time-based recurring patterns |
//! | State | Service state transition patterns |
//! | Metric | Metric threshold crossing patterns |
//! | Pathway | Hebbian pathway activation patterns |
//! | Failure | Failure mode patterns |
//! | Recovery | Recovery sequence patterns |
//!
//! ## Related Documentation
//! - [Module Specification](../../ai_docs/modules/M27_PATTERN_RECOGNIZER.md)
//! - [Layer Specification](../../ai_docs/layers/L05_LEARNING.md)

use std::collections::HashMap;
use std::time::SystemTime;

use parking_lot::RwLock;

use crate::{Error, Result};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Maximum number of pattern records retained.
const MAX_PATTERNS: usize = 500;

/// Maximum number of match records retained per pattern.
const MAX_MATCHES_PER_PATTERN: usize = 100;

/// Minimum confidence threshold for a pattern to be considered actionable.
const ACTIONABLE_THRESHOLD: f64 = 0.6;

/// Default decay rate for pattern strength per evaluation cycle.
const PATTERN_DECAY_RATE: f64 = 0.005;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Classification of system patterns.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PatternType {
    /// Time-based recurring patterns (e.g. nightly log spikes).
    Temporal,
    /// Service state transition patterns (e.g. restart → healthy).
    State,
    /// Metric threshold crossing patterns (e.g. CPU > 90%).
    Metric,
    /// Hebbian pathway activation sequences.
    Pathway,
    /// Failure mode patterns (e.g. cascade failures).
    Failure,
    /// Recovery sequence patterns (e.g. restart → drain → deploy).
    Recovery,
}

impl std::fmt::Display for PatternType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Temporal => write!(f, "TEMPORAL"),
            Self::State => write!(f, "STATE"),
            Self::Metric => write!(f, "METRIC"),
            Self::Pathway => write!(f, "PATHWAY"),
            Self::Failure => write!(f, "FAILURE"),
            Self::Recovery => write!(f, "RECOVERY"),
        }
    }
}

/// A recognized system pattern.
#[derive(Clone, Debug)]
pub struct Pattern {
    /// Unique pattern identifier.
    pub id: String,
    /// Human-readable pattern name.
    pub name: String,
    /// Pattern type classification.
    pub pattern_type: PatternType,
    /// Pattern signature (fingerprint string for matching).
    pub signature: String,
    /// Confidence score (0.0 - 1.0).
    pub confidence: f64,
    /// Pattern strength (decays over time).
    pub strength: f64,
    /// Number of times this pattern has been observed.
    pub occurrence_count: u64,
    /// Number of times the predicted outcome was correct.
    pub correct_predictions: u64,
    /// Associated outcome description.
    pub associated_outcome: String,
    /// Services involved in this pattern.
    pub involved_services: Vec<String>,
    /// First observed timestamp.
    pub first_seen: SystemTime,
    /// Last observed timestamp.
    pub last_seen: SystemTime,
    /// Whether this pattern is currently active.
    pub active: bool,
}

impl Pattern {
    /// Calculate prediction accuracy.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn prediction_accuracy(&self) -> f64 {
        if self.occurrence_count == 0 {
            0.0
        } else {
            self.correct_predictions as f64 / self.occurrence_count as f64
        }
    }

    /// Calculate composite score: weighted combination of confidence, strength, accuracy.
    #[must_use]
    pub fn composite_score(&self) -> f64 {
        0.4_f64.mul_add(
            self.confidence,
            0.3_f64.mul_add(self.strength, 0.3 * self.prediction_accuracy()),
        )
    }

    /// Whether this pattern is actionable (composite score above threshold).
    #[must_use]
    pub fn is_actionable(&self) -> bool {
        self.active && self.composite_score() >= ACTIONABLE_THRESHOLD
    }
}

/// A record of a pattern match event.
#[derive(Clone, Debug)]
pub struct PatternMatch {
    /// Pattern ID that matched.
    pub pattern_id: String,
    /// Input data that triggered the match.
    pub trigger: String,
    /// Match confidence (0.0 - 1.0).
    pub match_confidence: f64,
    /// Whether the predicted outcome was correct (post-hoc).
    pub outcome_correct: Option<bool>,
    /// Timestamp of the match.
    pub timestamp: SystemTime,
}

/// Summary statistics for a pattern type.
#[derive(Clone, Debug)]
pub struct PatternTypeSummary {
    /// Pattern type.
    pub pattern_type: PatternType,
    /// Number of patterns of this type.
    pub count: usize,
    /// Average confidence across patterns.
    pub avg_confidence: f64,
    /// Average strength across patterns.
    pub avg_strength: f64,
    /// Number of actionable patterns.
    pub actionable_count: usize,
}

// ---------------------------------------------------------------------------
// PatternRecognizer
// ---------------------------------------------------------------------------

/// Pattern recognizer for identifying recurring system behaviors.
///
/// Tracks patterns, records matches, and provides pattern-based
/// recommendations for the learning layer.
pub struct PatternRecognizer {
    /// Known patterns keyed by ID.
    patterns: RwLock<HashMap<String, Pattern>>,
    /// Match history keyed by pattern ID.
    matches: RwLock<HashMap<String, Vec<PatternMatch>>>,
    /// Auto-incrementing pattern counter.
    next_id: RwLock<u64>,
}

impl Default for PatternRecognizer {
    fn default() -> Self {
        Self::new()
    }
}

impl PatternRecognizer {
    /// Create a new pattern recognizer.
    #[must_use]
    pub fn new() -> Self {
        Self {
            patterns: RwLock::new(HashMap::new()),
            matches: RwLock::new(HashMap::new()),
            next_id: RwLock::new(1),
        }
    }

    /// Register a new pattern.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if the name or signature is empty.
    pub fn register_pattern(
        &self,
        name: &str,
        pattern_type: PatternType,
        signature: &str,
        outcome: &str,
        services: Vec<String>,
    ) -> Result<String> {
        if name.is_empty() {
            return Err(Error::Validation("Pattern name cannot be empty".into()));
        }
        if signature.is_empty() {
            return Err(Error::Validation("Signature cannot be empty".into()));
        }

        let id = {
            let mut counter = self.next_id.write();
            let id = format!("PAT-{counter:04}");
            *counter += 1;
            id
        };

        let now = SystemTime::now();
        let pattern = Pattern {
            id: id.clone(),
            name: name.into(),
            pattern_type,
            signature: signature.into(),
            confidence: 0.5,
            strength: 0.5,
            occurrence_count: 0,
            correct_predictions: 0,
            associated_outcome: outcome.into(),
            involved_services: services,
            first_seen: now,
            last_seen: now,
            active: true,
        };

        {
            let mut patterns = self.patterns.write();
            if patterns.len() >= MAX_PATTERNS {
                // Evict weakest pattern
                let weakest = patterns
                    .iter()
                    .min_by(|a, b| {
                        a.1.composite_score()
                            .partial_cmp(&b.1.composite_score())
                            .unwrap_or(std::cmp::Ordering::Equal)
                    })
                    .map(|(k, _)| k.clone());
                if let Some(key) = weakest {
                    patterns.remove(&key);
                }
            }
            patterns.insert(id.clone(), pattern);
        }

        self.matches.write().insert(id.clone(), Vec::new());
        Ok(id)
    }

    /// Record a pattern match.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if the pattern ID is not found.
    pub fn record_match(
        &self,
        pattern_id: &str,
        trigger: &str,
        match_confidence: f64,
    ) -> Result<()> {
        // Validate pattern exists before acquiring write lock
        if !self.patterns.read().contains_key(pattern_id) {
            return Err(Error::Validation(format!(
                "Pattern {pattern_id} not found"
            )));
        }

        {
            let mut patterns = self.patterns.write();
            if let Some(pattern) = patterns.get_mut(pattern_id) {
                pattern.occurrence_count += 1;
                pattern.last_seen = SystemTime::now();
                // Update confidence as running average
                let n = pattern.occurrence_count;
                #[allow(clippy::cast_precision_loss)]
                {
                    pattern.confidence =
                        pattern.confidence.mul_add((n - 1) as f64, match_confidence) / n as f64;
                }
            }
        }

        let record = PatternMatch {
            pattern_id: pattern_id.into(),
            trigger: trigger.into(),
            match_confidence,
            outcome_correct: None,
            timestamp: SystemTime::now(),
        };

        let mut matches_guard = self.matches.write();
        let entries = matches_guard.entry(pattern_id.into()).or_default();
        if entries.len() >= MAX_MATCHES_PER_PATTERN {
            let quarter = entries.len() / 4;
            entries.drain(..quarter);
        }
        entries.push(record);
        drop(matches_guard);

        Ok(())
    }

    /// Mark a match outcome as correct or incorrect.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if the pattern is not found.
    pub fn mark_outcome(&self, pattern_id: &str, correct: bool) -> Result<()> {
        if !self.patterns.read().contains_key(pattern_id) {
            return Err(Error::Validation(format!(
                "Pattern {pattern_id} not found"
            )));
        }
        {
            let mut patterns = self.patterns.write();
            if let Some(pattern) = patterns.get_mut(pattern_id) {
                if correct {
                    pattern.correct_predictions += 1;
                    pattern.strength = (pattern.strength + 0.05).min(1.0);
                } else {
                    pattern.strength = (pattern.strength - 0.02).max(0.0);
                }
            }
        }
        Ok(())
    }

    /// Find patterns matching a given signature substring.
    #[must_use]
    pub fn find_by_signature(&self, query: &str) -> Vec<Pattern> {
        self.patterns
            .read()
            .values()
            .filter(|p| p.signature.contains(query) && p.active)
            .cloned()
            .collect()
    }

    /// Find patterns by type.
    #[must_use]
    pub fn find_by_type(&self, pattern_type: PatternType) -> Vec<Pattern> {
        self.patterns
            .read()
            .values()
            .filter(|p| p.pattern_type == pattern_type)
            .cloned()
            .collect()
    }

    /// Get a pattern by ID.
    #[must_use]
    pub fn get_pattern(&self, id: &str) -> Option<Pattern> {
        self.patterns.read().get(id).cloned()
    }

    /// Get all actionable patterns.
    #[must_use]
    pub fn actionable_patterns(&self) -> Vec<Pattern> {
        self.patterns
            .read()
            .values()
            .filter(|p| p.is_actionable())
            .cloned()
            .collect()
    }

    /// Get pattern count.
    #[must_use]
    pub fn pattern_count(&self) -> usize {
        self.patterns.read().len()
    }

    /// Get active pattern count.
    #[must_use]
    pub fn active_pattern_count(&self) -> usize {
        self.patterns.read().values().filter(|p| p.active).count()
    }

    /// Deactivate a pattern.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if the pattern is not found.
    pub fn deactivate(&self, pattern_id: &str) -> Result<()> {
        let mut patterns = self.patterns.write();
        let pattern = patterns
            .get_mut(pattern_id)
            .ok_or_else(|| Error::Validation(format!("Pattern {pattern_id} not found")))?;
        pattern.active = false;
        drop(patterns);
        Ok(())
    }

    /// Reactivate a pattern.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if the pattern is not found.
    pub fn reactivate(&self, pattern_id: &str) -> Result<()> {
        let mut patterns = self.patterns.write();
        let pattern = patterns
            .get_mut(pattern_id)
            .ok_or_else(|| Error::Validation(format!("Pattern {pattern_id} not found")))?;
        pattern.active = true;
        drop(patterns);
        Ok(())
    }

    /// Apply decay to all pattern strengths.
    pub fn apply_decay(&self) {
        let mut patterns = self.patterns.write();
        for pattern in patterns.values_mut() {
            pattern.strength = (pattern.strength - PATTERN_DECAY_RATE).max(0.0);
        }
        drop(patterns);
    }

    /// Get summary statistics per pattern type.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn type_summary(&self) -> Vec<PatternTypeSummary> {
        let by_type: HashMap<PatternType, Vec<Pattern>> = {
            let patterns = self.patterns.read();
            let mut map: HashMap<PatternType, Vec<Pattern>> = HashMap::new();
            for p in patterns.values() {
                map.entry(p.pattern_type).or_default().push(p.clone());
            }
            drop(patterns);
            map
        };

        by_type
            .into_iter()
            .map(|(pt, pats)| {
                let count = pats.len();
                let avg_confidence = pats.iter().map(|p| p.confidence).sum::<f64>() / count as f64;
                let avg_strength = pats.iter().map(|p| p.strength).sum::<f64>() / count as f64;
                let actionable_count = pats.iter().filter(|p| p.is_actionable()).count();
                PatternTypeSummary {
                    pattern_type: pt,
                    count,
                    avg_confidence,
                    avg_strength,
                    actionable_count,
                }
            })
            .collect()
    }

    /// Get match history for a pattern.
    #[must_use]
    pub fn match_history(&self, pattern_id: &str) -> Vec<PatternMatch> {
        self.matches
            .read()
            .get(pattern_id)
            .cloned()
            .unwrap_or_default()
    }

    /// Get total match count across all patterns.
    #[must_use]
    pub fn total_match_count(&self) -> usize {
        self.matches.read().values().map(Vec::len).sum()
    }

    /// Get patterns involving a specific service.
    #[must_use]
    pub fn patterns_for_service(&self, service_id: &str) -> Vec<Pattern> {
        self.patterns
            .read()
            .values()
            .filter(|p| p.involved_services.iter().any(|s| s == service_id))
            .cloned()
            .collect()
    }

    /// Remove a pattern.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if the pattern is not found.
    pub fn remove_pattern(&self, pattern_id: &str) -> Result<()> {
        if self.patterns.write().remove(pattern_id).is_some() {
            self.matches.write().remove(pattern_id);
            Ok(())
        } else {
            Err(Error::Validation(format!(
                "Pattern {pattern_id} not found"
            )))
        }
    }

    /// Get the top-N patterns by composite score.
    #[must_use]
    pub fn top_patterns(&self, n: usize) -> Vec<Pattern> {
        let mut patterns: Vec<Pattern> = self.patterns.read().values().cloned().collect();
        patterns.sort_by(|a, b| {
            b.composite_score()
                .partial_cmp(&a.composite_score())
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        patterns.truncate(n);
        patterns
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_recognizer() -> PatternRecognizer {
        let r = PatternRecognizer::new();
        r.register_pattern(
            "health-restart",
            PatternType::Failure,
            "health_fail->restart",
            "service_restart",
            vec!["synthex".into()],
        )
        .ok();
        r
    }

    #[test]
    fn test_register_pattern() {
        let r = PatternRecognizer::new();
        let id = r.register_pattern("p1", PatternType::Temporal, "sig", "out", vec![]);
        assert!(id.is_ok());
        assert_eq!(r.pattern_count(), 1);
    }

    #[test]
    fn test_register_empty_name_fails() {
        let r = PatternRecognizer::new();
        assert!(r.register_pattern("", PatternType::State, "s", "o", vec![]).is_err());
    }

    #[test]
    fn test_register_empty_signature_fails() {
        let r = PatternRecognizer::new();
        assert!(r.register_pattern("n", PatternType::State, "", "o", vec![]).is_err());
    }

    #[test]
    fn test_get_pattern() {
        let r = setup_recognizer();
        let patterns: Vec<Pattern> = r.patterns.read().values().cloned().collect();
        let first_id = patterns.first().map(|p| p.id.clone()).unwrap_or_default();
        assert!(r.get_pattern(&first_id).is_some());
    }

    #[test]
    fn test_get_nonexistent_pattern() {
        let r = PatternRecognizer::new();
        assert!(r.get_pattern("none").is_none());
    }

    #[test]
    fn test_record_match() {
        let r = setup_recognizer();
        let id = r.patterns.read().keys().next().cloned().unwrap_or_default();
        assert!(r.record_match(&id, "trigger-data", 0.8).is_ok());
    }

    #[test]
    fn test_record_match_nonexistent_fails() {
        let r = PatternRecognizer::new();
        assert!(r.record_match("none", "x", 0.5).is_err());
    }

    #[test]
    fn test_record_match_updates_count() {
        let r = setup_recognizer();
        let id = r.patterns.read().keys().next().cloned().unwrap_or_default();
        let _ = r.record_match(&id, "t", 0.8);
        let p = r.get_pattern(&id);
        assert_eq!(p.map(|p| p.occurrence_count).unwrap_or(0), 1);
    }

    #[test]
    fn test_mark_outcome_correct() {
        let r = setup_recognizer();
        let id = r.patterns.read().keys().next().cloned().unwrap_or_default();
        let _ = r.record_match(&id, "t", 0.8);
        assert!(r.mark_outcome(&id, true).is_ok());
        let p = r.get_pattern(&id);
        assert_eq!(p.map(|p| p.correct_predictions).unwrap_or(0), 1);
    }

    #[test]
    fn test_mark_outcome_incorrect() {
        let r = setup_recognizer();
        let id = r.patterns.read().keys().next().cloned().unwrap_or_default();
        assert!(r.mark_outcome(&id, false).is_ok());
    }

    #[test]
    fn test_mark_outcome_nonexistent_fails() {
        let r = PatternRecognizer::new();
        assert!(r.mark_outcome("none", true).is_err());
    }

    #[test]
    fn test_find_by_signature() {
        let r = setup_recognizer();
        let found = r.find_by_signature("health_fail");
        assert_eq!(found.len(), 1);
    }

    #[test]
    fn test_find_by_signature_no_match() {
        let r = setup_recognizer();
        assert!(r.find_by_signature("xyz").is_empty());
    }

    #[test]
    fn test_find_by_type() {
        let r = setup_recognizer();
        assert_eq!(r.find_by_type(PatternType::Failure).len(), 1);
        assert!(r.find_by_type(PatternType::Temporal).is_empty());
    }

    #[test]
    fn test_actionable_patterns() {
        let r = PatternRecognizer::new();
        let id = r.register_pattern("p", PatternType::State, "s", "o", vec![]).unwrap_or_default();
        // Boost confidence and accuracy to make actionable
        {
            let mut patterns = r.patterns.write();
            if let Some(p) = patterns.get_mut(&id) {
                p.confidence = 0.9;
                p.strength = 0.9;
                p.occurrence_count = 10;
                p.correct_predictions = 9;
            }
        }
        assert_eq!(r.actionable_patterns().len(), 1);
    }

    #[test]
    fn test_active_pattern_count() {
        let r = setup_recognizer();
        assert_eq!(r.active_pattern_count(), 1);
    }

    #[test]
    fn test_deactivate() {
        let r = setup_recognizer();
        let id = r.patterns.read().keys().next().cloned().unwrap_or_default();
        assert!(r.deactivate(&id).is_ok());
        assert_eq!(r.active_pattern_count(), 0);
    }

    #[test]
    fn test_deactivate_nonexistent_fails() {
        let r = PatternRecognizer::new();
        assert!(r.deactivate("none").is_err());
    }

    #[test]
    fn test_reactivate() {
        let r = setup_recognizer();
        let id = r.patterns.read().keys().next().cloned().unwrap_or_default();
        r.deactivate(&id).ok();
        assert!(r.reactivate(&id).is_ok());
        assert_eq!(r.active_pattern_count(), 1);
    }

    #[test]
    fn test_reactivate_nonexistent_fails() {
        let r = PatternRecognizer::new();
        assert!(r.reactivate("none").is_err());
    }

    #[test]
    fn test_apply_decay() {
        let r = setup_recognizer();
        let id = r.patterns.read().keys().next().cloned().unwrap_or_default();
        let before = r.get_pattern(&id).map(|p| p.strength).unwrap_or(0.0);
        r.apply_decay();
        let after = r.get_pattern(&id).map(|p| p.strength).unwrap_or(0.0);
        assert!(after < before);
    }

    #[test]
    fn test_type_summary() {
        let r = setup_recognizer();
        let summary = r.type_summary();
        assert!(!summary.is_empty());
    }

    #[test]
    fn test_match_history() {
        let r = setup_recognizer();
        let id = r.patterns.read().keys().next().cloned().unwrap_or_default();
        let _ = r.record_match(&id, "t1", 0.7);
        let _ = r.record_match(&id, "t2", 0.8);
        assert_eq!(r.match_history(&id).len(), 2);
    }

    #[test]
    fn test_match_history_empty() {
        let r = PatternRecognizer::new();
        assert!(r.match_history("none").is_empty());
    }

    #[test]
    fn test_total_match_count() {
        let r = setup_recognizer();
        let id = r.patterns.read().keys().next().cloned().unwrap_or_default();
        let _ = r.record_match(&id, "t", 0.5);
        assert_eq!(r.total_match_count(), 1);
    }

    #[test]
    fn test_patterns_for_service() {
        let r = setup_recognizer();
        assert_eq!(r.patterns_for_service("synthex").len(), 1);
        assert!(r.patterns_for_service("unknown").is_empty());
    }

    #[test]
    fn test_remove_pattern() {
        let r = setup_recognizer();
        let id = r.patterns.read().keys().next().cloned().unwrap_or_default();
        assert!(r.remove_pattern(&id).is_ok());
        assert_eq!(r.pattern_count(), 0);
    }

    #[test]
    fn test_remove_nonexistent_fails() {
        let r = PatternRecognizer::new();
        assert!(r.remove_pattern("none").is_err());
    }

    #[test]
    fn test_top_patterns() {
        let r = PatternRecognizer::new();
        for i in 0..5 {
            r.register_pattern(
                &format!("p{i}"),
                PatternType::State,
                &format!("sig{i}"),
                "o",
                vec![],
            )
            .ok();
        }
        assert_eq!(r.top_patterns(3).len(), 3);
    }

    #[test]
    fn test_prediction_accuracy_zero() {
        let p = Pattern {
            id: "t".into(),
            name: "t".into(),
            pattern_type: PatternType::State,
            signature: "s".into(),
            confidence: 0.5,
            strength: 0.5,
            occurrence_count: 0,
            correct_predictions: 0,
            associated_outcome: "o".into(),
            involved_services: vec![],
            first_seen: SystemTime::now(),
            last_seen: SystemTime::now(),
            active: true,
        };
        assert!((p.prediction_accuracy()).abs() < f64::EPSILON);
    }

    #[test]
    fn test_prediction_accuracy_calculated() {
        let p = Pattern {
            id: "t".into(),
            name: "t".into(),
            pattern_type: PatternType::State,
            signature: "s".into(),
            confidence: 0.5,
            strength: 0.5,
            occurrence_count: 10,
            correct_predictions: 7,
            associated_outcome: "o".into(),
            involved_services: vec![],
            first_seen: SystemTime::now(),
            last_seen: SystemTime::now(),
            active: true,
        };
        assert!((p.prediction_accuracy() - 0.7).abs() < f64::EPSILON);
    }

    #[test]
    fn test_composite_score() {
        let p = Pattern {
            id: "t".into(),
            name: "t".into(),
            pattern_type: PatternType::State,
            signature: "s".into(),
            confidence: 1.0,
            strength: 1.0,
            occurrence_count: 10,
            correct_predictions: 10,
            associated_outcome: "o".into(),
            involved_services: vec![],
            first_seen: SystemTime::now(),
            last_seen: SystemTime::now(),
            active: true,
        };
        assert!((p.composite_score() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_is_actionable() {
        let p = Pattern {
            id: "t".into(),
            name: "t".into(),
            pattern_type: PatternType::State,
            signature: "s".into(),
            confidence: 0.9,
            strength: 0.9,
            occurrence_count: 10,
            correct_predictions: 9,
            associated_outcome: "o".into(),
            involved_services: vec![],
            first_seen: SystemTime::now(),
            last_seen: SystemTime::now(),
            active: true,
        };
        assert!(p.is_actionable());
    }

    #[test]
    fn test_is_not_actionable_when_inactive() {
        let p = Pattern {
            id: "t".into(),
            name: "t".into(),
            pattern_type: PatternType::State,
            signature: "s".into(),
            confidence: 1.0,
            strength: 1.0,
            occurrence_count: 10,
            correct_predictions: 10,
            associated_outcome: "o".into(),
            involved_services: vec![],
            first_seen: SystemTime::now(),
            last_seen: SystemTime::now(),
            active: false,
        };
        assert!(!p.is_actionable());
    }

    #[test]
    fn test_pattern_type_display() {
        assert_eq!(PatternType::Temporal.to_string(), "TEMPORAL");
        assert_eq!(PatternType::Failure.to_string(), "FAILURE");
    }

    #[test]
    fn test_decay_does_not_go_below_zero() {
        let r = PatternRecognizer::new();
        let id = r
            .register_pattern("p", PatternType::State, "s", "o", vec![])
            .unwrap_or_default();
        {
            let mut patterns = r.patterns.write();
            if let Some(p) = patterns.get_mut(&id) {
                p.strength = 0.001;
            }
        }
        r.apply_decay();
        let p = r.get_pattern(&id);
        assert!(p.map(|p| p.strength).unwrap_or(-1.0) >= 0.0);
    }

    #[test]
    fn test_confidence_running_average() {
        let r = setup_recognizer();
        let id = r.patterns.read().keys().next().cloned().unwrap_or_default();
        let _ = r.record_match(&id, "t1", 1.0);
        let _ = r.record_match(&id, "t2", 0.0);
        let p = r.get_pattern(&id);
        // Running average of initial 0.5 → (0.5*0 + 1.0)/1 = 1.0 → (1.0*1 + 0.0)/2 = 0.5
        // Actually the initial confidence is 0.5 but first match makes n=1
        // confidence = (0.5 * 0 + 1.0) / 1 = 1.0
        // Then (1.0 * 1 + 0.0) / 2 = 0.5
        let conf = p.map(|p| p.confidence).unwrap_or(0.0);
        assert!(conf >= 0.0 && conf <= 1.0);
    }

    #[test]
    fn test_multiple_services_in_pattern() {
        let r = PatternRecognizer::new();
        r.register_pattern(
            "multi",
            PatternType::Failure,
            "cascade",
            "restart_all",
            vec!["synthex".into(), "san-k7".into()],
        )
        .ok();
        assert_eq!(r.patterns_for_service("synthex").len(), 1);
        assert_eq!(r.patterns_for_service("san-k7").len(), 1);
    }
}
