//! `Boolean`: `Boolean(v)` and `Boolean.prototype.toString` and `valueOf`.
//! `new Boolean` makes a wrapper object, which is phase 4's.

use super::{Native, install_constructor, methods, unsupported};
use crate::heap::HeapError;
use crate::runtime::Intrinsic;
use crate::value::Value;
use crate::vm::native::CallSite;
use crate::vm::{Cx, Throw, error, ops};

pub(super) fn install(cx: &mut Cx<'_>) -> Result<(), HeapError> {
    let prototype = cx.realm.intrinsic(Intrinsic::BooleanPrototype);
    install_constructor(cx, "Boolean", Native::BooleanConstructor, prototype)?;
    methods(
        cx,
        prototype,
        &[
            ("toString", Native::BooleanToString),
            ("valueOf", Native::BooleanValueOf),
        ],
    )
}

pub(super) fn constructor(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    if site.construct {
        return Err(unsupported(cx, "new Boolean"));
    }
    let value = site.arg(cx, 0);
    Ok(Value::bool(ops::to_boolean(cx.heap, value)))
}

fn this_boolean(cx: &mut Cx<'_>, site: &CallSite) -> Result<bool, Throw> {
    match site.this(cx).as_bool() {
        Some(value) => Ok(value),
        None => Err(error::type_error(cx, "not a boolean")),
    }
}

pub(super) fn to_string(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let value = this_boolean(cx, &site)?;
    let names = cx.realm.names;
    Ok(Value::string(if value {
        names.true_
    } else {
        names.false_
    }))
}

pub(super) fn value_of(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    this_boolean(cx, &site).map(Value::bool)
}
