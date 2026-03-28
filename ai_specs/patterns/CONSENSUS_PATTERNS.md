# PBFT Consensus Patterns Reference

> Practical Byzantine Fault Tolerance Patterns for Claude Code & CodeSynthor V7
> Derived from SYNTHEX, The Maintenance Engine, and CodeSynthor V7

---

## Overview

| Metric | Value |
|--------|-------|
| **Patterns** | 8 |
| **Priority** | P2 |
| **Algorithm** | PBFT (Castro-Liskov) |
| **Agents** | 40 (TME), 60 (CodeSynthor V7) |

---

## Pattern 1: PBFT Configuration (P0)

```rust
/// PBFT consensus parameters
pub mod pbft {
    /// Total number of agents in the network
    pub const N: u32 = 40;

    /// Maximum Byzantine (faulty) agents tolerated: f = (n-1)/3
    pub const F: u32 = 13;

    /// Quorum size required for consensus: q = 2f + 1
    pub const Q: u32 = 27;

    /// View change timeout in milliseconds
    pub const VIEW_CHANGE_TIMEOUT_MS: u64 = 30_000;

    /// Request timeout in milliseconds
    pub const REQUEST_TIMEOUT_MS: u64 = 5_000;

    /// Checkpoint interval (operations between checkpoints)
    pub const CHECKPOINT_INTERVAL: u64 = 100;

    /// Verify PBFT parameters are valid
    pub const fn verify_params() -> bool {
        // n >= 3f + 1 (required for Byzantine fault tolerance)
        N >= 3 * F + 1
            && Q == 2 * F + 1
            && F == (N - 1) / 3
    }

    /// Check if we have quorum
    pub const fn has_quorum(votes: u32) -> bool {
        votes >= Q
    }

    /// Check if we can tolerate more faults
    pub const fn can_tolerate_fault(current_faults: u32) -> bool {
        current_faults < F
    }
}

// Compile-time verification
const _: () = assert!(pbft::verify_params(), "Invalid PBFT parameters");
```

**Why**: PBFT parameters ensure Byzantine fault tolerance with mathematical guarantees.

---

## Pattern 2: Agent Roles (P0)

```rust
/// Agent roles in the NAM-05 framework
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AgentRole {
    /// Validates correctness of proposals
    Validator,
    /// Explores alternative solutions
    Explorer,
    /// Identifies flaws and risks
    Critic,
    /// Assesses cross-system impact
    Integrator,
    /// Matches historical precedents
    Historian,
}

impl AgentRole {
    /// Number of agents per role
    pub const fn count(&self) -> u32 {
        match self {
            Self::Validator => 20,
            Self::Explorer => 8,
            Self::Critic => 6,
            Self::Integrator => 4,
            Self::Historian => 2,
        }
    }

    /// Vote weight for this role
    pub const fn weight(&self) -> f64 {
        match self {
            Self::Validator => 1.0,
            Self::Explorer => 0.8,
            Self::Critic => 1.2,   // Critics have higher weight
            Self::Integrator => 1.0,
            Self::Historian => 0.8,
        }
    }

    /// Primary focus area
    pub const fn focus(&self) -> &'static str {
        match self {
            Self::Validator => "correctness_verification",
            Self::Explorer => "alternative_detection",
            Self::Critic => "flaw_identification",
            Self::Integrator => "cross_system_impact",
            Self::Historian => "precedent_matching",
        }
    }

    /// All roles
    pub const ALL: [AgentRole; 5] = [
        Self::Validator,
        Self::Explorer,
        Self::Critic,
        Self::Integrator,
        Self::Historian,
    ];
}

/// Agent identity
#[derive(Clone, Debug)]
pub struct Agent {
    pub id: String,
    pub role: AgentRole,
    pub tier: u8,
    pub is_primary: bool,
    pub public_key: Vec<u8>,
    pub last_active: DateTime<Utc>,
}

impl Agent {
    /// Create new agent
    pub fn new(id: String, role: AgentRole, tier: u8) -> Self {
        Self {
            id,
            role,
            tier,
            is_primary: false,
            public_key: Vec::new(),
            last_active: Utc::now(),
        }
    }

    /// Calculate weighted vote value
    pub fn vote_weight(&self) -> f64 {
        self.role.weight()
    }
}
```

**Why**: Role-based agents enable specialized consensus participation.

---

## Pattern 3: PBFT State Machine (P1)

```rust
/// PBFT protocol phases
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// Idle, waiting for request
    Idle,
    /// Pre-prepare received, waiting for prepares
    PrePrepare,
    /// Prepare quorum reached, waiting for commits
    Prepare,
    /// Commit quorum reached, executing
    Commit,
    /// Request executed, sending reply
    Reply,
    /// View change in progress
    ViewChange,
}

/// PBFT request tracking
#[derive(Clone, Debug)]
pub struct Request {
    pub id: String,
    pub client_id: String,
    pub operation: Vec<u8>,
    pub timestamp: DateTime<Utc>,
    pub view: u64,
    pub sequence: u64,
}

/// PBFT consensus state
pub struct PbftState {
    /// Current view number
    view: AtomicU64,

    /// Current sequence number
    sequence: AtomicU64,

    /// Local agent ID
    local_id: String,

    /// Is this agent the primary?
    is_primary: AtomicBool,

    /// Pending requests by ID
    pending: DashMap<String, PendingRequest>,

    /// Prepared certificates
    prepared: DashMap<(u64, u64), PreparedCertificate>,

    /// Committed operations
    committed: DashMap<u64, CommittedOperation>,
}

struct PendingRequest {
    request: Request,
    phase: Phase,
    prepares: HashSet<String>,
    commits: HashSet<String>,
    started_at: Instant,
}

struct PreparedCertificate {
    view: u64,
    sequence: u64,
    digest: [u8; 32],
    prepares: Vec<PrepareMessage>,
}

struct CommittedOperation {
    sequence: u64,
    result: Vec<u8>,
    executed_at: DateTime<Utc>,
}

impl PbftState {
    pub fn new(local_id: String) -> Self {
        Self {
            view: AtomicU64::new(0),
            sequence: AtomicU64::new(0),
            local_id,
            is_primary: AtomicBool::new(false),
            pending: DashMap::new(),
            prepared: DashMap::new(),
            committed: DashMap::new(),
        }
    }

    /// Check if this agent is the primary for current view
    pub fn is_primary(&self) -> bool {
        self.is_primary.load(Ordering::SeqCst)
    }

    /// Get current view
    pub fn current_view(&self) -> u64 {
        self.view.load(Ordering::SeqCst)
    }

    /// Advance to next sequence number
    pub fn next_sequence(&self) -> u64 {
        self.sequence.fetch_add(1, Ordering::SeqCst) + 1
    }
}
```

**Why**: Explicit state machine enables correct protocol implementation.

---

## Pattern 4: Message Types (P1)

```rust
use serde::{Deserialize, Serialize};

/// PBFT message types
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum PbftMessage {
    /// Client request
    Request(RequestMessage),

    /// Primary's pre-prepare
    PrePrepare(PrePrepareMessage),

    /// Replica's prepare
    Prepare(PrepareMessage),

    /// Replica's commit
    Commit(CommitMessage),

    /// Reply to client
    Reply(ReplyMessage),

    /// View change request
    ViewChange(ViewChangeMessage),

    /// New view announcement
    NewView(NewViewMessage),

    /// Checkpoint
    Checkpoint(CheckpointMessage),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RequestMessage {
    pub client_id: String,
    pub timestamp: i64,
    pub operation: Vec<u8>,
    pub signature: Vec<u8>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PrePrepareMessage {
    pub view: u64,
    pub sequence: u64,
    pub digest: [u8; 32],
    pub request: RequestMessage,
    pub primary_id: String,
    pub signature: Vec<u8>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PrepareMessage {
    pub view: u64,
    pub sequence: u64,
    pub digest: [u8; 32],
    pub replica_id: String,
    pub signature: Vec<u8>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CommitMessage {
    pub view: u64,
    pub sequence: u64,
    pub digest: [u8; 32],
    pub replica_id: String,
    pub signature: Vec<u8>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReplyMessage {
    pub view: u64,
    pub timestamp: i64,
    pub client_id: String,
    pub replica_id: String,
    pub result: Vec<u8>,
    pub signature: Vec<u8>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ViewChangeMessage {
    pub new_view: u64,
    pub replica_id: String,
    pub checkpoint_sequence: u64,
    pub prepared_certificates: Vec<PreparedCertificate>,
    pub signature: Vec<u8>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NewViewMessage {
    pub view: u64,
    pub primary_id: String,
    pub view_changes: Vec<ViewChangeMessage>,
    pub pre_prepares: Vec<PrePrepareMessage>,
    pub signature: Vec<u8>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CheckpointMessage {
    pub sequence: u64,
    pub state_digest: [u8; 32],
    pub replica_id: String,
    pub signature: Vec<u8>,
}

impl PbftMessage {
    /// Message type identifier
    pub fn type_id(&self) -> &'static str {
        match self {
            Self::Request(_) => "REQUEST",
            Self::PrePrepare(_) => "PRE-PREPARE",
            Self::Prepare(_) => "PREPARE",
            Self::Commit(_) => "COMMIT",
            Self::Reply(_) => "REPLY",
            Self::ViewChange(_) => "VIEW-CHANGE",
            Self::NewView(_) => "NEW-VIEW",
            Self::Checkpoint(_) => "CHECKPOINT",
        }
    }
}
```

**Why**: Strongly typed messages prevent protocol violations.

---

## Pattern 5: Consensus Engine (P1)

```rust
/// PBFT consensus engine
pub struct PbftEngine {
    state: Arc<PbftState>,
    agents: Arc<DashMap<String, Agent>>,
    transport: Arc<dyn Transport>,
    executor: Arc<dyn Executor>,
    metrics: Arc<ConsensusMetrics>,
}

#[async_trait]
pub trait Transport: Send + Sync {
    async fn broadcast(&self, msg: PbftMessage) -> Result<()>;
    async fn send_to(&self, target: &str, msg: PbftMessage) -> Result<()>;
    async fn receive(&self) -> Result<(String, PbftMessage)>;
}

#[async_trait]
pub trait Executor: Send + Sync {
    async fn execute(&self, operation: &[u8]) -> Result<Vec<u8>>;
}

impl PbftEngine {
    pub fn new(
        local_id: String,
        transport: Arc<dyn Transport>,
        executor: Arc<dyn Executor>,
    ) -> Self {
        Self {
            state: Arc::new(PbftState::new(local_id)),
            agents: Arc::new(DashMap::new()),
            transport,
            executor,
            metrics: Arc::new(ConsensusMetrics::default()),
        }
    }

    /// Submit request for consensus
    pub async fn submit(&self, operation: Vec<u8>) -> Result<Vec<u8>> {
        let request = Request {
            id: uuid::Uuid::new_v4().to_string(),
            client_id: self.state.local_id.clone(),
            operation,
            timestamp: Utc::now(),
            view: self.state.current_view(),
            sequence: 0,  // Assigned by primary
        };

        let msg = PbftMessage::Request(RequestMessage {
            client_id: request.client_id.clone(),
            timestamp: request.timestamp.timestamp(),
            operation: request.operation.clone(),
            signature: Vec::new(),  // Sign in production
        });

        // Send to primary
        let primary_id = self.get_primary_id();
        self.transport.send_to(&primary_id, msg).await?;

        // Wait for f+1 matching replies
        self.wait_for_reply(&request.id).await
    }

    /// Process incoming message
    pub async fn process(&self, sender: &str, msg: PbftMessage) -> Result<()> {
        match msg {
            PbftMessage::Request(req) if self.state.is_primary() => {
                self.handle_request(req).await
            }
            PbftMessage::PrePrepare(pp) => {
                self.handle_pre_prepare(sender, pp).await
            }
            PbftMessage::Prepare(p) => {
                self.handle_prepare(sender, p).await
            }
            PbftMessage::Commit(c) => {
                self.handle_commit(sender, c).await
            }
            PbftMessage::ViewChange(vc) => {
                self.handle_view_change(sender, vc).await
            }
            _ => Ok(()),
        }
    }

    /// Handle request as primary
    async fn handle_request(&self, req: RequestMessage) -> Result<()> {
        let seq = self.state.next_sequence();
        let digest = self.compute_digest(&req.operation);

        let pp = PrePrepareMessage {
            view: self.state.current_view(),
            sequence: seq,
            digest,
            request: req,
            primary_id: self.state.local_id.clone(),
            signature: Vec::new(),
        };

        self.transport.broadcast(PbftMessage::PrePrepare(pp)).await
    }

    /// Handle pre-prepare from primary
    async fn handle_pre_prepare(&self, sender: &str, pp: PrePrepareMessage) -> Result<()> {
        // Verify sender is primary
        if sender != self.get_primary_id() {
            return Err(Error::Consensus("Pre-prepare from non-primary".into()));
        }

        // Verify view and sequence
        if pp.view != self.state.current_view() {
            return Err(Error::Consensus("View mismatch".into()));
        }

        // Verify digest
        let expected_digest = self.compute_digest(&pp.request.operation);
        if pp.digest != expected_digest {
            return Err(Error::Consensus("Digest mismatch".into()));
        }

        // Send prepare
        let prepare = PrepareMessage {
            view: pp.view,
            sequence: pp.sequence,
            digest: pp.digest,
            replica_id: self.state.local_id.clone(),
            signature: Vec::new(),
        };

        self.transport.broadcast(PbftMessage::Prepare(prepare)).await
    }

    fn get_primary_id(&self) -> String {
        let view = self.state.current_view();
        let agent_ids: Vec<_> = self.agents.iter().map(|a| a.id.clone()).collect();
        if agent_ids.is_empty() {
            return self.state.local_id.clone();
        }
        agent_ids[(view as usize) % agent_ids.len()].clone()
    }

    fn compute_digest(&self, data: &[u8]) -> [u8; 32] {
        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();
        hasher.update(data);
        hasher.finalize().into()
    }

    async fn wait_for_reply(&self, _request_id: &str) -> Result<Vec<u8>> {
        // Wait for f+1 matching replies
        // Implementation would use channels and timeouts
        Ok(Vec::new())
    }

    async fn handle_prepare(&self, _sender: &str, _p: PrepareMessage) -> Result<()> {
        // Track prepares, broadcast commit when quorum reached
        Ok(())
    }

    async fn handle_commit(&self, _sender: &str, _c: CommitMessage) -> Result<()> {
        // Track commits, execute when quorum reached
        Ok(())
    }

    async fn handle_view_change(&self, _sender: &str, _vc: ViewChangeMessage) -> Result<()> {
        // Handle view change protocol
        Ok(())
    }
}
```

**Why**: Centralized engine manages all PBFT protocol logic.

---

## Pattern 6: Voting (P1)

```rust
/// Voting aggregation for consensus decisions
pub struct VoteAggregator {
    /// Votes by (request_id, vote_value)
    votes: DashMap<String, DashMap<String, Vec<Vote>>>,

    /// Quorum requirement
    quorum: u32,
}

#[derive(Clone, Debug)]
pub struct Vote {
    pub agent_id: String,
    pub role: AgentRole,
    pub value: VoteValue,
    pub weight: f64,
    pub timestamp: DateTime<Utc>,
    pub justification: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum VoteValue {
    Approve,
    Reject,
    Abstain,
}

impl VoteAggregator {
    pub fn new(quorum: u32) -> Self {
        Self {
            votes: DashMap::new(),
            quorum,
        }
    }

    /// Record a vote
    pub fn record(&self, request_id: &str, vote: Vote) {
        let votes = self.votes.entry(request_id.to_string())
            .or_insert_with(DashMap::new);

        votes.entry(vote.value.to_string())
            .or_insert_with(Vec::new)
            .push(vote);
    }

    /// Check if request has reached quorum
    pub fn has_quorum(&self, request_id: &str) -> Option<VoteValue> {
        let votes = self.votes.get(request_id)?;

        // Calculate weighted votes
        let mut approve_weight = 0.0;
        let mut reject_weight = 0.0;

        if let Some(approves) = votes.get("Approve") {
            approve_weight = approves.iter().map(|v| v.weight).sum();
        }

        if let Some(rejects) = votes.get("Reject") {
            reject_weight = rejects.iter().map(|v| v.weight).sum();
        }

        // Check for weighted quorum
        if approve_weight >= self.quorum as f64 {
            return Some(VoteValue::Approve);
        }

        if reject_weight >= self.quorum as f64 {
            return Some(VoteValue::Reject);
        }

        None
    }

    /// Get vote breakdown for a request
    pub fn breakdown(&self, request_id: &str) -> VoteBreakdown {
        let mut breakdown = VoteBreakdown::default();

        if let Some(votes) = self.votes.get(request_id) {
            for entry in votes.iter() {
                let count = entry.value().len();
                let weight: f64 = entry.value().iter().map(|v| v.weight).sum();

                match entry.key().as_str() {
                    "Approve" => {
                        breakdown.approve_count = count;
                        breakdown.approve_weight = weight;
                    }
                    "Reject" => {
                        breakdown.reject_count = count;
                        breakdown.reject_weight = weight;
                    }
                    "Abstain" => {
                        breakdown.abstain_count = count;
                    }
                    _ => {}
                }
            }
        }

        breakdown.total = breakdown.approve_count + breakdown.reject_count + breakdown.abstain_count;
        breakdown
    }

    /// Clear votes for completed request
    pub fn clear(&self, request_id: &str) {
        self.votes.remove(request_id);
    }
}

#[derive(Clone, Debug, Default)]
pub struct VoteBreakdown {
    pub total: usize,
    pub approve_count: usize,
    pub approve_weight: f64,
    pub reject_count: usize,
    pub reject_weight: f64,
    pub abstain_count: usize,
}

impl VoteValue {
    fn to_string(&self) -> String {
        match self {
            Self::Approve => "Approve".to_string(),
            Self::Reject => "Reject".to_string(),
            Self::Abstain => "Abstain".to_string(),
        }
    }
}
```

**Why**: Weighted voting enables role-based influence in consensus.

---

## Pattern 7: Human as Agent (P2)

```rust
/// Human agent integration (NAM R5)
pub struct HumanAgent {
    /// Agent identifier
    pub id: String,

    /// Human is always tier 0
    pub tier: u8,

    /// Vote weight
    pub weight: f64,

    /// Participation mode
    pub participation: ParticipationMode,

    /// Capabilities
    pub capabilities: HashSet<Capability>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParticipationMode {
    /// Human participates as peer with agents
    Peer,
    /// Human is only notified, doesn't vote
    Observer,
    /// Human can override consensus
    Override,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Capability {
    /// Can vote in consensus
    ConsensusVote,
    /// Can register dissent
    Dissent,
    /// Can override agent decisions
    Override,
    /// Can respond to escalations
    EscalationResponse,
    /// Can trigger emergency actions
    Emergency,
}

impl Default for HumanAgent {
    fn default() -> Self {
        Self {
            id: "@0.A".to_string(),  // NAM identifier
            tier: 0,                  // Foundation tier
            weight: 1.0,              // Standard weight
            participation: ParticipationMode::Peer,
            capabilities: [
                Capability::ConsensusVote,
                Capability::Dissent,
                Capability::Override,
                Capability::EscalationResponse,
            ].into_iter().collect(),
        }
    }
}

impl HumanAgent {
    /// Human has override capability
    pub fn can_override(&self) -> bool {
        self.capabilities.contains(&Capability::Override)
    }

    /// Human can vote
    pub fn can_vote(&self) -> bool {
        self.capabilities.contains(&Capability::ConsensusVote) &&
        self.participation != ParticipationMode::Observer
    }

    /// Create human vote
    pub fn vote(&self, value: VoteValue, justification: String) -> Vote {
        Vote {
            agent_id: self.id.clone(),
            role: AgentRole::Validator,  // Human treated as validator
            value,
            weight: self.weight,
            timestamp: Utc::now(),
            justification: Some(justification),
        }
    }
}

/// Integration point for human participation
pub struct HumanInterface {
    human: HumanAgent,
    pending_decisions: Arc<DashMap<String, PendingDecision>>,
    notification_tx: tokio::sync::mpsc::Sender<Notification>,
}

#[derive(Clone, Debug)]
pub struct PendingDecision {
    pub id: String,
    pub description: String,
    pub options: Vec<String>,
    pub timeout: DateTime<Utc>,
    pub escalation_tier: u8,
}

#[derive(Clone, Debug)]
pub struct Notification {
    pub id: String,
    pub message: String,
    pub severity: Severity,
    pub requires_response: bool,
}

#[derive(Clone, Copy, Debug)]
pub enum Severity {
    Info,
    Warning,
    Critical,
}

impl HumanInterface {
    /// Request human decision
    pub async fn request_decision(&self, decision: PendingDecision) -> Result<Option<VoteValue>> {
        let id = decision.id.clone();
        self.pending_decisions.insert(id.clone(), decision.clone());

        // Send notification
        self.notification_tx.send(Notification {
            id: id.clone(),
            message: decision.description.clone(),
            severity: match decision.escalation_tier {
                0..=1 => Severity::Info,
                2 => Severity::Warning,
                _ => Severity::Critical,
            },
            requires_response: true,
        }).await?;

        // Wait for response or timeout
        // In production, would use channels/callbacks
        Ok(None)
    }

    /// Record human response
    pub fn respond(&self, decision_id: &str, value: VoteValue, justification: String) -> Result<Vote> {
        if !self.pending_decisions.contains_key(decision_id) {
            return Err(Error::NotFound(format!("Decision {decision_id} not found")));
        }

        self.pending_decisions.remove(decision_id);
        Ok(self.human.vote(value, justification))
    }
}
```

**Why**: Human-as-agent (NAM R5) ensures human oversight in critical decisions.

---

## Pattern 8: Consensus Metrics (P2)

```rust
use std::sync::atomic::{AtomicU64, Ordering};

/// Metrics for consensus system monitoring
#[derive(Default)]
pub struct ConsensusMetrics {
    /// Total requests processed
    pub requests_total: AtomicU64,

    /// Successful consensus rounds
    pub consensus_success: AtomicU64,

    /// Failed consensus (no quorum)
    pub consensus_failed: AtomicU64,

    /// View changes executed
    pub view_changes: AtomicU64,

    /// Average consensus latency (ms)
    pub avg_latency_ms: AtomicU64,

    /// Active agents
    pub active_agents: AtomicU64,

    /// Byzantine faults detected
    pub byzantine_faults: AtomicU64,
}

impl ConsensusMetrics {
    pub fn record_request(&self) {
        self.requests_total.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_success(&self, latency_ms: u64) {
        self.consensus_success.fetch_add(1, Ordering::Relaxed);

        // Update running average
        let current = self.avg_latency_ms.load(Ordering::Relaxed);
        let count = self.consensus_success.load(Ordering::Relaxed);
        let new_avg = (current * (count - 1) + latency_ms) / count;
        self.avg_latency_ms.store(new_avg, Ordering::Relaxed);
    }

    pub fn record_failure(&self) {
        self.consensus_failed.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_view_change(&self) {
        self.view_changes.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_byzantine_fault(&self) {
        self.byzantine_faults.fetch_add(1, Ordering::Relaxed);
    }

    pub fn snapshot(&self) -> MetricsSnapshot {
        MetricsSnapshot {
            requests_total: self.requests_total.load(Ordering::Relaxed),
            consensus_success: self.consensus_success.load(Ordering::Relaxed),
            consensus_failed: self.consensus_failed.load(Ordering::Relaxed),
            view_changes: self.view_changes.load(Ordering::Relaxed),
            avg_latency_ms: self.avg_latency_ms.load(Ordering::Relaxed),
            active_agents: self.active_agents.load(Ordering::Relaxed),
            byzantine_faults: self.byzantine_faults.load(Ordering::Relaxed),
            success_rate: self.success_rate(),
        }
    }

    fn success_rate(&self) -> f64 {
        let success = self.consensus_success.load(Ordering::Relaxed);
        let total = self.requests_total.load(Ordering::Relaxed);
        if total == 0 {
            return 1.0;
        }
        success as f64 / total as f64
    }
}

#[derive(Clone, Debug)]
pub struct MetricsSnapshot {
    pub requests_total: u64,
    pub consensus_success: u64,
    pub consensus_failed: u64,
    pub view_changes: u64,
    pub avg_latency_ms: u64,
    pub active_agents: u64,
    pub byzantine_faults: u64,
    pub success_rate: f64,
}
```

**Why**: Metrics enable monitoring and alerting on consensus health.

---

## PBFT Protocol Flow

```
┌─────────┐     ┌─────────┐     ┌─────────┐     ┌─────────┐
│ Client  │     │ Primary │     │Replica 1│     │Replica 2│
└────┬────┘     └────┬────┘     └────┬────┘     └────┬────┘
     │               │               │               │
     │──Request─────>│               │               │
     │               │               │               │
     │               │─Pre-Prepare──>│               │
     │               │─Pre-Prepare──────────────────>│
     │               │               │               │
     │               │<──Prepare─────│               │
     │               │<──Prepare─────────────────────│
     │               │───Prepare────>│               │
     │               │───Prepare────────────────────>│
     │               │               │               │
     │               │<───Commit─────│               │
     │               │<───Commit─────────────────────│
     │               │────Commit────>│               │
     │               │────Commit────────────────────>│
     │               │               │               │
     │<──Reply───────│               │               │
     │<──Reply───────────────────────│               │
     │<──Reply───────────────────────────────────────│
     │               │               │               │
```

---

## Comparison: TME vs CodeSynthor V7

| Parameter | TME | CodeSynthor V7 |
|-----------|-----|----------------|
| Total agents (n) | 40 | 60 |
| Byzantine tolerance (f) | 13 | 19 |
| Quorum (q) | 27 | 41 |
| View change timeout | 30s | 60s |
| Checkpoint interval | 100 | 200 |

---

*Generated: 2026-01-28 | The Maintenance Engine v1.0.0*
