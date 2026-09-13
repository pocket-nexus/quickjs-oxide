//! The owned stack's one instruction match. Dynamic coercion and unimplemented
//! protocols exit before consuming their operands; S04–S07 replace that bridge.

use crate::engine::api::error::Error;
use crate::engine::code::bytecode::Instruction;
use crate::engine::heap::{BytecodeConstant, RawValue, SlotReleaseReadiness};
use crate::engine::value::Value;
use crate::engine::value::number::operations::Number;
use crate::engine::vm::bindings::FrameBinding;
use crate::engine::vm::exception::runtime_error_to_vm_error;
use crate::engine::vm::execution::RunningExecution;
use crate::engine::vm::frame::FrameId;
use crate::engine::vm::stack::{FrameWindow, SlotStore, copy_value};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum BindingSource {
    Closure,
    Local,
    Argument,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RunExit {
    ApplyEval(u16),
    Apply(crate::engine::code::bytecode::ApplyKind),
    Eval {
        arguments: u16,
        environment: u16,
    },
    Call {
        arguments: u16,
        method: bool,
        tail: bool,
    },
    GetField {
        index: u32,
        keep_receiver: bool,
    },
    GetElement {
        keep_receiver: bool,
        keep_key: bool,
    },
    InitializeDerived(u16),
    LexicalUninitialized(u16),
    Binding {
        source: BindingSource,
        index: u16,
        write: bool,
        checked: bool,
        keep: bool,
    },
    ClassInitializer(super::construct_driver::InitializerKind),
    DefineClass {
        name: u32,
        has_heritage: bool,
    },
    DefineProperty {
        key: Option<u32>,
        method: Option<(crate::engine::code::bytecode::DefineMethodKind, bool)>,
    },
    Environment(super::environment_driver::Operation),
    GetSuper,
    ReturnDerived(u16),
    InitDerivedConstructor,
    Construct(u16),
    ConvertAdd,
    ConvertPlus,
    ConvertPropertyKey,
    NormalizeThis,
    Arguments(crate::engine::code::bytecode::ArgumentsKind),
    Rest(u16),
    InstantiateClosure(u32),
    SetName(Option<u32>),
    CloseCaptured(u16),
    ResetCaptured(u16),
    Catch(u32),
    DropCatch,
    NipCatch,
    Throw,
    BindingError {
        index: u32,
        redeclaration: bool,
    },
    PrivateInitialize {
        index: u16,
        kind: super::private_bindings::Initialization,
    },
    PrivateAccess {
        source: crate::engine::code::bytecode::PrivateNameSource,
        access: super::private_access::Access,
    },
    StrictEquality(bool),
    Complete,
    Bridge,
}

fn number(value: &Value) -> Option<Number> {
    value.as_number_repr()
}
fn value(number: Number) -> Value {
    number.into()
}
fn immediate(value: &Value) -> bool {
    matches!(
        value,
        Value::Undefined | Value::Null | Value::Bool(_) | Value::Int(_) | Value::Float(_)
    )
}
fn binary(
    slots: &mut SlotStore,
    window: &mut FrameWindow,
    operation: impl FnOnce(Number, Number) -> Value,
) -> Result<bool, Error> {
    let (Some(left), Some(right)) = (
        number(slots.peek(window, 1)?),
        number(slots.peek(window, 0)?),
    ) else {
        return Ok(false);
    };
    let result = operation(left, right);
    slots.pop(window)?;
    slots.pop(window)?;
    slots.push(window, result)?;
    Ok(true)
}

fn release_displaced(
    runtime: &crate::engine::api::runtime::Runtime,
    old: FrameBinding,
) -> Result<(), Error> {
    let FrameBinding::Direct(mut old) = old else {
        return Err(Error::internal(
            "non-direct binding passed a direct release preflight",
        ));
    };
    // Between the initial proof and this commit, only moves and possibly one
    // retain occurred. Neither can invalidate the no-drain proof.
    if !runtime
        .try_release_slot_value(&mut old)
        .map_err(runtime_error_to_vm_error)?
    {
        return Err(Error::internal(
            "slot release proof changed without a callback",
        ));
    }
    Ok(())
}

pub(super) fn run(execution: &mut RunningExecution, id: FrameId) -> Result<RunExit, Error> {
    let frame = execution.frames.current_mut(id)?;
    let slots = &mut execution.slots;
    let runtime = frame.cold.function.runtime();
    loop {
        frame.fault_pc = frame.resume_pc;
        let instruction = frame
            .executable
            .code
            .get(frame.fault_pc)
            .ok_or_else(|| Error::internal("owned bytecode ended without return"))?;
        let window = &mut frame.window;
        #[cfg(feature = "profiling")]
        let observed_depth = slots.depth(window);
        let mut next_pc = frame
            .fault_pc
            .checked_add(1)
            .ok_or_else(|| Error::internal("owned program counter overflow"))?;
        let handled = match instruction {
            Instruction::Call(arguments)
            | Instruction::TailCall(arguments)
            | Instruction::CallMethod(arguments)
            | Instruction::TailCallMethod(arguments) => {
                return Ok(RunExit::Call {
                    arguments: *arguments,
                    method: matches!(
                        instruction,
                        Instruction::CallMethod(_) | Instruction::TailCallMethod(_)
                    ),
                    tail: matches!(
                        instruction,
                        Instruction::TailCall(_) | Instruction::TailCallMethod(_)
                    ),
                });
            }
            Instruction::Apply(kind) => return Ok(RunExit::Apply(*kind)),
            Instruction::ApplySuper => {
                return Ok(RunExit::Apply(
                    crate::engine::code::bytecode::ApplyKind::Construct,
                ));
            }
            Instruction::ApplyEval { environment } => return Ok(RunExit::ApplyEval(*environment)),
            Instruction::Eval {
                argument_count,
                environment,
            } => {
                return Ok(RunExit::Eval {
                    arguments: *argument_count,
                    environment: *environment,
                });
            }
            Instruction::PushThis => {
                let value = if let Some(value) = &frame.cold.normalized_this {
                    copy_value(value)?
                } else if frame.executable.metadata.strict
                    || matches!(frame.cold.input.this_value, Value::Object(_))
                {
                    copy_value(&frame.cold.input.this_value)?
                } else if matches!(frame.cold.input.this_value, Value::Undefined | Value::Null) {
                    copy_value(&Value::Object(frame.cold.input.callee_global.clone()))?
                } else {
                    return Ok(RunExit::NormalizeThis);
                };
                slots.push(window, value)?;
                true
            }
            Instruction::GetField(index) | Instruction::GetField2(index) => {
                return Ok(RunExit::GetField {
                    index: *index,
                    keep_receiver: matches!(instruction, Instruction::GetField2(_)),
                });
            }
            Instruction::GetArrayEl | Instruction::GetArrayEl2 | Instruction::GetArrayEl3 => {
                return Ok(RunExit::GetElement {
                    keep_receiver: !matches!(instruction, Instruction::GetArrayEl),
                    keep_key: matches!(instruction, Instruction::GetArrayEl3),
                });
            }
            Instruction::Construct(count) | Instruction::ConstructSuper(count) => {
                return Ok(RunExit::Construct(*count));
            }
            Instruction::PushNewTarget => {
                slots.push(window, copy_value(&frame.cold.input.new_target)?)?;
                true
            }
            Instruction::InitializeDerivedLocal(index) => {
                return Ok(RunExit::InitializeDerived(*index));
            }
            Instruction::GetSuper => return Ok(RunExit::GetSuper),
            Instruction::ReturnDerived(index) => return Ok(RunExit::ReturnDerived(*index)),
            Instruction::CheckCtor => !matches!(frame.cold.input.new_target, Value::Undefined),
            Instruction::PushActiveFunction => {
                slots.push(window, Value::Object(frame.cold.function.clone()))?;
                #[cfg(feature = "profiling")]
                crate::engine::api::profiling::record_owned_storage(
                    crate::engine::api::profiling::OwnedStorageEvent::Copy { heap_root: true },
                );
                true
            }
            Instruction::InitDerivedConstructor => return Ok(RunExit::InitDerivedConstructor),
            Instruction::PutVar(index) | Instruction::PutVarInit(index) => {
                return Ok(RunExit::Environment(
                    super::environment_driver::Operation::Put {
                        source: super::environment_driver::WriteTarget::Global {
                            index: *index,
                            initialize: matches!(instruction, Instruction::PutVarInit(_)),
                        },
                        name: 0, // Global names come from the authenticated closure descriptor.
                        strict: frame.executable.metadata.strict,
                        check_presence: true,
                    },
                ));
            }
            Instruction::DeleteVar(index) => {
                return Ok(RunExit::Environment(
                    super::environment_driver::Operation::GlobalDelete(*index),
                ));
            }
            Instruction::GetVar(index) | Instruction::GetVarUndef(index) => {
                return Ok(RunExit::Environment(
                    super::environment_driver::Operation::GlobalGet {
                        index: *index,
                        strict: matches!(instruction, Instruction::GetVar(_)),
                    },
                ));
            }
            Instruction::GlobalReference(index) => {
                return Ok(RunExit::Environment(
                    super::environment_driver::Operation::GlobalReference(*index),
                ));
            }
            Instruction::GetRefValue(name) | Instruction::GetRefValueUndef(name) => {
                return Ok(RunExit::Environment(
                    super::environment_driver::Operation::ReadReference {
                        name: *name,
                        strict: matches!(instruction, Instruction::GetRefValue(_))
                            && frame.executable.metadata.strict,
                    },
                ));
            }
            Instruction::DeleteDynamicBinding { source, name } => {
                return Ok(RunExit::Environment(
                    super::environment_driver::Operation::Delete {
                        source: *source,
                        name: *name,
                    },
                ));
            }
            Instruction::DeleteEvalVariable { source, name } => {
                return Ok(RunExit::Environment(
                    super::environment_driver::Operation::Delete {
                        source: crate::engine::code::bytecode::DynamicEnvironmentSource::Eval(
                            *source,
                        ),
                        name: *name,
                    },
                ));
            }
            Instruction::DynamicEnvironmentObject(source) => {
                return Ok(RunExit::Environment(
                    super::environment_driver::Operation::Object(*source),
                ));
            }
            Instruction::HasDynamicBinding { source, name } => {
                return Ok(RunExit::Environment(
                    super::environment_driver::Operation::Has {
                        source: *source,
                        name: *name,
                    },
                ));
            }
            Instruction::PutDynamicBinding { source, name } => {
                return Ok(RunExit::Environment(
                    super::environment_driver::Operation::Put {
                        source: super::environment_driver::WriteTarget::Dynamic(*source),
                        name: *name,
                        strict: frame.executable.metadata.strict,
                        check_presence: true,
                    },
                ));
            }
            Instruction::PutEvalVariable { source, name } => {
                return Ok(RunExit::Environment(
                    super::environment_driver::Operation::Put {
                        source: super::environment_driver::WriteTarget::Dynamic(
                            crate::engine::code::bytecode::DynamicEnvironmentSource::Eval(*source),
                        ),
                        name: *name,
                        strict: false,
                        check_presence: false,
                    },
                ));
            }
            Instruction::PutRefValue(name) => {
                return Ok(RunExit::Environment(
                    super::environment_driver::Operation::Put {
                        source: super::environment_driver::WriteTarget::Reference,
                        name: *name,
                        strict: frame.executable.metadata.strict,
                        check_presence: true,
                    },
                ));
            }
            Instruction::HasEvalVariable { source, name } => {
                return Ok(RunExit::Environment(
                    super::environment_driver::Operation::Has {
                        source: crate::engine::code::bytecode::DynamicEnvironmentSource::Eval(
                            *source,
                        ),
                        name: *name,
                    },
                ));
            }
            Instruction::GetEvalVariable { source, name } => {
                return Ok(RunExit::Environment(
                    super::environment_driver::Operation::Get {
                        source: crate::engine::code::bytecode::DynamicEnvironmentSource::Eval(
                            *source,
                        ),
                        name: *name,
                        strict: false,
                    },
                ));
            }
            Instruction::DefineEvalVariable { source, name } => {
                return Ok(RunExit::Environment(
                    super::environment_driver::Operation::Define {
                        source: *source,
                        name: *name,
                    },
                ));
            }
            Instruction::GetDynamicBinding { source, name } => {
                return Ok(RunExit::Environment(
                    super::environment_driver::Operation::Get {
                        source: *source,
                        name: *name,
                        strict: frame.executable.metadata.strict,
                    },
                ));
            }
            Instruction::ForOfStart
            | Instruction::ForOfNext(_)
            | Instruction::IteratorClose
            | Instruction::IteratorClosePreserve
            | Instruction::IteratorDropPreserve
            | Instruction::IteratorDetachPreserve => {
                use super::iterator_driver::Operation;
                let op = match instruction {
                    Instruction::ForOfStart => Operation::Start,
                    Instruction::ForOfNext(offset) => Operation::Next(usize::from(*offset)),
                    Instruction::IteratorClose => Operation::Close,
                    Instruction::IteratorClosePreserve => Operation::ClosePreserve,
                    Instruction::IteratorDropPreserve => Operation::DropPreserve,
                    Instruction::IteratorDetachPreserve => Operation::DetachPreserve,
                    _ => unreachable!(),
                };
                return Ok(RunExit::Environment(
                    super::environment_driver::Operation::Iterator(op),
                ));
            }
            Instruction::Append => {
                return Ok(RunExit::Environment(
                    super::environment_driver::Operation::Append,
                ));
            }
            Instruction::DefineArrayEl => {
                return Ok(RunExit::Environment(
                    super::environment_driver::Operation::DefineArrayElement,
                ));
            }
            Instruction::ArrayFrom(count) => {
                return Ok(RunExit::Environment(
                    super::environment_driver::Operation::CreateArray(*count),
                ));
            }
            Instruction::Object => {
                return Ok(RunExit::Environment(
                    super::environment_driver::Operation::CreateObject,
                ));
            }
            Instruction::VariableEnvironment => {
                return Ok(RunExit::Environment(
                    super::environment_driver::Operation::CreateVariable,
                ));
            }
            Instruction::ToObject => {
                if matches!(slots.peek(window, 0)?, Value::Object(_)) {
                    true
                } else {
                    return Ok(RunExit::Environment(
                        super::environment_driver::Operation::ToObject,
                    ));
                }
            }
            Instruction::ToPropKey => match slots.peek(window, 0)? {
                Value::Int(_) | Value::String(_) => true,
                Value::Symbol(symbol) if symbol.belongs_to(runtime) => true,
                _ => return Ok(RunExit::ConvertPropertyKey),
            },
            Instruction::DefineFieldComputed => {
                return Ok(RunExit::DefineProperty {
                    key: None,
                    method: None,
                });
            }
            Instruction::DefineMethodComputed { kind, enumerable } => {
                return Ok(RunExit::DefineProperty {
                    key: None,
                    method: Some((*kind, *enumerable)),
                });
            }
            Instruction::DefineField(key) => {
                return Ok(RunExit::DefineProperty {
                    key: Some(*key),
                    method: None,
                });
            }
            Instruction::DefineMethod {
                key,
                kind,
                enumerable,
            } => {
                return Ok(RunExit::DefineProperty {
                    key: Some(*key),
                    method: Some((*kind, *enumerable)),
                });
            }
            Instruction::DefineClass { name, has_heritage } => {
                return Ok(RunExit::DefineClass {
                    name: *name,
                    has_heritage: *has_heritage,
                });
            }
            Instruction::InstallClassInstanceInitializer => {
                return Ok(RunExit::ClassInitializer(
                    super::construct_driver::InitializerKind::Install,
                ));
            }
            Instruction::CallClassInstanceInitializer => {
                return Ok(RunExit::ClassInitializer(
                    super::construct_driver::InitializerKind::Instance,
                ));
            }
            Instruction::RunClassStaticInitializer => {
                return Ok(RunExit::ClassInitializer(
                    super::construct_driver::InitializerKind::Static,
                ));
            }
            Instruction::CallClassStaticBlock => {
                return Ok(RunExit::ClassInitializer(
                    super::construct_driver::InitializerKind::Block,
                ));
            }
            Instruction::Nop | Instruction::MarkSuperCall => true,
            Instruction::PushI32(number) => {
                slots.push(window, Value::Int(*number))?;
                true
            }
            Instruction::Undefined => {
                slots.push(window, Value::Undefined)?;
                true
            }
            Instruction::Null => {
                slots.push(window, Value::Null)?;
                true
            }
            Instruction::PushTrue => {
                slots.push(window, Value::Bool(true))?;
                true
            }
            Instruction::PushFalse => {
                slots.push(window, Value::Bool(false))?;
                true
            }
            Instruction::PushConst(index) => {
                let result = match frame.executable.constant(*index) {
                    Some(BytecodeConstant::Value(RawValue::Int(number))) => {
                        Some(Value::Int(*number))
                    }
                    Some(BytecodeConstant::Value(RawValue::Float(number))) => {
                        Some(Value::Float(*number))
                    }
                    Some(BytecodeConstant::Value(RawValue::Undefined)) => Some(Value::Undefined),
                    Some(BytecodeConstant::Value(RawValue::Null)) => Some(Value::Null),
                    Some(BytecodeConstant::Value(RawValue::Bool(value))) => {
                        Some(Value::Bool(*value))
                    }
                    Some(BytecodeConstant::Value(RawValue::String(value))) => {
                        Some(Value::String(value.clone()))
                    }
                    Some(BytecodeConstant::Value(RawValue::BigInt(value))) => {
                        Some(Value::BigInt(value.clone()))
                    }
                    _ => None,
                };
                if let Some(value) = result {
                    #[cfg(feature = "profiling")]
                    crate::engine::api::profiling::record_owned_storage(
                        crate::engine::api::profiling::OwnedStorageEvent::Copy { heap_root: false },
                    );
                    slots.push(window, value)?;
                    true
                } else {
                    false
                }
            }
            Instruction::GetVarRef(index)
            | Instruction::GetVarRefCheck(index)
            | Instruction::PutVarRef(index)
            | Instruction::SetVarRef(index)
            | Instruction::PutVarRefCheck(index) => {
                return Ok(RunExit::Binding {
                    source: BindingSource::Closure,
                    index: *index,
                    write: matches!(
                        instruction,
                        Instruction::PutVarRef(_)
                            | Instruction::SetVarRef(_)
                            | Instruction::PutVarRefCheck(_)
                    ),
                    checked: matches!(
                        instruction,
                        Instruction::GetVarRefCheck(_) | Instruction::PutVarRefCheck(_)
                    ),
                    keep: matches!(instruction, Instruction::SetVarRef(_)),
                });
            }
            Instruction::GetLocal(index)
            | Instruction::GetLocalCheck(index)
            | Instruction::PutLocal(index)
            | Instruction::SetLocal(index)
            | Instruction::PutLocalCheck(index)
            | Instruction::SetLocalCheck(index)
                if matches!(slots.local(window, *index)?, FrameBinding::Captured(_)) =>
            {
                return Ok(RunExit::Binding {
                    source: BindingSource::Local,
                    index: *index,
                    write: !matches!(
                        instruction,
                        Instruction::GetLocal(_) | Instruction::GetLocalCheck(_)
                    ),
                    checked: matches!(
                        instruction,
                        Instruction::GetLocalCheck(_)
                            | Instruction::PutLocalCheck(_)
                            | Instruction::SetLocalCheck(_)
                    ),
                    keep: matches!(
                        instruction,
                        Instruction::SetLocal(_) | Instruction::SetLocalCheck(_)
                    ),
                });
            }
            Instruction::GetArg(index)
            | Instruction::PutArg(index)
            | Instruction::SetArg(index)
                if matches!(slots.parameter(window, *index)?, FrameBinding::Captured(_)) =>
            {
                return Ok(RunExit::Binding {
                    source: BindingSource::Argument,
                    index: *index,
                    write: !matches!(instruction, Instruction::GetArg(_)),
                    checked: false,
                    keep: matches!(instruction, Instruction::SetArg(_)),
                });
            }
            Instruction::InitializeLocal(index)
                if matches!(slots.local(window, *index)?, FrameBinding::Captured(_))
                    && frame.executable.local_definitions[usize::from(*index)].kind
                        == crate::engine::code::function::metadata::ClosureVariableKind::Normal =>
            {
                return Ok(RunExit::Binding {
                    source: BindingSource::Local,
                    index: *index,
                    write: true,
                    checked: false,
                    keep: false,
                });
            }
            Instruction::GetLocal(index) | Instruction::GetLocalCheck(index) => {
                if let FrameBinding::Direct(value) = slots.local(window, *index)? {
                    let copied = copy_value(value)?;
                    slots.push(window, copied)?;
                    true
                } else if matches!(instruction, Instruction::GetLocalCheck(_))
                    && matches!(slots.local(window, *index)?, FrameBinding::Uninitialized)
                {
                    return Ok(RunExit::LexicalUninitialized(*index));
                } else {
                    false
                }
            }
            Instruction::ThrowReadOnly(index) | Instruction::ThrowRedeclaration(index) => {
                return Ok(RunExit::BindingError {
                    index: *index,
                    redeclaration: matches!(instruction, Instruction::ThrowRedeclaration(_)),
                });
            }
            Instruction::InitializePrivateName(index)
            | Instruction::InitializePrivateMethod(index)
            | Instruction::InitializePrivateAccessor(index) => {
                use super::private_bindings::Initialization;
                let kind = match instruction {
                    Instruction::InitializePrivateName(_) => Initialization::Name,
                    Instruction::InitializePrivateMethod(_) => Initialization::Method,
                    _ => Initialization::Accessor,
                };
                return Ok(RunExit::PrivateInitialize {
                    index: *index,
                    kind,
                });
            }
            Instruction::GetPrivateField(source)
            | Instruction::GetPrivateField2(source)
            | Instruction::PutPrivateField(source)
            | Instruction::DefinePrivateField(source)
            | Instruction::PrivateIn(source) => {
                use super::private_access::Access;
                let access = match instruction {
                    Instruction::GetPrivateField(_) => Access::Get,
                    Instruction::GetPrivateField2(_) => Access::GetKeep,
                    Instruction::PutPrivateField(_) => Access::Put,
                    Instruction::DefinePrivateField(_) => Access::Define,
                    _ => Access::In,
                };
                return Ok(RunExit::PrivateAccess {
                    source: *source,
                    access,
                });
            }
            Instruction::Arguments(kind) => return Ok(RunExit::Arguments(*kind)),
            Instruction::Rest(start) => return Ok(RunExit::Rest(*start)),
            Instruction::SetName(index) => return Ok(RunExit::SetName(Some(*index))),
            Instruction::SetNameComputed => return Ok(RunExit::SetName(None)),
            Instruction::FClosure(index) => return Ok(RunExit::InstantiateClosure(*index)),
            Instruction::CloseLocal(index) => {
                // An uncaptured local keeps its value until the next scope entry.
                // Captured cells must first root and detach their shared value.
                if matches!(slots.local(window, *index)?, FrameBinding::Captured(_)) {
                    return Ok(RunExit::CloseCaptured(*index));
                } else {
                    frame.cold.reusable_captured_locals[usize::from(*index)] = false;
                    true
                }
            }
            Instruction::SetLocalUninitialized(index) => {
                if matches!(slots.local(window, *index)?, FrameBinding::Captured(_)) {
                    return Ok(RunExit::ResetCaptured(*index));
                }
                let ready = match slots.local(window, *index)? {
                    FrameBinding::Uninitialized => true,
                    FrameBinding::Direct(old) => {
                        runtime
                            .slot_value_release_readiness(old)
                            .map_err(runtime_error_to_vm_error)?
                            == SlotReleaseReadiness::Ready
                    }
                    _ => false,
                };
                if ready {
                    let old = slots.replace_local(window, *index, FrameBinding::Uninitialized)?;
                    frame.cold.reusable_captured_locals[usize::from(*index)] = false;
                    if matches!(old, FrameBinding::Direct(_)) {
                        release_displaced(runtime, old)?;
                    }
                }
                ready
            }
            Instruction::InitializeLocal(index) => {
                let definition = frame.executable.local_definitions[usize::from(*index)];
                if definition.kind
                    == crate::engine::code::function::metadata::ClosureVariableKind::WithObject
                {
                    return Ok(RunExit::Environment(
                        super::environment_driver::Operation::InitializeWith(*index),
                    ));
                }
                let ready = definition.is_lexical
                    && definition.kind
                        == crate::engine::code::function::metadata::ClosureVariableKind::Normal
                    && match slots.local(window, *index)? {
                        FrameBinding::Uninitialized => true,
                        FrameBinding::Direct(old) => {
                            runtime
                                .slot_value_release_readiness(old)
                                .map_err(runtime_error_to_vm_error)?
                                == SlotReleaseReadiness::Ready
                        }
                        _ => false,
                    };
                if ready {
                    let next = slots.pop(window)?;
                    let old = slots.replace_local(window, *index, FrameBinding::Direct(next))?;
                    if matches!(old, FrameBinding::Direct(_)) {
                        release_displaced(runtime, old)?;
                    }
                }
                ready
            }
            Instruction::PutLocalCheck(index) | Instruction::SetLocalCheck(index)
                if matches!(slots.local(window, *index)?, FrameBinding::Uninitialized) =>
            {
                return Ok(RunExit::LexicalUninitialized(*index));
            }
            Instruction::PutLocal(index)
            | Instruction::SetLocal(index)
            | Instruction::PutLocalCheck(index)
            | Instruction::SetLocalCheck(index) => {
                if matches!(slots.local(window, *index)?, FrameBinding::Direct(old) if runtime.slot_value_release_readiness(old).map_err(runtime_error_to_vm_error)? == SlotReleaseReadiness::Ready)
                {
                    let next = if matches!(
                        instruction,
                        Instruction::SetLocal(_) | Instruction::SetLocalCheck(_)
                    ) {
                        copy_value(slots.peek(window, 0)?)?
                    } else {
                        slots.pop(window)?
                    };
                    let old = slots.replace_local(window, *index, FrameBinding::Direct(next))?;
                    release_displaced(runtime, old)?;
                    true
                } else {
                    false
                }
            }
            Instruction::GetArg(index) => {
                if let FrameBinding::Direct(value) = slots.parameter(window, *index)? {
                    let copied = copy_value(value)?;
                    slots.push(window, copied)?;
                    true
                } else {
                    false
                }
            }
            Instruction::PutArg(index) | Instruction::SetArg(index) => {
                if matches!(slots.parameter(window, *index)?, FrameBinding::Direct(old) if runtime.slot_value_release_readiness(old).map_err(runtime_error_to_vm_error)? == SlotReleaseReadiness::Ready)
                {
                    let next = if matches!(instruction, Instruction::SetArg(_)) {
                        copy_value(slots.peek(window, 0)?)?
                    } else {
                        slots.pop(window)?
                    };
                    let old =
                        slots.replace_parameter(window, *index, FrameBinding::Direct(next))?;
                    release_displaced(runtime, old)?;
                    true
                } else {
                    false
                }
            }
            Instruction::Dup => {
                slots.insert_copy(window, 0, 0)?;
                true
            }
            Instruction::Dup1 => {
                slots.insert_copy(window, 1, 1)?;
                true
            }
            Instruction::Dup3 => {
                slots.duplicate_operands(window, 3)?;
                true
            }
            Instruction::Insert2 | Instruction::Insert3 | Instruction::Insert4 => {
                let count = match instruction {
                    Instruction::Insert2 => 2,
                    Instruction::Insert3 => 3,
                    _ => 4,
                };
                slots.peek(window, count - 1)?;
                slots.insert_copy(window, 0, count)?;
                true
            }
            Instruction::Perm3 | Instruction::Perm4 | Instruction::Perm5 => {
                let count = match instruction {
                    Instruction::Perm3 => 2,
                    Instruction::Perm4 => 3,
                    _ => 4,
                };
                slots.rotate_operands(window, 1, count, false)?;
                true
            }
            Instruction::Rot4Left => {
                slots.rotate_operands(window, 0, 4, true)?;
                true
            }
            Instruction::Drop => {
                if slots.release_operand(window, 0, runtime)? {
                    slots.pop(window)?;
                    true
                } else {
                    false
                }
            }
            Instruction::Swap => {
                slots.rotate_operands(window, 0, 2, false)?;
                true
            }
            Instruction::Nip => {
                if slots.release_operand(window, 1, runtime)? {
                    let right = slots.pop(window)?;
                    slots.pop(window)?;
                    slots.push(window, right)?;
                    true
                } else {
                    false
                }
            }
            Instruction::Add => {
                if binary(slots, window, |a, b| value(a.add(b)))? {
                    true
                } else {
                    return Ok(RunExit::ConvertAdd);
                }
            }
            Instruction::Sub => binary(slots, window, |a, b| value(a.sub(b)))?,
            Instruction::Mul => binary(slots, window, |a, b| value(a.mul(b)))?,
            Instruction::Div => binary(slots, window, |a, b| value(a.div(b)))?,
            Instruction::Mod => binary(slots, window, |a, b| value(a.rem(b)))?,
            Instruction::Pow => binary(slots, window, |a, b| value(a.pow(b)))?,
            Instruction::Shl => binary(slots, window, |a, b| {
                Value::Int(a.int32().wrapping_shl(b.int32() as u32 & 31))
            })?,
            Instruction::Sar => binary(slots, window, |a, b| {
                Value::Int(a.int32() >> (b.int32() as u32 & 31))
            })?,
            Instruction::Shr => binary(slots, window, |a, b| {
                value(Number::compact(f64::from(
                    (a.int32() as u32) >> (b.int32() as u32 & 31),
                )))
            })?,
            Instruction::BitAnd => binary(slots, window, |a, b| Value::Int(a.int32() & b.int32()))?,
            Instruction::BitOr => binary(slots, window, |a, b| Value::Int(a.int32() | b.int32()))?,
            Instruction::BitXor => binary(slots, window, |a, b| Value::Int(a.int32() ^ b.int32()))?,
            Instruction::Lt => binary(slots, window, |a, b| Value::Bool(a.float() < b.float()))?,
            Instruction::Lte => binary(slots, window, |a, b| Value::Bool(a.float() <= b.float()))?,
            Instruction::Gt => binary(slots, window, |a, b| Value::Bool(a.float() > b.float()))?,
            Instruction::Gte => binary(slots, window, |a, b| Value::Bool(a.float() >= b.float()))?,
            Instruction::StrictEq | Instruction::StrictNeq => {
                let negate = matches!(instruction, Instruction::StrictNeq);
                if binary(slots, window, |a, b| {
                    Value::Bool((a.float() == b.float()) != negate)
                })? {
                    true
                } else {
                    return Ok(RunExit::StrictEquality(negate));
                }
            }
            Instruction::Eq => binary(slots, window, |a, b| Value::Bool(a.float() == b.float()))?,
            Instruction::Neq => binary(slots, window, |a, b| Value::Bool(a.float() != b.float()))?,
            Instruction::Neg
            | Instruction::Plus
            | Instruction::BitNot
            | Instruction::Inc
            | Instruction::Dec
            | Instruction::PostInc
            | Instruction::PostDec => {
                if let Some(old) = number(slots.peek(window, 0)?) {
                    let next = match instruction {
                        Instruction::Neg => old.negate(),
                        Instruction::Plus => old,
                        Instruction::BitNot => Number::Int(!old.int32()),
                        _ => old.update(matches!(
                            instruction,
                            Instruction::Inc | Instruction::PostInc
                        )),
                    };
                    if !matches!(instruction, Instruction::PostInc | Instruction::PostDec) {
                        slots.pop(window)?;
                    }
                    slots.push(window, value(next))?;
                    true
                } else if matches!(instruction, Instruction::Plus) {
                    return Ok(RunExit::ConvertPlus);
                } else {
                    false
                }
            }
            Instruction::Goto(target) => {
                next_pc = *target as usize;
                true
            }
            Instruction::IfTrue(target) | Instruction::IfFalse(target)
                if immediate(slots.peek(window, 0)?) =>
            {
                let truthy = slots.pop(window)?.to_boolean_primitive();
                if truthy == matches!(instruction, Instruction::IfTrue(_)) {
                    next_pc = *target as usize;
                }
                true
            }
            Instruction::Catch(target) => return Ok(RunExit::Catch(*target)),
            Instruction::DropCatch => return Ok(RunExit::DropCatch),
            Instruction::NipCatch => return Ok(RunExit::NipCatch),
            Instruction::Throw => return Ok(RunExit::Throw),
            Instruction::Gosub(target) => {
                let pc = i32::try_from(next_pc)
                    .map_err(|_| Error::internal("gosub return PC does not fit Int"))?;
                slots.push(window, Value::Int(pc))?;
                next_pc = *target as usize;
                true
            }
            Instruction::Ret => {
                let Value::Int(target) = slots.pop(window)? else {
                    return Err(Error::internal("invalid ret value"));
                };
                next_pc =
                    usize::try_from(target).map_err(|_| Error::internal("invalid ret value"))?;
                if next_pc >= frame.executable.code.len() {
                    return Err(Error::internal("invalid ret value"));
                }
                true
            }
            Instruction::DropGosub => {
                if !matches!(slots.pop(window)?, Value::Int(_)) {
                    return Err(Error::internal("invalid gosub cleanup value"));
                }
                true
            }
            Instruction::Return => {
                execution.pending = Some(slots.pop(window)?);
                frame.resume_pc = next_pc;
                #[cfg(feature = "profiling")]
                crate::engine::api::profiling::record_owned_instruction(observed_depth);
                return Ok(RunExit::Complete);
            }
            Instruction::ReturnUndefined => {
                execution.pending = Some(Value::Undefined);
                frame.resume_pc = next_pc;
                #[cfg(feature = "profiling")]
                crate::engine::api::profiling::record_owned_instruction(observed_depth);
                return Ok(RunExit::Complete);
            }
            _ => false,
        };
        if !handled {
            return Ok(RunExit::Bridge);
        }
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_instruction(observed_depth);
        frame.resume_pc = next_pc;
    }
}

#[cfg(all(test, feature = "profiling"))]
mod tests {
    use crate::engine::api::profiling::CostProfile;
    use crate::engine::api::{Runtime, Value};
    use crate::engine::heap::SlotReleaseReadiness;

    #[test]
    fn ordinary_recursion_uses_one_execution_on_a_two_mib_native_stack() {
        std::thread::Builder::new()
            .stack_size(2 * 1024 * 1024)
            .spawn(|| {
                let runtime = Runtime::new();
                let mut context = runtime.new_context();
                let Value::Object(function) = context
                    .eval("(function f(n){if(n===0)return 0;return 1+f(n-1)})")
                    .unwrap()
                else {
                    panic!("expected function");
                };
                let callable = runtime.as_callable(&function).unwrap().unwrap();
                let profile = CostProfile::start();
                assert_eq!(
                    context
                        .call(&callable, Value::Undefined, &[Value::Int(1000)])
                        .unwrap(),
                    Value::Int(1000)
                );
                let costs = profile.snapshot();
                assert_eq!(costs.owned_storage.maximum_frame_depth, 1001, "{costs:?}");
                assert_eq!(costs.owned_storage.frames_pushed, 1001, "{costs:?}");
                assert_eq!(costs.legacy_dispatches, 0, "{costs:?}");
                assert_eq!(costs.owned_bridge_exits, 0, "{costs:?}");
                assert!(runtime.0.state.borrow().active_frames.is_empty());
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn tail_calls_return_through_owned_parents() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let Value::Object(function) = context
            .eval("(function f(n){if(n===0)return 7;return f(n-1)})")
            .unwrap()
        else {
            panic!("expected function");
        };
        let callable = runtime.as_callable(&function).unwrap().unwrap();
        let profile = CostProfile::start();
        assert_eq!(
            context
                .call(&callable, Value::Undefined, &[Value::Int(64)])
                .unwrap(),
            Value::Int(7)
        );
        let costs = profile.snapshot();
        assert_eq!(costs.owned_storage.maximum_frame_depth, 65);
        assert_eq!(costs.legacy_dispatches, 0);
        assert_eq!(costs.owned_bridge_exits, 0);
        assert!(runtime.0.state.borrow().active_frames.is_empty());
    }

    #[test]
    fn child_legacy_throw_propagates_through_owned_parent_once() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let profile = CostProfile::start();
        let result = context.eval("var calls=0;function outer(f){return 1+f()} function inner(){calls++; 'x' in {};throw 42} var result;try{outer(inner)}catch(e){result=e} result*10+calls").unwrap();
        assert_eq!(result, Value::Int(421));
        let costs = profile.snapshot();
        assert_eq!(costs.owned_storage.maximum_frame_depth, 3, "{costs:?}");
        assert!(costs.owned_bridge_exits > 0, "{costs:?}");
        assert!(costs.legacy_dispatches > 0, "{costs:?}");
        assert!(runtime.0.state.borrow().active_frames.is_empty());
    }

    #[test]
    fn ordinary_loop_finishes_in_the_owned_core() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let Value::Object(function) = context
            .eval("(function(n) { var s=0; for(var i=0;i<n;i++) s=s+i; return s; })")
            .unwrap()
        else {
            panic!("expected function");
        };
        let callable = runtime.as_callable(&function).unwrap().unwrap();
        let profile = CostProfile::start();
        let result = context
            .call(&callable, Value::Undefined, &[Value::Int(100)])
            .unwrap();
        assert_eq!(result, Value::Int(4950));
        let costs = profile.snapshot();
        assert!(costs.owned_instructions > 1000, "{costs:?}");
        assert_eq!(
            costs.owned_bridge_exits, 0,
            "the ordinary numeric loop must not use the bridge"
        );
        assert_eq!(
            costs.legacy_dispatches, 0,
            "the measured call must finish entirely in the owned core"
        );
    }

    #[test]
    fn shared_object_binding_replacement_finishes_without_a_bridge() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let Value::Object(function) = context
            .eval("(function(a){var x=a;x=0;a=0;return 1})")
            .unwrap()
        else {
            panic!("expected function");
        };
        let callable = runtime.as_callable(&function).unwrap().unwrap();
        let root = runtime.new_object(None).unwrap();
        let argument = Value::Object(root.try_clone().unwrap());
        let profile = CostProfile::start();
        assert_eq!(
            context
                .call(&callable, Value::Undefined, &[argument])
                .unwrap(),
            Value::Int(1)
        );
        let costs = profile.snapshot();
        assert!(costs.owned_instructions > 0, "{costs:?}");
        assert_eq!(costs.owned_bridge_exits, 0, "{costs:?}");
        assert_eq!(costs.legacy_dispatches, 0, "{costs:?}");
        assert_eq!(costs.owned_storage.frames_pushed, 1);
        assert!(costs.owned_storage.copied_heap_roots > 0);
        assert!(costs.owned_storage.hot_heap_root_releases > 0);
        assert!(
            matches!(
                runtime
                    .slot_value_release_readiness(&Value::Object(root))
                    .unwrap(),
                SlotReleaseReadiness::QueueCapacity | SlotReleaseReadiness::Drain
            ),
            "the completed call must leave no extra root for its object argument"
        );
    }

    #[test]
    fn numeric_edges_use_the_owned_core() {
        for (expression, expected) in [
            ("2147483647+1", 2147483648.0_f64),
            ("-2147483648-1", -2147483649.0),
            ("0*-1", -0.0),
            ("1/0", f64::INFINITY),
            ("-0%3", -0.0),
            ("(-1)**(1/0)", f64::NAN),
            ("-1>>>0", 4294967295.0),
            ("1<<33", 2.0),
        ] {
            let runtime = Runtime::new();
            let mut context = runtime.new_context();
            let Value::Object(function) = context
                .eval(&format!("(function(){{return {expression}}})"))
                .unwrap()
            else {
                panic!("expected function");
            };
            let callable = runtime.as_callable(&function).unwrap().unwrap();
            let profile = CostProfile::start();
            let result = context
                .call(&callable, Value::Undefined, &[])
                .unwrap()
                .as_number()
                .unwrap();
            if expected.is_nan() {
                assert!(result.is_nan());
            } else {
                assert_eq!(result.to_bits(), expected.to_bits(), "{expression}");
            }
            assert!(profile.snapshot().owned_instructions > 0, "{expression}");
            assert_eq!(profile.snapshot().owned_bridge_exits, 0, "{expression}");
            assert_eq!(profile.snapshot().legacy_dispatches, 0, "{expression}");
        }
    }

    #[test]
    fn shared_primitive_literals_return_without_a_bridge() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        for (expression, expected) in [
            (
                "'owned literal'",
                Value::String(crate::engine::value::JsString::from_static("owned literal")),
            ),
            (
                "170141183460469231731687303715884105727n",
                Value::BigInt(i128::MAX.into()),
            ),
        ] {
            let Value::Object(function) = context
                .eval(&format!("(function(){{return {expression}}})"))
                .unwrap()
            else {
                panic!("expected function");
            };
            let callable = runtime.as_callable(&function).unwrap().unwrap();
            let profile = CostProfile::start();
            let result = context.call(&callable, Value::Undefined, &[]).unwrap();
            let costs = profile.snapshot();
            assert_eq!(costs.owned_bridge_exits, 0, "{costs:?}");
            assert_eq!(costs.legacy_dispatches, 0, "{costs:?}");
            assert!(costs.owned_instructions > 0, "{costs:?}");
            drop(callable);
            drop(function);
            assert_eq!(result, expected);
        }
    }

    #[test]
    fn string_and_bigint_addition_use_owned_conversion() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let Value::Object(function) = context.eval("(function(a,b){return a+b})").unwrap() else {
            panic!("expected function");
        };
        let callable = runtime.as_callable(&function).unwrap().unwrap();
        for (arguments, expected) in [
            (
                [
                    Value::String(crate::engine::value::JsString::from_static("1")),
                    Value::Int(2),
                ],
                Value::String(crate::engine::value::JsString::from_static("12")),
            ),
            (
                [Value::BigInt(1.into()), Value::BigInt(2.into())],
                Value::BigInt(3.into()),
            ),
        ] {
            let profile = CostProfile::start();
            assert_eq!(
                context
                    .call(&callable, Value::Undefined, &arguments)
                    .unwrap(),
                expected
            );
            let costs = profile.snapshot();
            assert!(costs.owned_instructions > 0, "{costs:?}");
            assert_eq!(costs.owned_bridge_exits, 0, "{costs:?}");
            assert_eq!(costs.legacy_dispatches, 0, "{costs:?}");
        }
    }

    #[test]
    fn cold_conversion_handoff_preserves_operands_pc_and_one_callback() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let profile = CostProfile::start();
        let result = context.eval_with_filename("var calls=0; function f(a){var x=3;return (x+a)+(x=2)} var v=f({valueOf(){calls++; 'x' in {};return 4}}); v*10+calls", "owned-handoff.js").unwrap();
        assert_eq!(result, Value::Int(91));
        let costs = profile.snapshot();
        assert!(costs.owned_instructions > 0);
        assert!(costs.owned_bridge_exits > 0);
        assert!(costs.legacy_dispatches > 0);
    }
}

/// Strict equality has no user conversion. Retain both operands until the
/// result is known, then release them outside the resident instruction match.
#[inline(never)]
pub(super) fn strict_comparison(
    execution: &mut RunningExecution,
    id: FrameId,
    negate: bool,
) -> Result<(), Error> {
    let frame = execution.frames.current_mut(id)?;
    #[cfg(feature = "profiling")]
    let depth = execution.slots.depth(&frame.window);
    let right = execution.slots.pop(&mut frame.window)?;
    let left = execution.slots.pop(&mut frame.window)?;
    let equal = left.strict_equal(&right);
    execution
        .slots
        .push(&mut frame.window, Value::Bool(equal != negate))?;
    frame.resume_pc = frame
        .fault_pc
        .checked_add(1)
        .ok_or_else(|| Error::internal("comparison resume PC overflow"))?;
    #[cfg(feature = "profiling")]
    crate::engine::api::profiling::record_owned_instruction(depth);
    Ok(())
}
