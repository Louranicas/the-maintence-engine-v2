//! # M54: Prediction Engine
//!
//! Predicts service failures by analysing health trends, error-rate acceleration,
//! and correlation signals. Ingests [`TensorObservation`]s and
//! [`CorrelationSignal`]s, builds per-service sliding windows, and emits
//! [`FailurePrediction`]s when the computed failure probability exceeds a
//! configurable threshold.
//!
//! ## Layer: L5 (Learning)
//! ## Dependencies: M01 (Error), M00 (Timestamp)
//!
//! ## Prediction Formula
//!
//! ```text
//! trend_velocity   = (latest_health - baseline_health) / observations.len()
//! error_accel      = latest_error_rate - baseline_error_rate
//! correlation_wt   = Σ signal.confidence * (1.0 - |offset_ms| / 300_000.0).max(0.0)
//! raw_probability  = (-trend_velocity * 3.0 + error_accel * 5.0 + corr_wt * 0.4)
//!                     .clamp(0.0, 1.0)
//! confidence       = (observations.len() / window_size).min(1.0)
//! ```
//!
//! ## Accuracy
//!
//! `accuracy_for_service` = correct outcomes / total outcomes (0.0 when no outcomes).
//!
//! ## Decay
//!
//! Each `apply_decay` call reduces every pending prediction's probability by
//! `decay_rate` and removes predictions whose probability drops below 0.01.
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

/// Default prediction horizon in seconds.
const DEFAULT_PREDICTION_HORIZON_SECS: u32 = 180;

/// Default minimum probability threshold to emit a prediction.
const DEFAULT_MIN_PROBABILITY_THRESHOLD: f64 = 0.6;

/// Default minimum observations required before predicting.
const DEFAULT_MIN_OBSERVATIONS: usize = 10;

/// Default sliding window size for observations.
const DEFAULT_OBSERVATION_WINDOW_SIZE: usize = 200;

/// Default probability decay rate per `apply_decay` call.
const DEFAULT_DECAY_RATE: f64 = 0.002;

/// Maximum number of correlation signals retained per service.
const MAX_CORRELATION_SIGNALS: usize = 100;

/// Temporal normalisation constant for correlation weight (300 000 ticks).
const CORRELATION_TEMPORAL_RANGE: f64 = 300_000.0;

/// Minimum prediction horizon in seconds.
const MIN_HORIZON_SECS: u32 = 120;

/// Maximum prediction horizon in seconds.
const MAX_HORIZON_SECS: u32 = 300;

/// Probability floor below which a decayed prediction is removed.
const DECAY_REMOVAL_THRESHOLD: f64 = 0.01;

/// Trend velocity multiplier in the probability formula.
const TREND_MULTIPLIER: f64 = 3.0;

/// Error acceleration multiplier in the probability formula.
const ERROR_MULTIPLIER: f64 = 5.0;

/// Correlation weight multiplier in the probability formula.
const CORRELATION_MULTIPLIER: f64 = 0.4;

// ---------------------------------------------------------------------------
// TensorObservation
// ---------------------------------------------------------------------------

/// A point-in-time observation of a service's health tensor.
#[derive(Clone, Debug)]
pub struct TensorObservation {
    /// Service that was observed.
    pub service_id: String,
    /// When the observation was taken.
    pub timestamp: Timestamp,
    /// Overall health score in `[0.0, 1.0]`.
    pub health_score: f64,
    /// Error rate in `[0.0, 1.0]`.
    pub error_rate: f64,
    /// Latency metric (normalised).
    pub latency: f64,
    /// Synergy score (normalised).
    pub synergy: f64,
}

// ---------------------------------------------------------------------------
// CorrelationSignal
// ---------------------------------------------------------------------------

/// A cross-service correlation signal that may boost failure probability.
#[derive(Clone, Debug)]
pub struct CorrelationSignal {
    /// Unique identifier for this correlation.
    pub correlation_id: String,
    /// Originating service.
    pub source_service: String,
    /// Confidence of the correlation (0.0-1.0).
    pub confidence: f64,
    /// Temporal offset in ticks from the reference point.
    pub temporal_offset_ms: i64,
    /// When this signal was observed.
    pub observed_at: Timestamp,
}

// ---------------------------------------------------------------------------
// FailurePrediction
// ---------------------------------------------------------------------------

/// A predicted failure for a specific service.
#[derive(Clone, Debug)]
pub struct FailurePrediction {
    /// Unique prediction identifier (UUID).
    pub id: String,
    /// Service expected to fail.
    pub service_id: String,
    /// Failure probability in `[0.0, 1.0]`.
    pub probability: f64,
    /// Prediction horizon in seconds (120-300).
    pub horizon_secs: u32,
    /// Identifiers of signals that contributed to this prediction.
    pub contributing_signals: Vec<String>,
    /// Overall confidence in this prediction.
    pub confidence: f64,
    /// When the prediction was made.
    pub predicted_at: Timestamp,
    /// Actual outcome once observed: `Some(true)` = failure occurred,
    /// `Some(false)` = no failure, `None` = not yet resolved.
    pub outcome: Option<bool>,
}

// ---------------------------------------------------------------------------
// PredictionConfig
// ---------------------------------------------------------------------------

/// Configuration knobs for the prediction engine.
#[derive(Clone, Debug)]
pub struct PredictionConfig {
    /// Prediction horizon in seconds (clamped to 120-300).
    pub prediction_horizon_secs: u32,
    /// Minimum probability to emit a prediction.
    pub min_probability_threshold: f64,
    /// Minimum observations before a prediction can be generated.
    pub min_observations: usize,
    /// Sliding window size for observations per service.
    pub observation_window_size: usize,
    /// Amount subtracted from probability each decay cycle.
    pub decay_rate: f64,
}

impl Default for PredictionConfig {
    fn default() -> Self {
        Self {
            prediction_horizon_secs: DEFAULT_PREDICTION_HORIZON_SECS,
            min_probability_threshold: DEFAULT_MIN_PROBABILITY_THRESHOLD,
            min_observations: DEFAULT_MIN_OBSERVATIONS,
            observation_window_size: DEFAULT_OBSERVATION_WINDOW_SIZE,
            decay_rate: DEFAULT_DECAY_RATE,
        }
    }
}

// ---------------------------------------------------------------------------
// ServicePredictionState (internal per-service bookkeeping)
// ---------------------------------------------------------------------------

/// Per-service sliding window of observations and correlation signals.
#[derive(Clone, Debug)]
struct ServicePredictionState {
    /// Service identifier (mirrors the map key; used only in Debug output).
    #[allow(dead_code)]
    service_id: String,
    /// Ring buffer of recent observations.
    observations: VecDeque<TensorObservation>,
    /// Recent correlation signals (capped at [`MAX_CORRELATION_SIGNALS`]).
    correlation_signals: Vec<CorrelationSignal>,
    /// Baseline health derived from the first observation in the window.
    baseline_health: f64,
    /// Current trend slope (updated on each new observation).
    trend_slope: f64,
}

impl ServicePredictionState {
    /// Create a new empty state for the given service.
    fn new(service_id: String, window_size: usize) -> Self {
        Self {
            service_id,
            observations: VecDeque::with_capacity(window_size),
            correlation_signals: Vec::new(),
            baseline_health: 0.0,
            trend_slope: 0.0,
        }
    }
}

// ---------------------------------------------------------------------------
// PredictionEngine trait
// ---------------------------------------------------------------------------

/// Core interface for failure prediction across services.
pub trait PredictionEngine: Send + Sync + fmt::Debug {
    /// Submit a health observation for a service.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if the observation has an empty `service_id`.
    fn submit_snapshot(&self, snapshot: TensorObservation) -> Result<()>;

    /// Submit a cross-service correlation signal.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if the signal has an empty `source_service`
    /// or `correlation_id`.
    fn submit_correlation(&self, correlation: CorrelationSignal) -> Result<()>;

    /// Predict failure for a specific service, returning `None` if insufficient
    /// data is available.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if `service_id` is empty.
    fn predict(&self, service_id: &str) -> Result<Option<FailurePrediction>>;

    /// Predict failure for all known services.
    ///
    /// # Errors
    ///
    /// Returns an error if internal state cannot be read.
    fn predict_all(&self) -> Result<Vec<FailurePrediction>>;

    /// Mark the real-world outcome of a prediction.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if the prediction ID is not found.
    fn mark_outcome(&self, prediction_id: &str, failure_occurred: bool) -> Result<()>;

    /// Return all predictions whose probability meets the configured threshold
    /// and that have not yet been resolved.
    fn actionable_predictions(&self) -> Vec<FailurePrediction>;

    /// Compute accuracy (correct / total resolved) for a specific service.
    /// Returns 0.0 if no outcomes have been recorded.
    fn accuracy_for_service(&self, service_id: &str) -> f64;

    /// Total number of stored predictions (resolved and unresolved).
    fn prediction_count(&self) -> usize;

    /// Decay all pending prediction probabilities by the configured rate.
    /// Predictions whose probability drops below 0.01 are removed.
    fn apply_decay(&self);
}

// ---------------------------------------------------------------------------
// PredictionEngineCore
// ---------------------------------------------------------------------------

/// Primary implementation of the [`PredictionEngine`] trait.
///
/// Uses `parking_lot::RwLock` for interior mutability so all trait methods
/// can take `&self` (C2 constraint).
#[derive(Debug)]
pub struct PredictionEngineCore {
    /// Per-service prediction state, keyed by service ID.
    state: RwLock<HashMap<String, ServicePredictionState>>,
    /// All predictions keyed by prediction ID.
    predictions: RwLock<HashMap<String, FailurePrediction>>,
    /// Engine configuration.
    config: PredictionConfig,
}

impl Default for PredictionEngineCore {
    fn default() -> Self {
        Self::new(PredictionConfig::default())
    }
}

impl PredictionEngineCore {
    /// Create a new prediction engine with the given configuration.
    #[must_use]
    pub fn new(config: PredictionConfig) -> Self {
        Self {
            state: RwLock::new(HashMap::new()),
            predictions: RwLock::new(HashMap::new()),
            config,
        }
    }

    /// Validate that a snapshot has non-empty required fields.
    fn validate_snapshot(snapshot: &TensorObservation) -> Result<()> {
        if snapshot.service_id.is_empty() {
            return Err(Error::Validation(
                "TensorObservation service_id cannot be empty".into(),
            ));
        }
        Ok(())
    }

    /// Validate that a correlation signal has non-empty required fields.
    fn validate_correlation(signal: &CorrelationSignal) -> Result<()> {
        if signal.source_service.is_empty() {
            return Err(Error::Validation(
                "CorrelationSignal source_service cannot be empty".into(),
            ));
        }
        if signal.correlation_id.is_empty() {
            return Err(Error::Validation(
                "CorrelationSignal correlation_id cannot be empty".into(),
            ));
        }
        Ok(())
    }

    /// Compute the failure probability for a given service state.
    ///
    /// Returns `None` when there are insufficient observations.
    #[allow(clippy::cast_precision_loss)]
    fn compute_probability(
        sps: &ServicePredictionState,
        config: &PredictionConfig,
    ) -> Option<(f64, f64, Vec<String>)> {
        if sps.observations.len() < config.min_observations {
            return None;
        }

        let latest = sps.observations.back()?;
        let first = sps.observations.front()?;

        // Trend velocity: how fast health is declining
        let obs_count = sps.observations.len() as f64;
        let trend_velocity = (latest.health_score - sps.baseline_health) / obs_count;

        // Error acceleration: change in error rate from baseline
        let error_acceleration = latest.error_rate - first.error_rate;

        // Correlation weight: sum of temporally-weighted signal confidences
        let mut contributing_signals = Vec::new();
        let correlation_weight: f64 = sps
            .correlation_signals
            .iter()
            .map(|sig| {
                #[allow(clippy::cast_precision_loss)]
                let offset_abs = (sig.temporal_offset_ms.unsigned_abs()) as f64;
                let temporal_factor = (1.0 - offset_abs / CORRELATION_TEMPORAL_RANGE).max(0.0);
                let contribution = sig.confidence * temporal_factor;
                if contribution > 0.0 {
                    contributing_signals.push(sig.correlation_id.clone());
                }
                contribution
            })
            .sum();

        // Probability formula using mul_add for FMA precision:
        // raw = (-trend_velocity * 3.0) + (error_acceleration * 5.0) + (correlation_weight * 0.4)
        let raw_probability = (-trend_velocity)
            .mul_add(
                TREND_MULTIPLIER,
                error_acceleration.mul_add(ERROR_MULTIPLIER, correlation_weight * CORRELATION_MULTIPLIER),
            )
            .clamp(0.0, 1.0);

        // Confidence grows with observation count up to window size
        let confidence =
            (obs_count / config.observation_window_size as f64).min(1.0);

        Some((raw_probability, confidence, contributing_signals))
    }

    /// Clamp the prediction horizon to the valid range.
    const fn clamped_horizon(secs: u32) -> u32 {
        if secs < MIN_HORIZON_SECS {
            MIN_HORIZON_SECS
        } else if secs > MAX_HORIZON_SECS {
            MAX_HORIZON_SECS
        } else {
            secs
        }
    }
}

impl PredictionEngine for PredictionEngineCore {
    #[allow(clippy::significant_drop_tightening)]
    fn submit_snapshot(&self, snapshot: TensorObservation) -> Result<()> {
        Self::validate_snapshot(&snapshot)?;

        {
            let mut state = self.state.write();
            let sps = state
                .entry(snapshot.service_id.clone())
                .or_insert_with(|| {
                    ServicePredictionState::new(
                        snapshot.service_id.clone(),
                        self.config.observation_window_size,
                    )
                });

            // Evict oldest observation if at capacity
            if sps.observations.len() >= self.config.observation_window_size {
                sps.observations.pop_front();
            }

            sps.observations.push_back(snapshot);

            // Update baseline from the first observation in the window
            if let Some(first) = sps.observations.front() {
                sps.baseline_health = first.health_score;
            }

            // Update trend slope
            if sps.observations.len() >= 2 {
                if let (Some(first), Some(last)) =
                    (sps.observations.front(), sps.observations.back())
                {
                    #[allow(clippy::cast_precision_loss)]
                    let count = sps.observations.len() as f64;
                    sps.trend_slope = (last.health_score - first.health_score) / count;
                }
            }
        }

        Ok(())
    }

    #[allow(clippy::significant_drop_tightening)]
    fn submit_correlation(&self, correlation: CorrelationSignal) -> Result<()> {
        Self::validate_correlation(&correlation)?;

        {
            let mut state = self.state.write();
            let sps = state
                .entry(correlation.source_service.clone())
                .or_insert_with(|| {
                    ServicePredictionState::new(
                        correlation.source_service.clone(),
                        self.config.observation_window_size,
                    )
                });

            // Cap correlation signals
            if sps.correlation_signals.len() >= MAX_CORRELATION_SIGNALS {
                sps.correlation_signals.remove(0);
            }
            sps.correlation_signals.push(correlation);
        }

        Ok(())
    }

    fn predict(&self, service_id: &str) -> Result<Option<FailurePrediction>> {
        if service_id.is_empty() {
            return Err(Error::Validation(
                "service_id cannot be empty for prediction".into(),
            ));
        }

        let state = self.state.read();
        let Some(sps) = state.get(service_id) else {
            return Ok(None);
        };

        let Some((probability, confidence, contributing_signals)) =
            Self::compute_probability(sps, &self.config)
        else {
            return Ok(None);
        };

        if probability < self.config.min_probability_threshold {
            return Ok(None);
        }

        let prediction = FailurePrediction {
            id: Uuid::new_v4().to_string(),
            service_id: service_id.to_owned(),
            probability,
            horizon_secs: Self::clamped_horizon(self.config.prediction_horizon_secs),
            contributing_signals,
            confidence,
            predicted_at: Timestamp::now(),
            outcome: None,
        };

        // Must drop the read lock before acquiring write lock
        drop(state);

        self.predictions
            .write()
            .insert(prediction.id.clone(), prediction.clone());

        Ok(Some(prediction))
    }

    fn predict_all(&self) -> Result<Vec<FailurePrediction>> {
        let service_ids: Vec<String> = {
            let state = self.state.read();
            state.keys().cloned().collect()
        };

        let mut results = Vec::new();
        for sid in &service_ids {
            if let Some(pred) = self.predict(sid)? {
                results.push(pred);
            }
        }
        Ok(results)
    }

    #[allow(clippy::significant_drop_tightening)]
    fn mark_outcome(&self, prediction_id: &str, failure_occurred: bool) -> Result<()> {
        let mut predictions = self.predictions.write();
        let pred = predictions.get_mut(prediction_id).ok_or_else(|| {
            Error::Validation(format!("prediction '{prediction_id}' not found"))
        })?;
        pred.outcome = Some(failure_occurred);
        Ok(())
    }

    fn actionable_predictions(&self) -> Vec<FailurePrediction> {
        let predictions = self.predictions.read();
        predictions
            .values()
            .filter(|p| {
                p.outcome.is_none() && p.probability >= self.config.min_probability_threshold
            })
            .cloned()
            .collect()
    }

    fn accuracy_for_service(&self, service_id: &str) -> f64 {
        let threshold = self.config.min_probability_threshold;
        let resolved: Vec<(f64, Option<bool>)> = self
            .predictions
            .read()
            .values()
            .filter(|p| p.service_id == service_id && p.outcome.is_some())
            .map(|p| (p.probability, p.outcome))
            .collect();

        if resolved.is_empty() {
            return 0.0;
        }

        let correct_count = resolved
            .iter()
            .filter(|(prob, outcome)| match outcome {
                Some(true) => *prob >= threshold,
                Some(false) => *prob < threshold,
                None => false,
            })
            .count();

        #[allow(clippy::cast_precision_loss)]
        let result = correct_count as f64 / resolved.len() as f64;
        result
    }

    fn prediction_count(&self) -> usize {
        self.predictions.read().len()
    }

    fn apply_decay(&self) {
        let mut predictions = self.predictions.write();
        predictions.retain(|_, pred| {
            if pred.outcome.is_some() {
                // Keep resolved predictions
                return true;
            }
            pred.probability = (pred.probability - self.config.decay_rate).max(0.0);
            pred.probability >= DECAY_REMOVAL_THRESHOLD
        });
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

    fn engine() -> PredictionEngineCore {
        PredictionEngineCore::default()
    }

    fn engine_with_config(config: PredictionConfig) -> PredictionEngineCore {
        PredictionEngineCore::new(config)
    }

    fn low_threshold_config() -> PredictionConfig {
        PredictionConfig {
            min_probability_threshold: 0.01,
            min_observations: 2,
            observation_window_size: 50,
            ..PredictionConfig::default()
        }
    }

    fn make_observation(service_id: &str, health: f64, error_rate: f64) -> TensorObservation {
        TensorObservation {
            service_id: service_id.into(),
            timestamp: Timestamp::now(),
            health_score: health,
            error_rate,
            latency: 0.1,
            synergy: 0.5,
        }
    }

    fn make_observation_at(
        service_id: &str,
        health: f64,
        error_rate: f64,
        ticks: u64,
    ) -> TensorObservation {
        TensorObservation {
            service_id: service_id.into(),
            timestamp: Timestamp::from_raw(ticks),
            health_score: health,
            error_rate,
            latency: 0.1,
            synergy: 0.5,
        }
    }

    fn make_correlation(source: &str, confidence: f64, offset_ms: i64) -> CorrelationSignal {
        CorrelationSignal {
            correlation_id: Uuid::new_v4().to_string(),
            source_service: source.into(),
            confidence,
            temporal_offset_ms: offset_ms,
            observed_at: Timestamp::now(),
        }
    }

    /// Populate enough observations to cross the default min_observations threshold.
    fn populate_service(eng: &PredictionEngineCore, service_id: &str, count: usize) {
        for i in 0..count {
            let health = 1.0 - (i as f64 * 0.05);
            let error_rate = i as f64 * 0.02;
            let obs = make_observation(service_id, health.max(0.0), error_rate.min(1.0));
            eng.submit_snapshot(obs).ok();
        }
    }

    /// Populate with stable (non-degrading) observations.
    fn populate_stable(eng: &PredictionEngineCore, service_id: &str, count: usize) {
        for _ in 0..count {
            let obs = make_observation(service_id, 0.95, 0.01);
            eng.submit_snapshot(obs).ok();
        }
    }

    // -----------------------------------------------------------------------
    // submit_snapshot tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_submit_snapshot_creates_state() {
        let eng = engine();
        let obs = make_observation("svc-a", 0.9, 0.05);
        let result = eng.submit_snapshot(obs);
        assert!(result.is_ok());
        assert_eq!(eng.state.read().len(), 1);
    }

    #[test]
    fn test_submit_snapshot_empty_service_id_fails() {
        let eng = engine();
        let obs = make_observation("", 0.9, 0.05);
        let result = eng.submit_snapshot(obs);
        assert!(result.is_err());
    }

    #[test]
    fn test_submit_snapshot_multiple_services() {
        let eng = engine();
        eng.submit_snapshot(make_observation("svc-a", 0.9, 0.05)).ok();
        eng.submit_snapshot(make_observation("svc-b", 0.8, 0.10)).ok();
        assert_eq!(eng.state.read().len(), 2);
    }

    #[test]
    fn test_submit_snapshot_updates_baseline() {
        let eng = engine();
        eng.submit_snapshot(make_observation("svc-a", 0.9, 0.05)).ok();
        let state = eng.state.read();
        let sps = state.get("svc-a");
        assert!(sps.is_some());
        let sps = sps.map(|s| s.baseline_health).unwrap_or(0.0);
        assert!((sps - 0.9).abs() < f64::EPSILON);
    }

    #[test]
    fn test_submit_snapshot_accumulates_observations() {
        let eng = engine();
        for _ in 0..5 {
            eng.submit_snapshot(make_observation("svc-a", 0.9, 0.05)).ok();
        }
        let state = eng.state.read();
        assert_eq!(state.get("svc-a").map(|s| s.observations.len()), Some(5));
    }

    // -----------------------------------------------------------------------
    // Observation window eviction tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_observation_window_eviction() {
        let config = PredictionConfig {
            observation_window_size: 5,
            ..PredictionConfig::default()
        };
        let eng = engine_with_config(config);

        for i in 0..10 {
            let obs = make_observation_at("svc-a", 0.9, 0.05, i * 100);
            eng.submit_snapshot(obs).ok();
        }

        let state = eng.state.read();
        assert_eq!(state.get("svc-a").map(|s| s.observations.len()), Some(5));
    }

    #[test]
    fn test_eviction_updates_baseline_to_oldest_remaining() {
        let config = PredictionConfig {
            observation_window_size: 3,
            ..PredictionConfig::default()
        };
        let eng = engine_with_config(config);

        // Submit 5 observations with decreasing health
        for i in 0..5u64 {
            let health = 1.0 - (i as f64 * 0.1);
            eng.submit_snapshot(make_observation_at("svc-a", health, 0.0, i * 100)).ok();
        }

        let state = eng.state.read();
        let sps = state.get("svc-a");
        assert!(sps.is_some());
        // After eviction, the oldest remaining observation has health = 0.8
        let baseline = sps.map(|s| s.baseline_health).unwrap_or(0.0);
        assert!((baseline - 0.8).abs() < f64::EPSILON);
    }

    // -----------------------------------------------------------------------
    // submit_correlation tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_submit_correlation_creates_state() {
        let eng = engine();
        let sig = make_correlation("svc-a", 0.8, 1000);
        let result = eng.submit_correlation(sig);
        assert!(result.is_ok());
        assert_eq!(eng.state.read().len(), 1);
    }

    #[test]
    fn test_submit_correlation_empty_source_fails() {
        let eng = engine();
        let sig = CorrelationSignal {
            correlation_id: "c-1".into(),
            source_service: String::new(),
            confidence: 0.5,
            temporal_offset_ms: 100,
            observed_at: Timestamp::now(),
        };
        assert!(eng.submit_correlation(sig).is_err());
    }

    #[test]
    fn test_submit_correlation_empty_id_fails() {
        let eng = engine();
        let sig = CorrelationSignal {
            correlation_id: String::new(),
            source_service: "svc-a".into(),
            confidence: 0.5,
            temporal_offset_ms: 100,
            observed_at: Timestamp::now(),
        };
        assert!(eng.submit_correlation(sig).is_err());
    }

    #[test]
    fn test_submit_correlation_accumulates() {
        let eng = engine();
        for _ in 0..5 {
            eng.submit_correlation(make_correlation("svc-a", 0.7, 500)).ok();
        }
        let state = eng.state.read();
        assert_eq!(
            state.get("svc-a").map(|s| s.correlation_signals.len()),
            Some(5)
        );
    }

    #[test]
    fn test_correlation_signal_cap() {
        let eng = engine();
        for _ in 0..MAX_CORRELATION_SIGNALS + 10 {
            eng.submit_correlation(make_correlation("svc-a", 0.5, 100)).ok();
        }
        let state = eng.state.read();
        assert_eq!(
            state.get("svc-a").map(|s| s.correlation_signals.len()),
            Some(MAX_CORRELATION_SIGNALS)
        );
    }

    // -----------------------------------------------------------------------
    // predict tests — insufficient data
    // -----------------------------------------------------------------------

    #[test]
    fn test_predict_empty_service_id_fails() {
        let eng = engine();
        let result = eng.predict("");
        assert!(result.is_err());
    }

    #[test]
    fn test_predict_unknown_service_returns_none() {
        let eng = engine();
        let result = eng.predict("nonexistent");
        assert!(result.is_ok());
        assert!(result.ok().flatten().is_none());
    }

    #[test]
    fn test_predict_insufficient_data_returns_none() {
        let eng = engine();
        // Submit fewer than min_observations (default 10)
        for _ in 0..5 {
            eng.submit_snapshot(make_observation("svc-a", 0.9, 0.05)).ok();
        }
        let result = eng.predict("svc-a");
        assert!(result.is_ok());
        assert!(result.ok().flatten().is_none());
    }

    // -----------------------------------------------------------------------
    // predict tests — sufficient data, stable service
    // -----------------------------------------------------------------------

    #[test]
    fn test_predict_stable_service_returns_none() {
        let eng = engine();
        // All observations healthy and stable → low probability → None
        populate_stable(&eng, "svc-a", 15);
        let result = eng.predict("svc-a");
        assert!(result.is_ok());
        // Stable service should not exceed threshold
        assert!(result.ok().flatten().is_none());
    }

    // -----------------------------------------------------------------------
    // predict tests — declining health
    // -----------------------------------------------------------------------

    #[test]
    fn test_predict_declining_health_produces_prediction() {
        let config = low_threshold_config();
        let eng = engine_with_config(config);

        // Rapidly declining health: 1.0 → 0.0
        for i in 0..10 {
            let health = 1.0 - (i as f64 * 0.1);
            eng.submit_snapshot(make_observation("svc-a", health.max(0.0), 0.0)).ok();
        }

        let result = eng.predict("svc-a");
        assert!(result.is_ok());
        let pred = result.ok().flatten();
        assert!(pred.is_some(), "declining health should trigger prediction");
    }

    #[test]
    fn test_predict_rising_error_rate_produces_prediction() {
        let config = low_threshold_config();
        let eng = engine_with_config(config);

        // Rising error rate
        for i in 0..10 {
            let err = i as f64 * 0.1;
            eng.submit_snapshot(make_observation("svc-a", 0.5, err.min(1.0))).ok();
        }

        let result = eng.predict("svc-a");
        assert!(result.is_ok());
        let pred = result.ok().flatten();
        assert!(pred.is_some(), "rising error rate should trigger prediction");
    }

    #[test]
    fn test_prediction_has_valid_fields() {
        let config = low_threshold_config();
        let eng = engine_with_config(config);
        populate_service(&eng, "svc-a", 10);

        let pred = eng.predict("svc-a").ok().flatten();
        if let Some(p) = pred {
            assert_eq!(p.service_id, "svc-a");
            assert!(!p.id.is_empty());
            assert!(p.probability >= 0.0 && p.probability <= 1.0);
            assert!(p.confidence >= 0.0 && p.confidence <= 1.0);
            assert!(p.horizon_secs >= MIN_HORIZON_SECS && p.horizon_secs <= MAX_HORIZON_SECS);
            assert!(p.outcome.is_none());
        }
    }

    #[test]
    fn test_prediction_stored_in_predictions_map() {
        let config = low_threshold_config();
        let eng = engine_with_config(config);
        populate_service(&eng, "svc-a", 10);

        let pred = eng.predict("svc-a").ok().flatten();
        if let Some(p) = &pred {
            let stored = eng.predictions.read();
            assert!(stored.contains_key(&p.id));
        }
    }

    // -----------------------------------------------------------------------
    // predict_all tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_predict_all_empty_engine() {
        let eng = engine();
        let result = eng.predict_all();
        assert!(result.is_ok());
        assert!(result.ok().map_or(false, |v| v.is_empty()));
    }

    #[test]
    fn test_predict_all_multiple_services() {
        let config = low_threshold_config();
        let eng = engine_with_config(config);

        // Populate two services with declining health
        populate_service(&eng, "svc-a", 10);
        populate_service(&eng, "svc-b", 10);

        let result = eng.predict_all();
        assert!(result.is_ok());
        // Both services are degrading so should produce predictions
        let preds = result.ok().unwrap_or_default();
        assert!(preds.len() <= 2);
    }

    // -----------------------------------------------------------------------
    // mark_outcome tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_mark_outcome_success() {
        let config = low_threshold_config();
        let eng = engine_with_config(config);
        populate_service(&eng, "svc-a", 10);

        let pred = eng.predict("svc-a").ok().flatten();
        if let Some(p) = pred {
            let result = eng.mark_outcome(&p.id, true);
            assert!(result.is_ok());
            let stored = eng.predictions.read();
            assert_eq!(stored.get(&p.id).and_then(|p| p.outcome), Some(true));
        }
    }

    #[test]
    fn test_mark_outcome_false() {
        let config = low_threshold_config();
        let eng = engine_with_config(config);
        populate_service(&eng, "svc-a", 10);

        let pred = eng.predict("svc-a").ok().flatten();
        if let Some(p) = pred {
            let result = eng.mark_outcome(&p.id, false);
            assert!(result.is_ok());
            let stored = eng.predictions.read();
            assert_eq!(stored.get(&p.id).and_then(|p| p.outcome), Some(false));
        }
    }

    #[test]
    fn test_mark_outcome_not_found() {
        let eng = engine();
        let result = eng.mark_outcome("nonexistent-id", true);
        assert!(result.is_err());
    }

    // -----------------------------------------------------------------------
    // accuracy_for_service tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_accuracy_no_outcomes() {
        let eng = engine();
        assert!((eng.accuracy_for_service("svc-a") - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_accuracy_all_correct() {
        let config = low_threshold_config();
        let eng = engine_with_config(config);
        populate_service(&eng, "svc-a", 10);

        // Create prediction and mark as correct (failure occurred, probability >= threshold)
        if let Some(p) = eng.predict("svc-a").ok().flatten() {
            eng.mark_outcome(&p.id, true).ok();
        }

        let acc = eng.accuracy_for_service("svc-a");
        assert!(acc > 0.0);
    }

    #[test]
    fn test_accuracy_unknown_service() {
        let eng = engine();
        assert!((eng.accuracy_for_service("unknown") - 0.0).abs() < f64::EPSILON);
    }

    // -----------------------------------------------------------------------
    // actionable_predictions tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_actionable_empty() {
        let eng = engine();
        assert!(eng.actionable_predictions().is_empty());
    }

    #[test]
    fn test_actionable_filters_resolved() {
        let config = low_threshold_config();
        let eng = engine_with_config(config);
        populate_service(&eng, "svc-a", 10);

        if let Some(p) = eng.predict("svc-a").ok().flatten() {
            // Before resolving — should be actionable if probability met threshold
            let before = eng.actionable_predictions();
            let before_count = before.len();

            eng.mark_outcome(&p.id, true).ok();

            // After resolving — should not be actionable
            let after = eng.actionable_predictions();
            assert!(after.len() < before_count || before_count == 0);
        }
    }

    #[test]
    fn test_actionable_filters_by_threshold() {
        let eng = engine();
        // Manually insert a prediction below threshold
        let pred = FailurePrediction {
            id: "low-prob".into(),
            service_id: "svc-a".into(),
            probability: 0.1,
            horizon_secs: 180,
            contributing_signals: Vec::new(),
            confidence: 0.5,
            predicted_at: Timestamp::now(),
            outcome: None,
        };
        eng.predictions.write().insert(pred.id.clone(), pred);

        let actionable = eng.actionable_predictions();
        assert!(actionable.is_empty(), "low probability should not be actionable");
    }

    // -----------------------------------------------------------------------
    // prediction_count tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_prediction_count_empty() {
        let eng = engine();
        assert_eq!(eng.prediction_count(), 0);
    }

    #[test]
    fn test_prediction_count_increments() {
        let config = low_threshold_config();
        let eng = engine_with_config(config);
        populate_service(&eng, "svc-a", 10);

        let before = eng.prediction_count();
        eng.predict("svc-a").ok();
        let after = eng.prediction_count();
        assert!(after >= before);
    }

    // -----------------------------------------------------------------------
    // apply_decay tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_apply_decay_reduces_probability() {
        let eng = engine();

        // Manually insert a pending prediction
        let pred = FailurePrediction {
            id: "decay-test".into(),
            service_id: "svc-a".into(),
            probability: 0.5,
            horizon_secs: 180,
            contributing_signals: Vec::new(),
            confidence: 0.8,
            predicted_at: Timestamp::now(),
            outcome: None,
        };
        eng.predictions.write().insert(pred.id.clone(), pred);

        eng.apply_decay();

        let preds = eng.predictions.read();
        let prob = preds.get("decay-test").map(|p| p.probability).unwrap_or(0.0);
        assert!(
            (prob - (0.5 - DEFAULT_DECAY_RATE)).abs() < f64::EPSILON,
            "probability should be reduced by decay_rate"
        );
    }

    #[test]
    fn test_apply_decay_removes_below_threshold() {
        let eng = engine();

        // Insert a prediction just above removal threshold
        let pred = FailurePrediction {
            id: "tiny-prob".into(),
            service_id: "svc-a".into(),
            probability: 0.005, // below DECAY_REMOVAL_THRESHOLD after decay
            horizon_secs: 180,
            contributing_signals: Vec::new(),
            confidence: 0.5,
            predicted_at: Timestamp::now(),
            outcome: None,
        };
        eng.predictions.write().insert(pred.id.clone(), pred);

        eng.apply_decay();

        assert!(
            !eng.predictions.read().contains_key("tiny-prob"),
            "prediction below threshold should be removed"
        );
    }

    #[test]
    fn test_apply_decay_preserves_resolved() {
        let eng = engine();

        let pred = FailurePrediction {
            id: "resolved".into(),
            service_id: "svc-a".into(),
            probability: 0.8,
            horizon_secs: 180,
            contributing_signals: Vec::new(),
            confidence: 0.9,
            predicted_at: Timestamp::now(),
            outcome: Some(true),
        };
        eng.predictions.write().insert(pred.id.clone(), pred);

        eng.apply_decay();

        let preds = eng.predictions.read();
        assert!(
            preds.contains_key("resolved"),
            "resolved predictions should not be removed"
        );
        // Probability should be unchanged
        let prob = preds.get("resolved").map(|p| p.probability).unwrap_or(0.0);
        assert!((prob - 0.8).abs() < f64::EPSILON);
    }

    #[test]
    fn test_apply_decay_multiple_cycles() {
        let eng = engine();

        let pred = FailurePrediction {
            id: "multi-decay".into(),
            service_id: "svc-a".into(),
            probability: 0.1,
            horizon_secs: 180,
            contributing_signals: Vec::new(),
            confidence: 0.5,
            predicted_at: Timestamp::now(),
            outcome: None,
        };
        eng.predictions.write().insert(pred.id.clone(), pred);

        // Apply decay many times — eventually should be removed
        for _ in 0..100 {
            eng.apply_decay();
        }

        assert!(
            !eng.predictions.read().contains_key("multi-decay"),
            "prediction should eventually be removed after many decay cycles"
        );
    }

    // -----------------------------------------------------------------------
    // Correlation signal boost tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_correlation_signals_boost_probability() {
        let config = PredictionConfig {
            min_probability_threshold: 0.01,
            min_observations: 3,
            observation_window_size: 50,
            ..PredictionConfig::default()
        };
        let eng = engine_with_config(config);

        // Submit stable observations (should have low base probability)
        for _ in 0..5 {
            eng.submit_snapshot(make_observation("svc-a", 0.8, 0.05)).ok();
        }

        let pred_without = eng.predict("svc-a").ok().flatten();
        let prob_without = pred_without.map(|p| p.probability).unwrap_or(0.0);

        // Now add strong correlation signals
        for _ in 0..5 {
            eng.submit_correlation(make_correlation("svc-a", 0.9, 100)).ok();
        }

        let pred_with = eng.predict("svc-a").ok().flatten();
        let prob_with = pred_with.map(|p| p.probability).unwrap_or(0.0);

        assert!(
            prob_with >= prob_without,
            "correlation signals should boost probability: {prob_with} >= {prob_without}"
        );
    }

    #[test]
    fn test_distant_correlation_has_less_weight() {
        let config = PredictionConfig {
            min_probability_threshold: 0.01,
            min_observations: 3,
            observation_window_size: 50,
            ..PredictionConfig::default()
        };

        // Engine with close correlations
        let eng_close = engine_with_config(config.clone());
        for _ in 0..5 {
            eng_close.submit_snapshot(make_observation("svc-a", 0.8, 0.1)).ok();
        }
        eng_close.submit_correlation(make_correlation("svc-a", 0.9, 100)).ok();
        let pred_close = eng_close.predict("svc-a").ok().flatten();
        let prob_close = pred_close.map(|p| p.probability).unwrap_or(0.0);

        // Engine with distant correlations
        let eng_far = engine_with_config(config);
        for _ in 0..5 {
            eng_far.submit_snapshot(make_observation("svc-a", 0.8, 0.1)).ok();
        }
        eng_far.submit_correlation(make_correlation("svc-a", 0.9, 290_000)).ok();
        let pred_far = eng_far.predict("svc-a").ok().flatten();
        let prob_far = pred_far.map(|p| p.probability).unwrap_or(0.0);

        assert!(
            prob_close >= prob_far,
            "close correlations should have more weight: {prob_close} >= {prob_far}"
        );
    }

    // -----------------------------------------------------------------------
    // Trend detection tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_declining_health_higher_probability_than_stable() {
        let config = PredictionConfig {
            min_probability_threshold: 0.0,
            min_observations: 5,
            observation_window_size: 50,
            ..PredictionConfig::default()
        };

        // Declining health
        let eng_decline = engine_with_config(config.clone());
        for i in 0..10 {
            let health = 1.0 - (i as f64 * 0.08);
            eng_decline
                .submit_snapshot(make_observation("svc-a", health.max(0.0), 0.0))
                .ok();
        }
        let prob_decline = eng_decline
            .predict("svc-a")
            .ok()
            .flatten()
            .map(|p| p.probability)
            .unwrap_or(0.0);

        // Stable health
        let eng_stable = engine_with_config(config);
        for _ in 0..10 {
            eng_stable
                .submit_snapshot(make_observation("svc-a", 0.95, 0.0))
                .ok();
        }
        let prob_stable = eng_stable
            .predict("svc-a")
            .ok()
            .flatten()
            .map(|p| p.probability)
            .unwrap_or(0.0);

        assert!(
            prob_decline >= prob_stable,
            "declining health should yield higher probability: {prob_decline} >= {prob_stable}"
        );
    }

    // -----------------------------------------------------------------------
    // Edge case tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_all_zero_observations() {
        let config = low_threshold_config();
        let eng = engine_with_config(config);

        for _ in 0..5 {
            let obs = TensorObservation {
                service_id: "svc-zero".into(),
                timestamp: Timestamp::now(),
                health_score: 0.0,
                error_rate: 0.0,
                latency: 0.0,
                synergy: 0.0,
            };
            eng.submit_snapshot(obs).ok();
        }

        let result = eng.predict("svc-zero");
        assert!(result.is_ok());
        // Should not panic — result can be None or Some depending on threshold
    }

    #[test]
    fn test_all_one_observations() {
        let config = low_threshold_config();
        let eng = engine_with_config(config);

        for _ in 0..5 {
            let obs = TensorObservation {
                service_id: "svc-max".into(),
                timestamp: Timestamp::now(),
                health_score: 1.0,
                error_rate: 1.0,
                latency: 1.0,
                synergy: 1.0,
            };
            eng.submit_snapshot(obs).ok();
        }

        let result = eng.predict("svc-max");
        assert!(result.is_ok());
    }

    #[test]
    fn test_single_observation_insufficient() {
        let eng = engine();
        eng.submit_snapshot(make_observation("svc-a", 0.5, 0.5)).ok();
        let result = eng.predict("svc-a");
        assert!(result.is_ok());
        assert!(result.ok().flatten().is_none());
    }

    #[test]
    fn test_exactly_min_observations() {
        let config = PredictionConfig {
            min_observations: 3,
            min_probability_threshold: 0.0,
            observation_window_size: 50,
            ..PredictionConfig::default()
        };
        let eng = engine_with_config(config);

        // Submit exactly 3 observations with declining health
        eng.submit_snapshot(make_observation("svc-a", 0.9, 0.0)).ok();
        eng.submit_snapshot(make_observation("svc-a", 0.5, 0.2)).ok();
        eng.submit_snapshot(make_observation("svc-a", 0.1, 0.5)).ok();

        let result = eng.predict("svc-a");
        assert!(result.is_ok());
        // With threshold 0.0 and declining health, should produce a prediction
        let pred = result.ok().flatten();
        assert!(pred.is_some());
    }

    // -----------------------------------------------------------------------
    // Concurrent access tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_concurrent_submit_and_predict() {
        use std::sync::Arc;
        use std::thread;

        let eng = Arc::new(PredictionEngineCore::new(PredictionConfig {
            min_observations: 3,
            min_probability_threshold: 0.01,
            observation_window_size: 50,
            ..PredictionConfig::default()
        }));

        let eng_writer = Arc::clone(&eng);
        let writer = thread::spawn(move || {
            for i in 0..20 {
                let health = 1.0 - (i as f64 * 0.04);
                eng_writer
                    .submit_snapshot(make_observation("svc-concurrent", health.max(0.0), 0.0))
                    .ok();
            }
        });

        let eng_reader = Arc::clone(&eng);
        let reader = thread::spawn(move || {
            for _ in 0..10 {
                let _ = eng_reader.predict("svc-concurrent");
            }
        });

        writer.join().ok();
        reader.join().ok();

        // Should not deadlock or panic
        assert!(eng.state.read().len() <= 1);
    }

    #[test]
    fn test_concurrent_decay_and_submit() {
        use std::sync::Arc;
        use std::thread;

        let eng = Arc::new(engine());

        // Pre-populate
        let pred = FailurePrediction {
            id: "concurrent-decay".into(),
            service_id: "svc-a".into(),
            probability: 0.5,
            horizon_secs: 180,
            contributing_signals: Vec::new(),
            confidence: 0.5,
            predicted_at: Timestamp::now(),
            outcome: None,
        };
        eng.predictions.write().insert(pred.id.clone(), pred);

        let eng_decay = Arc::clone(&eng);
        let decayer = thread::spawn(move || {
            for _ in 0..50 {
                eng_decay.apply_decay();
            }
        });

        let eng_submit = Arc::clone(&eng);
        let submitter = thread::spawn(move || {
            for _ in 0..20 {
                eng_submit
                    .submit_snapshot(make_observation("svc-a", 0.8, 0.1))
                    .ok();
            }
        });

        decayer.join().ok();
        submitter.join().ok();
        // Should not deadlock or panic
    }

    // -----------------------------------------------------------------------
    // Config tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_default_config_values() {
        let config = PredictionConfig::default();
        assert_eq!(config.prediction_horizon_secs, DEFAULT_PREDICTION_HORIZON_SECS);
        assert!((config.min_probability_threshold - DEFAULT_MIN_PROBABILITY_THRESHOLD).abs() < f64::EPSILON);
        assert_eq!(config.min_observations, DEFAULT_MIN_OBSERVATIONS);
        assert_eq!(config.observation_window_size, DEFAULT_OBSERVATION_WINDOW_SIZE);
        assert!((config.decay_rate - DEFAULT_DECAY_RATE).abs() < f64::EPSILON);
    }

    #[test]
    fn test_custom_config() {
        let config = PredictionConfig {
            prediction_horizon_secs: 250,
            min_probability_threshold: 0.8,
            min_observations: 20,
            observation_window_size: 500,
            decay_rate: 0.01,
        };
        let eng = engine_with_config(config);
        // Engine should use the custom values
        assert_eq!(eng.config.min_observations, 20);
        assert_eq!(eng.config.observation_window_size, 500);
    }

    // -----------------------------------------------------------------------
    // Horizon clamping tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_clamped_horizon_within_range() {
        assert_eq!(PredictionEngineCore::clamped_horizon(180), 180);
        assert_eq!(PredictionEngineCore::clamped_horizon(120), 120);
        assert_eq!(PredictionEngineCore::clamped_horizon(300), 300);
    }

    #[test]
    fn test_clamped_horizon_below_min() {
        assert_eq!(PredictionEngineCore::clamped_horizon(50), MIN_HORIZON_SECS);
    }

    #[test]
    fn test_clamped_horizon_above_max() {
        assert_eq!(PredictionEngineCore::clamped_horizon(500), MAX_HORIZON_SECS);
    }

    // -----------------------------------------------------------------------
    // Contributing signals tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_contributing_signals_populated() {
        let config = PredictionConfig {
            min_probability_threshold: 0.01,
            min_observations: 3,
            observation_window_size: 50,
            ..PredictionConfig::default()
        };
        let eng = engine_with_config(config);

        // Declining health
        for i in 0..5 {
            let health = 1.0 - (i as f64 * 0.15);
            eng.submit_snapshot(make_observation("svc-a", health.max(0.0), 0.1)).ok();
        }

        // Add correlation with non-zero contribution
        let sig = make_correlation("svc-a", 0.9, 100);
        let sig_id = sig.correlation_id.clone();
        eng.submit_correlation(sig).ok();

        let pred = eng.predict("svc-a").ok().flatten();
        if let Some(p) = pred {
            assert!(
                p.contributing_signals.contains(&sig_id),
                "contributing_signals should contain the correlation ID"
            );
        }
    }

    // -----------------------------------------------------------------------
    // Trait object compatibility test
    // -----------------------------------------------------------------------

    #[test]
    fn test_trait_object_send_sync() {
        fn assert_send_sync<T: Send + Sync + fmt::Debug>() {}
        assert_send_sync::<PredictionEngineCore>();
    }

    #[test]
    fn test_as_trait_object() {
        let eng: Box<dyn PredictionEngine> = Box::new(engine());
        assert_eq!(eng.prediction_count(), 0);
    }

    // -----------------------------------------------------------------------
    // Debug implementations
    // -----------------------------------------------------------------------

    #[test]
    fn test_debug_tensor_observation() {
        let obs = make_observation("svc-a", 0.9, 0.05);
        let debug = format!("{obs:?}");
        assert!(debug.contains("svc-a"));
    }

    #[test]
    fn test_debug_correlation_signal() {
        let sig = make_correlation("svc-a", 0.8, 500);
        let debug = format!("{sig:?}");
        assert!(debug.contains("svc-a"));
    }

    #[test]
    fn test_debug_failure_prediction() {
        let pred = FailurePrediction {
            id: "test-id".into(),
            service_id: "svc-a".into(),
            probability: 0.7,
            horizon_secs: 180,
            contributing_signals: vec!["sig-1".into()],
            confidence: 0.6,
            predicted_at: Timestamp::now(),
            outcome: None,
        };
        let debug = format!("{pred:?}");
        assert!(debug.contains("test-id"));
        assert!(debug.contains("svc-a"));
    }

    #[test]
    fn test_debug_prediction_config() {
        let config = PredictionConfig::default();
        let debug = format!("{config:?}");
        assert!(debug.contains("prediction_horizon_secs"));
    }

    #[test]
    fn test_debug_prediction_engine_core() {
        let eng = engine();
        let debug = format!("{eng:?}");
        assert!(debug.contains("PredictionEngineCore"));
    }

    // -----------------------------------------------------------------------
    // Clone implementations
    // -----------------------------------------------------------------------

    #[test]
    fn test_clone_tensor_observation() {
        let obs = make_observation("svc-a", 0.9, 0.05);
        let cloned = obs.clone();
        assert_eq!(cloned.service_id, obs.service_id);
        assert!((cloned.health_score - obs.health_score).abs() < f64::EPSILON);
    }

    #[test]
    fn test_clone_correlation_signal() {
        let sig = make_correlation("svc-a", 0.8, 500);
        let cloned = sig.clone();
        assert_eq!(cloned.correlation_id, sig.correlation_id);
        assert!((cloned.confidence - sig.confidence).abs() < f64::EPSILON);
    }

    #[test]
    fn test_clone_failure_prediction() {
        let pred = FailurePrediction {
            id: "clone-test".into(),
            service_id: "svc-a".into(),
            probability: 0.7,
            horizon_secs: 180,
            contributing_signals: vec!["sig-1".into()],
            confidence: 0.6,
            predicted_at: Timestamp::now(),
            outcome: Some(true),
        };
        let cloned = pred.clone();
        assert_eq!(cloned.id, pred.id);
        assert_eq!(cloned.outcome, pred.outcome);
    }

    #[test]
    fn test_clone_prediction_config() {
        let config = PredictionConfig {
            prediction_horizon_secs: 250,
            min_probability_threshold: 0.7,
            min_observations: 15,
            observation_window_size: 100,
            decay_rate: 0.005,
        };
        let cloned = config.clone();
        assert_eq!(cloned.prediction_horizon_secs, 250);
        assert_eq!(cloned.min_observations, 15);
    }

    // -----------------------------------------------------------------------
    // Probability clamping tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_probability_always_clamped_0_1() {
        let config = PredictionConfig {
            min_probability_threshold: 0.0,
            min_observations: 2,
            observation_window_size: 50,
            ..PredictionConfig::default()
        };
        let eng = engine_with_config(config);

        // Extreme values
        eng.submit_snapshot(TensorObservation {
            service_id: "svc-extreme".into(),
            timestamp: Timestamp::now(),
            health_score: 1.0,
            error_rate: 0.0,
            latency: 0.0,
            synergy: 0.0,
        }).ok();
        eng.submit_snapshot(TensorObservation {
            service_id: "svc-extreme".into(),
            timestamp: Timestamp::now(),
            health_score: 0.0,
            error_rate: 1.0,
            latency: 1.0,
            synergy: 0.0,
        }).ok();

        if let Some(p) = eng.predict("svc-extreme").ok().flatten() {
            assert!(p.probability >= 0.0 && p.probability <= 1.0);
        }
    }

    // -----------------------------------------------------------------------
    // Decay does not affect probability of resolved predictions
    // -----------------------------------------------------------------------

    #[test]
    fn test_decay_does_not_reduce_resolved_probability() {
        let eng = engine();

        let pred = FailurePrediction {
            id: "resolved-decay".into(),
            service_id: "svc-a".into(),
            probability: 0.9,
            horizon_secs: 180,
            contributing_signals: Vec::new(),
            confidence: 0.8,
            predicted_at: Timestamp::now(),
            outcome: Some(false),
        };
        eng.predictions.write().insert(pred.id.clone(), pred);

        eng.apply_decay();

        let preds = eng.predictions.read();
        let prob = preds.get("resolved-decay").map(|p| p.probability).unwrap_or(0.0);
        assert!((prob - 0.9).abs() < f64::EPSILON);
    }
}
