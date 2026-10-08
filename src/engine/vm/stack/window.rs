//! One authenticated continuous execution borrow. No arena mutation API escapes.
use super::{Error, FrameBinding, FrameWindow, JsValue, Runtime, SlotStore};
use crate::engine::value::number::operations::Number;

mod getter;

/// One checked external entry followed by ordinary frame transitions.
///
/// The exclusive execution borrow prevents a caller, legacy helper, or host
/// callback from replacing the admitted stores. The only structural changes
/// available through this lease are ordinary installation and retirement;
/// both publish their resulting current window before returning. A frame turn
/// lends that actual window, rather than accepting a detached window identity.
/// End this lease before crossing any other driver or host boundary.
pub(in crate::engine::vm) struct FrameExecution<'a> {
    execution: &'a mut crate::engine::vm::execution::RunningExecution,
}

/// Borrowed projections of the current frame. No frame window or JS owner is
/// copied, and these borrows prevent a call/return transition during execution.
pub(in crate::engine::vm) struct FrameTurn<'a> {
    #[cfg(test)]
    pub id: crate::engine::vm::frame::FrameId,
    pub property_generation: &'a mut u64,
    pub active_frame: crate::engine::vm::frames::ActiveFrameToken,
    pub owners: &'a mut crate::engine::vm::frame::FrameCold,
    pub executable: &'a crate::engine::code::runtime::PublishedFunctionSnapshot,
    pub transaction: FrameTransaction<'a>,
    pub fault_pc: &'a mut usize,
    pub resume_pc: &'a mut usize,
    pub pending: &'a mut Option<JsValue>,
    pub selected_native: &'a mut Option<crate::engine::object::LinkedNativeSelection>,
    pub selected_named_read: &'a mut Option<crate::engine::vm::property_driver::SelectedNamedRead>,
}

impl<'a> FrameExecution<'a> {
    /// External and legacy entry remains checked. Internal turns use the
    /// actual top frame established by this admission or a completed producer.
    pub(in crate::engine::vm) fn admit(
        execution: &'a mut crate::engine::vm::execution::RunningExecution,
        id: crate::engine::vm::frame::FrameId,
    ) -> Result<Self, Error> {
        let frame = execution.frames.current_mut(id)?;
        execution.slots.check_current(&frame.window)?;
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_execution_event("core.window_authentication");
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_execution_event("core.pc_authentication");
        if !frame.executable.exec.is_boundary(frame.resume_pc) {
            return Err(Error::internal(
                "execution entry PC is not an instruction boundary",
            ));
        }
        Ok(Self { execution })
    }

    /// Internal re-entry of the actual current frame. The frame identity is
    /// always compared; window and PC checks run in checked builds only.
    #[inline]
    pub(in crate::engine::vm) fn admit_trusted(
        execution: &'a mut crate::engine::vm::execution::RunningExecution,
        id: crate::engine::vm::frame::FrameId,
    ) -> Result<Self, Error> {
        #[cfg(any(debug_assertions, feature = "checked-handles"))]
        {
            Self::admit(execution, id)
        }
        #[cfg(not(any(debug_assertions, feature = "checked-handles")))]
        {
            execution.frames.current_mut(id)?;
            Ok(Self { execution })
        }
    }

    /// Whether one more native continuation fits the logical frame limit.
    #[inline]
    pub(in crate::engine::vm) fn can_push_with_continuations(&self) -> bool {
        self.execution.frames.can_push_with_continuations(0)
    }

    #[inline(always)]
    pub(in crate::engine::vm) fn frame(&mut self) -> FrameTurn<'_> {
        let (_id, frame) = self
            .execution
            .frames
            .current_frame_mut()
            .expect("an admitted ordinary execution has a current frame");
        let body = &mut *frame.cold;
        FrameTurn {
            #[cfg(test)]
            id: _id,
            property_generation: &mut frame.property_generation,
            active_frame: frame.active_frame,
            owners: &mut body.owners,
            executable: &body.executable,
            // Admission checked this window. The store cannot change during
            // a turn; ordinary install/pop establishes the next current one.
            transaction: FrameTransaction {
                store: &mut self.execution.slots,
                window: &mut body.window,
            },
            fault_pc: &mut frame.fault_pc,
            resume_pc: &mut frame.resume_pc,
            pending: &mut self.execution.pending,
            selected_native: &mut self.execution.selected_native,
            selected_named_read: &mut self.execution.selected_named_read,
        }
    }

    /// End the frame projection before making its current PC and ancestors
    /// observable. Registration uses the segment's existing state access.
    pub(in crate::engine::vm) fn materialize_in_state(
        &mut self,
        state: &mut crate::engine::heap::runtime::RuntimeState,
    ) -> Result<(), Error> {
        self.execution.frames.materialize_in_state(state)
    }

    #[allow(clippy::too_many_arguments)]
    pub(in crate::engine::vm) fn enter_ordinary(
        &mut self,
        runtime: &Runtime,
        state: &mut crate::engine::heap::runtime::RuntimeState,
        arguments: u16,
        method: bool,
        tail: bool,
        fallthrough: crate::engine::vm::execute::FallthroughPc,
    ) -> Result<crate::engine::vm::driver::ordinary::Entry, Error> {
        use crate::engine::vm::driver::ordinary::{Entry, prepare_ordinary_in_state};
        // A weak outer method hint may remain while its ordinary arguments
        // execute. Preflight declines only if the actual callee matches it.
        let (prepared, depth) = {
            let turn = self.frame();
            let depth = turn.transaction.window.depth;
            let prepared = prepare_ordinary_in_state(
                runtime,
                state,
                &turn.transaction,
                turn.executable,
                *turn.fault_pc,
                usize::from(arguments),
                method,
                turn.selected_native,
            )?;
            (prepared, depth)
        };
        let Some((call, checked)) = prepared else {
            return Ok(Entry::General);
        };
        if !self.execution.frames.can_push() || runtime.bytecode_call_would_overflow() {
            #[cfg(feature = "profiling")]
            crate::engine::api::profiling::record_owned_execution_event(
                "core.call_decline.overflow",
            );
            return Ok(Entry::General);
        }
        // The same exclusive lease produced these operands. Between preflight
        // and consumption, only reservation/immutable authentication can occur.
        self.install_current_ordinary(runtime, state, call, checked, tail, fallthrough)?;
        #[cfg(feature = "profiling")]
        {
            crate::engine::api::profiling::record_owned_execution_event(
                "ordinary_call.carried_fallthrough",
            );
            crate::engine::api::profiling::record_owned_instruction(depth);
        }
        #[cfg(not(feature = "profiling"))]
        let _ = depth;
        Ok(Entry::Ordinary)
    }

    // Constructor selection, receiver allocation and guarded publication are
    // a complete transaction. Keep its allocation/rollback machinery outside
    // the instruction loop; the ordinary call and numeric paths stay local.
    #[inline(never)]
    pub(in crate::engine::vm) fn enter_constructor(
        &mut self,
        runtime: &Runtime,
        state: &mut crate::engine::heap::runtime::RuntimeState,
        arguments: u16,
        fallthrough: crate::engine::vm::execute::FallthroughPc,
    ) -> Result<bool, Error> {
        use crate::engine::heap::runtime::owned_values::OwnedValueGuard;
        if !self.execution.frames.can_push_with_continuations(0)
            || runtime.bytecode_call_would_overflow()
        {
            return Ok(false);
        }
        let count = usize::from(arguments);
        let (selected, observed_depth) = {
            let turn = self.frame();
            let depth = turn.transaction.window.depth;
            let selected = crate::engine::vm::construct_driver::prepare_ordinary_base_in_state(
                runtime,
                state,
                &turn.transaction,
                count,
            )?;
            (selected, depth)
        };
        let Some((call, prototype)) = selected else {
            return Ok(false);
        };
        // Receiver allocation may request GC, but service remains with the
        // execution consumer after this installer publishes every input owner.
        let receiver = state
            .new_empty_ordinary_object(&runtime.0.poisoned, prototype)
            .map_err(super::runtime_error_to_vm_error)?;
        let mut receiver =
            OwnedValueGuard::new(state, &runtime.0.poisoned, JsValue::Object(receiver));
        let (state, receiver) = receiver.parts();
        self.install_current_constructor(runtime, state, call, count, fallthrough, receiver)?;
        #[cfg(feature = "profiling")]
        {
            crate::engine::api::profiling::record_owned_instruction(observed_depth);
            crate::engine::api::profiling::record_owned_execution_event(
                "constructor_base_lazy_install",
            );
            crate::engine::api::profiling::record_owned_execution_event("constructor_argv_elided");
        }
        #[cfg(not(feature = "profiling"))]
        let _ = observed_depth;
        Ok(true)
    }

    /// The legacy call driver supplies a detached checked operand witness.
    /// Recheck its identity/depth within this admitted lease before invoking
    /// the same installer used by uninterrupted internal calls.
    #[allow(clippy::too_many_arguments)]
    pub(in crate::engine::vm) fn install_ordinary(
        &mut self,
        runtime: &Runtime,
        state: &mut crate::engine::heap::runtime::RuntimeState,
        call: crate::engine::vm::call::ordinary::OrdinaryCall,
        checked: CheckedOrdinaryCallOperands,
        tail: bool,
        fallthrough: crate::engine::vm::execute::FallthroughPc,
    ) -> Result<(), Error> {
        {
            let turn = self.frame();
            if turn.transaction.window.id != checked.window_id
                || turn.transaction.window.depth != checked.depth
            {
                return Err(Error::internal(
                    "ordinary call operands changed after validation",
                ));
            }
        }
        self.install_current_ordinary(runtime, state, call, checked, tail, fallthrough)
    }

    /// Sole ordinary slot installer. This method cannot be called through a
    /// raw RunningExecution or a caller-supplied current-frame identity.
    #[allow(clippy::too_many_arguments)]
    fn install_current_ordinary(
        &mut self,
        runtime: &Runtime,
        state: &mut crate::engine::heap::runtime::RuntimeState,
        call: crate::engine::vm::call::ordinary::OrdinaryCall,
        checked: CheckedOrdinaryCallOperands,
        tail: bool,
        fallthrough: crate::engine::vm::execute::FallthroughPc,
    ) -> Result<(), Error> {
        #[cfg(feature = "profiling")]
        let _timer =
            crate::engine::api::profiling::PhaseTimer::start_vm_sampled("ordinary.install.sampled");
        #[cfg(feature = "profiling")]
        {
            let count = checked.count();
            let method = checked.method();
            use crate::engine::api::profiling::record_owned_execution_event as record;
            record(if method {
                "ordinary_install.method"
            } else {
                "ordinary_install.function"
            });
            record(match count {
                0 => "ordinary_install.args0",
                1 => "ordinary_install.args1",
                2 => "ordinary_install.args2",
                3 => "ordinary_install.args3",
                _ => "ordinary_install.args4plus",
            });
        }
        self.install_current_child(
            runtime,
            state,
            call,
            ChildEntry::Ordinary { checked, tail },
            fallthrough,
        )
    }

    /// Sole Base constructor installer under the same exclusive lease that
    /// selected its actual callee/newTarget/arguments. Receiver edges remain
    /// guarded until the common slot initializer and frame publication succeed.
    #[allow(clippy::too_many_arguments)]
    fn install_current_constructor(
        &mut self,
        runtime: &Runtime,
        state: &mut crate::engine::heap::runtime::RuntimeState,
        call: crate::engine::vm::call::ordinary::OrdinaryCall,
        count: usize,
        fallthrough: crate::engine::vm::execute::FallthroughPc,
        receiver: &mut Option<JsValue>,
    ) -> Result<(), Error> {
        use crate::engine::heap::runtime::owned_values::OwnedValueGuard;
        // The guard owns the receiver edge, so its copy takes the trusted retain.
        let this_value = state
            .dup_owned_jsvalue(receiver.as_ref().expect("guarded constructor receiver"))
            .map_err(super::runtime_error_to_vm_error)?;
        let mut this_value = OwnedValueGuard::new(state, &runtime.0.poisoned, this_value);
        let (state, this_value) = this_value.parts();
        self.install_current_child(
            runtime,
            state,
            call,
            ChildEntry::Constructor {
                count,
                this_value,
                receiver,
            },
            fallthrough,
        )
    }

    /// The one frame publication shared by ordinary calls and Base
    /// constructors. Inlined into both installers, so each keeps a
    /// specialized body.
    #[inline(always)]
    fn install_current_child(
        &mut self,
        runtime: &Runtime,
        state: &mut crate::engine::heap::runtime::RuntimeState,
        call: crate::engine::vm::call::ordinary::OrdinaryCall,
        entry: ChildEntry<'_>,
        fallthrough: crate::engine::vm::execute::FallthroughPc,
    ) -> Result<(), Error> {
        use crate::engine::vm::frame::{
            ConstructorReturn, Frame, ReturnOwner, ReturnTarget, ReturnValue,
        };
        let execution = &mut *self.execution;
        let (function, executable, closure) = call.into_slot_parts();
        let depth = execution.frames.depth() + 1;
        execution.call_storage.reserve_depth(depth)?;
        let (parent, frame) = execution
            .frames
            .current_frame_mut()
            .expect("an admitted call has a caller");
        let caller_realm = frame.executable.realm;
        let (flags, flag_bytes) = if executable.has_captured_locals {
            execution
                .call_storage
                .capture_flags(executable.local_definitions.len())?
        } else {
            (Vec::new(), 0)
        };
        let mut prepared = execution.frames.prepare_push()?;
        let (_, frame) = prepared
            .current_frame_mut()
            .expect("frame publication retains its caller");
        let transaction = FrameTransaction {
            store: &mut execution.slots,
            window: &mut frame.cold.window,
        };
        let layout = executable.frame_layout();
        // The private continuation comes from the instruction that produced
        // this call. No caller instruction or slot changed during preflight.
        let (installed, tail, receiver) = match entry {
            ChildEntry::Ordinary { checked, tail } => {
                #[cfg(feature = "profiling")]
                let _timer = crate::engine::api::profiling::PhaseTimer::start_vm_sampled(
                    "ordinary.install.slots.sampled",
                );
                let installed = transaction.install_ordinary_window(
                    runtime,
                    state,
                    &layout,
                    checked,
                    function,
                    executable.observes_arguments,
                )?;
                (installed, tail, None)
            }
            ChildEntry::Constructor {
                count,
                this_value,
                receiver,
            } => {
                let installed = transaction.install_constructor_window(
                    runtime,
                    state,
                    &layout,
                    count,
                    function,
                    executable.observes_arguments,
                    this_value,
                )?;
                (installed, false, Some(receiver))
            }
        };
        frame.resume_pc = fallthrough.index();
        let (mut cold, frame_bytes) = if receiver.is_some() {
            execution.call_storage.vacant_rare()
        } else {
            execution.call_storage.vacant(caller_realm)
        };
        cold.occupy(
            ReturnTarget {
                value_use: ReturnValue::Push,
                owner: ReturnOwner::Frame(parent),
                tail,
                operation: None,
            },
            crate::engine::vm::closure::FrameFunction::shared(
                runtime.domain_id(),
                installed.function,
                closure,
            ),
            installed.input,
            flags,
            executable,
            installed.window,
        );
        if let Some(receiver) = receiver {
            cold.constructor_return = Some(ConstructorReturn::Base(
                receiver
                    .take()
                    .expect("guarded constructor return receiver"),
            ));
        }
        prepared.install(Frame {
            property_generation: 0,
            iterator_generation: 0,
            caller_realm,
            active_frame: crate::engine::vm::frames::ActiveFrameToken::unmaterialized(),
            fault_pc: 0,
            resume_pc: 0,
            cold,
        });
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_call_storage(frame_bytes, flag_bytes, 0);
        #[cfg(not(feature = "profiling"))]
        let _ = (frame_bytes, flag_bytes);
        Ok(())
    }

    /// Retire actual current frames, preserving a registered result through
    /// Base normalization and tail propagation. Legacy roots/continuations
    /// return with the actual remaining frame and pending owner untouched.
    pub(in crate::engine::vm) fn finish_ordinary(
        &mut self,
        runtime: &Runtime,
        state: &mut crate::engine::heap::runtime::RuntimeState,
    ) -> Result<crate::engine::vm::driver::ordinary::ReturnProgress, Error> {
        use crate::engine::vm::{driver::ordinary::ReturnProgress, frame::ReturnValue};
        let execution = &mut *self.execution;
        loop {
            let (_, frame) = execution
                .frames
                .current_frame_mut()
                .expect("an admitted ordinary return has a current frame");
            let Some(target) = frame.cold.state_return() else {
                #[cfg(feature = "profiling")]
                frame.cold.record_state_return_decline();
                return Ok(ReturnProgress::Declined);
            };
            if target.operation.is_some() {
                #[cfg(feature = "profiling")]
                crate::engine::api::profiling::record_owned_execution_event(
                    "core.return_decline.operation",
                );
                return Ok(ReturnProgress::Declined);
            }
            if frame.executable.root().is_some() {
                #[cfg(feature = "profiling")]
                crate::engine::api::profiling::record_owned_execution_event(
                    "core.return_decline.public_root",
                );
                return Ok(ReturnProgress::Declined);
            }
            if execution.pending.is_none() {
                #[cfg(feature = "profiling")]
                crate::engine::api::profiling::record_owned_execution_event(
                    "core.return_decline.missing_result",
                );
                return Ok(ReturnProgress::Declined);
            }
            if let Err(error) = frame
                .cold
                .normalize_base_return_in_state(state, &mut execution.pending)
            {
                runtime.0.poisoned.set(true);
                return Err(super::runtime_error_to_vm_error(error));
            }
            let mut frame = execution
                .frames
                .pop_current()
                .expect("ordinary retirement owns the actual top frame");
            if let Err(error) = execution.slots.clear_current_frame_owned_in_state(
                state,
                &runtime.0.poisoned,
                frame.window.take(),
            ) {
                runtime.0.poisoned.set(true);
                return Err(error);
            }
            if let Err(error) = execution.call_storage.recycle(state, frame.cold) {
                runtime.0.poisoned.set(true);
                return Err(super::runtime_error_to_vm_error(error));
            }
            // Return destinations may originate in a legacy/materialized entry;
            // preserve this semantic routing check, independently of admission.
            let destination = target.frame()?;
            let (current, parent) = execution.frames.current_frame_mut().ok_or_else(|| {
                Error::internal("frame identity is not the current execution frame")
            })?;
            if current != destination {
                return Err(Error::internal(
                    "frame identity is not the current execution frame",
                ));
            }
            if target.tail {
                #[cfg(feature = "profiling")]
                crate::engine::api::profiling::record_owned_execution_event(
                    "ordinary_tail_return_direct",
                );
                continue;
            }
            if matches!(target.value_use, ReturnValue::Push) {
                FrameTransaction {
                    store: &mut execution.slots,
                    window: &mut parent.cold.window,
                }
                .slots()
                .push_pending(&mut execution.pending)?;
            } else {
                let value = execution
                    .pending
                    .take()
                    .expect("registered discarded result");
                if let Err(error) = state.release_jsvalue(value) {
                    runtime.0.poisoned.set(true);
                    return Err(super::runtime_error_to_vm_error(error));
                }
            }
            #[cfg(feature = "profiling")]
            crate::engine::api::profiling::record_owned_execution_event("ordinary_return_direct");
            return Ok(ReturnProgress::Returned);
        }
    }
}

/// What the shared installer moves into the child window.
enum ChildEntry<'a> {
    Ordinary {
        checked: CheckedOrdinaryCallOperands,
        tail: bool,
    },
    Constructor {
        count: usize,
        this_value: &'a mut Option<JsValue>,
        receiver: &'a mut Option<JsValue>,
    },
}

/// A frame-owned binding addressed without an operand-stack value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::engine::vm) enum DirectSlot {
    Local(u16),
    Argument(u16),
}

/// Exclusive ownership of one authenticated frame window across short execution
/// borrows. The store and window cannot be pushed, popped or replaced while this
/// transaction exists. Allocation or observable release may happen between
/// `slots()` borrows; admitted non-observable releases may finish within one.
/// No slot reference may escape into a callback or observable release.
pub(in crate::engine::vm) struct FrameTransaction<'a> {
    store: &'a mut SlotStore,
    window: &'a mut FrameWindow,
}

/// A single-use proof for the outgoing operands of one direct ordinary call.
///
/// The driver may carry this across authentication and allocation, but must not
/// write a caller slot, switch frames, or invoke JavaScript before consuming it.
/// A legacy installer checks window identity and depth again; those checks
/// alone do not certify unchanged slots after an arbitrary intervening write.
/// Internal producers instead consume this within their exclusive execution
/// lease, with no slot mutation or JavaScript between preflight and transfer.
pub(in crate::engine::vm) struct CheckedOrdinaryCallOperands {
    pub(super) window_id: u64,
    pub(super) depth: usize,
    pub(super) count: usize,
    pub(super) method: bool,
}

impl CheckedOrdinaryCallOperands {
    #[cfg(feature = "profiling")]
    pub(in crate::engine::vm) fn count(&self) -> usize {
        self.count
    }

    #[cfg(feature = "profiling")]
    pub(in crate::engine::vm) fn method(&self) -> bool {
        self.method
    }
}

impl FrameTransaction<'_> {
    #[cfg(feature = "profiling")]
    pub(in crate::engine::vm) fn operand_depth(&self) -> usize {
        self.window.depth
    }

    /// Consume the selected caller lease to publish a Base constructor. No
    /// caller-supplied detached window or mutable slot access can intervene.
    #[allow(clippy::too_many_arguments)]
    fn install_constructor_window(
        self,
        runtime: &Runtime,
        state: &mut crate::engine::heap::runtime::RuntimeState,
        layout: &crate::engine::code::function::layout::FrameLayout<'_>,
        count: usize,
        function: crate::engine::heap::ObjectId,
        observes_arguments: bool,
        receiver: &mut Option<JsValue>,
    ) -> Result<super::call::InstalledOrdinaryFrame, Error> {
        self.store.push_current_constructor_frame_in_state(
            runtime,
            state,
            layout,
            self.window,
            count,
            function,
            observes_arguments,
            receiver,
        )
    }
    /// Consume the actual caller window before publishing its child. Only the
    /// private execution producer combines this lease with preflighted operands.
    #[allow(clippy::too_many_arguments)]
    fn install_ordinary_window(
        self,
        runtime: &Runtime,
        state: &mut crate::engine::heap::runtime::RuntimeState,
        layout: &crate::engine::code::function::layout::FrameLayout<'_>,
        checked: CheckedOrdinaryCallOperands,
        function: crate::engine::heap::ObjectId,
        observes_arguments: bool,
    ) -> Result<super::call::InstalledOrdinaryFrame, Error> {
        self.store.push_current_ordinary_frame_in_state(
            runtime,
            state,
            layout,
            self.window,
            checked,
            function,
            observes_arguments,
        )
    }
    pub(in crate::engine::vm) fn peek(&self, offset: usize) -> Result<&JsValue, Error> {
        self.store.peek_current(self.window, offset)
    }
    pub(in crate::engine::vm) fn validate_call_value_domains(
        &self,
        runtime: &Runtime,
        count: usize,
        method: bool,
    ) -> Result<bool, Error> {
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_execution_event("call_value_domain_validation");
        self.store
            .validate_call_value_domains_current(self.window, runtime, count, method)
    }

    /// Validate the receiver, arguments, and callee before an ordinary call's
    /// first fallible authentication step.
    pub(in crate::engine::vm) fn validate_ordinary_call_operands(
        &self,
        count: usize,
        method: bool,
    ) -> Result<CheckedOrdinaryCallOperands, Error> {
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_execution_event("call_value_domain_validation");
        // Preserve the existing domain-check order: receiver, then arguments
        // from left to right. The driver has already peeked the callee for
        // selection; the final read makes the sealed proof complete.
        if method {
            self.peek(
                count
                    .checked_add(1)
                    .ok_or_else(SlotStore::operand_stack_underflow)?,
            )?;
        }
        for offset in (0..count).rev() {
            self.peek(offset)?;
        }
        self.peek(count)?;
        Ok(CheckedOrdinaryCallOperands {
            window_id: self.window.id,
            depth: self.window.depth,
            count,
            method,
        })
    }
    /// A call-site hit. The caller has already read the deepest operand
    /// (receiver or callee) as a direct value; operand slots above it always
    /// hold direct values, so the per-argument reads are skipped.
    #[inline]
    pub(in crate::engine::vm) fn check_ordinary_call_depth(
        &self,
        count: usize,
        method: bool,
    ) -> Result<CheckedOrdinaryCallOperands, Error> {
        debug_assert!((0..=count + usize::from(method)).all(|offset| self.peek(offset).is_ok()));
        Ok(CheckedOrdinaryCallOperands {
            window_id: self.window.id,
            depth: self.window.depth,
            count,
            method,
        })
    }
    /// Consume native operands already checked within this transaction: the
    /// callee by native selection, and receiver/arguments by domain validation.
    /// No mutable slot access may intervene. Classification executes no JS and
    /// keeps the callee in its original slot until this owning transfer.
    pub(in crate::engine::vm) fn take_validated_native_call_operands(
        &mut self,
        runtime: &Runtime,
        logical_active_depth: usize,
        count: usize,
        method: bool,
    ) -> Result<(Vec<JsValue>, JsValue, crate::engine::object::CallableRef), Error> {
        self.store
            .reserve_native_argument_depth(logical_active_depth.saturating_add(1))?;
        let arguments =
            self.store
                .take_native_arguments_current::<true>(self.window, count, method)?;
        let JsValue::Object(function) = self.store.pop_current(self.window)? else {
            unreachable!("native selection authenticated the callee object")
        };
        // The slot's original edge becomes the activation's callable owner.
        // The callee stays continuously live until normal activation cleanup,
        // so its former retain/release pair is unnecessary.
        let callable = crate::engine::object::CallableRef::from_validated_object(
            crate::engine::object::ObjectRef::from_owned_handle(runtime.clone(), function),
        );
        // Releasing the old duplicate also drained unrelated pending edges.
        // Preserve that observation boundary before consuming the receiver.
        if runtime.0.deferred_references.has_pending() {
            drop(
                runtime
                    .operation()
                    .map_err(super::runtime_error_to_vm_error)?,
            );
        }
        let receiver = if method {
            self.store.pop_current(self.window)?
        } else {
            JsValue::Undefined
        };
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_execution_event(
            "native_callee_owner_transferred",
        );
        Ok((arguments, receiver, callable))
    }

    pub(in crate::engine::vm) fn slots(&mut self) -> FrameSlots<'_> {
        FrameSlots {
            store: self.store,
            window: self.window,
        }
    }
}

impl SlotStore {
    pub(in crate::engine::vm) fn frame_transaction<'a>(
        &'a mut self,
        window: &'a mut FrameWindow,
    ) -> Result<FrameTransaction<'a>, Error> {
        self.check_current(window)?;
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_execution_event("core.window_authentication");
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_execution_event(
            "frame_authentication.transaction",
        );
        Ok(FrameTransaction {
            store: self,
            window,
        })
    }
}

pub(in crate::engine::vm) struct FrameSlots<'a> {
    pub(super) store: &'a mut SlotStore,
    pub(super) window: &'a mut FrameWindow,
}

/// An authenticated, short-lived local destination. Required inputs must be read
/// before admission so source/destination aliases preserve their old values.
pub(in crate::engine::vm) struct AdmittedLocalDestination<'a> {
    slot: &'a mut JsValue,
    pub old_number: Option<Number>,
}

impl AdmittedLocalDestination<'_> {
    #[inline(always)]
    pub(in crate::engine::vm) fn commit(self, value: Number) {
        *self.slot = match value {
            Number::Int(value) => JsValue::Int(value),
            Number::Float(value) => JsValue::Float(value),
        };
    }
}

impl FrameSlots<'_> {
    /// Admit a numeric local and borrow a distinct direct Array source in one
    /// frame access. The receiver remains rooted by its frame binding for the
    /// lifetime of both borrows; no owning handle is created.
    pub(in crate::engine::vm) fn admit_numeric_local_with_source(
        &mut self,
        destination: u16,
        source: DirectSlot,
    ) -> Option<(AdmittedLocalDestination<'_>, &JsValue)> {
        self.admit_local_with_source(destination, source, true)
    }

    /// Product assignment only replaces an initialized, owner-free scalar;
    /// its displaced value need not participate in Number arithmetic.
    pub(in crate::engine::vm) fn admit_scalar_local_with_source(
        &mut self,
        destination: u16,
        source: DirectSlot,
    ) -> Option<(AdmittedLocalDestination<'_>, &JsValue)> {
        self.admit_local_with_source(destination, source, false)
    }

    fn admit_local_with_source(
        &mut self,
        destination: u16,
        source: DirectSlot,
        require_numeric: bool,
    ) -> Option<(AdmittedLocalDestination<'_>, &JsValue)> {
        let locals = self.window.locals();
        let destination = locals.start.checked_add(usize::from(destination))?;
        if destination >= locals.end {
            return None;
        }
        let (source_range, source_index) = match source {
            DirectSlot::Local(index) => (locals, usize::from(index)),
            DirectSlot::Argument(index) => (self.window.parameters(), usize::from(index)),
        };
        let source = source_range.start.checked_add(source_index)?;
        if source >= source_range.end || source == destination {
            return None;
        }
        let (destination_binding, source_binding) = if destination < source {
            let (before, after) = self.store.slots.split_at_mut(source);
            (before.get_mut(destination)?, after.first()?.as_ref()?)
        } else {
            let (before, after) = self.store.slots.split_at_mut(destination);
            (after.first_mut()?, before.get(source)?.as_ref()?)
        };
        let FrameBinding::Direct(destination) = destination_binding.as_mut()? else {
            return None;
        };
        let old_number = destination.as_number_repr();
        if require_numeric {
            old_number?;
        } else if !matches!(
            destination,
            JsValue::Undefined
                | JsValue::Null
                | JsValue::Bool(_)
                | JsValue::Int(_)
                | JsValue::Float(_)
        ) {
            return None;
        }
        let FrameBinding::Direct(base) = source_binding else {
            return None;
        };
        Some((
            AdmittedLocalDestination {
                slot: destination,
                old_number,
            },
            base,
        ))
    }

    pub(in crate::engine::vm) fn direct_value(&self, source: DirectSlot) -> Option<&JsValue> {
        let region = match source {
            DirectSlot::Local(_) => self.window.locals(),
            DirectSlot::Argument(_) => self.window.parameters(),
        };
        let index = match source {
            DirectSlot::Local(index) | DirectSlot::Argument(index) => usize::from(index),
        };
        let FrameBinding::Direct(value) = self.store.slots[region].get(index)?.as_ref()? else {
            return None;
        };
        Some(value)
    }

    /// Move a scalar copy straight from a direct binding into the operand
    /// window. No owner is acquired, so a failed capacity check needs no
    /// pending-owner cleanup. Other bindings keep the ordinary owning read.
    #[inline(always)]
    pub(in crate::engine::vm) fn push_direct_immediate(
        &mut self,
        source: DirectSlot,
    ) -> Result<bool, Error> {
        let value = match self.direct_value(source) {
            Some(JsValue::Undefined) => JsValue::Undefined,
            Some(JsValue::Null) => JsValue::Null,
            Some(JsValue::Bool(value)) => JsValue::Bool(*value),
            Some(JsValue::Int(value)) => JsValue::Int(*value),
            Some(JsValue::Float(value)) => JsValue::Float(*value),
            Some(JsValue::ShortBigInt(value)) => JsValue::ShortBigInt(*value),
            _ => return Ok(false),
        };
        self.push(value)?;
        Ok(true)
    }

    /// The caller has verified a direct numeric binding and one free operand
    /// slot. Push first, then replace the scalar binding without transferring
    /// any heap owner.
    pub(in crate::engine::vm) fn commit_numeric_update_and_read(
        &mut self,
        slot: DirectSlot,
        updated: Number,
        read: Number,
    ) -> Result<(), Error> {
        let value = match read {
            Number::Int(value) => JsValue::Int(value),
            Number::Float(value) => JsValue::Float(value),
        };
        self.push(value)?;
        let value = match updated {
            Number::Int(value) => JsValue::Int(value),
            Number::Float(value) => JsValue::Float(value),
        };
        let old = match slot {
            DirectSlot::Local(index) => self.replace_local(index, FrameBinding::Direct(value))?,
            DirectSlot::Argument(index) => {
                self.replace_parameter(index, FrameBinding::Direct(value))?
            }
        };
        debug_assert!(matches!(
            old,
            FrameBinding::Direct(JsValue::Int(_) | JsValue::Float(_))
        ));
        Ok(())
    }

    /// Commit a verified numeric local update whose expression result is
    /// discarded. The old direct number owns no heap resource.
    #[inline]
    pub(in crate::engine::vm) fn commit_number_local_discard(
        &mut self,
        index: u16,
        updated: Number,
    ) -> Result<(), Error> {
        let value = match updated {
            Number::Int(value) => JsValue::Int(value),
            Number::Float(value) => JsValue::Float(value),
        };
        let Some(FrameBinding::Direct(old)) = self.store.slots[self.window.locals()]
            .get_mut(usize::from(index))
            .and_then(Option::as_mut)
        else {
            return Err(Error::internal("numeric local is not a direct binding"));
        };
        if !matches!(old, JsValue::Int(_) | JsValue::Float(_)) {
            return Err(Error::internal("numeric local changed before commit"));
        }
        *old = value;
        Ok(())
    }

    /// Commit a proven numeric ++local and replace the existing top Number.
    /// Both slots are authenticated before either scalar value changes.
    pub(in crate::engine::vm) fn commit_number_local_and_top(
        &mut self,
        index: u16,
        updated: Number,
        result: Number,
    ) -> Result<(), Error> {
        if self.immediate_local(index).is_none() || self.peek(0)?.as_number_repr().is_none() {
            return Err(Error::internal("numeric preincrement admission changed"));
        }
        let local = self.window.locals().start + usize::from(index);
        let top = self.window.operands().start + self.window.depth - 1;
        let number_value = |number| match number {
            Number::Int(value) => JsValue::Int(value),
            Number::Float(value) => JsValue::Float(value),
        };
        self.store.slots[local] = Some(FrameBinding::Direct(number_value(updated)));
        self.store.slots[top] = Some(FrameBinding::Direct(number_value(result)));
        Ok(())
    }

    #[cfg(test)]
    pub(in crate::engine::vm) fn property_ic_read(
        &mut self,
        runtime: &Runtime,
        executable: &crate::engine::code::runtime::PublishedFunctionSnapshot,
        operation: super::NamedReadOperation,
        native: &mut Option<crate::engine::object::LinkedNativeSelection>,
    ) -> Result<super::PropertyReadProgress, Error> {
        self.property_ic_read_in_state(
            &mut runtime.0.state.borrow_mut(),
            &runtime.0.poisoned,
            runtime.domain_id(),
            executable,
            operation,
            native,
        )
    }

    pub(in crate::engine::vm) fn has_operand_capacity(&self, extra: usize) -> bool {
        self.window
            .depth
            .checked_add(extra)
            .is_some_and(|depth| depth <= self.window.end - self.window.locals_end)
    }

    pub(in crate::engine::vm) fn peek(&self, from_top: usize) -> Result<&JsValue, Error> {
        self.store.peek_current(self.window, from_top)
    }

    pub(in crate::engine::vm) fn push(&mut self, value: JsValue) -> Result<(), Error> {
        self.store.push_current(self.window, value)
    }

    /// On failure the caller keeps its owner until this borrow has ended.
    pub(in crate::engine::vm) fn push_pending(
        &mut self,
        value: &mut Option<JsValue>,
    ) -> Result<(), Error> {
        self.store.push_pending_current(self.window, value)
    }

    // Preserve the direct SlotStore call at numeric operand consumers.
    #[inline]
    pub(in crate::engine::vm) fn pop(&mut self) -> Result<JsValue, Error> {
        self.store.pop_current(self.window)
    }

    #[inline(always)]
    pub(in crate::engine::vm) fn local(&self, index: u16) -> Result<&FrameBinding, Error> {
        self.store.local_current(self.window, index)
    }

    #[inline(always)]
    pub(in crate::engine::vm) fn parameter(&self, index: u16) -> Result<&FrameBinding, Error> {
        self.store.parameter_current(self.window, index)
    }

    /// Non-owning numeric read of a direct local binding. Bounds, binding kind
    /// and domain form one guard; every miss leaves canonical diagnostics and
    /// evaluation untouched, and no owner edge is created.
    #[inline]
    pub(in crate::engine::vm) fn immediate_local(&self, index: u16) -> Option<Number> {
        let index = usize::from(index);
        if index >= self.window.locals().len() {
            return None;
        }
        let FrameBinding::Direct(value) =
            self.store.slots[self.window.locals().start + index].as_ref()?
        else {
            return None;
        };
        value.as_number_repr()
    }

    /// Non-owning numeric read of a direct parameter binding; shaped exactly
    /// like `immediate_local` over the parameter region.
    #[inline]
    pub(in crate::engine::vm) fn immediate_parameter(&self, index: u16) -> Option<Number> {
        let index = usize::from(index);
        if index >= self.window.parameters().len() {
            return None;
        }
        let FrameBinding::Direct(value) =
            self.store.slots[self.window.parameters().start + index].as_ref()?
        else {
            return None;
        };
        value.as_number_repr()
    }

    #[inline]
    pub(in crate::engine::vm) fn store_proven_number_operand(
        &mut self,
        destination: DirectSlot,
        keep: bool,
    ) -> bool {
        self.store
            .store_proven_number_operand_current(self.window, destination, keep)
    }

    #[inline(always)]
    pub(in crate::engine::vm) fn replace_local(
        &mut self,
        index: u16,
        value: FrameBinding,
    ) -> Result<FrameBinding, Error> {
        self.store.replace_local_current(self.window, index, value)
    }

    #[inline(always)]
    pub(in crate::engine::vm) fn replace_parameter(
        &mut self,
        index: u16,
        value: FrameBinding,
    ) -> Result<FrameBinding, Error> {
        self.store
            .replace_parameter_current(self.window, index, value)
    }

    pub(in crate::engine::vm) fn rotate_operands(
        &mut self,
        skip_top: usize,
        count: usize,
        left: bool,
    ) -> Result<(), Error> {
        self.store
            .rotate_operands_current(self.window, skip_top, count, left)
    }

    #[cfg(test)]
    pub(in crate::engine::vm) fn array_immediate_read(
        &mut self,
        runtime: &Runtime,
    ) -> Result<bool, Error> {
        self.array_read_in_state(&mut runtime.0.state.borrow_mut(), &runtime.0.poisoned)
    }

    #[cfg(test)]
    pub(in crate::engine::vm) fn ordinary_field_immediate_read(
        &mut self,
        runtime: &Runtime,
        executable: &crate::engine::code::runtime::PublishedFunctionSnapshot,
        key_index: u32,
    ) -> Result<bool, Error> {
        self.store.ordinary_field_immediate_read_current(
            self.window,
            runtime,
            executable,
            key_index,
        )
    }

    pub(in crate::engine::vm) fn binary_number(
        &mut self,
        operation: impl FnOnce(
            crate::engine::value::number::operations::Number,
            crate::engine::value::number::operations::Number,
        ) -> JsValue,
    ) -> Result<bool, Error> {
        self.store.binary_number_current(self.window, operation)
    }

    pub(in crate::engine::vm) fn number_pair_branch(
        &mut self,
        compare: impl FnOnce(
            crate::engine::value::number::operations::Number,
            crate::engine::value::number::operations::Number,
        ) -> bool,
    ) -> Result<Option<bool>, Error> {
        self.store.number_pair_branch_current(self.window, compare)
    }
}

#[cfg(test)]
mod primitive_transaction_tests {
    use super::*;
    use crate::engine::code::function::metadata::{ClosureVariableKind, VariableDefinition};
    use crate::engine::code::region::{DirectSource, NumberSource, PublishedNumericRegion};
    use crate::engine::code::runtime::PublishedFunctionSnapshot;
    use crate::engine::value::Value;
    use crate::engine::vm::stack::FrameStorage;

    fn execution_frame(
        runtime: &Runtime,
        execution: &mut crate::engine::vm::execution::RunningExecution,
        realm: crate::engine::heap::ContextId,
        code: &[crate::engine::code::bytecode::Instruction],
        operands: Vec<JsValue>,
    ) -> crate::engine::vm::frame::FrameId {
        use crate::engine::vm::{
            CallInput,
            closure::FrameFunction,
            frame::{ColdFrame, FrameCold, FrameEntry},
            frames::ActiveFrameToken,
        };
        let mut executable = PublishedFunctionSnapshot::empty_for_test(realm);
        executable.exec = crate::engine::code::exec::ExecCode::encode(code).unwrap();
        executable.metadata.max_stack = 2;
        let function = runtime.new_object(None).unwrap();
        crate::engine::vm::driver::push_frame(
            runtime,
            execution,
            FrameEntry {
                property_generation: 0,
                iterator_generation: 0,
                caller_realm: realm,
                active_frame: ActiveFrameToken(0),
                initialize_bindings: false,
                executable,
                cold: ColdFrame::new(FrameCold {
                    rare: Default::default(),
                    return_to: None,
                    entry_guard: None,
                    function: FrameFunction::new(function, Default::default())
                        .unwrap()
                        .into(),
                    reusable_captured_locals: Vec::new(),
                    input: CallInput::new(JsValue::Undefined, JsValue::Undefined, None).into(),
                }),
                storage: FrameStorage {
                    original_arguments: Vec::new(),
                    parameters: Vec::new(),
                    locals: Vec::new(),
                    operands,
                },
            },
        )
        .unwrap()
    }

    fn running(runtime: &Runtime) -> crate::engine::vm::execution::RunningExecution {
        crate::engine::vm::execution::RunningExecution::new(
            runtime,
            crate::engine::vm::execution::ExecutionLimits {
                frames: 8,
                slots: 64,
            },
        )
        .unwrap()
    }

    #[test]
    fn frame_execution_admits_once_and_lends_current_slots_without_rechecking() {
        use crate::engine::code::bytecode::Instruction;
        let runtime = Runtime::new();
        let context = runtime.new_context().unwrap();
        let mut execution = running(&runtime);
        let id = execution_frame(
            &runtime,
            &mut execution,
            context.realm,
            &[Instruction::ReturnUndefined],
            vec![JsValue::Int(7)],
        );
        #[cfg(feature = "profiling")]
        let profile = crate::engine::api::profiling::CostProfile::start();
        {
            let mut segment = FrameExecution::admit(&mut execution, id).unwrap();
            for value in [9, 11] {
                let mut turn = segment.frame();
                assert_eq!(turn.id, id);
                assert_eq!(*turn.property_generation, (value - 9) as u64 / 2);
                *turn.property_generation += 1;
                assert!(!turn.active_frame.is_materialized());
                assert_eq!(*turn.fault_pc, 0);
                assert!(turn.selected_named_read.is_none());
                assert_eq!(
                    turn.transaction.slots().pop().unwrap(),
                    JsValue::Int(value - 2)
                );
                turn.transaction.slots().push(JsValue::Int(value)).unwrap();
                assert!(turn.transaction.slots().push(JsValue::Int(0)).is_ok());
                // Safe capacity/occupancy checks remain inside the turn.
                assert!(turn.transaction.slots().push(JsValue::Int(0)).is_err());
                assert_eq!(turn.transaction.slots().pop().unwrap(), JsValue::Int(0));
            }
        }
        #[cfg(feature = "profiling")]
        {
            let events = profile.snapshot().owned_execution_events;
            assert_eq!(events.get("slot_authentication"), Some(&1));
            assert_eq!(events.get("core.pc_authentication"), Some(&1));
        }
    }

    #[test]
    fn frame_execution_rejects_foreign_store_without_consuming_owners() {
        use crate::engine::code::bytecode::Instruction;
        let runtime = Runtime::new();
        let context = runtime.new_context().unwrap();
        let mut execution = running(&runtime);
        let id = execution_frame(
            &runtime,
            &mut execution,
            context.realm,
            &[Instruction::ReturnUndefined],
            vec![JsValue::Int(7)],
        );
        let original = std::mem::replace(&mut execution.slots, SlotStore::new(64));
        let result = FrameExecution::admit(&mut execution, id);
        assert!(
            matches!(result, Err(ref error) if error.message() == "frame window is not the active arena window")
        );
        execution.slots = original;
        let mut segment = FrameExecution::admit(&mut execution, id).unwrap();
        assert_eq!(
            segment.frame().transaction.peek(0).unwrap(),
            &JsValue::Int(7)
        );
    }

    #[test]
    fn frame_execution_rejects_suspended_parent_and_wide_instruction_middle() {
        use crate::engine::code::bytecode::Instruction;
        let runtime = Runtime::new();
        let context = runtime.new_context().unwrap();
        let mut execution = running(&runtime);
        let parent = execution_frame(
            &runtime,
            &mut execution,
            context.realm,
            &[Instruction::ReturnUndefined],
            vec![],
        );
        let child = execution_frame(
            &runtime,
            &mut execution,
            context.realm,
            &[Instruction::PushI32(i32::MAX), Instruction::Return],
            vec![JsValue::Int(7)],
        );
        assert!(
            matches!(FrameExecution::admit(&mut execution, parent), Err(ref error)
            if error.message() == "frame identity is not the current execution frame")
        );
        execution.frames.current_mut(child).unwrap().resume_pc = 1;
        assert!(
            matches!(FrameExecution::admit(&mut execution, child), Err(ref error)
            if error.message() == "execution entry PC is not an instruction boundary")
        );
        execution.frames.current_mut(child).unwrap().resume_pc = 0;
        let mut segment = FrameExecution::admit(&mut execution, child).unwrap();
        assert_eq!(
            segment.frame().transaction.peek(0).unwrap(),
            &JsValue::Int(7)
        );
    }

    #[test]
    fn frame_execution_lends_installed_child_and_restored_parent() {
        use crate::engine::{
            code::bytecode::Instruction,
            vm::{
                driver::ordinary::{Entry, ReturnProgress},
                execute::FallthroughPc,
            },
        };
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        let child = runtime
            .into_jsvalue(context.eval("(function(){return 42})").unwrap())
            .unwrap();
        let JsValue::Object(child_id) = child else {
            panic!("bytecode function")
        };
        let mut execution = running(&runtime);
        let parent = execution_frame(
            &runtime,
            &mut execution,
            context.realm,
            &[Instruction::Call(0), Instruction::Return],
            vec![JsValue::Object(child_id)],
        );
        #[cfg(feature = "profiling")]
        let profile = crate::engine::api::profiling::CostProfile::start();
        let mut state = runtime.0.state.borrow_mut();
        {
            let mut segment = FrameExecution::admit(&mut execution, parent).unwrap();
            let fallthrough = {
                let turn = segment.frame();
                FallthroughPc::from_decoded(turn.executable.exec.decode_published(0).unwrap())
            };
            assert!(matches!(
                segment
                    .enter_ordinary(&runtime, &mut state, 0, false, false, fallthrough)
                    .unwrap(),
                Entry::Ordinary
            ));
            {
                let turn = segment.frame();
                assert_ne!(turn.id, parent);
                assert_eq!(*turn.resume_pc, 0);
                assert_eq!(turn.owners.function.object_id(), child_id);
                *turn.pending = Some(JsValue::Int(42));
            }
            assert!(matches!(
                segment.finish_ordinary(&runtime, &mut state).unwrap(),
                ReturnProgress::Returned
            ));
            let mut turn = segment.frame();
            assert_eq!(turn.id, parent);
            assert_eq!(*turn.resume_pc, fallthrough.index());
            assert_eq!(turn.transaction.slots().pop().unwrap(), JsValue::Int(42));
            assert!(turn.pending.is_none());
        }
        assert!(
            state.heap.object(child_id).is_err(),
            "retirement releases the callee's final owner"
        );
        #[cfg(feature = "profiling")]
        {
            let events = profile.snapshot().owned_execution_events;
            assert_eq!(events.get("slot_authentication"), Some(&1));
            assert_eq!(events.get("core.window_authentication"), Some(&1));
            assert_eq!(events.get("core.pc_authentication"), Some(&1));
            assert_eq!(events.get("ordinary_return_direct"), Some(&1));
        }
    }

    #[test]
    fn legacy_installer_rejects_changed_operand_witness_before_transfer() {
        use crate::engine::{
            code::bytecode::Instruction,
            vm::{call::ordinary::DirectSelection, execute::FallthroughPc},
        };
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        let child = runtime
            .into_jsvalue(context.eval("(function(){return 42})").unwrap())
            .unwrap();
        let JsValue::Object(child_id) = child else {
            panic!("bytecode function")
        };
        let mut execution = running(&runtime);
        let parent = execution_frame(
            &runtime,
            &mut execution,
            context.realm,
            &[Instruction::Call(0), Instruction::Return],
            vec![JsValue::Object(child_id)],
        );
        let mut state = runtime.0.state.borrow_mut();
        {
            let mut segment = FrameExecution::admit(&mut execution, parent).unwrap();
            let (checked, fallthrough) = {
                let mut turn = segment.frame();
                let checked = turn
                    .transaction
                    .validate_ordinary_call_operands(0, false)
                    .unwrap();
                let fallthrough =
                    FallthroughPc::from_decoded(turn.executable.exec.decode_published(0).unwrap());
                turn.transaction.slots().push(JsValue::Int(7)).unwrap();
                (checked, fallthrough)
            };
            let DirectSelection::Ordinary(selected) =
                DirectSelection::select_in_state(&runtime, &state, child_id).unwrap()
            else {
                panic!("ordinary selection")
            };
            let call = selected
                .authenticate_slot_in_state(&runtime, &state)
                .unwrap();
            assert!(
                matches!(segment.install_ordinary(&runtime, &mut state, call, checked, false, fallthrough),
                Err(ref error) if error.to_string().contains("operands changed"))
            );
            let mut turn = segment.frame();
            assert_eq!(turn.id, parent);
            assert_eq!(turn.transaction.slots().pop().unwrap(), JsValue::Int(7));
            assert_eq!(
                turn.transaction.peek(0).unwrap(),
                &JsValue::Object(child_id)
            );
            assert_eq!(state.heap.object_strong_count(child_id).unwrap(), 1);
        }
    }

    #[test]
    fn frame_execution_failed_install_preserves_current_frame_and_callee_owner() {
        use crate::engine::{code::bytecode::Instruction, vm::execute::FallthroughPc};
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        let child = runtime
            .into_jsvalue(
                context
                    .eval("(function(a){let b=1,c=2;return a+b+c})")
                    .unwrap(),
            )
            .unwrap();
        let JsValue::Object(child_id) = child else {
            panic!("bytecode function")
        };
        let mut execution = crate::engine::vm::execution::RunningExecution::new(
            &runtime,
            crate::engine::vm::execution::ExecutionLimits {
                frames: 8,
                slots: 2,
            },
        )
        .unwrap();
        let parent = execution_frame(
            &runtime,
            &mut execution,
            context.realm,
            &[Instruction::Call(1), Instruction::Return],
            vec![JsValue::Object(child_id), JsValue::Int(7)],
        );
        {
            let mut state = runtime.0.state.borrow_mut();
            let mut segment = FrameExecution::admit(&mut execution, parent).unwrap();
            let fallthrough = {
                let turn = segment.frame();
                FallthroughPc::from_decoded(turn.executable.exec.decode_published(0).unwrap())
            };
            assert!(
                matches!(segment.enter_ordinary(&runtime, &mut state, 1, false, false, fallthrough), Err(ref error)
                if error.message() == "execution slot limit exceeded")
            );
            let turn = segment.frame();
            assert_eq!(turn.id, parent);
            assert_eq!(*turn.resume_pc, 0);
            assert_eq!(turn.transaction.peek(0).unwrap(), &JsValue::Int(7));
            assert_eq!(
                turn.transaction.peek(1).unwrap(),
                &JsValue::Object(child_id)
            );
            assert_eq!(state.heap.object_strong_count(child_id).unwrap(), 1);
        }
        drop(execution);
        assert!(runtime.0.state.borrow().heap.object(child_id).is_err());
    }

    #[test]
    fn frame_transaction_keeps_owners_across_gc_and_partial_output_failure() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        let mut layout = PublishedFunctionSnapshot::empty_for_test(context.realm);
        layout.metadata.max_stack = 1;
        let mut store = SlotStore::new(8);
        let mut window = store
            .push_frame(
                &runtime,
                &layout.frame_layout(),
                FrameStorage {
                    original_arguments: vec![],
                    parameters: vec![],
                    locals: vec![],
                    operands: vec![],
                },
            )
            .unwrap();
        store
            .push(
                &mut window,
                runtime
                    .into_jsvalue(context.eval("({tag:42})").unwrap())
                    .unwrap(),
            )
            .unwrap();
        let mut owner;
        {
            let mut transaction = store.frame_transaction(&mut window).unwrap();
            owner = Some(transaction.slots().pop().unwrap());
            // No FrameSlots is live during collection. The moved owner roots
            // the object independently of the exclusive frame transaction.
            runtime.run_gc().unwrap();
            transaction.slots().push(JsValue::Int(7)).unwrap();
            assert!(transaction.slots().push_pending(&mut owner).is_err());
            assert!(
                owner.is_some(),
                "failed output keeps its owner outside the borrow"
            );
            assert_eq!(transaction.slots().pop().unwrap(), JsValue::Int(7));
            transaction.slots().push_pending(&mut owner).unwrap();
        }
        assert!(owner.is_none());
        let popped = store.pop(&mut window).unwrap();
        let Value::Object(object) = runtime.root_value(&popped).unwrap() else {
            panic!("object")
        };
        runtime.release_jsvalue(popped).unwrap();
        assert_eq!(
            context
                .get_property(&object, &runtime.intern_property_key("tag").unwrap())
                .unwrap(),
            Value::Int(42)
        );
    }

    #[test]
    fn primitive_transaction_pending_owners_survive_failed_commits() {
        let runtime = Runtime::new();
        let context = runtime.new_context().expect("create context");
        let mut owner = PublishedFunctionSnapshot::empty_for_test(context.realm);
        owner.metadata.max_stack = 1;
        let mut store = SlotStore::new(1);
        let mut window = store
            .push_frame(
                &runtime,
                &owner.frame_layout(),
                FrameStorage {
                    original_arguments: vec![],
                    parameters: vec![],
                    locals: vec![],
                    operands: vec![],
                },
            )
            .unwrap();
        store.push(&mut window, JsValue::Int(7)).unwrap();
        let object = runtime.new_object(None).unwrap();
        let object_id = object.object_id();
        let mut pending = Some(JsValue::Object(object.into_handle()));
        {
            let mut slots = store.borrow_frame_slots(&mut window).unwrap();
            assert!(slots.push_pending(&mut pending).is_err());
            assert!(
                pending.is_some(),
                "failure may not release the final owner inside FrameSlots"
            );
            assert_eq!(slots.peek(0).unwrap(), &JsValue::Int(7));
        }
        assert!(runtime.0.state.borrow().heap.object(object_id).is_ok());
        runtime.release_jsvalue(pending.take().unwrap()).unwrap();
        runtime.run_gc().unwrap();
        assert!(runtime.0.state.borrow().heap.object(object_id).is_err());
    }

    #[test]
    fn numeric_region_peak_capacity_rejects_before_any_operand_change() {
        let runtime = Runtime::new();
        let context = runtime.new_context().expect("create context");
        let mut layout = PublishedFunctionSnapshot::empty_for_test(context.realm);
        layout.metadata.max_stack = 2;
        layout.metadata.argument_count = 1;
        layout.metadata.local_count = 1;
        let definition = VariableDefinition {
            name: None,
            is_lexical: false,
            is_const: false,
            is_parameter_initializer: false,
            kind: ClosureVariableKind::Normal,
        };
        layout.argument_definitions = std::rc::Rc::from([definition]);
        layout.local_definitions = std::rc::Rc::from([definition]);
        let receiver = runtime.new_object(None).unwrap();
        let receiver_id = receiver.object_id();
        let mut store = SlotStore::new(4);
        let mut window = store
            .push_frame(
                &runtime,
                &layout.frame_layout(),
                FrameStorage {
                    original_arguments: vec![],
                    parameters: vec![FrameBinding::Direct(JsValue::Object(
                        receiver.into_handle(),
                    ))],
                    locals: vec![FrameBinding::Direct(JsValue::Int(7))],
                    operands: vec![],
                },
            )
            .unwrap();
        {
            let mut slots = store.borrow_frame_slots(&mut window).unwrap();
            assert!(!slots.has_operand_capacity(3));
            assert!(slots.has_operand_capacity(2));
            let region = PublishedNumericRegion {
                array: DirectSource::Argument(0),
                index: NumberSource::Immediate(0),
                value: NumberSource::Immediate(2),
                producer_index: None,
                shared_update_index: false,
                destination: 0,
                checked: false,
                comparison: crate::engine::code::exec_opcode::Opcode::Nop,
                when_true: false,
                fallthrough_pc: 0,
                peak: 3,
            };
            assert_eq!(
                crate::engine::vm::execute::numeric_local_array_region(
                    &mut slots,
                    &mut runtime.0.state.borrow_mut(),
                    &region,
                    true,
                ),
                Err(crate::engine::numeric_region_miss::NumericRegionMiss::OperandCapacity),
            );
        }
        assert_eq!(store.depth(&window), 0);
        assert!(matches!(
            store.local_current(&window, 0).unwrap(),
            FrameBinding::Direct(JsValue::Int(7))
        ));
        assert!(runtime.0.state.borrow().heap.object(receiver_id).is_ok());
        store.push(&mut window, JsValue::Int(7)).unwrap();
        store.push(&mut window, JsValue::Int(8)).unwrap();
        assert!(store.push(&mut window, JsValue::Int(9)).is_err());
        assert_eq!(store.pop(&mut window).unwrap(), JsValue::Int(8));
        assert_eq!(store.pop(&mut window).unwrap(), JsValue::Int(7));
        store.clear_frame(&runtime, window).unwrap();
    }
}
