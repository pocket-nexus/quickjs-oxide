//! One authenticated continuous execution borrow. No arena mutation API escapes.
use super::{Error, FrameBinding, FrameWindow, JsValue, Runtime, SlotStore, StoreMode};

pub(in crate::engine::vm) enum LinkedReadCompletion {
    Completed,
    Declined,
    LookupError(Error),
    Pending(crate::engine::object::OrdinaryRead),
}

/// Exclusive ownership of one authenticated frame window across short execution
/// borrows. The store and window cannot be pushed, popped or replaced while this
/// transaction exists. Allocation/release may happen between `slots()` borrows;
/// ordinary completion carries no slot reference then. The dedicated primitive
/// local callback may borrow primitive locals while allocating and commit a
/// uniquely owned String append after successful reservation. It cannot expose
/// references, execute JS, mutate other execution state or bypass domain checks.
pub(in crate::engine::vm) struct FrameTransaction<'a> {
    store: &'a mut SlotStore,
    window: &'a mut FrameWindow,
    #[cfg(any(test, oxide_scalar_tos, oxide_owned_tos))]
    tos: super::tos::ScalarTos,
    // Admission is fixed for the transaction, independently of the mutable
    // cached value. An outer Option<ScalarTos> shares the value's enum niche,
    // forcing each short borrow to reload that tag after cache mutations.
    #[cfg(any(test, oxide_scalar_tos, oxide_owned_tos))]
    tos_enabled: bool,
}
impl FrameTransaction<'_> {
    /// Dedicated admission for a completed numeric String/heap-BigInt output.
    /// Ordinary push remains scalar-only. A canonical transaction declines;
    /// decline/error preserve the pending owner for the caller's normal path.
    #[cfg(any(test, oxide_owned_tos))]
    pub(in crate::engine::vm) fn cache_numeric_output(
        &mut self,
        value: &mut Option<JsValue>,
    ) -> Result<bool, Error> {
        if !self.tos_enabled {
            return Ok(false);
        }
        self.store
            .tos_cache_numeric_output(self.window, &mut self.tos, value)
    }

    #[cfg(any(test, oxide_owned_tos))]
    pub(in crate::engine::vm) fn has_owned_numeric_output(&self) -> bool {
        self.tos_enabled && self.tos.has_owned_numeric_output()
    }

    #[inline]
    pub(in crate::engine::vm) fn canonicalize(&mut self, reason: &'static str) {
        #[cfg(any(test, oxide_scalar_tos, oxide_owned_tos))]
        if self.tos_enabled {
            self.tos.canonicalize(self.store, self.window, reason);
        }
        #[cfg(not(any(test, oxide_scalar_tos, oxide_owned_tos)))]
        let _ = reason;
    }

    /// A helper may push scalars and then release owners before returning.
    /// Keep that entire short borrow canonical, not just its entry.
    #[inline(always)]
    pub(in crate::engine::vm) fn canonical_slots(&mut self, reason: &'static str) -> RunSlots<'_> {
        self.canonicalize(reason);
        RunSlots {
            store: self.store,
            window: self.window,
            #[cfg(any(test, oxide_scalar_tos, oxide_owned_tos))]
            tos: None,
        }
    }

    #[inline(always)]
    pub(in crate::engine::vm) fn peek(&self, offset: usize) -> Result<&JsValue, Error> {
        #[cfg(any(test, oxide_scalar_tos, oxide_owned_tos))]
        if self.tos_enabled {
            return self.tos.peek(self.store, self.window, offset);
        }
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
        // Authentication is read-only and may coexist with a borrowed callable.
        // Read the logical stack, including a cached top, without moving owners.
        let _ = runtime;
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
        Ok(true)
    }
    pub(in crate::engine::vm) fn take_native_call_operands(
        &mut self,
        runtime: &Runtime,
        logical_active_depth: usize,
        count: usize,
        method: bool,
    ) -> Result<(Vec<JsValue>, JsValue), Error> {
        self.canonicalize("tos.spill.call");
        self.store
            .reserve_native_argument_depth(logical_active_depth.saturating_add(1))?;
        self.store
            .take_native_call_operands_current(runtime, self.window, count, method)
    }

    /// The callback may allocate primitive storage and commit a unique String
    /// append after reservation. It cannot execute JS or let input references
    /// escape. Aliasing locals get a temporary RHS owner, preventing mutation of
    /// the value that must also remain the original RHS.
    pub(in crate::engine::vm) fn with_local_add_inputs<T>(
        &mut self,
        left: u16,
        right: u16,
        consume: impl FnOnce(&mut JsValue, &JsValue) -> T,
    ) -> Result<Option<T>, Error> {
        self.canonicalize("tos.spill.local_add");
        let (left, right) = (usize::from(left), usize::from(right));
        let locals = &mut self.store.slots[self.window.locals()];
        if left >= locals.len() || right >= locals.len() {
            return Err(Error::internal("owned local index is out of bounds"));
        }
        if left == right {
            let binding = locals[left]
                .as_mut()
                .ok_or_else(|| Error::internal("owned local is vacant"))?;
            let FrameBinding::Direct(value) = binding else {
                return Ok(None);
            };
            if !local_add_values(value, value) {
                return Ok(None);
            }
            // Aliased locals cannot expose `&mut` and `&` views of the same
            // owner to the append callback at once; decline to the canonical
            // path until the fused append accepts a single-view callback.
            let _ = consume;
            return Ok(None);
        }
        let (left, right) = if left < right {
            let (before, after) = locals.split_at_mut(right);
            (&mut before[left], &after[0])
        } else {
            let (before, after) = locals.split_at_mut(left);
            (&mut after[0], &before[right])
        };
        let left = left
            .as_mut()
            .ok_or_else(|| Error::internal("owned local is vacant"))?;
        let FrameBinding::Direct(left) = left else {
            return Ok(None);
        };
        let right = right
            .as_ref()
            .ok_or_else(|| Error::internal("owned local is vacant"))?;
        let FrameBinding::Direct(right) = right else {
            return Ok(None);
        };
        if !local_add_values(left, right) {
            return Ok(None);
        }
        Ok(Some(consume(left, right)))
    }
    pub(in crate::engine::vm) fn with_local_add_constant<T>(
        &mut self,
        left: u16,
        right: &JsValue,
        consume: impl FnOnce(&mut JsValue, &JsValue) -> T,
    ) -> Result<Option<T>, Error> {
        self.canonicalize("tos.spill.local_add");
        let local = self.store.slots[self.window.locals()]
            .get_mut(usize::from(left))
            .ok_or_else(|| Error::internal("owned local index is out of bounds"))?
            .as_mut()
            .ok_or_else(|| Error::internal("owned local is vacant"))?;
        let FrameBinding::Direct(left) = local else {
            return Ok(None);
        };
        if !local_add_values(left, right) {
            return Ok(None);
        }
        Ok(Some(consume(left, right)))
    }
    /// Prepend `C + R`: the constant is the mutable left operand and the local
    /// is borrowed immutably for the allocation. Concatenation order is fixed
    /// by the caller; the shared constant buffer is never appended into.
    pub(in crate::engine::vm) fn with_local_add_constant_left<T>(
        &mut self,
        local: u16,
        constant: &mut JsValue,
        consume: impl FnOnce(&mut JsValue, &JsValue) -> T,
    ) -> Result<Option<T>, Error> {
        self.canonicalize("tos.spill.local_add");
        let local = self.store.slots[self.window.locals()]
            .get(usize::from(local))
            .ok_or_else(|| Error::internal("owned local index is out of bounds"))?
            .as_ref()
            .ok_or_else(|| Error::internal("owned local is vacant"))?;
        let FrameBinding::Direct(local) = local else {
            return Ok(None);
        };
        if !local_add_values(constant, local) {
            return Ok(None);
        }
        Ok(Some(consume(constant, local)))
    }
    /// Only for a transaction made by `frame_transaction_with_scalar_tos`.
    /// The run loop proves this with the same const condition as construction;
    /// other callers use `slots` or `canonical_slots` to retain mode checks.
    #[cfg(any(test, oxide_scalar_tos, oxide_owned_tos))]
    #[inline(always)]
    pub(in crate::engine::vm) fn cache_slots(&mut self) -> RunSlots<'_> {
        debug_assert!(self.tos_enabled);
        RunSlots {
            store: self.store,
            window: self.window,
            tos: Some(&mut self.tos),
        }
    }

    #[inline(always)]
    pub(in crate::engine::vm) fn slots(&mut self) -> RunSlots<'_> {
        RunSlots {
            store: self.store,
            window: self.window,
            #[cfg(any(test, oxide_scalar_tos, oxide_owned_tos))]
            tos: self.tos_enabled.then_some(&mut self.tos),
        }
    }
}

#[cfg(any(test, oxide_scalar_tos, oxide_owned_tos))]
impl Drop for FrameTransaction<'_> {
    fn drop(&mut self) {
        if self.tos_enabled {
            self.tos.restore(self.store);
        }
    }
}

impl SlotStore {
    pub(in crate::engine::vm) fn frame_transaction<'a>(
        &'a mut self,
        window: &'a mut FrameWindow,
    ) -> Result<FrameTransaction<'a>, Error> {
        self.check_current(window)?;
        Ok(FrameTransaction {
            store: self,
            window,
            #[cfg(any(test, oxide_scalar_tos, oxide_owned_tos))]
            tos: super::tos::ScalarTos::new(),
            #[cfg(any(test, oxide_scalar_tos, oxide_owned_tos))]
            tos_enabled: false,
        })
    }

    #[cfg(any(test, oxide_scalar_tos, oxide_owned_tos))]
    pub(in crate::engine::vm) fn frame_transaction_with_scalar_tos<'a>(
        &'a mut self,
        window: &'a mut FrameWindow,
    ) -> Result<FrameTransaction<'a>, Error> {
        self.check_current(window)?;
        Ok(FrameTransaction {
            store: self,
            window,
            tos: super::tos::ScalarTos::new(),
            tos_enabled: true,
        })
    }

    /// A single-use owning-read transaction at a published driver boundary.
    /// The canonical lookup may retain roots/allocate, so no RunSlots exists
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
        complete: impl FnOnce(&mut RunSlots<'_>, &mut Option<JsValue>) -> Result<(), Error>,
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
        complete: impl FnOnce(&mut RunSlots<'_>, &mut Option<JsValue>) -> Result<(), Error>,
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
                // This owner stays outside the output window even on failure.
                let mut value = Some(value.unwrap_or(JsValue::Undefined));
                {
                    let mut slots = RunSlots {
                        store: self,
                        window,
                        #[cfg(any(test, oxide_scalar_tos, oxide_owned_tos))]
                        tos: None,
                    };
                    #[cfg(feature = "profiling")]
                    crate::engine::api::profiling::record_owned_execution_event(
                        "linked_read_output_attempt",
                    );
                    complete(&mut slots, &mut value)?;
                }
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

pub(in crate::engine::vm) struct RunSlots<'a> {
    pub(super) store: &'a mut SlotStore,
    pub(super) window: &'a mut FrameWindow,
    #[cfg(any(test, oxide_scalar_tos, oxide_owned_tos))]
    pub(super) tos: Option<&'a mut super::tos::ScalarTos>,
}
impl RunSlots<'_> {
    #[cfg(test)]
    pub(in crate::engine::vm) fn has_owned_numeric_output(&self) -> bool {
        self.tos
            .as_deref()
            .is_some_and(|tos| tos.has_owned_numeric_output())
    }

    #[inline]
    pub(in crate::engine::vm) fn canonicalize(&mut self, reason: &'static str) {
        #[cfg(any(test, oxide_scalar_tos, oxide_owned_tos))]
        if let Some(tos) = self.tos.as_deref_mut() {
            tos.canonicalize(self.store, self.window, reason);
        }
        #[cfg(not(any(test, oxide_scalar_tos, oxide_owned_tos)))]
        let _ = reason;
    }

    pub(in crate::engine::vm) fn property_ic_read(
        &mut self,
        runtime: &Runtime,
        executable: &crate::engine::code::runtime::PublishedFunctionSnapshot,
        pc: usize,
        key_index: u32,
        keep_receiver: bool,
        native: &mut Option<crate::engine::object::LinkedNativeSelection>,
    ) -> Result<bool, Error> {
        self.canonicalize("tos.spill.property");
        self.store.property_ic_read_current(
            self.window,
            runtime,
            executable,
            pc,
            key_index,
            keep_receiver,
            native,
        )
    }

    pub(in crate::engine::vm) fn has_operand_capacity(&self, extra: usize) -> bool {
        self.window
            .depth
            .checked_add(extra)
            .is_some_and(|depth| depth <= self.window.end - self.window.locals_end)
    }

    #[cfg(feature = "profiling")]
    pub(in crate::engine::vm) fn depth(&self) -> usize {
        self.window.depth
    }
    #[inline(always)]
    pub(in crate::engine::vm) fn peek(&self, from_top: usize) -> Result<&JsValue, Error> {
        #[cfg(any(test, oxide_scalar_tos, oxide_owned_tos))]
        if let Some(tos) = self.tos.as_deref() {
            return tos.peek(self.store, self.window, from_top);
        }
        self.store.peek_current(self.window, from_top)
    }

    #[inline(always)]
    pub(in crate::engine::vm) fn push(&mut self, value: JsValue) -> Result<(), Error> {
        #[cfg(any(test, oxide_scalar_tos, oxide_owned_tos))]
        if let Some(tos) = self.tos.as_deref_mut() {
            return self.store.tos_push(self.window, tos, value);
        }
        self.store.push_current(self.window, value)
    }

    /// On failure the caller keeps its owner until this borrow has ended.
    pub(in crate::engine::vm) fn push_pending(
        &mut self,
        value: &mut Option<JsValue>,
    ) -> Result<(), Error> {
        #[cfg(any(test, oxide_scalar_tos, oxide_owned_tos))]
        if let Some(tos) = self.tos.as_deref_mut() {
            return self.store.tos_push_pending(self.window, tos, value);
        }
        self.store.push_pending_current(self.window, value)
    }

    pub(in crate::engine::vm) fn replace_local_pending(
        &mut self,
        index: u16,
        value: &mut Option<FrameBinding>,
    ) -> Result<FrameBinding, Error> {
        self.store
            .replace_local_pending_current(self.window, index, value)
    }

    // Preserve the direct SlotStore call at numeric operand consumers.
    #[inline(always)]
    pub(in crate::engine::vm) fn pop(&mut self) -> Result<JsValue, Error> {
        #[cfg(any(test, oxide_scalar_tos, oxide_owned_tos))]
        if let Some(tos) = self.tos.as_deref_mut() {
            return self.store.tos_pop(self.window, tos);
        }
        self.store.pop_current(self.window)
    }

    pub(in crate::engine::vm) fn local_add_constant_supported(
        &self,
        runtime: &Runtime,
        left: u16,
    ) -> Result<bool, Error> {
        if self
            .window
            .depth
            .checked_add(2)
            .is_none_or(|depth| depth > self.window.end - self.window.locals_end)
        {
            return Ok(false);
        }
        let FrameBinding::Direct(left) = self.local(left)? else {
            return Ok(false);
        };
        let _ = runtime;
        Ok(!matches!(left, JsValue::Object(_)))
    }
    pub(in crate::engine::vm) fn local_add_supported(
        &self,
        runtime: &Runtime,
        left: u16,
        right: u16,
    ) -> Result<bool, Error> {
        if self
            .window
            .depth
            .checked_add(2)
            .is_none_or(|depth| depth > self.window.end - self.window.locals_end)
        {
            return Ok(false);
        }
        // Aliased locals cannot expose `&mut` and `&` views of the same owner
        // to the fused append; the canonical local-add sequence handles them.
        if left == right {
            return Ok(false);
        }
        let FrameBinding::Direct(left) = self.local(left)? else {
            return Ok(false);
        };
        // This guard runs at the first GetLocal. A malformed later index must
        // be diagnosed by its own instruction after the left copy is pushed.
        let Ok(FrameBinding::Direct(right)) = self.local(right) else {
            return Ok(false);
        };
        let _ = runtime;
        Ok(local_add_values(left, right))
    }

    #[inline]
    pub(in crate::engine::vm) fn local(&self, index: u16) -> Result<&FrameBinding, Error> {
        self.store.local_current(self.window, index)
    }

    #[inline]
    pub(in crate::engine::vm) fn parameter(&self, index: u16) -> Result<&FrameBinding, Error> {
        self.store.parameter_current(self.window, index)
    }

    pub(in crate::engine::vm) fn replace_local(
        &mut self,
        index: u16,
        value: FrameBinding,
    ) -> Result<FrameBinding, Error> {
        self.store.replace_local_current(self.window, index, value)
    }

    /// A non-direct target declines without consuming or retaining the top.
    /// The caller must establish the binding and displaced-release semantics.
    /// Success transfers the displaced Direct payload to the caller.
    #[inline(always)]
    pub(in crate::engine::vm) fn store_local_from_top(
        &mut self,
        runtime: &Runtime,
        index: u16,
        mode: StoreMode,
    ) -> Result<Option<JsValue>, Error> {
        #[cfg(any(test, oxide_scalar_tos, oxide_owned_tos))]
        if let Some(tos) = self.tos.as_deref_mut() {
            return self
                .store
                .tos_store_local(self.window, tos, runtime, index, mode);
        }
        self.store
            .store_local_from_top_current(self.window, runtime, index, mode)
    }

    #[inline(always)]
    pub(in crate::engine::vm) fn store_parameter_from_top(
        &mut self,
        runtime: &Runtime,
        index: u16,
        mode: StoreMode,
    ) -> Result<Option<JsValue>, Error> {
        #[cfg(any(test, oxide_scalar_tos, oxide_owned_tos))]
        if let Some(tos) = self.tos.as_deref_mut() {
            return self
                .store
                .tos_store_parameter(self.window, tos, runtime, index, mode);
        }
        self.store
            .store_parameter_from_top_current(self.window, runtime, index, mode)
    }

    pub(in crate::engine::vm) fn rotate_operands(
        &mut self,
        skip_top: usize,
        count: usize,
        left: bool,
    ) -> Result<(), Error> {
        self.canonicalize("tos.spill.shuffle");
        self.store
            .rotate_operands_current(self.window, skip_top, count, left)
    }

    pub(in crate::engine::vm) fn insert_copy(
        &mut self,
        runtime: &Runtime,
        source_from_top: usize,
        destination_from_top: usize,
    ) -> Result<(), Error> {
        self.canonicalize("tos.spill.shuffle");
        self.store
            .insert_copy_current(runtime, self.window, source_from_top, destination_from_top)
    }

    pub(in crate::engine::vm) fn duplicate_operands(
        &mut self,
        runtime: &Runtime,
        count: usize,
    ) -> Result<(), Error> {
        self.canonicalize("tos.spill.shuffle");
        self.store
            .duplicate_operands_current(runtime, self.window, count)
    }

    pub(in crate::engine::vm) fn release_operand(
        &mut self,
        from_top: usize,
        runtime: &Runtime,
    ) -> Result<bool, Error> {
        self.canonicalize("tos.spill.release");
        self.store
            .release_operand_current(self.window, from_top, runtime)
    }

    pub(in crate::engine::vm) fn typed_array_number_write(
        &mut self,
        runtime: &Runtime,
    ) -> Result<bool, Error> {
        self.canonicalize("tos.spill.property");
        self.store
            .typed_array_number_write_current(self.window, runtime)
    }

    pub(in crate::engine::vm) fn array_kept_immediate_read(
        &mut self,
        runtime: &Runtime,
        keep_key: bool,
    ) -> Result<bool, Error> {
        self.canonicalize("tos.spill.property");
        // Keep the whole operation canonical, including the output push and
        // any following release. Calling the cache-aware push would reopen a hole.
        let index = match self.store.peek_current(self.window, 0)? {
            JsValue::Int(index) if *index >= 0 => *index as u32,
            JsValue::String(id) => {
                if !keep_key
                    && !matches!(
                        runtime.slot_value_release_readiness_jsvalue(
                            self.store.peek_current(self.window, 0)?
                        ),
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
        let Some(value) =
            runtime.try_dense_array_kept_read(self.store.peek_current(self.window, 1)?, index)
        else {
            return Ok(false);
        };
        if !keep_key {
            runtime
                .release_jsvalue(self.store.pop_current(self.window)?)
                .map_err(crate::engine::vm::exception::runtime_error_to_vm_error)?;
        }
        self.store.push_current(self.window, value)?;
        Ok(true)
    }
    pub(in crate::engine::vm) fn property_ic_write_scalar(
        &mut self,
        runtime: &Runtime,
        executable: &crate::engine::code::runtime::PublishedFunctionSnapshot,
        pc: usize,
        key: u32,
    ) -> Result<bool, Error> {
        self.canonicalize("tos.spill.property");
        self.store
            .property_ic_write_scalar_current(self.window, runtime, executable, pc, key)
    }

    pub(in crate::engine::vm) fn array_immediate_read(
        &mut self,
        runtime: &Runtime,
    ) -> Result<bool, Error> {
        self.canonicalize("tos.spill.property");
        self.store
            .array_immediate_read_current(self.window, runtime)
    }

    pub(in crate::engine::vm) fn ordinary_field_immediate_read(
        &mut self,
        runtime: &Runtime,
        executable: &crate::engine::code::runtime::PublishedFunctionSnapshot,
        key_index: u32,
    ) -> Result<bool, Error> {
        self.canonicalize("tos.spill.property");
        self.store.ordinary_field_immediate_read_current(
            self.window,
            runtime,
            executable,
            key_index,
        )
    }

    #[inline(always)]
    pub(in crate::engine::vm) fn binary_number(
        &mut self,
        operation: impl FnOnce(
            crate::engine::value::number::operations::Number,
            crate::engine::value::number::operations::Number,
        ) -> JsValue,
    ) -> Result<bool, Error> {
        #[cfg(any(test, oxide_scalar_tos, oxide_owned_tos))]
        if let Some(tos) = self.tos.as_deref_mut() {
            return self.store.tos_binary_number(self.window, tos, operation);
        }
        self.store.binary_number_current(self.window, operation)
    }
    #[inline(always)]
    pub(in crate::engine::vm) fn consume_number_pair(
        &mut self,
        operation: impl FnOnce(
            crate::engine::value::number::operations::Number,
            crate::engine::value::number::operations::Number,
        ) -> bool,
    ) -> Result<Option<bool>, Error> {
        #[cfg(any(test, oxide_scalar_tos, oxide_owned_tos))]
        if let Some(tos) = self.tos.as_deref_mut() {
            return self
                .store
                .tos_consume_number_pair(self.window, tos, operation);
        }
        self.store
            .consume_number_pair_current(self.window, operation)
    }

    #[inline(always)]
    pub(in crate::engine::vm) fn update_number_local(
        &mut self,
        index: u16,
        operation: impl FnOnce(
            crate::engine::value::number::operations::Number,
        ) -> (
            crate::engine::value::number::operations::Number,
            Option<crate::engine::value::number::operations::Number>,
        ),
    ) -> Result<bool, Error> {
        #[cfg(any(test, oxide_scalar_tos, oxide_owned_tos))]
        if let Some(tos) = self.tos.as_deref_mut() {
            return self
                .store
                .tos_update_number_local(self.window, tos, index, operation);
        }
        self.store
            .update_number_local_current(self.window, index, operation)
    }
}

fn local_add_values(left: &JsValue, right: &JsValue) -> bool {
    !matches!(left, JsValue::Object(_))
        && !matches!(right, JsValue::Object(_))
        && (matches!(
            left,
            JsValue::String(_) | JsValue::BigInt(_) | JsValue::ShortBigInt(_)
        ) || matches!(
            right,
            JsValue::String(_) | JsValue::BigInt(_) | JsValue::ShortBigInt(_)
        ))
}

#[cfg(test)]
mod primitive_transaction_tests {
    use super::*;
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
            .code
            .iter()
            .find_map(|op| match op {
                crate::engine::code::bytecode::Instruction::GetField(index) => Some(*index),
                _ => None,
            })
            .unwrap();
        (executable, index)
    }

    #[test]
    fn local_add_declines_bad_rhs_before_the_canonical_left_copy() {
        use crate::engine::code::function::metadata::{ClosureVariableKind, VariableDefinition};
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let left = context.eval("'left'").unwrap();
        let mut layout = PublishedFunctionSnapshot::empty_for_test(context.realm);
        layout.metadata.local_count = 1;
        layout.metadata.max_stack = 2;
        layout.local_definitions = std::rc::Rc::from([VariableDefinition {
            name: None,
            is_lexical: false,
            is_const: false,
            is_parameter_initializer: false,
            kind: ClosureVariableKind::Normal,
        }]);
        let mut store = SlotStore::new(8);
        let mut window = store
            .push_frame(
                &runtime,
                &layout.frame_layout(),
                FrameStorage {
                    original_arguments: vec![],
                    parameters: vec![],
                    locals: vec![FrameBinding::Direct(
                        runtime.into_jsvalue(left.clone()).unwrap(),
                    )],
                    operands: vec![],
                },
            )
            .unwrap();
        {
            let mut slots = store.run_window(&mut window).unwrap();
            assert!(!slots.local_add_supported(&runtime, 0, u16::MAX).unwrap());
            assert_eq!(slots.window.depth, 0);
            let FrameBinding::Direct(value) = slots.local(0).unwrap() else {
                panic!("left")
            };
            let value = runtime.dup_jsvalue(value).unwrap();
            slots.push(value).unwrap();
            assert!(slots.local(u16::MAX).is_err());
            assert_eq!(slots.window.depth, 1);
            assert_eq!(runtime.root_value(slots.peek(0).unwrap()).unwrap(), left);
        }
        let storage = store.take_frame(&runtime, window).unwrap();
        crate::engine::vm::stack::release_frame_storage(&runtime, storage);
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
            // No RunSlots is live during collection. The moved owner roots
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
            let mut slots = store.run_window(&mut window).unwrap();
            assert!(slots.push_pending(&mut pending).is_err());
            assert!(
                pending.is_some(),
                "failure may not release the final owner inside RunSlots"
            );
            assert_eq!(slots.peek(0).unwrap(), &JsValue::Int(7));
        }
        assert!(runtime.0.state.borrow().heap.object(object_id).is_ok());
        let mut binding = Some(FrameBinding::Direct(pending.take().unwrap()));
        {
            let mut slots = store.run_window(&mut window).unwrap();
            assert!(slots.replace_local_pending(0, &mut binding).is_err());
            assert!(binding.is_some());
        }
        if let Some(FrameBinding::Direct(value)) = binding.take() {
            runtime.release_jsvalue(value).unwrap();
        }
        runtime.run_gc().unwrap();
        assert!(runtime.0.state.borrow().heap.object(object_id).is_err());
    }
}
