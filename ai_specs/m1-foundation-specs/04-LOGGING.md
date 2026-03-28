# M03 Logging — logging.rs

> **File:** `src/m1_foundation/logging.rs` | **LOC:** ~854 | **Tests:** 15
> **Role:** Structured logging with correlation IDs, context propagation, and tracing integration

---

## CorrelationProvider Trait

```rust
pub trait CorrelationProvider: Send + Sync {
    fn correlation_id(&self) -> &str;
    fn child(&self, operation: &str) -> Box<dyn CorrelationProvider>;
    fn agent_id(&self) -> Option<&str>;  // default: None (NAM R5)
}
```

Concrete implementor: `LogContext`

---

## LogContext

```rust
#[derive(Debug, Clone, Default)]
pub struct LogContext {
    pub correlation_id: String,
    pub service_id: Option<String>,
    pub layer: Option<String>,
    pub module: Option<String>,
    pub agent_id: Option<String>,   // NAM R5
}
```

| Method | Returns | Notes |
|--------|---------|-------|
| `new()` | `Self` | Empty context, generates correlation_id |
| `with_context(service, layer, module)` | `Self` | Full context with generated correlation_id |
| `child_context()` | `Self` | New correlation_id, inherits service/layer |
| `with_module(module)` | `Self` | Overrides module field |
| `with_layer(layer)` | `Self` | Overrides layer field |
| `with_agent(agent_id)` | `Self` | Sets agent_id (NAM R5) |
| `to_tensor_position()` | `Tensor12D` | D0=hash(svc_id), D2=layer/6, D5=0.5, D6=1.0 |

All methods are `#[must_use]`.

Implements `CorrelationProvider` — `child()` boxes `with_module(operation)`.

---

## LogFormat / LogLevel

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LogFormat { Json, #[default] Pretty, Compact }

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum LogLevel { Trace, Debug, #[default] Info, Warn, Error }
```

Both implement `Display`, `FromStr`. `LogLevel` has `const fn to_tracing_level(self) -> Level`.

---

## LogConfig

```rust
pub struct LogConfig {
    pub level: String,
    pub format: LogFormat,
    pub include_timestamps: bool,
    pub include_targets: bool,
    pub include_file_line: bool,
    pub include_thread_ids: bool,
    pub include_span_events: bool,
}
```

| Factory | Profile |
|---------|---------|
| `default()` | Pretty, info, timestamps+targets |
| `development()` | Pretty, debug, file/line enabled |
| `production()` | JSON, info, thread_ids enabled |
| `from_env()` | Reads RUST_LOG / MAINTENANCE_ENGINE_LOG |

---

## Initialization

```rust
pub fn init_logging(config: &LogConfig) -> Result<()>     // errors if already initialized
pub fn try_init_logging(config: &LogConfig)               // infallible (ignores errors)
pub fn is_logging_initialized() -> bool
```

**Concurrency:** `static LOGGING_INITIALIZED: OnceLock<bool>` — set-once, thread-safe.
`init_logging` errors on second call. `try_init_logging` silently absorbs — safe for test contexts.

---

## Context Propagation

```rust
pub fn with_context<F, R>(ctx: &LogContext, f: F) -> R
    where F: FnOnce() -> R
pub async fn with_context_async<F, R>(ctx: &LogContext, f: F) -> R
    where F: std::future::Future<Output = R>
```

Creates a tracing span with correlation_id, service, layer, module fields.

---

## Correlation ID Generation

```rust
pub fn generate_correlation_id() -> String       // UUID v4 (36 chars)
pub fn generate_short_correlation_id() -> String  // first 8 chars of UUID v4
```

---

## Re-exports

```rust
pub use tracing::{debug, error, info, trace, warn};
pub use tracing::{debug_span, error_span, info_span, trace_span, warn_span};
```

---

*M03 Logging Spec v1.0 | 2026-03-01*
