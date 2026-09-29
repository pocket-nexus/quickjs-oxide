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
    fn int32_preserves_integer_payload_and_float_conversion() {
        for value in [i32::MIN, i32::MIN + 1, -1, 0, 1, i32::MAX - 1, i32::MAX] {
            assert_eq!(Number::Int(value).int32(), value);
            assert_eq!(Number::Float(f64::from(value)).int32(), value);
        }
        for (input, expected) in [
            (f64::NAN, 0),
            (f64::INFINITY, 0),
            (f64::NEG_INFINITY, 0),
            (-0.0, 0),
            (2.9, 2),
            (-2.9, -2),
            (2_147_483_648.0, i32::MIN),
            (4_294_967_295.0, -1),
            (4_294_967_298.0, 2),
            (-4_294_967_298.0, -2),
            (f64::MAX, 0),
        ] {
            assert_eq!(Number::Float(input).int32(), expected, "{input:?}");
        }
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
        // An Int payload has already satisfied ToInt32. Preserve that fact
        // instead of converting to f64 and reducing it modulo 2^32 again.
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
