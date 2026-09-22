use super::*;
use crate::engine::api::Runtime;
use crate::engine::code::function::metadata::{ClosureVariableKind, VariableDefinition};
use crate::engine::code::runtime::PublishedFunctionSnapshot;
use crate::engine::vm::stack::{FrameStorage, FrameTransaction, RunSlots, StoreMode};
use std::rc::Rc;

fn frame(runtime: &Runtime, capacity: u16) -> (SlotStore, FrameWindow) {
    let context = runtime.new_context();
    let mut layout = PublishedFunctionSnapshot::empty_for_test(context.realm);
    layout.metadata.local_count = 1;
    layout.metadata.argument_count = 1;
    layout.metadata.max_stack = capacity;
    let definitions: Rc<[VariableDefinition]> = Rc::from([VariableDefinition {
        name: None,
        is_lexical: false,
        is_const: false,
        is_parameter_initializer: false,
        kind: ClosureVariableKind::Normal,
    }]);
    layout.local_definitions = Rc::clone(&definitions);
    layout.argument_definitions = definitions;
    let mut store = SlotStore::new(32);
    let window = store
        .push_frame(
            runtime,
            &layout.frame_layout(),
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

fn transaction<'a>(
    store: &'a mut SlotStore,
    window: &'a mut FrameWindow,
    cached: bool,
) -> FrameTransaction<'a> {
    if cached {
        store.frame_transaction_with_scalar_tos(window).unwrap()
    } else {
        store.frame_transaction(window).unwrap()
    }
}

fn scalar_bits(value: &JsValue) -> (u8, u64) {
    match value {
        JsValue::Undefined => (0, 0),
        JsValue::Null => (1, 0),
        JsValue::Bool(value) => (2, u64::from(*value)),
        JsValue::Int(value) => (3, *value as u64),
        JsValue::Float(value) => (4, value.to_bits()),
        JsValue::ShortBigInt(value) => (5, *value as u64),
        _ => panic!("expected scalar"),
    }
}

fn cached_top(slots: &RunSlots<'_>) -> bool {
    slots.tos.as_ref().is_some_and(|tos| tos.value.is_some())
}

#[test]
fn scalar_tos_short_borrows_share_top_and_pop_does_not_promote_backing() {
    let runtime = Runtime::new();
    for cached in [false, true] {
        let (mut store, mut window) = frame(&runtime, 2);
        {
            let mut tx = transaction(&mut store, &mut window, cached);
            tx.slots().push(JsValue::Int(11)).unwrap();
            assert_eq!(tx.peek(0).unwrap(), &JsValue::Int(11));
            {
                let mut slots = tx.slots();
                assert_eq!(cached_top(&slots), cached);
                slots.push(JsValue::Int(22)).unwrap();
                assert_eq!(slots.peek(1).unwrap(), &JsValue::Int(11));
                assert_eq!(
                    slots.store.slots[slots.window.operands().start]
                        .as_ref()
                        .map(|binding| matches!(binding, FrameBinding::Direct(JsValue::Int(11)))),
                    Some(true)
                );
                assert_eq!(slots.pop().unwrap(), JsValue::Int(22));
                assert!(!cached_top(&slots));
                assert_eq!(slots.pop().unwrap(), JsValue::Int(11));
                assert!(slots.pop().is_err());
                assert!(slots.peek(usize::MAX).is_err());
            }
            tx.slots().push(JsValue::Float(-0.0)).unwrap();
        }
        assert_eq!(
            scalar_bits(store.peek(&window, 0).unwrap()),
            (4, (-0.0f64).to_bits())
        );
        store.clear_frame(&runtime, window).unwrap();
    }
}

#[test]
fn scalar_tos_capacity_and_pending_failure_preserve_cached_owner() {
    for cached in [false, true] {
        for capacity in [0, 1] {
            let runtime = Runtime::new();
            let (mut store, mut window) = frame(&runtime, capacity);
            let object = runtime.new_object(None).unwrap();
            let id = object.object_id();
            let mut pending = Some(JsValue::Object(object.into_handle()));
            {
                let mut tx = transaction(&mut store, &mut window, cached);
                if capacity == 1 {
                    tx.slots().push(JsValue::Int(42)).unwrap();
                }
                let mut slots = tx.slots();
                assert!(slots.push(JsValue::Int(99)).is_err());
                assert!(slots.push_pending(&mut pending).is_err());
                assert!(pending.is_some());
                assert_eq!(slots.window.depth, usize::from(capacity));
                if capacity == 1 {
                    assert_eq!(slots.peek(0).unwrap(), &JsValue::Int(42));
                    assert_eq!(cached_top(&slots), cached);
                }
            }
            assert_eq!(runtime.0.state.borrow().heap.object_strong_count(id), Ok(1));
            runtime.release_jsvalue(pending.take().unwrap()).unwrap();
            store.clear_frame(&runtime, window).unwrap();
        }
    }
}

#[test]
fn scalar_tos_dirty_next_slot_fails_before_spill_or_pending_take() {
    let runtime = Runtime::new();
    let (mut store, mut window) = frame(&runtime, 2);
    // Corrupt only the future destination before the cache is admitted; the
    // cache's own reserved hole remains authenticated throughout the test.
    store.slots[window.operands().start + 1] = Some(FrameBinding::Direct(JsValue::Int(99)));
    {
        let mut tx = transaction(&mut store, &mut window, true);
        tx.slots().push(JsValue::Int(42)).unwrap();
        let mut pending = Some(JsValue::Int(8));
        let mut slots = tx.slots();
        assert!(slots.push_pending(&mut pending).is_err());
        assert_eq!(pending, Some(JsValue::Int(8)));
        assert!(cached_top(&slots));
        assert!(slots.store.slots[slots.window.operands().start].is_none());
        assert_eq!(slots.window.depth, 1);
    }
    store.slots[window.operands().start + 1] = None;
    store.clear_frame(&runtime, window).unwrap();
}

#[test]
fn scalar_tos_canonical_borrow_and_owning_push_close_the_hole() {
    let runtime = Runtime::new();
    let (mut store, mut window) = frame(&runtime, 3);
    let object = runtime.new_object(None).unwrap();
    let id = object.object_id();
    {
        let mut tx = transaction(&mut store, &mut window, true);
        tx.slots().push(JsValue::Int(1)).unwrap();
        {
            let mut slots = tx.canonical_slots("tos.spill.test_helper");
            slots.push(JsValue::Bool(true)).unwrap();
            assert!(!cached_top(&slots));
            assert!(
                slots.store.slots[slots.window.operands().start..slots.window.operands().start + 2]
                    .iter()
                    .all(Option::is_some)
            );
        }
        tx.slots()
            .push(JsValue::Object(object.into_handle()))
            .unwrap();
        assert!(!cached_top(&tx.slots()));
        tx.canonicalize("tos.spill.test_gc");
        runtime.run_gc().unwrap();
    }
    assert_eq!(runtime.0.state.borrow().heap.object_strong_count(id), Ok(1));
    store.clear_frame(&runtime, window).unwrap();
    assert!(runtime.0.state.borrow().heap.object(id).is_err());
}

#[test]
fn scalar_tos_owning_push_spills_previous_scalar_and_default_windows_stay_canonical() {
    let runtime = Runtime::new();
    let (mut store, mut window) = frame(&runtime, 3);
    let object = runtime.new_object(None).unwrap();
    let id = object.object_id();
    {
        let mut tx = transaction(&mut store, &mut window, true);
        tx.slots().push(JsValue::Float(-0.0)).unwrap();
        assert!(cached_top(&tx.slots()));
        tx.slots()
            .push(JsValue::Object(object.into_handle()))
            .unwrap();
        let slots = tx.slots();
        assert!(!cached_top(&slots));
        assert_eq!(
            scalar_bits(slots.peek(1).unwrap()),
            (4, (-0.0f64).to_bits())
        );
        assert!(
            slots.store.slots[slots.window.operands().start..slots.window.operands().start + 2]
                .iter()
                .all(Option::is_some)
        );
    }
    runtime.run_gc().unwrap();
    assert_eq!(runtime.0.state.borrow().heap.object_strong_count(id), Ok(1));
    {
        let mut tx = store.frame_transaction(&mut window).unwrap();
        tx.slots().push(JsValue::Int(9)).unwrap();
        assert!(tx.slots().tos.is_none());
        assert!(!cached_top(&tx.slots()));
    }
    {
        let mut slots = store.run_window(&mut window).unwrap();
        assert!(slots.tos.is_none());
        assert_eq!(slots.pop().unwrap(), JsValue::Int(9));
        slots.push(JsValue::Bool(false)).unwrap();
        assert!(!cached_top(&slots));
    }
    store.clear_frame(&runtime, window).unwrap();
    assert!(runtime.0.state.borrow().heap.object(id).is_err());
}

#[test]
fn scalar_tos_result_error_and_unwind_restore_the_committed_prefix() {
    for cached in [false, true] {
        for unwind in [false, true] {
            let runtime = Runtime::new();
            let (mut store, mut window) = frame(&runtime, 2);
            let result =
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> Result<(), Error> {
                    let mut tx = transaction(&mut store, &mut window, cached);
                    tx.slots().push(JsValue::Int(19))?;
                    let old = tx
                        .slots()
                        .store_local_from_top(&runtime, 0, StoreMode::Consume)?
                        .unwrap();
                    assert!(matches!(old, JsValue::Int(7)));
                    tx.slots().push(JsValue::Int(23))?;
                    if unwind {
                        panic!("committed scalar prefix");
                    }
                    Err(Error::internal("committed scalar prefix"))
                }));
            if unwind {
                assert!(result.is_err());
            } else {
                assert!(result.unwrap().is_err());
            }
            assert_eq!(window.depth, 1);
            assert_eq!(store.peek(&window, 0).unwrap(), &JsValue::Int(23));
            assert!(matches!(
                store.local(&window, 0).unwrap(),
                FrameBinding::Direct(JsValue::Int(19))
            ));
            #[cfg(feature = "profiling")]
            assert_eq!(store.live_slots, 4);
            store.clear_frame(&runtime, window).unwrap();
        }
    }
}

#[test]
fn scalar_tos_all_scalar_bits_and_shuffle_match_canonical() {
    let runtime = Runtime::new();
    let mut observations = Vec::new();
    for cached in [false, true] {
        let (mut store, mut window) = frame(&runtime, 8);
        {
            let mut tx = transaction(&mut store, &mut window, cached);
            for value in [
                JsValue::Undefined,
                JsValue::Null,
                JsValue::Bool(true),
                JsValue::Int(-7),
                JsValue::Float(-0.0),
                JsValue::Float(f64::from_bits(0x7ff8_0000_0000_0042)),
                JsValue::ShortBigInt(i64::MIN),
            ] {
                tx.slots().push(value).unwrap();
            }
            tx.slots().rotate_operands(0, 3, true).unwrap();
            tx.slots().insert_copy(&runtime, 0, 1).unwrap();
            tx.canonicalize("tos.spill.test");
            tx.canonicalize("tos.spill.test");
            let mut values = Vec::new();
            let mut slots = tx.slots();
            while slots.window.depth != 0 {
                values.push(scalar_bits(&slots.pop().unwrap()));
            }
            observations.push(values);
        }
        store.clear_frame(&runtime, window).unwrap();
    }
    assert_eq!(observations[0], observations[1]);
}

mod sequence;
mod transactions;
