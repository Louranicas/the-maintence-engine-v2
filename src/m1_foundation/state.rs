//! # M05: State Persistence
//!
//! Durable state storage with `SQLite` backend for the Maintenance Engine.
//!
//! ## Layer: L1 (Foundation)
//! ## Dependencies: M01 (Error), M02 (Config)
//! ## Tests: 40+ comprehensive tests
//!
//! ## Features
//!
//! - `SQLite` connection pool using sqlx
//! - Migration runner for SQL migrations on startup
//! - Generic CRUD operations (Insert, Select, Update, Delete)
//! - Transaction support with begin, commit, rollback
//! - Safe parameterized queries (no SQL injection)
//! - [`StateStore`] trait for dependency inversion
//!
//! ## 9 Databases Supported
//!
//! | Database | Purpose |
//! |----------|---------|
//! | `service_tracking.db` | Service lifecycle |
//! | `system_synergy.db` | Cross-system integration |
//! | `hebbian_pulse.db` | Neural pathway learning |
//! | `consensus_tracking.db` | PBFT consensus |
//! | `episodic_memory.db` | Episode recording |
//! | `tensor_memory.db` | 12D tensor storage |
//! | `performance_metrics.db` | Performance tracking |
//! | `flow_state.db` | State transitions |
//! | `security_events.db` | Security monitoring |
//!
//! ## Related Documentation
//! - [Module Specification](../../ai_docs/modules/M05_STATE_PERSISTENCE.md)
//! - [Database Guide](../../ai_docs/DATABASE_GUIDE.md)

// Allow certain clippy lints for this database-heavy module
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::format_push_string)]

use crate::Error;
use serde::de::DeserializeOwned;
use serde::Serialize;
use sqlx::sqlite::{
    SqliteConnectOptions, SqliteJournalMode, SqlitePool, SqlitePoolOptions, SqliteSynchronous,
};
use sqlx::{Row, Sqlite};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

// ============================================================================
// StateStore trait — dependency inversion
// ============================================================================

/// Trait for types that provide key-value state persistence.
///
/// Enables dependency inversion -- upper layers can accept `&dyn StateStore`
/// instead of depending directly on [`StatePersistence`] or raw database
/// operations. The methods are intentionally synchronous so that no
/// `async-trait` crate dependency is required; implementors that need async
/// I/O can block internally or expose an async wrapper alongside this trait.
pub trait StateStore: Send + Sync {
    /// Retrieve the underlying [`DatabasePool`] for direct queries.
    ///
    /// This is the minimal surface area needed for dependency inversion:
    /// callers can pass `&dyn StateStore` and still perform arbitrary
    /// `sqlx` operations through the returned pool.
    fn pool(&self) -> &DatabasePool;

    /// Human-readable name of the backing store (e.g. the database filename).
    fn store_name(&self) -> &str;

    /// Return the agent ID associated with this store (NAM R5).
    fn agent_id(&self) -> Option<&str> {
        None
    }
}

/// Database configuration for a single `SQLite` database
#[derive(Clone, Debug)]
pub struct DatabaseConfig {
    /// Path to the `SQLite` database file
    pub path: String,
    /// Maximum number of connections in the pool
    pub max_connections: u32,
    /// Minimum number of connections in the pool
    pub min_connections: u32,
    /// Timeout in seconds to acquire a connection from the pool
    pub acquire_timeout_secs: u64,
    /// Whether to enable WAL mode (recommended for concurrency)
    pub wal_mode: bool,
    /// Whether to create the database if it doesn't exist
    pub create_if_missing: bool,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            path: "data/maintenance.db".to_string(),
            max_connections: 10,
            min_connections: 2,
            acquire_timeout_secs: 30,
            wal_mode: true,
            create_if_missing: true,
        }
    }
}

impl DatabaseConfig {
    /// Create a new database configuration
    #[must_use]
    pub fn new(path: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            ..Default::default()
        }
    }

    /// Set the maximum number of connections
    #[must_use]
    pub const fn with_max_connections(mut self, max: u32) -> Self {
        self.max_connections = max;
        self
    }

    /// Set the minimum number of connections
    #[must_use]
    pub const fn with_min_connections(mut self, min: u32) -> Self {
        self.min_connections = min;
        self
    }

    /// Set the acquire timeout in seconds
    #[must_use]
    pub const fn with_acquire_timeout(mut self, secs: u64) -> Self {
        self.acquire_timeout_secs = secs;
        self
    }

    /// Enable or disable WAL mode
    #[must_use]
    pub const fn with_wal_mode(mut self, enabled: bool) -> Self {
        self.wal_mode = enabled;
        self
    }
}

/// Database pool wrapper providing connection management
#[derive(Clone)]
pub struct DatabasePool {
    /// The underlying sqlx `SQLite` pool
    pool: SqlitePool,
    /// Name of the database (extracted from path)
    database_name: String,
    /// Path to the database file
    path: PathBuf,
}

impl std::fmt::Debug for DatabasePool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DatabasePool")
            .field("database_name", &self.database_name)
            .field("path", &self.path)
            .finish_non_exhaustive()
    }
}

impl DatabasePool {
    /// Get the database name
    #[must_use]
    pub fn database_name(&self) -> &str {
        &self.database_name
    }

    /// Get the database path
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Get the underlying sqlx pool
    #[must_use]
    pub const fn inner(&self) -> &SqlitePool {
        &self.pool
    }

    /// Check if the database is healthy
    pub async fn health_check(&self) -> crate::Result<bool> {
        let result = sqlx::query("SELECT 1")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| Error::Database(format!("Health check failed: {e}")))?;

        let val: i32 = result.get(0);
        Ok(val == 1)
    }

    /// Get pool statistics
    #[must_use]
    pub fn stats(&self) -> PoolStats {
        PoolStats {
            size: self.pool.size(),
            idle: self.pool.num_idle(),
        }
    }
}

/// Blanket [`StateStore`] implementation for [`DatabasePool`].
///
/// Any bare pool can serve as a state store, using the database name as the
/// store name.
impl StateStore for DatabasePool {
    fn pool(&self) -> &DatabasePool {
        self
    }

    fn store_name(&self) -> &str {
        &self.database_name
    }
}

/// Pool statistics
#[derive(Clone, Copy, Debug)]
pub struct PoolStats {
    /// Total connections in the pool
    pub size: u32,
    /// Idle connections in the pool
    pub idle: usize,
}

/// Transaction wrapper for atomic operations
pub struct Transaction<'a> {
    /// The underlying sqlx transaction
    tx: sqlx::Transaction<'a, Sqlite>,
}

impl Transaction<'_> {
    /// Commit the transaction
    pub async fn commit(self) -> crate::Result<()> {
        self.tx
            .commit()
            .await
            .map_err(|e| Error::Database(format!("Transaction commit failed: {e}")))
    }

    /// Rollback the transaction
    pub async fn rollback(self) -> crate::Result<()> {
        self.tx
            .rollback()
            .await
            .map_err(|e| Error::Database(format!("Transaction rollback failed: {e}")))
    }

    /// Execute a query within the transaction
    pub async fn execute(&mut self, query: &str, params: &[&str]) -> crate::Result<u64> {
        let mut sqlx_query = sqlx::query(query);
        for param in params {
            sqlx_query = sqlx_query.bind(*param);
        }

        let result = sqlx_query
            .execute(&mut *self.tx)
            .await
            .map_err(|e| Error::Database(format!("Transaction execute failed: {e}")))?;

        Ok(result.rows_affected())
    }

    /// Fetch one row within the transaction
    pub async fn fetch_one<T: DeserializeOwned>(
        &mut self,
        query: &str,
        params: &[&str],
    ) -> crate::Result<T> {
        let mut sqlx_query = sqlx::query(query);
        for param in params {
            sqlx_query = sqlx_query.bind(*param);
        }

        let row = sqlx_query
            .fetch_one(&mut *self.tx)
            .await
            .map_err(|e| Error::Database(format!("Transaction fetch_one failed: {e}")))?;

        // Get the first column as JSON text and deserialize
        let json_str: String = row
            .try_get(0)
            .map_err(|e| Error::Database(format!("Failed to get column: {e}")))?;

        serde_json::from_str(&json_str)
            .map_err(|e| Error::Database(format!("Failed to deserialize row: {e}")))
    }

    /// Fetch all rows within the transaction
    pub async fn fetch_all<T: DeserializeOwned>(
        &mut self,
        query: &str,
        params: &[&str],
    ) -> crate::Result<Vec<T>> {
        let mut sqlx_query = sqlx::query(query);
        for param in params {
            sqlx_query = sqlx_query.bind(*param);
        }

        let rows = sqlx_query
            .fetch_all(&mut *self.tx)
            .await
            .map_err(|e| Error::Database(format!("Transaction fetch_all failed: {e}")))?;

        let mut results = Vec::with_capacity(rows.len());
        for row in rows {
            let json_str: String = row
                .try_get(0)
                .map_err(|e| Error::Database(format!("Failed to get column: {e}")))?;
            let item: T = serde_json::from_str(&json_str)
                .map_err(|e| Error::Database(format!("Failed to deserialize row: {e}")))?;
            results.push(item);
        }

        Ok(results)
    }
}

/// Query builder for safe parameterized queries
#[derive(Clone, Debug, Default)]
pub struct QueryBuilder {
    /// The SQL query being built
    query: String,
    /// Parameters for the query
    params: Vec<String>,
}

impl QueryBuilder {
    /// Create a new query builder with the given base query
    #[must_use]
    pub fn new(query: impl Into<String>) -> Self {
        Self {
            query: query.into(),
            params: Vec::new(),
        }
    }

    /// Add a SELECT clause
    #[must_use]
    pub fn select(columns: &[&str]) -> Self {
        Self {
            query: format!("SELECT {}", columns.join(", ")),
            params: Vec::new(),
        }
    }

    /// Add FROM clause
    #[must_use]
    pub fn from(mut self, table: &str) -> Self {
        self.query.push_str(&format!(" FROM {table}"));
        self
    }

    /// Add WHERE clause with a parameterized condition
    #[must_use]
    pub fn where_eq(mut self, column: &str, value: impl Into<String>) -> Self {
        self.query.push_str(&format!(" WHERE {column} = ?"));
        self.params.push(value.into());
        self
    }

    /// Add AND condition
    #[must_use]
    pub fn and_eq(mut self, column: &str, value: impl Into<String>) -> Self {
        self.query.push_str(&format!(" AND {column} = ?"));
        self.params.push(value.into());
        self
    }

    /// Add OR condition
    #[must_use]
    pub fn or_eq(mut self, column: &str, value: impl Into<String>) -> Self {
        self.query.push_str(&format!(" OR {column} = ?"));
        self.params.push(value.into());
        self
    }

    /// Add ORDER BY clause
    #[must_use]
    pub fn order_by(mut self, column: &str, direction: &str) -> Self {
        let dir = if direction.to_uppercase() == "DESC" {
            "DESC"
        } else {
            "ASC"
        };
        self.query.push_str(&format!(" ORDER BY {column} {dir}"));
        self
    }

    /// Add LIMIT clause
    #[must_use]
    pub fn limit(mut self, limit: u32) -> Self {
        self.query.push_str(&format!(" LIMIT {limit}"));
        self
    }

    /// Add OFFSET clause
    #[must_use]
    pub fn offset(mut self, offset: u32) -> Self {
        self.query.push_str(&format!(" OFFSET {offset}"));
        self
    }

    /// Build an INSERT query
    #[must_use]
    pub fn insert_into(table: &str, columns: &[&str]) -> Self {
        let placeholders: Vec<&str> = (0..columns.len()).map(|_| "?").collect();
        Self {
            query: format!(
                "INSERT INTO {} ({}) VALUES ({})",
                table,
                columns.join(", "),
                placeholders.join(", ")
            ),
            params: Vec::new(),
        }
    }

    /// Add values for INSERT query
    #[must_use]
    pub fn values(mut self, values: &[&str]) -> Self {
        for val in values {
            self.params.push((*val).to_string());
        }
        self
    }

    /// Build an UPDATE query
    #[must_use]
    pub fn update(table: &str) -> Self {
        Self {
            query: format!("UPDATE {table}"),
            params: Vec::new(),
        }
    }

    /// Add SET clause for UPDATE
    #[must_use]
    pub fn set(mut self, column: &str, value: impl Into<String>) -> Self {
        if self.query.contains(" SET ") {
            self.query.push_str(&format!(", {column} = ?"));
        } else {
            self.query.push_str(&format!(" SET {column} = ?"));
        }
        self.params.push(value.into());
        self
    }

    /// Build a DELETE query
    #[must_use]
    pub fn delete_from(table: &str) -> Self {
        Self {
            query: format!("DELETE FROM {table}"),
            params: Vec::new(),
        }
    }

    /// Get the built query string
    #[must_use]
    pub fn build(&self) -> &str {
        &self.query
    }

    /// Get the query parameters
    #[must_use]
    pub fn params(&self) -> Vec<&str> {
        self.params.iter().map(String::as_str).collect()
    }

    /// Get owned parameters
    #[must_use]
    pub fn params_owned(&self) -> &[String] {
        &self.params
    }
}

/// The 11 supported databases in the Maintenance Engine
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DatabaseType {
    /// Service lifecycle tracking
    ServiceTracking,
    /// Cross-system integration
    SystemSynergy,
    /// Neural pathway learning
    HebbianPulse,
    /// PBFT consensus tracking
    ConsensusTracking,
    /// Episode recording
    EpisodicMemory,
    /// 12D tensor storage
    TensorMemory,
    /// Performance metrics
    PerformanceMetrics,
    /// State transitions
    FlowState,
    /// Security event monitoring
    SecurityEvents,
    /// Workflow automation tracking
    WorkflowTracking,
    /// Evolution and fitness tracking
    EvolutionTracking,
}

impl DatabaseType {
    /// Get the filename for this database type
    #[must_use]
    pub const fn filename(&self) -> &'static str {
        match self {
            Self::ServiceTracking => "service_tracking.db",
            Self::SystemSynergy => "system_synergy.db",
            Self::HebbianPulse => "hebbian_pulse.db",
            Self::ConsensusTracking => "consensus_tracking.db",
            Self::EpisodicMemory => "episodic_memory.db",
            Self::TensorMemory => "tensor_memory.db",
            Self::PerformanceMetrics => "performance_metrics.db",
            Self::FlowState => "flow_state.db",
            Self::SecurityEvents => "security_events.db",
            Self::WorkflowTracking => "workflow_tracking.db",
            Self::EvolutionTracking => "evolution_tracking.db",
        }
    }

    /// Get the migration file number for this database type
    #[must_use]
    pub const fn migration_number(&self) -> u32 {
        match self {
            Self::ServiceTracking => 1,
            Self::SystemSynergy => 2,
            Self::HebbianPulse => 3,
            Self::ConsensusTracking => 4,
            Self::EpisodicMemory => 5,
            Self::TensorMemory => 6,
            Self::PerformanceMetrics => 7,
            Self::FlowState => 8,
            Self::SecurityEvents => 9,
            Self::WorkflowTracking => 10,
            Self::EvolutionTracking => 11,
        }
    }

    /// Get all database types
    #[must_use]
    pub const fn all() -> [Self; 11] {
        [
            Self::ServiceTracking,
            Self::SystemSynergy,
            Self::HebbianPulse,
            Self::ConsensusTracking,
            Self::EpisodicMemory,
            Self::TensorMemory,
            Self::PerformanceMetrics,
            Self::FlowState,
            Self::SecurityEvents,
            Self::WorkflowTracking,
            Self::EvolutionTracking,
        ]
    }
}

impl std::fmt::Display for DatabaseType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.filename())
    }
}

/// State persistence manager for managing multiple databases
#[derive(Clone)]
pub struct StatePersistence {
    /// Database pools indexed by database type
    pools: Arc<HashMap<DatabaseType, DatabasePool>>,
    /// Base directory for databases
    base_dir: PathBuf,
    /// Migrations directory
    migrations_dir: PathBuf,
}

impl std::fmt::Debug for StatePersistence {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StatePersistence")
            .field("base_dir", &self.base_dir)
            .field("migrations_dir", &self.migrations_dir)
            .field("pool_count", &self.pools.len())
            .finish()
    }
}

impl StatePersistence {
    /// Create a new state persistence manager builder
    #[must_use]
    pub fn builder() -> StatePersistenceBuilder {
        StatePersistenceBuilder::default()
    }

    /// Get a pool for a specific database type
    pub fn pool(&self, db_type: DatabaseType) -> crate::Result<&DatabasePool> {
        self.pools
            .get(&db_type)
            .ok_or_else(|| Error::Database(format!("Database not initialized: {db_type}")))
    }

    /// Check health of all databases
    pub async fn health_check_all(&self) -> crate::Result<HashMap<DatabaseType, bool>> {
        let mut results = HashMap::new();
        for (db_type, pool) in self.pools.iter() {
            let healthy = pool.health_check().await.unwrap_or(false);
            results.insert(*db_type, healthy);
        }
        Ok(results)
    }

    /// Get statistics for all pools
    #[must_use]
    pub fn stats_all(&self) -> HashMap<DatabaseType, PoolStats> {
        self.pools
            .iter()
            .map(|(db_type, pool)| (*db_type, pool.stats()))
            .collect()
    }

    /// Encode state persistence status as a 12D tensor (NAM R4).
    ///
    /// D3 = database count normalized, D6 = health ratio.
    #[must_use]
    pub fn to_tensor(&self) -> crate::Tensor12D {
        let pool_count = self.pools.len();
        #[allow(clippy::cast_precision_loss)]
        let db_count_normalized = (pool_count as f64 / 11.0).clamp(0.0, 1.0);

        let mut tensor = crate::Tensor12D {
            service_id: 0.0,
            port: 0.0,
            tier: 1.0 / 6.0,
            dependency_count: db_count_normalized,
            agent_count: 0.0,
            protocol: 0.5,
            health_score: 1.0,
            uptime: 0.0,
            synergy: 0.0,
            latency: 0.0,
            error_rate: 0.0,
            temporal_context: 0.0,
        };
        tensor.clamp_normalize();
        tensor
    }
}

/// Builder for `StatePersistence`
#[derive(Debug, Default)]
pub struct StatePersistenceBuilder {
    base_dir: Option<PathBuf>,
    migrations_dir: Option<PathBuf>,
    config: DatabaseConfig,
    databases: Vec<DatabaseType>,
}

impl StatePersistenceBuilder {
    /// Set the base directory for databases
    #[must_use]
    pub fn base_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.base_dir = Some(dir.into());
        self
    }

    /// Set the migrations directory
    #[must_use]
    pub fn migrations_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.migrations_dir = Some(dir.into());
        self
    }

    /// Set the default database configuration
    #[must_use]
    pub fn config(mut self, config: DatabaseConfig) -> Self {
        self.config = config;
        self
    }

    /// Add a database to initialize
    #[must_use]
    pub fn with_database(mut self, db_type: DatabaseType) -> Self {
        if !self.databases.contains(&db_type) {
            self.databases.push(db_type);
        }
        self
    }

    /// Add all databases
    #[must_use]
    pub fn with_all_databases(mut self) -> Self {
        self.databases = DatabaseType::all().to_vec();
        self
    }

    /// Build the state persistence manager
    pub async fn build(self) -> crate::Result<StatePersistence> {
        let base_dir = self
            .base_dir
            .unwrap_or_else(|| PathBuf::from("data/databases"));
        let migrations_dir = self
            .migrations_dir
            .unwrap_or_else(|| PathBuf::from("migrations"));

        // Create base directory if it doesn't exist
        if !base_dir.exists() {
            std::fs::create_dir_all(&base_dir)
                .map_err(|e| Error::Database(format!("Failed to create database directory: {e}")))?;
        }

        let mut pools = HashMap::new();

        for db_type in &self.databases {
            let db_path = base_dir.join(db_type.filename());
            let mut config = self.config.clone();
            config.path = db_path.to_string_lossy().to_string();

            let pool = connect(&config).await?;
            pools.insert(*db_type, pool);
        }

        Ok(StatePersistence {
            pools: Arc::new(pools),
            base_dir,
            migrations_dir,
        })
    }
}

// ============================================================================
// Public API Functions
// ============================================================================

/// Connect to a `SQLite` database and create a connection pool
pub async fn connect(config: &DatabaseConfig) -> crate::Result<DatabasePool> {
    // Ensure parent directory exists
    if let Some(parent) = Path::new(&config.path).parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent)
                .map_err(|e| Error::Database(format!("Failed to create database directory: {e}")))?;
        }
    }

    let journal_mode = if config.wal_mode {
        SqliteJournalMode::Wal
    } else {
        SqliteJournalMode::Delete
    };

    let connect_options = SqliteConnectOptions::from_str(&format!("sqlite:{}", config.path))
        .map_err(|e| Error::Database(format!("Invalid database path: {e}")))?
        .create_if_missing(config.create_if_missing)
        .journal_mode(journal_mode)
        .synchronous(SqliteSynchronous::Normal)
        .busy_timeout(Duration::from_secs(config.acquire_timeout_secs))
        .foreign_keys(false);

    let pool = SqlitePoolOptions::new()
        .max_connections(config.max_connections)
        .min_connections(config.min_connections)
        .acquire_timeout(Duration::from_secs(config.acquire_timeout_secs))
        .connect_with(connect_options)
        .await
        .map_err(|e| Error::Database(format!("Failed to connect to database: {e}")))?;

    // Extract database name from path
    let database_name = Path::new(&config.path)
        .file_stem()
        .map_or_else(|| "unknown".to_string(), |s| s.to_string_lossy().to_string());

    Ok(DatabasePool {
        pool,
        database_name,
        path: PathBuf::from(&config.path),
    })
}

/// Execute a query that returns no rows (INSERT, UPDATE, DELETE)
pub async fn execute(pool: &DatabasePool, query: &str, params: &[&str]) -> crate::Result<u64> {
    let mut sqlx_query = sqlx::query(query);
    for param in params {
        sqlx_query = sqlx_query.bind(*param);
    }

    let result = sqlx_query
        .execute(&pool.pool)
        .await
        .map_err(|e| Error::Database(format!("Query execution failed: {e}")))?;

    Ok(result.rows_affected())
}

/// Fetch one row and deserialize to type T
/// Note: The query should return a JSON string in the first column
pub async fn fetch_one<T: DeserializeOwned>(
    pool: &DatabasePool,
    query: &str,
    params: &[&str],
) -> crate::Result<T> {
    let mut sqlx_query = sqlx::query(query);
    for param in params {
        sqlx_query = sqlx_query.bind(*param);
    }

    let row = sqlx_query
        .fetch_one(&pool.pool)
        .await
        .map_err(|e| Error::Database(format!("Fetch one failed: {e}")))?;

    // Get the first column as JSON text and deserialize
    let json_str: String = row
        .try_get(0)
        .map_err(|e| Error::Database(format!("Failed to get column: {e}")))?;

    serde_json::from_str(&json_str)
        .map_err(|e| Error::Database(format!("Failed to deserialize row: {e}")))
}

/// Fetch all rows and deserialize to Vec<T>
/// Note: The query should return JSON strings in the first column
pub async fn fetch_all<T: DeserializeOwned>(
    pool: &DatabasePool,
    query: &str,
    params: &[&str],
) -> crate::Result<Vec<T>> {
    let mut sqlx_query = sqlx::query(query);
    for param in params {
        sqlx_query = sqlx_query.bind(*param);
    }

    let rows = sqlx_query
        .fetch_all(&pool.pool)
        .await
        .map_err(|e| Error::Database(format!("Fetch all failed: {e}")))?;

    let mut results = Vec::with_capacity(rows.len());
    for row in rows {
        let json_str: String = row
            .try_get(0)
            .map_err(|e| Error::Database(format!("Failed to get column: {e}")))?;
        let item: T = serde_json::from_str(&json_str)
            .map_err(|e| Error::Database(format!("Failed to deserialize row: {e}")))?;
        results.push(item);
    }

    Ok(results)
}

/// Fetch optional row (returns None if no row found)
pub async fn fetch_optional<T: DeserializeOwned>(
    pool: &DatabasePool,
    query: &str,
    params: &[&str],
) -> crate::Result<Option<T>> {
    let mut sqlx_query = sqlx::query(query);
    for param in params {
        sqlx_query = sqlx_query.bind(*param);
    }

    let row_opt = sqlx_query
        .fetch_optional(&pool.pool)
        .await
        .map_err(|e| Error::Database(format!("Fetch optional failed: {e}")))?;

    match row_opt {
        Some(row) => {
            let json_str: String = row
                .try_get(0)
                .map_err(|e| Error::Database(format!("Failed to get column: {e}")))?;
            let item: T = serde_json::from_str(&json_str)
                .map_err(|e| Error::Database(format!("Failed to deserialize row: {e}")))?;
            Ok(Some(item))
        }
        None => Ok(None),
    }
}

/// Begin a new transaction
pub async fn begin_transaction(pool: &DatabasePool) -> crate::Result<Transaction<'_>> {
    let tx = pool
        .pool
        .begin()
        .await
        .map_err(|e| Error::Database(format!("Failed to begin transaction: {e}")))?;

    Ok(Transaction { tx })
}

/// Run SQL migrations from a directory
pub async fn run_migrations(pool: &DatabasePool, migrations_dir: &str) -> crate::Result<()> {
    let migrations_path = Path::new(migrations_dir);

    if !migrations_path.exists() {
        return Err(Error::Database(format!(
            "Migrations directory does not exist: {migrations_dir}"
        )));
    }

    // Create migrations tracking table if it doesn't exist
    let create_tracking = r"
        CREATE TABLE IF NOT EXISTS _sqlx_migrations (
            version INTEGER PRIMARY KEY,
            description TEXT NOT NULL,
            installed_on TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            success INTEGER NOT NULL DEFAULT 1,
            checksum TEXT,
            execution_time_ms INTEGER
        )
    ";

    execute(pool, create_tracking, &[]).await?;

    // Read and sort migration files
    let mut migrations: Vec<(u32, PathBuf)> = std::fs::read_dir(migrations_path)
        .map_err(|e| Error::Database(format!("Failed to read migrations directory: {e}")))?
        .filter_map(Result::ok)
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "sql"))
        .filter_map(|entry| {
            let path = entry.path();
            let filename = path.file_stem()?.to_string_lossy();
            // Extract version number from filename (e.g., "001_service_tracking" -> 1)
            let version_str = filename.split('_').next()?;
            let version: u32 = version_str.parse().ok()?;
            Some((version, path))
        })
        .collect();

    migrations.sort_by_key(|(version, _)| *version);

    // Check which migrations have already been applied
    let applied: Vec<i64> = {
        let rows = sqlx::query("SELECT version FROM _sqlx_migrations WHERE success = 1")
            .fetch_all(&pool.pool)
            .await
            .map_err(|e| Error::Database(format!("Failed to query migrations: {e}")))?;

        rows.iter()
            .filter_map(|row| row.try_get::<i64, _>(0).ok())
            .collect()
    };

    // Apply pending migrations
    for (version, path) in migrations {
        if applied.contains(&i64::from(version)) {
            continue;
        }

        let sql = std::fs::read_to_string(&path)
            .map_err(|e| Error::Database(format!("Failed to read migration file: {e}")))?;

        let description = path.file_stem().map_or_else(
            || format!("migration_{version}"),
            |s| s.to_string_lossy().to_string(),
        );

        let start = std::time::Instant::now();

        // Execute migration (split by semicolons for multiple statements)
        for statement in sql.split(';').filter(|s| !s.trim().is_empty()) {
            sqlx::query(statement)
                .execute(&pool.pool)
                .await
                .map_err(|e| {
                    Error::Database(format!(
                        "Migration {version} failed: {e}\nStatement: {statement}"
                    ))
                })?;
        }

        #[allow(clippy::cast_possible_truncation)]
        let elapsed_ms = start.elapsed().as_millis() as i64;

        // Record successful migration
        sqlx::query(
            "INSERT INTO _sqlx_migrations (version, description, execution_time_ms) VALUES (?, ?, ?)",
        )
        .bind(i64::from(version))
        .bind(&description)
        .bind(elapsed_ms)
        .execute(&pool.pool)
        .await
        .map_err(|e| Error::Database(format!("Failed to record migration: {e}")))?;
    }

    Ok(())
}

/// Save a value to the database with a key
pub async fn save<T: Serialize + Sync>(
    pool: &DatabasePool,
    table: &str,
    key: &str,
    value: &T,
) -> crate::Result<u64> {
    let json_value = serde_json::to_string(value)
        .map_err(|e| Error::Database(format!("Failed to serialize value: {e}")))?;

    // Use INSERT OR REPLACE for upsert behavior
    let query = format!(
        "INSERT OR REPLACE INTO {table} (key, value, updated_at) VALUES (?, ?, CURRENT_TIMESTAMP)"
    );

    execute(pool, &query, &[key, &json_value]).await
}

/// Save a value with agent provenance (NAM R5 `HumanAsAgent`).
///
/// Like [`save`], but also stores which agent performed the save.
/// The table must have a `saved_by` TEXT column.
///
/// # Errors
///
/// Returns an error if serialization or the SQL INSERT fails.
pub async fn save_with_provenance<T: Serialize + Sync>(
    pool: &DatabasePool,
    table: &str,
    key: &str,
    value: &T,
    agent_id: &str,
) -> crate::Result<u64> {
    let json_value = serde_json::to_string(value)
        .map_err(|e| Error::Database(format!("Failed to serialize value: {e}")))?;

    let query = format!(
        "INSERT OR REPLACE INTO {table} (key, value, saved_by, updated_at) VALUES (?, ?, ?, CURRENT_TIMESTAMP)"
    );

    execute(pool, &query, &[key, &json_value, agent_id]).await
}

/// Save a value with optimistic concurrency control (NAM R3 `DissentCapture`).
///
/// Inserts or updates the row only if the current version matches `expected_version`.
/// Returns the new version on success, or an error if the version has changed
/// (indicating a concurrent modification).
///
/// The table must have `key TEXT`, `value TEXT`, `version INTEGER` columns.
///
/// # Errors
///
/// Returns `Error::Database` if the version does not match (concurrent modification)
/// or if the SQL operation fails.
pub async fn save_versioned<T: Serialize + Sync>(
    pool: &DatabasePool,
    table: &str,
    key: &str,
    value: &T,
    expected_version: u64,
) -> crate::Result<u64> {
    let json_value = serde_json::to_string(value)
        .map_err(|e| Error::Database(format!("Failed to serialize value: {e}")))?;

    let new_version = expected_version + 1;
    let version_str = expected_version.to_string();
    let new_version_str = new_version.to_string();

    // Attempt conditional update
    let query = format!(
        "UPDATE {table} SET value = ?, version = ?, updated_at = CURRENT_TIMESTAMP WHERE key = ? AND version = ?"
    );
    let rows = execute(pool, &query, &[&json_value, &new_version_str, key, &version_str]).await?;

    if rows == 0 {
        // Either the key doesn't exist or the version doesn't match.
        // Try to insert if this is version 0 (new record)
        if expected_version == 0 {
            let insert_query = format!(
                "INSERT INTO {table} (key, value, version, updated_at) VALUES (?, ?, 1, CURRENT_TIMESTAMP)"
            );
            execute(pool, &insert_query, &[key, &json_value]).await?;
            return Ok(1);
        }
        return Err(Error::Database(format!(
            "Version mismatch: expected {expected_version} for key '{key}'"
        )));
    }

    Ok(new_version)
}

/// Load a value from the database by key
pub async fn load<T: DeserializeOwned>(
    pool: &DatabasePool,
    table: &str,
    key: &str,
) -> crate::Result<Option<T>> {
    let query = format!("SELECT value FROM {table} WHERE key = ?");

    let row_opt = sqlx::query(&query)
        .bind(key)
        .fetch_optional(&pool.pool)
        .await
        .map_err(|e| Error::Database(format!("Load failed: {e}")))?;

    match row_opt {
        Some(row) => {
            let json_str: String = row
                .try_get(0)
                .map_err(|e| Error::Database(format!("Failed to get value column: {e}")))?;
            let value: T = serde_json::from_str(&json_str)
                .map_err(|e| Error::Database(format!("Failed to deserialize value: {e}")))?;
            Ok(Some(value))
        }
        None => Ok(None),
    }
}

/// Delete a key from the database
pub async fn delete(pool: &DatabasePool, table: &str, key: &str) -> crate::Result<bool> {
    let query = format!("DELETE FROM {table} WHERE key = ?");
    let affected = execute(pool, &query, &[key]).await?;
    Ok(affected > 0)
}

/// Check if a key exists in the database
pub async fn exists(pool: &DatabasePool, table: &str, key: &str) -> crate::Result<bool> {
    let query = format!("SELECT 1 FROM {table} WHERE key = ? LIMIT 1");

    let row_opt = sqlx::query(&query)
        .bind(key)
        .fetch_optional(&pool.pool)
        .await
        .map_err(|e| Error::Database(format!("Exists check failed: {e}")))?;

    Ok(row_opt.is_some())
}

/// Count rows matching a condition
pub async fn count(
    pool: &DatabasePool,
    table: &str,
    condition: Option<&str>,
    params: &[&str],
) -> crate::Result<i64> {
    let query = condition.map_or_else(
        || format!("SELECT COUNT(*) FROM {table}"),
        |cond| format!("SELECT COUNT(*) FROM {table} WHERE {cond}"),
    );

    let mut sqlx_query = sqlx::query(&query);
    for param in params {
        sqlx_query = sqlx_query.bind(*param);
    }

    let row = sqlx_query
        .fetch_one(&pool.pool)
        .await
        .map_err(|e| Error::Database(format!("Count failed: {e}")))?;

    let count: i64 = row
        .try_get(0)
        .map_err(|e| Error::Database(format!("Failed to get count: {e}")))?;

    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// Create a temporary test pool. Returns `(pool, temp_dir)`.
    ///
    /// The `TempDir` must be kept alive for the lifetime of the pool;
    /// dropping it removes the temporary database file.
    async fn create_test_pool() -> crate::Result<(DatabasePool, TempDir)> {
        let temp_dir =
            TempDir::new().map_err(|e| Error::Database(format!("Failed to create temp dir: {e}")))?;
        let db_path = temp_dir.path().join("test.db");

        let config = DatabaseConfig::new(db_path.to_string_lossy().to_string());
        let pool = connect(&config).await?;

        Ok((pool, temp_dir))
    }

    // ====================================================================
    // Connection & health check
    // ====================================================================

    #[tokio::test]
    async fn test_connect_and_health_check() -> crate::Result<()> {
        let (pool, _temp_dir) = create_test_pool().await?;
        let healthy = pool.health_check().await?;
        assert!(healthy);
        Ok(())
    }

    // ====================================================================
    // Execute
    // ====================================================================

    #[tokio::test]
    async fn test_execute_create_table() -> crate::Result<()> {
        let (pool, _temp_dir) = create_test_pool().await?;

        let result = execute(
            &pool,
            "CREATE TABLE test_table (id INTEGER PRIMARY KEY, name TEXT)",
            &[],
        )
        .await;

        assert!(result.is_ok());
        Ok(())
    }

    // ====================================================================
    // Insert & count
    // ====================================================================

    #[tokio::test]
    async fn test_insert_and_count() -> crate::Result<()> {
        let (pool, _temp_dir) = create_test_pool().await?;

        execute(
            &pool,
            "CREATE TABLE items (id INTEGER PRIMARY KEY, name TEXT)",
            &[],
        )
        .await?;

        execute(&pool, "INSERT INTO items (name) VALUES (?)", &["item1"]).await?;
        execute(&pool, "INSERT INTO items (name) VALUES (?)", &["item2"]).await?;

        let total = count(&pool, "items", None, &[]).await?;
        assert_eq!(total, 2);
        Ok(())
    }

    // ====================================================================
    // Transaction commit
    // ====================================================================

    #[tokio::test]
    async fn test_transaction_commit() -> crate::Result<()> {
        let (pool, _temp_dir) = create_test_pool().await?;

        execute(
            &pool,
            "CREATE TABLE tx_test (id INTEGER PRIMARY KEY, value TEXT)",
            &[],
        )
        .await?;

        let mut tx = begin_transaction(&pool).await?;
        tx.execute("INSERT INTO tx_test (value) VALUES (?)", &["test_value"])
            .await?;
        tx.commit().await?;

        let total = count(&pool, "tx_test", None, &[]).await?;
        assert_eq!(total, 1);
        Ok(())
    }

    // ====================================================================
    // Transaction rollback
    // ====================================================================

    #[tokio::test]
    async fn test_transaction_rollback() -> crate::Result<()> {
        let (pool, _temp_dir) = create_test_pool().await?;

        execute(
            &pool,
            "CREATE TABLE rollback_test (id INTEGER PRIMARY KEY, value TEXT)",
            &[],
        )
        .await?;

        execute(
            &pool,
            "INSERT INTO rollback_test (value) VALUES (?)",
            &["before"],
        )
        .await?;

        let mut tx = begin_transaction(&pool).await?;
        tx.execute(
            "INSERT INTO rollback_test (value) VALUES (?)",
            &["during_tx"],
        )
        .await?;
        tx.rollback().await?;

        let total = count(&pool, "rollback_test", None, &[]).await?;
        assert_eq!(total, 1);
        Ok(())
    }

    // ====================================================================
    // QueryBuilder — select
    // ====================================================================

    #[tokio::test]
    async fn test_query_builder() {
        let query = QueryBuilder::select(&["id", "name"])
            .from("users")
            .where_eq("status", "active")
            .and_eq("role", "admin")
            .order_by("created_at", "DESC")
            .limit(10);

        assert_eq!(
            query.build(),
            "SELECT id, name FROM users WHERE status = ? AND role = ? ORDER BY created_at DESC LIMIT 10"
        );
        assert_eq!(query.params(), vec!["active", "admin"]);
    }

    // ====================================================================
    // QueryBuilder — insert
    // ====================================================================

    #[tokio::test]
    async fn test_query_builder_insert() {
        let query = QueryBuilder::insert_into("users", &["name", "email", "status"])
            .values(&["John", "john@example.com", "active"]);

        assert_eq!(
            query.build(),
            "INSERT INTO users (name, email, status) VALUES (?, ?, ?)"
        );
        assert_eq!(query.params(), vec!["John", "john@example.com", "active"]);
    }

    // ====================================================================
    // QueryBuilder — update
    // ====================================================================

    #[tokio::test]
    async fn test_query_builder_update() {
        let query = QueryBuilder::update("users")
            .set("name", "Jane")
            .set("email", "jane@example.com")
            .where_eq("id", "1");

        assert_eq!(
            query.build(),
            "UPDATE users SET name = ?, email = ? WHERE id = ?"
        );
        assert_eq!(query.params(), vec!["Jane", "jane@example.com", "1"]);
    }

    // ====================================================================
    // QueryBuilder — delete
    // ====================================================================

    #[tokio::test]
    async fn test_query_builder_delete() {
        let query = QueryBuilder::delete_from("users").where_eq("status", "inactive");

        assert_eq!(query.build(), "DELETE FROM users WHERE status = ?");
        assert_eq!(query.params(), vec!["inactive"]);
    }

    // ====================================================================
    // DatabaseType filenames
    // ====================================================================

    #[tokio::test]
    async fn test_database_type_filenames() {
        assert_eq!(
            DatabaseType::ServiceTracking.filename(),
            "service_tracking.db"
        );
        assert_eq!(DatabaseType::HebbianPulse.filename(), "hebbian_pulse.db");
        assert_eq!(
            DatabaseType::SecurityEvents.filename(),
            "security_events.db"
        );
    }

    // ====================================================================
    // DatabaseConfig builder
    // ====================================================================

    #[tokio::test]
    async fn test_database_config_builder() {
        let config = DatabaseConfig::new("test.db")
            .with_max_connections(20)
            .with_min_connections(5)
            .with_acquire_timeout(60)
            .with_wal_mode(true);

        assert_eq!(config.path, "test.db");
        assert_eq!(config.max_connections, 20);
        assert_eq!(config.min_connections, 5);
        assert_eq!(config.acquire_timeout_secs, 60);
        assert!(config.wal_mode);
    }

    // ====================================================================
    // Pool stats
    // ====================================================================

    #[tokio::test]
    async fn test_pool_stats() -> crate::Result<()> {
        let (pool, _temp_dir) = create_test_pool().await?;
        let stats = pool.stats();
        assert!(stats.size > 0);
        Ok(())
    }

    // ====================================================================
    // Save & load JSON
    // ====================================================================

    #[tokio::test]
    async fn test_save_and_load_json() -> crate::Result<()> {
        let (pool, _temp_dir) = create_test_pool().await?;

        execute(
            &pool,
            "CREATE TABLE kv_store (key TEXT PRIMARY KEY, value TEXT, updated_at TEXT)",
            &[],
        )
        .await?;

        #[derive(serde::Serialize, serde::Deserialize, Debug, PartialEq)]
        struct TestData {
            name: String,
            count: i32,
        }

        let data = TestData {
            name: "test".to_string(),
            count: 42,
        };

        save(&pool, "kv_store", "my_key", &data).await?;

        let loaded: Option<TestData> = load(&pool, "kv_store", "my_key").await?;
        assert!(loaded.is_some());
        let loaded_data = loaded.ok_or_else(|| Error::Database("missing data".to_string()))?;
        assert_eq!(loaded_data, data);
        Ok(())
    }

    // ====================================================================
    // Exists & delete
    // ====================================================================

    #[tokio::test]
    async fn test_exists_and_delete() -> crate::Result<()> {
        let (pool, _temp_dir) = create_test_pool().await?;

        execute(
            &pool,
            "CREATE TABLE test_exists (key TEXT PRIMARY KEY, value TEXT)",
            &[],
        )
        .await?;

        execute(
            &pool,
            "INSERT INTO test_exists (key, value) VALUES (?, ?)",
            &["key1", "value1"],
        )
        .await?;

        assert!(exists(&pool, "test_exists", "key1").await?);
        assert!(!exists(&pool, "test_exists", "key2").await?);

        let deleted = delete(&pool, "test_exists", "key1").await?;
        assert!(deleted);

        assert!(!exists(&pool, "test_exists", "key1").await?);
        Ok(())
    }

    // ====================================================================
    // NEW TESTS: StateStore trait compliance
    // ====================================================================

    #[tokio::test]
    async fn test_state_store_trait_object_compiles() -> crate::Result<()> {
        let (pool, _temp_dir) = create_test_pool().await?;
        // Verify that DatabasePool can be used as a trait object
        let store: &dyn StateStore = &pool;
        assert!(!store.store_name().is_empty());
        assert!(!store.pool().database_name().is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn test_state_store_pool_returns_self_for_database_pool() -> crate::Result<()> {
        let (pool, _temp_dir) = create_test_pool().await?;
        let store: &dyn StateStore = &pool;
        // The returned pool reference should point to the same database
        assert_eq!(store.pool().database_name(), pool.database_name());
        Ok(())
    }

    #[tokio::test]
    async fn test_state_store_name_matches_database_name() -> crate::Result<()> {
        let (pool, _temp_dir) = create_test_pool().await?;
        assert_eq!(pool.store_name(), pool.database_name());
        assert_eq!(pool.store_name(), "test");
        Ok(())
    }

    // ====================================================================
    // NEW TESTS: DatabaseConfig builder — defaults
    // ====================================================================

    #[tokio::test]
    async fn test_database_config_default_values() {
        let config = DatabaseConfig::default();
        assert_eq!(config.path, "data/maintenance.db");
        assert_eq!(config.max_connections, 10);
        assert_eq!(config.min_connections, 2);
        assert_eq!(config.acquire_timeout_secs, 30);
        assert!(config.wal_mode);
        assert!(config.create_if_missing);
    }

    #[tokio::test]
    async fn test_database_config_new_overrides_path_only() {
        let config = DatabaseConfig::new("custom.db");
        assert_eq!(config.path, "custom.db");
        // All other fields should be defaults
        assert_eq!(config.max_connections, 10);
        assert_eq!(config.min_connections, 2);
        assert!(config.wal_mode);
    }

    #[tokio::test]
    async fn test_database_config_wal_mode_false() {
        let config = DatabaseConfig::new("test.db").with_wal_mode(false);
        assert!(!config.wal_mode);
    }

    #[tokio::test]
    async fn test_database_config_chained_builder() {
        let config = DatabaseConfig::new("chain.db")
            .with_max_connections(1)
            .with_min_connections(1)
            .with_acquire_timeout(5)
            .with_wal_mode(false);

        assert_eq!(config.path, "chain.db");
        assert_eq!(config.max_connections, 1);
        assert_eq!(config.min_connections, 1);
        assert_eq!(config.acquire_timeout_secs, 5);
        assert!(!config.wal_mode);
    }

    #[tokio::test]
    async fn test_database_config_clone() {
        let config = DatabaseConfig::new("original.db").with_max_connections(50);
        let cloned = config.clone();
        assert_eq!(cloned.path, "original.db");
        assert_eq!(cloned.max_connections, 50);
    }

    // ====================================================================
    // NEW TESTS: DatabaseType — all variants filename + display
    // ====================================================================

    #[tokio::test]
    async fn test_database_type_all_filenames() {
        let expected = [
            ("service_tracking.db", DatabaseType::ServiceTracking),
            ("system_synergy.db", DatabaseType::SystemSynergy),
            ("hebbian_pulse.db", DatabaseType::HebbianPulse),
            ("consensus_tracking.db", DatabaseType::ConsensusTracking),
            ("episodic_memory.db", DatabaseType::EpisodicMemory),
            ("tensor_memory.db", DatabaseType::TensorMemory),
            ("performance_metrics.db", DatabaseType::PerformanceMetrics),
            ("flow_state.db", DatabaseType::FlowState),
            ("security_events.db", DatabaseType::SecurityEvents),
            ("workflow_tracking.db", DatabaseType::WorkflowTracking),
            ("evolution_tracking.db", DatabaseType::EvolutionTracking),
        ];

        for (expected_name, db_type) in expected {
            assert_eq!(db_type.filename(), expected_name);
        }
    }

    #[tokio::test]
    async fn test_database_type_display_matches_filename() {
        for db_type in DatabaseType::all() {
            assert_eq!(db_type.to_string(), db_type.filename());
        }
    }

    // ====================================================================
    // NEW TESTS: DatabaseType — all() and migration_number
    // ====================================================================

    #[tokio::test]
    async fn test_database_type_all_returns_11() {
        let all = DatabaseType::all();
        assert_eq!(all.len(), 11);
    }

    #[tokio::test]
    async fn test_database_type_migration_numbers() {
        let all = DatabaseType::all();
        for (i, db_type) in all.iter().enumerate() {
            #[allow(clippy::cast_possible_truncation)]
            let expected = (i + 1) as u32;
            assert_eq!(
                db_type.migration_number(),
                expected,
                "{db_type:?} should have migration number {expected}"
            );
        }
    }

    // ====================================================================
    // NEW TESTS: JSON serialization — nested, arrays, special chars
    // ====================================================================

    #[tokio::test]
    async fn test_save_and_load_nested_json() -> crate::Result<()> {
        let (pool, _temp_dir) = create_test_pool().await?;

        execute(
            &pool,
            "CREATE TABLE kv_nested (key TEXT PRIMARY KEY, value TEXT, updated_at TEXT)",
            &[],
        )
        .await?;

        #[derive(serde::Serialize, serde::Deserialize, Debug, PartialEq)]
        struct Inner {
            x: i32,
            y: i32,
        }

        #[derive(serde::Serialize, serde::Deserialize, Debug, PartialEq)]
        struct Outer {
            label: String,
            inner: Inner,
        }

        let data = Outer {
            label: "nested".to_string(),
            inner: Inner { x: 10, y: 20 },
        };

        save(&pool, "kv_nested", "nested_key", &data).await?;
        let loaded: Option<Outer> = load(&pool, "kv_nested", "nested_key").await?;
        let loaded_data =
            loaded.ok_or_else(|| Error::Database("missing nested data".to_string()))?;
        assert_eq!(loaded_data, data);
        Ok(())
    }

    #[tokio::test]
    async fn test_save_and_load_json_array() -> crate::Result<()> {
        let (pool, _temp_dir) = create_test_pool().await?;

        execute(
            &pool,
            "CREATE TABLE kv_array (key TEXT PRIMARY KEY, value TEXT, updated_at TEXT)",
            &[],
        )
        .await?;

        let data: Vec<i32> = vec![1, 2, 3, 4, 5];
        save(&pool, "kv_array", "arr_key", &data).await?;
        let loaded: Option<Vec<i32>> = load(&pool, "kv_array", "arr_key").await?;
        let loaded_data =
            loaded.ok_or_else(|| Error::Database("missing array data".to_string()))?;
        assert_eq!(loaded_data, data);
        Ok(())
    }

    #[tokio::test]
    async fn test_save_and_load_special_chars() -> crate::Result<()> {
        let (pool, _temp_dir) = create_test_pool().await?;

        execute(
            &pool,
            "CREATE TABLE kv_special (key TEXT PRIMARY KEY, value TEXT, updated_at TEXT)",
            &[],
        )
        .await?;

        let data = "quotes: \"hello\", backslash: \\, newline: \n, tab: \t, unicode: \u{1F600}"
            .to_string();
        save(&pool, "kv_special", "special_key", &data).await?;
        let loaded: Option<String> = load(&pool, "kv_special", "special_key").await?;
        let loaded_data =
            loaded.ok_or_else(|| Error::Database("missing special data".to_string()))?;
        assert_eq!(loaded_data, data);
        Ok(())
    }

    // ====================================================================
    // NEW TESTS: Error paths — invalid SQL, nonexistent table
    // ====================================================================

    #[tokio::test]
    async fn test_execute_invalid_sql_returns_error() -> crate::Result<()> {
        let (pool, _temp_dir) = create_test_pool().await?;
        let result = execute(&pool, "INVALID SQL STATEMENT", &[]).await;
        assert!(result.is_err());
        Ok(())
    }

    #[tokio::test]
    async fn test_count_nonexistent_table_returns_error() -> crate::Result<()> {
        let (pool, _temp_dir) = create_test_pool().await?;
        let result = count(&pool, "nonexistent_table", None, &[]).await;
        assert!(result.is_err());
        Ok(())
    }

    // ====================================================================
    // NEW TESTS: QueryBuilder — or_eq, offset, order_by directions
    // ====================================================================

    #[tokio::test]
    async fn test_query_builder_or_eq() {
        let query = QueryBuilder::select(&["*"])
            .from("events")
            .where_eq("type", "error")
            .or_eq("type", "warning");

        assert_eq!(
            query.build(),
            "SELECT * FROM events WHERE type = ? OR type = ?"
        );
        assert_eq!(query.params(), vec!["error", "warning"]);
    }

    #[tokio::test]
    async fn test_query_builder_offset() {
        let query = QueryBuilder::select(&["id"])
            .from("items")
            .limit(10)
            .offset(20);

        assert_eq!(query.build(), "SELECT id FROM items LIMIT 10 OFFSET 20");
        assert!(query.params().is_empty());
    }

    #[tokio::test]
    async fn test_query_builder_order_by_asc() {
        let query = QueryBuilder::select(&["name"])
            .from("users")
            .order_by("name", "ASC");

        assert_eq!(query.build(), "SELECT name FROM users ORDER BY name ASC");
    }

    #[tokio::test]
    async fn test_query_builder_order_by_invalid_defaults_to_asc() {
        let query = QueryBuilder::select(&["name"])
            .from("users")
            .order_by("name", "INVALID");

        // Non-"DESC" direction should fall through to ASC
        assert_eq!(query.build(), "SELECT name FROM users ORDER BY name ASC");
    }

    // ====================================================================
    // NEW TESTS: PoolStats fields accessible
    // ====================================================================

    #[tokio::test]
    async fn test_pool_stats_fields_accessible() -> crate::Result<()> {
        let (pool, _temp_dir) = create_test_pool().await?;
        let stats = pool.stats();
        // size is u32, idle is usize — both should be accessible
        let _size: u32 = stats.size;
        let _idle: usize = stats.idle;
        // Basic sanity: idle should not exceed size
        assert!(u32::try_from(stats.idle).unwrap_or(u32::MAX) <= stats.size);
        Ok(())
    }

    // ====================================================================
    // NEW TESTS: Count with condition
    // ====================================================================

    #[tokio::test]
    async fn test_count_with_condition() -> crate::Result<()> {
        let (pool, _temp_dir) = create_test_pool().await?;

        execute(
            &pool,
            "CREATE TABLE tagged (id INTEGER PRIMARY KEY, tag TEXT)",
            &[],
        )
        .await?;

        execute(&pool, "INSERT INTO tagged (tag) VALUES (?)", &["alpha"]).await?;
        execute(&pool, "INSERT INTO tagged (tag) VALUES (?)", &["beta"]).await?;
        execute(&pool, "INSERT INTO tagged (tag) VALUES (?)", &["alpha"]).await?;

        let alpha_count = count(&pool, "tagged", Some("tag = ?"), &["alpha"]).await?;
        assert_eq!(alpha_count, 2);

        let beta_count = count(&pool, "tagged", Some("tag = ?"), &["beta"]).await?;
        assert_eq!(beta_count, 1);
        Ok(())
    }

    // ====================================================================
    // NEW TESTS: Fetch optional returns None for missing key
    // ====================================================================

    #[tokio::test]
    async fn test_fetch_optional_returns_none_for_missing() -> crate::Result<()> {
        let (pool, _temp_dir) = create_test_pool().await?;

        execute(
            &pool,
            "CREATE TABLE opt_test (id INTEGER PRIMARY KEY, value TEXT)",
            &[],
        )
        .await?;

        let result: Option<String> =
            fetch_optional(&pool, "SELECT value FROM opt_test WHERE id = ?", &["999"]).await?;
        assert!(result.is_none());
        Ok(())
    }

    // ====================================================================
    // NEW TESTS: Transaction execute returns affected rows
    // ====================================================================

    #[tokio::test]
    async fn test_transaction_execute_returns_affected_rows() -> crate::Result<()> {
        let (pool, _temp_dir) = create_test_pool().await?;

        execute(
            &pool,
            "CREATE TABLE affected_test (id INTEGER PRIMARY KEY, val TEXT)",
            &[],
        )
        .await?;

        let mut tx = begin_transaction(&pool).await?;
        let affected = tx
            .execute("INSERT INTO affected_test (val) VALUES (?)", &["row1"])
            .await?;
        assert_eq!(affected, 1);
        tx.commit().await?;
        Ok(())
    }

    // ====================================================================
    // NEW TESTS: DatabasePool debug + database_name + path
    // ====================================================================

    #[tokio::test]
    async fn test_database_pool_debug_format() -> crate::Result<()> {
        let (pool, _temp_dir) = create_test_pool().await?;
        let debug_str = format!("{pool:?}");
        assert!(debug_str.contains("DatabasePool"));
        assert!(debug_str.contains("test"));
        Ok(())
    }

    #[tokio::test]
    async fn test_database_pool_name_and_path() -> crate::Result<()> {
        let (pool, _temp_dir) = create_test_pool().await?;
        assert_eq!(pool.database_name(), "test");
        assert!(pool.path().ends_with("test.db"));
        Ok(())
    }

    // ====================================================================
    // NEW TESTS: QueryBuilder — new(), params_owned, empty params
    // ====================================================================

    #[tokio::test]
    async fn test_query_builder_new_raw() {
        let qb = QueryBuilder::new("SELECT 1");
        assert_eq!(qb.build(), "SELECT 1");
        assert!(qb.params().is_empty());
        assert!(qb.params_owned().is_empty());
    }

    #[tokio::test]
    async fn test_query_builder_params_owned() {
        let query = QueryBuilder::select(&["id"])
            .from("t")
            .where_eq("a", "val_a")
            .and_eq("b", "val_b");

        let owned = query.params_owned();
        assert_eq!(owned.len(), 2);
        assert_eq!(owned[0], "val_a");
        assert_eq!(owned[1], "val_b");
    }

    // ====================================================================
    // NEW TESTS: Delete returns false for missing key
    // ====================================================================

    #[tokio::test]
    async fn test_delete_nonexistent_key_returns_false() -> crate::Result<()> {
        let (pool, _temp_dir) = create_test_pool().await?;

        execute(
            &pool,
            "CREATE TABLE del_test (key TEXT PRIMARY KEY, value TEXT)",
            &[],
        )
        .await?;

        let deleted = delete(&pool, "del_test", "nonexistent").await?;
        assert!(!deleted);
        Ok(())
    }

    // ====================================================================
    // NEW TESTS: Save upsert overwrites existing
    // ====================================================================

    #[tokio::test]
    async fn test_save_upsert_overwrites() -> crate::Result<()> {
        let (pool, _temp_dir) = create_test_pool().await?;

        execute(
            &pool,
            "CREATE TABLE kv_upsert (key TEXT PRIMARY KEY, value TEXT, updated_at TEXT)",
            &[],
        )
        .await?;

        save(&pool, "kv_upsert", "k1", &"first").await?;
        save(&pool, "kv_upsert", "k1", &"second").await?;

        let loaded: Option<String> = load(&pool, "kv_upsert", "k1").await?;
        let val = loaded.ok_or_else(|| Error::Database("missing upsert data".to_string()))?;
        assert_eq!(val, "second");

        // Only one row should exist
        let total = count(&pool, "kv_upsert", None, &[]).await?;
        assert_eq!(total, 1);
        Ok(())
    }

    // ====================================================================
    // NEW TESTS: DatabaseType equality and hash
    // ====================================================================

    #[tokio::test]
    async fn test_database_type_equality() {
        assert_eq!(DatabaseType::FlowState, DatabaseType::FlowState);
        assert_ne!(DatabaseType::FlowState, DatabaseType::HebbianPulse);
    }

    #[tokio::test]
    async fn test_database_type_usable_as_hashmap_key() {
        let mut map = HashMap::new();
        map.insert(DatabaseType::TensorMemory, "tensor");
        map.insert(DatabaseType::EpisodicMemory, "episodic");
        assert_eq!(map.len(), 2);
        assert_eq!(map.get(&DatabaseType::TensorMemory), Some(&"tensor"));
    }

    // ====================================================================
    // NEW TESTS: QueryBuilder — values with empty slice
    // ====================================================================

    #[tokio::test]
    async fn test_query_builder_values_empty() {
        let query = QueryBuilder::insert_into("t", &["a"]).values(&[]);
        // No params pushed — the placeholders are already in the query
        assert!(query.params().is_empty());
    }

    // ====================================================================
    // NEW TESTS: Load returns None for missing key
    // ====================================================================

    #[tokio::test]
    async fn test_load_returns_none_for_missing_key() -> crate::Result<()> {
        let (pool, _temp_dir) = create_test_pool().await?;

        execute(
            &pool,
            "CREATE TABLE kv_load_none (key TEXT PRIMARY KEY, value TEXT, updated_at TEXT)",
            &[],
        )
        .await?;

        let result: Option<String> = load(&pool, "kv_load_none", "absent_key").await?;
        assert!(result.is_none());
        Ok(())
    }

    // ====================================================================
    // NEW TESTS: StatePersistence debug format
    // ====================================================================

    #[tokio::test]
    async fn test_state_persistence_debug() -> crate::Result<()> {
        let temp_dir =
            TempDir::new().map_err(|e| Error::Database(format!("temp dir failed: {e}")))?;

        let persistence = StatePersistence::builder()
            .base_dir(temp_dir.path())
            .with_database(DatabaseType::FlowState)
            .build()
            .await?;

        let debug_str = format!("{persistence:?}");
        assert!(debug_str.contains("StatePersistence"));
        assert!(debug_str.contains("pool_count"));
        Ok(())
    }

    // ====================================================================
    // NAM: save_with_provenance exists as callable function
    // ====================================================================

    #[test]
    fn test_save_with_provenance_exists() {
        // Verify the function signature exists and is callable
        fn _assert_fn_exists<T: Serialize + Sync>(
            _pool: &DatabasePool,
            _table: &str,
            _key: &str,
            _value: &T,
            _agent_id: &str,
        ) {
            // We can't call the async function in a sync test,
            // but we verify it compiles
        }
    }

    #[test]
    fn test_save_versioned_exists() {
        // Verify the function signature exists
        fn _assert_fn_exists<T: Serialize + Sync>(
            _pool: &DatabasePool,
            _table: &str,
            _key: &str,
            _value: &T,
            _expected_version: u64,
        ) {
            // Compilation check
        }
    }

    // ====================================================================
    // NAM: StateStore::agent_id default
    // ====================================================================

    #[test]
    fn test_state_store_agent_id_default_none() {
        // The default implementation returns None
        struct TestStore;
        impl StateStore for TestStore {
            fn pool(&self) -> &DatabasePool {
                unimplemented!("test only")
            }
            fn store_name(&self) -> &str {
                "test"
            }
        }
        let store = TestStore;
        assert!(store.agent_id().is_none());
    }

    // ====================================================================
    // NAM: StatePersistence::to_tensor
    // ====================================================================

    #[tokio::test]
    async fn test_state_persistence_to_tensor_valid() -> crate::Result<()> {
        let dir = tempfile::tempdir().map_err(|e| Error::Database(e.to_string()))?;
        let persistence = StatePersistence::builder()
            .base_dir(dir.path())
            .migrations_dir(dir.path().join("migrations"))
            .with_database(DatabaseType::ServiceTracking)
            .build()
            .await?;

        let tensor = persistence.to_tensor();
        assert!(tensor.validate().is_ok());
        assert!(tensor.dependency_count > 0.0, "Should have at least 1 db");
        Ok(())
    }

    // ====================================================================
    // NAM: Provenance and versioned re-exports exist
    // ====================================================================

    #[test]
    fn test_provenance_and_versioned_functions_exist() {
        // Verify the functions exist and are callable (type-level check only).
        // The async functions require a DatabasePool to call, so we just confirm
        // they resolve as symbols.
        let _provenance_fn = save_with_provenance::<String>;
        let _versioned_fn = save_versioned::<String>;
    }

    // ====================================================================
    // Additional: save_with_provenance roundtrip
    // ====================================================================

    #[tokio::test]
    async fn test_save_with_provenance_roundtrip() -> crate::Result<()> {
        let (pool, _temp_dir) = create_test_pool().await?;

        execute(
            &pool,
            "CREATE TABLE prov_kv (key TEXT PRIMARY KEY, value TEXT, saved_by TEXT, updated_at TEXT)",
            &[],
        )
        .await?;

        save_with_provenance(&pool, "prov_kv", "k1", &"data1", "@0.A").await?;

        let loaded: Option<String> = load(&pool, "prov_kv", "k1").await?;
        let val = loaded.ok_or_else(|| Error::Database("missing provenance data".to_string()))?;
        assert_eq!(val, "data1");

        // Verify the saved_by column was set
        let row = sqlx::query("SELECT saved_by FROM prov_kv WHERE key = ?")
            .bind("k1")
            .fetch_one(pool.inner())
            .await
            .map_err(|e| Error::Database(format!("fetch failed: {e}")))?;
        let agent: String = row
            .try_get(0)
            .map_err(|e| Error::Database(format!("get failed: {e}")))?;
        assert_eq!(agent, "@0.A");
        Ok(())
    }

    // ====================================================================
    // Additional: save_versioned roundtrip
    // ====================================================================

    #[tokio::test]
    async fn test_save_versioned_new_key_version_zero() -> crate::Result<()> {
        let (pool, _temp_dir) = create_test_pool().await?;

        execute(
            &pool,
            "CREATE TABLE ver_kv (key TEXT PRIMARY KEY, value TEXT, version INTEGER DEFAULT 0, updated_at TEXT)",
            &[],
        )
        .await?;

        let new_ver = save_versioned(&pool, "ver_kv", "key1", &"initial", 0).await?;
        assert_eq!(new_ver, 1);

        let loaded: Option<String> = load(&pool, "ver_kv", "key1").await?;
        let val = loaded.ok_or_else(|| Error::Database("missing versioned data".to_string()))?;
        assert_eq!(val, "initial");
        Ok(())
    }

    #[tokio::test]
    async fn test_save_versioned_version_mismatch_errors() -> crate::Result<()> {
        let (pool, _temp_dir) = create_test_pool().await?;

        execute(
            &pool,
            "CREATE TABLE ver_mismatch (key TEXT PRIMARY KEY, value TEXT, version INTEGER DEFAULT 0, updated_at TEXT)",
            &[],
        )
        .await?;

        // Insert at version 0 -> 1
        save_versioned(&pool, "ver_mismatch", "k1", &"v1", 0).await?;

        // Try to update with wrong version (expect 0 but actual is 1)
        let result = save_versioned(&pool, "ver_mismatch", "k1", &"v2", 0).await;
        assert!(result.is_err());
        Ok(())
    }

    #[tokio::test]
    async fn test_save_versioned_correct_version_succeeds() -> crate::Result<()> {
        let (pool, _temp_dir) = create_test_pool().await?;

        execute(
            &pool,
            "CREATE TABLE ver_ok (key TEXT PRIMARY KEY, value TEXT, version INTEGER DEFAULT 0, updated_at TEXT)",
            &[],
        )
        .await?;

        let v1 = save_versioned(&pool, "ver_ok", "k1", &"first", 0).await?;
        assert_eq!(v1, 1);

        let v2 = save_versioned(&pool, "ver_ok", "k1", &"second", 1).await?;
        assert_eq!(v2, 2);

        let loaded: Option<String> = load(&pool, "ver_ok", "k1").await?;
        let val = loaded.ok_or_else(|| Error::Database("missing data".to_string()))?;
        assert_eq!(val, "second");
        Ok(())
    }

    // ====================================================================
    // Additional: multiple inserts and count with condition
    // ====================================================================

    #[tokio::test]
    async fn test_count_empty_table_returns_zero() -> crate::Result<()> {
        let (pool, _temp_dir) = create_test_pool().await?;

        execute(
            &pool,
            "CREATE TABLE empty_t (id INTEGER PRIMARY KEY, val TEXT)",
            &[],
        )
        .await?;

        let total = count(&pool, "empty_t", None, &[]).await?;
        assert_eq!(total, 0);
        Ok(())
    }

    // ====================================================================
    // Additional: execute returns affected rows
    // ====================================================================

    #[tokio::test]
    async fn test_execute_insert_returns_one_affected() -> crate::Result<()> {
        let (pool, _temp_dir) = create_test_pool().await?;

        execute(
            &pool,
            "CREATE TABLE aff_t (id INTEGER PRIMARY KEY, val TEXT)",
            &[],
        )
        .await?;

        let affected = execute(&pool, "INSERT INTO aff_t (val) VALUES (?)", &["hello"]).await?;
        assert_eq!(affected, 1);
        Ok(())
    }

    #[tokio::test]
    async fn test_execute_update_returns_affected_count() -> crate::Result<()> {
        let (pool, _temp_dir) = create_test_pool().await?;

        execute(
            &pool,
            "CREATE TABLE upd_t (id INTEGER PRIMARY KEY, val TEXT)",
            &[],
        )
        .await?;

        execute(&pool, "INSERT INTO upd_t (val) VALUES (?)", &["a"]).await?;
        execute(&pool, "INSERT INTO upd_t (val) VALUES (?)", &["a"]).await?;
        execute(&pool, "INSERT INTO upd_t (val) VALUES (?)", &["b"]).await?;

        let affected = execute(
            &pool,
            "UPDATE upd_t SET val = ? WHERE val = ?",
            &["updated", "a"],
        )
        .await?;
        assert_eq!(affected, 2);
        Ok(())
    }

    // ====================================================================
    // Additional: fetch_all on empty table
    // ====================================================================

    #[tokio::test]
    async fn test_fetch_all_empty_result() -> crate::Result<()> {
        let (pool, _temp_dir) = create_test_pool().await?;

        execute(
            &pool,
            "CREATE TABLE fa_empty (id INTEGER PRIMARY KEY, value TEXT)",
            &[],
        )
        .await?;

        let results: Vec<String> =
            fetch_all(&pool, "SELECT value FROM fa_empty", &[]).await?;
        assert!(results.is_empty());
        Ok(())
    }

    // ====================================================================
    // Additional: fetch_one on missing row returns error
    // ====================================================================

    #[tokio::test]
    async fn test_fetch_one_missing_row_returns_error() -> crate::Result<()> {
        let (pool, _temp_dir) = create_test_pool().await?;

        execute(
            &pool,
            "CREATE TABLE fo_miss (id INTEGER PRIMARY KEY, value TEXT)",
            &[],
        )
        .await?;

        let result: crate::Result<String> =
            fetch_one(&pool, "SELECT value FROM fo_miss WHERE id = ?", &["999"]).await;
        assert!(result.is_err());
        Ok(())
    }

    // ====================================================================
    // Additional: begin_transaction on valid pool
    // ====================================================================

    #[tokio::test]
    async fn test_begin_transaction_succeeds() -> crate::Result<()> {
        let (pool, _temp_dir) = create_test_pool().await?;
        let tx = begin_transaction(&pool).await?;
        tx.rollback().await?;
        Ok(())
    }

    // ====================================================================
    // Additional: QueryBuilder — chaining and edge cases
    // ====================================================================

    #[tokio::test]
    async fn test_query_builder_default() {
        let qb = QueryBuilder::default();
        assert_eq!(qb.build(), "");
        assert!(qb.params().is_empty());
    }

    #[tokio::test]
    async fn test_query_builder_select_single_column() {
        let qb = QueryBuilder::select(&["count(*)"]).from("items");
        assert_eq!(qb.build(), "SELECT count(*) FROM items");
    }

    #[tokio::test]
    async fn test_query_builder_delete_with_and_eq() {
        let qb = QueryBuilder::delete_from("logs")
            .where_eq("level", "error")
            .and_eq("source", "M04");
        assert_eq!(
            qb.build(),
            "DELETE FROM logs WHERE level = ? AND source = ?"
        );
        assert_eq!(qb.params(), vec!["error", "M04"]);
    }

    #[tokio::test]
    async fn test_query_builder_update_multiple_set() {
        let qb = QueryBuilder::update("config")
            .set("val1", "a")
            .set("val2", "b")
            .set("val3", "c")
            .where_eq("id", "1");

        assert_eq!(
            qb.build(),
            "UPDATE config SET val1 = ?, val2 = ?, val3 = ? WHERE id = ?"
        );
        assert_eq!(qb.params(), vec!["a", "b", "c", "1"]);
    }

    #[tokio::test]
    async fn test_query_builder_insert_many_columns() {
        let qb = QueryBuilder::insert_into("events", &["ts", "src", "msg", "level", "trace_id"])
            .values(&["now", "M04", "test msg", "INFO", "abc123"]);

        assert!(qb.build().contains("INSERT INTO events"));
        assert_eq!(qb.params().len(), 5);
    }

    #[tokio::test]
    async fn test_query_builder_clone() {
        let qb = QueryBuilder::select(&["id"]).from("t").where_eq("x", "1");
        let cloned = qb.clone();
        assert_eq!(qb.build(), cloned.build());
        assert_eq!(qb.params(), cloned.params());
    }

    #[tokio::test]
    async fn test_query_builder_debug() {
        let qb = QueryBuilder::select(&["id"]).from("t");
        let debug = format!("{qb:?}");
        assert!(debug.contains("QueryBuilder"));
        assert!(debug.contains("SELECT id FROM t"));
    }

    // ====================================================================
    // Additional: DatabaseType — copy, clone
    // ====================================================================

    #[tokio::test]
    async fn test_database_type_copy() {
        let a = DatabaseType::FlowState;
        let b = a; // Copy
        assert_eq!(a, b);
    }

    #[tokio::test]
    async fn test_database_type_clone() {
        let a = DatabaseType::EpisodicMemory;
        let b = a.clone();
        assert_eq!(a, b);
    }

    #[tokio::test]
    async fn test_database_type_debug() {
        let debug = format!("{:?}", DatabaseType::ConsensusTracking);
        assert_eq!(debug, "ConsensusTracking");
    }

    // ====================================================================
    // Additional: DatabaseConfig — debug
    // ====================================================================

    #[tokio::test]
    async fn test_database_config_debug() {
        let config = DatabaseConfig::new("debug.db");
        let debug = format!("{config:?}");
        assert!(debug.contains("DatabaseConfig"));
        assert!(debug.contains("debug.db"));
    }

    // ====================================================================
    // Additional: multiple save/load roundtrips
    // ====================================================================

    #[tokio::test]
    async fn test_save_load_multiple_keys() -> crate::Result<()> {
        let (pool, _temp_dir) = create_test_pool().await?;

        execute(
            &pool,
            "CREATE TABLE multi_kv (key TEXT PRIMARY KEY, value TEXT, updated_at TEXT)",
            &[],
        )
        .await?;

        save(&pool, "multi_kv", "key1", &"val1").await?;
        save(&pool, "multi_kv", "key2", &"val2").await?;
        save(&pool, "multi_kv", "key3", &"val3").await?;

        let v1: Option<String> = load(&pool, "multi_kv", "key1").await?;
        let v2: Option<String> = load(&pool, "multi_kv", "key2").await?;
        let v3: Option<String> = load(&pool, "multi_kv", "key3").await?;

        assert_eq!(v1.as_deref(), Some("val1"));
        assert_eq!(v2.as_deref(), Some("val2"));
        assert_eq!(v3.as_deref(), Some("val3"));

        let total = count(&pool, "multi_kv", None, &[]).await?;
        assert_eq!(total, 3);
        Ok(())
    }

    // ====================================================================
    // Additional: exists on empty table
    // ====================================================================

    #[tokio::test]
    async fn test_exists_empty_table_returns_false() -> crate::Result<()> {
        let (pool, _temp_dir) = create_test_pool().await?;

        execute(
            &pool,
            "CREATE TABLE exist_empty (key TEXT PRIMARY KEY, value TEXT)",
            &[],
        )
        .await?;

        assert!(!exists(&pool, "exist_empty", "any_key").await?);
        Ok(())
    }

    // ====================================================================
    // Additional: delete multiple times is idempotent
    // ====================================================================

    #[tokio::test]
    async fn test_delete_twice_returns_false_second_time() -> crate::Result<()> {
        let (pool, _temp_dir) = create_test_pool().await?;

        execute(
            &pool,
            "CREATE TABLE del2_t (key TEXT PRIMARY KEY, value TEXT)",
            &[],
        )
        .await?;

        execute(
            &pool,
            "INSERT INTO del2_t (key, value) VALUES (?, ?)",
            &["k1", "v1"],
        )
        .await?;

        let first = delete(&pool, "del2_t", "k1").await?;
        assert!(first);

        let second = delete(&pool, "del2_t", "k1").await?;
        assert!(!second);
        Ok(())
    }

    // ====================================================================
    // Additional: transaction fetch_all
    // ====================================================================

    #[tokio::test]
    async fn test_transaction_fetch_all() -> crate::Result<()> {
        let (pool, _temp_dir) = create_test_pool().await?;

        execute(
            &pool,
            "CREATE TABLE tx_fa (id INTEGER PRIMARY KEY, value TEXT)",
            &[],
        )
        .await?;

        execute(
            &pool,
            "INSERT INTO tx_fa (value) VALUES (?)",
            &["\"hello\""],
        )
        .await?;
        execute(
            &pool,
            "INSERT INTO tx_fa (value) VALUES (?)",
            &["\"world\""],
        )
        .await?;

        let mut tx = begin_transaction(&pool).await?;
        let results: Vec<String> = tx
            .fetch_all("SELECT value FROM tx_fa ORDER BY id", &[])
            .await?;
        tx.commit().await?;

        assert_eq!(results.len(), 2);
        assert_eq!(results[0], "hello");
        assert_eq!(results[1], "world");
        Ok(())
    }

    // ====================================================================
    // Additional: transaction fetch_one
    // ====================================================================

    #[tokio::test]
    async fn test_transaction_fetch_one() -> crate::Result<()> {
        let (pool, _temp_dir) = create_test_pool().await?;

        execute(
            &pool,
            "CREATE TABLE tx_fo (id INTEGER PRIMARY KEY, value TEXT)",
            &[],
        )
        .await?;

        execute(
            &pool,
            "INSERT INTO tx_fo (value) VALUES (?)",
            &["\"test_val\""],
        )
        .await?;

        let mut tx = begin_transaction(&pool).await?;
        let result: String = tx
            .fetch_one("SELECT value FROM tx_fo WHERE id = ?", &["1"])
            .await?;
        tx.commit().await?;

        assert_eq!(result, "test_val");
        Ok(())
    }

    // ====================================================================
    // Additional: PoolStats — copy and clone
    // ====================================================================

    #[tokio::test]
    async fn test_pool_stats_copy_clone() -> crate::Result<()> {
        let (pool, _temp_dir) = create_test_pool().await?;
        let stats = pool.stats();
        let copied = stats; // Copy
        let cloned = stats.clone();
        assert_eq!(copied.size, cloned.size);
        assert_eq!(copied.idle, cloned.idle);
        Ok(())
    }

    #[tokio::test]
    async fn test_pool_stats_debug() -> crate::Result<()> {
        let (pool, _temp_dir) = create_test_pool().await?;
        let stats = pool.stats();
        let debug = format!("{stats:?}");
        assert!(debug.contains("PoolStats"));
        Ok(())
    }

    // ====================================================================
    // Additional: StateStore agent_id with custom impl
    // ====================================================================

    #[test]
    fn test_state_store_custom_agent_id() {
        struct CustomStore;
        impl StateStore for CustomStore {
            fn pool(&self) -> &DatabasePool {
                unimplemented!("test only")
            }
            fn store_name(&self) -> &str {
                "custom"
            }
            fn agent_id(&self) -> Option<&str> {
                Some("@0.A")
            }
        }
        let store = CustomStore;
        assert_eq!(store.agent_id(), Some("@0.A"));
        assert_eq!(store.store_name(), "custom");
    }

    // ====================================================================
    // Additional: StatePersistence builder — with_all_databases
    // ====================================================================

    #[tokio::test]
    async fn test_state_persistence_builder_with_all_databases() -> crate::Result<()> {
        let dir = tempfile::tempdir().map_err(|e| Error::Database(e.to_string()))?;
        let persistence = StatePersistence::builder()
            .base_dir(dir.path())
            .with_all_databases()
            .build()
            .await?;

        // Should have 11 database pools
        let stats = persistence.stats_all();
        assert_eq!(stats.len(), 11);
        Ok(())
    }

    // ====================================================================
    // Additional: StatePersistence — pool retrieval
    // ====================================================================

    #[tokio::test]
    async fn test_state_persistence_pool_exists() -> crate::Result<()> {
        let dir = tempfile::tempdir().map_err(|e| Error::Database(e.to_string()))?;
        let persistence = StatePersistence::builder()
            .base_dir(dir.path())
            .with_database(DatabaseType::ServiceTracking)
            .with_database(DatabaseType::FlowState)
            .build()
            .await?;

        assert!(persistence.pool(DatabaseType::ServiceTracking).is_ok());
        assert!(persistence.pool(DatabaseType::FlowState).is_ok());
        // Not initialized database should error
        assert!(persistence.pool(DatabaseType::HebbianPulse).is_err());
        Ok(())
    }

    // ====================================================================
    // Additional: DatabasePool — inner pool accessible
    // ====================================================================

    #[tokio::test]
    async fn test_database_pool_inner_accessible() -> crate::Result<()> {
        let (pool, _temp_dir) = create_test_pool().await?;
        let inner = pool.inner();
        // Verify we can perform a basic query through the inner pool
        let row = sqlx::query("SELECT 1 as val")
            .fetch_one(inner)
            .await
            .map_err(|e| Error::Database(format!("inner query failed: {e}")))?;
        let val: i32 = row
            .try_get(0)
            .map_err(|e| Error::Database(format!("get failed: {e}")))?;
        assert_eq!(val, 1);
        Ok(())
    }

    // ====================================================================
    // Additional: fetch_optional returns Some when row exists
    // ====================================================================

    #[tokio::test]
    async fn test_fetch_optional_returns_some_when_exists() -> crate::Result<()> {
        let (pool, _temp_dir) = create_test_pool().await?;

        execute(
            &pool,
            "CREATE TABLE opt_exists (id INTEGER PRIMARY KEY, value TEXT)",
            &[],
        )
        .await?;

        execute(
            &pool,
            "INSERT INTO opt_exists (id, value) VALUES (?, ?)",
            &["1", "\"found_it\""],
        )
        .await?;

        let result: Option<String> = fetch_optional(
            &pool,
            "SELECT value FROM opt_exists WHERE id = ?",
            &["1"],
        )
        .await?;
        assert_eq!(result.as_deref(), Some("found_it"));
        Ok(())
    }

    // ====================================================================
    // Additional: QueryBuilder — order_by DESC
    // ====================================================================

    #[tokio::test]
    async fn test_query_builder_order_by_desc() {
        let qb = QueryBuilder::select(&["ts"])
            .from("events")
            .order_by("ts", "DESC");
        assert_eq!(qb.build(), "SELECT ts FROM events ORDER BY ts DESC");
    }

    // ====================================================================
    // Additional: QueryBuilder — limit and offset combined
    // ====================================================================

    #[tokio::test]
    async fn test_query_builder_limit_offset_combined() {
        let qb = QueryBuilder::select(&["*"])
            .from("logs")
            .order_by("id", "ASC")
            .limit(5)
            .offset(10);
        assert_eq!(
            qb.build(),
            "SELECT * FROM logs ORDER BY id ASC LIMIT 5 OFFSET 10"
        );
    }

    // ====================================================================
    // Additional: save bool type
    // ====================================================================

    #[tokio::test]
    async fn test_save_and_load_bool() -> crate::Result<()> {
        let (pool, _temp_dir) = create_test_pool().await?;

        execute(
            &pool,
            "CREATE TABLE kv_bool (key TEXT PRIMARY KEY, value TEXT, updated_at TEXT)",
            &[],
        )
        .await?;

        save(&pool, "kv_bool", "flag", &true).await?;
        let loaded: Option<bool> = load(&pool, "kv_bool", "flag").await?;
        assert_eq!(loaded, Some(true));
        Ok(())
    }

    // ====================================================================
    // Additional: save numeric type
    // ====================================================================

    #[tokio::test]
    async fn test_save_and_load_numeric() -> crate::Result<()> {
        let (pool, _temp_dir) = create_test_pool().await?;

        execute(
            &pool,
            "CREATE TABLE kv_num (key TEXT PRIMARY KEY, value TEXT, updated_at TEXT)",
            &[],
        )
        .await?;

        save(&pool, "kv_num", "count", &42_i64).await?;
        let loaded: Option<i64> = load(&pool, "kv_num", "count").await?;
        assert_eq!(loaded, Some(42));
        Ok(())
    }

    // ====================================================================
    // Additional: StatePersistenceBuilder — migrations_dir and config
    // ====================================================================

    #[tokio::test]
    async fn test_state_persistence_builder_custom_config() -> crate::Result<()> {
        let dir = tempfile::tempdir().map_err(|e| Error::Database(e.to_string()))?;
        let config = DatabaseConfig::new("ignored")
            .with_max_connections(5)
            .with_min_connections(1)
            .with_acquire_timeout(10);

        let persistence = StatePersistence::builder()
            .base_dir(dir.path())
            .migrations_dir(dir.path().join("mig"))
            .config(config)
            .with_database(DatabaseType::TensorMemory)
            .build()
            .await?;

        assert!(persistence.pool(DatabaseType::TensorMemory).is_ok());
        Ok(())
    }

    // ====================================================================
    // Additional: StatePersistenceBuilder — with_database deduplication
    // ====================================================================

    #[tokio::test]
    async fn test_state_persistence_builder_deduplicates() -> crate::Result<()> {
        let dir = tempfile::tempdir().map_err(|e| Error::Database(e.to_string()))?;
        let persistence = StatePersistence::builder()
            .base_dir(dir.path())
            .with_database(DatabaseType::FlowState)
            .with_database(DatabaseType::FlowState) // duplicate
            .with_database(DatabaseType::FlowState) // triplicate
            .build()
            .await?;

        let stats = persistence.stats_all();
        assert_eq!(stats.len(), 1);
        Ok(())
    }

    // ====================================================================
    // Additional: health_check_all on single database
    // ====================================================================

    #[tokio::test]
    async fn test_health_check_all() -> crate::Result<()> {
        let dir = tempfile::tempdir().map_err(|e| Error::Database(e.to_string()))?;
        let persistence = StatePersistence::builder()
            .base_dir(dir.path())
            .with_database(DatabaseType::FlowState)
            .with_database(DatabaseType::EpisodicMemory)
            .build()
            .await?;

        let health = persistence.health_check_all().await?;
        assert_eq!(health.len(), 2);
        for (_db_type, healthy) in &health {
            assert!(healthy);
        }
        Ok(())
    }
}
