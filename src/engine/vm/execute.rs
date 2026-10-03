//! The sole execution loop for published `ExecCode` words.
//! A continuation names the semantic operation that must leave the short
//! no-JavaScript frame borrow. The driver owns observable calls and suspension.

use crate::engine::api::{error::Error, runtime::Runtime};
use crate::engine::code::bytecode::{
    ApplyKind, ArgumentsKind, DefineMethodKind, DynamicEnvironmentSource, EvalVariableSource,
    IteratorCallKind, PrivateNameSource, WithObjectSource,
};
use crate::engine::code::exec::PublishedDecoded;
use crate::engine::code::exec_opcode::Opcode;
use crate::engine::code::region::{DirectSource, NumberSource, PublishedNumericRegion};
use crate::engine::heap::runtime::RuntimeState;
use crate::engine::heap::{BytecodeConstant, RawValue};
use crate::engine::numeric_region_miss::NumericRegionMiss as Miss;
use crate::engine::value::JsValue;
use crate::engine::value::number::operations::Number;
use crate::engine::vm::bindings::FrameBinding;
use crate::engine::vm::exception::runtime_error_to_vm_error;
use crate::engine::vm::execution::RunningExecution;
use crate::engine::vm::frame::FrameId;
use crate::engine::vm::frames::ActiveFrameToken;
use crate::engine::vm::stack::{
    DirectSlot, FrameExecution, FrameSlots, FrameTransaction, FrameTurn, NamedReadOperation,
    PropertyReadProgress, StoreProgress, copy_value_in_state,
};

#[cfg(test)]
thread_local! {
    static NUMERIC_REGION_HITS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static NUMERIC_REGION_ATTEMPTS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static NUMERIC_REGION_MISSES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
pub(crate) fn test_numeric_region_counts<T>(run: impl FnOnce() -> T) -> (T, (usize, usize, usize)) {
    let previous = (
        NUMERIC_REGION_ATTEMPTS.replace(0),
        NUMERIC_REGION_HITS.replace(0),
        NUMERIC_REGION_MISSES.replace(0),
    );
    struct Restore((usize, usize, usize));
    impl Drop for Restore {
        fn drop(&mut self) {
            NUMERIC_REGION_ATTEMPTS.set(self.0.0);
            NUMERIC_REGION_HITS.set(self.0.1);
            NUMERIC_REGION_MISSES.set(self.0.2);
        }
    }
    let _restore = Restore(previous);
    let result = run();
    let counts = (
        NUMERIC_REGION_ATTEMPTS.get(),
        NUMERIC_REGION_HITS.get(),
        NUMERIC_REGION_MISSES.get(),
    );
    (result, counts)
}

#[cfg(test)]
pub(crate) fn test_numeric_region_hits<T>(run: impl FnOnce() -> T) -> (T, usize) {
    let previous = NUMERIC_REGION_HITS.replace(0);
    struct Restore(usize);
    impl Drop for Restore {
        fn drop(&mut self) {
            NUMERIC_REGION_HITS.set(self.0);
        }
    }
    let _restore = Restore(previous);
    let result = run();
    let hits = NUMERIC_REGION_HITS.get();
    (result, hits)
}

/// Short-lived access to one frame's slots and execution word cursor. The
/// transaction owns the frame window; `with_slots` ends its borrow before a
/// caller can publish a PC, invoke JavaScript, or perform observable cleanup.
struct FrameCursor<'a> {
    transaction: FrameTransaction<'a>,
    published_fault: &'a mut usize,
    published_resume: &'a mut usize,
    poisoned: &'a std::cell::Cell<bool>,
    fault: usize,
    resume: usize,
}

impl<'a> FrameCursor<'a> {
    fn new(
        transaction: FrameTransaction<'a>,
        fault: &'a mut usize,
        resume: &'a mut usize,
        poisoned: &'a std::cell::Cell<bool>,
    ) -> Self {
        Self {
            transaction,
            fault: *fault,
            resume: *resume,
            published_fault: fault,
            published_resume: resume,
            poisoned,
        }
    }

    fn begin(&mut self) -> usize {
        self.fault = self.resume;
        self.fault
    }

    fn advance(&mut self, next: usize) {
        self.resume = next;
    }

    fn with_slots<T>(
        &mut self,
        operation: impl FnOnce(&mut FrameSlots<'_>) -> Result<T, Error>,
    ) -> Result<T, Error> {
        let mut slots = self.transaction.slots();
        operation(&mut slots)
    }

    fn move_owned(&mut self) -> Result<JsValue, Error> {
        self.with_slots(|slots| slots.pop())
    }

    fn copy_owned(&self, state: &mut RuntimeState, value: &JsValue) -> Result<JsValue, Error> {
        copy_value_in_state(state, value)
    }

    fn commit_push(&mut self, value: JsValue) -> Result<(), Error> {
        self.with_slots(|slots| slots.push(value))
    }

    fn commit_owned(&mut self, state: &mut RuntimeState, value: JsValue) -> Result<(), Error> {
        let mut pending = Some(value);
        let result = self.with_slots(|slots| slots.push_pending(&mut pending));
        if let Some(value) = pending {
            state
                .release_owned_jsvalue(self.poisoned, value)
                .map_err(runtime_error_to_vm_error)?;
        }
        result
    }

    /// Lookup and cleanup use the same state access. Fault publication updates
    /// an already materialized frame; an error materializes the virtual frame
    /// only after this segment returns.
    fn strict_comparison(
        &mut self,
        state: &mut RuntimeState,
        token: ActiveFrameToken,
    ) -> Result<bool, Error> {
        self.publish_fault(state, token)?;
        let equal = self.with_slots(|slots| {
            state
                .strict_equal_jsvalue(slots.peek(1)?, slots.peek(0)?)
                .map_err(runtime_error_to_vm_error)
        })?;
        // Both slots were authenticated above. A comparison error leaves both
        // owners in place for frame cleanup; no owner crosses a fallible lookup.
        let right = self.move_owned()?;
        let left = self.move_owned()?;
        state
            .release_owned_jsvalue(self.poisoned, left)
            .map_err(runtime_error_to_vm_error)?;
        state
            .release_owned_jsvalue(self.poisoned, right)
            .map_err(runtime_error_to_vm_error)?;
        Ok(equal)
    }

    fn publish_fault(
        &mut self,
        state: &mut RuntimeState,
        token: ActiveFrameToken,
    ) -> Result<(), Error> {
        *self.published_fault = self.fault;
        if token.is_materialized() {
            state
                .update_active_bytecode_pc(token, super::BytecodePc::new(self.fault))
                .map_err(runtime_error_to_vm_error)?;
        }
        Ok(())
    }
}

impl Drop for FrameCursor<'_> {
    fn drop(&mut self) {
        *self.published_fault = self.fault;
        *self.published_resume = self.resume;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum BindingSource {
    Closure,
    Local,
    Argument,
}

/// The boundary after the currently decoded operation. Carrying this fact does
/// not commit the frame to advancing past the operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct FallthroughPc(u32);

impl FallthroughPc {
    pub(super) fn from_decoded(decoded: PublishedDecoded<'_>) -> Self {
        Self(decoded.next_pc)
    }

    pub(super) fn index(self) -> usize {
        self.0 as usize
    }
}

#[derive(Clone, Copy)]
struct PendingNamedRead {
    index: u32,
    keep_receiver: bool,
    fallthrough: FallthroughPc,
}

impl PendingNamedRead {
    fn action(self) -> VmAction {
        VmAction::GetField {
            index: self.index,
            keep_receiver: self.keep_receiver,
            fallthrough: self.fallthrough,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum VmAction {
    Import,
    Pure(super::pure_operations::PureOperation),
    ApplyEval(u16),
    Apply(ApplyKind),
    Eval {
        arguments: u16,
        environment: u16,
    },
    Call {
        arguments: u16,
        method: bool,
        tail: bool,
        fallthrough: FallthroughPc,
    },
    SetProperty(Option<u32>),
    GetField {
        index: u32,
        keep_receiver: bool,
        fallthrough: FallthroughPc,
    },
    GetElement {
        keep_receiver: bool,
        keep_key: bool,
        fallthrough: FallthroughPc,
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
        method: Option<(DefineMethodKind, bool)>,
    },
    Environment(super::environment_driver::Operation),
    GetSuper,
    Predicate(super::predicate_driver::Kind),
    HomeObject,
    SuperProperty(super::super_property_driver::Kind),
    ReturnDerived(u16),
    InitDerivedConstructor,
    Construct {
        arguments: u16,
        fallthrough: FallthroughPc,
    },
    ConvertAdd,
    ConvertPlus,
    ConvertPropertyKey,
    NormalizeThis,
    Arguments(ArgumentsKind),
    Rest(u16),
    InstantiateClosure(u32),
    SetName(Option<u32>),
    CloseCaptured(u16),
    ResetCaptured(u16),
    Catch(u32),
    DropCatch,
    NipCatch,
    Throw,
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
        source: PrivateNameSource,
        access: super::private_access::Access,
    },
    Numeric {
        kind: super::numeric::operation::NumericKind,
        fallthrough: FallthroughPc,
    },
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

impl VmAction {
    pub(super) fn observes_activation(&self) -> bool {
        !matches!(self, Self::Call { .. } | Self::Complete)
    }

    #[cfg(feature = "profiling")]
    pub(super) fn diagnostic_name(self) -> &'static str {
        match self {
            Self::Import => "execute.action.import",
            Self::Pure(_) => "execute.action.pure",
            Self::ApplyEval(_) => "execute.action.apply_eval",
            Self::Apply(_) => "execute.action.apply",
            Self::Eval { .. } => "execute.action.eval",
            Self::Call { .. } => "execute.action.call",
            Self::SetProperty(_) => "execute.action.set_property",
            Self::GetField { .. } => "execute.action.get_field",
            Self::GetElement { .. } => "execute.action.get_element",
            Self::InitializeDerived(_) => "execute.action.initialize_derived",
            Self::LexicalUninitialized(_) => "execute.action.lexical_uninitialized",
            Self::Binding { .. } => "execute.action.binding",
            Self::ClassInitializer(_) => "execute.action.class_initializer",
            Self::DefineClass { .. } => "execute.action.define_class",
            Self::DefineProperty { .. } => "execute.action.define_property",
            Self::Environment(_) => "execute.action.environment",
            Self::GetSuper => "execute.action.get_super",
            Self::Predicate(_) => "execute.action.predicate",
            Self::HomeObject => "execute.action.home_object",
            Self::SuperProperty(_) => "execute.action.super_property",
            Self::ReturnDerived(_) => "execute.action.return_derived",
            Self::InitDerivedConstructor => "execute.action.init_derived_constructor",
            Self::Construct { .. } => "execute.action.construct",
            Self::ConvertAdd => "execute.action.convert_add",
            Self::ConvertPlus => "execute.action.convert_plus",
            Self::ConvertPropertyKey => "execute.action.convert_property_key",
            Self::NormalizeThis => "execute.action.normalize_this",
            Self::Arguments(_) => "execute.action.arguments",
            Self::Rest(_) => "execute.action.rest",
            Self::InstantiateClosure(_) => "execute.action.instantiate_closure",
            Self::SetName(_) => "execute.action.set_name",
            Self::CloseCaptured(_) => "execute.action.close_captured",
            Self::ResetCaptured(_) => "execute.action.reset_captured",
            Self::Catch(_) => "execute.action.catch",
            Self::DropCatch => "execute.action.drop_catch",
            Self::NipCatch => "execute.action.nip_catch",
            Self::Throw => "execute.action.throw",
            Self::Materialize => "execute.action.materialize",
            Self::BindingError { .. } => "execute.action.binding_error",
            Self::PrivateInitialize { .. } => "execute.action.private_initialize",
            Self::PrivateAccess { .. } => "execute.action.private_access",
            Self::Numeric { .. } => "execute.action.numeric",
            Self::ForIn(_) => "execute.action.for_in",
            Self::CopyData { .. } => "execute.action.copy_data",
            #[cfg(all(test, feature = "profiling"))]
            Self::ReleaseOperand { .. } => "execute.action.release_operand",
            Self::Complete => "execute.action.complete",
            Self::Suspend(_) => "execute.action.suspend",
            Self::Bridge => "execute.action.bridge",
        }
    }
}

#[cfg(test)]
pub(super) fn execute_frame(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    id: FrameId,
) -> Result<VmAction, Error> {
    let mut state = runtime.0.state.borrow_mut();
    execute_frame_in_state(runtime, &mut state, execution, id)
}

pub(super) fn execute_frame_in_state(
    runtime: &Runtime,
    state: &mut RuntimeState,
    execution: &mut RunningExecution,
    id: FrameId,
) -> Result<VmAction, Error> {
    #[cfg(feature = "profiling")]
    crate::engine::api::profiling::record_owned_execution_event("core.frame_executor_entry");
    let mut segment = FrameExecution::admit(execution, id)?;
    loop {
        let action = {
            let FrameTurn {
                property_generation,
                active_frame,
                owners,
                executable,
                transaction,
                fault_pc,
                resume_pc,
                pending,
                selected_named_read,
                selected_native,
                ..
            } = segment.frame();
            #[cfg(feature = "profiling")]
            crate::engine::api::profiling::record_execution_static_in_state(
                runtime, state, executable,
            );
            let mut cursor =
                FrameCursor::new(transaction, fault_pc, resume_pc, &runtime.0.poisoned);
            // End the actual current frame projection before installing or
            // retiring a frame; all opcodes share this one dispatch loop.
            'dispatch: loop {
                let pc = cursor.begin();
                #[cfg(feature = "profiling")]
                crate::engine::api::profiling::record_numeric_rejection_visit(
                    runtime, executable, pc,
                );
                let decoded = executable
                    .exec
                    .decode_published(pc as u32)
                    .map_err(|_| Error::internal("published execution word is invalid"))?;
                let mut next = decoded.next_pc as usize;
                let operand = decoded.operand(0);
                match decoded.opcode {
                    Opcode::Nop | Opcode::MarkSuperCall => {}
                    Opcode::PushI32 => {
                        let value = if decoded.next_pc == pc as u32 + 1 {
                            i32::from(operand as i16)
                        } else {
                            operand as i32
                        };
                        cursor.commit_push(JsValue::Int(value))?;
                    }
                    Opcode::Undefined => cursor.commit_push(JsValue::Undefined)?,
                    Opcode::Null => cursor.commit_push(JsValue::Null)?,
                    Opcode::PushFalse => cursor.commit_push(JsValue::Bool(false))?,
                    Opcode::PushTrue => cursor.commit_push(JsValue::Bool(true))?,
                    Opcode::PushConst => match executable.constant(operand) {
                        Some(BytecodeConstant::Value(RawValue::Int(value))) => {
                            cursor.commit_push(JsValue::Int(*value))?
                        }
                        Some(BytecodeConstant::Value(RawValue::Float(value))) => {
                            cursor.commit_push(JsValue::Float(*value))?
                        }
                        Some(BytecodeConstant::Value(RawValue::Undefined)) => {
                            cursor.commit_push(JsValue::Undefined)?
                        }
                        Some(BytecodeConstant::Value(RawValue::Null)) => {
                            cursor.commit_push(JsValue::Null)?
                        }
                        Some(BytecodeConstant::Value(RawValue::Bool(value))) => {
                            cursor.commit_push(JsValue::Bool(*value))?
                        }
                        Some(BytecodeConstant::Value(RawValue::ShortBigInt(value))) => {
                            cursor.commit_push(JsValue::ShortBigInt(*value))?
                        }
                        Some(BytecodeConstant::Value(RawValue::String(value))) => {
                            let owned = cursor.copy_owned(state, &JsValue::String(*value))?;
                            cursor.commit_owned(state, owned)?;
                        }
                        Some(BytecodeConstant::Value(RawValue::BigInt(value))) => {
                            let owned = cursor.copy_owned(state, &JsValue::BigInt(*value))?;
                            cursor.commit_owned(state, owned)?;
                        }
                        _ => {
                            break 'dispatch Ok(VmAction::Pure(
                                super::pure_operations::PureOperation::Constant(operand),
                            ));
                        }
                    },
                    Opcode::PushThis | Opcode::BorrowedFieldThis => {
                        let normalized = owners
                            .rare
                            .get()
                            .and_then(|rare| rare.normalized_this.as_ref());
                        if decoded.opcode == Opcode::BorrowedFieldThis {
                            let base = normalized.unwrap_or(&owners.input.this_value);
                            // Capture only the field key: capturing PublishedDecoded here
                            // duplicates its aggregate in the shared dispatch loop.
                            let field_index = decoded.operand(1);
                            let selected = cursor.with_slots(|slots| {
                                if !slots.has_operand_capacity(1)
                                    || !borrowed_this_read_ready_in_state(state, base)
                                {
                                    return Ok(None);
                                }
                                let mut miss =
                                    crate::engine::object::NamedSelectionMiss::ContinueLookup;
                                let value = state.select_linked_data_into(
                                    runtime.domain_id(),
                                    base,
                                    executable,
                                    next,
                                    field_index,
                                    true,
                                    &mut None,
                                    &mut miss,
                                );
                                Ok(value.or_else(|| {
                                    matches!(
                                        miss,
                                        crate::engine::object::NamedSelectionMiss::CompleteAbsent
                                    )
                                    .then_some(JsValue::Undefined)
                                }))
                            })?;
                            #[cfg(feature = "profiling")]
                            crate::engine::api::profiling::record_execution_outcome(
                                runtime,
                                executable,
                                pc,
                                "borrowed_this_field",
                                if selected.is_some() {
                                    None
                                } else {
                                    Some("guard")
                                },
                            );
                            if let Some(value) = selected {
                                cursor.commit_owned(state, value)?;
                                #[cfg(feature = "profiling")]
                                crate::engine::api::profiling::record_owned_execution_event(
                                    "local_completion.borrowed_this_read",
                                );
                                cursor.advance(decoded.operand(2) as usize);
                                continue;
                            }
                        }
                        let value = if let Some(value) = normalized {
                            cursor.copy_owned(state, value)?
                        } else if executable.metadata.strict
                            || matches!(owners.input.this_value, JsValue::Object(_))
                        {
                            cursor.copy_owned(state, &owners.input.this_value)?
                        } else if matches!(
                            owners.input.this_value,
                            JsValue::Undefined | JsValue::Null
                        ) {
                            let global = owners
                                .input
                                .callee_global_in_state(state, executable.realm)?;
                            cursor.copy_owned(state, &JsValue::Object(global))?
                        } else {
                            break 'dispatch Ok(VmAction::NormalizeThis);
                        };
                        cursor.commit_owned(state, value)?;
                    }
                    Opcode::PushNewTarget => {
                        let value = cursor.copy_owned(state, &owners.input.new_target)?;
                        cursor.commit_owned(state, value)?;
                    }
                    Opcode::PushActiveFunction => {
                        let object = owners.function.object_id();
                        state
                            .heap
                            .retain_object(object)
                            .map_err(|error| Error::internal(error.to_string()))?;
                        cursor.commit_owned(state, JsValue::Object(object))?;
                    }
                    Opcode::CheckCtor => {
                        if matches!(owners.input.new_target, JsValue::Undefined) {
                            break 'dispatch Ok(VmAction::Pure(
                                super::pure_operations::PureOperation::ConstructorWithoutNew,
                            ));
                        }
                    }
                    Opcode::NumberLocalInc => {
                        let index = published_u16(operand);
                        #[cfg(feature = "profiling")]
                        crate::engine::api::profiling::record_execution_dispatch(
                            runtime, executable, pc, true,
                        );
                        let result = cursor.with_slots(|slots| {
                            let Some(value) = slots.immediate_local(index) else {
                                return Ok(None);
                            };
                            if !slots.has_operand_capacity(2) {
                                return Ok(None);
                            }
                            Ok(Some(value.add(Number::Int(1))))
                        })?;
                        if let Some(value) = result {
                            #[cfg(feature = "profiling")]
                            crate::engine::api::profiling::record_execution_outcome(
                                runtime,
                                executable,
                                pc,
                                "number_local_inc",
                                None,
                            );
                            cursor.commit_push(number_value(value))?;
                            cursor.advance(next + 2);
                            continue;
                        }
                        #[cfg(feature = "profiling")]
                        crate::engine::api::profiling::record_execution_outcome(
                            runtime,
                            executable,
                            pc,
                            "number_local_inc",
                            Some("guard"),
                        );
                        if let Some(action) = read_local::<false>(&mut cursor, state, index)? {
                            break 'dispatch Ok(action);
                        }
                    }
                    Opcode::NumericArrayAccumulate
                    | Opcode::NumericArrayStoreProduct
                    | Opcode::NumericArrayCopyElement
                    | Opcode::NumericArrayAddPreInc
                    | Opcode::NumericArrayStoreAndLocal
                    | Opcode::NumericArrayUpdateElement
                    | Opcode::NumericArrayCompareBranch => {
                        let region = executable.exec.numeric_region(operand).ok_or_else(|| {
                            Error::internal("published numeric region is missing")
                        })?;
                        #[cfg(test)]
                        NUMERIC_REGION_ATTEMPTS.set(NUMERIC_REGION_ATTEMPTS.get() + 1);
                        #[cfg(feature = "profiling")]
                        crate::engine::api::profiling::record_execution_dispatch(
                            runtime, executable, pc, true,
                        );
                        let mut miss_reason = None;
                        let hit = match decoded.opcode {
                            Opcode::NumericArrayAccumulate | Opcode::NumericArrayStoreProduct => {
                                match cursor.with_slots(|slots| {
                                    Ok(numeric_local_array_region(
                                        slots,
                                        state,
                                        region,
                                        decoded.opcode == Opcode::NumericArrayAccumulate,
                                    ))
                                })? {
                                    Ok(()) => true,
                                    Err(reason) => {
                                        miss_reason = Some(reason);
                                        false
                                    }
                                }
                            }
                            Opcode::NumericArrayUpdateElement => {
                                let generation = property_generation.checked_add(1);
                                let result = cursor.with_slots(|slots| {
                                    if generation.is_none() {
                                        return Ok(Err(Miss::PropertyGenerationOverflow));
                                    }
                                    if !slots.has_operand_capacity(region.peak as usize) {
                                        return Ok(Err(Miss::OperandCapacity));
                                    }
                                    let Some(index) =
                                        region_number(slots, region.index).and_then(array_index)
                                    else {
                                        return Ok(Err(Miss::IndexNotNumericInteger));
                                    };
                                    let delta = if let Some(producer) = region.producer_index {
                                        let product = executable
                                            .exec
                                            .product_source(producer)
                                            .expect("verified product payload");
                                        let Some(scale) = region_number(slots, product.scale)
                                        else {
                                            return Ok(Err(Miss::ScaleNotNumber));
                                        };
                                        let product_index = if region.shared_update_index {
                                            index
                                        } else {
                                            let Some(product_index) =
                                                region_number(slots, product.index)
                                                    .and_then(array_index)
                                            else {
                                                return Ok(Err(Miss::IndexNotNumericInteger));
                                            };
                                            product_index
                                        };
                                        let Some(product_base) =
                                            slots.direct_value(region_direct_slot(product.array))
                                        else {
                                            return Ok(Err(Miss::ReceiverBindingUnavailable));
                                        };
                                        let element = match state
                                            .peek_dense_number_result(product_base, product_index)
                                        {
                                            Ok(value) => value,
                                            Err(reason) => return Ok(Err(reason)),
                                        };
                                        element.mul(scale)
                                    } else {
                                        let Some(delta) = region_number(slots, region.value) else {
                                            return Ok(Err(Miss::DeltaNotNumber));
                                        };
                                        delta
                                    };
                                    let Some(base) =
                                        slots.direct_value(region_direct_slot(region.array))
                                    else {
                                        return Ok(Err(Miss::ReceiverBindingUnavailable));
                                    };
                                    Ok(state.try_add_array_own_number(base, index, delta))
                                })?;
                                match result {
                                    Ok(()) => {
                                        *property_generation = generation.unwrap();
                                        true
                                    }
                                    Err(reason) => {
                                        miss_reason = Some(reason);
                                        false
                                    }
                                }
                            }
                            Opcode::NumericArrayCopyElement => {
                                let generation = property_generation.checked_add(1);
                                let result = cursor.with_slots(|slots| {
                                    if generation.is_none() {
                                        return Ok(Err(Miss::PropertyGenerationOverflow));
                                    }
                                    if !slots.has_operand_capacity(region.peak as usize) {
                                        return Ok(Err(Miss::OperandCapacity));
                                    }
                                    let Some(index) =
                                        region_number(slots, region.index).and_then(array_index)
                                    else {
                                        return Ok(Err(Miss::IndexNotNumericInteger));
                                    };
                                    let source = executable
                                        .exec
                                        .copy_source(
                                            region
                                                .producer_index
                                                .expect("verified copy payload index"),
                                        )
                                        .expect("verified copy payload");
                                    let Some(source_index) =
                                        region_number(slots, source.index).and_then(array_index)
                                    else {
                                        return Ok(Err(Miss::IndexNotNumericInteger));
                                    };
                                    let Some(source_base) =
                                        slots.direct_value(region_direct_slot(source.array))
                                    else {
                                        return Ok(Err(Miss::ReceiverBindingUnavailable));
                                    };
                                    let value = match state
                                        .peek_dense_number_result(source_base, source_index)
                                    {
                                        Ok(value) => value,
                                        Err(reason) => return Ok(Err(reason)),
                                    };
                                    let Some(target_base) =
                                        slots.direct_value(region_direct_slot(region.array))
                                    else {
                                        return Ok(Err(Miss::ReceiverBindingUnavailable));
                                    };
                                    Ok(state.try_replace_array_own_number(
                                        target_base,
                                        index,
                                        value,
                                    ))
                                })?;
                                match result {
                                    Ok(()) => {
                                        *property_generation = generation.unwrap();
                                        true
                                    }
                                    Err(reason) => {
                                        miss_reason = Some(reason);
                                        false
                                    }
                                }
                            }
                            Opcode::NumericArrayAddPreInc => {
                                let result =
                                    cursor.with_slots(|slots| {
                                        if !slots.has_operand_capacity(region.peak as usize) {
                                            return Ok(Err(Miss::OperandCapacity));
                                        }
                                        let Some(accumulator) =
                                            slots.peek(0).ok().and_then(JsValue::as_number_repr)
                                        else {
                                            return Ok(Err(Miss::AccumulatorNotNumber));
                                        };
                                        let index_slot = match region.index {
                                            NumberSource::Direct(DirectSource::Local(slot))
                                            | NumberSource::Direct(DirectSource::CheckedLocal(
                                                slot,
                                            )) => slot,
                                            _ => unreachable!("verified preincrement index"),
                                        };
                                        let Some(old_index) = slots.immediate_local(index_slot)
                                        else {
                                            return Ok(Err(Miss::IndexNotNumber));
                                        };
                                        let updated_index = old_index.add(Number::Int(1));
                                        let Some(index) = array_index(updated_index) else {
                                            return Ok(Err(Miss::IndexNotNumericInteger));
                                        };
                                        let Some(base) =
                                            slots.direct_value(region_direct_slot(region.array))
                                        else {
                                            return Ok(Err(Miss::ReceiverBindingUnavailable));
                                        };
                                        let element =
                                            match state.peek_dense_number_result(base, index) {
                                                Ok(value) => value,
                                                Err(reason) => return Ok(Err(reason)),
                                            };
                                        slots.commit_number_local_and_top(
                                            index_slot,
                                            updated_index,
                                            accumulator.add(element),
                                        )?;
                                        Ok(Ok(()))
                                    })?;
                                match result {
                                    Ok(()) => true,
                                    Err(reason) => {
                                        miss_reason = Some(reason);
                                        false
                                    }
                                }
                            }
                            Opcode::NumericArrayStoreAndLocal => {
                                cursor.publish_fault(state, active_frame)?;
                                let generation = property_generation.checked_add(1);
                                let result = cursor.with_slots(|slots| {
                                    if generation.is_none() {
                                        return Ok(Err(Miss::PropertyGenerationOverflow));
                                    }
                                    if !slots.has_operand_capacity(region.peak as usize) {
                                        return Ok(Err(Miss::OperandCapacity));
                                    }
                                    let Some(value) = slots.peek(0)?.as_number_repr() else {
                                        return Ok(Err(Miss::DeltaNotNumber));
                                    };
                                    let Some(index) =
                                        slots.peek(1)?.as_number_repr().and_then(array_index)
                                    else {
                                        return Ok(Err(Miss::IndexNotNumericInteger));
                                    };
                                    let eligible = matches!(
                                        slots.local(region.destination)?,
                                        FrameBinding::Direct(
                                            JsValue::Undefined
                                                | JsValue::Null
                                                | JsValue::Bool(_)
                                                | JsValue::Int(_)
                                                | JsValue::Float(_)
                                        )
                                    );
                                    if !eligible {
                                        return Ok(Err(Miss::DestinationOrReceiverUnavailable));
                                    }
                                    let base = slots.peek(2)?;
                                    if let Err(reason) =
                                        state.try_replace_array_own_number(base, index, value)
                                    {
                                        return Ok(Err(reason));
                                    }
                                    let old = slots.replace_local(
                                        region.destination,
                                        FrameBinding::Direct(number_value(value)),
                                    )?;
                                    debug_assert!(matches!(
                                        old,
                                        FrameBinding::Direct(
                                            JsValue::Undefined
                                                | JsValue::Null
                                                | JsValue::Bool(_)
                                                | JsValue::Int(_)
                                                | JsValue::Float(_)
                                        )
                                    ));
                                    let _value = slots.pop()?;
                                    let _index = slots.pop()?;
                                    Ok(Ok(slots.pop()?))
                                })?;
                                match result {
                                    Ok(base) => {
                                        *property_generation = generation.unwrap();
                                        state
                                            .release_owned_jsvalue(&runtime.0.poisoned, base)
                                            .map_err(runtime_error_to_vm_error)?;
                                        true
                                    }
                                    Err(reason) => {
                                        miss_reason = Some(reason);
                                        false
                                    }
                                }
                            }
                            Opcode::NumericArrayCompareBranch => {
                                let result = cursor.with_slots(|slots| {
                                    if !slots.has_operand_capacity(region.peak as usize) {
                                        return Ok(Err(Miss::OperandCapacity));
                                    }
                                    let Some(index) =
                                        region_number(slots, region.index).and_then(array_index)
                                    else {
                                        return Ok(Err(Miss::IndexNotNumericInteger));
                                    };
                                    let Some(rhs) = region_number(slots, region.value) else {
                                        return Ok(Err(Miss::RhsNotNumber));
                                    };
                                    let Some(base) =
                                        slots.direct_value(region_direct_slot(region.array))
                                    else {
                                        return Ok(Err(Miss::ReceiverBindingUnavailable));
                                    };
                                    Ok(state.peek_dense_number_result(base, index).map(|left| {
                                        compare_direct_numbers(region.comparison as u16, left, rhs)
                                    }))
                                })?;
                                if let Ok(decision) = result {
                                    #[cfg(feature = "profiling")]
                                    crate::engine::api::profiling::record_execution_outcome(
                                        runtime,
                                        executable,
                                        pc,
                                        "numeric_array_compare_branch",
                                        None,
                                    );
                                    #[cfg(test)]
                                    NUMERIC_REGION_HITS.set(NUMERIC_REGION_HITS.get() + 1);
                                    let target = if decision == region.when_true {
                                        decoded.operand(1) as usize
                                    } else {
                                        region.fallthrough_pc as usize
                                    };
                                    cursor.advance(target);
                                    continue;
                                }
                                miss_reason = result.err();
                                false
                            }
                            _ => unreachable!("numeric region opcode was already selected"),
                        };
                        #[cfg(feature = "profiling")]
                        crate::engine::api::profiling::record_execution_outcome(
                            runtime,
                            executable,
                            pc,
                            match decoded.opcode {
                                Opcode::NumericArrayAccumulate => "numeric_array_accumulate",
                                Opcode::NumericArrayStoreProduct => "numeric_array_store_product",
                                Opcode::NumericArrayCopyElement => "numeric_array_copy_element",
                                Opcode::NumericArrayAddPreInc => "numeric_array_add_preinc",
                                Opcode::NumericArrayStoreAndLocal => {
                                    "numeric_array_store_and_local"
                                }
                                Opcode::NumericArrayUpdateElement => "numeric_array_update_element",
                                Opcode::NumericArrayCompareBranch => "numeric_array_compare_branch",
                                _ => unreachable!(),
                            },
                            if hit {
                                None
                            } else {
                                miss_reason.map(Miss::name)
                            },
                        );
                        #[cfg(not(feature = "profiling"))]
                        let _ = miss_reason;
                        if hit {
                            #[cfg(test)]
                            NUMERIC_REGION_HITS.set(NUMERIC_REGION_HITS.get() + 1);
                            cursor.advance(decoded.operand(1) as usize);
                            continue;
                        }
                        #[cfg(test)]
                        NUMERIC_REGION_MISSES.set(NUMERIC_REGION_MISSES.get() + 1);
                        let first = match decoded.opcode {
                            Opcode::NumericArrayStoreAndLocal => {
                                cursor.with_slots(|slots| {
                                    slots.peek(2)?;
                                    slots.insert_copy_in_state(state, 0, 3)
                                })?;
                                None
                            }
                            Opcode::NumericArrayAccumulate => {
                                if region.checked {
                                    read_local::<true>(&mut cursor, state, region.destination)?
                                } else {
                                    read_local::<false>(&mut cursor, state, region.destination)?
                                }
                            }
                            _ => match region.array {
                                DirectSource::Local(index) => {
                                    read_local::<false>(&mut cursor, state, index)?
                                }
                                DirectSource::CheckedLocal(index) => {
                                    read_local::<true>(&mut cursor, state, index)?
                                }
                                DirectSource::Argument(index) => {
                                    read_arg(&mut cursor, state, index)?
                                }
                            },
                        };
                        if let Some(action) = first {
                            break 'dispatch Ok(action);
                        }
                        cursor.advance(decoded.operand(2) as usize);
                        continue;
                    }
                    Opcode::UpdateLocalDiscard | Opcode::UpdateLocalDiscardCheck => {
                        let index = published_u16(operand & 0x1fff);
                        let descriptor = operand >> 13;
                        #[cfg(feature = "profiling")]
                        crate::engine::api::profiling::record_execution_dispatch(
                            runtime, executable, pc, true,
                        );
                        let hit = cursor.with_slots(|slots| {
                            if !slots.has_operand_capacity(if descriptor & 2 != 0 { 2 } else { 1 })
                            {
                                return Ok(false);
                            }
                            let Some(old) = slots
                                .direct_value(DirectSlot::Local(index))
                                .and_then(JsValue::as_number_repr)
                            else {
                                return Ok(false);
                            };
                            slots.commit_number_local_discard(
                                index,
                                old.update(descriptor & 1 != 0),
                            )?;
                            Ok(true)
                        })?;
                        #[cfg(feature = "profiling")]
                        crate::engine::api::profiling::record_execution_outcome(
                            runtime,
                            executable,
                            pc,
                            "update_local_discard",
                            if hit { None } else { Some("guard") },
                        );
                        if hit {
                            cursor.advance(
                                (decoded.next_pc + if descriptor & 4 != 0 { 3 } else { 2 })
                                    as usize,
                            );
                            continue;
                        }
                        if decoded.opcode == Opcode::UpdateLocalDiscardCheck {
                            if let Some(action) = read_local::<true>(&mut cursor, state, index)? {
                                break 'dispatch Ok(action);
                            }
                        } else if let Some(action) = read_local::<false>(&mut cursor, state, index)?
                        {
                            break 'dispatch Ok(action);
                        }
                    }
                    Opcode::GetLocal => {
                        let index = published_u16(operand);
                        #[cfg(feature = "profiling")]
                        crate::engine::api::profiling::record_execution_dispatch(
                            runtime, executable, pc, false,
                        );
                        if let Some(action) = read_local::<false>(&mut cursor, state, index)? {
                            break 'dispatch Ok(action);
                        }
                    }
                    Opcode::GetLocalCheck => {
                        let index = published_u16(operand);
                        #[cfg(feature = "profiling")]
                        crate::engine::api::profiling::record_execution_dispatch(
                            runtime, executable, pc, false,
                        );
                        if let Some(action) = read_local::<true>(&mut cursor, state, index)? {
                            break 'dispatch Ok(action);
                        }
                    }
                    Opcode::NumberArgInc => {
                        let index = published_u16(operand);
                        #[cfg(feature = "profiling")]
                        crate::engine::api::profiling::record_execution_dispatch(
                            runtime, executable, pc, true,
                        );
                        let result = cursor.with_slots(|slots| {
                            let Some(value) = slots.immediate_parameter(index) else {
                                return Ok(None);
                            };
                            if !slots.has_operand_capacity(2) {
                                return Ok(None);
                            }
                            Ok(Some(value.add(Number::Int(1))))
                        })?;
                        if let Some(value) = result {
                            #[cfg(feature = "profiling")]
                            crate::engine::api::profiling::record_execution_outcome(
                                runtime,
                                executable,
                                pc,
                                "number_arg_inc",
                                None,
                            );
                            cursor.commit_push(number_value(value))?;
                            cursor.advance(next + 2);
                            continue;
                        }
                        #[cfg(feature = "profiling")]
                        crate::engine::api::profiling::record_execution_outcome(
                            runtime,
                            executable,
                            pc,
                            "number_arg_inc",
                            Some("guard"),
                        );
                        if let Some(action) = read_arg(&mut cursor, state, index)? {
                            break 'dispatch Ok(action);
                        }
                    }
                    Opcode::DensePreUpdateLocal
                    | Opcode::DensePreUpdateArg
                    | Opcode::DensePostUpdateLocal
                    | Opcode::DensePostUpdateLocalCheck
                    | Opcode::DensePostUpdateArg => {
                        let base_index = published_u16(operand);
                        let postfix = matches!(
                            decoded.opcode,
                            Opcode::DensePostUpdateLocal
                                | Opcode::DensePostUpdateLocalCheck
                                | Opcode::DensePostUpdateArg
                        );
                        #[cfg(feature = "profiling")]
                        crate::engine::api::profiling::record_execution_dispatch(
                            runtime, executable, pc, true,
                        );
                        let update = decoded.operand(1);
                        let update_index = published_u16(update & 0xffff);
                        let base = if matches!(
                            decoded.opcode,
                            Opcode::DensePreUpdateLocal
                                | Opcode::DensePostUpdateLocal
                                | Opcode::DensePostUpdateLocalCheck
                        ) {
                            DirectSlot::Local(base_index)
                        } else {
                            DirectSlot::Argument(base_index)
                        };
                        let update_slot = if update & 0x1_0000 != 0 {
                            DirectSlot::Argument(update_index)
                        } else {
                            DirectSlot::Local(update_index)
                        };
                        let increment = update & 0x2_0000 == 0;
                        let hit = cursor.with_slots(|slots| {
                            if !slots.has_operand_capacity(if postfix { 3 } else { 2 }) {
                                return Ok(false);
                            }
                            let Some(old) = slots
                                .direct_value(update_slot)
                                .and_then(JsValue::as_number_repr)
                            else {
                                return Ok(false);
                            };
                            let updated = old.update(increment);
                            let key = if postfix { old } else { updated };
                            let Number::Int(index) = key else {
                                return Ok(false);
                            };
                            let Ok(index) = u32::try_from(index) else {
                                return Ok(false);
                            };
                            let Some(base) = slots.direct_value(base) else {
                                return Ok(false);
                            };
                            let Some(read) = state.peek_dense_number(base, index) else {
                                return Ok(false);
                            };
                            slots.commit_numeric_update_and_read(update_slot, updated, read)?;
                            Ok(true)
                        })?;
                        #[cfg(feature = "profiling")]
                        crate::engine::api::profiling::record_execution_outcome(
                            runtime,
                            executable,
                            pc,
                            if postfix {
                                "dense_post_update"
                            } else {
                                "dense_pre_update"
                            },
                            if hit { None } else { Some("guard") },
                        );
                        if hit {
                            cursor.advance(decoded.operand(2) as usize);
                            continue;
                        }
                        if decoded.opcode == Opcode::DensePostUpdateLocalCheck {
                            if let Some(action) =
                                read_local::<true>(&mut cursor, state, base_index)?
                            {
                                break 'dispatch Ok(action);
                            }
                        } else if matches!(
                            decoded.opcode,
                            Opcode::DensePreUpdateLocal | Opcode::DensePostUpdateLocal
                        ) {
                            if let Some(action) =
                                read_local::<false>(&mut cursor, state, base_index)?
                            {
                                break 'dispatch Ok(action);
                            }
                        } else if let Some(action) = read_arg(&mut cursor, state, base_index)? {
                            break 'dispatch Ok(action);
                        }
                    }
                    Opcode::FieldAccSetDrop => {
                        let accumulator = published_u16(operand);
                        let packed = decoded.operand(1);
                        let base_index = published_u16(packed & 0xffff);
                        let field_index = packed >> 16;
                        #[cfg(feature = "profiling")]
                        crate::engine::api::profiling::record_execution_dispatch(
                            runtime, executable, pc, true,
                        );
                        let hit = cursor.with_slots(|slots| {
                            if !slots.has_operand_capacity(2) {
                                return Ok(false);
                            }
                            let Some(sum) = slots
                                .direct_value(DirectSlot::Local(accumulator))
                                .and_then(JsValue::as_number_repr)
                            else {
                                return Ok(false);
                            };
                            let Some(JsValue::Object(base)) =
                                slots.direct_value(DirectSlot::Local(base_index))
                            else {
                                return Ok(false);
                            };
                            let Some(read) = state.property_ic_peek_number(
                                runtime.domain_id(),
                                *base,
                                executable,
                                next + 1,
                                field_index,
                            ) else {
                                return Ok(false);
                            };
                            slots.commit_number_local_discard(accumulator, sum.add(read))?;
                            Ok(true)
                        })?;
                        #[cfg(feature = "profiling")]
                        crate::engine::api::profiling::record_execution_outcome(
                            runtime,
                            executable,
                            pc,
                            "field_acc_set_drop",
                            if hit { None } else { Some("guard") },
                        );
                        if hit {
                            cursor.advance((decoded.next_pc + 5) as usize);
                            continue;
                        }
                        if let Some(action) = read_local::<false>(&mut cursor, state, accumulator)?
                        {
                            break 'dispatch Ok(action);
                        }
                    }
                    Opcode::DenseAccIndexSetDrop => {
                        let accumulator = published_u16(operand);
                        let packed = decoded.operand(1);
                        let base_index = published_u16(packed & 0xffff);
                        let key_index = published_u16(packed >> 16);
                        let mask = i32::from(decoded.operand(2) as u16 as i16);
                        #[cfg(feature = "profiling")]
                        crate::engine::api::profiling::record_execution_dispatch(
                            runtime, executable, pc, true,
                        );
                        let hit = cursor.with_slots(|slots| {
                            if !slots.has_operand_capacity(4) {
                                return Ok(false);
                            }
                            let Some(sum) = slots
                                .direct_value(DirectSlot::Local(accumulator))
                                .and_then(JsValue::as_number_repr)
                            else {
                                return Ok(false);
                            };
                            let Some(key) = slots
                                .direct_value(DirectSlot::Local(key_index))
                                .and_then(JsValue::as_number_repr)
                            else {
                                return Ok(false);
                            };
                            let Ok(index) = u32::try_from(key.int32() & mask) else {
                                return Ok(false);
                            };
                            let Some(base) = slots.direct_value(DirectSlot::Local(base_index))
                            else {
                                return Ok(false);
                            };
                            let Some(read) = state.peek_dense_number(base, index) else {
                                return Ok(false);
                            };
                            slots.commit_number_local_discard(accumulator, sum.add(read))?;
                            Ok(true)
                        })?;
                        #[cfg(feature = "profiling")]
                        crate::engine::api::profiling::record_execution_outcome(
                            runtime,
                            executable,
                            pc,
                            "dense_acc_index_set_drop",
                            if hit { None } else { Some("guard") },
                        );
                        if hit {
                            cursor.advance((decoded.next_pc + 8) as usize);
                            continue;
                        }
                        if let Some(action) = read_local::<false>(&mut cursor, state, accumulator)?
                        {
                            break 'dispatch Ok(action);
                        }
                    }
                    Opcode::DenseIndexBinaryLocal | Opcode::DenseIndexBinaryArg => {
                        let base_index = published_u16(operand);
                        let slots_operand = decoded.operand(1);
                        let descriptor = decoded.operand(2);
                        let operation = Opcode::from_raw((descriptor & 0x3ff) as u16)
                            .expect("published numeric operation was verified");
                        let key_index = published_u16(slots_operand & 0xffff);
                        let rhs_bits = published_u16(slots_operand >> 16);
                        let base = if decoded.opcode == Opcode::DenseIndexBinaryLocal {
                            DirectSlot::Local(base_index)
                        } else {
                            DirectSlot::Argument(base_index)
                        };
                        let key = if descriptor & 0x400 != 0 {
                            DirectSlot::Argument(key_index)
                        } else {
                            DirectSlot::Local(key_index)
                        };
                        let rhs_mode = (descriptor >> 11) & 3;
                        #[cfg(feature = "profiling")]
                        crate::engine::api::profiling::record_execution_dispatch(
                            runtime, executable, pc, true,
                        );
                        let read = cursor.with_slots(|slots| {
                            Ok(try_dense_index_binary(
                                slots, state, base, key, rhs_mode, rhs_bits, operation,
                            ))
                        })?;
                        #[cfg(feature = "profiling")]
                        crate::engine::api::profiling::record_execution_outcome(
                            runtime,
                            executable,
                            pc,
                            "dense_index_binary",
                            if read.is_some() { None } else { Some("guard") },
                        );
                        if let Some(value) = read {
                            cursor.commit_push(number_value(value))?;
                            cursor.advance((decoded.next_pc + 4) as usize);
                            continue;
                        }
                        if decoded.opcode == Opcode::DenseIndexBinaryLocal {
                            if let Some(action) =
                                read_local::<false>(&mut cursor, state, base_index)?
                            {
                                break 'dispatch Ok(action);
                            }
                        } else if let Some(action) = read_arg(&mut cursor, state, base_index)? {
                            break 'dispatch Ok(action);
                        }
                    }
                    Opcode::DenseReadBinaryLocal | Opcode::DenseReadBinaryArg => {
                        let base_index = published_u16(operand);
                        let slots_operand = decoded.operand(1);
                        let descriptor = decoded.operand(2);
                        let operation = Opcode::from_raw((descriptor & 0x3ff) as u16)
                            .expect("published numeric operation was verified");
                        let key_index = published_u16(slots_operand & 0xffff);
                        let rhs_index = published_u16(slots_operand >> 16);
                        let base = if decoded.opcode == Opcode::DenseReadBinaryLocal {
                            DirectSlot::Local(base_index)
                        } else {
                            DirectSlot::Argument(base_index)
                        };
                        let key = if descriptor & 0x400 != 0 {
                            DirectSlot::Argument(key_index)
                        } else {
                            DirectSlot::Local(key_index)
                        };
                        let rhs_mode = (descriptor >> 11) & 3;
                        #[cfg(feature = "profiling")]
                        crate::engine::api::profiling::record_execution_dispatch(
                            runtime, executable, pc, true,
                        );
                        let result = cursor.with_slots(|slots| {
                            Ok(try_dense_read_binary(
                                slots, state, base, key, rhs_mode, rhs_index, operation,
                            ))
                        })?;
                        #[cfg(feature = "profiling")]
                        crate::engine::api::profiling::record_execution_outcome(
                            runtime,
                            executable,
                            pc,
                            "dense_read_binary",
                            if result.is_some() {
                                None
                            } else {
                                Some("guard")
                            },
                        );
                        if let Some(value) = result {
                            cursor.commit_push(value)?;
                            cursor.advance((decoded.next_pc + 4) as usize);
                            continue;
                        }
                        if decoded.opcode == Opcode::DenseReadBinaryLocal {
                            if let Some(action) =
                                read_local::<false>(&mut cursor, state, base_index)?
                            {
                                break 'dispatch Ok(action);
                            }
                        } else if let Some(action) = read_arg(&mut cursor, state, base_index)? {
                            break 'dispatch Ok(action);
                        }
                    }
                    Opcode::DenseReadLocal | Opcode::DenseReadArg => {
                        let base_index = published_u16(operand);
                        #[cfg(feature = "profiling")]
                        crate::engine::api::profiling::record_execution_dispatch(
                            runtime, executable, pc, true,
                        );
                        let key = decoded.operand(1);
                        let key_index = published_u16(key & 0xffff);
                        let base = if decoded.opcode == Opcode::DenseReadLocal {
                            DirectSlot::Local(base_index)
                        } else {
                            DirectSlot::Argument(base_index)
                        };
                        let key = if key & 0x1_0000 != 0 {
                            DirectSlot::Argument(key_index)
                        } else {
                            DirectSlot::Local(key_index)
                        };
                        let read = cursor.with_slots(|slots| {
                            if !slots.has_operand_capacity(2) {
                                return Ok(None);
                            }
                            let Some(Number::Int(index)) =
                                slots.direct_value(key).and_then(JsValue::as_number_repr)
                            else {
                                return Ok(None);
                            };
                            let Ok(index) = u32::try_from(index) else {
                                return Ok(None);
                            };
                            Ok(slots
                                .direct_value(base)
                                .and_then(|base| state.peek_dense_number(base, index)))
                        })?;
                        #[cfg(feature = "profiling")]
                        crate::engine::api::profiling::record_execution_outcome(
                            runtime,
                            executable,
                            pc,
                            "dense_read",
                            if read.is_some() { None } else { Some("guard") },
                        );
                        if let Some(value) = read {
                            cursor.commit_push(number_value(value))?;
                            cursor.advance(decoded.operand(2) as usize);
                            continue;
                        }
                        if decoded.opcode == Opcode::DenseReadLocal {
                            if let Some(action) =
                                read_local::<false>(&mut cursor, state, base_index)?
                            {
                                break 'dispatch Ok(action);
                            }
                        } else if let Some(action) = read_arg(&mut cursor, state, base_index)? {
                            break 'dispatch Ok(action);
                        }
                    }
                    Opcode::BorrowedFieldLocal | Opcode::BorrowedFieldArg => {
                        let base_index = published_u16(operand);
                        #[cfg(feature = "profiling")]
                        crate::engine::api::profiling::record_execution_dispatch(
                            runtime, executable, pc, true,
                        );
                        let base = if decoded.opcode == Opcode::BorrowedFieldLocal {
                            DirectSlot::Local(base_index)
                        } else {
                            DirectSlot::Argument(base_index)
                        };
                        let mut miss = crate::engine::object::NamedSelectionMiss::ContinueLookup;
                        // Keep the shared decoder out of this closure as well.
                        let field_index = decoded.operand(1);
                        let selection = cursor.with_slots(|slots| {
                            if !slots.has_operand_capacity(1) {
                                return Ok(None);
                            }
                            let Some(base @ JsValue::Object(_)) = slots.direct_value(base) else {
                                return Ok(None);
                            };
                            Ok(state.select_linked_data_into(
                                runtime.domain_id(),
                                base,
                                executable,
                                next,
                                field_index,
                                true,
                                &mut None,
                                &mut miss,
                            ))
                        })?;
                        let selection = selection.or_else(|| {
                            matches!(
                                miss,
                                crate::engine::object::NamedSelectionMiss::CompleteAbsent
                            )
                            .then_some(JsValue::Undefined)
                        });
                        #[cfg(feature = "profiling")]
                        crate::engine::api::profiling::record_execution_outcome(
                            runtime,
                            executable,
                            pc,
                            "borrowed_field",
                            if selection.is_some() {
                                None
                            } else {
                                Some("guard")
                            },
                        );
                        if let Some(value) = selection {
                            cursor.commit_owned(state, value)?;
                            #[cfg(feature = "profiling")]
                            crate::engine::api::profiling::record_owned_execution_event(
                                "local_completion.borrowed_named_read",
                            );
                            cursor.advance(decoded.operand(2) as usize);
                            continue;
                        }
                        match miss {
                            crate::engine::object::NamedSelectionMiss::Accessor(getter) => {
                                let selected = cursor.with_slots(|slots| {
                                    let receiver = slots.direct_value(base).ok_or_else(|| {
                                        Error::internal("selected borrowed receiver disappeared")
                                    })?;
                                    super::property_driver::OwnedGetterSelection::prepare(
                                        state,
                                        &runtime.0.poisoned,
                                        receiver,
                                        getter,
                                    )
                                })?;
                                let load = if decoded.opcode == Opcode::BorrowedFieldLocal {
                                    read_local::<false>(&mut cursor, state, base_index)
                                } else {
                                    read_arg(&mut cursor, state, base_index)
                                };
                                match load {
                                    Ok(None) => {
                                        debug_assert!(selected_named_read.is_none());
                                        *selected_named_read = Some(
                                            super::property_driver::SelectedNamedRead::Getter(
                                                selected,
                                            ),
                                        );
                                        cursor.advance(next);
                                        cursor.begin();
                                        break 'dispatch Ok(VmAction::GetField {
                                            index: decoded.operand(1),
                                            keep_receiver: false,
                                            fallthrough: FallthroughPc(decoded.operand(2)),
                                        });
                                    }
                                    Ok(Some(action)) => {
                                        selected.release(state, &runtime.0.poisoned)?;
                                        break 'dispatch Ok(action);
                                    }
                                    Err(error) => {
                                        let _ = selected.release(state, &runtime.0.poisoned);
                                        return Err(error);
                                    }
                                }
                            }
                            crate::engine::object::NamedSelectionMiss::ContinueLookup => {}
                            crate::engine::object::NamedSelectionMiss::CompleteAbsent => {
                                unreachable!("absence completed above")
                            }
                        }
                        if decoded.opcode == Opcode::BorrowedFieldLocal {
                            if let Some(action) =
                                read_local::<false>(&mut cursor, state, base_index)?
                            {
                                break 'dispatch Ok(action);
                            }
                        } else if let Some(action) = read_arg(&mut cursor, state, base_index)? {
                            break 'dispatch Ok(action);
                        }
                    }
                    Opcode::CompareBranchStack => {
                        let descriptor = operand;
                        let comparison = Opcode::from_raw((descriptor & 0x3ff) as u16)
                            .ok_or_else(|| Error::internal("invalid published comparison"))?;
                        #[cfg(feature = "profiling")]
                        crate::engine::api::profiling::record_execution_dispatch(
                            runtime, executable, pc, true,
                        );
                        let decision = cursor.with_slots(|slots| {
                            slots.number_pair_branch(|left, right| {
                                compare_direct_numbers(comparison as u16, left, right)
                            })
                        })?;
                        #[cfg(feature = "profiling")]
                        crate::engine::api::profiling::record_execution_outcome(
                            runtime,
                            executable,
                            pc,
                            "compare_branch_stack",
                            if decision.is_some() {
                                None
                            } else {
                                Some("guard")
                            },
                        );
                        if let Some(decision) = decision {
                            cursor.advance(if decision == (descriptor & 0x400 != 0) {
                                decoded.operand(1) as usize
                            } else {
                                next + 2
                            });
                            continue;
                        }
                        if matches!(comparison, Opcode::Eq | Opcode::Neq) {
                            if let Some(equal) = cursor.with_slots(|slots| {
                                slots.nullish_equality_in_state(state, &runtime.0.poisoned)
                            })? {
                                let decision = equal != (comparison == Opcode::Neq);
                                cursor.advance(if decision == (descriptor & 0x400 != 0) {
                                    decoded.operand(1) as usize
                                } else {
                                    next + 2
                                });
                                continue;
                            }
                        }
                        let completed = cursor.with_slots(|slots| {
                            slots.binary_number(|left, right| {
                                binary_number_result(comparison, left, right)
                            })
                        })?;
                        if !completed {
                            if matches!(comparison, Opcode::StrictEq | Opcode::StrictNeq) {
                                let decision = cursor.strict_comparison(state, active_frame)?
                                    != (comparison == Opcode::StrictNeq);
                                #[cfg(feature = "profiling")]
                                crate::engine::api::profiling::record_owned_execution_event(
                                    "strict_comparison.local_branch",
                                );
                                cursor.advance(if decision == (descriptor & 0x400 != 0) {
                                    decoded.operand(1) as usize
                                } else {
                                    next + 2
                                });
                                continue;
                            }
                            break 'dispatch Ok(VmAction::Numeric {
                                kind: super::numeric::operation::NumericKind::for_opcode(
                                    comparison,
                                )
                                .ok_or_else(|| Error::internal("comparison has no operation"))?,
                                fallthrough: FallthroughPc::from_decoded(decoded),
                            });
                        }
                    }
                    Opcode::CompareBranchLocalLt | Opcode::CompareBranchArgLt => {
                        let left_index = published_u16(operand);
                        let descriptor = decoded.operand(1);
                        let right_index = published_u16(descriptor & 0xffff);
                        let left = if decoded.opcode == Opcode::CompareBranchLocalLt {
                            DirectSlot::Local(left_index)
                        } else {
                            DirectSlot::Argument(left_index)
                        };
                        let right = if descriptor & 0x1_0000 != 0 {
                            DirectSlot::Argument(right_index)
                        } else {
                            DirectSlot::Local(right_index)
                        };
                        #[cfg(feature = "profiling")]
                        crate::engine::api::profiling::record_execution_dispatch(
                            runtime, executable, pc, true,
                        );
                        let decision = cursor.with_slots(|slots| {
                            if !slots.has_operand_capacity(2) {
                                return Ok(None);
                            }
                            let left = slots.direct_value(left).and_then(JsValue::as_number_repr);
                            let right = slots.direct_value(right).and_then(JsValue::as_number_repr);
                            Ok(left
                                .zip(right)
                                .map(|(left, right)| left.float() < right.float()))
                        })?;
                        #[cfg(feature = "profiling")]
                        crate::engine::api::profiling::record_execution_outcome(
                            runtime,
                            executable,
                            pc,
                            "compare_branch_lt",
                            if decision.is_some() {
                                None
                            } else {
                                Some("guard")
                            },
                        );
                        if let Some(decision) = decision {
                            let target = if decision == (descriptor & 0x800_0000 != 0) {
                                decoded.operand(2) as usize
                            } else {
                                next + 4
                            };
                            cursor.advance(target);
                            continue;
                        }
                        if decoded.opcode == Opcode::CompareBranchLocalLt {
                            if let Some(action) =
                                read_local::<false>(&mut cursor, state, left_index)?
                            {
                                break 'dispatch Ok(action);
                            }
                        } else if let Some(action) = read_arg(&mut cursor, state, left_index)? {
                            break 'dispatch Ok(action);
                        }
                    }
                    Opcode::CompareBranchLocal | Opcode::CompareBranchArg => {
                        let left_index = published_u16(operand);
                        let descriptor = decoded.operand(1);
                        let right_index = published_u16(descriptor & 0xffff);
                        let left = if decoded.opcode == Opcode::CompareBranchLocal {
                            DirectSlot::Local(left_index)
                        } else {
                            DirectSlot::Argument(left_index)
                        };
                        let right = if descriptor & 0x1_0000 != 0 {
                            DirectSlot::Argument(right_index)
                        } else {
                            DirectSlot::Local(right_index)
                        };
                        #[cfg(feature = "profiling")]
                        crate::engine::api::profiling::record_execution_dispatch(
                            runtime, executable, pc, true,
                        );
                        let decision = cursor.with_slots(|slots| {
                            if !slots.has_operand_capacity(2) {
                                return Ok(None);
                            }
                            let left = slots.direct_value(left).and_then(JsValue::as_number_repr);
                            let right = slots.direct_value(right).and_then(JsValue::as_number_repr);
                            Ok(left.zip(right).map(|(left, right)| {
                                compare_direct_numbers(
                                    ((descriptor >> 17) & 0x3ff) as u16,
                                    left,
                                    right,
                                )
                            }))
                        })?;
                        #[cfg(feature = "profiling")]
                        crate::engine::api::profiling::record_execution_outcome(
                            runtime,
                            executable,
                            pc,
                            "compare_branch",
                            if decision.is_some() {
                                None
                            } else {
                                Some("guard")
                            },
                        );
                        if let Some(decision) = decision {
                            let target = if decision == (descriptor & 0x800_0000 != 0) {
                                decoded.operand(2) as usize
                            } else {
                                next + 4
                            };
                            cursor.advance(target);
                            continue;
                        }
                        if decoded.opcode == Opcode::CompareBranchLocal {
                            if let Some(action) =
                                read_local::<false>(&mut cursor, state, left_index)?
                            {
                                break 'dispatch Ok(action);
                            }
                        } else if let Some(action) = read_arg(&mut cursor, state, left_index)? {
                            break 'dispatch Ok(action);
                        }
                    }
                    Opcode::GetVarRef | Opcode::GetVarRefCheck => {
                        let index = published_u16(operand);
                        let value = if cursor
                            .with_slots(|slots| Ok(slots.has_operand_capacity(1)))?
                        {
                            let root = owners
                                .function
                                .closures()
                                .get(runtime, usize::from(index))
                                .ok_or_else(|| {
                                    Error::internal("closure variable index is out of bounds")
                                })?;
                            super::bindings::try_read_captured_immediate_in_state(state, root.id())
                        } else {
                            None
                        };
                        #[cfg(feature = "profiling")]
                        crate::engine::api::profiling::record_execution_outcome(
                            runtime,
                            executable,
                            pc,
                            "captured_read",
                            if value.is_some() { None } else { Some("guard") },
                        );
                        if let Some(value) = value {
                            cursor.commit_push(value)?;
                        } else {
                            break 'dispatch Ok(VmAction::Binding {
                                source: BindingSource::Closure,
                                index,
                                write: false,
                                checked: decoded.opcode == Opcode::GetVarRefCheck,
                                keep: false,
                            });
                        }
                    }
                    Opcode::GetVar | Opcode::GetVarUndef => {
                        let index = published_u16(operand);
                        // Check space before taking the output edge. The cell keeps its
                        // own edge through commit; errors publish this cursor's fault
                        // PC and materialize in ready::run before they are propagated.
                        if cursor.with_slots(|slots| Ok(slots.has_operand_capacity(1)))?
                            && let Some(value) =
                                super::environment_driver::try_read_global_cell_in_state(
                                    runtime,
                                    state,
                                    executable,
                                    owners.function.closures(),
                                    index,
                                )?
                        {
                            cursor.commit_owned(state, value)?;
                            #[cfg(feature = "profiling")]
                            crate::engine::api::profiling::record_owned_execution_event(
                                "global_cell.local_complete",
                            );
                        } else {
                            break 'dispatch Ok(VmAction::Environment(
                                super::environment_driver::Operation::GlobalGet {
                                    index,
                                    strict: decoded.opcode == Opcode::GetVar,
                                },
                            ));
                        }
                    }
                    Opcode::GetArg => {
                        let index = published_u16(operand);
                        #[cfg(feature = "profiling")]
                        crate::engine::api::profiling::record_execution_dispatch(
                            runtime, executable, pc, false,
                        );
                        if let Some(action) = read_arg(&mut cursor, state, index)? {
                            break 'dispatch Ok(action);
                        }
                    }
                    Opcode::PutLocal
                    | Opcode::SetLocal
                    | Opcode::PutLocalCheck
                    | Opcode::SetLocalCheck => {
                        let index = published_u16(operand);
                        let keep =
                            matches!(decoded.opcode, Opcode::SetLocal | Opcode::SetLocalCheck);
                        let checked = matches!(
                            decoded.opcode,
                            Opcode::PutLocalCheck | Opcode::SetLocalCheck
                        );
                        let binding =
                            cursor.with_slots(|slots| Ok(binding_class(slots.local(index)?)))?;
                        if binding == BindingClass::Captured {
                            break 'dispatch Ok(VmAction::Binding {
                                source: BindingSource::Local,
                                index,
                                write: true,
                                checked,
                                keep,
                            });
                        }
                        if checked && binding == BindingClass::Uninitialized {
                            break 'dispatch Ok(VmAction::LexicalUninitialized(index));
                        }
                        if binding == BindingClass::DirectNumber
                            && cursor.with_slots(|slots| {
                                Ok(slots
                                    .store_proven_number_operand(DirectSlot::Local(index), keep))
                            })?
                        {
                            cursor.advance(next);
                            continue;
                        }
                        let progress = cursor.with_slots(|slots| {
                            if keep {
                                slots.set_direct_in_state(
                                    state,
                                    &runtime.0.poisoned,
                                    DirectSlot::Local(index),
                                )
                            } else {
                                slots.put_direct_in_state(
                                    state,
                                    &runtime.0.poisoned,
                                    DirectSlot::Local(index),
                                )
                            }
                        })?;
                        if progress == StoreProgress::Committed {
                            cursor.advance(next);
                            continue;
                        }
                        if !active_frame.is_materialized() {
                            break 'dispatch Ok(VmAction::Materialize);
                        }
                        cursor.publish_fault(state, active_frame)?;
                        let old = cursor.with_slots(|slots| {
                            let next = if keep {
                                copy_value_in_state(state, slots.peek(0)?)?
                            } else {
                                slots.pop()?
                            };
                            slots.replace_local(index, FrameBinding::Direct(next))
                        })?;
                        super::bindings::release_frame_binding_in_state(state, old).inspect_err(
                            |_| {
                                runtime.0.poisoned.set(true);
                            },
                        )?;
                    }
                    Opcode::PutArg | Opcode::SetArg => {
                        let index = published_u16(operand);
                        let keep = decoded.opcode == Opcode::SetArg;
                        let binding = cursor
                            .with_slots(|slots| Ok(binding_class(slots.parameter(index)?)))?;
                        if binding == BindingClass::Captured {
                            break 'dispatch Ok(VmAction::Binding {
                                source: BindingSource::Argument,
                                index,
                                write: true,
                                checked: false,
                                keep,
                            });
                        }
                        if binding == BindingClass::DirectNumber
                            && cursor.with_slots(|slots| {
                                Ok(slots
                                    .store_proven_number_operand(DirectSlot::Argument(index), keep))
                            })?
                        {
                            cursor.advance(next);
                            continue;
                        }
                        let progress = cursor.with_slots(|slots| {
                            if keep {
                                slots.set_direct_in_state(
                                    state,
                                    &runtime.0.poisoned,
                                    DirectSlot::Argument(index),
                                )
                            } else {
                                slots.put_direct_in_state(
                                    state,
                                    &runtime.0.poisoned,
                                    DirectSlot::Argument(index),
                                )
                            }
                        })?;
                        if progress == StoreProgress::Committed {
                            cursor.advance(next);
                            continue;
                        }
                        if !active_frame.is_materialized() {
                            break 'dispatch Ok(VmAction::Materialize);
                        }
                        cursor.publish_fault(state, active_frame)?;
                        let old = cursor.with_slots(|slots| {
                            let next = if keep {
                                copy_value_in_state(state, slots.peek(0)?)?
                            } else {
                                slots.pop()?
                            };
                            slots.replace_parameter(index, FrameBinding::Direct(next))
                        })?;
                        super::bindings::release_frame_binding_in_state(state, old).inspect_err(
                            |_| {
                                runtime.0.poisoned.set(true);
                            },
                        )?;
                    }
                    Opcode::InitializeLocal => {
                        let index = published_u16(operand);
                        let definition = executable.local_definitions[usize::from(index)];
                        use crate::engine::code::function::metadata::ClosureVariableKind;
                        if definition.kind == ClosureVariableKind::WithObject {
                            break 'dispatch Ok(VmAction::Environment(
                                super::environment_driver::Operation::InitializeWith(index),
                            ));
                        }
                        let class =
                            cursor.with_slots(|slots| Ok(binding_class(slots.local(index)?)))?;
                        if class == BindingClass::Captured
                            && definition.kind == ClosureVariableKind::Normal
                        {
                            break 'dispatch Ok(VmAction::Binding {
                                source: BindingSource::Local,
                                index,
                                write: true,
                                checked: false,
                                keep: false,
                            });
                        }
                        if !definition.is_lexical
                            || !matches!(
                                class,
                                BindingClass::Direct
                                    | BindingClass::DirectNumber
                                    | BindingClass::Uninitialized
                            )
                        {
                            return Err(Error::internal("local initializer has no valid binding"));
                        }
                        if cursor.with_slots(|slots| {
                            slots.initialize_direct_local_in_state(
                                state,
                                &runtime.0.poisoned,
                                index,
                            )
                        })? == StoreProgress::Committed
                        {
                            cursor.advance(next);
                            continue;
                        }
                        if !active_frame.is_materialized() {
                            break 'dispatch Ok(VmAction::Materialize);
                        }
                        cursor.publish_fault(state, active_frame)?;
                        let old = cursor.with_slots(|slots| {
                            let value = slots.pop()?;
                            slots.replace_local(index, FrameBinding::Direct(value))
                        })?;
                        super::bindings::release_frame_binding_in_state(state, old).inspect_err(
                            |_| {
                                runtime.0.poisoned.set(true);
                            },
                        )?;
                    }
                    Opcode::CloseLocal => {
                        let index = published_u16(operand);
                        let class =
                            cursor.with_slots(|slots| Ok(binding_class(slots.local(index)?)))?;
                        if class == BindingClass::Captured {
                            break 'dispatch Ok(VmAction::CloseCaptured(index));
                        }
                        if let Some(flag) =
                            owners.reusable_captured_locals.get_mut(usize::from(index))
                        {
                            *flag = false;
                        }
                    }
                    Opcode::SetLocalUninitialized => {
                        let index = published_u16(operand);
                        let class =
                            cursor.with_slots(|slots| Ok(binding_class(slots.local(index)?)))?;
                        if class == BindingClass::Captured {
                            break 'dispatch Ok(VmAction::ResetCaptured(index));
                        }
                        if class != BindingClass::Uninitialized {
                            if cursor.with_slots(|slots| {
                                slots.reset_direct_local_in_state(state, &runtime.0.poisoned, index)
                            })? == StoreProgress::Committed
                            {
                                if let Some(flag) =
                                    owners.reusable_captured_locals.get_mut(usize::from(index))
                                {
                                    *flag = false;
                                }
                                cursor.advance(next);
                                continue;
                            }
                            if !active_frame.is_materialized() {
                                break 'dispatch Ok(VmAction::Materialize);
                            }
                            cursor.publish_fault(state, active_frame)?;
                            let old = cursor.with_slots(|slots| {
                                slots.replace_local(index, FrameBinding::Uninitialized)
                            })?;
                            super::bindings::release_frame_binding_in_state(state, old)
                                .inspect_err(|_| {
                                    runtime.0.poisoned.set(true);
                                })?;
                        }
                        if let Some(flag) =
                            owners.reusable_captured_locals.get_mut(usize::from(index))
                        {
                            *flag = false;
                        }
                    }
                    Opcode::Dup | Opcode::Dup1 | Opcode::Dup3 => {
                        cursor.with_slots(|slots| match decoded.opcode {
                            Opcode::Dup => slots.insert_copy_in_state(state, 0, 0),
                            Opcode::Dup1 => slots.insert_copy_in_state(state, 1, 1),
                            _ => slots.duplicate_operands_in_state(state, 3),
                        })?;
                    }
                    Opcode::Insert2 | Opcode::Insert3 | Opcode::Insert4 => {
                        let count = match decoded.opcode {
                            Opcode::Insert2 => 2,
                            Opcode::Insert3 => 3,
                            _ => 4,
                        };
                        cursor.with_slots(|slots| {
                            slots.peek(count - 1)?;
                            slots.insert_copy_in_state(state, 0, count)
                        })?;
                    }
                    Opcode::Perm3 | Opcode::Perm4 | Opcode::Perm5 => {
                        let count = match decoded.opcode {
                            Opcode::Perm3 => 2,
                            Opcode::Perm4 => 3,
                            _ => 4,
                        };
                        cursor.with_slots(|slots| slots.rotate_operands(1, count, false))?;
                    }
                    Opcode::Rot4Left => {
                        cursor.with_slots(|slots| slots.rotate_operands(0, 4, true))?
                    }
                    Opcode::Swap => {
                        cursor.with_slots(|slots| slots.rotate_operands(0, 2, false))?
                    }
                    Opcode::Drop | Opcode::Nip => {
                        let removed_offset = usize::from(decoded.opcode == Opcode::Nip);
                        let immediate = cursor
                            .with_slots(|slots| Ok(is_immediate(slots.peek(removed_offset)?)))?;
                        if !immediate {
                            cursor.publish_fault(state, active_frame)?;
                        }
                        let removed = cursor.with_slots(|slots| {
                            if decoded.opcode == Opcode::Drop {
                                slots.pop()
                            } else {
                                slots.peek(1)?;
                                let kept = slots.pop()?;
                                let removed = slots.pop()?;
                                slots.push(kept)?;
                                Ok(removed)
                            }
                        })?;
                        if !immediate {
                            state
                                .release_owned_jsvalue(&runtime.0.poisoned, removed)
                                .map_err(runtime_error_to_vm_error)?;
                        }
                    }
                    Opcode::Add
                    | Opcode::Sub
                    | Opcode::Mul
                    | Opcode::Div
                    | Opcode::Mod
                    | Opcode::Pow
                    | Opcode::Shl
                    | Opcode::Sar
                    | Opcode::Shr
                    | Opcode::BitAnd
                    | Opcode::BitOr
                    | Opcode::BitXor
                    | Opcode::Eq
                    | Opcode::Neq
                    | Opcode::Lt
                    | Opcode::Lte
                    | Opcode::Gt
                    | Opcode::Gte
                    | Opcode::StrictEq
                    | Opcode::StrictNeq => {
                        let completed = cursor.with_slots(|slots| {
                            slots.binary_number(|left, right| {
                                binary_number_result(decoded.opcode, left, right)
                            })
                        })?;
                        if !completed {
                            if matches!(decoded.opcode, Opcode::Eq | Opcode::Neq) {
                                if let Some(equal) = cursor.with_slots(|slots| {
                                    slots.nullish_equality_in_state(state, &runtime.0.poisoned)
                                })? {
                                    cursor.commit_push(JsValue::Bool(
                                        equal != (decoded.opcode == Opcode::Neq),
                                    ))?;
                                    cursor.advance(next);
                                    continue;
                                }
                            }
                            if matches!(decoded.opcode, Opcode::StrictEq | Opcode::StrictNeq) {
                                let equal = cursor.strict_comparison(state, active_frame)?;
                                cursor.commit_push(JsValue::Bool(
                                    equal != (decoded.opcode == Opcode::StrictNeq),
                                ))?;
                                #[cfg(feature = "profiling")]
                                crate::engine::api::profiling::record_owned_execution_event(
                                    "strict_comparison.local_value",
                                );
                                cursor.advance(next);
                                continue;
                            }
                            break 'dispatch Ok(VmAction::Numeric {
                                kind: super::numeric::operation::NumericKind::for_opcode(
                                    decoded.opcode,
                                )
                                .ok_or_else(|| {
                                    Error::internal("numeric opcode has no operation")
                                })?,
                                fallthrough: FallthroughPc::from_decoded(decoded),
                            });
                        }
                    }
                    Opcode::Neg
                    | Opcode::Plus
                    | Opcode::BitNot
                    | Opcode::Inc
                    | Opcode::Dec
                    | Opcode::PostInc
                    | Opcode::PostDec => {
                        let completed = cursor.with_slots(|slots| {
                            let Some(old) = slots.peek(0)?.as_number_repr() else {
                                return Ok(false);
                            };
                            if matches!(decoded.opcode, Opcode::PostInc | Opcode::PostDec)
                                && !slots.has_operand_capacity(1)
                            {
                                return Ok(false);
                            }
                            let value = match decoded.opcode {
                                Opcode::Neg => old.negate(),
                                Opcode::Plus => old,
                                Opcode::BitNot => Number::Int(!old.int32()),
                                _ => old.update(matches!(
                                    decoded.opcode,
                                    Opcode::Inc | Opcode::PostInc
                                )),
                            };
                            if !matches!(decoded.opcode, Opcode::PostInc | Opcode::PostDec) {
                                let _ = slots.pop()?;
                            }
                            slots.push(number_value(value))?;
                            Ok(true)
                        })?;
                        if !completed {
                            if decoded.opcode == Opcode::Plus {
                                break 'dispatch Ok(VmAction::ConvertPlus);
                            }
                            break 'dispatch Ok(VmAction::Numeric {
                                kind: super::numeric::operation::NumericKind::for_opcode(
                                    decoded.opcode,
                                )
                                .ok_or_else(|| {
                                    Error::internal("numeric opcode has no operation")
                                })?,
                                fallthrough: FallthroughPc::from_decoded(decoded),
                            });
                        }
                    }
                    Opcode::Not => {
                        let immediate =
                            cursor.with_slots(|slots| Ok(is_immediate(slots.peek(0)?)))?;
                        if immediate {
                            let value = cursor.move_owned()?;
                            cursor.commit_push(JsValue::Bool(!value.to_boolean_primitive()))?;
                            cursor.advance(next);
                            continue;
                        }
                        let truthy = cursor.with_slots(|slots| {
                            state
                                .value_to_boolean_jsvalue(slots.peek(0)?)
                                .map_err(runtime_error_to_vm_error)
                        })?;
                        cursor.publish_fault(state, active_frame)?;
                        let old = cursor.move_owned()?;
                        cursor.commit_push(JsValue::Bool(!truthy))?;
                        state
                            .release_owned_jsvalue(&runtime.0.poisoned, old)
                            .map_err(runtime_error_to_vm_error)?;
                    }
                    Opcode::GetFieldCached | Opcode::GetField2Cached => {
                        let keep_receiver = decoded.opcode == Opcode::GetField2Cached;
                        let mut native = None;
                        let pending = PendingNamedRead {
                            index: operand,
                            keep_receiver,
                            fallthrough: FallthroughPc::from_decoded(decoded),
                        };
                        let step = cursor.with_slots(|slots| {
                            slots.property_ic_read_in_state(
                                state,
                                &runtime.0.poisoned,
                                runtime.domain_id(),
                                executable,
                                NamedReadOperation {
                                    site: pc,
                                    key_index: operand,
                                    keep_receiver,
                                },
                                &mut native,
                            )
                        })?;
                        // This weak selection belongs to the latest named
                        // read. Nested argument reads replace it; the native
                        // consumer also checks the actual callee identity.
                        *selected_native = native;
                        #[cfg(feature = "profiling")]
                        crate::engine::api::profiling::record_execution_outcome(
                            runtime,
                            executable,
                            pc,
                            "field_cache",
                            if matches!(step, PropertyReadProgress::Completed) {
                                None
                            } else {
                                Some("guard")
                            },
                        );
                        match step {
                            PropertyReadProgress::Completed => {
                                #[cfg(feature = "profiling")]
                                crate::engine::api::profiling::record_owned_execution_event(
                                    "local_completion.named_read",
                                );
                            }
                            PropertyReadProgress::Driver => {
                                break 'dispatch Ok(pending.action());
                            }
                            PropertyReadProgress::Selected(getter) => {
                                let read = cursor.with_slots(|slots| {
                                    Ok(super::property_driver::OwnedGetterSelection::prepare(
                                        state,
                                        &runtime.0.poisoned,
                                        slots.peek(0)?,
                                        getter,
                                    ))
                                })?;
                                debug_assert!(selected_named_read.is_none());
                                *selected_named_read = Some(match read {
                                    Ok(read) => {
                                        super::property_driver::SelectedNamedRead::Getter(read)
                                    }
                                    Err(error) => {
                                        super::property_driver::SelectedNamedRead::LookupError(
                                            error,
                                        )
                                    }
                                });
                                break 'dispatch Ok(pending.action());
                            }
                        }
                    }
                    Opcode::GetArrayElDense
                    | Opcode::GetArrayEl2Dense
                    | Opcode::GetArrayEl3Dense => {
                        let keep_receiver = decoded.opcode != Opcode::GetArrayElDense;
                        let keep_key = decoded.opcode == Opcode::GetArrayEl3Dense;
                        let hit = cursor.with_slots(|slots| {
                            if keep_receiver {
                                slots.array_kept_immediate_read_in_state(
                                    state,
                                    &runtime.0.poisoned,
                                    keep_key,
                                )
                            } else {
                                slots.array_immediate_read_in_state(state, &runtime.0.poisoned)
                            }
                        })?;
                        #[cfg(feature = "profiling")]
                        crate::engine::api::profiling::record_execution_outcome(
                            runtime,
                            executable,
                            pc,
                            "dense_array_read",
                            if hit { None } else { Some("guard") },
                        );
                        if !hit {
                            break 'dispatch Ok(VmAction::GetElement {
                                keep_receiver,
                                keep_key,
                                fallthrough: FallthroughPc::from_decoded(decoded),
                            });
                        }
                    }
                    Opcode::PutField => {
                        let Some(generation) = property_generation.checked_add(1) else {
                            break 'dispatch Ok(VmAction::SetProperty(Some(operand)));
                        };
                        if !cursor.with_slots(|slots| {
                            slots.try_scalar_field_write_in_state(
                                state,
                                &runtime.0.poisoned,
                                runtime.domain_id(),
                                executable,
                                operand,
                            )
                        })? {
                            break 'dispatch Ok(VmAction::SetProperty(Some(operand)));
                        }
                        *property_generation = generation;
                    }
                    Opcode::PutArrayEl => {
                        let Some(generation) = property_generation.checked_add(1) else {
                            break 'dispatch Ok(VmAction::SetProperty(None));
                        };
                        if !cursor.with_slots(|slots| {
                            slots.try_scalar_element_write_in_state(state, &runtime.0.poisoned)
                        })? {
                            break 'dispatch Ok(VmAction::SetProperty(None));
                        }
                        *property_generation = generation;
                    }
                    Opcode::Goto => next = operand as usize,
                    Opcode::IfTrue | Opcode::IfFalse => {
                        let immediate =
                            cursor.with_slots(|slots| Ok(is_immediate(slots.peek(0)?)))?;
                        if immediate {
                            let truthy = cursor.move_owned()?.to_boolean_primitive();
                            if truthy == (decoded.opcode == Opcode::IfTrue) {
                                next = operand as usize;
                            }
                        } else {
                            let truthy = cursor.with_slots(|slots| {
                                state
                                    .value_to_boolean_jsvalue(slots.peek(0)?)
                                    .map_err(runtime_error_to_vm_error)
                            })?;
                            cursor.publish_fault(state, active_frame)?;
                            let condition = cursor.move_owned()?;
                            state
                                .release_owned_jsvalue(&runtime.0.poisoned, condition)
                                .map_err(runtime_error_to_vm_error)?;
                            if truthy == (decoded.opcode == Opcode::IfTrue) {
                                next = operand as usize;
                            }
                        }
                    }
                    Opcode::Gosub => {
                        let return_pc = i32::try_from(next)
                            .map_err(|_| Error::internal("gosub return PC exceeds Int"))?;
                        cursor.commit_push(JsValue::Int(return_pc))?;
                        next = operand as usize;
                    }
                    Opcode::Ret => {
                        // A dynamic target is not a published static continuation.
                        // Validate it while its owner remains in the frame storage.
                        next = cursor.with_slots(|slots| {
                            let JsValue::Int(target) = slots.peek(0)? else {
                                return Err(Error::internal("invalid ret value"));
                            };
                            let target = usize::try_from(*target)
                                .map_err(|_| Error::internal("invalid ret value"))?;
                            if executable.exec.opcode_at_exec(target).is_none() {
                                return Err(Error::internal(
                                    "ret target is not an instruction boundary",
                                ));
                            }
                            Ok(target)
                        })?;
                        let _address = cursor.move_owned()?;
                    }
                    Opcode::DropGosub => {
                        cursor.with_slots(|slots| {
                            if !matches!(slots.peek(0)?, JsValue::Int(_)) {
                                return Err(Error::internal("invalid gosub cleanup value"));
                            }
                            Ok(())
                        })?;
                        let _address = cursor.move_owned()?;
                    }
                    Opcode::InitialYield
                    | Opcode::Yield
                    | Opcode::YieldStar
                    | Opcode::AsyncYieldStar
                    | Opcode::Await => {
                        let kind = match decoded.opcode {
                            Opcode::InitialYield => super::VmSuspendKind::Initial,
                            Opcode::Yield => super::VmSuspendKind::Yield,
                            Opcode::YieldStar => super::VmSuspendKind::YieldStar,
                            Opcode::AsyncYieldStar => super::VmSuspendKind::AsyncYieldStar,
                            _ => super::VmSuspendKind::Await,
                        };
                        if kind != super::VmSuspendKind::Initial {
                            cursor.with_slots(|slots| {
                                slots.peek(0)?;
                                Ok(())
                            })?;
                        }
                        cursor.advance(next);
                        break 'dispatch Ok(VmAction::Suspend(kind));
                    }
                    Opcode::Call
                    | Opcode::TailCall
                    | Opcode::CallMethod
                    | Opcode::TailCallMethod => {
                        break 'dispatch Ok(VmAction::Call {
                            arguments: published_u16(operand),
                            method: matches!(
                                decoded.opcode,
                                Opcode::CallMethod | Opcode::TailCallMethod
                            ),
                            tail: matches!(
                                decoded.opcode,
                                Opcode::TailCall | Opcode::TailCallMethod
                            ),
                            fallthrough: FallthroughPc::from_decoded(decoded),
                        });
                    }
                    Opcode::Return => {
                        *pending = Some(cursor.move_owned()?);
                        cursor.advance(next);
                        break 'dispatch Ok(VmAction::Complete);
                    }
                    Opcode::ReturnUndefined => {
                        *pending = Some(JsValue::Undefined);
                        cursor.advance(next);
                        break 'dispatch Ok(VmAction::Complete);
                    }
                    Opcode::Throw => break 'dispatch Ok(VmAction::Throw),
                    _ => {
                        // Materialize only the fallback operands here. Passing the whole
                        // decoder makes its aggregate spill on every dispatch iteration.
                        break 'dispatch deferred_action(
                            decoded.opcode,
                            operand,
                            decoded.operand_or_zero(1),
                            decoded.operand_or_zero(2),
                            FallthroughPc::from_decoded(decoded),
                            executable.metadata.strict,
                        )?
                        .ok_or_else(|| {
                            Error::internal("published opcode has no execution handler")
                        });
                    }
                }
                cursor.advance(next);
            }
        }?;
        match action {
            VmAction::Call {
                arguments,
                method,
                tail,
                fallthrough,
            } => {
                match segment.enter_ordinary(
                    runtime,
                    state,
                    arguments,
                    method,
                    tail,
                    fallthrough,
                )? {
                    super::driver::ordinary::Entry::Ordinary => {
                        #[cfg(feature = "profiling")]
                        crate::engine::api::profiling::record_owned_execution_event(
                            "core.internal_call",
                        );
                        continue;
                    }
                    super::driver::ordinary::Entry::General => return Ok(action),
                    super::driver::ordinary::Entry::Native(_)
                    | super::driver::ordinary::Entry::NativeReady => {
                        return Err(Error::internal(
                            "ordinary segment entered a native activation",
                        ));
                    }
                }
            }
            VmAction::Construct {
                arguments,
                fallthrough,
            } => {
                if !segment.enter_constructor(runtime, state, arguments, fallthrough)? {
                    return Ok(action);
                }
                // The allocation is now published in a complete child frame;
                // no temporary owner lies outside execution storage at GC.
                state
                    .collect_if_requested(&runtime.0.gc_pressure)
                    .map_err(runtime_error_to_vm_error)?;
                #[cfg(feature = "profiling")]
                crate::engine::api::profiling::record_owned_execution_event(
                    "core.internal_construct",
                );
                continue;
            }
            VmAction::Complete => match segment.finish_ordinary(runtime, state)? {
                super::driver::ordinary::ReturnProgress::Returned => {
                    #[cfg(feature = "profiling")]
                    crate::engine::api::profiling::record_owned_execution_event(
                        "core.internal_return",
                    );
                    continue;
                }
                super::driver::ordinary::ReturnProgress::Declined => return Ok(action),
                super::driver::ordinary::ReturnProgress::Property(_) => {
                    return Err(Error::internal(
                        "ordinary segment entered a property continuation",
                    ));
                }
            },
            _ => return Ok(action),
        }
    }
}

#[inline(always)]
fn region_direct_slot(source: DirectSource) -> DirectSlot {
    match source {
        DirectSource::Local(index) | DirectSource::CheckedLocal(index) => DirectSlot::Local(index),
        DirectSource::Argument(index) => DirectSlot::Argument(index),
    }
}

#[inline(always)]
fn region_number(slots: &FrameSlots<'_>, source: NumberSource) -> Option<Number> {
    match source {
        NumberSource::Direct(DirectSource::Local(index)) => slots.immediate_local(index),
        NumberSource::Direct(DirectSource::CheckedLocal(index)) => slots.immediate_local(index),
        NumberSource::Direct(DirectSource::Argument(index)) => slots.immediate_parameter(index),
        NumberSource::Immediate(value) => Some(Number::Int(value)),
        NumberSource::Constant { value, .. } => Some(value),
    }
}

#[inline(always)]
pub(in crate::engine::vm) fn numeric_local_array_region(
    slots: &mut FrameSlots<'_>,
    state: &mut RuntimeState,
    region: &PublishedNumericRegion,
    accumulate: bool,
) -> Result<(), Miss> {
    if !slots.has_operand_capacity(region.peak as usize) {
        return Err(Miss::OperandCapacity);
    }
    let Some(index_number) = region_number(slots, region.index) else {
        return Err(Miss::IndexNotNumber);
    };
    let Some(index) = array_index(index_number) else {
        return Err(Miss::IndexNotNumericInteger);
    };
    let Some(scale) = region_number(slots, region.value) else {
        return Err(Miss::ScaleNotNumber);
    };
    let admitted = if accumulate {
        slots.admit_numeric_local_with_source(region.destination, region_direct_slot(region.array))
    } else {
        slots.admit_scalar_local_with_source(region.destination, region_direct_slot(region.array))
    };
    let Some((destination, base)) = admitted else {
        return Err(Miss::DestinationOrReceiverUnavailable);
    };
    let element = state.peek_dense_number_result(base, index)?;
    let product = element.mul(scale);
    let result = if accumulate {
        destination
            .old_number
            .expect("numeric admission carries old Number")
            .add(product)
    } else {
        product
    };
    destination.commit(result);
    Ok(())
}

#[inline(always)]
fn array_index(value: Number) -> Option<u32> {
    let value = value.float();
    (value >= 0.0 && value < f64::from(u32::MAX) && value.trunc() == value).then_some(value as u32)
}

#[inline(always)]
fn read_local<const CHECKED: bool>(
    cursor: &mut FrameCursor<'_>,
    state: &mut RuntimeState,
    index: u16,
) -> Result<Option<VmAction>, Error> {
    if cursor.with_slots(|slots| slots.push_direct_immediate(DirectSlot::Local(index)))? {
        return Ok(None);
    }
    let (copied, uninitialized) = cursor.with_slots(|slots| match slots.local(index)? {
        FrameBinding::Direct(value) => {
            copy_value_in_state(state, value).map(|value| (Some(value), false))
        }
        FrameBinding::Captured(id) if slots.has_operand_capacity(1) => Ok((
            super::bindings::try_read_captured_immediate_in_state(state, *id),
            false,
        )),
        FrameBinding::Uninitialized => Ok((None, true)),
        _ => Ok((None, false)),
    })?;
    if let Some(value) = copied {
        cursor.commit_owned(state, value)?;
        return Ok(None);
    }
    if CHECKED && uninitialized {
        return Ok(Some(VmAction::LexicalUninitialized(index)));
    }
    Ok(Some(VmAction::Binding {
        source: BindingSource::Local,
        index,
        write: false,
        checked: CHECKED,
        keep: false,
    }))
}

#[inline(always)]
fn read_arg(
    cursor: &mut FrameCursor<'_>,
    state: &mut RuntimeState,
    index: u16,
) -> Result<Option<VmAction>, Error> {
    if cursor.with_slots(|slots| slots.push_direct_immediate(DirectSlot::Argument(index)))? {
        return Ok(None);
    }
    let copied = cursor.with_slots(|slots| match slots.parameter(index)? {
        FrameBinding::Direct(value) => copy_value_in_state(state, value).map(Some),
        FrameBinding::Captured(id) if slots.has_operand_capacity(1) => Ok(
            super::bindings::try_read_captured_immediate_in_state(state, *id),
        ),
        _ => Ok(None),
    })?;
    if let Some(value) = copied {
        cursor.commit_owned(state, value)?;
        return Ok(None);
    }
    Ok(Some(VmAction::Binding {
        source: BindingSource::Argument,
        index,
        write: false,
        checked: false,
        keep: false,
    }))
}

/// Operations requiring observable semantics leave the frame window untouched.
/// The action carries operands decoded from the one published word stream;
/// there is no execution-time `Instruction` reconstruction or alternate loop.
fn deferred_action(
    opcode: Opcode,
    a: u32,
    b: u32,
    c: u32,
    fallthrough: FallthroughPc,
    strict: bool,
) -> Result<Option<VmAction>, Error> {
    use super::environment_driver::{Operation as E, WriteTarget};
    use super::iterator_driver::{Operation as I, suspension::Operation as S};
    use super::pure_operations::PureOperation as P;
    let action = match opcode {
        Opcode::PushAtomValueIndex => VmAction::Pure(P::AtomValue(a)),
        Opcode::RegExp => VmAction::Pure(P::RegExp(a)),
        Opcode::ThrowDeleteSuper => VmAction::Pure(P::DeleteSuper),
        Opcode::InitializeModuleImportCollision => {
            VmAction::Pure(P::InitializeModuleImportCollision(checked_u16(a)?))
        }
        Opcode::InitializeVarRef | Opcode::InitializeDerivedVarRef => {
            VmAction::Pure(P::InitializeClosure {
                index: checked_u16(a)?,
                derived: opcode == Opcode::InitializeDerivedVarRef,
            })
        }
        Opcode::SetProto => VmAction::Pure(P::SetPrototype),
        Opcode::IteratorCheckObject => VmAction::Pure(P::IteratorCheckObject),
        Opcode::ThrowIteratorMissingThrow => VmAction::Pure(P::IteratorMissingThrow),
        Opcode::TypeOf => VmAction::Pure(P::TypeOf),
        Opcode::IsUndefinedOrNull => VmAction::Pure(P::IsUndefinedOrNull),
        Opcode::IsUndefined => VmAction::Pure(P::IsUndefined),
        Opcode::IsNull => VmAction::Pure(P::IsNull),
        Opcode::TypeOfIsUndefined => VmAction::Pure(P::TypeOfIsUndefined),
        Opcode::TypeOfIsFunction => VmAction::Pure(P::TypeOfIsFunction),
        Opcode::PutVarRef | Opcode::SetVarRef | Opcode::PutVarRefCheck => VmAction::Binding {
            source: BindingSource::Closure,
            index: checked_u16(a)?,
            write: true,
            checked: opcode == Opcode::PutVarRefCheck,
            keep: opcode == Opcode::SetVarRef,
        },
        Opcode::Catch => VmAction::Catch(a),
        Opcode::DropCatch => VmAction::DropCatch,
        Opcode::NipCatch => VmAction::NipCatch,
        Opcode::ToObject => VmAction::Environment(E::ToObject),
        Opcode::ToPropKey => VmAction::ConvertPropertyKey,

        Opcode::GlobalReference => VmAction::Environment(E::GlobalReference(checked_u16(a)?)),
        Opcode::DeleteVar => VmAction::Environment(E::GlobalDelete(checked_u16(a)?)),
        Opcode::GetVar | Opcode::GetVarUndef => VmAction::Environment(E::GlobalGet {
            index: checked_u16(a)?,
            strict: opcode == Opcode::GetVar,
        }),
        Opcode::PutVar | Opcode::PutVarInit => VmAction::Environment(E::Put {
            source: WriteTarget::Global {
                index: checked_u16(a)?,
                initialize: opcode == Opcode::PutVarInit,
            },
            name: 0,
            strict,
            check_presence: true,
        }),
        Opcode::GetRefValue | Opcode::GetRefValueUndef => VmAction::Environment(E::ReadReference {
            name: a,
            strict: opcode == Opcode::GetRefValue && strict,
        }),
        Opcode::PutRefValue => VmAction::Environment(E::Put {
            source: WriteTarget::Reference,
            name: a,
            strict,
            check_presence: true,
        }),
        Opcode::DynamicEnvironmentObject => {
            VmAction::Environment(E::Object(decode_dynamic_source(a)?))
        }
        Opcode::HasDynamicBinding | Opcode::HasEvalVariable => {
            let source = if opcode == Opcode::HasEvalVariable {
                DynamicEnvironmentSource::Eval(decode_eval_source(a)?)
            } else {
                decode_dynamic_source(a)?
            };
            VmAction::Environment(E::Has { source, name: b })
        }
        Opcode::GetDynamicBinding | Opcode::GetEvalVariable => {
            let source = if opcode == Opcode::GetEvalVariable {
                DynamicEnvironmentSource::Eval(decode_eval_source(a)?)
            } else {
                decode_dynamic_source(a)?
            };
            VmAction::Environment(E::Get {
                source,
                name: b,
                strict: opcode == Opcode::GetDynamicBinding && strict,
            })
        }
        Opcode::PutDynamicBinding | Opcode::PutEvalVariable => {
            let source = if opcode == Opcode::PutEvalVariable {
                DynamicEnvironmentSource::Eval(decode_eval_source(a)?)
            } else {
                decode_dynamic_source(a)?
            };
            VmAction::Environment(E::Put {
                source: WriteTarget::Dynamic(source),
                name: b,
                strict: opcode == Opcode::PutDynamicBinding && strict,
                check_presence: opcode == Opcode::PutDynamicBinding,
            })
        }
        Opcode::DeleteDynamicBinding | Opcode::DeleteEvalVariable => {
            let source = if opcode == Opcode::DeleteEvalVariable {
                DynamicEnvironmentSource::Eval(decode_eval_source(a)?)
            } else {
                decode_dynamic_source(a)?
            };
            VmAction::Environment(E::Delete { source, name: b })
        }
        Opcode::DefineEvalVariable => VmAction::Environment(E::Define {
            source: decode_eval_source(a)?,
            name: b,
        }),
        Opcode::VariableEnvironment => VmAction::Environment(E::CreateVariable),
        Opcode::Object => VmAction::Environment(E::CreateObject),
        Opcode::ArrayFrom => VmAction::Environment(E::CreateArray(checked_u16(a)?)),
        Opcode::DefineArrayEl => VmAction::Environment(E::DefineArrayElement),
        Opcode::Append => VmAction::Environment(E::Append),

        Opcode::ForInStart => VmAction::ForIn(false),
        Opcode::ForInNext => VmAction::ForIn(true),
        Opcode::IteratorStart => VmAction::Environment(E::Iterator(I::Suspend(S::Start {
            asynchronous: false,
            delegating: true,
        }))),
        Opcode::AsyncIteratorStart => VmAction::Environment(E::Iterator(I::Suspend(S::Start {
            asynchronous: true,
            delegating: true,
        }))),
        Opcode::ForAwaitOfStart => VmAction::Environment(E::Iterator(I::Suspend(S::Start {
            asynchronous: true,
            delegating: false,
        }))),
        Opcode::ForAwaitOfNext => VmAction::Environment(E::Iterator(I::Suspend(S::AwaitNext))),
        Opcode::IteratorNext => VmAction::Environment(E::Iterator(I::Suspend(S::Next))),
        Opcode::IteratorCall => VmAction::Environment(E::Iterator(I::Suspend(S::Call(
            decode_iterator_call_kind(a)?,
        )))),
        Opcode::IteratorGetValueDone => VmAction::Environment(E::Iterator(I::Suspend(S::Parse))),
        Opcode::ForOfStart => VmAction::Environment(E::Iterator(I::Start)),
        Opcode::ForOfNext => VmAction::Environment(E::Iterator(I::Next(a as usize))),
        Opcode::IteratorClose => VmAction::Environment(E::Iterator(I::Close)),
        Opcode::IteratorClosePreserve => VmAction::Environment(E::Iterator(I::ClosePreserve)),
        Opcode::IteratorDropPreserve => VmAction::Environment(E::Iterator(I::DropPreserve)),
        Opcode::IteratorDetachPreserve => VmAction::Environment(E::Iterator(I::DetachPreserve)),

        Opcode::GetField | Opcode::GetField2 => VmAction::GetField {
            index: a,
            keep_receiver: opcode == Opcode::GetField2,
            fallthrough,
        },
        Opcode::GetArrayEl | Opcode::GetArrayEl2 | Opcode::GetArrayEl3 => VmAction::GetElement {
            keep_receiver: opcode != Opcode::GetArrayEl,
            keep_key: opcode == Opcode::GetArrayEl3,
            fallthrough,
        },
        Opcode::PutField => VmAction::SetProperty(Some(a)),
        Opcode::PutArrayEl => VmAction::SetProperty(None),
        Opcode::GetSuper => VmAction::GetSuper,
        Opcode::PushHomeObject => VmAction::HomeObject,
        Opcode::GetSuperValue => VmAction::SuperProperty(super::super_property_driver::Kind::Read),
        Opcode::GetSuperValueForCall => {
            VmAction::SuperProperty(super::super_property_driver::Kind::Call)
        }
        Opcode::PutSuperValue => VmAction::SuperProperty(super::super_property_driver::Kind::Write),
        Opcode::InstanceOf => VmAction::Predicate(super::predicate_driver::Kind::Instance),
        Opcode::In => VmAction::Predicate(super::predicate_driver::Kind::Has),
        Opcode::Delete => VmAction::Predicate(super::predicate_driver::Kind::Delete),
        Opcode::DefineField => VmAction::DefineProperty {
            key: Some(a),
            method: None,
        },
        Opcode::DefineFieldComputed => VmAction::DefineProperty {
            key: None,
            method: None,
        },
        Opcode::DefineMethod => VmAction::DefineProperty {
            key: Some(a),
            method: Some((decode_method_kind(b)?, b_is_true(c)?)),
        },
        Opcode::DefineMethodComputed => VmAction::DefineProperty {
            key: None,
            method: Some((decode_method_kind(a)?, b_is_true(b)?)),
        },
        Opcode::DefineClass => VmAction::DefineClass {
            name: a,
            has_heritage: b_is_true(b)?,
        },
        Opcode::InstallClassInstanceInitializer => {
            VmAction::ClassInitializer(super::construct_driver::InitializerKind::Install)
        }
        Opcode::CallClassInstanceInitializer => {
            VmAction::ClassInitializer(super::construct_driver::InitializerKind::Instance)
        }
        Opcode::RunClassStaticInitializer => {
            VmAction::ClassInitializer(super::construct_driver::InitializerKind::Static)
        }
        Opcode::CallClassStaticBlock => {
            VmAction::ClassInitializer(super::construct_driver::InitializerKind::Block)
        }
        Opcode::CopyDataProperties => VmAction::CopyData {
            target: 1,
            source: 0,
            excluded: None,
        },
        Opcode::CopyDataPropertiesExcluded => VmAction::CopyData {
            target: checked_u8(a)?,
            source: checked_u8(b)?,
            excluded: Some(checked_u8(c)?),
        },

        Opcode::InitializeDerivedLocal => VmAction::InitializeDerived(checked_u16(a)?),
        Opcode::ReturnDerived => VmAction::ReturnDerived(checked_u16(a)?),
        Opcode::InitDerivedConstructor => VmAction::InitDerivedConstructor,
        Opcode::Construct | Opcode::ConstructSuper => VmAction::Construct {
            arguments: checked_u16(a)?,
            fallthrough,
        },
        Opcode::Apply => VmAction::Apply(decode_apply_kind(a)?),
        Opcode::ApplySuper => VmAction::Apply(ApplyKind::Construct),
        Opcode::ApplyEval => VmAction::ApplyEval(checked_u16(a)?),
        Opcode::Eval => VmAction::Eval {
            arguments: checked_u16(a)?,
            environment: checked_u16(b)?,
        },
        Opcode::Import => VmAction::Import,
        Opcode::Arguments => VmAction::Arguments(decode_arguments_kind(a)?),
        Opcode::Rest => VmAction::Rest(checked_u16(a)?),
        Opcode::FClosure => VmAction::InstantiateClosure(a),
        Opcode::SetName => VmAction::SetName(Some(a)),
        Opcode::SetNameComputed => VmAction::SetName(None),
        Opcode::ThrowReadOnly | Opcode::ThrowRedeclaration => VmAction::BindingError {
            index: a,
            redeclaration: opcode == Opcode::ThrowRedeclaration,
        },
        Opcode::InitializePrivateName
        | Opcode::InitializePrivateMethod
        | Opcode::InitializePrivateAccessor => VmAction::PrivateInitialize {
            index: checked_u16(a)?,
            kind: match opcode {
                Opcode::InitializePrivateName => super::private_bindings::Initialization::Name,
                Opcode::InitializePrivateMethod => super::private_bindings::Initialization::Method,
                _ => super::private_bindings::Initialization::Accessor,
            },
        },
        Opcode::GetPrivateField
        | Opcode::GetPrivateField2
        | Opcode::PutPrivateField
        | Opcode::DefinePrivateField
        | Opcode::PrivateIn => VmAction::PrivateAccess {
            source: decode_private_source(a)?,
            access: match opcode {
                Opcode::GetPrivateField => super::private_access::Access::Get,
                Opcode::GetPrivateField2 => super::private_access::Access::GetKeep,
                Opcode::PutPrivateField => super::private_access::Access::Put,
                Opcode::DefinePrivateField => super::private_access::Access::Define,
                _ => super::private_access::Access::In,
            },
        },
        _ => return Ok(None),
    };
    Ok(Some(action))
}

fn checked_u8(value: u32) -> Result<u8, Error> {
    u8::try_from(value).map_err(|_| Error::internal("published u8 operand is invalid"))
}

fn b_is_true(value: u32) -> Result<bool, Error> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(Error::internal("published Boolean operand is invalid")),
    }
}

fn decode_eval_source(value: u32) -> Result<EvalVariableSource, Error> {
    let index = checked_u16(value & 0xffff)?;
    match value >> 16 {
        0 => Ok(EvalVariableSource::Local(index)),
        1 => Ok(EvalVariableSource::Closure(index)),
        _ => Err(Error::internal("published eval source is invalid")),
    }
}

fn decode_dynamic_source(value: u32) -> Result<DynamicEnvironmentSource, Error> {
    let index = checked_u16(value & 0xffff)?;
    match value >> 16 {
        0 => Ok(DynamicEnvironmentSource::Eval(EvalVariableSource::Local(
            index,
        ))),
        1 => Ok(DynamicEnvironmentSource::Eval(EvalVariableSource::Closure(
            index,
        ))),
        2 => Ok(DynamicEnvironmentSource::With(WithObjectSource::Local(
            index,
        ))),
        3 => Ok(DynamicEnvironmentSource::With(WithObjectSource::Closure(
            index,
        ))),
        _ => Err(Error::internal("published dynamic source is invalid")),
    }
}

fn decode_private_source(value: u32) -> Result<PrivateNameSource, Error> {
    let index = checked_u16(value & 0xffff)?;
    match value >> 16 {
        0 => Ok(PrivateNameSource::Local(index)),
        1 => Ok(PrivateNameSource::Closure(index)),
        _ => Err(Error::internal("published private source is invalid")),
    }
}

fn decode_method_kind(value: u32) -> Result<DefineMethodKind, Error> {
    match value {
        0 => Ok(DefineMethodKind::Method),
        1 => Ok(DefineMethodKind::Getter),
        2 => Ok(DefineMethodKind::Setter),
        _ => Err(Error::internal("published method kind is invalid")),
    }
}

fn decode_iterator_call_kind(value: u32) -> Result<IteratorCallKind, Error> {
    match value {
        0 => Ok(IteratorCallKind::ReturnWithValue),
        1 => Ok(IteratorCallKind::ThrowWithValue),
        2 => Ok(IteratorCallKind::ReturnWithoutValue),
        _ => Err(Error::internal("published iterator call kind is invalid")),
    }
}

fn decode_apply_kind(value: u32) -> Result<ApplyKind, Error> {
    match value {
        0 => Ok(ApplyKind::Call),
        1 => Ok(ApplyKind::Construct),
        _ => Err(Error::internal("published apply kind is invalid")),
    }
}

fn decode_arguments_kind(value: u32) -> Result<ArgumentsKind, Error> {
    match value {
        0 => Ok(ArgumentsKind::Mapped),
        1 => Ok(ArgumentsKind::Unmapped),
        _ => Err(Error::internal("published arguments kind is invalid")),
    }
}

#[inline(always)]
fn published_u16(value: u32) -> u16 {
    debug_assert!(value <= u32::from(u16::MAX));
    value as u16
}

fn checked_u16(value: u32) -> Result<u16, Error> {
    u16::try_from(value).map_err(|_| Error::internal("published u16 operand is invalid"))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum BindingClass {
    Direct,
    DirectNumber,
    Captured,
    Uninitialized,
    Other,
}

fn binding_class(binding: &FrameBinding) -> BindingClass {
    match binding {
        FrameBinding::Direct(value) if value.as_number_repr().is_some() => {
            BindingClass::DirectNumber
        }
        FrameBinding::Direct(_) => BindingClass::Direct,
        FrameBinding::Captured(_) => BindingClass::Captured,
        FrameBinding::Uninitialized => BindingClass::Uninitialized,
        FrameBinding::Private(_) | FrameBinding::PrivateCallable(_) => BindingClass::Other,
    }
}

fn is_immediate(value: &JsValue) -> bool {
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

fn number_value(number: Number) -> JsValue {
    match number {
        Number::Int(value) => JsValue::Int(value),
        Number::Float(value) => JsValue::Float(value),
    }
}

#[inline(always)]
fn dense_binary_rhs(slots: &FrameSlots<'_>, mode: u32, bits: u16) -> Option<Number> {
    match mode {
        0 => slots
            .direct_value(DirectSlot::Local(bits))
            .and_then(JsValue::as_number_repr),
        1 => slots
            .direct_value(DirectSlot::Argument(bits))
            .and_then(JsValue::as_number_repr),
        2 => Some(Number::Int(i32::from(bits as i16))),
        _ => unreachable!("published numeric source was verified"),
    }
}

#[inline(never)]
fn try_dense_read_binary(
    slots: &FrameSlots<'_>,
    state: &mut RuntimeState,
    base: DirectSlot,
    key: DirectSlot,
    rhs_mode: u32,
    rhs_bits: u16,
    operation: Opcode,
) -> Option<JsValue> {
    if !slots.has_operand_capacity(2) {
        return None;
    }
    let Number::Int(index) = slots.direct_value(key)?.as_number_repr()? else {
        return None;
    };
    let index = u32::try_from(index).ok()?;
    let right = dense_binary_rhs(slots, rhs_mode, rhs_bits)?;
    let left = state.peek_dense_number(slots.direct_value(base)?, index)?;
    Some(binary_number_result(operation, left, right))
}

#[inline(never)]
fn try_dense_index_binary(
    slots: &FrameSlots<'_>,
    state: &mut RuntimeState,
    base: DirectSlot,
    key: DirectSlot,
    rhs_mode: u32,
    rhs_bits: u16,
    operation: Opcode,
) -> Option<Number> {
    if !slots.has_operand_capacity(3) {
        return None;
    }
    let key = slots.direct_value(key)?.as_number_repr()?;
    let rhs = dense_binary_rhs(slots, rhs_mode, rhs_bits)?;
    let JsValue::Int(index) = binary_number_result(operation, key, rhs) else {
        return None;
    };
    let index = u32::try_from(index).ok()?;
    state.peek_dense_number(slots.direct_value(base)?, index)
}

#[inline(always)]
fn compare_direct_numbers(opcode: u16, left: Number, right: Number) -> bool {
    let (left, right) = (left.float(), right.float());
    match opcode {
        x if x == Opcode::Lt as u16 => left < right,
        x if x == Opcode::Lte as u16 => left <= right,
        x if x == Opcode::Gt as u16 => left > right,
        x if x == Opcode::Gte as u16 => left >= right,
        x if x == Opcode::Eq as u16 || x == Opcode::StrictEq as u16 => left == right,
        x if x == Opcode::Neq as u16 || x == Opcode::StrictNeq as u16 => left != right,
        _ => unreachable!("published comparison opcode was verified"),
    }
}

fn binary_number_result(opcode: Opcode, left: Number, right: Number) -> JsValue {
    match opcode {
        Opcode::Add => number_value(left.add(right)),
        Opcode::Sub => number_value(left.sub(right)),
        Opcode::Mul => number_value(left.mul(right)),
        Opcode::Div => number_value(left.div(right)),
        Opcode::Mod => number_value(left.rem(right)),
        Opcode::Pow => number_value(left.pow(right)),
        Opcode::Shl => JsValue::Int(left.int32().wrapping_shl(right.int32() as u32 & 31)),
        Opcode::Sar => JsValue::Int(left.int32() >> (right.int32() as u32 & 31)),
        Opcode::Shr => number_value(Number::compact(f64::from(
            (left.int32() as u32) >> (right.int32() as u32 & 31),
        ))),
        Opcode::BitAnd => JsValue::Int(left.int32() & right.int32()),
        Opcode::BitOr => JsValue::Int(left.int32() | right.int32()),
        Opcode::BitXor => JsValue::Int(left.int32() ^ right.int32()),
        Opcode::Eq | Opcode::StrictEq => JsValue::Bool(left.float() == right.float()),
        Opcode::Neq | Opcode::StrictNeq => JsValue::Bool(left.float() != right.float()),
        Opcode::Lt => JsValue::Bool(left.float() < right.float()),
        Opcode::Lte => JsValue::Bool(left.float() <= right.float()),
        Opcode::Gt => JsValue::Bool(left.float() > right.float()),
        Opcode::Gte => JsValue::Bool(left.float() >= right.float()),
        _ => unreachable!("non-numeric opcode reached Number-only handler"),
    }
}

fn borrowed_this_read_ready_in_state(state: &RuntimeState, base: &JsValue) -> bool {
    let JsValue::Object(id) = base else {
        return false;
    };
    state
        .heap
        .object_strong_count(*id)
        .is_ok_and(|count| count != 0 && count < u32::MAX - 2)
}

// Removing the temporary this owner must not remove a checked-retain failure,
// immortal transition, or cleanup checkpoint. Leave room for an aliasing result.
#[cfg(test)]
mod captured_read_tests;
#[cfg(test)]
mod continuous_call_tests;
#[cfg(test)]
mod dynamic_ret_tests;

#[cfg(test)]
mod execution_span_tests {
    use crate::engine::api::{Runtime, Value};

    #[test]
    fn carried_call_continuation_resumes_nested_method_and_throwing_calls_once() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        assert_eq!(
            context
                .eval(
                    r#"(() => {
            let log=[];
            function f(x){log.push(x);return x+1;}
            const obj={m(x){return f(x)+this.bias},bias:2};
            function tail(x){return obj.m(x);}
            let value=f(f(1))+tail(4);
            try{(function(){throw 7;})()}catch(e){value+=e;}
            return value===17 && log.join() === '1,2,4';
        })()"#
                )
                .unwrap(),
            Value::Bool(true)
        );
    }

    #[test]
    fn borrowed_this_field_handles_aliases_prototypes_accessors_and_primitives() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        assert_eq!(
            context
                .eval(
                    r#"(() => {
            function read() { return this.x; }
            let proto = {x: 3}, obj = Object.create(proto), calls = 0;
            for(let i=0;i<20;i++) if(read.call(obj)!==3) return false;
            proto.x=4;
            if(read.call(obj)!==4) return false;
            obj.x=obj;
            if(read.call(obj)!==obj) return false;
            delete obj.x;
            Object.defineProperty(proto,'x',{configurable:true,get(){calls++;return this;}});
            if(read.call(obj)!==obj || calls!==1) return false;
            let proxy=new Proxy(obj,{get(t,k,r){calls++;return 9;}});
            if(read.call(proxy)!==9 || calls!==2) return false;
            delete proto.x;
            if(read.call(obj)!==undefined) return false;
            function length(){return this.length;}
            if(length.call('abc')!==3) return false;
            function strict(){'use strict';return this.x;}
            try{strict.call(null);return false;}catch(e){if(!(e instanceof TypeError))return false;}
            globalThis.x=42;
            if(read.call(null)!==42) return false;
            delete globalThis.x;
            return true;
        })()"#
                )
                .unwrap(),
            Value::Bool(true)
        );
        let Value::Object(function) = context.eval("(function(){return this.x})").unwrap() else {
            panic!("function");
        };
        let callable = runtime.as_callable(&function).unwrap().unwrap();
        let crate::engine::vm::call::CallableExecution::Bytecode { bytecode, .. } =
            runtime.bytecode_for_callable(&callable).unwrap()
        else {
            panic!("bytecode");
        };
        let executable = runtime.snapshot_function_bytecode(&bytecode).unwrap();
        assert!(
            (0..executable.exec.instruction_len()).any(|pc| executable.exec.opcode_at_source(pc)
                == Some(crate::engine::code::exec_opcode::Opcode::BorrowedFieldThis))
        );
    }

    #[test]
    fn borrowed_this_admission_preserves_counts_without_draining_external_cleanup() {
        use crate::engine::{heap::RawId, value::JsValue};
        let runtime = Runtime::new();
        let object = runtime.new_object(None).unwrap();
        let id = object.object_id();
        let value = JsValue::Object(id);
        assert!(super::borrowed_this_read_ready_in_state(
            &runtime.0.state.borrow(),
            &value
        ));
        for count in [u32::MAX - 2, u32::MAX - 1, u32::MAX] {
            runtime
                .0
                .state
                .borrow_mut()
                .heap
                .set_strong_count_for_test(RawId::Object(id), count);
            assert!(!super::borrowed_this_read_ready_in_state(
                &runtime.0.state.borrow(),
                &value
            ));
        }
        runtime
            .0
            .state
            .borrow_mut()
            .heap
            .set_strong_count_for_test(RawId::Object(id), 1);
        let pending = runtime.new_object(None).unwrap();
        {
            let _borrow = runtime.0.state.borrow();
            drop(pending);
        }
        assert!(super::borrowed_this_read_ready_in_state(
            &runtime.0.state.borrow(),
            &value
        ));
        assert!(runtime.0.deferred_references.has_pending());
    }

    #[test]
    fn strict_local_completion_preserves_all_value_kinds() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        let result = context.eval(r#"
            (() => {
                function eq(a,b) { const materialize={}; return a === b; }
                function ne(a,b) { const materialize={}; return a !== b; }
                function branch(a,b) { const box={x:a}; if(box.x === b) return true; return false; }
                function branchNe(a,b) { const box={x:a}; if(box.x !== b) return true; return false; }
                let calls=0;
                const o={valueOf(){calls++;throw 1}}, p=new Proxy(o,{get(){calls++;throw 2}});
                const s=Symbol('s');
                const large=123456789012345678901234567890n;
                const rows=[
                    [undefined,undefined,true], [null,null,true], [null,undefined,false],
                    [true,true,true], [true,false,false], [true,1,false],
                    [0,-0,true], [NaN,NaN,false], [Infinity,Infinity,true],
                    [o,o,true], [o,{},false], [o,null,false], [p,p,true], [p,o,false],
                    [s,s,true], [s,Symbol('s'),false], [s,'s',false],
                    ['abc','a'+'bc',true], ['abc','abd',false], ['1',1,false],
                    [1n,1n,true], [1n,2n,false], [1n,1,false],
                    [large,large+0n,true], [large,large+1n,false], [1n,large,false],
                    [large,1n,false], [large,o,false]
                ];
                for(const row of rows) {
                    const [a,b,want]=row;
                    if(eq(a,b)!==want || ne(a,b)===want || branch(a,b)!==want || branchNe(a,b)===want)
                        return false;
                }
                return calls===0;
            })()
        "#).unwrap();
        assert_eq!(result, Value::Bool(true));
    }

    #[test]
    fn strict_local_completion_preserves_operand_effects_and_throws() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        let result = context.eval(r#"
            (() => {
                let log='';
                function left() { log+='l'; return {}; }
                function right() { log+='r'; return {}; }
                function fail() { log+='x'; throw 'stop'; }
                function value() { const materialize={}; return left()===right(); }
                function branch() { const materialize={}; if(left()!==right()) return true; return false; }
                if(value() || !branch() || log!=='lrlr') return false;
                try { left()===fail(); return false; } catch(e) { if(e!=='stop') return false; }
                const revoked=Proxy.revocable({},{}); revoked.revoke();
                const materialize={};
                return log==='lrlrlx' && revoked.proxy===revoked.proxy && revoked.proxy!==null;
            })()
        "#).unwrap();
        assert_eq!(result, Value::Bool(true));
    }

    #[test]
    fn strict_local_completion_releases_temporary_heap_owners() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        let _ = context
            .eval(
                r#"
            function strictTemporaryOwners() {
                const materialize={};
                for(let i=0;i<50;i++) {
                    if({}==={}) throw 'object identity';
                    if(('a'.repeat(40))!==('a'.repeat(40))) throw 'string content';
                    if((123456789012345678901234567890n+1n)!==(123456789012345678901234567890n+1n))
                        throw 'bigint content';
                    if(Symbol('s')===Symbol('s')) throw 'symbol identity';
                }
                return true;
            }
            strictTemporaryOwners();
        "#,
            )
            .unwrap();
        runtime.run_gc().unwrap();
        let before = runtime.heap_counts().expect("runtime state").live;
        assert_eq!(
            context.eval("strictTemporaryOwners()").unwrap(),
            Value::Bool(true)
        );
        runtime.run_gc().unwrap();
        assert_eq!(runtime.heap_counts().expect("runtime state").live, before);
    }

    #[test]
    fn compare_branch_guard_preserves_coercion_and_branch_result() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        let result = context
            .eval(
                "(() => { let calls=0; function f(a,b){ if(a<b)return 1; return 2; } \
                 const value={valueOf(){calls++;return 1}}; \
                 return f(1,2)===1 && f(3,2)===2 && f(value,2)===1 && calls===1; })()",
            )
            .unwrap();
        assert_eq!(result, Value::Bool(true));
    }

    #[test]
    fn stack_compare_branch_falls_back_before_coercion_and_keeps_exception_order() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        let result = context
            .eval(
                "(() => { let log=''; function f(o, y) { if (o.x < y) return 1; return 2; } \
             let o={get x(){log+='g';return {valueOf(){log+='v';return 1}}}}; \
             let a=f(o,2), b=f({x:3},2); \
             let thrown=false; try { f({get x(){throw 'boom'}},2) } catch(e) { thrown=e==='boom' } \
             return a===1 && b===2 && log==='gv' && thrown; })()",
            )
            .unwrap();
        assert_eq!(result, Value::Bool(true));
    }

    #[test]
    fn discarded_local_update_preserves_postfix_and_coercion() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        let result = context
            .eval(
                "(() => { let calls=0; function f(v) { let x=v; x++; return x; } \
                 let object={valueOf(){calls++;return 4}}; \
                 return f(3)===4 && f(object)===5 && calls===1; })()",
            )
            .unwrap();
        assert_eq!(result, Value::Bool(true));
    }

    #[test]
    fn dense_post_update_read_keeps_old_index_and_proxy_fallback() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        let result = context
            .eval(
                "(() => { function f(a,i){ let value=a[i++]; return [value,i]; } \
                 let dense=f([11,22],0), calls=''; \
                 let proxy=new Proxy([11],{get(t,k){calls+=String(k);return Reflect.get(t,k)}}); \
                 let slow=f(proxy,0); \
                 return dense[0]===11 && dense[1]===1 && slow[0]===11 \
                     && slow[1]===1 && calls==='0'; })()",
            )
            .unwrap();
        assert_eq!(result, Value::Bool(true));
    }

    #[test]
    fn dense_read_binary_preserves_getter_and_conversion_order_on_guard_miss() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        let result = context
            .eval(
                "(() => { function f(a,i,x){ return a[i]+x; } \
                 let log=''; const proxy=new Proxy([2],{get(t,k){log+='g';return Reflect.get(t,k)}}); \
                 const object={valueOf(){log+='v';return 3}}; \
                 function mask(a,i){ return a[i]&0x3fff; } \
                 return f([2],0,3)===5 && f(proxy,0,3)===5 \
                     && f([2],0,object)===5 && mask([65535],0)===16383 \
                     && mask(proxy,0)===2 && log==='gvg'; })()",
            )
            .unwrap();
        assert_eq!(result, Value::Bool(true));
    }

    #[test]
    fn dense_index_binary_falls_back_before_proxy_or_index_coercion() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        let result = context
            .eval(
                "(() => { function f(a,i){ return a[i+1]; } \
                 let log=''; const proxy=new Proxy([7,9],{get(t,k){log+='g';return Reflect.get(t,k)}}); \
                 const key={valueOf(){log+='v';return 0}}; \
                 return f([7,9],0)===9 && f(proxy,0)===9 \
                     && f([7,9],key)===9 && log==='gv'; })()",
            )
            .unwrap();
        assert_eq!(result, Value::Bool(true));
    }

    #[test]
    fn dense_accumulator_index_preserves_writeback_and_guard_order() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        let result = context
            .eval(
                "(() => { function f(a,j){let b=a,i=j,s=0; s+=b[i&3]; return s} \
                 let log=''; const proxy=new Proxy([2],{get(t,k){log+='g';return Reflect.get(t,k)}}); \
                 const key={valueOf(){log+='v';return 0}}; \
                 return f([2],0)===2 && f([1.5],0)===1.5 && f(proxy,0)===2 \
                     && f([2],key)===2 && Number.isNaN(f([2],1)) && log==='gv'; })()",
            )
            .unwrap();
        assert_eq!(result, Value::Bool(true));
    }

    #[test]
    fn field_accumulator_preserves_getter_proxy_and_conversion_order() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        let result = context
            .eval(
                "(() => { function f(o,n){let b=o,s=0; for(let i=0;i<n;i++) s+=b.x; return s} \
                 let log=''; const getter={get x(){log+='g';return 2}}; \
                 const proxy=new Proxy({x:2},{get(t,k){log+='p';return Reflect.get(t,k)}}); \
                 const value={valueOf(){log+='v';return 3}}; \
                 return f({x:2},3)===6 && f(getter,2)===4 && f(proxy,1)===2 \
                     && f({x:value},1)===3 && log==='ggpv'; })()",
            )
            .unwrap();
        assert_eq!(result, Value::Bool(true));
    }
}

#[cfg(all(test, feature = "profiling"))]
mod named_native_fact_tests;
