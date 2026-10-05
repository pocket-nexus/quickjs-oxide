//! Local native owners use the execution's State access. No Runtime owner or
//! durable progress exists while a consumer can finish without JavaScript.
use super::*;
use crate::engine::{
    api::{
        error::{NativeErrorKind, NativeErrorMessage},
        runtime_error::RuntimeError,
    },
    builtins::native::{ArrayPopKind, ArrayPushKind, NativeFunctionId},
    heap::{ContextId, ObjectId, runtime::RuntimeState},
    object::CallableRef,
    vm::{
        call::NativeInvocation,
        frames::{ActiveFrameRestore, publish_native_in_state},
    },
};
use std::cell::Cell;

struct NativeStateGuard<'a> {
    state: &'a mut RuntimeState,
    poisoned: &'a Cell<bool>,
    function: Option<ObjectId>,
    invocation: Option<NativeInvocation>,
    arguments: Vec<JsValue>,
    frame: Option<ActiveFrameRestore>,
    result: Option<Completion>,
}

impl NativeStateGuard<'_> {
    fn retire(&mut self) -> Result<(), RuntimeError> {
        if std::thread::panicking() {
            self.poisoned.set(true);
        }
        if self.poisoned.get() {
            return Err(RuntimeError::Poisoned);
        }
        if let Some(frame) = self.frame.take() {
            frame
                .finish(self.state)
                .inspect_err(|_| self.poisoned.set(true))?;
        }
        if let Some(function) = self.function.take() {
            self.state
                .release_owned_jsvalue(self.poisoned, JsValue::Object(function))?;
        }
        for argument in self.arguments.drain(..) {
            self.state.release_owned_jsvalue(self.poisoned, argument)?;
        }
        if let Some(invocation) = self.invocation.take() {
            let value = match invocation {
                NativeInvocation::Call { this_value }
                | NativeInvocation::Getter { this_value }
                | NativeInvocation::Setter { this_value } => this_value,
                NativeInvocation::Construct { new_target } => new_target,
            };
            self.state.release_owned_jsvalue(self.poisoned, value)?;
        }
        Ok(())
    }
}
impl Drop for NativeStateGuard<'_> {
    fn drop(&mut self) {
        if self.retire().is_ok()
            && let Some(result) = self.result.take()
        {
            let (Completion::Return(value) | Completion::Throw(value)) = result;
            let _ = self.state.release_owned_jsvalue(self.poisoned, value);
        }
    }
}

/// Existing ABI inputs stay with the caller on decline. Only the cold,
/// unmigrated effect boundary reconstructs a public callable root. The direct
/// consumer transfers its callee into this guard and retires it using State.
#[allow(clippy::too_many_arguments)]
pub(super) fn try_array(
    runtime: &Runtime,
    slots: &mut super::super::stack::SlotStore,
    callable: &mut Option<CallableRef>,
    realm: ContextId,
    target: NativeFunctionId,
    minimum: u8,
    invocation: &mut Option<NativeInvocation>,
    arguments: &mut Vec<JsValue>,
) -> Result<Option<Completion>, Error> {
    let function = callable
        .as_ref()
        .expect("native callable owner")
        .as_object()
        .object_id();
    let mut state = runtime.0.state.borrow_mut();
    let mut guard = NativeStateGuard {
        state: &mut state,
        poisoned: &runtime.0.poisoned,
        function: None,
        invocation: invocation.take(),
        arguments: std::mem::take(arguments),
        frame: None,
        result: None,
    };
    if !callable
        .as_ref()
        .expect("native callable owner")
        .belongs_to(runtime)
    {
        return Err(runtime_error_to_vm_error(RuntimeError::WrongRuntime(
            "native callable",
        )));
    }
    // This is the existing empty-capacity pool, reserved before publication.
    // Skipping it would turn every single-argument push into a fresh Vec.
    slots.reserve_native_argument_depth(guard.state.active_frames.len() + 1)?;
    guard.frame = Some(
        publish_native_in_state(
            guard.state,
            function,
            realm,
            target,
            minimum,
            guard.arguments.len(),
        )
        .map_err(runtime_error_to_vm_error)?,
    );
    let outcome = match (
        target,
        guard.invocation.as_ref().expect("native invocation owner"),
    ) {
        (NativeFunctionId::ArrayConstructor, NativeInvocation::Construct { new_target }) => guard
            .state
            .try_complete_array_constructor(guard.poisoned, realm, new_target, &guard.arguments),
        (NativeFunctionId::ArrayConstructor, NativeInvocation::Call { .. }) => {
            guard.state.try_complete_array_constructor(
                guard.poisoned,
                realm,
                &JsValue::Undefined,
                &guard.arguments,
            )
        }
        (
            NativeFunctionId::ArrayPrototypePush(ArrayPushKind::Push),
            NativeInvocation::Call {
                this_value: JsValue::Object(object),
            },
        ) if guard.arguments.len() == 1 => guard
            .state
            .try_dense_push(*object, &guard.arguments[0])
            .map(|value| value.map(Completion::Return)),
        (
            NativeFunctionId::ArrayPrototypePop(ArrayPopKind::Pop),
            NativeInvocation::Call {
                this_value: JsValue::Object(object),
            },
        ) => guard
            .state
            .try_dense_pop(*object)
            .map(|value| value.map(Completion::Return)),
        _ => Ok(None),
    };
    match outcome {
        Ok(None) => {
            guard
                .frame
                .take()
                .expect("native frame")
                .finish(guard.state)
                .inspect_err(|_| guard.poisoned.set(true))
                .map_err(runtime_error_to_vm_error)?;
            *invocation = guard.invocation.take();
            *arguments = std::mem::take(&mut guard.arguments);
            Ok(None)
        }
        result => {
            guard.function = Some(
                callable
                    .take()
                    .expect("native callable owner")
                    .into_object()
                    .into_execution_handle(),
            );
            guard.result = Some(match result {
                Ok(Some(result)) => result,
                Err(RuntimeError::Engine(error))
                    if NativeErrorKind::from_javascript_error(error.kind()).is_some() =>
                {
                    let kind = NativeErrorKind::from_javascript_error(error.kind())
                        .expect("native Error kind");
                    let message = error
                        .native_message()
                        .cloned()
                        .unwrap_or_else(|| NativeErrorMessage::from_utf8(error.message()));
                    let value = guard
                        .state
                        .new_native_error_from_message(guard.poisoned, realm, kind, message)
                        .map_err(runtime_error_to_vm_error)?;
                    Completion::Throw(JsValue::Object(value))
                }
                Err(error) => return Err(runtime_error_to_vm_error(error)),
                Ok(None) => unreachable!(),
            });
            guard.retire().map_err(runtime_error_to_vm_error)?;
            slots.recycle_native_argument_buffer(std::mem::take(&mut guard.arguments));
            #[cfg(feature = "profiling")]
            crate::engine::api::profiling::record_owned_execution_event(
                "native_completed_through_state_guard",
            );
            Ok(guard.result.take())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "profiling")]
    use crate::engine::api::profiling::CostProfile;
    use crate::engine::heap::RawId;

    #[cfg(feature = "profiling")]
    #[test]
    fn synchronous_array_consumers_do_not_publish_waiting_transport() {
        let runtime = Runtime::new();
        let profile = CostProfile::start();
        let mut context = runtime.new_context().unwrap();
        assert_eq!(
            context
                .eval(
                    r#"(function(){
            var a=new Array(), b=Array(4), o={};
            a.push(o); a.push('s'); a.push(123456789012345678901234567890n);
            a.push(Symbol.for('move')); var s=a.pop(), n=a.pop(), t=a.pop(), p=a.pop();
            return b.length===4 && a.length===0 && s===Symbol.for('move') &&
                n===123456789012345678901234567890n && t==='s' && p===o;
        })()"#
                )
                .unwrap(),
            Value::Bool(true)
        );
        let costs = profile.snapshot();
        assert_eq!(
            costs
                .owned_execution_events
                .get("query.array_native.completed_without_callback")
                .copied()
                .unwrap_or(0),
            0
        );
        assert_eq!(
            costs
                .owned_execution_events
                .get("progress.array_mutation.created")
                .copied()
                .unwrap_or(0),
            0
        );
        assert!(
            costs
                .owned_execution_events
                .get("native_completed_through_state_guard")
                .copied()
                .unwrap_or(0)
                >= 10
        );
        assert!(runtime.0.state.borrow().active_frames.is_empty());
    }

    #[test]
    fn array_prefix_preserves_prototype_and_indexed_set_effect_order() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        assert_eq!(context.eval(r#"(function(){
            var log=[], marker={}, proto=Object.create(Array.prototype);
            Object.defineProperty(proto,'0',{set(v){log.push(v);throw marker},configurable:true});
            function C(){}; Object.defineProperty(C,'prototype',{value:proto});
            try {Reflect.construct(Array,[7,8],C);return false}
            catch(e){if(e!==marker || log.join()!=='7')return false}
            var N=new Proxy(function(){},{get(t,k){if(k==='prototype'){log.push('p');throw marker}return t[k]}});
            try {Reflect.construct(Array,[-1],N);return false}
            catch(e){if(e!==marker || log.join()!=='7,p')return false}
            var zero=new Array(-0);
            try {new Array(-1);return false}
            catch(e){return e instanceof RangeError && Object.is(zero.length,0)}
        })()"#).unwrap(), Value::Bool(true));
        assert!(runtime.0.state.borrow().active_frames.is_empty());
    }

    #[test]
    fn failed_checked_prototype_retain_retires_native_frame_and_inputs() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        let prototype = runtime
            .0
            .state
            .borrow()
            .heap
            .context(context.realm)
            .unwrap()
            .array_prototype;
        let previous = runtime
            .0
            .state
            .borrow()
            .heap
            .object_strong_count(prototype)
            .unwrap();
        runtime
            .0
            .state
            .borrow_mut()
            .heap
            .set_strong_count_for_test(RawId::Object(prototype), u32::MAX);
        assert!(context.eval("new Array()").is_err());
        assert!(!runtime.is_poisoned());
        {
            let mut state = runtime.0.state.borrow_mut();
            assert!(state.active_frames.is_empty());
            state
                .heap
                .set_strong_count_for_test(RawId::Object(prototype), previous);
        }
        assert_eq!(context.eval("new Array().length").unwrap(), Value::Int(0));
    }

    #[test]
    fn panic_quarantines_guard_before_any_owned_cleanup() {
        let runtime = Runtime::new();
        let function = runtime.new_object(None).unwrap().into_handle();
        let input = runtime.new_object(None).unwrap().into_handle();
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut state = runtime.0.state.borrow_mut();
            let _guard = NativeStateGuard {
                state: &mut state,
                poisoned: &runtime.0.poisoned,
                function: Some(function),
                invocation: Some(NativeInvocation::Call {
                    this_value: JsValue::Object(input),
                }),
                arguments: vec![],
                frame: None,
                result: None,
            };
            panic!("injected native body panic");
        }));
        assert!(caught.is_err());
        assert!(runtime.is_poisoned());
        let state = runtime.0.state.borrow();
        assert!(state.heap.object(function).is_ok());
        assert!(state.heap.object(input).is_ok());
        drop(state);
        drop(runtime);
    }

    #[test]
    fn destructive_retirement_failure_leaves_owned_suffix_quarantined() {
        let runtime = Runtime::new();
        let function = runtime.new_object(None).unwrap().into_handle();
        let input = runtime.new_object(None).unwrap().into_handle();
        let mut state = runtime.0.state.borrow_mut();
        state
            .heap
            .set_strong_count_for_test(RawId::Object(function), 0);
        {
            let mut guard = NativeStateGuard {
                state: &mut state,
                poisoned: &runtime.0.poisoned,
                function: Some(function),
                invocation: Some(NativeInvocation::Call {
                    this_value: JsValue::Object(input),
                }),
                arguments: vec![],
                frame: None,
                result: None,
            };
            assert!(guard.retire().is_err());
        }
        assert!(runtime.is_poisoned());
        assert_eq!(state.heap.object_strong_count(input).unwrap(), 1);
        drop(state);
        drop(runtime);
    }

    #[test]
    fn foreign_native_rejection_retires_only_local_inputs() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        let foreign = Runtime::new();
        let mut other = foreign.new_context().unwrap();
        let mut callable = Some(
            foreign
                .callable_from_value(other.eval("Array").unwrap())
                .unwrap(),
        );
        let receiver = runtime.new_object(None).unwrap().into_handle();
        let argument = runtime.new_object(None).unwrap().into_handle();
        let mut invocation = Some(NativeInvocation::Call {
            this_value: JsValue::Object(receiver),
        });
        let mut arguments = vec![JsValue::Object(argument)];
        let mut slots = super::super::super::stack::SlotStore::new(1024);
        assert!(
            try_array(
                &runtime,
                &mut slots,
                &mut callable,
                context.realm,
                NativeFunctionId::ArrayConstructor,
                1,
                &mut invocation,
                &mut arguments
            )
            .is_err()
        );
        assert!(callable.is_some());
        assert!(invocation.is_none() && arguments.is_empty());
        let state = runtime.0.state.borrow();
        assert!(state.heap.object(receiver).is_err());
        assert!(state.heap.object(argument).is_err());
        assert!(state.active_frames.is_empty());
        drop(state);
        assert_eq!(context.eval("new Array().length").unwrap(), Value::Int(0));
    }
}
