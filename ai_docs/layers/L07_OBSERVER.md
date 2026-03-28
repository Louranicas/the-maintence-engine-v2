# Layer 7: Observer

> **L07_OBSERVER** | Cross-Cutting Observer Layer | [Back to Index](../INDEX.md)

---

## Navigation

| Direction | Link |
|-----------|------|
| Up | [INDEX.md](../INDEX.md) |
| Previous | [L06_CONSENSUS.md](L06_CONSENSUS.md) |
| Related | [evolution_chamber_ai_docs/INDEX.md](../evolution_chamber_ai_docs/INDEX.md) |
| Specs | [evolution_chamber_ai_specs/INDEX.md](../../ai_specs/evolution_chamber_ai_specs/INDEX.md) |

---

## Layer Overview

The Observer Layer (L7) is a cross-cutting observation layer that monitors all 6 existing layers (L1-L6) via EventBus (M23) subscription without modifying them. It provides advanced log correlation, emergent behavior detection, and RALPH-loop enhanced evolution. L7 is non-invasive by design: the existing layers remain completely unaware of its existence.

**Status: PLANNED** -- comprehensive specifications and documentation exist but implementation has not yet started.

### Layer Properties

| Property | Value |
|----------|-------|
| Layer ID | L7 |
| Layer Name | Observer |
| Source Directory | `src/m7_observer/` (planned) |
| Dependencies | L1-L6 (read-only via EventBus M23) |
| Dependents | External Systems |
| Modules | M37-M39 |
| Utilities | Observer Bus, Fitness Evaluator |
| Integration Mode | Optional (`observer: Option<ObserverLayer>`) |
| Concurrency | `parking_lot::RwLock` |
| Database | observer_state.db |
| Status | **PLANNED** |
| Estimated LOC | ~6,600 |
| Estimated Tests | ~300 |

---

## Architecture

```
+------------------------------------------------------------------+
|                  EXISTING ENGINE (L1-L6)                          |
|                                                                  |
|  L1 Foundation  L2 Services  L3 Core Logic                      |
|  L4 Integration L5 Learning  L6 Consensus                       |
|       |              |             |                             |
|       +------+-------+------+------+                             |
|              |              |                                    |
|         [ EventBus ]   [ EventBus ]                              |
|              |              |                                    |
+--------------+--------------+------------------------------------+
               |              |
          SUBSCRIBES     SUBSCRIBES
               |              |
+--------------v--------------v------------------------------------+
|                  L7: OBSERVER LAYER                               |
|                                                                  |
|  +---------------------------+  +---------------------------+    |
|  |   M37 Log Correlator      |  |   M38 Emergence Detector  |    |
|  |                           |  |                           |    |
|  |  - Cross-layer correlation |  |  - Cascade analysis       |    |
|  |  - Temporal windowing      |  |  - Synergy delta tracking |    |
|  |  - Timeline construction   |  |  - Resonance detection    |    |
|  +-------------+-------------+  +-------------+-------------+    |
|                |                              |                  |
|                +------------+-----------------+                  |
|                             |                                    |
|  +---------------------------+---------------------------+       |
|  |         M39 Evolution Chamber (RALPH Loop)            |       |
|  |                                                       |       |
|  |  - Recognize patterns      - Analyze cross-layer      |       |
|  |  - Learn meta-patterns     - Plan mutations           |       |
|  |  - Harmonize system        - Fitness scoring (12D)    |       |
|  +-------------------------------------------------------+       |
|                                                                  |
|  Utilities: Observer Bus (~500 LOC), Fitness Evaluator (~800 LOC)|
|                                                                  |
|  PUBLISHES TO: observation.*, emergence.*, evolution.*           |
+------------------------------------------------------------------+
```

---

## Module Reference (M37-M39)

| Module | File | Purpose | Est. LOC | Status |
|--------|------|---------|----------|--------|
| M37 | `log_correlator.rs` | Cross-layer event correlation engine | ~1,400 | PLANNED |
| M38 | `emergence_detector.rs` | System-level emergent behavior detection | ~1,500 | PLANNED |
| M39 | `evolution_chamber.rs` | RALPH loop meta-learning and evolution | ~1,800 | PLANNED |

### Utilities

| Utility | File | Purpose | Est. LOC | Status |
|---------|------|---------|----------|--------|
| Observer Bus | `observer_bus.rs` | Internal L7 pub/sub connecting M37/M38/M39 | ~500 | PLANNED |
| Fitness Evaluator | `fitness_evaluator.rs` | 12D tensor fitness scoring for evolution candidates | ~800 | PLANNED |

---

## EventBus Channels

### Subscribed Channels (Read-Only from L1-L6)

| Channel | Source Layer | Event Types | Consumer |
|---------|-------------|-------------|----------|
| `health.*` | L2 Services | HealthCheck, ServiceDown, ServiceUp | M37, M38 |
| `remediation.*` | L3 Core Logic | ActionProposed, ActionExecuted, OutcomeRecorded | M37, M38, M39 |
| `learning.*` | L5 Learning | PathwayStrengthened, PatternRecognized, Pruned | M38, M39 |
| `consensus.*` | L6 Consensus | ProposalSubmitted, VoteCast, ConsensusAchieved | M37, M38 |
| `integration.*` | L4 Integration | BridgeEvent, ServiceConnected, ServiceDisconnected | M37 |
| `metrics.*` | L1 Foundation | MetricRecorded, ThresholdBreached, AnomalyDetected | M37, M38 |

### Published Channels (New, L7-Owned)

| Channel | Publisher | Event Types | Consumers |
|---------|-----------|-------------|-----------|
| `observation.*` | M37 Log Correlator | CorrelationFound, CrossLayerPattern, TimelineBuilt | M38, M39, External |
| `emergence.*` | M38 Emergence Detector | EmergenceBehaviorDetected, SystemPhaseShift, AttractorFound | M39, External |
| `evolution.*` | M39 Evolution Chamber | CandidateProposed, FitnessScored, EvolutionApplied | External |

---

## RALPH Loop (M39)

The RALPH loop is the core evolution protocol within M39, operating in 5 phases:

| Phase | Name | Description |
|-------|------|-------------|
| R | Recognize | Pattern recognition from correlated observations |
| A | Analyze | Cross-layer analysis of detected patterns |
| L | Learn | Meta-learning from analysis results |
| P | Plan | Mutation candidate generation and fitness scoring |
| H | Harmonize | Apply verified mutations, rollback if degraded |

### Key Parameters

| Parameter | Default | Description |
|-----------|---------|-------------|
| Recognize interval | 30s | Pattern recognition cycle |
| Analyze depth | 3 layers | Cross-layer analysis depth |
| Learn rate | 0.05 | Meta-learning rate |
| Propose threshold | 0.75 | Minimum fitness to propose |
| Horizon | 100 observations | Rolling window size |

---

## 12D Tensor Fitness Scoring

The Fitness Evaluator scores evolution candidates using the existing 12D tensor dimensions:

```
[D0:service_id, D1:port, D2:tier, D3:deps, D4:agents, D5:protocol,
 D6:health, D7:uptime, D8:synergy, D9:latency, D10:error_rate, D11:temporal]
```

Fitness is computed as a weighted combination of tensor dimensions, with trend analysis and stability factored in. See [FITNESS_EVALUATOR.md](../evolution_chamber_ai_docs/FITNESS_EVALUATOR.md) and [FITNESS_FUNCTION_SPEC.md](../../ai_specs/evolution_chamber_ai_specs/FITNESS_FUNCTION_SPEC.md) for details.

---

## Inter-Layer Communication

### Events from L1-L6

L7 subscribes to 6 EventBus channels (read-only). It never sends commands to L1-L6 directly.

```rust
pub enum L7InputEvent {
    Health(HealthEvent),         // from L2
    Remediation(RemediationEvent), // from L3
    Learning(LearningEvent),     // from L5
    Consensus(ConsensusEvent),   // from L6
    Integration(IntegrationEvent), // from L4
    Metrics(MetricsEvent),       // from L1
}
```

### Events to External Systems

```rust
pub enum L7OutputEvent {
    CorrelationFound { correlation: Correlation },
    EmergenceBehaviorDetected { behavior: EmergenceBehavior },
    CandidateProposed { candidate: EvolutionCandidate },
    FitnessScored { candidate_id: CandidateId, score: f64 },
    EvolutionApplied { candidate_id: CandidateId, result: EvolutionResult },
}
```

---

## Configuration (Planned)

```toml
[layer.L7]
enabled = true
startup_order = 7

[layer.L7.observer]
integration_mode = "optional"
concurrency = "parking_lot"
database = "observer_state.db"

[layer.L7.correlation]
window_ms = 5000
min_confidence = 0.6

[layer.L7.emergence]
cascade_depth_threshold = 3
synergy_delta_threshold = 0.15

[layer.L7.evolution]
max_concurrent_mutations = 3
auto_apply_threshold = 0.10
rollback_threshold = -0.02
min_generation_interval_ms = 60000

[layer.L7.ralph]
recognize_interval_s = 30
analyze_depth = 3
learn_rate = 0.05
propose_threshold = 0.75
horizon = 100

[layer.L7.fitness]
history_capacity = 200
trend_window = 10
```

---

## Metrics (Planned)

| Metric | Type | Description |
|--------|------|-------------|
| `me_l7_correlations_found` | Counter | Cross-layer correlations detected |
| `me_l7_emergence_events` | Counter | Emergent behaviors detected |
| `me_l7_evolution_candidates` | Counter | Evolution candidates proposed |
| `me_l7_fitness_scores` | Histogram | Fitness score distribution |
| `me_l7_mutations_applied` | Counter | Mutations successfully applied |
| `me_l7_mutations_rolled_back` | Counter | Mutations rolled back |
| `me_l7_ralph_cycles` | Counter | RALPH loop cycles completed |
| `me_l7_ralph_duration_ms` | Histogram | RALPH cycle duration |
| `me_l7_bus_events_received` | Counter | Events received from L1-L6 |
| `me_l7_bus_events_published` | Counter | Events published to L7 channels |

---

## Detailed Documentation

For comprehensive specifications and design documentation, see:

| Resource | Path |
|----------|------|
| Evolution Chamber Docs (9 files) | [evolution_chamber_ai_docs/INDEX.md](../evolution_chamber_ai_docs/INDEX.md) |
| Evolution Chamber Specs (10 files) | [evolution_chamber_ai_specs/INDEX.md](../../ai_specs/evolution_chamber_ai_specs/INDEX.md) |
| M37 Log Correlator | [M37_LOG_CORRELATOR.md](../evolution_chamber_ai_docs/M37_LOG_CORRELATOR.md) |
| M38 Emergence Detector | [M38_EMERGENCE_DETECTOR.md](../evolution_chamber_ai_docs/M38_EMERGENCE_DETECTOR.md) |
| M39 Evolution Chamber | [M39_EVOLUTION_CHAMBER.md](../evolution_chamber_ai_docs/M39_EVOLUTION_CHAMBER.md) |
| Observer Bus | [OBSERVER_BUS.md](../evolution_chamber_ai_docs/OBSERVER_BUS.md) |
| Fitness Evaluator | [FITNESS_EVALUATOR.md](../evolution_chamber_ai_docs/FITNESS_EVALUATOR.md) |
| Integration Guide | [INTEGRATION_GUIDE.md](../evolution_chamber_ai_docs/INTEGRATION_GUIDE.md) |
| Data Flow | [DATA_FLOW.md](../evolution_chamber_ai_docs/DATA_FLOW.md) |

---

## Navigation

| Direction | Link |
|-----------|------|
| Up | [INDEX.md](../INDEX.md) |
| Previous | [L06_CONSENSUS.md](L06_CONSENSUS.md) |
| Evolution Chamber Docs | [evolution_chamber_ai_docs/INDEX.md](../evolution_chamber_ai_docs/INDEX.md) |
| Evolution Chamber Specs | [evolution_chamber_ai_specs/INDEX.md](../../ai_specs/evolution_chamber_ai_specs/INDEX.md) |
| EventBus (M23) | [../modules/M23_EVENT_BUS.md](../modules/M23_EVENT_BUS.md) |

---

*[Back to Index](../INDEX.md) | [Previous: L06 Consensus](L06_CONSENSUS.md)*
