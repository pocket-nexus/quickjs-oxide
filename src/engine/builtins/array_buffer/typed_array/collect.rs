//! TypedArray iterable collection keeps the pinned ordinary-call and no-close policy.
use crate::engine::{
    api::{error::NativeErrorKind, runtime::Runtime, runtime_error::RuntimeError},
    builtins::native::TypedArrayElementKind,
    heap::ContextId,
    object::{CallableRef, ObjectRef, PropertyKey, WellKnownSymbol},
    value::{Value, conversion::NativeConversion},
    vm::Completion,
};

pub(crate) enum TypedIteratorMethodStep {
    Complete(NativeConversion<Option<CallableRef>>),
    Read {
        receiver: Value,
        key: PropertyKey,
        resume: TypedIteratorMethodResume,
    },
}
pub(crate) struct TypedIteratorMethodResume {
    realm: ContextId,
}
impl TypedIteratorMethodStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        source: Value,
    ) -> Result<Self, RuntimeError> {
        if matches!(source, Value::Null | Value::Undefined) {
            return Ok(Self::Complete(NativeConversion::Throw(
                runtime.new_native_error(realm, NativeErrorKind::Type, "cannot get iterator")?,
            )));
        }
        Ok(Self::Read {
            receiver: source,
            key: PropertyKey::from(runtime.well_known_symbol(WellKnownSymbol::Iterator)),
            resume: TypedIteratorMethodResume { realm },
        })
    }
}
impl TypedIteratorMethodResume {
    pub(crate) fn resume(
        self,
        runtime: &Runtime,
        reply: Completion,
    ) -> Result<TypedIteratorMethodStep, RuntimeError> {
        let value = match reply {
            Completion::Return(value) => value,
            Completion::Throw(value) => {
                return Ok(TypedIteratorMethodStep::Complete(NativeConversion::Throw(
                    value,
                )));
            }
        };
        if matches!(value, Value::Null | Value::Undefined) {
            return Ok(TypedIteratorMethodStep::Complete(NativeConversion::Value(
                None,
            )));
        }
        let callable = match value {
            Value::Object(object) => runtime.as_callable(&object)?,
            _ => None,
        };
        Ok(TypedIteratorMethodStep::Complete(match callable {
            Some(value) => NativeConversion::Value(Some(value)),
            None => NativeConversion::Throw(runtime.new_native_error(
                self.realm,
                NativeErrorKind::Type,
                "value is not iterable",
            )?),
        }))
    }
}
pub(crate) fn finish_method(
    runtime: &Runtime,
    realm: ContextId,
    mut step: TypedIteratorMethodStep,
) -> Result<NativeConversion<Option<CallableRef>>, RuntimeError> {
    loop {
        step = match step {
            TypedIteratorMethodStep::Complete(result) => return Ok(result),
            TypedIteratorMethodStep::Read {
                receiver,
                key,
                resume,
            } => resume.resume(
                runtime,
                runtime.get_value_property_in_realm(realm, receiver, &key)?,
            )?,
        };
    }
}

pub(crate) enum TypedCollectStep {
    Complete(NativeConversion<Vec<Value>>),
    Call {
        callable: CallableRef,
        receiver: Value,
        resume: TypedCollectResume,
    },
    Read {
        object: ObjectRef,
        key: PropertyKey,
        resume: TypedCollectResume,
    },
}
enum Phase {
    Factory,
    NextMethod,
    NextResult,
    Done,
    Value,
}
pub(crate) struct TypedCollectResume {
    realm: ContextId,
    _method: CallableRef,
    iterator: Option<ObjectRef>,
    next: Option<CallableRef>,
    iteration: Option<ObjectRef>,
    done_key: Option<PropertyKey>,
    value_key: Option<PropertyKey>,
    maximum: u64,
    values: Vec<Value>,
    phase: Phase,
}
impl TypedCollectStep {
    pub(crate) fn start(
        realm: ContextId,
        source: Value,
        method: CallableRef,
        element: TypedArrayElementKind,
    ) -> Self {
        Self::Call {
            callable: method.clone(),
            receiver: source,
            resume: TypedCollectResume {
                realm,
                _method: method,
                iterator: None,
                next: None,
                iteration: None,
                done_key: None,
                value_key: None,
                maximum: super::MAX_ARRAY_BUFFER_LENGTH / u64::from(element.byte_length()),
                values: Vec::new(),
                phase: Phase::Factory,
            },
        }
    }
}
impl TypedCollectResume {
    fn abrupt(self, value: Value) -> TypedCollectStep {
        TypedCollectStep::Complete(NativeConversion::Throw(value))
    }
    fn fail(self, runtime: &Runtime, message: &str) -> Result<TypedCollectStep, RuntimeError> {
        let error = runtime.new_native_error(self.realm, NativeErrorKind::Type, message)?;
        Ok(self.abrupt(error))
    }
    fn next(mut self) -> Result<TypedCollectStep, RuntimeError> {
        self.iteration = None;
        self.phase = Phase::NextResult;
        Ok(TypedCollectStep::Call {
            callable: self
                .next
                .as_ref()
                .ok_or(RuntimeError::Invariant(
                    "TypedArray iterator lost cached next",
                ))?
                .clone(),
            receiver: Value::Object(
                self.iterator
                    .as_ref()
                    .ok_or(RuntimeError::Invariant(
                        "TypedArray collection lost iterator",
                    ))?
                    .clone(),
            ),
            resume: self,
        })
    }
    pub(crate) fn resume(
        mut self,
        runtime: &Runtime,
        reply: Completion,
    ) -> Result<TypedCollectStep, RuntimeError> {
        let value = match reply {
            Completion::Return(value) => value,
            Completion::Throw(value) => return Ok(self.abrupt(value)),
        };
        match self.phase {
            Phase::Factory => {
                let Value::Object(iterator) = value else {
                    return self.fail(runtime, "not an object");
                };
                self.iterator = Some(iterator.clone());
                self.phase = Phase::NextMethod;
                Ok(TypedCollectStep::Read {
                    object: iterator,
                    key: runtime.intern_property_key("next")?,
                    resume: self,
                })
            }
            Phase::NextMethod => {
                let next = match value {
                    Value::Object(object) => runtime.as_callable(&object)?,
                    _ => None,
                };
                let Some(next) = next else {
                    return self.fail(runtime, "not a function");
                };
                self.next = Some(next);
                self.done_key = Some(runtime.intern_property_key("done")?);
                self.value_key = Some(runtime.intern_property_key("value")?);
                self.next()
            }
            Phase::NextResult => {
                let Value::Object(iteration) = value else {
                    return self.fail(runtime, "iterator must return an object");
                };
                self.iteration = Some(iteration.clone());
                self.phase = Phase::Done;
                Ok(TypedCollectStep::Read {
                    object: iteration,
                    key: self
                        .done_key
                        .as_ref()
                        .ok_or(RuntimeError::Invariant("TypedArray iterator lost done key"))?
                        .clone(),
                    resume: self,
                })
            }
            Phase::Done => {
                if runtime.value_to_boolean(&value)? {
                    return Ok(TypedCollectStep::Complete(NativeConversion::Value(
                        self.values,
                    )));
                }
                // This limit is observed before Get(value), unlike the shared
                // generic next consumer. No failure path closes the iterator.
                if self.values.len() as u64 == self.maximum {
                    let error = runtime.typed_array_invalid_length(self.realm)?;
                    return Ok(self.abrupt(error));
                }
                self.phase = Phase::Value;
                Ok(TypedCollectStep::Read {
                    object: self
                        .iteration
                        .as_ref()
                        .ok_or(RuntimeError::Invariant("TypedArray iterator lost result"))?
                        .clone(),
                    key: self
                        .value_key
                        .as_ref()
                        .ok_or(RuntimeError::Invariant(
                            "TypedArray iterator lost value key",
                        ))?
                        .clone(),
                    resume: self,
                })
            }
            Phase::Value => {
                if self.values.try_reserve(1).is_err() {
                    let error = runtime.new_native_error(
                        self.realm,
                        NativeErrorKind::Internal,
                        "out of memory",
                    )?;
                    return Ok(self.abrupt(error));
                }
                self.values.push(value);
                self.next()
            }
        }
    }
}
pub(crate) fn finish_collect(
    runtime: &Runtime,
    realm: ContextId,
    mut step: TypedCollectStep,
) -> Result<NativeConversion<Vec<Value>>, RuntimeError> {
    loop {
        step = match step {
            TypedCollectStep::Complete(result) => return Ok(result),
            TypedCollectStep::Call {
                callable,
                receiver,
                resume,
            } => resume.resume(
                runtime,
                runtime.call_internal(realm, &callable, receiver, &[])?,
            )?,
            TypedCollectStep::Read {
                object,
                key,
                resume,
            } => resume.resume(
                runtime,
                runtime.get_property_in_realm(realm, &object, &key)?,
            )?,
        };
    }
}
