# Module 25: Hebbian Manager

> **M25_HEBBIAN_MANAGER** | Layer 5: Learning | [Back to Index](INDEX.md)

## Overview

The Hebbian Manager is responsible for creating, maintaining, and evolving neural pathways between system components using Hebbian learning principles. It manages the creation and lifecycle of pathway connections that strengthen when simultaneously active and weaken through disuse.

## Layer Context

| Property | Value |
|----------|-------|
| Layer | L5: Learning |
| Module ID | M25 |
| Source | `src/m5_learning/mod.rs` |
| Purpose | Pathway management and Hebbian learning |
| Type | Core learning engine |

## STDP Configuration

| Parameter | Value | Description |
|-----------|-------|-------------|
| LTP Rate | 0.1 | Long-Term Potentiation increase |
| LTD Rate | 0.05 | Long-Term Depression decrease |
| STDP Window | 100ms | Timing window for coincidence detection |
| Decay Rate | 0.001 | Pathway strength decay per cycle |
| Min Strength | 0.1 | Minimum threshold before pruning |

## Core Types

### HebbianPathway

```rust
pub struct HebbianPathway {
    pub id: String,                                    // Unique pathway ID
    pub source: String,                                // Source module/service
    pub target: String,                                // Target module/service
    pub strength: f64,                                 // Pathway strength (0.0 - 1.0)
    pub pathway_type: PathwayType,                     // Classification of pathway
    pub ltp_count: u64,                                // Long-Term Potentiation events
    pub ltd_count: u64,                                // Long-Term Depression events
    pub activation_count: u64,                         // Total activation count
    pub stdp_delta: f64,                               // Accumulated timing-based changes
    pub success_count: u64,                            // Successful activations
    pub failure_count: u64,                            // Failed activations
    pub last_activation: Option<SystemTime>,           // Last activation timestamp
    pub last_success: Option<SystemTime>,              // Last success timestamp
}
```

### PathwayType

```rust
pub enum PathwayType {
    ServiceToService,     // Service to service communication
    AgentToAgent,         // Agent to agent collaboration
    SystemToSystem,       // System to system integration
    PatternToOutcome,     // Pattern to outcome association
    ConfigToBehavior,     // Configuration to behavior mapping
    MetricToAction,       // Metric to action trigger
}
```

## API

### Pathway Creation

```rust
// Create a new pathway
pub fn new(source: impl Into<String>, target: impl Into<String>) -> HebbianPathway

// Example
let pathway = HebbianPathway::new("maintenance", "service_restart");
```

### Pathway Analysis

```rust
// Calculate success rate (success / total activations)
pub fn success_rate(&self) -> f64
// Range: 0.0 - 1.0, default 0.5 for new pathways

// Calculate routing weight (strength * success rate)
pub fn routing_weight(&self) -> f64
// Used for determining pathway priority in routing decisions
```

### Pathway Modification

```rust
// Apply LTP (strengthening) - called on successful activation
pub fn apply_ltp(&mut self, config: &StdpConfig)
// Increases strength by ltp_rate (capped at 1.0)

// Apply LTD (weakening) - called on failed activation
pub fn apply_ltd(&mut self, config: &StdpConfig)
// Decreases strength by ltd_rate (minimum 0.1)

// Record successful activation
pub fn record_success(&mut self, config: &StdpConfig)
// Increments success_count, activation_count, applies LTP

// Record failed activation
pub fn record_failure(&mut self, config: &StdpConfig)
// Increments failure_count, activation_count, applies LTD
```

### Pathway Pruning

```rust
// Determine if pathway should be removed
pub fn should_prune(&self, min_strength: f64, inactive_days: u64) -> bool
// Returns true if:
// - Strength < min_strength AND
// - Last successful activation > inactive_days ago
// - Used to clean up weak, unused pathways
```

## Default Pathways

The system initializes with 9 default pathways:

1. **maintenance → service_restart** - Restart services on issues
2. **maintenance → database_vacuum** - Database maintenance
3. **maintenance → cache_cleanup** - Cache invalidation
4. **maintenance → session_rotation** - Session management
5. **health_failure → service_restart** - Health-based restart
6. **latency_spike → cache_cleanup** - Latency response
7. **memory_pressure → session_rotation** - Memory management
8. **consensus_proposal → agent_vote** - Voting pathway
9. **dissent_detected → learning_update** - Learning updates

## Pathway Lifecycle

```
New Pathway (strength: 0.5)
    ↓
Activation → Success → LTP (strength increases) → Stronger pathway
           ↓ Failure → LTD (strength decreases) → Weaker pathway
                           ↓
                    Pruning Eligible (strength < 0.1 + age > N days)
                           ↓
                      Removed from system
```

## Related Modules

- **M26_STDP_PROCESSOR** - Implements spike-timing dependent plasticity
- **M27_PATTERN_RECOGNIZER** - Detects patterns that trigger pathways
- **M28_PATHWAY_PRUNER** - Removes weak pathways
- **M29_MEMORY_CONSOLIDATOR** - Consolidates pathways across memory layers
- **M30_ANTIPATTERN_DETECTOR** - Identifies negative pathway associations

## Integration Points

### With M26 (STDP Processor)
Hebbian Manager uses STDP configuration to apply LTP/LTD updates with precise timing windows.

### With M28 (Pathway Pruner)
Weak pathways identified by `should_prune()` are candidates for removal by the pruner.

### With Service Mesh (M12)
Pathways inform routing weights in the service mesh controller.

### With Learning System
Pathways represent learned associations between system states and effective actions.

---

*[Back to Index](INDEX.md)*
