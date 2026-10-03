//! One authenticated continuous execution borrow. No arena mutation API escapes.
use super::{Error, FrameBinding, FrameWindow, JsValue, Runtime, SlotStore};
use crate::engine::value::number::operations::Number;

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

    pub(in crate::engine::vm) fn property_ic_read(
        &mut self,
        runtime: &Runtime,
        executable: &crate::engine::code::runtime::PublishedFunctionSnapshot,
        operation: super::NamedReadOperation,
        native: &mut Option<crate::engine::object::LinkedNativeSelection>,
    ) -> Result<super::PropertyReadProgress, Error> {
        self.store
            .property_ic_read_current(self.window, runtime, executable, operation, native)
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

    pub(in crate::engine::vm) fn insert_copy(
        &mut self,
        runtime: &Runtime,
        source_from_top: usize,
        destination_from_top: usize,
    ) -> Result<(), Error> {
        self.store
            .insert_copy_current(runtime, self.window, source_from_top, destination_from_top)
    }

    pub(in crate::engine::vm) fn duplicate_operands(
        &mut self,
        runtime: &Runtime,
        count: usize,
    ) -> Result<(), Error> {
        self.store
            .duplicate_operands_current(runtime, self.window, count)
    }

    pub(in crate::engine::vm) fn array_kept_immediate_read(
        &mut self,
        runtime: &Runtime,
        keep_key: bool,
    ) -> Result<bool, Error> {
        let index = match self.peek(0)? {
            JsValue::Int(index) if *index >= 0 => *index as u32,
            JsValue::String(id) => {
                if !keep_key
                    && !matches!(
                        runtime.slot_value_release_readiness_jsvalue(self.peek(0)?),
                        Ok(crate::engine::heap::SlotReleaseReadiness::Ready)
                    )
                {
                    return Ok(false);
                }
                let text = runtime.0.state.borrow().heap.string_fast(*id).clone();
                let Some(index) = crate::engine::atom::AtomTable::canonical_array_index(&text)
                else {
                    return Ok(false);
                };
                index
            }
            _ => return Ok(false),
        };
        let Some(value) = runtime.try_dense_array_kept_read(self.peek(1)?, index) else {
            return Ok(false);
        };
        if !keep_key {
            runtime
                .release_jsvalue(self.pop()?)
                .map_err(crate::engine::vm::exception::runtime_error_to_vm_error)?;
        }
        self.push(value)?;
        Ok(true)
    }
    pub(in crate::engine::vm) fn array_immediate_read(
        &mut self,
        runtime: &Runtime,
    ) -> Result<bool, Error> {
        self.store
            .array_immediate_read_current(self.window, runtime)
    }

    /// A decline leaves both operands untouched for the property driver.
    /// Success cannot run cleanup, and consumes the same two owners as PutField.
    #[inline(always)]
    pub(in crate::engine::vm) fn try_scalar_field_write(
        &mut self,
        runtime: &Runtime,
        executable: &crate::engine::code::runtime::PublishedFunctionSnapshot,
        key_index: u32,
    ) -> Result<bool, Error> {
        if !runtime
            .try_linked_scalar_field_write(self.peek(1)?, self.peek(0)?, executable, key_index)
            .map_err(crate::engine::vm::exception::runtime_error_to_vm_error)?
        {
            return Ok(false);
        }
        let _scalar = self.pop()?;
        let base = self.pop()?;
        runtime
            .release_jsvalue(base)
            .map_err(crate::engine::vm::exception::runtime_error_to_vm_error)?;
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_execution_event(
            "ordinary_scalar_field_write_in_execute",
        );
        Ok(true)
    }

    /// Commit a scalar element write while the operands remain rooted. Both
    /// leaves prove that retiring the receiver cannot run heap cleanup; a
    /// decline leaves the three operands for the general property protocol.
    #[inline(always)]
    pub(in crate::engine::vm) fn try_scalar_element_write(
        &mut self,
        runtime: &Runtime,
    ) -> Result<bool, Error> {
        let JsValue::Int(index) = self.peek(1)? else {
            return Ok(false);
        };
        let Ok(index) = u32::try_from(*index) else {
            return Ok(false);
        };
        let base = self.peek(2)?;
        let value = self.peek(0)?;
        let dense = runtime
            .try_dense_array_write_scalar(base, index, value)
            .map_err(crate::engine::vm::exception::runtime_error_to_vm_error)?;
        let typed = if dense {
            false
        } else {
            match value {
                JsValue::Int(value) => {
                    runtime.try_typed_array_number_write(base, index, f64::from(*value))
                }
                JsValue::Float(value) => runtime.try_typed_array_number_write(base, index, *value),
                _ => false,
            }
        };
        if !dense && !typed {
            return Ok(false);
        }
        let value = self.pop()?;
        let key = self.pop()?;
        let base = self.pop()?;
        debug_assert!(matches!(
            value,
            JsValue::Undefined
                | JsValue::Null
                | JsValue::Bool(_)
                | JsValue::Int(_)
                | JsValue::Float(_)
                | JsValue::ShortBigInt(_)
        ));
        debug_assert!(matches!(key, JsValue::Int(_)));
        runtime
            .release_jsvalue(base)
            .map_err(crate::engine::vm::exception::runtime_error_to_vm_error)?;
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_execution_event(if typed {
            "typed_array_number_write_in_execute"
        } else {
            "dense_array_scalar_write_in_execute"
        });
        Ok(true)
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
        let mut context = runtime.new_context();
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
        let mut context = runtime.new_context();
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
        let mut context = runtime.new_context();
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
        let mut context = runtime.new_context();
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
        let context = runtime.new_context();
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
        let context = runtime.new_context();
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
                    &mut slots, &runtime, &region, true,
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
