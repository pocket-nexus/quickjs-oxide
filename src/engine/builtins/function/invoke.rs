//! Complete forwarding family. Semantic owners are raw, admitted edges;
//! actual Get/Call/Construct effects are consumed by the shared Query engine.
use crate::engine::{
    api::{
        error::{Error, ErrorKind, NativeErrorKind, NativeErrorMessage},
        runtime::Runtime,
        runtime_error::RuntimeError,
    },
    builtins::native::{NativeFunctionId, ReflectKind},
    heap::{
        ContextId, ObjectId, ObjectKind,
        runtime::{RuntimeState, owned_values::OwnedValueGuard},
    },
    value::{JsValue, conversion::NativeConversion},
    vm::{
        Completion,
        call::{NativeArguments, NativeInvocation},
    },
};
use std::cell::Cell;

#[derive(Clone, Copy)]
pub(crate) enum InvokeKind {
    Call,
    Apply,
    ReflectApply,
    ReflectConstruct,
}
impl InvokeKind {
    pub(crate) fn for_target(target: NativeFunctionId) -> Option<Self> {
        Some(match target {
            NativeFunctionId::FunctionPrototypeCall => Self::Call,
            NativeFunctionId::FunctionPrototypeApply => Self::Apply,
            NativeFunctionId::Reflect(ReflectKind::Apply) => Self::ReflectApply,
            NativeFunctionId::Reflect(ReflectKind::Construct) => Self::ReflectConstruct,
            _ => return None,
        })
    }
}
pub(crate) enum InvokeStep {
    Construct(Box<InvokeConstruct>),
    Complete(Completion),
    CyclePublished(Completion),
    Arguments { resume: InvokeResume },
    Call(Box<InvokeCall>),
}
pub(crate) enum InvokeCallTarget {
    Callable(ObjectId),
    NonCallableProxy(ObjectId),
}
impl InvokeCallTarget {
    pub(crate) fn id(&self) -> ObjectId {
        match self {
            Self::Callable(id) | Self::NonCallableProxy(id) => *id,
        }
    }
}
pub(crate) enum InvokeNewTarget {
    Validated(ObjectId),
    Raw(JsValue),
}
pub(crate) struct InvokeCall {
    pub(crate) target: InvokeCallTarget,
    pub(crate) receiver: JsValue,
    pub(crate) arguments: Vec<JsValue>,
}
pub(crate) struct InvokeConstruct {
    pub(crate) target: ObjectId,
    pub(crate) new_target: InvokeNewTarget,
    pub(crate) arguments: Vec<JsValue>,
}
pub(crate) struct InvokeResume(Box<InvokeResumeState>);
struct InvokeResumeState {
    arguments: Vec<JsValue>,
    arguments_value: Option<JsValue>,
    realm: ContextId,
    target: ForwardTarget,
    callability_temporary: Option<ObjectId>,
}
enum ForwardTarget {
    Empty,
    Call {
        target: Option<InvokeCallTarget>,
        receiver: Option<JsValue>,
    },
    Construct {
        target: Option<JsValue>,
        new_target: Option<InvokeNewTarget>,
    },
}
enum Effect {
    Arguments,
    Call,
    Construct,
    Complete(Completion, bool),
}
const _: () = assert!(std::mem::size_of::<InvokeResume>() <= 8);

impl InvokeStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        kind: InvokeKind,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Self, RuntimeError> {
        let _operation = runtime.operation()?;
        let result = Self::start_in_state(
            &mut runtime.0.state.borrow_mut(),
            &runtime.0.poisoned,
            realm,
            kind,
            invocation,
            arguments,
        );
        runtime.check_poison()?;
        result
    }
    pub(crate) fn start_in_state(
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
        realm: ContextId,
        kind: InvokeKind,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Self, RuntimeError> {
        let NativeInvocation::Call { this_value } = invocation else {
            return Err(RuntimeError::Invariant(
                "call forwarding requires a generic invocation",
            ));
        };
        let mut owner = InvokeResume::empty(realm);
        let result = owner.start_body(state, poisoned, kind, this_value, arguments);
        owner.finish_effect(state, poisoned, result)
    }
    pub(crate) fn start_spread_in_state(
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
        realm: ContextId,
        kind: crate::engine::code::bytecode::ApplyKind,
        target: &JsValue,
        receiver: &JsValue,
        value: &JsValue,
    ) -> Result<Self, RuntimeError> {
        let mut owner = InvokeResume::empty(realm);
        let result = (|| {
            // OP_apply checks callable before argsList even in construct mode.
            let callable = checked_callable(state, poisoned, target, false)?;
            owner.0.target = ForwardTarget::Call {
                target: Some(InvokeCallTarget::Callable(callable)),
                receiver: None,
            };
            if matches!(value, JsValue::Null | JsValue::Undefined) {
                owner.set_receiver(state.dup_jsvalue(receiver)?);
                return Ok(Effect::Call);
            }
            match kind {
                crate::engine::code::bytecode::ApplyKind::Call => {
                    owner.set_receiver(state.dup_jsvalue(receiver)?)
                }
                crate::engine::code::bytecode::ApplyKind::Construct => {
                    // Keep the actual callability temporary until both target
                    // and raw newTarget have been checked and armed.
                    let raw_target = state.dup_jsvalue(target)?;
                    let mut raw_target = OwnedValueGuard::new(state, poisoned, raw_target);
                    let (state, raw_target) = raw_target.parts();
                    let new_target = state.dup_jsvalue(receiver)?;
                    let old = std::mem::replace(
                        &mut owner.0.target,
                        ForwardTarget::Construct {
                            target: raw_target.take(),
                            new_target: Some(InvokeNewTarget::Raw(new_target)),
                        },
                    );
                    let ForwardTarget::Call {
                        target: Some(InvokeCallTarget::Callable(id)),
                        receiver: None,
                    } = old
                    else {
                        unreachable!()
                    };
                    owner.0.callability_temporary = Some(id);
                }
            }
            owner.0.arguments_value = Some(state.dup_jsvalue(value)?);
            if let Some(callable) = owner.0.callability_temporary.take() {
                state.release_owned_jsvalue(poisoned, JsValue::Object(callable))?;
            }
            Ok(Effect::Arguments)
        })();
        owner.finish_effect(state, poisoned, result)
    }
    pub(crate) fn retire_in_state(
        self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
    ) -> Result<(), RuntimeError> {
        self.retire_with(&mut |value| state.release_owned_jsvalue(poisoned, value))
    }
    fn retire_with(
        self,
        release: &mut impl FnMut(JsValue) -> Result<(), RuntimeError>,
    ) -> Result<(), RuntimeError> {
        match self {
            Self::Complete(Completion::Return(value) | Completion::Throw(value))
            | Self::CyclePublished(Completion::Return(value) | Completion::Throw(value)) => {
                release(value)
            }
            Self::Arguments { resume } => resume.retire_with(release),
            Self::Call(request) => {
                release(request.receiver)?;
                for value in request.arguments {
                    release(value)?;
                }
                release(JsValue::Object(request.target.id()))
            }
            Self::Construct(request) => {
                for value in request.arguments {
                    release(value)?;
                }
                release(JsValue::Object(request.target))?;
                request.new_target.retire_with(release)
            }
        }
    }
    pub(crate) fn retire_at_boundary(self, runtime: &Runtime) -> Result<(), RuntimeError> {
        let _unwind = runtime.unwind_guard();
        if runtime.skip_cleanup() {
            return Err(RuntimeError::Poisoned);
        }
        if let Ok(mut state) = runtime.0.state.try_borrow_mut() {
            return self.retire_in_state(&mut state, &runtime.0.poisoned);
        }
        self.retire_with(&mut |value| {
            runtime.release_jsvalue(value)?;
            runtime.check_poison()
        })
    }
}
impl InvokeResume {
    fn empty(realm: ContextId) -> Self {
        Self(Box::new(InvokeResumeState {
            arguments: Vec::new(),
            arguments_value: None,
            realm,
            target: ForwardTarget::Empty,
            callability_temporary: None,
        }))
    }
    fn set_receiver(&mut self, receiver: JsValue) {
        let ForwardTarget::Call { receiver: slot, .. } = &mut self.0.target else {
            unreachable!()
        };
        *slot = Some(receiver);
    }
    fn type_error(
        &self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
        message: &str,
    ) -> Result<Effect, RuntimeError> {
        Ok(Effect::Complete(
            Completion::Throw(JsValue::Object(state.new_native_error_from_message(
                poisoned,
                self.0.realm,
                NativeErrorKind::Type,
                NativeErrorMessage::from_utf8(message),
            )?)),
            true,
        ))
    }
    fn start_body(
        &mut self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
        kind: InvokeKind,
        this_value: &JsValue,
        arguments: &NativeArguments,
    ) -> Result<Effect, RuntimeError> {
        if matches!(kind, InvokeKind::ReflectConstruct) {
            self.0.target = ForwardTarget::Construct {
                target: None,
                new_target: None,
            };
            if arguments.actual_arg_count > 2 {
                let value = arguments.readable.get(2).ok_or(RuntimeError::Invariant(
                    "Reflect.construct newTarget argv was not readable",
                ))?;
                if !matches!(value, JsValue::Object(_)) {
                    return Ok(Effect::Complete(
                        Completion::Throw(state.new_not_constructor_error_jsvalue(
                            poisoned,
                            self.0.realm,
                            value,
                        )?),
                        true,
                    ));
                }
                let owned = state.dup_jsvalue(value)?;
                match checked_constructor_owned(state, poisoned, self.0.realm, owned)? {
                    NativeConversion::Throw(value) => {
                        return Ok(Effect::Complete(Completion::Throw(value), true));
                    }
                    NativeConversion::Value(new_target) => {
                        let ForwardTarget::Construct {
                            new_target: slot, ..
                        } = &mut self.0.target
                        else {
                            unreachable!()
                        };
                        *slot = Some(InvokeNewTarget::Validated(new_target));
                    }
                }
            }
            // QuickJS ordering: explicit newTarget, list, then target producer.
            self.0.arguments_value = Some(state.dup_jsvalue(&arguments.readable[1])?);
            let target = state.dup_jsvalue(&arguments.readable[0])?;
            let ForwardTarget::Construct { target: slot, .. } = &mut self.0.target else {
                unreachable!()
            };
            *slot = Some(target);
            return Ok(Effect::Arguments);
        }
        let target = match kind {
            InvokeKind::Call => {
                let promoted = state.dup_jsvalue(this_value)?;
                let mut promoted = OwnedValueGuard::new(state, poisoned, promoted);
                let (state, promoted) = promoted.parts();
                let Some(JsValue::Object(id)) = promoted.as_ref() else {
                    state.release_owned_jsvalue(poisoned, promoted.take().expect("call target"))?;
                    return self.type_error(state, poisoned, "not a function");
                };
                let id = *id;
                let target = if state.object_id_has_call_capability(id)? {
                    InvokeCallTarget::Callable(id)
                } else if state.heap.object(id)?.kind == ObjectKind::Proxy {
                    InvokeCallTarget::NonCallableProxy(id)
                } else {
                    state.release_owned_jsvalue(poisoned, promoted.take().expect("call target"))?;
                    return self.type_error(state, poisoned, "not a function");
                };
                promoted.take();
                target
            }
            InvokeKind::Apply => {
                if !matches!(this_value, JsValue::Object(_)) {
                    return self.type_error(state, poisoned, "not a function");
                }
                match checked_callable(state, poisoned, this_value, true) {
                    Ok(target) => InvokeCallTarget::Callable(target),
                    Err(RuntimeError::Engine(error)) if error.kind() == ErrorKind::Type => {
                        return self.type_error(state, poisoned, "not a function");
                    }
                    Err(error) => return Err(error),
                }
            }
            InvokeKind::ReflectApply => InvokeCallTarget::Callable(checked_callable(
                state,
                poisoned,
                &arguments.readable[0],
                false,
            )?),
            InvokeKind::ReflectConstruct => unreachable!(),
        };
        self.0.target = ForwardTarget::Call {
            target: Some(target),
            receiver: None,
        };
        let receiver_index = usize::from(matches!(kind, InvokeKind::ReflectApply));
        self.set_receiver(
            state.dup_jsvalue(
                arguments
                    .readable
                    .get(receiver_index)
                    .unwrap_or(&JsValue::Undefined),
            )?,
        );
        if matches!(kind, InvokeKind::Call) {
            let copied = (|| {
                self.0
                    .arguments
                    .try_reserve_exact(arguments.actual_arg_count.saturating_sub(1))
                    .map_err(|_| RuntimeError::Invariant("function.call argv allocation failed"))?;
                for value in arguments.readable[..arguments.actual_arg_count]
                    .iter()
                    .skip(1)
                {
                    self.0.arguments.push(state.dup_jsvalue(value)?);
                }
                Ok::<_, RuntimeError>(())
            })();
            if let Err(error) = copied {
                // Call's receiver precedes the copied argv prefix on rollback.
                // Keep the suffix and target armed if this first retirement
                // fails; finish_effect observes poison before traversing them.
                let ForwardTarget::Call { receiver, .. } = &mut self.0.target else {
                    unreachable!()
                };
                state.release_owned_jsvalue(poisoned, receiver.take().expect("Call receiver"))?;
                return Err(error);
            }
            #[cfg(feature = "profiling")]
            {
                crate::engine::api::profiling::record_call_buffer_capacity(
                    "function.call_suffix",
                    0,
                    self.0.arguments.capacity(),
                    size_of::<JsValue>(),
                );
                crate::engine::api::profiling::record_call_buffer_js_value_copies(
                    "function.call_suffix",
                    &self.0.arguments,
                );
            }
            return Ok(Effect::Call);
        }
        self.0.arguments_value = Some(state.dup_jsvalue(&arguments.readable[receiver_index + 1])?);
        if matches!(kind, InvokeKind::Apply)
            && matches!(
                self.0.arguments_value,
                Some(JsValue::Null | JsValue::Undefined)
            )
        {
            self.0.arguments_value.take();
            return Ok(Effect::Call);
        }
        Ok(Effect::Arguments)
    }
    pub(crate) fn take_arguments_value(&mut self) -> JsValue {
        self.0
            .arguments_value
            .take()
            .expect("InvokeStep Arguments value")
    }
    pub(crate) fn arguments(
        self,
        runtime: &Runtime,
        result: NativeConversion<Vec<JsValue>>,
    ) -> Result<InvokeStep, RuntimeError> {
        let _unwind = runtime.unwind_guard();
        let result = self.arguments_in_state(
            &mut runtime.0.state.borrow_mut(),
            &runtime.0.poisoned,
            result,
        );
        runtime.check_poison()?;
        result
    }
    pub(crate) fn arguments_in_state(
        mut self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
        result: NativeConversion<Vec<JsValue>>,
    ) -> Result<InvokeStep, RuntimeError> {
        let result = match result {
            NativeConversion::Throw(value) => Ok(Effect::Complete(Completion::Throw(value), false)),
            NativeConversion::Value(arguments) => {
                #[cfg(feature = "profiling")]
                crate::engine::api::profiling::record_call_buffer_observed(
                    "invoke.argv_carrier",
                    arguments.capacity(),
                    size_of::<JsValue>(),
                );
                self.0.arguments = arguments;
                self.after_arguments(state, poisoned)
            }
        };
        self.finish_effect(state, poisoned, result)
    }
    fn after_arguments(
        &mut self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
    ) -> Result<Effect, RuntimeError> {
        let ForwardTarget::Construct { target, new_target } = &mut self.0.target else {
            return Ok(Effect::Call);
        };
        let owned = target.take().expect("forward constructor target");
        let target_id = match checked_constructor_owned(state, poisoned, self.0.realm, owned)? {
            NativeConversion::Value(target) => target,
            NativeConversion::Throw(value) => {
                return Ok(Effect::Complete(Completion::Throw(value), true));
            }
        };
        *target = Some(JsValue::Object(target_id));
        if new_target.is_none() {
            let JsValue::Object(id) = state.dup_jsvalue(&JsValue::Object(target_id))? else {
                unreachable!()
            };
            *new_target = Some(InvokeNewTarget::Validated(id));
        }
        Ok(Effect::Construct)
    }
    fn finish_effect(
        mut self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
        result: Result<Effect, RuntimeError>,
    ) -> Result<InvokeStep, RuntimeError> {
        if poisoned.get() {
            return Err(RuntimeError::Poisoned);
        }
        match result {
            Ok(Effect::Arguments) => Ok(InvokeStep::Arguments { resume: self }),
            Ok(Effect::Call) => {
                let ForwardTarget::Call { target, receiver } = &mut self.0.target else {
                    unreachable!()
                };
                Ok(InvokeStep::Call(Box::new(InvokeCall {
                    target: target.take().expect("forward callee"),
                    receiver: receiver.take().expect("forward receiver"),
                    arguments: std::mem::take(&mut self.0.arguments),
                })))
            }
            Ok(Effect::Construct) => {
                let ForwardTarget::Construct { target, new_target } = &mut self.0.target else {
                    unreachable!()
                };
                let Some(JsValue::Object(target)) = target.take() else {
                    unreachable!()
                };
                Ok(InvokeStep::Construct(Box::new(InvokeConstruct {
                    target,
                    new_target: new_target.take().expect("forward newTarget"),
                    arguments: std::mem::take(&mut self.0.arguments),
                })))
            }
            Ok(Effect::Complete(completion, fresh)) => {
                let throwing = matches!(completion, Completion::Throw(_));
                let (Completion::Return(value) | Completion::Throw(value)) = completion;
                let mut result = OwnedValueGuard::new(state, poisoned, value);
                let (state, result) = result.parts();
                self.retire_in_state(state, poisoned)?;
                let value = result.take().expect("Invoke completion");
                let completion = if throwing {
                    Completion::Throw(value)
                } else {
                    Completion::Return(value)
                };
                Ok(if fresh {
                    InvokeStep::CyclePublished(completion)
                } else {
                    InvokeStep::Complete(completion)
                })
            }
            Err(error) => {
                let realm = self.0.realm;
                self.retire_in_state(state, poisoned)?;
                if let RuntimeError::Engine(error) = &error
                    && let Some(kind) =
                        crate::engine::api::error::NativeErrorKind::from_javascript_error(
                            error.kind(),
                        )
                {
                    let message = error.native_message().cloned().unwrap_or_else(|| {
                        crate::engine::api::error::NativeErrorMessage::from_utf8(error.message())
                    });
                    let object =
                        state.new_native_error_from_message(poisoned, realm, kind, message)?;
                    return Ok(InvokeStep::CyclePublished(Completion::Throw(
                        JsValue::Object(object),
                    )));
                }
                Err(error)
            }
        }
    }
    pub(crate) fn retire_in_state(
        self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
    ) -> Result<(), RuntimeError> {
        self.retire_with(&mut |value| state.release_owned_jsvalue(poisoned, value))
    }
    fn retire_with(
        mut self,
        release: &mut impl FnMut(JsValue) -> Result<(), RuntimeError>,
    ) -> Result<(), RuntimeError> {
        if let Some(value) = self.0.arguments_value.take() {
            release(value)?;
        }
        for value in self.0.arguments.drain(..) {
            release(value)?;
        }
        std::mem::replace(&mut self.0.target, ForwardTarget::Empty).retire_with(release)?;
        if let Some(callable) = self.0.callability_temporary.take() {
            release(JsValue::Object(callable))?;
        }
        Ok(())
    }
    pub(crate) fn retire_at_boundary(self, runtime: &Runtime) -> Result<(), RuntimeError> {
        self.retire_with(&mut |value| {
            if runtime.skip_cleanup() {
                return Err(RuntimeError::Poisoned);
            }
            runtime.release_jsvalue(value)?;
            runtime.check_poison()
        })
    }
}
impl ForwardTarget {
    fn retire_with(
        self,
        release: &mut impl FnMut(JsValue) -> Result<(), RuntimeError>,
    ) -> Result<(), RuntimeError> {
        match self {
            Self::Empty => Ok(()),
            Self::Call { target, receiver } => {
                if let Some(receiver) = receiver {
                    release(receiver)?;
                }
                if let Some(target) = target {
                    release(JsValue::Object(target.id()))?;
                }
                Ok(())
            }
            Self::Construct { target, new_target } => {
                if let Some(target) = target {
                    release(target)?;
                }
                if let Some(new_target) = new_target {
                    new_target.retire_with(release)?;
                }
                Ok(())
            }
        }
    }
}
impl InvokeNewTarget {
    fn retire_with(
        self,
        release: &mut impl FnMut(JsValue) -> Result<(), RuntimeError>,
    ) -> Result<(), RuntimeError> {
        release(match self {
            Self::Validated(id) => JsValue::Object(id),
            Self::Raw(value) => value,
        })
    }
}
fn checked_callable(
    state: &mut RuntimeState,
    poisoned: &Cell<bool>,
    value: &JsValue,
    extra_temporary: bool,
) -> Result<ObjectId, RuntimeError> {
    let JsValue::Object(id) = value else {
        return Err(RuntimeError::Engine(Error::new(
            ErrorKind::Type,
            "not a function",
        )));
    };
    let id = *id;
    let promoted = state.dup_jsvalue(value)?;
    let mut promoted = OwnedValueGuard::new(state, poisoned, promoted);
    let (state, promoted) = promoted.parts();
    if !state.object_id_has_call_capability(id)? {
        state.release_owned_jsvalue(
            poisoned,
            promoted.take().expect("rejected callable temporary"),
        )?;
        return Err(RuntimeError::Engine(Error::new(
            ErrorKind::Type,
            "not a function",
        )));
    }
    if extra_temporary {
        let callable = state.dup_jsvalue(promoted.as_ref().expect("callable temporary"))?;
        let mut callable = OwnedValueGuard::new(state, poisoned, callable);
        let (state, callable) = callable.parts();
        state.release_owned_jsvalue(poisoned, promoted.take().expect("callable temporary"))?;
        callable.take();
    } else {
        promoted.take();
    }
    Ok(id)
}
fn checked_constructor_owned(
    state: &mut RuntimeState,
    poisoned: &Cell<bool>,
    realm: ContextId,
    value: JsValue,
) -> Result<NativeConversion<ObjectId>, RuntimeError> {
    let mut value = OwnedValueGuard::new(state, poisoned, value);
    let (state, value) = value.parts();
    let Some(JsValue::Object(id)) = value.as_ref() else {
        state.release_owned_jsvalue(poisoned, value.take().expect("constructor input"))?;
        return Err(RuntimeError::Engine(Error::new(
            ErrorKind::Type,
            "not a function",
        )));
    };
    let id = *id;
    if state.heap.object(id)?.is_constructor {
        value.take();
        return Ok(NativeConversion::Value(id));
    }
    let error = state.new_not_constructor_error_jsvalue(
        poisoned,
        realm,
        value.as_ref().expect("constructor input"),
    )?;
    let mut error = OwnedValueGuard::new(state, poisoned, error);
    let (state, error) = error.parts();
    state.release_owned_jsvalue(poisoned, value.take().expect("constructor input"))?;
    Ok(NativeConversion::Throw(
        error.take().expect("constructor throw"),
    ))
}
pub(crate) fn finish(
    runtime: &Runtime,
    realm: ContextId,
    step: InvokeStep,
) -> Result<Completion, RuntimeError> {
    crate::engine::vm::execute_invoke_step(runtime, realm, step).map_err(RuntimeError::from)
}
const _: () = assert!(std::mem::size_of::<InvokeStep>() <= 64);

#[cfg(test)]
#[path = "invoke/state_tests.rs"]
mod state_tests;
