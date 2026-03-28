//! # The Maintenance Engine V2
//!
//! Next-generation maintenance framework for the ULTRAPLATE Developer Environment,
//! evolved from ME V1 with deep Nexus Controller and Oscillating Vortex Memory integration.
//! Provides autonomous service management, health monitoring, Hebbian learning,
//! PBFT consensus, NAM-compliant multi-agent coordination, Kuramoto field coherence
//! tracking, and morphogenic adaptation.
//!
//! ## Architecture Overview
//!
//! 8 layers with 48+ modular components:
//!
//! | Layer | Name | Modules | Purpose |
//! |-------|------|---------|---------|
//! | L1 | Foundation | M00-M08, M43 | Error handling, config, logging, signals, tensor |
//! | L2 | Services | M09-M12 | Service registry, health, lifecycle, resilience |
//! | L3 | Core Logic | M13-M18 | Pipeline, remediation, confidence, action, outcome |
//! | L4 | Integration | M19-M24, M42, M46-M47 | REST, gRPC, WS, IPC, bridges |
//! | L5 | Learning | M25-M30, M41 | Hebbian, STDP, pattern, pruning |
//! | L6 | Consensus | M31-M36 | PBFT, agents, voting, dissent, quorum |
//! | L7 | Observer | M37-M40, M44-M45 | Correlation, emergence, RALPH, thermal |
//! | L8 | Nexus | N01-N06 | Field bridge, intent routing, K-regime, STDP bridge |
//!
//! ## 12D Tensor Encoding
//!
//! Every service state is encoded as a 12-dimensional tensor:
//! ```text
//! [service_id, port, tier, deps, agents, protocol, health, uptime, synergy, latency, error_rate, temporal]
//! ```
//!
//! ## NAM Compliance Target: 95%
//!
//! - R1 `SelfQuery`: Autonomous SQL query loops + Nexus field capture
//! - R2 `HebbianRouting`: Pathway-weighted routing + STDP co-activation
//! - R3 `DissentCapture`: Disagreement learning + cascade semantics
//! - R4 `FieldVisualization`: Machine-readable topology + Kuramoto r
//! - R5 `HumanAsAgent`: Human @0.A integration at Tier 0
//!
//! ## Related Documentation
//! - [AI Docs Index](../ai_docs/INDEX.md)
//! - [Scaffolding Master Plan](../SCAFFOLDING_MASTER_PLAN.md)
//! - [Nexus Specs](../ai_specs/nexus-specs/)

#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used)]
#![warn(clippy::pedantic)]
#![warn(missing_docs)]

// Layer 1: Foundation
pub mod m1_foundation;

// Layer 2: Services
pub mod m2_services;

// Layer 3: Core Logic
pub mod m3_core_logic;

// Layer 4: Integration
pub mod m4_integration;

// Layer 5: Learning
pub mod m5_learning;

// Layer 6: Consensus
pub mod m6_consensus;

// Layer 7: Observer
pub mod m7_observer;

// Layer 8: Nexus Integration (NEW in V2)
pub mod nexus;

// Database Manager
pub mod database;

// Tool definitions for Tool Library registration
pub mod tools;

// Central Engine Orchestrator
pub mod engine;

// Re-export commonly used types
pub use m1_foundation::{Error, Result};

/// Prelude module for convenient imports
pub mod prelude {
    pub use crate::engine::Engine;
    pub use crate::m1_foundation::{Error, Result};
    pub use crate::m2_services::ServiceState;
    pub use crate::m5_learning::HebbianPathway;
    pub use crate::m6_consensus::ConsensusProposal;
    pub use crate::m7_observer::ObserverLayer;
}

/// 12-dimensional tensor encoding for service state
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Tensor12D {
    /// D0: Service ID (normalized hash)
    pub service_id: f64,
    /// D1: Port (port/65535)
    pub port: f64,
    /// D2: Tier (tier/6)
    pub tier: f64,
    /// D3: Dependency count (log normalized)
    pub dependency_count: f64,
    /// D4: Agent count (agents/40)
    pub agent_count: f64,
    /// D5: Protocol (enum encoding)
    pub protocol: f64,
    /// D6: Health score (0-1)
    pub health_score: f64,
    /// D7: Uptime ratio (0-1)
    pub uptime: f64,
    /// D8: Synergy score (0-1)
    pub synergy: f64,
    /// D9: Latency (1 - `latency_ms`/2000)
    pub latency: f64,
    /// D10: Error rate (0-1)
    pub error_rate: f64,
    /// D11: Temporal context (time encoding)
    pub temporal_context: f64,
}

impl Tensor12D {
    /// Create a new tensor from raw dimensions
    #[must_use]
    pub const fn new(dimensions: [f64; 12]) -> Self {
        Self {
            service_id: dimensions[0],
            port: dimensions[1],
            tier: dimensions[2],
            dependency_count: dimensions[3],
            agent_count: dimensions[4],
            protocol: dimensions[5],
            health_score: dimensions[6],
            uptime: dimensions[7],
            synergy: dimensions[8],
            latency: dimensions[9],
            error_rate: dimensions[10],
            temporal_context: dimensions[11],
        }
    }

    /// Convert tensor to byte representation (96 bytes)
    #[must_use]
    pub fn to_bytes(&self) -> [u8; 96] {
        let mut bytes = [0u8; 96];
        let dims = self.to_array();
        for (i, &val) in dims.iter().enumerate() {
            bytes[i * 8..(i + 1) * 8].copy_from_slice(&val.to_le_bytes());
        }
        bytes
    }

    /// Convert tensor to array
    #[must_use]
    pub const fn to_array(&self) -> [f64; 12] {
        [
            self.service_id,
            self.port,
            self.tier,
            self.dependency_count,
            self.agent_count,
            self.protocol,
            self.health_score,
            self.uptime,
            self.synergy,
            self.latency,
            self.error_rate,
            self.temporal_context,
        ]
    }

    /// Calculate Euclidean distance to another tensor
    #[must_use]
    pub fn distance(&self, other: &Self) -> f64 {
        let a = self.to_array();
        let b = other.to_array();
        a.iter()
            .zip(b.iter())
            .map(|(x, y)| (x - y).powi(2))
            .sum::<f64>()
            .sqrt()
    }

    /// Validate all dimensions are in [0, 1] range
    ///
    /// # Errors
    ///
    /// Returns `Error::TensorValidation` if any dimension is outside [0, 1] range,
    /// or if any dimension contains NaN or infinite values.
    pub fn validate(&self) -> Result<()> {
        for (i, &val) in self.to_array().iter().enumerate() {
            if !(0.0..=1.0).contains(&val) {
                return Err(Error::TensorValidation {
                    dimension: i,
                    value: val,
                });
            }
            if val.is_nan() || val.is_infinite() {
                return Err(Error::TensorValidation {
                    dimension: i,
                    value: val,
                });
            }
        }
        Ok(())
    }

    /// Clamp and normalize all dimensions to [0, 1]
    pub fn clamp_normalize(&mut self) {
        let dims = [
            &mut self.service_id,
            &mut self.port,
            &mut self.tier,
            &mut self.dependency_count,
            &mut self.agent_count,
            &mut self.protocol,
            &mut self.health_score,
            &mut self.uptime,
            &mut self.synergy,
            &mut self.latency,
            &mut self.error_rate,
            &mut self.temporal_context,
        ];
        for val in dims {
            if val.is_nan() {
                *val = 0.5;
            }
            *val = val.clamp(0.0, 1.0);
        }
    }
}

/// STDP (Spike-Timing-Dependent Plasticity) configuration
#[derive(Clone, Copy, Debug)]
pub struct StdpConfig {
    /// Long-Term Potentiation rate (strengthening)
    pub ltp_rate: f64,
    /// Long-Term Depression rate (weakening)
    pub ltd_rate: f64,
    /// STDP timing window in milliseconds
    pub stdp_window_ms: u64,
    /// Decay rate for unused pathways
    pub decay_rate: f64,
}

impl Default for StdpConfig {
    fn default() -> Self {
        Self {
            ltp_rate: 0.1,
            ltd_rate: 0.05,
            stdp_window_ms: 100,
            decay_rate: 0.1, // HRS-001 fix: was 0.001 (100x too low)
        }
    }
}

/// Escalation tier for remediation actions
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EscalationTier {
    /// L0: Auto-execute when confidence >= 0.9
    L0AutoExecute,
    /// L1: Notify human, execute after timeout
    L1NotifyHuman,
    /// L2: Require explicit human approval
    L2RequireApproval,
    /// L3: PBFT consensus (quorum 27/40)
    L3PbftConsensus,
}

/// Agent role in the heterogeneous agent system (NAM-05)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AgentRole {
    /// Correctness verification (20 agents)
    Validator,
    /// Alternative detection (8 agents)
    Explorer,
    /// Flaw detection (6 agents, weight 1.2)
    Critic,
    /// Cross-system impact (4 agents)
    Integrator,
    /// Precedent matching (2 agents)
    Historian,
}

impl AgentRole {
    /// Get the default vote weight for this role
    #[must_use]
    pub const fn vote_weight(&self) -> f64 {
        match self {
            Self::Validator | Self::Integrator => 1.0,
            Self::Explorer | Self::Historian => 0.8,
            Self::Critic => 1.2,
        }
    }

    /// Get the target count for this role in a 40-agent fleet
    #[must_use]
    pub const fn target_count(&self) -> usize {
        match self {
            Self::Validator => 20,
            Self::Explorer => 8,
            Self::Critic => 6,
            Self::Integrator => 4,
            Self::Historian => 2,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tensor_validation() {
        let mut tensor = Tensor12D::default();
        assert!(tensor.validate().is_ok());

        tensor.health_score = 1.5; // Invalid
        assert!(tensor.validate().is_err());

        tensor.clamp_normalize();
        assert!(tensor.validate().is_ok());
        assert!((tensor.health_score - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_tensor_distance() {
        let t1 = Tensor12D::default();
        let t2 = Tensor12D::new([0.5; 12]);
        let dist = t1.distance(&t2);
        assert!(dist > 0.0);
    }

    #[test]
    fn test_agent_role_weights() {
        assert!((AgentRole::Critic.vote_weight() - 1.2).abs() < f64::EPSILON);
        assert_eq!(AgentRole::Validator.target_count(), 20);
    }
}
