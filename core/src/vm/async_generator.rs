//! Async generators: the request queue and the `AsyncGenerator*` abstract
//! operations over the suspension of `vm::suspend`.
//!
//! The record's second slot is the queue: an object whose internal slots
//! hold five values per request (kind, value, and the capability's promise,
//! resolve and reject). The compiler already awaits a yielded value, a
//! returned value and the value of a `return` resumption in bytecode, so the
//! driver only completes requests and drains the queue.

use super::error;
use super::iter;
use super::jobs;
use super::native::CallSite;
use super::suspend::{self, COMPLETED, EXECUTING, Resumption, START, YIELD};
use super::{Cx, Throw};
use crate::heap::object::InternalTag;
use crate::runtime::Intrinsic;
use crate::value::{ObjectId, Value};

const NEXT: i32 = 0;
const RETURN: i32 = 1;
const THROW: i32 = 2;
const REQUEST: usize = 5;

/// The spec's `draining-queue`: the body has finished, and the queue's
/// requests are being settled or a `return` request's value awaited
/// (`AsyncGeneratorAwaitReturn`). Requests made meanwhile only queue.
const DRAINING: u8 = 6;

fn root(cx: &mut Cx<'_>, value: Value) {
    cx.heap.roots().push(value);
}

fn queue(cx: &mut Cx<'_>, generator: ObjectId) -> Result<ObjectId, Throw> {
    if let Some(queue) = cx.heap.internal_slot(generator, 1).as_object() {
        return Ok(queue);
    }
    let queue = crate::builtins::native_data(cx, &[])?;
    cx.heap
        .set_internal_slot(generator, 1, Value::object(queue));
    Ok(queue)
}

fn queue_len(cx: &Cx<'_>, generator: ObjectId) -> usize {
    match cx.heap.internal_slot(generator, 1).as_object() {
        Some(queue) => cx.heap.internal(queue).slots.len() / REQUEST,
        None => 0,
    }
}

/// The first request: kind and value.
fn first(cx: &Cx<'_>, generator: ObjectId) -> (i32, Value) {
    let queue = cx
        .heap
        .internal_slot(generator, 1)
        .as_object()
        .expect("a queue");
    let slots = &cx.heap.internal(queue).slots;
    (slots[0].as_int32().unwrap_or(NEXT), slots[1])
}

/// `AsyncGeneratorCompleteStep`: settles the first request's promise with
/// `{ value, done }` or rejects it.
fn complete_step(
    cx: &mut Cx<'_>,
    generator: ObjectId,
    outcome: Result<Value, Throw>,
    done: bool,
) -> Result<(), Throw> {
    let queue = cx
        .heap
        .internal_slot(generator, 1)
        .as_object()
        .expect("a queue");
    let request: Vec<Value> = cx.heap.internal_mut(queue).slots.drain(..REQUEST).collect();
    for &value in &request {
        root(cx, value);
    }
    match outcome {
        Ok(value) => {
            let result = iter::result_object(cx, value, done)?;
            root(cx, result);
            super::call(cx, request[3], Value::UNDEFINED, &[result])?;
        }
        Err(Throw(reason)) if super::error::catchable(cx) => {
            root(cx, reason);
            super::call(cx, request[4], Value::UNDEFINED, &[reason])?;
        }
        Err(throw) => return Err(throw),
    }
    Ok(())
}

/// `AsyncGeneratorResume` with the first request's completion.
fn resume_with_request(cx: &mut Cx<'_>, generator: ObjectId) -> Result<(), Throw> {
    let (kind, value) = first(cx, generator);
    let resumption = match (suspend::state(cx, generator), kind) {
        (START, _) => Resumption::Start,
        (_, RETURN) => Resumption::Return(value),
        (_, THROW) => Resumption::Throw(value),
        _ => Resumption::Next(value),
    };
    let result = suspend::resume(cx, generator, resumption);
    after_run(cx, generator, result)
}

/// What follows a run of the body: at a `yield`, the first request
/// completes while the generator still counts as executing (requests made
/// meanwhile only queue) and the next one (if any) resumes it at once
/// (AsyncGeneratorYield); on completion the first request completes and
/// the queue drains; while it awaits, nothing.
fn after_run(
    cx: &mut Cx<'_>,
    generator: ObjectId,
    result: Result<Value, Throw>,
) -> Result<(), Throw> {
    match suspend::state(cx, generator) {
        YIELD => {
            let value = result?;
            root(cx, value);
            suspend::set_state(cx, generator, EXECUTING);
            let settled = complete_step(cx, generator, Ok(value), false);
            suspend::set_state(cx, generator, YIELD);
            settled?;
            if queue_len(cx, generator) == 0 {
                return Ok(());
            }
            resume_with_request(cx, generator)
        }
        EXECUTING => {
            suspend::complete(cx, generator);
            suspend::set_state(cx, generator, DRAINING);
            complete_step(cx, generator, result, true)?;
            drain(cx, generator)
        }
        _ => Ok(()),
    }
}

/// `AsyncGeneratorDrainQueue`: settles the requests in order until the queue
/// is empty (the generator is then completed) or a `return` request awaits.
fn drain(cx: &mut Cx<'_>, generator: ObjectId) -> Result<(), Throw> {
    loop {
        if queue_len(cx, generator) == 0 {
            suspend::set_state(cx, generator, COMPLETED);
            return Ok(());
        }
        let (kind, value) = first(cx, generator);
        match kind {
            RETURN => return await_return(cx, generator, value),
            THROW => complete_step(cx, generator, Err(Throw(value)), true)?,
            _ => complete_step(cx, generator, Ok(Value::UNDEFINED), true)?,
        }
    }
}

/// `AsyncGeneratorAwaitReturn`: awaits the value of the first (a `return`)
/// request before completing it.
fn await_return(cx: &mut Cx<'_>, generator: ObjectId, value: Value) -> Result<(), Throw> {
    let constructor = Value::object(cx.realm.intrinsic(Intrinsic::Promise));
    match jobs::promise_resolve(cx, constructor, value) {
        Ok(promise) => {
            root(cx, promise);
            jobs::perform_then(
                cx,
                promise.as_object().expect("a promise"),
                Value::UNDEFINED,
                Value::UNDEFINED,
                None,
                Value::object(generator),
            )
        }
        Err(throw) if super::error::catchable(cx) => {
            complete_step(cx, generator, Err(throw), true)?;
            drain(cx, generator)
        }
        Err(throw) => Err(throw),
    }
}

/// An awaited promise settled for an async generator: the `await` in its
/// body, or `AsyncGeneratorAwaitReturn`'s.
pub(crate) fn resume_await(
    cx: &mut Cx<'_>,
    generator: ObjectId,
    resumption: Resumption,
) -> Result<(), Throw> {
    if suspend::state(cx, generator) == DRAINING {
        let outcome = match resumption {
            Resumption::Rejected(reason) => Err(Throw(reason)),
            Resumption::Fulfilled(value) => Ok(value),
            _ => Ok(Value::UNDEFINED),
        };
        complete_step(cx, generator, outcome, true)?;
        return drain(cx, generator);
    }
    let result = suspend::resume(cx, generator, resumption);
    after_run(cx, generator, result)
}

/// `AsyncGenerator.prototype.next`, `return` and `throw`: enqueue the
/// request and resume a suspended generator.
fn request(cx: &mut Cx<'_>, site: CallSite, kind: i32) -> Result<Value, Throw> {
    let constructor = Value::object(cx.realm.intrinsic(Intrinsic::Promise));
    let capability = jobs::new_capability(cx, constructor)?;
    let this = site.this(cx);
    let value = site.arg(cx, 0);
    let Some(generator) = this
        .as_object()
        .filter(|&object| cx.heap.internal_tag(object) == InternalTag::AsyncGenerator)
    else {
        let error = error::type_error(cx, "not an AsyncGenerator object");
        root(cx, error.0);
        super::call(cx, capability[2], Value::UNDEFINED, &[error.0])?;
        return Ok(capability[0]);
    };
    let mut state = suspend::state(cx, generator);
    if kind == THROW && state == START {
        suspend::complete(cx, generator);
        state = COMPLETED;
    }
    if state == COMPLETED && kind != RETURN {
        if kind == THROW {
            super::call(cx, capability[2], Value::UNDEFINED, &[value])?;
        } else {
            let result = iter::result_object(cx, Value::UNDEFINED, true)?;
            root(cx, result);
            super::call(cx, capability[1], Value::UNDEFINED, &[result])?;
        }
        return Ok(capability[0]);
    }
    let queue = queue(cx, generator)?;
    for slot in [
        Value::int32(kind),
        value,
        capability[0],
        capability[1],
        capability[2],
    ] {
        cx.heap
            .push_internal_slot(queue, slot)
            .map_err(|error| error::heap_error(cx, error))?;
    }
    match state {
        START if kind == RETURN => {
            suspend::complete(cx, generator);
            suspend::set_state(cx, generator, DRAINING);
            await_return(cx, generator, value)?;
        }
        COMPLETED if kind == RETURN => {
            suspend::set_state(cx, generator, DRAINING);
            await_return(cx, generator, value)?;
        }
        START | YIELD if queue_len(cx, generator) == 1 => resume_with_request(cx, generator)?,
        _ => {}
    }
    Ok(capability[0])
}

pub(crate) fn next(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    request(cx, site, NEXT)
}

pub(crate) fn return_(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    request(cx, site, RETURN)
}

pub(crate) fn throw(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    request(cx, site, THROW)
}
