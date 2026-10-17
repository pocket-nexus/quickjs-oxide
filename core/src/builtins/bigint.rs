//! `BigInt` (phase 4, section 6.1 row 5): the function (callable, not
//! constructible), `asIntN`, `asUintN`, and `BigInt.prototype`'s
//! `toString(radix)`, `toLocaleString` and `valueOf`.

use super::{Native, install_constructor, methods, string_text, to_string_tag};
use crate::heap::HeapError;
use crate::heap::object::ObjectKind;
use crate::runtime::Intrinsic;
use crate::value::{Kind, Value};
use crate::vm::bigint::{Big, BigError};
use crate::vm::native::CallSite;
use crate::vm::object as internal;
use crate::vm::{Cx, Throw, error, ops};

pub(super) fn install(cx: &mut Cx<'_>) -> Result<(), HeapError> {
    let object_prototype = cx.realm.intrinsic(Intrinsic::ObjectPrototype);
    let prototype = super::plain_object(cx, Some(object_prototype), 4)?;
    cx.set_intrinsic(Intrinsic::BigIntPrototype, prototype);
    let bigint = install_constructor(cx, Native::BigIntConstructor, prototype)?;
    cx.set_intrinsic(Intrinsic::BigInt, bigint);
    methods(cx, bigint, &[Native::BigIntAsIntN, Native::BigIntAsUintN])?;
    methods(
        cx,
        prototype,
        &[
            Native::BigIntToLocaleString,
            Native::BigIntToString,
            Native::BigIntValueOf,
        ],
    )?;
    to_string_tag(cx, prototype, "BigInt")
}

fn alloc(cx: &mut Cx<'_>, value: Result<Big, BigError>) -> Result<Value, Throw> {
    match value {
        Ok(value) => cx
            .heap
            .alloc_bigint(value.negative, &value.limbs)
            .map(Value::bigint)
            .map_err(|error| error::heap_error(cx, error)),
        Err(_) => Err(error::range_error(cx, "BigInt is too large")),
    }
}

/// `ToBigInt(value)`.
pub(crate) fn to_bigint(cx: &mut Cx<'_>, value: Value) -> Result<Value, Throw> {
    convert(cx, value, false)
}

/// `ToBigInt(value)`; `constructor` selects the `BigInt(value)` messages
/// (QuickJS's `JS_ToBigIntCtorFree`) over `ToBigInt`'s.
fn convert(cx: &mut Cx<'_>, value: Value, constructor: bool) -> Result<Value, Throw> {
    let primitive = ops::to_primitive(cx, value, ops::Hint::Number)?;
    let failure = if constructor {
        "cannot convert to BigInt"
    } else {
        "cannot convert to bigint"
    };
    match primitive.kind() {
        Kind::BigInt => Ok(primitive),
        Kind::Bool => alloc(cx, Ok(Big::from_u64(u64::from(primitive == Value::TRUE)))),
        Kind::String => {
            let text = ops::string_text(cx.heap, primitive);
            match Big::parse(&text) {
                Some(value) => alloc(cx, Ok(value)),
                None => Err(error::throw(
                    cx,
                    crate::ErrorKind::Syntax,
                    "invalid bigint literal",
                )),
            }
        }
        Kind::Undefined | Kind::Null | Kind::Empty | Kind::Number | Kind::Int32 | Kind::Symbol => {
            Err(error::type_error(cx, failure))
        }
        Kind::Object => unreachable!("ToPrimitive returned an object"),
    }
}

/// `BigInt(value)`.
pub(super) fn constructor(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    if site.construct {
        return Err(error::type_error(cx, "BigInt is not a constructor"));
    }
    let value = site.arg(cx, 0);
    let primitive = ops::to_primitive(cx, value, ops::Hint::Number)?;
    if let Some(number) = primitive.as_f64() {
        if !number.is_finite() {
            return Err(error::range_error(
                cx,
                "cannot convert NaN or Infinity to BigInt",
            ));
        }
        if number.trunc() != number {
            return Err(error::range_error(
                cx,
                "cannot convert to BigInt: not an integer",
            ));
        }
        let big = Big::from_f64(number).expect("an integer");
        return alloc(cx, Ok(big));
    }
    convert(cx, primitive, true)
}

/// `thisBigIntValue`.
fn this_bigint(cx: &mut Cx<'_>, site: &CallSite) -> Result<Value, Throw> {
    let this = site.this(cx);
    if this.as_bigint().is_some() {
        return Ok(this);
    }
    if let Some(object) = this.as_object()
        && cx.heap.object_kind(object) == ObjectKind::BigIntWrapper
    {
        return Ok(cx.heap.internal_slot(object, 0));
    }
    Err(error::type_error(cx, "not a BigInt"))
}

pub(super) fn value_of(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    this_bigint(cx, &site)
}

pub(super) fn to_locale_string(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let value = this_bigint(cx, &site)?;
    ops::to_string(cx, value)
}

pub(super) fn to_string(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let value = this_bigint(cx, &site)?;
    let radix = site.arg(cx, 0);
    let radix = if radix.is_undefined() {
        10.0
    } else {
        internal::to_integer_or_infinity(cx, radix)?
    };
    if !(2.0..=36.0).contains(&radix) {
        return Err(error::range_error(cx, "radix must be between 2 and 36"));
    }
    let big = ops::big(cx.heap, value);
    string_text(cx, &radix_text(&big, radix as u32))
}

/// The digits of `value` in `radix`.
pub(crate) fn radix_text(value: &Big, radix: u32) -> String {
    if radix == 10 {
        return value.to_string_radix10();
    }
    if value.is_zero() {
        return "0".into();
    }
    const DIGITS: &[u8; 36] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let mut out = Vec::new();
    let mut rest = Big::new(false, value.limbs.clone());
    let divisor = Big::from_u64(u64::from(radix));
    while !rest.is_zero() {
        let (quotient, remainder) = rest.div_rem(&divisor).expect("a non-zero divisor");
        out.push(DIGITS[remainder.limbs.first().copied().unwrap_or(0) as usize]);
        rest = quotient;
    }
    if value.negative {
        out.push(b'-');
    }
    out.reverse();
    String::from_utf8(out).expect("ASCII digits")
}

/// `asIntN` and `asUintN`: `bigint mod 2^bits`, as a signed value for
/// `asIntN`.
fn as_n(cx: &mut Cx<'_>, site: CallSite, signed: bool) -> Result<Value, Throw> {
    let bits = site.arg(cx, 0);
    let bits = to_index(cx, bits)?;
    let value = site.arg(cx, 1);
    let value = to_bigint(cx, value)?;
    cx.heap.roots().push(value);
    if bits == 0 {
        return alloc(cx, Ok(Big::zero()));
    }
    let big = ops::big(cx.heap, value);
    // Values that fit need no reduction.
    let magnitude_bits = big.limbs.len() as u64 * 64
        - u64::from(big.limbs.last().map_or(64, |top| top.leading_zeros()));
    if !signed && !big.negative && magnitude_bits <= bits {
        return Ok(value);
    }
    if signed && magnitude_bits < bits {
        return Ok(value);
    }
    if bits > (1 << 30) {
        return Err(error::range_error(cx, "BigInt is too large"));
    }
    let modulus = Big::from_u64(1).shl(&Big::from_u64(bits));
    let Ok(modulus) = modulus else {
        return Err(error::range_error(cx, "BigInt is too large"));
    };
    let (_, mut remainder) = big.div_rem(&modulus).expect("a non-zero modulus");
    if remainder.negative {
        remainder = remainder.add(&modulus).expect("within the limit");
    }
    if signed {
        let half = Big::from_u64(1)
            .shl(&Big::from_u64(bits - 1))
            .expect("within the limit");
        if remainder.cmp(&half) != std::cmp::Ordering::Less {
            remainder = remainder.sub(&modulus).expect("within the limit");
        }
    }
    alloc(cx, Ok(remainder))
}

pub(super) fn as_int_n(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    as_n(cx, site, true)
}

pub(super) fn as_uint_n(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    as_n(cx, site, false)
}

/// `ToIndex(value)`.
pub(crate) fn to_index(cx: &mut Cx<'_>, value: Value) -> Result<u64, Throw> {
    if value.is_undefined() {
        return Ok(0);
    }
    let integer = internal::to_integer_or_infinity(cx, value)?;
    if !(0.0..=9_007_199_254_740_991.0).contains(&integer) {
        return Err(error::range_error(cx, "invalid array index"));
    }
    Ok(integer as u64)
}
