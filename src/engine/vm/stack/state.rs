//! Slot consumers using the execution segment's existing state access.

use super::{
    Error, FrameBinding, FrameSlots, JsValue, NamedReadOperation, PropertyReadProgress,
    runtime_error_to_vm_error,
};
use crate::engine::heap::runtime::RuntimeState;

#[inline(always)]
pub(in crate::engine::vm) fn copy_value_in_state(
    state: &mut RuntimeState,
    value: &JsValue,
) -> Result<JsValue, Error> {
    let copied = match value {
        JsValue::Undefined => JsValue::Undefined,
        JsValue::Null => JsValue::Null,
        JsValue::Bool(value) => JsValue::Bool(*value),
        JsValue::Int(value) => JsValue::Int(*value),
        JsValue::Float(value) => JsValue::Float(*value),
        JsValue::ShortBigInt(value) => JsValue::ShortBigInt(*value),
        _ => return copy_reference_in_state(state, value),
    };
    #[cfg(feature = "profiling")]
    super::record_copy(value);
    Ok(copied)
}

#[inline(never)]
fn copy_reference_in_state(state: &mut RuntimeState, value: &JsValue) -> Result<JsValue, Error> {
    let copied = match value {
        // Keep the stack's existing trusted leaf-retain/saturation behavior.
        JsValue::String(id) => {
            state.heap.retain_string_fast(*id);
            JsValue::String(*id)
        }
        JsValue::BigInt(id) => {
            state.heap.retain_bigint_fast(*id);
            JsValue::BigInt(*id)
        }
        _ => state
            .dup_jsvalue(value)
            .map_err(runtime_error_to_vm_error)?,
    };
    #[cfg(feature = "profiling")]
    super::record_copy(value);
    Ok(copied)
}

impl FrameSlots<'_> {
    pub(in crate::engine::vm) fn nullish_equality_in_state(
        &mut self,
        state: &mut RuntimeState,
        poisoned: &std::cell::Cell<bool>,
    ) -> Result<Option<bool>, Error> {
        let (Ok(left), Ok(right)) = (self.peek(1), self.peek(0)) else {
            return Ok(None);
        };
        let other = if matches!(left, JsValue::Null | JsValue::Undefined) {
            right
        } else if matches!(right, JsValue::Null | JsValue::Undefined) {
            left
        } else {
            return Ok(None);
        };
        let equal = match other {
            JsValue::Null | JsValue::Undefined => true,
            JsValue::Object(id) => {
                let count = state
                    .heap
                    .object_strong_count(*id)
                    .map_err(|error| Error::internal(error.to_string()))?;
                if count == 0 || count >= u32::MAX - 1 {
                    return Ok(None);
                }
                state
                    .heap
                    .object(*id)
                    .map_err(|error| Error::internal(error.to_string()))?
                    .is_html_dda
            }
            _ => return Ok(None),
        };
        let right = self.pop()?;
        let left = self.pop()?;
        state
            .release_owned_jsvalue(poisoned, left)
            .map_err(runtime_error_to_vm_error)?;
        state
            .release_owned_jsvalue(poisoned, right)
            .map_err(runtime_error_to_vm_error)?;
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_execution_event("nullish_comparison.local");
        Ok(Some(equal))
    }

    pub(in crate::engine::vm) fn put_direct_in_state(
        &mut self,
        state: &mut RuntimeState,
        poisoned: &std::cell::Cell<bool>,
        destination: super::DirectSlot,
    ) -> Result<super::StoreProgress, Error> {
        self.transfer_direct_in_state(state, poisoned, destination, false)
    }

    pub(in crate::engine::vm) fn set_direct_in_state(
        &mut self,
        state: &mut RuntimeState,
        poisoned: &std::cell::Cell<bool>,
        destination: super::DirectSlot,
    ) -> Result<super::StoreProgress, Error> {
        self.transfer_direct_in_state(state, poisoned, destination, true)
    }

    pub(in crate::engine::vm) fn initialize_direct_local_in_state(
        &mut self,
        state: &mut RuntimeState,
        poisoned: &std::cell::Cell<bool>,
        destination: u16,
    ) -> Result<super::StoreProgress, Error> {
        self.transfer_direct_in_state(
            state,
            poisoned,
            super::DirectSlot::Local(destination),
            false,
        )
    }

    pub(in crate::engine::vm) fn reset_direct_local_in_state(
        &mut self,
        state: &mut RuntimeState,
        poisoned: &std::cell::Cell<bool>,
        destination: u16,
    ) -> Result<super::StoreProgress, Error> {
        let index = self.destination_index(super::DirectSlot::Local(destination))?;
        match self.store.slots[index].as_ref() {
            Some(FrameBinding::Uninitialized) => return Ok(super::StoreProgress::Committed),
            Some(FrameBinding::Direct(_)) => {}
            Some(_) => return Ok(super::StoreProgress::NeedsObservation),
            None => return Err(Error::internal("owned local is vacant")),
        }
        let FrameBinding::Direct(old) = self.store.slots[index]
            .replace(FrameBinding::Uninitialized)
            .expect("admitted direct local")
        else {
            unreachable!("admitted direct local")
        };
        state
            .release_owned_jsvalue(poisoned, old)
            .map_err(runtime_error_to_vm_error)?;
        Ok(super::StoreProgress::Committed)
    }

    fn transfer_direct_in_state(
        &mut self,
        state: &mut RuntimeState,
        poisoned: &std::cell::Cell<bool>,
        destination: super::DirectSlot,
        keep: bool,
    ) -> Result<super::StoreProgress, Error> {
        let index = self.destination_index(destination)?;
        let old = self.store.slots[index]
            .as_ref()
            .ok_or_else(|| Error::internal("owned destination is vacant"))?;
        self.peek(0)?;
        if !matches!(old, FrameBinding::Direct(_) | FrameBinding::Uninitialized) {
            return Ok(super::StoreProgress::NeedsObservation);
        }
        // Retain failure leaves the source and destination untouched. Moving
        // consumes the original source edge; source/destination aliases work
        // because the old destination is released after the new edge arrives.
        let value = if keep {
            copy_value_in_state(state, self.peek(0)?)?
        } else {
            self.pop()?
        };
        let old = self.store.slots[index]
            .replace(FrameBinding::Direct(value))
            .expect("admitted direct destination");
        match old {
            FrameBinding::Direct(old) => state
                .release_owned_jsvalue(poisoned, old)
                .map_err(runtime_error_to_vm_error)?,
            FrameBinding::Uninitialized => {}
            _ => unreachable!("admitted direct destination"),
        }
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_execution_event("local_completion.store");
        Ok(super::StoreProgress::Committed)
    }

    pub(in crate::engine::vm) fn property_ic_read_in_state(
        &mut self,
        state: &mut RuntimeState,
        poisoned: &std::cell::Cell<bool>,
        domain: u64,
        executable: &crate::engine::code::runtime::PublishedFunctionSnapshot,
        operation: NamedReadOperation,
        native: &mut Option<crate::engine::object::LinkedNativeSelection>,
    ) -> Result<PropertyReadProgress, Error> {
        let output_index = if operation.keep_receiver {
            let Ok(index) = self.store.operand_push_index(self.window) else {
                return Ok(PropertyReadProgress::Driver);
            };
            Some(index)
        } else {
            None
        };
        let mut miss = crate::engine::object::NamedSelectionMiss::ContinueLookup;
        let value = match state.select_linked_data_into(
            domain,
            self.peek(0)?,
            executable,
            operation.site,
            operation.key_index,
            operation.keep_receiver,
            native,
            &mut miss,
        ) {
            Some(value) => value,
            None => match miss {
                crate::engine::object::NamedSelectionMiss::CompleteAbsent => JsValue::Undefined,
                crate::engine::object::NamedSelectionMiss::Accessor(getter) => {
                    return Ok(PropertyReadProgress::Selected(getter));
                }
                crate::engine::object::NamedSelectionMiss::ContinueLookup => {
                    return Ok(PropertyReadProgress::Driver);
                }
            },
        };
        if let Some(index) = output_index {
            self.store.install_operand(self.window, index, value);
        } else {
            let index = self.window.operands().start + self.window.depth - 1;
            let Some(FrameBinding::Direct(base)) =
                self.store.slots[index].replace(FrameBinding::Direct(value))
            else {
                unreachable!("selected property receiver occupied its authenticated slot")
            };
            state
                .release_owned_jsvalue(poisoned, base)
                .map_err(runtime_error_to_vm_error)?;
        }
        Ok(PropertyReadProgress::Completed)
    }

    pub(in crate::engine::vm) fn insert_copy_in_state(
        &mut self,
        state: &mut RuntimeState,
        source: usize,
        destination: usize,
    ) -> Result<(), Error> {
        self.peek(source)?;
        if destination > self.window.depth {
            return Err(Error::internal("owned insertion exceeds verified capacity"));
        }
        let index = self.store.operand_push_index(self.window)?;
        let copied = copy_value_in_state(state, self.peek(source)?)?;
        self.store.install_operand(self.window, index, copied);
        self.rotate_operands(0, destination + 1, false)
    }

    pub(in crate::engine::vm) fn duplicate_operands_in_state(
        &mut self,
        state: &mut RuntimeState,
        count: usize,
    ) -> Result<(), Error> {
        let source = count
            .checked_sub(1)
            .ok_or_else(|| Error::internal("empty owned operand duplication"))?;
        self.peek(source)?;
        if !self.has_operand_capacity(count) {
            return Err(Error::internal(
                "owned duplication exceeds verified capacity",
            ));
        }
        for _ in 0..count {
            // A failed retain leaves each completed copy in execution storage.
            let index = self.store.operand_push_index(self.window)?;
            let copied = copy_value_in_state(state, self.peek(source)?)?;
            self.store.install_operand(self.window, index, copied);
        }
        Ok(())
    }

    pub(in crate::engine::vm) fn array_kept_read_in_state(
        &mut self,
        state: &mut RuntimeState,
        poisoned: &std::cell::Cell<bool>,
        keep_key: bool,
    ) -> Result<bool, Error> {
        let index = match self.peek(0)? {
            JsValue::Int(index) if *index >= 0 => *index as u32,
            JsValue::String(id) => {
                let Some(index) = crate::engine::atom::AtomTable::canonical_array_index(
                    state.heap.string_fast(*id),
                ) else {
                    return Ok(false);
                };
                index
            }
            _ => return Ok(false),
        };
        let Some(value) = state.try_dense_array_borrowed_read(self.peek(1)?, index) else {
            return Ok(false);
        };
        // Capacity is checked before any owner moves out of its slot.
        if keep_key {
            self.store.operand_push_index(self.window)?;
        } else {
            self.top_direct_mut()?;
        }
        let value = copy_value_in_state(state, &value)?;
        // Install the result before retiring the key. A cleanup failure still
        // leaves its owner reachable by execution's error/unwind traversal.
        if keep_key {
            self.push(value)?;
        } else {
            let key = std::mem::replace(self.top_direct_mut()?, value);
            state
                .release_owned_jsvalue(poisoned, key)
                .map_err(runtime_error_to_vm_error)?;
        }
        Ok(true)
    }

    pub(in crate::engine::vm) fn array_read_in_state(
        &mut self,
        state: &mut RuntimeState,
        poisoned: &std::cell::Cell<bool>,
    ) -> Result<bool, Error> {
        let index = match self.peek(0)? {
            JsValue::Int(index) if *index >= 0 => *index as u32,
            JsValue::String(id) => {
                let Some(index) = crate::engine::atom::AtomTable::canonical_array_index(
                    state.heap.string_fast(*id),
                ) else {
                    return Ok(false);
                };
                index
            }
            _ => return Ok(false),
        };
        let Some(value) = state.try_array_value_read(self.peek(1)?, index) else {
            return Ok(false);
        };
        // Retain while the receiver still owns the selected edge. The
        // resulting owner is committed before either input is retired.
        let value = copy_value_in_state(state, &value)?;
        let key = self.pop()?;
        let base = self.pop()?;
        // Reuses an occupied operand position; no allocation or capacity
        // failure can follow the lookup and removal of these two owners.
        self.push(value)?;
        state
            .release_owned_jsvalue(poisoned, key)
            .map_err(runtime_error_to_vm_error)?;
        state
            .release_owned_jsvalue(poisoned, base)
            .map_err(runtime_error_to_vm_error)?;
        Ok(true)
    }

    pub(in crate::engine::vm) fn try_owned_field_write_in_state(
        &mut self,
        state: &mut RuntimeState,
        poisoned: &std::cell::Cell<bool>,
        pressure: &crate::engine::heap::gc_pressure::GcPressure,
        domain: u64,
        executable: &crate::engine::code::runtime::PublishedFunctionSnapshot,
        key: u32,
    ) -> Result<bool, Error> {
        let JsValue::Object(object) = self.peek(1)? else {
            return Ok(false);
        };
        let object = *object;
        let input = self.top_direct_mut()?;
        let stored = state
            .try_store_owned_linked_field(poisoned, domain, object, input, executable, key, None)
            .map_err(runtime_error_to_vm_error)?;
        match stored {
            crate::engine::object::FieldStore::Miss => return Ok(false),
            crate::engine::object::FieldStore::LayoutPublished => {
                self.finish_added_field_write(state, poisoned, pressure)?;
                return Ok(true);
            }
            crate::engine::object::FieldStore::Existing => {}
        }
        self.retire_field_write(state, poisoned)?;
        Ok(true)
    }

    /// Only a new layout needs a collection checkpoint. Keep this scheduling
    /// boundary outside the existing-slot consumer; no tag or pressure borrow
    /// survives across that consumer's two owner releases.
    #[inline(never)]
    fn finish_added_field_write(
        &mut self,
        state: &mut RuntimeState,
        poisoned: &std::cell::Cell<bool>,
        pressure: &crate::engine::heap::gc_pressure::GcPressure,
    ) -> Result<(), Error> {
        self.retire_field_write(state, poisoned)?;
        state
            .collect_if_requested(pressure, poisoned)
            .map_err(runtime_error_to_vm_error)
    }

    #[inline]
    fn retire_field_write(
        &mut self,
        state: &mut RuntimeState,
        poisoned: &std::cell::Cell<bool>,
    ) -> Result<(), Error> {
        // Selection never moved the receiver. The input now owns the old slot
        // value, and it is retired before the receiver's final owner can die.
        state
            .release_owned_jsvalue(poisoned, self.pop()?)
            .map_err(runtime_error_to_vm_error)?;
        state
            .release_owned_jsvalue(poisoned, self.pop()?)
            .map_err(runtime_error_to_vm_error)?;
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_execution_event(
            "ordinary_owned_field_write_in_execute",
        );
        Ok(())
    }

    /// Keep the temporary owner in the execution store until publication. A
    /// declined operation leaves the exact value and stack depth unchanged.
    fn top_direct_mut(&mut self) -> Result<&mut JsValue, Error> {
        self.peek(0)?;
        let top = self.window.operands().start + self.window.depth - 1;
        match self.store.slots[top].as_mut() {
            Some(FrameBinding::Direct(value)) => Ok(value),
            _ => Err(Error::internal("admitted operand is not direct")),
        }
    }

    pub(in crate::engine::vm) fn try_owned_element_write_in_state(
        &mut self,
        state: &mut RuntimeState,
        poisoned: &std::cell::Cell<bool>,
    ) -> Result<bool, Error> {
        let JsValue::Int(index) = self.peek(1)? else {
            return Ok(false);
        };
        let Ok(index) = u32::try_from(*index) else {
            return Ok(false);
        };
        let dense = if let JsValue::Object(object) = self.peek(2)? {
            let object = *object;
            let input = self.top_direct_mut()?;
            state
                .try_exchange_dense_value(object, index, input)
                .map_err(runtime_error_to_vm_error)?
                || state
                    .try_append_dense_value(object, index, input)
                    .map_err(runtime_error_to_vm_error)?
        } else {
            false
        };
        let typed = !dense
            && match self.peek(0)? {
                JsValue::Int(value) => {
                    state.try_typed_array_number_write(self.peek(2)?, index, f64::from(*value))
                }
                JsValue::Float(value) => {
                    state.try_typed_array_number_write(self.peek(2)?, index, *value)
                }
                _ => false,
            };
        if !dense && !typed {
            return Ok(false);
        }
        // Replacement moved the old entry into the frame. Append consumed the
        // new owner and left undefined. Typed writes only admit numbers.
        state
            .release_owned_jsvalue(poisoned, self.pop()?)
            .map_err(runtime_error_to_vm_error)?;
        let _integer_key = self.pop()?;
        state
            .release_owned_jsvalue(poisoned, self.pop()?)
            .map_err(runtime_error_to_vm_error)?;
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_execution_event(if dense {
            "dense_array_owned_write_in_execute"
        } else {
            "typed_array_number_write_in_execute"
        });
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{
        api::Runtime,
        code::{
            function::metadata::{ClosureVariableKind, VariableDefinition},
            runtime::PublishedFunctionSnapshot,
        },
        vm::stack::{DirectSlot, FrameStorage, SlotStore, StoreProgress},
    };

    #[test]
    fn dense_reads_commit_all_value_kinds_before_receiver_retirement() {
        for expression in [
            "[{}]",
            "['heap string']",
            "[9223372036854775808n]",
            "[Symbol('element')]",
            "[undefined]",
            "(()=>{let a=[];a[0]=a;return a})()",
        ] {
            for (keep_receiver, keep_key) in [(false, false), (true, false), (true, true)] {
                let runtime = Runtime::new();
                let mut context = runtime.new_context().unwrap();
                let base = runtime
                    .into_jsvalue(context.eval(expression).unwrap())
                    .unwrap();
                let JsValue::Object(array) = &base else {
                    panic!("array")
                };
                let array = *array;
                let raw = runtime
                    .0
                    .state
                    .borrow()
                    .heap
                    .object(array)
                    .unwrap()
                    .dense_array_value(0)
                    .unwrap()
                    .clone();
                let mut layout = PublishedFunctionSnapshot::empty_for_test(context.realm);
                layout.metadata.max_stack = 4;
                let mut store = SlotStore::new(4);
                let mut window = store
                    .push_frame(
                        &runtime,
                        &layout.frame_layout(),
                        FrameStorage {
                            original_arguments: vec![],
                            parameters: vec![],
                            locals: vec![],
                            operands: vec![JsValue::Int(99), base, JsValue::Int(0)],
                        },
                    )
                    .unwrap();
                {
                    let mut state = runtime.0.state.borrow_mut();
                    let mut slots = store.borrow_frame_slots(&mut window).unwrap();
                    let hit = if keep_receiver {
                        slots.array_kept_read_in_state(&mut state, &runtime.0.poisoned, keep_key)
                    } else {
                        slots.array_read_in_state(&mut state, &runtime.0.poisoned)
                    }
                    .unwrap();
                    assert!(hit, "{expression}");
                    assert_eq!(
                        slots.peek(0).unwrap(),
                        &JsValue::from_raw(raw.clone()).unwrap()
                    );
                    assert_eq!(
                        slots.window.depth,
                        2 + usize::from(keep_receiver) + usize::from(keep_key)
                    );
                    assert_eq!(
                        slots.peek(slots.window.depth - 1).unwrap(),
                        &JsValue::Int(99)
                    );
                    if keep_receiver {
                        assert_eq!(
                            slots.peek(1 + usize::from(keep_key)).unwrap(),
                            &JsValue::Object(array)
                        );
                    }
                    if keep_key {
                        assert_eq!(slots.peek(1).unwrap(), &JsValue::Int(0));
                    }
                    if let crate::engine::heap::RawValue::Object(target) = raw {
                        let expected = if target == array {
                            2 + u32::from(keep_receiver)
                        } else if keep_receiver {
                            2
                        } else {
                            1
                        };
                        assert_eq!(state.heap.object_strong_count(target), Ok(expected));
                    }
                }
                store.clear_frame(&runtime, window).unwrap();
                runtime.run_gc().unwrap();
                assert!(!runtime.is_poisoned());
                assert!(!runtime.0.deferred_references.has_pending());
            }
        }
    }

    #[cfg(feature = "profiling")]
    #[test]
    fn dense_heap_reads_finish_without_property_driver_handoff() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        let _ = context
            .eval("var denseReceiver=[{}];function readDenseValue(){return denseReceiver[0]}")
            .unwrap();
        let profile = crate::engine::api::profiling::CostProfile::start();
        assert_eq!(
            context
                .eval("readDenseValue() === denseReceiver[0]")
                .unwrap(),
            crate::engine::value::Value::Bool(true)
        );
        assert_eq!(
            profile
                .snapshot()
                .owned_execution_events
                .get("property_read_action_exit")
                .copied()
                .unwrap_or(0),
            0
        );
    }

    #[test]
    fn dense_read_failure_preserves_inputs_before_any_owner_transfer() {
        for capacity_failure in [false, true] {
            let runtime = Runtime::new();
            let mut context = runtime.new_context().unwrap();
            let base = runtime.into_jsvalue(context.eval("[{}]").unwrap()).unwrap();
            let JsValue::Object(array) = &base else {
                panic!("array")
            };
            let array = *array;
            let crate::engine::heap::RawValue::Object(target) = runtime
                .0
                .state
                .borrow()
                .heap
                .object(array)
                .unwrap()
                .dense_array_value(0)
                .unwrap()
                .clone()
            else {
                panic!("object element")
            };
            let mut layout = PublishedFunctionSnapshot::empty_for_test(context.realm);
            layout.metadata.max_stack = 2;
            let mut store = SlotStore::new(4);
            let mut window = store
                .push_frame(
                    &runtime,
                    &layout.frame_layout(),
                    FrameStorage {
                        original_arguments: vec![],
                        parameters: vec![],
                        locals: vec![],
                        operands: vec![base, JsValue::Int(0)],
                    },
                )
                .unwrap();
            {
                let mut state = runtime.0.state.borrow_mut();
                if !capacity_failure {
                    state.heap.set_strong_count_for_test(
                        crate::engine::heap::RawId::Object(target),
                        u32::MAX,
                    );
                }
                let mut slots = store.borrow_frame_slots(&mut window).unwrap();
                let result = if capacity_failure {
                    slots.array_kept_read_in_state(&mut state, &runtime.0.poisoned, true)
                } else {
                    slots.array_read_in_state(&mut state, &runtime.0.poisoned)
                };
                assert!(result.is_err());
                assert_eq!(slots.window.depth, 2);
                assert_eq!(slots.peek(0).unwrap(), &JsValue::Int(0));
                assert_eq!(slots.peek(1).unwrap(), &JsValue::Object(array));
                assert_eq!(
                    state.heap.object_strong_count(target),
                    Ok(if capacity_failure { 1 } else { u32::MAX })
                );
                state
                    .heap
                    .set_strong_count_for_test(crate::engine::heap::RawId::Object(target), 1);
            }
            store.clear_frame(&runtime, window).unwrap();
            assert!(runtime.0.state.borrow().heap.object(target).is_err());
            assert!(!runtime.is_poisoned());
        }
    }

    #[test]
    fn direct_replacement_releases_final_owner_and_kept_alias_survives() {
        let runtime = Runtime::new();
        let context = runtime.new_context().unwrap();
        let mut layout = PublishedFunctionSnapshot::empty_for_test(context.realm);
        layout.metadata.max_stack = 3;
        layout.metadata.local_count = 1;
        layout.local_definitions = std::rc::Rc::from([VariableDefinition {
            name: None,
            is_lexical: false,
            is_const: false,
            is_parameter_initializer: false,
            kind: ClosureVariableKind::Normal,
        }]);
        let old = runtime.new_object(None).unwrap();
        let old_id = old.object_id();
        let alias = runtime.new_object(None).unwrap();
        let alias_id = alias.object_id();
        let mut store = SlotStore::new(4);
        let mut window = store
            .push_frame(
                &runtime,
                &layout.frame_layout(),
                FrameStorage {
                    original_arguments: vec![],
                    parameters: vec![],
                    locals: vec![FrameBinding::Direct(JsValue::Object(old.into_handle()))],
                    operands: vec![JsValue::Object(alias.into_handle())],
                },
            )
            .unwrap();
        {
            let mut state = runtime.0.state.borrow_mut();
            let mut slots = store.borrow_frame_slots(&mut window).unwrap();
            assert!(state.heap.object(old_id).is_ok());
            assert_eq!(
                slots
                    .set_direct_in_state(&mut state, &runtime.0.poisoned, DirectSlot::Local(0))
                    .unwrap(),
                StoreProgress::Committed
            );
            assert!(state.heap.object(old_id).is_err());
            assert_eq!(state.heap.object_strong_count(alias_id).unwrap(), 2);
            assert_eq!(
                slots
                    .put_direct_in_state(&mut state, &runtime.0.poisoned, DirectSlot::Local(0))
                    .unwrap(),
                StoreProgress::Committed
            );
            assert_eq!(state.heap.object_strong_count(alias_id).unwrap(), 1);
            assert_eq!(
                slots
                    .reset_direct_local_in_state(&mut state, &runtime.0.poisoned, 0)
                    .unwrap(),
                StoreProgress::Committed
            );
            assert!(state.heap.object(alias_id).is_err());
        }
        store.clear_frame(&runtime, window).unwrap();
        assert!(!runtime.0.deferred_references.has_pending());
    }

    #[test]
    fn duplicate_failure_leaves_every_committed_prefix_owner_in_slots() {
        let runtime = Runtime::new();
        let context = runtime.new_context().unwrap();
        let mut layout = PublishedFunctionSnapshot::empty_for_test(context.realm);
        layout.metadata.max_stack = 4;
        let overflowing = runtime.new_object(None).unwrap();
        let overflow_id = overflowing.object_id();
        let first = runtime.new_object(None).unwrap();
        let first_id = first.object_id();
        let mut store = SlotStore::new(4);
        let mut window = store
            .push_frame(
                &runtime,
                &layout.frame_layout(),
                FrameStorage {
                    original_arguments: vec![],
                    parameters: vec![],
                    locals: vec![],
                    operands: vec![
                        JsValue::Object(first.into_handle()),
                        JsValue::Object(overflowing.into_handle()),
                    ],
                },
            )
            .unwrap();
        {
            let mut state = runtime.0.state.borrow_mut();
            state.heap.set_strong_count_for_test(
                crate::engine::heap::RawId::Object(overflow_id),
                u32::MAX,
            );
            let mut slots = store.borrow_frame_slots(&mut window).unwrap();
            assert!(slots.duplicate_operands_in_state(&mut state, 2).is_err());
            assert_eq!(slots.window.depth, 3);
            assert_eq!(state.heap.object_strong_count(first_id).unwrap(), 2);
            // Restore the injected count so the ordinary teardown checks all edges.
            state
                .heap
                .set_strong_count_for_test(crate::engine::heap::RawId::Object(overflow_id), 1);
        }
        store.clear_frame(&runtime, window).unwrap();
        let state = runtime.0.state.borrow();
        assert!(state.heap.object(first_id).is_err());
        assert!(state.heap.object(overflow_id).is_err());
    }
}
