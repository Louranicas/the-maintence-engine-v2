//! # M20: gRPC Client
//!
//! Simulated gRPC (HTTP/2 binary RPC) communication client for the
//! Maintenance Engine. Manages service stubs, serialization metadata,
//! streaming state, and call history for high-performance binary RPC
//! with ULTRAPLATE services that expose gRPC interfaces.
//!
//! ## Layer: L4 (Integration)
//! ## Dependencies: M01 (Error), L4 mod.rs types
//!
//! ## Features
//!
//! - Service stub registration with proto metadata
//! - Unary, server-streaming, and bidirectional call simulation
//! - Per-service connection state management
//! - Call history with configurable capacity (500 entries)
//! - Retry policy with exponential backoff
//! - Latency and throughput metrics per stub
//!
//! ## Primary Consumer
//!
//! Tool Maker (port 8103) exposes a gRPC interface for binary tool
//! compilation requests. Default timeout: 3000ms.
//!
//! ## Related Documentation
//! - [Module Specification](../../ai_docs/modules/M20_GRPC_CLIENT.md)
//! - [Layer Specification](../../ai_docs/layers/L04_INTEGRATION.md)

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use uuid::Uuid;

use crate::{Error, Result};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Maximum number of call records retained per client.
const CALL_LOG_CAPACITY: usize = 500;

/// Default gRPC timeout in milliseconds.
const DEFAULT_TIMEOUT_MS: u64 = 3000;

/// Maximum retry attempts for transient failures.
const DEFAULT_MAX_RETRIES: u32 = 3;

/// Base delay for exponential backoff in milliseconds.
const BACKOFF_BASE_MS: u64 = 100;

/// Backoff multiplier per retry.
const BACKOFF_MULTIPLIER: f64 = 2.0;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// gRPC call type (matching protobuf service method kinds).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CallType {
    /// Single request, single response.
    Unary,
    /// Single request, stream of responses.
    ServerStreaming,
    /// Stream of requests, single response.
    ClientStreaming,
    /// Bidirectional stream.
    BidirectionalStreaming,
}

impl std::fmt::Display for CallType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unary => write!(f, "UNARY"),
            Self::ServerStreaming => write!(f, "SERVER_STREAMING"),
            Self::ClientStreaming => write!(f, "CLIENT_STREAMING"),
            Self::BidirectionalStreaming => write!(f, "BIDI_STREAMING"),
        }
    }
}

/// gRPC status code (subset of the official codes).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GrpcStatus {
    /// Success.
    Ok,
    /// Client cancelled the request.
    Cancelled,
    /// Unknown error.
    Unknown,
    /// Invalid argument from client.
    InvalidArgument,
    /// Deadline exceeded.
    DeadlineExceeded,
    /// Resource not found.
    NotFound,
    /// Resource already exists.
    AlreadyExists,
    /// Permission denied.
    PermissionDenied,
    /// Resource exhausted (rate limit, quota).
    ResourceExhausted,
    /// Server is unavailable.
    Unavailable,
    /// Internal server error.
    Internal,
    /// Not implemented.
    Unimplemented,
}

impl GrpcStatus {
    /// Whether this status represents a successful call.
    #[must_use]
    pub const fn is_ok(self) -> bool {
        matches!(self, Self::Ok)
    }

    /// Whether this status represents a retryable error.
    #[must_use]
    pub const fn is_retryable(self) -> bool {
        matches!(
            self,
            Self::Unavailable | Self::DeadlineExceeded | Self::ResourceExhausted
        )
    }

    /// Numeric code aligned with the gRPC specification.
    #[must_use]
    pub const fn code(self) -> u32 {
        match self {
            Self::Ok => 0,
            Self::Cancelled => 1,
            Self::Unknown => 2,
            Self::InvalidArgument => 3,
            Self::DeadlineExceeded => 4,
            Self::NotFound => 5,
            Self::AlreadyExists => 6,
            Self::PermissionDenied => 7,
            Self::ResourceExhausted => 8,
            Self::Unavailable => 14,
            Self::Internal => 13,
            Self::Unimplemented => 12,
        }
    }
}

/// Connection state for a gRPC channel to a remote service.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectionState {
    /// Not yet connected.
    Idle,
    /// Establishing connection.
    Connecting,
    /// Connected and ready.
    Ready,
    /// Transient failure, will retry.
    TransientFailure,
    /// Permanently shut down.
    Shutdown,
}

/// Metadata about a protobuf service definition.
#[derive(Clone, Debug)]
pub struct ProtoServiceDef {
    /// Fully-qualified service name (e.g. `ultraplate.tool_maker.v1.Compiler`).
    pub service_name: String,
    /// Package namespace.
    pub package: String,
    /// Known method names.
    pub methods: Vec<String>,
    /// Proto file version.
    pub proto_version: String,
}

/// A registered gRPC service stub.
#[derive(Clone, Debug)]
pub struct GrpcStub {
    /// ULTRAPLATE service ID (e.g. `tool-maker`).
    pub service_id: String,
    /// Remote host.
    pub host: String,
    /// Remote port.
    pub port: u16,
    /// Proto service definition.
    pub proto: ProtoServiceDef,
    /// Connection state.
    pub state: ConnectionState,
    /// Timeout per call in milliseconds.
    pub timeout_ms: u64,
    /// Maximum retries.
    pub max_retries: u32,
    /// Total calls made.
    pub total_calls: u64,
    /// Successful calls.
    pub successful_calls: u64,
    /// Failed calls.
    pub failed_calls: u64,
    /// Cumulative latency for average computation.
    pub cumulative_latency_ms: u64,
    /// Timestamp when the stub was registered.
    pub registered_at: DateTime<Utc>,
    /// Last call timestamp.
    pub last_call: Option<DateTime<Utc>>,
}

impl GrpcStub {
    /// Calculate average latency in milliseconds.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn average_latency_ms(&self) -> f64 {
        if self.total_calls == 0 {
            0.0
        } else {
            self.cumulative_latency_ms as f64 / self.total_calls as f64
        }
    }

    /// Calculate success rate.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn success_rate(&self) -> f64 {
        if self.total_calls == 0 {
            1.0
        } else {
            self.successful_calls as f64 / self.total_calls as f64
        }
    }
}

/// A record of a single gRPC call for audit and metrics.
#[derive(Clone, Debug)]
pub struct CallRecord {
    /// Unique call identifier.
    pub id: String,
    /// Service stub that was called.
    pub service_id: String,
    /// Method name invoked.
    pub method: String,
    /// Call type.
    pub call_type: CallType,
    /// gRPC status returned.
    pub status: GrpcStatus,
    /// Call duration in milliseconds.
    pub duration_ms: u64,
    /// Request payload size in bytes.
    pub request_bytes: usize,
    /// Response payload size in bytes.
    pub response_bytes: usize,
    /// Number of retry attempts used.
    pub retries: u32,
    /// Timestamp of the call.
    pub timestamp: DateTime<Utc>,
}

/// A simulated gRPC response.
#[derive(Clone, Debug)]
pub struct GrpcResponse {
    /// gRPC status.
    pub status: GrpcStatus,
    /// Serialized response payload.
    pub payload: Vec<u8>,
    /// Response metadata (trailing headers).
    pub metadata: HashMap<String, String>,
    /// Call duration in milliseconds.
    pub duration_ms: u64,
}

/// Retry policy configuration.
#[derive(Clone, Copy, Debug)]
pub struct RetryPolicy {
    /// Maximum retries.
    pub max_retries: u32,
    /// Base delay in milliseconds.
    pub base_delay_ms: u64,
    /// Backoff multiplier.
    pub multiplier: f64,
    /// Maximum delay cap in milliseconds.
    pub max_delay_ms: u64,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: DEFAULT_MAX_RETRIES,
            base_delay_ms: BACKOFF_BASE_MS,
            multiplier: BACKOFF_MULTIPLIER,
            max_delay_ms: 5000,
        }
    }
}

impl RetryPolicy {
    /// Calculate the delay for a given retry attempt (0-indexed).
    #[must_use]
    pub fn delay_for_attempt(&self, attempt: u32) -> u64 {
        // Safe: base_delay_ms is practically small (100-10000), fits in u32 losslessly.
        let base = f64::from(u32::try_from(self.base_delay_ms).unwrap_or(u32::MAX));
        // Cap exponent at 30 to avoid overflow; i32::try_from is safe for values <= 30.
        let exponent = i32::try_from(attempt.min(30)).unwrap_or(30);
        let delay = base * self.multiplier.powi(exponent);
        // Clamp to non-negative before converting to u64.
        let clamped = delay.max(0.0);
        // Safety: clamped is guaranteed non-negative by the .max(0.0) above.
        // Truncation is intentional: fractional milliseconds are irrelevant for delay.
        #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
        let delay_u64 = clamped as u64;
        delay_u64.min(self.max_delay_ms)
    }
}

// ---------------------------------------------------------------------------
// GrpcClient
// ---------------------------------------------------------------------------

/// gRPC client for communicating with ULTRAPLATE services.
///
/// Manages service stubs, simulates gRPC calls, tracks call history,
/// and provides connection state management.
pub struct GrpcClient {
    /// Registered service stubs keyed by service ID.
    stubs: RwLock<HashMap<String, GrpcStub>>,
    /// Call history (bounded).
    call_log: RwLock<Vec<CallRecord>>,
    /// Retry policy.
    retry_policy: RetryPolicy,
}

impl Default for GrpcClient {
    fn default() -> Self {
        Self::new()
    }
}

impl GrpcClient {
    /// Create a new gRPC client with the default retry policy.
    #[must_use]
    pub fn new() -> Self {
        Self {
            stubs: RwLock::new(HashMap::new()),
            call_log: RwLock::new(Vec::new()),
            retry_policy: RetryPolicy::default(),
        }
    }

    /// Create a new gRPC client with a custom retry policy.
    #[must_use]
    pub fn with_retry_policy(policy: RetryPolicy) -> Self {
        Self {
            stubs: RwLock::new(HashMap::new()),
            call_log: RwLock::new(Vec::new()),
            retry_policy: policy,
        }
    }

    /// Register a gRPC service stub.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if the service ID or host is empty,
    /// or if the port is zero.
    pub fn register_stub(
        &self,
        service_id: &str,
        host: &str,
        port: u16,
        proto: ProtoServiceDef,
    ) -> Result<()> {
        if service_id.is_empty() {
            return Err(Error::Validation("Service ID cannot be empty".into()));
        }
        if host.is_empty() {
            return Err(Error::Validation("Host cannot be empty".into()));
        }
        if port == 0 {
            return Err(Error::Validation("Port cannot be zero".into()));
        }

        let stub = GrpcStub {
            service_id: service_id.into(),
            host: host.into(),
            port,
            proto,
            state: ConnectionState::Idle,
            timeout_ms: DEFAULT_TIMEOUT_MS,
            max_retries: self.retry_policy.max_retries,
            total_calls: 0,
            successful_calls: 0,
            failed_calls: 0,
            cumulative_latency_ms: 0,
            registered_at: Utc::now(),
            last_call: None,
        };

        self.stubs.write().insert(service_id.into(), stub);
        Ok(())
    }

    /// Remove a service stub.
    ///
    /// # Errors
    ///
    /// Returns `Error::ServiceNotFound` if the stub does not exist.
    pub fn remove_stub(&self, service_id: &str) -> Result<()> {
        let removed = self.stubs.write().remove(service_id);
        if removed.is_some() {
            Ok(())
        } else {
            Err(Error::ServiceNotFound(service_id.into()))
        }
    }

    /// Get a snapshot of a registered stub.
    #[must_use]
    pub fn get_stub(&self, service_id: &str) -> Option<GrpcStub> {
        self.stubs.read().get(service_id).cloned()
    }

    /// Get all registered service IDs.
    #[must_use]
    pub fn stub_ids(&self) -> Vec<String> {
        self.stubs.read().keys().cloned().collect()
    }

    /// Get the number of registered stubs.
    #[must_use]
    pub fn stub_count(&self) -> usize {
        self.stubs.read().len()
    }

    /// Set connection state for a stub.
    ///
    /// # Errors
    ///
    /// Returns `Error::ServiceNotFound` if the stub does not exist.
    pub fn set_connection_state(
        &self,
        service_id: &str,
        state: ConnectionState,
    ) -> Result<()> {
        let mut stubs = self.stubs.write();
        let stub = stubs
            .get_mut(service_id)
            .ok_or_else(|| Error::ServiceNotFound(service_id.into()))?;
        stub.state = state;
        drop(stubs);
        Ok(())
    }

    /// Get the connection state for a stub.
    #[must_use]
    pub fn connection_state(&self, service_id: &str) -> Option<ConnectionState> {
        self.stubs.read().get(service_id).map(|s| s.state)
    }

    /// Simulate a gRPC call.
    ///
    /// Records the call in the log and updates stub metrics. In this
    /// simulated implementation, the call succeeds if the stub is in
    /// `Ready` state and the method exists in the proto definition.
    ///
    /// # Errors
    ///
    /// Returns `Error::ServiceNotFound` if the stub does not exist.
    /// Returns `Error::Network` if the stub is not in `Ready` state.
    pub fn call(
        &self,
        service_id: &str,
        method: &str,
        call_type: CallType,
        request_bytes: usize,
    ) -> Result<GrpcResponse> {
        let (state, method_exists) = {
            let stubs = self.stubs.read();
            let stub = stubs
                .get(service_id)
                .ok_or_else(|| Error::ServiceNotFound(service_id.into()))?;
            let result = (stub.state, stub.proto.methods.iter().any(|m| m == method));
            drop(stubs);
            result
        };

        let (status, duration_ms, response_bytes) = match state {
            ConnectionState::Ready => {
                if method_exists {
                    (GrpcStatus::Ok, 2, 64)
                } else {
                    (GrpcStatus::Unimplemented, 1, 0)
                }
            }
            ConnectionState::Shutdown => (GrpcStatus::Unavailable, 0, 0),
            ConnectionState::TransientFailure => (GrpcStatus::Unavailable, 1, 0),
            ConnectionState::Idle | ConnectionState::Connecting => {
                (GrpcStatus::Unavailable, 0, 0)
            }
        };

        let record = CallRecord {
            id: Uuid::new_v4().to_string(),
            service_id: service_id.into(),
            method: method.into(),
            call_type,
            status,
            duration_ms,
            request_bytes,
            response_bytes,
            retries: 0,
            timestamp: Utc::now(),
        };

        // Update stub metrics
        {
            let mut stubs = self.stubs.write();
            if let Some(stub) = stubs.get_mut(service_id) {
                stub.total_calls += 1;
                stub.cumulative_latency_ms += duration_ms;
                stub.last_call = Some(Utc::now());
                if status.is_ok() {
                    stub.successful_calls += 1;
                } else {
                    stub.failed_calls += 1;
                }
            }
        }

        // Append to call log
        {
            let mut log = self.call_log.write();
            if log.len() >= CALL_LOG_CAPACITY {
                let quarter = log.len() / 4;
                log.drain(..quarter);
            }
            log.push(record);
        }

        if status.is_ok() {
            Ok(GrpcResponse {
                status,
                payload: vec![0u8; response_bytes],
                metadata: HashMap::new(),
                duration_ms,
            })
        } else {
            Err(Error::Network {
                target: service_id.into(),
                message: format!("gRPC call failed: {status:?}"),
            })
        }
    }

    /// Get the number of recorded calls.
    #[must_use]
    pub fn call_count(&self) -> usize {
        self.call_log.read().len()
    }

    /// Get calls for a specific service.
    #[must_use]
    pub fn calls_for_service(&self, service_id: &str) -> Vec<CallRecord> {
        self.call_log
            .read()
            .iter()
            .filter(|r| r.service_id == service_id)
            .cloned()
            .collect()
    }

    /// Get calls by method name.
    #[must_use]
    pub fn calls_for_method(&self, method: &str) -> Vec<CallRecord> {
        self.call_log
            .read()
            .iter()
            .filter(|r| r.method == method)
            .cloned()
            .collect()
    }

    /// Get overall success rate across all stubs.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn overall_success_rate(&self) -> f64 {
        let (total, success) = {
            let stubs = self.stubs.read();
            let total: u64 = stubs.values().map(|s| s.total_calls).sum();
            let success: u64 = stubs.values().map(|s| s.successful_calls).sum();
            drop(stubs);
            (total, success)
        };
        if total == 0 {
            1.0
        } else {
            success as f64 / total as f64
        }
    }

    /// Get overall average latency across all stubs.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn overall_average_latency_ms(&self) -> f64 {
        let (total, cumulative) = {
            let stubs = self.stubs.read();
            let total: u64 = stubs.values().map(|s| s.total_calls).sum();
            let cumulative: u64 = stubs.values().map(|s| s.cumulative_latency_ms).sum();
            drop(stubs);
            (total, cumulative)
        };
        if total == 0 {
            0.0
        } else {
            cumulative as f64 / total as f64
        }
    }

    /// Set the timeout for a specific stub.
    ///
    /// # Errors
    ///
    /// Returns `Error::ServiceNotFound` if the stub does not exist.
    pub fn set_timeout(&self, service_id: &str, timeout_ms: u64) -> Result<()> {
        let mut stubs = self.stubs.write();
        let stub = stubs
            .get_mut(service_id)
            .ok_or_else(|| Error::ServiceNotFound(service_id.into()))?;
        stub.timeout_ms = timeout_ms;
        drop(stubs);
        Ok(())
    }

    /// Get the retry policy.
    #[must_use]
    pub const fn retry_policy(&self) -> &RetryPolicy {
        &self.retry_policy
    }

    /// Clear all call records.
    pub fn clear_call_log(&self) {
        self.call_log.write().clear();
    }

    /// Get a list of stubs in `TransientFailure` state.
    #[must_use]
    pub fn failed_stubs(&self) -> Vec<String> {
        self.stubs
            .read()
            .iter()
            .filter(|(_, s)| s.state == ConnectionState::TransientFailure)
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// Get a list of stubs in `Ready` state.
    #[must_use]
    pub fn ready_stubs(&self) -> Vec<String> {
        self.stubs
            .read()
            .iter()
            .filter(|(_, s)| s.state == ConnectionState::Ready)
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// Reset metrics for a specific stub.
    ///
    /// # Errors
    ///
    /// Returns `Error::ServiceNotFound` if the stub does not exist.
    pub fn reset_metrics(&self, service_id: &str) -> Result<()> {
        let mut stubs = self.stubs.write();
        let stub = stubs
            .get_mut(service_id)
            .ok_or_else(|| Error::ServiceNotFound(service_id.into()))?;
        stub.total_calls = 0;
        stub.successful_calls = 0;
        stub.failed_calls = 0;
        stub.cumulative_latency_ms = 0;
        stub.last_call = None;
        drop(stubs);
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Default stubs for ULTRAPLATE services with gRPC interfaces
// ---------------------------------------------------------------------------

/// Create default gRPC stubs for ULTRAPLATE services.
#[must_use]
pub fn default_grpc_stubs() -> Vec<(String, String, u16, ProtoServiceDef)> {
    vec![
        (
            "tool-maker".into(),
            "localhost".into(),
            8103,
            ProtoServiceDef {
                service_name: "ultraplate.tool_maker.v1.Compiler".into(),
                package: "ultraplate.tool_maker.v1".into(),
                methods: vec![
                    "CompileTool".into(),
                    "ValidateSpec".into(),
                    "StreamLogs".into(),
                    "GetStatus".into(),
                ],
                proto_version: "3".into(),
            },
        ),
        (
            "bash-engine".into(),
            "localhost".into(),
            8102,
            ProtoServiceDef {
                service_name: "ultraplate.bash_engine.v1.Executor".into(),
                package: "ultraplate.bash_engine.v1".into(),
                methods: vec![
                    "Execute".into(),
                    "StreamOutput".into(),
                    "Cancel".into(),
                    "GetHistory".into(),
                ],
                proto_version: "3".into(),
            },
        ),
    ]
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn test_proto() -> ProtoServiceDef {
        ProtoServiceDef {
            service_name: "test.v1.Service".into(),
            package: "test.v1".into(),
            methods: vec!["DoWork".into(), "StreamData".into(), "GetStatus".into()],
            proto_version: "3".into(),
        }
    }

    fn setup_client_with_stub() -> GrpcClient {
        let client = GrpcClient::new();
        client
            .register_stub("svc-a", "localhost", 9000, test_proto())
            .ok();
        client.set_connection_state("svc-a", ConnectionState::Ready).ok();
        client
    }

    #[test]
    fn test_register_stub() {
        let client = GrpcClient::new();
        assert!(client
            .register_stub("svc-a", "localhost", 9000, test_proto())
            .is_ok());
        assert_eq!(client.stub_count(), 1);
    }

    #[test]
    fn test_register_stub_empty_id_fails() {
        let client = GrpcClient::new();
        assert!(client.register_stub("", "localhost", 9000, test_proto()).is_err());
    }

    #[test]
    fn test_register_stub_empty_host_fails() {
        let client = GrpcClient::new();
        assert!(client.register_stub("svc", "", 9000, test_proto()).is_err());
    }

    #[test]
    fn test_register_stub_zero_port_fails() {
        let client = GrpcClient::new();
        assert!(client.register_stub("svc", "localhost", 0, test_proto()).is_err());
    }

    #[test]
    fn test_remove_stub() {
        let client = setup_client_with_stub();
        assert!(client.remove_stub("svc-a").is_ok());
        assert_eq!(client.stub_count(), 0);
    }

    #[test]
    fn test_remove_nonexistent_stub_fails() {
        let client = GrpcClient::new();
        assert!(client.remove_stub("nonexistent").is_err());
    }

    #[test]
    fn test_get_stub() {
        let client = setup_client_with_stub();
        let stub = client.get_stub("svc-a");
        assert!(stub.is_some());
        assert_eq!(stub.map(|s| s.port).unwrap_or(0), 9000);
    }

    #[test]
    fn test_get_nonexistent_stub() {
        let client = GrpcClient::new();
        assert!(client.get_stub("none").is_none());
    }

    #[test]
    fn test_stub_ids() {
        let client = GrpcClient::new();
        client.register_stub("a", "h", 1, test_proto()).ok();
        client.register_stub("b", "h", 2, test_proto()).ok();
        let ids = client.stub_ids();
        assert_eq!(ids.len(), 2);
    }

    #[test]
    fn test_set_connection_state() {
        let client = setup_client_with_stub();
        assert!(client.set_connection_state("svc-a", ConnectionState::Connecting).is_ok());
        assert_eq!(
            client.connection_state("svc-a"),
            Some(ConnectionState::Connecting)
        );
    }

    #[test]
    fn test_set_connection_state_nonexistent_fails() {
        let client = GrpcClient::new();
        assert!(client.set_connection_state("x", ConnectionState::Ready).is_err());
    }

    #[test]
    fn test_connection_state_none_for_missing() {
        let client = GrpcClient::new();
        assert!(client.connection_state("x").is_none());
    }

    #[test]
    fn test_call_success() {
        let client = setup_client_with_stub();
        let resp = client.call("svc-a", "DoWork", CallType::Unary, 32);
        assert!(resp.is_ok());
    }

    #[test]
    fn test_call_unimplemented_method() {
        let client = setup_client_with_stub();
        let resp = client.call("svc-a", "NoSuchMethod", CallType::Unary, 32);
        assert!(resp.is_err());
    }

    #[test]
    fn test_call_not_ready_fails() {
        let client = GrpcClient::new();
        client.register_stub("svc", "h", 1, test_proto()).ok();
        let resp = client.call("svc", "DoWork", CallType::Unary, 32);
        assert!(resp.is_err());
    }

    #[test]
    fn test_call_nonexistent_service_fails() {
        let client = GrpcClient::new();
        assert!(client.call("x", "m", CallType::Unary, 0).is_err());
    }

    #[test]
    fn test_call_records_in_log() {
        let client = setup_client_with_stub();
        let _ = client.call("svc-a", "DoWork", CallType::Unary, 32);
        assert_eq!(client.call_count(), 1);
    }

    #[test]
    fn test_call_updates_metrics() {
        let client = setup_client_with_stub();
        let _ = client.call("svc-a", "DoWork", CallType::Unary, 32);
        let stub = client.get_stub("svc-a");
        assert!(stub.is_some());
        let s = stub.unwrap_or_else(|| unreachable!());
        assert_eq!(s.total_calls, 1);
        assert_eq!(s.successful_calls, 1);
    }

    #[test]
    fn test_calls_for_service() {
        let client = setup_client_with_stub();
        client.register_stub("svc-b", "h", 2, test_proto()).ok();
        client.set_connection_state("svc-b", ConnectionState::Ready).ok();
        let _ = client.call("svc-a", "DoWork", CallType::Unary, 10);
        let _ = client.call("svc-b", "DoWork", CallType::Unary, 10);
        assert_eq!(client.calls_for_service("svc-a").len(), 1);
        assert_eq!(client.calls_for_service("svc-b").len(), 1);
    }

    #[test]
    fn test_calls_for_method() {
        let client = setup_client_with_stub();
        let _ = client.call("svc-a", "DoWork", CallType::Unary, 10);
        let _ = client.call("svc-a", "GetStatus", CallType::Unary, 10);
        assert_eq!(client.calls_for_method("DoWork").len(), 1);
    }

    #[test]
    fn test_overall_success_rate_initial() {
        let client = GrpcClient::new();
        assert!((client.overall_success_rate() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_overall_success_rate_after_calls() {
        let client = setup_client_with_stub();
        let _ = client.call("svc-a", "DoWork", CallType::Unary, 10);
        assert!((client.overall_success_rate() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_overall_average_latency() {
        let client = setup_client_with_stub();
        let _ = client.call("svc-a", "DoWork", CallType::Unary, 10);
        assert!(client.overall_average_latency_ms() >= 0.0);
    }

    #[test]
    fn test_overall_average_latency_no_calls() {
        let client = GrpcClient::new();
        assert!((client.overall_average_latency_ms()).abs() < f64::EPSILON);
    }

    #[test]
    fn test_set_timeout() {
        let client = setup_client_with_stub();
        assert!(client.set_timeout("svc-a", 10000).is_ok());
        let stub = client.get_stub("svc-a");
        assert_eq!(stub.map(|s| s.timeout_ms).unwrap_or(0), 10000);
    }

    #[test]
    fn test_set_timeout_nonexistent_fails() {
        let client = GrpcClient::new();
        assert!(client.set_timeout("x", 5000).is_err());
    }

    #[test]
    fn test_clear_call_log() {
        let client = setup_client_with_stub();
        let _ = client.call("svc-a", "DoWork", CallType::Unary, 10);
        client.clear_call_log();
        assert_eq!(client.call_count(), 0);
    }

    #[test]
    fn test_failed_stubs() {
        let client = GrpcClient::new();
        client.register_stub("a", "h", 1, test_proto()).ok();
        client.register_stub("b", "h", 2, test_proto()).ok();
        client.set_connection_state("a", ConnectionState::TransientFailure).ok();
        client.set_connection_state("b", ConnectionState::Ready).ok();
        assert_eq!(client.failed_stubs().len(), 1);
    }

    #[test]
    fn test_ready_stubs() {
        let client = GrpcClient::new();
        client.register_stub("a", "h", 1, test_proto()).ok();
        client.register_stub("b", "h", 2, test_proto()).ok();
        client.set_connection_state("a", ConnectionState::Ready).ok();
        assert_eq!(client.ready_stubs().len(), 1);
    }

    #[test]
    fn test_reset_metrics() {
        let client = setup_client_with_stub();
        let _ = client.call("svc-a", "DoWork", CallType::Unary, 10);
        assert!(client.reset_metrics("svc-a").is_ok());
        let stub = client.get_stub("svc-a");
        assert_eq!(stub.map(|s| s.total_calls).unwrap_or(1), 0);
    }

    #[test]
    fn test_reset_metrics_nonexistent_fails() {
        let client = GrpcClient::new();
        assert!(client.reset_metrics("x").is_err());
    }

    #[test]
    fn test_stub_average_latency() {
        let mut stub = GrpcStub {
            service_id: "t".into(),
            host: "h".into(),
            port: 1,
            proto: test_proto(),
            state: ConnectionState::Ready,
            timeout_ms: 3000,
            max_retries: 3,
            total_calls: 10,
            successful_calls: 8,
            failed_calls: 2,
            cumulative_latency_ms: 50,
            registered_at: Utc::now(),
            last_call: None,
        };
        assert!((stub.average_latency_ms() - 5.0).abs() < f64::EPSILON);
        stub.total_calls = 0;
        assert!((stub.average_latency_ms()).abs() < f64::EPSILON);
    }

    #[test]
    fn test_stub_success_rate() {
        let stub = GrpcStub {
            service_id: "t".into(),
            host: "h".into(),
            port: 1,
            proto: test_proto(),
            state: ConnectionState::Ready,
            timeout_ms: 3000,
            max_retries: 3,
            total_calls: 10,
            successful_calls: 7,
            failed_calls: 3,
            cumulative_latency_ms: 0,
            registered_at: Utc::now(),
            last_call: None,
        };
        assert!((stub.success_rate() - 0.7).abs() < f64::EPSILON);
    }

    #[test]
    fn test_grpc_status_is_ok() {
        assert!(GrpcStatus::Ok.is_ok());
        assert!(!GrpcStatus::Internal.is_ok());
    }

    #[test]
    fn test_grpc_status_is_retryable() {
        assert!(GrpcStatus::Unavailable.is_retryable());
        assert!(GrpcStatus::DeadlineExceeded.is_retryable());
        assert!(!GrpcStatus::NotFound.is_retryable());
    }

    #[test]
    fn test_grpc_status_code() {
        assert_eq!(GrpcStatus::Ok.code(), 0);
        assert_eq!(GrpcStatus::Internal.code(), 13);
        assert_eq!(GrpcStatus::Unavailable.code(), 14);
    }

    #[test]
    fn test_call_type_display() {
        assert_eq!(CallType::Unary.to_string(), "UNARY");
        assert_eq!(
            CallType::BidirectionalStreaming.to_string(),
            "BIDI_STREAMING"
        );
    }

    #[test]
    fn test_retry_policy_default() {
        let p = RetryPolicy::default();
        assert_eq!(p.max_retries, DEFAULT_MAX_RETRIES);
        assert_eq!(p.base_delay_ms, BACKOFF_BASE_MS);
    }

    #[test]
    fn test_retry_policy_delay_backoff() {
        let p = RetryPolicy::default();
        let d0 = p.delay_for_attempt(0);
        let d1 = p.delay_for_attempt(1);
        let d2 = p.delay_for_attempt(2);
        assert!(d1 > d0);
        assert!(d2 > d1);
    }

    #[test]
    fn test_retry_policy_delay_capped() {
        let p = RetryPolicy {
            max_delay_ms: 500,
            ..RetryPolicy::default()
        };
        assert!(p.delay_for_attempt(100) <= 500);
    }

    #[test]
    fn test_default_grpc_stubs() {
        let stubs = default_grpc_stubs();
        assert!(stubs.len() >= 2);
        assert!(stubs.iter().any(|(id, _, _, _)| id == "tool-maker"));
    }

    #[test]
    fn test_call_log_capacity_bounded() {
        let client = setup_client_with_stub();
        for _ in 0..600 {
            let _ = client.call("svc-a", "DoWork", CallType::Unary, 1);
        }
        assert!(client.call_count() <= CALL_LOG_CAPACITY);
    }

    #[test]
    fn test_multiple_stubs_independent() {
        let client = GrpcClient::new();
        client.register_stub("a", "h", 1, test_proto()).ok();
        client.register_stub("b", "h", 2, test_proto()).ok();
        client.set_connection_state("a", ConnectionState::Ready).ok();
        client.set_connection_state("b", ConnectionState::Shutdown).ok();
        assert!(client.call("a", "DoWork", CallType::Unary, 1).is_ok());
        assert!(client.call("b", "DoWork", CallType::Unary, 1).is_err());
    }

    #[test]
    fn test_server_streaming_call() {
        let client = setup_client_with_stub();
        let resp = client.call("svc-a", "StreamData", CallType::ServerStreaming, 16);
        assert!(resp.is_ok());
    }

    #[test]
    fn test_bidi_streaming_call() {
        let client = setup_client_with_stub();
        let resp = client.call(
            "svc-a",
            "StreamData",
            CallType::BidirectionalStreaming,
            16,
        );
        assert!(resp.is_ok());
    }

    #[test]
    fn test_with_custom_retry_policy() {
        let policy = RetryPolicy {
            max_retries: 5,
            base_delay_ms: 200,
            multiplier: 3.0,
            max_delay_ms: 10000,
        };
        let client = GrpcClient::with_retry_policy(policy);
        assert_eq!(client.retry_policy().max_retries, 5);
    }

    #[test]
    fn test_shutdown_state_call_fails() {
        let client = GrpcClient::new();
        client.register_stub("svc", "h", 1, test_proto()).ok();
        client.set_connection_state("svc", ConnectionState::Shutdown).ok();
        assert!(client.call("svc", "DoWork", CallType::Unary, 10).is_err());
    }

    #[test]
    fn test_connection_state_idle_initial() {
        let client = GrpcClient::new();
        client.register_stub("svc", "h", 1, test_proto()).ok();
        assert_eq!(
            client.connection_state("svc"),
            Some(ConnectionState::Idle)
        );
    }

    #[test]
    fn test_response_payload_not_empty_on_success() {
        let client = setup_client_with_stub();
        let resp = client.call("svc-a", "DoWork", CallType::Unary, 32);
        if let Ok(r) = resp {
            assert!(!r.payload.is_empty());
        }
    }

    #[test]
    fn test_failed_call_increments_failure_count() {
        let client = GrpcClient::new();
        client.register_stub("svc", "h", 1, test_proto()).ok();
        // Idle state → call fails
        let _ = client.call("svc", "DoWork", CallType::Unary, 10);
        let stub = client.get_stub("svc");
        assert_eq!(stub.map(|s| s.failed_calls).unwrap_or(0), 1);
    }
}
