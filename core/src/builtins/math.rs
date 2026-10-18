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
    // QuickJS's `js_math_funcs`, in order.
    methods(
        cx,
        math,
        &[
            Native::MathMin,
            Native::MathMax,
            Native::MathAbs,
            Native::MathFloor,
            Native::MathCeil,
            Native::MathRound,
            Native::MathSqrt,
            Native::MathAcos,
            Native::MathAsin,
            Native::MathAtan,
            Native::MathAtan2,
            Native::MathCos,
            Native::MathExp,
            Native::MathLog,
            Native::MathPow,
            Native::MathSin,
            Native::MathTan,
            Native::MathTrunc,
            Native::MathSign,
            Native::MathCosh,
            Native::MathSinh,
            Native::MathTanh,
            Native::MathAcosh,
            Native::MathAsinh,
            Native::MathAtanh,
            Native::MathExpm1,
            Native::MathLog1p,
            Native::MathLog2,
            Native::MathLog10,
            Native::MathCbrt,
            Native::MathHypot,
            Native::MathRandom,
            Native::MathF16round,
            Native::MathFround,
            Native::MathImul,
            Native::MathClz32,
            Native::MathSumPrecise,
        ],
    )?;
    let constant = PropertyFlags::data(false, false, false);
    for (name, value) in [
        ("E", std::f64::consts::E),
        ("LN10", std::f64::consts::LN_10),
        ("LN2", std::f64::consts::LN_2),
        ("LOG2E", std::f64::consts::LOG2_E),
        ("LOG10E", std::f64::consts::LOG10_E),
        ("PI", std::f64::consts::PI),
        ("SQRT1_2", std::f64::consts::FRAC_1_SQRT_2),
        ("SQRT2", std::f64::consts::SQRT_2),
    ] {
        define(cx, math, name, Value::number(value), constant)?;
    }
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
    acosh => fdlibm_acosh;
    asin => f64::asin;
    asinh => f64::asinh;
    atan => f64::atan;
    atanh => fdlibm_atanh;
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

/// fdlibm's `atanh`, as the current engine computes it: `f64::atanh`'s
/// single expression loses thousands of ULPs next to -1.
fn fdlibm_atanh(value: f64) -> f64 {
    let magnitude = value.abs();
    // Tiny inputs are their own correctly rounded result (and keep -0).
    if magnitude < f64::from_bits(0x3e30_0000_0000_0000) {
        return value;
    }
    if magnitude > 1.0 || value.is_nan() {
        return f64::NAN;
    }
    if magnitude == 1.0 {
        return f64::INFINITY.copysign(value);
    }
    let result = if magnitude < 0.5 {
        let twice = magnitude + magnitude;
        0.5 * (twice + twice * magnitude / (1.0 - magnitude)).ln_1p()
    } else {
        0.5 * ((magnitude + magnitude) / (1.0 - magnitude)).ln_1p()
    };
    result.copysign(value)
}

/// fdlibm's `acosh`: `ln(x + sqrt(x² - 1))` loses dozens of ULPs just above
/// one.
fn fdlibm_acosh(value: f64) -> f64 {
    if value.is_nan() || value < 1.0 {
        return f64::NAN;
    }
    if value == 1.0 {
        return 0.0;
    }
    if value.is_infinite() {
        return value;
    }
    if value < 2.0 {
        let delta = value - 1.0;
        return (delta + (delta * delta + 2.0 * delta).sqrt()).ln_1p();
    }
    if value < 268_435_456.0 {
        return (2.0 * value - 1.0 / (value + (value * value - 1.0).sqrt())).ln();
    }
    value.ln() + std::f64::consts::LN_2
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
        return Err(error::not_iterable_nullish(cx, items));
    }
    let mut record = iter::get_iterator(cx, items)?;
    let mut values = Vec::new();
    while let Some(value) = iter::step(cx, &mut record)? {
        if values.len() as u64 + 1 >= (1u64 << 53) {
            let error = error::range_error(cx, "too many values");
            iter::close(cx, &record, Err(error))?;
        }
        let Some(value) = value.as_f64() else {
            let error = error::type_error(cx, "not a number");
            iter::close(cx, &record, Err(error))?;
            unreachable!("close rethrows");
        };
        values.push(value);
    }
    let mut sum = SumPrecise::new();
    for value in values {
        sum.add(value);
    }
    Ok(number(sum.result()))
}

/// The exact sum of `Math.sumPrecise` (ES2026): a fixed-point accumulator
/// in 56-bit limbs (as QuickJS's `SumPreciseState`), renormalized before a
/// limb can leave the `i64` range. Each addition adds less than 2^56 to a
/// limb, so 127 additions after a renormalization stay below 2^63 (QuickJS
/// waits 250, and its limbs wrap).
struct SumPrecise {
    state: SumState,
    counter: u32,
    limbs: usize,
    acc: [i64; SUM_ACC_LEN],
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SumState {
    Finite,
    Infinity,
    MinusInfinity,
    NaN,
}

const SP_LIMB_BITS: u32 = 56;
const SP_RND_BITS: u32 = SP_LIMB_BITS - 53;
const SP_LIMB_MASK: i64 = (1 << SP_LIMB_BITS) - 1;
const SUM_ACC_LEN: usize = 39;
const SUM_COUNTER_INIT: u32 = 127;

impl SumPrecise {
    fn new() -> SumPrecise {
        SumPrecise {
            state: SumState::Finite,
            counter: SUM_COUNTER_INIT,
            limbs: 0,
            acc: [0; SUM_ACC_LEN],
        }
    }

    fn renorm(&mut self) {
        let mut carry = 0i64;
        for index in 0..self.limbs {
            let value = self.acc[index].wrapping_add(carry);
            self.acc[index] = value & SP_LIMB_MASK;
            carry = value >> SP_LIMB_BITS;
        }
        if carry != 0 && self.limbs < SUM_ACC_LEN {
            self.acc[self.limbs] = carry;
            self.limbs += 1;
        }
    }

    fn add(&mut self, value: f64) {
        let bits = value.to_bits();
        let negative = bits >> 63 == 1;
        let exponent = ((bits >> 52) & 0x7ff) as u32;
        let mut mantissa = bits & ((1 << 52) - 1);
        let (position, shift);
        if exponent == 2047 {
            self.state = if mantissa != 0
                || self.state == SumState::NaN
                || (self.state == SumState::MinusInfinity && !negative)
                || (self.state == SumState::Infinity && negative)
            {
                SumState::NaN
            } else if negative {
                SumState::MinusInfinity
            } else {
                SumState::Infinity
            };
            return;
        } else if exponent == 0 {
            if mantissa == 0 {
                if self.limbs == 0 && !negative {
                    self.limbs = 1;
                }
                return;
            }
            position = 0;
            shift = 0;
        } else {
            mantissa |= 1 << 52;
            position = ((exponent - 1) / SP_LIMB_BITS) as usize;
            shift = (exponent - 1) % SP_LIMB_BITS;
        }
        let low = ((mantissa << shift) as i64) & SP_LIMB_MASK;
        let high = (mantissa >> (SP_LIMB_BITS - shift)) as i64;
        if negative {
            self.acc[position] = self.acc[position].wrapping_sub(low);
            self.acc[position + 1] = self.acc[position + 1].wrapping_sub(high);
        } else {
            self.acc[position] = self.acc[position].wrapping_add(low);
            self.acc[position + 1] = self.acc[position + 1].wrapping_add(high);
        }
        self.limbs = self.limbs.max(position + 2);
        self.counter -= 1;
        if self.counter == 0 {
            self.counter = SUM_COUNTER_INIT;
            self.renorm();
        }
    }

    fn result(&mut self) -> f64 {
        match self.state {
            SumState::Finite => {}
            SumState::Infinity => return f64::INFINITY,
            SumState::MinusInfinity => return f64::NEG_INFINITY,
            SumState::NaN => return f64::NAN,
        }
        self.renorm();
        let mut n = self.limbs;
        if n == 0 {
            return -0.0;
        }
        while n > 0 && self.acc[n - 1] == 0 {
            n -= 1;
        }
        if n == 0 {
            return 0.0;
        }
        let negative = self.acc[n - 1] < 0;
        if negative {
            let mut carry = 1u64;
            for index in 0..n - 1 {
                let value = (SP_LIMB_MASK as u64)
                    .wrapping_sub(self.acc[index] as u64)
                    .wrapping_add(carry);
                carry = value >> SP_LIMB_BITS;
                self.acc[index] = (value & SP_LIMB_MASK as u64) as i64;
            }
            self.acc[n - 1] = self.acc[n - 1]
                .wrapping_neg()
                .wrapping_add(carry as i64)
                .wrapping_sub(1);
            while n > 1 && self.acc[n - 1] == 0 {
                n -= 1;
            }
        }
        let sign = u64::from(negative) << 63;
        if n == 1 && (self.acc[0] as u64) < (1 << 52) {
            return f64::from_bits(sign | self.acc[0] as u64);
        }
        let mut p = n - 1;
        let mut m = self.acc[p] as u64;
        let shift = m.leading_zeros() as i64 - (64 - i64::from(SP_LIMB_BITS));
        let mut e = (n as i64) * i64::from(SP_LIMB_BITS) - shift - 52;
        if shift != 0 {
            m = m.wrapping_shl(shift as u32);
            if p > 0 {
                p -= 1;
                let shift1 = SP_LIMB_BITS as i64 - shift;
                let below = self.acc[p] as u64;
                let nonzero = below & ((1u64 << shift1) - 1);
                m = m | (below >> shift1) | u64::from(nonzero != 0);
            }
        }
        if m & ((1 << SP_RND_BITS) - 1) == 1 << (SP_RND_BITS - 1) {
            while p > 0 {
                p -= 1;
                if self.acc[p] != 0 {
                    m |= 1;
                    break;
                }
            }
        }
        let addend = (1u64 << (SP_RND_BITS - 1)) - 1 + ((m >> SP_RND_BITS) & 1);
        m = m.wrapping_add(addend) >> SP_RND_BITS;
        if m == 1 << 53 {
            e += 1;
        }
        if e >= 2047 {
            f64::from_bits(sign | (2047 << 52))
        } else {
            f64::from_bits(sign | ((e as u64) << 52) | (m & ((1 << 52) - 1)))
        }
    }
}
