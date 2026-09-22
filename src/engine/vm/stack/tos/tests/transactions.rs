use super::*;
use crate::engine::value::number::operations::Number;

#[test]
fn scalar_tos_direct_store_modes_preserve_displaced_owner_and_source() {
    for cached in [false, true] {
        for parameter in [false, true] {
            for mode in [StoreMode::Consume, StoreMode::Keep] {
                let runtime = Runtime::new();
                let (mut store, mut window) = frame(&runtime, 2);
                let object = runtime.new_object(None).unwrap();
                let id = object.object_id();
                let destination = if parameter {
                    window.parameters().start
                } else {
                    window.locals().start
                };
                store.slots[destination] =
                    Some(FrameBinding::Direct(JsValue::Object(object.into_handle())));
                let old;
                {
                    let mut tx = transaction(&mut store, &mut window, cached);
                    tx.slots().push(JsValue::Int(42)).unwrap();
                    let mut slots = tx.slots();
                    old = if parameter {
                        slots.store_parameter_from_top(&runtime, 0, mode)
                    } else {
                        slots.store_local_from_top(&runtime, 0, mode)
                    }
                    .unwrap()
                    .unwrap();
                    if matches!(mode, StoreMode::Keep) {
                        assert_eq!(slots.peek(0).unwrap(), &JsValue::Int(42));
                        assert_eq!(cached_top(&slots), cached);
                    } else {
                        assert_eq!(slots.window.depth, 0);
                        assert!(!cached_top(&slots));
                    }
                }
                assert_eq!(runtime.0.state.borrow().heap.object_strong_count(id), Ok(1));
                assert!(matches!(
                    store.slots[destination],
                    Some(FrameBinding::Direct(JsValue::Int(42)))
                ));
                runtime.release_jsvalue(old).unwrap();
                assert!(runtime.0.state.borrow().heap.object(id).is_err());
                store.clear_frame(&runtime, window).unwrap();
            }
        }
    }
}

#[test]
fn scalar_tos_store_decline_and_bad_index_leave_top_untouched() {
    for parameter in [false, true] {
        for mode in [StoreMode::Consume, StoreMode::Keep] {
            let runtime = Runtime::new();
            let (mut store, mut window) = frame(&runtime, 1);
            let destination = if parameter {
                window.parameters().start
            } else {
                window.locals().start
            };
            store.slots[destination] = Some(FrameBinding::Uninitialized);
            {
                let mut tx = transaction(&mut store, &mut window, true);
                tx.slots().push(JsValue::Int(42)).unwrap();
                let mut slots = tx.slots();
                let declined = if parameter {
                    slots.store_parameter_from_top(&runtime, 0, mode)
                } else {
                    slots.store_local_from_top(&runtime, 0, mode)
                }
                .unwrap();
                assert!(declined.is_none());
                let failed = if parameter {
                    slots.store_parameter_from_top(&runtime, u16::MAX, mode)
                } else {
                    slots.store_local_from_top(&runtime, u16::MAX, mode)
                };
                assert!(failed.is_err());
                assert!(cached_top(&slots));
                assert_eq!(slots.peek(0).unwrap(), &JsValue::Int(42));
                assert!(matches!(
                    slots.store.slots[destination],
                    Some(FrameBinding::Uninitialized)
                ));
            }
            store.clear_frame(&runtime, window).unwrap();
        }
    }
}

#[test]
fn scalar_tos_number_callbacks_panic_before_commit_and_decline_without_replay() {
    for cached in [false, true] {
        for pair in [false, true] {
            let runtime = Runtime::new();
            let (mut store, mut window) = frame(&runtime, 2);
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let mut tx = transaction(&mut store, &mut window, cached);
                tx.slots().push(JsValue::Int(4)).unwrap();
                tx.slots().push(JsValue::Float(-0.0)).unwrap();
                if pair {
                    let _ = tx
                        .slots()
                        .consume_number_pair(|_, _| panic!("number pair callback"));
                } else {
                    let _ = tx.slots().binary_number(|_, _| panic!("number callback"));
                }
            }));
            assert!(result.is_err());
            assert_eq!(window.depth, 2);
            assert_eq!(
                scalar_bits(store.peek(&window, 0).unwrap()),
                (4, (-0.0f64).to_bits())
            );
            assert_eq!(store.peek(&window, 1).unwrap(), &JsValue::Int(4));
            store.clear_frame(&runtime, window).unwrap();
        }
        let runtime = Runtime::new();
        let (mut store, mut window) = frame(&runtime, 2);
        {
            let mut tx = transaction(&mut store, &mut window, cached);
            tx.slots().push(JsValue::Int(3)).unwrap();
            tx.slots().push(JsValue::ShortBigInt(4)).unwrap();
            assert!(
                !tx.slots()
                    .binary_number(|_, _| panic!("declined callback"))
                    .unwrap()
            );
            assert_eq!(
                tx.slots()
                    .consume_number_pair(|_, _| panic!("declined comparison"))
                    .unwrap(),
                None
            );
            assert_eq!(cached_top(&tx.slots()), cached);
        }
        assert_eq!(window.depth, 2);
        store.clear_frame(&runtime, window).unwrap();
    }
}

#[test]
fn scalar_tos_number_local_callback_panic_preserves_local_and_cached_top() {
    for cached in [false, true] {
        let runtime = Runtime::new();
        let (mut store, mut window) = frame(&runtime, 2);
        let mut calls = 0;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut tx = transaction(&mut store, &mut window, cached);
            tx.slots().push(JsValue::Int(19)).unwrap();
            let _ = tx.slots().update_number_local(0, |_| {
                calls += 1;
                panic!("local callback before commit")
            });
        }));
        assert!(result.is_err());
        assert_eq!(calls, 1);
        assert_eq!(window.depth, 1);
        assert_eq!(store.peek(&window, 0).unwrap(), &JsValue::Int(19));
        assert!(matches!(
            store.local(&window, 0).unwrap(),
            FrameBinding::Direct(JsValue::Int(7))
        ));
        #[cfg(feature = "profiling")]
        assert_eq!(store.live_slots, 4);
        store.clear_frame(&runtime, window).unwrap();
    }
}

#[test]
fn scalar_tos_number_callback_owning_output_is_committed_once_to_backing() {
    for cached in [false, true] {
        for source in [
            "({tag:42})",
            "'heap string result'",
            "170141183460469231731687303715884105727n",
            "Symbol('tos')",
        ] {
            let runtime = Runtime::new();
            let mut context = runtime.new_context();
            let expected = context.eval(source).unwrap();
            let output = runtime.into_jsvalue(expected.clone()).unwrap();
            let (mut store, mut window) = frame(&runtime, 2);
            let mut calls = 0;
            {
                let mut tx = transaction(&mut store, &mut window, cached);
                tx.slots().push(JsValue::Int(1)).unwrap();
                tx.slots().push(JsValue::Int(2)).unwrap();
                assert!(
                    tx.slots()
                        .binary_number(|_, _| {
                            calls += 1;
                            output
                        })
                        .unwrap()
                );
                let slots = tx.slots();
                assert!(!cached_top(&slots));
                assert_eq!(slots.window.depth, 1);
                assert!(slots.store.slots[slots.window.operands().start].is_some());
            }
            assert_eq!(calls, 1);
            runtime.run_gc().unwrap();
            let output = store.pop(&mut window).unwrap();
            assert_eq!(runtime.root_value(&output).unwrap(), expected);
            runtime.release_jsvalue(output).unwrap();
            store.clear_frame(&runtime, window).unwrap();
        }
    }
}

#[test]
fn scalar_tos_number_local_capacity_and_compare_transactions_match_canonical() {
    for cached in [false, true] {
        let runtime = Runtime::new();
        let (mut store, mut window) = frame(&runtime, 2);
        {
            let mut tx = transaction(&mut store, &mut window, cached);
            tx.slots().push(JsValue::Int(3)).unwrap();
            tx.slots().push(JsValue::Int(4)).unwrap();
            assert!(
                tx.slots()
                    .update_number_local(0, |previous| (previous.update(true), Some(previous)))
                    .is_err()
            );
            assert!(matches!(
                tx.slots().local(0).unwrap(),
                FrameBinding::Direct(JsValue::Int(7))
            ));
            assert_eq!(cached_top(&tx.slots()), cached);
            assert!(tx.slots().binary_number(|a, b| a.add(b).into()).unwrap());
            assert_eq!(tx.peek(0).unwrap(), &JsValue::Int(7));
            assert_eq!(cached_top(&tx.slots()), cached);
            assert!(
                tx.slots()
                    .update_number_local(0, |previous| (Number::Int(8), Some(previous)))
                    .unwrap()
            );
            assert_eq!(
                tx.slots()
                    .consume_number_pair(|left, right| left.float() == right.float())
                    .unwrap(),
                Some(true)
            );
            assert_eq!(tx.slots().window.depth, 0);
            assert!(!cached_top(&tx.slots()));
            assert!(
                tx.slots()
                    .consume_number_pair(|_, _| panic!("underflow"))
                    .is_err()
            );
        }
        assert!(matches!(
            store.local(&window, 0).unwrap(),
            FrameBinding::Direct(JsValue::Int(8))
        ));
        #[cfg(feature = "profiling")]
        assert_eq!(store.live_slots, 3);
        store.clear_frame(&runtime, window).unwrap();
    }
}

#[test]
fn scalar_tos_number_pair_authenticates_both_bindings_before_decline() {
    let runtime = Runtime::new();
    let (mut store, mut window) = frame(&runtime, 2);
    {
        let mut tx = transaction(&mut store, &mut window, true);
        // Exercise the cache-aware Number facade with both inputs in backing.
        // A non-Number left must not hide a malformed right operand.
        {
            let mut slots = tx.canonical_slots("tos.spill.test_pair_setup");
            slots.push(JsValue::Bool(true)).unwrap();
            slots.push(JsValue::Int(4)).unwrap();
            let right = slots.window.operands().start + 1;
            slots.store.slots[right] = Some(FrameBinding::Uninitialized);
        }
        let mut slots = tx.slots();
        assert!(
            slots
                .binary_number(|_, _| panic!("malformed right must precede decline"))
                .is_err()
        );
        assert!(
            slots
                .consume_number_pair(|_, _| panic!("malformed right must precede decline"))
                .is_err()
        );
        assert_eq!(slots.window.depth, 2);
        assert!(!cached_top(&slots));
        let left = slots.window.operands().start;
        assert!(matches!(
            slots.store.slots[left],
            Some(FrameBinding::Direct(JsValue::Bool(true)))
        ));
        assert!(matches!(
            slots.store.slots[left + 1],
            Some(FrameBinding::Uninitialized)
        ));
        slots.store.slots[left + 1] = Some(FrameBinding::Direct(JsValue::Int(4)));
        assert!(
            !slots
                .binary_number(|_, _| panic!("non-Number pair must decline"))
                .unwrap()
        );
        assert_eq!(slots.window.depth, 2);
        slots.store.slots[left] = Some(FrameBinding::Direct(JsValue::Int(10)));
        let mut calls = 0;
        assert!(
            slots
                .binary_number(|a, b| {
                    calls += 1;
                    a.sub(b).into()
                })
                .unwrap()
        );
        assert_eq!(calls, 1);
        assert_eq!(slots.pop().unwrap(), JsValue::Int(6));
    }
    store.clear_frame(&runtime, window).unwrap();
}

#[test]
fn scalar_tos_number_pair_invalid_left_preserves_cached_right_for_retry() {
    let runtime = Runtime::new();
    let (mut store, mut window) = frame(&runtime, 2);
    {
        let mut tx = transaction(&mut store, &mut window, true);
        tx.slots().push(JsValue::Int(10)).unwrap();
        tx.slots().push(JsValue::Int(4)).unwrap();
        let mut slots = tx.slots();
        let left = slots.window.operands().start;
        slots.store.slots[left] = Some(FrameBinding::Uninitialized);
        assert!(
            slots
                .binary_number(|_, _| panic!("malformed left must not evaluate"))
                .is_err()
        );
        assert_eq!(slots.window.depth, 2);
        assert!(cached_top(&slots));
        assert_eq!(slots.peek(0).unwrap(), &JsValue::Int(4));
        assert!(slots.store.slots[left + 1].is_none());
        slots.store.slots[left] = Some(FrameBinding::Direct(JsValue::Int(10)));
        assert_eq!(
            slots
                .consume_number_pair(|a, b| a.float() > b.float())
                .unwrap(),
            Some(true)
        );
        assert_eq!(slots.window.depth, 0);
        assert!(!cached_top(&slots));
    }
    store.clear_frame(&runtime, window).unwrap();
}
