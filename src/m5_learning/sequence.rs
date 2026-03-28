//! # M55: Sequence Detector
//!
//! Detects ordered event sequences across services, tracking temporal patterns
//! that precede failures, trigger recoveries, or signal maintenance windows.
//! Ingests [`ServiceEvent`]s into a sliding buffer, matches them against
//! registered [`SequencePattern`]s, and emits [`DetectedSequence`]s when a
//! complete ordered match is found within the configured time window.
//!
//! ## Layer: L5 (Learning)
//! ## Dependencies: M01 (Error), M00 (Timestamp)
//!
//! ## Confidence Formula
//!
//! ```text
//! confidence = occurrence_count / (occurrence_count + 5.0)
//! ```
//!
//! Bayesian smoothed — a new pattern with 0 occurrences has confidence 0.0,
//! 5 occurrences yields 0.5, and 20 occurrences yields ~0.8.
//!
//! ## Partial Matching
//!
//! On each ingested event the detector checks every active pattern:
//! - If the event type matches the **first** element, a new partial match starts.
//! - If it matches the **next** expected element of an existing partial, the
//!   match extends.
//! - A partial match that exceeds `max_window_ms` ticks is discarded.
//! - A fully matched sequence emits a [`DetectedSequence`] and updates the
//!   pattern's occurrence statistics.
//!
//! ## Related Documentation
//! - [Layer Specification](../../ai_docs/layers/L05_LEARNING.md)

use std::collections::{HashMap, VecDeque};
use std::fmt;

use parking_lot::RwLock;
use uuid::Uuid;

use crate::m1_foundation::shared_types::Timestamp;
use crate::{Error, Result};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Default event buffer capacity.
const DEFAULT_EVENT_BUFFER_SIZE: usize = 10_000;

/// Default maximum number of registered patterns.
const DEFAULT_MAX_PATTERNS: usize = 300;

/// Default partial-match timeout in ticks (treated as milliseconds).
const DEFAULT_PARTIAL_MATCH_TIMEOUT_MS: u64 = 60_000;

/// Default minimum confidence for a pattern to be considered valid.
const DEFAULT_MIN_CONFIDENCE: f64 = 0.55;

/// Default decay rate subtracted from confidence per `apply_decay` call.
const DEFAULT_DECAY_RATE: f64 = 0.003;

/// Maximum number of detections retained.
const MAX_DETECTIONS: usize = 500;

/// Bayesian smoothing constant for confidence calculation.
const BAYESIAN_SMOOTHING: f64 = 5.0;

/// Minimum number of event types in a sequence pattern.
const MIN_EVENT_TYPES: usize = 2;

/// Maximum number of event types in a sequence pattern.
const MAX_EVENT_TYPES: usize = 5;

/// Confidence threshold below which a pattern is deactivated.
const DEACTIVATION_THRESHOLD: f64 = 0.1;

// ---------------------------------------------------------------------------
// SequenceOutcome
// ---------------------------------------------------------------------------

/// Classification of a detected sequence's operational meaning.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SequenceOutcome {
    /// The sequence precedes a service failure.
    FailurePrecursor,
    /// The sequence signals an ongoing recovery.
    RecoverySignal,
    /// The sequence indicates a maintenance window should be opened.
    MaintenanceTrigger,
    /// Outcome not yet determined.
    Unknown,
}

impl fmt::Display for SequenceOutcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FailurePrecursor => write!(f, "FAILURE_PRECURSOR"),
            Self::RecoverySignal => write!(f, "RECOVERY_SIGNAL"),
            Self::MaintenanceTrigger => write!(f, "MAINTENANCE_TRIGGER"),
            Self::Unknown => write!(f, "UNKNOWN"),
        }
    }
}

// ---------------------------------------------------------------------------
// ServiceEvent
// ---------------------------------------------------------------------------

/// A raw service event ingested by the detector.
#[derive(Clone, Debug)]
pub struct ServiceEvent {
    /// Unique event identifier.
    pub event_id: String,
    /// Originating service.
    pub service_id: String,
    /// Categorised event type string (e.g. `"cpu_spike"`, `"restart"`).
    pub event_type: String,
    /// Severity label (e.g. `"HIGH"`, `"CRITICAL"`).
    pub severity: String,
    /// Monotonic timestamp of the event.
    pub timestamp: Timestamp,
}

// ---------------------------------------------------------------------------
// SequencePattern
// ---------------------------------------------------------------------------

/// A registered ordered sequence of event types to watch for.
#[derive(Clone, Debug)]
pub struct SequencePattern {
    /// Unique pattern identifier (UUID).
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// Ordered list of event types that comprise the sequence (2-5 elements).
    pub event_types: Vec<String>,
    /// Maximum tick span from first to last event for a valid match.
    pub max_window_ms: u64,
    /// Minimum occurrences before the pattern is considered meaningful.
    pub min_occurrences: u32,
    /// Number of times this sequence has been fully matched.
    pub occurrence_count: u64,
    /// Mean interval (in ticks) between consecutive events in matched sequences.
    pub mean_interval_ms: f64,
    /// Standard deviation of inter-event intervals.
    pub stddev_interval_ms: f64,
    /// Bayesian-smoothed confidence score.
    pub confidence: f64,
    /// Pattern strength (decays over time).
    pub strength: f64,
    /// Operational outcome associated with this sequence.
    pub associated_outcome: SequenceOutcome,
    /// When the pattern was first registered.
    pub first_seen: Timestamp,
    /// When the pattern last matched.
    pub last_seen: Timestamp,
    /// Whether the pattern is actively being matched.
    pub active: bool,
}

// ---------------------------------------------------------------------------
// DetectedSequence
// ---------------------------------------------------------------------------

/// A fully matched sequence instance.
#[derive(Clone, Debug)]
pub struct DetectedSequence {
    /// Unique detection identifier (UUID).
    pub id: String,
    /// ID of the pattern that matched.
    pub pattern_id: String,
    /// Event IDs that participated in the match.
    pub matched_events: Vec<String>,
    /// Distinct services involved across the matched events.
    pub services_involved: Vec<String>,
    /// Confidence of the parent pattern at detection time.
    pub confidence: f64,
    /// Timestamp of the first matched event.
    pub first_event_at: Timestamp,
    /// Timestamp of the last matched event.
    pub last_event_at: Timestamp,
    /// Tick span from first to last event.
    pub span_ms: u64,
    /// Outcome inherited from the parent pattern.
    pub outcome: SequenceOutcome,
}

// ---------------------------------------------------------------------------
// SequenceStatistics
// ---------------------------------------------------------------------------

/// Aggregate statistics for the sequence detector.
#[derive(Clone, Debug)]
pub struct SequenceStatistics {
    /// Total registered patterns (active + inactive).
    pub total_patterns: usize,
    /// Currently active patterns.
    pub active_patterns: usize,
    /// Total number of detections emitted.
    pub total_detections: usize,
    /// Average confidence across all active patterns.
    pub avg_confidence: f64,
}

// ---------------------------------------------------------------------------
// SequenceDetectorConfig
// ---------------------------------------------------------------------------

/// Configuration for the [`SequenceDetectorCore`].
#[derive(Clone, Debug)]
pub struct SequenceDetectorConfig {
    /// Maximum number of events retained in the sliding buffer.
    pub event_buffer_size: usize,
    /// Maximum number of registered patterns.
    pub max_patterns: usize,
    /// Ticks before an incomplete partial match expires.
    pub partial_match_timeout_ms: u64,
    /// Minimum confidence for a pattern to be considered valid.
    pub min_confidence: f64,
    /// Amount subtracted from confidence each decay cycle.
    pub decay_rate: f64,
}

impl Default for SequenceDetectorConfig {
    fn default() -> Self {
        Self {
            event_buffer_size: DEFAULT_EVENT_BUFFER_SIZE,
            max_patterns: DEFAULT_MAX_PATTERNS,
            partial_match_timeout_ms: DEFAULT_PARTIAL_MATCH_TIMEOUT_MS,
            min_confidence: DEFAULT_MIN_CONFIDENCE,
            decay_rate: DEFAULT_DECAY_RATE,
        }
    }
}

// ---------------------------------------------------------------------------
// PartialMatch (internal)
// ---------------------------------------------------------------------------

/// Tracks an in-progress match against a pattern.
#[derive(Clone, Debug)]
struct PartialMatch {
    /// Pattern being matched.
    pattern_id: String,
    /// Event IDs matched so far.
    matched_event_ids: Vec<String>,
    /// Services seen so far.
    services: Vec<String>,
    /// Index of the next expected event type in the pattern.
    next_index: usize,
    /// Timestamp of the first matched event.
    first_event_at: Timestamp,
    /// Timestamp of the most recently matched event.
    last_event_at: Timestamp,
}

// ---------------------------------------------------------------------------
// SequenceDetector trait
// ---------------------------------------------------------------------------

/// Core interface for sequence detection.
pub trait SequenceDetector: Send + Sync + fmt::Debug {
    /// Ingest a raw service event into the detector.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if the event has empty required fields.
    fn ingest_event(&self, event: ServiceEvent) -> Result<()>;

    /// Return all detections emitted so far (most recent last).
    ///
    /// # Errors
    ///
    /// Returns an error if internal state cannot be read.
    fn detect_sequences(&self) -> Result<Vec<DetectedSequence>>;

    /// Register a new sequence pattern.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if `event_types` length is outside 2..=5,
    /// or if required fields are empty.
    fn register_sequence(&self, pattern: SequencePattern) -> Result<String>;

    /// Find all detections whose parent pattern contains `event_type`.
    fn find_matching_sequences(&self, event_type: &str) -> Vec<DetectedSequence>;

    /// Total number of registered patterns (active + inactive).
    fn sequence_count(&self) -> usize;

    /// Number of currently active patterns.
    fn active_sequence_count(&self) -> usize;

    /// Return the top `n` patterns ordered by confidence descending.
    fn top_sequences(&self, n: usize) -> Vec<SequencePattern>;

    /// Mark a sequence pattern as beneficial or detrimental.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if the sequence ID is not found.
    fn mark_sequence_outcome(&self, sequence_id: &str, beneficial: bool) -> Result<()>;

    /// Apply decay to all pattern confidences; deactivate those below threshold.
    fn apply_decay(&self);

    /// Compute aggregate statistics.
    fn statistics(&self) -> SequenceStatistics;
}

// ---------------------------------------------------------------------------
// SequenceDetectorCore
// ---------------------------------------------------------------------------

/// Primary implementation of the [`SequenceDetector`] trait.
///
/// Uses `parking_lot::RwLock` for interior mutability so all trait methods
/// can take `&self` (C2 constraint).
#[derive(Debug)]
pub struct SequenceDetectorCore {
    /// Registered patterns keyed by ID.
    patterns: RwLock<HashMap<String, SequencePattern>>,
    /// Completed detections (capped at [`MAX_DETECTIONS`]).
    detections: RwLock<Vec<DetectedSequence>>,
    /// Sliding event buffer.
    event_buffer: RwLock<VecDeque<ServiceEvent>>,
    /// In-progress partial matches.
    partial_matches: RwLock<Vec<PartialMatch>>,
    /// Configuration.
    config: SequenceDetectorConfig,
}

impl Default for SequenceDetectorCore {
    fn default() -> Self {
        Self::new(SequenceDetectorConfig::default())
    }
}

impl SequenceDetectorCore {
    /// Create a new detector with the given configuration.
    #[must_use]
    pub fn new(config: SequenceDetectorConfig) -> Self {
        Self {
            patterns: RwLock::new(HashMap::new()),
            detections: RwLock::new(Vec::new()),
            event_buffer: RwLock::new(VecDeque::with_capacity(config.event_buffer_size)),
            partial_matches: RwLock::new(Vec::new()),
            config,
        }
    }

    /// Validate a `ServiceEvent` has non-empty required fields.
    fn validate_event(event: &ServiceEvent) -> Result<()> {
        if event.event_id.is_empty() {
            return Err(Error::Validation("event_id cannot be empty".into()));
        }
        if event.service_id.is_empty() {
            return Err(Error::Validation("service_id cannot be empty".into()));
        }
        if event.event_type.is_empty() {
            return Err(Error::Validation("event_type cannot be empty".into()));
        }
        if event.severity.is_empty() {
            return Err(Error::Validation("severity cannot be empty".into()));
        }
        Ok(())
    }

    /// Validate a `SequencePattern` meets structural requirements.
    fn validate_pattern(pattern: &SequencePattern) -> Result<()> {
        if pattern.name.is_empty() {
            return Err(Error::Validation(
                "sequence pattern name cannot be empty".into(),
            ));
        }
        let len = pattern.event_types.len();
        if !(MIN_EVENT_TYPES..=MAX_EVENT_TYPES).contains(&len) {
            return Err(Error::Validation(format!(
                "event_types length must be {MIN_EVENT_TYPES}-{MAX_EVENT_TYPES}, got {len}"
            )));
        }
        for (i, et) in pattern.event_types.iter().enumerate() {
            if et.is_empty() {
                return Err(Error::Validation(format!(
                    "event_types[{i}] cannot be empty"
                )));
            }
        }
        Ok(())
    }

    /// Calculate Bayesian-smoothed confidence from occurrence count.
    #[allow(clippy::cast_precision_loss)]
    fn bayesian_confidence(occurrence_count: u64) -> f64 {
        let count = occurrence_count as f64;
        count / (count + BAYESIAN_SMOOTHING)
    }

    /// Advance existing partial matches and collect completions.
    fn advance_partials(
        &self,
        event: &ServiceEvent,
        patterns: &HashMap<String, SequencePattern>,
        partials: &mut Vec<PartialMatch>,
        detections: &mut Vec<DetectedSequence>,
    ) -> Vec<(String, u64, Timestamp)> {
        let mut completed: Vec<(String, u64, Timestamp)> = Vec::new();
        let mut i = 0;
        while i < partials.len() {
            let elapsed = event.timestamp.elapsed_since(partials[i].first_event_at);
            if elapsed > self.config.partial_match_timeout_ms {
                partials.swap_remove(i);
                continue;
            }

            let Some(pattern) = patterns.get(&partials[i].pattern_id) else {
                partials.swap_remove(i);
                continue;
            };

            if !pattern.active {
                partials.swap_remove(i);
                continue;
            }

            let idx = partials[i].next_index;
            let matches_next = idx < pattern.event_types.len()
                && pattern.event_types[idx] == event.event_type;
            let within_window =
                event.timestamp.elapsed_since(partials[i].first_event_at) <= pattern.max_window_ms;

            if matches_next && within_window {
                partials[i].matched_event_ids.push(event.event_id.clone());
                if !partials[i].services.contains(&event.service_id) {
                    partials[i].services.push(event.service_id.clone());
                }
                partials[i].next_index += 1;
                partials[i].last_event_at = event.timestamp;

                if partials[i].next_index == pattern.event_types.len() {
                    let pm = partials.swap_remove(i);
                    let span_ms = pm.last_event_at.elapsed_since(pm.first_event_at);
                    Self::emit_detection(&pm, pattern, span_ms, detections);
                    completed.push((pm.pattern_id, span_ms, event.timestamp));
                    continue;
                }
            } else if matches_next {
                partials.swap_remove(i);
                continue;
            }
            i += 1;
        }
        completed
    }

    /// Emit a completed detection into the detections buffer.
    fn emit_detection(
        pm: &PartialMatch,
        pattern: &SequencePattern,
        span_ms: u64,
        detections: &mut Vec<DetectedSequence>,
    ) {
        let detection = DetectedSequence {
            id: Uuid::new_v4().to_string(),
            pattern_id: pm.pattern_id.clone(),
            matched_events: pm.matched_event_ids.clone(),
            services_involved: pm.services.clone(),
            confidence: pattern.confidence,
            first_event_at: pm.first_event_at,
            last_event_at: pm.last_event_at,
            span_ms,
            outcome: pattern.associated_outcome,
        };
        if detections.len() >= MAX_DETECTIONS {
            detections.remove(0);
        }
        detections.push(detection);
    }

    /// Start new partial matches for patterns whose first event type matches.
    fn start_new_partials(
        event: &ServiceEvent,
        patterns: &HashMap<String, SequencePattern>,
        partials: &mut Vec<PartialMatch>,
    ) {
        for pattern in patterns.values() {
            if !pattern.active {
                continue;
            }
            if pattern.event_types.first().map(String::as_str) == Some(&event.event_type) {
                partials.push(PartialMatch {
                    pattern_id: pattern.id.clone(),
                    matched_event_ids: vec![event.event_id.clone()],
                    services: vec![event.service_id.clone()],
                    next_index: 1,
                    first_event_at: event.timestamp,
                    last_event_at: event.timestamp,
                });
            }
        }
    }

    /// Update pattern statistics after completed detections.
    fn update_pattern_stats(&self, completed: &[(String, u64, Timestamp)]) {
        let mut patterns = self.patterns.write();
        for (pid, span_ms, last_ts) in completed {
            if let Some(pat) = patterns.get_mut(pid) {
                pat.occurrence_count += 1;
                pat.last_seen = *last_ts;
                pat.confidence = Self::bayesian_confidence(pat.occurrence_count);

                #[allow(clippy::cast_precision_loss)]
                let span_f = *span_ms as f64;
                if pat.occurrence_count == 1 {
                    pat.mean_interval_ms = span_f;
                    pat.stddev_interval_ms = 0.0;
                } else {
                    let old_mean = pat.mean_interval_ms;
                    #[allow(clippy::cast_precision_loss)]
                    let n = pat.occurrence_count as f64;
                    let new_mean = old_mean + (span_f - old_mean) / n;
                    let delta_pre = span_f - old_mean;
                    let delta_post = span_f - new_mean;
                    let old_var = pat.stddev_interval_ms.powi(2);
                    let new_var = delta_pre.mul_add(delta_post, -old_var) / n + old_var;
                    pat.mean_interval_ms = new_mean;
                    pat.stddev_interval_ms = new_var.max(0.0).sqrt();
                }
            }
        }
    }

    /// Attempt to advance partial matches and emit completed detections.
    fn process_event_matches(&self, event: &ServiceEvent) {
        let patterns = self.patterns.read();
        let mut partials = self.partial_matches.write();
        let mut detections = self.detections.write();

        let completed =
            self.advance_partials(event, &patterns, &mut partials, &mut detections);
        Self::start_new_partials(event, &patterns, &mut partials);

        drop(patterns);
        drop(partials);
        drop(detections);

        if !completed.is_empty() {
            self.update_pattern_stats(&completed);
        }
    }
}

impl SequenceDetector for SequenceDetectorCore {
    fn ingest_event(&self, event: ServiceEvent) -> Result<()> {
        Self::validate_event(&event)?;

        // Buffer the event.
        {
            let mut buf = self.event_buffer.write();
            if buf.len() >= self.config.event_buffer_size {
                buf.pop_front();
            }
            buf.push_back(event.clone());
        }

        // Attempt pattern matching.
        self.process_event_matches(&event);

        Ok(())
    }

    fn detect_sequences(&self) -> Result<Vec<DetectedSequence>> {
        Ok(self.detections.read().clone())
    }

    fn register_sequence(&self, mut pattern: SequencePattern) -> Result<String> {
        Self::validate_pattern(&pattern)?;

        // Ensure the pattern has a valid ID.
        if pattern.id.is_empty() {
            pattern.id = Uuid::new_v4().to_string();
        }

        let id = pattern.id.clone();

        {
            let mut patterns = self.patterns.write();
            if patterns.len() >= self.config.max_patterns && !patterns.contains_key(&id) {
                // Evict lowest-confidence pattern.
                let weakest = patterns
                    .iter()
                    .min_by(|a, b| {
                        a.1.confidence
                            .partial_cmp(&b.1.confidence)
                            .unwrap_or(std::cmp::Ordering::Equal)
                    })
                    .map(|(k, _)| k.clone());
                if let Some(key) = weakest {
                    patterns.remove(&key);
                }
            }
            patterns.insert(id.clone(), pattern);
        }

        Ok(id)
    }

    fn find_matching_sequences(&self, event_type: &str) -> Vec<DetectedSequence> {
        let patterns = self.patterns.read();
        let pattern_ids: Vec<String> = patterns
            .iter()
            .filter(|(_, p)| p.event_types.iter().any(|et| et == event_type))
            .map(|(id, _)| id.clone())
            .collect();
        drop(patterns);

        let detections = self.detections.read();
        detections
            .iter()
            .filter(|d| pattern_ids.contains(&d.pattern_id))
            .cloned()
            .collect()
    }

    fn sequence_count(&self) -> usize {
        self.patterns.read().len()
    }

    fn active_sequence_count(&self) -> usize {
        self.patterns.read().values().filter(|p| p.active).count()
    }

    fn top_sequences(&self, n: usize) -> Vec<SequencePattern> {
        let mut sorted: Vec<SequencePattern> =
            self.patterns.read().values().cloned().collect();
        sorted.sort_by(|a, b| {
            b.confidence
                .partial_cmp(&a.confidence)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        sorted.truncate(n);
        sorted
    }

    fn mark_sequence_outcome(&self, sequence_id: &str, beneficial: bool) -> Result<()> {
        let mut patterns = self.patterns.write();
        let pattern = patterns
            .get_mut(sequence_id)
            .ok_or_else(|| {
                Error::Validation(format!("sequence pattern '{sequence_id}' not found"))
            })?;

        if beneficial {
            pattern.strength = (pattern.strength + 0.1).min(1.0);
        } else {
            pattern.strength = (pattern.strength - 0.1).max(0.0);
        }
        drop(patterns);

        Ok(())
    }

    fn apply_decay(&self) {
        let mut patterns = self.patterns.write();
        for pattern in patterns.values_mut() {
            pattern.confidence = (pattern.confidence - self.config.decay_rate).max(0.0);
            if pattern.confidence < DEACTIVATION_THRESHOLD {
                pattern.active = false;
            }
        }
    }

    fn statistics(&self) -> SequenceStatistics {
        let patterns = self.patterns.read();
        let total_patterns = patterns.len();
        let active: Vec<&SequencePattern> =
            patterns.values().filter(|p| p.active).collect();
        let active_patterns = active.len();
        let avg_confidence = if active.is_empty() {
            0.0
        } else {
            #[allow(clippy::cast_precision_loss)]
            let avg =
                active.iter().map(|p| p.confidence).sum::<f64>() / active.len() as f64;
            avg
        };
        drop(patterns);

        let total_detections = self.detections.read().len();

        SequenceStatistics {
            total_patterns,
            active_patterns,
            total_detections,
            avg_confidence,
        }
    }
}

// ---------------------------------------------------------------------------
// Helper: create a SequencePattern with defaults
// ---------------------------------------------------------------------------

/// Create a new `SequencePattern` with sensible defaults.
///
/// # Arguments
///
/// * `name` - Human-readable pattern name.
/// * `event_types` - Ordered event types (2-5 elements).
#[must_use]
pub fn new_sequence_pattern(name: impl Into<String>, event_types: Vec<String>) -> SequencePattern {
    let now = Timestamp::now();
    SequencePattern {
        id: Uuid::new_v4().to_string(),
        name: name.into(),
        event_types,
        max_window_ms: 30_000,
        min_occurrences: 3,
        occurrence_count: 0,
        mean_interval_ms: 0.0,
        stddev_interval_ms: 0.0,
        confidence: 0.0,
        strength: 0.5,
        associated_outcome: SequenceOutcome::Unknown,
        first_seen: now,
        last_seen: now,
        active: true,
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

    fn make_event(event_type: &str, timestamp_ticks: u64) -> ServiceEvent {
        ServiceEvent {
            event_id: Uuid::new_v4().to_string(),
            service_id: "svc-a".into(),
            event_type: event_type.into(),
            severity: "HIGH".into(),
            timestamp: Timestamp::from_raw(timestamp_ticks),
        }
    }

    fn make_event_with_id(
        event_id: &str,
        event_type: &str,
        service_id: &str,
        timestamp_ticks: u64,
    ) -> ServiceEvent {
        ServiceEvent {
            event_id: event_id.into(),
            service_id: service_id.into(),
            event_type: event_type.into(),
            severity: "MEDIUM".into(),
            timestamp: Timestamp::from_raw(timestamp_ticks),
        }
    }

    fn make_pattern(name: &str, event_types: &[&str]) -> SequencePattern {
        new_sequence_pattern(
            name,
            event_types.iter().map(|s| (*s).to_string()).collect(),
        )
    }

    fn detector() -> SequenceDetectorCore {
        SequenceDetectorCore::default()
    }

    fn detector_with_config(config: SequenceDetectorConfig) -> SequenceDetectorCore {
        SequenceDetectorCore::new(config)
    }

    // -----------------------------------------------------------------------
    // Registration tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_register_pattern_returns_id() {
        let det = detector();
        let pat = make_pattern("test-pat", &["A", "B"]);
        let id = det.register_sequence(pat).unwrap_or_default();
        assert!(!id.is_empty());
    }

    #[test]
    fn test_register_increases_count() {
        let det = detector();
        assert_eq!(det.sequence_count(), 0);
        let pat = make_pattern("p1", &["A", "B"]);
        let _ = det.register_sequence(pat);
        assert_eq!(det.sequence_count(), 1);
    }

    #[test]
    fn test_register_empty_name_rejected() {
        let det = detector();
        let mut pat = make_pattern("", &["A", "B"]);
        pat.name = String::new();
        let result = det.register_sequence(pat);
        assert!(result.is_err());
    }

    #[test]
    fn test_register_empty_event_types_rejected() {
        let det = detector();
        let pat = SequencePattern {
            id: Uuid::new_v4().to_string(),
            name: "bad".into(),
            event_types: vec![],
            max_window_ms: 30_000,
            min_occurrences: 3,
            occurrence_count: 0,
            mean_interval_ms: 0.0,
            stddev_interval_ms: 0.0,
            confidence: 0.0,
            strength: 0.5,
            associated_outcome: SequenceOutcome::Unknown,
            first_seen: Timestamp::now(),
            last_seen: Timestamp::now(),
            active: true,
        };
        assert!(det.register_sequence(pat).is_err());
    }

    #[test]
    fn test_register_single_event_type_rejected() {
        let det = detector();
        let pat = new_sequence_pattern("single", vec!["A".into()]);
        assert!(det.register_sequence(pat).is_err());
    }

    #[test]
    fn test_register_six_event_types_rejected() {
        let det = detector();
        let types: Vec<String> = (0..6).map(|i| format!("E{i}")).collect();
        let pat = new_sequence_pattern("too-many", types);
        assert!(det.register_sequence(pat).is_err());
    }

    #[test]
    fn test_register_five_event_types_accepted() {
        let det = detector();
        let types: Vec<String> = (0..5).map(|i| format!("E{i}")).collect();
        let pat = new_sequence_pattern("five", types);
        assert!(det.register_sequence(pat).is_ok());
    }

    #[test]
    fn test_register_two_event_types_accepted() {
        let det = detector();
        let pat = make_pattern("two", &["A", "B"]);
        assert!(det.register_sequence(pat).is_ok());
    }

    #[test]
    fn test_register_empty_event_type_element_rejected() {
        let det = detector();
        let pat = new_sequence_pattern("empty-el", vec!["A".into(), String::new()]);
        assert!(det.register_sequence(pat).is_err());
    }

    #[test]
    fn test_register_pattern_with_empty_id_gets_uuid() {
        let det = detector();
        let mut pat = make_pattern("auto-id", &["X", "Y"]);
        pat.id = String::new();
        let id = det.register_sequence(pat).unwrap_or_default();
        assert!(!id.is_empty());
    }

    // -----------------------------------------------------------------------
    // Ingest validation tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_ingest_valid_event() {
        let det = detector();
        let ev = make_event("cpu_spike", 100);
        assert!(det.ingest_event(ev).is_ok());
    }

    #[test]
    fn test_ingest_empty_event_id_rejected() {
        let det = detector();
        let ev = ServiceEvent {
            event_id: String::new(),
            service_id: "svc".into(),
            event_type: "X".into(),
            severity: "LOW".into(),
            timestamp: Timestamp::now(),
        };
        assert!(det.ingest_event(ev).is_err());
    }

    #[test]
    fn test_ingest_empty_service_id_rejected() {
        let det = detector();
        let ev = ServiceEvent {
            event_id: "e1".into(),
            service_id: String::new(),
            event_type: "X".into(),
            severity: "LOW".into(),
            timestamp: Timestamp::now(),
        };
        assert!(det.ingest_event(ev).is_err());
    }

    #[test]
    fn test_ingest_empty_event_type_rejected() {
        let det = detector();
        let ev = ServiceEvent {
            event_id: "e1".into(),
            service_id: "svc".into(),
            event_type: String::new(),
            severity: "LOW".into(),
            timestamp: Timestamp::now(),
        };
        assert!(det.ingest_event(ev).is_err());
    }

    #[test]
    fn test_ingest_empty_severity_rejected() {
        let det = detector();
        let ev = ServiceEvent {
            event_id: "e1".into(),
            service_id: "svc".into(),
            event_type: "X".into(),
            severity: String::new(),
            timestamp: Timestamp::now(),
        };
        assert!(det.ingest_event(ev).is_err());
    }

    // -----------------------------------------------------------------------
    // 2-event sequence detection
    // -----------------------------------------------------------------------

    #[test]
    fn test_detect_two_event_sequence() {
        let det = detector();
        let pat = make_pattern("ab-seq", &["A", "B"]);
        let _ = det.register_sequence(pat);

        let _ = det.ingest_event(make_event("A", 100));
        let _ = det.ingest_event(make_event("B", 200));

        let detections = det.detect_sequences().unwrap_or_default();
        assert_eq!(detections.len(), 1);
        assert_eq!(detections[0].matched_events.len(), 2);
    }

    #[test]
    fn test_detect_two_event_span() {
        let det = detector();
        let pat = make_pattern("ab-span", &["A", "B"]);
        let _ = det.register_sequence(pat);

        let _ = det.ingest_event(make_event("A", 1000));
        let _ = det.ingest_event(make_event("B", 1500));

        let detections = det.detect_sequences().unwrap_or_default();
        assert_eq!(detections.len(), 1);
        assert_eq!(detections[0].span_ms, 500);
    }

    // -----------------------------------------------------------------------
    // 3-event sequence detection
    // -----------------------------------------------------------------------

    #[test]
    fn test_detect_three_event_sequence() {
        let det = detector();
        let pat = make_pattern("abc-seq", &["A", "B", "C"]);
        let _ = det.register_sequence(pat);

        let _ = det.ingest_event(make_event("A", 100));
        let _ = det.ingest_event(make_event("B", 200));
        let _ = det.ingest_event(make_event("C", 300));

        let detections = det.detect_sequences().unwrap_or_default();
        assert_eq!(detections.len(), 1);
        assert_eq!(detections[0].matched_events.len(), 3);
        assert_eq!(detections[0].span_ms, 200);
    }

    // -----------------------------------------------------------------------
    // Partial match timeout
    // -----------------------------------------------------------------------

    #[test]
    fn test_partial_match_timeout() {
        let config = SequenceDetectorConfig {
            partial_match_timeout_ms: 500,
            ..SequenceDetectorConfig::default()
        };
        let det = detector_with_config(config);
        let pat = make_pattern("timeout-test", &["A", "B"]);
        let _ = det.register_sequence(pat);

        let _ = det.ingest_event(make_event("A", 100));
        // B arrives too late (past the 500 tick timeout).
        let _ = det.ingest_event(make_event("B", 700));

        let detections = det.detect_sequences().unwrap_or_default();
        assert!(detections.is_empty());
    }

    #[test]
    fn test_partial_match_within_timeout() {
        let config = SequenceDetectorConfig {
            partial_match_timeout_ms: 500,
            ..SequenceDetectorConfig::default()
        };
        let det = detector_with_config(config);
        let pat = make_pattern("within-timeout", &["A", "B"]);
        let _ = det.register_sequence(pat);

        let _ = det.ingest_event(make_event("A", 100));
        let _ = det.ingest_event(make_event("B", 500));

        let detections = det.detect_sequences().unwrap_or_default();
        assert_eq!(detections.len(), 1);
    }

    // -----------------------------------------------------------------------
    // Window constraint on pattern
    // -----------------------------------------------------------------------

    #[test]
    fn test_pattern_window_exceeded() {
        let det = detector();
        let mut pat = make_pattern("window-test", &["A", "B"]);
        pat.max_window_ms = 100;
        let _ = det.register_sequence(pat);

        let _ = det.ingest_event(make_event("A", 1000));
        let _ = det.ingest_event(make_event("B", 1200));

        let detections = det.detect_sequences().unwrap_or_default();
        assert!(detections.is_empty());
    }

    #[test]
    fn test_pattern_window_respected() {
        let det = detector();
        let mut pat = make_pattern("window-ok", &["A", "B"]);
        pat.max_window_ms = 200;
        let _ = det.register_sequence(pat);

        let _ = det.ingest_event(make_event("A", 1000));
        let _ = det.ingest_event(make_event("B", 1100));

        let detections = det.detect_sequences().unwrap_or_default();
        assert_eq!(detections.len(), 1);
    }

    // -----------------------------------------------------------------------
    // Confidence formula
    // -----------------------------------------------------------------------

    #[test]
    fn test_bayesian_confidence_zero_occurrences() {
        let c = SequenceDetectorCore::bayesian_confidence(0);
        assert!((c - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_bayesian_confidence_five_occurrences() {
        let c = SequenceDetectorCore::bayesian_confidence(5);
        assert!((c - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_bayesian_confidence_twenty_occurrences() {
        let c = SequenceDetectorCore::bayesian_confidence(20);
        assert!((c - 0.8).abs() < f64::EPSILON);
    }

    #[test]
    fn test_bayesian_confidence_monotonically_increasing() {
        let mut prev = 0.0;
        for i in 1..=100 {
            let c = SequenceDetectorCore::bayesian_confidence(i);
            assert!(c > prev);
            prev = c;
        }
    }

    #[test]
    fn test_bayesian_confidence_approaches_one() {
        let c = SequenceDetectorCore::bayesian_confidence(1_000_000);
        assert!(c > 0.999);
        assert!(c <= 1.0);
    }

    // -----------------------------------------------------------------------
    // Decay
    // -----------------------------------------------------------------------

    #[test]
    fn test_decay_reduces_confidence() {
        let det = detector();
        let mut pat = make_pattern("decay-test", &["A", "B"]);
        pat.confidence = 0.8;
        let id = det.register_sequence(pat).unwrap_or_default();

        det.apply_decay();

        let top = det.top_sequences(1);
        assert_eq!(top.len(), 1);
        assert!((top[0].confidence - (0.8 - DEFAULT_DECAY_RATE)).abs() < f64::EPSILON);
        assert_eq!(top[0].id, id);
    }

    #[test]
    fn test_decay_deactivates_low_confidence() {
        let det = detector();
        let mut pat = make_pattern("low-conf", &["A", "B"]);
        pat.confidence = 0.05;
        let _ = det.register_sequence(pat);

        det.apply_decay();

        assert_eq!(det.active_sequence_count(), 0);
    }

    #[test]
    fn test_decay_does_not_go_below_zero() {
        let det = detector();
        let mut pat = make_pattern("floor", &["X", "Y"]);
        pat.confidence = 0.001;
        let _ = det.register_sequence(pat);

        det.apply_decay();

        let top = det.top_sequences(1);
        assert!(top[0].confidence >= 0.0);
    }

    #[test]
    fn test_multiple_decays_eventually_deactivate() {
        let det = detector();
        let mut pat = make_pattern("multi-decay", &["A", "B"]);
        pat.confidence = 0.5;
        let _ = det.register_sequence(pat);

        // Apply decay many times.
        for _ in 0..200 {
            det.apply_decay();
        }

        assert_eq!(det.active_sequence_count(), 0);
    }

    // -----------------------------------------------------------------------
    // Statistics
    // -----------------------------------------------------------------------

    #[test]
    fn test_statistics_empty() {
        let det = detector();
        let stats = det.statistics();
        assert_eq!(stats.total_patterns, 0);
        assert_eq!(stats.active_patterns, 0);
        assert_eq!(stats.total_detections, 0);
        assert!((stats.avg_confidence - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_statistics_with_patterns() {
        let det = detector();
        let mut p1 = make_pattern("s1", &["A", "B"]);
        p1.confidence = 0.6;
        let mut p2 = make_pattern("s2", &["C", "D"]);
        p2.confidence = 0.4;
        let _ = det.register_sequence(p1);
        let _ = det.register_sequence(p2);

        let stats = det.statistics();
        assert_eq!(stats.total_patterns, 2);
        assert_eq!(stats.active_patterns, 2);
        assert!((stats.avg_confidence - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_statistics_with_inactive_pattern() {
        let det = detector();
        let mut p1 = make_pattern("active", &["A", "B"]);
        p1.confidence = 0.8;
        let mut p2 = make_pattern("inactive", &["C", "D"]);
        p2.confidence = 0.3;
        p2.active = false;
        let _ = det.register_sequence(p1);
        let _ = det.register_sequence(p2);

        let stats = det.statistics();
        assert_eq!(stats.total_patterns, 2);
        assert_eq!(stats.active_patterns, 1);
        assert!((stats.avg_confidence - 0.8).abs() < f64::EPSILON);
    }

    #[test]
    fn test_statistics_after_detection() {
        let det = detector();
        let pat = make_pattern("det-stat", &["A", "B"]);
        let _ = det.register_sequence(pat);

        let _ = det.ingest_event(make_event("A", 100));
        let _ = det.ingest_event(make_event("B", 200));

        let stats = det.statistics();
        assert_eq!(stats.total_detections, 1);
    }

    // -----------------------------------------------------------------------
    // top_sequences ordering
    // -----------------------------------------------------------------------

    #[test]
    fn test_top_sequences_ordered_by_confidence() {
        let det = detector();
        let mut low = make_pattern("low", &["A", "B"]);
        low.confidence = 0.2;
        let mut mid = make_pattern("mid", &["C", "D"]);
        mid.confidence = 0.5;
        let mut high = make_pattern("high", &["E", "F"]);
        high.confidence = 0.9;

        let _ = det.register_sequence(low);
        let _ = det.register_sequence(mid);
        let _ = det.register_sequence(high);

        let top = det.top_sequences(3);
        assert_eq!(top.len(), 3);
        assert!((top[0].confidence - 0.9).abs() < f64::EPSILON);
        assert!((top[1].confidence - 0.5).abs() < f64::EPSILON);
        assert!((top[2].confidence - 0.2).abs() < f64::EPSILON);
    }

    #[test]
    fn test_top_sequences_truncates() {
        let det = detector();
        for i in 0..10 {
            let mut pat = make_pattern(&format!("p{i}"), &["A", "B"]);
            pat.confidence = f64::from(i) / 10.0;
            let _ = det.register_sequence(pat);
        }

        let top = det.top_sequences(3);
        assert_eq!(top.len(), 3);
    }

    #[test]
    fn test_top_sequences_when_empty() {
        let det = detector();
        let top = det.top_sequences(5);
        assert!(top.is_empty());
    }

    // -----------------------------------------------------------------------
    // mark_outcome
    // -----------------------------------------------------------------------

    #[test]
    fn test_mark_outcome_beneficial_increases_strength() {
        let det = detector();
        let mut pat = make_pattern("benefit", &["A", "B"]);
        pat.strength = 0.5;
        let id = det.register_sequence(pat).unwrap_or_default();

        det.mark_sequence_outcome(&id, true)
            .unwrap_or_default();

        let top = det.top_sequences(1);
        assert!((top[0].strength - 0.6).abs() < f64::EPSILON);
    }

    #[test]
    fn test_mark_outcome_detrimental_decreases_strength() {
        let det = detector();
        let mut pat = make_pattern("detriment", &["A", "B"]);
        pat.strength = 0.5;
        let id = det.register_sequence(pat).unwrap_or_default();

        det.mark_sequence_outcome(&id, false)
            .unwrap_or_default();

        let top = det.top_sequences(1);
        assert!((top[0].strength - 0.4).abs() < f64::EPSILON);
    }

    #[test]
    fn test_mark_outcome_strength_capped_at_one() {
        let det = detector();
        let mut pat = make_pattern("cap-high", &["A", "B"]);
        pat.strength = 0.95;
        let id = det.register_sequence(pat).unwrap_or_default();

        det.mark_sequence_outcome(&id, true)
            .unwrap_or_default();

        let top = det.top_sequences(1);
        assert!((top[0].strength - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_mark_outcome_strength_floored_at_zero() {
        let det = detector();
        let mut pat = make_pattern("cap-low", &["A", "B"]);
        pat.strength = 0.05;
        let id = det.register_sequence(pat).unwrap_or_default();

        det.mark_sequence_outcome(&id, false)
            .unwrap_or_default();

        let top = det.top_sequences(1);
        assert!(top[0].strength >= 0.0);
    }

    #[test]
    fn test_mark_outcome_unknown_id_returns_error() {
        let det = detector();
        let result = det.mark_sequence_outcome("nonexistent", true);
        assert!(result.is_err());
    }

    // -----------------------------------------------------------------------
    // Event buffer cap
    // -----------------------------------------------------------------------

    #[test]
    fn test_event_buffer_capped() {
        let config = SequenceDetectorConfig {
            event_buffer_size: 5,
            ..SequenceDetectorConfig::default()
        };
        let det = detector_with_config(config);

        for i in 0..10 {
            let _ = det.ingest_event(make_event("A", i));
        }

        let buf = det.event_buffer.read();
        assert_eq!(buf.len(), 5);
    }

    // -----------------------------------------------------------------------
    // find_matching_sequences
    // -----------------------------------------------------------------------

    #[test]
    fn test_find_matching_no_detections() {
        let det = detector();
        let pat = make_pattern("find-test", &["A", "B"]);
        let _ = det.register_sequence(pat);

        let matches = det.find_matching_sequences("A");
        assert!(matches.is_empty());
    }

    #[test]
    fn test_find_matching_with_detections() {
        let det = detector();
        let pat = make_pattern("find-det", &["A", "B"]);
        let _ = det.register_sequence(pat);

        let _ = det.ingest_event(make_event("A", 100));
        let _ = det.ingest_event(make_event("B", 200));

        let matches = det.find_matching_sequences("A");
        assert_eq!(matches.len(), 1);
    }

    #[test]
    fn test_find_matching_wrong_type_returns_empty() {
        let det = detector();
        let pat = make_pattern("find-wrong", &["A", "B"]);
        let _ = det.register_sequence(pat);

        let _ = det.ingest_event(make_event("A", 100));
        let _ = det.ingest_event(make_event("B", 200));

        let matches = det.find_matching_sequences("Z");
        assert!(matches.is_empty());
    }

    // -----------------------------------------------------------------------
    // Concurrent access
    // -----------------------------------------------------------------------

    #[test]
    fn test_concurrent_ingest_and_read() {
        use std::sync::Arc;
        use std::thread;

        let det = Arc::new(SequenceDetectorCore::default());
        let pat = make_pattern("concurrent", &["A", "B"]);
        let _ = det.register_sequence(pat);

        let det_writer = Arc::clone(&det);
        let writer = thread::spawn(move || {
            for i in 0..100 {
                let _ = det_writer.ingest_event(make_event("A", i * 10));
                let _ = det_writer.ingest_event(make_event("B", i * 10 + 5));
            }
        });

        let det_reader = Arc::clone(&det);
        let reader = thread::spawn(move || {
            for _ in 0..100 {
                let _ = det_reader.detect_sequences();
                let _ = det_reader.statistics();
            }
        });

        writer.join().unwrap_or_default();
        reader.join().unwrap_or_default();

        // No panics or deadlocks — just verify we can still read.
        let stats = det.statistics();
        assert!(stats.total_patterns > 0);
    }

    // -----------------------------------------------------------------------
    // Outcome enum
    // -----------------------------------------------------------------------

    #[test]
    fn test_sequence_outcome_display() {
        assert_eq!(SequenceOutcome::FailurePrecursor.to_string(), "FAILURE_PRECURSOR");
        assert_eq!(SequenceOutcome::RecoverySignal.to_string(), "RECOVERY_SIGNAL");
        assert_eq!(
            SequenceOutcome::MaintenanceTrigger.to_string(),
            "MAINTENANCE_TRIGGER"
        );
        assert_eq!(SequenceOutcome::Unknown.to_string(), "UNKNOWN");
    }

    #[test]
    fn test_sequence_outcome_equality() {
        assert_eq!(SequenceOutcome::Unknown, SequenceOutcome::Unknown);
        assert_ne!(SequenceOutcome::FailurePrecursor, SequenceOutcome::RecoverySignal);
    }

    #[test]
    fn test_sequence_outcome_copy() {
        let a = SequenceOutcome::FailurePrecursor;
        let b = a;
        assert_eq!(a, b);
    }

    // -----------------------------------------------------------------------
    // Pattern occurrence updates on detection
    // -----------------------------------------------------------------------

    #[test]
    fn test_occurrence_count_increments_on_detection() {
        let det = detector();
        let pat = make_pattern("occur-test", &["A", "B"]);
        let id = det.register_sequence(pat).unwrap_or_default();

        let _ = det.ingest_event(make_event("A", 100));
        let _ = det.ingest_event(make_event("B", 200));

        let patterns = det.patterns.read();
        let p = &patterns[&id];
        assert_eq!(p.occurrence_count, 1);
    }

    #[test]
    fn test_confidence_updated_after_detection() {
        let det = detector();
        let pat = make_pattern("conf-update", &["A", "B"]);
        let id = det.register_sequence(pat).unwrap_or_default();

        let _ = det.ingest_event(make_event("A", 100));
        let _ = det.ingest_event(make_event("B", 200));

        let patterns = det.patterns.read();
        let p = &patterns[&id];
        // 1 / (1 + 5) = ~0.1667
        let expected = 1.0 / 6.0;
        assert!((p.confidence - expected).abs() < 1e-10);
    }

    #[test]
    fn test_mean_interval_after_single_detection() {
        let det = detector();
        let pat = make_pattern("mean-test", &["A", "B"]);
        let id = det.register_sequence(pat).unwrap_or_default();

        let _ = det.ingest_event(make_event("A", 1000));
        let _ = det.ingest_event(make_event("B", 1300));

        let patterns = det.patterns.read();
        let p = &patterns[&id];
        assert!((p.mean_interval_ms - 300.0).abs() < f64::EPSILON);
        assert!((p.stddev_interval_ms - 0.0).abs() < f64::EPSILON);
    }

    // -----------------------------------------------------------------------
    // Multiple detections
    // -----------------------------------------------------------------------

    #[test]
    fn test_multiple_sequential_detections() {
        let det = detector();
        let pat = make_pattern("multi-det", &["X", "Y"]);
        let _ = det.register_sequence(pat);

        let _ = det.ingest_event(make_event("X", 100));
        let _ = det.ingest_event(make_event("Y", 200));
        let _ = det.ingest_event(make_event("X", 300));
        let _ = det.ingest_event(make_event("Y", 400));

        let detections = det.detect_sequences().unwrap_or_default();
        assert_eq!(detections.len(), 2);
    }

    // -----------------------------------------------------------------------
    // Inactive pattern not matched
    // -----------------------------------------------------------------------

    #[test]
    fn test_inactive_pattern_not_matched() {
        let det = detector();
        let mut pat = make_pattern("inactive", &["A", "B"]);
        pat.active = false;
        let _ = det.register_sequence(pat);

        let _ = det.ingest_event(make_event("A", 100));
        let _ = det.ingest_event(make_event("B", 200));

        let detections = det.detect_sequences().unwrap_or_default();
        assert!(detections.is_empty());
    }

    // -----------------------------------------------------------------------
    // Detection cap
    // -----------------------------------------------------------------------

    #[test]
    fn test_detections_capped_at_max() {
        let det = detector();
        let pat = make_pattern("cap-det", &["A", "B"]);
        let _ = det.register_sequence(pat);

        for i in 0..600_u64 {
            let _ = det.ingest_event(make_event("A", i * 100));
            let _ = det.ingest_event(make_event("B", i * 100 + 50));
        }

        let detections = det.detect_sequences().unwrap_or_default();
        assert!(detections.len() <= MAX_DETECTIONS);
    }

    // -----------------------------------------------------------------------
    // Services involved tracking
    // -----------------------------------------------------------------------

    #[test]
    fn test_services_involved_tracked() {
        let det = detector();
        let pat = make_pattern("svc-track", &["A", "B"]);
        let _ = det.register_sequence(pat);

        let _ = det.ingest_event(make_event_with_id("e1", "A", "svc-alpha", 100));
        let _ = det.ingest_event(make_event_with_id("e2", "B", "svc-beta", 200));

        let detections = det.detect_sequences().unwrap_or_default();
        assert_eq!(detections.len(), 1);
        assert_eq!(detections[0].services_involved.len(), 2);
        assert!(detections[0].services_involved.contains(&"svc-alpha".to_string()));
        assert!(detections[0].services_involved.contains(&"svc-beta".to_string()));
    }

    #[test]
    fn test_services_involved_deduplicated() {
        let det = detector();
        let pat = make_pattern("svc-dedup", &["A", "B"]);
        let _ = det.register_sequence(pat);

        let _ = det.ingest_event(make_event_with_id("e1", "A", "svc-same", 100));
        let _ = det.ingest_event(make_event_with_id("e2", "B", "svc-same", 200));

        let detections = det.detect_sequences().unwrap_or_default();
        assert_eq!(detections.len(), 1);
        assert_eq!(detections[0].services_involved.len(), 1);
    }

    // -----------------------------------------------------------------------
    // Pattern eviction on max capacity
    // -----------------------------------------------------------------------

    #[test]
    fn test_pattern_eviction_at_max_capacity() {
        let config = SequenceDetectorConfig {
            max_patterns: 3,
            ..SequenceDetectorConfig::default()
        };
        let det = detector_with_config(config);

        let mut p1 = make_pattern("p1", &["A", "B"]);
        p1.confidence = 0.9;
        let mut p2 = make_pattern("p2", &["C", "D"]);
        p2.confidence = 0.1;
        let mut p3 = make_pattern("p3", &["E", "F"]);
        p3.confidence = 0.5;

        let _ = det.register_sequence(p1);
        let _ = det.register_sequence(p2);
        let _ = det.register_sequence(p3);

        assert_eq!(det.sequence_count(), 3);

        // Adding a 4th should evict the lowest confidence (p2 at 0.1).
        let mut p4 = make_pattern("p4", &["G", "H"]);
        p4.confidence = 0.7;
        let _ = det.register_sequence(p4);

        assert_eq!(det.sequence_count(), 3);
    }

    // -----------------------------------------------------------------------
    // new_sequence_pattern helper
    // -----------------------------------------------------------------------

    #[test]
    fn test_new_sequence_pattern_defaults() {
        let pat = new_sequence_pattern("test", vec!["A".into(), "B".into()]);
        assert!(!pat.id.is_empty());
        assert_eq!(pat.name, "test");
        assert_eq!(pat.event_types.len(), 2);
        assert_eq!(pat.max_window_ms, 30_000);
        assert_eq!(pat.min_occurrences, 3);
        assert_eq!(pat.occurrence_count, 0);
        assert!((pat.confidence - 0.0).abs() < f64::EPSILON);
        assert!((pat.strength - 0.5).abs() < f64::EPSILON);
        assert_eq!(pat.associated_outcome, SequenceOutcome::Unknown);
        assert!(pat.active);
    }

    // -----------------------------------------------------------------------
    // Config defaults
    // -----------------------------------------------------------------------

    #[test]
    fn test_config_defaults() {
        let cfg = SequenceDetectorConfig::default();
        assert_eq!(cfg.event_buffer_size, DEFAULT_EVENT_BUFFER_SIZE);
        assert_eq!(cfg.max_patterns, DEFAULT_MAX_PATTERNS);
        assert_eq!(cfg.partial_match_timeout_ms, DEFAULT_PARTIAL_MATCH_TIMEOUT_MS);
        assert!((cfg.min_confidence - DEFAULT_MIN_CONFIDENCE).abs() < f64::EPSILON);
        assert!((cfg.decay_rate - DEFAULT_DECAY_RATE).abs() < f64::EPSILON);
    }

    // -----------------------------------------------------------------------
    // Default impl for SequenceDetectorCore
    // -----------------------------------------------------------------------

    #[test]
    fn test_detector_core_default() {
        let det = SequenceDetectorCore::default();
        assert_eq!(det.sequence_count(), 0);
        assert_eq!(det.active_sequence_count(), 0);
    }

    // -----------------------------------------------------------------------
    // Unrelated events do not trigger matches
    // -----------------------------------------------------------------------

    #[test]
    fn test_unrelated_events_no_match() {
        let det = detector();
        let pat = make_pattern("ab-only", &["A", "B"]);
        let _ = det.register_sequence(pat);

        let _ = det.ingest_event(make_event("X", 100));
        let _ = det.ingest_event(make_event("Y", 200));

        let detections = det.detect_sequences().unwrap_or_default();
        assert!(detections.is_empty());
    }

    // -----------------------------------------------------------------------
    // Out-of-order events do not match
    // -----------------------------------------------------------------------

    #[test]
    fn test_out_of_order_no_match() {
        let det = detector();
        let pat = make_pattern("ordered", &["A", "B"]);
        let _ = det.register_sequence(pat);

        // Send B before A.
        let _ = det.ingest_event(make_event("B", 100));
        let _ = det.ingest_event(make_event("A", 200));

        let detections = det.detect_sequences().unwrap_or_default();
        assert!(detections.is_empty());
    }

    // -----------------------------------------------------------------------
    // Outcome inherited in detection
    // -----------------------------------------------------------------------

    #[test]
    fn test_detection_inherits_outcome() {
        let det = detector();
        let mut pat = make_pattern("outcome-inh", &["A", "B"]);
        pat.associated_outcome = SequenceOutcome::FailurePrecursor;
        let _ = det.register_sequence(pat);

        let _ = det.ingest_event(make_event("A", 100));
        let _ = det.ingest_event(make_event("B", 200));

        let detections = det.detect_sequences().unwrap_or_default();
        assert_eq!(detections[0].outcome, SequenceOutcome::FailurePrecursor);
    }

    // -----------------------------------------------------------------------
    // active_sequence_count vs sequence_count
    // -----------------------------------------------------------------------

    #[test]
    fn test_active_vs_total_count() {
        let det = detector();
        let p1 = make_pattern("active1", &["A", "B"]);
        let mut p2 = make_pattern("inactive1", &["C", "D"]);
        p2.active = false;
        let _ = det.register_sequence(p1);
        let _ = det.register_sequence(p2);

        assert_eq!(det.sequence_count(), 2);
        assert_eq!(det.active_sequence_count(), 1);
    }
}
