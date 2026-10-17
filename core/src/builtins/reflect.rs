//! `Reflect` (section 3.1): the natives over the internal methods and
//! nothing else.

use super::{Native, define, methods, plain_object, to_string_tag};
use crate::heap::HeapError;
use crate::runtime::Intrinsic;
use crate::value::{ObjectId, Value};
use crate::vm::native::CallSite;
use crate::vm::object as internal;
use crate::vm::{Cx, Throw, error, ops};

pub(super) fn install(cx: &mut Cx<'_>) -> Result<(), HeapError> {
    let prototype = cx.realm.intrinsic(Intrinsic::ObjectPrototype);
    let reflect = plain_object(cx, Some(prototype), 16)?;
    methods(
        cx,
        reflect,
        &[
            Native::ReflectApply,
            Native::ReflectConstruct,
            Native::ReflectDefineProperty,
            Native::ReflectDeleteProperty,
            Native::ReflectGet,
            Native::ReflectGetOwnPropertyDescriptor,
            Native::ReflectGetPrototypeOf,
            Native::ReflectHas,
            Native::ReflectIsExtensible,
            Native::ReflectOwnKeys,
            Native::ReflectPreventExtensions,
            Native::ReflectSet,
            Native::ReflectSetPrototypeOf,
        ],
    )?;
    to_string_tag(cx, reflect, "Reflect")?;
    let global = cx.realm.global();
    define(
        cx,
        global,
        "Reflect",
        Value::object(reflect),
        super::hidden(),
    )
}

fn target(cx: &mut Cx<'_>, site: &CallSite) -> Result<ObjectId, Throw> {
    match site.arg(cx, 0).as_object() {
        Some(object) => Ok(object),
        None => Err(error::type_error(cx, "not an object")),
    }
}

fn key(cx: &mut Cx<'_>, site: &CallSite, index: usize) -> Result<crate::PropertyKey, Throw> {
    let value = site.arg(cx, index);
    let key = ops::to_property_key(cx, value)?;
    cx.heap.roots().push(key.to_value());
    Ok(key)
}

pub(super) fn apply(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let function = site.arg(cx, 0);
    if !ops::is_callable(cx.heap, function) {
        return Err(error::type_error(cx, "not a function"));
    }
    let this = site.arg(cx, 1);
    let list = site.arg(cx, 2);
    let args = internal::list_from_array_like(cx, list)?;
    crate::vm::call(cx, function, this, &args)
}

pub(super) fn construct(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let function = site.arg(cx, 0);
    let new_target = if site.argc > 2 {
        site.arg(cx, 2)
    } else {
        function
    };
    if !internal::is_constructor(cx, function) {
        return Err(construct_refused(cx, &site, function, new_target));
    }
    if !internal::is_constructor(cx, new_target) {
        return Err(error::not_a_constructor(cx, new_target));
    }
    let list = site.arg(cx, 1);
    let args = internal::list_from_array_like(cx, list)?;
    crate::vm::construct(cx, function, &args, new_target)
}

/// `Reflect.construct` on a `target` that is not a constructor, with the
/// error QuickJS reaches first: its `newTarget` check, then the arguments
/// list's `not a object`, then the construction itself.
#[cold]
#[inline(never)]
fn construct_refused(cx: &mut Cx<'_>, site: &CallSite, target: Value, new_target: Value) -> Throw {
    if site.argc > 2 && !internal::is_constructor(cx, new_target) {
        return error::not_a_constructor(cx, new_target);
    }
    if site.arg(cx, 1).as_object().is_none() {
        return error::type_error(cx, "not a object");
    }
    error::construct_error(cx, target)
}

pub(super) fn define_property(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let object = target(cx, &site)?;
    let key = key(cx, &site, 1)?;
    let attributes = site.arg(cx, 2);
    let descriptor = internal::to_property_descriptor(cx, attributes)?;
    internal::define_own_property(cx, object, key, &descriptor).map(Value::bool)
}

pub(super) fn delete_property(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let object = target(cx, &site)?;
    let key = key(cx, &site, 1)?;
    internal::delete(cx, object, key).map(Value::bool)
}

pub(super) fn get(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let object = target(cx, &site)?;
    let key = key(cx, &site, 1)?;
    let receiver = if site.argc > 2 {
        site.arg(cx, 2)
    } else {
        Value::object(object)
    };
    internal::get(cx, object, key, receiver)
}

pub(super) fn get_own_property_descriptor(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let object = target(cx, &site)?;
    let key = key(cx, &site, 1)?;
    match internal::get_own_property(cx, object, key)? {
        Some(descriptor) => internal::from_property_descriptor(cx, &descriptor),
        None => Ok(Value::UNDEFINED),
    }
}

pub(super) fn get_prototype_of(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let object = target(cx, &site)?;
    internal::get_prototype_of(cx, object)
}

pub(super) fn has(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let object = target(cx, &site)?;
    let key = key(cx, &site, 1)?;
    internal::has_property(cx, object, key).map(Value::bool)
}

pub(super) fn is_extensible(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let object = target(cx, &site)?;
    internal::is_extensible(cx, object).map(Value::bool)
}

pub(super) fn own_keys(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let object = target(cx, &site)?;
    let keys = internal::own_property_keys(cx, object)?;
    let mut list = Vec::with_capacity(keys.len());
    for key in keys {
        let value = super::object::key_string(cx, key)?;
        cx.heap.roots().push(value);
        list.push(value);
    }
    internal::array_from_list(cx, &list).map(Value::object)
}

pub(super) fn prevent_extensions(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let object = target(cx, &site)?;
    internal::prevent_extensions(cx, object).map(Value::bool)
}

pub(super) fn set(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let object = target(cx, &site)?;
    let key = key(cx, &site, 1)?;
    let value = site.arg(cx, 2);
    let receiver = if site.argc > 3 {
        site.arg(cx, 3)
    } else {
        Value::object(object)
    };
    internal::set(cx, object, key, value, receiver).map(Value::bool)
}

pub(super) fn set_prototype_of(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let object = target(cx, &site)?;
    let prototype = site.arg(cx, 1);
    if prototype.as_object().is_none() && !prototype.is_null() {
        return Err(error::type_error(cx, "not an object"));
    }
    internal::set_prototype_of(cx, object, prototype).map(Value::bool)
}
