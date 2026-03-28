//! # M00: Shared Types
//!
//! Pure vocabulary types for cross-module coordination within the Maintenance Engine.
//! This module contains **zero logic and zero I/O** — only type definitions,
//! constructors, and trivial accessors.
//!
//! ## Layer: L1 (Foundation)
//! ## Dependencies: None (leaf module)
//!
//! ## Types
//!
//! | Type | Purpose |
//! |------|---------|
//! | [`ModuleId`] | Typed identity for all 42 modules (M01-M42) |
//! | [`AgentId`] | Typed bridge from [`AgentOrigin`](super::AgentOrigin) to operational `&str` |
//! | [`Timestamp`] | Monotonic cycle counter (no chrono) |
//! | [`HealthReport`] | Per-module health snapshot |
//! | [`DimensionIndex`] | Enum mapping 12D tensor dimension names |
//! | [`CoverageBitmap`] | Bitmask tracking which of the 12 dimensions are populated |
//!
//! ## Design Invariants
//!
//! - Every type is `Send + Sync`
//! - No `unsafe`, no panics, no I/O
//! - `const fn` wherever the compiler allows
//! - All constructors are `#[must_use]`

use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};

// ============================================================================
// ModuleId
// ============================================================================

/// Typed identity for a module in the Maintenance Engine.
///
/// Wraps a `&'static str` with compile-time constants for M01–M42.
/// Replaces free-text strings in places like [`LearningSignal.source`](super::LearningSignal).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ModuleId(&'static str);

impl ModuleId {
    // L1: Foundation
    /// M01: Error Taxonomy
    pub const M01: Self = Self("M01");
    /// M02: Configuration
    pub const M02: Self = Self("M02");
    /// M03: Logging
    pub const M03: Self = Self("M03");
    /// M04: Metrics
    pub const M04: Self = Self("M04");
    /// M05: State Persistence
    pub const M05: Self = Self("M05");
    /// M06: Resource Manager
    pub const M06: Self = Self("M06");

    // L2: Services
    /// M07: Service Types
    pub const M07: Self = Self("M07");
    /// M08: Health Monitor
    pub const M08: Self = Self("M08");
    /// M09: Lifecycle Manager
    pub const M09: Self = Self("M09");
    /// M10: Service Discovery
    pub const M10: Self = Self("M10");
    /// M11: Load Balancer
    pub const M11: Self = Self("M11");
    /// M12: Circuit Breaker
    pub const M12: Self = Self("M12");

    // L3: Core Logic
    /// M13: Pipeline Manager
    pub const M13: Self = Self("M13");
    /// M14: Remediation Engine
    pub const M14: Self = Self("M14");
    /// M15: Confidence Calculator
    pub const M15: Self = Self("M15");
    /// M16: Action Executor
    pub const M16: Self = Self("M16");
    /// M17: Outcome Recorder
    pub const M17: Self = Self("M17");
    /// M18: Feedback Loop
    pub const M18: Self = Self("M18");

    // L4: Integration
    /// M19: REST Client
    pub const M19: Self = Self("M19");
    /// M20: gRPC Client
    pub const M20: Self = Self("M20");
    /// M21: WebSocket Client
    pub const M21: Self = Self("M21");
    /// M22: IPC Manager
    pub const M22: Self = Self("M22");
    /// M23: Event Bus
    pub const M23: Self = Self("M23");
    /// M24: Bridge Manager
    pub const M24: Self = Self("M24");

    // L5: Learning
    /// M25: Hebbian Manager
    pub const M25: Self = Self("M25");
    /// M26: STDP Processor
    pub const M26: Self = Self("M26");
    /// M27: Pattern Recognizer
    pub const M27: Self = Self("M27");
    /// M28: Pathway Pruner
    pub const M28: Self = Self("M28");
    /// M29: Memory Consolidator
    pub const M29: Self = Self("M29");
    /// M30: Anti-Pattern Detector
    pub const M30: Self = Self("M30");

    // L6: Consensus
    /// M31: PBFT Manager
    pub const M31: Self = Self("M31");
    /// M32: Agent Coordinator
    pub const M32: Self = Self("M32");
    /// M33: Vote Collector
    pub const M33: Self = Self("M33");
    /// M34: View Change Handler
    pub const M34: Self = Self("M34");
    /// M35: Dissent Tracker
    pub const M35: Self = Self("M35");
    /// M36: Quorum Calculator
    pub const M36: Self = Self("M36");

    // L7: Observer
    /// M37: Log Correlator
    pub const M37: Self = Self("M37");
    /// M38: Emergence Detector
    pub const M38: Self = Self("M38");
    /// M39: Evolution Chamber
    pub const M39: Self = Self("M39");

    // HRS-001: Neural Homeostasis
    /// M40: Thermal Controller
    pub const M40: Self = Self("M40");
    /// M41: Decay Auditor
    pub const M41: Self = Self("M41");
    /// M42: Diagnostics Engine
    pub const M42: Self = Self("M42");

    // Infrastructure modules (newly ID'd, Session 068)
    /// M43: NAM Utilities
    pub const M43: Self = Self("M43");
    /// M44: Observer Bus
    pub const M44: Self = Self("M44");
    /// M45: Fitness Evaluator
    pub const M45: Self = Self("M45");
    /// M46: Peer Bridge
    pub const M46: Self = Self("M46");
    /// M47: Tool Registrar
    pub const M47: Self = Self("M47");

    /// All known module IDs in order.
    pub const ALL: [Self; 47] = [
        Self::M01, Self::M02, Self::M03, Self::M04, Self::M05, Self::M06,
        Self::M07, Self::M08, Self::M09, Self::M10, Self::M11, Self::M12,
        Self::M13, Self::M14, Self::M15, Self::M16, Self::M17, Self::M18,
        Self::M19, Self::M20, Self::M21, Self::M22, Self::M23, Self::M24,
        Self::M25, Self::M26, Self::M27, Self::M28, Self::M29, Self::M30,
        Self::M31, Self::M32, Self::M33, Self::M34, Self::M35, Self::M36,
        Self::M37, Self::M38, Self::M39, Self::M40, Self::M41, Self::M42,
        Self::M43, Self::M44, Self::M45, Self::M46, Self::M47,
    ];

    /// Create a `ModuleId` from a static string.
    ///
    /// Prefer the named constants (`ModuleId::M01`, etc.) over this constructor.
    #[must_use]
    pub const fn new(id: &'static str) -> Self {
        Self(id)
    }

    /// Return the raw string identifier (e.g. `"M01"`).
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        self.0
    }

    /// Extract the numeric suffix (e.g. `ModuleId::M01` → `1`).
    ///
    /// Returns `None` if the string does not start with `'M'` followed by digits.
    #[must_use]
    pub fn number(&self) -> Option<u8> {
        let s = self.0.strip_prefix('M')?;
        s.parse::<u8>().ok()
    }

    /// Return the layer (1-based) this module belongs to.
    ///
    /// | Modules | Layer |
    /// |---------|-------|
    /// | M01-M06 | 1 |
    /// | M07-M12 | 2 |
    /// | M13-M18 | 3 |
    /// | M19-M24 | 4 |
    /// | M25-M30 | 5 |
    /// | M31-M36 | 6 |
    /// | M37-M42 | 7 |
    /// | M43     | 1 | (NAM Utilities)
    /// | M44-M45 | 7 | (Observer Bus, Fitness Evaluator)
    /// | M46-M47 | 4 | (Peer Bridge, Tool Registrar)
    ///
    /// Returns `None` if the module number is outside the known range.
    #[must_use]
    pub fn layer(&self) -> Option<u8> {
        let n = self.number()?;
        match n {
            1..=6 | 43 => Some(1),   // L1: M01-M06 + M43 (NAM Utilities)
            7..=12 => Some(2),        // L2: M07-M12
            13..=18 => Some(3),       // L3: M13-M18
            19..=24 | 46 | 47 => Some(4), // L4: M19-M24 + M46 (Peer Bridge) + M47 (Tool Registrar)
            25..=30 => Some(5),       // L5: M25-M30
            31..=36 => Some(6),       // L6: M31-M36
            37..=42 | 44 | 45 => Some(7), // L7: M37-M42 + M44 (Observer Bus) + M45 (Fitness Evaluator)
            _ => None,
        }
    }
}

impl AsRef<str> for ModuleId {
    fn as_ref(&self) -> &str {
        self.0
    }
}

impl fmt::Display for ModuleId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

// ============================================================================
// AgentId
// ============================================================================

/// Typed operational identity for an agent.
///
/// Bridges [`AgentOrigin`](super::AgentOrigin) (enum with structured data) to a
/// flat `String` identifier suitable for maps, logs, and wire formats.
///
/// Prefix convention:
/// - `"sys:"` — system-level automated operations
/// - `"human:"` — human agent (NAM R5)
/// - `"svc:"` — ULTRAPLATE service
/// - `"agent:"` — CVA-NAM fleet agent
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AgentId(String);

impl AgentId {
    /// System-level agent (no specific actor).
    #[must_use]
    pub fn system() -> Self {
        Self("sys:system".to_string())
    }

    /// Human agent (NAM R5 — peer, not supervisor).
    #[must_use]
    pub fn human() -> Self {
        Self("human:@0.A".to_string())
    }

    /// ULTRAPLATE service agent.
    #[must_use]
    pub fn service(service_id: &str) -> Self {
        Self(format!("svc:{service_id}"))
    }

    /// CVA-NAM fleet agent.
    #[must_use]
    pub fn agent(agent_id: &str) -> Self {
        Self(format!("agent:{agent_id}"))
    }

    /// Create from an arbitrary string (for deserialization).
    #[must_use]
    pub fn from_raw(raw: impl Into<String>) -> Self {
        Self(raw.into())
    }

    /// Return the raw string identifier.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Return the prefix (everything before the first `:`).
    #[must_use]
    pub fn prefix(&self) -> &str {
        self.0.split(':').next().unwrap_or(&self.0)
    }

    /// Check whether this is a system agent.
    #[must_use]
    pub fn is_system(&self) -> bool {
        self.0.starts_with("sys:")
    }

    /// Check whether this is a human agent.
    #[must_use]
    pub fn is_human(&self) -> bool {
        self.0.starts_with("human:")
    }

    /// Check whether this is a service agent.
    #[must_use]
    pub fn is_service(&self) -> bool {
        self.0.starts_with("svc:")
    }

    /// Check whether this is a fleet agent.
    #[must_use]
    pub fn is_agent(&self) -> bool {
        self.0.starts_with("agent:")
    }
}

impl AsRef<str> for AgentId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<AgentId> for String {
    fn from(id: AgentId) -> Self {
        id.0
    }
}

impl fmt::Display for AgentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

// ============================================================================
// Timestamp
// ============================================================================

/// Global monotonic counter for ordering events.
static GLOBAL_TICK: AtomicU64 = AtomicU64::new(1);

/// Monotonic cycle-counter timestamp (NOT wall-clock time).
///
/// Every call to [`Timestamp::now()`] returns a strictly increasing value,
/// making it safe for STDP timing windows and causal ordering.
///
/// Per ULTRAPLATE convention: no `chrono`, no `SystemTime` — only cycle counters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Timestamp(u64);

impl Timestamp {
    /// The zero timestamp (epoch).
    pub const ZERO: Self = Self(0);

    /// Acquire the next strictly-increasing tick.
    #[must_use]
    pub fn now() -> Self {
        Self(GLOBAL_TICK.fetch_add(1, Ordering::Relaxed))
    }

    /// Create a timestamp from a raw value (for deserialization / testing).
    #[must_use]
    pub const fn from_raw(ticks: u64) -> Self {
        Self(ticks)
    }

    /// Return the raw tick value.
    #[must_use]
    pub const fn ticks(&self) -> u64 {
        self.0
    }

    /// Compute the number of ticks elapsed since `earlier`.
    ///
    /// Returns `0` if `earlier` is after `self` (saturating).
    #[must_use]
    pub const fn elapsed_since(&self, earlier: Self) -> u64 {
        self.0.saturating_sub(earlier.0)
    }

    /// Check whether `self` is within `window` ticks of `other`.
    #[must_use]
    pub const fn within_window(&self, other: Self, window: u64) -> bool {
        self.0.abs_diff(other.0) <= window
    }
}

impl Default for Timestamp {
    fn default() -> Self {
        Self::ZERO
    }
}

impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "T{}", self.0)
    }
}

// ============================================================================
// HealthReport
// ============================================================================

/// Per-module health snapshot.
///
/// Health score is clamped to `[0.0, 1.0]` at construction.
#[derive(Debug, Clone, PartialEq)]
pub struct HealthReport {
    /// Which module produced this report.
    pub module_id: ModuleId,
    /// Health score in `[0.0, 1.0]`.
    pub health_score: f64,
    /// When this report was generated.
    pub timestamp: Timestamp,
    /// Optional human-readable details.
    pub details: Option<String>,
}

impl HealthReport {
    /// Create a new health report.
    ///
    /// `health_score` is clamped to `[0.0, 1.0]`.
    #[must_use]
    pub fn new(module_id: ModuleId, health_score: f64) -> Self {
        Self {
            module_id,
            health_score: health_score.clamp(0.0, 1.0),
            timestamp: Timestamp::now(),
            details: None,
        }
    }

    /// Attach human-readable details.
    #[must_use]
    pub fn with_details(mut self, details: impl Into<String>) -> Self {
        self.details = Some(details.into());
        self
    }

    /// Override the timestamp (for testing or replay).
    #[must_use]
    pub const fn with_timestamp(mut self, timestamp: Timestamp) -> Self {
        self.timestamp = timestamp;
        self
    }

    /// Whether this module is considered healthy (score >= 0.5).
    #[must_use]
    pub fn is_healthy(&self) -> bool {
        self.health_score >= 0.5
    }

    /// Whether this module is in critical state (score < 0.2).
    #[must_use]
    pub fn is_critical(&self) -> bool {
        self.health_score < 0.2
    }
}

impl fmt::Display for HealthReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Health({}: {:.3} at {})",
            self.module_id, self.health_score, self.timestamp
        )
    }
}

// ============================================================================
// DimensionIndex
// ============================================================================

/// Named index into the 12D tensor.
///
/// Maps human-readable dimension names to their zero-based positions in
/// [`Tensor12D`](crate::Tensor12D).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum DimensionIndex {
    /// D0: Service identifier (normalized hash)
    ServiceId = 0,
    /// D1: Port (port/65535)
    Port = 1,
    /// D2: Tier (tier/6)
    Tier = 2,
    /// D3: Dependency count (log normalized)
    DependencyCount = 3,
    /// D4: Agent count (agents/40)
    AgentCount = 4,
    /// D5: Protocol (enum encoding)
    Protocol = 5,
    /// D6: Health score (0-1)
    HealthScore = 6,
    /// D7: Uptime ratio (0-1)
    Uptime = 7,
    /// D8: Synergy score (0-1)
    Synergy = 8,
    /// D9: Latency (1 - `latency_ms` / 2000)
    Latency = 9,
    /// D10: Error rate (0-1)
    ErrorRate = 10,
    /// D11: Temporal context (time encoding)
    TemporalContext = 11,
}

impl DimensionIndex {
    /// All 12 dimensions in index order.
    pub const ALL: [Self; 12] = [
        Self::ServiceId,
        Self::Port,
        Self::Tier,
        Self::DependencyCount,
        Self::AgentCount,
        Self::Protocol,
        Self::HealthScore,
        Self::Uptime,
        Self::Synergy,
        Self::Latency,
        Self::ErrorRate,
        Self::TemporalContext,
    ];

    /// Return the zero-based index of this dimension.
    #[must_use]
    pub const fn index(self) -> usize {
        self as usize
    }

    /// Return the human-readable name of this dimension.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::ServiceId => "service_id",
            Self::Port => "port",
            Self::Tier => "tier",
            Self::DependencyCount => "dependency_count",
            Self::AgentCount => "agent_count",
            Self::Protocol => "protocol",
            Self::HealthScore => "health_score",
            Self::Uptime => "uptime",
            Self::Synergy => "synergy",
            Self::Latency => "latency",
            Self::ErrorRate => "error_rate",
            Self::TemporalContext => "temporal_context",
        }
    }

    /// Look up a dimension by zero-based index.
    ///
    /// Returns `None` if `index >= 12`.
    #[must_use]
    pub const fn from_index(index: usize) -> Option<Self> {
        match index {
            0 => Some(Self::ServiceId),
            1 => Some(Self::Port),
            2 => Some(Self::Tier),
            3 => Some(Self::DependencyCount),
            4 => Some(Self::AgentCount),
            5 => Some(Self::Protocol),
            6 => Some(Self::HealthScore),
            7 => Some(Self::Uptime),
            8 => Some(Self::Synergy),
            9 => Some(Self::Latency),
            10 => Some(Self::ErrorRate),
            11 => Some(Self::TemporalContext),
            _ => None,
        }
    }

    /// Look up a dimension by name.
    ///
    /// Returns `None` if the name is not recognised.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "service_id" => Some(Self::ServiceId),
            "port" => Some(Self::Port),
            "tier" => Some(Self::Tier),
            "dependency_count" => Some(Self::DependencyCount),
            "agent_count" => Some(Self::AgentCount),
            "protocol" => Some(Self::Protocol),
            "health_score" => Some(Self::HealthScore),
            "uptime" => Some(Self::Uptime),
            "synergy" => Some(Self::Synergy),
            "latency" => Some(Self::Latency),
            "error_rate" => Some(Self::ErrorRate),
            "temporal_context" => Some(Self::TemporalContext),
            _ => None,
        }
    }
}

impl fmt::Display for DimensionIndex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "D{}:{}", self.index(), self.name())
    }
}

// ============================================================================
// CoverageBitmap
// ============================================================================

/// Bitmask tracking which of the 12 tensor dimensions are populated.
///
/// Internally a `u16` with only the bottom 12 bits used.
/// All operations are `const fn` where possible.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct CoverageBitmap(u16);

/// Mask to ensure only the bottom 12 bits are used.
const COVERAGE_MASK: u16 = 0x0FFF;

impl CoverageBitmap {
    /// Empty bitmap (no dimensions covered).
    pub const EMPTY: Self = Self(0);

    /// Full bitmap (all 12 dimensions covered).
    pub const FULL: Self = Self(COVERAGE_MASK);

    /// Create a bitmap from a raw `u16` (masked to 12 bits).
    #[must_use]
    pub const fn from_raw(bits: u16) -> Self {
        Self(bits & COVERAGE_MASK)
    }

    /// Return the raw `u16` value.
    #[must_use]
    pub const fn raw(self) -> u16 {
        self.0
    }

    /// Set a dimension as covered.
    #[must_use]
    pub const fn with_dimension(self, dim: DimensionIndex) -> Self {
        // dim.index() is always 0..11 so the shift is safe within u16
        let bit = 1_u16 << (dim as u8);
        Self((self.0 | bit) & COVERAGE_MASK)
    }

    /// Check whether a dimension is covered.
    #[must_use]
    pub const fn is_covered(self, dim: DimensionIndex) -> bool {
        let bit = 1_u16 << (dim as u8);
        (self.0 & bit) != 0
    }

    /// Count the number of covered dimensions.
    #[must_use]
    pub const fn count(self) -> u32 {
        self.0.count_ones()
    }

    /// Union of two bitmaps (OR).
    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self((self.0 | other.0) & COVERAGE_MASK)
    }

    /// Intersection of two bitmaps (AND).
    #[must_use]
    pub const fn intersection(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }

    /// Coverage ratio (0.0–1.0).
    #[must_use]
    pub fn coverage_ratio(self) -> f64 {
        // count_ones() returns u32; max is 12 which fits in u8
        f64::from(self.count()) / 12.0
    }

    /// Return the list of covered dimension indices.
    #[must_use]
    pub fn covered_dimensions(self) -> Vec<DimensionIndex> {
        DimensionIndex::ALL
            .iter()
            .copied()
            .filter(|dim| self.is_covered(*dim))
            .collect()
    }

    /// Return the list of uncovered dimension indices.
    #[must_use]
    pub fn uncovered_dimensions(self) -> Vec<DimensionIndex> {
        DimensionIndex::ALL
            .iter()
            .copied()
            .filter(|dim| !self.is_covered(*dim))
            .collect()
    }
}

impl fmt::Display for CoverageBitmap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Coverage({}/12 = {:.0}%)", self.count(), self.coverage_ratio() * 100.0)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ==== ModuleId tests ====

    #[test]
    fn test_module_id_constants_exist() {
        assert_eq!(ModuleId::M01.as_str(), "M01");
        assert_eq!(ModuleId::M42.as_str(), "M42");
        assert_eq!(ModuleId::M43.as_str(), "M43");
        assert_eq!(ModuleId::M47.as_str(), "M47");
    }

    #[test]
    fn test_module_id_all_count() {
        assert_eq!(ModuleId::ALL.len(), 47);
    }

    #[test]
    fn test_module_id_all_unique() {
        let mut seen = std::collections::HashSet::new();
        for id in &ModuleId::ALL {
            assert!(seen.insert(id.as_str()), "Duplicate: {id}");
        }
    }

    #[test]
    fn test_module_id_number() {
        assert_eq!(ModuleId::M01.number(), Some(1));
        assert_eq!(ModuleId::M10.number(), Some(10));
        assert_eq!(ModuleId::M42.number(), Some(42));
    }

    #[test]
    fn test_module_id_number_custom() {
        let custom = ModuleId::new("custom");
        assert_eq!(custom.number(), None);
    }

    #[test]
    fn test_module_id_layer() {
        assert_eq!(ModuleId::M01.layer(), Some(1));
        assert_eq!(ModuleId::M06.layer(), Some(1));
        assert_eq!(ModuleId::M07.layer(), Some(2));
        assert_eq!(ModuleId::M12.layer(), Some(2));
        assert_eq!(ModuleId::M13.layer(), Some(3));
        assert_eq!(ModuleId::M18.layer(), Some(3));
        assert_eq!(ModuleId::M19.layer(), Some(4));
        assert_eq!(ModuleId::M25.layer(), Some(5));
        assert_eq!(ModuleId::M31.layer(), Some(6));
        assert_eq!(ModuleId::M37.layer(), Some(7));
        assert_eq!(ModuleId::M42.layer(), Some(7));
        assert_eq!(ModuleId::M43.layer(), Some(1)); // NAM Utilities → L1
        assert_eq!(ModuleId::M44.layer(), Some(7)); // Observer Bus → L7
        assert_eq!(ModuleId::M45.layer(), Some(7)); // Fitness Evaluator → L7
        assert_eq!(ModuleId::M46.layer(), Some(4)); // Peer Bridge → L4
        assert_eq!(ModuleId::M47.layer(), Some(4)); // Tool Registrar → L4
    }

    #[test]
    fn test_module_id_display() {
        assert_eq!(ModuleId::M04.to_string(), "M04");
    }

    #[test]
    fn test_module_id_as_ref_str() {
        let id = ModuleId::M01;
        let s: &str = id.as_ref();
        assert_eq!(s, "M01");
    }

    #[test]
    fn test_module_id_copy() {
        let a = ModuleId::M01;
        let b = a;
        assert_eq!(a, b);
    }

    #[test]
    fn test_module_id_ordering() {
        assert!(ModuleId::M01 < ModuleId::M02);
        assert!(ModuleId::M09 < ModuleId::M10);
    }

    #[test]
    fn test_module_id_hash_set() {
        let mut set = std::collections::HashSet::new();
        set.insert(ModuleId::M01);
        set.insert(ModuleId::M01);
        set.insert(ModuleId::M02);
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn test_module_id_all_layers_covered() {
        for id in &ModuleId::ALL {
            assert!(id.layer().is_some(), "{id} has no layer");
            assert!(id.number().is_some(), "{id} has no number");
        }
    }

    #[test]
    fn test_module_id_layer_boundaries() {
        // First and last of each layer
        assert_eq!(ModuleId::M01.layer(), ModuleId::M06.layer());
        assert_ne!(ModuleId::M06.layer(), ModuleId::M07.layer());
        assert_eq!(ModuleId::M07.layer(), ModuleId::M12.layer());
    }

    // ==== AgentId tests ====

    #[test]
    fn test_agent_id_system() {
        let id = AgentId::system();
        assert_eq!(id.as_str(), "sys:system");
        assert!(id.is_system());
        assert!(!id.is_human());
    }

    #[test]
    fn test_agent_id_human() {
        let id = AgentId::human();
        assert_eq!(id.as_str(), "human:@0.A");
        assert!(id.is_human());
        assert!(!id.is_system());
    }

    #[test]
    fn test_agent_id_service() {
        let id = AgentId::service("synthex");
        assert_eq!(id.as_str(), "svc:synthex");
        assert!(id.is_service());
    }

    #[test]
    fn test_agent_id_agent() {
        let id = AgentId::agent("a-001");
        assert_eq!(id.as_str(), "agent:a-001");
        assert!(id.is_agent());
    }

    #[test]
    fn test_agent_id_prefix() {
        assert_eq!(AgentId::system().prefix(), "sys");
        assert_eq!(AgentId::human().prefix(), "human");
        assert_eq!(AgentId::service("x").prefix(), "svc");
        assert_eq!(AgentId::agent("y").prefix(), "agent");
    }

    #[test]
    fn test_agent_id_as_ref_str() {
        let id = AgentId::service("san-k7");
        let s: &str = id.as_ref();
        assert_eq!(s, "svc:san-k7");
    }

    #[test]
    fn test_agent_id_into_string() {
        let id = AgentId::human();
        let s: String = id.into();
        assert_eq!(s, "human:@0.A");
    }

    #[test]
    fn test_agent_id_display() {
        assert_eq!(AgentId::system().to_string(), "sys:system");
    }

    #[test]
    fn test_agent_id_from_raw() {
        let id = AgentId::from_raw("custom:test");
        assert_eq!(id.as_str(), "custom:test");
        assert!(!id.is_system());
        assert!(!id.is_human());
    }

    #[test]
    fn test_agent_id_equality() {
        assert_eq!(AgentId::system(), AgentId::system());
        assert_ne!(AgentId::system(), AgentId::human());
    }

    #[test]
    fn test_agent_id_ordering() {
        let a = AgentId::agent("a");
        let b = AgentId::agent("b");
        assert!(a < b);
    }

    #[test]
    fn test_agent_id_hash_set() {
        let mut set = std::collections::HashSet::new();
        set.insert(AgentId::system());
        set.insert(AgentId::system());
        set.insert(AgentId::human());
        assert_eq!(set.len(), 2);
    }

    // ==== Timestamp tests ====

    #[test]
    fn test_timestamp_zero() {
        assert_eq!(Timestamp::ZERO.ticks(), 0);
    }

    #[test]
    fn test_timestamp_now_monotonic() {
        let a = Timestamp::now();
        let b = Timestamp::now();
        let c = Timestamp::now();
        assert!(a < b);
        assert!(b < c);
    }

    #[test]
    fn test_timestamp_from_raw() {
        let ts = Timestamp::from_raw(42);
        assert_eq!(ts.ticks(), 42);
    }

    #[test]
    fn test_timestamp_elapsed_since() {
        let a = Timestamp::from_raw(10);
        let b = Timestamp::from_raw(15);
        assert_eq!(b.elapsed_since(a), 5);
    }

    #[test]
    fn test_timestamp_elapsed_since_saturating() {
        let a = Timestamp::from_raw(20);
        let b = Timestamp::from_raw(10);
        assert_eq!(b.elapsed_since(a), 0);
    }

    #[test]
    fn test_timestamp_within_window() {
        let a = Timestamp::from_raw(100);
        let b = Timestamp::from_raw(105);
        assert!(a.within_window(b, 10));
        assert!(!a.within_window(b, 3));
    }

    #[test]
    fn test_timestamp_within_window_symmetric() {
        let a = Timestamp::from_raw(100);
        let b = Timestamp::from_raw(105);
        assert_eq!(a.within_window(b, 5), b.within_window(a, 5));
    }

    #[test]
    fn test_timestamp_display() {
        let ts = Timestamp::from_raw(999);
        assert_eq!(ts.to_string(), "T999");
    }

    #[test]
    fn test_timestamp_default_is_zero() {
        assert_eq!(Timestamp::default(), Timestamp::ZERO);
    }

    #[test]
    fn test_timestamp_copy() {
        let a = Timestamp::from_raw(7);
        let b = a;
        assert_eq!(a, b);
    }

    #[test]
    fn test_timestamp_ordering() {
        let a = Timestamp::from_raw(1);
        let b = Timestamp::from_raw(2);
        assert!(a < b);
        assert!(b > a);
    }

    #[test]
    fn test_timestamp_hash_set() {
        let mut set = std::collections::HashSet::new();
        set.insert(Timestamp::from_raw(1));
        set.insert(Timestamp::from_raw(1));
        set.insert(Timestamp::from_raw(2));
        assert_eq!(set.len(), 2);
    }

    // ==== HealthReport tests ====

    #[test]
    fn test_health_report_new() {
        let report = HealthReport::new(ModuleId::M01, 0.85);
        assert_eq!(report.module_id, ModuleId::M01);
        assert!((report.health_score - 0.85).abs() < f64::EPSILON);
        assert!(report.details.is_none());
    }

    #[test]
    fn test_health_report_clamping() {
        let report = HealthReport::new(ModuleId::M02, 1.5);
        assert!((report.health_score - 1.0).abs() < f64::EPSILON);
        let report2 = HealthReport::new(ModuleId::M03, -0.5);
        assert!(report2.health_score.abs() < f64::EPSILON);
    }

    #[test]
    fn test_health_report_with_details() {
        let report = HealthReport::new(ModuleId::M04, 0.9)
            .with_details("all subsystems nominal");
        assert_eq!(report.details.as_deref(), Some("all subsystems nominal"));
    }

    #[test]
    fn test_health_report_with_timestamp() {
        let ts = Timestamp::from_raw(42);
        let report = HealthReport::new(ModuleId::M05, 1.0).with_timestamp(ts);
        assert_eq!(report.timestamp, ts);
    }

    #[test]
    fn test_health_report_is_healthy() {
        assert!(HealthReport::new(ModuleId::M01, 0.5).is_healthy());
        assert!(HealthReport::new(ModuleId::M01, 1.0).is_healthy());
        assert!(!HealthReport::new(ModuleId::M01, 0.49).is_healthy());
    }

    #[test]
    fn test_health_report_is_critical() {
        assert!(HealthReport::new(ModuleId::M01, 0.1).is_critical());
        assert!(HealthReport::new(ModuleId::M01, 0.0).is_critical());
        assert!(!HealthReport::new(ModuleId::M01, 0.2).is_critical());
    }

    #[test]
    fn test_health_report_display() {
        let report = HealthReport::new(ModuleId::M01, 0.75)
            .with_timestamp(Timestamp::from_raw(100));
        let display = report.to_string();
        assert!(display.contains("M01"));
        assert!(display.contains("0.750"));
        assert!(display.contains("T100"));
    }

    #[test]
    fn test_health_report_clone_eq() {
        let a = HealthReport::new(ModuleId::M01, 0.5)
            .with_timestamp(Timestamp::from_raw(1));
        let b = a.clone();
        assert_eq!(a, b);
    }

    // ==== DimensionIndex tests ====

    #[test]
    fn test_dimension_all_count() {
        assert_eq!(DimensionIndex::ALL.len(), 12);
    }

    #[test]
    fn test_dimension_indices_sequential() {
        for (i, dim) in DimensionIndex::ALL.iter().enumerate() {
            assert_eq!(dim.index(), i);
        }
    }

    #[test]
    fn test_dimension_from_index_roundtrip() {
        for dim in &DimensionIndex::ALL {
            let recovered = DimensionIndex::from_index(dim.index());
            assert_eq!(recovered, Some(*dim));
        }
    }

    #[test]
    fn test_dimension_from_index_out_of_range() {
        assert_eq!(DimensionIndex::from_index(12), None);
        assert_eq!(DimensionIndex::from_index(255), None);
    }

    #[test]
    fn test_dimension_from_name_roundtrip() {
        for dim in &DimensionIndex::ALL {
            let recovered = DimensionIndex::from_name(dim.name());
            assert_eq!(recovered, Some(*dim));
        }
    }

    #[test]
    fn test_dimension_from_name_unknown() {
        assert_eq!(DimensionIndex::from_name("unknown"), None);
    }

    #[test]
    fn test_dimension_names_unique() {
        let mut names = std::collections::HashSet::new();
        for dim in &DimensionIndex::ALL {
            assert!(names.insert(dim.name()), "Duplicate name: {}", dim.name());
        }
    }

    #[test]
    fn test_dimension_specific_values() {
        assert_eq!(DimensionIndex::ServiceId.index(), 0);
        assert_eq!(DimensionIndex::HealthScore.index(), 6);
        assert_eq!(DimensionIndex::TemporalContext.index(), 11);
    }

    #[test]
    fn test_dimension_display() {
        assert_eq!(DimensionIndex::ServiceId.to_string(), "D0:service_id");
        assert_eq!(DimensionIndex::HealthScore.to_string(), "D6:health_score");
    }

    #[test]
    fn test_dimension_equality() {
        assert_eq!(DimensionIndex::Port, DimensionIndex::Port);
        assert_ne!(DimensionIndex::Port, DimensionIndex::Tier);
    }

    // ==== CoverageBitmap tests ====

    #[test]
    fn test_coverage_empty() {
        let c = CoverageBitmap::EMPTY;
        assert_eq!(c.count(), 0);
        assert!(!c.is_covered(DimensionIndex::ServiceId));
        assert!(c.coverage_ratio().abs() < f64::EPSILON);
    }

    #[test]
    fn test_coverage_full() {
        let c = CoverageBitmap::FULL;
        assert_eq!(c.count(), 12);
        for dim in &DimensionIndex::ALL {
            assert!(c.is_covered(*dim));
        }
        assert!((c.coverage_ratio() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_coverage_with_dimension() {
        let c = CoverageBitmap::EMPTY
            .with_dimension(DimensionIndex::HealthScore)
            .with_dimension(DimensionIndex::Uptime);
        assert_eq!(c.count(), 2);
        assert!(c.is_covered(DimensionIndex::HealthScore));
        assert!(c.is_covered(DimensionIndex::Uptime));
        assert!(!c.is_covered(DimensionIndex::Port));
    }

    #[test]
    fn test_coverage_idempotent() {
        let c = CoverageBitmap::EMPTY
            .with_dimension(DimensionIndex::Port)
            .with_dimension(DimensionIndex::Port);
        assert_eq!(c.count(), 1);
    }

    #[test]
    fn test_coverage_union() {
        let a = CoverageBitmap::EMPTY.with_dimension(DimensionIndex::Port);
        let b = CoverageBitmap::EMPTY.with_dimension(DimensionIndex::Tier);
        let c = a.union(b);
        assert_eq!(c.count(), 2);
        assert!(c.is_covered(DimensionIndex::Port));
        assert!(c.is_covered(DimensionIndex::Tier));
    }

    #[test]
    fn test_coverage_intersection() {
        let a = CoverageBitmap::EMPTY
            .with_dimension(DimensionIndex::Port)
            .with_dimension(DimensionIndex::Tier);
        let b = CoverageBitmap::EMPTY
            .with_dimension(DimensionIndex::Tier)
            .with_dimension(DimensionIndex::Synergy);
        let c = a.intersection(b);
        assert_eq!(c.count(), 1);
        assert!(c.is_covered(DimensionIndex::Tier));
    }

    #[test]
    fn test_coverage_covered_dimensions() {
        let c = CoverageBitmap::EMPTY
            .with_dimension(DimensionIndex::ServiceId)
            .with_dimension(DimensionIndex::TemporalContext);
        let dims = c.covered_dimensions();
        assert_eq!(dims.len(), 2);
        assert_eq!(dims[0], DimensionIndex::ServiceId);
        assert_eq!(dims[1], DimensionIndex::TemporalContext);
    }

    #[test]
    fn test_coverage_uncovered_dimensions() {
        let c = CoverageBitmap::FULL;
        assert!(c.uncovered_dimensions().is_empty());
        let c2 = CoverageBitmap::EMPTY;
        assert_eq!(c2.uncovered_dimensions().len(), 12);
    }

    #[test]
    fn test_coverage_ratio() {
        let c = CoverageBitmap::EMPTY
            .with_dimension(DimensionIndex::Port)
            .with_dimension(DimensionIndex::Tier)
            .with_dimension(DimensionIndex::Synergy);
        assert!((c.coverage_ratio() - 0.25).abs() < f64::EPSILON);
    }

    #[test]
    fn test_coverage_display() {
        let c = CoverageBitmap::EMPTY
            .with_dimension(DimensionIndex::Port);
        let display = c.to_string();
        assert!(display.contains("1/12"));
    }

    #[test]
    fn test_coverage_from_raw_masks() {
        let c = CoverageBitmap::from_raw(0xFFFF);
        assert_eq!(c.count(), 12); // Only bottom 12 bits
        assert_eq!(c, CoverageBitmap::FULL);
    }

    #[test]
    fn test_coverage_default_is_empty() {
        assert_eq!(CoverageBitmap::default(), CoverageBitmap::EMPTY);
    }
}
