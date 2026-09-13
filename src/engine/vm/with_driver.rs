//! HasBinding resumes unscopables and exclusion reads without replaying HasProperty.
use super::{
    Completion,
    call::{BytecodeCallRequest, CallableExecution},
    driver::{CallStep, push_frame},
    exception::runtime_error_to_vm_error,
    execution::RunningExecution,
    frame::{FrameId, OperationTarget, ReturnTarget, ReturnValue},
};
use crate::engine::api::{error::Error, runtime::Runtime};
use crate::engine::code::bytecode::DynamicEnvironmentSource;
use crate::engine::code::function::metadata::FunctionKind;
use crate::engine::heap::ContextId;
use crate::engine::object::{ObjectRef, OrdinaryRead, PropertyKey, WellKnownSymbol};
use crate::engine::value::Value;

#[derive(Clone, Copy)]
enum Stage {
    Unscopables,
    Excluded,
}
pub(super) struct PendingHas {
    identity: u64,
    frame: FrameId,
    realm: ContextId,
    key: PropertyKey,
    stage: Stage,
}

#[inline(never)]
pub(super) fn start(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    id: FrameId,
    source: DynamicEnvironmentSource,
    name: u32,
    identity: u64,
) -> Result<CallStep, Error> {
    let realm = execution.frames.current_mut(id)?.executable.realm;
    let result = (|| {
        let frame = execution.frames.current_mut(id)?;
        let object = super::environment_bindings::dynamic_object(
            runtime,
            &frame.executable,
            source,
            |index| execution.slots.local(&frame.window, index).ok(),
            &frame.cold.closure_slots,
        )?;
        let atom = frame
            .executable
            .property_key_atoms
            .as_ref()
            .and_then(|atoms| atoms.get(name as usize))
            .copied()
            .filter(|atom| !atom.is_null())
            .ok_or_else(|| Error::internal("dynamic name has no linked key"))?;
        let key = PropertyKey::from_borrowed_atom(runtime.clone(), atom)
            .map_err(|e| Error::internal(e.to_string()))?;
        let present = match runtime
            .prepare_ordinary_read(&object, &key, Value::Object(object.clone()))
            .map_err(runtime_error_to_vm_error)?
        {
            OrdinaryRead::Complete(value) => value.is_some(),
            OrdinaryRead::Call { .. } => true,
            OrdinaryRead::Special { .. } => return Ok(CallStep::Bridge),
        };
        if !present || !matches!(source, DynamicEnvironmentSource::With(_)) {
            return finish(execution, id, present);
        }
        let unscopables =
            PropertyKey::from(runtime.well_known_symbol(WellKnownSymbol::Unscopables));
        read(
            runtime,
            execution,
            PendingHas {
                identity,
                frame: id,
                realm,
                key,
                stage: Stage::Unscopables,
            },
            object,
            unscopables,
        )
    })();
    materialize(runtime, realm, result)
}

fn materialize(
    runtime: &Runtime,
    realm: ContextId,
    result: Result<CallStep, Error>,
) -> Result<CallStep, Error> {
    let Err(error) = result else { return result };
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

fn finish(execution: &mut RunningExecution, id: FrameId, present: bool) -> Result<CallStep, Error> {
    let frame = execution.frames.current_mut(id)?;
    #[cfg(feature = "profiling")]
    let depth = execution.slots.depth(&frame.window);
    execution
        .slots
        .push(&mut frame.window, Value::Bool(present))?;
    frame.resume_pc = frame
        .fault_pc
        .checked_add(1)
        .ok_or_else(|| Error::internal("HasBinding resume PC overflow"))?;
    #[cfg(feature = "profiling")]
    crate::engine::api::profiling::record_owned_instruction(depth);
    Ok(CallStep::Entered)
}

/// Only the current property-read step may use the remaining synchronous
/// fallback. Once a callback ran, there is no whole-instruction handoff.
fn read(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    pending: PendingHas,
    object: ObjectRef,
    key: PropertyKey,
) -> Result<CallStep, Error> {
    let completion = match runtime
        .prepare_ordinary_read(&object, &key, Value::Object(object.clone()))
        .map_err(runtime_error_to_vm_error)?
    {
        OrdinaryRead::Complete(value) => Completion::Return(value.unwrap_or(Value::Undefined)),
        OrdinaryRead::Call { getter, receiver } => {
            if let CallableExecution::Bytecode {
                bytecode,
                closure_slots,
            } = runtime
                .bytecode_for_callable(&getter)
                .map_err(runtime_error_to_vm_error)?
            {
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
                if normal {
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
                            value_use: ReturnValue::Push,
                            frame: pending.frame,
                            tail: false,
                            operation: Some(OperationTarget::HasBinding(pending.identity)),
                        },
                    };
                    let entry = request.prepare(runtime)?;
                    let frame = execution.frames.current_mut(pending.frame)?;
                    if frame.cold.has_binding_wait.is_some()
                        || frame.cold.class_wait.is_some()
                        || frame.cold.constructor_wait.is_some()
                        || frame.cold.conversion.is_some()
                    {
                        return Err(Error::internal("HasBinding overwrote pending operation"));
                    }
                    frame.cold.has_binding_wait = Some(pending);
                    push_frame(execution, entry)?;
                    return Ok(CallStep::Entered);
                }
            }
            runtime
                .call_internal(pending.realm, &getter, receiver, &[])
                .map_err(runtime_error_to_vm_error)?
        }
        OrdinaryRead::Special { .. } => runtime
            .get_property_in_realm(pending.realm, &object, &key)
            .map_err(runtime_error_to_vm_error)?,
    };
    resume(runtime, execution, pending, completion)
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
        .has_binding_wait
        .take()
        .ok_or_else(|| Error::internal("HasBinding reply has no owner"))?;
    if target.operation != Some(OperationTarget::HasBinding(pending.identity)) {
        return Err(Error::internal("HasBinding reply identity mismatch"));
    }
    let realm = pending.realm;
    let result = resume(runtime, execution, pending, completion);
    materialize(runtime, realm, result)
}

fn resume(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    mut pending: PendingHas,
    completion: Completion,
) -> Result<CallStep, Error> {
    let value = match completion {
        Completion::Return(value) => value,
        Completion::Throw(value) => return Ok(CallStep::Complete(Completion::Throw(value))),
    };
    match pending.stage {
        Stage::Unscopables => {
            if let Value::Object(object) = value {
                pending.stage = Stage::Excluded;
                let key = pending.key.clone();
                read(runtime, execution, pending, object, key)
            } else {
                finish(execution, pending.frame, true)
            }
        }
        Stage::Excluded => {
            let excluded = runtime
                .value_to_boolean(&value)
                .map_err(runtime_error_to_vm_error)?;
            finish(execution, pending.frame, !excluded)
        }
    }
}
