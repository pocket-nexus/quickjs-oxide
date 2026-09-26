//! The owned stack's one instruction match. Dynamic coercion and unimplemented
//! protocols exit before consuming their operands; S04–S07 replace that bridge.

use crate::engine::api::error::Error;
use crate::engine::code::bytecode::Instruction;
use crate::engine::code::fusion::{DirectSlot, LocalFusionChoice};
use crate::engine::heap::{BytecodeConstant, RawValue, SlotReleaseReadiness};
use crate::engine::value::JsValue;
use crate::engine::value::number::operations::Number;
use crate::engine::vm::bindings::FrameBinding;
use crate::engine::vm::exception::{heap_error_to_vm_error, runtime_error_to_vm_error};
use crate::engine::vm::execution::RunningExecution;
use crate::engine::vm::frame::FrameId;
use crate::engine::vm::stack::{RunSlots, copy_value};

// Stage B exit-transfer budget: every `RunExit` stays within one 16-byte
// transfer so the outlined driver bridge keeps its current call footprint.
// Recheck the stage B measurements before widening any variant.
const _: () = assert!(std::mem::size_of::<RunExit>() == 16);

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
    AddLocal,
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
    /// Owned arithmetic error in execution.pending; operands already consumed.
    PrimitiveThrow,
    Materialize,
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
    CopyData {
        target: u8,
        source: u8,
        excluded: Option<u8>,
    },
    #[cfg(all(test, feature = "profiling"))]
    ReleaseOperand {
        keep_top: bool,
    },
    Complete,
    Suspend(super::VmSuspendKind),
    Bridge,
}

impl RunExit {
    /// Whether this exit can expose the activation before the next run entry.
    pub(super) fn observes_activation(&self) -> bool {
        !matches!(self, Self::Call { .. } | Self::Complete)
    }

    #[cfg(feature = "profiling")]
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
            Self::AddLocal => "run_exit.AddLocal",
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
            Self::PrimitiveThrow => "run_exit.PrimitiveThrow",
            Self::Materialize => "run_exit.Materialize",
            Self::BindingError { .. } => "run_exit.BindingError",
            Self::PrivateInitialize { .. } => "run_exit.PrivateInitialize",
            Self::PrivateAccess { .. } => "run_exit.PrivateAccess",
            Self::StrictEquality(..) => "run_exit.StrictEquality",
            Self::Numeric(..) => "run_exit.Numeric",
            Self::ForIn(..) => "run_exit.ForIn",
            Self::CopyData { .. } => "run_exit.CopyData",
            #[cfg(all(test, feature = "profiling"))]
            Self::ReleaseOperand { .. } => "run_exit.ReleaseOperand",
            Self::Complete => "run_exit.Complete",
            Self::Suspend(..) => "run_exit.Suspend",
            Self::Bridge => "run_exit.Bridge",
        }
    }
}

mod cold;
mod fusion;
mod numeric;
mod program_counter;
mod property;
use program_counter::ProgramCounter;

#[cfg(test)]
pub(super) fn test_supported_numeric(
    slots: &RunSlots<'_>,
    kind: super::numeric::operation::NumericKind,
) -> bool {
    numeric::supported(slots, kind)
}

#[cfg(test)]
pub(super) fn test_complete_numeric(
    runtime: &crate::engine::api::runtime::Runtime,
    realm: crate::engine::heap::ContextId,
    transaction: &mut super::stack::FrameTransaction<'_>,
    kind: super::numeric::operation::NumericKind,
    thrown: &mut Option<JsValue>,
    active_frame: super::frames::ActiveFrameToken,
    fault_pc: usize,
) -> Result<bool, Error> {
    numeric::complete(
        runtime,
        realm,
        transaction,
        kind,
        thrown,
        active_frame,
        fault_pc,
    )
}

/// Values surrendered by the run loop's explicit outside-borrow releases.
trait ReleaseDropped {
    fn release_dropped(self, runtime: &crate::engine::api::runtime::Runtime) -> Result<(), Error>;
}

impl ReleaseDropped for JsValue {
    fn release_dropped(self, runtime: &crate::engine::api::runtime::Runtime) -> Result<(), Error> {
        runtime
            .release_jsvalue(self)
            .map_err(runtime_error_to_vm_error)
    }
}

impl ReleaseDropped for FrameBinding {
    fn release_dropped(self, runtime: &crate::engine::api::runtime::Runtime) -> Result<(), Error> {
        super::bindings::release_frame_binding(runtime, self)
    }
}

impl ReleaseDropped for (JsValue, JsValue) {
    fn release_dropped(self, runtime: &crate::engine::api::runtime::Runtime) -> Result<(), Error> {
        runtime
            .release_jsvalue(self.0)
            .map_err(runtime_error_to_vm_error)?;
        runtime
            .release_jsvalue(self.1)
            .map_err(runtime_error_to_vm_error)
    }
}

fn release_dropped(
    runtime: &crate::engine::api::runtime::Runtime,
    dropped: impl ReleaseDropped,
) -> Result<(), Error> {
    dropped.release_dropped(runtime)
}

fn number(value: &JsValue) -> Option<Number> {
    value.as_number_repr()
}
fn value(number: Number) -> JsValue {
    match number {
        Number::Int(value) => JsValue::Int(value),
        Number::Float(value) => JsValue::Float(value),
    }
}
fn immediate(value: &JsValue) -> bool {
    matches!(
        value,
        JsValue::Undefined
            | JsValue::Null
            | JsValue::Bool(_)
            | JsValue::Int(_)
            | JsValue::Float(_)
            | JsValue::ShortBigInt(_)
    )
}
fn binary(
    slots: &mut RunSlots<'_>,
    operation: impl FnOnce(Number, Number) -> JsValue,
) -> Result<bool, Error> {
    slots.binary_number(operation)
}

fn release_displaced(
    runtime: &crate::engine::api::runtime::Runtime,
    old: FrameBinding,
) -> Result<(), Error> {
    let FrameBinding::Direct(mut old) = old else {
        return Err(cold::internal(
            "non-direct binding passed a direct release preflight",
        ));
    };
    // The caller proved `Ready` in the same instruction handling. Between
    // that proof and this commit only moves and possibly one retain occurred;
    // neither can drain or invalidate the no-drain proof.
    runtime
        .release_slot_value_jsvalue_ready(&mut old)
        .map_err(runtime_error_to_vm_error)?;
    Ok(())
}

/// Only inline scalars can be discarded without touching runtime storage.
/// String and heap BigInt own arena edges, so their last release must follow
/// the same readiness/publication discipline as objects and symbols.
fn primitive_release_owner(value: &JsValue) -> bool {
    immediate(value)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DirectWriteClass {
    Number,
    Ready,
    NeedsBoundary,
    Other,
}

#[inline(always)]
fn direct_write_class(
    runtime: &crate::engine::api::runtime::Runtime,
    binding: &FrameBinding,
) -> Result<DirectWriteClass, Error> {
    match binding {
        FrameBinding::Direct(JsValue::Int(_) | JsValue::Float(_)) => Ok(DirectWriteClass::Number),
        FrameBinding::Direct(old) => {
            if runtime
                .slot_value_release_readiness_jsvalue(old)
                .map_err(runtime_error_to_vm_error)?
                == SlotReleaseReadiness::Ready
            {
                Ok(DirectWriteClass::Ready)
            } else {
                Ok(DirectWriteClass::NeedsBoundary)
            }
        }
        _ => Ok(DirectWriteClass::Other),
    }
}

/// Fused "binding read + linked field read": complete the following GetField
/// against a base object borrowed from a live binding (frame slot, this, or a
/// captured/global cell). The binding keeps the base alive and neither the IC
/// hit nor the immediate leaf read can execute JS, mutate the arena, or
/// release an owner, so the canonical retain/release round trip on a
/// temporary base owner is skipped entirely. Only the pushed property value
/// gains a new owner edge. `None` declines back to the canonical two
/// instruction pair, which also performs IC warm-up on misses.
#[inline]
fn borrowed_base_field_read(
    runtime: &crate::engine::api::runtime::Runtime,
    executable: &crate::engine::code::runtime::PublishedFunctionSnapshot,
    field_pc: usize,
    key: u32,
    base: &JsValue,
) -> Option<JsValue> {
    // `keep_receiver = true` is the exact contract of this fused read: the
    // borrowed base is never released afterwards, so the fast read must not
    // demand the base-release readiness that canonical droppable-base reads
    // pre-prove. The discarded native selection has no observable effect.
    let mut native = None;
    let value =
        runtime.property_ic_read_fast(base, executable, field_pc, key, true, &mut native)?;
    #[cfg(feature = "profiling")]
    cold::event("fusion.BorrowedBaseField");
    Some(value)
}

// Explicit drops end the NoJs slot borrow before publication or owner release.
#[allow(clippy::drop_non_drop)]
pub(super) fn run(execution: &mut RunningExecution, id: FrameId) -> Result<RunExit, Error> {
    let frame = execution.frames.current_mut(id)?;
    let body = &mut *frame.cold;
    let executable = &*body.executable;
    let mut transaction = execution.slots.frame_transaction(&mut body.window)?;
    let mut slots = transaction.slots();
    let cold = &mut body.owners;
    let runtime = cold.function.runtime();
    #[cfg(feature = "profiling")]
    crate::engine::api::profiling::record_fusion_static(runtime, executable);
    let mut pc = ProgramCounter::new(&mut frame.fault_pc, &mut frame.resume_pc);
    // Preserve the cold path's observation order, but keep this authenticated
    // frame resident. No slot borrow crosses active-PC publication or Drop.
    macro_rules! release_outside_slots {
        ($operation:expr) => {{
            if !frame.active_frame.is_materialized() {
                return Ok(RunExit::Materialize);
            }
            drop(slots);
            pc.publish_fault();
            runtime
                .update_active_bytecode_pc(frame.active_frame, super::BytecodePc::new(pc.fault))
                .map_err(runtime_error_to_vm_error)?;
            slots = transaction.slots();
            let released = $operation;
            drop(slots);
            release_dropped(runtime, released)?;
            slots = transaction.slots();
        }};
    }
    macro_rules! resident_property {
        ($operation:expr) => {{
            if !frame.active_frame.is_materialized() {
                return Ok(RunExit::Materialize);
            }
            drop(slots);
            pc.publish_fault();
            runtime
                .update_active_bytecode_pc(frame.active_frame, super::BytecodePc::new(pc.fault))
                .map_err(runtime_error_to_vm_error)?;
            let handled =
                property::complete(runtime, executable, pc.fault, &mut transaction, $operation)?;
            slots = transaction.slots();
            handled
        }};
    }
    'execute: loop {
        pc.fault = pc.resume;
        let instruction = executable
            .code
            .get(pc.fault)
            .ok_or_else(|| cold::internal("owned bytecode ended without return"))?;
        #[cfg(feature = "profiling")]
        let observed_depth = slots.depth();
        let mut next_pc = pc
            .fault
            .checked_add(1)
            .ok_or_else(|| cold::internal("owned program counter overflow"))?;
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
            Instruction::PushThis => 'push_this: {
                let normalized = cold
                    .rare
                    .get()
                    .and_then(|rare| rare.normalized_this.as_ref());
                // Borrowed-base fusion: the frame owns its (possibly
                // normalized) this for the whole activation, so a following
                // linked field read borrows it instead of copying and then
                // releasing a temporary owner edge.
                if let Some(Instruction::GetField(key)) = executable.code.get(next_pc) {
                    let this_value = normalized.unwrap_or(&cold.input.this_value);
                    if matches!(this_value, JsValue::Object(_)) {
                        if let Some(value) =
                            borrowed_base_field_read(runtime, executable, next_pc, *key, this_value)
                        {
                            #[cfg(feature = "profiling")]
                            cold::instruction(observed_depth + 1);
                            slots.push(value)?;
                            next_pc += 1;
                            break 'push_this true;
                        }
                    }
                }
                let value = if let Some(value) = normalized {
                    runtime
                        .dup_jsvalue(value)
                        .map_err(runtime_error_to_vm_error)?
                } else if executable.metadata.strict
                    || matches!(cold.input.this_value, JsValue::Object(_))
                {
                    copy_value(runtime, &cold.input.this_value)?
                } else if matches!(cold.input.this_value, JsValue::Undefined | JsValue::Null) {
                    copy_value(
                        runtime,
                        &JsValue::Object(
                            cold.input
                                .callee_global(runtime, executable.realm)?
                                .object_id(),
                        ),
                    )?
                } else {
                    return Ok(RunExit::NormalizeThis);
                };
                slots.push(value)?;
                true
            }
            Instruction::PutField(index) => {
                let Some(identity) = frame.property_generation.checked_add(1) else {
                    return Ok(RunExit::SetProperty(Some(*index)));
                };
                match slots.property_ic_write_scalar(runtime, executable, pc.fault, *index)? {
                    Some(true) => {}
                    Some(false) => return Ok(RunExit::SetProperty(Some(*index))),
                    None if resident_property!(property::Operation::Write(*index)) => {}
                    None => return Ok(RunExit::SetProperty(Some(*index))),
                }
                frame.property_generation = identity;
                true
            }
            Instruction::PutArrayEl => {
                let Some(identity) = frame.property_generation.checked_add(1) else {
                    return Ok(RunExit::SetProperty(None));
                };
                if !slots.typed_array_number_write(runtime)?
                    && !resident_property!(property::Operation::ElementWrite)
                {
                    return Ok(RunExit::SetProperty(None));
                }
                frame.property_generation = identity;
                true
            }
            Instruction::GetField(index) => {
                let mut native = None;
                if !slots.property_ic_read(
                    runtime,
                    executable,
                    pc.fault,
                    *index,
                    false,
                    &mut native,
                )? {
                    return Ok(RunExit::GetField {
                        index: *index,
                        keep_receiver: false,
                    });
                }
                true
            }
            Instruction::GetField2(index) => {
                let mut native = None;
                if !slots.property_ic_read(
                    runtime,
                    executable,
                    pc.fault,
                    *index,
                    true,
                    &mut native,
                )? {
                    return Ok(RunExit::GetField {
                        index: *index,
                        keep_receiver: true,
                    });
                }
                let candidate = executable
                    .fusion
                    .method_call(pc.fault)
                    .filter(|count| slots.has_operand_capacity(*count));
                if let Some(count) = candidate {
                    #[cfg(feature = "profiling")]
                    cold::instruction(observed_depth);
                    let start = pc.fault;
                    for offset in 0..count {
                        pc.fault = start + offset + 1;
                        pc.resume = pc.fault;
                        let Some(argument) = super::method_arguments::argument(
                            runtime,
                            &slots,
                            &executable.code[pc.fault],
                        )?
                        else {
                            // Earlier argument pushes are already canonical;
                            // resume this non-direct binding at its own PC.
                            continue 'execute;
                        };
                        slots.push(argument)?;
                        #[cfg(feature = "profiling")]
                        cold::instruction(observed_depth + offset + 1);
                    }
                    pc.fault = start + count + 1;
                    pc.resume = pc.fault;
                    #[cfg(feature = "profiling")]
                    cold::event("method_call_span");
                    execution.selected_native = native;
                    return Ok(RunExit::Call {
                        arguments: count as u16,
                        method: true,
                        tail: matches!(executable.code[pc.fault], Instruction::TailCallMethod(_)),
                    });
                }
                true
            }
            Instruction::GetArrayEl => {
                if !slots.array_immediate_read(runtime)? {
                    return Ok(RunExit::GetElement {
                        keep_receiver: false,
                        keep_key: false,
                    });
                }
                true
            }
            Instruction::GetArrayEl2 | Instruction::GetArrayEl3 => {
                if !slots.array_kept_immediate_read(
                    runtime,
                    matches!(instruction, Instruction::GetArrayEl3),
                )? && !resident_property!(property::Operation::ElementRead(matches!(
                    instruction,
                    Instruction::GetArrayEl3
                ))) {
                    return Ok(RunExit::GetElement {
                        keep_receiver: true,
                        keep_key: matches!(instruction, Instruction::GetArrayEl3),
                    });
                }
                true
            }
            Instruction::Construct(count) | Instruction::ConstructSuper(count) => {
                return Ok(RunExit::Construct(*count));
            }
            Instruction::PushNewTarget => {
                slots.push(copy_value(runtime, &cold.input.new_target)?)?;
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
                    target: *target_depth,
                    source: *source_depth,
                    excluded: Some(*excluded_depth),
                });
            }
            Instruction::InstanceOf => {
                return Ok(RunExit::Predicate(super::predicate_driver::Kind::Instance));
            }
            Instruction::In => return Ok(RunExit::Predicate(super::predicate_driver::Kind::Has)),
            Instruction::Delete => {
                if !resident_property!(property::Operation::Delete) {
                    return Ok(RunExit::Predicate(super::predicate_driver::Kind::Delete));
                }
                frame.property_generation = frame.property_generation.saturating_add(1);
                true
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
            Instruction::CheckCtor if !matches!(cold.input.new_target, JsValue::Undefined) => true,
            Instruction::CheckCtor => {
                return Ok(RunExit::Pure(
                    super::pure_operations::PureOperation::ConstructorWithoutNew,
                ));
            }
            Instruction::PushActiveFunction => {
                let id = cold.function.object_id();
                runtime
                    .retain_object_handle(id)
                    .map_err(heap_error_to_vm_error)?;
                slots.push(JsValue::Object(id))?;
                #[cfg(feature = "profiling")]
                cold::storage(crate::engine::api::profiling::OwnedStorageEvent::Copy {
                    heap_root: true,
                });
                true
            }
            Instruction::InitDerivedConstructor => return Ok(RunExit::InitDerivedConstructor),
            Instruction::PutVar(index) | Instruction::PutVarInit(index) => {
                let root = executable
                    .closure_variables
                    .get(usize::from(*index))
                    .filter(|descriptor| {
                        !descriptor.kind.is_private()
                            && matches!(
                                descriptor.name,
                                crate::engine::code::function::metadata::ClosureVariableName::Atom(
                                    _
                                )
                            )
                    })
                    .and_then(|_| cold.closure_slots.get(usize::from(*index)));
                let stored = if matches!(instruction, Instruction::PutVar(_)) {
                    if let Some(root) = root {
                        super::bindings::try_write_immediate_cell(
                            runtime,
                            &root,
                            slots.peek(0)?,
                            None,
                        )
                    } else {
                        false
                    }
                } else {
                    false
                };
                if stored {
                    drop(slots.pop()?);
                    #[cfg(feature = "profiling")]
                    cold::event("global_immediate_cell_write");
                    true
                } else {
                    return Ok(RunExit::Environment(
                        super::environment_driver::Operation::Put {
                            source: super::environment_driver::WriteTarget::Global {
                                index: *index,
                                initialize: matches!(instruction, Instruction::PutVarInit(_)),
                            },
                            name: 0, // Global names come from the authenticated closure descriptor.
                            strict: executable.metadata.strict,
                            check_presence: true,
                        },
                    ));
                }
            }
            Instruction::DeleteVar(index) => {
                return Ok(RunExit::Environment(
                    super::environment_driver::Operation::GlobalDelete(*index),
                ));
            }
            Instruction::GetVar(index) | Instruction::GetVarUndef(index) => 'get_var: {
                let root = executable
                    .closure_variables
                    .get(usize::from(*index))
                    .filter(|descriptor| {
                        !descriptor.kind.is_private()
                            && matches!(
                                descriptor.name,
                                crate::engine::code::function::metadata::ClosureVariableName::Atom(
                                    _
                                )
                            )
                    })
                    .and_then(|_| cold.closure_slots.get(usize::from(*index)));
                // Borrowed-base fusion: the live global cell keeps the base
                // object alive across this non-reentrant linked field read,
                // so no temporary base owner edge is created or released.
                if let (Some(root), Some(Instruction::GetField(key))) =
                    (root.as_ref(), executable.code.get(next_pc))
                {
                    if let Some(base) = runtime.borrow_cell_object_fast(root) {
                        if let Some(value) = borrowed_base_field_read(
                            runtime,
                            executable,
                            next_pc,
                            *key,
                            &JsValue::Object(base),
                        ) {
                            #[cfg(feature = "profiling")]
                            cold::instruction(observed_depth + 1);
                            slots.push(value)?;
                            next_pc += 1;
                            break 'get_var true;
                        }
                    }
                }
                let immediate =
                    root.and_then(|root| super::bindings::read_run_cell(runtime, &root));
                if let Some((value, _owned)) = immediate {
                    slots.push(value)?;
                    #[cfg(feature = "profiling")]
                    cold::event(if _owned {
                        "global_owned_cell_read"
                    } else {
                        "global_immediate_cell_read"
                    });
                    true
                } else if let Some(value) = super::environment_driver::try_global_own_read(
                    runtime,
                    executable,
                    &cold.closure_slots,
                    *index,
                )? {
                    slots.push(value)?;
                    true
                } else {
                    return Ok(RunExit::Environment(
                        super::environment_driver::Operation::GlobalGet {
                            index: *index,
                            strict: matches!(instruction, Instruction::GetVar(_)),
                        },
                    ));
                }
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
                            && executable.metadata.strict,
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
                        strict: executable.metadata.strict,
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
                        strict: executable.metadata.strict,
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
                        strict: executable.metadata.strict,
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
                if matches!(slots.peek(0)?, JsValue::Object(_)) {
                    true
                } else {
                    return Ok(RunExit::Environment(
                        super::environment_driver::Operation::ToObject,
                    ));
                }
            }
            Instruction::ToPropKey => match slots.peek(0)? {
                JsValue::Int(_) | JsValue::String(_) => true,
                JsValue::Symbol(_) => true,
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
                if !resident_property!(property::Operation::Define(*key)) {
                    return Ok(RunExit::DefineProperty {
                        key: Some(*key),
                        method: None,
                    });
                }
                frame.property_generation = frame.property_generation.saturating_add(1);
                true
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
                slots.push(JsValue::Int(*number))?;
                true
            }
            Instruction::Undefined => {
                slots.push(JsValue::Undefined)?;
                true
            }
            Instruction::Null => {
                slots.push(JsValue::Null)?;
                true
            }
            Instruction::PushTrue => {
                slots.push(JsValue::Bool(true))?;
                true
            }
            Instruction::PushFalse => {
                slots.push(JsValue::Bool(false))?;
                true
            }
            Instruction::PushConst(index) => {
                // Constant-left prepend `C + R` begins at this PushConst. The
                // constant must be a String and the local a direct, non-Object,
                // domain-valid binding; otherwise the canonical push runs.
                if executable.fusion.const_add_span(pc.fault).is_some()
                    && matches!(
                        executable.constant(*index),
                        Some(BytecodeConstant::Value(RawValue::String(_)))
                    )
                    && let Some(Instruction::GetLocal(right) | Instruction::GetLocalCheck(right)) =
                        executable.code.get(pc.fault + 1)
                    && slots.local_add_constant_supported(runtime, *right)?
                {
                    return Ok(RunExit::AddLocal);
                }
                // The published bytecode node owns the constant-pool edge;
                // the pushed operand duplicates it (scalars copy for free).
                let result = match executable.constant(*index) {
                    Some(BytecodeConstant::Value(RawValue::Int(number))) => {
                        Some(JsValue::Int(*number))
                    }
                    Some(BytecodeConstant::Value(RawValue::Float(number))) => {
                        Some(JsValue::Float(*number))
                    }
                    Some(BytecodeConstant::Value(RawValue::Undefined)) => Some(JsValue::Undefined),
                    Some(BytecodeConstant::Value(RawValue::Null)) => Some(JsValue::Null),
                    Some(BytecodeConstant::Value(RawValue::Bool(value))) => {
                        Some(JsValue::Bool(*value))
                    }
                    Some(BytecodeConstant::Value(RawValue::String(value))) => Some(
                        runtime
                            .dup_jsvalue(&JsValue::String(*value))
                            .map_err(runtime_error_to_vm_error)?,
                    ),
                    Some(BytecodeConstant::Value(RawValue::ShortBigInt(value))) => {
                        Some(JsValue::ShortBigInt(*value))
                    }
                    Some(BytecodeConstant::Value(RawValue::BigInt(value))) => Some(
                        runtime
                            .dup_jsvalue(&JsValue::BigInt(*value))
                            .map_err(runtime_error_to_vm_error)?,
                    ),
                    _ => None,
                };
                if let Some(value) = result {
                    #[cfg(feature = "profiling")]
                    cold::storage(crate::engine::api::profiling::OwnedStorageEvent::Copy {
                        heap_root: false,
                    });
                    slots.push(value)?;
                    true
                } else {
                    return Ok(RunExit::Pure(
                        super::pure_operations::PureOperation::Constant(*index),
                    ));
                }
            }
            Instruction::GetVarRef(index) | Instruction::GetVarRefCheck(index) => 'get_var_ref: {
                // Borrowed-base fusion: an initialized captured cell holding
                // an object cannot be in TDZ, so the checked variant needs no
                // separate authentication before completing the field read.
                if let (Some(root), Some(Instruction::GetField(key))) = (
                    cold.closure_slots.get(usize::from(*index)),
                    executable.code.get(next_pc),
                ) {
                    if let Some(base) = runtime.borrow_cell_object_fast(&root) {
                        if let Some(value) = borrowed_base_field_read(
                            runtime,
                            executable,
                            next_pc,
                            *key,
                            &JsValue::Object(base),
                        ) {
                            #[cfg(feature = "profiling")]
                            cold::instruction(observed_depth + 1);
                            slots.push(value)?;
                            next_pc += 1;
                            break 'get_var_ref true;
                        }
                    }
                }
                if let Some((value, _owned)) = cold
                    .closure_slots
                    .get(usize::from(*index))
                    .and_then(|root| super::bindings::read_run_cell(runtime, &root))
                {
                    slots.push(value)?;
                    #[cfg(feature = "profiling")]
                    cold::event(if _owned {
                        "captured_owned_cell_read"
                    } else {
                        "captured_immediate_cell_read"
                    });
                    true
                } else {
                    return Ok(RunExit::Binding {
                        source: BindingSource::Closure,
                        index: *index,
                        write: false,
                        checked: matches!(instruction, Instruction::GetVarRefCheck(_)),
                        keep: false,
                    });
                }
            }
            Instruction::PutVarRef(index)
            | Instruction::SetVarRef(index)
            | Instruction::PutVarRefCheck(index) => {
                let stored = if let (Some(root), Some(descriptor)) = (
                    cold.closure_slots.get(usize::from(*index)),
                    executable.closure_variables.get(usize::from(*index)),
                ) {
                    super::bindings::try_write_immediate_cell(
                        runtime,
                        &root,
                        slots.peek(0)?,
                        Some((descriptor.is_lexical, descriptor.is_const, descriptor.kind)),
                    )
                } else {
                    false
                };
                if stored {
                    if !matches!(instruction, Instruction::SetVarRef(_)) {
                        drop(slots.pop()?);
                    }
                    #[cfg(feature = "profiling")]
                    cold::event("captured_immediate_cell_write");
                    true
                } else {
                    return Ok(RunExit::Binding {
                        source: BindingSource::Closure,
                        index: *index,
                        write: true,
                        checked: matches!(instruction, Instruction::PutVarRefCheck(_)),
                        keep: matches!(instruction, Instruction::SetVarRef(_)),
                    });
                }
            }
            Instruction::PutLocal(index)
            | Instruction::SetLocal(index)
            | Instruction::PutLocalCheck(index)
            | Instruction::SetLocalCheck(index)
                if matches!(slots.local(*index)?, FrameBinding::Captured(_)) =>
            {
                let stored = if let (FrameBinding::Captured(var_ref), Some(definition)) = (
                    slots.local(*index)?,
                    executable.local_definitions.get(usize::from(*index)),
                ) {
                    super::bindings::try_write_immediate_cell(
                        runtime,
                        &crate::engine::heap::roots::VarRefView::from_frame(runtime, *var_ref),
                        slots.peek(0)?,
                        Some((definition.is_lexical, definition.is_const, definition.kind)),
                    )
                } else {
                    false
                };
                if stored {
                    if !matches!(
                        instruction,
                        Instruction::SetLocal(_) | Instruction::SetLocalCheck(_)
                    ) {
                        drop(slots.pop()?);
                    }
                    #[cfg(feature = "profiling")]
                    cold::event("captured_immediate_cell_write");
                    true
                } else {
                    return Ok(RunExit::Binding {
                        source: BindingSource::Local,
                        index: *index,
                        write: true,
                        checked: matches!(
                            instruction,
                            Instruction::PutLocalCheck(_) | Instruction::SetLocalCheck(_)
                        ),
                        keep: matches!(
                            instruction,
                            Instruction::SetLocal(_) | Instruction::SetLocalCheck(_)
                        ),
                    });
                }
            }
            Instruction::GetArg(index)
            | Instruction::PutArg(index)
            | Instruction::SetArg(index)
                if matches!(slots.parameter(*index)?, FrameBinding::Captured(_)) =>
            {
                #[cfg(feature = "profiling")]
                if matches!(instruction, Instruction::GetArg(_)) {
                    crate::engine::api::profiling::record_fusion_dispatch(
                        runtime,
                        executable,
                        pc.fault,
                        executable.fusion.entry(pc.fault).has_candidate(),
                    );
                }
                let immediate = if matches!(instruction, Instruction::GetArg(_)) {
                    match slots.parameter(*index)? {
                        FrameBinding::Captured(var_ref) => super::bindings::read_run_cell(
                            runtime,
                            &crate::engine::heap::roots::VarRefView::from_frame(runtime, *var_ref),
                        ),
                        _ => None,
                    }
                } else {
                    None
                };
                if let Some((value, _owned)) = immediate {
                    slots.push(value)?;
                    #[cfg(feature = "profiling")]
                    cold::event(if _owned {
                        "captured_owned_cell_read"
                    } else {
                        "captured_immediate_cell_read"
                    });
                    true
                } else if !matches!(instruction, Instruction::GetArg(_))
                    && match (
                        slots.parameter(*index)?,
                        executable.argument_definitions.get(usize::from(*index)),
                    ) {
                        (FrameBinding::Captured(var_ref), Some(definition)) => {
                            super::bindings::try_write_immediate_cell(
                                runtime,
                                &crate::engine::heap::roots::VarRefView::from_frame(
                                    runtime, *var_ref,
                                ),
                                slots.peek(0)?,
                                Some((definition.is_lexical, definition.is_const, definition.kind)),
                            )
                        }
                        _ => false,
                    }
                {
                    if !matches!(instruction, Instruction::SetArg(_)) {
                        drop(slots.pop()?);
                    }
                    #[cfg(feature = "profiling")]
                    cold::event("captured_immediate_cell_write");
                    true
                } else {
                    return Ok(RunExit::Binding {
                        source: BindingSource::Argument,
                        index: *index,
                        write: !matches!(instruction, Instruction::GetArg(_)),
                        checked: false,
                        keep: matches!(instruction, Instruction::SetArg(_)),
                    });
                }
            }
            Instruction::InitializeLocal(index)
                if matches!(slots.local(*index)?, FrameBinding::Captured(_))
                    && executable.local_definitions[usize::from(*index)].kind
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
                let fusion_entry = executable.fusion.entry(pc.fault);
                #[cfg(feature = "profiling")]
                crate::engine::api::profiling::record_fusion_dispatch(
                    runtime,
                    executable,
                    pc.fault,
                    fusion_entry.has_candidate(),
                );
                if !fusion_entry.is_empty() {
                    match fusion_entry.local_choice() {
                        LocalFusionChoice::LocalAdd(instructions) => {
                            // S2/S4: a numeric pair or numeric literal completes
                            // inside the scalar domain; every other kind keeps the
                            // outlined primitive-addition bridge.
                            if fusion::numeric_local_add(&mut slots, executable, pc.fault, *index)
                                .is_some()
                            {
                                #[cfg(feature = "profiling")]
                                crate::engine::api::profiling::record_fusion_outcome(
                                    runtime,
                                    executable,
                                    pc.fault,
                                    "local_add",
                                    None,
                                );
                                #[cfg(feature = "profiling")]
                                fusion::record_span(
                                    &executable.code[pc.fault..pc.fault + instructions],
                                    observed_depth,
                                );
                                pc.resume = pc.fault + instructions;
                                continue;
                            }
                            #[cfg(feature = "profiling")]
                            crate::engine::api::profiling::record_fusion_outcome(
                                runtime,
                                executable,
                                pc.fault,
                                "local_add",
                                Some("guard"),
                            );
                            let supported = match executable.code.get(pc.fault + 1) {
                                Some(
                                    Instruction::GetLocal(right)
                                    | Instruction::GetLocalCheck(right),
                                ) => slots.local_add_supported(runtime, *index, *right)?,
                                Some(Instruction::PushConst(constant))
                                    if matches!(
                                        executable.constant(*constant),
                                        Some(BytecodeConstant::Value(RawValue::String(_)))
                                    ) =>
                                {
                                    slots.local_add_constant_supported(runtime, *index)?
                                }
                                _ => false,
                            };
                            if supported {
                                return Ok(RunExit::AddLocal);
                            }
                        }
                        LocalFusionChoice::Update(update) => {
                            let updated = fusion::update_local(&mut slots, *index, update);
                            #[cfg(feature = "profiling")]
                            crate::engine::api::profiling::record_fusion_outcome(
                                runtime,
                                executable,
                                pc.fault,
                                "update",
                                match &updated {
                                    Ok(true) => None,
                                    Ok(false) => Some("guard"),
                                    Err(_) => Some("error"),
                                },
                            );
                            if updated? {
                                #[cfg(feature = "profiling")]
                                fusion::record_span(
                                    &executable.code[pc.fault..pc.fault + update.instructions],
                                    observed_depth,
                                );
                                pc.resume = pc.fault + update.instructions;
                                continue;
                            }
                        }
                        // S1: two direct producers, one numeric comparison, one
                        // conditional branch. Nothing is pushed or popped, so a guard
                        // miss leaves the canonical span start untouched.
                        LocalFusionChoice::CompareBranch(instructions) => {
                            if let Some(next) = fusion::local_compare_branch(
                                &slots,
                                &executable.code[pc.fault..],
                                pc.fault,
                                instructions,
                            ) {
                                #[cfg(feature = "profiling")]
                                crate::engine::api::profiling::record_fusion_outcome(
                                    runtime, executable, pc.fault, "compare", None,
                                );
                                #[cfg(feature = "profiling")]
                                fusion::record_span(
                                    &executable.code[pc.fault..pc.fault + instructions],
                                    observed_depth,
                                );
                                pc.resume = next;
                                continue;
                            }
                            #[cfg(feature = "profiling")]
                            crate::engine::api::profiling::record_fusion_outcome(
                                runtime,
                                executable,
                                pc.fault,
                                "compare",
                                Some("guard"),
                            );
                        }
                        // S3: one direct object base completes one linked field read
                        // into the numeric accumulator. The location-cache peek is
                        // non-owning; every other shape stays canonical.
                        LocalFusionChoice::FieldAdd(instructions) => {
                            if fusion::numeric_local_field_add(
                                &mut slots, runtime, executable, pc.fault, *index,
                            )
                            .is_some()
                            {
                                #[cfg(feature = "profiling")]
                                crate::engine::api::profiling::record_fusion_outcome(
                                    runtime,
                                    executable,
                                    pc.fault,
                                    "field_add",
                                    None,
                                );
                                #[cfg(feature = "profiling")]
                                fusion::record_span(
                                    &executable.code[pc.fault..pc.fault + instructions],
                                    observed_depth,
                                );
                                pc.resume = pc.fault + instructions;
                                continue;
                            }
                            #[cfg(feature = "profiling")]
                            crate::engine::api::profiling::record_fusion_outcome(
                                runtime,
                                executable,
                                pc.fault,
                                "field_add",
                                Some("guard"),
                            );
                        }
                        LocalFusionChoice::Dense(kind) => {
                            if let Some(end) = fusion::try_numeric_span(
                                &mut slots,
                                runtime,
                                executable,
                                pc.fault,
                                kind,
                                &mut frame.property_generation,
                            ) {
                                #[cfg(feature = "profiling")]
                                fusion::record_span(
                                    &executable.code[pc.fault..end],
                                    observed_depth,
                                );
                                pc.resume = end;
                                continue;
                            }
                        }
                        LocalFusionChoice::Canonical => {}
                    }
                }
                match slots.local(*index)? {
                    FrameBinding::Direct(value) => {
                        // Borrowed-base fusion: the frame slot keeps this base
                        // alive, so a following linked field read borrows it
                        // instead of copying an owner edge that the read would
                        // release right away.
                        // Check the operand kind first: scalar-heavy loops
                        // must not pay the next-instruction load.
                        let fused = if matches!(value, JsValue::Object(_)) {
                            match executable.code.get(next_pc) {
                                Some(Instruction::GetField(key)) => borrowed_base_field_read(
                                    runtime, executable, next_pc, *key, value,
                                ),
                                _ => None,
                            }
                        } else {
                            None
                        };
                        if let Some(result) = fused {
                            #[cfg(feature = "profiling")]
                            cold::instruction(observed_depth + 1);
                            slots.push(result)?;
                            next_pc += 1;
                        } else {
                            let copied = copy_value(runtime, value)?;
                            slots.push(copied)?;
                        }
                        true
                    }
                    FrameBinding::Captured(var_ref) => {
                        let view =
                            crate::engine::heap::roots::VarRefView::from_frame(runtime, *var_ref);
                        let fused = match executable.code.get(next_pc) {
                            Some(Instruction::GetField(key)) => {
                                runtime.borrow_cell_object_fast(&view).and_then(|base| {
                                    borrowed_base_field_read(
                                        runtime,
                                        executable,
                                        next_pc,
                                        *key,
                                        &JsValue::Object(base),
                                    )
                                })
                            }
                            _ => None,
                        };
                        if let Some(result) = fused {
                            #[cfg(feature = "profiling")]
                            cold::instruction(observed_depth + 1);
                            slots.push(result)?;
                            next_pc += 1;
                            true
                        } else if let Some((value, _owned)) =
                            super::bindings::read_run_cell(runtime, &view)
                        {
                            slots.push(value)?;
                            #[cfg(feature = "profiling")]
                            cold::event(if _owned {
                                "captured_owned_cell_read"
                            } else {
                                "captured_immediate_cell_read"
                            });
                            true
                        } else {
                            return Ok(RunExit::Binding {
                                source: BindingSource::Local,
                                index: *index,
                                write: false,
                                checked: matches!(instruction, Instruction::GetLocalCheck(_)),
                                keep: false,
                            });
                        }
                    }
                    FrameBinding::Uninitialized
                        if matches!(instruction, Instruction::GetLocalCheck(_)) =>
                    {
                        return Ok(RunExit::LexicalUninitialized(*index));
                    }
                    _ => false,
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
                    if let Some(flag) = cold.reusable_captured_locals.get_mut(usize::from(*index)) {
                        *flag = false;
                    }
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
                            .slot_value_release_readiness_jsvalue(old)
                            .map_err(runtime_error_to_vm_error)?
                            == SlotReleaseReadiness::Ready
                    }
                    _ => false,
                };
                if ready {
                    let old = slots.replace_local(*index, FrameBinding::Uninitialized)?;
                    if let Some(flag) = cold.reusable_captured_locals.get_mut(usize::from(*index)) {
                        *flag = false;
                    }
                    if matches!(old, FrameBinding::Direct(_)) {
                        release_displaced(runtime, old)?;
                    }
                }
                if !ready {
                    release_outside_slots!({
                        let old = slots.replace_local(*index, FrameBinding::Uninitialized)?;
                        if let Some(flag) =
                            cold.reusable_captured_locals.get_mut(usize::from(*index))
                        {
                            *flag = false;
                        }
                        old
                    });
                }
                true
            }
            Instruction::InitializeLocal(index) => {
                let definition = executable.local_definitions[usize::from(*index)];
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
                                .slot_value_release_readiness_jsvalue(old)
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
                    release_outside_slots!({
                        let next = slots.pop()?;
                        slots.replace_local(*index, FrameBinding::Direct(next))?
                    });
                    true
                } else {
                    ready
                }
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
                let keep = matches!(
                    instruction,
                    Instruction::SetLocal(_) | Instruction::SetLocalCheck(_)
                );
                let write_class = direct_write_class(runtime, slots.local(*index)?)?;
                if write_class == DirectWriteClass::Number
                    && slots.store_proven_number_operand(DirectSlot::Local(*index), keep)
                {
                    true
                } else if matches!(
                    write_class,
                    DirectWriteClass::Number | DirectWriteClass::Ready
                ) {
                    let next = if keep {
                        copy_value(runtime, slots.peek(0)?)?
                    } else {
                        slots.pop()?
                    };
                    let old = slots.replace_local(*index, FrameBinding::Direct(next))?;
                    if write_class == DirectWriteClass::Number {
                        // The authenticated previous binding is an inline Number.
                        drop(old);
                    } else {
                        release_displaced(runtime, old)?;
                    }
                    true
                } else if write_class == DirectWriteClass::NeedsBoundary {
                    release_outside_slots!({
                        let next = if keep {
                            copy_value(runtime, slots.peek(0)?)?
                        } else {
                            slots.pop()?
                        };
                        slots.replace_local(*index, FrameBinding::Direct(next))?
                    });
                    true
                } else {
                    false
                }
            }
            Instruction::GetArg(index) => {
                #[cfg(feature = "profiling")]
                crate::engine::api::profiling::record_fusion_dispatch(
                    runtime,
                    executable,
                    pc.fault,
                    executable.fusion.entry(pc.fault).has_candidate(),
                );
                if let Some(kind) = executable.fusion.dense_span(pc.fault) {
                    if let Some(end) = fusion::try_numeric_span(
                        &mut slots,
                        runtime,
                        executable,
                        pc.fault,
                        kind,
                        &mut frame.property_generation,
                    ) {
                        #[cfg(feature = "profiling")]
                        fusion::record_span(&executable.code[pc.fault..end], observed_depth);
                        pc.resume = end;
                        continue;
                    }
                }
                if let FrameBinding::Direct(value) = slots.parameter(*index)? {
                    let copied = copy_value(runtime, value)?;
                    slots.push(copied)?;
                    true
                } else {
                    false
                }
            }
            Instruction::PutArg(index) | Instruction::SetArg(index) => {
                let keep = matches!(instruction, Instruction::SetArg(_));
                let write_class = direct_write_class(runtime, slots.parameter(*index)?)?;
                if write_class == DirectWriteClass::Number
                    && slots.store_proven_number_operand(DirectSlot::Argument(*index), keep)
                {
                    true
                } else if matches!(
                    write_class,
                    DirectWriteClass::Number | DirectWriteClass::Ready
                ) {
                    let next = if keep {
                        copy_value(runtime, slots.peek(0)?)?
                    } else {
                        slots.pop()?
                    };
                    let old = slots.replace_parameter(*index, FrameBinding::Direct(next))?;
                    if write_class == DirectWriteClass::Number {
                        drop(old);
                    } else {
                        release_displaced(runtime, old)?;
                    }
                    true
                } else if write_class == DirectWriteClass::NeedsBoundary {
                    release_outside_slots!({
                        let next = if keep {
                            copy_value(runtime, slots.peek(0)?)?
                        } else {
                            slots.pop()?
                        };
                        slots.replace_parameter(*index, FrameBinding::Direct(next))?
                    });
                    true
                } else {
                    false
                }
            }
            Instruction::Dup => {
                slots.insert_copy(runtime, 0, 0)?;
                true
            }
            Instruction::Dup1 => {
                slots.insert_copy(runtime, 1, 1)?;
                true
            }
            Instruction::Dup3 => {
                slots.duplicate_operands(runtime, 3)?;
                true
            }
            Instruction::Insert2 | Instruction::Insert3 | Instruction::Insert4 => {
                let count = match instruction {
                    Instruction::Insert2 => 2,
                    Instruction::Insert3 => 3,
                    _ => 4,
                };
                slots.peek(count - 1)?;
                slots.insert_copy(runtime, 0, count)?;
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
                    drop(slots.pop()?);
                    true
                } else if primitive_release_owner(slots.peek(0)?) {
                    let released = slots.pop()?;
                    release_dropped(runtime, released)?;
                    true
                } else {
                    release_outside_slots!(slots.pop()?);
                    true
                }
            }
            Instruction::Swap => {
                slots.rotate_operands(0, 2, false)?;
                true
            }
            Instruction::Nip => {
                if slots.release_operand(1, runtime)? {
                    let right = slots.pop()?;
                    drop(slots.pop()?);
                    slots.push(right)?;
                    true
                } else if primitive_release_owner(slots.peek(1)?) {
                    let kept = slots.pop()?;
                    let released = slots.pop()?;
                    slots.push(kept)?;
                    release_dropped(runtime, released)?;
                    true
                } else {
                    release_outside_slots!({
                        slots.peek(1)?;
                        let kept = slots.pop()?;
                        let released = slots.pop()?;
                        slots.push(kept)?;
                        released
                    });
                    true
                }
            }
            Instruction::Add => binary(&mut slots, |a, b| value(a.add(b)))?,
            Instruction::Sub => binary(&mut slots, |a, b| value(a.sub(b)))?,
            Instruction::Mul => binary(&mut slots, |a, b| value(a.mul(b)))?,
            Instruction::Div => binary(&mut slots, |a, b| value(a.div(b)))?,
            Instruction::Mod => binary(&mut slots, |a, b| value(a.rem(b)))?,
            Instruction::Pow => binary(&mut slots, |a, b| value(a.pow(b)))?,
            Instruction::Shl => binary(&mut slots, |a, b| {
                JsValue::Int(a.int32().wrapping_shl(b.int32() as u32 & 31))
            })?,
            Instruction::Sar => binary(&mut slots, |a, b| {
                JsValue::Int(a.int32() >> (b.int32() as u32 & 31))
            })?,
            Instruction::Shr => binary(&mut slots, |a, b| {
                value(Number::compact(f64::from(
                    (a.int32() as u32) >> (b.int32() as u32 & 31),
                )))
            })?,
            Instruction::BitAnd => binary(&mut slots, |a, b| JsValue::Int(a.int32() & b.int32()))?,
            Instruction::BitOr => binary(&mut slots, |a, b| JsValue::Int(a.int32() | b.int32()))?,
            Instruction::BitXor => binary(&mut slots, |a, b| JsValue::Int(a.int32() ^ b.int32()))?,
            Instruction::Lt
            | Instruction::Lte
            | Instruction::Gt
            | Instruction::Gte
            | Instruction::Eq
            | Instruction::Neq
            | Instruction::StrictEq
            | Instruction::StrictNeq
                if executable.fusion.compare_branch(pc.fault)
                    && (!matches!(instruction, Instruction::StrictEq | Instruction::StrictNeq)
                        || (number(slots.peek(0)?).is_some()
                            && number(slots.peek(1)?).is_some())) =>
            {
                let branch_pc = pc.fault + 1;
                let Some(target) =
                    fusion::compare_branch(&mut slots, instruction, &executable.code[branch_pc])?
                else {
                    if matches!(instruction, Instruction::StrictEq | Instruction::StrictNeq) {
                        return Ok(RunExit::StrictEquality(matches!(
                            instruction,
                            Instruction::StrictNeq
                        )));
                    }
                    return Ok(RunExit::Numeric(
                        super::numeric::operation::NumericKind::for_instruction(instruction)
                            .ok_or_else(|| cold::internal("comparison has no numeric operation"))?,
                    ));
                };
                #[cfg(feature = "profiling")]
                fusion::record_span(&executable.code[pc.fault..pc.fault + 2], observed_depth);
                pc.resume = if target == usize::MAX {
                    pc.fault + 2
                } else {
                    target
                };
                continue;
            }
            Instruction::Lt => binary(&mut slots, |a, b| JsValue::Bool(a.float() < b.float()))?,
            Instruction::Lte => binary(&mut slots, |a, b| JsValue::Bool(a.float() <= b.float()))?,
            Instruction::Gt => binary(&mut slots, |a, b| JsValue::Bool(a.float() > b.float()))?,
            Instruction::Gte => binary(&mut slots, |a, b| JsValue::Bool(a.float() >= b.float()))?,
            Instruction::StrictEq | Instruction::StrictNeq => {
                let negate = matches!(instruction, Instruction::StrictNeq);
                if binary(&mut slots, |a, b| {
                    JsValue::Bool((a.float() == b.float()) != negate)
                })? {
                    true
                } else {
                    {
                        // Rope (non-flat) spellings take the resident string
                        // comparison; the arena dereference is pure.
                        let rope = match (slots.peek(1)?, slots.peek(0)?) {
                            (JsValue::String(left), JsValue::String(right)) => {
                                let state = runtime.0.state.borrow();
                                let heap = &state.heap;
                                let left = heap.string_fast(*left);
                                let right = heap.string_fast(*right);
                                !left.is_flat() || !right.is_flat()
                            }
                            _ => false,
                        };
                        if rope {
                            return Ok(RunExit::StrictEquality(negate));
                        }
                    }
                    let equal = runtime
                        .strict_equal_jsvalue(slots.peek(1)?, slots.peek(0)?)
                        .map_err(runtime_error_to_vm_error)?
                        != negate;
                    // Straight-line operand checks; see the numeric fallback
                    // below for why a range loop is avoided here.
                    // Destructure the Result (see the numeric fallback below):
                    // moving the Err variant out keeps the whole-Result drop
                    // glue off the Ok path.
                    #[inline(always)]
                    fn release_observable(operand: Result<&JsValue, Error>) -> bool {
                        match operand {
                            Ok(value) => {
                                matches!(value, JsValue::Object(_) | JsValue::Symbol(_))
                            }
                            Err(error) => {
                                drop(error);
                                false
                            }
                        }
                    }
                    let observable =
                        release_observable(slots.peek(0)) || release_observable(slots.peek(1));
                    if observable {
                        release_outside_slots!({
                            let right = slots.pop()?;
                            let left = slots.pop()?;
                            slots.push(JsValue::Bool(equal))?;
                            (left, right)
                        });
                    } else {
                        let right = slots.pop()?;
                        let left = slots.pop()?;
                        slots.push(JsValue::Bool(equal))?;
                        drop(slots);
                        release_dropped(runtime, (left, right))?;
                        slots = transaction.slots();
                    }
                    true
                }
            }
            Instruction::Eq => binary(&mut slots, |a, b| JsValue::Bool(a.float() == b.float()))?,
            Instruction::Neq => binary(&mut slots, |a, b| JsValue::Bool(a.float() != b.float()))?,
            Instruction::Not => {
                // Includes Annex B HTMLDDA objects; metadata lookup cannot run JS.
                let result = !runtime
                    .value_to_boolean_jsvalue(slots.peek(0)?)
                    .map_err(runtime_error_to_vm_error)?;
                if matches!(slots.peek(0)?, JsValue::Object(_) | JsValue::Symbol(_)) {
                    release_outside_slots!({
                        let input = slots.pop()?;
                        slots.push(JsValue::Bool(result))?;
                        input
                    });
                } else {
                    let input = slots.pop()?;
                    slots.push(JsValue::Bool(result))?;
                    drop(slots);
                    release_dropped(runtime, input)?;
                    slots = transaction.slots();
                }
                true
            }
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
                        drop(slots.pop()?);
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
                    .map_err(|_| cold::internal("gosub return PC does not fit Int"))?;
                slots.push(JsValue::Int(pc))?;
                next_pc = *target as usize;
                true
            }
            Instruction::Ret => {
                let JsValue::Int(target) = slots.pop()? else {
                    return Err(cold::internal("invalid ret value"));
                };
                next_pc =
                    usize::try_from(target).map_err(|_| cold::internal("invalid ret value"))?;
                if next_pc >= executable.code.len() {
                    return Err(cold::internal("invalid ret value"));
                }
                true
            }
            Instruction::DropGosub => {
                if !matches!(slots.pop()?, JsValue::Int(_)) {
                    return Err(cold::internal("invalid gosub cleanup value"));
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
                cold::instruction(observed_depth);
                return Ok(RunExit::Suspend(kind));
            }
            Instruction::Return => {
                execution.pending = Some(slots.pop()?);
                pc.resume = next_pc;
                #[cfg(feature = "profiling")]
                cold::instruction(observed_depth);
                return Ok(RunExit::Complete);
            }
            Instruction::ReturnUndefined => {
                execution.pending = Some(JsValue::Undefined);
                pc.resume = next_pc;
                #[cfg(feature = "profiling")]
                cold::instruction(observed_depth);
                return Ok(RunExit::Complete);
            }
        };
        if !handled {
            if let Some(kind) = super::numeric::operation::NumericKind::for_instruction(instruction)
            {
                if numeric::supported(&slots, kind) {
                    // Symbol release and BigInt errors may observe the stack.
                    // Number/String/bool coercions cannot construct a JS error.
                    // Straight-line operand checks: a range loop materializes
                    // the peek Result in memory and keeps its drop glue on the
                    // success path of every iteration.
                    // Destructure the Result instead of matching a temporary:
                    // moving the Err variant out keeps the whole-Result drop
                    // glue off the Ok path.
                    #[inline(always)]
                    fn observes_stack(operand: Result<&JsValue, Error>) -> bool {
                        match operand {
                            Ok(value) => matches!(
                                value,
                                JsValue::Symbol(_) | JsValue::BigInt(_) | JsValue::ShortBigInt(_)
                            ),
                            Err(error) => {
                                drop(error);
                                false
                            }
                        }
                    }
                    if !frame.active_frame.is_materialized()
                        && (observes_stack(slots.peek(0))
                            || (!kind.unary() && observes_stack(slots.peek(1))))
                    {
                        return Ok(RunExit::Materialize);
                    }
                    // Preserve active-PC admission before consuming operands,
                    // then keep this transaction and run frame across parsing.
                    // The active PC is published lazily by numeric::complete
                    // only if a JavaScript error is materialized.
                    drop(slots);
                    pc.publish_fault();
                    if !numeric::complete(
                        runtime,
                        executable.realm,
                        &mut transaction,
                        kind,
                        &mut execution.pending,
                        frame.active_frame,
                        pc.fault,
                    )? {
                        return Ok(RunExit::PrimitiveThrow);
                    }
                    slots = transaction.slots();
                } else {
                    return Ok(RunExit::Numeric(kind));
                }
            } else {
                return Ok(RunExit::Bridge);
            }
        }
        #[cfg(feature = "profiling")]
        cold::instruction(observed_depth);
        pc.resume = next_pc;
    }
}

#[cfg(test)]
mod borrowed_base_field_tests {
    use crate::engine::api::{Runtime, Value};

    #[test]
    fn borrowed_base_field_reads_observe_live_bindings_for_every_base_kind() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        drop(
            context
                .eval("var fusedGlobal = { x: 1, o: { answer: 40 } };")
                .unwrap(),
        );
        assert_eq!(
            context
                .eval(
                    r#"(function(){
            var a = fusedGlobal.x + fusedGlobal.o.answer;
            fusedGlobal = { x: 2, o: { answer: 50 } };
            var b = fusedGlobal.x;
            var local = { y: 7 };
            var c = local.y;
            let cell = { z: 1 };
            function readZ() { return cell.z; }
            function swap() { cell = { z: 9 }; }
            var d = readZ(); swap(); var e = readZ();
            var obj = { v: 5, read: function () { return this.v; } };
            var f = obj.read();
            return a + b + c + d + e + f;
        })()"#
                )
                .unwrap(),
            Value::Int(65)
        );
    }

    #[test]
    fn borrowed_base_field_reads_claim_no_base_owner() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        drop(context.eval("var countGlobal = { x: 3 };").unwrap());
        let Value::Object(base) = context.eval("countGlobal").unwrap() else {
            panic!("expected object global");
        };
        let id = base.object_id();
        let strong = |runtime: &Runtime| {
            runtime
                .0
                .state
                .borrow()
                .heap
                .object_strong_count(id)
                .unwrap()
        };
        let before = strong(&runtime);
        assert_eq!(
            context
                .eval("(function(){var s=0;for(var i=0;i<100;i++)s+=countGlobal.x;return s;})()")
                .unwrap(),
            Value::Int(300)
        );
        assert_eq!(strong(&runtime), before);
        drop(base);
    }

    #[test]
    fn borrowed_base_field_fallback_preserves_getters_prototypes_tdz_and_invalidation() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        drop(
            context
                .eval("var getterGlobal = { count: 0, get g() { this.count++; return this.count; } };")
                .unwrap(),
        );
        assert_eq!(
            context
                .eval("(function(){var a=getterGlobal.g,b=getterGlobal.g;return a===1&&b===2;})()")
                .unwrap(),
            Value::Bool(true)
        );
        drop(
            context
                .eval("var protoGlobal = Object.create({ p: 11 }); protoGlobal.own = 1;")
                .unwrap(),
        );
        assert_eq!(
            context
                .eval("(function(){var s=0;for(var i=0;i<4;i++)s+=protoGlobal.p;return s;})()")
                .unwrap(),
            Value::Int(44)
        );
        // A captured lexical cell stays in TDZ until initialized, and the
        // fused read must not observe the uninitialized cell.
        assert_eq!(
            context
                .eval(
                    r#"(function(){
            var threw = false;
            function readField() { return boxed.q; }
            try { readField(); } catch (e) { threw = e instanceof ReferenceError; }
            let boxed = { q: 3 };
            return threw && readField() === 3;
        })()"#
                )
                .unwrap(),
            Value::Bool(true)
        );
        // Warm fused reads must still observe a later accessor redefinition.
        drop(context.eval("var swapGlobal = { x: 1 };").unwrap());
        assert_eq!(
            context
                .eval(
                    r#"(function(){
            var s = 0;
            for (var i = 0; i < 4; i++) s += swapGlobal.x;
            Object.defineProperty(swapGlobal, 'x', { get() { return 42; } });
            return s + swapGlobal.x;
        })()"#
                )
                .unwrap(),
            Value::Int(46)
        );
    }
}

#[cfg(all(test, feature = "profiling"))]
mod tests {
    use crate::engine::api::profiling::CostProfile;
    use crate::engine::api::{Runtime, Value};
    use crate::engine::heap::SlotReleaseReadiness;

    #[test]
    fn borrowed_base_field_fusion_engages_for_global_captured_local_and_this_bases() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        drop(context.eval("var fusionGlobal = { x: 1 };").unwrap());
        // Cold linked-read sites warm their inline caches through the
        // canonical pair; the fused borrowed-base read takes over afterwards,
        // even when the binding holds the base object's only owner edge.
        let warm = r#"(function(){
            let cell = { z: 2 };
            function readZ() { return cell.z; }
            var local = { y: 3 };
            var obj = { v: 4, read: function () { return this.v; } };
            var s = 0;
            for (var i = 0; i < 5; i++) {
                s += fusionGlobal.x + readZ() + local.y + obj.read();
            }
            return s;
        })()"#;
        let profile = CostProfile::start();
        assert_eq!(context.eval(warm).unwrap(), Value::Int(50));
        let costs = profile.snapshot();
        assert!(
            costs
                .owned_execution_events
                .get("fusion.BorrowedBaseField")
                .copied()
                .unwrap_or(0)
                >= 12,
            "{costs:?}"
        );
    }

    #[test]
    fn property_ic_resides_for_own_prototype_and_method_reads() {
        for holder in ["({x:{answer:42}})", "Object.create({x:{answer:42}})"] {
            let runtime = Runtime::new();
            let mut context = runtime.new_context();
            drop(context.eval(&format!("var icHolder={holder}; function icRead(n){{var r;for(var i=0;i<n;i++)r=icHolder.x;return r;}} icRead(2)")).unwrap());
            let expected = context.eval("icHolder.x").unwrap();
            let profile = CostProfile::start();
            assert_eq!(context.eval("icRead(20)").unwrap(), expected);
            let costs = profile.snapshot();
            assert_eq!(
                costs.owned_execution_events.get("property_ic.hit"),
                Some(&20)
            );
            assert_eq!(
                costs
                    .owned_execution_events
                    .get("run_exit.GetField")
                    .copied()
                    .unwrap_or(0),
                0
            );
        }
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        drop(context.eval("var icMethodHolder={min:Math.min};function icMethod(n){var r;for(var i=0;i<n;i++)r=icMethodHolder.min(42,43);return r;}icMethod(2)").unwrap());
        let profile = CostProfile::start();
        assert_eq!(context.eval("icMethod(20)").unwrap(), Value::Int(42));
        let costs = profile.snapshot();
        assert_eq!(
            costs.owned_execution_events.get("property_ic.hit"),
            Some(&20)
        );
        assert_eq!(
            costs.owned_execution_events.get("method_call_span"),
            Some(&20)
        );
        assert_eq!(
            costs
                .owned_execution_events
                .get("run_exit.GetField")
                .copied()
                .unwrap_or(0),
            0
        );
        drop(profile);
        drop(context.eval("icMethodHolder.min=Math.max").unwrap());
        assert_eq!(context.eval("icMethod(3)").unwrap(), Value::Int(43));
    }

    #[test]
    fn property_ic_invalidations_preserve_accessor_receiver_and_current_value() {
        for source in [
            "var o={x:1};function r(){return o.x;}r();r();o.x=42;r()",
            "var p={x:1},o=Object.create(p);function r(){return o.x;}r();r();Object.defineProperty(p,'x',{get(){return this.y},configurable:true});o.y=42;r()",
            "var p={x:1},m=Object.create(p),o=Object.create(m);function r(){return o.x;}r();r();Object.setPrototypeOf(m,{x:42});r()",
            "var o={x:1};function r(){return o.x;}r();r();delete o.x;Object.setPrototypeOf(o,{x:42});r()",
            "var o={x:1};function r(){return o.x;}r();r();Object.defineProperty(o,'x',{get(){return 42}});r()",
            "var o={x:1};function r(){return o.x;}r();r();for(var i=0;i<40;i++)o['k'+i]=i;delete o.k0;o.x=42;r()",
            "var o={v:42,f(){return this.v}};function r(){return o.f()}r();r();r()",
        ] {
            let runtime = Runtime::new();
            let mut context = runtime.new_context();
            assert_eq!(context.eval(source).unwrap(), Value::Int(42), "{source}");
            assert!(runtime.0.state.borrow().active_frames.is_empty());
        }
    }

    #[test]
    fn resident_ordinary_fields_keep_scalar_updates_and_following_pc() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let profile = CostProfile::start();
        assert_eq!(context.eval("(function(){var o={x:0,u:undefined,n:null,b:true,f:1.5,z:-0};for(var i=0;i<8;i++){o.x=i;o.x+=1;if(o.x!==i+1)return 0;}if(o.u!==undefined||o.n!==null||o.b!==true||o.f!==1.5||!Object.is(o.z,-0))return 0;try{throw o.x}catch(e){return e+34}})()").unwrap(),Value::Int(42));
        let costs = profile.snapshot();
        assert!(
            costs
                .owned_execution_events
                .get("ordinary_field_immediate_read_in_run")
                .copied()
                .unwrap_or(0)
                + costs
                    .owned_execution_events
                    .get("property_ic.hit")
                    .copied()
                    .unwrap_or(0)
                >= 8
        );
        {
            let event = "property_write_ic.hit";
            assert!(
                costs
                    .owned_execution_events
                    .get(event)
                    .copied()
                    .unwrap_or(0)
                    >= 8,
                "{event}: {costs:?}"
            );
        }
        assert!(runtime.0.state.borrow().active_frames.is_empty());
    }

    #[test]
    fn resident_ordinary_field_fallback_preserves_accessor_proxy_strict_and_receiver_rules() {
        for source in [
            "(function(){var n=0,o={get x(){n++;return 41}};return o.x+n})()",
            "(function(){var n=0,o={set x(v){n++;this.y=v}};o.x=41;return o.y+n})()",
            "(function(){var n=0,o=new Proxy({x:40},{get(t,k,r){n++;return Reflect.get(t,k,r)},set(t,k,v,r){n++;return Reflect.set(t,k,v,r)}});o.x=40;return o.x+n})()",
            "(function(){var marker={},n=0,o={get x(){n++;throw marker}};try{o.x;return 0}catch(e){return e===marker&&n===1?42:0}})()",
            "(function(){'use strict';var o=Object.freeze({x:7});try{o.x=17;return 0}catch(e){return e instanceof TypeError&&o.x===7?42:0}})()",
            "(function(){var o=Object.create({x:7});o.x=42;return o.x})()",
            "(function(){var marker={},o={x:marker};if(o.x!==marker)return 0;o.x=42;return o.x})()",
            "(function(){var o={x:42,f(){return this.x}};return o.f()})()",
            "(function(){return ({x:42}).x})()",
        ] {
            let runtime = Runtime::new();
            let mut context = runtime.new_context();
            assert_eq!(context.eval(source).unwrap(), Value::Int(42), "{source}");
            assert!(runtime.0.state.borrow().active_frames.is_empty());
        }
    }

    #[test]
    fn resident_typed_reads_share_decoding_and_following_exception_pc() {
        for kind in [
            "Int8",
            "Uint8",
            "Uint8Clamped",
            "Int16",
            "Uint16",
            "Int32",
            "Uint32",
            "Float16",
            "Float32",
            "Float64",
        ] {
            let runtime = Runtime::new();
            let mut context = runtime.new_context();
            let source = format!(
                "(function(){{var a=new {kind}Array(1),n=0;for(var value of [257.5,-129.5,NaN,Infinity,-0]){{a[0]=value;var x=a[0];if(!Object.is(x,Reflect.get(a,'0')))return 0;n++}}a[0]=37;try{{throw a[0]}}catch(e){{return e+n}}}})()"
            );
            let profile = CostProfile::start();
            assert_eq!(context.eval(&source).unwrap(), Value::Int(42), "{kind}");
            let costs = profile.snapshot();
            assert!(
                costs
                    .owned_execution_events
                    .get("typed_array_number_read_leaf")
                    .copied()
                    .unwrap_or(0)
                    >= 6,
                "{kind}: {costs:?}"
            );
        }
    }

    #[test]
    fn resident_typed_read_fallback_keeps_resizing_conversion_and_proxy_once() {
        for source in [
            "(function(){var b=new ArrayBuffer(4,{maxByteLength:8}),a=new Uint8Array(b,2,2),track=new Uint8Array(b,2);a[0]=42;b.resize(1);if(a[0]!==undefined||track[0]!==undefined)return 0;b.resize(8);a[0]=42;return a[0]===42&&track[0]===42&&track[5]===0?42:0})()",
            "(function(){var a=new Uint8Array([42]),n=0,key={toString(){n++;a.buffer.transfer();return '0'}};return a[key]===undefined&&n===1?42:0})()",
            "(function(){var b=new ArrayBuffer(4,{maxByteLength:8}),a=new Uint8Array(b),n=0,key={toString(){n++;b.resize(0);return '0'}};return a[key]===undefined&&n===1?42:0})()",
            "(function(){var a=new Uint8Array([42]),n=0,marker={},key={toString(){n++;throw marker}};try{a[key];return 0}catch(e){return e===marker&&n===1?42:0}})()",
            "(function(){var n=0,a=new Proxy(new Uint8Array([41]),{get(t,k){n++;return Reflect.get(t,k)}});return a[0]+n})()",
            "(function(){var a=new Uint8Array(new SharedArrayBuffer(1));a[0]=42;return a[0]})()",
            "(function(){var a=new BigInt64Array([42n]);return a[0]===42n?42:0})()",
            "(function(){return new Uint8Array([42])[0]})()",
            "(function(){var a=new Uint8Array([42]);return a[-1]===undefined&&a['-0']===undefined&&a[1]===undefined?42:0})()",
        ] {
            let runtime = Runtime::new();
            let mut context = runtime.new_context();
            assert_eq!(context.eval(source).unwrap(), Value::Int(42), "{source}");
            assert!(runtime.0.state.borrow().active_frames.is_empty());
        }
    }

    #[test]
    fn resident_dense_reads_keep_scalar_results_and_following_pc() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let profile = CostProfile::start();
        assert_eq!(context.eval("(function(){var a=[undefined,null,true,42,1.5,-0];if(a[0]!==undefined||a[1]!==null||a[2]!==true||a[3]!==42||a[4]!==1.5||!Object.is(a[5],-0))return 0;try{var x=a[3];throw x}catch(e){return e}})()").unwrap(), Value::Int(42));
        let costs = profile.snapshot();
        let canonical_reads = costs
            .owned_execution_events
            .get("array_immediate_read_in_run")
            .copied()
            .unwrap_or(0);
        let fused_reads = costs
            .owned_execution_events
            .get("fusion.DenseRead")
            .copied()
            .unwrap_or(0);
        assert!(canonical_reads + fused_reads >= 7, "{costs:?}");
    }

    #[test]
    fn resident_dense_read_fallback_keeps_getter_proxy_and_method_effects_once() {
        for source in [
            "(function(){var n=0,a=[];Object.defineProperty(a,0,{get(){n++;return 41}});return a[0]+n})()",
            "(function(){var n=0,a=new Proxy([41],{get(t,k,r){n++;return Reflect.get(t,k,r)}});return a[0]+n})()",
            "(function(){var n=0,marker={},a=[];Object.defineProperty(a,0,{get(){n++;throw marker}});try{a[0];return 0}catch(e){return n===1&&e===marker?42:0}})()",
            "(function(){var a=[,];Object.setPrototypeOf(a,{0:42});return a[0]})()",
            "(function(){var marker={},a=[marker];return a[0]===marker?42:0})()",
            "(function(){var a=[function(){return this.x}];a.x=42;return a[0]()})()",
            "(function(){var a=[40];a[0]++;return a[0]+1})()",
            "(function(){return [42][0]})()",
        ] {
            let runtime = Runtime::new();
            let mut context = runtime.new_context();
            assert_eq!(context.eval(source).unwrap(), Value::Int(42), "{source}");
            assert!(runtime.0.state.borrow().active_frames.is_empty());
        }
    }

    #[test]
    fn resident_typed_writes_share_numeric_encoding_and_preserve_following_pc() {
        for kind in [
            "Int8",
            "Uint8",
            "Uint8Clamped",
            "Int16",
            "Uint16",
            "Int32",
            "Uint32",
            "Float16",
            "Float32",
            "Float64",
        ] {
            let runtime = Runtime::new();
            let mut context = runtime.new_context();
            let source = format!(
                "(function(){{var C={kind}Array, a=new C(1), b=new C(1),n=0;for(var value of [257.5,-129.5,NaN,Infinity,-0]){{a[0]=value;Reflect.set(b,'0',value);if(!Object.is(a[0],b[0]))return 0;n++}}try{{a[0]=7;throw 35}}catch(e){{return a[0]+e+n}}}})()"
            );
            let profile = CostProfile::start();
            assert_eq!(context.eval(&source).unwrap(), Value::Int(47), "{kind}");
            let costs = profile.snapshot();
            assert!(
                costs
                    .owned_execution_events
                    .get("typed_array_number_write_in_run")
                    .copied()
                    .unwrap_or(0)
                    >= 6,
                "{kind}: {costs:?}"
            );
        }
    }

    #[test]
    fn resident_typed_write_fallback_keeps_conversion_exception_and_receiver_rules() {
        for source in [
            "(function(){var a=new Uint8Array(1),n=0;a[0]={valueOf(){n++;return 42}};return a[0]===42&&n===1?42:0})()",
            "(function(){var a=new Uint8Array(1),marker={},n=0;try{a[0]={valueOf(){n++;throw marker}};return 0}catch(e){return e===marker&&n===1&&a[0]===0?42:0}})()",
            "(function(){'use strict';var a=new Uint8Array(1);a[3]=42;a[-1]=42;a['-0']=42;return a[0]===0&&a[3]===undefined&&a['-0']===undefined?42:0})()",
            "(function(){var a=new Uint8Array(1),n=0;a.buffer.transfer();a[0]=7;a[0]={valueOf(){n++;return 42}};return a[0]===undefined&&n===1?42:0})()",
            "(function(){var a=new Uint8Array(new SharedArrayBuffer(1));a[0]=42;return a[0]})()",
            "(function(){var a=new BigInt64Array(1);try{a[0]=42;return 0}catch(e){a[0]=42n;return e instanceof TypeError&&a[0]===42n?42:0}})()",
            "(function(){var n=0,p=new Proxy(new Uint8Array(1),{set(t,k,v){n++;return Reflect.set(t,k,v,t)}});p[0]=42;return n===1?42:0})()",
        ] {
            let runtime = Runtime::new();
            let mut context = runtime.new_context();
            assert_eq!(context.eval(source).unwrap(), Value::Int(42), "{source}");
            assert!(runtime.0.state.borrow().active_frames.is_empty());
        }
    }

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
        // Both frame PCs are materialized once when the owned loop exits.
        let fault_writes = costs.owned_execution_events["run_frame_fault_pc_write"];
        assert_eq!(fault_writes, 1);
        assert_eq!(costs.owned_execution_events["run_frame_resume_pc_write"], 1);
        // Completion did not change an observable PC after the last publication.
        assert_eq!(
            costs
                .owned_execution_events
                .get("runtime_pc_publication")
                .copied()
                .unwrap_or(0),
            0
        );
        assert!(
            costs.owned_execution_events["slot_authentication"] < 20,
            "{costs:?}"
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
            let _costs = profile.snapshot();
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
    }
}

/// Strict equality has no user conversion. Retain both operands until the
/// result is known, then release them outside the resident instruction match.
#[inline(never)]
pub(super) fn strict_comparison(
    runtime: &crate::engine::api::runtime::Runtime,
    execution: &mut RunningExecution,
    id: FrameId,
    negate: bool,
) -> Result<(), Error> {
    let frame = execution.frames.current_mut(id)?;
    #[cfg(feature = "profiling")]
    let depth = execution.slots.depth(&frame.window);
    let right = execution.slots.pop(&mut frame.window)?;
    let left = execution.slots.pop(&mut frame.window)?;
    let equal = runtime
        .strict_equal_jsvalue(&left, &right)
        .map_err(runtime_error_to_vm_error)?;
    runtime
        .release_jsvalue(left)
        .map_err(runtime_error_to_vm_error)?;
    runtime
        .release_jsvalue(right)
        .map_err(runtime_error_to_vm_error)?;
    execution
        .slots
        .push(&mut frame.window, JsValue::Bool(equal != negate))?;
    frame.resume_pc = frame
        .fault_pc
        .checked_add(1)
        .ok_or_else(|| cold::internal("comparison resume PC overflow"))?;
    #[cfg(feature = "profiling")]
    cold::instruction(depth);
    Ok(())
}

#[cfg(test)]
mod resident_semantics {
    use crate::engine::api::{Runtime, Value};

    #[test]
    fn method_span_resumes_at_later_captured_argument() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        assert_eq!(
            context
                .eval(
                    r#"
                    function call(a) {
                        let captured = 7;
                        function read() { return captured; }
                        return Math.max(a, captured) + read();
                    }
                    Math.max;
                    call(3) === 14 && call(9) === 16 && call(3) === 14
                    "#,
                )
                .unwrap(),
            Value::Bool(true)
        );
        let callable = runtime
            .callable_from_value(context.eval("call").unwrap())
            .unwrap();
        let crate::engine::vm::call::CallableExecution::Bytecode { bytecode, .. } =
            runtime.bytecode_for_callable(&callable).unwrap()
        else {
            panic!("call was not bytecode");
        };
        let executable = runtime.snapshot_function_bytecode(&bytecode).unwrap();
        assert!(
            executable
                .code
                .iter()
                .enumerate()
                .any(|(pc, _)| executable.fusion.method_call(pc) == Some(2))
        );
    }

    #[test]
    fn resident_add_keeps_default_hint_order_and_errors() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        assert_eq!(
            context
                .eval(
                    r#"
          let order=[];
          let a={ [Symbol.toPrimitive](hint){order.push('a:'+hint);return 'x'} };
          let b={ [Symbol.toPrimitive](hint){order.push('b:'+hint);return 2} };
          let result=a+b;
          let threw=false;try { 1n + 2; } catch(e){threw=e instanceof TypeError}
          result==='x2' && order.join(',')==='a:default,b:default' && threw &&
            ('x'+3==='x3') && (2n+3n===5n)
        "#
                )
                .unwrap(),
            Value::Bool(true)
        );
    }

    #[test]
    fn resident_equality_and_not_preserve_values_without_coercion() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        assert_eq!(
            context
                .eval(
                    r#"
          let o={valueOf(){throw 1},toString(){throw 2}};
          let s=Symbol('a');let rope='x';for(let i=0;i<12;i++)rope+=rope;
          let values=[undefined,null,false,true,0,-0,NaN,1,'','x',1n,s,o,rope];
          let ok=true;
          for(let i=0;i<values.length;i++)for(let j=0;j<values.length;j++){
             let a=values[i],b=values[j];
             if ((a===b)!==(Object.is(a,b)||(a===0&&b===0)) && !(a!==a&&b!==b))ok=false;
             if ((a!==b)===(a===b))ok=false;
          }
          ok && !undefined && !null && !false && !0 && !NaN && !'' && !0n &&
            !!o && !!s && !!rope && (rope===rope.slice(0)) && (s!==Symbol('a'))
        "#
                )
                .unwrap(),
            Value::Bool(true)
        );
    }
}
