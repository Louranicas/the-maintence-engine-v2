#![allow(clippy::missing_errors_doc)]
#![allow(clippy::module_name_repetitions)]

//! # Database Manager
//!
//! High-level database abstraction wrapping [`StatePersistence`] from
//! [`m1_foundation::state`](crate::m1_foundation::state), providing typed
//! write/read operations for the L7 Observer data.
//!
//! ## Supported Tables
//!
//! | Database | Tables |
//! |----------|--------|
//! | `EvolutionTracking` | `fitness_history`, `emergence_log`, `mutation_log`, `correlation_log` |
//! | `TensorMemory` | `tensor_snapshots` |
//! | `ServiceTracking` | `service_events` |
//! | `PerformanceMetrics` | `performance_samples` |
//!
//! ## Related Documentation
//! - [Database Guide](../ai_docs/DATABASE_GUIDE.md)
//! - [M05 State Persistence](crate::m1_foundation::state)

use crate::m1_foundation::state::{execute, DatabaseType, StatePersistence};
use crate::{Error, Result};
use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use sqlx::Row;
use std::path::Path;

// ============================================================================
// Health Cache
// ============================================================================

/// Cached result of the most recent database health check.
struct DatabaseHealthCache {
    last_check: Option<DateTime<Utc>>,
    all_healthy: bool,
    db_count: usize,
}

impl DatabaseHealthCache {
    /// Create a new empty health cache.
    const fn empty() -> Self {
        Self {
            last_check: None,
            all_healthy: false,
            db_count: 0,
        }
    }
}

// ============================================================================
// Data Types
// ============================================================================

/// A single fitness evaluation record from the evolution chamber.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct FitnessHistoryEntry {
    /// ISO-8601 timestamp of the evaluation.
    pub timestamp: String,
    /// Computed fitness score.
    pub fitness: f64,
    /// Serialized system state label (e.g. "healthy", "degraded").
    pub system_state: String,
    /// SHA-256 hash of the tensor snapshot used for evaluation.
    pub tensor_hash: String,
    /// Generation number in the evolutionary loop.
    pub generation: u64,
}

/// A point-in-time capture of the 12D tensor.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TensorSnapshot {
    /// ISO-8601 timestamp.
    pub timestamp: String,
    /// The 12 dimension values.
    pub dimensions: [f64; 12],
    /// Origin module that produced the snapshot.
    pub source: String,
    /// Observer tick counter at capture time.
    pub tick: u64,
}

/// An emergence event detected by the L7 emergence detector.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct EmergenceEntry {
    /// Unique identifier for this emergence event.
    pub id: String,
    /// Classification (cascade, synergy, resonance, phase).
    pub emergence_type: String,
    /// Detection confidence in [0, 1].
    pub confidence: f64,
    /// Severity level in [0, 1].
    pub severity: f64,
    /// ISO-8601 timestamp of detection.
    pub detected_at: String,
    /// Human-readable description of the emergence.
    pub description: String,
}

/// A recorded mutation from the RALPH evolution loop.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MutationEntry {
    /// Unique identifier for this mutation.
    pub id: String,
    /// Generation in which the mutation was created.
    pub generation: u64,
    /// The parameter that was mutated.
    pub target_parameter: String,
    /// Original value before mutation.
    pub original_value: f64,
    /// Value after mutation.
    pub mutated_value: f64,
    /// Whether the mutation was applied to the live system.
    pub applied: bool,
    /// Whether the mutation was subsequently rolled back.
    pub rolled_back: bool,
    /// ISO-8601 timestamp.
    pub timestamp: String,
}

/// A log-correlator link between events.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CorrelationEntry {
    /// Unique correlation identifier.
    pub id: String,
    /// Correlation channel name.
    pub channel: String,
    /// Type of the correlated event.
    pub event_type: String,
    /// Number of linked events in this correlation.
    pub link_count: u32,
    /// ISO-8601 timestamp.
    pub timestamp: String,
}

/// A service health observation from L2 health monitoring.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ServiceEventEntry {
    /// Service identifier.
    pub service_id: String,
    /// Type of event (`health_check`, `restart`, `circuit_open`, ...).
    pub event_type: String,
    /// Health score at time of event.
    pub health_score: f64,
    /// Round-trip latency in milliseconds.
    pub latency_ms: f64,
    /// ISO-8601 timestamp.
    pub timestamp: String,
}

/// A single performance measurement.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PerformanceSample {
    /// Name of the metric (e.g. `"pipeline_latency"`).
    pub metric_name: String,
    /// Measured value.
    pub value: f64,
    /// Unit of measurement (e.g. "ms", "bytes", "ratio").
    pub unit: String,
    /// ISO-8601 timestamp.
    pub timestamp: String,
}

/// Persisted cognitive state for temporal continuity across restarts (NAM-T2).
///
/// Serialised as a single-row JSON blob in `evolution_tracking.db` so that
/// the observer layer can resume from its last known state instead of starting
/// cold every time the binary restarts.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CognitiveState {
    /// M37 correlation window size in milliseconds.
    pub window_size_ms: u64,
    /// M39 RALPH generation counter.
    pub generation: u64,
    /// Monotonic observer tick counter.
    pub tick_count: u64,
    /// Consecutive ticks with zero correlations (dormancy tracking).
    pub zero_correlation_streak: u64,
    /// Current fitness value.
    pub fitness: f64,
    /// ISO-8601 timestamp of last save.
    pub saved_at: String,
}

/// Summary report from a database health check.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DatabaseHealthReport {
    /// Total number of databases checked.
    pub total_databases: usize,
    /// Number of healthy databases.
    pub healthy_databases: usize,
    /// Whether every database is healthy.
    pub all_healthy: bool,
    /// ISO-8601 timestamp of the check.
    pub checked_at: String,
}

// ============================================================================
// Database Manager
// ============================================================================

/// High-level database manager wrapping [`StatePersistence`] with typed
/// write/read operations for L7 Observer data.
pub struct DatabaseManager {
    persistence: StatePersistence,
    health_cache: RwLock<DatabaseHealthCache>,
}

impl DatabaseManager {
    /// Try to create a `DatabaseManager`, returning `None` on failure
    /// instead of propagating the error. Suitable for fail-silent
    /// initialization in production (e.g. `AppState.db`).
    pub async fn new_optional(base_dir: &Path) -> Option<Self> {
        match Self::new(base_dir).await {
            Ok(mgr) => Some(mgr),
            Err(e) => {
                tracing::warn!(error = %e, "DatabaseManager initialization failed (non-fatal)");
                None
            }
        }
    }

    /// Create a new `DatabaseManager`, initializing all database pools and
    /// creating tables if they do not already exist.
    pub async fn new(base_dir: &Path) -> Result<Self> {
        let persistence = StatePersistence::builder()
            .base_dir(base_dir)
            .migrations_dir(base_dir.join("migrations"))
            .with_all_databases()
            .build()
            .await?;

        let manager = Self {
            persistence,
            health_cache: RwLock::new(DatabaseHealthCache::empty()),
        };

        manager.create_tables().await?;

        Ok(manager)
    }

    /// Create a `DatabaseManager` with only a specific subset of databases
    /// (useful for testing).
    pub async fn with_databases(base_dir: &Path, db_types: &[DatabaseType]) -> Result<Self> {
        let mut builder = StatePersistence::builder()
            .base_dir(base_dir)
            .migrations_dir(base_dir.join("migrations"));

        for &db_type in db_types {
            builder = builder.with_database(db_type);
        }

        let persistence = builder.build().await?;

        let manager = Self {
            persistence,
            health_cache: RwLock::new(DatabaseHealthCache::empty()),
        };

        manager.create_tables_for(db_types).await?;

        Ok(manager)
    }

    // ========================================================================
    // Write Operations
    // ========================================================================

    /// Write a fitness history entry to the `EvolutionTracking` database.
    pub async fn write_fitness_history(&self, entry: &FitnessHistoryEntry) -> Result<u64> {
        let pool = self.persistence.pool(DatabaseType::EvolutionTracking)?;
        let json = serde_json::to_string(entry)
            .map_err(|e| Error::Database(format!("Failed to serialize fitness history: {e}")))?;
        execute(
            pool,
            "INSERT INTO fitness_history (data, created_at) VALUES (?, datetime('now'))",
            &[&json],
        )
        .await
    }

    /// Write a tensor snapshot to the `TensorMemory` database.
    ///
    /// Maps [`TensorSnapshot::dimensions`] array to individual `d0`–`d11` columns
    /// in the structured `tensor_snapshots` schema.  Generates a unique
    /// `snapshot_id` from the source name and tick counter.
    pub async fn write_tensor_snapshot(&self, snapshot: &TensorSnapshot) -> Result<u64> {
        let pool = self.persistence.pool(DatabaseType::TensorMemory)?;
        let d = &snapshot.dimensions;
        let snapshot_id = format!("{}-{}", snapshot.source, snapshot.tick);
        // Integer columns: scale normalised [0,1] tensor back to schema ranges.
        // d0: service index (0-12), d1: port (0-65535), d2: tier (1-5),
        // d3: dep count (0-12), d4: agent count (0-40), d5: protocol (0-3),
        // d7: uptime seconds (0+).
        let to_int = |v: f64, scale: f64, min: f64| -> String {
            // Round to nearest integer, clamp to [min, scale]. No f64→i32 cast
            // needed — format with zero decimal places and SQLite coerces to INTEGER.
            format!("{:.0}", v.mul_add(scale, 0.0).round().clamp(min, scale))
        };
        let d0 = to_int(d[0], 12.0, 0.0);
        let d1 = to_int(d[1], 65535.0, 0.0);
        let d2 = to_int(d[2], 5.0, 1.0);  // CHECK: BETWEEN 1 AND 5
        let d3 = to_int(d[3], 12.0, 0.0);
        let d4 = to_int(d[4], 40.0, 0.0);
        let d5 = to_int(d[5], 3.0, 0.0);  // CHECK: BETWEEN 0 AND 3
        let d7 = to_int(d[7], 86400.0, 0.0);
        // REAL columns: clamp to [0.0, 1.0] for CHECK constraints.
        let to_real = |v: f64| -> String { format!("{}", v.clamp(0.0, 1.0)) };
        let d6 = to_real(d[6]);
        let d8 = to_real(d[8]);
        let d9 = to_real(d[9]);
        let d10 = to_real(d[10]);
        let d11 = to_real(d[11]);
        execute(
            pool,
            "INSERT INTO tensor_snapshots \
             (snapshot_id, service_id, timestamp, \
              d0_service_id, d1_port, d2_tier, d3_deps, d4_agents, d5_protocol, \
              d6_health, d7_uptime, d8_synergy, d9_latency, d10_error_rate, d11_temporal) \
             VALUES (?, ?, datetime('now'), ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            &[
                snapshot_id.as_str(),
                snapshot.source.as_str(),
                d0.as_str(), d1.as_str(), d2.as_str(), d3.as_str(),
                d4.as_str(), d5.as_str(), d6.as_str(), d7.as_str(),
                d8.as_str(), d9.as_str(), d10.as_str(), d11.as_str(),
            ],
        )
        .await
    }

    /// Write an emergence event to the `EvolutionTracking` database.
    pub async fn write_emergence(&self, entry: &EmergenceEntry) -> Result<u64> {
        let pool = self.persistence.pool(DatabaseType::EvolutionTracking)?;
        let json = serde_json::to_string(entry)
            .map_err(|e| Error::Database(format!("Failed to serialize emergence entry: {e}")))?;
        execute(
            pool,
            "INSERT INTO emergence_log (data, created_at) VALUES (?, datetime('now'))",
            &[&json],
        )
        .await
    }

    /// Write a mutation record to the `EvolutionTracking` database.
    pub async fn write_mutation(&self, entry: &MutationEntry) -> Result<u64> {
        let pool = self.persistence.pool(DatabaseType::EvolutionTracking)?;
        let json = serde_json::to_string(entry)
            .map_err(|e| Error::Database(format!("Failed to serialize mutation entry: {e}")))?;
        execute(
            pool,
            "INSERT INTO mutation_log (data, created_at) VALUES (?, datetime('now'))",
            &[&json],
        )
        .await
    }

    /// Write a correlation record to the `EvolutionTracking` database.
    pub async fn write_correlation(&self, entry: &CorrelationEntry) -> Result<u64> {
        let pool = self.persistence.pool(DatabaseType::EvolutionTracking)?;
        let json = serde_json::to_string(entry)
            .map_err(|e| Error::Database(format!("Failed to serialize correlation entry: {e}")))?;
        execute(
            pool,
            "INSERT INTO correlation_log (data, created_at) VALUES (?, datetime('now'))",
            &[&json],
        )
        .await
    }

    /// Write a service event to the `ServiceTracking` database.
    ///
    /// Maps [`ServiceEventEntry`] fields to the structured `service_events` schema:
    /// `service_id` is resolved via sub-query on the `services` table by name,
    /// remaining fields map to `event_type`, `event_data` (JSON), and `severity`.
    pub async fn write_service_event(&self, entry: &ServiceEventEntry) -> Result<u64> {
        let pool = self.persistence.pool(DatabaseType::ServiceTracking)?;
        let event_data = format!(
            r#"{{"health_score":{},"latency_ms":{}}}"#,
            entry.health_score, entry.latency_ms,
        );
        let severity = if entry.health_score >= 0.8 {
            "info"
        } else if entry.health_score >= 0.5 {
            "warning"
        } else {
            "error"
        };
        execute(
            pool,
            "INSERT INTO service_events \
             (service_id, event_type, event_data, severity, timestamp) \
             VALUES (\
               COALESCE(\
                 (SELECT id FROM services WHERE name = ?),\
                 (SELECT id FROM services WHERE name LIKE ? || '%' LIMIT 1)\
               ), \
               ?, ?, ?, datetime('now'))",
            &[
                entry.service_id.as_str(),
                entry.service_id.as_str(),
                entry.event_type.as_str(),
                event_data.as_str(),
                severity,
            ],
        )
        .await
    }

    /// Write a performance sample to the `PerformanceMetrics` database.
    ///
    /// Maps the generic [`PerformanceSample`] to the structured
    /// `performance_samples` schema.  The `metric_name` is stored as `service_id`
    /// for indexing, while the `value` populates `avg_response_ms`.  Required NOT
    /// NULL columns receive safe defaults.
    pub async fn write_performance_sample(&self, sample: &PerformanceSample) -> Result<u64> {
        let pool = self.persistence.pool(DatabaseType::PerformanceMetrics)?;
        let sample_id = format!("{}-{}", sample.metric_name, sample.timestamp);
        let value_str = format!("{}", sample.value);
        execute(
            pool,
            "INSERT INTO performance_samples \
             (sample_id, service_id, timestamp, cpu_percent, memory_mb, avg_response_ms) \
             VALUES (?, ?, datetime('now'), 0.0, 0.0, ?)",
            &[sample_id.as_str(), sample.metric_name.as_str(), value_str.as_str()],
        )
        .await
    }

    /// Upsert the cognitive state snapshot (NAM-T2: temporal continuity).
    ///
    /// Uses `INSERT OR REPLACE` on a single-row table so only the latest
    /// state is stored.
    pub async fn write_cognitive_state(&self, state: &CognitiveState) -> Result<u64> {
        let pool = self.persistence.pool(DatabaseType::EvolutionTracking)?;
        let json = serde_json::to_string(state).map_err(|e| {
            Error::Database(format!("Failed to serialize cognitive state: {e}"))
        })?;
        execute(
            pool,
            "INSERT OR REPLACE INTO cognitive_state (id, data, updated_at) VALUES (1, ?, datetime('now'))",
            &[&json],
        )
        .await
    }

    /// Load the last persisted cognitive state, or `None` if no state exists.
    pub async fn read_cognitive_state(&self) -> Result<Option<CognitiveState>> {
        let pool = self.persistence.pool(DatabaseType::EvolutionTracking)?;
        let rows = sqlx::query("SELECT data FROM cognitive_state WHERE id = 1")
            .fetch_all(pool.inner())
            .await
            .map_err(|e| {
                Error::Database(format!("Failed to read cognitive state: {e}"))
            })?;
        if rows.is_empty() {
            return Ok(None);
        }
        let json: String = rows[0].try_get("data").map_err(|e| {
            Error::Database(format!("Failed to get cognitive state data column: {e}"))
        })?;
        let state: CognitiveState = serde_json::from_str(&json).map_err(|e| {
            Error::Database(format!("Failed to deserialize cognitive state: {e}"))
        })?;
        Ok(Some(state))
    }

    // ========================================================================
    // Read Operations
    // ========================================================================

    /// Load the most recent fitness history entries, ordered newest-first.
    pub async fn load_fitness_history(&self, limit: u32) -> Result<Vec<FitnessHistoryEntry>> {
        let pool = self.persistence.pool(DatabaseType::EvolutionTracking)?;
        let rows = sqlx::query(
            "SELECT data FROM fitness_history ORDER BY id DESC LIMIT ?",
        )
        .bind(i64::from(limit))
        .fetch_all(pool.inner())
        .await
        .map_err(|e| Error::Database(format!("Failed to load fitness history: {e}")))?;

        deserialize_rows(&rows)
    }

    /// Load the most recent mutation entries, ordered newest-first.
    pub async fn load_recent_mutations(&self, limit: u32) -> Result<Vec<MutationEntry>> {
        let pool = self.persistence.pool(DatabaseType::EvolutionTracking)?;
        let rows =
            sqlx::query("SELECT data FROM mutation_log ORDER BY created_at DESC LIMIT ?")
                .bind(i64::from(limit))
                .fetch_all(pool.inner())
                .await
                .map_err(|e| {
                    Error::Database(format!("Failed to load recent mutations: {e}"))
                })?;

        deserialize_rows(&rows)
    }

    /// Load the single most recent fitness history entry, or `None` if the
    /// table is empty.
    pub async fn read_latest_fitness(&self) -> Result<Option<FitnessHistoryEntry>> {
        let entries = self.load_fitness_history(1).await?;
        Ok(entries.into_iter().next())
    }

    /// Load service events that were created after `since` (ISO-8601 string).
    pub async fn read_service_events_since(&self, since: &str) -> Result<Vec<ServiceEventEntry>> {
        let pool = self.persistence.pool(DatabaseType::ServiceTracking)?;
        let rows = sqlx::query(
            "SELECT s.name, e.event_type, \
                    COALESCE(json_extract(e.event_data, '$.health_score'), 0.0), \
                    COALESCE(json_extract(e.event_data, '$.latency_ms'), 0.0), \
                    e.timestamp \
             FROM service_events e \
             JOIN services s ON s.id = e.service_id \
             WHERE e.timestamp > ? ORDER BY e.timestamp DESC",
        )
        .bind(since)
        .fetch_all(pool.inner())
        .await
        .map_err(|e| Error::Database(format!("Failed to load service events since {since}: {e}")))?;

        rows_to_service_events(&rows)
    }

    /// Load all service health events (most recent per service).
    pub async fn load_service_health(&self) -> Result<Vec<ServiceEventEntry>> {
        let pool = self.persistence.pool(DatabaseType::ServiceTracking)?;
        let rows = sqlx::query(
            "SELECT s.name, e.event_type, \
                    COALESCE(json_extract(e.event_data, '$.health_score'), 0.0), \
                    COALESCE(json_extract(e.event_data, '$.latency_ms'), 0.0), \
                    e.timestamp \
             FROM service_events e \
             JOIN services s ON s.id = e.service_id \
             ORDER BY e.timestamp DESC",
        )
        .fetch_all(pool.inner())
        .await
        .map_err(|e| Error::Database(format!("Failed to load service health: {e}")))?;

        rows_to_service_events(&rows)
    }

    // ========================================================================
    // Health
    // ========================================================================

    /// Run a health check against every configured database pool.
    pub async fn health_check_all(&self) -> Result<DatabaseHealthReport> {
        let health_map = self.persistence.health_check_all().await?;

        let total = health_map.len();
        let healthy = health_map.values().filter(|&&v| v).count();
        let all_healthy = healthy == total;
        let now = Utc::now();
        let checked_at = now.to_rfc3339();

        // Update cache with scoped lock
        {
            let mut cache = self.health_cache.write();
            cache.last_check = Some(now);
            cache.all_healthy = all_healthy;
            cache.db_count = total;
        }

        Ok(DatabaseHealthReport {
            total_databases: total,
            healthy_databases: healthy,
            all_healthy,
            checked_at,
        })
    }

    // ========================================================================
    // Accessors
    // ========================================================================

    /// Get a reference to the underlying [`StatePersistence`].
    #[must_use]
    pub const fn persistence(&self) -> &StatePersistence {
        &self.persistence
    }

    // ========================================================================
    // Internal Helpers
    // ========================================================================

    /// Create all required tables across the relevant databases.
    async fn create_tables(&self) -> Result<()> {
        self.create_tables_for(&DatabaseType::all()).await
    }

    /// Create tables only for the given database types.
    async fn create_tables_for(&self, db_types: &[DatabaseType]) -> Result<()> {
        for &db_type in db_types {
            match db_type {
                DatabaseType::EvolutionTracking => {
                    self.create_evolution_tables().await?;
                }
                DatabaseType::TensorMemory => {
                    self.create_tensor_tables().await?;
                }
                DatabaseType::ServiceTracking => {
                    self.create_service_tables().await?;
                }
                DatabaseType::PerformanceMetrics => {
                    self.create_performance_tables().await?;
                }
                // Other database types do not have tables managed by this module.
                _ => {}
            }
        }
        Ok(())
    }

    async fn create_evolution_tables(&self) -> Result<()> {
        let pool = self.persistence.pool(DatabaseType::EvolutionTracking)?;

        execute(
            pool,
            "CREATE TABLE IF NOT EXISTS fitness_history (\
                id INTEGER PRIMARY KEY AUTOINCREMENT, \
                data TEXT NOT NULL, \
                created_at TEXT NOT NULL DEFAULT (datetime('now'))\
            )",
            &[],
        )
        .await?;

        execute(
            pool,
            "CREATE TABLE IF NOT EXISTS emergence_log (\
                id INTEGER PRIMARY KEY AUTOINCREMENT, \
                data TEXT NOT NULL, \
                created_at TEXT NOT NULL DEFAULT (datetime('now'))\
            )",
            &[],
        )
        .await?;

        execute(
            pool,
            "CREATE TABLE IF NOT EXISTS mutation_log (\
                id INTEGER PRIMARY KEY AUTOINCREMENT, \
                data TEXT NOT NULL, \
                created_at TEXT NOT NULL DEFAULT (datetime('now'))\
            )",
            &[],
        )
        .await?;

        execute(
            pool,
            "CREATE TABLE IF NOT EXISTS correlation_log (\
                id INTEGER PRIMARY KEY AUTOINCREMENT, \
                data TEXT NOT NULL, \
                created_at TEXT NOT NULL DEFAULT (datetime('now'))\
            )",
            &[],
        )
        .await?;

        // NAM-T2: Single-row cognitive state for temporal continuity.
        execute(
            pool,
            "CREATE TABLE IF NOT EXISTS cognitive_state (\
                id INTEGER PRIMARY KEY CHECK (id = 1), \
                data TEXT NOT NULL, \
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))\
            )",
            &[],
        )
        .await?;

        Ok(())
    }

    async fn create_tensor_tables(&self) -> Result<()> {
        let pool = self.persistence.pool(DatabaseType::TensorMemory)?;

        execute(
            pool,
            "CREATE TABLE IF NOT EXISTS tensor_snapshots (\
                id INTEGER PRIMARY KEY AUTOINCREMENT, \
                snapshot_id TEXT NOT NULL UNIQUE, \
                service_id TEXT NOT NULL, \
                timestamp TEXT NOT NULL DEFAULT (datetime('now')), \
                d0_service_id INTEGER NOT NULL DEFAULT 0, \
                d1_port INTEGER NOT NULL DEFAULT 0, \
                d2_tier INTEGER NOT NULL DEFAULT 1, \
                d3_deps INTEGER NOT NULL DEFAULT 0, \
                d4_agents INTEGER NOT NULL DEFAULT 0, \
                d5_protocol INTEGER NOT NULL DEFAULT 0, \
                d6_health REAL NOT NULL DEFAULT 0.0, \
                d7_uptime INTEGER NOT NULL DEFAULT 0, \
                d8_synergy REAL NOT NULL DEFAULT 0.0, \
                d9_latency REAL NOT NULL DEFAULT 0.0, \
                d10_error_rate REAL NOT NULL DEFAULT 0.0, \
                d11_temporal REAL NOT NULL DEFAULT 0.0\
            )",
            &[],
        )
        .await?;

        Ok(())
    }

    async fn create_service_tables(&self) -> Result<()> {
        let pool = self.persistence.pool(DatabaseType::ServiceTracking)?;

        // Parent table referenced by service_events FK.
        execute(
            pool,
            "CREATE TABLE IF NOT EXISTS services (\
                id INTEGER PRIMARY KEY AUTOINCREMENT, \
                name TEXT NOT NULL UNIQUE, \
                status TEXT NOT NULL DEFAULT 'unknown'\
            )",
            &[],
        )
        .await?;

        execute(
            pool,
            "CREATE TABLE IF NOT EXISTS service_events (\
                id INTEGER PRIMARY KEY AUTOINCREMENT, \
                service_id INTEGER, \
                event_type TEXT NOT NULL DEFAULT 'health_check', \
                event_data TEXT, \
                severity TEXT NOT NULL DEFAULT 'info', \
                timestamp TEXT NOT NULL DEFAULT (datetime('now')), \
                FOREIGN KEY (service_id) REFERENCES services(id) ON DELETE CASCADE\
            )",
            &[],
        )
        .await?;

        Ok(())
    }

    async fn create_performance_tables(&self) -> Result<()> {
        let pool = self.persistence.pool(DatabaseType::PerformanceMetrics)?;

        execute(
            pool,
            "CREATE TABLE IF NOT EXISTS performance_samples (\
                id INTEGER PRIMARY KEY AUTOINCREMENT, \
                sample_id TEXT NOT NULL UNIQUE, \
                service_id TEXT NOT NULL, \
                timestamp TEXT NOT NULL DEFAULT (datetime('now')), \
                cpu_percent REAL NOT NULL DEFAULT 0.0, \
                memory_mb REAL NOT NULL DEFAULT 0.0, \
                avg_response_ms REAL DEFAULT 0.0\
            )",
            &[],
        )
        .await?;

        Ok(())
    }
}

// ============================================================================
// Utility Functions
// ============================================================================

/// Deserialize a set of sqlx rows where column 0 is a JSON `TEXT` string.
fn deserialize_rows<T: serde::de::DeserializeOwned>(
    rows: &[sqlx::sqlite::SqliteRow],
) -> Result<Vec<T>> {
    let mut results = Vec::with_capacity(rows.len());
    for row in rows {
        let json_str: String = row
            .try_get(0)
            .map_err(|e| Error::Database(format!("Failed to get data column: {e}")))?;
        let item: T = serde_json::from_str(&json_str)
            .map_err(|e| Error::Database(format!("Failed to deserialize row: {e}")))?;
        results.push(item);
    }
    Ok(results)
}

/// Convert structured `service_events` rows (joined with `services`) to
/// [`ServiceEventEntry`] values.  Expected column order:
/// 0=name, 1=`event_type`, 2=`health_score`, 3=`latency_ms`, 4=timestamp.
fn rows_to_service_events(rows: &[sqlx::sqlite::SqliteRow]) -> Result<Vec<ServiceEventEntry>> {
    let mut results = Vec::with_capacity(rows.len());
    for row in rows {
        let service_id: String = row
            .try_get(0)
            .map_err(|e| Error::Database(format!("Failed to get service name: {e}")))?;
        let event_type: String = row
            .try_get(1)
            .map_err(|e| Error::Database(format!("Failed to get event_type: {e}")))?;
        let health_score: f64 = row
            .try_get(2)
            .map_err(|e| Error::Database(format!("Failed to get health_score: {e}")))?;
        let latency_ms: f64 = row
            .try_get(3)
            .map_err(|e| Error::Database(format!("Failed to get latency_ms: {e}")))?;
        let timestamp: String = row
            .try_get(4)
            .map_err(|e| Error::Database(format!("Failed to get timestamp: {e}")))?;
        results.push(ServiceEventEntry {
            service_id,
            event_type,
            health_score,
            latency_ms,
            timestamp,
        });
    }
    Ok(results)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // ====================================================================
    // Helpers
    // ====================================================================

    fn make_fitness_entry() -> FitnessHistoryEntry {
        FitnessHistoryEntry {
            timestamp: "2026-01-29T12:00:00Z".to_string(),
            fitness: 0.95,
            system_state: "healthy".to_string(),
            tensor_hash: "abc123".to_string(),
            generation: 1,
        }
    }

    fn make_tensor_snapshot() -> TensorSnapshot {
        TensorSnapshot {
            timestamp: "2026-01-29T12:00:00Z".to_string(),
            dimensions: [0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 0.05, 0.01, 0.99],
            source: "fitness_evaluator".to_string(),
            tick: 42,
        }
    }

    fn make_emergence_entry() -> EmergenceEntry {
        EmergenceEntry {
            id: "em-001".to_string(),
            emergence_type: "cascade".to_string(),
            confidence: 0.87,
            severity: 0.4,
            detected_at: "2026-01-29T12:00:00Z".to_string(),
            description: "Cascade detected in service mesh".to_string(),
        }
    }

    fn make_mutation_entry() -> MutationEntry {
        MutationEntry {
            id: "mut-001".to_string(),
            generation: 5,
            target_parameter: "ltp_rate".to_string(),
            original_value: 0.1,
            mutated_value: 0.12,
            applied: true,
            rolled_back: false,
            timestamp: "2026-01-29T12:00:00Z".to_string(),
        }
    }

    fn make_correlation_entry() -> CorrelationEntry {
        CorrelationEntry {
            id: "cor-001".to_string(),
            channel: "health".to_string(),
            event_type: "degradation".to_string(),
            link_count: 3,
            timestamp: "2026-01-29T12:00:00Z".to_string(),
        }
    }

    fn make_service_event() -> ServiceEventEntry {
        ServiceEventEntry {
            service_id: "synthex".to_string(),
            event_type: "health_check".to_string(),
            health_score: 0.98,
            latency_ms: 12.5,
            timestamp: "2026-01-29T12:00:00Z".to_string(),
        }
    }

    fn make_performance_sample() -> PerformanceSample {
        PerformanceSample {
            metric_name: "pipeline_latency".to_string(),
            value: 45.2,
            unit: "ms".to_string(),
            timestamp: "2026-01-29T12:00:00Z".to_string(),
        }
    }

    async fn create_full_manager() -> (DatabaseManager, TempDir) {
        let temp = TempDir::new().expect("create temp dir");
        let mgr = DatabaseManager::new(temp.path()).await.expect("create manager");
        seed_services(&mgr).await;
        (mgr, temp)
    }

    async fn create_subset_manager(db_types: &[DatabaseType]) -> (DatabaseManager, TempDir) {
        let temp = TempDir::new().expect("create temp dir");
        let mgr = DatabaseManager::with_databases(temp.path(), db_types)
            .await
            .expect("create subset manager");
        seed_services(&mgr).await;
        (mgr, temp)
    }

    /// Insert a minimal set of service rows so that `service_events.service_id`
    /// FK lookups resolve in tests.
    async fn seed_services(mgr: &DatabaseManager) {
        if let Ok(pool) = mgr.persistence().pool(DatabaseType::ServiceTracking) {
            for name in &["synthex", "san-k7", "nais", "codesynthor", "devops-engine"] {
                let _ = execute(
                    pool,
                    "INSERT OR IGNORE INTO services (name) VALUES (?)",
                    &[*name],
                )
                .await;
            }
        }
    }

    // ====================================================================
    // 1-6: Construction tests
    // ====================================================================

    #[tokio::test]
    async fn test_new_creates_manager() {
        let (mgr, _tmp) = create_full_manager().await;
        let report = mgr.health_check_all().await.expect("health check");
        assert_eq!(report.total_databases, 11);
        assert!(report.all_healthy);
    }

    #[tokio::test]
    async fn test_with_databases_subset() {
        let types = [DatabaseType::EvolutionTracking, DatabaseType::TensorMemory];
        let (mgr, _tmp) = create_subset_manager(&types).await;
        let report = mgr.health_check_all().await.expect("health check");
        assert_eq!(report.total_databases, 2);
        assert!(report.all_healthy);
    }

    #[tokio::test]
    async fn test_with_single_database() {
        let types = [DatabaseType::ServiceTracking];
        let (mgr, _tmp) = create_subset_manager(&types).await;
        let report = mgr.health_check_all().await.expect("health check");
        assert_eq!(report.total_databases, 1);
    }

    #[tokio::test]
    async fn test_persistence_accessor() {
        let (mgr, _tmp) = create_full_manager().await;
        let persistence = mgr.persistence();
        let pool = persistence.pool(DatabaseType::EvolutionTracking);
        assert!(pool.is_ok());
    }

    #[tokio::test]
    async fn test_persistence_pool_name() {
        let (mgr, _tmp) = create_full_manager().await;
        let pool = mgr
            .persistence()
            .pool(DatabaseType::EvolutionTracking)
            .expect("pool");
        assert_eq!(pool.database_name(), "evolution_tracking");
    }

    #[tokio::test]
    async fn test_with_empty_database_list() {
        let types: &[DatabaseType] = &[];
        let (mgr, _tmp) = create_subset_manager(types).await;
        let report = mgr.health_check_all().await.expect("health check");
        assert_eq!(report.total_databases, 0);
        assert!(report.all_healthy);
    }

    // ====================================================================
    // 7-13: Fitness history write/read roundtrips
    // ====================================================================

    #[tokio::test]
    async fn test_write_fitness_history() {
        let (mgr, _tmp) = create_full_manager().await;
        let entry = make_fitness_entry();
        let affected = mgr.write_fitness_history(&entry).await.expect("write");
        assert_eq!(affected, 1);
    }

    #[tokio::test]
    async fn test_read_fitness_history_roundtrip() {
        let (mgr, _tmp) = create_full_manager().await;
        let entry = make_fitness_entry();
        mgr.write_fitness_history(&entry).await.expect("write");
        let loaded = mgr.load_fitness_history(10).await.expect("load");
        assert_eq!(loaded.len(), 1);
        assert!((loaded[0].fitness - 0.95).abs() < f64::EPSILON);
        assert_eq!(loaded[0].system_state, "healthy");
    }

    #[tokio::test]
    async fn test_empty_fitness_history() {
        let (mgr, _tmp) = create_full_manager().await;
        let loaded = mgr.load_fitness_history(10).await.expect("load");
        assert!(loaded.is_empty());
    }

    #[tokio::test]
    async fn test_multiple_fitness_writes() {
        let (mgr, _tmp) = create_full_manager().await;
        for i in 0_u32..5 {
            let mut entry = make_fitness_entry();
            entry.generation = u64::from(i);
            entry.fitness = f64::from(i).mul_add(0.1, 0.5);
            mgr.write_fitness_history(&entry).await.expect("write");
        }
        let loaded = mgr.load_fitness_history(10).await.expect("load");
        assert_eq!(loaded.len(), 5);
    }

    #[tokio::test]
    async fn test_fitness_history_limit() {
        let (mgr, _tmp) = create_full_manager().await;
        for i in 0_u32..10 {
            let mut entry = make_fitness_entry();
            entry.generation = u64::from(i);
            mgr.write_fitness_history(&entry).await.expect("write");
        }
        let loaded = mgr.load_fitness_history(3).await.expect("load");
        assert_eq!(loaded.len(), 3);
    }

    #[tokio::test]
    async fn test_fitness_tensor_hash_preserved() {
        let (mgr, _tmp) = create_full_manager().await;
        let mut entry = make_fitness_entry();
        entry.tensor_hash = "deadbeef".to_string();
        mgr.write_fitness_history(&entry).await.expect("write");
        let loaded = mgr.load_fitness_history(1).await.expect("load");
        assert_eq!(loaded[0].tensor_hash, "deadbeef");
    }

    #[tokio::test]
    async fn test_fitness_generation_preserved() {
        let (mgr, _tmp) = create_full_manager().await;
        let mut entry = make_fitness_entry();
        entry.generation = 999;
        mgr.write_fitness_history(&entry).await.expect("write");
        let loaded = mgr.load_fitness_history(1).await.expect("load");
        assert_eq!(loaded[0].generation, 999);
    }

    // ====================================================================
    // 14-18: Tensor snapshot write/read
    // ====================================================================

    #[tokio::test]
    async fn test_write_tensor_snapshot() {
        let (mgr, _tmp) = create_full_manager().await;
        let snap = make_tensor_snapshot();
        let affected = mgr.write_tensor_snapshot(&snap).await.expect("write");
        assert_eq!(affected, 1);
    }

    #[tokio::test]
    async fn test_tensor_snapshot_dimensions_preserved() {
        let types = [DatabaseType::TensorMemory];
        let (mgr, _tmp) = create_subset_manager(&types).await;
        let snap = make_tensor_snapshot();
        mgr.write_tensor_snapshot(&snap).await.expect("write");

        // Verify via raw query on structured columns.
        let pool = mgr
            .persistence()
            .pool(DatabaseType::TensorMemory)
            .expect("pool");
        let row = sqlx::query(
            "SELECT d6_health, d11_temporal, service_id FROM tensor_snapshots LIMIT 1",
        )
        .fetch_one(pool.inner())
        .await
        .expect("fetch");
        // D6 = 0.7 (7th element, index 6), D11 = 0.99 (index 11).
        let d6: f64 = row.try_get("d6_health").expect("d6");
        let d11: f64 = row.try_get("d11_temporal").expect("d11");
        assert!((d6 - 0.7).abs() < 0.01);
        assert!((d11 - 0.99).abs() < 0.01);
    }

    #[tokio::test]
    async fn test_tensor_snapshot_source_preserved() {
        let (mgr, _tmp) = create_full_manager().await;
        let mut snap = make_tensor_snapshot();
        snap.source = "evolution_chamber".to_string();
        mgr.write_tensor_snapshot(&snap).await.expect("write");

        let pool = mgr
            .persistence()
            .pool(DatabaseType::TensorMemory)
            .expect("pool");
        let row = sqlx::query("SELECT service_id FROM tensor_snapshots LIMIT 1")
            .fetch_one(pool.inner())
            .await
            .expect("fetch");
        let source: String = row.try_get("service_id").expect("service_id");
        assert_eq!(source, "evolution_chamber");
    }

    #[tokio::test]
    async fn test_tensor_snapshot_tick_preserved() {
        let (mgr, _tmp) = create_full_manager().await;
        let mut snap = make_tensor_snapshot();
        snap.tick = 12345;
        mgr.write_tensor_snapshot(&snap).await.expect("write");

        let pool = mgr
            .persistence()
            .pool(DatabaseType::TensorMemory)
            .expect("pool");
        let row = sqlx::query("SELECT snapshot_id FROM tensor_snapshots LIMIT 1")
            .fetch_one(pool.inner())
            .await
            .expect("fetch");
        let snapshot_id: String = row.try_get("snapshot_id").expect("snapshot_id");
        // snapshot_id = "{source}-{tick}"
        assert!(snapshot_id.ends_with("-12345"));
    }

    #[tokio::test]
    async fn test_multiple_tensor_snapshots() {
        let (mgr, _tmp) = create_full_manager().await;
        for i in 0_u32..4 {
            let mut snap = make_tensor_snapshot();
            snap.tick = u64::from(i);
            mgr.write_tensor_snapshot(&snap).await.expect("write");
        }
        let pool = mgr
            .persistence()
            .pool(DatabaseType::TensorMemory)
            .expect("pool");
        let rows = sqlx::query("SELECT id FROM tensor_snapshots")
            .fetch_all(pool.inner())
            .await
            .expect("fetch");
        assert_eq!(rows.len(), 4);
    }

    // ====================================================================
    // 19-22: Emergence write/read
    // ====================================================================

    #[tokio::test]
    async fn test_write_emergence() {
        let (mgr, _tmp) = create_full_manager().await;
        let entry = make_emergence_entry();
        let affected = mgr.write_emergence(&entry).await.expect("write");
        assert_eq!(affected, 1);
    }

    #[tokio::test]
    async fn test_emergence_roundtrip() {
        let (mgr, _tmp) = create_full_manager().await;
        let entry = make_emergence_entry();
        mgr.write_emergence(&entry).await.expect("write");

        let pool = mgr
            .persistence()
            .pool(DatabaseType::EvolutionTracking)
            .expect("pool");
        let rows = sqlx::query("SELECT data FROM emergence_log")
            .fetch_all(pool.inner())
            .await
            .expect("fetch");
        let items: Vec<EmergenceEntry> = deserialize_rows(&rows).expect("deser");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].emergence_type, "cascade");
        assert!((items[0].confidence - 0.87).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn test_emergence_severity_preserved() {
        let (mgr, _tmp) = create_full_manager().await;
        let mut entry = make_emergence_entry();
        entry.severity = 0.92;
        mgr.write_emergence(&entry).await.expect("write");

        let pool = mgr
            .persistence()
            .pool(DatabaseType::EvolutionTracking)
            .expect("pool");
        let rows = sqlx::query("SELECT data FROM emergence_log")
            .fetch_all(pool.inner())
            .await
            .expect("fetch");
        let items: Vec<EmergenceEntry> = deserialize_rows(&rows).expect("deser");
        assert!((items[0].severity - 0.92).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn test_emergence_description_preserved() {
        let (mgr, _tmp) = create_full_manager().await;
        let mut entry = make_emergence_entry();
        entry.description = "Resonance pattern in L5 learning layer".to_string();
        mgr.write_emergence(&entry).await.expect("write");

        let pool = mgr
            .persistence()
            .pool(DatabaseType::EvolutionTracking)
            .expect("pool");
        let rows = sqlx::query("SELECT data FROM emergence_log")
            .fetch_all(pool.inner())
            .await
            .expect("fetch");
        let items: Vec<EmergenceEntry> = deserialize_rows(&rows).expect("deser");
        assert_eq!(items[0].description, "Resonance pattern in L5 learning layer");
    }

    // ====================================================================
    // 23-27: Mutation write/read
    // ====================================================================

    #[tokio::test]
    async fn test_write_mutation() {
        let (mgr, _tmp) = create_full_manager().await;
        let entry = make_mutation_entry();
        let affected = mgr.write_mutation(&entry).await.expect("write");
        assert_eq!(affected, 1);
    }

    #[tokio::test]
    async fn test_mutation_roundtrip() {
        let (mgr, _tmp) = create_full_manager().await;
        let entry = make_mutation_entry();
        mgr.write_mutation(&entry).await.expect("write");
        let loaded = mgr.load_recent_mutations(10).await.expect("load");
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].target_parameter, "ltp_rate");
        assert!((loaded[0].original_value - 0.1).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn test_empty_mutations() {
        let (mgr, _tmp) = create_full_manager().await;
        let loaded = mgr.load_recent_mutations(10).await.expect("load");
        assert!(loaded.is_empty());
    }

    #[tokio::test]
    async fn test_mutation_applied_flag() {
        let (mgr, _tmp) = create_full_manager().await;
        let mut entry = make_mutation_entry();
        entry.applied = false;
        entry.rolled_back = true;
        mgr.write_mutation(&entry).await.expect("write");
        let loaded = mgr.load_recent_mutations(1).await.expect("load");
        assert!(!loaded[0].applied);
        assert!(loaded[0].rolled_back);
    }

    #[tokio::test]
    async fn test_mutation_limit() {
        let (mgr, _tmp) = create_full_manager().await;
        for i in 0_u32..8 {
            let mut entry = make_mutation_entry();
            entry.generation = u64::from(i);
            mgr.write_mutation(&entry).await.expect("write");
        }
        let loaded = mgr.load_recent_mutations(3).await.expect("load");
        assert_eq!(loaded.len(), 3);
    }

    // ====================================================================
    // 28-30: Correlation write/read
    // ====================================================================

    #[tokio::test]
    async fn test_write_correlation() {
        let (mgr, _tmp) = create_full_manager().await;
        let entry = make_correlation_entry();
        let affected = mgr.write_correlation(&entry).await.expect("write");
        assert_eq!(affected, 1);
    }

    #[tokio::test]
    async fn test_correlation_roundtrip() {
        let (mgr, _tmp) = create_full_manager().await;
        let entry = make_correlation_entry();
        mgr.write_correlation(&entry).await.expect("write");

        let pool = mgr
            .persistence()
            .pool(DatabaseType::EvolutionTracking)
            .expect("pool");
        let rows = sqlx::query("SELECT data FROM correlation_log")
            .fetch_all(pool.inner())
            .await
            .expect("fetch");
        let items: Vec<CorrelationEntry> = deserialize_rows(&rows).expect("deser");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].channel, "health");
        assert_eq!(items[0].link_count, 3);
    }

    #[tokio::test]
    async fn test_correlation_event_type_preserved() {
        let (mgr, _tmp) = create_full_manager().await;
        let mut entry = make_correlation_entry();
        entry.event_type = "latency_spike".to_string();
        mgr.write_correlation(&entry).await.expect("write");

        let pool = mgr
            .persistence()
            .pool(DatabaseType::EvolutionTracking)
            .expect("pool");
        let rows = sqlx::query("SELECT data FROM correlation_log")
            .fetch_all(pool.inner())
            .await
            .expect("fetch");
        let items: Vec<CorrelationEntry> = deserialize_rows(&rows).expect("deser");
        assert_eq!(items[0].event_type, "latency_spike");
    }

    // ====================================================================
    // 31-35: Service event write/read
    // ====================================================================

    #[tokio::test]
    async fn test_write_service_event() {
        let (mgr, _tmp) = create_full_manager().await;
        let entry = make_service_event();
        let affected = mgr.write_service_event(&entry).await.expect("write");
        assert_eq!(affected, 1);
    }

    #[tokio::test]
    async fn test_service_event_roundtrip() {
        let (mgr, _tmp) = create_full_manager().await;
        let entry = make_service_event();
        mgr.write_service_event(&entry).await.expect("write");
        let loaded = mgr.load_service_health().await.expect("load");
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].service_id, "synthex");
        assert!((loaded[0].health_score - 0.98).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn test_empty_service_health() {
        let (mgr, _tmp) = create_full_manager().await;
        let loaded = mgr.load_service_health().await.expect("load");
        assert!(loaded.is_empty());
    }

    #[tokio::test]
    async fn test_service_latency_preserved() {
        let (mgr, _tmp) = create_full_manager().await;
        let mut entry = make_service_event();
        entry.latency_ms = 250.75;
        mgr.write_service_event(&entry).await.expect("write");
        let loaded = mgr.load_service_health().await.expect("load");
        assert!((loaded[0].latency_ms - 250.75).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn test_multiple_service_events() {
        let (mgr, _tmp) = create_full_manager().await;
        let services = ["synthex", "san-k7", "nais", "codesynthor"];
        for svc in &services {
            let mut entry = make_service_event();
            entry.service_id = (*svc).to_string();
            mgr.write_service_event(&entry).await.expect("write");
        }
        let loaded = mgr.load_service_health().await.expect("load");
        assert_eq!(loaded.len(), 4);
    }

    // ====================================================================
    // 36-39: Performance sample write/read
    // ====================================================================

    #[tokio::test]
    async fn test_write_performance_sample() {
        let (mgr, _tmp) = create_full_manager().await;
        let sample = make_performance_sample();
        let affected = mgr
            .write_performance_sample(&sample)
            .await
            .expect("write");
        assert_eq!(affected, 1);
    }

    #[tokio::test]
    async fn test_performance_sample_roundtrip() {
        let (mgr, _tmp) = create_full_manager().await;
        let sample = make_performance_sample();
        mgr.write_performance_sample(&sample).await.expect("write");

        let pool = mgr
            .persistence()
            .pool(DatabaseType::PerformanceMetrics)
            .expect("pool");
        let row = sqlx::query(
            "SELECT service_id, avg_response_ms FROM performance_samples LIMIT 1",
        )
        .fetch_one(pool.inner())
        .await
        .expect("fetch");
        let metric: String = row.try_get("service_id").expect("service_id");
        let value: f64 = row.try_get("avg_response_ms").expect("value");
        assert_eq!(metric, "pipeline_latency");
        assert!((value - 45.2).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn test_performance_unit_preserved() {
        let (mgr, _tmp) = create_full_manager().await;
        let mut sample = make_performance_sample();
        sample.unit = "bytes".to_string();
        mgr.write_performance_sample(&sample).await.expect("write");

        let pool = mgr
            .persistence()
            .pool(DatabaseType::PerformanceMetrics)
            .expect("pool");
        let row = sqlx::query("SELECT sample_id FROM performance_samples LIMIT 1")
            .fetch_one(pool.inner())
            .await
            .expect("fetch");
        let sample_id: String = row.try_get("sample_id").expect("sample_id");
        // sample_id = "{metric_name}-{timestamp}"
        assert!(sample_id.starts_with("pipeline_latency-"));
    }

    #[tokio::test]
    async fn test_multiple_performance_samples() {
        let (mgr, _tmp) = create_full_manager().await;
        for i in 0..6 {
            let mut sample = make_performance_sample();
            sample.value = f64::from(i) * 10.0;
            // Ensure unique sample_id by varying the timestamp
            sample.timestamp = format!("2026-01-29T12:00:0{i}Z");
            mgr.write_performance_sample(&sample)
                .await
                .expect("write");
        }
        let pool = mgr
            .persistence()
            .pool(DatabaseType::PerformanceMetrics)
            .expect("pool");
        let rows = sqlx::query("SELECT id FROM performance_samples")
            .fetch_all(pool.inner())
            .await
            .expect("fetch");
        assert_eq!(rows.len(), 6);
    }

    // ====================================================================
    // 40-44: Health check tests
    // ====================================================================

    #[tokio::test]
    async fn test_health_check_all_reports_all_healthy() {
        let (mgr, _tmp) = create_full_manager().await;
        let report = mgr.health_check_all().await.expect("health check");
        assert!(report.all_healthy);
        assert_eq!(report.total_databases, 11);
        assert_eq!(report.healthy_databases, 11);
    }

    #[tokio::test]
    async fn test_health_check_report_has_timestamp() {
        let (mgr, _tmp) = create_full_manager().await;
        let report = mgr.health_check_all().await.expect("health check");
        assert!(!report.checked_at.is_empty());
    }

    #[tokio::test]
    async fn test_health_check_updates_cache() {
        let (mgr, _tmp) = create_full_manager().await;

        // Before health check, cache should be empty
        {
            let cache = mgr.health_cache.read();
            assert!(cache.last_check.is_none());
        }

        mgr.health_check_all().await.expect("health check");

        // After health check, cache should be populated
        {
            let cache = mgr.health_cache.read();
            assert!(cache.last_check.is_some());
            assert!(cache.all_healthy);
            assert_eq!(cache.db_count, 11);
        }
    }

    #[tokio::test]
    async fn test_health_check_subset() {
        let types = [DatabaseType::EvolutionTracking, DatabaseType::ServiceTracking];
        let (mgr, _tmp) = create_subset_manager(&types).await;
        let report = mgr.health_check_all().await.expect("health check");
        assert_eq!(report.total_databases, 2);
        assert!(report.all_healthy);
    }

    #[tokio::test]
    async fn test_health_report_serialization() {
        let report = DatabaseHealthReport {
            total_databases: 11,
            healthy_databases: 11,
            all_healthy: true,
            checked_at: "2026-01-29T12:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&report).expect("serialize");
        let deser: DatabaseHealthReport = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(deser.total_databases, 11);
        assert!(deser.all_healthy);
    }

    // ====================================================================
    // 45-50: Cross-cutting / edge case tests
    // ====================================================================

    #[tokio::test]
    async fn test_write_to_subset_evolution_only() {
        let types = [DatabaseType::EvolutionTracking];
        let (mgr, _tmp) = create_subset_manager(&types).await;

        // Writing to evolution tracking should work
        let entry = make_fitness_entry();
        let result = mgr.write_fitness_history(&entry).await;
        assert!(result.is_ok());

        // Writing to service tracking should fail (pool not initialized)
        let svc = make_service_event();
        let result = mgr.write_service_event(&svc).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_data_types_serialize_deserialize() {
        // Verify all data types roundtrip through serde_json
        let fitness_json =
            serde_json::to_string(&make_fitness_entry()).expect("serialize fitness");
        let _: FitnessHistoryEntry =
            serde_json::from_str(&fitness_json).expect("deserialize fitness");

        let tensor_json =
            serde_json::to_string(&make_tensor_snapshot()).expect("serialize tensor");
        let _: TensorSnapshot =
            serde_json::from_str(&tensor_json).expect("deserialize tensor");

        let emergence_json =
            serde_json::to_string(&make_emergence_entry()).expect("serialize emergence");
        let _: EmergenceEntry =
            serde_json::from_str(&emergence_json).expect("deserialize emergence");

        let mutation_json =
            serde_json::to_string(&make_mutation_entry()).expect("serialize mutation");
        let _: MutationEntry =
            serde_json::from_str(&mutation_json).expect("deserialize mutation");

        let correlation_json =
            serde_json::to_string(&make_correlation_entry()).expect("serialize correlation");
        let _: CorrelationEntry =
            serde_json::from_str(&correlation_json).expect("deserialize correlation");

        let service_json =
            serde_json::to_string(&make_service_event()).expect("serialize service");
        let _: ServiceEventEntry =
            serde_json::from_str(&service_json).expect("deserialize service");

        let perf_json =
            serde_json::to_string(&make_performance_sample()).expect("serialize perf");
        let _: PerformanceSample =
            serde_json::from_str(&perf_json).expect("deserialize perf");
    }

    #[tokio::test]
    async fn test_concurrent_writes_same_table() {
        let (mgr, _tmp) = create_full_manager().await;
        let mgr = std::sync::Arc::new(mgr);

        let mut handles = Vec::new();
        for i in 0_u32..10 {
            let mgr_clone = std::sync::Arc::clone(&mgr);
            handles.push(tokio::spawn(async move {
                let mut entry = make_fitness_entry();
                entry.generation = u64::from(i);
                mgr_clone
                    .write_fitness_history(&entry)
                    .await
                    .expect("concurrent write");
            }));
        }
        for handle in handles {
            handle.await.expect("join");
        }

        let loaded = mgr.load_fitness_history(20).await.expect("load");
        assert_eq!(loaded.len(), 10);
    }

    #[tokio::test]
    async fn test_fitness_entry_clone() {
        let entry = make_fitness_entry();
        let cloned = entry.clone();
        assert_eq!(cloned.generation, entry.generation);
        assert_eq!(cloned.system_state, entry.system_state);
    }

    #[tokio::test]
    async fn test_mutation_entry_debug_format() {
        let entry = make_mutation_entry();
        let debug_str = format!("{entry:?}");
        assert!(debug_str.contains("ltp_rate"));
        assert!(debug_str.contains("MutationEntry"));
    }

    // ====================================================================
    // Cognitive state persistence (NAM-T2)
    // ====================================================================

    fn make_cognitive_state() -> CognitiveState {
        CognitiveState {
            window_size_ms: 60_000,
            generation: 5,
            tick_count: 42,
            zero_correlation_streak: 0,
            fitness: 0.381,
            saved_at: "2026-03-10T20:00:00Z".to_string(),
        }
    }

    #[tokio::test]
    async fn test_write_cognitive_state() {
        let (mgr, _tmp) = create_full_manager().await;
        let state = make_cognitive_state();
        let affected = mgr.write_cognitive_state(&state).await.expect("write");
        assert_eq!(affected, 1);
    }

    #[tokio::test]
    async fn test_cognitive_state_roundtrip() {
        let (mgr, _tmp) = create_full_manager().await;
        let state = make_cognitive_state();
        mgr.write_cognitive_state(&state).await.expect("write");
        let loaded = mgr.read_cognitive_state().await.expect("read");
        let loaded = loaded.expect("should exist");
        assert_eq!(loaded.window_size_ms, 60_000);
        assert_eq!(loaded.generation, 5);
        assert_eq!(loaded.tick_count, 42);
        assert_eq!(loaded.zero_correlation_streak, 0);
        assert!((loaded.fitness - 0.381).abs() < 1e-10);
    }

    #[tokio::test]
    async fn test_cognitive_state_empty_db() {
        let (mgr, _tmp) = create_full_manager().await;
        let loaded = mgr.read_cognitive_state().await.expect("read");
        assert!(loaded.is_none());
    }

    #[tokio::test]
    async fn test_cognitive_state_upsert() {
        let (mgr, _tmp) = create_full_manager().await;
        let mut state = make_cognitive_state();
        mgr.write_cognitive_state(&state).await.expect("write1");
        state.tick_count = 100;
        state.fitness = 0.55;
        mgr.write_cognitive_state(&state).await.expect("write2");
        let loaded = mgr.read_cognitive_state().await.expect("read");
        let loaded = loaded.expect("should exist");
        // Second write overwrites first (single-row upsert)
        assert_eq!(loaded.tick_count, 100);
        assert!((loaded.fitness - 0.55).abs() < 1e-10);
    }

    #[tokio::test]
    async fn test_cognitive_state_serialization() {
        let state = make_cognitive_state();
        let json = serde_json::to_string(&state).expect("serialize");
        let deser: CognitiveState = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(deser.generation, state.generation);
        assert_eq!(deser.window_size_ms, state.window_size_ms);
    }
}
