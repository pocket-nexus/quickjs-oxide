//! Promise algorithms expose observable reads and calls as owned replies.
use super::{RootedPromiseCapability, Runtime, RuntimeError};
mod aggregate;
mod capability;
mod convenience;
mod finally;
mod jobs;
mod resolve;
mod then;
use crate::engine::builtins::native::{NativeFunctionId, PromiseNativeKind, PromiseResolvingKind};
use crate::engine::heap::{ContextId, InternalCallableData, PromiseState};
use crate::engine::object::{CallableRef, ObjectRef, PropertyKey};
use crate::engine::value::{Value, conversion::NativeConversion};
use crate::engine::vm::{
    Completion,
    call::{ConstructorPrototypeSource, NativeArguments, NativeInvocation},
};

pub(crate) enum PromiseStep {
    Next {
        iterator: ObjectRef,
        method: Value,
        resume: Box<PromiseResume>,
    },
    Close {
        iterator: ObjectRef,
        completion: Completion,
        resume: Box<PromiseResume>,
    },
    Nested {
        step: Box<PromiseStep>,
        resume: Box<PromiseResume>,
    },
    Complete(Completion),
    Read {
        receiver: Value,
        key: PropertyKey,
        resume: Box<PromiseResume>,
    },
    Call {
        callable: CallableRef,
        receiver: Value,
        arguments: Vec<Value>,
        resume: Box<PromiseResume>,
    },
    Construct {
        target: crate::engine::vm::call::ConstructorRef,
        arguments: Vec<Value>,
        resume: Box<PromiseResume>,
    },
    Prototype {
        new_target: Value,
        resume: Box<PromiseResume>,
    },
}

pub(crate) struct PromiseResume {
    realm: ContextId,
    phase: Phase,
}

enum Phase {
    Identity,
    IgnoreReturn,
    AggregateCapability {
        constructor: ObjectRef,
        iterable: Value,
        kind: PromiseNativeKind,
    },
    Aggregate(aggregate::Phase),
    InvokeThen {
        receiver: Value,
        arguments: Vec<Value>,
    },
    Finally(finally::Phase),
    ConvenienceCapability {
        kind: PromiseNativeKind,
        arguments: NativeArguments,
    },
    TryCallback(RootedPromiseCapability),
    CatchThen {
        receiver: Value,
        handler: Value,
    },
    Thenable(CallableRef),
    Reaction(Option<jobs::ReactionTargets>),
    Capability {
        executor: CallableRef,
        after: Box<PromiseResume>,
    },
    StaticConstructor {
        constructor: ObjectRef,
        argument: Value,
        kind: PromiseNativeKind,
    },
    StaticCapability {
        argument: Value,
        kind: PromiseNativeKind,
    },
    ThenConstructor {
        promise: ObjectRef,
        handlers: [Value; 2],
    },
    ThenSpecies {
        promise: ObjectRef,
        handlers: [Value; 2],
    },
    ThenCapability {
        promise: ObjectRef,
        handlers: [Value; 2],
    },
    ResolveThen {
        promise: ObjectRef,
        resolution: ObjectRef,
    },
    ConstructorPrototype {
        executor: CallableRef,
    },
    ConstructorExecutor {
        capability: RootedPromiseCapability,
    },
    ReturnPromise(ObjectRef),
}

impl PromiseStep {
    pub(super) fn ignore_return(realm: ContextId, callable: CallableRef, argument: Value) -> Self {
        Self::Call {
            callable,
            receiver: Value::Undefined,
            arguments: vec![argument],
            resume: Box::new(PromiseResume {
                realm,
                phase: Phase::IgnoreReturn,
            }),
        }
    }
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        target: NativeFunctionId,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Self, RuntimeError> {
        match target {
            NativeFunctionId::Promise(
                kind @ (PromiseNativeKind::All
                | PromiseNativeKind::AllSettled
                | PromiseNativeKind::Any
                | PromiseNativeKind::Race),
            ) => Self::aggregate(runtime, realm, kind, invocation, arguments),
            NativeFunctionId::PromiseAllResolveElement => {
                runtime.prepare_promise_all_resolve_element(realm, invocation.clone(), arguments)
            }
            NativeFunctionId::PromiseAllSettledElement(kind) => runtime
                .prepare_promise_all_settled_element(kind, realm, invocation.clone(), arguments),
            NativeFunctionId::PromiseAnyRejectElement => {
                runtime.prepare_promise_any_reject_element(realm, invocation.clone(), arguments)
            }
            NativeFunctionId::Promise(PromiseNativeKind::Finally)
            | NativeFunctionId::PromiseFinallyHandler(_) => {
                finally::start(runtime, realm, target, invocation, arguments)
            }
            NativeFunctionId::PromiseFinallyThunk(kind) => runtime
                .call_promise_finally_thunk(kind, invocation.clone())
                .map(Self::Complete),
            NativeFunctionId::Promise(
                kind @ (PromiseNativeKind::Try | PromiseNativeKind::WithResolvers),
            ) => Self::convenience(runtime, realm, kind, invocation, arguments),
            NativeFunctionId::Promise(PromiseNativeKind::Catch) => {
                let NativeInvocation::Call { this_value } = invocation else {
                    return Err(RuntimeError::Invariant(
                        "Promise.prototype.catch received a constructor invocation",
                    ));
                };
                let handler =
                    arguments
                        .readable
                        .first()
                        .cloned()
                        .ok_or(RuntimeError::Invariant(
                            "Promise.catch reject argv was not padded",
                        ))?;
                Ok(Self::Read {
                    receiver: this_value.clone(),
                    key: runtime.intern_property_key("then")?,
                    resume: Box::new(PromiseResume {
                        realm,
                        phase: Phase::CatchThen {
                            receiver: this_value.clone(),
                            handler,
                        },
                    }),
                })
            }
            NativeFunctionId::Promise(PromiseNativeKind::Then) => {
                Self::then(runtime, realm, invocation, arguments)
            }
            NativeFunctionId::Promise(
                kind @ (PromiseNativeKind::Resolve | PromiseNativeKind::Reject),
            ) => {
                let NativeInvocation::Call { this_value } = invocation else {
                    return Err(RuntimeError::Invariant(
                        "Promise resolve/reject received a constructor invocation",
                    ));
                };
                Self::static_resolve(
                    runtime,
                    realm,
                    kind,
                    this_value.clone(),
                    arguments
                        .readable
                        .first()
                        .cloned()
                        .ok_or(RuntimeError::Invariant(
                            "Promise resolve/reject argv was not padded",
                        ))?,
                )
            }
            NativeFunctionId::PromiseResolving(kind) => {
                Self::resolving(runtime, realm, kind, invocation, arguments)
            }
            NativeFunctionId::Promise(PromiseNativeKind::Constructor) => {
                let NativeInvocation::Construct { new_target } = invocation else {
                    return Err(RuntimeError::Invariant(
                        "Promise constructor did not receive a constructor invocation",
                    ));
                };
                let executor =
                    runtime.callable_from_value(arguments.readable.first().cloned().ok_or(
                        RuntimeError::Invariant("Promise executor argv was not padded"),
                    )?)?;
                Ok(Self::Prototype {
                    new_target: new_target.clone(),
                    resume: Box::new(PromiseResume {
                        realm,
                        phase: Phase::ConstructorPrototype { executor },
                    }),
                })
            }
            NativeFunctionId::Promise(PromiseNativeKind::Species) => runtime
                .call_promise_species(invocation.clone())
                .map(Self::Complete),
            NativeFunctionId::PromiseCapabilityExecutor => runtime
                .call_promise_capability_executor(realm, invocation.clone(), arguments)
                .map(Self::Complete),
            _ => Err(RuntimeError::Invariant("unregistered Promise operation")),
        }
    }

    fn resolving(
        runtime: &Runtime,
        realm: ContextId,
        target_kind: PromiseResolvingKind,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Self, RuntimeError> {
        let NativeInvocation::Call { .. } = invocation else {
            return Err(RuntimeError::Invariant(
                "Promise resolving function received a constructor invocation",
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
                "Promise resolving function had no internal capture",
            ))?;
        let InternalCallableData::PromiseResolving {
            promise,
            already_resolved,
            kind,
        } = internal
        else {
            return Err(RuntimeError::Invariant(
                "Promise resolving function had the wrong internal capture",
            ));
        };
        if kind != target_kind {
            return Err(RuntimeError::Invariant(
                "Promise resolving target disagreed with its capture",
            ));
        }
        if already_resolved.replace(true) {
            return Ok(Self::Complete(Completion::Return(Value::Undefined)));
        }
        let resolution = arguments
            .readable
            .first()
            .cloned()
            .ok_or(RuntimeError::Invariant(
                "Promise resolving argv was not padded",
            ))?;
        let promise = ObjectRef::from_borrowed_handle(runtime.clone(), promise)?;
        if kind == PromiseResolvingKind::Reject {
            runtime.settle_promise(realm, &promise, PromiseState::Rejected, resolution)?;
        } else if let Value::Object(object) = resolution {
            if object == promise {
                let reason = runtime.new_native_error(
                    realm,
                    crate::engine::api::error::NativeErrorKind::Type,
                    "promise self resolution",
                )?;
                runtime.settle_promise(realm, &promise, PromiseState::Rejected, reason)?;
            } else {
                return Ok(Self::Read {
                    receiver: Value::Object(object.clone()),
                    key: runtime.intern_property_key("then")?,
                    resume: Box::new(PromiseResume {
                        realm,
                        phase: Phase::ResolveThen {
                            promise,
                            resolution: object,
                        },
                    }),
                });
            }
        } else {
            runtime.settle_promise(realm, &promise, PromiseState::Fulfilled, resolution)?;
        }
        Ok(Self::Complete(Completion::Return(Value::Undefined)))
    }

    pub(crate) fn finish(
        self,
        runtime: &Runtime,
        realm: ContextId,
    ) -> Result<Completion, RuntimeError> {
        #[cfg(feature = "stack-vm")]
        {
            return crate::engine::vm::execute_root(
                runtime.clone(),
                realm,
                crate::engine::vm::RootOperation::Promise(self),
            )
            .map_err(RuntimeError::Engine);
        }
        #[cfg(not(feature = "stack-vm"))]
        {
            self.finish_legacy(runtime, realm)
        }
    }

    #[cfg(not(feature = "stack-vm"))]
    fn finish_legacy(
        self,
        runtime: &Runtime,
        realm: ContextId,
    ) -> Result<Completion, RuntimeError> {
        let mut step = self;
        loop {
            step = match step {
                Self::Complete(completion) => return Ok(completion),
                Self::Next {
                    iterator,
                    method,
                    resume,
                } => resume.next(
                    runtime,
                    runtime.object_iterator_next(realm, &iterator, method)?,
                )?,
                Self::Close {
                    iterator,
                    completion,
                    resume,
                } => {
                    let step = crate::engine::builtins::iterator::step::CloseStep::start(
                        runtime, realm, iterator, completion,
                    )?;
                    resume.resume(
                        runtime,
                        crate::engine::builtins::iterator::step::finish_close(
                            runtime, realm, step,
                        )?,
                    )?
                }
                Self::Nested { step, resume } => {
                    resume.resume(runtime, step.finish_legacy(runtime, realm)?)?
                }
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
                Self::Construct {
                    target,
                    arguments,
                    resume,
                } => resume.resume(
                    runtime,
                    runtime.construct_constructor_internal(realm, &target, &target, &arguments)?,
                )?,
                Self::Prototype { new_target, resume } => {
                    let source = crate::engine::vm::call::prototype::finish(
                        runtime,
                        realm,
                        crate::engine::vm::call::prototype::ProtoSourceStep::start(
                            runtime, realm, new_target,
                        )?,
                    )?;
                    resume.prototype(runtime, source)?
                }
            };
        }
    }
}

impl PromiseResume {
    pub(crate) fn prototype(
        self: Box<Self>,
        runtime: &Runtime,
        result: NativeConversion<ConstructorPrototypeSource>,
    ) -> Result<PromiseStep, RuntimeError> {
        let Phase::ConstructorPrototype { executor } = self.phase else {
            return Err(RuntimeError::Invariant(
                "Promise prototype reply has wrong phase",
            ));
        };
        let prototype = match result {
            NativeConversion::Throw(value) => {
                return Ok(PromiseStep::Complete(Completion::Throw(value)));
            }
            NativeConversion::Value(ConstructorPrototypeSource::Explicit(object)) => object,
            NativeConversion::Value(ConstructorPrototypeSource::Realm(realm)) => {
                ObjectRef::from_borrowed_handle(
                    runtime.clone(),
                    runtime.promise_realm_data(realm)?.prototype,
                )?
            }
        };
        let promise = runtime.new_promise_object(&prototype)?;
        let (resolve, reject) = runtime.create_promise_resolving_functions(self.realm, &promise)?;
        let arguments = vec![
            Value::Object(resolve.as_object().clone()),
            Value::Object(reject.as_object().clone()),
        ];
        Ok(PromiseStep::Call {
            callable: executor,
            receiver: Value::Undefined,
            arguments,
            resume: Box::new(Self {
                realm: self.realm,
                phase: Phase::ConstructorExecutor {
                    capability: RootedPromiseCapability {
                        promise,
                        resolve,
                        reject,
                    },
                },
            }),
        })
    }

    pub(crate) fn resume(
        self: Box<Self>,
        runtime: &Runtime,
        completion: Completion,
    ) -> Result<PromiseStep, RuntimeError> {
        let realm = self.realm;
        match self.phase {
            Phase::ConvenienceCapability { .. } => Err(RuntimeError::Invariant(
                "Promise convenience expected capability",
            )),
            Phase::TryCallback(capability) => convenience::settle(realm, capability, completion),
            Phase::Finally(phase) => finally::resume(runtime, realm, phase, completion),
            Phase::InvokeThen {
                receiver,
                arguments,
            } => {
                let value = match completion {
                    Completion::Return(value) => value,
                    Completion::Throw(value) => {
                        return Ok(PromiseStep::Complete(Completion::Throw(value)));
                    }
                };
                match runtime.promise_callable(realm, value)? {
                    NativeConversion::Throw(value) => {
                        Ok(PromiseStep::Complete(Completion::Throw(value)))
                    }
                    NativeConversion::Value(callable) => Ok(PromiseStep::Call {
                        callable,
                        receiver,
                        arguments,
                        resume: Box::new(Self {
                            realm,
                            phase: Phase::Identity,
                        }),
                    }),
                }
            }
            Phase::AggregateCapability { .. } => Err(RuntimeError::Invariant(
                "Promise aggregate expected capability",
            )),
            Phase::Aggregate(phase) => aggregate::resume(runtime, realm, phase, completion),
            Phase::IgnoreReturn => Ok(PromiseStep::Complete(match completion {
                Completion::Return(_) => Completion::Return(Value::Undefined),
                other => other,
            })),
            Phase::Identity => Ok(PromiseStep::Complete(completion)),
            Phase::CatchThen { receiver, handler } => {
                let method = match completion {
                    Completion::Throw(value) => {
                        return Ok(PromiseStep::Complete(Completion::Throw(value)));
                    }
                    Completion::Return(value) => value,
                };
                let callable = if let Value::Object(object) = method {
                    runtime.as_callable(&object)?
                } else {
                    None
                };
                let Some(callable) = callable else {
                    return capability::error(runtime, realm, "not a function");
                };
                Ok(PromiseStep::Call {
                    callable,
                    receiver,
                    arguments: vec![Value::Undefined, handler],
                    resume: Box::new(Self {
                        realm,
                        phase: Phase::Identity,
                    }),
                })
            }
            Phase::Thenable(reject) => match completion {
                Completion::Return(value) => Ok(PromiseStep::Complete(Completion::Return(value))),
                Completion::Throw(reason) => Ok(PromiseStep::Call {
                    callable: reject,
                    receiver: Value::Undefined,
                    arguments: vec![reason],
                    resume: Box::new(Self {
                        realm,
                        phase: Phase::Identity,
                    }),
                }),
            },
            Phase::Reaction(targets) => {
                let Some(targets) = targets else {
                    return Ok(PromiseStep::Complete(Completion::Return(Value::Undefined)));
                };
                let (target, value) = match completion {
                    Completion::Return(value) => (targets.resolve, value),
                    Completion::Throw(value) => (targets.reject, value),
                };
                let callable = runtime
                    .as_callable(&target)?
                    .ok_or(RuntimeError::Invariant(
                        "Promise reaction capability was no longer callable",
                    ))?;
                Ok(PromiseStep::Call {
                    callable,
                    receiver: Value::Undefined,
                    arguments: vec![value],
                    resume: Box::new(Self {
                        realm,
                        phase: Phase::Identity,
                    }),
                })
            }
            Phase::Capability { executor, after } => {
                let capability = runtime.finish_promise_capability(realm, &executor, completion)?;
                after.capability_ready(runtime, capability)
            }
            Phase::ThenConstructor { promise, handlers } => {
                then::constructor(runtime, realm, promise, handlers, completion)
            }
            Phase::ThenSpecies { promise, handlers } => {
                then::species(runtime, realm, promise, handlers, completion)
            }
            Phase::StaticConstructor {
                constructor,
                argument,
                kind,
            } => resolve::constructor(runtime, realm, constructor, argument, kind, completion),
            Phase::ThenCapability { .. } | Phase::StaticCapability { .. } => Err(
                RuntimeError::Invariant("Promise operation expected a capability"),
            ),
            Phase::ResolveThen {
                promise,
                resolution,
            } => {
                match completion {
                    Completion::Throw(reason) => {
                        runtime.settle_promise(realm, &promise, PromiseState::Rejected, reason)?
                    }
                    Completion::Return(then) => {
                        let then = if let Value::Object(object) = then {
                            runtime.as_callable(&object)?
                        } else {
                            None
                        };
                        if let Some(then) = then {
                            runtime.enqueue_promise_resolve_thenable_job(
                                realm,
                                promise.object_id(),
                                resolution.object_id(),
                                then.as_object().object_id(),
                            )?;
                        } else {
                            runtime.settle_promise(
                                realm,
                                &promise,
                                PromiseState::Fulfilled,
                                Value::Object(resolution),
                            )?;
                        }
                    }
                }
                Ok(PromiseStep::Complete(Completion::Return(Value::Undefined)))
            }
            Phase::ConstructorExecutor { capability } => {
                if let Completion::Throw(reason) = completion {
                    return Ok(PromiseStep::Call {
                        callable: capability.reject,
                        receiver: Value::Undefined,
                        arguments: vec![reason],
                        resume: Box::new(Self {
                            realm,
                            phase: Phase::ReturnPromise(capability.promise),
                        }),
                    });
                }
                Ok(PromiseStep::Complete(Completion::Return(Value::Object(
                    capability.promise,
                ))))
            }
            Phase::ReturnPromise(promise) => Ok(PromiseStep::Complete(match completion {
                Completion::Throw(value) => Completion::Throw(value),
                Completion::Return(_) => Completion::Return(Value::Object(promise)),
            })),
            Phase::ConstructorPrototype { .. } => Err(RuntimeError::Invariant(
                "Promise constructor expected prototype source",
            )),
        }
    }
}

impl PromiseResume {
    pub(crate) fn next(
        self: Box<Self>,
        runtime: &Runtime,
        result: crate::engine::builtins::object::ObjectIteratorStep,
    ) -> Result<PromiseStep, RuntimeError> {
        let Phase::Aggregate(aggregate::Phase::Next(state)) = self.phase else {
            return Err(RuntimeError::Invariant(
                "Promise iterator reply has wrong phase",
            ));
        };
        state.next(runtime, self.realm, result)
    }
}
