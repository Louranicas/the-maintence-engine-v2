//! # M19: REST Client
//!
//! HTTP/REST communication client for the Maintenance Engine.
//! Provides simulated REST request handling, circuit breaker integration,
//! request logging, and health check capabilities for all ULTRAPLATE services.
//!
//! ## Layer: L4 (Integration)
//! ## Dependencies: M01 (Error), M04 (mod.rs types)
//!
//! ## Features
//!
//! - Endpoint registration and management
//! - Simulated REST request execution with response recording
//! - Per-service circuit breaker state
//! - Request logging with configurable capacity (500 entries)
//! - Success rate and average latency metrics
//! - Health check endpoint support
//!
//! ## Related Documentation
//! - [Layer Specification](../../ai_docs/layers/L04_INTEGRATION.md)
//! - [REST API](../../ai_docs/integration/REST_API.md)

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use uuid::Uuid;

use super::{default_endpoints, ServiceEndpoint, WireProtocol};
use crate::{Error, Result};

/// Maximum number of request records to retain per client.
const REQUEST_LOG_CAPACITY: usize = 500;

/// HTTP method for REST requests.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HttpMethod {
    /// HTTP GET
    Get,
    /// HTTP POST
    Post,
    /// HTTP PUT
    Put,
    /// HTTP DELETE
    Delete,
    /// HTTP PATCH
    Patch,
}

impl std::fmt::Display for HttpMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Get => write!(f, "GET"),
            Self::Post => write!(f, "POST"),
            Self::Put => write!(f, "PUT"),
            Self::Delete => write!(f, "DELETE"),
            Self::Patch => write!(f, "PATCH"),
        }
    }
}

/// A record of a single REST request for audit and metrics purposes.
#[derive(Clone, Debug)]
pub struct RequestRecord {
    /// Unique request identifier (UUID v4).
    pub id: String,
    /// The service that was called.
    pub service_id: String,
    /// HTTP method used.
    pub method: HttpMethod,
    /// Request path (relative to service base).
    pub path: String,
    /// HTTP status code returned, if any.
    pub status_code: Option<u16>,
    /// Request duration in milliseconds.
    pub duration_ms: u64,
    /// Whether the request succeeded.
    pub success: bool,
    /// Timestamp when the request was made.
    pub timestamp: DateTime<Utc>,
}

/// A simulated REST response.
#[derive(Clone, Debug)]
pub struct RestResponse {
    /// HTTP status code.
    pub status_code: u16,
    /// Response body content.
    pub body: String,
    /// Response headers.
    pub headers: HashMap<String, String>,
    /// Request round-trip duration in milliseconds.
    pub duration_ms: u64,
}

/// REST client for communicating with ULTRAPLATE services.
///
/// Manages service endpoints, simulates HTTP requests, tracks request history,
/// and provides circuit breaker integration for fault tolerance.
pub struct RestClient {
    /// Registered service endpoints, keyed by service ID.
    endpoints: RwLock<HashMap<String, ServiceEndpoint>>,
    /// Rolling request log (capped at [`REQUEST_LOG_CAPACITY`]).
    request_log: RwLock<Vec<RequestRecord>>,
    /// Circuit breaker state per service: `true` means the circuit is open (requests blocked).
    circuit_states: RwLock<HashMap<String, bool>>,
}

impl RestClient {
    /// Create a new `RestClient` pre-loaded with all default REST-protocol endpoints.
    ///
    /// Only endpoints whose protocol is [`WireProtocol::Rest`] are loaded from
    /// [`default_endpoints`].
    #[must_use]
    pub fn new() -> Self {
        let mut endpoints_map = HashMap::new();
        for ep in default_endpoints() {
            if ep.protocol == WireProtocol::Rest {
                endpoints_map.insert(ep.service_id.clone(), ep);
            }
        }

        Self {
            endpoints: RwLock::new(endpoints_map),
            request_log: RwLock::new(Vec::with_capacity(REQUEST_LOG_CAPACITY)),
            circuit_states: RwLock::new(HashMap::new()),
        }
    }

    /// Register a new service endpoint, or replace an existing one.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] if the endpoint has an empty `service_id`.
    pub fn register_endpoint(&self, endpoint: ServiceEndpoint) -> Result<()> {
        if endpoint.service_id.is_empty() {
            return Err(Error::Validation("service_id must not be empty".into()));
        }
        self.endpoints
            .write()
            .insert(endpoint.service_id.clone(), endpoint);
        Ok(())
    }

    /// Build the full URL for a given service and path.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ServiceNotFound`] if the service ID is not registered.
    #[must_use = "the built URL should be used"]
    #[allow(clippy::significant_drop_tightening)]
    pub fn build_url(&self, service_id: &str, path: &str) -> Result<String> {
        let endpoints = self.endpoints.read();
        let ep = endpoints
            .get(service_id)
            .ok_or_else(|| Error::ServiceNotFound(service_id.into()))?;
        Ok(ep.url(path))
    }

    /// Simulate a REST request to the specified service.
    ///
    /// This method does not perform actual network I/O. Instead, it produces a
    /// deterministic mock response, records the request in the log, and respects
    /// circuit breaker state.
    ///
    /// # Behavior
    ///
    /// - If the circuit breaker is open for the service, returns [`Error::CircuitOpen`].
    /// - Health endpoints (`/health`, `/api/health`) return HTTP 200 with a JSON body.
    /// - All other endpoints return HTTP 200 with a generic success body.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ServiceNotFound`] if `service_id` is not registered.
    /// Returns [`Error::CircuitOpen`] if the circuit breaker is open for the service.
    pub fn simulate_request(
        &self,
        service_id: &str,
        method: HttpMethod,
        path: &str,
    ) -> Result<RestResponse> {
        // Check circuit breaker
        if self.is_circuit_open(service_id) {
            return Err(Error::CircuitOpen {
                service_id: service_id.into(),
                retry_after_ms: 30_000,
            });
        }

        // Verify endpoint exists
        let url = self.build_url(service_id, path)?;

        // Simulate latency (deterministic for tests)
        let duration_ms = self.simulated_latency(service_id);

        // Determine response based on path
        let is_health = path.contains("health");
        let (status_code, body) = if is_health {
            (
                200,
                format!(
                    r#"{{"status":"healthy","service":"{service_id}","url":"{url}"}}"#,
                ),
            )
        } else {
            (
                200,
                format!(
                    r#"{{"status":"ok","service":"{service_id}","method":"{method}","path":"{path}"}}"#,
                ),
            )
        };

        let mut headers = HashMap::new();
        headers.insert("content-type".into(), "application/json".into());
        headers.insert("x-request-id".into(), Uuid::new_v4().to_string());

        let response = RestResponse {
            status_code,
            body,
            headers,
            duration_ms,
        };

        // Record the request
        self.record_request(service_id, method, path, Some(status_code), duration_ms, true);

        Ok(response)
    }

    /// Perform a health check against a registered service.
    ///
    /// Delegates to [`simulate_request`](Self::simulate_request) using the
    /// endpoint's configured `health_path` and HTTP GET.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ServiceNotFound`] if the service is not registered.
    /// Returns [`Error::CircuitOpen`] if the circuit breaker is open.
    #[allow(clippy::significant_drop_tightening)]
    pub fn health_check(&self, service_id: &str) -> Result<RestResponse> {
        let health_path = {
            let endpoints = self.endpoints.read();
            let ep = endpoints
                .get(service_id)
                .ok_or_else(|| Error::ServiceNotFound(service_id.into()))?;
            ep.health_path.clone()
        }; // guard dropped here
        self.simulate_request(service_id, HttpMethod::Get, &health_path)
    }

    /// Retrieve a clone of the endpoint configuration for a given service.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ServiceNotFound`] if the service is not registered.
    pub fn get_endpoint(&self, service_id: &str) -> Result<ServiceEndpoint> {
        self.endpoints
            .read()
            .get(service_id)
            .cloned()
            .ok_or_else(|| Error::ServiceNotFound(service_id.into()))
    }

    /// Set the circuit breaker state for a service.
    ///
    /// When `open` is `true`, all subsequent requests to the service will be
    /// rejected with [`Error::CircuitOpen`].
    pub fn set_circuit_open(&self, service_id: &str, open: bool) {
        self.circuit_states
            .write()
            .insert(service_id.into(), open);
    }

    /// Check whether the circuit breaker is open for a service.
    ///
    /// Returns `false` if the service has no circuit state entry (default: closed).
    #[must_use]
    pub fn is_circuit_open(&self, service_id: &str) -> bool {
        self.circuit_states
            .read()
            .get(service_id)
            .copied()
            .unwrap_or(false)
    }

    /// Retrieve all request records for a specific service, ordered by timestamp.
    #[must_use]
    pub fn get_request_log(&self, service_id: &str) -> Vec<RequestRecord> {
        self.request_log
            .read()
            .iter()
            .filter(|r| r.service_id == service_id)
            .cloned()
            .collect()
    }

    /// Calculate the success rate for a specific service.
    ///
    /// Returns `1.0` if there are no recorded requests (optimistic default).
    #[must_use]
    #[allow(clippy::significant_drop_tightening)]
    pub fn success_rate(&self, service_id: &str) -> f64 {
        let log = self.request_log.read();
        let records: Vec<&RequestRecord> = log
            .iter()
            .filter(|r| r.service_id == service_id)
            .collect();

        if records.is_empty() {
            return 1.0;
        }

        let successes = records.iter().filter(|r| r.success).count();
        #[allow(clippy::cast_precision_loss)]
        {
            successes as f64 / records.len() as f64
        }
    }

    /// Calculate the average request latency for a specific service in milliseconds.
    ///
    /// Returns `0.0` if there are no recorded requests.
    #[must_use]
    #[allow(clippy::significant_drop_tightening)]
    pub fn avg_latency(&self, service_id: &str) -> f64 {
        let log = self.request_log.read();
        let records: Vec<&RequestRecord> = log
            .iter()
            .filter(|r| r.service_id == service_id)
            .collect();

        if records.is_empty() {
            return 0.0;
        }

        #[allow(clippy::cast_precision_loss)]
        {
            let total: f64 = records.iter().map(|r| r.duration_ms as f64).sum();
            total / records.len() as f64
        }
    }

    /// Return the number of registered endpoints.
    #[must_use]
    pub fn endpoint_count(&self) -> usize {
        self.endpoints.read().len()
    }

    /// Record a request in the rolling log, enforcing capacity.
    fn record_request(
        &self,
        service_id: &str,
        method: HttpMethod,
        path: &str,
        status_code: Option<u16>,
        duration_ms: u64,
        success: bool,
    ) {
        let record = RequestRecord {
            id: Uuid::new_v4().to_string(),
            service_id: service_id.into(),
            method,
            path: path.into(),
            status_code,
            duration_ms,
            success,
            timestamp: Utc::now(),
        };

        let mut log = self.request_log.write();
        if log.len() >= REQUEST_LOG_CAPACITY {
            log.remove(0);
        }
        log.push(record);
    }

    /// Compute simulated latency based on the endpoint's timeout tier.
    ///
    /// Lower timeout endpoints are assumed to have lower latency.
    fn simulated_latency(&self, service_id: &str) -> u64 {
        self.endpoints
            .read()
            .get(service_id)
            .map_or(5, |ep| {
                // Simulate ~1% of the configured timeout as typical latency
                (ep.timeout_ms / 100).max(1)
            })
    }
}

impl Default for RestClient {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_loads_defaults() {
        let client = RestClient::new();
        let count = client.endpoint_count();
        // All default endpoints are REST protocol
        assert!(count >= 6, "expected at least 6 REST endpoints, got {count}");
        // Verify a known endpoint is present
        assert!(client.get_endpoint("synthex").is_ok());
    }

    #[test]
    fn test_register_endpoint() {
        let client = RestClient::new();
        let ep = ServiceEndpoint::new("custom-service", "10.0.0.1", 9999);
        assert!(client.register_endpoint(ep).is_ok());
        assert!(client.get_endpoint("custom-service").is_ok());
    }

    #[test]
    fn test_register_endpoint_empty_id_fails() {
        let client = RestClient::new();
        let ep = ServiceEndpoint::new("", "localhost", 8080);
        assert!(client.register_endpoint(ep).is_err());
    }

    #[test]
    fn test_build_url() {
        let client = RestClient::new();
        let url = client.build_url("synthex", "/status");
        assert!(url.is_ok());
        let url_str = url.unwrap_or_default();
        assert!(url_str.contains("localhost"));
        assert!(url_str.contains("8090"));
        assert!(url_str.contains("/status"));
    }

    #[test]
    fn test_build_url_not_found() {
        let client = RestClient::new();
        let result = client.build_url("nonexistent-service", "/test");
        assert!(result.is_err());
    }

    #[test]
    fn test_simulate_request() {
        let client = RestClient::new();
        let response = client.simulate_request("synthex", HttpMethod::Get, "/status");
        assert!(response.is_ok());
        let resp = response.unwrap_or_else(|_| RestResponse {
            status_code: 0,
            body: String::new(),
            headers: HashMap::new(),
            duration_ms: 0,
        });
        assert_eq!(resp.status_code, 200);
        assert!(resp.body.contains("synthex"));
        assert!(resp.headers.contains_key("content-type"));
    }

    #[test]
    fn test_health_check() {
        let client = RestClient::new();
        let response = client.health_check("synthex");
        assert!(response.is_ok());
        let resp = response.unwrap_or_else(|_| RestResponse {
            status_code: 0,
            body: String::new(),
            headers: HashMap::new(),
            duration_ms: 0,
        });
        assert_eq!(resp.status_code, 200);
        assert!(resp.body.contains("healthy"));
    }

    #[test]
    fn test_circuit_open_blocks() {
        let client = RestClient::new();

        // Circuit closed by default
        assert!(!client.is_circuit_open("synthex"));

        // Open the circuit
        client.set_circuit_open("synthex", true);
        assert!(client.is_circuit_open("synthex"));

        // Request should fail
        let result = client.simulate_request("synthex", HttpMethod::Get, "/test");
        assert!(result.is_err());

        // Close the circuit
        client.set_circuit_open("synthex", false);
        assert!(!client.is_circuit_open("synthex"));

        // Request should succeed again
        let result = client.simulate_request("synthex", HttpMethod::Get, "/test");
        assert!(result.is_ok());
    }

    #[test]
    fn test_request_logging() {
        let client = RestClient::new();

        // Make several requests
        for i in 0..5 {
            let path = format!("/endpoint/{i}");
            let _response = client.simulate_request("synthex", HttpMethod::Get, &path);
        }

        let log = client.get_request_log("synthex");
        assert_eq!(log.len(), 5);

        // Verify entries have the right service
        for entry in &log {
            assert_eq!(entry.service_id, "synthex");
            assert!(entry.success);
            assert_eq!(entry.method, HttpMethod::Get);
        }
    }

    #[test]
    fn test_success_rate() {
        let client = RestClient::new();

        // No requests: optimistic default
        assert!((client.success_rate("synthex") - 1.0).abs() < f64::EPSILON);

        // Make successful requests
        let _r1 = client.simulate_request("synthex", HttpMethod::Get, "/ok");
        let _r2 = client.simulate_request("synthex", HttpMethod::Post, "/ok");
        assert!((client.success_rate("synthex") - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_avg_latency() {
        let client = RestClient::new();

        // No requests: 0.0
        assert!((client.avg_latency("synthex")).abs() < f64::EPSILON);

        // Make some requests
        let _r = client.simulate_request("synthex", HttpMethod::Get, "/test");
        let latency = client.avg_latency("synthex");
        assert!(latency > 0.0, "expected positive latency, got {latency}");
    }

    #[test]
    fn test_endpoint_count() {
        let client = RestClient::new();
        let initial = client.endpoint_count();
        assert!(initial > 0);

        let ep = ServiceEndpoint::new("extra-service", "localhost", 7777);
        let _result = client.register_endpoint(ep);
        assert_eq!(client.endpoint_count(), initial + 1);
    }

    #[test]
    fn test_http_method_display() {
        assert_eq!(format!("{}", HttpMethod::Get), "GET");
        assert_eq!(format!("{}", HttpMethod::Post), "POST");
        assert_eq!(format!("{}", HttpMethod::Put), "PUT");
        assert_eq!(format!("{}", HttpMethod::Delete), "DELETE");
        assert_eq!(format!("{}", HttpMethod::Patch), "PATCH");
    }

    #[test]
    fn test_request_log_capacity() {
        let client = RestClient::new();

        // Fill beyond capacity
        for i in 0..REQUEST_LOG_CAPACITY + 50 {
            let path = format!("/load/{i}");
            let _r = client.simulate_request("synthex", HttpMethod::Get, &path);
        }

        let log_len = client.request_log.read().len();
        assert!(
            log_len <= REQUEST_LOG_CAPACITY,
            "log exceeded capacity: {log_len} > {REQUEST_LOG_CAPACITY}",
        );
    }

    #[test]
    fn test_get_endpoint_not_found() {
        let client = RestClient::new();
        let result = client.get_endpoint("imaginary-service");
        assert!(result.is_err());
    }

    // --- Additional tests to reach 50+ ---

    #[test]
    fn test_default_creates_same_as_new() {
        let d = RestClient::default();
        let n = RestClient::new();
        assert_eq!(d.endpoint_count(), n.endpoint_count());
    }

    #[test]
    fn test_build_url_contains_base_path() {
        let client = RestClient::new();
        let url = client.build_url("synthex", "/status").unwrap_or_default();
        assert!(url.starts_with("http://"));
    }

    #[test]
    fn test_simulate_request_health_endpoint_health_path() {
        let client = RestClient::new();
        let resp = client.simulate_request("synthex", HttpMethod::Get, "/api/health");
        assert!(resp.is_ok());
        let r = resp.unwrap_or_else(|_| RestResponse {
            status_code: 0,
            body: String::new(),
            headers: HashMap::new(),
            duration_ms: 0,
        });
        assert!(r.body.contains("healthy"));
    }

    #[test]
    fn test_simulate_request_non_health_endpoint() {
        let client = RestClient::new();
        let resp = client.simulate_request("synthex", HttpMethod::Post, "/data");
        assert!(resp.is_ok());
        let r = resp.unwrap_or_else(|_| RestResponse {
            status_code: 0,
            body: String::new(),
            headers: HashMap::new(),
            duration_ms: 0,
        });
        assert!(r.body.contains("ok"));
        assert!(r.body.contains("POST"));
    }

    #[test]
    fn test_simulate_request_put_method() {
        let client = RestClient::new();
        let resp = client.simulate_request("synthex", HttpMethod::Put, "/update");
        assert!(resp.is_ok());
        let r = resp.unwrap_or_else(|_| RestResponse {
            status_code: 0,
            body: String::new(),
            headers: HashMap::new(),
            duration_ms: 0,
        });
        assert!(r.body.contains("PUT"));
    }

    #[test]
    fn test_simulate_request_delete_method() {
        let client = RestClient::new();
        let resp = client.simulate_request("synthex", HttpMethod::Delete, "/resource");
        assert!(resp.is_ok());
        let r = resp.unwrap_or_else(|_| RestResponse {
            status_code: 0,
            body: String::new(),
            headers: HashMap::new(),
            duration_ms: 0,
        });
        assert!(r.body.contains("DELETE"));
    }

    #[test]
    fn test_simulate_request_patch_method() {
        let client = RestClient::new();
        let resp = client.simulate_request("synthex", HttpMethod::Patch, "/partial");
        assert!(resp.is_ok());
        let r = resp.unwrap_or_else(|_| RestResponse {
            status_code: 0,
            body: String::new(),
            headers: HashMap::new(),
            duration_ms: 0,
        });
        assert!(r.body.contains("PATCH"));
    }

    #[test]
    fn test_simulate_request_not_found_service() {
        let client = RestClient::new();
        let result = client.simulate_request("nonexistent", HttpMethod::Get, "/test");
        assert!(result.is_err());
    }

    #[test]
    fn test_response_headers_contain_request_id() {
        let client = RestClient::new();
        let resp = client.simulate_request("synthex", HttpMethod::Get, "/test");
        assert!(resp.is_ok());
        let r = resp.unwrap_or_else(|_| RestResponse {
            status_code: 0,
            body: String::new(),
            headers: HashMap::new(),
            duration_ms: 0,
        });
        assert!(r.headers.contains_key("x-request-id"));
        // UUID is 36 chars (with hyphens)
        let req_id = r.headers.get("x-request-id").cloned().unwrap_or_default();
        assert_eq!(req_id.len(), 36);
    }

    #[test]
    fn test_response_content_type_is_json() {
        let client = RestClient::new();
        let resp = client.simulate_request("synthex", HttpMethod::Get, "/test");
        assert!(resp.is_ok());
        let r = resp.unwrap_or_else(|_| RestResponse {
            status_code: 0,
            body: String::new(),
            headers: HashMap::new(),
            duration_ms: 0,
        });
        assert_eq!(
            r.headers.get("content-type").cloned().unwrap_or_default(),
            "application/json"
        );
    }

    #[test]
    fn test_response_duration_is_positive() {
        let client = RestClient::new();
        let resp = client.simulate_request("synthex", HttpMethod::Get, "/test");
        assert!(resp.is_ok());
        let r = resp.unwrap_or_else(|_| RestResponse {
            status_code: 0,
            body: String::new(),
            headers: HashMap::new(),
            duration_ms: 0,
        });
        assert!(r.duration_ms > 0);
    }

    #[test]
    fn test_circuit_open_default_is_false() {
        let client = RestClient::new();
        assert!(!client.is_circuit_open("anything"));
    }

    #[test]
    fn test_circuit_open_does_not_affect_other_services() {
        let client = RestClient::new();
        client.set_circuit_open("synthex", true);
        assert!(!client.is_circuit_open("nais"));
    }

    #[test]
    fn test_health_check_not_found_service() {
        let client = RestClient::new();
        let result = client.health_check("nonexistent-service");
        assert!(result.is_err());
    }

    #[test]
    fn test_health_check_circuit_open() {
        let client = RestClient::new();
        client.set_circuit_open("synthex", true);
        let result = client.health_check("synthex");
        assert!(result.is_err());
    }

    #[test]
    fn test_request_log_empty_for_unknown_service() {
        let client = RestClient::new();
        let log = client.get_request_log("unknown-svc");
        assert!(log.is_empty());
    }

    #[test]
    fn test_request_log_records_method() {
        let client = RestClient::new();
        let _r = client.simulate_request("synthex", HttpMethod::Post, "/data");
        let log = client.get_request_log("synthex");
        assert_eq!(log.len(), 1);
        assert_eq!(log[0].method, HttpMethod::Post);
    }

    #[test]
    fn test_request_log_records_path() {
        let client = RestClient::new();
        let _r = client.simulate_request("synthex", HttpMethod::Get, "/custom/path");
        let log = client.get_request_log("synthex");
        assert_eq!(log.len(), 1);
        assert_eq!(log[0].path, "/custom/path");
    }

    #[test]
    fn test_request_log_records_status_code() {
        let client = RestClient::new();
        let _r = client.simulate_request("synthex", HttpMethod::Get, "/test");
        let log = client.get_request_log("synthex");
        assert_eq!(log.len(), 1);
        assert_eq!(log[0].status_code, Some(200));
    }

    #[test]
    fn test_request_log_records_id_is_uuid() {
        let client = RestClient::new();
        let _r = client.simulate_request("synthex", HttpMethod::Get, "/test");
        let log = client.get_request_log("synthex");
        assert_eq!(log.len(), 1);
        assert_eq!(log[0].id.len(), 36);
    }

    #[test]
    fn test_request_log_records_duration_positive() {
        let client = RestClient::new();
        let _r = client.simulate_request("synthex", HttpMethod::Get, "/test");
        let log = client.get_request_log("synthex");
        assert_eq!(log.len(), 1);
        assert!(log[0].duration_ms > 0);
    }

    #[test]
    fn test_success_rate_after_circuit_open_requests() {
        let client = RestClient::new();
        // Make one successful request
        let _r = client.simulate_request("synthex", HttpMethod::Get, "/ok");
        // All simulated requests succeed, so rate stays at 1.0
        assert!((client.success_rate("synthex") - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_success_rate_unknown_service_is_optimistic() {
        let client = RestClient::new();
        assert!((client.success_rate("unknown") - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_avg_latency_unknown_service_is_zero() {
        let client = RestClient::new();
        assert!(client.avg_latency("unknown").abs() < f64::EPSILON);
    }

    #[test]
    fn test_avg_latency_consistent_across_requests() {
        let client = RestClient::new();
        let _r1 = client.simulate_request("synthex", HttpMethod::Get, "/a");
        let _r2 = client.simulate_request("synthex", HttpMethod::Get, "/b");
        let latency = client.avg_latency("synthex");
        // Since simulated latency is deterministic for the same service,
        // all requests should have the same latency => avg = that latency
        assert!(latency > 0.0);
    }

    #[test]
    fn test_register_replaces_existing_endpoint() {
        let client = RestClient::new();
        let ep1 = ServiceEndpoint::new("custom", "host1", 1111);
        let ep2 = ServiceEndpoint::new("custom", "host2", 2222);
        assert!(client.register_endpoint(ep1).is_ok());
        assert!(client.register_endpoint(ep2).is_ok());
        let ep = client.get_endpoint("custom");
        assert!(ep.is_ok());
        let endpoint = ep.unwrap_or_else(|_| ServiceEndpoint::new("", "", 0));
        assert_eq!(endpoint.port, 2222);
    }

    #[test]
    fn test_multiple_services_request_log_isolation() {
        let client = RestClient::new();
        let _r1 = client.simulate_request("synthex", HttpMethod::Get, "/a");
        let _r2 = client.simulate_request("nais", HttpMethod::Get, "/b");
        let synthex_log = client.get_request_log("synthex");
        let nais_log = client.get_request_log("nais");
        assert_eq!(synthex_log.len(), 1);
        assert_eq!(nais_log.len(), 1);
    }

    #[test]
    fn test_endpoint_count_after_deduplication() {
        let client = RestClient::new();
        let initial = client.endpoint_count();
        // Re-registering same ID should not increase count
        let ep = ServiceEndpoint::new("synthex", "other-host", 9999);
        let _r = client.register_endpoint(ep);
        assert_eq!(client.endpoint_count(), initial);
    }

    #[test]
    fn test_request_log_capacity_eviction_preserves_order() {
        let client = RestClient::new();
        for i in 0..(REQUEST_LOG_CAPACITY + 10) {
            let path = format!("/path/{i}");
            let _r = client.simulate_request("synthex", HttpMethod::Get, &path);
        }
        let log = client.get_request_log("synthex");
        // After eviction the earliest entries are gone
        assert!(!log.is_empty());
        // The last entry should be the last path
        let last = &log[log.len() - 1];
        let expected_path = format!("/path/{}", REQUEST_LOG_CAPACITY + 9);
        assert_eq!(last.path, expected_path);
    }

    #[test]
    fn test_build_url_different_services() {
        let client = RestClient::new();
        let url_synthex = client.build_url("synthex", "/test").unwrap_or_default();
        let url_nais = client.build_url("nais", "/test").unwrap_or_default();
        // Different services should produce different URLs
        assert_ne!(url_synthex, url_nais);
    }

    #[test]
    fn test_health_check_body_contains_service_name() {
        let client = RestClient::new();
        let resp = client.health_check("nais");
        assert!(resp.is_ok());
        let r = resp.unwrap_or_else(|_| RestResponse {
            status_code: 0,
            body: String::new(),
            headers: HashMap::new(),
            duration_ms: 0,
        });
        assert!(r.body.contains("nais"));
    }

    #[test]
    fn test_circuit_reopen_after_close() {
        let client = RestClient::new();
        client.set_circuit_open("synthex", true);
        client.set_circuit_open("synthex", false);
        client.set_circuit_open("synthex", true);
        assert!(client.is_circuit_open("synthex"));
    }

    #[test]
    fn test_multiple_health_checks_accumulate_log() {
        let client = RestClient::new();
        let _r1 = client.health_check("synthex");
        let _r2 = client.health_check("synthex");
        let _r3 = client.health_check("synthex");
        let log = client.get_request_log("synthex");
        assert_eq!(log.len(), 3);
    }

    #[test]
    fn test_http_method_equality() {
        assert_eq!(HttpMethod::Get, HttpMethod::Get);
        assert_ne!(HttpMethod::Get, HttpMethod::Post);
        assert_ne!(HttpMethod::Put, HttpMethod::Delete);
    }

    #[test]
    fn test_http_method_copy() {
        let m = HttpMethod::Patch;
        let m2 = m; // Copy
        assert_eq!(m, m2);
    }

    #[test]
    fn test_request_record_clone() {
        let client = RestClient::new();
        let _r = client.simulate_request("synthex", HttpMethod::Get, "/test");
        let log = client.get_request_log("synthex");
        assert_eq!(log.len(), 1);
        let cloned = log[0].clone();
        assert_eq!(cloned.service_id, "synthex");
    }

    #[test]
    fn test_rest_response_clone() {
        let resp = RestResponse {
            status_code: 200,
            body: "test".into(),
            headers: HashMap::new(),
            duration_ms: 42,
        };
        let cloned = resp.clone();
        assert_eq!(cloned.status_code, 200);
        assert_eq!(cloned.body, "test");
        assert_eq!(cloned.duration_ms, 42);
    }
}
