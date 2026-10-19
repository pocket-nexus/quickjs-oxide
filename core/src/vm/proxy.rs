//! Proxy exotic objects: every internal method through the handler's trap, with
//! the invariant checks of 10.5.1–10.5.13 against the target. A proxy's
//! internal slot 0 is the target, slot 1 the handler and slot 2 whether it is
//! revoked: as in QuickJS, a revoked proxy keeps both, so an internal method
//! that was running when its trap revoked the proxy still has them.

use super::error;
use super::object::{self, Descriptor};
use super::ops;
use super::{Cx, Throw};
use crate::heap::Heap;
use crate::heap::atom::{KeyKind, PropertyKey};
use crate::heap::object::{ObjectKind, flags};
use crate::value::{ObjectId, Value};

const REVOKED: usize = 2;

fn root(cx: &mut Cx<'_>, value: Value) {
    cx.heap.roots().push(value);
}

/// Whether `proxy` has been revoked.
pub(crate) fn is_revoked(heap: &Heap, proxy: ObjectId) -> bool {
    heap.internal_slot(proxy, REVOKED).as_bool() == Some(true)
}

/// The target of `proxy`, revoked or not.
pub(crate) fn target(heap: &Heap, proxy: ObjectId) -> ObjectId {
    heap.internal_slot(proxy, 0).as_object().expect("a target")
}

/// Marks `proxy` revoked.
pub(crate) fn revoke(heap: &mut Heap, proxy: ObjectId) {
    heap.set_internal_slot(proxy, REVOKED, Value::bool(true));
}

/// Roots `key` for the trap: a fresh atom (a computed key's) has no other
/// holder, and the trap runs JavaScript before the checks read the key.
fn root_key(cx: &mut Cx<'_>, key: PropertyKey) {
    if let Some(value) = key.heap_value() {
        root(cx, value);
    }
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

/// What an internal method of a proxy runs.
enum Dispatch {
    /// No proxy on the chain had the trap: the method of this object, which
    /// is not a proxy.
    Forward(ObjectId),
    /// The trap of the proxy whose target and handler these are.
    Trap {
        target: ObjectId,
        handler: ObjectId,
        trap: Value,
    },
}

/// QuickJS's `get_proxy_method` on `proxy`, then on its target for as long
/// as the handler has no trap `name` and the target is a proxy. QuickJS
/// recurses there; this loop charges each level `frame` bytes of the native
/// stack instead (`None` for the forwardings QuickJS compiles as tail
/// calls), so an empty-handler chain overflows at QuickJS's depth.
fn dispatch(
    cx: &mut Cx<'_>,
    mut proxy: ObjectId,
    name: &str,
    frame: Option<usize>,
) -> Result<Dispatch, Throw> {
    let key = cx
        .heap
        .property_key(name)
        .map_err(|error| object::heap_error(cx, error))?;
    let budget = cx.heap.vm.native_remaining();
    let mut used = 0usize;
    loop {
        if let Some(frame) = frame {
            if used >= budget {
                return Err(error::stack_overflow(cx));
            }
            used += frame;
        }
        if is_revoked(cx.heap, proxy) {
            return Err(error::type_error(cx, "revoked proxy"));
        }
        let target = target(cx.heap, proxy);
        let handler = cx
            .heap
            .internal_slot(proxy, 1)
            .as_object()
            .expect("a handler");
        let mut trap = object::get_property(cx, handler, key)?;
        if trap.is_null() {
            trap = Value::UNDEFINED;
        }
        if !trap.is_undefined() {
            root(cx, trap);
            return Ok(Dispatch::Trap {
                target,
                handler,
                trap,
            });
        }
        if cx.heap.object_kind(target) != ObjectKind::Proxy {
            return Ok(Dispatch::Forward(target));
        }
        proxy = target;
    }
}

pub(crate) fn get_prototype_of(cx: &mut Cx<'_>, proxy: ObjectId) -> Result<Value, Throw> {
    scoped(cx, |cx| {
        let (target, handler, trap) = match dispatch(cx, proxy, "getPrototypeOf", Some(128))? {
            Dispatch::Forward(target) => return object::get_prototype_of(cx, target),
            Dispatch::Trap {
                target,
                handler,
                trap,
                ..
            } => (target, handler, trap),
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
        if !object::same_value(cx, actual, result) {
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
        let (target, handler, trap) = match dispatch(cx, proxy, "setPrototypeOf", Some(64))? {
            Dispatch::Forward(target) => {
                return object::set_prototype_of(cx, target, prototype);
            }
            Dispatch::Trap {
                target,
                handler,
                trap,
                ..
            } => (target, handler, trap),
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
        if !object::same_value(cx, prototype, actual) {
            return Err(invariant(cx, "prototype"));
        }
        Ok(true)
    })
}

pub(crate) fn is_extensible(cx: &mut Cx<'_>, proxy: ObjectId) -> Result<bool, Throw> {
    scoped(cx, |cx| {
        let (target, handler, trap) = match dispatch(cx, proxy, "isExtensible", None)? {
            Dispatch::Forward(target) => return object::is_extensible(cx, target),
            Dispatch::Trap {
                target,
                handler,
                trap,
                ..
            } => (target, handler, trap),
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
        let (target, handler, trap) = match dispatch(cx, proxy, "preventExtensions", None)? {
            Dispatch::Forward(target) => return object::prevent_extensions(cx, target),
            Dispatch::Trap {
                target,
                handler,
                trap,
                ..
            } => (target, handler, trap),
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
        root_key(cx, key);
        let (target, handler, trap) = match dispatch(cx, proxy, "getOwnPropertyDescriptor", None)? {
            Dispatch::Forward(target) => {
                let descriptor = object::get_own_property(cx, target, key)?;
                let values = descriptor
                    .iter()
                    .flat_map(|d| [d.value, d.get, d.set])
                    .flatten()
                    .collect();
                return Ok(descriptor.map(|descriptor| (descriptor, values)));
            }
            Dispatch::Trap {
                target,
                handler,
                trap,
                ..
            } => (target, handler, trap),
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
        // 10.5.5 steps 10–15.
        if result.is_undefined() {
            let Some(current) = current else {
                return Ok(None);
            };
            if current.configurable == Some(false) || !object::is_extensible(cx, target)? {
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
                Some(current) if current.configurable == Some(false) => {
                    if complete.writable == Some(false) && current.writable == Some(true) {
                        return Err(invariant(cx, "getOwnPropertyDescriptor"));
                    }
                }
                _ => return Err(invariant(cx, "getOwnPropertyDescriptor")),
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
    cx.heap.vm.proxy_refused = false;
    scoped(cx, |cx| {
        root_key(cx, key);
        let (target, handler, trap) = match dispatch(cx, proxy, "defineProperty", None)? {
            Dispatch::Forward(target) => {
                return object::define_own_property(cx, target, key, descriptor);
            }
            Dispatch::Trap {
                target,
                handler,
                trap,
                ..
            } => (target, handler, trap),
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
            cx.heap.vm.proxy_refused = true;
            return Ok(false);
        }
        // 10.5.6 steps 11–16.
        let current = object::get_own_property(cx, target, key)?;
        if let Some(current) = &current {
            current.root(cx);
        }
        let extensible = object::is_extensible(cx, target)?;
        let setting_non_configurable = descriptor.configurable == Some(false);
        let Some(current) = current else {
            if !extensible || setting_non_configurable {
                return Err(invariant(cx, "defineProperty"));
            }
            return Ok(true);
        };
        if !object::compatible(cx, extensible, descriptor, Some(&current)) {
            return Err(invariant(cx, "defineProperty"));
        }
        if current.configurable == Some(true) && setting_non_configurable {
            return Err(invariant(cx, "defineProperty"));
        }
        if !current.is_accessor()
            && current.configurable == Some(false)
            && current.writable == Some(true)
            && descriptor.writable == Some(false)
        {
            return Err(invariant(cx, "defineProperty"));
        }
        Ok(true)
    })
}

pub(crate) fn has(cx: &mut Cx<'_>, proxy: ObjectId, key: PropertyKey) -> Result<bool, Throw> {
    scoped(cx, |cx| {
        root_key(cx, key);
        let (target, handler, trap) = match dispatch(cx, proxy, "has", Some(128))? {
            Dispatch::Forward(target) => return object::has_property(cx, target, key),
            Dispatch::Trap {
                target,
                handler,
                trap,
                ..
            } => (target, handler, trap),
        };
        let name = key_argument(cx, key)?;
        let result = super::call(
            cx,
            trap,
            Value::object(handler),
            &[Value::object(target), name],
        )?;
        let result = ops::to_boolean(cx.heap, result);
        // 10.5.7 step 9.
        if !result
            && let Some(current) = object::get_own_property(cx, target, key)?
            && (current.configurable != Some(true) || !object::is_extensible(cx, target)?)
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
        root_key(cx, key);
        let (target, handler, trap) = match dispatch(cx, proxy, "get", Some(512))? {
            Dispatch::Forward(target) => return object::get(cx, target, key, receiver),
            Dispatch::Trap {
                target,
                handler,
                trap,
                ..
            } => (target, handler, trap),
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
    cx.heap.vm.proxy_refused = false;
    scoped(cx, |cx| {
        root_key(cx, key);
        let (target, handler, trap) = match dispatch(cx, proxy, "set", Some(256))? {
            Dispatch::Forward(target) => return object::set(cx, target, key, value, receiver),
            Dispatch::Trap {
                target,
                handler,
                trap,
                ..
            } => (target, handler, trap),
        };
        let name = key_argument(cx, key)?;
        let result = super::call(
            cx,
            trap,
            Value::object(handler),
            &[Value::object(target), name, value, receiver],
        )?;
        if !ops::to_boolean(cx.heap, result) {
            cx.heap.vm.proxy_refused = true;
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
        root_key(cx, key);
        let (target, handler, trap) = match dispatch(cx, proxy, "deleteProperty", Some(256))? {
            Dispatch::Forward(target) => return object::delete(cx, target, key),
            Dispatch::Trap {
                target,
                handler,
                trap,
                ..
            } => (target, handler, trap),
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
        if let Some(current) = object::get_own_property(cx, target, key)? {
            if current.configurable != Some(true) {
                return Err(invariant(cx, "deleteProperty"));
            }
            if !object::is_extensible(cx, target)? {
                return Err(invariant(cx, "deleteProperty"));
            }
        }
        Ok(true)
    })
}

pub(crate) fn own_keys(cx: &mut Cx<'_>, proxy: ObjectId) -> Result<Vec<PropertyKey>, Throw> {
    let mark = cx.heap.roots().mark();
    let keys = scoped(cx, |cx| {
        let (target, handler, trap) = match dispatch(cx, proxy, "ownKeys", Some(256))? {
            Dispatch::Forward(target) => return object::own_property_keys(cx, target),
            Dispatch::Trap {
                target,
                handler,
                trap,
                ..
            } => (target, handler, trap),
        };
        let result = super::call(cx, trap, Value::object(handler), &[Value::object(target)])?;
        root(cx, result);
        // 10.5.11: CreateListFromArrayLike(result, property-key).
        let Some(array) = result.as_object() else {
            return Err(error::type_error(
                cx,
                "proxy: ownKeys must return an object",
            ));
        };
        let length = object::length_of_array_like(cx, array)?;
        let mut keys: Vec<PropertyKey> = Vec::new();
        for index in 0..length {
            let element_key = object::number_key(cx, index as f64)?;
            let element = super::property::get(cx, result, element_key)?;
            if element.as_string().is_none() && element.as_symbol().is_none() {
                return Err(error::type_error(
                    cx,
                    "proxy: properties must be strings or symbols",
                ));
            }
            root(cx, element);
            let key = ops::to_property_key(cx, element)?;
            object::root_keys(cx, &[key]);
            keys.push(key);
        }
        for (index, key) in keys.iter().enumerate().skip(1) {
            if keys[..index].contains(key) {
                return Err(error::type_error(cx, "proxy: duplicate property"));
            }
        }
        // Steps 9–19: every target key's descriptor first, then the checks.
        let extensible = object::is_extensible(cx, target)?;
        let target_keys = object::own_property_keys(cx, target)?;
        let mut nonconfigurable = Vec::new();
        let mut configurable = Vec::new();
        for key in target_keys {
            match object::get_own_property(cx, target, key)? {
                Some(descriptor) if descriptor.configurable == Some(false) => {
                    nonconfigurable.push(key);
                }
                _ => configurable.push(key),
            }
        }
        if extensible && nonconfigurable.is_empty() {
            return Ok(keys);
        }
        let mut unchecked = keys.clone();
        let mut take = |cx: &mut Cx<'_>, key: PropertyKey, message: &str| match unchecked
            .iter()
            .position(|&k| k == key)
        {
            Some(position) => {
                unchecked.remove(position);
                Ok(())
            }
            None => Err(error::type_error(cx, message)),
        };
        for key in nonconfigurable {
            take(
                cx,
                key,
                "proxy: target property must be present in proxy ownKeys",
            )?;
        }
        if extensible {
            return Ok(keys);
        }
        for key in configurable {
            take(
                cx,
                key,
                "proxy: target property must be present in proxy ownKeys",
            )?;
        }
        if !unchecked.is_empty() {
            return Err(error::type_error(
                cx,
                "proxy: property not present in target were returned by non extensible proxy",
            ));
        }
        Ok(keys)
    });
    // The scope cut the keys' roots; root them again for the caller.
    cx.heap.roots().truncate(mark);
    let keys = keys?;
    object::root_keys(cx, &keys);
    Ok(keys)
}

/// `[[Call]]` of a proxy: the `apply` trap with the arguments as an array.
/// A proxy made from a non-callable target has no `[[Call]]` (10.5.12):
/// calling it throws before any trap is read.
pub(crate) fn call(
    cx: &mut Cx<'_>,
    proxy: ObjectId,
    this: Value,
    args: &[Value],
) -> Result<Value, Throw> {
    if !cx.heap.header(proxy).has(flags::CALLABLE) {
        return Err(error::type_error(cx, "not a function"));
    }
    scoped(cx, |cx| {
        for &arg in args {
            root(cx, arg);
        }
        root(cx, this);
        let (target, handler, trap) = match dispatch(cx, proxy, "apply", Some(1024))? {
            Dispatch::Forward(target) => {
                return super::call(cx, Value::object(target), this, args);
            }
            Dispatch::Trap {
                target,
                handler,
                trap,
                ..
            } => (target, handler, trap),
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
/// an object. A proxy whose target is no constructor has no
/// `[[Construct]]` (10.5.13): `new` throws before any trap is read.
pub(crate) fn construct(
    cx: &mut Cx<'_>,
    proxy: ObjectId,
    args: &[Value],
    new_target: Value,
) -> Result<Value, Throw> {
    if !cx
        .heap
        .header(target(cx.heap, proxy))
        .has(flags::CONSTRUCTOR)
    {
        return Err(error::not_a_constructor(
            cx,
            Value::object(target(cx.heap, proxy)),
        ));
    }
    scoped(cx, |cx| {
        for &arg in args {
            root(cx, arg);
        }
        root(cx, new_target);
        let (target, handler, trap) = match dispatch(cx, proxy, "construct", Some(256))? {
            Dispatch::Forward(target) => {
                return super::construct(cx, Value::object(target), args, new_target);
            }
            Dispatch::Trap {
                target,
                handler,
                trap,
                ..
            } => (target, handler, trap),
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
    let callable = header.flags & (flags::CALLABLE | flags::CONSTRUCTOR);
    cx.heap
        .alloc_internal(
            ObjectKind::Proxy,
            crate::heap::object::InternalTag::None,
            None,
            0,
            &[target, handler, Value::bool(false)],
            Default::default(),
            callable,
        )
        .map_err(|error| object::heap_error(cx, error))
}
