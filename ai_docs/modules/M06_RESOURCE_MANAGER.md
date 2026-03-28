# Module M06: Resource Manager

> **M06_RESOURCE_MANAGER** | Resource Lifecycle Management | Layer: L1 Foundation | [Back to Index](../INDEX.md)

---

## Navigation

| Direction | Link |
|-----------|------|
| Up | [INDEX.md](../INDEX.md) |
| Layer | [L01_FOUNDATION.md](../layers/L01_FOUNDATION.md) |
| Previous | [M05_STATE_PERSISTENCE.md](M05_STATE_PERSISTENCE.md) |
| Next | [L02_SERVICES.md](../layers/L02_SERVICES.md) |
| Related | [M04_METRICS_COLLECTOR.md](M04_METRICS_COLLECTOR.md), [L02_SERVICES.md](../layers/L02_SERVICES.md) |

---

## Module Overview

The Resource Manager (M06) provides resource allocation, lifecycle management, and capacity planning for the Maintenance Engine. It manages connection pools, memory budgets, CPU affinity, file descriptors, and ensures graceful resource release during shutdown. It is the guardian of system resources and prevents resource exhaustion.

### Module Properties

| Property | Value |
|----------|-------|
| Module ID | M06 |
| Module Name | Resource Manager |
| Layer | L1 (Foundation) |
| Version | 1.0 |
| Dependencies | M02 (Configuration), M03 (Logging), M04 (Metrics), M05 (State) |
| Dependents | L2-L6 (All Layers) |
| Criticality | Critical |
| Startup Order | 6 (last in L1, after M05) |

---

## Architecture Diagram

```
+------------------------------------------------------------------+
|                     M06: Resource Manager                          |
+------------------------------------------------------------------+
|                                                                    |
|  +----------------+     +------------------+     +---------------+ |
|  |  Resource      |     |   Pool Manager   |     |   Budget      | |
|  |  Registry      |---->|   (Connections)  |---->|   Controller  | |
|  +----------------+     +------------------+     +---------------+ |
|         ^                       |                       |         |
|         |                       v                       v         |
|  +----------------+     +------------------+     +---------------+ |
|  | Allocation     |     |   Memory         |     |   Threshold   | |
|  | Tracker        |     |   Manager        |     |   Monitor     | |
|  +----------------+     +------------------+     +---------------+ |
|         ^                       |                       |         |
|         |                       v                       v         |
|  +----------------+     +------------------+     +---------------+ |
|  | Request        |     |   File Descriptor|     |   Alert       | |
|  | Queue          |     |   Manager        |     |   Engine      | |
|  +----------------+     +------------------+     +---------------+ |
|         |                       |                       |         |
|         v                       v                       v         |
|  +----------------+     +------------------+     +---------------+ |
|  |  Lifecycle     |     |   CPU Affinity   |     |   Cleanup     | |
|  |  Manager       |     |   Controller     |     |   Service     | |
|  +----------------+     +------------------+     +---------------+ |
|                                                                    |
+------------------------------------------------------------------+
```

---

## Core Data Structures

### Resource Types and Handles

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Resource types managed
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ResourceType {
    /// Database connections
    DatabaseConnection,
    /// HTTP client connections
    HttpConnection,
    /// TCP socket connections
    TcpConnection,
    /// Memory allocation
    Memory,
    /// File descriptors
    FileDescriptor,
    /// Worker threads
    WorkerThread,
    /// Async task slots
    AsyncTask,
    /// Semaphore permits
    Semaphore,
    /// Channel capacity
    Channel,
    /// Custom resource
    Custom(u32),
}

/// Resource handle for tracking allocations
#[derive(Debug, Clone)]
pub struct ResourceHandle {
    /// Unique handle ID
    pub id: String,
    /// Resource type
    pub resource_type: ResourceType,
    /// Allocation size/count
    pub size: u64,
    /// Owner (module/layer)
    pub owner: ResourceOwner,
    /// Allocation timestamp
    pub allocated_at: DateTime<Utc>,
    /// Time-to-live (optional)
    pub ttl: Option<std::time::Duration>,
    /// Priority for eviction
    pub priority: ResourcePriority,
    /// Current state
    pub state: ResourceState,
}

/// Resource owner identification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceOwner {
    /// Layer ID
    pub layer: String,
    /// Module ID
    pub module: String,
    /// Component (optional)
    pub component: Option<String>,
    /// Request ID (for tracing)
    pub request_id: Option<String>,
}

/// Resource priority levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ResourcePriority {
    /// Critical - never evict
    Critical = 4,
    /// High - evict last
    High = 3,
    /// Normal - standard eviction
    Normal = 2,
    /// Low - evict first
    Low = 1,
    /// Background - evict immediately under pressure
    Background = 0,
}

/// Resource state
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum ResourceState {
    /// Resource is active
    Active,
    /// Resource is idle
    Idle,
    /// Resource is being released
    Releasing,
    /// Resource has been released
    Released,
    /// Resource is in error state
    Error,
}

/// Resource allocation request
#[derive(Debug, Clone)]
pub struct ResourceRequest {
    /// Resource type
    pub resource_type: ResourceType,
    /// Requested amount
    pub amount: u64,
    /// Owner information
    pub owner: ResourceOwner,
    /// Priority
    pub priority: ResourcePriority,
    /// Timeout for allocation
    pub timeout: Option<std::time::Duration>,
    /// TTL for the resource
    pub ttl: Option<std::time::Duration>,
    /// Allow partial allocation
    pub allow_partial: bool,
}

/// Resource budget definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceBudget {
    /// Resource type
    pub resource_type: ResourceType,
    /// Maximum allowed
    pub max_limit: u64,
    /// Soft limit (warning threshold)
    pub soft_limit: u64,
    /// Reserved for critical operations
    pub reserved: u64,
    /// Current allocation
    pub current: u64,
    /// Per-module limits (optional)
    pub module_limits: HashMap<String, u64>,
}

/// Resource pool configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolConfig {
    /// Pool name
    pub name: String,
    /// Resource type
    pub resource_type: ResourceType,
    /// Minimum pool size
    pub min_size: usize,
    /// Maximum pool size
    pub max_size: usize,
    /// Idle timeout
    pub idle_timeout: std::time::Duration,
    /// Max lifetime
    pub max_lifetime: std::time::Duration,
    /// Acquire timeout
    pub acquire_timeout: std::time::Duration,
    /// Health check interval
    pub health_check_interval: std::time::Duration,
}

/// Connection pool state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolState {
    /// Pool name
    pub name: String,
    /// Total connections
    pub total: usize,
    /// Active connections
    pub active: usize,
    /// Idle connections
    pub idle: usize,
    /// Waiting requests
    pub waiting: usize,
    /// Total connections created
    pub total_created: u64,
    /// Total connections destroyed
    pub total_destroyed: u64,
    /// Total timeouts
    pub timeouts: u64,
}

/// Memory budget
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryBudget {
    /// Total memory limit (bytes)
    pub total_limit: u64,
    /// Per-layer limits
    pub layer_limits: HashMap<String, u64>,
    /// Current usage
    pub current_usage: u64,
    /// Peak usage
    pub peak_usage: u64,
    /// GC threshold
    pub gc_threshold: u64,
}

/// Resource status report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceStatus {
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// Overall health
    pub healthy: bool,
    /// Budget statuses
    pub budgets: HashMap<ResourceType, BudgetStatus>,
    /// Pool statuses
    pub pools: HashMap<String, PoolState>,
    /// Active handles count
    pub active_handles: usize,
    /// Memory usage
    pub memory: MemoryStatus,
    /// File descriptor usage
    pub file_descriptors: FdStatus,
    /// Alerts
    pub alerts: Vec<ResourceAlert>,
}

/// Budget status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetStatus {
    pub resource_type: ResourceType,
    pub current: u64,
    pub max: u64,
    pub utilization: f64,
    pub status: BudgetHealth,
}

/// Budget health status
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum BudgetHealth {
    /// Healthy (< soft limit)
    Healthy,
    /// Warning (>= soft limit, < max)
    Warning,
    /// Critical (>= max)
    Critical,
    /// Exhausted (at max, requests waiting)
    Exhausted,
}

/// Memory status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryStatus {
    /// Total available
    pub total: u64,
    /// Currently used
    pub used: u64,
    /// Free
    pub free: u64,
    /// Usage percentage
    pub utilization: f64,
    /// Heap usage
    pub heap: u64,
    /// Stack usage estimate
    pub stack: u64,
}

/// File descriptor status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FdStatus {
    /// Total available
    pub total: u64,
    /// Currently used
    pub used: u64,
    /// Soft limit
    pub soft_limit: u64,
    /// Hard limit
    pub hard_limit: u64,
}

/// Resource alert
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceAlert {
    /// Alert ID
    pub id: String,
    /// Alert type
    pub alert_type: ResourceAlertType,
    /// Resource type
    pub resource_type: ResourceType,
    /// Severity
    pub severity: AlertSeverity,
    /// Message
    pub message: String,
    /// Current value
    pub current_value: u64,
    /// Threshold value
    pub threshold: u64,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
}

/// Resource alert types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ResourceAlertType {
    /// Approaching soft limit
    SoftLimitApproaching,
    /// Soft limit exceeded
    SoftLimitExceeded,
    /// Approaching hard limit
    HardLimitApproaching,
    /// Hard limit exceeded
    HardLimitExceeded,
    /// Resource exhausted
    ResourceExhausted,
    /// Leak detected
    LeakDetected,
    /// Pool unhealthy
    PoolUnhealthy,
    /// Allocation timeout
    AllocationTimeout,
}

/// Alert severity
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum AlertSeverity {
    Info,
    Warning,
    Error,
    Critical,
}

/// Cleanup report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleanupReport {
    /// Cleanup timestamp
    pub timestamp: DateTime<Utc>,
    /// Resources released by type
    pub released: HashMap<ResourceType, u64>,
    /// Handles cleaned up
    pub handles_cleaned: usize,
    /// Memory reclaimed (bytes)
    pub memory_reclaimed: u64,
    /// Duration (ms)
    pub duration_ms: u64,
}
```

### Resource Manager State

```rust
/// Main resource manager
pub struct ResourceManager {
    /// Resource registry
    registry: Arc<RwLock<ResourceRegistry>>,
    /// Budget controller
    budgets: Arc<RwLock<HashMap<ResourceType, ResourceBudget>>>,
    /// Connection pools
    pools: Arc<RwLock<HashMap<String, Box<dyn ResourcePool>>>>,
    /// Memory manager
    memory_manager: Arc<MemoryManager>,
    /// File descriptor manager
    fd_manager: Arc<FdManager>,
    /// Threshold monitor
    threshold_monitor: Arc<ThresholdMonitor>,
    /// Cleanup service
    cleanup_service: Arc<CleanupService>,
    /// Configuration
    config: ResourceConfig,
    /// Metrics reference
    metrics: Arc<MetricsCollector>,
    /// Logger
    logger: Logger,
}

/// Resource registry
pub struct ResourceRegistry {
    /// Active handles
    handles: HashMap<String, ResourceHandle>,
    /// Handles by owner
    by_owner: HashMap<String, Vec<String>>,
    /// Handles by type
    by_type: HashMap<ResourceType, Vec<String>>,
    /// Total allocations
    total_allocations: u64,
    /// Total releases
    total_releases: u64,
}

/// Memory manager
pub struct MemoryManager {
    /// Memory budget
    budget: RwLock<MemoryBudget>,
    /// Allocator statistics
    allocator_stats: RwLock<AllocatorStats>,
    /// GC enabled
    gc_enabled: bool,
}

/// File descriptor manager
pub struct FdManager {
    /// Current usage
    current: std::sync::atomic::AtomicU64,
    /// Soft limit
    soft_limit: u64,
    /// Hard limit
    hard_limit: u64,
    /// Tracked descriptors
    tracked: RwLock<HashMap<String, FdEntry>>,
}

/// Threshold monitor
pub struct ThresholdMonitor {
    /// Check interval
    interval: std::time::Duration,
    /// Alert callbacks
    alert_handlers: Vec<Box<dyn AlertHandler>>,
    /// Shutdown signal
    shutdown: tokio::sync::watch::Sender<bool>,
}

/// Cleanup service
pub struct CleanupService {
    /// Cleanup interval
    interval: std::time::Duration,
    /// Idle timeout
    idle_timeout: std::time::Duration,
    /// Leak detection enabled
    leak_detection: bool,
    /// Leak threshold
    leak_threshold: std::time::Duration,
}

/// Resource pool trait
#[async_trait::async_trait]
pub trait ResourcePool: Send + Sync {
    /// Acquire resource from pool
    async fn acquire(&self, timeout: std::time::Duration) -> Result<Box<dyn PooledResource>, ResourceError>;

    /// Return resource to pool
    async fn release(&self, resource: Box<dyn PooledResource>) -> Result<(), ResourceError>;

    /// Get pool state
    fn state(&self) -> PoolState;

    /// Health check
    async fn health_check(&self) -> bool;

    /// Resize pool
    async fn resize(&self, min: usize, max: usize) -> Result<(), ResourceError>;

    /// Shutdown pool
    async fn shutdown(&self) -> Result<(), ResourceError>;
}

/// Pooled resource trait
pub trait PooledResource: Send + Sync {
    /// Check if resource is valid
    fn is_valid(&self) -> bool;

    /// Reset resource for reuse
    fn reset(&mut self);

    /// Get resource age
    fn age(&self) -> std::time::Duration;
}

/// Alert handler trait
pub trait AlertHandler: Send + Sync {
    /// Handle alert
    fn handle(&self, alert: &ResourceAlert);
}

/// Resource configuration
#[derive(Debug, Clone)]
pub struct ResourceConfig {
    /// Default memory limit (bytes)
    pub default_memory_limit: u64,
    /// Default connection limit
    pub default_connection_limit: usize,
    /// Default file descriptor limit
    pub default_fd_limit: u64,
    /// Cleanup interval (seconds)
    pub cleanup_interval_secs: u64,
    /// Idle timeout (seconds)
    pub idle_timeout_secs: u64,
    /// Enable leak detection
    pub leak_detection_enabled: bool,
    /// Leak threshold (seconds)
    pub leak_threshold_secs: u64,
    /// Threshold check interval (seconds)
    pub threshold_check_interval_secs: u64,
    /// Soft limit percentage
    pub soft_limit_percentage: f64,
}
```

---

## Public API

```rust
impl ResourceManager {
    /// Create new resource manager
    pub fn new(config: ResourceConfig) -> Result<Self, ResourceError>;

    /// Initialize resource manager
    pub async fn initialize(&mut self) -> Result<(), ResourceError>;

    /// Allocate resource
    pub async fn allocate(&self, request: ResourceRequest) -> Result<ResourceHandle, ResourceError>;

    /// Try allocate (non-blocking)
    pub fn try_allocate(&self, request: ResourceRequest) -> Result<Option<ResourceHandle>, ResourceError>;

    /// Release resource
    pub async fn release(&self, handle: ResourceHandle) -> Result<(), ResourceError>;

    /// Get resource status
    pub fn status(&self) -> ResourceStatus;

    /// Get budget for resource type
    pub fn budget(&self, resource_type: ResourceType) -> Option<ResourceBudget>;

    /// Set budget limit
    pub fn set_budget(&self, resource_type: ResourceType, max_limit: u64, soft_limit: u64);

    /// Set module limit
    pub fn set_module_limit(&self, resource_type: ResourceType, module: &str, limit: u64);

    /// Get pool state
    pub fn pool_state(&self, pool_name: &str) -> Option<PoolState>;

    /// Create connection pool
    pub async fn create_pool(&self, config: PoolConfig) -> Result<(), ResourceError>;

    /// Acquire from pool
    pub async fn acquire_pooled(
        &self,
        pool_name: &str,
        timeout: std::time::Duration,
    ) -> Result<Box<dyn PooledResource>, ResourceError>;

    /// Release to pool
    pub async fn release_pooled(
        &self,
        pool_name: &str,
        resource: Box<dyn PooledResource>,
    ) -> Result<(), ResourceError>;

    /// Get memory status
    pub fn memory_status(&self) -> MemoryStatus;

    /// Get file descriptor status
    pub fn fd_status(&self) -> FdStatus;

    /// Run cleanup
    pub async fn cleanup(&self) -> CleanupReport;

    /// Force GC
    pub fn force_gc(&self);

    /// Get handles for owner
    pub fn handles_for_owner(&self, owner: &ResourceOwner) -> Vec<ResourceHandle>;

    /// Get handles by type
    pub fn handles_by_type(&self, resource_type: ResourceType) -> Vec<ResourceHandle>;

    /// Register alert handler
    pub fn register_alert_handler(&self, handler: Box<dyn AlertHandler>);

    /// Check for leaks
    pub fn check_leaks(&self) -> Vec<ResourceHandle>;

    /// Shutdown gracefully
    pub async fn shutdown(&mut self) -> Result<CleanupReport, ResourceError>;

    /// Health check
    pub fn health_check(&self) -> bool;
}

/// Resource guard for automatic release
pub struct ResourceGuard<'a> {
    manager: &'a ResourceManager,
    handle: Option<ResourceHandle>,
}

impl<'a> ResourceGuard<'a> {
    pub fn new(manager: &'a ResourceManager, handle: ResourceHandle) -> Self;

    /// Get the handle
    pub fn handle(&self) -> &ResourceHandle;

    /// Take ownership of handle (prevent auto-release)
    pub fn take(mut self) -> ResourceHandle;
}

impl<'a> Drop for ResourceGuard<'a> {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            // Spawn release task
            let manager = self.manager.clone();
            tokio::spawn(async move {
                let _ = manager.release(handle).await;
            });
        }
    }
}
```

---

## Configuration

```toml
[foundation.resources]
version = "1.0"

[foundation.resources.limits]
# Default memory limit (MB)
default_memory_mb = 2048
# Default connection limit
default_connections = 100
# Default file descriptor limit
default_file_descriptors = 10000
# Default worker threads
default_worker_threads = 8
# Default async tasks
default_async_tasks = 1000

[foundation.resources.budgets.database_connection]
max_limit = 100
soft_limit = 80
reserved = 10

[foundation.resources.budgets.http_connection]
max_limit = 200
soft_limit = 160
reserved = 20

[foundation.resources.budgets.memory]
max_limit = 2147483648  # 2GB
soft_limit = 1717986918  # 1.6GB
reserved = 214748364    # 200MB

[foundation.resources.budgets.file_descriptor]
max_limit = 10000
soft_limit = 8000
reserved = 500

[foundation.resources.pools.database]
min_size = 10
max_size = 100
idle_timeout_secs = 300
max_lifetime_secs = 3600
acquire_timeout_secs = 30
health_check_interval_secs = 30

[foundation.resources.pools.http]
min_size = 20
max_size = 200
idle_timeout_secs = 60
max_lifetime_secs = 600
acquire_timeout_secs = 10
health_check_interval_secs = 15

[foundation.resources.cleanup]
# Cleanup interval (seconds)
interval_secs = 60
# Idle timeout (seconds)
idle_timeout_secs = 300
# Enable leak detection
leak_detection_enabled = true
# Leak threshold (seconds)
leak_threshold_secs = 3600

[foundation.resources.monitoring]
# Threshold check interval (seconds)
check_interval_secs = 10
# Soft limit percentage (for warnings)
soft_limit_percentage = 0.8
# Alert cooldown (seconds)
alert_cooldown_secs = 300

[foundation.resources.module_limits]
# Per-module limits
"L2.M07" = { memory_mb = 512, connections = 30 }
"L3.M12" = { memory_mb = 256, connections = 20 }
"L4.M18" = { memory_mb = 128, connections = 10 }
```

---

## Bi-Directional Data Flow

### Inbound Data Flow

```
+------------------------------------------------------------------+
|                     INBOUND DATA SOURCES                          |
+------------------------------------------------------------------+
|                                                                    |
|  +-------------+   +-------------+   +-------------+              |
|  | All Layers  |   |    M02      |   |    M04      |              |
|  | L1-L6       |   |   Config    |   |   Metrics   |              |
|  | (Requests)  |   |   Manager   |   |  Collector  |              |
|  +------+------+   +------+------+   +------+------+              |
|         |                 |                 |                      |
|         v                 v                 v                      |
|    Allocation        Config Updates    Resource                    |
|    Requests          (limits, etc)     Alerts                      |
|         |                 |                 |                      |
|         +--------+--------+--------+--------+                      |
|                  |                 |                               |
|                  v                 v                               |
|         +------------------+  +------------------+                 |
|         |  Request Queue   |  |  Config Handler  |                 |
|         +------------------+  +------------------+                 |
|                                                                    |
+------------------------------------------------------------------+
```

#### Inbound Message Types

```rust
/// Messages received by Resource Manager
#[derive(Debug, Clone)]
pub enum ResourceInboundMessage {
    /// Allocation request
    AllocationRequest {
        request: ResourceRequest,
        response_channel: tokio::sync::oneshot::Sender<Result<ResourceHandle, ResourceError>>,
    },

    /// Release request
    ReleaseRequest {
        handle: ResourceHandle,
        response_channel: tokio::sync::oneshot::Sender<Result<(), ResourceError>>,
    },

    /// Pool acquire request
    PoolAcquireRequest {
        pool_name: String,
        timeout: std::time::Duration,
        response_channel: tokio::sync::oneshot::Sender<Result<Box<dyn PooledResource>, ResourceError>>,
    },

    /// Pool release request
    PoolReleaseRequest {
        pool_name: String,
        resource: Box<dyn PooledResource>,
    },

    /// Configuration update from M02
    ConfigUpdate {
        config: ResourceConfig,
        budgets: HashMap<ResourceType, ResourceBudget>,
    },

    /// Budget update request
    BudgetUpdate {
        resource_type: ResourceType,
        max_limit: u64,
        soft_limit: u64,
    },

    /// Module limit update
    ModuleLimitUpdate {
        module: String,
        resource_type: ResourceType,
        limit: u64,
    },

    /// Status request
    StatusRequest {
        response_channel: tokio::sync::oneshot::Sender<ResourceStatus>,
    },

    /// Cleanup request
    CleanupRequest {
        force: bool,
        response_channel: tokio::sync::oneshot::Sender<CleanupReport>,
    },

    /// Threshold alert from M04
    ThresholdAlert {
        resource_type: ResourceType,
        current_value: f64,
        threshold: f64,
    },

    /// Storage alert from M05
    StorageAlert {
        alert_type: String,
        current_bytes: u64,
        threshold_bytes: u64,
    },
}
```

#### Inbound Sequence Diagram

```
                    INBOUND FLOW

    L1-L6           M02 Config         M04 Metrics
    Modules         Manager            Collector
        |               |                   |
        | allocate()    | ConfigUpdate      | ThresholdAlert
        | release()     | (budgets)         | (resource alerts)
        v               v                   v
    +----------------------------------------------+
    |          Resource Manager Inbox               |
    +----------------------------------------------+
                        |
                        v
              +------------------+
              |  Request Router  |
              +------------------+
                        |
         +--------------+--------------+
         |              |              |
         v              v              v
    +----------+  +-----------+  +-----------+
    | Budget   |  |   Pool    |  | Threshold |
    | Check    |  |  Manager  |  |  Update   |
    +----------+  +-----------+  +-----------+
         |              |              |
         v              v              v
    +----------------------------------------------+
    |             Resource Registry                 |
    +----------------------------------------------+
                        |
                        v
              +------------------+
              |  Allocation/     |
              |  Release         |
              +------------------+
```

### Outbound Data Flow

```
+------------------------------------------------------------------+
|                     OUTBOUND DATA TARGETS                         |
+------------------------------------------------------------------+
|                                                                    |
|         +------------------+                                       |
|         | Resource Manager |                                       |
|         |  (Source)        |                                       |
|         +--------+---------+                                       |
|                  |                                                 |
|    +-------------+-------------+-------------+-------------+       |
|    |             |             |             |             |       |
|    v             v             v             v             v       |
| +------+   +------+   +------+   +------+   +------+              |
| | M03  |   | M04  |   | M05  |   | L2-6 |   | Ops  |              |
| | Log  |   |Metric|   |State |   |Handle|   |Alert |              |
| +------+   +------+   +------+   +------+   +------+              |
|    |             |             |             |             |       |
|    v             v             v             v             v       |
| Resource     Resource     Resource      Handle       Alert         |
| Events       Metrics      State         Results      Notifications |
|                                                                    |
+------------------------------------------------------------------+
```

#### Outbound Message Types

```rust
/// Messages sent by Resource Manager
#[derive(Debug, Clone)]
pub enum ResourceOutboundMessage {
    /// Allocation result
    AllocationResult {
        request_id: String,
        result: Result<ResourceHandle, ResourceError>,
        allocation_time_ms: u64,
    },

    /// Release confirmation
    ReleaseConfirmation {
        handle_id: String,
        success: bool,
    },

    /// Resource metrics to M04
    ResourceMetrics {
        budgets: HashMap<ResourceType, BudgetStatus>,
        pools: HashMap<String, PoolState>,
        memory: MemoryStatus,
        file_descriptors: FdStatus,
        timestamp: DateTime<Utc>,
    },

    /// Resource state to M05
    ResourceState {
        active_handles: usize,
        total_allocations: u64,
        total_releases: u64,
        pool_states: HashMap<String, PoolState>,
    },

    /// Resource log to M03
    ResourceLog {
        operation: String,
        resource_type: ResourceType,
        handle_id: Option<String>,
        success: bool,
        details: HashMap<String, String>,
    },

    /// Alert notification
    AlertNotification {
        alert: ResourceAlert,
    },

    /// Cleanup report
    CleanupComplete {
        report: CleanupReport,
    },

    /// Pool health update
    PoolHealthUpdate {
        pool_name: String,
        healthy: bool,
        state: PoolState,
    },

    /// Leak detection result
    LeakDetected {
        handles: Vec<ResourceHandle>,
        total_leaked: u64,
    },
}
```

#### Outbound Sequence Diagram

```
                    OUTBOUND FLOW

                 Resource Manager
                        |
                        | Operation Complete
                        v
              +------------------+
              |  Event Emitter   |
              +------------------+
                        |
         +--------------+--------------+--------------+
         |              |              |              |
         v              v              v              v
    +----------+  +-----------+  +-----------+  +-----------+
    |    M03   |  |    M04    |  |    M05    |  |   Ops     |
    |  Logging |  |  Metrics  |  |   State   |  |  Alerts   |
    +----------+  +-----------+  +-----------+  +-----------+
         |              |              |              |
         v              v              v              v
    Resource       Budget        Resource        Alert
    Events         Metrics       Snapshots       Handlers


    Alert Path:

              +------------------+
              |  Threshold       |
              |  Exceeded        |
              +--------+---------+
                       |
                       v
              +------------------+
              |  Alert Engine    |
              +--------+---------+
                       |
         +-------------+-------------+
         |             |             |
         v             v             v
    +----------+  +-----------+  +-----------+
    |    M03   |  |    Ops    |  |  External |
    |   Log    |  |  Console  |  |  Webhook  |
    +----------+  +-----------+  +-----------+
```

### Cross-Module Dependencies

```
+------------------------------------------------------------------+
|            BI-DIRECTIONAL DEPENDENCY MATRIX (M06)                 |
+------------------------------------------------------------------+
|                                                                    |
|  Module    | M06 Reads From        | M06 Writes To                |
|  ----------|------------------------|-----------------------------  |
|  M01       | Error codes for       | Resource errors for          |
|            | resource errors        | classification               |
|  ----------|------------------------|-----------------------------  |
|  M02       | Resource limits,      | -                            |
|            | pool configs           |                              |
|  ----------|------------------------|-----------------------------  |
|  M03       | -                      | Resource events,             |
|            |                        | allocation logs              |
|  ----------|------------------------|-----------------------------  |
|  M04       | Threshold alerts      | Resource metrics,            |
|            |                        | utilization stats            |
|  ----------|------------------------|-----------------------------  |
|  M05       | -                      | Resource state               |
|            |                        | snapshots                    |
|  ----------|------------------------|-----------------------------  |
|  L2-L6     | Allocation requests   | Resource handles,            |
|            |                        | pool resources               |
|                                                                    |
+------------------------------------------------------------------+

Communication Patterns:
+------------------------------------------------------------------+
|  Pattern          | Source  | Target  | Type        | Frequency  |
|  -----------------|---------|---------|-------------|----------- |
|  Allocate         | L1-L6   | M06     | Sync        | Continuous |
|  Release          | L1-L6   | M06     | Async       | Continuous |
|  Pool Acquire     | L1-L6   | M06     | Sync        | Continuous |
|  Pool Release     | L1-L6   | M06     | Async       | Continuous |
|  Config Update    | M02     | M06     | Event       | On change  |
|  Threshold Alert  | M04     | M06     | Async       | On thresh  |
|  Metrics Export   | M06     | M04     | Async       | Periodic   |
|  State Export     | M06     | M05     | Async       | Periodic   |
|  Cleanup          | Timer   | M06     | Async       | Scheduled  |
+------------------------------------------------------------------+

Error Propagation:
+------------------------------------------------------------------+
|  Error Source     | Propagates To       | Action                  |
|  -----------------|---------------------|------------------------ |
|  Budget Exceeded  | Requester           | Reject + Queue          |
|  Pool Exhausted   | Requester           | Timeout + Retry         |
|  Memory Pressure  | M04 (alert)         | GC + Evict              |
|  Leak Detected    | M03 (log), Ops      | Alert + Clean           |
|  FD Exhausted     | M03 (log), Ops      | Critical Alert          |
+------------------------------------------------------------------+
```

### Contextual Flow

```
+------------------------------------------------------------------+
|                   RESOURCE ALLOCATION LIFECYCLE                    |
+------------------------------------------------------------------+
|                                                                    |
|  1. REQUEST PHASE                                                  |
|     +------------------+                                           |
|     |  allocate()      |  (From any module/layer)                 |
|     +--------+---------+                                           |
|              |                                                     |
|  2. VALIDATE PHASE                                                 |
|              v                                                     |
|     +------------------+                                           |
|     |  Budget Check    |  (Within limits?)                        |
|     +--------+---------+                                           |
|              |                                                     |
|              +--------+--------+                                   |
|              |                 |                                   |
|         within limit      over limit                               |
|              |                 |                                   |
|              v                 v                                   |
|     +------------------+ +------------------+                      |
|     |   Allocate       | |   Queue/Reject   |                      |
|     +--------+---------+ +------------------+                      |
|              |                                                     |
|  3. REGISTER PHASE                                                 |
|              v                                                     |
|     +------------------+                                           |
|     |  Create Handle   |  (Track in registry)                     |
|     +--------+---------+                                           |
|              |                                                     |
|  4. RETURN PHASE                                                   |
|              v                                                     |
|     +------------------+                                           |
|     |  Return Handle   |  (To requester)                          |
|     +--------+---------+                                           |
|              |                                                     |
|  5. USE PHASE                                                      |
|              v                                                     |
|     +------------------+                                           |
|     |  Resource Active |  (In use by module)                      |
|     +--------+---------+                                           |
|              |                                                     |
|  6. RELEASE PHASE                                                  |
|              v                                                     |
|     +------------------+                                           |
|     |  release()       |  (Explicit or guard drop)                |
|     +--------+---------+                                           |
|              |                                                     |
|  7. CLEANUP PHASE                                                  |
|              v                                                     |
|     +------------------+                                           |
|     |  Update Budget   |  (Decrement usage)                       |
|     +--------+---------+                                           |
|              |                                                     |
|              v                                                     |
|     +------------------+                                           |
|     |  Remove Handle   |  (From registry)                         |
|     +------------------+                                           |
|                                                                    |
+------------------------------------------------------------------+
```

#### Connection Pool Flow

```
+------------------------------------------------------------------+
|                   CONNECTION POOL LIFECYCLE                        |
+------------------------------------------------------------------+
|                                                                    |
|     acquire()                                                      |
|         |                                                          |
|         v                                                          |
|  +-------------+                                                   |
|  |  Pool Check |                                                   |
|  +------+------+                                                   |
|         |                                                          |
|    +----+----+                                                     |
|    |         |                                                     |
|   Idle     Empty                                                   |
|    |         |                                                     |
|    v         v                                                     |
|  +-----+ +-------------+                                           |
|  |Get  | |  Create New |                                           |
|  |Idle | |  (if < max) |                                           |
|  +--+--+ +------+------+                                           |
|     |           |                                                  |
|     +-----+-----+                                                  |
|           |                                                        |
|           v                                                        |
|    +-------------+                                                 |
|    | Health Check|                                                 |
|    +------+------+                                                 |
|           |                                                        |
|      +----+----+                                                   |
|      |         |                                                   |
|   Healthy   Unhealthy                                              |
|      |         |                                                   |
|      v         v                                                   |
|  +------+ +----------+                                             |
|  |Return| |Destroy & |                                             |
|  |Conn  | |Create New|                                             |
|  +------+ +----------+                                             |
|      |                                                             |
|      v                                                             |
|  [Connection Used]                                                 |
|      |                                                             |
|      v                                                             |
|   release()                                                        |
|      |                                                             |
|      v                                                             |
|  +-------------+                                                   |
|  | Return Pool |                                                   |
|  +------+------+                                                   |
|         |                                                          |
|    +----+----+                                                     |
|    |         |                                                     |
|  Valid    Invalid                                                  |
|    |         |                                                     |
|    v         v                                                     |
|  +-----+ +--------+                                                |
|  |Add  | |Destroy |                                                |
|  |Idle | +--------+                                                |
|  +-----+                                                           |
|                                                                    |
+------------------------------------------------------------------+
```

---

## Metrics

| Metric | Type | Description |
|--------|------|-------------|
| `me_m06_allocations_total` | Counter | Total allocations by type |
| `me_m06_releases_total` | Counter | Total releases by type |
| `me_m06_allocation_duration_ms` | Histogram | Allocation latency |
| `me_m06_active_handles` | Gauge | Currently active handles |
| `me_m06_budget_utilization` | Gauge | Budget utilization by type (0-1) |
| `me_m06_pool_connections_total` | Gauge | Total pool connections |
| `me_m06_pool_connections_active` | Gauge | Active pool connections |
| `me_m06_pool_connections_idle` | Gauge | Idle pool connections |
| `me_m06_pool_acquire_duration_ms` | Histogram | Pool acquire latency |
| `me_m06_pool_timeouts_total` | Counter | Pool acquire timeouts |
| `me_m06_memory_used_bytes` | Gauge | Memory usage |
| `me_m06_fd_used` | Gauge | File descriptors used |
| `me_m06_cleanups_total` | Counter | Cleanup runs |
| `me_m06_leaks_detected_total` | Counter | Detected leaks |

---

## Error Codes

| Code | Name | Description | Severity | Recovery |
|------|------|-------------|----------|----------|
| E6001 | RESOURCE_BUDGET_EXCEEDED | Resource budget exhausted | Warning | Queue/Reject |
| E6002 | RESOURCE_POOL_EXHAUSTED | Connection pool exhausted | Warning | Wait/Timeout |
| E6003 | RESOURCE_ALLOCATION_TIMEOUT | Allocation timed out | Warning | Retry |
| E6004 | RESOURCE_INVALID_HANDLE | Invalid resource handle | Error | Log + Ignore |
| E6005 | RESOURCE_ALREADY_RELEASED | Resource already released | Warning | Log + Ignore |
| E6006 | RESOURCE_LEAK_DETECTED | Potential resource leak | Warning | Alert + Clean |
| E6007 | RESOURCE_POOL_UNHEALTHY | Pool health check failed | Error | Recreate pool |
| E6008 | RESOURCE_MEMORY_PRESSURE | Memory pressure detected | Warning | GC + Evict |
| E6009 | RESOURCE_FD_EXHAUSTED | File descriptors exhausted | Critical | Emergency clean |
| E6010 | RESOURCE_SHUTDOWN_TIMEOUT | Shutdown timeout | Warning | Force release |

---

## Navigation

| Direction | Link |
|-----------|------|
| Up | [INDEX.md](../INDEX.md) |
| Layer | [L01_FOUNDATION.md](../layers/L01_FOUNDATION.md) |
| Previous | [M05_STATE_PERSISTENCE.md](M05_STATE_PERSISTENCE.md) |
| Next | [L02_SERVICES.md](../layers/L02_SERVICES.md) |
| Related | [M04_METRICS_COLLECTOR.md](M04_METRICS_COLLECTOR.md) |

---

*[Back to Index](../INDEX.md) | [Layer: L01 Foundation](../layers/L01_FOUNDATION.md)*
