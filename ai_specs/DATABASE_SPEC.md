# Database Specification

> Technical database specification for The Maintenance Engine v1.0.0

---

## 9 Databases Overview

| # | Database | Purpose | Size Est. |
|---|----------|---------|-----------|
| 1 | service_tracking.db | Service lifecycle | ~16KB |
| 2 | system_synergy.db | Cross-system integration | ~15KB |
| 3 | hebbian_pulse.db | Neural pathway learning | ~17KB |
| 4 | consensus_tracking.db | PBFT consensus | ~15KB |
| 5 | episodic_memory.db | Episode recording | ~12KB |
| 6 | tensor_memory.db | 12D tensor storage | ~9KB |
| 7 | performance_metrics.db | Performance/SLOs | ~12KB |
| 8 | flow_state.db | State machines | ~12KB |
| 9 | security_events.db | Security monitoring | ~16KB |

---

## 1. service_tracking.db

### Tables

| Table | Purpose | Key Columns |
|-------|---------|-------------|
| services | Service registry | id, name, port, tier, protocol |
| service_state | Current state | service_id, status, health_score |
| dependencies | Service deps | source_id, target_id, type |
| health_checks | Health history | service_id, status, latency_ms |

### Key Indexes
```sql
idx_services_port ON services(port)
idx_service_state_status ON service_state(status)
idx_health_timestamp ON health_checks(timestamp)
```

### Key Views
```sql
v_system_health       -- Aggregated health
v_service_overview    -- Service summary
v_dependency_graph    -- Dependency tree
```

---

## 2. system_synergy.db

### Tables

| Table | Purpose | Key Columns |
|-------|---------|-------------|
| system_connections | System links | source_system, target_system |
| synergy_scores | Synergy values | connection_id, synergy_score |
| integration_events | Integration log | event_type, systems, timestamp |

### Key Indexes
```sql
idx_connections_source ON system_connections(source_system)
idx_synergy_score ON synergy_scores(synergy_score)
```

---

## 3. hebbian_pulse.db

### Tables

| Table | Purpose | Key Columns |
|-------|---------|-------------|
| pathways | Neural pathways | id, source, target, weight |
| activations | Activation history | pathway_id, timestamp, success |
| weight_history | Weight changes | pathway_id, old_weight, new_weight |

### Key Indexes
```sql
idx_pathways_weight ON pathways(weight)
idx_pathways_source ON pathways(source)
idx_activations_pathway ON activations(pathway_id)
```

### STDP Columns
| Column | Type | Description |
|--------|------|-------------|
| weight | REAL | 0.0-1.0 |
| activation_count | INTEGER | Total activations |
| success_rate | REAL | Success ratio |
| last_activated | TEXT | ISO timestamp |

---

## 4. consensus_tracking.db

### Tables

| Table | Purpose | Key Columns |
|-------|---------|-------------|
| proposals | PBFT proposals | id, type, status, view |
| votes | Agent votes | proposal_id, agent_id, vote |
| views | View history | view_number, leader_id |
| checkpoints | State checkpoints | sequence, state_hash |

### PBFT Columns
| Column | Type | Description |
|--------|------|-------------|
| view | INTEGER | Current view number |
| sequence | INTEGER | Message sequence |
| phase | TEXT | pre-prepare/prepare/commit |
| votes_received | INTEGER | Vote count |

---

## 5. episodic_memory.db

### Tables

| Table | Purpose | Key Columns |
|-------|---------|-------------|
| episodes | Episode records | id, type, start_time, end_time |
| episode_events | Episode events | episode_id, event_type, data |
| episode_outcomes | Outcomes | episode_id, success, metrics |

### NAM-06 Compliance
- Full episode recording
- Event correlation
- Outcome tracking

---

## 6. tensor_memory.db

### Tables

| Table | Purpose | Key Columns |
|-------|---------|-------------|
| tensor_snapshots | 12D tensors | service_id, d0-d11, magnitude |
| tensor_operations | Operations | type, input_id, output_id |
| tensor_similarities | Similarity cache | a_id, b_id, cosine, euclidean |
| tensor_clusters | Clustering | cluster_id, centroid_d0-d11 |

### 12D Tensor Columns
```sql
d0_service_id INTEGER,
d1_port INTEGER,
d2_tier INTEGER,
d3_deps INTEGER,
d4_agents INTEGER,
d5_protocol INTEGER,
d6_health REAL,
d7_uptime INTEGER,
d8_synergy REAL,
d9_latency REAL,
d10_error_rate REAL,
d11_temporal REAL,
magnitude REAL GENERATED ALWAYS AS (sqrt(...))
```

---

## 7. performance_metrics.db

### Tables

| Table | Purpose | Key Columns |
|-------|---------|-------------|
| performance_samples | Metric samples | service_id, cpu, memory, latency |
| slo_definitions | SLO configs | metric, target, comparison |
| slo_violations | SLO breaches | slo_id, actual, target, severity |
| error_budgets | Error budgets | service_id, budget, consumed |

### Key SLO Columns
| Column | Type | Description |
|--------|------|-------------|
| target_value | REAL | SLO target |
| comparison | TEXT | lt/lte/gt/gte/eq |
| burn_rate | REAL | Budget burn rate |

---

## 8. flow_state.db

### Tables

| Table | Purpose | Key Columns |
|-------|---------|-------------|
| state_machines | Machine defs | id, current_state, service_id |
| state_definitions | State defs | machine_id, state_name, type |
| transition_definitions | Transitions | from_state, to_state, trigger |
| state_history | History | machine_id, from, to, timestamp |

### State Types
| Type | Description |
|------|-------------|
| initial | Starting state |
| normal | Regular state |
| terminal | End state |
| error | Error state |

---

## 9. security_events.db

### Tables

| Table | Purpose | Key Columns |
|-------|---------|-------------|
| security_events | Events | type, severity, source, action |
| security_alerts | Alerts | type, severity, status, title |
| access_audit | Access log | principal, resource, action |
| threat_indicators | Threats | type, value, severity |
| compliance_checks | Compliance | framework, requirement, status |

### Event Types
```sql
CHECK (event_type IN (
  'auth_success', 'auth_failure', 'auth_lockout',
  'access_granted', 'access_denied', 'privilege_escalation',
  'config_change', 'service_start', 'service_stop',
  'anomaly_detected', 'rate_limit', 'input_validation'
))
```

### Severity Levels
| Level | Value | Auto-Alert |
|-------|-------|------------|
| info | 0 | No |
| low | 1 | No |
| medium | 2 | Optional |
| high | 3 | Yes |
| critical | 4 | Immediate |

---

## Common Patterns

### Timestamp Format
All timestamps use ISO 8601: `datetime('now')`

### UUID Generation
```sql
lower(hex(randomblob(16)))
```

### Generated Columns
```sql
column_name TYPE GENERATED ALWAYS AS (expression) STORED
```

### Common Indexes
```sql
idx_{table}_timestamp ON {table}(timestamp)
idx_{table}_status ON {table}(status)
idx_{table}_service ON {table}(service_id)
```

---

*Generated: 2026-01-28 | The Maintenance Engine v1.0.0*
