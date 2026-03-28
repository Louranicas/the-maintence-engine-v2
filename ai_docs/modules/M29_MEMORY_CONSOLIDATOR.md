# Module 29: Memory Consolidator

> **M29_MEMORY_CONSOLIDATOR** | Layer 5: Learning | [Back to Index](INDEX.md)

## Overview

The Memory Consolidator manages the progression of learned pathways and patterns through multiple memory layers, from volatile working memory to persistent long-term storage. It implements biological memory consolidation, where repeated or important memories are transferred to more stable storage, enabling system-wide learning retention and knowledge persistence.

## Layer Context

| Property | Value |
|----------|-------|
| Layer | L5: Learning |
| Module ID | M29 |
| Source | `src/m5_learning/mod.rs` |
| Purpose | Memory layer management and consolidation |
| Type | Memory hierarchy manager |

## STDP Configuration

| Parameter | Value | Description |
|-----------|-------|-------------|
| LTP Rate | 0.1 | Used during promotion to higher layers |
| LTD Rate | 0.05 | Used during demotion to lower layers |
| STDP Window | 100ms | Temporal coordination |
| Decay Rate | 0.001 | Layer-based retention decay |

## Memory Layer Architecture

### Memory Layers

```rust
pub enum MemoryLayer {
    Working,          // Active, volatile (seconds)
    ShortTerm,        // Minutes to hours
    LongTerm,         // Persistent (days/months)
    Episodic,         // Event sequences, contexts
}
```

### Layer Characteristics

| Layer | Duration | Capacity | Access Speed | Persistence |
|-------|----------|----------|--------------|------------|
| **Working** | 0-30s | Unlimited | Instant | Volatile |
| **ShortTerm** | 1h-24h | ~10K pathways | Fast | Degrading |
| **LongTerm** | 30d-years | Unlimited | Slower | Persistent |
| **Episodic** | Event-based | Unbounded | Variable | Contextual |

## Memory Consolidation Process

### Phase 1: Working Memory

```
Pathway Creation
        ↓
Working Memory (active, volatile)
- Raw sensory-equivalent data
- Immediate system events
- Real-time activations
- Short duration (~30 seconds)
```

**Properties:**
- Fastest access
- Lowest storage cost
- No persistence
- Continuously overwritten

### Phase 2: Short-Term Memory

```
Repeated Activation or Importance Signal
        ↓
Short-Term Memory (hours to days)
- Consolidated from working memory
- Accessed multiple times
- Recognized patterns
- Moderate persistence
```

**Properties:**
- Still volatile but longer retention
- Database-backed storage
- Moderate access cost
- Natural decay over time

### Phase 3: Long-Term Memory

```
High Activation Count or Strategic Importance
        ↓
Long-Term Memory (persistent)
- Frequently used pathways
- High success rate pathways
- Fundamental system patterns
- Permanent storage
```

**Properties:**
- Permanent persistence
- Slowest access
- Highest storage cost
- Rarely decays

### Phase 4: Episodic Memory

```
Event Context and Sequence Information
        ↓
Episodic Memory (event-based)
- Event sequences
- Contextual information
- Temporal relationships
- Outcome associations
```

**Properties:**
- Event-structured storage
- Context-aware retrieval
- Supports learning from experiences
- Supports pattern recognition

## Consolidation Event Types

```rust
pub struct ConsolidationEvent {
    pub entity_type: String,            // "pathway", "pattern", etc.
    pub entity_id: String,              // Unique identifier
    pub from_layer: MemoryLayer,        // Source layer
    pub to_layer: MemoryLayer,          // Target layer
    pub consolidation_type: ConsolidationType,  // Type of change
    pub strength_before: f64,           // Strength before consolidation
    pub strength_after: f64,            // Strength after consolidation
    pub timestamp: SystemTime,          // When consolidation occurred
}
```

### Consolidation Types

```rust
pub enum ConsolidationType {
    Promotion,      // Move to higher/more persistent layer
    Demotion,       // Move to lower/more volatile layer
    Pruning,        // Remove from memory entirely
    Reactivation,   // Restore dormant memory
}
```

## Consolidation Promotions

### Working → Short-Term

**Trigger Conditions:**
- Pathway activation count ≥ 3
- Pathway age ≥ 1 minute
- Activation within last hour
- Strength ≥ 0.3

**Effect:**
- Transfer to database-backed storage
- Start decay timer
- Increase access priority
- Enable pattern recognition

```
Working Memory Pathway:
├─ path_id: "health_failure_restart"
├─ activation_count: 3
├─ strength: 0.6
└─ age: 2 minutes

→ Trigger: activation_count >= 3
→ Type: Promotion
→ strength_after: 0.62 (slight boost)

Short-Term Memory:
├─ Persistent storage enabled
├─ Decay timer: 24 hours
├─ Retrieval cost: Medium
```

### Short-Term → Long-Term

**Trigger Conditions:**
- Pathway activation count ≥ 20
- Success rate ≥ 0.7
- Age ≥ 1 day
- Strength ≥ 0.7
- Stable access pattern

**Effect:**
- Move to permanent storage
- Increase access speed (caching)
- Mark as "core pathway"
- Enable cross-system routing

```
Short-Term Memory Pathway:
├─ path_id: "latency_cache_cleanup"
├─ activation_count: 25
├─ success_rate: 0.84
├─ strength: 0.85
└─ age: 5 days

→ Trigger: activation_count >= 20 AND success_rate >= 0.7
→ Type: Promotion
→ strength_after: 0.87 (LTP boost)

Long-Term Memory:
├─ Permanent storage
├─ Cached for fast access
├─ Core system pathway
├─ Available for cross-service use
```

## Consolidation Demotions

### Long-Term → Short-Term

**Trigger Conditions:**
- Success rate drops below 0.4
- Consecutive failures ≥ 5
- Pathway relevance decreases
- System learns better alternatives

**Effect:**
- Reduce access priority
- Activate decay timer
- Mark for potential pruning
- Enable alternative pathways

```
Long-Term Memory Pathway:
├─ path_id: "old_remediation"
├─ success_rate: 0.35 (degraded)
├─ consecutive_failures: 7
└─ strength: 0.42

→ Trigger: success_rate < 0.4
→ Type: Demotion
→ strength_after: 0.37 (LTD penalty)

Short-Term Memory:
├─ Decay timer reactivated
├─ Alternative pathways prioritized
├─ Monitoring for further decline
```

### Short-Term → Working

**Trigger Conditions:**
- Age > decay_period with no activation
- Strength drops below 0.2
- Pathway becomes irrelevant
- Memory space needed

**Effect:**
- Remove from persistent storage
- Return to volatile memory
- Eventual loss if not reactivated

## API

### Consolidation Events

```rust
pub struct ConsolidationEvent {
    pub entity_type: String,
    pub entity_id: String,
    pub from_layer: MemoryLayer,
    pub to_layer: MemoryLayer,
    pub consolidation_type: ConsolidationType,
    pub strength_before: f64,
    pub strength_after: f64,
    pub timestamp: SystemTime,
}
```

### Memory Layer Queries

```rust
// Retrieve pathways from specific layer
pub fn get_pathways_from_layer(layer: MemoryLayer) -> Vec<HebbianPathway>

// Check pathway's current layer
pub fn get_pathway_layer(pathway_id: &str) -> MemoryLayer

// List all consolidation events
pub fn get_consolidation_history() -> Vec<ConsolidationEvent>
```

## Memory State Transitions

```
┌─────────────┐
│   Working   │  (milliseconds)
└──────┬──────┘
       │ if activations >= 3
       │ and age >= 1 min
       ↓
┌──────────────┐
│  ShortTerm   │  (hours)
└──────┬───────┘
       │ if activations >= 20
       │ and success_rate >= 0.7
       ↓
┌──────────────┐
│  LongTerm    │  (persistent)
└──────┬───────┘
       │ if success_rate < 0.4
       │ or age > decay_period
       ↓
┌──────────────┐
│   Episodic   │  (event-based)
└──────────────┘
```

## Consolidation Timeline Example

```
T=0ms:    Health failure detected
          → Pathway created in Working Memory
          → strength: 0.5

T=100ms:  Service restart initiated
          → Pathway activated (count: 1)
          → Health improves
          → LTP applied: strength: 0.6

T=200ms:  Health failure detected again
          → Pathway activated (count: 2)
          → Service restart works
          → LTP applied: strength: 0.7

T=5000ms: Health failure detected again
          → Pathway activated (count: 3)
          → Service restart succeeds
          → LTP applied: strength: 0.8
          → Trigger: Promotion to Short-Term
          → strength_after: 0.82
          → Event logged to database
          → Consolidation timestamp: T=5000ms

T=1hour:  Multiple activations within hour
          → ShortTerm pathway still active
          → Maintains strength through success
          → Access cost reduces (caching)

T=24hrs:  Continued successful activations
          → activation_count: 35
          → success_rate: 0.91
          → strength: 0.92
          → Trigger: Promotion to Long-Term
          → strength_after: 0.94 (LTP boost)
          → Permanent storage activated
          → Cached for fast access
          → Available for cross-system use
```

## Memory Management Statistics

### Monitoring Consolidation

```rust
pub struct ConsolidationStats {
    pub working_count: u32,          // Pathways in working memory
    pub shortterm_count: u32,        // Pathways in short-term
    pub longterm_count: u32,         // Pathways in long-term
    pub episodic_count: u32,         // Episodic records
    
    pub promotions_total: u32,       // Promotions executed
    pub demotions_total: u32,        // Demotions executed
    pub prunings_total: u32,         // Pruning events
    
    pub avg_promotion_time: Duration,  // Time to first promotion
    pub consolidation_rate: f64,     // Pathways consolidated per minute
}
```

## Related Modules

- **M25_HEBBIAN_MANAGER** - Creates pathways that consolidator manages
- **M26_STDP_PROCESSOR** - Provides strength changes for consolidation decisions
- **M27_PATTERN_RECOGNIZER** - Patterns consolidated into episodic memory
- **M28_PATHWAY_PRUNER** - Removes pathways from all memory layers
- **M30_ANTIPATTERN_DETECTOR** - Detects patterns for demotion to protect system

## Consolidation Benefits

### System Learning
- Pathways strengthen through reinforcement
- Patterns become persistent and reliable
- Knowledge accumulates over time
- System improves with experience

### Performance
- Frequently used pathways cached
- Decision-making accelerates
- Memory efficiently utilized
- Retrieval optimized by layer

### Adaptability
- Failed pathways demoted and replaced
- System forgets ineffective strategies
- Alternative pathways prioritized
- Dynamic response to changing conditions

---

*[Back to Index](INDEX.md)*
