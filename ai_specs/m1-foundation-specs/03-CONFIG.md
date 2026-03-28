# M02 Configuration — config.rs

> **File:** `src/m1_foundation/config.rs` | **LOC:** ~1,114 | **Tests:** 14
> **Role:** Configuration loading, validation, hot-reload with SIGHUP

---

## ConfigProvider Trait

```rust
pub trait ConfigProvider: Send + Sync {
    fn get(&self) -> Result<Config>;
    fn validate(&self) -> Result<()>;
    fn reload(&self) -> Result<Config>;
    fn change_history(&self) -> Vec<ConfigChangeEvent>;  // default: Vec::new()
    fn agent_id(&self) -> Option<&str>;                   // default: None (NAM R5)
}
```

Concrete implementor: `ConfigManager`

---

## Config Struct

```rust
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Config {
    pub host: String,           // default "0.0.0.0",  env ME_HOST
    pub port: u16,              // default 8080,        env ME_PORT
    pub grpc_port: u16,         // default 8081,        env ME_GRPC_PORT
    pub ws_port: u16,           // default 8082,        env ME_WS_PORT
    pub database_path: String,  // default "data/maintenance.db"
    pub log_level: String,      // default "info"
}
```

| Method | Returns | Notes |
|--------|---------|-------|
| `load()` | `Result<Self>` | TOML file + env overlay + validation |
| `load_from_path(&Path)` | `Result<Self>` | Custom base path |
| `validate(&self)` | `Result<()>` | Port conflicts, valid log level, non-empty fields |
| `defaults()` | `Self` | #[must_use] |
| `to_tensor()` | `Tensor12D` | D1=port/65535, D2=1/6, D6=1.0 |

---

## ConfigBuilder

```rust
ConfigBuilder::new()
    .with_base_path(&path)   // optional
    .skip_files()            // const fn — skip TOML loading
    .skip_env()              // const fn — skip env overlay
    .host("127.0.0.1")
    .port(9000)              // const fn
    .grpc_port(9001)         // const fn
    .ws_port(9002)           // const fn
    .database_path("/data/me.db")
    .log_level("debug")
    .build()?                // Result<Config> — validates
```

All setters are `#[must_use]`. `build()` is the terminal consuming method.

---

## ConfigManager

```rust
// Internal state:
config: Arc<parking_lot::RwLock<Config>>
reload_flag: Arc<AtomicBool>

// Construction:
ConfigManager::new() -> Result<Self>              // loads from default path
ConfigManager::with_base_path(path) -> Result<Self>
ConfigManager::from_config(Config) -> Self        // direct, no file loading

// Operations:
manager.get() -> Config                           // read lock, clone
manager.read() -> RwLockReadGuard<'_, Config>     // borrowed read
manager.reload() -> Result<ConfigChangeEvent>     // preserves previous on error
manager.validate() -> ValidationResult
manager.request_reload()                          // sets AtomicBool flag
manager.reload_requested() -> bool                // reads AtomicBool
manager.start_hot_reload() -> Result<()>          // async, SIGHUP handler (Unix only)
```

**Concurrency:** `Arc<RwLock<Config>>` for read-heavy access. `AtomicBool` (SeqCst) for reload signaling.

**Hot-reload:** On SIGHUP, sets `reload_flag` → next `reload()` call re-reads TOML + env + validates. Previous config preserved on failure.

---

## Validation

```rust
pub struct ValidationResult { pub errors: Vec<ValidationError>, pub warnings: Vec<ValidationWarning> }
pub struct ValidationError { pub field: String, pub message: String }
pub struct ValidationWarning { pub field: String, pub message: String }
```

Validation rules:
- Host must not be empty
- All ports must be > 0
- Ports must not conflict (port != grpc_port != ws_port)
- Log level must be valid (trace/debug/info/warn/error)
- Database path must not be empty

---

## Errors Produced

- `Error::Config(String)` — file read, TOML parse, SIGHUP registration
- `Error::Validation(String)` — port conflict, invalid log level, empty fields

---

## Constants

```rust
pub(crate) const ENV_PREFIX: &str = "ME_";
pub(crate) const DEFAULT_CONFIG_PATH: &str = "config/default.toml";
pub(crate) const LOCAL_CONFIG_PATH: &str = "config/local.toml";
```

---

*M02 Configuration Spec v1.0 | 2026-03-01*
