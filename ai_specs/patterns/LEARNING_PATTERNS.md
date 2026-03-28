# Hebbian Learning Patterns Reference

> STDP and Hebbian Patterns for Claude Code & CodeSynthor V7
> Derived from SYNTHEX, The Maintenance Engine, and CodeSynthor V7

---

## Overview

| Metric | Value |
|--------|-------|
| **Patterns** | 10 |
| **Priority** | P2 |
| **Learning Types** | Hebbian, STDP, Reinforcement |

---

## Pattern 1: Hebbian Pathway (P0)

```rust
/// A connection between two services that strengthens with co-activation.
/// "Neurons that fire together, wire together."
#[derive(Clone, Debug)]
pub struct HebbianPathway {
    /// Source service identifier
    pub source_id: String,

    /// Target service identifier
    pub target_id: String,

    /// Connection strength [0.1, 1.0]
    pub strength: f64,

    /// Long-term potentiation factor
    pub ltp: f64,

    /// Long-term depression factor
    pub ltd: f64,

    /// Activation count
    pub activations: u64,

    /// Last activation timestamp
    pub last_activated: DateTime<Utc>,

    /// Creation timestamp
    pub created_at: DateTime<Utc>,
}

impl HebbianPathway {
    /// Minimum strength threshold (below this, pathway is pruned)
    pub const MIN_STRENGTH: f64 = 0.1;

    /// Maximum strength
    pub const MAX_STRENGTH: f64 = 1.0;

    /// Default LTP rate
    pub const DEFAULT_LTP: f64 = 0.1;

    /// Default LTD rate
    pub const DEFAULT_LTD: f64 = 0.05;

    pub fn new(source_id: String, target_id: String) -> Self {
        Self {
            source_id,
            target_id,
            strength: 0.5,  // Start at midpoint
            ltp: Self::DEFAULT_LTP,
            ltd: Self::DEFAULT_LTD,
            activations: 0,
            last_activated: Utc::now(),
            created_at: Utc::now(),
        }
    }

    /// Strengthen the pathway (LTP)
    pub fn potentiate(&mut self) {
        self.strength = (self.strength + self.ltp).min(Self::MAX_STRENGTH);
        self.activations += 1;
        self.last_activated = Utc::now();
    }

    /// Weaken the pathway (LTD)
    pub fn depress(&mut self) {
        self.strength = (self.strength - self.ltd).max(Self::MIN_STRENGTH);
        self.last_activated = Utc::now();
    }

    /// Check if pathway should be pruned
    pub fn should_prune(&self) -> bool {
        self.strength <= Self::MIN_STRENGTH
    }

    /// Apply time-based decay
    pub fn apply_decay(&mut self, decay_rate: f64) {
        let hours_since = (Utc::now() - self.last_activated).num_hours() as f64;
        let decay = (-hours_since * decay_rate).exp();
        self.strength *= decay;
    }
}
```

**Why**: Hebbian pathways encode successful service interactions for routing optimization.

---

## Pattern 2: STDP Learning (P0)

```rust
/// Spike-Timing Dependent Plasticity implementation.
/// Strengthens pathways when effect follows cause (positive delta_t).
pub struct StdpLearner {
    /// Time window for learning (milliseconds)
    pub window_ms: i64,

    /// Maximum strength change per event
    pub max_delta: f64,

    /// Decay constant for LTP
    pub tau_plus: f64,

    /// Decay constant for LTD
    pub tau_minus: f64,
}

impl Default for StdpLearner {
    fn default() -> Self {
        Self {
            window_ms: 100,      // 100ms learning window
            max_delta: 0.1,     // Max 10% change
            tau_plus: 20.0,     // LTP decay constant
            tau_minus: 20.0,    // LTD decay constant
        }
    }
}

impl StdpLearner {
    /// Calculate weight change based on spike timing.
    /// delta_t = t_post - t_pre (positive = causal, negative = anti-causal)
    pub fn calculate_delta(&self, delta_t_ms: i64) -> f64 {
        if delta_t_ms.abs() > self.window_ms {
            return 0.0;  // Outside learning window
        }

        let delta_t = delta_t_ms as f64;

        if delta_t > 0.0 {
            // Causal: pre before post -> LTP
            self.max_delta * (-delta_t / self.tau_plus).exp()
        } else if delta_t < 0.0 {
            // Anti-causal: post before pre -> LTD
            -self.max_delta * (delta_t / self.tau_minus).exp()
        } else {
            0.0  // Simultaneous: no change
        }
    }

    /// Apply STDP update to a pathway
    pub fn apply(&self, pathway: &mut HebbianPathway, delta_t_ms: i64) {
        let delta = self.calculate_delta(delta_t_ms);

        if delta > 0.0 {
            pathway.strength = (pathway.strength + delta).min(HebbianPathway::MAX_STRENGTH);
        } else {
            pathway.strength = (pathway.strength + delta).max(HebbianPathway::MIN_STRENGTH);
        }

        pathway.last_activated = Utc::now();
    }
}
```

**Why**: STDP enables causal learning - effects that follow causes strengthen the connection.

---

## Pattern 3: Pathway Network (P1)

```rust
use dashmap::DashMap;

/// Network of Hebbian pathways between services
pub struct PathwayNetwork {
    /// Pathways indexed by (source, target) pair
    pathways: Arc<DashMap<(String, String), HebbianPathway>>,

    /// STDP learner instance
    stdp: StdpLearner,

    /// Global decay rate per hour
    decay_rate: f64,
}

impl PathwayNetwork {
    pub fn new() -> Self {
        Self {
            pathways: Arc::new(DashMap::new()),
            stdp: StdpLearner::default(),
            decay_rate: 0.001,  // 0.1% decay per hour
        }
    }

    /// Get or create pathway between two services
    pub fn get_or_create(&self, source: &str, target: &str) -> HebbianPathway {
        let key = (source.to_string(), target.to_string());

        self.pathways.entry(key.clone())
            .or_insert_with(|| HebbianPathway::new(key.0, key.1))
            .clone()
    }

    /// Record successful interaction (source -> target)
    pub fn record_success(&self, source: &str, target: &str, latency_ms: i64) {
        let key = (source.to_string(), target.to_string());

        if let Some(mut pathway) = self.pathways.get_mut(&key) {
            // Positive STDP: successful call strengthens pathway
            self.stdp.apply(&mut pathway, latency_ms);
        } else {
            let mut pathway = HebbianPathway::new(source.to_string(), target.to_string());
            pathway.potentiate();
            self.pathways.insert(key, pathway);
        }
    }

    /// Record failed interaction (source -> target)
    pub fn record_failure(&self, source: &str, target: &str) {
        let key = (source.to_string(), target.to_string());

        if let Some(mut pathway) = self.pathways.get_mut(&key) {
            pathway.depress();
        }
    }

    /// Get strongest pathways from a source
    pub fn strongest_from(&self, source: &str, limit: usize) -> Vec<HebbianPathway> {
        let mut pathways: Vec<_> = self.pathways.iter()
            .filter(|p| p.key().0 == source)
            .map(|p| p.value().clone())
            .collect();

        pathways.sort_by(|a, b| b.strength.partial_cmp(&a.strength).unwrap());
        pathways.truncate(limit);
        pathways
    }

    /// Prune weak pathways
    pub fn prune(&self) -> usize {
        let to_remove: Vec<_> = self.pathways.iter()
            .filter(|p| p.value().should_prune())
            .map(|p| p.key().clone())
            .collect();

        let count = to_remove.len();
        for key in to_remove {
            self.pathways.remove(&key);
        }
        count
    }

    /// Apply decay to all pathways
    pub fn apply_global_decay(&self) {
        for mut pathway in self.pathways.iter_mut() {
            pathway.apply_decay(self.decay_rate);
        }
    }
}
```

**Why**: Network structure enables pathway-based routing decisions.

---

## Pattern 4: Learning Events (P1)

```rust
/// Events that trigger learning updates
#[derive(Clone, Debug)]
pub enum LearningEvent {
    /// Service call succeeded
    CallSuccess {
        source: String,
        target: String,
        latency_ms: u64,
        timestamp: DateTime<Utc>,
    },

    /// Service call failed
    CallFailure {
        source: String,
        target: String,
        error_type: String,
        timestamp: DateTime<Utc>,
    },

    /// Health check completed
    HealthCheck {
        service_id: String,
        health_score: f64,
        timestamp: DateTime<Utc>,
    },

    /// Remediation completed
    RemediationComplete {
        service_id: String,
        success: bool,
        duration_ms: u64,
        timestamp: DateTime<Utc>,
    },

    /// Consensus reached
    ConsensusReached {
        decision_id: String,
        participants: Vec<String>,
        outcome: String,
        timestamp: DateTime<Utc>,
    },
}

impl LearningEvent {
    pub fn timestamp(&self) -> DateTime<Utc> {
        match self {
            Self::CallSuccess { timestamp, .. } => *timestamp,
            Self::CallFailure { timestamp, .. } => *timestamp,
            Self::HealthCheck { timestamp, .. } => *timestamp,
            Self::RemediationComplete { timestamp, .. } => *timestamp,
            Self::ConsensusReached { timestamp, .. } => *timestamp,
        }
    }
}

/// Process learning events and update pathways
pub struct LearningProcessor {
    network: Arc<PathwayNetwork>,
    event_buffer: Arc<Mutex<Vec<LearningEvent>>>,
    batch_size: usize,
}

impl LearningProcessor {
    pub fn new(network: Arc<PathwayNetwork>) -> Self {
        Self {
            network,
            event_buffer: Arc::new(Mutex::new(Vec::new())),
            batch_size: 100,
        }
    }

    pub async fn submit(&self, event: LearningEvent) {
        let mut buffer = self.event_buffer.lock().await;
        buffer.push(event);

        if buffer.len() >= self.batch_size {
            let events = std::mem::take(&mut *buffer);
            drop(buffer);  // Release lock before processing
            self.process_batch(events).await;
        }
    }

    async fn process_batch(&self, events: Vec<LearningEvent>) {
        for event in events {
            match event {
                LearningEvent::CallSuccess { source, target, latency_ms, .. } => {
                    self.network.record_success(&source, &target, latency_ms as i64);
                }
                LearningEvent::CallFailure { source, target, .. } => {
                    self.network.record_failure(&source, &target);
                }
                LearningEvent::RemediationComplete { service_id, success, .. } => {
                    if success {
                        // Strengthen pathways to this service
                        for pathway in self.network.pathways.iter_mut() {
                            if pathway.key().1 == service_id {
                                pathway.potentiate();
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }
}
```

**Why**: Event-driven learning decouples observation from weight updates.

---

## Pattern 5: Pathway Persistence (P1)

```rust
use rusqlite::{Connection, params};

pub struct PathwayStore {
    pool: DbPool,
}

impl PathwayStore {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    /// Save pathway to database
    pub fn save(&self, pathway: &HebbianPathway) -> Result<()> {
        let conn = self.pool.get()?;
        conn.execute(
            "INSERT OR REPLACE INTO hebbian_pathways
             (source_id, target_id, strength, ltp, ltd, activations, last_activated, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                pathway.source_id,
                pathway.target_id,
                pathway.strength,
                pathway.ltp,
                pathway.ltd,
                pathway.activations as i64,
                pathway.last_activated.to_rfc3339(),
                pathway.created_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    /// Load all pathways
    pub fn load_all(&self) -> Result<Vec<HebbianPathway>> {
        let conn = self.pool.get()?;
        let mut stmt = conn.prepare(
            "SELECT source_id, target_id, strength, ltp, ltd, activations, last_activated, created_at
             FROM hebbian_pathways
             WHERE strength > ?1",
        )?;

        let pathways = stmt.query_map([HebbianPathway::MIN_STRENGTH], |row| {
            Ok(HebbianPathway {
                source_id: row.get(0)?,
                target_id: row.get(1)?,
                strength: row.get(2)?,
                ltp: row.get(3)?,
                ltd: row.get(4)?,
                activations: row.get::<_, i64>(5)? as u64,
                last_activated: DateTime::parse_from_rfc3339(&row.get::<_, String>(6)?)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
                created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(7)?)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
            })
        })?;

        pathways.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|e| Error::Database(e.to_string()))
    }

    /// Get strongest pathway between two services
    pub fn get_strongest(&self, source: &str, target: &str) -> Result<Option<HebbianPathway>> {
        let conn = self.pool.get()?;
        let mut stmt = conn.prepare(
            "SELECT source_id, target_id, strength, ltp, ltd, activations, last_activated, created_at
             FROM hebbian_pathways
             WHERE source_id = ?1 AND target_id = ?2",
        )?;

        stmt.query_row([source, target], |row| {
            Ok(HebbianPathway {
                source_id: row.get(0)?,
                target_id: row.get(1)?,
                strength: row.get(2)?,
                ltp: row.get(3)?,
                ltd: row.get(4)?,
                activations: row.get::<_, i64>(5)? as u64,
                last_activated: DateTime::parse_from_rfc3339(&row.get::<_, String>(6)?)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
                created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(7)?)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
            })
        })
        .optional()
        .map_err(|e| Error::Database(e.to_string()))
    }
}
```

**Why**: Persistence enables learning to survive restarts.

---

## Pattern 6: Reinforcement Integration (P1)

```rust
/// Reward signal for reinforcement learning integration
#[derive(Clone, Debug)]
pub struct Reward {
    pub value: f64,           // [-1.0, 1.0]
    pub source: String,       // What generated the reward
    pub context: String,      // Additional context
    pub timestamp: DateTime<Utc>,
}

impl Reward {
    pub fn positive(value: f64, source: &str) -> Self {
        Self {
            value: value.clamp(0.0, 1.0),
            source: source.to_string(),
            context: String::new(),
            timestamp: Utc::now(),
        }
    }

    pub fn negative(value: f64, source: &str) -> Self {
        Self {
            value: -value.clamp(0.0, 1.0),
            source: source.to_string(),
            context: String::new(),
            timestamp: Utc::now(),
        }
    }
}

/// Combine Hebbian and reinforcement learning
pub struct HybridLearner {
    network: Arc<PathwayNetwork>,
    /// Weight for Hebbian updates
    hebbian_weight: f64,
    /// Weight for reinforcement updates
    rl_weight: f64,
    /// Discount factor for future rewards
    gamma: f64,
}

impl HybridLearner {
    pub fn new(network: Arc<PathwayNetwork>) -> Self {
        Self {
            network,
            hebbian_weight: 0.7,
            rl_weight: 0.3,
            gamma: 0.95,
        }
    }

    /// Apply combined learning update
    pub fn learn(&self, source: &str, target: &str, reward: &Reward, delta_t_ms: i64) {
        let key = (source.to_string(), target.to_string());

        if let Some(mut pathway) = self.network.pathways.get_mut(&key) {
            // Calculate Hebbian delta
            let hebbian_delta = self.network.stdp.calculate_delta(delta_t_ms);

            // Calculate RL delta (reward-modulated)
            let rl_delta = reward.value * 0.1;  // Max 10% change from reward

            // Combined update
            let total_delta = self.hebbian_weight * hebbian_delta + self.rl_weight * rl_delta;

            pathway.strength = (pathway.strength + total_delta)
                .clamp(HebbianPathway::MIN_STRENGTH, HebbianPathway::MAX_STRENGTH);

            pathway.last_activated = Utc::now();
        }
    }
}
```

**Why**: Hybrid learning combines correlation-based and reward-based signals.

---

## Pattern 7: Pathway-Based Routing (P2)

```rust
/// Use learned pathways for intelligent routing
pub struct PathwayRouter {
    network: Arc<PathwayNetwork>,
    /// Minimum strength to consider a pathway
    min_routing_strength: f64,
    /// Exploration rate (epsilon-greedy)
    exploration_rate: f64,
}

impl PathwayRouter {
    pub fn new(network: Arc<PathwayNetwork>) -> Self {
        Self {
            network,
            min_routing_strength: 0.3,
            exploration_rate: 0.1,  // 10% random exploration
        }
    }

    /// Select best target from candidates based on pathway strength
    pub fn select_target(&self, source: &str, candidates: &[String]) -> Option<String> {
        if candidates.is_empty() {
            return None;
        }

        // Epsilon-greedy exploration
        if rand::random::<f64>() < self.exploration_rate {
            let idx = rand::random::<usize>() % candidates.len();
            return Some(candidates[idx].clone());
        }

        // Find strongest pathway to any candidate
        let mut best: Option<(String, f64)> = None;

        for target in candidates {
            let key = (source.to_string(), target.clone());
            if let Some(pathway) = self.network.pathways.get(&key) {
                if pathway.strength >= self.min_routing_strength {
                    match &best {
                        None => best = Some((target.clone(), pathway.strength)),
                        Some((_, strength)) if pathway.strength > *strength => {
                            best = Some((target.clone(), pathway.strength));
                        }
                        _ => {}
                    }
                }
            }
        }

        // Fall back to first candidate if no strong pathway
        best.map(|(t, _)| t).or_else(|| candidates.first().cloned())
    }

    /// Get routing scores for all candidates
    pub fn score_candidates(&self, source: &str, candidates: &[String]) -> Vec<(String, f64)> {
        candidates.iter().map(|target| {
            let key = (source.to_string(), target.clone());
            let score = self.network.pathways.get(&key)
                .map(|p| p.strength)
                .unwrap_or(0.0);
            (target.clone(), score)
        }).collect()
    }
}
```

**Why**: Routing based on learned pathways optimizes for historically successful interactions.

---

## Pattern 8: Learning Metrics (P2)

```rust
/// Metrics for monitoring learning system health
pub struct LearningMetrics {
    /// Total pathway count
    pub pathway_count: u64,

    /// Average pathway strength
    pub avg_strength: f64,

    /// Pathways above routing threshold
    pub strong_pathways: u64,

    /// Pathways at minimum (near pruning)
    pub weak_pathways: u64,

    /// Learning events processed
    pub events_processed: u64,

    /// Prunes executed
    pub prunes_executed: u64,
}

impl LearningMetrics {
    pub fn collect(network: &PathwayNetwork) -> Self {
        let pathway_count = network.pathways.len() as u64;

        let (sum, strong, weak) = network.pathways.iter().fold(
            (0.0, 0u64, 0u64),
            |(sum, strong, weak), p| {
                let s = p.strength;
                (
                    sum + s,
                    strong + if s >= 0.5 { 1 } else { 0 },
                    weak + if s <= 0.2 { 1 } else { 0 },
                )
            }
        );

        Self {
            pathway_count,
            avg_strength: if pathway_count > 0 { sum / pathway_count as f64 } else { 0.0 },
            strong_pathways: strong,
            weak_pathways: weak,
            events_processed: 0,  // Set externally
            prunes_executed: 0,   // Set externally
        }
    }
}
```

**Why**: Metrics enable monitoring and tuning of learning system.

---

## Pattern 9: Batch Learning (P2)

```rust
/// Efficient batch updates for high-throughput scenarios
pub struct BatchLearner {
    network: Arc<PathwayNetwork>,
    pending_updates: Mutex<HashMap<(String, String), Vec<f64>>>,
}

impl BatchLearner {
    pub fn new(network: Arc<PathwayNetwork>) -> Self {
        Self {
            network,
            pending_updates: Mutex::new(HashMap::new()),
        }
    }

    /// Queue an update for batch processing
    pub async fn queue_update(&self, source: &str, target: &str, delta: f64) {
        let key = (source.to_string(), target.to_string());
        let mut updates = self.pending_updates.lock().await;
        updates.entry(key).or_insert_with(Vec::new).push(delta);
    }

    /// Apply all queued updates
    pub async fn flush(&self) {
        let updates = {
            let mut pending = self.pending_updates.lock().await;
            std::mem::take(&mut *pending)
        };

        for ((source, target), deltas) in updates {
            let key = (source.clone(), target.clone());

            // Average the deltas
            let avg_delta: f64 = deltas.iter().sum::<f64>() / deltas.len() as f64;

            self.network.pathways.entry(key.clone())
                .or_insert_with(|| HebbianPathway::new(source, target))
                .strength = (self.network.pathways.get(&key)
                    .map(|p| p.strength)
                    .unwrap_or(0.5) + avg_delta)
                    .clamp(HebbianPathway::MIN_STRENGTH, HebbianPathway::MAX_STRENGTH);
        }
    }
}
```

**Why**: Batch updates reduce lock contention in high-throughput scenarios.

---

## Pattern 10: Learning Configuration (P2)

```rust
/// Configuration for learning system tuning
#[derive(Clone, Debug)]
pub struct LearningConfig {
    /// STDP window in milliseconds
    pub stdp_window_ms: i64,

    /// Maximum STDP delta per event
    pub stdp_max_delta: f64,

    /// LTP time constant
    pub tau_plus: f64,

    /// LTD time constant
    pub tau_minus: f64,

    /// Global decay rate per hour
    pub decay_rate_per_hour: f64,

    /// Minimum pathway strength before pruning
    pub prune_threshold: f64,

    /// Batch size for learning updates
    pub batch_size: usize,

    /// Exploration rate for routing
    pub exploration_rate: f64,

    /// Minimum strength for routing consideration
    pub routing_threshold: f64,
}

impl Default for LearningConfig {
    fn default() -> Self {
        Self {
            stdp_window_ms: 100,
            stdp_max_delta: 0.1,
            tau_plus: 20.0,
            tau_minus: 20.0,
            decay_rate_per_hour: 0.001,
            prune_threshold: 0.1,
            batch_size: 100,
            exploration_rate: 0.1,
            routing_threshold: 0.3,
        }
    }
}

impl LearningConfig {
    /// Production configuration with conservative learning
    pub fn production() -> Self {
        Self {
            stdp_max_delta: 0.05,      // Slower learning
            decay_rate_per_hour: 0.0005, // Slower forgetting
            exploration_rate: 0.05,    // Less exploration
            ..Default::default()
        }
    }

    /// Development configuration with fast learning
    pub fn development() -> Self {
        Self {
            stdp_max_delta: 0.2,       // Faster learning
            decay_rate_per_hour: 0.01, // Faster forgetting
            exploration_rate: 0.2,     // More exploration
            ..Default::default()
        }
    }
}
```

**Why**: Configuration enables tuning learning behavior for different environments.

---

## Learning System Constants

| Parameter | Default | Range | Description |
|-----------|---------|-------|-------------|
| STDP Window | 100ms | 10-500ms | Time window for causal learning |
| LTP Rate | 0.1 | 0.01-0.5 | Potentiation strength |
| LTD Rate | 0.05 | 0.01-0.5 | Depression strength |
| Decay Rate | 0.001/hr | 0.0001-0.01 | Strength decay over time |
| Prune Threshold | 0.1 | 0.05-0.2 | Minimum strength to keep |
| Routing Threshold | 0.3 | 0.2-0.5 | Minimum for routing use |

---

*Generated: 2026-01-28 | The Maintenance Engine v1.0.0*
