# L8: Nexus Integration Layer Specification

> Target: ~6,000 LOC | 6 modules (N01-N06) | 300+ tests | **NEW IN V2**

---

## Layer Purpose

The Nexus layer bridges the Maintenance Engine V2 with the Oscillating Vortex Memory (OVM) and Nexus Controller systems. It provides Kuramoto field coherence tracking, intent-based service routing, K-regime awareness, cross-system STDP learning, evolution gating, and morphogenic adaptation. This layer is the primary V2 enhancement over V1.

---

## Source Patterns

| Component | Source | What It Provides |
|-----------|--------|------------------|
| Field Bridge | VMS `HookEngine` | Pre/post field capture, r-tracking |
| Intent Router | VMS `IntentEncoder` | 12D tensor cosine routing to services |
| Regime Manager | VMS `SwarmCoordinator` | K-regime detection and transitions |
| STDP Bridge | VMS `StdpKernel` | Tool chain learning, co-activation |
| Evolution Gate | VMS `EvolutionChamber` | Mutation testing before deployment |
| Morphogenic Adapter | VMS `MorphogenicEngine` | Adaptation on |r_delta| > threshold |

---

## Module Specifications

### N01: Field Bridge (`field_bridge.rs`)

**Purpose:** Track Kuramoto order parameter r before and after every significant operation, providing field coherence awareness to all layers.

**Target:** ~1,000 LOC, 50+ tests

**Key Types:**
```rust
pub struct FieldBridge { inner: RwLock<FieldBridgeInner> }

pub struct FieldSnapshot {
    r: f64,                    // Kuramoto order parameter [0.0, 1.0]
    r_trend: FieldTrend,       // Rising, Stable, Falling
    temperature: f64,          // Thermal state
    active_oscillators: usize, // Number of coupled oscillators
    timestamp: Timestamp,
}

pub struct FieldCapture {
    r_before: f64,
    r_after: f64,
    r_delta: f64,              // r_after - r_before
    operation: String,
    duration: Duration,
}

pub enum FieldTrend { Rising, Stable, Falling, Volatile }
```

**Key Traits:**
```rust
pub trait FieldBridgeOps: Send + Sync {
    fn current_r(&self) -> Result<f64>;
    fn capture_pre(&self) -> Result<f64>;
    fn capture_post(&self, r_before: f64, operation: &str) -> Result<FieldCapture>;
    fn field_snapshot(&self) -> Result<FieldSnapshot>;
    fn r_history(&self, limit: usize) -> Result<Vec<(Timestamp, f64)>>;
    fn is_coherent(&self) -> Result<bool>;  // r > 0.7
}
```

**Integration:**
- Connects to SVF WebSocket `/ws/field-evolution` via M21
- Polls VMS MCP `/mcp/tools/call` with `{"tool":"coherence_report"}` via M19
- Writes r-history to `tensor_memory.db`

**Tensor Contribution:** D8 (synergy via r), D11 (temporal via field trend)

**Signals:** `FieldCoherenceChanged`, `FieldVolatile`, `FieldCritical` (r < 0.3)

---

### N02: Intent Router (`intent_router.rs`)

**Purpose:** Route maintenance intents to appropriate ULTRAPLATE services using 12D tensor cosine similarity.

**Target:** ~1,000 LOC, 50+ tests

**Key Types:**
```rust
pub struct IntentRouter { inner: RwLock<IntentRouterInner> }

pub struct Intent {
    description: String,
    tensor: [f64; 12],        // Encoded intent
    priority: Priority,
    source: ModuleId,
}

pub struct RouteResult {
    target_service: ServiceId,
    similarity: f64,           // Cosine similarity
    confidence: f64,
    fallback: Option<ServiceId>,
}
```

**Key Traits:**
```rust
pub trait IntentRouting: Send + Sync {
    fn encode_intent(&self, description: &str) -> Result<[f64; 12]>;
    fn route(&self, intent: &Intent) -> Result<RouteResult>;
    fn service_tensor(&self, service: &ServiceId) -> Result<[f64; 12]>;
    fn update_service_tensor(&self, service: &ServiceId, tensor: [f64; 12]) -> Result<()>;
}
```

**Routing Algorithm:**
```
cosine_sim(intent_tensor, service_tensor) = dot(a, b) / (|a| * |b|)
Select service with highest cosine similarity above threshold (0.3)
```

**Service Tensor Map (initial):**
- SYNTHEX: diagnostics, health, neural
- SAN-K7: orchestration, modules, integration
- DevOps: pipelines, deployment, CI/CD
- ME: maintenance, monitoring, remediation
- SVF: memory, field, tensor

---

### N03: Regime Manager (`regime_manager.rs`)

**Purpose:** Detect and manage Kuramoto coupling regime (K-value) transitions.

**Target:** ~900 LOC, 50+ tests

**Key Types:**
```rust
pub struct RegimeManager { inner: RwLock<RegimeInner> }

pub enum KRegime {
    Swarm,   // K < 1.0 — independent parallel agents, low coupling
    Fleet,   // 1.0 <= K < 2.0 — coordinated parallel, medium coupling
    Armada,  // K >= 2.0 — synchronized convergence, max coupling
}

pub struct RegimeStatus {
    current_k: f64,
    regime: KRegime,
    r: f64,
    transition_history: Vec<RegimeTransition>,
}
```

**Key Traits:**
```rust
pub trait RegimeOps: Send + Sync {
    fn current_regime(&self) -> Result<KRegime>;
    fn current_k(&self) -> Result<f64>;
    fn suggest_regime(&self, task_complexity: f64, agent_count: u32) -> Result<KRegime>;
    fn transition_to(&self, target: KRegime) -> Result<RegimeTransition>;
    fn is_stable(&self) -> Result<bool>;
}
```

**Regime Selection Heuristic:**
- Simple independent tasks → Swarm (K=0.5)
- Multi-step coordinated work → Fleet (K=1.5)
- Critical convergence/consensus → Armada (K=3.0)

---

### N04: STDP Bridge (`stdp_bridge.rs`)

**Purpose:** Record tool chain STDP co-activations from service interactions, feeding back into L5 Hebbian learning.

**Target:** ~900 LOC, 50+ tests

**Key Types:**
```rust
pub struct StdpBridge { inner: RwLock<StdpBridgeInner> }

pub struct ToolChainRecord {
    tools: Vec<ToolId>,
    services: Vec<ServiceId>,
    duration: Duration,
    success: bool,
    r_delta: f64,
}

pub struct CoActivation {
    source: ServiceId,
    target: ServiceId,
    delta: f64,              // +0.05 per co-activation (C12)
    timestamp: Timestamp,
}
```

**Key Traits:**
```rust
pub trait StdpBridgeOps: Send + Sync {
    fn record_interaction(&self, source: ServiceId, target: ServiceId) -> Result<()>;
    fn record_tool_chain(&self, chain: ToolChainRecord) -> Result<()>;
    fn co_activation_count(&self, source: &ServiceId, target: &ServiceId) -> Result<u64>;
    fn synergy_pairs(&self, threshold: f64) -> Result<Vec<(ServiceId, ServiceId, f64)>>;
}
```

**Integration:** Feeds into M25 Hebbian Manager and M26 STDP Processor in L5.

---

### N05: Evolution Gate (`evolution_gate.rs`)

**Purpose:** Gate deployments and configuration changes through RALPH evolution testing.

**Target:** ~1,000 LOC, 50+ tests

**Key Types:**
```rust
pub struct EvolutionGate { inner: RwLock<EvolutionGateInner> }

pub struct GateRequest {
    change: ProposedChange,
    baseline_r: f64,
    min_r_threshold: f64,     // Default: baseline_r (must not degrade)
    max_steps: u32,           // Default: 500
    sphere_count: u32,        // Default: 5
    k_value: f64,             // Default: 1.0
}

pub struct GateResult {
    r_baseline: f64,
    r_after: f64,
    r_delta: f64,
    verdict: GateVerdict,     // Pass / Fail / Inconclusive
    steps_run: u32,
    mutations_tested: u32,
}

pub enum GateVerdict { Pass, Fail, Inconclusive }
```

**Key Traits:**
```rust
pub trait EvolutionGating: Send + Sync {
    fn evaluate(&self, request: GateRequest) -> Result<GateResult>;
    fn quick_check(&self, change: &ProposedChange) -> Result<GateVerdict>;
    fn history(&self, limit: usize) -> Result<Vec<GateResult>>;
}
```

**Gate Protocol:**
1. Capture baseline field state (r_baseline)
2. Apply proposed change in isolated sandbox
3. Run RALPH loop (K=1.0, 500 steps, 5 spheres)
4. Measure r_after
5. Pass if `r_after >= r_baseline`, Fail otherwise

---

### N06: Morphogenic Adapter (`morphogenic_adapter.rs`)

**Purpose:** Trigger adaptive responses when field coherence shifts significantly (|r_delta| > 0.05).

**Target:** ~1,000 LOC, 50+ tests

**Key Types:**
```rust
pub struct MorphogenicAdapter { inner: RwLock<MorphogenicInner> }

pub enum AdaptationType {
    IncreaseK,     // More coupling (r too low)
    DecreaseK,     // Less coupling (r too high, system rigid)
    RebalanceSTDP, // Adjust learning rates
    TriggerPruning,// Prune weak pathways
    EmitWarning,   // Alert but don't act
}

pub struct AdaptationEvent {
    trigger: FieldCapture,
    adaptation: AdaptationType,
    parameters_before: Vec<(String, f64)>,
    parameters_after: Vec<(String, f64)>,
    timestamp: Timestamp,
}
```

**Key Traits:**
```rust
pub trait MorphogenicOps: Send + Sync {
    fn should_adapt(&self, capture: &FieldCapture) -> Result<bool>;
    fn select_adaptation(&self, capture: &FieldCapture) -> Result<AdaptationType>;
    fn apply_adaptation(&self, adaptation: AdaptationType) -> Result<AdaptationEvent>;
    fn adaptation_history(&self, limit: usize) -> Result<Vec<AdaptationEvent>>;
}
```

**Adaptation Logic:**
- r_delta > +0.05 and r > 0.95 → DecreaseK (too rigid)
- r_delta < -0.05 and r < 0.5 → IncreaseK (losing coherence)
- |r_delta| > 0.1 → TriggerPruning + RebalanceSTDP (major shift)
- 0.05 < |r_delta| < 0.1 → EmitWarning (monitor)

---

## Layer Coordinator (`mod.rs`)

**Target:** ~700 LOC, 30+ tests

**Provides:**
```rust
pub struct NexusLayer {
    field_bridge: Arc<FieldBridge>,
    intent_router: Arc<IntentRouter>,
    regime_manager: Arc<RegimeManager>,
    stdp_bridge: Arc<StdpBridge>,
    evolution_gate: Arc<EvolutionGate>,
    morphogenic_adapter: Arc<MorphogenicAdapter>,
}
```

**Key Methods:**
- `NexusLayer::builder()` — builder pattern with all 6 modules
- `status()` → `NexusStatus` aggregate (r, K, regime, adaptations, pathways)
- `observe_field()` — single field observation cycle
- `gate_deployment(change)` — evolution gate check before deployment
- `process_field_capture(capture)` — adaptation pipeline

**HTTP Endpoints (served by main.rs):**
- `GET /api/nexus/status` — NexusStatus
- `GET /api/nexus/field` — FieldSnapshot
- `GET /api/nexus/regime` — RegimeStatus
- `POST /api/nexus/gate` — Run evolution gate
- `GET /api/nexus/adaptations` — Adaptation history

---

## Design Constraints

- C1: Can import from ALL lower layers (L1-L7) — top of the DAG
- C2: All trait methods `&self`
- C3: `TensorContributor` on N01 (D8, D11), N02 (routing quality)
- C4: Zero unsafe/unwrap/expect
- C11: This layer IS the field capture layer — all modules participate
- C12: N04 is the canonical STDP recorder

---

## Databases Used

- `tensor_memory.db` — r-history, field snapshots
- `hebbian_pulse.db` — STDP co-activations via N04
- `evolution_tracking.db` — gate results via N05
- `system_synergy.db` — synergy pairs via N04

---

## Test Strategy

- Unit tests: 50+ per module
- Integration: `tests/l8_nexus_integration.rs`
- Property: r always in [0.0, 1.0], K regime transitions are monotonic, adaptations are idempotent within cool-down period
- Mock: Field bridge mocked for unit tests (no live SVF dependency)
