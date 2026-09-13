//! RegExp search restores lastIndex only after successful execution and reread.
use crate::engine::{
    api::{error::NativeErrorKind, runtime::Runtime, runtime_error::RuntimeError},
    heap::ContextId,
    object::{ObjectRef, PropertyKey, operations::InternalSetResult},
    value::{JsString, Value, conversion::NativeConversion},
    vm::{
        Completion, ToPrimitiveHint,
        call::{NativeArguments, NativeInvocation},
    },
};
pub(crate) enum RegExpSearchStep {
    Complete(Completion),
    Primitive {
        value: Value,
        resume: RegExpSearchResume,
    },
    Read {
        object: ObjectRef,
        key: PropertyKey,
        resume: RegExpSearchResume,
    },
    Set {
        object: ObjectRef,
        key: PropertyKey,
        value: Value,
        resume: RegExpSearchResume,
    },
    Exec {
        regexp: Value,
        input: Value,
        resume: RegExpSearchResume,
    },
}
pub(crate) struct RegExpSearchResume {
    realm: ContextId,
    regexp: ObjectRef,
    phase: SearchPhase,
}
enum SearchPhase {
    Input,
    Previous(JsString),
    InitialSet { input: JsString, previous: Value },
    Exec(Value),
    Current { previous: Value, result: Value },
    Restored(Value),
    Index,
}
impl RegExpSearchStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Self, RuntimeError> {
        let NativeInvocation::Call { this_value } = invocation else {
            return Err(RuntimeError::Invariant(
                "RegExp @@search did not receive a generic invocation",
            ));
        };
        let Value::Object(regexp) = this_value else {
            return Ok(Self::Complete(Completion::Throw(
                runtime.new_native_error(realm, NativeErrorKind::Type, "not an object")?,
            )));
        };
        Ok(Self::Primitive {
            value: arguments
                .readable
                .first()
                .ok_or(RuntimeError::Invariant(
                    "RegExp @@search input argv was not padded",
                ))?
                .clone(),
            resume: RegExpSearchResume {
                realm,
                regexp: regexp.clone(),
                phase: SearchPhase::Input,
            },
        })
    }
}
impl RegExpSearchResume {
    fn execute(self, input: JsString, previous: Value) -> RegExpSearchStep {
        RegExpSearchStep::Exec {
            regexp: Value::Object(self.regexp.clone()),
            input: Value::String(input),
            resume: Self {
                phase: SearchPhase::Exec(previous),
                ..self
            },
        }
    }
    fn result(self, runtime: &Runtime, result: Value) -> Result<RegExpSearchStep, RuntimeError> {
        match result {
            Value::Null => Ok(RegExpSearchStep::Complete(Completion::Return(Value::Int(
                -1,
            )))),
            Value::Object(result) => Ok(RegExpSearchStep::Read {
                object: result,
                key: runtime.intern_property_key("index")?,
                resume: Self {
                    phase: SearchPhase::Index,
                    ..self
                },
            }),
            _ => Err(RuntimeError::Invariant(
                "RegExpExec returned neither an object nor null",
            )),
        }
    }
    pub(crate) fn resume(
        self,
        runtime: &Runtime,
        result: Completion,
    ) -> Result<RegExpSearchStep, RuntimeError> {
        // Both RegExpExec and the post-exec Get throw without restoration.
        let value = match result {
            Completion::Return(value) => value,
            Completion::Throw(value) => {
                return Ok(RegExpSearchStep::Complete(Completion::Throw(value)));
            }
        };
        match self.phase {
            SearchPhase::Input => {
                if matches!(value, Value::Object(_)) {
                    return Err(RuntimeError::Invariant(
                        "RegExp search input conversion returned an object",
                    ));
                }
                let input = match runtime.native_to_js_string(self.realm, &value)? {
                    NativeConversion::Value(value) => value,
                    NativeConversion::Throw(value) => {
                        return Ok(RegExpSearchStep::Complete(Completion::Throw(value)));
                    }
                };
                Ok(RegExpSearchStep::Read {
                    object: self.regexp.clone(),
                    key: runtime.intern_property_key("lastIndex")?,
                    resume: Self {
                        phase: SearchPhase::Previous(input),
                        ..self
                    },
                })
            }
            SearchPhase::Previous(input) => {
                if value.same_value(&Value::Int(0)) {
                    Ok(Self {
                        phase: SearchPhase::Index,
                        ..self
                    }
                    .execute(input, value))
                } else {
                    Ok(RegExpSearchStep::Set {
                        object: self.regexp.clone(),
                        key: runtime.intern_property_key("lastIndex")?,
                        value: Value::Int(0),
                        resume: Self {
                            phase: SearchPhase::InitialSet {
                                input,
                                previous: value,
                            },
                            ..self
                        },
                    })
                }
            }
            SearchPhase::Exec(previous) => Ok(RegExpSearchStep::Read {
                object: self.regexp.clone(),
                key: runtime.intern_property_key("lastIndex")?,
                resume: Self {
                    phase: SearchPhase::Current {
                        previous,
                        result: value,
                    },
                    ..self
                },
            }),
            SearchPhase::Current { previous, result } => {
                if value.same_value(&previous) {
                    Self {
                        phase: SearchPhase::Index,
                        ..self
                    }
                    .result(runtime, result)
                } else {
                    Ok(RegExpSearchStep::Set {
                        object: self.regexp.clone(),
                        key: runtime.intern_property_key("lastIndex")?,
                        value: previous,
                        resume: Self {
                            phase: SearchPhase::Restored(result),
                            ..self
                        },
                    })
                }
            }
            SearchPhase::Index => Ok(RegExpSearchStep::Complete(Completion::Return(value))),
            _ => Err(RuntimeError::Invariant(
                "RegExp search Set received an untyped reply",
            )),
        }
    }
    pub(crate) fn set(
        self,
        runtime: &Runtime,
        result: NativeConversion<InternalSetResult>,
    ) -> Result<RegExpSearchStep, RuntimeError> {
        let key = runtime.intern_property_key("lastIndex")?;
        if let Some(value) = runtime.finish_set_property_or_throw(self.realm, &key, result)? {
            return Ok(RegExpSearchStep::Complete(Completion::Throw(value)));
        }
        match self.phase {
            SearchPhase::InitialSet { input, previous } => Ok(Self {
                phase: SearchPhase::Index,
                ..self
            }
            .execute(input, previous)),
            SearchPhase::Restored(result) => Self {
                phase: SearchPhase::Index,
                ..self
            }
            .result(runtime, result),
            _ => Err(RuntimeError::Invariant(
                "RegExp search received an unexpected Set reply",
            )),
        }
    }
}
impl Runtime {
    pub(crate) fn call_regexp_symbol_search(
        &self,
        realm: ContextId,
        invocation: NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Completion, RuntimeError> {
        let mut step = RegExpSearchStep::start(self, realm, &invocation, arguments)?;
        loop {
            step = match step {
                RegExpSearchStep::Complete(result) => return Ok(result),
                RegExpSearchStep::Primitive { value, resume } => {
                    let result = if matches!(value, Value::Object(_)) {
                        self.to_primitive(realm, value, ToPrimitiveHint::String)?
                    } else {
                        Completion::Return(value)
                    };
                    resume.resume(self, result)?
                }
                RegExpSearchStep::Read {
                    object,
                    key,
                    resume,
                } => resume.resume(self, self.get_property_in_realm(realm, &object, &key)?)?,
                RegExpSearchStep::Exec {
                    regexp,
                    input,
                    resume,
                } => resume.resume(self, self.regexp_exec_abstract(realm, regexp, input)?)?,
                RegExpSearchStep::Set {
                    object,
                    key,
                    value,
                    resume,
                } => resume.set(
                    self,
                    self.internal_set(realm, &object, &key, value, Value::Object(object.clone()))?,
                )?,
            };
        }
    }
}
