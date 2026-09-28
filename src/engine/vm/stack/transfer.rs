//! Ownership-aware writes to direct frame bindings.
//!
//! Admission and commit share one authenticated `FrameSlots` borrow. A declined
//! write leaves both the operand and destination untouched for the existing
//! observing path.
use super::window::{DirectSlot, FrameSlots};
#[cfg(feature = "profiling")]
use super::{Cost, record_owned_storage};
use super::{Error, FrameBinding, JsValue, Runtime, copy_value};
use crate::engine::heap::SlotReleaseReadiness;
use crate::engine::vm::exception::runtime_error_to_vm_error;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::engine::vm) enum StoreProgress {
    Committed,
    NeedsObservation,
}

#[derive(Clone, Copy)]
enum Transfer {
    Put,
    Set,
    Initialize,
}

#[derive(Clone, Copy)]
enum Displaced {
    Uninitialized,
    Scalar,
    Owned,
}

impl FrameSlots<'_> {
    pub(in crate::engine::vm) fn put_direct(
        &mut self,
        runtime: &Runtime,
        destination: DirectSlot,
    ) -> Result<StoreProgress, Error> {
        self.transfer_direct(runtime, destination, Transfer::Put)
    }

    pub(in crate::engine::vm) fn set_direct(
        &mut self,
        runtime: &Runtime,
        destination: DirectSlot,
    ) -> Result<StoreProgress, Error> {
        self.transfer_direct(runtime, destination, Transfer::Set)
    }

    pub(in crate::engine::vm) fn initialize_direct_local(
        &mut self,
        runtime: &Runtime,
        destination: u16,
    ) -> Result<StoreProgress, Error> {
        self.transfer_direct(
            runtime,
            DirectSlot::Local(destination),
            Transfer::Initialize,
        )
    }

    /// Reset a direct lexical slot with the same displaced-owner admission as
    /// the ordinary stores. Captured and special slots remain driver work.
    pub(in crate::engine::vm) fn reset_direct_local(
        &mut self,
        runtime: &Runtime,
        destination: u16,
    ) -> Result<StoreProgress, Error> {
        let index = self.destination_index(DirectSlot::Local(destination))?;
        let old = self.store.slots[index]
            .as_ref()
            .ok_or_else(|| Error::internal("owned local is vacant"))?;
        if matches!(old, FrameBinding::Uninitialized) {
            return Ok(StoreProgress::Committed);
        }
        let Some(class) = admit_displaced(runtime, old)? else {
            record_observation();
            return Ok(StoreProgress::NeedsObservation);
        };
        let old = self.store.slots[index]
            .replace(FrameBinding::Uninitialized)
            .expect("admitted direct local disappeared");
        #[cfg(feature = "profiling")]
        record_owned_storage(Cost::Move(2));
        release_displaced(runtime, old, class)?;
        record_completion(class);
        Ok(StoreProgress::Committed)
    }

    fn transfer_direct(
        &mut self,
        runtime: &Runtime,
        destination: DirectSlot,
        operation: Transfer,
    ) -> Result<StoreProgress, Error> {
        let destination_index = self.destination_index(destination)?;
        let old = self.store.slots[destination_index]
            .as_ref()
            .ok_or_else(|| Error::internal("owned destination is vacant"))?;
        // Match the canonical operand validation before any ownership work.
        let source = self.peek(0)?;
        if !is_scalar(source)
            && (runtime.0.deferred_references.has_pending()
                || runtime
                    .0
                    .state
                    .try_borrow()
                    .map_or(true, |state| state.heap.has_pending_zero_cleanup()))
        {
            record_observation();
            return Ok(StoreProgress::NeedsObservation);
        }
        let Some(class) = admit_displaced(runtime, old)? else {
            record_observation();
            return Ok(StoreProgress::NeedsObservation);
        };
        // A copied owner is obtained before the first mutation. There is no
        // fallible destination lookup or other runtime work after this point.
        let copy = if matches!(operation, Transfer::Set) {
            Some(copy_value(runtime, source)?)
        } else {
            None
        };
        let value = match copy {
            Some(value) => value,
            None => {
                let operand_index = self.window.operands().start + self.window.depth - 1;
                let Some(FrameBinding::Direct(value)) = self.store.slots[operand_index].take()
                else {
                    unreachable!("peek authenticated the operand")
                };
                self.window.depth -= 1;
                #[cfg(feature = "profiling")]
                {
                    self.store.live_slots -= 1;
                    record_owned_storage(Cost::Clear(1));
                }
                value
            }
        };
        let old = self.store.slots[destination_index]
            .replace(FrameBinding::Direct(value))
            .expect("admitted direct destination disappeared");
        #[cfg(feature = "profiling")]
        record_owned_storage(Cost::Move(2));
        release_displaced(runtime, old, class)?;
        record_completion(class);
        Ok(StoreProgress::Committed)
    }

    fn destination_index(&self, destination: DirectSlot) -> Result<usize, Error> {
        let (region, index) = match destination {
            DirectSlot::Local(index) => (self.window.locals(), usize::from(index)),
            DirectSlot::Argument(index) => (self.window.parameters(), usize::from(index)),
        };
        if index >= region.len() {
            return Err(Error::internal("owned destination index is out of bounds"));
        }
        Ok(region.start + index)
    }
}

fn admit_displaced(runtime: &Runtime, old: &FrameBinding) -> Result<Option<Displaced>, Error> {
    let class = match old {
        FrameBinding::Uninitialized => Displaced::Uninitialized,
        FrameBinding::Direct(value) if is_scalar(value) => Displaced::Scalar,
        FrameBinding::Direct(value) => {
            if runtime
                .slot_value_release_readiness_jsvalue(value)
                .map_err(runtime_error_to_vm_error)?
                != SlotReleaseReadiness::Ready
            {
                return Ok(None);
            }
            Displaced::Owned
        }
        FrameBinding::Captured(_) | FrameBinding::Private(_) | FrameBinding::PrivateCallable(_) => {
            return Ok(None);
        }
    };
    Ok(Some(class))
}

fn is_scalar(value: &JsValue) -> bool {
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

fn release_displaced(runtime: &Runtime, old: FrameBinding, class: Displaced) -> Result<(), Error> {
    match (class, old) {
        (Displaced::Uninitialized, FrameBinding::Uninitialized) => Ok(()),
        (Displaced::Scalar | Displaced::Owned, FrameBinding::Direct(value)) => runtime
            .release_jsvalue(value)
            .map_err(runtime_error_to_vm_error),
        _ => unreachable!("displaced binding changed during admission"),
    }
}

#[inline]
fn record_completion(_class: Displaced) {
    #[cfg(feature = "profiling")]
    crate::engine::api::profiling::record_owned_execution_event(match _class {
        Displaced::Uninitialized => "ordinary_store.complete_uninitialized",
        Displaced::Scalar => "ordinary_store.complete_scalar",
        Displaced::Owned => "ordinary_store.complete_owned",
    });
}

#[inline]
fn record_observation() {
    #[cfg(feature = "profiling")]
    crate::engine::api::profiling::record_owned_execution_event("ordinary_store.observe");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::api::Runtime;
    use crate::engine::code::function::metadata::{ClosureVariableKind, VariableDefinition};
    use crate::engine::code::runtime::PublishedFunctionSnapshot;
    use crate::engine::vm::stack::{FrameStorage, FrameWindow, SlotStore};
    use std::rc::Rc;

    fn frame(
        runtime: &Runtime,
        local: FrameBinding,
        parameter: FrameBinding,
    ) -> (SlotStore, FrameWindow) {
        let context = runtime.new_context();
        let mut owner = PublishedFunctionSnapshot::empty_for_test(context.realm);
        owner.metadata.argument_count = 1;
        owner.metadata.local_count = 1;
        owner.metadata.max_stack = 2;
        let definition = VariableDefinition {
            name: None,
            is_lexical: false,
            is_const: false,
            is_parameter_initializer: false,
            kind: ClosureVariableKind::Normal,
        };
        owner.argument_definitions = Rc::from([definition]);
        owner.local_definitions = Rc::from([definition]);
        let mut store = SlotStore::new(20);
        let window = store
            .push_frame(
                runtime,
                &owner.frame_layout(),
                FrameStorage {
                    original_arguments: vec![],
                    parameters: vec![parameter],
                    locals: vec![local],
                    operands: vec![],
                },
            )
            .unwrap();
        (store, window)
    }

    #[test]
    fn put_moves_object_into_undefined_without_an_extra_owner() {
        let runtime = Runtime::new();
        let (mut store, mut window) = frame(
            &runtime,
            FrameBinding::Direct(JsValue::Undefined),
            FrameBinding::Direct(JsValue::Int(3)),
        );
        let object = runtime.new_object(None).unwrap();
        let id = object.object_id();
        {
            let mut slots = store.borrow_frame_slots(&mut window).unwrap();
            slots.push(JsValue::Object(object.into_handle())).unwrap();
            assert_eq!(
                slots.put_direct(&runtime, DirectSlot::Local(0)).unwrap(),
                StoreProgress::Committed
            );
            assert!(slots.peek(0).is_err());
            assert!(
                matches!(slots.direct_value(DirectSlot::Local(0)), Some(JsValue::Object(actual)) if *actual == id)
            );
        }
        assert_eq!(window.depth, 0);
        assert_eq!(
            runtime
                .0
                .state
                .borrow()
                .heap
                .object_strong_count(id)
                .unwrap(),
            1
        );
        store.clear_frame(&runtime, window).unwrap();
    }

    #[test]
    fn set_copies_owner_and_preserves_operand_for_parameter() {
        let runtime = Runtime::new();
        let (mut store, mut window) = frame(
            &runtime,
            FrameBinding::Direct(JsValue::Undefined),
            FrameBinding::Direct(JsValue::Int(3)),
        );
        let object = runtime.new_object(None).unwrap();
        let id = object.object_id();
        {
            let mut slots = store.borrow_frame_slots(&mut window).unwrap();
            slots.push(JsValue::Object(object.into_handle())).unwrap();
            assert_eq!(
                slots.set_direct(&runtime, DirectSlot::Argument(0)).unwrap(),
                StoreProgress::Committed
            );
            assert!(matches!(slots.peek(0), Ok(JsValue::Object(actual)) if *actual == id));
            assert!(
                matches!(slots.direct_value(DirectSlot::Argument(0)), Some(JsValue::Object(actual)) if *actual == id)
            );
        }
        assert_eq!(window.depth, 1);
        assert_eq!(
            runtime
                .0
                .state
                .borrow()
                .heap
                .object_strong_count(id)
                .unwrap(),
            2
        );
        store.clear_frame(&runtime, window).unwrap();
    }

    #[test]
    fn last_owner_declines_and_leaves_original_slots() {
        let runtime = Runtime::new();
        let object = runtime.new_object(None).unwrap();
        let id = object.object_id();
        let (mut store, mut window) = frame(
            &runtime,
            FrameBinding::Direct(JsValue::Object(object.into_handle())),
            FrameBinding::Direct(JsValue::Int(3)),
        );
        {
            let mut slots = store.borrow_frame_slots(&mut window).unwrap();
            slots.push(JsValue::Int(17)).unwrap();
            assert_eq!(
                slots.put_direct(&runtime, DirectSlot::Local(0)).unwrap(),
                StoreProgress::NeedsObservation
            );
            assert_eq!(slots.peek(0).unwrap(), &JsValue::Int(17));
            assert!(
                matches!(slots.direct_value(DirectSlot::Local(0)), Some(JsValue::Object(actual)) if *actual == id)
            );
        }
        assert_eq!(window.depth, 1);
        assert_eq!(
            runtime
                .0
                .state
                .borrow()
                .heap
                .object_strong_count(id)
                .unwrap(),
            1
        );
        store.clear_frame(&runtime, window).unwrap();
    }

    #[test]
    fn identity_alias_preserves_each_distinct_slot_owner() {
        let runtime = Runtime::new();
        let object = runtime.new_object(None).unwrap();
        let id = object.object_id();
        let (mut store, mut window) = frame(
            &runtime,
            FrameBinding::Direct(JsValue::Object(object.into_handle())),
            FrameBinding::Direct(JsValue::Undefined),
        );
        {
            let mut slots = store.borrow_frame_slots(&mut window).unwrap();
            let operand =
                copy_value(&runtime, slots.direct_value(DirectSlot::Local(0)).unwrap()).unwrap();
            slots.push(operand).unwrap();
            assert_eq!(
                runtime
                    .0
                    .state
                    .borrow()
                    .heap
                    .object_strong_count(id)
                    .unwrap(),
                2
            );
            assert_eq!(
                slots.set_direct(&runtime, DirectSlot::Local(0)).unwrap(),
                StoreProgress::Committed
            );
            assert_eq!(
                runtime
                    .0
                    .state
                    .borrow()
                    .heap
                    .object_strong_count(id)
                    .unwrap(),
                2
            );
            assert_eq!(
                slots.put_direct(&runtime, DirectSlot::Local(0)).unwrap(),
                StoreProgress::Committed
            );
            assert!(slots.peek(0).is_err());
        }
        assert_eq!(
            runtime
                .0
                .state
                .borrow()
                .heap
                .object_strong_count(id)
                .unwrap(),
            1
        );
        store.clear_frame(&runtime, window).unwrap();
    }

    #[test]
    fn initialize_accepts_tdz_and_direct_scalar_destinations() {
        let runtime = Runtime::new();
        for local in [
            FrameBinding::Uninitialized,
            FrameBinding::Direct(JsValue::Int(8)),
        ] {
            let (mut store, mut window) =
                frame(&runtime, local, FrameBinding::Direct(JsValue::Undefined));
            {
                let mut slots = store.borrow_frame_slots(&mut window).unwrap();
                slots.push(JsValue::Bool(true)).unwrap();
                assert_eq!(
                    slots.initialize_direct_local(&runtime, 0).unwrap(),
                    StoreProgress::Committed
                );
                assert_eq!(
                    slots.direct_value(DirectSlot::Local(0)),
                    Some(&JsValue::Bool(true))
                );
                assert!(slots.peek(0).is_err());
            }
            store.clear_frame(&runtime, window).unwrap();
        }
    }
}
