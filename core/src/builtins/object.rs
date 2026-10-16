//! `Object`: the constructor and `Object.prototype`'s `toString`, `valueOf`
//! and `hasOwnProperty`.

use super::{
    Native, install_constructor, methods, new_plain, string_text, this_object, unsupported,
};
use crate::heap::HeapError;
use crate::heap::atom::KeyKind;
use crate::heap::object::ObjectKind;
use crate::runtime::Intrinsic;
use crate::value::{Kind, Value};
use crate::vm::native::CallSite;
use crate::vm::{Cx, Throw, ops, property};

pub(super) fn install(cx: &mut Cx<'_>) -> Result<(), HeapError> {
    let prototype = cx.realm.intrinsic(Intrinsic::ObjectPrototype);
    install_constructor(cx, "Object", Native::ObjectConstructor, prototype)?;
    methods(
        cx,
        prototype,
        &[
            ("toString", Native::ObjectToString),
            ("valueOf", Native::ObjectValueOf),
            ("hasOwnProperty", Native::ObjectHasOwnProperty),
        ],
    )
}

/// `Object(value)` and `new Object(value)`: a new object for `undefined`
/// and `null`, the value itself for an object. Primitives would be wrapped,
/// which is phase 4's.
pub(super) fn constructor(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let value = site.arg(cx, 0);
    match value.kind() {
        Kind::Undefined | Kind::Null => new_plain(cx, 4).map(Value::object),
        Kind::Object => Ok(value),
        _ => Err(unsupported(cx, "wrapper objects")),
    }
}

/// `Object.prototype.toString`: `[object Tag]` by the receiver's class
/// (there is no `Symbol.toStringTag` in phase 3).
pub(super) fn to_string(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let this = site.this(cx);
    let tag = match this.kind() {
        Kind::Undefined => "Undefined",
        Kind::Null => "Null",
        Kind::String => "String",
        Kind::Number | Kind::Int32 => "Number",
        Kind::Bool => "Boolean",
        Kind::Symbol => "Symbol",
        Kind::BigInt => "BigInt",
        Kind::Empty => "Undefined",
        Kind::Object => {
            let object = this.as_object().unwrap();
            match cx.heap.object_kind(object) {
                ObjectKind::Array => "Array",
                ObjectKind::Function => "Function",
                ObjectKind::Arguments => "Arguments",
                ObjectKind::RegExp => "RegExp",
                _ if super::is_error(cx, object) => "Error",
                _ => "Object",
            }
        }
    };
    string_text(cx, &format!("[object {tag}]"))
}

/// `Object.prototype.valueOf`: the receiver as an object.
pub(super) fn value_of(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    this_object(cx, &site).map(Value::object)
}

/// `Object.prototype.hasOwnProperty(key)`.
pub(super) fn has_own_property(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let key = site.arg(cx, 0);
    let key = ops::to_property_key(cx, key)?;
    let this = site.this(cx);
    if let Some(string) = this.as_string() {
        let length = cx.heap.string(string).repr.len();
        let own = match key.kind() {
            KeyKind::Index(index) => (index as usize) < length,
            KeyKind::String(id) => id == cx.realm.names.length,
            KeyKind::Symbol(_) => false,
        };
        return Ok(Value::bool(own));
    }
    let object = this_object(cx, &site)?;
    let own = property::own(cx.heap, &cx.realm.names, object, key).is_some();
    Ok(Value::bool(own))
}
