# M08 Tensor Registry — tensor_registry.rs

> **File:** `src/m1_foundation/tensor_registry.rs` | **LOC:** ~1,335 | **Tests:** ~80
> **Role:** Coverage-aware 12D tensor composition from multiple module contributors

---

## TensorContributor Trait

```rust
pub trait TensorContributor: Send + Sync + fmt::Debug {
    fn contribute(&self) -> ContributedTensor;
    fn contributor_kind(&self) -> ContributorKind;
    fn module_id(&self) -> &str;
}
```

**Object safety:** verified (compile-test). Used as `Arc<dyn TensorContributor>`.

All methods take `&self` — implementations use interior mutability (RwLock) if needed.

---

## ContributorKind

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ContributorKind { Snapshot, Stream }
```

Traits: `Display` ("Snapshot", "Stream")

---

## ContributedTensor

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct ContributedTensor {
    pub tensor: Tensor12D,
    pub coverage: CoverageBitmap,
    pub kind: ContributorKind,
}
```

| Method | Signature | Notes |
|--------|-----------|-------|
| `new(tensor, coverage, kind)` | `const fn -> Self` | Pure constructor |
| `dimension_value(dim)` | `const fn -> Option<f64>` | Some if covered, None if not |

Traits: `Display` ("Contributed(Snapshot, 4/12)")

---

## ComposedTensor

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct ComposedTensor {
    pub tensor: Tensor12D,
    pub coverage: CoverageBitmap,
    pub contributor_count: usize,
    pub snapshot_count: usize,
    pub stream_count: usize,
}
```

| Method | Returns | Notes |
|--------|---------|-------|
| `coverage_ratio()` | `f64` | count/12 |
| `is_fully_covered()` | `bool` | coverage == FULL |
| `dead_dimensions()` | `Vec<DimensionIndex>` | Uncovered dims |

Traits: `Display` ("Composed(12/12, contributors=4, snap=2, stream=2)")

---

## TensorRegistry

```rust
#[derive(Debug, Default)]
pub struct TensorRegistry {
    contributors: Vec<Arc<dyn TensorContributor>>,
}
```

| Method | Signature | Notes |
|--------|-----------|-------|
| `new()` | `-> Self` | Empty registry |
| `register(&mut self, contributor)` | `()` | Appends, no limit |
| `contributor_count()` | `-> usize` | |
| `compose()` | `-> ComposedTensor` | All contributors |
| `compose_filtered(kind)` | `-> ComposedTensor` | Filtered by kind |
| `inventory()` | `-> Vec<ContributorInventoryEntry>` | Current state snapshot |

---

## Composition Algorithm

```
1. For each contributor (optionally filtered by kind):
   a. Call contribute() → get ContributedTensor
   b. For each covered dimension i:
      - dim_sums[i] += tensor[i]
      - dim_counts[i] += 1
   c. overall_coverage = union(all contributor coverages)
   d. Track snapshot_count / stream_count

2. For each dimension i where dim_counts[i] > 0:
   - composed[i] = (dim_sums[i] / dim_counts[i]).clamp(0.0, 1.0)

3. Return ComposedTensor with:
   - tensor: the averaged values
   - coverage: union bitmap
   - contributor_count, snapshot_count, stream_count
```

**Key invariant:** Output always in [0.0, 1.0] per dimension (clamped after averaging).

**D6 overlap example:** M10 and M11 both contribute D6 (health). The algorithm averages them: `(M10_D6 + M11_D6) / 2`. This blends probe-based health with lifecycle-based health.

---

## Concurrency Model

**TensorRegistry has NO internal synchronization:**
- `register(&mut self)` — requires exclusive ownership (setup-phase only)
- `compose(&self)` — safe for concurrent reads
- Caller is responsible for wrapping in `RwLock` if runtime registration needed

**Contributors are thread-safe:** `Arc<dyn TensorContributor>` with `Send + Sync` bounds.

**Contrast with SignalBus:** SignalBus wraps its subscriber list in `Arc<RwLock<>>`. TensorRegistry does not — designed for setup-once, compose-many.

---

## Type Alias

```rust
pub type TensorDimension = DimensionIndex;
```

---

## ContributorInventoryEntry

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContributorInventoryEntry {
    pub module_id: String,
    pub kind: ContributorKind,
    pub coverage: CoverageBitmap,
}
```

Traits: `Display` ("Contributor(M04, Stream, 12/12)")

---

## Error Conditions

**Zero.** All methods are infallible:
- `register()` has no limit check
- `compose()` returns empty ComposedTensor when no contributors match
- `inventory()` returns empty Vec when no contributors

---

## L2 Contributor Map

| Module | Dimensions | Kind | Coverage |
|--------|------------|------|----------|
| M09 ServiceRegistry | D0, D2, D3, D4 | Snapshot | 4/12 |
| M10 HealthMonitor | D6, D10 | Snapshot | 2/12 |
| M11 Lifecycle | D6, D7 | Snapshot | 2/12 |
| M12 Resilience | D9, D10 | Snapshot | 2/12 |
| **Union** | D0,D2,D3,D4,D6,D7,D9,D10 | — | **8/12** |

Unused dimensions (no contributor): D1 (Port), D5 (Protocol), D8 (Synergy), D11 (TemporalContext)

---

*M08 Tensor Registry Spec v1.0 | 2026-03-01*
