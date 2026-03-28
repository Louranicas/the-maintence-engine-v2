# Event Channel Specification - L7 Observer Layer

```json
{"v":"1.0.0","type":"CHANNEL_SPEC","layer":7,"new_channels":3,"existing_channels":6}
```

**Version:** 1.0.0
**Layer:** L7 (Observer)
**Scope:** 3 new EventBus channels for L7 output
**Related:** [PIPELINE_SPEC.md](../PIPELINE_SPEC.md), [LOG_CORRELATOR_SPEC.md](LOG_CORRELATOR_SPEC.md), [FITNESS_FUNCTION_SPEC.md](FITNESS_FUNCTION_SPEC.md)

---

## Navigation

| Direction | Target |
|-----------|--------|
| Up | [INDEX.md](INDEX.md) |
| Prev | [FITNESS_FUNCTION_SPEC.md](FITNESS_FUNCTION_SPEC.md) |
| Next | [TYPE_DEFINITIONS_SPEC.md](TYPE_DEFINITIONS_SPEC.md) |

---

## 1. Overview

L7 Observer Layer creates 3 new EventBus channels in addition to the 6 existing ones established in L1-L6. These channels carry L7 output to external consumers without requiring direct L7 module access. All 3 channels are created during engine startup, immediately after `EventBus::new()`.

### Channel Inventory

| # | Channel | Source Layer | Status |
|---|---------|-------------|--------|
| 1 | `health` | L2 | Existing |
| 2 | `remediation` | L3 | Existing |
| 3 | `learning` | L5 | Existing |
| 4 | `consensus` | L6 | Existing |
| 5 | `integration` | L4 | Existing |
| 6 | `metrics` | L1 | Existing |
| 7 | **`observation`** | **L7** | **NEW** |
| 8 | **`emergence`** | **L7** | **NEW** |
| 9 | **`evolution`** | **L7** | **NEW** |

---

## 2. Existing Channels (Reference)

| Channel | Source Layer | Event Types | Approximate Rate |
|---------|-------------|-------------|------------------|
| `health` | L2 (Services) | `health_check`, `health_change`, `health_alert` | ~10/s |
| `remediation` | L3 (Core Logic) | `action_started`, `action_completed`, `action_failed` | ~1/s |
| `learning` | L5 (Learning) | `ltp_applied`, `ltd_applied`, `pathway_created`, `pathway_pruned` | ~5/s |
| `consensus` | L6 (Consensus) | `round_started`, `vote_cast`, `round_completed`, `view_change` | ~2/s |
| `integration` | L4 (Integration) | `bridge_connected`, `bridge_failed`, `message_sent` | ~3/s |
| `metrics` | L1 (Foundation) | `metric_recorded`, `metric_alert`, `metric_threshold` | ~20/s |

---

## 3. New Channel: `observation`

### 3.1 Channel Properties

| Property | Value |
|----------|-------|
| Channel Name | `observation` |
| Source Module | M37 LogCorrelator |
| Created At | Engine startup (after `EventBus::new()`) |
| Subscriber ID | External consumers only (L7 internal uses ObserverBus) |
| Payload Format | JSON string (serde_json serialization) |

### 3.2 Event Types

| Event Type | Payload Schema | Frequency | Description |
|------------|---------------|-----------|-------------|
| `correlation_found` | See 3.3 | ~100/s peak | A new cross-layer correlation has been detected |
| `recurring_pattern` | See 3.4 | ~1/min | A recurring event pattern has been identified or updated |
| `correlation_stats` | See 3.5 | ~1/min | Periodic aggregate statistics from the correlator |

### 3.3 `correlation_found` Payload

```json
{
  "event_id": "550e8400-e29b-41d4-a716-446655440000",
  "correlations": [
    {
      "target": "660e8400-e29b-41d4-a716-446655440001",
      "type": "Temporal",
      "confidence": 0.85,
      "delta_ms": 120
    },
    {
      "target": "770e8400-e29b-41d4-a716-446655440002",
      "type": "Causal",
      "confidence": 0.72,
      "delta_ms": 340
    }
  ]
}
```

| Field | Type | Description |
|-------|------|-------------|
| `event_id` | String (UUID v4) | The source event that was correlated |
| `correlations` | Array | List of discovered correlation links |
| `correlations[].target` | String (UUID v4) | Target event ID |
| `correlations[].type` | String enum | One of: `Temporal`, `Causal`, `Resonance`, `Cascade`, `Periodic` |
| `correlations[].confidence` | f64 [0.0, 1.0] | Confidence in the correlation |
| `correlations[].delta_ms` | i64 | Signed time delta in milliseconds |

### 3.4 `recurring_pattern` Payload

```json
{
  "pattern_id": "880e8400-e29b-41d4-a716-446655440003",
  "sequence": ["health_check", "remediation_started"],
  "count": 5,
  "interval_ms": 30000
}
```

| Field | Type | Description |
|-------|------|-------------|
| `pattern_id` | String (UUID v4) | Unique pattern identifier |
| `sequence` | Array[String] | Ordered event_type sequence forming the pattern |
| `count` | u64 | Total occurrences observed |
| `interval_ms` | u64 | Mean interval between occurrences in milliseconds |

### 3.5 `correlation_stats` Payload

```json
{
  "total_ingested": 1000,
  "total_correlations": 450,
  "active_windows": 12,
  "patterns": 3
}
```

| Field | Type | Description |
|-------|------|-------------|
| `total_ingested` | u64 | Total events ingested since startup |
| `total_correlations` | u64 | Total correlation links discovered |
| `active_windows` | usize | Currently open correlation windows |
| `patterns` | usize | Detected recurring patterns count |

---

## 4. New Channel: `emergence`

### 4.1 Channel Properties

| Property | Value |
|----------|-------|
| Channel Name | `emergence` |
| Source Module | M38 EmergenceDetector |
| Created At | Engine startup |
| Subscriber ID | External consumers, L3 escalation |
| Payload Format | JSON string (serde_json serialization) |

### 4.2 Event Types

| Event Type | Payload Schema | Frequency | Severity | Description |
|------------|---------------|-----------|----------|-------------|
| `cascading_failure` | See 4.3 | ~0.1/hr | Critical | Failure propagating through dependency chain |
| `synergy_amplification` | See 4.4 | ~1/hr | Informational | Cross-service cooperation amplifying performance |
| `self_organizing_recovery` | See 4.5 | ~0.5/hr | Notable | System self-healed without human intervention |
| `resonance_pattern` | See 4.6 | ~0.2/hr | Notable | Oscillating behavior detected across layers |
| `load_shedding` | See 4.7 | ~0.3/hr | Warning | Overloaded service redistributing work |
| `pathway_convergence` | See 4.8 | ~0.5/hr | Informational | Multiple Hebbian pathways converging on target |
| `adaptive_threshold` | See 4.9 | ~0.2/hr | Informational | System autonomously adjusted an operational threshold |
| `emergence_stats` | See 4.10 | ~1/min | Informational | Periodic aggregate emergence statistics |

### 4.3 `cascading_failure` Payload

```json
{
  "origin": "service_a",
  "affected": ["service_b", "service_c"],
  "depth": 3,
  "blast_radius": 0.25
}
```

| Field | Type | Description |
|-------|------|-------------|
| `origin` | String | Service where the failure originated |
| `affected` | Array[String] | Services impacted by the cascade |
| `depth` | u32 | Cascade propagation depth (hops from origin) |
| `blast_radius` | f64 [0.0, 1.0] | Fraction of total services affected |

### 4.4 `synergy_amplification` Payload

```json
{
  "services": ["synthex", "san-k7"],
  "delta": 0.18,
  "trigger": "cross_service_optimization"
}
```

| Field | Type | Description |
|-------|------|-------------|
| `services` | Array[String] | Services exhibiting synergy |
| `delta` | f64 | Synergy score improvement magnitude |
| `trigger` | String | Event or condition that triggered the amplification |

### 4.5 `self_organizing_recovery` Payload

```json
{
  "failed": "service_x",
  "pathway": ["circuit_breaker", "health_check", "restart"],
  "time_ms": 5000
}
```

| Field | Type | Description |
|-------|------|-------------|
| `failed` | String | Service that experienced failure |
| `pathway` | Array[String] | Ordered remediation steps taken autonomously |
| `time_ms` | u64 | Total recovery time in milliseconds |

### 4.6 `resonance_pattern` Payload

```json
{
  "layers": [2, 5],
  "frequency_ms": 30000,
  "amplitude": 0.85,
  "phase": 0.92
}
```

| Field | Type | Description |
|-------|------|-------------|
| `layers` | Array[u8] | Layers exhibiting resonant behavior (1-6) |
| `frequency_ms` | u64 | Oscillation period in milliseconds |
| `amplitude` | f64 [0.0, 1.0] | Strength of the oscillation |
| `phase` | f64 [0.0, 1.0] | Phase alignment between layers (1.0 = perfect sync) |

### 4.7 `load_shedding` Payload

```json
{
  "overloaded": "service_a",
  "shed_to": ["service_b"],
  "before": 0.95,
  "after": 0.72
}
```

| Field | Type | Description |
|-------|------|-------------|
| `overloaded` | String | Service that exceeded load threshold |
| `shed_to` | Array[String] | Services that absorbed redirected load |
| `before` | f64 [0.0, 1.0] | Load metric before shedding |
| `after` | f64 [0.0, 1.0] | Load metric after shedding |

### 4.8 `pathway_convergence` Payload

```json
{
  "pathways": ["a->b", "c->b"],
  "target": "b",
  "combined": 1.8
}
```

| Field | Type | Description |
|-------|------|-------------|
| `pathways` | Array[String] | Hebbian pathways converging on the target |
| `target` | String | Convergence target service/module |
| `combined` | f64 | Combined pathway weight at convergence point |

### 4.9 `adaptive_threshold` Payload

```json
{
  "metric": "health_check_interval",
  "old": 5000,
  "new": 3000,
  "trigger": "increased_failures"
}
```

| Field | Type | Description |
|-------|------|-------------|
| `metric` | String | The operational metric whose threshold was adjusted |
| `old` | u64 | Previous threshold value |
| `new` | u64 | New threshold value |
| `trigger` | String | Condition that triggered the adjustment |

### 4.10 `emergence_stats` Payload

```json
{
  "total_detected": 15,
  "by_severity": {
    "informational": 8,
    "notable": 5,
    "warning": 1,
    "critical": 1
  }
}
```

| Field | Type | Description |
|-------|------|-------------|
| `total_detected` | u64 | Total emergence events detected since startup |
| `by_severity` | Object | Breakdown by severity classification |
| `by_severity.informational` | u64 | Count of informational events |
| `by_severity.notable` | u64 | Count of notable events |
| `by_severity.warning` | u64 | Count of warning events |
| `by_severity.critical` | u64 | Count of critical events |

---

## 5. New Channel: `evolution`

### 5.1 Channel Properties

| Property | Value |
|----------|-------|
| Channel Name | `evolution` |
| Source Module | M39 EvolutionChamber |
| Created At | Engine startup |
| Subscriber ID | Audit log, human operators, external dashboards |
| Payload Format | JSON string (serde_json serialization) |

### 5.2 Event Types

| Event Type | Payload Schema | Frequency | Description |
|------------|---------------|-----------|-------------|
| `generation_started` | See 5.3 | ~1/min | New evolution generation begun |
| `mutation_generated` | See 5.4 | ~3/min | Candidate mutation proposed |
| `mutation_applied` | See 5.5 | ~2/min | Mutation applied to live system |
| `mutation_committed` | See 5.6 | ~1.5/min | Mutation confirmed (fitness improved) |
| `mutation_rolled_back` | See 5.7 | ~0.5/min | Mutation reverted (fitness dropped) |
| `consensus_requested` | See 5.8 | Rare | High-impact mutation escalated to PBFT |
| `generation_completed` | See 5.9 | ~1/min | Evolution generation summary |
| `fitness_report` | See 5.10 | ~1/min | Current system fitness snapshot |

### 5.3 `generation_started` Payload

```json
{
  "generation": 42,
  "system_state": "Stable",
  "fitness": 0.87
}
```

| Field | Type | Description |
|-------|------|-------------|
| `generation` | u64 | Monotonically increasing generation counter |
| `system_state` | String enum | One of: `Stable`, `Improving`, `Declining`, `Volatile` |
| `fitness` | f64 [0.0, 1.0] | Overall system fitness at generation start |

### 5.4 `mutation_generated` Payload

```json
{
  "id": "990e8400-e29b-41d4-a716-446655440004",
  "type": "PathwayAdjustment",
  "expected_delta": 0.05,
  "consensus_required": false
}
```

| Field | Type | Description |
|-------|------|-------------|
| `id` | String (UUID v4) | Unique mutation identifier |
| `type` | String enum | Mutation classification (see 5.11) |
| `expected_delta` | f64 | Predicted fitness change magnitude |
| `consensus_required` | bool | Whether PBFT consensus is needed before application |

### 5.5 `mutation_applied` Payload

```json
{
  "id": "990e8400-e29b-41d4-a716-446655440004",
  "type": "ThresholdAdjustment",
  "fitness_before": 0.87
}
```

| Field | Type | Description |
|-------|------|-------------|
| `id` | String (UUID v4) | Mutation identifier (matches `mutation_generated.id`) |
| `type` | String enum | Mutation classification |
| `fitness_before` | f64 [0.0, 1.0] | System fitness immediately before application |

### 5.6 `mutation_committed` Payload

```json
{
  "id": "990e8400-e29b-41d4-a716-446655440004",
  "actual_delta": 0.03,
  "fitness_after": 0.90
}
```

| Field | Type | Description |
|-------|------|-------------|
| `id` | String (UUID v4) | Mutation identifier |
| `actual_delta` | f64 | Actual fitness change observed |
| `fitness_after` | f64 [0.0, 1.0] | System fitness after commitment |

### 5.7 `mutation_rolled_back` Payload

```json
{
  "id": "990e8400-e29b-41d4-a716-446655440004",
  "actual_delta": -0.05,
  "reason": "fitness_drop"
}
```

| Field | Type | Description |
|-------|------|-------------|
| `id` | String (UUID v4) | Mutation identifier |
| `actual_delta` | f64 | Fitness change that triggered rollback (negative) |
| `reason` | String | Human-readable rollback reason |

### 5.8 `consensus_requested` Payload

```json
{
  "mutation_id": "990e8400-e29b-41d4-a716-446655440004",
  "type": "LoadBalancerReweight",
  "expected_delta": 0.12
}
```

| Field | Type | Description |
|-------|------|-------------|
| `mutation_id` | String (UUID v4) | Mutation identifier requiring consensus |
| `type` | String enum | Mutation classification |
| `expected_delta` | f64 | Predicted fitness impact (triggers consensus when > threshold) |

### 5.9 `generation_completed` Payload

```json
{
  "generation": 42,
  "applied": 2,
  "committed": 1,
  "rolled_back": 1,
  "fitness_delta": 0.02
}
```

| Field | Type | Description |
|-------|------|-------------|
| `generation` | u64 | Generation counter (matches `generation_started`) |
| `applied` | u32 | Total mutations applied this generation |
| `committed` | u32 | Mutations that improved fitness and were kept |
| `rolled_back` | u32 | Mutations reverted due to fitness decline |
| `fitness_delta` | f64 | Net fitness change across the generation |

### 5.10 `fitness_report` Payload

```json
{
  "overall": 0.89,
  "trend": "Improving",
  "weakest": "D9_latency",
  "strongest": "D7_uptime"
}
```

| Field | Type | Description |
|-------|------|-------------|
| `overall` | f64 [0.0, 1.0] | Overall system fitness score |
| `trend` | String enum | One of: `Improving`, `Stable`, `Declining`, `Volatile` |
| `weakest` | String | Dimension name with the lowest weighted score |
| `strongest` | String | Dimension name with the highest weighted score |

### 5.11 Mutation Type Enum

| Mutation Type | Description | Consensus Required |
|---------------|-------------|-------------------|
| `PathwayAdjustment` | Hebbian pathway weight modification | No |
| `ThresholdAdjustment` | Operational threshold change | No |
| `WeightRebalance` | Fitness dimension weight tuning | No |
| `CircuitBreakerTune` | Circuit breaker parameter change | No |
| `LoadBalancerReweight` | Load balancer weight redistribution | **Yes** (high impact) |
| `ServiceRestart` | Service restart recommendation | **Yes** (critical) |
| `TierPromotion` | Service tier elevation | **Yes** (structural) |
| `AgentReallocation` | Agent role/count redistribution | **Yes** (consensus) |

---

## 6. Channel Creation Code

```rust
use crate::m4_integration::EventBus;

/// Create L7 Observer Layer channels during engine startup.
///
/// # Preconditions
/// - EventBus has been initialized via `EventBus::new()`
/// - Existing 6 channels (health, remediation, learning, consensus,
///   integration, metrics) are already created
///
/// # Postconditions
/// - 3 new channels available: observation, emergence, evolution
/// - Channels are empty (no events or subscribers)
///
/// # Errors
/// - `Error::Validation("channel already exists")` if channel name is duplicate
pub fn create_observer_channels(event_bus: &EventBus) -> Result<()> {
    event_bus.create_channel("observation")?;
    event_bus.create_channel("emergence")?;
    event_bus.create_channel("evolution")?;
    Ok(())
}
```

### Channel Initialization Order

| Step | Action | Channel Count |
|------|--------|---------------|
| 1 | `EventBus::new()` | 0 |
| 2 | L1-L6 channel creation | 6 |
| 3 | `create_observer_channels()` | **9** |
| 4 | Module subscriptions | 9 (with subscribers) |

---

## 7. Payload Serialization

All payloads are JSON strings, matching the existing EventBus convention where `EventRecord.payload` is `String`. L7 modules serialize via `serde_json` before publishing. No new dependencies are required since `serde_json` is already used throughout the codebase.

### Serialization Contract

| Property | Value |
|----------|-------|
| Format | JSON (UTF-8 string) |
| Library | `serde_json::to_string()` |
| New Dependencies | None |
| Max Payload Size | 4 KB (soft limit, log warning if exceeded) |
| Encoding | All f64 values serialized with full precision |
| UUID Format | Standard hyphenated (8-4-4-4-12) |

### Rust Type Derivations

```rust
/// All L7 event payloads derive these traits for serialization.
/// This matches the established pattern across L1-L6 modules.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct CorrelationFoundPayload {
    pub event_id: String,
    pub correlations: Vec<CorrelationLinkPayload>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct CorrelationLinkPayload {
    pub target: String,
    #[serde(rename = "type")]
    pub correlation_type: String,
    pub confidence: f64,
    pub delta_ms: i64,
}

// ... additional payload types follow the same pattern
```

---

## 8. Rate Limiting

| Channel | Max Rate | Burst | Rationale |
|---------|----------|-------|-----------|
| `observation` | 200/s | 500 | High-frequency correlation output; M37 may detect hundreds of correlations per second during incident spikes |
| `emergence` | 10/s | 50 | Bursty during cascading incidents; normally low-frequency |
| `evolution` | 5/s | 20 | Low-frequency mutation lifecycle; bounded by generation interval |

### Rate Limiting Implementation

| Property | Value |
|----------|-------|
| Algorithm | Token bucket |
| Enforcement | Publisher-side (L7 modules check before `publish()`) |
| Overflow behavior | Drop event silently, increment `rate_limited_count` metric |
| Backpressure | None (fire-and-forget semantics match existing channels) |

### Rate Comparison with Existing Channels

| Channel | Rate | L7 Comparison |
|---------|------|---------------|
| `metrics` (L1) | ~20/s | `observation` is 10x higher (peak) |
| `health` (L2) | ~10/s | `observation` is 20x higher (peak) |
| `learning` (L5) | ~5/s | `emergence` is 2x higher (peak) |
| `consensus` (L6) | ~2/s | `evolution` is 2.5x higher (sustained) |

---

## 9. Consumer Guidelines

### 9.1 Subscription Recommendations

| Consumer Type | Recommended Channels | Filter |
|---------------|---------------------|--------|
| Dashboard UI | `evolution`, `emergence` | All event types |
| Audit logger | `evolution` | All event types (durable) |
| Alert system | `emergence` | `cascading_failure`, `load_shedding` only |
| Analytics pipeline | `observation` | `correlation_stats`, `recurring_pattern` only |
| L3 Escalation | `emergence` | `cascading_failure`, `self_organizing_recovery` |
| Human operator | `evolution` | `fitness_report`, `generation_completed` |

### 9.2 Channel-Specific Guidance

| Channel | Guidance |
|---------|----------|
| `observation` | **High-volume** -- subscribe with specific event_type filters to reduce noise. The `correlation_found` event can fire at ~100/s during incidents. Prefer `correlation_stats` for periodic summaries. |
| `emergence` | **Escalation-critical** -- `cascading_failure` and `load_shedding` events may trigger L3 escalation. Consume promptly to avoid delayed remediation. |
| `evolution` | **Audit-critical** -- ensure durable consumption for compliance. The `mutation_applied` / `mutation_committed` / `mutation_rolled_back` sequence forms an audit trail for all system modifications. |

### 9.3 Event Ordering Guarantees

| Guarantee | Scope |
|-----------|-------|
| Per-channel FIFO | Events within a single channel are delivered in publish order |
| No cross-channel ordering | Events across different channels have no ordering guarantee |
| At-most-once delivery | Matches existing EventBus semantics (no durability layer) |
| No deduplication | Consumers must handle potential duplicates during high load |

---

## 10. Rust Type Definitions for All Payloads

### 10.1 Observation Channel Types

```rust
use serde::{Serialize, Deserialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CorrelationFoundPayload {
    pub event_id: String,
    pub correlations: Vec<CorrelationLinkPayload>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CorrelationLinkPayload {
    pub target: String,
    #[serde(rename = "type")]
    pub correlation_type: String,
    pub confidence: f64,
    pub delta_ms: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RecurringPatternPayload {
    pub pattern_id: String,
    pub sequence: Vec<String>,
    pub count: u64,
    pub interval_ms: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CorrelationStatsPayload {
    pub total_ingested: u64,
    pub total_correlations: u64,
    pub active_windows: usize,
    pub patterns: usize,
}
```

### 10.2 Emergence Channel Types

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CascadingFailurePayload {
    pub origin: String,
    pub affected: Vec<String>,
    pub depth: u32,
    pub blast_radius: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SynergyAmplificationPayload {
    pub services: Vec<String>,
    pub delta: f64,
    pub trigger: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SelfOrganizingRecoveryPayload {
    pub failed: String,
    pub pathway: Vec<String>,
    pub time_ms: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResonancePatternPayload {
    pub layers: Vec<u8>,
    pub frequency_ms: u64,
    pub amplitude: f64,
    pub phase: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LoadSheddingPayload {
    pub overloaded: String,
    pub shed_to: Vec<String>,
    pub before: f64,
    pub after: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PathwayConvergencePayload {
    pub pathways: Vec<String>,
    pub target: String,
    pub combined: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AdaptiveThresholdPayload {
    pub metric: String,
    pub old: u64,
    pub new: u64,
    pub trigger: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EmergenceStatsPayload {
    pub total_detected: u64,
    pub by_severity: EmergenceSeverityCounts,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EmergenceSeverityCounts {
    pub informational: u64,
    pub notable: u64,
    pub warning: u64,
    pub critical: u64,
}
```

### 10.3 Evolution Channel Types

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GenerationStartedPayload {
    pub generation: u64,
    pub system_state: String,
    pub fitness: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MutationGeneratedPayload {
    pub id: String,
    #[serde(rename = "type")]
    pub mutation_type: String,
    pub expected_delta: f64,
    pub consensus_required: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MutationAppliedPayload {
    pub id: String,
    #[serde(rename = "type")]
    pub mutation_type: String,
    pub fitness_before: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MutationCommittedPayload {
    pub id: String,
    pub actual_delta: f64,
    pub fitness_after: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MutationRolledBackPayload {
    pub id: String,
    pub actual_delta: f64,
    pub reason: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConsensusRequestedPayload {
    pub mutation_id: String,
    #[serde(rename = "type")]
    pub mutation_type: String,
    pub expected_delta: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GenerationCompletedPayload {
    pub generation: u64,
    pub applied: u32,
    pub committed: u32,
    pub rolled_back: u32,
    pub fitness_delta: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FitnessReportPayload {
    pub overall: f64,
    pub trend: String,
    pub weakest: String,
    pub strongest: String,
}
```

---

## 11. Cross-Spec Dependencies

| Dependency | Source Spec | Integration Point |
|------------|-------------|-------------------|
| EventBus (M23) | [PIPELINE_SPEC.md](../PIPELINE_SPEC.md) | Channel creation and event publishing |
| EventRecord type | M23 Event Bus (L4) | Payload wrapping for all 3 channels |
| LogCorrelator (M37) | [LOG_CORRELATOR_SPEC.md](LOG_CORRELATOR_SPEC.md) | Publisher to `observation` channel |
| EmergenceDetector (M38) | EMERGENCE_DETECTOR_SPEC.md | Publisher to `emergence` channel |
| EvolutionChamber (M39) | RALPH_LOOP_SPEC.md | Publisher to `evolution` channel |
| FitnessEvaluator | [FITNESS_FUNCTION_SPEC.md](FITNESS_FUNCTION_SPEC.md) | Fitness data for `evolution` channel events |
| PBFT Consensus (M31) | [PBFT_SPEC.md](../PBFT_SPEC.md) | `consensus_requested` event triggers PBFT round |
| Escalation (M14) | [ESCALATION_SPEC.md](../ESCALATION_SPEC.md) | `emergence` channel events feed L3 escalation |
| Error taxonomy (M01) | M01 Error (L1) | Error::Validation for channel creation failures |

---

## 12. Version History

| Version | Date | Changes |
|---------|------|---------|
| 1.0.0 | 2026-01-29 | Initial specification |

---

*Generated: 2026-01-29 | The Maintenance Engine v1.0.0 | L7 Observer Layer*
