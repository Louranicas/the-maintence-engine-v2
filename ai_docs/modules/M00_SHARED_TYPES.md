# Module M00: Shared Types

> **M00_SHARED_TYPES** | Pure Vocabulary Types for Cross-Module Coordination | Layer: L1 Foundation | [Back to Index](../INDEX.md)

---

## Navigation

| Direction | Link |
|-----------|------|
| Up | [INDEX.md](../INDEX.md) |
| Layer | [L01_FOUNDATION.md](../layers/L01_FOUNDATION.md) |
| Related | [M07_SIGNALS.md](M07_SIGNALS.md) |
| Related | [M08_TENSOR_REGISTRY.md](M08_TENSOR_REGISTRY.md) |
| Related | [M01_ERROR_TAXONOMY.md](M01_ERROR_TAXONOMY.md) |
| L2 Services | [L02_SERVICES.md](../layers/L02_SERVICES.md) |

---

## Module Specification

### Overview

The Shared Types module provides pure vocabulary types for cross-module coordination within the Maintenance Engine. It contains zero logic and zero I/O -- only type definitions, constructors, and trivial accessors. Every type defined here is `Send + Sync`, uses `const fn` wherever the compiler allows, and all constructors are marked `#[must_use]`. This module is the leaf dependency of the entire crate: no other internal module is imported.

### Module Properties

| Property | Value |
|----------|-------|
| Module ID | M00 |
| Module Name | Shared Types |
| Layer | L1 (Foundation) |
| Version | 1.0.0 |
| Source File | `src/m1_foundation/shared_types.rs` |
| LOC | ~1,210 |
| Tests | 55 |
| Dependencies | None (leaf module) |
| Dependents | All modules (M01-M42) |

---

## Architecture Diagram

```
+-----------------------------------------------------------------------------------+
|                         M00: SHARED TYPES                                         |
+-----------------------------------------------------------------------------------+
|                                                                                   |
|  +------------------------+    +------------------------+    +-----------------+  |
|  |      MODULE ID         |    |      AGENT ID          |    |   TIMESTAMP     |  |
|  |                        |    |                        |    |                 |  |
|  | - &'static str wrapper |    | - String wrapper       |    | - AtomicU64     |  |
|  | - M01-M42 constants    |    | - Prefix convention:   |    |   monotonic     |  |
|  | - layer() -> 1..7      |    |   sys: human: svc:     |    | - now() strictly|  |
|  | - number() -> 1..42    |    |   agent:               |    |   increasing    |  |
|  | - ALL: [Self; 42]      |    | - is_system/human/...  |    | - ZERO constant |  |
|  +------------------------+    +------------------------+    +-----------------+  |
|                                                                                   |
|  +------------------------+    +------------------------+    +-----------------+  |
|  |    HEALTH REPORT       |    |   DIMENSION INDEX      |    | COVERAGE BITMAP |  |
|  |                        |    |                        |    |                 |  |
|  | - module_id: ModuleId  |    | - 12 variants (D0-D11) |    | - u16 bitmask   |  |
|  | - health_score: f64    |    | - #[repr(u8)]          |    | - EMPTY / FULL  |  |
|  |   clamped [0.0, 1.0]   |    | - index(), name()      |    | - union/inter   |  |
|  | - timestamp: Timestamp |    | - from_index/from_name |    | - coverage_ratio|  |
|  | - is_healthy/critical  |    | - ALL: [Self; 12]      |    | - covered_dims  |  |
|  +------------------------+    +------------------------+    +-----------------+  |
|                                                                                   |
+-----------------------------------------------------------------------------------+
        |                    |                    |                    |
        v                    v                    v                    v
   [All L1 Modules]    [L2 Services]       [L5 Learning]       [L6 Consensus]
   M01-M06, M07, M08   M09-M12             M25-M30             M31-M36
```

---

## Core Data Structures

### ModuleId

```rust
/// Typed identity for a module in the Maintenance Engine.
///
/// Wraps a `&'static str` with compile-time constants for M01-M42.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ModuleId(&'static str);

impl ModuleId {
    // L1: Foundation
    pub const M01: Self = Self("M01");  // Error Taxonomy
    pub const M02: Self = Self("M02");  // Configuration
    // ... M03-M06 ...

    // L2: Services
    pub const M07: Self = Self("M07");  // Service Types
    // ... M08-M12 ...

    // L3-L6: M13-M36
    // L7: Observer (M37-M39)
    // HRS-001: Neural Homeostasis (M40-M42)

    /// All known module IDs in order.
    pub const ALL: [Self; 42] = [ /* M01..M42 */ ];

    /// Create a `ModuleId` from a static string.
    #[must_use]
    pub const fn new(id: &'static str) -> Self;

    /// Return the raw string identifier (e.g. `"M01"`).
    #[must_use]
    pub const fn as_str(&self) -> &'static str;

    /// Extract the numeric suffix (e.g. `ModuleId::M01` -> `1`).
    #[must_use]
    pub fn number(&self) -> Option<u8>;

    /// Return the layer (1-based) this module belongs to.
    /// M01-M06 -> 1, M07-M12 -> 2, ..., M37-M42 -> 7
    #[must_use]
    pub fn layer(&self) -> Option<u8>;
}
```

### AgentId

```rust
/// Typed operational identity for an agent.
///
/// Prefix convention:
/// - `"sys:"` -- system-level automated operations
/// - `"human:"` -- human agent (NAM R5)
/// - `"svc:"` -- ULTRAPLATE service
/// - `"agent:"` -- CVA-NAM fleet agent
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AgentId(String);

impl AgentId {
    #[must_use] pub fn system() -> Self;          // "sys:system"
    #[must_use] pub fn human() -> Self;           // "human:@0.A"
    #[must_use] pub fn service(id: &str) -> Self; // "svc:{id}"
    #[must_use] pub fn agent(id: &str) -> Self;   // "agent:{id}"
    #[must_use] pub fn from_raw(raw: impl Into<String>) -> Self;
    #[must_use] pub fn as_str(&self) -> &str;
    #[must_use] pub fn prefix(&self) -> &str;
    #[must_use] pub fn is_system(&self) -> bool;
    #[must_use] pub fn is_human(&self) -> bool;
    #[must_use] pub fn is_service(&self) -> bool;
    #[must_use] pub fn is_agent(&self) -> bool;
}
```

### Timestamp

```rust
/// Monotonic cycle-counter timestamp (NOT wall-clock time).
///
/// Every call to `Timestamp::now()` returns a strictly increasing value,
/// making it safe for STDP timing windows and causal ordering.
///
/// Per ULTRAPLATE convention: no chrono, no SystemTime -- only cycle counters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Timestamp(u64);

/// Global monotonic counter backing Timestamp::now().
static GLOBAL_TICK: AtomicU64 = AtomicU64::new(1);

impl Timestamp {
    pub const ZERO: Self = Self(0);

    #[must_use] pub fn now() -> Self;                          // fetch_add(1, Relaxed)
    #[must_use] pub const fn from_raw(ticks: u64) -> Self;
    #[must_use] pub const fn ticks(&self) -> u64;
    #[must_use] pub const fn elapsed_since(&self, earlier: Self) -> u64;  // saturating
    #[must_use] pub const fn within_window(&self, other: Self, window: u64) -> bool;
}
```

### HealthReport

```rust
/// Per-module health snapshot.
/// Health score is clamped to [0.0, 1.0] at construction.
#[derive(Debug, Clone, PartialEq)]
pub struct HealthReport {
    pub module_id: ModuleId,
    pub health_score: f64,       // clamped [0.0, 1.0]
    pub timestamp: Timestamp,
    pub details: Option<String>,
}

impl HealthReport {
    #[must_use] pub fn new(module_id: ModuleId, health_score: f64) -> Self;
    #[must_use] pub fn with_details(self, details: impl Into<String>) -> Self;
    #[must_use] pub const fn with_timestamp(self, timestamp: Timestamp) -> Self;
    #[must_use] pub fn is_healthy(&self) -> bool;   // score >= 0.5
    #[must_use] pub fn is_critical(&self) -> bool;  // score < 0.2
}
```

### DimensionIndex

```rust
/// Named index into the 12D tensor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum DimensionIndex {
    ServiceId = 0,       // D0: Service identifier (normalized hash)
    Port = 1,            // D1: Port (port/65535)
    Tier = 2,            // D2: Tier (tier/6)
    DependencyCount = 3, // D3: Dependency count (log normalized)
    AgentCount = 4,      // D4: Agent count (agents/40)
    Protocol = 5,        // D5: Protocol (enum encoding)
    HealthScore = 6,     // D6: Health score (0-1)
    Uptime = 7,          // D7: Uptime ratio (0-1)
    Synergy = 8,         // D8: Synergy score (0-1)
    Latency = 9,         // D9: Latency (1 - latency_ms/2000)
    ErrorRate = 10,      // D10: Error rate (0-1)
    TemporalContext = 11,// D11: Temporal context (time encoding)
}

impl DimensionIndex {
    pub const ALL: [Self; 12] = [ /* all variants */ ];
    #[must_use] pub const fn index(self) -> usize;
    #[must_use] pub const fn name(self) -> &'static str;
    #[must_use] pub const fn from_index(index: usize) -> Option<Self>;
    #[must_use] pub fn from_name(name: &str) -> Option<Self>;
}
```

### CoverageBitmap

```rust
/// Bitmask tracking which of the 12 tensor dimensions are populated.
/// Internally a u16 with only the bottom 12 bits used.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct CoverageBitmap(u16);

impl CoverageBitmap {
    pub const EMPTY: Self = Self(0);
    pub const FULL: Self = Self(0x0FFF);

    #[must_use] pub const fn from_raw(bits: u16) -> Self;       // masked to 12 bits
    #[must_use] pub const fn raw(self) -> u16;
    #[must_use] pub const fn with_dimension(self, dim: DimensionIndex) -> Self;
    #[must_use] pub const fn is_covered(self, dim: DimensionIndex) -> bool;
    #[must_use] pub const fn count(self) -> u32;                 // popcount
    #[must_use] pub const fn union(self, other: Self) -> Self;   // OR
    #[must_use] pub const fn intersection(self, other: Self) -> Self; // AND
    #[must_use] pub fn coverage_ratio(self) -> f64;              // count/12
    #[must_use] pub fn covered_dimensions(self) -> Vec<DimensionIndex>;
    #[must_use] pub fn uncovered_dimensions(self) -> Vec<DimensionIndex>;
}
```

---

## Public API

### Type Constructors (All `#[must_use]`)

```rust
// ModuleId -- prefer named constants
let id = ModuleId::M01;
let custom = ModuleId::new("M99");

// AgentId -- prefix-based factory methods
let sys = AgentId::system();           // "sys:system"
let human = AgentId::human();          // "human:@0.A"
let svc = AgentId::service("synthex"); // "svc:synthex"
let agent = AgentId::agent("a-001");   // "agent:a-001"

// Timestamp -- monotonic ordering
let ts = Timestamp::now();             // strictly increasing
let ts2 = Timestamp::from_raw(42);     // for testing/replay

// HealthReport -- clamped construction
let report = HealthReport::new(ModuleId::M04, 0.85)
    .with_details("all subsystems nominal");

// CoverageBitmap -- builder-style
let coverage = CoverageBitmap::EMPTY
    .with_dimension(DimensionIndex::HealthScore)
    .with_dimension(DimensionIndex::Uptime);
```

### Trait Implementations

| Type | Traits |
|------|--------|
| `ModuleId` | `Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Display, AsRef<str>` |
| `AgentId` | `Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Display, AsRef<str>, Into<String>` |
| `Timestamp` | `Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Display, Default` |
| `HealthReport` | `Debug, Clone, PartialEq, Display` |
| `DimensionIndex` | `Debug, Clone, Copy, PartialEq, Eq, Hash, Display` |
| `CoverageBitmap` | `Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Display` |

---

## Configuration

This module has no configuration -- all types are defined at compile time. The only runtime state is the `GLOBAL_TICK` atomic counter backing `Timestamp::now()`.

---

## Metrics

| Metric | Type | Description |
|--------|------|-------------|
| N/A | -- | M00 emits no metrics (pure types, zero I/O) |

---

## Error Codes

| Code | Name | Severity | Description | Recovery |
|------|------|----------|-------------|----------|
| N/A | -- | -- | M00 produces no errors (infallible constructors) | -- |

---

## Design Invariants

| Invariant | Enforcement |
|-----------|-------------|
| No `unsafe` code | `#![forbid(unsafe_code)]` at crate root |
| No panics | No `.unwrap()`, `.expect()`, `panic!()` |
| No I/O | Module imports only `std::fmt` and `std::sync::atomic` |
| All `Send + Sync` | Verified by compile-time tests |
| Health score clamped | `clamp(0.0, 1.0)` in `HealthReport::new()` |
| Coverage masked to 12 bits | `& 0x0FFF` in all `CoverageBitmap` operations |
| Timestamp monotonic | `AtomicU64::fetch_add(1, Relaxed)` in `Timestamp::now()` |

---

## Related Modules

- **M07_SIGNALS**: Consumes `ModuleId`, `Timestamp` for signal emission context
- **M08_TENSOR_REGISTRY**: Consumes `DimensionIndex`, `CoverageBitmap` for tensor composition
- **M01_ERROR_TAXONOMY**: Uses `ModuleId` in error context
- **M04_METRICS_COLLECTOR**: Uses `Timestamp` for metric timestamps
- **M05_STATE_PERSISTENCE**: Uses `ModuleId`, `Timestamp` for state snapshots
- **L2 Services (M09-M12)**: Use `Timestamp`, `HealthReport`, `DimensionIndex` throughout
- **L5 Learning (M25-M30)**: Use `Timestamp` for STDP timing windows
- **L6 Consensus (M31-M36)**: Use `AgentId`, `ModuleId` for vote tracking

---

## Navigation

| Direction | Link |
|-----------|------|
| Up | [INDEX.md](../INDEX.md) |
| Layer | [L01_FOUNDATION.md](../layers/L01_FOUNDATION.md) |
| Next | [M01_ERROR_TAXONOMY.md](M01_ERROR_TAXONOMY.md) |
| Related | [M07_SIGNALS.md](M07_SIGNALS.md) |
| Related | [M08_TENSOR_REGISTRY.md](M08_TENSOR_REGISTRY.md) |

---

*[Back to Index](../INDEX.md) | [Layer: L01 Foundation](../layers/L01_FOUNDATION.md)*
