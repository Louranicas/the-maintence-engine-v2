# M00 Vocabulary Types — shared_types.rs

> **File:** `src/m1_foundation/shared_types.rs` | **LOC:** ~1,210 | **Tests:** ~88
> **Role:** Foundation vocabulary types consumed by every layer

---

## Types at a Glance

| Type | Kind | Copy | Hash | Const | Purpose |
|------|------|------|------|-------|---------|
| `ModuleId` | newtype(`&'static str`) | Yes | Yes | Yes | 42 module identifiers M01-M42 |
| `AgentId` | newtype(`String`) | No | Yes | No | Prefixed agent identifiers (sys/human/svc/agent) |
| `Timestamp` | newtype(`u64`) | Yes | Yes | Yes | Monotonic atomic tick counter |
| `HealthReport` | struct | No | No | — | Module health snapshot with score + details |
| `DimensionIndex` | enum(`#[repr(u8)]`) | Yes | Yes | Yes | 12 tensor dimensions D0-D11 |
| `CoverageBitmap` | newtype(`u16`) | Yes | Yes | Yes | 12-bit dimension coverage mask |

---

## ModuleId

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ModuleId(&'static str);
```

**42 constants:** `M01`..`M42` + `ALL: [Self; 42]`

| Method | Signature | Notes |
|--------|-----------|-------|
| `new` | `const fn(id: &'static str) -> Self` | #[must_use] |
| `as_str` | `const fn(&self) -> &'static str` | #[must_use] |
| `number` | `fn(&self) -> Option<u8>` | Parses "M{N}" → Some(N), else None |
| `layer` | `fn(&self) -> Option<u8>` | M01-06→L1, M07-12→L2, ..., M37-42→L7 |

**Traits:** `Display` ("M04"), `AsRef<str>`

---

## AgentId

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AgentId(String);
```

| Factory | Result | Prefix |
|---------|--------|--------|
| `system()` | `"sys:system"` | `sys:` |
| `human()` | `"human:@0.A"` | `human:` |
| `service(id)` | `"svc:{id}"` | `svc:` |
| `agent(id)` | `"agent:{id}"` | `agent:` |
| `from_raw(s)` | raw string | unchecked |

**Query methods:** `is_system()`, `is_human()`, `is_service()`, `is_agent()`, `prefix()`, `as_str()`

**Traits:** `Display`, `AsRef<str>`, `From<AgentId> for String`

---

## Timestamp

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Timestamp(u64);
```

**Global state:** `static GLOBAL_TICK: AtomicU64` — incremented with `Ordering::Relaxed`

| Method | Signature | Notes |
|--------|-----------|-------|
| `now` | `fn() -> Self` | Atomic fetch_add(1) — strictly increasing, never repeats |
| `from_raw` | `const fn(ticks: u64) -> Self` | For testing/deserialization |
| `ticks` | `const fn(&self) -> u64` | Raw value |
| `elapsed_since` | `const fn(&self, earlier: Self) -> u64` | Saturating subtraction |
| `within_window` | `const fn(&self, other: Self, window: u64) -> bool` | Symmetric abs_diff <= window |

**Constants:** `ZERO = Timestamp(0)`

**Traits:** `Display` ("T999"), `Default` (ZERO)

---

## HealthReport

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct HealthReport {
    pub module_id: ModuleId,
    pub health_score: f64,        // clamped [0.0, 1.0] at construction
    pub timestamp: Timestamp,
    pub details: Option<String>,
}
```

| Method | Notes |
|--------|-------|
| `new(module_id, health_score)` | Clamps score, sets timestamp=now() |
| `with_details(impl Into<String>)` | Builder chain |
| `with_timestamp(Timestamp)` | const fn, for testing |
| `is_healthy()` | score >= 0.5 |
| `is_critical()` | score < 0.2 |

**Traits:** `Display` ("Health(M04: 0.950 at T123)")

---

## DimensionIndex

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum DimensionIndex {
    ServiceId = 0, Port = 1, Tier = 2, DependencyCount = 3,
    AgentCount = 4, Protocol = 5, HealthScore = 6, Uptime = 7,
    Synergy = 8, Latency = 9, ErrorRate = 10, TemporalContext = 11,
}
```

| Method | Signature | Notes |
|--------|-----------|-------|
| `index` | `const fn(self) -> usize` | 0..=11 |
| `name` | `const fn(self) -> &'static str` | e.g. "health_score" |
| `from_index` | `const fn(usize) -> Option<Self>` | None if >= 12 |
| `from_name` | `fn(&str) -> Option<Self>` | None if unknown |

**Constants:** `ALL: [Self; 12]`

**Traits:** `Display` ("D6:health_score")

---

## CoverageBitmap

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct CoverageBitmap(u16);  // bottom 12 bits only
```

| Method | Signature | Notes |
|--------|-----------|-------|
| `from_raw` | `const fn(bits: u16) -> Self` | Masked to 12 bits |
| `with_dimension` | `const fn(self, dim) -> Self` | Sets bit, chainable |
| `is_covered` | `const fn(self, dim) -> bool` | Bit test |
| `count` | `const fn(self) -> u32` | Popcount |
| `union` | `const fn(self, other) -> Self` | Bitwise OR |
| `intersection` | `const fn(self, other) -> Self` | Bitwise AND |
| `coverage_ratio` | `fn(self) -> f64` | count/12 |
| `covered_dimensions` | `fn(self) -> Vec<DimensionIndex>` | |
| `uncovered_dimensions` | `fn(self) -> Vec<DimensionIndex>` | |

**Constants:** `EMPTY = CoverageBitmap(0)`, `FULL = CoverageBitmap(0x0FFF)`

**Traits:** `Display` ("Coverage(4/12 = 33%)"), `Default` (EMPTY)

---

## Design Notes

- All 6 types are `#[must_use]` on every public method (44+ annotations)
- 18 `const fn` methods in this file alone
- `Timestamp::now()` uses `Relaxed` ordering — sufficient for monotonic tick counter, no sequential consistency needed
- `CoverageBitmap` is purely functional — every method returns a new value, enabling `const fn` composition

---

*M00 Vocabulary Types Spec v1.0 | 2026-03-01*
