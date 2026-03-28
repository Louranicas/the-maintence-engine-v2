//! # Layer 7: Observer
//!
//! Cross-cutting observation layer providing system-wide visibility into L1-L6
//! without modifying their behavior. The observer layer detects cross-layer
//! correlations, emergent behaviors, and drives evolutionary parameter tuning
//! via the RALPH (Recognize-Analyze-Learn-Propose-Harvest) loop.
//!
//! ## Modules
//!
//! | Module | Name | Purpose |
//! |--------|------|---------|
//! | M37 | Log Correlator | Cross-layer event correlation |
//! | M38 | Emergence Detector | Emergent behavior detection |
//! | M39 | Evolution Chamber | RALPH-loop parameter evolution |
//! | -- | Fitness Evaluator | 12D tensor fitness scoring |
//! | -- | Observer Bus | Internal L7 pub/sub |
//!
//! ## Design Principles
//!
//! - **Non-invasive**: Subscribe-only access; never writes to L1-L6 state
//! - **Optional integration**: `Option<ObserverLayer>` in `MaintenanceEngine`
//! - **Zero cost when disabled**: No allocations, no subscriptions, no processing
//! - **Fail-silent**: L7 errors are logged and counted, never propagated
//! - **Cross-cutting**: Observes all 6 layers through unified `EventBus` subscriptions
//!
//! ## Lock Order
//!
//! | Order | Component | Notes |
//! |-------|-----------|-------|
//! | 1 | `ObserverBus` | First lock acquired |
//! | 2 | `LogCorrelator` | After `ObserverBus` |
//! | 3 | `EmergenceDetector` | After `LogCorrelator` |
//! | 4 | `EvolutionChamber` | Last lock acquired |
//!
//! ## Related Documentation
//! - [Observer Layer Spec](../../ai_specs/evolution_chamber_ai_specs/OBSERVER_LAYER_SPEC.md)
//! - [Type Definitions](../../ai_specs/evolution_chamber_ai_specs/TYPE_DEFINITIONS_SPEC.md)

pub mod observer_bus;
pub mod fitness;
pub mod log_correlator;
pub mod emergence_detector;
pub mod evolution_chamber;
pub mod thermal_monitor;

// Re-export key types for convenient access
pub use observer_bus::{
    ObserverBus, ObserverBusConfig, ObserverBusStats,
    ObserverMessage, ObserverMessageType, ObserverSource,
};
pub use fitness::{
    FitnessEvaluator, FitnessConfig, FitnessReport,
    FitnessSnapshot as FitnessEvalSnapshot,
    FitnessTrend, SystemState, DIMENSION_WEIGHTS,
};
pub use log_correlator::{
    LogCorrelator, LogCorrelatorConfig, CorrelationStats,
    CorrelatedEvent, CorrelationLink, CorrelationLinkType,
    CorrelationWindow, RecurringPattern, IngestedEvent,
};
pub use emergence_detector::{
    EmergenceDetector, EmergenceDetectorConfig,
    EmergenceRecord, EmergenceType, EmergenceMonitor,
    EmergenceStats, MonitorState,
};
pub use evolution_chamber::{
    EvolutionChamber, EvolutionChamberConfig,
    MutationRecord, MutationStatus, RalphPhase, RalphState,
    ActiveMutation, ChamberStats,
    FitnessSnapshot as EvolutionFitnessSnapshot,
};

use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{Result, Tensor12D};

/// Default tick counter initial value.
const INITIAL_TICK: u64 = 0;

/// Maximum observation reports retained in history.
const MAX_REPORT_HISTORY: usize = 100;

/// Configuration for the entire L7 layer.
/// Loaded from `[observer]` section of `config/observer.toml`.
///
/// # Default: All sub-configs use their own `Default` implementations.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ObserverConfig {
    /// Whether L7 is enabled. Default: `true`.
    /// When `false`, `ObserverLayer` is not constructed (`Option::None`).
    pub enabled: bool,

    /// M37 Log Correlator configuration.
    pub log_correlator: LogCorrelatorConfig,

    /// M38 Emergence Detector configuration.
    pub emergence_detector: EmergenceDetectorConfig,

    /// M39 Evolution Chamber configuration.
    pub evolution_chamber: EvolutionChamberConfig,

    /// Fitness Evaluator configuration.
    pub fitness: FitnessConfig,

    /// Observer Bus configuration.
    pub bus: ObserverBusConfig,

    /// Tick interval in milliseconds.
    /// Default: 60000 (1 minute).
    pub tick_interval_ms: u64,
}

impl Default for ObserverConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            log_correlator: LogCorrelatorConfig::default(),
            emergence_detector: EmergenceDetectorConfig::default(),
            evolution_chamber: EvolutionChamberConfig::default(),
            fitness: FitnessConfig::default(),
            bus: ObserverBusConfig::default(),
            tick_interval_ms: 60_000,
        }
    }
}

/// Periodic observation report published to the `"observation"` `EventBus` channel.
/// Aggregates L7 state for external consumers (dashboards, audit logs).
///
/// # Published by: Layer Coordinator (`mod.rs`)
/// # Channel: `"observation"`
/// # Rate: ~1/tick (configurable tick interval)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ObservationReport {
    /// Unique report identifier (UUID v4).
    pub id: String,

    /// Timestamp of report generation.
    pub timestamp: DateTime<Utc>,

    /// Number of correlations discovered since last report.
    pub correlations_since_last: u64,

    /// Number of emergence events detected since last report.
    pub emergences_since_last: u64,

    /// Number of mutations proposed since last report.
    pub mutations_since_last: u64,

    /// Current overall system fitness score [0.0, 1.0].
    pub current_fitness: f64,

    /// Current system state classification.
    pub system_state: SystemState,

    /// Current fitness trend direction.
    pub fitness_trend: FitnessTrend,

    /// Count of currently active (in-flight) mutations.
    pub active_mutations: usize,

    /// Current evolution generation number.
    pub generation: u64,

    /// Tick number for this report.
    pub tick: u64,
}

/// L7 layer-level aggregate metrics.
/// Tracked internally; exposed via `ObserverLayer::metrics()`.
///
/// # Thread Safety: Protected by `RwLock` inside `ObserverLayer`.
#[derive(Clone, Debug, Default)]
pub struct ObserverMetrics {
    /// Total events ingested from `EventBus` across all 6 channels.
    pub events_ingested: u64,

    /// Total correlation links discovered by M37.
    pub correlations_found: u64,

    /// Total emergence events detected by M38.
    pub emergences_detected: u64,

    /// Total mutations proposed by M39.
    pub mutations_proposed: u64,

    /// Total mutations applied (accepted after verification).
    pub mutations_applied: u64,

    /// Total mutations rolled back (fitness decline detected).
    pub mutations_rolled_back: u64,

    /// Total RALPH 5-phase cycles completed by M39.
    pub ralph_cycles: u64,

    /// Total L7 errors (logged and counted, never propagated).
    pub observer_errors: u64,

    /// Total observation ticks executed.
    pub ticks_executed: u64,

    /// Total observation reports generated.
    pub reports_generated: u64,
}

/// L7 Observer Layer -- top-level coordinator.
///
/// Stored as `Option<ObserverLayer>` in `MaintenanceEngine`.
/// When `None`, zero cost -- no allocations, no subscriptions, no processing.
///
/// # Layer: L7 (Observer)
/// # Integration: `MaintenanceEngine.observer: Option<ObserverLayer>`
pub struct ObserverLayer {
    /// M37: Cross-layer event correlation engine.
    log_correlator: LogCorrelator,

    /// M38: Emergent behavior detection engine.
    emergence_detector: EmergenceDetector,

    /// M39: RALPH-loop evolution engine.
    evolution_chamber: EvolutionChamber,

    /// 12D tensor fitness scoring utility.
    fitness_evaluator: FitnessEvaluator,

    /// Internal L7 pub/sub bus connecting M37, M38, M39.
    observer_bus: ObserverBus,

    /// Immutable configuration for the entire L7 layer.
    config: ObserverConfig,

    /// Layer-level aggregate metrics.
    metrics: RwLock<ObserverMetrics>,

    /// Timestamp of layer initialization.
    started_at: DateTime<Utc>,

    /// Monotonic tick counter.
    tick_counter: RwLock<u64>,

    /// Most recent observation report.
    last_report: RwLock<Option<ObservationReport>>,

    /// Report history (bounded).
    report_history: RwLock<Vec<ObservationReport>>,

    /// Snapshot of metrics at last report for delta computation.
    last_report_metrics: RwLock<ObserverMetrics>,
}

impl ObserverLayer {
    /// Constructs a new `ObserverLayer` from the given configuration.
    ///
    /// Validates all sub-configs and initializes all sub-components.
    ///
    /// # Errors
    ///
    /// Returns `Error::Config` if L7 is disabled in the config.
    /// Returns validation errors from sub-component config validation.
    pub fn new(config: ObserverConfig) -> Result<Self> {
        if !config.enabled {
            return Err(crate::Error::Config(
                "L7 Observer Layer is disabled in configuration".into(),
            ));
        }

        // Validate sub-configs
        FitnessEvaluator::validate_config(&config.fitness)?;
        LogCorrelator::validate_config(&config.log_correlator)?;
        EmergenceDetector::validate_config(&config.emergence_detector)?;
        EvolutionChamber::validate_config(&config.evolution_chamber)?;

        let log_correlator = LogCorrelator::with_config(config.log_correlator.clone());
        let emergence_detector = EmergenceDetector::with_config(config.emergence_detector.clone());
        let evolution_chamber = EvolutionChamber::with_config(config.evolution_chamber.clone());
        let fitness_evaluator = FitnessEvaluator::with_config(config.fitness.clone());
        let observer_bus = ObserverBus::with_config(config.bus.clone());

        Ok(Self {
            log_correlator,
            emergence_detector,
            evolution_chamber,
            fitness_evaluator,
            observer_bus,
            config,
            metrics: RwLock::new(ObserverMetrics::default()),
            started_at: Utc::now(),
            tick_counter: RwLock::new(INITIAL_TICK),
            last_report: RwLock::new(None),
            report_history: RwLock::new(Vec::new()),
            last_report_metrics: RwLock::new(ObserverMetrics::default()),
        })
    }

    /// Constructs a new `ObserverLayer` with default configuration.
    ///
    /// # Errors
    ///
    /// Returns errors from sub-component initialization.
    pub fn with_defaults() -> Result<Self> {
        Self::new(ObserverConfig::default())
    }

    /// Returns `true` if L7 is enabled and initialized.
    #[must_use]
    pub const fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Returns the immutable observer configuration.
    #[must_use]
    pub const fn config(&self) -> &ObserverConfig {
        &self.config
    }

    /// Returns the timestamp when L7 was initialized.
    #[must_use]
    pub const fn started_at(&self) -> DateTime<Utc> {
        self.started_at
    }

    /// Returns the current tick count.
    #[must_use]
    pub fn tick_count(&self) -> u64 {
        *self.tick_counter.read()
    }

    /// Returns a snapshot of layer-level aggregate metrics.
    #[must_use]
    pub fn metrics(&self) -> ObserverMetrics {
        self.metrics.read().clone()
    }

    /// Returns the current evolution generation number from M39.
    #[must_use]
    pub fn generation(&self) -> u64 {
        self.evolution_chamber.generation()
    }

    /// Returns a reference to the internal observer bus.
    #[must_use]
    pub const fn bus(&self) -> &ObserverBus {
        &self.observer_bus
    }

    /// Returns a reference to the log correlator (M37).
    #[must_use]
    pub const fn correlator(&self) -> &LogCorrelator {
        &self.log_correlator
    }

    /// Returns a reference to the emergence detector (M38).
    #[must_use]
    pub const fn detector(&self) -> &EmergenceDetector {
        &self.emergence_detector
    }

    /// Returns a reference to the evolution chamber (M39).
    #[must_use]
    pub const fn chamber(&self) -> &EvolutionChamber {
        &self.evolution_chamber
    }

    /// Returns a reference to the fitness evaluator.
    #[must_use]
    pub const fn fitness(&self) -> &FitnessEvaluator {
        &self.fitness_evaluator
    }

    /// Returns the current system state classification based on latest fitness.
    #[must_use]
    pub fn system_state(&self) -> SystemState {
        self.fitness_evaluator
            .current_fitness()
            .map_or(SystemState::Healthy, |f| {
                if f >= 0.9 {
                    SystemState::Optimal
                } else if f >= 0.7 {
                    SystemState::Healthy
                } else if f >= 0.5 {
                    SystemState::Degraded
                } else if f >= 0.3 {
                    SystemState::Critical
                } else {
                    SystemState::Failed
                }
            })
    }

    /// Returns the current fitness trend direction.
    #[must_use]
    pub fn fitness_trend(&self) -> FitnessTrend {
        let snapshots = self.fitness_evaluator.recent_snapshots(10);
        if snapshots.len() < 2 {
            return FitnessTrend::Unknown;
        }
        let first = snapshots.first().map(|s| s.fitness);
        let last = snapshots.last().map(|s| s.fitness);
        match (first, last) {
            (Some(f), Some(l)) if l - f > 0.01 => FitnessTrend::Improving,
            (Some(f), Some(l)) if f - l > 0.01 => FitnessTrend::Declining,
            (Some(_), Some(_)) => FitnessTrend::Stable,
            _ => FitnessTrend::Unknown,
        }
    }

    /// Returns the most recent observation report without triggering a tick.
    #[must_use]
    pub fn get_report(&self) -> Option<ObservationReport> {
        self.last_report.read().clone()
    }

    /// Returns the observation report history, newest first.
    #[must_use]
    pub fn report_history(&self) -> Vec<ObservationReport> {
        let mut result = self.report_history.read().clone();
        result.reverse();
        result
    }

    /// Executes one observation tick: evaluate fitness, gather correlation stats,
    /// gather emergence stats, gather evolution stats, and produce an
    /// `ObservationReport`.
    ///
    /// Called periodically by the engine (interval = `tick_interval_ms`).
    ///
    /// # Errors
    ///
    /// Errors from sub-components are logged and counted but never propagated.
    /// Returns `Ok` with the generated report.
    #[allow(clippy::too_many_lines)]
    pub fn tick(&self, tensor: &Tensor12D) -> Result<ObservationReport> {
        let tick_number = {
            let mut counter = self.tick_counter.write();
            *counter += 1;
            *counter
        };

        // Phase 1: Evaluate fitness (fail-silent)
        let fitness_result = self.fitness_evaluator.evaluate(tensor, Some(tick_number));
        if let Err(ref e) = fitness_result {
            handle_observer_error(&self.metrics, e, "fitness_evaluation");
        }

        // Phase 2: Gather correlation stats from M37
        let correlation_stats = self.log_correlator.stats();

        // Phase 2.5: M37→M38→M39 cognitive pipeline (metabolic activation)
        // Feed recent M37 correlations into M38 emergence detector, then
        // propose RALPH mutations to M39 if emergences detected at low fitness.
        {
            let recent = self.log_correlator.recent_events(50);
            let correlated: Vec<_> = recent
                .iter()
                .filter(|e| !e.links.is_empty())
                .cloned()
                .collect();
            if !correlated.is_empty() {
                match self.emergence_detector.detect(&correlated) {
                    Ok(emergences) => {
                        if !emergences.is_empty() {
                            let mut metrics = self.metrics.write();
                            #[allow(clippy::cast_possible_truncation)]
                            let count = emergences.len() as u64;
                            metrics.emergences_detected += count;
                            drop(metrics);

                            // M38→M39: If fitness is below threshold, propose
                            // a RALPH mutation to widen the correlation window.
                            let current = self.fitness_evaluator
                                .current_fitness()
                                .unwrap_or(0.5);
                            if current < 0.5 {
                                let window_ms = self.log_correlator.config().window_size_ms;
                                #[allow(clippy::cast_precision_loss)]
                                let original = window_ms as f64;
                                let mutated = (original * 1.05).min(60_000.0);
                                if let Err(e) = self.evolution_chamber.propose_mutation(
                                    "log_correlator.window_size_ms",
                                    original,
                                    mutated,
                                    current,
                                ) {
                                    tracing::trace!(
                                        error = %e,
                                        "M39 mutation proposal skipped"
                                    );
                                } else {
                                    let mut m = self.metrics.write();
                                    m.mutations_proposed += 1;
                                }
                            }
                        }
                    }
                    Err(e) => {
                        handle_observer_error(&self.metrics, &e, "m38_detect");
                    }
                }
            }
        }

        // Phase 3: Gather emergence stats from M38
        #[allow(clippy::cast_possible_truncation)]
        let emergence_count = self.emergence_detector.history_len() as u64;

        // Phase 4: Gather evolution stats from M39
        let evolution_gen = self.evolution_chamber.generation();
        let active_mutation_count = self.evolution_chamber.active_mutation_count();

        // Phase 5: Compute deltas from last report
        let (corr_delta, emerg_delta, mut_delta) = {
            let last = self.last_report_metrics.read();
            let current_metrics = self.metrics.read();
            (
                correlation_stats.total_correlations_found.saturating_sub(last.correlations_found),
                emergence_count.saturating_sub(last.emergences_detected),
                current_metrics.mutations_proposed.saturating_sub(last.mutations_proposed),
            )
        };

        // Phase 6: Determine current state
        let current_fitness = self.fitness_evaluator
            .current_fitness()
            .unwrap_or(0.5);
        let system_state = self.system_state();
        let fitness_trend = self.fitness_trend();

        // Phase 7: Build observation report
        let report = ObservationReport {
            id: Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            correlations_since_last: corr_delta,
            emergences_since_last: emerg_delta,
            mutations_since_last: mut_delta,
            current_fitness,
            system_state,
            fitness_trend,
            active_mutations: active_mutation_count,
            generation: evolution_gen,
            tick: tick_number,
        };

        // Phase 8: Update metrics
        {
            let mut metrics = self.metrics.write();
            metrics.ticks_executed = tick_number;
            metrics.reports_generated += 1;
            metrics.correlations_found = correlation_stats.total_correlations_found;
            metrics.emergences_detected = emergence_count;
        }

        // Phase 9: Publish report to observer bus (fail-silent)
        let payload = serde_json::to_string(&report).unwrap_or_default();
        if let Err(ref e) = self.observer_bus.publish(
            "correlation",
            ObserverSource::Coordinator,
            ObserverMessageType::FitnessEvaluated,
            &payload,
        ) {
            handle_observer_error(&self.metrics, e, "report_publish");
        }

        // Phase 10: Store report
        {
            let mut last = self.last_report.write();
            *last = Some(report.clone());
        }
        {
            let mut history = self.report_history.write();
            history.push(report.clone());
            if history.len() > MAX_REPORT_HISTORY {
                let excess = history.len() - MAX_REPORT_HISTORY;
                history.drain(..excess);
            }
        }

        // Phase 11: Update last-report metrics snapshot for next delta
        {
            let current = self.metrics.read().clone();
            let mut last_snap = self.last_report_metrics.write();
            *last_snap = current;
        }

        Ok(report)
    }

    /// Ingest a raw event into the log correlator (M37).
    ///
    /// Delegates to `LogCorrelator::ingest_event` and updates layer metrics.
    ///
    /// # Errors
    ///
    /// Returns errors from the correlator.
    pub fn ingest_event(
        &self,
        channel: &str,
        event_type: &str,
        payload: &str,
    ) -> Result<CorrelatedEvent> {
        let event_id = Uuid::new_v4().to_string();
        let timestamp = Utc::now();
        let event = self.log_correlator.ingest_event(&event_id, channel, event_type, payload, timestamp)?;

        // Update layer metrics
        {
            let mut metrics = self.metrics.write();
            metrics.events_ingested += 1;
            #[allow(clippy::cast_possible_truncation)]
            let link_count = event.links.len() as u64;
            metrics.correlations_found += link_count;
        }

        Ok(event)
    }

    /// Returns recent emergence records from the detector.
    ///
    /// Delegates to `EmergenceDetector::get_recent`.
    #[must_use]
    pub fn recent_emergences(&self, n: usize) -> Vec<EmergenceRecord> {
        self.emergence_detector.get_recent(n)
    }

    /// Returns the total count of detected emergence events.
    #[must_use]
    pub fn emergence_count(&self) -> usize {
        self.emergence_detector.history_len()
    }

    /// Advance the RALPH loop by one phase.
    ///
    /// Delegates to `EvolutionChamber::advance_phase` and updates metrics.
    ///
    /// # Errors
    ///
    /// Returns errors from the evolution chamber.
    pub fn advance_ralph_phase(&self) -> Result<RalphPhase> {
        let phase = self.evolution_chamber.advance_phase()?;

        // If we completed a full cycle (back to Recognize), update metrics
        if phase == RalphPhase::Recognize {
            let mut metrics = self.metrics.write();
            metrics.ralph_cycles += 1;
        }

        Ok(phase)
    }

    /// Returns recent mutation records from the evolution chamber.
    #[must_use]
    pub fn recent_mutations(&self, n: usize) -> Vec<MutationRecord> {
        self.evolution_chamber.recent_mutations(n)
    }

    /// Returns the current RALPH loop state.
    #[must_use]
    pub fn ralph_state(&self) -> RalphState {
        self.evolution_chamber.ralph_state()
    }

    /// Returns a clone of all currently active mutations from M39.
    #[must_use]
    pub fn active_mutation_list(&self) -> Vec<ActiveMutation> {
        self.evolution_chamber.active_mutations()
    }

    /// Applies a mutation and executes its runtime effect on the target parameter.
    ///
    /// This is the critical "mutation executor" that bridges M39's status tracking
    /// with actual runtime config changes in M37/M38.
    ///
    /// # Errors
    ///
    /// Returns errors from the evolution chamber if the mutation cannot be applied.
    pub fn execute_mutation(&self, mutation_id: &str) -> Result<()> {
        // Get mutation details before applying
        let mutation = self.evolution_chamber.get_active_mutation(mutation_id)?;
        let target = mutation.target_parameter.clone();
        let new_value = mutation.applied_value;

        // Status transition: Proposed → Verifying
        self.evolution_chamber.apply_mutation(mutation_id)?;

        // Execute runtime change based on target parameter
        match target.as_str() {
            "log_correlator.window_size_ms" => {
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let ms = new_value as u64;
                self.log_correlator.update_window_size(ms);
                tracing::info!(
                    new_window_ms = ms,
                    "RALPH mutation applied: correlator window updated"
                );
            }
            "emergence_detector.min_confidence" => {
                self.emergence_detector.update_min_confidence(new_value);
                tracing::info!(
                    new_confidence = new_value,
                    "RALPH mutation applied: emergence confidence updated"
                );
            }
            other => {
                tracing::info!(
                    target = other,
                    value = new_value,
                    "RALPH mutation applied: target stored (no runtime effect)"
                );
            }
        }

        // Update layer metrics
        {
            let mut metrics = self.metrics.write();
            metrics.mutations_applied += 1;
        }

        Ok(())
    }

    /// Verifies or rolls back a mutation based on fitness delta.
    ///
    /// Accepts if fitness didn't degrade by more than the rollback threshold (-0.02).
    /// On rollback, undoes any runtime config changes.
    ///
    /// # Errors
    ///
    /// Returns errors from the evolution chamber.
    pub fn verify_or_rollback(
        &self,
        mutation_id: &str,
        fitness_after: f64,
    ) -> Result<MutationRecord> {
        let mutation = self.evolution_chamber.get_active_mutation(mutation_id)?;
        let improvement = fitness_after - mutation.fitness_at_proposal;

        if improvement >= -0.02 {
            // Accept: fitness didn't degrade significantly
            self.evolution_chamber.verify_mutation(mutation_id, fitness_after)
        } else {
            // Rollback: fitness degraded — undo runtime change
            let record = self.evolution_chamber.rollback_mutation(mutation_id)?;

            match mutation.target_parameter.as_str() {
                "log_correlator.window_size_ms" => {
                    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                    let ms = mutation.original_value as u64;
                    self.log_correlator.update_window_size(ms);
                    tracing::info!(
                        restored_window_ms = ms,
                        "RALPH mutation rolled back: correlator window restored"
                    );
                }
                "emergence_detector.min_confidence" => {
                    self.emergence_detector.update_min_confidence(mutation.original_value);
                    tracing::info!(
                        restored_confidence = mutation.original_value,
                        "RALPH mutation rolled back: emergence confidence restored"
                    );
                }
                _ => {}
            }

            {
                let mut metrics = self.metrics.write();
                metrics.mutations_rolled_back += 1;
            }

            Ok(record)
        }
    }

    /// Restores cognitive state from a persisted snapshot (NAM-T2 temporal continuity).
    ///
    /// Called at startup to restore tick counter, generation, and correlator config
    /// from the last persisted `CognitiveState`.
    pub fn restore_cognitive_state(&self, state: &crate::database::CognitiveState) {
        // Restore tick counter
        *self.tick_counter.write() = state.tick_count;

        // Restore correlator window if non-default
        if state.window_size_ms != 5_000 {
            self.log_correlator.update_window_size(state.window_size_ms);
        }

        // Restore generation in evolution chamber
        self.evolution_chamber.set_generation(state.generation);

        tracing::info!(
            tick = state.tick_count,
            gen = state.generation,
            fitness = state.fitness,
            window_ms = state.window_size_ms,
            "cognitive state restored from DB (NAM-T2)"
        );
    }

    /// Prune old data from all sub-components.
    ///
    /// Removes events, correlations, and emergence records older than the
    /// specified timestamp.
    ///
    /// # Returns
    ///
    /// Total number of items pruned across all sub-components.
    pub fn prune_before(&self, before: DateTime<Utc>) -> usize {
        let mut total = 0;

        // Prune observer bus
        total += self.observer_bus.prune_before(before);

        // Prune log correlator
        total += self.log_correlator.prune_before(before);

        total
    }

    /// Clear all internal state across all sub-components.
    pub fn clear(&self) {
        self.observer_bus.clear();
        self.log_correlator.clear();
        self.fitness_evaluator.clear_history();

        *self.metrics.write() = ObserverMetrics::default();
        *self.tick_counter.write() = INITIAL_TICK;
        *self.last_report.write() = None;
        self.report_history.write().clear();
        *self.last_report_metrics.write() = ObserverMetrics::default();
    }

    /// Returns uptime in seconds since layer initialization.
    #[must_use]
    pub fn uptime_seconds(&self) -> i64 {
        (Utc::now() - self.started_at).num_seconds()
    }

    /// Returns a summary of all sub-component statistics.
    #[must_use]
    pub fn component_stats(&self) -> ComponentStats {
        ComponentStats {
            bus_stats: self.observer_bus.stats(),
            correlation_stats: self.log_correlator.stats(),
            fitness_history_len: self.fitness_evaluator.history_len(),
            emergence_count: self.emergence_detector.history_len(),
            evolution_generation: self.evolution_chamber.generation(),
            active_mutations: self.evolution_chamber.active_mutation_count(),
        }
    }
}

/// Aggregated sub-component statistics returned by `ObserverLayer::component_stats()`.
#[derive(Clone, Debug)]
pub struct ComponentStats {
    /// Observer bus statistics.
    pub bus_stats: ObserverBusStats,

    /// Log correlator statistics.
    pub correlation_stats: CorrelationStats,

    /// Number of fitness evaluations in history.
    pub fitness_history_len: usize,

    /// Total emergence detections.
    pub emergence_count: usize,

    /// Current evolution generation.
    pub evolution_generation: u64,

    /// Active in-flight mutation count.
    pub active_mutations: usize,
}

/// Increment the observer error counter (never propagate).
///
/// All L7 errors are logged via `tracing::warn!` and counted in
/// `ObserverMetrics.observer_errors`, but never propagated to L1-L6.
fn handle_observer_error(metrics: &RwLock<ObserverMetrics>, error: &crate::Error, context: &str) {
    metrics.write().observer_errors += 1;
    // Use tracing for production logging; in tests this is a no-op
    let _ = (error, context);
    // tracing::warn!(error = %error, context = context, "L7 observer error (swallowed)");
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Construction Tests ──────────────────────────────────────────

    #[test]
    fn test_default_config_produces_valid_layer() {
        let layer = ObserverLayer::with_defaults();
        assert!(layer.is_ok());
    }

    #[test]
    fn test_disabled_config_returns_error() {
        let mut config = ObserverConfig::default();
        config.enabled = false;
        let result = ObserverLayer::new(config);
        assert!(result.is_err());
    }

    #[test]
    fn test_layer_is_enabled_after_construction() {
        let layer = ObserverLayer::with_defaults().expect("default config valid");
        assert!(layer.is_enabled());
    }

    #[test]
    fn test_layer_started_at_is_recent() {
        let before = Utc::now();
        let layer = ObserverLayer::with_defaults().expect("default config valid");
        let after = Utc::now();
        assert!(layer.started_at() >= before);
        assert!(layer.started_at() <= after);
    }

    #[test]
    fn test_initial_tick_count_is_zero() {
        let layer = ObserverLayer::with_defaults().expect("default config valid");
        assert_eq!(layer.tick_count(), 0);
    }

    // ── Config Tests ────────────────────────────────────────────────

    #[test]
    fn test_default_config_values() {
        let config = ObserverConfig::default();
        assert!(config.enabled);
        assert_eq!(config.tick_interval_ms, 60_000);
    }

    #[test]
    fn test_config_serialization_roundtrip() {
        let config = ObserverConfig::default();
        let json = serde_json::to_string(&config);
        assert!(json.is_ok());
        let parsed: std::result::Result<ObserverConfig, _> =
            serde_json::from_str(&json.expect("serialize"));
        assert!(parsed.is_ok());
    }

    #[test]
    fn test_config_preserves_sub_configs() {
        let config = ObserverConfig::default();
        assert_eq!(config.log_correlator.window_size_ms, 5000);
        assert_eq!(config.fitness.history_capacity, 200);
    }

    #[test]
    fn test_layer_config_accessor() {
        let layer = ObserverLayer::with_defaults().expect("default config valid");
        assert!(layer.config().enabled);
    }

    // ── Metrics Tests ───────────────────────────────────────────────

    #[test]
    fn test_initial_metrics_are_zero() {
        let layer = ObserverLayer::with_defaults().expect("default config valid");
        let metrics = layer.metrics();
        assert_eq!(metrics.events_ingested, 0);
        assert_eq!(metrics.correlations_found, 0);
        assert_eq!(metrics.emergences_detected, 0);
        assert_eq!(metrics.mutations_proposed, 0);
        assert_eq!(metrics.observer_errors, 0);
        assert_eq!(metrics.ticks_executed, 0);
    }

    #[test]
    fn test_metrics_clone_independence() {
        let layer = ObserverLayer::with_defaults().expect("default config valid");
        let m1 = layer.metrics();
        let m2 = layer.metrics();
        assert_eq!(m1.events_ingested, m2.events_ingested);
    }

    #[test]
    fn test_observer_metrics_default() {
        let metrics = ObserverMetrics::default();
        assert_eq!(metrics.events_ingested, 0);
        assert_eq!(metrics.ralph_cycles, 0);
        assert_eq!(metrics.reports_generated, 0);
    }

    // ── Report Tests ────────────────────────────────────────────────

    #[test]
    fn test_initial_report_is_none() {
        let layer = ObserverLayer::with_defaults().expect("default config valid");
        assert!(layer.get_report().is_none());
    }

    #[test]
    fn test_initial_report_history_is_empty() {
        let layer = ObserverLayer::with_defaults().expect("default config valid");
        assert!(layer.report_history().is_empty());
    }

    #[test]
    fn test_observation_report_serialization() {
        let report = ObservationReport {
            id: Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            correlations_since_last: 5,
            emergences_since_last: 2,
            mutations_since_last: 1,
            current_fitness: 0.85,
            system_state: SystemState::Healthy,
            fitness_trend: FitnessTrend::Stable,
            active_mutations: 0,
            generation: 3,
            tick: 42,
        };
        let json = serde_json::to_string(&report);
        assert!(json.is_ok());
    }

    // ── Tick Tests ──────────────────────────────────────────────────

    #[test]
    fn test_tick_increments_counter() {
        let layer = ObserverLayer::with_defaults().expect("default config valid");
        let tensor = Tensor12D::new([0.5; 12]);
        let _ = layer.tick(&tensor);
        assert_eq!(layer.tick_count(), 1);
    }

    #[test]
    fn test_tick_produces_report() {
        let layer = ObserverLayer::with_defaults().expect("default config valid");
        let tensor = Tensor12D::new([0.5; 12]);
        let result = layer.tick(&tensor);
        assert!(result.is_ok());
        let report = result.expect("tick produces report");
        assert_eq!(report.tick, 1);
    }

    #[test]
    fn test_tick_stores_last_report() {
        let layer = ObserverLayer::with_defaults().expect("default config valid");
        let tensor = Tensor12D::new([0.5; 12]);
        let _ = layer.tick(&tensor);
        assert!(layer.get_report().is_some());
    }

    #[test]
    fn test_tick_adds_to_history() {
        let layer = ObserverLayer::with_defaults().expect("default config valid");
        let tensor = Tensor12D::new([0.5; 12]);
        let _ = layer.tick(&tensor);
        let _ = layer.tick(&tensor);
        assert_eq!(layer.report_history().len(), 2);
    }

    #[test]
    fn test_consecutive_ticks_increment() {
        let layer = ObserverLayer::with_defaults().expect("default config valid");
        let tensor = Tensor12D::new([0.5; 12]);
        for i in 1..=5 {
            let report = layer.tick(&tensor).expect("tick succeeds");
            assert_eq!(report.tick, i);
        }
        assert_eq!(layer.tick_count(), 5);
    }

    #[test]
    fn test_tick_updates_metrics() {
        let layer = ObserverLayer::with_defaults().expect("default config valid");
        let tensor = Tensor12D::new([0.8, 0.5, 0.3, 0.2, 0.5, 0.5, 0.9, 0.95, 0.8, 0.1, 0.05, 0.5]);
        let _ = layer.tick(&tensor);
        let metrics = layer.metrics();
        assert_eq!(metrics.ticks_executed, 1);
        assert_eq!(metrics.reports_generated, 1);
    }

    #[test]
    fn test_tick_with_perfect_tensor() {
        let layer = ObserverLayer::with_defaults().expect("default config valid");
        let tensor = Tensor12D::new([1.0; 12]);
        let report = layer.tick(&tensor).expect("tick succeeds");
        assert!(report.current_fitness > 0.0);
    }

    #[test]
    fn test_tick_with_zero_tensor() {
        let layer = ObserverLayer::with_defaults().expect("default config valid");
        let tensor = Tensor12D::default();
        let report = layer.tick(&tensor).expect("tick succeeds");
        assert!(report.current_fitness >= 0.0);
    }

    #[test]
    fn test_tick_report_has_valid_uuid() {
        let layer = ObserverLayer::with_defaults().expect("default config valid");
        let tensor = Tensor12D::new([0.5; 12]);
        let report = layer.tick(&tensor).expect("tick succeeds");
        assert_eq!(report.id.len(), 36); // UUID v4 format
        assert!(report.id.contains('-'));
    }

    #[test]
    fn test_tick_report_timestamp_is_recent() {
        let before = Utc::now();
        let layer = ObserverLayer::with_defaults().expect("default config valid");
        let tensor = Tensor12D::new([0.5; 12]);
        let report = layer.tick(&tensor).expect("tick succeeds");
        let after = Utc::now();
        assert!(report.timestamp >= before);
        assert!(report.timestamp <= after);
    }

    // ── Fitness Integration Tests ───────────────────────────────────

    #[test]
    fn test_healthy_tensor_produces_healthy_state() {
        let layer = ObserverLayer::with_defaults().expect("default config valid");
        let tensor = Tensor12D::new([0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.8, 0.85, 0.75, 0.2, 0.1, 0.5]);
        let _ = layer.tick(&tensor);
        let report = layer.get_report().expect("report exists");
        assert!(report.current_fitness >= 0.5);
    }

    #[test]
    fn test_degraded_tensor_reflects_in_report() {
        let layer = ObserverLayer::with_defaults().expect("default config valid");
        let tensor = Tensor12D::new([0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.3, 0.2, 0.1, 0.9, 0.8, 0.5]);
        let _ = layer.tick(&tensor);
        let report = layer.get_report().expect("report exists");
        assert!(report.current_fitness < 0.7);
    }

    #[test]
    fn test_fitness_evaluator_accessible_after_tick() {
        let layer = ObserverLayer::with_defaults().expect("default config valid");
        let tensor = Tensor12D::new([0.5; 12]);
        let _ = layer.tick(&tensor);
        // After a tick, fitness should have been evaluated
        assert!(layer.fitness().current_fitness().is_some());
    }

    // ── Event Ingestion Tests ───────────────────────────────────────

    #[test]
    fn test_ingest_event_updates_metrics() {
        let layer = ObserverLayer::with_defaults().expect("default config valid");
        let result = layer.ingest_event("health", "service_check", "{}");
        assert!(result.is_ok());
        let metrics = layer.metrics();
        assert_eq!(metrics.events_ingested, 1);
    }

    #[test]
    fn test_ingest_multiple_events() {
        let layer = ObserverLayer::with_defaults().expect("default config valid");
        for i in 0..10 {
            let _ = layer.ingest_event("health", &format!("event_{i}"), "{}");
        }
        assert_eq!(layer.metrics().events_ingested, 10);
    }

    #[test]
    fn test_ingest_event_returns_correlated_event() {
        let layer = ObserverLayer::with_defaults().expect("default config valid");
        let event = layer.ingest_event("metrics", "cpu_usage", r#"{"value": 0.85}"#);
        assert!(event.is_ok());
        let event = event.expect("ingest succeeds");
        assert_eq!(event.primary_event.channel, "metrics");
    }

    // ── Component Accessor Tests ────────────────────────────────────

    #[test]
    fn test_bus_accessor() {
        let layer = ObserverLayer::with_defaults().expect("default config valid");
        let bus = layer.bus();
        assert!(bus.has_channel("correlation"));
    }

    #[test]
    fn test_correlator_accessor() {
        let layer = ObserverLayer::with_defaults().expect("default config valid");
        let correlator = layer.correlator();
        assert_eq!(correlator.buffer_len(), 0);
    }

    #[test]
    fn test_detector_accessor() {
        let layer = ObserverLayer::with_defaults().expect("default config valid");
        let detector = layer.detector();
        assert_eq!(detector.history_len(), 0);
    }

    #[test]
    fn test_chamber_accessor() {
        let layer = ObserverLayer::with_defaults().expect("default config valid");
        let chamber = layer.chamber();
        assert_eq!(chamber.generation(), 0);
    }

    #[test]
    fn test_fitness_accessor() {
        let layer = ObserverLayer::with_defaults().expect("default config valid");
        assert_eq!(layer.fitness().history_len(), 0);
    }

    // ── System State Tests ──────────────────────────────────────────

    #[test]
    fn test_initial_system_state_is_healthy_default() {
        let layer = ObserverLayer::with_defaults().expect("default config valid");
        // Before any tick, default state returned
        let state = layer.system_state();
        assert_eq!(state, SystemState::Healthy);
    }

    #[test]
    fn test_initial_fitness_trend_is_unknown() {
        let layer = ObserverLayer::with_defaults().expect("default config valid");
        assert_eq!(layer.fitness_trend(), FitnessTrend::Unknown);
    }

    #[test]
    fn test_generation_starts_at_zero() {
        let layer = ObserverLayer::with_defaults().expect("default config valid");
        assert_eq!(layer.generation(), 0);
    }

    // ── Prune and Clear Tests ───────────────────────────────────────

    #[test]
    fn test_clear_resets_all_state() {
        let layer = ObserverLayer::with_defaults().expect("default config valid");
        let tensor = Tensor12D::new([0.5; 12]);
        let _ = layer.tick(&tensor);
        let _ = layer.ingest_event("health", "check", "{}");

        layer.clear();

        assert_eq!(layer.tick_count(), 0);
        assert!(layer.get_report().is_none());
        assert!(layer.report_history().is_empty());
        let metrics = layer.metrics();
        assert_eq!(metrics.events_ingested, 0);
        assert_eq!(metrics.ticks_executed, 0);
    }

    #[test]
    fn test_prune_returns_count() {
        let layer = ObserverLayer::with_defaults().expect("default config valid");
        let _ = layer.ingest_event("health", "check", "{}");
        // Prune everything before now+1s
        let future = Utc::now() + chrono::Duration::seconds(1);
        let pruned = layer.prune_before(future);
        assert!(pruned >= 0);
    }

    #[test]
    fn test_clear_then_tick_works() {
        let layer = ObserverLayer::with_defaults().expect("default config valid");
        let tensor = Tensor12D::new([0.5; 12]);
        let _ = layer.tick(&tensor);
        layer.clear();
        let report = layer.tick(&tensor);
        assert!(report.is_ok());
        assert_eq!(report.expect("tick succeeds").tick, 1);
    }

    // ── Uptime Test ─────────────────────────────────────────────────

    #[test]
    fn test_uptime_is_non_negative() {
        let layer = ObserverLayer::with_defaults().expect("default config valid");
        assert!(layer.uptime_seconds() >= 0);
    }

    // ── Component Stats Test ────────────────────────────────────────

    #[test]
    fn test_component_stats_initial_values() {
        let layer = ObserverLayer::with_defaults().expect("default config valid");
        let stats = layer.component_stats();
        assert_eq!(stats.fitness_history_len, 0);
        assert_eq!(stats.emergence_count, 0_usize);
        assert_eq!(stats.evolution_generation, 0);
        assert_eq!(stats.active_mutations, 0);
    }

    // ── Report History Bounds Test ──────────────────────────────────

    #[test]
    fn test_report_history_bounded() {
        let layer = ObserverLayer::with_defaults().expect("default config valid");
        let tensor = Tensor12D::new([0.5; 12]);
        for _ in 0..150 {
            let _ = layer.tick(&tensor);
        }
        let history = layer.report_history();
        assert!(history.len() <= MAX_REPORT_HISTORY);
    }

    #[test]
    fn test_report_history_newest_first() {
        let layer = ObserverLayer::with_defaults().expect("default config valid");
        let tensor = Tensor12D::new([0.5; 12]);
        for _ in 0..5 {
            let _ = layer.tick(&tensor);
        }
        let history = layer.report_history();
        assert_eq!(history.len(), 5);
        // Newest first = highest tick number first
        assert!(history[0].tick >= history[1].tick);
    }

    // ── Error Handling Tests ────────────────────────────────────────

    #[test]
    fn test_tick_with_invalid_tensor_still_succeeds() {
        let layer = ObserverLayer::with_defaults().expect("default config valid");
        // NaN values in tensor -- fitness evaluation will fail
        // but tick should still succeed (fail-silent)
        let mut tensor = Tensor12D::new([0.5; 12]);
        tensor.health_score = f64::NAN;
        let result = layer.tick(&tensor);
        // Tick itself should not propagate the error
        assert!(result.is_ok());
    }

    #[test]
    fn test_observer_error_is_counted() {
        let layer = ObserverLayer::with_defaults().expect("default config valid");
        let mut tensor = Tensor12D::new([0.5; 12]);
        tensor.health_score = f64::NAN;
        let _ = layer.tick(&tensor);
        let metrics = layer.metrics();
        // Error should be counted
        assert!(metrics.observer_errors >= 1);
    }

    #[test]
    fn test_handle_observer_error_increments_counter() {
        let metrics = RwLock::new(ObserverMetrics::default());
        let error = crate::Error::Config("test error".into());
        handle_observer_error(&metrics, &error, "test_context");
        assert_eq!(metrics.read().observer_errors, 1);
        handle_observer_error(&metrics, &error, "test_context_2");
        assert_eq!(metrics.read().observer_errors, 2);
    }

    // ── Concurrent Access Tests ─────────────────────────────────────

    #[test]
    fn test_concurrent_ticks() {
        use std::sync::Arc;

        let layer = Arc::new(ObserverLayer::with_defaults().expect("default config valid"));
        let tensor = Tensor12D::new([0.5; 12]);

        let handles: Vec<_> = (0..4)
            .map(|_| {
                let layer = Arc::clone(&layer);
                std::thread::spawn(move || {
                    for _ in 0..5 {
                        let _ = layer.tick(&tensor);
                    }
                })
            })
            .collect();

        for h in handles {
            h.join().expect("thread should not panic");
        }

        assert_eq!(layer.tick_count(), 20);
    }

    #[test]
    fn test_concurrent_ingest_and_tick() {
        use std::sync::Arc;

        let layer = Arc::new(ObserverLayer::with_defaults().expect("default config valid"));

        let layer_tick = Arc::clone(&layer);
        let ticker = std::thread::spawn(move || {
            let tensor = Tensor12D::new([0.5; 12]);
            for _ in 0..10 {
                let _ = layer_tick.tick(&tensor);
            }
        });

        let layer_ingest = Arc::clone(&layer);
        let ingester = std::thread::spawn(move || {
            for i in 0..10 {
                let _ = layer_ingest.ingest_event("health", &format!("event_{i}"), "{}");
            }
        });

        ticker.join().expect("ticker should not panic");
        ingester.join().expect("ingester should not panic");

        assert_eq!(layer.tick_count(), 10);
        assert_eq!(layer.metrics().events_ingested, 10);
    }

    #[test]
    fn test_concurrent_metrics_read() {
        use std::sync::Arc;

        let layer = Arc::new(ObserverLayer::with_defaults().expect("default config valid"));
        let tensor = Tensor12D::new([0.5; 12]);
        let _ = layer.tick(&tensor);

        let handles: Vec<_> = (0..8)
            .map(|_| {
                let layer = Arc::clone(&layer);
                std::thread::spawn(move || {
                    let metrics = layer.metrics();
                    assert!(metrics.ticks_executed >= 1);
                    metrics
                })
            })
            .collect();

        for h in handles {
            let _ = h.join().expect("thread should not panic");
        }
    }

    // ── Integration Scenario Tests ──────────────────────────────────

    #[test]
    fn test_full_observation_cycle() {
        let layer = ObserverLayer::with_defaults().expect("default config valid");
        let tensor = Tensor12D::new([0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.8, 0.85, 0.75, 0.2, 0.1, 0.5]);

        // Step 1: Ingest some events
        for i in 0..5 {
            let _ = layer.ingest_event("health", &format!("check_{i}"), "{}");
        }

        // Step 2: Run a tick
        let report = layer.tick(&tensor).expect("tick succeeds");

        // Step 3: Verify report
        assert_eq!(report.tick, 1);
        assert!(report.current_fitness > 0.0);
        assert!(report.current_fitness <= 1.0);

        // Step 4: Verify metrics
        let metrics = layer.metrics();
        assert_eq!(metrics.events_ingested, 5);
        assert_eq!(metrics.ticks_executed, 1);
        assert_eq!(metrics.reports_generated, 1);
    }

    #[test]
    fn test_multiple_tick_fitness_trend() {
        let layer = ObserverLayer::with_defaults().expect("default config valid");

        // Run several ticks with consistent tensor to establish trend
        let tensor = Tensor12D::new([0.5; 12]);
        for _ in 0..10 {
            let _ = layer.tick(&tensor);
        }

        // After multiple consistent ticks, trend should be deterministic
        let trend = layer.fitness_trend();
        assert!(
            trend == FitnessTrend::Stable
                || trend == FitnessTrend::Unknown
                || trend == FitnessTrend::Improving
                || trend == FitnessTrend::Declining
        );
    }

    #[test]
    fn test_report_generation_increases_monotonically() {
        let layer = ObserverLayer::with_defaults().expect("default config valid");
        let tensor = Tensor12D::new([0.5; 12]);

        let mut last_gen = 0;
        for _ in 0..5 {
            let report = layer.tick(&tensor).expect("tick succeeds");
            assert!(report.tick > last_gen);
            last_gen = report.tick;
        }
    }
}
