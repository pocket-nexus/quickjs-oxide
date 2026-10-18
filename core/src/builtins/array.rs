//! `Array` (phase 4, section 6.1 row 2): the constructor, `from`, `of`,
//! `isArray`, and every `Array.prototype` method, in the specification's
//! step order over generic array-likes, with direct element access for a
//! plain array (an `Array` whose elements are dense and whose prototypes
//! have no index properties). `Symbol.species` is dropped (phase 0): the
//! methods that would create a species instance create a plain array.
//! `sort` is QuickJS's `rqsort` with its comparison and write-back shortcuts.

use std::cmp::Ordering;

use super::{
    Native, accessor, install_constructor, link_constructor, methods, plain_object, string_units,
    this_object,
};
use crate::heap::HeapError;
use crate::heap::atom::PropertyKey;
use crate::heap::object::{ObjectKind, flags};
use crate::runtime::{Intrinsic, Names, WellKnown};
use crate::shape::PropertyFlags;
use crate::value::{ObjectId, Value};
use crate::vm::native::CallSite;
use crate::vm::object::{self as internal};
use crate::vm::{Cx, Throw, error, iter, number, ops};

/// `new Array(n)` allocates its elements eagerly up to this length; a
/// longer one gets them as they are written.
const EAGER_ELEMENTS: u32 = 1 << 20;

/// 2^53 − 1, the largest array-like length.
const MAX_LENGTH: u64 = 9_007_199_254_740_991;

pub(super) fn install(cx: &mut Cx<'_>) -> Result<(), HeapError> {
    let prototype = cx.realm.intrinsic(Intrinsic::ArrayPrototype);
    // QuickJS's `js_array_funcs` and `js_array_proto_funcs`, in order.
    let array = install_constructor(cx, Native::ArrayConstructor)?;
    cx.set_intrinsic(Intrinsic::Array, array);
    methods(
        cx,
        array,
        &[Native::ArrayIsArray, Native::ArrayFrom, Native::ArrayOf],
    )?;
    let species = WellKnown::key(cx.realm.symbols.species);
    accessor(
        cx,
        array,
        species,
        "[Symbol.species]",
        Some(Native::SpeciesGetter),
        None,
    )?;
    methods(
        cx,
        prototype,
        &[
            Native::ArrayAt,
            Native::ArrayWith,
            Native::ArrayConcat,
            Native::ArrayEvery,
            Native::ArraySome,
            Native::ArrayForEach,
            Native::ArrayMap,
            Native::ArrayFilter,
            Native::ArrayReduce,
            Native::ArrayReduceRight,
            Native::ArrayFill,
            Native::ArrayFind,
            Native::ArrayFindIndex,
            Native::ArrayFindLast,
            Native::ArrayFindLastIndex,
            Native::ArrayIndexOf,
            Native::ArrayLastIndexOf,
            Native::ArrayIncludes,
            Native::ArrayJoin,
            Native::ArrayToString,
            Native::ArrayToLocaleString,
            Native::ArrayPop,
            Native::ArrayPush,
            Native::ArrayShift,
            Native::ArrayUnshift,
            Native::ArrayReverse,
            Native::ArrayToReversed,
            Native::ArraySort,
            Native::ArrayToSorted,
            Native::ArraySlice,
            Native::ArraySplice,
            Native::ArrayToSpliced,
            Native::ArrayCopyWithin,
            Native::ArrayFlatMap,
            Native::ArrayFlat,
            Native::ArrayValues,
            Native::ArrayKeys,
            Native::ArrayEntries,
        ],
    )?;
    let values_key = cx.heap.property_key("values")?;
    let values = cx
        .heap
        .lookup_own(prototype, values_key)
        .map(|(slot, _)| cx.heap.get_slot(prototype, slot))
        .and_then(|value| value.as_object())
        .expect("Array.prototype.values");
    cx.set_intrinsic(Intrinsic::ArrayValues, values);
    super::define_key(
        cx,
        prototype,
        WellKnown::key(cx.realm.symbols.iterator),
        Value::object(values),
        PropertyFlags::data(true, false, true),
    )?;
    // `Array.prototype[Symbol.unscopables]`: unscopables are dropped
    // (phase 0), the object is still there.
    let unscopables = plain_object(cx, None, 16)?;
    for name in [
        "at",
        "copyWithin",
        "entries",
        "fill",
        "find",
        "findIndex",
        "findLast",
        "findLastIndex",
        "flat",
        "flatMap",
        "includes",
        "keys",
        "toReversed",
        "toSorted",
        "toSpliced",
        "values",
    ] {
        super::define(cx, unscopables, name, Value::TRUE, PropertyFlags::DEFAULT)?;
    }
    super::define_key(
        cx,
        prototype,
        WellKnown::key(cx.realm.symbols.unscopables),
        Value::object(unscopables),
        PropertyFlags::data(false, false, true),
    )?;
    link_constructor(cx, array, prototype)
}

fn root(cx: &mut Cx<'_>, value: Value) {
    cx.heap.roots().push(value);
}

// ---- element access ----

/// Whether `object` is an array whose elements may be read directly: dense
/// and with no index property on any prototype.
#[inline]
fn plain(cx: &Cx<'_>, object: ObjectId) -> bool {
    let header = cx.heap.header(object);
    header.kind == ObjectKind::Array && !header.has(flags::SPARSE) && cx.heap.holes_writable()
}

/// Whether elements may also be written directly: extensible, with a
/// writable `length`.
#[inline]
fn plain_writable(cx: &Cx<'_>, object: ObjectId) -> bool {
    plain(cx, object) && {
        let header = cx.heap.header(object);
        header.has(flags::EXTENSIBLE) && !header.has(flags::LENGTH_READONLY)
    }
}

fn key_at(cx: &mut Cx<'_>, index: u64) -> Result<PropertyKey, Throw> {
    internal::number_key(cx, index as f64)
}

/// `Get(O, ToString(index))`.
fn get_at(cx: &mut Cx<'_>, object: ObjectId, index: u64) -> Result<Value, Throw> {
    if index < u64::from(u32::MAX) {
        let value = cx.heap.get_element(object, index as u32);
        if !value.is_empty() {
            return Ok(value);
        }
        if plain(cx, object) {
            return Ok(Value::UNDEFINED);
        }
    }
    let key = key_at(cx, index)?;
    internal::get_property(cx, object, key)
}

/// `HasProperty(O, ToString(index))`.
fn has_at(cx: &mut Cx<'_>, object: ObjectId, index: u64) -> Result<bool, Throw> {
    if index < u64::from(u32::MAX) {
        if !cx.heap.get_element(object, index as u32).is_empty() {
            return Ok(true);
        }
        if plain(cx, object) {
            return Ok(false);
        }
    }
    let key = key_at(cx, index)?;
    internal::has_property(cx, object, key)
}

/// `Set(O, ToString(index), value, true)`.
fn set_at(cx: &mut Cx<'_>, object: ObjectId, index: u64, value: Value) -> Result<(), Throw> {
    if index < u64::from(u32::MAX)
        && plain_writable(cx, object)
        && (index as u32) < cx.heap.array_length(object)
        && !cx.heap.get_element(object, index as u32).is_empty()
    {
        return cx
            .heap
            .set_element(object, index as u32, value)
            .map_err(|error| error::heap_error(cx, error));
    }
    let key = key_at(cx, index)?;
    internal::put(cx, object, key, value, true)
}

/// `CreateDataPropertyOrThrow(O, ToString(index), value)`.
fn create_at(cx: &mut Cx<'_>, object: ObjectId, index: u64, value: Value) -> Result<(), Throw> {
    let key = key_at(cx, index)?;
    internal::create_data_property_or_throw(cx, object, key, value)
}

/// QuickJS's `JS_TryGetPropertyInt64`: `HasProperty`, then `Get` when
/// present, else `undefined`.
fn try_get_at(cx: &mut Cx<'_>, object: ObjectId, index: u64) -> Result<Value, Throw> {
    if has_at(cx, object, index)? {
        get_at(cx, object, index)
    } else {
        Ok(Value::UNDEFINED)
    }
}

/// `DeletePropertyOrThrow(O, ToString(index))`.
fn delete_at(cx: &mut Cx<'_>, object: ObjectId, index: u64) -> Result<(), Throw> {
    let key = key_at(cx, index)?;
    internal::delete_or_throw(cx, object, key)
}

/// `Set(O, "length", length, true)`.
fn set_length(cx: &mut Cx<'_>, object: ObjectId, length: u64) -> Result<(), Throw> {
    if plain_writable(cx, object) && length <= u64::from(cx.heap.array_length(object)) {
        cx.heap.set_array_length(object, length as u32);
        return Ok(());
    }
    let key = Names::key(cx.realm.names.length);
    internal::put(cx, object, key, number::number(length as f64), true)
}

fn length_of(cx: &mut Cx<'_>, object: ObjectId) -> Result<u64, Throw> {
    internal::length_of_array_like(cx, object)
}

/// `RelativeIndex`: an integer argument against `length`, clamped to
/// `0..=length`.
fn relative_index(cx: &mut Cx<'_>, value: Value, length: u64, default: u64) -> Result<u64, Throw> {
    if value.is_undefined() {
        return Ok(default);
    }
    let relative = internal::to_integer_or_infinity(cx, value)?;
    let length = length as f64;
    Ok(if relative < 0.0 {
        (length + relative).max(0.0) as u64
    } else {
        relative.min(length) as u64
    })
}

fn callback(cx: &mut Cx<'_>, site: &CallSite) -> Result<Value, Throw> {
    let function = site.arg(cx, 0);
    if !ops::is_callable(cx.heap, function) {
        return Err(error::type_error(cx, "not a function"));
    }
    Ok(function)
}

fn index_value(index: u64) -> Value {
    number::number(index as f64)
}

/// `ArrayCreate(length)` (the species constructor is dropped).
fn array_create(cx: &mut Cx<'_>, length: u64) -> Result<ObjectId, Throw> {
    let array = internal::array_create(cx, length)?;
    root(cx, Value::object(array));
    Ok(array)
}

/// QuickJS's `js_allocate_fast_array`: the change-by-copy methods' result,
/// limited to `INT32_MAX` elements before any element is read.
fn fast_array_create(cx: &mut Cx<'_>, length: u64) -> Result<ObjectId, Throw> {
    if length > i32::MAX as u64 {
        return Err(error::range_error(cx, "invalid array length"));
    }
    array_create(cx, length)
}

// ---- the constructor ----

/// `Array(...)`: a length, the arguments as elements, or a single
/// non-number element; the prototype comes from `new.target`.
pub(super) fn constructor(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let prototype = if site.construct && site.new_target != site.function {
        internal::prototype_from_constructor(cx, site.new_target, Intrinsic::ArrayPrototype)?
    } else {
        cx.realm.intrinsic(Intrinsic::ArrayPrototype)
    };
    let array = if site.argc == 1 && site.arg(cx, 0).is_number() {
        let number = site.arg(cx, 0).as_f64().unwrap();
        let length = number::to_uint32(number);
        if f64::from(length) != number {
            return Err(error::range_error(cx, "invalid array length"));
        }
        let array = cx
            .heap
            .alloc_array(Some(prototype), length.min(EAGER_ELEMENTS))
            .map_err(|error| error::heap_error(cx, error))?;
        cx.heap.set_array_length(array, length);
        array
    } else {
        let array = cx
            .heap
            .alloc_array(Some(prototype), site.argc as u32)
            .map_err(|error| error::heap_error(cx, error))?;
        let elements = cx.heap.header(array).elements();
        for index in 0..site.argc {
            let value = site.arg(cx, index);
            cx.heap.values.set(elements, index as u32, value);
        }
        cx.heap.set_array_length(array, site.argc as u32);
        array
    };
    Ok(Value::object(array))
}

pub(super) fn is_array(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let value = site.arg(cx, 0);
    internal::is_array(cx, value).map(Value::bool)
}

/// `Array.from(items, mapFn, thisArg)`.
pub(super) fn from(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let constructor = site.this(cx);
    let (items, map, this_arg) = (site.arg(cx, 0), site.arg(cx, 1), site.arg(cx, 2));
    if !map.is_undefined() && !ops::is_callable(cx.heap, map) {
        return Err(error::type_error(cx, "not a function"));
    }
    let key = WellKnown::key(cx.realm.symbols.iterator);
    let using_iterator = internal::get_method(cx, items, key)?;
    if let Some(method) = using_iterator {
        root(cx, method);
        let target = if internal::is_constructor(cx, constructor) {
            let target = crate::vm::construct(cx, constructor, &[], constructor)?;
            target.as_object().expect("a constructed object")
        } else {
            internal::array_create(cx, 0)?
        };
        root(cx, Value::object(target));
        let mut record = if iter::is_pristine_array(cx, items) {
            iter::get_iterator(cx, items)?
        } else {
            iter::from_method(cx, items, method)?
        };
        let mut index = 0u64;
        loop {
            if index >= MAX_LENGTH {
                let error = error::type_error(cx, "array too long");
                return Err(close_with(cx, &record, error));
            }
            let Some(value) = iter::step(cx, &mut record)? else {
                break;
            };
            root(cx, value);
            let step = (|| {
                let value = if map.is_undefined() {
                    value
                } else {
                    crate::vm::call(cx, map, this_arg, &[value, index_value(index)])?
                };
                create_at(cx, target, index, value)
            })();
            if let Err(throw) = step {
                return Err(close_with(cx, &record, throw));
            }
            index += 1;
        }
        set_length(cx, target, index)?;
        return Ok(Value::object(target));
    }
    let source = internal::to_object(cx, items)?;
    root(cx, Value::object(source));
    let length = length_of(cx, source)?;
    let target = if internal::is_constructor(cx, constructor) {
        let target = crate::vm::construct(cx, constructor, &[index_value(length)], constructor)?;
        target.as_object().expect("a constructed object")
    } else {
        internal::array_create(cx, length)?
    };
    root(cx, Value::object(target));
    for index in 0..length {
        let value = get_at(cx, source, index)?;
        let value = if map.is_undefined() {
            value
        } else {
            root(cx, value);
            crate::vm::call(cx, map, this_arg, &[value, index_value(index)])?
        };
        create_at(cx, target, index, value)?;
    }
    set_length(cx, target, length)?;
    Ok(Value::object(target))
}

/// Closes the iterator for a throw completion and returns the throw.
fn close_with(cx: &mut Cx<'_>, record: &iter::Iter, throw: Throw) -> Throw {
    match iter::close(cx, record, Err(throw)) {
        Err(throw) => throw,
        Ok(()) => throw,
    }
}

/// `Array.of(...items)`.
pub(super) fn of(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let constructor = site.this(cx);
    let length = site.argc as u64;
    let target = if internal::is_constructor(cx, constructor) {
        let target = crate::vm::construct(cx, constructor, &[index_value(length)], constructor)?;
        target.as_object().expect("a constructed object")
    } else {
        internal::array_create(cx, length)?
    };
    root(cx, Value::object(target));
    for index in 0..site.argc {
        let value = site.arg(cx, index);
        create_at(cx, target, index as u64, value)?;
    }
    set_length(cx, target, length)?;
    Ok(Value::object(target))
}

// ---- mutators ----

pub(super) fn push(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let object = this_object(cx, &site)?;
    // A push that would pass the largest length takes the generic path,
    // which stores the elements and then fails setting `length`.
    if plain_writable(cx, object)
        && u64::from(cx.heap.array_length(object)) + site.argc as u64 <= u64::from(u32::MAX - 1)
    {
        let mut length = cx.heap.array_length(object);
        for index in 0..site.argc {
            let value = site.arg(cx, index);
            cx.heap
                .set_element(object, length, value)
                .map_err(|error| error::heap_error(cx, error))?;
            length += 1;
        }
        return Ok(crate::vm::property::length_value(length));
    }
    let length = length_of(cx, object)?;
    if length + site.argc as u64 > MAX_LENGTH {
        return Err(error::type_error(cx, "Array loo long"));
    }
    for index in 0..site.argc {
        let value = site.arg(cx, index);
        set_at(cx, object, length + index as u64, value)?;
    }
    let length = length + site.argc as u64;
    set_length(cx, object, length)?;
    Ok(index_value(length))
}

pub(super) fn pop(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let object = this_object(cx, &site)?;
    if plain_writable(cx, object) {
        let length = cx.heap.array_length(object);
        if length == 0 {
            return Ok(Value::UNDEFINED);
        }
        let value = cx.heap.get_element(object, length - 1);
        let value = if value.is_empty() {
            Value::UNDEFINED
        } else {
            value
        };
        cx.heap.set_array_length(object, length - 1);
        return Ok(value);
    }
    let length = length_of(cx, object)?;
    if length == 0 {
        set_length(cx, object, 0)?;
        return Ok(Value::UNDEFINED);
    }
    let value = get_at(cx, object, length - 1)?;
    root(cx, value);
    delete_at(cx, object, length - 1)?;
    set_length(cx, object, length - 1)?;
    Ok(value)
}

pub(super) fn shift(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let object = this_object(cx, &site)?;
    let length = length_of(cx, object)?;
    if length == 0 {
        set_length(cx, object, 0)?;
        return Ok(Value::UNDEFINED);
    }
    let first = get_at(cx, object, 0)?;
    root(cx, first);
    if plain_writable(cx, object) && length <= u64::from(u32::MAX) {
        // Dense: move the elements down in place.
        let length = length as u32;
        let elements = cx.heap.header(object).elements();
        let count = length.min(elements.len) as usize;
        let slice = cx.heap.values.slice_mut(elements);
        slice.copy_within(1..count, 0);
        if count > 0 {
            slice[count - 1] = Value::EMPTY;
        }
        cx.heap.set_array_length(object, length - 1);
        return Ok(first);
    }
    for index in 1..length {
        if has_at(cx, object, index)? {
            let value = get_at(cx, object, index)?;
            set_at(cx, object, index - 1, value)?;
        } else {
            delete_at(cx, object, index - 1)?;
        }
    }
    delete_at(cx, object, length - 1)?;
    set_length(cx, object, length - 1)?;
    Ok(first)
}

pub(super) fn unshift(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let object = this_object(cx, &site)?;
    let length = length_of(cx, object)?;
    let count = site.argc as u64;
    if count > 0 {
        if length + count > MAX_LENGTH {
            return Err(error::type_error(cx, "Array loo long"));
        }
        let mut index = length;
        while index > 0 {
            let from = index - 1;
            let to = from + count;
            if has_at(cx, object, from)? {
                let value = get_at(cx, object, from)?;
                set_at(cx, object, to, value)?;
            } else {
                delete_at(cx, object, to)?;
            }
            index -= 1;
        }
        for at in 0..site.argc {
            let value = site.arg(cx, at);
            set_at(cx, object, at as u64, value)?;
        }
    }
    set_length(cx, object, length + count)?;
    Ok(index_value(length + count))
}

pub(super) fn reverse(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let object = this_object(cx, &site)?;
    let length = length_of(cx, object)?;
    if plain_writable(cx, object) && length == u64::from(cx.heap.array_length(object)) {
        let elements = cx.heap.header(object).elements();
        let count = (length as u32).min(elements.len) as usize;
        if count as u64 == length {
            cx.heap.values.slice_mut(elements)[..count].reverse();
            return Ok(Value::object(object));
        }
    }
    let middle = length / 2;
    let mut lower = 0;
    while lower != middle {
        let upper = length - lower - 1;
        let lower_exists = has_at(cx, object, lower)?;
        let lower_value = if lower_exists {
            get_at(cx, object, lower)?
        } else {
            Value::UNDEFINED
        };
        root(cx, lower_value);
        let upper_exists = has_at(cx, object, upper)?;
        let upper_value = if upper_exists {
            get_at(cx, object, upper)?
        } else {
            Value::UNDEFINED
        };
        root(cx, upper_value);
        match (lower_exists, upper_exists) {
            (true, true) => {
                set_at(cx, object, lower, upper_value)?;
                set_at(cx, object, upper, lower_value)?;
            }
            (false, true) => {
                set_at(cx, object, lower, upper_value)?;
                delete_at(cx, object, upper)?;
            }
            (true, false) => {
                delete_at(cx, object, lower)?;
                set_at(cx, object, upper, lower_value)?;
            }
            (false, false) => {}
        }
        lower += 1;
    }
    Ok(Value::object(object))
}

/// `splice(start, deleteCount, ...items)`.
pub(super) fn splice(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let object = this_object(cx, &site)?;
    let length = length_of(cx, object)?;
    let start_value = site.arg(cx, 0);
    let start = relative_index(cx, start_value, length, 0)?;
    let insert = site.argc.saturating_sub(2) as u64;
    let delete = if site.argc == 0 {
        0
    } else if site.argc == 1 {
        length - start
    } else {
        let count = site.arg(cx, 1);
        let count = internal::to_integer_or_infinity(cx, count)?;
        count.clamp(0.0, (length - start) as f64) as u64
    };
    if length + insert - delete > MAX_LENGTH {
        return Err(error::type_error(cx, "Array loo long"));
    }
    let removed = array_create(cx, delete)?;
    for offset in 0..delete {
        let from = start + offset;
        if has_at(cx, object, from)? {
            let value = get_at(cx, object, from)?;
            create_at(cx, removed, offset, value)?;
        }
    }
    set_length(cx, removed, delete)?;
    if insert < delete {
        for index in start..length - delete {
            let (from, to) = (index + delete, index + insert);
            if has_at(cx, object, from)? {
                let value = get_at(cx, object, from)?;
                set_at(cx, object, to, value)?;
            } else {
                delete_at(cx, object, to)?;
            }
        }
        let mut index = length;
        while index > length - delete + insert {
            delete_at(cx, object, index - 1)?;
            index -= 1;
        }
    } else if insert > delete {
        let mut index = length - delete;
        while index > start {
            let (from, to) = (index + delete - 1, index + insert - 1);
            if has_at(cx, object, from)? {
                let value = get_at(cx, object, from)?;
                set_at(cx, object, to, value)?;
            } else {
                delete_at(cx, object, to)?;
            }
            index -= 1;
        }
    }
    for offset in 0..insert as usize {
        let value = site.arg(cx, offset + 2);
        set_at(cx, object, start + offset as u64, value)?;
    }
    set_length(cx, object, length - delete + insert)?;
    Ok(Value::object(removed))
}

/// `copyWithin(target, start, end)`.
pub(super) fn copy_within(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let object = this_object(cx, &site)?;
    let length = length_of(cx, object)?;
    let (target, start, end) = (site.arg(cx, 0), site.arg(cx, 1), site.arg(cx, 2));
    let to = relative_index(cx, target, length, 0)?;
    let from = relative_index(cx, start, length, 0)?;
    let end = relative_index(cx, end, length, length)?;
    let count = end.saturating_sub(from).min(length - to);
    let (mut from, mut to, step) = if from < to && to < from + count {
        (from + count - 1, to + count - 1, -1i64)
    } else {
        (from, to, 1i64)
    };
    for _ in 0..count {
        if has_at(cx, object, from)? {
            let value = get_at(cx, object, from)?;
            set_at(cx, object, to, value)?;
        } else {
            delete_at(cx, object, to)?;
        }
        from = (from as i64 + step) as u64;
        to = (to as i64 + step) as u64;
    }
    Ok(Value::object(object))
}

/// `fill(value, start, end)`.
pub(super) fn fill(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let object = this_object(cx, &site)?;
    let length = length_of(cx, object)?;
    let (value, start, end) = (site.arg(cx, 0), site.arg(cx, 1), site.arg(cx, 2));
    let start = relative_index(cx, start, length, 0)?;
    let end = relative_index(cx, end, length, length)?;
    for index in start..end {
        set_at(cx, object, index, value)?;
    }
    Ok(Value::object(object))
}

// ---- readers ----

pub(super) fn at(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let object = this_object(cx, &site)?;
    let length = length_of(cx, object)?;
    let index = site.arg(cx, 0);
    let relative = internal::to_integer_or_infinity(cx, index)?;
    let index = if relative >= 0.0 {
        relative
    } else {
        length as f64 + relative
    };
    if index < 0.0 || index >= length as f64 {
        return Ok(Value::UNDEFINED);
    }
    get_at(cx, object, index as u64)
}

/// `join(separator)`, also behind `toString` with `,`.
pub(super) fn join(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let object = this_object(cx, &site)?;
    let length = length_of(cx, object)?;
    let separator = site.arg(cx, 0);
    let separator: Vec<u16> = if separator.is_undefined() {
        vec![u16::from(b',')]
    } else {
        let separator = super::to_string_id(cx, separator)?;
        cx.heap.string(separator).repr().units().collect()
    };
    join_with(cx, object, length, &separator)
}

/// The joined text of elements `0..length`; `undefined` and `null` join as
/// empty strings.
pub(crate) fn join_with(
    cx: &mut Cx<'_>,
    object: ObjectId,
    length: u64,
    separator: &[u16],
) -> Result<Value, Throw> {
    if !cx.heap.vm.enter_native() {
        return Err(error::stack_overflow(cx));
    }
    let result = (|| {
        let mut joined: Vec<u16> = Vec::new();
        for index in 0..length {
            if index > 0 {
                joined.extend_from_slice(separator);
            }
            let value = get_at(cx, object, index)?;
            if value.is_undefined() || value.is_null() {
                continue;
            }
            if let Some(string) = value.as_string() {
                joined.extend(cx.heap.string(string).repr().units());
                continue;
            }
            // The text is copied out before anything else allocates.
            let text = ops::to_string(cx, value)?;
            joined.extend(cx.heap.string(text.as_string().unwrap()).repr().units());
        }
        string_units(cx, &joined)
    })();
    cx.heap.vm.leave_native();
    result
}

/// `toString`: `this.join()` when callable, else `Object.prototype.toString`.
pub(super) fn to_string(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let object = this_object(cx, &site)?;
    let join = internal::get_property(cx, object, Names::key(cx.realm.names.join))?;
    if ops::is_callable(cx.heap, join) {
        // The intrinsic join on the receiver itself needs no call.
        let intrinsic = join.as_object().is_some_and(|function| {
            cx.heap.object_kind(function) == ObjectKind::Function
                && cx.heap.function_record(function).native == Native::ArrayJoin.index()
        });
        if intrinsic {
            let length = length_of(cx, object)?;
            return join_with(cx, object, length, &[u16::from(b',')]);
        }
        return crate::vm::call(cx, join, Value::object(object), &[]);
    }
    super::object::to_string(cx, site)
}

/// `toLocaleString`: each element's `toLocaleString()`, joined by `,`.
pub(super) fn to_locale_string(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let object = this_object(cx, &site)?;
    let length = length_of(cx, object)?;
    if !cx.heap.vm.enter_native() {
        return Err(error::stack_overflow(cx));
    }
    let result = (|| {
        let mut joined: Vec<u16> = Vec::new();
        let key = cx
            .heap
            .property_key("toLocaleString")
            .map_err(|error| error::heap_error(cx, error))?;
        for index in 0..length {
            if index > 0 {
                joined.push(u16::from(b','));
            }
            let value = get_at(cx, object, index)?;
            if value.is_undefined() || value.is_null() {
                continue;
            }
            root(cx, value);
            let text = internal::invoke(cx, value, key, &[])?;
            let text = super::to_string_id(cx, text)?;
            joined.extend(cx.heap.string(text).repr().units());
        }
        string_units(cx, &joined)
    })();
    cx.heap.vm.leave_native();
    result
}

pub(super) fn slice(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let object = this_object(cx, &site)?;
    let length = length_of(cx, object)?;
    let (start, end) = (site.arg(cx, 0), site.arg(cx, 1));
    let start = relative_index(cx, start, length, 0)?;
    let end = relative_index(cx, end, length, length)?;
    let count = end.saturating_sub(start);
    if plain(cx, object) && end <= u64::from(cx.heap.array_length(object)) {
        let result = array_create(cx, count)?;
        let source = cx.heap.header(object).elements();
        let target = cx.heap.header(result).elements();
        for offset in 0..count as u32 {
            let value = cx.heap.values.get_or_empty(source, start as u32 + offset);
            if !value.is_empty() {
                cx.heap.values.set(target, offset, value);
            }
        }
        return Ok(Value::object(result));
    }
    let result = array_create(cx, count)?;
    for offset in 0..count {
        let from = start + offset;
        if has_at(cx, object, from)? {
            let value = get_at(cx, object, from)?;
            create_at(cx, result, offset, value)?;
        }
    }
    set_length(cx, result, count)?;
    Ok(Value::object(result))
}

/// `IsConcatSpreadable(O)`.
fn is_concat_spreadable(cx: &mut Cx<'_>, value: Value) -> Result<bool, Throw> {
    let Some(object) = value.as_object() else {
        return Ok(false);
    };
    let key = WellKnown::key(cx.realm.symbols.is_concat_spreadable);
    let spreadable = internal::get_property(cx, object, key)?;
    if !spreadable.is_undefined() {
        return Ok(ops::to_boolean(cx.heap, spreadable));
    }
    internal::is_array(cx, value)
}

pub(super) fn concat(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let this = this_object(cx, &site)?;
    let result = array_create(cx, 0)?;
    let mut next = 0u64;
    for index in 0..=site.argc {
        let item = if index == 0 {
            Value::object(this)
        } else {
            site.arg(cx, index - 1)
        };
        if is_concat_spreadable(cx, item)? {
            let source = item.as_object().unwrap();
            let length = length_of(cx, source)?;
            if next + length > MAX_LENGTH {
                return Err(error::type_error(cx, "Array loo long"));
            }
            for at in 0..length {
                if has_at(cx, source, at)? {
                    let value = get_at(cx, source, at)?;
                    create_at(cx, result, next, value)?;
                }
                next += 1;
            }
        } else {
            if next >= MAX_LENGTH {
                return Err(error::type_error(cx, "Array loo long"));
            }
            create_at(cx, result, next, item)?;
            next += 1;
        }
    }
    set_length(cx, result, next)?;
    Ok(Value::object(result))
}

pub(super) fn index_of(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let object = this_object(cx, &site)?;
    let length = length_of(cx, object)?;
    if length == 0 {
        return Ok(Value::int32(-1));
    }
    let search = site.arg(cx, 0);
    let from = site.arg(cx, 1);
    let from = relative_index(cx, from, length, 0)?;
    for index in from..length {
        if has_at(cx, object, index)? {
            let value = get_at(cx, object, index)?;
            if ops::strict_eq(cx.heap, value, search) {
                return Ok(index_value(index));
            }
        }
    }
    Ok(Value::int32(-1))
}

pub(super) fn last_index_of(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let object = this_object(cx, &site)?;
    let length = length_of(cx, object)?;
    if length == 0 {
        return Ok(Value::int32(-1));
    }
    let search = site.arg(cx, 0);
    let from = if site.argc > 1 {
        let from = site.arg(cx, 1);
        internal::to_integer_or_infinity(cx, from)?
    } else {
        length as f64 - 1.0
    };
    let mut index = if from >= 0.0 {
        from.min(length as f64 - 1.0)
    } else {
        length as f64 + from
    };
    while index >= 0.0 {
        let at = index as u64;
        if has_at(cx, object, at)? {
            let value = get_at(cx, object, at)?;
            if ops::strict_eq(cx.heap, value, search) {
                return Ok(index_value(at));
            }
        }
        index -= 1.0;
    }
    Ok(Value::int32(-1))
}

pub(super) fn includes(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let object = this_object(cx, &site)?;
    let length = length_of(cx, object)?;
    if length == 0 {
        return Ok(Value::FALSE);
    }
    let search = site.arg(cx, 0);
    let from = site.arg(cx, 1);
    let from = relative_index(cx, from, length, 0)?;
    for index in from..length {
        let value = get_at(cx, object, index)?;
        if internal::same_value_zero(cx, value, search) {
            return Ok(Value::TRUE);
        }
    }
    Ok(Value::FALSE)
}

// ---- callbacks ----

#[derive(Clone, Copy, PartialEq, Eq)]
enum Walk {
    ForEach,
    Every,
    Some,
    Map,
    Filter,
}

fn each(cx: &mut Cx<'_>, site: CallSite, kind: Walk) -> Result<Value, Throw> {
    let object = this_object(cx, &site)?;
    let length = length_of(cx, object)?;
    let function = callback(cx, &site)?;
    let this_arg = site.arg(cx, 1);
    let result = match kind {
        Walk::Map => Some(array_create(cx, length)?),
        Walk::Filter => Some(array_create(cx, 0)?),
        _ => None,
    };
    let mut kept = 0u64;
    for index in 0..length {
        if !has_at(cx, object, index)? {
            continue;
        }
        let value = get_at(cx, object, index)?;
        let mark = cx.heap.roots().mark();
        root(cx, value);
        let selected = crate::vm::call(
            cx,
            function,
            this_arg,
            &[value, index_value(index), Value::object(object)],
        )?;
        match kind {
            Walk::ForEach => {}
            Walk::Every => {
                if !ops::to_boolean(cx.heap, selected) {
                    return Ok(Value::FALSE);
                }
            }
            Walk::Some => {
                if ops::to_boolean(cx.heap, selected) {
                    return Ok(Value::TRUE);
                }
            }
            Walk::Map => create_at(cx, result.unwrap(), index, selected)?,
            Walk::Filter => {
                if ops::to_boolean(cx.heap, selected) {
                    create_at(cx, result.unwrap(), kept, value)?;
                    kept += 1;
                }
            }
        }
        cx.heap.roots().truncate(mark);
    }
    Ok(match kind {
        Walk::ForEach => Value::UNDEFINED,
        Walk::Every => Value::TRUE,
        Walk::Some => Value::FALSE,
        Walk::Map | Walk::Filter => Value::object(result.unwrap()),
    })
}

pub(super) fn for_each(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    each(cx, site, Walk::ForEach)
}

pub(super) fn every(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    each(cx, site, Walk::Every)
}

pub(super) fn some(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    each(cx, site, Walk::Some)
}

pub(super) fn map(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    each(cx, site, Walk::Map)
}

pub(super) fn filter(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    each(cx, site, Walk::Filter)
}

/// `find`, `findIndex`, `findLast`, `findLastIndex`: every index, holes
/// included.
fn find(cx: &mut Cx<'_>, site: CallSite, last: bool, index_result: bool) -> Result<Value, Throw> {
    let object = this_object(cx, &site)?;
    // 23.1.3.9 step 1: `O = ToObject(this)` is the predicate's third argument.
    let receiver = Value::object(object);
    let length = length_of(cx, object)?;
    let predicate = callback(cx, &site)?;
    let this_arg = site.arg(cx, 1);
    for step in 0..length {
        let index = if last { length - 1 - step } else { step };
        let value = get_at(cx, object, index)?;
        let mark = cx.heap.roots().mark();
        root(cx, value);
        let found = crate::vm::call(
            cx,
            predicate,
            this_arg,
            &[value, index_value(index), receiver],
        )?;
        cx.heap.roots().truncate(mark);
        if ops::to_boolean(cx.heap, found) {
            return Ok(if index_result {
                index_value(index)
            } else {
                value
            });
        }
    }
    Ok(if index_result {
        Value::int32(-1)
    } else {
        Value::UNDEFINED
    })
}

pub(super) fn find_(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    find(cx, site, false, false)
}

pub(super) fn find_index(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    find(cx, site, false, true)
}

pub(super) fn find_last(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    find(cx, site, true, false)
}

pub(super) fn find_last_index(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    find(cx, site, true, true)
}

fn reduce_impl(cx: &mut Cx<'_>, site: CallSite, right: bool) -> Result<Value, Throw> {
    let object = this_object(cx, &site)?;
    let length = length_of(cx, object)?;
    let function = callback(cx, &site)?;
    let order = |step: u64| if right { length - 1 - step } else { step };
    let mut step = 0;
    let mut accumulator = if site.argc >= 2 {
        site.arg(cx, 1)
    } else {
        loop {
            if step >= length {
                return Err(error::type_error(cx, "empty array"));
            }
            let index = order(step);
            step += 1;
            if has_at(cx, object, index)? {
                break get_at(cx, object, index)?;
            }
        }
    };
    while step < length {
        let index = order(step);
        step += 1;
        if !has_at(cx, object, index)? {
            continue;
        }
        let mark = cx.heap.roots().mark();
        root(cx, accumulator);
        let value = get_at(cx, object, index)?;
        root(cx, value);
        accumulator = crate::vm::call(
            cx,
            function,
            Value::UNDEFINED,
            &[
                accumulator,
                value,
                index_value(index),
                Value::object(object),
            ],
        )?;
        cx.heap.roots().truncate(mark);
    }
    Ok(accumulator)
}

pub(super) fn reduce(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    reduce_impl(cx, site, false)
}

pub(super) fn reduce_right(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    reduce_impl(cx, site, true)
}

/// `FlattenIntoArray`.
#[allow(clippy::too_many_arguments)]
fn flatten(
    cx: &mut Cx<'_>,
    target: ObjectId,
    source: ObjectId,
    length: u64,
    start: u64,
    depth: f64,
    mapper: Option<(Value, Value)>,
) -> Result<u64, Throw> {
    if !cx.heap.vm.enter_native() {
        return Err(error::stack_overflow(cx));
    }
    let result = (|| {
        let mut next = start;
        for index in 0..length {
            if !has_at(cx, source, index)? {
                continue;
            }
            let mut element = get_at(cx, source, index)?;
            root(cx, element);
            if let Some((function, this_arg)) = mapper {
                element = crate::vm::call(
                    cx,
                    function,
                    this_arg,
                    &[element, index_value(index), Value::object(source)],
                )?;
                root(cx, element);
            }
            if depth > 0.0 && internal::is_array(cx, element)? {
                let inner = element.as_object().unwrap();
                let inner_length = length_of(cx, inner)?;
                next = flatten(cx, target, inner, inner_length, next, depth - 1.0, None)?;
            } else {
                if next >= MAX_LENGTH {
                    return Err(error::type_error(cx, "Array too long"));
                }
                create_at(cx, target, next, element)?;
                next += 1;
            }
        }
        Ok(next)
    })();
    cx.heap.vm.leave_native();
    result
}

pub(super) fn flat(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let object = this_object(cx, &site)?;
    let length = length_of(cx, object)?;
    let depth = site.arg(cx, 0);
    let depth = if depth.is_undefined() {
        1.0
    } else {
        internal::to_integer_or_infinity(cx, depth)?.max(0.0)
    };
    let target = array_create(cx, 0)?;
    flatten(cx, target, object, length, 0, depth, None)?;
    Ok(Value::object(target))
}

pub(super) fn flat_map(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let object = this_object(cx, &site)?;
    let length = length_of(cx, object)?;
    let function = callback(cx, &site)?;
    let this_arg = site.arg(cx, 1);
    let target = array_create(cx, 0)?;
    flatten(
        cx,
        target,
        object,
        length,
        0,
        1.0,
        Some((function, this_arg)),
    )?;
    Ok(Value::object(target))
}

// ---- sort ----

/// A stable merge sort with any fallible comparison; `values` stay rooted by
/// the caller.
pub(crate) fn merge_sort_by(
    cx: &mut Cx<'_>,
    values: &mut Vec<Value>,
    mut compare: impl FnMut(&mut Cx<'_>, Value, Value) -> Result<Ordering, Throw>,
) -> Result<(), Throw> {
    let length = values.len();
    if length < 2 {
        return Ok(());
    }
    // Insertion-sort short runs, then merge.
    const RUN: usize = 8;
    let mut start = 0;
    while start < length {
        let end = (start + RUN).min(length);
        for index in start + 1..end {
            let mut at = index;
            while at > start && compare(cx, values[at - 1], values[at])? == Ordering::Greater {
                values.swap(at - 1, at);
                at -= 1;
            }
        }
        start = end;
    }
    let mut buffer = values.clone();
    let mut width = RUN;
    while width < length {
        let mut left = 0;
        while left < length {
            let middle = (left + width).min(length);
            let right = (left + 2 * width).min(length);
            let (mut i, mut j, mut k) = (left, middle, left);
            while i < middle && j < right {
                if compare(cx, values[i], values[j])? == Ordering::Greater {
                    buffer[k] = values[j];
                    j += 1;
                } else {
                    buffer[k] = values[i];
                    i += 1;
                }
                k += 1;
            }
            buffer[k..k + middle - i].copy_from_slice(&values[i..middle]);
            k += middle - i;
            buffer[k..k + right - j].copy_from_slice(&values[j..right]);
            left = right;
        }
        std::mem::swap(values, &mut buffer);
        width *= 2;
    }
    Ok(())
}

/// One element of QuickJS's `js_array_sort`: the value, its string once the
/// default comparison has converted it, and its original index.
struct Slot {
    value: Value,
    text: Option<Value>,
    position: u64,
}

/// QuickJS's `js_array_cmp_generic`: the comparator's sign (skipped for
/// raw-identical values) or the order of the lazily converted strings, then
/// the original positions. After a throw every comparison is 0.
fn compare_slots(
    cx: &mut Cx<'_>,
    comparator: Value,
    failure: &mut Option<Throw>,
    slots: &mut [Slot],
    a: usize,
    b: usize,
) -> i32 {
    if failure.is_some() {
        return 0;
    }
    let result = (|| -> Result<i32, Throw> {
        if !comparator.is_undefined() {
            let (x, y) = (slots[a].value, slots[b].value);
            if x == y {
                return Ok(0);
            }
            let result = crate::vm::call(cx, comparator, Value::UNDEFINED, &[x, y])?;
            let number = match result.as_int32() {
                Some(int) => f64::from(int),
                None => ops::to_number(cx, result)?,
            };
            return Ok(i32::from(number > 0.0) - i32::from(number < 0.0));
        }
        for index in [a, b] {
            if slots[index].text.is_none() {
                let text = ops::to_string(cx, slots[index].value)?;
                root(cx, text);
                slots[index].text = Some(text);
            }
        }
        let (x, y) = (slots[a].text.unwrap(), slots[b].text.unwrap());
        let x = cx.heap.string(x.as_string().unwrap()).repr().units();
        let y = cx.heap.string(y.as_string().unwrap()).repr().units();
        Ok(match x.cmp(y) {
            Ordering::Less => -1,
            Ordering::Equal => 0,
            Ordering::Greater => 1,
        })
    })();
    match result {
        Ok(0) => {
            let (x, y) = (slots[a].position, slots[b].position);
            i32::from(x > y) - i32::from(x < y)
        }
        Ok(order) => order,
        Err(throw) => {
            *failure = Some(throw);
            0
        }
    }
}

/// QuickJS's `heapsortx` over `base[start..start + n]`, `rqsort`'s fallback
/// past depth 50.
fn heapsort<T>(
    base: &mut [T],
    start: usize,
    n: usize,
    cmp: &mut impl FnMut(&mut [T], usize, usize) -> i32,
) {
    if n <= 1 {
        return;
    }
    let sift = |base: &mut [T], cmp: &mut dyn FnMut(&mut [T], usize, usize) -> i32, mut r, end| {
        loop {
            let mut c = r * 2 + 1;
            if c >= end {
                break;
            }
            if c < end - 1 && cmp(base, start + c, start + c + 1) <= 0 {
                c += 1;
            }
            if cmp(base, start + r, start + c) > 0 {
                break;
            }
            base.swap(start + r, start + c);
            r = c;
        }
    };
    let mut i = n / 2;
    while i > 0 {
        i -= 1;
        sift(base, cmp, i, n);
    }
    let mut i = n - 1;
    while i > 0 {
        base.swap(start, start + i);
        sift(base, cmp, 0, i);
        i -= 1;
    }
}

/// QuickJS's `rqsort` (cutils.c): a three-way quicksort with a median of
/// three pivot and insertion sort below seven elements, whose comparison
/// sequence the comparator observes.
pub(crate) fn rqsort<T>(base: &mut [T], mut cmp: impl FnMut(&mut [T], usize, usize) -> i32) {
    fn swap_block<T>(base: &mut [T], a: usize, b: usize, span: usize) {
        for offset in 0..span {
            base.swap(a + offset, b + offset);
        }
    }
    if base.len() < 2 {
        return;
    }
    let mut stack: Vec<(usize, usize, u32)> = vec![(0, base.len(), 0)];
    while let Some((mut ptr, mut nmemb, mut depth)) = stack.pop() {
        while nmemb > 6 {
            depth += 1;
            if depth > 50 {
                heapsort(base, ptr, nmemb, &mut cmp);
                nmemb = 0;
                break;
            }
            let m4 = nmemb >> 2;
            let (a, b, c) = (ptr + m4, ptr + 2 * m4, ptr + 3 * m4);
            let m = if cmp(base, a, b) < 0 {
                if cmp(base, b, c) < 0 {
                    b
                } else if cmp(base, a, c) < 0 {
                    c
                } else {
                    a
                }
            } else if cmp(base, b, c) > 0 {
                b
            } else if cmp(base, a, c) < 0 {
                a
            } else {
                c
            };
            base.swap(ptr, m);
            let mut i = 1usize;
            let mut lt = 1usize;
            let mut pi = ptr + 1;
            let mut plt = ptr + 1;
            let mut gt = nmemb;
            let top = ptr + nmemb;
            let mut pj = top;
            let mut pgt = top;
            loop {
                while pi < pj {
                    let c = cmp(base, ptr, pi);
                    if c < 0 {
                        break;
                    }
                    if c == 0 {
                        base.swap(plt, pi);
                        lt += 1;
                        plt += 1;
                    }
                    i += 1;
                    pi += 1;
                }
                loop {
                    pj -= 1;
                    if pi >= pj {
                        break;
                    }
                    let c = cmp(base, ptr, pj);
                    if c > 0 {
                        break;
                    }
                    if c == 0 {
                        gt -= 1;
                        pgt -= 1;
                        base.swap(pgt, pj);
                    }
                }
                if pi >= pj {
                    break;
                }
                base.swap(pi, pj);
                i += 1;
                pi += 1;
            }
            let span = (plt - ptr).min(pi - plt);
            lt = i - lt;
            swap_block(base, ptr, pi - span, span);
            let span2 = pgt - pi;
            let span = (top - pgt).min(span2);
            pgt = top - span2;
            gt = nmemb - (gt - i);
            swap_block(base, pi, top - span, span);
            if lt > nmemb - gt {
                stack.push((ptr, lt, depth));
                ptr = pgt;
                nmemb -= gt;
            } else {
                stack.push((pgt, nmemb - gt, depth));
                nmemb = lt;
            }
        }
        let top = ptr + nmemb;
        let mut pi = ptr + 1;
        while pi < top {
            let mut pj = pi;
            while pj > ptr && cmp(base, pj - 1, pj) > 0 {
                base.swap(pj, pj - 1);
                pj -= 1;
            }
            pi += 1;
        }
    }
}

/// The elements `js_array_sort` sorts: every present non-`undefined` value
/// with its index (read by `HasProperty` then `Get`), rooted, and the count
/// of `undefined` values.
fn sort_slots(
    cx: &mut Cx<'_>,
    object: ObjectId,
    length: u64,
    comparator: Value,
) -> Result<(Vec<Slot>, u64), Throw> {
    let mut slots = Vec::new();
    let mut undefined = 0u64;
    for index in 0..length {
        if !has_at(cx, object, index)? {
            continue;
        }
        let value = get_at(cx, object, index)?;
        if value.is_undefined() {
            undefined += 1;
            continue;
        }
        root(cx, value);
        slots.push(Slot {
            value,
            text: None,
            position: index,
        });
    }
    let mut failure = None;
    rqsort(&mut slots, |slots, a, b| {
        compare_slots(cx, comparator, &mut failure, slots, a, b)
    });
    match failure {
        Some(throw) => Err(throw),
        None => Ok((slots, undefined)),
    }
}

fn comparator(cx: &mut Cx<'_>, site: &CallSite) -> Result<Value, Throw> {
    let comparator = site.arg(cx, 0);
    if !comparator.is_undefined() && !ops::is_callable(cx.heap, comparator) {
        return Err(error::type_error(cx, "not a function"));
    }
    Ok(comparator)
}

pub(super) fn sort(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let comparator = comparator(cx, &site)?;
    let object = this_object(cx, &site)?;
    let length = length_of(cx, object)?;
    let (slots, undefined) = sort_slots(cx, object, length, comparator)?;
    // 23.1.3.30 step 7: `Set` of every sorted index.
    let count = slots.len() as u64;
    for (index, slot) in slots.iter().enumerate() {
        set_at(cx, object, index as u64, slot.value)?;
    }
    for index in count..count + undefined {
        set_at(cx, object, index, Value::UNDEFINED)?;
    }
    // `DeletePropertyOrThrow` of every index past the sorted items, present
    // or not.
    for index in count + undefined..length {
        delete_at(cx, object, index)?;
    }
    Ok(Value::object(object))
}

/// `js_array_toSorted`: a dense copy (holes read as `undefined`), sorted as
/// `js_array_sort` sorts it.
pub(super) fn to_sorted(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let comparator = comparator(cx, &site)?;
    let object = this_object(cx, &site)?;
    let length = length_of(cx, object)?;
    let target = fast_array_create(cx, length)?;
    let mut slots = Vec::new();
    let mut undefined = 0u64;
    for index in 0..length {
        let value = try_get_at(cx, object, index)?;
        if value.is_undefined() {
            undefined += 1;
            continue;
        }
        root(cx, value);
        slots.push(Slot {
            value,
            text: None,
            position: index,
        });
    }
    let mut failure = None;
    rqsort(&mut slots, |slots, a, b| {
        compare_slots(cx, comparator, &mut failure, slots, a, b)
    });
    if let Some(throw) = failure {
        return Err(throw);
    }
    for (index, slot) in slots.iter().enumerate() {
        create_at(cx, target, index as u64, slot.value)?;
    }
    for index in slots.len() as u64..slots.len() as u64 + undefined {
        create_at(cx, target, index, Value::UNDEFINED)?;
    }
    Ok(Value::object(target))
}

pub(super) fn to_reversed(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let object = this_object(cx, &site)?;
    let length = length_of(cx, object)?;
    let target = fast_array_create(cx, length)?;
    for index in 0..length {
        let value = try_get_at(cx, object, length - 1 - index)?;
        create_at(cx, target, index, value)?;
    }
    Ok(Value::object(target))
}

pub(super) fn to_spliced(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let object = this_object(cx, &site)?;
    let length = length_of(cx, object)?;
    let start = site.arg(cx, 0);
    let start = relative_index(cx, start, length, 0)?;
    let insert = site.argc.saturating_sub(2) as u64;
    let skip = if site.argc == 0 {
        0
    } else if site.argc == 1 {
        length - start
    } else {
        let count = site.arg(cx, 1);
        let count = internal::to_integer_or_infinity(cx, count)?;
        count.clamp(0.0, (length - start) as f64) as u64
    };
    let new_length = length + insert - skip;
    if new_length > MAX_LENGTH {
        return Err(error::type_error(cx, "invalid array length"));
    }
    let target = fast_array_create(cx, new_length)?;
    let mut to = 0u64;
    for index in 0..start {
        let value = try_get_at(cx, object, index)?;
        create_at(cx, target, to, value)?;
        to += 1;
    }
    for offset in 0..insert as usize {
        let value = site.arg(cx, offset + 2);
        create_at(cx, target, to, value)?;
        to += 1;
    }
    for index in start + skip..length {
        let value = try_get_at(cx, object, index)?;
        create_at(cx, target, to, value)?;
        to += 1;
    }
    Ok(Value::object(target))
}

/// QuickJS's `invalid array index: {idx}`, `idx` the saturated `int64`
/// index made relative to `length`.
#[cold]
#[inline(never)]
fn invalid_index(cx: &mut Cx<'_>, length: u64, relative: f64) -> Throw {
    // `as` saturates as `JS_ToInt64Sat` does.
    let mut index = relative as i64;
    if index < 0 {
        index = index.saturating_add(length as i64);
    }
    error::range_error(cx, &format!("invalid array index: {index}"))
}

pub(super) fn with(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let object = this_object(cx, &site)?;
    let length = length_of(cx, object)?;
    let index = site.arg(cx, 0);
    let relative = internal::to_integer_or_infinity(cx, index)?;
    let actual = if relative >= 0.0 {
        relative
    } else {
        length as f64 + relative
    };
    if actual >= length as f64 || actual < 0.0 {
        return Err(invalid_index(cx, length, relative));
    }
    let replacement = site.arg(cx, 1);
    let target = fast_array_create(cx, length)?;
    for index in 0..length {
        let value = if index == actual as u64 {
            replacement
        } else {
            try_get_at(cx, object, index)?
        };
        create_at(cx, target, index, value)?;
    }
    Ok(Value::object(target))
}
