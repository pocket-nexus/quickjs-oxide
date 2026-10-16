//! `Number`: `Number(v)`, and `Number.prototype.toString(radix)` and
//! `valueOf`. `new Number` makes a wrapper object, which is phase 4's.

use super::{Native, install_constructor, methods, string_text, to_integer, unsupported};
use crate::heap::HeapError;
use crate::runtime::Intrinsic;
use crate::value::Value;
use crate::vm::native::CallSite;
use crate::vm::{Cx, Throw, error, number, ops, radix};

pub(super) fn install(cx: &mut Cx<'_>) -> Result<(), HeapError> {
    let prototype = cx.realm.intrinsic(Intrinsic::NumberPrototype);
    install_constructor(cx, "Number", Native::NumberConstructor, prototype)?;
    methods(
        cx,
        prototype,
        &[
            ("toString", Native::NumberToString),
            ("valueOf", Native::NumberValueOf),
        ],
    )
}

pub(super) fn constructor(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    if site.construct {
        return Err(unsupported(cx, "new Number"));
    }
    if site.argc == 0 {
        return Ok(Value::int32(0));
    }
    let value = site.arg(cx, 0);
    let numeric = ops::to_numeric(cx, value)?;
    if numeric.as_bigint().is_some() {
        // The decimal text parses to the correctly rounded number.
        let text = ops::big(cx.heap, numeric).to_string_radix10();
        let units: Vec<u16> = text.encode_utf16().collect();
        return Ok(number::number(number::parse(&units)));
    }
    Ok(numeric)
}

/// The receiver as a number (`thisNumberValue`).
fn this_number(cx: &mut Cx<'_>, site: &CallSite) -> Result<f64, Throw> {
    match site.this(cx).as_f64() {
        Some(value) => Ok(value),
        None => Err(error::type_error(cx, "not a number")),
    }
}

pub(super) fn to_string(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let value = this_number(cx, &site)?;
    let radix = site.arg(cx, 0);
    let radix = if radix.is_undefined() {
        10.0
    } else {
        to_integer(cx, radix)?
    };
    if !(2.0..=36.0).contains(&radix) {
        return Err(error::range_error(cx, "radix must be between 2 and 36"));
    }
    if radix == 10.0 {
        return ops::to_string(cx, site.this(cx));
    }
    string_text(cx, &radix::to_string_radix(value, radix as u32))
}

pub(super) fn value_of(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    this_number(cx, &site).map(number::number)
}
