//! One authenticated continuous execution borrow. No arena mutation API escapes.
use super::{Error, FrameBinding, FrameWindow, JsValue, Runtime, SlotStore};
use crate::engine::value::number::operations::Number;

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
        // The ordinary segment cannot consume a native activation. Preserve
        // its already selected facts for the legacy boundary's consumer.
        if self.execution.selected_native.is_some() {
            return Ok(crate::engine::vm::driver::ordinary::Entry::General);
        }
        let id = self
            .execution
            .frames
            .current_id()
            .expect("an admitted ordinary execution has a current frame");
        crate::engine::vm::driver::ordinary::enter_selected_in_state(
            runtime,
            state,
            self.execution,
            id,
            arguments,
            method,
            tail,
            None,
            fallthrough,
        )
    }

    pub(in crate::engine::vm) fn finish_ordinary(
        &mut self,
        runtime: &Runtime,
        state: &mut crate::engine::heap::runtime::RuntimeState,
    ) -> Result<crate::engine::vm::driver::ordinary::ReturnProgress, Error> {
        let id = self
            .execution
            .frames
            .current_id()
            .expect("an admitted ordinary execution has a current frame");
        crate::engine::vm::driver::ordinary::finish_in_state(runtime, state, self.execution, id)
    }
}

/// A frame-owned binding addressed without an operand-stack value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::engine::vm) enum DirectSlot {
    Local(u16),
    Argument(u16),
}

pub(in crate::engine::vm) enum LinkedReadCompletion {
    Completed,
    Declined,
    LookupError(Error),
    Pending(crate::engine::object::OrdinaryRead),
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
/// The install path checks window identity and depth again; those checks alone
/// do not certify unchanged slot contents after an arbitrary intervening write.
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
            drop(runtime.operation());
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

    /// A single-use owning-read transaction at a published driver boundary.
    /// The canonical lookup may retain roots/allocate, so no FrameSlots exists
    /// during lookup. It can only select a getter, never execute one. Exclusive
    /// store/window borrows prove that no slot or identity can change between
    /// authentication and the subsequent short output window.
    #[cfg(test)]
    pub(in crate::engine::vm) fn with_linked_own_read(
        &mut self,
        window: &mut FrameWindow,
        runtime: &Runtime,
        executable: &crate::engine::code::runtime::PublishedFunctionSnapshot,
        index: u32,
        complete: impl FnOnce(&mut FrameSlots<'_>, &mut Option<JsValue>) -> Result<(), Error>,
    ) -> Result<LinkedReadCompletion, Error> {
        self.with_linked_own_read_selected(window, runtime, executable, index, None, complete)
    }
    pub(in crate::engine::vm) fn with_linked_own_read_selected(
        &mut self,
        window: &mut FrameWindow,
        runtime: &Runtime,
        executable: &crate::engine::code::runtime::PublishedFunctionSnapshot,
        index: u32,
        native: Option<&mut Option<crate::engine::object::LinkedNativeSelection>>,
        complete: impl FnOnce(&mut FrameSlots<'_>, &mut Option<JsValue>) -> Result<(), Error>,
    ) -> Result<LinkedReadCompletion, Error> {
        use crate::engine::object::OrdinaryRead;
        self.check_current(window)?;
        let base = self.peek_current(window, 0)?;
        if executable
            .property_key_atoms
            .as_ref()
            .and_then(|atoms| atoms.get(index as usize))
            .is_none_or(|atom| atom.is_null())
        {
            return Err(Error::internal("property read has no linked key"));
        }
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_execution_event("linked_read_lookup_attempt");
        let selected =
            match runtime.prepare_linked_own_read_selected(base, executable, index, native) {
                Ok(selected) => selected,
                Err(error) => {
                    return Ok(LinkedReadCompletion::LookupError(
                        crate::engine::vm::exception::runtime_error_to_vm_error(error),
                    ));
                }
            };
        match selected {
            Some(OrdinaryRead::Complete(value)) => {
                #[cfg(feature = "profiling")]
                {
                    use crate::engine::heap::ObjectKind;
                    use crate::engine::heap::SlotReleaseReadiness;
                    if let JsValue::Object(id) = base
                        && let Ok(state) = runtime.0.state.try_borrow()
                        && let Ok(object) = state.heap.object(*id)
                    {
                        let kind = match object.kind {
                            ObjectKind::Array => "linked_read_completed.kind_array",
                            ObjectKind::NativeFunction
                            | ObjectKind::BoundFunction
                            | ObjectKind::BytecodeFunction => "linked_read_completed.kind_function",
                            ObjectKind::Map
                            | ObjectKind::Set
                            | ObjectKind::WeakMap
                            | ObjectKind::WeakSet
                            | ObjectKind::MapIterator
                            | ObjectKind::SetIterator => "linked_read_completed.kind_collection",
                            ObjectKind::Ordinary => "linked_read_completed.kind_ordinary",
                            _ => "linked_read_completed.kind_other",
                        };
                        crate::engine::api::profiling::record_owned_execution_event(kind);
                    }
                    let reason = if matches!(base, JsValue::Object(_)) {
                        match runtime.slot_value_release_readiness_jsvalue(base) {
                            Ok(SlotReleaseReadiness::Ready) => {
                                "linked_read_completed.receiver_ready"
                            }
                            Ok(SlotReleaseReadiness::QueueCapacity) => {
                                "linked_read_completed.receiver_queue_capacity"
                            }
                            Ok(SlotReleaseReadiness::Drain) => {
                                "linked_read_completed.receiver_drain"
                            }
                            Ok(SlotReleaseReadiness::Deferred) => {
                                "linked_read_completed.receiver_deferred"
                            }
                            Ok(SlotReleaseReadiness::Borrowed) => {
                                "linked_read_completed.receiver_borrowed"
                            }
                            Ok(SlotReleaseReadiness::PrimitiveStorage) => {
                                "linked_read_completed.receiver_primitive_storage"
                            }
                            Err(_) => "linked_read_completed.receiver_error",
                        }
                    } else {
                        "linked_read_completed.non_object_receiver"
                    };
                    crate::engine::api::profiling::record_owned_execution_event(reason);
                }
                // This owner stays outside the output window even on failure.
                let mut value = Some(value.unwrap_or(JsValue::Undefined));
                let result = {
                    let mut slots = FrameSlots {
                        store: self,
                        window,
                    };
                    #[cfg(feature = "profiling")]
                    crate::engine::api::profiling::record_owned_execution_event(
                        "linked_read_output_attempt",
                    );
                    complete(&mut slots, &mut value)
                };
                if let Some(value) = value {
                    runtime
                        .release_jsvalue(value)
                        .map_err(crate::engine::vm::exception::runtime_error_to_vm_error)?;
                }
                result?;
                #[cfg(feature = "profiling")]
                crate::engine::api::profiling::record_owned_execution_event(
                    "linked_read_output_completed",
                );
                Ok(LinkedReadCompletion::Completed)
            }
            Some(read) => {
                #[cfg(feature = "profiling")]
                crate::engine::api::profiling::record_owned_execution_event(
                    "linked_read_selected_pending",
                );
                Ok(LinkedReadCompletion::Pending(read))
            }
            None => {
                #[cfg(feature = "profiling")]
                crate::engine::api::profiling::record_owned_execution_event("linked_read_declined");
                Ok(LinkedReadCompletion::Declined)
            }
        }
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
        self.array_immediate_read_in_state(&mut runtime.0.state.borrow_mut(), &runtime.0.poisoned)
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
                    input: CallInput::new(runtime, JsValue::Undefined, JsValue::Undefined, None)
                        .into(),
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

    fn linked_executable(
        runtime: &Runtime,
        context: &mut crate::engine::api::Context,
    ) -> (PublishedFunctionSnapshot, u32) {
        let function = context.eval("(function(o){return o.x})").unwrap();
        let callable = runtime.callable_from_value(function).unwrap();
        let crate::engine::vm::call::CallableExecution::Bytecode { bytecode, .. } =
            runtime.bytecode_for_callable(&callable).unwrap()
        else {
            panic!("bytecode");
        };
        let executable = runtime.snapshot_function_bytecode(&bytecode).unwrap();
        let index = executable
            .exec
            .test_ir()
            .iter()
            .find_map(|op| match op {
                crate::engine::code::bytecode::Instruction::GetField(index) => Some(*index),
                _ => None,
            })
            .unwrap();
        (executable, index)
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
    fn linked_owning_read_authenticates_once_and_keeps_result_after_last_base_release() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        let (executable, index) = linked_executable(&runtime, &mut context);
        let mut store = SlotStore::new(8);
        let mut layout = PublishedFunctionSnapshot::empty_for_test(context.realm);
        layout.metadata.max_stack = 1;
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
        let base = context.eval("({x:{tag:42}})").unwrap();
        store
            .push(&mut window, runtime.into_jsvalue(base).unwrap())
            .unwrap();
        let mut old_base = None;
        #[cfg(feature = "profiling")]
        let profile = crate::engine::api::profiling::CostProfile::start();
        assert!(matches!(
            store
                .with_linked_own_read(&mut window, &runtime, &executable, index, |slots, value| {
                    old_base = Some(slots.pop()?);
                    slots.push_pending(value)
                })
                .unwrap(),
            LinkedReadCompletion::Completed
        ));
        #[cfg(feature = "profiling")]
        {
            assert_eq!(
                profile
                    .snapshot()
                    .owned_execution_events
                    .get("slot_authentication")
                    .copied(),
                Some(1)
            );
            drop(profile);
        }
        if let Some(value) = old_base.take() {
            runtime.release_jsvalue(value).unwrap();
        }
        runtime.run_gc().unwrap();
        let popped = store.pop(&mut window).unwrap();
        let Value::Object(result) = runtime.root_value(&popped).unwrap() else {
            panic!("result");
        };
        runtime.release_jsvalue(popped).unwrap();
        assert_eq!(
            context
                .get_property(&result, &runtime.intern_property_key("tag").unwrap())
                .unwrap(),
            Value::Int(42)
        );
    }

    #[test]
    fn linked_owning_read_separates_slot_errors_from_lookup_errors_without_consumption() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        let (executable, index) = linked_executable(&runtime, &mut context);
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
        let result =
            store.with_linked_own_read(&mut window, &runtime, &executable, u32::MAX, |_, _| {
                panic!("invalid input committed")
            });
        assert!(
            matches!(result, Err(ref error) if error.message()=="owned operand stack underflow")
        );
        // A reclaimed base handle forces the lookup path to report a lookup
        // error instead of committing the operand.
        let blocked = runtime.new_object(None).unwrap();
        let stale_handle = blocked.into_handle();
        runtime
            .release_jsvalue(JsValue::Object(stale_handle))
            .unwrap();
        runtime.run_gc().unwrap();
        store
            .push(&mut window, JsValue::Object(stale_handle))
            .unwrap();
        assert!(matches!(
            store
                .with_linked_own_read(&mut window, &runtime, &executable, index, |_, _| panic!(
                    "stale input committed"
                ))
                .unwrap(),
            LinkedReadCompletion::LookupError(_)
        ));
        assert_eq!(store.depth(&window), 1);
    }

    #[test]
    fn linked_owning_read_pending_getter_is_selected_once_without_consuming_base() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        let (executable, index) = linked_executable(&runtime, &mut context);
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
        let base = context.eval("globalThis.linkedCalls=0;globalThis.linkedBase={get x(){linkedCalls++;return 42}};linkedBase").unwrap();
        store
            .push(&mut window, runtime.into_jsvalue(base).unwrap())
            .unwrap();
        let LinkedReadCompletion::Pending(read) = store
            .with_linked_own_read(&mut window, &runtime, &executable, index, |_, _| {
                panic!("pending getter committed operands")
            })
            .unwrap()
        else {
            panic!("pending getter");
        };
        assert_eq!(store.depth(&window), 1);
        drop(
            context
                .eval("Object.defineProperty(linkedBase,'x',{get(){throw 99}})")
                .unwrap(),
        );
        let result = runtime
            .finish_prepared_read(
                context.realm,
                &runtime.intern_property_key("x").unwrap(),
                read,
            )
            .unwrap();
        assert!(matches!(
            result,
            crate::engine::value::conversion::NativeConversion::Value(Some(Value::Int(42)))
        ));
        assert_eq!(context.eval("linkedCalls").unwrap(), Value::Int(1));
        let base = store.pop(&mut window).unwrap();
        runtime.release_jsvalue(base).unwrap();
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
