//! # Learning Tool Definitions
//!
//! Intelligence tools for the Maintenance Engine. These tools provide
//! learning cycle execution and Hebbian pathway analysis.
//!
//! Both tools operate on Layer 5 (Learning).

use super::{ToolCategory, ToolDefinition};

/// Get learning/intelligence tool definitions (2 tools).
#[must_use]
pub fn definitions() -> Vec<ToolDefinition> {
    vec![
        learning_cycle(),
        pathway_analysis(),
    ]
}

/// Create a tool definition with the Intelligence category.
fn make_def(id: &str, name: &str, description: &str, layer: &str, endpoint: &str) -> ToolDefinition {
    ToolDefinition {
        id: id.into(),
        name: name.into(),
        description: description.into(),
        category: ToolCategory::Intelligence,
        layer: layer.into(),
        endpoint: endpoint.into(),
        method: "POST".into(),
        service: "maintenance-engine".into(),
        version: "1.0.0".into(),
    }
}

/// Execute a learning cycle using STDP parameters.
///
/// Triggers a full Hebbian learning cycle: pathway activation,
/// LTP/LTD weight updates, pruning, and memory consolidation.
/// Returns updated pathway weights and learning metrics.
fn learning_cycle() -> ToolDefinition {
    make_def(
        "me-learning-cycle",
        "Learning Cycle",
        "Execute a Hebbian learning cycle using STDP parameters \
         (LTP=0.1, LTD=0.05, window=100ms). Triggers pathway activation, \
         weight updates, pruning, and memory consolidation. Returns \
         updated pathway weights and learning metrics.",
        "L5",
        "/api/tools/learning-cycle",
    )
}

/// Analyze current Hebbian pathway state.
///
/// Returns pathway graph with weights, activation counts,
/// LTP/LTD ratios, pruning candidates, and anti-pattern
/// detections across the learning subsystem.
fn pathway_analysis() -> ToolDefinition {
    make_def(
        "me-pathway-analysis",
        "Pathway Analysis",
        "Analyze current Hebbian pathway state. Returns pathway graph \
         with weights, activation counts, LTP/LTD ratios, pruning candidates, \
         and anti-pattern detections across the learning subsystem.",
        "L5",
        "/api/tools/pathway-analysis",
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
    fn test_learning_cycle_id() {
        let defs = definitions();
        assert_eq!(defs[0].id, "me-learning-cycle");
    }

    #[test]
    fn test_pathway_analysis_id() {
        let defs = definitions();
        assert_eq!(defs[1].id, "me-pathway-analysis");
    }

    #[test]
    fn test_all_intelligence_category() {
        for def in &definitions() {
            assert_eq!(def.category, ToolCategory::Intelligence);
        }
    }

    #[test]
    fn test_all_layer_l5() {
        for def in &definitions() {
            assert_eq!(def.layer, "L5");
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
        assert_eq!(deserialized[0].id, "me-learning-cycle");
    }
}
