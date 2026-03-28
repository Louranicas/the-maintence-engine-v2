# M06 Resources — resources.rs

> **File:** `src/m1_foundation/resources.rs` | **LOC:** ~1,271 | **Tests:** 16
> **Role:** System resource collection (/proc), limit checking, adaptive limits (NAM R2)

---

## ResourceCollector Trait

```rust
pub trait ResourceCollector: Send + Sync {
    fn collect(&self) -> Result<SystemResources>;
    fn check_limits(&self) -> Vec<ResourceAlert>;
    fn health_score(&self) -> f64;
    fn agent_id(&self) -> Option<&str> { None }                    // default (NAM R5)
    fn to_tensor(&self) -> Tensor12D { Tensor12D::default() }      // default (NAM R4)
}
```

Concrete implementor: `ResourceManager`

---

## SystemResources

```rust
#[derive(Clone, Debug)]
pub struct SystemResources {
    pub cpu_usage_percent: f64,
    pub memory_usage_percent: f64,
    pub memory_total_bytes: u64,
    pub memory_available_bytes: u64,
    pub disk_usage_percent: f64,
    pub disk_total_bytes: u64,
    pub disk_available_bytes: u64,
    pub open_file_descriptors: u32,
    pub timestamp: SystemTime,
}
```

Tensor contribution: `to_tensor()` → D2=tier, D5=protocol, D6=1-cpu, D9=1.0, D10=cpu

---

## ResourceLimits

```rust
pub const fn new(max_cpu: f64, max_memory: f64, max_disk: f64, max_files: u32) -> Self
```

Default: cpu=80%, memory=85%, disk=90%, files=1000

`validate(&self)` → `Error::Validation` if any percent outside [0.0, 100.0]

---

## AdaptiveResourceLimits (NAM R2)

```rust
#[derive(Debug, Clone)]
pub struct AdaptiveResourceLimits {
    pub base_limits: ResourceLimits,
    pub pathway_strength: f64,    // Hebbian pathway influence
}
```

`effective_limits()` returns adjusted `ResourceLimits` based on `pathway_strength`.

---

## ResourceAlert

```rust
#[derive(Clone, Debug, PartialEq)]
pub enum ResourceAlert {
    CpuHigh { current: f64, threshold: f64 },
    MemoryHigh { current: f64, threshold: f64 },
    DiskHigh { current: f64, threshold: f64 },
    OpenFilesHigh { current: u32, threshold: u32 },
}
```

Traits: `Display` ("CPU usage high: 95.0% (threshold: 80.0%)")

---

## ResourceManager

```rust
pub fn new() -> Self                             // default limits
pub fn with_limits(limits: ResourceLimits) -> Self
pub fn collect_and_check(&mut self) -> Result<(SystemResources, Vec<ResourceAlert>)>
pub fn set_limits(&mut self, limits: ResourceLimits) -> Result<()>  // validates
pub fn is_healthy(&self) -> bool
pub fn health_score(&self) -> f64
pub fn alert_history(&self) -> &[(SystemTime, ResourceAlert)]
pub fn utilization_summary(&self) -> HashMap<String, f64>
```

**Concurrency:** `&mut self` methods — no interior mutability. Caller wraps in `RwLock` if needed.

**Alert history:** Bounded ring buffer (max 100 entries).

---

## Free Functions

```rust
pub fn collect_resources() -> Result<SystemResources>          // reads /proc/stat, /proc/meminfo, /proc/mounts
pub fn get_process_info() -> Result<ProcessInfo>               // reads /proc/{pid}/fd, /proc/{pid}/status
pub fn check_limits(resources, limits) -> Vec<ResourceAlert>   // pure function
pub fn format_resources(resources) -> String
pub fn format_alerts(alerts) -> String
pub fn compute_health_score(snapshot, limits) -> f64           // 1.0 if None
```

**Platform:** Linux-specific (`/proc` filesystem). Non-Linux: returns zeros (soft degradation, no error).

---

## Clippy Allowances

- `cast_precision_loss` — u64 → f64 for percentage calculation
- `cast_possible_truncation` — u64 → u32 for file descriptor count

---

*M06 Resources Spec v1.0 | 2026-03-01*
