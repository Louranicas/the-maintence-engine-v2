# Module M01: Error Taxonomy

> **M01_ERROR_TAXONOMY** | 11D Tensor Encoding | Layer: L1 Foundation | [Back to Index](../INDEX.md)

---

## Navigation

| Direction | Link |
|-----------|------|
| Up | [INDEX.md](../INDEX.md) |
| Layer | [L01_FOUNDATION.md](../layers/L01_FOUNDATION.md) |
| Related | [L03_LEARNING.md](../layers/L03_LEARNING.md) |
| Pattern | [PATTERN_001_CIRCUIT_BREAKER.md](../patterns/PATTERN_001_CIRCUIT_BREAKER.md) |

---

## Module Specification

### Overview

The Error Taxonomy module provides a comprehensive 11-dimensional tensor encoding system for error classification. This enables semantic error analysis, similarity computation, and machine learning-based pattern recognition across the Maintenance Engine.

### Module Properties

| Property | Value |
|----------|-------|
| Module ID | M01 |
| Module Name | Error Taxonomy |
| Layer | L1 (Foundation) |
| Version | 2.0 |
| Dependencies | None |
| Dependents | M3 (Logging), L3 (Learning), L5 (Remediation) |

---

## 11D Tensor Encoding

### Dimension Overview

```
Error Vector: [D1, D2, D3, D4, D5, D6, D7, D8, D9, D10, D11]
              │   │   │   │   │   │   │   │   │   │    │
              │   │   │   │   │   │   │   │   │   │    └─ D11: Recovery Complexity
              │   │   │   │   │   │   │   │   │   └──── D10: Data Impact
              │   │   │   │   │   │   │   │   └──────── D9: Security Implication
              │   │   │   │   │   │   │   └────────── D8: User Impact
              │   │   │   │   │   │   └──────────── D7: Blast Radius
              │   │   │   │   │   └────────────── D6: Recurrence Pattern
              │   │   │   │   └──────────────── D5: Temporal Pattern
              │   │   │   └────────────────── D4: Propagation Type
              │   │   └──────────────────── D3: Severity Level
              │   └────────────────────── D2: Component Type
              └──────────────────────── D1: Error Category
```

### Dimension Specifications

#### D1: Error Category (0.0 - 1.0)

```rust
pub enum ErrorCategory {
    Infrastructure = 0,  // 0.0 - 0.2
    Application = 1,     // 0.2 - 0.4
    Data = 2,            // 0.4 - 0.6
    Security = 3,        // 0.6 - 0.8
    Performance = 4,     // 0.8 - 1.0
}

impl ErrorCategory {
    pub fn to_dimension(&self) -> f32 {
        match self {
            Self::Infrastructure => 0.1,
            Self::Application => 0.3,
            Self::Data => 0.5,
            Self::Security => 0.7,
            Self::Performance => 0.9,
        }
    }
}
```

#### D2: Component Type (0.0 - 1.0)

| Value Range | Component |
|-------------|-----------|
| 0.0 - 0.1 | Hardware |
| 0.1 - 0.2 | Network |
| 0.2 - 0.3 | Storage |
| 0.3 - 0.4 | Database |
| 0.4 - 0.5 | Cache |
| 0.5 - 0.6 | Queue |
| 0.6 - 0.7 | API |
| 0.7 - 0.8 | Service |
| 0.8 - 0.9 | Frontend |
| 0.9 - 1.0 | External |

#### D3: Severity Level (0.0 - 1.0)

```rust
pub enum Severity {
    Debug = 0,      // 0.0 - Information only
    Info = 1,       // 0.2 - Normal operation
    Warning = 2,    // 0.4 - Potential issue
    Error = 3,      // 0.6 - Service degradation
    Critical = 4,   // 0.8 - Service outage
    Fatal = 5,      // 1.0 - System failure
}
```

#### D4: Propagation Type (0.0 - 1.0)

| Value | Propagation | Description |
|-------|-------------|-------------|
| 0.0 | Isolated | Single component affected |
| 0.25 | Local | Same service affected |
| 0.5 | Regional | Same cluster affected |
| 0.75 | Cross-Service | Multiple services affected |
| 1.0 | Global | Entire system affected |

#### D5: Temporal Pattern (0.0 - 1.0)

| Value | Pattern | Description |
|-------|---------|-------------|
| 0.0 | One-time | Single occurrence |
| 0.2 | Sporadic | Occasional, no pattern |
| 0.4 | Periodic | Regular intervals |
| 0.6 | Bursty | Clusters of occurrences |
| 0.8 | Continuous | Ongoing |
| 1.0 | Escalating | Increasing frequency |

#### D6: Recurrence Pattern (0.0 - 1.0)

| Value | Recurrence | Description |
|-------|------------|-------------|
| 0.0 | First Time | Never seen before |
| 0.25 | Rare | Seen < 5 times |
| 0.5 | Occasional | Seen 5-20 times |
| 0.75 | Frequent | Seen 20-100 times |
| 1.0 | Chronic | Seen > 100 times |

#### D7: Blast Radius (0.0 - 1.0)

```rust
pub fn calculate_blast_radius(affected_services: usize, total_services: usize) -> f32 {
    (affected_services as f32 / total_services as f32).min(1.0)
}
```

#### D8: User Impact (0.0 - 1.0)

| Value | Impact | Description |
|-------|--------|-------------|
| 0.0 | None | No user impact |
| 0.2 | Minimal | < 1% users affected |
| 0.4 | Low | 1-10% users affected |
| 0.6 | Medium | 10-50% users affected |
| 0.8 | High | 50-90% users affected |
| 1.0 | Total | > 90% users affected |

#### D9: Security Implication (0.0 - 1.0)

| Value | Security | Description |
|-------|----------|-------------|
| 0.0 | None | No security impact |
| 0.2 | Low | Minor exposure |
| 0.4 | Medium | Potential data access |
| 0.6 | High | Data breach possible |
| 0.8 | Critical | Active exploitation |
| 1.0 | Catastrophic | Systemic compromise |

#### D10: Data Impact (0.0 - 1.0)

| Value | Data Impact | Description |
|-------|-------------|-------------|
| 0.0 | None | No data affected |
| 0.2 | Temporary | Transient data loss |
| 0.4 | Recoverable | Data recoverable from backup |
| 0.6 | Partial Loss | Some data unrecoverable |
| 0.8 | Major Loss | Significant data loss |
| 1.0 | Total Loss | Complete data loss |

#### D11: Recovery Complexity (0.0 - 1.0)

| Value | Complexity | Description |
|-------|------------|-------------|
| 0.0 | Automatic | Self-heals |
| 0.2 | Simple | Single action fix |
| 0.4 | Moderate | Multiple steps |
| 0.6 | Complex | Expert intervention |
| 0.8 | Major | Infrastructure changes |
| 1.0 | Critical | Full rebuild required |

---

## Error Vector API

### Core Structure

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct ErrorVector {
    /// 11-dimensional error encoding
    pub dimensions: [f32; 11],

    /// Source error information
    pub source: ErrorSource,

    /// Timestamp of error
    pub timestamp: DateTime<Utc>,

    /// Error code
    pub code: ErrorCode,

    /// Human-readable message
    pub message: String,

    /// Additional context
    pub context: HashMap<String, Value>,
}

impl ErrorVector {
    /// Create new error vector
    pub fn new(dimensions: [f32; 11], source: ErrorSource, message: String) -> Self;

    /// Calculate magnitude (overall severity score)
    pub fn magnitude(&self) -> f32 {
        self.dimensions.iter().map(|d| d * d).sum::<f32>().sqrt()
    }

    /// Calculate cosine similarity with another vector
    pub fn similarity(&self, other: &ErrorVector) -> f32 {
        let dot_product: f32 = self.dimensions.iter()
            .zip(other.dimensions.iter())
            .map(|(a, b)| a * b)
            .sum();
        dot_product / (self.magnitude() * other.magnitude())
    }

    /// Get primary category
    pub fn category(&self) -> ErrorCategory {
        ErrorCategory::from_dimension(self.dimensions[0])
    }

    /// Get severity level
    pub fn severity(&self) -> Severity {
        Severity::from_dimension(self.dimensions[2])
    }
}
```

### Error Taxonomy Service

```rust
pub struct ErrorTaxonomy {
    /// Encode an error into 11D vector
    pub fn encode(&self, error: &dyn Error, context: &ErrorContext) -> ErrorVector;

    /// Classify error vector into category
    pub fn classify(&self, vector: &ErrorVector) -> Classification;

    /// Find similar errors in history
    pub fn find_similar(&self, vector: &ErrorVector, threshold: f32) -> Vec<ErrorVector>;

    /// Cluster errors by similarity
    pub fn cluster(&self, vectors: &[ErrorVector]) -> Vec<ErrorCluster>;

    /// Get recommended remediation tier
    pub fn recommended_tier(&self, vector: &ErrorVector) -> EscalationTier;
}
```

### Encoding Example

```rust
// Example: Database connection timeout
let error_vector = ErrorVector {
    dimensions: [
        0.5,  // D1: Data (database-related)
        0.35, // D2: Database component
        0.6,  // D3: Error severity
        0.5,  // D4: Regional propagation
        0.4,  // D5: Periodic pattern
        0.5,  // D6: Occasional recurrence
        0.3,  // D7: 30% services affected
        0.4,  // D8: Low user impact
        0.0,  // D9: No security implication
        0.0,  // D10: No data impact
        0.2,  // D11: Simple recovery
    ],
    source: ErrorSource::Database("primary-db".to_string()),
    timestamp: Utc::now(),
    code: ErrorCode::new("E3042"),
    message: "Database connection timeout after 5000ms".to_string(),
    context: HashMap::new(),
};

// Calculate severity score
let magnitude = error_vector.magnitude(); // ~1.1

// Determine escalation tier
let tier = taxonomy.recommended_tier(&error_vector); // L1 Standard
```

---

## Error Categories

### Category Hierarchy

```
Error Categories
├── E1000-E1999: Infrastructure
│   ├── E1000-E1099: Hardware
│   ├── E1100-E1199: Network
│   ├── E1200-E1299: Storage
│   └── E1300-E1399: Virtualization
├── E2000-E2999: Application
│   ├── E2000-E2099: Process
│   ├── E2100-E2199: Memory
│   ├── E2200-E2299: Runtime
│   └── E2300-E2399: Dependency
├── E3000-E3999: Data
│   ├── E3000-E3099: Corruption
│   ├── E3100-E3199: Consistency
│   ├── E3200-E3299: Availability
│   └── E3300-E3399: Integrity
├── E4000-E4999: Security
│   ├── E4000-E4099: Authentication
│   ├── E4100-E4199: Authorization
│   ├── E4200-E4299: Encryption
│   └── E4300-E4399: Audit
└── E5000-E5999: Performance
    ├── E5000-E5099: Latency
    ├── E5100-E5199: Throughput
    ├── E5200-E5299: Resource
    └── E5300-E5399: Capacity
```

---

## Similarity Computation

### Cosine Similarity

```rust
impl ErrorTaxonomy {
    /// Calculate similarity between two error vectors
    pub fn similarity(&self, a: &ErrorVector, b: &ErrorVector) -> f64 {
        let dot_product: f64 = a.dimensions.iter()
            .zip(b.dimensions.iter())
            .map(|(x, y)| (*x as f64) * (*y as f64))
            .sum();

        let mag_a: f64 = a.dimensions.iter()
            .map(|x| (*x as f64).powi(2))
            .sum::<f64>()
            .sqrt();

        let mag_b: f64 = b.dimensions.iter()
            .map(|x| (*x as f64).powi(2))
            .sum::<f64>()
            .sqrt();

        if mag_a == 0.0 || mag_b == 0.0 {
            return 0.0;
        }

        dot_product / (mag_a * mag_b)
    }
}
```

### Weighted Similarity

```rust
/// Weighted similarity with dimension importance
pub fn weighted_similarity(
    a: &ErrorVector,
    b: &ErrorVector,
    weights: &[f32; 11],
) -> f64 {
    let weighted_dot: f64 = a.dimensions.iter()
        .zip(b.dimensions.iter())
        .zip(weights.iter())
        .map(|((x, y), w)| (*x as f64) * (*y as f64) * (*w as f64))
        .sum();

    // ... normalize by weighted magnitudes
}
```

---

---

## Bi-Directional Data Flow

### Inbound Data Flow

```
┌─────────────────────────────────────────────────────────────────────┐
│                     INBOUND DATA FLOW TO M01                         │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│   ┌─────────────┐     ┌─────────────┐     ┌─────────────┐          │
│   │   L2: M07   │────►│             │◄────│   L5: M25   │          │
│   │   Health    │     │     M01     │     │ Remediation │          │
│   │   Monitor   │     │    Error    │     │   Engine    │          │
│   └─────────────┘     │   Taxonomy  │     └─────────────┘          │
│                       │             │                               │
│   ┌─────────────┐     │             │     ┌─────────────┐          │
│   │   L6: M31   │────►│             │◄────│   L3: M15   │          │
│   │ API Gateway │     └─────────────┘     │  Pattern    │          │
│   │ (External)  │                         │ Recognition │          │
│   └─────────────┘                         └─────────────┘          │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

#### Inbound Message Types

```rust
/// Messages received by M01 Error Taxonomy
pub enum M01InboundMessage {
    /// From L2 M07: Raw error event requiring encoding
    EncodeRequest {
        error: Box<dyn std::error::Error + Send + Sync>,
        context: ErrorContext,
        source_service: ServiceId,
        timestamp: DateTime<Utc>,
    },

    /// From L5 M25: Request to classify error for remediation
    ClassifyRequest {
        error_id: ErrorId,
        vector: Option<ErrorVector>,
        urgency: UrgencyLevel,
    },

    /// From L3 M15: Similarity query for pattern matching
    SimilarityQuery {
        reference_vector: ErrorVector,
        threshold: f32,
        max_results: usize,
    },

    /// From L6 M31: External API request for error info
    ExternalLookup {
        error_code: ErrorCode,
        include_history: bool,
    },
}
```

#### Inbound Trigger Conditions

| Source | Trigger | Data Format | Frequency |
|--------|---------|-------------|-----------|
| M07 Health Monitor | Health check failure | Raw Error + Context | On error |
| M25 Remediation | Pre-remediation classification | ErrorId | Per action |
| M15 Pattern Recognition | Pattern matching | ErrorVector | On learning |
| M31 API Gateway | External query | ErrorCode | On request |

---

### Outbound Data Flow

```
┌─────────────────────────────────────────────────────────────────────┐
│                    OUTBOUND DATA FLOW FROM M01                       │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│                       ┌─────────────┐                               │
│                       │     M01     │                               │
│                       │    Error    │                               │
│                       │   Taxonomy  │                               │
│                       └──────┬──────┘                               │
│                              │                                      │
│         ┌────────────────────┼────────────────────┐                │
│         │                    │                    │                │
│         ▼                    ▼                    ▼                │
│   ┌─────────────┐     ┌─────────────┐     ┌─────────────┐         │
│   │   M03       │     │   M15/M16   │     │   M25       │         │
│   │  Logging    │     │  L3 Pattern │     │ Remediation │         │
│   │  System     │     │  & Cluster  │     │   Engine    │         │
│   └─────────────┘     └─────────────┘     └─────────────┘         │
│                                                                     │
│   ┌─────────────┐     ┌─────────────┐     ┌─────────────┐         │
│   │   M04       │     │   M14       │     │   M26       │         │
│   │  Metrics    │     │  Pathway    │     │ Escalation  │         │
│   │ Collector   │     │  Manager    │     │  Manager    │         │
│   └─────────────┘     └─────────────┘     └─────────────┘         │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

#### Outbound Message Types

```rust
/// Messages sent by M01 Error Taxonomy
pub enum M01OutboundMessage {
    /// To M03 Logging: Structured error log entry
    LogEntry {
        vector: ErrorVector,
        classification: Classification,
        log_level: LogLevel,
    },

    /// To M04 Metrics: Error metrics update
    MetricsUpdate {
        category: ErrorCategory,
        severity: Severity,
        dimensions: [f32; 11],
    },

    /// To M15/M16 L3: Encoded vector for learning
    EncodedVector {
        vector: ErrorVector,
        similarity_candidates: Vec<(ErrorVector, f32)>,
    },

    /// To M14 Pathway Manager: Activation signal
    PathwayActivation {
        error_vector: ErrorVector,
        recommended_pathways: Vec<PathwayId>,
        confidence_scores: Vec<f32>,
    },

    /// To M25 Remediation: Classification result
    ClassificationResult {
        error_id: ErrorId,
        vector: ErrorVector,
        recommended_tier: EscalationTier,
        similar_errors: Vec<SimilarError>,
    },

    /// To M26 Escalation: Severity assessment
    SeverityAssessment {
        error_id: ErrorId,
        magnitude: f32,
        blast_radius: f32,
        recommended_escalation: EscalationTier,
    },
}
```

#### Outbound Event Triggers

| Target | Trigger | Data Format | Latency Req |
|--------|---------|-------------|-------------|
| M03 Logging | Every error encoded | LogEntry | <5ms |
| M04 Metrics | Every error encoded | MetricsUpdate | <10ms |
| M15 Pattern | Similarity threshold met | EncodedVector | <50ms |
| M14 Pathway | Error matches pathway | PathwayActivation | <20ms |
| M25 Remediation | Classification complete | ClassificationResult | <30ms |
| M26 Escalation | High severity error | SeverityAssessment | <10ms |

---

### Cross-Module Dependencies

#### Bidirectional Dependency Matrix

```
             M01  M02  M03  M04  M05  M06  M07  M14  M15  M16  M25  M26
M01 Error     -   R    W    W    W    -    R    W    W    W    W    W
─────────────────────────────────────────────────────────────────────
R = Reads from (inbound)
W = Writes to (outbound)
- = No direct dependency
```

#### Synchronous vs Asynchronous Communication

| Communication | Type | Reason |
|---------------|------|--------|
| M01 → M03 Logging | Async | Non-blocking, fire-and-forget |
| M01 → M04 Metrics | Async | Batched, non-critical path |
| M01 → M15 Pattern | Async | Learning is background process |
| M01 → M25 Remediation | Sync | Classification needed before action |
| M07 → M01 Encode | Sync | Immediate encoding required |

#### Error Propagation Paths

```rust
/// Error propagation from M01 to dependents
pub enum M01ErrorPropagation {
    /// Encoding failure → M03 logs error, M04 increments failure counter
    EncodingFailure {
        original_error: Box<dyn std::error::Error>,
        fallback_vector: ErrorVector,  // Default high-severity vector
    },

    /// Classification timeout → M25 uses default tier, M26 escalates
    ClassificationTimeout {
        error_id: ErrorId,
        default_tier: EscalationTier,
    },

    /// Similarity search failure → M15 bypasses pattern matching
    SimilarityFailure {
        vector: ErrorVector,
        reason: String,
    },
}
```

---

### Contextual Flow

#### Data Transformation Pipeline

```
Raw Error Event
       │
       ▼
┌──────────────────┐
│  1. EXTRACTION   │  Extract error message, stack trace, context
└────────┬─────────┘
         │
         ▼
┌──────────────────┐
│  2. ANALYSIS     │  Determine category, severity, propagation
└────────┬─────────┘
         │
         ▼
┌──────────────────┐
│  3. ENCODING     │  Map to 11D tensor space
└────────┬─────────┘
         │
         ▼
┌──────────────────┐
│  4. ENRICHMENT   │  Add historical context, similar errors
└────────┬─────────┘
         │
         ▼
┌──────────────────┐
│  5. ROUTING      │  Dispatch to M03, M04, M15, M25, M26
└────────┬─────────┘
         │
         ▼
   ErrorVector + Classification
```

#### State Machine

```rust
pub enum M01State {
    /// Ready to receive errors
    Idle,

    /// Processing error encoding
    Encoding {
        error_id: ErrorId,
        started_at: Instant,
    },

    /// Searching for similar errors
    Searching {
        error_id: ErrorId,
        vector: ErrorVector,
    },

    /// Classifying and routing
    Classifying {
        error_id: ErrorId,
        vector: ErrorVector,
        candidates: Vec<SimilarError>,
    },

    /// Error condition
    Error {
        reason: String,
        recoverable: bool,
    },
}

impl M01State {
    pub fn transition(&self, event: M01Event) -> Self {
        match (self, event) {
            (Idle, M01Event::EncodeRequest(req)) => Encoding {
                error_id: req.id,
                started_at: Instant::now(),
            },
            (Encoding { .. }, M01Event::EncodingComplete(vector)) => Searching {
                error_id: vector.id,
                vector,
            },
            (Searching { .. }, M01Event::SearchComplete(candidates)) => Classifying {
                error_id: self.error_id(),
                vector: self.vector(),
                candidates,
            },
            (Classifying { .. }, M01Event::ClassificationComplete) => Idle,
            (_, M01Event::Error(reason)) => Error {
                reason,
                recoverable: true,
            },
            _ => self.clone(),
        }
    }
}
```

#### Data Lifecycle Within Module

1. **Creation**: ErrorVector created from raw error + context
2. **Validation**: Dimensions clamped to [0.0, 1.0]
3. **Caching**: Vector cached with TTL for similarity queries
4. **Distribution**: Dispatched to multiple downstream modules
5. **Archival**: Persisted to M05 State Persistence for history
6. **Expiration**: Old vectors pruned from cache after TTL

---

## Integration with Learning Layer

The Error Taxonomy module integrates with L3 (Learning) to:

1. **Pattern Recognition**: Cluster similar errors for pathway learning
2. **Pathway Activation**: Map error vectors to remediation pathways
3. **Feedback Loop**: Update patterns based on remediation outcomes

```rust
// L3 uses error vectors to activate pathways
impl LearningLayer {
    pub fn find_pathway(&self, error: &ErrorVector) -> Option<Pathway> {
        let similar_patterns = self.taxonomy.find_similar(error, 0.85);

        similar_patterns.iter()
            .filter_map(|pattern| self.pathways.get_by_source(pattern))
            .max_by(|a, b| a.weight.partial_cmp(&b.weight).unwrap())
    }
}
```

---

## Metrics

| Metric | Type | Description |
|--------|------|-------------|
| `me_m01_errors_encoded` | Counter | Total errors encoded |
| `me_m01_encoding_duration_ms` | Histogram | Encoding latency |
| `me_m01_similarity_queries` | Counter | Similarity computations |
| `me_m01_clusters_total` | Gauge | Active error clusters |

---

## Configuration

```toml
[layer.L1.M1]
version = "2.0"
tensor_dimensions = 11
similarity_threshold = 0.85
clustering_algorithm = "dbscan"
cluster_min_samples = 5
cluster_epsilon = 0.1
cache_enabled = true
cache_size = 10000
```

---

## Navigation

| Direction | Link |
|-----------|------|
| Up | [INDEX.md](../INDEX.md) |
| Layer | [L01_FOUNDATION.md](../layers/L01_FOUNDATION.md) |
| Related | [L03_LEARNING.md](../layers/L03_LEARNING.md) |
| Pattern | [PATTERN_001_CIRCUIT_BREAKER.md](../patterns/PATTERN_001_CIRCUIT_BREAKER.md) |
| API | [REST_API.md](../integration/REST_API.md) |

---

*[Back to Index](../INDEX.md) | [Layer: L01 Foundation](../layers/L01_FOUNDATION.md)*
