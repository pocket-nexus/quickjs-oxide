//! Bytecode constructor bodies run as explicit child frames. Prototype lookup
//! ordinary getter replies resume the pending constructor; exotic reads still
//! use a current-step synchronous boundary.
use crate::engine::api::{error::Error, runtime::Runtime};
use crate::engine::code::function::metadata::{ConstructorKind, FunctionKind};
use crate::engine::value::Value;
use crate::engine::value::conversion::NativeConversion;
use crate::engine::vm::Completion;
use crate::engine::vm::call::{BytecodeCallRequest, CallableExecution};
use crate::engine::vm::driver::{CallStep, push_frame};
use crate::engine::vm::exception::runtime_error_to_vm_error;
use crate::engine::vm::execution::RunningExecution;
use crate::engine::vm::frame::{ConstructorReturn, FrameId, ReturnTarget};

pub(super) fn enter(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    id: FrameId,
    count: u16,
    identity: u64,
) -> Result<CallStep, Error> {
    let count = usize::from(count);
    let frame = execution.frames.current_mut(id)?;
    let Value::Object(object) = execution.slots.peek(&frame.window, count + 1)? else {
        return Ok(CallStep::Bridge);
    };
    if !object.belongs_to(runtime)
        || !runtime
            .is_constructor(object)
            .map_err(runtime_error_to_vm_error)?
    {
        return Ok(CallStep::Bridge);
    }
    let Some(callable) = runtime
        .as_callable(object)
        .map_err(runtime_error_to_vm_error)?
    else {
        return Ok(CallStep::Bridge);
    };
    let new_target = execution.slots.peek(&frame.window, count)?.clone();
    if runtime
        .validate_value_domain(&new_target, "raw construct new target")
        .is_err()
    {
        return Ok(CallStep::Bridge);
    }
    let mut arguments = Vec::new();
    arguments
        .try_reserve_exact(count)
        .map_err(|_| Error::internal("construct arguments allocation failed"))?;
    for offset in (0..count).rev() {
        let value = execution.slots.peek(&frame.window, offset)?;
        if runtime
            .validate_value_domain(value, "construct argument")
            .is_err()
        {
            return Ok(CallStep::Bridge);
        }
        arguments.push(value.clone());
    }
    enter_request(
        runtime,
        execution,
        id,
        callable,
        new_target,
        arguments,
        count + 2,
        identity,
    )
}

pub(super) fn enter_default_derived(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    id: FrameId,
    identity: u64,
) -> Result<CallStep, Error> {
    let frame = execution.frames.current_mut(id)?;
    if matches!(frame.cold.input.new_target, Value::Undefined) {
        return Ok(CallStep::Bridge);
    }
    let Some(object) = runtime
        .get_prototype_of(&frame.cold.function)
        .map_err(runtime_error_to_vm_error)?
    else {
        return Ok(CallStep::Bridge);
    };
    if !runtime
        .is_constructor(&object)
        .map_err(runtime_error_to_vm_error)?
    {
        return Ok(CallStep::Bridge);
    }
    let Some(callable) = runtime
        .as_callable(&object)
        .map_err(runtime_error_to_vm_error)?
    else {
        return Ok(CallStep::Bridge);
    };
    let arguments = execution
        .slots
        .snapshot_actual_arguments(&frame.window, runtime)?;
    let new_target = frame.cold.input.new_target.clone();
    if runtime
        .validate_value_domain(&new_target, "raw construct new target")
        .is_err()
        || arguments.iter().any(|value| {
            runtime
                .validate_value_domain(value, "construct argument")
                .is_err()
        })
    {
        return Ok(CallStep::Bridge);
    }

    enter_request(
        runtime, execution, id, callable, new_target, arguments, 0, identity,
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn enter_request(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    id: FrameId,
    mut callable: crate::engine::object::CallableRef,
    mut new_target: Value,
    mut arguments: Vec<Value>,
    operand_count: usize,
    identity: u64,
) -> Result<CallStep, Error> {
    let realm = execution.frames.current_mut(id)?.executable.realm;
    let (bytecode, closure_slots, kind) = loop {
        if !runtime
            .is_constructor(callable.as_object())
            .map_err(runtime_error_to_vm_error)?
        {
            return Ok(CallStep::Bridge);
        }
        match runtime
            .bytecode_for_callable(&callable)
            .map_err(runtime_error_to_vm_error)?
        {
            CallableExecution::Bound {
                target,
                arguments: bound,
                ..
            } => {
                arguments = match runtime
                    .concatenate_bound_arguments(realm, &bound, &arguments)
                    .map_err(runtime_error_to_vm_error)?
                {
                    NativeConversion::Value(arguments) => arguments,
                    NativeConversion::Throw(value) => {
                        return Ok(CallStep::Complete(Completion::Throw(value)));
                    }
                };
                if matches!(&new_target, Value::Object(object) if object == callable.as_object()) {
                    new_target = Value::Object(target.as_object().clone());
                }
                callable = target;
            }
            CallableExecution::Bytecode {
                bytecode,
                closure_slots,
            } => {
                let metadata = runtime
                    .0
                    .state
                    .borrow()
                    .heap
                    .function_bytecode(bytecode.bytecode_id())
                    .map_err(|error| Error::internal(error.to_string()))?
                    .metadata;
                if metadata.function_kind != FunctionKind::Normal {
                    return Ok(CallStep::Bridge);
                }
                break (bytecode, closure_slots, metadata.constructor_kind);
            }
            _ => return Ok(CallStep::Bridge),
        }
    };
    if kind == ConstructorKind::None {
        return Err(Error::internal(
            "constructor bit disagrees with bytecode constructor metadata",
        ));
    }
    let request = BytecodeCallRequest {
        callable,
        receiver: Value::Undefined,
        new_target,
        arguments,
        bytecode,
        closure_slots,
        caller_realm: realm,
        return_to: ReturnTarget {
            value_use: crate::engine::vm::frame::ReturnValue::Push,
            frame: id,
            tail: false,
            operation: None,
        },
    };
    let pending = PendingConstructor {
        identity,
        request,
        operand_count,
    };
    if kind == ConstructorKind::Derived {
        return install(runtime, execution, pending, ConstructorReturn::Derived);
    }
    if let Value::Object(object) = &pending.request.new_target {
        use crate::engine::object::OrdinaryRead;
        let key = runtime
            .intern_property_key("prototype")
            .map_err(|error| Error::internal(error.to_string()))?;
        match runtime
            .prepare_ordinary_read(object, &key, Value::Object(object.clone()))
            .map_err(runtime_error_to_vm_error)?
        {
            OrdinaryRead::Complete(value) => {
                return finish_prototype(
                    runtime,
                    execution,
                    pending,
                    Completion::Return(value.unwrap_or(Value::Undefined)),
                );
            }
            OrdinaryRead::Call { getter, receiver } => {
                if let CallableExecution::Bytecode {
                    bytecode,
                    closure_slots,
                } = runtime
                    .bytecode_for_callable(&getter)
                    .map_err(runtime_error_to_vm_error)?
                {
                    let metadata = runtime
                        .0
                        .state
                        .borrow()
                        .heap
                        .function_bytecode(bytecode.bytecode_id())
                        .map_err(|error| Error::internal(error.to_string()))?
                        .metadata;
                    if metadata.function_kind == FunctionKind::Normal {
                        if !execution.frames.can_push() || runtime.bytecode_call_would_overflow() {
                            return runtime
                                .bytecode_stack_overflow_completion(realm, &bytecode)
                                .map(CallStep::Complete)
                                .map_err(runtime_error_to_vm_error);
                        }
                        let getter_request = BytecodeCallRequest {
                            callable: getter,
                            receiver,
                            new_target: Value::Undefined,
                            arguments: Vec::new(),
                            bytecode,
                            closure_slots,
                            caller_realm: realm,
                            return_to: ReturnTarget {
                                value_use: crate::engine::vm::frame::ReturnValue::Push,
                                frame: id,
                                tail: false,
                                operation: Some(super::frame::OperationTarget::Constructor(
                                    identity,
                                )),
                            },
                        };
                        let entry = getter_request.prepare(runtime)?;
                        let parent = execution.frames.current_mut(id)?;
                        if parent.cold.constructor_wait.is_some()
                            || parent.cold.conversion.is_some()
                        {
                            return Err(Error::internal(
                                "constructor overwrote a pending operation",
                            ));
                        }
                        parent.cold.constructor_wait = Some(pending);
                        push_frame(execution, entry)?;
                        return Ok(CallStep::Entered);
                    }
                }
                let completion = runtime
                    .call_internal(realm, &getter, receiver, &[])
                    .map_err(runtime_error_to_vm_error)?;
                return finish_prototype(runtime, execution, pending, completion);
            }
            OrdinaryRead::Special { .. } => {}
        }
    }
    // Unmigrated prototype reads execute only this step, without frame borrows.
    let receiver = match runtime
        .create_from_constructor_value(realm, &pending.request.new_target)
        .map_err(runtime_error_to_vm_error)?
    {
        Completion::Return(value) => value,
        Completion::Throw(value) => return Ok(CallStep::Complete(Completion::Throw(value))),
    };
    install(
        runtime,
        execution,
        pending,
        ConstructorReturn::Base(receiver),
    )
}

pub(super) struct PendingConstructor {
    identity: u64,
    request: BytecodeCallRequest,
    operand_count: usize,
}

pub(super) fn reply(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    target: ReturnTarget,
    completion: Completion,
) -> Result<CallStep, Error> {
    let parent = execution.frames.current_mut(target.frame)?;
    let pending = parent
        .cold
        .constructor_wait
        .take()
        .ok_or_else(|| Error::internal("constructor reply has no pending owner"))?;
    if target.operation != Some(super::frame::OperationTarget::Constructor(pending.identity)) {
        return Err(Error::internal("constructor reply identity mismatch"));
    }
    finish_prototype(runtime, execution, pending, completion)
}

fn finish_prototype(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    pending: PendingConstructor,
    completion: Completion,
) -> Result<CallStep, Error> {
    let prototype = match completion {
        Completion::Throw(value) => return Ok(CallStep::Complete(Completion::Throw(value))),
        Completion::Return(Value::Object(object)) => object,
        Completion::Return(_) => {
            let realm = match runtime
                .function_realm_from_value(
                    pending.request.caller_realm,
                    &pending.request.new_target,
                )
                .map_err(runtime_error_to_vm_error)?
            {
                NativeConversion::Value(realm) => realm,
                NativeConversion::Throw(value) => {
                    return Ok(CallStep::Complete(Completion::Throw(value)));
                }
            };
            let prototype = runtime
                .0
                .state
                .borrow()
                .heap
                .context(realm)
                .map_err(|error| Error::internal(error.to_string()))?
                .object_prototype;
            crate::engine::object::ObjectRef::from_borrowed_handle(runtime.clone(), prototype)
                .map_err(|error| Error::internal(error.to_string()))?
        }
    };
    let receiver = Value::Object(
        runtime
            .new_object(Some(&prototype))
            .map_err(runtime_error_to_vm_error)?,
    );
    install(
        runtime,
        execution,
        pending,
        ConstructorReturn::Base(receiver),
    )
}

fn install(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    pending: PendingConstructor,
    result: ConstructorReturn,
) -> Result<CallStep, Error> {
    let PendingConstructor {
        mut request,
        operand_count,
        ..
    } = pending;
    if !execution.frames.can_push() || runtime.bytecode_call_would_overflow() {
        return runtime
            .bytecode_stack_overflow_completion(request.caller_realm, &request.bytecode)
            .map(CallStep::Complete)
            .map_err(runtime_error_to_vm_error);
    }
    if let ConstructorReturn::Base(receiver) = &result {
        request.receiver = receiver.clone();
    }
    let frame = execution.frames.current_mut(request.return_to.frame)?;
    #[cfg(feature = "profiling")]
    let observed_depth = execution.slots.depth(&frame.window);
    for _ in 0..operand_count {
        execution.slots.pop(&mut frame.window)?;
    }
    frame.resume_pc = frame
        .fault_pc
        .checked_add(1)
        .ok_or_else(|| Error::internal("construct resume PC overflow"))?;
    let mut entry = request.prepare(runtime)?;
    entry.cold.constructor_return = Some(result);
    push_frame(execution, entry)?;
    #[cfg(feature = "profiling")]
    crate::engine::api::profiling::record_owned_instruction(observed_depth);
    Ok(CallStep::Entered)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum InitializerKind {
    Install,
    Instance,
    Static,
    Block,
}

/// No replay after begin installs brands or commits static initialization. Publication authenticates
/// class initializers as ordinary bytecode functions with zero parameters.
#[inline(never)]
pub(super) fn initializer(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    id: FrameId,
    mode: InitializerKind,
) -> Result<CallStep, Error> {
    let frame = execution.frames.current_mut(id)?;
    let realm = frame.executable.realm;
    #[cfg(feature = "profiling")]
    let depth = execution.slots.depth(&frame.window);
    let result = (|| -> Result<CallStep, Error> {
        let frame = execution.frames.current_mut(id)?;
        let (initializer, receiver) = match mode {
            InitializerKind::Install => {
                runtime
                    .install_class_instance_initializer(
                        realm,
                        execution.slots.peek(&frame.window, 2)?.clone(),
                        execution.slots.peek(&frame.window, 1)?.clone(),
                        execution.slots.peek(&frame.window, 0)?.clone(),
                    )
                    .map_err(runtime_error_to_vm_error)?;
                (None, Value::Undefined)
            }
            InitializerKind::Instance => {
                let receiver = execution.slots.peek(&frame.window, 1)?.clone();
                let callable = runtime
                    .begin_class_instance_initializer(
                        realm,
                        execution.slots.peek(&frame.window, 0)?.clone(),
                        &receiver,
                    )
                    .map_err(runtime_error_to_vm_error)?;
                (callable, receiver)
            }
            InitializerKind::Static => {
                let (callable, receiver) = runtime
                    .begin_class_static_initializer(
                        realm,
                        execution.slots.peek(&frame.window, 1)?.clone(),
                        execution.slots.peek(&frame.window, 0)?.clone(),
                    )
                    .map_err(runtime_error_to_vm_error)?;
                (Some(callable), receiver)
            }
            InitializerKind::Block => {
                let receiver = frame.cold.input.this_value.clone();
                let callable = runtime
                    .begin_class_static_block(
                        realm,
                        &frame.cold.function,
                        &receiver,
                        execution.slots.peek(&frame.window, 0)?.clone(),
                    )
                    .map_err(runtime_error_to_vm_error)?;
                (Some(callable), receiver)
            }
        };
        let request = if let Some(callable) = initializer {
            let CallableExecution::Bytecode {
                bytecode,
                closure_slots,
            } = runtime
                .bytecode_for_callable(&callable)
                .map_err(runtime_error_to_vm_error)?
            else {
                return Err(Error::internal(
                    "authenticated class initializer is not bytecode",
                ));
            };
            let kind = runtime
                .0
                .state
                .borrow()
                .heap
                .function_bytecode(bytecode.bytecode_id())
                .map_err(|error| Error::internal(error.to_string()))?
                .metadata
                .function_kind;
            if kind != FunctionKind::Normal {
                return Err(Error::internal(
                    "authenticated class initializer is not ordinary bytecode",
                ));
            }
            Some(BytecodeCallRequest {
                callable,
                receiver,
                new_target: Value::Undefined,
                arguments: Vec::new(),
                bytecode,
                closure_slots,
                caller_realm: realm,
                return_to: ReturnTarget {
                    frame: id,
                    tail: false,
                    operation: None,
                    value_use: super::frame::ReturnValue::Discard,
                },
            })
        } else {
            None
        };
        if let Some(request) = &request {
            if !execution.frames.can_push() || runtime.bytecode_call_would_overflow() {
                return runtime
                    .bytecode_stack_overflow_completion(realm, &request.bytecode)
                    .map(CallStep::Complete)
                    .map_err(runtime_error_to_vm_error);
            }
        }
        let frame = execution.frames.current_mut(id)?;
        execution.slots.pop(&mut frame.window)?;
        frame.resume_pc = frame
            .fault_pc
            .checked_add(1)
            .ok_or_else(|| Error::internal("initializer resume PC overflow"))?;
        if let Some(request) = request {
            let entry = request.prepare(runtime)?;
            push_frame(execution, entry)?;
        }
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_instruction(depth);
        Ok(CallStep::Entered)
    })();
    match result {
        Err(error) => {
            let Some(kind) =
                crate::engine::api::error::NativeErrorKind::from_javascript_error(error.kind())
            else {
                return Err(error);
            };
            Ok(CallStep::Complete(Completion::Throw(
                runtime
                    .new_native_error_from_error(realm, kind, &error)
                    .map_err(runtime_error_to_vm_error)?,
            )))
        }
        result => result,
    }
}

/// Object heritage suspends on an ordinary bytecode prototype getter.
/// Other class publication steps are NoJs.
#[inline(never)]
pub(super) fn define_class(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    id: FrameId,
    name: u32,
    has_heritage: bool,
    identity: u64,
) -> Result<CallStep, Error> {
    use crate::engine::heap::{BytecodeConstant, RawValue};
    let frame = execution.frames.current_mut(id)?;
    let realm = frame.executable.realm;
    let parent = execution.slots.peek(&frame.window, 1)?;
    let Some(BytecodeConstant::Value(RawValue::String(name))) = frame.executable.constant(name)
    else {
        return Err(Error::internal("class name is not a published string"));
    };
    if has_heritage && let Value::Object(parent) = parent {
        let pending = PendingClass {
            identity,
            frame: id,
            realm,
            parent: parent.clone(),
            constructor: execution.slots.peek(&frame.window, 0)?.clone(),
            name: name.clone(),
        };
        return enter_class_parent(runtime, execution, pending);
    }
    let result = runtime.define_class_pair(
        realm,
        parent.clone(),
        execution.slots.peek(&frame.window, 0)?.clone(),
        name,
        has_heritage,
    );
    finish_class_result(runtime, execution, id, result)
}

fn finish_class_result(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    id: FrameId,
    result: Result<
        crate::engine::vm::DefineClassOutcome,
        crate::engine::api::runtime_error::RuntimeError,
    >,
) -> Result<CallStep, Error> {
    use crate::engine::vm::DefineClassOutcome;
    let frame = execution.frames.current_mut(id)?;
    let realm = frame.executable.realm;
    #[cfg(feature = "profiling")]
    let depth = execution.slots.depth(&frame.window);
    // No replay once the fresh constructor/prototype pair is published.
    execution.slots.pop(&mut frame.window)?;
    execution.slots.pop(&mut frame.window)?;
    frame.resume_pc = frame
        .fault_pc
        .checked_add(1)
        .ok_or_else(|| Error::internal("class definition resume PC overflow"))?;
    #[cfg(feature = "profiling")]
    crate::engine::api::profiling::record_owned_instruction(depth);
    match result {
        Ok(DefineClassOutcome::Defined {
            constructor,
            prototype,
        }) => {
            execution.slots.push(&mut frame.window, constructor)?;
            execution.slots.push(&mut frame.window, prototype)?;
            Ok(CallStep::Entered)
        }
        Ok(DefineClassOutcome::Throw(value)) => Ok(CallStep::Complete(Completion::Throw(value))),
        Err(error) => {
            let error = runtime_error_to_vm_error(error);
            let Some(kind) =
                crate::engine::api::error::NativeErrorKind::from_javascript_error(error.kind())
            else {
                return Err(error);
            };
            Ok(CallStep::Complete(Completion::Throw(
                runtime
                    .new_native_error_from_error(realm, kind, &error)
                    .map_err(runtime_error_to_vm_error)?,
            )))
        }
    }
}

pub(super) struct PendingClass {
    identity: u64,
    frame: FrameId,
    realm: crate::engine::heap::ContextId,
    parent: crate::engine::object::ObjectRef,
    constructor: Value,
    name: crate::engine::value::JsString,
}

/// Class heritage also reads function objects, whose lazy prototype properties
/// use the existing own-property kernel. Exotics remain a pre-call handoff.
fn class_prototype_read(
    runtime: &Runtime,
    parent: &crate::engine::object::ObjectRef,
) -> Result<crate::engine::object::OrdinaryRead, Error> {
    use crate::engine::heap::ObjectPayload;
    use crate::engine::object::{CompleteOrdinaryPropertyDescriptor, OrdinaryRead};
    let key = runtime
        .intern_property_key("prototype")
        .map_err(|error| Error::internal(error.to_string()))?;
    let receiver = Value::Object(parent.clone());
    let mut current = parent.clone();
    loop {
        let read = runtime
            .prepare_ordinary_read(&current, &key, receiver.clone())
            .map_err(runtime_error_to_vm_error)?;
        let OrdinaryRead::Special { ref object, .. } = read else {
            return Ok(read);
        };
        let is_function = matches!(
            &runtime
                .0
                .state
                .borrow()
                .heap
                .object(object.object_id())
                .map_err(|e| Error::internal(e.to_string()))?
                .payload,
            ObjectPayload::BytecodeFunction { .. }
                | ObjectPayload::BoundFunction { .. }
                | ObjectPayload::NativeFunction { .. }
        );
        if !is_function {
            return Ok(read);
        }
        match runtime
            .get_own_property(object, &key)
            .map_err(runtime_error_to_vm_error)?
        {
            Some(CompleteOrdinaryPropertyDescriptor::Data { value, .. }) => {
                return Ok(OrdinaryRead::Complete(Some(value)));
            }
            Some(CompleteOrdinaryPropertyDescriptor::Accessor {
                get: Some(getter), ..
            }) => return Ok(OrdinaryRead::Call { getter, receiver }),
            Some(CompleteOrdinaryPropertyDescriptor::Accessor { get: None, .. }) => {
                return Ok(OrdinaryRead::Complete(Some(Value::Undefined)));
            }
            None => {}
        }
        let Some(next) = runtime
            .get_prototype_of(object)
            .map_err(runtime_error_to_vm_error)?
        else {
            return Ok(OrdinaryRead::Complete(None));
        };
        current = next;
    }
}

#[inline(never)]
fn enter_class_parent(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    pending: PendingClass,
) -> Result<CallStep, Error> {
    use crate::engine::object::OrdinaryRead;
    if let Err(error) = runtime.validate_class_parent(&pending.parent) {
        return finish_class_result(runtime, execution, pending.frame, Err(error));
    }
    match class_prototype_read(runtime, &pending.parent)? {
        OrdinaryRead::Complete(value) => finish_class_reply(
            runtime,
            execution,
            pending,
            Completion::Return(value.unwrap_or(Value::Undefined)),
        ),
        OrdinaryRead::Special { .. } => Ok(CallStep::Bridge),
        OrdinaryRead::Call { getter, receiver } => {
            let CallableExecution::Bytecode {
                bytecode,
                closure_slots,
            } = runtime
                .bytecode_for_callable(&getter)
                .map_err(runtime_error_to_vm_error)?
            else {
                return Ok(CallStep::Bridge);
            };
            if runtime
                .0
                .state
                .borrow()
                .heap
                .function_bytecode(bytecode.bytecode_id())
                .map_err(|e| Error::internal(e.to_string()))?
                .metadata
                .function_kind
                != FunctionKind::Normal
            {
                return Ok(CallStep::Bridge);
            }
            if !execution.frames.can_push() || runtime.bytecode_call_would_overflow() {
                return runtime
                    .bytecode_stack_overflow_completion(pending.realm, &bytecode)
                    .map(CallStep::Complete)
                    .map_err(runtime_error_to_vm_error);
            }
            let request = BytecodeCallRequest {
                callable: getter,
                receiver,
                new_target: Value::Undefined,
                arguments: Vec::new(),
                bytecode,
                closure_slots,
                caller_realm: pending.realm,
                return_to: ReturnTarget {
                    value_use: super::frame::ReturnValue::Push,
                    frame: pending.frame,
                    tail: false,
                    operation: Some(super::frame::OperationTarget::ClassDefinition(
                        pending.identity,
                    )),
                },
            };
            let entry = request.prepare(runtime)?;
            let frame = execution.frames.current_mut(pending.frame)?;
            if frame.cold.class_wait.is_some()
                || frame.cold.constructor_wait.is_some()
                || frame.cold.conversion.is_some()
            {
                return Err(Error::internal(
                    "class definition overwrote pending operation",
                ));
            }
            frame.cold.class_wait = Some(pending);
            push_frame(execution, entry)?;
            Ok(CallStep::Entered)
        }
    }
}

#[inline(never)]
pub(super) fn reply_class(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    target: ReturnTarget,
    completion: Completion,
) -> Result<CallStep, Error> {
    let pending = execution
        .frames
        .current_mut(target.frame)?
        .cold
        .class_wait
        .take()
        .ok_or_else(|| Error::internal("class reply has no pending owner"))?;
    if target.operation
        != Some(super::frame::OperationTarget::ClassDefinition(
            pending.identity,
        ))
    {
        return Err(Error::internal("class reply identity mismatch"));
    }
    finish_class_reply(runtime, execution, pending, completion)
}

fn finish_class_reply(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    pending: PendingClass,
    completion: Completion,
) -> Result<CallStep, Error> {
    let result = match completion {
        Completion::Throw(value) => Ok(crate::engine::vm::DefineClassOutcome::Throw(value)),
        Completion::Return(prototype) => runtime.finish_derived_class_pair(
            pending.realm,
            pending.constructor,
            &pending.name,
            pending.parent,
            prototype,
        ),
    };
    finish_class_result(runtime, execution, pending.frame, result)
}

/// Define a field or method on an ordinary object or bytecode constructor.
/// Classification precedes any property, name, or HomeObject mutation.
#[inline(never)]
pub(super) fn define_property(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    id: FrameId,
    key: Option<u32>,
    method: Option<(crate::engine::code::bytecode::DefineMethodKind, bool)>,
) -> Result<CallStep, Error> {
    use crate::engine::heap::{ObjectKind, ObjectPayload};
    use crate::engine::object::{PropertyKey, operations::PropertyDefineOutcome};
    let frame = execution.frames.current_mut(id)?;
    let realm = frame.executable.realm;
    let Value::Object(object) = execution
        .slots
        .peek(&frame.window, 1 + usize::from(key.is_none()))?
    else {
        return Ok(CallStep::Bridge);
    };
    let value = execution.slots.peek(&frame.window, 0)?;
    if !object.belongs_to(runtime) {
        return Ok(CallStep::Bridge);
    }
    {
        let state = runtime.0.state.borrow();
        let target = state
            .heap
            .object(object.object_id())
            .map_err(|e| Error::internal(e.to_string()))?;
        if !matches!(
            (target.kind, &target.payload),
            (ObjectKind::Ordinary, ObjectPayload::Ordinary)
                | (_, ObjectPayload::BytecodeFunction { .. })
        ) {
            return Ok(CallStep::Bridge);
        }
        if method.is_some() {
            let Value::Object(function) = value else {
                return Ok(CallStep::Bridge);
            };
            if !function.belongs_to(runtime)
                || !matches!(
                    state
                        .heap
                        .object(function.object_id())
                        .map_err(|e| Error::internal(e.to_string()))?
                        .payload,
                    ObjectPayload::BytecodeFunction { .. }
                )
            {
                return Ok(CallStep::Bridge);
            }
        }
    }
    let computed = key.is_none();
    let key = match key {
        Some(index) => {
            let atom = frame
                .executable
                .property_key_atoms
                .as_ref()
                .and_then(|atoms| atoms.get(index as usize))
                .copied()
                .filter(|atom| !atom.is_null())
                .ok_or_else(|| Error::internal("definition has no linked property key"))?;
            PropertyKey::from_borrowed_atom(runtime.clone(), atom)
                .map_err(|e| Error::internal(e.to_string()))?
        }
        None => super::property_keys::canonical(runtime, execution.slots.peek(&frame.window, 1)?)?,
    };
    #[cfg(feature = "profiling")]
    let depth = execution.slots.depth(&frame.window);
    let result = match method {
        Some((kind, enumerable)) => runtime.define_object_literal_method(
            realm,
            object,
            &key,
            value.clone(),
            kind,
            enumerable,
        ),
        None => runtime.define_public_class_field(realm, object, &key, value.clone()),
    };
    execution.slots.pop(&mut frame.window)?;
    if computed {
        execution.slots.pop(&mut frame.window)?;
    }
    frame.resume_pc = frame
        .fault_pc
        .checked_add(1)
        .ok_or_else(|| Error::internal("property definition resume PC overflow"))?;
    #[cfg(feature = "profiling")]
    crate::engine::api::profiling::record_owned_instruction(depth);
    let error = match result {
        Ok(PropertyDefineOutcome::Defined(true)) => return Ok(CallStep::Entered),
        Ok(PropertyDefineOutcome::Throw(value)) => {
            return Ok(CallStep::Complete(Completion::Throw(value)));
        }
        Ok(PropertyDefineOutcome::Defined(false)) => Error::new(
            crate::engine::api::error::ErrorKind::Type,
            "property is not configurable",
        ),
        Err(error) => runtime_error_to_vm_error(error),
    };
    let Some(kind) =
        crate::engine::api::error::NativeErrorKind::from_javascript_error(error.kind())
    else {
        return Err(error);
    };
    Ok(CallStep::Complete(Completion::Throw(
        runtime
            .new_native_error_from_error(realm, kind, &error)
            .map_err(runtime_error_to_vm_error)?,
    )))
}
