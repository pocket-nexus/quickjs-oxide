//! `Math` (phase 4, section 6.1 row 5): every function and constant. The
//! transcendental functions call the platform libm through Rust's `f64`
//! methods, as the current core does (on the reference machine both use
//! glibc). `round` is QuickJS's half-up on the bits, `random` its
//! xorshift64*, `sumPrecise` an exact (Shewchuk) summation.

use super::{Native, define, methods, plain_object, to_string_tag};
use crate::heap::HeapError;
use crate::runtime::Intrinsic;
use crate::shape::PropertyFlags;
use crate::value::Value;
use crate::vm::native::CallSite;
use crate::vm::number::number;
use crate::vm::{Cx, Throw, error, iter, ops};

pub(super) fn install(cx: &mut Cx<'_>) -> Result<(), HeapError> {
    let prototype = cx.realm.intrinsic(Intrinsic::ObjectPrototype);
    let math = plain_object(cx, Some(prototype), 48)?;
    let constant = PropertyFlags::data(false, false, false);
    for (name, value) in [
        ("E", std::f64::consts::E),
        ("LN10", std::f64::consts::LN_10),
        ("LN2", std::f64::consts::LN_2),
        ("LOG10E", std::f64::consts::LOG10_E),
        ("LOG2E", std::f64::consts::LOG2_E),
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
            Native::MathAbs,
            Native::MathAcos,
            Native::MathAcosh,
            Native::MathAsin,
            Native::MathAsinh,
            Native::MathAtan,
            Native::MathAtanh,
            Native::MathAtan2,
            Native::MathCbrt,
            Native::MathCeil,
            Native::MathClz32,
            Native::MathCos,
            Native::MathCosh,
            Native::MathExp,
            Native::MathExpm1,
            Native::MathFloor,
            Native::MathFround,
            Native::MathF16round,
            Native::MathHypot,
            Native::MathImul,
            Native::MathLog,
            Native::MathLog1p,
            Native::MathLog10,
            Native::MathLog2,
            Native::MathMax,
            Native::MathMin,
            Native::MathPow,
            Native::MathRandom,
            Native::MathRound,
            Native::MathSign,
            Native::MathSin,
            Native::MathSinh,
            Native::MathSqrt,
            Native::MathSumPrecise,
            Native::MathTan,
            Native::MathTanh,
            Native::MathTrunc,
        ],
    )?;
    to_string_tag(cx, math, "Math")?;
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
                let op: fn(f64) -> f64 = $op;
                if let Some(int) = site.arg(cx, 0).as_int32() {
                    return Ok(number(op(f64::from(int))));
                }
                let value = arg_number(cx, &site, 0)?;
                Ok(number(op(value)))
            }
        )*
    };
}

unary! {
    abs => f64::abs;
    acos => f64::acos;
    acosh => f64::acosh;
    asin => f64::asin;
    asinh => f64::asinh;
    atan => f64::atan;
    atanh => f64::atanh;
    cbrt => f64::cbrt;
    ceil => f64::ceil;
    cos => f64::cos;
    cosh => f64::cosh;
    exp => f64::exp;
    expm1 => f64::exp_m1;
    floor => f64::floor;
    fround => |value| f64::from(value as f32);
    f16round => |value| crate::vm::number::from_float16_bits(crate::vm::number::to_float16_bits(value));
    log => f64::ln;
    log1p => f64::ln_1p;
    log10 => f64::log10;
    log2 => f64::log2;
    round => round_half_up;
    sign => sign_of;
    sin => f64::sin;
    sinh => f64::sinh;
    sqrt => f64::sqrt;
    tan => f64::tan;
    tanh => f64::tanh;
    trunc => f64::trunc;
}

fn sign_of(value: f64) -> f64 {
    if value.is_nan() || value == 0.0 {
        value
    } else if value > 0.0 {
        1.0
    } else {
        -1.0
    }
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

pub(super) fn atan2(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let y = arg_number(cx, &site, 0)?;
    let x = arg_number(cx, &site, 1)?;
    Ok(number(y.atan2(x)))
}

pub(super) fn clz32(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let value = arg_number(cx, &site, 0)?;
    Ok(Value::int32(
        crate::vm::number::to_uint32(value).leading_zeros() as i32,
    ))
}

pub(super) fn imul(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let a = arg_number(cx, &site, 0)?;
    let b = arg_number(cx, &site, 1)?;
    let product = crate::vm::number::to_int32(a).wrapping_mul(crate::vm::number::to_int32(b));
    Ok(Value::int32(product))
}

/// `Math.hypot`: every argument converted first; an infinity wins over NaN.
pub(super) fn hypot(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let mut values = Vec::with_capacity(site.argc);
    for index in 0..site.argc {
        values.push(arg_number(cx, &site, index)?);
    }
    if values.iter().any(|value| value.is_infinite()) {
        return Ok(Value::number(f64::INFINITY));
    }
    if values.iter().any(|value| value.is_nan()) {
        return Ok(Value::number(f64::NAN));
    }
    let largest = values
        .iter()
        .fold(0.0f64, |max, value| max.max(value.abs()));
    if largest == 0.0 {
        return Ok(Value::int32(0));
    }
    // Scaled with compensation, as QuickJS does through `hypot`.
    let mut sum = 0.0;
    let mut compensation = 0.0;
    for value in &values {
        let scaled = value / largest;
        let term = scaled * scaled - compensation;
        let total = sum + term;
        compensation = (total - sum) - term;
        sum = total;
    }
    Ok(number(sum.sqrt() * largest))
}

/// `Math.min` and `Math.max`: every argument converted, `NaN` wins, and
/// `-0` orders below `+0`.
fn extremum(cx: &mut Cx<'_>, site: CallSite, max: bool) -> Result<Value, Throw> {
    let mut result = if max {
        f64::NEG_INFINITY
    } else {
        f64::INFINITY
    };
    for index in 0..site.argc {
        let value = site.arg(cx, index);
        let value = match value.as_int32() {
            Some(int) => f64::from(int),
            None => arg_number(cx, &site, index)?,
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

/// `Math.sumPrecise(items)`: the exact sum of the numbers an iterable
/// yields, rounded once. Every value must be a number.
pub(super) fn sum_precise(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let items = site.arg(cx, 0);
    if items.is_undefined() || items.is_null() {
        return Err(error::type_error(cx, "not an object"));
    }
    let mut record = iter::get_iterator(cx, items)?;
    let mut values = Vec::new();
    while let Some(value) = iter::step(cx, &mut record)? {
        let Some(value) = value.as_f64() else {
            let error = error::type_error(cx, "Math.sumPrecise: not a number");
            iter::close(cx, &record, Err(error))?;
            unreachable!("close rethrows");
        };
        values.push(value);
        if values.len() > (1 << 53) {
            let error = error::range_error(cx, "too many values");
            iter::close(cx, &record, Err(error))?;
        }
    }
    Ok(number(exact_sum(&values)))
}

/// The correctly rounded sum (the proposal's specification: -0 for no
/// values or only -0s, infinities and NaN by their rules, Shewchuk's
/// partials otherwise with the overflow handled by a 2^1024 offset).
fn exact_sum(values: &[f64]) -> f64 {
    let mut positive_infinity = false;
    let mut negative_infinity = false;
    let mut all_negative_zero = true;
    for &value in values {
        if value.is_nan() {
            return f64::NAN;
        }
        if value == f64::INFINITY {
            positive_infinity = true;
        } else if value == f64::NEG_INFINITY {
            negative_infinity = true;
        }
        if !(value == 0.0 && value.is_sign_negative()) {
            all_negative_zero = false;
        }
    }
    if positive_infinity && negative_infinity {
        return f64::NAN;
    }
    if positive_infinity {
        return f64::INFINITY;
    }
    if negative_infinity {
        return f64::NEG_INFINITY;
    }
    if all_negative_zero {
        return -0.0;
    }
    // Shewchuk's algorithm, overflowing partials counted in units of
    // 2^1024 (the proposal's reference implementation).
    // 2^1023 and 2^971, the unit in the last place of `f64::MAX`.
    const TWO_1023: f64 = f64::from_bits(0x7FE0_0000_0000_0000);
    const MAX_ULP: f64 = f64::from_bits(0x7CA0_0000_0000_0000);
    let mut partials: Vec<f64> = Vec::new();
    let mut overflow = 0i64;
    for &value in values {
        let mut x = value;
        let mut used = 0;
        for index in 0..partials.len() {
            let mut y = partials[index];
            if x.abs() < y.abs() {
                std::mem::swap(&mut x, &mut y);
            }
            let (mut high, mut low) = two_sum(x, y);
            if high.is_infinite() {
                let sign = if high > 0.0 { 1.0 } else { -1.0 };
                overflow += sign as i64;
                x = (x - sign * TWO_1023) - sign * TWO_1023;
                if x.abs() < y.abs() {
                    std::mem::swap(&mut x, &mut y);
                }
                (high, low) = two_sum(x, y);
            }
            if low != 0.0 {
                partials[used] = low;
                used += 1;
            }
            x = high;
        }
        partials.truncate(used);
        if x != 0.0 {
            partials.push(x);
        }
    }
    let mut n = partials.len() as isize - 1;
    let mut high = 0.0;
    let mut low = 0.0;
    if overflow != 0 {
        let next = if n >= 0 { partials[n as usize] } else { 0.0 };
        n -= 1;
        if overflow.abs() > 1 || (overflow > 0 && next > 0.0) || (overflow < 0 && next < 0.0) {
            return if overflow > 0 {
                f64::INFINITY
            } else {
                f64::NEG_INFINITY
            };
        }
        (high, low) = two_sum(overflow as f64 * TWO_1023, next / 2.0);
        low *= 2.0;
        if (2.0 * high).is_infinite() {
            if high > 0.0 {
                if high == TWO_1023
                    && low == -(MAX_ULP / 2.0)
                    && n >= 0
                    && partials[n as usize] < 0.0
                {
                    return f64::MAX;
                }
                return f64::INFINITY;
            }
            if high == -TWO_1023 && low == MAX_ULP / 2.0 && n >= 0 && partials[n as usize] > 0.0 {
                return -f64::MAX;
            }
            return f64::NEG_INFINITY;
        }
        if low != 0.0 {
            let at = (n + 1) as usize;
            if at < partials.len() {
                partials[at] = low;
            } else {
                partials.push(low);
            }
            n += 1;
            low = 0.0;
        }
        high *= 2.0;
    }
    while n >= 0 {
        let x = high;
        let y = partials[n as usize];
        n -= 1;
        (high, low) = two_sum(x, y);
        if low != 0.0 {
            break;
        }
    }
    if n >= 0
        && ((low < 0.0 && partials[n as usize] < 0.0) || (low > 0.0 && partials[n as usize] > 0.0))
    {
        let y = low * 2.0;
        let x = high + y;
        let rounded = x - high;
        if y == rounded {
            high = x;
        }
    }
    high
}

/// `(a + b, the rounding error)`.
fn two_sum(a: f64, b: f64) -> (f64, f64) {
    let high = a + b;
    let low = b - (high - a);
    (high, low)
}
