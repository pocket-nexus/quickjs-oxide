//! Checked receiver roles and terminal retirement remain explicit under a held lease.
use super::*;

#[test]
fn date_converting_checked_receiver_and_argv_failures_restore_only_acquired_edges() {
    let (runtime, observations) = runtime(false);
    let mut context = runtime.new_context().unwrap();
    let receiver = date(&runtime, &mut context);
    let first = runtime.new_object(None).unwrap().into_handle();
    let symbol = runtime.new_symbol(None).unwrap();
    let mut state = runtime.0.state.borrow_mut();
    let receiver_count = state
        .heap
        .object_strong_count(receiver.object_id())
        .unwrap();
    state
        .heap
        .set_strong_count_for_test(RawId::Object(receiver.object_id()), u32::MAX);
    let error = start(
        &mut state,
        &runtime,
        context.realm,
        DateNativeKind::SetField(DateSetFieldKind::Minutes),
        JsValue::Object(receiver.object_id()),
        vec![JsValue::Int(1)],
        1,
    );
    state
        .heap
        .set_strong_count_for_test(RawId::Object(receiver.object_id()), receiver_count);
    assert!(matches!(
        error,
        Err(RuntimeError::Heap(HeapError::Overflow { .. }))
    ));
    assert!(observations.borrow().is_empty());
    let index = state.atoms.unbrand(symbol.atom()).unwrap();
    state.atoms.set_ref_count_for_test(index, u32::MAX);
    let error = start(
        &mut state,
        &runtime,
        context.realm,
        DateNativeKind::SetField(DateSetFieldKind::UtcSeconds),
        JsValue::Object(receiver.object_id()),
        vec![JsValue::Object(first), JsValue::Symbol(index)],
        2,
    );
    assert_eq!(
        state.atoms.resolve(symbol.atom()).unwrap().ref_count,
        Some(u32::MAX)
    );
    state.atoms.set_ref_count_for_test(index, 1);
    assert!(matches!(
        error,
        Err(RuntimeError::Atom(
            crate::engine::atom::AtomError::RefCountOverflow(_)
        ))
    ));
    assert_eq!(state.heap.object_strong_count(first), Ok(1));
    assert_eq!(
        state.heap.object_strong_count(receiver.object_id()),
        Ok(receiver_count)
    );
    assert_eq!(
        state.heap.date_value(receiver.object_id()),
        Ok(946684800123.0)
    );
    state
        .release_owned_jsvalue(&runtime.0.poisoned, JsValue::Object(first))
        .unwrap();
    assert!(!runtime.is_poisoned());
}

#[test]
fn date_converting_to_primitive_keeps_both_checked_roles_and_exact_max_sentinel_contract() {
    let (runtime, _) = runtime(false);
    let context = runtime.new_context().unwrap();
    let object = runtime.new_object(None).unwrap();
    let hint = runtime
        .into_jsvalue(Value::String(JsString::from_static("number")))
        .unwrap();
    let mut state = runtime.0.state.borrow_mut();
    let count = state.heap.object_strong_count(object.object_id()).unwrap();
    state
        .heap
        .set_strong_count_for_test(RawId::Object(object.object_id()), u32::MAX - 1);
    let error = start(
        &mut state,
        &runtime,
        context.realm,
        DateNativeKind::ToPrimitive,
        JsValue::Object(object.object_id()),
        vec![JsValue::from_raw(hint.as_raw()).unwrap()],
        1,
    );
    let observed = state.heap.object_strong_count(object.object_id());
    state
        .heap
        .set_strong_count_for_test(RawId::Object(object.object_id()), count);
    assert!(matches!(
        error,
        Err(RuntimeError::Heap(HeapError::Overflow { .. }))
    ));
    // The first checked role reaches the established immortal heap sentinel;
    // the second checked role still rejects instead of being silently elided.
    assert_eq!(observed, Ok(u32::MAX));
    assert!(!runtime.is_poisoned());
    let step = start(
        &mut state,
        &runtime,
        context.realm,
        DateNativeKind::ToPrimitive,
        JsValue::Object(object.object_id()),
        vec![JsValue::from_raw(hint.as_raw()).unwrap()],
        1,
    )
    .unwrap();
    assert_eq!(
        state.heap.object_strong_count(object.object_id()),
        Ok(count + 1)
    );
    assert!(matches!(
        step,
        DatePrototypeStep::OrdinaryPrimitive {
            hint: ToPrimitiveHint::Number,
            ..
        }
    ));
    step.retire_in_state(&mut state, &runtime.0.poisoned)
        .unwrap();
    assert_eq!(
        state.heap.object_strong_count(object.object_id()),
        Ok(count)
    );
    state
        .release_owned_jsvalue(&runtime.0.poisoned, hint)
        .unwrap();
}

#[test]
fn date_converting_json_read_abandonment_retires_distinct_receiver_and_request_roles() {
    let (runtime, _) = runtime(false);
    let context = runtime.new_context().unwrap();
    let object = runtime.new_object(None).unwrap();
    let owners = Rc::strong_count(&runtime.0);
    let host_owners = Rc::strong_count(&runtime.0.host_services);
    let mut state = runtime.0.state.borrow_mut();
    let count = state.heap.object_strong_count(object.object_id()).unwrap();
    let DatePrototypeStep::Primitive { value, resume, .. } = start(
        &mut state,
        &runtime,
        context.realm,
        DateNativeKind::ToJson,
        JsValue::Object(object.object_id()),
        Vec::new(),
        0,
    )
    .unwrap() else {
        panic!("direct object must not report wrapper publication")
    };
    state
        .release_owned_jsvalue(&runtime.0.poisoned, value)
        .unwrap();
    let step = resume
        .resume_in_state(
            &mut state,
            &runtime.0.poisoned,
            Completion::Return(JsValue::Int(1)),
        )
        .unwrap();
    assert!(
        matches!(&step, DatePrototypeStep::Read { object: id, receiver:JsValue::Object(receiver),key,.. }
        if *id == object.object_id() && id == receiver && *key == state.pinned_atoms.get(PinnedAtom::ToISOString))
    );
    assert_eq!(
        state.heap.object_strong_count(object.object_id()),
        Ok(count + 3)
    );
    step.retire_in_state(&mut state, &runtime.0.poisoned)
        .unwrap();
    assert_eq!(
        state.heap.object_strong_count(object.object_id()),
        Ok(count)
    );
    assert_eq!(Rc::strong_count(&runtime.0), owners);
    assert_eq!(Rc::strong_count(&runtime.0.host_services), host_owners);
    assert!(!runtime.0.deferred_references.has_pending());
}

#[test]
fn date_converting_json_boxing_and_fresh_error_report_actual_publication_without_runtime_owner() {
    let (runtime, _) = runtime(false);
    let context = runtime.new_context().unwrap();
    let owners = Rc::strong_count(&runtime.0);
    let mut state = runtime.0.state.borrow_mut();
    let step = start(
        &mut state,
        &runtime,
        context.realm,
        DateNativeKind::ToJson,
        JsValue::Bool(true),
        Vec::new(),
        0,
    )
    .unwrap();
    assert!(matches!(
        &step,
        DatePrototypeStep::CyclePublishedPrimitive {
            value: JsValue::Object(_),
            ..
        }
    ));
    let DatePrototypeStep::CyclePublishedPrimitive {
        value: JsValue::Object(object),
        ..
    } = &step
    else {
        unreachable!()
    };
    assert_eq!(state.heap.object_strong_count(*object), Ok(2));
    step.retire_in_state(&mut state, &runtime.0.poisoned)
        .unwrap();
    let step = start(
        &mut state,
        &runtime,
        context.realm,
        DateNativeKind::SetTime,
        JsValue::Bool(true),
        vec![JsValue::Int(1)],
        1,
    )
    .unwrap();
    let DatePrototypeStep::CyclePublished(Completion::Throw(JsValue::Object(error))) = &step else {
        panic!("actual brand Error producer must carry allocation publication")
    };
    let prototype = state
        .heap
        .context(context.realm)
        .unwrap()
        .native_error_prototypes[NativeErrorKind::Type.index()];
    assert_eq!(
        state
            .heap
            .shape(state.heap.object(*error).unwrap().shape)
            .unwrap()
            .prototype(),
        prototype
    );
    step.retire_in_state(&mut state, &runtime.0.poisoned)
        .unwrap();
    assert_eq!(Rc::strong_count(&runtime.0), owners);
    assert!(!runtime.is_poisoned());
}

fn queue_invalid_zero(state: &mut RuntimeState, first: ObjectId, later: ObjectId) {
    state
        .heap
        .queue_release_for_test(RawId::Object(first))
        .unwrap();
    state
        .heap
        .queue_release_for_test(RawId::Object(later))
        .unwrap();
    state
        .heap
        .set_strong_count_for_test(RawId::Object(first), 1);
}

#[test]
fn date_converting_json_error_publication_failure_skips_method_receiver_and_queue_suffix() {
    let (runtime, _) = runtime(false);
    let context = runtime.new_context().unwrap();
    let receiver = runtime.new_object(None).unwrap().into_handle();
    let method = runtime.new_object(None).unwrap().into_handle();
    let first = runtime.new_object(None).unwrap().into_handle();
    let later = runtime.new_object(None).unwrap().into_handle();
    let mut state = runtime.0.state.borrow_mut();
    let before = state.heap.counts().object_nodes;
    let resume = DatePrototypeResume::new(context.realm, receiver, Phase::JsonMethod, 0);
    queue_invalid_zero(&mut state, first, later);
    let result = resume.resume_in_state(
        &mut state,
        &runtime.0.poisoned,
        Completion::Return(JsValue::Object(method)),
    );
    assert!(matches!(result, Err(RuntimeError::Poisoned)));
    assert!(runtime.is_poisoned());
    assert_eq!(state.heap.counts().object_nodes, before + 1);
    assert_eq!(state.heap.object_strong_count(receiver), Ok(1));
    assert_eq!(state.heap.object_strong_count(method), Ok(1));
    assert_eq!(state.heap.object_strong_count(later), Ok(0));
    assert!(state.heap.has_pending_zero_cleanup());
}

#[test]
fn date_converting_terminal_retirement_stops_before_argument_reply_and_receiver_suffix() {
    let (runtime, _) = runtime(false);
    let mut context = runtime.new_context().unwrap();
    let receiver = date(&runtime, &mut context).into_handle();
    let first = runtime.new_object(None).unwrap().into_handle();
    let suffix = runtime.new_object(None).unwrap().into_handle();
    let reply = runtime.new_object(None).unwrap().into_handle();
    let doomed = runtime.new_object(None).unwrap().into_handle();
    let later = runtime.new_object(None).unwrap().into_handle();
    let mut resume = DatePrototypeResume::new(context.realm, receiver, Phase::Time, 2);
    resume
        .0
        .arguments
        .extend([JsValue::Object(first), JsValue::Object(suffix)]);
    resume.0.reply = Some(JsValue::Object(reply));
    let mut state = runtime.0.state.borrow_mut();
    queue_invalid_zero(&mut state, doomed, later);
    let result = resume.number_in_state(
        &mut state,
        &runtime.0.poisoned,
        runtime.0.host_services.as_ref(),
        NativeConversion::Value(42.0),
    );
    assert!(matches!(result, Err(RuntimeError::Poisoned)));
    assert_eq!(state.heap.date_value(receiver), Ok(42.0));
    assert_eq!(state.heap.object_strong_count(first), Ok(0));
    assert_eq!(state.heap.object_strong_count(suffix), Ok(1));
    assert_eq!(state.heap.object_strong_count(reply), Ok(1));
    assert_eq!(state.heap.object_strong_count(receiver), Ok(1));
    assert!(runtime.is_poisoned());
}

#[test]
fn date_converting_host_panics_quarantine_original_and_post_conversion_receiver_roles() {
    for kind in [
        DateNativeKind::SetField(DateSetFieldKind::Minutes),
        DateNativeKind::SetYear,
    ] {
        let (runtime, observations) = runtime(true);
        let mut context = runtime.new_context().unwrap();
        let receiver = date(&runtime, &mut context);
        let mut state = runtime.0.state.borrow_mut();
        let count = state
            .heap
            .object_strong_count(receiver.object_id())
            .unwrap();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let step = start(
                &mut state,
                &runtime,
                context.realm,
                kind,
                JsValue::Object(receiver.object_id()),
                vec![JsValue::Int(2000)],
                1,
            )
            .unwrap();
            let DatePrototypeStep::Number { resume, .. } = step else {
                unreachable!()
            };
            state
                .heap
                .set_date_value(receiver.object_id(), 0.0)
                .unwrap();
            let _ = resume.number_in_state(
                &mut state,
                &runtime.0.poisoned,
                runtime.0.host_services.as_ref(),
                NativeConversion::Value(2000.0),
            );
        }));
        assert!(result.is_err());
        assert!(runtime.is_poisoned());
        assert_eq!(
            state.heap.object_strong_count(receiver.object_id()),
            Ok(count + 1)
        );
        assert_eq!(
            &*observations.borrow(),
            if kind == DateNativeKind::SetYear {
                &[0][..]
            } else {
                &[946684800123][..]
            }
        );
    }
}

#[test]
fn date_converting_local_host_schedule_keeps_snapshot_and_post_conversion_queries() {
    let (runtime, observations) = runtime(false);
    let mut context = runtime.new_context().unwrap();
    let receiver = date(&runtime, &mut context);
    let mut state = runtime.0.state.borrow_mut();
    let DatePrototypeStep::Number { resume, .. } = start(
        &mut state,
        &runtime,
        context.realm,
        DateNativeKind::SetField(DateSetFieldKind::Minutes),
        JsValue::Object(receiver.object_id()),
        vec![JsValue::Int(7)],
        1,
    )
    .unwrap() else {
        unreachable!()
    };
    assert_eq!(&*observations.borrow(), &[946684800123]);
    state
        .heap
        .set_date_value(receiver.object_id(), 0.0)
        .unwrap();
    let step = resume
        .number_in_state(
            &mut state,
            &runtime.0.poisoned,
            runtime.0.host_services.as_ref(),
            NativeConversion::Value(7.0),
        )
        .unwrap();
    assert!(matches!(
        step,
        DatePrototypeStep::Complete(Completion::Return(_))
    ));
    assert_eq!(&*observations.borrow(), &[946684800123, 946685220123]);
    assert_eq!(
        state.heap.date_value(receiver.object_id()),
        Ok(946685220123.0)
    );
    observations.borrow_mut().clear();
    let DatePrototypeStep::Number { resume, .. } = start(
        &mut state,
        &runtime,
        context.realm,
        DateNativeKind::SetYear,
        JsValue::Object(receiver.object_id()),
        vec![JsValue::Int(2000)],
        1,
    )
    .unwrap() else {
        unreachable!()
    };
    assert!(observations.borrow().is_empty());
    state
        .heap
        .set_date_value(receiver.object_id(), 0.0)
        .unwrap();
    let step = resume
        .number_in_state(
            &mut state,
            &runtime.0.poisoned,
            runtime.0.host_services.as_ref(),
            NativeConversion::Value(2000.0),
        )
        .unwrap();
    assert!(matches!(
        step,
        DatePrototypeStep::Complete(Completion::Return(_))
    ));
    assert_eq!(&*observations.borrow(), &[0, 946684800000]);
    assert_eq!(
        state.heap.date_value(receiver.object_id()),
        Ok(946684800000.0)
    );
}

#[test]
fn date_converting_external_abandonment_while_state_busy_coordinates_without_poison() {
    let (runtime, _) = runtime(false);
    let mut context = runtime.new_context().unwrap();
    let receiver = date(&runtime, &mut context).into_handle();
    let input = runtime.new_object(None).unwrap().into_handle();
    let suffix = runtime.new_object(None).unwrap().into_handle();
    let owners = Rc::strong_count(&runtime.0);
    let mut resume = DatePrototypeResume::new(context.realm, receiver, Phase::Time, 2);
    resume.0.arguments.push_back(JsValue::Object(suffix));
    let step = DatePrototypeStep::Number {
        value: JsValue::Object(input),
        resume,
    };
    let state = runtime.0.state.borrow_mut();
    {
        let _owner = DateBoundaryGuard {
            runtime: &runtime,
            step: Some(step),
        };
    }
    assert!(!runtime.is_poisoned());
    assert!(runtime.0.deferred_references.has_pending());
    assert_eq!(Rc::strong_count(&runtime.0), owners);
    assert_eq!(state.heap.object_strong_count(input), Ok(1));
    assert_eq!(state.heap.object_strong_count(suffix), Ok(1));
    assert_eq!(state.heap.object_strong_count(receiver), Ok(1));
    drop(state);
    runtime.drain_deferred_references().unwrap();
    let state = runtime.0.state.borrow();
    assert!(state.heap.object(input).is_err());
    assert!(state.heap.object(suffix).is_err());
    assert!(state.heap.object(receiver).is_err());
    assert!(!runtime.0.deferred_references.has_pending());
    assert!(!runtime.is_poisoned());
}
