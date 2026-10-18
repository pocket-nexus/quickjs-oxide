//! `Iterator` and the iterator helpers: the abstract constructor,
//! `Iterator.from` and `Iterator.concat`, and on `%Iterator.prototype%` the
//! lazy helpers (`map`, `filter`, `take`, `drop`, `flatMap`) and the eager
//! consumers (`reduce`, `toArray`, `forEach`, `some`, `every`, `find`).
//!
//! A helper is an internal record (tag `IteratorHelper`) whose slots hold
//! the underlying iterator and its `next`, the helper's kind, its callback,
//! its limit or counter, the generator state, and for `flatMap` the inner
//! iterator and its `next`. `next` runs the helper's closure (27.1.4.x)
//! from one yield to the next; `return` resumes it with a return
//! completion, which closes the iterators.

use super::{Native, methods, plain_object, to_string_tag};
use crate::heap::HeapError;
use crate::heap::object::{InternalTag, ObjectKind};
use crate::runtime::{Intrinsic, Names, WellKnown};
use crate::shape::PropertyFlags;
use crate::value::{ObjectId, Value};
use crate::vm::native::CallSite;
use crate::vm::object as internal;
use crate::vm::{Cx, Throw, error, iter, ops, property};

pub(super) fn install(cx: &mut Cx<'_>) -> Result<(), HeapError> {
    let iterator_prototype = cx.realm.intrinsic(Intrinsic::IteratorPrototype);
    methods(
        cx,
        iterator_prototype,
        &[
            Native::IteratorDrop,
            Native::IteratorFilter,
            Native::IteratorFlatMap,
            Native::IteratorMap,
            Native::IteratorTake,
            Native::IteratorEvery,
            Native::IteratorFind,
            Native::IteratorForEach,
            Native::IteratorSome,
            Native::IteratorReduce,
            Native::IteratorToArray,
        ],
    )?;
    let constructor = super::native_function(cx, Native::IteratorConstructor)?;
    cx.set_intrinsic(Intrinsic::Iterator, constructor);
    methods(
        cx,
        constructor,
        &[Native::IteratorConcat, Native::IteratorFrom],
    )?;
    let names = cx.realm.names;
    cx.heap.add_property(
        constructor,
        Names::key(names.prototype),
        PropertyFlags::data(false, false, false),
        Value::object(iterator_prototype),
    )?;
    // 27.1.4.1: `get constructor` and `set constructor` (QuickJS uses one
    // nameless function as both halves).
    super::accessor(
        cx,
        iterator_prototype,
        Names::key(names.constructor),
        "constructor",
        Some(Native::IteratorConstructorGetter),
        Some(Native::IteratorConstructorSetter),
    )?;
    super::accessor(
        cx,
        iterator_prototype,
        WellKnown::key(cx.realm.symbols.to_string_tag),
        "[Symbol.toStringTag]",
        Some(Native::IteratorToStringTagGetter),
        Some(Native::IteratorToStringTagSetter),
    )?;
    let global = cx.realm.global();
    super::define(
        cx,
        global,
        "Iterator",
        Value::object(constructor),
        super::hidden(),
    )?;

    let helper = plain_object(cx, Some(iterator_prototype), 3)?;
    cx.set_intrinsic(Intrinsic::IteratorHelperPrototype, helper);
    methods(
        cx,
        helper,
        &[Native::IteratorHelperNext, Native::IteratorHelperReturn],
    )?;
    to_string_tag(cx, helper, "Iterator Helper")?;
    let wrap = plain_object(cx, Some(iterator_prototype), 2)?;
    cx.set_intrinsic(Intrinsic::WrapForValidIteratorPrototype, wrap);
    methods(
        cx,
        wrap,
        &[Native::WrapForValidNext, Native::WrapForValidReturn],
    )
}

fn root(cx: &mut Cx<'_>, value: Value) {
    cx.heap.roots().push(value);
}

// ---- the constructor and its accessors ----

/// `Iterator()`: abstract; only a subclass constructs.
pub(super) fn constructor(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let iterator = cx.realm.intrinsic(Intrinsic::Iterator);
    if !site.construct {
        return Err(error::type_error(cx, "constructor requires 'new'"));
    }
    if site.new_target == Value::object(iterator) {
        return Err(error::type_error(cx, "abstract class not constructable"));
    }
    let prototype =
        internal::prototype_from_constructor(cx, site.new_target, Intrinsic::IteratorPrototype)?;
    cx.heap
        .alloc_internal(
            ObjectKind::Builtin,
            InternalTag::Iterator,
            Some(prototype),
            0,
            &[],
            Default::default(),
            0,
        )
        .map(Value::object)
        .map_err(|error| error::heap_error(cx, error))
}

/// `%Iterator.prototype%.constructor`'s getter and setter.
pub(super) fn constructor_getter(cx: &mut Cx<'_>, _site: CallSite) -> Result<Value, Throw> {
    Ok(Value::object(cx.realm.intrinsic(Intrinsic::Iterator)))
}

pub(super) fn constructor_setter(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let value = site.arg(cx, 0);
    let this = site.this(cx);
    let names = cx.realm.names;
    setter_ignoring_prototype(cx, this, Names::key(names.constructor), value)
}

pub(super) fn to_string_tag_getter(cx: &mut Cx<'_>, _site: CallSite) -> Result<Value, Throw> {
    Ok(Value::string(cx.realm.names.iterator_tag))
}

pub(super) fn to_string_tag_setter(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let value = site.arg(cx, 0);
    let this = site.this(cx);
    let key = WellKnown::key(cx.realm.symbols.to_string_tag);
    setter_ignoring_prototype(cx, this, key, value)
}

/// `SetterThatIgnoresPrototypeProperties(this, %Iterator.prototype%, p, v)`.
fn setter_ignoring_prototype(
    cx: &mut Cx<'_>,
    this: Value,
    key: crate::PropertyKey,
    value: Value,
) -> Result<Value, Throw> {
    let Some(object) = this.as_object() else {
        return Err(error::type_error(cx, "not an object"));
    };
    if object == cx.realm.intrinsic(Intrinsic::IteratorPrototype) {
        return Err(error::type_error(cx, "Cannot assign to read only property"));
    }
    if internal::get_own_property(cx, object, key)?.is_none() {
        internal::create_data_property_or_throw(cx, object, key, value)?;
    } else {
        internal::put(cx, object, key, value, true)?;
    }
    Ok(Value::UNDEFINED)
}

// ---- Iterator.from and the wrapper ----

/// `IteratorStepValue` over a record's iterator and `next`: the value, or
/// `None` when done (`value` is not read then).
fn step_value(cx: &mut Cx<'_>, iterator: Value, next: Value) -> Result<Option<Value>, Throw> {
    let result = crate::vm::call(cx, next, iterator, &[])?;
    step_result(cx, result)
}

/// [`step_value`] calling `next` without a native frame of its own, as
/// QuickJS's concat steps its items (the backtrace shows no extra frame).
fn step_value_direct(
    cx: &mut Cx<'_>,
    iterator: Value,
    next: Value,
) -> Result<Option<Value>, Throw> {
    let result = crate::vm::call_next(cx, next, iterator)?;
    step_result(cx, result)
}

fn step_result(cx: &mut Cx<'_>, result: Value) -> Result<Option<Value>, Throw> {
    let Some(result) = result.as_object() else {
        return Err(error::type_error(cx, "iterator must return an object"));
    };
    root(cx, Value::object(result));
    let names = cx.realm.names;
    let done = internal::get_property(cx, result, Names::key(names.done))?;
    if ops::to_boolean(cx.heap, done) {
        return Ok(None);
    }
    let value = internal::get_property(cx, result, Names::key(names.value))?;
    root(cx, value);
    Ok(Some(value))
}

/// `IteratorStep`: whether the iterator produced a value (it is not read).
fn step_only(cx: &mut Cx<'_>, iterator: Value, next: Value) -> Result<bool, Throw> {
    let result = crate::vm::call(cx, next, iterator, &[])?;
    let Some(result) = result.as_object() else {
        return Err(error::type_error(cx, "iterator must return an object"));
    };
    root(cx, Value::object(result));
    let done = internal::get_property(cx, result, Names::key(cx.realm.names.done))?;
    Ok(!ops::to_boolean(cx.heap, done))
}

/// `GetIteratorFlattenable(value, primitives)` (7.4.x): the iterator and its
/// `next`. A string primitive is iterated when `strings` allows it.
fn flattenable(cx: &mut Cx<'_>, value: Value, strings: bool) -> Result<(Value, Value), Throw> {
    if value.as_object().is_none() && !(strings && value.as_string().is_some()) {
        return Err(error::type_error(cx, "not an object"));
    }
    let key = WellKnown::key(cx.realm.symbols.iterator);
    let iterator = match internal::get_method(cx, value, key)? {
        None => value,
        Some(method) => {
            root(cx, method);
            crate::vm::call(cx, method, value, &[])?
        }
    };
    if iterator.as_object().is_none() {
        return Err(error::type_error(cx, "not an object"));
    }
    root(cx, iterator);
    let next = property::get(cx, iterator, Names::key(cx.realm.names.next))?;
    root(cx, next);
    Ok((iterator, next))
}

/// `Iterator.from(value)` (27.1.3.2.1).
pub(super) fn from(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let value = site.arg(cx, 0);
    let (iterator, next) = flattenable(cx, value, true)?;
    let constructor = cx.realm.intrinsic(Intrinsic::Iterator);
    if internal::ordinary_has_instance(cx, Value::object(constructor), iterator)? {
        return Ok(iterator);
    }
    let prototype = cx.realm.intrinsic(Intrinsic::WrapForValidIteratorPrototype);
    cx.heap
        .alloc_internal(
            ObjectKind::Builtin,
            InternalTag::WrapForValidIterator,
            Some(prototype),
            0,
            &[iterator, next],
            Default::default(),
            0,
        )
        .map(Value::object)
        .map_err(|error| error::heap_error(cx, error))
}

fn this_tagged(cx: &mut Cx<'_>, site: &CallSite, tag: InternalTag) -> Result<ObjectId, Throw> {
    let this = site.this(cx);
    match this.as_object() {
        Some(object) if cx.heap.internal_tag(object) == tag => Ok(object),
        _ => Err(error::object_expected(
            cx,
            match tag {
                InternalTag::WrapForValidIterator => "Iterator Wrap",
                InternalTag::IteratorHelper => "Iterator Helper",
                _ => "Iterator Concat",
            },
        )),
    }
}

/// `%WrapForValidIteratorPrototype%.next`: the wrapped `next`'s result.
pub(super) fn wrap_next(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let wrapper = this_tagged(cx, &site, InternalTag::WrapForValidIterator)?;
    let iterator = cx.heap.internal_slot(wrapper, 0);
    let next = cx.heap.internal_slot(wrapper, 1);
    crate::vm::call(cx, next, iterator, &[])
}

/// `%WrapForValidIteratorPrototype%.return`: the wrapped `return`'s result.
pub(super) fn wrap_return(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let wrapper = this_tagged(cx, &site, InternalTag::WrapForValidIterator)?;
    let iterator = cx.heap.internal_slot(wrapper, 0);
    let key = Names::key(cx.realm.names.return_);
    match internal::get_method(cx, iterator, key)? {
        None => iter::result_object(cx, Value::UNDEFINED, true),
        Some(method) => crate::vm::call(cx, method, iterator, &[]),
    }
}

// ---- the helpers ----

/// A helper's kind.
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
enum Kind {
    Map = 0,
    Filter = 1,
    Take = 2,
    Drop = 3,
    FlatMap = 4,
    Concat = 5,
}

// Slots of a helper record: the underlying iterator and `next`, the kind,
// the callback, the limit or counter, the generator state (started,
// executing, done), and for `flatMap` the inner iterator and its `next`.
// A concat's iterator and `next` are the open item's (undefined between
// items), its count the index of that item, and its items follow as
// (open method, iterable) pairs.
const ITERATOR: usize = 0;
const NEXT: usize = 1;
const KIND: usize = 2;
const CALLBACK: usize = 3;
const COUNT: usize = 4;
const EXECUTING: usize = 5;
const DONE: usize = 6;
const INNER: usize = 7;
const INNER_NEXT: usize = 8;
const STARTED: usize = 9;
const CONCAT_ITEMS: usize = 10;

/// The receiver of an `Iterator.prototype` method: an object.
fn this_iterator(cx: &mut Cx<'_>, site: &CallSite) -> Result<Value, Throw> {
    let this = site.this(cx);
    if this.as_object().is_none() {
        return Err(error::type_error(cx, "not an object"));
    }
    Ok(this)
}

/// Closes `iterator` (whose `next` was not read yet) after an argument
/// check failed, rethrowing that failure.
fn close_unread(cx: &mut Cx<'_>, iterator: Value, failure: Throw) -> Throw {
    let record = iter::Iter::Protocol {
        iterator,
        next: Value::UNDEFINED,
    };
    match iter::close(cx, &record, Err(failure)) {
        Err(throw) => throw,
        Ok(()) => unreachable!("close rethrows"),
    }
}

/// A helper (27.1.4.x): the limit's conversion or the callback's check,
/// whose failure closes the receiver, then GetIteratorDirect (the
/// receiver's `next`).
fn create_helper(cx: &mut Cx<'_>, site: CallSite, kind: Kind) -> Result<Value, Throw> {
    let iterator = this_iterator(cx, &site)?;
    let argument = site.arg(cx, 0);
    let mut callback = Value::UNDEFINED;
    let mut count = 0i64;
    match kind {
        Kind::Take | Kind::Drop => {
            let limit = match ops::to_number(cx, argument) {
                Ok(limit) => limit,
                Err(throw) => return Err(close_unread(cx, iterator, throw)),
            };
            // ToIntegerOrInfinity (27.1.4.13, 27.1.4.4): a count past 2^53 - 1
            // never runs out, as +Infinity does not.
            count = if limit.is_nan() || limit.trunc() < 0.0 {
                -1
            } else {
                limit.trunc().min(((1u64 << 53) - 1) as f64) as i64
            };
            if count < 0 {
                let throw = error::range_error(cx, "must be positive");
                return Err(close_unread(cx, iterator, throw));
            }
        }
        _ => {
            if !ops::is_callable(cx.heap, argument) {
                let throw = error::type_error(cx, "not a function");
                return Err(close_unread(cx, iterator, throw));
            }
            callback = argument;
        }
    }
    let next = property::get(cx, iterator, Names::key(cx.realm.names.next))?;
    root(cx, next);
    let prototype = cx.realm.intrinsic(Intrinsic::IteratorHelperPrototype);
    cx.heap
        .alloc_internal(
            ObjectKind::Builtin,
            InternalTag::IteratorHelper,
            Some(prototype),
            0,
            &[
                iterator,
                next,
                Value::int32(kind as i32),
                callback,
                Value::number(count as f64),
                Value::FALSE,
                Value::FALSE,
                Value::UNDEFINED,
                Value::UNDEFINED,
                Value::FALSE,
            ],
            Default::default(),
            0,
        )
        .map(Value::object)
        .map_err(|error| error::heap_error(cx, error))
}

pub(super) fn map(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    create_helper(cx, site, Kind::Map)
}

pub(super) fn filter(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    create_helper(cx, site, Kind::Filter)
}

pub(super) fn take(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    create_helper(cx, site, Kind::Take)
}

pub(super) fn drop(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    create_helper(cx, site, Kind::Drop)
}

pub(super) fn flat_map(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    create_helper(cx, site, Kind::FlatMap)
}

fn slot(cx: &Cx<'_>, helper: ObjectId, index: usize) -> Value {
    cx.heap.internal_slot(helper, index)
}

fn set_slot(cx: &mut Cx<'_>, helper: ObjectId, index: usize, value: Value) {
    cx.heap.set_internal_slot(helper, index, value);
}

/// How a helper step failed: IfAbruptCloseIterator closes the underlying
/// iterator (keeping the failure); a failure of the underlying iterator's
/// own step does not.
enum Failure {
    Close(Throw),
    NoClose(Throw),
}

fn take_count(cx: &mut Cx<'_>, helper: ObjectId) -> f64 {
    slot(cx, helper, COUNT).as_f64().unwrap()
}

/// The counter value passed to the callback, incremented.
fn next_index(cx: &mut Cx<'_>, helper: ObjectId) -> Value {
    let count = take_count(cx, helper);
    set_slot(cx, helper, COUNT, Value::number(count + 1.0));
    crate::vm::number::number(count)
}

/// The helper's closure run from its last yield to the next one: the value
/// yielded, or `None` when it returns.
fn helper_step(cx: &mut Cx<'_>, helper: ObjectId) -> Result<Option<Value>, Failure> {
    let kind = slot(cx, helper, KIND).as_int32().unwrap();
    let (iterator, next) = (slot(cx, helper, ITERATOR), slot(cx, helper, NEXT));
    let callback = slot(cx, helper, CALLBACK);
    let step = |cx: &mut Cx<'_>| step_value(cx, iterator, next).map_err(Failure::NoClose);
    match kind {
        k if k == Kind::Map as i32 => {
            let Some(item) = step(cx)? else {
                return Ok(None);
            };
            let index = next_index(cx, helper);
            crate::vm::call(cx, callback, Value::UNDEFINED, &[item, index])
                .map(Some)
                .map_err(Failure::Close)
        }
        k if k == Kind::Filter as i32 => loop {
            let Some(item) = step(cx)? else {
                return Ok(None);
            };
            let index = next_index(cx, helper);
            let selected = crate::vm::call(cx, callback, Value::UNDEFINED, &[item, index])
                .map_err(Failure::Close)?;
            if ops::to_boolean(cx.heap, selected) {
                return Ok(Some(item));
            }
        },
        k if k == Kind::Take as i32 => {
            if take_count(cx, helper) == 0.0 {
                // `IteratorClose(iterated, ReturnCompletion(undefined))`.
                let record = iter::Iter::Protocol {
                    iterator,
                    next: Value::UNDEFINED,
                };
                iter::close(cx, &record, Ok(())).map_err(Failure::NoClose)?;
                return Ok(None);
            }
            let count = take_count(cx, helper);
            set_slot(cx, helper, COUNT, Value::number(count - 1.0));
            step(cx)
        }
        k if k == Kind::Drop as i32 => {
            // The skipped values are stepped with IteratorStep, unread.
            while take_count(cx, helper) > 0.0 {
                let count = take_count(cx, helper);
                set_slot(cx, helper, COUNT, Value::number(count - 1.0));
                if !step_only(cx, iterator, next).map_err(Failure::NoClose)? {
                    return Ok(None);
                }
            }
            step(cx)
        }
        k if k == Kind::Concat as i32 => loop {
            if slot(cx, helper, ITERATOR).is_undefined() {
                let index = take_count(cx, helper) as usize;
                let count = (cx.heap.internal(helper).slots.len() - CONCAT_ITEMS) / 2;
                if index >= count {
                    return Ok(None);
                }
                let at = CONCAT_ITEMS + index * 2;
                let (method, iterable) = (slot(cx, helper, at), slot(cx, helper, at + 1));
                let iterator =
                    crate::vm::call(cx, method, iterable, &[]).map_err(Failure::NoClose)?;
                if iterator.as_object().is_none() {
                    return Err(Failure::NoClose(error::type_error(cx, "not an object")));
                }
                root(cx, iterator);
                let next = property::get(cx, iterator, Names::key(cx.realm.names.next))
                    .map_err(Failure::NoClose)?;
                set_slot(cx, helper, ITERATOR, iterator);
                set_slot(cx, helper, NEXT, next);
            }
            let (iterator, next) = (slot(cx, helper, ITERATOR), slot(cx, helper, NEXT));
            match step_value_direct(cx, iterator, next).map_err(Failure::NoClose)? {
                Some(value) => return Ok(Some(value)),
                None => {
                    set_slot(cx, helper, ITERATOR, Value::UNDEFINED);
                    set_slot(cx, helper, NEXT, Value::UNDEFINED);
                    let index = take_count(cx, helper);
                    set_slot(cx, helper, COUNT, Value::number(index + 1.0));
                }
            }
        },
        _ => loop {
            if slot(cx, helper, INNER).is_undefined() {
                let Some(item) = step(cx)? else {
                    return Ok(None);
                };
                let index = next_index(cx, helper);
                let mapped = crate::vm::call(cx, callback, Value::UNDEFINED, &[item, index])
                    .map_err(Failure::Close)?;
                root(cx, mapped);
                let (inner, inner_next) = flattenable(cx, mapped, false).map_err(Failure::Close)?;
                set_slot(cx, helper, INNER, inner);
                set_slot(cx, helper, INNER_NEXT, inner_next);
            }
            let (inner, inner_next) = (slot(cx, helper, INNER), slot(cx, helper, INNER_NEXT));
            match step_value(cx, inner, inner_next).map_err(Failure::Close)? {
                Some(value) => return Ok(Some(value)),
                None => {
                    set_slot(cx, helper, INNER, Value::UNDEFINED);
                    set_slot(cx, helper, INNER_NEXT, Value::UNDEFINED);
                }
            }
        },
    }
}

/// `%IteratorHelperPrototype%.next` (27.1.2.1.1): GeneratorResume of the
/// helper's closure.
pub(super) fn helper_next(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let helper = this_tagged(cx, &site, InternalTag::IteratorHelper)?;
    if slot(cx, helper, EXECUTING) == Value::TRUE {
        return Err(error::type_error(cx, "cannot invoke a running iterator"));
    }
    if slot(cx, helper, DONE) == Value::TRUE {
        return iter::result_object(cx, Value::UNDEFINED, true);
    }
    set_slot(cx, helper, STARTED, Value::TRUE);
    set_slot(cx, helper, EXECUTING, Value::TRUE);
    let result = match helper_step(cx, helper) {
        Ok(value) => Ok(value),
        Err(Failure::NoClose(throw)) => Err(throw),
        Err(Failure::Close(throw)) => {
            let underlying = iter::Iter::Protocol {
                iterator: slot(cx, helper, ITERATOR),
                next: Value::UNDEFINED,
            };
            match iter::close(cx, &underlying, Err(throw)) {
                Err(throw) => Err(throw),
                Ok(()) => unreachable!("close rethrows"),
            }
        }
    };
    set_slot(cx, helper, EXECUTING, Value::FALSE);
    match result {
        Ok(Some(value)) => iter::result_object(cx, value, false),
        Ok(None) => {
            set_slot(cx, helper, DONE, Value::TRUE);
            iter::result_object(cx, Value::UNDEFINED, true)
        }
        Err(throw) => {
            set_slot(cx, helper, DONE, Value::TRUE);
            Err(throw)
        }
    }
}

/// `%IteratorHelperPrototype%.return` (27.1.2.1.2): before the first `next`
/// the underlying iterator is closed; at a yield the closure resumes with a
/// return completion, which closes `flatMap`'s inner iterator (a failure of
/// that closes the underlying one and wins) and then the underlying one.
pub(super) fn helper_return(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let helper = this_tagged(cx, &site, InternalTag::IteratorHelper)?;
    if slot(cx, helper, EXECUTING) == Value::TRUE {
        return Err(error::type_error(cx, "cannot invoke a running iterator"));
    }
    if slot(cx, helper, DONE) == Value::TRUE {
        return iter::result_object(cx, Value::UNDEFINED, true);
    }
    let underlying = iter::Iter::Protocol {
        iterator: slot(cx, helper, ITERATOR),
        next: Value::UNDEFINED,
    };
    // A concat has no underlying iterator before its first `next`.
    if slot(cx, helper, STARTED) != Value::TRUE {
        set_slot(cx, helper, DONE, Value::TRUE);
        if !slot(cx, helper, ITERATOR).is_undefined() {
            iter::close(cx, &underlying, Ok(()))?;
        }
        return iter::result_object(cx, Value::UNDEFINED, true);
    }
    set_slot(cx, helper, EXECUTING, Value::TRUE);
    let inner = slot(cx, helper, INNER);
    let result = (|| {
        if !inner.is_undefined() {
            let record = iter::Iter::Protocol {
                iterator: inner,
                next: Value::UNDEFINED,
            };
            if let Err(throw) = iter::close(cx, &record, Ok(())) {
                return iter::close(cx, &underlying, Err(throw));
            }
        }
        iter::close(cx, &underlying, Ok(()))
    })();
    set_slot(cx, helper, EXECUTING, Value::FALSE);
    set_slot(cx, helper, DONE, Value::TRUE);
    result?;
    iter::result_object(cx, Value::UNDEFINED, true)
}

// ---- the consumers ----

/// The receiver's direct record after checking `callback`, closing the
/// receiver if it is not callable.
fn consumer(cx: &mut Cx<'_>, site: &CallSite) -> Result<(iter::Iter, Value), Throw> {
    let iterator = this_iterator(cx, site)?;
    let callback = site.arg(cx, 0);
    if !ops::is_callable(cx.heap, callback) {
        let throw = error::type_error(cx, "not a function");
        return Err(close_unread(cx, iterator, throw));
    }
    Ok((iter::direct(cx, iterator)?, callback))
}

/// Runs `callback(value, counter)` over the iterator until `stop` says so;
/// returns the value that stopped it.
fn consume(
    cx: &mut Cx<'_>,
    site: &CallSite,
    mut stop: impl FnMut(&Cx<'_>, Value) -> bool,
) -> Result<Option<(Value, Value)>, Throw> {
    let (mut record, callback) = consumer(cx, site)?;
    let mut counter = 0.0;
    while let Some(value) = iter::step(cx, &mut record)? {
        let mark = cx.heap.roots().mark();
        root(cx, value);
        let result = crate::vm::call(
            cx,
            callback,
            Value::UNDEFINED,
            &[value, crate::vm::number::number(counter)],
        );
        counter += 1.0;
        let result = match result {
            Ok(result) => result,
            Err(throw) => {
                iter::close(cx, &record, Err(throw))?;
                unreachable!("close rethrows");
            }
        };
        cx.heap.roots().truncate(mark);
        if stop(cx, result) {
            root(cx, value);
            iter::close(cx, &record, Ok(()))?;
            return Ok(Some((value, result)));
        }
    }
    Ok(None)
}

pub(super) fn for_each(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    consume(cx, &site, |_, _| false)?;
    Ok(Value::UNDEFINED)
}

pub(super) fn some(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let found = consume(cx, &site, |cx, result| ops::to_boolean(cx.heap, result))?;
    Ok(Value::bool(found.is_some()))
}

pub(super) fn every(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let failed = consume(cx, &site, |cx, result| !ops::to_boolean(cx.heap, result))?;
    Ok(Value::bool(failed.is_none()))
}

pub(super) fn find(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let found = consume(cx, &site, |cx, result| ops::to_boolean(cx.heap, result))?;
    Ok(found.map_or(Value::UNDEFINED, |(value, _)| value))
}

/// `reduce(reducer, initialValue)`.
pub(super) fn reduce(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let (mut record, reducer) = consumer(cx, &site)?;
    let mut counter = 0.0;
    let mut accumulator = if site.argc >= 2 {
        site.arg(cx, 1)
    } else {
        match iter::step(cx, &mut record)? {
            Some(value) => {
                counter = 1.0;
                value
            }
            None => return Err(error::type_error(cx, "empty iterator")),
        }
    };
    let mark = cx.heap.roots().mark();
    root(cx, accumulator);
    while let Some(value) = iter::step(cx, &mut record)? {
        root(cx, value);
        let result = crate::vm::call(
            cx,
            reducer,
            Value::UNDEFINED,
            &[accumulator, value, crate::vm::number::number(counter)],
        );
        counter += 1.0;
        accumulator = match result {
            Ok(result) => result,
            Err(throw) => {
                iter::close(cx, &record, Err(throw))?;
                unreachable!("close rethrows");
            }
        };
        cx.heap.roots().truncate(mark);
        root(cx, accumulator);
    }
    Ok(accumulator)
}

pub(super) fn to_array(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let iterator = this_iterator(cx, &site)?;
    let mut record = iter::direct(cx, iterator)?;
    let array = internal::array_create(cx, 0)?;
    root(cx, Value::object(array));
    while let Some(value) = iter::step(cx, &mut record)? {
        root(cx, value);
        cx.heap
            .push_element(array, value)
            .map_err(|error| error::heap_error(cx, error))?;
    }
    Ok(Value::object(array))
}

// ---- Iterator.concat ----

/// `Iterator.concat(...items)`: an Iterator Helper over the items' iterators
/// in turn, each opened when reached.
pub(super) fn concat(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let mut slots = vec![
        Value::UNDEFINED,
        Value::UNDEFINED,
        Value::int32(Kind::Concat as i32),
        Value::UNDEFINED,
        Value::int32(0),
        Value::FALSE,
        Value::FALSE,
        Value::UNDEFINED,
        Value::UNDEFINED,
        Value::FALSE,
    ];
    let key = WellKnown::key(cx.realm.symbols.iterator);
    for index in 0..site.argc {
        let item = site.arg(cx, index);
        if item.as_object().is_none() {
            return Err(error::type_error(cx, "not an object"));
        }
        let Some(method) = internal::get_method_or(cx, item, key, "not a function")? else {
            return Err(error::type_error(cx, "not a function"));
        };
        root(cx, method);
        slots.push(method);
        slots.push(item);
    }
    let prototype = cx.realm.intrinsic(Intrinsic::IteratorHelperPrototype);
    cx.heap
        .alloc_internal(
            ObjectKind::Builtin,
            InternalTag::IteratorHelper,
            Some(prototype),
            0,
            &slots,
            Default::default(),
            0,
        )
        .map(Value::object)
        .map_err(|error| error::heap_error(cx, error))
}
