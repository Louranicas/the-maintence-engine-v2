# Layer 5: Learning

> **L05_LEARNING** | Hebbian Learning Layer | [Back to Index](../INDEX.md)

---

## Navigation

| Direction | Link |
|-----------|------|
| Up | [INDEX.md](../INDEX.md) |
| Previous | [L04_INTEGRATION.md](L04_INTEGRATION.md) |
| Next | [L06_CONSENSUS.md](L06_CONSENSUS.md) |
| Related | [STDP_SPEC.md](../../ai_specs/STDP_SPEC.md) |

---

## Layer Overview

The Learning Layer (L5) implements Hebbian learning principles for adaptive system behavior. Using Spike-Timing-Dependent Plasticity (STDP), it strengthens or weakens neural pathways based on temporal correlation of events, enabling the system to learn optimal response patterns over time.

### Layer Properties

| Property | Value |
|----------|-------|
| Layer ID | L5 |
| Layer Name | Learning |
| Source Directory | `src/m5_learning/` |
| Dependencies | L1-L4 |
| Dependents | L6 |
| Modules | M25-M30 |
| Learning Model | Hebbian with STDP |
| Primary Functions | STDP, Pathway Management, Pattern Learning |

---

## Architecture

```
+------------------------------------------------------------------+
|                      L5: Learning Layer                            |
+------------------------------------------------------------------+
|                                                                  |
|  +---------------------------+  +---------------------------+    |
|  |     Hebbian Engine        |  |    STDP Engine            |    |
|  |                           |  |                           |    |
|  |  - Pathway creation       |  |  - Timing correlation     |    |
|  |  - Weight management      |  |  - LTP/LTD calculation    |    |
|  |  - Activation tracking    |  |  - Weight updates         |    |
|  |  - Pathway pruning        |  |  - Decay functions        |    |
|  +-------------+-------------+  +-------------+-------------+    |
|                |                              |                  |
|                +------------+-----------------+                  |
|                             |                                    |
|  +---------------------------+---------------------------+       |
|  |         Pattern Recognition & Memory                  |       |
|  |                                                       |       |
|  |  - Episodic memory        - Homeostatic regulation    |       |
|  |  - Pattern consolidation  - Adaptive learning         |       |
|  |  - Sequence detection     - Memory layers             |       |
|  +-------------------------------------------------------+       |
|                                                                  |
+------------------------------------------------------------------+
```

---

## Module Reference (M25-M30)

| Module | File | Purpose |
|--------|------|---------|
| M25 | `hebbian.rs` | Hebbian engine - pathway management |
| M26 | `stdp.rs` | STDP learning - timing-based plasticity |
| M27 | `homeostatic.rs` | Homeostatic regulation - balance maintenance |
| M28 | `episodic.rs` | Episodic memory - event recording |
| M29 | `pattern.rs` | Pattern recognition - sequence detection |
| M30 | `adaptation.rs` | Adaptive learning - dynamic adjustment |

---

## Core Concepts

### Hebbian Learning Principle

> "Neurons that fire together, wire together."

When an error pattern consistently leads to a successful remediation action, the pathway weight between them is strengthened. Conversely, when a remediation fails, the pathway is weakened.

### STDP (Spike-Timing-Dependent Plasticity)

```
Weight Change (Δw) based on timing:

    Δw
    ^
    |     *
    |    ***
    |   *****
    |  *******
----+-------------------> Δt (ms)
    |        *******
    |         *****
    |          ***
    |           *

If pre-synaptic spike before post-synaptic: LTP (strengthen)
If pre-synaptic spike after post-synaptic: LTD (weaken)
```

---

## Core Types

### HebbianPathway

```rust
#[derive(Debug, Clone)]
pub struct HebbianPathway {
    /// Unique pathway identifier
    pub id: PathwayId,

    /// Source neuron (error pattern)
    pub source: NeuronId,

    /// Target neuron (remediation action)
    pub target: NeuronId,

    /// Current weight [0.0, 1.0]
    pub weight: f64,

    /// Number of times activated
    pub activation_count: u64,

    /// Success rate of this pathway
    pub success_rate: f64,

    /// Creation timestamp
    pub created_at: DateTime<Utc>,

    /// Last activation timestamp
    pub last_activated: Option<DateTime<Utc>>,

    /// Last weight update timestamp
    pub last_updated: DateTime<Utc>,

    /// Pathway type
    pub pathway_type: PathwayType,
}
```

### PathwayType

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum PathwayType {
    /// Error to remediation mapping
    ErrorRemediation,

    /// Service to service connection
    ServiceConnection,

    /// Pattern sequence
    PatternSequence,

    /// Escalation pathway
    Escalation,

    /// Feedback loop
    Feedback,
}
```

### MemoryLayer

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum MemoryLayer {
    /// Short-term working memory (seconds)
    ShortTerm,

    /// Intermediate buffer (minutes)
    Intermediate,

    /// Long-term consolidated (hours+)
    LongTerm,

    /// Permanent reference patterns
    Permanent,
}
```

### HebbianPulse

```rust
#[derive(Debug, Clone)]
pub struct HebbianPulse {
    /// Pulse source
    pub source: ServiceId,

    /// Pulse timestamp
    pub timestamp: DateTime<Utc>,

    /// Pulse strength [0.0, 1.0]
    pub strength: f64,

    /// Associated context
    pub context: PulseContext,

    /// Triggered pathways
    pub activated_pathways: Vec<PathwayId>,
}
```

---

## STDP Engine

### STDP Configuration

```rust
pub struct StdpConfig {
    /// Time constant for LTP (ms)
    pub tau_plus: f64,          // Default: 20.0

    /// Time constant for LTD (ms)
    pub tau_minus: f64,         // Default: 20.0

    /// Maximum LTP amplitude
    pub a_plus: f64,            // Default: 0.1 (LTP_RATE)

    /// Maximum LTD amplitude
    pub a_minus: f64,           // Default: 0.05 (LTD_RATE)

    /// Minimum pathway weight
    pub w_min: f64,             // Default: 0.0

    /// Maximum pathway weight
    pub w_max: f64,             // Default: 1.0

    /// Weight decay rate per second
    pub decay_rate: f64,        // Default: 0.001

    /// STDP timing window (ms)
    pub window_ms: u64,         // Default: 100
}
```

### Global STDP Parameters

| Parameter | Value | Description |
|-----------|-------|-------------|
| LTP Rate | 0.1 | Long-Term Potentiation (strengthening) |
| LTD Rate | 0.05 | Long-Term Depression (weakening) |
| STDP Window | 100ms | Timing window for plasticity |
| Decay Rate | 0.001 | Weight decay per second |
| tau_plus | 20ms | LTP time constant |
| tau_minus | 20ms | LTD time constant |

### Weight Update Algorithm

```rust
impl StdpEngine {
    /// Calculate weight change based on spike timing
    pub fn calculate_delta_w(&self, delta_t_ms: f64) -> f64 {
        if delta_t_ms > 0.0 {
            // LTP: Pre before post -> strengthen
            self.config.a_plus * (-delta_t_ms / self.config.tau_plus).exp()
        } else {
            // LTD: Post before pre -> weaken
            -self.config.a_minus * (delta_t_ms / self.config.tau_minus).exp()
        }
    }

    /// Apply weight update with bounds
    pub fn update_weight(&mut self, pathway: &mut HebbianPathway, delta_w: f64) {
        pathway.weight = (pathway.weight + delta_w)
            .clamp(self.config.w_min, self.config.w_max);
        pathway.last_updated = Utc::now();
    }

    /// Apply decay to all pathways
    pub fn apply_decay(&mut self, pathways: &mut [HebbianPathway], elapsed_secs: f64) {
        for pathway in pathways {
            let decay = self.config.decay_rate * elapsed_secs;
            pathway.weight = (pathway.weight - decay).max(self.config.w_min);
        }
    }
}
```

### STDP Engine API

```rust
pub struct StdpEngine {
    pub fn new(config: StdpConfig) -> Self;

    /// Record spike event
    pub fn record_spike(&mut self, neuron_id: NeuronId, timestamp: Instant);

    /// Process spike pair for STDP update
    pub fn process_spike_pair(&mut self, pre: &Spike, post: &Spike);

    /// Update all pathways based on recorded spikes
    pub fn update_pathways(&mut self);

    /// Get pathway weight
    pub fn get_weight(&self, from: NeuronId, to: NeuronId) -> Option<f64>;

    /// Set learning rate multiplier
    pub fn set_learning_rate(&mut self, rate: f64);

    /// Pause learning (freeze weights)
    pub fn pause_learning(&mut self);

    /// Resume learning
    pub fn resume_learning(&mut self);
}
```

---

## Hebbian Engine

### Pathway Manager API

```rust
pub struct HebbianEngine {
    /// Create a new pathway
    pub fn create_pathway(&mut self, source: NeuronId, target: NeuronId) -> PathwayId;

    /// Remove pathway
    pub fn remove_pathway(&mut self, id: PathwayId) -> Result<()>;

    /// Get pathway by ID
    pub fn get_pathway(&self, id: PathwayId) -> Option<&HebbianPathway>;

    /// Find pathways from source
    pub fn find_pathways(&self, source: NeuronId) -> Vec<&HebbianPathway>;

    /// Get strongest pathway from source
    pub fn get_strongest_pathway(&self, source: NeuronId) -> Option<&HebbianPathway>;

    /// Prune weak pathways below threshold
    pub fn prune_weak_pathways(&mut self, threshold: f64) -> Vec<PathwayId>;

    /// List all pathways
    pub fn list_all(&self) -> Vec<&HebbianPathway>;

    /// Export pathway graph for visualization
    pub fn export_graph(&self) -> PathwayGraph;

    /// Record pathway activation
    pub fn activate(&mut self, pathway_id: PathwayId, success: bool);
}
```

### Pathway Pruning Configuration

Pathways are pruned when:
1. Weight falls below threshold (default: 0.05)
2. No activation in 30 days
3. Success rate below 10% after 100+ activations

```rust
pub struct PruningConfig {
    pub weight_threshold: f64,       // Default: 0.05
    pub inactivity_days: u32,        // Default: 30
    pub min_activations: u64,        // Default: 100
    pub min_success_rate: f64,       // Default: 0.10
    pub prune_interval_hours: u32,   // Default: 24
}
```

---

## Homeostatic Regulation

### Homeostatic Targets

```rust
pub struct HomeostaticConfig {
    /// Target average pathway weight
    pub target_weight: f64,          // Default: 0.5

    /// Target LTP:LTD ratio
    pub target_ltp_ltd_ratio: f64,   // Default: 2.0-4.0

    /// Adaptation rate
    pub adaptation_rate: f64,        // Default: 0.01

    /// Target system health
    pub target_health: f64,          // Default: 0.95

    /// Target synergy score
    pub target_synergy: f64,         // Default: 0.90

    /// Target uptime
    pub target_uptime: f64,          // Default: 0.99
}
```

### Homeostatic API

```rust
pub struct HomeostaticRegulator {
    /// Check if system is in homeostatic balance
    pub fn is_balanced(&self) -> bool;

    /// Get current deviation from targets
    pub fn get_deviation(&self) -> HomeostaticDeviation;

    /// Apply homeostatic correction
    pub fn correct(&mut self, engine: &mut HebbianEngine);

    /// Get LTP:LTD ratio
    pub fn ltp_ltd_ratio(&self) -> f64;
}
```

---

## Episodic Memory

### Episode Recording

```rust
pub struct EpisodicMemory {
    /// Record new episode
    pub fn record(&mut self, episode: Episode);

    /// Query similar episodes
    pub fn query_similar(&self, pattern: &Pattern, limit: usize) -> Vec<&Episode>;

    /// Consolidate short-term to long-term
    pub fn consolidate(&mut self);

    /// Get episodes by time range
    pub fn by_time_range(&self, start: DateTime<Utc>, end: DateTime<Utc>) -> Vec<&Episode>;
}

pub struct Episode {
    pub id: EpisodeId,
    pub timestamp: DateTime<Utc>,
    pub trigger: EpisodeTrigger,
    pub actions: Vec<Action>,
    pub outcome: Outcome,
    pub context: EpisodeContext,
    pub memory_layer: MemoryLayer,
}
```

### Consolidation Events

```rust
#[derive(Debug, Clone)]
pub struct ConsolidationEvent {
    /// Episodes being consolidated
    pub episodes: Vec<EpisodeId>,

    /// Resulting pattern
    pub pattern: Pattern,

    /// Consolidation timestamp
    pub timestamp: DateTime<Utc>,

    /// Source memory layer
    pub from_layer: MemoryLayer,

    /// Target memory layer
    pub to_layer: MemoryLayer,
}
```

---

## Pattern Recognition

### Pattern API

```rust
pub struct PatternRecognition {
    /// Cluster similar error patterns
    pub fn cluster_errors(&self, errors: &[ErrorVector]) -> Vec<ErrorCluster>;

    /// Find most similar known pattern
    pub fn find_similar(&self, error: &ErrorVector, threshold: f64) -> Option<ErrorCluster>;

    /// Detect sequence patterns in history
    pub fn detect_sequences(&self, history: &[ErrorEvent]) -> Vec<ErrorSequence>;

    /// Predict likely next error
    pub fn predict_next(&self, current: &SystemState) -> Vec<PredictedError>;
}
```

---

## Feedback Loop

### Remediation Feedback

```rust
pub struct RemediationFeedback {
    /// Pathway that was used
    pub pathway_id: PathwayId,

    /// Was remediation successful?
    pub success: bool,

    /// Time taken for remediation
    pub duration_ms: u64,

    /// Error vector that triggered remediation
    pub error_vector: ErrorVector,

    /// Remediation action taken
    pub action: RemediationAction,

    /// Timestamp
    pub timestamp: DateTime<Utc>,
}

impl StdpEngine {
    /// Process feedback to update pathway weights
    pub fn process_feedback(&mut self, feedback: RemediationFeedback) {
        if feedback.success {
            // Successful: strengthen pathway (LTP)
            let delta_w = self.config.a_plus;
            self.update_pathway_weight(feedback.pathway_id, delta_w);
        } else {
            // Failed: weaken pathway (LTD)
            let delta_w = -self.config.a_minus;
            self.update_pathway_weight(feedback.pathway_id, delta_w);
        }
    }
}
```

---

## Inter-Layer Communication

### Events from L4 (Integration)

```rust
pub enum L4InputEvent {
    ServiceStateChanged { service: ServiceId, tensor: Tensor12D },
    BridgeConnected { service: ServiceId },
    SynergyUpdated { from: ServiceId, to: ServiceId, score: f64 },
}
```

### Events to L6 (Consensus)

```rust
pub enum L5OutputEvent {
    PathwayStrengthened { pathway: PathwayId, old_weight: f64, new_weight: f64 },
    PathwayWeakened { pathway: PathwayId, old_weight: f64, new_weight: f64 },
    PatternRecognized { pattern: Pattern, confidence: f64 },
    LearningAnomaly { description: String, severity: Severity },
    RemediationRecommendation { error: ErrorVector, action: RemediationAction, confidence: f64 },
}
```

---

## Metrics

| Metric | Type | Description |
|--------|------|-------------|
| `me_l5_pathways_total` | Gauge | Total number of pathways |
| `me_l5_pathway_weight` | Histogram | Distribution of pathway weights |
| `me_l5_learning_rate` | Gauge | Current learning rate |
| `me_l5_activations_total` | Counter | Total pathway activations |
| `me_l5_ltp_events` | Counter | LTP (strengthening) events |
| `me_l5_ltd_events` | Counter | LTD (weakening) events |
| `me_l5_pruned_pathways` | Counter | Pathways pruned |
| `me_l5_pattern_clusters` | Gauge | Number of error pattern clusters |
| `me_l5_ltp_ltd_ratio` | Gauge | Current LTP:LTD ratio |
| `me_l5_homeostatic_deviation` | Gauge | Deviation from homeostatic targets |

---

## Configuration

```toml
[layer.L5]
enabled = true
startup_order = 5

[layer.L5.stdp]
ltp_rate = 0.1
ltd_rate = 0.05
window_ms = 100
decay_rate = 0.001
tau_plus = 20.0
tau_minus = 20.0
w_min = 0.0
w_max = 1.0
learning_enabled = true

[layer.L5.pathways]
initial_weight = 0.5
prune_threshold = 0.05
prune_interval_hours = 24
max_pathways = 10000

[layer.L5.homeostatic]
target_weight = 0.5
target_ltp_ltd_ratio = 3.0
adaptation_rate = 0.01
target_health = 0.95
target_synergy = 0.90

[layer.L5.pattern_recognition]
clustering_algorithm = "dbscan"
similarity_threshold = 0.85
min_sequence_length = 2
max_sequence_length = 10

[layer.L5.episodic]
max_short_term_episodes = 1000
consolidation_interval_hours = 1
retention_days = 90
```

---

## CLI Commands

```bash
# View pathway statistics
./maintenance-engine learning stats

# List top pathways by weight
./maintenance-engine learning pathways --top 20

# View specific pathway
./maintenance-engine learning pathway --id PW-001

# Trigger manual prune
./maintenance-engine learning prune --threshold 0.1

# Export pathway graph
./maintenance-engine learning export --format dot > pathways.dot

# Pause learning
./maintenance-engine learning pause

# Resume learning
./maintenance-engine learning resume

# View LTP/LTD ratio
./maintenance-engine learning ratio

# Check homeostatic balance
./maintenance-engine learning homeostatic

# Reset all pathways (dangerous!)
./maintenance-engine learning reset --confirm
```

---

## Navigation

| Direction | Link |
|-----------|------|
| Up | [INDEX.md](../INDEX.md) |
| Previous | [L04_INTEGRATION.md](L04_INTEGRATION.md) |
| Next | [L06_CONSENSUS.md](L06_CONSENSUS.md) |
| Related Spec | [STDP_SPEC.md](../../ai_specs/STDP_SPEC.md) |
| API | [REST_API.md](../integration/REST_API.md) |

---

*[Back to Index](../INDEX.md) | [Previous: L04 Integration](L04_INTEGRATION.md) | [Next: L06 Consensus](L06_CONSENSUS.md)*
