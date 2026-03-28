//! # Tensor Tool Definitions
//!
//! Core tensor tools for the Maintenance Engine. These tools provide
//! 12D tensor state inspection and comparison capabilities.
//!
//! Both tools operate on Layer 1 (Foundation).

use super::{ToolCategory, ToolDefinition};

/// Get tensor/core tool definitions (2 tools).
#[must_use]
pub fn definitions() -> Vec<ToolDefinition> {
    vec![
        tensor_snapshot(),
        tensor_compare(),
    ]
}

/// Create a tool definition with the Core category.
fn make_def(id: &str, name: &str, description: &str, layer: &str, endpoint: &str) -> ToolDefinition {
    ToolDefinition {
        id: id.into(),
        name: name.into(),
        description: description.into(),
        category: ToolCategory::Core,
        layer: layer.into(),
        endpoint: endpoint.into(),
        method: "POST".into(),
        service: "maintenance-engine".into(),
        version: "1.0.0".into(),
    }
}

/// Get the current 12D tensor state snapshot.
///
/// Returns the full 12-dimensional tensor encoding:
/// `[service_id, port, tier, deps, agents, protocol, health,
///  uptime, synergy, latency, error_rate, temporal]`
/// along with validation status and per-dimension labels.
fn tensor_snapshot() -> ToolDefinition {
    make_def(
        "me-tensor-snapshot",
        "Tensor Snapshot",
        "Get the current 12D tensor state snapshot. Returns the full \
         12-dimensional encoding [service_id, port, tier, deps, agents, \
         protocol, health, uptime, synergy, latency, error_rate, temporal] \
         with validation status, per-dimension labels, and timestamp.",
        "L1",
        "/api/tools/tensor-snapshot",
    )
}

/// Compare two tensor states by computing distance and delta.
///
/// Accepts two tensor state identifiers (timestamps or snapshot IDs)
/// and returns Euclidean distance, per-dimension deltas, drift
/// classification, and anomaly flags.
fn tensor_compare() -> ToolDefinition {
    make_def(
        "me-tensor-compare",
        "Tensor Compare",
        "Compare two 12D tensor states. Accepts two tensor snapshot \
         identifiers (timestamps or IDs) and returns Euclidean distance, \
         per-dimension deltas, drift classification (stable/drifting/anomalous), \
         and anomaly flags for each dimension.",
        "L1",
        "/api/tools/tensor-compare",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_definitions_count() {
        assert_eq!(definitions().len(), 2);
    }

    #[test]
    fn test_tensor_snapshot_id() {
        let defs = definitions();
        assert_eq!(defs[0].id, "me-tensor-snapshot");
    }

    #[test]
    fn test_tensor_compare_id() {
        let defs = definitions();
        assert_eq!(defs[1].id, "me-tensor-compare");
    }

    #[test]
    fn test_all_core_category() {
        for def in &definitions() {
            assert_eq!(def.category, ToolCategory::Core);
        }
    }

    #[test]
    fn test_all_layer_l1() {
        for def in &definitions() {
            assert_eq!(def.layer, "L1");
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
    fn test_unique_ids() {
        let defs = definitions();
        let mut ids: Vec<&str> = defs.iter().map(|d| d.id.as_str()).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), 2);
    }

    #[test]
    fn test_unique_endpoints() {
        let defs = definitions();
        let mut endpoints: Vec<&str> = defs.iter().map(|d| d.endpoint.as_str()).collect();
        endpoints.sort_unstable();
        endpoints.dedup();
        assert_eq!(endpoints.len(), 2);
    }

    #[test]
    fn test_serialization_roundtrip() {
        let defs = definitions();
        let json = serde_json::to_string(&defs).expect("should serialize");
        let deserialized: Vec<ToolDefinition> =
            serde_json::from_str(&json).expect("should deserialize");
        assert_eq!(deserialized.len(), 2);
        assert_eq!(deserialized[0].id, "me-tensor-snapshot");
    }
}
