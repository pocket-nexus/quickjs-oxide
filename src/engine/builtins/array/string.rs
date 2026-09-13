//! Array stringification retains partial output and failure ordering across callbacks.
#[cfg(feature = "stack-vm")]
use crate::engine::builtins::native::NativeFunctionId;
use crate::engine::{
    api::{error::NativeErrorKind, runtime::Runtime, runtime_error::RuntimeError},
    builtins::native::ArrayJoinKind,
    heap::ContextId,
    object::{CallableRef, ObjectRef, PropertyKey},
    value::{JsString, JsStringBuilder, JsStringError, Value, conversion::NativeConversion},
    vm::{
        Completion,
        call::{NativeArguments, NativeInvocation},
    },
};
#[derive(Clone, Copy)]
pub(crate) enum ArrayStringKind {
    Join(ArrayJoinKind),
    ToString,
}
impl ArrayStringKind {
    #[cfg(feature = "stack-vm")]
    pub(crate) fn for_target(target: NativeFunctionId) -> Option<Self> {
        match target {
            NativeFunctionId::ArrayPrototypeJoin(kind) => Some(Self::Join(kind)),
            NativeFunctionId::ArrayPrototypeToString => Some(Self::ToString),
            _ => None,
        }
    }
}
pub(crate) enum ArrayStringStep {
    Complete(Completion),
    Read {
        receiver: Value,
        key: PropertyKey,
        resume: ArrayStringResume,
    },
    Number {
        value: Value,
        resume: ArrayStringResume,
    },
    String {
        value: Value,
        resume: ArrayStringResume,
    },
    Call {
        callable: CallableRef,
        receiver: Value,
        resume: ArrayStringResume,
    },
    ObjectTag {
        receiver: Value,
    },
}
enum Phase {
    Length,
    Number,
    Separator,
    Element,
    LocaleMethod,
    LocaleResult,
    ElementString,
    JoinMethod,
    JoinResult,
}
pub(crate) struct ArrayStringResume {
    realm: ContextId,
    kind: ArrayStringKind,
    object: ObjectRef,
    phase: Phase,
    separator_value: Value,
    separator: JsString,
    output: JsStringBuilder,
    separator_error: Option<JsStringError>,
    length: u64,
    index: u64,
    element: Value,
}
impl ArrayStringStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        kind: ArrayStringKind,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
        string_limit: usize,
    ) -> Result<Self, RuntimeError> {
        let NativeInvocation::Call { this_value } = invocation else {
            return Err(RuntimeError::Invariant(
                "Array string method requires generic invocation",
            ));
        };
        let object = match runtime.native_to_object(realm, this_value.clone())? {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => return Ok(Self::Complete(Completion::Throw(value))),
        };
        let to_string = matches!(kind, ArrayStringKind::ToString);
        Ok(Self::Read {
            receiver: Value::Object(object.clone()),
            key: runtime.intern_property_key(if to_string { "join" } else { "length" })?,
            resume: ArrayStringResume {
                realm,
                kind,
                object,
                phase: if to_string {
                    Phase::JoinMethod
                } else {
                    Phase::Length
                },
                separator_value: arguments
                    .readable
                    .first()
                    .cloned()
                    .unwrap_or(Value::Undefined),
                separator: JsString::from_static(","),
                output: JsStringBuilder::with_limit(0, string_limit),
                separator_error: None,
                length: 0,
                index: 0,
                element: Value::Undefined,
            },
        })
    }
}
impl ArrayStringResume {
    pub(crate) fn resume(
        mut self,
        runtime: &Runtime,
        result: Completion,
    ) -> Result<ArrayStringStep, RuntimeError> {
        let value = match result {
            Completion::Return(value) => value,
            Completion::Throw(value) => {
                return Ok(ArrayStringStep::Complete(Completion::Throw(value)));
            }
        };
        match self.phase {
            Phase::Length => {
                self.phase = Phase::Number;
                Ok(ArrayStringStep::Number {
                    value,
                    resume: self,
                })
            }
            Phase::Element => {
                if matches!(value, Value::Undefined | Value::Null) {
                    self.index += 1;
                    return self.next(runtime);
                }
                if matches!(
                    self.kind,
                    ArrayStringKind::Join(ArrayJoinKind::ToLocaleString)
                ) {
                    self.element = value.clone();
                    self.phase = Phase::LocaleMethod;
                    return Ok(ArrayStringStep::Read {
                        receiver: value,
                        key: runtime.intern_property_key("toLocaleString")?,
                        resume: self,
                    });
                }
                self.element_string(value)
            }
            Phase::LocaleMethod => {
                let callable = match value {
                    Value::Object(object) => runtime.as_callable(&object)?,
                    _ => None,
                };
                let Some(callable) = callable else {
                    return Ok(ArrayStringStep::Complete(Completion::Throw(
                        runtime.new_native_error(
                            self.realm,
                            NativeErrorKind::Type,
                            "not a function",
                        )?,
                    )));
                };
                self.phase = Phase::LocaleResult;
                Ok(ArrayStringStep::Call {
                    callable,
                    receiver: self.element.clone(),
                    resume: self,
                })
            }
            Phase::LocaleResult => {
                self.element = Value::Undefined;
                self.element_string(value)
            }
            Phase::JoinMethod => {
                let callable = match value {
                    Value::Object(object) => runtime.as_callable(&object)?,
                    _ => None,
                };
                if let Some(callable) = callable {
                    self.phase = Phase::JoinResult;
                    Ok(ArrayStringStep::Call {
                        callable,
                        receiver: Value::Object(self.object.clone()),
                        resume: self,
                    })
                } else {
                    Ok(ArrayStringStep::ObjectTag {
                        receiver: Value::Object(self.object),
                    })
                }
            }
            Phase::JoinResult => Ok(ArrayStringStep::Complete(Completion::Return(value))),
            _ => Err(RuntimeError::Invariant("Array string value phase mismatch")),
        }
    }
    fn element_string(mut self, value: Value) -> Result<ArrayStringStep, RuntimeError> {
        if let Some(error) = self.separator_error {
            return Err(error.into());
        }
        self.phase = Phase::ElementString;
        Ok(ArrayStringStep::String {
            value,
            resume: self,
        })
    }
    pub(crate) fn number(
        mut self,
        runtime: &Runtime,
        result: NativeConversion<f64>,
    ) -> Result<ArrayStringStep, RuntimeError> {
        if !matches!(self.phase, Phase::Number) {
            return Err(RuntimeError::Invariant(
                "Array string number phase mismatch",
            ));
        }
        self.length = match result {
            NativeConversion::Value(value) => Runtime::length_from_number(value),
            NativeConversion::Throw(value) => {
                return Ok(ArrayStringStep::Complete(Completion::Throw(value)));
            }
        };
        if matches!(self.kind, ArrayStringKind::Join(ArrayJoinKind::Join))
            && !matches!(self.separator_value, Value::Undefined)
        {
            self.phase = Phase::Separator;
            let value = std::mem::replace(&mut self.separator_value, Value::Undefined);
            return Ok(ArrayStringStep::String {
                value,
                resume: self,
            });
        }
        self.next(runtime)
    }
    pub(crate) fn string(
        mut self,
        runtime: &Runtime,
        result: NativeConversion<JsString>,
    ) -> Result<ArrayStringStep, RuntimeError> {
        let value = match result {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => {
                return Ok(ArrayStringStep::Complete(Completion::Throw(value)));
            }
        };
        match self.phase {
            Phase::Separator => self.separator = value,
            Phase::ElementString => {
                self.output.push_js_string(&value)?;
                self.index += 1;
            }
            _ => {
                return Err(RuntimeError::Invariant(
                    "Array string conversion phase mismatch",
                ));
            }
        }
        self.next(runtime)
    }
    fn next(mut self, runtime: &Runtime) -> Result<ArrayStringStep, RuntimeError> {
        if self.index == self.length {
            if let Some(error) = self.separator_error {
                return Err(error.into());
            }
            return Ok(ArrayStringStep::Complete(Completion::Return(
                Value::String(self.output.finish()?),
            )));
        }
        if self.index != 0
            && let Err(error) = self.output.push_js_string(&self.separator)
        {
            self.separator_error.get_or_insert(error);
        }
        self.phase = Phase::Element;
        Ok(ArrayStringStep::Read {
            receiver: Value::Object(self.object.clone()),
            key: runtime.property_key_for_index(u64::from(self.index as u32))?,
            resume: self,
        })
    }
}
pub(crate) fn finish(
    runtime: &Runtime,
    realm: ContextId,
    mut step: ArrayStringStep,
) -> Result<Completion, RuntimeError> {
    loop {
        step = match step {
            ArrayStringStep::Complete(result) => return Ok(result),
            ArrayStringStep::Read {
                receiver,
                key,
                resume,
            } => resume.resume(
                runtime,
                runtime.get_value_property_in_realm(realm, receiver, &key)?,
            )?,
            ArrayStringStep::Number { value, resume } => {
                resume.number(runtime, runtime.native_to_number(realm, &value)?)?
            }
            ArrayStringStep::String { value, resume } => {
                resume.string(runtime, runtime.native_to_js_string(realm, &value)?)?
            }
            ArrayStringStep::Call {
                callable,
                receiver,
                resume,
            } => resume.resume(
                runtime,
                runtime.call_internal(realm, &callable, receiver, &[])?,
            )?,
            ArrayStringStep::ObjectTag { receiver } => {
                return runtime.call_object_prototype_to_string(
                    realm,
                    NativeInvocation::Call {
                        this_value: receiver,
                    },
                );
            }
        };
    }
}
