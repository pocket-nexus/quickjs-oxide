//! Integer conversion kernels after JavaScript ToNumber has completed.

/// The pure numeric part of QuickJS `JS_ToInt32Sat` after `ToNumber` has
/// completed. NaN becomes zero, finite values truncate toward zero, and both
/// finite overflow and infinities saturate.
#[must_use]
pub fn to_int32_sat(value: f64) -> i32 {
    if value.is_nan() {
        0
    } else if value < f64::from(i32::MIN) {
        i32::MIN
    } else if value > f64::from(i32::MAX) {
        i32::MAX
    } else {
        value as i32
    }
}

/// The numeric kernel of ECMAScript `ToInt32` after `ToNumber` completes.
///
/// Unlike [`to_int32_sat`], this path truncates and then reduces modulo 2^32;
/// global `parseInt` uses it for its radix argument.
#[must_use]
#[allow(clippy::cast_possible_truncation)]
pub fn to_int32(value: f64) -> i32 {
    // A binary64 number already contains its integer bits. As in pinned
    // QuickJS's JS_ToInt32Free, select those bits instead of computing fmod.
    // to_bits is a safe representation conversion on every Rust target.
    let bits = value.to_bits();
    let exponent = ((bits >> 52) & 0x7ff) as u32;
    if exponent <= 1023 + 30 {
        // Finite magnitude < 2^31, including subnormals and either zero.
        // The ordinary Rust cast truncates toward zero and is in range.
        value as i32
    } else if exponent <= 1023 + 83 {
        let significand = (bits & ((1_u64 << 52) - 1)) | (1_u64 << 52);
        // For unbiased exponents 31..83, this shift is 11..63. Losing
        // high bits performs modulo 2^32; dropping the low 32 truncates
        // fractional bits. Neither shift can exceed its operand width.
        let unsigned = ((significand << (exponent - (1023 + 20))) >> 32) as u32;
        let signed = unsigned as i32;
        if bits >> 63 != 0 {
            signed.wrapping_neg()
        } else {
            signed
        }
    } else {
        // Finite values here are multiples of 2^32. The same branch also
        // handles both infinities and every NaN payload, as ECMAScript requires.
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn int32_sat_matches_quickjs_numeric_kernel() {
        assert_eq!(to_int32_sat(f64::NAN), 0);
        assert_eq!(to_int32_sat(-0.0), 0);
        assert_eq!(to_int32_sat(2.9), 2);
        assert_eq!(to_int32_sat(-2.9), -2);
        assert_eq!(to_int32_sat(f64::INFINITY), i32::MAX);
        assert_eq!(to_int32_sat(f64::NEG_INFINITY), i32::MIN);
        assert_eq!(to_int32_sat(4_294_967_298.0), i32::MAX);
        assert_eq!(to_int32_sat(-4_294_967_298.0), i32::MIN);
    }

    #[test]
    fn int32_wraps_for_parse_int_radices() {
        assert_eq!(to_int32(f64::NAN), 0);
        assert_eq!(to_int32(f64::INFINITY), 0);
        assert_eq!(to_int32(2.9), 2);
        assert_eq!(to_int32(-2.9), -2);
        assert_eq!(to_int32(4_294_967_298.0), 2);
        assert_eq!(to_int32(-4_294_967_294.0), 2);
        assert_eq!(to_int32(2_147_483_648.0), i32::MIN);
    }

    #[test]
    fn int32_bit_selection_matches_modulo_for_binary64_exponents_and_payloads() {
        // Independent arithmetic oracle: the previous ECMAScript kernel.
        fn modulo_reference(value: f64) -> i32 {
            if !value.is_finite() || value == 0.0 {
                return 0;
            }
            let modulo = value.trunc() % 4_294_967_296.0;
            let unsigned = if modulo < 0.0 {
                modulo + 4_294_967_296.0
            } else {
                modulo
            };
            if unsigned >= 2_147_483_648.0 {
                (unsigned - 4_294_967_296.0) as i32
            } else {
                unsigned as i32
            }
        }

        let check = |bits| {
            let value = f64::from_bits(bits);
            assert_eq!(to_int32(value), modulo_reference(value), "{bits:#018x}");
        };
        // Every exponent, both signs, and significands around integer,
        // fractional and wrap boundaries, including noncanonical NaNs.
        for exponent in 0..2048_u64 {
            for fraction in [
                0,
                1,
                (1 << 20) - 1,
                1 << 20,
                (1 << 51) - 1,
                1 << 51,
                (1 << 52) - 1,
            ] {
                for sign in [0, 1_u64 << 63] {
                    check(sign | (exponent << 52) | fraction);
                }
            }
        }
        let mut bits = 0x6a09_e667_f3bc_c909_u64;
        for _ in 0..100_000 {
            bits ^= bits << 13;
            bits ^= bits >> 7;
            bits ^= bits << 17;
            check(bits);
        }
    }
}
