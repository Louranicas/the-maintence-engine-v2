# M05 State Persistence — state.rs

> **File:** `src/m1_foundation/state.rs` | **LOC:** ~1,209 | **Tests:** 10
> **Role:** SQLite database connectivity, query building, migration support, 11 database types

---

## StateStore Trait

```rust
pub trait StateStore: Send + Sync {
    fn pool(&self) -> &DatabasePool;
    fn store_name(&self) -> &str;
    fn agent_id(&self) -> Option<&str> { None }  // default (NAM R5)
}
```

`DatabasePool` implements `StateStore` (blanket impl: pool=self, store_name=database_name).

---

## DatabaseType Enum (11 variants)

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DatabaseType {
    ServiceTracking, SystemSynergy, HebbianPulse, ConsensusTracking,
    EpisodicMemory, TensorMemory, PerformanceMetrics, FlowState,
    SecurityEvents, WorkflowTracking, EvolutionTracking,
}
```

| Method | Signature | Notes |
|--------|-----------|-------|
| `filename` | `const fn(&self) -> &'static str` | e.g. "service_tracking.db" |
| `migration_number` | `const fn(&self) -> u32` | 1-11 |
| `all` | `const fn() -> [Self; 11]` | All variants |

Traits: `Display` (delegates to filename)

---

## DatabaseConfig (Builder)

```rust
DatabaseConfig::new(path)
    .with_max_connections(20)    // const fn, default 10
    .with_min_connections(5)     // const fn, default 2
    .with_acquire_timeout(60)    // const fn, default 30 secs
    .with_wal_mode(true)         // const fn, default true
```

Default: `path="data/maintenance.db"`, max=10, min=2, timeout=30, wal=true

---

## DatabasePool

```rust
// Clone (backed by SqlitePool — internally Arc-shared)
pub fn database_name(&self) -> &str
pub fn path(&self) -> &str
pub fn inner(&self) -> &SqlitePool          // const fn
pub fn stats(&self) -> PoolStats
pub async fn health_check(&self) -> Result<bool>
```

---

## QueryBuilder (Fluent)

```rust
// SELECT
QueryBuilder::select(&["col1", "col2"])
    .from("table")
    .where_eq("col", value)
    .and_eq("col", value)
    .or_eq("col", value)
    .order_by("col", "ASC")
    .limit(10).offset(0)
    .build() -> &str

// INSERT
QueryBuilder::insert_into("table", &["col1", "col2"])
    .values(&["v1", "v2"])
    .build() -> &str

// UPDATE
QueryBuilder::update("table")
    .set("col", value)
    .where_eq("col", value)
    .build() -> &str

// DELETE
QueryBuilder::delete_from("table")
    .where_eq("col", value)
    .build() -> &str
```

Terminal methods: `.build() -> &str`, `.params() -> Vec<&str>`, `.params_owned() -> &[String]`

---

## Free Functions (all async)

| Function | Returns | Notes |
|----------|---------|-------|
| `connect(&DatabaseConfig)` | `DatabasePool` | Creates pool with SQLite options |
| `execute(pool, query, params)` | `u64` (rows affected) | |
| `fetch_one<T: DeserializeOwned>(pool, query, params)` | `T` | Errors if no row |
| `fetch_all<T: DeserializeOwned>(pool, query, params)` | `Vec<T>` | |
| `fetch_optional<T: DeserializeOwned>(pool, query, params)` | `Option<T>` | |
| `begin_transaction(pool)` | `Transaction<'_>` | |
| `run_migrations(pool, dir)` | `()` | Reads .sql files, tracks applied |
| `save<T: Serialize>(pool, table, key, value)` | `u64` | UPSERT pattern |
| `save_with_provenance<T>(pool, table, key, value, agent_id)` | `u64` | NAM R5 attribution |
| `save_versioned<T>(pool, table, key, value, version)` | `u64` | Optimistic concurrency |
| `load<T: DeserializeOwned>(pool, table, key)` | `Option<T>` | |
| `delete(pool, table, key)` | `bool` | |
| `exists(pool, table, key)` | `bool` | |
| `count(pool, table, condition, params)` | `i64` | |

---

## Transaction

```rust
pub async fn commit(self) -> Result<()>
pub async fn rollback(self) -> Result<()>
pub async fn execute(&mut self, query, params) -> Result<u64>
pub async fn fetch_one<T>(&mut self, query, params) -> Result<T>
pub async fn fetch_all<T>(&mut self, query, params) -> Result<Vec<T>>
```

---

## StatePersistence (Multi-Database Manager)

```rust
StatePersistence::builder()
    .base_dir(PathBuf)
    .migrations_dir(PathBuf)
    .config(DatabaseConfig)
    .with_database(DatabaseType)    // or .with_all_databases()
    .build().await -> Result<Self>
```

| Method | Returns | Notes |
|--------|---------|-------|
| `pool(&self, DatabaseType)` | `Result<&DatabasePool>` | Error if not initialized |
| `health_check_all(&self)` | `HashMap<DatabaseType, bool>` | |
| `stats_all(&self)` | `HashMap<DatabaseType, PoolStats>` | |
| `to_tensor(&self)` | `Tensor12D` | D2=tier, D3=db_count/11, D6=1.0 |

Internal: `Arc<HashMap<DatabaseType, DatabasePool>>` — clone-safe.

---

## Errors Produced

All errors are `Error::Database(String)` with descriptive messages:
- Connection failures, SQL execution errors, migration failures
- `"Version mismatch: expected {N} for key '{k}'"` (optimistic concurrency)
- `"Database not initialized: {type}"`

---

*M05 State Persistence Spec v1.0 | 2026-03-01*
