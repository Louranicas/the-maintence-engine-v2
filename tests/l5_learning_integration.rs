//! # L5 Learning Layer Integration Tests
//!
//! Comprehensive cross-module integration tests for Layer 5 (Learning),
//! covering `HebbianManager`, `StdpProcessor`, `PatternRecognizer`, `PathwayPruner`,
//! `MemoryConsolidator`, and `AntiPatternDetector` interactions.
//!
//! ## Test Categories
//!
//! | Category | Tests | Description |
//! |----------|-------|-------------|
//! | HebbianManager | 15 | Pathway CRUD, LTP/LTD, routing, decay, pulse |
//! | StdpProcessor | 8 | Spike recording, window processing, weight calc |
//! | PatternRecognizer | 3 | Creation, detection, deactivation |
//! | PathwayPruner | 3 | Pruning weak pathways, threshold config |
//! | MemoryConsolidator | 2 | Creation, store + activate lifecycle |
//! | AntiPatternDetector | 7 | Detection, resolution, violation queries |
//! | Cross-module L5 | 4 | Decay+STDP, failure+prune, full cycle, bounds |

mod common;

use std::collections::HashMap;

use maintenance_engine::m5_learning::antipattern::{AntiPatternCategory, AntiPatternDetector};
use maintenance_engine::m5_learning::consolidator::MemoryConsolidator;
use maintenance_engine::m5_learning::hebbian::HebbianManager;
use maintenance_engine::m5_learning::pattern::{PatternRecognizer, PatternType};
use maintenance_engine::m5_learning::pruner::{PathwayPruner, PruningPolicy};
use maintenance_engine::m5_learning::stdp::{SpikeType, StdpProcessor};
use maintenance_engine::m5_learning::{MemoryLayer, PathwayType, PulseTrigger};

// =========================================================================
// HebbianManager Tests
// =========================================================================

#[test]
fn hebbian_new_loads_default_pathways() {
    let manager = HebbianManager::new();
    assert!(
        manager.pathway_count() >= 9,
        "HebbianManager::new() should load at least 9 default pathways, got {}",
        manager.pathway_count()
    );
}

#[test]
fn hebbian_add_pathway_returns_key() {
    let manager = HebbianManager::new();
    let key = manager
        .add_pathway("alpha", "beta", PathwayType::AgentToAgent)
        .expect("adding a new pathway should succeed");
    assert_eq!(key, "alpha->beta", "pathway key format should be source->target");
}

#[test]
fn hebbian_add_duplicate_pathway_rejected() {
    let manager = HebbianManager::new();
    manager
        .add_pathway("dup_src", "dup_tgt", PathwayType::ServiceToService)
        .expect("first add should succeed");
    let result = manager.add_pathway("dup_src", "dup_tgt", PathwayType::ServiceToService);
    assert!(
        result.is_err(),
        "adding a duplicate pathway should return Err"
    );
}

#[test]
fn hebbian_remove_pathway_succeeds_then_fails() {
    let manager = HebbianManager::new();
    let key = manager
        .add_pathway("rm_src", "rm_tgt", PathwayType::SystemToSystem)
        .expect("add should succeed");
    manager
        .remove_pathway(&key)
        .expect("first remove should succeed");
    let result = manager.remove_pathway(&key);
    assert!(
        result.is_err(),
        "removing an already-removed pathway should fail"
    );
}

#[test]
fn hebbian_get_pathway_returns_clone() {
    let manager = HebbianManager::new();
    let key = manager
        .add_pathway("get_s", "get_t", PathwayType::PatternToOutcome)
        .expect("add should succeed");
    let pathway = manager
        .get_pathway(&key)
        .expect("get_pathway should succeed for existing key");
    assert_eq!(pathway.source, "get_s");
    assert_eq!(pathway.target, "get_t");
    assert!(
        (pathway.strength - 0.5).abs() < f64::EPSILON,
        "default strength should be 0.5"
    );
}

#[test]
fn hebbian_strengthen_increases_strength() {
    let manager = HebbianManager::new();
    let key = manager
        .add_pathway("str_s", "str_t", PathwayType::ServiceToService)
        .expect("add should succeed");
    let initial = manager
        .get_pathway(&key)
        .expect("get should succeed")
        .strength;
    let after = manager.strengthen(&key).expect("strengthen should succeed");
    assert!(
        after > initial,
        "strength should increase after LTP: {after} > {initial}"
    );
    assert!(
        (after - 0.6).abs() < f64::EPSILON,
        "strength should be 0.6 after one LTP from 0.5"
    );
}

#[test]
fn hebbian_weaken_decreases_strength() {
    let manager = HebbianManager::new();
    let key = manager
        .add_pathway("wk_s", "wk_t", PathwayType::ServiceToService)
        .expect("add should succeed");
    let initial = manager
        .get_pathway(&key)
        .expect("get should succeed")
        .strength;
    let after = manager.weaken(&key).expect("weaken should succeed");
    assert!(
        after < initial,
        "strength should decrease after LTD: {after} < {initial}"
    );
    assert!(
        (after - 0.45).abs() < f64::EPSILON,
        "strength should be 0.45 after one LTD from 0.5"
    );
}

#[test]
fn hebbian_record_success_applies_ltp_and_increments_counts() {
    let manager = HebbianManager::new();
    let key = manager
        .add_pathway("succ_s", "succ_t", PathwayType::MetricToAction)
        .expect("add should succeed");
    let after = manager
        .record_success(&key)
        .expect("record_success should succeed");
    assert!(after > 0.5, "success should strengthen pathway via LTP");

    let pathway = manager
        .get_pathway(&key)
        .expect("get should succeed after record_success");
    assert_eq!(pathway.success_count, 1, "success_count should be 1");
    assert_eq!(pathway.activation_count, 1, "activation_count should be 1");
    assert_eq!(pathway.ltp_count, 1, "ltp_count should be 1");
}

#[test]
fn hebbian_record_failure_applies_ltd_and_increments_counts() {
    let manager = HebbianManager::new();
    let key = manager
        .add_pathway("fail_s", "fail_t", PathwayType::ConfigToBehavior)
        .expect("add should succeed");
    let after = manager
        .record_failure(&key)
        .expect("record_failure should succeed");
    assert!(after < 0.5, "failure should weaken pathway via LTD");

    let pathway = manager
        .get_pathway(&key)
        .expect("get should succeed after record_failure");
    assert_eq!(pathway.failure_count, 1, "failure_count should be 1");
    assert_eq!(pathway.activation_count, 1, "activation_count should be 1");
    assert_eq!(pathway.ltd_count, 1, "ltd_count should be 1");
}

#[test]
fn hebbian_routing_weight_is_strength_times_success_rate() {
    let manager = HebbianManager::new();
    let _ = manager
        .add_pathway("rw_s", "rw_t", PathwayType::ServiceToService)
        .expect("add should succeed");
    let weight = manager.get_routing_weight("rw_s", "rw_t");
    assert!(
        (weight - 0.25).abs() < f64::EPSILON,
        "routing weight should be 0.5 * 0.5 = 0.25, got {weight}"
    );
    let absent = manager.get_routing_weight("no", "path");
    assert!(
        absent.abs() < f64::EPSILON,
        "non-existent pathway routing weight should be 0.0"
    );
}

#[test]
fn hebbian_get_strongest_pathways_ordered() {
    let manager = HebbianManager::new();
    let key_strong = manager
        .add_pathway("strong_a", "strong_b", PathwayType::MetricToAction)
        .expect("add should succeed");
    for _ in 0..6 {
        let _ = manager.strengthen(&key_strong);
    }
    let strongest = manager.get_strongest_pathways(1);
    assert!(!strongest.is_empty(), "should return at least one pathway");
    assert!(
        strongest[0].strength >= 1.0 - f64::EPSILON,
        "strongest pathway should be at or near 1.0"
    );
}

#[test]
fn hebbian_get_weakest_pathways_ordered() {
    let manager = HebbianManager::new();
    let key_weak = manager
        .add_pathway("weak_a", "weak_b", PathwayType::ServiceToService)
        .expect("add should succeed");
    for _ in 0..7 {
        let _ = manager.weaken(&key_weak);
    }
    let weakest = manager.get_weakest_pathways(1);
    assert!(!weakest.is_empty(), "should return at least one pathway");
    assert!(
        weakest[0].strength <= 0.15 + f64::EPSILON,
        "weakest pathway should be near floor, got {}",
        weakest[0].strength
    );
}

#[test]
fn hebbian_apply_decay_reduces_all_strengths() {
    let manager = HebbianManager::new();
    let count = manager.pathway_count();
    let affected = manager.apply_decay();
    assert_eq!(
        affected, count,
        "decay should affect all pathways starting at 0.5"
    );
    let weakest = manager.get_weakest_pathways(1);
    assert!(
        weakest[0].strength < 0.5,
        "decayed pathway should be below 0.5"
    );
}

#[test]
fn hebbian_pulse_trigger_increments_counter() {
    let manager = HebbianManager::new();
    let pulse1 = manager
        .trigger_pulse(PulseTrigger::Manual)
        .expect("first pulse should succeed");
    assert_eq!(pulse1.pulse_number, 1, "first pulse should be number 1");
    assert_eq!(pulse1.trigger_type, PulseTrigger::Manual);
    assert!(pulse1.total_pathways > 0);
    assert!(pulse1.average_strength > 0.0);

    let pulse2 = manager
        .trigger_pulse(PulseTrigger::TimeInterval)
        .expect("second pulse should succeed");
    assert_eq!(pulse2.pulse_number, 2, "second pulse should be number 2");
}

#[test]
fn hebbian_pathway_metrics_track_activations() {
    let manager = HebbianManager::new();
    let key = manager
        .add_pathway("met_s", "met_t", PathwayType::ServiceToService)
        .expect("add should succeed");
    let _ = manager.strengthen(&key);
    let _ = manager.record_success(&key);

    let metrics = manager
        .get_metrics(&key)
        .expect("get_metrics should succeed");
    assert_eq!(metrics.pathway_key, key);
    assert!(
        metrics.total_ltp >= 2,
        "strengthen + record_success both apply LTP"
    );
}

// =========================================================================
// StdpProcessor Tests
// =========================================================================

#[test]
fn stdp_new_has_default_config() {
    let processor = StdpProcessor::new();
    let config = processor.get_config();
    assert!(
        (config.ltp_rate - 0.1).abs() < f64::EPSILON,
        "default ltp_rate should be 0.1"
    );
    assert!(
        (config.ltd_rate - 0.05).abs() < f64::EPSILON,
        "default ltd_rate should be 0.05"
    );
    assert_eq!(config.window_ms, 100, "default window should be 100ms");
}

#[test]
fn stdp_custom_config_retained() {
    let config = maintenance_engine::m5_learning::stdp::StdpConfig {
        ltp_rate: 0.2,
        ltd_rate: 0.1,
        tau_plus_ms: 30.0,
        tau_minus_ms: 25.0,
        window_ms: 200,
        weight_min: 0.05,
        weight_max: 0.95,
        decay_rate: 0.002,
    };
    let processor = StdpProcessor::with_config(config);
    let cfg = processor.get_config();
    assert!(
        (cfg.ltp_rate - 0.2).abs() < f64::EPSILON,
        "custom ltp_rate should be 0.2"
    );
    assert_eq!(cfg.window_ms, 200, "custom window should be 200ms");
}

#[test]
fn stdp_record_spike_validates_inputs() {
    let processor = StdpProcessor::new();
    processor
        .record_spike("src", "tgt", 100, SpikeType::PreSynaptic)
        .expect("valid spike should succeed");
    assert_eq!(processor.event_count(), 1);

    let err_empty_src = processor.record_spike("", "tgt", 200, SpikeType::PreSynaptic);
    assert!(err_empty_src.is_err(), "empty source should fail");

    let err_empty_tgt = processor.record_spike("src", "", 300, SpikeType::PostSynaptic);
    assert!(err_empty_tgt.is_err(), "empty target should fail");
}

#[test]
fn stdp_process_window_finds_pairs_within_window() {
    let processor = StdpProcessor::new();
    processor
        .record_spike("node_a", "node_b", 100, SpikeType::PreSynaptic)
        .expect("pre-spike recording should succeed");
    processor
        .record_spike("node_a", "node_b", 110, SpikeType::PostSynaptic)
        .expect("post-spike recording should succeed");
    processor
        .record_spike("node_c", "node_d", 105, SpikeType::PreSynaptic)
        .expect("different pathway spike should succeed");

    let pairs = processor
        .process_window()
        .expect("process_window should succeed");
    assert_eq!(pairs.len(), 1, "should find exactly one timing pair");
    assert_eq!(pairs[0].pre_id, "node_a");
    assert_eq!(pairs[0].post_id, "node_b");
    assert_eq!(pairs[0].delta_t_ms, 10, "delta_t should be 110 - 100 = 10");
    assert!(
        pairs[0].weight_change > 0.0,
        "positive delta_t should produce LTP (positive weight change)"
    );
}

#[test]
fn stdp_process_window_empty_events_returns_empty() {
    let processor = StdpProcessor::new();
    let pairs = processor
        .process_window()
        .expect("empty process_window should succeed");
    assert!(pairs.is_empty(), "no events means no pairs");
}

#[test]
fn stdp_weight_change_ltp_ltd_and_zero() {
    let processor = StdpProcessor::new();

    // Positive delta_t -> LTP: ltp_rate * exp(-dt / tau_plus)
    let potentiation_change = processor.calculate_weight_change(10);
    let potentiation_expected = 0.1 * (-10.0_f64 / 20.0).exp();
    assert!(
        (potentiation_change - potentiation_expected).abs() < 1e-10,
        "LTP formula mismatch: got {potentiation_change}, expected {potentiation_expected}"
    );

    // Negative delta_t -> LTD: -ltd_rate * exp(dt / tau_minus)
    let depression_change = processor.calculate_weight_change(-10);
    let depression_expected = -(0.05 * (-10.0_f64 / 20.0).exp());
    assert!(
        (depression_change - depression_expected).abs() < 1e-10,
        "LTD formula mismatch: got {depression_change}, expected {depression_expected}"
    );

    // Zero delta -> no change
    let zero = processor.calculate_weight_change(0);
    assert!(
        zero.abs() < f64::EPSILON,
        "zero delta_t should produce zero change"
    );
}

#[test]
fn stdp_apply_to_pathways_produces_keys() {
    let processor = StdpProcessor::new();
    processor
        .record_spike("svc_a", "svc_b", 1000, SpikeType::PreSynaptic)
        .expect("pre-spike should succeed");
    processor
        .record_spike("svc_a", "svc_b", 1015, SpikeType::PostSynaptic)
        .expect("post-spike should succeed");

    let pairs = processor
        .process_window()
        .expect("process_window should succeed");
    let updates = processor.apply_to_pathways(&pairs);
    assert_eq!(updates.len(), 1, "should produce one update");
    assert_eq!(
        updates[0].0, "svc_a->svc_b",
        "key format should be source->target"
    );
    assert!(
        updates[0].1 > 0.0,
        "positive delta_t should yield positive weight delta"
    );
}

#[test]
fn stdp_clear_old_events_removes_stale() {
    let processor = StdpProcessor::new();
    processor
        .record_spike("a", "b", 50, SpikeType::PreSynaptic)
        .expect("spike at t=50 should succeed");
    processor
        .record_spike("a", "b", 100, SpikeType::PreSynaptic)
        .expect("spike at t=100 should succeed");
    processor
        .record_spike("a", "b", 150, SpikeType::PostSynaptic)
        .expect("spike at t=150 should succeed");
    processor
        .record_spike("a", "b", 200, SpikeType::PostSynaptic)
        .expect("spike at t=200 should succeed");

    assert_eq!(processor.event_count(), 4);
    let removed = processor.clear_old_events(120);
    assert_eq!(removed, 2, "events at t=50 and t=100 should be removed");
    assert_eq!(processor.event_count(), 2);
}

// =========================================================================
// PatternRecognizer Tests
// =========================================================================

#[test]
fn pattern_recognizer_creation_and_registration() {
    let recognizer = PatternRecognizer::new();
    assert_eq!(recognizer.pattern_count(), 0, "new recognizer should be empty");

    let id = recognizer
        .register_pattern(
            "health-restart",
            PatternType::Failure,
            "health_fail->restart",
            "service_restart",
            vec!["synthex".into()],
        )
        .expect("registering a valid pattern should succeed");
    assert!(id.starts_with("PAT-"), "pattern ID should start with PAT-");
    assert_eq!(recognizer.pattern_count(), 1);
}

#[test]
fn pattern_recognizer_match_detection_updates_confidence() {
    let recognizer = PatternRecognizer::new();
    let id = recognizer
        .register_pattern(
            "latency-spike",
            PatternType::Metric,
            "latency>500ms",
            "cache_cleanup",
            vec!["nais".into()],
        )
        .expect("registration should succeed");

    recognizer
        .record_match(&id, "latency=750ms", 0.9)
        .expect("record_match should succeed");
    let pattern = recognizer
        .get_pattern(&id)
        .expect("pattern should exist after match");
    assert_eq!(
        pattern.occurrence_count, 1,
        "occurrence should be 1 after one match"
    );
    assert!(
        pattern.confidence > 0.0,
        "confidence should be positive after a match"
    );
}

#[test]
fn pattern_recognizer_deactivation_excludes_from_signature_search() {
    let recognizer = PatternRecognizer::new();
    let id = recognizer
        .register_pattern(
            "cascade-fail",
            PatternType::Failure,
            "cascade_failure_pattern",
            "full_restart",
            vec!["synthex".into(), "san-k7".into()],
        )
        .expect("registration should succeed");

    assert_eq!(
        recognizer.find_by_signature("cascade").len(),
        1,
        "should find active pattern by signature"
    );

    recognizer
        .deactivate(&id)
        .expect("deactivate should succeed");
    assert!(
        recognizer.find_by_signature("cascade").is_empty(),
        "deactivated pattern should not appear in signature search"
    );
    assert_eq!(
        recognizer.active_pattern_count(),
        0,
        "active count should be 0 after deactivation"
    );
}

// =========================================================================
// PathwayPruner Tests
// =========================================================================

#[test]
fn pruner_creation_and_add_pathway() {
    let pruner = PathwayPruner::new();
    assert_eq!(pruner.pathway_count(), 0, "new pruner should be empty");

    let pathway = common::make_pathway("prune_s", "prune_t");
    pruner
        .add_pathway(pathway)
        .expect("adding pathway to pruner should succeed");
    assert_eq!(pruner.pathway_count(), 1);
}

#[test]
fn pruner_prune_weak_removes_low_strength_pathways() {
    let config = PruningPolicy::Aggressive.to_config();
    let pruner = PathwayPruner::with_config(config);

    // Add a weak pathway (strength 0.1 < aggressive min_strength 0.3)
    let weak = common::make_pathway_with_strength("weak_s", "weak_t", 0.1);
    pruner
        .add_pathway(weak)
        .expect("adding weak pathway should succeed");

    // Add a strong pathway (strength 0.8 >= 0.3)
    let mut strong = common::make_pathway("strong_s", "strong_t");
    strong.strength = 0.8;
    strong.activation_count = 20;
    strong.success_count = 8;
    strong.failure_count = 2;
    strong.last_activation = Some(std::time::SystemTime::now());
    strong.last_success = Some(std::time::SystemTime::now());
    pruner
        .add_pathway(strong)
        .expect("adding strong pathway should succeed");

    let report = pruner.prune().expect("prune should succeed");
    assert!(
        report.pruned_count >= 1,
        "at least the weak pathway should be pruned"
    );
    assert!(
        pruner.pathway_count() <= 1,
        "only the strong pathway should remain"
    );
}

#[test]
fn pruner_threshold_config_conservative_vs_aggressive() {
    let conservative = PruningPolicy::Conservative.to_config();
    let aggressive = PruningPolicy::Aggressive.to_config();

    assert!(
        conservative.min_strength < aggressive.min_strength,
        "conservative min_strength ({}) should be less than aggressive ({})",
        conservative.min_strength,
        aggressive.min_strength
    );
    assert!(
        conservative.inactive_days > aggressive.inactive_days,
        "conservative inactive_days ({}) should be more than aggressive ({})",
        conservative.inactive_days,
        aggressive.inactive_days
    );
    assert!(
        conservative.min_activations < aggressive.min_activations,
        "conservative min_activations ({}) should be less than aggressive ({})",
        conservative.min_activations,
        aggressive.min_activations
    );
}

// =========================================================================
// MemoryConsolidator Tests
// =========================================================================

#[test]
fn consolidator_creation_and_store() {
    let consolidator = MemoryConsolidator::new();
    assert_eq!(consolidator.item_count(), 0, "new consolidator should be empty");

    let id = consolidator
        .store("pathway", "p1", HashMap::new())
        .expect("store should succeed");
    assert!(id.starts_with("MEM-"), "item ID should start with MEM-");
    assert_eq!(consolidator.item_count(), 1);
    assert_eq!(
        consolidator.layer_count(MemoryLayer::Working),
        1,
        "new items should start in Working layer"
    );
}

#[test]
fn consolidator_activate_and_record_success_failure() {
    let consolidator = MemoryConsolidator::new();
    let id = consolidator
        .store("pattern", "test_pattern", HashMap::new())
        .expect("store should succeed");

    consolidator
        .activate_item(&id)
        .expect("activate should succeed");
    consolidator
        .record_item_success(&id)
        .expect("record_success should succeed");
    consolidator
        .record_item_failure(&id)
        .expect("record_failure should succeed");

    let item = consolidator
        .get_item(&id)
        .expect("item should exist after operations");
    assert_eq!(item.activation_count, 1, "activation_count should be 1");
    assert_eq!(item.success_count, 1, "success_count should be 1");
    assert_eq!(item.failure_count, 1, "failure_count should be 1");
}

// =========================================================================
// AntiPatternDetector Tests
// =========================================================================

#[test]
fn antipattern_new_loads_fifteen_defaults() {
    let detector = AntiPatternDetector::new();
    assert!(
        detector.pattern_count() >= 15,
        "should have at least 15 default anti-patterns, got {}",
        detector.pattern_count()
    );
}

#[test]
fn antipattern_detect_violation_records_detection() {
    let detector = AntiPatternDetector::new();
    let detection = detector
        .detect("AP-C001", "Found .unwrap() in handler.rs:42")
        .expect("detecting a known pattern should succeed");
    assert_eq!(detection.pattern_id, "AP-C001");
    assert!(
        (detection.severity - 1.0).abs() < f64::EPSILON,
        "AP-C001 severity should be 1.0"
    );
    assert!(!detection.resolved, "new detection should be unresolved");
}

#[test]
fn antipattern_detect_unknown_pattern_fails() {
    let detector = AntiPatternDetector::new();
    let result = detector.detect("AP-NONEXISTENT", "context");
    assert!(result.is_err(), "detecting unknown pattern should fail");
}

#[test]
fn antipattern_resolve_marks_as_resolved() {
    let detector = AntiPatternDetector::new();
    let detection = detector
        .detect("AP-C002", "unsafe block in module.rs")
        .expect("detect should succeed");
    detector
        .resolve(&detection.id)
        .expect("resolve should succeed");

    let unresolved = detector.get_unresolved();
    assert!(
        !unresolved.iter().any(|d| d.id == detection.id),
        "resolved detection should not appear in unresolved list"
    );
}

#[test]
fn antipattern_get_unresolved_returns_newest_first() {
    let detector = AntiPatternDetector::new();
    let _ = detector
        .detect("AP-C001", "first violation")
        .expect("first detect should succeed");
    let _ = detector
        .detect("AP-C002", "second violation")
        .expect("second detect should succeed");
    let _ = detector
        .detect("AP-C003", "third violation")
        .expect("third detect should succeed");

    let unresolved = detector.get_unresolved();
    assert_eq!(unresolved.len(), 3, "should have 3 unresolved detections");
    assert_eq!(
        unresolved[0].pattern_id, "AP-C003",
        "newest detection should be first"
    );
}

#[test]
fn antipattern_most_frequent_returns_top_n() {
    let detector = AntiPatternDetector::new();
    for _ in 0..5 {
        let _ = detector
            .detect("AP-C001", "unwrap violation")
            .expect("detect AP-C001 should succeed");
    }
    for _ in 0..3 {
        let _ = detector
            .detect("AP-W001", "edit without read")
            .expect("detect AP-W001 should succeed");
    }
    let _ = detector
        .detect("AP-A001", "bypass consensus")
        .expect("detect AP-A001 should succeed");

    let top = detector.most_frequent_violations(2);
    assert_eq!(top.len(), 2, "should return top 2");
    assert_eq!(top[0].0, "AP-C001", "most frequent should be AP-C001");
    assert_eq!(top[0].1, 5, "AP-C001 should have 5 violations");
    assert_eq!(top[1].0, "AP-W001", "second most frequent should be AP-W001");
    assert_eq!(top[1].1, 3, "AP-W001 should have 3 violations");
}

#[test]
fn antipattern_detections_by_category_filters_correctly() {
    let detector = AntiPatternDetector::new();
    let _ = detector
        .detect("AP-C001", "code issue 1")
        .expect("detect AP-C001 should succeed");
    let _ = detector
        .detect("AP-C002", "code issue 2")
        .expect("detect AP-C002 should succeed");
    let _ = detector
        .detect("AP-W001", "workflow issue")
        .expect("detect AP-W001 should succeed");
    let _ = detector
        .detect("AP-A001", "architecture issue")
        .expect("detect AP-A001 should succeed");
    let _ = detector
        .detect("AP-X001", "consensus issue")
        .expect("detect AP-X001 should succeed");

    let code = detector.get_detections_by_category(AntiPatternCategory::Code);
    assert_eq!(code.len(), 2, "should have 2 Code category detections");

    let workflow = detector.get_detections_by_category(AntiPatternCategory::Workflow);
    assert_eq!(workflow.len(), 1, "should have 1 Workflow category detection");

    let arch = detector.get_detections_by_category(AntiPatternCategory::Architecture);
    assert_eq!(arch.len(), 1, "should have 1 Architecture category detection");

    let consensus = detector.get_detections_by_category(AntiPatternCategory::Consensus);
    assert_eq!(consensus.len(), 1, "should have 1 Consensus category detection");
}

// =========================================================================
// Cross-Module L5 Workflow Tests
// =========================================================================

#[test]
fn cross_decay_then_stdp_reinforcement() {
    let manager = HebbianManager::new();
    let key = manager
        .add_pathway("decay_s", "decay_t", PathwayType::ServiceToService)
        .expect("add should succeed");

    // Apply decay 10 times: each reduces by 0.001
    for _ in 0..10 {
        manager.apply_decay();
    }
    let after_decay = manager
        .get_pathway(&key)
        .expect("pathway should exist after decay")
        .strength;
    assert!(
        after_decay < 0.5,
        "strength should be reduced after decay cycles"
    );
    assert!(
        (after_decay - 0.49).abs() < 1e-10,
        "expected ~0.49 after 10 decay cycles, got {after_decay}"
    );

    // Reinforce via record_success (applies LTP)
    let _ = manager
        .record_success(&key)
        .expect("record_success should succeed");
    let after_reinforce = manager
        .get_pathway(&key)
        .expect("pathway should exist after reinforcement")
        .strength;
    assert!(
        after_reinforce > after_decay,
        "STDP reinforcement (success) should restore strength above decayed value"
    );
}

#[test]
fn cross_repeated_failures_weaken_below_prune_threshold() {
    let manager = HebbianManager::new();
    let key = manager
        .add_pathway("fragile_s", "fragile_t", PathwayType::ServiceToService)
        .expect("add should succeed");

    // Fail 8 times: each applies LTD (-0.05), from 0.5 -> 0.1 floor
    for _ in 0..8 {
        let _ = manager
            .record_failure(&key)
            .expect("record_failure should succeed");
    }
    let weakened = manager
        .get_pathway(&key)
        .expect("pathway should exist");
    assert!(
        weakened.strength <= 0.1 + f64::EPSILON,
        "strength should be at floor (0.1) after 8 failures"
    );

    // Feed into pruner with Moderate config (min_strength 0.2)
    let pruner = PathwayPruner::new();
    pruner
        .add_pathway(weakened)
        .expect("adding weakened pathway to pruner should succeed");
    let candidates = pruner.identify_candidates();
    assert!(
        !candidates.is_empty(),
        "weakened pathway should be a pruning candidate"
    );
}

#[test]
fn cross_full_learning_cycle_decay_stdp_pattern() {
    // Full L5 cycle: HebbianManager + StdpProcessor + PatternRecognizer

    // 1. Create pathway and register a pattern
    let manager = HebbianManager::new();
    let key = manager
        .add_pathway("cycle_s", "cycle_t", PathwayType::PatternToOutcome)
        .expect("add should succeed");

    let recognizer = PatternRecognizer::new();
    let pat_id = recognizer
        .register_pattern(
            "cycle-pattern",
            PatternType::Pathway,
            "cycle_s->cycle_t",
            "strengthen",
            vec!["cycle_s".into()],
        )
        .expect("pattern registration should succeed");

    // 2. Simulate STDP spike timing
    let processor = StdpProcessor::new();
    processor
        .record_spike("cycle_s", "cycle_t", 1000, SpikeType::PreSynaptic)
        .expect("pre-spike should succeed");
    processor
        .record_spike("cycle_s", "cycle_t", 1010, SpikeType::PostSynaptic)
        .expect("post-spike should succeed");

    let pairs = processor
        .process_window()
        .expect("process_window should succeed");
    assert_eq!(pairs.len(), 1, "should find one STDP timing pair");

    let updates = processor.apply_to_pathways(&pairs);
    assert_eq!(updates.len(), 1);
    assert_eq!(updates[0].0, key, "update key should match pathway key");
    assert!(
        updates[0].1 > 0.0,
        "positive delta_t should produce positive weight change"
    );

    // 3. Apply the STDP update by recording success
    let _ = manager
        .record_success(&key)
        .expect("record_success should succeed");

    // 4. Record pattern match
    recognizer
        .record_match(&pat_id, "cycle activation", 0.85)
        .expect("record_match should succeed");

    // 5. Apply decay
    manager.apply_decay();

    // 6. Verify final state: pathway stronger than initial 0.5
    let final_pathway = manager
        .get_pathway(&key)
        .expect("pathway should exist after full cycle");
    // 0.5 + 0.1 (LTP from success) - 0.001 (decay) = 0.599
    assert!(
        final_pathway.strength > 0.5,
        "pathway should be net-strengthened after success + decay: got {}",
        final_pathway.strength
    );
    assert!(
        final_pathway.success_count >= 1,
        "should have at least one success recorded"
    );

    // Verify pattern was updated
    let pattern = recognizer
        .get_pattern(&pat_id)
        .expect("pattern should still exist");
    assert_eq!(
        pattern.occurrence_count, 1,
        "pattern should have 1 occurrence"
    );
}

#[test]
fn cross_pathway_strength_bounded_after_many_ltp_ltd() {
    let manager = HebbianManager::new();
    let key = manager
        .add_pathway("bound_s", "bound_t", PathwayType::ServiceToService)
        .expect("add should succeed");

    // Apply LTP 50 times -- should cap at 1.0
    for _ in 0..50 {
        let _ = manager.strengthen(&key);
    }
    let strong = manager
        .get_pathway(&key)
        .expect("pathway should exist")
        .strength;
    assert!(
        strong <= 1.0 + f64::EPSILON,
        "strength must not exceed 1.0, got {strong}"
    );
    assert!(
        (strong - 1.0).abs() < f64::EPSILON,
        "should reach 1.0 cap after 50 LTP events"
    );

    // Apply LTD 100 times -- should floor at 0.1
    for _ in 0..100 {
        let _ = manager.weaken(&key);
    }
    let weak = manager
        .get_pathway(&key)
        .expect("pathway should exist")
        .strength;
    assert!(
        weak >= 0.1 - f64::EPSILON,
        "strength must not go below 0.1 (MIN_STRENGTH), got {weak}"
    );

    // Verify using common helper
    let pathway = manager
        .get_pathway(&key)
        .expect("pathway should exist for bounds check");
    common::assert_pathway_bounded(&pathway);
}
