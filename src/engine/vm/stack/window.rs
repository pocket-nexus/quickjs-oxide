//! One authenticated continuous execution borrow. No arena mutation API escapes.
use super::{Error, FrameBinding, FrameWindow, JsValue, Runtime, SlotStore};
use crate::engine::code::fusion::DirectSlot;
use crate::engine::heap::ObjectId;
use crate::engine::value::number::operations::Number;

#[derive(Clone, Copy, Debug)]
pub(in crate::engine::vm) enum NumericDestination {
    Push,
    #[allow(dead_code)] // Reserved for the authenticated accumulator spans.
    Local(u16),
}

#[derive(Clone, Copy, Debug)]
pub(in crate::engine::vm) struct NumberUpdate {
    pub slot: DirectSlot,
    pub value: Number,
}

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
    pub(in crate::engine::vm) fn take_native_call_operands(
        &mut self,
        runtime: &Runtime,
        logical_active_depth: usize,
        count: usize,
        method: bool,
    ) -> Result<(Vec<JsValue>, JsValue), Error> {
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
    pub(in crate::engine::vm) fn slots(&mut self) -> RunSlots<'_> {
        RunSlots {
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
        Ok(FrameTransaction {
            store: self,
            window,
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
}
impl RunSlots<'_> {
    /// Borrow a direct binding only for this execution borrow. No owner is
    /// created, and the returned reference cannot outlive a subsequent commit.
    #[inline]
    #[allow(clippy::needless_lifetimes)] // Spell out the short borrow, not the frame lifetime.
    pub(in crate::engine::vm) fn direct_value<'borrow>(
        &'borrow self,
        source: DirectSlot,
    ) -> Option<&'borrow JsValue> {
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

    #[inline]
    #[allow(dead_code)] // Store spans use this before their final heap commit.
    pub(in crate::engine::vm) fn numeric_span_room(&self, extra_peak: u8) -> bool {
        self.store
            .numeric_span_room_current(self.window, extra_peak)
    }

    #[inline]
    pub(in crate::engine::vm) fn try_commit_number(
        &mut self,
        destination: NumericDestination,
        result: Number,
        update: Option<NumberUpdate>,
        extra_peak: u8,
    ) -> bool {
        self.store
            .try_commit_number_current(self.window, destination, result, update, extra_peak)
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

    pub(in crate::engine::vm) fn replace_local_pending(
        &mut self,
        index: u16,
        value: &mut Option<FrameBinding>,
    ) -> Result<FrameBinding, Error> {
        self.store
            .replace_local_pending_current(self.window, index, value)
    }

    // Preserve the direct SlotStore call at numeric operand consumers.
    #[inline]
    pub(in crate::engine::vm) fn pop(&mut self) -> Result<JsValue, Error> {
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

    pub(in crate::engine::vm) fn local(&self, index: u16) -> Result<&FrameBinding, Error> {
        self.store.local_current(self.window, index)
    }

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

    /// Non-owning object identity read of a direct local binding for the
    /// IC-based field-addition span. The copied id carries no owner edge; only
    /// the outlined guard and the cache peek consume it.
    #[inline]
    pub(in crate::engine::vm) fn immediate_object_local(&self, index: u16) -> Option<ObjectId> {
        let index = usize::from(index);
        if index >= self.window.locals().len() {
            return None;
        }
        let FrameBinding::Direct(JsValue::Object(object)) =
            self.store.slots[self.window.locals().start + index].as_ref()?
        else {
            return None;
        };
        Some(*object)
    }

    /// Write a number back into a local whose direct numeric binding was
    /// already proved by `immediate_local` for the same index.
    #[inline]
    pub(in crate::engine::vm) fn store_number_local(&mut self, index: u16, value: Number) {
        self.store
            .store_number_local_current(self.window, index, value);
    }

    #[inline]
    pub(in crate::engine::vm) fn replace_local(
        &mut self,
        index: u16,
        value: FrameBinding,
    ) -> Result<FrameBinding, Error> {
        self.store.replace_local_current(self.window, index, value)
    }

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

    pub(in crate::engine::vm) fn release_operand(
        &mut self,
        from_top: usize,
        runtime: &Runtime,
    ) -> Result<bool, Error> {
        self.store
            .release_operand_current(self.window, from_top, runtime)
    }

    pub(in crate::engine::vm) fn typed_array_number_write(
        &mut self,
        runtime: &Runtime,
    ) -> Result<bool, Error> {
        self.store
            .typed_array_number_write_current(self.window, runtime)
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
    pub(in crate::engine::vm) fn property_ic_write_scalar(
        &mut self,
        runtime: &Runtime,
        executable: &crate::engine::code::runtime::PublishedFunctionSnapshot,
        pc: usize,
        key: u32,
    ) -> Result<bool, Error> {
        self.store
            .property_ic_write_scalar_current(self.window, runtime, executable, pc, key)
    }

    pub(in crate::engine::vm) fn array_immediate_read(
        &mut self,
        runtime: &Runtime,
    ) -> Result<bool, Error> {
        self.store
            .array_immediate_read_current(self.window, runtime)
    }

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
    pub(in crate::engine::vm) fn consume_number_pair(
        &mut self,
        operation: impl FnOnce(
            crate::engine::value::number::operations::Number,
            crate::engine::value::number::operations::Number,
        ) -> bool,
    ) -> Result<Option<bool>, Error> {
        self.store
            .consume_number_pair_current(self.window, operation)
    }

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
