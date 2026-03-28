//! # Remediation Tool Definitions
//!
//! Operations tools for the Maintenance Engine. These tools provide
//! remediation submission, queue status inspection, and pipeline
//! manager status.
//!
//! All 3 tools operate on Layer 3 (Core Logic).

use super::{ToolCategory, ToolDefinition};

/// Get remediation/operations tool definitions (3 tools).
#[must_use]
pub fn definitions() -> Vec<ToolDefinition> {
    vec![
        submit_remediation(),
        remediation_status(),
        pipeline_status(),
    ]
}

/// Create a tool definition with the Operations category.
fn make_def(id: &str, name: &str, description: &str, layer: &str, endpoint: &str) -> ToolDefinition {
    ToolDefinition {
        id: id.into(),
        name: name.into(),
        description: description.into(),
        category: ToolCategory::Operations,
        layer: layer.into(),
        endpoint: endpoint.into(),
        method: "POST".into(),
        service: "maintenance-engine".into(),
        version: "1.0.0".into(),
    }
}

/// Submit a remediation request for a service or component.
///
/// Accepts a target service ID, remediation action type,
/// severity level, and optional parameters. The request is
/// routed through the escalation tier system (L0-L3).
fn submit_remediation() -> ToolDefinition {
    make_def(
        "me-submit-remediation",
        "Submit Remediation",
        "Submit a remediation request for a service or component. \
         Accepts target service ID, remediation action type, severity level, \
         and optional parameters. Routed through escalation tiers (L0-L3) \
         based on confidence and severity.",
        "L3",
        "/api/tools/submit-remediation",
    )
}

/// Get the current remediation queue status.
///
/// Returns the number of pending, active, and completed
/// remediation requests, grouped by escalation tier and severity.
fn remediation_status() -> ToolDefinition {
    make_def(
        "me-remediation-status",
        "Remediation Status",
        "Get the current remediation queue status. Returns the number \
         of pending, active, and completed remediation requests, grouped \
         by escalation tier (L0-L3) and severity level.",
        "L3",
        "/api/tools/remediation-status",
    )
}

/// Get pipeline manager status across all 8 core pipelines.
///
/// Returns per-pipeline health, throughput, SLO compliance,
/// and current execution statistics.
fn pipeline_status() -> ToolDefinition {
    make_def(
        "me-pipeline-status",
        "Pipeline Status",
        "Get pipeline manager status across all 8 core pipelines. \
         Returns per-pipeline health, throughput, SLO compliance, \
         current execution statistics, and recent error summaries.",
        "L3",
        "/api/tools/pipeline-status",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_definitions_count() {
        assert_eq!(definitions().len(), 3);
    }

    #[test]
    fn test_submit_remediation_id() {
        let defs = definitions();
        assert_eq!(defs[0].id, "me-submit-remediation");
    }

    #[test]
    fn test_remediation_status_id() {
        let defs = definitions();
        assert_eq!(defs[1].id, "me-remediation-status");
    }

    #[test]
    fn test_pipeline_status_id() {
        let defs = definitions();
        assert_eq!(defs[2].id, "me-pipeline-status");
    }

    #[test]
    fn test_all_operations_category() {
        for def in &definitions() {
            assert_eq!(def.category, ToolCategory::Operations);
        }
    }

    #[test]
    fn test_all_layer_l3() {
        for def in &definitions() {
            assert_eq!(def.layer, "L3");
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
        assert_eq!(ids.len(), 3);
    }

    #[test]
    fn test_unique_endpoints() {
        let defs = definitions();
        let mut endpoints: Vec<&str> = defs.iter().map(|d| d.endpoint.as_str()).collect();
        endpoints.sort_unstable();
        endpoints.dedup();
        assert_eq!(endpoints.len(), 3);
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
        assert_eq!(deserialized.len(), 3);
        assert_eq!(deserialized[0].id, "me-submit-remediation");
    }
}
