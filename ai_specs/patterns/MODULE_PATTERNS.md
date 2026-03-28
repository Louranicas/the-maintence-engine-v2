# Module Organization Patterns Reference

> Code Structure Patterns for Claude Code & CodeSynthor V7
> Derived from SYNTHEX, The Maintenance Engine, and CodeSynthor V7

---

## Overview

| Metric | Value |
|--------|-------|
| **Patterns** | 10 |
| **Priority** | P1 |
| **Source Modules** | 143 modules across 3 codebases |

---

## Pattern 1: Layered Architecture (P0)

```
the_maintenance_engine/
├── Cargo.toml
├── src/
│   ├── lib.rs                 # Public API exports
│   ├── main.rs                # Binary entry point
│   │
│   ├── l1_foundation/         # Layer 1: Core infrastructure
│   │   ├── mod.rs
│   │   ├── m01_service_registry/
│   │   ├── m02_config_loader/
│   │   ├── m03_logging/
│   │   ├── m04_error/
│   │   ├── m05_types/
│   │   └── m06_traits/
│   │
│   ├── l2_services/           # Layer 2: Service management
│   │   ├── mod.rs
│   │   ├── m07_health_monitor/
│   │   ├── m08_dependency_graph/
│   │   ├── m09_lifecycle/
│   │   ├── m10_discovery/
│   │   ├── m11_routing/
│   │   └── m12_cache/
│   │
│   ├── l3_learning/           # Layer 3: Intelligence
│   │   ├── mod.rs
│   │   ├── m13_hebbian/
│   │   ├── m14_stdp/
│   │   ├── m15_pathway/
│   │   ├── m16_memory/
│   │   ├── m17_prediction/
│   │   └── m18_adaptation/
│   │
│   ├── l4_consensus/          # Layer 4: Distributed agreement
│   │   ├── mod.rs
│   │   ├── m19_pbft/
│   │   ├── m20_voting/
│   │   ├── m21_quorum/
│   │   ├── m22_agents/
│   │   ├── m23_messages/
│   │   └── m24_state_machine/
│   │
│   ├── l5_remediation/        # Layer 5: Self-healing
│   │   ├── mod.rs
│   │   ├── m25_detector/
│   │   ├── m26_analyzer/
│   │   ├── m27_planner/
│   │   ├── m28_executor/
│   │   ├── m29_validator/
│   │   └── m30_rollback/
│   │
│   └── l6_integration/        # Layer 6: External interfaces
│       ├── mod.rs
│       ├── m31_api/
│       ├── m32_cli/
│       ├── m33_events/
│       ├── m34_metrics/
│       ├── m35_plugins/
│       └── m36_orchestration/
│
├── migrations/                # Database schemas
├── tests/                     # Integration tests
└── ai_docs/                   # AI-readable documentation
```

**Why**: Layered architecture enforces dependency direction and separation of concerns.

---

## Pattern 2: Module Structure (P0)

```rust
// src/l2_services/m07_health_monitor/mod.rs

//! Health monitoring module for service liveness and readiness.
//!
//! # Overview
//! Provides continuous health assessment of registered services.
//!
//! # Components
//! - [`HealthMonitor`]: Main monitoring coordinator
//! - [`HealthCheck`]: Individual check definitions
//! - [`HealthStatus`]: Check result representations

mod health_monitor;
mod health_check;
mod health_status;
mod config;

// Re-export public API
pub use health_monitor::HealthMonitor;
pub use health_check::{HealthCheck, CheckType, CheckResult};
pub use health_status::{HealthStatus, StatusLevel};
pub use config::HealthConfig;

// Internal-only items stay private
use config::InternalConfig;

#[cfg(test)]
mod tests;
```

**Why**: Clear public API with hidden implementation details.

---

## Pattern 3: Feature-Gated Modules (P0)

```rust
// src/lib.rs

#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used)]

// Core layers always available
pub mod l1_foundation;
pub mod l2_services;

// Optional layers behind feature flags
#[cfg(feature = "learning")]
pub mod l3_learning;

#[cfg(feature = "consensus")]
pub mod l4_consensus;

#[cfg(feature = "remediation")]
pub mod l5_remediation;

#[cfg(feature = "api")]
pub mod l6_integration;

// Cargo.toml
// [features]
// default = ["learning", "consensus"]
// full = ["learning", "consensus", "remediation", "api"]
// learning = []
// consensus = ["learning"]  # consensus requires learning
// remediation = ["consensus"]
// api = ["remediation"]
```

**Why**: Feature flags enable minimal builds and optional functionality.

---

## Pattern 4: Prelude Pattern (P1)

```rust
// src/prelude.rs

//! Commonly used items re-exported for convenience.
//!
//! ```rust
//! use the_maintenance_engine::prelude::*;
//! ```

// Core types
pub use crate::l1_foundation::m04_error::{Error, Result};
pub use crate::l1_foundation::m05_types::{ServiceId, Tensor12D};
pub use crate::l1_foundation::m06_traits::{Plugin, Validator, Executor};

// Common traits
pub use async_trait::async_trait;
pub use tracing::{debug, error, info, trace, warn};

// Standard library essentials
pub use std::sync::Arc;
pub use std::collections::HashMap;

// src/lib.rs
pub mod prelude;
```

**Why**: Prelude reduces boilerplate in module imports.

---

## Pattern 5: Internal Module (P1)

```rust
// src/l2_services/m07_health_monitor/internal.rs

//! Internal implementation details not exposed in public API.

use super::*;

/// Internal worker that runs health checks
pub(super) struct HealthWorker {
    config: Arc<HealthConfig>,
    checks: Vec<HealthCheck>,
    results: Arc<DashMap<ServiceId, HealthStatus>>,
}

impl HealthWorker {
    pub(super) fn new(config: Arc<HealthConfig>) -> Self {
        Self {
            config,
            checks: Vec::new(),
            results: Arc::new(DashMap::new()),
        }
    }

    pub(super) async fn run_checks(&self) -> Vec<CheckResult> {
        // Implementation hidden from external users
        let mut results = Vec::new();
        for check in &self.checks {
            results.push(check.execute().await);
        }
        results
    }
}

// src/l2_services/m07_health_monitor/health_monitor.rs
mod internal;
use internal::HealthWorker;

pub struct HealthMonitor {
    worker: HealthWorker,  // Internal type, not exposed
}
```

**Why**: Internal modules hide implementation complexity.

---

## Pattern 6: Cross-Layer Communication (P1)

```rust
// Events flow up, commands flow down
// Use message passing between layers

// src/l1_foundation/m06_traits/events.rs
pub trait EventEmitter {
    fn emit(&self, event: Event);
    fn subscribe(&self, handler: Box<dyn EventHandler>);
}

pub trait EventHandler: Send + Sync {
    fn handle(&self, event: &Event);
    fn event_types(&self) -> &[EventType];
}

// src/l2_services/m07_health_monitor/health_monitor.rs
impl EventEmitter for HealthMonitor {
    fn emit(&self, event: Event) {
        // Emit to L3 learning layer
        self.event_bus.publish(event);
    }
}

// src/l3_learning/m13_hebbian/learner.rs
impl EventHandler for HebbianLearner {
    fn handle(&self, event: &Event) {
        match event {
            Event::ServiceHealthChanged { .. } => {
                self.update_pathways(event);
            }
            _ => {}
        }
    }

    fn event_types(&self) -> &[EventType] {
        &[EventType::ServiceHealthChanged]
    }
}
```

**Why**: Loose coupling between layers via events enables independent evolution.

---

## Pattern 7: Module Dependencies (P1)

```rust
// Cargo.toml workspace layout for large projects

// Root Cargo.toml
[workspace]
members = [
    "crates/core",
    "crates/services",
    "crates/learning",
    "crates/consensus",
    "crates/api",
]

// crates/core/Cargo.toml
[package]
name = "maintenance-core"
version = "0.1.0"

[dependencies]
thiserror = "1.0"
tracing = "0.1"

// crates/services/Cargo.toml
[package]
name = "maintenance-services"
version = "0.1.0"

[dependencies]
maintenance-core = { path = "../core" }
tokio = { version = "1.0", features = ["full"] }
dashmap = "5.5"

// crates/learning/Cargo.toml
[package]
name = "maintenance-learning"
version = "0.1.0"

[dependencies]
maintenance-core = { path = "../core" }
maintenance-services = { path = "../services" }
```

**Why**: Workspace layout enables parallel compilation and clear dependencies.

---

## Pattern 8: Test Organization (P1)

```rust
// src/l2_services/m07_health_monitor/tests.rs

#[cfg(test)]
mod tests {
    use super::*;

    // Unit tests in same file
    mod health_check_tests {
        use super::*;

        #[test]
        fn test_health_check_creation() {
            let check = HealthCheck::new("test", CheckType::Http);
            assert_eq!(check.name(), "test");
        }

        #[tokio::test]
        async fn test_health_check_execution() {
            let check = HealthCheck::mock_success();
            let result = check.execute().await;
            assert!(result.is_healthy());
        }
    }

    mod health_monitor_tests {
        use super::*;

        fn create_test_monitor() -> HealthMonitor {
            HealthMonitor::new(HealthConfig::default())
        }

        #[tokio::test]
        async fn test_monitor_start_stop() {
            let monitor = create_test_monitor();
            monitor.start().await.unwrap();
            assert!(monitor.is_running());
            monitor.stop().await.unwrap();
            assert!(!monitor.is_running());
        }
    }
}

// tests/integration/health_monitoring.rs (integration tests)
use the_maintenance_engine::prelude::*;
use the_maintenance_engine::l2_services::m07_health_monitor::*;

#[tokio::test]
async fn test_health_monitoring_integration() {
    // Full integration test with real components
}
```

**Why**: Organized tests enable fast feedback and comprehensive coverage.

---

## Pattern 9: Documentation Module (P2)

```rust
// src/l1_foundation/mod.rs

//! # Layer 1: Foundation
//!
//! Core infrastructure components that all other layers depend on.
//!
//! ## Modules
//!
//! | Module | Purpose | Priority |
//! |--------|---------|----------|
//! | [`m01_service_registry`] | Service registration and lookup | P0 |
//! | [`m02_config_loader`] | Configuration management | P0 |
//! | [`m03_logging`] | Structured logging | P0 |
//! | [`m04_error`] | Error types and handling | P0 |
//! | [`m05_types`] | Core type definitions | P0 |
//! | [`m06_traits`] | Shared trait definitions | P0 |
//!
//! ## Dependencies
//!
//! ```text
//! L1 Foundation has no internal layer dependencies.
//! All other layers depend on L1.
//! ```
//!
//! ## Example
//!
//! ```rust
//! use the_maintenance_engine::l1_foundation::prelude::*;
//!
//! let registry = ServiceRegistry::new();
//! registry.register(Service::new("api", 8080))?;
//! ```

pub mod m01_service_registry;
pub mod m02_config_loader;
pub mod m03_logging;
pub mod m04_error;
pub mod m05_types;
pub mod m06_traits;

/// Prelude for L1 types
pub mod prelude {
    pub use super::m04_error::{Error, Result};
    pub use super::m05_types::*;
    pub use super::m06_traits::*;
}
```

**Why**: Module-level documentation helps navigation and understanding.

---

## Pattern 10: Startup Sequence Module (P1)

```rust
// src/startup.rs

//! Application startup and initialization sequence.

use crate::prelude::*;
use crate::l1_foundation::*;
use crate::l2_services::*;

#[cfg(feature = "learning")]
use crate::l3_learning::*;

#[cfg(feature = "consensus")]
use crate::l4_consensus::*;

pub struct Application {
    config: Arc<Config>,
    registry: Arc<ServiceRegistry>,
    health_monitor: Arc<HealthMonitor>,
    #[cfg(feature = "learning")]
    learner: Arc<HebbianLearner>,
    #[cfg(feature = "consensus")]
    consensus: Arc<PbftEngine>,
}

impl Application {
    /// Initialize application with dependency injection
    pub async fn new(config: Config) -> Result<Self> {
        let config = Arc::new(config);

        // Phase 1: Foundation
        tracing::info!("Starting L1 Foundation...");
        let registry = Arc::new(ServiceRegistry::new());

        // Phase 2: Services
        tracing::info!("Starting L2 Services...");
        let health_monitor = Arc::new(
            HealthMonitor::new(config.health.clone())
        );

        // Phase 3: Learning (if enabled)
        #[cfg(feature = "learning")]
        let learner = {
            tracing::info!("Starting L3 Learning...");
            Arc::new(HebbianLearner::new(config.learning.clone()))
        };

        // Phase 4: Consensus (if enabled)
        #[cfg(feature = "consensus")]
        let consensus = {
            tracing::info!("Starting L4 Consensus...");
            Arc::new(PbftEngine::new(config.consensus.clone()))
        };

        Ok(Self {
            config,
            registry,
            health_monitor,
            #[cfg(feature = "learning")]
            learner,
            #[cfg(feature = "consensus")]
            consensus,
        })
    }

    /// Start all components
    pub async fn start(&self) -> Result<()> {
        self.health_monitor.start().await?;

        #[cfg(feature = "learning")]
        self.learner.start().await?;

        #[cfg(feature = "consensus")]
        self.consensus.start().await?;

        tracing::info!("Application started successfully");
        Ok(())
    }

    /// Graceful shutdown
    pub async fn shutdown(&self) -> Result<()> {
        tracing::info!("Shutting down...");

        #[cfg(feature = "consensus")]
        self.consensus.stop().await?;

        #[cfg(feature = "learning")]
        self.learner.stop().await?;

        self.health_monitor.stop().await?;

        tracing::info!("Shutdown complete");
        Ok(())
    }
}
```

**Why**: Structured startup ensures correct initialization order.

---

## Module Naming Conventions

| Pattern | Example | Usage |
|---------|---------|-------|
| `l{n}_layer_name` | `l1_foundation` | Layer directories |
| `m{nn}_module_name` | `m07_health_monitor` | Module directories |
| `snake_case.rs` | `health_check.rs` | File names |
| `PascalCase` | `HealthMonitor` | Type names |
| `SCREAMING_SNAKE` | `MAX_RETRIES` | Constants |
| `snake_case` | `check_health` | Function names |

---

## Module Size Guidelines

| Category | Lines | Files | Action |
|----------|-------|-------|--------|
| Small | <200 | 1 | Single file is fine |
| Medium | 200-500 | 2-5 | Split by responsibility |
| Large | 500-1000 | 5-10 | Consider sub-modules |
| Too Large | >1000 | >10 | Must refactor |

---

*Generated: 2026-01-28 | The Maintenance Engine v1.0.0*
