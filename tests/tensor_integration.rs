//! Integration tests for the 12D Tensor system across all usage sites.

mod common;

use maintenance_engine::Tensor12D;

// =========================================================================
// Group 1: Basic Tensor Operations (8 tests)
// =========================================================================

#[test]
fn test_tensor_new_stores_all_dimensions() {
    let dims = [0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 0.05, 0.01, 0.99];
    let tensor = Tensor12D::new(dims);
    let arr = tensor.to_array();
    for (i, (&expected, &actual)) in dims.iter().zip(arr.iter()).enumerate() {
        assert!(
            (expected - actual).abs() < f64::EPSILON,
            "dimension {i} mismatch: expected {expected}, got {actual}"
        );
    }
}

#[test]
fn test_tensor_default_all_zeros() {
    let tensor = Tensor12D::default();
    let arr = tensor.to_array();
    for (i, &val) in arr.iter().enumerate() {
        assert!(
            val.abs() < f64::EPSILON,
            "dimension {i} should be 0.0, got {val}"
        );
    }
}

#[test]
fn test_tensor_to_array_roundtrip() {
    let dims = [0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 0.05, 0.01, 0.99];
    let tensor = Tensor12D::new(dims);
    let arr = tensor.to_array();
    let tensor2 = Tensor12D::new(arr);
    let arr2 = tensor2.to_array();
    for (i, (&a, &b)) in arr.iter().zip(arr2.iter()).enumerate() {
        assert!(
            (a - b).abs() < f64::EPSILON,
            "dimension {i} changed after roundtrip"
        );
    }
}

#[test]
fn test_tensor_to_bytes_length() {
    let tensor = Tensor12D::new([0.5; 12]);
    let bytes = tensor.to_bytes();
    assert_eq!(bytes.len(), 96, "12 f64 values = 96 bytes");
}

#[test]
fn test_tensor_to_bytes_non_zero() {
    let tensor = Tensor12D::new([0.5; 12]);
    let bytes = tensor.to_bytes();
    let has_nonzero = bytes.iter().any(|&b| b != 0);
    assert!(has_nonzero, "bytes should contain non-zero values for 0.5");
}

#[test]
fn test_tensor_field_access_matches_array() {
    let dims = [0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 0.05, 0.01, 0.99];
    let tensor = Tensor12D::new(dims);
    assert!((tensor.service_id - dims[0]).abs() < f64::EPSILON);
    assert!((tensor.port - dims[1]).abs() < f64::EPSILON);
    assert!((tensor.tier - dims[2]).abs() < f64::EPSILON);
    assert!((tensor.dependency_count - dims[3]).abs() < f64::EPSILON);
    assert!((tensor.agent_count - dims[4]).abs() < f64::EPSILON);
    assert!((tensor.protocol - dims[5]).abs() < f64::EPSILON);
    assert!((tensor.health_score - dims[6]).abs() < f64::EPSILON);
    assert!((tensor.uptime - dims[7]).abs() < f64::EPSILON);
    assert!((tensor.synergy - dims[8]).abs() < f64::EPSILON);
    assert!((tensor.latency - dims[9]).abs() < f64::EPSILON);
    assert!((tensor.error_rate - dims[10]).abs() < f64::EPSILON);
    assert!((tensor.temporal_context - dims[11]).abs() < f64::EPSILON);
}

#[test]
fn test_tensor_clone() {
    let tensor = Tensor12D::new([0.42; 12]);
    let cloned = tensor;
    let arr_orig = tensor.to_array();
    let arr_clone = cloned.to_array();
    for (i, (&a, &b)) in arr_orig.iter().zip(arr_clone.iter()).enumerate() {
        assert!(
            (a - b).abs() < f64::EPSILON,
            "clone dimension {i} mismatch"
        );
    }
}

#[test]
fn test_tensor_debug_format() {
    let tensor = Tensor12D::new([0.5; 12]);
    let debug = format!("{tensor:?}");
    assert!(debug.contains("Tensor12D"));
    assert!(debug.contains("0.5"));
}

// =========================================================================
// Group 2: Validation (8 tests)
// =========================================================================

#[test]
fn test_tensor_validate_valid_range() {
    let tensor = Tensor12D::new([0.5; 12]);
    assert!(tensor.validate().is_ok());
}

#[test]
fn test_tensor_validate_all_zeros() {
    let tensor = Tensor12D::default();
    assert!(tensor.validate().is_ok());
}

#[test]
fn test_tensor_validate_all_ones() {
    let tensor = Tensor12D::new([1.0; 12]);
    assert!(tensor.validate().is_ok());
}

#[test]
fn test_tensor_validate_above_one_rejected() {
    let mut tensor = Tensor12D::new([0.5; 12]);
    tensor.health_score = 1.5;
    assert!(tensor.validate().is_err());
}

#[test]
fn test_tensor_validate_below_zero_rejected() {
    let mut tensor = Tensor12D::new([0.5; 12]);
    tensor.latency = -0.1;
    assert!(tensor.validate().is_err());
}

#[test]
fn test_tensor_validate_nan_rejected() {
    let mut tensor = Tensor12D::new([0.5; 12]);
    tensor.error_rate = f64::NAN;
    assert!(tensor.validate().is_err());
}

#[test]
fn test_tensor_validate_infinity_rejected() {
    let mut tensor = Tensor12D::new([0.5; 12]);
    tensor.uptime = f64::INFINITY;
    assert!(tensor.validate().is_err());
}

#[test]
fn test_tensor_validate_negative_infinity_rejected() {
    let mut tensor = Tensor12D::new([0.5; 12]);
    tensor.synergy = f64::NEG_INFINITY;
    assert!(tensor.validate().is_err());
}

// =========================================================================
// Group 3: Clamp and Normalize (5 tests)
// =========================================================================

#[test]
fn test_tensor_clamp_normalize_above_one() {
    let mut tensor = Tensor12D::new([0.5; 12]);
    tensor.health_score = 2.0;
    tensor.clamp_normalize();
    assert!((tensor.health_score - 1.0).abs() < f64::EPSILON);
    assert!(tensor.validate().is_ok());
}

#[test]
fn test_tensor_clamp_normalize_below_zero() {
    let mut tensor = Tensor12D::new([0.5; 12]);
    tensor.latency = -5.0;
    tensor.clamp_normalize();
    assert!(tensor.latency.abs() < f64::EPSILON);
    assert!(tensor.validate().is_ok());
}

#[test]
fn test_tensor_clamp_normalize_nan_becomes_half() {
    let mut tensor = Tensor12D::new([0.5; 12]);
    tensor.error_rate = f64::NAN;
    tensor.clamp_normalize();
    assert!((tensor.error_rate - 0.5).abs() < f64::EPSILON);
    assert!(tensor.validate().is_ok());
}

#[test]
fn test_tensor_clamp_normalize_preserves_valid_values() {
    let mut tensor = Tensor12D::new([0.42; 12]);
    tensor.clamp_normalize();
    let arr = tensor.to_array();
    for (i, &val) in arr.iter().enumerate() {
        assert!(
            (val - 0.42).abs() < f64::EPSILON,
            "dimension {i} should remain 0.42, got {val}"
        );
    }
}

#[test]
fn test_tensor_clamp_normalize_all_out_of_range() {
    let mut tensor = Tensor12D::new([5.0; 12]);
    tensor.clamp_normalize();
    let arr = tensor.to_array();
    for (i, &val) in arr.iter().enumerate() {
        assert!(
            (val - 1.0).abs() < f64::EPSILON,
            "dimension {i} should be clamped to 1.0, got {val}"
        );
    }
    assert!(tensor.validate().is_ok());
}

// =========================================================================
// Group 4: Distance Calculations (6 tests)
// =========================================================================

#[test]
fn test_tensor_distance_to_self_is_zero() {
    let tensor = Tensor12D::new([0.5; 12]);
    let dist = tensor.distance(&tensor);
    assert!(dist.abs() < f64::EPSILON, "distance to self should be 0, got {dist}");
}

#[test]
fn test_tensor_distance_symmetric() {
    let t1 = Tensor12D::new([0.1; 12]);
    let t2 = Tensor12D::new([0.9; 12]);
    let d1 = t1.distance(&t2);
    let d2 = t2.distance(&t1);
    assert!(
        (d1 - d2).abs() < f64::EPSILON,
        "distance should be symmetric: {d1} vs {d2}"
    );
}

#[test]
fn test_tensor_distance_positive() {
    let t1 = Tensor12D::new([0.0; 12]);
    let t2 = Tensor12D::new([1.0; 12]);
    let dist = t1.distance(&t2);
    assert!(dist > 0.0, "distance between different tensors should be positive");
}

#[test]
fn test_tensor_distance_triangle_inequality() {
    let t1 = Tensor12D::new([0.0; 12]);
    let t2 = Tensor12D::new([0.5; 12]);
    let t3 = Tensor12D::new([1.0; 12]);
    let d12 = t1.distance(&t2);
    let d23 = t2.distance(&t3);
    let d13 = t1.distance(&t3);
    assert!(
        d13 <= d12 + d23 + f64::EPSILON,
        "triangle inequality violated: d13={d13}, d12+d23={}", d12 + d23
    );
}

#[test]
fn test_tensor_distance_known_value() {
    // All zeros vs all ones: sqrt(12 * 1.0^2) = sqrt(12)
    let t1 = Tensor12D::default();
    let t2 = Tensor12D::new([1.0; 12]);
    let dist = t1.distance(&t2);
    let expected = 12.0_f64.sqrt();
    assert!(
        (dist - expected).abs() < 1e-10,
        "expected sqrt(12)={expected}, got {dist}"
    );
}

#[test]
fn test_tensor_distance_ordering() {
    let healthy = common::make_healthy_tensor();
    let degraded = common::make_degraded_tensor();
    let critical = common::make_critical_tensor();
    let max_tensor = common::make_max_tensor();

    let d_healthy = healthy.distance(&max_tensor);
    let d_critical = critical.distance(&max_tensor);
    // Healthy tensor should be closer to max than critical
    assert!(
        d_healthy < d_critical,
        "healthy should be closer to max than critical: {d_healthy} vs {d_critical}"
    );
    // Degraded should be between healthy and critical
    let d_degraded = degraded.distance(&max_tensor);
    assert!(
        d_healthy <= d_degraded + f64::EPSILON,
        "healthy should be closer to max than degraded"
    );
}

// =========================================================================
// Group 5: Tensor Fixtures (5 tests)
// =========================================================================

#[test]
fn test_fixture_healthy_tensor_validates() {
    let tensor = common::make_healthy_tensor();
    assert!(tensor.validate().is_ok());
}

#[test]
fn test_fixture_degraded_tensor_validates() {
    let tensor = common::make_degraded_tensor();
    assert!(tensor.validate().is_ok());
}

#[test]
fn test_fixture_critical_tensor_validates() {
    let tensor = common::make_critical_tensor();
    assert!(tensor.validate().is_ok());
}

#[test]
fn test_fixture_zero_tensor_validates() {
    let tensor = common::make_zero_tensor();
    assert!(tensor.validate().is_ok());
}

#[test]
fn test_fixture_max_tensor_validates() {
    let tensor = common::make_max_tensor();
    assert!(tensor.validate().is_ok());
}

// =========================================================================
// Group 6: Tensor with Engine (5 tests)
// =========================================================================

#[test]
fn test_engine_build_tensor_validates() {
    let engine = common::make_engine();
    let tensor = engine.build_tensor();
    assert!(tensor.validate().is_ok(), "engine tensor should be valid");
}

#[test]
fn test_engine_tensor_health_in_range() {
    let engine = common::make_engine();
    let tensor = engine.build_tensor();
    assert!(
        (0.0..=1.0).contains(&tensor.health_score),
        "health_score should be in [0,1], got {}",
        tensor.health_score
    );
}

#[test]
fn test_engine_tensor_synergy_in_range() {
    let engine = common::make_engine();
    let tensor = engine.build_tensor();
    assert!(
        (0.0..=1.0).contains(&tensor.synergy),
        "synergy should be in [0,1], got {}",
        tensor.synergy
    );
}

#[test]
fn test_engine_tensor_agent_count_in_range() {
    let engine = common::make_engine();
    let tensor = engine.build_tensor();
    assert!(
        (0.0..=1.0).contains(&tensor.agent_count),
        "agent_count should be in [0,1], got {}",
        tensor.agent_count
    );
}

#[test]
fn test_engine_tensor_all_dimensions_in_range() {
    let engine = common::make_engine();
    let tensor = engine.build_tensor();
    let arr = tensor.to_array();
    for (i, &val) in arr.iter().enumerate() {
        assert!(
            (0.0..=1.0).contains(&val),
            "dimension {i} out of range: {val}"
        );
    }
}

// =========================================================================
// Group 7: Tensor with Observer (4 tests)
// =========================================================================

#[test]
fn test_observer_tick_with_healthy_tensor() {
    let engine = common::make_engine();
    if let Some(obs) = engine.observer() {
        let tensor = common::make_healthy_tensor();
        let result = obs.tick(&tensor);
        assert!(result.is_ok());
    }
}

#[test]
fn test_observer_tick_with_degraded_tensor() {
    let engine = common::make_engine();
    if let Some(obs) = engine.observer() {
        let tensor = common::make_degraded_tensor();
        let result = obs.tick(&tensor);
        assert!(result.is_ok());
    }
}

#[test]
fn test_observer_fitness_differs_by_tensor() {
    let engine = common::make_engine();
    if let Some(obs) = engine.observer() {
        let healthy = common::make_healthy_tensor();
        let critical = common::make_critical_tensor();

        let r1 = obs.tick(&healthy);
        obs.clear();
        let r2 = obs.tick(&critical);

        if let (Ok(rep1), Ok(rep2)) = (r1, r2) {
            assert!(
                (rep1.current_fitness - rep2.current_fitness).abs() > 0.01,
                "fitness should differ between healthy and critical tensors"
            );
        }
    }
}

#[test]
fn test_observer_tick_with_engine_tensor() {
    let engine = common::make_engine();
    let tensor = engine.build_tensor();
    if let Some(obs) = engine.observer() {
        let result = obs.tick(&tensor);
        assert!(result.is_ok());
        assert_eq!(obs.tick_count(), 1);
    }
}
