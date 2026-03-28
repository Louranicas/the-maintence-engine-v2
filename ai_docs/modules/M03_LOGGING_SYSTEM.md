# Module M03: Logging System

> **M03_LOGGING_SYSTEM** | Structured Logging | Layer: L1 Foundation | [Back to Index](../INDEX.md)

---

## Navigation

| Direction | Link |
|-----------|------|
| Up | [INDEX.md](../INDEX.md) |
| Layer | [L01_FOUNDATION.md](../layers/L01_FOUNDATION.md) |
| Previous | [M02_CONFIGURATION_MANAGER.md](M02_CONFIGURATION_MANAGER.md) |
| Next | [M04_METRICS_COLLECTOR.md](M04_METRICS_COLLECTOR.md) |
| Related | [M01_ERROR_TAXONOMY.md](M01_ERROR_TAXONOMY.md), [M04_METRICS_COLLECTOR.md](M04_METRICS_COLLECTOR.md) |

---

## Module Overview

The Logging System (M03) provides structured, correlated logging across all layers and modules of the Maintenance Engine. It supports multiple output destinations, log level filtering, async writing, and integration with the Error Taxonomy (M01) for semantic error classification.

### Module Properties

| Property | Value |
|----------|-------|
| Module ID | M03 |
| Module Name | Logging System |
| Layer | L1 (Foundation) |
| Version | 1.0 |
| Dependencies | M01 (Error Taxonomy), M02 (Configuration) |
| Dependents | M04 (Metrics), L2-L6 (All Layers) |
| Criticality | Critical |
| Startup Order | 3 (after M02) |

---

## Architecture Diagram

```
+------------------------------------------------------------------+
|                      M03: Logging System                          |
+------------------------------------------------------------------+
|                                                                    |
|  +----------------+     +------------------+     +---------------+ |
|  |  Log Ingester  |     |   Log Processor  |     |  Log Router   | |
|  |  (Async Queue) |---->|   (Format/Enrich)|---->|  (Dispatch)   | |
|  +----------------+     +------------------+     +-------+-------+ |
|         ^                       |                       |         |
|         |                       v                       |         |
|  +----------------+     +------------------+            |         |
|  | Correlation ID |     |   M01 Error      |            |         |
|  |   Generator    |     |   Classifier     |            |         |
|  +----------------+     +------------------+            |         |
|                                                         |         |
|         +-----------------------------------------------+         |
|         |                       |                       |         |
|         v                       v                       v         |
|  +----------------+     +------------------+     +---------------+ |
|  |  Stdout/Stderr |     |   File Writer    |     |   Syslog      | |
|  |    Output      |     |   (Rotating)     |     |   Forwarder   | |
|  +----------------+     +------------------+     +---------------+ |
|         |                       |                       |         |
|         v                       v                       v         |
|  +----------------+     +------------------+     +---------------+ |
|  |  Console       |     |   Log Files      |     |   Remote      | |
|  |  Display       |     |   (Compressed)   |     |   Collector   | |
|  +----------------+     +------------------+     +---------------+ |
|                                                                    |
+------------------------------------------------------------------+
```

---

## Core Data Structures

### Log Entry and Fields

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Log severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[repr(u8)]
pub enum LogLevel {
    /// Detailed debugging information
    Trace = 0,
    /// Debugging information
    Debug = 1,
    /// Informational messages
    Info = 2,
    /// Warning conditions
    Warn = 3,
    /// Error conditions
    Error = 4,
    /// Fatal/critical errors
    Fatal = 5,
}

impl LogLevel {
    /// Convert from string
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "trace" => Some(LogLevel::Trace),
            "debug" => Some(LogLevel::Debug),
            "info" => Some(LogLevel::Info),
            "warn" | "warning" => Some(LogLevel::Warn),
            "error" => Some(LogLevel::Error),
            "fatal" | "critical" => Some(LogLevel::Fatal),
            _ => None,
        }
    }
}

/// Correlation context for distributed tracing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorrelationContext {
    /// Unique correlation ID (trace ID)
    pub correlation_id: String,
    /// Span ID within trace
    pub span_id: String,
    /// Parent span ID (if any)
    pub parent_span_id: Option<String>,
    /// Trace flags
    pub trace_flags: u8,
    /// Baggage items (propagated context)
    pub baggage: HashMap<String, String>,
}

impl CorrelationContext {
    /// Generate new correlation context
    pub fn new() -> Self {
        Self {
            correlation_id: Uuid::new_v4().to_string(),
            span_id: Uuid::new_v4().to_string()[..16].to_string(),
            parent_span_id: None,
            trace_flags: 1, // Sampled
            baggage: HashMap::new(),
        }
    }

    /// Create child span
    pub fn child(&self) -> Self {
        Self {
            correlation_id: self.correlation_id.clone(),
            span_id: Uuid::new_v4().to_string()[..16].to_string(),
            parent_span_id: Some(self.span_id.clone()),
            trace_flags: self.trace_flags,
            baggage: self.baggage.clone(),
        }
    }
}

/// Structured log fields
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LogFields {
    /// Key-value pairs for structured logging
    pub fields: HashMap<String, LogValue>,
}

/// Log field value types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum LogValue {
    String(String),
    Integer(i64),
    Float(f64),
    Boolean(bool),
    Array(Vec<LogValue>),
    Object(HashMap<String, LogValue>),
    Null,
}

impl LogFields {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert<K: Into<String>, V: Into<LogValue>>(&mut self, key: K, value: V) -> &mut Self {
        self.fields.insert(key.into(), value.into());
        self
    }

    pub fn with<K: Into<String>, V: Into<LogValue>>(mut self, key: K, value: V) -> Self {
        self.fields.insert(key.into(), value.into());
        self
    }
}

/// Complete log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    /// Timestamp of log event
    pub timestamp: DateTime<Utc>,
    /// Log severity level
    pub level: LogLevel,
    /// Layer producing log
    pub layer: String,
    /// Module producing log
    pub module: String,
    /// Component within module
    pub component: Option<String>,
    /// Correlation context
    pub correlation: CorrelationContext,
    /// Log message
    pub message: String,
    /// Structured fields
    pub fields: LogFields,
    /// Error vector (if error log, from M01)
    pub error_vector: Option<[f32; 11]>,
    /// Error code (if error log)
    pub error_code: Option<String>,
    /// Source location
    pub source: LogSource,
}

/// Source code location
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogSource {
    /// File path
    pub file: String,
    /// Line number
    pub line: u32,
    /// Function name (if available)
    pub function: Option<String>,
}

/// Log output destination configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogOutput {
    /// Output type
    pub output_type: LogOutputType,
    /// Minimum level for this output
    pub min_level: LogLevel,
    /// Output format
    pub format: LogFormat,
    /// Filter expression (optional)
    pub filter: Option<String>,
    /// Buffer size
    pub buffer_size: usize,
    /// Flush interval (ms)
    pub flush_interval_ms: u64,
}

/// Types of log outputs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LogOutputType {
    /// Standard output
    Stdout,
    /// Standard error
    Stderr,
    /// File output
    File {
        path: String,
        max_size_mb: usize,
        max_files: usize,
        compress: bool,
    },
    /// Syslog output
    Syslog {
        facility: String,
        hostname: Option<String>,
    },
    /// TCP socket
    Tcp {
        address: String,
        port: u16,
        tls: bool,
    },
    /// UDP socket
    Udp {
        address: String,
        port: u16,
    },
}

/// Log output formats
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LogFormat {
    /// JSON format
    Json,
    /// Compact JSON (single line)
    JsonCompact,
    /// Human-readable text
    Text,
    /// Logfmt key=value format
    Logfmt,
    /// Custom format template
    Custom(String),
}

/// Logger instance configuration
#[derive(Debug, Clone)]
pub struct LoggerConfig {
    /// Global minimum level
    pub global_level: LogLevel,
    /// Per-module level overrides
    pub module_levels: HashMap<String, LogLevel>,
    /// Output destinations
    pub outputs: Vec<LogOutput>,
    /// Include source location
    pub include_source: bool,
    /// Include error vectors
    pub include_error_vectors: bool,
    /// Async queue size
    pub queue_size: usize,
    /// Worker thread count
    pub worker_threads: usize,
}
```

### Logger State

```rust
use std::sync::Arc;
use tokio::sync::mpsc;

/// Main logger instance
pub struct Logger {
    /// Logger configuration
    config: Arc<LoggerConfig>,
    /// Correlation context (thread-local style)
    correlation: CorrelationContext,
    /// Layer identifier
    layer: String,
    /// Module identifier
    module: String,
    /// Component identifier
    component: Option<String>,
    /// Log entry sender (async queue)
    sender: mpsc::Sender<LogEntry>,
    /// Error taxonomy for error classification
    error_taxonomy: Arc<ErrorTaxonomy>,
}

/// Logger registry for managing loggers
pub struct LoggerRegistry {
    /// Root logger configuration
    config: Arc<LoggerConfig>,
    /// Log processing pipeline
    pipeline: Arc<LogPipeline>,
    /// Active loggers
    loggers: HashMap<String, Logger>,
    /// Output writers
    writers: Vec<Arc<dyn LogWriter>>,
    /// Metrics collector reference
    metrics: Option<Arc<MetricsCollector>>,
}

/// Log processing pipeline
pub struct LogPipeline {
    /// Input channel
    receiver: mpsc::Receiver<LogEntry>,
    /// Processors (enrichment, filtering)
    processors: Vec<Box<dyn LogProcessor>>,
    /// Routers (output dispatch)
    routers: Vec<Box<dyn LogRouter>>,
    /// Shutdown signal
    shutdown: tokio::sync::watch::Receiver<bool>,
}

/// Trait for log processors
pub trait LogProcessor: Send + Sync {
    /// Process log entry (may modify or filter)
    fn process(&self, entry: &mut LogEntry) -> bool;
}

/// Trait for log routers
pub trait LogRouter: Send + Sync {
    /// Route log entry to appropriate outputs
    fn route(&self, entry: &LogEntry, outputs: &[Arc<dyn LogWriter>]);
}

/// Trait for log writers
#[async_trait::async_trait]
pub trait LogWriter: Send + Sync {
    /// Write log entry
    async fn write(&self, entry: &LogEntry) -> Result<(), LogError>;
    /// Flush pending writes
    async fn flush(&self) -> Result<(), LogError>;
    /// Check if writer accepts entry
    fn accepts(&self, entry: &LogEntry) -> bool;
}
```

---

## Public API

```rust
impl Logger {
    /// Create new logger for module
    pub fn new(layer: &str, module: &str) -> Self;

    /// Create logger with component
    pub fn with_component(layer: &str, module: &str, component: &str) -> Self;

    /// Set correlation context
    pub fn with_correlation(&self, correlation: CorrelationContext) -> Self;

    /// Create child logger with child span
    pub fn child(&self) -> Self;

    /// Log trace message
    pub fn trace(&self, message: &str, fields: LogFields);

    /// Log debug message
    pub fn debug(&self, message: &str, fields: LogFields);

    /// Log info message
    pub fn info(&self, message: &str, fields: LogFields);

    /// Log warning message
    pub fn warn(&self, message: &str, fields: LogFields);

    /// Log error message (with optional error for M01 classification)
    pub fn error(&self, message: &str, fields: LogFields, error: Option<&dyn std::error::Error>);

    /// Log fatal message
    pub fn fatal(&self, message: &str, fields: LogFields, error: Option<&dyn std::error::Error>);

    /// Log with explicit level
    pub fn log(&self, level: LogLevel, message: &str, fields: LogFields);

    /// Check if level is enabled
    pub fn is_enabled(&self, level: LogLevel) -> bool;

    /// Get current correlation context
    pub fn correlation(&self) -> &CorrelationContext;

    /// Create span for operation timing
    pub fn span(&self, name: &str) -> LogSpan;
}

/// Log span for timing operations
impl LogSpan {
    /// Start the span
    pub fn start(&mut self);

    /// End the span and log duration
    pub fn end(&mut self);

    /// Add field to span
    pub fn set(&mut self, key: &str, value: impl Into<LogValue>);

    /// Mark span as error
    pub fn set_error(&mut self, error: &dyn std::error::Error);
}

impl LoggerRegistry {
    /// Create new registry
    pub fn new(config: LoggerConfig) -> Result<Self, LogError>;

    /// Get or create logger
    pub fn logger(&mut self, layer: &str, module: &str) -> Logger;

    /// Update configuration (hot-reload)
    pub fn update_config(&mut self, config: LoggerConfig) -> Result<(), LogError>;

    /// Set global log level
    pub fn set_level(&mut self, level: LogLevel);

    /// Set module-specific log level
    pub fn set_module_level(&mut self, module: &str, level: LogLevel);

    /// Flush all outputs
    pub async fn flush(&self) -> Result<(), LogError>;

    /// Shutdown logging system
    pub async fn shutdown(&mut self) -> Result<(), LogError>;

    /// Get log statistics
    pub fn statistics(&self) -> LogStatistics;
}
```

---

## Configuration

```toml
[foundation.logging]
version = "1.0"

[foundation.logging.global]
# Global minimum log level
level = "info"
# Include source file/line
include_source = true
# Include error vectors from M01
include_error_vectors = true
# Async queue size
queue_size = 10000
# Worker threads
worker_threads = 2

[foundation.logging.levels]
# Per-module level overrides
"L1.M01" = "debug"
"L1.M02" = "info"
"L3" = "debug"  # Learning layer verbose

[[foundation.logging.outputs]]
type = "stdout"
min_level = "info"
format = "text"

[[foundation.logging.outputs]]
type = "file"
min_level = "debug"
format = "json"
path = "/var/log/maintenance-engine/engine.log"
max_size_mb = 100
max_files = 10
compress = true
buffer_size = 8192
flush_interval_ms = 1000

[[foundation.logging.outputs]]
type = "file"
min_level = "error"
format = "json"
path = "/var/log/maintenance-engine/error.log"
max_size_mb = 50
max_files = 5
compress = true

[[foundation.logging.outputs]]
type = "syslog"
min_level = "warn"
format = "text"
facility = "local0"

[foundation.logging.format]
# JSON format settings
timestamp_format = "rfc3339"
flatten_fields = false
include_level_name = true
include_level_number = false
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
|  | All Layers  |   |    M01      |   |    M02      |              |
|  | L1-L6       |   |   Error     |   |   Config    |              |
|  | (Log Calls) |   |  Taxonomy   |   |   Manager   |              |
|  +------+------+   +------+------+   +------+------+              |
|         |                 |                 |                      |
|         v                 v                 v                      |
|    Log Messages      Error Vectors     Config Updates              |
|    (trace-fatal)     (11D tensor)      (levels, outputs)          |
|         |                 |                 |                      |
|         +--------+--------+--------+--------+                      |
|                  |                 |                               |
|                  v                 v                               |
|         +------------------+  +------------------+                 |
|         |   Log Ingester   |  |  Config Handler  |                 |
|         |   (Async Queue)  |  |  (Hot Reload)    |                 |
|         +------------------+  +------------------+                 |
|                                                                    |
+------------------------------------------------------------------+
```

#### Inbound Message Types

```rust
/// Messages received by Logging System
#[derive(Debug, Clone)]
pub enum LogInboundMessage {
    /// Log entry from any module/layer
    LogEntry {
        entry: LogEntry,
    },

    /// Configuration update from M02
    ConfigUpdate {
        config: LoggerConfig,
        timestamp: DateTime<Utc>,
    },

    /// Error classification from M01
    ErrorClassification {
        error_code: String,
        error_vector: [f32; 11],
        context: ErrorContext,
    },

    /// Correlation context propagation
    CorrelationPropagate {
        correlation: CorrelationContext,
        target_modules: Vec<String>,
    },

    /// Flush request
    FlushRequest {
        outputs: Option<Vec<String>>,
        response_channel: tokio::sync::oneshot::Sender<Result<(), LogError>>,
    },

    /// Level change request
    LevelChangeRequest {
        module: Option<String>,
        new_level: LogLevel,
    },

    /// Statistics request
    StatsRequest {
        response_channel: tokio::sync::oneshot::Sender<LogStatistics>,
    },
}

/// Error context from M01
#[derive(Debug, Clone)]
pub struct ErrorContext {
    /// Error category
    pub category: String,
    /// Component type
    pub component: String,
    /// Severity mapping
    pub severity: String,
    /// Recommended action
    pub action: String,
}
```

#### Inbound Sequence Diagram

```
                    INBOUND FLOW

    L1-L6           M01 Error          M02 Config
    Modules         Taxonomy           Manager
        |               |                   |
        | log()         | classify()        | ConfigUpdate
        | (any level)   | (on error)        | (hot reload)
        v               v                   v
    +----------------------------------------------+
    |            Log Ingester Queue                 |
    +----------------------------------------------+
                        |
                        v
              +------------------+
              |  Log Processor   |
              |  Pipeline        |
              +------------------+
                        |
         +--------------+--------------+
         |              |              |
         v              v              v
    +----------+  +-----------+  +-----------+
    | Enricher |  | Classifier|  | Filter    |
    | (M01)    |  | (Level)   |  | (Module)  |
    +----------+  +-----------+  +-----------+
         |              |              |
         +--------------+--------------+
                        |
                        v
              +------------------+
              |    Log Router    |
              +------------------+
```

### Outbound Data Flow

```
+------------------------------------------------------------------+
|                     OUTBOUND DATA TARGETS                         |
+------------------------------------------------------------------+
|                                                                    |
|         +------------------+                                       |
|         |  Logging System  |                                       |
|         |  (Source)        |                                       |
|         +--------+---------+                                       |
|                  |                                                 |
|    +-------------+-------------+-------------+-------------+       |
|    |             |             |             |             |       |
|    v             v             v             v             v       |
| +------+   +------+   +------+   +------+   +------+              |
| | M04  |   | M05  |   | File |   |Syslog|   | TCP/ |              |
| |Metric|   |State |   |System|   |      |   | UDP  |              |
| +------+   +------+   +------+   +------+   +------+              |
|    |             |             |             |             |       |
|    v             v             v             v             v       |
| Log Volume   Critical     Log Files    System      Remote          |
| Metrics      Errors       (Rotating)   Logs       Collectors       |
|                                                                    |
+------------------------------------------------------------------+
```

#### Outbound Message Types

```rust
/// Messages sent by Logging System
#[derive(Debug, Clone)]
pub enum LogOutboundMessage {
    /// Log volume metrics to M04
    LogVolumeMetrics {
        total_logs: u64,
        logs_by_level: HashMap<LogLevel, u64>,
        logs_by_module: HashMap<String, u64>,
        queue_depth: usize,
        dropped_logs: u64,
        period_seconds: u64,
    },

    /// Critical error notification to M05 (for persistence)
    CriticalErrorLog {
        entry: LogEntry,
        requires_persistence: bool,
    },

    /// Formatted log output
    FormattedLog {
        output: String,
        format: LogFormat,
        destination: LogOutputType,
    },

    /// Correlation context export (for distributed tracing)
    CorrelationExport {
        correlation: CorrelationContext,
        entries: Vec<LogEntry>,
    },

    /// Statistics update
    StatsUpdate {
        statistics: LogStatistics,
        timestamp: DateTime<Utc>,
    },

    /// Output health status
    OutputHealth {
        output_id: String,
        healthy: bool,
        error: Option<String>,
        last_write: DateTime<Utc>,
    },
}

/// Log statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogStatistics {
    /// Total logs processed
    pub total_processed: u64,
    /// Logs per level
    pub by_level: HashMap<LogLevel, u64>,
    /// Logs per module
    pub by_module: HashMap<String, u64>,
    /// Current queue depth
    pub queue_depth: usize,
    /// Dropped logs (queue full)
    pub dropped: u64,
    /// Output statistics
    pub outputs: Vec<OutputStatistics>,
    /// Average processing time (us)
    pub avg_processing_us: u64,
    /// Uptime seconds
    pub uptime_seconds: u64,
}

/// Per-output statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputStatistics {
    /// Output identifier
    pub output_id: String,
    /// Output type
    pub output_type: String,
    /// Logs written
    pub logs_written: u64,
    /// Bytes written
    pub bytes_written: u64,
    /// Write errors
    pub errors: u64,
    /// Last write timestamp
    pub last_write: DateTime<Utc>,
}
```

#### Outbound Sequence Diagram

```
                    OUTBOUND FLOW

                  Logging System
                        |
                        | LogEntry processed
                        v
              +------------------+
              |    Log Router    |
              +------------------+
                        |
         +--------------+--------------+--------------+
         |              |              |              |
         v              v              v              v
    +----------+  +-----------+  +-----------+  +-----------+
    |  M04     |  |   File    |  |  Syslog   |  | Network   |
    |  Metrics |  |  Writer   |  |  Writer   |  |  Writer   |
    +----------+  +-----------+  +-----------+  +-----------+
         |              |              |              |
         v              v              v              v
    Log Volume     engine.log     /dev/log      Remote
    Counters       error.log                    Collector


    Special Path (Critical Errors):

              +------------------+
              |  Critical Error  |
              |  Detected        |
              +--------+---------+
                       |
                       v
              +------------------+
              |      M05         |
              | State Persistence|
              +------------------+
                       |
                       v
              +------------------+
              |   Error Event    |
              |   Persisted      |
              +------------------+
```

### Cross-Module Dependencies

```
+------------------------------------------------------------------+
|            BI-DIRECTIONAL DEPENDENCY MATRIX (M03)                 |
+------------------------------------------------------------------+
|                                                                    |
|  Module    | M03 Reads From        | M03 Writes To                |
|  ----------|------------------------|-----------------------------  |
|  M01       | Error vectors for     | Classified errors for        |
|            | log enrichment        | error logging                |
|  ----------|------------------------|-----------------------------  |
|  M02       | Log configuration     | -                            |
|            | (levels, outputs)     |                              |
|  ----------|------------------------|-----------------------------  |
|  M04       | -                      | Log volume metrics,          |
|            |                        | throughput stats             |
|  ----------|------------------------|-----------------------------  |
|  M05       | -                      | Critical error logs          |
|            |                        | for persistence              |
|  ----------|------------------------|-----------------------------  |
|  M06       | -                      | Resource alerts via          |
|            |                        | error logging                |
|  ----------|------------------------|-----------------------------  |
|  L2-L6     | -                      | All layers emit logs         |
|            |                        | (as consumers)               |
|                                                                    |
+------------------------------------------------------------------+

Communication Patterns:
+------------------------------------------------------------------+
|  Pattern          | Source  | Target  | Type        | Frequency  |
|  -----------------|---------|---------|-------------|----------- |
|  Log Emit         | L1-L6   | M03     | Async       | Continuous |
|  Config Update    | M02     | M03     | Event       | On change  |
|  Error Classify   | M01     | M03     | Sync        | On error   |
|  Metrics Export   | M03     | M04     | Async       | Periodic   |
|  Critical Persist | M03     | M05     | Async       | On critical|
|  Flush            | Any     | M03     | Sync        | On demand  |
+------------------------------------------------------------------+

Error Propagation:
+------------------------------------------------------------------+
|  Error Source     | Propagates To       | Action                  |
|  -----------------|---------------------|------------------------ |
|  Output Failure   | M01 (classify)      | Retry + Fallback        |
|  Queue Full       | M04 (metrics)       | Drop + Alert            |
|  Parse Error      | M01 (classify)      | Log malformed entry     |
|  Config Invalid   | M02 (feedback)      | Keep previous config    |
+------------------------------------------------------------------+
```

### Contextual Flow

```
+------------------------------------------------------------------+
|                   LOG ENTRY LIFECYCLE                              |
+------------------------------------------------------------------+
|                                                                    |
|  1. EMIT PHASE                                                     |
|     +------------------+                                           |
|     |  logger.info()   |  (Any module/layer)                      |
|     +--------+---------+                                           |
|              |                                                     |
|              v                                                     |
|     +------------------+                                           |
|     |   LogEntry       |  (Message + Fields + Correlation)        |
|     |   Created        |                                           |
|     +--------+---------+                                           |
|              |                                                     |
|  2. QUEUE PHASE                                                    |
|              v                                                     |
|     +------------------+                                           |
|     |  Async Queue     |  (Non-blocking enqueue)                  |
|     +--------+---------+                                           |
|              |                                                     |
|  3. PROCESS PHASE                                                  |
|              v                                                     |
|     +------------------+                                           |
|     |   Enrichment     |  (Add error vector if error)             |
|     +--------+---------+                                           |
|              |                                                     |
|              v                                                     |
|     +------------------+                                           |
|     |   Level Filter   |  (Check module/global level)             |
|     +--------+---------+                                           |
|              |                                                     |
|              v                                                     |
|     +------------------+                                           |
|     |   Format         |  (JSON/Text/Logfmt)                      |
|     +--------+---------+                                           |
|              |                                                     |
|  4. ROUTE PHASE                                                    |
|              v                                                     |
|     +------------------+                                           |
|     |   Route to       |  (Match output criteria)                 |
|     |   Outputs        |                                           |
|     +--------+---------+                                           |
|              |                                                     |
|  5. WRITE PHASE                                                    |
|              +-------+-------+-------+                             |
|              |       |       |       |                             |
|              v       v       v       v                             |
|           stdout   file   syslog  network                         |
|                                                                    |
|  6. METRICS PHASE (Async)                                          |
|     +------------------+                                           |
|     |   Update M04     |  (Log volume, throughput)                |
|     +------------------+                                           |
|                                                                    |
+------------------------------------------------------------------+
```

#### Log Format Examples

```json
// JSON format
{
  "timestamp": "2026-01-28T12:00:00.123456Z",
  "level": "ERROR",
  "layer": "L3",
  "module": "M12",
  "component": "hebbian_pathway",
  "correlation_id": "corr-abc123-def456",
  "span_id": "span-789xyz",
  "message": "Pathway activation failed",
  "fields": {
    "pathway_id": "pw-001",
    "activation_weight": 0.75,
    "failure_reason": "insufficient_confidence"
  },
  "error_vector": [0.3, 0.6, 0.6, 0.25, 0.2, 0.4, 0.1, 0.2, 0.0, 0.0, 0.4],
  "error_code": "E3201",
  "source": {
    "file": "src/learning/hebbian.rs",
    "line": 342,
    "function": "activate_pathway"
  }
}
```

```
// Text format
2026-01-28T12:00:00.123Z ERROR [L3.M12.hebbian_pathway] corr=abc123 Pathway activation failed pathway_id=pw-001 weight=0.75 reason=insufficient_confidence
```

```
// Logfmt format
ts=2026-01-28T12:00:00.123Z level=error layer=L3 module=M12 component=hebbian_pathway correlation_id=abc123 msg="Pathway activation failed" pathway_id=pw-001 activation_weight=0.75 failure_reason=insufficient_confidence
```

---

## Metrics

| Metric | Type | Description |
|--------|------|-------------|
| `me_m03_logs_total` | Counter | Total logs processed by level |
| `me_m03_logs_dropped_total` | Counter | Logs dropped (queue full) |
| `me_m03_queue_depth` | Gauge | Current queue depth |
| `me_m03_processing_duration_us` | Histogram | Log processing latency |
| `me_m03_output_writes_total` | Counter | Writes per output |
| `me_m03_output_bytes_total` | Counter | Bytes written per output |
| `me_m03_output_errors_total` | Counter | Write errors per output |
| `me_m03_output_flush_duration_ms` | Histogram | Flush latency per output |
| `me_m03_correlation_spans_active` | Gauge | Active correlation spans |
| `me_m03_errors_classified` | Counter | Errors classified by M01 |

---

## Error Codes

| Code | Name | Description | Severity | Recovery |
|------|------|-------------|----------|----------|
| E3001 | LOG_QUEUE_FULL | Async queue at capacity | Warning | Drop + Alert |
| E3002 | LOG_OUTPUT_FAILED | Output write failed | Error | Retry + Fallback |
| E3003 | LOG_FORMAT_ERROR | Failed to format log entry | Warning | Use fallback format |
| E3004 | LOG_FILE_ROTATION_FAILED | Log file rotation failed | Error | Alert + Continue |
| E3005 | LOG_SYSLOG_UNAVAILABLE | Syslog not available | Warning | Skip syslog output |
| E3006 | LOG_NETWORK_TIMEOUT | Network output timeout | Warning | Buffer + Retry |
| E3007 | LOG_CONFIG_INVALID | Invalid logging configuration | Warning | Keep current |
| E3008 | LOG_CORRELATION_INVALID | Invalid correlation context | Warning | Generate new |
| E3009 | LOG_LEVEL_UNKNOWN | Unknown log level | Warning | Default to INFO |
| E3010 | LOG_SHUTDOWN_TIMEOUT | Shutdown timeout (unflushed logs) | Warning | Force shutdown |

---

## Navigation

| Direction | Link |
|-----------|------|
| Up | [INDEX.md](../INDEX.md) |
| Layer | [L01_FOUNDATION.md](../layers/L01_FOUNDATION.md) |
| Previous | [M02_CONFIGURATION_MANAGER.md](M02_CONFIGURATION_MANAGER.md) |
| Next | [M04_METRICS_COLLECTOR.md](M04_METRICS_COLLECTOR.md) |
| Related | [M01_ERROR_TAXONOMY.md](M01_ERROR_TAXONOMY.md) |

---

*[Back to Index](../INDEX.md) | [Layer: L01 Foundation](../layers/L01_FOUNDATION.md)*
