# ME V2 — Session 068 Comprehensive Recommendations

> **Current state:** fitness=0.612, health=0.650, metabolic=0.327 (HEALTHY)
> **Layers:** L1=1.00 L2=0.33 L3=0.50 L4=1.00 L5=0.49 L6=0.70 L7=1.00
> **RALPH:** gen=5, mutations=0, cycles=8 (INERT)
> **Source:** 5 stress test agents, 3 code exploration agents, 3 fleet panes, 34 commits, 5 bugs fixed
> **Date:** 2026-03-29 | **Session:** 068

---

## TIER 1: CRITICAL IMPACT — Fix These First (~120 LOC total, fitness 0.61→0.80+)

### R1. Fix L2 Health Scoring Bug (fitness impact: +0.25)

**Current:** L2=0.33 despite 11/12 services reachable with health_score=1.0. The L2 layer score computation reports `services_healthy=3` when 11 are actually healthy.

**Root cause:** The `HealthMonitor::healthy_ratio()` computation counts services as `Unknown` (not Healthy) unless they've passed through the `Healthy` state transition in the circuit breaker FSM. Most services go from `Unknown` → polled → `Healthy` but the L2 aggregate score uses a different counter that doesn't track this transition correctly.

**Fix:** In `engine.rs::compute_layer_health()` for L2, use the actual `health_monitor.aggregate_health()` value (which correctly returns ~0.92 for 11/12 services) instead of the `healthy_count / total_count` ratio that only counts FSM-transitioned services.

**LOC:** ~15
**Impact:** L2: 0.33→0.92. Overall health: 0.65→0.82. Single highest-leverage fix.

---

### R2. Wire EventBus → PV2 External Subscriber Bridge (fitness impact: +0.05, metabolic: enables flow)

**Current:** EventBus has 6 channels producing 250 events/min with **zero external subscribers**. 4,991 events produced into a void. This is GAP-D from V1, still unfixed.

**Root cause:** `spawn_pv2_eventbus_bridge` in main.rs POSTs events to PV2:8132/bus/events, but PV2's POST endpoint may not exist or may not be consuming them. The bridge is fire-and-forget with no delivery confirmation.

**Fix:** Two parts:
1. Verify PV2:8132 has a `POST /bus/events` endpoint (this was Action 1 in the Session 064 deployment plan — may already exist)
2. Add subscriber registration in ME V2 startup: `event_bus.subscribe("pv2-bridge", "health", None)` for each channel, making the `subscriber_count` non-zero

**LOC:** ~30
**Impact:** Events flow into the habitat nervous system. ORAC can observe ME V2's internal events via PV2. Foundation for metabolic activation.

---

### R3. Make RALPH Actually Mutate (fitness impact: enables evolution)

**Current:** RALPH gen=5, mutations=0, cycles=8. The evolution chamber is cycling through Recognize→Analyze→Learn→Propose→Harvest but **never proposing mutations**. 1,000 emergence events and 58,820 correlations are observed but never acted upon.

**Root cause:** Three compounding issues:
1. The `propose_mutation()` threshold requires fitness to be below 0.5 for auto-proposals from M38→M39 bridge (current fitness is 0.61 — above threshold)
2. The `fitness_driven_mutations()` path only fires when `tick % 10 == 0 && generation > 5` — generation is only 5
3. The `dormancy_response()` only mutates after `zero_correlation_streak >= 3` — correlations ARE being found (1,460/tick), so streak stays at 0

**Fix:** Lower the auto-proposal fitness threshold from 0.5 to 0.7 (or remove it — mutations should happen at any fitness level). Add a generation-independent mutation path that fires every N ticks regardless of generation count. Wire the V2 Advanced Evolution Chamber's 4-source Learn phase (emergence→dimension→pathway→structural deficit) to provide mutation hints.

**LOC:** ~50
**Impact:** RALPH starts actually evolving parameters. Fitness can climb above the 0.61 plateau.

---

## TIER 2: HIGH IMPACT — Enable V2 Differentiation (~200 LOC total)

### R4. Connect ORAC to ME V2 (bidirectional)

**Current:** ORAC's `m23_me_bridge` is hardcoded to port 8080 (ME V1). ME V2 on port 8180 is completely invisible to ORAC. ORAC reports `me_fitness=0.609` (V1's value, not V2's 0.612).

**Fix options:**
- **Quick:** Change ORAC's `ME_PORT` constant from 8080 to 8180 and rebuild ORAC
- **Better:** Make `ME_PORT` configurable via environment variable: `ME_ADDR=127.0.0.1:8180`
- **Best:** Register ME V2 as a new ORAC bridge alongside ME V1, allowing ORAC to monitor both

**LOC:** 5-30 depending on approach (in ORAC codebase, not ME V2)
**Impact:** ORAC fitness signal reflects ME V2's actual state. RALPH mutations become ME V2-aware. Habitat intelligence sees the new engine.

---

### R5. Wire STDP Co-Activation to Generate Timing Pairs

**Current:** 12 Hebbian pathways all at identical strength (0.492). `timing_pairs_processed: 0`. The STDP processor is configured but receives zero spike events. The `spawn_health_polling` C12 wiring calls `stdp_bridge.record_interaction()` (N04) but this goes to the L8 StdpBridge, not to M26 StdpProcessor.

**Fix:** In `spawn_health_polling`, after the C12 `stdp_bridge.record_interaction()` call, also call `state.engine.stdp_processor().record_spike(source, target, timestamp, SpikeType::PreSynaptic)` to feed the L5 STDP processor directly. This creates the timing pairs that drive pathway differentiation.

**LOC:** ~20
**Impact:** Pathways differentiate from uniform 0.492 to weighted values reflecting actual service interaction patterns. L5 health improves from 0.49 toward 0.70+.

---

### R6. Wire Remediation Worker to Process Pending Requests

**Current:** `submit-remediation` tool accepts requests (65+ submissions in stress test) but `active: 0, success_rate: 0.0`. Requests queue in `pending_requests` but nothing consumes them.

**Fix:** Add a `spawn_remediation_worker` background task that polls `engine.pending_remediations()` every 30s and calls `engine.auto_remediate()` for each pending request. Gate execution by escalation tier: L0 auto-execute, L1 notify+execute, L2/L3 wait for approval via M50 ApprovalWorkflow.

**LOC:** ~40
**Impact:** Remediation pipeline becomes functional. Submissions are processed, outcomes recorded, feeding the FeedbackLoop (M18) and eventually learning.

---

### R7. Connect PeerBridge Failures to Learning Signals

**Current:** `PeerBridgeManager` tracks circuit breaker state and synergy per service, but failures stay internal — no EventBus publish, no SignalBus emission, no Hebbian pathway update.

**Fix:** In `spawn_peer_polling`, after each poll cycle, publish per-service health results to EventBus `integration` channel. On circuit breaker state transitions (Closed→Open, Open→Closed), emit a `LearningEvent` via SignalBus to feed M25 HebbianManager with LTP (recovery) or LTD (failure) signals.

**LOC:** ~30
**Impact:** Bridge health changes become learning signals. Pathways strengthen for reliable service pairs, weaken for unreliable ones.

---

## TIER 3: MEDIUM IMPACT — Complete the Wiring (~300 LOC total)

### R8. Couple ME V2 Thermal with SYNTHEX

**Current:** ME V2 runs independent thermal model at T=0.40 while SYNTHEX is at T=0.55. They share no thermal data. ORAC correctly mirrors SYNTHEX but ME V2 doesn't.

**Fix:** In `spawn_thermal_polling`, after reading SYNTHEX's `/v3/thermal` response, feed the actual SYNTHEX temperature into ME V2's ThermalMonitor instead of computing an independent value. The thermal monitor should reflect SYNTHEX's PID-controlled temperature, not its own model.

**LOC:** ~20
**Impact:** ME V2's `/api/v3/thermal` reflects real SYNTHEX state. Thermal-driven emergence detection (ThermalSpike) triggers from actual thermal events.

---

### R9. Implement Checkpoint Restore on Startup

**Current:** M56 CheckpointManager can save `CognitiveSnapshot` with generation, fitness, mutation counts, RALPH phase — but EvolutionChamber has no `restore_from_checkpoint()` method. On restart, RALPH starts from gen 0 with zero history.

**Fix:** Add `pub fn restore_from_snapshot(&self, snapshot: &CognitiveSnapshot) -> Result<()>` to EvolutionChamber that sets generation, cycle_number, current_phase, paused flag, and seeds fitness_snapshots from the checkpoint's fitness_history. Call it in `Engine::new()` after loading the latest checkpoint from DB.

**LOC:** ~50
**Impact:** RALPH state survives restarts. No more "gen=0 on every restart" — 31 generations of evolution history preserved. Enables long-running autonomous improvement.

---

### R10. Wire Pattern→Antipattern→PBFT Escalation

**Current:** M27 PatternRecognizer detects patterns, M30 AntiPatternDetector has 15 registered patterns, M31 PbftManager has 41 agents — but no code connects them. Pattern detection never escalates to consensus.

**Fix:** In the observer tick cycle, after emergence detection, check actionable patterns against antipattern detector. If a high-severity antipattern is confirmed, create a PBFT proposal via `pbft_manager.create_proposal()`. Wire the PBFT outcome through M50 ApprovalWorkflow.

**LOC:** ~60
**Impact:** The consensus system activates for the first time. 41 agents start voting on detected issues. NAM-R3 DissentCapture begins functioning.

---

### R11. Wire Active Dissent into Consensus Pipeline

**Current:** M57 ActiveDissentGenerator produces 3 counterarguments per proposal (one per AgentPerspective). M35 DissentTracker records passive dissent. They don't connect to each other.

**Fix:** Before each PBFT voting round, call `dissent_generator.pipeline_dissent(&proposal)` and feed the generated `DissentEvent` records into `dissent_tracker.record_dissent()`. After consensus, call `dissent_tracker.mark_valuable(id)` for any dissent that correctly predicted the outcome.

**LOC:** ~30
**Impact:** NAM-R3 DissentCapture goes from 0% to 40%+. Active counterarguments are generated for every proposal. Valuable dissent is tracked and reinforced.

---

### R12. Add Auth→RateLimit→OracBridge Request Chain

**Current:** M51 (Auth), M52 (RateLimit), M53 (OracBridge) are three independent modules. No code sequences them into an auth→rate-check→execute chain.

**Fix:** In `spawn_orac_bridge_polling`, before each poll cycle, verify a service token via `auth_manager.verify_token()` and check rate limits via `rate_limiter.check_and_consume("orac-bridge", ServiceTier::Tier1)`. On `RateDecision::Reject`, skip the poll and log the throttle.

**LOC:** ~25
**Impact:** ORAC bridge respects authentication and rate limiting. Security infrastructure activates. The Auth→RateLimit chain is validated in production.

---

## TIER 4: LOWER IMPACT — Polish and Harden (~200 LOC total)

### R13. Diversify Emergence Detection Types

**Current:** All 1,000 emergences are `AttractorFormation` with identical confidence (0.655). Only 2 of 8 emergence types fire (AttractorFormation + CascadeFailure). PhaseTransition, SynergyShift, ResonanceCycle, BeneficialEmergence never trigger.

**Fix:** Lower detection thresholds for underrepresented emergence types. Add synthetic probes that test each detector: inject test events that should trigger PhaseTransition (value ratio > 2.0), SynergyShift (|delta| >= 0.15), etc. Verify all 8 types can fire under realistic conditions.

**LOC:** ~40
**Impact:** RALPH receives diverse emergence signals. The 4-source Learn phase has richer input. Evolution proposals target actual system problems rather than only attractor formation.

---

### R14. Fix L6 Lock Ordering in dissent.rs

**Current:** `record_dissent()` holds 3 nested `std::sync::RwLock::write()` guards during re-indexing when the buffer cap is reached: `dissent_events` → `dissent_by_proposal` → `dissent_by_agent`.

**Fix:** Restructure to collect the new index data before acquiring any write locks, then acquire and swap in a single pass. Or replace `std::sync::RwLock` with `parking_lot::RwLock` which eliminates the poison risk and performs better under contention.

**LOC:** ~30
**Impact:** Eliminates potential deadlock under concurrent dissent recording. Improves L6 thread safety.

---

### R15. Replace Hardcoded JSON in All API Handlers

**Current:** F13 was one instance of a hardcoded JSON literal bypassing live config. There may be other handlers that return static values instead of reading from the Engine's actual state.

**Fix:** Audit all `handle_*` functions in main.rs. Replace any literal numeric values with accessor calls on the Engine struct. Specifically check: `/api/consensus` for PBFT constants, `/api/status` for architecture counts, `/api/observer` for any static values.

**LOC:** ~30
**Impact:** API responses always reflect live state. Eliminates the class of bug where config changes don't propagate to monitoring.

---

### R16. Push ME V2 to Git Remote

**Current:** ME V2 has 34 commits on local `main` but no remote configured. All work is only on the local machine.

**Fix:** Create GitLab and GitHub repositories, add remotes, push:
```bash
git remote add gitlab git@gitlab.com:lukeomahoney/the-maintenance-engine-v2.git
git remote add origin git@github.com:Louranicas/the-maintenance-engine-v2.git
git push -u gitlab main && git push -u origin main
```

**LOC:** 0
**Impact:** Code is backed up and accessible from anywhere. Enables CI/CD, code review, and collaboration.

---

### R17. Register ME V2 in devenv.toml

**Current:** ME V2 is not registered with the ULTRAPLATE developer environment manager. It runs manually via `nohup bin/maintenance_engine_v2 start --port 8180 &`.

**Fix:** Add a `[[services]]` entry to `~/.config/devenv/devenv.toml`:
```toml
[[services]]
id = "maintenance-engine-v2"
name = "Maintenance Engine V2"
working_dir = "/home/louranicas/claude-code-workspace/the_maintenance_engine_v2"
command = "./bin/maintenance_engine_v2"
args = ["start", "--port", "8180"]
auto_start = true
dependencies = ["devops-engine"]
```

**LOC:** ~15 (config, not code)
**Impact:** ME V2 starts/stops/restarts with the habitat. Health monitoring via devenv. Process lifecycle management.

---

### R18. Implement `From<u8> for ServiceTier`

**Current:** M51 Auth's `TokenIdentity.tier` is a `u8` while M52 RateLimit works with `ServiceTier` (enum). Callers must manually map between them, risking silent mismatches.

**Fix:** Add `impl From<u8> for ServiceTier` on the ServiceTier enum in `m2_services/mod.rs`.

**LOC:** ~10
**Impact:** Type-safe tier conversion. Eliminates the class of bug where tier 0 silently maps to the wrong rate limit.

---

## TIER 5: ARCHITECTURAL EVOLUTION — Next-Generation Work

### R19. Implement Advanced Evolution Chamber (V2 Design)

**Reference:** `ai_docs/ADVANCED_EVOLUTION_CHAMBER_V2.md` (537 lines, fully designed)

The V2 design addresses every observed stagnation pattern:
- 4-source Learn phase (emergence→dimension→pathway→structural deficit)
- 5 evolution strategies (Conservative, Exploratory, StructuralRepair, Convergence, Morphogenic)
- Field-coherence gate via N05 (test-then-apply, not apply-then-rollback)
- Mandatory mutation recording (zero NULL fields — fixes V1's blind evolution)
- 15s evolution tick (target 240 gen/h vs current 1.8 gen/h)
- Cognitive state persistence via M56

**LOC:** ~881 (enhancement to existing 1,619 LOC module)
**Impact:** Transforms RALPH from inert to actively evolving. Estimated fitness trajectory: 0.61→0.85+ over 500 ticks.

---

### R20. Implement L8 Nexus Runtime Integration (N01-N06 full wiring)

**Current:** All 6 Nexus modules are implemented with 50+ tests each, but only N01 (FieldBridge), N03 (RegimeManager), and N06 (MorphogenicAdapter) are wired into the runtime via `spawn_field_tracking`. N02 (IntentRouter), N04 (StdpBridge), and N05 (EvolutionGate) are initialized in the Engine but never called from any background task.

**Fix:** Wire N02 into service routing decisions (replace round-robin with intent-tensor-weighted routing). Wire N04 into health polling (already partially done via C12 but needs L5 StdpProcessor bridging). Wire N05 into RALPH's Harvest phase (gate mutations through field coherence test before applying).

**LOC:** ~150
**Impact:** The full L8 Nexus layer becomes operational. K-regime awareness, intent-based routing, field-gated evolution, and STDP co-activation learning all activate.

---

### R21. Implement EventBus Callback Delivery

**Current:** `EventBus::publish()` computes `delivered_to` IDs but never invokes any consumer callback. The entire learning pipeline (L5 Hebbian, L5 STDP, L5 Pattern) depends on external polling that doesn't exist.

**Fix:** Add a `Subscriber` trait with an `on_event(&self, event: &EventRecord)` callback. Store `Arc<dyn Subscriber>` alongside subscriber IDs. In `publish()`, call each subscriber's `on_event()` synchronously (matching SignalBus's delivery pattern). This turns the EventBus from passive bookkeeping into active event delivery.

**LOC:** ~80
**Impact:** Fundamental architectural upgrade. Events flow automatically from producers to consumers. Learning pipeline activates without polling. Remediation events trigger responses. Consensus events create dissent records.

---

## Implementation Priority Matrix

```
                     HIGH IMPACT
                         │
        R1(L2 fix)       │       R19(Adv Evolution)
        R3(RALPH mutate) │       R21(EventBus callbacks)
        R2(EventBus→PV2) │       R20(L8 full wiring)
                         │
  LOW ───────────────────┼─────────────────── HIGH
  EFFORT                 │                    EFFORT
                         │
        R4(ORAC bridge)  │       R10(Pattern→PBFT)
        R5(STDP timing)  │       R9(Checkpoint restore)
        R6(Remediation)  │       R8(Thermal coupling)
        R7(PeerBridge→Learn) │   R11(Dissent wiring)
        R16(Git push)    │       R13(Emergence diversity)
        R17(devenv reg)  │       R14(L6 lock fix)
                         │
                     LOW IMPACT
```

---

## Execution Order (Recommended)

```
Sprint 1 — Unlock fitness ceiling (~4 hours, +0.20 fitness):
  R1  Fix L2 health scoring            15 LOC
  R3  Make RALPH mutate                 50 LOC
  R5  Wire STDP timing pairs           20 LOC
  R16 Push to Git remote                0 LOC

Sprint 2 — Connect the nervous system (~4 hours, enables metabolic flow):
  R2  EventBus → PV2 bridge            30 LOC
  R4  Connect ORAC to ME V2            30 LOC
  R7  PeerBridge → learning signals    30 LOC
  R6  Remediation worker               40 LOC

Sprint 3 — Activate consensus + evolution (~6 hours, enables NAM):
  R10 Pattern → PBFT escalation        60 LOC
  R11 Active dissent → consensus       30 LOC
  R9  Checkpoint restore               50 LOC
  R8  Thermal coupling                 20 LOC

Sprint 4 — Advanced architecture (~8 hours, transforms system):
  R19 Advanced Evolution Chamber       881 LOC
  R20 L8 Nexus full wiring             150 LOC
  R21 EventBus callback delivery        80 LOC

Sprint 5 — Polish (~2 hours):
  R12 Auth → RateLimit chain            25 LOC
  R13 Emergence diversity               40 LOC
  R14 L6 lock ordering fix              30 LOC
  R15 Audit hardcoded handlers          30 LOC
  R17 devenv registration               15 LOC
  R18 From<u8> for ServiceTier          10 LOC
```

---

## Expected Trajectory

| Metric | Current | After Sprint 1 | After Sprint 2 | After Sprint 3 | After Sprint 4 |
|--------|---------|----------------|----------------|----------------|----------------|
| Fitness | 0.612 | 0.80+ | 0.82+ | 0.85+ | 0.88+ |
| L2 Services | 0.33 | 0.92 | 0.92 | 0.92 | 0.95 |
| L5 Learning | 0.49 | 0.60 | 0.65 | 0.70 | 0.80 |
| RALPH mutations | 0 | 10+/hour | 10+/hour | 50+/hour | 240+/hour |
| Metabolic product | 0.327 | 0.45 | 0.55+ | 0.60+ | 0.70+ |
| NAM compliance | 0% | 15% | 30% | 55% | 75% |
| EventBus subscribers | 0 | 0 | 1+ (PV2) | 1+ | 5+ (callbacks) |
| PBFT ballots | 0 | 0 | 0 | 5+ | 20+ |
| Emergence types | 2/8 | 2/8 | 2/8 | 4/8 | 8/8 |

---

## Cross-References

| Resource | Path |
|----------|------|
| Advanced Evolution Chamber V2 | `ai_docs/ADVANCED_EVOLUTION_CHAMBER_V2.md` |
| Habitat Integration Spec | `ai_specs/HABITAT_INTEGRATION_SPEC.md` |
| Meta Tree Mind Map V2 | `ai_docs/META_TREE_MIND_MAP_V2.md` |
| Session 064 Deploy Plan | `~/projects/shared-context/Session 064 — Future Deployment Plan (NAM Book ACP).md` |
| Session 068 Stress Test Results | `~/projects/shared-context/Session 068 — ME V2 Stress Test Results.md` |
| Session 068 Synthesis | `~/projects/shared-context/Session 068 — ME V2 Scaffold Deployment and Module Alignment.md` |

---

*21 recommendations from 200+ hours of live data, 5 stress tests, 3 code explorations, 5 bugs fixed.*
*Prioritized by fitness impact × implementation effort. Sprint 1 delivers +0.20 fitness in ~4 hours.*
*Session 068 | 2026-03-29*
