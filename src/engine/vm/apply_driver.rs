//! Spread calls consume a prepared Array snapshot through existing call/construct frames.
use super::{
    Completion,
    call::{BytecodeCallRequest, CallableExecution},
    driver::{CallStep, push_frame},
    exception::runtime_error_to_vm_error,
    execution::RunningExecution,
    frame::{FrameId, ReturnTarget, ReturnValue},
};
use crate::engine::{
    api::{Error, ErrorKind, error::NativeErrorKind, runtime::Runtime},
    code::{bytecode::ApplyKind, function::metadata::FunctionKind},
    object::CallableRef,
    value::{Value, conversion::NativeConversion},
};

#[inline(never)]
pub(super) fn step(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    id: FrameId,
    kind: ApplyKind,
    identity: u64,
) -> Result<CallStep, Error> {
    let realm = execution.frames.current_mut(id)?.executable.realm;
    let result = prepare(runtime, execution, id, kind, identity);
    let Err(error) = result else { return result };
    let Some(kind) = NativeErrorKind::from_javascript_error(error.kind()) else {
        return Err(error);
    };
    Ok(CallStep::Complete(Completion::Throw(
        runtime
            .new_native_error_from_error(realm, kind, &error)
            .map_err(runtime_error_to_vm_error)?,
    )))
}

fn prepare(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    id: FrameId,
    kind: ApplyKind,
    identity: u64,
) -> Result<CallStep, Error> {
    let frame = execution.frames.current_mut(id)?;
    let realm = frame.executable.realm;
    // OP_apply's callability check precedes argument-list inspection for both magic values.
    let callable = runtime
        .callable_from_value(execution.slots.peek(&frame.window, 2)?.clone())
        .map_err(runtime_error_to_vm_error)?;
    let receiver = execution.slots.peek(&frame.window, 1)?.clone();
    let array = execution.slots.peek(&frame.window, 0)?;
    let (arguments, kind) = if matches!(array, Value::Undefined | Value::Null) {
        // The pinned nullish shortcut performs an ordinary call even for construct mode.
        (Vec::new(), ApplyKind::Call)
    } else {
        let Value::Object(array) = array else {
            return Err(Error::new(ErrorKind::Type, "not a object"));
        };
        let Some(arguments) = runtime
            .prepare_fast_array_arguments(realm, array)
            .map_err(runtime_error_to_vm_error)?
        else {
            return Ok(CallStep::Bridge);
        };
        let arguments = match arguments {
            NativeConversion::Value(arguments) => arguments,
            NativeConversion::Throw(value) => {
                return Ok(CallStep::Complete(Completion::Throw(value)));
            }
        };
        (arguments, kind)
    };
    match kind {
        ApplyKind::Construct => {
            match runtime
                .constructor_from_value(realm, Value::Object(callable.as_object().clone()))
                .map_err(runtime_error_to_vm_error)?
            {
                NativeConversion::Value(_) => {}
                NativeConversion::Throw(value) => {
                    return Ok(CallStep::Complete(Completion::Throw(value)));
                }
            }
            super::construct_driver::enter_request(
                runtime, execution, id, callable, receiver, arguments, 3, identity,
            )
        }
        ApplyKind::Call => enter_call(runtime, execution, id, callable, receiver, arguments),
    }
}

fn enter_call(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    id: FrameId,
    mut callable: CallableRef,
    mut receiver: Value,
    mut arguments: Vec<Value>,
) -> Result<CallStep, Error> {
    let realm = execution.frames.current_mut(id)?.executable.realm;
    runtime
        .validate_value_domain(&receiver, "call this value")
        .map_err(runtime_error_to_vm_error)?;
    for argument in &arguments {
        runtime
            .validate_value_domain(argument, "call argument")
            .map_err(runtime_error_to_vm_error)?;
    }
    let (bytecode, closure_slots) = loop {
        match runtime
            .bytecode_for_callable(&callable)
            .map_err(runtime_error_to_vm_error)?
        {
            CallableExecution::Bytecode {
                bytecode,
                closure_slots,
            } => break (bytecode, closure_slots),
            CallableExecution::Bound {
                target,
                this_value,
                arguments: bound,
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
                receiver = this_value;
                callable = target;
            }
            _ => return Ok(CallStep::Bridge),
        }
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
            .bytecode_stack_overflow_completion(realm, &bytecode)
            .map(CallStep::Complete)
            .map_err(runtime_error_to_vm_error);
    }
    let request = BytecodeCallRequest {
        callable,
        receiver,
        arguments,
        bytecode,
        closure_slots,
        caller_realm: realm,
        new_target: Value::Undefined,
        return_to: ReturnTarget {
            frame: id,
            value_use: ReturnValue::Push,
            tail: false,
            operation: None,
        },
    };
    let entry = request.prepare(runtime)?;
    let frame = execution.frames.current_mut(id)?;
    #[cfg(feature = "profiling")]
    let depth = execution.slots.depth(&frame.window);
    // The callee entry owns every argument before releasing the carrier and original operands.
    for _ in 0..3 {
        execution.slots.pop(&mut frame.window)?;
    }
    frame.resume_pc = frame
        .fault_pc
        .checked_add(1)
        .ok_or_else(|| Error::internal("Apply resume PC overflow"))?;
    push_frame(execution, entry)?;
    #[cfg(feature = "profiling")]
    crate::engine::api::profiling::record_owned_instruction(depth);
    Ok(CallStep::Entered)
}
