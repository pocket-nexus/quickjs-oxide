//! `Map`, `Set`, `WeakMap`, `WeakSet`, `WeakRef` and `FinalizationRegistry`
//! (phase 4, section 6.1 row 10), over the ordered table of
//! `heap::table` and the collector's weak references (section 3.4).
//!
//! A collection is an internal record (tags `Map`, `Set`, `WeakMap`,
//! `WeakSet`) whose slots are its entries. A collection iterator's slots
//! are the collection and its kind, its data the cursor. A `WeakRef`'s slot
//! 0 is its target; a registry's slot 0 is the cleanup callback, then a cell
//! of three slots (target, held value, unregister token) per registration.

use std::cell::Cell;
use std::rc::Rc;

use super::{Native, accessor, getter, install_constructor, methods, to_string_tag};
use crate::PropertyKey;
use crate::heap::HeapError;
use crate::heap::object::{InternalData, InternalTag, ObjectKind};
use crate::heap::table::Table;
use crate::runtime::{Intrinsic, Names, WellKnown};
use crate::value::{Kind, ObjectId, Value};
use crate::vm::native::CallSite;
use crate::vm::object as internal;
use crate::vm::{Cx, Throw, error, iter, ops};

/// What a collection iterator yields.
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
enum IterKind {
    Keys = 0,
    Values = 1,
    Entries = 2,
}

pub(super) fn install(cx: &mut Cx<'_>) -> Result<(), HeapError> {
    let object_prototype = cx.realm.intrinsic(Intrinsic::ObjectPrototype);
    let iterator_prototype = cx.realm.intrinsic(Intrinsic::IteratorPrototype);
    let species = WellKnown::key(cx.realm.symbols.species);
    let iterator = WellKnown::key(cx.realm.symbols.iterator);

    // Map.
    let prototype = super::plain_object(cx, Some(object_prototype), 16)?;
    cx.set_intrinsic(Intrinsic::MapPrototype, prototype);
    let map = install_constructor(cx, Native::MapConstructor, prototype)?;
    methods(cx, map, &[Native::MapGroupBy])?;
    accessor(
        cx,
        map,
        species,
        "[Symbol.species]",
        Some(Native::SpeciesGetter),
        None,
    )?;
    methods(
        cx,
        prototype,
        &[
            Native::MapSet,
            Native::MapGet,
            Native::MapGetOrInsert,
            Native::MapGetOrInsertComputed,
            Native::MapHas,
            Native::MapDelete,
            Native::MapClear,
        ],
    )?;
    getter(cx, prototype, "size", Native::MapSize)?;
    methods(
        cx,
        prototype,
        &[
            Native::MapForEach,
            Native::MapValues,
            Native::MapKeys,
            Native::MapEntries,
        ],
    )?;
    alias(cx, prototype, "entries", iterator)?;
    to_string_tag(cx, prototype, "Map")?;
    let map_iterator = super::plain_object(cx, Some(iterator_prototype), 2)?;
    cx.set_intrinsic(Intrinsic::MapIteratorPrototype, map_iterator);
    methods(cx, map_iterator, &[Native::MapIteratorNext])?;
    to_string_tag(cx, map_iterator, "Map Iterator")?;

    // Set.
    let prototype = super::plain_object(cx, Some(object_prototype), 24)?;
    cx.set_intrinsic(Intrinsic::SetPrototype, prototype);
    let set = install_constructor(cx, Native::SetConstructor, prototype)?;
    // QuickJS installs `Set.groupBy`, which groups into a Map.
    methods(cx, set, &[Native::MapGroupBy])?;
    accessor(
        cx,
        set,
        species,
        "[Symbol.species]",
        Some(Native::SpeciesGetter),
        None,
    )?;
    methods(
        cx,
        prototype,
        &[
            Native::SetAdd,
            Native::SetHas,
            Native::SetDelete,
            Native::SetClear,
        ],
    )?;
    getter(cx, prototype, "size", Native::SetSize)?;
    methods(
        cx,
        prototype,
        &[
            Native::SetForEach,
            Native::SetIsDisjointFrom,
            Native::SetIsSubsetOf,
            Native::SetIsSupersetOf,
            Native::SetIntersection,
            Native::SetDifference,
            Native::SetSymmetricDifference,
            Native::SetUnion,
            Native::SetValues,
        ],
    )?;
    let keys = cx.heap.property_key("keys")?;
    alias(cx, prototype, "values", keys)?;
    alias(cx, prototype, "values", iterator)?;
    methods(cx, prototype, &[Native::SetEntries])?;
    to_string_tag(cx, prototype, "Set")?;
    let set_iterator = super::plain_object(cx, Some(iterator_prototype), 2)?;
    cx.set_intrinsic(Intrinsic::SetIteratorPrototype, set_iterator);
    methods(cx, set_iterator, &[Native::SetIteratorNext])?;
    to_string_tag(cx, set_iterator, "Set Iterator")?;

    // WeakMap and WeakSet.
    let prototype = super::plain_object(cx, Some(object_prototype), 8)?;
    cx.set_intrinsic(Intrinsic::WeakMapPrototype, prototype);
    install_constructor(cx, Native::WeakMapConstructor, prototype)?;
    methods(
        cx,
        prototype,
        &[
            Native::WeakMapSet,
            Native::WeakMapGet,
            Native::WeakMapGetOrInsert,
            Native::WeakMapGetOrInsertComputed,
            Native::WeakMapHas,
            Native::WeakMapDelete,
        ],
    )?;
    to_string_tag(cx, prototype, "WeakMap")?;
    let prototype = super::plain_object(cx, Some(object_prototype), 4)?;
    cx.set_intrinsic(Intrinsic::WeakSetPrototype, prototype);
    install_constructor(cx, Native::WeakSetConstructor, prototype)?;
    methods(
        cx,
        prototype,
        &[
            Native::WeakSetAdd,
            Native::WeakSetHas,
            Native::WeakSetDelete,
        ],
    )?;
    to_string_tag(cx, prototype, "WeakSet")?;

    // WeakRef and FinalizationRegistry.
    let prototype = super::plain_object(cx, Some(object_prototype), 2)?;
    cx.set_intrinsic(Intrinsic::WeakRefPrototype, prototype);
    install_constructor(cx, Native::WeakRefConstructor, prototype)?;
    methods(cx, prototype, &[Native::WeakRefDeref])?;
    to_string_tag(cx, prototype, "WeakRef")?;
    let prototype = super::plain_object(cx, Some(object_prototype), 3)?;
    cx.set_intrinsic(Intrinsic::FinalizationRegistryPrototype, prototype);
    install_constructor(cx, Native::FinalizationRegistryConstructor, prototype)?;
    methods(
        cx,
        prototype,
        &[
            Native::FinalizationRegistryRegister,
            Native::FinalizationRegistryUnregister,
        ],
    )?;
    to_string_tag(cx, prototype, "FinalizationRegistry")
}

/// Defines `key` of `object` as the function already at `name`.
fn alias(cx: &mut Cx<'_>, object: ObjectId, name: &str, key: PropertyKey) -> Result<(), HeapError> {
    let from = cx.heap.property_key(name)?;
    let (slot, _) = cx
        .heap
        .lookup_own(object, from)
        .expect("an installed method");
    let function = cx.heap.get_slot(object, slot);
    super::define_key(cx, object, key, function, super::hidden())
}

// ---- records ----

fn root(cx: &mut Cx<'_>, value: Value) {
    cx.heap.roots().push(value);
}

/// A new collection of kind `tag` inheriting from `prototype`, rooted.
fn new_collection(
    cx: &mut Cx<'_>,
    tag: InternalTag,
    prototype: ObjectId,
) -> Result<ObjectId, Throw> {
    let width = if matches!(tag, InternalTag::Map | InternalTag::WeakMap) {
        2
    } else {
        1
    };
    let object = cx
        .heap
        .alloc_internal(
            ObjectKind::Builtin,
            tag,
            Some(prototype),
            0,
            &[],
            InternalData::Table(Box::new(Table::new(width))),
            0,
        )
        .map_err(|error| error::heap_error(cx, error))?;
    root(cx, Value::object(object));
    Ok(object)
}

/// The receiver as a collection of kind `tag`.
fn this_collection(
    cx: &mut Cx<'_>,
    site: &CallSite,
    tag: InternalTag,
    name: &str,
) -> Result<ObjectId, Throw> {
    let this = site.this(cx);
    match this.as_object() {
        Some(object) if cx.heap.internal_tag(object) == tag => Ok(object),
        _ => Err(error::type_error(cx, &format!("{name} object expected"))),
    }
}

fn set_entry(cx: &mut Cx<'_>, object: ObjectId, key: Value, value: Value) -> Result<(), Throw> {
    cx.heap
        .table_set(object, key, value)
        .map_err(|error| error::heap_error(cx, error))
}

/// `-0` as `+0`, as a key is stored.
fn canonical(key: Value) -> Value {
    if key.as_f64() == Some(0.0) {
        Value::int32(0)
    } else {
        key
    }
}

/// Whether `value` is the native function `native`.
fn is_native(cx: &Cx<'_>, value: Value, native: Native) -> bool {
    value.as_object().is_some_and(|object| {
        cx.heap.object_kind(object) == ObjectKind::Function
            && cx.heap.function_record(object).native == native.index()
    })
}

/// `CanBeHeldWeakly(value)`: an object or a symbol not in the registry.
fn can_be_held_weakly(cx: &Cx<'_>, value: Value) -> bool {
    match value.kind() {
        Kind::Object => true,
        Kind::Symbol => {
            let symbol = value.as_symbol().unwrap();
            let registered = cx
                .heap
                .symbol_description(symbol)
                .is_some_and(|description| {
                    let units = ops::units(cx.heap, description);
                    cx.host.symbol_registry.get(&units) == Some(&symbol)
                });
            !registered && !cx.heap.symbol(symbol).private
        }
        _ => false,
    }
}

// ---- constructors ----

/// `Map(iterable)` and `Set(iterable)`: `AddEntriesFromIterable` through the
/// new object's `set` or `add`, or straight into the table when that is the
/// intrinsic method.
fn construct_collection(
    cx: &mut Cx<'_>,
    site: &CallSite,
    tag: InternalTag,
    default: Intrinsic,
    adder_name: &str,
    intrinsic_adder: Native,
) -> Result<Value, Throw> {
    if !site.construct {
        return Err(error::type_error(cx, "constructor requires 'new'"));
    }
    let prototype = internal::prototype_from_constructor(cx, site.new_target, default)?;
    let object = new_collection(cx, tag, prototype)?;
    let iterable = site.arg(cx, 0);
    if iterable.is_undefined() || iterable.is_null() {
        return Ok(Value::object(object));
    }
    let key = cx
        .heap
        .property_key(adder_name)
        .map_err(|error| error::heap_error(cx, error))?;
    let adder = internal::get_property(cx, object, key)?;
    if !ops::is_callable(cx.heap, adder) {
        return Err(error::type_error(cx, "set/add is not a function"));
    }
    root(cx, adder);
    let direct = is_native(cx, adder, intrinsic_adder);
    let pairs = matches!(tag, InternalTag::Map | InternalTag::WeakMap);
    let mut iterator = iter::get_iterator(cx, iterable)?;
    while let Some(item) = iter::step(cx, &mut iterator)? {
        let mark = cx.heap.roots().mark();
        root(cx, item);
        let result = (|| {
            if pairs {
                let Some(entry) = item.as_object() else {
                    return Err(error::type_error(cx, "iterator value is not an object"));
                };
                let key = internal::get_property(cx, entry, PropertyKey::index(0).unwrap())?;
                root(cx, key);
                let value = internal::get_property(cx, entry, PropertyKey::index(1).unwrap())?;
                root(cx, value);
                if direct && tag == InternalTag::Map {
                    set_entry(cx, object, key, value)
                } else {
                    crate::vm::call(cx, adder, Value::object(object), &[key, value]).map(drop)
                }
            } else if direct && tag == InternalTag::Set {
                set_entry(cx, object, canonical(item), Value::EMPTY)
            } else {
                crate::vm::call(cx, adder, Value::object(object), &[item]).map(drop)
            }
        })();
        cx.heap.roots().truncate(mark);
        if let Err(throw) = result {
            iter::close(cx, &iterator, Err(throw))?;
            unreachable!("close rethrows");
        }
    }
    Ok(Value::object(object))
}

pub(super) fn map_constructor(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    construct_collection(
        cx,
        &site,
        InternalTag::Map,
        Intrinsic::MapPrototype,
        "set",
        Native::MapSet,
    )
}

pub(super) fn set_constructor(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    construct_collection(
        cx,
        &site,
        InternalTag::Set,
        Intrinsic::SetPrototype,
        "add",
        Native::SetAdd,
    )
}

pub(super) fn weak_map_constructor(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    construct_collection(
        cx,
        &site,
        InternalTag::WeakMap,
        Intrinsic::WeakMapPrototype,
        "set",
        Native::WeakMapSet,
    )
}

pub(super) fn weak_set_constructor(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    construct_collection(
        cx,
        &site,
        InternalTag::WeakSet,
        Intrinsic::WeakSetPrototype,
        "add",
        Native::WeakSetAdd,
    )
}

/// `Map.groupBy(items, callback)` (also `Set.groupBy` in QuickJS).
pub(super) fn group_by(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let items = site.arg(cx, 0);
    let callback = site.arg(cx, 1);
    let groups = super::object::group(cx, items, callback, false)?;
    let prototype = cx.realm.intrinsic(Intrinsic::MapPrototype);
    let map = new_collection(cx, InternalTag::Map, prototype)?;
    for (key, members) in groups {
        let array = internal::array_from_list(cx, &members)?;
        root(cx, Value::object(array));
        set_entry(cx, map, key, Value::object(array))?;
    }
    Ok(Value::object(map))
}

// ---- Map.prototype ----

pub(super) fn map_get(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let map = this_collection(cx, &site, InternalTag::Map, "Map")?;
    let key = site.arg(cx, 0);
    Ok(cx.heap.table_get(map, key).unwrap_or(Value::UNDEFINED))
}

pub(super) fn map_set(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let map = this_collection(cx, &site, InternalTag::Map, "Map")?;
    let (key, value) = (site.arg(cx, 0), site.arg(cx, 1));
    set_entry(cx, map, key, value)?;
    Ok(Value::object(map))
}

pub(super) fn map_has(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let map = this_collection(cx, &site, InternalTag::Map, "Map")?;
    let key = site.arg(cx, 0);
    Ok(Value::bool(cx.heap.table_find(map, key).is_some()))
}

pub(super) fn map_delete(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let map = this_collection(cx, &site, InternalTag::Map, "Map")?;
    let key = site.arg(cx, 0);
    Ok(Value::bool(cx.heap.table_delete(map, key)))
}

pub(super) fn map_clear(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let map = this_collection(cx, &site, InternalTag::Map, "Map")?;
    cx.heap.table_clear(map);
    Ok(Value::UNDEFINED)
}

pub(super) fn map_size(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let map = this_collection(cx, &site, InternalTag::Map, "Map")?;
    Ok(Value::int32(cx.heap.table(map).len() as i32))
}

/// `getOrInsert(key, value)`.
pub(super) fn map_get_or_insert(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let map = this_collection(cx, &site, InternalTag::Map, "Map")?;
    get_or_insert(cx, &site, map)
}

fn get_or_insert(cx: &mut Cx<'_>, site: &CallSite, map: ObjectId) -> Result<Value, Throw> {
    let (key, value) = (site.arg(cx, 0), site.arg(cx, 1));
    if let Some(existing) = cx.heap.table_get(map, key) {
        return Ok(existing);
    }
    set_entry(cx, map, key, value)?;
    Ok(value)
}

/// `getOrInsertComputed(key, callback)`: the callback runs with the key
/// absent, and its result is stored even if the callback added the key.
pub(super) fn map_get_or_insert_computed(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let map = this_collection(cx, &site, InternalTag::Map, "Map")?;
    get_or_insert_computed(cx, &site, map)
}

fn get_or_insert_computed(cx: &mut Cx<'_>, site: &CallSite, map: ObjectId) -> Result<Value, Throw> {
    let (key, callback) = (site.arg(cx, 0), site.arg(cx, 1));
    if !ops::is_callable(cx.heap, callback) {
        return Err(error::type_error(cx, "not a function"));
    }
    if let Some(existing) = cx.heap.table_get(map, key) {
        return Ok(existing);
    }
    let key = canonical(key);
    let value = crate::vm::call(cx, callback, Value::UNDEFINED, &[key])?;
    root(cx, value);
    set_entry(cx, map, key, value)?;
    Ok(value)
}

/// `forEach(callback, thisArg)` for a map or a set: entries added during
/// the walk are visited, deleted ones are not.
fn for_each(cx: &mut Cx<'_>, site: &CallSite, object: ObjectId, set: bool) -> Result<Value, Throw> {
    let (callback, this_arg) = (site.arg(cx, 0), site.arg(cx, 1));
    if !ops::is_callable(cx.heap, callback) {
        return Err(error::type_error(cx, "not a function"));
    }
    let cursor = cx.heap.table_cursor(object);
    while let Some((key, value)) = cx.heap.table_next(object, &cursor) {
        let value = if set { key } else { value };
        let mark = cx.heap.roots().mark();
        root(cx, key);
        root(cx, value);
        let result = crate::vm::call(cx, callback, this_arg, &[value, key, Value::object(object)]);
        cx.heap.roots().truncate(mark);
        result?;
    }
    Ok(Value::UNDEFINED)
}

pub(super) fn map_for_each(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let map = this_collection(cx, &site, InternalTag::Map, "Map")?;
    for_each(cx, &site, map, false)
}

/// `CreateMapIterator` / `CreateSetIterator`.
fn new_iterator(
    cx: &mut Cx<'_>,
    object: ObjectId,
    kind: IterKind,
    tag: InternalTag,
) -> Result<Value, Throw> {
    let prototype = cx.realm.intrinsic(if tag == InternalTag::MapIterator {
        Intrinsic::MapIteratorPrototype
    } else {
        Intrinsic::SetIteratorPrototype
    });
    let cursor = cx.heap.table_cursor(object);
    cx.heap
        .alloc_internal(
            ObjectKind::Builtin,
            tag,
            Some(prototype),
            0,
            &[Value::object(object), Value::int32(kind as i32)],
            InternalData::Cursor(cursor),
            0,
        )
        .map(Value::object)
        .map_err(|error| error::heap_error(cx, error))
}

pub(super) fn map_keys(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let map = this_collection(cx, &site, InternalTag::Map, "Map")?;
    new_iterator(cx, map, IterKind::Keys, InternalTag::MapIterator)
}

pub(super) fn map_values(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let map = this_collection(cx, &site, InternalTag::Map, "Map")?;
    new_iterator(cx, map, IterKind::Values, InternalTag::MapIterator)
}

pub(super) fn map_entries(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let map = this_collection(cx, &site, InternalTag::Map, "Map")?;
    new_iterator(cx, map, IterKind::Entries, InternalTag::MapIterator)
}

/// `%MapIteratorPrototype%.next` and `%SetIteratorPrototype%.next`.
fn iterator_next(cx: &mut Cx<'_>, site: &CallSite, tag: InternalTag) -> Result<Value, Throw> {
    let this = site.this(cx);
    let iterator = match this.as_object() {
        Some(object) if cx.heap.internal_tag(object) == tag => object,
        _ => {
            return Err(error::type_error(
                cx,
                "not an iterator of the expected kind",
            ));
        }
    };
    let Some(object) = cx.heap.internal_slot(iterator, 0).as_object() else {
        return iter::result_object(cx, Value::UNDEFINED, true);
    };
    let InternalData::Cursor(cursor) = &cx.heap.internal(iterator).data else {
        unreachable!("a collection iterator");
    };
    let cursor: Rc<Cell<u32>> = cursor.clone();
    let Some((key, value)) = cx.heap.table_next(object, &cursor) else {
        cx.heap.set_internal_slot(iterator, 0, Value::UNDEFINED);
        cx.heap.internal_mut(iterator).data = InternalData::None;
        return iter::result_object(cx, Value::UNDEFINED, true);
    };
    let kind = cx.heap.internal_slot(iterator, 1).as_int32().unwrap();
    let result = match kind {
        0 => key,
        1 if tag == InternalTag::MapIterator => value,
        1 => key,
        _ => {
            let second = if tag == InternalTag::MapIterator {
                value
            } else {
                key
            };
            root(cx, key);
            root(cx, second);
            Value::object(internal::array_from_list(cx, &[key, second])?)
        }
    };
    iter::result_object(cx, result, false)
}

pub(super) fn map_iterator_next(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    iterator_next(cx, &site, InternalTag::MapIterator)
}

pub(super) fn set_iterator_next(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    iterator_next(cx, &site, InternalTag::SetIterator)
}

// ---- Set.prototype ----

pub(super) fn set_add(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let set = this_collection(cx, &site, InternalTag::Set, "Set")?;
    let value = site.arg(cx, 0);
    set_entry(cx, set, value, Value::EMPTY)?;
    Ok(Value::object(set))
}

pub(super) fn set_has(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let set = this_collection(cx, &site, InternalTag::Set, "Set")?;
    let value = site.arg(cx, 0);
    Ok(Value::bool(cx.heap.table_find(set, value).is_some()))
}

pub(super) fn set_delete(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let set = this_collection(cx, &site, InternalTag::Set, "Set")?;
    let value = site.arg(cx, 0);
    Ok(Value::bool(cx.heap.table_delete(set, value)))
}

pub(super) fn set_clear(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let set = this_collection(cx, &site, InternalTag::Set, "Set")?;
    cx.heap.table_clear(set);
    Ok(Value::UNDEFINED)
}

pub(super) fn set_size(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let set = this_collection(cx, &site, InternalTag::Set, "Set")?;
    Ok(Value::int32(cx.heap.table(set).len() as i32))
}

pub(super) fn set_for_each(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let set = this_collection(cx, &site, InternalTag::Set, "Set")?;
    for_each(cx, &site, set, true)
}

pub(super) fn set_values(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let set = this_collection(cx, &site, InternalTag::Set, "Set")?;
    new_iterator(cx, set, IterKind::Values, InternalTag::SetIterator)
}

pub(super) fn set_entries(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let set = this_collection(cx, &site, InternalTag::Set, "Set")?;
    new_iterator(cx, set, IterKind::Entries, InternalTag::SetIterator)
}

/// A set record (`GetSetRecord`): the other set-like's object, size and
/// `has` and `keys` methods, all rooted.
struct SetRecord {
    set: ObjectId,
    size: f64,
    has: Value,
    keys: Value,
}

fn set_record(cx: &mut Cx<'_>, other: Value) -> Result<SetRecord, Throw> {
    let Some(set) = other.as_object() else {
        return Err(error::type_error(cx, "set-like object expected"));
    };
    let names = cx.realm.names;
    let size = internal::get_property(cx, set, Names::key(names.size))?;
    let size = ops::to_number(cx, size)?;
    if size.is_nan() {
        return Err(error::type_error(cx, "size is not a number"));
    }
    let size = internal::to_integer_or_infinity(cx, Value::number(size))?;
    if size < 0.0 {
        return Err(error::range_error(cx, "invalid size"));
    }
    let key = cx
        .heap
        .property_key("has")
        .map_err(|error| error::heap_error(cx, error))?;
    let has = internal::get_property(cx, set, key)?;
    if !ops::is_callable(cx.heap, has) {
        return Err(error::type_error(cx, "has is not a function"));
    }
    root(cx, has);
    let key = cx
        .heap
        .property_key("keys")
        .map_err(|error| error::heap_error(cx, error))?;
    let keys = internal::get_property(cx, set, key)?;
    if !ops::is_callable(cx.heap, keys) {
        return Err(error::type_error(cx, "keys is not a function"));
    }
    root(cx, keys);
    Ok(SetRecord {
        set,
        size,
        has,
        keys,
    })
}

impl SetRecord {
    fn has(&self, cx: &mut Cx<'_>, value: Value) -> Result<bool, Throw> {
        let result = crate::vm::call(cx, self.has, Value::object(self.set), &[value])?;
        Ok(ops::to_boolean(cx.heap, result))
    }

    /// The iterator of `keys()`, with `next` read once.
    fn keys(&self, cx: &mut Cx<'_>) -> Result<iter::Iter, Throw> {
        let iterator = crate::vm::call(cx, self.keys, Value::object(self.set), &[])?;
        if iterator.as_object().is_none() {
            return Err(error::type_error(cx, "keys() result is not an object"));
        }
        root(cx, iterator);
        iter::direct(cx, iterator)
    }
}

/// A copy of `set`'s entries in a new `%Set%`.
fn copy_set(cx: &mut Cx<'_>, set: ObjectId) -> Result<ObjectId, Throw> {
    let prototype = cx.realm.intrinsic(Intrinsic::SetPrototype);
    let result = new_collection(cx, InternalTag::Set, prototype)?;
    let cursor = cx.heap.table_cursor(set);
    while let Some((key, _)) = cx.heap.table_next(set, &cursor) {
        set_entry(cx, result, key, Value::EMPTY)?;
    }
    Ok(result)
}

/// Visits the entries of `set` present when called, in order, also while
/// `visit` adds or deletes entries: the bound is a cursor too.
fn each_original(
    cx: &mut Cx<'_>,
    set: ObjectId,
    mut visit: impl FnMut(&mut Cx<'_>, Value) -> Result<bool, Throw>,
) -> Result<bool, Throw> {
    let cursor = cx.heap.table_cursor(set);
    let end = cx.heap.table_cursor(set);
    end.set(cx.heap.table(set).entries() as u32);
    while cursor.get() < end.get() {
        let Some((key, _)) = cx.heap.table_next(set, &cursor) else {
            break;
        };
        if cursor.get() > end.get() {
            break;
        }
        let mark = cx.heap.roots().mark();
        root(cx, key);
        let result = visit(cx, key);
        cx.heap.roots().truncate(mark);
        if !result? {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Runs `visit` over the other set's keys, closing the iterator when
/// `visit` returns false.
fn each_key(
    cx: &mut Cx<'_>,
    record: &SetRecord,
    mut visit: impl FnMut(&mut Cx<'_>, Value) -> Result<bool, Throw>,
) -> Result<bool, Throw> {
    let mut iterator = record.keys(cx)?;
    while let Some(value) = iter::step(cx, &mut iterator)? {
        let value = canonical(value);
        let mark = cx.heap.roots().mark();
        root(cx, value);
        let result = visit(cx, value);
        cx.heap.roots().truncate(mark);
        if !result? {
            iter::close(cx, &iterator, Ok(()))?;
            return Ok(false);
        }
    }
    Ok(true)
}

fn set_size_of(cx: &Cx<'_>, set: ObjectId) -> f64 {
    cx.heap.table(set).len() as f64
}

pub(super) fn set_union(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let set = this_collection(cx, &site, InternalTag::Set, "Set")?;
    let other = site.arg(cx, 0);
    let record = set_record(cx, other)?;
    let result = copy_set(cx, set)?;
    each_key(cx, &record, |cx, value| {
        set_entry(cx, result, value, Value::EMPTY).map(|()| true)
    })?;
    Ok(Value::object(result))
}

pub(super) fn set_intersection(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let set = this_collection(cx, &site, InternalTag::Set, "Set")?;
    let other = site.arg(cx, 0);
    let record = set_record(cx, other)?;
    let prototype = cx.realm.intrinsic(Intrinsic::SetPrototype);
    let result = new_collection(cx, InternalTag::Set, prototype)?;
    if set_size_of(cx, set) <= record.size {
        each_original(cx, set, |cx, value| {
            if record.has(cx, value)? {
                set_entry(cx, result, value, Value::EMPTY)?;
            }
            Ok(true)
        })?;
    } else {
        each_key(cx, &record, |cx, value| {
            if cx.heap.table_find(set, value).is_some() {
                set_entry(cx, result, value, Value::EMPTY)?;
            }
            Ok(true)
        })?;
    }
    Ok(Value::object(result))
}

pub(super) fn set_difference(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let set = this_collection(cx, &site, InternalTag::Set, "Set")?;
    let other = site.arg(cx, 0);
    let record = set_record(cx, other)?;
    let result = copy_set(cx, set)?;
    if set_size_of(cx, set) <= record.size {
        each_original(cx, set, |cx, value| {
            if record.has(cx, value)? {
                cx.heap.table_delete(result, value);
            }
            Ok(true)
        })?;
    } else {
        each_key(cx, &record, |cx, value| {
            cx.heap.table_delete(result, value);
            Ok(true)
        })?;
    }
    Ok(Value::object(result))
}

pub(super) fn set_symmetric_difference(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let set = this_collection(cx, &site, InternalTag::Set, "Set")?;
    let other = site.arg(cx, 0);
    let record = set_record(cx, other)?;
    let result = copy_set(cx, set)?;
    each_key(cx, &record, |cx, value| {
        if cx.heap.table_find(set, value).is_some() {
            cx.heap.table_delete(result, value);
        } else {
            set_entry(cx, result, value, Value::EMPTY)?;
        }
        Ok(true)
    })?;
    Ok(Value::object(result))
}

pub(super) fn set_is_subset_of(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let set = this_collection(cx, &site, InternalTag::Set, "Set")?;
    let other = site.arg(cx, 0);
    let record = set_record(cx, other)?;
    if set_size_of(cx, set) > record.size {
        return Ok(Value::FALSE);
    }
    each_original(cx, set, |cx, value| record.has(cx, value)).map(Value::bool)
}

pub(super) fn set_is_superset_of(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let set = this_collection(cx, &site, InternalTag::Set, "Set")?;
    let other = site.arg(cx, 0);
    let record = set_record(cx, other)?;
    if set_size_of(cx, set) < record.size {
        return Ok(Value::FALSE);
    }
    each_key(cx, &record, |cx, value| {
        Ok(cx.heap.table_find(set, value).is_some())
    })
    .map(Value::bool)
}

pub(super) fn set_is_disjoint_from(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let set = this_collection(cx, &site, InternalTag::Set, "Set")?;
    let other = site.arg(cx, 0);
    let record = set_record(cx, other)?;
    if set_size_of(cx, set) <= record.size {
        each_original(cx, set, |cx, value| record.has(cx, value).map(|has| !has)).map(Value::bool)
    } else {
        each_key(cx, &record, |cx, value| {
            Ok(cx.heap.table_find(set, value).is_none())
        })
        .map(Value::bool)
    }
}

// ---- WeakMap and WeakSet ----

fn weak_key(cx: &mut Cx<'_>, key: Value) -> Result<Value, Throw> {
    if can_be_held_weakly(cx, key) {
        Ok(key)
    } else {
        Err(error::type_error(cx, "invalid value used as weak key"))
    }
}

pub(super) fn weak_map_get(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let map = this_collection(cx, &site, InternalTag::WeakMap, "WeakMap")?;
    let key = site.arg(cx, 0);
    Ok(cx.heap.table_get(map, key).unwrap_or(Value::UNDEFINED))
}

pub(super) fn weak_map_set(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let map = this_collection(cx, &site, InternalTag::WeakMap, "WeakMap")?;
    let key = site.arg(cx, 0);
    let key = weak_key(cx, key)?;
    let value = site.arg(cx, 1);
    set_entry(cx, map, key, value)?;
    Ok(Value::object(map))
}

pub(super) fn weak_map_has(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let map = this_collection(cx, &site, InternalTag::WeakMap, "WeakMap")?;
    let key = site.arg(cx, 0);
    Ok(Value::bool(cx.heap.table_find(map, key).is_some()))
}

pub(super) fn weak_map_delete(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let map = this_collection(cx, &site, InternalTag::WeakMap, "WeakMap")?;
    let key = site.arg(cx, 0);
    Ok(Value::bool(cx.heap.table_delete(map, key)))
}

pub(super) fn weak_map_get_or_insert(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let map = this_collection(cx, &site, InternalTag::WeakMap, "WeakMap")?;
    let key = site.arg(cx, 0);
    weak_key(cx, key)?;
    get_or_insert(cx, &site, map)
}

pub(super) fn weak_map_get_or_insert_computed(
    cx: &mut Cx<'_>,
    site: CallSite,
) -> Result<Value, Throw> {
    let map = this_collection(cx, &site, InternalTag::WeakMap, "WeakMap")?;
    let key = site.arg(cx, 0);
    weak_key(cx, key)?;
    get_or_insert_computed(cx, &site, map)
}

pub(super) fn weak_set_add(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let set = this_collection(cx, &site, InternalTag::WeakSet, "WeakSet")?;
    let value = site.arg(cx, 0);
    let value = weak_key(cx, value)?;
    set_entry(cx, set, value, Value::EMPTY)?;
    Ok(Value::object(set))
}

pub(super) fn weak_set_has(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let set = this_collection(cx, &site, InternalTag::WeakSet, "WeakSet")?;
    let value = site.arg(cx, 0);
    Ok(Value::bool(cx.heap.table_find(set, value).is_some()))
}

pub(super) fn weak_set_delete(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let set = this_collection(cx, &site, InternalTag::WeakSet, "WeakSet")?;
    let value = site.arg(cx, 0);
    Ok(Value::bool(cx.heap.table_delete(set, value)))
}

// ---- WeakRef and FinalizationRegistry ----

pub(super) fn weak_ref_constructor(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    if !site.construct {
        return Err(error::type_error(cx, "constructor requires 'new'"));
    }
    let target = site.arg(cx, 0);
    if !can_be_held_weakly(cx, target) {
        return Err(error::type_error(cx, "invalid target"));
    }
    let prototype =
        internal::prototype_from_constructor(cx, site.new_target, Intrinsic::WeakRefPrototype)?;
    cx.heap.kept.push(target);
    cx.heap
        .alloc_internal(
            ObjectKind::Builtin,
            InternalTag::WeakRef,
            Some(prototype),
            0,
            &[target],
            InternalData::None,
            0,
        )
        .map(Value::object)
        .map_err(|error| error::heap_error(cx, error))
}

pub(super) fn weak_ref_deref(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let weak = this_collection(cx, &site, InternalTag::WeakRef, "WeakRef")?;
    let target = cx.heap.internal_slot(weak, 0);
    if !target.is_undefined() {
        cx.heap.kept.push(target);
    }
    Ok(target)
}

pub(super) fn finalization_registry_constructor(
    cx: &mut Cx<'_>,
    site: CallSite,
) -> Result<Value, Throw> {
    if !site.construct {
        return Err(error::type_error(cx, "constructor requires 'new'"));
    }
    let callback = site.arg(cx, 0);
    if !ops::is_callable(cx.heap, callback) {
        return Err(error::type_error(cx, "cleanup callback is not a function"));
    }
    let prototype = internal::prototype_from_constructor(
        cx,
        site.new_target,
        Intrinsic::FinalizationRegistryPrototype,
    )?;
    cx.heap
        .alloc_internal(
            ObjectKind::Builtin,
            InternalTag::FinalizationRegistry,
            Some(prototype),
            0,
            &[callback],
            InternalData::None,
            0,
        )
        .map(Value::object)
        .map_err(|error| error::heap_error(cx, error))
}

pub(super) fn finalization_registry_register(
    cx: &mut Cx<'_>,
    site: CallSite,
) -> Result<Value, Throw> {
    let registry = this_collection(
        cx,
        &site,
        InternalTag::FinalizationRegistry,
        "FinalizationRegistry",
    )?;
    let (target, held, token) = (site.arg(cx, 0), site.arg(cx, 1), site.arg(cx, 2));
    if !can_be_held_weakly(cx, target) {
        return Err(error::type_error(cx, "invalid target"));
    }
    if ops::same_value_zero_heap(cx.heap, target, held) {
        return Err(error::type_error(cx, "target and held value must differ"));
    }
    let token = if token.is_undefined() {
        Value::EMPTY
    } else if can_be_held_weakly(cx, token) {
        token
    } else {
        return Err(error::type_error(cx, "invalid unregister token"));
    };
    cx.heap
        .reserve_internal_slots(registry, 3, &[target, held, token])
        .map_err(|error| error::heap_error(cx, error))?;
    let slots = &mut cx.heap.internal_mut(registry).slots;
    slots.extend_from_slice(&[target, held, token]);
    Ok(Value::UNDEFINED)
}

pub(super) fn finalization_registry_unregister(
    cx: &mut Cx<'_>,
    site: CallSite,
) -> Result<Value, Throw> {
    let registry = this_collection(
        cx,
        &site,
        InternalTag::FinalizationRegistry,
        "FinalizationRegistry",
    )?;
    let token = site.arg(cx, 0);
    if !can_be_held_weakly(cx, token) {
        return Err(error::type_error(cx, "invalid unregister token"));
    }
    let slots = &mut cx.heap.internal_mut(registry).slots;
    let mut removed = false;
    for cell in slots[1..].chunks_mut(3) {
        if !cell[0].is_empty() && cell[2] == token {
            cell.fill(Value::EMPTY);
            removed = true;
        }
    }
    Ok(Value::bool(removed))
}
