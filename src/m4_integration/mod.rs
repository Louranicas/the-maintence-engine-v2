//! # Layer 4: Integration
//!
//! External service integration and communication bridges.
//!
//! ## Modules
//!
//! | Module | Name | Purpose |
//! |--------|------|---------|
//! | M19 | REST Client | HTTP/REST communication |
//! | M20 | gRPC Client | gRPC communication |
//! | M21 | WebSocket Client | Real-time streaming |
//! | M22 | IPC Manager | Inter-process communication |
//! | M23 | Event Bus | Event distribution |
//! | M24 | Bridge Manager | Service bridge coordination |
//!
//! ## 12 ULTRAPLATE Services
//!
//! | Service | Port | Protocol | Tier |
//! |---------|------|----------|------|
//! | SYNTHEX | 8090/8091 | REST/WS | 1 |
//! | SAN-K7 | 8100 | REST/IPC | 1 |
//! | NAIS | 8101 | REST | 2 |
//! | CCM | 8104 | REST | 3 |
//! | Bash Engine | 8102 | REST/IPC | 5 |
//! | Tool Maker | 8103 | REST/gRPC | 5 |
//!
//! Retired (do not re-add): `CodeSynthor` V7 :8110 (S091), `DevOps` V2 :8081 (S091),
//! `Tool` `Library` V1 :8105 (S093), `Prometheus` `Swarm` V1 :10001 (S088),
//! `Architect` `Agent` :9001 (S093), `library-agent` :8083 (S093).
//!
//! ## Related Documentation
//! - [Layer Specification](../../ai_docs/layers/L04_INTEGRATION.md)
//! - [REST API](../../ai_docs/integration/REST_API.md)

pub mod rest;
pub mod grpc;
pub mod websocket;
pub mod ipc;
pub mod event_bus;
pub mod bridge;
pub mod cascade_bridge;
pub mod peer_bridge;
pub mod tool_registrar;
pub mod auth;
pub mod rate_limiter;
pub mod orac_bridge;

// Re-export key types for convenient access
pub use rest::{RestClient, RestResponse, HttpMethod, RequestRecord};
pub use grpc::{GrpcClient, GrpcResponse, GrpcStatus, CallType};
pub use websocket::{WebSocketClient, WsMessage, WsConnectionState, FrameType};
pub use ipc::{IpcManager, IpcSocket, IpcMessageType, SocketState};
pub use event_bus::{EventBus, EventRecord, Subscription, ChannelInfo};
pub use bridge::{BridgeManager, ServiceBridge, BridgeStatus};
pub use peer_bridge::{PeerBridgeManager, PeerHealthState, PeerConfig, MeshHealthSummary};
pub use tool_registrar::{ToolRegistrar, RegistrationReport, ToolRegistrationStatus};
pub use auth::{AuthManager, Authenticator, TokenType, TokenIdentity, IssuedToken, VerifiedClaims, SecurityEvent, SecurityEventType, AuthAuditSummary, AuthConfig};
pub use rate_limiter::{RateLimiter, RateLimiting, RateDecision, BucketState, TierRateConfig, RateLimiterConfig, RateLimiterStats};
pub use orac_bridge::{OracBridge, OracBridgeManager, OracBridgeConfig, OracHealthSnapshot, OracBlackboardSnapshot, MeHookEvent, MeHookType, OracHookStats, OracBridgeHealth};

/// Wire protocol types
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WireProtocol {
    /// REST/HTTP
    Rest,
    /// gRPC
    Grpc,
    /// WebSocket
    WebSocket,
    /// Unix Domain Socket
    Ipc,
}

impl WireProtocol {
    /// Get default timeout for this protocol
    #[must_use]
    pub const fn default_timeout_ms(&self) -> u64 {
        match self {
            Self::Rest => 5000,
            Self::Grpc => 3000,
            Self::WebSocket => 10000,
            Self::Ipc => 1000,
        }
    }
}

/// Service endpoint configuration
#[derive(Clone, Debug)]
pub struct ServiceEndpoint {
    /// Service ID
    pub service_id: String,
    /// Host address
    pub host: String,
    /// Port number
    pub port: u16,
    /// Protocol
    pub protocol: WireProtocol,
    /// Health endpoint path
    pub health_path: String,
    /// Base path for API
    pub base_path: String,
    /// Request timeout in ms
    pub timeout_ms: u64,
    /// Enable retry
    pub retry_enabled: bool,
    /// Max retry attempts
    pub max_retries: u32,
}

impl ServiceEndpoint {
    /// Create a new service endpoint
    #[must_use]
    pub fn new(service_id: impl Into<String>, host: impl Into<String>, port: u16) -> Self {
        Self {
            service_id: service_id.into(),
            host: host.into(),
            port,
            protocol: WireProtocol::Rest,
            health_path: "/api/health".into(),
            base_path: "/api".into(),
            timeout_ms: 5000,
            retry_enabled: true,
            max_retries: 3,
        }
    }

    /// Get the full URL for an endpoint path
    #[must_use]
    pub fn url(&self, path: &str) -> String {
        let protocol = match self.protocol {
            WireProtocol::WebSocket => "ws",
            WireProtocol::Rest | WireProtocol::Grpc | WireProtocol::Ipc => "http",
        };
        format!("{protocol}://{}:{}{}{path}", self.host, self.port, self.base_path)
    }

    /// Get the health check URL
    #[must_use]
    pub fn health_url(&self) -> String {
        let protocol = match self.protocol {
            WireProtocol::WebSocket => "ws",
            WireProtocol::Rest | WireProtocol::Grpc | WireProtocol::Ipc => "http",
        };
        format!("{protocol}://{}:{}{}", self.host, self.port, self.health_path)
    }
}

/// Default ULTRAPLATE service endpoints
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn default_endpoints() -> Vec<ServiceEndpoint> {
    vec![
        // Tier 1: Core
        ServiceEndpoint {
            service_id: "synthex".into(),
            host: "localhost".into(),
            port: 8090,
            protocol: WireProtocol::Rest,
            health_path: "/api/health".into(),
            base_path: "/api".into(),
            timeout_ms: 10000,
            retry_enabled: true,
            max_retries: 3,
        },
        ServiceEndpoint {
            service_id: "san-k7".into(),
            host: "localhost".into(),
            port: 8100,
            protocol: WireProtocol::Rest,
            health_path: "/health".into(),
            base_path: String::new(),
            timeout_ms: 10000,
            retry_enabled: true,
            max_retries: 3,
        },
        // Tier 2: Intelligence
        ServiceEndpoint {
            service_id: "nais".into(),
            host: "localhost".into(),
            port: 8101,
            protocol: WireProtocol::Rest,
            health_path: "/health".into(),
            base_path: "/api".into(),
            timeout_ms: 50000,
            retry_enabled: true,
            max_retries: 3,
        },
        // codesynthor-v7 (8110) retired S091 — superseded by V8 (8111)
        // devops-engine V2 (8081) retired S091 — superseded by V3 (8082)
        // Tier 3: Integration
        // tool-library V1 (8105) retired S093 — superseded by V2 hb binary (CLI, no port)
        // library-agent (8083) removed: disabled in devenv, was dragging fitness tensor
        ServiceEndpoint {
            service_id: "ccm".into(),
            host: "localhost".into(),
            port: 8104,
            protocol: WireProtocol::Rest,
            health_path: "/health".into(),
            base_path: "/api".into(),
            timeout_ms: 100_000,
            retry_enabled: true,
            max_retries: 3,
        },
        // Tier 5: Execution
        ServiceEndpoint {
            service_id: "bash-engine".into(),
            host: "localhost".into(),
            port: 8102,
            protocol: WireProtocol::Rest,
            health_path: "/health".into(),
            base_path: "/api".into(),
            timeout_ms: 500_000,
            retry_enabled: true,
            max_retries: 2,
        },
        ServiceEndpoint {
            service_id: "tool-maker".into(),
            host: "localhost".into(),
            port: 8103,
            protocol: WireProtocol::Rest,
            health_path: "/health".into(),
            base_path: "/api".into(),
            timeout_ms: 500_000,
            retry_enabled: true,
            max_retries: 2,
        },
    ]
}

/// Wire weight matrix entry
#[derive(Clone, Debug)]
pub struct WireWeight {
    /// Source service
    pub source: String,
    /// Target service
    pub target: String,
    /// Weight multiplier
    pub weight: f64,
    /// Latency SLO in ms
    pub latency_slo_ms: u64,
    /// Error budget
    pub error_budget: f64,
}

/// Default wire weight matrix
#[must_use]
pub fn default_wire_weights() -> Vec<WireWeight> {
    vec![
        WireWeight { source: "maintenance-engine".into(), target: "synthex".into(), weight: 1.5, latency_slo_ms: 10, error_budget: 0.001 },
        WireWeight { source: "maintenance-engine".into(), target: "san-k7".into(), weight: 1.5, latency_slo_ms: 10, error_budget: 0.001 },
        WireWeight { source: "maintenance-engine".into(), target: "nais".into(), weight: 1.3, latency_slo_ms: 50, error_budget: 0.005 },
        // codesynthor-v7 / devops-engine / tool-library wire weights removed S097 (retired services)
        WireWeight { source: "maintenance-engine".into(), target: "ccm".into(), weight: 1.2, latency_slo_ms: 100, error_budget: 0.01 },
        // library-agent wire weight removed (disabled service)
        WireWeight { source: "maintenance-engine".into(), target: "bash-engine".into(), weight: 1.0, latency_slo_ms: 500, error_budget: 0.02 },
        WireWeight { source: "maintenance-engine".into(), target: "tool-maker".into(), weight: 1.0, latency_slo_ms: 500, error_budget: 0.02 },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_endpoint_url() {
        let endpoint = ServiceEndpoint::new("test", "localhost", 8080);
        assert_eq!(endpoint.url("/status"), "http://localhost:8080/api/status");
        assert_eq!(endpoint.health_url(), "http://localhost:8080/api/health");
    }

    #[test]
    fn test_default_endpoints() {
        let endpoints = default_endpoints();
        assert!(endpoints.len() >= 6); // 6 after library-agent + 3 retired-service removals
        assert!(endpoints.iter().any(|e| e.service_id == "synthex"));
    }

    #[test]
    fn test_wire_weights() {
        let weights = default_wire_weights();
        let synthex_weight = weights.iter().find(|w| w.target == "synthex");
        assert!(synthex_weight.is_some());
        assert!((synthex_weight.map(|w| w.weight).unwrap_or(0.0) - 1.5).abs() < f64::EPSILON);
    }
}
