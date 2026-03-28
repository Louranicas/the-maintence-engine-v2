# M01 Error Taxonomy — error.rs

> **File:** `src/m1_foundation/error.rs` | **LOC:** ~1,530 | **Tests:** ~44
> **Role:** Unified error type for entire crate with classification, severity, and tensor signal mapping

---

## Error Enum (16 variants)

```rust
#[derive(Debug)]  // Clone + PartialEq: manual impls (std::io::Error not Clone/PartialEq)
pub enum Error { ... }
pub type Result<T> = std::result::Result<T, Error>;
```

| Variant | Code | Severity | Retryable | Transient | Category |
|---------|------|----------|-----------|-----------|----------|
| `Config(String)` | 1000 | Low | No | No | config |
| `Database(String)` | 1100 | Medium | Conditional* | No | database |
| `Network { target, message }` | 1200 | Medium | Yes | Yes | network |
| `CircuitOpen { service_id, retry_after_ms }` | 1201 | Medium | Yes | Yes | network |
| `Timeout { operation, timeout_ms }` | 1202 | Medium | Yes | Yes | network |
| `ConsensusQuorum { required, received }` | 1300 | High | Yes | Yes | consensus |
| `ViewChange { current_view, new_view }` | 1301 | Critical | No | No | consensus |
| `PathwayNotFound { source, target }` | 1400 | Low | No | No | learning |
| `TensorValidation { dimension, value }` | 1401 | Medium | No | No | learning |
| `Validation(String)` | 1500 | Low | No | No | validation |
| `Io(std::io::Error)` | 1600 | Medium | Conditional** | Conditional** | io |
| `Pipeline(String)` | 1700 | High | No | No | other |
| `ServiceNotFound(String)` | 1800 | Low | No | No | other |
| `HealthCheckFailed { service_id, reason }` | 1801 | High | No | No | other |
| `EscalationRequired { from_tier, to_tier, reason }` | 1802 | Critical | No | No | other |
| `Other(String)` | 1900 | Low | No | No | other |

`*Database`: retryable if message contains "locked" or "busy"
`**Io`: retryable for ConnectionRefused/Reset/TimedOut/Interrupted/WouldBlock; transient for TimedOut/WouldBlock/Interrupted

---

## Severity Enum

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity { Low, Medium, High, Critical }
```

Traits: `Display` ("LOW"/"MEDIUM"/"HIGH"/"CRITICAL"), ordered Low < Medium < High < Critical

---

## ErrorClassifier Trait

```rust
pub trait ErrorClassifier {
    fn is_retryable(&self) -> bool;
    fn is_transient(&self) -> bool;
    fn severity(&self) -> Severity;
    fn error_code(&self) -> u32 { 0 }                // default
    fn error_category(&self) -> &'static str { "other" }  // default
}
```

`Error` implements `ErrorClassifier` with all 5 methods populated (see table above).

---

## AnnotatedError

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct AnnotatedError {
    pub error: Error,
    pub origin: Option<AgentOrigin>,
    pub confidence: Confidence,
}
```

| Method | Signature | Notes |
|--------|-----------|-------|
| `new` | `const fn(error: Error) -> Self` | origin=None, confidence=certain() |
| `with_origin` | `fn(self, AgentOrigin) -> Self` | Builder chain |
| `with_confidence` | `const fn(self, Confidence) -> Self` | Builder chain |

Implements `std::error::Error` with `source()` → always `Some(&self.error)`

---

## Tensor Signal Mapping

```rust
impl Error {
    pub fn to_tensor_signal(&self) -> Tensor12D
    // D6 = health: Critical→0.1, High→0.3, Medium→0.5, Low→0.8
    // D2 = tier: maps error_category to tier weight
    // D10 = error_rate: Critical→0.9, High→0.7, Medium→0.5, Low→0.2
    // Output is clamp_normalized
}
```

---

## From Conversions

```rust
impl From<std::io::Error> for Error  → Error::Io(e)
impl From<String> for Error          → Error::Other(s)
```

---

## Manual Trait Implementations

- `Clone for Error`: deep clones all variants; `Io` variant clones via `io::Error::new(kind, to_string())`
- `PartialEq for Error`: `Io` compared by `kind() + to_string()`; all other variants by field equality
- `Eq for Error`: marker impl following manual `PartialEq`
- `std::error::Error for Error`: `source()` returns `Some(inner)` only for `Io` variant

---

*M01 Error Taxonomy Spec v1.0 | 2026-03-01*
