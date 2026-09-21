//! Aggregate and race loops retain iterator-close and capability ordering.
use super::{PromiseResume, PromiseStep, capability};
use crate::engine::api::{runtime::Runtime, runtime_error::RuntimeError};
use crate::engine::builtins::{native::PromiseNativeKind, promise::RootedPromiseCapability};
use crate::engine::heap::ContextId;
use crate::engine::object::{CallableRef, ObjectRef, PropertyKey};
use crate::engine::value::{JsValue, conversion::NativeConversion};
use crate::engine::vm::{
    Completion,
    call::{NativeArguments, NativeInvocation},
};
use crate::engine::{
    api::error::NativeErrorKind, builtins::object::ObjectIteratorStep, object::WellKnownSymbol,
};
use std::{cell::Cell, rc::Rc};

pub(super) enum Phase {
    Resolve(Acquire),
    Method {
        state: Acquire,
        resolve: CallableRef,
    },
    Iterator {
        state: Acquire,
        resolve: CallableRef,
    },
    NextMethod {
        state: Acquire,
        resolve: CallableRef,
        iterator: ObjectRef,
    },
    Next(Box<Loop>),
    Resolved(Box<Loop>),
    Then(Box<Loop>),
    Terminal(RootedPromiseCapability),
    Closed(RootedPromiseCapability),
}
/// Aggregate acquisition and loop operands stay owned across every observable effect.
pub(super) struct Inputs {
    runtime: Runtime,
    iterable: JsValue,
    method: JsValue,
    reply: JsValue,
}
impl Inputs {
    fn new(runtime: &Runtime, iterable: JsValue) -> Self {
        Self {
            runtime: runtime.clone(),
            iterable,
            method: JsValue::Undefined,
            reply: JsValue::Undefined,
        }
    }
    fn take_reply(&mut self) -> JsValue {
        std::mem::replace(&mut self.reply, JsValue::Undefined)
    }
}
impl Drop for Inputs {
    fn drop(&mut self) {
        for value in [&mut self.iterable, &mut self.method, &mut self.reply] {
            let _ = self
                .runtime
                .release_jsvalue(std::mem::replace(value, JsValue::Undefined));
        }
    }
}
pub(super) struct Acquire {
    constructor: ObjectRef,
    inputs: Inputs,
    kind: PromiseNativeKind,
    capability: RootedPromiseCapability,
}
pub(super) struct Loop {
    constructor: ObjectRef,
    kind: PromiseNativeKind,
    capability: RootedPromiseCapability,
    resolve: CallableRef,
    iterator: ObjectRef,
    inputs: Inputs,
    aggregate: Option<Elements>,
}
struct Elements {
    values: ObjectRef,
    remaining: Rc<Cell<i32>>,
    index: u32,
}
fn continuation(runtime: &Runtime, realm: ContextId, phase: Phase) -> Box<PromiseResume> {
    Box::new(PromiseResume {
        runtime: runtime.clone(),
        pending_effect: super::PromiseStepPending::default(),
        realm,
        phase: super::Phase::Aggregate(phase),
    })
}
fn reject(
    runtime: &Runtime,
    realm: ContextId,
    capability: RootedPromiseCapability,
    reason: JsValue,
) -> Result<PromiseStep, RuntimeError> {
    super::convenience::settle(runtime, realm, capability, Completion::Throw(reason))
}
impl PromiseStep {
    pub(in crate::engine::builtins::promise) fn aggregate(
        runtime: &Runtime,
        realm: ContextId,
        kind: PromiseNativeKind,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Self, RuntimeError> {
        if !matches!(
            kind,
            PromiseNativeKind::All
                | PromiseNativeKind::AllSettled
                | PromiseNativeKind::Any
                | PromiseNativeKind::Race
        ) {
            return Err(RuntimeError::Invariant(
                "Promise aggregate received non-aggregate selector",
            ));
        }
        let NativeInvocation::Call { this_value } = invocation else {
            return Err(RuntimeError::Invariant(
                "Promise aggregate received constructor invocation",
            ));
        };
        let JsValue::Object(object_id) = this_value else {
            return capability::error(runtime, realm, "not an object");
        };
        let object = ObjectRef::from_borrowed_handle(runtime.clone(), *object_id)?;
        let constructor = match runtime
            .constructor_from_jsvalue(realm, JsValue::Object(object.clone().into_handle()))?
        {
            NativeConversion::Throw(value) => {
                return Ok(Self::Complete(Completion::Throw(value)));
            }
            NativeConversion::Value(constructor) => constructor,
        };
        let iterable = runtime.dup_jsvalue(arguments.readable.first().ok_or(
            RuntimeError::Invariant("Promise aggregate iterable argv was not padded"),
        )?)?;
        Box::new(PromiseResume {
            runtime: runtime.clone(),
            pending_effect: super::PromiseStepPending::default(),
            realm,
            phase: super::Phase::AggregateCapability {
                constructor: object.clone(),
                inputs: Inputs::new(runtime, iterable),
                kind,
            },
        })
        .capability(runtime, Some(constructor))
    }
}
pub(super) fn ready(
    runtime: &Runtime,
    realm: ContextId,
    constructor: ObjectRef,
    inputs: Inputs,
    kind: PromiseNativeKind,
    capability: RootedPromiseCapability,
) -> Result<PromiseStep, RuntimeError> {
    Ok({
        let __pending_field_key =
            runtime.pinned_property_key(crate::engine::atom::pinned::PinnedAtom::Resolve)?;
        let __pending_field_receiver = JsValue::Object(constructor.clone().into_handle());
        let __pending_field_resume = continuation(
            runtime,
            realm,
            Phase::Resolve(Acquire {
                constructor,
                inputs,
                kind,
                capability,
            }),
        );
        PromiseStep::request_read(
            __pending_field_receiver,
            __pending_field_key,
            __pending_field_resume,
        )
    })
}
pub(super) fn resume(
    runtime: &Runtime,
    realm: ContextId,
    phase: Phase,
    completion: Completion,
) -> Result<PromiseStep, RuntimeError> {
    // Iterator acquisition errors reject without close; only the body closes.
    let value = match completion {
        Completion::Return(value) => value,
        Completion::Throw(reason) => {
            return match phase {
                Phase::Resolve(state)
                | Phase::Method { state, .. }
                | Phase::Iterator { state, .. }
                | Phase::NextMethod { state, .. } => {
                    reject(runtime, realm, state.capability, reason)
                }
                Phase::Resolved(state) | Phase::Then(state) => state.close(realm, reason),
                Phase::Terminal(capability) | Phase::Closed(capability) => {
                    reject(runtime, realm, capability, reason)
                }
                Phase::Next(_) => {
                    runtime.release_jsvalue(reason)?;
                    Err(RuntimeError::Invariant(
                        "Promise next expected iterator reply",
                    ))
                }
            };
        }
    };
    match phase {
        Phase::Resolve(state) => {
            let conversion = runtime.promise_callable(realm, &value);
            runtime.release_jsvalue(value)?;
            match conversion? {
                NativeConversion::Throw(reason) => reject(runtime, realm, state.capability, reason),
                NativeConversion::Value(resolve) => Ok({
                    let __pending_field_receiver = runtime.dup_jsvalue(&state.inputs.iterable)?;
                    let __pending_field_key =
                        PropertyKey::from(runtime.well_known_symbol(WellKnownSymbol::Iterator));
                    let __pending_field_resume =
                        continuation(runtime, realm, Phase::Method { state, resolve });
                    PromiseStep::request_read(
                        __pending_field_receiver,
                        __pending_field_key,
                        __pending_field_resume,
                    )
                }),
            }
        }
        Phase::Method { state, resolve } => {
            let conversion = runtime.promise_callable(realm, &value);
            runtime.release_jsvalue(value)?;
            match conversion? {
                NativeConversion::Throw(discarded) => {
                    runtime.release_jsvalue(discarded)?;
                    let reason = runtime.new_native_error_jsvalue(
                        realm,
                        NativeErrorKind::Type,
                        "value is not iterable",
                    )?;
                    reject(runtime, realm, state.capability, reason)
                }
                NativeConversion::Value(callable) => Ok({
                    let __pending_field_callable = callable;
                    let __pending_field_receiver = runtime.dup_jsvalue(&state.inputs.iterable)?;
                    let __pending_field_arguments = Vec::new();
                    let __pending_field_resume =
                        continuation(runtime, realm, Phase::Iterator { state, resolve });
                    PromiseStep::request_call(
                        __pending_field_callable,
                        __pending_field_receiver,
                        __pending_field_arguments,
                        __pending_field_resume,
                    )
                }),
            }
        }
        Phase::Iterator { state, resolve } => match value {
            JsValue::Object(iterator_id) => {
                let iterator = ObjectRef::from_owned_handle(runtime.clone(), iterator_id);
                Ok({
                    let __pending_field_key = runtime
                        .pinned_property_key(crate::engine::atom::pinned::PinnedAtom::Next)?;
                    let __pending_field_receiver = JsValue::Object(iterator.clone().into_handle());
                    let __pending_field_resume = continuation(
                        runtime,
                        realm,
                        Phase::NextMethod {
                            state,
                            resolve,
                            iterator,
                        },
                    );
                    PromiseStep::request_read(
                        __pending_field_receiver,
                        __pending_field_key,
                        __pending_field_resume,
                    )
                })
            }
            other => {
                runtime.release_jsvalue(other)?;
                let reason = runtime.new_native_error_jsvalue(
                    realm,
                    NativeErrorKind::Type,
                    "not an object",
                )?;
                reject(runtime, realm, state.capability, reason)
            }
        },
        Phase::NextMethod {
            mut state,
            resolve,
            iterator,
        } => {
            state.inputs.method = value;
            let aggregate = if state.kind == PromiseNativeKind::Race {
                None
            } else {
                Some(Elements {
                    values: runtime.new_array(realm)?,
                    remaining: Rc::new(Cell::new(1)),
                    index: 0,
                })
            };
            runtime.release_jsvalue(std::mem::replace(
                &mut state.inputs.iterable,
                JsValue::Undefined,
            ))?;
            Ok(Box::new(Loop {
                constructor: state.constructor,
                kind: state.kind,
                capability: state.capability,
                resolve,
                iterator,
                inputs: state.inputs,
                aggregate,
            })
            .advance(realm))
        }
        Phase::Resolved(mut state) => {
            state.inputs.reply = value;
            let arguments = if let Some(elements) = &state.aggregate {
                let handlers = match runtime.prepare_promise_aggregate_handlers(
                    realm,
                    state.kind,
                    &elements.values,
                    &state.capability,
                    &elements.remaining,
                    elements.index,
                )? {
                    NativeConversion::Value(handlers) => handlers,
                    NativeConversion::Throw(reason) => {
                        return state.close(realm, reason);
                    }
                };
                let Some(count) = elements.remaining.get().checked_add(1) else {
                    for handler in handlers {
                        runtime.release_jsvalue(handler)?;
                    }
                    return state.overflow(runtime, realm);
                };
                elements.remaining.set(count);
                Vec::from(handlers)
            } else {
                vec![
                    JsValue::Object(state.capability.resolve.as_object().clone().into_handle()),
                    JsValue::Object(state.capability.reject.as_object().clone().into_handle()),
                ]
            };
            Ok({
                let __pending_field_step = Box::new(PromiseStep::invoke_then(
                    runtime,
                    realm,
                    state.inputs.take_reply(),
                    arguments,
                )?);
                let __pending_field_resume = continuation(runtime, realm, Phase::Then(state));
                PromiseStep::request_nested(__pending_field_step, __pending_field_resume)
            })
        }
        Phase::Then(mut state) => {
            runtime.release_jsvalue(value)?;
            if let Some(elements) = &mut state.aggregate {
                let Some(index) = elements
                    .index
                    .checked_add(1)
                    .filter(|index| *index != u32::MAX)
                else {
                    return state.overflow(runtime, realm);
                };
                elements.index = index;
            }
            Ok(state.advance(realm))
        }
        Phase::Terminal(capability) => {
            runtime.release_jsvalue(value)?;
            Ok(PromiseStep::Complete(Completion::Return(JsValue::Object(
                capability.promise.into_handle(),
            ))))
        }
        Phase::Closed(_) | Phase::Next(_) => {
            runtime.release_jsvalue(value)?;
            Err(RuntimeError::Invariant(
                "Promise aggregate unexpected reply",
            ))
        }
    }
}
impl Loop {
    fn advance(self: Box<Self>, realm: ContextId) -> PromiseStep {
        let runtime = self.constructor.runtime().clone();
        {
            let __pending_field_iterator = self.iterator.clone();
            let __pending_field_method = self
                .constructor
                .runtime()
                .dup_jsvalue(&self.inputs.method)
                .expect("aggregate next method must be a live edge");
            let __pending_field_resume = continuation(&runtime, realm, Phase::Next(self));
            PromiseStep::request_next(
                __pending_field_iterator,
                __pending_field_method,
                __pending_field_resume,
            )
        }
    }
    // Consume the existing suspended-loop box here, keeping its payload out of the reply transport.
    #[allow(clippy::boxed_local)]
    fn close(
        self: Box<Self>,
        realm: ContextId,
        reason: JsValue,
    ) -> Result<PromiseStep, RuntimeError> {
        let runtime = self.constructor.runtime().clone();
        Ok({
            let __pending_field_iterator = self.iterator;
            let __pending_field_completion = Completion::Throw(reason);
            let __pending_field_resume =
                continuation(&runtime, realm, Phase::Closed(self.capability));
            PromiseStep::request_close(
                __pending_field_iterator,
                __pending_field_completion,
                __pending_field_resume,
            )
        })
    }
    fn overflow(
        self: Box<Self>,
        runtime: &Runtime,
        realm: ContextId,
    ) -> Result<PromiseStep, RuntimeError> {
        let reason = runtime.new_native_error_jsvalue(
            realm,
            NativeErrorKind::Range,
            "too many Promise aggregate elements",
        )?;
        self.close(realm, reason)
    }
    pub(super) fn next(
        self: Box<Self>,
        runtime: &Runtime,
        realm: ContextId,
        result: ObjectIteratorStep,
    ) -> Result<PromiseStep, RuntimeError> {
        match result {
            ObjectIteratorStep::Throw(reason) => reject(runtime, realm, self.capability, reason),
            ObjectIteratorStep::Yield(value) => Ok({
                let __pending_field_callable = self.resolve.clone();
                let __pending_field_receiver =
                    JsValue::Object(self.constructor.clone().into_handle());
                let __pending_field_arguments = vec![value];
                let __pending_field_resume = continuation(runtime, realm, Phase::Resolved(self));
                PromiseStep::request_call(
                    __pending_field_callable,
                    __pending_field_receiver,
                    __pending_field_arguments,
                    __pending_field_resume,
                )
            }),
            ObjectIteratorStep::Done => {
                if let Some(elements) = &self.aggregate {
                    let count =
                        elements
                            .remaining
                            .get()
                            .checked_sub(1)
                            .ok_or(RuntimeError::Invariant(
                                "Promise aggregate remaining-elements counter underflowed",
                            ))?;
                    elements.remaining.set(count);
                    if count == 0 {
                        let (callable, value) = if self.kind == PromiseNativeKind::Any {
                            (
                                self.capability.reject.clone(),
                                JsValue::Object(
                                    runtime
                                        .new_internal_aggregate_error(
                                            realm,
                                            elements.values.clone(),
                                        )?
                                        .into_handle(),
                                ),
                            )
                        } else {
                            (
                                self.capability.resolve.clone(),
                                JsValue::Object(elements.values.clone().into_handle()),
                            )
                        };
                        return Ok({
                            let __pending_field_callable = callable;
                            let __pending_field_receiver = JsValue::Undefined;
                            let __pending_field_arguments = vec![value];
                            let __pending_field_resume =
                                continuation(runtime, realm, Phase::Terminal(self.capability));
                            PromiseStep::request_call(
                                __pending_field_callable,
                                __pending_field_receiver,
                                __pending_field_arguments,
                                __pending_field_resume,
                            )
                        });
                    }
                }
                Ok(PromiseStep::Complete(Completion::Return(JsValue::Object(
                    self.capability.promise.into_handle(),
                ))))
            }
        }
    }
}
