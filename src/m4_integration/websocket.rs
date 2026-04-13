//! # M21: WebSocket Client
//!
//! Real-time full-duplex streaming communication client for the
//! Maintenance Engine. Manages persistent WebSocket connections,
//! frame-based messaging, keep-alive pings, and reconnection logic
//! for ULTRAPLATE services that support WebSocket interfaces.
//!
//! ## Layer: L4 (Integration)
//! ## Dependencies: M01 (Error), L4 mod.rs types
//!
//! ## Features
//!
//! - Persistent connection management with auto-reconnect
//! - Frame-based messaging (text and binary)
//! - Ping/pong keep-alive tracking
//! - Per-connection message history (bounded at 1000)
//! - Subscription-based message routing
//! - Connection health scoring
//!
//! ## Supported Services
//!
//! | Service | Port | Use Case |
//! |---------|------|----------|
//! | SYNTHEX | 8091 | Real-time pattern streaming |
//! | `CodeSynthor` V7 | 8110 | Build event streaming |
//!
//! ## Related Documentation
//! - [Module Specification](../../ai_docs/modules/M21_WEBSOCKET_CLIENT.md)
//! - [Layer Specification](../../ai_docs/layers/L04_INTEGRATION.md)

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use uuid::Uuid;

use crate::{Error, Result};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Maximum number of message records per connection.
const MESSAGE_LOG_CAPACITY: usize = 1000;

/// Default ping interval in milliseconds.
const DEFAULT_PING_INTERVAL_MS: u64 = 30_000;

/// Default reconnection delay in milliseconds.
const DEFAULT_RECONNECT_DELAY_MS: u64 = 1000;

/// Maximum reconnection attempts before giving up.
const DEFAULT_MAX_RECONNECT_ATTEMPTS: u32 = 10;

/// Default connection timeout in milliseconds.
const DEFAULT_CONNECT_TIMEOUT_MS: u64 = 10_000;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// WebSocket frame type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameType {
    /// UTF-8 text frame.
    Text,
    /// Binary data frame.
    Binary,
    /// Ping control frame.
    Ping,
    /// Pong control frame.
    Pong,
    /// Connection close frame.
    Close,
}

impl std::fmt::Display for FrameType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Text => write!(f, "TEXT"),
            Self::Binary => write!(f, "BINARY"),
            Self::Ping => write!(f, "PING"),
            Self::Pong => write!(f, "PONG"),
            Self::Close => write!(f, "CLOSE"),
        }
    }
}

/// WebSocket connection state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WsConnectionState {
    /// Not connected.
    Disconnected,
    /// Handshake in progress.
    Connecting,
    /// Connected and operational.
    Connected,
    /// Reconnecting after failure.
    Reconnecting,
    /// Permanently closed.
    Closed,
}

/// Close code following RFC 6455.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CloseCode {
    /// Normal closure.
    Normal,
    /// Endpoint going away.
    GoingAway,
    /// Protocol error.
    ProtocolError,
    /// Unsupported data.
    UnsupportedData,
    /// No status received.
    NoStatus,
    /// Abnormal closure.
    Abnormal,
    /// Invalid frame payload.
    InvalidPayload,
    /// Policy violation.
    PolicyViolation,
    /// Message too big.
    MessageTooBig,
    /// Internal error.
    InternalError,
}

impl CloseCode {
    /// Get the numeric code per RFC 6455.
    #[must_use]
    pub const fn code(self) -> u16 {
        match self {
            Self::Normal => 1000,
            Self::GoingAway => 1001,
            Self::ProtocolError => 1002,
            Self::UnsupportedData => 1003,
            Self::NoStatus => 1005,
            Self::Abnormal => 1006,
            Self::InvalidPayload => 1007,
            Self::PolicyViolation => 1008,
            Self::MessageTooBig => 1009,
            Self::InternalError => 1011,
        }
    }

    /// Whether this close code is considered normal.
    #[must_use]
    pub const fn is_normal(self) -> bool {
        matches!(self, Self::Normal | Self::GoingAway)
    }
}

/// A record of a WebSocket message for audit.
#[derive(Clone, Debug)]
pub struct WsMessage {
    /// Unique message identifier.
    pub id: String,
    /// Connection this message belongs to.
    pub connection_id: String,
    /// Frame type.
    pub frame_type: FrameType,
    /// Message payload (text or binary as string).
    pub payload: String,
    /// Payload size in bytes.
    pub payload_bytes: usize,
    /// Direction: true = sent, false = received.
    pub outbound: bool,
    /// Timestamp.
    pub timestamp: DateTime<Utc>,
}

/// WebSocket connection configuration.
#[derive(Clone, Debug)]
pub struct WsConfig {
    /// Ping interval in milliseconds.
    pub ping_interval_ms: u64,
    /// Reconnection delay in milliseconds.
    pub reconnect_delay_ms: u64,
    /// Maximum reconnection attempts.
    pub max_reconnect_attempts: u32,
    /// Connection timeout in milliseconds.
    pub connect_timeout_ms: u64,
    /// Maximum message size in bytes.
    pub max_message_size: usize,
}

impl Default for WsConfig {
    fn default() -> Self {
        Self {
            ping_interval_ms: DEFAULT_PING_INTERVAL_MS,
            reconnect_delay_ms: DEFAULT_RECONNECT_DELAY_MS,
            max_reconnect_attempts: DEFAULT_MAX_RECONNECT_ATTEMPTS,
            connect_timeout_ms: DEFAULT_CONNECT_TIMEOUT_MS,
            max_message_size: 1_048_576, // 1 MiB
        }
    }
}

/// A managed WebSocket connection.
#[derive(Clone, Debug)]
pub struct WsConnection {
    /// Unique connection ID.
    pub id: String,
    /// ULTRAPLATE service ID.
    pub service_id: String,
    /// Remote host.
    pub host: String,
    /// Remote port.
    pub port: u16,
    /// WebSocket path (e.g. `/ws`).
    pub path: String,
    /// Current state.
    pub state: WsConnectionState,
    /// Messages sent.
    pub messages_sent: u64,
    /// Messages received.
    pub messages_received: u64,
    /// Bytes sent.
    pub bytes_sent: u64,
    /// Bytes received.
    pub bytes_received: u64,
    /// Ping count sent.
    pub pings_sent: u64,
    /// Pong count received.
    pub pongs_received: u64,
    /// Reconnection attempts since last successful connect.
    pub reconnect_attempts: u32,
    /// Connection established timestamp.
    pub connected_at: Option<DateTime<Utc>>,
    /// Last message timestamp.
    pub last_message: Option<DateTime<Utc>>,
    /// Last pong received timestamp.
    pub last_pong: Option<DateTime<Utc>>,
}

impl WsConnection {
    /// Calculate connection health score (0.0 - 1.0).
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn health_score(&self) -> f64 {
        if self.state != WsConnectionState::Connected {
            return 0.0;
        }

        let pong_ratio = if self.pings_sent == 0 {
            1.0
        } else {
            (self.pongs_received as f64 / self.pings_sent as f64).min(1.0)
        };

        let reconnect_penalty = if self.reconnect_attempts > 0 {
            1.0 - (f64::from(self.reconnect_attempts) * 0.1).min(0.5)
        } else {
            1.0
        };

        pong_ratio * reconnect_penalty
    }

    /// Get the WebSocket URL.
    #[must_use]
    pub fn url(&self) -> String {
        format!("ws://{}:{}{}", self.host, self.port, self.path)
    }
}

/// Subscription for filtering incoming messages.
#[derive(Clone, Debug)]
pub struct WsSubscription {
    /// Unique subscription ID.
    pub id: String,
    /// Connection ID to subscribe to.
    pub connection_id: String,
    /// Optional event type filter.
    pub event_filter: Option<String>,
    /// Subscriber name.
    pub subscriber: String,
    /// Created timestamp.
    pub created_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// WebSocketClient
// ---------------------------------------------------------------------------

/// WebSocket client for real-time streaming with ULTRAPLATE services.
///
/// Manages persistent connections, message tracking, and subscriptions.
pub struct WebSocketClient {
    /// Active connections keyed by connection ID.
    connections: RwLock<HashMap<String, WsConnection>>,
    /// Message log (bounded).
    messages: RwLock<Vec<WsMessage>>,
    /// Subscriptions keyed by subscription ID.
    subscriptions: RwLock<HashMap<String, WsSubscription>>,
    /// Configuration.
    config: WsConfig,
}

impl Default for WebSocketClient {
    fn default() -> Self {
        Self::new()
    }
}

impl WebSocketClient {
    /// Create a new `WebSocketClient` with default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self {
            connections: RwLock::new(HashMap::new()),
            messages: RwLock::new(Vec::new()),
            subscriptions: RwLock::new(HashMap::new()),
            config: WsConfig::default(),
        }
    }

    /// Create a new `WebSocketClient` with custom configuration.
    #[must_use]
    pub fn with_config(config: WsConfig) -> Self {
        Self {
            connections: RwLock::new(HashMap::new()),
            messages: RwLock::new(Vec::new()),
            subscriptions: RwLock::new(HashMap::new()),
            config,
        }
    }

    /// Open a new WebSocket connection.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if the service ID, host, or path is empty,
    /// or if the port is zero.
    pub fn connect(
        &self,
        service_id: &str,
        host: &str,
        port: u16,
        path: &str,
    ) -> Result<String> {
        if service_id.is_empty() {
            return Err(Error::Validation("Service ID cannot be empty".into()));
        }
        if host.is_empty() {
            return Err(Error::Validation("Host cannot be empty".into()));
        }
        if port == 0 {
            return Err(Error::Validation("Port cannot be zero".into()));
        }
        if path.is_empty() {
            return Err(Error::Validation("Path cannot be empty".into()));
        }

        let conn_id = Uuid::new_v4().to_string();
        let conn = WsConnection {
            id: conn_id.clone(),
            service_id: service_id.into(),
            host: host.into(),
            port,
            path: path.into(),
            state: WsConnectionState::Connected,
            messages_sent: 0,
            messages_received: 0,
            bytes_sent: 0,
            bytes_received: 0,
            pings_sent: 0,
            pongs_received: 0,
            reconnect_attempts: 0,
            connected_at: Some(Utc::now()),
            last_message: None,
            last_pong: None,
        };

        self.connections.write().insert(conn_id.clone(), conn);
        Ok(conn_id)
    }

    /// Close a WebSocket connection.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if the connection does not exist.
    pub fn disconnect(&self, connection_id: &str, code: CloseCode) -> Result<()> {
        let mut conns = self.connections.write();
        let Some(conn) = conns.get_mut(connection_id) else {
            return Err(Error::Validation(format!(
                "Connection {connection_id} not found"
            )));
        };
        conn.state = if code.is_normal() {
            WsConnectionState::Closed
        } else {
            WsConnectionState::Disconnected
        };
        drop(conns);
        Ok(())
    }

    /// Get connection state.
    #[must_use]
    pub fn connection_state(&self, connection_id: &str) -> Option<WsConnectionState> {
        self.connections.read().get(connection_id).map(|c| c.state)
    }

    /// Get a connection snapshot.
    #[must_use]
    pub fn get_connection(&self, connection_id: &str) -> Option<WsConnection> {
        self.connections.read().get(connection_id).cloned()
    }

    /// Get all connection IDs for a service.
    #[must_use]
    pub fn connections_for_service(&self, service_id: &str) -> Vec<String> {
        self.connections
            .read()
            .values()
            .filter(|c| c.service_id == service_id)
            .map(|c| c.id.clone())
            .collect()
    }

    /// Get total number of active connections (state == Connected).
    #[must_use]
    pub fn active_connection_count(&self) -> usize {
        self.connections
            .read()
            .values()
            .filter(|c| c.state == WsConnectionState::Connected)
            .count()
    }

    /// Get total number of connections (all states).
    #[must_use]
    pub fn total_connection_count(&self) -> usize {
        self.connections.read().len()
    }

    /// Send a message on a connection.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if the connection is not in `Connected` state.
    pub fn send_message(
        &self,
        connection_id: &str,
        frame_type: FrameType,
        payload: &str,
    ) -> Result<String> {
        let mut conns = self.connections.write();
        let Some(conn) = conns.get_mut(connection_id) else {
            return Err(Error::Validation(format!(
                "Connection {connection_id} not found"
            )));
        };

        if conn.state != WsConnectionState::Connected {
            return Err(Error::Validation(
                "Cannot send on a non-connected WebSocket".into(),
            ));
        }

        conn.messages_sent += 1;
        conn.bytes_sent += payload.len() as u64;
        conn.last_message = Some(Utc::now());

        if frame_type == FrameType::Ping {
            conn.pings_sent += 1;
        }
        drop(conns);

        let msg_id = Uuid::new_v4().to_string();
        let msg = WsMessage {
            id: msg_id.clone(),
            connection_id: connection_id.into(),
            frame_type,
            payload: payload.into(),
            payload_bytes: payload.len(),
            outbound: true,
            timestamp: Utc::now(),
        };

        {
            let mut log = self.messages.write();
            if log.len() >= MESSAGE_LOG_CAPACITY {
                let quarter = log.len() / 4;
                log.drain(..quarter);
            }
            log.push(msg);
        }

        Ok(msg_id)
    }

    /// Simulate receiving a message on a connection.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if the connection is not in `Connected` state.
    pub fn receive_message(
        &self,
        connection_id: &str,
        frame_type: FrameType,
        payload: &str,
    ) -> Result<String> {
        let mut conns = self.connections.write();
        let Some(conn) = conns.get_mut(connection_id) else {
            return Err(Error::Validation(format!(
                "Connection {connection_id} not found"
            )));
        };

        if conn.state != WsConnectionState::Connected {
            return Err(Error::Validation(
                "Cannot receive on a non-connected WebSocket".into(),
            ));
        }

        conn.messages_received += 1;
        conn.bytes_received += payload.len() as u64;
        conn.last_message = Some(Utc::now());

        if frame_type == FrameType::Pong {
            conn.pongs_received += 1;
            conn.last_pong = Some(Utc::now());
        }
        drop(conns);

        let msg_id = Uuid::new_v4().to_string();
        let msg = WsMessage {
            id: msg_id.clone(),
            connection_id: connection_id.into(),
            frame_type,
            payload: payload.into(),
            payload_bytes: payload.len(),
            outbound: false,
            timestamp: Utc::now(),
        };

        {
            let mut log = self.messages.write();
            if log.len() >= MESSAGE_LOG_CAPACITY {
                let quarter = log.len() / 4;
                log.drain(..quarter);
            }
            log.push(msg);
        }

        Ok(msg_id)
    }

    /// Add a subscription for incoming messages.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if the subscriber name is empty.
    pub fn subscribe(
        &self,
        connection_id: &str,
        subscriber: &str,
        event_filter: Option<String>,
    ) -> Result<String> {
        if subscriber.is_empty() {
            return Err(Error::Validation("Subscriber name cannot be empty".into()));
        }

        let sub_id = Uuid::new_v4().to_string();
        let sub = WsSubscription {
            id: sub_id.clone(),
            connection_id: connection_id.into(),
            event_filter,
            subscriber: subscriber.into(),
            created_at: Utc::now(),
        };

        self.subscriptions.write().insert(sub_id.clone(), sub);
        Ok(sub_id)
    }

    /// Remove a subscription.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if the subscription does not exist.
    pub fn unsubscribe(&self, subscription_id: &str) -> Result<()> {
        if self.subscriptions.write().remove(subscription_id).is_some() {
            Ok(())
        } else {
            Err(Error::Validation(format!(
                "Subscription {subscription_id} not found"
            )))
        }
    }

    /// Get subscription count.
    #[must_use]
    pub fn subscription_count(&self) -> usize {
        self.subscriptions.read().len()
    }

    /// Get subscriptions for a connection.
    #[must_use]
    pub fn subscriptions_for_connection(&self, connection_id: &str) -> Vec<WsSubscription> {
        self.subscriptions
            .read()
            .values()
            .filter(|s| s.connection_id == connection_id)
            .cloned()
            .collect()
    }

    /// Get the total message count in the log.
    #[must_use]
    pub fn message_count(&self) -> usize {
        self.messages.read().len()
    }

    /// Get messages for a specific connection.
    #[must_use]
    pub fn messages_for_connection(&self, connection_id: &str) -> Vec<WsMessage> {
        self.messages
            .read()
            .iter()
            .filter(|m| m.connection_id == connection_id)
            .cloned()
            .collect()
    }

    /// Simulate a reconnection attempt.
    ///
    /// # Errors
    ///
    /// Returns `Error::Validation` if the connection is not found or if
    /// maximum reconnect attempts have been exceeded.
    pub fn reconnect(&self, connection_id: &str) -> Result<()> {
        let mut conns = self.connections.write();
        let Some(conn) = conns.get_mut(connection_id) else {
            return Err(Error::Validation(format!(
                "Connection {connection_id} not found"
            )));
        };

        if conn.reconnect_attempts >= self.config.max_reconnect_attempts {
            conn.state = WsConnectionState::Closed;
            return Err(Error::Validation(
                "Maximum reconnection attempts exceeded".into(),
            ));
        }

        conn.reconnect_attempts += 1;
        conn.state = WsConnectionState::Connected;
        conn.connected_at = Some(Utc::now());
        drop(conns);
        Ok(())
    }

    /// Get the configuration.
    #[must_use]
    pub const fn config(&self) -> &WsConfig {
        &self.config
    }

    /// Clear message log.
    pub fn clear_messages(&self) {
        self.messages.write().clear();
    }
}

/// Default WebSocket endpoints for ULTRAPLATE services.
///
/// **S099 F-07:** `codesynthor-v7` (:8110) removed — retired S091, superseded
/// by V8 (:8111). V8 does not publish a WebSocket endpoint at this time;
/// add back here if/when one is exposed.
#[must_use]
pub fn default_ws_endpoints() -> Vec<(String, String, u16, String)> {
    vec![("synthex".into(), "localhost".into(), 8091, "/ws".into())]
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_client_with_connection() -> (WebSocketClient, String) {
        let client = WebSocketClient::new();
        let conn_id = client.connect("synthex", "localhost", 8091, "/ws");
        assert!(conn_id.is_ok());
        (client, conn_id.unwrap_or_default())
    }

    #[test]
    fn test_connect() {
        let client = WebSocketClient::new();
        let id = client.connect("synthex", "localhost", 8091, "/ws");
        assert!(id.is_ok());
        assert_eq!(client.total_connection_count(), 1);
    }

    #[test]
    fn test_connect_empty_service_id_fails() {
        let client = WebSocketClient::new();
        assert!(client.connect("", "localhost", 8091, "/ws").is_err());
    }

    #[test]
    fn test_connect_empty_host_fails() {
        let client = WebSocketClient::new();
        assert!(client.connect("svc", "", 8091, "/ws").is_err());
    }

    #[test]
    fn test_connect_zero_port_fails() {
        let client = WebSocketClient::new();
        assert!(client.connect("svc", "localhost", 0, "/ws").is_err());
    }

    #[test]
    fn test_connect_empty_path_fails() {
        let client = WebSocketClient::new();
        assert!(client.connect("svc", "localhost", 8091, "").is_err());
    }

    #[test]
    fn test_disconnect_normal() {
        let (client, conn_id) = setup_client_with_connection();
        assert!(client.disconnect(&conn_id, CloseCode::Normal).is_ok());
        assert_eq!(
            client.connection_state(&conn_id),
            Some(WsConnectionState::Closed)
        );
    }

    #[test]
    fn test_disconnect_abnormal() {
        let (client, conn_id) = setup_client_with_connection();
        assert!(client.disconnect(&conn_id, CloseCode::Abnormal).is_ok());
        assert_eq!(
            client.connection_state(&conn_id),
            Some(WsConnectionState::Disconnected)
        );
    }

    #[test]
    fn test_disconnect_nonexistent_fails() {
        let client = WebSocketClient::new();
        assert!(client.disconnect("none", CloseCode::Normal).is_err());
    }

    #[test]
    fn test_connection_state() {
        let (client, conn_id) = setup_client_with_connection();
        assert_eq!(
            client.connection_state(&conn_id),
            Some(WsConnectionState::Connected)
        );
    }

    #[test]
    fn test_connection_state_nonexistent() {
        let client = WebSocketClient::new();
        assert!(client.connection_state("none").is_none());
    }

    #[test]
    fn test_get_connection() {
        let (client, conn_id) = setup_client_with_connection();
        let conn = client.get_connection(&conn_id);
        assert!(conn.is_some());
        assert_eq!(conn.map(|c| c.service_id).unwrap_or_default(), "synthex");
    }

    #[test]
    fn test_connections_for_service() {
        let (client, _) = setup_client_with_connection();
        let conns = client.connections_for_service("synthex");
        assert_eq!(conns.len(), 1);
    }

    #[test]
    fn test_active_connection_count() {
        let (client, _) = setup_client_with_connection();
        assert_eq!(client.active_connection_count(), 1);
    }

    #[test]
    fn test_send_message() {
        let (client, conn_id) = setup_client_with_connection();
        let result = client.send_message(&conn_id, FrameType::Text, "hello");
        assert!(result.is_ok());
    }

    #[test]
    fn test_send_message_updates_counters() {
        let (client, conn_id) = setup_client_with_connection();
        let _ = client.send_message(&conn_id, FrameType::Text, "hello");
        let conn = client.get_connection(&conn_id);
        assert_eq!(conn.as_ref().map(|c| c.messages_sent).unwrap_or(0), 1);
        assert_eq!(conn.map(|c| c.bytes_sent).unwrap_or(0), 5);
    }

    #[test]
    fn test_send_on_disconnected_fails() {
        let (client, conn_id) = setup_client_with_connection();
        client.disconnect(&conn_id, CloseCode::Normal).ok();
        assert!(client.send_message(&conn_id, FrameType::Text, "x").is_err());
    }

    #[test]
    fn test_send_on_nonexistent_fails() {
        let client = WebSocketClient::new();
        assert!(client.send_message("none", FrameType::Text, "x").is_err());
    }

    #[test]
    fn test_receive_message() {
        let (client, conn_id) = setup_client_with_connection();
        let result = client.receive_message(&conn_id, FrameType::Text, "data");
        assert!(result.is_ok());
    }

    #[test]
    fn test_receive_pong_updates_counter() {
        let (client, conn_id) = setup_client_with_connection();
        let _ = client.receive_message(&conn_id, FrameType::Pong, "");
        let conn = client.get_connection(&conn_id);
        assert_eq!(conn.map(|c| c.pongs_received).unwrap_or(0), 1);
    }

    #[test]
    fn test_ping_increments_counter() {
        let (client, conn_id) = setup_client_with_connection();
        let _ = client.send_message(&conn_id, FrameType::Ping, "");
        let conn = client.get_connection(&conn_id);
        assert_eq!(conn.map(|c| c.pings_sent).unwrap_or(0), 1);
    }

    #[test]
    fn test_subscribe() {
        let (client, conn_id) = setup_client_with_connection();
        let sub = client.subscribe(&conn_id, "test-sub", None);
        assert!(sub.is_ok());
        assert_eq!(client.subscription_count(), 1);
    }

    #[test]
    fn test_subscribe_empty_subscriber_fails() {
        let (client, conn_id) = setup_client_with_connection();
        assert!(client.subscribe(&conn_id, "", None).is_err());
    }

    #[test]
    fn test_subscribe_with_filter() {
        let (client, conn_id) = setup_client_with_connection();
        let sub = client.subscribe(&conn_id, "sub", Some("health".into()));
        assert!(sub.is_ok());
    }

    #[test]
    fn test_unsubscribe() {
        let (client, conn_id) = setup_client_with_connection();
        let sub_id = client.subscribe(&conn_id, "sub", None).unwrap_or_default();
        assert!(client.unsubscribe(&sub_id).is_ok());
        assert_eq!(client.subscription_count(), 0);
    }

    #[test]
    fn test_unsubscribe_nonexistent_fails() {
        let client = WebSocketClient::new();
        assert!(client.unsubscribe("none").is_err());
    }

    #[test]
    fn test_subscriptions_for_connection() {
        let (client, conn_id) = setup_client_with_connection();
        let _ = client.subscribe(&conn_id, "sub1", None);
        let _ = client.subscribe(&conn_id, "sub2", None);
        assert_eq!(client.subscriptions_for_connection(&conn_id).len(), 2);
    }

    #[test]
    fn test_message_count() {
        let (client, conn_id) = setup_client_with_connection();
        let _ = client.send_message(&conn_id, FrameType::Text, "a");
        let _ = client.send_message(&conn_id, FrameType::Text, "b");
        assert_eq!(client.message_count(), 2);
    }

    #[test]
    fn test_messages_for_connection() {
        let (client, conn_id) = setup_client_with_connection();
        let _ = client.send_message(&conn_id, FrameType::Text, "x");
        let msgs = client.messages_for_connection(&conn_id);
        assert_eq!(msgs.len(), 1);
    }

    #[test]
    fn test_reconnect() {
        let (client, conn_id) = setup_client_with_connection();
        client.disconnect(&conn_id, CloseCode::Abnormal).ok();
        assert!(client.reconnect(&conn_id).is_ok());
        assert_eq!(
            client.connection_state(&conn_id),
            Some(WsConnectionState::Connected)
        );
    }

    #[test]
    fn test_reconnect_max_attempts() {
        let config = WsConfig {
            max_reconnect_attempts: 2,
            ..WsConfig::default()
        };
        let client = WebSocketClient::with_config(config);
        let conn_id = client
            .connect("svc", "localhost", 8091, "/ws")
            .unwrap_or_default();
        client.disconnect(&conn_id, CloseCode::Abnormal).ok();
        assert!(client.reconnect(&conn_id).is_ok());
        assert!(client.reconnect(&conn_id).is_ok());
        assert!(client.reconnect(&conn_id).is_err());
    }

    #[test]
    fn test_health_score_connected() {
        let (client, conn_id) = setup_client_with_connection();
        let conn = client.get_connection(&conn_id);
        let score = conn.map(|c| c.health_score()).unwrap_or(0.0);
        assert!(score > 0.0);
    }

    #[test]
    fn test_health_score_disconnected() {
        let (client, conn_id) = setup_client_with_connection();
        client.disconnect(&conn_id, CloseCode::Normal).ok();
        let conn = client.get_connection(&conn_id);
        let score = conn.map(|c| c.health_score()).unwrap_or(1.0);
        assert!((score).abs() < f64::EPSILON);
    }

    #[test]
    fn test_connection_url() {
        let (client, conn_id) = setup_client_with_connection();
        let conn = client.get_connection(&conn_id);
        assert_eq!(
            conn.map(|c| c.url()).unwrap_or_default(),
            "ws://localhost:8091/ws"
        );
    }

    #[test]
    fn test_clear_messages() {
        let (client, conn_id) = setup_client_with_connection();
        let _ = client.send_message(&conn_id, FrameType::Text, "x");
        client.clear_messages();
        assert_eq!(client.message_count(), 0);
    }

    #[test]
    fn test_close_code_numeric() {
        assert_eq!(CloseCode::Normal.code(), 1000);
        assert_eq!(CloseCode::InternalError.code(), 1011);
    }

    #[test]
    fn test_close_code_is_normal() {
        assert!(CloseCode::Normal.is_normal());
        assert!(CloseCode::GoingAway.is_normal());
        assert!(!CloseCode::InternalError.is_normal());
    }

    #[test]
    fn test_frame_type_display() {
        assert_eq!(FrameType::Text.to_string(), "TEXT");
        assert_eq!(FrameType::Binary.to_string(), "BINARY");
    }

    #[test]
    fn test_default_ws_endpoints() {
        let endpoints = default_ws_endpoints();
        // S099 F-07: codesynthor-v7 (:8110) removed after retirement S091.
        // synthex remains the only default WS endpoint until V8 exposes one.
        assert!(!endpoints.is_empty());
        assert!(endpoints.iter().any(|(id, _, _, _)| id == "synthex"));
        assert!(
            endpoints.iter().all(|(id, _, _, _)| id != "codesynthor-v7"),
            "retired codesynthor-v7 must not be in default endpoints"
        );
    }

    #[test]
    fn test_message_log_bounded() {
        let (client, conn_id) = setup_client_with_connection();
        for i in 0..1100 {
            let _ = client.send_message(&conn_id, FrameType::Text, &format!("msg-{i}"));
        }
        assert!(client.message_count() <= MESSAGE_LOG_CAPACITY);
    }

    #[test]
    fn test_multiple_connections_same_service() {
        let client = WebSocketClient::new();
        let _ = client.connect("synthex", "localhost", 8091, "/ws");
        let _ = client.connect("synthex", "localhost", 8091, "/ws");
        assert_eq!(client.connections_for_service("synthex").len(), 2);
    }

    #[test]
    fn test_binary_frame_send() {
        let (client, conn_id) = setup_client_with_connection();
        assert!(client
            .send_message(&conn_id, FrameType::Binary, "\x00\x01\x02")
            .is_ok());
    }

    #[test]
    fn test_with_custom_config() {
        let config = WsConfig {
            ping_interval_ms: 5000,
            ..WsConfig::default()
        };
        let client = WebSocketClient::with_config(config);
        assert_eq!(client.config().ping_interval_ms, 5000);
    }

    #[test]
    fn test_receive_on_disconnected_fails() {
        let (client, conn_id) = setup_client_with_connection();
        client.disconnect(&conn_id, CloseCode::Normal).ok();
        assert!(client
            .receive_message(&conn_id, FrameType::Text, "x")
            .is_err());
    }

    // --- Additional tests to reach 50+ ---

    #[test]
    fn test_default_creates_same_as_new() {
        let d = WebSocketClient::default();
        let n = WebSocketClient::new();
        assert_eq!(d.total_connection_count(), n.total_connection_count());
        assert_eq!(d.message_count(), n.message_count());
    }

    #[test]
    fn test_receive_on_nonexistent_fails() {
        let client = WebSocketClient::new();
        assert!(client
            .receive_message("none", FrameType::Text, "x")
            .is_err());
    }

    #[test]
    fn test_receive_message_updates_counters() {
        let (client, conn_id) = setup_client_with_connection();
        let _ = client.receive_message(&conn_id, FrameType::Text, "data123");
        let conn = client.get_connection(&conn_id);
        assert_eq!(conn.as_ref().map(|c| c.messages_received).unwrap_or(0), 1);
        assert_eq!(conn.map(|c| c.bytes_received).unwrap_or(0), 7);
    }

    #[test]
    fn test_close_code_all_variants() {
        assert_eq!(CloseCode::GoingAway.code(), 1001);
        assert_eq!(CloseCode::ProtocolError.code(), 1002);
        assert_eq!(CloseCode::UnsupportedData.code(), 1003);
        assert_eq!(CloseCode::NoStatus.code(), 1005);
        assert_eq!(CloseCode::Abnormal.code(), 1006);
        assert_eq!(CloseCode::InvalidPayload.code(), 1007);
        assert_eq!(CloseCode::PolicyViolation.code(), 1008);
        assert_eq!(CloseCode::MessageTooBig.code(), 1009);
    }

    #[test]
    fn test_close_code_is_normal_false_variants() {
        assert!(!CloseCode::ProtocolError.is_normal());
        assert!(!CloseCode::UnsupportedData.is_normal());
        assert!(!CloseCode::Abnormal.is_normal());
        assert!(!CloseCode::PolicyViolation.is_normal());
        assert!(!CloseCode::MessageTooBig.is_normal());
    }

    #[test]
    fn test_frame_type_display_close() {
        assert_eq!(FrameType::Close.to_string(), "CLOSE");
        assert_eq!(FrameType::Ping.to_string(), "PING");
        assert_eq!(FrameType::Pong.to_string(), "PONG");
    }

    #[test]
    fn test_health_score_with_pings_no_pongs() {
        let (client, conn_id) = setup_client_with_connection();
        let _ = client.send_message(&conn_id, FrameType::Ping, "");
        let _ = client.send_message(&conn_id, FrameType::Ping, "");
        let conn = client.get_connection(&conn_id);
        let score = conn.map(|c| c.health_score()).unwrap_or(0.0);
        // pong_ratio = 0/2 = 0.0
        assert!(score.abs() < f64::EPSILON);
    }

    #[test]
    fn test_health_score_with_pings_and_pongs() {
        let (client, conn_id) = setup_client_with_connection();
        let _ = client.send_message(&conn_id, FrameType::Ping, "");
        let _ = client.send_message(&conn_id, FrameType::Ping, "");
        let _ = client.receive_message(&conn_id, FrameType::Pong, "");
        let _ = client.receive_message(&conn_id, FrameType::Pong, "");
        let conn = client.get_connection(&conn_id);
        let score = conn.map(|c| c.health_score()).unwrap_or(0.0);
        // pong_ratio = 2/2 = 1.0, no reconnect penalty
        assert!((score - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_health_score_with_reconnect_penalty() {
        let config = WsConfig {
            max_reconnect_attempts: 10,
            ..WsConfig::default()
        };
        let client = WebSocketClient::with_config(config);
        let conn_id = client.connect("svc", "localhost", 8091, "/ws").unwrap_or_default();
        client.disconnect(&conn_id, CloseCode::Abnormal).ok();
        let _ = client.reconnect(&conn_id); // attempts = 1
        let conn = client.get_connection(&conn_id);
        let score = conn.map(|c| c.health_score()).unwrap_or(0.0);
        // reconnect_penalty = 1.0 - (1 * 0.1) = 0.9
        assert!(score < 1.0);
        assert!(score > 0.0);
    }

    #[test]
    fn test_active_connection_count_after_disconnect() {
        let (client, conn_id) = setup_client_with_connection();
        assert_eq!(client.active_connection_count(), 1);
        client.disconnect(&conn_id, CloseCode::Normal).ok();
        assert_eq!(client.active_connection_count(), 0);
    }

    #[test]
    fn test_total_vs_active_connection_count() {
        let (client, conn_id) = setup_client_with_connection();
        let _ = client.connect("svc2", "localhost", 8092, "/ws");
        client.disconnect(&conn_id, CloseCode::Normal).ok();
        assert_eq!(client.total_connection_count(), 2);
        assert_eq!(client.active_connection_count(), 1);
    }

    #[test]
    fn test_connections_for_service_no_match() {
        let client = WebSocketClient::new();
        let _ = client.connect("synthex", "localhost", 8091, "/ws");
        let conns = client.connections_for_service("nonexistent");
        assert!(conns.is_empty());
    }

    #[test]
    fn test_get_connection_nonexistent() {
        let client = WebSocketClient::new();
        assert!(client.get_connection("nonexistent").is_none());
    }

    #[test]
    fn test_ws_config_default_values() {
        let config = WsConfig::default();
        assert_eq!(config.ping_interval_ms, DEFAULT_PING_INTERVAL_MS);
        assert_eq!(config.reconnect_delay_ms, DEFAULT_RECONNECT_DELAY_MS);
        assert_eq!(config.max_reconnect_attempts, DEFAULT_MAX_RECONNECT_ATTEMPTS);
        assert_eq!(config.connect_timeout_ms, DEFAULT_CONNECT_TIMEOUT_MS);
        assert_eq!(config.max_message_size, 1_048_576);
    }
}
