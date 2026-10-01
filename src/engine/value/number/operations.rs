//! Pure ECMAScript Number arithmetic shared by bytecode and builtins.

/// Pinned QuickJS `js_pow` kernel shared by the `**` bytecode and
/// `Math.pow`.  C's `pow` result for a unit-magnitude base and an infinite
/// exponent is not the ECMAScript result, so QuickJS handles it explicitly.
#[must_use]
pub fn pow(base: f64, exponent: f64) -> f64 {
    if !exponent.is_finite() && base.abs() == 1.0 {
        f64::NAN
    } else {
        base.powf(exponent)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pow_matches_quickjs_unit_base_infinity_rule() {
        assert!(pow(1.0, f64::INFINITY).is_nan());
        assert!(pow(-1.0, f64::NEG_INFINITY).is_nan());
        assert_eq!(pow(2.0, 10.0), 1024.0);
    }

    #[test]
    fn final_only_product_compaction_matches_two_number_operations() {
        let values = [
            0.0,
            -0.0,
            1.0,
            -1.0,
            2.0,
            f64::from(i32::MAX),
            f64::from(i32::MIN),
            2_147_483_648.0,
            f64::MIN_POSITIVE,
            f64::from_bits(1),
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NAN,
        ];
        for old in values {
            for element in values {
                for scale in values {
                    let old = Number::compact(old);
                    let element = Number::compact(element);
                    let scale = Number::compact(scale);
                    let generic = old.add(element.mul(scale));
                    let candidate =
                        Number::compact(old.float() + (element.float() * scale.float()));
                    assert_eq!(generic.float().to_bits(), candidate.float().to_bits());
                }
            }
        }
    }

    fn assert_product_matches_binary64(left: Number, right: Number) {
        let expected = left.float() * right.float();
        let actual = left.mul(right).float();
        if expected.is_nan() {
            assert!(actual.is_nan());
        } else {
            assert_eq!(actual.to_bits(), expected.to_bits(), "{left:?} * {right:?}");
        }
    }

    #[test]
    fn integer_multiply_preserves_boundaries_and_signed_zero() {
        let values = [
            i32::MIN,
            i32::MIN + 1,
            -46_341,
            -46_340,
            -16_384,
            -1,
            0,
            1,
            16_383,
            16_384,
            46_340,
            46_341,
            i32::MAX - 1,
            i32::MAX,
        ];
        for left in values {
            for right in values {
                assert_product_matches_binary64(Number::Int(left), Number::Int(right));
            }
        }
        assert!(matches!(
            Number::Int(46_340).mul(Number::Int(46_340)),
            Number::Int(2_147_395_600)
        ));
        assert!(matches!(
            Number::Int(46_341).mul(Number::Int(46_341)),
            Number::Float(_)
        ));
        assert!(matches!(
            Number::Int(i32::MIN).mul(Number::Int(1)),
            Number::Int(i32::MIN)
        ));
        assert!(matches!(
            Number::Int(i32::MIN).mul(Number::Int(-1)),
            Number::Float(2_147_483_648.0)
        ));
        for (left, right) in [(0, -1), (-1, 0), (0, i32::MIN), (i32::MIN, 0)] {
            assert!(
                matches!(Number::Int(left).mul(Number::Int(right)), Number::Float(value) if value.to_bits() == (-0.0f64).to_bits())
            );
        }

        // Exercise wide overflow products and small integer products with an
        // independent binary64 oracle, including values above exact f64 range.
        let mut state = 0x6d2b_79f5u32;
        for _ in 0..32_768 {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let left = state as i32;
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let right = state as i32;
            assert_product_matches_binary64(Number::Int(left), Number::Int(right));
            assert_product_matches_binary64(Number::Int(left >> 18), Number::Int(right >> 18));
        }
    }

    #[test]
    fn integer_multiply_keeps_mixed_and_float_semantics() {
        let values = [
            Number::Int(0),
            Number::Int(-1),
            Number::Int(i32::MIN),
            Number::Float(-0.0),
            Number::Float(0.5),
            Number::Float(-0.5),
            Number::Float(f64::from_bits(1)),
            Number::Float(f64::MAX),
            Number::Float(f64::INFINITY),
            Number::Float(f64::NEG_INFINITY),
            Number::Float(f64::NAN),
        ];
        for left in values {
            for right in values {
                assert_product_matches_binary64(left, right);
            }
        }
    }

    #[cfg(feature = "profiling")]
    #[test]
    fn integer_multiply_records_actual_kernel_admission() {
        let profile = crate::engine::api::profiling::CostProfile::start();
        let _ = Number::Int(3).mul(Number::Int(7));
        let _ = Number::Int(0).mul(Number::Int(-7));
        let _ = Number::Int(i32::MAX).mul(Number::Int(2));
        let _ = Number::Float(0.5).mul(Number::Int(2));
        let events = profile.snapshot().owned_execution_events;
        for event in [
            "number_mul.int_result",
            "number_mul.int_negative_zero",
            "number_mul.int_overflow",
            "number_mul.float_operands",
        ] {
            assert_eq!(events.get(event).copied(), Some(1));
        }
    }
}

/// Numeric representation only: no Value, conversion, Runtime or operand stack.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Number {
    Int(i32),
    Float(f64),
}

impl Number {
    pub(crate) fn compact(value: f64) -> Self {
        if value == f64::from(value as i32) && !(value == 0.0 && value.is_sign_negative()) {
            Self::Int(value as i32)
        } else {
            Self::Float(value)
        }
    }

    pub(crate) fn float(self) -> f64 {
        match self {
            Self::Int(value) => f64::from(value),
            Self::Float(value) => value,
        }
    }

    pub(crate) fn add(self, rhs: Self) -> Self {
        if let (Self::Int(a), Self::Int(b)) = (self, rhs) {
            if let Some(value) = a.checked_add(b) {
                return Self::Int(value);
            }
        }
        Self::compact(self.float() + rhs.float())
    }

    pub(crate) fn sub(self, rhs: Self) -> Self {
        if let (Self::Int(a), Self::Int(b)) = (self, rhs) {
            if let Some(value) = a.checked_sub(b) {
                return Self::Int(value);
            }
        }
        Self::compact(self.float() - rhs.float())
    }

    pub(crate) fn mul(self, rhs: Self) -> Self {
        if let (Self::Int(a), Self::Int(b)) = (self, rhs) {
            if let Some(value) = a.checked_mul(b) {
                // Integer zero has no sign; a negative operand still makes
                // zero multiplication produce the observable Number -0.
                if value == 0 && (a | b) < 0 {
                    #[cfg(feature = "profiling")]
                    crate::engine::api::profiling::record_owned_execution_event(
                        "number_mul.int_negative_zero",
                    );
                    return Self::Float(-0.0);
                }
                #[cfg(feature = "profiling")]
                crate::engine::api::profiling::record_owned_execution_event(
                    "number_mul.int_result",
                );
                return Self::Int(value);
            }
            #[cfg(feature = "profiling")]
            crate::engine::api::profiling::record_owned_execution_event("number_mul.int_overflow");
        } else {
            #[cfg(feature = "profiling")]
            crate::engine::api::profiling::record_owned_execution_event(
                "number_mul.float_operands",
            );
        }
        Self::compact(self.float() * rhs.float())
    }

    pub(crate) fn div(self, rhs: Self) -> Self {
        Self::compact(self.float() / rhs.float())
    }

    pub(crate) fn rem(self, rhs: Self) -> Self {
        Self::compact(self.float() % rhs.float())
    }

    pub(crate) fn pow(self, rhs: Self) -> Self {
        Self::compact(pow(self.float(), rhs.float()))
    }

    pub(crate) fn int32(self) -> i32 {
        match self {
            Self::Int(value) => value,
            Self::Float(value) => super::integer::to_int32(value),
        }
    }
    pub(crate) fn negate(self) -> Self {
        match self {
            Self::Float(value) => Self::Float(-value),
            Self::Int(value) => Self::compact(-f64::from(value)),
        }
    }
    pub(crate) fn update(self, increment: bool) -> Self {
        match self {
            Self::Int(value) => {
                let next = if increment {
                    value.checked_add(1)
                } else {
                    value.checked_sub(1)
                };
                next.map_or_else(
                    || Self::Float(f64::from(value) + if increment { 1.0 } else { -1.0 }),
                    Self::Int,
                )
            }
            Self::Float(value) => Self::Float(if increment { value + 1.0 } else { value - 1.0 }),
        }
    }
}
