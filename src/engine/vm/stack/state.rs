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

    pub(in crate::engine::vm) fn array_kept_immediate_read_in_state(
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
        let Some(value) = state.try_dense_array_kept_read(self.peek(1)?, index) else {
            return Ok(false);
        };
        // Capacity is checked before any owner moves out of its slot.
        if keep_key {
            self.store.operand_push_index(self.window)?;
        }
        if !keep_key {
            state
                .release_owned_jsvalue(poisoned, self.pop()?)
                .map_err(runtime_error_to_vm_error)?;
        }
        self.push(value)?;
        Ok(true)
    }

    pub(in crate::engine::vm) fn array_immediate_read_in_state(
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
        let Some(value) = state.try_array_immediate_read(self.peek(1)?, index) else {
            return Ok(false);
        };
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

    pub(in crate::engine::vm) fn try_scalar_field_write_in_state(
        &mut self,
        state: &mut RuntimeState,
        poisoned: &std::cell::Cell<bool>,
        domain: u64,
        executable: &crate::engine::code::runtime::PublishedFunctionSnapshot,
        key: u32,
    ) -> Result<bool, Error> {
        if !state
            .try_linked_scalar_field_write(domain, self.peek(1)?, self.peek(0)?, executable, key)
            .map_err(runtime_error_to_vm_error)?
        {
            return Ok(false);
        }
        let _scalar = self.pop()?;
        state
            .release_owned_jsvalue(poisoned, self.pop()?)
            .map_err(runtime_error_to_vm_error)?;
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_execution_event(
            "ordinary_scalar_field_write_in_execute",
        );
        Ok(true)
    }

    pub(in crate::engine::vm) fn try_scalar_element_write_in_state(
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
        let base = self.peek(2)?;
        let value = self.peek(0)?;
        let dense = state
            .try_dense_array_write_scalar(base, index, value)
            .map_err(runtime_error_to_vm_error)?;
        let typed = !dense
            && match value {
                JsValue::Int(value) => {
                    state.try_typed_array_number_write(base, index, f64::from(*value))
                }
                JsValue::Float(value) => state.try_typed_array_number_write(base, index, *value),
                _ => false,
            };
        if !dense && !typed {
            return Ok(false);
        }
        let _value = self.pop()?;
        let _key = self.pop()?;
        state
            .release_owned_jsvalue(poisoned, self.pop()?)
            .map_err(runtime_error_to_vm_error)?;
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_execution_event(if dense {
            "dense_array_scalar_write_in_execute"
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
