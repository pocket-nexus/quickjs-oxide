//! Actual resident producers carry one fact until an owned completion is ready.
use super::*;
use crate::engine::{
    builtins::{
        MathKind, MathStep, NumericStep, ScalarTextStep,
        continuation::NativeStep,
        native::{
            BigIntAsNKind, DateNativeKind, DateStringMethod, MathUnaryKind, NumberFormatKind,
            PrimitiveKind,
        },
        primitive::{numeric::NumericKind, text::ScalarTextKind},
    },
    code::exec_opcode::Opcode,
    heap::{GcPolicy, RawId, runtime::RuntimeState},
    value::{
        JsString,
        conversion::{
            BigIntPrimitiveStep, IndexPrimitiveStep, NumberPrimitiveStep, StringPrimitiveStep,
            number::NumberStep,
        },
    },
    vm::{
        call::{NativeArguments, NativeInvocation, NativeInvocationAdaptation, NativeInvokeMode},
        execute::{VmAction, execute_frame_in_state},
        property_driver::read_completion_tests::read_fixture,
    },
};

fn unreachable_cycle(
    runtime: &Runtime,
    context: &mut crate::engine::api::Context,
) -> crate::engine::heap::ObjectId {
    let Value::Object(cycle) = context
        .eval("(()=>{let o={};o.self=o;return o})()")
        .unwrap()
    else {
        panic!("cycle")
    };
    let id = cycle.into_handle();
    runtime.release_jsvalue(JsValue::Object(id)).unwrap();
    id
}
fn drop_completion(state: &mut RuntimeState, runtime: &Runtime, completion: Completion) {
    let (Completion::Return(value) | Completion::Throw(value)) = completion;
    state
        .release_owned_jsvalue(&runtime.0.poisoned, value)
        .unwrap();
}
fn numeric_result(state: &mut RuntimeState, runtime: &Runtime, step: NumericStep) {
    let NumericStep::CyclePublished(Completion::Throw(value)) = step else {
        panic!("fresh numeric diagnostic fact")
    };
    let JsValue::Object(object) = value else {
        panic!("Error")
    };
    assert!(state.heap.object(object).is_ok());
    state
        .release_owned_jsvalue(&runtime.0.poisoned, value)
        .unwrap();
}

#[test]
fn direct_number_and_number_step_tag_symbol_and_bigint_factory_errors() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let symbol = runtime.new_symbol(None).unwrap();
    let mut state = runtime.0.state.borrow_mut();
    let index = state.atoms.unbrand(symbol.atom()).unwrap();
    for value in [JsValue::Symbol(index), JsValue::ShortBigInt(1)] {
        let NumberPrimitiveStep::CyclePublishedThrow(error) = state
            .number_from_primitive_jsvalue_with_publication(
                &runtime.0.poisoned,
                context.realm,
                &value,
            )
            .unwrap()
        else {
            panic!("fresh ToNumber Error")
        };
        state
            .release_owned_jsvalue(&runtime.0.poisoned, error)
            .unwrap();
        let value = state.dup_jsvalue(&value).unwrap();
        let NumberStep::CyclePublished(NativeConversion::Throw(error)) =
            NumberStep::start_jsvalue_in_state(
                &mut state,
                &runtime.0.poisoned,
                context.realm,
                value,
            )
            .unwrap()
        else {
            panic!("NumberStep must preserve the direct factory fact")
        };
        state
            .release_owned_jsvalue(&runtime.0.poisoned, error)
            .unwrap();
    }
    assert!(matches!(
        state
            .number_from_primitive_jsvalue_with_publication(
                &runtime.0.poisoned,
                context.realm,
                &JsValue::Int(42)
            )
            .unwrap(),
        NumberPrimitiveStep::Value(value) if value == 42.0
    ));
    assert_eq!(
        state.atoms.resolve(symbol.atom()).unwrap().ref_count,
        Some(1)
    );
    assert!(!runtime.is_poisoned());
    drop(state);
    // Every legacy local NumberStep consumer keeps its old Complete ABI.
    for source in [
        "isNaN(Symbol())",
        "parseInt('1', Symbol())",
        "Array.prototype.push.call({length:Symbol()}, 1)",
        "Array.prototype.slice.call({length:Symbol()})",
        "Array.prototype.values.call({length:Symbol()}).next()",
    ] {
        assert_eq!(
            context.eval(source),
            Err(crate::engine::api::runtime_error::RuntimeError::Exception),
            "{source}"
        );
    }
}

#[test]
fn bigint_parse_type_and_index_range_sites_tag_only_their_own_errors() {
    let runtime = Runtime::new();
    let context = runtime.new_context().unwrap();
    let mut state = runtime.0.state.borrow_mut();
    let BigIntPrimitiveStep::CyclePublishedThrow(error) = state
        .native_bigint_from_string_with_publication(
            &runtime.0.poisoned,
            context.realm,
            &JsString::from_static("invalid!"),
        )
        .unwrap()
    else {
        panic!("fresh SyntaxError")
    };
    state
        .release_owned_jsvalue(&runtime.0.poisoned, error)
        .unwrap();
    let BigIntPrimitiveStep::CyclePublishedThrow(error) = state
        .bigint_from_primitive_jsvalue_with_publication(
            &runtime.0.poisoned,
            context.realm,
            &JsValue::Null,
        )
        .unwrap()
    else {
        panic!("fresh TypeError")
    };
    state
        .release_owned_jsvalue(&runtime.0.poisoned, error)
        .unwrap();
    let IndexPrimitiveStep::CyclePublishedThrow(error) = state
        .index_from_number_with_publication(&runtime.0.poisoned, context.realm, -1.0)
        .unwrap()
    else {
        panic!("fresh RangeError")
    };
    state
        .release_owned_jsvalue(&runtime.0.poisoned, error)
        .unwrap();
    assert!(
        matches!(state.native_bigint_from_string_with_publication(&runtime.0.poisoned, context.realm, &JsString::from_static("42")).unwrap(), BigIntPrimitiveStep::Value(value) if value.as_i64() == Some(42))
    );
    assert!(matches!(
        state
            .index_from_number_with_publication(&runtime.0.poisoned, context.realm, 2.0)
            .unwrap(),
        IndexPrimitiveStep::Value(2)
    ));
}

#[test]
fn string_suffix_preserves_propagated_throw_without_new_publication() {
    let runtime = Runtime::new();
    let context = runtime.new_context().unwrap();
    let symbol = runtime.new_symbol(None).unwrap();
    let original = runtime.new_object(None).unwrap().into_handle();
    let mut state = runtime.0.state.borrow_mut();
    let index = state.atoms.unbrand(symbol.atom()).unwrap();
    let value = state.dup_jsvalue(&JsValue::Symbol(index)).unwrap();
    let StringPrimitiveStep::CyclePublishedThrow(error) = state
        .finish_string_value_with_publication(
            &runtime.0.poisoned,
            context.realm,
            Completion::Return(value),
        )
        .unwrap()
    else {
        panic!("fresh Symbol ToString diagnostic")
    };
    state
        .release_owned_jsvalue(&runtime.0.poisoned, error)
        .unwrap();
    let count = state.heap.counts().object_nodes;
    let StringPrimitiveStep::Throw(JsValue::Object(error)) = state
        .finish_string_value_with_publication(
            &runtime.0.poisoned,
            context.realm,
            Completion::Throw(JsValue::Object(original)),
        )
        .unwrap()
    else {
        panic!("propagated throw")
    };
    assert_eq!(error, original);
    assert_eq!(state.heap.counts().object_nodes, count);
    state
        .release_owned_jsvalue(&runtime.0.poisoned, JsValue::Object(error))
        .unwrap();
    assert_eq!(
        state.atoms.resolve(symbol.atom()).unwrap().ref_count,
        Some(1)
    );
}

#[test]
fn math_immediate_diagnostic_and_pure_completion_keep_distinct_output_paths() {
    let runtime = Runtime::new();
    let context = runtime.new_context().unwrap();
    let mut state = runtime.0.state.borrow_mut();
    let invocation = NativeInvocation::Call {
        this_value: JsValue::Undefined,
    };
    let arguments = NativeArguments {
        actual_arg_count: 1,
        readable: vec![JsValue::ShortBigInt(1)],
    };
    let MathStep::CyclePublished(completion @ Completion::Throw(_)) = MathStep::start_in_state(
        &mut state,
        &runtime.0.poisoned,
        context.realm,
        MathKind::Unary(MathUnaryKind::Abs),
        &invocation,
        &arguments,
    )
    .unwrap() else {
        panic!("Math prefix factory fact")
    };
    drop_completion(&mut state, &runtime, completion);
    let arguments = NativeArguments {
        actual_arg_count: 1,
        readable: vec![JsValue::Int(-42)],
    };
    assert!(matches!(
        MathStep::start_in_state(
            &mut state,
            &runtime.0.poisoned,
            context.realm,
            MathKind::Unary(MathUnaryKind::Abs),
            &invocation,
            &arguments
        )
        .unwrap(),
        MathStep::Complete(Completion::Return(JsValue::Int(42)))
    ));
}

#[test]
fn numeric_brand_radix_digits_and_width_fact_survive_domain_retirement() {
    let runtime = Runtime::new();
    let context = runtime.new_context().unwrap();
    let mut state = runtime.0.state.borrow_mut();
    let invocation = NativeInvocation::Call {
        this_value: JsValue::Undefined,
    };
    let arguments = NativeArguments {
        actual_arg_count: 1,
        readable: vec![JsValue::Undefined],
    };
    let step = NumericStep::start_in_state(
        &mut state,
        &runtime.0.poisoned,
        context.realm,
        NumericKind::ToString(PrimitiveKind::Number),
        &invocation,
        &arguments,
    )
    .unwrap();
    numeric_result(&mut state, &runtime, step);
    for (kind, argument) in [
        (NumericKind::ToString(PrimitiveKind::Number), 1),
        (NumericKind::Format(NumberFormatKind::Fixed), 101),
    ] {
        let arguments = NativeArguments {
            actual_arg_count: 1,
            readable: vec![JsValue::Int(argument)],
        };
        let NumericStep::Number { value, resume } = NumericStep::start_in_state(
            &mut state,
            &runtime.0.poisoned,
            context.realm,
            kind,
            &NativeInvocation::Call {
                this_value: JsValue::Int(42),
            },
            &arguments,
        )
        .unwrap() else {
            panic!("ordered numeric child")
        };
        state
            .release_owned_jsvalue(&runtime.0.poisoned, value)
            .unwrap();
        let step = resume
            .number_in_state(
                &mut state,
                &runtime.0.poisoned,
                NativeConversion::Value(f64::from(argument)),
            )
            .unwrap();
        numeric_result(&mut state, &runtime, step);
    }
    let arguments = NativeArguments {
        actual_arg_count: 2,
        readable: vec![JsValue::Int(-1), JsValue::ShortBigInt(1)],
    };
    let NumericStep::Number { value, resume } = NumericStep::start_in_state(
        &mut state,
        &runtime.0.poisoned,
        context.realm,
        NumericKind::BigIntAsN(BigIntAsNKind::AsIntN),
        &invocation,
        &arguments,
    )
    .unwrap() else {
        panic!("width child")
    };
    state
        .release_owned_jsvalue(&runtime.0.poisoned, value)
        .unwrap();
    let step = resume
        .number_in_state(
            &mut state,
            &runtime.0.poisoned,
            NativeConversion::Value(-1.0),
        )
        .unwrap();
    numeric_result(&mut state, &runtime, step);
}

#[test]
fn numeric_bigint_parse_and_allocation_failure_keep_the_factory_fact() {
    let runtime = Runtime::new();
    let context = runtime.new_context().unwrap();
    let mut state = runtime.0.state.borrow_mut();
    let invalid = state
        .heap
        .allocate_string(JsString::from_static("invalid!"))
        .unwrap();
    // Upstream returns short -1 unchanged at large widths. The actual
    // allocation guard needs its permitted extra sign limb, then truncation
    // one bit beyond the nominal limit but below the stored width.
    use crate::engine::value::bigint::{JsBigInt, MAX_BIGINT_BITS};
    let extended = JsBigInt::one()
        .shl(&JsBigInt::from(MAX_BIGINT_BITS - 1))
        .unwrap();
    let extended = state.heap.allocate_bigint(extended).unwrap();
    for (input, width) in [
        (JsValue::String(invalid), 3.0),
        (JsValue::BigInt(extended), (MAX_BIGINT_BITS + 1) as f64),
    ] {
        let arguments = NativeArguments {
            actual_arg_count: 2,
            readable: vec![JsValue::Int(3), input],
        };
        let NumericStep::Number { value, resume } = NumericStep::start_in_state(
            &mut state,
            &runtime.0.poisoned,
            context.realm,
            NumericKind::BigIntAsN(BigIntAsNKind::AsUintN),
            &NativeInvocation::Call {
                this_value: JsValue::Undefined,
            },
            &arguments,
        )
        .unwrap() else {
            panic!("width child")
        };
        state
            .release_owned_jsvalue(&runtime.0.poisoned, value)
            .unwrap();
        let NumericStep::Primitive { value, resume } = resume
            .number_in_state(
                &mut state,
                &runtime.0.poisoned,
                NativeConversion::Value(width),
            )
            .unwrap()
        else {
            panic!("BigInt primitive child")
        };
        let step = resume
            .primitive_in_state(&mut state, &runtime.0.poisoned, Completion::Return(value))
            .unwrap();
        numeric_result(&mut state, &runtime, step);
        // The manual invocation retains its original readable inputs; the
        // conversion children consumed only the checked independent snapshots.
        for value in arguments.readable {
            state
                .release_owned_jsvalue(&runtime.0.poisoned, value)
                .unwrap();
        }
    }
}

#[test]
fn pure_primitive_brand_and_readonly_date_brand_iso_outputs_are_tagged() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let Value::Object(date) = context.eval("new Date(NaN)").unwrap() else {
        panic!("Date")
    };
    let mut state = runtime.0.state.borrow_mut();
    let invocation = NativeInvocation::Call {
        this_value: JsValue::Undefined,
    };
    let value_of = state
        .call_primitive_prototype_value_of(
            &runtime.0.poisoned,
            context.realm,
            PrimitiveKind::Number,
            &invocation,
        )
        .unwrap();
    let description = state
        .call_symbol_prototype_description(
            &runtime.0.poisoned,
            context.realm,
            &NativeInvocation::Getter {
                this_value: JsValue::Null,
            },
        )
        .unwrap();
    let brand = state
        .call_date_readonly_native_with_publication(
            &runtime.0.poisoned,
            runtime.0.host_services.as_ref(),
            context.realm,
            DateNativeKind::TimeValue,
            &invocation,
        )
        .unwrap();
    let iso = state
        .call_date_readonly_native_with_publication(
            &runtime.0.poisoned,
            runtime.0.host_services.as_ref(),
            context.realm,
            DateNativeKind::String(DateStringMethod::IsoString),
            &NativeInvocation::Call {
                this_value: JsValue::Object(date.object_id()),
            },
        )
        .unwrap();
    for step in [value_of, description, brand, iso] {
        let NativeStep::CyclePublishedComplete(completion @ Completion::Throw(_)) = step else {
            panic!("actual pure factory fact")
        };
        drop_completion(&mut state, &runtime, completion);
    }
    assert_eq!(state.heap.object_strong_count(date.object_id()), Ok(1));
}

#[test]
fn scalar_text_nullish_and_conversion_fact_preserve_leaf_completion_distinction() {
    let runtime = Runtime::new();
    let context = runtime.new_context().unwrap();
    let symbol = runtime.new_symbol(None).unwrap();
    let mut state = runtime.0.state.borrow_mut();
    let symbol = state.atoms.unbrand(symbol.atom()).unwrap();
    let arguments = NativeArguments {
        actual_arg_count: 1,
        readable: vec![JsValue::Symbol(symbol)],
    };
    for this_value in [JsValue::Null, JsValue::Symbol(symbol)] {
        let ScalarTextStep::CyclePublished(completion @ Completion::Throw(_)) =
            ScalarTextStep::start_in_state(
                &mut state,
                &runtime.0.poisoned,
                context.realm,
                ScalarTextKind::CharCodeAt,
                &NativeInvocation::Call { this_value },
                &arguments,
            )
            .unwrap()
        else {
            panic!("actual scalar factory fact")
        };
        drop_completion(&mut state, &runtime, completion);
    }
    let input = state
        .heap
        .allocate_string(JsString::from_static("abc"))
        .unwrap();
    let ScalarTextStep::CyclePublished(completion @ Completion::Throw(_)) =
        ScalarTextStep::start_in_state(
            &mut state,
            &runtime.0.poisoned,
            context.realm,
            ScalarTextKind::CharCodeAt,
            &NativeInvocation::Call {
                this_value: JsValue::String(input),
            },
            &arguments,
        )
        .unwrap()
    else {
        panic!("actual direct Number error fact")
    };
    drop_completion(&mut state, &runtime, completion);
    let arguments = NativeArguments {
        actual_arg_count: 1,
        readable: vec![JsValue::Int(0)],
    };
    let step = ScalarTextStep::start_in_state(
        &mut state,
        &runtime.0.poisoned,
        context.realm,
        ScalarTextKind::CharAt(crate::engine::builtins::native::StringCharAtKind::CharAt),
        &NativeInvocation::Call {
            this_value: JsValue::String(input),
        },
        &arguments,
    )
    .unwrap();
    assert!(matches!(
        step,
        ScalarTextStep::Complete(Completion::Return(JsValue::String(_)))
    ));
    step.retire_in_state(&mut state, &runtime.0.poisoned)
        .unwrap();
    state
        .release_owned_jsvalue(&runtime.0.poisoned, JsValue::String(input))
        .unwrap();
}

#[test]
fn scalar_iterator_immediate_and_resumed_objects_publish_after_suffix_retirement() {
    let runtime = Runtime::new();
    let context = runtime.new_context().unwrap();
    let receiver = runtime.new_object(None).unwrap();
    let marker = runtime.new_object(None).unwrap();
    let mut state = runtime.0.state.borrow_mut();
    let input = state
        .heap
        .allocate_string(JsString::from_static("abc"))
        .unwrap();
    let arguments = NativeArguments {
        actual_arg_count: 1,
        readable: vec![JsValue::Object(marker.object_id())],
    };
    let direct = ScalarTextStep::start_in_state(
        &mut state,
        &runtime.0.poisoned,
        context.realm,
        ScalarTextKind::Iterator,
        &NativeInvocation::Call {
            this_value: JsValue::String(input),
        },
        &arguments,
    )
    .unwrap();
    let ScalarTextStep::CyclePublished(Completion::Return(JsValue::Object(first))) = direct else {
        panic!("immediate iterator fact")
    };
    assert_eq!(state.heap.object_strong_count(marker.object_id()), Ok(1));
    let ScalarTextStep::String { value, resume } = ScalarTextStep::start_in_state(
        &mut state,
        &runtime.0.poisoned,
        context.realm,
        ScalarTextKind::Iterator,
        &NativeInvocation::Call {
            this_value: JsValue::Object(receiver.object_id()),
        },
        &arguments,
    )
    .unwrap() else {
        panic!("source ToString")
    };
    state
        .release_owned_jsvalue(&runtime.0.poisoned, value)
        .unwrap();
    assert_eq!(state.heap.object_strong_count(marker.object_id()), Ok(2));
    let ScalarTextStep::CyclePublished(Completion::Return(JsValue::Object(second))) = resume
        .string_in_state(
            &mut state,
            &runtime.0.poisoned,
            NativeConversion::Value(JsString::from_static("xyz")),
        )
        .unwrap()
    else {
        panic!("resumed iterator fact")
    };
    assert_eq!(state.heap.object_strong_count(marker.object_id()), Ok(1));
    assert!(state.heap.object(first).is_ok() && state.heap.object(second).is_ok());
    for value in [
        JsValue::String(input),
        JsValue::Object(first),
        JsValue::Object(second),
    ] {
        state
            .release_owned_jsvalue(&runtime.0.poisoned, value)
            .unwrap();
    }
}

#[test]
fn tagged_native_and_string_reply_fact_is_consumed_once_with_live_raw_error_owner() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    runtime.set_gc_policy(GcPolicy::Manual).unwrap();
    let cycle = unreachable_cycle(&runtime, &mut context);
    let mut storage = QueryStorage::default();
    let mut query = storage.acquire(context.realm, Vec::new(), Finish::Root);
    let mut state = runtime.0.state.borrow_mut();
    let error = state
        .new_native_error_from_message(
            &runtime.0.poisoned,
            context.realm,
            crate::engine::api::error::NativeErrorKind::Type,
            crate::engine::api::error::NativeErrorMessage::from_utf8("live"),
        )
        .unwrap();
    let mut step = Step::try_from(NativeStep::CyclePublishedComplete(Completion::Throw(
        JsValue::Object(error),
    )))
    .unwrap();
    assert!(step.has_raw_owner());
    assert!(
        query
            .advance_raw_in_state(&runtime, &mut state, &mut step)
            .unwrap()
            .cycle_published
    );
    assert!(
        !query
            .advance_raw_in_state(&runtime, &mut state, &mut step)
            .unwrap()
            .cycle_published
    );
    runtime.0.gc_pressure.policy.set(GcPolicy::Automatic);
    runtime.0.gc_pressure.remaining.set(0);
    state
        .collect_if_requested(&runtime.0.gc_pressure, &runtime.0.poisoned)
        .unwrap();
    assert!(state.heap.object(error).is_ok());
    assert!(state.heap.object(cycle).is_err());
    step.retire_raw_in_state(&mut state, &runtime.0.poisoned)
        .unwrap();
    let error = state
        .new_native_error_from_message(
            &runtime.0.poisoned,
            context.realm,
            crate::engine::api::error::NativeErrorKind::Type,
            crate::engine::api::error::NativeErrorMessage::from_utf8("reply"),
        )
        .unwrap();
    let mut step = Step::CyclePublishedStringReply {
        value: Some(NativeConversion::Throw(JsValue::Object(error))),
        resume: Some(Resume::Identity),
    };
    assert!(step.has_raw_owner());
    assert!(
        query
            .advance_raw_in_state(&runtime, &mut state, &mut step)
            .unwrap()
            .cycle_published
    );
    assert!(
        matches!(step, Step::StringReply { value: Some(NativeConversion::Throw(JsValue::Object(id))), .. } if id == error)
    );
    step.retire_raw_in_state(&mut state, &runtime.0.poisoned)
        .unwrap();
}

fn named_get(
    runtime: &Runtime,
    context: &mut crate::engine::api::Context,
    source: &str,
) -> (RunningExecution, FrameId, usize) {
    let input = runtime.into_jsvalue(context.eval(source).unwrap()).unwrap();
    let (mut execution, id) = read_fixture(
        runtime,
        context,
        "(function(o){return o.publicationValue})",
        Opcode::GetFieldCached,
    );
    let frame = execution.frames.current_mut(id).unwrap();
    execution.slots.push(&mut frame.window, input).unwrap();
    let pc = frame.resume_pc;
    (execution, id, pc)
}

#[test]
fn actual_resident_error_getter_services_pressure_with_throw_owner_and_fault_pc_live() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    runtime.set_gc_policy(GcPolicy::Manual).unwrap();
    let cycle = unreachable_cycle(&runtime, &mut context);
    let (mut execution, id, pc) = named_get(
        &runtime,
        &mut context,
        "Object.defineProperty({},'publicationValue',{get:Number.prototype.valueOf})",
    );
    runtime.0.gc_pressure.policy.set(GcPolicy::Automatic);
    runtime.0.gc_pressure.remaining.set(0);
    let mut state = runtime.0.state.borrow_mut();
    assert!(matches!(
        execute_frame_in_state(&runtime, &mut state, &mut execution, id).unwrap(),
        VmAction::Throw
    ));
    let frame = execution.frames.current_mut(id).unwrap();
    assert_eq!((frame.fault_pc, frame.resume_pc), (pc, pc));
    let JsValue::Object(error) = execution.slots.peek(&frame.window, 0).unwrap() else {
        panic!("thrown Error owner")
    };
    assert!(state.heap.object(*error).is_ok());
    assert!(state.heap.object(cycle).is_err());
    assert!(!runtime.0.gc_pressure.requested());
}

#[test]
fn actual_immediate_and_callback_string_iterators_service_pressure_with_result_live() {
    for resumed in [false, true] {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        runtime.set_gc_policy(GcPolicy::Manual).unwrap();
        let cycle = unreachable_cycle(&runtime, &mut context);
        let source = if resumed {
            "Object.defineProperty({toString(){return 'xyz'}},'publicationValue',{get:String.prototype[Symbol.iterator]})"
        } else {
            "(()=>{Object.defineProperty(String.prototype,'publicationValue',{get:String.prototype[Symbol.iterator]});return 'abc'})()"
        };
        let (mut execution, id, _) = named_get(&runtime, &mut context, source);
        runtime.0.gc_pressure.policy.set(GcPolicy::Automatic);
        runtime.0.gc_pressure.remaining.set(0);
        let mut state = runtime.0.state.borrow_mut();
        assert!(matches!(
            execute_frame_in_state(&runtime, &mut state, &mut execution, id).unwrap(),
            VmAction::Complete
        ));
        let Some(JsValue::Object(iterator)) = execution.pending.as_ref() else {
            panic!("iterator result owner")
        };
        assert!(state.heap.object(*iterator).is_ok());
        assert!(state.heap.object(cycle).is_err());
        assert!(!runtime.0.gc_pressure.requested());
        assert!(!runtime.is_poisoned());
    }
}

#[test]
fn nonallocating_resident_native_getter_does_not_service_unrelated_pressure() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    runtime.set_gc_policy(GcPolicy::Manual).unwrap();
    let cycle = unreachable_cycle(&runtime, &mut context);
    let (mut execution, id, _) = named_get(
        &runtime,
        &mut context,
        "Object.defineProperty({},'publicationValue',{get:Number.isFinite})",
    );
    runtime.0.gc_pressure.policy.set(GcPolicy::Automatic);
    runtime.0.gc_pressure.remaining.set(0);
    let mut state = runtime.0.state.borrow_mut();
    assert!(matches!(
        execute_frame_in_state(&runtime, &mut state, &mut execution, id).unwrap(),
        VmAction::Complete
    ));
    assert_eq!(execution.pending, Some(JsValue::Bool(false)));
    assert!(state.heap.object(cycle).is_ok());
    assert!(runtime.0.gc_pressure.requested());
}

#[test]
fn actual_catch_and_iterator_loops_keep_owned_errors_and_iterators_alive() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    assert_eq!(context.eval("(()=>{let count=0,first;for(let i=0;i<32;i++){try{Math.abs(1n)}catch(e){if(!first)first=e;if(!(e instanceof TypeError))return false;count++}}let text='';for(let i=0;i<32;i++){let iterator='xy'[Symbol.iterator]();text+=iterator.next().value}return count===32 && first.message.length>0 && text.length===32})()").unwrap(), Value::Bool(true));
    assert!(!runtime.is_poisoned());
}

#[test]
fn constructor_adaptation_tags_only_successful_error_factory_after_input_retirement() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let callable = runtime
        .callable_from_value(context.eval("Map").unwrap())
        .unwrap();
    let CallableExecution::Native {
        target,
        realm,
        min_readable_args,
    } = runtime.bytecode_for_callable(&callable).unwrap()
    else {
        panic!("constructor native")
    };
    let input = runtime.new_object(None).unwrap().into_handle();
    let prepared = runtime
        .prepare_native_invocation_jsvalue(
            &callable,
            realm,
            target,
            min_readable_args,
            NativeInvocation::Call {
                this_value: JsValue::Object(input),
            },
            Vec::new(),
            NativeInvokeMode::Ordinary,
        )
        .unwrap();
    let mut call = prepared.into_inner();
    let mut state = runtime.0.state.borrow_mut();
    let invocation = std::mem::replace(
        &mut call.invocation,
        NativeInvocation::Call {
            this_value: JsValue::Undefined,
        },
    );
    let NativeInvocationAdaptation::CyclePublishedComplete(completion @ Completion::Throw(_)) =
        state
            .adapt_native_invocation(
                &runtime.0.poisoned,
                target,
                realm,
                invocation,
                &call.activation.arguments,
            )
            .unwrap()
    else {
        panic!("actual ABI Error factory fact")
    };
    assert!(state.heap.object(input).is_err());
    drop_completion(&mut state, &runtime, completion);
    call.abandon(&mut state, &runtime.0.poisoned);
    assert!(!runtime.is_poisoned());
}

#[test]
fn constructor_rejection_cleanup_failure_stops_before_factory_and_argv_suffix() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let callable = runtime
        .callable_from_value(context.eval("Map").unwrap())
        .unwrap();
    let CallableExecution::Native {
        target,
        realm,
        min_readable_args,
    } = runtime.bytecode_for_callable(&callable).unwrap()
    else {
        panic!("constructor native")
    };
    let input = runtime.new_object(None).unwrap().into_handle();
    let suffix = runtime.new_object(None).unwrap().into_handle();
    let prepared = runtime
        .prepare_native_invocation_jsvalue(
            &callable,
            realm,
            target,
            min_readable_args,
            NativeInvocation::Call {
                this_value: JsValue::Object(input),
            },
            vec![JsValue::Object(suffix)],
            NativeInvokeMode::Ordinary,
        )
        .unwrap();
    let mut call = prepared.into_inner();
    let mut state = runtime.0.state.borrow_mut();
    let before = state.heap.counts().object_nodes;
    state
        .heap
        .set_strong_count_for_test(RawId::Object(input), 0);
    let invocation = std::mem::replace(
        &mut call.invocation,
        NativeInvocation::Call {
            this_value: JsValue::Undefined,
        },
    );
    assert!(
        state
            .adapt_native_invocation(
                &runtime.0.poisoned,
                target,
                realm,
                invocation,
                &call.activation.arguments
            )
            .is_err()
    );
    assert!(runtime.is_poisoned());
    assert_eq!(
        state.heap.counts().object_nodes,
        before,
        "the Error factory cannot run after destructive input cleanup fails"
    );
    assert_eq!(state.heap.object_strong_count(suffix), Ok(1));
    call.abandon(&mut state, &runtime.0.poisoned);
    assert_eq!(state.heap.object_strong_count(suffix), Ok(1));
}

#[test]
fn tagged_string_reply_fatal_cleanup_stops_before_parent_snapshot_suffix() {
    let runtime = Runtime::new();
    let context = runtime.new_context().unwrap();
    let receiver = runtime.new_object(None).unwrap();
    let suffix = runtime.new_object(None).unwrap().into_handle();
    let broken = runtime.new_object(None).unwrap().into_handle();
    let mut state = runtime.0.state.borrow_mut();
    let arguments = NativeArguments {
        actual_arg_count: 1,
        readable: vec![JsValue::Object(suffix)],
    };
    let ScalarTextStep::String { value, resume } = ScalarTextStep::start_in_state(
        &mut state,
        &runtime.0.poisoned,
        context.realm,
        ScalarTextKind::Concat,
        &NativeInvocation::Call {
            this_value: JsValue::Object(receiver.object_id()),
        },
        &arguments,
    )
    .unwrap() else {
        panic!("parent source")
    };
    state
        .release_owned_jsvalue(&runtime.0.poisoned, value)
        .unwrap();
    let mut step = Step::CyclePublishedStringReply {
        value: Some(NativeConversion::Throw(JsValue::Object(broken))),
        resume: Some(Resume::ScalarText(resume)),
    };
    state
        .heap
        .set_strong_count_for_test(RawId::Object(broken), 0);
    assert!(
        step.retire_raw_in_state(&mut state, &runtime.0.poisoned)
            .is_err()
    );
    assert!(runtime.is_poisoned());
    assert_eq!(state.heap.object_strong_count(suffix), Ok(2));
    assert!(matches!(
        step,
        Step::CyclePublishedStringReply {
            value: None,
            resume: Some(_)
        }
    ));
    drop(state);
    step.release_owned(&runtime);
    assert_eq!(
        runtime.0.state.borrow().heap.object_strong_count(suffix),
        Ok(2)
    );
}

#[test]
fn readonly_date_host_panic_quarantines_before_any_following_owner_retirement() {
    #[derive(Debug)]
    struct PanicTimezone;
    impl crate::engine::host::HostServices for PanicTimezone {
        fn now_millis(&self) -> i64 {
            0
        }
        fn timezone_offset_minutes(&self, _: i64) -> i32 {
            panic!("publication host probe")
        }
        fn random_seed(&self) -> u64 {
            1
        }
    }
    let runtime = Runtime::new_with_host_services(PanicTimezone);
    let mut context = runtime.new_context().unwrap();
    let Value::Object(date) = context.eval("new Date(0)").unwrap() else {
        panic!("Date")
    };
    let suffix = runtime.new_object(None).unwrap().into_handle();
    let mut state = runtime.0.state.borrow_mut();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        state.call_date_readonly_native_with_publication(
            &runtime.0.poisoned,
            runtime.0.host_services.as_ref(),
            context.realm,
            DateNativeKind::TimezoneOffset,
            &NativeInvocation::Call {
                this_value: JsValue::Object(date.object_id()),
            },
        )
    }));
    assert!(result.is_err());
    assert!(runtime.is_poisoned());
    assert_eq!(state.heap.object_strong_count(date.object_id()), Ok(1));
    assert_eq!(state.heap.object_strong_count(suffix), Ok(1));
}

#[test]
fn set_progress_keeps_actual_setter_owners_until_query_publishes_the_callback() {
    use crate::engine::object::{SetAction, SetProgress};
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let Value::Object(target) = context.eval("({set x(v){this.received=v}})").unwrap() else {
        panic!("setter target")
    };
    let argument = runtime.new_object(None).unwrap();
    let key = runtime.intern_property_key("x").unwrap();
    let mut storage = QueryStorage::default();
    let mut query = storage.acquire(context.realm, vec![Resume::RootSet], Finish::Root);
    let mut state = runtime.0.state.borrow_mut();
    let data = state.heap.object(target.object_id()).unwrap();
    let index = state
        .heap
        .shape(data.shape)
        .unwrap()
        .find(crate::engine::atom::AtomIdx::from_raw(key.atom().raw()))
        .unwrap() as usize;
    let crate::engine::heap::PropertySlot::Accessor { set, .. } = &data.slots[index] else {
        panic!("actual accessor")
    };
    let setter = set.option().unwrap();
    let setter_count = state.heap.object_strong_count(setter).unwrap();
    let receiver = state
        .dup_jsvalue(&JsValue::Object(target.object_id()))
        .unwrap();
    let value = state
        .dup_jsvalue(&JsValue::Object(argument.object_id()))
        .unwrap();
    let mut progress = Some(
        state
            .start_set_borrowed(
                &runtime.0.poisoned,
                Some(context.realm),
                target.object_id(),
                key.atom(),
                value,
                receiver,
            )
            .unwrap(),
    );
    assert!(
        !request::set::advance_set_progress_in_state(
            &mut state,
            &runtime.0.poisoned,
            &mut progress,
        )
        .unwrap()
    );
    assert!(matches!(
        progress.as_ref(),
        Some(SetProgress::Complete(SetAction::Call {
            function,
            receiver: JsValue::Object(receiver),
            argument: JsValue::Object(value),
        })) if *function == setter && *receiver == target.object_id() && *value == argument.object_id()
    ));
    assert_eq!(state.heap.object_strong_count(setter), Ok(setter_count + 1));
    assert_eq!(state.heap.object_strong_count(target.object_id()), Ok(2));
    assert_eq!(state.heap.object_strong_count(argument.object_id()), Ok(2));
    let mut step = Step::SetProgress(progress);
    let next = query
        .advance_raw_in_state(&runtime, &mut state, &mut step)
        .unwrap();
    assert!(matches!(next.effect, state::StateEffect::Callback));
    assert!(!next.cycle_published);
    assert!(matches!(step, Step::RawCall { .. }));
    assert_eq!(
        query.parents.len(),
        1,
        "setter reply still owns its Set parent"
    );
    step.retire_raw_in_state(&mut state, &runtime.0.poisoned)
        .unwrap();
    query.retire_raw_in_state(&runtime, &mut state).unwrap();
    assert_eq!(state.heap.object_strong_count(setter), Ok(setter_count));
    assert_eq!(state.heap.object_strong_count(target.object_id()), Ok(1));
    assert_eq!(state.heap.object_strong_count(argument.object_id()), Ok(1));
    assert!(!runtime.is_poisoned());
}

#[test]
fn query_state_set_parent_consumes_actual_autoinit_publication_once() {
    use crate::engine::object::{SetAction, SetProgress};
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    runtime.set_gc_policy(GcPolicy::Manual).unwrap();
    let cycle = unreachable_cycle(&runtime, &mut context);
    let Value::Object(target) = context.eval("(function transportTarget(){})").unwrap() else {
        panic!("bytecode constructor")
    };
    let marker = runtime.new_object(None).unwrap();
    let key = runtime
        .pinned_property_key(crate::engine::atom::pinned::PinnedAtom::Prototype)
        .unwrap();
    let mut storage = QueryStorage::default();
    let mut query = storage.acquire(context.realm, vec![Resume::RootSet], Finish::Root);
    let rc = std::rc::Rc::strong_count(&runtime.0);
    let mut state = runtime.0.state.borrow_mut();
    let data = state.heap.object(target.object_id()).unwrap();
    let index = state
        .heap
        .shape(data.shape)
        .unwrap()
        .find(crate::engine::atom::AtomIdx::from_raw(key.atom().raw()))
        .unwrap() as usize;
    assert!(matches!(
        &data.slots[index],
        crate::engine::heap::PropertySlot::AutoInit(_)
    ));
    let value = state
        .dup_jsvalue(&JsValue::Object(marker.object_id()))
        .unwrap();
    let receiver = state
        .dup_jsvalue(&JsValue::Object(target.object_id()))
        .unwrap();
    let progress = state
        .start_set_borrowed(
            &runtime.0.poisoned,
            Some(context.realm),
            target.object_id(),
            key.atom(),
            value,
            receiver,
        )
        .unwrap();
    assert!(matches!(
        progress,
        SetProgress::CyclePublished(SetAction::Complete)
    ));
    let mut step = Step::SetProgress(Some(progress));
    let next = query
        .advance_raw_in_state(&runtime, &mut state, &mut step)
        .unwrap();
    assert!(matches!(next.effect, state::StateEffect::Complete));
    assert!(next.cycle_published);
    assert!(query.parents.is_empty());
    assert!(matches!(
        step,
        Step::Complete(Some(Completion::Return(JsValue::Bool(true))))
    ));
    assert!(
        !query
            .advance_raw_in_state(&runtime, &mut state, &mut step)
            .unwrap()
            .cycle_published
    );
    runtime.0.gc_pressure.policy.set(GcPolicy::Automatic);
    runtime.0.gc_pressure.remaining.set(0);
    state
        .collect_if_requested(&runtime.0.gc_pressure, &runtime.0.poisoned)
        .unwrap();
    assert!(state.heap.object(cycle).is_err());
    let data = state.heap.object(target.object_id()).unwrap();
    assert!(matches!(
        &data.slots[index],
        crate::engine::heap::PropertySlot::Data(crate::engine::heap::RawValue::Object(id))
            if *id == marker.object_id()
    ));
    assert_eq!(state.heap.object_strong_count(marker.object_id()), Ok(2));
    assert_eq!(std::rc::Rc::strong_count(&runtime.0), rc);
    step.retire_raw_in_state(&mut state, &runtime.0.poisoned)
        .unwrap();
    assert!(!runtime.is_poisoned());
}

#[test]
fn query_legacy_regexp_set_reply_keeps_actual_parent_until_boundary_retirement() {
    use crate::engine::{builtins::RegExpSearchStep, object::SetProgress};
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let Value::Object(regexp) = context
        .eval("(()=>{const r=/a/;r.lastIndex=7;return r})()")
        .unwrap()
    else {
        panic!("RegExp target")
    };
    let original_count = runtime
        .0
        .state
        .borrow()
        .heap
        .object_strong_count(regexp.object_id())
        .unwrap();
    let original_rc = std::rc::Rc::strong_count(&runtime.0);
    // These protocol inputs borrow the public RegExp and own only an Int argv.
    let invocation = NativeInvocation::Call {
        this_value: JsValue::Object(regexp.object_id()),
    };
    let arguments = NativeArguments {
        actual_arg_count: 1,
        readable: vec![JsValue::Int(42)],
    };
    let RegExpSearchStep::Primitive { mut resume } =
        RegExpSearchStep::start(&runtime, context.realm, &invocation, &arguments).unwrap()
    else {
        panic!("actual search primitive phase")
    };
    let value = resume.take_primitive_value();
    let RegExpSearchStep::Read { mut resume } =
        resume.resume(&runtime, Completion::Return(value)).unwrap()
    else {
        panic!("actual lastIndex read")
    };
    let read_object = resume.take_read_object();
    let read_key = resume.take_read_key();
    let previous = context.get_property(&read_object, &read_key).unwrap();
    let previous = runtime.into_jsvalue(previous).unwrap();
    drop(read_key);
    drop(read_object);
    let RegExpSearchStep::Set { mut resume } = resume
        .resume(&runtime, Completion::Return(previous))
        .unwrap()
    else {
        panic!("nonzero lastIndex requires the real search Set phase")
    };
    let object = resume.take_set_object();
    let key = resume.take_set_key();
    let value = resume.take_set_value();
    let receiver = runtime
        .dup_jsvalue(&JsValue::Object(object.object_id()))
        .unwrap();
    let mut storage = QueryStorage::default();
    let mut query = storage.acquire(
        context.realm,
        vec![Resume::RegExpSearch(resume)],
        Finish::Root,
    );
    let mut step;
    {
        let mut state = runtime.0.state.borrow_mut();
        let progress = state
            .start_set_borrowed(
                &runtime.0.poisoned,
                Some(context.realm),
                object.object_id(),
                key.atom(),
                value,
                receiver,
            )
            .unwrap();
        assert!(matches!(progress, SetProgress::Complete(_)));
        step = Step::SetProgress(Some(progress));
        let next = query
            .advance_raw_in_state(&runtime, &mut state, &mut step)
            .unwrap();
        assert!(matches!(next.effect, state::StateEffect::Boundary));
        assert!(!next.cycle_published);
        assert!(query.parents.is_empty());
        assert!(matches!(
            &step,
            Step::SetReply {
                action: Some(crate::engine::object::SetAction::Complete),
                resume: Some(Resume::RegExpSearch(_)),
            }
        ));
        assert_eq!(
            state.heap.object_strong_count(regexp.object_id()),
            Ok(original_count + 2),
            "public RegExp, extracted request object and armed legacy parent"
        );
    }
    drop(key);
    drop(object);
    let Step::SetReply { action, resume } = &mut step else {
        unreachable!()
    };
    let action = action.take().unwrap();
    let parent = resume.take().unwrap();
    step = parent
        .set(&runtime, action.into_boundary(&runtime))
        .unwrap();
    assert!(matches!(step, Step::RegExpExec { .. }));
    step.release_owned(&runtime);
    assert_eq!(
        runtime
            .0
            .state
            .borrow()
            .heap
            .object_strong_count(regexp.object_id()),
        Ok(original_count)
    );
    assert_eq!(std::rc::Rc::strong_count(&runtime.0), original_rc);
    assert!(!runtime.is_poisoned());
}
