//! # M38: Emergence Detector
//!
//! Consumes correlated events from M37 (via Observer Bus) and detects
//! emergent system behaviors that cannot be predicted from individual
//! component states alone.
//!
//! ## Layer: L7 (Observer)
//! ## Lock Order: 3 (after `LogCorrelator`, before `EvolutionChamber`)
//! ## Dependencies: M37 (`LogCorrelator`), M01 (Error)
//!
//! ## Emergence Types
//!
//! | Type | Detection Criteria |
//! |------|--------------------|
//! | `CascadeFailure` | Correlated failures spanning >= `cascade_depth_threshold` services |
//! | `SynergyShift` | Synergy delta exceeds `synergy_delta_threshold` |
//! | `ResonanceCycle` | Repeated correlation patterns >= `resonance_min_cycles` |
//! | `AttractorFormation` | Multiple correlations converge on the same service set |
//! | `PhaseTransition` | Metric crosses regime boundary (large value jump) |
//! | `BeneficialEmergence` | Self-healing pathway detected via recovery events |
//!
//! ## Related Documentation
//! - [Emergence Detector Spec](../../ai_specs/evolution_chamber_ai_specs/EMERGENCE_DETECTOR_SPEC.md)
//! - [Layer Specification](../../ai_docs/layers/L07_OBSERVER.md)

use std::collections::{HashMap, VecDeque};

use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{Error, Result};

use super::log_correlator::CorrelatedEvent;

/// Default cascade depth threshold before flagging cascade failure.
const DEFAULT_CASCADE_DEPTH_THRESHOLD: u32 = 3;

/// Default synergy delta magnitude for shift detection.
const DEFAULT_SYNERGY_DELTA_THRESHOLD: f64 = 0.15;

/// Default minimum observed cycles for resonance detection.
const DEFAULT_RESONANCE_MIN_CYCLES: u32 = 3;

/// Default history capacity (emergence records retained).
const DEFAULT_HISTORY_CAPACITY: usize = 1000;

/// Default detection interval in milliseconds.
const DEFAULT_DETECTION_INTERVAL_MS: u64 = 1000;

/// Default minimum confidence to register an emergence.
const DEFAULT_MIN_CONFIDENCE: f64 = 0.7;

/// Phase transition ratio threshold: `new_value` / `old_value` must exceed this
/// for the transition to be considered a phase change.
const PHASE_TRANSITION_RATIO: f64 = 2.0;

/// Recovery time ceiling (ms) for beneficial emergence detection.
/// Recoveries faster than this indicate self-healing pathways.
const BENEFICIAL_RECOVERY_CEILING_MS: u64 = 5000;

/// Classification of emergent system behaviors.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EmergenceType {
    /// Correlated failures cascading across multiple services.
    CascadeFailure,
    /// Significant shift in cross-service synergy scores.
    SynergyShift,
    /// Repeated cyclic patterns detected in correlation data.
    ResonanceCycle,
    /// Multiple correlations converging on the same attractor set.
    AttractorFormation,
    /// A system metric crossing a regime boundary.
    PhaseTransition,
    /// Self-healing pathway detected through recovery events.
    BeneficialEmergence,
    /// Cascade pipeline amplification exceeding safe bounds.
    /// Detected when signal amplification across pipeline stages surpasses threshold.
    CascadeAmplification,
    /// System temperature exceeding thermal regulation bounds.
    /// Detected when the V3 thermal controller reports overheating.
    ThermalRunaway,
}

/// Type alias for backward compatibility with `mod.rs` re-exports.
pub type EmergentBehavior = EmergenceType;

/// Severity classification for emergence events.
///
/// Maps a continuous severity score [0.0, 1.0] to a discrete class.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EmergenceSeverity {
    /// Severity < 0.4
    Low,
    /// 0.4 <= severity < 0.7
    Medium,
    /// 0.7 <= severity < 0.9
    High,
    /// Severity >= 0.9
    Critical,
}

impl EmergenceSeverity {
    /// Classifies a numeric severity into a discrete class.
    #[must_use]
    pub fn from_score(severity: f64) -> Self {
        if severity >= 0.9 {
            Self::Critical
        } else if severity >= 0.7 {
            Self::High
        } else if severity >= 0.4 {
            Self::Medium
        } else {
            Self::Low
        }
    }
}

impl std::fmt::Display for EmergenceSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Medium => write!(f, "medium"),
            Self::High => write!(f, "high"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

impl std::fmt::Display for EmergenceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CascadeFailure => write!(f, "cascade_failure"),
            Self::SynergyShift => write!(f, "synergy_shift"),
            Self::ResonanceCycle => write!(f, "resonance_cycle"),
            Self::AttractorFormation => write!(f, "attractor_formation"),
            Self::PhaseTransition => write!(f, "phase_transition"),
            Self::BeneficialEmergence => write!(f, "beneficial_emergence"),
            Self::CascadeAmplification => write!(f, "cascade_amplification"),
            Self::ThermalRunaway => write!(f, "thermal_runaway"),
        }
    }
}

/// A detected emergent behavior record.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EmergenceRecord {
    /// Unique record ID (UUID v4).
    pub id: String,
    /// Classification of the detected emergence.
    pub emergence_type: EmergenceType,
    /// Detection confidence [0.0, 1.0].
    pub confidence: f64,
    /// Severity assessment [0.0, 1.0].
    pub severity: f64,
    /// IDs of source correlations that contributed to this detection.
    pub source_correlations: Vec<String>,
    /// Layer indices affected by this emergence.
    pub affected_layers: Vec<u8>,
    /// Service identifiers affected by this emergence.
    pub affected_services: Vec<String>,
    /// Human-readable description of the emergence.
    pub description: String,
    /// Timestamp of detection.
    pub detected_at: DateTime<Utc>,
    /// Optional recommended remediation action.
    pub recommended_action: Option<String>,
}

/// State of an active emergence monitor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MonitorState {
    /// Actively watching for emergence signals.
    Watching,
    /// Threshold exceeded; emergence detected.
    Triggered,
    /// Recently triggered; suppressing duplicate detections.
    Cooldown,
}

/// An active monitor tracking a specific emergence behavior.
#[derive(Clone, Debug)]
pub struct EmergenceMonitor {
    /// Unique monitor identifier (UUID v4).
    pub monitor_id: String,
    /// The behavior type this monitor tracks.
    pub behavior_type: EmergenceType,
    /// Current state of the monitor.
    pub state: MonitorState,
    /// Evidence strings accumulated toward triggering.
    pub accumulated_evidence: Vec<String>,
    /// Current accumulated confidence [0.0, 1.0].
    pub confidence: f64,
    /// Timestamp when this monitor was created.
    pub started_at: DateTime<Utc>,
}

/// Configuration for the Emergence Detector.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EmergenceDetectorConfig {
    /// Minimum cascade depth before flagging `CascadeFailure`.
    pub cascade_depth_threshold: u32,
    /// Minimum synergy delta magnitude for `SynergyShift`.
    pub synergy_delta_threshold: f64,
    /// Minimum observed cycles for `ResonanceCycle`.
    pub resonance_min_cycles: u32,
    /// Maximum emergence records retained.
    pub history_capacity: usize,
    /// Detection interval in milliseconds.
    pub detection_interval_ms: u64,
    /// Minimum confidence to register an emergence.
    pub min_confidence: f64,
}

impl Default for EmergenceDetectorConfig {
    fn default() -> Self {
        Self {
            cascade_depth_threshold: DEFAULT_CASCADE_DEPTH_THRESHOLD,
            synergy_delta_threshold: DEFAULT_SYNERGY_DELTA_THRESHOLD,
            resonance_min_cycles: DEFAULT_RESONANCE_MIN_CYCLES,
            history_capacity: DEFAULT_HISTORY_CAPACITY,
            detection_interval_ms: DEFAULT_DETECTION_INTERVAL_MS,
            min_confidence: DEFAULT_MIN_CONFIDENCE,
        }
    }
}

/// Aggregate statistics for the Emergence Detector.
#[derive(Clone, Debug, Default)]
pub struct EmergenceStats {
    /// Total emergence records detected.
    pub total_detected: u64,
    /// Detections broken down by emergence type name.
    pub by_type: HashMap<String, u64>,
    /// Detections broken down by severity class name.
    pub by_severity_class: HashMap<String, u64>,
    /// Number of currently active monitors.
    pub active_monitors: usize,
    /// Total detection cycles executed.
    pub detection_cycles: u64,
}

/// M38: Emergence Detector.
///
/// Consumes correlated events from M37 (via Observer Bus) and detects
/// emergent system behaviors that cannot be predicted from individual
/// component states alone.
///
/// # Thread Safety
///
/// All mutable state is protected by `parking_lot::RwLock`.
///
/// # Lock Order
///
/// Lock order 3 (after `LogCorrelator`, before `EvolutionChamber`).
pub struct EmergenceDetector {
    /// Detected emergence records (current session).
    detected_behaviors: RwLock<Vec<EmergenceRecord>>,
    /// Active monitors keyed by monitor ID.
    active_monitors: RwLock<HashMap<String, EmergenceMonitor>>,
    /// Bounded history ring buffer.
    behavior_history: RwLock<VecDeque<EmergenceRecord>>,
    /// Immutable configuration.
    config: EmergenceDetectorConfig,
    /// Runtime-mutable `min_confidence` override (set by RALPH mutations).
    confidence_override: RwLock<Option<f64>>,
    /// Aggregate statistics.
    stats: RwLock<EmergenceStats>,
}

impl EmergenceDetector {
    /// Creates a new `EmergenceDetector` with default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self::with_config(EmergenceDetectorConfig::default())
    }

    /// Creates a new `EmergenceDetector` with the given configuration.
    #[must_use]
    pub fn with_config(config: EmergenceDetectorConfig) -> Self {
        Self {
            detected_behaviors: RwLock::new(Vec::new()),
            active_monitors: RwLock::new(HashMap::new()),
            behavior_history: RwLock::new(VecDeque::with_capacity(
                config.history_capacity.min(2000),
            )),
            confidence_override: RwLock::new(None),
            config,
            stats: RwLock::new(EmergenceStats::default()),
        }
    }

    /// Validates the given configuration.
    ///
    /// # Errors
    ///
    /// Returns `Error::Config` if any parameter is out of range:
    /// - `cascade_depth_threshold` must be >= 1
    /// - `synergy_delta_threshold` must be in (0.0, 1.0]
    /// - `resonance_min_cycles` must be >= 2
    /// - `history_capacity` must be > 0
    /// - `detection_interval_ms` must be > 0
    /// - `min_confidence` must be in [0.0, 1.0]
    pub fn validate_config(config: &EmergenceDetectorConfig) -> Result<()> {
        if config.cascade_depth_threshold < 1 {
            return Err(Error::Config(
                "cascade_depth_threshold must be >= 1".into(),
            ));
        }
        if config.synergy_delta_threshold <= 0.0 || config.synergy_delta_threshold > 1.0 {
            return Err(Error::Config(
                "synergy_delta_threshold must be in (0.0, 1.0]".into(),
            ));
        }
        if config.resonance_min_cycles < 2 {
            return Err(Error::Config(
                "resonance_min_cycles must be >= 2".into(),
            ));
        }
        if config.history_capacity == 0 {
            return Err(Error::Config(
                "history_capacity must be > 0".into(),
            ));
        }
        if config.detection_interval_ms == 0 {
            return Err(Error::Config(
                "detection_interval_ms must be > 0".into(),
            ));
        }
        if config.min_confidence < 0.0 || config.min_confidence > 1.0 {
            return Err(Error::Config(
                "min_confidence must be in [0.0, 1.0]".into(),
            ));
        }
        Ok(())
    }

    /// Detects a cascade failure from a set of affected services.
    ///
    /// A cascade failure is flagged when the cascade `depth` reaches
    /// or exceeds `cascade_depth_threshold` and enough services are
    /// involved.
    ///
    /// # Arguments
    ///
    /// * `affected` - Service identifiers involved in the cascade.
    /// * `depth` - Current cascade depth (hop count from origin).
    /// * `origin` - The originating service identifier.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if `affected` is empty or `origin` is empty.
    pub fn detect_cascade(
        &self,
        affected: &[String],
        depth: u32,
        origin: &str,
    ) -> Result<Option<EmergenceRecord>> {
        if affected.is_empty() {
            return Err(Error::Validation(
                "affected services must not be empty".into(),
            ));
        }
        if origin.is_empty() {
            return Err(Error::Validation("origin must not be empty".into()));
        }

        if depth < self.config.cascade_depth_threshold {
            return Ok(None);
        }

        // Confidence scales with depth and breadth of cascade
        #[allow(clippy::cast_precision_loss)]
        let depth_factor = f64::from(depth) / (f64::from(depth) + 2.0);
        #[allow(clippy::cast_precision_loss)]
        let breadth_factor = (affected.len() as f64) / (affected.len() as f64 + 3.0);
        let confidence = 0.5_f64
            .mul_add(depth_factor, 0.5 * breadth_factor)
            .clamp(0.0, 1.0);

        if confidence < self.config.min_confidence {
            return Ok(None);
        }

        let severity = Self::classify_severity(confidence, affected.len());

        let record = EmergenceRecord {
            id: Uuid::new_v4().to_string(),
            emergence_type: EmergenceType::CascadeFailure,
            confidence,
            severity,
            source_correlations: vec![format!("cascade:origin={origin}:depth={depth}")],
            affected_layers: Self::infer_layers_from_services(affected),
            affected_services: affected.to_vec(),
            description: format!(
                "Cascade failure detected: {count} services affected at depth {depth}, origin={origin}",
                count = affected.len()
            ),
            detected_at: Utc::now(),
            recommended_action: Some(format!(
                "Isolate origin service '{origin}' and apply circuit breaker to downstream dependencies"
            )),
        };

        self.record_emergence(&record);
        Ok(Some(record))
    }

    /// Detects a synergy shift across a set of services.
    ///
    /// A synergy shift is flagged when the absolute `delta` exceeds
    /// `synergy_delta_threshold`.
    ///
    /// # Arguments
    ///
    /// * `services` - Service identifiers exhibiting the shift.
    /// * `delta` - Magnitude of the synergy change (signed).
    /// * `pattern_id` - Correlation pattern identifier from M37.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if `services` is empty or `pattern_id` is empty.
    pub fn detect_synergy_shift(
        &self,
        services: &[String],
        delta: f64,
        pattern_id: &str,
    ) -> Result<Option<EmergenceRecord>> {
        if services.is_empty() {
            return Err(Error::Validation("services must not be empty".into()));
        }
        if pattern_id.is_empty() {
            return Err(Error::Validation("pattern_id must not be empty".into()));
        }

        let abs_delta = delta.abs();
        if abs_delta < self.config.synergy_delta_threshold {
            return Ok(None);
        }

        // Confidence grows as delta exceeds the threshold
        let excess_ratio = abs_delta / self.config.synergy_delta_threshold;
        let confidence = (excess_ratio / (excess_ratio + 1.0)).clamp(0.0, 1.0);

        if confidence < self.config.min_confidence {
            return Ok(None);
        }

        let severity = Self::classify_severity(confidence, services.len());
        let direction = if delta > 0.0 { "positive" } else { "negative" };

        let record = EmergenceRecord {
            id: Uuid::new_v4().to_string(),
            emergence_type: EmergenceType::SynergyShift,
            confidence,
            severity,
            source_correlations: vec![format!("synergy:pattern={pattern_id}:delta={delta:.4}")],
            affected_layers: Self::infer_layers_from_services(services),
            affected_services: services.to_vec(),
            description: format!(
                "Synergy {direction} shift of {abs_delta:.4} across {count} services (pattern={pattern_id})",
                count = services.len()
            ),
            detected_at: Utc::now(),
            recommended_action: if delta < 0.0 {
                Some("Investigate synergy degradation; consider pathway reinforcement".into())
            } else {
                Some("Beneficial synergy shift; consider reinforcing current topology".into())
            },
        };

        self.record_emergence(&record);
        Ok(Some(record))
    }

    /// Detects a resonance cycle from correlated layer oscillations.
    ///
    /// A resonance cycle is flagged when the observed `cycles` count
    /// meets or exceeds `resonance_min_cycles`.
    ///
    /// # Arguments
    ///
    /// * `layers` - Layer indices participating in the resonance.
    /// * `frequency_ms` - Oscillation period in milliseconds.
    /// * `cycles` - Number of observed cycles.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if `layers` is empty or `frequency_ms` is 0.
    pub fn detect_resonance(
        &self,
        layers: &[u8],
        frequency_ms: u64,
        cycles: u32,
    ) -> Result<Option<EmergenceRecord>> {
        if layers.is_empty() {
            return Err(Error::Validation("layers must not be empty".into()));
        }
        if frequency_ms == 0 {
            return Err(Error::Validation("frequency_ms must be > 0".into()));
        }

        if cycles < self.config.resonance_min_cycles {
            return Ok(None);
        }

        // Confidence rises with the number of excess cycles
        #[allow(clippy::cast_precision_loss)]
        let cycle_ratio =
            f64::from(cycles) / f64::from(self.config.resonance_min_cycles);
        let confidence = (1.0 - 1.0 / cycle_ratio).clamp(0.0, 1.0);

        if confidence < self.config.min_confidence {
            return Ok(None);
        }

        let severity = Self::classify_severity(confidence, layers.len());

        let record = EmergenceRecord {
            id: Uuid::new_v4().to_string(),
            emergence_type: EmergenceType::ResonanceCycle,
            confidence,
            severity,
            source_correlations: vec![format!(
                "resonance:freq={frequency_ms}ms:cycles={cycles}"
            )],
            affected_layers: layers.to_vec(),
            affected_services: Vec::new(),
            description: format!(
                "Resonance cycle detected: {cycles} cycles at {frequency_ms}ms across {layer_count} layers",
                layer_count = layers.len()
            ),
            detected_at: Utc::now(),
            recommended_action: Some(
                "Investigate feedback loops; consider dampening or phase-shifting oscillations".into(),
            ),
        };

        self.record_emergence(&record);
        Ok(Some(record))
    }

    /// Detects a phase transition in a named metric.
    ///
    /// A phase transition is flagged when the ratio between `new_value`
    /// and `old_value` exceeds `PHASE_TRANSITION_RATIO` (either direction).
    ///
    /// # Arguments
    ///
    /// * `metric` - The metric name that transitioned.
    /// * `old_value` - Previous metric value.
    /// * `new_value` - Current metric value.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if `metric` is empty or values are negative.
    pub fn detect_phase_transition(
        &self,
        metric: &str,
        old_value: f64,
        new_value: f64,
    ) -> Result<Option<EmergenceRecord>> {
        if metric.is_empty() {
            return Err(Error::Validation("metric must not be empty".into()));
        }
        if old_value < 0.0 || new_value < 0.0 {
            return Err(Error::Validation(
                "metric values must be non-negative".into(),
            ));
        }

        // Compute ratio; guard against division by zero
        let ratio = if old_value.abs() < f64::EPSILON {
            if new_value.abs() < f64::EPSILON {
                // Both zero: no transition
                return Ok(None);
            }
            // From zero to non-zero is always a phase transition
            PHASE_TRANSITION_RATIO + 1.0
        } else {
            new_value / old_value
        };

        let inverse_ratio = if ratio.abs() < f64::EPSILON {
            PHASE_TRANSITION_RATIO + 1.0
        } else {
            1.0 / ratio
        };

        let max_ratio = ratio.max(inverse_ratio);

        if max_ratio < PHASE_TRANSITION_RATIO {
            return Ok(None);
        }

        // Confidence scales with magnitude of the transition
        let confidence = (1.0 - PHASE_TRANSITION_RATIO / max_ratio).clamp(0.0, 1.0);

        if confidence < self.config.min_confidence {
            return Ok(None);
        }

        let severity = Self::classify_severity(confidence, 1);
        let direction = if new_value > old_value {
            "upward"
        } else {
            "downward"
        };

        let record = EmergenceRecord {
            id: Uuid::new_v4().to_string(),
            emergence_type: EmergenceType::PhaseTransition,
            confidence,
            severity,
            source_correlations: vec![format!(
                "phase:{metric}:{old_value:.4}->{new_value:.4}"
            )],
            affected_layers: Vec::new(),
            affected_services: Vec::new(),
            description: format!(
                "Phase transition in '{metric}': {old_value:.4} -> {new_value:.4} ({direction}, ratio={max_ratio:.2}x)"
            ),
            detected_at: Utc::now(),
            recommended_action: Some(format!(
                "Monitor metric '{metric}' for stabilisation; evaluate regime-specific thresholds"
            )),
        };

        self.record_emergence(&record);
        Ok(Some(record))
    }

    /// Detects a beneficial emergence (self-healing pathway).
    ///
    /// A beneficial emergence is flagged when a service recovers
    /// through a pathway faster than `BENEFICIAL_RECOVERY_CEILING_MS`.
    ///
    /// # Arguments
    ///
    /// * `service` - The service that recovered.
    /// * `pathway` - Ordered sequence of modules/steps in the recovery path.
    /// * `recovery_ms` - Time taken to recover in milliseconds.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if `service` is empty or `pathway` is empty.
    pub fn detect_beneficial_emergence(
        &self,
        service: &str,
        pathway: &[String],
        recovery_ms: u64,
    ) -> Result<Option<EmergenceRecord>> {
        if service.is_empty() {
            return Err(Error::Validation("service must not be empty".into()));
        }
        if pathway.is_empty() {
            return Err(Error::Validation("pathway must not be empty".into()));
        }

        if recovery_ms >= BENEFICIAL_RECOVERY_CEILING_MS {
            return Ok(None);
        }

        // Confidence: faster recovery = higher confidence
        #[allow(clippy::cast_precision_loss)]
        let speed_factor =
            1.0 - (recovery_ms as f64) / (BENEFICIAL_RECOVERY_CEILING_MS as f64);
        // Longer pathways with fast recovery are more impressive
        #[allow(clippy::cast_precision_loss)]
        let path_factor = (pathway.len() as f64) / (pathway.len() as f64 + 2.0);
        let confidence = 0.6_f64
            .mul_add(speed_factor, 0.4 * path_factor)
            .clamp(0.0, 1.0);

        if confidence < self.config.min_confidence {
            return Ok(None);
        }

        let severity = Self::classify_severity(confidence, 1);

        let record = EmergenceRecord {
            id: Uuid::new_v4().to_string(),
            emergence_type: EmergenceType::BeneficialEmergence,
            confidence,
            severity,
            source_correlations: vec![format!(
                "recovery:{service}:path_len={path_len}:ms={recovery_ms}",
                path_len = pathway.len()
            )],
            affected_layers: Vec::new(),
            affected_services: vec![service.to_string()],
            description: format!(
                "Beneficial emergence: service '{service}' self-healed in {recovery_ms}ms via {step_count}-step pathway",
                step_count = pathway.len()
            ),
            detected_at: Utc::now(),
            recommended_action: Some(format!(
                "Reinforce self-healing pathway: {}",
                pathway.join(" -> ")
            )),
        };

        self.record_emergence(&record);
        Ok(Some(record))
    }

    /// Analyzes a set of correlation IDs and affected layers for
    /// attractor formation patterns.
    ///
    /// An attractor is detected when multiple correlations converge
    /// on a common layer set, suggesting the system has entered a
    /// stable attractor basin.
    ///
    /// # Arguments
    ///
    /// * `correlation_ids` - IDs of correlations from M37.
    /// * `affected_layers` - Layers involved in the convergence.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if `correlation_ids` is empty.
    pub fn analyze_correlations(
        &self,
        correlation_ids: &[String],
        affected_layers: &[u8],
    ) -> Result<Option<EmergenceRecord>> {
        if correlation_ids.is_empty() {
            return Err(Error::Validation(
                "correlation_ids must not be empty".into(),
            ));
        }

        // Need at least 3 converging correlations to suggest an attractor
        if correlation_ids.len() < 3 {
            return Ok(None);
        }

        // Confidence scales with correlation count and layer coverage
        #[allow(clippy::cast_precision_loss)]
        let corr_factor =
            (correlation_ids.len() as f64) / (correlation_ids.len() as f64 + 3.0);
        #[allow(clippy::cast_precision_loss)]
        let layer_factor = if affected_layers.is_empty() {
            0.0
        } else {
            (affected_layers.len() as f64) / 6.0
        };
        let confidence = 0.6_f64
            .mul_add(corr_factor, 0.4 * layer_factor)
            .clamp(0.0, 1.0);

        if confidence < self.config.min_confidence {
            return Ok(None);
        }

        let severity = Self::classify_severity(confidence, affected_layers.len().max(1));

        let record = EmergenceRecord {
            id: Uuid::new_v4().to_string(),
            emergence_type: EmergenceType::AttractorFormation,
            confidence,
            severity,
            source_correlations: correlation_ids.to_vec(),
            affected_layers: affected_layers.to_vec(),
            affected_services: Vec::new(),
            description: format!(
                "Attractor formation: {corr_count} correlations converging across {layer_count} layers",
                corr_count = correlation_ids.len(),
                layer_count = affected_layers.len()
            ),
            detected_at: Utc::now(),
            recommended_action: Some(
                "Evaluate attractor basin stability; consider whether the attractor state is desirable".into(),
            ),
        };

        self.record_emergence(&record);
        Ok(Some(record))
    }

    /// Returns the most recent `n` emergence records.
    #[must_use]
    pub fn get_recent(&self, n: usize) -> Vec<EmergenceRecord> {
        let history = self.behavior_history.read();
        let start = history.len().saturating_sub(n);
        history.iter().skip(start).cloned().collect()
    }

    /// Returns all emergence records of the given type from current session.
    #[must_use]
    pub fn get_by_type(&self, emergence_type: EmergenceType) -> Vec<EmergenceRecord> {
        self.detected_behaviors
            .read()
            .iter()
            .filter(|r| r.emergence_type == emergence_type)
            .cloned()
            .collect()
    }

    /// Returns a specific emergence record by ID, if it exists.
    #[must_use]
    pub fn get_record(&self, id: &str) -> Option<EmergenceRecord> {
        self.detected_behaviors
            .read()
            .iter()
            .find(|r| r.id == id)
            .cloned()
    }

    /// Returns a snapshot of aggregate statistics.
    #[must_use]
    pub fn stats(&self) -> EmergenceStats {
        self.stats.read().clone()
    }

    /// Returns the number of records in the history ring buffer.
    #[must_use]
    pub fn history_len(&self) -> usize {
        self.behavior_history.read().len()
    }

    /// Returns the number of currently active monitors.
    #[must_use]
    pub fn active_monitor_count(&self) -> usize {
        self.active_monitors.read().len()
    }

    /// Clears all detected behaviors, history, monitors, and statistics.
    pub fn clear(&self) {
        self.detected_behaviors.write().clear();
        self.active_monitors.write().clear();
        self.behavior_history.write().clear();
        *self.stats.write() = EmergenceStats::default();
    }

    /// Classifies severity from confidence and the number of affected entities.
    ///
    /// Severity is computed as a weighted combination of confidence (70%)
    /// and entity breadth (30%), where entity breadth is normalized against
    /// a reference count of 6 (total layers).
    #[must_use]
    pub fn classify_severity(confidence: f64, affected_count: usize) -> f64 {
        #[allow(clippy::cast_precision_loss)]
        let breadth = (affected_count as f64 / 6.0).clamp(0.0, 1.0);
        0.7_f64.mul_add(confidence, 0.3 * breadth).clamp(0.0, 1.0)
    }

    /// Returns a reference to the immutable configuration.
    #[must_use]
    pub const fn config(&self) -> &EmergenceDetectorConfig {
        &self.config
    }

    /// Returns the effective `min_confidence`, accounting for runtime override.
    #[must_use]
    pub fn effective_min_confidence(&self) -> f64 {
        self.confidence_override
            .read()
            .unwrap_or(self.config.min_confidence)
    }

    /// Updates the minimum confidence threshold at runtime (called by RALPH mutation executor).
    ///
    /// The value is clamped to `[0.0, 1.0]`.
    pub fn update_min_confidence(&self, confidence: f64) {
        let clamped = confidence.clamp(0.0, 1.0);
        *self.confidence_override.write() = Some(clamped);
    }

    /// Returns the total number of detected emergence events.
    #[must_use]
    pub fn detected_count(&self) -> u64 {
        self.stats.read().total_detected
    }

    /// Returns the most recent `n` emergence records.
    ///
    /// Alias for [`get_recent`](Self::get_recent) used by the `ObserverLayer`
    /// coordinator in `mod.rs`.
    #[must_use]
    pub fn recent_emergences(&self, n: usize) -> Vec<EmergenceRecord> {
        self.get_recent(n)
    }

    /// Detects cascade amplification exceeding safe bounds.
    ///
    /// Flags when signal amplification across pipeline stages surpasses
    /// a threshold, indicating the cascade damping system is insufficient.
    ///
    /// # Arguments
    ///
    /// * `total_amplification` - The measured amplification factor across
    ///   all pipeline stages (e.g. 1814x).
    /// * `stage_count` - Number of stages in the cascade pipeline.
    /// * `threshold` - The maximum safe amplification factor.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if `stage_count` is 0 or `threshold` is
    /// not positive.
    pub fn detect_cascade_amplification(
        &self,
        total_amplification: f64,
        stage_count: u32,
        threshold: f64,
    ) -> Result<Option<EmergenceRecord>> {
        if stage_count == 0 {
            return Err(Error::Validation("stage_count must be > 0".into()));
        }
        if threshold <= 0.0 {
            return Err(Error::Validation("threshold must be positive".into()));
        }

        if total_amplification <= threshold {
            return Ok(None);
        }

        // Confidence scales with how far amplification exceeds the threshold
        let excess_ratio = total_amplification / threshold;
        let confidence = (1.0 - 1.0 / excess_ratio).clamp(0.0, 1.0);

        if confidence < self.config.min_confidence {
            return Ok(None);
        }

        let severity = Self::classify_severity(confidence, stage_count as usize);

        let record = EmergenceRecord {
            id: Uuid::new_v4().to_string(),
            emergence_type: EmergenceType::CascadeAmplification,
            confidence,
            severity,
            source_correlations: vec![format!(
                "cascade_amp:factor={total_amplification:.1}x:stages={stage_count}:threshold={threshold:.1}x"
            )],
            affected_layers: Vec::new(),
            affected_services: Vec::new(),
            description: format!(
                "Cascade amplification {total_amplification:.1}x across {stage_count} stages exceeds threshold {threshold:.1}x"
            ),
            detected_at: Utc::now(),
            recommended_action: Some(
                "Increase cascade damping factor or trip circuit breakers on saturated stages".into(),
            ),
        };

        self.record_emergence(&record);
        Ok(Some(record))
    }

    /// Detects thermal runaway in the V3 subsystem.
    ///
    /// Flags when system temperature exceeds the target by more than an
    /// acceptable margin, indicating the PID controller cannot maintain
    /// homeostasis.
    ///
    /// # Arguments
    ///
    /// * `current_temp` - Current system temperature [0.0, 1.0].
    /// * `target_temp` - Target temperature the PID controller aims for.
    /// * `margin` - Acceptable deviation above target before flagging.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if temperatures are outside [0.0, 1.0]
    /// or `margin` is negative.
    pub fn detect_thermal_runaway(
        &self,
        current_temp: f64,
        target_temp: f64,
        margin: f64,
    ) -> Result<Option<EmergenceRecord>> {
        if !(0.0..=1.0).contains(&current_temp) {
            return Err(Error::Validation(
                "current_temp must be in [0.0, 1.0]".into(),
            ));
        }
        if !(0.0..=1.0).contains(&target_temp) {
            return Err(Error::Validation(
                "target_temp must be in [0.0, 1.0]".into(),
            ));
        }
        if margin < 0.0 {
            return Err(Error::Validation("margin must be non-negative".into()));
        }

        let deviation = current_temp - target_temp;
        if deviation <= margin {
            return Ok(None);
        }

        // Confidence scales with how far temperature exceeds the margin
        let excess = deviation - margin;
        let max_possible = 1.0 - target_temp - margin;
        let confidence = if max_possible > 0.0 {
            (excess / max_possible).clamp(0.0, 1.0)
        } else {
            1.0
        };

        if confidence < self.config.min_confidence {
            return Ok(None);
        }

        let severity = Self::classify_severity(confidence, 1);

        let record = EmergenceRecord {
            id: Uuid::new_v4().to_string(),
            emergence_type: EmergenceType::ThermalRunaway,
            confidence,
            severity,
            source_correlations: vec![format!(
                "thermal:current={current_temp:.3}:target={target_temp:.3}:deviation={deviation:.3}"
            )],
            affected_layers: Vec::new(),
            affected_services: Vec::new(),
            description: format!(
                "Thermal runaway: temperature {current_temp:.3} exceeds target {target_temp:.3} by {deviation:.3} (margin={margin:.3})"
            ),
            detected_at: Utc::now(),
            recommended_action: Some(
                "Trigger emergency decay cycle and increase PID cooling gain".into(),
            ),
        };

        self.record_emergence(&record);
        Ok(Some(record))
    }

    /// Runs emergence detection against a batch of correlated events from M37.
    ///
    /// For each correlated event with sufficient links and confidence,
    /// this method decides which emergence type to probe and records
    /// any detections that pass the confidence threshold.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if internal detection sub-calls fail.
    pub fn detect(&self, events: &[CorrelatedEvent]) -> Result<Vec<EmergenceRecord>> {
        let mut records = Vec::new();

        for event in events {
            // Skip low-confidence correlations
            if event.confidence < self.config.min_confidence {
                continue;
            }

            // Extract affected layers from related events
            let mut layers: Vec<u8> = event
                .related_events
                .iter()
                .map(|e| e.source_layer)
                .collect();
            layers.push(event.primary_event.source_layer);
            layers.sort_unstable();
            layers.dedup();

            // Collect correlation IDs
            let correlation_ids: Vec<String> = event
                .links
                .iter()
                .map(|l| l.source_event_id.clone())
                .collect();

            // Attempt attractor detection on multi-correlation events
            if correlation_ids.len() >= 3 {
                if let Ok(Some(record)) = self.analyze_correlations(&correlation_ids, &layers) {
                    records.push(record);
                }
            }

            // Check for cascade-like patterns (many layers affected)
            if layers.len() >= 3 {
                let service_names: Vec<String> = layers
                    .iter()
                    .map(|l| format!("layer-{l}"))
                    .collect();
                #[allow(clippy::cast_possible_truncation)]
                let depth = layers.len() as u32;
                if let Ok(Some(record)) = self.detect_cascade(
                    &service_names,
                    depth,
                    &event.primary_event.event_id,
                ) {
                    records.push(record);
                }
            }

            // R13: Detect synergy shifts from correlation strength changes.
            // A high-confidence correlation with many related events suggests
            // synergy is shifting between the affected services.
            if event.confidence > 0.6 && event.related_events.len() >= 2 {
                let service_ids: Vec<String> = event
                    .related_events
                    .iter()
                    .map(|e| e.event_id.clone())
                    .collect();
                let delta = event.confidence - 0.5; // synthetic delta from confidence
                if let Ok(Some(record)) =
                    self.detect_synergy_shift(&service_ids, delta, &event.primary_event.event_id)
                {
                    records.push(record);
                }
            }

            // R13: Detect phase transitions from metric value jumps.
            // Use confidence spread across related events as a proxy
            // for regime boundary crossing.
            if event.related_events.len() >= 2 && event.confidence > 0.5 {
                // Synthetic metric: use event count as old_value, confidence as new_value
                #[allow(clippy::cast_precision_loss)]
                let related_count = event.related_events.len() as f64;
                let old_value = related_count * 0.3;
                let new_value = event.confidence * related_count;
                if old_value > 0.0 {
                    if let Ok(Some(record)) = self.detect_phase_transition(
                        &event.primary_event.event_id,
                        old_value,
                        new_value,
                    ) {
                        records.push(record);
                    }
                }
            }

            // R13: Detect resonance cycles from repeated correlation patterns.
            // If the same layers appear in multiple consecutive correlations,
            // it indicates a cyclic resonance pattern.
            if layers.len() >= 2 {
                // Use event count as cycle proxy
                #[allow(clippy::cast_possible_truncation)]
                let cycles = event.related_events.len() as u32;
                if cycles >= 2 {
                    if let Ok(Some(record)) =
                        self.detect_resonance(&layers, 1000, cycles)
                    {
                        records.push(record);
                    }
                }
            }

            // R13: Detect beneficial emergence from rapid recovery patterns.
            // If correlation confidence is high and affects few layers,
            // it may indicate a self-healing pathway.
            if event.confidence > 0.8 && layers.len() <= 2 {
                let pathway: Vec<String> = layers.iter().map(|l| format!("L{l}")).collect();
                if let Ok(Some(record)) = self.detect_beneficial_emergence(
                    &event.primary_event.event_id,
                    &pathway,
                    3000, // synthetic recovery time under ceiling
                ) {
                    records.push(record);
                }
            }
        }

        Ok(records)
    }

    // ---- Private helpers ----

    /// Records an emergence into both the `detected_behaviors` list and
    /// the bounded history ring buffer, then updates statistics.
    fn record_emergence(&self, record: &EmergenceRecord) {
        // Append to detected_behaviors
        self.detected_behaviors.write().push(record.clone());

        // Append to bounded history
        {
            let mut history = self.behavior_history.write();
            if history.len() >= self.config.history_capacity {
                history.pop_front();
            }
            history.push_back(record.clone());
        }

        // Update monitor: create or transition to Triggered
        self.update_monitor(record);

        // Update statistics
        {
            let mut stats = self.stats.write();
            stats.total_detected += 1;
            *stats
                .by_type
                .entry(record.emergence_type.to_string())
                .or_insert(0) += 1;
            let severity_class = Self::severity_class_name(record.severity);
            *stats
                .by_severity_class
                .entry(severity_class)
                .or_insert(0) += 1;
            stats.active_monitors = self.active_monitors.read().len();
            stats.detection_cycles += 1;
        }
    }

    /// Creates or updates a monitor for the given emergence type.
    fn update_monitor(&self, record: &EmergenceRecord) {
        let mut monitors = self.active_monitors.write();

        // Find existing monitor for this behavior type
        let existing_key = monitors
            .iter()
            .find(|(_, m)| m.behavior_type == record.emergence_type)
            .map(|(k, _)| k.clone());

        if let Some(key) = existing_key {
            if let Some(monitor) = monitors.get_mut(&key) {
                monitor.state = MonitorState::Triggered;
                monitor.confidence = record.confidence;
                monitor.accumulated_evidence.push(record.id.clone());
            }
        } else {
            let monitor = EmergenceMonitor {
                monitor_id: Uuid::new_v4().to_string(),
                behavior_type: record.emergence_type,
                state: MonitorState::Triggered,
                accumulated_evidence: vec![record.id.clone()],
                confidence: record.confidence,
                started_at: Utc::now(),
            };
            monitors.insert(monitor.monitor_id.clone(), monitor);
        }
    }

    /// Returns a human-readable severity class name.
    #[must_use]
    fn severity_class_name(severity: f64) -> String {
        if severity >= 0.9 {
            "critical".into()
        } else if severity >= 0.7 {
            "high".into()
        } else if severity >= 0.4 {
            "medium".into()
        } else {
            "low".into()
        }
    }

    /// Infers layer indices from service identifiers using a simple heuristic:
    /// hash each service name to a layer in [1, 6].
    #[must_use]
    fn infer_layers_from_services(services: &[String]) -> Vec<u8> {
        let mut layers: Vec<u8> = services
            .iter()
            .map(|s| {
                // Simple deterministic hash to layer
                let hash: u32 = s.bytes().fold(0u32, |acc, b| {
                    acc.wrapping_mul(31).wrapping_add(u32::from(b))
                });
                #[allow(clippy::cast_possible_truncation)]
                let layer = (hash % 6) as u8 + 1;
                layer
            })
            .collect();
        layers.sort_unstable();
        layers.dedup();
        layers
    }
}

impl Default for EmergenceDetector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    fn make_detector() -> EmergenceDetector {
        EmergenceDetector::new()
    }

    fn services(names: &[&str]) -> Vec<String> {
        names.iter().map(|s| (*s).to_string()).collect()
    }

    fn pathway(steps: &[&str]) -> Vec<String> {
        steps.iter().map(|s| (*s).to_string()).collect()
    }

    // ---- Config validation tests ----

    #[test]
    fn test_validate_config_default_ok() {
        assert!(EmergenceDetector::validate_config(&EmergenceDetectorConfig::default()).is_ok());
    }

    #[test]
    fn test_validate_config_rejects_invalid_params() {
        // Zero cascade depth
        let c1 = EmergenceDetectorConfig { cascade_depth_threshold: 0, ..Default::default() };
        assert!(EmergenceDetector::validate_config(&c1).is_err());
        // Zero synergy delta
        let c2 = EmergenceDetectorConfig { synergy_delta_threshold: 0.0, ..Default::default() };
        assert!(EmergenceDetector::validate_config(&c2).is_err());
        // Excessive synergy delta
        let c3 = EmergenceDetectorConfig { synergy_delta_threshold: 1.5, ..Default::default() };
        assert!(EmergenceDetector::validate_config(&c3).is_err());
        // Resonance cycles too low
        let c4 = EmergenceDetectorConfig { resonance_min_cycles: 1, ..Default::default() };
        assert!(EmergenceDetector::validate_config(&c4).is_err());
        // Zero history capacity
        let c5 = EmergenceDetectorConfig { history_capacity: 0, ..Default::default() };
        assert!(EmergenceDetector::validate_config(&c5).is_err());
        // Zero detection interval
        let c6 = EmergenceDetectorConfig { detection_interval_ms: 0, ..Default::default() };
        assert!(EmergenceDetector::validate_config(&c6).is_err());
        // Negative confidence
        let c7 = EmergenceDetectorConfig { min_confidence: -0.1, ..Default::default() };
        assert!(EmergenceDetector::validate_config(&c7).is_err());
        // Excessive confidence
        let c8 = EmergenceDetectorConfig { min_confidence: 1.1, ..Default::default() };
        assert!(EmergenceDetector::validate_config(&c8).is_err());
    }

    // ---- Cascade failure tests ----

    #[test]
    fn test_cascade_validation_errors() {
        let d = make_detector();
        // Empty affected services
        assert!(d.detect_cascade(&[], 5, "origin").is_err());
        // Empty origin
        assert!(d.detect_cascade(&services(&["svc-a"]), 5, "").is_err());
    }

    #[test]
    fn test_cascade_below_threshold_returns_none() {
        let d = make_detector();
        // depth=2 < default threshold=3
        let result = d.detect_cascade(&services(&["svc-a", "svc-b"]), 2, "svc-a");
        assert!(result.is_ok());
        assert!(result.unwrap_or(None).is_none());
    }

    #[test]
    fn test_cascade_at_threshold_with_many_services() {
        let d = EmergenceDetector::with_config(EmergenceDetectorConfig {
            min_confidence: 0.3,
            ..Default::default()
        });
        let svcs = services(&["svc-a", "svc-b", "svc-c", "svc-d", "svc-e"]);
        let result = d.detect_cascade(&svcs, 3, "svc-a");
        assert!(result.is_ok());
        let record = result.unwrap_or(None);
        assert!(record.is_some(), "cascade at threshold with many services should detect");
        let record = record.unwrap_or_else(|| unreachable!());
        assert_eq!(record.emergence_type, EmergenceType::CascadeFailure);
        assert!(record.confidence > 0.0);
        assert!(record.severity > 0.0);
        assert!(!record.affected_services.is_empty());
    }

    #[test]
    fn test_cascade_deep_high_confidence() {
        let d = EmergenceDetector::with_config(EmergenceDetectorConfig {
            min_confidence: 0.1,
            ..Default::default()
        });
        let svcs = services(&["a", "b", "c", "d", "e", "f"]);
        let result = d.detect_cascade(&svcs, 10, "a");
        assert!(result.is_ok());
        let record = result.unwrap_or(None);
        assert!(record.is_some());
        let r = record.unwrap_or_else(|| unreachable!());
        // Deep cascade with many services should have high confidence
        assert!(r.confidence > 0.5);
    }

    // ---- Synergy shift tests ----

    #[test]
    fn test_synergy_shift_validation_errors() {
        let d = make_detector();
        // Empty services
        assert!(d.detect_synergy_shift(&[], 0.3, "pattern-1").is_err());
        // Empty pattern_id
        assert!(d.detect_synergy_shift(&services(&["svc-a"]), 0.3, "").is_err());
    }

    #[test]
    fn test_synergy_shift_below_threshold_returns_none() {
        let d = make_detector();
        // delta=0.05 < default threshold=0.15
        let result = d.detect_synergy_shift(&services(&["svc-a"]), 0.05, "p1");
        assert!(result.is_ok());
        assert!(result.unwrap_or(None).is_none());
    }

    #[test]
    fn test_synergy_shift_positive_detection() {
        let d = EmergenceDetector::with_config(EmergenceDetectorConfig {
            min_confidence: 0.3,
            ..Default::default()
        });
        let result = d.detect_synergy_shift(&services(&["svc-a", "svc-b"]), 0.5, "p1");
        assert!(result.is_ok());
        let record = result.unwrap_or(None);
        assert!(record.is_some());
        let r = record.unwrap_or_else(|| unreachable!());
        assert_eq!(r.emergence_type, EmergenceType::SynergyShift);
        assert!(r.description.contains("positive"));
    }

    #[test]
    fn test_synergy_shift_negative_detection() {
        let d = EmergenceDetector::with_config(EmergenceDetectorConfig {
            min_confidence: 0.3,
            ..Default::default()
        });
        let result = d.detect_synergy_shift(&services(&["svc-a"]), -0.4, "p2");
        assert!(result.is_ok());
        let record = result.unwrap_or(None);
        assert!(record.is_some());
        let r = record.unwrap_or_else(|| unreachable!());
        assert!(r.description.contains("negative"));
    }

    // ---- Resonance cycle tests ----

    #[test]
    fn test_resonance_validation_errors() {
        let d = make_detector();
        // Empty layers
        assert!(d.detect_resonance(&[], 100, 5).is_err());
        // Zero frequency
        assert!(d.detect_resonance(&[1, 2], 0, 5).is_err());
    }

    #[test]
    fn test_resonance_below_min_cycles_returns_none() {
        let d = make_detector();
        // cycles=2 < default min=3
        let result = d.detect_resonance(&[1, 2], 100, 2);
        assert!(result.is_ok());
        assert!(result.unwrap_or(None).is_none());
    }

    #[test]
    fn test_resonance_detection_above_threshold() {
        let d = EmergenceDetector::with_config(EmergenceDetectorConfig {
            min_confidence: 0.3,
            ..Default::default()
        });
        let result = d.detect_resonance(&[1, 2, 3], 500, 10);
        assert!(result.is_ok());
        let record = result.unwrap_or(None);
        assert!(record.is_some());
        let r = record.unwrap_or_else(|| unreachable!());
        assert_eq!(r.emergence_type, EmergenceType::ResonanceCycle);
        assert!(!r.affected_layers.is_empty());
    }

    // ---- Phase transition tests ----

    #[test]
    fn test_phase_transition_validation_errors() {
        let d = make_detector();
        // Empty metric
        assert!(d.detect_phase_transition("", 1.0, 5.0).is_err());
        // Negative values
        assert!(d.detect_phase_transition("latency", -1.0, 5.0).is_err());
    }

    #[test]
    fn test_phase_transition_small_change_returns_none() {
        let d = make_detector();
        // 1.5x is below PHASE_TRANSITION_RATIO of 2.0x
        let result = d.detect_phase_transition("latency", 100.0, 150.0);
        assert!(result.is_ok());
        assert!(result.unwrap_or(None).is_none());
    }

    #[test]
    fn test_phase_transition_large_upward() {
        let d = EmergenceDetector::with_config(EmergenceDetectorConfig {
            min_confidence: 0.1,
            ..Default::default()
        });
        let result = d.detect_phase_transition("error_rate", 0.01, 0.1);
        assert!(result.is_ok());
        let record = result.unwrap_or(None);
        assert!(record.is_some());
        let r = record.unwrap_or_else(|| unreachable!());
        assert_eq!(r.emergence_type, EmergenceType::PhaseTransition);
        assert!(r.description.contains("upward"));
    }

    #[test]
    fn test_phase_transition_from_zero() {
        let d = EmergenceDetector::with_config(EmergenceDetectorConfig {
            min_confidence: 0.1,
            ..Default::default()
        });
        let result = d.detect_phase_transition("connections", 0.0, 5.0);
        assert!(result.is_ok());
        let record = result.unwrap_or(None);
        assert!(record.is_some());
    }

    #[test]
    fn test_phase_transition_both_zero_returns_none() {
        let d = make_detector();
        let result = d.detect_phase_transition("metric", 0.0, 0.0);
        assert!(result.is_ok());
        assert!(result.unwrap_or(None).is_none());
    }

    // ---- Beneficial emergence tests ----

    #[test]
    fn test_beneficial_validation_errors() {
        let d = make_detector();
        // Empty service
        assert!(d.detect_beneficial_emergence("", &pathway(&["step-1"]), 100).is_err());
        // Empty pathway
        assert!(d.detect_beneficial_emergence("svc-a", &[], 100).is_err());
    }

    #[test]
    fn test_beneficial_slow_recovery_returns_none() {
        let d = make_detector();
        // Recovery at ceiling or above is not considered beneficial
        let result = d.detect_beneficial_emergence(
            "svc-a",
            &pathway(&["detect", "isolate", "restart"]),
            BENEFICIAL_RECOVERY_CEILING_MS,
        );
        assert!(result.is_ok());
        assert!(result.unwrap_or(None).is_none());
    }

    #[test]
    fn test_beneficial_fast_recovery_detected() {
        let d = EmergenceDetector::with_config(EmergenceDetectorConfig {
            min_confidence: 0.3,
            ..Default::default()
        });
        let path = pathway(&["detect", "isolate", "restart", "verify"]);
        let result = d.detect_beneficial_emergence("svc-a", &path, 200);
        assert!(result.is_ok());
        let record = result.unwrap_or(None);
        assert!(record.is_some());
        let r = record.unwrap_or_else(|| unreachable!());
        assert_eq!(r.emergence_type, EmergenceType::BeneficialEmergence);
        assert!(r.description.contains("self-healed"));
    }

    // ---- Analyze correlations / attractor tests ----

    #[test]
    fn test_analyze_correlations_empty_fails() {
        let d = make_detector();
        let result = d.analyze_correlations(&[], &[1, 2]);
        assert!(result.is_err());
    }

    #[test]
    fn test_analyze_correlations_too_few_returns_none() {
        let d = make_detector();
        let ids = services(&["c1", "c2"]);
        let result = d.analyze_correlations(&ids, &[1, 2, 3]);
        assert!(result.is_ok());
        assert!(result.unwrap_or(None).is_none());
    }

    #[test]
    fn test_analyze_correlations_attractor_detected() {
        let d = EmergenceDetector::with_config(EmergenceDetectorConfig {
            min_confidence: 0.3,
            ..Default::default()
        });
        let ids = services(&["c1", "c2", "c3", "c4", "c5"]);
        let result = d.analyze_correlations(&ids, &[1, 2, 3, 4]);
        assert!(result.is_ok());
        let record = result.unwrap_or(None);
        assert!(record.is_some());
        let r = record.unwrap_or_else(|| unreachable!());
        assert_eq!(r.emergence_type, EmergenceType::AttractorFormation);
    }

    // ---- Severity classification tests ----

    #[test]
    fn test_classify_severity_ranges() {
        // Zero confidence yields low severity
        let s0 = EmergenceDetector::classify_severity(0.0, 1);
        assert!(s0 >= 0.0 && s0 <= 1.0);
        // Full confidence, one entity
        let s1 = EmergenceDetector::classify_severity(1.0, 1);
        assert!(s1 > 0.7);
        // Full confidence, all 6 layers -> 1.0
        let s6 = EmergenceDetector::classify_severity(1.0, 6);
        assert!((s6 - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_classify_severity_clamped_above_six() {
        // Breadth is clamped to 1.0 even with > 6 entities
        let s = EmergenceDetector::classify_severity(1.0, 12);
        assert!((s - 1.0).abs() < 1e-10);
    }

    // ---- Retrieval and state tests ----

    #[test]
    fn test_new_detector_empty_state_and_queries() {
        let d = make_detector();
        assert_eq!(d.history_len(), 0);
        assert_eq!(d.active_monitor_count(), 0);
        assert_eq!(d.stats().total_detected, 0);
        // All retrieval methods return empty on a fresh detector
        assert!(d.get_recent(5).is_empty());
        assert!(d.get_by_type(EmergenceType::CascadeFailure).is_empty());
        assert!(d.get_record("nonexistent").is_none());
    }

    #[test]
    fn test_get_record_found() {
        let d = EmergenceDetector::with_config(EmergenceDetectorConfig {
            min_confidence: 0.1,
            ..Default::default()
        });
        let svcs = services(&["a", "b", "c", "d"]);
        let result = d.detect_cascade(&svcs, 5, "a");
        let record = result.unwrap_or(None).unwrap_or_else(|| unreachable!());
        let id = record.id.clone();
        let found = d.get_record(&id);
        assert!(found.is_some());
        let found = found.unwrap_or_else(|| unreachable!());
        assert_eq!(found.id, id);
    }

    #[test]
    fn test_get_by_type_filters_correctly() {
        let d = EmergenceDetector::with_config(EmergenceDetectorConfig {
            min_confidence: 0.1,
            ..Default::default()
        });
        let svcs = services(&["a", "b", "c", "d"]);
        let _r = d.detect_cascade(&svcs, 5, "a");
        let _r = d.detect_phase_transition("metric", 1.0, 10.0);

        let cascades = d.get_by_type(EmergenceType::CascadeFailure);
        assert_eq!(cascades.len(), 1);
        let phases = d.get_by_type(EmergenceType::PhaseTransition);
        assert_eq!(phases.len(), 1);
        let resonances = d.get_by_type(EmergenceType::ResonanceCycle);
        assert!(resonances.is_empty());
    }

    #[test]
    fn test_get_recent_returns_most_recent() {
        let d = EmergenceDetector::with_config(EmergenceDetectorConfig {
            min_confidence: 0.1,
            ..Default::default()
        });
        let svcs = services(&["a", "b", "c", "d"]);
        for _ in 0..5 {
            let _r = d.detect_cascade(&svcs, 5, "a");
        }
        let recent = d.get_recent(3);
        assert_eq!(recent.len(), 3);
    }

    #[test]
    fn test_history_capacity_enforcement() {
        let d = EmergenceDetector::with_config(EmergenceDetectorConfig {
            history_capacity: 5,
            min_confidence: 0.1,
            ..Default::default()
        });
        let svcs = services(&["a", "b", "c", "d"]);
        for _ in 0..10 {
            let _r = d.detect_cascade(&svcs, 5, "a");
        }
        assert_eq!(d.history_len(), 5);
    }

    #[test]
    fn test_clear_resets_all_state() {
        let d = EmergenceDetector::with_config(EmergenceDetectorConfig {
            min_confidence: 0.1,
            ..Default::default()
        });
        let svcs = services(&["a", "b", "c"]);
        let _r = d.detect_cascade(&svcs, 5, "a");
        assert!(d.history_len() > 0);
        assert!(d.active_monitor_count() > 0);

        d.clear();
        assert_eq!(d.history_len(), 0);
        assert_eq!(d.active_monitor_count(), 0);
        assert_eq!(d.stats().total_detected, 0);
    }

    // ---- Statistics tests ----

    #[test]
    fn test_stats_increment_total() {
        let d = EmergenceDetector::with_config(EmergenceDetectorConfig {
            min_confidence: 0.1,
            ..Default::default()
        });
        let svcs = services(&["a", "b", "c", "d"]);
        let _r = d.detect_cascade(&svcs, 5, "a");
        let _r = d.detect_cascade(&svcs, 6, "b");
        assert_eq!(d.stats().total_detected, 2);
    }

    #[test]
    fn test_stats_by_type() {
        let d = EmergenceDetector::with_config(EmergenceDetectorConfig {
            min_confidence: 0.1,
            ..Default::default()
        });
        let svcs = services(&["a", "b", "c", "d"]);
        let _r = d.detect_cascade(&svcs, 5, "a");
        let _r = d.detect_phase_transition("m", 1.0, 20.0);
        let stats = d.stats();
        assert_eq!(
            *stats.by_type.get("cascade_failure").unwrap_or(&0),
            1
        );
        assert_eq!(
            *stats.by_type.get("phase_transition").unwrap_or(&0),
            1
        );
    }

    #[test]
    fn test_stats_by_severity_class() {
        let d = EmergenceDetector::with_config(EmergenceDetectorConfig {
            min_confidence: 0.1,
            ..Default::default()
        });
        let svcs = services(&["a", "b", "c", "d"]);
        let _r = d.detect_cascade(&svcs, 5, "a");
        let stats = d.stats();
        let total_severity_entries: u64 = stats.by_severity_class.values().sum();
        assert_eq!(total_severity_entries, 1);
    }

    // ---- Monitor state tests ----

    #[test]
    fn test_monitor_created_on_detection() {
        let d = EmergenceDetector::with_config(EmergenceDetectorConfig {
            min_confidence: 0.1,
            ..Default::default()
        });
        assert_eq!(d.active_monitor_count(), 0);
        let svcs = services(&["a", "b", "c", "d"]);
        let _r = d.detect_cascade(&svcs, 5, "a");
        assert!(d.active_monitor_count() > 0);
    }

    #[test]
    fn test_monitor_reused_for_same_type() {
        let d = EmergenceDetector::with_config(EmergenceDetectorConfig {
            min_confidence: 0.1,
            ..Default::default()
        });
        let svcs = services(&["a", "b", "c", "d"]);
        let _r = d.detect_cascade(&svcs, 5, "a");
        let _r = d.detect_cascade(&svcs, 6, "b");
        // Only one monitor for CascadeFailure
        let monitors = d.active_monitors.read();
        let cascade_monitors: Vec<_> = monitors
            .values()
            .filter(|m| m.behavior_type == EmergenceType::CascadeFailure)
            .collect();
        assert_eq!(cascade_monitors.len(), 1);
        // Should have 2 evidence entries
        assert_eq!(cascade_monitors[0].accumulated_evidence.len(), 2);
    }

    #[test]
    fn test_monitor_triggered_state() {
        let d = EmergenceDetector::with_config(EmergenceDetectorConfig {
            min_confidence: 0.1,
            ..Default::default()
        });
        let svcs = services(&["a", "b", "c", "d"]);
        let _r = d.detect_cascade(&svcs, 5, "a");
        let monitors = d.active_monitors.read();
        let monitor = monitors.values().next();
        assert!(monitor.is_some());
        let m = monitor.unwrap_or_else(|| unreachable!());
        assert_eq!(m.state, MonitorState::Triggered);
    }

    // ---- Thread safety tests ----

    #[test]
    fn test_concurrent_cascade_detection() {
        let d = Arc::new(EmergenceDetector::with_config(EmergenceDetectorConfig {
            min_confidence: 0.1,
            ..Default::default()
        }));
        let mut handles = Vec::new();

        for t in 0..4 {
            let d_clone = Arc::clone(&d);
            handles.push(thread::spawn(move || {
                for i in 0..5 {
                    let svcs = services(&["a", "b", "c", "d"]);
                    let _r = d_clone.detect_cascade(
                        &svcs,
                        5,
                        &format!("origin-t{t}-{i}"),
                    );
                }
            }));
        }

        for handle in handles {
            let _r = handle.join();
        }

        assert_eq!(d.stats().total_detected, 20);
    }

    #[test]
    fn test_concurrent_mixed_detection() {
        let d = Arc::new(EmergenceDetector::with_config(EmergenceDetectorConfig {
            min_confidence: 0.1,
            ..Default::default()
        }));
        let mut handles = Vec::new();

        // Thread 1: cascades
        {
            let d_clone = Arc::clone(&d);
            handles.push(thread::spawn(move || {
                let svcs = services(&["a", "b", "c", "d"]);
                for _ in 0..5 {
                    let _r = d_clone.detect_cascade(&svcs, 5, "origin");
                }
            }));
        }

        // Thread 2: phase transitions
        {
            let d_clone = Arc::clone(&d);
            handles.push(thread::spawn(move || {
                for _ in 0..5 {
                    let _r = d_clone.detect_phase_transition("m", 1.0, 20.0);
                }
            }));
        }

        // Thread 3: reads
        {
            let d_clone = Arc::clone(&d);
            handles.push(thread::spawn(move || {
                for _ in 0..10 {
                    let _s = d_clone.stats();
                    let _h = d_clone.history_len();
                    let _r = d_clone.get_recent(5);
                }
            }));
        }

        for handle in handles {
            let _r = handle.join();
        }

        assert_eq!(d.stats().total_detected, 10);
    }

    // ---- Edge case tests ----

    #[test]
    fn test_emergence_type_display() {
        assert_eq!(EmergenceType::CascadeFailure.to_string(), "cascade_failure");
        assert_eq!(EmergenceType::SynergyShift.to_string(), "synergy_shift");
        assert_eq!(EmergenceType::ResonanceCycle.to_string(), "resonance_cycle");
        assert_eq!(EmergenceType::AttractorFormation.to_string(), "attractor_formation");
        assert_eq!(EmergenceType::PhaseTransition.to_string(), "phase_transition");
        assert_eq!(EmergenceType::BeneficialEmergence.to_string(), "beneficial_emergence");
        assert_eq!(EmergenceType::CascadeAmplification.to_string(), "cascade_amplification");
        assert_eq!(EmergenceType::ThermalRunaway.to_string(), "thermal_runaway");
    }

    #[test]
    fn test_config_accessor_and_default() {
        let d = make_detector();
        assert_eq!(d.config().cascade_depth_threshold, DEFAULT_CASCADE_DEPTH_THRESHOLD);
        assert!((d.config().synergy_delta_threshold - DEFAULT_SYNERGY_DELTA_THRESHOLD).abs() < 1e-10);
        assert_eq!(d.config().resonance_min_cycles, DEFAULT_RESONANCE_MIN_CYCLES);
        assert_eq!(d.config().history_capacity, DEFAULT_HISTORY_CAPACITY);
        assert_eq!(d.config().detection_interval_ms, DEFAULT_DETECTION_INTERVAL_MS);
        assert!((d.config().min_confidence - DEFAULT_MIN_CONFIDENCE).abs() < 1e-10);
        // Default trait produces equivalent state
        let d2 = EmergenceDetector::default();
        assert_eq!(d2.history_len(), 0);
        assert_eq!(d2.active_monitor_count(), 0);
    }

    #[test]
    fn test_record_has_uuid() {
        let d = EmergenceDetector::with_config(EmergenceDetectorConfig {
            min_confidence: 0.1,
            ..Default::default()
        });
        let svcs = services(&["a", "b", "c", "d"]);
        let result = d.detect_cascade(&svcs, 5, "a");
        let record = result.unwrap_or(None).unwrap_or_else(|| unreachable!());
        assert!(record.id.contains('-'), "expected UUID format");
    }

    #[test]
    fn test_record_timestamp_is_recent() {
        let before = Utc::now();
        let d = EmergenceDetector::with_config(EmergenceDetectorConfig {
            min_confidence: 0.1,
            ..Default::default()
        });
        let svcs = services(&["a", "b", "c", "d"]);
        let result = d.detect_cascade(&svcs, 5, "a");
        let after = Utc::now();
        let record = result.unwrap_or(None).unwrap_or_else(|| unreachable!());
        assert!(record.detected_at >= before);
        assert!(record.detected_at <= after);
    }

    #[test]
    fn test_confidence_always_in_range() {
        let d = EmergenceDetector::with_config(EmergenceDetectorConfig {
            min_confidence: 0.0,
            ..Default::default()
        });

        // Generate a variety of detections
        let svcs = services(&["a", "b", "c"]);
        let _r = d.detect_cascade(&svcs, 3, "a");
        let _r = d.detect_synergy_shift(&svcs, 0.5, "p1");
        let _r = d.detect_resonance(&[1, 2], 100, 5);
        let _r = d.detect_phase_transition("m", 1.0, 10.0);
        let _r = d.detect_beneficial_emergence("a", &pathway(&["step"]), 100);

        let all = d.get_recent(100);
        for record in &all {
            assert!(
                record.confidence >= 0.0 && record.confidence <= 1.0,
                "confidence out of range: {}",
                record.confidence
            );
            assert!(
                record.severity >= 0.0 && record.severity <= 1.0,
                "severity out of range: {}",
                record.severity
            );
        }
    }

    #[test]
    fn test_infer_layers_deterministic_and_deduplicates() {
        // Deterministic: same input -> same output
        let svcs = services(&["synthex", "nais", "san-k7"]);
        let layers1 = EmergenceDetector::infer_layers_from_services(&svcs);
        let layers2 = EmergenceDetector::infer_layers_from_services(&svcs);
        assert_eq!(layers1, layers2);
        // All layers in [1, 6]
        for &l in &layers1 {
            assert!(l >= 1 && l <= 6);
        }
        // Deduplication: no duplicate layer values
        let svcs2 = services(&["a", "a_clone"]);
        let layers = EmergenceDetector::infer_layers_from_services(&svcs2);
        let mut sorted = layers.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(layers.len(), sorted.len());
    }

    #[test]
    fn test_cascade_recommended_action_mentions_origin() {
        let d = EmergenceDetector::with_config(EmergenceDetectorConfig {
            min_confidence: 0.1,
            ..Default::default()
        });
        let svcs = services(&["alpha", "beta", "gamma", "delta"]);
        let result = d.detect_cascade(&svcs, 5, "alpha");
        let record = result.unwrap_or(None).unwrap_or_else(|| unreachable!());
        let action = record.recommended_action.unwrap_or_default();
        assert!(action.contains("alpha"), "action should reference origin service");
    }

    #[test]
    fn test_synergy_shift_severity_scales_with_services() {
        let d = EmergenceDetector::with_config(EmergenceDetectorConfig {
            min_confidence: 0.1,
            ..Default::default()
        });
        let small = services(&["a"]);
        let large = services(&["a", "b", "c", "d", "e", "f"]);
        let r1 = d.detect_synergy_shift(&small, 0.8, "p1").unwrap_or(None);
        let r2 = d.detect_synergy_shift(&large, 0.8, "p2").unwrap_or(None);
        assert!(r1.is_some());
        assert!(r2.is_some());
        let s1 = r1.unwrap_or_else(|| unreachable!()).severity;
        let s2 = r2.unwrap_or_else(|| unreachable!()).severity;
        // More affected services should yield higher severity
        assert!(s2 >= s1, "larger service set should have higher severity: {s1} vs {s2}");
    }

    #[test]
    fn test_resonance_confidence_increases_with_cycles() {
        let d = EmergenceDetector::with_config(EmergenceDetectorConfig {
            min_confidence: 0.0,
            ..Default::default()
        });
        let r_low = d.detect_resonance(&[1, 2], 100, 4).unwrap_or(None);
        let r_high = d.detect_resonance(&[1, 2], 100, 100).unwrap_or(None);
        assert!(r_low.is_some());
        assert!(r_high.is_some());
        let c_low = r_low.unwrap_or_else(|| unreachable!()).confidence;
        let c_high = r_high.unwrap_or_else(|| unreachable!()).confidence;
        assert!(c_high > c_low, "more cycles should yield higher confidence: {c_low} vs {c_high}");
    }

    #[test]
    fn test_beneficial_faster_recovery_higher_confidence() {
        let d = EmergenceDetector::with_config(EmergenceDetectorConfig {
            min_confidence: 0.0,
            ..Default::default()
        });
        let path = pathway(&["detect", "fix", "verify"]);
        let r_slow = d.detect_beneficial_emergence("svc", &path, 4000).unwrap_or(None);
        let r_fast = d.detect_beneficial_emergence("svc", &path, 100).unwrap_or(None);
        assert!(r_slow.is_some());
        assert!(r_fast.is_some());
        let c_slow = r_slow.unwrap_or_else(|| unreachable!()).confidence;
        let c_fast = r_fast.unwrap_or_else(|| unreachable!()).confidence;
        assert!(c_fast > c_slow, "faster recovery should yield higher confidence: {c_slow} vs {c_fast}");
    }

    // ---- Cascade amplification tests (V3 integration) ----

    #[test]
    fn test_cascade_amplification_validation_errors() {
        let d = make_detector();
        // Zero stage_count
        assert!(d.detect_cascade_amplification(100.0, 0, 10.0).is_err());
        // Non-positive threshold
        assert!(d.detect_cascade_amplification(100.0, 12, 0.0).is_err());
        assert!(d.detect_cascade_amplification(100.0, 12, -1.0).is_err());
    }

    #[test]
    fn test_cascade_amplification_below_threshold_returns_none() {
        let d = make_detector();
        // Amplification of 5x is below 500x threshold
        let result = d.detect_cascade_amplification(5.0, 12, 500.0);
        assert!(result.is_ok());
        assert!(result.unwrap_or(None).is_none());
    }

    #[test]
    fn test_cascade_amplification_detected() {
        let d = EmergenceDetector::with_config(EmergenceDetectorConfig {
            min_confidence: 0.1,
            ..Default::default()
        });
        // 1814x amplification exceeds 500x threshold
        let result = d.detect_cascade_amplification(1814.0, 12, 500.0);
        assert!(result.is_ok());
        let record = result.unwrap_or(None);
        assert!(record.is_some(), "cascade amplification above threshold should detect");
        let r = record.unwrap_or_else(|| unreachable!());
        assert_eq!(r.emergence_type, EmergenceType::CascadeAmplification);
        assert!(r.confidence > 0.5);
        assert!(r.description.contains("1814"));
    }

    // ---- Thermal runaway tests (V3 integration) ----

    #[test]
    fn test_thermal_runaway_validation_errors() {
        let d = make_detector();
        // Temperature out of range
        assert!(d.detect_thermal_runaway(1.5, 0.5, 0.1).is_err());
        assert!(d.detect_thermal_runaway(-0.1, 0.5, 0.1).is_err());
        assert!(d.detect_thermal_runaway(0.9, 1.5, 0.1).is_err());
        // Negative margin
        assert!(d.detect_thermal_runaway(0.9, 0.5, -0.1).is_err());
    }

    #[test]
    fn test_thermal_runaway_within_margin_returns_none() {
        let d = make_detector();
        // 0.55 is only 0.05 above target 0.50, within 0.1 margin
        let result = d.detect_thermal_runaway(0.55, 0.50, 0.1);
        assert!(result.is_ok());
        assert!(result.unwrap_or(None).is_none());
    }

    #[test]
    fn test_thermal_runaway_detected() {
        let d = EmergenceDetector::with_config(EmergenceDetectorConfig {
            min_confidence: 0.1,
            ..Default::default()
        });
        // Temperature 0.95, target 0.50, margin 0.1 -> deviation 0.45 > 0.1
        let result = d.detect_thermal_runaway(0.95, 0.50, 0.10);
        assert!(result.is_ok());
        let record = result.unwrap_or(None);
        assert!(record.is_some(), "thermal runaway above margin should detect");
        let r = record.unwrap_or_else(|| unreachable!());
        assert_eq!(r.emergence_type, EmergenceType::ThermalRunaway);
        assert!(r.confidence > 0.5);
        assert!(r.description.contains("0.950"));
        let action = r.recommended_action.unwrap_or_default();
        assert!(action.contains("decay"), "should recommend decay cycle");
    }
}
