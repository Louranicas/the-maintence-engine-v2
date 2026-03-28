//! # M30: Anti-Pattern Detector
//!
//! Negative reinforcement through anti-pattern detection for the Maintenance Engine.
//!
//! Maintains a registry of known anti-patterns (unsafe code, workflow violations,
//! architectural deviations, consensus bypasses) and records detections when
//! these patterns are observed. Detections can be resolved once addressed, and
//! the detector tracks violation frequencies to surface the most common issues.
//!
//! ## Layer: L5 (Learning)
//! ## Dependencies: M01 (Error)
//! ## Tests: 8+
//!
//! ## 12D Tensor Encoding
//! ```text
//! [30/36, 0.0, 5/6, 0, 0, 0.5, health, uptime, synergy, latency, error_rate, temporal]
//! ```
//!
//! ## Default Anti-Patterns
//!
//! The detector ships with 15 pre-defined anti-patterns across 4 categories:
//!
//! | Category | Count | IDs |
//! |----------|-------|-----|
//! | Code | 4 | AP-C001 through AP-C004 |
//! | Workflow | 4 | AP-W001 through AP-W004 |
//! | Architecture | 4 | AP-A001 through AP-A004 |
//! | Consensus | 3 | AP-X001 through AP-X003 |
//!
//! ## Related Documentation
//! - [Layer Specification](../../ai_docs/layers/L05_LEARNING.md)
//! - [Anti-Pattern Registry](../../.claude/anti_patterns.json)

use std::collections::HashMap;
use std::time::SystemTime;

use parking_lot::RwLock;

use crate::{Error, Result};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Maximum number of detections retained in the detection log.
const DETECTION_LOG_CAPACITY: usize = 500;

// ---------------------------------------------------------------------------
// AntiPatternCategory
// ---------------------------------------------------------------------------

/// Category of anti-pattern for classification and filtering.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AntiPatternCategory {
    /// Code-level anti-patterns (unsafe, unwrap, panic, unbounded channels).
    Code,
    /// Workflow anti-patterns (skipping steps, ignoring guards).
    Workflow,
    /// Architectural anti-patterns (bypassing layers, hardcoding).
    Architecture,
    /// Consensus anti-patterns (ignoring dissent, skipping checks).
    Consensus,
}

impl std::fmt::Display for AntiPatternCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Code => write!(f, "Code"),
            Self::Workflow => write!(f, "Workflow"),
            Self::Architecture => write!(f, "Architecture"),
            Self::Consensus => write!(f, "Consensus"),
        }
    }
}

// ---------------------------------------------------------------------------
// AntiPattern
// ---------------------------------------------------------------------------

/// Definition of a known anti-pattern.
///
/// Each anti-pattern has a unique identifier (e.g. `"AP-C001"`), a human-readable
/// name, a description, a severity score, and a detection rule string that
/// downstream processors can use for matching.
#[derive(Clone, Debug)]
pub struct AntiPattern {
    /// Unique anti-pattern identifier (e.g. `"AP-C001"`).
    pub id: String,
    /// Classification category.
    pub category: AntiPatternCategory,
    /// Short human-readable name.
    pub name: String,
    /// Detailed description of why this is an anti-pattern.
    pub description: String,
    /// Severity score from 0.0 (informational) to 1.0 (critical).
    pub severity: f64,
    /// Detection rule string (used by downstream pattern matchers).
    pub detection_rule: String,
}

// ---------------------------------------------------------------------------
// Detection
// ---------------------------------------------------------------------------

/// A recorded instance of an anti-pattern being detected.
///
/// Detections are immutable once created but can be marked as resolved
/// via [`AntiPatternDetector::resolve`].
#[derive(Clone, Debug)]
pub struct Detection {
    /// Unique detection identifier (UUID v4 format).
    pub id: String,
    /// ID of the anti-pattern that was detected.
    pub pattern_id: String,
    /// Contextual description of where/how the pattern was detected.
    pub context: String,
    /// Severity inherited from the anti-pattern definition.
    pub severity: f64,
    /// Timestamp when the detection occurred.
    pub timestamp: SystemTime,
    /// Whether this detection has been resolved.
    pub resolved: bool,
}

// ---------------------------------------------------------------------------
// AntiPatternDetector
// ---------------------------------------------------------------------------

/// Thread-safe anti-pattern detection engine.
///
/// Maintains a registry of known anti-patterns, a bounded log of detections,
/// and per-pattern violation counts. All internal state is protected by
/// `parking_lot::RwLock` for safe concurrent access.
///
/// # Construction
///
/// ```rust
/// use maintenance_engine::m5_learning::antipattern::AntiPatternDetector;
///
/// let detector = AntiPatternDetector::new();
/// assert_eq!(detector.pattern_count(), 15); // 15 default anti-patterns
/// ```
pub struct AntiPatternDetector {
    /// Registry of known anti-patterns.
    patterns: RwLock<Vec<AntiPattern>>,
    /// Bounded detection log (most recent detections).
    detections: RwLock<Vec<Detection>>,
    /// Per-pattern violation counts keyed by pattern ID.
    violation_counts: RwLock<HashMap<String, u32>>,
}

impl AntiPatternDetector {
    // -------------------------------------------------------------------
    // Construction
    // -------------------------------------------------------------------

    /// Create a new `AntiPatternDetector` pre-loaded with 15 default anti-patterns.
    ///
    /// The defaults cover Code (AP-C001 through AP-C004), Workflow (AP-W001
    /// through AP-W004), Architecture (AP-A001 through AP-A004), and Consensus
    /// (AP-X001 through AP-X003) categories.
    #[must_use]
    pub fn new() -> Self {
        let patterns = Self::default_patterns();
        let violation_counts = HashMap::new();

        Self {
            patterns: RwLock::new(patterns),
            detections: RwLock::new(Vec::new()),
            violation_counts: RwLock::new(violation_counts),
        }
    }

    /// Build the 15 default anti-pattern definitions.
    fn default_patterns() -> Vec<AntiPattern> {
        let mut patterns = Self::default_code_patterns();
        patterns.extend(Self::default_workflow_patterns());
        patterns.extend(Self::default_architecture_patterns());
        patterns.extend(Self::default_consensus_patterns());
        patterns
    }

    /// Code anti-patterns (AP-C001 through AP-C004).
    fn default_code_patterns() -> Vec<AntiPattern> {
        vec![
            AntiPattern {
                id: "AP-C001".to_string(),
                category: AntiPatternCategory::Code,
                name: "unwrap_in_production".to_string(),
                description: "Using .unwrap() in production code can cause panics. \
                    Use Result propagation or provide fallback values instead."
                    .to_string(),
                severity: 1.0,
                detection_rule: "match .unwrap() calls outside #[cfg(test)]".to_string(),
            },
            AntiPattern {
                id: "AP-C002".to_string(),
                category: AntiPatternCategory::Code,
                name: "unsafe_block".to_string(),
                description: "Using unsafe blocks bypasses Rust's safety guarantees. \
                    The Maintenance Engine forbids all unsafe code."
                    .to_string(),
                severity: 1.0,
                detection_rule: "match unsafe { } blocks in source".to_string(),
            },
            AntiPattern {
                id: "AP-C003".to_string(),
                category: AntiPatternCategory::Code,
                name: "panic_in_handler".to_string(),
                description: "Using panic!() or similar macros in request handlers \
                    causes abrupt termination instead of graceful error handling."
                    .to_string(),
                severity: 0.9,
                detection_rule: "match panic!(), todo!(), unimplemented!() in handlers"
                    .to_string(),
            },
            AntiPattern {
                id: "AP-C004".to_string(),
                category: AntiPatternCategory::Code,
                name: "unbounded_channel".to_string(),
                description: "Unbounded channels can cause unbounded memory growth \
                    under load. Always use bounded channels with backpressure."
                    .to_string(),
                severity: 0.7,
                detection_rule: "match unbounded() channel constructors".to_string(),
            },
        ]
    }

    /// Workflow anti-patterns (AP-W001 through AP-W004).
    fn default_workflow_patterns() -> Vec<AntiPattern> {
        vec![
            AntiPattern {
                id: "AP-W001".to_string(),
                category: AntiPatternCategory::Workflow,
                name: "edit_without_read".to_string(),
                description: "Editing a file without first reading it risks overwriting \
                    content or making changes based on stale assumptions."
                    .to_string(),
                severity: 1.0,
                detection_rule: "detect Write/Edit without preceding Read on same path"
                    .to_string(),
            },
            AntiPattern {
                id: "AP-W002".to_string(),
                category: AntiPatternCategory::Workflow,
                name: "skip_health_check".to_string(),
                description: "Deploying or restarting a service without verifying health \
                    afterwards can leave broken services in production."
                    .to_string(),
                severity: 0.8,
                detection_rule: "detect restart/deploy without subsequent health check"
                    .to_string(),
            },
            AntiPattern {
                id: "AP-W003".to_string(),
                category: AntiPatternCategory::Workflow,
                name: "ignore_circuit_breaker".to_string(),
                description: "Ignoring circuit breaker state and continuing to send \
                    requests to a failing service amplifies cascading failures."
                    .to_string(),
                severity: 0.9,
                detection_rule: "detect requests sent while circuit is Open".to_string(),
            },
            AntiPattern {
                id: "AP-W004".to_string(),
                category: AntiPatternCategory::Workflow,
                name: "skip_confidence_check".to_string(),
                description: "Executing remediation actions without checking confidence \
                    scores can lead to incorrect or harmful interventions."
                    .to_string(),
                severity: 0.8,
                detection_rule: "detect action execution without confidence >= threshold"
                    .to_string(),
            },
        ]
    }

    /// Architecture anti-patterns (AP-A001 through AP-A004).
    fn default_architecture_patterns() -> Vec<AntiPattern> {
        vec![
            AntiPattern {
                id: "AP-A001".to_string(),
                category: AntiPatternCategory::Architecture,
                name: "bypass_consensus".to_string(),
                description: "Bypassing the PBFT consensus mechanism for critical \
                    actions undermines multi-agent safety guarantees."
                    .to_string(),
                severity: 1.0,
                detection_rule: "detect L3 actions without PBFT round".to_string(),
            },
            AntiPattern {
                id: "AP-A002".to_string(),
                category: AntiPatternCategory::Architecture,
                name: "direct_database_write".to_string(),
                description: "Writing directly to databases without going through \
                    the persistence layer bypasses validation and auditing."
                    .to_string(),
                severity: 0.7,
                detection_rule: "detect raw SQL INSERT/UPDATE outside persistence module"
                    .to_string(),
            },
            AntiPattern {
                id: "AP-A003".to_string(),
                category: AntiPatternCategory::Architecture,
                name: "hardcoded_thresholds".to_string(),
                description: "Hardcoding threshold values instead of using configuration \
                    prevents adaptive tuning and environment-specific overrides."
                    .to_string(),
                severity: 0.6,
                detection_rule: "detect magic numbers in comparison expressions".to_string(),
            },
            AntiPattern {
                id: "AP-A004".to_string(),
                category: AntiPatternCategory::Architecture,
                name: "missing_tensor_update".to_string(),
                description: "Failing to update the 12D tensor after state changes \
                    causes stale representations and incorrect routing decisions."
                    .to_string(),
                severity: 0.5,
                detection_rule: "detect state mutation without tensor recalculation"
                    .to_string(),
            },
        ]
    }

    /// Consensus anti-patterns (AP-X001 through AP-X003).
    fn default_consensus_patterns() -> Vec<AntiPattern> {
        vec![
            AntiPattern {
                id: "AP-X001".to_string(),
                category: AntiPatternCategory::Consensus,
                name: "skip_minority_check".to_string(),
                description: "Proceeding with consensus without recording minority \
                    opinions violates NAM R3 (DissentCapture) requirements."
                    .to_string(),
                severity: 0.9,
                detection_rule: "detect consensus completion without dissent log entry"
                    .to_string(),
            },
            AntiPattern {
                id: "AP-X002".to_string(),
                category: AntiPatternCategory::Consensus,
                name: "ignore_dissent".to_string(),
                description: "Ignoring dissenting votes without analysis prevents \
                    the system from learning from alternative perspectives."
                    .to_string(),
                severity: 0.8,
                detection_rule: "detect dissent votes discarded without recording"
                    .to_string(),
            },
            AntiPattern {
                id: "AP-X003".to_string(),
                category: AntiPatternCategory::Consensus,
                name: "human_as_supervisor".to_string(),
                description: "Treating the human as a supervisor rather than a peer \
                    agent (@0.A) violates NAM R5 (HumanAsAgent) principles."
                    .to_string(),
                severity: 0.7,
                detection_rule: "detect elevated human authority outside @0.A role"
                    .to_string(),
            },
        ]
    }

    // -------------------------------------------------------------------
    // Pattern Registration
    // -------------------------------------------------------------------

    /// Register a new anti-pattern definition.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] if a pattern with the same `id` already exists,
    /// or if the severity is outside the [0.0, 1.0] range.
    pub fn register_pattern(
        &self,
        id: impl Into<String>,
        category: AntiPatternCategory,
        name: impl Into<String>,
        description: impl Into<String>,
        severity: f64,
    ) -> Result<()> {
        if !(0.0..=1.0).contains(&severity) {
            return Err(Error::Validation(format!(
                "Anti-pattern severity must be between 0.0 and 1.0, got {severity}"
            )));
        }

        let id = id.into();
        let mut guard = self.patterns.write();

        if guard.iter().any(|p| p.id == id) {
            return Err(Error::Validation(format!(
                "Anti-pattern '{id}' already exists"
            )));
        }

        guard.push(AntiPattern {
            id,
            category,
            name: name.into(),
            description: description.into(),
            severity,
            detection_rule: String::new(),
        });
        drop(guard);

        Ok(())
    }

    // -------------------------------------------------------------------
    // Detection
    // -------------------------------------------------------------------

    /// Record a detection of an anti-pattern.
    ///
    /// Creates a new [`Detection`] record with a generated UUID, inheriting
    /// the severity from the anti-pattern definition. The detection is appended
    /// to the bounded detection log and the violation count is incremented.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] if the `pattern_id` is not registered.
    pub fn detect(
        &self,
        pattern_id: impl Into<String>,
        context: impl Into<String>,
    ) -> Result<Detection> {
        let pattern_id = pattern_id.into();
        let context = context.into();

        // Look up the pattern to get severity
        let severity = {
            let guard = self.patterns.read();
            guard
                .iter()
                .find(|p| p.id == pattern_id)
                .map(|p| p.severity)
                .ok_or_else(|| {
                    Error::Validation(format!(
                        "Anti-pattern '{pattern_id}' is not registered"
                    ))
                })?
        };

        let detection = Detection {
            id: uuid::Uuid::new_v4().to_string(),
            pattern_id: pattern_id.clone(),
            context,
            severity,
            timestamp: SystemTime::now(),
            resolved: false,
        };

        // Append to detection log (bounded)
        {
            let mut log = self.detections.write();
            if log.len() >= DETECTION_LOG_CAPACITY {
                log.remove(0);
            }
            log.push(detection.clone());
        }

        // Increment violation count
        {
            let mut counts = self.violation_counts.write();
            *counts.entry(pattern_id).or_insert(0) += 1;
        }

        Ok(detection)
    }

    /// Mark a detection as resolved.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] if no detection with the given `detection_id` exists.
    pub fn resolve(&self, detection_id: &str) -> Result<()> {
        let mut guard = self.detections.write();
        let detection = guard
            .iter_mut()
            .find(|d| d.id == detection_id)
            .ok_or_else(|| {
                Error::Validation(format!(
                    "Detection '{detection_id}' not found"
                ))
            })?;
        detection.resolved = true;
        drop(guard);
        Ok(())
    }

    // -------------------------------------------------------------------
    // Queries
    // -------------------------------------------------------------------

    /// Get the violation count for a specific anti-pattern.
    #[must_use]
    pub fn get_violations(&self, pattern_id: &str) -> u32 {
        self.violation_counts
            .read()
            .get(pattern_id)
            .copied()
            .unwrap_or(0)
    }

    /// Get all unresolved detections, ordered by timestamp (newest first).
    #[must_use]
    pub fn get_unresolved(&self) -> Vec<Detection> {
        let guard = self.detections.read();
        let mut unresolved: Vec<Detection> = guard
            .iter()
            .filter(|d| !d.resolved)
            .cloned()
            .collect();
        drop(guard);
        unresolved.reverse();
        unresolved
    }

    /// Get all detections matching a specific anti-pattern category.
    #[must_use]
    pub fn get_detections_by_category(&self, category: AntiPatternCategory) -> Vec<Detection> {
        let patterns_guard = self.patterns.read();
        let category_ids: Vec<String> = patterns_guard
            .iter()
            .filter(|p| p.category == category)
            .map(|p| p.id.clone())
            .collect();
        drop(patterns_guard);

        let det_guard = self.detections.read();
        det_guard
            .iter()
            .filter(|d| category_ids.contains(&d.pattern_id))
            .cloned()
            .collect()
    }

    /// Get the total number of violation events recorded across all patterns.
    #[must_use]
    pub fn violation_count(&self) -> usize {
        let guard = self.violation_counts.read();
        #[allow(clippy::cast_possible_truncation)]
        let total: u32 = guard.values().sum();
        drop(guard);
        total as usize
    }

    /// Get the total number of registered anti-patterns.
    #[must_use]
    pub fn pattern_count(&self) -> usize {
        self.patterns.read().len()
    }

    /// Get the `n` most frequently violated patterns, sorted by descending count.
    ///
    /// Returns a vector of `(pattern_id, violation_count)` tuples.
    #[must_use]
    pub fn most_frequent_violations(&self, n: usize) -> Vec<(String, u32)> {
        let guard = self.violation_counts.read();
        let mut entries: Vec<(String, u32)> = guard
            .iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect();
        drop(guard);
        entries.sort_by(|a, b| b.1.cmp(&a.1));
        entries.truncate(n);
        entries
    }
}

impl Default for AntiPatternDetector {
    fn default() -> Self {
        Self::new()
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_loads_defaults() {
        let detector = AntiPatternDetector::new();
        assert_eq!(detector.pattern_count(), 15, "Should load 15 default anti-patterns");
    }

    #[test]
    fn test_register_pattern() {
        let detector = AntiPatternDetector::new();

        let result = detector.register_pattern(
            "AP-CUSTOM-001",
            AntiPatternCategory::Code,
            "custom_pattern",
            "A custom test pattern",
            0.5,
        );
        assert!(result.is_ok());
        assert_eq!(detector.pattern_count(), 16);

        // Duplicate should fail
        let dup = detector.register_pattern(
            "AP-CUSTOM-001",
            AntiPatternCategory::Code,
            "dup",
            "dup",
            0.5,
        );
        assert!(dup.is_err());

        // Invalid severity should fail
        let bad_sev = detector.register_pattern(
            "AP-BAD",
            AntiPatternCategory::Code,
            "bad",
            "bad",
            1.5,
        );
        assert!(bad_sev.is_err());
    }

    #[test]
    fn test_detect_pattern() {
        let detector = AntiPatternDetector::new();

        let result = detector.detect("AP-C001", "Found .unwrap() in handler.rs:42");
        assert!(result.is_ok());

        if let Ok(detection) = result {
            assert_eq!(detection.pattern_id, "AP-C001");
            assert!((detection.severity - 1.0).abs() < f64::EPSILON);
            assert!(!detection.resolved);
            assert!(detection.context.contains("unwrap"));
        }

        // Unknown pattern should fail
        let unknown = detector.detect("AP-NONEXISTENT", "test");
        assert!(unknown.is_err());
    }

    #[test]
    fn test_resolve_detection() {
        let detector = AntiPatternDetector::new();

        let detection = detector.detect("AP-C002", "unsafe block in module.rs").ok();
        assert!(detection.is_some());

        if let Some(d) = detection {
            assert!(detector.resolve(&d.id).is_ok());

            // After resolving, it should not appear in unresolved
            let unresolved = detector.get_unresolved();
            assert!(
                !unresolved.iter().any(|u| u.id == d.id),
                "Resolved detection should not appear in unresolved list"
            );
        }

        // Resolving nonexistent detection should fail
        assert!(detector.resolve("nonexistent-id").is_err());
    }

    #[test]
    fn test_violation_counts() {
        let detector = AntiPatternDetector::new();

        let _ = detector.detect("AP-C001", "violation 1");
        let _ = detector.detect("AP-C001", "violation 2");
        let _ = detector.detect("AP-C001", "violation 3");
        let _ = detector.detect("AP-W001", "workflow violation");

        assert_eq!(detector.get_violations("AP-C001"), 3);
        assert_eq!(detector.get_violations("AP-W001"), 1);
        assert_eq!(detector.get_violations("AP-NONEXISTENT"), 0);
        assert_eq!(detector.violation_count(), 4);
    }

    #[test]
    fn test_unresolved() {
        let detector = AntiPatternDetector::new();

        let d1 = detector.detect("AP-C001", "first").ok();
        let _ = detector.detect("AP-C002", "second");
        let _ = detector.detect("AP-C003", "third");

        assert_eq!(detector.get_unresolved().len(), 3);

        // Resolve one
        if let Some(d) = d1 {
            let _ = detector.resolve(&d.id);
        }
        assert_eq!(detector.get_unresolved().len(), 2);
    }

    #[test]
    fn test_detections_by_category() {
        let detector = AntiPatternDetector::new();

        let _ = detector.detect("AP-C001", "code issue 1");
        let _ = detector.detect("AP-C002", "code issue 2");
        let _ = detector.detect("AP-W001", "workflow issue");
        let _ = detector.detect("AP-A001", "arch issue");
        let _ = detector.detect("AP-X001", "consensus issue");

        let code_dets = detector.get_detections_by_category(AntiPatternCategory::Code);
        assert_eq!(code_dets.len(), 2, "Should find 2 Code detections");

        let workflow_dets = detector.get_detections_by_category(AntiPatternCategory::Workflow);
        assert_eq!(workflow_dets.len(), 1, "Should find 1 Workflow detection");

        let arch_dets = detector.get_detections_by_category(AntiPatternCategory::Architecture);
        assert_eq!(arch_dets.len(), 1, "Should find 1 Architecture detection");

        let consensus_dets = detector.get_detections_by_category(AntiPatternCategory::Consensus);
        assert_eq!(consensus_dets.len(), 1, "Should find 1 Consensus detection");
    }

    #[test]
    fn test_most_frequent() {
        let detector = AntiPatternDetector::new();

        // Create varying violation counts
        for _ in 0..5 {
            let _ = detector.detect("AP-C001", "unwrap");
        }
        for _ in 0..3 {
            let _ = detector.detect("AP-W001", "edit without read");
        }
        let _ = detector.detect("AP-A001", "bypass");

        let top = detector.most_frequent_violations(2);
        assert_eq!(top.len(), 2);
        assert_eq!(top[0].0, "AP-C001");
        assert_eq!(top[0].1, 5);
        assert_eq!(top[1].0, "AP-W001");
        assert_eq!(top[1].1, 3);
    }

    #[test]
    fn test_default_pattern_severities() {
        let detector = AntiPatternDetector::new();
        let patterns = detector.patterns.read();

        // Verify a few key severities
        let c001 = patterns.iter().find(|p| p.id == "AP-C001");
        assert!(c001.is_some());
        if let Some(p) = c001 {
            assert!((p.severity - 1.0).abs() < f64::EPSILON, "AP-C001 severity should be 1.0");
            assert_eq!(p.name, "unwrap_in_production");
        }

        let a003 = patterns.iter().find(|p| p.id == "AP-A003");
        assert!(a003.is_some());
        if let Some(p) = a003 {
            assert!((p.severity - 0.6).abs() < f64::EPSILON, "AP-A003 severity should be 0.6");
            assert_eq!(p.name, "hardcoded_thresholds");
        }

        let x003 = patterns.iter().find(|p| p.id == "AP-X003");
        assert!(x003.is_some());
        if let Some(p) = x003 {
            assert!((p.severity - 0.7).abs() < f64::EPSILON, "AP-X003 severity should be 0.7");
            assert_eq!(p.name, "human_as_supervisor");
        }
    }

    #[test]
    fn test_default_pattern_categories() {
        let detector = AntiPatternDetector::new();
        let patterns = detector.patterns.read();

        let code_count = patterns.iter().filter(|p| p.category == AntiPatternCategory::Code).count();
        let workflow_count = patterns.iter().filter(|p| p.category == AntiPatternCategory::Workflow).count();
        let arch_count = patterns.iter().filter(|p| p.category == AntiPatternCategory::Architecture).count();
        let consensus_count = patterns.iter().filter(|p| p.category == AntiPatternCategory::Consensus).count();

        assert_eq!(code_count, 4, "Should have 4 Code anti-patterns");
        assert_eq!(workflow_count, 4, "Should have 4 Workflow anti-patterns");
        assert_eq!(arch_count, 4, "Should have 4 Architecture anti-patterns");
        assert_eq!(consensus_count, 3, "Should have 3 Consensus anti-patterns");
    }

    #[test]
    fn test_detection_log_capacity() {
        let detector = AntiPatternDetector::new();

        // Fill beyond capacity
        for i in 0..550 {
            let _ = detector.detect("AP-C001", format!("violation {i}"));
        }

        let unresolved = detector.get_unresolved();
        assert!(
            unresolved.len() <= DETECTION_LOG_CAPACITY,
            "Detection log should be bounded at capacity"
        );
    }
}
