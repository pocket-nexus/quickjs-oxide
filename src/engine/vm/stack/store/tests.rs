use super::*;
use crate::engine::api::Value;
use crate::engine::code::function::metadata::{ClosureVariableKind, VariableDefinition};
use crate::engine::code::runtime::PublishedFunctionSnapshot;
use crate::engine::value::JsValue;
use crate::engine::vm::bindings::release_frame_binding;
use crate::engine::vm::stack::{FrameStorage, RunSlots};
use std::rc::Rc;

#[derive(Clone, Copy)]
enum Target {
    Local,
    Parameter,
}

impl Target {
    fn store(
        self,
        slots: &mut RunSlots<'_>,
        runtime: &Runtime,
        index: u16,
        mode: StoreMode,
    ) -> Result<Option<FrameBinding>, Error> {
        match self {
            Self::Local => slots.store_local_from_top(runtime, index, mode),
            Self::Parameter => slots.store_parameter_from_top(runtime, index, mode),
        }
    }

    fn offset(self, window: &FrameWindow) -> usize {
        match self {
            Self::Local => window.locals().start,
            Self::Parameter => window.parameters().start,
        }
    }
}

fn frame(runtime: &Runtime) -> (SlotStore, FrameWindow) {
    let context = runtime.new_context();
    let mut owner = PublishedFunctionSnapshot::empty_for_test(context.realm);
    owner.metadata.local_count = 1;
    owner.metadata.argument_count = 1;
    owner.metadata.max_stack = 2;
    let definitions: Rc<[VariableDefinition]> = Rc::from([VariableDefinition {
        name: None,
        is_lexical: false,
        is_const: false,
        is_parameter_initializer: false,
        kind: ClosureVariableKind::Normal,
    }]);
    owner.local_definitions = Rc::clone(&definitions);
    owner.argument_definitions = definitions;
    let mut store = SlotStore::new(16);
    let window = store
        .push_frame(
            runtime,
            &owner.frame_layout(),
            FrameStorage {
                original_arguments: vec![JsValue::Int(5)],
                parameters: vec![FrameBinding::Direct(JsValue::Int(7))],
                locals: vec![FrameBinding::Direct(JsValue::Int(7))],
                operands: vec![],
            },
        )
        .unwrap();
    (store, window)
}

#[test]
fn direct_store_consume_moves_and_keep_retains_exactly_one_owner() {
    for target in [Target::Local, Target::Parameter] {
        for mode in [StoreMode::Consume, StoreMode::Keep] {
            let runtime = Runtime::new();
            let (mut store, mut window) = frame(&runtime);
            let previous = runtime.new_object(None).unwrap();
            let previous_id = previous.object_id();
            store.slots[target.offset(&window)] = Some(FrameBinding::Direct(JsValue::Object(
                previous.into_handle(),
            )));
            let next = runtime.new_object(None).unwrap();
            let next_id = next.object_id();
            store.push(&mut window, JsValue::Int(11)).unwrap();
            store
                .push(&mut window, JsValue::Object(next.into_handle()))
                .unwrap();
            let old = {
                let mut transaction = store.frame_transaction(&mut window).unwrap();
                target
                    .store(&mut transaction.slots(), &runtime, 0, mode)
                    .unwrap()
                    .unwrap()
            };
            let keep = matches!(mode, StoreMode::Keep);
            assert_eq!(window.depth, if keep { 2 } else { 1 });
            assert!(matches!(
                &store.slots[target.offset(&window)],
                Some(FrameBinding::Direct(JsValue::Object(id))) if *id == next_id
            ));
            assert_eq!(
                runtime.0.state.borrow().heap.object_strong_count(next_id),
                Ok(if keep { 2 } else { 1 })
            );
            // Replacing a binding cannot reclaim the displaced final owner.
            assert_eq!(
                runtime
                    .0
                    .state
                    .borrow()
                    .heap
                    .object_strong_count(previous_id),
                Ok(1)
            );
            release_frame_binding(&runtime, old).unwrap();
            assert!(runtime.0.state.borrow().heap.object(previous_id).is_err());
            if keep {
                runtime
                    .release_jsvalue(store.pop(&mut window).unwrap())
                    .unwrap();
                assert_eq!(
                    runtime.0.state.borrow().heap.object_strong_count(next_id),
                    Ok(1)
                );
            }
            assert_eq!(store.pop(&mut window).unwrap(), JsValue::Int(11));
            assert!(store.slots[window.operands()].iter().all(Option::is_none));
            assert!(matches!(
                &store.slots[window.original_arguments().start],
                Some(FrameBinding::Direct(JsValue::Int(5)))
            ));
            #[cfg(feature = "profiling")]
            assert_eq!(store.live_slots, 3);
            store.clear_frame(&runtime, window).unwrap();
            assert!(runtime.0.state.borrow().heap.object(next_id).is_err());
        }
    }
}

#[test]
fn direct_store_self_alias_keeps_each_live_slot_owned() {
    for target in [Target::Local, Target::Parameter] {
        for mode in [StoreMode::Consume, StoreMode::Keep] {
            let runtime = Runtime::new();
            let (mut store, mut window) = frame(&runtime);
            let object = runtime.new_object(None).unwrap();
            let id = object.object_id();
            let operand = JsValue::Object(object.into_handle());
            store.slots[target.offset(&window)] =
                Some(FrameBinding::Direct(runtime.dup_jsvalue(&operand).unwrap()));
            store.push(&mut window, operand).unwrap();
            let old = target
                .store(
                    &mut store.run_window(&mut window).unwrap(),
                    &runtime,
                    0,
                    mode,
                )
                .unwrap()
                .unwrap();
            let keep = matches!(mode, StoreMode::Keep);
            assert_eq!(
                runtime.0.state.borrow().heap.object_strong_count(id),
                Ok(if keep { 3 } else { 2 })
            );
            release_frame_binding(&runtime, old).unwrap();
            assert_eq!(
                runtime.0.state.borrow().heap.object_strong_count(id),
                Ok(if keep { 2 } else { 1 })
            );
            store.clear_frame(&runtime, window).unwrap();
            assert!(runtime.0.state.borrow().heap.object(id).is_err());
        }
    }
}

#[test]
fn direct_store_preflight_errors_and_declines_preserve_both_slots() {
    for target in [Target::Local, Target::Parameter] {
        for mode in [StoreMode::Consume, StoreMode::Keep] {
            let runtime = Runtime::new();
            let (mut store, mut window) = frame(&runtime);
            let destination = target.offset(&window);
            assert!(
                target
                    .store(
                        &mut store.run_window(&mut window).unwrap(),
                        &runtime,
                        0,
                        mode
                    )
                    .is_err()
            );
            store.push(&mut window, JsValue::Int(42)).unwrap();
            assert!(
                target
                    .store(
                        &mut store.run_window(&mut window).unwrap(),
                        &runtime,
                        1,
                        mode
                    )
                    .is_err()
            );
            let old = store.slots[destination].take().unwrap();
            assert!(
                target
                    .store(
                        &mut store.run_window(&mut window).unwrap(),
                        &runtime,
                        0,
                        mode
                    )
                    .is_err()
            );
            store.slots[destination] = Some(FrameBinding::Uninitialized);
            assert!(
                target
                    .store(
                        &mut store.run_window(&mut window).unwrap(),
                        &runtime,
                        0,
                        mode
                    )
                    .unwrap()
                    .is_none()
            );
            let captured = runtime
                .new_var_ref_rooted(Value::Int(13), false, false, ClosureVariableKind::Normal)
                .unwrap();
            store.slots[destination] = Some(FrameBinding::Captured(captured.clone()));
            assert!(
                target
                    .store(
                        &mut store.run_window(&mut window).unwrap(),
                        &runtime,
                        0,
                        mode
                    )
                    .unwrap()
                    .is_none()
            );
            assert_eq!(
                runtime.read_var_ref_rooted(&captured).unwrap(),
                Value::Int(13)
            );
            store.slots[destination] = Some(old);
            let top = window.operands().start;
            store.slots[top] = Some(FrameBinding::Uninitialized);
            assert!(
                target
                    .store(
                        &mut store.run_window(&mut window).unwrap(),
                        &runtime,
                        0,
                        mode
                    )
                    .is_err()
            );
            assert!(matches!(
                store.slots[top],
                Some(FrameBinding::Uninitialized)
            ));
            assert!(matches!(
                store.slots[destination],
                Some(FrameBinding::Direct(JsValue::Int(7)))
            ));
            assert_eq!(window.depth, 1);
            store.slots[top] = Some(FrameBinding::Direct(JsValue::Int(42)));
            assert_eq!(store.pop(&mut window).unwrap(), JsValue::Int(42));
            store.clear_frame(&runtime, window).unwrap();
        }
    }
}

#[test]
fn direct_store_failed_keep_retain_does_not_replace_or_consume() {
    for target in [Target::Local, Target::Parameter] {
        let runtime = Runtime::new();
        let (mut store, mut window) = frame(&runtime);
        let stale = runtime.new_object(None).unwrap().into_handle();
        runtime.release_jsvalue(JsValue::Object(stale)).unwrap();
        store.push(&mut window, JsValue::Object(stale)).unwrap();
        assert!(
            target
                .store(
                    &mut store.run_window(&mut window).unwrap(),
                    &runtime,
                    0,
                    StoreMode::Keep,
                )
                .is_err()
        );
        assert_eq!(window.depth, 1);
        assert_eq!(store.peek(&window, 0).unwrap(), &JsValue::Object(stale));
        assert!(matches!(
            store.slots[target.offset(&window)],
            Some(FrameBinding::Direct(JsValue::Int(7)))
        ));
        // This deliberately stale diagnostic handle no longer owns an edge.
        let _stale = store.pop(&mut window).unwrap();
        store.clear_frame(&runtime, window).unwrap();
    }
}

#[test]
fn direct_store_neither_releases_displaced_owners_nor_drains_pending_releases() {
    for mode in [StoreMode::Consume, StoreMode::Keep] {
        let runtime = Runtime::new();
        let (mut store, mut window) = frame(&runtime);
        let previous = runtime.new_object(None).unwrap();
        let previous_id = previous.object_id();
        store.slots[window.locals().start] = Some(FrameBinding::Direct(JsValue::Object(
            previous.into_handle(),
        )));
        store.push(&mut window, JsValue::Int(42)).unwrap();
        let pending = runtime.new_object(None).unwrap();
        let pending_id = pending.object_id();
        {
            let state = runtime.0.state.borrow();
            runtime
                .release_jsvalue(JsValue::Object(pending.into_handle()))
                .unwrap();
            assert!(runtime.0.deferred_references.has_pending());
            let old = store
                .run_window(&mut window)
                .unwrap()
                .store_local_from_top(&runtime, 0, mode)
                .unwrap()
                .unwrap();
            assert_eq!(state.heap.object_strong_count(previous_id), Ok(1));
            assert_eq!(state.heap.object_strong_count(pending_id), Ok(1));
            assert!(runtime.0.deferred_references.has_pending());
            release_frame_binding(&runtime, old).unwrap();
            assert_eq!(state.heap.object_strong_count(previous_id), Ok(1));
        }
        drop(runtime.operation());
        assert!(runtime.0.state.borrow().heap.object(previous_id).is_err());
        assert!(runtime.0.state.borrow().heap.object(pending_id).is_err());
        assert!(!runtime.0.deferred_references.has_pending());
        store.clear_frame(&runtime, window).unwrap();
    }
}

#[test]
fn direct_store_keep_preserves_string_bigint_and_symbol_after_top_release() {
    for target in [Target::Local, Target::Parameter] {
        for source in [
            "'owned store string'",
            "123456789012345678901234567890n",
            "Symbol('store')",
        ] {
            let runtime = Runtime::new();
            let mut context = runtime.new_context();
            let expected = context.eval(source).unwrap();
            let (mut store, mut window) = frame(&runtime);
            store
                .push(&mut window, runtime.unroot_value(&expected).unwrap())
                .unwrap();
            let old = target
                .store(
                    &mut store.run_window(&mut window).unwrap(),
                    &runtime,
                    0,
                    StoreMode::Keep,
                )
                .unwrap()
                .unwrap();
            release_frame_binding(&runtime, old).unwrap();
            runtime
                .release_jsvalue(store.pop(&mut window).unwrap())
                .unwrap();
            let Some(FrameBinding::Direct(value)) = &store.slots[target.offset(&window)] else {
                panic!("store lost its direct value");
            };
            assert_eq!(runtime.root_value(value).unwrap(), expected);
            store.clear_frame(&runtime, window).unwrap();
        }
    }
}
