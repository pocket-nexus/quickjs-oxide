//! Eager iterator consumers share owned iteration, callback and close phases.
use super::{
    ObjectIteratorStep,
    step::{CloseStep, NextStep, finish_close, finish_next},
};
use crate::engine::{
    api::{
        Error, ErrorKind, error::NativeErrorKind, runtime::Runtime, runtime_error::RuntimeError,
    },
    builtins::native::NativeFunctionId,
    heap::{ContextId, IteratorConsumerKind},
    object::{CallableRef, ObjectRef, PropertyKey},
    value::{Value, conversion::NativeConversion},
    vm::{
        Completion,
        call::{NativeArguments, NativeInvocation},
    },
};
#[derive(Clone, Copy)]
pub(crate) enum ConsumeKind {
    Predicate(IteratorConsumerKind),
    Reduce,
    Array,
}
impl ConsumeKind {
    #[cfg(feature = "stack-vm")]
    pub(crate) fn for_target(target: NativeFunctionId) -> Option<Self> {
        match target {
            NativeFunctionId::IteratorPrototypeConsume(kind) => Some(Self::Predicate(kind)),
            NativeFunctionId::IteratorPrototypeReduce => Some(Self::Reduce),
            NativeFunctionId::IteratorPrototypeToArray => Some(Self::Array),
            _ => None,
        }
    }
}
pub(crate) enum ConsumeStep {
    Complete(Completion),
    Read {
        object: ObjectRef,
        key: PropertyKey,
        resume: ConsumeResume,
    },
    Next {
        iterator: ObjectRef,
        method: Value,
        resume: ConsumeResume,
    },
    Call {
        callable: CallableRef,
        arguments: Vec<Value>,
        resume: ConsumeResume,
    },
    Close {
        iterator: ObjectRef,
        completion: Completion,
    },
}
pub(crate) struct ConsumeResume {
    realm: ContextId,
    kind: ConsumeKind,
    source: ObjectRef,
    next: Value,
    callback: Option<CallableRef>,
    accumulator: Option<Value>,
    array: Option<ObjectRef>,
    index: i64,
    phase: Phase,
}
enum Phase {
    Method,
    Next,
    Callback(Value),
}
impl ConsumeStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        kind: ConsumeKind,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Self, RuntimeError> {
        let source = match runtime.iterator_receiver(realm, invocation.clone())? {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => return Ok(Self::Complete(Completion::Throw(value))),
        };
        let callback = if matches!(kind, ConsumeKind::Array) {
            None
        } else {
            let value = arguments
                .readable
                .first()
                .cloned()
                .ok_or(RuntimeError::Invariant(
                    "Iterator consumer callback was not padded",
                ))?;
            match runtime.iterator_callable_value(realm, value)? {
                NativeConversion::Value(callback) => Some(callback),
                NativeConversion::Throw(value) => {
                    return Ok(Self::Close {
                        iterator: source,
                        completion: Completion::Throw(value),
                    });
                }
            }
        };
        let accumulator = if matches!(kind, ConsumeKind::Reduce) && arguments.actual_arg_count > 1 {
            Some(
                arguments
                    .readable
                    .get(1)
                    .cloned()
                    .ok_or(RuntimeError::Invariant(
                        "Iterator reduce initial value disappeared",
                    ))?,
            )
        } else {
            None
        };
        Ok(Self::Read {
            object: source.clone(),
            key: runtime.intern_property_key("next")?,
            resume: ConsumeResume {
                realm,
                kind,
                source,
                next: Value::Undefined,
                callback,
                accumulator,
                array: None,
                index: 0,
                phase: Phase::Method,
            },
        })
    }
}
impl ConsumeResume {
    fn close(self, completion: Completion) -> ConsumeStep {
        ConsumeStep::Close {
            iterator: self.source,
            completion,
        }
    }
    fn next_step(mut self) -> ConsumeStep {
        self.phase = Phase::Next;
        ConsumeStep::Next {
            iterator: self.source.clone(),
            method: self.next.clone(),
            resume: self,
        }
    }
    pub(crate) fn resume(
        mut self,
        runtime: &Runtime,
        reply: Completion,
    ) -> Result<ConsumeStep, RuntimeError> {
        let value = match reply {
            Completion::Return(value) => value,
            Completion::Throw(value) => {
                return Ok(
                    if matches!(self.phase, Phase::Callback(_))
                        || matches!(self.kind, ConsumeKind::Reduce)
                    {
                        self.close(Completion::Throw(value))
                    } else {
                        ConsumeStep::Complete(Completion::Throw(value))
                    },
                );
            }
        };
        match std::mem::replace(&mut self.phase, Phase::Next) {
            Phase::Method => {
                self.next = value;
                if matches!(self.kind, ConsumeKind::Array) {
                    self.array = Some(runtime.new_array(self.realm)?);
                }
                Ok(self.next_step())
            }
            Phase::Callback(item) => {
                self.index = self.index.wrapping_add(1);
                let early = match self.kind {
                    ConsumeKind::Predicate(IteratorConsumerKind::Every) => {
                        (!runtime.value_to_boolean(&value)?).then_some(Value::Bool(false))
                    }
                    ConsumeKind::Predicate(IteratorConsumerKind::Some) => runtime
                        .value_to_boolean(&value)?
                        .then_some(Value::Bool(true)),
                    ConsumeKind::Predicate(IteratorConsumerKind::Find) => {
                        runtime.value_to_boolean(&value)?.then_some(item)
                    }
                    ConsumeKind::Predicate(IteratorConsumerKind::ForEach) => None,
                    ConsumeKind::Reduce => {
                        self.accumulator = Some(value);
                        None
                    }
                    ConsumeKind::Array => {
                        return Err(RuntimeError::Invariant(
                            "Iterator.toArray received a callback reply",
                        ));
                    }
                };
                Ok(if let Some(value) = early {
                    self.close(Completion::Return(value))
                } else {
                    self.next_step()
                })
            }
            Phase::Next => Err(RuntimeError::Invariant(
                "Iterator consumer received a completion in step phase",
            )),
        }
    }
    pub(crate) fn next(
        mut self,
        runtime: &Runtime,
        reply: ObjectIteratorStep,
    ) -> Result<ConsumeStep, RuntimeError> {
        if !matches!(self.phase, Phase::Next) {
            return Err(RuntimeError::Invariant(
                "Iterator consumer next has wrong phase",
            ));
        }
        let item = match reply {
            ObjectIteratorStep::Throw(value) => {
                return Ok(ConsumeStep::Complete(Completion::Throw(value)));
            }
            ObjectIteratorStep::Done => {
                let value = match self.kind {
                    ConsumeKind::Predicate(IteratorConsumerKind::Every) => Value::Bool(true),
                    ConsumeKind::Predicate(IteratorConsumerKind::Some) => Value::Bool(false),
                    ConsumeKind::Predicate(
                        IteratorConsumerKind::Find | IteratorConsumerKind::ForEach,
                    ) => Value::Undefined,
                    ConsumeKind::Array => Value::Object(
                        self.array
                            .take()
                            .ok_or(RuntimeError::Invariant("Iterator.toArray result missing"))?,
                    ),
                    ConsumeKind::Reduce => match self.accumulator.take() {
                        Some(value) => value,
                        None => {
                            let error = runtime.new_native_error(
                                self.realm,
                                NativeErrorKind::Type,
                                "empty iterator",
                            )?;
                            return Ok(self.close(Completion::Throw(error)));
                        }
                    },
                };
                return Ok(ConsumeStep::Complete(Completion::Return(value)));
            }
            ObjectIteratorStep::Yield(value) => value,
        };
        if matches!(self.kind, ConsumeKind::Array) {
            // This unpublished fresh Array has no callback-capable definition;
            // CreateDataProperty deliberately bypasses inherited index setters.
            let array = self
                .array
                .as_ref()
                .ok_or(RuntimeError::Invariant("Iterator.toArray result missing"))?;
            if let Some(value) =
                runtime.create_array_data_property(self.realm, array, self.index as u32, item)?
            {
                return Ok(ConsumeStep::Complete(Completion::Throw(value)));
            }
            self.index = u32::try_from(self.index)
                .ok()
                .and_then(|index| index.checked_add(1))
                .map(i64::from)
                .ok_or_else(|| {
                    RuntimeError::Engine(Error::new(ErrorKind::Range, "invalid array length"))
                })?;
            return Ok(self.next_step());
        }
        if matches!(self.kind, ConsumeKind::Reduce) && self.accumulator.is_none() {
            self.accumulator = Some(item);
            self.index = 1;
            return Ok(self.next_step());
        }
        let callable = self.callback.clone().ok_or(RuntimeError::Invariant(
            "Iterator consumer callback missing",
        ))?;
        let arguments = match self.kind {
            ConsumeKind::Reduce => vec![
                self.accumulator.take().ok_or(RuntimeError::Invariant(
                    "Iterator reduce accumulator missing",
                ))?,
                item.clone(),
                Value::number(self.index as f64),
            ],
            _ => vec![item.clone(), Value::number(self.index as f64)],
        };
        self.phase = Phase::Callback(item);
        Ok(ConsumeStep::Call {
            callable,
            arguments,
            resume: self,
        })
    }
}
pub(crate) fn finish(
    runtime: &Runtime,
    realm: ContextId,
    mut step: ConsumeStep,
) -> Result<Completion, RuntimeError> {
    loop {
        step = match step {
            ConsumeStep::Complete(result) => return Ok(result),
            ConsumeStep::Close {
                iterator,
                completion,
            } => {
                return finish_close(
                    runtime,
                    realm,
                    CloseStep::start(runtime, realm, iterator, completion)?,
                );
            }
            ConsumeStep::Read {
                object,
                key,
                resume,
            } => resume.resume(
                runtime,
                runtime.get_property_in_realm(realm, &object, &key)?,
            )?,
            ConsumeStep::Call {
                callable,
                arguments,
                resume,
            } => resume.resume(
                runtime,
                runtime.call_internal(realm, &callable, Value::Undefined, &arguments)?,
            )?,
            ConsumeStep::Next {
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
        };
    }
}
