//! Property access: own lookup, the prototype walk, accessors, and the miss
//! paths that fill the inline caches of `vm::ic`.
//!
//! Index keys live in the element range while it stays dense (an index at
//! most a few times past the range); a far index becomes a named property
//! with an index key, and every read checks the elements first.
//!
//! An accessor property occupies two slots, the getter then the setter
//! (phase 3, section 3); the slow paths call them through the re-entrant
//! [`call`](super::call). No accessor is cached.

use super::error;
use super::ic::{IcEntry, IcKind, STRING_RECEIVER_TAG};
use super::{Cx, Throw};
use crate::heap::Heap;
use crate::heap::atom::{KeyKind, PropertyKey};
use crate::heap::object::{NONE, ObjectKind, flags};
use crate::runtime::{Intrinsic, Names};
use crate::shape::{PropertyFlags, STATIC_CELL};
use crate::value::{Kind, ObjectId, ValidityCellId, Value};

/// A property found by a lookup.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Found {
    Data(Value),
    /// The getter and the setter, `undefined` for an absent half.
    Accessor(Value, Value),
}

/// An own property.
#[inline]
pub(crate) fn own(heap: &Heap, names: &Names, object: ObjectId, key: PropertyKey) -> Option<Found> {
    match key.kind() {
        KeyKind::Index(index) => {
            let value = heap.get_element(object, index);
            if !value.is_empty() {
                return Some(Found::Data(value));
            }
            if let Some(cell) = super::arguments::mapped_cell(heap, object, index) {
                return Some(Found::Data(heap.scope_get(cell, 0, 0)));
            }
        }
        KeyKind::String(id) if id == names.length => {
            if heap.object_kind(object) == ObjectKind::Array {
                return Some(Found::Data(length_value(heap.array_length(object))));
            }
        }
        _ => {}
    }
    let (slot, property) = heap.lookup_own(object, key)?;
    Some(slot_value(heap, object, slot, property))
}

#[inline]
fn slot_value(heap: &Heap, object: ObjectId, slot: u32, property: PropertyFlags) -> Found {
    if property.has(PropertyFlags::ACCESSOR) {
        Found::Accessor(heap.get_slot(object, slot), heap.get_slot(object, slot + 1))
    } else {
        Found::Data(heap.get_slot(object, slot))
    }
}

/// `key` on `object` or its prototypes.
#[inline]
pub(crate) fn find(
    heap: &Heap,
    names: &Names,
    object: ObjectId,
    key: PropertyKey,
) -> Option<Found> {
    let mut current = object;
    loop {
        if let Some(found) = own(heap, names, current, key) {
            return Some(found);
        }
        current = heap.prototype_of(current)?;
    }
}

/// The value of a found property for `receiver`: a getter is called.
pub(crate) fn value_of(
    cx: &mut Cx<'_>,
    found: Option<Found>,
    receiver: Value,
) -> Result<Value, Throw> {
    match found {
        None => Ok(Value::UNDEFINED),
        Some(Found::Data(value)) => Ok(value),
        Some(Found::Accessor(getter, _)) => call_getter(cx, getter, receiver),
    }
}

/// Calls a getter with `receiver` as `this`; an absent getter reads
/// `undefined`. The getter is held by its holder's slot until its frame
/// names it.
#[inline(never)]
fn call_getter(cx: &mut Cx<'_>, getter: Value, receiver: Value) -> Result<Value, Throw> {
    if getter.is_undefined() {
        return Ok(Value::UNDEFINED);
    }
    super::call(cx, getter, receiver, &[])
}

/// `object[key]` with `object` as the receiver.
#[inline]
pub(crate) fn get_value(
    cx: &mut Cx<'_>,
    object: ObjectId,
    key: PropertyKey,
) -> Result<Value, Throw> {
    let found = find(cx.heap, &cx.realm.names, object, key);
    value_of(cx, found, Value::object(object))
}

pub(crate) fn length_value(length: u32) -> Value {
    match i32::try_from(length) {
        Ok(length) => Value::int32(length),
        Err(_) => Value::number(f64::from(length)),
    }
}

/// `receiver[key]` for any value.
#[inline(never)]
pub(crate) fn get(cx: &mut Cx<'_>, receiver: Value, key: PropertyKey) -> Result<Value, Throw> {
    let object = match receiver.kind() {
        Kind::Object => receiver.as_object().unwrap(),
        Kind::String => {
            let id = receiver.as_string().unwrap();
            let length = cx.heap.string(id).repr.len();
            match key.kind() {
                KeyKind::String(name) if name == cx.realm.names.length => {
                    return Ok(length_value(length as u32));
                }
                KeyKind::Index(index) if (index as usize) < length => {
                    let unit = cx.heap.string(id).repr.unit(index as usize);
                    let string = cx
                        .heap
                        .intern_units(&[unit])
                        .map_err(|_| error::oom(cx.realm))?;
                    return Ok(Value::string(string));
                }
                _ => cx.realm.intrinsic(Intrinsic::StringPrototype),
            }
        }
        Kind::Number | Kind::Int32 => cx.realm.intrinsic(Intrinsic::NumberPrototype),
        Kind::Bool => cx.realm.intrinsic(Intrinsic::BooleanPrototype),
        Kind::Undefined | Kind::Null => {
            let message = format!(
                "cannot read property '{}' of {}",
                key_text(cx.heap, key),
                if receiver.is_null() {
                    "null"
                } else {
                    "undefined"
                }
            );
            return Err(error::type_error(cx, &message));
        }
        _ => cx.realm.intrinsic(Intrinsic::ObjectPrototype),
    };
    let found = find(cx.heap, &cx.realm.names, object, key);
    value_of(cx, found, receiver)
}

/// `GetField`'s miss path (phase 3, section 2.2): a probe of a polymorphic
/// row, then the uncached read, and a fill of cache entry `site` when it ends
/// in a data property of an object in transition-tree mode, found on the
/// receiver itself, on its prototype chain, or on `%String.prototype%`'s
/// chain for a string receiver. Accessors, dictionary-mode receivers and
/// holders, index keys and complete misses leave the entry as it was.
/// `fill` is false for the reserved no-cache site.
#[inline(never)]
pub(crate) fn get_cached(
    cx: &mut Cx<'_>,
    receiver: Value,
    key: PropertyKey,
    site: usize,
    fill: bool,
) -> Result<Value, Throw> {
    if let Some(row) = cx.heap.ics.poly_row(site) {
        let heap = &*cx.heap;
        for &entry in row {
            let hit = super::ic::get_hit(&heap.objects, &heap.cells, &heap.values, entry, receiver);
            if let Some(value) = hit {
                cx.heap.ics.count_poly_hit(IcKind::Get);
                return Ok(value);
            }
        }
    }
    let names = cx.realm.names;
    let named = matches!(key.kind(), KeyKind::String(id) if id != names.length)
        || matches!(key.kind(), KeyKind::Symbol(_));
    let (start, string) = match receiver.kind() {
        Kind::Object if fill && !matches!(key.kind(), KeyKind::Index(_)) => {
            (receiver.as_object().unwrap(), false)
        }
        Kind::String if fill && named => (cx.realm.intrinsic(Intrinsic::StringPrototype), true),
        _ => {
            cx.heap.ics.count_miss(site, false);
            return get(cx, receiver, key);
        }
    };
    let heap = &*cx.heap;
    let mut holder = start;
    let walked = loop {
        if !named && heap.object_kind(holder) == ObjectKind::Array {
            break Err(Some(Found::Data(length_value(heap.array_length(holder)))));
        }
        if let Some((slot, property)) = heap.lookup_own(holder, key) {
            if property.has(PropertyFlags::ACCESSOR)
                || heap.is_dictionary(holder)
                || (!string && heap.is_dictionary(start))
            {
                break Err(Some(slot_value(heap, holder, slot, property)));
            }
            break Ok(slot);
        }
        match heap.prototype_of(holder) {
            Some(prototype) => holder = prototype,
            None => break Err(None),
        }
    };
    let slot = match walked {
        Ok(slot) => slot,
        Err(found) => {
            cx.heap.ics.count_miss(site, false);
            return value_of(cx, found, receiver);
        }
    };
    // The receiver is in a register and the holder on its chain (or the
    // chain of an intrinsic), so both survive the cell's allocation; the
    // value is read after it.
    let cell = if string {
        cx.heap.chain_cell(start)
    } else if holder == start {
        Ok(ValidityCellId(NONE))
    } else {
        cx.heap.validity_cell(start)
    };
    let heap = &mut *cx.heap;
    let mut filled = false;
    if let Ok(cell) = cell {
        let shape = if string {
            STRING_RECEIVER_TAG
        } else {
            heap.shape_of(start).index()
        };
        // A string entry always names its holder; an own entry names none.
        let holder = if holder == start && !string {
            NONE
        } else {
            holder.index()
        };
        filled = heap.ics.record(
            site,
            IcEntry {
                w0: shape,
                w1: holder,
                w2: slot,
                w3: cell.index(),
            },
        );
    }
    heap.ics.count_miss(site, filled);
    Ok(heap.get_slot(holder, slot))
}

/// `SetField`'s miss path (phase 3, section 2.3): a probe of a polymorphic
/// row, then the uncached write, and a fill when the write stored to an
/// existing own writable data property, or added a data property through a
/// transition of a non-dictionary, extensible object that is not a
/// prototype. Accessors, read-only properties and dictionary objects go
/// uncached.
#[inline(never)]
pub(crate) fn set_cached(
    cx: &mut Cx<'_>,
    receiver: Value,
    key: PropertyKey,
    value: Value,
    strict: bool,
    site: usize,
    fill: bool,
) -> Result<(), Throw> {
    if let Some(row) = cx.heap.ics.poly_row(site) {
        let row = *row;
        let heap = &mut *cx.heap;
        for entry in row {
            if super::ic::set_hit(
                &mut heap.objects,
                &heap.cells,
                &mut heap.values,
                entry,
                receiver,
                value,
            ) {
                heap.ics.count_poly_hit(IcKind::Set);
                return Ok(());
            }
        }
    }
    let names = cx.realm.names;
    let cacheable = fill
        && match key.kind() {
            KeyKind::String(id) => id != names.length,
            KeyKind::Symbol(_) => true,
            KeyKind::Index(_) => false,
        };
    let Some(object) = receiver.as_object().filter(|_| cacheable) else {
        cx.heap.ics.count_miss(site, false);
        return set(cx, receiver, key, value, strict);
    };
    let heap = &*cx.heap;
    let header = *heap.header(object);
    let before = header.shape;
    let plain = !header.has(flags::DICTIONARY);
    let own = heap.lookup_own(object, key);
    // An add is cacheable only if the write consults no setter and no
    // read-only property on the chain, decided before it runs: a setter may
    // itself add the key.
    let inherited_plain = own.is_none() && chain_allows_add(heap, object, key);
    set_on(cx, object, key, value, strict)?;
    let heap = &mut *cx.heap;
    let entry = match own {
        Some((slot, property)) if plain && is_plain_data(property) => Some(IcEntry {
            w0: before.index(),
            w1: before.index(),
            w2: slot,
            w3: STATIC_CELL.index(),
        }),
        None if plain
            && inherited_plain
            && header.flags & (flags::EXTENSIBLE | flags::USED_AS_PROTOTYPE)
                == flags::EXTENSIBLE =>
        {
            let after = heap.shape_of(object);
            match heap.lookup_own(object, key) {
                Some((slot, property))
                    if !heap.is_dictionary(object)
                        && property == PropertyFlags::DEFAULT
                        && heap.shape(after).parent == Some(before) =>
                {
                    // The object is in a register; the cell guards the
                    // chain the write consulted.
                    match heap.validity_cell(object) {
                        Ok(cell) => Some(IcEntry {
                            w0: before.index(),
                            w1: after.index(),
                            w2: slot,
                            w3: cell.index(),
                        }),
                        Err(_) => None,
                    }
                }
                _ => None,
            }
        }
        _ => None,
    };
    let heap = &mut *cx.heap;
    let filled = entry.is_some_and(|entry| heap.ics.record(site, entry));
    heap.ics.count_miss(site, filled);
    Ok(())
}

/// Whether no prototype of `object` has `key` as an accessor or a read-only
/// data property, so a write adds it to `object`.
fn chain_allows_add(heap: &Heap, object: ObjectId, key: PropertyKey) -> bool {
    let mut current = heap.prototype_of(object);
    while let Some(prototype) = current {
        if let Some((_, property)) = heap.lookup_own(prototype, key) {
            return is_plain_data(property);
        }
        current = heap.prototype_of(prototype);
    }
    true
}

fn is_plain_data(property: PropertyFlags) -> bool {
    property.has(PropertyFlags::WRITABLE) && !property.has(PropertyFlags::ACCESSOR)
}

/// Whether an element write at `index` stays in the dense range.
#[inline]
fn dense(heap: &Heap, object: ObjectId, index: u32) -> bool {
    let header = heap.header(object);
    let reach = header.elements().len.max(8).saturating_mul(4);
    index < reach
}

/// `receiver[key] = value`.
#[inline(never)]
pub(crate) fn set(
    cx: &mut Cx<'_>,
    receiver: Value,
    key: PropertyKey,
    value: Value,
    strict: bool,
) -> Result<(), Throw> {
    match receiver.kind() {
        Kind::Object => set_on(cx, receiver.as_object().unwrap(), key, value, strict),
        Kind::Undefined | Kind::Null => {
            let message = format!(
                "cannot set property '{}' of {}",
                key_text(cx.heap, key),
                if receiver.is_null() {
                    "null"
                } else {
                    "undefined"
                }
            );
            Err(error::type_error(cx, &message))
        }
        _ => {
            // A setter on the primitive's prototype chain still runs.
            let prototype = match receiver.kind() {
                Kind::String => cx.realm.intrinsic(Intrinsic::StringPrototype),
                Kind::Number | Kind::Int32 => cx.realm.intrinsic(Intrinsic::NumberPrototype),
                Kind::Bool => cx.realm.intrinsic(Intrinsic::BooleanPrototype),
                _ => cx.realm.intrinsic(Intrinsic::ObjectPrototype),
            };
            if let Some(Found::Accessor(_, setter)) = find(cx.heap, &cx.realm.names, prototype, key)
            {
                return call_setter(cx, setter, receiver, value, key, strict);
            }
            if strict {
                let message = format!("cannot create property '{}'", key_text(cx.heap, key));
                return Err(error::type_error(cx, &message));
            }
            Ok(())
        }
    }
}

#[inline(never)]
pub(crate) fn set_on(
    cx: &mut Cx<'_>,
    object: ObjectId,
    key: PropertyKey,
    value: Value,
    strict: bool,
) -> Result<(), Throw> {
    let heap = &mut *cx.heap;
    match key.kind() {
        KeyKind::Index(index) => {
            if let Some(cell) = super::arguments::mapped_cell(heap, object, index) {
                heap.scope_set(cell, 0, 0, value);
                return Ok(());
            }
            if heap.lookup_own(object, key).is_none() && dense(heap, object, index) {
                // An inherited setter or read-only index on a prototype is
                // phase 4's: elements are written directly.
                return heap
                    .set_element(object, index, value)
                    .map_err(|_| error::oom(cx.realm));
            }
        }
        KeyKind::String(id) if id == cx.realm.names.length => {
            if heap.object_kind(object) == ObjectKind::Array {
                return set_length(cx, object, value);
            }
        }
        _ => {}
    }
    if let Some((slot, property)) = heap.lookup_own(object, key) {
        if property.has(PropertyFlags::ACCESSOR) {
            let setter = heap.get_slot(object, slot + 1);
            return call_setter(cx, setter, Value::object(object), value, key, strict);
        }
        if property.has(PropertyFlags::WRITABLE) {
            heap.set_slot(object, slot, value);
            return Ok(());
        }
        return read_only(cx, key, strict);
    }
    // An inherited setter runs; an inherited read-only property blocks the
    // write.
    let mut current = heap.prototype_of(object);
    while let Some(prototype) = current {
        if let Some((slot, property)) = heap.lookup_own(prototype, key) {
            if property.has(PropertyFlags::ACCESSOR) {
                let setter = heap.get_slot(prototype, slot + 1);
                return call_setter(cx, setter, Value::object(object), value, key, strict);
            }
            if !property.has(PropertyFlags::WRITABLE) {
                return read_only(cx, key, strict);
            }
            break;
        }
        current = heap.prototype_of(prototype);
    }
    add(cx, object, key, value, strict)
}

/// Calls a setter with `receiver` as `this`; an absent setter is a
/// read-only property.
#[inline(never)]
fn call_setter(
    cx: &mut Cx<'_>,
    setter: Value,
    receiver: Value,
    value: Value,
    key: PropertyKey,
    strict: bool,
) -> Result<(), Throw> {
    if setter.is_undefined() {
        if strict {
            let message = format!("no setter for property '{}'", key_text(cx.heap, key));
            return Err(error::type_error(cx, &message));
        }
        return Ok(());
    }
    super::call(cx, setter, receiver, &[value]).map(|_| ())
}

fn read_only(cx: &mut Cx<'_>, key: PropertyKey, strict: bool) -> Result<(), Throw> {
    if strict {
        let message = format!("'{}' is read-only", key_text(cx.heap, key));
        Err(error::type_error(cx, &message))
    } else {
        Ok(())
    }
}

/// Adds an absent own property.
fn add(
    cx: &mut Cx<'_>,
    object: ObjectId,
    key: PropertyKey,
    value: Value,
    strict: bool,
) -> Result<(), Throw> {
    let heap = &mut *cx.heap;
    if !heap.header(object).has(flags::EXTENSIBLE) {
        if strict {
            return Err(error::type_error(cx, "object is not extensible"));
        }
        return Ok(());
    }
    heap.add_property(object, key, PropertyFlags::DEFAULT, value)
        .map_err(|_| error::oom(cx.realm))?;
    if let KeyKind::Index(index) = key.kind()
        && heap.object_kind(object) == ObjectKind::Array
        && index >= heap.array_length(object)
    {
        heap.set_array_length(object, index + 1);
    }
    Ok(())
}

/// Defines an own data property (object and array literals): overwrites an
/// existing data property in place and replaces an accessor.
#[inline(never)]
pub(crate) fn define(
    cx: &mut Cx<'_>,
    object: ObjectId,
    key: PropertyKey,
    value: Value,
) -> Result<(), Throw> {
    let heap = &mut *cx.heap;
    if let Some((slot, property)) = heap.lookup_own(object, key) {
        if !property.has(PropertyFlags::ACCESSOR) {
            heap.set_slot(object, slot, value);
            return Ok(());
        }
        let mark = heap.roots().mark();
        heap.roots().push(value);
        let result = heap
            .delete_property(object, key)
            .and_then(|_| heap.add_property(object, key, PropertyFlags::DEFAULT, value));
        heap.roots().truncate(mark);
        return result.map_err(|_| error::oom(cx.realm));
    }
    if let KeyKind::Index(index) = key.kind()
        && dense(heap, object, index)
    {
        return heap
            .set_element(object, index, value)
            .map_err(|_| error::oom(cx.realm));
    }
    add(cx, object, key, value, false)
}

/// Defines one half of an own accessor property (`get x() {}` and
/// `set x(v) {}` in a literal, and the builtins' accessors): the other half
/// of an existing accessor is kept; a data property is replaced.
#[inline(never)]
pub(crate) fn define_accessor(
    cx: &mut Cx<'_>,
    object: ObjectId,
    key: PropertyKey,
    function: Value,
    setter: bool,
    flags: PropertyFlags,
) -> Result<(), Throw> {
    let heap = &mut *cx.heap;
    if let Some((slot, property)) = heap.lookup_own(object, key)
        && property.has(PropertyFlags::ACCESSOR)
    {
        heap.set_slot(object, slot + u32::from(setter), function);
        return Ok(());
    }
    let mark = heap.roots().mark();
    heap.roots().push(function);
    let result = (|| {
        if heap.lookup_own(object, key).is_some() {
            heap.delete_property(object, key)?;
        }
        let (getter, setter) = if setter {
            (Value::UNDEFINED, function)
        } else {
            (function, Value::UNDEFINED)
        };
        heap.add_accessor(object, key, flags, getter, setter)
    })();
    heap.roots().truncate(mark);
    result.map_err(|_| error::oom(cx.realm))
}

/// `array.length = value`.
fn set_length(cx: &mut Cx<'_>, array: ObjectId, value: Value) -> Result<(), Throw> {
    let length = match value.as_int32() {
        Some(length) if length >= 0 => length as u32,
        _ => {
            let number = super::ops::to_number(cx, value)?;
            let length = super::number::to_uint32(number);
            if f64::from(length) != number {
                return Err(error::range_error(cx, "invalid array length"));
            }
            length
        }
    };
    let heap = &mut *cx.heap;
    let old = heap.array_length(array);
    heap.set_array_length(array, length);
    if length < old {
        // Far indices stored as named properties go too.
        let cut: Vec<PropertyKey> = heap
            .own_keys(array)
            .into_iter()
            .filter(|key| matches!(key.kind(), KeyKind::Index(index) if index >= length))
            .collect();
        for key in cut {
            heap.delete_property(array, key)
                .map_err(|_| error::oom(cx.realm))?;
        }
    }
    Ok(())
}

/// `delete receiver[key]`.
#[inline(never)]
pub(crate) fn delete(
    cx: &mut Cx<'_>,
    receiver: Value,
    key: PropertyKey,
    strict: bool,
) -> Result<bool, Throw> {
    let object = match receiver.kind() {
        Kind::Object => receiver.as_object().unwrap(),
        Kind::Undefined | Kind::Null => {
            let message = format!(
                "cannot delete property '{}' of {}",
                key_text(cx.heap, key),
                if receiver.is_null() {
                    "null"
                } else {
                    "undefined"
                }
            );
            return Err(error::type_error(cx, &message));
        }
        Kind::String => {
            let length = cx.heap.string(receiver.as_string().unwrap()).repr.len();
            let own = match key.kind() {
                KeyKind::String(id) => id == cx.realm.names.length,
                KeyKind::Index(index) => (index as usize) < length,
                KeyKind::Symbol(_) => false,
            };
            return non_configurable(cx, key, own, strict);
        }
        _ => return Ok(true),
    };
    let heap = &mut *cx.heap;
    match key.kind() {
        KeyKind::Index(index) => {
            if super::arguments::mapped_cell(heap, object, index).is_some() {
                super::arguments::unmap(heap, object, index);
                return Ok(true);
            }
            if !heap.get_element(object, index).is_empty() {
                heap.set_element(object, index, Value::EMPTY)
                    .map_err(|_| error::oom(cx.realm))?;
                return Ok(true);
            }
        }
        KeyKind::String(id) if id == cx.realm.names.length => {
            if heap.object_kind(object) == ObjectKind::Array {
                return non_configurable(cx, key, true, strict);
            }
        }
        _ => {}
    }
    match heap.lookup_own(object, key) {
        None => Ok(true),
        Some((_, property)) if !property.has(PropertyFlags::CONFIGURABLE) => {
            non_configurable(cx, key, true, strict)
        }
        Some(_) => {
            heap.delete_property(object, key)
                .map_err(|_| error::oom(cx.realm))?;
            Ok(true)
        }
    }
}

fn non_configurable(
    cx: &mut Cx<'_>,
    key: PropertyKey,
    present: bool,
    strict: bool,
) -> Result<bool, Throw> {
    if !present {
        return Ok(true);
    }
    if strict {
        let message = format!("could not delete property '{}'", key_text(cx.heap, key));
        return Err(error::type_error(cx, &message));
    }
    Ok(false)
}

/// `key in object`, along the prototype chain.
pub(crate) fn has(heap: &Heap, names: &Names, object: ObjectId, key: PropertyKey) -> bool {
    find(heap, names, object, key).is_some()
}

/// A key as text, for messages.
pub(crate) fn key_text(heap: &Heap, key: PropertyKey) -> String {
    match key.kind() {
        KeyKind::Index(index) => index.to_string(),
        KeyKind::String(id) => heap.string_text(id),
        KeyKind::Symbol(id) => match heap.symbol_description(id) {
            Some(description) => format!("Symbol({})", heap.string_text(description)),
            None => "Symbol()".into(),
        },
    }
}

// ---- for-in (phase 3, section 5.4) ----

/// `ForInStart`: an enumeration of `value`'s enumerable string keys in the
/// order of `EnumerateObjectProperties`: each object's own index keys
/// ascending, then its own named keys in insertion order, then its
/// prototype's, skipping keys seen at an earlier level (enumerable or not)
/// and symbols. A string's indices come first; `undefined` and `null`
/// enumerate nothing. Index keys are kept as `Int32` and named keys as their
/// atoms; `ForInNext` turns an index into a string as it yields it.
#[inline(never)]
pub(crate) fn for_in_start(cx: &mut Cx<'_>, value: Value) -> Result<Value, Throw> {
    let heap = &*cx.heap;
    let mut keys: Vec<Value> = Vec::new();
    let mut seen = crate::heap::hash::FxHashSet::default();
    let start = match value.kind() {
        Kind::Undefined | Kind::Null => None,
        Kind::Object => value.as_object(),
        Kind::String => {
            let length = heap.string(value.as_string().unwrap()).repr.len();
            for index in 0..length.min(i32::MAX as usize) {
                keys.push(Value::int32(index as i32));
                if let Some(key) = PropertyKey::index(index as u32) {
                    seen.insert(key);
                }
            }
            Some(cx.realm.intrinsic(Intrinsic::StringPrototype))
        }
        Kind::Number | Kind::Int32 => Some(cx.realm.intrinsic(Intrinsic::NumberPrototype)),
        Kind::Bool => Some(cx.realm.intrinsic(Intrinsic::BooleanPrototype)),
        _ => Some(cx.realm.intrinsic(Intrinsic::ObjectPrototype)),
    };
    let mut current = start;
    while let Some(object) = current {
        for key in own_keys_with_arguments(heap, object) {
            if !seen.insert(key) {
                continue;
            }
            let enumerable = match heap.lookup_own(object, key) {
                Some((_, property)) => property.has(PropertyFlags::ENUMERABLE),
                // Elements and mapped arguments are enumerable data.
                None => true,
            };
            if !enumerable {
                continue;
            }
            match key.kind() {
                KeyKind::Index(index) => keys.push(Value::int32(index as i32)),
                KeyKind::String(id) => keys.push(Value::string(id)),
                KeyKind::Symbol(_) => {}
            }
        }
        current = heap.prototype_of(object);
    }
    let count = keys.len() as u32;
    let iterator = cx
        .heap
        .alloc_object_with_elements(ObjectKind::ForInIterator, None, 2, count)
        .map_err(|error| error::heap_error(cx, error))?;
    // Keys are Int32 or atoms, and `value` is in a register: nothing to root.
    let heap = &mut *cx.heap;
    let elements = heap.header(iterator).elements();
    for (index, &key) in keys.iter().enumerate() {
        heap.values.set(elements, index as u32, key);
    }
    heap.set_raw_slot(iterator, 0, value);
    heap.set_raw_slot(iterator, 1, Value::int32(count as i32));
    heap.set_payload(iterator, 0);
    Ok(Value::object(iterator))
}

/// Own keys in `[[OwnPropertyKeys]]` order, with the mapped indices of an
/// `arguments` object, whose elements are holes there.
fn own_keys_with_arguments(heap: &Heap, object: ObjectId) -> Vec<PropertyKey> {
    let mut keys = heap.own_keys(object);
    if heap.object_kind(object) == ObjectKind::Arguments && heap.header(object).payload != NONE {
        let mut mapped: Vec<PropertyKey> = (0..heap.header(object).elements().len)
            .filter(|&index| super::arguments::mapped_cell(heap, object, index).is_some())
            .filter_map(PropertyKey::index)
            .collect();
        if !mapped.is_empty() {
            mapped.extend(keys);
            mapped.sort_by_key(|key| match key.kind() {
                KeyKind::Index(index) => (0, index),
                _ => (1, 0),
            });
            keys = mapped;
        }
    }
    keys
}

/// `ForInNext`: the next key of `iterator` whose property still exists (a
/// key deleted during the loop is skipped, as QuickJS does), as a string;
/// `None` when the enumeration is exhausted.
#[inline(never)]
pub(crate) fn for_in_next(cx: &mut Cx<'_>, iterator: Value) -> Result<Option<Value>, Throw> {
    let iterator = iterator.as_object().expect("a for-in enumeration");
    loop {
        let heap = &mut *cx.heap;
        let cursor = heap.header(iterator).payload;
        let count = heap.raw_slot(iterator, 1).as_int32().unwrap_or(0) as u32;
        if cursor >= count {
            return Ok(None);
        }
        heap.set_payload(iterator, cursor + 1);
        let key = heap.get_element(iterator, cursor);
        let object = heap.raw_slot(iterator, 0);
        let property_key = match key.as_int32() {
            Some(index) => PropertyKey::index(index as u32).expect("an index key"),
            None => PropertyKey::atom(key.as_string().expect("a key atom")),
        };
        if let Some(object) = object.as_object()
            && !has(heap, &cx.realm.names, object, property_key)
        {
            continue;
        }
        return match key.as_int32() {
            Some(_) => super::ops::to_string(cx, key).map(Some),
            None => Ok(Some(key)),
        };
    }
}
