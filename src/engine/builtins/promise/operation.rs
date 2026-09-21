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
use crate::engine::value::{JsValue, Value, conversion::NativeConversion};
use crate::engine::vm::{
    Completion,
    call::{ConstructorPrototypeSource, NativeArguments, NativeInvocation},
};

pub(crate) enum PromiseStep {
    Next { resume: Box<PromiseResume> },
    Close { resume: Box<PromiseResume> },
    Nested { resume: Box<PromiseResume> },
    Complete(Completion),
    Read { resume: Box<PromiseResume> },
    Call { resume: Box<PromiseResume> },
    Construct { resume: Box<PromiseResume> },
    Prototype { resume: Box<PromiseResume> },
}

pub(crate) struct PromiseResume {
    pending_effect: PromiseStepPending,
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
        handlers: then::ThenHandlers,
    },
    ThenSpecies {
        promise: ObjectRef,
        handlers: then::ThenHandlers,
    },
    ThenCapability {
        promise: ObjectRef,
        handlers: then::ThenHandlers,
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
        {
            let __pending_field_callable = callable;
            let __pending_field_receiver = JsValue::Undefined;
            let __pending_field_arguments = vec![match argument {
                Value::Object(object) => JsValue::Object(object.into_handle()),
                other => __pending_field_callable
                    .runtime()
                    .into_jsvalue(other)
                    .expect("Promise ignore-return argument conversion"),
            }];
            let __pending_field_resume = Box::new(PromiseResume {
                pending_effect: PromiseStepPending::default(),
                realm,
                phase: Phase::IgnoreReturn,
            });
            Self::request_call(
                __pending_field_callable,
                __pending_field_receiver,
                __pending_field_arguments,
                __pending_field_resume,
            )
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
                runtime.prepare_promise_all_resolve_element(realm, invocation, arguments)
            }
            NativeFunctionId::PromiseAllSettledElement(kind) => {
                runtime.prepare_promise_all_settled_element(kind, realm, invocation, arguments)
            }
            NativeFunctionId::PromiseAnyRejectElement => {
                runtime.prepare_promise_any_reject_element(realm, invocation, arguments)
            }
            NativeFunctionId::Promise(PromiseNativeKind::Finally)
            | NativeFunctionId::PromiseFinallyHandler(_) => {
                finally::start(runtime, realm, target, invocation, arguments)
            }
            NativeFunctionId::PromiseFinallyThunk(kind) => runtime
                .call_promise_finally_thunk(kind, invocation.dup(runtime)?)
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
                let handler = runtime.root_value(arguments.readable.first().ok_or(
                    RuntimeError::Invariant("Promise.catch reject argv was not padded"),
                )?)?;
                Ok({
                    let __pending_field_receiver = runtime.dup_jsvalue(this_value)?;
                    let __pending_field_key = runtime
                        .pinned_property_key(crate::engine::atom::pinned::PinnedAtom::Then)?;
                    let __pending_field_resume = Box::new(PromiseResume {
                        pending_effect: PromiseStepPending::default(),
                        realm,
                        phase: Phase::CatchThen {
                            receiver: runtime.root_value(this_value)?,
                            handler,
                        },
                    });
                    Self::request_read(
                        __pending_field_receiver,
                        __pending_field_key,
                        __pending_field_resume,
                    )
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
                    runtime.root_value(this_value)?,
                    runtime.root_value(arguments.readable.first().ok_or(
                        RuntimeError::Invariant("Promise resolve/reject argv was not padded"),
                    )?)?,
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
                let executor = runtime.callable_from_value(runtime.root_value(
                    arguments.readable.first().ok_or(RuntimeError::Invariant(
                        "Promise executor argv was not padded",
                    ))?,
                )?)?;
                Ok({
                    let __pending_field_new_target = runtime.dup_jsvalue(new_target)?;
                    let __pending_field_resume = Box::new(PromiseResume {
                        pending_effect: PromiseStepPending::default(),
                        realm,
                        phase: Phase::ConstructorPrototype { executor },
                    });
                    Self::request_prototype(__pending_field_new_target, __pending_field_resume)
                })
            }
            NativeFunctionId::Promise(PromiseNativeKind::Species) => {
                runtime.call_promise_species(invocation).map(Self::Complete)
            }
            NativeFunctionId::PromiseCapabilityExecutor => runtime
                .call_promise_capability_executor(realm, invocation.dup(runtime)?, arguments)
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
            return Ok(Self::Complete(Completion::Return(JsValue::Undefined)));
        }
        let resolution = runtime.root_value(arguments.readable.first().ok_or(
            RuntimeError::Invariant("Promise resolving argv was not padded"),
        )?)?;
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
                return Ok({
                    let __pending_field_receiver = JsValue::Object(object.clone().into_handle());
                    let __pending_field_key = runtime
                        .pinned_property_key(crate::engine::atom::pinned::PinnedAtom::Then)?;
                    let __pending_field_resume = Box::new(PromiseResume {
                        pending_effect: PromiseStepPending::default(),
                        realm,
                        phase: Phase::ResolveThen {
                            promise,
                            resolution: object,
                        },
                    });
                    Self::request_read(
                        __pending_field_receiver,
                        __pending_field_key,
                        __pending_field_resume,
                    )
                });
            }
        } else {
            runtime.settle_promise(realm, &promise, PromiseState::Fulfilled, resolution)?;
        }
        Ok(Self::Complete(Completion::Return(JsValue::Undefined)))
    }

    pub(crate) fn finish(
        self,
        runtime: &Runtime,
        realm: ContextId,
    ) -> Result<Completion, RuntimeError> {
        {
            crate::engine::vm::execute_root(
                runtime.clone(),
                realm,
                crate::engine::vm::RootOperation::Promise(self),
            )
            .map_err(RuntimeError::Engine)
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
                return Ok(PromiseStep::Complete(Completion::Throw(
                    runtime.into_jsvalue(value)?,
                )));
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
            JsValue::Object(resolve.as_object().clone().into_handle()),
            JsValue::Object(reject.as_object().clone().into_handle()),
        ];
        Ok({
            let __pending_field_callable = executor;
            let __pending_field_receiver = JsValue::Undefined;
            let __pending_field_arguments = arguments;
            let __pending_field_resume = Box::new(Self {
                pending_effect: PromiseStepPending::default(),
                realm: self.realm,
                phase: Phase::ConstructorExecutor {
                    capability: RootedPromiseCapability {
                        promise,
                        resolve,
                        reject,
                    },
                },
            });
            PromiseStep::request_call(
                __pending_field_callable,
                __pending_field_receiver,
                __pending_field_arguments,
                __pending_field_resume,
            )
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
            Phase::TryCallback(capability) => {
                convenience::settle(runtime, realm, capability, completion)
            }
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
                let callable = runtime.promise_callable(realm, &value);
                runtime.release_jsvalue(value)?;
                match callable? {
                    NativeConversion::Throw(value) => Ok(PromiseStep::Complete(Completion::Throw(
                        runtime.into_jsvalue(value)?,
                    ))),
                    NativeConversion::Value(callable) => Ok({
                        let __pending_field_callable = callable;
                        let __pending_field_receiver = runtime.into_jsvalue(receiver)?;
                        let __pending_field_arguments = arguments
                            .into_iter()
                            .map(|value| runtime.into_jsvalue(value))
                            .collect::<Result<Vec<_>, _>>()?;
                        let __pending_field_resume = Box::new(Self {
                            pending_effect: PromiseStepPending::default(),
                            realm,
                            phase: Phase::Identity,
                        });
                        PromiseStep::request_call(
                            __pending_field_callable,
                            __pending_field_receiver,
                            __pending_field_arguments,
                            __pending_field_resume,
                        )
                    }),
                }
            }
            Phase::AggregateCapability { .. } => Err(RuntimeError::Invariant(
                "Promise aggregate expected capability",
            )),
            Phase::Aggregate(phase) => aggregate::resume(runtime, realm, phase, completion),
            Phase::IgnoreReturn => Ok(PromiseStep::Complete(match completion {
                Completion::Return(_) => Completion::Return(JsValue::Undefined),
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
                let callable = match &method {
                    JsValue::Object(id) => ObjectRef::from_borrowed_handle(runtime.clone(), *id)
                        .map_err(RuntimeError::from)
                        .and_then(|object| runtime.as_callable(&object)),
                    _ => Ok(None),
                };
                runtime.release_jsvalue(method)?;
                let Some(callable) = callable? else {
                    return capability::error(runtime, realm, "not a function");
                };
                Ok({
                    let __pending_field_callable = callable;
                    let __pending_field_receiver = runtime.into_jsvalue(receiver)?;
                    let __pending_field_arguments =
                        vec![JsValue::Undefined, runtime.into_jsvalue(handler)?];
                    let __pending_field_resume = Box::new(Self {
                        pending_effect: PromiseStepPending::default(),
                        realm,
                        phase: Phase::Identity,
                    });
                    PromiseStep::request_call(
                        __pending_field_callable,
                        __pending_field_receiver,
                        __pending_field_arguments,
                        __pending_field_resume,
                    )
                })
            }
            Phase::Thenable(reject) => match completion {
                Completion::Return(value) => Ok(PromiseStep::Complete(Completion::Return(value))),
                Completion::Throw(reason) => Ok({
                    let __pending_field_callable = reject;
                    let __pending_field_receiver = JsValue::Undefined;
                    let __pending_field_arguments = vec![reason];
                    let __pending_field_resume = Box::new(Self {
                        pending_effect: PromiseStepPending::default(),
                        realm,
                        phase: Phase::Identity,
                    });
                    PromiseStep::request_call(
                        __pending_field_callable,
                        __pending_field_receiver,
                        __pending_field_arguments,
                        __pending_field_resume,
                    )
                }),
            },
            Phase::Reaction(targets) => {
                let Some(targets) = targets else {
                    return Ok(PromiseStep::Complete(Completion::Return(
                        JsValue::Undefined,
                    )));
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
                Ok({
                    let __pending_field_callable = callable;
                    let __pending_field_receiver = JsValue::Undefined;
                    let __pending_field_arguments = vec![value];
                    let __pending_field_resume = Box::new(Self {
                        pending_effect: PromiseStepPending::default(),
                        realm,
                        phase: Phase::Identity,
                    });
                    PromiseStep::request_call(
                        __pending_field_callable,
                        __pending_field_receiver,
                        __pending_field_arguments,
                        __pending_field_resume,
                    )
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
                    Completion::Throw(reason) => runtime.settle_promise(
                        realm,
                        &promise,
                        PromiseState::Rejected,
                        runtime.root_and_release_jsvalue(reason)?,
                    )?,
                    Completion::Return(then) => {
                        let callable = match &then {
                            JsValue::Object(id) => {
                                ObjectRef::from_borrowed_handle(runtime.clone(), *id)
                                    .map_err(RuntimeError::from)
                                    .and_then(|object| runtime.as_callable(&object))
                            }
                            _ => Ok(None),
                        };
                        runtime.release_jsvalue(then)?;
                        if let Some(then) = callable? {
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
                Ok(PromiseStep::Complete(Completion::Return(
                    JsValue::Undefined,
                )))
            }
            Phase::ConstructorExecutor { capability } => {
                if let Completion::Throw(reason) = completion {
                    return Ok({
                        let __pending_field_callable = capability.reject;
                        let __pending_field_receiver = JsValue::Undefined;
                        let __pending_field_arguments = vec![reason];
                        let __pending_field_resume = Box::new(Self {
                            pending_effect: PromiseStepPending::default(),
                            realm,
                            phase: Phase::ReturnPromise(capability.promise),
                        });
                        PromiseStep::request_call(
                            __pending_field_callable,
                            __pending_field_receiver,
                            __pending_field_arguments,
                            __pending_field_resume,
                        )
                    });
                }
                Ok(PromiseStep::Complete(Completion::Return(
                    runtime.into_jsvalue(Value::Object(capability.promise))?,
                )))
            }
            Phase::ReturnPromise(promise) => Ok(PromiseStep::Complete(match completion {
                Completion::Throw(value) => Completion::Throw(value),
                Completion::Return(_) => {
                    Completion::Return(runtime.into_jsvalue(Value::Object(promise))?)
                }
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

#[derive(Default)]
struct PromiseStepPending {
    next_iterator: Option<ObjectRef>,
    next_method: Option<JsValue>,
    close_iterator: Option<ObjectRef>,
    close_completion: Option<Completion>,
    nested_step: Option<Box<PromiseStep>>,
    read_receiver: Option<JsValue>,
    read_key: Option<PropertyKey>,
    call_callable: Option<CallableRef>,
    call_receiver: Option<JsValue>,
    call_arguments: Option<Vec<JsValue>>,
    construct_target: Option<crate::engine::vm::call::ConstructorRef>,
    construct_arguments: Option<Vec<JsValue>>,
    prototype_new_target: Option<JsValue>,
}
impl PromiseStep {
    pub(crate) fn request_next(
        iterator: ObjectRef,
        method: JsValue,
        mut resume: Box<PromiseResume>,
    ) -> Self {
        resume.pending_effect.next_iterator = Some(iterator);
        resume.pending_effect.next_method = Some(method);
        Self::Next { resume }
    }
    pub(crate) fn request_close(
        iterator: ObjectRef,
        completion: Completion,
        mut resume: Box<PromiseResume>,
    ) -> Self {
        resume.pending_effect.close_iterator = Some(iterator);
        resume.pending_effect.close_completion = Some(completion);
        Self::Close { resume }
    }
    pub(crate) fn request_nested(step: Box<PromiseStep>, mut resume: Box<PromiseResume>) -> Self {
        resume.pending_effect.nested_step = Some(step);
        Self::Nested { resume }
    }
    pub(crate) fn request_read(
        receiver: JsValue,
        key: PropertyKey,
        mut resume: Box<PromiseResume>,
    ) -> Self {
        resume.pending_effect.read_receiver = Some(receiver);
        resume.pending_effect.read_key = Some(key);
        Self::Read { resume }
    }
    pub(crate) fn request_call(
        callable: CallableRef,
        receiver: JsValue,
        arguments: Vec<JsValue>,
        mut resume: Box<PromiseResume>,
    ) -> Self {
        resume.pending_effect.call_callable = Some(callable);
        resume.pending_effect.call_receiver = Some(receiver);
        resume.pending_effect.call_arguments = Some(arguments);
        Self::Call { resume }
    }
    pub(crate) fn request_construct(
        target: crate::engine::vm::call::ConstructorRef,
        arguments: Vec<JsValue>,
        mut resume: Box<PromiseResume>,
    ) -> Self {
        resume.pending_effect.construct_target = Some(target);
        resume.pending_effect.construct_arguments = Some(arguments);
        Self::Construct { resume }
    }
    pub(crate) fn request_prototype(new_target: JsValue, mut resume: Box<PromiseResume>) -> Self {
        resume.pending_effect.prototype_new_target = Some(new_target);
        Self::Prototype { resume }
    }
}
impl PromiseResume {
    pub(crate) fn take_next_iterator(&mut self) -> ObjectRef {
        self.pending_effect
            .next_iterator
            .take()
            .expect("PromiseStep Next iterator")
    }
    pub(crate) fn take_next_method(&mut self) -> JsValue {
        self.pending_effect
            .next_method
            .take()
            .expect("PromiseStep Next method")
    }
    pub(crate) fn take_close_iterator(&mut self) -> ObjectRef {
        self.pending_effect
            .close_iterator
            .take()
            .expect("PromiseStep Close iterator")
    }
    pub(crate) fn take_close_completion(&mut self) -> Completion {
        self.pending_effect
            .close_completion
            .take()
            .expect("PromiseStep Close completion")
    }
    pub(crate) fn take_nested_step(&mut self) -> Box<PromiseStep> {
        self.pending_effect
            .nested_step
            .take()
            .expect("PromiseStep Nested step")
    }
    pub(crate) fn take_read_receiver(&mut self) -> JsValue {
        self.pending_effect
            .read_receiver
            .take()
            .expect("PromiseStep Read receiver")
    }
    pub(crate) fn take_read_key(&mut self) -> PropertyKey {
        self.pending_effect
            .read_key
            .take()
            .expect("PromiseStep Read key")
    }
    pub(crate) fn take_call_callable(&mut self) -> CallableRef {
        self.pending_effect
            .call_callable
            .take()
            .expect("PromiseStep Call callable")
    }
    pub(crate) fn take_call_receiver(&mut self) -> JsValue {
        self.pending_effect
            .call_receiver
            .take()
            .expect("PromiseStep Call receiver")
    }
    pub(crate) fn take_call_arguments(&mut self) -> Vec<JsValue> {
        self.pending_effect
            .call_arguments
            .take()
            .expect("PromiseStep Call arguments")
    }
    pub(crate) fn take_construct_target(&mut self) -> crate::engine::vm::call::ConstructorRef {
        self.pending_effect
            .construct_target
            .take()
            .expect("PromiseStep Construct target")
    }
    pub(crate) fn take_construct_arguments(&mut self) -> Vec<JsValue> {
        self.pending_effect
            .construct_arguments
            .take()
            .expect("PromiseStep Construct arguments")
    }
    pub(crate) fn take_prototype_new_target(&mut self) -> JsValue {
        self.pending_effect
            .prototype_new_target
            .take()
            .expect("PromiseStep Prototype new_target")
    }
}
const _: () = assert!(std::mem::size_of::<PromiseStep>() <= 64);

// S11 all-domain protocol bound; inline completion stays allocation-free.
const _: () = assert!(std::mem::size_of::<PromiseStep>() <= 64);
