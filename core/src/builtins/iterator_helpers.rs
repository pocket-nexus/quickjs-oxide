//! `Iterator` and the iterator helpers (phase 4, section 6.1 row 8): the
//! abstract constructor, `Iterator.from` and `Iterator.concat`, and on
//! `%Iterator.prototype%` the lazy helpers (`map`, `filter`, `take`,
//! `drop`, `flatMap`) and the eager consumers (`reduce`, `toArray`,
//! `forEach`, `some`, `every`, `find`).
//!
//! A helper is a native generator: an internal record (tag
//! `IteratorHelper`) whose slots hold the underlying iterator record, the
//! helper's kind, its callback, the counter and its generator state, and
//! for `flatMap` the inner iterator record. `next` runs one step of the
//! closure the specification describes; `return` closes what is open.

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
    if !site.construct || site.new_target == Value::object(iterator) {
        return Err(error::type_error(cx, "abstract class not constructable"));
    }
    let prototype =
        internal::prototype_from_constructor(cx, site.new_target, Intrinsic::IteratorPrototype)?;
    cx.heap
        .alloc_object(ObjectKind::Ordinary, Some(prototype), 0)
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

/// `GetIteratorFlattenable(value, primitives)`: an iterator record, rooted.
fn flattenable(cx: &mut Cx<'_>, value: Value, strings: bool) -> Result<iter::Iter, Throw> {
    if value.as_object().is_none() && !(strings && value.as_string().is_some()) {
        return Err(error::type_error(cx, "not an object"));
    }
    let key = WellKnown::key(cx.realm.symbols.iterator);
    let method = property::get(cx, value, key)?;
    let iterator = if method.is_undefined() || method.is_null() {
        value
    } else {
        root(cx, method);
        crate::vm::call(cx, method, value, &[])?
    };
    if iterator.as_object().is_none() {
        return Err(error::type_error(cx, "iterator is not an object"));
    }
    iter::direct(cx, iterator)
}

/// `Iterator.from(value)`.
pub(super) fn from(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let value = site.arg(cx, 0);
    let iter::Iter::Protocol { iterator, next } = flattenable(cx, value, true)? else {
        unreachable!("a direct record");
    };
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
        _ => Err(error::type_error(
            cx,
            "not an iterator of the expected kind",
        )),
    }
}

pub(super) fn wrap_next(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let wrapper = this_tagged(cx, &site, InternalTag::WrapForValidIterator)?;
    let iterator = cx.heap.internal_slot(wrapper, 0);
    let next = cx.heap.internal_slot(wrapper, 1);
    crate::vm::call(cx, next, iterator, &[])
}

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
}

/// The generator states of a helper.
const SUSPENDED_START: i32 = 0;
const SUSPENDED_YIELD: i32 = 1;
const EXECUTING: i32 = 2;
const COMPLETED: i32 = 3;

// Slots of a helper record.
const ITERATOR: usize = 0;
const KIND: usize = 2;
const CALLBACK: usize = 3;
const COUNTER: usize = 4;
const STATE: usize = 5;
const REMAINING: usize = 6;
const INNER: usize = 7;
const INNER_NEXT: usize = 8;

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

fn create_helper(cx: &mut Cx<'_>, site: CallSite, kind: Kind) -> Result<Value, Throw> {
    let iterator = this_iterator(cx, &site)?;
    let argument = site.arg(cx, 0);
    let mut callback = Value::UNDEFINED;
    let mut remaining = 0.0;
    match kind {
        Kind::Take | Kind::Drop => {
            let limit = match ops::to_number(cx, argument) {
                Ok(limit) => limit,
                Err(throw) => return Err(close_unread(cx, iterator, throw)),
            };
            if limit.is_nan() {
                let throw = error::range_error(cx, "must be positive");
                return Err(close_unread(cx, iterator, throw));
            }
            let limit = internal::to_integer_or_infinity(cx, Value::number(limit))?;
            if limit < 0.0 {
                let throw = error::range_error(cx, "must be positive");
                return Err(close_unread(cx, iterator, throw));
            }
            remaining = limit;
        }
        _ => {
            if !ops::is_callable(cx.heap, argument) {
                let throw = error::type_error(cx, "not a function");
                return Err(close_unread(cx, iterator, throw));
            }
            callback = argument;
        }
    }
    let iter::Iter::Protocol { next, .. } = iter::direct(cx, iterator)? else {
        unreachable!("a direct record");
    };
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
                Value::int32(0),
                Value::int32(SUSPENDED_START),
                Value::number(remaining),
                Value::UNDEFINED,
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

fn record(cx: &Cx<'_>, helper: ObjectId, at: usize) -> iter::Iter {
    iter::Iter::Protocol {
        iterator: slot(cx, helper, at),
        next: slot(cx, helper, at + 1),
    }
}

/// Calls the helper's callback with the value and the counter, closing the
/// underlying iterator if it throws.
fn call_callback(cx: &mut Cx<'_>, helper: ObjectId, value: Value) -> Result<Value, Throw> {
    let callback = slot(cx, helper, CALLBACK);
    let counter = slot(cx, helper, COUNTER);
    let result = crate::vm::call(cx, callback, Value::UNDEFINED, &[value, counter]);
    let next = crate::vm::number::number(counter.as_f64().unwrap() + 1.0);
    set_slot(cx, helper, COUNTER, next);
    match result {
        Ok(result) => Ok(result),
        Err(throw) => {
            let underlying = record(cx, helper, ITERATOR);
            iter::close(cx, &underlying, Err(throw))?;
            unreachable!("close rethrows")
        }
    }
}

/// One step of a helper's closure: the next value, or `None` when done.
fn helper_step(cx: &mut Cx<'_>, helper: ObjectId) -> Result<Option<Value>, Throw> {
    let kind = slot(cx, helper, KIND).as_int32().unwrap();
    let mut underlying = record(cx, helper, ITERATOR);
    loop {
        if kind == Kind::FlatMap as i32 && !slot(cx, helper, INNER).is_undefined() {
            let mut inner = record(cx, helper, INNER);
            match iter::step(cx, &mut inner) {
                Ok(Some(value)) => return Ok(Some(value)),
                Ok(None) => {
                    set_slot(cx, helper, INNER, Value::UNDEFINED);
                    set_slot(cx, helper, INNER_NEXT, Value::UNDEFINED);
                    continue;
                }
                Err(throw) => {
                    iter::close(cx, &underlying, Err(throw))?;
                    unreachable!("close rethrows");
                }
            }
        }
        if kind == Kind::Take as i32 {
            let remaining = slot(cx, helper, REMAINING).as_f64().unwrap();
            if remaining == 0.0 {
                iter::close(cx, &underlying, Ok(()))?;
                return Ok(None);
            }
            if remaining != f64::INFINITY {
                set_slot(cx, helper, REMAINING, Value::number(remaining - 1.0));
            }
        }
        if kind == Kind::Drop as i32 {
            loop {
                let remaining = slot(cx, helper, REMAINING).as_f64().unwrap();
                if remaining <= 0.0 {
                    break;
                }
                if remaining != f64::INFINITY {
                    set_slot(cx, helper, REMAINING, Value::number(remaining - 1.0));
                }
                if iter::step(cx, &mut underlying)?.is_none() {
                    return Ok(None);
                }
            }
        }
        let Some(value) = iter::step(cx, &mut underlying)? else {
            return Ok(None);
        };
        let mark = cx.heap.roots().mark();
        root(cx, value);
        let result = match kind {
            k if k == Kind::Map as i32 => call_callback(cx, helper, value).map(Some),
            k if k == Kind::Filter as i32 => {
                let selected = call_callback(cx, helper, value)?;
                Ok(ops::to_boolean(cx.heap, selected).then_some(value))
            }
            k if k == Kind::FlatMap as i32 => {
                let mapped = call_callback(cx, helper, value)?;
                root(cx, mapped);
                match flattenable(cx, mapped, false) {
                    Ok(iter::Iter::Protocol { iterator, next }) => {
                        set_slot(cx, helper, INNER, iterator);
                        set_slot(cx, helper, INNER_NEXT, next);
                        Ok(None)
                    }
                    Ok(_) => unreachable!("a direct record"),
                    Err(throw) => {
                        iter::close(cx, &underlying, Err(throw))?;
                        unreachable!("close rethrows");
                    }
                }
            }
            _ => Ok(Some(value)),
        };
        cx.heap.roots().truncate(mark);
        match result? {
            Some(value) => return Ok(Some(value)),
            None => continue,
        }
    }
}

/// `%IteratorHelperPrototype%.next`.
pub(super) fn helper_next(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let helper = this_tagged(cx, &site, InternalTag::IteratorHelper)?;
    match slot(cx, helper, STATE).as_int32().unwrap() {
        EXECUTING => return Err(error::type_error(cx, "generator is already running")),
        COMPLETED => return iter::result_object(cx, Value::UNDEFINED, true),
        _ => {}
    }
    set_slot(cx, helper, STATE, Value::int32(EXECUTING));
    let result = helper_step(cx, helper);
    match result {
        Ok(Some(value)) => {
            set_slot(cx, helper, STATE, Value::int32(SUSPENDED_YIELD));
            iter::result_object(cx, value, false)
        }
        Ok(None) => {
            set_slot(cx, helper, STATE, Value::int32(COMPLETED));
            iter::result_object(cx, Value::UNDEFINED, true)
        }
        Err(throw) => {
            set_slot(cx, helper, STATE, Value::int32(COMPLETED));
            Err(throw)
        }
    }
}

/// `%IteratorHelperPrototype%.return`.
pub(super) fn helper_return(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let helper = this_tagged(cx, &site, InternalTag::IteratorHelper)?;
    match slot(cx, helper, STATE).as_int32().unwrap() {
        EXECUTING => return Err(error::type_error(cx, "generator is already running")),
        COMPLETED => return iter::result_object(cx, Value::UNDEFINED, true),
        _ => {}
    }
    // A suspended helper resumes to close what it holds, so it is running
    // meanwhile; one never started is completed first.
    let started = slot(cx, helper, STATE).as_int32() == Some(SUSPENDED_YIELD);
    set_slot(
        cx,
        helper,
        STATE,
        Value::int32(if started { EXECUTING } else { COMPLETED }),
    );
    let underlying = record(cx, helper, ITERATOR);
    let result = (|| {
        if !slot(cx, helper, INNER).is_undefined() {
            let inner = record(cx, helper, INNER);
            if let Err(throw) = iter::close(cx, &inner, Ok(())) {
                iter::close(cx, &underlying, Err(throw))?;
            }
        }
        iter::close(cx, &underlying, Ok(()))
    })();
    set_slot(cx, helper, STATE, Value::int32(COMPLETED));
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

// Slots of a concat record: the state, the index of the current item, the
// open inner record, then (open method, iterable) per item.
const CONCAT_STATE: usize = 0;
const CONCAT_INDEX: usize = 1;
const CONCAT_INNER: usize = 2;
const CONCAT_ITEMS: usize = 4;

/// `Iterator.concat(...items)`.
pub(super) fn concat(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let mut slots = vec![
        Value::int32(SUSPENDED_START),
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
        let Some(method) = internal::get_method(cx, item, key)? else {
            return Err(error::type_error(cx, "not iterable"));
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

fn concat_step(cx: &mut Cx<'_>, concat: ObjectId) -> Result<Option<Value>, Throw> {
    loop {
        if !slot(cx, concat, CONCAT_INNER).is_undefined() {
            let iterator = slot(cx, concat, CONCAT_INNER);
            let next = slot(cx, concat, CONCAT_INNER + 1);
            let result = crate::vm::call(cx, next, iterator, &[])?;
            let Some(result) = result.as_object() else {
                return Err(error::type_error(cx, "iterator result is not an object"));
            };
            root(cx, Value::object(result));
            let names = cx.realm.names;
            let done = internal::get_property(cx, result, Names::key(names.done))?;
            if ops::to_boolean(cx.heap, done) {
                set_slot(cx, concat, CONCAT_INNER, Value::UNDEFINED);
                set_slot(cx, concat, CONCAT_INNER + 1, Value::UNDEFINED);
                continue;
            }
            return internal::get_property(cx, result, Names::key(names.value)).map(Some);
        }
        let index = slot(cx, concat, CONCAT_INDEX).as_int32().unwrap() as usize;
        let at = CONCAT_ITEMS + index * 2;
        if at >= cx.heap.internal(concat).slots.len() {
            return Ok(None);
        }
        set_slot(cx, concat, CONCAT_INDEX, Value::int32(index as i32 + 1));
        let method = slot(cx, concat, at);
        let iterable = slot(cx, concat, at + 1);
        let iterator = crate::vm::call(cx, method, iterable, &[])?;
        if iterator.as_object().is_none() {
            return Err(error::type_error(cx, "iterator is not an object"));
        }
        let iter::Iter::Protocol { iterator, next } = iter::direct(cx, iterator)? else {
            unreachable!("a direct record");
        };
        set_slot(cx, concat, CONCAT_INNER, iterator);
        set_slot(cx, concat, CONCAT_INNER + 1, next);
    }
}

pub(super) fn concat_next(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let concat = this_tagged(cx, &site, InternalTag::IteratorConcat)?;
    match slot(cx, concat, CONCAT_STATE).as_int32().unwrap() {
        EXECUTING => return Err(error::type_error(cx, "generator is already running")),
        COMPLETED => return iter::result_object(cx, Value::UNDEFINED, true),
        _ => {}
    }
    set_slot(cx, concat, CONCAT_STATE, Value::int32(EXECUTING));
    match concat_step(cx, concat) {
        Ok(Some(value)) => {
            set_slot(cx, concat, CONCAT_STATE, Value::int32(SUSPENDED_YIELD));
            iter::result_object(cx, value, false)
        }
        Ok(None) => {
            set_slot(cx, concat, CONCAT_STATE, Value::int32(COMPLETED));
            iter::result_object(cx, Value::UNDEFINED, true)
        }
        Err(throw) => {
            set_slot(cx, concat, CONCAT_STATE, Value::int32(COMPLETED));
            Err(throw)
        }
    }
}

pub(super) fn concat_return(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let concat = this_tagged(cx, &site, InternalTag::IteratorConcat)?;
    match slot(cx, concat, CONCAT_STATE).as_int32().unwrap() {
        EXECUTING => return Err(error::type_error(cx, "generator is already running")),
        COMPLETED => return iter::result_object(cx, Value::UNDEFINED, true),
        _ => {}
    }
    let started = slot(cx, concat, CONCAT_STATE).as_int32() == Some(SUSPENDED_YIELD);
    set_slot(
        cx,
        concat,
        CONCAT_STATE,
        Value::int32(if started { EXECUTING } else { COMPLETED }),
    );
    let result = if slot(cx, concat, CONCAT_INNER).is_undefined() {
        Ok(())
    } else {
        let inner = record(cx, concat, CONCAT_INNER);
        iter::close(cx, &inner, Ok(()))
    };
    set_slot(cx, concat, CONCAT_STATE, Value::int32(COMPLETED));
    result?;
    iter::result_object(cx, Value::UNDEFINED, true)
}
