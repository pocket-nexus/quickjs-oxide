//! Destination indexing and direct binding ownership tests.
use super::Error;
use super::window::{DirectSlot, FrameSlots};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::engine::vm) enum StoreProgress {
    Committed,
    NeedsObservation,
}

impl FrameSlots<'_> {
    pub(super) fn destination_index(&self, destination: DirectSlot) -> Result<usize, Error> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::api::Runtime;
    use crate::engine::code::function::metadata::{ClosureVariableKind, VariableDefinition};
    use crate::engine::code::runtime::PublishedFunctionSnapshot;
    use crate::engine::vm::stack::{FrameBinding, JsValue, copy_value};
    use crate::engine::vm::stack::{FrameStorage, FrameWindow, SlotStore};
    use std::rc::Rc;

    fn frame(
        runtime: &Runtime,
        local: FrameBinding,
        parameter: FrameBinding,
    ) -> (SlotStore, FrameWindow) {
        let context = runtime.new_context().expect("create context");
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
                slots
                    .put_direct_in_state(
                        &mut runtime.0.state.borrow_mut(),
                        &runtime.0.poisoned,
                        DirectSlot::Local(0)
                    )
                    .unwrap(),
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
                slots
                    .set_direct_in_state(
                        &mut runtime.0.state.borrow_mut(),
                        &runtime.0.poisoned,
                        DirectSlot::Argument(0)
                    )
                    .unwrap(),
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
    fn displaced_last_owner_is_released_under_current_state() {
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
                slots
                    .put_direct_in_state(
                        &mut runtime.0.state.borrow_mut(),
                        &runtime.0.poisoned,
                        DirectSlot::Local(0)
                    )
                    .unwrap(),
                StoreProgress::Committed
            );
            assert!(slots.peek(0).is_err());
            assert!(matches!(
                slots.direct_value(DirectSlot::Local(0)),
                Some(JsValue::Int(17))
            ));
        }
        assert_eq!(window.depth, 0);
        assert!(runtime.0.state.borrow().heap.object(id).is_err());
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
                slots
                    .set_direct_in_state(
                        &mut runtime.0.state.borrow_mut(),
                        &runtime.0.poisoned,
                        DirectSlot::Local(0)
                    )
                    .unwrap(),
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
                slots
                    .put_direct_in_state(
                        &mut runtime.0.state.borrow_mut(),
                        &runtime.0.poisoned,
                        DirectSlot::Local(0)
                    )
                    .unwrap(),
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
                    slots
                        .initialize_direct_local_in_state(
                            &mut runtime.0.state.borrow_mut(),
                            &runtime.0.poisoned,
                            0
                        )
                        .unwrap(),
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
