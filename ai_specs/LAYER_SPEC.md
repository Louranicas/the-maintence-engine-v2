# Layer Specification

## Layer Overview

| Layer | ID | Name | Modules | Primary Function |
|-------|-------|------|---------|------------------|
| L1 | 0x01 | Foundation | M1.1-M1.6 | Core infrastructure |
| L2 | 0x02 | Processing | M2.1-M2.6 | Data transformation |
| L3 | 0x03 | Integration | M3.1-M3.6 | External connectivity |
| L4 | 0x04 | Intelligence | M4.1-M4.6 | NAM/ANAM processing |
| L5 | 0x05 | Consensus | M5.1-M5.6 | PBFT coordination |
| L6 | 0x06 | Orchestration | M6.1-M6.6 | System management |
| L7 | 0x07 | Observer | M7.1-M7.3 | Cross-cutting observation (PLANNED) |

---

## L1: Foundation Layer

### Modules

| ID | Module | Purpose |
|----|--------|---------|
| M1.1 | Config | Configuration management |
| M1.2 | Logging | Structured logging |
| M1.3 | Metrics | Telemetry collection |
| M1.4 | Storage | Persistence abstraction |
| M1.5 | Crypto | Cryptographic primitives |
| M1.6 | Network | Transport layer |

### Key APIs

```rust
// M1.1 Config
fn load_config(path: &Path) -> Result<Config>;
fn get<T: DeserializeOwned>(key: &str) -> Option<T>;
fn watch(key: &str) -> Receiver<ConfigChange>;

// M1.2 Logging
fn init_tracing(config: &LogConfig) -> Result<Guard>;
fn span(name: &str) -> Span;

// M1.3 Metrics
fn counter(name: &str, labels: &[(&str, &str)]) -> Counter;
fn histogram(name: &str) -> Histogram;
fn gauge(name: &str) -> Gauge;

// M1.4 Storage
async fn get(key: &[u8]) -> Result<Option<Vec<u8>>>;
async fn put(key: &[u8], value: &[u8]) -> Result<()>;
async fn delete(key: &[u8]) -> Result<()>;

// M1.5 Crypto
fn hash(data: &[u8]) -> Hash256;
fn sign(key: &PrivateKey, data: &[u8]) -> Signature;
fn verify(key: &PublicKey, data: &[u8], sig: &Signature) -> bool;

// M1.6 Network
async fn connect(addr: SocketAddr) -> Result<Connection>;
async fn listen(addr: SocketAddr) -> Result<Listener>;
```

### Dependencies
- External: tokio, serde, tracing, ring
- Internal: None (base layer)

### Configuration

| Parameter | Type | Default |
|-----------|------|---------|
| config.reload_interval | Duration | 30s |
| log.level | String | "info" |
| metrics.endpoint | String | "0.0.0.0:9090" |
| storage.backend | Enum | "rocksdb" |

---

## L2: Processing Layer

### Modules

| ID | Module | Purpose |
|----|--------|---------|
| M2.1 | Parser | Input parsing |
| M2.2 | Validator | Schema validation |
| M2.3 | Transformer | Data transformation |
| M2.4 | Encoder | 12D tensor encoding |
| M2.5 | Pipeline | Flow orchestration |
| M2.6 | Cache | Hot data caching |

### Key APIs

```rust
// M2.1 Parser
fn parse<T: FromStr>(input: &str) -> Result<T>;
fn parse_json<T: DeserializeOwned>(input: &[u8]) -> Result<T>;

// M2.2 Validator
fn validate<T: Validate>(value: &T) -> Result<()>;
fn schema(name: &str) -> &Schema;

// M2.3 Transformer
fn transform<F, T>(input: F) -> Result<T> where F: Into<T>;
fn map<F>(items: Vec<F>, f: impl Fn(F) -> F) -> Vec<F>;

// M2.4 Encoder
fn encode_12d(data: &[f64]) -> Tensor12D;
fn decode_12d(tensor: &Tensor12D) -> Vec<f64>;
fn normalize(tensor: &mut Tensor12D);

// M2.5 Pipeline
async fn execute(pipeline: &Pipeline, input: Input) -> Result<Output>;
fn register(name: &str, stage: Stage);

// M2.6 Cache
async fn get_cached<T>(key: &str) -> Option<T>;
async fn set_cached<T>(key: &str, value: T, ttl: Duration);
fn invalidate(pattern: &str);
```

### Tensor Encoding (12D)

| Dimension | Range | Purpose |
|-----------|-------|---------|
| D0 | [0, 1] | Confidence |
| D1 | [0, 1] | Complexity |
| D2 | [0, 1] | Priority |
| D3 | [0, 1] | Urgency |
| D4 | [0, 1] | Impact |
| D5 | [0, 1] | Risk |
| D6 | [-1, 1] | Sentiment |
| D7 | [0, 1] | Relevance |
| D8 | [0, 1] | Novelty |
| D9 | [0, 1] | Coherence |
| D10 | [0, 1] | Completeness |
| D11 | [0, 1] | Actionability |

### Dependencies
- External: serde_json, jsonschema
- Internal: L1 (Storage, Config)

---

## L3: Integration Layer

### Modules

| ID | Module | Purpose |
|----|--------|---------|
| M3.1 | REST | HTTP API server |
| M3.2 | gRPC | RPC server |
| M3.3 | WebSocket | Real-time streaming |
| M3.4 | Queue | Message queuing |
| M3.5 | Database | DB connections |
| M3.6 | External | Third-party APIs |

### Key APIs

```rust
// M3.1 REST
fn router() -> Router;
async fn serve(addr: SocketAddr, router: Router) -> Result<()>;

// M3.2 gRPC
fn service<T: Service>() -> T;
async fn serve_grpc(addr: SocketAddr) -> Result<()>;

// M3.3 WebSocket
async fn accept(stream: TcpStream) -> Result<WebSocket>;
async fn broadcast(msg: Message);
fn subscribe(topic: &str) -> Receiver<Message>;

// M3.4 Queue
async fn publish(topic: &str, msg: &[u8]) -> Result<()>;
async fn subscribe(topic: &str) -> Result<Consumer>;
async fn ack(delivery: Delivery) -> Result<()>;

// M3.5 Database
async fn pool(config: &DbConfig) -> Result<Pool>;
async fn query<T>(sql: &str, params: &[&dyn ToSql]) -> Result<Vec<T>>;
async fn execute(sql: &str) -> Result<u64>;

// M3.6 External
async fn request(method: Method, url: &str) -> Result<Response>;
fn client(base_url: &str) -> HttpClient;
```

### Database Connections (9 DBs)

| DB | Type | Port | Purpose |
|----|------|------|---------|
| PostgreSQL | RDBMS | 5432 | Primary data |
| Redis | KV | 6379 | Cache/sessions |
| ClickHouse | OLAP | 8123 | Analytics |
| TimescaleDB | Time-series | 5433 | Metrics |
| Elasticsearch | Search | 9200 | Full-text search |
| Neo4j | Graph | 7687 | Relationships |
| MongoDB | Document | 27017 | Unstructured |
| Milvus | Vector | 19530 | Embeddings |
| RocksDB | Embedded | - | Local storage |

### Dependencies
- External: axum, tonic, sqlx, redis
- Internal: L1 (Network, Config), L2 (Parser, Validator)

---

## L4: Intelligence Layer

### Modules

| ID | Module | Purpose |
|----|--------|---------|
| M4.1 | NAM | Neural Associative Memory |
| M4.2 | ANAM | Adaptive NAM |
| M4.3 | STDP | Learning algorithm |
| M4.4 | Inference | Model inference |
| M4.5 | Embeddings | Vector generation |
| M4.6 | Escalation | Tier management |

### Key APIs

```rust
// M4.1 NAM
fn recall(pattern: &Tensor12D) -> Vec<Association>;
fn store(key: &Tensor12D, value: &Tensor12D);
fn similarity(a: &Tensor12D, b: &Tensor12D) -> f64;

// M4.2 ANAM
fn adapt(feedback: &Feedback) -> Result<()>;
fn evolve(generation: u32) -> Genome;
fn fitness(individual: &Individual) -> f64;

// M4.3 STDP
fn update_weight(pre: f64, post: f64, dt: f64) -> f64;
fn apply_stdp(network: &mut Network, spikes: &[Spike]);
fn decay_weights(network: &mut Network, rate: f64);

// M4.4 Inference
async fn infer(model: &str, input: &Tensor) -> Result<Tensor>;
fn load_model(path: &Path) -> Result<Model>;
fn quantize(model: &Model, bits: u8) -> Model;

// M4.5 Embeddings
async fn embed(text: &str) -> Result<Vec<f64>>;
async fn batch_embed(texts: &[&str]) -> Result<Vec<Vec<f64>>>;
fn cosine_similarity(a: &[f64], b: &[f64]) -> f64;

// M4.6 Escalation
fn evaluate_complexity(input: &Input) -> f64;
fn select_tier(complexity: f64) -> ModelTier;
async fn escalate(request: Request, tier: ModelTier) -> Result<Response>;
```

### STDP Configuration

| Parameter | Value | Description |
|-----------|-------|-------------|
| tau_plus | 20ms | LTP time constant |
| tau_minus | 20ms | LTD time constant |
| A_plus | 0.1 | LTP amplitude |
| A_minus | 0.05 | LTD amplitude |
| w_max | 1.0 | Maximum weight |
| w_min | 0.0 | Minimum weight |

### Dependencies
- External: candle, tokenizers
- Internal: L2 (Encoder, Pipeline), L3 (Database)

---

## L5: Consensus Layer

### Modules

| ID | Module | Purpose |
|----|--------|---------|
| M5.1 | PBFT | Byzantine consensus |
| M5.2 | View | View management |
| M5.3 | Checkpoint | State snapshots |
| M5.4 | Recovery | Fault recovery |
| M5.5 | Membership | Node management |
| M5.6 | Crypto | Consensus crypto |

### Key APIs

```rust
// M5.1 PBFT
async fn propose(request: Request) -> Result<ConsensusResult>;
async fn pre_prepare(msg: PrePrepare) -> Result<()>;
async fn prepare(msg: Prepare) -> Result<()>;
async fn commit(msg: Commit) -> Result<()>;

// M5.2 View
fn current_view() -> ViewNumber;
fn primary(view: ViewNumber) -> NodeId;
async fn view_change(new_view: ViewNumber) -> Result<()>;

// M5.3 Checkpoint
async fn create_checkpoint(seq: SeqNumber) -> Result<Checkpoint>;
fn stable_checkpoint() -> &Checkpoint;
fn garbage_collect(checkpoint: &Checkpoint);

// M5.4 Recovery
async fn recover(node: NodeId) -> Result<()>;
async fn sync_state(from: NodeId) -> Result<()>;
fn validate_state(state: &State) -> bool;

// M5.5 Membership
fn nodes() -> &[NodeId];
fn add_node(node: NodeId) -> Result<()>;
fn remove_node(node: NodeId) -> Result<()>;
fn is_primary() -> bool;

// M5.6 Crypto
fn threshold_sign(msg: &[u8], share: &KeyShare) -> PartialSig;
fn combine_sigs(partials: &[PartialSig]) -> Result<Signature>;
fn verify_threshold(msg: &[u8], sig: &Signature) -> bool;
```

### PBFT Parameters

| Parameter | Value | Formula |
|-----------|-------|---------|
| n | 40 | Total nodes |
| f | 13 | floor((n-1)/3) |
| q | 27 | 2f + 1 |
| timeout_base | 5s | Initial view timeout |
| timeout_mult | 2x | Exponential backoff |

### Dependencies
- External: ed25519-dalek, threshold-crypto
- Internal: L1 (Network, Crypto), L4 (Inference)

---

## L6: Orchestration Layer

### Modules

| ID | Module | Purpose |
|----|--------|---------|
| M6.1 | Scheduler | Task scheduling |
| M6.2 | Workflow | Multi-step flows |
| M6.3 | Monitor | Health monitoring |
| M6.4 | Autoscale | Resource scaling |
| M6.5 | Deploy | Deployment management |
| M6.6 | Admin | Administrative ops |

### Key APIs

```rust
// M6.1 Scheduler
async fn schedule(task: Task, when: Schedule) -> Result<JobId>;
fn cancel(job_id: JobId) -> Result<()>;
fn list_jobs() -> Vec<Job>;

// M6.2 Workflow
async fn execute_workflow(wf: Workflow) -> Result<WorkflowResult>;
fn define_workflow(steps: Vec<Step>) -> Workflow;
fn retry_step(wf_id: WorkflowId, step: usize) -> Result<()>;

// M6.3 Monitor
fn health_check() -> HealthStatus;
fn metrics() -> MetricsSnapshot;
fn alerts() -> Vec<Alert>;

// M6.4 Autoscale
fn current_scale() -> ScaleState;
async fn scale_to(replicas: u32) -> Result<()>;
fn set_policy(policy: ScalePolicy);

// M6.5 Deploy
async fn deploy(artifact: &Artifact, env: Environment) -> Result<Deployment>;
async fn rollback(deployment: DeploymentId) -> Result<()>;
fn status(deployment: DeploymentId) -> DeploymentStatus;

// M6.6 Admin
fn system_status() -> SystemStatus;
async fn shutdown(graceful: bool) -> Result<()>;
fn audit_log() -> Vec<AuditEntry>;
```

### Dependencies
- External: cron, k8s-openapi
- Internal: All lower layers

---

## L7: Observer Layer (PLANNED)

> **Status: PLANNED** -- Specifications complete, implementation not yet started.
> Detailed specs: [evolution_chamber_ai_specs/INDEX.md](evolution_chamber_ai_specs/INDEX.md)
> Detailed docs: [../ai_docs/evolution_chamber_ai_docs/INDEX.md](../ai_docs/evolution_chamber_ai_docs/INDEX.md)

### Modules

| ID | Module | Purpose |
|----|--------|---------|
| M7.1 | Log Correlator | Cross-layer event correlation (~1,400 LOC) |
| M7.2 | Emergence Detector | Emergent behavior detection (~1,500 LOC) |
| M7.3 | Evolution Chamber | RALPH loop meta-learning and evolution (~1,800 LOC) |

### Utilities

| Utility | Purpose | Est. LOC |
|---------|---------|----------|
| Observer Bus | Internal L7 pub/sub connecting M7.1/M7.2/M7.3 | ~500 |
| Fitness Evaluator | 12D tensor fitness scoring for evolution candidates | ~800 |

### Key APIs

```rust
// M7.1 Log Correlator
fn correlate(events: &[LayerEvent]) -> Vec<Correlation>;
fn build_timeline(window: Duration) -> Timeline;
fn cross_layer_pattern(layers: &[LayerId]) -> Option<Pattern>;

// M7.2 Emergence Detector
fn detect_cascade(events: &[CorrelatedEvent]) -> Option<Cascade>;
fn synergy_delta(before: &Tensor12D, after: &Tensor12D) -> f64;
fn detect_resonance(history: &[Observation]) -> Option<ResonanceCycle>;

// M7.3 Evolution Chamber (RALPH Loop)
fn recognize(observations: &[Observation]) -> Vec<Pattern>;
fn analyze(patterns: &[Pattern], depth: usize) -> Analysis;
fn learn(analysis: &Analysis, rate: f64) -> MetaLearning;
fn plan(learning: &MetaLearning) -> Vec<EvolutionCandidate>;
fn harmonize(candidate: &EvolutionCandidate) -> Result<EvolutionResult>;
```

### RALPH Loop Parameters

| Parameter | Value | Description |
|-----------|-------|-------------|
| recognize_interval | 30s | Pattern recognition cycle |
| analyze_depth | 3 | Cross-layer analysis depth |
| learn_rate | 0.05 | Meta-learning rate |
| propose_threshold | 0.75 | Minimum fitness to propose |
| horizon | 100 | Rolling observation window |

### EventBus Channels

| Direction | Channel | Event Types |
|-----------|---------|-------------|
| Subscribe | `health.*`, `remediation.*`, `learning.*`, `consensus.*`, `integration.*`, `metrics.*` | All L1-L6 events (read-only) |
| Publish | `observation.*` | CorrelationFound, CrossLayerPattern, TimelineBuilt |
| Publish | `emergence.*` | EmergenceBehaviorDetected, SystemPhaseShift, AttractorFound |
| Publish | `evolution.*` | CandidateProposed, FitnessScored, EvolutionApplied |

### Integration

| Property | Value |
|----------|-------|
| Mode | Optional (`observer: Option<ObserverLayer>`) |
| Input | EventBus (M23) subscriptions -- 6 channels |
| Output | 3 new EventBus channels |
| Concurrency | `parking_lot::RwLock` |
| Database | observer_state.db |
| Estimated LOC | ~6,600 total |
| Estimated Tests | ~300 |

### Dependencies
- External: parking_lot
- Internal: L4 (M23 EventBus), L1-L6 (event subscriptions, read-only)

---

## Inter-Layer Communication

```
L7 ··· observes (read-only) ···················> L1-L6 (PLANNED)
   observer

L6 ──────────────────────────────────────────┐
 │ orchestrate                               │
L5 ←─────────────────────────────────────────┤
 │ consensus                                 │
L4 ←─────────────────────────────────────────┤
 │ intelligence                              │
L3 ←─────────────────────────────────────────┤
 │ integration                               │
L2 ←─────────────────────────────────────────┤
 │ processing                                │
L1 ←─────────────────────────────────────────┘
   foundation
```

### Message Flow

| Direction | Protocol | Format |
|-----------|----------|--------|
| Up (L1→L6) | Events | Protobuf |
| Down (L6→L1) | Commands | Protobuf |
| Lateral | Channels | Bincode |

### Latency Budgets

| Layer | Budget | P99 Target |
|-------|--------|------------|
| L1 | 5ms | 10ms |
| L2 | 10ms | 20ms |
| L3 | 20ms | 40ms |
| L4 | 30ms | 60ms |
| L5 | 25ms | 50ms |
| L6 | 10ms | 20ms |
| L7 (PLANNED) | 15ms | 30ms |
| **Total** | **115ms** | **230ms** |
