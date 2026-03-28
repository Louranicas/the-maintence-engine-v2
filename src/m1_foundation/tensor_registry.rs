//! # M08: Tensor Registry
//!
//! Coverage-aware tensor composition replacing the monolithic
//! [`build_foundation_tensor()`](super::build_foundation_tensor) with a principled,
//! extensible registry of contributors.
//!
//! ## Layer: L1 (Foundation)
//! ## Dependencies: `lib.rs` ([`Tensor12D`](crate::Tensor12D)), M00 (`shared_types`)
//!
//! ## Composition Algorithm
//!
//! Per-dimension averaging only over contributors that actually set that dimension:
//! ```text
//! for each dimension D:
//!   collect all contributors where coverage.is_set(D)
//!   if count > 0:  composed[D] = sum(values) / count, coverage.set(D)
//!   else:          composed[D] = 0.0, coverage bit unset
//! ```
//!
//! ## Contributor Kinds
//!
//! | Kind | Semantics |
//! |------|-----------|
//! | Snapshot | Point-in-time tensor (config, state) |
//! | Stream | Continuously updated tensor (metrics, health) |
//!
//! ## Related Documentation
//! - [Tensor Spec](../../ai_specs/TENSOR_SPEC.md)

use std::fmt;
use std::sync::Arc;

use super::shared_types::{CoverageBitmap, DimensionIndex};

// ============================================================================
// TensorDimension
// ============================================================================

/// Named dimension in the 12D tensor, with mapping to array indices.
///
/// This is a local alias that mirrors [`DimensionIndex`] but provides
/// tensor-registry-specific semantics and documentation.
pub type TensorDimension = DimensionIndex;

// ============================================================================
// ContributorKind
// ============================================================================

/// Classification of tensor contributors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ContributorKind {
    /// Point-in-time tensor (e.g. configuration, persisted state).
    Snapshot,
    /// Continuously updated tensor (e.g. live metrics, health scores).
    Stream,
}

impl fmt::Display for ContributorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Snapshot => write!(f, "Snapshot"),
            Self::Stream => write!(f, "Stream"),
        }
    }
}

// ============================================================================
// ContributedTensor
// ============================================================================

/// A tensor contributed by a single source, with coverage metadata.
#[derive(Debug, Clone, PartialEq)]
pub struct ContributedTensor {
    /// The raw 12D tensor values.
    pub tensor: crate::Tensor12D,
    /// Which dimensions are actually populated.
    pub coverage: CoverageBitmap,
    /// Whether this is a snapshot or stream contribution.
    pub kind: ContributorKind,
}

impl ContributedTensor {
    /// Create a new contributed tensor.
    #[must_use]
    pub const fn new(
        tensor: crate::Tensor12D,
        coverage: CoverageBitmap,
        kind: ContributorKind,
    ) -> Self {
        Self {
            tensor,
            coverage,
            kind,
        }
    }

    /// Read a specific dimension's value, returning `None` if uncovered.
    #[must_use]
    pub const fn dimension_value(&self, dim: DimensionIndex) -> Option<f64> {
        if self.coverage.is_covered(dim) {
            Some(self.tensor.to_array()[dim.index()])
        } else {
            None
        }
    }
}

impl fmt::Display for ContributedTensor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Contributed({}, {})",
            self.kind, self.coverage
        )
    }
}

// ============================================================================
// TensorContributor (trait)
// ============================================================================

/// A source that can produce a tensor with coverage metadata.
///
/// Implementors must be `Send + Sync` for safe use across threads.
pub trait TensorContributor: Send + Sync + fmt::Debug {
    /// Produce the current tensor contribution.
    fn contribute(&self) -> ContributedTensor;

    /// Whether this is a snapshot or stream contributor.
    fn contributor_kind(&self) -> ContributorKind;

    /// Human-readable identifier for this contributor.
    fn module_id(&self) -> &str;
}

// ============================================================================
// ComposedTensor
// ============================================================================

/// Result of composing multiple [`ContributedTensor`]s via the registry.
#[derive(Debug, Clone, PartialEq)]
pub struct ComposedTensor {
    /// The composed 12D tensor.
    pub tensor: crate::Tensor12D,
    /// Union of all contributor coverage.
    pub coverage: CoverageBitmap,
    /// How many contributors participated.
    pub contributor_count: usize,
    /// How many were Snapshot contributors.
    pub snapshot_count: usize,
    /// How many were Stream contributors.
    pub stream_count: usize,
}

impl ComposedTensor {
    /// Coverage ratio (0.0–1.0).
    #[must_use]
    pub fn coverage_ratio(&self) -> f64 {
        self.coverage.coverage_ratio()
    }

    /// Whether all 12 dimensions are covered.
    #[must_use]
    pub fn is_fully_covered(&self) -> bool {
        self.coverage == CoverageBitmap::FULL
    }

    /// Return the list of dimensions with no contributor.
    #[must_use]
    pub fn dead_dimensions(&self) -> Vec<DimensionIndex> {
        self.coverage.uncovered_dimensions()
    }
}

impl fmt::Display for ComposedTensor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Composed({}, contributors={}, snap={}, stream={})",
            self.coverage, self.contributor_count, self.snapshot_count, self.stream_count
        )
    }
}

// ============================================================================
// ContributorInventoryEntry
// ============================================================================

/// Diagnostic entry describing a registered contributor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContributorInventoryEntry {
    /// The contributor's module ID.
    pub module_id: String,
    /// Snapshot or Stream.
    pub kind: ContributorKind,
    /// Which dimensions this contributor covers.
    pub coverage: CoverageBitmap,
}

impl fmt::Display for ContributorInventoryEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Contributor({}, {}, {})",
            self.module_id, self.kind, self.coverage
        )
    }
}

// ============================================================================
// TensorRegistry
// ============================================================================

/// Registry of tensor contributors with coverage-aware composition.
///
/// Contributors are registered once and polled at composition time.
/// The registry itself holds no mutable tensor state — it only orchestrates.
#[derive(Debug, Default)]
pub struct TensorRegistry {
    contributors: Vec<Arc<dyn TensorContributor>>,
}

impl TensorRegistry {
    /// Create a new empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            contributors: Vec::new(),
        }
    }

    /// Register a tensor contributor.
    pub fn register(&mut self, contributor: Arc<dyn TensorContributor>) {
        self.contributors.push(contributor);
    }

    /// Return the number of registered contributors.
    #[must_use]
    pub fn contributor_count(&self) -> usize {
        self.contributors.len()
    }

    /// Compose all contributors into a single tensor.
    ///
    /// Per-dimension averaging: each dimension's value is the mean of all
    /// contributors that have that dimension covered. Uncovered dimensions
    /// remain at 0.0 with the coverage bit unset.
    #[must_use]
    pub fn compose(&self) -> ComposedTensor {
        self.compose_inner(None)
    }

    /// Compose only contributors of a specific kind.
    #[must_use]
    pub fn compose_filtered(&self, kind: ContributorKind) -> ComposedTensor {
        self.compose_inner(Some(kind))
    }

    /// Return a diagnostic inventory of all registered contributors.
    #[must_use]
    pub fn inventory(&self) -> Vec<ContributorInventoryEntry> {
        self.contributors
            .iter()
            .map(|c| {
                let contributed = c.contribute();
                ContributorInventoryEntry {
                    module_id: c.module_id().to_string(),
                    kind: c.contributor_kind(),
                    coverage: contributed.coverage,
                }
            })
            .collect()
    }

    /// Internal composition logic shared between `compose()` and `compose_filtered()`.
    fn compose_inner(&self, filter: Option<ContributorKind>) -> ComposedTensor {
        let mut dim_sums = [0.0f64; 12];
        let mut dim_counts = [0u32; 12];
        let mut overall_coverage = CoverageBitmap::EMPTY;
        let mut contributor_count = 0usize;
        let mut snapshot_count = 0usize;
        let mut stream_count = 0usize;

        for contributor in &self.contributors {
            if let Some(kind_filter) = filter {
                if contributor.contributor_kind() != kind_filter {
                    continue;
                }
            }

            let contributed = contributor.contribute();
            contributor_count += 1;
            match contributed.kind {
                ContributorKind::Snapshot => snapshot_count += 1,
                ContributorKind::Stream => stream_count += 1,
            }

            let array = contributed.tensor.to_array();
            for dim in &DimensionIndex::ALL {
                if contributed.coverage.is_covered(*dim) {
                    let idx = dim.index();
                    dim_sums[idx] += array[idx];
                    dim_counts[idx] += 1;
                    overall_coverage = overall_coverage.with_dimension(*dim);
                }
            }
        }

        let mut dims = [0.0f64; 12];
        for i in 0..12 {
            if dim_counts[i] > 0 {
                dims[i] = (dim_sums[i] / f64::from(dim_counts[i])).clamp(0.0, 1.0);
            }
        }

        ComposedTensor {
            tensor: crate::Tensor12D::new(dims),
            coverage: overall_coverage,
            contributor_count,
            snapshot_count,
            stream_count,
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Tensor12D;

    // ==== TensorDimension tests ====

    #[test]
    fn test_tensor_dimension_is_dimension_index() {
        // TensorDimension is a type alias — verify it works identically
        let d: TensorDimension = DimensionIndex::HealthScore;
        assert_eq!(d.index(), 6);
        assert_eq!(d.name(), "health_score");
    }

    #[test]
    fn test_tensor_dimension_all() {
        assert_eq!(TensorDimension::ALL.len(), 12);
    }

    #[test]
    fn test_tensor_dimension_from_index() {
        assert_eq!(TensorDimension::from_index(0), Some(DimensionIndex::ServiceId));
        assert_eq!(TensorDimension::from_index(11), Some(DimensionIndex::TemporalContext));
        assert_eq!(TensorDimension::from_index(12), None);
    }

    #[test]
    fn test_tensor_dimension_from_name() {
        assert_eq!(TensorDimension::from_name("synergy"), Some(DimensionIndex::Synergy));
        assert_eq!(TensorDimension::from_name("bogus"), None);
    }

    #[test]
    fn test_tensor_dimension_indices_sequential() {
        for (i, dim) in TensorDimension::ALL.iter().enumerate() {
            assert_eq!(dim.index(), i);
        }
    }

    #[test]
    fn test_tensor_dimension_display() {
        assert_eq!(TensorDimension::Latency.to_string(), "D9:latency");
    }

    #[test]
    fn test_tensor_dimension_equality_and_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(TensorDimension::Port);
        set.insert(TensorDimension::Port);
        assert_eq!(set.len(), 1);
    }

    #[test]
    fn test_tensor_dimension_names_unique() {
        let mut names = std::collections::HashSet::new();
        for dim in &TensorDimension::ALL {
            assert!(names.insert(dim.name()));
        }
    }

    // ==== ContributorKind tests ====

    #[test]
    fn test_contributor_kind_display() {
        assert_eq!(ContributorKind::Snapshot.to_string(), "Snapshot");
        assert_eq!(ContributorKind::Stream.to_string(), "Stream");
    }

    #[test]
    fn test_contributor_kind_equality() {
        assert_eq!(ContributorKind::Snapshot, ContributorKind::Snapshot);
        assert_ne!(ContributorKind::Snapshot, ContributorKind::Stream);
    }

    #[test]
    fn test_contributor_kind_copy() {
        let a = ContributorKind::Stream;
        let b = a;
        assert_eq!(a, b);
    }

    // ==== ContributedTensor tests ====

    #[test]
    fn test_contributed_tensor_dimension_value_covered() {
        let mut tensor = Tensor12D::default();
        tensor.health_score = 0.85;
        let coverage = CoverageBitmap::EMPTY.with_dimension(DimensionIndex::HealthScore);
        let ct = ContributedTensor::new(tensor, coverage, ContributorKind::Stream);
        assert_eq!(ct.dimension_value(DimensionIndex::HealthScore), Some(0.85));
    }

    #[test]
    fn test_contributed_tensor_dimension_value_uncovered() {
        let tensor = Tensor12D::default();
        let coverage = CoverageBitmap::EMPTY;
        let ct = ContributedTensor::new(tensor, coverage, ContributorKind::Snapshot);
        assert_eq!(ct.dimension_value(DimensionIndex::HealthScore), None);
    }

    #[test]
    fn test_contributed_tensor_display() {
        let tensor = Tensor12D::default();
        let coverage = CoverageBitmap::EMPTY.with_dimension(DimensionIndex::Port);
        let ct = ContributedTensor::new(tensor, coverage, ContributorKind::Snapshot);
        let display = ct.to_string();
        assert!(display.contains("Snapshot"));
        assert!(display.contains("1/12"));
    }

    #[test]
    fn test_contributed_tensor_clone_eq() {
        let tensor = Tensor12D::new([0.5; 12]);
        let coverage = CoverageBitmap::FULL;
        let a = ContributedTensor::new(tensor, coverage, ContributorKind::Stream);
        let b = a.clone();
        assert_eq!(a, b);
    }

    // ==== ComposedTensor tests ====

    #[test]
    fn test_composed_tensor_fully_covered() {
        let composed = ComposedTensor {
            tensor: Tensor12D::new([0.5; 12]),
            coverage: CoverageBitmap::FULL,
            contributor_count: 3,
            snapshot_count: 1,
            stream_count: 2,
        };
        assert!(composed.is_fully_covered());
        assert!((composed.coverage_ratio() - 1.0).abs() < f64::EPSILON);
        assert!(composed.dead_dimensions().is_empty());
    }

    #[test]
    fn test_composed_tensor_partial_coverage() {
        let composed = ComposedTensor {
            tensor: Tensor12D::default(),
            coverage: CoverageBitmap::EMPTY
                .with_dimension(DimensionIndex::HealthScore)
                .with_dimension(DimensionIndex::Uptime),
            contributor_count: 1,
            snapshot_count: 0,
            stream_count: 1,
        };
        assert!(!composed.is_fully_covered());
        assert_eq!(composed.dead_dimensions().len(), 10);
    }

    #[test]
    fn test_composed_tensor_empty() {
        let composed = ComposedTensor {
            tensor: Tensor12D::default(),
            coverage: CoverageBitmap::EMPTY,
            contributor_count: 0,
            snapshot_count: 0,
            stream_count: 0,
        };
        assert!(!composed.is_fully_covered());
        assert_eq!(composed.dead_dimensions().len(), 12);
        assert!(composed.coverage_ratio().abs() < f64::EPSILON);
    }

    #[test]
    fn test_composed_tensor_display() {
        let composed = ComposedTensor {
            tensor: Tensor12D::default(),
            coverage: CoverageBitmap::FULL,
            contributor_count: 2,
            snapshot_count: 1,
            stream_count: 1,
        };
        let display = composed.to_string();
        assert!(display.contains("contributors=2"));
        assert!(display.contains("snap=1"));
        assert!(display.contains("stream=1"));
    }

    #[test]
    fn test_composed_tensor_clone_eq() {
        let a = ComposedTensor {
            tensor: Tensor12D::new([0.5; 12]),
            coverage: CoverageBitmap::FULL,
            contributor_count: 1,
            snapshot_count: 1,
            stream_count: 0,
        };
        let b = a.clone();
        assert_eq!(a, b);
    }

    // ==== TensorRegistry tests ====

    /// Mock contributor for testing.
    #[derive(Debug)]
    struct MockContributor {
        id: &'static str,
        tensor: Tensor12D,
        coverage: CoverageBitmap,
        kind: ContributorKind,
    }

    impl MockContributor {
        fn new(
            id: &'static str,
            tensor: Tensor12D,
            coverage: CoverageBitmap,
            kind: ContributorKind,
        ) -> Self {
            Self {
                id,
                tensor,
                coverage,
                kind,
            }
        }
    }

    impl TensorContributor for MockContributor {
        fn contribute(&self) -> ContributedTensor {
            ContributedTensor::new(self.tensor, self.coverage, self.kind)
        }

        fn contributor_kind(&self) -> ContributorKind {
            self.kind
        }

        fn module_id(&self) -> &str {
            self.id
        }
    }

    #[test]
    fn test_registry_new_empty() {
        let reg = TensorRegistry::new();
        assert_eq!(reg.contributor_count(), 0);
    }

    #[test]
    fn test_registry_register() {
        let mut reg = TensorRegistry::new();
        let c = Arc::new(MockContributor::new(
            "M02",
            Tensor12D::default(),
            CoverageBitmap::EMPTY,
            ContributorKind::Snapshot,
        ));
        reg.register(c);
        assert_eq!(reg.contributor_count(), 1);
    }

    #[test]
    fn test_registry_compose_empty() {
        let reg = TensorRegistry::new();
        let composed = reg.compose();
        assert_eq!(composed.contributor_count, 0);
        assert_eq!(composed.coverage, CoverageBitmap::EMPTY);
    }

    #[test]
    fn test_registry_compose_single_contributor() {
        let mut reg = TensorRegistry::new();
        let mut tensor = Tensor12D::default();
        tensor.health_score = 0.8;
        tensor.uptime = 0.95;
        let coverage = CoverageBitmap::EMPTY
            .with_dimension(DimensionIndex::HealthScore)
            .with_dimension(DimensionIndex::Uptime);
        reg.register(Arc::new(MockContributor::new(
            "M04",
            tensor,
            coverage,
            ContributorKind::Stream,
        )));

        let composed = reg.compose();
        assert_eq!(composed.contributor_count, 1);
        assert_eq!(composed.stream_count, 1);
        assert_eq!(composed.snapshot_count, 0);
        assert!((composed.tensor.health_score - 0.8).abs() < f64::EPSILON);
        assert!((composed.tensor.uptime - 0.95).abs() < f64::EPSILON);
        assert!(composed.coverage.is_covered(DimensionIndex::HealthScore));
        assert!(composed.coverage.is_covered(DimensionIndex::Uptime));
        assert!(!composed.coverage.is_covered(DimensionIndex::Port));
    }

    #[test]
    fn test_registry_compose_averaging() {
        let mut reg = TensorRegistry::new();

        // Contributor A: health = 0.6
        let mut t1 = Tensor12D::default();
        t1.health_score = 0.6;
        let c1 = CoverageBitmap::EMPTY.with_dimension(DimensionIndex::HealthScore);
        reg.register(Arc::new(MockContributor::new(
            "A", t1, c1, ContributorKind::Stream,
        )));

        // Contributor B: health = 0.8
        let mut t2 = Tensor12D::default();
        t2.health_score = 0.8;
        let c2 = CoverageBitmap::EMPTY.with_dimension(DimensionIndex::HealthScore);
        reg.register(Arc::new(MockContributor::new(
            "B", t2, c2, ContributorKind::Stream,
        )));

        let composed = reg.compose();
        // Average: (0.6 + 0.8) / 2 = 0.7
        assert!((composed.tensor.health_score - 0.7).abs() < f64::EPSILON);
    }

    #[test]
    fn test_registry_compose_non_overlapping() {
        let mut reg = TensorRegistry::new();

        // A covers health
        let mut t1 = Tensor12D::default();
        t1.health_score = 0.9;
        let c1 = CoverageBitmap::EMPTY.with_dimension(DimensionIndex::HealthScore);
        reg.register(Arc::new(MockContributor::new(
            "A", t1, c1, ContributorKind::Stream,
        )));

        // B covers uptime
        let mut t2 = Tensor12D::default();
        t2.uptime = 0.7;
        let c2 = CoverageBitmap::EMPTY.with_dimension(DimensionIndex::Uptime);
        reg.register(Arc::new(MockContributor::new(
            "B", t2, c2, ContributorKind::Snapshot,
        )));

        let composed = reg.compose();
        assert!((composed.tensor.health_score - 0.9).abs() < f64::EPSILON);
        assert!((composed.tensor.uptime - 0.7).abs() < f64::EPSILON);
        assert_eq!(composed.coverage.count(), 2);
    }

    #[test]
    fn test_registry_compose_filtered_snapshot() {
        let mut reg = TensorRegistry::new();

        let mut t1 = Tensor12D::default();
        t1.health_score = 0.5;
        let c1 = CoverageBitmap::EMPTY.with_dimension(DimensionIndex::HealthScore);
        reg.register(Arc::new(MockContributor::new(
            "snap", t1, c1, ContributorKind::Snapshot,
        )));

        let mut t2 = Tensor12D::default();
        t2.health_score = 0.9;
        let c2 = CoverageBitmap::EMPTY.with_dimension(DimensionIndex::HealthScore);
        reg.register(Arc::new(MockContributor::new(
            "stream", t2, c2, ContributorKind::Stream,
        )));

        let snap_only = reg.compose_filtered(ContributorKind::Snapshot);
        assert_eq!(snap_only.contributor_count, 1);
        assert_eq!(snap_only.snapshot_count, 1);
        assert_eq!(snap_only.stream_count, 0);
        assert!((snap_only.tensor.health_score - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_registry_compose_filtered_stream() {
        let mut reg = TensorRegistry::new();

        let mut t1 = Tensor12D::default();
        t1.uptime = 0.3;
        let c1 = CoverageBitmap::EMPTY.with_dimension(DimensionIndex::Uptime);
        reg.register(Arc::new(MockContributor::new(
            "snap", t1, c1, ContributorKind::Snapshot,
        )));

        let mut t2 = Tensor12D::default();
        t2.uptime = 0.8;
        let c2 = CoverageBitmap::EMPTY.with_dimension(DimensionIndex::Uptime);
        reg.register(Arc::new(MockContributor::new(
            "stream", t2, c2, ContributorKind::Stream,
        )));

        let stream_only = reg.compose_filtered(ContributorKind::Stream);
        assert_eq!(stream_only.contributor_count, 1);
        assert!((stream_only.tensor.uptime - 0.8).abs() < f64::EPSILON);
    }

    #[test]
    fn test_registry_compose_clamping() {
        let mut reg = TensorRegistry::new();

        // Contributor with out-of-range value (shouldn't happen in practice)
        let tensor = Tensor12D::new([1.5; 12]);
        reg.register(Arc::new(MockContributor::new(
            "over",
            tensor,
            CoverageBitmap::FULL,
            ContributorKind::Stream,
        )));

        let composed = reg.compose();
        // All values should be clamped to 1.0
        for &val in &composed.tensor.to_array() {
            assert!((val - 1.0).abs() < f64::EPSILON);
        }
    }

    #[test]
    fn test_registry_compose_uncovered_stays_zero() {
        let mut reg = TensorRegistry::new();

        let mut tensor = Tensor12D::default();
        tensor.health_score = 0.5;
        // Only cover health — port should remain 0.0
        let coverage = CoverageBitmap::EMPTY.with_dimension(DimensionIndex::HealthScore);
        reg.register(Arc::new(MockContributor::new(
            "partial",
            tensor,
            coverage,
            ContributorKind::Snapshot,
        )));

        let composed = reg.compose();
        assert!(composed.tensor.port.abs() < f64::EPSILON);
        assert!(!composed.coverage.is_covered(DimensionIndex::Port));
    }

    #[test]
    fn test_registry_compose_many_contributors() {
        let mut reg = TensorRegistry::new();

        for i in 0..10 {
            let mut tensor = Tensor12D::default();
            tensor.synergy = f64::from(i) / 10.0;
            let coverage = CoverageBitmap::EMPTY.with_dimension(DimensionIndex::Synergy);
            reg.register(Arc::new(MockContributor::new(
                "batch",
                tensor,
                coverage,
                ContributorKind::Stream,
            )));
        }

        let composed = reg.compose();
        assert_eq!(composed.contributor_count, 10);
        // Average of 0.0, 0.1, 0.2, ..., 0.9 = 0.45
        assert!((composed.tensor.synergy - 0.45).abs() < 1e-10);
    }

    #[test]
    fn test_registry_inventory() {
        let mut reg = TensorRegistry::new();
        reg.register(Arc::new(MockContributor::new(
            "M02",
            Tensor12D::default(),
            CoverageBitmap::EMPTY.with_dimension(DimensionIndex::Tier),
            ContributorKind::Snapshot,
        )));
        reg.register(Arc::new(MockContributor::new(
            "M04",
            Tensor12D::default(),
            CoverageBitmap::FULL,
            ContributorKind::Stream,
        )));

        let inv = reg.inventory();
        assert_eq!(inv.len(), 2);
        assert_eq!(inv[0].module_id, "M02");
        assert_eq!(inv[0].kind, ContributorKind::Snapshot);
        assert_eq!(inv[0].coverage.count(), 1);
        assert_eq!(inv[1].module_id, "M04");
        assert_eq!(inv[1].kind, ContributorKind::Stream);
        assert_eq!(inv[1].coverage.count(), 12);
    }

    #[test]
    fn test_registry_inventory_entry_display() {
        let entry = ContributorInventoryEntry {
            module_id: "M04".to_string(),
            kind: ContributorKind::Stream,
            coverage: CoverageBitmap::FULL,
        };
        let display = entry.to_string();
        assert!(display.contains("M04"));
        assert!(display.contains("Stream"));
    }

    #[test]
    fn test_registry_default() {
        let reg = TensorRegistry::default();
        assert_eq!(reg.contributor_count(), 0);
    }

    // ==== Integration tests ====

    #[test]
    fn test_full_foundation_composition() {
        let mut reg = TensorRegistry::new();

        // Config contributor (snapshot): tier, service_id
        let mut config_tensor = Tensor12D::default();
        config_tensor.tier = 0.167; // 1/6
        config_tensor.service_id = 0.5;
        let config_cov = CoverageBitmap::EMPTY
            .with_dimension(DimensionIndex::Tier)
            .with_dimension(DimensionIndex::ServiceId);
        reg.register(Arc::new(MockContributor::new(
            "config",
            config_tensor,
            config_cov,
            ContributorKind::Snapshot,
        )));

        // Resources contributor (stream): health, uptime
        let mut res_tensor = Tensor12D::default();
        res_tensor.health_score = 0.95;
        res_tensor.uptime = 0.99;
        let res_cov = CoverageBitmap::EMPTY
            .with_dimension(DimensionIndex::HealthScore)
            .with_dimension(DimensionIndex::Uptime);
        reg.register(Arc::new(MockContributor::new(
            "resources",
            res_tensor,
            res_cov,
            ContributorKind::Stream,
        )));

        // Metrics contributor (stream): latency, error_rate
        let mut met_tensor = Tensor12D::default();
        met_tensor.latency = 0.85;
        met_tensor.error_rate = 0.02;
        let met_cov = CoverageBitmap::EMPTY
            .with_dimension(DimensionIndex::Latency)
            .with_dimension(DimensionIndex::ErrorRate);
        reg.register(Arc::new(MockContributor::new(
            "metrics",
            met_tensor,
            met_cov,
            ContributorKind::Stream,
        )));

        let composed = reg.compose();
        assert_eq!(composed.contributor_count, 3);
        assert_eq!(composed.snapshot_count, 1);
        assert_eq!(composed.stream_count, 2);
        assert_eq!(composed.coverage.count(), 6);
        assert!((composed.tensor.tier - 0.167).abs() < f64::EPSILON);
        assert!((composed.tensor.health_score - 0.95).abs() < f64::EPSILON);
        assert!((composed.tensor.latency - 0.85).abs() < f64::EPSILON);
    }

    #[test]
    fn test_composition_matches_build_foundation_tensor_for_full_overlap() {
        // When 3 contributors all cover all 12 dims, compose() should
        // give the same result as the old build_foundation_tensor()
        let t1 = Tensor12D::new([0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 0.1, 0.2, 0.3]);
        let t2 = Tensor12D::new([0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 0.1, 0.2, 0.3, 0.4, 0.5]);
        let t3 = Tensor12D::new([0.5, 0.6, 0.7, 0.8, 0.9, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7]);

        let old_result = super::super::build_foundation_tensor(&t1, &t2, &t3);

        let mut reg = TensorRegistry::new();
        reg.register(Arc::new(MockContributor::new(
            "c", t1, CoverageBitmap::FULL, ContributorKind::Snapshot,
        )));
        reg.register(Arc::new(MockContributor::new(
            "r", t2, CoverageBitmap::FULL, ContributorKind::Stream,
        )));
        reg.register(Arc::new(MockContributor::new(
            "m", t3, CoverageBitmap::FULL, ContributorKind::Stream,
        )));
        let new_result = reg.compose();

        let old_arr = old_result.to_array();
        let new_arr = new_result.tensor.to_array();
        for i in 0..12 {
            assert!(
                (old_arr[i] - new_arr[i]).abs() < 1e-10,
                "Dim {i} mismatch: old={} new={}",
                old_arr[i],
                new_arr[i]
            );
        }
    }

    #[test]
    fn test_registry_compose_filtered_empty_result() {
        let mut reg = TensorRegistry::new();
        reg.register(Arc::new(MockContributor::new(
            "snap_only",
            Tensor12D::default(),
            CoverageBitmap::FULL,
            ContributorKind::Snapshot,
        )));

        let stream_result = reg.compose_filtered(ContributorKind::Stream);
        assert_eq!(stream_result.contributor_count, 0);
        assert_eq!(stream_result.coverage, CoverageBitmap::EMPTY);
    }

    // ==== [COMPILE] Trait safety tests ====

    #[test]
    fn test_tensor_contributor_is_object_safe() {
        // [COMPILE] TensorContributor must be usable as a trait object.
        fn accept_boxed(_c: Box<dyn TensorContributor>) {}
        let c = Box::new(MockContributor::new(
            "compile",
            Tensor12D::default(),
            CoverageBitmap::EMPTY,
            ContributorKind::Snapshot,
        ));
        accept_boxed(c);
    }

    #[test]
    fn test_tensor_contributor_is_send_sync() {
        // [COMPILE] TensorContributor trait objects must be Send + Sync.
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<Arc<dyn TensorContributor>>();
    }

    // ==== [INVARIANT] tests ====

    #[test]
    fn test_compose_output_always_clamped_to_unit_invariant() {
        // [INVARIANT] Every dimension in compose output is in [0.0, 1.0].
        let mut reg = TensorRegistry::new();
        reg.register(Arc::new(MockContributor::new(
            "over",
            Tensor12D::new([2.0; 12]),
            CoverageBitmap::FULL,
            ContributorKind::Stream,
        )));
        reg.register(Arc::new(MockContributor::new(
            "under",
            Tensor12D::new([-1.0; 12]),
            CoverageBitmap::FULL,
            ContributorKind::Stream,
        )));
        let composed = reg.compose();
        for &val in &composed.tensor.to_array() {
            assert!(
                val >= 0.0 && val <= 1.0,
                "Composed value {val} outside [0,1]"
            );
        }
    }

    #[test]
    fn test_compose_contributor_count_matches_registered() {
        // [INVARIANT] contributor_count equals the number of registered contributors.
        let mut reg = TensorRegistry::new();
        for i in 0..5 {
            let id: &'static str = match i {
                0 => "A",
                1 => "B",
                2 => "C",
                3 => "D",
                _ => "E",
            };
            reg.register(Arc::new(MockContributor::new(
                id,
                Tensor12D::default(),
                CoverageBitmap::EMPTY,
                ContributorKind::Snapshot,
            )));
        }
        let composed = reg.compose();
        assert_eq!(composed.contributor_count, reg.contributor_count());
    }

    #[test]
    fn test_coverage_count_matches_covered_dims() {
        // [INVARIANT] coverage.count() equals the number of dimensions actually set.
        let mut reg = TensorRegistry::new();
        let cov = CoverageBitmap::EMPTY
            .with_dimension(DimensionIndex::HealthScore)
            .with_dimension(DimensionIndex::Port)
            .with_dimension(DimensionIndex::Synergy);
        reg.register(Arc::new(MockContributor::new(
            "three-dims",
            Tensor12D::new([0.5; 12]),
            cov,
            ContributorKind::Stream,
        )));
        let composed = reg.compose();
        assert_eq!(composed.coverage.count(), 3);
    }

    // ==== [BOUNDARY] tests ====

    #[test]
    fn test_compose_single_dim_single_contributor() {
        // [BOUNDARY] Minimal composition: 1 contributor, 1 dimension.
        let mut reg = TensorRegistry::new();
        let mut tensor = Tensor12D::default();
        tensor.error_rate = 0.42;
        let cov = CoverageBitmap::EMPTY.with_dimension(DimensionIndex::ErrorRate);
        reg.register(Arc::new(MockContributor::new(
            "minimal",
            tensor,
            cov,
            ContributorKind::Snapshot,
        )));
        let composed = reg.compose();
        assert!((composed.tensor.error_rate - 0.42).abs() < f64::EPSILON);
        assert_eq!(composed.coverage.count(), 1);
        assert_eq!(composed.dead_dimensions().len(), 11);
    }

    #[test]
    fn test_compose_boundary_values_0_and_1() {
        // [BOUNDARY] Values at exact boundaries remain exact.
        let mut reg = TensorRegistry::new();
        let tensor = Tensor12D::new([0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0]);
        reg.register(Arc::new(MockContributor::new(
            "bounds",
            tensor,
            CoverageBitmap::FULL,
            ContributorKind::Stream,
        )));
        let composed = reg.compose();
        let arr = composed.tensor.to_array();
        for (i, &val) in arr.iter().enumerate() {
            if i % 2 == 0 {
                assert!(val.abs() < f64::EPSILON, "D{i} should be 0.0");
            } else {
                assert!((val - 1.0).abs() < f64::EPSILON, "D{i} should be 1.0");
            }
        }
    }

    #[test]
    fn test_compose_all_dims_all_contributors_boundary() {
        // [BOUNDARY] 12 contributors each covering exactly 1 dimension.
        let mut reg = TensorRegistry::new();
        let names: [&'static str; 12] = [
            "D0", "D1", "D2", "D3", "D4", "D5", "D6", "D7", "D8", "D9", "D10", "D11",
        ];
        for (i, dim) in DimensionIndex::ALL.iter().enumerate() {
            let mut vals = [0.0f64; 12];
            vals[i] = f64::from(i as u8 + 1) / 12.0;
            let cov = CoverageBitmap::EMPTY.with_dimension(*dim);
            reg.register(Arc::new(MockContributor::new(
                names[i],
                Tensor12D::new(vals),
                cov,
                ContributorKind::Stream,
            )));
        }
        let composed = reg.compose();
        assert!(composed.is_fully_covered());
        assert_eq!(composed.contributor_count, 12);
    }

    // ==== [PROPERTY] tests ====

    #[test]
    fn test_compose_dims_always_in_unit_interval_property() {
        // [PROPERTY] For any set of contributors, all composed dims are in [0.0, 1.0].
        let mut reg = TensorRegistry::new();
        let values_sets: [[f64; 12]; 4] = [
            [0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 0.95, 0.99, 1.0],
            [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            [1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0],
            [0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.5],
        ];
        let ids: [&'static str; 4] = ["A", "B", "C", "D"];
        for (i, vals) in values_sets.iter().enumerate() {
            reg.register(Arc::new(MockContributor::new(
                ids[i],
                Tensor12D::new(*vals),
                CoverageBitmap::FULL,
                ContributorKind::Stream,
            )));
        }
        let composed = reg.compose();
        for &val in &composed.tensor.to_array() {
            assert!(val >= 0.0 && val <= 1.0, "Value {val} outside [0,1]");
        }
    }

    #[test]
    fn test_compose_coverage_is_union_of_contributors() {
        // [PROPERTY] Composed coverage is the union of all contributor coverages.
        let mut reg = TensorRegistry::new();
        let c1 = CoverageBitmap::EMPTY
            .with_dimension(DimensionIndex::HealthScore)
            .with_dimension(DimensionIndex::Port);
        let c2 = CoverageBitmap::EMPTY
            .with_dimension(DimensionIndex::Synergy)
            .with_dimension(DimensionIndex::Port);
        reg.register(Arc::new(MockContributor::new(
            "A",
            Tensor12D::new([0.5; 12]),
            c1,
            ContributorKind::Snapshot,
        )));
        reg.register(Arc::new(MockContributor::new(
            "B",
            Tensor12D::new([0.5; 12]),
            c2,
            ContributorKind::Stream,
        )));
        let composed = reg.compose();
        let expected = c1.union(c2);
        assert_eq!(composed.coverage, expected);
    }

    #[test]
    fn test_compose_snapshot_plus_stream_covers_all() {
        // [PROPERTY] Filtered compose of snapshots + streams covers all contributors.
        let mut reg = TensorRegistry::new();
        reg.register(Arc::new(MockContributor::new(
            "snap",
            Tensor12D::new([0.3; 12]),
            CoverageBitmap::FULL,
            ContributorKind::Snapshot,
        )));
        reg.register(Arc::new(MockContributor::new(
            "stream",
            Tensor12D::new([0.7; 12]),
            CoverageBitmap::FULL,
            ContributorKind::Stream,
        )));
        let snap_only = reg.compose_filtered(ContributorKind::Snapshot);
        let stream_only = reg.compose_filtered(ContributorKind::Stream);
        assert_eq!(
            snap_only.contributor_count + stream_only.contributor_count,
            reg.contributor_count()
        );
    }

    // ==== [NEGATIVE] tests ====

    #[test]
    fn test_compose_negative_values_clamped_to_zero() {
        // [NEGATIVE] Contributors with negative values get clamped to 0.0.
        let mut reg = TensorRegistry::new();
        reg.register(Arc::new(MockContributor::new(
            "negative",
            Tensor12D::new([-0.5; 12]),
            CoverageBitmap::FULL,
            ContributorKind::Stream,
        )));
        let composed = reg.compose();
        for &val in &composed.tensor.to_array() {
            assert!(val.abs() < f64::EPSILON, "Expected 0.0, got {val}");
        }
    }

    #[test]
    fn test_compose_contributor_with_empty_coverage_no_effect() {
        // [NEGATIVE] A contributor with EMPTY coverage doesn't affect composition.
        let mut reg = TensorRegistry::new();
        let mut tensor = Tensor12D::default();
        tensor.health_score = 0.8;
        let cov = CoverageBitmap::EMPTY.with_dimension(DimensionIndex::HealthScore);
        reg.register(Arc::new(MockContributor::new(
            "real",
            tensor,
            cov,
            ContributorKind::Stream,
        )));
        reg.register(Arc::new(MockContributor::new(
            "empty",
            Tensor12D::new([0.999; 12]),
            CoverageBitmap::EMPTY,
            ContributorKind::Snapshot,
        )));
        let composed = reg.compose();
        // The "empty" contributor shouldn't affect health_score.
        assert!((composed.tensor.health_score - 0.8).abs() < f64::EPSILON);
        assert_eq!(composed.coverage.count(), 1);
    }

    // ==== [INTEGRATION] additional tests ====

    #[test]
    fn test_compose_ordering_invariant() {
        // [INTEGRATION] Registration order doesn't affect compose result.
        let t1 = Tensor12D::new([0.2; 12]);
        let t2 = Tensor12D::new([0.8; 12]);

        let mut reg_a = TensorRegistry::new();
        reg_a.register(Arc::new(MockContributor::new("X", t1, CoverageBitmap::FULL, ContributorKind::Snapshot)));
        reg_a.register(Arc::new(MockContributor::new("Y", t2, CoverageBitmap::FULL, ContributorKind::Stream)));

        let mut reg_b = TensorRegistry::new();
        reg_b.register(Arc::new(MockContributor::new("Y", t2, CoverageBitmap::FULL, ContributorKind::Stream)));
        reg_b.register(Arc::new(MockContributor::new("X", t1, CoverageBitmap::FULL, ContributorKind::Snapshot)));

        let a_arr = reg_a.compose().tensor.to_array();
        let b_arr = reg_b.compose().tensor.to_array();
        for i in 0..12 {
            assert!(
                (a_arr[i] - b_arr[i]).abs() < 1e-10,
                "Dim {i} differs: {} vs {}",
                a_arr[i],
                b_arr[i]
            );
        }
    }

    #[test]
    fn test_compose_identical_contributors_gives_same_value() {
        // [REGRESSION] 3 identical contributors should yield the same value.
        let mut reg = TensorRegistry::new();
        let tensor = Tensor12D::new([0.6; 12]);
        for id in &["A", "B", "C"] {
            reg.register(Arc::new(MockContributor::new(
                id,
                tensor,
                CoverageBitmap::FULL,
                ContributorKind::Stream,
            )));
        }
        let composed = reg.compose();
        for &val in &composed.tensor.to_array() {
            assert!((val - 0.6).abs() < f64::EPSILON, "Expected 0.6, got {val}");
        }
    }

    #[test]
    fn test_dead_dimensions_complement_coverage() {
        // [PROPERTY] dead_dimensions + covered dimensions = 12.
        let cov = CoverageBitmap::EMPTY
            .with_dimension(DimensionIndex::HealthScore)
            .with_dimension(DimensionIndex::Port)
            .with_dimension(DimensionIndex::Synergy)
            .with_dimension(DimensionIndex::Latency);
        let composed = ComposedTensor {
            tensor: Tensor12D::default(),
            coverage: cov,
            contributor_count: 1,
            snapshot_count: 0,
            stream_count: 1,
        };
        let dead = composed.dead_dimensions();
        assert_eq!(dead.len() + composed.coverage.count() as usize, 12);
    }

    #[test]
    fn test_inventory_matches_registered_order() {
        // [INTEGRATION] Inventory preserves registration order.
        let mut reg = TensorRegistry::new();
        let ids = ["first", "second", "third"];
        for &id in &ids {
            reg.register(Arc::new(MockContributor::new(
                id,
                Tensor12D::default(),
                CoverageBitmap::EMPTY,
                ContributorKind::Snapshot,
            )));
        }
        let inv = reg.inventory();
        for (i, entry) in inv.iter().enumerate() {
            assert_eq!(entry.module_id, ids[i]);
        }
    }

    #[test]
    fn test_compose_mixed_kinds_full_scenario() {
        // [INTEGRATION] Realistic scenario: 2 snapshots + 3 streams, partial overlap.
        let mut reg = TensorRegistry::new();

        // Snapshot 1: tier + service_id
        let mut t = Tensor12D::default();
        t.tier = 0.5;
        t.service_id = 0.25;
        let c = CoverageBitmap::EMPTY
            .with_dimension(DimensionIndex::Tier)
            .with_dimension(DimensionIndex::ServiceId);
        reg.register(Arc::new(MockContributor::new("snap1", t, c, ContributorKind::Snapshot)));

        // Snapshot 2: tier + protocol (overlapping tier)
        let mut t2 = Tensor12D::default();
        t2.tier = 0.3;
        t2.protocol = 0.75;
        let c2 = CoverageBitmap::EMPTY
            .with_dimension(DimensionIndex::Tier)
            .with_dimension(DimensionIndex::Protocol);
        reg.register(Arc::new(MockContributor::new("snap2", t2, c2, ContributorKind::Snapshot)));

        // Stream 1: health + uptime
        let mut t3 = Tensor12D::default();
        t3.health_score = 0.95;
        t3.uptime = 0.99;
        let c3 = CoverageBitmap::EMPTY
            .with_dimension(DimensionIndex::HealthScore)
            .with_dimension(DimensionIndex::Uptime);
        reg.register(Arc::new(MockContributor::new("stream1", t3, c3, ContributorKind::Stream)));

        let composed = reg.compose();
        assert_eq!(composed.contributor_count, 3);
        assert_eq!(composed.snapshot_count, 2);
        assert_eq!(composed.stream_count, 1);
        // Tier is averaged: (0.5 + 0.3) / 2 = 0.4
        assert!((composed.tensor.tier - 0.4).abs() < f64::EPSILON);
        // Health is from single contributor
        assert!((composed.tensor.health_score - 0.95).abs() < f64::EPSILON);
        assert_eq!(composed.coverage.count(), 5);
    }
}
