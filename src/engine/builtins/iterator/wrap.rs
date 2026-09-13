//! Wrapped iterator next parses results; return forwards the original result object.
use super::{
    ObjectIteratorStep,
    step::{NextStep, finish_next},
};
use crate::engine::{
    api::{error::NativeErrorKind, runtime::Runtime, runtime_error::RuntimeError},
    heap::{ContextId, HeapError, IteratorResumeKind},
    object::{CallableRef, ObjectRef, PropertyKey},
    value::{Value, conversion::NativeConversion},
    vm::{Completion, call::NativeInvocation},
};
pub(crate) enum WrapStep {
    Complete(Completion),
    Read {
        receiver: Value,
        key: PropertyKey,
        resume: WrapResume,
    },
    Call {
        callable: CallableRef,
        receiver: Value,
        resume: WrapResume,
    },
    Next {
        iterator: ObjectRef,
        method: Value,
        resume: WrapResume,
    },
    Parse {
        result: Completion,
        resume: WrapResume,
    },
}
pub(crate) struct WrapResume {
    realm: ContextId,
    source: Value,
    phase: Phase,
}
enum Phase {
    ReturnMethod,
    ReturnResult,
    NextResult,
    Next,
}
impl WrapStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        mode: IteratorResumeKind,
        invocation: &NativeInvocation,
    ) -> Result<Self, RuntimeError> {
        let receiver = match runtime.iterator_receiver(realm, invocation.clone())? {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => return Ok(Self::Complete(Completion::Throw(value))),
        };
        let state = {
            runtime
                .0
                .state
                .borrow()
                .heap
                .iterator_wrap_state(receiver.object_id())
        };
        let (source, next) = match state {
            Ok(state) => state,
            Err(HeapError::Invariant(_)) => {
                return Ok(Self::Complete(Completion::Throw(
                    runtime.new_native_error(
                        realm,
                        NativeErrorKind::Type,
                        "not an Iterator Wrap",
                    )?,
                )));
            }
            Err(error) => return Err(error.into()),
        };
        let source = runtime.root_raw_value(&source)?;
        let resume = WrapResume {
            realm,
            source: source.clone(),
            phase: Phase::ReturnMethod,
        };
        match mode {
            IteratorResumeKind::Return => Ok(Self::Read {
                receiver: source,
                key: runtime.intern_property_key("return")?,
                resume,
            }),
            IteratorResumeKind::Next => {
                let method = runtime.root_raw_value(&next)?;
                if let Value::Object(iterator) = source {
                    return Ok(Self::Next {
                        iterator,
                        method,
                        resume: WrapResume {
                            phase: Phase::Next,
                            ..resume
                        },
                    });
                }
                let callable = match runtime.iterator_callable_value(realm, method)? {
                    NativeConversion::Value(value) => value,
                    NativeConversion::Throw(value) => {
                        return Ok(Self::Complete(Completion::Throw(value)));
                    }
                };
                Ok(Self::Call {
                    callable,
                    receiver: source,
                    resume: WrapResume {
                        phase: Phase::NextResult,
                        ..resume
                    },
                })
            }
        }
    }
}
impl WrapResume {
    pub(crate) fn resume(
        mut self,
        runtime: &Runtime,
        reply: Completion,
    ) -> Result<WrapStep, RuntimeError> {
        if matches!(self.phase, Phase::NextResult) {
            self.phase = Phase::Next;
            return Ok(WrapStep::Parse {
                result: reply,
                resume: self,
            });
        }
        let value = match reply {
            Completion::Return(value) => value,
            Completion::Throw(value) => return Ok(WrapStep::Complete(Completion::Throw(value))),
        };
        match self.phase {
            Phase::ReturnMethod => {
                if matches!(value, Value::Undefined | Value::Null) {
                    return Ok(WrapStep::Complete(Completion::Return(Value::Object(
                        runtime.new_iterator_result(self.realm, Value::Undefined, true)?,
                    ))));
                }
                let callable = match runtime.iterator_callable_value(self.realm, value)? {
                    NativeConversion::Value(value) => value,
                    NativeConversion::Throw(value) => {
                        return Ok(WrapStep::Complete(Completion::Throw(value)));
                    }
                };
                self.phase = Phase::ReturnResult;
                Ok(WrapStep::Call {
                    callable,
                    receiver: self.source.clone(),
                    resume: self,
                })
            }
            Phase::ReturnResult => Ok(WrapStep::Complete(if matches!(value, Value::Object(_)) {
                Completion::Return(value)
            } else {
                Completion::Throw(runtime.new_native_error(
                    self.realm,
                    NativeErrorKind::Type,
                    "iterator must return an object",
                )?)
            })),
            _ => Err(RuntimeError::Invariant(
                "Iterator Wrap completion phase mismatch",
            )),
        }
    }
    pub(crate) fn next(
        self,
        runtime: &Runtime,
        reply: ObjectIteratorStep,
    ) -> Result<WrapStep, RuntimeError> {
        if !matches!(self.phase, Phase::Next) {
            return Err(RuntimeError::Invariant("Iterator Wrap next phase mismatch"));
        }
        let (value, done) = match reply {
            ObjectIteratorStep::Throw(value) => {
                return Ok(WrapStep::Complete(Completion::Throw(value)));
            }
            ObjectIteratorStep::Yield(value) => (value, false),
            ObjectIteratorStep::Done => (Value::Undefined, true),
        };
        Ok(WrapStep::Complete(Completion::Return(Value::Object(
            runtime.new_iterator_result(self.realm, value, done)?,
        ))))
    }
}
pub(crate) fn finish(
    runtime: &Runtime,
    realm: ContextId,
    mut step: WrapStep,
) -> Result<Completion, RuntimeError> {
    loop {
        step = match step {
            WrapStep::Complete(result) => return Ok(result),
            WrapStep::Read {
                receiver,
                key,
                resume,
            } => resume.resume(
                runtime,
                runtime.get_value_property_in_realm(realm, receiver, &key)?,
            )?,
            WrapStep::Call {
                callable,
                receiver,
                resume,
            } => resume.resume(
                runtime,
                runtime.call_internal(realm, &callable, receiver, &[])?,
            )?,
            WrapStep::Next {
                iterator,
                method,
                resume,
            } => resume.next(
                runtime,
                finish_next(
                    runtime,
                    realm,
                    NextStep::start(runtime, realm, iterator, method)?,
                )?,
            )?,
            WrapStep::Parse { result, resume } => resume.next(
                runtime,
                finish_next(
                    runtime,
                    realm,
                    NextStep::parse_result(runtime, realm, result)?,
                )?,
            )?,
        };
    }
}
