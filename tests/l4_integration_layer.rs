//! # L4 Integration Layer - Cross-Module Integration Tests
//!
//! Comprehensive tests covering interactions between `EventBus`, `BridgeManager`,
//! `RestClient`, `GrpcClient`, `WebSocketClient`, `IpcManager`, `ServiceEndpoint`,
//! `WireProtocol`, `ToolRegistrar`, and cross-module event flows.
//!
//! ## Test Categories
//!
//! 1. `EventBus`: channels, subscriptions, publishing, filtering, history
//! 2. `BridgeManager`: registration, health, synergy, wire weights
//! 3. `RestClient`: endpoints, simulated requests, circuit breaker
//! 4. gRPC/WebSocket/IPC: creation, basic state
//! 5. `ServiceEndpoint`: URL construction, default endpoints
//! 6. `WireProtocol`: timeouts, weight matrix
//! 7. `ToolRegistrar`: creation, payload, reporting, tool status
//! 8. Cross-module: event bus to bridge flow, endpoint to bridge mapping

mod common;

use maintenance_engine::m4_integration::*;
use maintenance_engine::*;

// =========================================================================
// 1. EventBus Tests
// =========================================================================

#[test]
fn event_bus_default_has_six_channels() {
    let bus = EventBus::new();
    assert_eq!(
        bus.channel_count(),
        6,
        "EventBus must initialise with exactly 6 default channels"
    );
}

#[test]
fn event_bus_default_channel_names() {
    let bus = EventBus::new();
    let channels = bus.list_channels();
    let expected = ["health", "remediation", "learning", "consensus", "integration", "metrics"];
    for name in &expected {
        assert!(
            channels.contains(&(*name).to_string()),
            "default channel '{name}' missing from `EventBus`"
        );
    }
}

#[test]
fn event_bus_create_custom_channel() {
    let bus = EventBus::new();
    bus.create_channel("custom-alerts")
        .expect("creating a new channel should succeed");
    assert_eq!(
        bus.channel_count(),
        7,
        "channel count should be 7 after adding one custom channel"
    );
    let info = bus
        .get_channel_info("custom-alerts")
        .expect("custom channel info should be retrievable");
    assert_eq!(info.name, "custom-alerts");
    assert_eq!(info.event_count, 0);
    assert_eq!(info.subscriber_count, 0);
}

#[test]
fn event_bus_duplicate_channel_rejected() {
    let bus = EventBus::new();
    let result = bus.create_channel("health");
    assert!(
        result.is_err(),
        "creating a duplicate channel must return an error"
    );
}

#[test]
fn event_bus_subscribe_and_unsubscribe() {
    let bus = EventBus::new();
    bus.subscribe("agent-alpha", "health", None)
        .expect("subscribing to an existing channel should succeed");
    bus.subscribe("agent-beta", "health", None)
        .expect("a second subscriber should succeed");

    let subs = bus.get_subscribers("health");
    assert_eq!(subs.len(), 2, "two subscribers expected on health channel");

    bus.unsubscribe("agent-alpha", "health")
        .expect("unsubscribing should succeed");
    let subs_after = bus.get_subscribers("health");
    assert_eq!(subs_after.len(), 1, "one subscriber should remain after unsubscribe");
    assert_eq!(subs_after[0].subscriber_id, "agent-beta");
}

#[test]
fn event_bus_publish_event_delivered_to_subscribers() {
    let bus = EventBus::new();
    bus.subscribe("watcher-1", "metrics", None)
        .expect("subscribe should succeed");
    bus.subscribe("watcher-2", "metrics", None)
        .expect("subscribe should succeed");

    let record = bus
        .publish("metrics", "cpu_spike", r#"{"cpu":92.5}"#, "monitor-agent")
        .expect("publishing to a valid channel should succeed");

    assert_eq!(record.channel, "metrics");
    assert_eq!(record.event_type, "cpu_spike");
    assert_eq!(record.source, "monitor-agent");
    assert_eq!(
        record.delivered_to.len(),
        2,
        "event should be delivered to both subscribers"
    );
    assert!(record.delivered_to.contains(&"watcher-1".to_string()));
    assert!(record.delivered_to.contains(&"watcher-2".to_string()));
}

#[test]
fn event_bus_filtered_subscription_delivery() {
    let bus = EventBus::new();
    bus.subscribe("all-watcher", "health", None)
        .expect("subscribe without filter should succeed");
    bus.subscribe("critical-only", "health", Some("critical".into()))
        .expect("subscribe with filter should succeed");

    // Non-critical event: only all-watcher receives
    let info_event = bus
        .publish("health", "info", r#"{"msg":"ok"}"#, "monitor")
        .expect("publish info event should succeed");
    assert_eq!(info_event.delivered_to.len(), 1);
    assert!(info_event.delivered_to.contains(&"all-watcher".to_string()));

    // Critical event: both receive
    let critical_event = bus
        .publish("health", "critical", r#"{"msg":"down"}"#, "monitor")
        .expect("publish critical event should succeed");
    assert_eq!(critical_event.delivered_to.len(), 2);
}

#[test]
fn event_bus_get_events_by_channel() {
    let bus = EventBus::new();
    bus.subscribe("s1", "learning", None)
        .expect("subscribe should succeed");

    for i in 0..5 {
        bus.publish("learning", "pathway_update", &format!(r#"{{"seq":{i}}}"#), "hebbian")
            .expect("publish should succeed");
    }

    let events = bus.get_events("learning", 3);
    assert_eq!(events.len(), 3, "should return at most 3 events");

    let all_events = bus.get_events("learning", 100);
    assert_eq!(all_events.len(), 5, "should return all 5 events when limit is large");

    let empty = bus.get_events("consensus", 10);
    assert!(empty.is_empty(), "no events should exist on consensus channel");
}

#[test]
fn event_bus_total_events_across_channels() {
    let bus = EventBus::new();
    assert_eq!(bus.total_events(), 0);

    bus.publish("health", "ping", "{}", "test")
        .expect("publish should succeed");
    bus.publish("metrics", "latency", "{}", "test")
        .expect("publish should succeed");
    bus.publish("health", "pong", "{}", "test")
        .expect("publish should succeed");

    assert_eq!(bus.total_events(), 3, "total events should count across all channels");
}

#[test]
fn event_bus_list_channels_includes_custom() {
    let bus = EventBus::new();
    bus.create_channel("alerts")
        .expect("create custom channel should succeed");
    bus.create_channel("audit")
        .expect("create custom channel should succeed");

    let channels = bus.list_channels();
    assert!(channels.contains(&"alerts".to_string()));
    assert!(channels.contains(&"audit".to_string()));
    assert_eq!(bus.channel_count(), 8, "6 default + 2 custom = 8");
}

#[test]
fn event_bus_channel_info_reflects_publishes() {
    let bus = EventBus::new();
    bus.subscribe("s1", "consensus", None)
        .expect("subscribe should succeed");

    bus.publish("consensus", "vote", "{}", "pbft")
        .expect("publish should succeed");
    bus.publish("consensus", "commit", "{}", "pbft")
        .expect("publish should succeed");

    let info = bus
        .get_channel_info("consensus")
        .expect("channel info should exist");
    assert_eq!(info.event_count, 2);
    assert_eq!(info.subscriber_count, 1);
}

// =========================================================================
// 2. BridgeManager Tests
// =========================================================================

#[test]
fn bridge_manager_default_wire_weights_loaded() {
    let mgr = BridgeManager::new();
    let weights = mgr.wire_weights();
    assert!(
        !weights.is_empty(),
        "BridgeManager should load default wire weights"
    );
    assert!(
        weights.len() >= 10,
        "at least 10 wire weight entries expected"
    );
}

#[test]
fn bridge_manager_register_and_get_active_bridges() {
    let mgr = BridgeManager::new();
    let id1 = mgr
        .register_bridge("maintenance-engine", "synthex", WireProtocol::Rest)
        .expect("registering a bridge should succeed");
    let id2 = mgr
        .register_bridge("maintenance-engine", "nais", WireProtocol::Rest)
        .expect("registering a second bridge should succeed");

    let active = mgr.get_active_bridges();
    assert_eq!(active.len(), 2, "both bridges should be active initially");

    // Degrade one bridge to failed
    mgr.update_health(&id1, 0.1)
        .expect("updating health should succeed");
    let active_after = mgr.get_active_bridges();
    assert_eq!(active_after.len(), 1, "only one bridge should remain active");
    let failed = mgr.get_failed_bridges();
    assert_eq!(failed.len(), 1, "one bridge should be in failed state");

    // Verify the surviving active bridge
    let surviving = mgr
        .get_bridge(&id2)
        .expect("bridge should still exist");
    assert_eq!(surviving.status, BridgeStatus::Active);
}

#[test]
fn bridge_manager_overall_synergy_in_range() {
    let mgr = BridgeManager::new();
    let overall = mgr.overall_synergy();
    assert!(
        overall > 0.0 && overall <= 1.0,
        "overall synergy should be in (0.0, 1.0], got {overall}"
    );
}

#[test]
fn bridge_manager_bridge_count_tracks_registrations() {
    let mgr = BridgeManager::new();
    assert_eq!(mgr.bridge_count(), 0, "no bridges registered initially");

    let id = mgr
        .register_bridge("a", "b", WireProtocol::Grpc)
        .expect("register should succeed");
    assert_eq!(mgr.bridge_count(), 1);

    mgr.deregister_bridge(&id)
        .expect("deregister should succeed");
    assert_eq!(mgr.bridge_count(), 0);
}

#[test]
fn bridge_manager_record_request_updates_metrics() {
    let mgr = BridgeManager::new();
    let id = mgr
        .register_bridge("synthex", "san-k7", WireProtocol::Rest)
        .expect("register should succeed");

    mgr.record_request(&id, 10.0, true)
        .expect("record request should succeed");
    mgr.record_request(&id, 30.0, true)
        .expect("record request should succeed");
    mgr.record_request(&id, 20.0, false)
        .expect("record request should succeed");

    let bridge = mgr.get_bridge(&id).expect("bridge should exist");
    assert_eq!(bridge.request_count, 3);
    assert_eq!(bridge.error_count, 1);
    // Average latency: (10 + 30 + 20) / 3 = 20
    assert!(
        (bridge.latency_ms - 20.0).abs() < f64::EPSILON,
        "average latency should be 20.0, got {}",
        bridge.latency_ms
    );
    assert!(bridge.last_active.is_some());
}

// =========================================================================
// 3. RestClient Tests
// =========================================================================

#[test]
fn rest_client_default_endpoints_loaded() {
    let client = RestClient::new();
    let count = client.endpoint_count();
    assert!(
        count >= 8,
        "RestClient should load at least 8 default REST endpoints, got {count}"
    );
    assert!(
        client.get_endpoint("synthex").is_ok(),
        "synthex endpoint should be present"
    );
}

#[test]
fn rest_client_endpoint_count_increments_on_register() {
    let client = RestClient::new();
    let initial = client.endpoint_count();

    let ep = ServiceEndpoint::new("test-svc", "10.0.0.1", 7777);
    client
        .register_endpoint(ep)
        .expect("registering endpoint should succeed");

    assert_eq!(
        client.endpoint_count(),
        initial + 1,
        "endpoint count should increase by 1"
    );
}

#[test]
fn rest_client_simulate_request_records_to_log() {
    let client = RestClient::new();

    let resp = client
        .simulate_request("synthex", HttpMethod::Get, "/status")
        .expect("simulate request should succeed for registered service");
    assert_eq!(resp.status_code, 200);
    assert!(resp.body.contains("synthex"));

    let log = client.get_request_log("synthex");
    assert_eq!(log.len(), 1, "one request should be logged");
    assert_eq!(log[0].service_id, "synthex");
    assert_eq!(log[0].method, HttpMethod::Get);
    assert!(log[0].success);
}

#[test]
fn rest_client_circuit_breaker_blocks_requests() {
    let client = RestClient::new();

    assert!(
        !client.is_circuit_open("synthex"),
        "circuit should be closed by default"
    );

    client.set_circuit_open("synthex", true);
    assert!(client.is_circuit_open("synthex"));

    let result = client.simulate_request("synthex", HttpMethod::Get, "/test");
    assert!(
        result.is_err(),
        "request should be blocked when circuit is open"
    );

    client.set_circuit_open("synthex", false);
    let result_ok = client.simulate_request("synthex", HttpMethod::Get, "/test");
    assert!(
        result_ok.is_ok(),
        "request should succeed after circuit is closed"
    );
}

// =========================================================================
// 4. gRPC / WebSocket / IPC Basic State Tests
// =========================================================================

#[test]
fn grpc_client_creation_and_stub_count() {
    let client = GrpcClient::new();
    assert_eq!(
        client.stub_count(),
        0,
        "GrpcClient should start with zero stubs"
    );
    assert_eq!(client.call_count(), 0);
}

#[test]
fn grpc_client_register_stub_and_retrieve() {
    let client = GrpcClient::new();
    let proto = m4_integration::grpc::ProtoServiceDef {
        service_name: "ultraplate.toolmaker.v1.Compiler".into(),
        package: "ultraplate.toolmaker.v1".into(),
        methods: vec!["Compile".into(), "Status".into()],
        proto_version: "3".into(),
    };

    client
        .register_stub("tool-maker", "localhost", 8103, proto)
        .expect("registering stub should succeed");
    assert_eq!(client.stub_count(), 1);

    let stub = client
        .get_stub("tool-maker")
        .expect("stub should be retrievable");
    assert_eq!(stub.service_id, "tool-maker");
    assert_eq!(stub.port, 8103);
}

#[test]
fn websocket_client_creation_and_state() {
    let client = WebSocketClient::new();
    assert_eq!(
        client.total_connection_count(),
        0,
        "WebSocketClient should start with no connections"
    );
    assert_eq!(client.active_connection_count(), 0);
    assert_eq!(client.message_count(), 0);
}

#[test]
fn websocket_client_connect_and_state() {
    let client = WebSocketClient::new();
    let conn_id = client
        .connect("synthex", "localhost", 8091, "/ws/patterns")
        .expect("connecting should succeed");

    assert_eq!(client.total_connection_count(), 1);
    let state = client
        .connection_state(&conn_id)
        .expect("connection state should be retrievable");
    assert_eq!(state, WsConnectionState::Connected);
}

#[test]
fn ipc_manager_creation_and_registration() {
    let mgr = IpcManager::new();
    assert_eq!(mgr.socket_count(), 0, "IpcManager should start with no sockets");

    let path = mgr
        .register("san-k7")
        .expect("registering socket should succeed");
    assert!(
        path.contains("san-k7.sock"),
        "socket path should contain service ID"
    );
    assert_eq!(mgr.socket_count(), 1);

    let socket = mgr
        .get_socket("san-k7")
        .expect("socket should be retrievable");
    assert_eq!(socket.state, SocketState::Listening);
}

#[test]
fn ipc_manager_connect_and_send() {
    let mgr = IpcManager::with_socket_dir("/tmp/test-l4");
    mgr.register("src-svc").expect("register source should succeed");
    mgr.register("tgt-svc").expect("register target should succeed");
    mgr.connect("src-svc").expect("connect source should succeed");
    mgr.connect("tgt-svc").expect("connect target should succeed");

    let msg_id = mgr
        .send("src-svc", "tgt-svc", IpcMessageType::Request, "hello-ipc")
        .expect("send should succeed on connected sockets");
    assert!(!msg_id.is_empty(), "message ID should be non-empty");
    assert_eq!(mgr.message_count(), 1);
}

// =========================================================================
// 5. ServiceEndpoint Tests
// =========================================================================

#[test]
fn service_endpoint_url_construction() {
    let ep = ServiceEndpoint::new("my-service", "10.0.0.1", 9090);
    let url = ep.url("/status");
    assert_eq!(
        url, "http://10.0.0.1:9090/api/status",
        "URL should combine protocol, host, port, base_path, and path"
    );
}

#[test]
fn service_endpoint_health_url() {
    let ep = ServiceEndpoint::new("my-service", "localhost", 8080);
    let health = ep.health_url();
    assert_eq!(
        health, "http://localhost:8080/api/health",
        "health URL should use the default health_path"
    );
}

#[test]
fn service_endpoint_websocket_protocol_url() {
    let mut ep = ServiceEndpoint::new("ws-svc", "localhost", 8091);
    ep.protocol = WireProtocol::WebSocket;
    let url = ep.url("/stream");
    assert!(
        url.starts_with("ws://"),
        "WebSocket endpoints should produce ws:// URLs, got {url}"
    );
}

#[test]
fn default_endpoints_contains_known_services() {
    let endpoints = m4_integration::default_endpoints();
    assert!(
        endpoints.len() >= 10,
        "at least 10 default endpoints expected, got {}",
        endpoints.len()
    );

    let ids: Vec<&str> = endpoints.iter().map(|e| e.service_id.as_str()).collect();
    assert!(ids.contains(&"synthex"), "synthex must be in default endpoints");
    assert!(ids.contains(&"san-k7"), "san-k7 must be in default endpoints");
    assert!(ids.contains(&"nais"), "nais must be in default endpoints");
    assert!(ids.contains(&"tool-maker"), "tool-maker must be in default endpoints");
}

// =========================================================================
// 6. WireProtocol Tests
// =========================================================================

#[test]
fn wire_protocol_default_timeouts() {
    assert_eq!(WireProtocol::Rest.default_timeout_ms(), 5000);
    assert_eq!(WireProtocol::Grpc.default_timeout_ms(), 3000);
    assert_eq!(WireProtocol::WebSocket.default_timeout_ms(), 10000);
    assert_eq!(WireProtocol::Ipc.default_timeout_ms(), 1000);
}

#[test]
fn wire_weight_matrix_covers_all_core_targets() {
    let weights = m4_integration::default_wire_weights();
    assert!(
        weights.len() >= 10,
        "wire weight matrix should have at least 10 entries, got {}",
        weights.len()
    );

    let targets: Vec<&str> = weights.iter().map(|w| w.target.as_str()).collect();
    assert!(targets.contains(&"synthex"), "synthex must be in wire weight targets");
    assert!(targets.contains(&"san-k7"), "san-k7 must be in wire weight targets");
    assert!(targets.contains(&"nais"), "nais must be in wire weight targets");
    assert!(targets.contains(&"bash-engine"), "bash-engine must be in wire weight targets");
    assert!(targets.contains(&"tool-maker"), "tool-maker must be in wire weight targets");

    // All weights should be positive
    for w in &weights {
        assert!(
            w.weight > 0.0,
            "wire weight for {} -> {} should be positive, got {}",
            w.source,
            w.target,
            w.weight
        );
    }
}

// =========================================================================
// 7. ToolRegistrar Tests
// =========================================================================

#[test]
fn tool_registrar_creation_with_port() {
    let registrar = ToolRegistrar::new(8080)
        .expect("creating registrar on port 8080 should succeed");
    assert_eq!(registrar.service_port(), 8080);
}

#[test]
fn tool_registrar_tool_count_is_15() {
    let registrar = ToolRegistrar::new(8080)
        .expect("creating registrar should succeed");
    assert_eq!(
        registrar.tool_count(),
        15,
        "registrar should manage exactly 15 tools"
    );
}

#[test]
fn tool_registrar_initially_not_registered() {
    let registrar = ToolRegistrar::new(8080)
        .expect("creating registrar should succeed");
    assert!(
        !registrar.is_registered(),
        "registrar should not be registered initially"
    );
    assert_eq!(registrar.registered_count(), 0);
}

#[test]
fn tool_registrar_build_payload() {
    let registrar = ToolRegistrar::new(9090)
        .expect("creating registrar should succeed");
    let payload = registrar.build_payload();
    assert_eq!(payload.service_id, "maintenance-engine");
    assert_eq!(payload.port, 9090);
    assert_eq!(
        payload.tool_count(),
        15,
        "payload should contain all 15 tools"
    );
    assert!(!payload.service_name.is_empty());
    assert!(!payload.version.is_empty());
    assert_eq!(payload.host, "localhost");

    // Verify each tool entry is populated
    for entry in &payload.tools {
        assert!(!entry.id.is_empty(), "tool entry ID should be non-empty");
        assert!(!entry.name.is_empty(), "tool entry name should be non-empty");
        assert!(!entry.endpoint.is_empty(), "tool entry endpoint should be non-empty");
        assert!(!entry.method.is_empty(), "tool entry method should be non-empty");
    }
}

#[test]
fn tool_registrar_registration_report() {
    let registrar = ToolRegistrar::new(8080)
        .expect("creating registrar should succeed");
    let report = registrar.registration_report();

    assert_eq!(report.total_tools, 15);
    assert_eq!(report.registered_count, 0);
    assert_eq!(report.failed_count, 0);
    assert_eq!(report.pending_count, 15);
    assert!(!report.all_registered());
    assert!(!report.has_failures());
    assert!(
        report.registration_ratio().abs() < f64::EPSILON,
        "registration ratio should be 0.0 when nothing is registered"
    );
}

#[test]
fn tool_registrar_tool_status_lookup() {
    let registrar = ToolRegistrar::new(8080)
        .expect("creating registrar should succeed");

    // Known tool should exist
    let status = registrar.tool_status("me-health-check");
    assert!(
        status.is_some(),
        "me-health-check should be a known tool ID"
    );
    if let Some(s) = status {
        assert!(!s.registered);
        assert!(s.last_attempt.is_none());
    }

    // Unknown tool should return None
    let unknown = registrar.tool_status("nonexistent-tool-xyz");
    assert!(
        unknown.is_none(),
        "unknown tool IDs should return None"
    );
}

// =========================================================================
// 8. Cross-Module Tests
// =========================================================================

#[test]
fn cross_module_event_bus_to_bridge_event_flow() {
    // Simulate: a health event on the EventBus triggers bridge health updates
    let bus = EventBus::new();
    let bridge_mgr = BridgeManager::new();

    // Register a bridge
    let bridge_id = bridge_mgr
        .register_bridge("maintenance-engine", "synthex", WireProtocol::Rest)
        .expect("bridge registration should succeed");

    // Subscribe the bridge manager (conceptually) to health events
    bus.subscribe("bridge-manager", "health", None)
        .expect("subscribing should succeed");

    // Publish a degradation event
    let event = bus
        .publish(
            "health",
            "service_degraded",
            r#"{"service":"synthex","health_score":0.4}"#,
            "health-monitor",
        )
        .expect("publishing should succeed");

    assert!(
        event.delivered_to.contains(&"bridge-manager".to_string()),
        "bridge-manager should receive the event"
    );

    // Simulate the bridge manager reacting to the event by updating bridge health
    bridge_mgr
        .update_health(&bridge_id, 0.4)
        .expect("updating bridge health should succeed");

    let bridge = bridge_mgr
        .get_bridge(&bridge_id)
        .expect("bridge should exist");
    assert_eq!(
        bridge.status,
        BridgeStatus::Degraded,
        "bridge should be degraded after health update to 0.4"
    );
}

#[test]
fn cross_module_endpoint_to_bridge_mapping() {
    // Verify that default endpoints can be mapped to bridges
    let endpoints = m4_integration::default_endpoints();
    let bridge_mgr = BridgeManager::new();

    // Register bridges for a subset of endpoints
    let mut bridge_ids = Vec::new();
    for ep in endpoints.iter().take(3) {
        let id = bridge_mgr
            .register_bridge("maintenance-engine", &ep.service_id, ep.protocol)
            .expect("bridge registration should succeed");
        bridge_ids.push((ep.service_id.clone(), id));
    }

    assert_eq!(bridge_mgr.bridge_count(), 3);

    // Verify each bridge is retrievable and has the right target
    for (service_id, bridge_id) in &bridge_ids {
        let bridge = bridge_mgr
            .get_bridge(bridge_id)
            .expect("bridge should be retrievable");
        assert_eq!(bridge.target_service, *service_id);
        assert_eq!(bridge.source_service, "maintenance-engine");
    }

    // Verify bridges_for_service returns all 3 for maintenance-engine
    let me_bridges = bridge_mgr.get_bridges_for_service("maintenance-engine");
    assert_eq!(
        me_bridges.len(),
        3,
        "maintenance-engine should have 3 bridges"
    );
}

#[test]
fn cross_module_rest_client_with_bridge_synergy() {
    let rest_client = RestClient::new();
    let bridge_mgr = BridgeManager::new();

    // Register a bridge for synthex
    let bridge_id = bridge_mgr
        .register_bridge("maintenance-engine", "synthex", WireProtocol::Rest)
        .expect("bridge registration should succeed");

    // Make a simulated REST request
    let response = rest_client
        .simulate_request("synthex", HttpMethod::Get, "/health")
        .expect("simulate request should succeed");
    assert_eq!(response.status_code, 200);

    // Record the latency on the bridge
    #[allow(clippy::cast_precision_loss)]
    let latency = response.duration_ms as f64;
    bridge_mgr
        .record_request(&bridge_id, latency, true)
        .expect("recording request on bridge should succeed");

    let bridge = bridge_mgr
        .get_bridge(&bridge_id)
        .expect("bridge should exist");
    assert_eq!(bridge.request_count, 1);
    assert!(bridge.last_active.is_some());

    // Check wire weight exists for this pair
    let weight = bridge_mgr.get_wire_weight("maintenance-engine", "synthex");
    assert!(
        weight.is_some(),
        "wire weight should exist for maintenance-engine -> synthex"
    );
}
