# 12D Tensor Encoding Specification

**Version:** 1.1.0
**Related:** [STDP_SPEC.md](STDP_SPEC.md), [NAM_SPEC.md](NAM_SPEC.md), [DATABASE_SPEC.md](DATABASE_SPEC.md), [patterns/TENSOR_PATTERNS.md](patterns/TENSOR_PATTERNS.md)

---

## Overview

This specification defines the 12-dimensional tensor encoding system used for representing service state across the maintenance engine. Each dimension is normalized to the range [0, 1] to enable consistent mathematical operations.

---

## Dimension Table

| D# | Name | Range | Encoding | Description |
|----|------|-------|----------|-------------|
| D0 | service_id | 0-1 | hash(id) % 65536 / 65535 | Service identifier |
| D1 | port | 0-1 | port / 65535 | Port number |
| D2 | tier | 0-1 | tier / 5 | Service tier (1-5) |
| D3 | deps | 0-1 | log(deps+1) / log(MAX_DEPS) | Dependency count |
| D4 | agents | 0-1 | agents / 40 | Agent count |
| D5 | protocol | 0-1 | enum (REST=0, gRPC=0.33, WS=0.67, IPC=1) | Protocol |
| D6 | health | 0-1 | health_score | Health (0=dead, 1=perfect) |
| D7 | uptime | 0-1 | uptime_seconds / MAX_UPTIME | Uptime ratio |
| D8 | synergy | 0-1 | synergy_score | Cross-system synergy |
| D9 | latency | 0-1 | 1 - (latency_ms / 2000) | Latency (inverted) |
| D10 | error_rate | 0-1 | errors / requests | Error rate |
| D11 | temporal | 0-1 | time_encoding() | Temporal context |

---

## Dimension Details

### D0: Service ID
- **Purpose**: Unique identifier for the service
- **Encoding**: `hash(id) % 65536 / 65535`
- **Notes**: Uses consistent hashing to distribute services uniformly

### D1: Port
- **Purpose**: Network port binding
- **Encoding**: `port / 65535`
- **Notes**: Full port range normalization

### D2: Tier
- **Purpose**: Service criticality level
- **Encoding**: `tier / 5`
- **Values**:
  - Tier 1 (0.2): Critical infrastructure
  - Tier 2 (0.4): Core services
  - Tier 3 (0.6): Support services
  - Tier 4 (0.8): Optional services
  - Tier 5 (1.0): Development/test

### D3: Dependencies
- **Purpose**: Dependency complexity measure
- **Encoding**: `log(deps+1) / log(MAX_DEPS)`
- **Notes**: Logarithmic scaling prevents outlier dominance

### D4: Agents
- **Purpose**: Number of assigned agents
- **Encoding**: `agents / 40`
- **Notes**: Based on 40-agent swarm maximum

### D5: Protocol
- **Purpose**: Communication protocol type
- **Encoding**: Discrete enum mapping
- **Values**:
  - REST: 0.00
  - gRPC: 0.33
  - WebSocket: 0.67
  - IPC: 1.00

### D6: Health
- **Purpose**: Current health status
- **Encoding**: `health_score` (direct)
- **Range**: 0 = dead, 1 = perfect

### D7: Uptime
- **Purpose**: Service availability ratio
- **Encoding**: `uptime_seconds / MAX_UPTIME`
- **Notes**: MAX_UPTIME typically 30 days in seconds

### D8: Synergy
- **Purpose**: Cross-system integration score
- **Encoding**: `synergy_score` (direct)
- **Notes**: Measures inter-service cooperation efficiency

### D9: Latency
- **Purpose**: Response time indicator
- **Encoding**: `1 - (latency_ms / 2000)`
- **Notes**: Inverted so lower latency = higher value

### D10: Error Rate
- **Purpose**: Failure frequency
- **Encoding**: `errors / requests`
- **Notes**: Direct ratio, lower is better

### D11: Temporal
- **Purpose**: Time-based context encoding
- **Encoding**: `time_encoding()`
- **Notes**: Captures cyclical patterns (time of day, day of week)

---

## Tensor Operations

| Operation | Formula | Purpose |
|-----------|---------|---------|
| Magnitude | `sqrt(sum(d^2))` | Overall state intensity |
| Distance | `sqrt(sum((a-b)^2))` | State difference |
| Cosine Similarity | `dot(a,b)/(|a|*|b|)` | State similarity |
| Normalize | `d/|d|` | Unit tensor |

### Operation Details

#### Magnitude
```
|T| = sqrt(D0^2 + D1^2 + ... + D11^2)
```
Maximum magnitude: `sqrt(12) ≈ 3.464`

#### Euclidean Distance
```
dist(A, B) = sqrt(sum((Ai - Bi)^2) for i in 0..11)
```
Maximum distance: `sqrt(12) ≈ 3.464`

#### Cosine Similarity
```
cos(A, B) = (A · B) / (|A| * |B|)
```
Range: [-1, 1], where 1 = identical direction

#### Normalization
```
normalize(T) = T / |T|
```
Result: Unit tensor with magnitude 1

---

## Rust Implementation

```rust
use std::hash::{Hash, Hasher};
use std::collections::hash_map::DefaultHasher;

/// 12-dimensional tensor for service state encoding
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tensor12D {
    pub service_id: f64,   // D0
    pub port: f64,         // D1
    pub tier: f64,         // D2
    pub deps: f64,         // D3
    pub agents: f64,       // D4
    pub protocol: f64,     // D5
    pub health: f64,       // D6
    pub uptime: f64,       // D7
    pub synergy: f64,      // D8
    pub latency: f64,      // D9
    pub error_rate: f64,   // D10
    pub temporal: f64,     // D11
}

impl Tensor12D {
    /// Create a new tensor from an array of 12 dimensions
    pub const fn new(dimensions: [f64; 12]) -> Self {
        Self {
            service_id: dimensions[0],
            port: dimensions[1],
            tier: dimensions[2],
            deps: dimensions[3],
            agents: dimensions[4],
            protocol: dimensions[5],
            health: dimensions[6],
            uptime: dimensions[7],
            synergy: dimensions[8],
            latency: dimensions[9],
            error_rate: dimensions[10],
            temporal: dimensions[11],
        }
    }

    /// Convert tensor to 96-byte binary representation
    pub fn to_bytes(&self) -> [u8; 96] {
        let mut bytes = [0u8; 96];
        let dims = self.as_array();
        for (i, &dim) in dims.iter().enumerate() {
            let dim_bytes = dim.to_le_bytes();
            bytes[i * 8..(i + 1) * 8].copy_from_slice(&dim_bytes);
        }
        bytes
    }

    /// Create tensor from 96-byte binary representation
    pub fn from_bytes(bytes: &[u8; 96]) -> Self {
        let mut dims = [0.0f64; 12];
        for i in 0..12 {
            let mut dim_bytes = [0u8; 8];
            dim_bytes.copy_from_slice(&bytes[i * 8..(i + 1) * 8]);
            dims[i] = f64::from_le_bytes(dim_bytes);
        }
        Self::new(dims)
    }

    /// Calculate Euclidean distance to another tensor
    pub fn distance(&self, other: &Self) -> f64 {
        let a = self.as_array();
        let b = other.as_array();
        a.iter()
            .zip(b.iter())
            .map(|(x, y)| (x - y).powi(2))
            .sum::<f64>()
            .sqrt()
    }

    /// Calculate magnitude (L2 norm)
    pub fn magnitude(&self) -> f64 {
        self.as_array()
            .iter()
            .map(|x| x.powi(2))
            .sum::<f64>()
            .sqrt()
    }

    /// Calculate cosine similarity with another tensor
    pub fn cosine_similarity(&self, other: &Self) -> f64 {
        let a = self.as_array();
        let b = other.as_array();
        let dot: f64 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let mag_a = self.magnitude();
        let mag_b = other.magnitude();
        if mag_a == 0.0 || mag_b == 0.0 {
            0.0
        } else {
            dot / (mag_a * mag_b)
        }
    }

    /// Normalize to unit tensor
    pub fn normalize(&self) -> Self {
        let mag = self.magnitude();
        if mag == 0.0 {
            return *self;
        }
        let dims: [f64; 12] = self.as_array().map(|d| d / mag);
        Self::new(dims)
    }

    /// Validate all dimensions are in [0, 1] range
    pub fn validate(&self) -> Result<(), TensorError> {
        for (i, &dim) in self.as_array().iter().enumerate() {
            if dim < 0.0 || dim > 1.0 {
                return Err(TensorError::OutOfRange {
                    dimension: i,
                    value: dim,
                });
            }
            if dim.is_nan() || dim.is_infinite() {
                return Err(TensorError::InvalidValue {
                    dimension: i,
                    value: dim,
                });
            }
        }
        Ok(())
    }

    /// Convert to array representation
    pub fn as_array(&self) -> [f64; 12] {
        [
            self.service_id,
            self.port,
            self.tier,
            self.deps,
            self.agents,
            self.protocol,
            self.health,
            self.uptime,
            self.synergy,
            self.latency,
            self.error_rate,
            self.temporal,
        ]
    }
}

/// Errors that can occur during tensor operations
#[derive(Debug, Clone)]
pub enum TensorError {
    OutOfRange { dimension: usize, value: f64 },
    InvalidValue { dimension: usize, value: f64 },
}

/// Protocol encoding helper
pub enum Protocol {
    Rest,
    Grpc,
    WebSocket,
    Ipc,
}

impl Protocol {
    pub fn encode(&self) -> f64 {
        match self {
            Protocol::Rest => 0.0,
            Protocol::Grpc => 0.33,
            Protocol::WebSocket => 0.67,
            Protocol::Ipc => 1.0,
        }
    }
}

/// Service ID encoding helper
pub fn encode_service_id(id: &str) -> f64 {
    let mut hasher = DefaultHasher::new();
    id.hash(&mut hasher);
    let hash = hasher.finish();
    ((hash % 65536) as f64) / 65535.0
}

/// Temporal encoding helper (example: time of day)
pub fn encode_temporal(hour: u32, minute: u32) -> f64 {
    let minutes_of_day = hour * 60 + minute;
    (minutes_of_day as f64) / 1440.0  // 24 * 60 = 1440 minutes in a day
}
```

---

## Storage Format

### Binary Format (96 bytes)
```
Offset  Size  Type   Field
0       8     f64    D0 (service_id)
8       8     f64    D1 (port)
16      8     f64    D2 (tier)
24      8     f64    D3 (deps)
32      8     f64    D4 (agents)
40      8     f64    D5 (protocol)
48      8     f64    D6 (health)
56      8     f64    D7 (uptime)
64      8     f64    D8 (synergy)
72      8     f64    D9 (latency)
80      8     f64    D10 (error_rate)
88      8     f64    D11 (temporal)
```

### JSON Format
```json
{
  "d0": 0.5,
  "d1": 0.3,
  "d2": 0.4,
  "d3": 0.2,
  "d4": 0.75,
  "d5": 0.33,
  "d6": 0.95,
  "d7": 0.99,
  "d8": 0.87,
  "d9": 0.92,
  "d10": 0.01,
  "d11": 0.5
}
```

### Named JSON Format (alternative)
```json
{
  "service_id": 0.5,
  "port": 0.3,
  "tier": 0.4,
  "deps": 0.2,
  "agents": 0.75,
  "protocol": 0.33,
  "health": 0.95,
  "uptime": 0.99,
  "synergy": 0.87,
  "latency": 0.92,
  "error_rate": 0.01,
  "temporal": 0.5
}
```

---

## Constants

```rust
pub const TENSOR_DIMENSIONS: usize = 12;
pub const TENSOR_BYTE_SIZE: usize = 96;
pub const MAX_DEPS: u32 = 100;
pub const MAX_AGENTS: u32 = 40;
pub const MAX_UPTIME_SECONDS: u64 = 2_592_000; // 30 days
pub const MAX_LATENCY_MS: u32 = 2000;
pub const MAX_PORT: u16 = 65535;
pub const MAX_TIER: u8 = 5;
```

---

## Cross-Spec Dependencies

| Dependency | Source Spec | Integration Point |
|------------|-------------|-------------------|
| D6 health target | STDP_SPEC | Homeostatic r_target (0.85) |
| D8 synergy target | STDP_SPEC | Homeostatic c_target (0.90) |
| NAM R4 Visualization | NAM_SPEC | Field visualization format |
| Database storage | DATABASE_SPEC | tensor_memory.db |
| Pipeline encoding | PIPELINE_SPEC | PL-TENSOR-001 |
| Tensor patterns | patterns/TENSOR_PATTERNS.md | Implementation patterns |

---

## Usage in Other Systems

| System | Tensor Usage |
|--------|-------------|
| Health Monitoring | D6, D7, D10 for service health |
| Hebbian Learning | D6, D8 for homeostatic feedback |
| NAM Compliance | D11 for NAM score tracking |
| Escalation | D10 for error rate thresholds |
| Service Discovery | D0, D1, D2 for service identification |

---

## Version History

| Version | Date | Changes |
|---------|------|---------|
| 1.1.0 | 2026-01-28 | Added cross-references, usage documentation |
| 1.0.0 | 2026-01-28 | Initial specification |
