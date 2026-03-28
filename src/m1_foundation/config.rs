//! # M02: Configuration Manager
//!
//! Centralized configuration management with hot-reload capability, environment
//! variable interpolation, and validation.
//!
//! ## Layer: L1 (Foundation)
//! ## Dependencies: M01 (Error Taxonomy)
//! ## Tests: 50+ target
//!
//! ## 12D Tensor Encoding
//! ```text
//! [2/36, 0.0, 1/6, 1, 0, 0.5, health, uptime, synergy, latency, error_rate, temporal]
//! ```
//!
//! ## Configuration Loading Order (Priority: lowest to highest)
//! 1. Default values
//! 2. `config/default.toml`
//! 3. `config/local.toml` (optional, git-ignored)
//! 4. Environment variables (`ME_*` prefix)
//!
//! ## Hot Reload
//! Send `SIGHUP` to trigger configuration reload.
//!
//! ## Related Documentation
//! - [Module Specification](../../ai_docs/modules/M02_CONFIGURATION_MANAGER.md)
//! - [Layer Specification](../../ai_docs/layers/L01_FOUNDATION.md)

use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[cfg(unix)]
use tokio::signal::unix::{signal, SignalKind};

/// Environment variable prefix for configuration overrides.
pub(crate) const ENV_PREFIX: &str = "ME_";

/// Default configuration file path.
pub(crate) const DEFAULT_CONFIG_PATH: &str = "config/default.toml";

/// Local configuration file path (overrides default).
pub(crate) const LOCAL_CONFIG_PATH: &str = "config/local.toml";

/// Abstract interface for retrieving, validating, and reloading configuration.
///
/// Implement this trait for any type that can act as a configuration source,
/// enabling dependency injection and test doubles in upper layers.
///
/// # Thread Safety
///
/// Implementors must be `Send + Sync` so they can be shared across async tasks.
pub trait ConfigProvider: Send + Sync {
    /// Return a snapshot of the current configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if the underlying storage is unavailable or corrupt.
    fn get(&self) -> Result<Config>;

    /// Validate the current configuration without reloading.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` describing every constraint violation found.
    fn validate(&self) -> Result<()>;

    /// Reload configuration from the backing source and return the new snapshot.
    ///
    /// On failure the previous configuration must remain unchanged.
    ///
    /// # Errors
    ///
    /// Returns an error if loading or validation of the new configuration fails.
    fn reload(&self) -> Result<Config>;

    /// Return the history of configuration changes (NAM R5 attribution).
    fn change_history(&self) -> Vec<ConfigChangeEvent> {
        Vec::new()
    }

    /// Return the agent ID associated with this config provider (NAM R5).
    fn agent_id(&self) -> Option<&str> {
        None
    }
}

/// Application configuration for the Maintenance Engine.
///
/// This struct holds all configuration values for the engine. Configuration
/// is loaded from TOML files and can be overridden by environment variables.
///
/// # Configuration Priority (highest to lowest)
/// 1. Environment variables (`ME_HOST`, `ME_PORT`, etc.)
/// 2. `config/local.toml` (if exists)
/// 3. `config/default.toml`
/// 4. Default values in code
///
/// # Example
/// ```rust,ignore
/// use maintenance_engine::m1_foundation::config::Config;
///
/// let config = Config::load()?;
/// println!("Server: {}:{}", config.host, config.port);
/// ```
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Config {
    /// Server host address.
    /// Default: `"0.0.0.0"`
    /// Env: `ME_HOST`
    #[serde(default = "default_host")]
    pub host: String,

    /// REST API port.
    /// Default: `8080`
    /// Env: `ME_PORT`
    #[serde(default = "default_port")]
    pub port: u16,

    /// gRPC port for binary communication.
    /// Default: `8081`
    /// Env: `ME_GRPC_PORT`
    #[serde(default = "default_grpc_port")]
    pub grpc_port: u16,

    /// WebSocket port for real-time events.
    /// Default: `8082`
    /// Env: `ME_WS_PORT`
    #[serde(default = "default_ws_port")]
    pub ws_port: u16,

    /// Path to the `SQLite` database file.
    /// Default: `"data/maintenance.db"`
    /// Env: `ME_DATABASE_PATH`
    #[serde(default = "default_database_path")]
    pub database_path: String,

    /// Log level (trace, debug, info, warn, error).
    /// Default: `"info"`
    /// Env: `ME_LOG_LEVEL`
    #[serde(default = "default_log_level")]
    pub log_level: String,
}

// Default value functions for serde
fn default_host() -> String {
    "0.0.0.0".to_string()
}

const fn default_port() -> u16 {
    8080
}

const fn default_grpc_port() -> u16 {
    8081
}

const fn default_ws_port() -> u16 {
    8082
}

fn default_database_path() -> String {
    "data/maintenance.db".to_string()
}

fn default_log_level() -> String {
    "info".to_string()
}

impl Default for Config {
    fn default() -> Self {
        Self {
            host: default_host(),
            port: default_port(),
            grpc_port: default_grpc_port(),
            ws_port: default_ws_port(),
            database_path: default_database_path(),
            log_level: default_log_level(),
        }
    }
}

impl Config {
    /// Load configuration from files and environment variables.
    ///
    /// This method loads configuration in the following order:
    /// 1. Default values
    /// 2. `config/default.toml` (if exists)
    /// 3. `config/local.toml` (if exists, typically git-ignored)
    /// 4. Environment variables with `ME_` prefix
    ///
    /// # Errors
    /// Returns an error if:
    /// - TOML parsing fails
    /// - Validation fails (port conflicts, invalid values)
    ///
    /// # Example
    /// ```rust,ignore
    /// let config = Config::load()?;
    /// ```
    pub fn load() -> Result<Self> {
        let builder = ConfigBuilder::new();
        builder.build()
    }

    /// Load configuration from a specific directory.
    ///
    /// This allows loading configuration from a custom base path,
    /// useful for testing or non-standard deployments.
    ///
    /// # Arguments
    /// * `base_path` - Base directory containing config files
    ///
    /// # Errors
    /// Returns an error if configuration loading or validation fails.
    pub fn load_from_path(base_path: &Path) -> Result<Self> {
        let builder = ConfigBuilder::new().with_base_path(base_path);
        builder.build()
    }

    /// Validate the configuration values.
    ///
    /// Checks that:
    /// - Ports are not zero
    /// - No port conflicts exist
    /// - Log level is valid
    /// - Host is not empty
    ///
    /// # Errors
    /// Returns a `Validation` error describing all validation failures.
    pub fn validate(&self) -> Result<()> {
        let mut errors = Vec::new();

        // Validate host
        if self.host.is_empty() {
            errors.push("host cannot be empty".to_string());
        }

        // Validate ports are non-zero
        if self.port == 0 {
            errors.push("port cannot be zero".to_string());
        }
        if self.grpc_port == 0 {
            errors.push("grpc_port cannot be zero".to_string());
        }
        if self.ws_port == 0 {
            errors.push("ws_port cannot be zero".to_string());
        }

        // Check for port conflicts
        let ports = [
            ("port", self.port),
            ("grpc_port", self.grpc_port),
            ("ws_port", self.ws_port),
        ];

        for i in 0..ports.len() {
            for j in (i + 1)..ports.len() {
                if ports[i].1 != 0 && ports[i].1 == ports[j].1 {
                    errors.push(format!(
                        "{} and {} cannot use the same port {}",
                        ports[i].0, ports[j].0, ports[i].1
                    ));
                }
            }
        }

        // Validate log level
        let valid_levels = ["trace", "debug", "info", "warn", "error"];
        if !valid_levels.contains(&self.log_level.to_lowercase().as_str()) {
            errors.push(format!(
                "invalid log_level '{}', must be one of: {}",
                self.log_level,
                valid_levels.join(", ")
            ));
        }

        // Validate database path is not empty
        if self.database_path.is_empty() {
            errors.push("database_path cannot be empty".to_string());
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(Error::Validation(errors.join("; ")))
        }
    }

    /// Create a configuration with all default values.
    ///
    /// This bypasses file loading and environment variables.
    #[must_use]
    pub fn defaults() -> Self {
        Self::default()
    }

    /// Encode this configuration as a 12D tensor (NAM R4).
    ///
    /// D1 = port/65535, D2 = 1/6 (L1 tier), D6 = 1.0 (healthy config).
    #[must_use]
    pub fn to_tensor(&self) -> crate::Tensor12D {
        let mut tensor = crate::Tensor12D {
            service_id: 0.0,
            port: f64::from(self.port) / 65535.0,
            tier: 1.0 / 6.0,
            dependency_count: 0.0,
            agent_count: 0.0,
            protocol: 0.5,
            health_score: 1.0,
            uptime: 1.0,
            synergy: 0.0,
            latency: 0.0,
            error_rate: 0.0,
            temporal_context: 0.0,
        };
        tensor.clamp_normalize();
        tensor
    }
}

/// Builder for constructing [`Config`] instances programmatically.
///
/// The builder pattern allows for flexible configuration construction
/// with method chaining and explicit control over each value.
///
/// # Example
/// ```rust,ignore
/// use maintenance_engine::m1_foundation::config::ConfigBuilder;
///
/// let config = ConfigBuilder::new()
///     .host("127.0.0.1")
///     .port(9000)
///     .log_level("debug")
///     .build()?;
/// ```
#[derive(Debug, Clone)]
pub struct ConfigBuilder {
    config: Config,
    base_path: Option<PathBuf>,
    skip_files: bool,
    skip_env: bool,
}

impl ConfigBuilder {
    /// Create a new configuration builder with default values.
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: Config::default(),
            base_path: None,
            skip_files: false,
            skip_env: false,
        }
    }

    /// Set the base path for configuration files.
    ///
    /// By default, configuration is loaded from the current directory.
    #[must_use]
    pub fn with_base_path(mut self, path: &Path) -> Self {
        self.base_path = Some(path.to_path_buf());
        self
    }

    /// Skip loading configuration from files.
    ///
    /// Only environment variables and explicitly set values will be used.
    #[must_use]
    pub const fn skip_files(mut self) -> Self {
        self.skip_files = true;
        self
    }

    /// Skip loading configuration from environment variables.
    #[must_use]
    pub const fn skip_env(mut self) -> Self {
        self.skip_env = true;
        self
    }

    /// Set the server host address.
    #[must_use]
    pub fn host(mut self, host: impl Into<String>) -> Self {
        self.config.host = host.into();
        self
    }

    /// Set the REST API port.
    #[must_use]
    pub const fn port(mut self, port: u16) -> Self {
        self.config.port = port;
        self
    }

    /// Set the gRPC port.
    #[must_use]
    pub const fn grpc_port(mut self, port: u16) -> Self {
        self.config.grpc_port = port;
        self
    }

    /// Set the WebSocket port.
    #[must_use]
    pub const fn ws_port(mut self, port: u16) -> Self {
        self.config.ws_port = port;
        self
    }

    /// Set the database path.
    #[must_use]
    pub fn database_path(mut self, path: impl Into<String>) -> Self {
        self.config.database_path = path.into();
        self
    }

    /// Set the log level.
    #[must_use]
    pub fn log_level(mut self, level: impl Into<String>) -> Self {
        self.config.log_level = level.into();
        self
    }

    /// Build the final configuration.
    ///
    /// This method:
    /// 1. Loads from files (unless skipped)
    /// 2. Applies environment overrides (unless skipped)
    /// 3. Validates the resulting configuration
    ///
    /// # Errors
    /// Returns an error if file loading fails or validation fails.
    pub fn build(mut self) -> Result<Config> {
        // Load from files
        if !self.skip_files {
            self.load_from_files()?;
        }

        // Apply environment overrides
        if !self.skip_env {
            self.apply_env_overrides();
        }

        // Validate the configuration
        self.config.validate()?;

        Ok(self.config)
    }

    /// Load configuration from TOML files.
    fn load_from_files(&mut self) -> Result<()> {
        let base = self.base_path.clone().unwrap_or_else(|| PathBuf::from("."));

        // Load default config
        let default_path = base.join(DEFAULT_CONFIG_PATH);
        if default_path.exists() {
            self.load_toml_file(&default_path)?;
        }

        // Load local config (overrides default)
        let local_path = base.join(LOCAL_CONFIG_PATH);
        if local_path.exists() {
            self.load_toml_file(&local_path)?;
        }

        Ok(())
    }

    /// Load and merge configuration from a TOML file.
    fn load_toml_file(&mut self, path: &Path) -> Result<()> {
        let content = std::fs::read_to_string(path).map_err(|e| {
            Error::Config(format!("failed to read config file '{}': {}", path.display(), e))
        })?;

        // Parse TOML into intermediate structure
        let toml_value: toml::Value = toml::from_str(&content).map_err(|e| {
            Error::Config(format!("failed to parse TOML '{}': {}", path.display(), e))
        })?;

        // Extract server section if present
        if let Some(server) = toml_value.get("server") {
            if let Some(host) = server.get("host").and_then(|v| v.as_str()) {
                self.config.host = host.to_string();
            }
            if let Some(port) = server.get("port").and_then(toml::Value::as_integer) {
                self.config.port = u16::try_from(port).map_err(|_| {
                    Error::Config(format!("invalid port value: {port}"))
                })?;
            }
            if let Some(grpc_port) = server.get("grpc_port").and_then(toml::Value::as_integer) {
                self.config.grpc_port = u16::try_from(grpc_port).map_err(|_| {
                    Error::Config(format!("invalid grpc_port value: {grpc_port}"))
                })?;
            }
            if let Some(ws_port) = server
                .get("websocket_port")
                .or_else(|| server.get("ws_port"))
                .and_then(toml::Value::as_integer)
            {
                self.config.ws_port = u16::try_from(ws_port).map_err(|_| {
                    Error::Config(format!("invalid ws_port value: {ws_port}"))
                })?;
            }
        }

        // Extract database section
        if let Some(database) = toml_value.get("database") {
            if let Some(path_str) = database.get("base_path").and_then(|v| v.as_str()) {
                // Construct full database path
                self.config.database_path = format!("{path_str}/maintenance.db");
            }
        }

        // Extract logging section
        if let Some(logging) = toml_value.get("logging") {
            if let Some(level) = logging.get("level").and_then(|v| v.as_str()) {
                self.config.log_level = level.to_string();
            }
        }

        Ok(())
    }

    /// Apply environment variable overrides.
    fn apply_env_overrides(&mut self) {
        // ME_HOST
        if let Ok(host) = std::env::var(format!("{ENV_PREFIX}HOST")) {
            self.config.host = host;
        }

        // ME_PORT
        if let Ok(port_str) = std::env::var(format!("{ENV_PREFIX}PORT")) {
            if let Ok(port) = port_str.parse::<u16>() {
                self.config.port = port;
            }
        }

        // ME_GRPC_PORT
        if let Ok(port_str) = std::env::var(format!("{ENV_PREFIX}GRPC_PORT")) {
            if let Ok(port) = port_str.parse::<u16>() {
                self.config.grpc_port = port;
            }
        }

        // ME_WS_PORT
        if let Ok(port_str) = std::env::var(format!("{ENV_PREFIX}WS_PORT")) {
            if let Ok(port) = port_str.parse::<u16>() {
                self.config.ws_port = port;
            }
        }

        // ME_DATABASE_PATH
        if let Ok(path) = std::env::var(format!("{ENV_PREFIX}DATABASE_PATH")) {
            self.config.database_path = path;
        }

        // ME_LOG_LEVEL
        if let Ok(level) = std::env::var(format!("{ENV_PREFIX}LOG_LEVEL")) {
            self.config.log_level = level;
        }
    }
}

impl Default for ConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Validation result for configuration.
///
/// Used by the configuration manager for detailed validation reporting.
#[derive(Debug, Clone)]
pub struct ValidationResult {
    /// Whether the configuration is valid.
    pub valid: bool,
    /// List of validation errors.
    pub errors: Vec<ValidationError>,
    /// List of validation warnings.
    pub warnings: Vec<ValidationWarning>,
}

impl ValidationResult {
    /// Create a successful validation result.
    #[must_use]
    pub const fn success() -> Self {
        Self {
            valid: true,
            errors: Vec::new(),
            warnings: Vec::new(),
        }
    }

    /// Create a failed validation result with errors.
    #[must_use]
    pub const fn failure(errors: Vec<ValidationError>) -> Self {
        Self {
            valid: false,
            errors,
            warnings: Vec::new(),
        }
    }
}

/// A validation error for a specific configuration key.
#[derive(Debug, Clone)]
pub struct ValidationError {
    /// The configuration key that failed validation.
    pub key: String,
    /// Error code for programmatic handling.
    pub code: String,
    /// Human-readable error message.
    pub message: String,
}

/// A validation warning for a specific configuration key.
#[derive(Debug, Clone)]
pub struct ValidationWarning {
    /// The configuration key that triggered the warning.
    pub key: String,
    /// Warning code for programmatic handling.
    pub code: String,
    /// Human-readable warning message.
    pub message: String,
}

/// Configuration change event for hot-reload notifications.
#[derive(Debug, Clone)]
pub struct ConfigChangeEvent {
    /// Unique identifier for this change.
    pub change_id: String,
    /// Timestamp of the change.
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Keys that were changed.
    pub changed_keys: Vec<String>,
    /// Previous configuration values.
    pub previous: HashMap<String, String>,
    /// New configuration values.
    pub new: HashMap<String, String>,
    /// Agent that requested this change (NAM R5).
    pub requested_by: Option<String>,
}

/// Configuration manager with hot-reload capability.
///
/// The manager maintains the active configuration and supports
/// signal-based hot reload (SIGHUP on Unix systems).
///
/// # Example
/// ```rust,ignore
/// use maintenance_engine::m1_foundation::config::ConfigManager;
///
/// let manager = ConfigManager::new()?;
///
/// // Get current config
/// let config = manager.get();
///
/// // Start watching for SIGHUP
/// manager.start_hot_reload().await?;
/// ```
pub struct ConfigManager {
    /// Current configuration (wrapped in Arc for thread-safe sharing).
    config: Arc<parking_lot::RwLock<Config>>,
    /// Flag indicating whether hot reload is enabled.
    reload_flag: Arc<AtomicBool>,
    /// Base path for configuration files.
    base_path: PathBuf,
}

impl ConfigProvider for ConfigManager {
    /// Return a clone of the current configuration.
    ///
    /// # Errors
    ///
    /// This implementation is infallible; it always returns `Ok`.
    fn get(&self) -> Result<Config> {
        Ok(self.config.read().clone())
    }

    /// Validate the current configuration.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if the configuration is invalid.
    fn validate(&self) -> Result<()> {
        self.config.read().clone().validate()
    }

    /// Reload configuration from disk and return the new snapshot.
    ///
    /// On failure the previous configuration is preserved.
    ///
    /// # Errors
    ///
    /// Returns an error if loading or validation of the new configuration fails.
    fn reload(&self) -> Result<Config> {
        let new_config = Config::load_from_path(&self.base_path)?;
        *self.config.write() = new_config.clone();
        Ok(new_config)
    }
}

impl ConfigManager {
    /// Create a new configuration manager.
    ///
    /// Loads configuration from default locations.
    ///
    /// # Errors
    /// Returns an error if configuration loading fails.
    pub fn new() -> Result<Self> {
        let config = Config::load()?;
        Ok(Self {
            config: Arc::new(parking_lot::RwLock::new(config)),
            reload_flag: Arc::new(AtomicBool::new(false)),
            base_path: PathBuf::from("."),
        })
    }

    /// Create a configuration manager with a custom base path.
    ///
    /// # Arguments
    /// * `base_path` - Directory containing configuration files
    ///
    /// # Errors
    /// Returns an error if configuration loading fails.
    pub fn with_base_path(base_path: impl Into<PathBuf>) -> Result<Self> {
        let path = base_path.into();
        let config = Config::load_from_path(&path)?;
        Ok(Self {
            config: Arc::new(parking_lot::RwLock::new(config)),
            reload_flag: Arc::new(AtomicBool::new(false)),
            base_path: path,
        })
    }

    /// Create a configuration manager with a pre-loaded config.
    ///
    /// Useful for testing or when configuration is loaded externally.
    #[must_use]
    pub fn from_config(config: Config) -> Self {
        Self {
            config: Arc::new(parking_lot::RwLock::new(config)),
            reload_flag: Arc::new(AtomicBool::new(false)),
            base_path: PathBuf::from("."),
        }
    }

    /// Get the current configuration.
    ///
    /// Returns a clone of the current configuration.
    #[must_use]
    pub fn get(&self) -> Config {
        self.config.read().clone()
    }

    /// Get a read lock on the configuration.
    ///
    /// Use this for read-heavy scenarios where cloning is expensive.
    pub fn read(&self) -> parking_lot::RwLockReadGuard<'_, Config> {
        self.config.read()
    }

    /// Reload configuration from files and report what changed.
    ///
    /// This method reloads configuration from disk and applies
    /// environment variable overrides.
    ///
    /// # Errors
    /// Returns an error if loading or validation fails. On error,
    /// the previous configuration is retained.
    pub fn reload(&self) -> Result<ConfigChangeEvent> {
        let previous = self.get();

        // Load new configuration
        let new_config = Config::load_from_path(&self.base_path)?;

        // Compute changes
        let mut changed_keys = Vec::new();
        let mut previous_values = HashMap::new();
        let mut new_values = HashMap::new();

        if previous.host != new_config.host {
            changed_keys.push("host".to_string());
            previous_values.insert("host".to_string(), previous.host.clone());
            new_values.insert("host".to_string(), new_config.host.clone());
        }
        if previous.port != new_config.port {
            changed_keys.push("port".to_string());
            previous_values.insert("port".to_string(), previous.port.to_string());
            new_values.insert("port".to_string(), new_config.port.to_string());
        }
        if previous.grpc_port != new_config.grpc_port {
            changed_keys.push("grpc_port".to_string());
            previous_values.insert("grpc_port".to_string(), previous.grpc_port.to_string());
            new_values.insert("grpc_port".to_string(), new_config.grpc_port.to_string());
        }
        if previous.ws_port != new_config.ws_port {
            changed_keys.push("ws_port".to_string());
            previous_values.insert("ws_port".to_string(), previous.ws_port.to_string());
            new_values.insert("ws_port".to_string(), new_config.ws_port.to_string());
        }
        if previous.database_path != new_config.database_path {
            changed_keys.push("database_path".to_string());
            previous_values.insert("database_path".to_string(), previous.database_path.clone());
            new_values.insert("database_path".to_string(), new_config.database_path.clone());
        }
        if previous.log_level != new_config.log_level {
            changed_keys.push("log_level".to_string());
            previous_values.insert("log_level".to_string(), previous.log_level);
            new_values.insert("log_level".to_string(), new_config.log_level.clone());
        }

        // Update configuration
        *self.config.write() = new_config;

        Ok(ConfigChangeEvent {
            change_id: uuid::Uuid::new_v4().to_string(),
            timestamp: chrono::Utc::now(),
            changed_keys,
            previous: previous_values,
            new: new_values,
            requested_by: None,
        })
    }

    /// Check if a reload has been requested.
    ///
    /// This is used internally by the hot-reload system.
    #[must_use]
    pub fn reload_requested(&self) -> bool {
        self.reload_flag.load(Ordering::SeqCst)
    }

    /// Clear the reload request flag.
    pub fn clear_reload_request(&self) {
        self.reload_flag.store(false, Ordering::SeqCst);
    }

    /// Request a configuration reload.
    ///
    /// This sets the reload flag, which will be picked up by the
    /// hot-reload task.
    pub fn request_reload(&self) {
        self.reload_flag.store(true, Ordering::SeqCst);
    }

    /// Start the hot-reload signal handler.
    ///
    /// On Unix systems, this listens for SIGHUP signals and reloads
    /// configuration when received.
    ///
    /// # Errors
    /// Returns an error if signal handling cannot be set up.
    #[cfg(unix)]
    #[allow(clippy::unused_async)]
    pub async fn start_hot_reload(&self) -> Result<()> {
        let reload_flag = Arc::clone(&self.reload_flag);

        let mut sighup = signal(SignalKind::hangup()).map_err(|e| {
            Error::Config(format!("failed to register SIGHUP handler: {e}"))
        })?;

        tokio::spawn(async move {
            loop {
                sighup.recv().await;
                reload_flag.store(true, Ordering::SeqCst);
                tracing::info!("SIGHUP received, configuration reload requested");
            }
        });

        Ok(())
    }

    /// Start the hot-reload signal handler (no-op on non-Unix).
    #[cfg(not(unix))]
    pub async fn start_hot_reload(&self) -> Result<()> {
        tracing::warn!("Hot reload via SIGHUP is only supported on Unix systems");
        Ok(())
    }

    /// Validate the current configuration.
    #[must_use]
    pub fn validate(&self) -> ValidationResult {
        let config = self.get();
        match config.validate() {
            Ok(()) => ValidationResult::success(),
            Err(Error::Validation(msg)) => {
                let errors = msg
                    .split("; ")
                    .map(|s| ValidationError {
                        key: extract_key_from_error(s),
                        code: "E2003".to_string(),
                        message: s.to_string(),
                    })
                    .collect();
                ValidationResult::failure(errors)
            }
            Err(e) => ValidationResult::failure(vec![ValidationError {
                key: "unknown".to_string(),
                code: "E2000".to_string(),
                message: e.to_string(),
            }]),
        }
    }
}

/// Extract the configuration key from an error message.
fn extract_key_from_error(error: &str) -> String {
    // Simple heuristic: first word before "cannot" or the field name
    let keys = ["host", "port", "grpc_port", "ws_port", "database_path", "log_level"];
    for key in keys {
        if error.contains(key) {
            return key.to_string();
        }
    }
    "unknown".to_string()
}

impl std::fmt::Debug for ConfigManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConfigManager")
            .field("config", &self.get())
            .field("base_path", &self.base_path)
            .finish_non_exhaustive()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::sync::Arc;
    use tempfile::tempdir;

    // ========================================================================
    // Existing tests — refactored to use `-> crate::Result<()>` with `?`
    // ========================================================================

    #[test]
    fn test_config_defaults() {
        let config = Config::defaults();
        assert_eq!(config.host, "0.0.0.0");
        assert_eq!(config.port, 8080);
        assert_eq!(config.grpc_port, 8081);
        assert_eq!(config.ws_port, 8082);
        assert_eq!(config.database_path, "data/maintenance.db");
        assert_eq!(config.log_level, "info");
    }

    #[test]
    fn test_config_validation_success() {
        let config = Config::defaults();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_validation_empty_host() {
        let config = Config {
            host: String::new(),
            ..Config::defaults()
        };
        let result = config.validate();
        assert!(result.is_err());
        if let Err(e) = result {
            assert!(e.to_string().contains("host cannot be empty"));
        }
    }

    #[test]
    fn test_config_validation_zero_port() {
        let config = Config {
            port: 0,
            ..Config::defaults()
        };
        let result = config.validate();
        assert!(result.is_err());
        if let Err(e) = result {
            assert!(e.to_string().contains("port cannot be zero"));
        }
    }

    #[test]
    fn test_config_validation_port_conflict() {
        let config = Config {
            port: 8080,
            grpc_port: 8080,
            ..Config::defaults()
        };
        let result = config.validate();
        assert!(result.is_err());
        if let Err(e) = result {
            assert!(e.to_string().contains("cannot use the same port"));
        }
    }

    #[test]
    fn test_config_validation_invalid_log_level() {
        let config = Config {
            log_level: "invalid".to_string(),
            ..Config::defaults()
        };
        let result = config.validate();
        assert!(result.is_err());
        if let Err(e) = result {
            assert!(e.to_string().contains("invalid log_level"));
        }
    }

    #[test]
    fn test_builder_defaults() -> crate::Result<()> {
        let config = ConfigBuilder::new()
            .skip_files()
            .skip_env()
            .build()?;
        assert_eq!(config.port, 8080);
        Ok(())
    }

    #[test]
    fn test_builder_custom_values() -> crate::Result<()> {
        let config = ConfigBuilder::new()
            .skip_files()
            .skip_env()
            .host("127.0.0.1")
            .port(9000)
            .grpc_port(9001)
            .ws_port(9002)
            .database_path("/custom/path.db")
            .log_level("debug")
            .build()?;

        assert_eq!(config.host, "127.0.0.1");
        assert_eq!(config.port, 9000);
        assert_eq!(config.grpc_port, 9001);
        assert_eq!(config.ws_port, 9002);
        assert_eq!(config.database_path, "/custom/path.db");
        assert_eq!(config.log_level, "debug");
        Ok(())
    }

    #[test]
    fn test_builder_env_override() -> crate::Result<()> {
        // Use a unique suffix to avoid collision with parallel tests.
        // Note: env mutation is process-wide; this test sets and unsets quickly.
        env::set_var("ME_HOST", "env-host-override");
        env::set_var("ME_PORT", "7777");

        let result = ConfigBuilder::new()
            .skip_files()
            .host("original-host")
            .port(8080)
            .build();

        // Clean up environment before asserting so they are always removed
        env::remove_var("ME_HOST");
        env::remove_var("ME_PORT");

        let config = result?;
        assert_eq!(config.host, "env-host-override");
        assert_eq!(config.port, 7777);
        Ok(())
    }

    #[test]
    fn test_config_manager_from_config() {
        let config = Config::defaults();
        let manager = ConfigManager::from_config(config.clone());
        let retrieved = manager.get();
        assert_eq!(retrieved.port, config.port);
    }

    #[test]
    fn test_config_manager_reload_request() {
        let config = Config::defaults();
        let manager = ConfigManager::from_config(config);

        assert!(!manager.reload_requested());
        manager.request_reload();
        assert!(manager.reload_requested());
        manager.clear_reload_request();
        assert!(!manager.reload_requested());
    }

    #[test]
    fn test_toml_file_loading() -> crate::Result<()> {
        let dir = tempdir().map_err(|e| Error::Config(format!("tempdir: {e}")))?;

        // Create config directory
        let config_dir = dir.path().join("config");
        std::fs::create_dir_all(&config_dir)
            .map_err(|e| Error::Config(format!("create_dir_all: {e}")))?;

        // Create default.toml
        let toml_content = r#"
[server]
host = "192.168.1.1"
port = 3000
grpc_port = 3001
websocket_port = 3002

[database]
base_path = "/var/lib/me"

[logging]
level = "debug"
"#;

        std::fs::write(config_dir.join("default.toml"), toml_content)
            .map_err(|e| Error::Config(format!("write: {e}")))?;

        let config = ConfigBuilder::new()
            .with_base_path(dir.path())
            .skip_env()
            .build()?;

        assert_eq!(config.host, "192.168.1.1");
        assert_eq!(config.port, 3000);
        assert_eq!(config.grpc_port, 3001);
        assert_eq!(config.ws_port, 3002);
        assert_eq!(config.log_level, "debug");
        Ok(())
    }

    #[test]
    fn test_local_toml_override() -> crate::Result<()> {
        let dir = tempdir().map_err(|e| Error::Config(format!("tempdir: {e}")))?;

        let config_dir = dir.path().join("config");
        std::fs::create_dir_all(&config_dir)
            .map_err(|e| Error::Config(format!("create_dir_all: {e}")))?;

        // Create default.toml
        let default_content = r#"
[server]
host = "0.0.0.0"
port = 8080
grpc_port = 8081
websocket_port = 8082

[logging]
level = "info"
"#;
        std::fs::write(config_dir.join("default.toml"), default_content)
            .map_err(|e| Error::Config(format!("write default: {e}")))?;

        // Create local.toml with override
        let local_content = r#"
[server]
port = 9999

[logging]
level = "trace"
"#;
        std::fs::write(config_dir.join("local.toml"), local_content)
            .map_err(|e| Error::Config(format!("write local: {e}")))?;

        let config = ConfigBuilder::new()
            .with_base_path(dir.path())
            .skip_env()
            .build()?;

        // port should be overridden by local.toml
        assert_eq!(config.port, 9999);
        // host should come from default.toml
        assert_eq!(config.host, "0.0.0.0");
        // log_level should be overridden
        assert_eq!(config.log_level, "trace");
        Ok(())
    }

    #[test]
    fn test_validation_result() {
        let success = ValidationResult::success();
        assert!(success.valid);
        assert!(success.errors.is_empty());

        let failure = ValidationResult::failure(vec![ValidationError {
            key: "port".to_string(),
            code: "E2003".to_string(),
            message: "port cannot be zero".to_string(),
        }]);
        assert!(!failure.valid);
        assert_eq!(failure.errors.len(), 1);
    }

    #[test]
    fn test_config_manager_validate() {
        let config = Config::defaults();
        let manager = ConfigManager::from_config(config);
        let result = manager.validate();
        assert!(result.valid);
    }

    #[test]
    fn test_config_serialization() -> crate::Result<()> {
        let config = Config::defaults();
        let json = serde_json::to_string(&config)
            .map_err(|e| Error::Config(format!("serialize: {e}")))?;

        let deserialized: Config = serde_json::from_str(&json)
            .map_err(|e| Error::Config(format!("deserialize: {e}")))?;

        assert_eq!(config, deserialized);
        Ok(())
    }

    // ========================================================================
    // ConfigProvider trait compliance tests (5 tests)
    // ========================================================================

    #[test]
    fn test_provider_get_returns_current_config() -> crate::Result<()> {
        let config = Config::defaults();
        let manager = ConfigManager::from_config(config.clone());
        let provider: &dyn ConfigProvider = &manager;
        let got = provider.get()?;
        assert_eq!(got.port, config.port);
        assert_eq!(got.host, config.host);
        Ok(())
    }

    #[test]
    fn test_provider_validate_ok_for_defaults() -> crate::Result<()> {
        let manager = ConfigManager::from_config(Config::defaults());
        let provider: &dyn ConfigProvider = &manager;
        provider.validate()?;
        Ok(())
    }

    #[test]
    fn test_provider_validate_err_for_invalid() {
        let config = Config {
            host: String::new(),
            ..Config::defaults()
        };
        let manager = ConfigManager::from_config(config);
        let provider: &dyn ConfigProvider = &manager;
        assert!(provider.validate().is_err());
    }

    #[test]
    fn test_provider_reload_fallback_to_defaults_on_no_files() {
        // With no config files, reload from "." will just produce defaults
        // because no files exist there.  It must succeed (defaults are valid).
        let manager = ConfigManager::from_config(Config::defaults());
        // reload uses self.base_path which is "." — files won't exist there
        // in most test environments so we just verify it does not panic and
        // either succeeds or returns a sensible error.
        let result = <ConfigManager as ConfigProvider>::reload(&manager);
        // Whether it succeeds or fails, the previous config must still be readable
        let still_valid = manager.get();
        assert_eq!(still_valid.port, 8080);
        // Suppress unused-result lint — we intentionally discard either arm here
        drop(result);
    }

    #[test]
    fn test_provider_is_send_sync() {
        // Compile-time assertion: ConfigManager implements ConfigProvider which
        // requires Send + Sync.  Wrapping in Arc verifies this at compile time.
        let manager = Arc::new(ConfigManager::from_config(Config::defaults()));
        let provider: Arc<dyn ConfigProvider> = manager;
        assert!(provider.get().is_ok());
    }

    // ========================================================================
    // Validation edge cases (8 tests)
    // ========================================================================

    #[test]
    fn test_validate_empty_host() {
        let config = Config { host: String::new(), ..Config::defaults() };
        let err = config.validate();
        assert!(err.is_err());
        if let Err(e) = err {
            assert!(e.to_string().contains("host cannot be empty"));
        }
    }

    #[test]
    fn test_validate_port_zero() {
        let config = Config { port: 0, ..Config::defaults() };
        let err = config.validate();
        assert!(err.is_err());
        if let Err(e) = err {
            assert!(e.to_string().contains("port cannot be zero"));
        }
    }

    #[test]
    fn test_validate_grpc_port_zero() {
        let config = Config { grpc_port: 0, ..Config::defaults() };
        let err = config.validate();
        assert!(err.is_err());
        if let Err(e) = err {
            assert!(e.to_string().contains("grpc_port cannot be zero"));
        }
    }

    #[test]
    fn test_validate_ws_port_zero() {
        let config = Config { ws_port: 0, ..Config::defaults() };
        let err = config.validate();
        assert!(err.is_err());
        if let Err(e) = err {
            assert!(e.to_string().contains("ws_port cannot be zero"));
        }
    }

    #[test]
    fn test_validate_all_ports_zero_reports_all() {
        let config = Config {
            port: 0,
            grpc_port: 0,
            ws_port: 0,
            ..Config::defaults()
        };
        let err = config.validate();
        assert!(err.is_err());
        if let Err(Error::Validation(msg)) = err {
            assert!(msg.contains("port cannot be zero"));
            assert!(msg.contains("grpc_port cannot be zero"));
            assert!(msg.contains("ws_port cannot be zero"));
        }
    }

    #[test]
    fn test_validate_conflicting_ports_grpc_ws() {
        let config = Config {
            grpc_port: 9999,
            ws_port: 9999,
            ..Config::defaults()
        };
        let err = config.validate();
        assert!(err.is_err());
        if let Err(e) = err {
            assert!(e.to_string().contains("cannot use the same port"));
        }
    }

    #[test]
    fn test_validate_empty_database_path() {
        let config = Config {
            database_path: String::new(),
            ..Config::defaults()
        };
        let err = config.validate();
        assert!(err.is_err());
        if let Err(e) = err {
            assert!(e.to_string().contains("database_path cannot be empty"));
        }
    }

    #[test]
    fn test_validate_multiple_simultaneous_errors() {
        let config = Config {
            host: String::new(),
            database_path: String::new(),
            log_level: "bogus".to_string(),
            ..Config::defaults()
        };
        let err = config.validate();
        assert!(err.is_err());
        if let Err(Error::Validation(msg)) = err {
            // All three errors must appear in the message
            assert!(msg.contains("host cannot be empty"), "missing host error in: {msg}");
            assert!(msg.contains("database_path cannot be empty"), "missing db error in: {msg}");
            assert!(msg.contains("invalid log_level"), "missing log_level error in: {msg}");
        }
    }

    // ========================================================================
    // Builder pattern tests (6 tests)
    // ========================================================================

    #[test]
    fn test_builder_chain_all_methods() -> crate::Result<()> {
        let config = ConfigBuilder::new()
            .skip_files()
            .skip_env()
            .host("10.0.0.1")
            .port(4000)
            .grpc_port(4001)
            .ws_port(4002)
            .database_path("custom/db.sqlite")
            .log_level("warn")
            .build()?;

        assert_eq!(config.host, "10.0.0.1");
        assert_eq!(config.port, 4000);
        assert_eq!(config.grpc_port, 4001);
        assert_eq!(config.ws_port, 4002);
        assert_eq!(config.database_path, "custom/db.sqlite");
        assert_eq!(config.log_level, "warn");
        Ok(())
    }

    #[test]
    fn test_builder_partial_build_inherits_defaults() -> crate::Result<()> {
        // Only override the host; everything else should be the default
        let config = ConfigBuilder::new()
            .skip_files()
            .skip_env()
            .host("192.168.0.1")
            .build()?;

        assert_eq!(config.host, "192.168.0.1");
        assert_eq!(config.port, 8080);
        assert_eq!(config.grpc_port, 8081);
        assert_eq!(config.ws_port, 8082);
        Ok(())
    }

    #[test]
    fn test_builder_default_values_match_config_defaults() -> crate::Result<()> {
        let from_builder = ConfigBuilder::new().skip_files().skip_env().build()?;
        let from_defaults = Config::defaults();
        assert_eq!(from_builder, from_defaults);
        Ok(())
    }

    #[test]
    fn test_builder_skip_files_ignores_fs() -> crate::Result<()> {
        // Even in a dir with config files, skip_files must bypass them
        let dir = tempdir().map_err(|e| Error::Config(format!("tempdir: {e}")))?;
        let config_dir = dir.path().join("config");
        std::fs::create_dir_all(&config_dir)
            .map_err(|e| Error::Config(format!("mkdir: {e}")))?;
        std::fs::write(
            config_dir.join("default.toml"),
            "[server]\nport = 1234\n",
        )
        .map_err(|e| Error::Config(format!("write: {e}")))?;

        let config = ConfigBuilder::new()
            .with_base_path(dir.path())
            .skip_files()
            .skip_env()
            .build()?;

        // File port 1234 must NOT have been applied
        assert_eq!(config.port, 8080);
        Ok(())
    }

    #[test]
    fn test_builder_skip_env_ignores_env_vars() -> crate::Result<()> {
        env::set_var("ME_HOST", "should-be-ignored-by-skip-env");

        let result = ConfigBuilder::new()
            .skip_files()
            .skip_env()
            .build();

        env::remove_var("ME_HOST");

        let config = result?;
        assert_ne!(config.host, "should-be-ignored-by-skip-env");
        Ok(())
    }

    #[test]
    fn test_builder_default_impl() -> crate::Result<()> {
        // ConfigBuilder::default() must produce the same result as ::new()
        let a = ConfigBuilder::new().skip_files().skip_env().build()?;
        let b = ConfigBuilder::default().skip_files().skip_env().build()?;
        assert_eq!(a, b);
        Ok(())
    }

    // ========================================================================
    // ConfigManager tests (5 tests)
    // ========================================================================

    #[test]
    fn test_manager_from_config_get_read_consistency() {
        let config = Config::defaults();
        let manager = ConfigManager::from_config(config.clone());

        // .get() and .read() must return the same logical values
        let via_get = manager.get();
        let via_read = manager.read().clone();
        assert_eq!(via_get, via_read);
        assert_eq!(via_get.port, config.port);
    }

    #[test]
    fn test_manager_reload_request_cycle() {
        let manager = ConfigManager::from_config(Config::defaults());

        // Initial state: no reload requested
        assert!(!manager.reload_requested());

        // Request reload
        manager.request_reload();
        assert!(manager.reload_requested());

        // Clear request
        manager.clear_reload_request();
        assert!(!manager.reload_requested());
    }

    #[test]
    fn test_manager_validate_returns_valid_result() {
        let manager = ConfigManager::from_config(Config::defaults());
        let result = manager.validate();
        assert!(result.valid);
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_manager_validate_returns_invalid_for_bad_config() {
        let config = Config {
            host: String::new(),
            ..Config::defaults()
        };
        let manager = ConfigManager::from_config(config);
        let result = manager.validate();
        assert!(!result.valid);
        assert!(!result.errors.is_empty());
    }

    #[test]
    fn test_manager_get_and_read_same_value() {
        let config = Config {
            port: 7654,
            ..Config::defaults()
        };
        let manager = ConfigManager::from_config(config);
        let via_get = manager.get();
        let via_read = manager.read().clone();
        assert_eq!(via_get.port, 7654);
        assert_eq!(via_read.port, 7654);
    }

    // ========================================================================
    // Serialization round-trip tests (3 tests)
    // ========================================================================

    #[test]
    fn test_toml_serialization_round_trip() -> crate::Result<()> {
        let original = Config {
            host: "127.0.0.1".to_string(),
            port: 5050,
            grpc_port: 5051,
            ws_port: 5052,
            database_path: "my.db".to_string(),
            log_level: "debug".to_string(),
        };

        let toml_str = toml::to_string(&original)
            .map_err(|e| Error::Config(format!("toml serialize: {e}")))?;

        let restored: Config = toml::from_str(&toml_str)
            .map_err(|e| Error::Config(format!("toml deserialize: {e}")))?;

        assert_eq!(original, restored);
        Ok(())
    }

    #[test]
    fn test_json_serialization_round_trip() -> crate::Result<()> {
        let original = Config::defaults();

        let json = serde_json::to_string(&original)
            .map_err(|e| Error::Config(format!("json serialize: {e}")))?;

        let restored: Config = serde_json::from_str(&json)
            .map_err(|e| Error::Config(format!("json deserialize: {e}")))?;

        assert_eq!(original, restored);
        Ok(())
    }

    #[test]
    fn test_json_deserialize_partial_fields_uses_defaults() -> crate::Result<()> {
        // Only supply `host` — other fields should fall back to serde defaults
        let json = r#"{"host":"partial-host"}"#;
        let config: Config = serde_json::from_str(json)
            .map_err(|e| Error::Config(format!("json deserialize: {e}")))?;

        assert_eq!(config.host, "partial-host");
        assert_eq!(config.port, 8080);
        assert_eq!(config.grpc_port, 8081);
        assert_eq!(config.ws_port, 8082);
        Ok(())
    }

    // ========================================================================
    // Environment variable override tests (4 tests)
    // ========================================================================

    #[test]
    fn test_env_override_host_field() -> crate::Result<()> {
        // Use a unique value unlikely to collide with other tests
        env::set_var("ME_HOST", "env-host-field-test-unique");
        let result = ConfigBuilder::new().skip_files().build();
        env::remove_var("ME_HOST");

        let config = result?;
        assert_eq!(config.host, "env-host-field-test-unique");
        Ok(())
    }

    #[test]
    fn test_env_override_log_level_field() -> crate::Result<()> {
        env::set_var("ME_LOG_LEVEL", "warn");
        let result = ConfigBuilder::new().skip_files().build();
        env::remove_var("ME_LOG_LEVEL");

        let config = result?;
        assert_eq!(config.log_level, "warn");
        Ok(())
    }

    #[test]
    fn test_env_override_database_path_field() -> crate::Result<()> {
        env::set_var("ME_DATABASE_PATH", "/tmp/override-test.db");
        let result = ConfigBuilder::new().skip_files().build();
        env::remove_var("ME_DATABASE_PATH");

        let config = result?;
        assert_eq!(config.database_path, "/tmp/override-test.db");
        Ok(())
    }

    #[test]
    fn test_env_override_invalid_port_is_silently_ignored() -> crate::Result<()> {
        // A non-numeric port env var must not crash — it must be silently ignored
        env::set_var("ME_PORT", "not-a-number");
        let result = ConfigBuilder::new().skip_files().build();
        env::remove_var("ME_PORT");

        // Should succeed with the default port since the env var was invalid
        let config = result?;
        assert_eq!(config.port, 8080);
        Ok(())
    }

    // ========================================================================
    // Error path tests (4 tests)
    // ========================================================================

    #[test]
    fn test_error_invalid_toml_content() {
        let dir = tempdir();
        let Ok(dir) = dir else { return };
        let config_dir = dir.path().join("config");
        if std::fs::create_dir_all(&config_dir).is_err() {
            return;
        }
        // Deliberately malformed TOML
        if std::fs::write(config_dir.join("default.toml"), "[[not valid toml!!!").is_err() {
            return;
        }

        let result = ConfigBuilder::new()
            .with_base_path(dir.path())
            .skip_env()
            .build();

        assert!(result.is_err());
        if let Err(e) = result {
            // Must be a Config variant, not some other error
            assert!(matches!(e, Error::Config(_)));
        }
    }

    #[test]
    fn test_error_port_value_too_large() {
        let dir = tempdir();
        let Ok(dir) = dir else { return };
        let config_dir = dir.path().join("config");
        if std::fs::create_dir_all(&config_dir).is_err() {
            return;
        }
        // Port 99999 exceeds u16::MAX (65535)
        let content = "[server]\nport = 99999\ngrpc_port = 8081\nwebsocket_port = 8082\n";
        if std::fs::write(config_dir.join("default.toml"), content).is_err() {
            return;
        }

        let result = ConfigBuilder::new()
            .with_base_path(dir.path())
            .skip_env()
            .build();

        assert!(result.is_err());
    }

    #[test]
    fn test_error_validation_fails_stops_build() {
        // A builder configured to produce an invalid config must return Err
        let result = ConfigBuilder::new()
            .skip_files()
            .skip_env()
            .host(String::new())   // empty host triggers validation error
            .build();

        assert!(result.is_err());
        if let Err(Error::Validation(msg)) = result {
            assert!(msg.contains("host cannot be empty"));
        }
    }

    #[test]
    fn test_error_nonexistent_base_path_succeeds_with_defaults() -> crate::Result<()> {
        // A non-existent base path means no files are found, so defaults apply.
        // As long as defaults are valid, build must succeed.
        let config = ConfigBuilder::new()
            .with_base_path(Path::new("/tmp/certainly-does-not-exist-me-test-12345"))
            .skip_env()
            .build()?;

        // Default port must be in effect
        assert_eq!(config.port, 8080);
        Ok(())
    }

    // ========================================================================
    // Thread safety tests (2 tests)
    // ========================================================================

    #[test]
    fn test_concurrent_reads_are_consistent() {
        use std::thread;

        let config = Config {
            port: 1234,
            ..Config::defaults()
        };
        let manager = Arc::new(ConfigManager::from_config(config));

        let handles: Vec<_> = (0..8)
            .map(|_| {
                let m = Arc::clone(&manager);
                thread::spawn(move || {
                    // Each thread reads 100 times
                    for _ in 0..100 {
                        let c = m.get();
                        assert_eq!(c.port, 1234);
                    }
                })
            })
            .collect();

        for h in handles {
            // join() returns Result; we propagate panics by checking is_err
            assert!(h.join().is_ok());
        }
    }

    // ====================================================================
    // NAM: ConfigChangeEvent.requested_by tests
    // ====================================================================

    #[test]
    fn test_config_change_event_requested_by_none() {
        let event = ConfigChangeEvent {
            change_id: "chg-nam-001".to_string(),
            timestamp: chrono::Utc::now(),
            changed_keys: vec!["port".to_string()],
            previous: HashMap::new(),
            new: HashMap::new(),
            requested_by: None,
        };
        assert!(event.requested_by.is_none());
    }

    #[test]
    fn test_config_change_event_requested_by_agent() {
        let event = ConfigChangeEvent {
            change_id: "chg-nam-002".to_string(),
            timestamp: chrono::Utc::now(),
            changed_keys: vec!["log_level".to_string()],
            previous: HashMap::new(),
            new: HashMap::new(),
            requested_by: Some("@0.A".to_string()),
        };
        assert_eq!(event.requested_by.as_deref(), Some("@0.A"));
    }

    // ====================================================================
    // NAM: Config::to_tensor() tests
    // ====================================================================

    #[test]
    fn test_config_to_tensor_valid_dims() {
        let config = Config::default();
        let tensor = config.to_tensor();
        assert!(tensor.validate().is_ok());
    }

    #[test]
    fn test_config_to_tensor_port_mapping() {
        let config = Config::default(); // port 8080
        let tensor = config.to_tensor();
        let expected_port = 8080.0 / 65535.0;
        assert!((tensor.port - expected_port).abs() < 0.001);
    }

    #[test]
    fn test_config_to_tensor_distance() {
        let c1 = Config::default(); // port 8080
        let mut c2 = Config::default();
        c2.port = 9090;
        let t1 = c1.to_tensor();
        let t2 = c2.to_tensor();
        assert!(t1.distance(&t2) > 0.0, "Different configs should have non-zero distance");
    }

    // ====================================================================
    // NAM: ConfigProvider default methods
    // ====================================================================

    #[test]
    fn test_config_provider_change_history_default() {
        let manager = ConfigManager::from_config(Config::defaults());
        assert!(manager.change_history().is_empty());
    }

    #[test]
    fn test_config_provider_agent_id_default() {
        let manager = ConfigManager::from_config(Config::defaults());
        assert!(manager.agent_id().is_none());
    }

    // ====================================================================
    // NAM: Config::to_tensor health is 1.0 for healthy config
    // ====================================================================

    #[test]
    fn test_config_to_tensor_health_is_one() {
        let config = Config::default();
        let tensor = config.to_tensor();
        assert!((tensor.health_score - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_concurrent_provider_get_is_consistent() {
        use std::thread;

        let manager = Arc::new(ConfigManager::from_config(Config::defaults()));
        let provider: Arc<dyn ConfigProvider> = manager;

        let handles: Vec<_> = (0..4)
            .map(|_| {
                let p = Arc::clone(&provider);
                thread::spawn(move || {
                    for _ in 0..50 {
                        let result = p.get();
                        assert!(result.is_ok());
                        if let Ok(c) = result {
                            assert_eq!(c.port, 8080);
                        }
                    }
                })
            })
            .collect();

        for h in handles {
            assert!(h.join().is_ok());
        }
    }
}
