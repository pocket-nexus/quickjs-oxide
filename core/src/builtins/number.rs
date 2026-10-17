//! `Number` (phase 4, section 6.1 row 5): the constructor and wrapper, the
//! constants and `isFinite`, `isInteger`, `isNaN`, `isSafeInteger`,
//! `parseFloat` and `parseInt` (the global functions themselves), and
//! `Number.prototype`'s `toString(radix)`, `toFixed`, `toExponential`,
//! `toPrecision`, `toLocaleString` and `valueOf` with QuickJS's digits.

use super::{Native, define, install_constructor, link_constructor, methods, string_text};
use crate::heap::HeapError;
use crate::heap::object::ObjectKind;
use crate::runtime::Intrinsic;
use crate::shape::PropertyFlags;
use crate::value::Value;
use crate::vm::native::CallSite;
use crate::vm::object as internal;
use crate::vm::{Cx, Throw, error, number, ops, radix};

pub(super) fn install(cx: &mut Cx<'_>) -> Result<(), HeapError> {
    let prototype = cx.realm.intrinsic(Intrinsic::NumberPrototype);
    // QuickJS's `js_number_funcs` and `js_number_proto_funcs`, in order.
    let constructor = install_constructor(cx, Native::NumberConstructor)?;
    cx.set_intrinsic(Intrinsic::Number, constructor);
    // `Number.parseInt` and `Number.parseFloat` are the global functions.
    let global = cx.realm.global();
    for name in ["parseInt", "parseFloat"] {
        let key = cx.heap.property_key(name)?;
        let (slot, _) = cx.heap.lookup_own(global, key).expect("a global function");
        let function = cx.heap.get_slot(global, slot);
        define(cx, constructor, name, function, super::hidden())?;
    }
    methods(
        cx,
        constructor,
        &[
            Native::NumberIsNaN,
            Native::NumberIsFinite,
            Native::NumberIsInteger,
            Native::NumberIsSafeInteger,
        ],
    )?;
    let constant = PropertyFlags::data(false, false, false);
    for (name, value) in [
        ("MAX_VALUE", f64::MAX),
        ("MIN_VALUE", f64::from_bits(1)),
        ("NaN", f64::NAN),
        ("NEGATIVE_INFINITY", f64::NEG_INFINITY),
        ("POSITIVE_INFINITY", f64::INFINITY),
        ("EPSILON", f64::EPSILON),
        ("MAX_SAFE_INTEGER", 9_007_199_254_740_991.0),
        ("MIN_SAFE_INTEGER", -9_007_199_254_740_991.0),
    ] {
        define(cx, constructor, name, Value::number(value), constant)?;
    }
    methods(
        cx,
        prototype,
        &[
            Native::NumberToExponential,
            Native::NumberToFixed,
            Native::NumberToPrecision,
            Native::NumberToString,
            Native::NumberToLocaleString,
            Native::NumberValueOf,
        ],
    )?;
    link_constructor(cx, constructor, prototype)
}

/// `Number(value)`; `new Number(value)` wraps it.
pub(super) fn constructor(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let value = if site.argc == 0 {
        Value::int32(0)
    } else {
        let value = site.arg(cx, 0);
        let numeric = ops::to_numeric(cx, value)?;
        if numeric.as_bigint().is_some() {
            // The decimal text parses to the correctly rounded number.
            let text = ops::big(cx.heap, numeric).to_string_radix10();
            let units: Vec<u16> = text.encode_utf16().collect();
            number::number(number::parse(&units))
        } else {
            numeric
        }
    };
    if !site.construct {
        return Ok(value);
    }
    let prototype =
        internal::prototype_from_constructor(cx, site.new_target, Intrinsic::NumberPrototype)?;
    internal::wrap(cx, ObjectKind::NumberWrapper, value, prototype).map(Value::object)
}

/// The receiver as a number (`thisNumberValue`): a number or a wrapper.
pub(crate) fn this_number(cx: &mut Cx<'_>, site: &CallSite) -> Result<f64, Throw> {
    let this = site.this(cx);
    if let Some(value) = this.as_f64() {
        return Ok(value);
    }
    if let Some(object) = this.as_object()
        && cx.heap.object_kind(object) == ObjectKind::NumberWrapper
    {
        return Ok(cx.heap.internal_slot(object, 0).as_f64().unwrap());
    }
    Err(error::type_error(cx, "not a number"))
}

pub(super) fn to_string(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let value = this_number(cx, &site)?;
    let radix = site.arg(cx, 0);
    let radix = if radix.is_undefined() {
        10.0
    } else {
        internal::to_integer_or_infinity(cx, radix)?
    };
    if !(2.0..=36.0).contains(&radix) {
        return Err(error::range_error(cx, "radix must be between 2 and 36"));
    }
    if radix == 10.0 {
        return ops::to_string(cx, number::number(value));
    }
    string_text(cx, &radix::to_string_radix(value, radix as u32))
}

/// `toLocaleString`: QuickJS formats as `toString()`.
pub(super) fn to_locale_string(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let value = this_number(cx, &site)?;
    ops::to_string(cx, number::number(value))
}

pub(super) fn value_of(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    this_number(cx, &site).map(number::number)
}

/// A digit-count argument (`ToIntegerOrInfinity`).
fn digits(cx: &mut Cx<'_>, value: Value) -> Result<f64, Throw> {
    internal::to_integer_or_infinity(cx, value)
}

pub(super) fn to_fixed(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let value = this_number(cx, &site)?;
    let fraction = site.arg(cx, 0);
    let fraction = digits(cx, fraction)?;
    if !(0.0..=f64::from(radix::MAX_DIGITS)).contains(&fraction) {
        return Err(error::range_error(cx, "invalid number of digits"));
    }
    string_text(cx, &radix::to_fixed(value, fraction as i32))
}

pub(super) fn to_exponential(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let value = this_number(cx, &site)?;
    let argument = site.arg(cx, 0);
    let fraction = digits(cx, argument)?;
    if !value.is_finite() {
        return string_text(cx, &radix::to_exponential(value, None));
    }
    if !(0.0..=f64::from(radix::MAX_DIGITS)).contains(&fraction) {
        return Err(error::range_error(cx, "invalid number of digits"));
    }
    let fraction = if argument.is_undefined() {
        None
    } else {
        Some(fraction as i32)
    };
    string_text(cx, &radix::to_exponential(value, fraction))
}

pub(super) fn to_precision(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let value = this_number(cx, &site)?;
    let argument = site.arg(cx, 0);
    if argument.is_undefined() {
        return ops::to_string(cx, number::number(value));
    }
    let precision = digits(cx, argument)?;
    if !value.is_finite() {
        return ops::to_string(cx, number::number(value));
    }
    if !(1.0..=f64::from(radix::MAX_DIGITS)).contains(&precision) {
        return Err(error::range_error(cx, "invalid number of digits"));
    }
    string_text(cx, &radix::to_precision(value, precision as i32))
}

pub(super) fn is_finite(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    Ok(Value::bool(
        site.arg(cx, 0).as_f64().is_some_and(f64::is_finite),
    ))
}

pub(super) fn is_nan(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    Ok(Value::bool(
        site.arg(cx, 0).as_f64().is_some_and(f64::is_nan),
    ))
}

pub(super) fn is_integer(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    Ok(Value::bool(site.arg(cx, 0).as_f64().is_some_and(|value| {
        value.is_finite() && value.trunc() == value
    })))
}

pub(super) fn is_safe_integer(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    Ok(Value::bool(site.arg(cx, 0).as_f64().is_some_and(|value| {
        value.is_finite() && value.trunc() == value && value.abs() <= 9_007_199_254_740_991.0
    })))
}
