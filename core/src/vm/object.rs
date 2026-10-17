//! The object internal methods (phase 4, section 3.1): `[[GetOwnProperty]]`,
//! `[[DefineOwnProperty]]`, `[[HasProperty]]`, `[[Get]]`, `[[Set]]`,
//! `[[Delete]]`, `[[OwnPropertyKeys]]`, `[[GetPrototypeOf]]`,
//! `[[SetPrototypeOf]]`, `[[PreventExtensions]]` and `[[IsExtensible]]`,
//! dispatched on the object kind before the ordinary algorithm, and the
//! abstract operations the builtins share over them.
//!
//! These are the slow paths. The loop's inline caches and `vm::property`'s
//! heap-only walks handle ordinary objects first; they come here when they
//! meet an exotic object ([`ObjectKind::is_exotic`]) or a case the walks do
//! not model (a receiver other than the holder, element attributes).
//!
//! An own property of an ordinary object lives in one of five places: a
//! dense element (writable, enumerable, configurable data), a mapped
//! `arguments` cell, an array's `length` (in the header), a string
//! wrapper's index (the string's unit) or a shape slot.
//!
//! Every value a function here holds across an allocation or a call is
//! rooted on the root stack. Keys returned by [`own_property_keys`] stay
//! rooted until the caller cuts the stack back.

use super::arguments;
use super::error::{self, ErrorKind};
use super::ops;
use super::property::{self, length_value};
use super::proxy;
use super::{Cx, Throw, module, typed};
use crate::heap::atom::{KeyKind, PropertyKey};
use crate::heap::object::{InternalTag, NONE, ObjectKind, flags};
use crate::runtime::{Intrinsic, Names, WellKnown};
use crate::shape::PropertyFlags;
use crate::value::{Kind, ObjectId, ScopeId, Value};

/// A property descriptor: every field optional, as `ToPropertyDescriptor`
/// produces it. A complete descriptor (from [`get_own_property`]) has every
/// field of its kind.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Descriptor {
    pub value: Option<Value>,
    pub get: Option<Value>,
    pub set: Option<Value>,
    pub writable: Option<bool>,
    pub enumerable: Option<bool>,
    pub configurable: Option<bool>,
}

impl Descriptor {
    pub const fn data(value: Value, writable: bool, enumerable: bool, configurable: bool) -> Self {
        Descriptor {
            value: Some(value),
            get: None,
            set: None,
            writable: Some(writable),
            enumerable: Some(enumerable),
            configurable: Some(configurable),
        }
    }

    /// Writable, enumerable, configurable data: `CreateDataProperty`'s.
    pub const fn plain(value: Value) -> Self {
        Descriptor::data(value, true, true, true)
    }

    pub const fn accessor(get: Value, set: Value, enumerable: bool, configurable: bool) -> Self {
        Descriptor {
            value: None,
            get: Some(get),
            set: Some(set),
            writable: None,
            enumerable: Some(enumerable),
            configurable: Some(configurable),
        }
    }

    pub const fn is_accessor(&self) -> bool {
        self.get.is_some() || self.set.is_some()
    }

    pub const fn is_data(&self) -> bool {
        self.value.is_some() || self.writable.is_some()
    }

    pub const fn is_generic(&self) -> bool {
        !self.is_accessor() && !self.is_data()
    }

    /// The flags of a complete descriptor.
    fn flags(&self) -> PropertyFlags {
        let enumerable = self.enumerable.unwrap_or(false);
        let configurable = self.configurable.unwrap_or(false);
        if self.is_accessor() {
            PropertyFlags::accessor(enumerable, configurable)
        } else {
            PropertyFlags::data(self.writable.unwrap_or(false), enumerable, configurable)
        }
    }

    fn from_flags(flags: PropertyFlags, first: Value, second: Value) -> Self {
        let enumerable = flags.has(PropertyFlags::ENUMERABLE);
        let configurable = flags.has(PropertyFlags::CONFIGURABLE);
        if flags.has(PropertyFlags::ACCESSOR) {
            Descriptor::accessor(first, second, enumerable, configurable)
        } else {
            Descriptor::data(
                first,
                flags.has(PropertyFlags::WRITABLE),
                enumerable,
                configurable,
            )
        }
    }

    /// Roots the descriptor's values.
    pub fn root(&self, cx: &mut Cx<'_>) {
        for value in [self.value, self.get, self.set].into_iter().flatten() {
            cx.heap.roots().push(value);
        }
    }
}

#[inline]
fn root(cx: &mut Cx<'_>, value: Value) {
    cx.heap.roots().push(value);
}

pub(crate) fn heap_error(cx: &Cx<'_>, error: crate::HeapError) -> Throw {
    error::heap_error(cx, error)
}

// ---- [[GetOwnProperty]] ----

/// The own property `key` of `object` as a complete descriptor.
pub(crate) fn get_own_property(
    cx: &mut Cx<'_>,
    object: ObjectId,
    key: PropertyKey,
) -> Result<Option<Descriptor>, Throw> {
    match cx.heap.object_kind(object) {
        ObjectKind::Proxy => proxy::get_own_property(cx, object, key),
        ObjectKind::StringWrapper => {
            if let Some(descriptor) = string_index(cx, object, key)? {
                return Ok(Some(descriptor));
            }
            Ok(ordinary_get_own(cx, object, key))
        }
        ObjectKind::TypedArray => match typed::numeric_index(cx.heap, key) {
            Some(index) => Ok(typed::get_element(cx, object, index)?
                .map(|value| Descriptor::data(value, true, true, true))),
            None => Ok(ordinary_get_own(cx, object, key)),
        },
        ObjectKind::ModuleNamespace => match module::namespace_position(cx, object, key) {
            Some(position) => {
                let value = module::namespace_value(cx, object, position)?;
                Ok(Some(Descriptor::data(value, true, true, false)))
            }
            None => Ok(ordinary_get_own(cx, object, key)),
        },
        _ => Ok(ordinary_get_own(cx, object, key)),
    }
}

/// A string wrapper's own index property.
fn string_index(
    cx: &mut Cx<'_>,
    object: ObjectId,
    key: PropertyKey,
) -> Result<Option<Descriptor>, Throw> {
    let KeyKind::Index(index) = key.kind() else {
        return Ok(None);
    };
    let string = cx
        .heap
        .internal_slot(object, 0)
        .as_string()
        .expect("a string");
    let repr = &cx.heap.string(string).repr();
    if index as usize >= repr.len() {
        return Ok(None);
    }
    let unit = repr.unit(index as usize);
    let text = cx
        .heap
        .intern_units(&[unit])
        .map_err(|error| heap_error(cx, error))?;
    Ok(Some(Descriptor::data(
        Value::string(text),
        false,
        true,
        false,
    )))
}

/// `OrdinaryGetOwnProperty`, which never runs JavaScript.
pub(crate) fn ordinary_get_own(
    cx: &Cx<'_>,
    object: ObjectId,
    key: PropertyKey,
) -> Option<Descriptor> {
    let heap = &*cx.heap;
    match key.kind() {
        KeyKind::Index(index) => {
            if let Some(cell) = arguments::mapped_cell(heap, object, index) {
                let value = heap.scope_get(cell, 0, 0);
                let flags = heap
                    .lookup_own(object, key)
                    .map_or(PropertyFlags::DEFAULT, |(_, flags)| flags);
                return Some(Descriptor::from_flags(flags, value, Value::UNDEFINED));
            }
            let value = heap.get_element(object, index);
            if !value.is_empty() {
                return Some(Descriptor::plain(value));
            }
        }
        KeyKind::String(id) if id == cx.realm.names.length => {
            if heap.object_kind(object) == ObjectKind::Array {
                let writable = !heap.header(object).has(flags::LENGTH_READONLY);
                let length = length_value(heap.array_length(object));
                return Some(Descriptor::data(length, writable, false, false));
            }
        }
        _ => {}
    }
    let (slot, flags) = heap.lookup_own(object, key)?;
    let first = heap.get_slot(object, slot);
    let second = if flags.has(PropertyFlags::ACCESSOR) {
        heap.get_slot(object, slot + 1)
    } else {
        Value::UNDEFINED
    };
    Some(Descriptor::from_flags(flags, first, second))
}

// ---- [[DefineOwnProperty]] ----

/// `object.[[DefineOwnProperty]](key, descriptor)`. The descriptor's values
/// must be rooted by the caller.
pub(crate) fn define_own_property(
    cx: &mut Cx<'_>,
    object: ObjectId,
    key: PropertyKey,
    descriptor: &Descriptor,
) -> Result<bool, Throw> {
    match cx.heap.object_kind(object) {
        ObjectKind::Proxy => proxy::define_own_property(cx, object, key, descriptor),
        ObjectKind::Array => array_define(cx, object, key, descriptor),
        ObjectKind::Arguments => arguments_define(cx, object, key, descriptor),
        ObjectKind::StringWrapper => {
            if let Some(current) = string_index(cx, object, key)? {
                return Ok(compatible(cx, false, descriptor, Some(&current)));
            }
            ordinary_define(cx, object, key, descriptor)
        }
        ObjectKind::TypedArray => match typed::numeric_index(cx.heap, key) {
            Some(index) => {
                if typed::valid_index(cx.heap, object, index).is_none()
                    || descriptor.configurable == Some(false)
                    || descriptor.enumerable == Some(false)
                    || descriptor.is_accessor()
                    || descriptor.writable == Some(false)
                {
                    return Ok(false);
                }
                if let Some(value) = descriptor.value {
                    typed::set_element(cx, object, index, value)?;
                }
                Ok(true)
            }
            None => ordinary_define(cx, object, key, descriptor),
        },
        ObjectKind::ModuleNamespace if !matches!(key.kind(), KeyKind::Symbol(_)) => {
            let Some(current) = get_own_property(cx, object, key)? else {
                return Ok(false);
            };
            if descriptor.configurable == Some(true)
                || descriptor.enumerable == Some(false)
                || descriptor.is_accessor()
                || descriptor.writable == Some(false)
            {
                return Ok(false);
            }
            Ok(match (descriptor.value, current.value) {
                (Some(value), Some(existing)) => same_value(cx, value, existing),
                _ => true,
            })
        }
        _ => ordinary_define(cx, object, key, descriptor),
    }
}

/// `IsCompatiblePropertyDescriptor` (validation without applying).
pub(crate) fn compatible(
    _cx: &Cx<'_>,
    extensible: bool,
    descriptor: &Descriptor,
    current: Option<&Descriptor>,
) -> bool {
    let Some(current) = current else {
        return extensible;
    };
    if descriptor.is_generic()
        && descriptor.enumerable.is_none()
        && descriptor.configurable.is_none()
    {
        return true;
    }
    if current.configurable == Some(false) {
        if descriptor.configurable == Some(true) {
            return false;
        }
        if descriptor.enumerable.is_some() && descriptor.enumerable != current.enumerable {
            return false;
        }
        if !descriptor.is_generic() && descriptor.is_accessor() != current.is_accessor() {
            return false;
        }
        if current.is_accessor() {
            if let Some(get) = descriptor.get
                && Some(get) != current.get
            {
                return false;
            }
            if let Some(set) = descriptor.set
                && Some(set) != current.set
            {
                return false;
            }
        } else if current.writable == Some(false) {
            if descriptor.writable == Some(true) {
                return false;
            }
            if let Some(value) = descriptor.value
                && !same_value(_cx, value, current.value.unwrap_or(Value::UNDEFINED))
            {
                return false;
            }
        }
    }
    true
}

/// `SameValue`.
pub(crate) fn same_value(cx: &Cx<'_>, x: Value, y: Value) -> bool {
    if let (Some(a), Some(b)) = (x.as_f64(), y.as_f64()) {
        if a.is_nan() && b.is_nan() {
            return true;
        }
        return a == b && a.is_sign_negative() == b.is_sign_negative();
    }
    ops::strict_eq(cx.heap, x, y)
}

/// `SameValueZero`.
pub(crate) fn same_value_zero(cx: &Cx<'_>, x: Value, y: Value) -> bool {
    if let (Some(a), Some(b)) = (x.as_f64(), y.as_f64()) {
        return a == b || (a.is_nan() && b.is_nan());
    }
    ops::strict_eq(cx.heap, x, y)
}

/// `OrdinaryDefineOwnProperty`: `ValidateAndApplyPropertyDescriptor` over
/// the object's own storage.
pub(crate) fn ordinary_define(
    cx: &mut Cx<'_>,
    object: ObjectId,
    key: PropertyKey,
    descriptor: &Descriptor,
) -> Result<bool, Throw> {
    let current = ordinary_get_own(cx, object, key);
    let extensible = cx.heap.header(object).has(flags::EXTENSIBLE);
    if !compatible(cx, extensible, descriptor, current.as_ref()) {
        return Ok(false);
    }
    let complete = match current {
        None => {
            if descriptor.is_accessor() {
                Descriptor {
                    value: None,
                    get: Some(descriptor.get.unwrap_or(Value::UNDEFINED)),
                    set: Some(descriptor.set.unwrap_or(Value::UNDEFINED)),
                    writable: None,
                    enumerable: Some(descriptor.enumerable.unwrap_or(false)),
                    configurable: Some(descriptor.configurable.unwrap_or(false)),
                }
            } else {
                Descriptor::data(
                    descriptor.value.unwrap_or(Value::UNDEFINED),
                    descriptor.writable.unwrap_or(false),
                    descriptor.enumerable.unwrap_or(false),
                    descriptor.configurable.unwrap_or(false),
                )
            }
        }
        Some(current) => merge(&current, descriptor),
    };
    store(cx, object, key, current.is_some(), &complete)?;
    Ok(true)
}

/// The descriptor `current` becomes when `descriptor` is applied to it.
fn merge(current: &Descriptor, descriptor: &Descriptor) -> Descriptor {
    let enumerable = descriptor.enumerable.or(current.enumerable);
    let configurable = descriptor.configurable.or(current.configurable);
    if descriptor.is_accessor() && !current.is_accessor() {
        return Descriptor {
            value: None,
            get: Some(descriptor.get.unwrap_or(Value::UNDEFINED)),
            set: Some(descriptor.set.unwrap_or(Value::UNDEFINED)),
            writable: None,
            enumerable,
            configurable,
        };
    }
    if descriptor.is_data() && current.is_accessor() {
        return Descriptor {
            value: Some(descriptor.value.unwrap_or(Value::UNDEFINED)),
            get: None,
            set: None,
            writable: Some(descriptor.writable.unwrap_or(false)),
            enumerable,
            configurable,
        };
    }
    if current.is_accessor() {
        Descriptor {
            value: None,
            get: descriptor.get.or(current.get),
            set: descriptor.set.or(current.set),
            writable: None,
            enumerable,
            configurable,
        }
    } else {
        Descriptor {
            value: descriptor.value.or(current.value),
            get: None,
            set: None,
            writable: descriptor.writable.or(current.writable),
            enumerable,
            configurable,
        }
    }
}

/// Writes a complete descriptor into the object's storage: a dense element
/// for writable, enumerable, configurable index data on an object that is
/// not sparse, a named property otherwise (making the object sparse first
/// when an element must change its attributes).
fn store(
    cx: &mut Cx<'_>,
    object: ObjectId,
    key: PropertyKey,
    exists: bool,
    complete: &Descriptor,
) -> Result<(), Throw> {
    let flags = complete.flags();
    let (first, second) = if complete.is_accessor() {
        (
            complete.get.unwrap_or(Value::UNDEFINED),
            complete.set.unwrap_or(Value::UNDEFINED),
        )
    } else {
        (complete.value.unwrap_or(Value::UNDEFINED), Value::UNDEFINED)
    };
    let mark = cx.heap.roots().mark();
    root(cx, Value::object(object));
    root(cx, first);
    root(cx, second);
    let result = (|| {
        let heap = &mut *cx.heap;
        if array_index(heap, key).is_some() {
            let sparse = heap.header(object).has(flags::SPARSE);
            if let KeyKind::Index(index) = key.kind() {
                let element = !sparse && !heap.get_element(object, index).is_empty();
                if !sparse
                    && flags == PropertyFlags::DEFAULT
                    && heap.lookup_own(object, key).is_none()
                    && (element || property::dense_index(heap, object, index))
                {
                    return heap.set_element(object, index, first);
                }
            }
            // An index property outside the element range: the object goes
            // sparse, so the element fast paths never miss a named index.
            if !sparse && heap.object_kind(object).has_elements() {
                heap.make_sparse(object)?;
            }
        }
        if exists && heap.lookup_own(object, key).is_some() {
            heap.reshape_property(object, key, flags, [first, second])
        } else if flags.has(PropertyFlags::ACCESSOR) {
            heap.add_accessor(object, key, flags, first, second)
        } else {
            heap.add_property(object, key, flags, first)
        }
    })();
    cx.heap.roots().truncate(mark);
    result.map_err(|error| heap_error(cx, error))?;
    if cx.heap.object_kind(object) == ObjectKind::Array
        && let Some(index) = array_index(cx.heap, key)
        && index >= cx.heap.array_length(object)
    {
        cx.heap.set_array_length(object, index + 1);
    }
    Ok(())
}

/// An array's `[[DefineOwnProperty]]`: `length` through `ArraySetLength`,
/// an index past a read-only `length` refused.
fn array_define(
    cx: &mut Cx<'_>,
    array: ObjectId,
    key: PropertyKey,
    descriptor: &Descriptor,
) -> Result<bool, Throw> {
    match key.kind() {
        KeyKind::String(id) if id == cx.realm.names.length => {
            array_set_length(cx, array, descriptor)
        }
        _ => {
            if let Some(index) = array_index(cx.heap, key) {
                let length = cx.heap.array_length(array);
                if index >= length && cx.heap.header(array).has(flags::LENGTH_READONLY) {
                    return Ok(false);
                }
            }
            ordinary_define(cx, array, key, descriptor)
        }
    }
}

/// The array index a key names: an immediate index or an atom past 2^31.
#[inline]
pub(crate) fn array_index(heap: &crate::Heap, key: PropertyKey) -> Option<u32> {
    match key.kind() {
        KeyKind::Index(index) => Some(index),
        KeyKind::String(id) => {
            // A key is an atom, never a rope.
            let index = heap.strings.get(id.0).array_index;
            (index != NONE).then_some(index)
        }
        KeyKind::Symbol(_) => None,
    }
}

/// `ArraySetLength`.
pub(crate) fn array_set_length(
    cx: &mut Cx<'_>,
    array: ObjectId,
    descriptor: &Descriptor,
) -> Result<bool, Throw> {
    let readonly = cx.heap.header(array).has(flags::LENGTH_READONLY);
    let length_descriptor = |cx: &Cx<'_>| {
        let length = length_value(cx.heap.array_length(array));
        Descriptor::data(length, !readonly, false, false)
    };
    let Some(value) = descriptor.value else {
        let current = length_descriptor(cx);
        if !compatible(cx, false, descriptor, Some(&current)) {
            return Ok(false);
        }
        if descriptor.writable == Some(false) {
            set_flag(cx, array, flags::LENGTH_READONLY);
        }
        return Ok(true);
    };
    let new_length = to_uint32(cx, value)?;
    let number = ops::to_number(cx, value)?;
    if f64::from(new_length) != number {
        return Err(error::range_error(cx, "invalid array length"));
    }
    let readonly = cx.heap.header(array).has(flags::LENGTH_READONLY);
    let current = Descriptor::data(
        length_value(cx.heap.array_length(array)),
        !readonly,
        false,
        false,
    );
    let mut check = *descriptor;
    check.value = Some(length_value(new_length));
    if !compatible(cx, false, &check, Some(&current)) {
        return Ok(false);
    }
    let old_length = cx.heap.array_length(array);
    if new_length >= old_length {
        cx.heap.set_array_length(array, new_length);
        if descriptor.writable == Some(false) {
            set_flag(cx, array, flags::LENGTH_READONLY);
        }
        return Ok(true);
    }
    if readonly {
        return Ok(false);
    }
    let kept = truncate(cx, array, new_length)?;
    if descriptor.writable == Some(false) {
        set_flag(cx, array, flags::LENGTH_READONLY);
    }
    Ok(kept == new_length)
}

fn set_flag(cx: &mut Cx<'_>, object: ObjectId, flag: u8) {
    cx.heap.set_object_flag(object, flag);
}

fn to_uint32(cx: &mut Cx<'_>, value: Value) -> Result<u32, Throw> {
    if let Some(int) = value.as_int32()
        && int >= 0
    {
        return Ok(int as u32);
    }
    let number = ops::to_number(cx, value)?;
    Ok(super::number::to_uint32(number))
}

/// Shortens an array to `length`, deleting from the end; stops at a
/// non-configurable index and returns the length reached.
pub(crate) fn truncate(cx: &mut Cx<'_>, array: ObjectId, length: u32) -> Result<u32, Throw> {
    let heap = &mut *cx.heap;
    let mut named: Vec<u32> = heap
        .own_keys(array)
        .into_iter()
        .filter_map(|key| match key.kind() {
            KeyKind::Index(index) if index >= length => Some(index),
            KeyKind::String(id) => {
                let index = heap.string(id).array_index;
                (index != NONE && index >= length).then_some(index)
            }
            _ => None,
        })
        .filter(|&index| heap.get_element(array, index).is_empty())
        .collect();
    named.sort_unstable_by(|a, b| b.cmp(a));
    for index in named {
        let key = index_key(cx, index)?;
        let heap = &mut *cx.heap;
        if let Some((_, flags)) = heap.lookup_own(array, key)
            && !flags.has(PropertyFlags::CONFIGURABLE)
        {
            heap.set_array_length(array, index + 1);
            return Ok(index + 1);
        }
        heap.delete_property(array, key)
            .map_err(|error| heap_error(cx, error))?;
    }
    cx.heap.set_array_length(array, length);
    Ok(length)
}

/// The key of an array index (an atom past 2^31).
pub(crate) fn index_key(cx: &mut Cx<'_>, index: u32) -> Result<PropertyKey, Throw> {
    if let Some(key) = PropertyKey::index(index) {
        return Ok(key);
    }
    cx.heap
        .property_key(&index.to_string())
        .map_err(|error| heap_error(cx, error))
}

/// The key of a number index up to 2^53 − 1.
pub(crate) fn number_key(cx: &mut Cx<'_>, index: f64) -> Result<PropertyKey, Throw> {
    if (0.0..4294967295.0).contains(&index) && index.fract() == 0.0 {
        return index_key(cx, index as u32);
    }
    let text = super::number::to_string(index);
    cx.heap
        .property_key(&text)
        .map_err(|error| heap_error(cx, error))
}

/// A mapped `arguments` object's `[[DefineOwnProperty]]`.
fn arguments_define(
    cx: &mut Cx<'_>,
    object: ObjectId,
    key: PropertyKey,
    descriptor: &Descriptor,
) -> Result<bool, Throw> {
    let KeyKind::Index(index) = key.kind() else {
        return ordinary_define(cx, object, key, descriptor);
    };
    let Some(cell) = arguments::mapped_cell(cx.heap, object, index) else {
        return ordinary_define(cx, object, key, descriptor);
    };
    let current = ordinary_get_own(cx, object, key).expect("a mapped index");
    if !compatible(cx, true, descriptor, Some(&current)) {
        return Ok(false);
    }
    let mut complete = merge(&current, descriptor);
    let unmap = descriptor.is_accessor() || descriptor.writable == Some(false);
    if !descriptor.is_accessor()
        && let Some(value) = descriptor.value
    {
        cx.heap.scope_set(cell, 0, 0, value);
    }
    if !unmap && complete.flags() == PropertyFlags::DEFAULT {
        if cx.heap.lookup_own(object, key).is_some() {
            cx.heap
                .delete_property(object, key)
                .map_err(|error| heap_error(cx, error))?;
        }
        return Ok(true);
    }
    if !complete.is_accessor() && complete.value.is_none() {
        complete.value = Some(cx.heap.scope_get(cell, 0, 0));
    }
    if unmap {
        arguments::unmap(cx.heap, object, index);
    }
    // A named property holds the attributes (and the value once unmapped).
    if !cx.heap.header(object).has(flags::SPARSE) {
        cx.heap
            .make_sparse(object)
            .map_err(|error| heap_error(cx, error))?;
    }
    let exists = cx.heap.lookup_own(object, key).is_some();
    store(cx, object, key, exists, &complete)?;
    Ok(true)
}

// ---- [[HasProperty]], [[Get]], [[Set]], [[Delete]] ----

/// `object.[[HasProperty]](key)`.
pub(crate) fn has_property(
    cx: &mut Cx<'_>,
    object: ObjectId,
    key: PropertyKey,
) -> Result<bool, Throw> {
    let mut current = object;
    loop {
        match cx.heap.object_kind(current) {
            ObjectKind::Proxy => return proxy::has(cx, current, key),
            ObjectKind::StringWrapper => {
                if string_index(cx, current, key)?.is_some() {
                    return Ok(true);
                }
            }
            ObjectKind::TypedArray => {
                if let Some(index) = typed::numeric_index(cx.heap, key) {
                    return Ok(typed::valid_index(cx.heap, current, index).is_some());
                }
            }
            ObjectKind::ModuleNamespace => {
                if module::namespace_position(cx, current, key).is_some() {
                    return Ok(true);
                }
            }
            _ => {}
        }
        if ordinary_get_own(cx, current, key).is_some() {
            return Ok(true);
        }
        match cx.heap.prototype_of(current) {
            Some(prototype) => current = prototype,
            None => return Ok(false),
        }
    }
}

/// `object.[[Get]](key, receiver)`.
pub(crate) fn get(
    cx: &mut Cx<'_>,
    object: ObjectId,
    key: PropertyKey,
    receiver: Value,
) -> Result<Value, Throw> {
    let mut current = object;
    loop {
        let descriptor = match cx.heap.object_kind(current) {
            ObjectKind::Proxy => return proxy::get(cx, current, key, receiver),
            ObjectKind::StringWrapper => match string_index(cx, current, key)? {
                Some(descriptor) => Some(descriptor),
                None => ordinary_get_own(cx, current, key),
            },
            ObjectKind::TypedArray => match typed::numeric_index(cx.heap, key) {
                Some(index) => {
                    return Ok(typed::get_element(cx, current, index)?.unwrap_or(Value::UNDEFINED));
                }
                None => ordinary_get_own(cx, current, key),
            },
            ObjectKind::ModuleNamespace => match module::namespace_position(cx, current, key) {
                Some(position) => return module::namespace_value(cx, current, position),
                None => ordinary_get_own(cx, current, key),
            },
            _ => ordinary_get_own(cx, current, key),
        };
        if let Some(descriptor) = descriptor {
            return match descriptor.get {
                Some(getter) if descriptor.is_accessor() => {
                    if getter.is_undefined() {
                        Ok(Value::UNDEFINED)
                    } else {
                        super::call(cx, getter, receiver, &[])
                    }
                }
                _ => Ok(descriptor.value.unwrap_or(Value::UNDEFINED)),
            };
        }
        match cx.heap.prototype_of(current) {
            Some(prototype) => current = prototype,
            None => return Ok(Value::UNDEFINED),
        }
    }
}

/// `object.[[Set]](key, value, receiver)`: `OrdinarySet` along the chain.
pub(crate) fn set(
    cx: &mut Cx<'_>,
    object: ObjectId,
    key: PropertyKey,
    value: Value,
    receiver: Value,
) -> Result<bool, Throw> {
    let mut current = object;
    let own = loop {
        let descriptor = match cx.heap.object_kind(current) {
            ObjectKind::Proxy => return proxy::set(cx, current, key, value, receiver),
            ObjectKind::ModuleNamespace => return Ok(false),
            ObjectKind::StringWrapper => match string_index(cx, current, key)? {
                Some(descriptor) => Some(descriptor),
                None => ordinary_get_own(cx, current, key),
            },
            ObjectKind::TypedArray => match typed::numeric_index(cx.heap, key) {
                Some(index) => {
                    if receiver == Value::object(current) {
                        typed::set_element(cx, current, index, value)?;
                        return Ok(true);
                    }
                    if typed::valid_index(cx.heap, current, index).is_none() {
                        return Ok(true);
                    }
                    Some(Descriptor::data(Value::UNDEFINED, true, true, true))
                }
                None => ordinary_get_own(cx, current, key),
            },
            _ => ordinary_get_own(cx, current, key),
        };
        if let Some(descriptor) = descriptor {
            break Some(descriptor);
        }
        match cx.heap.prototype_of(current) {
            Some(prototype) => current = prototype,
            None => break None,
        }
    };
    let own = own.unwrap_or(Descriptor::plain(Value::UNDEFINED));
    if own.is_accessor() {
        let setter = own.set.unwrap_or(Value::UNDEFINED);
        if setter.is_undefined() {
            return Ok(false);
        }
        super::call(cx, setter, receiver, &[value])?;
        return Ok(true);
    }
    if own.writable == Some(false) {
        return Ok(false);
    }
    let Some(receiver) = receiver.as_object() else {
        return Ok(false);
    };
    let mark = cx.heap.roots().mark();
    root(cx, value);
    root(cx, Value::object(receiver));
    let result = (|| match get_own_property(cx, receiver, key)? {
        Some(existing) => {
            if existing.is_accessor() || existing.writable == Some(false) {
                return Ok(false);
            }
            let update = Descriptor {
                value: Some(value),
                ..Descriptor::default()
            };
            define_own_property(cx, receiver, key, &update)
        }
        None => define_own_property(cx, receiver, key, &Descriptor::plain(value)),
    })();
    cx.heap.roots().truncate(mark);
    result
}

/// `object.[[Delete]](key)`.
pub(crate) fn delete(cx: &mut Cx<'_>, object: ObjectId, key: PropertyKey) -> Result<bool, Throw> {
    match cx.heap.object_kind(object) {
        ObjectKind::Proxy => return proxy::delete(cx, object, key),
        ObjectKind::StringWrapper => {
            if string_index(cx, object, key)?.is_some() {
                return Ok(false);
            }
        }
        ObjectKind::TypedArray => {
            if let Some(index) = typed::numeric_index(cx.heap, key) {
                return Ok(typed::valid_index(cx.heap, object, index).is_none());
            }
        }
        ObjectKind::ModuleNamespace => {
            if module::namespace_position(cx, object, key).is_some() {
                return Ok(false);
            }
        }
        _ => {}
    }
    ordinary_delete(cx, object, key)
}

/// `OrdinaryDelete`.
pub(crate) fn ordinary_delete(
    cx: &mut Cx<'_>,
    object: ObjectId,
    key: PropertyKey,
) -> Result<bool, Throw> {
    let heap = &mut *cx.heap;
    match key.kind() {
        KeyKind::Index(index) => {
            if arguments::mapped_cell(heap, object, index).is_some() {
                // A mapped index made non-configurable stays.
                if heap
                    .lookup_own(object, key)
                    .is_some_and(|(_, flags)| !flags.has(PropertyFlags::CONFIGURABLE))
                {
                    return Ok(false);
                }
                arguments::unmap(heap, object, index);
                if heap.lookup_own(object, key).is_some() {
                    heap.delete_property(object, key)
                        .map_err(|error| heap_error(cx, error))?;
                }
                return Ok(true);
            }
            if !heap.get_element(object, index).is_empty() {
                heap.set_element(object, index, Value::EMPTY)
                    .map_err(|error| heap_error(cx, error))?;
                return Ok(true);
            }
        }
        KeyKind::String(id) if id == cx.realm.names.length => {
            if heap.object_kind(object) == ObjectKind::Array {
                return Ok(false);
            }
        }
        _ => {}
    }
    match heap.lookup_own(object, key) {
        None => Ok(true),
        Some((_, flags)) if !flags.has(PropertyFlags::CONFIGURABLE) => Ok(false),
        Some(_) => {
            heap.delete_property(object, key)
                .map_err(|error| heap_error(cx, error))?;
            Ok(true)
        }
    }
}

// ---- [[OwnPropertyKeys]] ----

/// `object.[[OwnPropertyKeys]]()`: integer indices ascending, then strings,
/// then symbols, each in creation order. Symbol keys are rooted on the root
/// stack (the caller cuts it back).
pub(crate) fn own_property_keys(
    cx: &mut Cx<'_>,
    object: ObjectId,
) -> Result<Vec<PropertyKey>, Throw> {
    let keys = match cx.heap.object_kind(object) {
        ObjectKind::Proxy => return proxy::own_keys(cx, object),
        ObjectKind::StringWrapper => {
            let string = cx
                .heap
                .internal_slot(object, 0)
                .as_string()
                .expect("a string");
            let length = cx.heap.string_len(string) as u32;
            let mut keys: Vec<PropertyKey> = (0..length).filter_map(PropertyKey::index).collect();
            keys.extend(ordinary_own_keys(cx, object));
            keys
        }
        ObjectKind::ModuleNamespace => {
            let mut keys = module::namespace_keys(cx, object).to_vec();
            keys.extend(ordinary_own_keys(cx, object));
            keys
        }
        ObjectKind::TypedArray => {
            let length = typed::length(cx.heap, object).unwrap_or(0) as u32;
            let mut keys = Vec::with_capacity(length as usize);
            for index in 0..length {
                keys.push(index_key(cx, index)?);
            }
            keys.extend(ordinary_own_keys(cx, object));
            keys
        }
        _ => ordinary_own_keys(cx, object),
    };
    for key in &keys {
        if let KeyKind::Symbol(symbol) = key.kind() {
            cx.heap.roots().push(Value::symbol(symbol));
        }
    }
    Ok(keys)
}

/// `OrdinaryOwnPropertyKeys`, with the mapped indices of an `arguments`
/// object and an array's `length`.
pub(crate) fn ordinary_own_keys(cx: &Cx<'_>, object: ObjectId) -> Vec<PropertyKey> {
    let heap = &*cx.heap;
    let mut keys = heap.own_keys(object);
    let header = heap.header(object);
    if header.kind == ObjectKind::Arguments && header.payload != NONE {
        let mapped_count = heap
            .scope_get(ScopeId(header.payload), 0, 0)
            .as_int32()
            .unwrap_or(0) as u32;
        let mut mapped: Vec<PropertyKey> = (0..mapped_count)
            .filter(|&index| arguments::mapped_cell(heap, object, index).is_some())
            .filter_map(PropertyKey::index)
            .filter(|key| !keys.contains(key))
            .collect();
        if !mapped.is_empty() {
            mapped.extend(keys);
            sort_index_keys(heap, &mut mapped);
            keys = mapped;
        }
    }
    if header.kind == ObjectKind::Array {
        let length = Names::key(cx.realm.names.length);
        let first_named = keys
            .iter()
            .position(|key| !is_index_key(heap, *key))
            .unwrap_or(keys.len());
        keys.insert(first_named, length);
    }
    keys
}

fn is_index_key(heap: &crate::Heap, key: PropertyKey) -> bool {
    match key.kind() {
        KeyKind::Index(_) => true,
        KeyKind::String(id) => heap.string(id).array_index != NONE,
        KeyKind::Symbol(_) => false,
    }
}

/// Stable-sorts index keys first, ascending, leaving the rest in order.
fn sort_index_keys(heap: &crate::Heap, keys: &mut [PropertyKey]) {
    keys.sort_by_key(|key| match key.kind() {
        KeyKind::Index(index) => (0, index),
        KeyKind::String(id) if heap.string(id).array_index != NONE => {
            (0, heap.string(id).array_index)
        }
        _ => (1, 0),
    });
}

// ---- prototypes and extensibility ----

pub(crate) fn get_prototype_of(cx: &mut Cx<'_>, object: ObjectId) -> Result<Value, Throw> {
    if cx.heap.object_kind(object) == ObjectKind::Proxy {
        return proxy::get_prototype_of(cx, object);
    }
    Ok(cx
        .heap
        .prototype_of(object)
        .map_or(Value::NULL, Value::object))
}

/// `object.[[SetPrototypeOf]](prototype)`; `prototype` is an object or
/// `null`.
pub(crate) fn set_prototype_of(
    cx: &mut Cx<'_>,
    object: ObjectId,
    prototype: Value,
) -> Result<bool, Throw> {
    if cx.heap.object_kind(object) == ObjectKind::Proxy {
        return proxy::set_prototype_of(cx, object, prototype);
    }
    ordinary_set_prototype_of(cx, object, prototype)
}

pub(crate) fn ordinary_set_prototype_of(
    cx: &mut Cx<'_>,
    object: ObjectId,
    prototype: Value,
) -> Result<bool, Throw> {
    let new = prototype.as_object();
    let current = cx.heap.prototype_of(object);
    if current == new {
        return Ok(true);
    }
    // `%Object.prototype%` is an immutable-prototype exotic object.
    if object == cx.realm.intrinsic(Intrinsic::ObjectPrototype) {
        return Ok(false);
    }
    if !cx.heap.header(object).has(flags::EXTENSIBLE) {
        return Ok(false);
    }
    let mut walk = new;
    while let Some(step) = walk {
        if step == object {
            return Ok(false);
        }
        if cx.heap.object_kind(step) == ObjectKind::Proxy {
            break;
        }
        walk = cx.heap.prototype_of(step);
    }
    match cx.heap.set_prototype(object, new) {
        Ok(()) => Ok(true),
        Err(crate::HeapError::PrototypeCycle) => Ok(false),
        Err(error) => Err(heap_error(cx, error)),
    }
}

pub(crate) fn prevent_extensions(cx: &mut Cx<'_>, object: ObjectId) -> Result<bool, Throw> {
    match cx.heap.object_kind(object) {
        ObjectKind::Proxy => return proxy::prevent_extensions(cx, object),
        // A typed array whose length can change stays extensible.
        ObjectKind::TypedArray if !typed::fixed_length(cx.heap, object) => return Ok(false),
        _ => {}
    }
    ordinary_prevent_extensions(cx, object)
}

pub(crate) fn ordinary_prevent_extensions(
    cx: &mut Cx<'_>,
    object: ObjectId,
) -> Result<bool, Throw> {
    // Elements may only be added to extensible objects, which the dense
    // write paths do not check: they move to named properties first.
    cx.heap
        .make_sparse(object)
        .map_err(|error| heap_error(cx, error))?;
    cx.heap.clear_object_flag(object, flags::EXTENSIBLE);
    Ok(true)
}

pub(crate) fn is_extensible(cx: &mut Cx<'_>, object: ObjectId) -> Result<bool, Throw> {
    if cx.heap.object_kind(object) == ObjectKind::Proxy {
        return proxy::is_extensible(cx, object);
    }
    Ok(cx.heap.header(object).has(flags::EXTENSIBLE))
}

// ---- abstract operations ----

/// `CreateDataProperty`.
pub(crate) fn create_data_property(
    cx: &mut Cx<'_>,
    object: ObjectId,
    key: PropertyKey,
    value: Value,
) -> Result<bool, Throw> {
    // The ordinary common case: an absent key on an extensible object.
    let kind = cx.heap.object_kind(object);
    if !kind.is_exotic() && kind != ObjectKind::Arguments {
        let heap = &mut *cx.heap;
        let header = heap.header(object);
        if header.has(flags::EXTENSIBLE)
            && !matches!(key.kind(), KeyKind::String(id) if id == cx.realm.names.length)
            && ordinary_get_own(cx, object, key).is_none()
        {
            if let KeyKind::Index(index) = key.kind()
                && kind == ObjectKind::Array
                && index >= cx.heap.array_length(object)
                && cx.heap.header(object).has(flags::LENGTH_READONLY)
            {
                return Ok(false);
            }
            store(cx, object, key, false, &Descriptor::plain(value))?;
            return Ok(true);
        }
    }
    let mark = cx.heap.roots().mark();
    root(cx, value);
    let result = define_own_property(cx, object, key, &Descriptor::plain(value));
    cx.heap.roots().truncate(mark);
    result
}

pub(crate) fn create_data_property_or_throw(
    cx: &mut Cx<'_>,
    object: ObjectId,
    key: PropertyKey,
    value: Value,
) -> Result<(), Throw> {
    if create_data_property(cx, object, key, value)? {
        Ok(())
    } else {
        Err(cannot_define(cx, object, key))
    }
}

pub(crate) fn define_property_or_throw(
    cx: &mut Cx<'_>,
    object: ObjectId,
    key: PropertyKey,
    descriptor: &Descriptor,
) -> Result<(), Throw> {
    if define_own_property(cx, object, key, descriptor)? {
        Ok(())
    } else {
        Err(cannot_define(cx, object, key))
    }
}

/// A rejected `[[DefineOwnProperty]]` of `key` on `object`, with the reason
/// QuickJS's `JS_DefineProperty` gives.
#[cold]
#[inline(never)]
pub(crate) fn cannot_define(cx: &mut Cx<'_>, object: ObjectId, key: PropertyKey) -> Throw {
    let heap = &*cx.heap;
    let header = *heap.header(object);
    let message = match header.kind {
        ObjectKind::Proxy => "proxy: defineProperty exception",
        ObjectKind::TypedArray => match super::typed::numeric_index(heap, key) {
            Some(index) if index.fract() != 0.0 || !index.is_finite() => {
                "non integer index in typed array"
            }
            Some(index) if index.is_sign_negative() => "negative index in typed array",
            Some(index) if index >= super::typed::length(heap, object).unwrap_or(0) as f64 => {
                "out-of-bound index in typed array"
            }
            Some(_) => "invalid descriptor flags",
            None => cannot_define_ordinary(cx, object, key),
        },
        _ => cannot_define_ordinary(cx, object, key),
    };
    error::type_error(cx, message)
}

/// [`cannot_define`]'s reason on an object that is neither a proxy nor a
/// typed array index.
fn cannot_define_ordinary(cx: &Cx<'_>, object: ObjectId, key: PropertyKey) -> &'static str {
    let heap = &*cx.heap;
    let header = *heap.header(object);
    if property::own(heap, &cx.realm.names, object, key).is_some() {
        return "property is not configurable";
    }
    if !header.has(crate::heap::object::flags::EXTENSIBLE) {
        return "object is not extensible";
    }
    if header.kind == ObjectKind::Array
        && header.has(crate::heap::object::flags::LENGTH_READONLY)
        && array_index(heap, key).is_some_and(|index| index >= header.payload)
    {
        return "'length' is read-only";
    }
    "property is not configurable"
}

/// `Set(O, P, V, Throw)` on an object.
pub(crate) fn put(
    cx: &mut Cx<'_>,
    object: ObjectId,
    key: PropertyKey,
    value: Value,
    throw: bool,
) -> Result<(), Throw> {
    if cx.heap.object_kind(object).is_exotic() {
        let done = set(cx, object, key, value, Value::object(object))?;
        if !done && throw {
            return Err(property::set_failed(cx, object, key));
        }
        return Ok(());
    }
    property::set_on(cx, object, key, value, throw)
}

/// `DeletePropertyOrThrow`.
pub(crate) fn delete_or_throw(
    cx: &mut Cx<'_>,
    object: ObjectId,
    key: PropertyKey,
) -> Result<(), Throw> {
    if delete(cx, object, key)? {
        Ok(())
    } else {
        Err(error::type_error(cx, "could not delete property"))
    }
}

/// `Get(O, P)` on an object.
#[inline]
pub(crate) fn get_property(
    cx: &mut Cx<'_>,
    object: ObjectId,
    key: PropertyKey,
) -> Result<Value, Throw> {
    property::get(cx, Value::object(object), key)
}

/// `HasOwnProperty`.
pub(crate) fn has_own_property(
    cx: &mut Cx<'_>,
    object: ObjectId,
    key: PropertyKey,
) -> Result<bool, Throw> {
    Ok(get_own_property(cx, object, key)?.is_some())
}

/// `LengthOfArrayLike`: `ToLength(Get(O, "length"))`.
pub(crate) fn length_of_array_like(cx: &mut Cx<'_>, object: ObjectId) -> Result<u64, Throw> {
    if cx.heap.object_kind(object) == ObjectKind::Array {
        return Ok(u64::from(cx.heap.array_length(object)));
    }
    let length = get_property(cx, object, Names::key(cx.realm.names.length))?;
    to_length(cx, length)
}

/// `ToLength`.
pub(crate) fn to_length(cx: &mut Cx<'_>, value: Value) -> Result<u64, Throw> {
    if let Some(int) = value.as_int32() {
        return Ok(int.max(0) as u64);
    }
    let number = to_integer_or_infinity(cx, value)?;
    Ok(if number <= 0.0 {
        0
    } else {
        number.min(9007199254740991.0) as u64
    })
}

/// `ToIntegerOrInfinity`.
pub(crate) fn to_integer_or_infinity(cx: &mut Cx<'_>, value: Value) -> Result<f64, Throw> {
    if let Some(int) = value.as_int32() {
        return Ok(f64::from(int));
    }
    let number = ops::to_number(cx, value)?;
    Ok(if number.is_nan() {
        0.0
    } else {
        // `trunc` keeps -0; ToIntegerOrInfinity returns +0 for it.
        number.trunc() + 0.0
    })
}

/// `ToObject`.
pub(crate) fn to_object(cx: &mut Cx<'_>, value: Value) -> Result<ObjectId, Throw> {
    if let Some(object) = value.as_object() {
        return Ok(object);
    }
    let (kind, prototype) = match value.kind() {
        Kind::Undefined | Kind::Null | Kind::Empty => {
            return Err(error::type_error(cx, "cannot convert to object"));
        }
        Kind::String => (ObjectKind::StringWrapper, Intrinsic::StringPrototype),
        Kind::Number | Kind::Int32 => (ObjectKind::NumberWrapper, Intrinsic::NumberPrototype),
        Kind::Bool => (ObjectKind::BooleanWrapper, Intrinsic::BooleanPrototype),
        Kind::Symbol => (ObjectKind::SymbolWrapper, Intrinsic::SymbolPrototype),
        Kind::BigInt => (ObjectKind::BigIntWrapper, Intrinsic::BigIntPrototype),
        Kind::Object => unreachable!(),
    };
    let prototype = cx.realm.intrinsic(prototype);
    wrap(cx, kind, value, prototype)
}

/// A primitive wrapper of `kind` holding `value`, with `prototype`.
pub(crate) fn wrap(
    cx: &mut Cx<'_>,
    kind: ObjectKind,
    value: Value,
    prototype: ObjectId,
) -> Result<ObjectId, Throw> {
    let object = cx
        .heap
        .alloc_internal(
            kind,
            InternalTag::None,
            Some(prototype),
            1,
            &[value],
            Default::default(),
            0,
        )
        .map_err(|error| heap_error(cx, error))?;
    if kind == ObjectKind::StringWrapper {
        let length = cx.heap.string_len(value.as_string().unwrap());
        let key = Names::key(cx.realm.names.length);
        cx.heap
            .add_property(
                object,
                key,
                PropertyFlags::data(false, false, false),
                length_value(length as u32),
            )
            .map_err(|error| heap_error(cx, error))?;
    }
    Ok(object)
}

/// `GetMethod(V, P)`: `undefined` and `null` read as absent; anything else
/// not callable throws.
pub(crate) fn get_method(
    cx: &mut Cx<'_>,
    value: Value,
    key: PropertyKey,
) -> Result<Option<Value>, Throw> {
    let symbols = &cx.realm.symbols;
    let iteration =
        key == WellKnown::key(symbols.iterator) || key == WellKnown::key(symbols.async_iterator);
    let message = if iteration {
        "value is not iterable"
    } else {
        "not a function"
    };
    get_method_or(cx, value, key, message)
}

/// [`get_method`] throwing `message` (a TypeError) for a method that is
/// not callable.
pub(crate) fn get_method_or(
    cx: &mut Cx<'_>,
    value: Value,
    key: PropertyKey,
    message: &str,
) -> Result<Option<Value>, Throw> {
    let method = property::get(cx, value, key)?;
    if method.is_undefined() || method.is_null() {
        return Ok(None);
    }
    if !ops::is_callable(cx.heap, method) {
        return Err(error::type_error(cx, message));
    }
    Ok(Some(method))
}

/// `Invoke(V, P, args)`.
pub(crate) fn invoke(
    cx: &mut Cx<'_>,
    value: Value,
    key: PropertyKey,
    args: &[Value],
) -> Result<Value, Throw> {
    let mark = cx.heap.roots().mark();
    for &arg in args {
        root(cx, arg);
    }
    let result = (|| {
        let function = property::get(cx, value, key)?;
        super::call(cx, function, value, args)
    })();
    cx.heap.roots().truncate(mark);
    result
}

/// `IsConstructor`.
#[inline]
pub(crate) fn is_constructor(cx: &Cx<'_>, value: Value) -> bool {
    value
        .as_object()
        .is_some_and(|object| cx.heap.header(object).has(flags::CONSTRUCTOR))
}

/// `IsArray`: an array, or a proxy whose target is one.
pub(crate) fn is_array(cx: &mut Cx<'_>, value: Value) -> Result<bool, Throw> {
    let Some(mut object) = value.as_object() else {
        return Ok(false);
    };
    loop {
        match cx.heap.object_kind(object) {
            ObjectKind::Array => return Ok(true),
            ObjectKind::Proxy => {
                let handler = cx.heap.internal_slot(object, 1);
                if handler.is_null() {
                    return Err(error::type_error(cx, "revoked proxy"));
                }
                object = cx
                    .heap
                    .internal_slot(object, 0)
                    .as_object()
                    .expect("a target");
            }
            _ => return Ok(false),
        }
    }
}

/// `GetPrototypeFromConstructor(newTarget, default)`.
pub(crate) fn prototype_from_constructor(
    cx: &mut Cx<'_>,
    new_target: Value,
    default: Intrinsic,
) -> Result<ObjectId, Throw> {
    if let Some(constructor) = new_target.as_object() {
        let prototype = get_property(cx, constructor, Names::key(cx.realm.names.prototype))?;
        if let Some(prototype) = prototype.as_object() {
            return Ok(prototype);
        }
        function_realm(cx, constructor)?;
    }
    Ok(cx.realm.intrinsic(default))
}

/// `GetFunctionRealm`: one realm, so only its revoked-proxy `TypeError`
/// is observable.
fn function_realm(cx: &mut Cx<'_>, mut function: ObjectId) -> Result<(), Throw> {
    loop {
        match cx.heap.object_kind(function) {
            ObjectKind::Proxy => {
                if cx.heap.internal_slot(function, 1).is_null() {
                    return Err(error::type_error(cx, "revoked proxy"));
                }
                function = cx.heap.internal_slot(function, 0).as_object().unwrap();
            }
            ObjectKind::BoundFunction => {
                function = cx.heap.internal_slot(function, 0).as_object().unwrap();
            }
            _ => return Ok(()),
        }
    }
}

/// `OrdinaryHasInstance(C, O)`.
pub(crate) fn ordinary_has_instance(
    cx: &mut Cx<'_>,
    constructor: Value,
    value: Value,
) -> Result<bool, Throw> {
    if !ops::is_callable(cx.heap, constructor) {
        return Ok(false);
    }
    let constructor = constructor.as_object().unwrap();
    if cx.heap.object_kind(constructor) == ObjectKind::BoundFunction {
        let target = cx.heap.internal_slot(constructor, 0);
        return ops::instance_of(cx, value, target);
    }
    let Some(mut object) = value.as_object() else {
        return Ok(false);
    };
    let prototype = get_property(cx, constructor, Names::key(cx.realm.names.prototype))?;
    let Some(prototype) = prototype.as_object() else {
        return Err(error::type_error(
            cx,
            "operand 'prototype' property is not an object",
        ));
    };
    loop {
        let next = get_prototype_of(cx, object)?;
        match next.as_object() {
            None => return Ok(false),
            Some(next) if next == prototype => return Ok(true),
            Some(next) => object = next,
        }
    }
}

/// `CreateListFromArrayLike`: the values, rooted on the root stack.
pub(crate) fn list_from_array_like(cx: &mut Cx<'_>, value: Value) -> Result<Vec<Value>, Throw> {
    let Some(object) = value.as_object() else {
        return Err(error::type_error(cx, "not a object"));
    };
    let length = length_of_array_like(cx, object)?;
    if length > 65_535 * 16 {
        return Err(error::range_error(
            cx,
            "too many arguments in function call (only 65534 allowed)",
        ));
    }
    let mut list = Vec::with_capacity(length as usize);
    for index in 0..length as u32 {
        let key = index_key(cx, index)?;
        let element = get_property(cx, object, key)?;
        root(cx, element);
        list.push(element);
    }
    Ok(list)
}

/// `CreateArrayFromList`.
pub(crate) fn array_from_list(cx: &mut Cx<'_>, list: &[Value]) -> Result<ObjectId, Throw> {
    let prototype = cx.realm.intrinsic(Intrinsic::ArrayPrototype);
    let mark = cx.heap.roots().mark();
    for &value in list {
        root(cx, value);
    }
    let result = cx.heap.alloc_array(Some(prototype), list.len() as u32);
    cx.heap.roots().truncate(mark);
    let array = result.map_err(|error| heap_error(cx, error))?;
    let heap = &mut *cx.heap;
    let elements = heap.header(array).elements();
    for (index, &value) in list.iter().enumerate() {
        heap.values.set(elements, index as u32, value);
    }
    heap.set_array_length(array, list.len() as u32);
    Ok(array)
}

/// `ArrayCreate(length)` with `%Array.prototype%`.
pub(crate) fn array_create(cx: &mut Cx<'_>, length: u64) -> Result<ObjectId, Throw> {
    if length > u64::from(u32::MAX) {
        return Err(error::range_error(cx, "invalid array length"));
    }
    let prototype = cx.realm.intrinsic(Intrinsic::ArrayPrototype);
    let capacity = (length as u32).min(1 << 16);
    let array = cx
        .heap
        .alloc_array(Some(prototype), capacity)
        .map_err(|error| heap_error(cx, error))?;
    cx.heap.set_array_length(array, length as u32);
    Ok(array)
}

// ---- descriptors as objects ----

/// `ToPropertyDescriptor`. The descriptor's values are rooted.
pub(crate) fn to_property_descriptor(cx: &mut Cx<'_>, value: Value) -> Result<Descriptor, Throw> {
    let Some(object) = value.as_object() else {
        return Err(error::type_error(cx, "not an object"));
    };
    let names = cx.realm.names;
    let mut descriptor = Descriptor::default();
    let field = |cx: &mut Cx<'_>, name| -> Result<Option<Value>, Throw> {
        let key = Names::key(name);
        if !has_property(cx, object, key)? {
            return Ok(None);
        }
        let value = get_property(cx, object, key)?;
        root(cx, value);
        Ok(Some(value))
    };
    if let Some(value) = field(cx, names.enumerable)? {
        descriptor.enumerable = Some(ops::to_boolean(cx.heap, value));
    }
    if let Some(value) = field(cx, names.configurable)? {
        descriptor.configurable = Some(ops::to_boolean(cx.heap, value));
    }
    descriptor.value = field(cx, names.value)?;
    if let Some(value) = field(cx, names.writable)? {
        descriptor.writable = Some(ops::to_boolean(cx.heap, value));
    }
    if let Some(getter) = field(cx, names.get)? {
        if !getter.is_undefined() && !ops::is_callable(cx.heap, getter) {
            return Err(error::type_error(cx, "invalid getter"));
        }
        descriptor.get = Some(getter);
    }
    if let Some(setter) = field(cx, names.set)? {
        if !setter.is_undefined() && !ops::is_callable(cx.heap, setter) {
            return Err(error::type_error(cx, "invalid setter"));
        }
        descriptor.set = Some(setter);
    }
    if descriptor.is_accessor() && descriptor.is_data() {
        return Err(error::type_error(
            cx,
            "cannot have setter/getter and value or writable",
        ));
    }
    Ok(descriptor)
}

/// `FromPropertyDescriptor`.
pub(crate) fn from_property_descriptor(
    cx: &mut Cx<'_>,
    descriptor: &Descriptor,
) -> Result<Value, Throw> {
    let prototype = cx.realm.intrinsic(Intrinsic::ObjectPrototype);
    let mark = cx.heap.roots().mark();
    descriptor.root(cx);
    let result = (|| {
        let object = cx
            .heap
            .alloc_object(ObjectKind::Ordinary, Some(prototype), 4)
            .map_err(|error| heap_error(cx, error))?;
        root(cx, Value::object(object));
        let names = cx.realm.names;
        let mut fields = Vec::new();
        if let Some(value) = descriptor.value {
            fields.push((names.value, value));
        }
        if let Some(writable) = descriptor.writable {
            fields.push((names.writable, Value::bool(writable)));
        }
        if let Some(get) = descriptor.get {
            fields.push((names.get, get));
        }
        if let Some(set) = descriptor.set {
            fields.push((names.set, set));
        }
        if let Some(enumerable) = descriptor.enumerable {
            fields.push((names.enumerable, Value::bool(enumerable)));
        }
        if let Some(configurable) = descriptor.configurable {
            fields.push((names.configurable, Value::bool(configurable)));
        }
        for (name, value) in fields {
            cx.heap
                .add_property(object, Names::key(name), PropertyFlags::DEFAULT, value)
                .map_err(|error| heap_error(cx, error))?;
        }
        Ok(Value::object(object))
    })();
    cx.heap.roots().truncate(mark);
    result
}

// ---- integrity levels ----

/// `SetIntegrityLevel(O, sealed | frozen)`.
pub(crate) fn set_integrity_level(
    cx: &mut Cx<'_>,
    object: ObjectId,
    frozen: bool,
) -> Result<bool, Throw> {
    if !prevent_extensions(cx, object)? {
        return Ok(false);
    }
    let mark = cx.heap.roots().mark();
    let result = (|| {
        let keys = own_property_keys(cx, object)?;
        for key in keys {
            let descriptor = if frozen {
                match get_own_property(cx, object, key)? {
                    Some(current) if current.is_accessor() => Descriptor {
                        configurable: Some(false),
                        ..Descriptor::default()
                    },
                    Some(_) => Descriptor {
                        configurable: Some(false),
                        writable: Some(false),
                        ..Descriptor::default()
                    },
                    None => continue,
                }
            } else {
                Descriptor {
                    configurable: Some(false),
                    ..Descriptor::default()
                }
            };
            define_property_or_throw(cx, object, key, &descriptor)?;
        }
        Ok(true)
    })();
    cx.heap.roots().truncate(mark);
    result
}

/// `TestIntegrityLevel(O, sealed | frozen)`.
pub(crate) fn test_integrity_level(
    cx: &mut Cx<'_>,
    object: ObjectId,
    frozen: bool,
) -> Result<bool, Throw> {
    if is_extensible(cx, object)? {
        return Ok(false);
    }
    let mark = cx.heap.roots().mark();
    let result = (|| {
        for key in own_property_keys(cx, object)? {
            if let Some(current) = get_own_property(cx, object, key)? {
                if current.configurable == Some(true) {
                    return Ok(false);
                }
                if frozen && current.is_data() && current.writable == Some(true) {
                    return Ok(false);
                }
            }
        }
        Ok(true)
    })();
    cx.heap.roots().truncate(mark);
    result
}

/// `EnumerableOwnProperties(O, kind)` keys: the enumerable own string keys,
/// in order, rooted.
pub(crate) fn enumerable_own_keys(
    cx: &mut Cx<'_>,
    object: ObjectId,
) -> Result<Vec<PropertyKey>, Throw> {
    let mut keys = Vec::new();
    for key in own_property_keys(cx, object)? {
        if matches!(key.kind(), KeyKind::Symbol(_)) {
            continue;
        }
        if let Some(descriptor) = get_own_property(cx, object, key)?
            && descriptor.enumerable == Some(true)
        {
            keys.push(key);
        }
    }
    Ok(keys)
}

/// `CopyDataProperties(target, source, excluded)`.
pub(crate) fn copy_data_properties(
    cx: &mut Cx<'_>,
    target: ObjectId,
    source: Value,
    excluded: &[PropertyKey],
) -> Result<(), Throw> {
    if source.is_undefined() || source.is_null() {
        return Ok(());
    }
    let mark = cx.heap.roots().mark();
    let result = (|| {
        let from = to_object(cx, source)?;
        root(cx, Value::object(from));
        for key in own_property_keys(cx, from)? {
            if excluded.contains(&key) {
                continue;
            }
            if let Some(descriptor) = get_own_property(cx, from, key)?
                && descriptor.enumerable == Some(true)
            {
                let value = get(cx, from, key, Value::object(from))?;
                create_data_property_or_throw(cx, target, key, value)?;
            }
        }
        Ok(())
    })();
    cx.heap.roots().truncate(mark);
    result
}

/// `SpeciesConstructor` is not ported (phase 0 dropped `Symbol.species`):
/// the intrinsic constructor is used.
#[allow(dead_code)]
pub(crate) const SPECIES_DROPPED: () = ();

/// An `InternalError` for a construct the new core does not support.
#[cold]
pub(crate) fn unsupported(cx: &mut Cx<'_>, construct: &str) -> Throw {
    error::throw(
        cx,
        ErrorKind::Internal,
        &format!("unsupported: {construct}"),
    )
}
