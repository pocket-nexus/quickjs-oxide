//! The core's floating-point backend: `libm` and native Wasm operations.
//!
//! Keep operations available in `core` (such as `abs`, `copysign` and bit
//! conversions) native. Transcendentals may differ in their last bits from
//! platform libm; its architecture-specific optimizations remain enabled.

pub(crate) use libm::{
    atan, atan2, cbrt, cos, cosh, exp, expm1, log, log1p, log2, log10, sin, sinh, tan, tanh,
};

#[cfg(any(test, target_arch = "wasm32"))]
mod acos;
#[cfg(any(test, target_arch = "wasm32"))]
mod asin;
#[cfg(any(test, target_arch = "wasm32"))]
mod asinh;

#[cfg(target_arch = "wasm32")]
pub(crate) use self::{acos::acos, asin::asin, asinh::asinh};
#[cfg(not(target_arch = "wasm32"))]
pub(crate) use libm::{acos, asin, asinh, ceil, floor, pow, sqrt, trunc};
#[cfg(target_arch = "wasm32")]
pub(crate) use wasm::{ceil, floor, pow, sqrt, trunc};

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

    #[inline]
    pub(crate) fn pow(base: f64, exponent: f64) -> f64 {
        // libm's existing half-power shortcut also needs the hardware sqrt.
        // Both signs of zero produce +0 for a positive, noninteger exponent.
        if exponent == 0.5 && base >= 0.0 {
            sqrt(base.abs())
        } else {
            libm::pow(base, exponent)
        }
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

#[cfg(test)]
mod inverse_tests {
    fn same(actual: f64, expected: f64) {
        if expected.is_nan() {
            assert!(actual.is_nan());
        } else {
            assert_eq!(actual.to_bits(), expected.to_bits());
        }
    }

    #[test]
    fn inverse_functions_keep_libm_boundaries_and_signed_zero() {
        for value in [
            0.0,
            -0.0,
            f64::from_bits(1),
            -f64::from_bits(1),
            f64::MIN_POSITIVE,
            0.5,
            -0.5,
            0.975,
            -0.975,
            1.0,
            -1.0,
            2.0,
            -2.0,
            67_108_864.0,
            -67_108_864.0,
            f64::MAX,
            -f64::MAX,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NAN,
        ] {
            same(super::acos::acos(value), libm::acos(value));
            same(super::asin::asin(value), libm::asin(value));
            same(super::asinh::asinh(value), libm::asinh(value));
        }
        for boundary in [
            0x0010_0000_0000_0000u64,
            0x3c60_0000_0000_0000,
            0x3c60_0001_0000_0000,
            0x3e50_0000_0000_0000,
            0x3fe0_0000_0000_0000,
            0x3fef_3333_0000_0000,
            0x3ff0_0000_0000_0000,
            0x4000_0000_0000_0000,
            0x4190_0000_0000_0000,
        ] {
            for bits in [boundary - 1, boundary, boundary + 1] {
                for bits in [bits, bits | (1 << 63)] {
                    let value = f64::from_bits(bits);
                    same(super::acos::acos(value), libm::acos(value));
                    same(super::asin::asin(value), libm::asin(value));
                    same(super::asinh::asinh(value), libm::asinh(value));
                }
            }
        }
        for index in 1..2048 {
            let bits = (f64::from(index) / 1024.0 - 1.0).to_bits();
            for bits in [bits.wrapping_sub(1), bits, bits.wrapping_add(1)] {
                let value = f64::from_bits(bits);
                same(super::acos::acos(value), libm::acos(value));
                same(super::asin::asin(value), libm::asin(value));
                same(super::asinh::asinh(value), libm::asinh(value));
            }
        }
    }
}
