# Rust Coding Patterns Reference

> Claude Code & CodeSynthor V7 Optimized Pattern Library
> Derived from SYNTHEX, The Maintenance Engine, and CodeSynthor V7

**Version:** 1.1.0
**Parent Index:** [../INDEX.md](../INDEX.md)
**Related Specs:** [TENSOR_SPEC](../TENSOR_SPEC.md), [STDP_SPEC](../STDP_SPEC.md), [PBFT_SPEC](../PBFT_SPEC.md)

---

## Overview

| Metric | Value |
|--------|-------|
| **Pattern Files** | 12 |
| **Patterns Documented** | 120+ |
| **Antipatterns** | 40+ |
| **Source Codebases** | 3 |
| **Total LOC Analyzed** | 100K+ |

---

## Pattern Categories

| Pattern File | Focus | Patterns | Priority |
|--------------|-------|----------|----------|
| [RUST_CORE_PATTERNS.md](RUST_CORE_PATTERNS.md) | Core Rust idioms | 25 | P0 |
| [ERROR_PATTERNS.md](ERROR_PATTERNS.md) | Error handling | 15 | P0 |
| [CONCURRENCY_PATTERNS.md](CONCURRENCY_PATTERNS.md) | Thread-safe code | 12 | P0 |
| [DATABASE_PATTERNS.md](DATABASE_PATTERNS.md) | SQLite access | 14 | P1 |
| [MODULE_PATTERNS.md](MODULE_PATTERNS.md) | Code organization | 10 | P1 |
| [TENSOR_PATTERNS.md](TENSOR_PATTERNS.md) | 12D encoding | 8 | P1 |
| [LEARNING_PATTERNS.md](LEARNING_PATTERNS.md) | Hebbian/STDP | 10 | P2 |
| [CONSENSUS_PATTERNS.md](CONSENSUS_PATTERNS.md) | PBFT patterns | 8 | P2 |
| [INTEGRATION_PATTERNS.md](INTEGRATION_PATTERNS.md) | Cross-system | 12 | P2 |
| [ANTIPATTERNS.md](ANTIPATTERNS.md) | What to avoid | 40+ | P0 |

---

## Quick Reference - Top 10 Patterns

### 1. Result<T> Error Handling (P0)
```rust
// ALWAYS use Result<T>, NEVER panic
pub type Result<T> = std::result::Result<T, Error>;

pub fn operation() -> Result<T> {
    let value = fallible_op()?;  // Propagate with ?
    Ok(value)
}
```

### 2. Lock-Free Concurrency (P0)
```rust
use dashmap::DashMap;

pub struct Registry<T: Clone> {
    items: Arc<DashMap<String, T>>,
}
```

### 3. Structured Error Types (P0)
```rust
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Database error: {0}")]
    Database(String),

    #[error("Network error connecting to {target}: {message}")]
    Network { target: String, message: String },
}
```

### 4. Circuit Breaker (P1)
```rust
pub enum CircuitState { Closed, Open, HalfOpen }

impl CircuitBreaker {
    pub fn allow_request(&self) -> bool {
        match self.state {
            CircuitState::Closed => true,
            CircuitState::Open => false,
            CircuitState::HalfOpen => self.test_count < self.test_limit,
        }
    }
}
```

### 5. 12D Tensor Encoding (P1)
```rust
#[derive(Clone, Copy, Debug, Default)]
pub struct Tensor12D {
    pub d0_service_id: f64,  // Normalized [0,1]
    // ... 11 more dimensions
}

impl Tensor12D {
    pub fn validate(&self) -> Result<()> { /* bounds check */ }
    pub fn distance(&self, other: &Self) -> f64 { /* euclidean */ }
}
```

### 6. Multi-Tier Cache (P1)
```rust
pub struct TieredCache<K, V> {
    l1: Arc<DashMap<K, V>>,      // Hot: 1K items, 5s TTL
    l2: Arc<DashMap<K, V>>,      // Warm: 10K items, 30s TTL
    l3: Arc<SqlitePool>,         // Cold: Persistent
}
```

### 7. Trait-Based Plugins (P1)
```rust
#[async_trait]
pub trait Plugin: Send + Sync {
    fn id(&self) -> &str;
    async fn execute(&self, ctx: &Context) -> Result<Output>;
}
```

### 8. Feature-Gated Modules (P2)
```rust
#[cfg(feature = "api")]
pub mod rest_server;

#[cfg(feature = "consensus")]
pub mod pbft;
```

### 9. Hebbian Pathway (P2)
```rust
pub struct HebbianPathway {
    pub strength: f64,  // [0.1, 1.0]
    pub ltp: f64,       // Long-term potentiation
    pub ltd: f64,       // Long-term depression
}

impl HebbianPathway {
    pub fn apply_stdp(&mut self, delta_t: i64) {
        if delta_t > 0 { self.apply_ltp(); }
        else { self.apply_ltd(); }
    }
}
```

### 10. PBFT Consensus (P2)
```rust
pub const PBFT_N: u32 = 40;  // Total agents
pub const PBFT_F: u32 = 13;  // Byzantine tolerance
pub const PBFT_Q: u32 = 27;  // Quorum (2f + 1)

pub fn has_quorum(votes: u32) -> bool {
    votes >= PBFT_Q
}
```

---

## Strict Quality Standards

### Required Cargo.toml Settings
```toml
[workspace.lints.rust]
unsafe_code = "forbid"

[workspace.lints.clippy]
unwrap_used = "deny"
expect_used = "deny"
panic = "deny"
pedantic = "warn"

[profile.release]
lto = true
codegen-units = 1
panic = "abort"
strip = true
```

### Required lib.rs Attributes
```rust
#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![warn(clippy::pedantic)]
#![warn(missing_docs)]
```

---

## Source Codebases

| Codebase | LOC | Modules | Key Patterns |
|----------|-----|---------|--------------|
| SYNTHEX | 56K+ | 45 | Lock-free, Error taxonomy, Multi-DB |
| The Maintenance Engine | 30K+ | 36 | 12D Tensor, PBFT, Hebbian |
| CodeSynthor V7 | 20K+ | 62 | 11D Tensor, Enterprise gateway |

---

## Navigation

| Need | Go To |
|------|-------|
| Core Rust patterns | [RUST_CORE_PATTERNS.md](RUST_CORE_PATTERNS.md) |
| Error handling | [ERROR_PATTERNS.md](ERROR_PATTERNS.md) |
| Thread safety | [CONCURRENCY_PATTERNS.md](CONCURRENCY_PATTERNS.md) |
| Database access | [DATABASE_PATTERNS.md](DATABASE_PATTERNS.md) |
| Module organization | [MODULE_PATTERNS.md](MODULE_PATTERNS.md) |
| Tensor encoding | [TENSOR_PATTERNS.md](TENSOR_PATTERNS.md) |
| Learning systems | [LEARNING_PATTERNS.md](LEARNING_PATTERNS.md) |
| Consensus | [CONSENSUS_PATTERNS.md](CONSENSUS_PATTERNS.md) |
| Cross-system | [INTEGRATION_PATTERNS.md](INTEGRATION_PATTERNS.md) |
| What to avoid | [ANTIPATTERNS.md](ANTIPATTERNS.md) |

---

## Cross-Reference to Specs

| Pattern File | Primary Spec | Secondary Specs |
|--------------|-------------|-----------------|
| TENSOR_PATTERNS.md | [TENSOR_SPEC.md](../TENSOR_SPEC.md) | DATABASE_SPEC |
| LEARNING_PATTERNS.md | [STDP_SPEC.md](../STDP_SPEC.md) | NAM_SPEC, PIPELINE_SPEC |
| CONSENSUS_PATTERNS.md | [PBFT_SPEC.md](../PBFT_SPEC.md) | NAM_SPEC, ESCALATION_SPEC |
| DATABASE_PATTERNS.md | [DATABASE_SPEC.md](../DATABASE_SPEC.md) | MODULE_MATRIX |
| ERROR_PATTERNS.md | [API_SPEC.md](../API_SPEC.md) | ESCALATION_SPEC |
| INTEGRATION_PATTERNS.md | [SERVICE_SPEC.md](../SERVICE_SPEC.md) | PIPELINE_SPEC |

---

## Back to Main Index

- [AI Specs Index](../INDEX.md)
- [AI Docs Index](../../ai_docs/INDEX.md)
- [README](../../README.md)

---

## Version History

| Version | Date | Changes |
|---------|------|---------|
| 1.1.0 | 2026-01-28 | Added spec cross-references, back navigation |
| 1.0.0 | 2026-01-28 | Initial pattern index |

---

*Generated: 2026-01-28 | The Maintenance Engine v1.0.0*
