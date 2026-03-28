//! # L2 Services Integration Tests
//!
//! Comprehensive integration tests for Layer 2 (Services) modules:
//!
//! | Module | Coverage |
//! |--------|----------|
//! | M10 Health Monitor | probe management, thresholds, aggregation |
//! | M11 Lifecycle Manager | FSM transitions, restarts, backoff |
//! | M09 Service Registry | registry, tier/protocol filtering |
//! | M12 Resilience (Circuit + Balancer) | state machine, load distribution |
//! | Cross-module | circuit+health, lifecycle+circuit, discovery+balancer |
//!
//! ## Quality Gates
//!
//! - Zero `.unwrap()` / `.expect()` / `unsafe`
//! - Deterministic (no real I/O or timers)
//! - Compiles under `clippy::pedantic` + `clippy::nursery`

mod common;

use std::time::Duration;

use maintenance_engine::m1_foundation::shared_types::Timestamp;
use maintenance_engine::m2_services::resilience::{
    CircuitBreakerConfig, CircuitBreakerRegistry, Endpoint, LoadBalanceAlgorithm, LoadBalancer,
};
use maintenance_engine::m2_services::service_registry::{
    register_ultraplate_services, ServiceDefinition, ServiceRegistry,
};
use maintenance_engine::m2_services::{
    CircuitBreakerOps, CircuitState, HealthCheckResult, HealthMonitor, HealthMonitoring,
    HealthProbeBuilder, HealthStatus, LifecycleManager, LifecycleOps, LoadBalancing,
    RestartConfig, ServiceDiscovery, ServiceState, ServiceStatus, ServiceTier,
};

// =========================================================================
// Helpers
// =========================================================================

/// Build a valid health probe for a given service id.
fn build_probe(
    service_id: &str,
) -> maintenance_engine::m2_services::HealthProbe {
    let result = HealthProbeBuilder::new(
        service_id,
        format!("http://localhost/{service_id}/health"),
    )
    .build();
    assert!(result.is_ok(), "probe build should succeed for '{service_id}'");
    result.unwrap_or_else(|_| unreachable!())
}

/// Build a health probe with custom thresholds.
fn build_probe_thresholds(
    service_id: &str,
    healthy: u32,
    unhealthy: u32,
) -> maintenance_engine::m2_services::HealthProbe {
    let result = HealthProbeBuilder::new(
        service_id,
        format!("http://localhost/{service_id}/health"),
    )
    .healthy_threshold(healthy)
    .unhealthy_threshold(unhealthy)
    .build();
    assert!(result.is_ok());
    result.unwrap_or_else(|_| unreachable!())
}

/// Fabricate a healthy check result.
fn healthy_result(service_id: &str) -> HealthCheckResult {
    HealthCheckResult {
        service_id: service_id.to_owned(),
        status: HealthStatus::Healthy,
        response_time_ms: 42,
        timestamp: Timestamp::now(),
        message: Some("OK".into()),
        status_code: Some(200),
    }
}

/// Fabricate an unhealthy check result.
fn unhealthy_result(service_id: &str) -> HealthCheckResult {
    HealthCheckResult {
        service_id: service_id.to_owned(),
        status: HealthStatus::Unhealthy,
        response_time_ms: 5000,
        timestamp: Timestamp::now(),
        message: Some("Connection refused".into()),
        status_code: None,
    }
}

/// Bring a lifecycle-managed service from Stopped to Running.
fn drive_to_running(mgr: &LifecycleManager, id: &str) {
    let r1 = mgr.start_service(id);
    assert!(r1.is_ok(), "Stopped -> Starting should succeed");
    let r2 = mgr.mark_running(id);
    assert!(r2.is_ok(), "Starting -> Running should succeed");
}

// =========================================================================
// 1. HealthMonitor -- probe management
// =========================================================================

#[test]
fn health_register_and_unregister_probes() {
    let monitor = HealthMonitor::new();
    let probe_a = build_probe("alpha");
    let probe_b = build_probe("beta");

    assert!(monitor.register_probe(probe_a).is_ok());
    assert!(monitor.register_probe(probe_b).is_ok());
    assert_eq!(monitor.probe_count(), 2);

    assert!(monitor.unregister_probe("alpha").is_ok());
    assert_eq!(monitor.probe_count(), 1);

    // Unregistering again should fail.
    assert!(monitor.unregister_probe("alpha").is_err());
}

#[test]
fn health_duplicate_probe_registration_rejected() {
    let monitor = HealthMonitor::new();
    assert!(monitor.register_probe(build_probe("dup")).is_ok());
    assert!(monitor.register_probe(build_probe("dup")).is_err());
    assert_eq!(monitor.probe_count(), 1);
}

// =========================================================================
// 2. HealthMonitor -- consecutive failure / success thresholds
// =========================================================================

#[test]
fn health_consecutive_failures_trigger_unhealthy() {
    let monitor = HealthMonitor::new();
    assert!(
        monitor
            .register_probe(build_probe_thresholds("svc", 3, 3))
            .is_ok()
    );

    for _ in 0..3 {
        assert!(
            monitor
                .record_result("svc", unhealthy_result("svc"))
                .is_ok()
        );
    }

    let status = monitor.get_status("svc");
    assert!(status.is_ok());
    assert_eq!(status.ok(), Some(HealthStatus::Unhealthy));
}

#[test]
fn health_consecutive_successes_trigger_healthy() {
    let monitor = HealthMonitor::new();
    assert!(
        monitor
            .register_probe(build_probe_thresholds("svc", 2, 2))
            .is_ok()
    );

    for _ in 0..2 {
        assert!(
            monitor
                .record_result("svc", healthy_result("svc"))
                .is_ok()
        );
    }

    let status = monitor.get_status("svc");
    assert!(status.is_ok());
    assert_eq!(status.ok(), Some(HealthStatus::Healthy));
}

#[test]
fn health_recovery_from_unhealthy_to_healthy() {
    let monitor = HealthMonitor::new();
    assert!(
        monitor
            .register_probe(build_probe_thresholds("svc", 2, 2))
            .is_ok()
    );

    // Drive to unhealthy.
    for _ in 0..2 {
        assert!(
            monitor
                .record_result("svc", unhealthy_result("svc"))
                .is_ok()
        );
    }
    assert_eq!(
        monitor.get_status("svc").ok(),
        Some(HealthStatus::Unhealthy)
    );

    // Recover.
    for _ in 0..2 {
        assert!(
            monitor
                .record_result("svc", healthy_result("svc"))
                .is_ok()
        );
    }
    assert_eq!(monitor.get_status("svc").ok(), Some(HealthStatus::Healthy));
}

// =========================================================================
// 3. HealthMonitor -- aggregate health score
// =========================================================================

#[test]
fn health_aggregate_empty_returns_one() {
    let monitor = HealthMonitor::new();
    common::assert_f64_eq(monitor.aggregate_health(), 1.0, "empty aggregate");
}

#[test]
fn health_aggregate_mixed_statuses() {
    let monitor = HealthMonitor::new();

    // Service A -> healthy (threshold=1).
    assert!(
        monitor
            .register_probe(build_probe_thresholds("a", 1, 1))
            .is_ok()
    );
    assert!(
        monitor.record_result("a", healthy_result("a")).is_ok()
    );

    // Service B -> unhealthy (threshold=1).
    assert!(
        monitor
            .register_probe(build_probe_thresholds("b", 1, 1))
            .is_ok()
    );
    assert!(
        monitor
            .record_result("b", unhealthy_result("b"))
            .is_ok()
    );

    // Expected: (1.0 + 0.0) / 2 = 0.5
    common::assert_f64_eq(monitor.aggregate_health(), 0.5, "mixed aggregate");
}

// =========================================================================
// 4. CircuitBreakerRegistry -- state machine via registry
// =========================================================================

#[test]
fn circuit_closed_to_open_after_threshold() {
    let registry = CircuitBreakerRegistry::new();
    let config = CircuitBreakerConfig::builder()
        .failure_threshold(5)
        .open_timeout(Duration::from_secs(60))
        .build();
    assert!(registry.register_breaker("test-svc", config).is_ok());

    for i in 0..5 {
        let result = registry.record_failure("test-svc");
        assert!(result.is_ok());
        if i < 4 {
            assert_eq!(
                registry.get_state("test-svc").ok(),
                Some(CircuitState::Closed),
                "should still be closed at failure {i}"
            );
        }
    }
    assert_eq!(
        registry.get_state("test-svc").ok(),
        Some(CircuitState::Open)
    );
}

#[test]
fn circuit_half_open_success_closes() {
    let registry = CircuitBreakerRegistry::new();
    let config = CircuitBreakerConfig::builder()
        .failure_threshold(1)
        .success_threshold(3)
        .open_timeout(Duration::ZERO)
        .build();
    assert!(registry.register_breaker("test-svc", config).is_ok());

    // Trip to Open, then allow_request transitions to HalfOpen (zero timeout).
    assert!(registry.record_failure("test-svc").is_ok());
    assert!(registry.allow_request("test-svc").is_ok());
    assert_eq!(
        registry.get_state("test-svc").ok(),
        Some(CircuitState::HalfOpen)
    );

    // Three successes close the circuit.
    for _ in 0..3 {
        assert!(registry.record_success("test-svc").is_ok());
    }
    assert_eq!(
        registry.get_state("test-svc").ok(),
        Some(CircuitState::Closed)
    );
}

#[test]
fn circuit_allow_request_blocked_when_open() {
    let registry = CircuitBreakerRegistry::new();
    let config = CircuitBreakerConfig::builder()
        .failure_threshold(5)
        .open_timeout(Duration::from_secs(60))
        .build();
    assert!(registry.register_breaker("svc", config).is_ok());

    // Trip to Open.
    for _ in 0..5 {
        assert!(registry.record_failure("svc").is_ok());
    }
    assert_eq!(registry.get_state("svc").ok(), Some(CircuitState::Open));
    let result = registry.allow_request("svc");
    assert!(result.is_ok());
    assert_eq!(result.ok(), Some(false));
}

// =========================================================================
// 5. CircuitBreakerRegistry -- registry operations + state tracking
// =========================================================================

#[test]
fn circuit_registry_register_and_deregister() {
    let registry = CircuitBreakerRegistry::new();
    assert!(registry.register_default("svc-a").is_ok());
    assert!(registry.is_registered("svc-a"));
    assert_eq!(registry.breaker_count(), 1);

    assert!(registry.deregister_breaker("svc-a").is_ok());
    assert!(!registry.is_registered("svc-a"));
    assert_eq!(registry.breaker_count(), 0);
}

#[test]
fn circuit_registry_failure_threshold_opens() {
    let registry = CircuitBreakerRegistry::new();
    let config = CircuitBreakerConfig::builder()
        .failure_threshold(5)
        .open_timeout(Duration::from_secs(60))
        .build();
    assert!(registry.register_breaker("svc", config).is_ok());

    for _ in 0..5 {
        assert!(registry.record_failure("svc").is_ok());
    }

    let state = registry.get_state("svc");
    assert!(state.is_ok());
    assert_eq!(state.ok(), Some(CircuitState::Open));
}

#[test]
fn circuit_registry_open_blocks_requests() {
    let registry = CircuitBreakerRegistry::new();
    let config = CircuitBreakerConfig::builder()
        .failure_threshold(2)
        .open_timeout(Duration::from_secs(60))
        .build();
    assert!(registry.register_breaker("svc", config).is_ok());

    assert!(registry.record_failure("svc").is_ok());
    assert!(registry.record_failure("svc").is_ok());

    let allowed = registry.allow_request("svc");
    assert!(allowed.is_ok());
    assert_eq!(allowed.ok(), Some(false));
}

#[test]
fn circuit_registry_timeout_transitions_to_half_open() {
    let registry = CircuitBreakerRegistry::new();
    let config = CircuitBreakerConfig::builder()
        .failure_threshold(1)
        .open_timeout(Duration::ZERO) // immediate timeout for deterministic testing
        .build();
    assert!(registry.register_breaker("svc", config).is_ok());

    assert!(registry.record_failure("svc").is_ok());
    assert_eq!(registry.get_state("svc").ok(), Some(CircuitState::Open));

    // With 0ms timeout, allow_request transitions to HalfOpen immediately.
    let allowed = registry.allow_request("svc");
    assert_eq!(allowed.ok(), Some(true));
    assert_eq!(
        registry.get_state("svc").ok(),
        Some(CircuitState::HalfOpen)
    );
}

#[test]
fn circuit_registry_half_open_success_closes() {
    let registry = CircuitBreakerRegistry::new();
    let config = CircuitBreakerConfig::builder()
        .failure_threshold(1)
        .success_threshold(3)
        .open_timeout(Duration::ZERO)
        .build();
    assert!(registry.register_breaker("svc", config).is_ok());

    // Trip and transition to HalfOpen.
    assert!(registry.record_failure("svc").is_ok());
    assert!(registry.allow_request("svc").is_ok());
    assert_eq!(
        registry.get_state("svc").ok(),
        Some(CircuitState::HalfOpen)
    );

    // Three successes close the circuit.
    for _ in 0..3 {
        assert!(registry.record_success("svc").is_ok());
    }
    assert_eq!(
        registry.get_state("svc").ok(),
        Some(CircuitState::Closed)
    );
}

#[test]
fn circuit_registry_half_open_failure_reopens() {
    let registry = CircuitBreakerRegistry::new();
    let config = CircuitBreakerConfig::builder()
        .failure_threshold(1)
        .success_threshold(5)
        .open_timeout(Duration::ZERO)
        .build();
    assert!(registry.register_breaker("svc", config).is_ok());

    // Trip -> HalfOpen.
    assert!(registry.record_failure("svc").is_ok());
    assert!(registry.allow_request("svc").is_ok());
    assert_eq!(
        registry.get_state("svc").ok(),
        Some(CircuitState::HalfOpen)
    );

    // Any failure in HalfOpen should reopen.
    assert!(registry.record_failure("svc").is_ok());
    assert_eq!(
        registry.get_state("svc").ok(),
        Some(CircuitState::Open)
    );
}

// =========================================================================
// 6. LifecycleManager -- registration and transitions
// =========================================================================

#[test]
fn lifecycle_register_and_start() {
    let mgr = LifecycleManager::new();
    assert!(
        mgr.register("synthex", "SYNTHEX Engine", ServiceTier::Tier1, RestartConfig::default())
            .is_ok()
    );
    assert_eq!(mgr.service_count(), 1);

    let state = mgr.get_status("synthex");
    assert!(state.is_ok());
    assert_eq!(state.ok(), Some(ServiceStatus::Stopped));

    let t = mgr.start_service("synthex");
    assert!(t.is_ok());
    assert_eq!(
        mgr.get_status("synthex").ok(),
        Some(ServiceStatus::Starting)
    );
}

#[test]
fn lifecycle_full_start_stop_cycle() {
    let mgr = LifecycleManager::new();
    assert!(
        mgr.register("svc", "Test", ServiceTier::Tier3, RestartConfig::default())
            .is_ok()
    );

    // Stopped -> Starting -> Running
    drive_to_running(&mgr, "svc");
    assert_eq!(
        mgr.get_status("svc").ok(),
        Some(ServiceStatus::Running)
    );

    // Running -> Stopping
    let stop = mgr.stop_service("svc");
    assert!(stop.is_ok());

    // Stopping -> Stopped
    let stopped = mgr.mark_stopped("svc");
    assert!(stopped.is_ok());
    assert_eq!(
        mgr.get_status("svc").ok(),
        Some(ServiceStatus::Stopped)
    );
}

#[test]
fn lifecycle_restart_returns_backoff() {
    let mgr = LifecycleManager::new();
    assert!(
        mgr.register("svc", "Test", ServiceTier::Tier4, RestartConfig::default())
            .is_ok()
    );
    drive_to_running(&mgr, "svc");

    let result = mgr.restart_service("svc");
    assert!(result.is_ok());
    // Default initial backoff is 1s.
    if let Ok(backoff) = result {
        assert!(backoff >= Duration::from_secs(1));
    }
}

#[test]
fn lifecycle_max_restarts_enforced() {
    let mgr = LifecycleManager::new();
    assert!(
        mgr.register("svc", "Test", ServiceTier::Tier5, RestartConfig::default())
            .is_ok()
    );

    // Default max_restarts is 5. Exhaust them.
    drive_to_running(&mgr, "svc");
    for i in 0..5 {
        let result = mgr.restart_service("svc");
        assert!(result.is_ok(), "restart {i} should succeed");
        // After restart, service is in Starting. Move to Running for next.
        let running = mgr.mark_running("svc");
        assert!(running.is_ok());
    }

    // The 6th restart should be rejected.
    let result = mgr.restart_service("svc");
    assert!(result.is_err());
    assert!(!mgr.can_restart("svc").ok().is_some_and(|v| v));
}

#[test]
fn lifecycle_backoff_grows_with_restarts() {
    let mgr = LifecycleManager::new();
    assert!(
        mgr.register("svc", "Test", ServiceTier::Tier5, RestartConfig::default())
            .is_ok()
    );

    let b0 = mgr.get_restart_backoff("svc");
    assert!(b0.is_ok());
    // Default initial backoff = 1s.
    assert_eq!(b0.ok(), Some(Duration::from_secs(1)));

    drive_to_running(&mgr, "svc");
    assert!(mgr.restart_service("svc").is_ok());

    let b1 = mgr.get_restart_backoff("svc");
    assert_eq!(b1.ok(), Some(Duration::from_secs(2)));

    // Complete another cycle.
    assert!(mgr.mark_running("svc").is_ok());
    assert!(mgr.restart_service("svc").is_ok());

    let b2 = mgr.get_restart_backoff("svc");
    assert_eq!(b2.ok(), Some(Duration::from_secs(4)));
}

// =========================================================================
// 7. ServiceState -- new(), weighted_health(), update_tensor(), is_operational()
// =========================================================================

#[test]
fn service_state_new_defaults() {
    let svc = ServiceState::new("synthex", "SYNTHEX Engine", ServiceTier::Tier1, 8090);
    assert_eq!(svc.id, "synthex");
    assert_eq!(svc.name, "SYNTHEX Engine");
    assert_eq!(svc.tier, ServiceTier::Tier1);
    assert_eq!(svc.port, 8090);
    assert_eq!(svc.status, ServiceStatus::Stopped);
    assert_eq!(svc.health_status, HealthStatus::Healthy);
    common::assert_f64_eq(svc.health_score, 0.0, "initial health_score");
    common::assert_f64_eq(svc.synergy_score, 0.0, "initial synergy_score");
}

#[test]
fn service_state_weighted_health_by_tier() {
    // Tier1 weight = 1.5
    let mut t1 = ServiceState::new("a", "A", ServiceTier::Tier1, 8090);
    t1.health_score = 0.8;
    common::assert_f64_eq(t1.weighted_health(), 0.8 * 1.5, "Tier1 weighted");

    // Tier2 weight = 1.3
    let mut t2 = ServiceState::new("b", "B", ServiceTier::Tier2, 8101);
    t2.health_score = 0.8;
    common::assert_f64_eq(t2.weighted_health(), 0.8 * 1.3, "Tier2 weighted");

    // Tier3 weight = 1.2
    let mut t3 = ServiceState::new("c", "C", ServiceTier::Tier3, 8105);
    t3.health_score = 0.8;
    common::assert_f64_eq(t3.weighted_health(), 0.8 * 1.2, "Tier3 weighted");

    // Tier4 weight = 1.1
    let mut t4 = ServiceState::new("d", "D", ServiceTier::Tier4, 10001);
    t4.health_score = 0.8;
    common::assert_f64_eq(t4.weighted_health(), 0.8 * 1.1, "Tier4 weighted");

    // Tier5 weight = 1.0
    let mut t5 = ServiceState::new("e", "E", ServiceTier::Tier5, 8102);
    t5.health_score = 0.8;
    common::assert_f64_eq(t5.weighted_health(), 0.8 * 1.0, "Tier5 weighted");
}

#[test]
fn service_state_update_tensor_validates() {
    let mut svc = ServiceState::new("synthex", "SYNTHEX", ServiceTier::Tier1, 8090);
    svc.health_score = 0.95;
    svc.synergy_score = 0.88;
    svc.cpu_percent = 25.0;
    svc.uptime_seconds = 3600;
    svc.status = ServiceStatus::Running;

    svc.update_tensor();
    assert!(svc.tensor.validate().is_ok());
}

#[test]
fn service_state_is_operational() {
    let mut svc = ServiceState::new("test", "Test", ServiceTier::Tier5, 9000);
    // Stopped + Healthy -> not operational
    assert!(!svc.is_operational());

    svc.status = ServiceStatus::Running;
    svc.health_status = HealthStatus::Healthy;
    assert!(svc.is_operational());

    svc.health_status = HealthStatus::Unhealthy;
    assert!(!svc.is_operational());

    svc.health_status = HealthStatus::Degraded;
    assert!(svc.is_operational());
}

// =========================================================================
// 8. ServiceDiscovery -- register, lookup, filtering
// =========================================================================

#[test]
fn discovery_register_and_discover() {
    let registry = ServiceRegistry::new();
    let def = ServiceDefinition::builder("synthex", "SYNTHEX", "1.0.0")
        .tier(ServiceTier::Tier1)
        .port(8090)
        .build();
    assert!(registry.register(def).is_ok());
    assert_eq!(registry.service_count(), 1);

    let found = registry.discover("synthex");
    assert!(found.is_ok());
    if let Ok(svc) = found {
        assert_eq!(svc.service_id, "synthex");
        assert_eq!(svc.port, 8090);
    }
}

#[test]
fn discovery_list_services_returns_all() {
    let registry = ServiceRegistry::new();
    assert!(register_ultraplate_services(&registry).is_ok());
    assert_eq!(registry.list_services().len(), 12);
}

#[test]
fn discovery_filter_by_tier() {
    let registry = ServiceRegistry::new();
    assert!(register_ultraplate_services(&registry).is_ok());

    let tier1 = registry.discover_by_tier(ServiceTier::Tier1);
    assert_eq!(tier1.len(), 2); // SYNTHEX + SAN-K7

    let tier5 = registry.discover_by_tier(ServiceTier::Tier5);
    assert_eq!(tier5.len(), 2); // Bash Engine + Tool Maker
}

#[test]
fn discovery_filter_by_protocol() {
    let registry = ServiceRegistry::new();
    assert!(register_ultraplate_services(&registry).is_ok());

    let rest = registry.discover_by_protocol("REST");
    assert_eq!(rest.len(), 12);

    // Case-insensitive.
    let rest_lower = registry.discover_by_protocol("rest");
    assert_eq!(rest_lower.len(), 12);

    // No gRPC services in the default set.
    let grpc = registry.discover_by_protocol("gRPC");
    assert!(grpc.is_empty());
}

#[test]
fn discovery_deregister_cleans_deps() {
    let registry = ServiceRegistry::new();
    let svc_a = ServiceDefinition::builder("a", "A", "1.0.0")
        .tier(ServiceTier::Tier1)
        .port(8001)
        .build();
    let svc_b = ServiceDefinition::builder("b", "B", "1.0.0")
        .tier(ServiceTier::Tier2)
        .port(8002)
        .build();
    assert!(registry.register(svc_a).is_ok());
    assert!(registry.register(svc_b).is_ok());
    assert!(registry.add_dependency("a", "b").is_ok());

    // Deregistering B should clean the dependency from A.
    assert!(registry.deregister("b").is_ok());
    let deps = registry.get_dependencies("a");
    assert!(deps.is_ok());
    if let Ok(d) = deps {
        assert!(d.is_empty());
    }
}

// =========================================================================
// 9. LoadBalancer -- strategy selection & distribution
// =========================================================================

#[test]
fn balancer_round_robin_cycles() {
    let lb = LoadBalancer::new();
    assert!(
        lb.create_pool("api", LoadBalanceAlgorithm::RoundRobin)
            .is_ok()
    );
    assert!(
        lb.add_endpoint("api", Endpoint::new("ep1", "10.0.0.1", 8080, 1.0))
            .is_ok()
    );
    assert!(
        lb.add_endpoint("api", Endpoint::new("ep2", "10.0.0.2", 8080, 1.0))
            .is_ok()
    );

    let mut ids: Vec<String> = Vec::new();
    for _ in 0..4 {
        if let Ok(ep) = lb.select_endpoint("api") {
            ids.push(ep.id.clone());
        }
    }
    assert_eq!(ids.len(), 4);
    // Round-robin: pattern repeats every 2.
    assert_eq!(ids[0], ids[2]);
    assert_eq!(ids[1], ids[3]);
    assert_ne!(ids[0], ids[1]);
}

#[test]
fn balancer_empty_pool_returns_error() {
    let lb = LoadBalancer::new();
    assert!(
        lb.create_pool("empty", LoadBalanceAlgorithm::RoundRobin)
            .is_ok()
    );
    assert!(lb.select_endpoint("empty").is_err());
}

#[test]
fn balancer_all_unhealthy_returns_error() {
    let lb = LoadBalancer::new();
    assert!(
        lb.create_pool("pool", LoadBalanceAlgorithm::RoundRobin)
            .is_ok()
    );
    assert!(
        lb.add_endpoint("pool", Endpoint::new("ep1", "10.0.0.1", 8080, 1.0))
            .is_ok()
    );
    assert!(lb.mark_unhealthy("pool", "ep1").is_ok());
    assert!(lb.select_endpoint("pool").is_err());
}

#[test]
fn balancer_skips_unhealthy_endpoints() {
    let lb = LoadBalancer::new();
    assert!(
        lb.create_pool("svc", LoadBalanceAlgorithm::RoundRobin)
            .is_ok()
    );
    assert!(
        lb.add_endpoint("svc", Endpoint::new("healthy", "10.0.0.1", 8080, 1.0))
            .is_ok()
    );
    assert!(
        lb.add_endpoint("svc", Endpoint::new("sick", "10.0.0.2", 8080, 1.0))
            .is_ok()
    );
    assert!(lb.mark_unhealthy("svc", "sick").is_ok());

    // All selections should go to "healthy".
    for _ in 0..5 {
        if let Ok(ep) = lb.select_endpoint("svc") {
            assert_eq!(ep.id, "healthy");
        }
    }
}

#[test]
fn balancer_load_distribution_even_for_round_robin() {
    let lb = LoadBalancer::new();
    assert!(
        lb.create_pool("svc", LoadBalanceAlgorithm::RoundRobin)
            .is_ok()
    );
    assert!(
        lb.add_endpoint("svc", Endpoint::new("ep1", "10.0.0.1", 8080, 1.0))
            .is_ok()
    );
    assert!(
        lb.add_endpoint("svc", Endpoint::new("ep2", "10.0.0.2", 8080, 1.0))
            .is_ok()
    );

    for _ in 0..100 {
        let _ = lb.select_endpoint("svc");
    }

    let dist = lb.get_load_distribution("svc");
    assert!(dist.is_ok());
    if let Ok(d) = dist {
        assert_eq!(d.len(), 2);
        for (_, pct) in &d {
            assert!(
                (*pct - 0.5).abs() < f64::EPSILON,
                "expected 50%, got {pct}"
            );
        }
    }
}

// =========================================================================
// 10. Cross-module: circuit opens after health failures
// =========================================================================

#[test]
fn cross_circuit_opens_on_health_failures() {
    // Health monitor detects failures, then we propagate to circuit breaker.
    let monitor = HealthMonitor::new();
    assert!(
        monitor
            .register_probe(build_probe_thresholds("svc", 3, 3))
            .is_ok()
    );
    let registry = CircuitBreakerRegistry::new();
    let config = CircuitBreakerConfig::builder()
        .failure_threshold(3)
        .open_timeout(Duration::from_secs(60))
        .build();
    assert!(registry.register_breaker("svc", config).is_ok());

    // Simulate health failures feeding into the circuit breaker.
    for _ in 0..3 {
        assert!(
            monitor
                .record_result("svc", unhealthy_result("svc"))
                .is_ok()
        );
        assert!(registry.record_failure("svc").is_ok());
    }

    // Health monitor should show unhealthy.
    assert_eq!(
        monitor.get_status("svc").ok(),
        Some(HealthStatus::Unhealthy)
    );
    // Circuit breaker should be open.
    assert_eq!(
        registry.get_state("svc").ok(),
        Some(CircuitState::Open)
    );
    // Requests should be blocked.
    assert_eq!(registry.allow_request("svc").ok(), Some(false));
}

// =========================================================================
// 11. Cross-module: lifecycle restart resets circuit
// =========================================================================

#[test]
fn cross_lifecycle_restart_resets_circuit() {
    let mgr = LifecycleManager::new();
    assert!(
        mgr.register("svc", "Test", ServiceTier::Tier3, RestartConfig::default())
            .is_ok()
    );
    drive_to_running(&mgr, "svc");

    let registry = CircuitBreakerRegistry::new();
    let config = CircuitBreakerConfig::builder()
        .failure_threshold(1)
        .open_timeout(Duration::from_secs(60))
        .build();
    assert!(registry.register_breaker("svc", config).is_ok());

    // Trip the circuit.
    assert!(registry.record_failure("svc").is_ok());
    assert_eq!(
        registry.get_state("svc").ok(),
        Some(CircuitState::Open)
    );

    // Lifecycle restart.
    let result = mgr.restart_service("svc");
    assert!(result.is_ok());

    // After restart, reset the circuit breaker.
    assert!(registry.reset("svc").is_ok());
    assert_eq!(
        registry.get_state("svc").ok(),
        Some(CircuitState::Closed)
    );
    assert_eq!(registry.allow_request("svc").ok(), Some(true));
}

// =========================================================================
// 12. Cross-module: discovery + balancer integration
// =========================================================================

#[test]
fn cross_discovery_feeds_balancer() {
    // Register services in discovery, then populate balancer from them.
    let discovery = ServiceRegistry::new();
    let svc_a = ServiceDefinition::builder("api-1", "API Instance 1", "1.0.0")
        .tier(ServiceTier::Tier2)
        .host("10.0.0.1")
        .port(8080)
        .build();
    let svc_b = ServiceDefinition::builder("api-2", "API Instance 2", "1.0.0")
        .tier(ServiceTier::Tier2)
        .host("10.0.0.2")
        .port(8080)
        .build();
    assert!(discovery.register(svc_a).is_ok());
    assert!(discovery.register(svc_b).is_ok());

    // Feed discovered services into a load balancer pool.
    let lb = LoadBalancer::new();
    assert!(
        lb.create_pool("api", LoadBalanceAlgorithm::RoundRobin)
            .is_ok()
    );

    let services = discovery.discover_by_tier(ServiceTier::Tier2);
    for svc in &services {
        let ep = Endpoint::new(
            &svc.service_id,
            &svc.host,
            svc.port,
            svc.tier.weight(),
        );
        assert!(lb.add_endpoint("api", ep).is_ok());
    }

    let stats = lb.get_pool_stats("api");
    assert!(stats.is_ok());
    if let Ok(s) = stats {
        assert_eq!(s.total_endpoints, 2);
        assert_eq!(s.healthy_endpoints, 2);
    }

    // Select should work.
    let selected = lb.select_endpoint("api");
    assert!(selected.is_ok());
}

// =========================================================================
// 13. Lifecycle -- invalid transition rejected
// =========================================================================

#[test]
fn lifecycle_invalid_transition_rejected() {
    let mgr = LifecycleManager::new();
    assert!(
        mgr.register("svc", "Test", ServiceTier::Tier5, RestartConfig::default())
            .is_ok()
    );

    // Stopped -> Running directly is invalid (must go through Starting first).
    let result = mgr.mark_running("svc");
    assert!(result.is_err());

    // State should remain Stopped.
    assert_eq!(
        mgr.get_status("svc").ok(),
        Some(ServiceStatus::Stopped)
    );
}

// =========================================================================
// 14. Lifecycle -- restart from Failed state
// =========================================================================

#[test]
fn lifecycle_start_from_failed_state() {
    let mgr = LifecycleManager::new();
    assert!(
        mgr.register("svc", "Test", ServiceTier::Tier2, RestartConfig::default())
            .is_ok()
    );

    // Drive to Failed.
    assert!(mgr.start_service("svc").is_ok());
    assert!(mgr.mark_failed("svc").is_ok());
    assert_eq!(
        mgr.get_status("svc").ok(),
        Some(ServiceStatus::Failed)
    );

    // Failed -> Starting is valid via start_service.
    let t = mgr.start_service("svc");
    assert!(t.is_ok());
    assert_eq!(
        mgr.get_status("svc").ok(),
        Some(ServiceStatus::Starting)
    );
}

// =========================================================================
// 15. Circuit breaker stats tracking
// =========================================================================

#[test]
fn circuit_registry_stats_accurate() {
    let registry = CircuitBreakerRegistry::new();
    assert!(registry.register_default("svc").is_ok());

    // 3 successes + 1 failure = 4 total requests, 1 failure.
    for _ in 0..3 {
        assert!(registry.record_success("svc").is_ok());
    }
    assert!(registry.record_failure("svc").is_ok());

    let stats = registry.get_breaker_stats("svc");
    assert!(stats.is_ok());
    if let Ok(s) = stats {
        assert_eq!(s.total_requests, 4);
        assert_eq!(s.total_failures, 1);
        assert!((s.failure_rate - 0.25).abs() < f64::EPSILON);
        assert_eq!(s.state, CircuitState::Closed);
    }
}
