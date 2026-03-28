//! # M45: Fitness Evaluator
//!
//! 12D tensor fitness scoring utility for the L7 Observer Layer.
//! Evaluates system health using weighted dimension analysis, trend
//! detection, and stability assessment against the `Tensor12D` encoding.
//!
//! ## Layer: L7 (Observer)
//! ## Dependencies: `Tensor12D` (lib.rs), M01 (Error)
//!
//! ## Weight Categories (sum = 1.0)
//!
//! | Category | Weight | Dimensions |
//! |----------|--------|------------|
//! | Primary | 38% | D6 health (20%), D7 uptime (15%), D8 synergy (3%) |
//! | Secondary | 37% | D8 synergy (12%), D9 latency (10%), D10 `error_rate` (10%), D2 tier (5%) |
//! | Context | 20% | D3 deps (5%), D4 agents (5%), D5 protocol (3%), D0 `service_id` (5%), D11 temporal (2%) |
//! | Identity | 5% | D1 port (2%), remaining (3%) |
//!
//! ## Related Documentation
//! - [Fitness Function Spec](../../ai_specs/evolution_chamber_ai_specs/FITNESS_FUNCTION_SPEC.md)
//! - [Tensor Spec](../../ai_specs/TENSOR_SPEC.md)

use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{Error, Result, Tensor12D};

/// Dimension weights for 12D fitness scoring.
/// Sum = 1.0.
///
/// | D# | Name | Weight | Category |
/// |----|------|--------|----------|
/// | D0 | service_id | 0.05 | Identity |
/// | D1 | port | 0.02 | Identity |
/// | D2 | tier | 0.08 | Secondary |
/// | D3 | deps | 0.05 | Context |
/// | D4 | agents | 0.05 | Context |
/// | D5 | protocol | 0.03 | Context |
/// | D6 | health | 0.20 | Primary |
/// | D7 | uptime | 0.15 | Primary |
/// | D8 | synergy | 0.15 | Primary/Secondary |
/// | D9 | latency | 0.10 | Secondary |
/// | D10| error_rate | 0.10 | Secondary |
/// | D11| temporal | 0.02 | Context |
pub const DIMENSION_WEIGHTS: [f64; 12] = [
    0.05, // D0: service_id
    0.02, // D1: port
    0.08, // D2: tier
    0.05, // D3: dependency_count
    0.05, // D4: agent_count
    0.03, // D5: protocol
    0.20, // D6: health_score (PRIMARY)
    0.15, // D7: uptime (PRIMARY)
    0.15, // D8: synergy (PRIMARY)
    0.10, // D9: latency (SECONDARY)
    0.10, // D10: error_rate (SECONDARY)
    0.02, // D11: temporal_context
];

/// Dimension names for human-readable reports.
const DIMENSION_NAMES: [&str; 12] = [
    "service_id",
    "port",
    "tier",
    "dependency_count",
    "agent_count",
    "protocol",
    "health_score",
    "uptime",
    "synergy",
    "latency",
    "error_rate",
    "temporal_context",
];

/// Default fitness history capacity (`FitnessEvaluator`'s `FitnessReport` buffer).
/// Distinct from `EvolutionChamberConfig.fitness_history_capacity` (500),
/// which sizes M39's internal `FitnessSnapshot` buffer.
const DEFAULT_HISTORY_CAPACITY: usize = 200;

/// Default trend window size.
const DEFAULT_TREND_WINDOW: usize = 10;

/// Default stability tolerance (std dev below this = stable).
const DEFAULT_STABILITY_TOLERANCE: f64 = 0.02;

/// Default volatility threshold (std dev above this = volatile).
const DEFAULT_VOLATILITY_THRESHOLD: f64 = 0.10;

/// Fitness trend direction.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FitnessTrend {
    /// System fitness is improving.
    Improving,
    /// System fitness is stable.
    Stable,
    /// System fitness is declining.
    Declining,
    /// Not enough data to determine trend.
    Unknown,
}

/// System state classification based on fitness.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SystemState {
    /// Fitness >= 0.9 -- system is thriving.
    Optimal,
    /// 0.7 <= fitness < 0.9 -- normal operation.
    Healthy,
    /// 0.5 <= fitness < 0.7 -- some degradation.
    Degraded,
    /// 0.3 <= fitness < 0.5 -- significant issues.
    Critical,
    /// Fitness < 0.3 -- system failure.
    Failed,
}

impl SystemState {
    /// Classify system state from a fitness score.
    #[must_use]
    pub fn from_fitness(fitness: f64) -> Self {
        if fitness >= 0.9 {
            Self::Optimal
        } else if fitness >= 0.7 {
            Self::Healthy
        } else if fitness >= 0.5 {
            Self::Degraded
        } else if fitness >= 0.3 {
            Self::Critical
        } else {
            Self::Failed
        }
    }
}

/// A fitness evaluation report.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FitnessReport {
    /// Unique report ID (UUID v4).
    pub id: String,
    /// Evaluated tensor dimensions.
    pub tensor: [f64; 12],
    /// Overall weighted fitness score [0.0, 1.0].
    pub overall_score: f64,
    /// Per-dimension raw scores.
    pub dimension_scores: [f64; 12],
    /// Weighted contribution of each dimension.
    pub weighted_contributions: [f64; 12],
    /// Trend direction (-1.0 declining to +1.0 improving).
    pub trend: f64,
    /// Stability score (0.0 volatile to 1.0 stable).
    pub stability: f64,
    /// System state classification.
    pub system_state: SystemState,
    /// Fitness trend direction.
    pub fitness_trend: FitnessTrend,
    /// Evaluation timestamp.
    pub evaluated_at: DateTime<Utc>,
}

/// A historical fitness snapshot.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FitnessSnapshot {
    /// Snapshot timestamp.
    pub timestamp: DateTime<Utc>,
    /// Overall fitness at this point.
    pub fitness: f64,
    /// Tensor state.
    pub tensor: [f64; 12],
    /// Generation number (if during evolution).
    pub generation: Option<u64>,
}

/// Configuration for the Fitness Evaluator.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FitnessConfig {
    /// Maximum `FitnessReport` snapshots retained.
    /// Default: 200. This is distinct from `EvolutionChamberConfig.fitness_history_capacity`
    /// (default 500), which sizes M39's internal `FitnessSnapshot` buffer.
    pub history_capacity: usize,
    /// Number of snapshots used for trend calculation.
    pub trend_window: usize,
    /// Standard deviation below this is considered stable.
    pub stability_tolerance: f64,
    /// Standard deviation above this is considered volatile.
    pub volatility_threshold: f64,
}

impl Default for FitnessConfig {
    fn default() -> Self {
        Self {
            history_capacity: DEFAULT_HISTORY_CAPACITY,
            trend_window: DEFAULT_TREND_WINDOW,
            stability_tolerance: DEFAULT_STABILITY_TOLERANCE,
            volatility_threshold: DEFAULT_VOLATILITY_THRESHOLD,
        }
    }
}

/// 12D tensor fitness evaluator.
///
/// Evaluates system health using weighted dimension analysis, trend
/// detection, and stability assessment.
///
/// # Thread Safety
///
/// History is protected by `parking_lot::RwLock`.
pub struct FitnessEvaluator {
    /// Fitness history (bounded ring buffer).
    history: RwLock<Vec<FitnessSnapshot>>,
    /// Immutable configuration.
    config: FitnessConfig,
}

impl FitnessEvaluator {
    /// Creates a new `FitnessEvaluator` with default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self::with_config(FitnessConfig::default())
    }

    /// Creates a new `FitnessEvaluator` with the given configuration.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if configuration values are out of range.
    #[must_use] pub fn with_config(config: FitnessConfig) -> Self {
        Self {
            history: RwLock::new(Vec::with_capacity(config.history_capacity)),
            config,
        }
    }

    /// Validates the evaluator configuration.
    ///
    /// # Errors
    ///
    /// Returns `Error::Config` if any parameter is out of range.
    pub fn validate_config(config: &FitnessConfig) -> Result<()> {
        if config.history_capacity == 0 {
            return Err(Error::Config("history_capacity must be > 0".into()));
        }
        if config.trend_window == 0 {
            return Err(Error::Config("trend_window must be > 0".into()));
        }
        if config.trend_window > config.history_capacity {
            return Err(Error::Config(
                "trend_window must not exceed history_capacity".into(),
            ));
        }
        if config.stability_tolerance < 0.0 || config.stability_tolerance > 1.0 {
            return Err(Error::Config(
                "stability_tolerance must be in [0.0, 1.0]".into(),
            ));
        }
        if config.volatility_threshold < 0.0 || config.volatility_threshold > 1.0 {
            return Err(Error::Config(
                "volatility_threshold must be in [0.0, 1.0]".into(),
            ));
        }
        Ok(())
    }

    /// Evaluates the fitness of a `Tensor12D`, recording the result in history.
    ///
    /// # Errors
    ///
    /// Returns `Error::TensorValidation` if the tensor contains invalid values.
    pub fn evaluate(&self, tensor: &Tensor12D, generation: Option<u64>) -> Result<FitnessReport> {
        tensor.validate()?;

        let dims = tensor.to_array();
        let mut dimension_scores = [0.0_f64; 12];
        let mut weighted_contributions = [0.0_f64; 12];

        // Compute per-dimension scores
        // D9 (latency) and D10 (error_rate) are inverted: lower = better
        for (i, &val) in dims.iter().enumerate() {
            dimension_scores[i] = if i == 9 || i == 10 {
                1.0 - val // Invert latency and error_rate
            } else {
                val
            };
            weighted_contributions[i] = dimension_scores[i] * DIMENSION_WEIGHTS[i];
        }

        let overall_score: f64 = weighted_contributions.iter().sum();
        let overall_score = overall_score.clamp(0.0, 1.0);

        // Compute trend from history
        let trend = self.compute_trend(overall_score);
        let fitness_trend = self.classify_trend(trend);

        // Compute stability from history
        let stability = self.compute_stability();

        let system_state = SystemState::from_fitness(overall_score);

        let report = FitnessReport {
            id: Uuid::new_v4().to_string(),
            tensor: dims,
            overall_score,
            dimension_scores,
            weighted_contributions,
            trend,
            stability,
            system_state,
            fitness_trend,
            evaluated_at: Utc::now(),
        };

        // Record snapshot in history
        let snapshot = FitnessSnapshot {
            timestamp: report.evaluated_at,
            fitness: overall_score,
            tensor: dims,
            generation,
        };

        {
            let mut history = self.history.write();
            if history.len() >= self.config.history_capacity {
                history.remove(0);
            }
            history.push(snapshot);
        }

        Ok(report)
    }

    /// Computes trend from historical data and current score.
    /// Returns value in [-1.0, 1.0]. Positive = improving.
    fn compute_trend(&self, current: f64) -> f64 {
        let values: Vec<f64> = {
            let history = self.history.read();
            let window_size = self.config.trend_window.min(history.len());
            if window_size < 2 {
                return 0.0;
            }
            let start = history.len() - window_size;
            history[start..].iter().map(|s| s.fitness).collect()
        };

        // Simple linear regression slope over the trend window
        #[allow(clippy::cast_precision_loss)]
        let n = values.len() as f64;
        let mut acc_x = 0.0_f64;
        let mut acc_y = 0.0_f64;
        let mut acc_x_times_y = 0.0_f64;
        let mut acc_x2 = 0.0_f64;

        #[allow(clippy::cast_precision_loss)]
        for (i, &fitness) in values.iter().enumerate() {
            let x = i as f64;
            acc_x += x;
            acc_y += fitness;
            acc_x_times_y += x * fitness;
            acc_x2 += x * x;
        }

        // Include current value as the next point
        let x_current = n;
        acc_x += x_current;
        acc_y += current;
        acc_x_times_y += x_current * current;
        acc_x2 += x_current * x_current;
        let total_n = n + 1.0;

        let denominator = total_n.mul_add(acc_x2, -(acc_x * acc_x));
        if denominator.abs() < 1e-10 {
            return 0.0;
        }

        let slope = total_n.mul_add(acc_x_times_y, -(acc_x * acc_y)) / denominator;

        // Normalize slope to [-1, 1] range
        // A slope of 0.01 per step is a strong trend
        (slope * 100.0).clamp(-1.0, 1.0)
    }

    /// Classifies a numeric trend into a `FitnessTrend` variant.
    fn classify_trend(&self, trend: f64) -> FitnessTrend {
        let has_enough = self.history.read().len() >= 2;
        if !has_enough {
            return FitnessTrend::Unknown;
        }
        if trend > 0.1 {
            FitnessTrend::Improving
        } else if trend < -0.1 {
            FitnessTrend::Declining
        } else {
            FitnessTrend::Stable
        }
    }

    /// Computes stability as 1.0 - `normalized_stddev`.
    fn compute_stability(&self) -> f64 {
        let values: Vec<f64> = {
            let history = self.history.read();
            let window_size = self.config.trend_window.min(history.len());
            if window_size < 2 {
                return 1.0; // Assume stable when insufficient data
            }
            let start = history.len() - window_size;
            history[start..].iter().map(|s| s.fitness).collect()
        };

        #[allow(clippy::cast_precision_loss)]
        let mean: f64 = values.iter().sum::<f64>() / values.len() as f64;

        #[allow(clippy::cast_precision_loss)]
        let variance: f64 = values
            .iter()
            .map(|v| (v - mean).powi(2))
            .sum::<f64>()
            / values.len() as f64;

        let stddev = variance.sqrt();

        // Map stddev to stability: 0 stddev = 1.0 stability
        // volatility_threshold stddev = 0.0 stability
        if stddev <= self.config.stability_tolerance {
            1.0
        } else if stddev >= self.config.volatility_threshold {
            0.0
        } else {
            let range = self.config.volatility_threshold - self.config.stability_tolerance;
            if range.abs() < 1e-10 {
                return 0.5;
            }
            1.0 - ((stddev - self.config.stability_tolerance) / range)
        }
    }

    /// Returns the most recent `FitnessSnapshot`.
    #[must_use]
    pub fn latest_snapshot(&self) -> Option<FitnessSnapshot> {
        self.history.read().last().cloned()
    }

    /// Returns the most recent N snapshots.
    #[must_use]
    pub fn recent_snapshots(&self, n: usize) -> Vec<FitnessSnapshot> {
        let history = self.history.read();
        let start = history.len().saturating_sub(n);
        history[start..].to_vec()
    }

    /// Returns the current fitness score, or `None` if no evaluations done.
    #[must_use]
    pub fn current_fitness(&self) -> Option<f64> {
        self.history.read().last().map(|s| s.fitness)
    }

    /// Returns the current system state, or `None` if no evaluations done.
    #[must_use]
    pub fn current_state(&self) -> Option<SystemState> {
        self.current_fitness().map(SystemState::from_fitness)
    }

    /// Returns history size.
    #[must_use]
    pub fn history_len(&self) -> usize {
        self.history.read().len()
    }

    /// Clears all history.
    pub fn clear_history(&self) {
        self.history.write().clear();
    }

    /// Returns the dimension name for a given index.
    #[must_use]
    pub fn dimension_name(index: usize) -> Option<&'static str> {
        DIMENSION_NAMES.get(index).copied()
    }

    /// Returns all dimension names.
    #[must_use]
    pub const fn dimension_names() -> &'static [&'static str; 12] {
        &DIMENSION_NAMES
    }

    /// Returns the dimension weights.
    #[must_use]
    pub const fn dimension_weights() -> &'static [f64; 12] {
        &DIMENSION_WEIGHTS
    }

    /// Returns the average fitness over the last N snapshots.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn average_fitness(&self, n: usize) -> Option<f64> {
        let values: Vec<f64> = {
            let history = self.history.read();
            if history.is_empty() {
                return None;
            }
            let start = history.len().saturating_sub(n);
            history[start..].iter().map(|s| s.fitness).collect()
        };
        let sum: f64 = values.iter().sum();
        Some(sum / values.len() as f64)
    }

    /// Returns the min and max fitness values from history.
    #[must_use]
    pub fn fitness_range(&self) -> Option<(f64, f64)> {
        let values: Vec<f64> = {
            let history = self.history.read();
            if history.is_empty() {
                return None;
            }
            history.iter().map(|s| s.fitness).collect()
        };
        let mut min = f64::MAX;
        let mut max = f64::MIN;
        for &val in &values {
            if val < min {
                min = val;
            }
            if val > max {
                max = val;
            }
        }
        Some((min, max))
    }

    /// Returns the configuration.
    #[must_use]
    pub const fn config(&self) -> &FitnessConfig {
        &self.config
    }

    /// Returns the fitness delta between the two most recent evaluations.
    #[must_use]
    pub fn fitness_delta(&self) -> Option<f64> {
        let history = self.history.read();
        if history.len() < 2 {
            return None;
        }
        let len = history.len();
        Some(history[len - 1].fitness - history[len - 2].fitness)
    }

    /// Returns whether the system has been consistently improving.
    #[must_use]
    pub fn is_improving(&self, window: usize) -> bool {
        let values: Vec<f64> = {
            let history = self.history.read();
            if history.len() < window || window < 2 {
                return false;
            }
            let start = history.len() - window;
            history[start..].iter().map(|s| s.fitness).collect()
        };
        values.windows(2).all(|w| w[1] >= w[0] - 1e-10)
    }

    /// Returns whether the system has been consistently declining.
    #[must_use]
    pub fn is_declining(&self, window: usize) -> bool {
        let values: Vec<f64> = {
            let history = self.history.read();
            if history.len() < window || window < 2 {
                return false;
            }
            let start = history.len() - window;
            history[start..].iter().map(|s| s.fitness).collect()
        };
        values.windows(2).all(|w| w[1] <= w[0] + 1e-10)
    }
}

impl Default for FitnessEvaluator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_evaluator() -> FitnessEvaluator {
        FitnessEvaluator::new()
    }

    fn healthy_tensor() -> Tensor12D {
        Tensor12D::new([0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.95, 0.99, 0.90, 0.05, 0.02, 0.5])
    }

    fn degraded_tensor() -> Tensor12D {
        Tensor12D::new([0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.4, 0.5, 0.4, 0.7, 0.6, 0.5])
    }

    fn zero_tensor() -> Tensor12D {
        Tensor12D::default()
    }

    fn perfect_tensor() -> Tensor12D {
        Tensor12D::new([1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 0.0, 0.0, 1.0])
    }

    #[test]
    fn test_dimension_weights_sum_to_one() {
        let sum: f64 = DIMENSION_WEIGHTS.iter().sum();
        assert!((sum - 1.0).abs() < 1e-10, "weights sum to {sum}, expected 1.0");
    }

    #[test]
    fn test_evaluate_healthy_tensor() {
        let eval = make_evaluator();
        let report = eval.evaluate(&healthy_tensor(), None);
        assert!(report.is_ok());
        let report = report.unwrap_or_else(|_| unreachable!());
        assert!(report.overall_score > 0.7, "healthy tensor should score > 0.7, got {}", report.overall_score);
        assert!(matches!(report.system_state, SystemState::Healthy | SystemState::Optimal));
    }

    #[test]
    fn test_evaluate_degraded_tensor() {
        let eval = make_evaluator();
        let report = eval.evaluate(&degraded_tensor(), None);
        assert!(report.is_ok());
        let report = report.unwrap_or_else(|_| unreachable!());
        assert!(report.overall_score < 0.7, "degraded tensor should score < 0.7, got {}", report.overall_score);
    }

    #[test]
    fn test_evaluate_zero_tensor() {
        let eval = make_evaluator();
        let report = eval.evaluate(&zero_tensor(), None);
        assert!(report.is_ok());
        let report = report.unwrap_or_else(|_| unreachable!());
        // D9 and D10 are inverted: 0 latency/errors = 1.0 score
        assert!(report.overall_score > 0.0);
    }

    #[test]
    fn test_evaluate_perfect_tensor() {
        let eval = make_evaluator();
        let report = eval.evaluate(&perfect_tensor(), None);
        assert!(report.is_ok());
        let report = report.unwrap_or_else(|_| unreachable!());
        assert!((report.overall_score - 1.0).abs() < 1e-10, "perfect tensor should score 1.0, got {}", report.overall_score);
        assert_eq!(report.system_state, SystemState::Optimal);
    }

    #[test]
    fn test_evaluate_invalid_tensor() {
        let eval = make_evaluator();
        let mut tensor = healthy_tensor();
        tensor.health_score = 1.5;
        assert!(eval.evaluate(&tensor, None).is_err());
    }

    #[test]
    fn test_latency_inversion() {
        let eval = make_evaluator();
        // High latency = low fitness contribution
        let high_lat = Tensor12D::new([0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.9, 0.9, 0.9, 0.9, 0.0, 0.5]);
        let low_lat = Tensor12D::new([0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.9, 0.9, 0.9, 0.1, 0.0, 0.5]);
        let r_high = eval.evaluate(&high_lat, None).unwrap_or_else(|_| unreachable!());
        let _r_low = eval.evaluate(&low_lat, None).unwrap_or_else(|_| unreachable!());
        // D9 inverted: high latency = lower score
        assert!(r_high.dimension_scores[9] < 0.2);
    }

    #[test]
    fn test_error_rate_inversion() {
        let eval = make_evaluator();
        let report = eval.evaluate(&healthy_tensor(), None).unwrap_or_else(|_| unreachable!());
        // error_rate = 0.02 -> inverted = 0.98
        assert!(report.dimension_scores[10] > 0.95);
    }

    #[test]
    fn test_history_recording() {
        let eval = make_evaluator();
        assert_eq!(eval.history_len(), 0);
        let _r = eval.evaluate(&healthy_tensor(), None);
        assert_eq!(eval.history_len(), 1);
        let _r = eval.evaluate(&healthy_tensor(), None);
        assert_eq!(eval.history_len(), 2);
    }

    #[test]
    fn test_history_capacity_enforcement() {
        let config = FitnessConfig {
            history_capacity: 5,
            ..FitnessConfig::default()
        };
        let eval = FitnessEvaluator::with_config(config);
        for _ in 0..10 {
            let _r = eval.evaluate(&healthy_tensor(), None);
        }
        assert_eq!(eval.history_len(), 5);
    }

    #[test]
    fn test_latest_snapshot() {
        let eval = make_evaluator();
        assert!(eval.latest_snapshot().is_none());
        let _r = eval.evaluate(&healthy_tensor(), Some(1));
        let snap = eval.latest_snapshot();
        assert!(snap.is_some());
        let snap = snap.unwrap_or_else(|| unreachable!());
        assert_eq!(snap.generation, Some(1));
    }

    #[test]
    fn test_recent_snapshots() {
        let eval = make_evaluator();
        for _ in 0..5 {
            let _r = eval.evaluate(&healthy_tensor(), None);
        }
        let recent = eval.recent_snapshots(3);
        assert_eq!(recent.len(), 3);
    }

    #[test]
    fn test_current_fitness() {
        let eval = make_evaluator();
        assert!(eval.current_fitness().is_none());
        let _r = eval.evaluate(&healthy_tensor(), None);
        assert!(eval.current_fitness().is_some());
    }

    #[test]
    fn test_current_state() {
        let eval = make_evaluator();
        assert!(eval.current_state().is_none());
        let _r = eval.evaluate(&healthy_tensor(), None);
        let state = eval.current_state();
        assert!(state.is_some());
    }

    #[test]
    fn test_clear_history() {
        let eval = make_evaluator();
        let _r = eval.evaluate(&healthy_tensor(), None);
        assert_eq!(eval.history_len(), 1);
        eval.clear_history();
        assert_eq!(eval.history_len(), 0);
    }

    #[test]
    fn test_dimension_name() {
        assert_eq!(FitnessEvaluator::dimension_name(6), Some("health_score"));
        assert_eq!(FitnessEvaluator::dimension_name(12), None);
    }

    #[test]
    fn test_dimension_names_count() {
        assert_eq!(FitnessEvaluator::dimension_names().len(), 12);
    }

    #[test]
    fn test_dimension_weights_count() {
        assert_eq!(FitnessEvaluator::dimension_weights().len(), 12);
    }

    #[test]
    fn test_average_fitness() {
        let eval = make_evaluator();
        assert!(eval.average_fitness(5).is_none());
        let _r = eval.evaluate(&healthy_tensor(), None);
        let avg = eval.average_fitness(5);
        assert!(avg.is_some());
    }

    #[test]
    fn test_fitness_range() {
        let eval = make_evaluator();
        assert!(eval.fitness_range().is_none());
        let _r = eval.evaluate(&healthy_tensor(), None);
        let _r = eval.evaluate(&degraded_tensor(), None);
        let range = eval.fitness_range();
        assert!(range.is_some());
        let (min, max) = range.unwrap_or_else(|| unreachable!());
        assert!(min < max);
    }

    #[test]
    fn test_fitness_delta() {
        let eval = make_evaluator();
        assert!(eval.fitness_delta().is_none());
        let _r = eval.evaluate(&healthy_tensor(), None);
        assert!(eval.fitness_delta().is_none());
        let _r = eval.evaluate(&degraded_tensor(), None);
        let delta = eval.fitness_delta();
        assert!(delta.is_some());
        let delta = delta.unwrap_or(0.0);
        assert!(delta < 0.0, "healthy -> degraded should have negative delta");
    }

    #[test]
    fn test_system_state_from_fitness() {
        assert_eq!(SystemState::from_fitness(0.95), SystemState::Optimal);
        assert_eq!(SystemState::from_fitness(0.85), SystemState::Healthy);
        assert_eq!(SystemState::from_fitness(0.6), SystemState::Degraded);
        assert_eq!(SystemState::from_fitness(0.4), SystemState::Critical);
        assert_eq!(SystemState::from_fitness(0.1), SystemState::Failed);
    }

    #[test]
    fn test_system_state_boundaries() {
        assert_eq!(SystemState::from_fitness(0.9), SystemState::Optimal);
        assert_eq!(SystemState::from_fitness(0.7), SystemState::Healthy);
        assert_eq!(SystemState::from_fitness(0.5), SystemState::Degraded);
        assert_eq!(SystemState::from_fitness(0.3), SystemState::Critical);
        assert_eq!(SystemState::from_fitness(0.0), SystemState::Failed);
    }

    #[test]
    fn test_trend_unknown_with_no_history() {
        let eval = make_evaluator();
        let report = eval.evaluate(&healthy_tensor(), None).unwrap_or_else(|_| unreachable!());
        assert_eq!(report.fitness_trend, FitnessTrend::Unknown);
    }

    #[test]
    fn test_trend_stable_with_consistent_values() {
        let eval = make_evaluator();
        for _ in 0..15 {
            let _r = eval.evaluate(&healthy_tensor(), None);
        }
        let report = eval.evaluate(&healthy_tensor(), None).unwrap_or_else(|_| unreachable!());
        assert_eq!(report.fitness_trend, FitnessTrend::Stable);
    }

    #[test]
    fn test_stability_high_with_consistent_values() {
        let eval = make_evaluator();
        for _ in 0..15 {
            let _r = eval.evaluate(&healthy_tensor(), None);
        }
        let report = eval.evaluate(&healthy_tensor(), None).unwrap_or_else(|_| unreachable!());
        assert!(report.stability > 0.8, "stability should be high with consistent values, got {}", report.stability);
    }

    #[test]
    fn test_report_has_uuid() {
        let eval = make_evaluator();
        let report = eval.evaluate(&healthy_tensor(), None).unwrap_or_else(|_| unreachable!());
        assert!(!report.id.is_empty());
        assert!(report.id.contains('-'), "expected UUID format");
    }

    #[test]
    fn test_report_tensor_preserved() {
        let eval = make_evaluator();
        let tensor = healthy_tensor();
        let report = eval.evaluate(&tensor, None).unwrap_or_else(|_| unreachable!());
        let expected = tensor.to_array();
        for (i, (&a, &b)) in report.tensor.iter().zip(expected.iter()).enumerate() {
            assert!((a - b).abs() < 1e-10, "dimension {i} mismatch: {a} vs {b}");
        }
    }

    #[test]
    fn test_weighted_contributions_sum_to_score() {
        let eval = make_evaluator();
        let report = eval.evaluate(&healthy_tensor(), None).unwrap_or_else(|_| unreachable!());
        let sum: f64 = report.weighted_contributions.iter().sum();
        assert!((sum - report.overall_score).abs() < 1e-10);
    }

    #[test]
    fn test_config_validation_zero_capacity() {
        let config = FitnessConfig {
            history_capacity: 0,
            ..FitnessConfig::default()
        };
        assert!(FitnessEvaluator::validate_config(&config).is_err());
    }

    #[test]
    fn test_config_validation_zero_trend_window() {
        let config = FitnessConfig {
            trend_window: 0,
            ..FitnessConfig::default()
        };
        assert!(FitnessEvaluator::validate_config(&config).is_err());
    }

    #[test]
    fn test_config_validation_trend_exceeds_capacity() {
        let config = FitnessConfig {
            history_capacity: 5,
            trend_window: 10,
            ..FitnessConfig::default()
        };
        assert!(FitnessEvaluator::validate_config(&config).is_err());
    }

    #[test]
    fn test_config_validation_valid() {
        assert!(FitnessEvaluator::validate_config(&FitnessConfig::default()).is_ok());
    }

    #[test]
    fn test_default_evaluator() {
        let eval = FitnessEvaluator::default();
        assert_eq!(eval.history_len(), 0);
        assert_eq!(eval.config().history_capacity, DEFAULT_HISTORY_CAPACITY);
    }

    #[test]
    fn test_is_improving_insufficient_data() {
        let eval = make_evaluator();
        assert!(!eval.is_improving(5));
    }

    #[test]
    fn test_is_declining_insufficient_data() {
        let eval = make_evaluator();
        assert!(!eval.is_declining(5));
    }

    #[test]
    fn test_primary_dimensions_dominate() {
        let eval = make_evaluator();
        // Only set primary dimensions high, rest low
        let primary_high = Tensor12D::new([0.1, 0.1, 0.1, 0.1, 0.1, 0.1, 0.95, 0.99, 0.95, 0.01, 0.01, 0.1]);
        let report = eval.evaluate(&primary_high, None).unwrap_or_else(|_| unreachable!());
        // Primary dimensions (D6=0.20, D7=0.15, D8=0.15) = 50% of weight
        // With high values, should pull score up
        assert!(report.overall_score > 0.6, "primary dimensions should dominate: {}", report.overall_score);
    }

    #[test]
    fn test_health_dimension_highest_weight() {
        // D6 (health_score) has weight 0.20, the single highest
        assert_eq!(
            DIMENSION_WEIGHTS.iter().enumerate().max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal)).map(|(i, _)| i),
            Some(6)
        );
    }

    #[test]
    fn test_evaluate_records_generation() {
        let eval = make_evaluator();
        let _r = eval.evaluate(&healthy_tensor(), Some(42));
        let snap = eval.latest_snapshot().unwrap_or_else(|| unreachable!());
        assert_eq!(snap.generation, Some(42));
    }

    #[test]
    fn test_evaluate_records_no_generation() {
        let eval = make_evaluator();
        let _r = eval.evaluate(&healthy_tensor(), None);
        let snap = eval.latest_snapshot().unwrap_or_else(|| unreachable!());
        assert_eq!(snap.generation, None);
    }

    #[test]
    fn test_concurrent_evaluation() {
        use std::sync::Arc;
        use std::thread;

        let eval = Arc::new(FitnessEvaluator::new());
        let mut handles = Vec::new();

        for _ in 0..4 {
            let eval_clone = Arc::clone(&eval);
            handles.push(thread::spawn(move || {
                for _ in 0..10 {
                    let _r = eval_clone.evaluate(&healthy_tensor(), None);
                }
            }));
        }

        for handle in handles {
            let _r = handle.join();
        }

        assert_eq!(eval.history_len(), 40);
    }

    #[test]
    fn test_score_monotonic_with_improving_health() {
        let eval = make_evaluator();
        let mut scores = Vec::new();
        for i in 0..10 {
            #[allow(clippy::cast_precision_loss)]
            let health = 0.1 + (i as f64 * 0.09);
            let tensor = Tensor12D::new([0.5, 0.5, 0.5, 0.5, 0.5, 0.5, health, 0.9, 0.9, 0.1, 0.1, 0.5]);
            let report = eval.evaluate(&tensor, None).unwrap_or_else(|_| unreachable!());
            scores.push(report.overall_score);
        }
        // Scores should be monotonically increasing
        for window in scores.windows(2) {
            assert!(window[1] >= window[0] - 1e-10, "score decreased: {} -> {}", window[0], window[1]);
        }
    }

    #[test]
    fn test_config_accessor() {
        let eval = make_evaluator();
        assert_eq!(eval.config().trend_window, DEFAULT_TREND_WINDOW);
    }
}
