//! Iterator.from acquires next before the ordinary instance check.
use crate::engine::{
    api::{error::NativeErrorKind, runtime::Runtime, runtime_error::RuntimeError},
    heap::ContextId,
    object::{CallableRef, ObjectRef, PropertyKey, WellKnownSymbol},
    value::{Value, conversion::NativeConversion},
    vm::{
        Completion,
        call::{NativeArguments, NativeInvocation},
    },
};
pub(crate) enum FromStep {
    Complete(Completion),
    Read {
        receiver: Value,
        key: PropertyKey,
        resume: FromResume,
    },
    Call {
        callable: CallableRef,
        receiver: Value,
        resume: FromResume,
    },
    Instance {
        constructor: CallableRef,
        value: Value,
        resume: FromResume,
    },
}
pub(crate) struct FromResume {
    realm: ContextId,
    phase: Phase,
}
enum Phase {
    Method(Value),
    Iterator,
    Next(Value),
    Instance { iterator: Value, next: Value },
}
impl FromStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Self, RuntimeError> {
        if !matches!(invocation, NativeInvocation::Call { .. }) {
            return Err(RuntimeError::Invariant(
                "Iterator.from did not receive a generic invocation",
            ));
        }
        let input = arguments
            .readable
            .first()
            .cloned()
            .ok_or(RuntimeError::Invariant(
                "Iterator.from argument was not padded",
            ))?;
        if !matches!(input, Value::Object(_) | Value::String(_)) {
            return Ok(Self::Complete(Completion::Throw(
                runtime.new_native_error(
                    realm,
                    NativeErrorKind::Type,
                    "Iterator.from called on non-object",
                )?,
            )));
        }
        Ok(Self::Read {
            receiver: input.clone(),
            key: PropertyKey::from(runtime.well_known_symbol(WellKnownSymbol::Iterator)),
            resume: FromResume {
                realm,
                phase: Phase::Method(input),
            },
        })
    }
}
impl FromResume {
    fn next(mut self, runtime: &Runtime, iterator: Value) -> Result<FromStep, RuntimeError> {
        self.phase = Phase::Next(iterator.clone());
        Ok(FromStep::Read {
            receiver: iterator,
            key: runtime.intern_property_key("next")?,
            resume: self,
        })
    }
    pub(crate) fn resume(
        mut self,
        runtime: &Runtime,
        reply: Completion,
    ) -> Result<FromStep, RuntimeError> {
        let value = match reply {
            Completion::Return(value) => value,
            Completion::Throw(value) => return Ok(FromStep::Complete(Completion::Throw(value))),
        };
        match std::mem::replace(&mut self.phase, Phase::Iterator) {
            Phase::Method(input) => {
                if matches!(value, Value::Undefined | Value::Null) {
                    return self.next(runtime, input);
                }
                let callable = match runtime.iterator_callable_value(self.realm, value)? {
                    NativeConversion::Value(value) => value,
                    NativeConversion::Throw(value) => {
                        return Ok(FromStep::Complete(Completion::Throw(value)));
                    }
                };
                Ok(FromStep::Call {
                    callable,
                    receiver: input,
                    resume: self,
                })
            }
            Phase::Iterator => {
                if !matches!(value, Value::Object(_)) {
                    return Ok(FromStep::Complete(Completion::Throw(
                        runtime.new_native_error(
                            self.realm,
                            NativeErrorKind::Type,
                            "not an object",
                        )?,
                    )));
                }
                self.next(runtime, value)
            }
            Phase::Next(iterator) => {
                let constructor = runtime.iterator_realm_data(self.realm)?.constructor;
                let constructor = CallableRef::from_validated_object(
                    ObjectRef::from_borrowed_handle(runtime.clone(), constructor)?,
                );
                self.phase = Phase::Instance {
                    iterator: iterator.clone(),
                    next: value,
                };
                Ok(FromStep::Instance {
                    constructor,
                    value: iterator,
                    resume: self,
                })
            }
            Phase::Instance { iterator, next } => Ok(FromStep::Complete(Completion::Return(
                if runtime.value_to_boolean(&value)? {
                    iterator
                } else {
                    Value::Object(runtime.new_iterator_wrap(self.realm, &iterator, &next)?)
                },
            ))),
        }
    }
}
pub(crate) fn finish(
    runtime: &Runtime,
    realm: ContextId,
    mut step: FromStep,
) -> Result<Completion, RuntimeError> {
    loop {
        step = match step {
            FromStep::Complete(result) => return Ok(result),
            FromStep::Read {
                receiver,
                key,
                resume,
            } => resume.resume(
                runtime,
                runtime.get_value_property_in_realm(realm, receiver, &key)?,
            )?,
            FromStep::Call {
                callable,
                receiver,
                resume,
            } => resume.resume(
                runtime,
                runtime.call_internal(realm, &callable, receiver, &[])?,
            )?,
            FromStep::Instance {
                constructor,
                value,
                resume,
            } => resume.resume(
                runtime,
                runtime.ordinary_is_instance_of(realm, &constructor, value)?,
            )?,
        };
    }
}
