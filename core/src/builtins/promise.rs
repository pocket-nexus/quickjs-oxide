//! `Promise` (section 6.1 row 7): the constructor, `then`, `catch`,
//! `finally` and the static combinators, over `vm::jobs`. `Symbol.species`
//! is dropped (phase 0): `then` and `finally` use `%Promise%`.

use super::{
    Native, accessor, install_constructor, methods, native_closure, native_data, to_string_tag,
};
use crate::heap::HeapError;
use crate::runtime::{Intrinsic, Names, WellKnown};
use crate::shape::PropertyFlags;
use crate::value::{ObjectId, Value};
use crate::vm::jobs;
use crate::vm::native::CallSite;
use crate::vm::object as internal;
use crate::vm::{Cx, Throw, error, iter, ops};

pub(super) fn install(cx: &mut Cx<'_>) -> Result<(), HeapError> {
    let object_prototype = cx.realm.intrinsic(Intrinsic::ObjectPrototype);
    let prototype = super::plain_object(cx, Some(object_prototype), 8)?;
    cx.set_intrinsic(Intrinsic::PromisePrototype, prototype);
    let promise = install_constructor(cx, Native::PromiseConstructor, prototype)?;
    cx.set_intrinsic(Intrinsic::Promise, promise);
    methods(
        cx,
        promise,
        &[
            Native::PromiseAll,
            Native::PromiseAllSettled,
            Native::PromiseAny,
            Native::PromiseRace,
            Native::PromiseRejectStatic,
            Native::PromiseResolveStatic,
            Native::PromiseTry,
            Native::PromiseWithResolvers,
        ],
    )?;
    let species = WellKnown::key(cx.realm.symbols.species);
    accessor(
        cx,
        promise,
        species,
        "[Symbol.species]",
        Some(Native::SpeciesGetter),
        None,
    )?;
    methods(
        cx,
        prototype,
        &[
            Native::PromiseThen,
            Native::PromiseCatch,
            Native::PromiseFinally,
        ],
    )?;
    to_string_tag(cx, prototype, "Promise")
}

fn root(cx: &mut Cx<'_>, value: Value) {
    cx.heap.roots().push(value);
}

/// `get [Symbol.species]`: `this` (the accessor exists; nothing reads it).
pub(super) fn species_getter(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    Ok(site.this(cx))
}

/// `new Promise(executor)`.
pub(super) fn constructor(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    if !site.construct {
        return Err(error::type_error(cx, "Promise constructor requires 'new'"));
    }
    let executor = site.arg(cx, 0);
    if !ops::is_callable(cx.heap, executor) {
        return Err(error::type_error(cx, "Promise executor is not a function"));
    }
    let prototype =
        internal::prototype_from_constructor(cx, site.new_target, Intrinsic::PromisePrototype)?;
    let promise = jobs::new_promise(cx, prototype)?;
    root(cx, Value::object(promise));
    let (resolve, reject) = jobs::resolving_functions(cx, promise)?;
    if let Err(Throw(thrown)) = crate::vm::call(
        cx,
        executor,
        Value::UNDEFINED,
        &[Value::object(resolve), Value::object(reject)],
    ) {
        root(cx, thrown);
        crate::vm::call(cx, Value::object(reject), Value::UNDEFINED, &[thrown])?;
    }
    Ok(Value::object(promise))
}

pub(super) fn resolve_function(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    jobs::resolve_function(cx, site)
}

pub(super) fn reject_function(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    jobs::reject_function(cx, site)
}

pub(super) fn capability_executor(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    jobs::capability_executor(cx, site)
}

fn this_promise(cx: &mut Cx<'_>, site: &CallSite) -> Result<ObjectId, Throw> {
    let this = site.this(cx);
    if jobs::is_promise(cx, this) {
        return Ok(this.as_object().unwrap());
    }
    Err(error::type_error(cx, "not a promise"))
}

/// `Promise.prototype.then(onFulfilled, onRejected)`.
pub(super) fn then(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let promise = this_promise(cx, &site)?;
    let (on_fulfilled, on_rejected) = (site.arg(cx, 0), site.arg(cx, 1));
    let constructor = Value::object(cx.realm.intrinsic(Intrinsic::Promise));
    let capability = jobs::new_capability(cx, constructor)?;
    jobs::perform_then(
        cx,
        promise,
        on_fulfilled,
        on_rejected,
        Some(capability),
        Value::UNDEFINED,
    )?;
    Ok(capability[0])
}

/// `Promise.prototype.catch(onRejected)`: `this.then(undefined, onRejected)`.
pub(super) fn catch(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let this = site.this(cx);
    let on_rejected = site.arg(cx, 0);
    let key = Names::key(cx.realm.names.then);
    internal::invoke(cx, this, key, &[Value::UNDEFINED, on_rejected])
}

/// `Promise.prototype.finally(onFinally)`.
pub(super) fn finally(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let this = site.this(cx);
    if this.as_object().is_none() {
        return Err(error::type_error(cx, "not an object"));
    }
    let on_finally = site.arg(cx, 0);
    let (then_finally, catch_finally) = if ops::is_callable(cx.heap, on_finally) {
        let constructor = Value::object(cx.realm.intrinsic(Intrinsic::Promise));
        let data = native_data(cx, &[on_finally, constructor])?;
        root(cx, Value::object(data));
        let then_finally = native_closure(cx, Native::PromiseThenFinally, data)?;
        root(cx, Value::object(then_finally));
        let catch_finally = native_closure(cx, Native::PromiseCatchFinally, data)?;
        root(cx, Value::object(catch_finally));
        (Value::object(then_finally), Value::object(catch_finally))
    } else {
        (on_finally, on_finally)
    };
    let key = Names::key(cx.realm.names.then);
    internal::invoke(cx, this, key, &[then_finally, catch_finally])
}

/// The `finally` handlers: call `onFinally`, then pass the value through (or
/// rethrow the reason) once its result settles.
fn finally_handler(cx: &mut Cx<'_>, site: CallSite, rejected: bool) -> Result<Value, Throw> {
    let on_finally = site.data(cx, 0);
    let constructor = site.data(cx, 1);
    let value = site.arg(cx, 0);
    let result = crate::vm::call(cx, on_finally, Value::UNDEFINED, &[])?;
    root(cx, result);
    let promise = jobs::promise_resolve(cx, constructor, result)?;
    root(cx, promise);
    let data = native_data(cx, &[value])?;
    let native = if rejected {
        Native::PromiseThrower
    } else {
        Native::PromiseValueThunk
    };
    let thunk = native_closure(cx, native, data)?;
    let key = Names::key(cx.realm.names.then);
    internal::invoke(cx, promise, key, &[Value::object(thunk)])
}

pub(super) fn then_finally(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    finally_handler(cx, site, false)
}

pub(super) fn catch_finally(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    finally_handler(cx, site, true)
}

pub(super) fn value_thunk(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    Ok(site.data(cx, 0))
}

pub(super) fn thrower(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    Err(Throw(site.data(cx, 0)))
}

/// `Promise.resolve(x)`.
pub(super) fn resolve_static(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let constructor = site.this(cx);
    if constructor.as_object().is_none() {
        return Err(error::type_error(cx, "not an object"));
    }
    let value = site.arg(cx, 0);
    jobs::promise_resolve(cx, constructor, value)
}

/// `Promise.reject(r)`.
pub(super) fn reject_static(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let constructor = site.this(cx);
    let reason = site.arg(cx, 0);
    let [promise, _, reject] = jobs::new_capability(cx, constructor)?;
    crate::vm::call(cx, reject, Value::UNDEFINED, &[reason])?;
    Ok(promise)
}

/// `Promise.withResolvers()`.
pub(super) fn with_resolvers(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let constructor = site.this(cx);
    let [promise, resolve, reject] = jobs::new_capability(cx, constructor)?;
    let object = super::new_plain(cx, 3)?;
    root(cx, Value::object(object));
    let names = cx.realm.names;
    for (name, value) in [
        (names.promise, promise),
        (names.resolve, resolve),
        (names.reject, reject),
    ] {
        cx.heap
            .add_property(object, Names::key(name), PropertyFlags::DEFAULT, value)
            .map_err(|error| error::heap_error(cx, error))?;
    }
    Ok(Value::object(object))
}

/// `Promise.try(f, ...args)`.
pub(super) fn try_(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let constructor = site.this(cx);
    if constructor.as_object().is_none() {
        return Err(error::type_error(cx, "not an object"));
    }
    let [promise, resolve, reject] = jobs::new_capability(cx, constructor)?;
    let function = site.arg(cx, 0);
    let args: Vec<Value> = (1..site.argc).map(|index| site.arg(cx, index)).collect();
    match crate::vm::call(cx, function, Value::UNDEFINED, &args) {
        Ok(value) => {
            root(cx, value);
            crate::vm::call(cx, resolve, Value::UNDEFINED, &[value])?;
        }
        Err(Throw(reason)) => {
            root(cx, reason);
            crate::vm::call(cx, reject, Value::UNDEFINED, &[reason])?;
        }
    }
    Ok(promise)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Combinator {
    All,
    AllSettled,
    Any,
    Race,
}

/// `Promise.all`, `allSettled`, `any` and `race`: `PerformPromiseX` over the
/// iterable, with the capability rejected on an abrupt completion.
fn combinator(cx: &mut Cx<'_>, site: CallSite, kind: Combinator) -> Result<Value, Throw> {
    let constructor = site.this(cx);
    let capability = jobs::new_capability(cx, constructor)?;
    let iterable = site.arg(cx, 0);
    let outcome = (|| {
        let resolve = internal::get_method(cx, constructor, Names::key(cx.realm.names.resolve))?
            .ok_or_else(|| error::type_error(cx, "Promise.resolve is not a function"))?;
        root(cx, resolve);
        let mut record = iter::get_iterator(cx, iterable)?;
        let result = perform(cx, kind, constructor, resolve, capability, &mut record);
        match result {
            Ok(value) => Ok(value),
            Err((throw, from_iterator)) => {
                if !from_iterator {
                    iter::close(cx, &record, Err(throw))?;
                }
                Err(throw)
            }
        }
    })();
    match outcome {
        Ok(value) => Ok(value),
        Err(Throw(reason)) => {
            root(cx, reason);
            crate::vm::call(cx, capability[2], Value::UNDEFINED, &[reason])?;
            Ok(capability[0])
        }
    }
}

/// The loop of a combinator. An error carries whether it came from the
/// iterator itself or after it finished (it then is not closed).
fn perform(
    cx: &mut Cx<'_>,
    kind: Combinator,
    constructor: Value,
    resolve: Value,
    capability: [Value; 3],
    record: &mut iter::Iter,
) -> Result<Value, (Throw, bool)> {
    let values = internal::array_create(cx, 0).map_err(|throw| (throw, false))?;
    root(cx, Value::object(values));
    // The shared state of the element closures: the values array, the
    // remaining count, the capability.
    let shared = native_data(
        cx,
        &[
            Value::object(values),
            Value::int32(1),
            capability[0],
            capability[1],
            capability[2],
        ],
    )
    .map_err(|throw| (throw, false))?;
    root(cx, Value::object(shared));
    let mut index = 0u32;
    loop {
        let next = match iter::step(cx, record) {
            Ok(next) => next,
            Err(throw) => return Err((throw, true)),
        };
        let Some(value) = next else {
            break;
        };
        root(cx, value);
        let step = (|| {
            if kind != Combinator::Race {
                cx.heap
                    .set_element(values, index, Value::UNDEFINED)
                    .map_err(|error| error::heap_error(cx, error))?;
            }
            let next_promise = crate::vm::call(cx, resolve, constructor, &[value])?;
            root(cx, next_promise);
            let (on_fulfilled, on_rejected) = match kind {
                Combinator::Race => (capability[1], capability[2]),
                Combinator::All => (
                    element_closure(cx, Native::PromiseAllResolveElement, shared, index)?,
                    capability[2],
                ),
                Combinator::AllSettled => {
                    let fulfilled = element_closure(
                        cx,
                        Native::PromiseAllSettledResolveElement,
                        shared,
                        index,
                    )?;
                    root(cx, fulfilled);
                    // Both halves share one already-called flag: the second
                    // closure reuses the first's element record.
                    let data = cx
                        .heap
                        .function_scope(fulfilled.as_object().unwrap())
                        .expect("element data")
                        .object();
                    let rejected =
                        native_closure(cx, Native::PromiseAllSettledRejectElement, data)?;
                    (fulfilled, Value::object(rejected))
                }
                Combinator::Any => (
                    capability[1],
                    element_closure(cx, Native::PromiseAnyRejectElement, shared, index)?,
                ),
            };
            root(cx, on_fulfilled);
            root(cx, on_rejected);
            if kind != Combinator::Race {
                let remaining = cx.heap.internal_slot(shared, 1).as_int32().unwrap_or(0);
                cx.heap
                    .set_internal_slot(shared, 1, Value::int32(remaining + 1));
            }
            let key = Names::key(cx.realm.names.then);
            internal::invoke(cx, next_promise, key, &[on_fulfilled, on_rejected])?;
            Ok(())
        })();
        step.map_err(|throw| (throw, false))?;
        index += 1;
    }
    if kind == Combinator::Race {
        return Ok(capability[0]);
    }
    let remaining = cx.heap.internal_slot(shared, 1).as_int32().unwrap_or(0) - 1;
    cx.heap
        .set_internal_slot(shared, 1, Value::int32(remaining));
    if remaining == 0 {
        let finish = (|| {
            if kind == Combinator::Any {
                let error = aggregate_error(cx, values)?;
                crate::vm::call(cx, capability[2], Value::UNDEFINED, &[error])?;
            } else {
                crate::vm::call(
                    cx,
                    capability[1],
                    Value::UNDEFINED,
                    &[Value::object(values)],
                )?;
            }
            Ok(())
        })();
        // The iterator is done: it is not closed.
        finish.map_err(|throw| (throw, true))?;
    }
    Ok(capability[0])
}

/// An `AggregateError` whose `errors` are the array's elements.
fn aggregate_error(cx: &mut Cx<'_>, errors: ObjectId) -> Result<Value, Throw> {
    let error = crate::vm::error::new_error(cx, crate::ErrorKind::Aggregate, Value::UNDEFINED)
        .map_err(|error| error::heap_error(cx, error))?;
    root(cx, error);
    let key = Names::key(cx.realm.names.errors);
    cx.heap
        .add_property(
            error.as_object().unwrap(),
            key,
            PropertyFlags::data(true, false, true),
            Value::object(errors),
        )
        .map_err(|error| error::heap_error(cx, error))?;
    Ok(error)
}

/// An element closure: its own record holds the shared state, its index and
/// whether it has been called.
fn element_closure(
    cx: &mut Cx<'_>,
    native: Native,
    shared: ObjectId,
    index: u32,
) -> Result<Value, Throw> {
    let data = native_data(
        cx,
        &[
            Value::object(shared),
            Value::int32(index as i32),
            Value::FALSE,
        ],
    )?;
    native_closure(cx, native, data).map(Value::object)
}

/// The common part of the element functions: once only, store the element,
/// and settle when the last one arrives.
fn element(cx: &mut Cx<'_>, site: CallSite, element: Value, any: bool) -> Result<Value, Throw> {
    if site.data(cx, 2) == Value::TRUE {
        return Ok(Value::UNDEFINED);
    }
    site.set_data(cx, 2, Value::TRUE);
    let shared = site.data(cx, 0).as_object().expect("shared state");
    let index = site.data(cx, 1).as_int32().unwrap_or(0) as u32;
    let values = cx
        .heap
        .internal_slot(shared, 0)
        .as_object()
        .expect("values");
    cx.heap
        .set_element(values, index, element)
        .map_err(|error| error::heap_error(cx, error))?;
    let remaining = cx.heap.internal_slot(shared, 1).as_int32().unwrap_or(0) - 1;
    cx.heap
        .set_internal_slot(shared, 1, Value::int32(remaining));
    if remaining == 0 {
        if any {
            let reject = cx.heap.internal_slot(shared, 4);
            let error = aggregate_error(cx, values)?;
            return crate::vm::call(cx, reject, Value::UNDEFINED, &[error]);
        }
        let resolve = cx.heap.internal_slot(shared, 3);
        return crate::vm::call(cx, resolve, Value::UNDEFINED, &[Value::object(values)]);
    }
    Ok(Value::UNDEFINED)
}

pub(super) fn all_resolve_element(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let value = site.arg(cx, 0);
    element(cx, site, value, false)
}

/// `{ status, value }` or `{ status, reason }`.
fn settled_record(cx: &mut Cx<'_>, rejected: bool, value: Value) -> Result<Value, Throw> {
    let object = super::new_plain(cx, 2)?;
    root(cx, Value::object(object));
    let names = cx.realm.names;
    let (status, field) = if rejected {
        (names.rejected, names.reason)
    } else {
        (names.fulfilled, names.value)
    };
    let heap = &mut *cx.heap;
    heap.add_property(
        object,
        Names::key(names.status),
        PropertyFlags::DEFAULT,
        Value::string(status),
    )
    .and_then(|()| heap.add_property(object, Names::key(field), PropertyFlags::DEFAULT, value))
    .map_err(|error| error::heap_error(cx, error))?;
    Ok(Value::object(object))
}

pub(super) fn all_settled_resolve_element(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let value = site.arg(cx, 0);
    if site.data(cx, 2) == Value::TRUE {
        return Ok(Value::UNDEFINED);
    }
    let record = settled_record(cx, false, value)?;
    element(cx, site, record, false)
}

pub(super) fn all_settled_reject_element(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let value = site.arg(cx, 0);
    if site.data(cx, 2) == Value::TRUE {
        return Ok(Value::UNDEFINED);
    }
    let record = settled_record(cx, true, value)?;
    element(cx, site, record, false)
}

pub(super) fn any_reject_element(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let value = site.arg(cx, 0);
    element(cx, site, value, true)
}

pub(super) fn all(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    combinator(cx, site, Combinator::All)
}

pub(super) fn all_settled(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    combinator(cx, site, Combinator::AllSettled)
}

pub(super) fn any(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    combinator(cx, site, Combinator::Any)
}

pub(super) fn race(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    combinator(cx, site, Combinator::Race)
}
