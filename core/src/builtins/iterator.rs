//! `%IteratorPrototype%` and the array iterator (section 6.1 rows 2 and 8):
//! `Array.prototype.values`, `keys`, `entries` and `[Symbol.iterator]`, and
//! `%ArrayIteratorPrototype%.next`. An array iterator's internal slots are
//! the iterated object (`undefined` once done), the next index and the kind.

use super::{Native, methods, plain_object, symbol_method, to_string_tag};
use crate::heap::HeapError;
use crate::heap::object::{InternalTag, ObjectKind};
use crate::runtime::Intrinsic;
use crate::shape::PropertyFlags;
use crate::value::{ObjectId, Value};
use crate::vm::native::CallSite;
use crate::vm::object as internal;
use crate::vm::{Cx, Throw, error, iter};

/// What an array iterator yields.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub(crate) enum ArrayIterKind {
    Keys,
    Values,
    Entries,
}

pub(super) fn install(cx: &mut Cx<'_>) -> Result<(), HeapError> {
    let object_prototype = cx.realm.intrinsic(Intrinsic::ObjectPrototype);
    let iterator_prototype = plain_object(cx, Some(object_prototype), 8)?;
    cx.set_intrinsic(Intrinsic::IteratorPrototype, iterator_prototype);
    let iterator = cx.realm.symbols.iterator;
    symbol_method(
        cx,
        iterator_prototype,
        iterator,
        Native::IteratorPrototypeIterator,
        PropertyFlags::data(true, false, true),
    )?;
    let array_iterator_prototype = plain_object(cx, Some(iterator_prototype), 4)?;
    cx.set_intrinsic(Intrinsic::ArrayIteratorPrototype, array_iterator_prototype);
    methods(cx, array_iterator_prototype, &[Native::ArrayIteratorNext])?;
    to_string_tag(cx, array_iterator_prototype, "Array Iterator")?;
    let string_iterator_prototype = plain_object(cx, Some(iterator_prototype), 4)?;
    cx.set_intrinsic(
        Intrinsic::StringIteratorPrototype,
        string_iterator_prototype,
    );
    methods(cx, string_iterator_prototype, &[Native::StringIteratorNext])?;
    to_string_tag(cx, string_iterator_prototype, "String Iterator")?;
    Ok(())
}

/// `%IteratorPrototype%[Symbol.iterator]`: `this`.
pub(super) fn iterator_prototype_iterator(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    Ok(site.this(cx))
}

/// `CreateArrayIterator(array, kind)`.
pub(crate) fn create_array_iterator(
    cx: &mut Cx<'_>,
    object: Value,
    kind: ArrayIterKind,
) -> Result<Value, Throw> {
    let prototype = cx.realm.intrinsic(Intrinsic::ArrayIteratorPrototype);
    cx.heap
        .alloc_internal(
            ObjectKind::Builtin,
            InternalTag::ArrayIterator,
            Some(prototype),
            0,
            &[object, Value::int32(0), Value::int32(kind as i32)],
            Default::default(),
            0,
        )
        .map(Value::object)
        .map_err(|error| error::heap_error(cx, error))
}

fn array_iterator(cx: &mut Cx<'_>, site: CallSite, kind: ArrayIterKind) -> Result<Value, Throw> {
    let object = super::this_object(cx, &site)?;
    create_array_iterator(cx, Value::object(object), kind)
}

pub(super) fn values(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    array_iterator(cx, site, ArrayIterKind::Values)
}

pub(super) fn keys(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    array_iterator(cx, site, ArrayIterKind::Keys)
}

pub(super) fn entries(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    array_iterator(cx, site, ArrayIterKind::Entries)
}

fn this_iterator(cx: &mut Cx<'_>, site: &CallSite, tag: InternalTag) -> Result<ObjectId, Throw> {
    let this = site.this(cx);
    match this.as_object() {
        Some(object) if cx.heap.internal_tag(object) == tag => Ok(object),
        _ => Err(error::object_expected(
            cx,
            if tag == InternalTag::ArrayIterator {
                "Array Iterator"
            } else {
                "String Iterator"
            },
        )),
    }
}

/// `%ArrayIteratorPrototype%.next`.
pub(super) fn array_iterator_next(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let iterator = this_iterator(cx, &site, InternalTag::ArrayIterator)?;
    let target = cx.heap.internal_slot(iterator, 0);
    let Some(object) = target.as_object() else {
        return iter::result_object(cx, Value::UNDEFINED, true);
    };
    let index = cx.heap.internal_slot(iterator, 1).as_f64().unwrap_or(0.0);
    let kind = cx.heap.internal_slot(iterator, 2).as_int32().unwrap_or(1);
    let length = typed_array_length(cx, object)?;
    let length = match length {
        Some(length) => length as f64,
        None => internal::length_of_array_like(cx, object)? as f64,
    };
    if index >= length {
        cx.heap.set_internal_slot(iterator, 0, Value::UNDEFINED);
        return iter::result_object(cx, Value::UNDEFINED, true);
    }
    cx.heap
        .set_internal_slot(iterator, 1, crate::vm::number::number(index + 1.0));
    let index_value = crate::vm::number::number(index);
    if kind == ArrayIterKind::Keys as i32 {
        return iter::result_object(cx, index_value, false);
    }
    let key = internal::number_key(cx, index)?;
    let value = internal::get_property(cx, object, key)?;
    if kind == ArrayIterKind::Values as i32 {
        return iter::result_object(cx, value, false);
    }
    cx.heap.roots().push(value);
    let entry = internal::array_from_list(cx, &[index_value, value])?;
    iter::result_object(cx, Value::object(entry), false)
}

/// The length of a typed array (a `TypeError` once out of bounds), or
/// `None` for anything else.
pub(crate) fn typed_array_length(cx: &mut Cx<'_>, object: ObjectId) -> Result<Option<u64>, Throw> {
    if cx.heap.internal_tag(object) != InternalTag::TypedArray {
        return Ok(None);
    }
    crate::builtins::typed_array_length_or_throw(cx, object).map(Some)
}

/// `String.prototype[Symbol.iterator]`: an iterator over the code points of
/// `ToString(RequireObjectCoercible(this))`. Its internal slots are the
/// string (`undefined` once done) and the next index.
pub(super) fn string_iterator(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let string = super::this_string(cx, &site)?;
    let prototype = cx.realm.intrinsic(Intrinsic::StringIteratorPrototype);
    cx.heap
        .alloc_internal(
            ObjectKind::Builtin,
            InternalTag::StringIterator,
            Some(prototype),
            0,
            &[Value::string(string), Value::int32(0)],
            Default::default(),
            0,
        )
        .map(Value::object)
        .map_err(|error| error::heap_error(cx, error))
}

/// `%StringIteratorPrototype%.next`.
pub(super) fn string_iterator_next(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let iterator = this_iterator(cx, &site, InternalTag::StringIterator)?;
    let Some(string) = cx.heap.internal_slot(iterator, 0).as_string() else {
        return iter::result_object(cx, Value::UNDEFINED, true);
    };
    let index = cx.heap.internal_slot(iterator, 1).as_int32().unwrap_or(0) as usize;
    let repr = &cx.heap.string(string).repr();
    if index >= repr.len() {
        cx.heap.set_internal_slot(iterator, 0, Value::UNDEFINED);
        return iter::result_object(cx, Value::UNDEFINED, true);
    }
    let first = repr.unit(index);
    let width = if (0xD800..0xDC00).contains(&first)
        && index + 1 < repr.len()
        && (0xDC00..0xE000).contains(&repr.unit(index + 1))
    {
        2
    } else {
        1
    };
    let units: Vec<u16> = (index..index + width).map(|at| repr.unit(at)).collect();
    cx.heap
        .set_internal_slot(iterator, 1, Value::int32((index + width) as i32));
    let value = super::string_units(cx, &units)?;
    iter::result_object(cx, value, false)
}
