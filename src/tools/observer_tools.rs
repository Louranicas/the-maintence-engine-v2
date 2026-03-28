//! # Observer Tool Definitions
//!
//! Advanced observer tools for the Maintenance Engine. These tools provide
//! fitness evaluation snapshots and emergence detection reports from
//! the L7 Observer layer.
//!
//! Both tools operate on Layer 7 (Observer).

use super::{ToolCategory, ToolDefinition};

/// Get observer/advanced tool definitions (2 tools).
#[must_use]
pub fn definitions() -> Vec<ToolDefinition> {
    vec![
        fitness_snapshot(),
        emergence_report(),
    ]
}

/// Create a tool definition with the Advanced category.
fn make_def(id: &str, name: &str, description: &str, layer: &str, endpoint: &str) -> ToolDefinition {
    ToolDefinition {
        id: id.into(),
        name: name.into(),
        description: description.into(),
        category: ToolCategory::Advanced,
        layer: layer.into(),
        endpoint: endpoint.into(),
        method: "POST".into(),
        service: "maintenance-engine".into(),
        version: "1.0.0".into(),
    }
}

/// Get a current fitness evaluation snapshot.
///
/// Returns the 12D tensor fitness evaluation including per-dimension
/// scores, overall fitness, trend direction, and homeostatic targets.
fn fitness_snapshot() -> ToolDefinition {
    make_def(
        "me-fitness-snapshot",
        "Fitness Snapshot",
        "Get the current 12D tensor fitness evaluation snapshot. Returns \
         per-dimension fitness scores, overall fitness value, trend direction \
         (improving/stable/degrading), homeostatic target compliance, \
         and recent fitness history.",
        "L7",
        "/api/tools/fitness-snapshot",
    )
}

/// Get recent emergence detection report.
///
/// Returns detected emergence events including cascade patterns,
/// synergy amplifications, resonance detections, and phase transitions
/// observed by the M38 emergence detector.
fn emergence_report() -> ToolDefinition {
    make_def(
        "me-emergence-report",
        "Emergence Report",
        "Get recent emergence detection report from the M38 emergence detector. \
         Returns detected emergence events including cascade patterns, \
         synergy amplifications, resonance detections, phase transitions, \
         and RALPH evolution loop status.",
        "L7",
        "/api/tools/emergence-report",
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
    fn test_fitness_snapshot_id() {
        let defs = definitions();
        assert_eq!(defs[0].id, "me-fitness-snapshot");
    }

    #[test]
    fn test_emergence_report_id() {
        let defs = definitions();
        assert_eq!(defs[1].id, "me-emergence-report");
    }

    #[test]
    fn test_all_advanced_category() {
        for def in &definitions() {
            assert_eq!(def.category, ToolCategory::Advanced);
        }
    }

    #[test]
    fn test_all_layer_l7() {
        for def in &definitions() {
            assert_eq!(def.layer, "L7");
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
        assert_eq!(deserialized[0].id, "me-fitness-snapshot");
    }
}
