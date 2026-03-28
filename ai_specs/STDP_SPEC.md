# STDP/Hebbian Learning Specification

**Version:** 2.0.0
**Generation:** 2 (Enhanced with Homeostasis)
**Coverage:** STDP Learning, Homeostatic Feedback, Pathway Persistence, Cold Start, Learning Diagnostics
**Related:** [TENSOR_SPEC.md](TENSOR_SPEC.md), [NAM_SPEC.md](NAM_SPEC.md), [DATABASE_SPEC.md](DATABASE_SPEC.md), [patterns/LEARNING_PATTERNS.md](patterns/LEARNING_PATTERNS.md)

---

## 1. EXECUTIVE SUMMARY

The Hebbian Learning System implements STDP (Spike-Timing-Dependent Plasticity) to track service interaction effectiveness, strengthen successful pathways, and maintain system stability through homeostatic feedback. This specification covers all parameters, formulas, database schemas, and diagnostic requirements for the learning subsystem.

---

## 2. STDP CORE PARAMETERS

### 2.1 Primary Parameters

| Parameter | Symbol | Default | Range | Description |
|-----------|--------|---------|-------|-------------|
| LTP Rate | `eta_LTP` | 0.1 | [0.01, 0.3] | Long-Term Potentiation rate |
| LTD Rate | `eta_LTD` | 0.05 | [0.01, 0.2] | Long-Term Depression rate |
| STDP Window | `tau_STDP` | 100ms | [50, 200]ms | Timing window for plasticity |
| LTP Time Constant | `tau_plus` | 20ms | [10, 50]ms | Time constant for potentiation |
| LTD Time Constant | `tau_minus` | 20ms | [10, 50]ms | Time constant for depression |
| LTP Amplitude | `a_plus` | 0.01 | [0.005, 0.05] | LTP amplitude factor |
| LTD Amplitude | `a_minus` | 0.012 | [0.005, 0.05] | LTD amplitude factor |
| Weight Bounds | `[w_min, w_max]` | [0.0, 1.0] | Fixed | Hard limits |
| Initial Weight | `w_0` | 0.5 | Fixed | Starting weight for new pathways |
| Decay Rate | `lambda` | 0.001 | [0, 0.01] | Per-hour weight decay |
| Prune Threshold | `w_prune` | 0.05 | [0.01, 0.1] | Minimum weight before pruning |

### 2.2 Healthy Ratio Bounds

| Metric | Healthy Range | Alert Threshold |
|--------|---------------|-----------------|
| LTP:LTD Ratio | 1.5 - 5.0 | <1.5 or >5.0 |
| Weight Distribution Median | 0.4 - 0.6 | <0.3 or >0.7 |
| New Pathways/Day | 5 - 50 | <2 or >100 |
| Pathway Decay Rate | 0.001 - 0.01 | >0.02 |

---

## 3. WEIGHT UPDATE FORMULAS

### 3.1 STDP Weight Update

**On Pre-before-Post (Causal, LTP):**
```
if delta_t > 0:
    delta_w = a_plus * exp(-delta_t / tau_plus)
    w_new = w + eta_LTP * delta_w * (1 - w)  // Bounded LTP
```

**On Post-before-Pre (Anti-causal, LTD):**
```
if delta_t < 0:
    delta_w = -a_minus * exp(delta_t / tau_minus)
    w_new = w - eta_LTD * |delta_w| * w  // Bounded LTD
```

### 3.2 Simplified Update (Success/Failure)

**On Success (LTP):**
```
w_new = w + eta_LTP * (1 - w)
```

**On Failure (LTD):**
```
w_new = w - eta_LTD * w
```

### 3.3 Periodic Decay

```
w_new = w * (1 - lambda * hours_elapsed)
```

### 3.4 Edge Case Handling

| Condition | Behavior |
|-----------|----------|
| Weight at 0.0 | Prevent further LTD |
| Weight at 1.0 | Prevent further LTP |
| LTP/LTD ratio < 1.5 | Reset to defaults (0.1/0.05) |
| LTP/LTD ratio > 5.0 | Reset to defaults (0.1/0.05) |
| No recent events | Apply decay only |
| delta_t outside window | No weight change |
| Circular pathway | Detect and break cycle |

---

## 4. PATHWAY STRUCTURE

### 4.1 Pathway Data Structure

```rust
/// A Hebbian pathway connecting two services
#[derive(Clone, Debug)]
pub struct HebbianPathway {
    /// Unique pathway identifier
    pub id: PathwayId,

    /// Source service identifier
    pub source_id: String,

    /// Target service identifier
    pub target_id: String,

    /// Connection strength [0.0, 1.0]
    pub weight: f64,

    /// Long-term potentiation factor
    pub ltp: f64,

    /// Long-term depression factor
    pub ltd: f64,

    /// Total activation count
    pub activation_count: u64,

    /// Success count
    pub successes: u64,

    /// Failure count
    pub failures: u64,

    /// Last activation timestamp
    pub last_activated: DateTime<Utc>,

    /// Creation timestamp
    pub created_at: DateTime<Utc>,

    /// Success rate (computed)
    pub success_rate: f64,
}
```

### 4.2 Pathway Constants

```rust
pub const MIN_STRENGTH: f64 = 0.0;
pub const MAX_STRENGTH: f64 = 1.0;
pub const DEFAULT_LTP: f64 = 0.1;
pub const DEFAULT_LTD: f64 = 0.05;
pub const INITIAL_WEIGHT: f64 = 0.5;
pub const PRUNE_THRESHOLD: f64 = 0.05;
pub const STDP_WINDOW_MS: i64 = 100;
```

---

## 5. LEARNING CYCLE

### 5.1 Standard Learning Cycle

```
1. Detect activation pair (source, target)
2. Calculate delta_t = t_target - t_source
3. Check if |delta_t| <= STDP_WINDOW
4. Apply STDP rule based on delta_t sign
5. Update pathway weight (bounded)
6. Record learning event
7. Apply decay to inactive pathways
8. Prune pathways below threshold
```

### 5.2 Pathway Operations

| Operation | Condition | Action |
|-----------|-----------|--------|
| Create | New source-target pair | Initialize w = 0.5 |
| Strengthen | Success event | Apply LTP formula |
| Weaken | Failure event | Apply LTD formula |
| Decay | Hourly maintenance | Apply decay formula |
| Prune | w < PRUNE_THRESHOLD | Mark for deletion |
| Archive | last_activated > 30 days | Move to archive |

---

## 6. DATABASE SCHEMA

### 6.1 Pathway Table

```sql
CREATE TABLE hebbian_pathways (
    id INTEGER PRIMARY KEY,
    source_id TEXT NOT NULL,
    target_id TEXT NOT NULL,
    weight REAL DEFAULT 0.5,
    ltp_rate REAL DEFAULT 0.1,
    ltd_rate REAL DEFAULT 0.05,
    activation_count INTEGER DEFAULT 0,
    successes INTEGER DEFAULT 0,
    failures INTEGER DEFAULT 0,
    last_activated TIMESTAMP,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(source_id, target_id),
    CONSTRAINT weight_bounds CHECK (weight >= 0 AND weight <= 1)
);

CREATE INDEX idx_pathways_weight ON hebbian_pathways(weight DESC);
CREATE INDEX idx_pathways_source ON hebbian_pathways(source_id);
CREATE INDEX idx_pathways_target ON hebbian_pathways(target_id);
CREATE INDEX idx_pathways_last_activated ON hebbian_pathways(last_activated);
```

### 6.2 Learning Events Table

```sql
CREATE TABLE learning_events (
    id INTEGER PRIMARY KEY,
    pathway_id INTEGER REFERENCES hebbian_pathways(id),
    event_type TEXT CHECK (event_type IN ('LTP', 'LTD', 'DECAY', 'PRUNE')),
    delta REAL,
    weight_before REAL,
    weight_after REAL,
    delta_t_ms INTEGER,
    session_id TEXT,
    context TEXT,
    timestamp TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_events_pathway ON learning_events(pathway_id);
CREATE INDEX idx_events_type ON learning_events(event_type);
CREATE INDEX idx_events_timestamp ON learning_events(timestamp);
CREATE INDEX idx_events_session ON learning_events(session_id);
```

### 6.3 Storage Location

```
data/
├── hebbian_pulse.db      # SQLite: pathway weights & events
├── pathways_archive.db   # Archived/pruned pathways
└── learning_stats.json   # Aggregate statistics
```

---

## 7. HOMEOSTATIC FEEDBACK SYSTEM

### 7.1 Homeostatic Configuration

```rust
pub struct HomeostaticConfig {
    // Targets
    pub r_target: f64,      // 0.85 (order parameter / health)
    pub c_target: f64,      // 0.90 (coherence / synergy)
    pub s_target: f64,      // 0.85 (success rate)

    // Error weights
    pub w_r: f64,           // 0.3 (health weight)
    pub w_c: f64,           // 0.3 (synergy weight)
    pub w_s: f64,           // 0.4 (success weight)

    // Adaptation rates
    pub alpha: f64,         // 0.01 (coupling rate)
    pub beta: f64,          // 0.005 (range rate)
    pub gamma: f64,         // 0.02 (learning rate)

    // Bounds
    pub ltp_min: f64,       // 0.01
    pub ltp_max: f64,       // 0.3
    pub ltd_min: f64,       // 0.01
    pub ltd_max: f64,       // 0.2
}
```

### 7.2 Error Computation

```
e_r = r_target - r(t)     // Health error
e_c = c_target - c(t)     // Synergy error
e_s = s_target - s(t)     // Success rate error

E = w_r * e_r^2 + w_c * e_c^2 + w_s * e_s^2  // Combined error
```

### 7.3 Adaptation Rules

```
// Adapt LTP/LTD rates based on success
eta_LTP = eta_LTP * (1 + gamma * e_s)  // Increase if underperforming
eta_LTD = eta_LTD * (1 - gamma * e_s)  // Decrease if underperforming

// Clamp to bounds
eta_LTP = clamp(eta_LTP, ltp_min, ltp_max)
eta_LTD = clamp(eta_LTD, ltd_min, ltd_max)

// Maintain healthy ratio
if eta_LTP / eta_LTD < 1.5 OR eta_LTP / eta_LTD > 5.0:
    eta_LTP = DEFAULT_LTP
    eta_LTD = DEFAULT_LTD
```

### 7.4 Homeostatic Targets

| Target | Symbol | Value | Description |
|--------|--------|-------|-------------|
| Health Target | `r_target` | 0.85 | Average service health |
| Synergy Target | `c_target` | 0.90 | Cross-system synergy |
| Success Target | `s_target` | 0.85 | Pathway success rate |
| Alert Threshold | `E_alert` | 0.1 | Combined error threshold |

### 7.5 Safety Mechanisms

| Condition | Action |
|-----------|--------|
| LTP:LTD ratio < 1.5 | Reset to defaults |
| LTP:LTD ratio > 5.0 | Reset to defaults |
| E > 0.1 | Trigger alert |
| Adaptation diverging | Freeze adaptation |
| eta_LTP > ltp_max | Clamp to ltp_max |
| eta_LTD < ltd_min | Clamp to ltd_min |

---

## 8. COLD START HANDLING

### 8.1 Bootstrap Pathways

```json
{
  "version": "1.0",
  "pathways": [
    {"source": "SYNTHEX", "target": "SAN-K7", "weight": 0.7, "source": "bootstrap"},
    {"source": "SYNTHEX", "target": "NAIS", "weight": 0.7, "source": "bootstrap"},
    {"source": "SAN-K7", "target": "SYNTHEX", "weight": 0.7, "source": "bootstrap"},
    {"source": "NAIS", "target": "CodeSynthor", "weight": 0.65, "source": "bootstrap"},
    {"source": "Tool Library", "target": "All", "weight": 0.6, "source": "bootstrap"}
  ]
}
```

### 8.2 Cold Start Detection

```rust
fn detect_cold_start(db: &Database) -> ColdStartStatus {
    let pathway_count = db.count_pathways();
    let event_count = db.count_learning_events_last_24h();

    match (pathway_count, event_count) {
        (0, _) => ColdStartStatus::Fresh,
        (1..=10, 0..=5) => ColdStartStatus::Bootstrapping,
        (_, 0..=10) => ColdStartStatus::Stale,
        _ => ColdStartStatus::Warm,
    }
}
```

### 8.3 Bootstrap Sequence

```
1. Check: detect_cold_start()
2. If Fresh:
   a. Load bootstrap/default_pathways.json
   b. Insert with source="bootstrap"
   c. Set weight = bootstrap_weight * 0.8
3. If Stale:
   a. Apply decay to all weights
   b. Merge bootstrap pathways (if missing)
4. Initialize: Learning event tracking
5. Start: Homeostatic feedback loop
```

---

## 9. PATHWAY PRUNING

### 9.1 Pruning Criteria

| Criterion | Threshold | Action |
|-----------|-----------|--------|
| Weight decay | `weight < 0.05` | Soft delete |
| Inactivity | `last_activated > 30 days` | Archive |
| Low confidence | `successes + failures < 5` | Keep but flag |
| Zero success | `successes = 0 AND failures > 10` | Hard delete |

### 9.2 Pruning Schedule

| Trigger | Frequency | Scope |
|---------|-----------|-------|
| Hourly maintenance | 1 hour | Soft delete only |
| Daily maintenance | 02:00 UTC | Full prune |
| Pathway count > 1000 | On insert | Emergency prune |

---

## 10. LEARNING DIAGNOSTICS

### 10.1 Health Metrics

| Metric | Healthy Range | Alert Threshold |
|--------|---------------|-----------------|
| LTP:LTD ratio | 1.5 - 5.0 | <1.5 or >5.0 |
| Weight distribution median | 0.4 - 0.6 | <0.3 or >0.7 |
| New pathways/day | 5 - 50 | <2 or >100 |
| Pathway decay rate | 0.001 - 0.01 | >0.02 |
| Success rate | 0.7 - 0.95 | <0.5 |

### 10.2 Diagnostic Queries

```sql
-- Check LTP:LTD ratio (last 24h)
SELECT
    SUM(CASE WHEN event_type = 'LTP' THEN 1 ELSE 0 END) AS ltp_count,
    SUM(CASE WHEN event_type = 'LTD' THEN 1 ELSE 0 END) AS ltd_count,
    CAST(SUM(CASE WHEN event_type = 'LTP' THEN 1 ELSE 0 END) AS REAL) /
    NULLIF(SUM(CASE WHEN event_type = 'LTD' THEN 1 ELSE 0 END), 0) AS ratio
FROM learning_events
WHERE timestamp > datetime('now', '-24 hours');

-- Weight distribution
SELECT
    ROUND(weight, 1) AS bucket,
    COUNT(*) AS count
FROM hebbian_pathways
GROUP BY ROUND(weight, 1)
ORDER BY bucket;

-- Strongest pathways
SELECT source_id, target_id, weight,
       CAST(successes AS REAL) / NULLIF(successes + failures, 0) AS success_rate
FROM hebbian_pathways
WHERE weight > 0.7
ORDER BY weight DESC
LIMIT 10;

-- Weakest active pathways (candidates for pruning)
SELECT source_id, target_id, weight, last_activated
FROM hebbian_pathways
WHERE weight < 0.2 AND last_activated > datetime('now', '-7 days')
ORDER BY weight ASC
LIMIT 10;
```

### 10.3 Learning Report Schema

```markdown
# Learning Report: {date}

## Summary
- Active pathways: {count}
- LTP events (24h): {ltp_count}
- LTD events (24h): {ltd_count}
- LTP:LTD ratio: {ratio}
- Pathways pruned: {pruned}

## Top Pathways
1. {source} -> {target}: {weight} ({success_rate}% success)
...

## Anomalies
- {anomaly_description}
...

## Recommendations
- {recommendation}
...
```

---

## 11. RUST IMPLEMENTATION

### 11.1 STDP Learner

```rust
/// Spike-Timing Dependent Plasticity implementation
pub struct StdpLearner {
    pub window_ms: i64,
    pub max_delta: f64,
    pub tau_plus: f64,
    pub tau_minus: f64,
    pub a_plus: f64,
    pub a_minus: f64,
}

impl Default for StdpLearner {
    fn default() -> Self {
        Self {
            window_ms: 100,
            max_delta: 0.1,
            tau_plus: 20.0,
            tau_minus: 20.0,
            a_plus: 0.01,
            a_minus: 0.012,
        }
    }
}

impl StdpLearner {
    /// Calculate weight change based on spike timing
    pub fn calculate_delta(&self, delta_t_ms: i64) -> f64 {
        if delta_t_ms.abs() > self.window_ms {
            return 0.0;
        }

        let delta_t = delta_t_ms as f64;

        if delta_t > 0.0 {
            // Causal: pre before post -> LTP
            self.a_plus * (-delta_t / self.tau_plus).exp()
        } else if delta_t < 0.0 {
            // Anti-causal: post before pre -> LTD
            -self.a_minus * (delta_t / self.tau_minus).exp()
        } else {
            0.0
        }
    }

    /// Apply STDP update to a pathway
    pub fn apply(&self, pathway: &mut HebbianPathway, delta_t_ms: i64) {
        let delta = self.calculate_delta(delta_t_ms);

        if delta > 0.0 {
            pathway.weight = (pathway.weight + delta * (1.0 - pathway.weight))
                .min(MAX_STRENGTH);
            pathway.successes += 1;
        } else if delta < 0.0 {
            pathway.weight = (pathway.weight + delta * pathway.weight)
                .max(MIN_STRENGTH);
            pathway.failures += 1;
        }

        pathway.activation_count += 1;
        pathway.last_activated = Utc::now();
        pathway.success_rate = pathway.successes as f64
            / (pathway.successes + pathway.failures).max(1) as f64;
    }
}
```

### 11.2 Homeostatic Controller

```rust
pub struct HomeostaticController {
    config: HomeostaticConfig,
    current_ltp: f64,
    current_ltd: f64,
}

impl HomeostaticController {
    pub fn new(config: HomeostaticConfig) -> Self {
        Self {
            config,
            current_ltp: DEFAULT_LTP,
            current_ltd: DEFAULT_LTD,
        }
    }

    /// Update learning rates based on system state
    pub fn adapt(&mut self, health: f64, synergy: f64, success_rate: f64) {
        let e_r = self.config.r_target - health;
        let e_c = self.config.c_target - synergy;
        let e_s = self.config.s_target - success_rate;

        let combined_error = self.config.w_r * e_r.powi(2)
            + self.config.w_c * e_c.powi(2)
            + self.config.w_s * e_s.powi(2);

        // Adapt rates
        self.current_ltp *= 1.0 + self.config.gamma * e_s;
        self.current_ltd *= 1.0 - self.config.gamma * e_s;

        // Clamp to bounds
        self.current_ltp = self.current_ltp
            .clamp(self.config.ltp_min, self.config.ltp_max);
        self.current_ltd = self.current_ltd
            .clamp(self.config.ltd_min, self.config.ltd_max);

        // Enforce ratio
        let ratio = self.current_ltp / self.current_ltd;
        if ratio < 1.5 || ratio > 5.0 {
            self.current_ltp = DEFAULT_LTP;
            self.current_ltd = DEFAULT_LTD;
        }

        // Alert if error too high
        if combined_error > 0.1 {
            tracing::warn!(
                "Homeostatic error high: E={:.4}, health={:.2}, synergy={:.2}, success={:.2}",
                combined_error, health, synergy, success_rate
            );
        }
    }

    pub fn get_rates(&self) -> (f64, f64) {
        (self.current_ltp, self.current_ltd)
    }
}
```

---

## 12. CONFIGURATION FILES

### 12.1 TOML Configuration

```toml
[learning.stdp]
window_ms = 100
ltp_rate = 0.1
ltd_rate = 0.05
tau_plus = 20.0
tau_minus = 20.0
a_plus = 0.01
a_minus = 0.012
decay_rate = 0.001
prune_threshold = 0.05

[learning.homeostatic]
r_target = 0.85
c_target = 0.90
s_target = 0.85
w_r = 0.3
w_c = 0.3
w_s = 0.4
alpha = 0.01
beta = 0.005
gamma = 0.02
ltp_min = 0.01
ltp_max = 0.3
ltd_min = 0.01
ltd_max = 0.2

[learning.maintenance]
prune_interval_hours = 24
decay_interval_hours = 1
max_pathways = 1000
archive_after_days = 30
```

---

## 13. CROSS-SPEC DEPENDENCIES

| Dependency | Source Spec | Integration Point |
|------------|-------------|-------------------|
| 12D Tensor D6 (health) | TENSOR_SPEC | Homeostatic r_target |
| 12D Tensor D8 (synergy) | TENSOR_SPEC | Homeostatic c_target |
| Database storage | DATABASE_SPEC | hebbian_pulse.db |
| Pipeline PL-HEBBIAN-001 | PIPELINE_SPEC | Learning event processing |
| NAM R2 HebbianRouting | NAM_SPEC | Pathway-weighted routing |
| Learning Patterns | patterns/LEARNING_PATTERNS.md | Implementation patterns |

---

## 14. VERSION HISTORY

| Version | Date | Changes |
|---------|------|---------|
| 2.0.0 | 2026-01-28 | Complete rewrite with homeostasis, cold start, diagnostics |
| 1.0.0 | 2026-01-28 | Initial STDP parameters |

---

*Generated: 2026-01-28 | The Maintenance Engine v1.0.0*
