//! String split retains its fresh result through limit and separator coercion.
use crate::engine::{
    api::{error::NativeErrorKind, runtime::Runtime, runtime_error::RuntimeError},
    heap::ContextId,
    object::{ObjectRef, PropertyKey, WellKnownSymbol},
    value::{JsString, Value, conversion::NativeConversion},
    vm::{
        Completion, ToPrimitiveHint,
        call::{DirectCallTarget, NativeArguments, NativeInvocation},
    },
};
pub(crate) enum StringSplitStep {
    Complete(Completion),
    Read {
        object: ObjectRef,
        key: PropertyKey,
        resume: StringSplitResume,
    },
    Primitive {
        value: Value,
        hint: ToPrimitiveHint,
        resume: StringSplitResume,
    },
    Call {
        target: DirectCallTarget,
        receiver: Value,
        arguments: Vec<Value>,
        resume: StringSplitResume,
    },
}
pub(crate) struct StringSplitResume {
    realm: ContextId,
    receiver: Value,
    separator: Value,
    limit: Value,
    phase: SplitPhase,
}
enum SplitPhase {
    Method,
    Called,
    Source,
    Limit {
        source: JsString,
        result: ObjectRef,
    },
    Separator {
        source: JsString,
        result: ObjectRef,
        limit: u32,
    },
}
impl StringSplitStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Self, RuntimeError> {
        let NativeInvocation::Call { this_value } = invocation else {
            return Err(RuntimeError::Invariant(
                "String split did not receive a generic invocation",
            ));
        };
        if matches!(this_value, Value::Undefined | Value::Null) {
            return Ok(Self::Complete(Completion::Throw(
                runtime.new_native_error(
                    realm,
                    NativeErrorKind::Type,
                    "cannot convert to object",
                )?,
            )));
        }
        let separator = arguments
            .readable
            .first()
            .ok_or(RuntimeError::Invariant(
                "String split separator argv was not padded",
            ))?
            .clone();
        let limit = arguments
            .readable
            .get(1)
            .ok_or(RuntimeError::Invariant(
                "String split limit argv was not padded",
            ))?
            .clone();
        let resume = StringSplitResume {
            realm,
            receiver: this_value.clone(),
            separator,
            limit,
            phase: SplitPhase::Method,
        };
        if let Value::Object(object) = &resume.separator {
            Ok(Self::Read {
                object: object.clone(),
                key: PropertyKey::from(runtime.well_known_symbol(WellKnownSymbol::Split)),
                resume,
            })
        } else {
            Ok(resume.source())
        }
    }
}
impl StringSplitResume {
    fn source(self) -> StringSplitStep {
        StringSplitStep::Primitive {
            value: self.receiver.clone(),
            hint: ToPrimitiveHint::String,
            resume: Self {
                phase: SplitPhase::Source,
                ..self
            },
        }
    }
    fn separator(self, source: JsString, result: ObjectRef, limit: u32) -> StringSplitStep {
        StringSplitStep::Primitive {
            value: self.separator.clone(),
            hint: ToPrimitiveHint::String,
            resume: Self {
                phase: SplitPhase::Separator {
                    source,
                    result,
                    limit,
                },
                ..self
            },
        }
    }
    pub(crate) fn resume(
        self,
        runtime: &Runtime,
        result: Completion,
    ) -> Result<StringSplitStep, RuntimeError> {
        let value = match result {
            Completion::Return(value) => value,
            Completion::Throw(value) => {
                return Ok(StringSplitStep::Complete(Completion::Throw(value)));
            }
        };
        let realm = self.realm;
        match self.phase {
            SplitPhase::Method => {
                if matches!(value, Value::Undefined | Value::Null) {
                    return Ok(self.source());
                }
                let callable = match value {
                    Value::Object(object) => runtime.as_callable(&object)?,
                    _ => None,
                };
                let Some(callable) = callable else {
                    return Ok(StringSplitStep::Complete(Completion::Throw(
                        runtime.new_native_error(realm, NativeErrorKind::Type, "not a function")?,
                    )));
                };
                let mut arguments = Vec::new();
                if arguments.try_reserve_exact(2).is_err() {
                    return Ok(StringSplitStep::Complete(Completion::Throw(
                        runtime.new_native_error(
                            realm,
                            NativeErrorKind::Internal,
                            "out of memory",
                        )?,
                    )));
                }
                arguments.push(self.receiver.clone());
                arguments.push(self.limit.clone());
                Ok(StringSplitStep::Call {
                    target: DirectCallTarget::Callable(callable),
                    receiver: self.separator.clone(),
                    arguments,
                    resume: Self {
                        phase: SplitPhase::Called,
                        ..self
                    },
                })
            }
            SplitPhase::Called => Ok(StringSplitStep::Complete(Completion::Return(value))),
            SplitPhase::Source => {
                if matches!(value, Value::Object(_)) {
                    return Err(RuntimeError::Invariant(
                        "String split source conversion returned an object",
                    ));
                }
                let source = match runtime.native_to_js_string(realm, &value)? {
                    NativeConversion::Value(value) => value.linearize(),
                    NativeConversion::Throw(value) => {
                        return Ok(StringSplitStep::Complete(Completion::Throw(value)));
                    }
                };
                let result = runtime.new_array(realm)?;
                if matches!(self.limit, Value::Undefined) {
                    Ok(self.separator(source, result, u32::MAX))
                } else {
                    Ok(StringSplitStep::Primitive {
                        value: self.limit.clone(),
                        hint: ToPrimitiveHint::Number,
                        resume: Self {
                            phase: SplitPhase::Limit { source, result },
                            ..self
                        },
                    })
                }
            }
            SplitPhase::Limit { source, result } => {
                if matches!(value, Value::Object(_)) {
                    return Err(RuntimeError::Invariant(
                        "String split limit conversion returned an object",
                    ));
                }
                let limit = match runtime.native_to_number(realm, &value)? {
                    NativeConversion::Value(value) => Runtime::to_uint32_number(value),
                    NativeConversion::Throw(value) => {
                        return Ok(StringSplitStep::Complete(Completion::Throw(value)));
                    }
                };
                Ok(Self {
                    phase: SplitPhase::Called,
                    ..self
                }
                .separator(source, result, limit))
            }
            SplitPhase::Separator {
                source,
                result,
                limit,
            } => {
                if matches!(value, Value::Object(_)) {
                    return Err(RuntimeError::Invariant(
                        "String split separator conversion returned an object",
                    ));
                }
                let separator = match runtime.native_to_js_string(realm, &value)? {
                    NativeConversion::Value(value) => value.linearize(),
                    NativeConversion::Throw(value) => {
                        return Ok(StringSplitStep::Complete(Completion::Throw(value)));
                    }
                };
                Ok(StringSplitStep::Complete(runtime.finish_string_split(
                    realm,
                    source,
                    result,
                    &self.separator,
                    separator,
                    limit,
                )?))
            }
        }
    }
}
pub(super) fn finish(
    runtime: &Runtime,
    realm: ContextId,
    mut step: StringSplitStep,
) -> Result<Completion, RuntimeError> {
    loop {
        step = match step {
            StringSplitStep::Complete(result) => return Ok(result),
            StringSplitStep::Read {
                object,
                key,
                resume,
            } => resume.resume(
                runtime,
                runtime.get_property_in_realm(realm, &object, &key)?,
            )?,
            StringSplitStep::Primitive {
                value,
                hint,
                resume,
            } => {
                let result = if matches!(value, Value::Object(_)) {
                    runtime.to_primitive(realm, value, hint)?
                } else {
                    Completion::Return(value)
                };
                resume.resume(runtime, result)?
            }
            StringSplitStep::Call {
                target,
                receiver,
                arguments,
                resume,
            } => {
                let DirectCallTarget::Callable(callable) = target else {
                    return Err(RuntimeError::Invariant(
                        "String split requested an invalid call target",
                    ));
                };
                resume.resume(
                    runtime,
                    runtime.call_internal(realm, &callable, receiver, &arguments)?,
                )?
            }
        };
    }
}
