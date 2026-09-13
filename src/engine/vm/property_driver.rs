//! Schedule prepared property reads without replaying observable key conversion.
//! Storage selection remains in object; VM owns input transfer and child replies.
use super::{
    Completion,
    call::{BytecodeCallRequest, CallableExecution},
    call_bridge::Action,
    driver::{CallStep, push_frame},
    exception::runtime_error_to_vm_error,
    execution::RunningExecution,
    frame::{FrameId, ReturnTarget},
};
use crate::engine::{
    api::{Error, runtime::Runtime},
    code::function::metadata::FunctionKind,
    heap::ContextId,
    object::{OrdinaryRead, PropertyKey},
    value::{Value, conversion::NativeConversion},
};

#[derive(Clone, Copy)]
pub(super) enum ReadKey {
    Static(u32),
    Computed { keep_key: bool },
}

/// Converted inputs stay owned after ToPrimitive's reply, even if lookup next
/// reaches a Proxy or a callable whose domain continuation is still pending.
pub(super) struct ConvertedRead {
    pub base: Value,
    pub key: Value,
    pub keep_receiver: bool,
    pub keep_key: bool,
}

pub(super) fn throw_error(
    runtime: &Runtime,
    realm: ContextId,
    error: Error,
) -> Result<CallStep, Error> {
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

pub(super) fn read(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    id: FrameId,
    key_kind: ReadKey,
    keep_receiver: bool,
) -> Result<CallStep, Error> {
    let frame = execution.frames.current_mut(id)?;
    let computed = matches!(key_kind, ReadKey::Computed { .. });
    let base = execution.slots.peek(&frame.window, usize::from(computed))?;
    let realm = frame.executable.realm;
    if computed && matches!(base, Value::Null | Value::Undefined) {
        let key = execution.slots.peek(&frame.window, 0)?;
        let message = if matches!(key_kind, ReadKey::Computed { keep_key: true })
            && !matches!(key, Value::Int(_) | Value::String(_) | Value::Symbol(_))
        {
            "value has no property"
        } else if matches!(base, Value::Null) {
            "cannot read property of null"
        } else {
            "cannot read property of undefined"
        };
        return throw_error(
            runtime,
            realm,
            Error::new(crate::engine::api::error::ErrorKind::Type, message),
        );
    }
    let (key, retained_key) = match key_kind {
        ReadKey::Static(index) => {
            let Some(atom) = frame
                .executable
                .property_key_atoms
                .as_ref()
                .and_then(|atoms| atoms.get(index as usize))
                .copied()
                .filter(|atom| !atom.is_null())
            else {
                return Err(Error::internal("property read has no linked key"));
            };
            (
                PropertyKey::from_borrowed_atom(runtime.clone(), atom)
                    .map_err(|error| Error::internal(error.to_string()))?,
                None,
            )
        }
        ReadKey::Computed { keep_key } => {
            let value = execution.slots.peek(&frame.window, 0)?;
            if matches!(value, Value::Object(_)) {
                return Err(Error::internal(
                    "object property key did not enter its conversion operation",
                ));
            }
            let key = match runtime
                .native_to_property_key(realm, value.clone())
                .map_err(runtime_error_to_vm_error)?
            {
                NativeConversion::Value(key) => key,
                NativeConversion::Throw(value) => {
                    return Ok(CallStep::Complete(Completion::Throw(value)));
                }
            };
            let retained = keep_key
                .then(|| match value {
                    Value::Int(_) | Value::String(_) | Value::Symbol(_) => Ok(value.clone()),
                    value => value.to_js_string().map(Value::String),
                })
                .transpose()?;
            (key, retained)
        }
    };
    let base = base.clone();
    let depth = execution.slots.depth(&frame.window);
    finish_read(
        runtime,
        execution,
        id,
        base,
        key,
        retained_key,
        keep_receiver,
        1 + usize::from(computed),
        depth,
    )
}

pub(super) fn read_converted(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    id: FrameId,
    input: Box<ConvertedRead>,
) -> Result<CallStep, Error> {
    let ConvertedRead {
        base,
        key,
        keep_receiver,
        keep_key,
    } = *input;
    if matches!(key, Value::Object(_)) {
        return Err(Error::internal(
            "ToPrimitive returned an object property key",
        ));
    }
    let frame = execution.frames.current_mut(id)?;
    let realm = frame.executable.realm;
    let depth = execution.slots.depth(&frame.window) + 2;
    // After an object-key conversion, GetArrayEl3 retains String/Symbol, even
    // if ToPrimitive returned an Int. Direct Int keys retain their original tag.
    let retained = if keep_key {
        Some(match &key {
            Value::Symbol(_) | Value::String(_) => key.clone(),
            value => Value::String(value.to_js_string()?),
        })
    } else {
        None
    };
    let key = match runtime
        .native_to_property_key(realm, key)
        .map_err(runtime_error_to_vm_error)?
    {
        NativeConversion::Value(key) => key,
        NativeConversion::Throw(value) => return Ok(CallStep::Complete(Completion::Throw(value))),
    };
    finish_read(
        runtime,
        execution,
        id,
        base,
        key,
        retained,
        keep_receiver,
        0,
        depth,
    )
}

#[allow(clippy::too_many_arguments)]
fn finish_read(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    id: FrameId,
    base: Value,
    key: PropertyKey,
    retained_key: Option<Value>,
    keep_receiver: bool,
    consume: usize,
    depth: usize,
) -> Result<CallStep, Error> {
    let realm = execution.frames.current_mut(id)?.executable.realm;
    let read = match runtime.prepare_value_property_read(realm, base.clone(), &key) {
        Ok(read) => read,
        Err(error) => return throw_error(runtime, realm, runtime_error_to_vm_error(error)),
    };
    read_prepared(
        runtime,
        execution,
        id,
        base,
        key,
        read,
        retained_key,
        keep_receiver,
        consume,
        depth,
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn read_prepared(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    id: FrameId,
    preserved_receiver: Value,
    key: PropertyKey,
    read: OrdinaryRead,
    retained_key: Option<Value>,
    keep_receiver: bool,
    consume: usize,
    depth: usize,
) -> Result<CallStep, Error> {
    let realm = execution.frames.current_mut(id)?.executable.realm;
    let mut request = None;
    let mut deferred = None;
    let mut proxy = None;
    let mut proxy_callback = None;
    let value = match read {
        OrdinaryRead::Complete(value) => Some(value.unwrap_or(Value::Undefined)),
        OrdinaryRead::Call { getter, receiver } => {
            let super::call::NormalizedCallback {
                callable,
                receiver,
                arguments,
                classification,
            } = match super::call::normalize_callback(runtime, realm, getter, receiver, Vec::new())?
            {
                NativeConversion::Value(call) => call,
                NativeConversion::Throw(value) => {
                    return Ok(CallStep::Complete(Completion::Throw(value)));
                }
            };
            let normal = match &classification {
                CallableExecution::Bytecode { bytecode, .. } => {
                    let state = runtime.0.state.borrow();
                    state
                        .heap
                        .function_bytecode(bytecode.bytecode_id())
                        .map_err(|error| Error::internal(error.to_string()))?
                        .metadata
                        .function_kind
                        == FunctionKind::Normal
                }
                _ => false,
            };
            let is_proxy = matches!(classification, CallableExecution::Proxy);
            if let CallableExecution::Bytecode {
                bytecode,
                closure_slots,
            } = classification
                && normal
            {
                if !execution.frames.can_push() || runtime.bytecode_call_would_overflow() {
                    return runtime
                        .bytecode_stack_overflow_completion(realm, &bytecode)
                        .map(CallStep::Complete)
                        .map_err(runtime_error_to_vm_error);
                }
                request = Some(BytecodeCallRequest {
                    callable,
                    receiver,
                    arguments,
                    new_target: Value::Undefined,
                    bytecode,
                    closure_slots,
                    caller_realm: realm,
                    return_to: ReturnTarget {
                        value_use: super::frame::ReturnValue::Push,
                        frame: id,
                        tail: false,
                        operation: None,
                    },
                });
            } else if is_proxy {
                proxy_callback = Some((callable, receiver, arguments));
            } else {
                deferred = Some(Action::Call {
                    callable,
                    receiver,
                    arguments,
                });
            }
            None
        }
        OrdinaryRead::Special {
            object, receiver, ..
        } => {
            // The Proxy protocol owns the remaining lookup stages. Earlier
            // key conversion stays consumed when its callbacks suspend.
            proxy = Some((object, key, receiver));
            None
        }
    };
    let frame = execution.frames.current_mut(id)?;
    for _ in 0..consume {
        execution.slots.pop(&mut frame.window)?;
    }
    if keep_receiver {
        execution
            .slots
            .push(&mut frame.window, preserved_receiver)?;
    }
    if let Some(key) = retained_key {
        execution.slots.push(&mut frame.window, key)?;
    }
    if let Some((object, key, receiver)) = proxy {
        return super::proxy_get_driver::start(
            runtime, execution, id, object, key, receiver, depth,
        );
    }
    if let Some((callable, receiver, arguments)) = proxy_callback {
        return super::proxy_get_driver::start_call(
            runtime,
            execution,
            id,
            callable.as_object().clone(),
            receiver,
            arguments,
            false,
            depth,
        );
    }
    if let Some(action) = deferred {
        return super::call_bridge::prepare_property(execution, id, realm, action, depth);
    }
    frame.resume_pc = frame
        .fault_pc
        .checked_add(1)
        .ok_or_else(|| Error::internal("property resume PC overflow"))?;
    if let Some(request) = request {
        push_frame(execution, request.prepare(runtime)?)?;
    } else {
        execution.slots.push(
            &mut frame.window,
            value.ok_or_else(|| Error::internal("property result missing"))?,
        )?;
    }
    #[cfg(feature = "profiling")]
    crate::engine::api::profiling::record_owned_instruction(depth);
    Ok(CallStep::Entered)
}
