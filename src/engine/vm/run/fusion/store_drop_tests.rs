use crate::engine::{
    api::{Runtime, Value},
    code::{
        bytecode::Instruction,
        function::metadata::{ClosureVariableKind, VariableDefinition},
        fusion::StoreDrop,
        runtime::PublishedFunctionSnapshot,
    },
    value::JsValue,
    vm::{
        bindings::FrameBinding,
        stack::{FrameStorage, FrameWindow, SlotStore, release_frame_storage},
    },
};

fn frame(runtime: &Runtime, argument: bool, binding: FrameBinding) -> (SlotStore, FrameWindow) {
    let context = runtime.new_context();
    let mut layout = PublishedFunctionSnapshot::empty_for_test(context.realm);
    layout.metadata.local_count = 1;
    layout.metadata.argument_count = 1;
    layout.metadata.max_stack = 2;
    let definition = VariableDefinition {
        name: None,
        is_lexical: false,
        is_const: false,
        is_parameter_initializer: false,
        kind: ClosureVariableKind::Normal,
    };
    layout.local_definitions = vec![definition].into();
    layout.argument_definitions = vec![definition].into();
    let (local, parameter) = if argument {
        (FrameBinding::Direct(JsValue::Int(7)), binding)
    } else {
        (binding, FrameBinding::Direct(JsValue::Int(7)))
    };
    let mut store = SlotStore::new(16);
    let window = store
        .push_frame(
            runtime,
            &layout.frame_layout(),
            FrameStorage {
                original_arguments: vec![JsValue::Int(5)],
                parameters: vec![parameter],
                locals: vec![local],
                operands: vec![],
            },
        )
        .unwrap();
    (store, window)
}

fn target(argument: bool) -> (StoreDrop, Instruction) {
    if argument {
        (StoreDrop::Argument, Instruction::SetArg(0))
    } else {
        (StoreDrop::Local, Instruction::SetLocal(0))
    }
}

fn value_state(value: &JsValue) -> String {
    match value {
        JsValue::Float(value) => format!("Float({:016x})", value.to_bits()),
        _ => format!("{value:?}"),
    }
}

fn binding_state(binding: &FrameBinding) -> String {
    match binding {
        FrameBinding::Direct(value) => value_state(value),
        FrameBinding::Captured(root) => format!("Captured({:?})", root.id()),
        FrameBinding::Uninitialized => "TDZ".into(),
        _ => panic!("unexpected fixture binding"),
    }
}

fn target_state(store: &SlotStore, window: &FrameWindow, argument: bool) -> String {
    binding_state(
        if argument {
            store.parameter(window, 0)
        } else {
            store.local(window, 0)
        }
        .unwrap(),
    )
}

fn scalar(index: usize) -> JsValue {
    match index % 7 {
        0 => JsValue::Undefined,
        1 => JsValue::Null,
        2 => JsValue::Bool(true),
        3 => JsValue::Int(i32::MAX),
        4 => JsValue::Float(-0.0),
        5 => JsValue::ShortBigInt(i64::MIN),
        _ => JsValue::Float(f64::from_bits(0x7ff8_0000_0000_0042)),
    }
}

#[test]
fn store_drop_execution_consumes_all_scalar_kinds_at_full_capacity() {
    let runtime = Runtime::new();
    for cached in [false, true] {
        for argument in [false, true] {
            for index in 0..7 {
                let value = scalar(index);
                let expected = value_state(&value);
                let (mut store, mut window) =
                    frame(&runtime, argument, FrameBinding::Direct(scalar(index + 1)));
                store.push(&mut window, JsValue::Int(99)).unwrap();
                {
                    let mut transaction = if cached {
                        store
                            .frame_transaction_with_scalar_tos(&mut window)
                            .unwrap()
                    } else {
                        store.frame_transaction(&mut window).unwrap()
                    };
                    transaction.slots().push(value).unwrap();
                    let (kind, instruction) = target(argument);
                    assert!(
                        super::store_drop(&runtime, &mut transaction.slots(), kind, &instruction)
                            .unwrap()
                    );
                }
                assert_eq!(store.depth(&window), 1);
                assert_eq!(
                    value_state(store.peek(&window, 0).unwrap()),
                    value_state(&JsValue::Int(99))
                );
                assert_eq!(target_state(&store, &window, argument), expected);
                let storage = store.take_frame(&runtime, window).unwrap();
                assert_eq!(storage.original_arguments, vec![JsValue::Int(5)]);
                assert_eq!(storage.operands, vec![JsValue::Int(99)]);
                release_frame_storage(&runtime, storage);
            }
        }
    }
}

fn assert_decline(
    runtime: &Runtime,
    argument: bool,
    cached: bool,
    binding: FrameBinding,
    source: JsValue,
) {
    let expected_source = value_state(&source);
    let expected_binding = binding_state(&binding);
    let (mut store, mut window) = frame(runtime, argument, binding);
    store.push(&mut window, JsValue::Int(99)).unwrap();
    let before = runtime.heap_counts();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        {
            let mut transaction = if cached {
                store
                    .frame_transaction_with_scalar_tos(&mut window)
                    .unwrap()
            } else {
                store.frame_transaction(&mut window).unwrap()
            };
            transaction.slots().push(source).unwrap();
            let (kind, instruction) = target(argument);
            // A second attempt must see the same inputs, proving decline did
            // not consume the source or partly replace a target.
            for _ in 0..2 {
                assert!(
                    !super::store_drop(runtime, &mut transaction.slots(), kind, &instruction)
                        .unwrap()
                );
                assert_eq!(value_state(transaction.peek(0).unwrap()), expected_source);
            }
        }
        assert_eq!(runtime.heap_counts(), before);
        assert_eq!(store.depth(&window), 2);
        assert_eq!(
            value_state(store.peek(&window, 0).unwrap()),
            expected_source
        );
        assert_eq!(
            value_state(store.peek(&window, 1).unwrap()),
            value_state(&JsValue::Int(99))
        );
        assert_eq!(target_state(&store, &window, argument), expected_binding);
    }));
    // SlotStore carries manual JsValue edges. Release them even when an
    // assertion fails, so teardown cannot replace the original diagnostic.
    store.clear_frame(runtime, window).unwrap();
    if let Err(payload) = result {
        std::panic::resume_unwind(payload);
    }
}

#[test]
fn store_drop_execution_declines_heap_source_or_displaced_owner_unchanged() {
    for source in [
        "({tag:42})",
        "'heap store/drop string'",
        "1361129467683753853853498429727072845824n",
        "Symbol('store/drop')",
    ] {
        for argument in [false, true] {
            for cached in [false, true] {
                for heap_destination in [false, true] {
                    let runtime = Runtime::new();
                    let mut context = runtime.new_context();
                    let heap = runtime.into_jsvalue(context.eval(source).unwrap()).unwrap();
                    let (binding, value) = if heap_destination {
                        (FrameBinding::Direct(heap), JsValue::Int(42))
                    } else {
                        (FrameBinding::Direct(JsValue::Int(7)), heap)
                    };
                    assert_decline(&runtime, argument, cached, binding, value);
                }
            }
        }
    }
}

#[test]
fn store_drop_execution_declines_captured_and_tdz_without_mutation() {
    for argument in [false, true] {
        for cached in [false, true] {
            let runtime = Runtime::new();
            assert_decline(
                &runtime,
                argument,
                cached,
                FrameBinding::Uninitialized,
                JsValue::Int(42),
            );
            let root = runtime
                .new_var_ref_rooted(Value::Int(13), true, false, ClosureVariableKind::Normal)
                .unwrap();
            assert_decline(
                &runtime,
                argument,
                cached,
                FrameBinding::Captured(root.clone()),
                JsValue::Int(42),
            );
            assert_eq!(runtime.read_var_ref_rooted(&root).unwrap(), Value::Int(13));
            assert_eq!(
                runtime
                    .0
                    .state
                    .borrow()
                    .heap
                    .var_ref_strong_count(root.id()),
                Ok(1)
            );
        }
    }
}

#[test]
fn store_drop_execution_bad_certificate_and_index_keep_original_operands() {
    let runtime = Runtime::new();
    let (mut store, mut window) = frame(&runtime, false, FrameBinding::Direct(JsValue::Int(7)));
    store.push(&mut window, JsValue::Int(42)).unwrap();
    {
        let mut transaction = store
            .frame_transaction_with_scalar_tos(&mut window)
            .unwrap();
        for (kind, instruction) in [
            (StoreDrop::Local, Instruction::SetArg(0)),
            (StoreDrop::Argument, Instruction::SetLocal(0)),
            (StoreDrop::Local, Instruction::SetLocal(u16::MAX)),
            (StoreDrop::Argument, Instruction::SetArg(u16::MAX)),
        ] {
            assert!(
                super::store_drop(&runtime, &mut transaction.slots(), kind, &instruction).is_err()
            );
            assert_eq!(transaction.peek(0).unwrap(), &JsValue::Int(42));
        }
    }
    assert_eq!(store.depth(&window), 1);
    assert_eq!(
        target_state(&store, &window, false),
        value_state(&JsValue::Int(7))
    );
    store.clear_frame(&runtime, window).unwrap();
}

#[test]
fn store_drop_execution_decline_keeps_real_error_line_and_evaluation_order() {
    for source in [
        "(function storeDropTdz(){\nvar calls=0;\ntry{\nx=(calls++,42);\nlet x;\n}catch(e){return JSON.stringify([calls,e.name,e.stack])}\n})()",
        "(function storeDropConst(){\nvar calls=0;const x=1;\ntry{\nx=(calls++,42);\n}catch(e){return JSON.stringify([calls,x,e.name,e.stack])}\n})()",
    ] {
        let mut baseline = None;
        for mode in [0, 8, 15] {
            let runtime = Runtime::new();
            runtime.0.execution_mode_override.set(Some(mode));
            let mut context = runtime.new_context();
            let Value::String(result) = context
                .eval_with_filename(source, "store-drop-error.js")
                .unwrap()
            else {
                panic!("error observation");
            };
            let result = result.to_string();
            assert!(result.starts_with("[1,"), "RHS must execute once: {result}");
            assert!(result.contains("store-drop-error.js:4"), "{result}");
            if let Some(expected) = &baseline {
                assert_eq!(&result, expected, "mode {mode}");
            } else {
                baseline = Some(result);
            }
            assert!(runtime.0.state.borrow().active_frames.is_empty());
        }
    }
}
