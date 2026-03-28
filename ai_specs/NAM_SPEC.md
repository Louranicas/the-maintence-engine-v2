# NAM Compliance Specification

## Overview

Neural Autonomic Matrix (NAM) compliance ensures autonomous self-management capabilities across all production systems. This specification defines the requirements for achieving the target 92% NAM compliance score.

---

## NAM Compliance (Target: 92%)

### Requirements Table

| ID | Name | Target | Description |
|----|------|--------|-------------|
| R1 | SelfQuery | 90% | Autonomous SQL query loops |
| R2 | HebbianRouting | 88% | Pathway-weighted routing |
| R3 | DissentCapture | 85% | Disagreement learning |
| R4 | FieldVisualization | 90% | Machine-readable topology |
| R5 | HumanAsAgent | 95% | Human @0.A registration |

---

## R1: Self-Query (NAM-01)

### Description
Self-Query enables autonomous introspection through SQL query loops, allowing the system to monitor and analyze its own state without external intervention.

### Capabilities
- Query loops for self-introspection
- Autonomous database queries
- Real-time state analysis
- Trend detection and anomaly identification

### Implementation Status
| Component | File | Status |
|-----------|------|--------|
| Service State Queries | service_state.sql | Active |
| Synergy Matrix Queries | synergy_matrix.sql | Active |

### Query Types
```sql
-- Self-state query example
SELECT service_id, health_status, last_heartbeat,
       TIMESTAMPDIFF(SECOND, last_heartbeat, NOW()) as latency_seconds
FROM service_registry
WHERE active = 1;

-- Synergy matrix query example
SELECT source_service, target_service, synergy_score,
       weight_factor, last_updated
FROM synergy_matrix
WHERE synergy_score < 0.85
ORDER BY synergy_score ASC;
```

### Compliance Criteria
- [ ] Query execution latency < 100ms
- [ ] Query loop interval: 5-30 seconds configurable
- [ ] Error recovery within 3 attempts
- [ ] Logging of all query results

---

## R2: Hebbian Routing (NAM-02)

### Description
Implements Spike-Timing-Dependent Plasticity (STDP) based pathway strengthening for intelligent routing decisions.

### Parameters
| Parameter | Value | Description |
|-----------|-------|-------------|
| tau_plus | 20ms | Time constant for potentiation |
| tau_minus | 20ms | Time constant for depression |
| a_plus | 0.01 | Learning rate for potentiation |
| a_minus | 0.012 | Learning rate for depression |
| w_max | 1.0 | Maximum weight |
| w_min | 0.0 | Minimum weight |

### STDP Weight Update Formula
```
if delta_t > 0:
    delta_w = a_plus * exp(-delta_t / tau_plus)
else:
    delta_w = -a_minus * exp(delta_t / tau_minus)
```

### Routing Decision Algorithm
1. Collect pathway weights for all available routes
2. Apply softmax normalization
3. Select route based on weighted probability
4. Update weights based on outcome (success/failure)

### Compliance Criteria
- [ ] Weight updates within 50ms of outcome
- [ ] Pathway convergence within 1000 iterations
- [ ] Minimum 3 alternative routes per decision point

---

## R3: Dissent Capture (NAM-03)

### Description
Records and learns from minority opinions and disagreements in consensus processes, ensuring valuable dissent is not lost.

### Implementation
- Record minority opinions in all consensus votes
- Learn from disagreements through weight adjustment
- CRITIC role weighted at 1.2 for enhanced dissent influence

### Dissent Record Schema
```json
{
  "dissent_id": "uuid",
  "timestamp": "ISO8601",
  "context": {
    "decision_type": "string",
    "consensus_result": "string",
    "consensus_confidence": 0.0-1.0
  },
  "dissenting_agents": [
    {
      "agent_id": "string",
      "role": "string",
      "position": "string",
      "reasoning": "string",
      "confidence": 0.0-1.0
    }
  ],
  "outcome_tracking": {
    "tracked": true,
    "outcome_timestamp": "ISO8601",
    "outcome_aligned_with": "consensus|dissent"
  }
}
```

### Learning from Dissent
| Outcome | Weight Adjustment |
|---------|-------------------|
| Dissent correct | Increase dissenter weight by 0.05 |
| Consensus correct | No change (maintain diversity) |
| Unclear outcome | Record for future analysis |

### Compliance Criteria
- [ ] 100% dissent capture rate
- [ ] Outcome tracking for 90%+ decisions
- [ ] Weight adjustments applied within 24 hours

---

## R4: Field Visualization (NAM-04)

### Description
Machine-readable topology representation using 12-dimensional tensor encoding for system state visualization.

### 12D Tensor Structure
| Dimension | Index | Description | Range |
|-----------|-------|-------------|-------|
| Health | 0 | Service health score | 0.0-1.0 |
| Load | 1 | Current load percentage | 0.0-1.0 |
| Latency | 2 | Response latency (normalized) | 0.0-1.0 |
| Synergy | 3 | Cross-service synergy | 0.0-1.0 |
| Autonomy | 4 | Self-management capability | 0.0-1.0 |
| Memory | 5 | Memory utilization | 0.0-1.0 |
| Throughput | 6 | Request throughput (normalized) | 0.0-1.0 |
| Error Rate | 7 | Inverse error rate | 0.0-1.0 |
| Uptime | 8 | Service uptime ratio | 0.0-1.0 |
| Dependencies | 9 | Dependency health score | 0.0-1.0 |
| Queue Depth | 10 | Queue saturation (inverse) | 0.0-1.0 |
| NAM Score | 11 | Overall NAM compliance | 0.0-1.0 |

### Export Formats
```json
// JSON Export Format
{
  "service_id": "SYNTHEX",
  "timestamp": "2026-01-28T12:00:00Z",
  "tensor": [0.95, 0.72, 0.88, 0.94, 0.91, 0.65, 0.78, 0.97, 0.999, 0.92, 0.85, 0.92],
  "metadata": {
    "version": "1.0",
    "encoding": "float32"
  }
}
```

```
// Binary Export Format (12 x float32 = 48 bytes per service)
[4 bytes header][48 bytes tensor][4 bytes checksum]
```

### Compliance Criteria
- [ ] Tensor update frequency: 1 Hz minimum
- [ ] Export latency < 10ms
- [ ] Support both JSON and binary formats

---

## R5: Human As Agent (NAM-05)

### Description
Registers human operators as first-class agents within the system, enabling meaningful human-AI collaboration.

### Human Agent Registration
```json
{
  "agent_id": "@0.A",
  "tier": 0,
  "weight": 1.0,
  "role": "peer",
  "capabilities": [
    "consensus_vote",
    "dissent",
    "override",
    "escalation_response"
  ]
}
```

### Human Agent Properties
| Property | Value | Description |
|----------|-------|-------------|
| agent_id | @0.A | Special Tier-0 identifier |
| tier | 0 | Highest priority tier |
| weight | 1.0 | Full voting weight |
| role | peer | Equal participant, not supervisor |

### Human Capabilities
| Capability | Description |
|------------|-------------|
| consensus_vote | Participate in all consensus decisions |
| dissent | Register disagreement with reasoning |
| override | Emergency override capability |
| escalation_response | Respond to system escalations |

### Interaction Modes
1. **Passive Monitor**: Observe system state and decisions
2. **Active Participant**: Vote in consensus processes
3. **Override Authority**: Intervene in critical decisions
4. **Escalation Handler**: Respond to system-generated escalations

---

## Agent Distribution (NAM-05)

### Role Allocation
| Role | Count | Weight | Description |
|------|-------|--------|-------------|
| VALIDATOR | 20 | 1.0 | Verify outputs and decisions |
| EXPLORER | 8 | 0.8 | Discover new solutions |
| CRITIC | 6 | 1.2 | Challenge assumptions |
| INTEGRATOR | 4 | 1.0 | Combine partial solutions |
| HISTORIAN | 2 | 0.8 | Maintain context and memory |
| HUMAN (@0.A) | 1 | 1.0 | Human operator |

### Total Agent Capacity
- **Total Agents**: 41
- **Total Effective Weight**: 40.2
- **Human Weight Ratio**: 2.49%

### Role Weight Justification
| Role | Weight | Justification |
|------|--------|---------------|
| VALIDATOR | 1.0 | Baseline verification role |
| EXPLORER | 0.8 | Exploratory, may produce noise |
| CRITIC | 1.2 | Dissent is valuable (NAM-03) |
| INTEGRATOR | 1.0 | Balanced synthesis role |
| HISTORIAN | 0.8 | Support role, not decision-making |
| HUMAN | 1.0 | Equal peer status |

---

## Episodic Memory (NAM-06)

### Description
Records episodes for continuous learning and pattern recognition across sessions.

### Configuration
| Parameter | Value | Description |
|-----------|-------|-------------|
| Database | episodic_memory.db | SQLite database file |
| Retention | Configurable | Default: 90 days |
| Compression | LZ4 | Episode compression algorithm |
| Max Episodes | 1,000,000 | Per-database limit |

### Episode Schema
```sql
CREATE TABLE episodes (
    episode_id TEXT PRIMARY KEY,
    timestamp DATETIME NOT NULL,
    episode_type TEXT NOT NULL,
    context JSON NOT NULL,
    actions JSON NOT NULL,
    outcome JSON,
    reward REAL,
    metadata JSON,
    compressed BOOLEAN DEFAULT FALSE,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_episode_type ON episodes(episode_type);
CREATE INDEX idx_timestamp ON episodes(timestamp);
CREATE INDEX idx_reward ON episodes(reward);
```

### Episode Types
| Type | Description | Retention |
|------|-------------|-----------|
| DECISION | Consensus decision episode | 90 days |
| ERROR | Error recovery episode | 180 days |
| OPTIMIZATION | Performance optimization | 90 days |
| DISSENT | Dissent and outcome tracking | 365 days |
| HUMAN_INTERACTION | Human agent interactions | 365 days |

### Compliance Criteria
- [ ] Episode capture latency < 50ms
- [ ] Retrieval latency < 100ms for recent episodes
- [ ] Compression ratio > 3:1 for archived episodes

---

## Compliance Scoring

### Score Calculation
```
NAM_Score = (
    R1_Score * 0.20 +
    R2_Score * 0.18 +
    R3_Score * 0.17 +
    R4_Score * 0.20 +
    R5_Score * 0.25
) / 100
```

### Target Breakdown
| Requirement | Target | Weight | Weighted |
|-------------|--------|--------|----------|
| R1: SelfQuery | 90% | 0.20 | 18.0 |
| R2: HebbianRouting | 88% | 0.18 | 15.84 |
| R3: DissentCapture | 85% | 0.17 | 14.45 |
| R4: FieldVisualization | 90% | 0.20 | 18.0 |
| R5: HumanAsAgent | 95% | 0.25 | 23.75 |
| **Total** | | | **90.04%** |

### Current Status
Target: 92% (requires exceeding some component targets)

---

## Version History

| Version | Date | Changes |
|---------|------|---------|
| 1.0.0 | 2026-01-28 | Initial specification |
