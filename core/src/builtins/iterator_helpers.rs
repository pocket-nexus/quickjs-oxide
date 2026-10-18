//! `Iterator` and the iterator helpers (phase 4, section 6.1 row 8): the
//! abstract constructor, `Iterator.from` and `Iterator.concat`, and on
//! `%Iterator.prototype%` the lazy helpers (`map`, `filter`, `take`,
//! `drop`, `flatMap`) and the eager consumers (`reduce`, `toArray`,
//! `forEach`, `some`, `every`, `find`).
//!
//! A helper is QuickJS's `JSIteratorHelperData`: an internal record (tag
//! `IteratorHelper`) whose slots hold the underlying iterator and its
//! `next`, the helper's kind, its callback, its limit or counter, whether it
//! is running or done, and for `flatMap` the inner iterator. `next` and
//! `return` run QuickJS's `js_iterator_helper_next`, whose `return` steps
//! the underlying iterator with its `return` method.

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
    // QuickJS uses one nameless function as both halves of the
    // `constructor` accessor.
    let accessor = super::native_named(cx, Native::IteratorConstructorAccessor, "", None)?;
    cx.heap.add_accessor(
        iterator_prototype,
        Names::key(names.constructor),
        PropertyFlags::accessor(false, true),
        Value::object(accessor),
        Value::object(accessor),
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

    let concat = plain_object(cx, Some(iterator_prototype), 3)?;
    cx.set_intrinsic(Intrinsic::IteratorConcatPrototype, concat);
    methods(
        cx,
        concat,
        &[Native::IteratorConcatNext, Native::IteratorConcatReturn],
    )?;
    to_string_tag(cx, concat, "Iterator Concat")?;
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
pub(super) fn constructor_accessor(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    if site.argc == 0 {
        return Ok(Value::object(cx.realm.intrinsic(Intrinsic::Iterator)));
    }
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

/// QuickJS's `JS_IteratorNext(iterator, method)`: the value and whether
/// the iterator is done (`value` is not read then).
fn iterator_next(cx: &mut Cx<'_>, iterator: Value, method: Value) -> Result<(Value, bool), Throw> {
    let result = crate::vm::call(cx, method, iterator, &[])?;
    let Some(result) = result.as_object() else {
        return Err(error::type_error(cx, "iterator must return an object"));
    };
    root(cx, Value::object(result));
    let names = cx.realm.names;
    let done = internal::get_property(cx, result, Names::key(names.done))?;
    if ops::to_boolean(cx.heap, done) {
        return Ok((Value::UNDEFINED, true));
    }
    let value = internal::get_property(cx, result, Names::key(names.value))?;
    root(cx, value);
    Ok((value, false))
}

/// `Iterator.from(value)`, as QuickJS's `js_iterator_from`: a string or an
/// object without `@@iterator` is itself the iterator, primitive or not.
pub(super) fn from(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let value = site.arg(cx, 0);
    if value.as_object().is_none() && value.as_string().is_none() {
        return Err(error::type_error(cx, "Iterator.from called on non-object"));
    }
    let key = WellKnown::key(cx.realm.symbols.iterator);
    let method = property::get(cx, value, key)?;
    let iterator = if method.is_undefined() || method.is_null() {
        value
    } else {
        root(cx, method);
        let iterator = crate::vm::call(cx, method, value, &[])?;
        if iterator.as_object().is_none() {
            return Err(error::type_error(cx, "not an object"));
        }
        iterator
    };
    root(cx, iterator);
    let next = property::get(cx, iterator, Names::key(cx.realm.names.next))?;
    root(cx, next);
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

/// The wrapper's `next`: a fresh result from the wrapped iterator's.
pub(super) fn wrap_next(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let wrapper = this_tagged(cx, &site, InternalTag::WrapForValidIterator)?;
    let iterator = cx.heap.internal_slot(wrapper, 0);
    let next = cx.heap.internal_slot(wrapper, 1);
    let (value, done) = iterator_next(cx, iterator, next)?;
    iter::result_object(cx, value, done)
}

/// The wrapper's `return`: the wrapped iterator's `return` result itself.
pub(super) fn wrap_return(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let wrapper = this_tagged(cx, &site, InternalTag::WrapForValidIterator)?;
    let iterator = cx.heap.internal_slot(wrapper, 0);
    let key = Names::key(cx.realm.names.return_);
    let method = property::get(cx, iterator, key)?;
    if method.is_undefined() || method.is_null() {
        return iter::result_object(cx, Value::UNDEFINED, true);
    }
    let result = crate::vm::call(cx, method, iterator, &[])?;
    if result.as_object().is_none() {
        return Err(error::type_error(cx, "iterator must return an object"));
    }
    Ok(result)
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
}

// Slots of a helper record.
const ITERATOR: usize = 0;
const NEXT: usize = 1;
const KIND: usize = 2;
const CALLBACK: usize = 3;
const COUNT: usize = 4;
const EXECUTING: usize = 5;
const DONE: usize = 6;
const INNER: usize = 7;

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

/// QuickJS's `JS_ToInt64` of a finite number: its integer part modulo 2^64.
fn to_int64(number: f64) -> i64 {
    let bits = number.to_bits();
    let exponent = ((bits >> 52) & 0x7ff) as i64;
    if exponent <= 1023 + 62 {
        number as i64
    } else if exponent <= 1023 + 62 + 53 {
        let mantissa = (bits & ((1 << 52) - 1)) | (1 << 52);
        let value = mantissa.wrapping_shl((exponent - 1023 - 52) as u32) as i64;
        if bits >> 63 != 0 {
            value.wrapping_neg()
        } else {
            value
        }
    } else {
        0
    }
}

/// QuickJS's `js_create_iterator_helper`: the limit's conversion or the
/// callback's check, then the receiver's `next`; any failure closes the
/// receiver.
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
            count = if limit.is_nan() || limit == f64::NEG_INFINITY {
                -1
            } else if limit == f64::INFINITY {
                (1 << 53) - 1
            } else {
                to_int64(limit.trunc())
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
    let next = match property::get(cx, iterator, Names::key(cx.realm.names.next)) {
        Ok(next) => next,
        Err(throw) => return Err(close_unread(cx, iterator, throw)),
    };
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

/// How a helper step failed: QuickJS's `fail` closes the underlying
/// iterator (keeping the failure), `fail_no_close` does not.
enum Failure {
    Close(Throw),
    NoClose(Throw),
}

/// The method a helper steps the underlying iterator with: its `next`, or
/// for `return` its `return` method, read now.
fn step_method(cx: &mut Cx<'_>, helper: ObjectId, returning: bool) -> Result<Value, Failure> {
    if !returning {
        return Ok(slot(cx, helper, NEXT));
    }
    let iterator = slot(cx, helper, ITERATOR);
    let key = Names::key(cx.realm.names.return_);
    let method = property::get(cx, iterator, key).map_err(Failure::Close)?;
    root(cx, method);
    Ok(method)
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

/// One run of QuickJS's `js_iterator_helper_next`: the value and whether
/// the helper reports done, or the failure and that flag.
fn helper_body(
    cx: &mut Cx<'_>,
    helper: ObjectId,
    returning: bool,
    done: &mut bool,
) -> Result<Value, Failure> {
    let kind = slot(cx, helper, KIND).as_int32().unwrap();
    let iterator = slot(cx, helper, ITERATOR);
    let callback = slot(cx, helper, CALLBACK);
    *done = false;
    let next = |cx: &mut Cx<'_>, method: Value, done: &mut bool| {
        let result = iterator_next(cx, iterator, method).map_err(Failure::NoClose);
        *done = result.as_ref().is_ok_and(|&(_, done)| done);
        result.map(|(value, _)| value)
    };
    match kind {
        k if k == Kind::Drop as i32 => {
            let method = step_method(cx, helper, returning)?;
            while take_count(cx, helper) > 0.0 {
                let count = take_count(cx, helper);
                set_slot(cx, helper, COUNT, Value::number(count - 1.0));
                next(cx, method, done)?;
                if returning {
                    *done = true;
                }
                if *done {
                    return Ok(Value::UNDEFINED);
                }
            }
            next(cx, method, done)
        }
        k if k == Kind::Filter as i32 => {
            let method = step_method(cx, helper, returning)?;
            loop {
                let item = next(cx, method, done)?;
                if *done || returning {
                    return Ok(item);
                }
                let index = next_index(cx, helper);
                let selected = crate::vm::call(cx, callback, Value::UNDEFINED, &[item, index])
                    .map_err(Failure::Close)?;
                if ops::to_boolean(cx.heap, selected) {
                    return Ok(item);
                }
            }
        }
        k if k == Kind::Map as i32 => {
            let method = step_method(cx, helper, returning)?;
            let item = next(cx, method, done)?;
            if *done || returning {
                return Ok(item);
            }
            let index = next_index(cx, helper);
            crate::vm::call(cx, callback, Value::UNDEFINED, &[item, index]).map_err(Failure::Close)
        }
        k if k == Kind::Take as i32 => {
            if take_count(cx, helper) > 0.0 {
                let method = step_method(cx, helper, returning)?;
                let count = take_count(cx, helper);
                set_slot(cx, helper, COUNT, Value::number(count - 1.0));
                return next(cx, method, done);
            }
            *done = true;
            let record = iter::Iter::Protocol {
                iterator,
                next: Value::UNDEFINED,
            };
            iter::close(cx, &record, Ok(())).map_err(Failure::NoClose)?;
            Ok(Value::UNDEFINED)
        }
        _ => flat_map_body(cx, helper, returning, done),
    }
}

/// `flatMap`'s run. The inner iterator's `next` (or `return`) is read on
/// every step; a failure there closes the inner iterator, whose own failure
/// then replaces it, before the underlying one is closed. An exhausted inner
/// iterator is closed too, with any failure of that ignored.
fn flat_map_body(
    cx: &mut Cx<'_>,
    helper: ObjectId,
    returning: bool,
    done: &mut bool,
) -> Result<Value, Failure> {
    let iterator = slot(cx, helper, ITERATOR);
    let callback = slot(cx, helper, CALLBACK);
    loop {
        if slot(cx, helper, INNER).is_undefined() {
            let method = step_method(cx, helper, returning)?;
            let (item, finished) = iterator_next(cx, iterator, method).map_err(Failure::NoClose)?;
            *done = finished;
            if finished || returning {
                return Ok(item);
            }
            let index = next_index(cx, helper);
            let mapped = crate::vm::call(cx, callback, Value::UNDEFINED, &[item, index])
                .map_err(Failure::Close)?;
            root(cx, mapped);
            if mapped.as_object().is_none() {
                return Err(Failure::Close(error::type_error(cx, "not an object")));
            }
            let key = WellKnown::key(cx.realm.symbols.iterator);
            let method = property::get(cx, mapped, key).map_err(Failure::Close)?;
            let inner = if method.is_undefined() || method.is_null() {
                mapped
            } else {
                root(cx, method);
                let inner = crate::vm::call(cx, method, mapped, &[]).map_err(Failure::Close)?;
                if inner.as_object().is_none() {
                    return Err(Failure::Close(error::type_error(cx, "not an object")));
                }
                inner
            };
            set_slot(cx, helper, INNER, inner);
        }
        let inner = slot(cx, helper, INNER);
        let inner_record = iter::Iter::Protocol {
            iterator: inner,
            next: Value::UNDEFINED,
        };
        let names = cx.realm.names;
        let key = Names::key(if returning { names.return_ } else { names.next });
        let stepped = property::get(cx, inner, key).and_then(|method| {
            if returning && (method.is_undefined() || method.is_null()) {
                return Ok((Value::UNDEFINED, true));
            }
            root(cx, method);
            iterator_next(cx, inner, method)
        });
        match stepped {
            Ok((item, false)) => {
                *done = false;
                return Ok(item);
            }
            Ok((_, true)) => {
                *done = false;
                let _ = iter::close(cx, &inner_record, Ok(()));
                set_slot(cx, helper, INNER, Value::UNDEFINED);
            }
            Err(throw) => {
                let throw = match iter::close(cx, &inner_record, Ok(())) {
                    Err(replaced) => replaced,
                    Ok(()) => throw,
                };
                set_slot(cx, helper, INNER, Value::UNDEFINED);
                return Err(Failure::Close(throw));
            }
        }
    }
}

/// QuickJS's `js_iterator_helper_next` for `next` and `return`.
fn helper_resume(cx: &mut Cx<'_>, site: CallSite, returning: bool) -> Result<Value, Throw> {
    let helper = this_tagged(cx, &site, InternalTag::IteratorHelper)?;
    if slot(cx, helper, EXECUTING) == Value::TRUE {
        return Err(error::type_error(cx, "cannot invoke a running iterator"));
    }
    if slot(cx, helper, DONE) == Value::TRUE {
        return iter::result_object(cx, Value::UNDEFINED, true);
    }
    set_slot(cx, helper, EXECUTING, Value::TRUE);
    let mut done = false;
    let result = match helper_body(cx, helper, returning, &mut done) {
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
    set_slot(cx, helper, DONE, Value::bool(returning || done));
    set_slot(cx, helper, EXECUTING, Value::FALSE);
    let value = result?;
    iter::result_object(cx, value, done)
}

/// `%IteratorHelperPrototype%.next`.
pub(super) fn helper_next(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    helper_resume(cx, site, false)
}

/// `%IteratorHelperPrototype%.return`.
pub(super) fn helper_return(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    helper_resume(cx, site, true)
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

// Slots of a concat record (QuickJS's `JSIteratorConcatData`): whether it
// is running, the index of the current item, the open inner iterator and its
// `next` (`undefined` until read), then (open method, iterable) per item.
const CONCAT_RUNNING: usize = 0;
const CONCAT_INDEX: usize = 1;
const CONCAT_INNER: usize = 2;
const CONCAT_ITEMS: usize = 4;

/// `Iterator.concat(...items)`.
pub(super) fn concat(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let mut slots = vec![
        Value::FALSE,
        Value::int32(0),
        Value::UNDEFINED,
        Value::UNDEFINED,
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
    let prototype = cx.realm.intrinsic(Intrinsic::IteratorConcatPrototype);
    cx.heap
        .alloc_internal(
            ObjectKind::Builtin,
            InternalTag::IteratorConcat,
            Some(prototype),
            0,
            &slots,
            Default::default(),
            0,
        )
        .map(Value::object)
        .map_err(|error| error::heap_error(cx, error))
}

fn concat_count(cx: &Cx<'_>, concat: ObjectId) -> usize {
    (cx.heap.internal(concat).slots.len() - CONCAT_ITEMS) / 2
}

/// QuickJS's `js_iterator_concat_next` loop: a failure leaves the state as
/// it was, so the next call retries the same step (opening the item again,
/// or reading `next` again).
fn concat_step(cx: &mut Cx<'_>, concat: ObjectId) -> Result<Option<Value>, Throw> {
    loop {
        let index = slot(cx, concat, CONCAT_INDEX).as_int32().unwrap() as usize;
        if index >= concat_count(cx, concat) {
            return Ok(None);
        }
        let at = CONCAT_ITEMS + index * 2;
        let mut iterator = slot(cx, concat, CONCAT_INNER);
        if iterator.is_undefined() {
            let method = slot(cx, concat, at);
            let iterable = slot(cx, concat, at + 1);
            iterator = crate::vm::call(cx, method, iterable, &[])?;
            if iterator.as_object().is_none() {
                return Err(error::type_error(cx, "not an object"));
            }
            set_slot(cx, concat, CONCAT_INNER, iterator);
        }
        let mut next = slot(cx, concat, CONCAT_INNER + 1);
        if next.is_undefined() {
            next = crate::vm::property::get(cx, iterator, Names::key(cx.realm.names.next))?;
            set_slot(cx, concat, CONCAT_INNER + 1, next);
        }
        let result = crate::vm::call_next(cx, next, iterator)?;
        let Some(result) = result.as_object() else {
            return Err(error::type_error(cx, "iterator must return an object"));
        };
        root(cx, Value::object(result));
        let names = cx.realm.names;
        let done = internal::get_property(cx, result, Names::key(names.done))?;
        if !ops::to_boolean(cx.heap, done) {
            return internal::get_property(cx, result, Names::key(names.value)).map(Some);
        }
        set_slot(cx, concat, CONCAT_INNER, Value::UNDEFINED);
        set_slot(cx, concat, CONCAT_INNER + 1, Value::UNDEFINED);
        set_slot(cx, concat, at, Value::UNDEFINED);
        set_slot(cx, concat, at + 1, Value::UNDEFINED);
        set_slot(cx, concat, CONCAT_INDEX, Value::int32(index as i32 + 1));
    }
}

pub(super) fn concat_next(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let concat = this_tagged(cx, &site, InternalTag::IteratorConcat)?;
    if slot(cx, concat, CONCAT_RUNNING) == Value::TRUE {
        return Err(error::type_error(cx, "already running"));
    }
    set_slot(cx, concat, CONCAT_RUNNING, Value::TRUE);
    let result = concat_step(cx, concat);
    set_slot(cx, concat, CONCAT_RUNNING, Value::FALSE);
    match result? {
        Some(value) => iter::result_object(cx, value, false),
        None => iter::result_object(cx, Value::UNDEFINED, true),
    }
}

/// QuickJS's `js_iterator_concat_return`: the open inner iterator's
/// `return` result itself (calling a missing `return` throws), and the
/// concat is finished whatever that call did.
pub(super) fn concat_return(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let concat = this_tagged(cx, &site, InternalTag::IteratorConcat)?;
    if slot(cx, concat, CONCAT_RUNNING) == Value::TRUE {
        return Err(error::type_error(cx, "already running"));
    }
    let iterator = slot(cx, concat, CONCAT_INNER);
    let mut result = Ok(Value::UNDEFINED);
    if !iterator.is_undefined() {
        set_slot(cx, concat, CONCAT_RUNNING, Value::TRUE);
        let key = Names::key(cx.realm.names.return_);
        let method = crate::vm::property::get(cx, iterator, key);
        let method = match method {
            Ok(method) => method,
            Err(throw) => {
                set_slot(cx, concat, CONCAT_RUNNING, Value::FALSE);
                return Err(throw);
            }
        };
        result = crate::vm::call(cx, method, iterator, &[]);
        set_slot(cx, concat, CONCAT_RUNNING, Value::FALSE);
        if let Ok(value) = result {
            root(cx, value);
        }
    }
    let count = concat_count(cx, concat);
    for index in 0..count {
        set_slot(cx, concat, CONCAT_ITEMS + index * 2, Value::UNDEFINED);
        set_slot(cx, concat, CONCAT_ITEMS + index * 2 + 1, Value::UNDEFINED);
    }
    set_slot(cx, concat, CONCAT_INDEX, Value::int32(count as i32));
    set_slot(cx, concat, CONCAT_INNER, Value::UNDEFINED);
    set_slot(cx, concat, CONCAT_INNER + 1, Value::UNDEFINED);
    result
}
