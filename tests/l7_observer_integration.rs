//! Integration tests for L7 Observer layer modules.

mod common;

use maintenance_engine::m7_observer::{
    FitnessEvaluator, FitnessConfig, FitnessTrend, SystemState,
    ObserverBus, ObserverBusConfig, ObserverMessageType, ObserverSource,
    ObserverLayer, ObserverConfig, RalphPhase,
};
use maintenance_engine::Tensor12D;

// =========================================================================
// Group 1: ObserverLayer Construction (6 tests)
// =========================================================================

#[test]
fn test_observer_layer_with_defaults() {
    let layer = ObserverLayer::with_defaults();
    assert!(layer.is_ok());
}

#[test]
fn test_observer_layer_is_enabled() {
    if let Ok(layer) = ObserverLayer::with_defaults() {
        assert!(layer.is_enabled());
    }
}

#[test]
fn test_observer_layer_disabled_config_fails() {
    let config = ObserverConfig { enabled: false, ..ObserverConfig::default() };
    let result = ObserverLayer::new(config);
    assert!(result.is_err());
}

#[test]
fn test_observer_layer_initial_tick_count() {
    if let Ok(layer) = ObserverLayer::with_defaults() {
        assert_eq!(layer.tick_count(), 0);
    }
}

#[test]
fn test_observer_layer_initial_generation() {
    if let Ok(layer) = ObserverLayer::with_defaults() {
        assert_eq!(layer.generation(), 0);
    }
}

#[test]
fn test_observer_layer_config_accessor() {
    if let Ok(layer) = ObserverLayer::with_defaults() {
        assert!(layer.config().enabled);
        assert_eq!(layer.config().tick_interval_ms, 60_000);
    }
}

// =========================================================================
// Group 2: Tick Cycle (8 tests)
// =========================================================================

#[test]
fn test_tick_produces_report() {
    if let Ok(layer) = ObserverLayer::with_defaults() {
        let tensor = Tensor12D::new([0.5; 12]);
        let result = layer.tick(&tensor);
        assert!(result.is_ok());
    }
}

#[test]
fn test_tick_increments_counter() {
    if let Ok(layer) = ObserverLayer::with_defaults() {
        let tensor = Tensor12D::new([0.5; 12]);
        let _ = layer.tick(&tensor);
        assert_eq!(layer.tick_count(), 1);
        let _ = layer.tick(&tensor);
        assert_eq!(layer.tick_count(), 2);
    }
}

#[test]
fn test_tick_stores_last_report() {
    if let Ok(layer) = ObserverLayer::with_defaults() {
        assert!(layer.get_report().is_none());
        let tensor = Tensor12D::new([0.5; 12]);
        let _ = layer.tick(&tensor);
        assert!(layer.get_report().is_some());
    }
}

#[test]
fn test_tick_adds_to_history() {
    if let Ok(layer) = ObserverLayer::with_defaults() {
        let tensor = Tensor12D::new([0.5; 12]);
        for _ in 0..5 {
            let _ = layer.tick(&tensor);
        }
        assert_eq!(layer.report_history().len(), 5);
    }
}

#[test]
fn test_tick_report_has_correct_tick_number() {
    if let Ok(layer) = ObserverLayer::with_defaults() {
        let tensor = Tensor12D::new([0.5; 12]);
        for i in 1..=3 {
            if let Ok(report) = layer.tick(&tensor) {
                assert_eq!(report.tick, i);
            }
        }
    }
}

#[test]
fn test_tick_with_healthy_tensor() {
    if let Ok(layer) = ObserverLayer::with_defaults() {
        let tensor = common::make_healthy_tensor();
        let result = layer.tick(&tensor);
        assert!(result.is_ok());
        if let Ok(report) = result {
            assert!(report.current_fitness >= 0.0);
            assert!(report.current_fitness <= 1.0);
        }
    }
}

#[test]
fn test_tick_with_degraded_tensor() {
    if let Ok(layer) = ObserverLayer::with_defaults() {
        let tensor = common::make_degraded_tensor();
        let result = layer.tick(&tensor);
        assert!(result.is_ok());
    }
}

#[test]
fn test_tick_fail_silent_with_nan_tensor() {
    if let Ok(layer) = ObserverLayer::with_defaults() {
        let mut tensor = Tensor12D::new([0.5; 12]);
        tensor.health_score = f64::NAN;
        let result = layer.tick(&tensor);
        // Tick should succeed even with NaN (fail-silent design)
        assert!(result.is_ok());
    }
}

// =========================================================================
// Group 3: Metrics (5 tests)
// =========================================================================

#[test]
fn test_initial_metrics_all_zero() {
    if let Ok(layer) = ObserverLayer::with_defaults() {
        let m = layer.metrics();
        assert_eq!(m.events_ingested, 0);
        assert_eq!(m.correlations_found, 0);
        assert_eq!(m.emergences_detected, 0);
        assert_eq!(m.mutations_proposed, 0);
        assert_eq!(m.observer_errors, 0);
        assert_eq!(m.ticks_executed, 0);
        assert_eq!(m.reports_generated, 0);
    }
}

#[test]
fn test_metrics_after_tick() {
    if let Ok(layer) = ObserverLayer::with_defaults() {
        let tensor = Tensor12D::new([0.5; 12]);
        let _ = layer.tick(&tensor);
        let m = layer.metrics();
        assert_eq!(m.ticks_executed, 1);
        assert_eq!(m.reports_generated, 1);
    }
}

#[test]
fn test_metrics_after_ingest() {
    if let Ok(layer) = ObserverLayer::with_defaults() {
        let _ = layer.ingest_event("health", "check", "{}");
        let m = layer.metrics();
        assert_eq!(m.events_ingested, 1);
    }
}

#[test]
fn test_metrics_error_count_on_nan() {
    if let Ok(layer) = ObserverLayer::with_defaults() {
        let mut tensor = Tensor12D::new([0.5; 12]);
        tensor.health_score = f64::NAN;
        let _ = layer.tick(&tensor);
        let m = layer.metrics();
        assert!(m.observer_errors >= 1, "NaN should cause an observer error");
    }
}

#[test]
fn test_metrics_multiple_ticks() {
    if let Ok(layer) = ObserverLayer::with_defaults() {
        let tensor = Tensor12D::new([0.5; 12]);
        for _ in 0..10 {
            let _ = layer.tick(&tensor);
        }
        let m = layer.metrics();
        assert_eq!(m.ticks_executed, 10);
        assert_eq!(m.reports_generated, 10);
    }
}

// =========================================================================
// Group 4: Event Ingestion (4 tests)
// =========================================================================

#[test]
fn test_ingest_event_returns_correlated_event() {
    if let Ok(layer) = ObserverLayer::with_defaults() {
        let result = layer.ingest_event("health", "service_check", "{}");
        assert!(result.is_ok());
        if let Ok(event) = result {
            assert_eq!(event.primary_event.channel, "health");
        }
    }
}

#[test]
fn test_ingest_multiple_events() {
    if let Ok(layer) = ObserverLayer::with_defaults() {
        for i in 0..5 {
            let result = layer.ingest_event("metrics", &format!("event_{i}"), "{}");
            assert!(result.is_ok());
        }
        assert_eq!(layer.metrics().events_ingested, 5);
    }
}

#[test]
fn test_ingest_event_different_channels() {
    if let Ok(layer) = ObserverLayer::with_defaults() {
        let _ = layer.ingest_event("health", "check", "{}");
        let _ = layer.ingest_event("metrics", "cpu", "{}");
        let _ = layer.ingest_event("remediation", "action", "{}");
        assert_eq!(layer.metrics().events_ingested, 3);
    }
}

#[test]
fn test_ingest_event_with_json_payload() {
    if let Ok(layer) = ObserverLayer::with_defaults() {
        let payload = r#"{"service":"synthex","health":0.95,"latency_ms":12}"#;
        let result = layer.ingest_event("health", "service_check", payload);
        assert!(result.is_ok());
    }
}

// =========================================================================
// Group 5: Fitness Evaluation (5 tests)
// =========================================================================

#[test]
fn test_fitness_evaluator_construction() {
    let config = FitnessConfig::default();
    let evaluator = FitnessEvaluator::with_config(config);
    assert_eq!(evaluator.history_len(), 0);
    assert!(evaluator.current_fitness().is_none());
}

#[test]
fn test_fitness_evaluator_evaluate() {
    let config = FitnessConfig::default();
    let evaluator = FitnessEvaluator::with_config(config);
    let tensor = Tensor12D::new([0.5; 12]);
    let result = evaluator.evaluate(&tensor, Some(1));
    assert!(result.is_ok());
    assert!(evaluator.current_fitness().is_some());
}

#[test]
fn test_fitness_healthy_vs_critical() {
    let config = FitnessConfig::default();
    let evaluator = FitnessEvaluator::with_config(config);

    let healthy = common::make_healthy_tensor();
    let _ = evaluator.evaluate(&healthy, Some(1));
    let healthy_fitness = evaluator.current_fitness();

    evaluator.clear_history();

    let critical = common::make_critical_tensor();
    let _ = evaluator.evaluate(&critical, Some(2));
    let critical_fitness = evaluator.current_fitness();

    if let (Some(hf), Some(cf)) = (healthy_fitness, critical_fitness) {
        assert!(
            hf > cf,
            "healthy fitness ({hf}) should be higher than critical ({cf})"
        );
    }
}

#[test]
fn test_system_state_before_tick() {
    if let Ok(layer) = ObserverLayer::with_defaults() {
        let state = layer.system_state();
        assert_eq!(state, SystemState::Healthy);
    }
}

#[test]
fn test_fitness_trend_before_tick() {
    if let Ok(layer) = ObserverLayer::with_defaults() {
        let trend = layer.fitness_trend();
        assert_eq!(trend, FitnessTrend::Unknown);
    }
}

// =========================================================================
// Group 6: Observer Bus (4 tests)
// =========================================================================

#[test]
fn test_observer_bus_default_channels() {
    let config = ObserverBusConfig::default();
    let bus = ObserverBus::with_config(config);
    assert!(bus.has_channel("correlation"));
}

#[test]
fn test_observer_bus_publish() {
    let config = ObserverBusConfig::default();
    let bus = ObserverBus::with_config(config);
    let result = bus.publish(
        "correlation",
        ObserverSource::Coordinator,
        ObserverMessageType::FitnessEvaluated,
        "test payload",
    );
    assert!(result.is_ok());
}

#[test]
fn test_observer_bus_stats_initial() {
    let config = ObserverBusConfig::default();
    let bus = ObserverBus::with_config(config);
    let stats = bus.stats();
    assert_eq!(stats.total_messages, 0);
}

#[test]
fn test_observer_bus_stats_after_publish() {
    let config = ObserverBusConfig::default();
    let bus = ObserverBus::with_config(config);
    let _ = bus.publish(
        "correlation",
        ObserverSource::Coordinator,
        ObserverMessageType::FitnessEvaluated,
        "test",
    );
    let stats = bus.stats();
    assert!(stats.total_messages >= 1);
}

// =========================================================================
// Group 7: RALPH Evolution (4 tests)
// =========================================================================

#[test]
fn test_ralph_initial_phase() {
    if let Ok(layer) = ObserverLayer::with_defaults() {
        let state = layer.ralph_state();
        assert_eq!(state.current_phase, RalphPhase::Recognize);
    }
}

#[test]
fn test_ralph_phase_advancement() {
    if let Ok(layer) = ObserverLayer::with_defaults() {
        // Must start a cycle before advancing phases
        let _ = layer.chamber().start_cycle();
        let phase = layer.advance_ralph_phase();
        assert!(phase.is_ok());
        if let Ok(p) = phase {
            assert_eq!(p, RalphPhase::Analyze);
        }
    }
}

#[test]
fn test_ralph_full_cycle() {
    if let Ok(layer) = ObserverLayer::with_defaults() {
        // Must start a cycle before advancing phases
        let _ = layer.chamber().start_cycle();
        // Advance through all 5 phases: R→A→L→P→H→R
        let phases = [
            RalphPhase::Analyze,
            RalphPhase::Learn,
            RalphPhase::Propose,
            RalphPhase::Harvest,
            RalphPhase::Recognize, // back to start
        ];
        for expected in &phases {
            let result = layer.advance_ralph_phase();
            assert!(result.is_ok());
            if let Ok(p) = result {
                assert_eq!(p, *expected);
            }
        }
        // Check that a full cycle was counted
        let m = layer.metrics();
        assert_eq!(m.ralph_cycles, 1);
    }
}

#[test]
fn test_ralph_recent_mutations_initially_empty() {
    if let Ok(layer) = ObserverLayer::with_defaults() {
        let mutations = layer.recent_mutations(10);
        assert!(mutations.is_empty());
    }
}

// =========================================================================
// Group 8: Clear and Prune (4 tests)
// =========================================================================

#[test]
fn test_clear_resets_all_state() {
    if let Ok(layer) = ObserverLayer::with_defaults() {
        let tensor = Tensor12D::new([0.5; 12]);
        let _ = layer.tick(&tensor);
        let _ = layer.ingest_event("health", "check", "{}");
        assert!(layer.tick_count() > 0);

        layer.clear();

        assert_eq!(layer.tick_count(), 0);
        assert!(layer.get_report().is_none());
        assert!(layer.report_history().is_empty());
        assert_eq!(layer.metrics().events_ingested, 0);
        assert_eq!(layer.metrics().ticks_executed, 0);
    }
}

#[test]
fn test_clear_then_tick_works() {
    if let Ok(layer) = ObserverLayer::with_defaults() {
        let tensor = Tensor12D::new([0.5; 12]);
        let _ = layer.tick(&tensor);
        layer.clear();
        let result = layer.tick(&tensor);
        assert!(result.is_ok());
        if let Ok(report) = result {
            assert_eq!(report.tick, 1);
        }
    }
}

#[test]
fn test_prune_returns_count() {
    if let Ok(layer) = ObserverLayer::with_defaults() {
        let _ = layer.ingest_event("health", "check", "{}");
        let future = chrono::Utc::now() + chrono::Duration::seconds(10);
        let pruned = layer.prune_before(future);
        let _ = pruned;
    }
}

#[test]
fn test_report_history_bounded() {
    if let Ok(layer) = ObserverLayer::with_defaults() {
        let tensor = Tensor12D::new([0.5; 12]);
        for _ in 0..150 {
            let _ = layer.tick(&tensor);
        }
        let history = layer.report_history();
        assert!(history.len() <= 100);
    }
}

// =========================================================================
// Group 9: Component Accessors (4 tests)
// =========================================================================

#[test]
fn test_bus_accessor() {
    if let Ok(layer) = ObserverLayer::with_defaults() {
        let bus = layer.bus();
        assert!(bus.has_channel("correlation"));
    }
}

#[test]
fn test_correlator_accessor() {
    if let Ok(layer) = ObserverLayer::with_defaults() {
        let correlator = layer.correlator();
        assert_eq!(correlator.buffer_len(), 0);
    }
}

#[test]
fn test_detector_accessor() {
    if let Ok(layer) = ObserverLayer::with_defaults() {
        let detector = layer.detector();
        assert_eq!(detector.history_len(), 0);
    }
}

#[test]
fn test_chamber_accessor() {
    if let Ok(layer) = ObserverLayer::with_defaults() {
        let chamber = layer.chamber();
        assert_eq!(chamber.generation(), 0);
    }
}

// =========================================================================
// Group 10: Concurrent Access (2 tests)
// =========================================================================

#[test]
fn test_concurrent_ticks() {
    use std::sync::Arc;

    if let Ok(layer) = ObserverLayer::with_defaults() {
        let layer = Arc::new(layer);
        let tensor = Tensor12D::new([0.5; 12]);

        let handles: Vec<_> = (0..4)
            .map(|_| {
                let layer = Arc::clone(&layer);
                std::thread::spawn(move || {
                    for _ in 0..5 {
                        let _ = layer.tick(&tensor);
                    }
                })
            })
            .collect();

        for h in handles {
            h.join().ok();
        }

        assert_eq!(layer.tick_count(), 20);
    }
}

#[test]
fn test_concurrent_ingest_and_tick() {
    use std::sync::Arc;

    if let Ok(layer) = ObserverLayer::with_defaults() {
        let layer = Arc::new(layer);

        let layer_tick = Arc::clone(&layer);
        let ticker = std::thread::spawn(move || {
            let tensor = Tensor12D::new([0.5; 12]);
            for _ in 0..10 {
                let _ = layer_tick.tick(&tensor);
            }
        });

        let layer_ingest = Arc::clone(&layer);
        let ingester = std::thread::spawn(move || {
            for i in 0..10 {
                let _ = layer_ingest.ingest_event("health", &format!("ev_{i}"), "{}");
            }
        });

        ticker.join().ok();
        ingester.join().ok();

        assert_eq!(layer.tick_count(), 10);
        assert_eq!(layer.metrics().events_ingested, 10);
    }
}
