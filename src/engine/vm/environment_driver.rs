//! Environment ownership and ordinary dynamic getter calls.
use super::call::{BytecodeCallRequest, CallableExecution};
use super::driver::{CallStep, push_frame};
use super::frame::{ReturnTarget, ReturnValue};
use super::{
    Completion, exception::runtime_error_to_vm_error, execution::RunningExecution, frame::FrameId,
};
use crate::engine::api::{error::Error, runtime::Runtime};
use crate::engine::code::bytecode::{DynamicEnvironmentSource, EvalVariableSource};
use crate::engine::code::function::metadata::FunctionKind;
use crate::engine::value::{Value, conversion::NativeConversion};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum WriteTarget {
    Dynamic(DynamicEnvironmentSource),
    Reference,
    Global { index: u16, initialize: bool },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Operation {
    GlobalReference(u16),
    GlobalDelete(u16),
    GlobalGet {
        index: u16,
        strict: bool,
    },
    CreateVariable,
    CreateObject,
    CreateArray(u16),
    DefineArrayElement,
    Append,
    Iterator(super::iterator_driver::Operation),
    ToObject,
    InitializeWith(u16),
    Object(DynamicEnvironmentSource),
    Put {
        source: WriteTarget,
        name: u32,
        strict: bool,
        check_presence: bool,
    },
    Define {
        source: EvalVariableSource,
        name: u32,
    },
    Delete {
        source: DynamicEnvironmentSource,
        name: u32,
    },
    ReadReference {
        name: u32,
        strict: bool,
    },
    Has {
        source: DynamicEnvironmentSource,
        name: u32,
    },
    Get {
        source: DynamicEnvironmentSource,
        name: u32,
        strict: bool,
    },
}

#[inline(never)]
pub(super) fn step(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    id: FrameId,
    op: Operation,
) -> Result<CallStep, Error> {
    if let Operation::Iterator(op) = op {
        return super::iterator_driver::operation(runtime, execution, id, op);
    }
    if op == Operation::Append {
        return super::iterator_driver::start(runtime, execution, id);
    }
    if op == Operation::DefineArrayElement {
        return super::array_driver::define_element(runtime, execution, id);
    }
    let can_push = execution.frames.can_push();
    let frame = execution.frames.current_mut(id)?;
    let realm = frame.executable.realm;
    #[cfg(feature = "profiling")]
    let depth = execution.slots.depth(&frame.window);
    let mut request = None;
    let result = (|| -> Result<CallStep, Error> {
        match op {
            Operation::DefineArrayElement | Operation::Append | Operation::Iterator(_) => {
                unreachable!()
            }
            Operation::GlobalDelete(index) => {
                let deleted = if let Some(key) = super::environment_bindings::prepare_global_delete(
                    runtime,
                    &frame.executable,
                    &frame.cold.closure_slots,
                    index,
                )? {
                    let object = runtime
                        .global_object_for_realm(realm)
                        .map_err(runtime_error_to_vm_error)?;
                    let exists = if runtime
                        .has_own_property(&object, &key)
                        .map_err(runtime_error_to_vm_error)?
                    {
                        true
                    } else if let Some(prototype) = runtime
                        .get_prototype_of(&object)
                        .map_err(runtime_error_to_vm_error)?
                    {
                        match runtime
                            .prepare_ordinary_read(&prototype, &key, Value::Object(object.clone()))
                            .map_err(runtime_error_to_vm_error)?
                        {
                            crate::engine::object::OrdinaryRead::Complete(value) => value.is_some(),
                            crate::engine::object::OrdinaryRead::Call { .. } => true,
                            crate::engine::object::OrdinaryRead::Special { .. } => {
                                return Ok(CallStep::Bridge);
                            }
                        }
                    } else {
                        false
                    };
                    !exists
                        || runtime
                            .delete_property(&object, &key)
                            .map_err(runtime_error_to_vm_error)?
                } else {
                    false
                };
                execution
                    .slots
                    .push(&mut frame.window, Value::Bool(deleted))?;
            }
            Operation::GlobalReference(index) => {
                use super::environment_bindings::GlobalReference;
                let value = match super::environment_bindings::global_reference(
                    runtime,
                    realm,
                    &frame.executable,
                    &frame.cold.closure_slots,
                    index,
                )? {
                    GlobalReference::Lexical(object) => Value::Object(object),
                    GlobalReference::Object { object, key } => {
                        let present = if runtime
                            .has_own_property(&object, &key)
                            .map_err(runtime_error_to_vm_error)?
                        {
                            true
                        } else if let Some(prototype) = runtime
                            .get_prototype_of(&object)
                            .map_err(runtime_error_to_vm_error)?
                        {
                            match runtime
                                .prepare_ordinary_read(
                                    &prototype,
                                    &key,
                                    Value::Object(object.clone()),
                                )
                                .map_err(runtime_error_to_vm_error)?
                            {
                                crate::engine::object::OrdinaryRead::Complete(value) => {
                                    value.is_some()
                                }
                                crate::engine::object::OrdinaryRead::Call { .. } => true,
                                crate::engine::object::OrdinaryRead::Special { .. } => {
                                    return Ok(CallStep::Bridge);
                                }
                            }
                        } else {
                            false
                        };
                        if present {
                            Value::Object(object)
                        } else {
                            Value::Undefined
                        }
                    }
                };
                execution.slots.push(&mut frame.window, value)?;
            }

            Operation::Put {
                source,
                name,
                strict,
                check_presence,
            } => {
                use crate::engine::object::{
                    OrdinaryRead,
                    operations::{PropertySetAction, PropertySetRejection},
                };
                let key = if let WriteTarget::Global { index, initialize } = source {
                    match super::environment_bindings::prepare_global_write(
                        runtime,
                        &frame.executable,
                        &frame.cold.closure_slots,
                        index,
                        initialize,
                    )? {
                        super::environment_bindings::GlobalWrite::Cell(root) => {
                            let value = execution.slots.pop(&mut frame.window)?;
                            runtime
                                .write_var_ref(&root, value)
                                .map_err(runtime_error_to_vm_error)?;
                            return Ok(CallStep::Entered);
                        }
                        super::environment_bindings::GlobalWrite::Property(key) => key,
                    }
                } else {
                    linked_key(runtime, &frame.executable, name)?
                };
                let object = match source {
                    WriteTarget::Global { .. } => runtime
                        .global_object_for_realm(realm)
                        .map_err(runtime_error_to_vm_error)?,
                    WriteTarget::Dynamic(source) => super::environment_bindings::dynamic_object(
                        runtime,
                        &frame.executable,
                        source,
                        |index| execution.slots.local(&frame.window, index).ok(),
                        &frame.cold.closure_slots,
                    )?,
                    WriteTarget::Reference => match execution.slots.peek(&frame.window, 1)? {
                        Value::Object(object) if object.belongs_to(runtime) => object.clone(),
                        Value::Undefined if strict => {
                            return Err(runtime
                                .native_atom_error(
                                    crate::engine::api::ErrorKind::Reference,
                                    "'",
                                    &key,
                                    "' is not defined",
                                )
                                .map_err(runtime_error_to_vm_error)?);
                        }
                        Value::Undefined => runtime
                            .global_object_for_realm(realm)
                            .map_err(runtime_error_to_vm_error)?,
                        _ => return Err(Error::internal("invalid dynamic reference base")),
                    },
                };
                {
                    let state = runtime.0.state.borrow();
                    let data = state
                        .heap
                        .object(object.object_id())
                        .map_err(|e| Error::internal(e.to_string()))?;
                    if !matches!(
                        (data.kind, &data.payload),
                        (
                            crate::engine::heap::ObjectKind::Ordinary,
                            crate::engine::heap::ObjectPayload::Ordinary
                        ) | (
                            crate::engine::heap::ObjectKind::GlobalObject,
                            crate::engine::heap::ObjectPayload::GlobalObject { .. }
                        )
                    ) {
                        return Ok(CallStep::Bridge);
                    }
                }
                if matches!(source, WriteTarget::Reference)
                    && let Some(root) = runtime
                        .own_var_ref_root(&object, &key)
                        .map_err(runtime_error_to_vm_error)?
                {
                    // An own VarRef proves HasProperty without reading the TDZ value.
                    let cell = runtime
                        .0
                        .state
                        .borrow()
                        .heap
                        .var_ref(root.id())
                        .map_err(|e| Error::internal(e.to_string()))?
                        .clone();
                    if matches!(cell.value, crate::engine::heap::RawValue::Uninitialized) {
                        return Err(super::bindings::lexical_uninitialized_error(
                            runtime,
                            Some(key.atom()),
                            true,
                        )?);
                    }
                    if cell.is_const && strict {
                        return Err(super::bindings::lexical_read_only_error(
                            runtime,
                            Some(key.atom()),
                        )?);
                    }
                    let value = execution.slots.pop(&mut frame.window)?;
                    execution.slots.pop(&mut frame.window)?;
                    if !cell.is_const {
                        runtime
                            .write_var_ref(&root, value)
                            .map_err(runtime_error_to_vm_error)?;
                    }
                    return Ok(CallStep::Entered);
                }
                // This read-only probe never invokes getters. It proves that the
                // Set walk cannot enter an exotic protocol before selecting its setter.
                let present = match prepare_environment_read(runtime, &object, &key)? {
                    OrdinaryRead::Complete(value) => value.is_some(),
                    OrdinaryRead::Call { .. } => true,
                    OrdinaryRead::Special { .. } => return Ok(CallStep::Bridge),
                };
                if check_presence && strict && !present {
                    return Err(runtime
                        .native_atom_error(
                            crate::engine::api::ErrorKind::Reference,
                            "'",
                            &key,
                            "' is not defined",
                        )
                        .map_err(runtime_error_to_vm_error)?);
                }
                let value = execution.slots.peek(&frame.window, 0)?.clone();
                let action = runtime
                    .prepare_set_property_with_receiver_in_realm(
                        Some(realm),
                        &object,
                        &key,
                        value,
                        Value::Object(object.clone()),
                    )
                    .map_err(runtime_error_to_vm_error)?;
                match action {
                    PropertySetAction::Complete => {}
                    PropertySetAction::Rejected(_) if !strict => {}
                    PropertySetAction::Rejected(reason) => {
                        use crate::engine::api::ErrorKind;
                        return Err(match reason {
                            PropertySetRejection::ReadOnly => runtime
                                .native_atom_error(ErrorKind::Type, "'", &key, "' is read-only")
                                .map_err(runtime_error_to_vm_error)?,
                            PropertySetRejection::NoSetter => {
                                Error::new(ErrorKind::Type, "no setter for property")
                            }
                            PropertySetRejection::NotExtensible => {
                                Error::new(ErrorKind::Type, "object is not extensible")
                            }
                            _ => Error::internal(
                                "ordinary environment Set returned an exotic rejection",
                            ),
                        });
                    }
                    PropertySetAction::Throw(value) => {
                        return Ok(CallStep::Complete(Completion::Throw(value)));
                    }
                    PropertySetAction::RejectedProxyTrap => {
                        return Err(Error::internal("ordinary environment Set entered a Proxy"));
                    }
                    PropertySetAction::Call {
                        setter,
                        receiver,
                        argument,
                    } => {
                        let CallableExecution::Bytecode {
                            bytecode,
                            closure_slots,
                        } = runtime
                            .bytecode_for_callable(&setter)
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
                        request = Some(BytecodeCallRequest {
                            callable: setter,
                            receiver,
                            new_target: Value::Undefined,
                            arguments: vec![argument],
                            bytecode,
                            closure_slots,
                            caller_realm: realm,
                            return_to: ReturnTarget {
                                value_use: ReturnValue::Discard,
                                frame: id,
                                tail: false,
                                operation: None,
                            },
                        });
                    }
                }
                execution.slots.pop(&mut frame.window)?;
                if matches!(source, WriteTarget::Reference) {
                    execution.slots.pop(&mut frame.window)?;
                }
            }
            Operation::Define { source, name } => {
                use crate::engine::object::{
                    DescriptorField, OrdinaryPropertyDescriptor, operations::PropertyDefineOutcome,
                };
                let object = super::environment_bindings::eval_variable_object(
                    runtime,
                    &frame.executable,
                    source,
                    |index| execution.slots.local(&frame.window, index).ok(),
                    &frame.cold.closure_slots,
                )?;
                if runtime
                    .0
                    .state
                    .borrow()
                    .heap
                    .object(object.object_id())
                    .map_err(|e| Error::internal(e.to_string()))?
                    .kind
                    != crate::engine::heap::ObjectKind::Ordinary
                {
                    return Ok(CallStep::Bridge);
                }
                let key = linked_key(runtime, &frame.executable, name)?;
                let value = execution.slots.pop(&mut frame.window)?;
                match runtime
                    .define_own_property_in_realm(
                        Some(realm),
                        &object,
                        &key,
                        &OrdinaryPropertyDescriptor {
                            value: DescriptorField::Present(value),
                            writable: DescriptorField::Present(true),
                            enumerable: DescriptorField::Present(true),
                            configurable: DescriptorField::Present(true),
                            ..OrdinaryPropertyDescriptor::new()
                        },
                    )
                    .map_err(runtime_error_to_vm_error)?
                {
                    PropertyDefineOutcome::Defined(true) => {}
                    PropertyDefineOutcome::Defined(false) => {
                        return Err(Error::new(
                            crate::engine::api::error::ErrorKind::Type,
                            "property is not configurable",
                        ));
                    }
                    PropertyDefineOutcome::Throw(value) => {
                        return Ok(CallStep::Complete(Completion::Throw(value)));
                    }
                }
            }
            Operation::Has { .. } => {
                return Err(Error::internal("HasBinding bypassed continuation driver"));
            }
            Operation::Delete { source, name } => {
                let object = super::environment_bindings::dynamic_object(
                    runtime,
                    &frame.executable,
                    source,
                    |index| execution.slots.local(&frame.window, index).ok(),
                    &frame.cold.closure_slots,
                )?;
                {
                    let state = runtime.0.state.borrow();
                    let data = state
                        .heap
                        .object(object.object_id())
                        .map_err(|e| Error::internal(e.to_string()))?;
                    if !matches!(
                        (data.kind, &data.payload),
                        (
                            crate::engine::heap::ObjectKind::Ordinary,
                            crate::engine::heap::ObjectPayload::Ordinary
                        )
                    ) {
                        return Ok(CallStep::Bridge);
                    }
                }
                let key = linked_key(runtime, &frame.executable, name)?;
                match runtime
                    .internal_delete_property(realm, &object, &key)
                    .map_err(runtime_error_to_vm_error)?
                {
                    NativeConversion::Value(deleted) => execution
                        .slots
                        .push(&mut frame.window, Value::Bool(deleted))?,
                    NativeConversion::Throw(value) => {
                        return Ok(CallStep::Complete(Completion::Throw(value)));
                    }
                }
            }
            Operation::ReadReference { .. }
            | Operation::Object(_)
            | Operation::Get { .. }
            | Operation::GlobalGet { .. } => {
                let read = if let Operation::GlobalGet { index, strict } = op {
                    read_global_binding(
                        runtime,
                        &frame.executable,
                        &frame.cold.closure_slots,
                        index,
                        strict,
                    )?
                } else {
                    let object = match op {
                        Operation::ReadReference { name, .. } => {
                            let object = match execution.slots.peek(&frame.window, 0)? {
                                Value::Object(object) => object,
                                Value::Undefined => {
                                    let key = linked_key(runtime, &frame.executable, name)?;
                                    return Err(runtime
                                        .native_atom_error(
                                            crate::engine::api::ErrorKind::Reference,
                                            "'",
                                            &key,
                                            "' is not defined",
                                        )
                                        .map_err(runtime_error_to_vm_error)?);
                                }
                                _ => return Err(Error::internal("invalid dynamic reference base")),
                            };
                            if !object.belongs_to(runtime) {
                                return Ok(CallStep::Bridge);
                            }
                            object.clone()
                        }
                        Operation::Object(source) | Operation::Get { source, .. } => {
                            super::environment_bindings::dynamic_object(
                                runtime,
                                &frame.executable,
                                source,
                                |index| execution.slots.local(&frame.window, index).ok(),
                                &frame.cold.closure_slots,
                            )?
                        }
                        _ => unreachable!(),
                    };
                    if matches!(op, Operation::Object(_)) {
                        BindingRead::Value(Value::Object(object))
                    } else {
                        read_binding(runtime, &frame.executable, &object, op)?
                    }
                };
                match read {
                    BindingRead::Value(value) => execution.slots.push(&mut frame.window, value)?,
                    BindingRead::Bridge => return Ok(CallStep::Bridge),
                    BindingRead::Getter { getter, receiver } => {
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
                        request = Some(BytecodeCallRequest {
                            callable: getter,
                            receiver,
                            new_target: Value::Undefined,
                            arguments: Vec::new(),
                            bytecode,
                            closure_slots,
                            caller_realm: realm,
                            return_to: ReturnTarget {
                                value_use: ReturnValue::Push,
                                frame: id,
                                tail: false,
                                operation: None,
                            },
                        });
                    }
                }
            }
            Operation::CreateArray(count) => {
                let mut values = Vec::new();
                values
                    .try_reserve_exact(usize::from(count))
                    .map_err(|_| Error::internal("array elements allocation failed"))?;
                for _ in 0..count {
                    values.push(execution.slots.pop(&mut frame.window)?);
                }
                values.reverse();
                let array = runtime
                    .new_array_from_values(realm, values)
                    .map_err(runtime_error_to_vm_error)?;
                execution
                    .slots
                    .push(&mut frame.window, Value::Object(array))?;
            }
            Operation::CreateObject => {
                let object = runtime
                    .new_ordinary_object_in_realm(realm)
                    .map_err(runtime_error_to_vm_error)?;
                execution
                    .slots
                    .push(&mut frame.window, Value::Object(object))?;
            }
            Operation::CreateVariable => {
                if frame
                    .executable
                    .metadata
                    .eval_variable_object_local
                    .is_none()
                    && frame.executable.arg_eval_variable_object_local.is_none()
                {
                    return Err(Error::internal(
                        "variable-environment creation has no authenticated local",
                    ));
                }
                let object = runtime
                    .new_object(None)
                    .map_err(runtime_error_to_vm_error)?;
                execution
                    .slots
                    .push(&mut frame.window, Value::Object(object))?;
            }
            Operation::ToObject => {
                let value = execution.slots.pop(&mut frame.window)?;
                match runtime
                    .native_to_object(realm, value)
                    .map_err(runtime_error_to_vm_error)?
                {
                    NativeConversion::Value(object) => execution
                        .slots
                        .push(&mut frame.window, Value::Object(object))?,
                    NativeConversion::Throw(value) => {
                        return Ok(CallStep::Complete(Completion::Throw(value)));
                    }
                }
            }
            Operation::InitializeWith(index) => {
                let definition = frame.executable.local_definitions[usize::from(index)];
                if definition.kind
                    != crate::engine::code::function::metadata::ClosureVariableKind::WithObject
                {
                    return Err(Error::internal(
                        "with initialization received another binding kind",
                    ));
                }
                let value = execution.slots.pop(&mut frame.window)?;
                super::bindings::initialize_local_binding(
                    runtime,
                    definition.kind,
                    execution.slots.local_mut(&frame.window, index)?,
                    value,
                )?;
            }
        }
        Ok(CallStep::Entered)
    })();
    match result {
        Ok(CallStep::Entered) => {
            if let Some(request) = &request {
                if !can_push || runtime.bytecode_call_would_overflow() {
                    return runtime
                        .bytecode_stack_overflow_completion(realm, &request.bytecode)
                        .map(CallStep::Complete)
                        .map_err(runtime_error_to_vm_error);
                }
            }
            frame.resume_pc = frame
                .fault_pc
                .checked_add(1)
                .ok_or_else(|| Error::internal("environment resume PC overflow"))?;
            #[cfg(feature = "profiling")]
            crate::engine::api::profiling::record_owned_instruction(depth);
            if let Some(request) = request {
                let entry = request.prepare(runtime)?;
                push_frame(execution, entry)?;
            }
            Ok(CallStep::Entered)
        }
        Ok(completion) => Ok(completion),
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
    }
}

enum BindingRead {
    Value(Value),
    Getter {
        getter: crate::engine::object::CallableRef,
        receiver: Value,
    },
    Bridge,
}

/// Lookup owns a selected getter without invoking it.
fn read_binding(
    runtime: &Runtime,
    executable: &crate::engine::code::runtime::PublishedFunctionSnapshot,
    object: &crate::engine::object::ObjectRef,
    op: Operation,
) -> Result<BindingRead, Error> {
    use crate::engine::object::OrdinaryRead;
    let (name, strict) = match op {
        Operation::Get { name, strict, .. } | Operation::ReadReference { name, strict } => {
            (name, strict)
        }
        _ => return Err(Error::internal("dynamic read received unrelated operation")),
    };
    let key = linked_key(runtime, executable, name)?;
    if matches!(op, Operation::ReadReference { .. }) {
        let ordinary = {
            let state = runtime.0.state.borrow();
            let data = state
                .heap
                .object(object.object_id())
                .map_err(|e| Error::internal(e.to_string()))?;
            matches!(
                (data.kind, &data.payload),
                (
                    crate::engine::heap::ObjectKind::Ordinary,
                    crate::engine::heap::ObjectPayload::Ordinary
                )
            )
        };
        if ordinary
            && runtime
                .own_var_ref_root(object, &key)
                .map_err(runtime_error_to_vm_error)?
                .is_some()
        {
            // Own lexical storage establishes presence without a TDZ read.
            // The original descriptor kernel reads the cell and diagnoses TDZ.
            let Some(crate::engine::object::CompleteOrdinaryPropertyDescriptor::Data {
                value, ..
            }) = runtime
                .get_own_property(object, &key)
                .map_err(runtime_error_to_vm_error)?
            else {
                return Err(Error::internal(
                    "lexical reference lost its data descriptor",
                ));
            };
            return Ok(BindingRead::Value(value));
        }
    }
    let read = prepare_environment_read(runtime, object, &key)?;
    match read {
        OrdinaryRead::Complete(Some(value)) => Ok(BindingRead::Value(value)),
        OrdinaryRead::Complete(None) if strict => Err(runtime
            .native_atom_error(
                crate::engine::api::ErrorKind::Reference,
                "'",
                &key,
                "' is not defined",
            )
            .map_err(runtime_error_to_vm_error)?),
        OrdinaryRead::Complete(None) => Ok(BindingRead::Value(Value::Undefined)),
        crate::engine::object::OrdinaryRead::Call { getter, receiver } => {
            Ok(BindingRead::Getter { getter, receiver })
        }
        _ => Ok(BindingRead::Bridge),
    }
}

fn linked_key(
    runtime: &Runtime,
    executable: &crate::engine::code::runtime::PublishedFunctionSnapshot,
    name: u32,
) -> Result<crate::engine::object::PropertyKey, Error> {
    use crate::engine::object::PropertyKey;
    let atom = executable
        .property_key_atoms
        .as_ref()
        .and_then(|atoms| atoms.get(name as usize))
        .copied()
        .filter(|atom| !atom.is_null())
        .ok_or_else(|| Error::internal("dynamic name has no linked key"))?;
    PropertyKey::from_borrowed_atom(runtime.clone(), atom)
        .map_err(|e| Error::internal(e.to_string()))
}

/// Global storage retains its VarRefs and unresolved-name table. Read descriptors
/// through its original kernel, keeping accessor invocation in the owned driver.
pub(super) fn prepare_environment_read(
    runtime: &Runtime,
    object: &crate::engine::object::ObjectRef,
    key: &crate::engine::object::PropertyKey,
) -> Result<crate::engine::object::OrdinaryRead, Error> {
    use crate::engine::object::{CompleteOrdinaryPropertyDescriptor, OrdinaryRead};
    let global = {
        let state = runtime.0.state.borrow();
        let data = state
            .heap
            .object(object.object_id())
            .map_err(|e| Error::internal(e.to_string()))?;
        matches!(
            (data.kind, &data.payload),
            (
                crate::engine::heap::ObjectKind::GlobalObject,
                crate::engine::heap::ObjectPayload::GlobalObject { .. }
            )
        )
    };
    let receiver = Value::Object(object.clone());
    if !global
        || runtime
            .is_auto_init_own_property(object, key)
            .map_err(runtime_error_to_vm_error)?
    {
        return runtime
            .prepare_ordinary_read(object, key, receiver)
            .map_err(runtime_error_to_vm_error);
    }
    match runtime
        .get_own_property(object, key)
        .map_err(runtime_error_to_vm_error)?
    {
        Some(CompleteOrdinaryPropertyDescriptor::Data { value, .. }) => {
            Ok(OrdinaryRead::Complete(Some(value)))
        }
        Some(CompleteOrdinaryPropertyDescriptor::Accessor {
            get: Some(getter), ..
        }) => Ok(OrdinaryRead::Call { getter, receiver }),
        Some(CompleteOrdinaryPropertyDescriptor::Accessor { get: None, .. }) => {
            Ok(OrdinaryRead::Complete(Some(Value::Undefined)))
        }
        None => match runtime
            .get_prototype_of(object)
            .map_err(runtime_error_to_vm_error)?
        {
            Some(prototype) => runtime
                .prepare_ordinary_read(&prototype, key, receiver)
                .map_err(runtime_error_to_vm_error),
            None => Ok(OrdinaryRead::Complete(None)),
        },
    }
}

fn read_global_binding(
    runtime: &Runtime,
    executable: &crate::engine::code::runtime::PublishedFunctionSnapshot,
    roots: &[crate::engine::heap::roots::VarRefRoot],
    index: u16,
    strict: bool,
) -> Result<BindingRead, Error> {
    use crate::engine::{
        code::function::metadata::ClosureVariableName,
        heap::RawValue,
        object::{OrdinaryRead, PropertyKey},
    };
    let descriptor = executable
        .closure_variables
        .get(usize::from(index))
        .ok_or_else(|| Error::internal("global closure index is out of bounds"))?;
    if descriptor.kind.is_private() {
        return Err(Error::internal(
            "global read referenced a private-name binding",
        ));
    }
    let ClosureVariableName::Atom(atom) = descriptor.name else {
        return Err(Error::internal(
            "published global closure descriptor has no name atom",
        ));
    };
    let root = roots
        .get(usize::from(index))
        .ok_or_else(|| Error::internal("global closure slot is out of bounds"))?;
    if !root.belongs_to(runtime) {
        return Err(Error::internal("global closure belongs to another runtime"));
    }
    let value = runtime
        .raw_var_ref_value(root)
        .map_err(runtime_error_to_vm_error)?;
    if !matches!(value, RawValue::Uninitialized) {
        return runtime
            .root_raw_value(&value)
            .map(BindingRead::Value)
            .map_err(runtime_error_to_vm_error);
    }
    let key = PropertyKey::from_borrowed_atom(runtime.clone(), atom)
        .map_err(|e| Error::internal(e.to_string()))?;
    if descriptor.is_lexical {
        return Err(runtime
            .native_atom_error(
                crate::engine::api::ErrorKind::Reference,
                "",
                &key,
                " is not initialized",
            )
            .map_err(runtime_error_to_vm_error)?);
    }
    let object = runtime
        .global_object_for_realm(executable.realm)
        .map_err(runtime_error_to_vm_error)?;
    match prepare_environment_read(runtime, &object, &key)? {
        OrdinaryRead::Complete(Some(value)) => Ok(BindingRead::Value(value)),
        OrdinaryRead::Complete(None) if strict => Err(runtime
            .native_atom_error(
                crate::engine::api::ErrorKind::Reference,
                "'",
                &key,
                "' is not defined",
            )
            .map_err(runtime_error_to_vm_error)?),
        OrdinaryRead::Complete(None) => Ok(BindingRead::Value(Value::Undefined)),
        OrdinaryRead::Call { getter, receiver } => Ok(BindingRead::Getter { getter, receiver }),
        OrdinaryRead::Special { .. } => Ok(BindingRead::Bridge),
    }
}

#[inline(never)]
pub(super) fn reply(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    target: super::frame::ReturnTarget,
    completion: Completion,
) -> Result<CallStep, Error> {
    match target.operation {
        Some(super::frame::OperationTarget::Iterator(_)) => {
            super::iterator_driver::reply(runtime, execution, target, completion)
        }
        Some(super::frame::OperationTarget::HasBinding(_)) => {
            super::with_driver::reply(runtime, execution, target, completion)
        }
        _ => Err(Error::internal("environment reply has no operation")),
    }
}
