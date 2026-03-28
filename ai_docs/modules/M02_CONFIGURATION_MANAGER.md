# Module M02: Configuration Manager

> **M02_CONFIGURATION_MANAGER** | Centralized Configuration | Layer: L1 Foundation | [Back to Index](../INDEX.md)

---

## Navigation

| Direction | Link |
|-----------|------|
| Up | [INDEX.md](../INDEX.md) |
| Layer | [L01_FOUNDATION.md](../layers/L01_FOUNDATION.md) |
| Previous | [M01_ERROR_TAXONOMY.md](M01_ERROR_TAXONOMY.md) |
| Next | [M03_LOGGING_SYSTEM.md](M03_LOGGING_SYSTEM.md) |
| Related | [M05_STATE_PERSISTENCE.md](M05_STATE_PERSISTENCE.md) |

---

## Module Overview

The Configuration Manager (M02) provides centralized configuration management with hot-reload capability, environment variable interpolation, validation, and secret management integration. It serves as the single source of truth for all runtime configuration across the Maintenance Engine.

### Module Properties

| Property | Value |
|----------|-------|
| Module ID | M02 |
| Module Name | Configuration Manager |
| Layer | L1 (Foundation) |
| Version | 1.0 |
| Dependencies | M01 (Error Taxonomy) |
| Dependents | M03 (Logging), M04 (Metrics), M05 (State), M06 (Resource), L2-L6 |
| Criticality | Critical |
| Startup Order | 2 (after M01) |

---

## Architecture Diagram

```
+------------------------------------------------------------------+
|                    M02: Configuration Manager                      |
+------------------------------------------------------------------+
|                                                                    |
|  +-----------------+     +------------------+     +--------------+ |
|  |  Config Source  |     |   Config Cache   |     |   Watcher    | |
|  |    Loader       |---->|   (In-Memory)    |<----|   Service    | |
|  +-----------------+     +------------------+     +--------------+ |
|         |                        |                       |         |
|         v                        v                       v         |
|  +-----------------+     +------------------+     +--------------+ |
|  |   Environment   |     |   Validator      |     |  Hot-Reload  | |
|  |   Interpolator  |     |   Engine         |     |  Dispatcher  | |
|  +-----------------+     +------------------+     +--------------+ |
|         |                        |                       |         |
|         +------------+-----------+           +-----------+         |
|                      |                       |                     |
|                      v                       v                     |
|              +------------------+     +--------------+             |
|              |  Secret Manager  |     |  Subscriber  |             |
|              |  Integration     |     |  Registry    |             |
|              +------------------+     +--------------+             |
|                                                                    |
+------------------------------------------------------------------+
```

---

## Core Data Structures

### Configuration Value Types

```rust
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Configuration value supporting multiple types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum ConfigValue {
    /// String value
    String(String),
    /// Integer value
    Integer(i64),
    /// Float value
    Float(f64),
    /// Boolean value
    Boolean(bool),
    /// Array of values
    Array(Vec<ConfigValue>),
    /// Nested table/object
    Table(HashMap<String, ConfigValue>),
    /// Duration in milliseconds
    Duration(u64),
    /// Secret reference (resolved at runtime)
    Secret(SecretRef),
}

/// Reference to a secret value
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SecretRef {
    /// Secret provider (vault, env, file)
    pub provider: SecretProvider,
    /// Secret key/path
    pub key: String,
    /// Cache TTL in seconds (0 = no cache)
    pub cache_ttl: u64,
}

/// Supported secret providers
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SecretProvider {
    /// Environment variable
    Environment,
    /// HashiCorp Vault
    Vault { mount: String },
    /// Encrypted file
    EncryptedFile { path: PathBuf },
    /// AWS Secrets Manager
    AwsSecretsManager { region: String },
}

/// Configuration source metadata
#[derive(Debug, Clone)]
pub struct ConfigSource {
    /// Source identifier
    pub id: String,
    /// Source type (file, env, remote)
    pub source_type: ConfigSourceType,
    /// Path or URL
    pub location: String,
    /// Priority (higher overrides lower)
    pub priority: u32,
    /// Last loaded timestamp
    pub last_loaded: chrono::DateTime<chrono::Utc>,
    /// Checksum for change detection
    pub checksum: String,
}

/// Types of configuration sources
#[derive(Debug, Clone, PartialEq)]
pub enum ConfigSourceType {
    /// Local TOML file
    TomlFile,
    /// Local YAML file
    YamlFile,
    /// Local JSON file
    JsonFile,
    /// Environment variables
    Environment,
    /// Remote HTTP endpoint
    RemoteHttp,
    /// Consul KV store
    ConsulKv,
}

/// Configuration change event
#[derive(Debug, Clone)]
pub struct ConfigChangeEvent {
    /// Change ID (UUID)
    pub change_id: String,
    /// Timestamp of change
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Keys that changed
    pub changed_keys: Vec<String>,
    /// Source that triggered change
    pub source: ConfigSource,
    /// Previous values (for rollback)
    pub previous_values: HashMap<String, ConfigValue>,
    /// New values
    pub new_values: HashMap<String, ConfigValue>,
}

/// Validation result for configuration
#[derive(Debug, Clone)]
pub struct ValidationResult {
    /// Overall validity
    pub valid: bool,
    /// Validation errors
    pub errors: Vec<ValidationError>,
    /// Validation warnings
    pub warnings: Vec<ValidationWarning>,
    /// Schema version validated against
    pub schema_version: String,
}

/// Validation error detail
#[derive(Debug, Clone)]
pub struct ValidationError {
    /// Configuration key path
    pub key: String,
    /// Error code
    pub code: String,
    /// Human-readable message
    pub message: String,
    /// Expected value/type
    pub expected: Option<String>,
    /// Actual value/type
    pub actual: Option<String>,
}

/// Validation warning detail
#[derive(Debug, Clone)]
pub struct ValidationWarning {
    /// Configuration key path
    pub key: String,
    /// Warning code
    pub code: String,
    /// Human-readable message
    pub message: String,
    /// Suggestion for improvement
    pub suggestion: Option<String>,
}
```

### Configuration Manager State

```rust
/// Main configuration manager
pub struct ConfigManager {
    /// Cached configuration values
    cache: Arc<RwLock<HashMap<String, ConfigValue>>>,
    /// Configuration sources
    sources: Arc<RwLock<Vec<ConfigSource>>>,
    /// Configuration schema for validation
    schema: ConfigSchema,
    /// Hot-reload watcher
    watcher: Option<ConfigWatcher>,
    /// Subscriber registry for change notifications
    subscribers: Arc<RwLock<Vec<ConfigSubscriber>>>,
    /// Secret manager integration
    secret_manager: Arc<dyn SecretManager>,
    /// Error taxonomy for error classification
    error_taxonomy: Arc<ErrorTaxonomy>,
    /// Manager settings
    settings: ConfigManagerSettings,
}

/// Configuration manager settings
#[derive(Debug, Clone)]
pub struct ConfigManagerSettings {
    /// Enable hot-reload
    pub hot_reload_enabled: bool,
    /// Hot-reload check interval (ms)
    pub hot_reload_interval_ms: u64,
    /// Strict validation mode
    pub validation_strict: bool,
    /// Environment variable prefix
    pub env_prefix: String,
    /// Cache TTL for secrets (seconds)
    pub secret_cache_ttl: u64,
    /// Maximum config file size (bytes)
    pub max_file_size: usize,
}

/// Configuration watcher for hot-reload
pub struct ConfigWatcher {
    /// Watch paths
    pub watch_paths: Vec<PathBuf>,
    /// Debounce duration (ms)
    pub debounce_ms: u64,
    /// Shutdown signal
    pub shutdown: tokio::sync::watch::Sender<bool>,
}

/// Subscriber for configuration changes
pub struct ConfigSubscriber {
    /// Subscriber ID
    pub id: String,
    /// Keys to watch (empty = all)
    pub watch_keys: Vec<String>,
    /// Callback channel
    pub sender: tokio::sync::mpsc::Sender<ConfigChangeEvent>,
}
```

---

## Public API

```rust
impl ConfigManager {
    /// Create new configuration manager
    pub fn new(settings: ConfigManagerSettings) -> Result<Self, ConfigError>;

    /// Load configuration from path
    pub async fn load(&mut self, path: &Path) -> Result<(), ConfigError>;

    /// Load configuration from multiple sources
    pub async fn load_sources(&mut self, sources: Vec<ConfigSource>) -> Result<(), ConfigError>;

    /// Get typed configuration value
    pub fn get<T: DeserializeOwned>(&self, key: &str) -> Option<T>;

    /// Get configuration value with default
    pub fn get_or<T: DeserializeOwned>(&self, key: &str, default: T) -> T;

    /// Get required configuration value (error if missing)
    pub fn get_required<T: DeserializeOwned>(&self, key: &str) -> Result<T, ConfigError>;

    /// Set configuration value (runtime override)
    pub async fn set<T: Serialize>(&mut self, key: &str, value: T) -> Result<(), ConfigError>;

    /// Delete configuration key
    pub async fn delete(&mut self, key: &str) -> Result<Option<ConfigValue>, ConfigError>;

    /// Check if key exists
    pub fn contains(&self, key: &str) -> bool;

    /// Get all keys matching pattern
    pub fn keys(&self, pattern: &str) -> Vec<String>;

    /// Validate current configuration
    pub fn validate(&self) -> ValidationResult;

    /// Validate configuration against schema
    pub fn validate_against(&self, schema: &ConfigSchema) -> ValidationResult;

    /// Start hot-reload watcher
    pub async fn watch(&mut self) -> Result<ConfigWatcher, ConfigError>;

    /// Stop hot-reload watcher
    pub async fn stop_watching(&mut self) -> Result<(), ConfigError>;

    /// Subscribe to configuration changes
    pub async fn subscribe(&mut self, subscriber: ConfigSubscriber) -> Result<String, ConfigError>;

    /// Unsubscribe from configuration changes
    pub async fn unsubscribe(&mut self, subscriber_id: &str) -> Result<(), ConfigError>;

    /// Reload configuration from all sources
    pub async fn reload(&mut self) -> Result<ConfigChangeEvent, ConfigError>;

    /// Export configuration to file
    pub async fn export(&self, path: &Path, format: ConfigSourceType) -> Result<(), ConfigError>;

    /// Get configuration snapshot
    pub fn snapshot(&self) -> ConfigSnapshot;

    /// Restore configuration from snapshot
    pub async fn restore(&mut self, snapshot: ConfigSnapshot) -> Result<(), ConfigError>;

    /// Get environment interpolated value
    pub fn interpolate(&self, template: &str) -> Result<String, ConfigError>;

    /// Resolve secret reference
    pub async fn resolve_secret(&self, secret_ref: &SecretRef) -> Result<String, ConfigError>;
}

/// Configuration schema for validation
impl ConfigSchema {
    /// Load schema from file
    pub fn load(path: &Path) -> Result<Self, ConfigError>;

    /// Validate value against schema
    pub fn validate_value(&self, key: &str, value: &ConfigValue) -> ValidationResult;

    /// Get required keys
    pub fn required_keys(&self) -> Vec<String>;

    /// Get key type specification
    pub fn key_spec(&self, key: &str) -> Option<KeySpec>;
}
```

---

## Configuration

### Default Configuration (TOML)

```toml
[foundation.config]
version = "1.0"
hot_reload_enabled = true
hot_reload_interval_ms = 5000
validation_strict = true
env_prefix = "ME_"
secret_cache_ttl = 300
max_file_size = 10485760  # 10MB

[foundation.config.sources]
# Primary configuration file
primary = "/etc/maintenance-engine/config.toml"
# Override directory (files merged in alphabetical order)
override_dir = "/etc/maintenance-engine/config.d/"
# Environment variable prefix for overrides
environment_prefix = "ME_"

[foundation.config.schema]
# Path to JSON schema for validation
path = "/etc/maintenance-engine/schema/config.schema.json"
# Schema version
version = "1.0.0"

[foundation.config.secrets]
# Default secret provider
provider = "environment"

[foundation.config.secrets.vault]
# Vault address (if using Vault)
address = "https://vault.example.com:8200"
# Vault mount path
mount = "secret"
# Authentication method
auth_method = "kubernetes"

[foundation.config.watch]
# Paths to watch for changes
paths = [
    "/etc/maintenance-engine/config.toml",
    "/etc/maintenance-engine/config.d/"
]
# Debounce duration for file changes
debounce_ms = 1000
```

### Layer Configuration Example

```toml
# Layer-specific configurations

[layer.L1]
enabled = true
startup_order = 1

[layer.L1.M1]
tensor_dimensions = 11
similarity_threshold = 0.85
clustering_algorithm = "dbscan"

[layer.L1.M2]
# Self-referential: M02 manages its own config
config_path = "/etc/maintenance-engine/config.toml"
hot_reload = true

[layer.L1.M3]
level = "info"
format = "json"
outputs = ["stdout", "file"]
file_path = "/var/log/maintenance-engine/engine.log"

[layer.L1.M4]
export_interval_ms = 15000
retention_hours = 168
prometheus_port = 9090

[layer.L1.M5]
database_url = "${ME_DATABASE_URL}"  # Environment interpolation
pool_size = 20
migration_auto = true

[layer.L1.M6]
max_memory_mb = 2048
max_connections = 100
max_file_descriptors = 10000

# Service-specific configurations
[services.synthex]
host = "localhost"
port = 8090
timeout_ms = 5000
retry_count = 3

[services.san_k7]
host = "localhost"
port = 8100
timeout_ms = 10000
```

---

## Bi-Directional Data Flow

### Inbound Data Flow

```
+------------------------------------------------------------------+
|                     INBOUND DATA SOURCES                          |
+------------------------------------------------------------------+
|                                                                    |
|  +-------------+     +-------------+     +-------------+           |
|  |   M01       |     |   M05       |     |   External  |           |
|  |   Error     |     |   State     |     |   Config    |           |
|  |   Taxonomy  |     | Persistence |     |   Sources   |           |
|  +------+------+     +------+------+     +------+------+           |
|         |                   |                   |                  |
|         v                   v                   v                  |
|    Error Codes        State Restore       File/HTTP/Consul         |
|    for Config         Configuration       Config Updates           |
|    Validation         on Startup          and Overrides            |
|         |                   |                   |                  |
|         +--------+----------+----------+--------+                  |
|                  |                      |                          |
|                  v                      v                          |
|         +------------------+    +------------------+               |
|         |  ConfigManager   |    |  ConfigManager   |               |
|         |  .validate()     |    |  .load()         |               |
|         +------------------+    +------------------+               |
|                                                                    |
+------------------------------------------------------------------+
```

#### Inbound Message Types

```rust
/// Messages received by Configuration Manager
#[derive(Debug, Clone)]
pub enum ConfigInboundMessage {
    /// Request to load configuration from source
    LoadRequest {
        source: ConfigSource,
        priority: u32,
        merge_strategy: MergeStrategy,
    },

    /// Request to set configuration value
    SetRequest {
        key: String,
        value: ConfigValue,
        source: String,  // Who is setting
        persist: bool,   // Write to file
    },

    /// Request to get configuration value
    GetRequest {
        key: String,
        response_channel: tokio::sync::oneshot::Sender<Option<ConfigValue>>,
    },

    /// Request to validate configuration
    ValidateRequest {
        schema_version: Option<String>,
        response_channel: tokio::sync::oneshot::Sender<ValidationResult>,
    },

    /// State restoration from M05
    StateRestore {
        snapshot: ConfigSnapshot,
        timestamp: chrono::DateTime<chrono::Utc>,
    },

    /// Error taxonomy update from M01
    ErrorTaxonomyUpdate {
        error_codes: Vec<ErrorCode>,
        validation_rules: Vec<ValidationRule>,
    },

    /// Hot-reload trigger from file watcher
    HotReloadTrigger {
        changed_paths: Vec<PathBuf>,
        checksum: String,
    },

    /// Secret resolution request
    SecretResolutionRequest {
        secret_ref: SecretRef,
        response_channel: tokio::sync::oneshot::Sender<Result<String, ConfigError>>,
    },
}

/// Merge strategy for configuration sources
#[derive(Debug, Clone, PartialEq)]
pub enum MergeStrategy {
    /// Replace entire configuration
    Replace,
    /// Deep merge (nested values merged)
    DeepMerge,
    /// Shallow merge (top-level only)
    ShallowMerge,
    /// Append to arrays, merge tables
    AppendMerge,
}
```

#### Inbound Sequence Diagram

```
                    INBOUND FLOW

    M01 Error          M05 State         FileSystem
    Taxonomy          Persistence         Watcher
        |                  |                  |
        | ErrorTaxonomy    | StateRestore     | HotReloadTrigger
        | Update           | (startup)        | (runtime)
        v                  v                  v
    +------------------+------------------+------------------+
    |              ConfigManager Message Queue               |
    +--------------------------------------------------------+
                              |
                              v
                    +------------------+
                    |   Message        |
                    |   Router         |
                    +------------------+
                              |
            +-----------------+-----------------+
            |                 |                 |
            v                 v                 v
    +-------------+   +-------------+   +-------------+
    | Validation  |   |   Cache     |   |   Watcher   |
    |   Engine    |   |   Update    |   |   Handler   |
    +-------------+   +-------------+   +-------------+
            |                 |                 |
            v                 v                 v
    +------------------+------------------+------------------+
    |              Configuration State Updated               |
    +--------------------------------------------------------+
```

### Outbound Data Flow

```
+------------------------------------------------------------------+
|                     OUTBOUND DATA TARGETS                         |
+------------------------------------------------------------------+
|                                                                    |
|         +------------------+                                       |
|         |  ConfigManager   |                                       |
|         |  (Source)        |                                       |
|         +--------+---------+                                       |
|                  |                                                 |
|    +-------------+-------------+-------------+-------------+       |
|    |             |             |             |             |       |
|    v             v             v             v             v       |
| +------+   +------+   +------+   +------+   +------+              |
| |  M03 |   |  M04 |   |  M05 |   |  M06 |   | L2-L6|              |
| | Log  |   |Metric|   |State |   | Res  |   |Layers|              |
| +------+   +------+   +------+   +------+   +------+              |
|    |             |             |             |             |       |
|    v             v             v             v             v       |
| LogLevel     Metric       Config        Resource      Layer        |
| Config       Config       Snapshot      Limits        Configs      |
|                                                                    |
+------------------------------------------------------------------+
```

#### Outbound Message Types

```rust
/// Messages sent by Configuration Manager
#[derive(Debug, Clone)]
pub enum ConfigOutboundMessage {
    /// Configuration change notification
    ConfigChanged {
        event: ConfigChangeEvent,
        affected_modules: Vec<String>,
    },

    /// Configuration value response
    ConfigValue {
        key: String,
        value: Option<ConfigValue>,
        source: ConfigSource,
        timestamp: chrono::DateTime<chrono::Utc>,
    },

    /// Validation result notification
    ValidationComplete {
        result: ValidationResult,
        config_version: String,
    },

    /// State snapshot for persistence
    StateSnapshot {
        snapshot: ConfigSnapshot,
        checkpoint_id: String,
    },

    /// Log configuration update to M03
    LogConfigUpdate {
        log_level: String,
        log_format: String,
        log_outputs: Vec<String>,
    },

    /// Metrics configuration update to M04
    MetricsConfigUpdate {
        export_interval_ms: u64,
        retention_hours: u64,
        enabled_metrics: Vec<String>,
    },

    /// Resource limits update to M06
    ResourceLimitsUpdate {
        max_memory_mb: usize,
        max_connections: usize,
        max_file_descriptors: usize,
    },

    /// Layer configuration broadcast
    LayerConfigBroadcast {
        layer_id: String,
        config: HashMap<String, ConfigValue>,
        version: String,
    },

    /// Error notification to M01
    ConfigError {
        error: ConfigError,
        context: HashMap<String, String>,
    },
}
```

#### Outbound Sequence Diagram

```
                    OUTBOUND FLOW

                    ConfigManager
                         |
                         | ConfigChanged Event
                         v
              +--------------------+
              |   Change Notifier  |
              +--------------------+
                         |
         +-------+-------+-------+-------+
         |       |       |       |       |
         v       v       v       v       v
      +-----+ +-----+ +-----+ +-----+ +------+
      | M03 | | M04 | | M05 | | M06 | | L2-6 |
      +-----+ +-----+ +-----+ +-----+ +------+
         |       |       |       |       |
         v       v       v       v       v
      Log     Metric  Persist Resource Layer
      Level   Export  State   Limits   Update
      Update  Config  Config  Update
```

### Cross-Module Dependencies

```
+------------------------------------------------------------------+
|            BI-DIRECTIONAL DEPENDENCY MATRIX (M02)                 |
+------------------------------------------------------------------+
|                                                                    |
|  Module    | M02 Reads From        | M02 Writes To                |
|  ----------|------------------------|-----------------------------  |
|  M01       | Error codes,          | Config errors for            |
|            | validation rules       | classification               |
|  ----------|------------------------|-----------------------------  |
|  M03       | -                      | Log level, format,           |
|            |                        | output destinations          |
|  ----------|------------------------|-----------------------------  |
|  M04       | -                      | Metrics export config,       |
|            |                        | retention policies           |
|  ----------|------------------------|-----------------------------  |
|  M05       | Persisted config       | Config snapshots for         |
|            | state on startup       | persistence                  |
|  ----------|------------------------|-----------------------------  |
|  M06       | -                      | Resource limits,             |
|            |                        | pool sizes                   |
|  ----------|------------------------|-----------------------------  |
|  L2-L6     | -                      | Layer-specific               |
|            |                        | configurations               |
|                                                                    |
+------------------------------------------------------------------+

Communication Patterns:
+------------------------------------------------------------------+
|  Pattern          | Source  | Target  | Type        | Frequency  |
|  -----------------|---------|---------|-------------|----------- |
|  Config Load      | M05     | M02     | Sync        | Startup    |
|  Config Change    | M02     | M03-M06 | Async       | On change  |
|  Validation       | M01     | M02     | Sync        | On load    |
|  State Snapshot   | M02     | M05     | Async       | Periodic   |
|  Error Report     | M02     | M01     | Async       | On error   |
|  Hot Reload       | FS      | M02     | Event       | On modify  |
+------------------------------------------------------------------+

Error Propagation:
+------------------------------------------------------------------+
|  Error Source     | Propagates To       | Action                  |
|  -----------------|---------------------|------------------------ |
|  Invalid Config   | M01 (classify)      | Log + Alert             |
|  Missing Required | M03 (log)           | Startup Fail            |
|  Secret Failure   | M01 (classify)      | Retry + Fallback        |
|  Schema Mismatch  | M03 (log), M04      | Warning + Continue      |
+------------------------------------------------------------------+
```

### Contextual Flow

```
+------------------------------------------------------------------+
|              CONFIGURATION DATA LIFECYCLE                          |
+------------------------------------------------------------------+
|                                                                    |
|  1. LOAD PHASE                                                     |
|     +------------------+                                           |
|     | External Source  |  (File, Env, Remote)                      |
|     +--------+---------+                                           |
|              |                                                     |
|              v                                                     |
|     +------------------+                                           |
|     |     Parser       |  (TOML/YAML/JSON)                        |
|     +--------+---------+                                           |
|              |                                                     |
|              v                                                     |
|     +------------------+                                           |
|     |   Interpolator   |  (Env vars, secrets)                     |
|     +--------+---------+                                           |
|              |                                                     |
|  2. VALIDATE PHASE                                                 |
|              v                                                     |
|     +------------------+                                           |
|     |    Validator     |  (Schema check, M01 rules)               |
|     +--------+---------+                                           |
|              |                                                     |
|              v                                                     |
|     +------------------+                                           |
|     |   Merge Engine   |  (Priority-based merge)                  |
|     +--------+---------+                                           |
|              |                                                     |
|  3. CACHE PHASE                                                    |
|              v                                                     |
|     +------------------+                                           |
|     |   Config Cache   |  (In-memory, typed)                      |
|     +--------+---------+                                           |
|              |                                                     |
|  4. DISTRIBUTE PHASE                                               |
|              v                                                     |
|     +------------------+                                           |
|     | Change Notifier  |  (Broadcast to subscribers)              |
|     +--------+---------+                                           |
|              |                                                     |
|              +-------+-------+-------+                             |
|              |       |       |       |                             |
|              v       v       v       v                             |
|            M03     M04     M05     M06                             |
|                                                                    |
|  5. PERSIST PHASE (Periodic)                                       |
|     +------------------+                                           |
|     |   State Snap     |-----> M05 (State Persistence)            |
|     +------------------+                                           |
|                                                                    |
+------------------------------------------------------------------+
```

#### State Machine

```
+------------------------------------------------------------------+
|                CONFIGURATION MANAGER STATE MACHINE                 |
+------------------------------------------------------------------+
|                                                                    |
|                      +-------------+                               |
|                      | UNLOADED    |                               |
|                      +------+------+                               |
|                             |                                      |
|                             | load()                               |
|                             v                                      |
|                      +-------------+                               |
|                      |  LOADING    |                               |
|                      +------+------+                               |
|                             |                                      |
|              +--------------+--------------+                       |
|              |                             |                       |
|              | success                     | failure               |
|              v                             v                       |
|       +-------------+              +-------------+                 |
|       |  VALIDATING |              |   ERROR     |                 |
|       +------+------+              +------+------+                 |
|              |                            |                        |
|   +----------+----------+                 | retry                  |
|   |                     |                 v                        |
|   | valid          invalid         +-------------+                 |
|   v                     v          |  RETRYING   |                 |
| +-------------+  +-------------+   +------+------+                 |
| |   READY     |  | DEGRADED    |          |                        |
| +------+------+  +------+------+          +---------+              |
|        |                |                           |              |
|        | hot_reload     | partial_reload            |              |
|        v                v                           |              |
|   +-------------+  +-------------+                  |              |
|   |  RELOADING  |->| VALIDATING  |<-----------------+              |
|   +-------------+  +-------------+                                 |
|        |                                                           |
|        | stop()                                                    |
|        v                                                           |
|   +-------------+                                                  |
|   |  SHUTDOWN   |                                                  |
|   +-------------+                                                  |
|                                                                    |
+------------------------------------------------------------------+
```

---

## Metrics

| Metric | Type | Description |
|--------|------|-------------|
| `me_m02_config_loads_total` | Counter | Total configuration loads |
| `me_m02_config_reloads_total` | Counter | Hot-reload count |
| `me_m02_config_reload_duration_ms` | Histogram | Reload latency |
| `me_m02_config_keys_total` | Gauge | Total configuration keys |
| `me_m02_validation_errors_total` | Counter | Validation failures |
| `me_m02_secret_resolutions_total` | Counter | Secret resolutions |
| `me_m02_secret_resolution_duration_ms` | Histogram | Secret resolution latency |
| `me_m02_subscribers_active` | Gauge | Active change subscribers |
| `me_m02_change_notifications_total` | Counter | Change notifications sent |
| `me_m02_source_health` | Gauge | Config source health (0-1) |

---

## Error Codes

| Code | Name | Description | Severity | Recovery |
|------|------|-------------|----------|----------|
| E2001 | CONFIG_FILE_NOT_FOUND | Configuration file does not exist | Error | Use defaults |
| E2002 | CONFIG_PARSE_ERROR | Failed to parse configuration | Error | Reject load |
| E2003 | CONFIG_VALIDATION_FAILED | Configuration failed validation | Error | Use previous |
| E2004 | CONFIG_SCHEMA_MISMATCH | Schema version mismatch | Warning | Attempt load |
| E2005 | CONFIG_SECRET_RESOLUTION_FAILED | Failed to resolve secret | Error | Retry/Fail |
| E2006 | CONFIG_ENV_INTERPOLATION_FAILED | Environment variable not found | Warning | Use literal |
| E2007 | CONFIG_HOT_RELOAD_FAILED | Hot-reload failed | Warning | Keep current |
| E2008 | CONFIG_PERMISSION_DENIED | File permission denied | Error | Alert |
| E2009 | CONFIG_SOURCE_UNAVAILABLE | Remote config source unavailable | Warning | Use cached |
| E2010 | CONFIG_MERGE_CONFLICT | Conflicting configuration values | Warning | Priority wins |

---

## Navigation

| Direction | Link |
|-----------|------|
| Up | [INDEX.md](../INDEX.md) |
| Layer | [L01_FOUNDATION.md](../layers/L01_FOUNDATION.md) |
| Previous | [M01_ERROR_TAXONOMY.md](M01_ERROR_TAXONOMY.md) |
| Next | [M03_LOGGING_SYSTEM.md](M03_LOGGING_SYSTEM.md) |
| Related | [M05_STATE_PERSISTENCE.md](M05_STATE_PERSISTENCE.md) |

---

*[Back to Index](../INDEX.md) | [Layer: L01 Foundation](../layers/L01_FOUNDATION.md)*
