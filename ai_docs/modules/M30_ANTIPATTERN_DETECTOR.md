# Module 30: Anti-Pattern Detector

> **M30_ANTIPATTERN_DETECTOR** | Layer 5: Learning | [Back to Index](INDEX.md)

## Overview

The Anti-Pattern Detector identifies negative reinforcement patterns and harmful pathway associations that lead to undesired outcomes. It implements negative feedback mechanisms to prevent the system from learning and strengthening counterproductive patterns. This module acts as a "critic" in the learning system, preventing false positive learning and protecting system stability.

## Layer Context

| Property | Value |
|----------|-------|
| Layer | L5: Learning |
| Module ID | M30 |
| Source | `src/m5_learning/mod.rs` |
| Purpose | Negative reinforcement and anti-pattern detection |
| Type | Protective learning filter |

## STDP Configuration

| Parameter | Value | Description |
|-----------|-------|-------------|
| LTP Rate | 0.1 | Blocked for anti-patterns |
| LTD Rate | 0.05 | Aggressively applied |
| STDP Window | 100ms | Detection window |
| Decay Rate | 0.001 | Anti-patterns decay faster |

## Anti-Pattern Architecture

### Pattern Classification

The Anti-Pattern Detector identifies and categorizes harmful patterns:

```
Anti-Patterns
├─ False Positives
│  ├─ Patterns matching but leading to failures
│  ├─ Coincidental associations
│  └─ Temporal artifacts
│
├─ Counterproductive Patterns
│  ├─ Patterns that degrade performance
│  ├─ Patterns that increase errors
│  └─ Patterns that conflict with goals
│
├─ Cascading Failure Patterns
│  ├─ Patterns triggering multiple failures
│  ├─ Patterns with negative side effects
│  └─ Patterns destabilizing the system
│
└─ Learning Aberrations
   ├─ Over-fitting to noise
   ├─ System-wide pathological learning
   └─ Divergent pathway strengthening
```

## Anti-Pattern Detection Mechanisms

### 1. Outcome Monitoring

```
Pathway Activation
        ↓
Execute Associated Action
        ↓
Monitor System Metrics
        ↓
Evaluate Outcomes:
  ✓ Health improved? → Normal pathway (LTP)
  ✗ Health degraded? → Potential anti-pattern
  ✗ Latency increased? → Potential anti-pattern
  ✗ Errors increased? → Potential anti-pattern
  ✗ Multiple failures? → Confirmed anti-pattern
```

### 2. Consequence Analysis

```
Pathway: memory_pressure → session_rotation
Activation: YES
Expected: Memory reduced
Actual: Memory increased by 40%
Consequence: Negative

Anti-Pattern Score:
= (expected - actual) / expected
= (reduce 100MB - increase 40MB) / reduce 100MB
= -1.4 (highly negative)

Action: Apply anti-pattern suppression
```

### 3. Cascading Failure Detection

```
Primary Failure:
  Pathway A → Service X (failed)
              ↓
Cascading to:
  Service Y health drops
  Service Z latency spikes
  Quorum voting slowed
              ↓
Secondary Failures Detected

Anti-Pattern Level: SEVERE
Action: Immediate suppression
```

## Core Anti-Pattern Types

### Type 1: False Positives

```
Pattern Signature:
├─ Pattern matches context
├─ Pathway activates
├─ Expected outcome: positive
└─ Actual outcome: neutral or negative

Example:
  Pattern: "CPU spike detected"
  Pathway: CPU spike → process termination
  Expected: CPU reduction
  Actual: Service crash (worse)

Detection Trigger: outcome_worse_than_baseline
```

### Type 2: Counterproductive Patterns

```
Pattern Signature:
├─ Pattern matches correctly
├─ Pathway activates
├─ Intended goal: A
├─ Result: not A, or -A

Example:
  Pattern: "Memory pressure detected"
  Pathway: memory_pressure → aggressive_garbage_collection
  Intended: Reduce memory usage
  Result: Increased GC overhead, memory still high

Detection Trigger: goal_not_achieved
```

### Type 3: Cascading Failure Patterns

```
Pattern Signature:
├─ Primary pathway succeeds locally
├─ Secondary systems negatively affected
├─ Health score drops across system
├─ Multiple repair attempts triggered

Example:
  Pathway: consensus_stall → kill_slow_node
  Local: Removes stalled node
  Cascade: Quorum lost, voting unable to complete
           System enters worse state
           Multiple recovery attempts fail

Detection Trigger: system_health_decline
```

### Type 4: Learning Aberrations

```
Pattern Signature:
├─ Pathway strength increases despite poor outcomes
├─ LTP:LTD ratio inverted (more LTD than LTP)
├─ Pattern diverges from expected behavior
├─ System learning becomes pathological

Example:
  Pathway: error_detected → ignore_error
  Activation Count: 50
  Success Rate: 0.02 (2%)
  Strength: 0.85 (strong despite failures)
  
  Issue: Miscalibrated LTP/LTD application
         Learning algorithm failure

Detection Trigger: strength_quality_mismatch
```

## API

### Anti-Pattern Detection

```rust
// Evaluate if outcome represents anti-pattern
pub fn detect_antipattern(&self, 
    expected_outcome: f64,
    actual_outcome: f64
) -> bool

// Calculate anti-pattern severity
pub fn calculate_antipattern_score(&self) -> f64
// Range: -1.0 (severe) to 0.0 (none)

// Check for cascading failures
pub fn detect_cascade_failure(
    primary_pathway: &HebbianPathway,
    affected_services: &[ServiceMetric]
) -> bool
```

### Pattern Suppression

```rust
// Block a pathway from further activation
pub fn suppress_antipattern(&mut self, reason: &str)

// Apply aggressive LTD to anti-pattern
pub fn apply_antipattern_ltd(&mut self, config: &StdpConfig)
// Applies LTD multiple times per cycle

// Revert pathway to pre-activation state
pub fn rollback_antipattern_learning(&mut self, 
    previous_strength: f64
)
```

## Anti-Pattern Workflow

### Detection Phase

```
1. Pathway Activation
   └─ Record pre-activation metrics
      ├─ System health
      ├─ Performance baseline
      ├─ Error rate
      └─ Resource usage

2. Action Execution
   └─ Execute pathway's associated action

3. Outcome Measurement
   └─ Monitor metrics for period T (typically 100ms - 1s)
      ├─ Did health improve?
      ├─ Did performance degrade?
      ├─ Did errors increase?
      └─ Did side effects occur?

4. Analysis
   └─ Compare actual vs expected outcomes
      ├─ Outcome score: actual / expected
      ├─ Consequence severity
      ├─ Cascade potential
      └─ Anti-pattern confidence
```

### Response Phase

```
If Anti-Pattern Detected:

1. Immediate Actions
   ├─ Block pathway from further use
   ├─ Flag as suspicious
   ├─ Log event details
   └─ Alert monitoring system

2. Suppression
   ├─ Apply aggressive LTD (2x-5x normal)
   ├─ Reduce strength toward minimum
   ├─ Prevent further strengthening
   └─ Mark for isolation

3. Analysis
   ├─ Categorize anti-pattern type
   ├─ Identify root cause
   ├─ Check for cascading effects
   └─ Record for future reference

4. Recovery
   ├─ Activate alternative pathways
   ├─ Restore system to baseline
   ├─ Monitor for continued issues
   └─ Prevent re-activation
```

## Anti-Pattern Examples

### Example 1: Service Restart Loop

```
Anti-Pattern Detected: Cascading Restart Failure

Timeline:
T=0ms:    Health failure detected
          → service_restart pathway activated
T=10ms:   Service restarted
T=50ms:   Service comes up
T=60ms:   Service fails immediately
T=70ms:   Health still poor
          → Pattern matches again
          → service_restart pathway re-activates
T=80ms:   Service restarting again
T=150ms:  Service fails again
T=160ms:  Restart loop detected

Anti-Pattern Analysis:
├─ Expected: health improvement
├─ Actual: health oscillation
├─ Confidence: HIGH (6 consecutive failures)
├─ Severity: CRITICAL
└─ Type: Cascading Failure Pattern

Response:
1. Suppress service_restart pathway
2. Apply aggressive LTD: strength 0.6 → 0.2
3. Activate alternative: investigate_service_logs
4. Block pathway until manual review
5. Alert system administrator
```

### Example 2: Memory Thrashing

```
Anti-Pattern Detected: Memory Pressure Loop

Timeline:
T=0ms:    Memory pressure detected (85% utilization)
          → memory_pressure → aggressive_gc pathway
T=20ms:   Garbage collection runs
T=50ms:   Memory drops to 60%
T=51ms:   GC stops
T=200ms:  Memory climbs back to 88%
T=210ms:  Pattern matches again
          → aggressive_gc pathway re-activates
T=230ms:  GC runs again
T=300ms:  Memory oscillates 55%-90%

Anti-Pattern Analysis:
├─ Expected: sustained memory reduction
├─ Actual: memory oscillation
├─ Root Cause: GC not fixing underlying issue
├─ Confidence: HIGH
├─ Type: Counterproductive Pattern

Response:
1. Suppress aggressive_gc pathway (strength: 0.7 → 0.15)
2. Investigate root cause: memory leak?
3. Activate diagnostic pathway
4. Reduce GC activation threshold
5. Monitor for improvement
```

### Example 3: Voting Deadlock

```
Anti-Pattern Detected: Consensus Failure Loop

Timeline:
T=0s:     Consensus proposal submitted
          → consensus_proposal → agent_vote pathway
T=1s:     Voting begins
T=3s:     Deadlock detected (split vote)
          → dissent_detected → learning_update pathway
T=5s:     System attempts consensus again
T=6s:     Voting begins again
T=8s:     Deadlock again

Anti-Pattern Analysis:
├─ Expected: quorum reached, consensus achieved
├─ Actual: repeated deadlock
├─ Severity: HIGH (blocks system progress)
├─ Type: Cascading Failure Pattern

Response:
1. Suppress consensus_proposal pathway
2. Reduce proposal frequency
3. Enable consensus_timeout pathway
4. Allow external arbitration
5. Log for consensus algorithm review
```

## Anti-Pattern Statistics

```rust
pub struct AntiPatternStats {
    pub total_detected: u32,           // Total anti-patterns found
    pub false_positives: u32,          // Type 1: False positives
    pub counterproductive: u32,        // Type 2: Counterproductive
    pub cascading_failures: u32,       // Type 3: Cascading
    pub learning_aberrations: u32,     // Type 4: Learning errors
    
    pub pathways_suppressed: u32,      // Pathways blocked
    pub ltp_blocks: u32,               // LTP applications blocked
    pub rollbacks_executed: u32,       // Learning reversals
    
    pub false_positive_rate: f64,      // Suppressed correctly / total
    pub avg_detection_time: Duration,  // Time to detect
}
```

## Safety Mechanisms

### Prevention Strategies

```
1. Confidence Thresholding
   └─ Only suppress anti-patterns with high confidence
      └─ Prevents over-suppression of edge cases

2. Reversibility
   └─ Log all anti-pattern decisions
      └─ Allow reversal if diagnosis was wrong

3. Gradual Suppression
   └─ Apply incremental LTD rather than immediate blocking
      └─ Allows system recovery time

4. Cascading Effect Analysis
   └─ Monitor for system-wide impact
      └─ Escalate if multiple systems affected
```

### Protective Thresholds

| Threshold | Value | Purpose |
|-----------|-------|---------|
| Confidence Minimum | 0.8 | Suppress only high-confidence anti-patterns |
| Severity Threshold | MEDIUM | Suppress at MEDIUM and above severity |
| Failure Count | ≥ 3 | Require 3+ failures before classification |
| Time Window | 1000ms | Evaluate outcomes within 1 second |

## Related Modules

- **M25_HEBBIAN_MANAGER** - Creates pathways anti-detector protects
- **M26_STDP_PROCESSOR** - Applies aggressive LTD to anti-patterns
- **M27_PATTERN_RECOGNIZER** - Identifies patterns that become anti-patterns
- **M28_PATHWAY_PRUNER** - Removes persistently suppressed anti-patterns
- **M29_MEMORY_CONSOLIDATOR** - Prevents promotion of anti-pattern pathways

## Integration with NAM-03 (Dissent Capture)

The Anti-Pattern Detector integrates with the consensus system's dissent capture:

```
System Decision:    Activate pathway X
Anti-Detector:      Detects anti-pattern risk
Dissent Recorded:   "Pathway X contains anti-pattern (confidence: 0.87)"
Consensus Required: Quorum votes on suppression
Outcome:           Pathway suppressed or allowed based on quorum
```

This ensures distributed agreement on suppressing potentially harmful pathways.

---

*[Back to Index](INDEX.md)*
