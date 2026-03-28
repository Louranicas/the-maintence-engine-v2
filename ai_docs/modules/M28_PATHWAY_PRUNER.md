# Module 28: Pathway Pruner

> **M28_PATHWAY_PRUNER** | Layer 5: Learning | [Back to Index](INDEX.md)

## Overview

The Pathway Pruner maintains the health of the learning system by removing weak, ineffective, or stale pathways. It implements intelligent cleanup logic that prevents pathway network bloat while preserving valuable learned associations. This module ensures the system forgets ineffective strategies while retaining learned successes.

## Layer Context

| Property | Value |
|----------|-------|
| Layer | L5: Learning |
| Module ID | M28 |
| Source | `src/m5_learning/mod.rs` |
| Purpose | Weak pathway cleanup and network maintenance |
| Type | Network health manager |

## STDP Configuration

| Parameter | Value | Description |
|-----------|-------|-------------|
| LTP Rate | 0.1 | Used during cleanup analysis |
| LTD Rate | 0.05 | Indicates pathways for pruning |
| STDP Window | 100ms | Timing reference |
| Decay Rate | 0.001 | Contributes to pathway weakening |

## Pruning Decision Algorithm

### Pruning Criteria

A pathway becomes a pruning candidate when:

```rust
pub fn should_prune(&self, min_strength: f64, inactive_days: u64) -> bool {
    // Criterion 1: Strength below threshold
    if self.strength < min_strength {
        // Criterion 2: Inactive for specified duration
        if let Some(last) = self.last_success {
            if let Ok(duration) = SystemTime::now().duration_since(last) {
                return duration.as_secs() > inactive_days * 86400;
            }
        }
        return true;  // No success record = prune immediately
    }
    false  // Pathway is viable
}
```

### Pruning Thresholds

| Threshold | Default | Meaning |
|-----------|---------|---------|
| min_strength | 0.1-0.3 | Strength below this value |
| inactive_days | 7-30 | No successful use in N days |
| minimum_count | 5 | At least 5 activations before pruning |
| success_ratio | 0.2 | Less than 20% success rate |

## Pruning Decision Matrix

```
Pathway Evaluation:

┌─────────────────────────────────────────────────────────────┐
│ Is strength >= min_strength?                                │
├────────────────┬─────────────────────────────────────────────┤
│ YES            │ KEEP pathway (viable)                       │
│ NO             │ Check inactivity                            │
├────────────────┼─────────────────────────────────────────────┤
│                │ Has it succeeded in last N days?            │
├────────────────┼─────────────────────────────────────────────┤
│ YES            │ KEEP pathway (recently useful)              │
│ NO             │ Check minimum activity                      │
├────────────────┼─────────────────────────────────────────────┤
│                │ Activation count >= minimum?                │
├────────────────┼─────────────────────────────────────────────┤
│ YES            │ PRUNE pathway (weak & stale)                │
│ NO             │ KEEP pathway (insufficient data)            │
└────────────────┴─────────────────────────────────────────────┘
```

## Pathway Health Metrics

### Health Indicators

```rust
pub struct HebbianPathway {
    pub strength: f64,                     // Overall viability
    pub success_count: u64,                // Successful activations
    pub failure_count: u64,                // Failed activations
    pub activation_count: u64,             // Total uses
    pub ltp_count: u64,                    // Strengthening events
    pub ltd_count: u64,                    // Weakening events
    pub last_success: Option<SystemTime>,  // Recency metric
    pub last_activation: Option<SystemTime>, // Usage tracking
}
```

### Health Scoring

```
Health Score = (strength * 0.5) + (success_rate * 0.3) + (recency_factor * 0.2)

Where:
- strength: 0.0-1.0 (pathway strength)
- success_rate: success_count / (success_count + failure_count)
- recency_factor: 1.0 - (days_inactive / max_days)
```

## Pruning Workflow

### Phase 1: Pathway Audit

```
For each pathway in system:
    1. Calculate health score
    2. Check strength threshold
    3. Check inactivity period
    4. Evaluate activation history
    5. Determine pruning eligibility
```

### Phase 2: Candidate Identification

```
Identify candidates for removal:
┌─ Strength-based: strength < 0.1-0.3
├─ Inactivity-based: no success in 7-30 days
├─ Failure-based: failure_rate > 80%
└─ Obsolescence-based: never successfully activated
```

### Phase 3: Safety Check

```
Before pruning, verify:
✓ Pathway has minimum activation count
✓ Pathway not recently successful
✓ Alternative pathways exist (if critical)
✓ No dependent systems rely solely on pathway
✓ Health system not degraded if removed
```

### Phase 4: Pruning Execution

```
For each approved candidate:
    1. Log pathway removal (for analysis)
    2. Remove from active pathway network
    3. Archive statistics
    4. Update service routing tables
    5. Confirm removal completed
```

## Pruning Policies

### Conservative Policy (Production)

```
Thresholds:
- min_strength: 0.2
- inactive_days: 14
- minimum_activations: 10
- success_rate_minimum: 0.3

Frequency: Weekly audit
Impact: Slow pruning, maximum pathway retention
```

### Moderate Policy (Standard)

```
Thresholds:
- min_strength: 0.15
- inactive_days: 7
- minimum_activations: 5
- success_rate_minimum: 0.2

Frequency: Bi-weekly audit
Impact: Balanced pruning and retention
```

### Aggressive Policy (Cleanup)

```
Thresholds:
- min_strength: 0.1
- inactive_days: 3
- minimum_activations: 2
- success_rate_minimum: 0.1

Frequency: Daily audit
Impact: Rapid cleanup, aggressive learning
```

## Example Pruning Scenarios

### Scenario 1: Weak, Inactive Pathway

```
Pathway: latency_spike → disk_flush
Strength: 0.12
Last Success: 18 days ago
Activation Count: 3
Success Rate: 1/3 = 0.333

Evaluation:
- Strength < 0.2? YES (0.12 < 0.2)
- Inactive > 7 days? YES (18 > 7)
- Min activations met? NO (3 < 5 under moderate)

Decision: KEEP (insufficient data to prune)
Reason: Only 3 activations, need more evidence
```

### Scenario 2: Persistent Failure Pathway

```
Pathway: cache_miss → cache_rebuild
Strength: 0.08
Last Success: 45 days ago
Activation Count: 50
Success Rate: 5/50 = 0.1

Evaluation:
- Strength < 0.2? YES (0.08 < 0.2)
- Inactive > 7 days? YES (45 > 7)
- Min activations met? YES (50 >> 5)

Decision: PRUNE
Reason: Consistent failure over long period, no recent success
```

### Scenario 3: Recently Active Pathway

```
Pathway: consensus_proposal → agent_vote
Strength: 0.15
Last Success: 2 days ago
Activation Count: 20
Success Rate: 16/20 = 0.8

Evaluation:
- Strength < 0.2? YES (0.15 < 0.2)
- Inactive > 7 days? NO (2 < 7)

Decision: KEEP
Reason: Recently succeeded, still valuable despite low strength
```

### Scenario 4: Strong, Active Pathway

```
Pathway: health_failure → service_restart
Strength: 0.92
Last Success: 1 hour ago
Activation Count: 127
Success Rate: 115/127 = 0.906

Evaluation:
- Strength < 0.2? NO (0.92 >> 0.2)

Decision: KEEP
Reason: High strength and recent success
```

## Impact Analysis

### Benefits of Pruning

- **Network Efficiency**: Reduces pathway lookup time
- **Memory Conservation**: Removes unnecessary pathway storage
- **Decision Speed**: Fewer pathways to evaluate
- **Learning Focus**: System focuses on viable patterns
- **Cognitive Clarity**: Fewer conflicting pathways

### Risks of Aggressive Pruning

- **Knowledge Loss**: Removing potentially useful pathways
- **Relearning**: System must relearn pruned pathways
- **Service Disruption**: Critical pathways mistakenly removed
- **Learning Lag**: Reduced learning rate during recovery

## Pruning Triggers

### Scheduled Pruning

```
Weekly audit (conservative)
├─ Executed during low-activity periods
├─ Analyzes all pathways
├─ Batches pruning operations
└─ Logs all changes
```

### Triggered Pruning

```
On-demand triggers:
├─ Memory usage exceeds threshold
├─ Pathway count exceeds maximum
├─ System performance degrades
├─ Manual administrator request
└─ Learning system health check
```

### Adaptive Pruning

```
Based on system state:
├─ During high load: aggressive pruning
├─ During low load: conservative pruning
├─ After failures: protective (minimal pruning)
├─ During learning phases: none
└─ During consolidation: aggressive
```

## API

### Pruning Decision

```rust
pub fn should_prune(&self, min_strength: f64, inactive_days: u64) -> bool
```

### Pruning Policy Configuration

```rust
pub struct PruningPolicy {
    pub min_strength: f64,
    pub inactive_days: u64,
    pub minimum_activations: u64,
    pub success_rate_minimum: f64,
    pub frequency: PruneFrequency,
    pub enable_safety_check: bool,
}
```

## Related Modules

- **M25_HEBBIAN_MANAGER** - Creates pathways that pruner maintains
- **M26_STDP_PROCESSOR** - Weakens pathways through LTD (candidates for pruning)
- **M27_PATTERN_RECOGNIZER** - Failed patterns become pruning candidates
- **M29_MEMORY_CONSOLIDATOR** - Promotes strong pathways, marks weak for pruning
- **M30_ANTIPATTERN_DETECTOR** - Identifies false patterns for pruning

## Pruning Statistics

### Monitoring Pathways

```rust
pub struct PruningStats {
    pub total_pathways: u32,
    pub pruned_count: u32,
    pub kept_count: u32,
    pub prune_rate: f64,  // pruned / total
    pub avg_age_pruned: Duration,
    pub avg_strength_pruned: f64,
}
```

---

*[Back to Index](INDEX.md)*
