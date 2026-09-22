use super::*;
use crate::engine::api::Runtime;
use crate::engine::code::function::metadata::{ClosureVariableKind, VariableDefinition};
use crate::engine::code::runtime::PublishedFunctionSnapshot;
use crate::engine::heap::{BigIntId, StringId};
use crate::engine::value::{JsString, Value, bigint::JsBigInt};
use crate::engine::vm::stack::{FrameStorage, StoreMode};
use std::rc::Rc;

#[derive(Clone, Copy)]
enum Owner {
    String(StringId),
    BigInt(BigIntId),
}

impl Owner {
    fn alive(self, runtime: &Runtime) -> bool {
        let state = runtime.0.state.borrow();
        match self {
            Self::String(id) => state.heap.string(id).is_ok(),
            Self::BigInt(id) => state.heap.bigint(id).is_ok(),
        }
    }

    // These APIs return Some exactly at arena RC == 1. Do not mutate payloads.
    fn unique(self, runtime: &Runtime) -> bool {
        let mut state = runtime.0.state.borrow_mut();
        match self {
            Self::String(id) => state.heap.unique_string_mut(id).unwrap().is_some(),
            Self::BigInt(id) => state.heap.unique_bigint_mut(id).unwrap().is_some(),
        }
    }
}

fn output(runtime: &Runtime, bigint: bool) -> (Option<JsValue>, Owner) {
    let value = if bigint {
        Value::BigInt(JsBigInt::from(i128::MAX))
    } else {
        Value::String(JsString::from_static("owned numeric output"))
    };
    let value = runtime.into_jsvalue(value).unwrap();
    let owner = match &value {
        JsValue::String(id) => Owner::String(*id),
        JsValue::BigInt(id) => Owner::BigInt(*id),
        _ => panic!("expected an owning numeric output"),
    };
    (Some(value), owner)
}

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

#[test]
fn owned_tos_declines_canonical_transactions_and_non_numeric_owners_unchanged() {
    let runtime = Runtime::new();
    for bigint in [false, true] {
        let (mut store, mut window) = frame(&runtime, 0);
        let (mut pending, owner) = output(&runtime, bigint);
        {
            let mut tx = store.frame_transaction(&mut window).unwrap();
            assert!(!tx.cache_numeric_output(&mut pending).unwrap());
            assert!(!tx.has_owned_numeric_output());
            assert!(!tx.slots().has_owned_numeric_output());
        }
        assert!(pending.is_some());
        assert!(owner.unique(&runtime));
        runtime.release_jsvalue(pending.take().unwrap()).unwrap();
        assert!(!owner.alive(&runtime));
        store.clear_frame(&runtime, window).unwrap();
    }
    let (mut store, mut window) = frame(&runtime, 0);
    let object = runtime.new_object(None).unwrap();
    let object_id = object.object_id();
    {
        let mut tx = store
            .frame_transaction_with_scalar_tos(&mut window)
            .unwrap();
        for mut pending in [
            None,
            Some(JsValue::Undefined),
            Some(JsValue::Null),
            Some(JsValue::Bool(true)),
            Some(JsValue::Int(42)),
            Some(JsValue::Float(-0.0)),
            Some(JsValue::ShortBigInt(i64::MAX)),
            Some(JsValue::Object(object.into_handle())),
        ] {
            let expected = pending
                .as_ref()
                .map(|value| runtime.root_value(value).unwrap());
            assert!(!tx.cache_numeric_output(&mut pending).unwrap());
            assert_eq!(
                pending
                    .as_ref()
                    .map(|value| runtime.root_value(value).unwrap()),
                expected
            );
            assert_eq!(tx.slots().window.depth, 0);
            assert!(!tx.has_owned_numeric_output());
            if let Some(value) = pending {
                runtime.release_jsvalue(value).unwrap();
            }
        }
    }
    assert!(runtime.0.state.borrow().heap.object(object_id).is_err());
    store.clear_frame(&runtime, window).unwrap();
}

#[test]
fn owned_tos_ordinary_push_and_pending_push_keep_heap_values_canonical() {
    for bigint in [false, true] {
        for pending_push in [false, true] {
            let runtime = Runtime::new();
            let (mut store, mut window) = frame(&runtime, 2);
            let (mut pending, owner) = output(&runtime, bigint);
            {
                let mut tx = store
                    .frame_transaction_with_scalar_tos(&mut window)
                    .unwrap();
                tx.slots().push(JsValue::Int(42)).unwrap();
                if pending_push {
                    tx.slots().push_pending(&mut pending).unwrap();
                } else {
                    tx.slots().push(pending.take().unwrap()).unwrap();
                }
                assert!(!tx.has_owned_numeric_output());
                let slots = tx.slots();
                assert!(slots.tos.as_ref().unwrap().value.is_none());
                assert!(
                    slots.store.slots
                        [slots.window.operands().start..slots.window.operands().start + 2]
                        .iter()
                        .all(Option::is_some)
                );
            }
            assert!(owner.unique(&runtime));
            store.clear_frame(&runtime, window).unwrap();
            assert!(!owner.alive(&runtime));
        }
    }
}

#[test]
fn owned_tos_consume_moves_last_owner_to_local_or_parameter_and_returns_displaced_owner() {
    for bigint in [false, true] {
        for parameter in [false, true] {
            let runtime = Runtime::new();
            let (mut store, mut window) = frame(&runtime, 1);
            let (mut pending, owner) = output(&runtime, bigint);
            let object = runtime.new_object(None).unwrap();
            let displaced = object.object_id();
            let destination = if parameter {
                window.parameters().start
            } else {
                window.locals().start
            };
            store.slots[destination] =
                Some(FrameBinding::Direct(JsValue::Object(object.into_handle())));
            let old;
            {
                // Admission, consumption and transaction Drop must not borrow Runtime.
                // Keep the borrow alive until after the transaction is dropped.
                let _state = runtime.0.state.borrow_mut();
                let mut tx = store
                    .frame_transaction_with_scalar_tos(&mut window)
                    .unwrap();
                assert!(tx.cache_numeric_output(&mut pending).unwrap());
                assert!(pending.is_none());
                assert!(tx.has_owned_numeric_output());
                let mut slots = tx.slots();
                assert!(slots.has_owned_numeric_output());
                assert!(slots.store.slots[slots.window.operands().start].is_none());
                old = if parameter {
                    slots.store_parameter_from_top(&runtime, 0, StoreMode::Consume)
                } else {
                    slots.store_local_from_top(&runtime, 0, StoreMode::Consume)
                }
                .unwrap()
                .unwrap();
                assert_eq!(slots.window.depth, 0);
                assert!(!slots.has_owned_numeric_output());
                #[cfg(feature = "profiling")]
                assert_eq!(slots.store.live_slots, 3);
            }
            assert!(owner.unique(&runtime));
            assert_eq!(
                runtime.0.state.borrow().heap.object_strong_count(displaced),
                Ok(1)
            );
            runtime.release_jsvalue(old).unwrap();
            assert!(runtime.0.state.borrow().heap.object(displaced).is_err());
            store.clear_frame(&runtime, window).unwrap();
            assert!(!owner.alive(&runtime));
        }
    }
}

#[test]
fn owned_tos_keep_canonicalizes_before_duplicating_and_keeps_exactly_two_owners() {
    for bigint in [false, true] {
        for parameter in [false, true] {
            let runtime = Runtime::new();
            let (mut store, mut window) = frame(&runtime, 1);
            let (mut pending, owner) = output(&runtime, bigint);
            {
                let mut tx = store
                    .frame_transaction_with_scalar_tos(&mut window)
                    .unwrap();
                assert!(tx.cache_numeric_output(&mut pending).unwrap());
                let old = if parameter {
                    tx.slots()
                        .store_parameter_from_top(&runtime, 0, StoreMode::Keep)
                } else {
                    tx.slots()
                        .store_local_from_top(&runtime, 0, StoreMode::Keep)
                }
                .unwrap()
                .unwrap();
                runtime.release_jsvalue(old).unwrap();
                assert!(!tx.has_owned_numeric_output());
                assert!(!owner.unique(&runtime));
                let mut slots = tx.slots();
                assert_eq!(slots.window.depth, 1);
                assert!(slots.store.slots[slots.window.operands().start].is_some());
                runtime.release_jsvalue(slots.pop().unwrap()).unwrap();
                assert!(owner.unique(&runtime));
            }
            store.clear_frame(&runtime, window).unwrap();
            assert!(!owner.alive(&runtime));
        }
    }
}

#[test]
fn owned_tos_capacity_and_dirty_destination_errors_preserve_both_pending_and_cached_owners() {
    for bigint in [false, true] {
        for dirty_destination in [false, true] {
            let runtime = Runtime::new();
            let (mut store, mut window) = frame(&runtime, if dirty_destination { 2 } else { 1 });
            let (mut first, first_owner) = output(&runtime, bigint);
            let (mut pending, pending_owner) = output(&runtime, !bigint);
            let hole = window.operands().start;
            if dirty_destination {
                store.slots[hole + 1] = Some(FrameBinding::Direct(JsValue::Int(99)));
            }
            {
                let mut tx = store
                    .frame_transaction_with_scalar_tos(&mut window)
                    .unwrap();
                assert!(tx.cache_numeric_output(&mut first).unwrap());
                assert!(tx.cache_numeric_output(&mut pending).is_err());
                assert!(pending.is_some());
                assert!(tx.has_owned_numeric_output());
                let slots = tx.slots();
                assert_eq!(slots.window.depth, 1);
                assert!(slots.store.slots[hole].is_none());
                #[cfg(feature = "profiling")]
                assert_eq!(slots.store.live_slots, 4);
            }
            assert!(store.slots[hole].is_some());
            assert!(first_owner.unique(&runtime));
            assert!(pending_owner.unique(&runtime));
            runtime.release_jsvalue(pending.take().unwrap()).unwrap();
            assert!(!pending_owner.alive(&runtime));
            if dirty_destination {
                store.slots[hole + 1] = None;
            }
            store.clear_frame(&runtime, window).unwrap();
            assert!(!first_owner.alive(&runtime));
        }
    }
}

#[test]
fn owned_tos_next_push_restores_previous_heap_owner_before_reusing_the_cache() {
    for bigint in [false, true] {
        for next_kind in 0..3 {
            let runtime = Runtime::new();
            let (mut store, mut window) = frame(&runtime, 2);
            let (mut first, first_owner) = output(&runtime, bigint);
            let (mut second, second_owner) = output(&runtime, !bigint);
            {
                let mut tx = store
                    .frame_transaction_with_scalar_tos(&mut window)
                    .unwrap();
                assert!(tx.cache_numeric_output(&mut first).unwrap());
                match next_kind {
                    0 => tx.slots().push(JsValue::Int(42)).unwrap(),
                    1 => tx.slots().push_pending(&mut second).unwrap(),
                    _ => assert!(tx.cache_numeric_output(&mut second).unwrap()),
                }
                assert_eq!(tx.has_owned_numeric_output(), next_kind == 2);
                let slots = tx.slots();
                assert_eq!(slots.window.depth, 2);
                assert!(slots.store.slots[slots.window.operands().start].is_some());
                assert_eq!(
                    slots.store.slots[slots.window.operands().start + 1].is_none(),
                    next_kind != 1
                );
                #[cfg(feature = "profiling")]
                assert_eq!(slots.store.live_slots, 5);
            }
            assert!(first_owner.unique(&runtime));
            assert!(second_owner.unique(&runtime));
            if let Some(value) = second {
                runtime.release_jsvalue(value).unwrap();
            }
            store.clear_frame(&runtime, window).unwrap();
            assert!(!first_owner.alive(&runtime));
            assert!(!second_owner.alive(&runtime));
        }
    }
}

#[test]
fn owned_tos_consume_error_and_decline_leave_the_cached_owner_untouched() {
    for bigint in [false, true] {
        for parameter in [false, true] {
            let runtime = Runtime::new();
            let (mut store, mut window) = frame(&runtime, 1);
            let (mut pending, owner) = output(&runtime, bigint);
            let destination = if parameter {
                window.parameters().start
            } else {
                window.locals().start
            };
            store.slots[destination] = Some(FrameBinding::Uninitialized);
            {
                let mut tx = store
                    .frame_transaction_with_scalar_tos(&mut window)
                    .unwrap();
                assert!(tx.cache_numeric_output(&mut pending).unwrap());
                let mut slots = tx.slots();
                let declined = if parameter {
                    slots.store_parameter_from_top(&runtime, 0, StoreMode::Consume)
                } else {
                    slots.store_local_from_top(&runtime, 0, StoreMode::Consume)
                }
                .unwrap();
                assert!(declined.is_none());
                let failed = if parameter {
                    slots.store_parameter_from_top(&runtime, u16::MAX, StoreMode::Consume)
                } else {
                    slots.store_local_from_top(&runtime, u16::MAX, StoreMode::Consume)
                };
                assert!(failed.is_err());
                assert!(slots.has_owned_numeric_output());
                assert_eq!(slots.window.depth, 1);
                assert!(slots.store.slots[slots.window.operands().start].is_none());
            }
            assert!(owner.unique(&runtime));
            store.clear_frame(&runtime, window).unwrap();
            assert!(!owner.alive(&runtime));
        }
    }
}

#[test]
fn owned_tos_error_and_unwind_restore_last_owner_without_runtime_access() {
    for bigint in [false, true] {
        for unwind in [false, true] {
            let runtime = Runtime::new();
            let (mut store, mut window) = frame(&runtime, 2);
            let (mut pending, owner) = output(&runtime, bigint);
            let result =
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> Result<(), Error> {
                    // Declared first so the mutable heap borrow outlives tx Drop,
                    // including unwind. Restore must be a pure move into the hole.
                    let _state = runtime.0.state.borrow_mut();
                    let mut tx = store
                        .frame_transaction_with_scalar_tos(&mut window)
                        .unwrap();
                    tx.slots().push(JsValue::Int(19))?;
                    assert!(tx.cache_numeric_output(&mut pending)?);
                    assert!(tx.has_owned_numeric_output());
                    if unwind {
                        panic!("owning numeric output after committed prefix");
                    }
                    Err(Error::internal(
                        "owning numeric output after committed prefix",
                    ))
                }));
            if unwind {
                assert!(result.is_err());
            } else {
                assert!(result.unwrap().is_err());
            }
            assert!(pending.is_none());
            assert_eq!(window.depth, 2);
            assert_eq!(store.peek(&window, 1).unwrap(), &JsValue::Int(19));
            assert!(
                store.slots[window.operands().start..window.operands().start + 2]
                    .iter()
                    .all(Option::is_some)
            );
            #[cfg(feature = "profiling")]
            assert_eq!(store.live_slots, 5);
            assert!(owner.unique(&runtime));
            runtime.run_gc().unwrap();
            assert!(owner.unique(&runtime));
            store.clear_frame(&runtime, window).unwrap();
            assert!(!owner.alive(&runtime));
        }
    }
}

#[test]
fn owned_tos_shuffle_and_canonical_helper_restore_before_observing_or_releasing() {
    for bigint in [false, true] {
        let runtime = Runtime::new();
        let (mut store, mut window) = frame(&runtime, 2);
        let (mut pending, owner) = output(&runtime, bigint);
        {
            let mut tx = store
                .frame_transaction_with_scalar_tos(&mut window)
                .unwrap();
            assert!(tx.cache_numeric_output(&mut pending).unwrap());
            tx.slots().duplicate_operands(&runtime, 1).unwrap();
            assert!(!tx.has_owned_numeric_output());
            assert!(!owner.unique(&runtime));
            let value = tx.slots().pop().unwrap();
            runtime.release_jsvalue(value).unwrap();
            assert!(owner.unique(&runtime));
            let mut pending = Some(tx.slots().pop().unwrap());
            assert!(tx.cache_numeric_output(&mut pending).unwrap());
            {
                let mut slots = tx.canonical_slots("tos.spill.test_owned_helper");
                assert!(!slots.has_owned_numeric_output());
                assert!(slots.store.slots[slots.window.operands().start].is_some());
                runtime.release_jsvalue(slots.pop().unwrap()).unwrap();
                slots.push(JsValue::Int(23)).unwrap();
                assert!(slots.tos.is_none());
            }
            assert!(!tx.has_owned_numeric_output());
            assert!(!owner.alive(&runtime));
        }
        assert_eq!(store.peek(&window, 0).unwrap(), &JsValue::Int(23));
        store.clear_frame(&runtime, window).unwrap();
    }
}

#[test]
fn owned_tos_consume_release_preflight_preserves_cached_owner_for_borrowed_and_deferred_targets() {
    use crate::engine::heap::SlotReleaseReadiness;

    for bigint in [false, true] {
        for parameter in [false, true] {
            let runtime = Runtime::new();
            let (mut store, mut window) = frame(&runtime, 1);
            let (mut pending, owner) = output(&runtime, bigint);
            // Match heap/slot_ownership.rs's blocked-borrow fixture: one live
            // root, one displaced binding owner, and one queued release.
            let root = runtime.new_object(None).unwrap();
            let id = root.object_id();
            let displaced = root.try_clone().unwrap();
            let deferred = root.try_clone().unwrap();
            let destination = if parameter {
                window.parameters().start
            } else {
                window.locals().start
            };
            store.slots[destination] = Some(FrameBinding::Direct(JsValue::Object(
                displaced.into_handle(),
            )));
            let old;
            {
                let mut tx = store
                    .frame_transaction_with_scalar_tos(&mut window)
                    .unwrap();
                assert!(tx.cache_numeric_output(&mut pending).unwrap());
                // This is the run caller's displaced-release boundary, not
                // an extra runtime-dependent guard in the pure Consume move.
                let reject = |slots: &mut crate::engine::vm::stack::RunSlots<'_>, expected| {
                    let Some(FrameBinding::Direct(value)) = slots.store.slots[destination].as_mut()
                    else {
                        panic!("direct displaced owner");
                    };
                    assert_eq!(
                        runtime.slot_value_release_readiness_jsvalue(value).unwrap(),
                        expected
                    );
                    assert!(!runtime.try_release_slot_value_jsvalue(value).unwrap());
                    assert!(matches!(value, JsValue::Object(found) if *found == id));
                    assert!(slots.has_owned_numeric_output());
                    assert_eq!(slots.window.depth, 1);
                    assert!(slots.store.slots[slots.window.operands().start].is_none());
                    match (owner, slots.peek(0).unwrap()) {
                        (Owner::String(expected), JsValue::String(found)) => {
                            assert_eq!(expected, *found)
                        }
                        (Owner::BigInt(expected), JsValue::BigInt(found)) => {
                            assert_eq!(expected, *found)
                        }
                        _ => panic!("cached source changed during declined preflight"),
                    }
                    #[cfg(feature = "profiling")]
                    assert_eq!(slots.store.live_slots, 4);
                };
                {
                    let state = runtime.0.state.borrow();
                    for _ in 0..2 {
                        reject(&mut tx.slots(), SlotReleaseReadiness::Borrowed);
                        assert!(!runtime.0.deferred_references.has_pending());
                        assert_eq!(state.heap.object_strong_count(id), Ok(3));
                    }
                    drop(deferred);
                    assert_eq!(runtime.0.deferred_references.borrow().len(), 1);
                }
                for _ in 0..2 {
                    reject(&mut tx.slots(), SlotReleaseReadiness::Deferred);
                    assert_eq!(runtime.0.deferred_references.borrow().len(), 1);
                    assert_eq!(runtime.0.state.borrow().heap.object_strong_count(id), Ok(3));
                    assert!(owner.unique(&runtime));
                }
                tx.canonicalize("tos.spill.test_deferred_fallback");
                assert!(!tx.has_owned_numeric_output());
                assert_eq!(runtime.0.deferred_references.borrow().len(), 1);
                assert_eq!(tx.slots().window.depth, 1);
                runtime.drain_deferred_references().unwrap();
                assert!(!runtime.0.deferred_references.has_pending());
                assert_eq!(runtime.0.state.borrow().heap.object_strong_count(id), Ok(2));
                let mut slots = tx.slots();
                let FrameBinding::Direct(value) = slots.store.slots[destination].as_ref().unwrap()
                else {
                    panic!("displaced target changed before retry");
                };
                assert_eq!(
                    runtime.slot_value_release_readiness_jsvalue(value).unwrap(),
                    SlotReleaseReadiness::Ready
                );
                old = if parameter {
                    slots.store_parameter_from_top(&runtime, 0, StoreMode::Consume)
                } else {
                    slots.store_local_from_top(&runtime, 0, StoreMode::Consume)
                }
                .unwrap()
                .unwrap();
                assert_eq!(slots.window.depth, 0);
            }
            assert!(owner.unique(&runtime));
            runtime.release_jsvalue(old).unwrap();
            assert_eq!(runtime.0.state.borrow().heap.object_strong_count(id), Ok(1));
            drop(root);
            assert!(runtime.0.state.borrow().heap.object(id).is_err());
            store.clear_frame(&runtime, window).unwrap();
            assert!(!owner.alive(&runtime));
        }
    }
}

#[test]
fn owned_tos_symbol_admission_declines_and_ordinary_push_restores_the_heap_top() {
    for bigint in [false, true] {
        for pending_push in [false, true] {
            let runtime = Runtime::new();
            let (mut store, mut window) = frame(&runtime, 2);
            let (mut first, owner) = output(&runtime, bigint);
            let symbol = runtime.new_symbol(None).unwrap();
            let atom = symbol.atom();
            let mut symbol = Some(runtime.into_jsvalue(Value::Symbol(symbol)).unwrap());
            {
                let mut tx = store
                    .frame_transaction_with_scalar_tos(&mut window)
                    .unwrap();
                assert!(tx.cache_numeric_output(&mut first).unwrap());
                assert!(!tx.cache_numeric_output(&mut symbol).unwrap());
                assert!(matches!(symbol.as_ref(), Some(JsValue::Symbol(_))));
                assert!(tx.has_owned_numeric_output());
                assert_eq!(tx.slots().window.depth, 1);
                assert_eq!(
                    runtime
                        .0
                        .state
                        .borrow()
                        .atoms
                        .resolve(atom)
                        .unwrap()
                        .ref_count,
                    Some(1)
                );
                if pending_push {
                    tx.slots().push_pending(&mut symbol).unwrap();
                } else {
                    tx.slots().push(symbol.take().unwrap()).unwrap();
                }
                assert!(symbol.is_none());
                assert!(!tx.has_owned_numeric_output());
                let slots = tx.slots();
                assert_eq!(slots.window.depth, 2);
                assert!(slots.tos.as_ref().unwrap().value.is_none());
                assert!(matches!(slots.peek(0).unwrap(), JsValue::Symbol(_)));
                assert!(
                    slots.store.slots
                        [slots.window.operands().start..slots.window.operands().start + 2]
                        .iter()
                        .all(Option::is_some)
                );
            }
            assert!(owner.unique(&runtime));
            assert_eq!(
                runtime
                    .0
                    .state
                    .borrow()
                    .atoms
                    .resolve(atom)
                    .unwrap()
                    .ref_count,
                Some(1)
            );
            store.clear_frame(&runtime, window).unwrap();
            assert!(!owner.alive(&runtime));
            assert!(runtime.0.state.borrow().atoms.resolve(atom).is_err());
        }
    }
}
