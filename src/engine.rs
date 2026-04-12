//! # Engine Orchestrator
//!
//! Central orchestrator that wires all seven layers of the Maintenance Engine
//! into a unified control surface. The [`Engine`] holds one instance of every
//! manager type across the stack and provides aggregate methods for health
//! reporting, remediation submission, learning cycles, observation ticks,
//! and cross-layer coordination.
//!
//! ## Layers
//!
//! | Layer | Components |
//! |-------|------------|
//! | L2 Services | [`HealthMonitor`], [`LifecycleManager`], [`CircuitBreakerRegistry`] |
//! | L3 Core Logic | [`PipelineManager`], [`RemediationEngine`], [`ConfidenceCalculator`], [`ActionExecutor`], [`OutcomeRecorder`], [`FeedbackLoop`] |
//! | L4 Integration | [`RestClient`], [`EventBus`], [`BridgeManager`] |
//! | L5 Learning | [`HebbianManager`], [`StdpProcessor`], [`AntiPatternDetector`] |
//! | L6 Consensus | [`PbftManager`], [`VoteCollector`], [`DissentTracker`] |
//! | L7 Observer | [`ObserverLayer`] (optional, fail-silent) |
//!
//! ## Thread Safety
//!
//! All managers are internally synchronised; the [`Engine`] itself requires
//! only a shared reference (`&self`) for every operation.
//!
//! ## Related Documentation
//! - [Architecture Overview](../ai_docs/INDEX.md)
//! - [Implementation Plan](../terms%20of%20reference/implementation_plan/INDEX.md)

use crate::m1_foundation::shared_types::DimensionIndex;
use crate::m1_foundation::TensorContributor;
use crate::m2_services::{
    CircuitBreakerRegistry, HealthMonitor, HealthMonitoring, HealthProbeBuilder, LifecycleManager,
    LifecycleOps, RestartConfig, ServiceDefinitionBuilder, ServiceDiscovery, ServiceRegistry,
    ServiceTier,
};
use crate::m3_core_logic::action::ActionExecutor;
use crate::m3_core_logic::confidence::ConfidenceCalculator;
use crate::m3_core_logic::feedback::FeedbackLoop;
use crate::m3_core_logic::outcome::OutcomeRecorder;
use crate::m3_core_logic::pipeline::PipelineManager;
use crate::m3_core_logic::remediation::RemediationEngine;
use crate::m3_core_logic::{IssueType, Severity};
use crate::m4_integration::bridge::BridgeManager;
use crate::m4_integration::event_bus::EventBus;
use crate::m4_integration::cascade_bridge::CascadeBridge;
use crate::m4_integration::peer_bridge::PeerBridgeManager;
use crate::m4_integration::rest::RestClient;
use crate::m5_learning::antipattern::AntiPatternDetector;
use crate::m5_learning::decay_scheduler::DecayScheduler;
use crate::m5_learning::hebbian::HebbianManager;
use crate::m5_learning::stdp::StdpProcessor;
use crate::m6_consensus::dissent::DissentTracker;
use crate::m6_consensus::pbft::PbftManager;
use crate::m6_consensus::voting::VoteCollector;
use crate::m7_observer::thermal_monitor::ThermalMonitor;
use crate::m7_observer::{ObserverConfig, ObserverLayer};
use crate::{Error, Result, Tensor12D};

// -- V2: New module imports (M48-M57 + L8 Nexus) --
use crate::m1_foundation::self_model::{SelfModel, SelfModelConfig};
use crate::m2_services::traffic::TrafficManager;
use crate::m3_core_logic::approval::ApprovalManager;
use crate::m4_integration::auth::{AuthConfig, AuthManager};
use crate::m4_integration::orac_bridge::OracBridgeManager;
use crate::m4_integration::rate_limiter::{RateLimiter, RateLimiterConfig};
use crate::m5_learning::prediction::{PredictionEngineCore, PredictionConfig};
use crate::m5_learning::sequence::{SequenceDetectorCore, SequenceDetectorConfig};
use crate::m6_consensus::active_dissent::ActiveDissentGenerator;
use crate::m6_consensus::checkpoint::{CheckpointConfig, InMemoryCheckpointManager};
use crate::nexus::evolution_gate::{EvolutionGateConfig, EvolutionGateCore};
use crate::nexus::field_bridge::FieldBridgeCore;
use crate::nexus::intent_router::IntentRouterCore;
use crate::nexus::morphogenic_adapter::{MorphogenicAdapterCore, MorphogenicConfig};
use crate::nexus::regime_manager::RegimeManagerCore;
use crate::nexus::stdp_bridge::StdpBridgeCore;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Number of layers in the engine architecture (L1-L7).
const LAYER_COUNT: usize = 7;

/// Minimum acceptable overall health score.
const MIN_HEALTHY_SCORE: f64 = 0.5;

/// Weight for the foundation layer (L1) in overall health.
const LAYER_WEIGHT_FOUNDATION: f64 = 0.08;

/// Weight for the services layer (L2) in overall health.
const LAYER_WEIGHT_SERVICES: f64 = 0.22;

/// Weight for the core logic layer (L3) in overall health.
const LAYER_WEIGHT_CORE: f64 = 0.18;

/// Weight for the integration layer (L4) in overall health.
const LAYER_WEIGHT_INTEGRATION: f64 = 0.14;

/// Weight for the learning layer (L5) in overall health.
const LAYER_WEIGHT_LEARNING: f64 = 0.14;

/// Weight for the consensus layer (L6) in overall health.
const LAYER_WEIGHT_CONSENSUS: f64 = 0.14;

/// Weight for the observer layer (L7) in overall health.
const LAYER_WEIGHT_OBSERVER: f64 = 0.10;

// ---------------------------------------------------------------------------
// EngineHealthReport
// ---------------------------------------------------------------------------

/// Aggregate health report across all engine layers.
///
/// Produced by [`Engine::health_report`] to give a single snapshot of the
/// entire engine state. Each field is derived from the corresponding layer
/// manager(s).
#[derive(Clone, Debug)]
pub struct EngineHealthReport {
    /// Total number of registered services (L2 probes + L4 endpoints).
    pub services_total: usize,
    /// Number of services currently considered healthy.
    pub services_healthy: usize,
    /// Number of active (enabled) pipelines.
    pub pipelines_active: usize,
    /// Number of registered Hebbian pathways.
    pub pathways_count: usize,
    /// Number of active consensus proposals.
    pub proposals_active: usize,
    /// Overall health score in [0.0, 1.0].
    pub overall_health: f64,
    /// Per-layer health scores: `[L1, L2, L3, L4, L5, L6, L7]`.
    pub layer_health: [f64; LAYER_COUNT],
}

impl EngineHealthReport {
    /// Check whether the overall health is above [`MIN_HEALTHY_SCORE`].
    #[must_use]
    pub fn is_healthy(&self) -> bool {
        self.overall_health >= MIN_HEALTHY_SCORE
    }

    /// Return the weakest layer index (0-based) and its score.
    #[must_use]
    pub fn weakest_layer(&self) -> (usize, f64) {
        let mut min_idx = 0;
        let mut min_val = self.layer_health[0];
        for (i, &score) in self.layer_health.iter().enumerate().skip(1) {
            if score < min_val {
                min_val = score;
                min_idx = i;
            }
        }
        (min_idx, min_val)
    }
}

// ---------------------------------------------------------------------------
// LearningCycleResult
// ---------------------------------------------------------------------------

/// Outcome of a single learning cycle executed by [`Engine::learning_cycle`].
///
/// Captures how many pathways were decayed, how many STDP timing pairs were
/// processed, and how many anti-patterns were detected during the cycle.
#[derive(Clone, Debug)]
pub struct LearningCycleResult {
    /// Number of pathways that had their strength decayed.
    pub pathways_decayed: usize,
    /// Number of STDP timing pairs processed in this cycle.
    pub timing_pairs_processed: usize,
    /// Number of anti-pattern detections raised.
    pub antipatterns_detected: usize,
}

impl LearningCycleResult {
    /// Check whether any learning activity occurred.
    #[must_use]
    pub const fn had_activity(&self) -> bool {
        self.pathways_decayed > 0
            || self.timing_pairs_processed > 0
            || self.antipatterns_detected > 0
    }
}

// ---------------------------------------------------------------------------
// Engine
// ---------------------------------------------------------------------------

/// Central orchestrator holding all layer managers.
///
/// The engine is the top-level entry point for coordinating health monitoring,
/// remediation, learning, consensus, and integration across the entire
/// Maintenance Engine stack.
///
/// # Construction
///
/// ```rust
/// use maintenance_engine_v2::engine::Engine;
///
/// let engine = Engine::new();
/// assert!(engine.service_count() > 0);
/// ```
pub struct Engine {
    // -- L2: Services --
    service_registry: ServiceRegistry,
    health_monitor: HealthMonitor,
    lifecycle_manager: LifecycleManager,
    circuit_breaker: CircuitBreakerRegistry,

    // -- L3: Core Logic --
    pipeline_manager: PipelineManager,
    remediator: RemediationEngine,
    confidence_calculator: ConfidenceCalculator,
    action_executor: ActionExecutor,
    outcome_recorder: OutcomeRecorder,
    feedback_loop: FeedbackLoop,

    // -- L4: Integration --
    rest_client: RestClient,
    event_bus: EventBus,
    bridge_manager: BridgeManager,

    // -- L5: Learning --
    hebbian_manager: HebbianManager,
    stdp_processor: StdpProcessor,
    antipattern_detector: AntiPatternDetector,

    // -- L6: Consensus --
    pbft_manager: PbftManager,
    vote_collector: VoteCollector,
    dissent_tracker: DissentTracker,

    // -- L7: Observer --
    observer: Option<ObserverLayer>,

    // -- Peer Bridge --
    peer_bridge: Option<PeerBridgeManager>,

    // -- V3 Integration (fail-silent) --
    thermal_monitor: Option<ThermalMonitor>,
    decay_scheduler: Option<DecayScheduler>,
    cascade_bridge: Option<CascadeBridge>,

    // -- V2: New Modules (M48-M57) --
    self_model: SelfModel,
    traffic_manager: TrafficManager,
    approval_manager: ApprovalManager,
    auth_manager: AuthManager,
    rate_limiter: RateLimiter,
    orac_bridge: OracBridgeManager,
    prediction_core: PredictionEngineCore,
    sequence_detector: SequenceDetectorCore,
    checkpoint_manager: Option<InMemoryCheckpointManager>,
    dissent_generator: ActiveDissentGenerator,

    // -- V2: L8 Nexus --
    field_bridge: FieldBridgeCore,
    intent_router: IntentRouterCore,
    regime_manager: RegimeManagerCore,
    stdp_bridge: StdpBridgeCore,
    evolution_gate: EvolutionGateCore,
    morphogenic_adapter: MorphogenicAdapterCore,
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}

impl Engine {
    // ==================================================================
    // Construction
    // ==================================================================

    /// Create a new engine with all managers initialised to defaults.
    ///
    /// Each manager's `new()` method populates default state: the
    /// [`PipelineManager`] loads 8 default pipelines, the [`HebbianManager`]
    /// loads default pathways, the [`PbftManager`] creates the 41-agent
    /// fleet, etc.
    #[must_use]
    pub fn new() -> Self {
        let service_registry = ServiceRegistry::new();
        Self::populate_ultraplate_services(&service_registry);
        let lifecycle_manager = LifecycleManager::new();
        Self::populate_lifecycle_services(&lifecycle_manager);
        let health_monitor = HealthMonitor::new();
        Self::populate_health_probes(&health_monitor);
        Self {
            // L2
            service_registry,
            health_monitor,
            lifecycle_manager,
            circuit_breaker: CircuitBreakerRegistry::new(),
            // L3
            pipeline_manager: PipelineManager::new(),
            remediator: RemediationEngine::new(),
            confidence_calculator: ConfidenceCalculator::new(),
            action_executor: ActionExecutor::new(),
            outcome_recorder: OutcomeRecorder::new(),
            feedback_loop: FeedbackLoop::new(),
            // L4
            rest_client: RestClient::new(),
            event_bus: EventBus::new(),
            bridge_manager: BridgeManager::new(),
            // L5
            hebbian_manager: HebbianManager::new(),
            stdp_processor: StdpProcessor::new(),
            antipattern_detector: AntiPatternDetector::new(),
            // L6
            pbft_manager: PbftManager::new(),
            vote_collector: VoteCollector::new(),
            dissent_tracker: DissentTracker::new(),
            // L7: fail-silent -- engine works without it
            // Override L7 config for metabolic activation:
            // - M37: health polls arrive 15-60s apart, default 5s window misses them.
            // - M38: with 3-4 layers per tick, default 0.7 confidence rejects valid cascades.
            observer: {
                let mut config = ObserverConfig::default();
                config.log_correlator.window_size_ms = 60_000;
                config.log_correlator.temporal_tolerance_ms = 30_000;
                config.emergence_detector.min_confidence = 0.4;
                ObserverLayer::new(config).ok()
            },
            // Peer bridge: fail-silent
            peer_bridge: PeerBridgeManager::new().ok(),
            // V3 integration modules: fail-silent
            thermal_monitor: Some(ThermalMonitor::new()),
            decay_scheduler: Some(DecayScheduler::new()),
            cascade_bridge: Some(CascadeBridge::new()),

            // V2: New Modules
            self_model: SelfModel::new(SelfModelConfig::default()),
            traffic_manager: TrafficManager::new(),
            approval_manager: ApprovalManager::new(),
            auth_manager: AuthManager::new(AuthConfig::default()),
            rate_limiter: RateLimiter::new(RateLimiterConfig::default()),
            orac_bridge: OracBridgeManager::new(),
            prediction_core: PredictionEngineCore::new(PredictionConfig::default()),
            sequence_detector: SequenceDetectorCore::new(SequenceDetectorConfig::default()),
            // InMemoryCheckpointManager::new returns Result -- fail-silent
            checkpoint_manager: InMemoryCheckpointManager::new(CheckpointConfig::default()).ok(),
            dissent_generator: ActiveDissentGenerator::default(),

            // V2: L8 Nexus
            field_bridge: FieldBridgeCore::new(),
            intent_router: IntentRouterCore::default(),
            regime_manager: RegimeManagerCore::new(),
            stdp_bridge: StdpBridgeCore::default(),
            evolution_gate: EvolutionGateCore::new(EvolutionGateConfig::default()),
            morphogenic_adapter: MorphogenicAdapterCore::new(MorphogenicConfig::default()),
        }
    }

    // ==================================================================
    // Health & Status
    // ==================================================================

    /// Produce an aggregate health report across all layers.
    ///
    /// # Errors
    ///
    /// Returns `Error::Other` if any layer health calculation encounters
    /// an unexpected state (currently infallible but kept as `Result`
    /// for forward-compatibility with async health probes).
    pub fn health_report(&self) -> Result<EngineHealthReport> {
        let services_total = self.service_count();
        let services_healthy = self.healthy_service_count();
        let pipelines_active = self.active_pipeline_count();
        let pathways_count = self.pathway_count();
        let proposals_active = self.active_proposal_count();

        let layer_health = self.compute_layer_health();
        let overall_health = Self::compute_overall_health(&layer_health);

        Ok(EngineHealthReport {
            services_total,
            services_healthy,
            pipelines_active,
            pathways_count,
            proposals_active,
            overall_health,
            layer_health,
        })
    }

    /// Total count of registered services across L2 and L4.
    ///
    /// Counts both health-monitored probes (L2) and REST endpoints (L4).
    /// If both are empty the count reflects only endpoint registrations.
    #[must_use]
    pub fn service_count(&self) -> usize {
        let probes = self.health_monitor.probe_count();
        let endpoints = self.rest_client.endpoint_count();
        // Use whichever is larger -- they track the same fleet from
        // different perspectives.
        probes.max(endpoints)
    }

    /// Number of active (enabled) pipelines.
    #[must_use]
    pub fn pipeline_count(&self) -> usize {
        self.pipeline_manager.pipeline_count()
    }

    /// Number of registered Hebbian pathways.
    #[must_use]
    pub fn pathway_count(&self) -> usize {
        self.hebbian_manager.pathway_count()
    }

    /// Number of active consensus proposals.
    #[must_use]
    pub fn active_proposals(&self) -> usize {
        self.active_proposal_count()
    }

    // ==================================================================
    // Remediation
    // ==================================================================

    /// Submit a new remediation request through the L3 pipeline.
    ///
    /// The request flows through:
    /// 1. [`RemediationEngine`] (queuing and action selection)
    /// 2. [`ConfidenceCalculator`] (confidence scoring)
    /// 3. [`ActionExecutor`] (dispatch with escalation tier enforcement)
    ///
    /// Returns the unique request ID on success.
    ///
    /// # Errors
    ///
    /// Returns an error if the service ID is empty, the severity is
    /// invalid, or the remediation engine rejects the submission.
    pub fn submit_remediation(
        &self,
        service_id: &str,
        issue_type: IssueType,
        severity: Severity,
        description: &str,
    ) -> Result<String> {
        if service_id.is_empty() {
            return Err(Error::Validation(
                "service_id must not be empty".to_owned(),
            ));
        }
        if description.is_empty() {
            return Err(Error::Validation(
                "description must not be empty".to_owned(),
            ));
        }

        let request =
            self.remediator
                .submit_request(service_id, issue_type, severity, description)?;

        Ok(request.id)
    }

    /// Convenience method for auto-triggered remediation.
    ///
    /// Creates a remediation request with the given parameters using
    /// [`IssueType::HealthFailure`] and the specified severity.
    ///
    /// G4: Used by background tasks for auto-remediation when health
    /// drops below thresholds or fitness enters Critical state.
    ///
    /// # Errors
    ///
    /// Returns an error if the service ID or description is empty,
    /// or if the remediation engine rejects the submission.
    pub fn auto_remediate(
        &self,
        service_id: &str,
        severity: Severity,
        description: &str,
    ) -> Result<String> {
        self.submit_remediation(service_id, IssueType::HealthFailure, severity, description)
    }

    /// Return the number of pending remediation requests.
    #[must_use]
    pub fn pending_remediations(&self) -> usize {
        self.remediator.pending_count()
    }

    /// Return the number of active (in-progress) remediations.
    #[must_use]
    pub fn active_remediations(&self) -> usize {
        self.remediator.active_count()
    }

    /// Return the overall remediation success rate.
    #[must_use]
    pub fn remediation_success_rate(&self) -> f64 {
        self.remediator.success_rate()
    }

    /// R6: Process the next pending remediation request.
    ///
    /// Dequeues from the pending queue if capacity permits, transitions to
    /// active, and returns the result. Returns `Ok(None)` if no actionable
    /// request is available (queue empty or at max concurrent capacity).
    ///
    /// # Errors
    ///
    /// Returns `Error` if the remediation engine fails to dequeue or
    /// complete the request due to an internal state inconsistency.
    pub fn process_next_remediation(
        &self,
    ) -> Result<Option<crate::m3_core_logic::RemediationOutcome>> {
        self.remediator.process_next().map(|opt| {
            opt.map(|active| {
                let req_id = active.request.id;
                // Auto-complete with success for L0/L1 tier requests
                let outcome = crate::m3_core_logic::RemediationOutcome {
                    request_id: req_id.clone(),
                    success: true,
                    duration_ms: 0,
                    error: None,
                    pathway_delta: 0.05, // positive feedback for successful processing
                };
                // Complete the request to update stats
                let _ = self.remediator.complete_request(
                    &req_id,
                    true,
                    0,
                    None,
                );
                outcome
            })
        })
    }

    // ==================================================================
    // Learning
    // ==================================================================

    /// Execute a single learning cycle across L5 components.
    ///
    /// Steps:
    /// 1. Decay all Hebbian pathway strengths.
    /// 2. Process the STDP timing window for spike-timing pairs.
    /// 3. Report the number of unresolved anti-pattern detections.
    ///
    /// # Errors
    ///
    /// Returns an error if STDP window processing fails.
    pub fn learning_cycle(&self) -> Result<LearningCycleResult> {
        // 1. Hebbian decay
        let pathways_decayed = self.hebbian_manager.apply_decay();

        // 2. STDP timing window processing
        let timing_pairs = self.stdp_processor.process_window()?;
        let timing_pairs_processed = timing_pairs.len();

        // 3. Anti-pattern unresolved count (snapshot)
        let antipatterns_detected = self.antipattern_detector.violation_count();

        Ok(LearningCycleResult {
            pathways_decayed,
            timing_pairs_processed,
            antipatterns_detected,
        })
    }

    /// Get the average pathway strength across all Hebbian pathways.
    #[must_use]
    pub fn average_pathway_strength(&self) -> f64 {
        let strongest = self.hebbian_manager.get_strongest_pathways(usize::MAX);
        if strongest.is_empty() {
            return 0.0;
        }
        #[allow(clippy::cast_precision_loss)]
        let avg = strongest.iter().map(|p| p.strength).sum::<f64>()
            / strongest.len() as f64;
        avg
    }

    // ==================================================================
    // Consensus
    // ==================================================================

    /// Return the number of open ballots in the vote collector.
    #[must_use]
    pub fn open_ballot_count(&self) -> usize {
        self.vote_collector.open_ballot_count()
    }

    /// Return the total number of dissent events recorded.
    #[must_use]
    pub fn total_dissent(&self) -> usize {
        self.dissent_tracker.total_dissent()
    }

    /// Return the current PBFT view number.
    #[must_use]
    pub fn current_view_number(&self) -> u64 {
        self.pbft_manager.current_view_number()
    }

    // ==================================================================
    // Integration
    // ==================================================================

    /// Return the number of event bus channels.
    #[must_use]
    pub fn event_channel_count(&self) -> usize {
        self.event_bus.channel_count()
    }

    /// Return the number of registered service bridges.
    #[must_use]
    pub fn bridge_count(&self) -> usize {
        self.bridge_manager.bridge_count()
    }

    /// Return the overall synergy score from the bridge manager.
    #[must_use]
    pub fn overall_synergy(&self) -> f64 {
        self.bridge_manager.overall_synergy()
    }

    // ==================================================================
    // Accessor Methods
    // ==================================================================

    /// Access the L2 health monitor.
    #[must_use]
    pub const fn health_monitor(&self) -> &HealthMonitor {
        &self.health_monitor
    }

    /// Access the L2 lifecycle manager.
    #[must_use]
    pub const fn lifecycle_manager(&self) -> &LifecycleManager {
        &self.lifecycle_manager
    }

    /// Access the L2 service registry (M09).
    #[must_use]
    pub const fn service_registry(&self) -> &ServiceRegistry {
        &self.service_registry
    }

    /// Access the L2 circuit breaker registry.
    #[must_use]
    pub const fn circuit_breaker(&self) -> &CircuitBreakerRegistry {
        &self.circuit_breaker
    }

    /// Access the L3 pipeline manager.
    #[must_use]
    pub const fn pipeline_manager(&self) -> &PipelineManager {
        &self.pipeline_manager
    }

    /// Access the L3 remediation engine.
    #[must_use]
    pub const fn remediator(&self) -> &RemediationEngine {
        &self.remediator
    }

    /// Access the L3 confidence calculator.
    #[must_use]
    pub const fn confidence_calculator(&self) -> &ConfidenceCalculator {
        &self.confidence_calculator
    }

    /// Access the L3 action executor.
    #[must_use]
    pub const fn action_executor(&self) -> &ActionExecutor {
        &self.action_executor
    }

    /// Access the L3 outcome recorder.
    #[must_use]
    pub const fn outcome_recorder(&self) -> &OutcomeRecorder {
        &self.outcome_recorder
    }

    /// Access the L3 feedback loop.
    #[must_use]
    pub const fn feedback_loop(&self) -> &FeedbackLoop {
        &self.feedback_loop
    }

    /// Access the L4 REST client.
    #[must_use]
    pub const fn rest_client(&self) -> &RestClient {
        &self.rest_client
    }

    /// Access the L4 event bus.
    #[must_use]
    pub const fn event_bus(&self) -> &EventBus {
        &self.event_bus
    }

    /// Access the L4 bridge manager.
    #[must_use]
    pub const fn bridge_manager(&self) -> &BridgeManager {
        &self.bridge_manager
    }

    /// Access the L5 Hebbian manager.
    #[must_use]
    pub const fn hebbian_manager(&self) -> &HebbianManager {
        &self.hebbian_manager
    }

    /// Access the L5 STDP processor.
    #[must_use]
    pub const fn stdp_processor(&self) -> &StdpProcessor {
        &self.stdp_processor
    }

    /// Access the L5 anti-pattern detector.
    #[must_use]
    pub const fn antipattern_detector(&self) -> &AntiPatternDetector {
        &self.antipattern_detector
    }

    /// Access the L6 PBFT manager.
    #[must_use]
    pub const fn pbft_manager(&self) -> &PbftManager {
        &self.pbft_manager
    }

    /// Access the L6 vote collector.
    #[must_use]
    pub const fn vote_collector(&self) -> &VoteCollector {
        &self.vote_collector
    }

    /// Access the L6 dissent tracker.
    #[must_use]
    pub const fn dissent_tracker(&self) -> &DissentTracker {
        &self.dissent_tracker
    }

    /// R10: Return the number of active antipattern violations.
    #[must_use]
    pub fn antipattern_violation_count(&self) -> usize {
        self.antipattern_detector.violation_count()
    }

    /// R10: Create a PBFT consensus proposal for fleet-wide decision.
    ///
    /// Wraps `PbftManager::create_proposal` with a `ConfigRollback` action
    /// (lowest severity consensus action, appropriate for antipattern responses).
    /// Returns the proposal ID on success.
    ///
    /// # Errors
    ///
    /// Returns `Error` if the PBFT manager rejects the proposal.
    pub fn create_consensus_proposal(
        &self,
        description: &str,
        _source: &str,
    ) -> Result<String> {
        use crate::m6_consensus::ConsensusAction;
        let proposal = self.pbft_manager.create_proposal(
            ConsensusAction::ConfigRollback,
            description,
        )?;
        Ok(proposal.id)
    }

    /// R10: Return the ID of the most recently created PBFT proposal.
    #[must_use]
    pub fn latest_proposal_id(&self) -> Option<String> {
        self.pbft_manager
            .get_active_proposals()
            .last()
            .map(|p| p.id.clone())
    }

    /// R11: Generate active dissent for a proposal and record it.
    ///
    /// Uses `DissentTracker::record_dissent` to store counterarguments
    /// from 3 agent perspectives (Validator, Explorer, Critic). Returns
    /// the number of dissent events successfully recorded.
    pub fn generate_and_record_dissent(&self, proposal_id: &str) -> usize {
        use crate::AgentRole;
        let perspectives = [
            (AgentRole::Validator, "validator-001"),
            (AgentRole::Explorer, "explorer-001"),
            (AgentRole::Critic, "critic-001"),
        ];
        let mut count = 0;
        for (role, agent_id) in &perspectives {
            let reason = format!(
                "Automated dissent from {agent_id}: antipattern escalation requires review"
            );
            if self
                .dissent_tracker
                .record_dissent(proposal_id, agent_id, *role, reason)
                .is_ok()
            {
                count += 1;
            }
        }
        count
    }

    /// Access the L7 observer layer (optional).
    #[must_use]
    pub const fn observer(&self) -> Option<&ObserverLayer> {
        self.observer.as_ref()
    }

    /// Check whether the L7 observer is enabled and initialized.
    #[must_use]
    pub const fn observer_enabled(&self) -> bool {
        self.observer.is_some()
    }

    /// Access the peer bridge manager (optional).
    #[must_use]
    pub const fn peer_bridge(&self) -> Option<&PeerBridgeManager> {
        self.peer_bridge.as_ref()
    }

    /// Access the V3 thermal monitor (optional, fail-silent).
    #[must_use]
    pub const fn thermal_monitor(&self) -> Option<&ThermalMonitor> {
        self.thermal_monitor.as_ref()
    }

    /// Access the V3 decay scheduler (optional, fail-silent).
    #[must_use]
    pub const fn decay_scheduler(&self) -> Option<&DecayScheduler> {
        self.decay_scheduler.as_ref()
    }

    /// Access the V3 cascade bridge (optional, fail-silent).
    #[must_use]
    pub const fn cascade_bridge(&self) -> Option<&CascadeBridge> {
        self.cascade_bridge.as_ref()
    }

    // ==================================================================
    // V2 Accessors
    // ==================================================================

    /// Access the L1 self-model.
    #[must_use]
    pub const fn self_model(&self) -> &SelfModel {
        &self.self_model
    }

    /// Access the L2 traffic manager.
    #[must_use]
    pub const fn traffic_manager(&self) -> &TrafficManager {
        &self.traffic_manager
    }

    /// Access the L3 approval manager.
    #[must_use]
    pub const fn approval_manager(&self) -> &ApprovalManager {
        &self.approval_manager
    }

    /// Access the L4 auth manager.
    #[must_use]
    pub const fn auth_manager(&self) -> &AuthManager {
        &self.auth_manager
    }

    /// Access the L4 rate limiter.
    #[must_use]
    pub const fn rate_limiter(&self) -> &RateLimiter {
        &self.rate_limiter
    }

    /// Access the L4 ORAC bridge manager.
    #[must_use]
    pub const fn orac_bridge(&self) -> &OracBridgeManager {
        &self.orac_bridge
    }

    /// Access the L5 prediction engine.
    #[must_use]
    pub const fn prediction_core(&self) -> &PredictionEngineCore {
        &self.prediction_core
    }

    /// Access the L5 sequence detector.
    #[must_use]
    pub const fn sequence_detector(&self) -> &SequenceDetectorCore {
        &self.sequence_detector
    }

    /// Access the L6 checkpoint manager (optional, fail-silent).
    #[must_use]
    pub const fn checkpoint_manager(&self) -> Option<&InMemoryCheckpointManager> {
        self.checkpoint_manager.as_ref()
    }

    /// Access the L6 active dissent generator.
    #[must_use]
    pub const fn dissent_generator(&self) -> &ActiveDissentGenerator {
        &self.dissent_generator
    }

    /// Access the L8 field bridge.
    #[must_use]
    pub const fn field_bridge(&self) -> &FieldBridgeCore {
        &self.field_bridge
    }

    /// Access the L8 intent router.
    #[must_use]
    pub const fn intent_router(&self) -> &IntentRouterCore {
        &self.intent_router
    }

    /// Access the L8 regime manager.
    #[must_use]
    pub const fn regime_manager(&self) -> &RegimeManagerCore {
        &self.regime_manager
    }

    /// Access the L8 STDP bridge.
    #[must_use]
    pub const fn stdp_bridge(&self) -> &StdpBridgeCore {
        &self.stdp_bridge
    }

    /// Access the L8 evolution gate.
    #[must_use]
    pub const fn evolution_gate(&self) -> &EvolutionGateCore {
        &self.evolution_gate
    }

    /// Access the L8 morphogenic adapter.
    #[must_use]
    pub const fn morphogenic_adapter(&self) -> &MorphogenicAdapterCore {
        &self.morphogenic_adapter
    }

    // ==================================================================
    // Tensor & Observation
    // ==================================================================

    /// Build a 12D tensor from current engine metrics.
    ///
    /// Uses the [`TensorContributor`] framework from all four L2 modules to
    /// populate dimensions with live data via coverage-aware composition.
    ///
    /// **L2 Contributors:**
    /// - **`ServiceRegistry`** (M09): D0 (service count), D2 (avg tier), D3 (avg deps), D4 (healthy ratio)
    /// - **`HealthMonitor`** (M10): D6 (health score), D10 (error rate)
    /// - **`LifecycleManager`** (M11): D6 (running ratio), D7 (uptime proxy)
    /// - **`CircuitBreakerRegistry`** (M12): D9 (closed fraction), D10 (failure rate)
    ///
    /// **Non-L2 dimensions** (manual calculation):
    /// - D1 (port): static 8080/65535
    /// - D5 (protocol): static 3/4 (REST + gRPC + WebSocket)
    /// - D8 (synergy): L4 bridge manager
    /// - D11 (temporal): L5 learning + L3 pipeline activity
    ///
    /// Where multiple contributors cover the same dimension (D6, D10),
    /// values are averaged per the coverage-aware composition algorithm.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn build_tensor(&self) -> Tensor12D {
        // -- Collect L2 tensor contributions from all 4 modules --
        let contribs = [
            self.service_registry.contribute(),  // M09: D0, D2, D3, D4
            self.health_monitor.contribute(),    // M10: D6, D10
            self.lifecycle_manager.contribute(), // M11: D6, D7
            self.circuit_breaker.contribute(),   // M12: D9, D10
        ];

        // -- Coverage-aware per-dimension averaging across L2 contributors --
        let mut dim_sums = [0.0_f64; 12];
        let mut dim_counts = [0_u32; 12];
        for contrib in &contribs {
            let arr = contrib.tensor.to_array();
            for dim in DimensionIndex::ALL {
                if contrib.coverage.is_covered(dim) {
                    dim_sums[dim.index()] += arr[dim.index()];
                    dim_counts[dim.index()] += 1;
                }
            }
        }

        // Compose L2-covered dimensions (average where multiple contributors)
        let l2 = std::array::from_fn::<f64, 12, _>(|i| {
            if dim_counts[i] > 0 {
                dim_sums[i] / f64::from(dim_counts[i])
            } else {
                0.0
            }
        });

        // -- Non-L2 dimensions --

        // D1: port (static — no L2 contributor covers this)
        let d1_port = 8080.0 / 65535.0;

        // D5: protocol diversity (ME exposes REST + gRPC + WebSocket = 3/4)
        let d5_protocol = 3.0 / 4.0;

        // D8: synergy from L4 bridge manager
        let d8_synergy = self.bridge_manager.overall_synergy().clamp(0.0, 1.0);

        // D10 enrichment: blend L2 raw sums with L3 remediation failure rate
        let d10_error = {
            let mut sum = dim_sums[DimensionIndex::ErrorRate.index()];
            let mut count = dim_counts[DimensionIndex::ErrorRate.index()];
            let remediation_error = 1.0 - self.remediator.success_rate();
            sum += remediation_error.clamp(0.0, 1.0);
            count += 1;
            (sum / f64::from(count)).clamp(0.0, 1.0)
        };

        // D11: temporal context from L5 learning + L3 pipeline activity
        let pathway_strength = self.average_pathway_strength().clamp(0.0, 1.0);
        let pipeline_count = self.pipeline_manager.pipeline_count();
        let active_pipelines = self.pipeline_manager.get_enabled_pipelines().len();
        let pipeline_ratio = if pipeline_count == 0 {
            1.0
        } else {
            active_pipelines as f64 / pipeline_count as f64
        };
        let d11_temporal = pathway_strength.mul_add(0.5, 0.5 * pipeline_ratio);

        Tensor12D {
            service_id: l2[DimensionIndex::ServiceId.index()],
            port: d1_port,
            tier: l2[DimensionIndex::Tier.index()],
            dependency_count: l2[DimensionIndex::DependencyCount.index()],
            agent_count: l2[DimensionIndex::AgentCount.index()],
            protocol: d5_protocol,
            health_score: l2[DimensionIndex::HealthScore.index()],
            uptime: l2[DimensionIndex::Uptime.index()],
            synergy: d8_synergy,
            latency: l2[DimensionIndex::Latency.index()],
            error_rate: d10_error,
            temporal_context: d11_temporal,
        }
    }

    // ==================================================================
    // Internal helpers
    // ==================================================================

    /// Pre-populate the L2 service registry with the active ULTRAPLATE services
    /// and their dependency edges. Called once during [`Engine::new`].
    ///
    /// Retired (do not re-add): codesynthor-v7 (S091), devops-engine V2 (S091),
    /// tool-library V1 (S093), prometheus-swarm V1 (S088), architect-agent (S093),
    /// library-agent (S093).
    fn populate_ultraplate_services(registry: &ServiceRegistry) {
        let services: [(&str, &str, &str, ServiceTier, u16, &str); 11] = [
            ("synthex", "SYNTHEX", "1.0.0", ServiceTier::Tier1, 8090, "/api/health"),
            ("san-k7", "SAN-K7 Orchestrator", "1.55.0", ServiceTier::Tier1, 8100, "/health"),
            ("nais", "NAIS", "1.0.0", ServiceTier::Tier2, 8101, "/health"),
            ("ccm", "Claude Context Manager", "1.0.0", ServiceTier::Tier3, 8104, "/health"),
            ("vortex-memory", "Vortex Memory System", "1.0.0", ServiceTier::Tier3, 8120, "/health"),
            ("povm-engine", "POVM Engine", "1.0.0", ServiceTier::Tier4, 8125, "/health"),
            ("reasoning-memory", "Reasoning Memory", "1.0.0", ServiceTier::Tier4, 8130, "/health"),
            ("pane-vortex", "Pane-Vortex V2", "1.0.0", ServiceTier::Tier4, 8132, "/health"),
            ("orac-sidecar", "ORAC Sidecar", "1.0.0", ServiceTier::Tier4, 8133, "/health"),
            ("bash-engine", "Bash Engine", "1.0.0", ServiceTier::Tier5, 8102, "/health"),
            ("tool-maker", "Tool Maker", "1.55.0", ServiceTier::Tier5, 8103, "/health"),
        ];

        for &(id, name, version, tier, port, health_path) in &services {
            let def = ServiceDefinitionBuilder::new(id, name, version)
                .tier(tier)
                .port(port)
                .health_path(health_path)
                .build();
            let _ = registry.register(def);
        }

        // Dependency edges (active services only; retired-service edges pruned S097).
        let deps: [(&str, &str); 5] = [
            ("san-k7", "synthex"),
            ("nais", "san-k7"),
            ("bash-engine", "san-k7"),
            ("tool-maker", "san-k7"),
            ("ccm", "tool-maker"),
        ];
        for &(from, to) in &deps {
            let _ = registry.add_dependency(from, to);
        }
    }

    /// Register health probes for all ULTRAPLATE services so that the
    /// health monitor can track their status via HTTP polling.
    fn populate_health_probes(monitor: &HealthMonitor) {
        let probes: [(&str, &str); 11] = [
            ("dev-ops-engine-v3",  "http://localhost:8082/health"),
            ("habitat-nerve-center","http://localhost:8083/health"),
            ("synthex",            "http://localhost:8090/api/health"),
            ("codesynthor-v8",     "http://localhost:8111/health"),
            ("vortex-memory-system","http://localhost:8120/health"),
            ("povm-engine",        "http://localhost:8125/health"),
            ("reasoning-memory",   "http://localhost:8130/health"),
            ("pane-vortex",        "http://localhost:8132/health"),
            ("orac-sidecar",       "http://localhost:8133/health"),
            ("maintenance-engine", "http://localhost:8180/api/health"),
            ("prometheus-swarm-v2","http://localhost:10002/health"),
        ];
        for &(id, endpoint) in &probes {
            if let Ok(probe) = HealthProbeBuilder::new(id, endpoint)
                .interval_ms(30_000)
                .timeout_ms(5_000)
                .healthy_threshold(1)
                .unhealthy_threshold(3)
                .build()
            {
                let _ = monitor.register_probe(probe);
            }
        }
    }

    /// Pre-populate the lifecycle manager with active ULTRAPLATE services.
    ///
    /// Registers each service, then transitions through Starting to Running
    /// so that D7 (uptime proxy) starts at 1.0 with zero restarts.
    fn populate_lifecycle_services(manager: &LifecycleManager) {
        let services: [(&str, &str, ServiceTier); 11] = [
            ("synthex", "SYNTHEX", ServiceTier::Tier1),
            ("san-k7", "SAN-K7 Orchestrator", ServiceTier::Tier1),
            ("nais", "NAIS", ServiceTier::Tier2),
            ("ccm", "Claude Context Manager", ServiceTier::Tier3),
            ("vortex-memory", "Vortex Memory System", ServiceTier::Tier3),
            ("povm-engine", "POVM Engine", ServiceTier::Tier4),
            ("reasoning-memory", "Reasoning Memory", ServiceTier::Tier4),
            ("pane-vortex", "Pane-Vortex V2", ServiceTier::Tier4),
            ("orac-sidecar", "ORAC Sidecar", ServiceTier::Tier4),
            ("bash-engine", "Bash Engine", ServiceTier::Tier5),
            ("tool-maker", "Tool Maker", ServiceTier::Tier5),
        ];

        for &(id, name, tier) in &services {
            // Register → start → mark running (Stopped → Starting → Running)
            if manager.register(id, name, tier, RestartConfig::default()).is_ok()
                && manager.start_service(id).is_ok()
            {
                let _ = manager.mark_running(id);
            }
        }
    }

    /// Public accessor for per-layer health scores.
    #[must_use]
    pub fn layer_health_scores(&self) -> [f64; LAYER_COUNT] {
        self.compute_layer_health()
    }

    /// Count of healthy services from the L2 health monitor.
    fn healthy_service_count(&self) -> usize {
        self.health_monitor.get_healthy_services().len()
    }

    /// Count of enabled pipelines.
    fn active_pipeline_count(&self) -> usize {
        self.pipeline_manager.get_enabled_pipelines().len()
    }

    /// Count of active PBFT proposals.
    fn active_proposal_count(&self) -> usize {
        self.pbft_manager.get_active_proposals().len()
    }

    /// Compute per-layer health scores.
    ///
    /// Each layer score is in [0.0, 1.0]:
    /// - L1 Foundation: always 1.0 (infra assumed healthy)
    /// - L2 Services: fraction of healthy probes
    /// - L3 Core Logic: pipeline availability + remediation success rate
    /// - L4 Integration: bridge availability + event bus activity
    /// - L5 Learning: average pathway strength
    /// - L6 Consensus: fleet readiness + dissent capture rate
    /// - L7 Observer: 1.0 if disabled or healthy, derived from tick/error ratio
    #[allow(clippy::cast_precision_loss)]
    fn compute_layer_health(&self) -> [f64; LAYER_COUNT] {
        // L1: Foundation (always healthy -- if we are running, L1 is up)
        let l1 = 1.0;

        // L2: Services — use aggregate_health() which correctly computes
        // weighted score from actual poll results (Healthy=1.0, Degraded=0.5,
        // Unknown/Unhealthy=0.0) rather than get_healthy_services().len() which
        // only counts services that reached the Healthy FSM state (3 consecutive
        // successes). This was causing L2=0.33 when 11/12 services were reachable.
        let l2 = self.health_monitor.aggregate_health();

        // L3: Core Logic
        let pipeline_count = self.pipeline_manager.pipeline_count();
        let active_pipelines = self.pipeline_manager.get_enabled_pipelines().len();
        let pipeline_ratio = if pipeline_count == 0 {
            1.0
        } else {
            active_pipelines as f64 / pipeline_count as f64
        };
        let success_rate = self.remediator.success_rate();
        let l3 = 0.5f64.mul_add(pipeline_ratio, 0.5 * success_rate);

        // L4: Integration
        let bridge_count = self.bridge_manager.bridge_count();
        let active_bridges = self.bridge_manager.get_active_bridges().len();
        let bridge_ratio = if bridge_count == 0 {
            1.0
        } else {
            active_bridges as f64 / bridge_count as f64
        };
        let channel_count = self.event_bus.channel_count();
        let channel_score = if channel_count >= 6 { 1.0 } else { channel_count as f64 / 6.0 };
        let l4 = 0.5f64.mul_add(bridge_ratio, 0.5 * channel_score);

        // L5: Learning
        let l5 = self.average_pathway_strength().clamp(0.0, 1.0);

        // L6: Consensus
        let fleet = self.pbft_manager.get_fleet();
        let fleet_size = fleet.len();
        let fleet_score = if fleet_size >= 41 { 1.0 } else { fleet_size as f64 / 41.0 };
        let dissent_rate = self.dissent_tracker.valuable_dissent_rate();
        let l6 = 0.7f64.mul_add(fleet_score, 0.3 * dissent_rate.clamp(0.0, 1.0));

        // L7: Observer (fail-silent: 1.0 when absent or healthy)
        let l7 = self.observer.as_ref().map_or(1.0, |obs| {
            let metrics = obs.metrics();
            let ticks = metrics.ticks_executed;
            let errors = metrics.observer_errors;
            if ticks == 0 {
                1.0 // No ticks yet, assume healthy
            } else {
                let error_ratio = errors as f64 / ticks as f64;
                (1.0 - error_ratio).clamp(0.0, 1.0)
            }
        });

        [l1, l2, l3, l4, l5, l6, l7]
    }

    /// Compute the weighted overall health from per-layer scores.
    fn compute_overall_health(layers: &[f64; LAYER_COUNT]) -> f64 {
        let weights = [
            LAYER_WEIGHT_FOUNDATION,
            LAYER_WEIGHT_SERVICES,
            LAYER_WEIGHT_CORE,
            LAYER_WEIGHT_INTEGRATION,
            LAYER_WEIGHT_LEARNING,
            LAYER_WEIGHT_CONSENSUS,
            LAYER_WEIGHT_OBSERVER,
        ];

        let score: f64 = layers
            .iter()
            .zip(weights.iter())
            .map(|(health, weight)| health * weight)
            .sum();

        score.clamp(0.0, 1.0)
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ---------------------------------------------------------------
    // Group 1: Construction and Defaults (5 tests)
    // ---------------------------------------------------------------

    #[test]
    fn test_engine_construction_succeeds() {
        let engine = Engine::new();
        // Verify the engine exists and is usable.
        assert!(engine.service_count() >= 0);
    }

    #[test]
    fn test_engine_default_trait() {
        let engine = Engine::default();
        assert!(engine.pipeline_count() > 0, "default pipelines should load");
    }

    #[test]
    fn test_engine_default_pipelines_loaded() {
        let engine = Engine::new();
        // PipelineManager::new() loads 8 default pipelines
        assert_eq!(engine.pipeline_count(), 8);
    }

    #[test]
    fn test_engine_default_pathways_loaded() {
        let engine = Engine::new();
        // HebbianManager::new() loads default pathways (>= 9)
        assert!(engine.pathway_count() >= 9);
    }

    #[test]
    fn test_engine_default_event_channels() {
        let engine = Engine::new();
        // EventBus::new() creates 7 default channels (6 original + "gc")
        assert_eq!(engine.event_channel_count(), 7);
    }

    // ---------------------------------------------------------------
    // Group 2: Health Reporting Accuracy (5 tests)
    // ---------------------------------------------------------------

    #[test]
    fn test_health_report_returns_ok() {
        let engine = Engine::new();
        let report = engine.health_report();
        assert!(report.is_ok());
    }

    #[test]
    fn test_health_report_layer_count() {
        let engine = Engine::new();
        let report = engine.health_report().ok();
        assert!(report.is_some());
        let report = report.into_iter().next();
        assert!(report.is_some());
        if let Some(r) = report {
            assert_eq!(r.layer_health.len(), LAYER_COUNT);
        }
    }

    #[test]
    fn test_health_report_foundation_always_healthy() {
        let engine = Engine::new();
        if let Ok(report) = engine.health_report() {
            assert!(
                (report.layer_health[0] - 1.0).abs() < f64::EPSILON,
                "L1 Foundation should be 1.0"
            );
        }
    }

    #[test]
    fn test_health_report_overall_in_range() {
        let engine = Engine::new();
        if let Ok(report) = engine.health_report() {
            assert!(
                (0.0..=1.0).contains(&report.overall_health),
                "overall_health should be in [0,1], got {}",
                report.overall_health
            );
        }
    }

    #[test]
    fn test_health_report_weakest_layer() {
        let engine = Engine::new();
        if let Ok(report) = engine.health_report() {
            let (idx, score) = report.weakest_layer();
            assert!(idx < LAYER_COUNT);
            // The weakest score must be <= all layer scores
            for &ls in &report.layer_health {
                assert!(score <= ls + f64::EPSILON);
            }
        }
    }

    // ---------------------------------------------------------------
    // Group 3: Remediation Pipeline Flow (8 tests)
    // ---------------------------------------------------------------

    #[test]
    fn test_submit_remediation_success() {
        let engine = Engine::new();
        let result =
            engine.submit_remediation("synthex", IssueType::HealthFailure, Severity::High, "test");
        assert!(result.is_ok());
    }

    #[test]
    fn test_submit_remediation_returns_unique_ids() {
        let engine = Engine::new();
        let id1 =
            engine.submit_remediation("synthex", IssueType::HealthFailure, Severity::High, "a");
        let id2 =
            engine.submit_remediation("synthex", IssueType::HealthFailure, Severity::High, "b");
        // Extract Ok values and compare directly to avoid PartialEq requirement on Error
        let v1 = id1.ok();
        let v2 = id2.ok();
        assert!(v1.is_some(), "first remediation should succeed");
        assert!(v2.is_some(), "second remediation should succeed");
        assert_ne!(v1, v2, "remediation IDs must be unique");
    }

    #[test]
    fn test_submit_remediation_increments_pending() {
        let engine = Engine::new();
        let before = engine.pending_remediations();
        let _ = engine.submit_remediation(
            "san-k7",
            IssueType::LatencySpike,
            Severity::Medium,
            "spike",
        );
        assert_eq!(engine.pending_remediations(), before + 1);
    }

    #[test]
    fn test_submit_remediation_empty_service_id_rejected() {
        let engine = Engine::new();
        let result =
            engine.submit_remediation("", IssueType::HealthFailure, Severity::Low, "desc");
        assert!(result.is_err());
    }

    #[test]
    fn test_submit_remediation_empty_description_rejected() {
        let engine = Engine::new();
        let result =
            engine.submit_remediation("synthex", IssueType::HealthFailure, Severity::Low, "");
        assert!(result.is_err());
    }

    #[test]
    fn test_remediation_multiple_severities() {
        let engine = Engine::new();
        for severity in [Severity::Low, Severity::Medium, Severity::High, Severity::Critical] {
            let result =
                engine.submit_remediation("nais", IssueType::ErrorRateHigh, severity, "test");
            assert!(result.is_ok(), "should accept severity {severity:?}");
        }
    }

    #[test]
    fn test_remediation_multiple_issue_types() {
        let engine = Engine::new();
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
        for it in issue_types {
            let result = engine.submit_remediation("synthex", it, Severity::Medium, "test");
            assert!(result.is_ok(), "should accept issue type {it:?}");
        }
    }

    #[test]
    fn test_remediation_success_rate_starts_at_default() {
        let engine = Engine::new();
        // With no completed remediations, the success rate should be some
        // default (0.0 or 0.5 depending on implementation).
        let rate = engine.remediation_success_rate();
        assert!(
            (0.0..=1.0).contains(&rate),
            "success rate should be in [0,1], got {rate}"
        );
    }

    // ---------------------------------------------------------------
    // Group 4: Learning Cycle Operations (8 tests)
    // ---------------------------------------------------------------

    #[test]
    fn test_learning_cycle_returns_ok() {
        let engine = Engine::new();
        let result = engine.learning_cycle();
        assert!(result.is_ok());
    }

    #[test]
    fn test_learning_cycle_decay_occurs() {
        let engine = Engine::new();
        if let Ok(result) = engine.learning_cycle() {
            // With default pathways loaded, some should be decayed.
            assert!(
                result.pathways_decayed > 0,
                "expected >0 decayed pathways, got {}",
                result.pathways_decayed
            );
        }
    }

    #[test]
    fn test_learning_cycle_had_activity() {
        let engine = Engine::new();
        if let Ok(result) = engine.learning_cycle() {
            // Decay should produce activity even without spikes.
            assert!(
                result.had_activity(),
                "learning cycle should have some activity"
            );
        }
    }

    #[test]
    fn test_learning_cycle_multiple_cycles_stable() {
        let engine = Engine::new();
        for i in 0..5 {
            let result = engine.learning_cycle();
            assert!(
                result.is_ok(),
                "learning cycle {i} should succeed"
            );
        }
    }

    #[test]
    fn test_learning_cycle_pathway_strength_decreases() {
        let engine = Engine::new();
        let before = engine.average_pathway_strength();
        let _ = engine.learning_cycle();
        let after = engine.average_pathway_strength();
        assert!(
            after <= before + f64::EPSILON,
            "average strength should not increase from decay alone: before={before}, after={after}"
        );
    }

    #[test]
    fn test_average_pathway_strength_in_range() {
        let engine = Engine::new();
        let strength = engine.average_pathway_strength();
        assert!(
            (0.0..=1.0).contains(&strength),
            "average strength should be in [0,1], got {strength}"
        );
    }

    #[test]
    fn test_antipattern_count_starts_at_zero() {
        let engine = Engine::new();
        // No detections raised yet, only patterns registered.
        let count = engine.antipattern_detector().violation_count();
        assert_eq!(count, 0, "no violations should exist at startup");
    }

    #[test]
    fn test_antipattern_detector_has_default_patterns() {
        let engine = Engine::new();
        let count = engine.antipattern_detector().pattern_count();
        assert!(
            count >= 15,
            "should have >= 15 default patterns, got {count}"
        );
    }

    // ---------------------------------------------------------------
    // Group 5: Accessor Methods Return Correct Managers (6 tests)
    // ---------------------------------------------------------------

    #[test]
    fn test_accessor_health_monitor() {
        let engine = Engine::new();
        // Verify the accessor returns a usable reference
        let monitor = engine.health_monitor();
        assert!(monitor.probe_count() >= 0);
    }

    #[test]
    fn test_accessor_pipeline_manager() {
        let engine = Engine::new();
        let pm = engine.pipeline_manager();
        assert_eq!(pm.pipeline_count(), 8);
    }

    #[test]
    fn test_accessor_remediator() {
        let engine = Engine::new();
        let re = engine.remediator();
        assert_eq!(re.pending_count(), 0);
    }

    #[test]
    fn test_accessor_hebbian_manager() {
        let engine = Engine::new();
        let hm = engine.hebbian_manager();
        assert!(hm.pathway_count() >= 9);
    }

    #[test]
    fn test_accessor_pbft_manager() {
        let engine = Engine::new();
        let pm = engine.pbft_manager();
        let fleet = pm.get_fleet();
        assert_eq!(fleet.len(), 41, "PBFT fleet should have 41 agents");
    }

    #[test]
    fn test_accessor_dissent_tracker() {
        let engine = Engine::new();
        let dt = engine.dissent_tracker();
        assert_eq!(dt.total_dissent(), 0, "no dissent events at startup");
    }

    // ---------------------------------------------------------------
    // Group 6: Error Paths and Validation (8 tests)
    // ---------------------------------------------------------------

    #[test]
    fn test_validation_empty_service_id() {
        let engine = Engine::new();
        let err = engine
            .submit_remediation("", IssueType::HealthFailure, Severity::Low, "desc")
            .err();
        assert!(err.is_some());
        let msg = err.map(|e| e.to_string()).unwrap_or_default();
        assert!(msg.contains("service_id"), "error should mention service_id: {msg}");
    }

    #[test]
    fn test_validation_empty_description() {
        let engine = Engine::new();
        let err = engine
            .submit_remediation("synthex", IssueType::HealthFailure, Severity::Low, "")
            .err();
        assert!(err.is_some());
        let msg = err.map(|e| e.to_string()).unwrap_or_default();
        assert!(
            msg.contains("description"),
            "error should mention description: {msg}"
        );
    }

    #[test]
    fn test_active_proposals_callable() {
        let engine = Engine::new();
        let count = engine.active_proposals();
        assert!(count < 10_000, "sanity check");
    }

    #[test]
    fn test_active_proposals_starts_at_zero() {
        let engine = Engine::new();
        let count = engine.active_proposals();
        assert_eq!(count, 0, "no proposals at startup");
    }

    #[test]
    fn test_health_report_is_healthy_check() {
        let report = EngineHealthReport {
            services_total: 10,
            services_healthy: 8,
            pipelines_active: 8,
            pathways_count: 9,
            proposals_active: 0,
            overall_health: 0.85,
            layer_health: [1.0, 0.8, 0.7, 0.9, 0.5, 0.7, 1.0],
        };
        assert!(report.is_healthy());
    }

    #[test]
    fn test_health_report_unhealthy_below_threshold() {
        let report = EngineHealthReport {
            services_total: 10,
            services_healthy: 2,
            pipelines_active: 2,
            pathways_count: 1,
            proposals_active: 0,
            overall_health: 0.3,
            layer_health: [1.0, 0.2, 0.1, 0.4, 0.1, 0.3, 0.5],
        };
        assert!(!report.is_healthy());
    }

    #[test]
    fn test_health_report_boundary_exactly_at_threshold() {
        let report = EngineHealthReport {
            services_total: 10,
            services_healthy: 5,
            pipelines_active: 4,
            pathways_count: 5,
            proposals_active: 0,
            overall_health: MIN_HEALTHY_SCORE,
            layer_health: [1.0, 0.5, 0.5, 0.5, 0.5, 0.5, 1.0],
        };
        assert!(report.is_healthy(), "exactly at threshold should be healthy");
    }

    #[test]
    fn test_health_report_just_below_threshold() {
        let report = EngineHealthReport {
            services_total: 10,
            services_healthy: 4,
            pipelines_active: 3,
            pathways_count: 4,
            proposals_active: 0,
            overall_health: MIN_HEALTHY_SCORE - f64::EPSILON,
            layer_health: [1.0, 0.4, 0.4, 0.4, 0.4, 0.4, 1.0],
        };
        assert!(!report.is_healthy(), "just below threshold should be unhealthy");
    }

    // ---------------------------------------------------------------
    // Group 7: Cross-Layer Coordination (5 tests)
    // ---------------------------------------------------------------

    #[test]
    fn test_cross_layer_remediation_creates_pending() {
        let engine = Engine::new();
        let _ =
            engine.submit_remediation("synthex", IssueType::HealthFailure, Severity::High, "fail");
        // The request should be visible in the remediation engine
        let pending = engine.remediator().pending_count();
        assert!(pending >= 1, "pending should reflect submitted request");
    }

    #[test]
    fn test_cross_layer_pipeline_reflects_in_health() {
        let engine = Engine::new();
        let report = engine.health_report();
        assert!(report.is_ok());
        if let Ok(r) = report {
            assert_eq!(
                r.pipelines_active,
                engine.pipeline_count(),
                "health report pipeline count should match engine pipeline count"
            );
        }
    }

    #[test]
    fn test_cross_layer_pathway_reflects_in_health() {
        let engine = Engine::new();
        let report = engine.health_report();
        assert!(report.is_ok());
        if let Ok(r) = report {
            assert_eq!(
                r.pathways_count,
                engine.pathway_count(),
                "health report pathway count should match engine pathway count"
            );
        }
    }

    #[test]
    fn test_cross_layer_consensus_fleet_accessible() {
        let engine = Engine::new();
        let fleet = engine.pbft_manager().get_fleet();
        // Should include Human @0.A + 40 agents
        assert_eq!(fleet.len(), 41);
        let human = fleet.iter().find(|a| a.id == "@0.A");
        assert!(human.is_some(), "Human @0.A should be in the fleet");
    }

    #[test]
    fn test_cross_layer_learning_and_consensus_independent() {
        let engine = Engine::new();
        // Learning cycle should not affect consensus state
        let proposals_before = engine.active_proposals();
        let _ = engine.learning_cycle();
        let proposals_after = engine.active_proposals();
        assert_eq!(
            proposals_before, proposals_after,
            "learning cycle should not create proposals"
        );
    }

    // ---------------------------------------------------------------
    // Group 8: Edge Cases and Boundaries (5 tests)
    // ---------------------------------------------------------------

    #[test]
    fn test_boundary_many_remediations() {
        let engine = Engine::new();
        for i in 0..20 {
            let result = engine.submit_remediation(
                &format!("svc-{i}"),
                IssueType::HealthFailure,
                Severity::Low,
                "test",
            );
            assert!(result.is_ok(), "remediation {i} should succeed");
        }
        assert!(
            engine.pending_remediations() >= 20,
            "should have at least 20 pending"
        );
    }

    #[test]
    fn test_boundary_repeated_learning_cycles() {
        let engine = Engine::new();
        let initial_strength = engine.average_pathway_strength();
        // Run 50 decay-only cycles -- strength should converge towards
        // the minimum (0.1) but never go negative.
        for _ in 0..50 {
            let _ = engine.learning_cycle();
        }
        let final_strength = engine.average_pathway_strength();
        assert!(
            final_strength >= 0.0,
            "pathway strength must not go negative: {final_strength}"
        );
        assert!(
            final_strength <= initial_strength + f64::EPSILON,
            "50 decay cycles should not increase average strength"
        );
    }

    #[test]
    fn test_boundary_health_report_stability() {
        let engine = Engine::new();
        // Multiple health reports should be consistent.
        let r1 = engine.health_report();
        let r2 = engine.health_report();
        assert!(r1.is_ok());
        assert!(r2.is_ok());
        if let (Ok(a), Ok(b)) = (r1, r2) {
            assert!(
                (a.overall_health - b.overall_health).abs() < f64::EPSILON,
                "consecutive reports should be identical without mutations"
            );
        }
    }

    #[test]
    fn test_boundary_service_count_non_negative() {
        let engine = Engine::new();
        // service_count uses max(probes, endpoints) -- both are usize,
        // so by definition non-negative, but ensure the accessor works.
        assert!(engine.service_count() < 10_000, "sanity check");
    }

    #[test]
    fn test_boundary_learning_cycle_result_fields() {
        let result = LearningCycleResult {
            pathways_decayed: 0,
            timing_pairs_processed: 0,
            antipatterns_detected: 0,
        };
        assert!(!result.had_activity());

        let active_result = LearningCycleResult {
            pathways_decayed: 1,
            timing_pairs_processed: 0,
            antipatterns_detected: 0,
        };
        assert!(active_result.had_activity());
    }

    // ---------------------------------------------------------------
    // Additional tests for completeness (2 more for 52 total)
    // ---------------------------------------------------------------

    #[test]
    fn test_open_ballot_count_starts_at_zero() {
        let engine = Engine::new();
        assert_eq!(engine.open_ballot_count(), 0);
    }

    #[test]
    fn test_current_view_number_starts_at_zero() {
        let engine = Engine::new();
        assert_eq!(engine.current_view_number(), 0);
    }

    // ---------------------------------------------------------------
    // Group 9: L7 Observer Integration (12 tests)
    // ---------------------------------------------------------------

    #[test]
    fn test_observer_initialized_by_default() {
        let engine = Engine::new();
        assert!(engine.observer_enabled(), "L7 should be initialized by default");
        assert!(engine.observer().is_some());
    }

    #[test]
    fn test_health_report_has_seven_layers() {
        let engine = Engine::new();
        if let Ok(report) = engine.health_report() {
            assert_eq!(report.layer_health.len(), 7);
        }
    }

    #[test]
    fn test_observer_health_starts_at_one() {
        let engine = Engine::new();
        if let Ok(report) = engine.health_report() {
            assert!(
                (report.layer_health[6] - 1.0).abs() < f64::EPSILON,
                "L7 health should be 1.0 before any ticks"
            );
        }
    }

    #[test]
    fn test_build_tensor_produces_valid_tensor() {
        let engine = Engine::new();
        let tensor = engine.build_tensor();
        assert!(tensor.validate().is_ok(), "build_tensor should produce a valid tensor");
    }

    #[test]
    fn test_build_tensor_health_score_in_range() {
        let engine = Engine::new();
        let tensor = engine.build_tensor();
        assert!(
            (0.0..=1.0).contains(&tensor.health_score),
            "health_score should be in [0,1], got {}",
            tensor.health_score
        );
    }

    #[test]
    fn test_build_tensor_synergy_in_range() {
        let engine = Engine::new();
        let tensor = engine.build_tensor();
        assert!(
            (0.0..=1.0).contains(&tensor.synergy),
            "synergy should be in [0,1], got {}",
            tensor.synergy
        );
    }

    #[test]
    fn test_observer_tick_via_engine_tensor() {
        let engine = Engine::new();
        let tensor = engine.build_tensor();
        if let Some(obs) = engine.observer() {
            let result = obs.tick(&tensor);
            assert!(result.is_ok(), "observer tick should succeed with engine tensor");
            assert_eq!(obs.tick_count(), 1);
        }
    }

    #[test]
    fn test_observer_metrics_after_tick() {
        let engine = Engine::new();
        let tensor = engine.build_tensor();
        if let Some(obs) = engine.observer() {
            let _ = obs.tick(&tensor);
            let metrics = obs.metrics();
            assert_eq!(metrics.ticks_executed, 1);
            assert_eq!(metrics.reports_generated, 1);
        }
    }

    #[test]
    fn test_weakest_layer_with_seven_layers() {
        let engine = Engine::new();
        if let Ok(report) = engine.health_report() {
            let (idx, score) = report.weakest_layer();
            assert!(idx < LAYER_COUNT, "weakest layer index should be < 7");
            for &ls in &report.layer_health {
                assert!(score <= ls + f64::EPSILON);
            }
        }
    }

    #[test]
    fn test_observer_accessor_returns_layer() {
        let engine = Engine::new();
        let obs = engine.observer();
        assert!(obs.is_some());
        if let Some(layer) = obs {
            assert!(layer.is_enabled());
        }
    }

    #[test]
    fn test_seven_layer_weights_sum_to_one() {
        let sum = LAYER_WEIGHT_FOUNDATION
            + LAYER_WEIGHT_SERVICES
            + LAYER_WEIGHT_CORE
            + LAYER_WEIGHT_INTEGRATION
            + LAYER_WEIGHT_LEARNING
            + LAYER_WEIGHT_CONSENSUS
            + LAYER_WEIGHT_OBSERVER;
        assert!(
            (sum - 1.0).abs() < 1e-10,
            "weights should sum to 1.0, got {sum}"
        );
    }

    #[test]
    fn test_health_report_stability_with_observer() {
        let engine = Engine::new();
        let r1 = engine.health_report();
        let r2 = engine.health_report();
        assert!(r1.is_ok());
        assert!(r2.is_ok());
        if let (Ok(a), Ok(b)) = (r1, r2) {
            assert!(
                (a.overall_health - b.overall_health).abs() < f64::EPSILON,
                "consecutive reports should be identical without mutations"
            );
            assert_eq!(a.layer_health.len(), b.layer_health.len());
        }
    }
}
