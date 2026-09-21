//! Async-from-Sync continuation observes both result fields before assimilation.
use crate::engine::api::{error::NativeErrorKind, runtime::Runtime, runtime_error::RuntimeError};
use crate::engine::builtins::{
    native::{GeneratorResumeKind, NativeFunctionId},
    promise::RootedPromiseCapability,
};
use crate::engine::heap::{ContextId, HeapError, InternalCallableData};
use crate::engine::object::{CallableRef, ObjectRef, PropertyKey};
use crate::engine::value::{JsValue, Value, conversion::NativeConversion};
use crate::engine::vm::{
    Completion,
    call::{NativeArguments, NativeInvocation},
};

/// Reconstruct one owned internal value from the dormant wrapper record,
/// retaining every edge so the decoded value owns them independently.
fn decode_raw_jsvalue(
    runtime: &Runtime,
    raw: crate::engine::heap::RawValue,
) -> Result<JsValue, RuntimeError> {
    let value = JsValue::from_raw(raw).ok_or(RuntimeError::Invariant(
        "Async-from-Sync wrapper held an internal-only sentinel",
    ))?;
    runtime.dup_jsvalue(&value)
}

pub(crate) enum FromSyncStep {
    Complete(Completion),
    Read { resume: Box<FromSyncResume> },
    Call { resume: Box<FromSyncResume> },
    Resolve { resume: Box<FromSyncResume> },
    Close { resume: Box<FromSyncResume> },
}
pub(crate) struct FromSyncResume {
    runtime: Runtime,
    pending_effect: FromSyncStepPending,
    realm: ContextId,
    phase: Phase,
}
impl Drop for FromSyncResume {
    fn drop(&mut self) {
        for value in [
            self.pending_effect.read_receiver.take(),
            self.pending_effect.call_receiver.take(),
            self.pending_effect.resolve_value.take(),
        ]
        .into_iter()
        .flatten()
        {
            let _ = self.runtime.release_jsvalue(value);
        }
        if let Some(values) = self.pending_effect.call_arguments.take() {
            for value in values {
                let _ = self.runtime.release_jsvalue(value);
            }
        }
        if let Some(Completion::Return(value) | Completion::Throw(value)) =
            self.pending_effect.close_completion.take()
        {
            let _ = self.runtime.release_jsvalue(value);
        }
        match &mut self.phase {
            Phase::Method(state)
            | Phase::Result(state)
            | Phase::Done { state, .. }
            | Phase::Value { state, .. }
            | Phase::Promise { state, .. } => {
                for value in state.arguments.drain(..) {
                    let _ = self.runtime.release_jsvalue(value);
                }
            }
            _ => {}
        }
    }
}
enum Phase {
    Method(State),
    Result(State),
    Done { state: State, result: ObjectRef },
    Value { state: State, done: bool },
    Promise { state: State, done: bool },
    MissingThrow(RootedPromiseCapability),
    Reject(RootedPromiseCapability),
    Settled(ObjectRef),
    Identity,
}
struct State {
    capability: RootedPromiseCapability,
    iterator: ObjectRef,
    kind: GeneratorResumeKind,
    arguments: Vec<JsValue>,
}
fn continuation(runtime: &Runtime, realm: ContextId, phase: Phase) -> Box<FromSyncResume> {
    Box::new(FromSyncResume {
        runtime: runtime.clone(),
        pending_effect: FromSyncStepPending::default(),
        realm,
        phase,
    })
}
fn settle(
    runtime: &Runtime,
    realm: ContextId,
    capability: RootedPromiseCapability,
    completion: Completion,
) -> Result<FromSyncStep, RuntimeError> {
    let (callable, value) = match completion {
        Completion::Return(value) => (capability.resolve, value),
        Completion::Throw(value) => (capability.reject, value),
    };
    Ok({
        let __pending_field_callable = callable;
        let __pending_field_receiver = JsValue::Undefined;
        let __pending_field_arguments = vec![value];
        let __pending_field_resume =
            continuation(runtime, realm, Phase::Settled(capability.promise));
        FromSyncStep::request_call(
            __pending_field_callable,
            __pending_field_receiver,
            __pending_field_arguments,
            __pending_field_resume,
        )
    })
}
impl FromSyncStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        target: NativeFunctionId,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Self, RuntimeError> {
        if target == NativeFunctionId::AsyncFromSyncIteratorUnwrap {
            return runtime
                .call_async_from_sync_iterator_unwrap(realm, invocation.dup(runtime)?, arguments)
                .map(Self::Complete);
        }
        if target == NativeFunctionId::AsyncFromSyncIteratorClose {
            let NativeInvocation::Call { .. } = invocation else {
                return Err(RuntimeError::Invariant(
                    "Async-from-Sync close did not receive a call invocation",
                ));
            };
            let active = runtime.active_function()?;
            let internal = runtime
                .0
                .state
                .borrow()
                .heap
                .native_internal_callable(active.object_id())?
                .ok_or(RuntimeError::Invariant(
                    "Async-from-Sync close had no internal capture",
                ))?;
            let InternalCallableData::AsyncFromSyncIteratorClose { sync_iterator } = internal
            else {
                return Err(RuntimeError::Invariant(
                    "Async-from-Sync close had the wrong internal capture",
                ));
            };
            let reason = arguments
                .readable
                .first()
                .map(|value| runtime.dup_jsvalue(value))
                .transpose()?
                .ok_or(RuntimeError::Invariant(
                    "Async-from-Sync close argv was not padded",
                ))?;
            let iterator = ObjectRef::from_borrowed_handle(runtime.clone(), sync_iterator)?;
            return Ok({
                let __pending_field_iterator = iterator;
                let __pending_field_completion = Completion::Throw(reason);
                let __pending_field_resume = continuation(runtime, realm, Phase::Identity);
                Self::request_close(
                    __pending_field_iterator,
                    __pending_field_completion,
                    __pending_field_resume,
                )
            });
        }
        let NativeFunctionId::AsyncFromSyncIteratorResume(kind) = target else {
            return Err(RuntimeError::Invariant("wrong Async-from-Sync operation"));
        };
        // Allocation occurs even for a bad receiver, matching the intrinsic.
        let capability = runtime.new_default_promise_capability(realm)?;
        let NativeInvocation::Call { this_value } = invocation else {
            return Err(RuntimeError::Invariant(
                "Async-from-Sync resume did not receive a call invocation",
            ));
        };
        let argument = if arguments.actual_arg_count == 0 {
            None
        } else {
            Some(
                arguments
                    .readable
                    .first()
                    .map(|value| runtime.dup_jsvalue(value))
                    .transpose()?
                    .ok_or(RuntimeError::Invariant(
                        "Async-from-Sync resume argv was not padded",
                    ))?,
            )
        };
        let receiver = if let JsValue::Object(receiver) = this_value {
            Some(ObjectRef::from_borrowed_handle(runtime.clone(), *receiver)?)
        } else {
            None
        };
        let state = match receiver {
            Some(receiver) => runtime
                .0
                .state
                .borrow()
                .heap
                .async_from_sync_iterator_state(receiver.object_id()),
            None => Err(HeapError::Invariant("invalid Async-from-Sync receiver")),
        };
        let (iterator, cached_next) = match state {
            Ok(state) => state,
            Err(HeapError::Invariant(_)) => {
                let reason = runtime.new_native_error_jsvalue(
                    realm,
                    NativeErrorKind::Type,
                    "not an Async-from-Sync Iterator",
                )?;
                return settle(runtime, realm, capability, Completion::Throw(reason));
            }
            Err(error) => return Err(error.into()),
        };
        let iterator = ObjectRef::from_borrowed_handle(runtime.clone(), iterator)?;
        let state = State {
            capability,
            iterator,
            kind,
            arguments: match argument {
                Some(argument) => vec![argument],
                None => Vec::new(),
            },
        };
        match kind {
            GeneratorResumeKind::Next => continuation(runtime, realm, Phase::Method(state)).resume(
                runtime,
                Completion::Return(decode_raw_jsvalue(runtime, cached_next.clone())?),
            ),
            GeneratorResumeKind::Return | GeneratorResumeKind::Throw => Ok({
                let __pending_field_receiver =
                    runtime.into_jsvalue(Value::Object(state.iterator.clone()))?;
                let __pending_field_key =
                    runtime.intern_property_key(if kind == GeneratorResumeKind::Return {
                        "return"
                    } else {
                        "throw"
                    })?;
                let __pending_field_resume = continuation(runtime, realm, Phase::Method(state));
                Self::request_read(
                    __pending_field_receiver,
                    __pending_field_key,
                    __pending_field_resume,
                )
            }),
        }
    }
    pub(crate) fn finish(
        self,
        runtime: &Runtime,
        realm: ContextId,
    ) -> Result<Completion, RuntimeError> {
        {
            super::super::driver::execute_root(
                runtime.clone(),
                realm,
                super::super::driver::RootOperation::FromSync(self),
            )
            .map_err(RuntimeError::Engine)
        }
    }
}
impl FromSyncResume {
    pub(crate) fn resume(
        mut self: Box<Self>,
        runtime: &Runtime,
        completion: Completion,
    ) -> Result<FromSyncStep, RuntimeError> {
        let realm = self.realm;
        let phase = std::mem::replace(&mut self.phase, Phase::Identity);
        if let Phase::Identity = phase {
            return Ok(FromSyncStep::Complete(completion));
        }
        if let Phase::Settled(promise) = phase {
            return match completion {
                Completion::Return(value) => {
                    runtime.release_jsvalue(value)?;
                    Ok(FromSyncStep::Complete(Completion::Return(
                        runtime.into_jsvalue(Value::Object(promise))?,
                    )))
                }
                Completion::Throw(value) => {
                    runtime.release_jsvalue(value)?;
                    Err(RuntimeError::Invariant(
                        "intrinsic Promise resolving function threw",
                    ))
                }
            };
        }
        let value = match completion {
            Completion::Return(value) => value,
            Completion::Throw(reason) => {
                return Ok(match phase {
                    Phase::Promise { state, done }
                        if state.kind != GeneratorResumeKind::Return && !done =>
                    {
                        let __pending_field_iterator = state.iterator;
                        let __pending_field_completion = Completion::Throw(reason);
                        let __pending_field_resume =
                            self.continue_with(Phase::Reject(state.capability));
                        FromSyncStep::request_close(
                            __pending_field_iterator,
                            __pending_field_completion,
                            __pending_field_resume,
                        )
                    }
                    Phase::Method(state)
                    | Phase::Result(state)
                    | Phase::Done { state, .. }
                    | Phase::Value { state, .. }
                    | Phase::Promise { state, .. } => {
                        for argument in state.arguments {
                            runtime.release_jsvalue(argument)?;
                        }
                        self.settle(runtime, state.capability, Completion::Throw(reason))?
                    }
                    Phase::MissingThrow(capability) | Phase::Reject(capability) => {
                        self.settle(runtime, capability, Completion::Throw(reason))?
                    }
                    Phase::Identity | Phase::Settled(_) => unreachable!(),
                });
            }
        };
        match phase {
            Phase::Method(mut state) => {
                if matches!(value, JsValue::Undefined | JsValue::Null) {
                    return Ok(match state.kind {
                        GeneratorResumeKind::Return => {
                            let value = state
                                .arguments
                                .into_iter()
                                .next()
                                .unwrap_or(JsValue::Undefined);
                            let result = runtime.new_iterator_result_jsvalue(realm, value, true)?;
                            self.settle(
                                runtime,
                                state.capability,
                                Completion::Return(runtime.into_jsvalue(Value::Object(result))?),
                            )?
                        }
                        GeneratorResumeKind::Throw => {
                            for argument in std::mem::take(&mut state.arguments) {
                                runtime.release_jsvalue(argument)?;
                            }
                            let __pending_field_iterator = state.iterator;
                            let __pending_field_completion = Completion::Return(JsValue::Undefined);
                            let __pending_field_resume =
                                self.continue_with(Phase::MissingThrow(state.capability));
                            FromSyncStep::request_close(
                                __pending_field_iterator,
                                __pending_field_completion,
                                __pending_field_resume,
                            )
                        }
                        GeneratorResumeKind::Next => {
                            let reason = runtime.new_native_error_jsvalue(
                                realm,
                                NativeErrorKind::Type,
                                "not a function",
                            )?;
                            self.settle(runtime, state.capability, Completion::Throw(reason))?
                        }
                    });
                }
                let callable = runtime.async_from_sync_callable_jsvalue(realm, &value);
                runtime.release_jsvalue(value)?;
                let callable = match callable {
                    Ok(callable) => callable,
                    Err(error) => {
                        for argument in std::mem::take(&mut state.arguments) {
                            runtime.release_jsvalue(argument)?;
                        }
                        return Err(error);
                    }
                };
                let callable = match callable {
                    NativeConversion::Value(callable) => callable,
                    NativeConversion::Throw(reason) => {
                        for argument in std::mem::take(&mut state.arguments) {
                            runtime.release_jsvalue(argument)?;
                        }
                        return self.settle(
                            runtime,
                            state.capability,
                            Completion::Throw(runtime.into_jsvalue(reason)?),
                        );
                    }
                };
                Ok({
                    let __pending_field_callable = callable;
                    let __pending_field_receiver =
                        runtime.into_jsvalue(Value::Object(state.iterator.clone()))?;
                    let __pending_field_arguments = std::mem::take(&mut state.arguments);
                    let __pending_field_resume = self.continue_with(Phase::Result(state));
                    FromSyncStep::request_call(
                        __pending_field_callable,
                        __pending_field_receiver,
                        __pending_field_arguments,
                        __pending_field_resume,
                    )
                })
            }
            Phase::Result(state) => {
                let Value::Object(result) = runtime.root_and_release_jsvalue(value)? else {
                    let reason = runtime.new_native_error_jsvalue(
                        realm,
                        NativeErrorKind::Type,
                        "iterator must return an object",
                    )?;
                    return self.settle(runtime, state.capability, Completion::Throw(reason));
                };
                Ok({
                    let __pending_field_receiver =
                        runtime.into_jsvalue(Value::Object(result.clone()))?;
                    let __pending_field_key = runtime
                        .pinned_property_key(crate::engine::atom::pinned::PinnedAtom::Done)?;
                    let __pending_field_resume = self.continue_with(Phase::Done { state, result });
                    FromSyncStep::request_read(
                        __pending_field_receiver,
                        __pending_field_key,
                        __pending_field_resume,
                    )
                })
            }
            Phase::Done { state, result } => {
                let done = runtime.value_to_boolean_jsvalue(&value);
                runtime.release_jsvalue(value)?;
                let done = done?;
                Ok({
                    let __pending_field_receiver = runtime.into_jsvalue(Value::Object(result))?;
                    let __pending_field_key = runtime
                        .pinned_property_key(crate::engine::atom::pinned::PinnedAtom::Value)?;
                    let __pending_field_resume = self.continue_with(Phase::Value { state, done });
                    FromSyncStep::request_read(
                        __pending_field_receiver,
                        __pending_field_key,
                        __pending_field_resume,
                    )
                })
            }
            Phase::Value { state, done } => Ok({
                let __pending_field_value = value;
                let __pending_field_realm = realm;
                let __pending_field_resume = self.continue_with(Phase::Promise { state, done });
                FromSyncStep::request_resolve(
                    __pending_field_value,
                    __pending_field_realm,
                    __pending_field_resume,
                )
            }),
            Phase::Promise { state, done } => {
                let Value::Object(promise) = runtime.root_and_release_jsvalue(value)? else {
                    return Err(RuntimeError::Invariant(
                        "intrinsic PromiseResolve returned a non-object",
                    ));
                };
                let unwrap = runtime.new_internal_promise_function(
                    realm,
                    NativeFunctionId::AsyncFromSyncIteratorUnwrap,
                    1,
                    1,
                    InternalCallableData::AsyncFromSyncIteratorUnwrap { done },
                )?;
                let close = if state.kind != GeneratorResumeKind::Return && !done {
                    Some(runtime.new_internal_promise_function(
                        realm,
                        NativeFunctionId::AsyncFromSyncIteratorClose,
                        1,
                        1,
                        InternalCallableData::AsyncFromSyncIteratorClose {
                            sync_iterator: state.iterator.object_id(),
                        },
                    )?)
                } else {
                    None
                };
                runtime.perform_promise_then_with_capability(
                    realm,
                    &promise,
                    Some(&unwrap),
                    close.as_ref(),
                    &state.capability,
                )?;
                Ok(FromSyncStep::Complete(Completion::Return(
                    runtime.into_jsvalue(Value::Object(state.capability.promise))?,
                )))
            }
            Phase::MissingThrow(capability) => {
                let reason = runtime.new_native_error_jsvalue(
                    realm,
                    NativeErrorKind::Type,
                    "throw is not a method",
                )?;
                self.settle(runtime, capability, Completion::Throw(reason))
            }
            Phase::Reject(_) | Phase::Identity | Phase::Settled(_) => {
                Err(RuntimeError::Invariant("Async-from-Sync unexpected reply"))
            }
        }
    }
}
impl FromSyncResume {
    fn continue_with(mut self: Box<Self>, phase: Phase) -> Box<Self> {
        self.phase = phase;
        self
    }
    fn settle(
        self: Box<Self>,
        _runtime: &Runtime,
        capability: RootedPromiseCapability,
        completion: Completion,
    ) -> Result<FromSyncStep, RuntimeError> {
        let (callable, value) = match completion {
            Completion::Return(value) => (capability.resolve, value),
            Completion::Throw(value) => (capability.reject, value),
        };
        Ok(FromSyncStep::request_call(
            callable,
            JsValue::Undefined,
            vec![value],
            self.continue_with(Phase::Settled(capability.promise)),
        ))
    }
}

#[derive(Default)]
struct FromSyncStepPending {
    read_receiver: Option<JsValue>,
    read_key: Option<PropertyKey>,
    call_callable: Option<CallableRef>,
    call_receiver: Option<JsValue>,
    call_arguments: Option<Vec<JsValue>>,
    resolve_value: Option<JsValue>,
    resolve_realm: Option<ContextId>,
    close_iterator: Option<ObjectRef>,
    close_completion: Option<Completion>,
}
impl FromSyncStep {
    pub(crate) fn request_read(
        receiver: JsValue,
        key: PropertyKey,
        mut resume: Box<FromSyncResume>,
    ) -> Self {
        resume.pending_effect.read_receiver = Some(receiver);
        resume.pending_effect.read_key = Some(key);
        Self::Read { resume }
    }
    pub(crate) fn request_call(
        callable: CallableRef,
        receiver: JsValue,
        arguments: Vec<JsValue>,
        mut resume: Box<FromSyncResume>,
    ) -> Self {
        resume.pending_effect.call_callable = Some(callable);
        resume.pending_effect.call_receiver = Some(receiver);
        resume.pending_effect.call_arguments = Some(arguments);
        Self::Call { resume }
    }
    pub(crate) fn request_resolve(
        value: JsValue,
        realm: ContextId,
        mut resume: Box<FromSyncResume>,
    ) -> Self {
        resume.pending_effect.resolve_value = Some(value);
        resume.pending_effect.resolve_realm = Some(realm);
        Self::Resolve { resume }
    }
    pub(crate) fn request_close(
        iterator: ObjectRef,
        completion: Completion,
        mut resume: Box<FromSyncResume>,
    ) -> Self {
        resume.pending_effect.close_iterator = Some(iterator);
        resume.pending_effect.close_completion = Some(completion);
        Self::Close { resume }
    }
}
impl FromSyncResume {
    pub(crate) fn take_read_receiver(&mut self) -> JsValue {
        self.pending_effect
            .read_receiver
            .take()
            .expect("FromSyncStep Read receiver")
    }
    pub(crate) fn take_read_key(&mut self) -> PropertyKey {
        self.pending_effect
            .read_key
            .take()
            .expect("FromSyncStep Read key")
    }
    pub(crate) fn take_call_callable(&mut self) -> CallableRef {
        self.pending_effect
            .call_callable
            .take()
            .expect("FromSyncStep Call callable")
    }
    pub(crate) fn take_call_receiver(&mut self) -> JsValue {
        self.pending_effect
            .call_receiver
            .take()
            .expect("FromSyncStep Call receiver")
    }
    pub(crate) fn take_call_arguments(&mut self) -> Vec<JsValue> {
        self.pending_effect
            .call_arguments
            .take()
            .expect("FromSyncStep Call arguments")
    }
    pub(crate) fn take_resolve_value(&mut self) -> JsValue {
        self.pending_effect
            .resolve_value
            .take()
            .expect("FromSyncStep Resolve value")
    }
    pub(crate) fn take_resolve_realm(&mut self) -> ContextId {
        self.pending_effect
            .resolve_realm
            .take()
            .expect("FromSyncStep Resolve realm")
    }
    pub(crate) fn take_close_iterator(&mut self) -> ObjectRef {
        self.pending_effect
            .close_iterator
            .take()
            .expect("FromSyncStep Close iterator")
    }
    pub(crate) fn take_close_completion(&mut self) -> Completion {
        self.pending_effect
            .close_completion
            .take()
            .expect("FromSyncStep Close completion")
    }
}
const _: () = assert!(std::mem::size_of::<FromSyncStep>() <= 64);

// S11 all-domain protocol bound; inline completion stays allocation-free.
const _: () = assert!(std::mem::size_of::<FromSyncStep>() <= 64);
