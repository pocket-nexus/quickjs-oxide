//! The iterator protocol for natives and the loop's slow paths (phase 4,
//! section 4.2): `GetIterator`, `IteratorStepValue` and `IteratorClose`.
//!
//! An [`Iter`] over an unmodified `Array` (no own `@@iterator`, and the
//! realm's array iteration still pristine) walks the elements directly and
//! allocates neither the iterator nor any `{value, done}` result; every
//! other iterable goes through its `next` method.

use super::error;
use super::object;
use super::ops;
use super::property;
use super::{Cx, Throw};
use crate::heap::object::ObjectKind;
use crate::runtime::{Intrinsic, Names, WellKnown};
use crate::value::{ObjectId, Value};

/// An iterator record. The values it names are rooted on the root stack by
/// [`get_iterator`] until the caller cuts the stack back.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Iter {
    /// The general protocol: the iterator and its `next` method.
    Protocol { iterator: Value, next: Value },
    /// A pristine array walked by index; `length` is read live.
    Array { array: ObjectId, index: u32 },
}

/// Whether `value` iterates as an unmodified array: an `Array` whose
/// `@@iterator` and `%ArrayIteratorPrototype%.next` are the intrinsic ones.
pub(crate) fn is_pristine_array(cx: &Cx<'_>, value: Value) -> bool {
    let Some(array) = value.as_object() else {
        return false;
    };
    if cx.heap.object_kind(array) != ObjectKind::Array {
        return false;
    }
    let array_prototype = cx.realm.intrinsic(Intrinsic::ArrayPrototype);
    if cx.heap.prototype_of(array) != Some(array_prototype) {
        return false;
    }
    let iterator = WellKnown::key(cx.realm.symbols.iterator);
    if cx.heap.lookup_own(array, iterator).is_some() {
        return false;
    }
    array_iteration_pristine(cx)
}

/// Whether `%Array.prototype%[@@iterator]` is `%Array.prototype.values%`
/// and `%ArrayIteratorPrototype%.next` the intrinsic `next`: the
/// conditions under which walking an array by index is unobservable.
pub(crate) fn array_iteration_pristine(cx: &Cx<'_>) -> bool {
    let heap = &*cx.heap;
    let values = cx.realm.intrinsic(Intrinsic::ArrayValues);
    let array_prototype = cx.realm.intrinsic(Intrinsic::ArrayPrototype);
    if values == cx.realm.intrinsic(Intrinsic::ObjectPrototype) {
        // Array iterators are not installed.
        return false;
    }
    let iterator = WellKnown::key(cx.realm.symbols.iterator);
    match heap.lookup_own(array_prototype, iterator) {
        Some((slot, flags))
            if !flags.has(crate::shape::PropertyFlags::ACCESSOR)
                && heap.get_slot(array_prototype, slot) == Value::object(values) => {}
        _ => return false,
    }
    let iterator_prototype = cx.realm.intrinsic(Intrinsic::ArrayIteratorPrototype);
    match heap.lookup_own(iterator_prototype, Names::key(cx.realm.names.next)) {
        Some((slot, flags)) if !flags.has(crate::shape::PropertyFlags::ACCESSOR) => {
            let next = heap.get_slot(iterator_prototype, slot);
            next.as_object().is_some_and(|next| {
                heap.object_kind(next) == ObjectKind::Function
                    && heap.function_record(next).native
                        == crate::builtins::Native::ArrayIteratorNext.index()
            })
        }
        _ => false,
    }
}

/// `GetIterator(value, sync)`.
pub(crate) fn get_iterator(cx: &mut Cx<'_>, value: Value) -> Result<Iter, Throw> {
    if is_pristine_array(cx, value) {
        let array = value.as_object().unwrap();
        cx.heap.roots().push(value);
        return Ok(Iter::Array { array, index: 0 });
    }
    let key = WellKnown::key(cx.realm.symbols.iterator);
    let Some(method) = object::get_method(cx, value, key)? else {
        return Err(error::type_error(cx, "value is not iterable"));
    };
    cx.heap.roots().push(method);
    from_method(cx, value, method)
}

/// `GetIteratorFromMethod(value, method)`.
pub(crate) fn from_method(cx: &mut Cx<'_>, value: Value, method: Value) -> Result<Iter, Throw> {
    let iterator = super::call(cx, method, value, &[])?;
    if iterator.as_object().is_none() {
        return Err(error::type_error(cx, "not an object"));
    }
    cx.heap.roots().push(iterator);
    let next = property::get(cx, iterator, Names::key(cx.realm.names.next))?;
    cx.heap.roots().push(next);
    Ok(Iter::Protocol { iterator, next })
}

/// `GetIteratorDirect(obj)`: an iterator record over an object and its
/// `next`, for the iterator helpers.
pub(crate) fn direct(cx: &mut Cx<'_>, iterator: Value) -> Result<Iter, Throw> {
    if iterator.as_object().is_none() {
        return Err(error::type_error(cx, "not an object"));
    }
    cx.heap.roots().push(iterator);
    let next = property::get(cx, iterator, Names::key(cx.realm.names.next))?;
    cx.heap.roots().push(next);
    Ok(Iter::Protocol { iterator, next })
}

/// `IteratorStepValue`: the next value, or `None` when done. The value is
/// unrooted.
pub(crate) fn step(cx: &mut Cx<'_>, iter: &mut Iter) -> Result<Option<Value>, Throw> {
    match iter {
        Iter::Array { array, index } => {
            let length = cx.heap.array_length(*array);
            if *index >= length {
                return Ok(None);
            }
            let value = cx.heap.get_element(*array, *index);
            let value = if value.is_empty() {
                let key = object::index_key(cx, *index)?;
                object::get_property(cx, *array, key)?
            } else {
                value
            };
            *index += 1;
            Ok(Some(value))
        }
        Iter::Protocol { iterator, next } => {
            let result = super::call_next(cx, *next, *iterator)?;
            let Some(result) = result.as_object() else {
                return Err(error::type_error(cx, "iterator must return an object"));
            };
            let mark = cx.heap.roots().mark();
            cx.heap.roots().push(Value::object(result));
            let names = cx.realm.names;
            let outcome = (|| {
                let done = object::get_property(cx, result, Names::key(names.done))?;
                if ops::to_boolean(cx.heap, done) {
                    return Ok(None);
                }
                object::get_property(cx, result, Names::key(names.value)).map(Some)
            })();
            cx.heap.roots().truncate(mark);
            outcome
        }
    }
}

/// `IteratorClose(iter, completion)` for a completion that is a throw or a
/// normal early exit: calls `return` if present. A throw completion wins
/// over anything `return` does; a normal one reports `return`'s errors.
pub(crate) fn close(
    cx: &mut Cx<'_>,
    iter: &Iter,
    completion: Result<(), Throw>,
) -> Result<(), Throw> {
    let Iter::Protocol { iterator, .. } = *iter else {
        return completion;
    };
    let mark = cx.heap.roots().mark();
    if let Err(Throw(thrown)) = completion {
        cx.heap.roots().push(thrown);
    }
    let result = (|| {
        let key = Names::key(cx.realm.names.return_);
        let method = match object::get_method(cx, iterator, key) {
            Ok(method) => method,
            Err(throw) => return Err(throw),
        };
        let Some(method) = method else {
            return Ok(None);
        };
        super::call(cx, method, iterator, &[]).map(Some)
    })();
    cx.heap.roots().truncate(mark);
    completion?;
    match result? {
        Some(value) if value.as_object().is_none() => Err(error::type_error(cx, "not an object")),
        _ => Ok(()),
    }
}

/// IteratorToList(GetIterator(value)): every value of an iterable, rooted.
/// A throw while stepping does not close the iterator (7.4.x).
pub(crate) fn collect(cx: &mut Cx<'_>, value: Value) -> Result<Vec<Value>, Throw> {
    let mut iter = get_iterator(cx, value)?;
    let mut values = Vec::new();
    while let Some(value) = step(cx, &mut iter)? {
        cx.heap.roots().push(value);
        values.push(value);
    }
    Ok(values)
}

/// `CreateIterResultObject(value, done)`.
pub(crate) fn result_object(cx: &mut Cx<'_>, value: Value, done: bool) -> Result<Value, Throw> {
    let prototype = cx.realm.intrinsic(Intrinsic::ObjectPrototype);
    let mark = cx.heap.roots().mark();
    cx.heap.roots().push(value);
    let names = cx.realm.names;
    let result = (|| {
        let object = cx
            .heap
            .alloc_object(ObjectKind::Ordinary, Some(prototype), 2)?;
        cx.heap.add_property(
            object,
            Names::key(names.value),
            crate::shape::PropertyFlags::DEFAULT,
            value,
        )?;
        cx.heap.add_property(
            object,
            Names::key(names.done),
            crate::shape::PropertyFlags::DEFAULT,
            Value::bool(done),
        )?;
        Ok(Value::object(object))
    })();
    cx.heap.roots().truncate(mark);
    result.map_err(|error| object::heap_error(cx, error))
}

// ---- the loop's iteration opcodes ----

use super::Frame;
use crate::bytecode::{Insn, Opcode};

/// Runs one iteration opcode (section 4.2) with the root stack cut back
/// afterwards. A record occupies three registers: the iterator, its `next`
/// and a done flag; or, for a pristine array, the array, `Value::EMPTY` and
/// the next index.
#[inline(never)]
pub(super) fn iter_op(cx: &mut Cx<'_>, frame: &Frame, insn: Insn) -> Result<(), Throw> {
    let mark = cx.heap.roots().mark();
    let result = iter_op_rooted(cx, frame, insn);
    cx.heap.roots().truncate(mark);
    result
}

fn iter_op_rooted(cx: &mut Cx<'_>, frame: &Frame, insn: Insn) -> Result<(), Throw> {
    let base = frame.base as usize;
    let at = |register: u8| base + register as usize;
    match insn.op {
        Opcode::IterStart => {
            let iterable = cx.heap.vm.registers[at(insn.b)];
            let generic = insn.imm != 0;
            let pristine = !generic && is_pristine_array(cx, iterable);
            cx.heap.ics_mut().count_iteration(pristine);
            let record = if pristine {
                [iterable, Value::EMPTY, Value::int32(0)]
            } else {
                let key = WellKnown::key(cx.realm.symbols.iterator);
                let Some(method) = object::get_method(cx, iterable, key)? else {
                    return Err(error::type_error(cx, "value is not iterable"));
                };
                cx.heap.roots().push(method);
                match from_method(cx, iterable, method)? {
                    Iter::Protocol { iterator, next } => [iterator, next, Value::FALSE],
                    Iter::Array { .. } => unreachable!(),
                }
            };
            let first = at(insn.a);
            cx.heap.vm.registers[first..first + 3].copy_from_slice(&record);
        }
        Opcode::IterNext => {
            let record = at(insn.b);
            let (value, done) = record_next(cx, record)?;
            cx.heap.vm.registers[at(insn.a)] = value;
            cx.heap.vm.registers[at(insn.c)] = Value::bool(done);
        }
        Opcode::IterClose | Opcode::IterCloseThrow => {
            let record = at(insn.a);
            let registers = &cx.heap.vm.registers;
            let (iterator, mark, done) = (
                registers[record],
                registers[record + 1],
                registers[record + 2],
            );
            if mark.is_empty() || done == Value::TRUE {
                return Ok(());
            }
            cx.heap.vm.registers[record + 2] = Value::TRUE;
            let iter = Iter::Protocol {
                iterator,
                next: mark,
            };
            if insn.op == Opcode::IterClose {
                close(cx, &iter, Ok(()))?;
            } else {
                let thrown = cx.heap.vm.registers[at(insn.b)];
                let _ = close(cx, &iter, Err(Throw(thrown)));
            }
        }
        Opcode::IterCallNext => {
            let record = at(insn.b);
            let (iterator, next) = (
                cx.heap.vm.registers[record],
                cx.heap.vm.registers[record + 1],
            );
            let value = cx.heap.vm.registers[at(insn.c)];
            let result = super::call(cx, next, iterator, &[value])?;
            cx.heap.vm.registers[at(insn.a)] = result;
        }
        Opcode::IterCall => {
            let record = at(insn.b);
            let iterator = cx.heap.vm.registers[record];
            let value = cx.heap.vm.registers[at(insn.c)];
            // The lowering's kinds: 0 `return(value)`, 1 `throw(value)`,
            // 2 `return()`.
            let names = cx.realm.names;
            let name = if insn.imm == 1 {
                names.throw
            } else {
                names.return_
            };
            let method = object::get_method(cx, iterator, Names::key(name))?;
            let args: &[Value] = if insn.imm == 2 { &[] } else { &[value] };
            let (result, missing) = match method {
                Some(method) => (super::call(cx, method, iterator, args)?, false),
                None => (value, true),
            };
            let dst = at(insn.a);
            cx.heap.vm.registers[dst] = result;
            cx.heap.vm.registers[dst + 1] = Value::bool(missing);
        }
        Opcode::IterCheckObject => {
            if cx.heap.vm.registers[at(insn.a)].as_object().is_none() {
                return Err(error::type_error(cx, "iterator must return an object"));
            }
        }
        Opcode::IterStartAsync => {
            let iterable = cx.heap.vm.registers[at(insn.b)];
            let (iterator, next) = get_async_iterator(cx, iterable)?;
            let first = at(insn.a);
            cx.heap.vm.registers[first..first + 3].copy_from_slice(&[iterator, next, Value::FALSE]);
        }
        Opcode::IterNextRaw => {
            let record = at(insn.b);
            let (iterator, next) = (
                cx.heap.vm.registers[record],
                cx.heap.vm.registers[record + 1],
            );
            cx.heap.vm.registers[record + 2] = Value::TRUE;
            let result = super::call(cx, next, iterator, &[])?;
            cx.heap.vm.registers[at(insn.a)] = result;
        }
        Opcode::IterResult => {
            let record = at(insn.b);
            let result = cx.heap.vm.registers[at(insn.a)];
            let Some(result) = result.as_object() else {
                return Err(error::type_error(cx, "iterator must return an object"));
            };
            let names = cx.realm.names;
            let done = object::get_property(cx, result, Names::key(names.done))?;
            let done = ops::to_boolean(cx.heap, done);
            // 14.7.5.6 ForIn/OfBodyEvaluation: a done result returns without
            // reading `value`, and the record stays done (no close).
            let value = if done {
                Value::UNDEFINED
            } else {
                object::get_property(cx, result, Names::key(names.value))?
            };
            if !done {
                cx.heap.vm.registers[record + 2] = Value::FALSE;
            }
            cx.heap.vm.registers[at(insn.a)] = value;
            cx.heap.vm.registers[at(insn.c)] = Value::bool(done);
        }
        _ => unreachable!("{} is not an iteration form", insn.op.name()),
    }
    Ok(())
}

/// The next value of the record at register `record`, and whether it is
/// done. A generic record is marked done before `next` runs, so a throw
/// from it (or from reading the result) leaves it done and unclosed.
fn record_next(cx: &mut Cx<'_>, record: usize) -> Result<(Value, bool), Throw> {
    let registers = &cx.heap.vm.registers;
    let (first, mark, state) = (
        registers[record],
        registers[record + 1],
        registers[record + 2],
    );
    if mark.is_empty() {
        let array = first.as_object().expect("an array");
        let index = state.as_int32().unwrap_or(i32::MAX) as u32;
        let length = cx.heap.array_length(array);
        if index >= length {
            cx.heap.vm.registers[record + 2] = Value::int32(i32::MAX);
            return Ok((Value::UNDEFINED, true));
        }
        let value = cx.heap.get_element(array, index);
        let value = if value.is_empty() {
            let key = object::index_key(cx, index)?;
            object::get_property(cx, array, key)?
        } else {
            value
        };
        cx.heap.vm.registers[record + 2] = Value::int32(index as i32 + 1);
        return Ok((value, false));
    }
    if state == Value::TRUE {
        return Ok((Value::UNDEFINED, true));
    }
    cx.heap.vm.registers[record + 2] = Value::TRUE;
    let mut iter = Iter::Protocol {
        iterator: first,
        next: mark,
    };
    match step(cx, &mut iter)? {
        Some(value) => {
            cx.heap.vm.registers[record + 2] = Value::FALSE;
            Ok((value, false))
        }
        None => Ok((Value::UNDEFINED, true)),
    }
}

/// `GetIterator(value, async)`: the iterator and its `next`, rooted.
pub(crate) fn get_async_iterator(cx: &mut Cx<'_>, value: Value) -> Result<(Value, Value), Throw> {
    let key = WellKnown::key(cx.realm.symbols.async_iterator);
    let method = object::get_method(cx, value, key)?;
    let Some(method) = method else {
        // QuickJS calls the `@@iterator` value unchecked here.
        let key = WellKnown::key(cx.realm.symbols.iterator);
        let method = property::get(cx, value, key)?;
        if !ops::is_callable(cx.heap, method) {
            return Err(error::type_error(cx, "not a function"));
        }
        cx.heap.roots().push(method);
        let sync = from_method(cx, value, method)?;
        let Iter::Protocol { iterator, next } = sync else {
            unreachable!()
        };
        let wrapper = crate::builtins::async_from_sync_iterator(cx, iterator, next)?;
        cx.heap.roots().push(wrapper);
        let next = property::get(cx, wrapper, Names::key(cx.realm.names.next))?;
        cx.heap.roots().push(next);
        return Ok((wrapper, next));
    };
    cx.heap.roots().push(method);
    match from_method(cx, value, method)? {
        Iter::Protocol { iterator, next } => Ok((iterator, next)),
        Iter::Array { .. } => unreachable!(),
    }
}
