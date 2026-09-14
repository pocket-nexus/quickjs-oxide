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
use crate::engine::vm::stack::{RunSlots, copy_value};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum BindingSource {
    Closure,
    Local,
    Argument,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RunExit {
    Import,
    Pure(super::pure_operations::PureOperation),
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
    SetProperty(Option<u32>),
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
    Predicate(super::predicate_driver::Kind),
    HomeObject,
    SuperProperty(super::super_property_driver::Kind),
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
    Numeric(super::numeric::operation::NumericKind),
    ForIn(bool),
    LogicalNot,
    CopyData {
        target: usize,
        source: usize,
        excluded: Option<usize>,
    },
    ReplaceBinding {
        source: BindingSource,
        index: u16,
        keep: bool,
        uninitialized: bool,
    },
    ReleaseOperand {
        keep_top: bool,
    },
    Complete,
    Suspend(super::VmSuspendKind),
    Bridge,
}

#[cfg(feature = "profiling")]
impl RunExit {
    pub(super) fn diagnostic_name(self) -> &'static str {
        match self {
            Self::Import => "run_exit.Import",
            Self::Pure(..) => "run_exit.Pure",
            Self::ApplyEval(..) => "run_exit.ApplyEval",
            Self::Apply(..) => "run_exit.Apply",
            Self::Eval { .. } => "run_exit.Eval",
            Self::Call { .. } => "run_exit.Call",
            Self::SetProperty(..) => "run_exit.SetProperty",
            Self::GetField { .. } => "run_exit.GetField",
            Self::GetElement { .. } => "run_exit.GetElement",
            Self::InitializeDerived(..) => "run_exit.InitializeDerived",
            Self::LexicalUninitialized(..) => "run_exit.LexicalUninitialized",
            Self::Binding { .. } => "run_exit.Binding",
            Self::ClassInitializer(..) => "run_exit.ClassInitializer",
            Self::DefineClass { .. } => "run_exit.DefineClass",
            Self::DefineProperty { .. } => "run_exit.DefineProperty",
            Self::Environment(..) => "run_exit.Environment",
            Self::GetSuper => "run_exit.GetSuper",
            Self::Predicate(..) => "run_exit.Predicate",
            Self::HomeObject => "run_exit.HomeObject",
            Self::SuperProperty(..) => "run_exit.SuperProperty",
            Self::ReturnDerived(..) => "run_exit.ReturnDerived",
            Self::InitDerivedConstructor => "run_exit.InitDerivedConstructor",
            Self::Construct(..) => "run_exit.Construct",
            Self::ConvertAdd => "run_exit.ConvertAdd",
            Self::ConvertPlus => "run_exit.ConvertPlus",
            Self::ConvertPropertyKey => "run_exit.ConvertPropertyKey",
            Self::NormalizeThis => "run_exit.NormalizeThis",
            Self::Arguments(..) => "run_exit.Arguments",
            Self::Rest(..) => "run_exit.Rest",
            Self::InstantiateClosure(..) => "run_exit.InstantiateClosure",
            Self::SetName(..) => "run_exit.SetName",
            Self::CloseCaptured(..) => "run_exit.CloseCaptured",
            Self::ResetCaptured(..) => "run_exit.ResetCaptured",
            Self::Catch(..) => "run_exit.Catch",
            Self::DropCatch => "run_exit.DropCatch",
            Self::NipCatch => "run_exit.NipCatch",
            Self::Throw => "run_exit.Throw",
            Self::BindingError { .. } => "run_exit.BindingError",
            Self::PrivateInitialize { .. } => "run_exit.PrivateInitialize",
            Self::PrivateAccess { .. } => "run_exit.PrivateAccess",
            Self::StrictEquality(..) => "run_exit.StrictEquality",
            Self::Numeric(..) => "run_exit.Numeric",
            Self::ForIn(..) => "run_exit.ForIn",
            Self::LogicalNot => "run_exit.LogicalNot",
            Self::CopyData { .. } => "run_exit.CopyData",
            Self::ReplaceBinding { .. } => "run_exit.ReplaceBinding",
            Self::ReleaseOperand { .. } => "run_exit.ReleaseOperand",
            Self::Complete => "run_exit.Complete",
            Self::Suspend(..) => "run_exit.Suspend",
            Self::Bridge => "run_exit.Bridge",
        }
    }
}

mod program_counter;
use program_counter::ProgramCounter;

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
    slots: &mut RunSlots<'_>,
    operation: impl FnOnce(Number, Number) -> Value,
) -> Result<bool, Error> {
    slots.binary_number(operation)
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
    let mut slots = execution.slots.run_window(&mut frame.window)?;
    let runtime = frame.cold.function.runtime();
    let mut pc = ProgramCounter::new(&mut frame.fault_pc, &mut frame.resume_pc);
    loop {
        pc.fault = pc.resume;
        let instruction = frame
            .executable
            .code
            .get(pc.fault)
            .ok_or_else(|| Error::internal("owned bytecode ended without return"))?;
        #[cfg(feature = "profiling")]
        let observed_depth = slots.depth();
        let mut next_pc = pc
            .fault
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
                slots.push(value)?;
                true
            }
            Instruction::PutField(index) => return Ok(RunExit::SetProperty(Some(*index))),
            Instruction::PutArrayEl => return Ok(RunExit::SetProperty(None)),
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
                slots.push(copy_value(&frame.cold.input.new_target)?)?;
                true
            }
            Instruction::InitializeDerivedLocal(index) => {
                return Ok(RunExit::InitializeDerived(*index));
            }
            Instruction::CopyDataProperties => {
                return Ok(RunExit::CopyData {
                    target: 1,
                    source: 0,
                    excluded: None,
                });
            }
            Instruction::CopyDataPropertiesExcluded {
                target_depth,
                source_depth,
                excluded_depth,
            } => {
                return Ok(RunExit::CopyData {
                    target: usize::from(*target_depth),
                    source: usize::from(*source_depth),
                    excluded: Some(usize::from(*excluded_depth)),
                });
            }
            Instruction::InstanceOf => {
                return Ok(RunExit::Predicate(super::predicate_driver::Kind::Instance));
            }
            Instruction::In => return Ok(RunExit::Predicate(super::predicate_driver::Kind::Has)),
            Instruction::Delete => {
                return Ok(RunExit::Predicate(super::predicate_driver::Kind::Delete));
            }
            Instruction::GetSuper => return Ok(RunExit::GetSuper),
            Instruction::PushHomeObject => return Ok(RunExit::HomeObject),
            Instruction::GetSuperValue => {
                return Ok(RunExit::SuperProperty(
                    super::super_property_driver::Kind::Read,
                ));
            }
            Instruction::GetSuperValueForCall => {
                return Ok(RunExit::SuperProperty(
                    super::super_property_driver::Kind::Call,
                ));
            }
            Instruction::PutSuperValue => {
                return Ok(RunExit::SuperProperty(
                    super::super_property_driver::Kind::Write,
                ));
            }
            Instruction::ReturnDerived(index) => return Ok(RunExit::ReturnDerived(*index)),
            Instruction::CheckCtor if !matches!(frame.cold.input.new_target, Value::Undefined) => {
                true
            }
            Instruction::CheckCtor => {
                return Ok(RunExit::Pure(
                    super::pure_operations::PureOperation::ConstructorWithoutNew,
                ));
            }
            Instruction::PushActiveFunction => {
                slots.push(Value::Object(frame.cold.function.clone()))?;
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
            Instruction::ForInStart => return Ok(RunExit::ForIn(false)),
            Instruction::ForInNext => return Ok(RunExit::ForIn(true)),
            Instruction::IteratorStart
            | Instruction::AsyncIteratorStart
            | Instruction::ForAwaitOfStart
            | Instruction::ForAwaitOfNext
            | Instruction::IteratorNext
            | Instruction::IteratorCall(_)
            | Instruction::IteratorGetValueDone => {
                use super::iterator_driver::suspension::Operation;
                let operation = match instruction {
                    Instruction::IteratorStart => Operation::Start {
                        asynchronous: false,
                        delegating: true,
                    },
                    Instruction::AsyncIteratorStart => Operation::Start {
                        asynchronous: true,
                        delegating: true,
                    },
                    Instruction::ForAwaitOfStart => Operation::Start {
                        asynchronous: true,
                        delegating: false,
                    },
                    Instruction::ForAwaitOfNext => Operation::AwaitNext,
                    Instruction::IteratorNext => Operation::Next,
                    Instruction::IteratorCall(kind) => Operation::Call(*kind),
                    Instruction::IteratorGetValueDone => Operation::Parse,
                    _ => unreachable!(),
                };
                return Ok(RunExit::Environment(
                    super::environment_driver::Operation::Iterator(
                        super::iterator_driver::Operation::Suspend(operation),
                    ),
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
                if matches!(slots.peek(0)?, Value::Object(_)) {
                    true
                } else {
                    return Ok(RunExit::Environment(
                        super::environment_driver::Operation::ToObject,
                    ));
                }
            }
            Instruction::ToPropKey => match slots.peek(0)? {
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
            Instruction::PushAtomValueIndex(value) => {
                return Ok(RunExit::Pure(
                    super::pure_operations::PureOperation::AtomValue(*value),
                ));
            }
            Instruction::RegExp(index) => {
                return Ok(RunExit::Pure(
                    super::pure_operations::PureOperation::RegExp(*index),
                ));
            }
            Instruction::ThrowDeleteSuper => {
                return Ok(RunExit::Pure(
                    super::pure_operations::PureOperation::DeleteSuper,
                ));
            }
            Instruction::Import => return Ok(RunExit::Import),
            Instruction::InitializeModuleImportCollision(index) => {
                return Ok(RunExit::Pure(
                    super::pure_operations::PureOperation::InitializeModuleImportCollision(*index),
                ));
            }
            Instruction::InitializeVarRef(index) | Instruction::InitializeDerivedVarRef(index) => {
                return Ok(RunExit::Pure(
                    super::pure_operations::PureOperation::InitializeClosure {
                        index: *index,
                        derived: matches!(instruction, Instruction::InitializeDerivedVarRef(_)),
                    },
                ));
            }
            Instruction::SetProto => {
                return Ok(RunExit::Pure(
                    super::pure_operations::PureOperation::SetPrototype,
                ));
            }
            Instruction::IteratorCheckObject => {
                return Ok(RunExit::Pure(
                    super::pure_operations::PureOperation::IteratorCheckObject,
                ));
            }
            Instruction::ThrowIteratorMissingThrow => {
                return Ok(RunExit::Pure(
                    super::pure_operations::PureOperation::IteratorMissingThrow,
                ));
            }
            Instruction::TypeOf
            | Instruction::IsUndefinedOrNull
            | Instruction::IsUndefined
            | Instruction::IsNull
            | Instruction::TypeOfIsUndefined
            | Instruction::TypeOfIsFunction => {
                use super::pure_operations::PureOperation as P;
                let kind = match instruction {
                    Instruction::TypeOf => P::TypeOf,
                    Instruction::IsUndefinedOrNull => P::IsUndefinedOrNull,
                    Instruction::IsUndefined => P::IsUndefined,
                    Instruction::IsNull => P::IsNull,
                    Instruction::TypeOfIsUndefined => P::TypeOfIsUndefined,
                    _ => P::TypeOfIsFunction,
                };
                return Ok(RunExit::Pure(kind));
            }
            Instruction::Nop | Instruction::MarkSuperCall => true,
            Instruction::PushI32(number) => {
                slots.push(Value::Int(*number))?;
                true
            }
            Instruction::Undefined => {
                slots.push(Value::Undefined)?;
                true
            }
            Instruction::Null => {
                slots.push(Value::Null)?;
                true
            }
            Instruction::PushTrue => {
                slots.push(Value::Bool(true))?;
                true
            }
            Instruction::PushFalse => {
                slots.push(Value::Bool(false))?;
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
                    slots.push(value)?;
                    true
                } else {
                    return Ok(RunExit::Pure(
                        super::pure_operations::PureOperation::Constant(*index),
                    ));
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
                if matches!(slots.local(*index)?, FrameBinding::Captured(_)) =>
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
                if matches!(slots.parameter(*index)?, FrameBinding::Captured(_)) =>
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
                if matches!(slots.local(*index)?, FrameBinding::Captured(_))
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
                if let FrameBinding::Direct(value) = slots.local(*index)? {
                    let copied = copy_value(value)?;
                    slots.push(copied)?;
                    true
                } else if matches!(instruction, Instruction::GetLocalCheck(_))
                    && matches!(slots.local(*index)?, FrameBinding::Uninitialized)
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
                if matches!(slots.local(*index)?, FrameBinding::Captured(_)) {
                    return Ok(RunExit::CloseCaptured(*index));
                } else {
                    frame.cold.reusable_captured_locals[usize::from(*index)] = false;
                    true
                }
            }
            Instruction::SetLocalUninitialized(index) => {
                if matches!(slots.local(*index)?, FrameBinding::Captured(_)) {
                    return Ok(RunExit::ResetCaptured(*index));
                }
                let ready = match slots.local(*index)? {
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
                    let old = slots.replace_local(*index, FrameBinding::Uninitialized)?;
                    frame.cold.reusable_captured_locals[usize::from(*index)] = false;
                    if matches!(old, FrameBinding::Direct(_)) {
                        release_displaced(runtime, old)?;
                    }
                }
                if !ready {
                    return Ok(RunExit::ReplaceBinding {
                        source: BindingSource::Local,
                        index: *index,
                        keep: false,
                        uninitialized: true,
                    });
                }
                true
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
                    && match slots.local(*index)? {
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
                    let next = slots.pop()?;
                    let old = slots.replace_local(*index, FrameBinding::Direct(next))?;
                    if matches!(old, FrameBinding::Direct(_)) {
                        release_displaced(runtime, old)?;
                    }
                }
                if !ready
                    && definition.is_lexical
                    && definition.kind
                        == crate::engine::code::function::metadata::ClosureVariableKind::Normal
                    && matches!(slots.local(*index)?, FrameBinding::Direct(_))
                {
                    return Ok(RunExit::ReplaceBinding {
                        source: BindingSource::Local,
                        index: *index,
                        keep: false,
                        uninitialized: false,
                    });
                }
                ready
            }
            Instruction::PutLocalCheck(index) | Instruction::SetLocalCheck(index)
                if matches!(slots.local(*index)?, FrameBinding::Uninitialized) =>
            {
                return Ok(RunExit::LexicalUninitialized(*index));
            }
            Instruction::PutLocal(index)
            | Instruction::SetLocal(index)
            | Instruction::PutLocalCheck(index)
            | Instruction::SetLocalCheck(index) => {
                if matches!(slots.local(*index)?, FrameBinding::Direct(old) if runtime.slot_value_release_readiness(old).map_err(runtime_error_to_vm_error)? == SlotReleaseReadiness::Ready)
                {
                    let next = if matches!(
                        instruction,
                        Instruction::SetLocal(_) | Instruction::SetLocalCheck(_)
                    ) {
                        copy_value(slots.peek(0)?)?
                    } else {
                        slots.pop()?
                    };
                    let old = slots.replace_local(*index, FrameBinding::Direct(next))?;
                    release_displaced(runtime, old)?;
                    true
                } else if matches!(slots.local(*index)?, FrameBinding::Direct(_)) {
                    return Ok(RunExit::ReplaceBinding {
                        source: BindingSource::Local,
                        index: *index,
                        keep: matches!(
                            instruction,
                            Instruction::SetLocal(_) | Instruction::SetLocalCheck(_)
                        ),
                        uninitialized: false,
                    });
                } else {
                    false
                }
            }
            Instruction::GetArg(index) => {
                if let FrameBinding::Direct(value) = slots.parameter(*index)? {
                    let copied = copy_value(value)?;
                    slots.push(copied)?;
                    true
                } else {
                    false
                }
            }
            Instruction::PutArg(index) | Instruction::SetArg(index) => {
                if matches!(slots.parameter(*index)?, FrameBinding::Direct(old) if runtime.slot_value_release_readiness(old).map_err(runtime_error_to_vm_error)? == SlotReleaseReadiness::Ready)
                {
                    let next = if matches!(instruction, Instruction::SetArg(_)) {
                        copy_value(slots.peek(0)?)?
                    } else {
                        slots.pop()?
                    };
                    let old = slots.replace_parameter(*index, FrameBinding::Direct(next))?;
                    release_displaced(runtime, old)?;
                    true
                } else if matches!(slots.parameter(*index)?, FrameBinding::Direct(_)) {
                    return Ok(RunExit::ReplaceBinding {
                        source: BindingSource::Argument,
                        index: *index,
                        keep: matches!(instruction, Instruction::SetArg(_)),
                        uninitialized: false,
                    });
                } else {
                    false
                }
            }
            Instruction::Dup => {
                slots.insert_copy(0, 0)?;
                true
            }
            Instruction::Dup1 => {
                slots.insert_copy(1, 1)?;
                true
            }
            Instruction::Dup3 => {
                slots.duplicate_operands(3)?;
                true
            }
            Instruction::Insert2 | Instruction::Insert3 | Instruction::Insert4 => {
                let count = match instruction {
                    Instruction::Insert2 => 2,
                    Instruction::Insert3 => 3,
                    _ => 4,
                };
                slots.peek(count - 1)?;
                slots.insert_copy(0, count)?;
                true
            }
            Instruction::Perm3 | Instruction::Perm4 | Instruction::Perm5 => {
                let count = match instruction {
                    Instruction::Perm3 => 2,
                    Instruction::Perm4 => 3,
                    _ => 4,
                };
                slots.rotate_operands(1, count, false)?;
                true
            }
            Instruction::Rot4Left => {
                slots.rotate_operands(0, 4, true)?;
                true
            }
            Instruction::Drop => {
                if slots.release_operand(0, runtime)? {
                    slots.pop()?;
                    true
                } else {
                    return Ok(RunExit::ReleaseOperand { keep_top: false });
                }
            }
            Instruction::Swap => {
                slots.rotate_operands(0, 2, false)?;
                true
            }
            Instruction::Nip => {
                if slots.release_operand(1, runtime)? {
                    let right = slots.pop()?;
                    slots.pop()?;
                    slots.push(right)?;
                    true
                } else {
                    return Ok(RunExit::ReleaseOperand { keep_top: true });
                }
            }
            Instruction::Add => {
                if binary(&mut slots, |a, b| value(a.add(b)))? {
                    true
                } else {
                    return Ok(RunExit::ConvertAdd);
                }
            }
            Instruction::Sub => binary(&mut slots, |a, b| value(a.sub(b)))?,
            Instruction::Mul => binary(&mut slots, |a, b| value(a.mul(b)))?,
            Instruction::Div => binary(&mut slots, |a, b| value(a.div(b)))?,
            Instruction::Mod => binary(&mut slots, |a, b| value(a.rem(b)))?,
            Instruction::Pow => binary(&mut slots, |a, b| value(a.pow(b)))?,
            Instruction::Shl => binary(&mut slots, |a, b| {
                Value::Int(a.int32().wrapping_shl(b.int32() as u32 & 31))
            })?,
            Instruction::Sar => binary(&mut slots, |a, b| {
                Value::Int(a.int32() >> (b.int32() as u32 & 31))
            })?,
            Instruction::Shr => binary(&mut slots, |a, b| {
                value(Number::compact(f64::from(
                    (a.int32() as u32) >> (b.int32() as u32 & 31),
                )))
            })?,
            Instruction::BitAnd => binary(&mut slots, |a, b| Value::Int(a.int32() & b.int32()))?,
            Instruction::BitOr => binary(&mut slots, |a, b| Value::Int(a.int32() | b.int32()))?,
            Instruction::BitXor => binary(&mut slots, |a, b| Value::Int(a.int32() ^ b.int32()))?,
            Instruction::Lt => binary(&mut slots, |a, b| Value::Bool(a.float() < b.float()))?,
            Instruction::Lte => binary(&mut slots, |a, b| Value::Bool(a.float() <= b.float()))?,
            Instruction::Gt => binary(&mut slots, |a, b| Value::Bool(a.float() > b.float()))?,
            Instruction::Gte => binary(&mut slots, |a, b| Value::Bool(a.float() >= b.float()))?,
            Instruction::StrictEq | Instruction::StrictNeq => {
                let negate = matches!(instruction, Instruction::StrictNeq);
                if binary(&mut slots, |a, b| {
                    Value::Bool((a.float() == b.float()) != negate)
                })? {
                    true
                } else {
                    return Ok(RunExit::StrictEquality(negate));
                }
            }
            Instruction::Eq => binary(&mut slots, |a, b| Value::Bool(a.float() == b.float()))?,
            Instruction::Neq => binary(&mut slots, |a, b| Value::Bool(a.float() != b.float()))?,
            Instruction::Not => return Ok(RunExit::LogicalNot),
            Instruction::Neg
            | Instruction::Plus
            | Instruction::BitNot
            | Instruction::Inc
            | Instruction::Dec
            | Instruction::PostInc
            | Instruction::PostDec => {
                if let Some(old) = number(slots.peek(0)?) {
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
                        slots.pop()?;
                    }
                    slots.push(value(next))?;
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
                if immediate(slots.peek(0)?) =>
            {
                let truthy = slots.pop()?.to_boolean_primitive();
                if truthy == matches!(instruction, Instruction::IfTrue(_)) {
                    next_pc = *target as usize;
                }
                true
            }
            Instruction::IfTrue(target) | Instruction::IfFalse(target) => {
                return Ok(RunExit::Pure(
                    super::pure_operations::PureOperation::Branch {
                        target: *target,
                        when: matches!(instruction, Instruction::IfTrue(_)),
                    },
                ));
            }
            Instruction::Catch(target) => return Ok(RunExit::Catch(*target)),
            Instruction::DropCatch => return Ok(RunExit::DropCatch),
            Instruction::NipCatch => return Ok(RunExit::NipCatch),
            Instruction::Throw => return Ok(RunExit::Throw),
            Instruction::Gosub(target) => {
                let pc = i32::try_from(next_pc)
                    .map_err(|_| Error::internal("gosub return PC does not fit Int"))?;
                slots.push(Value::Int(pc))?;
                next_pc = *target as usize;
                true
            }
            Instruction::Ret => {
                let Value::Int(target) = slots.pop()? else {
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
                if !matches!(slots.pop()?, Value::Int(_)) {
                    return Err(Error::internal("invalid gosub cleanup value"));
                }
                true
            }
            Instruction::InitialYield
            | Instruction::Yield
            | Instruction::YieldStar
            | Instruction::AsyncYieldStar
            | Instruction::Await => {
                let kind = match instruction {
                    Instruction::InitialYield => super::VmSuspendKind::Initial,
                    Instruction::Yield => super::VmSuspendKind::Yield,
                    Instruction::YieldStar => super::VmSuspendKind::YieldStar,
                    Instruction::AsyncYieldStar => super::VmSuspendKind::AsyncYieldStar,
                    Instruction::Await => super::VmSuspendKind::Await,
                    _ => unreachable!(),
                };
                if kind != super::VmSuspendKind::Initial {
                    slots.peek(0)?;
                }
                pc.resume = next_pc;
                #[cfg(feature = "profiling")]
                crate::engine::api::profiling::record_owned_instruction(observed_depth);
                return Ok(RunExit::Suspend(kind));
            }
            Instruction::Return => {
                execution.pending = Some(slots.pop()?);
                pc.resume = next_pc;
                #[cfg(feature = "profiling")]
                crate::engine::api::profiling::record_owned_instruction(observed_depth);
                return Ok(RunExit::Complete);
            }
            Instruction::ReturnUndefined => {
                execution.pending = Some(Value::Undefined);
                pc.resume = next_pc;
                #[cfg(feature = "profiling")]
                crate::engine::api::profiling::record_owned_instruction(observed_depth);
                return Ok(RunExit::Complete);
            }
        };
        if !handled {
            if let Some(kind) = super::numeric::operation::NumericKind::for_instruction(instruction)
            {
                return Ok(RunExit::Numeric(kind));
            }
            return Ok(RunExit::Bridge);
        }
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_instruction(observed_depth);
        pc.resume = next_pc;
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
    fn child_throw_propagates_through_owned_parent_once() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let profile = CostProfile::start();
        let result = context.eval("var calls=0;function outer(f){return 1+f()} function inner(){calls++; [] instanceof Array;throw 42} var result;try{outer(inner)}catch(e){result=e} result*10+calls").unwrap();
        assert_eq!(result, Value::Int(421));
        let costs = profile.snapshot();
        assert_eq!(costs.owned_storage.maximum_frame_depth, 3, "{costs:?}");
        assert_eq!(costs.owned_bridge_exits, 0, "{costs:?}");
        assert_eq!(costs.legacy_dispatches, 0, "{costs:?}");
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
        assert_eq!(costs.owned_execution_events["run_frame_fault_pc_write"], 1);
        assert_eq!(costs.owned_execution_events["run_frame_resume_pc_write"], 1);
        assert_eq!(costs.owned_execution_events["runtime_pc_publication"], 1);
        assert!(
            costs.owned_execution_events["slot_authentication"] < 20,
            "{costs:?}"
        );
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
    fn last_object_binding_replacement_releases_in_owned_cold_operations() {
        for source in [
            "(function(){var x=new ArrayBuffer(16);x=42;return x})",
            "(function(){var x=new ArrayBuffer(16);return x=42})",
            "(function(a){a=new ArrayBuffer(16);a=42;return a})",
            "(function(a){a=new ArrayBuffer(16);return a=42})",
            "(function(){for(var i=0;i<2;i++){let x=new ArrayBuffer(16);if(i===1)return 42}})",
            "(function(){var x={child:new ArrayBuffer(16)};x=42;return x})",
        ] {
            let runtime = Runtime::new();
            let mut context = runtime.new_context();
            let Value::Object(function) = context.eval(source).unwrap() else {
                panic!("function expected")
            };
            let callable = runtime.as_callable(&function).unwrap().unwrap();
            let profile = CostProfile::start();
            assert_eq!(
                context.call(&callable, Value::Undefined, &[]).unwrap(),
                Value::Int(42),
                "{source}"
            );
            let costs = profile.snapshot();
            assert_eq!(costs.owned_bridge_exits, 0, "{source}: {costs:?}");
            assert_eq!(costs.legacy_dispatches, 0, "{source}: {costs:?}");
            assert_eq!(costs.owned_sync_call_bridges, 0, "{source}: {costs:?}");
            assert!(runtime.0.state.borrow().active_frames.is_empty());
        }
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
    fn owned_conversion_preserves_operands_pc_and_one_callback() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let profile = CostProfile::start();
        let result = context.eval_with_filename("var calls=0; function f(a){var x=3;return (x+a)+(x=2)} var v=f({valueOf(){calls++; [] instanceof Array;return 4}}); v*10+calls", "owned-handoff.js").unwrap();
        assert_eq!(result, Value::Int(91));
        let costs = profile.snapshot();
        assert!(costs.owned_instructions > 0);
        assert_eq!(costs.owned_bridge_exits, 0);
        assert_eq!(costs.legacy_dispatches, 0);
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
