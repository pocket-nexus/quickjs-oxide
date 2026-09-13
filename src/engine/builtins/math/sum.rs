//! sumPrecise retains the exact accumulator across iterator replies.
use super::SumPrecise;
use crate::engine::{
    api::{error::NativeErrorKind, runtime::Runtime, runtime_error::RuntimeError},
    builtins::{
        iterator::step::{CloseStep, NextStep, finish_close, finish_next},
        object::ObjectIteratorStep,
    },
    heap::ContextId,
    object::{CallableRef, ObjectRef, PropertyKey, WellKnownSymbol},
    value::Value,
    vm::{
        Completion,
        call::{NativeArguments, NativeInvocation},
    },
};
pub(crate) enum SumStep {
    Complete(Completion),
    Read {
        receiver: Value,
        key: PropertyKey,
        resume: SumResume,
    },
    Call {
        callable: CallableRef,
        receiver: Value,
        resume: SumResume,
    },
    Next {
        iterator: ObjectRef,
        next: Value,
        resume: SumResume,
    },
    Close {
        iterator: ObjectRef,
        completion: Completion,
    },
}
enum Phase {
    Method,
    Iterator,
    NextMethod,
    Next,
}
pub(crate) struct SumResume {
    realm: ContextId,
    phase: Phase,
    iterable: Value,
    iterator: Option<ObjectRef>,
    next: Value,
    sum: SumPrecise,
}
impl SumStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Self, RuntimeError> {
        if !matches!(invocation, NativeInvocation::Call { .. }) {
            return Err(RuntimeError::Invariant(
                "Math.sumPrecise requires generic invocation",
            ));
        }
        let iterable = arguments
            .readable
            .first()
            .cloned()
            .ok_or(RuntimeError::Invariant(
                "Math.sumPrecise argv was not padded",
            ))?;
        if matches!(iterable, Value::Null | Value::Undefined) {
            return Ok(Self::Complete(Completion::Throw(
                runtime.new_native_error(
                    realm,
                    NativeErrorKind::Type,
                    &format!(
                        "cannot read property 'Symbol.iterator' of {}",
                        if matches!(iterable, Value::Null) {
                            "null"
                        } else {
                            "undefined"
                        }
                    ),
                )?,
            )));
        }
        Ok(Self::Read {
            receiver: iterable.clone(),
            key: PropertyKey::from(runtime.well_known_symbol(WellKnownSymbol::Iterator)),
            resume: SumResume {
                realm,
                phase: Phase::Method,
                iterable,
                iterator: None,
                next: Value::Undefined,
                sum: SumPrecise::new(),
            },
        })
    }
}
impl SumResume {
    pub(crate) fn resume(
        mut self,
        runtime: &Runtime,
        result: Completion,
    ) -> Result<SumStep, RuntimeError> {
        let value = match result {
            Completion::Return(value) => value,
            Completion::Throw(value) => return Ok(SumStep::Complete(Completion::Throw(value))),
        };
        match self.phase {
            Phase::Method => {
                let callable = match value {
                    Value::Object(object) => runtime.as_callable(&object)?,
                    _ => None,
                };
                let Some(callable) = callable else {
                    return Ok(SumStep::Complete(Completion::Throw(
                        runtime.new_native_error(
                            self.realm,
                            NativeErrorKind::Type,
                            "value is not iterable",
                        )?,
                    )));
                };
                self.phase = Phase::Iterator;
                Ok(SumStep::Call {
                    callable,
                    receiver: self.iterable.clone(),
                    resume: self,
                })
            }
            Phase::Iterator => {
                let Value::Object(iterator) = value else {
                    return Ok(SumStep::Complete(Completion::Throw(
                        runtime.new_native_error(
                            self.realm,
                            NativeErrorKind::Type,
                            "not an object",
                        )?,
                    )));
                };
                self.iterable = Value::Undefined;
                self.iterator = Some(iterator.clone());
                self.phase = Phase::NextMethod;
                Ok(SumStep::Read {
                    receiver: Value::Object(iterator),
                    key: runtime.intern_property_key("next")?,
                    resume: self,
                })
            }
            Phase::NextMethod => {
                self.next = value;
                self.next()
            }
            _ => Err(RuntimeError::Invariant("Math sum value phase mismatch")),
        }
    }
    fn next(mut self) -> Result<SumStep, RuntimeError> {
        self.phase = Phase::Next;
        Ok(SumStep::Next {
            iterator: self
                .iterator
                .clone()
                .ok_or(RuntimeError::Invariant("Math sum iterator missing"))?,
            next: self.next.clone(),
            resume: self,
        })
    }
    pub(crate) fn item(
        mut self,
        runtime: &Runtime,
        result: ObjectIteratorStep,
    ) -> Result<SumStep, RuntimeError> {
        if !matches!(self.phase, Phase::Next) {
            return Err(RuntimeError::Invariant("Math sum iterator phase mismatch"));
        }
        let item = match result {
            ObjectIteratorStep::Yield(value) => value,
            ObjectIteratorStep::Done => {
                return Ok(SumStep::Complete(Completion::Return(Value::Float(
                    self.sum.result(),
                ))));
            }
            ObjectIteratorStep::Throw(value) => {
                return Ok(SumStep::Complete(Completion::Throw(value)));
            }
        };
        let number = match item {
            Value::Int(value) => f64::from(value),
            Value::Float(value) => value,
            _ => {
                return Ok(SumStep::Close {
                    iterator: self
                        .iterator
                        .ok_or(RuntimeError::Invariant("Math sum iterator missing"))?,
                    completion: Completion::Throw(runtime.new_native_error(
                        self.realm,
                        NativeErrorKind::Type,
                        "not a number",
                    )?),
                });
            }
        };
        self.sum.add(number);
        self.next()
    }
}
pub(crate) fn finish(
    runtime: &Runtime,
    realm: ContextId,
    mut step: SumStep,
) -> Result<Completion, RuntimeError> {
    loop {
        step = match step {
            SumStep::Complete(result) => return Ok(result),
            SumStep::Read {
                receiver,
                key,
                resume,
            } => resume.resume(
                runtime,
                runtime.get_value_property_in_realm(realm, receiver, &key)?,
            )?,
            SumStep::Call {
                callable,
                receiver,
                resume,
            } => resume.resume(
                runtime,
                runtime.call_internal(realm, &callable, receiver, &[])?,
            )?,
            SumStep::Next {
                iterator,
                next,
                resume,
            } => resume.item(
                runtime,
                finish_next(
                    runtime,
                    realm,
                    NextStep::start(runtime, realm, iterator, next)?,
                )?,
            )?,
            SumStep::Close {
                iterator,
                completion,
            } => {
                return finish_close(
                    runtime,
                    realm,
                    CloseStep::start(runtime, realm, iterator, completion)?,
                );
            }
        };
    }
}
