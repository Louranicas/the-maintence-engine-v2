//! # Health Tool Definitions
//!
//! Observability tools for the Maintenance Engine. These tools provide
//! health checking, layer-level health breakdown, service discovery
//! status, and circuit breaker state inspection.
//!
//! All 4 tools operate on Layer 2 (Services).

use super::{ToolCategory, ToolDefinition};

/// Get health/observability tool definitions (4 tools).
#[must_use]
pub fn definitions() -> Vec<ToolDefinition> {
    vec![
        health_check(),
        layer_health(),
        service_discovery(),
        circuit_status(),
    ]
}

/// Create a tool definition with the Observability category.
fn make_def(id: &str, name: &str, description: &str, layer: &str, endpoint: &str) -> ToolDefinition {
    ToolDefinition {
        id: id.into(),
        name: name.into(),
        description: description.into(),
        category: ToolCategory::Observability,
        layer: layer.into(),
        endpoint: endpoint.into(),
        method: "POST".into(),
        service: "maintenance-engine".into(),
        version: "1.0.0".into(),
    }
}

/// Full engine health check across all layers and services.
///
/// Returns aggregate health score, per-service status, uptime,
/// and current 12D tensor snapshot.
fn health_check() -> ToolDefinition {
    make_def(
        "me-health-check",
        "Health Check",
        "Full engine health check across all 7 layers and 13 services. \
         Returns aggregate health score, per-service status, uptime, \
         and current 12D tensor snapshot.",
        "L2",
        "/api/tools/health-check",
    )
}

/// Per-layer health breakdown with module-level granularity.
///
/// Returns health status for each of the 7 layers (L1-L7),
/// including module health within each layer.
fn layer_health() -> ToolDefinition {
    make_def(
        "me-layer-health",
        "Layer Health",
        "Per-layer health breakdown with module-level granularity. \
         Returns health status for each of the 7 layers (L1-L7), \
         including individual module health scores and error counts.",
        "L2",
        "/api/tools/layer-health",
    )
}

/// List all discovered services in the ULTRAPLATE mesh.
///
/// Returns service registry entries with port, tier, protocol,
/// and current connectivity status.
fn service_discovery() -> ToolDefinition {
    make_def(
        "me-service-discovery",
        "Service Discovery",
        "List all discovered services in the ULTRAPLATE mesh. \
         Returns service registry entries with port, tier, protocol, \
         current connectivity status, and last-seen timestamp.",
        "L2",
        "/api/tools/service-discovery",
    )
}

/// Inspect circuit breaker states for all monitored services.
///
/// Returns per-service circuit state (Closed, Open, `HalfOpen`),
/// failure counts, and recovery timestamps.
fn circuit_status() -> ToolDefinition {
    make_def(
        "me-circuit-status",
        "Circuit Status",
        "Inspect circuit breaker states for all monitored services. \
         Returns per-service circuit state (Closed, Open, HalfOpen), \
         failure counts, trip thresholds, and recovery timestamps.",
        "L2",
        "/api/tools/circuit-status",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_definitions_count() {
        assert_eq!(definitions().len(), 4);
    }

    #[test]
    fn test_health_check_id() {
        let defs = definitions();
        assert_eq!(defs[0].id, "me-health-check");
    }

    #[test]
    fn test_layer_health_id() {
        let defs = definitions();
        assert_eq!(defs[1].id, "me-layer-health");
    }

    #[test]
    fn test_service_discovery_id() {
        let defs = definitions();
        assert_eq!(defs[2].id, "me-service-discovery");
    }

    #[test]
    fn test_circuit_status_id() {
        let defs = definitions();
        assert_eq!(defs[3].id, "me-circuit-status");
    }

    #[test]
    fn test_all_observability_category() {
        for def in &definitions() {
            assert_eq!(def.category, ToolCategory::Observability);
        }
    }

    #[test]
    fn test_all_layer_l2() {
        for def in &definitions() {
            assert_eq!(def.layer, "L2");
        }
    }

    #[test]
    fn test_all_post_method() {
        for def in &definitions() {
            assert_eq!(def.method, "POST");
        }
    }

    #[test]
    fn test_all_service_name() {
        for def in &definitions() {
            assert_eq!(def.service, "maintenance-engine");
        }
    }

    #[test]
    fn test_all_version() {
        for def in &definitions() {
            assert_eq!(def.version, "1.0.0");
        }
    }

    #[test]
    fn test_all_endpoints_start_with_prefix() {
        for def in &definitions() {
            assert!(
                def.endpoint.starts_with("/api/tools/"),
                "endpoint {} should start with /api/tools/",
                def.endpoint
            );
        }
    }

    #[test]
    fn test_unique_ids() {
        let defs = definitions();
        let mut ids: Vec<&str> = defs.iter().map(|d| d.id.as_str()).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), 4);
    }

    #[test]
    fn test_unique_endpoints() {
        let defs = definitions();
        let mut endpoints: Vec<&str> = defs.iter().map(|d| d.endpoint.as_str()).collect();
        endpoints.sort_unstable();
        endpoints.dedup();
        assert_eq!(endpoints.len(), 4);
    }

    #[test]
    fn test_all_have_description() {
        for def in &definitions() {
            assert!(!def.description.is_empty(), "tool {} should have a description", def.id);
        }
    }

    #[test]
    fn test_all_have_name() {
        for def in &definitions() {
            assert!(!def.name.is_empty(), "tool {} should have a name", def.id);
        }
    }

    #[test]
    fn test_serialization_roundtrip() {
        let defs = definitions();
        let json = serde_json::to_string(&defs).expect("should serialize");
        let deserialized: Vec<ToolDefinition> =
            serde_json::from_str(&json).expect("should deserialize");
        assert_eq!(deserialized.len(), 4);
        assert_eq!(deserialized[0].id, "me-health-check");
    }
}
