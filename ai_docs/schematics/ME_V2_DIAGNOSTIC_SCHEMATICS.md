# ME V2 -- Diagnostic and Tuning Schematics

> **God-tier diagnostic reference for system operators and AI agents debugging Maintenance Engine V2.**
> **Fitness:** 0.612 | **Health:** 0.650 | **Metabolic:** 0.327 | **RALPH:** gen=5, mutations=0
> **Session:** 068 | **Date:** 2026-03-29

---

## Table of Contents

1. [Fitness Diagnostic Tree](#1-fitness-diagnostic-tree)
2. [Metabolic Health Diagnostic](#2-metabolic-health-diagnostic)
3. [Observer Pipeline Diagnostic](#3-observer-pipeline-diagnostic)
4. [EventBus Channel Health](#4-eventbus-channel-health)
5. [RALPH Evolution Diagnostic](#5-ralph-evolution-diagnostic)
6. [Lock Ordering Reference](#6-lock-ordering-reference)
7. [Wiring Gap Map](#7-wiring-gap-map)
8. [Tensor Dimension Diagnostic](#8-tensor-dimension-diagnostic)
9. [Service Mesh Health Matrix](#9-service-mesh-health-matrix)
10. [Cross-Service Data Flow Map](#10-cross-service-data-flow-map)
11. [Background Task Inventory](#11-background-task-inventory)
12. [Database Health Matrix](#12-database-health-matrix)
13. [Escalation Pipeline Diagnostic](#13-escalation-pipeline-diagnostic)
14. [STDP Learning Diagnostic](#14-stdp-learning-diagnostic)
15. [Thermal Subsystem Diagnostic](#15-thermal-subsystem-diagnostic)
16. [Tuning Knobs Reference](#16-tuning-knobs-reference)

---

## 1. Fitness Diagnostic Tree

When fitness is below target, trace the root cause through this decision tree. Each leaf
references the recommendation (R#) or fix (F#) that addresses it.

```
FITNESS < 0.70 (Degraded)
|
+-- Check L2 Services (weight 0.20 on D6)
|   +-- services_healthy < total --> R1: L2 health scoring bug
|   |   Root cause: healthy_ratio() counts FSM-transitioned services only
|   |   Fix: use aggregate_health() in compute_layer_health() (~15 LOC)
|   |   Impact: L2 0.33 --> 0.92, overall fitness +0.25
|   +-- library-agent circuit open --> F2 (FIXED session 068)
|   +-- 5 services missing from mesh --> F1 (FIXED session 068)
|   +-- circuit breaker stuck Open --> check M12 half_open_timeout
|   +-- health_monitor polling failed --> check spawn_health_polling task alive
|
+-- Check L5 Learning (weight on D11)
|   +-- pathway_strength uniform (all ~0.492) --> R5: no STDP timing pairs
|   |   Root cause: N04 StdpBridge records, but M26 StdpProcessor gets 0 spikes
|   |   Fix: call stdp_processor.record_spike() in spawn_health_polling (~20 LOC)
|   +-- decay_rate = 0.001 --> F13: hardcoded handler (FIXED session 068)
|   +-- learning events = 0 --> R7: no PeerBridge signals
|   |   Root cause: PeerBridge failures stay internal, no EventBus/SignalBus emission
|   |   Fix: publish per-service health to EventBus 'integration' channel (~30 LOC)
|   +-- LTP/LTD ratio < 0.10 --> check HRS-001 decay_auditor thresholds
|
+-- Check RALPH (mutations = 0)
|   +-- fitness > 0.5 --> R3: auto-proposal threshold too high
|   |   Fix: lower threshold from 0.5 to 0.7 or remove (~50 LOC)
|   +-- generation <= 5 --> R3: generation gate blocks fitness_driven_mutations
|   |   Fix: add generation-independent mutation path
|   +-- zero_correlation_streak = 0 --> R3: dormancy never triggers
|   |   Root cause: correlations ARE found (1,460/tick), streak stays at 0
|   +-- emergence types < 3 --> R13: only AttractorFormation + CascadeFailure fire
|
+-- Check D3 deps (0.083)
|   +-- dependency_count placeholder --> M49 TrafficAnalyzer needed
|   +-- all services report deps=0 --> M09 dep tracking incomplete
|
+-- Check D5 protocol (hardcoded)
|   +-- protocol diversity not tracked --> N02 IntentRouter needed (R20)
|   +-- static 0.750 value --> engine.rs build_tensor() hardcodes protocol dim
|
+-- Check D10 error_rate (0.556)
|   +-- high error_rate from health poll failures --> check network / service crashes
|   +-- error counting double-counts --> verify M10 + M12 don't overlap
|
+-- Check thermal subsystem
    +-- T diverged from SYNTHEX --> R8: independent thermal model, no coupling
    +-- T oscillating > 0.10 amplitude --> check PID gains (Kp, Ki, Kd)
    +-- decay_auditor corrections high --> HRS-001 overcorrecting
```

---

## 2. Metabolic Health Diagnostic

The metabolic product measures the health of the cross-service nervous system.

```
METABOLIC PRODUCT = ME_fitness x ORAC_fitness x PV2_r

Target:  > 0.55 (HEALTHY)
Current: 0.327

Breakdown:
  ME_fitness   = 0.612  --> Fix R1 (L2 scoring) to reach 0.80+
  ORAC_fitness = 0.704  --> Healthy, no action needed
  PV2_r        = 0.760  --> Declining, monitor via /sweep

Projection after fixes:
  After R1 only:         0.80 x 0.70 x 0.76 = 0.426 (below target)
  After R1+R3+R5:        0.85 x 0.70 x 0.76 = 0.452 (still below)
  After R1+R3+R5+R4:     0.85 x 0.75 x 0.80 = 0.510 (approaching)
  After full Sprint 1-3: 0.88 x 0.78 x 0.82 = 0.562 (TARGET MET)

DIAGNOSIS STEPS:
  1. curl localhost:8080/api/health          --> ME_fitness (check 'fitness' field)
  2. curl localhost:8133/health              --> ORAC (check 'fitness' in RALPH)
  3. curl localhost:8132/health              --> PV2 (check 'r' in Kuramoto field)
  4. Multiply all three. If < 0.55, identify the lowest contributor.
  5. If ME lowest  --> use Fitness Diagnostic Tree (Section 1)
  6. If ORAC lowest --> check ORAC bridges, RALPH phase, LTP/LTD
  7. If PV2 lowest  --> check sphere count, K regime, tick rate
```

---

## 3. Observer Pipeline Diagnostic

The observer pipeline is the core data flow that drives all intelligence in ME V2.
Events flow left to right; blockages at any stage starve downstream consumers.

```
EVENTS --> CORRELATIONS --> EMERGENCES --> MUTATIONS --> APPLIED
  M44        M37              M38             M39         M39
  ObsBus     LogCorrelator    EmergenceDet    EvolChamber EvolChamber

Current flow rates per tick (60s):
  Events ingested:        ~124/tick        OK (flowing)
  Correlations found:     ~1,480/tick      OK (11.9x ratio)
  Emergences detected:    1,000 total      CAPPED (ring buffer full)
  Mutations proposed:     0                BLOCKED (threshold too high)
  Mutations applied:      0                NO PROPOSALS to apply

BOTTLENECK ANALYSIS:
  Stage 1 (Events):       HEALTHY. 124 events/tick from 6 channels.
  Stage 2 (Correlations): HEALTHY. 1,480/tick with diverse patterns.
  Stage 3 (Emergences):   SATURATED. Ring buffer at 1,000 cap.
                          Only 2/8 types fire (AttractorFormation, CascadeFailure).
                          Fix: R13 (diversify detection thresholds).
  Stage 4 (Mutations):    BLOCKED. Three compounding gates:
                          Gate A: fitness < 0.5 for auto-proposal (current 0.612)
                          Gate B: tick%10==0 AND gen>5 (gen is 5, off by 1)
                          Gate C: zero_correlation_streak>=3 (streak=0)
                          Fix: R3 (lower/remove gates).
  Stage 5 (Applied):      NO INPUT. Zero proposals means zero applications.

QUICK FIX ORDER:
  1. R3  --> Unblock mutations (proposals will start flowing)
  2. R13 --> Diversify emergences (richer mutation hints)
  3. R19 --> Advanced Evolution Chamber (4-source Learn, 5 strategies)
```

---

## 4. EventBus Channel Health

```
CHANNEL          EVENTS    RATE        SUBSCRIBERS   STATUS
---------------------------------------------------------------------
health           683       62/min      0             PRODUCING, NO CONSUMER
integration      2,015     182/min     0             PRODUCING, NO CONSUMER
metrics          66        6/min       0             PRODUCING, NO CONSUMER
learning         4         0.4/min     0             STALLED
consensus        0         0/min       0             DEAD
remediation      0         0/min       0             DEAD

TOTAL:           2,768     250+/min    0             6 CHANNELS, 0 SUBSCRIBERS

ROOT CAUSE:
  EventBus::publish() increments delivered_to count but never invokes callbacks.
  The publish path: event_log.write() -> channel.events.push() -> subscriber scan
  The subscriber scan finds IDs but has no Subscriber trait callback to invoke.

FIX PLAN:
  R2  (short-term): Wire EventBus --> PV2:8132/bus/events bridge subscriber
      Add event_bus.subscribe("pv2-bridge", "health", None) at startup (~30 LOC)
  R21 (long-term):  Implement Subscriber trait with on_event() callback
      Turn EventBus from passive bookkeeping to active event delivery (~80 LOC)

STALLED/DEAD CHANNELS:
  'learning' (4 events): Only fires during STDP decay audit. Fix: R7 (PeerBridge signals)
  'consensus' (0 events): PBFT never activated. Fix: R10 (Pattern --> PBFT escalation)
  'remediation' (0 events): No remediation worker. Fix: R6 (spawn_remediation_worker)

VERIFICATION:
  curl -s localhost:8080/api/event-bus | jq '.channels'
  # Check: events > 0, subscriber_count > 0, rate > 0
```

---

## 5. RALPH Evolution Diagnostic

```
RALPH STATE:
  Generation:           5
  Phase:                cycling (Recognize --> Analyze --> Learn --> Propose --> Harvest)
  Cycles completed:     8
  Mutations proposed:   0
  Mutations applied:    0
  Fitness plateau:      0.612 (stable 30+ minutes)
  Emergence events:     1,000 (ring buffer capped)
  Correlations:         58,820
  Correlation rate:     1,460/tick

WHY NO MUTATIONS (3 compounding blocks):
  Block 1: Auto-proposal gate
    Condition: fitness < 0.5 required for M38-->M39 auto-proposal
    Current:   fitness = 0.612 (ABOVE threshold)
    Fix:       Lower to 0.7 or remove gate entirely

  Block 2: Fitness-driven mutation gate
    Condition: tick % 10 == 0 AND generation > 5
    Current:   generation = 5 (off-by-one, needs >5 not >=5)
    Fix:       Change to generation >= 5, or remove generation check

  Block 3: Dormancy response gate
    Condition: zero_correlation_streak >= 3
    Current:   streak = 0 (correlations found every tick)
    Fix:       Add time-based mutation trigger independent of correlation streak

EXPECTED BEHAVIOR AFTER R3 FIX:
  Mutations/hour:  10+ (conservative estimate)
  Gen/hour:        ~40 (up from 1.8)
  Fitness trend:   0.612 --> 0.75 (first 100 mutations)
                   0.75 --> 0.85 (next 200 mutations with STDP feedback)

VERIFICATION:
  curl -s localhost:8080/api/observer | jq '.evolution'
  # Check: mutations_proposed > 0, generation > 5, fitness trending up

ADVANCED (R19 Evolution Chamber V2):
  4-source Learn:        emergence -> dimension -> pathway -> structural deficit
  5 strategies:          Conservative, Exploratory, StructuralRepair, Convergence, Morphogenic
  Field-coherence gate:  test-then-apply via N05 (not apply-then-rollback)
  Target gen/hour:       240 (15s evolution tick)
```

---

## 6. Lock Ordering Reference

Locks must always be acquired in ascending L-order. Acquiring in reverse risks deadlock.

```
SAFE LOCK ORDER (acquire in this sequence, never reverse):

L1:  SignalBus.subscribers.read --> SignalBus.stats.write
     (parking_lot::RwLock -- no poison risk)

L2:  MonitorState.write --> (drop) --> SignalBus
     CircuitBreaker.write --> (drop) --> SignalBus
     (Always drop state lock before emitting signals)

L3:  PipelineManager.execution_log.write --> (drop) --> pipelines.write
     ApprovalManager.pending.write --> (drop) --> audit_log.write
     (Sequential acquisition with drop between)

L4:  EventBus ordering:
       channels.read --> subscribers.read --> event_log.write --> channels.write
     (Read locks acquired before write locks on same struct)

L5:  HebbianManager.pathways.write --> (drop) --> consolidation
     StdpProcessor.timing_pairs.write --> (drop) --> pathway updates
     (L5 modules use parking_lot -- fast, no poison)

L6:  PbftManager:
       proposals.write --> votes.read (NESTED -- fragile, never reverse)
     DissentTracker:
       events.write --> by_proposal.write --> by_agent.write (3 NESTED)
     (L6 uses std::sync::RwLock -- poison risk on panic!)

L7:  ObserverBus(1) --> LogCorrelator(2) --> EmergenceDetector(3) --> EvolutionChamber(4)
     (Pipeline order matches lock acquisition order)

DANGER ZONES:
  [CRITICAL] L6 dissent.rs: 3 nested std::sync::RwLock::write during re-index
    When buffer cap reached, record_dissent() holds all 3 simultaneously.
    Fix: R14 -- collect index data before acquiring locks, swap in one pass.
    Alt: Replace std::sync::RwLock with parking_lot::RwLock (eliminates poison).

  [HIGH] L6 pbft.rs: proposals.write held while votes.read acquired
    If another thread holds votes.write waiting for proposals.read --> deadlock.
    Mitigation: all PBFT operations go through single-threaded proposal queue.

  [MEDIUM] L4 EventBus: channels.write is last in chain
    If publish() is called while holding channels.write --> infinite recursion.
    Mitigation: publish() always takes channels.read first.

VERIFICATION:
  # Check for lock contention (requires tokio-console or tracing)
  RUSTFLAGS="--cfg tokio_unstable" cargo run
  # Monitor: lock wait times > 1ms indicate contention
```

---

## 7. Wiring Gap Map

Green (wired, data flows) vs Red (gap, needs implementation).

```
WIRED (active data flow):
  ME --health poll-------> 16 services (30s interval)               [OK]
  ME --EventBus bridge---> PV2:8132/bus/events (10s, fire-and-forget)[OK]
  ME --thermal poll------> SYNTHEX:8090/v3/* (60s)                  [OK]
  ME --field tracking----> PV2:8132/health (10s via N01+N03+N06)    [OK]
  ME --ORAC poll---------> ORAC:8133/health (30s, record only)      [OK]
  ME --self model--------> layer health (60s via M48)               [OK]
  ME --STDP C12----------> co-activation (on health poll)            [OK]
  ME --tool registration-> TL:8105/api/tools (startup, 15 tools)    [OK]
  ME --DevOps trigger----> DevOps:8081/pipeline/trigger (startup)    [OK]
  ME --decay trigger-----> SYNTHEX:8090/v3/decay/trigger (300s)      [OK]
  ORAC --me_bridge-------> ME V1:8080 (10s)                         [OK]

NOT WIRED (gaps requiring implementation):
  EventBus --> L5 Learning consumers              [R21] ~80 LOC
    No callback delivery. Events logged but never consumed by learning pipeline.

  PeerBridge failures --> learning signals          [R7]  ~30 LOC
    Circuit breaker transitions don't emit LTP/LTD to HebbianManager.

  Auth --> RateLimit --> OracBridge chain            [R12] ~25 LOC
    Three modules exist independently, no sequencing code.

  Pattern --> Antipattern --> PBFT escalation        [R10] ~60 LOC
    M27 detects, M30 classifies, M31 should vote. No pipeline connects them.

  Dissent generator --> Dissent tracker             [R11] ~30 LOC
    M57 generates counterarguments, M35 tracks dissent. No bridge.

  Checkpoint --> EvolutionChamber restore            [R9]  ~50 LOC
    M56 saves CognitiveSnapshot, M39 has no restore_from_snapshot().

  SYNTHEX thermal --> ME thermal (coupling)          [R8]  ~20 LOC
    ME runs independent thermal model. Should mirror SYNTHEX PID output.

  ORAC --> ME V2 (port 8180)                        [R4]  ~30 LOC
    ORAC hardcoded to ME V1 port 8080. ME V2 on 8180 is invisible.

  N02 IntentRouter (initialized, never called)       [R20] ~150 LOC (combined)
  N05 EvolutionGate (initialized, never called)      [R20]

  Remediation worker (no consumer for pending)       [R6]  ~40 LOC
    submit-remediation queues requests, nothing processes them.

WIRING DEPENDENCY ORDER:
  Phase 1: R1 (scoring) + R3 (RALPH gates) + R5 (STDP spikes)
  Phase 2: R2 (EventBus->PV2) + R4 (ORAC->MEv2) + R6 (remediation) + R7 (PeerBridge)
  Phase 3: R8 (thermal) + R9 (checkpoint) + R10 (PBFT) + R11 (dissent)
  Phase 4: R19 (Advanced Evolution) + R20 (Nexus full) + R21 (EventBus callbacks)
```

---

## 8. Tensor Dimension Diagnostic

The 12D tensor encodes system state. Each dimension has a weight, a contributing module,
and a current value. Bottleneck dimensions drag overall fitness.

```
DIM  NAME              VALUE   WEIGHT  WEIGHTED  SOURCE        STATUS
-------------------------------------------------------------------------
D0   service_id        1.000   0.05    0.050     M09           OK
D1   port              0.123   0.02    0.002     Engine        LOW (*)
D2   tier              0.486   0.08    0.039     M09           MID
D3   dependency_count  0.083   0.05    0.004     M09           CRITICAL (**)
D4   agent_count       0.917   0.05    0.046     Engine        OK
D5   protocol          0.750   0.03    0.023     Engine        OK (***)
D6   health_score      0.625   0.20    0.125     M10,M11       MID (****)
D7   uptime            1.000   0.15    0.150     M11           PERFECT
D8   synergy           0.833   0.15    0.125     M24           GOOD
D9   latency           1.000   0.10    0.100     M12           PERFECT (inv)
D10  error_rate        0.556   0.10    0.056     M10,M12       MID (inv)
D11  temporal_context  0.748   0.02    0.015     Engine        OK (*****)
-------------------------------------------------------------------------
                               1.00    0.735     WEIGHTED SUM

(*)     D1 LOW: 5 newly added services not in original port-to-normalized-value map.
        Fix: Update port normalization in engine.rs build_tensor().

(**)    D3 CRITICAL: dependency_count is a placeholder returning 0.0 for all services.
        Fix: Implement M49 TrafficAnalyzer to compute actual dep counts.
        Short-term: hardcode known dep counts from devenv.toml dependency graph.

(***)   D5 OK but STATIC: protocol diversity is hardcoded at 0.750.
        Fix: Wire N02 IntentRouter to track actual protocol usage per service.

(****)  D6 MID: LARGEST WEIGHT (0.20). Currently 0.625 due to L2 scoring bug.
        Fix: R1 will raise this to 0.92, adding +0.059 to weighted sum.
        This is the single highest-leverage dimension fix.

(*****) D11 WEAK PROXY: Uses timestamp-based heuristic instead of Nexus field data.
        Fix: Wire N01 FieldBridge coherence history as temporal_context source.

BOTTLENECK RANKING (by lost potential = weight x (1.0 - value)):
  1. D6  health_score:     0.20 x 0.375 = 0.075 lost  --> FIX R1
  2. D10 error_rate:       0.10 x 0.444 = 0.044 lost  --> reduce poll errors
  3. D2  tier:             0.08 x 0.514 = 0.041 lost  --> tier distribution issue
  4. D3  dependency_count: 0.05 x 0.917 = 0.046 lost  --> FIX (placeholder)
  5. D8  synergy:          0.15 x 0.167 = 0.025 lost  --> improve bridge health

VERIFICATION:
  curl -s localhost:8080/api/tensor | jq '.dimensions'
  # Each dimension should show value, weight, and contributing module
```

---

## 9. Service Mesh Health Matrix

```
SERVICE              PORT   HEALTH  LATENCY   SYNERGY  CIRCUIT    ADDED
-------------------------------------------------------------------------
devops-engine        8081   1.0     0.18ms    0.999    Closed     Original
synthex              8090   1.0     0.18ms    0.999    Closed     Original
san-k7               8100   1.0     0.20ms    0.999    Closed     Original
nais                 8101   1.0     0.24ms    0.999    Closed     Original
bash-engine          8102   1.0     0.12ms    0.999    Closed     Original
tool-maker           8103   1.0     0.11ms    0.999    Closed     Original
ccm                  8104   1.0     0.12ms    0.999    Closed     Original
tool-library         8105   1.0     0.12ms    0.999    Closed     Original
codesynthor-v7       8110   1.0     0.19ms    0.999    Closed     Original
vortex-memory        8120   1.0     --        --       N/A        F1 fix
povm-engine          8125   1.0     --        --       N/A        F1 fix
reasoning-memory     8130   1.0     --        --       N/A        F1 fix
pane-vortex          8132   1.0     --        --       N/A        F1 fix
orac-sidecar         8133   1.0     --        --       N/A        F1 fix
architect-agent      9001   1.0     0.34ms    0.999    Closed     Original
prometheus-swarm     10001  1.0     0.24ms    0.999    Closed     Original

LEGEND:
  Health: 1.0 = healthy, 0.0 = down, -1.0 = unknown
  Latency: average HTTP response time (-- = not yet tracked)
  Synergy: M24 bridge synergy score (-- = newly added, no history)
  Circuit: Closed = flowing, Open = blocked, HalfOpen = probing, N/A = no circuit breaker
  Added: Original = in initial mesh, F1 fix = added in session 068

NEWLY ADDED SERVICES (F1 FIX):
  VMS:8120, POVM:8125, RM:8130, PV2:8132, ORAC:8133 were missing from the mesh.
  They are now polled but don't yet have circuit breakers, latency tracking, or synergy
  scores. This is why D1 (port) is 0.123 -- the normalization map doesn't include them.

SERVICES NOT IN MESH:
  ME V2 itself (8180): Does not self-monitor (by design -- uses self-model M48 instead)
  library-agent (8083): Disabled service, not monitored
  sphere-vortex (8120): Disabled, VMS owns the port

CIRCUIT BREAKER THRESHOLDS:
  Open after:    5 consecutive failures
  Half-open at:  30s after opening
  Close after:   3 consecutive successes in half-open
  Reset backoff: exponential (30s, 60s, 120s, 240s max)

VERIFICATION:
  curl -s localhost:8080/api/health | jq '.services'
  curl -s localhost:8080/api/services | jq '.[] | {id, health, circuit_state}'
```

---

## 10. Cross-Service Data Flow Map

```
                                    THE HABITAT
    +-----------------------------------------------------------------+
    |                                                                   |
    |   SYNTHEX (brain, :8090)                                         |
    |     +-- V3 thermal PID (T=0.55, target=0.50)                    |
    |     +-- 4 heat sources: Hebbian(0.73) Cascade(0.13)             |
    |     |                   Resonance(0.73) CrossSync(1.0)          |
    |     +-- ME V2 reads: /v3/thermal, /v3/diagnostics (60s)         |
    |     +-- ME V2 triggers: /v3/decay/trigger (300s)                |
    |     +-- ORAC mirrors: m22_synthex_bridge (exact thermal match)  |
    |                                                                   |
    |   ORAC (proxy, :8133)                                            |
    |     +-- RALPH gen=15,990, fitness=0.704                          |
    |     +-- Reads ME V1(:8080): /api/health, /api/observer (10s)    |
    |     +-- Does NOT read ME V2(:8180) -- GAP R4                    |
    |     +-- 5 bridges: ME, PV2, SYNTHEX, POVM, RM                  |
    |     +-- LTP=3,732, LTD=0, emergence=3,520                      |
    |                                                                   |
    |   PV2 (field, :8132)                                             |
    |     +-- Kuramoto r=0.760 (declining), K=1.50 (Fleet regime)     |
    |     +-- 83 spheres, 660K+ ticks                                  |
    |     +-- ME V2 reads: /health (10s via spawn_field_tracking)     |
    |     +-- ME V2 posts: /bus/events (10s via EventBus bridge)      |
    |     +-- Bus: 1 subscriber, 1000 events (ring buffer)            |
    |                                                                   |
    |   VMS (memory, :8120)                                            |
    |     +-- r=0.999 (near-perfect coherence)                         |
    |     +-- 2,647 memories, morphogenic_cycle=242                    |
    |     +-- ME V2: newly added to mesh (F1 fix), health poll only   |
    |                                                                   |
    |   K7 (orchestrator, :8100)                                       |
    |     +-- 59/59 modules healthy, 11 nexus commands                 |
    |     +-- ME V2: health poll only (no nexus command integration)   |
    |                                                                   |
    |   POVM (persistence, :8125)                                      |
    |     +-- 2,437 memories, 119 crystallized                         |
    |     +-- ME V2: newly added to mesh, health poll only             |
    |     +-- ORAC: m26_povm_bridge (read+write, 60s)                 |
    |                                                                   |
    |   RM (reasoning, :8130)                                          |
    |     +-- 64,400 entries, TSV format (NOT JSON!)                   |
    |     +-- ME V2: newly added to mesh, health poll only             |
    |     +-- ORAC: m25_rm_bridge (TSV read+write, 60s)               |
    |                                                                   |
    +-----------------------------------------------------------------+

DATA FLOW DIAGRAM:

  ME V2 (:8080/8180)
    |
    +--[30s health]--> 16 services (GET /health)
    |                    |
    |                    +---> M10 HealthMonitor (aggregate)
    |                    +---> M12 CircuitBreaker (state transitions)
    |                    +---> N04 StdpBridge (co-activation C12)
    |                    +---> M45 FitnessEvaluator (12D tensor)
    |
    +--[10s EventBus]--> PV2:8132/bus/events (POST, 6 channels)
    |                      |
    |                      +---> PV2 bus ring buffer (1000 events)
    |                      +---> [GAP] No callback delivery to L5
    |
    +--[60s thermal]--> SYNTHEX:8090/v3/thermal (GET)
    |                    |
    |                    +---> M40 ThermalMonitor (T, PID state)
    |                    +---> [GAP] ME thermal independent, not coupled (R8)
    |
    +--[10s field]----> PV2:8132/health (GET)
    |                    |
    |                    +---> N01 FieldBridge (r, spheres, K)
    |                    +---> N03 RegimeManager (K-regime detection)
    |                    +---> N06 MorphogenicAdapter (|r_delta| > 0.05)
    |
    +--[60s observer]--> M44 ObserverBus --> M37 LogCorrelator
                           |                    |
                           |                    +---> M38 EmergenceDetector
                           |                           |
                           |                           +---> M39 EvolutionChamber
                           |                                  |
                           |                                  +---> [BLOCKED] (R3)
                           +---> [GAP] No M27 Pattern --> M31 PBFT (R10)
```

---

## 11. Background Task Inventory

These are the spawned background tasks running inside ME V2. If any task panics or stops,
the corresponding subsystem goes dark.

```
TASK NAME                 INTERVAL  MODULE      FUNCTION                    STATUS
---------------------------------------------------------------------------------------
spawn_health_polling      30s       M10,M12     poll all 16 services        RUNNING
spawn_observer_cycle      60s       M37-M39     correlate + emerge + evolve RUNNING
spawn_pv2_eventbus_bridge 10s       M23         POST events to PV2:8132    RUNNING
spawn_thermal_polling     60s       M40         GET SYNTHEX /v3/thermal    RUNNING
spawn_field_tracking      10s       N01,N03,N06 GET PV2:8132/health        RUNNING
spawn_self_model          60s       M48         compute layer health        RUNNING
spawn_orac_bridge_polling 30s       M53         GET ORAC:8133/health       RUNNING
spawn_decay_scheduler     300s      M41         POST SYNTHEX decay/trigger  RUNNING
spawn_checkpoint          300s      M56         save CognitiveSnapshot     RUNNING

MISSING TASKS (not spawned):
  spawn_remediation_worker   --     M14         process pending requests    R6
  spawn_eventbus_consumer    --     M23         deliver events to L5        R21
  spawn_dissent_pipeline     --     M35,M57     generate + track dissent    R11
  spawn_auth_refresh         --     M51         rotate service tokens       R12

TASK HEALTH CHECK:
  # All tasks log to tracing. Check for panics:
  grep -i 'panic\|thread.*panicked' /tmp/me-v2-session.log
  # Check task last-seen timestamps:
  curl -s localhost:8080/api/status | jq '.background_tasks'

TASK RESTART (if a task dies):
  Tasks are spawned in main.rs::main() after server bind.
  Full restart: kill process, re-launch binary.
  Individual task restart: not supported (requires code change for task handles).
```

---

## 12. Database Health Matrix

```
DATABASE                 SIZE     ROWS     WRITES/MIN  STATUS     NOTES
---------------------------------------------------------------------------
evolution_tracking.db    3.6MB    19,803   ~5          ACTIVE     Main fitness log
workflow_tracking.db     280KB    25       ~0          IDLE       Startup-only writes
service_tracking.db      260KB    13       ~2          ACTIVE     Health poll results
security_events.db       256KB    0        0           SCHEMA     Tables exist, no data
consensus_tracking.db    248KB    82       0           STALE      No PBFT votes (R10)
hebbian_pulse.db         240KB    2        ~0          IDLE       Only 2 decay pulses
flow_state.db            224KB    5        ~0          IDLE       Startup state only
system_synergy.db        212KB    40       ~1          ACTIVE     Synergy score updates
performance_metrics.db   204KB    7        ~0          IDLE       Infrequent samples
episodic_memory.db       192KB    25       ~0          IDLE       Manual episode records
tensor_memory.db         164KB    6        ~0          IDLE       Snapshot-only writes
remediation_log.db       0B       0        0           EMPTY      No worker (R6)

TOTAL: 12 databases, 5.9MB, 6 active, 4 idle, 1 schema-only, 1 empty

HEALTH CHECKS:
  # Integrity check all databases
  for db in data/databases/*.db; do
    echo "$(basename $db): $(sqlite3 "$db" 'PRAGMA integrity_check;')"
  done

  # WAL mode verification (all should be WAL for concurrent reads)
  for db in data/databases/*.db; do
    echo "$(basename $db): $(sqlite3 "$db" 'PRAGMA journal_mode;')"
  done

  # Size growth check (rapid growth = potential issue)
  du -sh data/databases/*.db | sort -rh

SCHEMA INSPECTION:
  sqlite3 -header -column data/databases/SERVICE_DB.db '.schema'
  # Always .schema before writing SQL (workspace rule)

DEAD DATABASE DIAGNOSIS:
  consensus_tracking.db: PBFT not activated. Fix: R10 (Pattern --> PBFT chain)
  remediation_log.db: No worker processing queue. Fix: R6 (spawn worker)
  security_events.db: M51 Auth creates tables but never writes events. Fix: R12
```

---

## 13. Escalation Pipeline Diagnostic

```
ESCALATION TIERS:

  L0 AUTO-EXECUTE (confidence >= 0.9, severity <= MEDIUM, timeout 0)
    |
    |  Status: PARTIALLY WORKING
    |  M16 ActionExecutor can execute, but M14 RemediationEngine
    |  has no worker consuming pending_requests (R6).
    |  Result: auto-execute path exists but is never entered.
    |
  L1 NOTIFY-HUMAN (confidence >= 0.7, severity <= HIGH, timeout 5min)
    |
    |  Status: NOT WIRED
    |  No notification channel configured. Human @0.A has no
    |  integration point. NAM-R5 (HumanAsAgent) at 0%.
    |  Fix: Add webhook/ORAC hook notification on L1 escalation.
    |
  L2 REQUIRE-APPROVAL (confidence < 0.7 OR severity = HIGH, timeout 30min)
    |
    |  Status: IMPLEMENTED BUT IDLE
    |  M50 ApprovalWorkflow has submit/approve/reject/escalate.
    |  No code creates ApprovalRequest records in production.
    |  Fix: Wire M14 to create ApprovalRequests for L2-tier actions.
    |
  L3 PBFT-CONSENSUS (critical actions, quorum 27/40)
    |
    |  Status: IMPLEMENTED BUT IDLE
    |  M31 PbftManager has 41 agents, create_proposal(), vote().
    |  No production code creates proposals. Zero ballots cast.
    |  Fix: R10 (Pattern --> Antipattern --> PBFT escalation chain)

VERIFICATION:
  curl -s localhost:8080/api/consensus | jq '.total_proposals, .total_votes'
  # Expected after R10: proposals > 0, votes > 0
  curl -s localhost:8080/api/remediation | jq '.pending, .active, .completed'
  # Expected after R6: pending decreasing, active > 0, completed > 0
```

---

## 14. STDP Learning Diagnostic

```
STDP STATE:
  Pathways:            12
  Pathway strength:    uniform 0.492 (all identical -- NO DIFFERENTIATION)
  LTP events:          0
  LTD events:          0
  Timing pairs:        0
  Decay rate:          0.1 (HRS-001 corrected from 0.001)
  Co-activation:       +0.05/call via C12 N04 StdpBridge

WHY NO DIFFERENTIATION:
  M26 StdpProcessor requires timing pairs (pre-synaptic spike, post-synaptic spike
  within 100ms window). Zero timing pairs have been recorded because:
  1. N04 StdpBridge records co-activation via record_interaction() but this
     writes to N04's internal state, NOT to M26 StdpProcessor's timing buffer
  2. No code bridges N04 --> M26 with actual spike events
  3. Without timing pairs, all pathways decay uniformly toward 0.5

FIX (R5, ~20 LOC):
  In spawn_health_polling, after N04 stdp_bridge.record_interaction():
    engine.stdp_processor().record_spike(
      source_service, target_service, now, SpikeType::PreSynaptic
    );
  This creates timing pairs. Pathways will then differentiate based on which
  service pairs are polled together (temporal correlation = LTP strengthening).

EXPECTED AFTER R5:
  Timing pairs:        100+/hour
  Pathway variance:    0.1+ (currently 0.0)
  Strongest pathway:   service pairs polled in same tick window
  Weakest pathway:     service pairs with uncorrelated poll timing
  L5 layer health:     0.49 --> 0.65+

TUNING:
  LTP_RATE (0.1):      Increase for faster learning, decrease for stability
  LTD_RATE (0.05):     Increase for faster forgetting, decrease for retention
  STDP_WINDOW (100ms): Increase for broader correlation, decrease for precision
  DECAY_RATE (0.1):    Decrease to slow pathway erosion (0.01 = very slow)

VERIFICATION:
  curl -s localhost:8080/api/learning | jq '.stdp'
  # Check: timing_pairs > 0, pathway strengths differ by > 0.05
  curl -s localhost:8080/api/learning | jq '.pathways[] | {id, strength}'
  # After R5: strengths should vary from 0.2 to 0.8 (not uniform 0.492)
```

---

## 15. Thermal Subsystem Diagnostic

```
THERMAL STATE:
  ME V2 temperature:     T = 0.40 (independent model)
  SYNTHEX temperature:   T = 0.55 (PID-controlled, target 0.50)
  Divergence:            0.15 (ME cooler than SYNTHEX)
  SYNTHEX PID gains:     Kp=0.3, Ki=0.05, Kd=0.1

HEAT SOURCES (SYNTHEX):
  Hebbian:     0.73  (active STDP pathways -- ORAC LTP=3,732)
  Cascade:     0.13  (low cascade activity)
  Resonance:   0.73  (Kuramoto field coupling)
  CrossSync:   1.00  (maximum cross-service sync)

WHY DIVERGED:
  ME V2 runs its own M40 ThermalMonitor with an independent temperature model.
  It reads SYNTHEX's /v3/thermal data but uses it for observation only, not as
  input to its own temperature. The two models drift independently.

FIX (R8, ~20 LOC):
  In spawn_thermal_polling, after reading SYNTHEX response:
    thermal_monitor.set_temperature(synthex_response.temperature);
  Instead of:
    thermal_monitor.update_from_model();  // independent calculation

THERMAL-DRIVEN EMERGENCE:
  ThermalSpike emergence detector (M38) triggers when |T_delta| > 0.10 per tick.
  Currently never fires because ME's independent model is smooth (no spikes).
  After R8: ME mirrors SYNTHEX oscillations, ThermalSpike can fire on real events.

THERMAL TUNING:
  target_temperature: 0.50 (SYNTHEX default, do not change)
  thermal_spike_threshold: 0.10 (|T_delta| for emergence detection)
  thermal_poll_interval: 60s (balance freshness vs load)

VERIFICATION:
  curl -s localhost:8080/api/v3/thermal | jq '.temperature, .target'
  curl -s localhost:8090/v3/thermal | jq '.temperature, .target'
  # After R8: both should report similar temperatures (within 0.02)
```

---

## 16. Tuning Knobs Reference

All configurable parameters, their current values, safe ranges, and the effect of changing them.

```
CATEGORY: HEALTH POLLING
  health_poll_interval_sec      30      [10-120]    Lower = faster detection, more load
  circuit_breaker_threshold     5       [3-10]      Lower = more sensitive, more flapping
  circuit_breaker_half_open_sec 30      [15-120]    Lower = faster recovery attempts
  circuit_breaker_close_after   3       [1-5]       Lower = faster recovery, less confidence

CATEGORY: OBSERVER PIPELINE
  observer_tick_sec             60      [15-300]    Lower = more correlations, more CPU
  correlation_window_sec        300     [60-600]    Wider = more correlations, more memory
  emergence_ring_buffer_size    1000    [100-10000] Larger = more history, more memory
  emergence_confidence_floor    0.3     [0.1-0.8]   Lower = more emergences, more noise

CATEGORY: RALPH EVOLUTION
  auto_proposal_fitness_gate    0.5     [0.3-0.9]   Lower = mutations at higher fitness
  fitness_driven_gen_gate       5       [0-100]     Lower = mutations start earlier
  dormancy_streak_threshold     3       [1-10]      Lower = faster dormancy response
  evolution_tick_sec            60      [15-300]    Lower = faster evolution, more CPU
  mutation_magnitude            0.1     [0.01-0.5]  Lower = conservative, higher = aggressive

CATEGORY: STDP LEARNING
  ltp_rate                      0.1     [0.01-0.5]  Higher = faster strengthening
  ltd_rate                      0.05    [0.01-0.3]  Higher = faster weakening
  stdp_window_ms                100     [20-500]    Wider = broader temporal correlation
  decay_rate                    0.1     [0.001-0.5] Higher = faster pathway erosion
  co_activation_delta           0.05    [0.01-0.2]  Higher = stronger per-call reinforcement

CATEGORY: NEXUS / KURAMOTO
  K_swarm                       0.5     [0.1-0.9]   Independent parallel threshold
  K_fleet                       1.5     [1.0-1.9]   Coordinated operation threshold
  K_armada                      3.0     [2.0-5.0]   Synchronized convergence threshold
  r_adaptation_threshold        0.05    [0.01-0.2]  Lower = more morphogenic triggers

CATEGORY: PBFT CONSENSUS
  pbft_n                        40      FIXED       Total agents (do not change)
  pbft_f                        13      FIXED       Byzantine tolerance (n = 3f + 1)
  pbft_q                        27      FIXED       Quorum (2f + 1)
  proposal_timeout_sec          300     [60-600]    Time for agents to vote
  view_change_timeout_sec       120     [30-300]    Time before view change on stuck round

CATEGORY: THERMAL
  target_temperature            0.50    [0.3-0.7]   Equilibrium point for PID controller
  thermal_spike_threshold       0.10    [0.05-0.3]  Sensitivity of ThermalSpike detector
  thermal_poll_interval_sec     60      [30-300]    SYNTHEX polling frequency
  decay_trigger_interval_sec    300     [120-600]   How often to trigger SYNTHEX decay

CATEGORY: ESCALATION
  l0_confidence_floor           0.9     [0.8-0.99]  Lower = more auto-executions
  l1_confidence_floor           0.7     [0.5-0.9]   Lower = fewer human notifications
  l1_timeout_sec                300     [60-600]    Wait before proceeding without human
  l2_timeout_sec                1800    [300-3600]  Wait for approval

ANTI-TUNING (DO NOT CHANGE):
  parking_lot vs std::sync:     L1-L5,L7 use parking_lot. L6 uses std::sync. (Design constraint)
  FMA for floats:               All float math uses mul_add(). (Precision constraint)
  WAL journal mode:             All 12 databases use WAL. (Concurrency constraint)
  #![forbid(unsafe_code)]:      Cannot be relaxed. (Security constraint)
```

---

## Quick Diagnostic Runbook

For the most common failure modes, follow these steps in order.

```
SYMPTOM: Fitness stuck at 0.61
  1. Check L2 score:   curl localhost:8080/api/health | jq '.layers.L2'
  2. If L2 < 0.50:     Apply R1 (L2 scoring fix, 15 LOC)
  3. Check RALPH:      curl localhost:8080/api/observer | jq '.evolution'
  4. If mutations=0:   Apply R3 (lower mutation gates, 50 LOC)
  5. Check STDP:       curl localhost:8080/api/learning | jq '.stdp'
  6. If pairs=0:       Apply R5 (wire timing pairs, 20 LOC)

SYMPTOM: Metabolic product < 0.40
  1. Identify lowest:  ME (api/health) vs ORAC (:8133/health) vs PV2 (:8132/health)
  2. If ME lowest:     Follow "Fitness stuck" runbook above
  3. If ORAC lowest:   Check ORAC bridges, RALPH phase, LTP count
  4. If PV2 lowest:    Check sphere count, K regime, r trend

SYMPTOM: EventBus 0 subscribers
  1. Check bridge:     curl localhost:8080/api/event-bus | jq '.bridge_status'
  2. If bridge alive:  PV2 may not have POST /bus/events endpoint (check PV2 routes)
  3. If bridge dead:   Check spawn_pv2_eventbus_bridge task in logs
  4. Long-term:        Apply R21 (EventBus callback delivery)

SYMPTOM: All pathways at 0.492
  1. Check spikes:     curl localhost:8080/api/learning | jq '.stdp.timing_pairs'
  2. If pairs=0:       Apply R5 (bridge N04 --> M26)
  3. If pairs>0:       Check decay_rate (should be 0.1, not 0.001)
  4. If decay=0.001:   F13 regression -- check handle_decay handler for hardcoded values

SYMPTOM: Service shows Unknown health
  1. Check circuit:    curl localhost:8080/api/services | jq '.[] | select(.id=="SVC")'
  2. If circuit=Open:  Service is unreachable. Check service port with curl.
  3. If circuit=Closed: Health poll not running. Check spawn_health_polling task.
  4. If newly added:   F1 services lack circuit breakers -- health poll works but FSM not initialized.

SYMPTOM: PBFT 0 proposals
  1. Check pipeline:   Pattern(M27) --> Antipattern(M30) --> PBFT(M31) -- all unwired
  2. Apply R10:        Wire pattern detection to PBFT proposal creation (~60 LOC)
  3. Verify agents:    curl localhost:8080/api/consensus | jq '.agents'
  4. Should show:      41 agents (20 VALIDATOR, 8 EXPLORER, 6 CRITIC, 4 INTEGRATOR, 2 HISTORIAN, 1 HUMAN)

SYMPTOM: Temperature diverged from SYNTHEX
  1. Compare:          ME: curl :8080/api/v3/thermal | jq .temperature
                       SX: curl :8090/v3/thermal | jq .temperature
  2. If diverged>0.10: Apply R8 (thermal coupling, 20 LOC)
  3. If both high:     Check SYNTHEX heat sources -- CrossSync=1.0 may be overheating
  4. If both low:      Check SYNTHEX cascade health -- low activity = cooling
```

---

## Navigation

| Resource | Path |
|----------|------|
| [Recommendations](../SESSION_068_RECOMMENDATIONS.md) | 21 prioritized recommendations |
| [Meta Tree Mind Map](../META_TREE_MIND_MAP_V2.md) | Full architecture tree + wiring |
| [Advanced Evolution Chamber](../ADVANCED_EVOLUTION_CHAMBER_V2.md) | RALPH V2 design |
| [Habitat Integration](../../ai_specs/HABITAT_INTEGRATION_SPEC.md) | Wiring spec |
| [Schematics Index](SCHEMATICS_INDEX.md) | Mermaid diagram index |
| [Master Index](../../MASTER_INDEX.md) | Project inventory |
| [README](../../README.md) | Quick start |

---

*Diagnostic Schematics | Session 068 | 2026-03-29*
