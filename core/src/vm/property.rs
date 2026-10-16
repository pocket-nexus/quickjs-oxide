//! Property access: own lookup, the prototype walk, and the miss paths that
//! fill the inline caches of `vm::ic`. Phase 2 objects have data properties
//! only.
//!
//! Index keys live in the element range while it stays dense (an index at
//! most a few times past the range); a far index becomes a named property
//! with an index key, and every read checks the elements first.

use super::error;
use super::ic::{IcEntry, STRING_RECEIVER_TAG};
use super::{Cx, Throw};
use crate::heap::Heap;
use crate::heap::atom::{KeyKind, PropertyKey};
use crate::heap::object::{NONE, ObjectKind, flags};
use crate::runtime::{Intrinsic, Names};
use crate::shape::PropertyFlags;
use crate::value::{Kind, ObjectId, ValidityCellId, Value};

/// The value of `key` on `object` or its prototypes, `undefined` if absent.
#[inline]
pub(crate) fn get_from(heap: &Heap, names: &Names, object: ObjectId, key: PropertyKey) -> Value {
    let mut current = object;
    loop {
        if let Some(value) = own(heap, names, current, key) {
            return value;
        }
        match heap.prototype_of(current) {
            Some(prototype) => current = prototype,
            None => return Value::UNDEFINED,
        }
    }
}

/// An own property's value.
#[inline]
pub(crate) fn own(heap: &Heap, names: &Names, object: ObjectId, key: PropertyKey) -> Option<Value> {
    match key.kind() {
        KeyKind::Index(index) => {
            let value = heap.get_element(object, index);
            if !value.is_empty() {
                return Some(value);
            }
        }
        KeyKind::String(id) if id == names.length => {
            if heap.object_kind(object) == ObjectKind::Array {
                return Some(length_value(heap.array_length(object)));
            }
        }
        _ => {}
    }
    heap.lookup_own(object, key)
        .map(|(slot, _)| heap.get_slot(object, slot))
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
    Ok(get_from(cx.heap, &cx.realm.names, object, key))
}

/// `GetField`'s miss path (phase 3, section 2.2): the uncached read, and a
/// fill of cache entry `site` when it ends in a data property of an object
/// in transition-tree mode, found on the receiver itself, on its prototype
/// chain, or on `%String.prototype%`'s chain for a string receiver.
/// Accessors, dictionary-mode receivers and holders, index keys and complete
/// misses leave the entry as it was. `fill` is false for the reserved
/// no-cache site.
#[inline(never)]
pub(crate) fn get_cached(
    cx: &mut Cx<'_>,
    receiver: Value,
    key: PropertyKey,
    site: usize,
    fill: bool,
) -> Result<Value, Throw> {
    let names = cx.realm.names;
    let named = matches!(key.kind(), KeyKind::String(id) if id != names.length)
        || matches!(key.kind(), KeyKind::Symbol(_));
    let (start, string) = match receiver.kind() {
        Kind::Object if fill && !matches!(key.kind(), KeyKind::Index(_)) => {
            (receiver.as_object().unwrap(), false)
        }
        Kind::String if fill && named => (cx.realm.intrinsic(Intrinsic::StringPrototype), true),
        _ => return get(cx, receiver, key),
    };
    let heap = &*cx.heap;
    let mut holder = start;
    let (slot, property) = loop {
        if !named && heap.object_kind(holder) == ObjectKind::Array {
            return Ok(length_value(heap.array_length(holder)));
        }
        if let Some(found) = heap.lookup_own(holder, key) {
            break found;
        }
        match heap.prototype_of(holder) {
            Some(prototype) => holder = prototype,
            None => return Ok(Value::UNDEFINED),
        }
    };
    let value = heap.get_slot(holder, slot);
    if property.has(PropertyFlags::ACCESSOR)
        || heap.is_dictionary(holder)
        || (!string && heap.is_dictionary(start))
    {
        return Ok(value);
    }
    // The receiver is in a register and the holder on its chain (or the
    // chain of an intrinsic), so both survive the cell's allocation; the
    // value is read again after it.
    let cell = if string {
        cx.heap.chain_cell(start)
    } else if holder == start {
        Ok(ValidityCellId(NONE))
    } else {
        cx.heap.validity_cell(start)
    };
    let heap = &mut *cx.heap;
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
        heap.ics.set(
            site,
            IcEntry {
                w0: shape,
                w1: holder,
                w2: slot,
                w3: cell.index(),
            },
        );
    }
    Ok(heap.get_slot(holder, slot))
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
        _ if strict => {
            let message = format!("cannot create property '{}'", key_text(cx.heap, key));
            Err(error::type_error(cx, &message))
        }
        _ => Ok(()),
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
            if heap.lookup_own(object, key).is_none() && dense(heap, object, index) {
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
        if property.has(PropertyFlags::WRITABLE) {
            heap.set_slot(object, slot, value);
            return Ok(());
        }
        return read_only(cx, key, strict);
    }
    // An inherited read-only property blocks the write.
    let mut current = heap.prototype_of(object);
    while let Some(prototype) = current {
        if let Some((_, property)) = heap.lookup_own(prototype, key) {
            if !property.has(PropertyFlags::WRITABLE) {
                return read_only(cx, key, strict);
            }
            break;
        }
        current = heap.prototype_of(prototype);
    }
    add(cx, object, key, value, strict)
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
/// existing one in place.
#[inline(never)]
pub(crate) fn define(
    cx: &mut Cx<'_>,
    object: ObjectId,
    key: PropertyKey,
    value: Value,
) -> Result<(), Throw> {
    let heap = &mut *cx.heap;
    if let Some((slot, _)) = heap.lookup_own(object, key) {
        heap.set_slot(object, slot, value);
        return Ok(());
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
    let mut current = object;
    loop {
        if own(heap, names, current, key).is_some() {
            return true;
        }
        match heap.prototype_of(current) {
            Some(prototype) => current = prototype,
            None => return false,
        }
    }
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
