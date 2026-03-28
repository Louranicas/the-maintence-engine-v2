# Module 26: STDP Processor

> **M26_STDP_PROCESSOR** | Layer 5: Learning | [Back to Index](INDEX.md)

## Overview

The STDP Processor implements Spike-Timing Dependent Plasticity (STDP), a fundamental learning mechanism where the timing between pre-synaptic and post-synaptic events determines whether a pathway is strengthened (LTP) or weakened (LTD). This creates precise, timing-aware pathway modifications based on temporal coincidence.

## Layer Context

| Property | Value |
|----------|-------|
| Layer | L5: Learning |
| Module ID | M26 |
| Source | `src/m5_learning/mod.rs` |
| Purpose | Spike-timing dependent plasticity implementation |
| Type | Temporal learning engine |

## STDP Configuration

| Parameter | Value | Description |
|-----------|-------|-------------|
| LTP Rate | 0.1 | Per-spike potentiation magnitude |
| LTD Rate | 0.05 | Per-spike depression magnitude |
| STDP Window | 100ms | Coincidence detection window |
| Decay Rate | 0.001 | Background decay per cycle |
| Timing Sensitivity | YES | Precise millisecond-level timing |

## STDP Mechanism

### Long-Term Potentiation (LTP)

Triggered when:
- Pre-synaptic event precedes post-synaptic event within STDP window
- Action on connection succeeds (reinforcement)
- System health improves after pathway activation

**Effect:** Increases pathway strength by `ltp_rate` (max 1.0)

```
Pre-synaptic activity
          |
          | (within 100ms)
          ↓
Post-synaptic activity
          |
          ↓
    Pathway Strengthened (+0.1)
```

### Long-Term Depression (LTD)

Triggered when:
- Post-synaptic event precedes pre-synaptic event (reversed timing)
- Action on connection fails (punishment)
- System health degrades after pathway activation

**Effect:** Decreases pathway strength by `ltd_rate` (min 0.1)

```
Post-synaptic activity
          |
          | (within 100ms, reversed)
          ↓
Pre-synaptic activity
          |
          ↓
    Pathway Weakened (-0.05)
```

## Core STDP Types

### StdpConfig

```rust
pub struct StdpConfig {
    pub ltp_rate: f64,              // 0.1 - Potentiation per spike
    pub ltd_rate: f64,              // 0.05 - Depression per spike
    pub stdp_window_ms: u64,        // 100 - Timing window in milliseconds
    pub decay_rate: f64,            // 0.001 - Background decay
}
```

### Pathway STDP Fields

```rust
pub struct HebbianPathway {
    pub ltp_count: u64,             // Count of LTP events
    pub ltd_count: u64,             // Count of LTD events
    pub stdp_delta: f64,            // Accumulated timing-based changes
    pub strength: f64,              // Current strength (0.0 - 1.0)
    pub last_activation: SystemTime, // For timing calculations
}
```

## STDP Algorithm

### 1. Event Detection

```
Monitor system events:
- Pre-synaptic: Source module activation
- Post-synaptic: Target module activation (output)
- Calculate delta_t = post_time - pre_time
```

### 2. Timing Analysis

```
If abs(delta_t) <= STDP_WINDOW (100ms):
    If delta_t > 0:
        LTP condition met (pre → post)
    Else:
        LTD condition met (post → pre)
Else:
    No plasticity change
```

### 3. Strength Modification

```
If LTP:
    new_strength = min(strength + ltp_rate, 1.0)
    ltp_count += 1

If LTD:
    new_strength = max(strength - ltd_rate, 0.1)
    ltd_count += 1

Background decay:
    new_strength -= decay_rate
```

## API

### Configuration

```rust
// Standard STDP configuration
impl Default for StdpConfig {
    fn default() -> Self {
        Self {
            ltp_rate: 0.1,
            ltd_rate: 0.05,
            stdp_window_ms: 100,
            decay_rate: 0.001,
        }
    }
}
```

### LTP Application

```rust
// Strengthen pathway (success case)
pub fn apply_ltp(&mut self, config: &StdpConfig) {
    self.strength = (self.strength + config.ltp_rate).min(1.0);
    self.ltp_count += 1;
    self.last_activation = Some(SystemTime::now());
}
```

### LTD Application

```rust
// Weaken pathway (failure case)
pub fn apply_ltd(&mut self, config: &StdpConfig) {
    self.strength = (self.strength - config.ltd_rate).max(0.1);
    self.ltd_count += 1;
    self.last_activation = Some(SystemTime::now());
}
```

### Success/Failure Recording

```rust
// Record successful activation (triggers LTP)
pub fn record_success(&mut self, config: &StdpConfig) {
    self.success_count += 1;
    self.activation_count += 1;
    self.apply_ltp(config);
    self.last_success = Some(SystemTime::now());
}

// Record failed activation (triggers LTD)
pub fn record_failure(&mut self, config: &StdpConfig) {
    self.failure_count += 1;
    self.activation_count += 1;
    self.apply_ltd(config);
}
```

## STDP in Action

### Example: Service Restart Pathway

```
Timeline:
T=0ms:     maintenance module activation (pre-synaptic)
T=50ms:    service_restart executes
T=55ms:    service health improves (post-synaptic)
T=105ms:   exceeds STDP window

Analysis:
delta_t = 105ms - 50ms = 55ms
Within STDP window (100ms)? YES
Delta_t positive (pre→post)? YES → LTP condition

Action:
Apply LTP: strength = 0.5 + 0.1 = 0.6
ltp_count incremented
last_activation updated
```

## Learning Dynamics

### LTP:LTD Ratio Analysis

The ratio of LTP to LTD events indicates pathway success:

```
LTP:LTD = 2:1 → Strong positive learning
LTP:LTD = 1:1 → Neutral/oscillating
LTP:LTD = 1:2 → Weak, declining pathway
```

### Strength Evolution

Pathways evolve over multiple activation cycles:

```
Cycle 1: 0.5 → Success → 0.6 (LTP)
Cycle 2: 0.6 → Success → 0.7 (LTP)
Cycle 3: 0.7 → Success → 0.8 (LTP)
Cycle 4: 0.8 → Failure → 0.75 (LTD)
Cycle 5: 0.75 → Success → 0.85 (LTP)
```

## Integration with Other Modules

### M25 (Hebbian Manager)
STDP Processor executes the low-level plasticity rules that Hebbian Manager uses to modify pathways.

### M27 (Pattern Recognizer)
Pattern recognition triggers STDP events when patterns are detected and outcomes measured.

### M28 (Pathway Pruner)
STDP-weakened pathways (LTD accumulation) become pruning candidates.

### M29 (Memory Consolidator)
Pathways with high LTP counts are candidates for promotion to higher memory layers.

## Biological Inspiration

STDP is modeled on biological synaptic plasticity in vertebrate brains:
- **Pre-post timing** determines synaptic strengthening
- **Millisecond precision** creates learning specificity
- **Bidirectional modification** allows both potentiation and depression
- **Activity-dependent** ensures learning only occurs with relevant activity

---

*[Back to Index](INDEX.md)*
