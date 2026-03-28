# Tensor Encoding Patterns Reference

> 12D Tensor Patterns for Claude Code & CodeSynthor V7
> Derived from SYNTHEX, The Maintenance Engine, and CodeSynthor V7

---

## Overview

| Metric | Value |
|--------|-------|
| **Patterns** | 8 |
| **Priority** | P1 |
| **Dimensions** | 12 (TME), 11 (CodeSynthor V7) |

---

## Pattern 1: 12D Tensor Definition (P0)

```rust
/// 12-dimensional tensor for service state encoding.
/// Each dimension normalized to [0.0, 1.0] for consistent distance calculations.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Tensor12D {
    /// D0: Service identifier (hashed to [0,1])
    pub d0_service_id: f64,

    /// D1: Port number (normalized: port / 65535)
    pub d1_port: f64,

    /// D2: Layer/tier (normalized: tier / 6)
    pub d2_tier: f64,

    /// D3: Dependency count (normalized: deps / max_deps)
    pub d3_dependencies: f64,

    /// D4: Active agent count (normalized: agents / max_agents)
    pub d4_agents: f64,

    /// D5: Protocol type (encoded: 0=http, 0.33=grpc, 0.67=tcp, 1=custom)
    pub d5_protocol: f64,

    /// D6: Health score [0,1] direct
    pub d6_health: f64,

    /// D7: Uptime ratio [0,1] direct
    pub d7_uptime: f64,

    /// D8: Synergy score [0,1] direct
    pub d8_synergy: f64,

    /// D9: Latency (normalized: 1 - min(latency_ms / 1000, 1))
    pub d9_latency: f64,

    /// D10: Error rate (inverted: 1 - error_rate)
    pub d10_error_rate: f64,

    /// D11: Temporal decay (time since last update)
    pub d11_temporal: f64,
}

impl Tensor12D {
    /// Number of dimensions
    pub const DIMENSIONS: usize = 12;

    /// Create tensor with all zeros
    pub const fn zero() -> Self {
        Self {
            d0_service_id: 0.0,
            d1_port: 0.0,
            d2_tier: 0.0,
            d3_dependencies: 0.0,
            d4_agents: 0.0,
            d5_protocol: 0.0,
            d6_health: 0.0,
            d7_uptime: 0.0,
            d8_synergy: 0.0,
            d9_latency: 0.0,
            d10_error_rate: 0.0,
            d11_temporal: 0.0,
        }
    }

    /// Create tensor with all ones (maximum state)
    pub const fn one() -> Self {
        Self {
            d0_service_id: 1.0,
            d1_port: 1.0,
            d2_tier: 1.0,
            d3_dependencies: 1.0,
            d4_agents: 1.0,
            d5_protocol: 1.0,
            d6_health: 1.0,
            d7_uptime: 1.0,
            d8_synergy: 1.0,
            d9_latency: 1.0,
            d10_error_rate: 1.0,
            d11_temporal: 1.0,
        }
    }
}
```

**Why**: Fixed-dimension tensors enable efficient vector operations and similarity calculations.

---

## Pattern 2: Validation (P0)

```rust
impl Tensor12D {
    /// Validate all dimensions are within [0.0, 1.0]
    pub fn validate(&self) -> Result<()> {
        let dims = self.as_array();
        for (i, &value) in dims.iter().enumerate() {
            if !value.is_finite() {
                return Err(Error::Validation(format!(
                    "Dimension {i} is not finite: {value}"
                )));
            }
            if !(0.0..=1.0).contains(&value) {
                return Err(Error::Validation(format!(
                    "Dimension {i} out of bounds: {value} (expected [0.0, 1.0])"
                )));
            }
        }
        Ok(())
    }

    /// Clamp all values to valid range [0.0, 1.0]
    pub fn clamp(&mut self) {
        let dims = self.as_array_mut();
        for value in dims.iter_mut() {
            *value = value.clamp(0.0, 1.0);
        }
    }

    /// Check if tensor has any NaN or infinite values
    pub fn is_valid(&self) -> bool {
        self.as_array().iter().all(|&v| v.is_finite() && (0.0..=1.0).contains(&v))
    }

    /// Convert to array for iteration
    pub fn as_array(&self) -> [f64; 12] {
        [
            self.d0_service_id,
            self.d1_port,
            self.d2_tier,
            self.d3_dependencies,
            self.d4_agents,
            self.d5_protocol,
            self.d6_health,
            self.d7_uptime,
            self.d8_synergy,
            self.d9_latency,
            self.d10_error_rate,
            self.d11_temporal,
        ]
    }

    fn as_array_mut(&mut self) -> [&mut f64; 12] {
        [
            &mut self.d0_service_id,
            &mut self.d1_port,
            &mut self.d2_tier,
            &mut self.d3_dependencies,
            &mut self.d4_agents,
            &mut self.d5_protocol,
            &mut self.d6_health,
            &mut self.d7_uptime,
            &mut self.d8_synergy,
            &mut self.d9_latency,
            &mut self.d10_error_rate,
            &mut self.d11_temporal,
        ]
    }
}
```

**Why**: Validation prevents invalid states from propagating through the system.

---

## Pattern 3: Distance Metrics (P1)

```rust
impl Tensor12D {
    /// Euclidean distance between two tensors
    pub fn euclidean_distance(&self, other: &Self) -> f64 {
        let a = self.as_array();
        let b = other.as_array();

        let sum_sq: f64 = a.iter()
            .zip(b.iter())
            .map(|(x, y)| (x - y).powi(2))
            .sum();

        sum_sq.sqrt()
    }

    /// Manhattan distance (L1 norm)
    pub fn manhattan_distance(&self, other: &Self) -> f64 {
        let a = self.as_array();
        let b = other.as_array();

        a.iter()
            .zip(b.iter())
            .map(|(x, y)| (x - y).abs())
            .sum()
    }

    /// Cosine similarity [-1, 1] where 1 = identical direction
    pub fn cosine_similarity(&self, other: &Self) -> f64 {
        let a = self.as_array();
        let b = other.as_array();

        let dot: f64 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let mag_a: f64 = a.iter().map(|x| x.powi(2)).sum::<f64>().sqrt();
        let mag_b: f64 = b.iter().map(|x| x.powi(2)).sum::<f64>().sqrt();

        if mag_a == 0.0 || mag_b == 0.0 {
            return 0.0;
        }

        dot / (mag_a * mag_b)
    }

    /// Weighted distance with per-dimension weights
    pub fn weighted_distance(&self, other: &Self, weights: &[f64; 12]) -> f64 {
        let a = self.as_array();
        let b = other.as_array();

        let sum_sq: f64 = a.iter()
            .zip(b.iter())
            .zip(weights.iter())
            .map(|((x, y), w)| w * (x - y).powi(2))
            .sum();

        sum_sq.sqrt()
    }
}

/// Default weights emphasizing health-related dimensions
pub const DEFAULT_WEIGHTS: [f64; 12] = [
    0.5,  // service_id - low weight
    0.3,  // port - low weight
    0.8,  // tier - medium weight
    0.7,  // dependencies - medium weight
    0.6,  // agents - medium weight
    0.4,  // protocol - low weight
    1.0,  // health - HIGH weight
    0.9,  // uptime - high weight
    0.9,  // synergy - high weight
    0.8,  // latency - medium-high weight
    0.9,  // error_rate - high weight
    0.7,  // temporal - medium weight
];
```

**Why**: Multiple distance metrics support different similarity use cases.

---

## Pattern 4: Tensor Operations (P1)

```rust
use std::ops::{Add, Sub, Mul};

impl Add for Tensor12D {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self {
            d0_service_id: self.d0_service_id + other.d0_service_id,
            d1_port: self.d1_port + other.d1_port,
            d2_tier: self.d2_tier + other.d2_tier,
            d3_dependencies: self.d3_dependencies + other.d3_dependencies,
            d4_agents: self.d4_agents + other.d4_agents,
            d5_protocol: self.d5_protocol + other.d5_protocol,
            d6_health: self.d6_health + other.d6_health,
            d7_uptime: self.d7_uptime + other.d7_uptime,
            d8_synergy: self.d8_synergy + other.d8_synergy,
            d9_latency: self.d9_latency + other.d9_latency,
            d10_error_rate: self.d10_error_rate + other.d10_error_rate,
            d11_temporal: self.d11_temporal + other.d11_temporal,
        }
    }
}

impl Sub for Tensor12D {
    type Output = Self;

    fn sub(self, other: Self) -> Self {
        Self {
            d0_service_id: self.d0_service_id - other.d0_service_id,
            d1_port: self.d1_port - other.d1_port,
            d2_tier: self.d2_tier - other.d2_tier,
            d3_dependencies: self.d3_dependencies - other.d3_dependencies,
            d4_agents: self.d4_agents - other.d4_agents,
            d5_protocol: self.d5_protocol - other.d5_protocol,
            d6_health: self.d6_health - other.d6_health,
            d7_uptime: self.d7_uptime - other.d7_uptime,
            d8_synergy: self.d8_synergy - other.d8_synergy,
            d9_latency: self.d9_latency - other.d9_latency,
            d10_error_rate: self.d10_error_rate - other.d10_error_rate,
            d11_temporal: self.d11_temporal - other.d11_temporal,
        }
    }
}

impl Mul<f64> for Tensor12D {
    type Output = Self;

    fn mul(self, scalar: f64) -> Self {
        Self {
            d0_service_id: self.d0_service_id * scalar,
            d1_port: self.d1_port * scalar,
            d2_tier: self.d2_tier * scalar,
            d3_dependencies: self.d3_dependencies * scalar,
            d4_agents: self.d4_agents * scalar,
            d5_protocol: self.d5_protocol * scalar,
            d6_health: self.d6_health * scalar,
            d7_uptime: self.d7_uptime * scalar,
            d8_synergy: self.d8_synergy * scalar,
            d9_latency: self.d9_latency * scalar,
            d10_error_rate: self.d10_error_rate * scalar,
            d11_temporal: self.d11_temporal * scalar,
        }
    }
}

impl Tensor12D {
    /// Linear interpolation between two tensors
    pub fn lerp(&self, other: &Self, t: f64) -> Self {
        let t = t.clamp(0.0, 1.0);
        *self * (1.0 - t) + *other * t
    }

    /// Magnitude (L2 norm) of the tensor
    pub fn magnitude(&self) -> f64 {
        self.as_array().iter().map(|x| x.powi(2)).sum::<f64>().sqrt()
    }

    /// Normalize to unit vector
    pub fn normalize(&self) -> Self {
        let mag = self.magnitude();
        if mag == 0.0 {
            return Self::zero();
        }
        *self * (1.0 / mag)
    }

    /// Dot product
    pub fn dot(&self, other: &Self) -> f64 {
        let a = self.as_array();
        let b = other.as_array();
        a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
    }
}
```

**Why**: Standard operations enable composable tensor manipulation.

---

## Pattern 5: Encoding/Decoding (P1)

```rust
use crate::l1_foundation::m05_types::Service;

impl Tensor12D {
    /// Encode a service state into tensor representation
    pub fn from_service(service: &Service, max_deps: u32, max_agents: u32) -> Self {
        let mut tensor = Self {
            d0_service_id: Self::hash_to_unit(&service.id),
            d1_port: service.port as f64 / 65535.0,
            d2_tier: service.tier as f64 / 6.0,
            d3_dependencies: service.dependencies.len() as f64 / max_deps as f64,
            d4_agents: service.active_agents as f64 / max_agents as f64,
            d5_protocol: Self::protocol_to_unit(&service.protocol),
            d6_health: service.health_score,
            d7_uptime: service.uptime_ratio,
            d8_synergy: service.synergy_score,
            d9_latency: 1.0 - (service.latency_ms as f64 / 1000.0).min(1.0),
            d10_error_rate: 1.0 - service.error_rate,
            d11_temporal: Self::time_decay(service.last_updated),
        };
        tensor.clamp();
        tensor
    }

    /// Hash string to [0,1] range
    fn hash_to_unit(s: &str) -> f64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        s.hash(&mut hasher);
        let hash = hasher.finish();
        (hash as f64) / (u64::MAX as f64)
    }

    /// Encode protocol to [0,1]
    fn protocol_to_unit(protocol: &str) -> f64 {
        match protocol.to_lowercase().as_str() {
            "http" | "https" => 0.0,
            "grpc" => 0.33,
            "tcp" => 0.67,
            _ => 1.0,
        }
    }

    /// Calculate temporal decay based on last update time
    fn time_decay(last_updated: DateTime<Utc>) -> f64 {
        let age = Utc::now() - last_updated;
        let hours = age.num_hours() as f64;

        // Exponential decay: 1.0 at 0 hours, ~0.37 at 24 hours, ~0.14 at 48 hours
        (-hours / 24.0).exp()
    }

    /// Extract health-related dimensions as a sub-tensor
    pub fn health_subset(&self) -> [f64; 5] {
        [
            self.d6_health,
            self.d7_uptime,
            self.d8_synergy,
            self.d9_latency,
            self.d10_error_rate,
        ]
    }
}
```

**Why**: Encoding transforms domain objects to tensor space consistently.

---

## Pattern 6: Tensor Storage (P1)

```rust
use rusqlite::{ToSql, types::{FromSql, ValueRef}};

impl ToSql for Tensor12D {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
        // Store as JSON array for readability and portability
        let array = self.as_array();
        let json = serde_json::to_string(&array)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        Ok(rusqlite::types::ToSqlOutput::Owned(
            rusqlite::types::Value::Text(json)
        ))
    }
}

impl FromSql for Tensor12D {
    fn column_result(value: ValueRef<'_>) -> rusqlite::types::FromSqlResult<Self> {
        match value {
            ValueRef::Text(bytes) => {
                let s = std::str::from_utf8(bytes)
                    .map_err(|e| rusqlite::types::FromSqlError::Other(Box::new(e)))?;
                let array: [f64; 12] = serde_json::from_str(s)
                    .map_err(|e| rusqlite::types::FromSqlError::Other(Box::new(e)))?;
                Ok(Self::from_array(array))
            }
            ValueRef::Blob(bytes) => {
                // Binary format: 12 * 8 bytes = 96 bytes
                if bytes.len() != 96 {
                    return Err(rusqlite::types::FromSqlError::InvalidBlobSize {
                        expected_size: 96,
                        blob_size: bytes.len(),
                    });
                }
                let mut array = [0.0f64; 12];
                for (i, chunk) in bytes.chunks_exact(8).enumerate() {
                    array[i] = f64::from_le_bytes(chunk.try_into().unwrap());
                }
                Ok(Self::from_array(array))
            }
            _ => Err(rusqlite::types::FromSqlError::InvalidType),
        }
    }
}

impl Tensor12D {
    /// Create from array
    pub fn from_array(array: [f64; 12]) -> Self {
        Self {
            d0_service_id: array[0],
            d1_port: array[1],
            d2_tier: array[2],
            d3_dependencies: array[3],
            d4_agents: array[4],
            d5_protocol: array[5],
            d6_health: array[6],
            d7_uptime: array[7],
            d8_synergy: array[8],
            d9_latency: array[9],
            d10_error_rate: array[10],
            d11_temporal: array[11],
        }
    }

    /// Convert to compact binary representation (96 bytes)
    pub fn to_bytes(&self) -> [u8; 96] {
        let mut bytes = [0u8; 96];
        for (i, &value) in self.as_array().iter().enumerate() {
            let value_bytes = value.to_le_bytes();
            bytes[i * 8..(i + 1) * 8].copy_from_slice(&value_bytes);
        }
        bytes
    }

    /// Create from binary representation
    pub fn from_bytes(bytes: &[u8; 96]) -> Self {
        let mut array = [0.0f64; 12];
        for (i, chunk) in bytes.chunks_exact(8).enumerate() {
            array[i] = f64::from_le_bytes(chunk.try_into().unwrap());
        }
        Self::from_array(array)
    }
}
```

**Why**: Efficient storage supports high-volume tensor persistence.

---

## Pattern 7: Tensor Index (P2)

```rust
use std::collections::BinaryHeap;
use std::cmp::Ordering;

/// K-nearest neighbor search structure
pub struct TensorIndex {
    tensors: Vec<(String, Tensor12D)>,  // (id, tensor)
}

#[derive(Clone)]
struct ScoredTensor {
    id: String,
    tensor: Tensor12D,
    distance: f64,
}

impl PartialEq for ScoredTensor {
    fn eq(&self, other: &Self) -> bool {
        self.distance == other.distance
    }
}

impl Eq for ScoredTensor {}

impl PartialOrd for ScoredTensor {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ScoredTensor {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reverse order for min-heap behavior
        other.distance.partial_cmp(&self.distance).unwrap_or(Ordering::Equal)
    }
}

impl TensorIndex {
    pub fn new() -> Self {
        Self { tensors: Vec::new() }
    }

    pub fn insert(&mut self, id: String, tensor: Tensor12D) {
        self.tensors.push((id, tensor));
    }

    pub fn remove(&mut self, id: &str) -> Option<Tensor12D> {
        if let Some(pos) = self.tensors.iter().position(|(i, _)| i == id) {
            Some(self.tensors.remove(pos).1)
        } else {
            None
        }
    }

    /// Find k-nearest neighbors to query tensor
    pub fn knn(&self, query: &Tensor12D, k: usize) -> Vec<(String, Tensor12D, f64)> {
        let mut heap = BinaryHeap::new();

        for (id, tensor) in &self.tensors {
            let distance = query.euclidean_distance(tensor);
            heap.push(ScoredTensor {
                id: id.clone(),
                tensor: *tensor,
                distance,
            });
        }

        let mut results = Vec::with_capacity(k);
        for _ in 0..k {
            if let Some(scored) = heap.pop() {
                results.push((scored.id, scored.tensor, scored.distance));
            } else {
                break;
            }
        }
        results
    }

    /// Find all tensors within distance threshold
    pub fn range_search(&self, query: &Tensor12D, threshold: f64) -> Vec<(String, Tensor12D, f64)> {
        self.tensors.iter()
            .filter_map(|(id, tensor)| {
                let distance = query.euclidean_distance(tensor);
                if distance <= threshold {
                    Some((id.clone(), *tensor, distance))
                } else {
                    None
                }
            })
            .collect()
    }

    /// Find the centroid of all tensors
    pub fn centroid(&self) -> Option<Tensor12D> {
        if self.tensors.is_empty() {
            return None;
        }

        let mut sum = Tensor12D::zero();
        for (_, tensor) in &self.tensors {
            sum = sum + *tensor;
        }
        Some(sum * (1.0 / self.tensors.len() as f64))
    }
}
```

**Why**: Spatial indexing enables efficient similarity searches.

---

## Pattern 8: Tensor Visualization (P2)

```rust
impl Tensor12D {
    /// Generate ASCII visualization of tensor
    pub fn to_ascii_bar(&self) -> String {
        let dims = [
            ("SVC", self.d0_service_id),
            ("PRT", self.d1_port),
            ("TIR", self.d2_tier),
            ("DEP", self.d3_dependencies),
            ("AGT", self.d4_agents),
            ("PRO", self.d5_protocol),
            ("HLT", self.d6_health),
            ("UPT", self.d7_uptime),
            ("SYN", self.d8_synergy),
            ("LAT", self.d9_latency),
            ("ERR", self.d10_error_rate),
            ("TMP", self.d11_temporal),
        ];

        let mut output = String::new();
        for (name, value) in dims {
            let bar_len = (value * 20.0) as usize;
            let bar = "█".repeat(bar_len);
            let empty = "░".repeat(20 - bar_len);
            output.push_str(&format!("{name} |{bar}{empty}| {value:.2}\n"));
        }
        output
    }

    /// Generate SVG radar chart
    pub fn to_svg_radar(&self, size: u32) -> String {
        let center = size as f64 / 2.0;
        let radius = center * 0.8;
        let dims = self.as_array();

        let mut points = Vec::new();
        for (i, &value) in dims.iter().enumerate() {
            let angle = (i as f64 / 12.0) * 2.0 * std::f64::consts::PI - std::f64::consts::FRAC_PI_2;
            let r = radius * value;
            let x = center + r * angle.cos();
            let y = center + r * angle.sin();
            points.push(format!("{x:.1},{y:.1}"));
        }

        format!(
            r#"<svg width="{size}" height="{size}" xmlns="http://www.w3.org/2000/svg">
  <polygon points="{}" fill="rgba(59, 130, 246, 0.5)" stroke="#3b82f6" stroke-width="2"/>
  <circle cx="{center}" cy="{center}" r="{radius}" fill="none" stroke="#e5e7eb" stroke-width="1"/>
</svg>"#,
            points.join(" ")
        )
    }

    /// Debug summary string
    pub fn summary(&self) -> String {
        format!(
            "Tensor12D[health={:.2}, synergy={:.2}, uptime={:.2}, latency={:.2}]",
            self.d6_health, self.d8_synergy, self.d7_uptime, self.d9_latency
        )
    }
}
```

**Why**: Visualization aids debugging and understanding tensor states.

---

## CodeSynthor V7 11D Comparison

| TME 12D | CSV7 11D | Notes |
|---------|----------|-------|
| d0_service_id | d0_module_id | Same concept |
| d1_port | - | Not in CSV7 |
| d2_tier | d1_layer | Same concept |
| d3_dependencies | d2_dependencies | Same |
| d4_agents | d3_agents | Same |
| d5_protocol | d4_protocol | Same |
| d6_health | d5_health | Same |
| d7_uptime | d6_uptime | Same |
| d8_synergy | d7_synergy | Same |
| d9_latency | d8_latency | Same |
| d10_error_rate | d9_error_rate | Same |
| d11_temporal | d10_temporal | Same |
| - | d11_complexity | CSV7 only |

---

## Performance Characteristics

| Operation | Time Complexity | Space |
|-----------|----------------|-------|
| Create | O(1) | 96 bytes |
| Distance | O(d) = O(12) | O(1) |
| KNN | O(n log k) | O(k) |
| Range search | O(n) | O(results) |
| Centroid | O(n) | O(1) |

---

*Generated: 2026-01-28 | The Maintenance Engine v1.0.0*
