//! # M48: Self Model
//!
//! Introspective self-model providing architecture descriptors, capability tracking,
//! layer health monitoring, and NAM compliance scoring for the Maintenance Engine.
//!
//! ## Layer: L1 (Foundation)
//! ## Dependencies: M00, M01, M04, M07, M08
//!
//! ## Design
//!
//! The self-model maintains a live representation of the engine's own architecture,
//! capabilities, and health state.  All trait methods take `&self` with interior
//! mutability via [`parking_lot::RwLock`], following the gold-standard L1 pattern.
//!
//! ## Key Invariants
//!
//! - All health scores are clamped to `[0.0, 1.0]`
//! - Layer indices are 0-based and must be `<= 7` (8 layers total)
//! - Capability names must be non-empty
//! - Timestamps use [`Timestamp::now()`] exclusively (C5: no chrono, no `SystemTime`)
//! - Owned returns through `RwLock` (C7: never return references)
//!
//! ## Related Documentation
//! - [Module Specification](../../ai_docs/modules/M48_SELF_MODEL.md)

use std::collections::HashMap;
use std::fmt;

use parking_lot::RwLock;

use crate::{Error, Result};

use super::shared_types::Timestamp;

// ============================================================================
// Constants
// ============================================================================

/// Default number of modules in the Maintenance Engine.
const DEFAULT_MODULE_COUNT: u8 = 48;

/// Default number of layers in the Maintenance Engine.
const DEFAULT_LAYER_COUNT: u8 = 8;

/// Default NAM compliance target.
const DEFAULT_NAM_TARGET: f64 = 0.95;

/// Default capability capacity.
const DEFAULT_CAPABILITY_CAPACITY: usize = 64;

/// Engine version string.
const ENGINE_VERSION: &str = "2.0.0";

/// Module identifier for health reports.
const SELF_MODEL_MODULE_ID: &str = "M48";

/// Layer names indexed by layer number (0-based).
const LAYER_NAMES: [&str; 8] = [
    "Foundation",
    "Services",
    "Core Logic",
    "Integration",
    "Learning",
    "Consensus",
    "Observer",
    "Nexus",
];

// ============================================================================
// CapabilityStatus
// ============================================================================

/// Status of a capability within the engine.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum CapabilityStatus {
    /// Capability is fully operational.
    Ready,
    /// Capability is partially functional or experiencing issues.
    Degraded,
    /// Capability is not available.
    Unavailable,
    /// Capability status has not been determined.
    #[default]
    Unknown,
}

impl fmt::Display for CapabilityStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ready => write!(f, "READY"),
            Self::Degraded => write!(f, "DEGRADED"),
            Self::Unavailable => write!(f, "UNAVAILABLE"),
            Self::Unknown => write!(f, "UNKNOWN"),
        }
    }
}

// ============================================================================
// CapabilityEntry
// ============================================================================

/// Describes a single capability tracked by the self-model.
#[derive(Debug, Clone, PartialEq)]
pub struct CapabilityEntry {
    /// Human-readable capability name (must be non-empty).
    pub name: String,
    /// Layer this capability belongs to (0-based, max 7).
    pub layer: u8,
    /// Current operational status.
    pub status: CapabilityStatus,
    /// Health score for this capability (0.0--1.0).
    pub health_score: f64,
    /// When this entry was last updated.
    pub last_updated: Timestamp,
}

impl CapabilityEntry {
    /// Create a new capability entry.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] if `name` is empty, `layer > 7`,
    /// or `health_score` is outside `[0.0, 1.0]`.
    pub fn new(
        name: impl Into<String>,
        layer: u8,
        status: CapabilityStatus,
        health_score: f64,
    ) -> Result<Self> {
        let name = name.into();
        validate_capability_name(&name)?;
        validate_layer(layer)?;
        validate_score(health_score, "health_score")?;
        Ok(Self {
            name,
            layer,
            status,
            health_score,
            last_updated: Timestamp::now(),
        })
    }
}

// ============================================================================
// ArchitectureDescriptor
// ============================================================================

/// Describes the high-level architecture of the Maintenance Engine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchitectureDescriptor {
    /// Total number of layers (default 8).
    pub layer_count: u8,
    /// Total number of modules (default 48).
    pub module_count: u8,
    /// NAM compliance requirements satisfied by this architecture.
    pub nam_requirements: Vec<String>,
    /// Engine version string.
    pub version: &'static str,
}

impl Default for ArchitectureDescriptor {
    fn default() -> Self {
        Self {
            layer_count: DEFAULT_LAYER_COUNT,
            module_count: DEFAULT_MODULE_COUNT,
            nam_requirements: vec![
                "R1:SelfQuery".to_string(),
                "R2:HebbianRouting".to_string(),
                "R3:DissentCapture".to_string(),
                "R4:FieldVisualization".to_string(),
                "R5:HumanAsAgent".to_string(),
            ],
            version: ENGINE_VERSION,
        }
    }
}

impl fmt::Display for ArchitectureDescriptor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ME v{} ({} layers, {} modules, {} NAM reqs)",
            self.version,
            self.layer_count,
            self.module_count,
            self.nam_requirements.len(),
        )
    }
}

// ============================================================================
// LayerStatusEntry
// ============================================================================

/// Health status for a single layer.
#[derive(Debug, Clone, PartialEq)]
pub struct LayerStatusEntry {
    /// Layer index (0-based).
    pub layer: u8,
    /// Layer name (e.g. "Foundation").
    pub name: &'static str,
    /// Health score for this layer (0.0--1.0).
    pub health_score: f64,
    /// Number of modules in this layer.
    pub module_count: u8,
    /// When this status was last updated.
    pub timestamp: Timestamp,
}

impl fmt::Display for LayerStatusEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "L{} {}: {:.3} ({} modules)",
            self.layer, self.name, self.health_score, self.module_count,
        )
    }
}

// ============================================================================
// RuntimeSnapshot
// ============================================================================

/// Point-in-time snapshot of the engine's runtime state.
#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeSnapshot {
    /// When this snapshot was taken.
    pub timestamp: Timestamp,
    /// Per-layer health scores (indexed by layer, 0--7).
    pub layer_health: [f64; 8],
    /// Number of modules currently active.
    pub active_module_count: u8,
    /// Computed NAM compliance score (0.0--1.0).
    pub nam_compliance_score: f64,
    /// Overall engine health (weighted average of layer health).
    pub overall_health: f64,
}

impl fmt::Display for RuntimeSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Snapshot at {}: health={:.3}, NAM={:.3}, modules={}",
            self.timestamp, self.overall_health, self.nam_compliance_score, self.active_module_count,
        )
    }
}

// ============================================================================
// SelfModelHealth
// ============================================================================

/// Comprehensive health report from the self-model.
#[derive(Debug, Clone, PartialEq)]
pub struct SelfModelHealth {
    /// Module ID producing this report.
    pub module_id: &'static str,
    /// Overall health score (0.0--1.0).
    pub overall_score: f64,
    /// Per-layer health entries.
    pub layer_entries: Vec<LayerStatusEntry>,
    /// Summary of capability statuses.
    pub capability_summary: CapabilitySummary,
}

impl fmt::Display for SelfModelHealth {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}: health={:.3}, layers={}, capabilities={}",
            self.module_id,
            self.overall_score,
            self.layer_entries.len(),
            self.capability_summary.total,
        )
    }
}

// ============================================================================
// CapabilitySummary
// ============================================================================

/// Aggregated summary of all tracked capabilities.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilitySummary {
    /// Total number of tracked capabilities.
    pub total: usize,
    /// Number in [`CapabilityStatus::Ready`] state.
    pub ready: usize,
    /// Number in [`CapabilityStatus::Degraded`] state.
    pub degraded: usize,
    /// Number in [`CapabilityStatus::Unavailable`] state.
    pub unavailable: usize,
    /// Number in [`CapabilityStatus::Unknown`] state.
    pub unknown: usize,
}

// ============================================================================
// SelfModelConfig
// ============================================================================

/// Configuration for the self-model.
#[derive(Debug, Clone, PartialEq)]
pub struct SelfModelConfig {
    /// Total number of modules (default 48).
    pub module_count: u8,
    /// Total number of layers (default 8).
    pub layer_count: u8,
    /// NAM compliance target (default 0.95).
    pub nam_target: f64,
    /// Maximum number of capabilities to track (default 64).
    pub capability_capacity: usize,
}

impl Default for SelfModelConfig {
    fn default() -> Self {
        Self {
            module_count: DEFAULT_MODULE_COUNT,
            layer_count: DEFAULT_LAYER_COUNT,
            nam_target: DEFAULT_NAM_TARGET,
            capability_capacity: DEFAULT_CAPABILITY_CAPACITY,
        }
    }
}

/// Builder for [`SelfModelConfig`].
#[derive(Debug, Clone)]
pub struct SelfModelConfigBuilder {
    /// Inner config being built.
    config: SelfModelConfig,
}

impl SelfModelConfigBuilder {
    /// Start building with defaults.
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: SelfModelConfig::default(),
        }
    }

    /// Set the module count.
    #[must_use]
    pub const fn module_count(mut self, count: u8) -> Self {
        self.config.module_count = count;
        self
    }

    /// Set the layer count.
    #[must_use]
    pub const fn layer_count(mut self, count: u8) -> Self {
        self.config.layer_count = count;
        self
    }

    /// Set the NAM compliance target.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] if `target` is outside `[0.0, 1.0]`.
    pub fn nam_target(mut self, target: f64) -> Result<Self> {
        validate_score(target, "nam_target")?;
        self.config.nam_target = target;
        Ok(self)
    }

    /// Set the capability capacity.
    #[must_use]
    pub const fn capability_capacity(mut self, capacity: usize) -> Self {
        self.config.capability_capacity = capacity;
        self
    }

    /// Consume the builder and produce the config.
    #[must_use]
    pub const fn build(self) -> SelfModelConfig {
        self.config
    }
}

impl Default for SelfModelConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// SelfModelProvider trait
// ============================================================================

/// Trait for querying the engine's self-model.
///
/// Provides read and update access to the engine's introspective view of
/// its own architecture, capabilities, layer health, and NAM compliance.
///
/// All methods take `&self` with interior mutability via [`RwLock`] per C2.
pub trait SelfModelProvider: Send + Sync + fmt::Debug {
    /// Return the architecture descriptor for this engine.
    fn architecture(&self) -> ArchitectureDescriptor;

    /// Return all tracked capabilities.
    fn capabilities(&self) -> Vec<CapabilityEntry>;

    /// Capture a point-in-time runtime snapshot.
    fn runtime_snapshot(&self) -> RuntimeSnapshot;

    /// Query the status of a specific capability by name.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] if `name` is empty.
    /// Returns [`Error::ServiceNotFound`] if the capability does not exist.
    fn capability_status(&self, name: &str) -> Result<CapabilityStatus>;

    /// Query the health status of a specific layer.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] if `layer > 7`.
    fn layer_status(&self, layer: u8) -> Result<LayerStatusEntry>;

    /// Compute the current NAM compliance score (0.0--1.0).
    fn nam_compliance(&self) -> f64;

    /// Return the total number of modules.
    fn module_count(&self) -> u8;

    /// Return the number of layers with health score above zero.
    fn active_layer_count(&self) -> u8;

    /// Generate a comprehensive health report.
    fn health_report(&self) -> SelfModelHealth;

    /// Insert or update a capability entry.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] if the entry has an empty name,
    /// invalid layer, or out-of-range health score.
    fn update_capability(&self, entry: CapabilityEntry) -> Result<()>;

    /// Update the health score for a specific layer.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] if `layer > 7` or `score` is outside `[0.0, 1.0]`.
    fn update_layer_health(&self, layer: u8, score: f64) -> Result<()>;
}

// ============================================================================
// SelfModel struct
// ============================================================================

/// Concrete implementation of [`SelfModelProvider`].
///
/// Maintains an introspective model of the engine's architecture, capability
/// registry, and per-layer health scores using interior mutability.
#[derive(Debug)]
pub struct SelfModel {
    /// Configuration for this self-model instance.
    config: SelfModelConfig,
    /// Static architecture descriptor.
    architecture: ArchitectureDescriptor,
    /// Mutable capability registry keyed by capability name.
    capabilities: RwLock<HashMap<String, CapabilityEntry>>,
    /// Mutable per-layer health scores (0-based index).
    layer_health: RwLock<[f64; 8]>,
}

impl SelfModel {
    /// Create a new self-model with the given configuration.
    #[must_use]
    pub fn new(config: SelfModelConfig) -> Self {
        let architecture = ArchitectureDescriptor {
            layer_count: config.layer_count,
            module_count: config.module_count,
            ..ArchitectureDescriptor::default()
        };
        Self {
            config,
            architecture,
            capabilities: RwLock::new(HashMap::new()),
            layer_health: RwLock::new([1.0; 8]),
        }
    }

    /// Create a new self-model with default configuration.
    #[must_use]
    pub fn with_defaults() -> Self {
        Self::new(SelfModelConfig::default())
    }

    /// Return a reference to the configuration.
    #[must_use]
    pub const fn config(&self) -> &SelfModelConfig {
        &self.config
    }

    /// Return the number of currently tracked capabilities.
    #[must_use]
    pub fn capability_count(&self) -> usize {
        self.capabilities.read().len()
    }

    /// Compute the overall health as a weighted average of layer health scores.
    ///
    /// Uses equal weighting across all layers.
    fn compute_overall_health(layer_health: &[f64; 8]) -> f64 {
        let sum: f64 = layer_health.iter().sum();
        sum / 8.0
    }

    /// Build a capability summary from the current state.
    fn build_capability_summary(capabilities: &HashMap<String, CapabilityEntry>) -> CapabilitySummary {
        let mut ready = 0usize;
        let mut degraded = 0usize;
        let mut unavailable = 0usize;
        let mut unknown = 0usize;

        for entry in capabilities.values() {
            match entry.status {
                CapabilityStatus::Ready => ready += 1,
                CapabilityStatus::Degraded => degraded += 1,
                CapabilityStatus::Unavailable => unavailable += 1,
                CapabilityStatus::Unknown => unknown += 1,
            }
        }

        CapabilitySummary {
            total: capabilities.len(),
            ready,
            degraded,
            unavailable,
            unknown,
        }
    }
}

impl SelfModelProvider for SelfModel {
    fn architecture(&self) -> ArchitectureDescriptor {
        self.architecture.clone()
    }

    fn capabilities(&self) -> Vec<CapabilityEntry> {
        self.capabilities.read().values().cloned().collect()
    }

    fn runtime_snapshot(&self) -> RuntimeSnapshot {
        let layer_health = *self.layer_health.read();
        let overall_health = Self::compute_overall_health(&layer_health);
        let nam = self.nam_compliance();

        RuntimeSnapshot {
            timestamp: Timestamp::now(),
            layer_health,
            active_module_count: self.config.module_count,
            nam_compliance_score: nam,
            overall_health,
        }
    }

    fn capability_status(&self, name: &str) -> Result<CapabilityStatus> {
        if name.is_empty() {
            return Err(Error::Validation(
                "capability name must not be empty".to_string(),
            ));
        }
        let guard = self.capabilities.read();
        guard
            .get(name)
            .map(|e| e.status)
            .ok_or_else(|| Error::ServiceNotFound(format!("capability not found: {name}")))
    }

    fn layer_status(&self, layer: u8) -> Result<LayerStatusEntry> {
        validate_layer(layer)?;
        let health_score = self.layer_health.read()[usize::from(layer)];
        let name = LAYER_NAMES[usize::from(layer)];
        let module_count = modules_per_layer(layer);

        Ok(LayerStatusEntry {
            layer,
            name,
            health_score,
            module_count,
            timestamp: Timestamp::now(),
        })
    }

    fn nam_compliance(&self) -> f64 {
        // NAM compliance is derived from layer health and capability readiness.
        // Layers contribute 70%, capability readiness contributes 30%.
        let layer_health = *self.layer_health.read();
        let overall_layer = Self::compute_overall_health(&layer_health);

        let capability_readiness = {
            let capabilities = self.capabilities.read();
            if capabilities.is_empty() {
                1.0
            } else {
                let ready_count = capabilities
                    .values()
                    .filter(|e| e.status == CapabilityStatus::Ready)
                    .count();
                #[allow(clippy::cast_precision_loss)]
                let ratio = ready_count as f64 / capabilities.len() as f64;
                ratio
            }
        };

        0.7f64.mul_add(overall_layer, 0.3 * capability_readiness)
    }

    fn module_count(&self) -> u8 {
        self.config.module_count
    }

    fn active_layer_count(&self) -> u8 {
        let count = self.layer_health.read().iter().filter(|&&h| h > 0.0).count();
        // Safe: count is at most 8, fits in u8.
        #[allow(clippy::cast_possible_truncation)]
        let result = count as u8;
        result
    }

    fn health_report(&self) -> SelfModelHealth {
        let layer_health = *self.layer_health.read();
        let overall = Self::compute_overall_health(&layer_health);

        let mut layer_entries = Vec::with_capacity(8);
        for i in 0..8u8 {
            let name = LAYER_NAMES[usize::from(i)];
            layer_entries.push(LayerStatusEntry {
                layer: i,
                name,
                health_score: layer_health[usize::from(i)],
                module_count: modules_per_layer(i),
                timestamp: Timestamp::now(),
            });
        }

        let capability_summary = Self::build_capability_summary(&self.capabilities.read());

        SelfModelHealth {
            module_id: SELF_MODEL_MODULE_ID,
            overall_score: overall,
            layer_entries,
            capability_summary,
        }
    }

    fn update_capability(&self, entry: CapabilityEntry) -> Result<()> {
        validate_capability_name(&entry.name)?;
        validate_layer(entry.layer)?;
        validate_score(entry.health_score, "health_score")?;

        self.capabilities.write().insert(entry.name.clone(), entry);
        Ok(())
    }

    fn update_layer_health(&self, layer: u8, score: f64) -> Result<()> {
        validate_layer(layer)?;
        validate_score(score, "layer health score")?;

        self.layer_health.write()[usize::from(layer)] = score;
        Ok(())
    }
}

// ============================================================================
// Validation helpers
// ============================================================================

/// Validate that a capability name is non-empty.
fn validate_capability_name(name: &str) -> Result<()> {
    if name.is_empty() {
        return Err(Error::Validation(
            "capability name must not be empty".to_string(),
        ));
    }
    Ok(())
}

/// Validate that a layer index is within bounds (0--7).
fn validate_layer(layer: u8) -> Result<()> {
    if layer > 7 {
        return Err(Error::Validation(format!(
            "layer index {layer} out of range (max 7)"
        )));
    }
    Ok(())
}

/// Validate that a score is within `[0.0, 1.0]`.
fn validate_score(score: f64, field_name: &str) -> Result<()> {
    if !(0.0..=1.0).contains(&score) {
        return Err(Error::Validation(format!(
            "{field_name} must be in [0.0, 1.0], got {score}"
        )));
    }
    Ok(())
}

/// Return the approximate module count for a given layer.
///
/// Based on the standard distribution across the 8-layer architecture.
const fn modules_per_layer(layer: u8) -> u8 {
    match layer {
        0 => 10, // L1: M00-M08 + M43 + M48
        1 => 4,  // L2: M09-M12
        3 => 8,  // L4: M19-M24 + M46-M47
        2 | 4 | 5 | 6 | 7 => 6, // L3, L5, L6, L7, L8: 6 modules each
        _ => 0,
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // Helper
    // ========================================================================

    fn default_model() -> SelfModel {
        SelfModel::with_defaults()
    }

    fn sample_capability(name: &str, layer: u8, status: CapabilityStatus) -> CapabilityEntry {
        CapabilityEntry::new(name, layer, status, 0.9).expect("valid test capability")
    }

    // ========================================================================
    // Group 1: Constructor and Defaults (6 tests)
    // ========================================================================

    #[test]
    fn test_default_config() {
        let config = SelfModelConfig::default();
        assert_eq!(config.module_count, 48);
        assert_eq!(config.layer_count, 8);
        assert!((config.nam_target - 0.95).abs() < f64::EPSILON);
        assert_eq!(config.capability_capacity, 64);
    }

    #[test]
    fn test_new_with_defaults() {
        let model = default_model();
        assert_eq!(model.config().module_count, 48);
        assert_eq!(model.config().layer_count, 8);
        assert_eq!(model.capability_count(), 0);
    }

    #[test]
    fn test_new_with_custom_config() {
        let config = SelfModelConfig {
            module_count: 32,
            layer_count: 6,
            nam_target: 0.90,
            capability_capacity: 128,
        };
        let model = SelfModel::new(config);
        assert_eq!(model.config().module_count, 32);
        assert_eq!(model.config().layer_count, 6);
    }

    #[test]
    fn test_builder_defaults() {
        let config = SelfModelConfigBuilder::new().build();
        assert_eq!(config.module_count, 48);
        assert_eq!(config.layer_count, 8);
    }

    #[test]
    fn test_builder_custom() {
        let config = SelfModelConfigBuilder::new()
            .module_count(24)
            .layer_count(4)
            .capability_capacity(32)
            .build();
        assert_eq!(config.module_count, 24);
        assert_eq!(config.layer_count, 4);
        assert_eq!(config.capability_capacity, 32);
    }

    #[test]
    fn test_builder_nam_target_valid() {
        let result = SelfModelConfigBuilder::new().nam_target(0.85);
        assert!(result.is_ok());
        let config = result.expect("valid").build();
        assert!((config.nam_target - 0.85).abs() < f64::EPSILON);
    }

    // ========================================================================
    // Group 2: Builder Validation (3 tests)
    // ========================================================================

    #[test]
    fn test_builder_nam_target_too_high() {
        let result = SelfModelConfigBuilder::new().nam_target(1.5);
        assert!(result.is_err());
    }

    #[test]
    fn test_builder_nam_target_negative() {
        let result = SelfModelConfigBuilder::new().nam_target(-0.1);
        assert!(result.is_err());
    }

    #[test]
    fn test_builder_default_trait() {
        let builder = SelfModelConfigBuilder::default();
        let config = builder.build();
        assert_eq!(config.module_count, DEFAULT_MODULE_COUNT);
    }

    // ========================================================================
    // Group 3: Architecture Descriptor (4 tests)
    // ========================================================================

    #[test]
    fn test_architecture_descriptor_default() {
        let model = default_model();
        let arch = model.architecture();
        assert_eq!(arch.layer_count, 8);
        assert_eq!(arch.module_count, 48);
        assert_eq!(arch.version, "2.0.0");
        assert_eq!(arch.nam_requirements.len(), 5);
    }

    #[test]
    fn test_architecture_descriptor_display() {
        let arch = ArchitectureDescriptor::default();
        let display = format!("{arch}");
        assert!(display.contains("2.0.0"));
        assert!(display.contains("8 layers"));
        assert!(display.contains("48 modules"));
    }

    #[test]
    fn test_architecture_custom_counts() {
        let config = SelfModelConfig {
            module_count: 32,
            layer_count: 6,
            ..SelfModelConfig::default()
        };
        let model = SelfModel::new(config);
        let arch = model.architecture();
        assert_eq!(arch.layer_count, 6);
        assert_eq!(arch.module_count, 32);
    }

    #[test]
    fn test_architecture_nam_requirements() {
        let arch = ArchitectureDescriptor::default();
        assert!(arch.nam_requirements.contains(&"R1:SelfQuery".to_string()));
        assert!(arch.nam_requirements.contains(&"R5:HumanAsAgent".to_string()));
    }

    // ========================================================================
    // Group 4: Capability CRUD (8 tests)
    // ========================================================================

    #[test]
    fn test_update_capability_insert() {
        let model = default_model();
        let entry = sample_capability("health_monitoring", 1, CapabilityStatus::Ready);
        assert!(model.update_capability(entry).is_ok());
        assert_eq!(model.capability_count(), 1);
    }

    #[test]
    fn test_update_capability_overwrite() {
        let model = default_model();
        let entry1 = sample_capability("pipeline", 2, CapabilityStatus::Unknown);
        let entry2 = sample_capability("pipeline", 2, CapabilityStatus::Ready);
        assert!(model.update_capability(entry1).is_ok());
        assert!(model.update_capability(entry2).is_ok());
        assert_eq!(model.capability_count(), 1);

        let status = model.capability_status("pipeline");
        assert!(status.is_ok());
        assert_eq!(status.expect("ok"), CapabilityStatus::Ready);
    }

    #[test]
    fn test_update_capability_empty_name() {
        let _model = default_model();
        let result = CapabilityEntry::new("", 0, CapabilityStatus::Ready, 0.5);
        assert!(result.is_err());
    }

    #[test]
    fn test_update_capability_invalid_layer() {
        let result = CapabilityEntry::new("test", 8, CapabilityStatus::Ready, 0.5);
        assert!(result.is_err());
    }

    #[test]
    fn test_update_capability_score_too_high() {
        let result = CapabilityEntry::new("test", 0, CapabilityStatus::Ready, 1.1);
        assert!(result.is_err());
    }

    #[test]
    fn test_update_capability_score_negative() {
        let result = CapabilityEntry::new("test", 0, CapabilityStatus::Ready, -0.1);
        assert!(result.is_err());
    }

    #[test]
    fn test_capabilities_returns_all() {
        let model = default_model();
        let _ = model.update_capability(sample_capability("a", 0, CapabilityStatus::Ready));
        let _ = model.update_capability(sample_capability("b", 1, CapabilityStatus::Degraded));
        let _ = model.update_capability(sample_capability("c", 2, CapabilityStatus::Unavailable));

        let caps = model.capabilities();
        assert_eq!(caps.len(), 3);
    }

    #[test]
    fn test_capability_entry_boundary_score() {
        let zero = CapabilityEntry::new("zero", 0, CapabilityStatus::Ready, 0.0);
        assert!(zero.is_ok());
        let one = CapabilityEntry::new("one", 0, CapabilityStatus::Ready, 1.0);
        assert!(one.is_ok());
    }

    // ========================================================================
    // Group 5: Capability Status Queries (5 tests)
    // ========================================================================

    #[test]
    fn test_capability_status_found() {
        let model = default_model();
        let _ = model.update_capability(sample_capability("logging", 0, CapabilityStatus::Ready));
        let status = model.capability_status("logging");
        assert!(status.is_ok());
        assert_eq!(status.expect("ok"), CapabilityStatus::Ready);
    }

    #[test]
    fn test_capability_status_not_found() {
        let model = default_model();
        let result = model.capability_status("nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_capability_status_empty_name_error() {
        let model = default_model();
        let result = model.capability_status("");
        assert!(result.is_err());
    }

    #[test]
    fn test_capability_status_default() {
        let status = CapabilityStatus::default();
        assert_eq!(status, CapabilityStatus::Unknown);
    }

    #[test]
    fn test_capability_status_display() {
        assert_eq!(format!("{}", CapabilityStatus::Ready), "READY");
        assert_eq!(format!("{}", CapabilityStatus::Degraded), "DEGRADED");
        assert_eq!(format!("{}", CapabilityStatus::Unavailable), "UNAVAILABLE");
        assert_eq!(format!("{}", CapabilityStatus::Unknown), "UNKNOWN");
    }

    // ========================================================================
    // Group 6: Layer Status (6 tests)
    // ========================================================================

    #[test]
    fn test_layer_status_valid() {
        let model = default_model();
        let status = model.layer_status(0);
        assert!(status.is_ok());
        let entry = status.expect("ok");
        assert_eq!(entry.layer, 0);
        assert_eq!(entry.name, "Foundation");
        assert!((entry.health_score - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_layer_status_all_layers() {
        let model = default_model();
        for i in 0..8u8 {
            let result = model.layer_status(i);
            assert!(result.is_ok(), "layer {i} should be valid");
        }
    }

    #[test]
    fn test_layer_status_invalid() {
        let model = default_model();
        let result = model.layer_status(8);
        assert!(result.is_err());
    }

    #[test]
    fn test_layer_status_after_update() {
        let model = default_model();
        let _ = model.update_layer_health(2, 0.75);
        let status = model.layer_status(2);
        assert!(status.is_ok());
        let entry = status.expect("ok");
        assert!((entry.health_score - 0.75).abs() < f64::EPSILON);
    }

    #[test]
    fn test_layer_status_display() {
        let entry = LayerStatusEntry {
            layer: 0,
            name: "Foundation",
            health_score: 0.95,
            module_count: 10,
            timestamp: Timestamp::now(),
        };
        let display = format!("{entry}");
        assert!(display.contains("Foundation"));
        assert!(display.contains("0.950"));
    }

    #[test]
    fn test_layer_names_coverage() {
        assert_eq!(LAYER_NAMES[0], "Foundation");
        assert_eq!(LAYER_NAMES[1], "Services");
        assert_eq!(LAYER_NAMES[7], "Nexus");
    }

    // ========================================================================
    // Group 7: Layer Health Updates (5 tests)
    // ========================================================================

    #[test]
    fn test_update_layer_health_valid() {
        let model = default_model();
        assert!(model.update_layer_health(0, 0.85).is_ok());
        let status = model.layer_status(0).expect("ok");
        assert!((status.health_score - 0.85).abs() < f64::EPSILON);
    }

    #[test]
    fn test_update_layer_health_invalid_layer() {
        let model = default_model();
        let result = model.update_layer_health(8, 0.5);
        assert!(result.is_err());
    }

    #[test]
    fn test_update_layer_health_score_too_high() {
        let model = default_model();
        let result = model.update_layer_health(0, 1.01);
        assert!(result.is_err());
    }

    #[test]
    fn test_update_layer_health_score_negative() {
        let model = default_model();
        let result = model.update_layer_health(0, -0.01);
        assert!(result.is_err());
    }

    #[test]
    fn test_update_layer_health_boundary_values() {
        let model = default_model();
        assert!(model.update_layer_health(0, 0.0).is_ok());
        assert!(model.update_layer_health(1, 1.0).is_ok());
    }

    // ========================================================================
    // Group 8: NAM Compliance (5 tests)
    // ========================================================================

    #[test]
    fn test_nam_compliance_all_healthy() {
        let model = default_model();
        // All layers default to 1.0, no capabilities => readiness = 1.0
        let compliance = model.nam_compliance();
        assert!((compliance - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_nam_compliance_degraded_layers() {
        let model = default_model();
        for i in 0..8u8 {
            let _ = model.update_layer_health(i, 0.5);
        }
        let compliance = model.nam_compliance();
        // 0.7 * 0.5 + 0.3 * 1.0 = 0.65
        assert!((compliance - 0.65).abs() < f64::EPSILON);
    }

    #[test]
    fn test_nam_compliance_with_ready_capabilities() {
        let model = default_model();
        let _ = model.update_capability(sample_capability("a", 0, CapabilityStatus::Ready));
        let _ = model.update_capability(sample_capability("b", 1, CapabilityStatus::Ready));
        let compliance = model.nam_compliance();
        // All layers 1.0, all capabilities ready => 0.7 * 1.0 + 0.3 * 1.0 = 1.0
        assert!((compliance - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_nam_compliance_with_mixed_capabilities() {
        let model = default_model();
        let _ = model.update_capability(sample_capability("a", 0, CapabilityStatus::Ready));
        let _ = model.update_capability(sample_capability("b", 1, CapabilityStatus::Degraded));
        // layers all 1.0, 1/2 capabilities ready => 0.7 * 1.0 + 0.3 * 0.5 = 0.85
        let compliance = model.nam_compliance();
        assert!((compliance - 0.85).abs() < f64::EPSILON);
    }

    #[test]
    fn test_nam_compliance_zero_health() {
        let model = default_model();
        for i in 0..8u8 {
            let _ = model.update_layer_health(i, 0.0);
        }
        let compliance = model.nam_compliance();
        // 0.7 * 0.0 + 0.3 * 1.0 = 0.3
        assert!((compliance - 0.3).abs() < f64::EPSILON);
    }

    // ========================================================================
    // Group 9: Runtime Snapshot (4 tests)
    // ========================================================================

    #[test]
    fn test_runtime_snapshot_default() {
        let model = default_model();
        let snapshot = model.runtime_snapshot();
        assert_eq!(snapshot.active_module_count, 48);
        assert!((snapshot.overall_health - 1.0).abs() < f64::EPSILON);
        assert!(snapshot.timestamp.ticks() > 0);
    }

    #[test]
    fn test_runtime_snapshot_reflects_layer_changes() {
        let model = default_model();
        let _ = model.update_layer_health(0, 0.5);
        let snapshot = model.runtime_snapshot();
        // (0.5 + 7*1.0) / 8 = 7.5 / 8 = 0.9375
        assert!((snapshot.overall_health - 0.9375).abs() < f64::EPSILON);
        assert!((snapshot.layer_health[0] - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_runtime_snapshot_display() {
        let snapshot = RuntimeSnapshot {
            timestamp: Timestamp::from_raw(42),
            layer_health: [1.0; 8],
            active_module_count: 48,
            nam_compliance_score: 0.95,
            overall_health: 0.98,
        };
        let display = format!("{snapshot}");
        assert!(display.contains("0.980"));
        assert!(display.contains("48"));
    }

    #[test]
    fn test_runtime_snapshot_nam_score() {
        let model = default_model();
        let snapshot = model.runtime_snapshot();
        let direct_nam = model.nam_compliance();
        assert!((snapshot.nam_compliance_score - direct_nam).abs() < f64::EPSILON);
    }

    // ========================================================================
    // Group 10: Health Report (4 tests)
    // ========================================================================

    #[test]
    fn test_health_report_default() {
        let model = default_model();
        let report = model.health_report();
        assert_eq!(report.module_id, "M48");
        assert!((report.overall_score - 1.0).abs() < f64::EPSILON);
        assert_eq!(report.layer_entries.len(), 8);
        assert_eq!(report.capability_summary.total, 0);
    }

    #[test]
    fn test_health_report_with_capabilities() {
        let model = default_model();
        let _ = model.update_capability(sample_capability("a", 0, CapabilityStatus::Ready));
        let _ = model.update_capability(sample_capability("b", 1, CapabilityStatus::Degraded));
        let _ = model.update_capability(sample_capability("c", 2, CapabilityStatus::Unavailable));
        let _ = model.update_capability(sample_capability("d", 3, CapabilityStatus::Unknown));

        let report = model.health_report();
        assert_eq!(report.capability_summary.total, 4);
        assert_eq!(report.capability_summary.ready, 1);
        assert_eq!(report.capability_summary.degraded, 1);
        assert_eq!(report.capability_summary.unavailable, 1);
        assert_eq!(report.capability_summary.unknown, 1);
    }

    #[test]
    fn test_health_report_display() {
        let model = default_model();
        let report = model.health_report();
        let display = format!("{report}");
        assert!(display.contains("M48"));
        assert!(display.contains("1.000"));
    }

    #[test]
    fn test_health_report_layer_entries_ordered() {
        let model = default_model();
        let report = model.health_report();
        for (i, entry) in report.layer_entries.iter().enumerate() {
            #[allow(clippy::cast_possible_truncation)]
            let expected = i as u8;
            assert_eq!(entry.layer, expected);
        }
    }

    // ========================================================================
    // Group 11: Module Count and Active Layers (4 tests)
    // ========================================================================

    #[test]
    fn test_module_count() {
        let model = default_model();
        assert_eq!(model.module_count(), 48);
    }

    #[test]
    fn test_active_layer_count_all_healthy() {
        let model = default_model();
        assert_eq!(model.active_layer_count(), 8);
    }

    #[test]
    fn test_active_layer_count_some_zero() {
        let model = default_model();
        let _ = model.update_layer_health(0, 0.0);
        let _ = model.update_layer_health(3, 0.0);
        assert_eq!(model.active_layer_count(), 6);
    }

    #[test]
    fn test_active_layer_count_all_zero() {
        let model = default_model();
        for i in 0..8u8 {
            let _ = model.update_layer_health(i, 0.0);
        }
        assert_eq!(model.active_layer_count(), 0);
    }

    // ========================================================================
    // Group 12: Thread Safety (4 tests)
    // ========================================================================

    #[test]
    fn test_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<SelfModel>();
    }

    #[test]
    fn test_trait_object_safety() {
        let model = SelfModel::with_defaults();
        let _provider: Box<dyn SelfModelProvider> = Box::new(model);
    }

    #[test]
    fn test_arc_shared() {
        use std::sync::Arc;
        let model = Arc::new(SelfModel::with_defaults());
        let _clone = Arc::clone(&model);
    }

    #[test]
    fn test_concurrent_reads_and_writes() {
        use std::sync::Arc;
        use std::thread;

        let model = Arc::new(SelfModel::with_defaults());
        let mut handles = Vec::new();

        // Spawn readers
        for _ in 0..4 {
            let m = Arc::clone(&model);
            handles.push(thread::spawn(move || {
                let _ = m.runtime_snapshot();
                let _ = m.capabilities();
                let _ = m.nam_compliance();
                let _ = m.active_layer_count();
            }));
        }

        // Spawn writers
        for i in 0..4u8 {
            let m = Arc::clone(&model);
            handles.push(thread::spawn(move || {
                let _ = m.update_layer_health(i, 0.5);
                let entry =
                    CapabilityEntry::new(format!("cap_{i}"), i, CapabilityStatus::Ready, 0.8);
                if let Ok(e) = entry {
                    let _ = m.update_capability(e);
                }
            }));
        }

        for handle in handles {
            handle.join().expect("thread panicked");
        }

        // Should have 4 capabilities after writes
        assert_eq!(model.capability_count(), 4);
    }

    // ========================================================================
    // Group 13: Validation Helpers (6 tests)
    // ========================================================================

    #[test]
    fn test_validate_capability_name_empty() {
        assert!(validate_capability_name("").is_err());
    }

    #[test]
    fn test_validate_capability_name_valid() {
        assert!(validate_capability_name("health").is_ok());
    }

    #[test]
    fn test_validate_layer_boundary() {
        assert!(validate_layer(0).is_ok());
        assert!(validate_layer(7).is_ok());
        assert!(validate_layer(8).is_err());
        assert!(validate_layer(255).is_err());
    }

    #[test]
    fn test_validate_score_in_range() {
        assert!(validate_score(0.0, "test").is_ok());
        assert!(validate_score(0.5, "test").is_ok());
        assert!(validate_score(1.0, "test").is_ok());
    }

    #[test]
    fn test_validate_score_out_of_range() {
        assert!(validate_score(-0.001, "test").is_err());
        assert!(validate_score(1.001, "test").is_err());
        assert!(validate_score(f64::NAN, "test").is_err());
        assert!(validate_score(f64::INFINITY, "test").is_err());
    }

    #[test]
    fn test_modules_per_layer() {
        assert_eq!(modules_per_layer(0), 10);
        assert_eq!(modules_per_layer(1), 4);
        assert_eq!(modules_per_layer(7), 6);
        assert_eq!(modules_per_layer(8), 0);
    }

    // ========================================================================
    // Group 14: Edge Cases and Miscellaneous (3 tests)
    // ========================================================================

    #[test]
    fn test_capability_summary_empty() {
        let map: HashMap<String, CapabilityEntry> = HashMap::new();
        let summary = SelfModel::build_capability_summary(&map);
        assert_eq!(summary.total, 0);
        assert_eq!(summary.ready, 0);
        assert_eq!(summary.degraded, 0);
        assert_eq!(summary.unavailable, 0);
        assert_eq!(summary.unknown, 0);
    }

    #[test]
    fn test_compute_overall_health_uniform() {
        let health = [0.8; 8];
        let overall = SelfModel::compute_overall_health(&health);
        assert!((overall - 0.8).abs() < f64::EPSILON);
    }

    #[test]
    fn test_compute_overall_health_mixed() {
        let health = [1.0, 0.5, 0.75, 0.25, 1.0, 0.0, 0.5, 1.0];
        let overall = SelfModel::compute_overall_health(&health);
        let expected = 5.0 / 8.0; // = 0.625
        assert!((overall - expected).abs() < f64::EPSILON);
    }
}
