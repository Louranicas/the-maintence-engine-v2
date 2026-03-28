//! Property-based tests for Tensor12D invariants.
//!
//! Uses proptest to verify mathematical properties that must hold
//! for all possible tensor values.

use maintenance_engine::Tensor12D;
use proptest::prelude::*;

/// Strategy that generates a valid Tensor12D (all dims in [0,1])
fn arb_valid_tensor() -> impl Strategy<Value = Tensor12D> {
    prop::array::uniform12(0.0_f64..=1.0).prop_map(Tensor12D::new)
}

/// Strategy that generates an arbitrary Tensor12D (dims can be out of range)
fn arb_any_tensor() -> impl Strategy<Value = Tensor12D> {
    prop::array::uniform12(-10.0_f64..=10.0).prop_map(Tensor12D::new)
}

proptest! {
    /// Valid tensors (all dims in [0,1]) must always pass validation.
    #[test]
    fn tensor_validate_accepts_valid(dims in prop::array::uniform12(0.0_f64..=1.0)) {
        let t = Tensor12D::new(dims);
        prop_assert!(t.validate().is_ok());
    }

    /// After clamp_normalize, any tensor must pass validation.
    #[test]
    fn tensor_clamp_always_produces_valid(dims in prop::array::uniform12(-10.0_f64..=10.0)) {
        let mut t = Tensor12D::new(dims);
        t.clamp_normalize();
        prop_assert!(t.validate().is_ok(), "clamped tensor failed validation");
    }

    /// After clamp_normalize, all dimensions must be in [0,1].
    #[test]
    fn tensor_clamp_all_dims_in_range(dims in prop::array::uniform12(-100.0_f64..=100.0)) {
        let mut t = Tensor12D::new(dims);
        t.clamp_normalize();
        for (i, &v) in t.to_array().iter().enumerate() {
            prop_assert!(
                (0.0..=1.0).contains(&v),
                "dim {} = {} after clamp, expected [0,1]",
                i, v
            );
        }
    }

    /// Distance must be symmetric: d(a,b) == d(b,a)
    #[test]
    fn tensor_distance_symmetric(
        a in arb_valid_tensor(),
        b in arb_valid_tensor(),
    ) {
        let d1 = a.distance(&b);
        let d2 = b.distance(&a);
        prop_assert!(
            (d1 - d2).abs() < 1e-10,
            "distance not symmetric: d(a,b)={}, d(b,a)={}",
            d1, d2
        );
    }

    /// Distance must be non-negative.
    #[test]
    fn tensor_distance_non_negative(
        a in arb_valid_tensor(),
        b in arb_valid_tensor(),
    ) {
        let d = a.distance(&b);
        prop_assert!(d >= 0.0, "distance was negative: {}", d);
    }

    /// Distance to self must be zero.
    #[test]
    fn tensor_distance_to_self_is_zero(t in arb_valid_tensor()) {
        let d = t.distance(&t);
        prop_assert!(d.abs() < 1e-10, "distance to self was {}", d);
    }

    /// Triangle inequality: d(a,c) <= d(a,b) + d(b,c)
    #[test]
    fn tensor_distance_triangle_inequality(
        a in arb_valid_tensor(),
        b in arb_valid_tensor(),
        c in arb_valid_tensor(),
    ) {
        let d_ac = a.distance(&c);
        let d_ab = a.distance(&b);
        let d_bc = b.distance(&c);
        prop_assert!(
            d_ac <= d_ab + d_bc + 1e-10,
            "triangle inequality violated: d(a,c)={} > d(a,b)+d(b,c)={}",
            d_ac, d_ab + d_bc
        );
    }

    /// to_array roundtrip: Tensor12D::new(t.to_array()) should produce same tensor
    #[test]
    fn tensor_to_array_roundtrip(dims in prop::array::uniform12(0.0_f64..=1.0)) {
        let t = Tensor12D::new(dims);
        let arr = t.to_array();
        let t2 = Tensor12D::new(arr);
        let d = t.distance(&t2);
        prop_assert!(d.abs() < 1e-10, "roundtrip distance was {}", d);
    }

    /// to_bytes produces exactly 96 bytes (12 * 8)
    #[test]
    fn tensor_to_bytes_length(dims in prop::array::uniform12(0.0_f64..=1.0)) {
        let t = Tensor12D::new(dims);
        let bytes = t.to_bytes();
        prop_assert_eq!(bytes.len(), 96);
    }

    /// Tensors with any dim outside [0,1] must fail validation
    #[test]
    fn tensor_out_of_range_fails_validation(
        dims in prop::array::uniform12(0.0_f64..=1.0),
        bad_dim in 0_usize..12,
        bad_val in prop::num::f64::ANY.prop_filter("must be out of range", |v| {
            !(0.0..=1.0).contains(v) && !v.is_nan()
        })
    ) {
        let mut arr = dims;
        arr[bad_dim] = bad_val;
        let t = Tensor12D::new(arr);
        prop_assert!(t.validate().is_err());
    }

    /// NaN dimensions should be replaced with 0.5 by clamp_normalize
    #[test]
    fn tensor_clamp_replaces_nan(
        dims in prop::array::uniform12(0.0_f64..=1.0),
        nan_dim in 0_usize..12,
    ) {
        let mut arr = dims;
        arr[nan_dim] = f64::NAN;
        let mut t = Tensor12D::new(arr);
        t.clamp_normalize();
        let result = t.to_array();
        prop_assert!(
            (result[nan_dim] - 0.5).abs() < 1e-10,
            "NaN dim {} should be 0.5, got {}",
            nan_dim, result[nan_dim]
        );
    }
}
