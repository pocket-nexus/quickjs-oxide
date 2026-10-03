//! The ordinary call/return loop's only entry and destruction transactions.
#[cfg(test)]
mod native_preparation_tests;

use crate::engine::{
    api::{Error, runtime::Runtime},
    vm::{
        call::ordinary::DirectSelection,
        exception::runtime_error_to_vm_error,
        execution::RunningExecution,
        frame::{FrameId, ReturnValue},
    },
};

pub(in crate::engine::vm) enum Entry {
    Ordinary,
    Native(super::CallStep),
    NativeReady,
    NativeComplete,
    NativeThrow,
    General,
}

pub(in crate::engine::vm) enum StateCall {
    Ordinary(
        crate::engine::vm::call::ordinary::OrdinaryCall,
        crate::engine::vm::stack::CheckedOrdinaryCallOperands,
    ),
    Native(crate::engine::vm::frames::NativeClassification),
}

#[cfg(all(test, feature = "profiling"))]
pub(super) fn enter(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    id: FrameId,
    count: u16,
    method: bool,
    tail: bool,
) -> Result<Entry, Error> {
    let frame = execution.frames.current_mut(id)?;
    let decoded = frame
        .executable
        .exec
        .decode_published(frame.fault_pc as u32)
        .map_err(|_| Error::internal("test ordinary call has no instruction"))?;
    let fallthrough = crate::engine::vm::execute::FallthroughPc::from_decoded(decoded);
    enter_selected(
        runtime,
        execution,
        id,
        count,
        method,
        tail,
        None,
        fallthrough,
    )
}

/// Preflight one ordinary call through an already admitted slot transaction.
/// Selection and authentication read metadata and execute no JavaScript; every
/// source edge stays in its caller slot until the lease's installer consumes it.
#[allow(clippy::too_many_arguments)]
pub(in crate::engine::vm) fn prepare_ordinary_in_state(
    runtime: &Runtime,
    state: &crate::engine::heap::runtime::RuntimeState,
    transaction: &crate::engine::vm::stack::FrameTransaction<'_>,
    executable: &crate::engine::code::runtime::PublishedFunctionSnapshot,
    fault_pc: usize,
    count: usize,
    method: bool,
    native_hint: &mut Option<crate::engine::object::LinkedNativeSelection>,
) -> Result<Option<StateCall>, Error> {
    transaction.peek(count + usize::from(method))?;
    let callable = transaction.peek(count)?;
    #[cfg(feature = "profiling")]
    crate::engine::api::profiling::record_callsite_callee(runtime, executable, fault_pc, callable);
    #[cfg(not(feature = "profiling"))]
    let _ = (executable, fault_pc);
    let crate::engine::value::JsValue::Object(function) = callable else {
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_execution_event("core.call_decline.general");
        return Ok(None);
    };
    if native_hint
        .as_ref()
        .is_some_and(|hint| hint.matches_in_domain(runtime.domain_id(), *function))
    {
        let hint = native_hint.as_ref().expect("matched native hint");
        if crate::engine::heap::runtime::RuntimeState::has_state_native_body(hint.target()) {
            transaction.validate_call_value_domains(runtime, count, method)?;
            let selected = crate::engine::vm::frames::NativeClassification::classify_linked(
                runtime,
                native_hint.take().expect("matched native hint"),
                callable,
            )
            .ok_or_else(|| Error::internal("linked native classification lost its callee"))?;
            return Ok(Some(StateCall::Native(selected)));
        }
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_execution_event(
            "core.call_decline.native_hint",
        );
        return Ok(None);
    }
    let selected = match DirectSelection::select_in_state(runtime, state, *function) {
        Ok(DirectSelection::Ordinary(selected)) => selected,
        Ok(DirectSelection::General) => {
            #[cfg(feature = "profiling")]
            crate::engine::api::profiling::record_owned_execution_event(
                "core.call_decline.general",
            );
            return Ok(None);
        }
        Ok(DirectSelection::Native(native)) => {
            let hint = crate::engine::object::LinkedNativeSelection::from_direct_native(native);
            if crate::engine::heap::runtime::RuntimeState::has_state_native_body(hint.target()) {
                transaction.validate_call_value_domains(runtime, count, method)?;
                let selected = crate::engine::vm::frames::NativeClassification::classify_linked(
                    runtime, hint, callable,
                )
                .ok_or_else(|| Error::internal("native classification lost its callee"))?;
                return Ok(Some(StateCall::Native(selected)));
            }
            *native_hint = Some(hint);
            #[cfg(feature = "profiling")]
            crate::engine::api::profiling::record_owned_execution_event("core.call_decline.native");
            return Ok(None);
        }
        Err(error) => {
            #[cfg(feature = "profiling")]
            crate::engine::api::profiling::record_owned_execution_event(
                "core.call_decline.authentication_error",
            );
            transaction.validate_call_value_domains(runtime, count, method)?;
            return Err(runtime_error_to_vm_error(error));
        }
    };
    let checked = transaction.validate_ordinary_call_operands(count, method)?;
    let call = selected
        .authenticate_slot_in_state(runtime, state)
        .map_err(|error| {
            #[cfg(feature = "profiling")]
            crate::engine::api::profiling::record_owned_execution_event(
                "core.call_decline.authentication_error",
            );
            runtime_error_to_vm_error(error)
        })?;
    Ok(Some(StateCall::Ordinary(call, checked)))
}

/// Test legacy entry still authenticates its supplied frame and window once.
#[cfg(test)]
#[allow(clippy::too_many_arguments)]
pub(in crate::engine::vm) fn enter_selected_in_state(
    runtime: &Runtime,
    state: &mut crate::engine::heap::runtime::RuntimeState,
    execution: &mut RunningExecution,
    id: FrameId,
    count: u16,
    method: bool,
    tail: bool,
    selected_native: Option<crate::engine::object::LinkedNativeSelection>,
    fallthrough: crate::engine::vm::execute::FallthroughPc,
) -> Result<Entry, Error> {
    if selected_native.is_some() {
        return Ok(Entry::General);
    }
    crate::engine::vm::stack::FrameExecution::admit(execution, id)?.enter_ordinary(
        runtime,
        state,
        count,
        method,
        tail,
        fallthrough,
    )
}

// These explicit drops end the authenticated slot lease before installing a
// child or entering native code; keep the boundary visible to reviewers.
#[allow(clippy::drop_non_drop, clippy::too_many_arguments)]
pub(super) fn enter_selected(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    id: FrameId,
    count: u16,
    method: bool,
    tail: bool,
    selected_native: Option<crate::engine::object::LinkedNativeSelection>,
    fallthrough: crate::engine::vm::execute::FallthroughPc,
) -> Result<Entry, Error> {
    #[cfg(feature = "profiling")]
    let _sample = crate::engine::api::profiling::VmCallSample::enter();
    // End this scope before executing native code or installing the child.
    // It covers the direct driver, unlike the legacy bytecode.prepare timer.
    #[cfg(feature = "profiling")]
    let prepare_timer =
        crate::engine::api::profiling::PhaseTimer::start_vm_sampled("direct.prepare.sampled");
    let logical_depth = execution.frames.logical_active_depth(runtime);
    let frame = execution.frames.current_mut(id)?;
    let count = usize::from(count);
    let depth = execution.slots.depth(&frame.window);
    #[cfg(feature = "profiling")]
    let profile_pc = frame.fault_pc;
    #[cfg(feature = "profiling")]
    let profile_body = &mut *frame.cold;
    #[cfg(feature = "profiling")]
    let mut transaction = execution
        .slots
        .frame_transaction(&mut profile_body.window)?;
    #[cfg(not(feature = "profiling"))]
    let mut transaction = execution.slots.frame_transaction(&mut frame.window)?;
    let entry_operand = transaction.peek(count + usize::from(method))?;
    enum Prepared {
        Ordinary(
            crate::engine::vm::call::ordinary::OrdinaryCall,
            crate::engine::vm::stack::CheckedOrdinaryCallOperands,
        ),
        Native(crate::engine::vm::frames::NativeClassification),
    }
    // End every Result/selection container holding a slot borrow before any
    // frame installation or operand transfer. Native facts remain pinned by
    // the callee slot until that same owner is transferred below; the ordinary
    // operand proof instead leaves this transaction for frame installation.
    let prepared = if let Some(selected) = selected_native {
        if !transaction.validate_call_value_domains(runtime, count, method)? {
            return Ok(Entry::General);
        }
        let linked = if method {
            transaction.peek(count)?
        } else {
            entry_operand
        };
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_callsite_callee(
            runtime,
            &profile_body.executable,
            profile_pc,
            linked,
        );
        let Some(selected) = crate::engine::vm::frames::NativeClassification::classify_linked(
            runtime, selected, linked,
        ) else {
            return Ok(Entry::General);
        };
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_execution_event(
            "native_linked_classification_consumed",
        );
        Prepared::Native(selected)
    } else {
        let callable_value = if method {
            transaction.peek(count)?
        } else {
            entry_operand
        };
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_callsite_callee(
            runtime,
            &profile_body.executable,
            profile_pc,
            callable_value,
        );
        let selection_result = {
            #[cfg(feature = "profiling")]
            let _timer = crate::engine::api::profiling::PhaseTimer::start_vm_sampled(
                "direct.select.sampled",
            );
            DirectSelection::select_jsvalue(runtime, callable_value)
        };
        match selection_result {
            Ok(DirectSelection::General) => return Ok(Entry::General),
            Ok(DirectSelection::Ordinary(ordinary)) => {
                // The sealed proof is consumed by the immediately following
                // ordinary installation. Authentication does not touch caller
                // slots or reenter JavaScript; metadata errors still follow
                // the original operand-domain error order.
                let checked = {
                    #[cfg(feature = "profiling")]
                    let _timer = crate::engine::api::profiling::PhaseTimer::start_vm_sampled(
                        "ordinary.validate.sampled",
                    );
                    transaction.validate_ordinary_call_operands(count, method)?
                };
                let call = {
                    #[cfg(feature = "profiling")]
                    let _timer = crate::engine::api::profiling::PhaseTimer::start_vm_sampled(
                        "ordinary.authenticate.sampled",
                    );
                    ordinary
                        .authenticate_slot(runtime)
                        .map_err(runtime_error_to_vm_error)?
                };
                Prepared::Ordinary(call, checked)
            }
            Ok(DirectSelection::Native(native)) => {
                if !transaction.validate_call_value_domains(runtime, count, method)? {
                    return Ok(Entry::General);
                }
                let selected =
                    crate::engine::vm::frames::NativeClassification::classify_selected(native);
                Prepared::Native(selected)
            }
            Err(error) => {
                if !transaction.validate_call_value_domains(runtime, count, method)? {
                    return Ok(Entry::General);
                }
                return Err(runtime_error_to_vm_error(error));
            }
        }
    };
    #[cfg(feature = "profiling")]
    drop(prepare_timer);
    match prepared {
        Prepared::Ordinary(call, checked) => {
            drop(transaction);
            if !execution.frames.can_push() || runtime.bytecode_call_would_overflow() {
                return Ok(Entry::General);
            }
            call.install(runtime, execution, id, checked, tail, fallthrough)?;
            #[cfg(feature = "profiling")]
            crate::engine::api::profiling::record_owned_execution_event(
                "ordinary_call.carried_fallthrough",
            );
            #[cfg(feature = "profiling")]
            crate::engine::api::profiling::record_owned_instruction(depth);
            Ok(Entry::Ordinary)
        }
        Prepared::Native(mut selected) => {
            let target = selected.target();
            let realm = selected.defining_realm();
            let minimum = selected.minimum();
            let operation = selected.take_operation();
            let (arguments, receiver, callable) = transaction.take_validated_native_call_operands(
                runtime,
                logical_depth,
                count,
                method,
            )?;
            let operands = crate::engine::vm::call::NativeCallGuard::from_operands(
                runtime,
                callable,
                realm,
                target,
                crate::engine::vm::call::NativeInvokeMode::Ordinary,
                crate::engine::vm::call::NativeInvocation::Call {
                    this_value: receiver,
                },
                arguments,
            );
            drop(transaction);
            if !execution.frames.can_push_with_continuations(0)
                || runtime.host_stack_would_overflow()
                || runtime.0.deferred_references.has_pending()
                || native_observes_activation(
                    runtime,
                    target,
                    operands.invocation.input(),
                    &operands.activation.arguments.readable,
                )
            {
                execution.frames.materialize(runtime)?;
            } else {
                #[cfg(feature = "profiling")]
                crate::engine::api::profiling::record_owned_execution_event(
                    "native_unobserved_entry",
                );
            }
            let result = super::super::proxy_get_driver::start_native_owned(
                runtime,
                execution,
                id,
                operands,
                minimum,
                tail,
                depth,
                Some(selected),
                operation,
            );
            if result.is_err() {
                // Resource/invariant errors from a proven NoJS leaf carry no
                // eagerly constructed JS exception. Publish its caller before
                // propagating the error through the ordinary error boundary.
                execution.frames.materialize(runtime)?;
            }
            let result = result?;
            if matches!(result, super::CallStep::Entered)
                && execution.frames.current_id() == Some(id)
                && !execution.frames.current_mut(id)?.cold.has_pending_query()
                && execution.pending.is_none()
            {
                Ok(Entry::NativeReady)
            } else {
                Ok(Entry::Native(result))
            }
        }
    }
}

/// Prove non-observation from the selected immutable native identity and actual
/// inputs. Every callback-capable or throwing conversion retains publication.
fn native_observes_activation(
    runtime: &Runtime,
    target: crate::engine::builtins::native::NativeFunctionId,
    receiver: &crate::engine::value::JsValue,
    arguments: &[crate::engine::value::JsValue],
) -> bool {
    native_observes_activation_in_state(&runtime.0.state.borrow(), target, receiver, arguments)
}

pub(in crate::engine::vm) fn native_observes_activation_in_state(
    state: &crate::engine::heap::runtime::RuntimeState,
    target: crate::engine::builtins::native::NativeFunctionId,
    receiver: &crate::engine::value::JsValue,
    arguments: &[crate::engine::value::JsValue],
) -> bool {
    use crate::engine::{
        builtins::native::{
            MapNativeKind as M, NativeFunctionId as N, SetNativeKind as S, WeakMapNativeKind as W,
            WeakSetNativeKind as WS,
        },
        heap::ObjectPayload,
        value::JsValue,
    };
    if matches!(target, N::NumberPredicate(_)) {
        return false;
    }
    if matches!(
        target,
        N::MathUnary(_)
            | N::MathBinary(_)
            | N::MathMinMax(_)
            | N::MathHypot
            | N::MathImul
            | N::MathClz32
    ) {
        return arguments.iter().any(|value| {
            !matches!(
                value,
                JsValue::Int(_)
                    | JsValue::Float(_)
                    | JsValue::Bool(_)
                    | JsValue::Null
                    | JsValue::Undefined
            )
        });
    }
    let JsValue::Object(object) = receiver else {
        return true;
    };
    let Ok(object) = state.heap.object(*object) else {
        return true;
    };
    !match (target, &object.payload) {
        (N::Map(M::Set | M::Get | M::Has | M::Delete | M::Clear), ObjectPayload::Map { .. }) => {
            true
        }
        (N::Set(S::Add | S::Has | S::Delete | S::Clear), ObjectPayload::Set { .. }) => true,
        (N::WeakMap(W::Get | W::Has | W::Delete), ObjectPayload::WeakMap { .. }) => true,
        (N::WeakMap(W::Set), ObjectPayload::WeakMap { .. }) => {
            matches!(arguments.first(), Some(JsValue::Object(_)))
        }
        (N::WeakSet(WS::Has | WS::Delete), ObjectPayload::WeakSet { .. }) => true,
        (N::WeakSet(WS::Add), ObjectPayload::WeakSet { .. }) => {
            matches!(arguments.first(), Some(JsValue::Object(_)))
        }
        _ => false,
    }
}

pub(in crate::engine::vm) enum ReturnProgress {
    Declined,
    Returned,
    Property(super::CallStep),
}
/// Test legacy return admission keeps malformed frame/window entries checked.
#[cfg(test)]
pub(in crate::engine::vm) fn finish_in_state(
    runtime: &Runtime,
    state: &mut crate::engine::heap::runtime::RuntimeState,
    execution: &mut RunningExecution,
    id: FrameId,
) -> Result<ReturnProgress, Error> {
    crate::engine::vm::stack::FrameExecution::admit(execution, id)?.finish_ordinary(runtime, state)
}

pub(super) fn finish(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    id: FrameId,
) -> Result<ReturnProgress, Error> {
    let frame = execution.frames.current_mut(id)?;
    let Some(target) = frame.cold.ordinary_return() else {
        return Ok(ReturnProgress::Declined);
    };
    if target.operation.is_some() && !execution.frames.can_reply_property_directly(target) {
        return Ok(ReturnProgress::Declined);
    }
    if execution.pending.is_none() {
        return Ok(ReturnProgress::Declined);
    }
    // Result ownership precedes window clearing and activation removal.
    let frame = execution.frames.pop(id)?;
    let mut frame =
        crate::engine::vm::frame::RetiredFrame::new(runtime, &mut execution.slots, frame);
    frame.clear_window()?;
    frame
        .recycle(&mut execution.call_storage)
        .map_err(runtime_error_to_vm_error)?;
    if target.operation.is_some() {
        let value = execution.pending.take().expect("retired property return");
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_execution_event("property_return_direct");
        return match crate::engine::vm::proxy_get_driver::reply(
            runtime,
            execution,
            target,
            crate::engine::vm::Completion::Return(value),
        )? {
            crate::engine::vm::proxy_get_driver::Progress::Call(step) => {
                Ok(ReturnProgress::Property(step))
            }
            crate::engine::vm::proxy_get_driver::Progress::Conversion(_) => {
                Err(Error::internal("property read returned conversion"))
            }
        };
    }
    let parent = execution.frames.current_mut(target.frame()?)?;
    if matches!(target.value_use, ReturnValue::Push) {
        let value = execution.pending.as_mut().expect("retired ordinary return");
        execution.slots.push_owned(&mut parent.window, value)?;
        execution.pending = None;
    } else {
        let value = execution.pending.take().expect("discarded ordinary return");
        runtime
            .release_jsvalue(value)
            .map_err(runtime_error_to_vm_error)?;
    }
    #[cfg(feature = "profiling")]
    crate::engine::api::profiling::record_owned_execution_event("ordinary_return_direct");
    Ok(ReturnProgress::Returned)
}

#[cfg(test)]
mod layout_tests {
    #[test]
    fn unhinted_native_callee_is_selected_once_and_keeps_its_slot_owner() {
        use crate::engine::{
            api::Runtime,
            value::JsValue,
            vm::{
                call::CallableExecution,
                execute::{VmAction, execute_frame_in_state},
                execution::{ExecutionLimits, RunningExecution},
            },
        };
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        let callable = runtime
            .callable_from_value(
                context
                    .eval("(function invoke(fn,arg){let result=fn(arg,9);return result})")
                    .unwrap(),
            )
            .unwrap();
        let CallableExecution::Bytecode {
            bytecode,
            closure_slots,
        } = runtime.bytecode_for_callable(&callable).unwrap()
        else {
            unreachable!()
        };
        let callee = runtime
            .into_jsvalue(context.eval("Math.min").unwrap())
            .unwrap();
        let JsValue::Object(function) = callee else {
            unreachable!()
        };
        let entry = crate::engine::vm::root_call::prepare_call(
            &runtime,
            context.realm,
            &callable,
            JsValue::Undefined,
            JsValue::Undefined,
            vec![callee, JsValue::Int(7)],
            bytecode,
            closure_slots,
        )
        .unwrap();
        let mut execution = RunningExecution::new(&runtime, ExecutionLimits::default()).unwrap();
        let parent =
            crate::engine::vm::driver::push_frame(&runtime, &mut execution, entry).unwrap();
        assert!(execution.selected_native.is_none());
        let runtime_owners = std::rc::Rc::strong_count(&runtime.0);
        #[cfg(feature = "profiling")]
        let profile = crate::engine::api::profiling::CostProfile::start();
        let action = {
            let mut state = runtime.0.state.borrow_mut();
            execute_frame_in_state(&runtime, &mut state, &mut execution, parent).unwrap()
        };
        let VmAction::Call {
            arguments,
            method,
            tail,
            fallthrough,
        } = action
        else {
            panic!("native call exits the ordinary execution segment")
        };
        assert_eq!(arguments, 2);
        assert!(!method && !tail);
        let selected = execution
            .selected_native
            .take()
            .expect("carried native fact");
        assert!(selected.matches_in_domain(runtime.domain_id(), function));
        assert!(!selected.matches_in_domain(runtime.domain_id().wrapping_add(1), function));
        let other = runtime.new_object(None).unwrap();
        assert!(!selected.matches_in_domain(runtime.domain_id(), other.object_id()));
        drop(other);
        assert_eq!(std::rc::Rc::strong_count(&runtime.0), runtime_owners);
        let frame = execution.frames.current_mut(parent).unwrap();
        assert_eq!(
            execution
                .slots
                .peek(&frame.window, usize::from(arguments))
                .unwrap(),
            &JsValue::Object(function),
        );
        assert!(matches!(
            super::enter_selected(
                &runtime,
                &mut execution,
                parent,
                arguments,
                method,
                tail,
                Some(selected),
                fallthrough,
            )
            .unwrap(),
            super::Entry::NativeReady
        ));
        {
            let mut state = runtime.0.state.borrow_mut();
            assert!(matches!(
                execute_frame_in_state(&runtime, &mut state, &mut execution, parent).unwrap(),
                VmAction::Complete
            ));
        }
        assert_eq!(execution.pending, Some(JsValue::Int(7)));
        assert!(execution.selected_native.is_none());
        #[cfg(feature = "profiling")]
        {
            let events = profile.snapshot().owned_execution_events;
            assert_eq!(events.get("direct_callee_payload_selection"), Some(&1));
            assert_eq!(events.get("core.call_decline.native"), Some(&1));
            assert_eq!(
                events.get("native_linked_classification_consumed"),
                Some(&1)
            );
            assert_eq!(events.get("native_callee_owner_transferred"), Some(&1));
        }
        assert!(!runtime.0.deferred_references.has_pending());
        drop(execution);
        assert!(!runtime.is_poisoned());
        assert!(runtime.0.state.borrow().active_frames.is_empty());
    }

    #[cfg(feature = "profiling")]
    #[test]
    fn outer_native_hint_survives_ordinary_argument_calls() {
        use crate::engine::api::{Runtime, Value, profiling::CostProfile};
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        drop(context.eval("Math.min;Math.max").unwrap());
        let profile = CostProfile::start();
        assert_eq!(
            context
                .eval("(()=>{function arg(){return 7};return Math.min(arg(),9)})()")
                .unwrap(),
            Value::Int(7)
        );
        let events = profile.snapshot().owned_execution_events;
        assert_eq!(
            events.get("native_linked_classification_consumed"),
            Some(&1)
        );
        assert_eq!(events.get("core.call_decline.native_hint"), Some(&1));
        drop(profile);
        assert_eq!(
            context
                .eval("(()=>{return Math.min((()=>{let f=Math.max;return f(3,7)})(),9)})()")
                .unwrap(),
            Value::Int(7)
        );
        assert!(runtime.0.state.borrow().active_frames.is_empty());
        assert!(!runtime.is_poisoned());
    }

    #[test]
    fn nonmethod_zero_argument_call_keeps_ordinary_native_and_general_entries() {
        use crate::engine::api::{Runtime, Value};
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        let source = "(()=>{function invoke(f){return f()}let calls=0;function ordinary(){calls++;return 42}let first=invoke(ordinary);let second=invoke(Math.max);let error=false;try{invoke(7)}catch(e){error=e instanceof TypeError}return first===42&&second===-Infinity&&error&&calls===1})()";
        assert_eq!(context.eval(source).unwrap(), Value::Bool(true));
        assert_eq!(runtime.0.active_frame_depth.get(), 0);
        assert!(runtime.0.state.borrow().active_frames.is_empty());
    }

    #[test]
    fn ordinary_operand_proof_preserves_method_receiver_and_proxy_fallback() {
        use crate::engine::api::{Runtime, Value};
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        let source = "(()=>{let calls=0;let holder={base:40,f(x){calls++;return this.base+x}};let first=holder.f(2);let original=holder.f;holder.f=new Proxy(original,{apply(target,receiver,args){calls++;return Reflect.apply(target,receiver,args)}});let second=holder.f(2);return first===42&&second===42&&calls===3})()";
        assert_eq!(context.eval(source).unwrap(), Value::Bool(true));
        assert_eq!(runtime.0.active_frame_depth.get(), 0);
        assert!(runtime.0.state.borrow().active_frames.is_empty());
    }

    #[test]
    fn literal_method_and_native_ready_keep_receivers_errors_and_argument_order() {
        use crate::engine::api::{Runtime, Value};
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        for source in [
            "Math.min(3, 2, 1) === 1 && Math.max() === -Infinity",
            "(()=>{let i=7;let r=Math.min(i,500);return r===7})()",
            "((i)=>{let r=Math.min(i,500);return r===7})(7)",
            "(()=>{let key={},value='text',m=new Map();m.set(key,value);return m.get(key)===value})()",
            "(()=>{let i=3;function capture(){return i}let r=Math.min(i,500);return r===capture()})()",
            "(()=>{try{Math.min(i,500);let i=3}catch(e){return e instanceof ReferenceError}return false})()",
            "(()=>{let i=2;let o={get m(){i=7;return Math.min}};let r=o.m(i,500);return r===7})()",
            "(()=>{let i=2;let o=new Proxy({m:Math.min},{get(t,k){i=9;return t[k]}});let r=o.m(i,500);return r===9})()",
            "(()=>{let n=0,a={valueOf(){n++;return 7}};let r=Math.min(a,500);return r===7&&n===1})()",
            "(()=>{let x=Symbol();try{Math.min(x,500)}catch(e){return e instanceof TypeError}return false})()",
            "(()=>{try{Math.min(Symbol())}catch(e){return e instanceof TypeError}return false})()",
            "Object.defineProperty(Math.min,'length',{value:99}); Math.min(2,3)===2",
            "(()=>{let o={x:9,m(a,b){return this.x+a+b}};return o.m(1,2)===12})()",
            "(()=>{let log='';let o={get m(){log+='g';return function(x){log+='c';return x}}};let x=o.m((log+='a',7));return x===7&&log==='gac'})()",
            "(()=>{let n=0;let o={get m(){n++;throw 8}};try{o.m(1,2)}catch(e){return e===8&&n===1}return false})()",
            "(()=>{let o={m:0};try{o.m(1,2)}catch(e){return e instanceof TypeError}return false})()",
            "(()=>{let m=new Map();m.set(1,2);return m.get(1)===2&&m.has(1)})()",
            "(()=>{let f=Math.min;Math.min=(a,b)=>a+b;let v=Math.min(2,3);Math.min=f;return v===5&&Math.min(2,3)===2})()",
        ] {
            assert_eq!(context.eval(source).unwrap(), Value::Bool(true), "{source}");
        }
        assert_eq!(runtime.0.active_frame_depth.get(), 0);
        assert!(runtime.0.state.borrow().active_frames.is_empty());
    }

    #[cfg(feature = "profiling")]
    #[test]
    fn variable_method_argument_preserves_native_call_result() {
        use crate::engine::api::{Runtime, Value, profiling::CostProfile};
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        // Resolve the lazy builtin before observing an ordinary method call.
        drop(context.eval("Math.min").unwrap());
        let profile = CostProfile::start();
        assert_eq!(
            context
                .eval("(()=>{let i=7;let result=Math.min(i,500);return result})()")
                .unwrap(),
            Value::Int(7)
        );
        let costs = profile.snapshot();
        assert!(costs.callsites.values().any(|site| site.calls > 0));
    }

    #[test]
    fn transferred_native_call_keeps_coercion_reentry_throw_and_actual_arity() {
        use crate::engine::api::{Runtime, Value};
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        for source in [
            "(()=>{let log='',marker={};let a={valueOf(){log+='a';return 3}},b={valueOf(){log+='b';throw marker}},c={valueOf(){log+='c';return 1}};try{Math.min(a,b,c)}catch(e){return e===marker&&log==='ab'}return false})()",
            "(()=>{let map=new Map(),key={},value={};map.set(key,value);let f=Math.min;let n=0;let a={valueOf(){n++;map.set(key,{x:42});return map.get(key).x}};return f(a,50)===42&&n===1&&map.get(key).x===42})()",
            "(()=>{let f=Math.max;return f()===-Infinity&&f(1,2,3,4,5,6)===6&&Number.isFinite(3,{})})()",
            "(()=>{let map=new Map(),key={},value={};let holder={call:map.set};let failed=false;try{holder.call(key,value)}catch(e){failed=e instanceof TypeError}map.set(key,value);return failed&&map.get(key)===value})()",
        ] {
            assert_eq!(context.eval(source).unwrap(), Value::Bool(true), "{source}");
            assert!(runtime.0.state.borrow().active_frames.is_empty());
        }
        runtime.run_gc().unwrap();
        assert_eq!(runtime.0.active_frame_depth.get(), 0);
    }

    #[cfg(feature = "profiling")]
    #[test]
    fn native_direct_entry_records_owner_transfer_and_validation_reuse() {
        use crate::engine::api::{Runtime, Value, profiling::CostProfile};
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        drop(
            context
                .eval("Math.min; Map.prototype.set; Map.prototype.get")
                .unwrap(),
        );
        let profile = CostProfile::start();
        assert_eq!(context.eval("(()=>{let map=new Map(),key={},value={x:42};map.set(key,value);let f=Math.min;return f(map.get(key).x,50)})()").unwrap(), Value::Int(42));
        let costs = profile.snapshot();
        let transferred = costs
            .owned_execution_events
            .get("native_callee_owner_transferred")
            .copied()
            .unwrap_or(0);
        assert!(transferred >= 3, "{:#?}", costs.owned_execution_events);
        assert_eq!(
            costs
                .owned_execution_events
                .get("native_argv_validation_reused"),
            Some(&transferred)
        );
    }

    #[test]
    fn unified_call_entry_keeps_the_ordinary_result_abi_size() {
        // The boxed error channel is one word, so a result transaction is now
        // governed by its own payload instead of an 80-byte error slot. The
        // ordinary completion stays at two words; the unified entry is bounded
        // by its own payload plus the one-word error channel.
        let word = std::mem::size_of::<usize>();
        assert_eq!(std::mem::size_of::<super::Error>(), word);
        assert_eq!(std::mem::size_of::<Result<(), super::Error>>(), word);
        assert!(std::mem::size_of::<Result<bool, super::Error>>() <= 2 * word);
        assert!(std::mem::size_of::<Result<super::Entry, super::Error>>() <= 3 * word);
    }
}

#[cfg(test)]
mod held_state_call_tests {
    use super::*;
    use crate::engine::{
        code::exec_opcode::Opcode,
        heap::ObjectId,
        value::JsValue,
        vm::{
            call::CallableExecution, driver::push_frame, execute::FallthroughPc,
            execution::ExecutionLimits,
        },
    };

    fn caller(
        runtime: &Runtime,
        context: &mut crate::engine::api::Context,
        method: bool,
    ) -> (RunningExecution, FrameId, FallthroughPc) {
        let source = if method {
            "(function invoke(object,arg){return object.method(arg)})"
        } else {
            "(function invoke(fn,arg){return fn(arg)})"
        };
        let callable = runtime
            .callable_from_value(context.eval(source).unwrap())
            .unwrap();
        let CallableExecution::Bytecode {
            bytecode,
            closure_slots,
            ..
        } = runtime.bytecode_for_callable(&callable).unwrap()
        else {
            unreachable!()
        };
        let entry = crate::engine::vm::root_call::prepare_call(
            runtime,
            context.realm,
            &callable,
            JsValue::Undefined,
            JsValue::Undefined,
            Vec::new(),
            bytecode,
            closure_slots,
        )
        .unwrap();
        let mut pc = 0;
        let fallthrough = loop {
            let decoded = entry.executable.exec.decode_published(pc).unwrap();
            if matches!(
                decoded.opcode,
                Opcode::Call | Opcode::CallMethod | Opcode::TailCall | Opcode::TailCallMethod
            ) {
                break FallthroughPc::from_decoded(decoded);
            }
            pc = decoded.next_pc;
        };
        let mut execution = RunningExecution::new(
            runtime,
            ExecutionLimits {
                frames: 8,
                slots: 256,
            },
        )
        .unwrap();
        let id = push_frame(runtime, &mut execution, entry).unwrap();
        (execution, id, fallthrough)
    }

    fn push(runtime: &Runtime, execution: &mut RunningExecution, id: FrameId, value: JsValue) {
        let frame = execution.frames.current_mut(id).unwrap();
        execution.slots.push(&mut frame.window, value).unwrap();
        assert!(!runtime.is_poisoned());
    }

    fn object(runtime: &Runtime) -> ObjectId {
        runtime.new_object(None).unwrap().into_handle()
    }

    fn next_call(execution: &mut RunningExecution, id: FrameId) -> FallthroughPc {
        let frame = execution.frames.current_mut(id).unwrap();
        let mut pc = 0;
        loop {
            let decoded = frame.executable.exec.decode_published(pc).unwrap();
            if matches!(
                decoded.opcode,
                Opcode::Call | Opcode::CallMethod | Opcode::TailCall | Opcode::TailCallMethod
            ) {
                return FallthroughPc::from_decoded(decoded);
            }
            pc = decoded.next_pc;
        }
    }

    #[test]
    fn method_call_and_return_hold_one_state_access_and_move_owners() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        let callee = context.eval("(function child(arg){return arg})").unwrap();
        let (mut execution, parent, next) = caller(&runtime, &mut context, true);
        let callee = runtime.into_jsvalue(callee).unwrap();
        let JsValue::Object(callee_id) = callee else {
            unreachable!()
        };
        let receiver = object(&runtime);
        let argument = object(&runtime);
        push(&runtime, &mut execution, parent, JsValue::Object(receiver));
        push(&runtime, &mut execution, parent, JsValue::Object(callee_id));
        push(&runtime, &mut execution, parent, JsValue::Object(argument));
        let runtime_owners = std::rc::Rc::strong_count(&runtime.0);
        let mut state = runtime.0.state.borrow_mut();
        assert!(matches!(
            enter_selected_in_state(
                &runtime,
                &mut state,
                &mut execution,
                parent,
                1,
                true,
                false,
                None,
                next,
            )
            .unwrap(),
            Entry::Ordinary
        ));
        let child = execution.frames.current_id().unwrap();
        assert_ne!(child, parent);
        assert!(
            execution
                .frames
                .current_mut(child)
                .unwrap()
                .executable
                .root()
                .is_none()
        );
        assert_eq!(state.heap.object_strong_count(receiver), Ok(1));
        assert_eq!(state.heap.object_strong_count(argument), Ok(1));
        execution.pending = Some(state.dup_jsvalue(&JsValue::Object(argument)).unwrap());
        assert!(matches!(
            finish_in_state(&runtime, &mut state, &mut execution, child).unwrap(),
            ReturnProgress::Returned
        ));
        assert_eq!(execution.frames.current_id(), Some(parent));
        assert_eq!(state.heap.object_strong_count(argument), Ok(1));
        assert!(state.heap.object(receiver).is_err());
        assert!(state.heap.object(callee_id).is_err());
        assert_eq!(std::rc::Rc::strong_count(&runtime.0), runtime_owners);
        assert!(!runtime.0.deferred_references.has_pending());
        assert!(execution.pending.is_none());
        let frame = execution.frames.current_mut(parent).unwrap();
        assert!(
            matches!(execution.slots.peek(&frame.window, 0).unwrap(), JsValue::Object(id) if *id == argument)
        );
        drop(state);
        drop(execution);
        assert!(runtime.0.state.borrow().heap.object(argument).is_err());
    }

    #[test]
    fn native_and_proxy_decline_before_owner_or_resume_changes() {
        for source in ["Math.max", "new Proxy(function(arg){return arg},{})"] {
            let runtime = Runtime::new();
            let mut context = runtime.new_context().unwrap();
            let callee = context.eval(source).unwrap();
            let (mut execution, parent, next) = caller(&runtime, &mut context, true);
            let callee = runtime.into_jsvalue(callee).unwrap();
            let JsValue::Object(callee_id) = callee else {
                unreachable!()
            };
            let receiver = object(&runtime);
            let argument = object(&runtime);
            push(&runtime, &mut execution, parent, JsValue::Object(receiver));
            push(&runtime, &mut execution, parent, JsValue::Object(callee_id));
            push(&runtime, &mut execution, parent, JsValue::Object(argument));
            let resume = execution.frames.current_mut(parent).unwrap().resume_pc;
            let mut state = runtime.0.state.borrow_mut();
            let counts = [receiver, callee_id, argument]
                .map(|id| state.heap.object_strong_count(id).unwrap());
            assert!(matches!(
                enter_selected_in_state(
                    &runtime,
                    &mut state,
                    &mut execution,
                    parent,
                    1,
                    true,
                    false,
                    None,
                    next,
                )
                .unwrap(),
                Entry::General
            ));
            assert_eq!(execution.frames.current_id(), Some(parent));
            let frame = execution.frames.current_mut(parent).unwrap();
            assert_eq!(frame.resume_pc, resume);
            assert_eq!(execution.slots.depth(&frame.window), 3);
            assert_eq!(
                [receiver, callee_id, argument]
                    .map(|id| state.heap.object_strong_count(id).unwrap()),
                counts
            );
            assert!(!runtime.0.deferred_references.has_pending());
        }
    }

    #[test]
    fn failed_direct_return_keeps_result_registered_for_abandonment() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        let callee = context.eval("(function child(arg){return arg})").unwrap();
        let (mut execution, parent, next) = caller(&runtime, &mut context, false);
        let callee = runtime.into_jsvalue(callee).unwrap();
        let argument = object(&runtime);
        push(&runtime, &mut execution, parent, callee);
        push(&runtime, &mut execution, parent, JsValue::Object(argument));
        {
            let mut state = runtime.0.state.borrow_mut();
            assert!(matches!(
                enter_selected_in_state(
                    &runtime,
                    &mut state,
                    &mut execution,
                    parent,
                    1,
                    false,
                    false,
                    None,
                    next,
                )
                .unwrap(),
                Entry::Ordinary
            ));
            let child = execution.frames.current_id().unwrap();
            execution.pending = Some(state.dup_jsvalue(&JsValue::Object(argument)).unwrap());
            // A corrupt return target is rejected after retirement while its
            // heap result remains in registered execution scratch.
            execution
                .frames
                .current_mut(child)
                .unwrap()
                .cold
                .return_to
                .as_mut()
                .unwrap()
                .owner = crate::engine::vm::frame::ReturnOwner::Frame(child);
            assert!(finish_in_state(&runtime, &mut state, &mut execution, child).is_err());
            assert_eq!(execution.frames.current_id(), Some(parent));
            assert!(execution.pending.is_some());
        }
        drop(execution);
        assert!(runtime.0.state.borrow().heap.object(argument).is_err());
        assert!(!runtime.is_poisoned());
    }

    #[test]
    fn base_constructor_selects_primitive_object_and_alias_returns_under_state() {
        for result_kind in 0..3 {
            let runtime = Runtime::new();
            let mut context = runtime.new_context().unwrap();
            let callee = context.eval("(function Base(arg){return arg})").unwrap();
            let (mut execution, parent, next) = caller(&runtime, &mut context, true);
            let callee = runtime.into_jsvalue(callee).unwrap();
            let receiver = object(&runtime);
            let other = object(&runtime);
            push(&runtime, &mut execution, parent, JsValue::Object(receiver));
            push(&runtime, &mut execution, parent, callee);
            push(&runtime, &mut execution, parent, JsValue::Undefined);
            let selected = {
                let mut state = runtime.0.state.borrow_mut();
                assert!(matches!(
                    enter_selected_in_state(
                        &runtime,
                        &mut state,
                        &mut execution,
                        parent,
                        1,
                        true,
                        false,
                        None,
                        next,
                    )
                    .unwrap(),
                    Entry::Ordinary
                ));
                let child = execution.frames.current_id().unwrap();
                // Direct constructor admission gives input and return selection
                // independent owners of the same receiver.
                state.heap.retain_object(receiver).unwrap();
                execution
                    .frames
                    .current_mut(child)
                    .unwrap()
                    .cold
                    .constructor_return = Some(crate::engine::vm::frame::ConstructorReturn::Base(
                    JsValue::Object(receiver),
                ));
                let selected = if result_kind == 0 {
                    execution.pending = Some(JsValue::Int(7));
                    state.release_jsvalue(JsValue::Object(other)).unwrap();
                    receiver
                } else if result_kind == 1 {
                    execution.pending = Some(JsValue::Object(other));
                    other
                } else {
                    execution.pending =
                        Some(state.dup_jsvalue(&JsValue::Object(receiver)).unwrap());
                    state.release_jsvalue(JsValue::Object(other)).unwrap();
                    receiver
                };
                assert!(matches!(
                    finish_in_state(&runtime, &mut state, &mut execution, child).unwrap(),
                    ReturnProgress::Returned
                ));
                assert_eq!(state.heap.object_strong_count(selected), Ok(1));
                if selected != receiver {
                    assert!(state.heap.object(receiver).is_err());
                }
                assert!(!runtime.0.deferred_references.has_pending());
                selected
            };
            drop(execution);
            assert!(runtime.0.state.borrow().heap.object(selected).is_err());
            assert!(!runtime.is_poisoned());
        }
    }

    #[test]
    fn ordinary_tail_chain_retires_under_one_state_access() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        let callees = [
            "(function outer(fn,arg){return fn(arg)})",
            "(function middle(fn,arg){return fn(arg)})",
            "(function leaf(arg){return arg})",
        ]
        .map(|source| runtime.into_jsvalue(context.eval(source).unwrap()).unwrap());
        let callee_ids = callees.each_ref().map(|value| match value {
            JsValue::Object(id) => *id,
            _ => unreachable!(),
        });
        let (mut execution, root, next) = caller(&runtime, &mut context, false);
        let result = object(&runtime);
        let runtime_owners = std::rc::Rc::strong_count(&runtime.0);
        let mut state = runtime.0.state.borrow_mut();
        let mut parent = root;
        let mut next = next;
        for (index, callee) in callees.into_iter().enumerate() {
            push(&runtime, &mut execution, parent, callee);
            push(
                &runtime,
                &mut execution,
                parent,
                if index == 0 {
                    JsValue::Object(result)
                } else {
                    JsValue::Undefined
                },
            );
            assert!(matches!(
                enter_selected_in_state(
                    &runtime,
                    &mut state,
                    &mut execution,
                    parent,
                    1,
                    false,
                    index != 0,
                    None,
                    next,
                )
                .unwrap(),
                Entry::Ordinary
            ));
            parent = execution.frames.current_id().unwrap();
            assert!(
                execution
                    .frames
                    .current_mut(parent)
                    .unwrap()
                    .cold
                    .rare
                    .get()
                    .is_none()
            );
            if index != 2 {
                next = next_call(&mut execution, parent);
            }
        }
        execution.pending = Some(state.dup_jsvalue(&JsValue::Object(result)).unwrap());
        assert!(matches!(
            finish_in_state(&runtime, &mut state, &mut execution, parent,).unwrap(),
            ReturnProgress::Returned
        ));
        assert_eq!(execution.frames.current_id(), Some(root));
        assert_eq!(execution.frames.depth(), 1);
        assert!(execution.pending.is_none());
        assert_eq!(state.heap.object_strong_count(result), Ok(1));
        assert!(
            callee_ids
                .into_iter()
                .all(|id| state.heap.object(id).is_err())
        );
        assert_eq!(std::rc::Rc::strong_count(&runtime.0), runtime_owners);
        assert!(!runtime.0.deferred_references.has_pending());
        drop(state);
        drop(execution);
        assert!(runtime.0.state.borrow().heap.object(result).is_err());
        assert!(!runtime.is_poisoned());
    }

    #[test]
    fn tail_result_stays_registered_at_root_and_derived_boundaries() {
        for derived in [false, true] {
            let runtime = Runtime::new();
            let mut context = runtime.new_context().unwrap();
            let outer = if derived {
                Some(
                    runtime
                        .into_jsvalue(
                            context
                                .eval("(function outer(fn,arg){return fn(arg)})")
                                .unwrap(),
                        )
                        .unwrap(),
                )
            } else {
                None
            };
            let leaf = runtime
                .into_jsvalue(context.eval("(function leaf(arg){return arg})").unwrap())
                .unwrap();
            let (mut execution, root, next) = caller(&runtime, &mut context, false);
            let result = object(&runtime);
            let mut state = runtime.0.state.borrow_mut();
            let (boundary, next) = if let Some(outer) = outer {
                push(&runtime, &mut execution, root, outer);
                push(&runtime, &mut execution, root, JsValue::Undefined);
                assert!(matches!(
                    enter_selected_in_state(
                        &runtime,
                        &mut state,
                        &mut execution,
                        root,
                        1,
                        false,
                        false,
                        None,
                        next,
                    )
                    .unwrap(),
                    Entry::Ordinary
                ));
                let outer = execution.frames.current_id().unwrap();
                execution
                    .frames
                    .current_mut(outer)
                    .unwrap()
                    .cold
                    .constructor_return =
                    Some(crate::engine::vm::frame::ConstructorReturn::Derived);
                (outer, next_call(&mut execution, outer))
            } else {
                (root, next)
            };
            push(&runtime, &mut execution, boundary, leaf);
            push(&runtime, &mut execution, boundary, JsValue::Object(result));
            assert!(matches!(
                enter_selected_in_state(
                    &runtime,
                    &mut state,
                    &mut execution,
                    boundary,
                    1,
                    false,
                    true,
                    None,
                    next,
                )
                .unwrap(),
                Entry::Ordinary
            ));
            let leaf = execution.frames.current_id().unwrap();
            execution.pending = Some(state.dup_jsvalue(&JsValue::Object(result)).unwrap());
            assert!(matches!(
                finish_in_state(&runtime, &mut state, &mut execution, leaf,).unwrap(),
                ReturnProgress::Declined
            ));
            assert_eq!(execution.frames.current_id(), Some(boundary));
            assert!(matches!(execution.pending, Some(JsValue::Object(id)) if id == result));
            assert_eq!(state.heap.object_strong_count(result), Ok(1));
            assert!(!runtime.0.deferred_references.has_pending());
            drop(state);
            drop(execution);
            assert!(runtime.0.state.borrow().heap.object(result).is_err());
            assert!(!runtime.is_poisoned());
        }
    }

    #[test]
    fn base_constructor_selects_receiver_after_tail_result_propagation() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        let outer = runtime
            .into_jsvalue(
                context
                    .eval("(function Base(fn,arg){return fn(arg)})")
                    .unwrap(),
            )
            .unwrap();
        let leaf = runtime
            .into_jsvalue(context.eval("(function leaf(arg){return arg})").unwrap())
            .unwrap();
        let (mut execution, root, next) = caller(&runtime, &mut context, false);
        let receiver = object(&runtime);
        let mut state = runtime.0.state.borrow_mut();
        push(&runtime, &mut execution, root, outer);
        push(&runtime, &mut execution, root, JsValue::Undefined);
        assert!(matches!(
            enter_selected_in_state(
                &runtime,
                &mut state,
                &mut execution,
                root,
                1,
                false,
                false,
                None,
                next,
            )
            .unwrap(),
            Entry::Ordinary
        ));
        let outer = execution.frames.current_id().unwrap();
        execution
            .frames
            .current_mut(outer)
            .unwrap()
            .cold
            .constructor_return = Some(crate::engine::vm::frame::ConstructorReturn::Base(
            JsValue::Object(receiver),
        ));
        let next = next_call(&mut execution, outer);
        push(&runtime, &mut execution, outer, leaf);
        push(&runtime, &mut execution, outer, JsValue::Undefined);
        assert!(matches!(
            enter_selected_in_state(
                &runtime,
                &mut state,
                &mut execution,
                outer,
                1,
                false,
                true,
                None,
                next,
            )
            .unwrap(),
            Entry::Ordinary
        ));
        let leaf = execution.frames.current_id().unwrap();
        execution.pending = Some(JsValue::Int(7));
        assert!(matches!(
            finish_in_state(&runtime, &mut state, &mut execution, leaf,).unwrap(),
            ReturnProgress::Returned
        ));
        assert_eq!(execution.frames.current_id(), Some(root));
        assert_eq!(state.heap.object_strong_count(receiver), Ok(1));
        assert!(execution.pending.is_none());
        assert!(!runtime.0.deferred_references.has_pending());
        drop(state);
        drop(execution);
        assert!(runtime.0.state.borrow().heap.object(receiver).is_err());
        assert!(!runtime.is_poisoned());
    }
}
