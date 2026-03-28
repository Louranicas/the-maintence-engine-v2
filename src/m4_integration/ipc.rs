//! # M22: IPC Manager
//!
//! Inter-process communication via Unix Domain Sockets for ultra-low-latency
//! messaging between co-located ULTRAPLATE services. Manages socket
//! registrations, simulated message exchange, and connection health.
//!
//! ## Layer: L4 (Integration)
//! ## Dependencies: M01 (Error), L4 mod.rs types
//!
//! ## Features
//!
//! - Socket path registration with convention `/var/run/maintenance/{id}.sock`
//! - Simulated send/receive with sub-millisecond latency tracking
//! - Per-socket message counters and health scoring
//! - Bounded message log (500 entries)
//! - Socket lifecycle management (bind, connect, close)
//!
//! ## Supported Services
//!
//! | Service | Socket | Use Case |
//! |---------|--------|----------|
//! | SAN-K7 | `san-k7.sock` | Orchestrator commands |
//! | Bash Engine | `bash-engine.sock` | Command execution |
//!
//! ## Related Documentation
//! - [Module Specification](../../ai_docs/modules/M22_IPC_MANAGER.md)
//! - [Layer Specification](../../ai_docs/layers/L04_INTEGRATION.md)

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use uuid::Uuid;

use crate::{Error, Result};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Default socket directory.
const SOCKET_DIR: &str = "/var/run/maintenance";

/// Maximum message log capacity.
const MESSAGE_LOG_CAPACITY: usize = 500;

/// Default IPC timeout in milliseconds.
const DEFAULT_TIMEOUT_MS: u64 = 1000;

/// Simulated IPC latency in microseconds.
const SIMULATED_LATENCY_US: u64 = 50;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// IPC socket state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SocketState {
    /// Socket not yet created.
    Unbound,
    /// Socket bound and listening.
    Listening,
    /// Connected to a peer.
    Connected,
    /// Connection error.
    Error,
    /// Socket closed.
    Closed,
}

/// IPC message type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IpcMessageType {
    /// Request message.
    Request,
    /// Response message.
    Response,
    /// Notification (fire-and-forget).
    Notification,
    /// Heartbeat ping.
    Heartbeat,
    /// Shutdown signal.
    Shutdown,
}

impl std::fmt::Display for IpcMessageType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Request => write!(f, "REQUEST"),
            Self::Response => write!(f, "RESPONSE"),
            Self::Notification => write!(f, "NOTIFICATION"),
            Self::Heartbeat => write!(f, "HEARTBEAT"),
            Self::Shutdown => write!(f, "SHUTDOWN"),
        }
    }
}

/// A registered IPC socket endpoint.
#[derive(Clone, Debug)]
pub struct IpcSocket {
    /// Service ID owning the socket.
    pub service_id: String,
    /// Socket file path.
    pub socket_path: String,
    /// Current state.
    pub state: SocketState,
    /// Timeout in milliseconds.
    pub timeout_ms: u64,
    /// Messages sent.
    pub messages_sent: u64,
    /// Messages received.
    pub messages_received: u64,
    /// Bytes sent.
    pub bytes_sent: u64,
    /// Bytes received.
    pub bytes_received: u64,
    /// Errors encountered.
    pub error_count: u64,
    /// Cumulative latency in microseconds.
    pub cumulative_latency_us: u64,
    /// Registration timestamp.
    pub registered_at: DateTime<Utc>,
    /// Last activity timestamp.
    pub last_activity: Option<DateTime<Utc>>,
}

impl IpcSocket {
    /// Average latency in microseconds.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn average_latency_us(&self) -> f64 {
        let total = self.messages_sent + self.messages_received;
        if total == 0 {
            0.0
        } else {
            self.cumulative_latency_us as f64 / total as f64
        }
    }

    /// Health score (0.0 - 1.0).
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn health_score(&self) -> f64 {
        if self.state != SocketState::Connected && self.state != SocketState::Listening {
            return 0.0;
        }
        let total = self.messages_sent + self.messages_received;
        if total == 0 {
            return 1.0;
        }
        let error_ratio = self.error_count as f64 / total as f64;
        (1.0 - error_ratio).max(0.0)
    }
}

/// A record of an IPC message exchange.
#[derive(Clone, Debug)]
pub struct IpcMessageRecord {
    /// Unique message identifier.
    pub id: String,
    /// Source service ID.
    pub source: String,
    /// Target service ID.
    pub target: String,
    /// Message type.
    pub message_type: IpcMessageType,
    /// Payload content.
    pub payload: String,
    /// Payload size in bytes.
    pub payload_bytes: usize,
    /// Latency in microseconds.
    pub latency_us: u64,
    /// Whether the exchange succeeded.
    pub success: bool,
    /// Timestamp.
    pub timestamp: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// IpcManager
// ---------------------------------------------------------------------------

/// IPC manager for Unix Domain Socket communication.
///
/// Manages socket registrations, simulated message exchanges,
/// and connection lifecycle for co-located ULTRAPLATE services.
pub struct IpcManager {
    /// Registered sockets keyed by service ID.
    sockets: RwLock<HashMap<String, IpcSocket>>,
    /// Message log (bounded).
    messages: RwLock<Vec<IpcMessageRecord>>,
    /// Socket directory prefix.
    socket_dir: String,
}

impl Default for IpcManager {
    fn default() -> Self {
        Self::new()
    }
}

impl IpcManager {
    /// Create a new IPC manager with the default socket directory.
    #[must_use]
    pub fn new() -> Self {
        Self {
            sockets: RwLock::new(HashMap::new()),
            messages: RwLock::new(Vec::new()),
            socket_dir: SOCKET_DIR.into(),
        }
    }

    /// Create a new IPC manager with a custom socket directory.
    #[must_use]
    pub fn with_socket_dir(dir: impl Into<String>) -> Self {
        Self {
            sockets: RwLock::new(HashMap::new()),
            messages: RwLock::new(Vec::new()),
            socket_dir: dir.into(),
        }
    }

    /// Generate the socket path for a service.
    #[must_use]
    pub fn socket_path(&self, service_id: &str) -> String {
        format!("{}/{service_id}.sock", self.socket_dir)
    }

    /// Register a socket for a service.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if the service ID is empty.
    pub fn register(&self, service_id: &str) -> Result<String> {
        if service_id.is_empty() {
            return Err(Error::Validation("Service ID cannot be empty".into()));
        }

        let path = self.socket_path(service_id);
        let socket = IpcSocket {
            service_id: service_id.into(),
            socket_path: path.clone(),
            state: SocketState::Listening,
            timeout_ms: DEFAULT_TIMEOUT_MS,
            messages_sent: 0,
            messages_received: 0,
            bytes_sent: 0,
            bytes_received: 0,
            error_count: 0,
            cumulative_latency_us: 0,
            registered_at: Utc::now(),
            last_activity: None,
        };

        self.sockets.write().insert(service_id.into(), socket);
        Ok(path)
    }

    /// Unregister a socket.
    ///
    /// # Errors
    ///
    /// Returns `Error::ServiceNotFound` if the socket is not registered.
    pub fn unregister(&self, service_id: &str) -> Result<()> {
        if self.sockets.write().remove(service_id).is_some() {
            Ok(())
        } else {
            Err(Error::ServiceNotFound(service_id.into()))
        }
    }

    /// Get a socket snapshot.
    #[must_use]
    pub fn get_socket(&self, service_id: &str) -> Option<IpcSocket> {
        self.sockets.read().get(service_id).cloned()
    }

    /// Get socket count.
    #[must_use]
    pub fn socket_count(&self) -> usize {
        self.sockets.read().len()
    }

    /// Get all registered service IDs.
    #[must_use]
    pub fn registered_services(&self) -> Vec<String> {
        self.sockets.read().keys().cloned().collect()
    }

    /// Set socket state.
    ///
    /// # Errors
    ///
    /// Returns `Error::ServiceNotFound` if the socket is not registered.
    pub fn set_state(&self, service_id: &str, state: SocketState) -> Result<()> {
        let mut sockets = self.sockets.write();
        if let Some(socket) = sockets.get_mut(service_id) {
            socket.state = state;
            drop(sockets);
            Ok(())
        } else {
            drop(sockets);
            Err(Error::ServiceNotFound(service_id.into()))
        }
    }

    /// Connect a socket to its peer (transition to Connected state).
    ///
    /// # Errors
    ///
    /// Returns `Error::ServiceNotFound` if the socket is not registered.
    /// Returns `Error::Validation` if the socket is not in `Listening` state.
    pub fn connect(&self, service_id: &str) -> Result<()> {
        let mut sockets = self.sockets.write();
        let socket = sockets
            .get_mut(service_id);

        if let Some(socket) = socket {
            if socket.state != SocketState::Listening {
                drop(sockets);
                return Err(Error::Validation(format!(
                    "Socket for {service_id} is not in Listening state"
                )));
            }
            socket.state = SocketState::Connected;
            drop(sockets);
            Ok(())
        } else {
            drop(sockets);
            Err(Error::ServiceNotFound(service_id.into()))
        }
    }

    /// Close a socket.
    ///
    /// # Errors
    ///
    /// Returns `Error::ServiceNotFound` if the socket is not registered.
    pub fn close(&self, service_id: &str) -> Result<()> {
        let mut sockets = self.sockets.write();
        if let Some(socket) = sockets.get_mut(service_id) {
            socket.state = SocketState::Closed;
            drop(sockets);
            Ok(())
        } else {
            drop(sockets);
            Err(Error::ServiceNotFound(service_id.into()))
        }
    }

    /// Simulate sending a message via IPC.
    ///
    /// # Errors
    ///
    /// Returns `Error::ServiceNotFound` if either source or target is not registered.
    /// Returns `Error::Network` if the source socket is not connected.
    pub fn send(
        &self,
        source: &str,
        target: &str,
        message_type: IpcMessageType,
        payload: &str,
    ) -> Result<String> {
        let (src_state, tgt_exists) = {
            let sockets = self.sockets.read();
            let src_state = sockets
                .get(source)
                .map(|s| s.state);
            let tgt = sockets.contains_key(target);
            drop(sockets);
            (
                src_state.ok_or_else(|| Error::ServiceNotFound(source.into()))?,
                tgt,
            )
        };

        if !tgt_exists {
            return Err(Error::ServiceNotFound(target.into()));
        }

        let success = src_state == SocketState::Connected;
        let latency_us = if success { SIMULATED_LATENCY_US } else { 0 };

        let msg_id = Uuid::new_v4().to_string();
        let record = IpcMessageRecord {
            id: msg_id.clone(),
            source: source.into(),
            target: target.into(),
            message_type,
            payload: payload.into(),
            payload_bytes: payload.len(),
            latency_us,
            success,
            timestamp: Utc::now(),
        };

        // Update socket metrics
        {
            let mut sockets = self.sockets.write();
            if let Some(src) = sockets.get_mut(source) {
                src.messages_sent += 1;
                src.bytes_sent += payload.len() as u64;
                src.cumulative_latency_us += latency_us;
                src.last_activity = Some(Utc::now());
                if !success {
                    src.error_count += 1;
                }
            }
            if let Some(tgt) = sockets.get_mut(target) {
                if success {
                    tgt.messages_received += 1;
                    tgt.bytes_received += payload.len() as u64;
                    tgt.cumulative_latency_us += latency_us;
                    tgt.last_activity = Some(Utc::now());
                }
            }
        }

        // Append to message log
        {
            let mut log = self.messages.write();
            if log.len() >= MESSAGE_LOG_CAPACITY {
                let quarter = log.len() / 4;
                log.drain(..quarter);
            }
            log.push(record);
        }

        if success {
            Ok(msg_id)
        } else {
            Err(Error::Network {
                target: target.into(),
                message: format!("IPC socket for {source} is not connected"),
            })
        }
    }

    /// Get message count.
    #[must_use]
    pub fn message_count(&self) -> usize {
        self.messages.read().len()
    }

    /// Get messages between two services.
    #[must_use]
    pub fn messages_between(&self, source: &str, target: &str) -> Vec<IpcMessageRecord> {
        self.messages
            .read()
            .iter()
            .filter(|m| m.source == source && m.target == target)
            .cloned()
            .collect()
    }

    /// Get messages by type.
    #[must_use]
    pub fn messages_by_type(&self, msg_type: IpcMessageType) -> Vec<IpcMessageRecord> {
        self.messages
            .read()
            .iter()
            .filter(|m| m.message_type == msg_type)
            .cloned()
            .collect()
    }

    /// Get overall health score across all sockets.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn overall_health(&self) -> f64 {
        let sockets = self.sockets.read();
        if sockets.is_empty() {
            return 1.0;
        }
        let total: f64 = sockets.values().map(IpcSocket::health_score).sum();
        total / sockets.len() as f64
    }

    /// Clear message log.
    pub fn clear_messages(&self) {
        self.messages.write().clear();
    }

    /// Set timeout for a socket.
    ///
    /// # Errors
    ///
    /// Returns `Error::ServiceNotFound` if the socket is not registered.
    pub fn set_timeout(&self, service_id: &str, timeout_ms: u64) -> Result<()> {
        let mut sockets = self.sockets.write();
        if let Some(socket) = sockets.get_mut(service_id) {
            socket.timeout_ms = timeout_ms;
            drop(sockets);
            Ok(())
        } else {
            drop(sockets);
            Err(Error::ServiceNotFound(service_id.into()))
        }
    }

    /// Get healthy socket count (Connected or Listening).
    #[must_use]
    pub fn healthy_socket_count(&self) -> usize {
        self.sockets
            .read()
            .values()
            .filter(|s| s.state == SocketState::Connected || s.state == SocketState::Listening)
            .count()
    }

    /// Reset metrics for a socket.
    ///
    /// # Errors
    ///
    /// Returns `Error::ServiceNotFound` if the socket is not registered.
    pub fn reset_metrics(&self, service_id: &str) -> Result<()> {
        let mut sockets = self.sockets.write();
        if let Some(socket) = sockets.get_mut(service_id) {
            socket.messages_sent = 0;
            socket.messages_received = 0;
            socket.bytes_sent = 0;
            socket.bytes_received = 0;
            socket.error_count = 0;
            socket.cumulative_latency_us = 0;
            socket.last_activity = None;
            drop(sockets);
            Ok(())
        } else {
            drop(sockets);
            Err(Error::ServiceNotFound(service_id.into()))
        }
    }
}

/// Default IPC-capable services.
#[must_use]
pub fn default_ipc_services() -> Vec<String> {
    vec!["san-k7".into(), "bash-engine".into()]
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_manager() -> IpcManager {
        let mgr = IpcManager::with_socket_dir("/tmp/test-ipc");
        mgr.register("san-k7").ok();
        mgr.register("bash-engine").ok();
        mgr
    }

    fn setup_connected() -> IpcManager {
        let mgr = setup_manager();
        mgr.connect("san-k7").ok();
        mgr.connect("bash-engine").ok();
        mgr
    }

    #[test]
    fn test_register_socket() {
        let mgr = IpcManager::new();
        let path = mgr.register("san-k7");
        assert!(path.is_ok());
        assert_eq!(mgr.socket_count(), 1);
    }

    #[test]
    fn test_register_empty_id_fails() {
        let mgr = IpcManager::new();
        assert!(mgr.register("").is_err());
    }

    #[test]
    fn test_socket_path_format() {
        let mgr = IpcManager::new();
        assert_eq!(
            mgr.socket_path("san-k7"),
            "/var/run/maintenance/san-k7.sock"
        );
    }

    #[test]
    fn test_custom_socket_dir() {
        let mgr = IpcManager::with_socket_dir("/tmp/custom");
        assert_eq!(mgr.socket_path("svc"), "/tmp/custom/svc.sock");
    }

    #[test]
    fn test_unregister() {
        let mgr = setup_manager();
        assert!(mgr.unregister("san-k7").is_ok());
        assert_eq!(mgr.socket_count(), 1);
    }

    #[test]
    fn test_unregister_nonexistent_fails() {
        let mgr = IpcManager::new();
        assert!(mgr.unregister("none").is_err());
    }

    #[test]
    fn test_get_socket() {
        let mgr = setup_manager();
        let socket = mgr.get_socket("san-k7");
        assert!(socket.is_some());
    }

    #[test]
    fn test_get_socket_nonexistent() {
        let mgr = IpcManager::new();
        assert!(mgr.get_socket("none").is_none());
    }

    #[test]
    fn test_registered_services() {
        let mgr = setup_manager();
        let services = mgr.registered_services();
        assert_eq!(services.len(), 2);
    }

    #[test]
    fn test_initial_state_listening() {
        let mgr = setup_manager();
        let socket = mgr.get_socket("san-k7");
        assert_eq!(
            socket.map(|s| s.state).unwrap_or(SocketState::Unbound),
            SocketState::Listening
        );
    }

    #[test]
    fn test_connect() {
        let mgr = setup_manager();
        assert!(mgr.connect("san-k7").is_ok());
        let socket = mgr.get_socket("san-k7");
        assert_eq!(
            socket.map(|s| s.state).unwrap_or(SocketState::Unbound),
            SocketState::Connected
        );
    }

    #[test]
    fn test_connect_nonexistent_fails() {
        let mgr = IpcManager::new();
        assert!(mgr.connect("none").is_err());
    }

    #[test]
    fn test_connect_already_connected_fails() {
        let mgr = setup_manager();
        mgr.connect("san-k7").ok();
        assert!(mgr.connect("san-k7").is_err());
    }

    #[test]
    fn test_close() {
        let mgr = setup_connected();
        assert!(mgr.close("san-k7").is_ok());
        let socket = mgr.get_socket("san-k7");
        assert_eq!(
            socket.map(|s| s.state).unwrap_or(SocketState::Unbound),
            SocketState::Closed
        );
    }

    #[test]
    fn test_close_nonexistent_fails() {
        let mgr = IpcManager::new();
        assert!(mgr.close("none").is_err());
    }

    #[test]
    fn test_send_success() {
        let mgr = setup_connected();
        let result = mgr.send("san-k7", "bash-engine", IpcMessageType::Request, "cmd");
        assert!(result.is_ok());
    }

    #[test]
    fn test_send_not_connected_fails() {
        let mgr = setup_manager(); // Listening, not Connected
        assert!(mgr
            .send("san-k7", "bash-engine", IpcMessageType::Request, "x")
            .is_err());
    }

    #[test]
    fn test_send_nonexistent_source_fails() {
        let mgr = setup_connected();
        assert!(mgr
            .send("none", "bash-engine", IpcMessageType::Request, "x")
            .is_err());
    }

    #[test]
    fn test_send_nonexistent_target_fails() {
        let mgr = setup_connected();
        assert!(mgr
            .send("san-k7", "none", IpcMessageType::Request, "x")
            .is_err());
    }

    #[test]
    fn test_send_updates_metrics() {
        let mgr = setup_connected();
        let _ = mgr.send("san-k7", "bash-engine", IpcMessageType::Request, "hello");
        let src = mgr.get_socket("san-k7");
        assert_eq!(src.as_ref().map(|s| s.messages_sent).unwrap_or(0), 1);
        assert_eq!(src.map(|s| s.bytes_sent).unwrap_or(0), 5);
    }

    #[test]
    fn test_send_updates_target_metrics() {
        let mgr = setup_connected();
        let _ = mgr.send("san-k7", "bash-engine", IpcMessageType::Request, "hello");
        let tgt = mgr.get_socket("bash-engine");
        assert_eq!(tgt.map(|s| s.messages_received).unwrap_or(0), 1);
    }

    #[test]
    fn test_message_count() {
        let mgr = setup_connected();
        let _ = mgr.send("san-k7", "bash-engine", IpcMessageType::Request, "a");
        let _ = mgr.send("san-k7", "bash-engine", IpcMessageType::Request, "b");
        assert_eq!(mgr.message_count(), 2);
    }

    #[test]
    fn test_messages_between() {
        let mgr = setup_connected();
        let _ = mgr.send("san-k7", "bash-engine", IpcMessageType::Request, "x");
        let msgs = mgr.messages_between("san-k7", "bash-engine");
        assert_eq!(msgs.len(), 1);
    }

    #[test]
    fn test_messages_by_type() {
        let mgr = setup_connected();
        let _ = mgr.send("san-k7", "bash-engine", IpcMessageType::Heartbeat, "");
        let _ = mgr.send("san-k7", "bash-engine", IpcMessageType::Request, "x");
        assert_eq!(mgr.messages_by_type(IpcMessageType::Heartbeat).len(), 1);
    }

    #[test]
    fn test_overall_health_all_connected() {
        let mgr = setup_connected();
        assert!(mgr.overall_health() > 0.0);
    }

    #[test]
    fn test_overall_health_empty() {
        let mgr = IpcManager::new();
        assert!((mgr.overall_health() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_clear_messages() {
        let mgr = setup_connected();
        let _ = mgr.send("san-k7", "bash-engine", IpcMessageType::Request, "x");
        mgr.clear_messages();
        assert_eq!(mgr.message_count(), 0);
    }

    #[test]
    fn test_set_timeout() {
        let mgr = setup_manager();
        assert!(mgr.set_timeout("san-k7", 500).is_ok());
        let socket = mgr.get_socket("san-k7");
        assert_eq!(socket.map(|s| s.timeout_ms).unwrap_or(0), 500);
    }

    #[test]
    fn test_set_timeout_nonexistent_fails() {
        let mgr = IpcManager::new();
        assert!(mgr.set_timeout("none", 500).is_err());
    }

    #[test]
    fn test_healthy_socket_count() {
        let mgr = setup_connected();
        assert_eq!(mgr.healthy_socket_count(), 2);
        mgr.close("san-k7").ok();
        assert_eq!(mgr.healthy_socket_count(), 1);
    }

    #[test]
    fn test_reset_metrics() {
        let mgr = setup_connected();
        let _ = mgr.send("san-k7", "bash-engine", IpcMessageType::Request, "x");
        assert!(mgr.reset_metrics("san-k7").is_ok());
        let socket = mgr.get_socket("san-k7");
        assert_eq!(socket.map(|s| s.messages_sent).unwrap_or(1), 0);
    }

    #[test]
    fn test_reset_metrics_nonexistent_fails() {
        let mgr = IpcManager::new();
        assert!(mgr.reset_metrics("none").is_err());
    }

    #[test]
    fn test_socket_health_score_connected() {
        let socket = IpcSocket {
            service_id: "t".into(),
            socket_path: "/t".into(),
            state: SocketState::Connected,
            timeout_ms: 1000,
            messages_sent: 10,
            messages_received: 10,
            bytes_sent: 100,
            bytes_received: 100,
            error_count: 0,
            cumulative_latency_us: 500,
            registered_at: Utc::now(),
            last_activity: None,
        };
        assert!((socket.health_score() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_socket_health_score_with_errors() {
        let socket = IpcSocket {
            service_id: "t".into(),
            socket_path: "/t".into(),
            state: SocketState::Connected,
            timeout_ms: 1000,
            messages_sent: 10,
            messages_received: 10,
            bytes_sent: 100,
            bytes_received: 100,
            error_count: 5,
            cumulative_latency_us: 500,
            registered_at: Utc::now(),
            last_activity: None,
        };
        assert!(socket.health_score() < 1.0);
        assert!(socket.health_score() > 0.0);
    }

    #[test]
    fn test_socket_health_score_closed() {
        let socket = IpcSocket {
            service_id: "t".into(),
            socket_path: "/t".into(),
            state: SocketState::Closed,
            timeout_ms: 1000,
            messages_sent: 0,
            messages_received: 0,
            bytes_sent: 0,
            bytes_received: 0,
            error_count: 0,
            cumulative_latency_us: 0,
            registered_at: Utc::now(),
            last_activity: None,
        };
        assert!((socket.health_score()).abs() < f64::EPSILON);
    }

    #[test]
    fn test_socket_average_latency() {
        let socket = IpcSocket {
            service_id: "t".into(),
            socket_path: "/t".into(),
            state: SocketState::Connected,
            timeout_ms: 1000,
            messages_sent: 5,
            messages_received: 5,
            bytes_sent: 0,
            bytes_received: 0,
            error_count: 0,
            cumulative_latency_us: 1000,
            registered_at: Utc::now(),
            last_activity: None,
        };
        assert!((socket.average_latency_us() - 100.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_set_state() {
        let mgr = setup_manager();
        assert!(mgr.set_state("san-k7", SocketState::Error).is_ok());
        let socket = mgr.get_socket("san-k7");
        assert_eq!(
            socket.map(|s| s.state).unwrap_or(SocketState::Unbound),
            SocketState::Error
        );
    }

    #[test]
    fn test_set_state_nonexistent_fails() {
        let mgr = IpcManager::new();
        assert!(mgr.set_state("none", SocketState::Error).is_err());
    }

    #[test]
    fn test_message_type_display() {
        assert_eq!(IpcMessageType::Request.to_string(), "REQUEST");
        assert_eq!(IpcMessageType::Heartbeat.to_string(), "HEARTBEAT");
    }

    #[test]
    fn test_default_ipc_services() {
        let services = default_ipc_services();
        assert!(services.len() >= 2);
        assert!(services.contains(&"san-k7".to_string()));
    }

    #[test]
    fn test_message_log_bounded() {
        let mgr = setup_connected();
        for i in 0..600 {
            let _ = mgr.send(
                "san-k7",
                "bash-engine",
                IpcMessageType::Request,
                &format!("m{i}"),
            );
        }
        assert!(mgr.message_count() <= MESSAGE_LOG_CAPACITY);
    }

    #[test]
    fn test_notification_fire_and_forget() {
        let mgr = setup_connected();
        let result = mgr.send(
            "san-k7",
            "bash-engine",
            IpcMessageType::Notification,
            "event",
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_shutdown_message() {
        let mgr = setup_connected();
        let result = mgr.send(
            "san-k7",
            "bash-engine",
            IpcMessageType::Shutdown,
            "",
        );
        assert!(result.is_ok());
    }

    // --- Additional tests to reach 50+ ---

    #[test]
    fn test_default_creates_same_as_new() {
        let d = IpcManager::default();
        let n = IpcManager::new();
        assert_eq!(d.socket_count(), n.socket_count());
    }

    #[test]
    fn test_register_returns_socket_path() {
        let mgr = IpcManager::with_socket_dir("/tmp/test");
        let path = mgr.register("my-svc");
        assert!(path.is_ok());
        assert_eq!(path.unwrap_or_default(), "/tmp/test/my-svc.sock");
    }

    #[test]
    fn test_register_duplicate_replaces() {
        let mgr = IpcManager::new();
        assert!(mgr.register("svc-a").is_ok());
        assert!(mgr.register("svc-a").is_ok()); // replaces
        assert_eq!(mgr.socket_count(), 1);
    }

    #[test]
    fn test_message_type_display_all_variants() {
        assert_eq!(IpcMessageType::Response.to_string(), "RESPONSE");
        assert_eq!(IpcMessageType::Notification.to_string(), "NOTIFICATION");
        assert_eq!(IpcMessageType::Shutdown.to_string(), "SHUTDOWN");
    }

    #[test]
    fn test_socket_health_score_listening() {
        let mgr = setup_manager(); // Listening state
        let socket = mgr.get_socket("san-k7");
        let score = socket.map(|s| s.health_score()).unwrap_or(-1.0);
        // Listening with no messages => 1.0
        assert!((score - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_socket_health_score_unbound() {
        let socket = IpcSocket {
            service_id: "t".into(),
            socket_path: "/t".into(),
            state: SocketState::Unbound,
            timeout_ms: 1000,
            messages_sent: 0,
            messages_received: 0,
            bytes_sent: 0,
            bytes_received: 0,
            error_count: 0,
            cumulative_latency_us: 0,
            registered_at: Utc::now(),
            last_activity: None,
        };
        assert!(socket.health_score().abs() < f64::EPSILON);
    }

    #[test]
    fn test_socket_health_score_error_state() {
        let socket = IpcSocket {
            service_id: "t".into(),
            socket_path: "/t".into(),
            state: SocketState::Error,
            timeout_ms: 1000,
            messages_sent: 10,
            messages_received: 10,
            bytes_sent: 100,
            bytes_received: 100,
            error_count: 0,
            cumulative_latency_us: 0,
            registered_at: Utc::now(),
            last_activity: None,
        };
        assert!(socket.health_score().abs() < f64::EPSILON);
    }

    #[test]
    fn test_socket_average_latency_no_messages() {
        let socket = IpcSocket {
            service_id: "t".into(),
            socket_path: "/t".into(),
            state: SocketState::Connected,
            timeout_ms: 1000,
            messages_sent: 0,
            messages_received: 0,
            bytes_sent: 0,
            bytes_received: 0,
            error_count: 0,
            cumulative_latency_us: 0,
            registered_at: Utc::now(),
            last_activity: None,
        };
        assert!(socket.average_latency_us().abs() < f64::EPSILON);
    }

    #[test]
    fn test_send_error_increments_error_count() {
        let mgr = setup_manager(); // Listening, not Connected
        // This send will fail because source is not Connected
        let _r = mgr.send("san-k7", "bash-engine", IpcMessageType::Request, "x");
        let socket = mgr.get_socket("san-k7");
        assert_eq!(socket.map(|s| s.error_count).unwrap_or(0), 1);
    }

    #[test]
    fn test_send_updates_last_activity() {
        let mgr = setup_connected();
        let _r = mgr.send("san-k7", "bash-engine", IpcMessageType::Request, "data");
        let socket = mgr.get_socket("san-k7");
        assert!(socket.as_ref().and_then(|s| s.last_activity).is_some());
    }

    #[test]
    fn test_overall_health_mixed_states() {
        let mgr = setup_connected();
        mgr.close("san-k7").ok(); // Now closed
        let health = mgr.overall_health();
        // One connected (1.0), one closed (0.0) => avg = 0.5
        assert!((health - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_messages_between_wrong_direction() {
        let mgr = setup_connected();
        let _r = mgr.send("san-k7", "bash-engine", IpcMessageType::Request, "x");
        let msgs = mgr.messages_between("bash-engine", "san-k7"); // reversed
        assert!(msgs.is_empty());
    }

    #[test]
    fn test_messages_by_type_empty() {
        let mgr = setup_connected();
        let msgs = mgr.messages_by_type(IpcMessageType::Response);
        assert!(msgs.is_empty());
    }

    #[test]
    fn test_response_message_type() {
        let mgr = setup_connected();
        let result = mgr.send(
            "bash-engine",
            "san-k7",
            IpcMessageType::Response,
            "result",
        );
        assert!(result.is_ok());
        let msgs = mgr.messages_by_type(IpcMessageType::Response);
        assert_eq!(msgs.len(), 1);
    }
}
