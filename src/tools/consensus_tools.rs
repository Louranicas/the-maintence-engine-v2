//! # Consensus Tool Definitions
//!
//! Core consensus tools for the Maintenance Engine. These tools provide
//! PBFT proposal listing and vote submission capabilities.
//!
//! Both tools operate on Layer 6 (Consensus).

use super::{ToolCategory, ToolDefinition};

/// Get consensus/core tool definitions (2 tools).
#[must_use]
pub fn definitions() -> Vec<ToolDefinition> {
    vec![
        view_proposals(),
        submit_vote(),
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

/// List active PBFT consensus proposals.
///
/// Returns all active proposals including proposal ID, proposer,
/// current vote tally, quorum status (27/40), and time remaining.
fn view_proposals() -> ToolDefinition {
    make_def(
        "me-view-proposals",
        "View Proposals",
        "List active PBFT consensus proposals. Returns all active proposals \
         including proposal ID, proposer agent, current vote tally, \
         quorum status (27/40 required), dissent records, and time remaining.",
        "L6",
        "/api/tools/view-proposals",
    )
}

/// Submit a vote on an active PBFT proposal.
///
/// Accepts proposal ID, vote (approve/reject), agent ID, and
/// optional dissent rationale. Vote weight is determined by
/// agent role (Validator=1.0, Critic=1.2, Explorer=0.8).
fn submit_vote() -> ToolDefinition {
    make_def(
        "me-submit-vote",
        "Submit Vote",
        "Submit a vote on an active PBFT consensus proposal. Accepts \
         proposal ID, vote (approve/reject), agent ID, and optional \
         dissent rationale. Vote weight determined by agent role \
         (Validator=1.0, Critic=1.2, Explorer=0.8).",
        "L6",
        "/api/tools/submit-vote",
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
    fn test_view_proposals_id() {
        let defs = definitions();
        assert_eq!(defs[0].id, "me-view-proposals");
    }

    #[test]
    fn test_submit_vote_id() {
        let defs = definitions();
        assert_eq!(defs[1].id, "me-submit-vote");
    }

    #[test]
    fn test_all_core_category() {
        for def in &definitions() {
            assert_eq!(def.category, ToolCategory::Core);
        }
    }

    #[test]
    fn test_all_layer_l6() {
        for def in &definitions() {
            assert_eq!(def.layer, "L6");
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
        assert_eq!(deserialized[0].id, "me-view-proposals");
    }
}
