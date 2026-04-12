//! # M47: Tool Registrar
//!
//! Manages registration and deregistration of the 15 Maintenance Engine tools
//! with the ULTRAPLATE Tool Library service at `http://localhost:8105`.
//!
//! ## Layer: L4 (Integration)
//!
//! ## Features
//!
//! - Bulk registration of all 15 tools via `ServiceRegistrationPayload`
//! - Per-tool status tracking with timestamps and error messages
//! - Retry logic with configurable backoff (3 attempts, 2s backoff)
//! - Graceful deregistration on shutdown
//! - Aggregate registration reporting
//! - Thread-safe state via `parking_lot::RwLock` and `AtomicBool`
//!
//! ## Thread Safety
//!
//! All mutable state is protected by `RwLock` from `parking_lot`. The
//! registrar requires only `&self` for all operations.
//!
//! ## Related Documentation
//! - [Tool Library](../../service_registry/SERVICE_REGISTRY.md)
//! - [Layer Specification](../../ai_docs/layers/L04_INTEGRATION.md)

#![allow(clippy::module_name_repetitions)]

use std::sync::atomic::{AtomicBool, Ordering};

use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

use crate::tools::{all_tool_definitions, ToolDefinition};
use crate::{Error, Result};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Tool Library registration endpoint.
const TOOL_LIBRARY_URL: &str = "http://localhost:8105";

/// Maximum retry attempts for registration.
const MAX_RETRIES: u32 = 3;

/// Retry backoff in seconds.
const RETRY_BACKOFF_SECS: u64 = 2;

/// Maintenance Engine service identifier.
const SERVICE_ID: &str = "maintenance-engine";

/// Maintenance Engine human-readable name.
const SERVICE_NAME: &str = "Maintenance Engine";

/// Maintenance Engine version.
const SERVICE_VERSION: &str = "1.0.0";

/// Default host for the Maintenance Engine.
const DEFAULT_HOST: &str = "localhost";

// ---------------------------------------------------------------------------
// Registration Status
// ---------------------------------------------------------------------------

/// Registration status for a single tool.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolRegistrationStatus {
    /// Tool identifier.
    pub tool_id: String,
    /// Whether the tool is currently registered.
    pub registered: bool,
    /// Timestamp of the last registration attempt.
    pub last_attempt: Option<DateTime<Utc>>,
    /// Error message from the last failed attempt, if any.
    pub error_message: Option<String>,
    /// Number of registration attempts made.
    pub attempts: u32,
}

impl ToolRegistrationStatus {
    /// Create a new pending status for a tool.
    #[must_use]
    fn new(tool_id: &str) -> Self {
        Self {
            tool_id: tool_id.to_string(),
            registered: false,
            last_attempt: None,
            error_message: None,
            attempts: 0,
        }
    }

    /// Mark this tool as successfully registered.
    fn mark_registered(&mut self) {
        self.registered = true;
        self.last_attempt = Some(Utc::now());
        self.error_message = None;
        self.attempts += 1;
    }

    /// Mark this tool as failed with an error message.
    fn mark_failed(&mut self, message: &str) {
        self.registered = false;
        self.last_attempt = Some(Utc::now());
        self.error_message = Some(message.to_string());
        self.attempts += 1;
    }

    /// Mark this tool as deregistered.
    fn mark_deregistered(&mut self) {
        self.registered = false;
        self.last_attempt = Some(Utc::now());
        self.error_message = None;
    }

    /// Check if this tool is in a pending state (never attempted).
    #[must_use]
    const fn is_pending(&self) -> bool {
        !self.registered && self.last_attempt.is_none()
    }
}

// ---------------------------------------------------------------------------
// Registration Report
// ---------------------------------------------------------------------------

/// Aggregate registration report across all tools.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RegistrationReport {
    /// Total number of tools managed by this registrar.
    pub total_tools: usize,
    /// Number of tools currently registered.
    pub registered_count: usize,
    /// Number of tools that failed registration.
    pub failed_count: usize,
    /// Number of tools pending registration (never attempted).
    pub pending_count: usize,
    /// Per-tool registration statuses.
    pub tools: Vec<ToolRegistrationStatus>,
}

impl RegistrationReport {
    /// Check if all tools are registered.
    #[must_use]
    pub const fn all_registered(&self) -> bool {
        self.registered_count == self.total_tools
    }

    /// Check if any tools have failed.
    #[must_use]
    pub const fn has_failures(&self) -> bool {
        self.failed_count > 0
    }

    /// Get the ratio of registered tools (0.0 to 1.0).
    #[must_use]
    pub fn registration_ratio(&self) -> f64 {
        if self.total_tools == 0 {
            return 0.0;
        }
        #[allow(clippy::cast_precision_loss)]
        {
            self.registered_count as f64 / self.total_tools as f64
        }
    }
}

// ---------------------------------------------------------------------------
// Service Registration Payload
// ---------------------------------------------------------------------------

/// Service registration payload for the Tool Library.
///
/// This is the JSON body sent to `POST {TOOL_LIBRARY_URL}/services/register`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ServiceRegistrationPayload {
    /// Service identifier.
    pub service_id: String,
    /// Human-readable service name.
    pub service_name: String,
    /// Service version string.
    pub version: String,
    /// Host address.
    pub host: String,
    /// Port number.
    pub port: u16,
    /// Tools provided by this service.
    pub tools: Vec<ToolRegistrationEntry>,
}

impl ServiceRegistrationPayload {
    /// Build a registration payload from the given tool definitions and port.
    #[must_use]
    fn from_definitions(definitions: &[ToolDefinition], port: u16) -> Self {
        let entries: Vec<ToolRegistrationEntry> = definitions
            .iter()
            .map(ToolRegistrationEntry::from_definition)
            .collect();

        Self {
            service_id: SERVICE_ID.to_string(),
            service_name: SERVICE_NAME.to_string(),
            version: SERVICE_VERSION.to_string(),
            host: DEFAULT_HOST.to_string(),
            port,
            tools: entries,
        }
    }

    /// Build a registration payload for a single tool.
    #[must_use]
    fn from_single(definition: &ToolDefinition, port: u16) -> Self {
        Self {
            service_id: SERVICE_ID.to_string(),
            service_name: SERVICE_NAME.to_string(),
            version: SERVICE_VERSION.to_string(),
            host: DEFAULT_HOST.to_string(),
            port,
            tools: vec![ToolRegistrationEntry::from_definition(definition)],
        }
    }

    /// Get the number of tools in this payload.
    #[must_use]
    pub const fn tool_count(&self) -> usize {
        self.tools.len()
    }
}

// ---------------------------------------------------------------------------
// Service Deregistration Payload
// ---------------------------------------------------------------------------

/// Service deregistration payload for the Tool Library.
///
/// This is the JSON body sent to `POST {TOOL_LIBRARY_URL}/services/deregister`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ServiceDeregistrationPayload {
    /// Service identifier to deregister.
    pub service_id: String,
    /// Version being deregistered.
    pub version: String,
}

impl ServiceDeregistrationPayload {
    /// Build a deregistration payload for the Maintenance Engine.
    #[must_use]
    fn for_maintenance_engine() -> Self {
        Self {
            service_id: SERVICE_ID.to_string(),
            version: SERVICE_VERSION.to_string(),
        }
    }
}

// ---------------------------------------------------------------------------
// Tool Registration Entry
// ---------------------------------------------------------------------------

/// Individual tool entry in a registration payload.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolRegistrationEntry {
    /// Tool identifier.
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// Tool description.
    pub description: String,
    /// Tool category.
    pub category: String,
    /// REST endpoint path.
    pub endpoint: String,
    /// HTTP method.
    pub method: String,
}

impl ToolRegistrationEntry {
    /// Create a registration entry from a tool definition.
    ///
    /// The `ToolCategory` enum is serialized to its `snake_case` string form
    /// for the JSON payload (matching the Tool Library's expected format).
    #[must_use]
    fn from_definition(def: &ToolDefinition) -> Self {
        // Serialize the category enum to its snake_case JSON string.
        let category_str = serde_json::to_string(&def.category)
            .unwrap_or_default()
            .trim_matches('"')
            .to_string();

        Self {
            id: def.id.clone(),
            name: def.name.clone(),
            description: def.description.clone(),
            category: category_str,
            endpoint: def.endpoint.clone(),
            method: def.method.clone(),
        }
    }
}

// ---------------------------------------------------------------------------
// Tool Registrar
// ---------------------------------------------------------------------------

/// Manages tool registration with the ULTRAPLATE Tool Library.
///
/// The registrar tracks the registration status of all 15 Maintenance Engine
/// tools, handles retry logic with exponential backoff, and provides aggregate
/// reporting on registration state.
///
/// # Thread Safety
///
/// All mutable state is protected by `parking_lot::RwLock`. The registrar
/// requires only `&self` for all operations including async registration.
///
/// # Example
///
/// ```rust,no_run
/// # use maintenance_engine::m4_integration::tool_registrar::ToolRegistrar;
/// # async fn example() -> maintenance_engine::Result<()> {
/// let registrar = ToolRegistrar::new(8180)?;
/// let count = registrar.register_all().await?;
/// let report = registrar.registration_report();
/// assert!(report.all_registered());
/// # Ok(())
/// # }
/// ```
pub struct ToolRegistrar {
    /// HTTP client for communicating with the Tool Library.
    http_client: reqwest::Client,
    /// Per-tool registration status, keyed by tool ID.
    registration_status: RwLock<Vec<ToolRegistrationStatus>>,
    /// Whether all tools are currently registered.
    is_registered: AtomicBool,
    /// Port on which the Maintenance Engine is running.
    service_port: u16,
}

impl ToolRegistrar {
    /// Create a new `ToolRegistrar` with status entries for all 15 tools.
    ///
    /// Initializes an HTTP client with a 10-second timeout for Tool Library
    /// communication.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Network`] if the HTTP client cannot be created.
    pub fn new(service_port: u16) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .map_err(|e| Error::Network {
                target: "tool_registrar".into(),
                message: format!("Failed to create HTTP client: {e}"),
            })?;

        let definitions = all_tool_definitions();
        let statuses: Vec<ToolRegistrationStatus> = definitions
            .iter()
            .map(|d| ToolRegistrationStatus::new(&d.id))
            .collect();

        Ok(Self {
            http_client: client,
            registration_status: RwLock::new(statuses),
            is_registered: AtomicBool::new(false),
            service_port,
        })
    }

    /// Check if all tools are currently registered with the Tool Library.
    #[must_use]
    pub fn is_registered(&self) -> bool {
        self.is_registered.load(Ordering::Relaxed)
    }

    /// Get the service port this registrar was configured with.
    #[must_use]
    pub const fn service_port(&self) -> u16 {
        self.service_port
    }

    /// Get the total number of tools managed by this registrar.
    #[must_use]
    pub fn tool_count(&self) -> usize {
        self.registration_status.read().len()
    }

    /// Get the number of tools currently registered.
    #[must_use]
    pub fn registered_count(&self) -> usize {
        self.registration_status
            .read()
            .iter()
            .filter(|s| s.registered)
            .count()
    }

    /// Generate an aggregate registration report.
    #[must_use]
    pub fn registration_report(&self) -> RegistrationReport {
        let statuses = self.registration_status.read();
        let total = statuses.len();
        let registered = statuses.iter().filter(|s| s.registered).count();
        let failed = statuses
            .iter()
            .filter(|s| !s.registered && s.error_message.is_some())
            .count();
        let pending = statuses.iter().filter(|s| s.is_pending()).count();

        RegistrationReport {
            total_tools: total,
            registered_count: registered,
            failed_count: failed,
            pending_count: pending,
            tools: statuses.clone(),
        }
    }

    /// Get the registration status for a specific tool.
    ///
    /// Returns `None` if the tool ID is not found.
    #[must_use]
    pub fn tool_status(&self, tool_id: &str) -> Option<ToolRegistrationStatus> {
        self.registration_status
            .read()
            .iter()
            .find(|s| s.tool_id == tool_id)
            .cloned()
    }

    /// Register all 15 tools with the Tool Library.
    ///
    /// Builds a [`ServiceRegistrationPayload`] from all tool definitions and
    /// POSTs it to `{TOOL_LIBRARY_URL}/services/register`. Retries up to
    /// [`MAX_RETRIES`] times with [`RETRY_BACKOFF_SECS`] backoff on failure.
    ///
    /// On success, all tool statuses are updated to `registered: true`.
    /// On failure, the error message is stored in each tool's status.
    ///
    /// Returns the count of successfully registered tools.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Network`] if all retry attempts fail.
    pub async fn register_all(&self) -> Result<usize> {
        let definitions = all_tool_definitions();
        if definitions.is_empty() {
            return Ok(0);
        }

        let payload = ServiceRegistrationPayload::from_definitions(&definitions, self.service_port);
        let url = format!("{TOOL_LIBRARY_URL}/services/register");

        let mut last_error = String::new();

        for attempt in 0..MAX_RETRIES {
            if attempt > 0 {
                tokio::time::sleep(std::time::Duration::from_secs(
                    RETRY_BACKOFF_SECS * u64::from(attempt),
                ))
                .await;
            }

            let result = self
                .http_client
                .post(&url)
                .json(&payload)
                .send()
                .await;

            match result {
                Ok(resp) if resp.status().is_success() => {
                    let count = self.mark_all_registered();
                    return Ok(count);
                }
                Ok(resp) => {
                    let status = resp.status();
                    let body = resp.text().await.unwrap_or_default();
                    last_error = format!("HTTP {status}: {body}");
                    self.update_attempt_counts(&last_error);
                }
                Err(e) => {
                    last_error = format!("Request failed: {e}");
                    self.update_attempt_counts(&last_error);
                }
            }
        }

        self.mark_all_failed(&last_error);
        Err(Error::Network {
            target: "tool-library".into(),
            message: format!(
                "Failed to register tools after {MAX_RETRIES} attempts: {last_error}"
            ),
        })
    }

    /// Deregister all tools from the Tool Library on shutdown.
    ///
    /// POSTs a deregistration payload to `{TOOL_LIBRARY_URL}/services/deregister`.
    /// Retries up to [`MAX_RETRIES`] times with backoff.
    ///
    /// Returns the count of tools that were deregistered (i.e., were previously
    /// registered).
    ///
    /// # Errors
    ///
    /// Returns [`Error::Network`] if all retry attempts fail.
    pub async fn deregister_all(&self) -> Result<usize> {
        let payload = ServiceDeregistrationPayload::for_maintenance_engine();
        let url = format!("{TOOL_LIBRARY_URL}/services/deregister");

        let previously_registered = self.registered_count();
        let mut last_error = String::new();

        for attempt in 0..MAX_RETRIES {
            if attempt > 0 {
                tokio::time::sleep(std::time::Duration::from_secs(
                    RETRY_BACKOFF_SECS * u64::from(attempt),
                ))
                .await;
            }

            let result = self
                .http_client
                .post(&url)
                .json(&payload)
                .send()
                .await;

            match result {
                Ok(resp) if resp.status().is_success() => {
                    self.mark_all_deregistered();
                    return Ok(previously_registered);
                }
                Ok(resp) => {
                    let status = resp.status();
                    last_error = format!("HTTP {status}");
                }
                Err(e) => {
                    last_error = format!("Request failed: {e}");
                }
            }
        }

        Err(Error::Network {
            target: "tool-library".into(),
            message: format!(
                "Failed to deregister tools after {MAX_RETRIES} attempts: {last_error}"
            ),
        })
    }

    /// Register a single tool with the Tool Library.
    ///
    /// Looks up the tool by ID from the standard definitions and sends a
    /// single-tool registration payload.
    ///
    /// Returns `true` if registration succeeded, `false` otherwise.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] if the tool ID is not found in definitions.
    /// Returns [`Error::Network`] if the HTTP request fails after retries.
    pub async fn register_single(&self, tool_id: &str) -> Result<bool> {
        let definitions = all_tool_definitions();
        let definition = definitions
            .iter()
            .find(|d| d.id == tool_id)
            .ok_or_else(|| Error::Validation(format!("Unknown tool ID: {tool_id}")))?;

        let payload = ServiceRegistrationPayload::from_single(definition, self.service_port);
        let url = format!("{TOOL_LIBRARY_URL}/services/register");

        let mut last_error = String::new();

        for attempt in 0..MAX_RETRIES {
            if attempt > 0 {
                tokio::time::sleep(std::time::Duration::from_secs(
                    RETRY_BACKOFF_SECS * u64::from(attempt),
                ))
                .await;
            }

            let result = self
                .http_client
                .post(&url)
                .json(&payload)
                .send()
                .await;

            match result {
                Ok(resp) if resp.status().is_success() => {
                    self.mark_tool_registered(tool_id);
                    return Ok(true);
                }
                Ok(resp) => {
                    let status = resp.status();
                    last_error = format!("HTTP {status}");
                    self.mark_tool_failed(tool_id, &last_error);
                }
                Err(e) => {
                    last_error = format!("Request failed: {e}");
                    self.mark_tool_failed(tool_id, &last_error);
                }
            }
        }

        Err(Error::Network {
            target: "tool-library".into(),
            message: format!(
                "Failed to register tool '{tool_id}' after {MAX_RETRIES} attempts: {last_error}"
            ),
        })
    }

    /// Build a full registration payload for all tools.
    ///
    /// This is useful for inspection or custom registration flows.
    #[must_use]
    pub fn build_payload(&self) -> ServiceRegistrationPayload {
        let definitions = all_tool_definitions();
        ServiceRegistrationPayload::from_definitions(&definitions, self.service_port)
    }

    /// Build a deregistration payload.
    ///
    /// This is useful for inspection or custom deregistration flows.
    #[must_use]
    pub fn build_deregistration_payload(&self) -> ServiceDeregistrationPayload {
        ServiceDeregistrationPayload::for_maintenance_engine()
    }

    // ---------------------------------------------------------------
    // Internal helpers
    // ---------------------------------------------------------------

    /// Mark all tools as registered and set the atomic flag.
    fn mark_all_registered(&self) -> usize {
        let mut statuses = self.registration_status.write();
        for status in statuses.iter_mut() {
            status.mark_registered();
        }
        let count = statuses.len();
        drop(statuses);
        self.is_registered.store(true, Ordering::Relaxed);
        count
    }

    /// Mark all tools as failed with the given error message.
    fn mark_all_failed(&self, message: &str) {
        let mut statuses = self.registration_status.write();
        for status in statuses.iter_mut() {
            if !status.registered {
                status.mark_failed(message);
            }
        }
        drop(statuses);
        self.is_registered.store(false, Ordering::Relaxed);
    }

    /// Increment attempt counts without changing registration state.
    fn update_attempt_counts(&self, message: &str) {
        let mut statuses = self.registration_status.write();
        for status in statuses.iter_mut() {
            if !status.registered {
                status.error_message = Some(message.to_string());
                status.last_attempt = Some(Utc::now());
            }
        }
    }

    /// Mark all tools as deregistered and clear the atomic flag.
    fn mark_all_deregistered(&self) {
        let mut statuses = self.registration_status.write();
        for status in statuses.iter_mut() {
            status.mark_deregistered();
        }
        drop(statuses);
        self.is_registered.store(false, Ordering::Relaxed);
    }

    /// Mark a single tool as registered.
    fn mark_tool_registered(&self, tool_id: &str) {
        let mut statuses = self.registration_status.write();
        if let Some(status) = statuses.iter_mut().find(|s| s.tool_id == tool_id) {
            status.mark_registered();
        }
        // Update atomic flag if all are registered
        let all_registered = statuses.iter().all(|s| s.registered);
        drop(statuses);
        if all_registered {
            self.is_registered.store(true, Ordering::Relaxed);
        }
    }

    /// Mark a single tool as failed.
    fn mark_tool_failed(&self, tool_id: &str, message: &str) {
        let mut statuses = self.registration_status.write();
        if let Some(status) = statuses.iter_mut().find(|s| s.tool_id == tool_id) {
            status.mark_failed(message);
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ── Construction Tests ────────────────────────────────────────

    #[test]
    fn test_new_creates_registrar() {
        let registrar = ToolRegistrar::new(8180);
        assert!(registrar.is_ok());
    }

    #[test]
    fn test_new_with_default_port() {
        let registrar = ToolRegistrar::new(8180);
        assert!(registrar.is_ok());
        let r = create_test_registrar();
        assert_eq!(r.service_port(), 8180);
    }

    #[test]
    fn test_new_initializes_15_tools() {
        let r = create_test_registrar();
        assert_eq!(r.tool_count(), 15);
    }

    #[test]
    fn test_new_not_registered() {
        let r = create_test_registrar();
        assert!(!r.is_registered());
    }

    #[test]
    fn test_service_port_stored() {
        let r = create_test_registrar();
        assert_eq!(r.service_port(), 8180);
    }

    #[test]
    fn test_custom_port() {
        let registrar = ToolRegistrar::new(9999);
        assert!(registrar.is_ok());
        if let Ok(r) = registrar {
            assert_eq!(r.service_port(), 9999);
        }
    }

    // ── Registration Status Tests ────────────────────────────────

    #[test]
    fn test_initial_status_is_pending() {
        let r = create_test_registrar();
        let statuses = r.registration_status.read();
        for status in statuses.iter() {
            assert!(!status.registered);
            assert!(status.last_attempt.is_none());
            assert!(status.error_message.is_none());
            assert_eq!(status.attempts, 0);
        }
    }

    #[test]
    fn test_tool_registration_status_new() {
        let status = ToolRegistrationStatus::new("test-tool");
        assert_eq!(status.tool_id, "test-tool");
        assert!(!status.registered);
        assert!(status.is_pending());
    }

    #[test]
    fn test_mark_registered() {
        let mut status = ToolRegistrationStatus::new("test-tool");
        status.mark_registered();
        assert!(status.registered);
        assert!(status.last_attempt.is_some());
        assert!(status.error_message.is_none());
        assert_eq!(status.attempts, 1);
    }

    #[test]
    fn test_mark_failed() {
        let mut status = ToolRegistrationStatus::new("test-tool");
        status.mark_failed("connection refused");
        assert!(!status.registered);
        assert!(status.last_attempt.is_some());
        assert_eq!(status.error_message.as_deref(), Some("connection refused"));
        assert_eq!(status.attempts, 1);
    }

    #[test]
    fn test_mark_deregistered() {
        let mut status = ToolRegistrationStatus::new("test-tool");
        status.mark_registered();
        assert!(status.registered);
        status.mark_deregistered();
        assert!(!status.registered);
        assert!(status.error_message.is_none());
    }

    #[test]
    fn test_is_pending_after_attempt() {
        let mut status = ToolRegistrationStatus::new("test-tool");
        assert!(status.is_pending());
        status.mark_failed("error");
        assert!(!status.is_pending());
    }

    #[test]
    fn test_multiple_attempts_increment() {
        let mut status = ToolRegistrationStatus::new("test-tool");
        status.mark_failed("error 1");
        status.mark_failed("error 2");
        status.mark_registered();
        assert_eq!(status.attempts, 3);
    }

    // ── Registration Report Tests ────────────────────────────────

    #[test]
    fn test_initial_report_all_pending() {
        let r = create_test_registrar();
        let report = r.registration_report();
        assert_eq!(report.total_tools, 15);
        assert_eq!(report.registered_count, 0);
        assert_eq!(report.failed_count, 0);
        assert_eq!(report.pending_count, 15);
    }

    #[test]
    fn test_report_after_mark_all_registered() {
        let r = create_test_registrar();
        let count = r.mark_all_registered();
        assert_eq!(count, 15);

        let report = r.registration_report();
        assert_eq!(report.registered_count, 15);
        assert_eq!(report.failed_count, 0);
        assert_eq!(report.pending_count, 0);
        assert!(report.all_registered());
    }

    #[test]
    fn test_report_after_mark_all_failed() {
        let r = create_test_registrar();
        r.mark_all_failed("timeout");

        let report = r.registration_report();
        assert_eq!(report.registered_count, 0);
        assert_eq!(report.failed_count, 15);
        assert_eq!(report.pending_count, 0);
        assert!(report.has_failures());
    }

    #[test]
    fn test_report_mixed_state() {
        let r = create_test_registrar();
        r.mark_tool_registered("me-health-check");
        r.mark_tool_failed("me-submit-remediation", "timeout");

        let report = r.registration_report();
        assert_eq!(report.registered_count, 1);
        assert_eq!(report.failed_count, 1);
        assert_eq!(report.pending_count, 13);
    }

    #[test]
    fn test_report_registration_ratio_zero() {
        let r = create_test_registrar();
        let report = r.registration_report();
        assert!(report.registration_ratio().abs() < f64::EPSILON);
    }

    #[test]
    fn test_report_registration_ratio_full() {
        let r = create_test_registrar();
        r.mark_all_registered();
        let report = r.registration_report();
        assert!((report.registration_ratio() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_report_registration_ratio_partial() {
        let r = create_test_registrar();
        r.mark_tool_registered("me-health-check");
        let report = r.registration_report();
        let expected = 1.0 / 15.0;
        assert!((report.registration_ratio() - expected).abs() < 1e-10);
    }

    #[test]
    fn test_report_all_registered_false_when_partial() {
        let r = create_test_registrar();
        r.mark_tool_registered("me-health-check");
        let report = r.registration_report();
        assert!(!report.all_registered());
    }

    // ── Tool Status Lookup Tests ─────────────────────────────────

    #[test]
    fn test_tool_status_exists() {
        let r = create_test_registrar();
        let status = r.tool_status("me-health-check");
        assert!(status.is_some());
        if let Some(s) = status {
            assert_eq!(s.tool_id, "me-health-check");
            assert!(!s.registered);
        }
    }

    #[test]
    fn test_tool_status_not_exists() {
        let r = create_test_registrar();
        let status = r.tool_status("nonexistent-tool");
        assert!(status.is_none());
    }

    #[test]
    fn test_tool_status_reflects_registration() {
        let r = create_test_registrar();
        r.mark_tool_registered("me-health-check");
        let status = r.tool_status("me-health-check");
        assert!(status.is_some());
        if let Some(s) = status {
            assert!(s.registered);
        }
    }

    #[test]
    fn test_tool_status_reflects_failure() {
        let r = create_test_registrar();
        r.mark_tool_failed("me-submit-remediation", "server error");
        let status = r.tool_status("me-submit-remediation");
        assert!(status.is_some());
        if let Some(s) = status {
            assert!(!s.registered);
            assert_eq!(s.error_message.as_deref(), Some("server error"));
        }
    }

    // ── Payload Construction Tests ───────────────────────────────

    #[test]
    fn test_build_payload_has_15_tools() {
        let r = create_test_registrar();
        let payload = r.build_payload();
        assert_eq!(payload.tool_count(), 15);
    }

    #[test]
    fn test_build_payload_service_id() {
        let r = create_test_registrar();
        let payload = r.build_payload();
        assert_eq!(payload.service_id, SERVICE_ID);
    }

    #[test]
    fn test_build_payload_service_name() {
        let r = create_test_registrar();
        let payload = r.build_payload();
        assert_eq!(payload.service_name, SERVICE_NAME);
    }

    #[test]
    fn test_build_payload_version() {
        let r = create_test_registrar();
        let payload = r.build_payload();
        assert_eq!(payload.version, SERVICE_VERSION);
    }

    #[test]
    fn test_build_payload_host() {
        let r = create_test_registrar();
        let payload = r.build_payload();
        assert_eq!(payload.host, DEFAULT_HOST);
    }

    #[test]
    fn test_build_payload_port() {
        let r = create_test_registrar();
        let payload = r.build_payload();
        assert_eq!(payload.port, 8180);
    }

    #[test]
    fn test_build_payload_custom_port() {
        if let Ok(r) = ToolRegistrar::new(9999) {
            let payload = r.build_payload();
            assert_eq!(payload.port, 9999);
        }
    }

    #[test]
    fn test_build_payload_tool_entries_populated() {
        let r = create_test_registrar();
        let payload = r.build_payload();
        for entry in &payload.tools {
            assert!(!entry.id.is_empty());
            assert!(!entry.name.is_empty());
            assert!(!entry.description.is_empty());
            assert!(!entry.category.is_empty());
            assert!(!entry.endpoint.is_empty());
            assert!(!entry.method.is_empty());
        }
    }

    #[test]
    fn test_build_deregistration_payload() {
        let r = create_test_registrar();
        let payload = r.build_deregistration_payload();
        assert_eq!(payload.service_id, SERVICE_ID);
        assert_eq!(payload.version, SERVICE_VERSION);
    }

    // ── Serialization Tests ──────────────────────────────────────

    #[test]
    fn test_payload_serialization() {
        let r = create_test_registrar();
        let payload = r.build_payload();
        let json = serde_json::to_string(&payload);
        assert!(json.is_ok());
    }

    #[test]
    fn test_payload_deserialization() {
        let r = create_test_registrar();
        let payload = r.build_payload();
        let json = serde_json::to_string(&payload).unwrap_or_default();
        let parsed: std::result::Result<ServiceRegistrationPayload, _> =
            serde_json::from_str(&json);
        assert!(parsed.is_ok());
    }

    #[test]
    fn test_deregistration_payload_serialization() {
        let payload = ServiceDeregistrationPayload::for_maintenance_engine();
        let json = serde_json::to_string(&payload);
        assert!(json.is_ok());
    }

    #[test]
    fn test_registration_status_serialization() {
        let status = ToolRegistrationStatus::new("test-tool");
        let json = serde_json::to_string(&status);
        assert!(json.is_ok());
    }

    #[test]
    fn test_registration_status_deserialization() {
        let status = ToolRegistrationStatus::new("test-tool");
        let json = serde_json::to_string(&status).unwrap_or_default();
        let parsed: std::result::Result<ToolRegistrationStatus, _> =
            serde_json::from_str(&json);
        assert!(parsed.is_ok());
    }

    #[test]
    fn test_registration_report_serialization() {
        let r = create_test_registrar();
        let report = r.registration_report();
        let json = serde_json::to_string(&report);
        assert!(json.is_ok());
    }

    #[test]
    fn test_tool_entry_serialization() {
        let entry = ToolRegistrationEntry {
            id: "test".into(),
            name: "Test Tool".into(),
            description: "A test tool".into(),
            category: "testing".into(),
            endpoint: "/api/test".into(),
            method: "POST".into(),
        };
        let json = serde_json::to_string(&entry);
        assert!(json.is_ok());
    }

    // ── Registration Flag Tests ──────────────────────────────────

    #[test]
    fn test_is_registered_initially_false() {
        let r = create_test_registrar();
        assert!(!r.is_registered());
    }

    #[test]
    fn test_is_registered_after_mark_all() {
        let r = create_test_registrar();
        r.mark_all_registered();
        assert!(r.is_registered());
    }

    #[test]
    fn test_is_registered_after_deregister() {
        let r = create_test_registrar();
        r.mark_all_registered();
        assert!(r.is_registered());
        r.mark_all_deregistered();
        assert!(!r.is_registered());
    }

    #[test]
    fn test_registered_count_initially_zero() {
        let r = create_test_registrar();
        assert_eq!(r.registered_count(), 0);
    }

    #[test]
    fn test_registered_count_after_mark_all() {
        let r = create_test_registrar();
        r.mark_all_registered();
        assert_eq!(r.registered_count(), 15);
    }

    #[test]
    fn test_registered_count_partial() {
        let r = create_test_registrar();
        r.mark_tool_registered("me-health-check");
        r.mark_tool_registered("me-submit-remediation");
        assert_eq!(r.registered_count(), 2);
    }

    // ── Single-Tool State Tests ──────────────────────────────────

    #[test]
    fn test_mark_single_tool_registered() {
        let r = create_test_registrar();
        r.mark_tool_registered("me-health-check");
        let status = r.tool_status("me-health-check");
        assert!(status.is_some());
        if let Some(s) = status {
            assert!(s.registered);
            assert_eq!(s.attempts, 1);
        }
    }

    #[test]
    fn test_mark_single_tool_failed() {
        let r = create_test_registrar();
        r.mark_tool_failed("me-tensor-snapshot", "network error");
        let status = r.tool_status("me-tensor-snapshot");
        assert!(status.is_some());
        if let Some(s) = status {
            assert!(!s.registered);
            assert_eq!(s.error_message.as_deref(), Some("network error"));
        }
    }

    #[test]
    fn test_mark_unknown_tool_is_noop() {
        let r = create_test_registrar();
        r.mark_tool_registered("nonexistent");
        // Should not crash; no status entry for nonexistent tool
        assert_eq!(r.registered_count(), 0);
    }

    // ── Error Cases ──────────────────────────────────────────────

    #[test]
    fn test_report_empty_registration_ratio() {
        let report = RegistrationReport {
            total_tools: 0,
            registered_count: 0,
            failed_count: 0,
            pending_count: 0,
            tools: vec![],
        };
        assert!(report.registration_ratio().abs() < f64::EPSILON);
    }

    #[test]
    fn test_report_no_failures_by_default() {
        let r = create_test_registrar();
        let report = r.registration_report();
        assert!(!report.has_failures());
    }

    #[test]
    fn test_all_tool_ids_present_in_registrar() {
        let r = create_test_registrar();
        let definitions = all_tool_definitions();
        for def in &definitions {
            let status = r.tool_status(&def.id);
            assert!(status.is_some(), "Missing status for tool: {}", def.id);
        }
    }

    // ── Helper ───────────────────────────────────────────────────

    /// Create a test registrar on the default port.
    fn create_test_registrar() -> ToolRegistrar {
        ToolRegistrar::new(8180).unwrap_or_else(|_| {
            // Fallback: should never happen as reqwest::Client::new() rarely fails
            ToolRegistrar {
                http_client: reqwest::Client::new(),
                registration_status: RwLock::new(Vec::new()),
                is_registered: AtomicBool::new(false),
                service_port: 8180,
            }
        })
    }
}
