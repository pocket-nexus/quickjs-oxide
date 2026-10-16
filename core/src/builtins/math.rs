//! `Math`: the functions and constants the V8 v7 programs use. The
//! transcendental functions call the platform libm through Rust's `f64`
//! methods, as the current core does (section 11: on the reference machine
//! both use glibc). `Math.random` is QuickJS's xorshift64*.

use super::{Native, define, methods};
use crate::heap::HeapError;
use crate::shape::PropertyFlags;
use crate::value::Value;
use crate::vm::native::CallSite;
use crate::vm::number::number;
use crate::vm::{Cx, Throw, ops};

pub(super) fn install(cx: &mut Cx<'_>) -> Result<(), HeapError> {
    let math = cx.heap.alloc_object(
        crate::ObjectKind::Ordinary,
        Some(cx.realm.intrinsic(crate::Intrinsic::ObjectPrototype)),
        32,
    )?;
    cx.heap.roots().push(Value::object(math));
    let constant = PropertyFlags::data(false, false, false);
    for (name, value) in [
        ("E", std::f64::consts::E),
        ("LN2", std::f64::consts::LN_2),
        ("LN10", std::f64::consts::LN_10),
        ("LOG2E", std::f64::consts::LOG2_E),
        ("LOG10E", std::f64::consts::LOG10_E),
        ("PI", std::f64::consts::PI),
        ("SQRT1_2", std::f64::consts::FRAC_1_SQRT_2),
        ("SQRT2", std::f64::consts::SQRT_2),
    ] {
        define(cx, math, name, Value::number(value), constant)?;
    }
    methods(
        cx,
        math,
        &[
            ("abs", Native::MathAbs),
            ("floor", Native::MathFloor),
            ("ceil", Native::MathCeil),
            ("round", Native::MathRound),
            ("min", Native::MathMin),
            ("max", Native::MathMax),
            ("pow", Native::MathPow),
            ("sqrt", Native::MathSqrt),
            ("log", Native::MathLog),
            ("exp", Native::MathExp),
            ("sin", Native::MathSin),
            ("cos", Native::MathCos),
            ("tan", Native::MathTan),
            ("asin", Native::MathAsin),
            ("acos", Native::MathAcos),
            ("atan", Native::MathAtan),
            ("random", Native::MathRandom),
        ],
    )?;
    let global = cx.realm.global();
    define(
        cx,
        global,
        "Math",
        Value::object(math),
        PropertyFlags::data(true, false, true),
    )
}

/// `ToNumber` of argument `index`, with the `Int32` case inline.
fn arg_number(cx: &mut Cx<'_>, site: &CallSite, index: usize) -> Result<f64, Throw> {
    let value = site.arg(cx, index);
    match value.as_f64() {
        Some(value) => Ok(value),
        None => ops::to_number(cx, value),
    }
}

macro_rules! unary {
    ($($name:ident => $op:expr;)*) => {
        $(
            pub(super) fn $name(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
                if let Some(int) = site.arg(cx, 0).as_int32() {
                    let op: fn(f64) -> f64 = $op;
                    return Ok(number(op(f64::from(int))));
                }
                let value = arg_number(cx, &site, 0)?;
                let op: fn(f64) -> f64 = $op;
                Ok(number(op(value)))
            }
        )*
    };
}

unary! {
    abs => f64::abs;
    floor => f64::floor;
    ceil => f64::ceil;
    round => round_half_up;
    sqrt => f64::sqrt;
    log => f64::ln;
    exp => f64::exp;
    sin => f64::sin;
    cos => f64::cos;
    tan => f64::tan;
    asin => f64::asin;
    acos => f64::acos;
    atan => f64::atan;
}

/// QuickJS's `js_math_round`: halves round toward +∞, on the bits.
fn round_half_up(value: f64) -> f64 {
    let mut bits = value.to_bits();
    let exponent = ((bits >> 52) & 0x7ff) as u32;
    if exponent < 1023 {
        // |value| < 1: ±1 above one half (and at +0.5), ±0 below.
        if exponent == 1022 && bits != 0xbfe0_0000_0000_0000 {
            bits = (bits & (1 << 63)) | (1023 << 52);
        } else {
            bits &= 1 << 63;
        }
    } else if exponent < 1023 + 52 {
        let sign = bits >> 63;
        let one = 1u64 << (52 - (exponent - 1023));
        let fraction = one - 1;
        bits = bits.wrapping_add((one >> 1).wrapping_sub(sign));
        bits &= !fraction;
    }
    f64::from_bits(bits)
}

/// `Math.min` and `Math.max`: every argument converted, `NaN` wins, and
/// `-0` orders below `+0`.
fn extremum(cx: &mut Cx<'_>, site: CallSite, max: bool) -> Result<Value, Throw> {
    let mut result = if max {
        f64::NEG_INFINITY
    } else {
        f64::INFINITY
    };
    let mut ints = true;
    for index in 0..site.argc {
        let value = site.arg(cx, index);
        let value = match value.as_int32() {
            Some(int) => f64::from(int),
            None => {
                ints = false;
                arg_number(cx, &site, index)?
            }
        };
        if result.is_nan() {
            continue;
        }
        if value.is_nan() {
            result = value;
        } else if max {
            if value > result || (value == 0.0 && result == 0.0 && result.is_sign_negative()) {
                result = value;
            }
        } else if value < result || (value == 0.0 && result == 0.0 && value.is_sign_negative()) {
            result = value;
        }
    }
    let _ = ints;
    Ok(number(result))
}

pub(super) fn min(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    extremum(cx, site, false)
}

pub(super) fn max(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    extremum(cx, site, true)
}

pub(super) fn pow(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let base = arg_number(cx, &site, 0)?;
    let exponent = arg_number(cx, &site, 1)?;
    Ok(number(crate::vm::number::pow(base, exponent)))
}

pub(super) fn random(cx: &mut Cx<'_>, _: CallSite) -> Result<Value, Throw> {
    let mut state = cx.host.random_state();
    state ^= state >> 12;
    state ^= state << 25;
    state ^= state >> 27;
    cx.host.set_random_state(state);
    let bits = state.wrapping_mul(0x2545_f491_4f6c_dd1d);
    // 52 random bits in the mantissa of a number in [1, 2), minus one.
    let value = f64::from_bits((1023 << 52) | (bits >> 12)) - 1.0;
    Ok(Value::number(value))
}
