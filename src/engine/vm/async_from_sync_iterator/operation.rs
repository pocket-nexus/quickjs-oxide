//! Async-from-Sync continuation observes both result fields before assimilation.
use crate::engine::api::{error::NativeErrorKind, runtime::Runtime, runtime_error::RuntimeError};
use crate::engine::builtins::{
    native::{GeneratorResumeKind, NativeFunctionId},
    promise::RootedPromiseCapability,
};
use crate::engine::heap::{ContextId, HeapError, InternalCallableData};
use crate::engine::object::{CallableRef, ObjectRef, PropertyKey};
use crate::engine::value::{Value, conversion::NativeConversion};
use crate::engine::vm::{
    Completion,
    call::{NativeArguments, NativeInvocation},
};
pub(crate) enum FromSyncStep {
    Complete(Completion),
    Read {
        receiver: Value,
        key: PropertyKey,
        resume: Box<FromSyncResume>,
    },
    Call {
        callable: CallableRef,
        receiver: Value,
        arguments: Vec<Value>,
        resume: Box<FromSyncResume>,
    },
    Resolve {
        value: Value,
        realm: ContextId,
        resume: Box<FromSyncResume>,
    },
    Close {
        iterator: ObjectRef,
        completion: Completion,
        resume: Box<FromSyncResume>,
    },
}
pub(crate) struct FromSyncResume {
    realm: ContextId,
    phase: Phase,
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
    arguments: Vec<Value>,
}
fn continuation(realm: ContextId, phase: Phase) -> Box<FromSyncResume> {
    Box::new(FromSyncResume { realm, phase })
}
fn settle(
    realm: ContextId,
    capability: RootedPromiseCapability,
    completion: Completion,
) -> FromSyncStep {
    let (callable, value) = match completion {
        Completion::Return(value) => (capability.resolve, value),
        Completion::Throw(value) => (capability.reject, value),
    };
    FromSyncStep::Call {
        callable,
        receiver: Value::Undefined,
        arguments: vec![value],
        resume: continuation(realm, Phase::Settled(capability.promise)),
    }
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
                .call_async_from_sync_iterator_unwrap(realm, invocation.clone(), arguments)
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
                .cloned()
                .ok_or(RuntimeError::Invariant(
                    "Async-from-Sync close argv was not padded",
                ))?;
            let iterator = ObjectRef::from_borrowed_handle(runtime.clone(), sync_iterator)?;
            return Ok(Self::Close {
                iterator,
                completion: Completion::Throw(reason),
                resume: continuation(realm, Phase::Identity),
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
        let argument = arguments
            .readable
            .first()
            .cloned()
            .ok_or(RuntimeError::Invariant(
                "Async-from-Sync resume argv was not padded",
            ))?;
        let receiver = if let Value::Object(receiver) = this_value {
            Some(receiver)
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
                let reason = runtime.new_native_error(
                    realm,
                    NativeErrorKind::Type,
                    "not an Async-from-Sync Iterator",
                )?;
                return Ok(settle(realm, capability, Completion::Throw(reason)));
            }
            Err(error) => return Err(error.into()),
        };
        let iterator = ObjectRef::from_borrowed_handle(runtime.clone(), iterator)?;
        let state = State {
            capability,
            iterator,
            kind,
            arguments: if arguments.actual_arg_count == 0 {
                Vec::new()
            } else {
                vec![argument]
            },
        };
        match kind {
            GeneratorResumeKind::Next => continuation(realm, Phase::Method(state)).resume(
                runtime,
                Completion::Return(runtime.root_raw_value(&cached_next)?),
            ),
            GeneratorResumeKind::Return | GeneratorResumeKind::Throw => Ok(Self::Read {
                receiver: Value::Object(state.iterator.clone()),
                key: runtime.intern_property_key(if kind == GeneratorResumeKind::Return {
                    "return"
                } else {
                    "throw"
                })?,
                resume: continuation(realm, Phase::Method(state)),
            }),
        }
    }
    pub(crate) fn finish(
        self,
        runtime: &Runtime,
        realm: ContextId,
    ) -> Result<Completion, RuntimeError> {
        #[cfg(feature = "stack-vm")]
        {
            super::super::driver::execute_root(
                runtime.clone(),
                realm,
                super::super::driver::RootOperation::FromSync(self),
            )
            .map_err(RuntimeError::Engine)
        }
        #[cfg(not(feature = "stack-vm"))]
        {
            let mut step = self;
            loop {
                step = match step {
                    Self::Complete(completion) => return Ok(completion),
                    Self::Read {
                        receiver,
                        key,
                        resume,
                    } => resume.resume(
                        runtime,
                        runtime.get_value_property_in_realm(realm, receiver, &key)?,
                    )?,
                    Self::Call {
                        callable,
                        receiver,
                        arguments,
                        resume,
                    } => resume.resume(
                        runtime,
                        runtime.call_internal(realm, &callable, receiver, &arguments)?,
                    )?,
                    Self::Resolve {
                        value,
                        realm,
                        resume,
                    } => {
                        resume.resume(runtime, runtime.promise_resolve_intrinsic(realm, value)?)?
                    }
                    Self::Close {
                        iterator,
                        completion,
                        resume,
                    } => {
                        let close = crate::engine::builtins::IteratorCloseStep::start(
                            runtime, realm, iterator, completion,
                        )?;
                        resume.resume(
                            runtime,
                            crate::engine::builtins::finish_iterator_close(runtime, realm, close)?,
                        )?
                    }
                };
            }
        }
    }
}
impl FromSyncResume {
    pub(crate) fn resume(
        self: Box<Self>,
        runtime: &Runtime,
        completion: Completion,
    ) -> Result<FromSyncStep, RuntimeError> {
        let realm = self.realm;
        if let Phase::Identity = self.phase {
            return Ok(FromSyncStep::Complete(completion));
        }
        if let Phase::Settled(promise) = self.phase {
            return match completion {
                Completion::Return(_) => Ok(FromSyncStep::Complete(Completion::Return(
                    Value::Object(promise),
                ))),
                Completion::Throw(_) => Err(RuntimeError::Invariant(
                    "intrinsic Promise resolving function threw",
                )),
            };
        }
        let value = match completion {
            Completion::Return(value) => value,
            Completion::Throw(reason) => {
                return Ok(match self.phase {
                    Phase::Promise { state, done }
                        if state.kind != GeneratorResumeKind::Return && !done =>
                    {
                        FromSyncStep::Close {
                            iterator: state.iterator,
                            completion: Completion::Throw(reason),
                            resume: continuation(realm, Phase::Reject(state.capability)),
                        }
                    }
                    Phase::Method(state)
                    | Phase::Result(state)
                    | Phase::Done { state, .. }
                    | Phase::Value { state, .. }
                    | Phase::Promise { state, .. } => {
                        settle(realm, state.capability, Completion::Throw(reason))
                    }
                    Phase::MissingThrow(capability) | Phase::Reject(capability) => {
                        settle(realm, capability, Completion::Throw(reason))
                    }
                    Phase::Identity | Phase::Settled(_) => unreachable!(),
                });
            }
        };
        match self.phase {
            Phase::Method(state) => {
                if matches!(value, Value::Undefined | Value::Null) {
                    return Ok(match state.kind {
                        GeneratorResumeKind::Return => {
                            let value = state
                                .arguments
                                .into_iter()
                                .next()
                                .unwrap_or(Value::Undefined);
                            let result = runtime.new_iterator_result(realm, value, true)?;
                            settle(
                                realm,
                                state.capability,
                                Completion::Return(Value::Object(result)),
                            )
                        }
                        GeneratorResumeKind::Throw => FromSyncStep::Close {
                            iterator: state.iterator,
                            completion: Completion::Return(Value::Undefined),
                            resume: continuation(realm, Phase::MissingThrow(state.capability)),
                        },
                        GeneratorResumeKind::Next => {
                            let reason = runtime.new_native_error(
                                realm,
                                NativeErrorKind::Type,
                                "not a function",
                            )?;
                            settle(realm, state.capability, Completion::Throw(reason))
                        }
                    });
                }
                let callable =
                    match runtime.async_from_sync_callable(realm, value, "not a function")? {
                        NativeConversion::Value(callable) => callable,
                        NativeConversion::Throw(reason) => {
                            return Ok(settle(realm, state.capability, Completion::Throw(reason)));
                        }
                    };
                Ok(FromSyncStep::Call {
                    callable,
                    receiver: Value::Object(state.iterator.clone()),
                    arguments: state.arguments.clone(),
                    resume: continuation(realm, Phase::Result(state)),
                })
            }
            Phase::Result(state) => {
                let Value::Object(result) = value else {
                    let reason = runtime.new_native_error(
                        realm,
                        NativeErrorKind::Type,
                        "iterator must return an object",
                    )?;
                    return Ok(settle(realm, state.capability, Completion::Throw(reason)));
                };
                Ok(FromSyncStep::Read {
                    receiver: Value::Object(result.clone()),
                    key: runtime.intern_property_key("done")?,
                    resume: continuation(realm, Phase::Done { state, result }),
                })
            }
            Phase::Done { state, result } => {
                let done = runtime.value_to_boolean(&value)?;
                Ok(FromSyncStep::Read {
                    receiver: Value::Object(result),
                    key: runtime.intern_property_key("value")?,
                    resume: continuation(realm, Phase::Value { state, done }),
                })
            }
            Phase::Value { state, done } => Ok(FromSyncStep::Resolve {
                value,
                realm,
                resume: continuation(realm, Phase::Promise { state, done }),
            }),
            Phase::Promise { state, done } => {
                let Value::Object(promise) = value else {
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
                Ok(FromSyncStep::Complete(Completion::Return(Value::Object(
                    state.capability.promise,
                ))))
            }
            Phase::MissingThrow(capability) => {
                let reason = runtime.new_native_error(
                    realm,
                    NativeErrorKind::Type,
                    "throw is not a method",
                )?;
                Ok(settle(realm, capability, Completion::Throw(reason)))
            }
            Phase::Reject(_) | Phase::Identity | Phase::Settled(_) => {
                Err(RuntimeError::Invariant("Async-from-Sync unexpected reply"))
            }
        }
    }
}
