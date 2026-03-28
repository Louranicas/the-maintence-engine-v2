//! # Integration Tests: Tool Definitions & Tool Registrar
//!
//! Comprehensive integration tests covering the 15 Maintenance Engine
//! tool definitions across 6 categories, plus the `ToolRegistrar` from
//! L4 Integration. All tests are deterministic and require no external
//! services.

mod common;

use std::collections::HashSet;

use maintenance_engine::m4_integration::tool_registrar::ToolRegistrar;
use maintenance_engine::tools::{
    all_tool_definitions, ToolCategory, ToolDefinition, ToolInvokeRequest, ToolInvokeResponse,
};
use maintenance_engine::tools::{
    consensus_tools, health_tools, learning_tools, observer_tools, remediation_tools, tensor_tools,
};

// =========================================================================
// 1. Tool Definitions: Global Properties
// =========================================================================

#[test]
fn tool_definitions_returns_exactly_15() {
    let defs = all_tool_definitions();
    assert_eq!(
        defs.len(),
        15,
        "all_tool_definitions() must return exactly 15 tools"
    );
}

#[test]
fn tool_definitions_have_unique_ids() {
    let defs = all_tool_definitions();
    let ids: HashSet<&str> = defs.iter().map(|d| d.id.as_str()).collect();
    assert_eq!(
        ids.len(),
        15,
        "all 15 tool IDs must be unique, found {} distinct",
        ids.len()
    );
}

#[test]
fn tool_definitions_all_endpoints_start_with_api_tools() {
    let defs = all_tool_definitions();
    for def in &defs {
        assert!(
            def.endpoint.starts_with("/api/tools/"),
            "tool '{}' endpoint '{}' must start with /api/tools/",
            def.id,
            def.endpoint
        );
    }
}

#[test]
fn tool_definitions_all_service_is_maintenance_engine() {
    let defs = all_tool_definitions();
    for def in &defs {
        assert_eq!(
            def.service,
            "maintenance-engine",
            "tool '{}' service must be 'maintenance-engine', got '{}'",
            def.id,
            def.service
        );
    }
}

#[test]
fn tool_definitions_all_method_is_post() {
    let defs = all_tool_definitions();
    for def in &defs {
        assert_eq!(
            def.method,
            "POST",
            "tool '{}' method must be POST, got '{}'",
            def.id,
            def.method
        );
    }
}

#[test]
fn tool_definitions_all_have_nonempty_fields() {
    let defs = all_tool_definitions();
    for def in &defs {
        assert!(!def.id.is_empty(), "tool ID must not be empty");
        assert!(
            !def.name.is_empty(),
            "tool '{}' name must not be empty",
            def.id
        );
        assert!(
            !def.description.is_empty(),
            "tool '{}' description must not be empty",
            def.id
        );
        assert!(
            !def.layer.is_empty(),
            "tool '{}' layer must not be empty",
            def.id
        );
        assert!(
            !def.version.is_empty(),
            "tool '{}' version must not be empty",
            def.id
        );
    }
}

// =========================================================================
// 2. Per-Category Counts
// =========================================================================

#[test]
fn health_tools_returns_4_definitions() {
    let defs = health_tools::definitions();
    assert_eq!(defs.len(), 4, "health_tools should return 4 definitions");
    for def in &defs {
        assert_eq!(def.category, ToolCategory::Observability);
    }
}

#[test]
fn remediation_tools_returns_3_definitions() {
    let defs = remediation_tools::definitions();
    assert_eq!(defs.len(), 3, "remediation_tools should return 3 definitions");
    for def in &defs {
        assert_eq!(def.category, ToolCategory::Operations);
    }
}

#[test]
fn learning_tools_returns_2_definitions() {
    let defs = learning_tools::definitions();
    assert_eq!(defs.len(), 2, "learning_tools should return 2 definitions");
    for def in &defs {
        assert_eq!(def.category, ToolCategory::Intelligence);
    }
}

#[test]
fn consensus_tools_returns_2_definitions() {
    let defs = consensus_tools::definitions();
    assert_eq!(defs.len(), 2, "consensus_tools should return 2 definitions");
    for def in &defs {
        assert_eq!(def.category, ToolCategory::Core);
    }
}

#[test]
fn observer_tools_returns_2_definitions() {
    let defs = observer_tools::definitions();
    assert_eq!(defs.len(), 2, "observer_tools should return 2 definitions");
    for def in &defs {
        assert_eq!(def.category, ToolCategory::Advanced);
    }
}

#[test]
fn tensor_tools_returns_2_definitions() {
    let defs = tensor_tools::definitions();
    assert_eq!(defs.len(), 2, "tensor_tools should return 2 definitions");
    for def in &defs {
        assert_eq!(def.category, ToolCategory::Core);
    }
}

// =========================================================================
// 3. ToolCategory Serialization
// =========================================================================

#[test]
fn tool_category_serializes_to_snake_case() {
    let cases = vec![
        (ToolCategory::Observability, r#""observability""#),
        (ToolCategory::Operations, r#""operations""#),
        (ToolCategory::Intelligence, r#""intelligence""#),
        (ToolCategory::Core, r#""core""#),
        (ToolCategory::Advanced, r#""advanced""#),
    ];
    for (category, expected) in cases {
        let json = serde_json::to_string(&category)
            .expect("ToolCategory should serialize to JSON");
        assert_eq!(json, expected);
    }
}

#[test]
fn tool_category_deserializes_from_snake_case() {
    let cases = vec![
        (r#""observability""#, ToolCategory::Observability),
        (r#""operations""#, ToolCategory::Operations),
        (r#""intelligence""#, ToolCategory::Intelligence),
        (r#""core""#, ToolCategory::Core),
        (r#""advanced""#, ToolCategory::Advanced),
    ];
    for (json, expected) in cases {
        let parsed: ToolCategory = serde_json::from_str(json)
            .expect("ToolCategory should deserialize from snake_case JSON");
        assert_eq!(parsed, expected);
    }
}

#[test]
fn all_tool_categories_covered_by_definitions() {
    let defs = all_tool_definitions();
    let mut categories: Vec<ToolCategory> = defs.iter().map(|d| d.category).collect();
    categories.dedup();
    assert!(categories.contains(&ToolCategory::Observability));
    assert!(categories.contains(&ToolCategory::Operations));
    assert!(categories.contains(&ToolCategory::Intelligence));
    assert!(categories.contains(&ToolCategory::Core));
    assert!(categories.contains(&ToolCategory::Advanced));
}

// =========================================================================
// 4. ToolInvokeRequest
// =========================================================================

#[test]
fn tool_invoke_request_default_deserialization() {
    let req: ToolInvokeRequest =
        serde_json::from_str("{}").expect("empty JSON should deserialize to ToolInvokeRequest");
    assert!(req.params.is_null(), "default params should be null");
    assert!(req.request_id.is_empty(), "default request_id should be empty");
    assert_eq!(req.timeout_ms, 0, "default timeout_ms should be 0");
}

#[test]
fn tool_invoke_request_with_params() {
    let json_str = r#"{"params":{"service_id":"synthex","severity":"high"},"request_id":"req-42","timeout_ms":5000}"#;
    let req: ToolInvokeRequest =
        serde_json::from_str(json_str).expect("ToolInvokeRequest should deserialize with params");
    assert_eq!(req.params["service_id"], "synthex");
    assert_eq!(req.params["severity"], "high");
    assert_eq!(req.request_id, "req-42");
    assert_eq!(req.timeout_ms, 5000);
}

#[test]
fn tool_invoke_request_roundtrip() {
    let original = ToolInvokeRequest {
        params: serde_json::json!({"target": "san-k7"}),
        request_id: "roundtrip-1".into(),
        timeout_ms: 3000,
    };
    let json = serde_json::to_string(&original)
        .expect("ToolInvokeRequest should serialize");
    let restored: ToolInvokeRequest = serde_json::from_str(&json)
        .expect("ToolInvokeRequest should deserialize from its own JSON");
    assert_eq!(restored.request_id, "roundtrip-1");
    assert_eq!(restored.timeout_ms, 3000);
    assert_eq!(restored.params["target"], "san-k7");
}

// =========================================================================
// 5. ToolInvokeResponse
// =========================================================================

#[test]
fn tool_invoke_response_success_without_tensor() {
    let resp = ToolInvokeResponse {
        success: true,
        data: serde_json::json!({"status": "healthy", "score": 0.95}),
        duration_ms: 2.3,
        request_id: "resp-1".into(),
        tensor: None,
    };
    let json = serde_json::to_string(&resp)
        .expect("ToolInvokeResponse should serialize");
    assert!(json.contains(r#""success":true"#), "JSON should contain success:true");
    assert!(!json.contains("tensor"), "tensor field should be skipped when None");
}

#[test]
fn tool_invoke_response_failure_construction() {
    let resp = ToolInvokeResponse {
        success: false,
        data: serde_json::json!({"error": "service unreachable"}),
        duration_ms: 100.0,
        request_id: "resp-err".into(),
        tensor: None,
    };
    assert!(!resp.success);
    assert_eq!(resp.data["error"], "service unreachable");
    assert_eq!(resp.request_id, "resp-err");
}

#[test]
fn tool_invoke_response_with_tensor() {
    let tensor_data: [f64; 12] =
        [0.5, 0.5, 0.3, 0.3, 0.5, 0.0, 0.95, 0.99, 0.9, 0.95, 0.02, 0.5];
    let resp = ToolInvokeResponse {
        success: true,
        data: serde_json::json!({"health": 0.95}),
        duration_ms: 1.0,
        request_id: "resp-tensor".into(),
        tensor: Some(tensor_data),
    };
    let json = serde_json::to_string(&resp)
        .expect("ToolInvokeResponse with tensor should serialize");
    assert!(json.contains("tensor"), "JSON should contain tensor field when Some");
    let restored: ToolInvokeResponse = serde_json::from_str(&json)
        .expect("ToolInvokeResponse with tensor should round-trip");
    let t = restored.tensor.expect("restored tensor should be Some");
    assert!((t[6] - 0.95).abs() < f64::EPSILON, "tensor D6 health should be 0.95");
    assert!((t[10] - 0.02).abs() < f64::EPSILON, "tensor D10 error_rate should be 0.02");
}

// =========================================================================
// 6. ToolRegistrar
// =========================================================================

#[test]
fn tool_registrar_new_creates_with_port() {
    let registrar =
        ToolRegistrar::new(8080).expect("ToolRegistrar::new(8080) should succeed");
    assert_eq!(registrar.service_port(), 8080, "service_port should be 8080");
}

#[test]
fn tool_registrar_tool_count_is_15() {
    let registrar =
        ToolRegistrar::new(8080).expect("ToolRegistrar::new should succeed");
    assert_eq!(registrar.tool_count(), 15, "registrar should manage exactly 15 tools");
}

#[test]
fn tool_registrar_initially_not_registered() {
    let registrar =
        ToolRegistrar::new(8080).expect("ToolRegistrar::new should succeed");
    assert!(!registrar.is_registered(), "registrar should not be registered initially");
    assert_eq!(registrar.registered_count(), 0, "registered_count should be 0 initially");
}

#[test]
fn tool_registrar_build_payload_has_all_tools() {
    let registrar =
        ToolRegistrar::new(8080).expect("ToolRegistrar::new should succeed");
    let payload = registrar.build_payload();
    assert_eq!(payload.tool_count(), 15, "build_payload should contain 15 tool entries");
    assert_eq!(payload.service_id, "maintenance-engine");
    assert_eq!(payload.host, "localhost");
    assert_eq!(payload.port, 8080);
}

#[test]
fn tool_registrar_registration_report_all_pending() {
    let registrar =
        ToolRegistrar::new(8080).expect("ToolRegistrar::new should succeed");
    let report = registrar.registration_report();
    assert_eq!(report.total_tools, 15);
    assert_eq!(report.registered_count, 0);
    assert_eq!(report.failed_count, 0);
    assert_eq!(report.pending_count, 15, "all 15 tools should be pending initially");
    assert!(!report.all_registered(), "all_registered should be false initially");
    assert!(!report.has_failures(), "has_failures should be false initially");
}

#[test]
fn tool_registrar_registration_ratio_zero_initially() {
    let registrar =
        ToolRegistrar::new(8080).expect("ToolRegistrar::new should succeed");
    let report = registrar.registration_report();
    assert!(
        report.registration_ratio().abs() < f64::EPSILON,
        "registration_ratio should be 0.0 initially, got {}",
        report.registration_ratio()
    );
}

#[test]
fn tool_registrar_custom_port_in_payload() {
    let registrar =
        ToolRegistrar::new(9090).expect("ToolRegistrar::new(9090) should succeed");
    let payload = registrar.build_payload();
    assert_eq!(payload.port, 9090, "payload port should reflect custom port 9090");
}

// =========================================================================
// 7. Cross-Module Consistency
// =========================================================================

#[test]
fn tool_ids_match_endpoint_paths() {
    let defs = all_tool_definitions();
    for def in &defs {
        assert!(
            def.id.starts_with("me-"),
            "tool ID '{}' should start with 'me-'",
            def.id
        );
        let id_suffix = &def.id["me-".len()..];
        let expected_endpoint = format!("/api/tools/{id_suffix}");
        assert_eq!(
            def.endpoint, expected_endpoint,
            "tool '{}' endpoint should be '{}', got '{}'",
            def.id, expected_endpoint, def.endpoint
        );
    }
}

#[test]
fn layer_assignments_are_correct() {
    for def in &health_tools::definitions() {
        assert_eq!(def.layer, "L2", "health tool '{}' should be on L2", def.id);
    }
    for def in &remediation_tools::definitions() {
        assert_eq!(def.layer, "L3", "remediation tool '{}' should be on L3", def.id);
    }
    for def in &learning_tools::definitions() {
        assert_eq!(def.layer, "L5", "learning tool '{}' should be on L5", def.id);
    }
    for def in &consensus_tools::definitions() {
        assert_eq!(def.layer, "L6", "consensus tool '{}' should be on L6", def.id);
    }
    for def in &observer_tools::definitions() {
        assert_eq!(def.layer, "L7", "observer tool '{}' should be on L7", def.id);
    }
    for def in &tensor_tools::definitions() {
        assert_eq!(def.layer, "L1", "tensor tool '{}' should be on L1", def.id);
    }
}

#[test]
fn all_definitions_serialization_roundtrip() {
    let defs = all_tool_definitions();
    let json = serde_json::to_string(&defs)
        .expect("all_tool_definitions should serialize to JSON");
    let restored: Vec<ToolDefinition> = serde_json::from_str(&json)
        .expect("all_tool_definitions JSON should deserialize back");
    assert_eq!(restored.len(), 15, "round-tripped definitions should still have 15 entries");
    assert_eq!(restored[0].id, defs[0].id);
    assert_eq!(restored[14].id, defs[14].id);
}

#[test]
fn build_payload_produces_valid_json() {
    let registrar =
        ToolRegistrar::new(8080).expect("ToolRegistrar::new should succeed");
    let payload = registrar.build_payload();
    let json = serde_json::to_string_pretty(&payload)
        .expect("build_payload should produce valid JSON");

    let value: serde_json::Value = serde_json::from_str(&json)
        .expect("payload JSON should parse as a Value");

    assert_eq!(value["service_id"], "maintenance-engine");
    assert_eq!(value["port"], 8080);
    let tools_array = value["tools"]
        .as_array()
        .expect("payload JSON should have a 'tools' array");
    assert_eq!(tools_array.len(), 15, "payload JSON tools array should have 15 entries");

    for entry in tools_array {
        assert!(entry["id"].is_string(), "tool entry should have a string 'id'");
        assert!(entry["endpoint"].is_string(), "tool entry should have a string 'endpoint'");
        assert_eq!(entry["method"], "POST", "tool entry method should be POST");
    }
}

#[test]
fn unique_endpoints_across_all_tools() {
    let defs = all_tool_definitions();
    let endpoints: HashSet<&str> = defs.iter().map(|d| d.endpoint.as_str()).collect();
    assert_eq!(
        endpoints.len(),
        15,
        "all 15 tool endpoints must be unique, found {} distinct",
        endpoints.len()
    );
}
