//! Terminal publication keeps actual call owners live without a semantic Query.
use super::*;
use crate::engine::{
    code::{
        bytecode::Instruction,
        function::{UnlinkedFunction, metadata::FunctionMetadata},
    },
    heap::{ContextId, ObjectId, ObjectPayload, WeakCollectionKey},
    object::property::PropertyDescriptor,
    vm::{
        call::{CallableExecution, NativeStateGuard},
        execution::ExecutionLimits,
    },
};

fn native_metadata(
    runtime: &Runtime,
    context: &mut crate::engine::api::Context,
    source: &str,
) -> (crate::engine::builtins::native::NativeFunctionId, u8) {
    let callable = runtime
        .callable_from_value(context.eval(source).unwrap())
        .unwrap();
    let CallableExecution::Native {
        target,
        min_readable_args,
        ..
    } = runtime.bytecode_for_callable(&callable).unwrap()
    else {
        panic!("native fixture")
    };
    (target, min_readable_args)
}

fn cycle(context: &mut crate::engine::api::Context) -> ObjectId {
    let Value::Object(value) = context
        .eval("(()=>{let o={};o.self=o;return o})()")
        .unwrap()
    else {
        panic!("cycle fixture")
    };
    value.into_handle()
}

fn cyclic_native(
    state: &mut RuntimeState,
    runtime: &Runtime,
    realm: ContextId,
    target: crate::engine::builtins::native::NativeFunctionId,
    minimum: u8,
) -> ObjectId {
    let prototype = state.heap.context(realm).unwrap().function_prototype;
    let function = state
        .new_native_builtin(
            &runtime.0.poisoned,
            prototype,
            realm,
            target,
            minimum,
            "publication",
            0,
        )
        .unwrap();
    let atom = state
        .pinned_atoms
        .get(crate::engine::atom::pinned::PinnedAtom::Prototype);
    assert!(
        state
            .define_raw_property_with_poison(
                &runtime.0.poisoned,
                function,
                atom,
                &PropertyDescriptor {
                    value: Some(crate::engine::heap::RawValue::Object(function)),
                    writable: Some(true),
                    enumerable: Some(true),
                    configurable: Some(true),
                    ..PropertyDescriptor::new()
                },
            )
            .unwrap()
    );
    function
}

fn weak(state: &mut RuntimeState, realm: ContextId, target: ObjectId) -> ObjectId {
    let prototype = state
        .heap
        .context(realm)
        .unwrap()
        .weak_ref
        .as_ref()
        .unwrap()
        .weak_ref_prototype;
    let shape = state.get_or_create_shape(Some(prototype), &[]).unwrap();
    let object = state
        .heap
        .allocate_weak_ref_object(shape, Vec::new(), WeakCollectionKey::Object(target))
        .unwrap();
    let cleanup = state.heap.release_shape(shape).unwrap();
    state.apply_cleanup(cleanup).unwrap();
    object
}

#[test]
fn terminal_service_preserves_real_throw_callee_argv_and_weak_liveness_before_finish() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    runtime.set_gc_policy(GcPolicy::Manual).unwrap();
    let (target, minimum) = native_metadata(&runtime, &mut context, "Number.prototype.valueOf");
    let receiver = cycle(&mut context);
    let garbage = unreachable_cycle(&runtime, &mut context);
    let owners = std::rc::Rc::strong_count(&runtime.0);
    let mut state = runtime.0.state.borrow_mut();
    let function = cyclic_native(&mut state, &runtime, context.realm, target, minimum);
    let weak_function = weak(&mut state, context.realm, function);
    let weak_receiver = weak(&mut state, context.realm, receiver);
    let argument = state.dup_jsvalue(&JsValue::Object(receiver)).unwrap();
    let call = state
        .prepare_native_call(
            &runtime.0.poisoned,
            runtime.domain_id(),
            function,
            context.realm,
            target,
            minimum,
            NativeInvocation::Call {
                this_value: JsValue::Object(receiver),
            },
            vec![argument],
            NativeInvokeMode::Ordinary,
            true,
            None,
        )
        .unwrap();
    let mut call = NativeStateGuard::new(&mut state, &runtime.0.poisoned, call);
    let (error, completion) = {
        let (state, prepared) = call.parts();
        let NativeStep::CyclePublishedComplete(
            completion @ Completion::Throw(JsValue::Object(error)),
        ) = state
            .invoke_state_native_body(
                &runtime.0.poisoned,
                runtime.0.host_services.as_ref(),
                target,
                context.realm,
                &prepared.invocation,
                &prepared.activation.arguments,
            )
            .unwrap()
        else {
            panic!("actual wrong-brand Error publication")
        };
        assert_eq!(state.active_frames.len(), 1);
        runtime.0.gc_pressure.policy.set(GcPolicy::Automatic);
        runtime.0.gc_pressure.remaining.set(0);
        let completion = state
            .service_native_completion(&runtime.0.gc_pressure, &runtime.0.poisoned, completion)
            .unwrap();
        assert_eq!(
            state.active_frames.len(),
            1,
            "service precedes descriptor restoration"
        );
        assert_eq!(
            state.heap.weak_ref_target(weak_function),
            Ok(Some(WeakCollectionKey::Object(function)))
        );
        assert_eq!(
            state.heap.weak_ref_target(weak_receiver),
            Ok(Some(WeakCollectionKey::Object(receiver)))
        );
        assert_eq!(state.heap.object_strong_count(receiver), Ok(3));
        assert!(state.heap.object(error).is_ok());
        assert!(state.heap.object(garbage).is_err());
        (error, completion)
    };
    let call = call.into_inner();
    let (completion, arguments) =
        call.finish_completion_reusing(&mut state, &runtime.0.poisoned, Ok(completion));
    let completion = completion.unwrap();
    assert!(arguments.is_empty());
    assert_eq!(state.heap.object_strong_count(function), Ok(1));
    assert_eq!(state.heap.object_strong_count(receiver), Ok(1));
    assert!(state.active_frames.is_empty());
    runtime.0.gc_pressure.remaining.set(0);
    state
        .collect_if_requested(&runtime.0.gc_pressure, &runtime.0.poisoned)
        .unwrap();
    assert!(matches!(
        state.heap.object(function),
        Err(crate::engine::heap::HeapError::Stale { .. })
    ));
    assert!(matches!(
        state.heap.object(receiver),
        Err(crate::engine::heap::HeapError::Stale { .. })
    ));
    // Weak pruning precedes trial deletion. The freed cycle's non-owning
    // identity is cleared by the next actual weak pass, not this cycle pass.
    runtime.0.gc_pressure.remaining.set(0);
    state
        .collect_if_requested(&runtime.0.gc_pressure, &runtime.0.poisoned)
        .unwrap();
    assert_eq!(state.heap.weak_ref_target(weak_function), Ok(None));
    assert_eq!(state.heap.weak_ref_target(weak_receiver), Ok(None));
    assert!(matches!(completion, Completion::Throw(JsValue::Object(id)) if id == error));
    assert_eq!(std::rc::Rc::strong_count(&runtime.0), owners);
    for value in [
        JsValue::Object(weak_function),
        JsValue::Object(weak_receiver),
    ] {
        state
            .release_owned_jsvalue(&runtime.0.poisoned, value)
            .unwrap();
    }
    drop_completion(&mut state, &runtime, completion);
}

#[test]
fn terminal_service_keeps_a_fresh_result_alias_owned_until_its_consumer_retires_it() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    runtime.set_gc_policy(GcPolicy::Manual).unwrap();
    let aliased = cycle(&mut context);
    let mut state = runtime.0.state.borrow_mut();
    let result = state
        .new_ordinary_object_in_realm(&runtime.0.poisoned, context.realm)
        .unwrap();
    let atom = state
        .pinned_atoms
        .get(crate::engine::atom::pinned::PinnedAtom::Prototype);
    assert!(
        state
            .define_raw_property_with_poison(
                &runtime.0.poisoned,
                result,
                atom,
                &PropertyDescriptor {
                    value: Some(crate::engine::heap::RawValue::Object(aliased)),
                    writable: Some(true),
                    enumerable: Some(true),
                    configurable: Some(true),
                    ..PropertyDescriptor::new()
                }
            )
            .unwrap()
    );
    state
        .release_owned_jsvalue(&runtime.0.poisoned, JsValue::Object(aliased))
        .unwrap();
    runtime.0.gc_pressure.policy.set(GcPolicy::Automatic);
    runtime.0.gc_pressure.remaining.set(0);
    let completion = state
        .service_native_completion(
            &runtime.0.gc_pressure,
            &runtime.0.poisoned,
            Completion::Return(JsValue::Object(result)),
        )
        .unwrap();
    assert!(matches!(completion, Completion::Return(JsValue::Object(id)) if id == result));
    assert!(
        state.heap.object(aliased).is_ok(),
        "the real result slot owns this alias"
    );
    drop_completion(&mut state, &runtime, completion);
    runtime.0.gc_pressure.remaining.set(0);
    state
        .collect_if_requested(&runtime.0.gc_pressure, &runtime.0.poisoned)
        .unwrap();
    assert!(state.heap.object(aliased).is_err());
}

fn method_frame(
    runtime: &Runtime,
    context: &mut crate::engine::api::Context,
    tail: bool,
) -> (RunningExecution, crate::engine::vm::frame::FrameId, usize) {
    let prefix = if tail { 4 } else { 5 };
    let mut instructions = vec![Instruction::Undefined; prefix];
    instructions.push(if tail {
        Instruction::TailCallMethod(2)
    } else {
        Instruction::CallMethod(2)
    });
    if !tail {
        instructions.push(Instruction::Return);
    }
    let bytecode = runtime
        .publish_unlinked_function(
            context.realm,
            UnlinkedFunction::fixture(
                instructions,
                Vec::new(),
                FunctionMetadata {
                    max_stack: prefix as u16,
                    strict: true,
                    ..Default::default()
                },
            ),
        )
        .unwrap();
    let caller = runtime
        .new_bytecode_closure(context.realm, &bytecode)
        .unwrap();
    let CallableExecution::Bytecode {
        bytecode,
        closure_slots,
    } = runtime.bytecode_for_callable(&caller).unwrap()
    else {
        panic!("published caller")
    };
    let entry = crate::engine::vm::root_call::prepare_call(
        runtime,
        context.realm,
        &caller,
        JsValue::Undefined,
        JsValue::Undefined,
        Vec::new(),
        bytecode,
        closure_slots,
    )
    .unwrap();
    let mut execution = RunningExecution::new(runtime, ExecutionLimits::default()).unwrap();
    let id = crate::engine::vm::driver::push_frame(runtime, &mut execution, entry).unwrap();
    let frame = execution.frames.current_mut(id).unwrap();
    let pc = frame.executable.exec.exec_pc(prefix as u32).unwrap() as usize;
    let actual = frame
        .executable
        .exec
        .decode_published(pc as u32)
        .unwrap()
        .opcode;
    assert_eq!(
        actual,
        if tail {
            Opcode::TailCallMethod
        } else {
            Opcode::CallMethod
        }
    );
    frame.fault_pc = pc;
    frame.resume_pc = pc;
    (execution, id, pc)
}

#[test]
fn resident_terminal_return_and_throw_use_real_call_and_tail_opcodes_without_a_query() {
    for thrown in [false, true] {
        for tail in [false, true] {
            let runtime = Runtime::new();
            let mut context = runtime.new_context().unwrap();
            runtime.set_gc_policy(GcPolicy::Manual).unwrap();
            let source = if thrown {
                "Number.prototype.valueOf"
            } else {
                "String.prototype[Symbol.iterator]"
            };
            let (target, minimum) = native_metadata(&runtime, &mut context, source);
            let marker = cycle(&mut context);
            let receiver = if thrown {
                runtime.dup_jsvalue(&JsValue::Object(marker)).unwrap()
            } else {
                runtime
                    .into_jsvalue(Value::String(JsString::from_static("abc")))
                    .unwrap()
            };
            let (mut execution, id, pc) = method_frame(&runtime, &mut context, tail);
            let function = {
                let mut state = runtime.0.state.borrow_mut();
                cyclic_native(&mut state, &runtime, context.realm, target, minimum)
            };
            let argument = runtime.dup_jsvalue(&JsValue::Object(marker)).unwrap();
            let frame = execution.frames.current_mut(id).unwrap();
            if !tail {
                execution
                    .slots
                    .push(&mut frame.window, JsValue::Int(99))
                    .unwrap();
            }
            for value in [
                receiver,
                JsValue::Object(function),
                JsValue::Object(marker),
                argument,
            ] {
                execution.slots.push(&mut frame.window, value).unwrap();
            }
            assert!(!execution.query_storage.has_cached_entry());
            runtime.0.gc_pressure.policy.set(GcPolicy::Automatic);
            runtime.0.gc_pressure.remaining.set(0);
            #[cfg(feature = "profiling")]
            let profile = crate::engine::api::profiling::CostProfile::start();
            let mut state = runtime.0.state.borrow_mut();
            let action = execute_frame_in_state(&runtime, &mut state, &mut execution, id).unwrap();
            assert!(
                !execution.query_storage.has_cached_entry(),
                "terminal fact must not acquire/recycle a Query"
            );
            assert!(
                state.heap.object(function).is_ok(),
                "callee was still a root during service"
            );
            assert_eq!(state.heap.object_strong_count(function), Ok(1));
            assert_eq!(
                state.heap.object_strong_count(marker),
                Ok(1),
                "only cycle remains after actual argv/this finish"
            );
            assert!(!runtime.0.gc_pressure.requested());
            if thrown {
                assert!(matches!(action, VmAction::Throw));
                let frame = execution.frames.current_mut(id).unwrap();
                assert_eq!((frame.fault_pc, frame.resume_pc), (pc, pc));
                if !tail {
                    assert_eq!(
                        execution.slots.peek(&frame.window, 1).unwrap(),
                        &JsValue::Int(99)
                    );
                }
                let JsValue::Object(error) = execution.slots.peek(&frame.window, 0).unwrap() else {
                    panic!("thrown Error")
                };
                assert!(matches!(
                    state.heap.object(*error).unwrap().payload,
                    ObjectPayload::Error
                ));
            } else {
                assert!(matches!(action, VmAction::Complete));
                let Some(JsValue::Object(result)) = execution.pending.as_ref() else {
                    panic!("iterator owner")
                };
                assert!(matches!(
                    state.heap.object(*result).unwrap().payload,
                    ObjectPayload::StringIterator { .. }
                ));
            }
            #[cfg(feature = "profiling")]
            {
                let events = profile.snapshot().owned_execution_events;
                assert_eq!(events.get("gc.automatic.started"), Some(&1));
                assert_eq!(events.get("query_storage_new"), None);
                assert_eq!(events.get("query_storage_reused"), None);
            }
            runtime.0.gc_pressure.remaining.set(0);
            state
                .collect_if_requested(&runtime.0.gc_pressure, &runtime.0.poisoned)
                .unwrap();
            assert!(state.heap.object(function).is_err());
            assert!(state.heap.object(marker).is_err());
            assert!(!runtime.is_poisoned());
        }
    }
}

#[test]
fn cold_local_terminal_factory_finishes_before_allocating_wait_storage() {
    for (source, input, thrown) in [
        (
            "String.prototype[Symbol.iterator]",
            Value::String(JsString::from_static("abc")),
            false,
        ),
        ("Number.prototype.valueOf", Value::Undefined, true),
    ] {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        runtime.set_gc_policy(GcPolicy::Manual).unwrap();
        let garbage = unreachable_cycle(&runtime, &mut context);
        let callable = runtime
            .callable_from_value(context.eval(source).unwrap())
            .unwrap();
        let mut selected =
            crate::engine::vm::frames::NativeClassification::select(&runtime, &callable)
                .unwrap()
                .unwrap();
        let target = selected.target();
        let realm = selected.defining_realm();
        let minimum = selected.minimum();
        let kind = selected.take_operation().unwrap();
        let receiver = runtime.into_jsvalue(input).unwrap();
        let mut slots = crate::engine::vm::stack::SlotStore::new(32);
        let mut storage = QueryStorage::default();
        runtime.0.gc_pressure.policy.set(GcPolicy::Automatic);
        runtime.0.gc_pressure.remaining.set(0);
        #[cfg(feature = "profiling")]
        let profile = crate::engine::api::profiling::CostProfile::start();
        let result = super::super::native::begin_local(
            &runtime,
            &mut slots,
            &mut storage,
            context.realm,
            callable,
            target,
            realm,
            minimum,
            receiver,
            Vec::new(),
            kind,
            Some(selected),
            true,
        )
        .unwrap();
        let super::super::native::LocalNativeResult::Complete(completion) = result else {
            panic!("terminal completion is not a wait")
        };
        assert_eq!(matches!(completion, Completion::Throw(_)), thrown);
        assert!(!storage.has_cached_entry());
        let mut state = runtime.0.state.borrow_mut();
        assert!(state.heap.object(garbage).is_err());
        assert!(state.active_frames.is_empty());
        assert!(!runtime.0.gc_pressure.requested());
        #[cfg(feature = "profiling")]
        {
            let snapshot = profile.snapshot();
            assert_eq!(
                snapshot.owned_execution_events.get("gc.automatic.started"),
                Some(&1)
            );
            assert!(
                !snapshot
                    .call_buffers
                    .contains_key("query.native_wait_payload")
            );
        }
        drop_completion(&mut state, &runtime, completion);
    }
}

#[test]
fn terminal_service_failure_quarantines_before_native_descriptor_and_suffix_retirement() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    runtime.set_gc_policy(GcPolicy::Manual).unwrap();
    let (target, minimum) = native_metadata(&runtime, &mut context, "Number.prototype.valueOf");
    let receiver = cycle(&mut context);
    let older = runtime.new_object(None).unwrap().into_handle();
    let later = runtime.new_object(None).unwrap().into_handle();
    let mut state = runtime.0.state.borrow_mut();
    let function = cyclic_native(&mut state, &runtime, context.realm, target, minimum);
    let argument = state.dup_jsvalue(&JsValue::Object(receiver)).unwrap();
    let call = state
        .prepare_native_call(
            &runtime.0.poisoned,
            runtime.domain_id(),
            function,
            context.realm,
            target,
            minimum,
            NativeInvocation::Call {
                this_value: JsValue::Object(receiver),
            },
            vec![argument],
            NativeInvokeMode::Ordinary,
            true,
            None,
        )
        .unwrap();
    let mut call = NativeStateGuard::new(&mut state, &runtime.0.poisoned, call);
    let error = {
        let (state, prepared) = call.parts();
        let NativeStep::CyclePublishedComplete(
            completion @ Completion::Throw(JsValue::Object(error)),
        ) = state
            .invoke_state_native_body(
                &runtime.0.poisoned,
                runtime.0.host_services.as_ref(),
                target,
                context.realm,
                &prepared.invocation,
                &prepared.activation.arguments,
            )
            .unwrap()
        else {
            panic!("actual Error factory")
        };
        state
            .heap
            .queue_release_for_test(RawId::Object(older))
            .unwrap();
        state
            .heap
            .queue_release_for_test(RawId::Object(later))
            .unwrap();
        state
            .heap
            .set_strong_count_for_test(RawId::Object(older), 1);
        runtime.0.gc_pressure.policy.set(GcPolicy::Automatic);
        runtime.0.gc_pressure.remaining.set(0);
        assert!(
            state
                .service_native_completion(&runtime.0.gc_pressure, &runtime.0.poisoned, completion)
                .is_err()
        );
        assert!(runtime.is_poisoned());
        assert_eq!(state.heap.object_strong_count(error), Ok(1));
        assert_eq!(state.heap.object_strong_count(function), Ok(2));
        assert_eq!(state.heap.object_strong_count(receiver), Ok(3));
        assert_eq!(state.heap.object_strong_count(later), Ok(0));
        assert_eq!(state.active_frames.len(), 1);
        error
    };
    drop(call);
    assert_eq!(state.heap.object_strong_count(error), Ok(1));
    assert_eq!(state.heap.object_strong_count(receiver), Ok(3));
    assert_eq!(state.active_frames.len(), 1);
}

#[test]
fn successful_terminal_service_does_not_hide_fatal_callee_cleanup_or_retire_argv_suffix() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    runtime.set_gc_policy(GcPolicy::Manual).unwrap();
    let (target, minimum) = native_metadata(&runtime, &mut context, "Number.prototype.valueOf");
    let receiver = cycle(&mut context);
    let bad = runtime.new_object(None).unwrap().into_handle();
    let mut state = runtime.0.state.borrow_mut();
    let prototype = state
        .heap
        .context(context.realm)
        .unwrap()
        .function_prototype;
    let function = state
        .new_native_builtin(
            &runtime.0.poisoned,
            prototype,
            context.realm,
            target,
            minimum,
            "cleanup",
            0,
        )
        .unwrap();
    let atom = state
        .pinned_atoms
        .get(crate::engine::atom::pinned::PinnedAtom::Prototype);
    assert!(
        state
            .define_raw_property_with_poison(
                &runtime.0.poisoned,
                function,
                atom,
                &PropertyDescriptor {
                    value: Some(crate::engine::heap::RawValue::Object(bad)),
                    writable: Some(true),
                    enumerable: Some(true),
                    configurable: Some(true),
                    ..PropertyDescriptor::new()
                }
            )
            .unwrap()
    );
    state
        .release_owned_jsvalue(&runtime.0.poisoned, JsValue::Object(bad))
        .unwrap();
    let argument = state.dup_jsvalue(&JsValue::Object(receiver)).unwrap();
    let call = state
        .prepare_native_call(
            &runtime.0.poisoned,
            runtime.domain_id(),
            function,
            context.realm,
            target,
            minimum,
            NativeInvocation::Call {
                this_value: JsValue::Object(receiver),
            },
            vec![argument],
            NativeInvokeMode::Ordinary,
            true,
            None,
        )
        .unwrap();
    let mut call = NativeStateGuard::new(&mut state, &runtime.0.poisoned, call);
    let (error, completion) = {
        let (state, prepared) = call.parts();
        let NativeStep::CyclePublishedComplete(
            completion @ Completion::Throw(JsValue::Object(error)),
        ) = state
            .invoke_state_native_body(
                &runtime.0.poisoned,
                runtime.0.host_services.as_ref(),
                target,
                context.realm,
                &prepared.invocation,
                &prepared.activation.arguments,
            )
            .unwrap()
        else {
            panic!("actual Error producer")
        };
        runtime.0.gc_pressure.policy.set(GcPolicy::Automatic);
        runtime.0.gc_pressure.remaining.set(0);
        let completion = state
            .service_native_completion(&runtime.0.gc_pressure, &runtime.0.poisoned, completion)
            .unwrap();
        assert!(!runtime.is_poisoned());
        assert_eq!(state.heap.object_strong_count(receiver), Ok(3));
        // Corrupt only the first real final-callee child after successful GC.
        // This reaches destructive retirement rather than stale root release.
        state.heap.set_strong_count_for_test(RawId::Object(bad), 0);
        (error, completion)
    };
    let call = call.into_inner();
    let (result, _) =
        call.finish_completion_reusing(&mut state, &runtime.0.poisoned, Ok(completion));
    assert!(result.is_err());
    assert!(runtime.is_poisoned());
    assert_eq!(
        state.heap.object_strong_count(receiver),
        Ok(3),
        "both actual input/argv owners remain quarantined"
    );
    assert_eq!(state.heap.object_strong_count(error), Ok(1));
    assert!(
        state.active_frames.is_empty(),
        "existing descriptor restoration still precedes callee cleanup"
    );
}
