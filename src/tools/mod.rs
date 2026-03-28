//! # Tool Definitions
//!
//! Tool definitions for registration with the ULTRAPLATE Tool Library.
//! Each tool provides a specific capability that can be invoked via
//! REST API. The Maintenance Engine exposes 15 tools across 6 categories.
//!
//! ## Tool Categories
//!
//! | Category | Count | Description |
//! |----------|-------|-------------|
//! | Observability | 4 | Health checking and service discovery |
//! | Operations | 3 | Remediation and pipeline management |
//! | Intelligence | 2 | Learning cycles and pathway analysis |
//! | Core | 4 | Consensus, voting, and tensor state |
//! | Advanced | 2 | Fitness evaluation and emergence detection |

pub mod health_tools;
pub mod remediation_tools;
pub mod learning_tools;
pub mod consensus_tools;
pub mod observer_tools;
pub mod tensor_tools;

use serde::{Deserialize, Serialize};

/// Tool category for grouping in the Tool Library.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolCategory {
    /// Observability and health checking
    Observability,
    /// Operations and remediation
    Operations,
    /// Intelligence and learning
    Intelligence,
    /// Core consensus and voting
    Core,
    /// Advanced observer features
    Advanced,
}

/// Tool definition for registration with the Tool Library.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolDefinition {
    /// Unique tool ID (e.g., "me-health-check")
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// Description of what the tool does
    pub description: String,
    /// Category for grouping
    pub category: ToolCategory,
    /// Engine layer this tool operates on
    pub layer: String,
    /// HTTP endpoint path (e.g., "/api/tools/health-check")
    pub endpoint: String,
    /// HTTP method (always POST for tool invocations)
    pub method: String,
    /// Service that provides this tool
    pub service: String,
    /// Version of the tool
    pub version: String,
}

/// Request envelope for tool invocations.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolInvokeRequest {
    /// Invocation parameters (tool-specific JSON)
    #[serde(default)]
    pub params: serde_json::Value,
    /// Unique request ID for correlation
    #[serde(default)]
    pub request_id: String,
    /// Timeout in milliseconds (0 = default)
    #[serde(default)]
    pub timeout_ms: u64,
}

/// Response envelope for tool invocations.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolInvokeResponse {
    /// Whether the invocation succeeded
    pub success: bool,
    /// Response data (tool-specific JSON)
    pub data: serde_json::Value,
    /// Duration of the invocation in milliseconds
    pub duration_ms: f64,
    /// Correlation request ID
    pub request_id: String,
    /// Current tensor snapshot (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tensor: Option<[f64; 12]>,
}

/// Get all 15 tool definitions.
#[must_use]
pub fn all_tool_definitions() -> Vec<ToolDefinition> {
    let mut tools = Vec::with_capacity(15);
    tools.extend(health_tools::definitions());
    tools.extend(remediation_tools::definitions());
    tools.extend(learning_tools::definitions());
    tools.extend(consensus_tools::definitions());
    tools.extend(observer_tools::definitions());
    tools.extend(tensor_tools::definitions());
    tools
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_definitions_count() {
        let defs = all_tool_definitions();
        assert_eq!(defs.len(), 15);
    }

    #[test]
    fn test_all_definitions_unique_ids() {
        let defs = all_tool_definitions();
        let mut ids: Vec<&str> = defs.iter().map(|d| d.id.as_str()).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), 15);
    }

    #[test]
    fn test_all_definitions_have_endpoints() {
        let defs = all_tool_definitions();
        for def in &defs {
            assert!(
                def.endpoint.starts_with("/api/tools/"),
                "tool {} endpoint should start with /api/tools/",
                def.id
            );
            assert_eq!(def.method, "POST");
            assert_eq!(def.service, "maintenance-engine");
        }
    }

    #[test]
    fn test_tool_invoke_request_default() {
        let json_str = "{}";
        let req: ToolInvokeRequest =
            serde_json::from_str(json_str).expect("should deserialize empty");
        assert!(req.params.is_null());
        assert!(req.request_id.is_empty());
        assert_eq!(req.timeout_ms, 0);
    }

    #[test]
    fn test_tool_invoke_response_serialization() {
        let resp = ToolInvokeResponse {
            success: true,
            data: serde_json::json!({"status": "ok"}),
            duration_ms: 1.5,
            request_id: "req-1".into(),
            tensor: None,
        };
        let json = serde_json::to_string(&resp).expect("should serialize");
        assert!(json.contains("\"success\":true"));
        assert!(!json.contains("tensor"));
    }
}
