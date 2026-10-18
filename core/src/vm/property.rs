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

/// The outcome of a heap-only walk of a prototype chain.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Lookup {
    Hit(Found),
    Miss,
    /// The walk met an exotic object (section 3.1): the lookup continues
    /// through `vm::object`'s internal methods from this holder.
    Exotic(ObjectId),
}

/// `key` on `object` or its prototypes, as far as the heap alone answers.
#[inline]
pub(crate) fn lookup(heap: &Heap, names: &Names, object: ObjectId, key: PropertyKey) -> Lookup {
    let mut current = object;
    loop {
        if heap.object_kind(current).is_exotic() {
            return Lookup::Exotic(current);
        }
        if let Some(found) = own(heap, names, current, key) {
            return Lookup::Hit(found);
        }
        match heap.prototype_of(current) {
            Some(prototype) => current = prototype,
            None => return Lookup::Miss,
        }
    }
}

/// QuickJS installs `%RegExp.prototype%.exec` as an AutoInit property that
/// a read materializes (see `Realm::regexp_exec_read`): records a read of
/// `key` (`exec`) from `object` that resolves to that property.
#[cold]
#[inline(never)]
pub(crate) fn note_exec_read(cx: &Cx<'_>, object: ObjectId, key: PropertyKey) {
    let prototype = cx.realm.intrinsic(Intrinsic::RegExpPrototype);
    let mut current = object;
    loop {
        if cx.heap.object_kind(current).is_exotic() {
            return;
        }
        if cx.heap.lookup_own(current, key).is_some() {
            if current == prototype {
                cx.realm.regexp_exec_read.set(true);
            }
            return;
        }
        match cx.heap.prototype_of(current) {
            Some(next) => current = next,
            None => return,
        }
    }
}

/// `[[Get]]` of `key` on `object` for `receiver`.
#[inline]
fn get_from(
    cx: &mut Cx<'_>,
    object: ObjectId,
    key: PropertyKey,
    receiver: Value,
) -> Result<Value, Throw> {
    if !cx.realm.regexp_exec_read.get() && key == Names::key(cx.realm.names.exec) {
        note_exec_read(cx, object, key);
    }
    match lookup(cx.heap, &cx.realm.names, object, key) {
        Lookup::Hit(found) => value_of(cx, Some(found), receiver),
        Lookup::Miss => Ok(Value::UNDEFINED),
        Lookup::Exotic(holder) => super::object::get(cx, holder, key, receiver),
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
    get_from(cx, object, key, Value::object(object))
}

pub(crate) fn length_value(length: u32) -> Value {
    match i32::try_from(length) {
        Ok(length) => Value::int32(length),
        Err(_) => Value::number(f64::from(length)),
    }
}

/// `receiver[key]` for a key not yet converted: a `null` or `undefined`
/// receiver throws before the key's conversion runs any JavaScript, without
/// naming the key (QuickJS's computed-read message).
#[inline(never)]
pub(crate) fn get_computed(cx: &mut Cx<'_>, receiver: Value, key: Value) -> Result<Value, Throw> {
    if receiver.is_null() || receiver.is_undefined() {
        let base = if receiver.is_null() {
            "null"
        } else {
            "undefined"
        };
        return Err(error::type_error(
            cx,
            &format!("cannot read property of {base}"),
        ));
    }
    let key = super::ops::to_property_key(cx, key)?;
    get(cx, receiver, key)
}

/// `receiver[key]` for any value.
#[inline(never)]
pub(crate) fn get(cx: &mut Cx<'_>, receiver: Value, key: PropertyKey) -> Result<Value, Throw> {
    let object = match receiver.kind() {
        Kind::Object => receiver.as_object().unwrap(),
        Kind::String => {
            let id = receiver.as_string().unwrap();
            let length = cx.heap.string_len(id);
            match key.kind() {
                KeyKind::String(name) if name == cx.realm.names.length => {
                    return Ok(length_value(length as u32));
                }
                KeyKind::Index(index) if (index as usize) < length => {
                    let unit = cx.heap.string(id).repr().unit(index as usize);
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
        Kind::Symbol => cx.realm.intrinsic(Intrinsic::SymbolPrototype),
        Kind::BigInt => cx.realm.intrinsic(Intrinsic::BigIntPrototype),
        Kind::Undefined | Kind::Null => {
            let message = format!(
                "cannot read property '{}' of {}",
                atom_text(cx.heap, key),
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
    get_from(cx, object, key, receiver)
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
    if cx.heap.ics.is_megamorphic(site)
        && let Some(object) = receiver.as_object()
    {
        let shape = cx.heap.shape_of(object).index();
        if let Some(entry) = cx.heap.ics.mega_entry(shape, key.raw()) {
            let heap = &*cx.heap;
            let hit = super::ic::get_hit(
                &heap.objects,
                &heap.cells,
                &heap.values,
                &heap.strings,
                entry,
                receiver,
            );
            if let Some(value) = hit {
                cx.heap.ics.count_poly_hit(IcKind::Get);
                return Ok(value);
            }
        }
    }
    if let Some(row) = cx.heap.ics.poly_row(site) {
        let heap = &*cx.heap;
        for &entry in row {
            let hit = super::ic::get_hit(
                &heap.objects,
                &heap.cells,
                &heap.values,
                &heap.strings,
                entry,
                receiver,
            );
            if let Some(value) = hit {
                cx.heap.ics.count_poly_hit(IcKind::Get);
                return Ok(value);
            }
        }
    }
    let names = cx.realm.names;
    let named = matches!(key.kind(), KeyKind::String(id) if id != names.length)
        || matches!(key.kind(), KeyKind::Symbol(_));
    // An array's or a string's `length` caches as the `LENGTH_TAG` entry.
    if fill && matches!(key.kind(), KeyKind::String(id) if id == names.length) {
        let length = match receiver.kind() {
            Kind::String => Some(cx.heap.string_len(receiver.as_string().unwrap())),
            Kind::Object
                if cx.heap.object_kind(receiver.as_object().unwrap()) == ObjectKind::Array =>
            {
                Some(cx.heap.array_length(receiver.as_object().unwrap()) as usize)
            }
            _ => None,
        };
        if let Some(length) = length {
            let entry = IcEntry {
                w0: super::ic::LENGTH_TAG,
                ..IcEntry::EMPTY
            };
            let filled = cx.heap.ics.record(site, entry);
            cx.heap.ics.count_miss(site, filled);
            return Ok(length_value(length as u32));
        }
    }
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
        if heap.object_kind(holder).is_exotic() {
            cx.heap.ics.count_miss(site, false);
            return get(cx, receiver, key);
        }
        if !named && heap.object_kind(holder) == ObjectKind::Array {
            break Err(Some(Found::Data(length_value(heap.array_length(holder)))));
        }
        if let Some((slot, property)) = heap.lookup_own(holder, key) {
            // A dictionary-mode prototype is cacheable: any change to it
            // clears the cell like any prototype's. A dictionary receiver is
            // not (its shape does not change with its properties).
            if property.has(PropertyFlags::ACCESSOR)
                || (heap.is_dictionary(holder) && holder == start)
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
    if !cx.realm.regexp_exec_read.get()
        && holder == cx.realm.intrinsic(Intrinsic::RegExpPrototype)
        && key == Names::key(names.exec)
    {
        cx.realm.regexp_exec_read.set(true);
    }
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
        let fill = IcEntry {
            w0: shape,
            w1: holder,
            w2: slot,
            w3: cell.index(),
        };
        if heap.ics.is_megamorphic(site) && !string {
            heap.ics.record_mega(key.raw(), fill);
            filled = true;
        } else {
            filled = heap.ics.record(site, fill);
        }
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

/// Whether an element write at `index` stays in the dense range: the object
/// is not sparse and the index is under 1,024 or at most four times, and at
/// most 64 Ki elements, past its range (so a fill from the top index down
/// stays dense, and writes at doubling indices go sparse instead of growing
/// the range without bound).
#[inline]
pub(crate) fn dense_index(heap: &Heap, object: ObjectId, index: u32) -> bool {
    let header = heap.header(object);
    if header.has(flags::SPARSE) || !header.kind.has_elements() {
        return false;
    }
    let len = header.elements().len;
    let reach = len
        .max(256)
        .saturating_mul(4)
        .min(len.saturating_add(1 << 16));
    index < reach
}

#[inline]
fn dense(heap: &Heap, object: ObjectId, index: u32) -> bool {
    dense_index(heap, object, index)
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
                atom_text(cx.heap, key),
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
                Kind::String => {
                    // A string's own indices and `length` are read-only.
                    let length = cx.heap.string_len(receiver.as_string().unwrap());
                    let own = match key.kind() {
                        KeyKind::Index(index) => (index as usize) < length,
                        KeyKind::String(id) => id == cx.realm.names.length,
                        KeyKind::Symbol(_) => false,
                    };
                    if own {
                        if strict && matches!(key.kind(), KeyKind::Index(_)) {
                            return Err(error::type_error(cx, "not an object"));
                        }
                        return read_only(cx, key, strict);
                    }
                    cx.realm.intrinsic(Intrinsic::StringPrototype)
                }
                Kind::Number | Kind::Int32 => cx.realm.intrinsic(Intrinsic::NumberPrototype),
                Kind::Bool => cx.realm.intrinsic(Intrinsic::BooleanPrototype),
                Kind::Symbol => cx.realm.intrinsic(Intrinsic::SymbolPrototype),
                Kind::BigInt => cx.realm.intrinsic(Intrinsic::BigIntPrototype),
                _ => cx.realm.intrinsic(Intrinsic::ObjectPrototype),
            };
            match lookup(cx.heap, &cx.realm.names, prototype, key) {
                Lookup::Hit(Found::Accessor(_, setter)) => {
                    return call_setter(cx, setter, receiver, value, strict);
                }
                Lookup::Exotic(holder) => {
                    let done = super::object::set(cx, holder, key, value, receiver)?;
                    if done || !strict {
                        return Ok(());
                    }
                }
                _ => {}
            }
            if strict {
                return Err(primitive_set_error(cx, prototype, key));
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
    let header = *heap.header(object);
    if header.kind.is_exotic() {
        return generic_set(cx, object, key, value, strict);
    }
    match key.kind() {
        KeyKind::Index(index) => {
            if let Some(cell) = super::arguments::mapped_cell(heap, object, index) {
                let writable = heap
                    .lookup_own(object, key)
                    .is_none_or(|(_, flags)| flags.has(PropertyFlags::WRITABLE));
                if writable {
                    heap.scope_set(cell, 0, 0, value);
                    return Ok(());
                }
                return read_only(cx, key, strict);
            }
            if !heap.get_element(object, index).is_empty() {
                // An existing element is writable data.
                return heap
                    .set_element(object, index, value)
                    .map_err(|_| error::oom(cx.realm));
            }
            // A new element: no inherited setter or read-only index may
            // intervene, the object must be extensible, and an array's
            // length writable past its end.
            if heap.lookup_own(object, key).is_none()
                && dense(heap, object, index)
                && header.has(flags::EXTENSIBLE)
                && heap.holes_writable()
                && !(header.kind == ObjectKind::Array
                    && header.has(flags::LENGTH_READONLY)
                    && index >= header.payload)
            {
                return heap
                    .set_element(object, index, value)
                    .map_err(|_| error::oom(cx.realm));
            }
        }
        KeyKind::String(id) if id == cx.realm.names.length => {
            if header.kind == ObjectKind::Array {
                // `OrdinarySet` fails on a read-only `length` before any
                // conversion; a writable one goes through `ArraySetLength`.
                if header.has(flags::LENGTH_READONLY) {
                    return read_only(cx, key, strict);
                }
                let descriptor = super::object::Descriptor {
                    value: Some(value),
                    ..Default::default()
                };
                if !super::object::array_set_length(cx, object, &descriptor)? {
                    return read_only(cx, key, strict);
                }
                return Ok(());
            }
        }
        _ => {}
    }
    if let Some((slot, property)) = heap.lookup_own(object, key) {
        if property.has(PropertyFlags::ACCESSOR) {
            let setter = heap.get_slot(object, slot + 1);
            return call_setter(cx, setter, Value::object(object), value, strict);
        }
        if property.has(PropertyFlags::WRITABLE) {
            heap.set_slot(object, slot, value);
            return Ok(());
        }
        return read_only(cx, key, strict);
    }
    // An inherited setter runs; an inherited read-only property blocks the
    // write; an exotic prototype decides itself.
    let mut current = heap.prototype_of(object);
    while let Some(prototype) = current {
        let kind = heap.object_kind(prototype);
        if kind.is_exotic() {
            return generic_set(cx, object, key, value, strict);
        }
        let inherited = match key.kind() {
            KeyKind::Index(index) if !heap.get_element(prototype, index).is_empty() => {
                Some(PropertyFlags::DEFAULT)
            }
            KeyKind::Index(index)
                if super::arguments::mapped_cell(heap, prototype, index).is_some() =>
            {
                Some(PropertyFlags::DEFAULT)
            }
            KeyKind::String(id) if id == cx.realm.names.length && kind == ObjectKind::Array => {
                let readonly = heap.header(prototype).has(flags::LENGTH_READONLY);
                Some(PropertyFlags::data(!readonly, false, false))
            }
            _ => match heap.lookup_own(prototype, key) {
                Some((slot, property)) if property.has(PropertyFlags::ACCESSOR) => {
                    let setter = heap.get_slot(prototype, slot + 1);
                    return call_setter(cx, setter, Value::object(object), value, strict);
                }
                found => found.map(|(_, property)| property),
            },
        };
        if let Some(property) = inherited {
            if !property.has(PropertyFlags::WRITABLE) {
                return read_only(cx, key, strict);
            }
            break;
        }
        current = heap.prototype_of(prototype);
    }
    add(cx, object, key, value, strict)
}

/// `[[Set]]` through the internal methods, for receivers and chains the
/// heap walk above does not model.
#[inline(never)]
fn generic_set(
    cx: &mut Cx<'_>,
    object: ObjectId,
    key: PropertyKey,
    value: Value,
    strict: bool,
) -> Result<(), Throw> {
    let done = super::object::set(cx, object, key, value, Value::object(object))?;
    if !done && strict {
        return Err(set_failed(cx, object, key));
    }
    Ok(())
}

/// A strict `[[Set]]` on `object` that returned false: QuickJS's
/// `proxy: cannot set property` when a proxy's `set` trap said no, `object
/// is not extensible` when no object on the chain has the property and the
/// first cannot take it, else `'{key}' is read-only`. A proxy without the
/// trap forwarded to its target, which gave the reason.
#[cold]
#[inline(never)]
pub(crate) fn set_failed(cx: &mut Cx<'_>, object: ObjectId, key: PropertyKey) -> Throw {
    let mut current = Some(object);
    let mut first = None;
    while let Some(mut holder) = current {
        let kind = cx.heap.object_kind(holder);
        if kind == ObjectKind::Proxy {
            if cx.heap.vm.proxy_refused {
                return error::type_error(cx, "proxy: cannot set property");
            }
            while cx.heap.object_kind(holder) == ObjectKind::Proxy {
                holder = super::proxy::target(cx.heap, holder);
            }
            current = Some(holder);
            continue;
        }
        first.get_or_insert(holder);
        if kind.is_exotic() || cx.heap.lookup_own(holder, key).is_some() {
            break;
        }
        current = cx.heap.prototype_of(holder);
    }
    if current.is_none()
        && let Some(first) = first
        && !cx.heap.header(first).has(flags::EXTENSIBLE)
    {
        return error::type_error(cx, "object is not extensible");
    }
    let message = error::atom_message(cx.heap, "'", key, "' is read-only");
    error::throw_units(cx, error::ErrorKind::Type, &message)
}

/// A strict write of a property a primitive's prototype chain (from
/// `prototype`) neither sets nor holds read-only: QuickJS's `not an
/// object`, or `'{key}' is read-only` for an inherited read-only property.
#[cold]
#[inline(never)]
fn primitive_set_error(cx: &mut Cx<'_>, prototype: ObjectId, key: PropertyKey) -> Throw {
    let mut current = Some(prototype);
    while let Some(holder) = current {
        if cx.heap.object_kind(holder).is_exotic() {
            break;
        }
        if let Some((_, property)) = cx.heap.lookup_own(holder, key) {
            if !property.has(PropertyFlags::ACCESSOR) && !property.has(PropertyFlags::WRITABLE) {
                let message = error::atom_message(cx.heap, "'", key, "' is read-only");
                return error::throw_units(cx, error::ErrorKind::Type, &message);
            }
            break;
        }
        current = cx.heap.prototype_of(holder);
    }
    error::type_error(cx, "not an object")
}

/// Calls a setter with `receiver` as `this`; an absent setter is a
/// read-only property.
#[inline(never)]
fn call_setter(
    cx: &mut Cx<'_>,
    setter: Value,
    receiver: Value,
    value: Value,
    strict: bool,
) -> Result<(), Throw> {
    if setter.is_undefined() {
        if strict {
            return Err(error::type_error(cx, "no setter for property"));
        }
        return Ok(());
    }
    super::call(cx, setter, receiver, &[value]).map(|_| ())
}

fn read_only(cx: &mut Cx<'_>, key: PropertyKey, strict: bool) -> Result<(), Throw> {
    if strict {
        let message = error::atom_message(cx.heap, "'", key, "' is read-only");
        Err(error::throw_units(cx, error::ErrorKind::Type, &message))
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
    if super::object::array_index(heap, key).is_some() {
        let done = super::object::create_data_property(cx, object, key, value)?;
        if !done && strict {
            return Err(error::type_error(cx, "'length' is read-only"));
        }
        return Ok(());
    }
    heap.add_property(object, key, PropertyFlags::DEFAULT, value)
        .map_err(|_| error::oom(cx.realm))?;
    Ok(())
}

/// `CreateDataPropertyOrThrow` for object and array literals and class
/// fields: an ordinary extensible object's own writable, enumerable,
/// configurable data property is overwritten in place.
#[inline(never)]
pub(crate) fn define(
    cx: &mut Cx<'_>,
    object: ObjectId,
    key: PropertyKey,
    value: Value,
) -> Result<(), Throw> {
    let heap = &mut *cx.heap;
    let header = heap.header(object);
    if header.kind.is_exotic() || !header.has(flags::EXTENSIBLE) {
        return define_slow(cx, object, key, value);
    }
    if let Some((slot, property)) = heap.lookup_own(object, key) {
        if property == PropertyFlags::DEFAULT {
            heap.set_slot(object, slot, value);
            return Ok(());
        }
        return define_slow(cx, object, key, value);
    }
    if let KeyKind::Index(index) = key.kind()
        && dense(heap, object, index)
    {
        return heap
            .set_element(object, index, value)
            .map_err(|_| error::oom(cx.realm));
    }
    add(cx, object, key, value, true)
}

/// [`define`] on an exotic or non-extensible object, or over a property it
/// cannot simply overwrite: a class field on a proxy, a frozen object or a
/// constructor's `prototype`.
#[inline(never)]
fn define_slow(
    cx: &mut Cx<'_>,
    object: ObjectId,
    key: PropertyKey,
    value: Value,
) -> Result<(), Throw> {
    super::object::create_data_property_or_throw(cx, object, key, value)
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
            return Err(error::type_error(cx, "cannot convert to object"));
        }
        Kind::String => {
            let length = cx.heap.string_len(receiver.as_string().unwrap());
            let own = match key.kind() {
                KeyKind::String(id) => id == cx.realm.names.length,
                KeyKind::Index(index) => (index as usize) < length,
                KeyKind::Symbol(_) => false,
            };
            return non_configurable(cx, key, own, strict);
        }
        _ => return Ok(true),
    };
    if cx.heap.object_kind(object).is_exotic() {
        let done = super::object::delete(cx, object, key)?;
        return non_configurable(cx, key, !done, strict).map(|_| done);
    }
    let heap = &mut *cx.heap;
    match key.kind() {
        KeyKind::Index(index) => {
            if super::arguments::mapped_cell(heap, object, index).is_some() {
                let done = super::object::ordinary_delete(cx, object, key)?;
                return non_configurable(cx, key, !done, strict).map(|_| done);
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
    _key: PropertyKey,
    present: bool,
    strict: bool,
) -> Result<bool, Throw> {
    if !present {
        return Ok(true);
    }
    if strict {
        return Err(error::type_error(cx, "could not delete property"));
    }
    Ok(false)
}

/// `key in object`, along the prototype chain.
pub(crate) fn has(cx: &mut Cx<'_>, object: ObjectId, key: PropertyKey) -> Result<bool, Throw> {
    match lookup(cx.heap, &cx.realm.names, object, key) {
        Lookup::Hit(_) => Ok(true),
        Lookup::Miss => Ok(false),
        Lookup::Exotic(holder) => super::object::has_property(cx, holder, key),
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

/// `key` as QuickJS's `JS_AtomGetStr` renders it in an error message: a
/// symbol is its bare description (a private name `#x`), an index decimal,
/// and a string with a non-ASCII unit is cut once 58 bytes of UTF-8 are
/// written (a 64-byte buffer less one 6-byte character).
#[cold]
#[inline(never)]
pub(crate) fn atom_text(heap: &Heap, key: PropertyKey) -> String {
    match key.kind() {
        KeyKind::Index(index) => index.to_string(),
        KeyKind::String(id) => string_atom_text(heap, id),
        KeyKind::Symbol(id) => match heap.symbol_description(id) {
            Some(description) => string_atom_text(heap, description),
            None => String::new(),
        },
    }
}

/// A string as [`atom_text`] renders it.
#[cold]
#[inline(never)]
pub(crate) fn string_atom_text(heap: &Heap, id: crate::value::StringId) -> String {
    let units: Vec<u16> = heap.string(id).repr().units().collect();
    units_atom_text(&units)
}

/// Text as [`atom_text`] renders it.
#[cold]
#[inline(never)]
pub(crate) fn str_atom_text(text: &str) -> String {
    let units: Vec<u16> = text.encode_utf16().collect();
    units_atom_text(&units)
}

fn units_atom_text(units: &[u16]) -> String {
    if units.iter().all(|&unit| unit < 0x80) {
        return String::from_utf16_lossy(units);
    }
    let mut written = 0;
    let mut end = units.len();
    for (index, &unit) in units.iter().enumerate() {
        if written >= 64 - 6 {
            end = index;
            break;
        }
        written += match unit {
            0..0x80 => 1,
            0x80..0x800 => 2,
            _ => 3,
        };
    }
    String::from_utf16_lossy(&units[..end])
}

// ---- for-in (phase 3, section 5.4) ----

/// `ForInStart`: QuickJS's `build_for_in_iterator`. The enumeration keeps
/// the object of the level it walks (slot 0), its key count (slot 1), the
/// set of keys seen once it reaches the prototype chain (slot 2, an object
/// with a null prototype) and its mode (slot 3, [`FOR_IN_KEYS`],
/// [`FOR_IN_ARRAY`] or [`FOR_IN_CHAIN`]); the cursor is the payload. The
/// keys of the level are elements in pairs: the key (an `Int32` index or an
/// atom) and whether it is enumerable.
///
/// A fast array without enumerable named properties records only its
/// element count. Any other object snapshots its own string keys, the
/// non-enumerable ones too. The prototypes are read when the own keys run
/// out ([`for_in_next`]); `undefined` and `null` enumerate nothing, and a
/// primitive enumerates its wrapper.
#[inline(never)]
pub(crate) fn for_in_start(cx: &mut Cx<'_>, value: Value) -> Result<Value, Throw> {
    let object = match value.kind() {
        Kind::Undefined | Kind::Null => None,
        Kind::Object => value.as_object(),
        _ => Some(super::object::to_object(cx, value)?),
    };
    let mark = cx.heap.roots().mark();
    let result = (|| {
        let (mode, count, keys) = match object {
            None => (FOR_IN_KEYS, 0, Vec::new()),
            Some(object) => {
                cx.heap.roots().push(Value::object(object));
                match fast_array_count(cx, object) {
                    Some(count) => (FOR_IN_ARRAY, count, Vec::new()),
                    None => {
                        let keys = level_keys(cx, object)?;
                        (FOR_IN_KEYS, keys.len() as u32, keys)
                    }
                }
            }
        };
        let iterator = cx
            .heap
            .alloc_object_with_elements(ObjectKind::ForInIterator, None, 4, 2 * keys.len() as u32)
            .map_err(|error| error::heap_error(cx, error))?;
        // Keys are Int32 or atoms, and the object is rooted: nothing to root.
        let heap = &mut *cx.heap;
        let elements = heap.header(iterator).elements();
        for (index, &(key, enumerable)) in keys.iter().enumerate() {
            heap.values.set(elements, 2 * index as u32, key.to_value());
            heap.values
                .set(elements, 2 * index as u32 + 1, Value::bool(enumerable));
        }
        let level = object.map_or(Value::UNDEFINED, Value::object);
        heap.set_raw_slot(iterator, 0, level);
        heap.set_raw_slot(iterator, 1, Value::int32(count as i32));
        heap.set_raw_slot(iterator, 2, Value::UNDEFINED);
        heap.set_raw_slot(iterator, 3, Value::int32(mode));
        heap.set_payload(iterator, 0);
        Ok(Value::object(iterator))
    })();
    cx.heap.roots().truncate(mark);
    result
}

/// A for-in enumeration of an own key snapshot.
const FOR_IN_KEYS: i32 = 0;
/// A for-in enumeration of a fast array's element indices.
const FOR_IN_ARRAY: i32 = 1;
/// A for-in enumeration in the prototype chain.
const FOR_IN_CHAIN: i32 = 2;

/// The element count of `object` when QuickJS enumerates it as a fast array
/// (an `Array` or `arguments` object in its dense form, with no enumerable
/// named property).
fn fast_array_count(cx: &Cx<'_>, object: ObjectId) -> Option<u32> {
    let heap = &*cx.heap;
    let kind = heap.object_kind(object);
    if !matches!(kind, ObjectKind::Array | ObjectKind::Arguments)
        || heap.header(object).has(crate::heap::object::flags::SPARSE)
        || heap.holey.contains(&object.0)
    {
        return None;
    }
    for key in heap.own_keys(object) {
        if let Some((_, flags)) = heap.lookup_own(object, key)
            && flags.has(PropertyFlags::ENUMERABLE)
            && !matches!(key.kind(), KeyKind::Symbol(_))
        {
            return None;
        }
    }
    let elements = heap.header(object).elements().len;
    let length = if kind == ObjectKind::Array {
        heap.array_length(object).min(elements)
    } else {
        elements
    };
    let mut count = 0;
    while count < length
        && (!heap.get_element(object, count).is_empty()
            || super::arguments::mapped_cell(heap, object, count).is_some())
    {
        count += 1;
    }
    Some(count)
}

/// QuickJS's `JS_GetOwnPropertyNamesInternal` with `JS_GPN_SET_ENUM`: the
/// own string keys of `object` (index keys ascending, then the others in
/// order) and whether each is enumerable. An exotic object answers through
/// its internal methods, which may run JavaScript.
fn level_keys(cx: &mut Cx<'_>, object: ObjectId) -> Result<Vec<(PropertyKey, bool)>, Throw> {
    let mut keys = Vec::new();
    if cx.heap.object_kind(object).is_exotic() {
        for key in super::object::own_property_keys(cx, object)? {
            if matches!(key.kind(), KeyKind::Symbol(_)) {
                continue;
            }
            let descriptor = super::object::get_own_property(cx, object, key)?;
            let enumerable =
                descriptor.is_some_and(|descriptor| descriptor.enumerable == Some(true));
            keys.push((key, enumerable));
        }
        return Ok(keys);
    }
    let heap = &*cx.heap;
    let is_array = heap.object_kind(object) == ObjectKind::Array;
    for key in own_keys_with_arguments(heap, object) {
        let enumerable = match key.kind() {
            KeyKind::Symbol(_) => continue,
            KeyKind::Index(_) => heap
                .lookup_own(object, key)
                .is_none_or(|(_, property)| property.has(PropertyFlags::ENUMERABLE)),
            KeyKind::String(_) => {
                // An array's `length` leads its named keys, as the first
                // property of QuickJS's array shape.
                if is_array
                    && keys.last().is_none_or(|&(key, _): &(PropertyKey, bool)| {
                        matches!(key.kind(), KeyKind::Index(_))
                    })
                {
                    keys.push((Names::key(cx.realm.names.length), false));
                }
                heap.lookup_own(object, key)
                    .is_some_and(|(_, property)| property.has(PropertyFlags::ENUMERABLE))
            }
        };
        keys.push((key, enumerable));
    }
    if is_array
        && !keys
            .iter()
            .any(|&(key, _)| key == Names::key(cx.realm.names.length))
    {
        keys.push((Names::key(cx.realm.names.length), false));
    }
    Ok(keys)
}

/// Whether `object` still has the own property `key` (QuickJS's
/// `JS_GetOwnPropertyInternal`, through an exotic object's methods).
fn has_own(cx: &mut Cx<'_>, object: ObjectId, key: PropertyKey) -> Result<bool, Throw> {
    if cx.heap.object_kind(object).is_exotic() {
        return Ok(super::object::get_own_property(cx, object, key)?.is_some());
    }
    Ok(super::object::ordinary_get_own(cx, object, key).is_some())
}

/// Own keys in `[[OwnPropertyKeys]]` order, with the mapped indices of an
/// `arguments` object, whose elements are holes there.
fn own_keys_with_arguments(heap: &Heap, object: ObjectId) -> Vec<PropertyKey> {
    let mut keys = heap.own_keys(object);
    if heap.object_kind(object) == ObjectKind::Arguments && heap.header(object).payload != NONE {
        let mut mapped: Vec<PropertyKey> = (0..super::arguments::mapped_count(heap, object))
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

/// `ForInNext`: QuickJS's `js_for_in_next`. The next key of the level whose
/// property is still an own property of that level's object, as a string;
/// `None` when the enumeration is exhausted. When the level's keys run out,
/// the first time the prototype chain is checked for an enumerable key (none
/// ends the enumeration), and the own keys become the seen set: a fast
/// array's as they are now, any other object's from its snapshot. Each
/// prototype's keys are then read as it is reached, skipping and recording
/// the keys seen.
#[inline(never)]
pub(crate) fn for_in_next(cx: &mut Cx<'_>, iterator: Value) -> Result<Option<Value>, Throw> {
    let iterator = iterator.as_object().expect("a for-in enumeration");
    loop {
        let heap = &mut *cx.heap;
        let cursor = heap.header(iterator).payload;
        let count = heap.raw_slot(iterator, 1).as_int32().unwrap_or(0) as u32;
        let mode = heap.raw_slot(iterator, 3).as_int32().unwrap_or(FOR_IN_KEYS);
        let Some(object) = heap.raw_slot(iterator, 0).as_object() else {
            return Ok(None);
        };
        if cursor >= count {
            if !for_in_next_level(cx, iterator, object, mode)? {
                cx.heap.set_raw_slot(iterator, 0, Value::UNDEFINED);
                return Ok(None);
            }
            continue;
        }
        heap.set_payload(iterator, cursor + 1);
        let key = if mode == FOR_IN_ARRAY {
            PropertyKey::index(cursor).expect("an index key")
        } else {
            let key = heap.get_element(iterator, 2 * cursor);
            let enumerable = heap.get_element(iterator, 2 * cursor + 1) == Value::TRUE;
            let key = match key.as_int32() {
                Some(index) => PropertyKey::index(index as u32).expect("an index key"),
                None => PropertyKey::atom(key.as_string().expect("a key atom")),
            };
            if mode == FOR_IN_CHAIN {
                let visited = heap.raw_slot(iterator, 2).as_object().expect("a seen set");
                if super::object::ordinary_get_own(cx, visited, key).is_some() {
                    continue;
                }
                super::object::create_data_property(cx, visited, key, Value::NULL)?;
            }
            if !enumerable {
                continue;
            }
            key
        };
        if !has_own(cx, object, key)? {
            continue;
        }
        return match key.kind() {
            KeyKind::Index(index) => {
                super::ops::to_string(cx, Value::int32(index as i32)).map(Some)
            }
            _ => Ok(Some(key.to_value())),
        };
    }
}

/// Moves a for-in enumeration whose level is exhausted to the next
/// prototype; `false` when there is none (or, the first time, when no
/// prototype has an enumerable key).
#[cold]
#[inline(never)]
fn for_in_next_level(
    cx: &mut Cx<'_>,
    iterator: ObjectId,
    object: ObjectId,
    mode: i32,
) -> Result<bool, Throw> {
    if mode != FOR_IN_CHAIN {
        // QuickJS's `js_for_in_prepare_prototype_chain_enum`.
        let mut current = object;
        loop {
            let prototype = super::object::get_prototype_of(cx, current)?;
            let Some(prototype) = prototype.as_object() else {
                return Ok(false);
            };
            let mark = cx.heap.roots().mark();
            cx.heap.roots().push(Value::object(prototype));
            let keys = level_keys(cx, prototype);
            cx.heap.roots().truncate(mark);
            if keys?.iter().any(|&(_, enumerable)| enumerable) {
                break;
            }
            current = prototype;
        }
        let seen = if mode == FOR_IN_ARRAY {
            level_keys(cx, object)?
        } else {
            let heap = &*cx.heap;
            let count = heap.raw_slot(iterator, 1).as_int32().unwrap_or(0) as u32;
            (0..count)
                .map(|index| {
                    let key = heap.get_element(iterator, 2 * index);
                    let key = match key.as_int32() {
                        Some(index) => PropertyKey::index(index as u32).expect("an index key"),
                        None => PropertyKey::atom(key.as_string().expect("a key atom")),
                    };
                    (key, false)
                })
                .collect()
        };
        let visited = cx
            .heap
            .alloc_object(ObjectKind::Ordinary, None, seen.len() as u32)
            .map_err(|error| error::heap_error(cx, error))?;
        cx.heap.set_raw_slot(iterator, 2, Value::object(visited));
        for (key, _) in seen {
            super::object::create_data_property(cx, visited, key, Value::NULL)?;
        }
        cx.heap
            .set_raw_slot(iterator, 3, Value::int32(FOR_IN_CHAIN));
    }
    let prototype = super::object::get_prototype_of(cx, object)?;
    let Some(prototype) = prototype.as_object() else {
        return Ok(false);
    };
    cx.heap.set_raw_slot(iterator, 0, Value::object(prototype));
    let keys = level_keys(cx, prototype)?;
    for (index, &(key, enumerable)) in keys.iter().enumerate() {
        let index = 2 * index as u32;
        cx.heap
            .set_element(iterator, index, key.to_value())
            .and_then(|()| {
                cx.heap
                    .set_element(iterator, index + 1, Value::bool(enumerable))
            })
            .map_err(|error| error::heap_error(cx, error))?;
    }
    cx.heap
        .set_raw_slot(iterator, 1, Value::int32(keys.len() as i32));
    cx.heap.set_payload(iterator, 0);
    Ok(true)
}
