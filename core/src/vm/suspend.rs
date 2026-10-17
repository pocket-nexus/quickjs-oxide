//! Suspension (phase 4, section 4.3): generator, async-function and
//! async-generator activations copy their register window out into a heap
//! record when they suspend and back in on top of the register file when
//! they resume, where they run as an entry frame of a nested loop.
//!
//! The record is the generator (or async state) object itself: internal
//! slot 0 the function, slot 1 the kind's own state (an async function's
//! promise, an async generator's request queue), the registers after them,
//! and a [`SavedFrame`] header. While an activation runs, its record is on
//! `VmStack::active`, where `Yield` and `Await` find it.

use super::error;
use super::interp;
use super::jobs;
use super::{Cx, Frame, Throw, frame_flags};
use crate::bytecode::{CodeUnit, Opcode};
use crate::heap::object::{FRAME_SLOTS, InternalData, InternalTag, ObjectKind, SavedFrame};
use crate::runtime::{Intrinsic, Names};
use crate::value::{BytecodeId, ObjectId, Value};

/// Activation states.
pub(crate) const START: u8 = 0;
pub(crate) const YIELD: u8 = 1;
pub(crate) const EXECUTING: u8 = 2;
pub(crate) const COMPLETED: u8 = 3;
pub(crate) const AWAIT: u8 = 4;

/// How a suspended activation continues.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Resumption {
    /// From the start (no value delivered).
    Start,
    /// `next(value)` at a `yield`.
    Next(Value),
    /// `return(value)` at a `yield`.
    Return(Value),
    /// `throw(value)` at a `yield`: thrown there (delivered as kind 2 at a
    /// `yield*`).
    Throw(Value),
    /// An awaited promise fulfilled with the value.
    Fulfilled(Value),
    /// An awaited promise rejected with the value: thrown at the `await`.
    Rejected(Value),
}

pub(crate) fn state(cx: &Cx<'_>, object: ObjectId) -> u8 {
    match &cx.heap.internal(object).data {
        InternalData::Frame(saved) => saved.state,
        _ => COMPLETED,
    }
}

pub(crate) fn set_state(cx: &mut Cx<'_>, object: ObjectId, state: u8) {
    if let InternalData::Frame(saved) = &mut cx.heap.internal_mut(object).data {
        saved.state = state;
    }
}

/// A new activation record of `tag` for the running frame, whose function
/// is `function`, with `aux` in its second slot.
pub(super) fn new_record(
    cx: &mut Cx<'_>,
    tag: InternalTag,
    prototype: ObjectId,
    function: ObjectId,
    aux: Value,
) -> Result<ObjectId, Throw> {
    cx.heap
        .alloc_internal(
            ObjectKind::Builtin,
            tag,
            Some(prototype),
            0,
            &[Value::object(function), aux],
            InternalData::Frame(SavedFrame {
                unit: 0,
                bytecode: 0,
                pc: 0,
                argc: 0,
                state: START,
            }),
            0,
        )
        .map_err(|error| error::heap_error(cx, error))
}

/// Copies the running `frame`'s registers and header into `record`, to
/// resume at `pc` in `state`.
pub(super) fn save_frame(
    cx: &mut Cx<'_>,
    frame: &Frame,
    unit: &CodeUnit,
    pc: u32,
    record: ObjectId,
    state: u8,
) {
    let base = frame.base as usize;
    let end = interp::frame_end(frame, unit) as usize;
    let heap = &mut *cx.heap;
    let registers = &heap.vm.registers[base..end];
    let internal = heap
        .payloads
        .internals
        .get_mut(heap.objects.get(record.index()).payload);
    internal.slots.truncate(FRAME_SLOTS);
    internal.slots.extend_from_slice(registers);
    internal.data = InternalData::Frame(SavedFrame {
        unit: frame.unit.index(),
        bytecode: frame.bytecode.index(),
        pc,
        argc: frame.argc,
        state,
    });
}

/// Restores `record`'s frame on top of the register file as an entry frame
/// and runs it until it suspends again (its state then says how) or
/// completes (its state is still [`EXECUTING`]; the caller marks it
/// completed).
pub(crate) fn resume(
    cx: &mut Cx<'_>,
    record: ObjectId,
    resumption: Resumption,
) -> Result<Value, Throw> {
    let InternalData::Frame(saved) = cx.heap.internal(record).data else {
        return Err(error::type_error(cx, "not a suspended activation"));
    };
    let function = cx
        .heap
        .internal_slot(record, 0)
        .as_object()
        .expect("a function");
    let registers: Vec<Value> = cx.heap.internal(record).slots[FRAME_SLOTS..].to_vec();
    let saved_top = cx.heap.vm.top;
    let base = saved_top as usize;
    let end = base + registers.len();
    if !cx.heap.vm.reserve(end) {
        return Err(error::stack_overflow(cx));
    }
    cx.heap.vm.registers[base..end].copy_from_slice(&registers);
    cx.heap.vm.top = end as u32;
    set_state(cx, record, EXECUTING);
    let unit_id = crate::bytecode::CodeUnitId(saved.unit);
    let unit = cx.code.get(unit_id);
    let frame = Frame {
        unit: unit_id,
        function,
        bytecode: BytecodeId(saved.bytecode),
        base: base as u32,
        return_pc: 0,
        return_dst: 0,
        argc: saved.argc,
        flags: frame_flags::ENTRY,
    };
    cx.heap.vm.frames.push(frame);
    let pc = saved.pc as usize;
    let point = pc.checked_sub(1).map(|at| unit.code[at]);
    let mut thrown = None;
    match (resumption, point) {
        (Resumption::Start, _) => {}
        (Resumption::Next(value) | Resumption::Return(value), Some(insn))
            if matches!(insn.op, Opcode::Yield | Opcode::YieldStar) =>
        {
            let kind = i32::from(matches!(resumption, Resumption::Return(_)));
            cx.heap.vm.registers[base + insn.a as usize] = value;
            cx.heap.vm.registers[base + insn.a as usize + 1] = Value::int32(kind);
        }
        (Resumption::Throw(value), Some(insn)) if insn.op == Opcode::YieldStar => {
            cx.heap.vm.registers[base + insn.a as usize] = value;
            cx.heap.vm.registers[base + insn.a as usize + 1] = Value::int32(2);
        }
        (Resumption::Fulfilled(value), Some(insn)) if insn.op == Opcode::Await => {
            cx.heap.vm.registers[base + insn.a as usize] = value;
        }
        (Resumption::Throw(value) | Resumption::Rejected(value), _) => thrown = Some(value),
        _ => {}
    }
    cx.heap.vm.active.push(record);
    let result = if !cx.heap.vm.enter_native() {
        cx.heap.vm.frames.pop();
        Err(error::stack_overflow(cx))
    } else {
        let result = match thrown {
            None => interp::run_at(cx, pc),
            Some(value) => interp::run_throwing(cx, pc, value),
        };
        cx.heap.vm.leave_native();
        result
    };
    cx.heap.vm.active.pop();
    cx.heap.vm.top = saved_top;
    result
}

/// Marks a finished activation completed and drops its registers.
pub(crate) fn complete(cx: &mut Cx<'_>, record: ObjectId) {
    let internal = cx.heap.internal_mut(record);
    internal.slots.truncate(FRAME_SLOTS);
    if let InternalData::Frame(saved) = &mut internal.data {
        saved.state = COMPLETED;
    }
}

/// The prototype of a generator object: the function's `prototype` when it
/// is an object, else the intrinsic one.
pub(super) fn generator_prototype(
    cx: &mut Cx<'_>,
    function: ObjectId,
    asynchronous: bool,
) -> Result<ObjectId, Throw> {
    let key = Names::key(cx.realm.names.prototype);
    let prototype = super::property::get_value(cx, function, key)?;
    Ok(prototype.as_object().unwrap_or_else(|| {
        cx.realm.intrinsic(if asynchronous {
            Intrinsic::AsyncGeneratorPrototype
        } else {
            Intrinsic::GeneratorPrototype
        })
    }))
}

// ---- async functions ----

/// The `AsyncStart` of an async function whose frame the loop just popped:
/// runs its body until it awaits or completes and returns its promise.
pub(super) fn start_async(cx: &mut Cx<'_>, record: ObjectId) -> Result<Value, Throw> {
    let promise = cx.heap.internal_slot(record, 1);
    let mark = cx.heap.roots().mark();
    cx.heap.roots().push(Value::object(record));
    let result = resume(cx, record, Resumption::Start);
    let outcome = finish_async(cx, record, result);
    cx.heap.roots().truncate(mark);
    outcome.map(|()| promise)
}

/// After an async function's activation ran: nothing while it awaits; its
/// promise settles when it completes.
fn finish_async(
    cx: &mut Cx<'_>,
    record: ObjectId,
    result: Result<Value, Throw>,
) -> Result<(), Throw> {
    if state(cx, record) == AWAIT {
        return Ok(());
    }
    complete(cx, record);
    let promise = cx
        .heap
        .internal_slot(record, 1)
        .as_object()
        .expect("a promise");
    match result {
        Ok(value) => {
            cx.heap.roots().push(value);
            jobs::resolve_promise(cx, promise, value)
        }
        Err(Throw(reason)) => {
            jobs::reject_promise(cx, promise, reason);
            Ok(())
        }
    }
}

/// `Await(value)` in the running activation: `PromiseResolve(%Promise%,
/// value)` and a reaction that resumes it. Runs before the frame is saved,
/// so a throw here is catchable at the `await`.
pub(super) fn await_value(cx: &mut Cx<'_>, value: Value) -> Result<(), Throw> {
    let record = *cx
        .heap
        .vm
        .active
        .last()
        .expect("an active async activation");
    let constructor = Value::object(cx.realm.intrinsic(Intrinsic::Promise));
    let mark = cx.heap.roots().mark();
    let result = (|| {
        let promise = jobs::promise_resolve(cx, constructor, value)?;
        cx.heap.roots().push(promise);
        let promise = promise.as_object().expect("a promise");
        jobs::perform_then(
            cx,
            promise,
            Value::UNDEFINED,
            Value::UNDEFINED,
            None,
            Value::object(record),
        )
    })();
    cx.heap.roots().truncate(mark);
    result
}

/// Resumes the activation an awaited promise settled for.
pub(crate) fn resume_await(
    cx: &mut Cx<'_>,
    record: ObjectId,
    argument: Value,
    rejected: bool,
) -> Result<(), Throw> {
    let resumption = if rejected {
        Resumption::Rejected(argument)
    } else {
        Resumption::Fulfilled(argument)
    };
    match cx.heap.internal_tag(record) {
        InternalTag::AsyncGenerator => super::async_generator::resume_await(cx, record, resumption),
        _ => {
            let result = resume(cx, record, resumption);
            finish_async(cx, record, result)
        }
    }
}
