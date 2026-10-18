//! Proxy exotic objects (phase 4, section 3.1): every internal method
//! through the handler's trap, with QuickJS's invariant checks against the
//! target (`js_proxy_*`, including the checks it leaves out). A proxy's
//! internal slot 0 is the target, slot 1 the handler and slot 2 whether it
//! is revoked: as in QuickJS, a revoked proxy keeps both, so an internal
//! method that was running when its trap revoked the proxy still has them.

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

/// QuickJS's `p->extensible` on a target: the object's own flag, which for a
/// proxy target is set (QuickJS reads it without calling `isExtensible`).
fn raw_extensible(heap: &Heap, object: ObjectId) -> bool {
    heap.object_kind(object) == ObjectKind::Proxy || heap.header(object).has(flags::EXTENSIBLE)
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
        proxy: ObjectId,
        target: ObjectId,
        handler: ObjectId,
        trap: Value,
    },
}

/// A check QuickJS makes on each proxy right after reading its trap.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Check {
    None,
    /// `js_proxy_call`: the proxy must have been made from a function.
    Callable,
    /// `js_proxy_call_constructor`: the target must be a constructor.
    Constructor,
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
    check: Check,
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
        match check {
            Check::None => {}
            Check::Callable => {
                if !cx.heap.header(proxy).has(flags::CALLABLE) {
                    return Err(error::type_error(cx, "not a function"));
                }
            }
            Check::Constructor => {
                if !cx.heap.header(target).has(flags::CONSTRUCTOR) {
                    return Err(error::not_a_constructor(cx, Value::object(target)));
                }
            }
        }
        if !trap.is_undefined() {
            root(cx, trap);
            return Ok(Dispatch::Trap {
                proxy,
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
        let (target, handler, trap) =
            match dispatch(cx, proxy, "getPrototypeOf", Some(128), Check::None)? {
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
        let (target, handler, trap) =
            match dispatch(cx, proxy, "setPrototypeOf", Some(64), Check::None)? {
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
        let (target, handler, trap) = match dispatch(cx, proxy, "isExtensible", None, Check::None)?
        {
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
        let (target, handler, trap) =
            match dispatch(cx, proxy, "preventExtensions", None, Check::None)? {
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

/// QuickJS's `check_define_prop_flags`: whether `descriptor` may redefine
/// `current`, comparing attributes only (no values, getters or setters).
fn check_define_flags(current: &Descriptor, descriptor: &Descriptor) -> bool {
    if current.configurable != Some(false) {
        return true;
    }
    if descriptor.configurable == Some(true) {
        return false;
    }
    if let Some(enumerable) = descriptor.enumerable
        && enumerable != current.enumerable.unwrap_or(false)
    {
        return false;
    }
    if !descriptor.is_generic() {
        let is_getset = current.is_accessor();
        if descriptor.is_accessor() != is_getset {
            return false;
        }
        if !is_getset && current.writable != Some(true) && descriptor.writable == Some(true) {
            return false;
        }
    }
    true
}

pub(crate) fn get_own_property(
    cx: &mut Cx<'_>,
    proxy: ObjectId,
    key: PropertyKey,
) -> Result<Option<Descriptor>, Throw> {
    let result = scoped(cx, |cx| {
        let (target, handler, trap) =
            match dispatch(cx, proxy, "getOwnPropertyDescriptor", None, Check::None)? {
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
        if result.is_undefined() {
            if let Some(current) = current
                && (current.configurable != Some(true) || !raw_extensible(cx.heap, target))
            {
                return Err(invariant(cx, "getOwnPropertyDescriptor"));
            }
            return Ok(None);
        }
        let extensible = object::is_extensible(cx, target)?;
        let descriptor = object::to_property_descriptor(cx, result)?;
        let complete = complete(&descriptor);
        match &current {
            Some(current) => {
                let mut flags = complete;
                if flags.is_accessor() {
                    flags.value = None;
                    flags.writable = None;
                }
                if !check_define_flags(current, &flags) {
                    return Err(invariant(cx, "getOwnPropertyDescriptor"));
                }
            }
            None => {
                if !extensible {
                    return Err(invariant(cx, "getOwnPropertyDescriptor"));
                }
            }
        }
        if complete.configurable == Some(false) {
            match &current {
                Some(current) if current.configurable == Some(false) => {
                    if !complete.is_accessor()
                        && complete.writable == Some(false)
                        && current.writable == Some(true)
                    {
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
        let (target, handler, trap) =
            match dispatch(cx, proxy, "defineProperty", None, Check::None)? {
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
        let current = object::get_own_property(cx, target, key)?;
        if let Some(current) = &current {
            current.root(cx);
        }
        let setting_non_configurable = descriptor.configurable == Some(false);
        let Some(current) = current else {
            if !raw_extensible(cx.heap, target) || setting_non_configurable {
                return Err(invariant(cx, "defineProperty"));
            }
            return Ok(true);
        };
        if !check_define_flags(&current, descriptor) {
            return Err(invariant(cx, "defineProperty"));
        }
        if current.configurable == Some(false) {
            let differs = |cx: &Cx<'_>, new: Option<Value>, old: Option<Value>| {
                new.is_some_and(|new| !object::same_value(cx, new, old.unwrap_or(Value::UNDEFINED)))
            };
            if current.is_accessor() {
                if differs(cx, descriptor.get, current.get)
                    || differs(cx, descriptor.set, current.set)
                {
                    return Err(invariant(cx, "defineProperty"));
                }
            } else if current.writable != Some(true) && differs(cx, descriptor.value, current.value)
            {
                return Err(invariant(cx, "defineProperty"));
            }
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
        let (target, handler, trap) = match dispatch(cx, proxy, "has", Some(128), Check::None)? {
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
        if !result
            && let Some(current) = object::get_own_property(cx, target, key)?
            && (current.configurable != Some(true) || !raw_extensible(cx.heap, target))
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
        let (target, handler, trap) = match dispatch(cx, proxy, "get", Some(512), Check::None)? {
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
        let (target, handler, trap) = match dispatch(cx, proxy, "set", Some(256), Check::None)? {
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
        let (target, handler, trap) =
            match dispatch(cx, proxy, "deleteProperty", Some(256), Check::None)? {
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
        let (proxy, target, handler, trap) =
            match dispatch(cx, proxy, "ownKeys", Some(256), Check::None)? {
                Dispatch::Forward(target) => return object::own_property_keys(cx, target),
                Dispatch::Trap {
                    proxy,
                    target,
                    handler,
                    trap,
                } => (proxy, target, handler, trap),
            };
        let result = super::call(cx, trap, Value::object(handler), &[Value::object(target)])?;
        root(cx, result);
        // QuickJS reads `length` and the elements of any value the trap
        // returns, and converts every element before checking duplicates.
        let length_key = crate::runtime::Names::key(cx.realm.names.length);
        let length = super::property::get(cx, result, length_key)?;
        let length = super::number::to_uint32(ops::to_number(cx, length)?);
        let mut keys: Vec<PropertyKey> = Vec::new();
        for index in 0..length {
            let element_key = object::index_key(cx, index)?;
            let element = super::property::get(cx, result, element_key)?;
            if element.as_string().is_none() && element.as_symbol().is_none() {
                return Err(error::type_error(
                    cx,
                    "proxy: properties must be strings or symbols",
                ));
            }
            root(cx, element);
            keys.push(ops::to_property_key(cx, element)?);
        }
        for (index, key) in keys.iter().enumerate().skip(1) {
            if keys[..index].contains(key) {
                return Err(error::type_error(cx, "proxy: duplicate property"));
            }
        }
        let extensible = object::is_extensible(cx, target)?;
        if is_revoked(cx.heap, proxy) {
            return Err(error::type_error(cx, "revoked proxy"));
        }
        let target_keys = object::own_property_keys(cx, target)?;
        let mut found = vec![false; keys.len()];
        for key in target_keys {
            if is_revoked(cx.heap, proxy) {
                return Err(error::type_error(cx, "revoked proxy"));
            }
            let Some(descriptor) = object::get_own_property(cx, target, key)? else {
                continue;
            };
            if descriptor.configurable != Some(true) || !extensible {
                let Some(position) = keys.iter().position(|&k| k == key) else {
                    return Err(error::type_error(
                        cx,
                        "proxy: target property must be present in proxy ownKeys",
                    ));
                };
                if !extensible {
                    found[position] = true;
                }
            }
        }
        if !extensible && found.contains(&false) {
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
/// QuickJS enters a proxy made from a non-callable target too, reading its
/// `apply` trap before reporting `not a function`.
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
        let (target, handler, trap) =
            match dispatch(cx, proxy, "apply", Some(1024), Check::Callable)? {
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
        let (target, handler, trap) =
            match dispatch(cx, proxy, "construct", Some(256), Check::Constructor)? {
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
