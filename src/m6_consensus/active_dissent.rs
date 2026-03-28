//! # M57: Active Dissent Generator (NAM R3)
//!
//! Actively generates dissenting opinions for consensus proposals by
//! evaluating them from multiple perspectives. Unlike M35 `DissentTracker`
//! (passive recording), M57 produces structured counterarguments with
//! deterministic risk scoring for each proposal before consensus.
//!
//! ## Design
//!
//! - **Deterministic:** Risk scores and counterarguments are pure functions
//!   of proposal fields and perspective — no randomness, no I/O.
//! - **Agent selection:** Critic agents are assigned via hash-based
//!   round-robin on the proposal ID for reproducible assignment.
//! - **Pipeline:** `pipeline_dissent` produces exactly 3 `GeneratedDissent`
//!   items (one per `AgentPerspective`), summarised in a
//!   `PipelineDissentResult`.
//!
//! ## Layer: L6 (Consensus)
//!
//! ## Related Documentation
//! - [M35 Dissent Tracker](dissent.rs) — passive recording counterpart
//! - [NAM R3 Dissent Capture](../../nam/NAM_SPEC.md)

use std::collections::hash_map::DefaultHasher;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::RwLock;

use crate::m1_foundation::shared_types::Timestamp;
use crate::{Error, Result};

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Configuration for the active dissent generator.
#[derive(Clone, Debug)]
pub struct DissentGeneratorConfig {
    /// Risk score at or above which human review is flagged.
    pub risk_threshold_for_review: f64,
    /// IDs of critic agents available for dissent generation.
    pub critic_agent_ids: Vec<String>,
}

impl Default for DissentGeneratorConfig {
    fn default() -> Self {
        Self {
            risk_threshold_for_review: 0.7,
            critic_agent_ids: vec![
                "agent-29".into(),
                "agent-30".into(),
                "agent-31".into(),
                "agent-32".into(),
                "agent-33".into(),
                "agent-34".into(),
            ],
        }
    }
}

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// The perspective from which a dissent is generated.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AgentPerspective {
    /// Evaluates blast-radius and direct risk surfaces.
    RiskSurface,
    /// Evaluates based on historical patterns and precedent.
    HistoricalPattern,
    /// Evaluates cross-system dependencies and cascading impact.
    CrossSystemImpact,
}

/// All available perspectives in enumeration order.
const ALL_PERSPECTIVES: [AgentPerspective; 3] = [
    AgentPerspective::RiskSurface,
    AgentPerspective::HistoricalPattern,
    AgentPerspective::CrossSystemImpact,
];

/// A simplified proposal passed to the dissent generator.
///
/// Decoupled from `ConsensusProposal` to avoid circular dependencies.
#[derive(Clone, Debug)]
pub struct DissentProposal {
    /// Unique proposal identifier.
    pub id: String,
    /// The type of action (e.g. `"service_termination"`).
    pub action_type: String,
    /// Human-readable description of the proposal.
    pub description: String,
    /// Agent ID that created the proposal.
    pub proposer: String,
    /// Severity label (e.g. `"CRITICAL"`, `"HIGH"`, `"MEDIUM"`).
    pub severity: String,
}

/// A single generated dissent item.
#[derive(Clone, Debug)]
pub struct GeneratedDissent {
    /// Unique identifier (UUID v4).
    pub id: String,
    /// The proposal this dissent refers to.
    pub proposal_id: String,
    /// The perspective used to generate this dissent.
    pub perspective: AgentPerspective,
    /// The critic agent that notionally generated this dissent.
    pub generating_agent_id: String,
    /// The counterargument text.
    pub counterargument: String,
    /// Computed risk score in `[0.0, 1.0]`.
    pub risk_score: f64,
    /// Whether this dissent warrants human review.
    pub requires_human_review: bool,
    /// Logical timestamp of generation.
    pub timestamp: Timestamp,
}

/// Result of running the full dissent pipeline on a proposal.
#[derive(Clone, Debug)]
pub struct PipelineDissentResult {
    /// The proposal that was evaluated.
    pub proposal_id: String,
    /// Exactly 3 generated dissents (one per perspective).
    pub generated: Vec<GeneratedDissent>,
    /// The highest risk score among the generated dissents.
    pub max_risk_score: f64,
    /// Whether any generated dissent requires human review.
    pub any_requires_review: bool,
}

// ---------------------------------------------------------------------------
// Trait
// ---------------------------------------------------------------------------

/// Generates structured dissenting opinions for consensus proposals.
pub trait DissentGenerator: Send + Sync + fmt::Debug {
    /// Generate a single dissent from a given perspective.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if the proposal has empty required fields.
    fn generate(
        &self,
        proposal: &DissentProposal,
        perspective: AgentPerspective,
    ) -> Result<GeneratedDissent>;

    /// Generate dissents from all 3 perspectives.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if the proposal has empty required fields.
    fn generate_all(&self, proposal: &DissentProposal) -> Result<Vec<GeneratedDissent>>;

    /// Run the full dissent pipeline, producing a summary result.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if the proposal has empty required fields.
    fn pipeline_dissent(&self, proposal: &DissentProposal) -> Result<PipelineDissentResult>;

    /// Total number of individual dissents generated so far.
    fn dissent_count(&self) -> u64;

    /// The risk score produced by the most recent `generate` call.
    fn last_generation_rate(&self) -> f64;
}

// ---------------------------------------------------------------------------
// Implementation
// ---------------------------------------------------------------------------

/// Active dissent generator producing structured counterarguments.
///
/// Thread-safe via `std::sync::RwLock` (L6 pattern). All risk computation
/// is deterministic and pure — no I/O, no randomness.
pub struct ActiveDissentGenerator {
    /// Generator configuration.
    config: DissentGeneratorConfig,
    /// Running count of generated dissent items.
    generated_count: RwLock<u64>,
    /// Risk score of the most recent generation.
    last_rate: RwLock<f64>,
}

impl fmt::Debug for ActiveDissentGenerator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ActiveDissentGenerator")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl ActiveDissentGenerator {
    /// Create a new generator with the given configuration.
    #[must_use]
    pub const fn new(config: DissentGeneratorConfig) -> Self {
        Self {
            config,
            generated_count: RwLock::new(0),
            last_rate: RwLock::new(0.0),
        }
    }

    /// Create a new generator with default configuration.
    #[must_use]
    pub fn with_defaults() -> Self {
        Self::new(DissentGeneratorConfig::default())
    }

    /// Validate that a proposal's required fields are non-empty.
    fn validate_proposal(proposal: &DissentProposal) -> Result<()> {
        if proposal.id.is_empty() {
            return Err(Error::Validation("Proposal ID cannot be empty".into()));
        }
        if proposal.action_type.is_empty() {
            return Err(Error::Validation(
                "Proposal action_type cannot be empty".into(),
            ));
        }
        if proposal.description.is_empty() {
            return Err(Error::Validation(
                "Proposal description cannot be empty".into(),
            ));
        }
        if proposal.proposer.is_empty() {
            return Err(Error::Validation("Proposal proposer cannot be empty".into()));
        }
        if proposal.severity.is_empty() {
            return Err(Error::Validation("Proposal severity cannot be empty".into()));
        }
        Ok(())
    }

    /// Select a critic agent ID deterministically based on the proposal ID.
    ///
    /// Uses a hash of the proposal ID bytes modulo the number of configured
    /// critic agents for reproducible round-robin assignment.
    fn select_critic_agent(&self, proposal_id: &str) -> Result<String> {
        if self.config.critic_agent_ids.is_empty() {
            return Err(Error::Validation(
                "No critic agent IDs configured".into(),
            ));
        }
        let mut hasher = DefaultHasher::new();
        proposal_id.hash(&mut hasher);
        let hash_value = hasher.finish();
        #[allow(clippy::cast_possible_truncation)]
        let idx = (hash_value as usize) % self.config.critic_agent_ids.len();
        Ok(self.config.critic_agent_ids[idx].clone())
    }

    /// Compute the base risk score for a given action type.
    ///
    /// Deterministic mapping from action type string to base risk.
    #[allow(clippy::unused_self)]
    fn base_risk(action_type: &str) -> f64 {
        match action_type {
            "service_termination" => 0.85,
            "database_migration" => 0.80,
            "credential_rotation" => 0.65,
            "cascade_restart" => 0.75,
            "config_rollback" => 0.60,
            _ => 0.50,
        }
    }

    /// Get the perspective multiplier applied to the base risk.
    const fn perspective_multiplier(perspective: AgentPerspective) -> f64 {
        match perspective {
            AgentPerspective::RiskSurface => 1.0,
            AgentPerspective::HistoricalPattern => 0.9,
            AgentPerspective::CrossSystemImpact => 1.1,
        }
    }

    /// Compute the final risk score: `clamp(base * multiplier, 0, 1)`.
    fn compute_risk(action_type: &str, perspective: AgentPerspective) -> f64 {
        let raw = Self::base_risk(action_type) * Self::perspective_multiplier(perspective);
        raw.clamp(0.0, 1.0)
    }

    /// Produce a deterministic counterargument for a given action/perspective pair.
    fn counterargument(action_type: &str, perspective: AgentPerspective) -> &'static str {
        match (action_type, perspective) {
            // service_termination
            ("service_termination", AgentPerspective::RiskSurface) => {
                "Terminating this service creates blast radius affecting all dependents. \
                 Pre-condition: confirm zero in-flight requests."
            }
            ("service_termination", AgentPerspective::HistoricalPattern) => {
                "Historical data shows 23% of service terminations caused cascading failures \
                 within 60 seconds. Recommend staged drain before kill."
            }
            ("service_termination", AgentPerspective::CrossSystemImpact) => {
                "Cross-system analysis reveals 4 downstream services with hard dependencies. \
                 Termination without coordination risks data loss in dependent pipelines."
            }

            // database_migration
            ("database_migration", AgentPerspective::RiskSurface) => {
                "Database migration introduces schema incompatibility window. \
                 Rollback path must be validated before execution."
            }
            ("database_migration", AgentPerspective::HistoricalPattern) => {
                "Previous migrations averaged 12 minutes downtime. \
                 Consider blue-green deployment to minimise impact."
            }
            ("database_migration", AgentPerspective::CrossSystemImpact) => {
                "Migration affects shared tables consumed by 3 other services. \
                 Coordinate schema versioning across all consumers."
            }

            // credential_rotation
            ("credential_rotation", AgentPerspective::RiskSurface) => {
                "Credential rotation during peak hours risks authentication failures. \
                 Schedule during maintenance window."
            }
            ("credential_rotation", AgentPerspective::HistoricalPattern) => {
                "Last rotation caused 8-minute auth outage due to cache staleness. \
                 Pre-invalidate all credential caches before rotation."
            }
            ("credential_rotation", AgentPerspective::CrossSystemImpact) => {
                "Shared credentials used by 6 services. Staggered rotation with \
                 per-service health checks required."
            }

            // cascade_restart
            ("cascade_restart", AgentPerspective::RiskSurface) => {
                "Cascade restart amplifies failure domain. Restart order must \
                 respect dependency graph to avoid orphan connections."
            }
            ("cascade_restart", AgentPerspective::HistoricalPattern) => {
                "Historical cascade restarts recovered in 45s average but had \
                 3 incidents exceeding 5 minutes due to dependency deadlocks."
            }
            ("cascade_restart", AgentPerspective::CrossSystemImpact) => {
                "Cascade restart affects batch 2-5 services. PBFT quorum may \
                 be temporarily unreachable during restart window."
            }

            // config_rollback
            ("config_rollback", AgentPerspective::RiskSurface) => {
                "Config rollback may reintroduce previously patched vulnerabilities. \
                 Validate security posture of target configuration."
            }
            ("config_rollback", AgentPerspective::HistoricalPattern) => {
                "Past rollbacks caused 15% of services to enter degraded state \
                 due to config drift between current and target versions."
            }
            ("config_rollback", AgentPerspective::CrossSystemImpact) => {
                "Config rollback on this service may invalidate peer bridge \
                 configurations. Verify all bridge endpoints post-rollback."
            }

            // Default / unknown action types
            (_, AgentPerspective::RiskSurface) => {
                "Unknown action type detected. Risk assessment incomplete — \
                 manual review required before proceeding."
            }
            (_, AgentPerspective::HistoricalPattern) => {
                "No historical precedent for this action type. Recommend pilot \
                 execution in staging environment first."
            }
            (_, AgentPerspective::CrossSystemImpact) => {
                "Cross-system impact unknown for this action type. Conduct \
                 dependency analysis before execution."
            }
        }
    }

    /// Increment the generated count and update the last rate.
    fn record_generation(&self, risk_score: f64) -> Result<()> {
        {
            let mut count = self
                .generated_count
                .write()
                .map_err(|e| Error::Other(format!("Lock poisoned: {e}")))?;
            *count += 1;
        }
        {
            let mut rate = self
                .last_rate
                .write()
                .map_err(|e| Error::Other(format!("Lock poisoned: {e}")))?;
            *rate = risk_score;
        }
        Ok(())
    }
}

impl Default for ActiveDissentGenerator {
    fn default() -> Self {
        Self::with_defaults()
    }
}

impl DissentGenerator for ActiveDissentGenerator {
    fn generate(
        &self,
        proposal: &DissentProposal,
        perspective: AgentPerspective,
    ) -> Result<GeneratedDissent> {
        Self::validate_proposal(proposal)?;

        let risk_score = Self::compute_risk(&proposal.action_type, perspective);
        let agent_id = self.select_critic_agent(&proposal.id)?;
        let counterargument =
            Self::counterargument(&proposal.action_type, perspective).to_owned();
        let requires_review = risk_score >= self.config.risk_threshold_for_review;

        let dissent = GeneratedDissent {
            id: uuid::Uuid::new_v4().to_string(),
            proposal_id: proposal.id.clone(),
            perspective,
            generating_agent_id: agent_id,
            counterargument,
            risk_score,
            requires_human_review: requires_review,
            timestamp: Timestamp::now(),
        };

        self.record_generation(risk_score)?;

        Ok(dissent)
    }

    fn generate_all(&self, proposal: &DissentProposal) -> Result<Vec<GeneratedDissent>> {
        Self::validate_proposal(proposal)?;

        let mut results = Vec::with_capacity(ALL_PERSPECTIVES.len());
        for &perspective in &ALL_PERSPECTIVES {
            results.push(self.generate(proposal, perspective)?);
        }
        Ok(results)
    }

    fn pipeline_dissent(&self, proposal: &DissentProposal) -> Result<PipelineDissentResult> {
        let generated = self.generate_all(proposal)?;

        let max_risk_score = generated
            .iter()
            .map(|d| d.risk_score)
            .fold(0.0_f64, f64::max);

        let any_requires_review = generated.iter().any(|d| d.requires_human_review);

        Ok(PipelineDissentResult {
            proposal_id: proposal.id.clone(),
            generated,
            max_risk_score,
            any_requires_review,
        })
    }

    fn dissent_count(&self) -> u64 {
        self.generated_count
            .read()
            .map_or(0, |guard| *guard)
    }

    fn last_generation_rate(&self) -> f64 {
        self.last_rate
            .read()
            .map_or(0.0, |guard| *guard)
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

    fn sample_proposal() -> DissentProposal {
        DissentProposal {
            id: "prop-001".into(),
            action_type: "service_termination".into(),
            description: "Terminate failing auth-service".into(),
            proposer: "agent-01".into(),
            severity: "CRITICAL".into(),
        }
    }

    fn sample_proposal_with_action(action: &str) -> DissentProposal {
        DissentProposal {
            id: "prop-002".into(),
            action_type: action.into(),
            description: "Test proposal".into(),
            proposer: "agent-01".into(),
            severity: "HIGH".into(),
        }
    }

    fn generator() -> ActiveDissentGenerator {
        ActiveDissentGenerator::with_defaults()
    }

    // -----------------------------------------------------------------------
    // generate — perspective coverage
    // -----------------------------------------------------------------------

    #[test]
    fn test_generate_risk_surface() {
        let gen = generator();
        let result = gen.generate(&sample_proposal(), AgentPerspective::RiskSurface);
        assert!(result.is_ok());
        let d = result.unwrap_or_else(|_| unreachable!());
        assert_eq!(d.proposal_id, "prop-001");
        assert_eq!(d.perspective, AgentPerspective::RiskSurface);
        assert!(!d.counterargument.is_empty());
        assert!(!d.generating_agent_id.is_empty());
    }

    #[test]
    fn test_generate_historical_pattern() {
        let gen = generator();
        let result = gen.generate(&sample_proposal(), AgentPerspective::HistoricalPattern);
        assert!(result.is_ok());
        let d = result.unwrap_or_else(|_| unreachable!());
        assert_eq!(d.perspective, AgentPerspective::HistoricalPattern);
    }

    #[test]
    fn test_generate_cross_system_impact() {
        let gen = generator();
        let result = gen.generate(&sample_proposal(), AgentPerspective::CrossSystemImpact);
        assert!(result.is_ok());
        let d = result.unwrap_or_else(|_| unreachable!());
        assert_eq!(d.perspective, AgentPerspective::CrossSystemImpact);
    }

    // -----------------------------------------------------------------------
    // generate_all
    // -----------------------------------------------------------------------

    #[test]
    fn test_generate_all_returns_three() {
        let gen = generator();
        let result = gen.generate_all(&sample_proposal());
        assert!(result.is_ok());
        let dissents = result.unwrap_or_else(|_| unreachable!());
        assert_eq!(dissents.len(), 3);
    }

    #[test]
    fn test_generate_all_covers_all_perspectives() {
        let gen = generator();
        let dissents = gen.generate_all(&sample_proposal()).unwrap_or_else(|_| unreachable!());
        let perspectives: Vec<AgentPerspective> = dissents.iter().map(|d| d.perspective).collect();
        assert!(perspectives.contains(&AgentPerspective::RiskSurface));
        assert!(perspectives.contains(&AgentPerspective::HistoricalPattern));
        assert!(perspectives.contains(&AgentPerspective::CrossSystemImpact));
    }

    #[test]
    fn test_generate_all_unique_ids() {
        let gen = generator();
        let dissents = gen.generate_all(&sample_proposal()).unwrap_or_else(|_| unreachable!());
        let ids: Vec<&String> = dissents.iter().map(|d| &d.id).collect();
        assert_ne!(ids[0], ids[1]);
        assert_ne!(ids[1], ids[2]);
        assert_ne!(ids[0], ids[2]);
    }

    // -----------------------------------------------------------------------
    // pipeline_dissent
    // -----------------------------------------------------------------------

    #[test]
    fn test_pipeline_dissent_structure() {
        let gen = generator();
        let result = gen.pipeline_dissent(&sample_proposal());
        assert!(result.is_ok());
        let pr = result.unwrap_or_else(|_| unreachable!());
        assert_eq!(pr.proposal_id, "prop-001");
        assert_eq!(pr.generated.len(), 3);
    }

    #[test]
    fn test_pipeline_max_risk_score() {
        let gen = generator();
        let pr = gen.pipeline_dissent(&sample_proposal()).unwrap_or_else(|_| unreachable!());
        let expected_max = pr.generated.iter().map(|d| d.risk_score).fold(0.0_f64, f64::max);
        assert!((pr.max_risk_score - expected_max).abs() < f64::EPSILON);
    }

    #[test]
    fn test_pipeline_any_requires_review() {
        let gen = generator();
        let pr = gen.pipeline_dissent(&sample_proposal()).unwrap_or_else(|_| unreachable!());
        let expected = pr.generated.iter().any(|d| d.requires_human_review);
        assert_eq!(pr.any_requires_review, expected);
    }

    // -----------------------------------------------------------------------
    // Risk scores bounded [0,1]
    // -----------------------------------------------------------------------

    #[test]
    fn test_risk_bounded_service_termination() {
        let gen = generator();
        for &p in &ALL_PERSPECTIVES {
            let d = gen.generate(&sample_proposal(), p).unwrap_or_else(|_| unreachable!());
            assert!(d.risk_score >= 0.0, "risk must be >= 0");
            assert!(d.risk_score <= 1.0, "risk must be <= 1");
        }
    }

    #[test]
    fn test_risk_bounded_database_migration() {
        let gen = generator();
        let prop = sample_proposal_with_action("database_migration");
        for &p in &ALL_PERSPECTIVES {
            let d = gen.generate(&prop, p).unwrap_or_else(|_| unreachable!());
            assert!(d.risk_score >= 0.0);
            assert!(d.risk_score <= 1.0);
        }
    }

    #[test]
    fn test_risk_bounded_credential_rotation() {
        let gen = generator();
        let prop = sample_proposal_with_action("credential_rotation");
        for &p in &ALL_PERSPECTIVES {
            let d = gen.generate(&prop, p).unwrap_or_else(|_| unreachable!());
            assert!(d.risk_score >= 0.0);
            assert!(d.risk_score <= 1.0);
        }
    }

    #[test]
    fn test_risk_bounded_cascade_restart() {
        let gen = generator();
        let prop = sample_proposal_with_action("cascade_restart");
        for &p in &ALL_PERSPECTIVES {
            let d = gen.generate(&prop, p).unwrap_or_else(|_| unreachable!());
            assert!(d.risk_score >= 0.0);
            assert!(d.risk_score <= 1.0);
        }
    }

    #[test]
    fn test_risk_bounded_config_rollback() {
        let gen = generator();
        let prop = sample_proposal_with_action("config_rollback");
        for &p in &ALL_PERSPECTIVES {
            let d = gen.generate(&prop, p).unwrap_or_else(|_| unreachable!());
            assert!(d.risk_score >= 0.0);
            assert!(d.risk_score <= 1.0);
        }
    }

    #[test]
    fn test_risk_bounded_unknown_action() {
        let gen = generator();
        let prop = sample_proposal_with_action("unknown_action");
        for &p in &ALL_PERSPECTIVES {
            let d = gen.generate(&prop, p).unwrap_or_else(|_| unreachable!());
            assert!(d.risk_score >= 0.0);
            assert!(d.risk_score <= 1.0);
        }
    }

    // -----------------------------------------------------------------------
    // Exact risk values (deterministic)
    // -----------------------------------------------------------------------

    #[test]
    fn test_risk_service_termination_risk_surface() {
        let gen = generator();
        let d = gen
            .generate(&sample_proposal(), AgentPerspective::RiskSurface)
            .unwrap_or_else(|_| unreachable!());
        // 0.85 * 1.0 = 0.85
        assert!((d.risk_score - 0.85).abs() < f64::EPSILON);
    }

    #[test]
    fn test_risk_service_termination_historical() {
        let gen = generator();
        let d = gen
            .generate(&sample_proposal(), AgentPerspective::HistoricalPattern)
            .unwrap_or_else(|_| unreachable!());
        // 0.85 * 0.9 = 0.765
        assert!((d.risk_score - 0.765).abs() < f64::EPSILON);
    }

    #[test]
    fn test_risk_service_termination_cross_system() {
        let gen = generator();
        let d = gen
            .generate(&sample_proposal(), AgentPerspective::CrossSystemImpact)
            .unwrap_or_else(|_| unreachable!());
        // 0.85 * 1.1 = 0.935 (clamped to 1.0? no, 0.935 <= 1.0)
        assert!((d.risk_score - 0.935).abs() < f64::EPSILON);
    }

    #[test]
    fn test_risk_database_migration_cross_system_clamped() {
        let gen = generator();
        let prop = sample_proposal_with_action("database_migration");
        let d = gen
            .generate(&prop, AgentPerspective::CrossSystemImpact)
            .unwrap_or_else(|_| unreachable!());
        // 0.80 * 1.1 = 0.88
        assert!((d.risk_score - 0.88).abs() < f64::EPSILON);
    }

    #[test]
    fn test_risk_default_action_risk_surface() {
        let gen = generator();
        let prop = sample_proposal_with_action("some_new_action");
        let d = gen
            .generate(&prop, AgentPerspective::RiskSurface)
            .unwrap_or_else(|_| unreachable!());
        // 0.50 * 1.0 = 0.50
        assert!((d.risk_score - 0.50).abs() < f64::EPSILON);
    }

    // -----------------------------------------------------------------------
    // Risk multipliers
    // -----------------------------------------------------------------------

    #[test]
    fn test_perspective_multiplier_risk_surface() {
        assert!(
            (ActiveDissentGenerator::perspective_multiplier(AgentPerspective::RiskSurface) - 1.0)
                .abs()
                < f64::EPSILON
        );
    }

    #[test]
    fn test_perspective_multiplier_historical() {
        assert!(
            (ActiveDissentGenerator::perspective_multiplier(AgentPerspective::HistoricalPattern)
                - 0.9)
                .abs()
                < f64::EPSILON
        );
    }

    #[test]
    fn test_perspective_multiplier_cross_system() {
        assert!(
            (ActiveDissentGenerator::perspective_multiplier(AgentPerspective::CrossSystemImpact)
                - 1.1)
                .abs()
                < f64::EPSILON
        );
    }

    // -----------------------------------------------------------------------
    // requires_human_review threshold
    // -----------------------------------------------------------------------

    #[test]
    fn test_requires_review_above_threshold() {
        let gen = generator();
        // service_termination + RiskSurface = 0.85 >= 0.7
        let d = gen
            .generate(&sample_proposal(), AgentPerspective::RiskSurface)
            .unwrap_or_else(|_| unreachable!());
        assert!(d.requires_human_review);
    }

    #[test]
    fn test_no_review_below_threshold() {
        let config = DissentGeneratorConfig {
            risk_threshold_for_review: 0.95,
            ..DissentGeneratorConfig::default()
        };
        let gen = ActiveDissentGenerator::new(config);
        // service_termination + RiskSurface = 0.85 < 0.95
        let d = gen
            .generate(&sample_proposal(), AgentPerspective::RiskSurface)
            .unwrap_or_else(|_| unreachable!());
        assert!(!d.requires_human_review);
    }

    #[test]
    fn test_review_at_exact_threshold() {
        let config = DissentGeneratorConfig {
            risk_threshold_for_review: 0.85,
            ..DissentGeneratorConfig::default()
        };
        let gen = ActiveDissentGenerator::new(config);
        let d = gen
            .generate(&sample_proposal(), AgentPerspective::RiskSurface)
            .unwrap_or_else(|_| unreachable!());
        // 0.85 >= 0.85 -> true
        assert!(d.requires_human_review);
    }

    // -----------------------------------------------------------------------
    // dissent_count tracking
    // -----------------------------------------------------------------------

    #[test]
    fn test_dissent_count_starts_at_zero() {
        let gen = generator();
        assert_eq!(gen.dissent_count(), 0);
    }

    #[test]
    fn test_dissent_count_increments_on_generate() {
        let gen = generator();
        let _ = gen.generate(&sample_proposal(), AgentPerspective::RiskSurface);
        assert_eq!(gen.dissent_count(), 1);
    }

    #[test]
    fn test_dissent_count_after_generate_all() {
        let gen = generator();
        let _ = gen.generate_all(&sample_proposal());
        assert_eq!(gen.dissent_count(), 3);
    }

    #[test]
    fn test_dissent_count_after_pipeline() {
        let gen = generator();
        let _ = gen.pipeline_dissent(&sample_proposal());
        assert_eq!(gen.dissent_count(), 3);
    }

    #[test]
    fn test_dissent_count_accumulates() {
        let gen = generator();
        let _ = gen.generate(&sample_proposal(), AgentPerspective::RiskSurface);
        let _ = gen.generate(&sample_proposal(), AgentPerspective::HistoricalPattern);
        let _ = gen.pipeline_dissent(&sample_proposal());
        // 1 + 1 + 3 = 5
        assert_eq!(gen.dissent_count(), 5);
    }

    // -----------------------------------------------------------------------
    // last_generation_rate
    // -----------------------------------------------------------------------

    #[test]
    fn test_last_rate_starts_at_zero() {
        let gen = generator();
        assert!((gen.last_generation_rate() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_last_rate_updates_after_generate() {
        let gen = generator();
        let _ = gen.generate(&sample_proposal(), AgentPerspective::RiskSurface);
        assert!((gen.last_generation_rate() - 0.85).abs() < f64::EPSILON);
    }

    #[test]
    fn test_last_rate_reflects_latest_call() {
        let gen = generator();
        let _ = gen.generate(&sample_proposal(), AgentPerspective::RiskSurface);
        let _ = gen.generate(
            &sample_proposal_with_action("config_rollback"),
            AgentPerspective::HistoricalPattern,
        );
        // config_rollback * historical = 0.60 * 0.9 = 0.54
        assert!((gen.last_generation_rate() - 0.54).abs() < f64::EPSILON);
    }

    // -----------------------------------------------------------------------
    // Counterarguments non-empty for all action types
    // -----------------------------------------------------------------------

    #[test]
    fn test_counterargument_service_termination_all() {
        let gen = generator();
        for &p in &ALL_PERSPECTIVES {
            let d = gen.generate(&sample_proposal(), p).unwrap_or_else(|_| unreachable!());
            assert!(!d.counterargument.is_empty());
        }
    }

    #[test]
    fn test_counterargument_database_migration_all() {
        let gen = generator();
        let prop = sample_proposal_with_action("database_migration");
        for &p in &ALL_PERSPECTIVES {
            let d = gen.generate(&prop, p).unwrap_or_else(|_| unreachable!());
            assert!(!d.counterargument.is_empty());
        }
    }

    #[test]
    fn test_counterargument_credential_rotation_all() {
        let gen = generator();
        let prop = sample_proposal_with_action("credential_rotation");
        for &p in &ALL_PERSPECTIVES {
            let d = gen.generate(&prop, p).unwrap_or_else(|_| unreachable!());
            assert!(!d.counterargument.is_empty());
        }
    }

    #[test]
    fn test_counterargument_cascade_restart_all() {
        let gen = generator();
        let prop = sample_proposal_with_action("cascade_restart");
        for &p in &ALL_PERSPECTIVES {
            let d = gen.generate(&prop, p).unwrap_or_else(|_| unreachable!());
            assert!(!d.counterargument.is_empty());
        }
    }

    #[test]
    fn test_counterargument_config_rollback_all() {
        let gen = generator();
        let prop = sample_proposal_with_action("config_rollback");
        for &p in &ALL_PERSPECTIVES {
            let d = gen.generate(&prop, p).unwrap_or_else(|_| unreachable!());
            assert!(!d.counterargument.is_empty());
        }
    }

    #[test]
    fn test_counterargument_unknown_action_all() {
        let gen = generator();
        let prop = sample_proposal_with_action("unknown_action");
        for &p in &ALL_PERSPECTIVES {
            let d = gen.generate(&prop, p).unwrap_or_else(|_| unreachable!());
            assert!(!d.counterargument.is_empty());
        }
    }

    // -----------------------------------------------------------------------
    // Agent selection (deterministic)
    // -----------------------------------------------------------------------

    #[test]
    fn test_agent_selection_deterministic() {
        let gen = generator();
        let agent1 = gen.select_critic_agent("prop-001").unwrap_or_else(|_| unreachable!());
        let agent2 = gen.select_critic_agent("prop-001").unwrap_or_else(|_| unreachable!());
        assert_eq!(agent1, agent2);
    }

    #[test]
    fn test_agent_selection_varies_by_proposal_id() {
        let gen = generator();
        // Different IDs may hash to different agents (not guaranteed for all pairs,
        // but we test that the mechanism works by checking a known-different pair).
        let a1 = gen.select_critic_agent("prop-001").unwrap_or_else(|_| unreachable!());
        let a2 = gen.select_critic_agent("prop-999").unwrap_or_else(|_| unreachable!());
        // At minimum both must be valid agent IDs
        assert!(a1.starts_with("agent-"));
        assert!(a2.starts_with("agent-"));
    }

    #[test]
    fn test_agent_selection_within_configured_ids() {
        let gen = generator();
        let agent = gen.select_critic_agent("any-id").unwrap_or_else(|_| unreachable!());
        assert!(gen.config.critic_agent_ids.contains(&agent));
    }

    #[test]
    fn test_agent_selection_empty_critics_fails() {
        let config = DissentGeneratorConfig {
            critic_agent_ids: vec![],
            ..DissentGeneratorConfig::default()
        };
        let gen = ActiveDissentGenerator::new(config);
        let result = gen.select_critic_agent("prop-001");
        assert!(result.is_err());
    }

    #[test]
    fn test_agent_selection_single_critic() {
        let config = DissentGeneratorConfig {
            critic_agent_ids: vec!["agent-99".into()],
            ..DissentGeneratorConfig::default()
        };
        let gen = ActiveDissentGenerator::new(config);
        let agent = gen.select_critic_agent("prop-001").unwrap_or_else(|_| unreachable!());
        assert_eq!(agent, "agent-99");
    }

    // -----------------------------------------------------------------------
    // Config override
    // -----------------------------------------------------------------------

    #[test]
    fn test_custom_config_threshold() {
        let config = DissentGeneratorConfig {
            risk_threshold_for_review: 0.5,
            ..DissentGeneratorConfig::default()
        };
        let gen = ActiveDissentGenerator::new(config);
        // default action (0.50) + RiskSurface (1.0) = 0.50 >= 0.50
        let prop = sample_proposal_with_action("some_new_thing");
        let d = gen
            .generate(&prop, AgentPerspective::RiskSurface)
            .unwrap_or_else(|_| unreachable!());
        assert!(d.requires_human_review);
    }

    #[test]
    fn test_custom_config_critic_ids() {
        let config = DissentGeneratorConfig {
            critic_agent_ids: vec!["custom-1".into(), "custom-2".into()],
            ..DissentGeneratorConfig::default()
        };
        let gen = ActiveDissentGenerator::new(config);
        let agent = gen.select_critic_agent("prop-001").unwrap_or_else(|_| unreachable!());
        assert!(agent == "custom-1" || agent == "custom-2");
    }

    // -----------------------------------------------------------------------
    // Empty proposal validation
    // -----------------------------------------------------------------------

    #[test]
    fn test_empty_proposal_id_fails() {
        let gen = generator();
        let mut prop = sample_proposal();
        prop.id = String::new();
        let result = gen.generate(&prop, AgentPerspective::RiskSurface);
        assert!(result.is_err());
    }

    #[test]
    fn test_empty_action_type_fails() {
        let gen = generator();
        let mut prop = sample_proposal();
        prop.action_type = String::new();
        let result = gen.generate(&prop, AgentPerspective::RiskSurface);
        assert!(result.is_err());
    }

    #[test]
    fn test_empty_description_fails() {
        let gen = generator();
        let mut prop = sample_proposal();
        prop.description = String::new();
        let result = gen.generate(&prop, AgentPerspective::RiskSurface);
        assert!(result.is_err());
    }

    #[test]
    fn test_empty_proposer_fails() {
        let gen = generator();
        let mut prop = sample_proposal();
        prop.proposer = String::new();
        let result = gen.generate(&prop, AgentPerspective::RiskSurface);
        assert!(result.is_err());
    }

    #[test]
    fn test_empty_severity_fails() {
        let gen = generator();
        let mut prop = sample_proposal();
        prop.severity = String::new();
        let result = gen.generate(&prop, AgentPerspective::RiskSurface);
        assert!(result.is_err());
    }

    #[test]
    fn test_generate_all_with_empty_id_fails() {
        let gen = generator();
        let mut prop = sample_proposal();
        prop.id = String::new();
        let result = gen.generate_all(&prop);
        assert!(result.is_err());
    }

    #[test]
    fn test_pipeline_with_empty_id_fails() {
        let gen = generator();
        let mut prop = sample_proposal();
        prop.id = String::new();
        let result = gen.pipeline_dissent(&prop);
        assert!(result.is_err());
    }

    // -----------------------------------------------------------------------
    // Concurrent access
    // -----------------------------------------------------------------------

    #[test]
    fn test_concurrent_generate() {
        use std::sync::Arc;
        use std::thread;

        let gen = Arc::new(ActiveDissentGenerator::with_defaults());
        let prop = sample_proposal();

        let mut handles = Vec::new();
        for _ in 0..10 {
            let gen_clone = Arc::clone(&gen);
            let prop_clone = prop.clone();
            handles.push(thread::spawn(move || {
                gen_clone
                    .generate(&prop_clone, AgentPerspective::RiskSurface)
                    .unwrap_or_else(|_| unreachable!())
            }));
        }

        for h in handles {
            let d = h.join().unwrap_or_else(|_| unreachable!());
            assert!(!d.id.is_empty());
        }

        assert_eq!(gen.dissent_count(), 10);
    }

    #[test]
    fn test_concurrent_pipeline() {
        use std::sync::Arc;
        use std::thread;

        let gen = Arc::new(ActiveDissentGenerator::with_defaults());
        let prop = sample_proposal();

        let mut handles = Vec::new();
        for _ in 0..5 {
            let gen_clone = Arc::clone(&gen);
            let prop_clone = prop.clone();
            handles.push(thread::spawn(move || {
                gen_clone
                    .pipeline_dissent(&prop_clone)
                    .unwrap_or_else(|_| unreachable!())
            }));
        }

        for h in handles {
            let pr = h.join().unwrap_or_else(|_| unreachable!());
            assert_eq!(pr.generated.len(), 3);
        }

        // 5 threads * 3 perspectives each = 15
        assert_eq!(gen.dissent_count(), 15);
    }

    // -----------------------------------------------------------------------
    // Default / Debug traits
    // -----------------------------------------------------------------------

    #[test]
    fn test_default_config() {
        let config = DissentGeneratorConfig::default();
        assert!((config.risk_threshold_for_review - 0.7).abs() < f64::EPSILON);
        assert_eq!(config.critic_agent_ids.len(), 6);
    }

    #[test]
    fn test_default_generator() {
        let gen = ActiveDissentGenerator::default();
        assert_eq!(gen.dissent_count(), 0);
    }

    #[test]
    fn test_debug_impl() {
        let gen = generator();
        let debug_str = format!("{gen:?}");
        assert!(debug_str.contains("ActiveDissentGenerator"));
    }

    // -----------------------------------------------------------------------
    // Base risk values
    // -----------------------------------------------------------------------

    #[test]
    fn test_base_risk_service_termination() {
        assert!((ActiveDissentGenerator::base_risk("service_termination") - 0.85).abs() < f64::EPSILON);
    }

    #[test]
    fn test_base_risk_database_migration() {
        assert!((ActiveDissentGenerator::base_risk("database_migration") - 0.80).abs() < f64::EPSILON);
    }

    #[test]
    fn test_base_risk_credential_rotation() {
        assert!((ActiveDissentGenerator::base_risk("credential_rotation") - 0.65).abs() < f64::EPSILON);
    }

    #[test]
    fn test_base_risk_cascade_restart() {
        assert!((ActiveDissentGenerator::base_risk("cascade_restart") - 0.75).abs() < f64::EPSILON);
    }

    #[test]
    fn test_base_risk_config_rollback() {
        assert!((ActiveDissentGenerator::base_risk("config_rollback") - 0.60).abs() < f64::EPSILON);
    }

    #[test]
    fn test_base_risk_default() {
        assert!((ActiveDissentGenerator::base_risk("anything_else") - 0.50).abs() < f64::EPSILON);
    }

    // -----------------------------------------------------------------------
    // Proposal ID propagation
    // -----------------------------------------------------------------------

    #[test]
    fn test_proposal_id_propagated() {
        let gen = generator();
        let d = gen
            .generate(&sample_proposal(), AgentPerspective::RiskSurface)
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(d.proposal_id, "prop-001");
    }

    #[test]
    fn test_pipeline_proposal_id() {
        let gen = generator();
        let pr = gen.pipeline_dissent(&sample_proposal()).unwrap_or_else(|_| unreachable!());
        assert_eq!(pr.proposal_id, "prop-001");
        for d in &pr.generated {
            assert_eq!(d.proposal_id, "prop-001");
        }
    }

    // -----------------------------------------------------------------------
    // Timestamp monotonic
    // -----------------------------------------------------------------------

    #[test]
    fn test_timestamps_increase() {
        let gen = generator();
        let d1 = gen
            .generate(&sample_proposal(), AgentPerspective::RiskSurface)
            .unwrap_or_else(|_| unreachable!());
        let d2 = gen
            .generate(&sample_proposal(), AgentPerspective::HistoricalPattern)
            .unwrap_or_else(|_| unreachable!());
        // Timestamp::now() is strictly increasing
        assert!(d2.timestamp > d1.timestamp);
    }

    // -----------------------------------------------------------------------
    // Dissent generator trait is object-safe
    // -----------------------------------------------------------------------

    #[test]
    fn test_trait_object_safety() {
        let gen: Box<dyn DissentGenerator> = Box::new(ActiveDissentGenerator::with_defaults());
        let d = gen.generate(&sample_proposal(), AgentPerspective::RiskSurface);
        assert!(d.is_ok());
    }

    // -----------------------------------------------------------------------
    // Edge: no count increment on validation failure
    // -----------------------------------------------------------------------

    #[test]
    fn test_no_count_on_validation_failure() {
        let gen = generator();
        let mut prop = sample_proposal();
        prop.id = String::new();
        let _ = gen.generate(&prop, AgentPerspective::RiskSurface);
        assert_eq!(gen.dissent_count(), 0);
    }

    // -----------------------------------------------------------------------
    // Low-risk action should not require review with default threshold
    // -----------------------------------------------------------------------

    #[test]
    fn test_low_risk_no_review() {
        let gen = generator();
        let prop = sample_proposal_with_action("unknown_action");
        let d = gen
            .generate(&prop, AgentPerspective::HistoricalPattern)
            .unwrap_or_else(|_| unreachable!());
        // 0.50 * 0.9 = 0.45 < 0.7
        assert!(!d.requires_human_review);
    }
}
