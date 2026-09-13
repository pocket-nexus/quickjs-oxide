//! Shared iterator result parsing and close policies, with owned callback replies.
use super::super::object::ObjectIteratorStep;
use crate::engine::{
    api::{error::NativeErrorKind, runtime::Runtime, runtime_error::RuntimeError},
    heap::ContextId,
    object::{CallableRef, ObjectRef, PropertyKey},
    value::Value,
    vm::{Completion, call::NativeInvokeOutcome},
};

pub(crate) enum NextStep {
    Complete(ObjectIteratorStep),
    Call {
        callable: CallableRef,
        iterator: ObjectRef,
        resume: NextResume,
    },
    Read {
        object: ObjectRef,
        key: PropertyKey,
        resume: NextResume,
    },
}
pub(crate) struct NextResume {
    realm: ContextId,
    phase: NextPhase,
}
enum NextPhase {
    Result,
    Done(ObjectRef),
    Value,
}
impl NextStep {
    pub(crate) fn parse_result(
        runtime: &Runtime,
        realm: ContextId,
        result: Completion,
    ) -> Result<Self, RuntimeError> {
        NextResume {
            realm,
            phase: NextPhase::Result,
        }
        .resume(runtime, result)
    }

    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        iterator: ObjectRef,
        method: Value,
    ) -> Result<Self, RuntimeError> {
        let callable = match method {
            Value::Object(ref object) => runtime.as_callable(object)?,
            _ => None,
        };
        let Some(callable) = callable else {
            return Ok(Self::Complete(ObjectIteratorStep::Throw(
                runtime.new_native_error(realm, NativeErrorKind::Type, "not a function")?,
            )));
        };
        Ok(Self::Call {
            callable,
            iterator,
            resume: NextResume {
                realm,
                phase: NextPhase::Result,
            },
        })
    }
}
impl NextResume {
    pub(crate) fn raw(
        self,
        runtime: &Runtime,
        result: NativeInvokeOutcome,
    ) -> Result<NextStep, RuntimeError> {
        if !matches!(self.phase, NextPhase::Result) {
            return Err(RuntimeError::Invariant(
                "raw iterator reply has the wrong phase",
            ));
        }
        match result {
            NativeInvokeOutcome::IteratorNextRaw { value, done } => {
                Ok(NextStep::Complete(if done {
                    ObjectIteratorStep::Done
                } else {
                    ObjectIteratorStep::Yield(value)
                }))
            }
            NativeInvokeOutcome::Completion(result) => self.resume(runtime, result),
        }
    }
    pub(crate) fn resume(
        self,
        runtime: &Runtime,
        result: Completion,
    ) -> Result<NextStep, RuntimeError> {
        let value = match result {
            Completion::Return(value) => value,
            Completion::Throw(value) => {
                return Ok(NextStep::Complete(ObjectIteratorStep::Throw(value)));
            }
        };
        let realm = self.realm;
        match self.phase {
            NextPhase::Result => {
                let Value::Object(object) = value else {
                    return Ok(NextStep::Complete(ObjectIteratorStep::Throw(
                        runtime.new_native_error(
                            realm,
                            NativeErrorKind::Type,
                            "iterator must return an object",
                        )?,
                    )));
                };
                Ok(NextStep::Read {
                    object: object.clone(),
                    key: runtime.intern_property_key("done")?,
                    resume: Self {
                        realm,
                        phase: NextPhase::Done(object),
                    },
                })
            }
            NextPhase::Done(object) => {
                if runtime.value_to_boolean(&value)? {
                    return Ok(NextStep::Complete(ObjectIteratorStep::Done));
                }
                Ok(NextStep::Read {
                    object,
                    key: runtime.intern_property_key("value")?,
                    resume: Self {
                        realm,
                        phase: NextPhase::Value,
                    },
                })
            }
            NextPhase::Value => Ok(NextStep::Complete(ObjectIteratorStep::Yield(value))),
        }
    }
}
pub(crate) fn finish_next(
    runtime: &Runtime,
    realm: ContextId,
    mut step: NextStep,
) -> Result<ObjectIteratorStep, RuntimeError> {
    loop {
        step = match step {
            NextStep::Complete(result) => return Ok(result),
            NextStep::Read {
                object,
                key,
                resume,
            } => resume.resume(
                runtime,
                runtime.get_property_in_realm(realm, &object, &key)?,
            )?,
            NextStep::Call {
                callable,
                iterator,
                resume,
            } => {
                let receiver = Value::Object(iterator);
                match runtime.try_call_native_iterator_next_raw(
                    realm,
                    &callable,
                    receiver.clone(),
                )? {
                    Some(result) => resume.raw(runtime, result)?,
                    None => resume.resume(
                        runtime,
                        runtime.call_internal(realm, &callable, receiver, &[])?,
                    )?,
                }
            }
        };
    }
}

pub(crate) enum CloseStep {
    Complete(Completion),
    Read {
        object: ObjectRef,
        key: PropertyKey,
        resume: CloseResume,
    },
    Call {
        callable: CallableRef,
        iterator: ObjectRef,
        resume: CloseResume,
    },
}
pub(crate) struct CloseResume {
    realm: ContextId,
    iterator: ObjectRef,
    completion: Completion,
    called: bool,
}
impl CloseStep {
    /// A pending Throw suppresses every JavaScript failure in close; a Return
    /// requires a callable return method and an object result.
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        iterator: ObjectRef,
        completion: Completion,
    ) -> Result<Self, RuntimeError> {
        Ok(Self::Read {
            object: iterator.clone(),
            key: runtime.intern_property_key("return")?,
            resume: CloseResume {
                realm,
                iterator,
                completion,
                called: false,
            },
        })
    }
}
impl CloseResume {
    pub(crate) fn resume(
        mut self,
        runtime: &Runtime,
        reply: Completion,
    ) -> Result<CloseStep, RuntimeError> {
        let preserving = matches!(self.completion, Completion::Throw(_));
        let value = match reply {
            Completion::Return(value) => value,
            Completion::Throw(value) => {
                return Ok(CloseStep::Complete(if preserving {
                    self.completion
                } else {
                    Completion::Throw(value)
                }));
            }
        };
        if self.called {
            return Ok(CloseStep::Complete(
                if preserving || matches!(value, Value::Object(_)) {
                    self.completion
                } else {
                    Completion::Throw(runtime.new_native_error(
                        self.realm,
                        NativeErrorKind::Type,
                        "not an object",
                    )?)
                },
            ));
        }
        if matches!(value, Value::Undefined | Value::Null) {
            return Ok(CloseStep::Complete(self.completion));
        }
        let callable = match value {
            Value::Object(ref object) => runtime.as_callable(object)?,
            _ => None,
        };
        let Some(callable) = callable else {
            return Ok(CloseStep::Complete(if preserving {
                self.completion
            } else {
                Completion::Throw(runtime.new_native_error(
                    self.realm,
                    NativeErrorKind::Type,
                    "not a function",
                )?)
            }));
        };
        self.called = true;
        Ok(CloseStep::Call {
            callable,
            iterator: self.iterator.clone(),
            resume: self,
        })
    }
}
pub(crate) fn finish_close(
    runtime: &Runtime,
    realm: ContextId,
    mut step: CloseStep,
) -> Result<Completion, RuntimeError> {
    loop {
        step = match step {
            CloseStep::Complete(result) => return Ok(result),
            CloseStep::Read {
                object,
                key,
                resume,
            } => resume.resume(
                runtime,
                runtime.get_property_in_realm(realm, &object, &key)?,
            )?,
            CloseStep::Call {
                callable,
                iterator,
                resume,
            } => resume.resume(
                runtime,
                runtime.call_internal(realm, &callable, Value::Object(iterator), &[])?,
            )?,
        };
    }
}
