//! # M43: NAM Foundation Primitives
//!
//! Core vocabulary types for NAM (Non-Anthropocentric Model) compliance
//! used by all M01–M06 foundation modules.
//!
//! ## NAM Requirements Addressed
//!
//! | Requirement | Type | Purpose |
//! |-------------|------|---------|
//! | R2 | [`LearningSignal`], [`Outcome`] | Hebbian routing feedback |
//! | R3 | [`Dissent`], [`Confidence`] | Dissent capture with uncertainty |
//! | R5 | [`AgentOrigin`], [`HUMAN_AGENT_TAG`] | Human @0.A as peer agent |
//!
//! ## Related Documentation
//! - [NAM Spec](../../ai_specs/NAM_SPEC.md)
//! - [Layer Specification](../../ai_docs/layers/L01_FOUNDATION.md)

use std::fmt;

use crate::AgentRole;

// ============================================================================
// Constants
// ============================================================================

/// Constant tag for the human agent (NAM R5).
///
/// The human is registered as agent `@0.A` — a peer, not a supervisor.
pub const HUMAN_AGENT_TAG: &str = "@0.A";

/// NAM compliance layer identifier for L1.
pub const LAYER_ID: &str = "L1";

/// Module count in this layer (M00–M08).
pub const MODULE_COUNT: u8 = 9;

// ============================================================================
// AgentOrigin
// ============================================================================

/// Agent identity — who performed an action (NAM R5 `HumanAsAgent`).
///
/// Every observable action in the system should be attributed to an agent.
/// The human is a peer agent (`@0.A`), not a supervisor.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub enum AgentOrigin {
    /// Human operator — @0.A peer, not supervisor.
    Human {
        /// Human agent tag (typically [`HUMAN_AGENT_TAG`]).
        tag: String,
    },
    /// ULTRAPLATE service acting autonomously.
    Service {
        /// Service identifier (e.g. "maintenance-engine", "synthex").
        service_id: String,
    },
    /// CVA-NAM fleet agent with role.
    Agent {
        /// Unique agent identifier.
        agent_id: String,
        /// Agent's role in the fleet.
        role: AgentRole,
    },
    /// System-level automated operation (no specific agent).
    #[default]
    System,
}

impl AgentOrigin {
    /// Create a human agent origin with the default `@0.A` tag.
    #[must_use]
    pub fn human() -> Self {
        Self::Human {
            tag: HUMAN_AGENT_TAG.to_string(),
        }
    }

    /// Create a service agent origin.
    #[must_use]
    pub fn service(service_id: impl Into<String>) -> Self {
        Self::Service {
            service_id: service_id.into(),
        }
    }

    /// Create a fleet agent origin with a role.
    #[must_use]
    pub fn agent(agent_id: impl Into<String>, role: AgentRole) -> Self {
        Self::Agent {
            agent_id: agent_id.into(),
            role,
        }
    }
}

impl fmt::Display for AgentOrigin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Human { tag } => write!(f, "Human({tag})"),
            Self::Service { service_id } => write!(f, "Service({service_id})"),
            Self::Agent { agent_id, role } => write!(f, "Agent({agent_id}, {role:?})"),
            Self::System => write!(f, "System"),
        }
    }
}

// ============================================================================
// AgentOrigin → AgentId bridge
// ============================================================================

impl From<&AgentOrigin> for super::shared_types::AgentId {
    fn from(origin: &AgentOrigin) -> Self {
        match origin {
            AgentOrigin::Human { tag } => Self::from_raw(format!("human:{tag}")),
            AgentOrigin::Service { service_id } => Self::service(service_id),
            AgentOrigin::Agent { agent_id, .. } => Self::agent(agent_id),
            AgentOrigin::System => Self::system(),
        }
    }
}

// ============================================================================
// Confidence
// ============================================================================

/// Confidence interval for uncertain values (NAM R3 `DissentCapture`).
///
/// All values are clamped to `[0.0, 1.0]` at construction.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Confidence {
    /// Point estimate (0.0–1.0).
    pub value: f64,
    /// Lower bound of confidence interval.
    pub lower: f64,
    /// Upper bound of confidence interval.
    pub upper: f64,
}

impl Default for Confidence {
    fn default() -> Self {
        Self::certain()
    }
}

impl Confidence {
    /// Full certainty — value = 1.0, interval [1.0, 1.0].
    #[must_use]
    pub const fn certain() -> Self {
        Self {
            value: 1.0,
            lower: 1.0,
            upper: 1.0,
        }
    }

    /// Maximum uncertainty — value = 0.5, interval [0.0, 1.0].
    #[must_use]
    pub const fn uncertain() -> Self {
        Self {
            value: 0.5,
            lower: 0.0,
            upper: 1.0,
        }
    }

    /// Create a new confidence with validated bounds.
    ///
    /// Values are clamped to `[0.0, 1.0]`. If `lower > upper`, they are swapped.
    #[must_use]
    pub fn new(value: f64, lower: f64, upper: f64) -> Self {
        let v = value.clamp(0.0, 1.0);
        let mut lo = lower.clamp(0.0, 1.0);
        let mut hi = upper.clamp(0.0, 1.0);
        if lo > hi {
            std::mem::swap(&mut lo, &mut hi);
        }
        Self {
            value: v,
            lower: lo,
            upper: hi,
        }
    }

    /// Validate that all fields are in `[0.0, 1.0]` and `lower <= upper`.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        (0.0..=1.0).contains(&self.value)
            && (0.0..=1.0).contains(&self.lower)
            && (0.0..=1.0).contains(&self.upper)
            && self.lower <= self.upper
    }
}

impl fmt::Display for Confidence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:.3} [{:.3}, {:.3}]",
            self.value, self.lower, self.upper
        )
    }
}

// ============================================================================
// Outcome
// ============================================================================

/// Outcome of an operation for learning feedback (NAM R2 `HebbianRouting`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Outcome {
    /// The operation completed successfully.
    Success,
    /// The operation failed.
    Failure,
    /// The operation partially succeeded.
    Partial,
}

impl fmt::Display for Outcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Success => write!(f, "Success"),
            Self::Failure => write!(f, "Failure"),
            Self::Partial => write!(f, "Partial"),
        }
    }
}

// ============================================================================
// LearningSignal
// ============================================================================

/// Learning signal emitted after an operation completes (NAM R2 `HebbianRouting`).
///
/// Upper layers consume these signals to strengthen or weaken Hebbian pathways
/// via STDP (Spike-Timing-Dependent Plasticity).
#[derive(Debug, Clone, PartialEq)]
pub struct LearningSignal {
    /// Source module or operation that generated this signal.
    pub source: String,
    /// Whether the operation succeeded, failed, or partially succeeded.
    pub outcome: Outcome,
    /// Magnitude of the learning signal (0.0–1.0).
    pub magnitude: f64,
    /// Associated Hebbian pathway ID (if known).
    pub pathway_id: Option<String>,
}

impl LearningSignal {
    /// Create a success signal with full magnitude.
    #[must_use]
    pub fn success(source: impl Into<String>) -> Self {
        Self {
            source: source.into(),
            outcome: Outcome::Success,
            magnitude: 1.0,
            pathway_id: None,
        }
    }

    /// Create a failure signal with full magnitude.
    #[must_use]
    pub fn failure(source: impl Into<String>) -> Self {
        Self {
            source: source.into(),
            outcome: Outcome::Failure,
            magnitude: 1.0,
            pathway_id: None,
        }
    }

    /// Create a partial-success signal with given magnitude.
    #[must_use]
    pub fn partial(source: impl Into<String>, magnitude: f64) -> Self {
        Self {
            source: source.into(),
            outcome: Outcome::Partial,
            magnitude: magnitude.clamp(0.0, 1.0),
            pathway_id: None,
        }
    }

    /// Attach a Hebbian pathway ID to this signal.
    #[must_use]
    pub fn with_pathway(mut self, pathway_id: impl Into<String>) -> Self {
        self.pathway_id = Some(pathway_id.into());
        self
    }
}

// ============================================================================
// Dissent
// ============================================================================

/// Structured dissent record (NAM R3 `DissentCapture`).
///
/// Minority opinions are first-class data in NAM — they are recorded,
/// not suppressed. Every dissent captures who dissented, what they
/// disagreed with, and their reasoning.
#[derive(Debug, Clone, PartialEq)]
pub struct Dissent {
    /// Agent expressing dissent.
    pub agent: AgentOrigin,
    /// What this dissent targets (decision ID, config key, etc.).
    pub target: String,
    /// Reasoning for the dissent.
    pub reasoning: String,
    /// Confidence in the dissent (0.0–1.0).
    pub confidence: f64,
    /// Proposed alternative (if any).
    pub alternative: Option<String>,
}

impl Dissent {
    /// Create a new dissent record.
    #[must_use]
    pub fn new(
        agent: AgentOrigin,
        target: impl Into<String>,
        reasoning: impl Into<String>,
    ) -> Self {
        Self {
            agent,
            target: target.into(),
            reasoning: reasoning.into(),
            confidence: 1.0,
            alternative: None,
        }
    }

    /// Set the confidence level for this dissent.
    #[must_use]
    pub const fn with_confidence(mut self, confidence: f64) -> Self {
        // const-compatible clamp
        let mut val = confidence;
        if val < 0.0 {
            val = 0.0;
        }
        if val > 1.0 {
            val = 1.0;
        }
        self.confidence = val;
        self
    }

    /// Propose an alternative action or value.
    #[must_use]
    pub fn with_alternative(mut self, alternative: impl Into<String>) -> Self {
        self.alternative = Some(alternative.into());
        self
    }

    /// Validate that confidence is in `[0.0, 1.0]`.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        (0.0..=1.0).contains(&self.confidence)
    }
}

impl fmt::Display for Dissent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Dissent({} on '{}': {} [conf={:.2}])",
            self.agent, self.target, self.reasoning, self.confidence
        )
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AgentRole;

    // ====================================================================
    // AgentOrigin tests
    // ====================================================================

    #[test]
    fn test_agent_origin_human_constructor() {
        let origin = AgentOrigin::human();
        assert_eq!(
            origin,
            AgentOrigin::Human {
                tag: "@0.A".to_string()
            }
        );
    }

    #[test]
    fn test_agent_origin_service_constructor() {
        let origin = AgentOrigin::service("maintenance-engine");
        assert_eq!(
            origin,
            AgentOrigin::Service {
                service_id: "maintenance-engine".to_string()
            }
        );
    }

    #[test]
    fn test_agent_origin_agent_constructor() {
        let origin = AgentOrigin::agent("agent-001", AgentRole::Validator);
        assert_eq!(
            origin,
            AgentOrigin::Agent {
                agent_id: "agent-001".to_string(),
                role: AgentRole::Validator
            }
        );
    }

    #[test]
    fn test_agent_origin_default_is_system() {
        assert_eq!(AgentOrigin::default(), AgentOrigin::System);
    }

    #[test]
    fn test_agent_origin_display_all_variants() {
        assert_eq!(AgentOrigin::human().to_string(), "Human(@0.A)");
        assert_eq!(
            AgentOrigin::service("synthex").to_string(),
            "Service(synthex)"
        );
        assert!(AgentOrigin::agent("a1", AgentRole::Critic)
            .to_string()
            .contains("Agent(a1"));
        assert_eq!(AgentOrigin::System.to_string(), "System");
    }

    #[test]
    fn test_agent_origin_clone_eq() {
        let origin = AgentOrigin::service("san-k7");
        let cloned = origin.clone();
        assert_eq!(origin, cloned);
    }

    #[test]
    fn test_agent_origin_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(AgentOrigin::human());
        set.insert(AgentOrigin::System);
        set.insert(AgentOrigin::service("a"));
        assert_eq!(set.len(), 3);
    }

    // ====================================================================
    // Confidence tests
    // ====================================================================

    #[test]
    fn test_confidence_certain() {
        let c = Confidence::certain();
        assert!((c.value - 1.0).abs() < f64::EPSILON);
        assert!((c.lower - 1.0).abs() < f64::EPSILON);
        assert!((c.upper - 1.0).abs() < f64::EPSILON);
        assert!(c.is_valid());
    }

    #[test]
    fn test_confidence_uncertain() {
        let c = Confidence::uncertain();
        assert!((c.value - 0.5).abs() < f64::EPSILON);
        assert!(c.lower.abs() < f64::EPSILON);
        assert!((c.upper - 1.0).abs() < f64::EPSILON);
        assert!(c.is_valid());
    }

    #[test]
    fn test_confidence_default_is_certain() {
        let c = Confidence::default();
        assert_eq!(c, Confidence::certain());
    }

    #[test]
    fn test_confidence_new_clamps() {
        let c = Confidence::new(1.5, -0.5, 2.0);
        assert!((c.value - 1.0).abs() < f64::EPSILON);
        assert!(c.lower.abs() < f64::EPSILON);
        assert!((c.upper - 1.0).abs() < f64::EPSILON);
        assert!(c.is_valid());
    }

    #[test]
    fn test_confidence_new_swaps_bounds() {
        let c = Confidence::new(0.5, 0.8, 0.2);
        assert!((c.lower - 0.2).abs() < f64::EPSILON);
        assert!((c.upper - 0.8).abs() < f64::EPSILON);
        assert!(c.is_valid());
    }

    #[test]
    fn test_confidence_display() {
        let c = Confidence::new(0.7, 0.5, 0.9);
        let display = c.to_string();
        assert!(display.contains("0.700"));
        assert!(display.contains("0.500"));
        assert!(display.contains("0.900"));
    }

    // ====================================================================
    // LearningSignal tests
    // ====================================================================

    #[test]
    fn test_learning_signal_success() {
        let sig = LearningSignal::success("M01");
        assert_eq!(sig.outcome, Outcome::Success);
        assert!((sig.magnitude - 1.0).abs() < f64::EPSILON);
        assert!(sig.pathway_id.is_none());
    }

    #[test]
    fn test_learning_signal_failure() {
        let sig = LearningSignal::failure("M02");
        assert_eq!(sig.outcome, Outcome::Failure);
        assert!((sig.magnitude - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_learning_signal_partial() {
        let sig = LearningSignal::partial("M03", 0.6);
        assert_eq!(sig.outcome, Outcome::Partial);
        assert!((sig.magnitude - 0.6).abs() < f64::EPSILON);
    }

    #[test]
    fn test_learning_signal_partial_clamps() {
        let sig = LearningSignal::partial("M04", 1.5);
        assert!((sig.magnitude - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_learning_signal_with_pathway() {
        let sig = LearningSignal::success("M05").with_pathway("path-001");
        assert_eq!(sig.pathway_id.as_deref(), Some("path-001"));
    }

    // ====================================================================
    // Dissent tests
    // ====================================================================

    #[test]
    fn test_dissent_construction() {
        let d = Dissent::new(AgentOrigin::human(), "decision-001", "insufficient data");
        assert_eq!(d.agent, AgentOrigin::human());
        assert_eq!(d.target, "decision-001");
        assert_eq!(d.reasoning, "insufficient data");
        assert!((d.confidence - 1.0).abs() < f64::EPSILON);
        assert!(d.alternative.is_none());
        assert!(d.is_valid());
    }

    #[test]
    fn test_dissent_with_confidence() {
        let d = Dissent::new(AgentOrigin::System, "cfg-port", "too low")
            .with_confidence(0.7);
        assert!((d.confidence - 0.7).abs() < f64::EPSILON);
    }

    #[test]
    fn test_dissent_with_alternative() {
        let d = Dissent::new(AgentOrigin::System, "action-001", "risky")
            .with_alternative("use safer approach");
        assert_eq!(d.alternative.as_deref(), Some("use safer approach"));
    }

    #[test]
    fn test_dissent_display() {
        let d = Dissent::new(AgentOrigin::human(), "d-001", "wrong approach");
        let display = d.to_string();
        assert!(display.contains("Dissent"));
        assert!(display.contains("d-001"));
        assert!(display.contains("wrong approach"));
    }

    #[test]
    fn test_dissent_confidence_clamps() {
        let d = Dissent::new(AgentOrigin::System, "x", "y").with_confidence(1.5);
        assert!((d.confidence - 1.0).abs() < f64::EPSILON);
        assert!(d.is_valid());
    }

    // ====================================================================
    // Outcome tests
    // ====================================================================

    #[test]
    fn test_outcome_display() {
        assert_eq!(Outcome::Success.to_string(), "Success");
        assert_eq!(Outcome::Failure.to_string(), "Failure");
        assert_eq!(Outcome::Partial.to_string(), "Partial");
    }

    #[test]
    fn test_outcome_equality() {
        assert_eq!(Outcome::Success, Outcome::Success);
        assert_ne!(Outcome::Success, Outcome::Failure);
    }

    // ====================================================================
    // Constants tests
    // ====================================================================

    #[test]
    fn test_human_agent_tag() {
        assert_eq!(HUMAN_AGENT_TAG, "@0.A");
    }

    #[test]
    fn test_layer_constants() {
        assert_eq!(LAYER_ID, "L1");
        assert_eq!(MODULE_COUNT, 9);
    }

    // ====================================================================
    // Integration: AgentOrigin uses crate::AgentRole
    // ====================================================================

    #[test]
    fn test_agent_origin_with_all_roles() {
        let roles = [
            AgentRole::Validator,
            AgentRole::Explorer,
            AgentRole::Critic,
            AgentRole::Integrator,
            AgentRole::Historian,
        ];
        for role in roles {
            let origin = AgentOrigin::agent("test", role);
            if let AgentOrigin::Agent { role: r, .. } = &origin {
                assert_eq!(*r, role);
            } else {
                panic!("Expected Agent variant");
            }
        }
    }

    #[test]
    fn test_agent_origin_role_vote_weight_accessible() {
        let origin = AgentOrigin::agent("critic-1", AgentRole::Critic);
        if let AgentOrigin::Agent { role, .. } = &origin {
            assert!((role.vote_weight() - 1.2).abs() < f64::EPSILON);
        } else {
            panic!("Expected Agent variant");
        }
    }

    // ====================================================================
    // AgentOrigin → AgentId bridge tests
    // ====================================================================

    #[test]
    fn test_agent_id_from_human_origin() {
        use crate::m1_foundation::shared_types::AgentId;
        let origin = AgentOrigin::human();
        let id = AgentId::from(&origin);
        assert_eq!(id.as_str(), "human:@0.A");
        assert!(id.is_human());
    }

    #[test]
    fn test_agent_id_from_service_origin() {
        use crate::m1_foundation::shared_types::AgentId;
        let origin = AgentOrigin::service("synthex");
        let id = AgentId::from(&origin);
        assert_eq!(id.as_str(), "svc:synthex");
        assert!(id.is_service());
    }

    #[test]
    fn test_agent_id_from_agent_origin() {
        use crate::m1_foundation::shared_types::AgentId;
        let origin = AgentOrigin::agent("a-001", AgentRole::Validator);
        let id = AgentId::from(&origin);
        assert_eq!(id.as_str(), "agent:a-001");
        assert!(id.is_agent());
    }

    #[test]
    fn test_agent_id_from_system_origin() {
        use crate::m1_foundation::shared_types::AgentId;
        let origin = AgentOrigin::System;
        let id = AgentId::from(&origin);
        assert_eq!(id.as_str(), "sys:system");
        assert!(id.is_system());
    }

    #[test]
    fn test_agent_id_from_all_origins() {
        use crate::m1_foundation::shared_types::AgentId;
        let origins = vec![
            AgentOrigin::human(),
            AgentOrigin::service("san-k7"),
            AgentOrigin::agent("x", AgentRole::Explorer),
            AgentOrigin::System,
        ];
        for origin in &origins {
            let _id = AgentId::from(origin);
        }
    }
}
