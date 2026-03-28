# Module Cross-Reference Matrix

> 39 module specifications for The Maintenance Engine v1.0.0 (36 implemented + 3 planned)

---

## Layer Summary

| Layer | ID | Modules | Primary Focus |
|-------|-----|---------|--------------|
| Foundation | L1 | M01-M06 | Error, config, logging, metrics, state, resources |
| Services | L2 | M07-M12 | Health, discovery, mesh, traffic, load balancing, circuit breaker |
| Learning | L3 | M13-M18 | STDP, pathways, patterns, clustering, sequences, prediction |
| Consensus | L4 | M19-M24 | PBFT, views, agents, quorum, checkpoints, messages |
| Remediation | L5 | M25-M30 | Engine, escalation, actions, approval, rollback, feedback |
| Integration | L6 | M31-M36 | API, events, bridges, auth, rate limiting, adapters |
| Observer | L7 | M37-M39 | Log correlation, emergence detection, evolution (PLANNED) |

---

## Complete Module Table

| ID | Name | Layer | Database | Primary API |
|----|------|-------|----------|-------------|
| M01 | Error Taxonomy | L1 | - | `classify(error) → ErrorVector[12]` |
| M02 | Configuration Manager | L1 | - | `get<T>(key) → Option<T>` |
| M03 | Logging System | L1 | - | `log(level, msg, fields)` |
| M04 | Metrics Collector | L1 | performance_metrics.db | `counter/gauge/histogram()` |
| M05 | State Persistence | L1 | service_tracking.db | `save/load(key, state)` |
| M06 | Resource Manager | L1 | - | `allocate/release(resource)` |
| M07 | Health Monitor | L2 | service_tracking.db | `check(service_id) → Health` |
| M08 | Service Discovery | L2 | service_tracking.db | `register/discover(service)` |
| M09 | Service Mesh Controller | L2 | system_synergy.db | `route(request) → endpoint` |
| M10 | Traffic Manager | L2 | flow_state.db | `shape(traffic) → policy` |
| M11 | Load Balancer | L2 | - | `balance(requests) → target` |
| M12 | Circuit Breaker | L2 | - | `guard(call) → Result` |
| M13 | STDP Engine | L3 | hebbian_pulse.db | `update_weights(pre, post, Δt)` |
| M14 | Pathway Manager | L3 | hebbian_pulse.db | `create/strengthen/prune()` |
| M15 | Pattern Recognition | L3 | tensor_memory.db | `recognize(tensor) → Pattern` |
| M16 | Error Clustering | L3 | tensor_memory.db | `cluster(errors) → Clusters` |
| M17 | Sequence Detection | L3 | episodic_memory.db | `detect(events) → Sequence` |
| M18 | Prediction Engine | L3 | tensor_memory.db | `predict(state) → Future` |
| M19 | PBFT Engine | L4 | consensus_tracking.db | `propose/prepare/commit()` |
| M20 | View Manager | L4 | consensus_tracking.db | `view_change(reason)` |
| M21 | Agent Coordinator | L4 | consensus_tracking.db | `coordinate(agents, task)` |
| M22 | Quorum Manager | L4 | consensus_tracking.db | `verify_quorum(votes)` |
| M23 | Checkpoint Manager | L4 | consensus_tracking.db | `checkpoint/restore()` |
| M24 | Message Handler | L4 | - | `send/receive(message)` |
| M25 | Remediation Engine | L5 | - | `remediate(error) → Action` |
| M26 | Escalation Manager | L5 | - | `escalate(action, tier)` |
| M27 | Action Registry | L5 | - | `register/lookup(action)` |
| M28 | Approval Workflow | L5 | - | `request/approve/reject()` |
| M29 | Rollback Handler | L5 | - | `rollback(action)` |
| M30 | Feedback Loop | L5 | hebbian_pulse.db | `feedback(outcome)` |
| M31 | API Gateway | L6 | - | `route(request) → response` |
| M32 | Event Streaming | L6 | - | `publish/subscribe(event)` |
| M33 | Service Bridge Hub | L6 | system_synergy.db | `bridge(service) → client` |
| M34 | Authentication Handler | L6 | security_events.db | `authenticate(credentials)` |
| M35 | Rate Limiter | L6 | security_events.db | `check_limit(key)` |
| M36 | External Adapters | L6 | - | `adapt(external) → internal` |
| M37 | Log Correlator | L7 (PLANNED) | observer_state.db | `correlate(events) → Correlation` |
| M38 | Emergence Detector | L7 (PLANNED) | observer_state.db | `detect(observations) → Emergence` |
| M39 | Evolution Chamber | L7 (PLANNED) | observer_state.db | `evolve(patterns) → Candidate` |

---

## Dependency Matrix

```
      M01 M02 M03 M04 M05 M06 M07 M08 M09 M10 M11 M12
M01    -   .   X   .   .   .   .   .   .   .   .   .
M02    X   -   X   .   X   .   .   .   .   .   .   .
M03    X   X   -   X   .   .   .   .   .   .   .   .
M04    X   X   X   -   X   .   .   .   .   .   .   .
M05    X   X   X   .   -   .   .   .   .   .   .   .
M06    X   X   X   X   .   -   .   .   .   .   .   .
M07    X   X   X   X   X   .   -   X   .   .   .   X
M08    X   X   X   X   X   .   .   -   X   .   .   .
M09    X   X   X   X   .   .   X   X   -   X   X   X
M10    X   X   X   X   .   .   X   .   X   -   X   X
M11    X   X   X   X   .   .   X   X   .   X   -   X
M12    X   X   X   X   X   .   X   .   .   .   .   -

      M13 M14 M15 M16 M17 M18 M19 M20 M21 M22 M23 M24
M13    -   X   .   .   .   .   .   .   .   .   .   .
M14    X   -   .   .   .   .   .   .   .   .   .   .
M15    .   X   -   X   .   .   .   .   .   .   .   .
M16    .   X   X   -   .   .   .   .   .   .   .   .
M17    .   X   X   .   -   X   .   .   .   .   .   .
M18    X   X   X   X   X   -   .   .   .   .   .   .
M19    .   .   .   .   .   .   -   X   X   X   X   X
M20    .   .   .   .   .   .   X   -   X   X   X   X
M21    .   .   .   .   .   .   X   X   -   X   X   X
M22    .   .   .   .   .   .   X   X   X   -   X   X
M23    .   .   .   .   .   .   X   X   X   X   -   X
M24    .   .   .   .   .   .   X   X   X   X   X   -

      M25 M26 M27 M28 M29 M30 M31 M32 M33 M34 M35 M36
M25    -   X   X   X   X   X   .   .   .   .   .   .
M26    X   -   X   X   .   .   .   .   .   .   .   .
M27    X   .   -   .   .   .   .   .   .   .   .   .
M28    X   X   X   -   .   .   .   .   .   .   .   .
M29    X   X   X   .   -   X   .   .   .   .   .   .
M30    X   .   .   .   X   -   .   .   .   .   .   .
M31    .   .   .   .   .   .   -   X   X   X   X   X
M32    .   .   .   .   .   .   X   -   X   X   .   .
M33    .   .   .   .   .   .   X   X   -   X   .   X
M34    .   .   .   .   .   .   X   .   .   -   X   .
M35    .   .   .   .   .   .   X   .   .   X   -   .
M36    .   .   .   .   .   .   X   X   X   X   X   -

Legend: X = depends on, . = no dependency
```

---

## Data Flow Paths

### Error → Remediation Flow
```
Error → M01(Classify) → M15(Pattern) → M14(Pathway) → M25(Remediate)
                                                            ↓
                                            M26(Escalate) → [L0/L1/L2/L3]
                                                            ↓
                                            M19(PBFT) ← if L3
                                                            ↓
                                            M30(Feedback) → M13(STDP)
```

### Health → Learning Flow
```
Service → M07(Health) → M04(Metrics) → M15(Pattern) → M13(STDP)
              ↓                                           ↓
          M05(State) → tensor_memory.db           M14(Pathway)
```

### External → Internal Flow
```
External → M31(Gateway) → M34(Auth) → M35(Limit) → M33(Bridge)
                                                        ↓
                                                   M09(Mesh) → Service
```

---

## Module API Signatures

### L1: Foundation

```rust
// M01: Error Taxonomy
pub fn classify(&self, error: &dyn Error) -> ErrorVector;
pub fn similarity(&self, a: &ErrorVector, b: &ErrorVector) -> f64;
pub fn encode(&self, error: &dyn Error) -> [f64; 12];

// M02: Configuration Manager
pub fn load(&mut self, path: &Path) -> Result<()>;
pub fn get<T: DeserializeOwned>(&self, key: &str) -> Option<T>;
pub fn watch(&self) -> ConfigWatcher;

// M03: Logging System
pub fn log(&self, level: Level, message: &str, fields: &LogFields);
pub fn with_correlation(&self, id: CorrelationId) -> Logger;

// M04: Metrics Collector
pub fn counter(&self, name: &str, labels: &Labels) -> Counter;
pub fn gauge(&self, name: &str, labels: &Labels) -> Gauge;
pub fn histogram(&self, name: &str, buckets: &[f64]) -> Histogram;

// M05: State Persistence
pub async fn save<T: Serialize>(&self, key: &str, state: &T) -> Result<()>;
pub async fn load<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>>;

// M06: Resource Manager
pub fn allocate(&mut self, request: ResourceRequest) -> Result<ResourceHandle>;
pub fn release(&mut self, handle: ResourceHandle) -> Result<()>;
```

### L2: Services

```rust
// M07: Health Monitor
pub async fn check(&self, service_id: &str) -> HealthStatus;
pub async fn check_all(&self) -> Vec<HealthStatus>;

// M08: Service Discovery
pub async fn register(&self, service: ServiceInfo) -> Result<()>;
pub async fn discover(&self, query: ServiceQuery) -> Vec<ServiceInfo>;

// M09-M12: Mesh, Traffic, Load Balancer, Circuit Breaker
pub fn route(&self, request: &Request) -> Result<Endpoint>;
pub fn balance(&self, targets: &[Target]) -> Target;
pub async fn guard<F, T>(&self, f: F) -> Result<T>;
```

### L3: Learning

```rust
// M13: STDP Engine
pub fn update(&mut self, pre: NeuronId, post: NeuronId, delta_t: Duration);

// M14: Pathway Manager
pub fn create(&mut self, source: NeuronId, target: NeuronId) -> PathwayId;
pub fn strengthen(&mut self, id: PathwayId, delta: f64);
pub fn prune(&mut self, threshold: f64) -> Vec<PathwayId>;

// M15-M18: Pattern, Clustering, Sequence, Prediction
pub fn recognize(&self, tensor: &Tensor12D) -> Option<Pattern>;
pub fn cluster(&self, tensors: &[Tensor12D]) -> Vec<Cluster>;
pub fn predict(&self, history: &[State]) -> Prediction;
```

### L4: Consensus

```rust
// M19: PBFT Engine
pub async fn propose(&self, proposal: Proposal) -> Result<ProposalId>;
pub async fn prepare(&self, id: ProposalId) -> Result<PrepareResult>;
pub async fn commit(&self, id: ProposalId) -> Result<CommitResult>;

// M20-M24: View, Agent, Quorum, Checkpoint, Message
pub async fn view_change(&self, reason: ViewChangeReason) -> Result<View>;
pub fn verify_quorum(&self, votes: &[Vote]) -> bool;
pub async fn checkpoint(&self) -> Result<CheckpointId>;
```

### L5: Remediation

```rust
// M25: Remediation Engine
pub async fn remediate(&self, error: &ClassifiedError) -> Result<ActionResult>;

// M26: Escalation Manager
pub fn determine_tier(&self, confidence: f64, severity: Severity) -> Tier;
pub async fn escalate(&self, action: &Action, tier: Tier) -> Result<()>;

// M27-M30: Actions, Approval, Rollback, Feedback
pub fn register(&mut self, action: ActionDefinition) -> ActionId;
pub async fn request_approval(&self, action: &Action) -> ApprovalRequest;
pub async fn rollback(&self, action: &ExecutedAction) -> Result<()>;
pub fn feedback(&self, outcome: Outcome) -> LearningSignal;
```

### L6: Integration

```rust
// M31: API Gateway
pub async fn route(&self, request: Request) -> Response;

// M32: Event Streaming
pub async fn publish(&self, topic: &str, event: Event) -> Result<()>;
pub fn subscribe(&self, topic: &str) -> Receiver<Event>;

// M33-M36: Bridge, Auth, Rate Limit, Adapters
pub fn bridge(&self, service: ExternalService) -> ServiceClient;
pub async fn authenticate(&self, credentials: Credentials) -> Result<Token>;
pub fn check_limit(&self, key: &str) -> RateLimitResult;
```

---

## Cross-Layer Communication

| From | To | Method | Data |
|------|-----|--------|------|
| L1→L2 | M01→M07 | Direct | ErrorVector |
| L2→L3 | M07→M13 | Event | HealthMetrics |
| L3→L4 | M14→M19 | Direct | PathwayWeight |
| L4→L5 | M19→M25 | Event | ConsensusResult |
| L5→L6 | M25→M31 | Direct | ActionResult |
| L6→L1 | M31→M03 | Direct | RequestLog |
| L1-L6→L7 | M23→M37 | Event (subscribe) | All layer events (PLANNED) |
| L7→Ext | M39→External | Event (publish) | EvolutionCandidate (PLANNED) |

---

*Generated: 2026-01-29 | The Maintenance Engine v1.0.0*
