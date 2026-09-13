//! Shared synchronous iterator calls with operation-specific close and record policies.
mod regions;
use super::{
    Completion,
    call::{BytecodeCallRequest, CallableExecution, NativeInvokeOutcome},
    driver::{CallStep, push_frame},
    exception::runtime_error_to_vm_error,
    execution::RunningExecution,
    frame::{FrameEntry, FrameId, OperationTarget, ReturnTarget, ReturnValue},
};
use crate::engine::{
    api::{Error, ErrorKind, error::NativeErrorKind, runtime::Runtime},
    builtins::native::{ArrayIteratorKind, NativeFunctionId, PrimitiveKind},
    code::function::metadata::FunctionKind,
    heap::{ContextId, ObjectKind, ObjectPayload},
    object::{
        CallableRef, DescriptorField, ObjectRef, OrdinaryPropertyDescriptor, OrdinaryRead,
        PropertyKey, WellKnownSymbol, operations::PropertyDefineOutcome,
    },
    value::{Value, conversion::NativeConversion},
};
pub(super) use regions::unwind;

#[derive(Clone, Copy)]
enum Stage {
    Start,
    Close,
    Finish,
    Probe,
    Method,
    Iterator,
    NextMethod,
    Next,
    Result,
    Done,
    Value,
    ReturnMethod,
    ReturnResult,
}

#[derive(Clone, Copy)]
enum Mode {
    Append,
    Start,
    Next { record_base: usize },
    Close { _instruction_depth: usize },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Operation {
    Start,
    Next(usize),
    Close,
    ClosePreserve,
    DropPreserve,
    DetachPreserve,
}

pub(super) struct PendingIterator {
    mode: Mode,
    yielded: Value,
    done: bool,
    frame: FrameId,
    pc: usize,
    generation: u64,
    realm: ContextId,
    array: Option<ObjectRef>,
    iterable: Value,
    position: u32,
    stage: Stage,
    builtin_probe: bool,
    iterator: Value,
    next: Value,
    result: Value,
    fast: Option<std::vec::IntoIter<Value>>,
    ready: bool,
    abrupt: Option<Value>,
}

enum Action {
    Read(Value, PropertyKey),
    Call(CallableRef, Value),
    Reply(Completion),
    Finish,
}

enum Invocation {
    Immediate(Completion),
    Enter(FrameEntry),
}

#[inline(never)]
pub(super) fn start(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    id: FrameId,
) -> Result<CallStep, Error> {
    let frame = execution.frames.current_mut(id)?;
    let Value::Object(array) = execution.slots.peek(&frame.window, 2)? else {
        return Ok(CallStep::Bridge);
    };
    if !array.belongs_to(runtime) {
        return Ok(CallStep::Bridge);
    }
    {
        let state = runtime.0.state.borrow();
        let object = state
            .heap
            .object(array.object_id())
            .map_err(|e| Error::internal(e.to_string()))?;
        if !matches!(
            (object.kind, &object.payload),
            (ObjectKind::Ordinary, ObjectPayload::Ordinary)
                | (ObjectKind::Array, ObjectPayload::Array { .. })
        ) {
            return Ok(CallStep::Bridge);
        }
    }
    let Value::Int(position) = execution.slots.peek(&frame.window, 1)? else {
        return Ok(CallStep::Bridge);
    };
    let array = array.clone();
    let position = *position as u32;
    let iterable = execution.slots.peek(&frame.window, 0)?.clone();
    let mut pending = PendingIterator::new(frame, id, Mode::Append)?;
    pending.array = Some(array);
    pending.position = position;
    pending.iterable = iterable;
    drive(runtime, execution, pending, None)
}

#[inline(never)]
pub(super) fn operation(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    id: FrameId,
    operation: Operation,
) -> Result<CallStep, Error> {
    let frame = execution.frames.current_mut(id)?;
    let mut pending = match operation {
        Operation::Start => {
            let mut pending = PendingIterator::new(frame, id, Mode::Start)?;
            pending.iterable = execution.slots.pop(&mut frame.window)?;
            return drive(runtime, execution, pending, None);
        }
        Operation::Next(offset) => {
            let Some(super::VmUnwindRegion::Iterator {
                record_base,
                enabled,
                asynchronous: false,
            }) = frame.cold.regions.last().copied()
            else {
                return Err(Error::internal(
                    "ForOfNext has no innermost synchronous iterator region",
                ));
            };
            let expected = record_base
                .checked_add(2)
                .and_then(|n| n.checked_add(offset))
                .ok_or_else(|| Error::internal("for-of offset overflow"))?;
            if execution.slots.depth(&frame.window) != expected {
                return Err(Error::internal(
                    "ForOfNext offset does not reach its iterator record",
                ));
            }
            let mut pending = PendingIterator::new(frame, id, Mode::Next { record_base })?;
            pending.iterator = execution.slots.peek(&frame.window, offset + 1)?.clone();
            pending.next = execution.slots.peek(&frame.window, offset)?.clone();
            pending.stage = if enabled { Stage::Next } else { Stage::Finish };
            pending.done = !enabled;
            pending
        }
        _ => {
            let preserve = operation != Operation::Close;
            let instruction_depth = execution.slots.depth(&frame.window);
            let (iterator, enabled, asynchronous) =
                regions::take(frame, &mut execution.slots, preserve)?;
            if asynchronous && !enabled {
                return Err(Error::internal(
                    "synchronous cleanup targeted a pending async iterator",
                ));
            }
            let mut pending = PendingIterator::new(
                frame,
                id,
                Mode::Close {
                    _instruction_depth: instruction_depth,
                },
            )?;
            if operation == Operation::DetachPreserve {
                if !enabled {
                    return Err(Error::internal(
                        "IteratorDetachPreserve targeted a disabled iterator region",
                    ));
                }
                execution.slots.push(&mut frame.window, iterator)?;
                pending.stage = Stage::Finish;
            } else {
                pending.iterator = iterator;
                pending.stage = if enabled && operation != Operation::DropPreserve {
                    Stage::Close
                } else {
                    Stage::Finish
                };
            }
            pending
        }
    };
    // These entries already have an iterator record; their first action has no prior JS reply.
    pending.result = Value::Undefined;
    drive(
        runtime,
        execution,
        pending,
        Some(Completion::Return(Value::Undefined)),
    )
}

fn close_unwind(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    id: FrameId,
    iterator: Value,
    value: Value,
) -> Result<CallStep, Error> {
    let frame = execution.frames.current_mut(id)?;
    let mut pending = PendingIterator::new(
        frame,
        id,
        Mode::Close {
            _instruction_depth: execution.slots.depth(&frame.window),
        },
    )?;
    pending.iterator = iterator;
    pending.abrupt = Some(value);
    pending.stage = Stage::Close;
    drive(
        runtime,
        execution,
        pending,
        Some(Completion::Return(Value::Undefined)),
    )
}

fn finish(
    execution: &mut RunningExecution,
    mut pending: Box<PendingIterator>,
) -> Result<CallStep, Error> {
    let frame = execution.frames.current_mut(pending.frame)?;
    #[cfg(feature = "profiling")]
    let depth = match pending.mode {
        Mode::Close { _instruction_depth } => _instruction_depth,
        Mode::Start => execution.slots.depth(&frame.window) + 1,
        _ => execution.slots.depth(&frame.window),
    };
    match pending.mode {
        Mode::Append => {
            for _ in 0..3 {
                execution.slots.pop(&mut frame.window)?;
            }
            if pending.abrupt.is_none() {
                execution.slots.push(
                    &mut frame.window,
                    Value::Object(
                        pending
                            .array
                            .take()
                            .ok_or_else(|| Error::internal("Append lost its target"))?,
                    ),
                )?;
                execution
                    .slots
                    .push(&mut frame.window, Value::Int(pending.position as i32))?;
            }
        }
        Mode::Start => {
            if pending.abrupt.is_none() {
                let record_base = execution.slots.depth(&frame.window);
                frame
                    .cold
                    .regions
                    .try_reserve(1)
                    .map_err(|_| Error::internal("iterator region allocation failed"))?;
                execution.slots.push(&mut frame.window, pending.iterator)?;
                execution.slots.push(&mut frame.window, pending.next)?;
                frame.cold.regions.push(super::VmUnwindRegion::Iterator {
                    record_base,
                    enabled: true,
                    asynchronous: false,
                });
            }
        }
        Mode::Next { record_base } => {
            if pending.done || pending.abrupt.is_some() {
                regions::disable(frame, &mut execution.slots, record_base)?;
            }
            if pending.abrupt.is_none() {
                execution.slots.push(&mut frame.window, pending.yielded)?;
                execution
                    .slots
                    .push(&mut frame.window, Value::Bool(pending.done))?;
            }
        }
        Mode::Close { .. } => {}
    }
    if let Some(value) = pending.abrupt {
        return Ok(CallStep::Complete(Completion::Throw(value)));
    }
    frame.resume_pc = pending
        .pc
        .checked_add(1)
        .ok_or_else(|| Error::internal("iterator resume PC overflow"))?;
    #[cfg(feature = "profiling")]
    crate::engine::api::profiling::record_owned_instruction(depth);
    Ok(CallStep::Entered)
}

#[inline(never)]
pub(super) fn reply(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    target: ReturnTarget,
    completion: Completion,
) -> Result<CallStep, Error> {
    let pending = execution
        .frames
        .current_mut(target.frame)?
        .cold
        .iterator_wait
        .take()
        .ok_or_else(|| Error::internal("iterator reply has no owner"))?;
    if target.operation != Some(OperationTarget::Iterator(pending.generation))
        || pending.frame != target.frame
    {
        return Err(Error::internal("iterator reply identity mismatch"));
    }
    drive(runtime, execution, pending, Some(completion))
}

fn materialize(runtime: &Runtime, realm: ContextId, error: Error) -> Result<Completion, Error> {
    let Some(kind) = NativeErrorKind::from_javascript_error(error.kind()) else {
        return Err(error);
    };
    Ok(Completion::Throw(
        runtime
            .new_native_error_from_error(realm, kind, &error)
            .map_err(runtime_error_to_vm_error)?,
    ))
}

#[inline(never)]
fn drive(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    mut pending: Box<PendingIterator>,
    mut response: Option<Completion>,
) -> Result<CallStep, Error> {
    loop {
        let action = match pending.advance(runtime, response.take()) {
            Ok(action) => action,
            Err(error) => {
                response = Some(materialize(runtime, pending.realm, error)?);
                continue;
            }
        };
        let result = match action {
            Action::Finish => return finish(execution, pending),
            Action::Reply(completion) => Ok(Invocation::Immediate(completion)),
            Action::Read(base, key) => read(runtime, execution, &pending, base, key),
            Action::Call(callable, receiver) => {
                invoke(runtime, execution, &pending, callable, receiver)
            }
        };
        match result {
            Ok(Invocation::Immediate(completion)) => response = Some(completion),
            Ok(Invocation::Enter(entry)) => {
                let frame = execution.frames.current_mut(pending.frame)?;
                if frame.cold.iterator_wait.is_some() {
                    return Err(Error::internal("iterator overwrote pending operation"));
                }
                frame.cold.iterator_wait = Some(pending);
                push_frame(execution, entry)?;
                return Ok(CallStep::Entered);
            }
            Err(error) => response = Some(materialize(runtime, pending.realm, error)?),
        }
    }
}

impl PendingIterator {
    fn new(frame: &mut super::frame::Frame, id: FrameId, mode: Mode) -> Result<Box<Self>, Error> {
        frame.cold.iterator_generation = frame
            .cold
            .iterator_generation
            .checked_add(1)
            .ok_or_else(|| Error::internal("iterator operation identity exhausted"))?;
        Ok(Box::new(Self {
            mode,
            yielded: Value::Undefined,
            done: false,
            frame: id,
            pc: frame.fault_pc,
            generation: frame.cold.iterator_generation,
            realm: frame.executable.realm,
            array: None,
            iterable: Value::Undefined,
            position: 0,
            stage: Stage::Start,
            builtin_probe: false,
            iterator: Value::Undefined,
            next: Value::Undefined,
            result: Value::Undefined,
            fast: None,
            ready: false,
            abrupt: None,
        }))
    }

    fn key(&self, runtime: &Runtime, name: &str) -> Result<PropertyKey, Error> {
        runtime
            .intern_property_key(name)
            .map_err(|e| Error::internal(e.to_string()))
    }

    fn advance(
        &mut self,
        runtime: &Runtime,
        response: Option<Completion>,
    ) -> Result<Action, Error> {
        let value = match response {
            Some(Completion::Throw(value)) => {
                if self.abrupt.is_some() {
                    return Ok(Action::Finish);
                }
                self.abrupt = Some(value);
                self.result = Value::Undefined;
                if !matches!(self.mode, Mode::Append) || !self.ready {
                    return Ok(Action::Finish);
                }
                self.stage = Stage::ReturnMethod;
                return Ok(Action::Read(
                    self.iterator.clone(),
                    self.key(runtime, "return")?,
                ));
            }
            Some(Completion::Return(value)) => value,
            None if matches!(self.stage, Stage::Start) => Value::Undefined,
            None => return Err(Error::internal("iterator stage lost its reply")),
        };
        match self.stage {
            Stage::Finish => Ok(Action::Finish),
            Stage::Close => {
                self.stage = Stage::ReturnMethod;
                Ok(Action::Read(
                    self.iterator.clone(),
                    self.key(runtime, "return")?,
                ))
            }
            Stage::Start => {
                self.stage = if matches!(self.mode, Mode::Append) {
                    Stage::Probe
                } else {
                    Stage::Method
                };
                Ok(Action::Read(
                    self.iterable.clone(),
                    PropertyKey::from(runtime.well_known_symbol(WellKnownSymbol::Iterator)),
                ))
            }
            Stage::Probe => {
                self.builtin_probe = super::iterator_support::is_direct_native_target(
                    runtime,
                    &value,
                    NativeFunctionId::ArrayPrototypeIterator(ArrayIteratorKind::Value),
                )?;
                // Release the first result before the second observable GetIterator lookup.
                drop(value);
                self.stage = Stage::Method;
                Ok(Action::Read(
                    self.iterable.clone(),
                    PropertyKey::from(runtime.well_known_symbol(WellKnownSymbol::Iterator)),
                ))
            }
            Stage::Method => {
                let callable = callable(runtime, value, "value is not iterable")?;
                self.stage = Stage::Iterator;
                let receiver = if matches!(self.mode, Mode::Start) {
                    std::mem::replace(&mut self.iterable, Value::Undefined)
                } else {
                    self.iterable.clone()
                };
                Ok(Action::Call(callable, receiver))
            }
            Stage::Iterator => {
                if !matches!(value, Value::Object(_)) {
                    return Err(Error::new(ErrorKind::Type, "not an object"));
                }
                self.iterator = value;
                self.stage = Stage::NextMethod;
                Ok(Action::Read(
                    self.iterator.clone(),
                    self.key(runtime, "next")?,
                ))
            }
            Stage::NextMethod => {
                self.next = value;
                if matches!(self.mode, Mode::Start) {
                    return Ok(Action::Finish);
                }
                self.fast = super::iterator_support::append_fast_array_values(
                    runtime,
                    &self.iterable,
                    &self.next,
                    self.builtin_probe,
                )?
                .map(Vec::into_iter);
                self.ready = true;
                self.stage = Stage::Next;
                Ok(Action::Reply(Completion::Return(Value::Undefined)))
            }
            Stage::Next => {
                if let Some(values) = self.fast.as_mut() {
                    let Some(value) = values.next() else {
                        return Ok(Action::Finish);
                    };
                    self.stage = Stage::Value;
                    return Ok(Action::Reply(Completion::Return(value)));
                }
                let next = callable(runtime, self.next.clone(), "not a function")?;
                self.stage = Stage::Result;
                match runtime
                    .try_call_native_iterator_next_raw(self.realm, &next, self.iterator.clone())
                    .map_err(runtime_error_to_vm_error)?
                {
                    Some(NativeInvokeOutcome::IteratorNextRaw { value, done }) => {
                        if done {
                            self.done = true;
                            return Ok(Action::Finish);
                        }
                        self.stage = Stage::Value;
                        Ok(Action::Reply(Completion::Return(value)))
                    }
                    Some(NativeInvokeOutcome::Completion(completion)) => {
                        Ok(Action::Reply(completion))
                    }
                    None => Ok(Action::Call(next, self.iterator.clone())),
                }
            }
            Stage::Result => {
                if !matches!(value, Value::Object(_)) {
                    return Err(Error::new(
                        ErrorKind::Type,
                        "iterator must return an object",
                    ));
                }
                self.result = value;
                self.stage = Stage::Done;
                Ok(Action::Read(
                    self.result.clone(),
                    self.key(runtime, "done")?,
                ))
            }
            Stage::Done => {
                if runtime
                    .value_to_boolean(&value)
                    .map_err(runtime_error_to_vm_error)?
                {
                    self.done = true;
                    return Ok(Action::Finish);
                }
                self.stage = Stage::Value;
                Ok(Action::Read(
                    self.result.clone(),
                    self.key(runtime, "value")?,
                ))
            }
            Stage::Value => {
                // The iterator-result object is no longer live when defining the element or closing.
                self.result = Value::Undefined;
                if matches!(self.mode, Mode::Next { .. }) {
                    self.yielded = value;
                    return Ok(Action::Finish);
                }
                let key = runtime
                    .intern_property_key(&self.position.to_string())
                    .map_err(|e| Error::internal(e.to_string()))?;
                let outcome = runtime
                    .define_own_property_in_realm(
                        Some(self.realm),
                        self.array
                            .as_ref()
                            .ok_or_else(|| Error::internal("Append lost its target"))?,
                        &key,
                        &OrdinaryPropertyDescriptor {
                            value: DescriptorField::Present(value),
                            writable: DescriptorField::Present(true),
                            enumerable: DescriptorField::Present(true),
                            configurable: DescriptorField::Present(true),
                            ..OrdinaryPropertyDescriptor::new()
                        },
                    )
                    .map_err(runtime_error_to_vm_error)?;
                match outcome {
                    PropertyDefineOutcome::Defined(true) => {}
                    PropertyDefineOutcome::Defined(false) => {
                        return Err(Error::new(ErrorKind::Type, "property is not configurable"));
                    }
                    PropertyDefineOutcome::Throw(value) => {
                        return Ok(Action::Reply(Completion::Throw(value)));
                    }
                }
                self.position = self.position.wrapping_add(1);
                self.stage = Stage::Next;
                Ok(Action::Reply(Completion::Return(Value::Undefined)))
            }
            Stage::ReturnMethod => {
                if matches!(value, Value::Undefined | Value::Null) {
                    return Ok(Action::Finish);
                }
                let method = callable(runtime, value, "not a function")?;
                self.stage = Stage::ReturnResult;
                Ok(Action::Call(method, self.iterator.clone()))
            }
            // With an exception pending, even a primitive return result is ignored.
            Stage::ReturnResult => {
                if self.abrupt.is_none() && !matches!(value, Value::Object(_)) {
                    return Err(Error::new(ErrorKind::Type, "not an object"));
                }
                Ok(Action::Finish)
            }
        }
    }
}

fn callable(runtime: &Runtime, value: Value, message: &str) -> Result<CallableRef, Error> {
    if let Value::Object(object) = value {
        if let Some(callable) = runtime
            .as_callable(&object)
            .map_err(runtime_error_to_vm_error)?
        {
            return Ok(callable);
        }
    }
    Err(Error::new(ErrorKind::Type, message))
}

fn read(
    runtime: &Runtime,
    execution: &RunningExecution,
    pending: &PendingIterator,
    base: Value,
    key: PropertyKey,
) -> Result<Invocation, Error> {
    let object = match &base {
        Value::Object(object) => object.clone(),
        Value::Null | Value::Undefined => {
            return Err(Error::new(
                ErrorKind::Type,
                if matches!(base, Value::Null) {
                    "cannot read property of null"
                } else {
                    "cannot read property of undefined"
                },
            ));
        }
        _ => {
            let kind = match base {
                Value::Bool(_) => PrimitiveKind::Boolean,
                Value::Int(_) | Value::Float(_) => PrimitiveKind::Number,
                Value::BigInt(_) => PrimitiveKind::BigInt,
                Value::Symbol(_) => PrimitiveKind::Symbol,
                Value::String(_) => PrimitiveKind::String,
                _ => unreachable!(),
            };
            // Only @@iterator is read on primitive sources; it has no string own property.
            runtime
                .primitive_prototype_for_realm(pending.realm, kind)
                .map_err(runtime_error_to_vm_error)?
        }
    };
    match runtime
        .prepare_ordinary_read(&object, &key, base.clone())
        .map_err(runtime_error_to_vm_error)?
    {
        OrdinaryRead::Complete(value) => Ok(Invocation::Immediate(Completion::Return(
            value.unwrap_or(Value::Undefined),
        ))),
        OrdinaryRead::Call { getter, receiver } => {
            invoke(runtime, execution, pending, getter, receiver)
        }
        OrdinaryRead::Special { .. } => runtime
            .internal_get(pending.realm, &object, &key, base)
            .map(Invocation::Immediate)
            .map_err(runtime_error_to_vm_error),
    }
}

fn invoke(
    runtime: &Runtime,
    execution: &RunningExecution,
    pending: &PendingIterator,
    original: CallableRef,
    receiver: Value,
) -> Result<Invocation, Error> {
    let mut callable = original.clone();
    let mut actual_receiver = receiver.clone();
    let mut arguments = Vec::new();
    loop {
        match runtime
            .bytecode_for_callable(&callable)
            .map_err(runtime_error_to_vm_error)?
        {
            CallableExecution::Bytecode {
                bytecode,
                closure_slots,
            } => {
                let normal = runtime
                    .0
                    .state
                    .borrow()
                    .heap
                    .function_bytecode(bytecode.bytecode_id())
                    .map_err(|e| Error::internal(e.to_string()))?
                    .metadata
                    .function_kind
                    == FunctionKind::Normal;
                if !normal {
                    break;
                }
                if !execution.frames.can_push() || runtime.bytecode_call_would_overflow() {
                    return runtime
                        .bytecode_stack_overflow_completion(pending.realm, &bytecode)
                        .map(Invocation::Immediate)
                        .map_err(runtime_error_to_vm_error);
                }
                return BytecodeCallRequest {
                    callable,
                    receiver: actual_receiver,
                    new_target: Value::Undefined,
                    arguments,
                    bytecode,
                    closure_slots,
                    caller_realm: pending.realm,
                    return_to: ReturnTarget {
                        value_use: ReturnValue::Push,
                        frame: pending.frame,
                        tail: false,
                        operation: Some(OperationTarget::Iterator(pending.generation)),
                    },
                }
                .prepare(runtime)
                .map(Invocation::Enter);
            }
            CallableExecution::Bound {
                target,
                this_value,
                arguments: bound,
            } => {
                arguments = match runtime
                    .concatenate_bound_arguments(pending.realm, &bound, &arguments)
                    .map_err(runtime_error_to_vm_error)?
                {
                    NativeConversion::Value(arguments) => arguments,
                    NativeConversion::Throw(value) => {
                        return Ok(Invocation::Immediate(Completion::Throw(value)));
                    }
                };
                callable = target;
                actual_receiver = this_value;
            }
            _ => break,
        }
    }
    // This selected native/exotic call remains synchronous until its domain migrates in S05.
    runtime
        .call_internal(pending.realm, &original, receiver, &[])
        .map(Invocation::Immediate)
        .map_err(runtime_error_to_vm_error)
}
