//! # Layer 8: Nexus Integration
//!
//! NEW in V2 — Nexus Controller bridge providing Kuramoto field coherence
//! tracking, K-regime awareness, STDP tool chain learning from VMS patterns,
//! evolution chamber mutation testing, and morphogenic adaptation triggers.
//!
//! ## Modules
//!
//! | Module | Name | Purpose |
//! |--------|------|---------|
//! | N01 | Field Bridge | Kuramoto r-tracking, pre/post field capture |
//! | N02 | Intent Router | 12D `IntentTensor` to service routing |
//! | N03 | Regime Manager | K-regime detection (Swarm/Fleet/Armada) |
//! | N04 | STDP Bridge | Tool chain STDP learning from service interactions |
//! | N05 | Evolution Gate | Mutation testing before deployments |
//! | N06 | Morphogenic Adapter | Adaptation triggers on |r_delta| > 0.05 |
//!
//! ## Design Constraints
//!
//! - C11: Every L4+ module has Nexus field capture (pre/post r)
//! - C12: All service interactions record STDP co-activation (+0.05/call)
//!
//! ## Related Documentation
//! - [Nexus Specs](../../ai_specs/nexus-specs/)
//! - [Scaffolding Master Plan](../../SCAFFOLDING_MASTER_PLAN.md)

pub mod field_bridge;
pub mod intent_router;
pub mod regime_manager;
pub mod stdp_bridge;
pub mod evolution_gate;
pub mod morphogenic_adapter;
