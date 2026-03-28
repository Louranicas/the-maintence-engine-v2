# Antipatterns - What to Avoid

> 40+ Patterns to Avoid in Rust Development

---

## Critical Antipatterns (P0 - Never Do)

### A01: Using .unwrap()

**Problem**: Panics on None/Err, crashes entire process

```rust
// BAD - Will panic if service not found
let service = services.get(id).unwrap();
let config = load_config().unwrap();

// GOOD - Handle the error case
let service = services.get(id)
    .ok_or_else(|| Error::ServiceNotFound(id.to_string()))?;
let config = load_config()?;
```

**Enforcement**:
```toml
[workspace.lints.clippy]
unwrap_used = "deny"
```

---

### A02: Using .expect()

**Problem**: Same as unwrap, just with a message

```rust
// BAD - Still panics
let port: u16 = env::var("PORT").expect("PORT must be set").parse().expect("PORT must be a number");

// GOOD - Return error
let port: u16 = env::var("PORT")
    .map_err(|_| Error::Config("PORT not set".into()))?
    .parse()
    .map_err(|_| Error::Config("PORT must be a number".into()))?;
```

**Enforcement**:
```toml
[workspace.lints.clippy]
expect_used = "deny"
```

---

### A03: Panic in Library Code

**Problem**: Crashes caller's application

```rust
// BAD - Never panic in libraries
pub fn validate_tensor(t: &Tensor12D) {
    if t.health < 0.0 {
        panic!("Health cannot be negative!");
    }
}

// GOOD - Return Result
pub fn validate_tensor(t: &Tensor12D) -> Result<()> {
    if t.health < 0.0 {
        return Err(Error::TensorValidation {
            dimension: "health".into(),
            value: t.health,
            expected: "[0.0, 1.0]".into(),
        });
    }
    Ok(())
}
```

**Enforcement**:
```toml
[workspace.lints.clippy]
panic = "deny"
```

---

### A04: Unsafe Code Without Justification

**Problem**: Memory safety bugs, undefined behavior

```rust
// BAD - Unsafe for performance is rarely justified
unsafe fn fast_copy(src: *const u8, dst: *mut u8, len: usize) {
    std::ptr::copy_nonoverlapping(src, dst, len);
}

// GOOD - Use safe APIs
fn copy_bytes(src: &[u8], dst: &mut [u8]) {
    dst[..src.len()].copy_from_slice(src);
}
```

**Enforcement**:
```rust
#![forbid(unsafe_code)]
```

---

### A05: Blocking in Async Context

**Problem**: Starves other tasks, defeats async benefits

```rust
// BAD - Blocks the async runtime
async fn read_file(path: &str) -> Result<String> {
    std::fs::read_to_string(path)  // BLOCKS!
        .map_err(Error::from)
}

// GOOD - Use async I/O
async fn read_file(path: &str) -> Result<String> {
    tokio::fs::read_to_string(path).await
        .map_err(Error::from)
}

// BAD - std Mutex in async
async fn update_state(state: &std::sync::Mutex<State>) {
    let mut guard = state.lock().unwrap();  // BLOCKS!
    // ... await something ...
}

// GOOD - tokio Mutex
async fn update_state(state: &tokio::sync::Mutex<State>) {
    let mut guard = state.lock().await;
    // ... await something ...
}
```

---

### A06: Silent Error Swallowing

**Problem**: Bugs go undetected, debugging impossible

```rust
// BAD - Error completely ignored
if let Ok(result) = operation() {
    use_result(result);
}
// What happened to the error?

// BAD - Converting to () silently
let _ = channel.send(event);

// GOOD - Log or propagate
match operation() {
    Ok(result) => use_result(result),
    Err(e) => tracing::warn!(error = %e, "Operation failed, using default"),
}

// GOOD - Explicit discard with logging
if let Err(e) = channel.send(event) {
    tracing::debug!(error = %e, "Channel closed, event dropped");
}
```

---

### A07: Hardcoded Credentials

**Problem**: Security vulnerability, cannot rotate

```rust
// BAD - Credentials in code
const API_KEY: &str = "sk-1234567890abcdef";
const DB_PASSWORD: &str = "supersecret123";

// GOOD - Environment variables
fn get_api_key() -> Result<String> {
    std::env::var("API_KEY")
        .map_err(|_| Error::Config("API_KEY not set".into()))
}

// GOOD - Config file (not in repo)
fn get_db_password(config: &Config) -> &str {
    &config.database.password
}
```

---

## High Priority Antipatterns (P1 - Avoid)

### A08: Clone Instead of Borrow

**Problem**: Unnecessary allocation, performance hit

```rust
// BAD - Cloning for no reason
fn process_name(name: String) {
    println!("{}", name);
}
process_name(service.name.clone());

// GOOD - Borrow when possible
fn process_name(name: &str) {
    println!("{}", name);
}
process_name(&service.name);
```

---

### A09: String Instead of &str in Parameters

**Problem**: Forces caller to allocate

```rust
// BAD - Requires owned String
fn find_service(id: String) -> Option<Service> { }
find_service(my_id.to_string());  // Allocation!

// GOOD - Accept borrowed
fn find_service(id: &str) -> Option<Service> { }
find_service(&my_id);  // No allocation

// BETTER - Accept anything string-like
fn find_service(id: impl AsRef<str>) -> Option<Service> {
    let id = id.as_ref();
    // ...
}
```

---

### A10: Magic Numbers

**Problem**: Unclear intent, hard to maintain

```rust
// BAD - What do these mean?
if score > 0.9 {
    execute_action();
}
if failures > 5 {
    open_circuit();
}

// GOOD - Named constants
const AUTO_EXECUTE_THRESHOLD: f64 = 0.9;
const CIRCUIT_BREAKER_THRESHOLD: u32 = 5;

if score > AUTO_EXECUTE_THRESHOLD {
    execute_action();
}
if failures > CIRCUIT_BREAKER_THRESHOLD {
    open_circuit();
}

// BETTER - Configuration
if score > config.escalation.l0_threshold {
    execute_action();
}
```

---

### A11: Mutex Poisoning Panics

**Problem**: One panic takes down everything

```rust
// BAD - Panics if mutex poisoned
let guard = mutex.lock().unwrap();

// GOOD - Handle poisoning
let guard = mutex.lock()
    .map_err(|_| Error::Other("Mutex poisoned".into()))?;

// GOOD - Recover from poisoning
let guard = mutex.lock().unwrap_or_else(|poisoned| {
    tracing::warn!("Recovering from poisoned mutex");
    poisoned.into_inner()
});
```

---

### A12: Unbounded Channels

**Problem**: Memory grows forever under load

```rust
// BAD - No backpressure
let (tx, rx) = mpsc::unbounded_channel();

// GOOD - Bounded with reasonable size
let (tx, rx) = mpsc::channel(1000);

// GOOD - Handle full channel
match tx.try_send(event) {
    Ok(()) => {}
    Err(mpsc::error::TrySendError::Full(event)) => {
        tracing::warn!("Channel full, dropping event");
        metrics.record_dropped_event();
    }
    Err(mpsc::error::TrySendError::Closed(_)) => {
        return Err(Error::Other("Channel closed".into()));
    }
}
```

---

### A13: Vec Instead of Iterator

**Problem**: Unnecessary intermediate allocation

```rust
// BAD - Creates intermediate Vec
let ids: Vec<String> = services.iter()
    .map(|s| s.id.clone())
    .collect();
let healthy: Vec<&Service> = ids.iter()
    .filter_map(|id| services.get(id))
    .collect();

// GOOD - Chain iterators
let healthy: Vec<&Service> = services.iter()
    .filter(|s| s.is_healthy())
    .collect();
```

---

### A14: Deeply Nested Code

**Problem**: Hard to read, maintain, test

```rust
// BAD - Pyramid of doom
fn process(input: Option<Input>) -> Result<Output> {
    if let Some(input) = input {
        if input.is_valid() {
            if let Ok(parsed) = parse(input) {
                if parsed.value > 0 {
                    return Ok(transform(parsed));
                }
            }
        }
    }
    Err(Error::Other("Failed".into()))
}

// GOOD - Early returns
fn process(input: Option<Input>) -> Result<Output> {
    let input = input.ok_or(Error::Other("No input".into()))?;

    if !input.is_valid() {
        return Err(Error::Validation("Invalid input".into()));
    }

    let parsed = parse(input)?;

    if parsed.value <= 0 {
        return Err(Error::Validation("Value must be positive".into()));
    }

    Ok(transform(parsed))
}
```

---

### A15: Mutable Static Variables

**Problem**: Thread safety issues, testing nightmares

```rust
// BAD - Global mutable state
static mut COUNTER: u64 = 0;

fn increment() {
    unsafe { COUNTER += 1; }  // Data race!
}

// GOOD - Thread-safe alternatives
static COUNTER: AtomicU64 = AtomicU64::new(0);

fn increment() {
    COUNTER.fetch_add(1, Ordering::Relaxed);
}

// BETTER - Pass state explicitly
fn increment(counter: &AtomicU64) {
    counter.fetch_add(1, Ordering::Relaxed);
}
```

---

### A16: Floating Point Equality

**Problem**: Precision issues cause false negatives

```rust
// BAD - Direct comparison
if health == 0.9 {
    execute();
}

// GOOD - Epsilon comparison
const EPSILON: f64 = 1e-10;
if (health - 0.9).abs() < EPSILON {
    execute();
}

// BETTER - Range check
if health >= 0.9 {
    execute();
}
```

---

### A17: Indexing Without Bounds Check

**Problem**: Panics on out-of-bounds

```rust
// BAD - Panics if index out of bounds
let item = vec[index];
let byte = bytes[5];

// GOOD - Safe access
let item = vec.get(index)
    .ok_or(Error::Other("Index out of bounds".into()))?;

// GOOD - With default
let byte = bytes.get(5).copied().unwrap_or(0);
```

---

### A18: Ignoring Future Return Values

**Problem**: Future never executes

```rust
// BAD - Future not awaited, never runs!
some_async_operation();

// GOOD - Await or spawn
some_async_operation().await?;

// GOOD - Fire and forget with spawn
tokio::spawn(some_async_operation());
```

---

## Medium Priority Antipatterns (P2 - Discouraged)

### A19: Excessive Cloning

**Problem**: Performance degradation

```rust
// BAD - Clone in hot path
fn process_all(items: &[Item]) {
    for item in items {
        let item = item.clone();  // Unnecessary clone
        process(item);
    }
}

// GOOD - Borrow
fn process_all(items: &[Item]) {
    for item in items {
        process(item);  // Pass reference
    }
}
```

---

### A20: Boolean Parameters

**Problem**: Unclear at call site

```rust
// BAD - What does true mean?
restart_service("synthex", true, false);

// GOOD - Named parameters via struct
restart_service("synthex", RestartOptions {
    graceful: true,
    force: false,
});

// GOOD - Builder pattern
restart_service("synthex")
    .graceful()
    .build()?;
```

---

### A21: Long Parameter Lists

**Problem**: Easy to mix up parameters

```rust
// BAD - Too many parameters
fn create_service(
    id: String,
    name: String,
    port: u16,
    host: String,
    timeout: u64,
    retries: u32,
    health_path: String,
) -> Service { }

// GOOD - Config struct
struct ServiceConfig {
    id: String,
    name: String,
    port: u16,
    host: String,
    timeout: u64,
    retries: u32,
    health_path: String,
}

fn create_service(config: ServiceConfig) -> Service { }
```

---

### A22: Generic Over-abstraction

**Problem**: Complexity without benefit

```rust
// BAD - Generic when not needed
fn add<T: std::ops::Add<Output = T>>(a: T, b: T) -> T {
    a + b
}

// GOOD - Concrete type is clearer
fn add(a: i32, b: i32) -> i32 {
    a + b
}

// OK - Generic when actually useful
fn find_by_id<T: HasId>(items: &[T], id: &str) -> Option<&T> {
    items.iter().find(|item| item.id() == id)
}
```

---

### A23: Stringly-Typed APIs

**Problem**: No compile-time type checking

```rust
// BAD - String for everything
fn set_status(status: &str) { }
set_status("runnnig");  // Typo compiles!

// GOOD - Enum
enum ServiceStatus { Stopped, Starting, Running, Stopping, Failed }
fn set_status(status: ServiceStatus) { }
set_status(ServiceStatus::Running);  // Type-safe
```

---

### A24: Returning Self-Referential Structs

**Problem**: Lifetime complexity, often impossible

```rust
// BAD - Self-referential (won't compile or is very complex)
struct Parser<'a> {
    data: String,
    slice: &'a str,  // Points into data - problematic!
}

// GOOD - Return owned data
struct ParseResult {
    tokens: Vec<Token>,
}

// GOOD - Or use indices
struct Parser {
    data: String,
    current_pos: usize,
}
```

---

### A25: Premature Optimization

**Problem**: Complexity without measured need

```rust
// BAD - Complex optimization without profiling
fn process(items: &[Item]) {
    // Hand-rolled SIMD, unsafe, complex...
}

// GOOD - Simple first, measure, then optimize
fn process(items: &[Item]) {
    items.iter().for_each(|item| {
        // Simple, readable implementation
    });
}
```

---

### A26: Not Using Clippy

**Problem**: Missing many subtle issues

```bash
# BAD - No linting
cargo build

# GOOD - Lint before build
cargo clippy -- -D warnings -W clippy::pedantic

# BETTER - In CI
cargo clippy --all-targets --all-features -- -D warnings
```

---

### A27: Ignoring Deprecation Warnings

**Problem**: Code will break in future versions

```rust
// BAD - Using deprecated API
#[allow(deprecated)]
fn old_way() {
    deprecated_function();
}

// GOOD - Update to new API
fn new_way() {
    replacement_function();
}
```

---

### A28: Mixing Business Logic with I/O

**Problem**: Hard to test, tightly coupled

```rust
// BAD - I/O mixed with logic
fn calculate_score(path: &str) -> f64 {
    let data = std::fs::read_to_string(path).unwrap();
    let parsed: Vec<f64> = serde_json::from_str(&data).unwrap();
    parsed.iter().sum::<f64>() / parsed.len() as f64
}

// GOOD - Separate I/O from logic
fn calculate_score(values: &[f64]) -> f64 {
    if values.is_empty() { return 0.0; }
    values.iter().sum::<f64>() / values.len() as f64
}

async fn load_and_calculate(path: &str) -> Result<f64> {
    let data = tokio::fs::read_to_string(path).await?;
    let values: Vec<f64> = serde_json::from_str(&data)?;
    Ok(calculate_score(&values))
}
```

---

### A29: Test-Only Code in Production

**Problem**: Bloated binary, potential security issues

```rust
// BAD - Test helpers in main code
pub fn create_test_service() -> Service {
    Service { id: "test".into(), .. }
}

// GOOD - Test code in test module
#[cfg(test)]
mod tests {
    fn create_test_service() -> Service {
        Service { id: "test".into(), .. }
    }
}

// GOOD - Test utilities in separate crate
// crates/test-utils/src/lib.rs
pub fn create_test_service() -> Service { }
```

---

### A30: Insufficient Error Context

**Problem**: Hard to debug production issues

```rust
// BAD - No context
fn load_config() -> Result<Config> {
    let content = std::fs::read_to_string("config.toml")?;
    Ok(toml::from_str(&content)?)
}
// Error: "No such file or directory" - which file?

// GOOD - Rich context
fn load_config(path: &str) -> Result<Config> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| Error::ConfigNotFound {
            path: path.to_string(),
        })?;

    toml::from_str(&content)
        .map_err(|e| Error::Config(format!(
            "Failed to parse {}: {}", path, e
        )))
}
```

---

## Database Antipatterns

### A31: N+1 Queries

```rust
// BAD - One query per service
for id in service_ids {
    let service = db.query("SELECT * FROM services WHERE id = ?", [id])?;
    results.push(service);
}

// GOOD - Batch query
let services = db.query(
    "SELECT * FROM services WHERE id IN (?)",
    [service_ids.join(",")]
)?;
```

---

### A32: No Connection Pooling

```rust
// BAD - New connection per request
async fn handle_request() {
    let conn = Database::connect().await?;
    // ...
}

// GOOD - Connection pool
async fn handle_request(pool: &Pool) {
    let conn = pool.acquire().await?;
    // ...
}
```

---

### A33: SQL Injection

```rust
// BAD - String interpolation
let query = format!("SELECT * FROM services WHERE name = '{}'", name);

// GOOD - Parameterized query
let query = "SELECT * FROM services WHERE name = ?";
db.query(query, [&name])?;
```

---

## Async Antipatterns

### A34: Holding Lock Across Await

```rust
// BAD - Lock held during await
async fn update() {
    let mut guard = state.lock().await;
    external_call().await;  // Lock still held!
    guard.value = 42;
}

// GOOD - Release lock before await
async fn update() {
    let current = {
        let guard = state.lock().await;
        guard.value
    };  // Lock released

    let new_value = external_call(current).await;

    let mut guard = state.lock().await;
    guard.value = new_value;
}
```

---

### A35: Spawning Without JoinHandle

```rust
// BAD - Lost task, can't cancel or wait
tokio::spawn(background_work());
// How do we know when it's done?

// GOOD - Keep handle
let handle = tokio::spawn(background_work());
// Later...
handle.await?;
```

---

## Summary Table

| # | Antipattern | Priority | Mitigation |
|---|-------------|----------|------------|
| A01 | .unwrap() | P0 | Use ? operator |
| A02 | .expect() | P0 | Use ? operator |
| A03 | panic! | P0 | Return Result |
| A04 | unsafe | P0 | Safe alternatives |
| A05 | Blocking in async | P0 | Async I/O |
| A06 | Silent errors | P0 | Log or propagate |
| A07 | Hardcoded credentials | P0 | Env vars |
| A08 | Unnecessary clone | P1 | Borrow |
| A09 | String params | P1 | &str or AsRef |
| A10 | Magic numbers | P1 | Constants |
| A11 | Mutex panics | P1 | Handle poisoning |
| A12 | Unbounded channels | P1 | Bounded + backpressure |
| A13 | Intermediate Vecs | P1 | Iterator chains |
| A14 | Nested code | P1 | Early returns |
| A15 | Mutable statics | P1 | Atomics or DI |
| A16 | Float equality | P1 | Epsilon comparison |
| A17 | Unchecked indexing | P1 | .get() |
| A18 | Unawaited futures | P1 | await or spawn |

---

*Generated: 2026-01-28 | The Maintenance Engine v1.0.0*
