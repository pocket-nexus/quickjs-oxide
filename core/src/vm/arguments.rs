//! A frame's actual arguments (phase 3, section 5.1) and the argument lists
//! of `apply`, `call` and spread calls (section 5.2).
//!
//! A function that reads its actual arguments (the `ARGUMENTS` unit flag)
//! finds the first `parameter_count` of them in its parameter registers and
//! the rest above its registers, where entering the frame kept them. A
//! mapped `arguments` object aliases the parameters through their cells: the
//! lowering gives every parameter of such a function a cell.

use super::error;
use super::property;
use super::{Cx, Frame, Throw};
use crate::bytecode::CodeUnit;
use crate::heap::object::{NONE, ObjectKind};
use crate::runtime::{Intrinsic, Names};
use crate::shape::PropertyFlags;
use crate::value::{ObjectId, ScopeId, Value};

/// The most arguments a list expands to, QuickJS's `JS_MAX_LOCAL_VARS`.
const MAX_ARGUMENTS: usize = 65_534;

/// Actual argument `index` of a frame: a parameter register (read through
/// its cell when the parameter is captured) or a kept extra argument.
fn actual(cx: &Cx<'_>, frame: &Frame, unit: &CodeUnit, index: usize) -> Value {
    let base = frame.base as usize;
    let parameters = unit.parameter_count as usize;
    let value = if index < parameters {
        cx.heap.vm.registers[base + 1 + index]
    } else {
        cx.heap.vm.registers[base + unit.register_count as usize + index - parameters]
    };
    unwrap_cell(cx, value)
}

/// Actual argument `index` of a frame, for natives and slow paths.
pub(crate) fn actual_argument(cx: &Cx<'_>, frame: &Frame, unit: &CodeUnit, index: usize) -> Value {
    actual(cx, frame, unit, index)
}

/// The value in a cell, or `value` itself: a register of a captured binding
/// holds its cell, and cells (scope objects) are never JavaScript values.
fn unwrap_cell(cx: &Cx<'_>, value: Value) -> Value {
    match value.as_object() {
        Some(object) if cx.heap.object_kind(object) == ObjectKind::Scope => {
            cx.heap.scope_get(ScopeId(object.index()), 0, 0)
        }
        _ => value,
    }
}

/// `NewArguments`: the running frame's `arguments` object. A mapped one
/// keeps the cells of the parameters that received an argument in a scope
/// object (slot 0 the mapped count), and its elements hold `Value::EMPTY` at
/// those indices, so the dense fast paths miss there.
#[inline(never)]
pub(crate) fn new_arguments(
    cx: &mut Cx<'_>,
    frame: &Frame,
    unit: &CodeUnit,
    mapped: bool,
) -> Result<Value, Throw> {
    let mark = cx.heap.roots().mark();
    let result = new_arguments_rooted(cx, frame, unit, mapped);
    cx.heap.roots().truncate(mark);
    result.map_err(|error| error::heap_error(cx, error))
}

fn new_arguments_rooted(
    cx: &mut Cx<'_>,
    frame: &Frame,
    unit: &CodeUnit,
    mapped: bool,
) -> Result<Value, crate::HeapError> {
    let argc = frame.argc as usize;
    let mapped_count = if mapped {
        argc.min(unit.parameter_count as usize)
    } else {
        0
    };
    let prototype = cx.realm.intrinsic(Intrinsic::ObjectPrototype);
    let object = cx.heap.alloc_object_with_elements(
        ObjectKind::Arguments,
        Some(prototype),
        4,
        argc as u32,
    )?;
    cx.heap.set_payload(object, NONE);
    cx.heap.roots().push(Value::object(object));
    let elements = cx.heap.header(object).elements();
    for index in mapped_count..argc {
        let value = actual(cx, frame, unit, index);
        cx.heap.values.set(elements, index as u32, value);
    }
    // An unmapped object (strict code or non-simple parameters) has the
    // throwing `callee`.
    let strict = !mapped;
    let length = Value::int32(argc as i32);
    let values = Value::object(cx.realm.intrinsic(Intrinsic::ArrayValues));
    let cache = usize::from(strict);
    if let Some(shape) = cx.heap.arguments_shapes[cache] {
        let thrower = Value::object(cx.realm.intrinsic(Intrinsic::ThrowTypeError));
        let slots = if strict {
            &[length, values, thrower, thrower][..]
        } else {
            &[length, values, Value::object(frame.function)][..]
        };
        cx.heap.apply_closure_shape(object, shape, slots);
    } else {
        add_properties(cx, object, frame, strict)?;
        if !cx.heap.is_dictionary(object) {
            cx.heap.arguments_shapes[cache] = Some(cx.heap.shape_of(object));
        }
    }
    if mapped_count > 0 {
        let scope = cx.heap.alloc_scope(None, 1 + mapped_count as u32)?;
        cx.heap
            .scope_set(scope, 0, 0, Value::int32(mapped_count as i32));
        for index in 0..mapped_count {
            let cell = cx.heap.vm.registers[frame.base as usize + 1 + index];
            debug_assert!(cell.as_object().is_some(), "a mapped parameter has a cell");
            cx.heap.scope_set(scope, 0, 1 + index as u32, cell);
        }
        cx.heap.set_payload(object, scope.object().index());
    }
    Ok(Value::object(object))
}

/// Adds the own properties of a new `arguments` object one by one (the
/// first of each kind, whose shape is then cached).
fn add_properties(
    cx: &mut Cx<'_>,
    object: ObjectId,
    frame: &Frame,
    strict: bool,
) -> Result<(), crate::HeapError> {
    let names = cx.realm.names;
    let hidden = PropertyFlags::data(true, false, true);
    cx.heap.add_property(
        object,
        Names::key(names.length),
        hidden,
        Value::int32(frame.argc as i32),
    )?;
    let values = cx.realm.intrinsic(Intrinsic::ArrayValues);
    cx.heap.add_property(
        object,
        crate::runtime::WellKnown::key(cx.realm.symbols.iterator),
        hidden,
        Value::object(values),
    )?;
    if strict {
        let thrower = Value::object(cx.realm.intrinsic(Intrinsic::ThrowTypeError));
        cx.heap.add_accessor(
            object,
            Names::key(names.callee),
            PropertyFlags::accessor(false, false),
            thrower,
            thrower,
        )
    } else {
        cx.heap.add_property(
            object,
            Names::key(names.callee),
            hidden,
            Value::object(frame.function),
        )
    }
}

/// The cell aliasing index `index` of a mapped `arguments` object, if the
/// index is still mapped.
#[inline]
pub(crate) fn mapped_cell(cx_heap: &crate::Heap, object: ObjectId, index: u32) -> Option<ScopeId> {
    let header = cx_heap.header(object);
    if header.kind != ObjectKind::Arguments || header.payload == NONE {
        return None;
    }
    let scope = ScopeId(header.payload);
    let mapped = cx_heap.scope_get(scope, 0, 0).as_int32()? as u32;
    if index >= mapped {
        return None;
    }
    let cell = cx_heap.scope_get(scope, 0, 1 + index);
    cell.as_object().map(|cell| ScopeId(cell.index()))
}

/// How many leading indices of an `arguments` object were mapped at its
/// creation (0 for an unmapped one).
pub(crate) fn mapped_count(cx_heap: &crate::Heap, object: ObjectId) -> u32 {
    let header = cx_heap.header(object);
    if header.kind != ObjectKind::Arguments || header.payload == NONE {
        return 0;
    }
    cx_heap
        .scope_get(ScopeId(header.payload), 0, 0)
        .as_int32()
        .map_or(0, |count| count as u32)
}

/// Ends the aliasing of a mapped index (`delete arguments[i]`): the cell
/// stays with the parameter, the `arguments` object forgets it.
pub(crate) fn unmap(cx_heap: &mut crate::Heap, object: ObjectId, index: u32) {
    let scope = ScopeId(cx_heap.header(object).payload);
    cx_heap.scope_set(scope, 0, 1 + index, Value::EMPTY);
}

/// `Rest a, start`: an array of the actual arguments from `start`.
#[inline(never)]
pub(crate) fn rest(
    cx: &mut Cx<'_>,
    frame: &Frame,
    unit: &CodeUnit,
    start: usize,
) -> Result<Value, Throw> {
    let count = (frame.argc as usize).saturating_sub(start);
    let prototype = cx.realm.intrinsic(Intrinsic::ArrayPrototype);
    let array = cx
        .heap
        .alloc_array(Some(prototype), count as u32)
        .map_err(|error| error::heap_error(cx, error))?;
    let elements = cx.heap.header(array).elements();
    for index in 0..count {
        let value = actual(cx, frame, unit, start + index);
        cx.heap.values.set(elements, index as u32, value);
    }
    cx.heap.set_array_length(array, count as u32);
    Ok(Value::object(array))
}

// ---- argument lists ----

/// `f.call(thisArg, …)` with `thisArg` in register `at`: the arguments after
/// it are already in place. Returns their count.
#[inline(never)]
pub(crate) fn call_window(cx: &mut Cx<'_>, at: usize, argc: usize) -> Result<usize, Throw> {
    if argc == 0 {
        cx.heap.vm.registers[at] = Value::UNDEFINED;
        return Ok(0);
    }
    Ok(argc - 1)
}

/// `f.apply(thisArg, list)` with `thisArg` in register `at` and the list
/// after it: the list's elements replace it from `at + 1`. Returns their
/// count.
#[inline(never)]
pub(crate) fn apply_window(cx: &mut Cx<'_>, at: usize, argc: usize) -> Result<usize, Throw> {
    if argc == 0 {
        cx.heap.vm.registers[at] = Value::UNDEFINED;
    }
    let list = if argc >= 2 {
        cx.heap.vm.registers[at + 1]
    } else {
        Value::UNDEFINED
    };
    expand(cx, at + 1, list)
}

/// A spread call with its receiver in register `at` and the array of its
/// arguments after it: the elements replace the array.
#[inline(never)]
pub(crate) fn spread(cx: &mut Cx<'_>, at: usize) -> Result<usize, Throw> {
    let list = cx.heap.vm.registers[at + 1];
    expand(cx, at + 1, list)
}

/// Writes the elements of an array-like `list` to the registers from
/// `first` (`CreateListFromArrayLike`). `undefined` and `null` give none. A
/// dense array or `arguments` object is copied directly; anything else is
/// read through `length` and its indices, which may run getters, so the
/// registers are made roots first.
fn expand(cx: &mut Cx<'_>, first: usize, list: Value) -> Result<usize, Throw> {
    if list.is_undefined() || list.is_null() {
        return Ok(0);
    }
    let Some(object) = list.as_object() else {
        return Err(error::type_error(cx, "not a object"));
    };
    let mark = cx.heap.roots().mark();
    cx.heap.roots().push(list);
    let result = expand_rooted(cx, first, object);
    cx.heap.roots().truncate(mark);
    result
}

fn expand_rooted(cx: &mut Cx<'_>, first: usize, object: ObjectId) -> Result<usize, Throw> {
    let length = array_like_length(cx, object)?;
    if length > MAX_ARGUMENTS {
        return Err(error::range_error(
            cx,
            "too many arguments in function call (only 65534 allowed)",
        ));
    }
    let end = first + length;
    if !cx.heap.vm.reserve(end) {
        return Err(error::stack_overflow(cx));
    }
    // The registers up to `end` become roots: clear what is above `top`.
    let top = cx.heap.vm.top as usize;
    if end > top {
        cx.heap.vm.registers[top.max(first)..end].fill(Value::UNDEFINED);
        cx.heap.vm.top = end as u32;
    }
    for index in 0..length {
        let value = element(cx, object, index as u32)?;
        cx.heap.vm.registers[first + index] = value;
    }
    Ok(length)
}

/// An array's length, or an array-like's `ToLength(length)`.
fn array_like_length(cx: &mut Cx<'_>, object: ObjectId) -> Result<usize, Throw> {
    if cx.heap.object_kind(object) == ObjectKind::Array {
        return Ok(cx.heap.array_length(object) as usize);
    }
    let names = cx.realm.names;
    let length = property::get_value(cx, object, Names::key(names.length))?;
    if let Some(length) = length.as_int32() {
        return Ok(length.max(0) as usize);
    }
    let number = super::ops::to_number(cx, length)?;
    Ok(if number.is_nan() || number <= 0.0 {
        0
    } else {
        number.min((MAX_ARGUMENTS + 1) as f64) as usize
    })
}

/// Element `index` of an array-like: a dense element, a mapped argument, or
/// a full property read (holes reach the prototypes).
#[inline]
fn element(cx: &mut Cx<'_>, object: ObjectId, index: u32) -> Result<Value, Throw> {
    let value = cx.heap.get_element(object, index);
    if !value.is_empty() {
        return Ok(value);
    }
    if let Some(cell) = mapped_cell(cx.heap, object, index) {
        return Ok(cx.heap.scope_get(cell, 0, 0));
    }
    let key = crate::PropertyKey::index(index).expect("an index below 2^31");
    property::get_value(cx, object, key)
}

/// `CreateListFromArrayLike` onto the root stack, for the `apply` native
/// outside the loop. Returns the root-stack index of the first element.
pub(crate) fn list_from_array_like(cx: &mut Cx<'_>, list: Value) -> Result<usize, Throw> {
    let start = cx.heap.roots().len();
    if list.is_undefined() || list.is_null() {
        return Ok(start);
    }
    let Some(object) = list.as_object() else {
        return Err(error::type_error(cx, "not a object"));
    };
    cx.heap.roots().push(list);
    let start = cx.heap.roots().len();
    let length = array_like_length(cx, object)?;
    if length > MAX_ARGUMENTS {
        return Err(error::range_error(
            cx,
            "too many arguments in function call (only 65534 allowed)",
        ));
    }
    for index in 0..length {
        let value = element(cx, object, index as u32)?;
        cx.heap.roots().push(value);
    }
    Ok(start)
}

/// `AppendSpread`: appends the values `source` iterates to `array` from
/// `index`; returns the next index. A pristine array is walked directly.
/// Anything else is GetIterator (one `@@iterator` read) and
/// IteratorStepValue until done (13.2.4.1, 13.3.8.1); a throw while
/// stepping does not close the iterator. A fast array whose method is
/// `%Array.prototype.values%` and whose iterator's `next` is the intrinsic
/// one is copied as it stands, which no code can observe.
#[inline(never)]
pub(crate) fn append_spread(
    cx: &mut Cx<'_>,
    array: Value,
    index: Value,
    source: Value,
) -> Result<Value, Throw> {
    let array = array.as_object().expect("a spread target array");
    let mut next = index.as_int32().expect("a spread index") as u32;
    let mark = cx.heap.roots().mark();
    cx.heap.roots().push(source);
    let result = (|| {
        let mut push = |cx: &mut Cx<'_>, value: Value| {
            let result = cx
                .heap
                .set_element(array, next, value)
                .map_err(|_| error::oom(cx.realm));
            next += 1;
            result
        };
        if super::iter::is_pristine_array(cx, source) {
            let mut iter = super::iter::get_iterator(cx, source)?;
            while let Some(value) = super::iter::step(cx, &mut iter)? {
                push(cx, value)?;
            }
            return Ok(());
        }
        let key = crate::runtime::WellKnown::key(cx.realm.symbols.iterator);
        let Some(method) = super::object::get_method(cx, source, key)? else {
            return Err(error::type_error(cx, "value is not iterable"));
        };
        cx.heap.roots().push(method);
        let array_values = method == Value::object(cx.realm.intrinsic(Intrinsic::ArrayValues));
        let mut iter = super::iter::from_method(cx, source, method)?;
        if array_values
            && let super::iter::Iter::Protocol { next: step, .. } = iter
            && step.as_object().is_some_and(|step| {
                cx.heap.object_kind(step) == ObjectKind::Function
                    && cx.heap.function_record(step).native
                        == crate::builtins::Native::ArrayIteratorNext.index()
            })
            && let Some(length) = fast_array_length(cx, source)
        {
            let source = source.as_object().unwrap();
            for index in 0..length {
                let value = cx.heap.get_element(source, index);
                push(cx, value)?;
            }
            return Ok(());
        }
        while let Some(value) = super::iter::step(cx, &mut iter)? {
            push(cx, value)?;
        }
        Ok(())
    })();
    cx.heap.roots().truncate(mark);
    result.map(|()| Value::int32(next as i32))
}

/// The length of `value` when it is a fast array in QuickJS's sense (dense,
/// with an element at every index below its length).
fn fast_array_length(cx: &Cx<'_>, value: Value) -> Option<u32> {
    let array = value.as_object()?;
    let heap = &*cx.heap;
    if heap.object_kind(array) != ObjectKind::Array
        || heap.header(array).has(crate::heap::object::flags::SPARSE)
        || heap.holey.contains(&array.0)
    {
        return None;
    }
    let length = heap.array_length(array);
    (0..length)
        .all(|index| !heap.get_element(array, index).is_empty())
        .then_some(length)
}
