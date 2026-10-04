use super::*;
use crate::engine::{
    heap::{HeapError, ObjectId, ObjectPayload, RawId},
    value::Value,
};

fn pending(
    state: &mut RuntimeState,
    poison: &Cell<bool>,
    realm: ContextId,
    kind: ScalarTextKind,
    receiver: ObjectId,
    arguments: &NativeArguments,
) -> (JsValue, ScalarTextResume) {
    let ScalarTextStep::String { value, resume } = ScalarTextStep::start_in_state(
        state,
        poison,
        realm,
        kind,
        &NativeInvocation::Call {
            this_value: JsValue::Object(receiver),
        },
        arguments,
    )
    .unwrap() else {
        panic!("object receiver must request ToString");
    };
    (value, resume)
}

#[test]
fn scalar_text_state_all_eight_selectors_complete_object_receiver_protocol() {
    let runtime = Runtime::new();
    let context = runtime.new_context().unwrap();
    let receiver = runtime.new_object(None).unwrap();
    let arguments = NativeArguments {
        actual_arg_count: 1,
        readable: vec![JsValue::Int(1)],
    };
    let _unwind = runtime.unwind_guard();
    let mut state = runtime.0.state.borrow_mut();
    for kind in [
        ScalarTextKind::CharAt(StringCharAtKind::At),
        ScalarTextKind::CharAt(StringCharAtKind::CharAt),
        ScalarTextKind::CharCodeAt,
        ScalarTextKind::CodePointAt,
        ScalarTextKind::Concat,
        ScalarTextKind::WellFormed(StringWellFormedKind::IsWellFormed),
        ScalarTextKind::WellFormed(StringWellFormedKind::ToWellFormed),
        ScalarTextKind::Iterator,
    ] {
        let (request, resume) = pending(
            &mut state,
            &runtime.0.poisoned,
            context.realm,
            kind,
            receiver.object_id(),
            &arguments,
        );
        state
            .release_owned_jsvalue(&runtime.0.poisoned, request)
            .unwrap();
        let step = resume
            .string_in_state(
                &mut state,
                &runtime.0.poisoned,
                NativeConversion::Value(JsString::from_static("abc")),
            )
            .unwrap();
        let step = match step {
            ScalarTextStep::Number { value, resume } => {
                assert!(matches!(value, JsValue::Int(1)));
                resume
                    .number_in_state(
                        &mut state,
                        &runtime.0.poisoned,
                        NativeConversion::Value(1.0),
                    )
                    .unwrap()
            }
            ScalarTextStep::String { value, resume } => {
                let reply = state
                    .finish_string_value(
                        &runtime.0.poisoned,
                        context.realm,
                        Completion::Return(value),
                    )
                    .unwrap();
                resume
                    .string_in_state(&mut state, &runtime.0.poisoned, reply)
                    .unwrap()
            }
            step => step,
        };
        let ScalarTextStep::Complete(Completion::Return(value)) = step else {
            panic!("scalar completion");
        };
        match (&kind, &value) {
            (ScalarTextKind::CharAt(_), JsValue::String(id)) => {
                assert_eq!(state.heap.string(*id).unwrap(), &JsString::from_static("b"))
            }
            (ScalarTextKind::CharCodeAt | ScalarTextKind::CodePointAt, JsValue::Int(98)) => {}
            (ScalarTextKind::Concat, JsValue::String(id)) => assert_eq!(
                state.heap.string(*id).unwrap(),
                &JsString::from_static("abc1")
            ),
            (
                ScalarTextKind::WellFormed(StringWellFormedKind::IsWellFormed),
                JsValue::Bool(true),
            ) => {}
            (
                ScalarTextKind::WellFormed(StringWellFormedKind::ToWellFormed),
                JsValue::String(id),
            ) => assert_eq!(
                state.heap.string(*id).unwrap(),
                &JsString::from_static("abc")
            ),
            (ScalarTextKind::Iterator, JsValue::Object(id)) => assert!(matches!(
                state.heap.object(*id).unwrap().payload,
                ObjectPayload::StringIterator { .. }
            )),
            _ => panic!("selector result"),
        }
        state
            .release_owned_jsvalue(&runtime.0.poisoned, value)
            .unwrap();
        assert_eq!(state.heap.object_strong_count(receiver.object_id()), Ok(1));
    }
    assert!(!runtime.0.deferred_references.has_pending());
}

#[test]
fn scalar_text_state_concat_snapshot_failure_precedes_receiver_retain() {
    let runtime = Runtime::new();
    let context = runtime.new_context().unwrap();
    let receiver = runtime.new_object(None).unwrap();
    let first = runtime.new_object(None).unwrap();
    let blocked = runtime.new_symbol(None).unwrap();
    let later = runtime.new_object(None).unwrap();
    let index = runtime
        .0
        .state
        .borrow()
        .atoms
        .unbrand(blocked.atom())
        .unwrap();
    let arguments = NativeArguments {
        actual_arg_count: 3,
        readable: vec![
            JsValue::Object(first.object_id()),
            JsValue::Symbol(index),
            JsValue::Object(later.object_id()),
        ],
    };
    let _unwind = runtime.unwind_guard();
    let mut state = runtime.0.state.borrow_mut();
    state.atoms.set_ref_count_for_test(index, u32::MAX);
    state
        .heap
        .set_strong_count_for_test(RawId::Object(receiver.object_id()), u32::MAX);
    let result = ScalarTextStep::start_in_state(
        &mut state,
        &runtime.0.poisoned,
        context.realm,
        ScalarTextKind::Concat,
        &NativeInvocation::Call {
            this_value: JsValue::Object(receiver.object_id()),
        },
        &arguments,
    );
    let blocked_count = state.atoms.resolve(blocked.atom()).unwrap().ref_count;
    let receiver_count = state.heap.object_strong_count(receiver.object_id());
    state.atoms.set_ref_count_for_test(index, 1);
    state
        .heap
        .set_strong_count_for_test(RawId::Object(receiver.object_id()), 1);
    assert!(matches!(
        result,
        Err(RuntimeError::Atom(
            crate::engine::atom::AtomError::RefCountOverflow(_)
        ))
    ));
    assert_eq!(blocked_count, Some(u32::MAX));
    assert_eq!(receiver_count, Ok(u32::MAX));
    assert_eq!(state.heap.object_strong_count(first.object_id()), Ok(1));
    assert_eq!(state.heap.object_strong_count(later.object_id()), Ok(1));
    assert!(!runtime.is_poisoned());
}

#[test]
fn scalar_text_state_receiver_overflow_retires_complete_argument_snapshot() {
    let runtime = Runtime::new();
    let context = runtime.new_context().unwrap();
    let receiver = runtime.new_object(None).unwrap();
    let first = runtime.new_object(None).unwrap();
    let last = runtime.new_object(None).unwrap();
    let arguments = NativeArguments {
        actual_arg_count: 2,
        readable: vec![
            JsValue::Object(first.object_id()),
            JsValue::Object(last.object_id()),
        ],
    };
    let _unwind = runtime.unwind_guard();
    let mut state = runtime.0.state.borrow_mut();
    state
        .heap
        .set_strong_count_for_test(RawId::Object(receiver.object_id()), u32::MAX);
    let result = ScalarTextStep::start_in_state(
        &mut state,
        &runtime.0.poisoned,
        context.realm,
        ScalarTextKind::Concat,
        &NativeInvocation::Call {
            this_value: JsValue::Object(receiver.object_id()),
        },
        &arguments,
    );
    state
        .heap
        .set_strong_count_for_test(RawId::Object(receiver.object_id()), 1);
    assert!(matches!(
        result,
        Err(RuntimeError::Heap(HeapError::Overflow { .. }))
    ));
    assert_eq!(state.heap.object_strong_count(first.object_id()), Ok(1));
    assert_eq!(state.heap.object_strong_count(last.object_id()), Ok(1));
    assert!(!runtime.is_poisoned());
}

#[test]
fn scalar_text_state_string_throw_precedes_phase_but_number_validates_phase() {
    let runtime = Runtime::new();
    let context = runtime.new_context().unwrap();
    let receiver = runtime.new_object(None).unwrap();
    let argument = runtime.new_object(None).unwrap();
    let arguments = NativeArguments {
        actual_arg_count: 1,
        readable: vec![JsValue::Object(argument.object_id())],
    };
    let first_throw = runtime.new_object(None).unwrap().into_handle();
    let second_throw = runtime.new_object(None).unwrap().into_handle();
    let _unwind = runtime.unwind_guard();
    let mut state = runtime.0.state.borrow_mut();
    let (request, resume) = pending(
        &mut state,
        &runtime.0.poisoned,
        context.realm,
        ScalarTextKind::CharCodeAt,
        receiver.object_id(),
        &arguments,
    );
    state
        .release_owned_jsvalue(&runtime.0.poisoned, request)
        .unwrap();
    let reply = resume
        .string_in_state(
            &mut state,
            &runtime.0.poisoned,
            NativeConversion::Throw(JsValue::Object(first_throw)),
        )
        .unwrap();
    assert!(
        matches!(reply, ScalarTextStep::Complete(Completion::Throw(JsValue::Object(id))) if id == first_throw)
    );
    assert_eq!(state.heap.object_strong_count(first_throw), Ok(1));
    state
        .release_owned_jsvalue(&runtime.0.poisoned, JsValue::Object(first_throw))
        .unwrap();
    let (request, resume) = pending(
        &mut state,
        &runtime.0.poisoned,
        context.realm,
        ScalarTextKind::CharCodeAt,
        receiver.object_id(),
        &arguments,
    );
    state
        .release_owned_jsvalue(&runtime.0.poisoned, request)
        .unwrap();
    let result = resume.number_in_state(
        &mut state,
        &runtime.0.poisoned,
        NativeConversion::Throw(JsValue::Object(second_throw)),
    );
    assert!(matches!(
        result,
        Err(RuntimeError::Invariant(
            "String scalar index phase mismatch"
        ))
    ));
    assert!(state.heap.object(second_throw).is_err());
    assert_eq!(state.heap.object_strong_count(argument.object_id()), Ok(1));
}

#[test]
fn scalar_text_state_resumed_concat_retires_chunk_without_linearizing_alias() {
    let runtime = Runtime::new();
    let context = runtime.new_context().unwrap();
    let receiver = runtime.new_object(None).unwrap();
    let leaf = JsString::try_from_utf8(&"x".repeat(1024)).unwrap();
    let rope = leaf.try_concat(&leaf).unwrap();
    let chunk = runtime.into_jsvalue(Value::String(rope.clone())).unwrap();
    let JsValue::String(id) = chunk else {
        panic!("chunk string");
    };
    let arguments = NativeArguments {
        actual_arg_count: 1,
        readable: vec![chunk],
    };
    let _unwind = runtime.unwind_guard();
    let mut state = runtime.0.state.borrow_mut();
    let (request, resume) = pending(
        &mut state,
        &runtime.0.poisoned,
        context.realm,
        ScalarTextKind::Concat,
        receiver.object_id(),
        &arguments,
    );
    assert_eq!(state.heap.strong_count(RawId::String(id)), Ok(2));
    state
        .release_owned_jsvalue(&runtime.0.poisoned, request)
        .unwrap();
    let ScalarTextStep::Complete(Completion::Return(JsValue::String(result))) = resume
        .string_in_state(
            &mut state,
            &runtime.0.poisoned,
            NativeConversion::Value(JsString::from_static("a")),
        )
        .unwrap()
    else {
        panic!("concat result");
    };
    assert!(!rope.is_flat());
    assert_eq!(state.heap.strong_count(RawId::String(id)), Ok(1));
    assert_eq!(state.heap.string(result).unwrap().len(), 2049);
    state
        .release_owned_jsvalue(&runtime.0.poisoned, JsValue::String(result))
        .unwrap();
    state
        .release_owned_jsvalue(&runtime.0.poisoned, JsValue::String(id))
        .unwrap();
}

#[test]
fn scalar_text_state_pending_retirement_stops_before_later_snapshot_on_poison() {
    let runtime = Runtime::new();
    let context = runtime.new_context().unwrap();
    let receiver = runtime.new_object(None).unwrap();
    let invalid = runtime.new_object(None).unwrap();
    let later = runtime.new_object(None).unwrap();
    let arguments = NativeArguments {
        actual_arg_count: 2,
        readable: vec![
            JsValue::Object(invalid.object_id()),
            JsValue::Object(later.object_id()),
        ],
    };
    let _unwind = runtime.unwind_guard();
    let mut state = runtime.0.state.borrow_mut();
    let (request, resume) = pending(
        &mut state,
        &runtime.0.poisoned,
        context.realm,
        ScalarTextKind::Concat,
        receiver.object_id(),
        &arguments,
    );
    state
        .release_owned_jsvalue(&runtime.0.poisoned, request)
        .unwrap();
    state
        .heap
        .set_strong_count_for_test(RawId::Object(invalid.object_id()), 0);
    assert!(
        resume
            .retire_in_state(&mut state, &runtime.0.poisoned)
            .is_err()
    );
    assert!(runtime.is_poisoned());
    assert_eq!(state.heap.object_strong_count(later.object_id()), Ok(2));
    assert_eq!(state.heap.object_strong_count(receiver.object_id()), Ok(1));
}

#[test]
fn scalar_text_state_iterator_prototype_checked_temporary_rejection_is_recoverable() {
    let runtime = Runtime::new();
    let context = runtime.new_context().unwrap();
    let _unwind = runtime.unwind_guard();
    let mut state = runtime.0.state.borrow_mut();
    let prototype = state
        .heap
        .context(context.realm)
        .unwrap()
        .string_iterator_prototype;
    let actual = state.heap.object_strong_count(prototype).unwrap();
    let before = state.heap.counts();
    state
        .heap
        .set_strong_count_for_test(RawId::Object(prototype), u32::MAX);
    let result = state.new_string_iterator(
        &runtime.0.poisoned,
        context.realm,
        JsString::from_static("a"),
    );
    let observed = state.heap.object_strong_count(prototype);
    state
        .heap
        .set_strong_count_for_test(RawId::Object(prototype), actual);
    assert!(matches!(
        result,
        Err(RuntimeError::Heap(HeapError::Overflow { .. }))
    ));
    assert_eq!(observed, Ok(u32::MAX));
    assert_eq!(state.heap.counts(), before);
    assert!(!runtime.is_poisoned());
    let iterator = state
        .new_string_iterator(
            &runtime.0.poisoned,
            context.realm,
            JsString::from_static("a"),
        )
        .unwrap();
    state
        .release_owned_jsvalue(&runtime.0.poisoned, JsValue::Object(iterator))
        .unwrap();
}

#[test]
fn scalar_text_state_iterator_publication_failure_quarantines_prototype_and_suffix() {
    let runtime = Runtime::new();
    let context = runtime.new_context().unwrap();
    let first = runtime.new_object(None).unwrap().into_handle();
    let later = runtime.new_object(None).unwrap().into_handle();
    let suffix = runtime.new_object(None).unwrap().into_handle();
    let _unwind = runtime.unwind_guard();
    let mut state = runtime.0.state.borrow_mut();
    let prototype = state
        .heap
        .context(context.realm)
        .unwrap()
        .string_iterator_prototype;
    let count = state.heap.object_strong_count(prototype).unwrap();
    let before = state.heap.counts();
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
    {
        let mut suffix_owner =
            OwnedValueGuard::new(&mut state, &runtime.0.poisoned, JsValue::Object(suffix));
        let (state, _) = suffix_owner.parts();
        assert_eq!(
            state.new_string_iterator(
                &runtime.0.poisoned,
                context.realm,
                JsString::from_static("a")
            ),
            Err(RuntimeError::Heap(HeapError::Invariant(
                "finalization count disagrees with its queue/cycle state"
            )))
        );
        assert!(runtime.is_poisoned());
    }
    assert_eq!(state.heap.counts().object_nodes, before.object_nodes + 1);
    assert_eq!(state.heap.object_strong_count(prototype), Ok(count + 2));
    assert_eq!(state.heap.object_strong_count(suffix), Ok(1));
    assert_eq!(state.heap.object_strong_count(later), Ok(0));
    assert!(state.heap.has_pending_zero_cleanup());
}

#[test]
fn scalar_text_vm_all_eight_selectors_use_raw_children_with_real_get_and_call() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    #[cfg(feature = "profiling")]
    let profile = crate::engine::api::profiling::CostProfile::start();
    assert_eq!(context.eval(r#"(() => {
        let trace = '';
        const receiver = { get [Symbol.toPrimitive]() {
            trace += 'rg;';
            return function(hint) { trace += 'r' + hint + ';'; return 'AB'; };
        }};
        const index = { valueOf() { trace += 'i;'; return 1; } };
        const chunk = { get toString() {
            trace += 'cg;'; return function() { trace += 'c;'; return 'Z'; };
        }};
        const names = ['at','charAt','charCodeAt','codePointAt','concat','isWellFormed','toWellFormed',Symbol.iterator];
        const functions = names.map(name => String.prototype[name]);
        if (functions[0].call(receiver,index) !== 'B') return false;
        if (functions[1].call(receiver,index) !== 'B') return false;
        if (functions[2].call(receiver,index) !== 66) return false;
        if (functions[3].call(receiver,index) !== 66) return false;
        if (functions[4].call(receiver,chunk) !== 'ABZ') return false;
        if (functions[5].call(receiver) !== true) return false;
        if (functions[6].call(receiver) !== 'AB') return false;
        const iterator = functions[7].call(receiver);
        if (iterator.next().value !== 'A' || iterator.next().value !== 'B' || !iterator.next().done) return false;
        return trace === 'rg;rstring;i;rg;rstring;i;rg;rstring;i;rg;rstring;i;rg;rstring;cg;c;rg;rstring;rg;rstring;rg;rstring;';
    })()"#).unwrap(), Value::Bool(true));
    assert!(runtime.0.state.borrow().active_frames.is_empty());
    assert!(!runtime.0.deferred_references.has_pending());
    assert!(!runtime.is_poisoned());
    #[cfg(feature = "profiling")]
    {
        let costs = profile.snapshot();
        let count = |name| costs.owned_execution_events.get(name).copied().unwrap_or(0);
        assert_eq!(count("native_scalar_text_state_body"), 8);
        assert!(count("tostring_state_reply") >= 9);
    }
}

#[test]
fn tostring_vm_legacy_typed_parents_receive_completed_raw_child_once() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    assert_eq!(
        context
            .eval(
                r#"(() => {
        let trace = '';
        const json = { get [Symbol.toPrimitive]() {
            trace += 'jg;'; return function(hint) { trace += 'j' + hint + ';'; return '{"n":3}'; };
        }};
        if (JSON.parse(json).n !== 3) return false;
        const message = { toString() { trace += 'e;'; return 'message'; } };
        if (new Error(message).message !== 'message') return false;
        const item = { toString() { trace += 'a;'; return 'array'; } };
        if ([item].join() !== 'array') return false;
        const bad = { get toString() { trace += 'bad;'; throw 37; } };
        try { JSON.parse(bad); return false; } catch (error) { if (error !== 37) return false; }
        try { new Error(bad); return false; } catch (error) { if (error !== 37) return false; }
        return String(Symbol('x')) === 'Symbol(x)' && trace === 'jg;jstring;e;a;bad;bad;';
    })()"#
            )
            .unwrap(),
        Value::Bool(true)
    );
    assert!(runtime.0.state.borrow().active_frames.is_empty());
    assert!(!runtime.0.deferred_references.has_pending());
    assert!(!runtime.is_poisoned());
}

#[test]
fn scalar_text_state_object_receiver_keeps_unused_first_argument_checked_owner() {
    let runtime = Runtime::new();
    let context = runtime.new_context().unwrap();
    let receiver = runtime.new_object(None).unwrap();
    let unused = runtime.new_object(None).unwrap();
    let arguments = NativeArguments {
        actual_arg_count: 1,
        readable: vec![JsValue::Object(unused.object_id())],
    };
    let _unwind = runtime.unwind_guard();
    let mut state = runtime.0.state.borrow_mut();
    for kind in [
        ScalarTextKind::WellFormed(StringWellFormedKind::IsWellFormed),
        ScalarTextKind::WellFormed(StringWellFormedKind::ToWellFormed),
        ScalarTextKind::Iterator,
    ] {
        state
            .heap
            .set_strong_count_for_test(RawId::Object(unused.object_id()), u32::MAX);
        let result = ScalarTextStep::start_in_state(
            &mut state,
            &runtime.0.poisoned,
            context.realm,
            kind,
            &NativeInvocation::Call {
                this_value: JsValue::Object(receiver.object_id()),
            },
            &arguments,
        );
        state
            .heap
            .set_strong_count_for_test(RawId::Object(unused.object_id()), 1);
        assert!(matches!(
            result,
            Err(RuntimeError::Heap(HeapError::Overflow { .. }))
        ));
        assert_eq!(state.heap.object_strong_count(receiver.object_id()), Ok(1));
        assert!(!runtime.is_poisoned());
    }
}
