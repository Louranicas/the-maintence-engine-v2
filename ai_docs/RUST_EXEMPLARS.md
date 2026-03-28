# Rust Exemplars — Concrete Code for L3-L8 Implementation

> Production-tested patterns extracted from ME v1 (56K LOC, 2,327 tests, 0 clippy warnings)
> Each exemplar is a self-contained, copy-adaptable code block

---

## E1: Confidence Calculation (L3 — M15)

From ME v1 `m3_core_logic/mod.rs`:

```rust
/// Weighted confidence score using FMA for precision.
///
/// Formula: 0.3*hist + 0.25*pattern + 0.2*severity + 0.15*pathway + 0.1*time
pub fn calculate_confidence(
    historical_success_rate: f64,
    pattern_match_strength: f64,
    severity_score: f64,
    pathway_weight: f64,
    time_factor: f64,
) -> f64 {
    let confidence = 0.3f64.mul_add(
        historical_success_rate,
        0.25f64.mul_add(
            pattern_match_strength,
            0.2f64.mul_add(
                severity_score,
                0.15f64.mul_add(pathway_weight, 0.1 * time_factor),
            ),
        ),
    );
    confidence.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_confidence_all_ones() {
        let c = calculate_confidence(1.0, 1.0, 1.0, 1.0, 1.0);
        assert!((c - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_confidence_all_zeros() {
        let c = calculate_confidence(0.0, 0.0, 0.0, 0.0, 0.0);
        assert!((c - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_confidence_clamped_to_unit() {
        let c = calculate_confidence(2.0, 2.0, 2.0, 2.0, 2.0);
        assert!((0.0..=1.0).contains(&c));
    }
}
```

---

## E2: Escalation Tier Logic (L3 — M14)

From ME v1 `m3_core_logic/remediation.rs`:

```rust
pub fn determine_tier(
    confidence: f64,
    severity: Severity,
    action: &RemediationAction,
) -> EscalationTier {
    // L3: PBFT consensus for critical/destructive actions
    if matches!(
        action,
        RemediationAction::ServiceRestart { graceful: false, .. }
            | RemediationAction::DatabaseVacuum { .. }
            | RemediationAction::ServiceMigration { .. }
    ) {
        return EscalationTier::L3PbftConsensus;
    }

    // L0: Auto-execute for high confidence + low severity
    if confidence >= 0.9 && severity <= Severity::Medium {
        return EscalationTier::L0AutoExecute;
    }

    // L1: Notify human for moderate confidence
    if confidence >= 0.7 && severity <= Severity::High {
        return EscalationTier::L1NotifyHuman;
    }

    // L2: Require approval for everything else
    EscalationTier::L2RequireApproval
}
```

---

## E3: PBFT Phase Machine (L6 — M31)

From ME v1 `m6_consensus/pbft.rs`:

```rust
pub fn advance_phase(&self, proposal_id: &str) -> Result<ConsensusPhase> {
    let mut proposals = self.proposals.write();
    let proposal = proposals.get_mut(proposal_id)
        .ok_or_else(|| Error::Validation(format!("Proposal '{proposal_id}' not found")))?;

    let next_phase = match proposal.phase {
        ConsensusPhase::PrePrepare => ConsensusPhase::Prepare,
        ConsensusPhase::Prepare => {
            let votes = self.votes.read();
            let approve_count = votes.get(proposal_id)
                .map_or(0, |v| v.iter().filter(|v| v.approve).count() as u32);
            if approve_count < PBFT_Q {
                return Err(Error::ConsensusQuorum {
                    required: PBFT_Q,
                    received: approve_count,
                });
            }
            ConsensusPhase::Commit
        }
        ConsensusPhase::Commit => ConsensusPhase::Execute,
        ConsensusPhase::Execute => ConsensusPhase::Complete,
        ConsensusPhase::Complete | ConsensusPhase::Failed => {
            return Err(Error::Validation("Cannot advance terminal phase".into()));
        }
    };

    proposal.phase = next_phase;
    drop(proposals);

    Ok(next_phase)
}
```

---

## E4: Circuit Breaker FSM (L2 — M12)

From ME v2 `m2_services/resilience.rs`:

```rust
fn record_failure(&self, service_id: &str) -> Result<()> {
    let mut breakers = self.breakers.write();
    let entry = breakers.get_mut(service_id)
        .ok_or_else(|| Error::ServiceNotFound(service_id.to_owned()))?;

    match entry.state {
        CircuitState::Closed => {
            entry.failure_count += 1;
            if entry.failure_count >= entry.config.failure_threshold {
                entry.state = CircuitState::Open;
                entry.last_state_change = Timestamp::now();
                entry.state_change_instant = Instant::now();
                drop(breakers);
                self.emit_degradation_signal(service_id);
                return Ok(());
            }
        }
        CircuitState::Open => {
            if entry.state_change_instant.elapsed() >= entry.config.open_timeout {
                entry.state = CircuitState::HalfOpen;
                entry.success_count = 0;
                entry.failure_count = 0;
                entry.last_state_change = Timestamp::now();
                entry.state_change_instant = Instant::now();
            }
        }
        CircuitState::HalfOpen => {
            entry.state = CircuitState::Open;  // Any failure reopens
            entry.success_count = 0;
            entry.failure_count = 0;
            entry.last_state_change = Timestamp::now();
            entry.state_change_instant = Instant::now();
            drop(breakers);
            self.emit_degradation_signal(service_id);
        }
    }
    Ok(())
}
```

---

## E5: Hebbian Manager Interior Mutability (L5 — M25)

From ME v1 `m5_learning/hebbian.rs`:

```rust
pub struct HebbianManager {
    pathways: RwLock<HashMap<String, HebbianPathway>>,
    pulse_history: RwLock<Vec<HebbianPulse>>,
    pathway_metrics: RwLock<HashMap<String, PathwayMetrics>>,
    pulse_counter: RwLock<u64>,
}

impl HebbianManager {
    pub fn strengthen(&self, key: &str) -> Result<f64> {
        let mut guard = self.pathways.write();
        let pathway = guard.get_mut(key)
            .ok_or_else(|| Error::PathwayNotFound {
                source: key.to_owned(),
                target: String::new(),
            })?;
        pathway.apply_ltp(&self.config);
        let new_strength = pathway.strength;
        drop(guard);  // Release pathway lock

        // Update metrics without holding pathway lock
        if let Some(metrics) = self.pathway_metrics.write().get_mut(key) {
            metrics.record_activation(new_strength);
        }

        Ok(new_strength)
    }
}
```

---

## E6: Health Probe Threshold FSM (L2 — M10)

From ME v2 `m2_services/health_monitor.rs`:

```rust
fn record_result(&self, service_id: &str, result: HealthCheckResult) -> Result<()> {
    let mut state = self.state.write();
    let svc = state.services.get_mut(service_id)
        .ok_or_else(|| Error::ServiceNotFound(service_id.to_owned()))?;

    let previous_status = svc.current_status;

    if result.is_success() {
        svc.consecutive_successes += 1;
        svc.consecutive_failures = 0;
    } else {
        svc.consecutive_failures += 1;
        svc.consecutive_successes = 0;
    }

    if svc.consecutive_successes >= svc.probe.healthy_threshold {
        svc.current_status = HealthStatus::Healthy;
    } else if svc.consecutive_failures >= svc.probe.unhealthy_threshold {
        svc.current_status = HealthStatus::Unhealthy;
    } else if svc.consecutive_failures > 0
        && svc.current_status == HealthStatus::Healthy
    {
        svc.current_status = HealthStatus::Degraded;
    }

    // History trimming (bounded collection)
    svc.history.push(result);
    if svc.history.len() > MAX_HISTORY {
        svc.history.drain(..svc.history.len() - MAX_HISTORY);
    }

    let new_status = svc.current_status;
    drop(state);

    if previous_status != new_status {
        self.emit_health_transition(service_id, previous_status, new_status);
    }
    Ok(())
}
```

---

## E7: TensorContributor (L2 — M10)

From ME v2 `m2_services/health_monitor.rs`:

```rust
impl TensorContributor for HealthMonitor {
    #[allow(clippy::cast_precision_loss)]
    fn contribute(&self) -> ContributedTensor {
        let state = self.state.read();
        let total = state.services.len();

        let health_score = if total > 0 {
            state.services.values()
                .map(|s| s.current_status.score())
                .sum::<f64>()
                / total as f64
        } else {
            1.0
        };

        let error_rate = if total > 0 {
            state.services.values()
                .filter(|s| s.current_status == HealthStatus::Unhealthy)
                .count() as f64
                / total as f64
        } else {
            0.0
        };

        drop(state);

        let tensor = Tensor12D::new([
            0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
            health_score,  // D6
            0.0, 0.0, 0.0,
            error_rate,    // D10
            0.0,
        ]);

        let coverage = CoverageBitmap::EMPTY
            .with_dimension(DimensionIndex::HealthScore)
            .with_dimension(DimensionIndex::ErrorRate);

        ContributedTensor::new(tensor, coverage, ContributorKind::Stream)
    }

    fn contributor_kind(&self) -> ContributorKind { ContributorKind::Stream }
    fn module_id(&self) -> &str { ModuleId::M10.as_str() }
}
```

---

## E8: Event Bus with Bounded Channel (L4 — M23)

From ME v1 `m4_integration/event_bus.rs`:

```rust
pub struct EventBus {
    sender: mpsc::Sender<Event>,
    receiver: Arc<Mutex<mpsc::Receiver<Event>>>,
    subscribers: RwLock<HashMap<String, Vec<Arc<dyn EventHandler>>>>,
}

impl EventBus {
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::channel(1000);  // Bounded!
        Self {
            sender,
            receiver: Arc::new(Mutex::new(receiver)),
            subscribers: RwLock::new(HashMap::new()),
        }
    }

    pub fn publish(&self, event: Event) -> Result<()> {
        self.sender.try_send(event)
            .map_err(|e| Error::Network {
                target: "event_bus".into(),
                message: format!("Channel full or closed: {e}"),
            })
    }
}
```

---

## E9: Agent Fleet (L6 — M32)

From ME v1 `m6_consensus/agent.rs`:

```rust
pub const HUMAN_AGENT_TAG: &str = "@0.A";

pub struct ConsensusAgent {
    pub agent_id: String,
    pub role: AgentRole,
    pub weight: f64,
    pub active: bool,
}

pub fn create_fleet() -> Vec<ConsensusAgent> {
    let mut fleet = Vec::with_capacity(41);

    // 20 Validators (weight 1.0)
    for i in 0..20 {
        fleet.push(ConsensusAgent {
            agent_id: format!("validator-{i:02}"),
            role: AgentRole::Validator,
            weight: 1.0,
            active: true,
        });
    }

    // 8 Explorers (weight 0.8)
    for i in 0..8 {
        fleet.push(ConsensusAgent {
            agent_id: format!("explorer-{i:02}"),
            role: AgentRole::Explorer,
            weight: 0.8,
            active: true,
        });
    }

    // 6 Critics (weight 1.2)
    for i in 0..6 {
        fleet.push(ConsensusAgent {
            agent_id: format!("critic-{i:02}"),
            role: AgentRole::Critic,
            weight: 1.2,
            active: true,
        });
    }

    // 4 Integrators (weight 1.0)
    for i in 0..4 {
        fleet.push(ConsensusAgent {
            agent_id: format!("integrator-{i:02}"),
            role: AgentRole::Integrator,
            weight: 1.0,
            active: true,
        });
    }

    // 2 Historians (weight 0.8)
    for i in 0..2 {
        fleet.push(ConsensusAgent {
            agent_id: format!("historian-{i:02}"),
            role: AgentRole::Historian,
            weight: 0.8,
            active: true,
        });
    }

    // Human peer (R5)
    fleet.push(ConsensusAgent {
        agent_id: HUMAN_AGENT_TAG.to_string(),
        role: AgentRole::Validator,
        weight: 1.0,
        active: true,
    });

    fleet  // 41 total: 40 synthetic + 1 human
}
```

---

## E10: Observer Bus (L7)

From ME v1 `m7_observer/observer_bus.rs`:

```rust
pub trait ObserverBusOps: Send + Sync {
    fn subscribe(&self, event_type: &str, handler: Arc<dyn EventHandler>) -> Result<()>;
    fn emit(&self, event: ObserverEvent) -> Result<()>;
    fn get_event_history(&self, limit: usize) -> Vec<ObserverEvent>;
}

pub enum ObserverEvent {
    HealthTransition {
        service_id: String,
        from: HealthStatus,
        to: HealthStatus,
    },
    CascadeDetected {
        magnitude: f64,
        affected_services: usize,
    },
    EmergenceDetected {
        emergence_type: EmergenceType,
        confidence: f64,
    },
    EvolutionTriggered {
        r_baseline: f64,
        r_after: f64,
        verdict: String,
    },
}
```

---

## E11: Valid Lifecycle Transitions (L2 — M11)

From ME v2 `m2_services/lifecycle.rs`:

```rust
fn is_valid_transition(from: ServiceStatus, to: ServiceStatus) -> bool {
    matches!(
        (from, to),
        (ServiceStatus::Stopped, ServiceStatus::Starting)
        | (ServiceStatus::Starting, ServiceStatus::Running)
        | (ServiceStatus::Starting, ServiceStatus::Failed)
        | (ServiceStatus::Running, ServiceStatus::Stopping)
        | (ServiceStatus::Running, ServiceStatus::Failed)
        | (ServiceStatus::Stopping, ServiceStatus::Stopped)
        | (ServiceStatus::Failed, ServiceStatus::Starting)  // Recovery
        | (ServiceStatus::Running, ServiceStatus::Running)   // Idempotent
        | (ServiceStatus::Stopped, ServiceStatus::Stopped)   // Idempotent
    )
}
```

---

## E12: Test Helper Factory (L2 — All Modules)

From ME v2 `m2_services/health_monitor.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn make_probe(id: &str) -> HealthProbe {
        HealthProbeBuilder::new(id, format!("http://localhost/{id}"))
            .interval_ms(10_000)
            .timeout_ms(2_000)
            .healthy_threshold(2)
            .unhealthy_threshold(2)
            .build()
            .expect("valid probe config")  // OK in tests
    }

    fn populated_monitor() -> HealthMonitor {
        let m = HealthMonitor::new();
        let _ = m.register_probe(make_probe("alpha"));
        let _ = m.register_probe(make_probe("beta"));
        let _ = m.register_probe(make_probe("gamma"));
        m
    }

    fn test_service(id: &str, tier: ServiceTier, port: u16) -> ServiceDefinition {
        ServiceDefinitionBuilder::new(id, format!("Service {id}"))
            .tier(tier)
            .port(port)
            .build()
            .expect("valid service")  // OK in tests
    }
}
```

---

## Exemplar Index

| ID | Pattern | Layer | Module | Key Takeaway |
|----|---------|-------|--------|-------------|
| E1 | Confidence FMA | L3 | M15 | FMA chain for precision |
| E2 | Escalation tiers | L3 | M14 | Priority: critical→L3, high conf→L0 |
| E3 | PBFT phases | L6 | M31 | Phase machine with quorum checks |
| E4 | Circuit breaker | L2 | M12 | 3-state FSM with timeout |
| E5 | Hebbian interior mut | L5 | M25 | Multiple RwLocks, separate concerns |
| E6 | Health threshold FSM | L2 | M10 | Consecutive count → state change |
| E7 | TensorContributor | L2 | M10 | Coverage bitmap + dimension mapping |
| E8 | Bounded event bus | L4 | M23 | Channel capacity 1000 |
| E9 | Agent fleet | L6 | M32 | 40+1 agents with role weights |
| E10 | Observer bus | L7 | — | Event-driven pub/sub |
| E11 | Lifecycle FSM | L2 | M11 | `matches!` for valid transitions |
| E12 | Test helpers | L2 | All | Factory functions, sensible defaults |

---

*12 exemplars from production code (ME v1: 56K LOC, 2,327 tests)*
