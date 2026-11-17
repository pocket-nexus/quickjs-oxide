//! The core's floating-point backend: `libm` and native Wasm operations.
//!
//! Keep operations available in `core` (such as `abs`, `copysign` and bit
//! conversions) native. Transcendentals may differ in their last bits from
//! platform libm; its architecture-specific optimizations remain enabled.

pub(crate) use libm::{
    acos, asin, asinh, atan, atan2, cbrt, cos, cosh, exp, expm1, log, log1p, log2, log10, pow, sin,
    sinh, tan, tanh,
};

#[cfg(not(target_arch = "wasm32"))]
pub(crate) use libm::{ceil, floor, sqrt, trunc};
#[cfg(target_arch = "wasm32")]
pub(crate) use wasm::{ceil, floor, sqrt, trunc};

#[cfg(target_arch = "wasm32")]
mod wasm {
    use core::arch::wasm32::{
        f64x2_ceil, f64x2_extract_lane, f64x2_floor, f64x2_splat, f64x2_sqrt, f64x2_trunc,
    };

    // Scalar Wasm intrinsics are nightly-only. These stable, safe SIMD forms
    // scalarize to f64 instructions when only lane zero is observed (Rust 1.99).
    // Keep the feature local; do not enable SIMD for the rest of the engine.
    #[inline]
    #[target_feature(enable = "simd128")]
    pub(crate) fn sqrt(value: f64) -> f64 {
        f64x2_extract_lane::<0>(f64x2_sqrt(f64x2_splat(value)))
    }

    #[inline]
    #[target_feature(enable = "simd128")]
    pub(crate) fn floor(value: f64) -> f64 {
        f64x2_extract_lane::<0>(f64x2_floor(f64x2_splat(value)))
    }

    #[inline]
    #[target_feature(enable = "simd128")]
    pub(crate) fn ceil(value: f64) -> f64 {
        f64x2_extract_lane::<0>(f64x2_ceil(f64x2_splat(value)))
    }

    #[inline]
    #[target_feature(enable = "simd128")]
    pub(crate) fn trunc(value: f64) -> f64 {
        f64x2_extract_lane::<0>(f64x2_trunc(f64x2_splat(value)))
    }
}

/// Match `f64::rem_euclid`: correct a negative remainder with the absolute
/// modulus. An exact negative multiple retains -0; rounding can produce the
/// absolute modulus for a tiny negative input, just as with the native method.
#[inline]
pub(crate) fn rem_euclid(value: f64, modulus: f64) -> f64 {
    let remainder = value % modulus;
    if remainder < 0.0 {
        remainder + modulus.abs()
    } else {
        remainder
    }
}

/// Match `f64::fract` by subtracting truncation, including +0 for negative
/// integers and NaN for infinities. Negative nonintegral inputs retain their sign.
#[inline]
pub(crate) fn fract(value: f64) -> f64 {
    value - trunc(value)
}

/// Match `f64::log(base)`'s quotient. Use only for estimates whose exact
/// comparisons subsequently correct the result (the radix formatter).
#[inline]
pub(crate) fn log_base(value: f64, base: f64) -> f64 {
    log(value) / log(base)
}

#[cfg(test)]
mod tests {
    use super::{fract, rem_euclid};

    #[test]
    fn fractional_part_edges() {
        assert_eq!(fract(-1.25), -0.25);
        assert_eq!(fract(-1.0).to_bits(), 0.0f64.to_bits());
        assert_eq!(fract(-0.0).to_bits(), 0.0f64.to_bits());
        assert_eq!(
            fract(-f64::from_bits(1)).to_bits(),
            (-f64::from_bits(1)).to_bits()
        );
        assert!(fract(f64::INFINITY).is_nan());
        assert!(fract(f64::NEG_INFINITY).is_nan());
        assert!(fract(f64::NAN).is_nan());
    }

    #[test]
    fn euclidean_remainder_edges() {
        assert_eq!(rem_euclid(-5.0, -3.0), 1.0);
        assert_eq!(rem_euclid(-6.0, 3.0).to_bits(), (-0.0f64).to_bits());
        assert_eq!(rem_euclid(-f64::from_bits(1), 3.0), 3.0);
        assert!(rem_euclid(1.0, 0.0).is_nan());
        assert!(rem_euclid(f64::INFINITY, 3.0).is_nan());
        assert!(rem_euclid(f64::NAN, 3.0).is_nan());
    }
}
