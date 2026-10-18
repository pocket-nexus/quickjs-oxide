//! `Boolean`: `Boolean(v)` and `Boolean.prototype.toString` and `valueOf`.
//! `new Boolean` makes a wrapper object.

use super::{Native, install_constructor, link_constructor, methods};
use crate::heap::HeapError;
use crate::heap::object::ObjectKind;
use crate::runtime::Intrinsic;
use crate::value::Value;
use crate::vm::native::CallSite;
use crate::vm::object as internal;
use crate::vm::{Cx, Throw, error, ops};

pub(super) fn install(cx: &mut Cx<'_>) -> Result<(), HeapError> {
    let prototype = cx.realm.intrinsic(Intrinsic::BooleanPrototype);
    let constructor = install_constructor(cx, Native::BooleanConstructor)?;
    cx.set_intrinsic(Intrinsic::Boolean, constructor);
    methods(
        cx,
        prototype,
        &[Native::BooleanToString, Native::BooleanValueOf],
    )?;
    link_constructor(cx, constructor, prototype)
}

/// `Boolean(value)`; `new Boolean(value)` wraps it.
pub(super) fn constructor(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let value = site.arg(cx, 0);
    let value = Value::bool(ops::to_boolean(cx.heap, value));
    if !site.construct {
        return Ok(value);
    }
    let prototype =
        internal::prototype_from_constructor(cx, site.new_target, Intrinsic::BooleanPrototype)?;
    internal::wrap(cx, ObjectKind::BooleanWrapper, value, prototype).map(Value::object)
}

/// `thisBooleanValue`: a boolean or a wrapper.
fn this_boolean(cx: &mut Cx<'_>, site: &CallSite) -> Result<bool, Throw> {
    let this = site.this(cx);
    if let Some(value) = this.as_bool() {
        return Ok(value);
    }
    if let Some(object) = this.as_object()
        && cx.heap.object_kind(object) == ObjectKind::BooleanWrapper
    {
        return Ok(cx.heap.internal_slot(object, 0).as_bool().unwrap());
    }
    Err(error::type_error(cx, "not a boolean"))
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
