//! Proxy exotic objects (phase 4, section 3.1): every internal method
//! through the handler's trap, with the specification's invariant checks
//! against the target. A proxy's internal slot 0 is the target, slot 1 the
//! handler, `null` once revoked.

use super::error;
use super::object::{self, Descriptor};
use super::ops;
use super::{Cx, Throw};
use crate::heap::atom::{KeyKind, PropertyKey};
use crate::heap::object::ObjectKind;
use crate::value::{ObjectId, Value};

fn root(cx: &mut Cx<'_>, value: Value) {
    cx.heap.roots().push(value);
}

/// The target and handler, or a `TypeError` for a revoked proxy.
fn parts(cx: &mut Cx<'_>, proxy: ObjectId) -> Result<(ObjectId, ObjectId), Throw> {
    let handler = cx.heap.internal_slot(proxy, 1);
    let Some(handler) = handler.as_object() else {
        return Err(error::type_error(cx, "revoked proxy"));
    };
    let target = cx
        .heap
        .internal_slot(proxy, 0)
        .as_object()
        .expect("a target");
    Ok((target, handler))
}

/// The trap `name` of `handler`, or `None` when absent. The proxy keeps the
/// handler and target alive; the trap is rooted.
fn trap(cx: &mut Cx<'_>, handler: ObjectId, name: &str) -> Result<Option<Value>, Throw> {
    let key = cx
        .heap
        .property_key(name)
        .map_err(|error| object::heap_error(cx, error))?;
    let method = object::get_method(cx, Value::object(handler), key)?;
    if let Some(method) = method {
        root(cx, method);
    }
    Ok(method)
}

fn key_value(key: PropertyKey) -> Value {
    match key.kind() {
        KeyKind::Index(index) => Value::int32(index as i32),
        _ => key.to_value(),
    }
}

/// A key as the string or symbol a trap receives.
fn key_argument(cx: &mut Cx<'_>, key: PropertyKey) -> Result<Value, Throw> {
    match key.kind() {
        KeyKind::Index(_) => {
            let value = ops::to_string(cx, key_value(key))?;
            root(cx, value);
            Ok(value)
        }
        _ => Ok(key.to_value()),
    }
}

/// Runs `body` with the root stack cut back afterwards, and the native
/// stack checked: a proxy chain recurses through Rust.
fn scoped<T>(
    cx: &mut Cx<'_>,
    body: impl FnOnce(&mut Cx<'_>) -> Result<T, Throw>,
) -> Result<T, Throw> {
    if !cx.heap.vm.enter_native() {
        return Err(error::stack_overflow(cx));
    }
    let mark = cx.heap.roots().mark();
    let result = body(cx);
    cx.heap.roots().truncate(mark);
    cx.heap.vm.leave_native();
    result
}

fn invariant(cx: &mut Cx<'_>, trap: &str) -> Throw {
    error::type_error(cx, &format!("proxy: inconsistent {trap}"))
}

pub(crate) fn get_prototype_of(cx: &mut Cx<'_>, proxy: ObjectId) -> Result<Value, Throw> {
    scoped(cx, |cx| {
        let (target, handler) = parts(cx, proxy)?;
        let Some(trap) = trap(cx, handler, "getPrototypeOf")? else {
            return object::get_prototype_of(cx, target);
        };
        let result = super::call(cx, trap, Value::object(handler), &[Value::object(target)])?;
        if result.as_object().is_none() && !result.is_null() {
            return Err(invariant(cx, "prototype"));
        }
        root(cx, result);
        if object::is_extensible(cx, target)? {
            return Ok(result);
        }
        let actual = object::get_prototype_of(cx, target)?;
        if actual != result {
            return Err(invariant(cx, "prototype"));
        }
        Ok(result)
    })
}

pub(crate) fn set_prototype_of(
    cx: &mut Cx<'_>,
    proxy: ObjectId,
    prototype: Value,
) -> Result<bool, Throw> {
    scoped(cx, |cx| {
        let (target, handler) = parts(cx, proxy)?;
        let Some(trap) = trap(cx, handler, "setPrototypeOf")? else {
            return object::set_prototype_of(cx, target, prototype);
        };
        let result = super::call(
            cx,
            trap,
            Value::object(handler),
            &[Value::object(target), prototype],
        )?;
        if !ops::to_boolean(cx.heap, result) {
            return Ok(false);
        }
        if object::is_extensible(cx, target)? {
            return Ok(true);
        }
        let actual = object::get_prototype_of(cx, target)?;
        if actual != prototype {
            return Err(invariant(cx, "prototype"));
        }
        Ok(true)
    })
}

pub(crate) fn is_extensible(cx: &mut Cx<'_>, proxy: ObjectId) -> Result<bool, Throw> {
    scoped(cx, |cx| {
        let (target, handler) = parts(cx, proxy)?;
        let Some(trap) = trap(cx, handler, "isExtensible")? else {
            return object::is_extensible(cx, target);
        };
        let result = super::call(cx, trap, Value::object(handler), &[Value::object(target)])?;
        let result = ops::to_boolean(cx.heap, result);
        if result != object::is_extensible(cx, target)? {
            return Err(invariant(cx, "isExtensible"));
        }
        Ok(result)
    })
}

pub(crate) fn prevent_extensions(cx: &mut Cx<'_>, proxy: ObjectId) -> Result<bool, Throw> {
    scoped(cx, |cx| {
        let (target, handler) = parts(cx, proxy)?;
        let Some(trap) = trap(cx, handler, "preventExtensions")? else {
            return object::prevent_extensions(cx, target);
        };
        let result = super::call(cx, trap, Value::object(handler), &[Value::object(target)])?;
        let result = ops::to_boolean(cx.heap, result);
        if result && object::is_extensible(cx, target)? {
            return Err(invariant(cx, "preventExtensions"));
        }
        Ok(result)
    })
}

pub(crate) fn get_own_property(
    cx: &mut Cx<'_>,
    proxy: ObjectId,
    key: PropertyKey,
) -> Result<Option<Descriptor>, Throw> {
    let result = scoped(cx, |cx| {
        let (target, handler) = parts(cx, proxy)?;
        let Some(trap) = trap(cx, handler, "getOwnPropertyDescriptor")? else {
            let descriptor = object::get_own_property(cx, target, key)?;
            return Ok(descriptor.map(|descriptor| (descriptor, Vec::new())));
        };
        let name = key_argument(cx, key)?;
        let result = super::call(
            cx,
            trap,
            Value::object(handler),
            &[Value::object(target), name],
        )?;
        if result.as_object().is_none() && !result.is_undefined() {
            return Err(invariant(cx, "getOwnPropertyDescriptor"));
        }
        root(cx, result);
        let current = object::get_own_property(cx, target, key)?;
        if let Some(current) = &current {
            current.root(cx);
        }
        if result.is_undefined() {
            let Some(current) = current else {
                return Ok(None);
            };
            if current.configurable == Some(false) {
                return Err(invariant(cx, "getOwnPropertyDescriptor"));
            }
            if !object::is_extensible(cx, target)? {
                return Err(invariant(cx, "getOwnPropertyDescriptor"));
            }
            return Ok(None);
        }
        let extensible = object::is_extensible(cx, target)?;
        let descriptor = object::to_property_descriptor(cx, result)?;
        let complete = complete(&descriptor);
        if !object::compatible(cx, extensible, &complete, current.as_ref()) {
            return Err(invariant(cx, "getOwnPropertyDescriptor"));
        }
        if complete.configurable == Some(false) {
            match &current {
                None => return Err(invariant(cx, "getOwnPropertyDescriptor")),
                Some(current) if current.configurable == Some(true) => {
                    return Err(invariant(cx, "getOwnPropertyDescriptor"));
                }
                Some(current) => {
                    if complete.writable == Some(false) && current.writable == Some(true) {
                        return Err(invariant(cx, "getOwnPropertyDescriptor"));
                    }
                }
            }
        }
        // The returned values must outlive the scope: the caller roots
        // them from the list below.
        let values = [complete.value, complete.get, complete.set]
            .into_iter()
            .flatten()
            .collect();
        Ok(Some((complete, values)))
    })?;
    Ok(
        result.map(|(descriptor, values): (Descriptor, Vec<Value>)| {
            for value in values {
                cx.heap.roots().push(value);
            }
            descriptor
        }),
    )
}

/// `CompletePropertyDescriptor`.
fn complete(descriptor: &Descriptor) -> Descriptor {
    let mut complete = *descriptor;
    if descriptor.is_generic() || descriptor.is_data() {
        complete.value.get_or_insert(Value::UNDEFINED);
        complete.writable.get_or_insert(false);
    } else {
        complete.get.get_or_insert(Value::UNDEFINED);
        complete.set.get_or_insert(Value::UNDEFINED);
    }
    complete.enumerable.get_or_insert(false);
    complete.configurable.get_or_insert(false);
    complete
}

pub(crate) fn define_own_property(
    cx: &mut Cx<'_>,
    proxy: ObjectId,
    key: PropertyKey,
    descriptor: &Descriptor,
) -> Result<bool, Throw> {
    scoped(cx, |cx| {
        let (target, handler) = parts(cx, proxy)?;
        let Some(trap) = trap(cx, handler, "defineProperty")? else {
            return object::define_own_property(cx, target, key, descriptor);
        };
        let name = key_argument(cx, key)?;
        let object = object::from_property_descriptor(cx, descriptor)?;
        root(cx, object);
        let result = super::call(
            cx,
            trap,
            Value::object(handler),
            &[Value::object(target), name, object],
        )?;
        if !ops::to_boolean(cx.heap, result) {
            return Ok(false);
        }
        let current = object::get_own_property(cx, target, key)?;
        if let Some(current) = &current {
            current.root(cx);
        }
        let extensible = object::is_extensible(cx, target)?;
        let setting_non_configurable = descriptor.configurable == Some(false);
        match current {
            None => {
                if !extensible || setting_non_configurable {
                    return Err(invariant(cx, "defineProperty"));
                }
            }
            Some(current) => {
                if !object::compatible(cx, extensible, descriptor, Some(&current)) {
                    return Err(invariant(cx, "defineProperty"));
                }
                if setting_non_configurable && current.configurable == Some(true) {
                    return Err(invariant(cx, "defineProperty"));
                }
                if current.is_data()
                    && current.configurable == Some(false)
                    && current.writable == Some(true)
                    && descriptor.writable == Some(false)
                {
                    return Err(invariant(cx, "defineProperty"));
                }
            }
        }
        Ok(true)
    })
}

pub(crate) fn has(cx: &mut Cx<'_>, proxy: ObjectId, key: PropertyKey) -> Result<bool, Throw> {
    scoped(cx, |cx| {
        let (target, handler) = parts(cx, proxy)?;
        let Some(trap) = trap(cx, handler, "has")? else {
            return object::has_property(cx, target, key);
        };
        let name = key_argument(cx, key)?;
        let result = super::call(
            cx,
            trap,
            Value::object(handler),
            &[Value::object(target), name],
        )?;
        let result = ops::to_boolean(cx.heap, result);
        if !result
            && let Some(current) = object::get_own_property(cx, target, key)?
            && (current.configurable == Some(false) || !object::is_extensible(cx, target)?)
        {
            return Err(invariant(cx, "has"));
        }
        Ok(result)
    })
}

pub(crate) fn get(
    cx: &mut Cx<'_>,
    proxy: ObjectId,
    key: PropertyKey,
    receiver: Value,
) -> Result<Value, Throw> {
    scoped(cx, |cx| {
        let (target, handler) = parts(cx, proxy)?;
        let Some(trap) = trap(cx, handler, "get")? else {
            return object::get(cx, target, key, receiver);
        };
        let name = key_argument(cx, key)?;
        let result = super::call(
            cx,
            trap,
            Value::object(handler),
            &[Value::object(target), name, receiver],
        )?;
        root(cx, result);
        if let Some(current) = object::get_own_property(cx, target, key)?
            && current.configurable == Some(false)
        {
            if current.is_data()
                && current.writable == Some(false)
                && !object::same_value(cx, result, current.value.unwrap_or(Value::UNDEFINED))
            {
                return Err(invariant(cx, "get"));
            }
            if current.is_accessor()
                && current.get.unwrap_or(Value::UNDEFINED).is_undefined()
                && !result.is_undefined()
            {
                return Err(invariant(cx, "get"));
            }
        }
        Ok(result)
    })
}

pub(crate) fn set(
    cx: &mut Cx<'_>,
    proxy: ObjectId,
    key: PropertyKey,
    value: Value,
    receiver: Value,
) -> Result<bool, Throw> {
    scoped(cx, |cx| {
        let (target, handler) = parts(cx, proxy)?;
        let Some(trap) = trap(cx, handler, "set")? else {
            return object::set(cx, target, key, value, receiver);
        };
        let name = key_argument(cx, key)?;
        let result = super::call(
            cx,
            trap,
            Value::object(handler),
            &[Value::object(target), name, value, receiver],
        )?;
        if !ops::to_boolean(cx.heap, result) {
            return Ok(false);
        }
        if let Some(current) = object::get_own_property(cx, target, key)?
            && current.configurable == Some(false)
        {
            if current.is_data()
                && current.writable == Some(false)
                && !object::same_value(cx, value, current.value.unwrap_or(Value::UNDEFINED))
            {
                return Err(invariant(cx, "set"));
            }
            if current.is_accessor() && current.set.unwrap_or(Value::UNDEFINED).is_undefined() {
                return Err(invariant(cx, "set"));
            }
        }
        Ok(true)
    })
}

pub(crate) fn delete(cx: &mut Cx<'_>, proxy: ObjectId, key: PropertyKey) -> Result<bool, Throw> {
    scoped(cx, |cx| {
        let (target, handler) = parts(cx, proxy)?;
        let Some(trap) = trap(cx, handler, "deleteProperty")? else {
            return object::delete(cx, target, key);
        };
        let name = key_argument(cx, key)?;
        let result = super::call(
            cx,
            trap,
            Value::object(handler),
            &[Value::object(target), name],
        )?;
        if !ops::to_boolean(cx.heap, result) {
            return Ok(false);
        }
        if let Some(current) = object::get_own_property(cx, target, key)?
            && (current.configurable == Some(false) || !object::is_extensible(cx, target)?)
        {
            return Err(invariant(cx, "deleteProperty"));
        }
        Ok(true)
    })
}

pub(crate) fn own_keys(cx: &mut Cx<'_>, proxy: ObjectId) -> Result<Vec<PropertyKey>, Throw> {
    let mark = cx.heap.roots().mark();
    let keys = scoped(cx, |cx| {
        let (target, handler) = parts(cx, proxy)?;
        let Some(trap) = trap(cx, handler, "ownKeys")? else {
            return object::own_property_keys(cx, target);
        };
        let result = super::call(cx, trap, Value::object(handler), &[Value::object(target)])?;
        root(cx, result);
        let Some(list) = result.as_object() else {
            return Err(invariant(cx, "ownKeys"));
        };
        let length = object::length_of_array_like(cx, list)?;
        let mut keys: Vec<PropertyKey> = Vec::new();
        for index in 0..length {
            let element_key = object::index_key(cx, index as u32)?;
            let element = object::get_property(cx, list, element_key)?;
            if element.as_string().is_none() && element.as_symbol().is_none() {
                return Err(error::type_error(
                    cx,
                    "proxy: properties must be strings or symbols",
                ));
            }
            root(cx, element);
            let key = ops::to_property_key(cx, element)?;
            if keys.contains(&key) {
                return Err(error::type_error(cx, "proxy: duplicate property"));
            }
            keys.push(key);
        }
        let extensible = object::is_extensible(cx, target)?;
        let target_keys = object::own_property_keys(cx, target)?;
        let mut configurable = Vec::new();
        let mut non_configurable = Vec::new();
        for key in target_keys {
            match object::get_own_property(cx, target, key)? {
                Some(descriptor) if descriptor.configurable == Some(false) => {
                    non_configurable.push(key)
                }
                _ => configurable.push(key),
            }
        }
        if extensible && non_configurable.is_empty() {
            return Ok(keys);
        }
        let mut unchecked = keys.clone();
        for key in non_configurable {
            let Some(position) = unchecked.iter().position(|&k| k == key) else {
                return Err(error::type_error(
                    cx,
                    "proxy: target property must be present in proxy ownKeys",
                ));
            };
            unchecked.remove(position);
        }
        if extensible {
            return Ok(keys);
        }
        for key in configurable {
            let Some(position) = unchecked.iter().position(|&k| k == key) else {
                return Err(error::type_error(
                    cx,
                    "proxy: target property must be present in proxy ownKeys",
                ));
            };
            unchecked.remove(position);
        }
        if !unchecked.is_empty() {
            return Err(error::type_error(
                cx,
                "proxy: property not present in target were returned by non extensible proxy",
            ));
        }
        Ok(keys)
    });
    // The scope cut the keys' roots; root the symbols again for the caller.
    cx.heap.roots().truncate(mark);
    let keys = keys?;
    for key in &keys {
        if let KeyKind::Symbol(symbol) = key.kind() {
            cx.heap.roots().push(Value::symbol(symbol));
        }
    }
    Ok(keys)
}

/// `[[Call]]` of a proxy: the `apply` trap with the arguments as an array.
pub(crate) fn call(
    cx: &mut Cx<'_>,
    proxy: ObjectId,
    this: Value,
    args: &[Value],
) -> Result<Value, Throw> {
    scoped(cx, |cx| {
        for &arg in args {
            root(cx, arg);
        }
        root(cx, this);
        let (target, handler) = parts(cx, proxy)?;
        let Some(trap) = trap(cx, handler, "apply")? else {
            return super::call(cx, Value::object(target), this, args);
        };
        let array = object::array_from_list(cx, args)?;
        root(cx, Value::object(array));
        super::call(
            cx,
            trap,
            Value::object(handler),
            &[Value::object(target), this, Value::object(array)],
        )
    })
}

/// `[[Construct]]` of a proxy: the `construct` trap, whose result must be
/// an object.
pub(crate) fn construct(
    cx: &mut Cx<'_>,
    proxy: ObjectId,
    args: &[Value],
    new_target: Value,
) -> Result<Value, Throw> {
    scoped(cx, |cx| {
        for &arg in args {
            root(cx, arg);
        }
        root(cx, new_target);
        let (target, handler) = parts(cx, proxy)?;
        let Some(trap) = trap(cx, handler, "construct")? else {
            return super::construct(cx, Value::object(target), args, new_target);
        };
        let array = object::array_from_list(cx, args)?;
        root(cx, Value::object(array));
        let result = super::call(
            cx,
            trap,
            Value::object(handler),
            &[Value::object(target), Value::object(array), new_target],
        )?;
        if result.as_object().is_none() {
            return Err(error::type_error(cx, "not an object"));
        }
        Ok(result)
    })
}

/// `ProxyCreate(target, handler)`.
pub(crate) fn create(cx: &mut Cx<'_>, target: Value, handler: Value) -> Result<ObjectId, Throw> {
    let (Some(target_object), Some(_)) = (target.as_object(), handler.as_object()) else {
        return Err(error::type_error(cx, "not an object"));
    };
    let header = *cx.heap.header(target_object);
    use crate::heap::object::flags;
    let callable = header.flags & (flags::CALLABLE | flags::CONSTRUCTOR);
    cx.heap
        .alloc_internal(
            ObjectKind::Proxy,
            crate::heap::object::InternalTag::None,
            None,
            0,
            &[target, handler],
            Default::default(),
            callable,
        )
        .map_err(|error| object::heap_error(cx, error))
}
