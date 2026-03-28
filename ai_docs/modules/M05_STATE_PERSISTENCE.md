# Module M05: State Persistence

> **M05_STATE_PERSISTENCE** | Durable State Storage | Layer: L1 Foundation | [Back to Index](../INDEX.md)

---

## Navigation

| Direction | Link |
|-----------|------|
| Up | [INDEX.md](../INDEX.md) |
| Layer | [L01_FOUNDATION.md](../layers/L01_FOUNDATION.md) |
| Previous | [M04_METRICS_COLLECTOR.md](M04_METRICS_COLLECTOR.md) |
| Next | [M06_RESOURCE_MANAGER.md](M06_RESOURCE_MANAGER.md) |
| Related | [M02_CONFIGURATION_MANAGER.md](M02_CONFIGURATION_MANAGER.md), [L03_LEARNING.md](../layers/L03_LEARNING.md) |

---

## Module Overview

The State Persistence module (M05) provides durable state storage with transactional guarantees, point-in-time recovery, and migration management. It serves as the persistence backbone for all Maintenance Engine state, including configuration snapshots, learning pathways, service states, and operational data.

### Module Properties

| Property | Value |
|----------|-------|
| Module ID | M05 |
| Module Name | State Persistence |
| Layer | L1 (Foundation) |
| Version | 1.0 |
| Dependencies | M02 (Configuration), M03 (Logging), M04 (Metrics) |
| Dependents | M02 (Config Restore), M06 (Resource State), L2-L6 (State Storage) |
| Criticality | Critical |
| Startup Order | 5 (after M04) |

---

## Architecture Diagram

```
+------------------------------------------------------------------+
|                    M05: State Persistence                          |
+------------------------------------------------------------------+
|                                                                    |
|  +----------------+     +------------------+     +---------------+ |
|  |  State Store   |     |   Transaction    |     |  Migration    | |
|  |  (Key-Value)   |---->|   Manager        |---->|  Engine       | |
|  +----------------+     +------------------+     +---------------+ |
|         ^                       |                       |         |
|         |                       v                       v         |
|  +----------------+     +------------------+     +---------------+ |
|  | Write-Ahead    |     |   Snapshot       |     |  Schema       | |
|  | Log (WAL)      |     |   Manager        |     |  Validator    | |
|  +----------------+     +------------------+     +---------------+ |
|         |                       |                       |         |
|         v                       v                       v         |
|  +----------------+     +------------------+     +---------------+ |
|  |  SQLite        |     |   Recovery       |     |  Compaction   | |
|  |  Backend       |     |   Engine         |     |  Service      | |
|  +----------------+     +------------------+     +---------------+ |
|                                                                    |
+------------------------------------------------------------------+
```

---

## Core Data Structures

### State Entry and Versioning

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// State entry with versioning
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateEntry {
    /// State key (unique identifier)
    pub key: String,
    /// State value (JSONB)
    pub value: serde_json::Value,
    /// Version number (monotonically increasing)
    pub version: u64,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    /// Last update timestamp
    pub updated_at: DateTime<Utc>,
    /// Time-to-live (optional, seconds)
    pub ttl: Option<u64>,
    /// State metadata
    pub metadata: StateMetadata,
}

/// State metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateMetadata {
    /// Owning layer
    pub layer: String,
    /// Owning module
    pub module: String,
    /// State category
    pub category: StateCategory,
    /// Size in bytes
    pub size_bytes: u64,
    /// Checksum for integrity
    pub checksum: String,
    /// Compression applied
    pub compressed: bool,
    /// Encryption applied
    pub encrypted: bool,
}

/// State categories
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum StateCategory {
    /// Configuration state
    Configuration,
    /// Service state
    Service,
    /// Learning pathway state
    Learning,
    /// Operational state
    Operational,
    /// Metric state
    Metric,
    /// Session state
    Session,
    /// Cache state
    Cache,
    /// Consensus state
    Consensus,
}

/// Transaction for atomic operations
#[derive(Debug)]
pub struct Transaction {
    /// Transaction ID
    pub id: String,
    /// Transaction operations
    pub operations: Vec<TransactionOp>,
    /// Started timestamp
    pub started_at: DateTime<Utc>,
    /// Isolation level
    pub isolation: IsolationLevel,
    /// Transaction state
    pub state: TransactionState,
}

/// Transaction operations
#[derive(Debug, Clone)]
pub enum TransactionOp {
    /// Insert new state
    Insert {
        key: String,
        value: serde_json::Value,
        metadata: StateMetadata,
    },
    /// Update existing state
    Update {
        key: String,
        value: serde_json::Value,
        expected_version: Option<u64>,
    },
    /// Delete state
    Delete {
        key: String,
        expected_version: Option<u64>,
    },
    /// Conditional check
    Check {
        key: String,
        condition: Condition,
    },
}

/// Conditional checks
#[derive(Debug, Clone)]
pub enum Condition {
    /// Key exists
    Exists,
    /// Key does not exist
    NotExists,
    /// Version equals
    VersionEquals(u64),
    /// Value matches
    ValueMatches(serde_json::Value),
}

/// Transaction isolation levels
#[derive(Debug, Clone, Copy)]
pub enum IsolationLevel {
    /// Read uncommitted
    ReadUncommitted,
    /// Read committed
    ReadCommitted,
    /// Repeatable read
    RepeatableRead,
    /// Serializable
    Serializable,
}

/// Transaction state
#[derive(Debug, Clone, Copy)]
pub enum TransactionState {
    /// Transaction active
    Active,
    /// Transaction committed
    Committed,
    /// Transaction rolled back
    RolledBack,
    /// Transaction failed
    Failed,
}

/// State snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateSnapshot {
    /// Snapshot ID
    pub id: String,
    /// Snapshot timestamp
    pub timestamp: DateTime<Utc>,
    /// Snapshot type
    pub snapshot_type: SnapshotType,
    /// Included categories
    pub categories: Vec<StateCategory>,
    /// Total entries
    pub entry_count: u64,
    /// Total size bytes
    pub size_bytes: u64,
    /// Checksum
    pub checksum: String,
    /// Compression
    pub compression: Option<CompressionType>,
    /// Location (path or URL)
    pub location: String,
}

/// Snapshot types
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum SnapshotType {
    /// Full snapshot of all state
    Full,
    /// Incremental since last snapshot
    Incremental,
    /// Differential since last full
    Differential,
    /// Point-in-time recovery marker
    PointInTime,
}

/// Compression types
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum CompressionType {
    None,
    Gzip,
    Lz4,
    Zstd,
}

/// Write-ahead log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalEntry {
    /// Log sequence number
    pub lsn: u64,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// Operation type
    pub operation: WalOperation,
    /// Transaction ID (if part of transaction)
    pub transaction_id: Option<String>,
    /// Checksum
    pub checksum: String,
}

/// WAL operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WalOperation {
    Insert { key: String, value: serde_json::Value },
    Update { key: String, value: serde_json::Value, old_version: u64 },
    Delete { key: String, old_value: serde_json::Value },
    TransactionStart { id: String },
    TransactionCommit { id: String },
    TransactionRollback { id: String },
    Checkpoint { snapshot_id: String },
}

/// Migration definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Migration {
    /// Migration version
    pub version: u64,
    /// Migration name
    pub name: String,
    /// Description
    pub description: String,
    /// Up SQL
    pub up_sql: String,
    /// Down SQL (rollback)
    pub down_sql: String,
    /// Checksum
    pub checksum: String,
    /// Applied timestamp (if applied)
    pub applied_at: Option<DateTime<Utc>>,
}

/// Query builder for state queries
#[derive(Debug, Clone)]
pub struct StateQuery {
    /// Key pattern (supports wildcards)
    pub key_pattern: Option<String>,
    /// Category filter
    pub category: Option<StateCategory>,
    /// Layer filter
    pub layer: Option<String>,
    /// Module filter
    pub module: Option<String>,
    /// Created after
    pub created_after: Option<DateTime<Utc>>,
    /// Updated after
    pub updated_after: Option<DateTime<Utc>>,
    /// Limit
    pub limit: Option<u64>,
    /// Offset
    pub offset: Option<u64>,
    /// Order by
    pub order_by: Option<OrderBy>,
}

/// Order by options
#[derive(Debug, Clone)]
pub struct OrderBy {
    pub field: String,
    pub direction: OrderDirection,
}

/// Order direction
#[derive(Debug, Clone, Copy)]
pub enum OrderDirection {
    Ascending,
    Descending,
}
```

### State Persistence Manager

```rust
use std::sync::Arc;
use tokio::sync::RwLock;

/// Main state persistence manager
pub struct StatePersistence {
    /// Database connection pool
    pool: Arc<DbPool>,
    /// Write-ahead log
    wal: Arc<RwLock<WriteAheadLog>>,
    /// Snapshot manager
    snapshot_manager: Arc<SnapshotManager>,
    /// Migration engine
    migration_engine: Arc<MigrationEngine>,
    /// Transaction manager
    transaction_manager: Arc<TransactionManager>,
    /// Recovery engine
    recovery_engine: Arc<RecoveryEngine>,
    /// Compaction service
    compaction: Arc<CompactionService>,
    /// Configuration
    config: PersistenceConfig,
    /// Metrics reference
    metrics: Arc<MetricsCollector>,
}

/// Database connection pool
pub struct DbPool {
    /// Pool of SQLite connections
    connections: Vec<rusqlite::Connection>,
    /// Pool size
    size: usize,
    /// Available connections
    available: tokio::sync::Semaphore,
}

/// Write-ahead log
pub struct WriteAheadLog {
    /// Current log file
    current_file: std::fs::File,
    /// Current LSN
    current_lsn: u64,
    /// Sync policy
    sync_policy: WalSyncPolicy,
    /// Max file size
    max_file_size: u64,
}

/// WAL sync policy
#[derive(Debug, Clone, Copy)]
pub enum WalSyncPolicy {
    /// Sync every write
    SyncEveryWrite,
    /// Sync periodically
    SyncPeriodic(u64),  // ms
    /// Sync on commit only
    SyncOnCommit,
}

/// Snapshot manager
pub struct SnapshotManager {
    /// Snapshot directory
    snapshot_dir: std::path::PathBuf,
    /// Active snapshots
    snapshots: RwLock<Vec<StateSnapshot>>,
    /// Retention policy
    retention: SnapshotRetention,
}

/// Snapshot retention policy
#[derive(Debug, Clone)]
pub struct SnapshotRetention {
    /// Keep last N full snapshots
    pub full_count: usize,
    /// Keep last N incremental snapshots
    pub incremental_count: usize,
    /// Max age for snapshots
    pub max_age: std::time::Duration,
}

/// Persistence configuration
#[derive(Debug, Clone)]
pub struct PersistenceConfig {
    /// Database path
    pub database_path: String,
    /// Pool size
    pub pool_size: usize,
    /// WAL directory
    pub wal_dir: String,
    /// WAL sync policy
    pub wal_sync: WalSyncPolicy,
    /// Snapshot directory
    pub snapshot_dir: String,
    /// Auto-migration
    pub auto_migrate: bool,
    /// Compression
    pub compression: CompressionType,
    /// Encryption key (if encrypting)
    pub encryption_key: Option<String>,
    /// Checkpoint interval (seconds)
    pub checkpoint_interval_secs: u64,
    /// Compaction interval (seconds)
    pub compaction_interval_secs: u64,
}
```

---

## Public API

```rust
impl StatePersistence {
    /// Create new state persistence
    pub async fn new(config: PersistenceConfig) -> Result<Self, PersistenceError>;

    /// Initialize database and run migrations
    pub async fn initialize(&mut self) -> Result<(), PersistenceError>;

    /// Save state
    pub async fn save<T: Serialize>(
        &self,
        key: &str,
        state: &T,
        metadata: StateMetadata,
    ) -> Result<u64, PersistenceError>;

    /// Load state
    pub async fn load<T: DeserializeOwned>(
        &self,
        key: &str,
    ) -> Result<Option<(T, u64)>, PersistenceError>;

    /// Load state with version check
    pub async fn load_version<T: DeserializeOwned>(
        &self,
        key: &str,
        version: u64,
    ) -> Result<Option<T>, PersistenceError>;

    /// Delete state
    pub async fn delete(&self, key: &str) -> Result<bool, PersistenceError>;

    /// Check if key exists
    pub async fn exists(&self, key: &str) -> Result<bool, PersistenceError>;

    /// Query states
    pub async fn query(&self, query: StateQuery) -> Result<Vec<StateEntry>, PersistenceError>;

    /// Start transaction
    pub async fn begin(&self) -> Result<Transaction, PersistenceError>;

    /// Execute transaction
    pub async fn transaction<F, R>(&self, f: F) -> Result<R, PersistenceError>
    where
        F: FnOnce(&Transaction) -> Result<R, PersistenceError>;

    /// Commit transaction
    pub async fn commit(&self, transaction: Transaction) -> Result<(), PersistenceError>;

    /// Rollback transaction
    pub async fn rollback(&self, transaction: Transaction) -> Result<(), PersistenceError>;

    /// Create snapshot
    pub async fn snapshot(&self, snapshot_type: SnapshotType) -> Result<StateSnapshot, PersistenceError>;

    /// Restore from snapshot
    pub async fn restore(&self, snapshot: &StateSnapshot) -> Result<(), PersistenceError>;

    /// List snapshots
    pub async fn list_snapshots(&self) -> Result<Vec<StateSnapshot>, PersistenceError>;

    /// Run migrations
    pub async fn migrate(&self) -> Result<Vec<Migration>, PersistenceError>;

    /// Rollback migration
    pub async fn rollback_migration(&self, version: u64) -> Result<(), PersistenceError>;

    /// Get migration status
    pub async fn migration_status(&self) -> Result<Vec<Migration>, PersistenceError>;

    /// Checkpoint (WAL to main database)
    pub async fn checkpoint(&self) -> Result<(), PersistenceError>;

    /// Compact database
    pub async fn compact(&self) -> Result<CompactionResult, PersistenceError>;

    /// Get statistics
    pub fn statistics(&self) -> PersistenceStatistics;

    /// Health check
    pub async fn health_check(&self) -> Result<PersistenceHealth, PersistenceError>;

    /// Shutdown gracefully
    pub async fn shutdown(&mut self) -> Result<(), PersistenceError>;
}

/// Transaction operations
impl Transaction {
    /// Insert value
    pub fn insert<T: Serialize>(
        &mut self,
        key: &str,
        value: &T,
        metadata: StateMetadata,
    ) -> &mut Self;

    /// Update value
    pub fn update<T: Serialize>(
        &mut self,
        key: &str,
        value: &T,
    ) -> &mut Self;

    /// Update with version check (optimistic locking)
    pub fn update_if_version<T: Serialize>(
        &mut self,
        key: &str,
        value: &T,
        expected_version: u64,
    ) -> &mut Self;

    /// Delete value
    pub fn delete(&mut self, key: &str) -> &mut Self;

    /// Add condition check
    pub fn check(&mut self, key: &str, condition: Condition) -> &mut Self;
}
```

---

## Configuration

```toml
[foundation.persistence]
version = "1.0"

[foundation.persistence.database]
# Primary database path
path = "data/maintenance_engine.db"
# Connection pool size
pool_size = 20
# Connection timeout (ms)
connection_timeout_ms = 5000
# Busy timeout (ms)
busy_timeout_ms = 30000
# Journal mode
journal_mode = "wal"
# Synchronous mode
synchronous = "normal"
# Cache size (pages, negative = KB)
cache_size = -64000  # 64MB

[foundation.persistence.wal]
# WAL directory
dir = "data/wal"
# Sync policy (every_write, periodic, on_commit)
sync_policy = "periodic"
# Sync interval (ms, if periodic)
sync_interval_ms = 100
# Max WAL file size (MB)
max_file_size_mb = 100
# Checkpoint interval (seconds)
checkpoint_interval_secs = 300

[foundation.persistence.snapshots]
# Snapshot directory
dir = "data/snapshots"
# Full snapshot interval (hours)
full_interval_hours = 24
# Incremental snapshot interval (hours)
incremental_interval_hours = 1
# Keep last N full snapshots
full_retention = 7
# Keep last N incremental snapshots
incremental_retention = 24
# Compression
compression = "zstd"

[foundation.persistence.migrations]
# Migrations directory
dir = "migrations"
# Auto-migrate on startup
auto_migrate = true
# Validate checksums
validate_checksums = true

[foundation.persistence.compaction]
# Compaction interval (hours)
interval_hours = 24
# Target fill factor
target_fill_factor = 0.8
# Vacuum threshold (fragmentation %)
vacuum_threshold = 20

[foundation.persistence.encryption]
# Enable encryption at rest
enabled = false
# Key derivation function
kdf = "argon2id"
# Encryption algorithm
algorithm = "aes-256-gcm"
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
|  |    M02      |   |    M04      |   |   L2-L6     |              |
|  |   Config    |   |   Metrics   |   |   Layers    |              |
|  |   Manager   |   |  Collector  |   |  (All)      |              |
|  +------+------+   +------+------+   +------+------+              |
|         |                 |                 |                      |
|         v                 v                 v                      |
|    Config            Metric            State                       |
|    Snapshots         Snapshots         Save/Update                 |
|         |                 |                 |                      |
|         +--------+--------+--------+--------+                      |
|                  |                 |                               |
|                  v                 v                               |
|         +------------------+  +------------------+                 |
|         |  Transaction     |  |   Direct Save    |                 |
|         |  Manager         |  |   (Single-key)   |                 |
|         +------------------+  +------------------+                 |
|                                                                    |
+------------------------------------------------------------------+
```

#### Inbound Message Types

```rust
/// Messages received by State Persistence
#[derive(Debug, Clone)]
pub enum PersistenceInboundMessage {
    /// Save state request
    SaveState {
        key: String,
        value: serde_json::Value,
        metadata: StateMetadata,
        response_channel: tokio::sync::oneshot::Sender<Result<u64, PersistenceError>>,
    },

    /// Load state request
    LoadState {
        key: String,
        response_channel: tokio::sync::oneshot::Sender<Result<Option<StateEntry>, PersistenceError>>,
    },

    /// Delete state request
    DeleteState {
        key: String,
        response_channel: tokio::sync::oneshot::Sender<Result<bool, PersistenceError>>,
    },

    /// Query states request
    QueryStates {
        query: StateQuery,
        response_channel: tokio::sync::oneshot::Sender<Result<Vec<StateEntry>, PersistenceError>>,
    },

    /// Transaction request
    TransactionRequest {
        operations: Vec<TransactionOp>,
        isolation: IsolationLevel,
        response_channel: tokio::sync::oneshot::Sender<Result<(), PersistenceError>>,
    },

    /// Snapshot request
    SnapshotRequest {
        snapshot_type: SnapshotType,
        categories: Option<Vec<StateCategory>>,
        response_channel: tokio::sync::oneshot::Sender<Result<StateSnapshot, PersistenceError>>,
    },

    /// Restore request
    RestoreRequest {
        snapshot: StateSnapshot,
        response_channel: tokio::sync::oneshot::Sender<Result<(), PersistenceError>>,
    },

    /// Configuration state from M02
    ConfigurationSnapshot {
        config: HashMap<String, serde_json::Value>,
        timestamp: DateTime<Utc>,
    },

    /// Metric snapshot from M04
    MetricSnapshot {
        metrics: HashMap<String, serde_json::Value>,
        timestamp: DateTime<Utc>,
    },

    /// Learning state from L3
    LearningStateUpdate {
        pathway_id: String,
        pathway_state: serde_json::Value,
        weights: Vec<f64>,
    },

    /// Service state from L2
    ServiceStateUpdate {
        service_id: String,
        state: serde_json::Value,
        health_score: f64,
    },
}
```

#### Inbound Sequence Diagram

```
                    INBOUND FLOW

    M02 Config      M04 Metrics        L2-L6 Layers
        |               |                   |
        | ConfigSnap    | MetricSnap        | save()/
        | (periodic)    | (periodic)        | transaction()
        v               v                   v
    +----------------------------------------------+
    |           State Persistence Inbox             |
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
    | Direct   |  |Transaction|  | Snapshot  |
    | Write    |  | Manager   |  | Manager   |
    +----------+  +-----------+  +-----------+
         |              |              |
         v              v              v
    +----------------------------------------------+
    |              Write-Ahead Log                  |
    +----------------------------------------------+
                        |
                        v
              +------------------+
              |  SQLite Backend  |
              +------------------+
```

### Outbound Data Flow

```
+------------------------------------------------------------------+
|                     OUTBOUND DATA TARGETS                         |
+------------------------------------------------------------------+
|                                                                    |
|         +------------------+                                       |
|         | State Persistence|                                       |
|         |  (Source)        |                                       |
|         +--------+---------+                                       |
|                  |                                                 |
|    +-------------+-------------+-------------+-------------+       |
|    |             |             |             |             |       |
|    v             v             v             v             v       |
| +------+   +------+   +------+   +------+   +------+              |
| | M02  |   | M03  |   | M04  |   | M06  |   | L2-6 |              |
| |Config|   | Log  |   |Metric|   | Res  |   |Layers|              |
| +------+   +------+   +------+   +------+   +------+              |
|    |             |             |             |             |       |
|    v             v             v             v             v       |
| Config       Operation    Storage       Resource      State        |
| Restore      Logs         Stats         Alerts        Results      |
|                                                                    |
+------------------------------------------------------------------+
```

#### Outbound Message Types

```rust
/// Messages sent by State Persistence
#[derive(Debug, Clone)]
pub enum PersistenceOutboundMessage {
    /// State loaded response
    StateLoaded {
        key: String,
        entry: Option<StateEntry>,
        from_cache: bool,
    },

    /// State saved confirmation
    StateSaved {
        key: String,
        version: u64,
        timestamp: DateTime<Utc>,
    },

    /// Transaction result
    TransactionResult {
        transaction_id: String,
        success: bool,
        operations_count: usize,
        duration_ms: u64,
    },

    /// Snapshot created notification
    SnapshotCreated {
        snapshot: StateSnapshot,
    },

    /// Restore completed notification
    RestoreCompleted {
        snapshot_id: String,
        entries_restored: u64,
        duration_ms: u64,
    },

    /// Configuration restore to M02
    ConfigurationRestore {
        config: HashMap<String, serde_json::Value>,
        version: u64,
        timestamp: DateTime<Utc>,
    },

    /// Operation log to M03
    OperationLog {
        operation: String,
        key: String,
        success: bool,
        duration_ms: u64,
    },

    /// Storage metrics to M04
    StorageMetrics {
        total_entries: u64,
        total_size_bytes: u64,
        wal_size_bytes: u64,
        cache_hit_ratio: f64,
    },

    /// Storage alert to M06
    StorageAlert {
        alert_type: StorageAlertType,
        current_value: u64,
        threshold: u64,
        message: String,
    },

    /// Health status
    HealthStatus {
        healthy: bool,
        database_ok: bool,
        wal_ok: bool,
        disk_space_ok: bool,
        last_checkpoint: DateTime<Utc>,
    },
}

/// Storage alert types
#[derive(Debug, Clone)]
pub enum StorageAlertType {
    DiskSpaceLow,
    WalSizeLarge,
    TransactionTimeout,
    DatabaseCorruption,
    ConnectionPoolExhausted,
}
```

#### Outbound Sequence Diagram

```
                    OUTBOUND FLOW

                 State Persistence
                        |
                        | Operation Complete
                        v
              +------------------+
              |  Result Router   |
              +------------------+
                        |
         +--------------+--------------+--------------+
         |              |              |              |
         v              v              v              v
    +----------+  +-----------+  +-----------+  +-----------+
    |    M02   |  |    M03    |  |    M04    |  |    M06    |
    | Config   |  |  Logging  |  |  Metrics  |  | Resource  |
    +----------+  +-----------+  +-----------+  +-----------+
         |              |              |              |
         v              v              v              v
    Config          Operation      Storage        Storage
    Restore         Logs           Stats          Alerts


    Startup Flow:

              +------------------+
              |  System Startup  |
              +--------+---------+
                       |
                       v
              +------------------+
              |  Load Persisted  |
              |  Configuration   |
              +--------+---------+
                       |
                       v
              +------------------+
              |      M02         |
              | Config Manager   |
              +------------------+
```

### Cross-Module Dependencies

```
+------------------------------------------------------------------+
|            BI-DIRECTIONAL DEPENDENCY MATRIX (M05)                 |
+------------------------------------------------------------------+
|                                                                    |
|  Module    | M05 Reads From        | M05 Writes To                |
|  ----------|------------------------|-----------------------------  |
|  M01       | -                      | Error events for             |
|            |                        | persistence                  |
|  ----------|------------------------|-----------------------------  |
|  M02       | Persistence config    | Configuration restore        |
|            |                        | on startup                   |
|  ----------|------------------------|-----------------------------  |
|  M03       | -                      | Operation logs,              |
|            |                        | audit trail                  |
|  ----------|------------------------|-----------------------------  |
|  M04       | -                      | Storage metrics,             |
|            |                        | performance stats            |
|  ----------|------------------------|-----------------------------  |
|  M06       | -                      | Storage alerts,              |
|            |                        | disk space warnings          |
|  ----------|------------------------|-----------------------------  |
|  L2        | Service state          | Service state restore        |
|            | persistence requests   |                              |
|  ----------|------------------------|-----------------------------  |
|  L3        | Learning state         | Pathway state restore        |
|            | persistence requests   |                              |
|  ----------|------------------------|-----------------------------  |
|  L4-L6     | State persistence      | State restore on startup     |
|            | requests               |                              |
|                                                                    |
+------------------------------------------------------------------+

Communication Patterns:
+------------------------------------------------------------------+
|  Pattern          | Source  | Target  | Type        | Frequency  |
|  -----------------|---------|---------|-------------|----------- |
|  State Save       | L1-L6   | M05     | Async       | Continuous |
|  State Load       | L1-L6   | M05     | Sync        | On demand  |
|  Transaction      | L1-L6   | M05     | Sync        | On demand  |
|  Config Save      | M02     | M05     | Async       | Periodic   |
|  Config Restore   | M05     | M02     | Sync        | Startup    |
|  Metric Save      | M04     | M05     | Async       | Periodic   |
|  Snapshot         | Timer   | M05     | Async       | Scheduled  |
|  Storage Metrics  | M05     | M04     | Async       | Periodic   |
|  Storage Alert    | M05     | M06     | Async       | On event   |
+------------------------------------------------------------------+

Error Propagation:
+------------------------------------------------------------------+
|  Error Source     | Propagates To       | Action                  |
|  -----------------|---------------------|------------------------ |
|  Database Error   | M01 (classify)      | Retry + Alert           |
|  WAL Corruption   | M03 (log), M06      | Recovery + Alert        |
|  Disk Full        | M06 (alert)         | Cleanup + Alert         |
|  Transaction Fail | Requester           | Rollback + Return       |
|  Migration Fail   | M03 (log)           | Halt startup            |
+------------------------------------------------------------------+
```

### Contextual Flow

```
+------------------------------------------------------------------+
|                   STATE DATA LIFECYCLE                             |
+------------------------------------------------------------------+
|                                                                    |
|  1. RECEIVE PHASE                                                  |
|     +------------------+                                           |
|     |  save() request  |  (From any module/layer)                 |
|     +--------+---------+                                           |
|              |                                                     |
|  2. VALIDATE PHASE                                                 |
|              v                                                     |
|     +------------------+                                           |
|     |  Schema Check    |  (Validate value structure)              |
|     +--------+---------+                                           |
|              |                                                     |
|  3. WAL PHASE                                                      |
|              v                                                     |
|     +------------------+                                           |
|     |  Write to WAL    |  (Durable before ack)                    |
|     +--------+---------+                                           |
|              |                                                     |
|  4. STORE PHASE                                                    |
|              v                                                     |
|     +------------------+                                           |
|     |  SQLite Write    |  (Insert/Update with version)            |
|     +--------+---------+                                           |
|              |                                                     |
|  5. ACK PHASE                                                      |
|              v                                                     |
|     +------------------+                                           |
|     |  Return Version  |  (New version number)                    |
|     +--------+---------+                                           |
|              |                                                     |
|  6. CHECKPOINT PHASE (Periodic)                                    |
|              v                                                     |
|     +------------------+                                           |
|     |  WAL Checkpoint  |  (Move WAL to main DB)                   |
|     +--------+---------+                                           |
|              |                                                     |
|  7. SNAPSHOT PHASE (Scheduled)                                     |
|              v                                                     |
|     +------------------+                                           |
|     |  Create Snapshot |  (Point-in-time backup)                  |
|     +------------------+                                           |
|                                                                    |
+------------------------------------------------------------------+
```

#### Database Schema

```sql
-- Main state table
CREATE TABLE me_state (
    key TEXT PRIMARY KEY,
    value JSONB NOT NULL,
    version INTEGER NOT NULL DEFAULT 1,
    layer TEXT NOT NULL,
    module TEXT NOT NULL,
    category TEXT NOT NULL,
    size_bytes INTEGER NOT NULL,
    checksum TEXT NOT NULL,
    compressed INTEGER NOT NULL DEFAULT 0,
    encrypted INTEGER NOT NULL DEFAULT 0,
    ttl INTEGER,
    expires_at TIMESTAMP,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Indexes
CREATE INDEX idx_me_state_layer ON me_state(layer);
CREATE INDEX idx_me_state_module ON me_state(module);
CREATE INDEX idx_me_state_category ON me_state(category);
CREATE INDEX idx_me_state_updated ON me_state(updated_at);
CREATE INDEX idx_me_state_expires ON me_state(expires_at) WHERE expires_at IS NOT NULL;

-- Version history (optional, for audit)
CREATE TABLE me_state_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    key TEXT NOT NULL,
    value JSONB NOT NULL,
    version INTEGER NOT NULL,
    operation TEXT NOT NULL, -- INSERT, UPDATE, DELETE
    changed_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Migration tracking
CREATE TABLE me_migrations (
    version INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    checksum TEXT NOT NULL,
    applied_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Snapshot tracking
CREATE TABLE me_snapshots (
    id TEXT PRIMARY KEY,
    snapshot_type TEXT NOT NULL,
    entry_count INTEGER NOT NULL,
    size_bytes INTEGER NOT NULL,
    checksum TEXT NOT NULL,
    compression TEXT,
    location TEXT NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);
```

---

## Metrics

| Metric | Type | Description |
|--------|------|-------------|
| `me_m05_operations_total` | Counter | Total operations by type |
| `me_m05_operation_duration_ms` | Histogram | Operation latency |
| `me_m05_transactions_total` | Counter | Total transactions |
| `me_m05_transaction_duration_ms` | Histogram | Transaction latency |
| `me_m05_entries_total` | Gauge | Total state entries |
| `me_m05_storage_bytes` | Gauge | Total storage size |
| `me_m05_wal_size_bytes` | Gauge | WAL file size |
| `me_m05_cache_hits_total` | Counter | Cache hits |
| `me_m05_cache_misses_total` | Counter | Cache misses |
| `me_m05_snapshots_total` | Counter | Snapshots created |
| `me_m05_checkpoints_total` | Counter | WAL checkpoints |
| `me_m05_compactions_total` | Counter | Database compactions |

---

## Error Codes

| Code | Name | Description | Severity | Recovery |
|------|------|-------------|----------|----------|
| E5001 | PERSISTENCE_DB_ERROR | Database operation failed | Error | Retry + Alert |
| E5002 | PERSISTENCE_WAL_ERROR | WAL operation failed | Critical | Recovery mode |
| E5003 | PERSISTENCE_KEY_NOT_FOUND | Key does not exist | Info | Return None |
| E5004 | PERSISTENCE_VERSION_CONFLICT | Optimistic lock conflict | Warning | Retry with refresh |
| E5005 | PERSISTENCE_TRANSACTION_FAILED | Transaction failed | Error | Rollback |
| E5006 | PERSISTENCE_SNAPSHOT_FAILED | Snapshot creation failed | Error | Alert |
| E5007 | PERSISTENCE_RESTORE_FAILED | Restore operation failed | Critical | Manual intervention |
| E5008 | PERSISTENCE_MIGRATION_FAILED | Migration failed | Critical | Halt startup |
| E5009 | PERSISTENCE_DISK_FULL | Disk space exhausted | Critical | Cleanup + Alert |
| E5010 | PERSISTENCE_CORRUPTION | Data corruption detected | Critical | Recovery mode |

---

## Navigation

| Direction | Link |
|-----------|------|
| Up | [INDEX.md](../INDEX.md) |
| Layer | [L01_FOUNDATION.md](../layers/L01_FOUNDATION.md) |
| Previous | [M04_METRICS_COLLECTOR.md](M04_METRICS_COLLECTOR.md) |
| Next | [M06_RESOURCE_MANAGER.md](M06_RESOURCE_MANAGER.md) |
| Related | [M02_CONFIGURATION_MANAGER.md](M02_CONFIGURATION_MANAGER.md) |

---

*[Back to Index](../INDEX.md) | [Layer: L01 Foundation](../layers/L01_FOUNDATION.md)*
