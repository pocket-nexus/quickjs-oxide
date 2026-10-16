//! `Array`: the constructor, `isArray`, and the prototype methods the V8 v7
//! programs call: `push`, `pop`, `join`, `slice`, `concat`, `indexOf`,
//! `toString`. Each has a fast path for an array receiver and reads any other
//! receiver through its properties.

use super::{
    Native, install_constructor, methods, relative, string_units, this_object, to_integer,
};
use crate::heap::HeapError;
use crate::heap::atom::PropertyKey;
use crate::heap::object::ObjectKind;
use crate::runtime::{Intrinsic, Names};
use crate::value::{ObjectId, Value};
use crate::vm::native::CallSite;
use crate::vm::{Cx, Throw, error, number, ops, property};

/// `new Array(n)` allocates its elements eagerly up to this length; a
/// longer one gets them as they are written.
const EAGER_ELEMENTS: u32 = 1 << 20;

pub(super) fn install(cx: &mut Cx<'_>) -> Result<(), HeapError> {
    let prototype = cx.realm.intrinsic(Intrinsic::ArrayPrototype);
    let array = install_constructor(cx, "Array", Native::ArrayConstructor, prototype)?;
    methods(cx, array, &[("isArray", Native::ArrayIsArray)])?;
    methods(
        cx,
        prototype,
        &[
            ("push", Native::ArrayPush),
            ("pop", Native::ArrayPop),
            ("join", Native::ArrayJoin),
            ("slice", Native::ArraySlice),
            ("concat", Native::ArrayConcat),
            ("indexOf", Native::ArrayIndexOf),
            ("toString", Native::ArrayToString),
        ],
    )
}

fn new_array(cx: &mut Cx<'_>, capacity: u32) -> Result<ObjectId, Throw> {
    let prototype = cx.realm.intrinsic(Intrinsic::ArrayPrototype);
    cx.heap
        .alloc_array(Some(prototype), capacity)
        .map_err(|error| error::heap_error(cx, error))
}

/// `Array(n)` (a length, holes), `Array(a, b, …)` and `Array(x)` for a
/// non-number `x`; `new` is the same.
pub(super) fn constructor(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    if site.argc == 1 {
        let length = site.arg(cx, 0);
        if length.is_number() {
            let number = length.as_f64().unwrap();
            let length = number::to_uint32(number);
            if f64::from(length) != number {
                return Err(error::range_error(cx, "invalid array length"));
            }
            let array = new_array(cx, length.min(EAGER_ELEMENTS))?;
            cx.heap.set_array_length(array, length);
            return Ok(Value::object(array));
        }
    }
    let array = new_array(cx, site.argc as u32)?;
    let elements = cx.heap.header(array).elements();
    for index in 0..site.argc {
        let value = site.arg(cx, index);
        cx.heap.values.set(elements, index as u32, value);
    }
    cx.heap.set_array_length(array, site.argc as u32);
    Ok(Value::object(array))
}

pub(super) fn is_array(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let value = site.arg(cx, 0);
    let array = value
        .as_object()
        .is_some_and(|object| cx.heap.object_kind(object) == ObjectKind::Array);
    Ok(Value::bool(array))
}

/// `ToLength(receiver.length)`.
fn length_of(cx: &mut Cx<'_>, object: ObjectId) -> Result<u32, Throw> {
    if cx.heap.object_kind(object) == ObjectKind::Array {
        return Ok(cx.heap.array_length(object));
    }
    let names = cx.realm.names;
    let length = property::get_value(cx, object, Names::key(names.length))?;
    let length = to_integer(cx, length)?;
    Ok(length.clamp(0.0, f64::from(u32::MAX - 1)) as u32)
}

/// Element `index` of an array-like: a dense element, or a property read
/// (holes reach the prototypes).
fn element(cx: &mut Cx<'_>, object: ObjectId, index: u32) -> Result<Value, Throw> {
    let value = cx.heap.get_element(object, index);
    if !value.is_empty() {
        return Ok(value);
    }
    match PropertyKey::index(index) {
        Some(key) => property::get_value(cx, object, key),
        None => Ok(Value::UNDEFINED),
    }
}

/// Whether `object` has `index` (own or inherited).
fn has_element(cx: &Cx<'_>, object: ObjectId, index: u32) -> bool {
    if !cx.heap.get_element(object, index).is_empty() {
        return true;
    }
    PropertyKey::index(index)
        .is_some_and(|key| property::has(cx.heap, &cx.realm.names, object, key))
}

fn set_length(cx: &mut Cx<'_>, object: ObjectId, length: u32) -> Result<(), Throw> {
    if cx.heap.object_kind(object) == ObjectKind::Array {
        cx.heap.set_array_length(object, length);
        return Ok(());
    }
    let names = cx.realm.names;
    let value = property::length_value(length);
    property::set_on(cx, object, Names::key(names.length), value, true)
}

pub(super) fn push(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let object = this_object(cx, &site)?;
    let mut length = length_of(cx, object)?;
    for index in 0..site.argc {
        let value = site.arg(cx, index);
        if cx.heap.object_kind(object) == ObjectKind::Array {
            cx.heap
                .set_element(object, length, value)
                .map_err(|_| error::oom(cx.realm))?;
        } else {
            let key = PropertyKey::index(length).expect("an index below 2^31");
            property::set_on(cx, object, key, value, true)?;
        }
        length += 1;
    }
    if cx.heap.object_kind(object) != ObjectKind::Array {
        set_length(cx, object, length)?;
    }
    Ok(property::length_value(length))
}

pub(super) fn pop(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let object = this_object(cx, &site)?;
    let length = length_of(cx, object)?;
    if length == 0 {
        set_length(cx, object, 0)?;
        return Ok(Value::UNDEFINED);
    }
    let value = element(cx, object, length - 1)?;
    // The value leaves the array: keep it rooted while the length shrinks.
    let mark = cx.heap.roots().mark();
    cx.heap.roots().push(value);
    let result = (|| {
        if cx.heap.object_kind(object) != ObjectKind::Array
            && let Some(key) = PropertyKey::index(length - 1)
        {
            property::delete(cx, Value::object(object), key, true)?;
        }
        set_length(cx, object, length - 1)
    })();
    cx.heap.roots().truncate(mark);
    result.map(|()| value)
}

/// `join(separator)`, also behind `toString` with `,`.
pub(super) fn join(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let object = this_object(cx, &site)?;
    let separator = site.arg(cx, 0);
    let separator: Vec<u16> = if separator.is_undefined() {
        vec![u16::from(b',')]
    } else {
        let separator = super::to_string_id(cx, separator)?;
        cx.heap.string(separator).repr.units().collect()
    };
    join_with(cx, object, &separator)
}

pub(crate) fn join_with(
    cx: &mut Cx<'_>,
    object: ObjectId,
    separator: &[u16],
) -> Result<Value, Throw> {
    if !cx.heap.vm.enter_native() {
        return Err(error::stack_overflow(cx));
    }
    let result = (|| {
        let length = length_of(cx, object)?;
        let mut joined: Vec<u16> = Vec::new();
        for index in 0..length {
            if index > 0 {
                joined.extend_from_slice(separator);
            }
            let value = element(cx, object, index)?;
            if value.is_undefined() || value.is_null() {
                continue;
            }
            // The text is copied out before anything else allocates.
            let text = super::to_string_id(cx, value)?;
            joined.extend(cx.heap.string(text).repr.units());
        }
        string_units(cx, &joined)
    })();
    cx.heap.vm.leave_native();
    result
}

pub(super) fn to_string(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let object = this_object(cx, &site)?;
    join_with(cx, object, &[u16::from(b',')])
}

pub(super) fn slice(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let object = this_object(cx, &site)?;
    let length = length_of(cx, object)? as usize;
    let start = site.arg(cx, 0);
    let start = relative(to_integer(cx, start)?, length);
    let end = site.arg(cx, 1);
    let end = if end.is_undefined() {
        length
    } else {
        relative(to_integer(cx, end)?, length)
    };
    let count = end.saturating_sub(start) as u32;
    let result = new_array(cx, count)?;
    let mark = cx.heap.roots().mark();
    cx.heap.roots().push(Value::object(result));
    let copied = (|| {
        for offset in 0..count {
            let index = start as u32 + offset;
            if has_element(cx, object, index) {
                let value = element(cx, object, index)?;
                cx.heap
                    .set_element(result, offset, value)
                    .map_err(|_| error::oom(cx.realm))?;
            }
        }
        Ok(())
    })();
    cx.heap.roots().truncate(mark);
    copied?;
    cx.heap.set_array_length(result, count);
    Ok(Value::object(result))
}

pub(super) fn concat(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let this = this_object(cx, &site)?;
    let result = new_array(cx, 0)?;
    let mark = cx.heap.roots().mark();
    cx.heap.roots().push(Value::object(result));
    let appended = (|| {
        let mut next = 0u32;
        for index in 0..=site.argc {
            let item = if index == 0 {
                Value::object(this)
            } else {
                site.arg(cx, index - 1)
            };
            match item.as_object() {
                Some(array) if cx.heap.object_kind(array) == ObjectKind::Array => {
                    let length = cx.heap.array_length(array);
                    for at in 0..length {
                        if has_element(cx, array, at) {
                            let value = element(cx, array, at)?;
                            cx.heap
                                .set_element(result, next, value)
                                .map_err(|_| error::oom(cx.realm))?;
                        }
                        next += 1;
                    }
                }
                _ => {
                    cx.heap
                        .set_element(result, next, item)
                        .map_err(|_| error::oom(cx.realm))?;
                    next += 1;
                }
            }
        }
        cx.heap.set_array_length(result, next);
        Ok(())
    })();
    cx.heap.roots().truncate(mark);
    appended.map(|()| Value::object(result))
}

pub(super) fn index_of(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let object = this_object(cx, &site)?;
    let length = length_of(cx, object)?;
    let search = site.arg(cx, 0);
    let from = site.arg(cx, 1);
    let from = relative(to_integer(cx, from)?, length as usize) as u32;
    for index in from..length {
        if has_element(cx, object, index) {
            let value = element(cx, object, index)?;
            if ops::strict_eq(cx.heap, value, search) {
                return Ok(Value::int32(index as i32));
            }
        }
    }
    Ok(Value::int32(-1))
}
