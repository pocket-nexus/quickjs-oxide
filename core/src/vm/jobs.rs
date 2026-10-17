//! The job queue and the promise machinery (phase 4, section 5.4).
//!
//! A promise is a [`InternalTag::Promise`] object whose internal slots are
//! its state, its result, whether it is handled, and then its pending
//! `then` records. A `then` record ([`InternalTag::PromiseReaction`]) holds
//! the capability (promise, resolve, reject; `undefined` when there is
//! none), the fulfill and reject handlers, and the suspended async frame an
//! `await` resumes. Nothing drains the queue implicitly: the host calls
//! [`run_jobs`] at its boundary.

use super::error;
use super::object;
use super::ops;
use super::{Cx, Throw};
use crate::builtins::{Native, native_closure, native_data};
use crate::heap::object::{InternalTag, ObjectKind};
use crate::runtime::{Intrinsic, Names};
use crate::value::{ObjectId, Value};

/// One pending job. Every value it names is a root until it runs.
#[derive(Clone, Debug)]
pub(crate) enum Job {
    /// `PromiseReactionJob`: a `then` record and the settled value.
    Reaction {
        record: ObjectId,
        argument: Value,
        rejected: bool,
    },
    /// `PromiseResolveThenableJob`.
    Thenable {
        promise: ObjectId,
        thenable: Value,
        then: Value,
    },
    /// A plain call (finalization callbacks).
    Call {
        function: Value,
        this: Value,
        args: Vec<Value>,
    },
}

impl Job {
    pub(crate) fn trace(&self, stack: &mut Vec<Value>) {
        match self {
            Job::Reaction {
                record, argument, ..
            } => {
                stack.push(Value::object(*record));
                stack.push(*argument);
            }
            Job::Thenable {
                promise,
                thenable,
                then,
            } => {
                stack.push(Value::object(*promise));
                stack.push(*thenable);
                stack.push(*then);
            }
            Job::Call {
                function,
                this,
                args,
            } => {
                stack.push(*function);
                stack.push(*this);
                stack.extend_from_slice(args);
            }
        }
    }
}

/// The states in a promise's slot 0.
pub(crate) const PENDING: i32 = 0;
pub(crate) const FULFILLED: i32 = 1;
pub(crate) const REJECTED: i32 = 2;

const STATE: usize = 0;
const RESULT: usize = 1;
pub(crate) const HANDLED: usize = 2;
const REACTIONS: usize = 3;

/// A `then` record's slots.
const CAPABILITY_PROMISE: usize = 0;
const CAPABILITY_RESOLVE: usize = 1;
const CAPABILITY_REJECT: usize = 2;
const ON_FULFILLED: usize = 3;
const ON_REJECTED: usize = 4;
const ASYNC: usize = 5;

fn root(cx: &mut Cx<'_>, value: Value) {
    cx.heap.roots().push(value);
}

/// Whether `value` is a promise.
pub(crate) fn is_promise(cx: &Cx<'_>, value: Value) -> bool {
    value
        .as_object()
        .is_some_and(|object| cx.heap.internal_tag(object) == InternalTag::Promise)
}

pub(crate) fn state(cx: &Cx<'_>, promise: ObjectId) -> i32 {
    cx.heap
        .internal_slot(promise, STATE)
        .as_int32()
        .unwrap_or(PENDING)
}

pub(crate) fn result(cx: &Cx<'_>, promise: ObjectId) -> Value {
    cx.heap.internal_slot(promise, RESULT)
}

/// A pending promise with `prototype`.
pub(crate) fn new_promise(cx: &mut Cx<'_>, prototype: ObjectId) -> Result<ObjectId, Throw> {
    cx.heap
        .alloc_internal(
            ObjectKind::Builtin,
            InternalTag::Promise,
            Some(prototype),
            0,
            &[Value::int32(PENDING), Value::UNDEFINED, Value::FALSE],
            Default::default(),
            0,
        )
        .map_err(|error| error::heap_error(cx, error))
}

/// `CreateResolvingFunctions(promise)`: two closures over one shared record
/// (the promise, then whether either has run).
pub(crate) fn resolving_functions(
    cx: &mut Cx<'_>,
    promise: ObjectId,
) -> Result<(ObjectId, ObjectId), Throw> {
    let data = native_data(cx, &[Value::object(promise), Value::FALSE])?;
    root(cx, Value::object(data));
    let resolve = native_closure(cx, Native::PromiseResolveFunction, data)?;
    root(cx, Value::object(resolve));
    let reject = native_closure(cx, Native::PromiseRejectFunction, data)?;
    root(cx, Value::object(reject));
    Ok((resolve, reject))
}

/// The `resolve` half of the resolving functions: once only.
pub(crate) fn resolve_function(
    cx: &mut Cx<'_>,
    site: super::native::CallSite,
) -> Result<Value, Throw> {
    if site.data(cx, 1) == Value::TRUE {
        return Ok(Value::UNDEFINED);
    }
    site.set_data(cx, 1, Value::TRUE);
    let promise = site.data(cx, 0).as_object().expect("a promise");
    let resolution = site.arg(cx, 0);
    resolve_promise(cx, promise, resolution)?;
    Ok(Value::UNDEFINED)
}

/// The `reject` half of the resolving functions: once only.
pub(crate) fn reject_function(
    cx: &mut Cx<'_>,
    site: super::native::CallSite,
) -> Result<Value, Throw> {
    if site.data(cx, 1) == Value::TRUE {
        return Ok(Value::UNDEFINED);
    }
    site.set_data(cx, 1, Value::TRUE);
    let promise = site.data(cx, 0).as_object().expect("a promise");
    let reason = site.arg(cx, 0);
    reject_promise(cx, promise, reason);
    Ok(Value::UNDEFINED)
}

/// The steps of a promise resolve function after the already-resolved
/// check: self-resolution rejects, a thenable is followed by a job, anything
/// else fulfills.
pub(crate) fn resolve_promise(
    cx: &mut Cx<'_>,
    promise: ObjectId,
    resolution: Value,
) -> Result<(), Throw> {
    if resolution == Value::object(promise) {
        let error = error::type_error(cx, "promise self resolution");
        reject_promise(cx, promise, error.0);
        return Ok(());
    }
    let Some(object) = resolution.as_object() else {
        fulfill_promise(cx, promise, resolution);
        return Ok(());
    };
    let mark = cx.heap.roots().mark();
    root(cx, resolution);
    let then = object::get_property(cx, object, Names::key(cx.realm.names.then));
    let result = match then {
        Err(Throw(thrown)) => {
            reject_promise(cx, promise, thrown);
            Ok(())
        }
        Ok(then) if ops::is_callable(cx.heap, then) => {
            cx.heap.jobs.push_back(Job::Thenable {
                promise,
                thenable: resolution,
                then,
            });
            Ok(())
        }
        Ok(_) => {
            fulfill_promise(cx, promise, resolution);
            Ok(())
        }
    };
    cx.heap.roots().truncate(mark);
    result
}

/// `FulfillPromise`.
pub(crate) fn fulfill_promise(cx: &mut Cx<'_>, promise: ObjectId, value: Value) {
    settle(cx, promise, FULFILLED, value);
}

/// `RejectPromise`.
pub(crate) fn reject_promise(cx: &mut Cx<'_>, promise: ObjectId, reason: Value) {
    settle(cx, promise, REJECTED, reason);
}

fn settle(cx: &mut Cx<'_>, promise: ObjectId, state: i32, value: Value) {
    let internal = cx.heap.internal_mut(promise);
    if internal.slots[STATE].as_int32() != Some(PENDING) {
        return;
    }
    let reactions: Vec<Value> = internal.slots.drain(REACTIONS..).collect();
    internal.slots[STATE] = Value::int32(state);
    internal.slots[RESULT] = value;
    let rejected = state == REJECTED;
    if rejected && internal.slots[HANDLED] != Value::TRUE {
        track_rejection(cx, promise, crate::host::RejectionEvent::Rejected);
    }
    for record in reactions {
        cx.heap.jobs.push_back(Job::Reaction {
            record: record.as_object().expect("a then record"),
            argument: value,
            rejected,
        });
    }
}

/// `HostPromiseRejectionTracker`: tells the host and, while the embedder
/// tracks rejections, keeps the unhandled ones.
#[cold]
fn track_rejection(cx: &mut Cx<'_>, promise: ObjectId, event: crate::host::RejectionEvent) {
    let value = Value::object(promise);
    if cx.host.track_rejections {
        match event {
            crate::host::RejectionEvent::Rejected => cx.heap.rejections.push(value),
            crate::host::RejectionEvent::Handled => cx.heap.rejections.retain(|&p| p != value),
        }
    }
    cx.host.hooks.promise_rejection(event, value);
}

/// `PerformPromiseThen(promise, onFulfilled, onRejected, capability)`; the
/// handlers are dropped unless callable. `awaiting` is the async frame an
/// `await` resumes instead of the handlers.
pub(crate) fn perform_then(
    cx: &mut Cx<'_>,
    promise: ObjectId,
    on_fulfilled: Value,
    on_rejected: Value,
    capability: Option<[Value; 3]>,
    awaiting: Value,
) -> Result<(), Throw> {
    let on_fulfilled = if ops::is_callable(cx.heap, on_fulfilled) {
        on_fulfilled
    } else {
        Value::UNDEFINED
    };
    let on_rejected = if ops::is_callable(cx.heap, on_rejected) {
        on_rejected
    } else {
        Value::UNDEFINED
    };
    let [cap_promise, cap_resolve, cap_reject] = capability.unwrap_or([Value::UNDEFINED; 3]);
    let record = cx
        .heap
        .alloc_internal(
            ObjectKind::Builtin,
            InternalTag::PromiseReaction,
            None,
            0,
            &[
                cap_promise,
                cap_resolve,
                cap_reject,
                on_fulfilled,
                on_rejected,
                awaiting,
            ],
            Default::default(),
            0,
        )
        .map_err(|error| error::heap_error(cx, error))?;
    match state(cx, promise) {
        PENDING => {
            cx.heap
                .push_internal_slot(promise, Value::object(record))
                .map_err(|error| error::heap_error(cx, error))?;
        }
        state => {
            if state == REJECTED && cx.heap.internal_slot(promise, HANDLED) != Value::TRUE {
                track_rejection(cx, promise, crate::host::RejectionEvent::Handled);
            }
            let argument = result(cx, promise);
            cx.heap.jobs.push_back(Job::Reaction {
                record,
                argument,
                rejected: state == REJECTED,
            });
        }
    }
    cx.heap.set_internal_slot(promise, HANDLED, Value::TRUE);
    Ok(())
}

/// `NewPromiseCapability(C)`: `[promise, resolve, reject]`, rooted. The
/// intrinsic constructor is built directly.
pub(crate) fn new_capability(cx: &mut Cx<'_>, constructor: Value) -> Result<[Value; 3], Throw> {
    let intrinsic = cx.realm.intrinsic(Intrinsic::Promise);
    if constructor == Value::object(intrinsic) {
        let prototype = cx.realm.intrinsic(Intrinsic::PromisePrototype);
        let promise = new_promise(cx, prototype)?;
        root(cx, Value::object(promise));
        let (resolve, reject) = resolving_functions(cx, promise)?;
        return Ok([
            Value::object(promise),
            Value::object(resolve),
            Value::object(reject),
        ]);
    }
    if !object::is_constructor(cx, constructor) {
        // QuickJS's callers require an object first.
        if constructor.as_object().is_none() {
            return Err(error::type_error(cx, "not an object"));
        }
        return Err(error::not_a_constructor(cx, constructor));
    }
    let data = native_data(cx, &[Value::UNDEFINED, Value::UNDEFINED])?;
    root(cx, Value::object(data));
    let executor = native_closure(cx, Native::PromiseCapabilityExecutor, data)?;
    root(cx, Value::object(executor));
    let promise = super::construct(cx, constructor, &[Value::object(executor)], constructor)?;
    root(cx, promise);
    let resolve = cx.heap.internal_slot(data, 0);
    let reject = cx.heap.internal_slot(data, 1);
    if !ops::is_callable(cx.heap, resolve) || !ops::is_callable(cx.heap, reject) {
        return Err(error::type_error(cx, "not a function"));
    }
    Ok([promise, resolve, reject])
}

/// `GetCapabilitiesExecutor`'s function.
pub(crate) fn capability_executor(
    cx: &mut Cx<'_>,
    site: super::native::CallSite,
) -> Result<Value, Throw> {
    if !site.data(cx, 0).is_undefined() || !site.data(cx, 1).is_undefined() {
        return Err(error::type_error(cx, "resolving function already set"));
    }
    let (resolve, reject) = (site.arg(cx, 0), site.arg(cx, 1));
    site.set_data(cx, 0, resolve);
    site.set_data(cx, 1, reject);
    Ok(Value::UNDEFINED)
}

/// `PromiseResolve(C, x)`.
pub(crate) fn promise_resolve(
    cx: &mut Cx<'_>,
    constructor: Value,
    value: Value,
) -> Result<Value, Throw> {
    if is_promise(cx, value) {
        let value_object = value.as_object().unwrap();
        let value_constructor =
            object::get_property(cx, value_object, Names::key(cx.realm.names.constructor))?;
        if value_constructor == constructor {
            return Ok(value);
        }
    }
    let [promise, resolve, _] = new_capability(cx, constructor)?;
    super::call(cx, resolve, Value::UNDEFINED, &[value])?;
    Ok(promise)
}

/// Runs one job.
fn run_job(cx: &mut Cx<'_>, job: Job) -> Result<(), Throw> {
    let mark = cx.heap.roots().mark();
    let mut values = Vec::new();
    job.trace(&mut values);
    for value in values {
        root(cx, value);
    }
    let result = match job {
        Job::Reaction {
            record,
            argument,
            rejected,
        } => reaction_job(cx, record, argument, rejected),
        Job::Thenable {
            promise,
            thenable,
            then,
        } => (|| {
            let (resolve, reject) = resolving_functions(cx, promise)?;
            if let Err(Throw(thrown)) = super::call(
                cx,
                then,
                thenable,
                &[Value::object(resolve), Value::object(reject)],
            ) {
                super::call(cx, Value::object(reject), Value::UNDEFINED, &[thrown])?;
            }
            Ok(())
        })(),
        Job::Call {
            function,
            this,
            args,
        } => super::call(cx, function, this, &args).map(|_| ()),
    };
    cx.heap.roots().truncate(mark);
    result
}

/// `PromiseReactionJob`.
fn reaction_job(
    cx: &mut Cx<'_>,
    record: ObjectId,
    argument: Value,
    rejected: bool,
) -> Result<(), Throw> {
    let slots = cx.heap.internal(record).slots.clone();
    if let Some(awaiting) = slots[ASYNC].as_object() {
        return super::suspend::resume_await(cx, awaiting, argument, rejected);
    }
    let handler = if rejected {
        slots[ON_REJECTED]
    } else {
        slots[ON_FULFILLED]
    };
    let outcome = if handler.is_undefined() {
        if rejected {
            Err(Throw(argument))
        } else {
            Ok(argument)
        }
    } else {
        super::call(cx, handler, Value::UNDEFINED, &[argument])
    };
    if slots[CAPABILITY_PROMISE].is_undefined() {
        return Ok(());
    }
    match outcome {
        Ok(value) => {
            root(cx, value);
            super::call(cx, slots[CAPABILITY_RESOLVE], Value::UNDEFINED, &[value])?;
        }
        Err(Throw(reason)) => {
            root(cx, reason);
            super::call(cx, slots[CAPABILITY_REJECT], Value::UNDEFINED, &[reason])?;
        }
    }
    Ok(())
}

/// Runs the oldest pending job, then `ClearKeptObjects`; `false` when the
/// queue was empty.
pub(crate) fn run_one_job(cx: &mut Cx<'_>) -> Result<bool, Throw> {
    let Some(job) = cx.heap.jobs.pop_front() else {
        return Ok(false);
    };
    let result = run_job(cx, job);
    cx.heap.kept.clear();
    result.map(|()| true)
}

/// Drains the job queue. A job that throws stops the drain and reports the
/// throw (the queue keeps the jobs after it).
pub(crate) fn run_jobs(cx: &mut Cx<'_>) -> Result<(), Throw> {
    // `ClearKeptObjects` after the script and after each job.
    cx.heap.kept.clear();
    while let Some(job) = cx.heap.jobs.pop_front() {
        let result = run_job(cx, job);
        cx.heap.kept.clear();
        result?;
    }
    Ok(())
}
