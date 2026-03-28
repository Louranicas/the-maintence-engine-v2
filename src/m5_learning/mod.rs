//! # Layer 5: Learning
//!
//! Hebbian learning, STDP integration, and pathway management.
//!
//! ## Modules
//!
//! | Module | Name | Purpose |
//! |--------|------|---------|
//! | M25 | Hebbian Manager | Pathway management |
//! | M26 | STDP Processor | Spike-timing plasticity |
//! | M27 | Pattern Recognizer | Pattern matching |
//! | M28 | Pathway Pruner | Weak pathway cleanup |
//! | M29 | Memory Consolidator | Memory layer management |
//! | M30 | Anti-Pattern Detector | Negative reinforcement |
//!
//! ## STDP Configuration
//!
//! - LTP Rate: 0.1 (Long-Term Potentiation)
//! - LTD Rate: 0.05 (Long-Term Depression)
//! - STDP Window: 100ms
//! - Decay Rate: 0.001
//!
//! ## Related Documentation
//! - [Layer Specification](../../ai_docs/layers/L05_LEARNING.md)
//! - [Hebbian Integration](../../nam/HEBBIAN_INTEGRATION.md)

pub mod antipattern;
pub mod consolidator;
pub mod decay_scheduler;
pub mod hebbian;
pub mod pattern;
pub mod pruner;
pub mod stdp;

use crate::StdpConfig;

/// Hebbian pathway between modules
#[derive(Clone, Debug)]
pub struct HebbianPathway {
    /// Unique pathway ID
    pub id: String,
    /// Source module/service
    pub source: String,
    /// Target module/service
    pub target: String,
    /// Pathway strength (0.0 - 1.0)
    pub strength: f64,
    /// Pathway type
    pub pathway_type: PathwayType,
    /// LTP (Long-Term Potentiation) events count
    pub ltp_count: u64,
    /// LTD (Long-Term Depression) events count
    pub ltd_count: u64,
    /// Total activation count
    pub activation_count: u64,
    /// STDP delta (accumulated timing-based changes)
    pub stdp_delta: f64,
    /// Success count
    pub success_count: u64,
    /// Failure count
    pub failure_count: u64,
    /// Last activation timestamp
    pub last_activation: Option<std::time::SystemTime>,
    /// Last success timestamp
    pub last_success: Option<std::time::SystemTime>,
}

impl Default for HebbianPathway {
    fn default() -> Self {
        Self {
            id: String::new(),
            source: String::new(),
            target: String::new(),
            strength: 0.5,
            pathway_type: PathwayType::ServiceToService,
            ltp_count: 0,
            ltd_count: 0,
            activation_count: 0,
            stdp_delta: 0.0,
            success_count: 0,
            failure_count: 0,
            last_activation: None,
            last_success: None,
        }
    }
}

impl HebbianPathway {
    /// Create a new pathway
    #[must_use]
    pub fn new(source: impl Into<String>, target: impl Into<String>) -> Self {
        let source = source.into();
        let target = target.into();
        let id = format!("{source}_{target}");
        Self {
            id,
            source,
            target,
            ..Default::default()
        }
    }

    /// Calculate success rate
    #[must_use]
    #[allow(clippy::cast_precision_loss)] // Counts won't exceed f64 mantissa precision
    pub fn success_rate(&self) -> f64 {
        let total = self.success_count + self.failure_count;
        if total == 0 {
            0.5 // Default neutral
        } else {
            self.success_count as f64 / total as f64
        }
    }

    /// Calculate routing weight (strength * success rate)
    #[must_use]
    pub fn routing_weight(&self) -> f64 {
        self.strength * self.success_rate()
    }

    /// Apply LTP (strengthening)
    pub fn apply_ltp(&mut self, config: &StdpConfig) {
        self.strength = (self.strength + config.ltp_rate).min(1.0);
        self.ltp_count += 1;
        self.last_activation = Some(std::time::SystemTime::now());
    }

    /// Apply LTD (weakening)
    pub fn apply_ltd(&mut self, config: &StdpConfig) {
        self.strength = (self.strength - config.ltd_rate).max(0.1);
        self.ltd_count += 1;
        self.last_activation = Some(std::time::SystemTime::now());
    }

    /// Record successful activation
    pub fn record_success(&mut self, config: &StdpConfig) {
        self.success_count += 1;
        self.activation_count += 1;
        self.apply_ltp(config);
        self.last_success = Some(std::time::SystemTime::now());
    }

    /// Record failed activation
    pub fn record_failure(&mut self, config: &StdpConfig) {
        self.failure_count += 1;
        self.activation_count += 1;
        self.apply_ltd(config);
    }

    /// Check if pathway should be pruned
    #[must_use]
    pub fn should_prune(&self, min_strength: f64, inactive_days: u64) -> bool {
        if self.strength < min_strength {
            if let Some(last) = self.last_success {
                if let Ok(duration) = std::time::SystemTime::now().duration_since(last) {
                    return duration.as_secs() > inactive_days * 86400;
                }
            }
            return true;
        }
        false
    }
}

/// Pathway type classification
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum PathwayType {
    /// Service to service communication
    #[default]
    ServiceToService,
    /// Agent to agent collaboration
    AgentToAgent,
    /// System to system integration
    SystemToSystem,
    /// Pattern to outcome association
    PatternToOutcome,
    /// Configuration to behavior mapping
    ConfigToBehavior,
    /// Metric to action trigger
    MetricToAction,
}

/// Memory consolidation layer
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MemoryLayer {
    /// Working memory (active, volatile)
    Working,
    /// Short-term memory (minutes to hours)
    ShortTerm,
    /// Long-term memory (persistent)
    LongTerm,
    /// Episodic memory (event sequences)
    Episodic,
}

/// Memory consolidation event
#[derive(Clone, Debug)]
pub struct ConsolidationEvent {
    /// Entity type
    pub entity_type: String,
    /// Entity ID
    pub entity_id: String,
    /// Source layer
    pub from_layer: MemoryLayer,
    /// Target layer
    pub to_layer: MemoryLayer,
    /// Consolidation type
    pub consolidation_type: ConsolidationType,
    /// Strength before
    pub strength_before: f64,
    /// Strength after
    pub strength_after: f64,
    /// Timestamp
    pub timestamp: std::time::SystemTime,
}

/// Consolidation type
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConsolidationType {
    /// Promote to higher layer
    Promotion,
    /// Demote to lower layer
    Demotion,
    /// Remove from memory
    Pruning,
    /// Reactivate dormant memory
    Reactivation,
}

/// Pulse event for batch pathway updates
#[derive(Clone, Debug)]
pub struct HebbianPulse {
    /// Pulse number
    pub pulse_number: u64,
    /// Trigger type
    pub trigger_type: PulseTrigger,
    /// Pathways reinforced (LTP)
    pub pathways_reinforced: u32,
    /// Pathways weakened (LTD)
    pub pathways_weakened: u32,
    /// Pathways pruned
    pub pathways_pruned: u32,
    /// New pathways created
    pub new_pathways: u32,
    /// Average strength after pulse
    pub average_strength: f64,
    /// Total pathways
    pub total_pathways: u32,
    /// Pulse duration in ms
    pub duration_ms: u64,
    /// Timestamp
    pub timestamp: std::time::SystemTime,
}

/// Pulse trigger type
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PulseTrigger {
    /// Triggered by action count threshold
    ActionCount,
    /// Triggered by time interval
    TimeInterval,
    /// Triggered by pattern detection
    PatternDetected,
    /// Manually triggered
    Manual,
}

/// Default maintenance pathways
#[must_use]
pub fn default_pathways() -> Vec<HebbianPathway> {
    vec![
        HebbianPathway::new("maintenance", "service_restart"),
        HebbianPathway::new("maintenance", "database_vacuum"),
        HebbianPathway::new("maintenance", "cache_cleanup"),
        HebbianPathway::new("maintenance", "session_rotation"),
        HebbianPathway::new("health_failure", "service_restart"),
        HebbianPathway::new("latency_spike", "cache_cleanup"),
        HebbianPathway::new("memory_pressure", "session_rotation"),
        HebbianPathway::new("consensus_proposal", "agent_vote"),
        HebbianPathway::new("dissent_detected", "learning_update"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pathway_creation() {
        let pathway = HebbianPathway::new("source", "target");
        assert_eq!(pathway.id, "source_target");
        assert!((pathway.strength - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_ltp_application() {
        let mut pathway = HebbianPathway::new("s", "t");
        let config = StdpConfig::default();

        pathway.apply_ltp(&config);
        assert!((pathway.strength - 0.6).abs() < f64::EPSILON);
        assert_eq!(pathway.ltp_count, 1);
    }

    #[test]
    fn test_ltd_application() {
        let mut pathway = HebbianPathway::new("s", "t");
        let config = StdpConfig::default();

        pathway.apply_ltd(&config);
        assert!((pathway.strength - 0.45).abs() < f64::EPSILON);
        assert_eq!(pathway.ltd_count, 1);
    }

    #[test]
    fn test_success_rate() {
        let mut pathway = HebbianPathway::new("s", "t");
        pathway.success_count = 8;
        pathway.failure_count = 2;

        assert!((pathway.success_rate() - 0.8).abs() < f64::EPSILON);
    }

    #[test]
    fn test_default_pathways() {
        let pathways = default_pathways();
        assert!(pathways.len() >= 9);
    }
}
