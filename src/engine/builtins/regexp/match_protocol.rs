//! RegExp @@match preserves global collection and empty-match progress.
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

pub(crate) fn advance_string_index(input: &JsString, index: u64, unicode: bool) -> u64 {
    let width = if unicode
        && usize::try_from(index)
            .ok()
            .filter(|index| *index < input.len())
            .and_then(|index| input.code_point_at(index))
            .is_some_and(|code_point| code_point > u32::from(u16::MAX))
    {
        2
    } else {
        1
    };
    index + width
}

pub(crate) enum RegExpMatchStep {
    Complete(Completion),
    Primitive {
        value: Value,
        hint: ToPrimitiveHint,
        resume: RegExpMatchResume,
    },
    Read {
        object: ObjectRef,
        key: PropertyKey,
        resume: RegExpMatchResume,
    },
    Set {
        object: ObjectRef,
        key: PropertyKey,
        value: Value,
        resume: RegExpMatchResume,
    },
    Exec {
        regexp: Value,
        input: Value,
        resume: RegExpMatchResume,
    },
}
pub(crate) struct RegExpMatchResume {
    realm: ContextId,
    regexp: ObjectRef,
    phase: MatchPhase,
}
struct MatchCollection {
    input: JsString,
    unicode: bool,
    matches: ObjectRef,
    zero: PropertyKey,
    count: u32,
}
enum MatchPhase {
    Input,
    Flags(JsString),
    FlagsString(JsString),
    InitialSet { input: JsString, unicode: bool },
    Single,
    Exec(MatchCollection),
    Match(MatchCollection),
    MatchString(MatchCollection),
    LastIndex(MatchCollection),
    LastIndexNumber(MatchCollection),
    AdvancedSet(MatchCollection),
}
impl RegExpMatchStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Self, RuntimeError> {
        let NativeInvocation::Call { this_value } = invocation else {
            return Err(RuntimeError::Invariant(
                "RegExp @@match did not receive a generic invocation",
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
                    "RegExp @@match input argv was not padded",
                ))?
                .clone(),
            hint: ToPrimitiveHint::String,
            resume: RegExpMatchResume {
                realm,
                regexp: regexp.clone(),
                phase: MatchPhase::Input,
            },
        })
    }
}
impl RegExpMatchResume {
    fn execute(self, state: MatchCollection) -> RegExpMatchStep {
        RegExpMatchStep::Exec {
            regexp: Value::Object(self.regexp.clone()),
            input: Value::String(state.input.clone()),
            resume: Self {
                phase: MatchPhase::Exec(state),
                ..self
            },
        }
    }
    pub(crate) fn set(
        self,
        runtime: &Runtime,
        result: NativeConversion<InternalSetResult>,
    ) -> Result<RegExpMatchStep, RuntimeError> {
        let key = runtime.intern_property_key("lastIndex")?;
        if let Some(value) = runtime.finish_set_property_or_throw(self.realm, &key, result)? {
            return Ok(RegExpMatchStep::Complete(Completion::Throw(value)));
        }
        match self.phase {
            MatchPhase::InitialSet { input, unicode } => {
                let matches = runtime.new_array(self.realm)?;
                let zero = runtime.intern_property_key("0")?;
                Ok(Self {
                    phase: MatchPhase::Single,
                    ..self
                }
                .execute(MatchCollection {
                    input,
                    unicode,
                    matches,
                    zero,
                    count: 0,
                }))
            }
            MatchPhase::AdvancedSet(state) => Ok(Self {
                phase: MatchPhase::Single,
                ..self
            }
            .execute(state)),
            _ => Err(RuntimeError::Invariant(
                "RegExp match received an unexpected Set reply",
            )),
        }
    }
    pub(crate) fn resume(
        self,
        runtime: &Runtime,
        result: Completion,
    ) -> Result<RegExpMatchStep, RuntimeError> {
        let value = match result {
            Completion::Return(value) => value,
            Completion::Throw(value) => {
                return Ok(RegExpMatchStep::Complete(Completion::Throw(value)));
            }
        };
        match self.phase {
            MatchPhase::Input => {
                let input = match match_string(runtime, self.realm, value)? {
                    NativeConversion::Value(value) => value,
                    NativeConversion::Throw(value) => {
                        return Ok(RegExpMatchStep::Complete(Completion::Throw(value)));
                    }
                };
                Ok(RegExpMatchStep::Read {
                    object: self.regexp.clone(),
                    key: runtime.intern_property_key("flags")?,
                    resume: Self {
                        phase: MatchPhase::Flags(input),
                        ..self
                    },
                })
            }
            MatchPhase::Flags(input) => Ok(RegExpMatchStep::Primitive {
                value,
                hint: ToPrimitiveHint::String,
                resume: Self {
                    phase: MatchPhase::FlagsString(input),
                    ..self
                },
            }),
            MatchPhase::FlagsString(input) => {
                let flags = match match_string(runtime, self.realm, value)? {
                    NativeConversion::Value(value) => value,
                    NativeConversion::Throw(value) => {
                        return Ok(RegExpMatchStep::Complete(Completion::Throw(value)));
                    }
                };
                if !flags.utf16_units().any(|unit| unit == u16::from(b'g')) {
                    return Ok(RegExpMatchStep::Exec {
                        regexp: Value::Object(self.regexp.clone()),
                        input: Value::String(input),
                        resume: Self {
                            phase: MatchPhase::Single,
                            ..self
                        },
                    });
                }
                let unicode = flags
                    .utf16_units()
                    .any(|unit| unit == u16::from(b'u') || unit == u16::from(b'v'));
                Ok(RegExpMatchStep::Set {
                    object: self.regexp.clone(),
                    key: runtime.intern_property_key("lastIndex")?,
                    value: Value::Int(0),
                    resume: Self {
                        phase: MatchPhase::InitialSet { input, unicode },
                        ..self
                    },
                })
            }
            MatchPhase::Single => Ok(RegExpMatchStep::Complete(Completion::Return(value))),
            MatchPhase::Exec(state) => {
                let result = match value {
                    Value::Null => {
                        return Ok(RegExpMatchStep::Complete(Completion::Return(
                            if state.count == 0 {
                                Value::Null
                            } else {
                                Value::Object(state.matches)
                            },
                        )));
                    }
                    Value::Object(result) => result,
                    _ => {
                        return Err(RuntimeError::Invariant(
                            "RegExpExec returned neither an object nor null",
                        ));
                    }
                };
                Ok(RegExpMatchStep::Read {
                    object: result,
                    key: state.zero.clone(),
                    resume: Self {
                        phase: MatchPhase::Match(state),
                        ..self
                    },
                })
            }
            MatchPhase::Match(state) => Ok(RegExpMatchStep::Primitive {
                value,
                hint: ToPrimitiveHint::String,
                resume: Self {
                    phase: MatchPhase::MatchString(state),
                    ..self
                },
            }),
            MatchPhase::MatchString(mut state) => {
                let matched = match match_string(runtime, self.realm, value)? {
                    NativeConversion::Value(value) => value,
                    NativeConversion::Throw(value) => {
                        return Ok(RegExpMatchStep::Complete(Completion::Throw(value)));
                    }
                };
                let empty = matched.is_empty();
                let Some(next) = state.count.checked_add(1) else {
                    return Ok(RegExpMatchStep::Complete(Completion::Throw(
                        runtime.new_native_error(
                            self.realm,
                            NativeErrorKind::Range,
                            "invalid array length",
                        )?,
                    )));
                };
                if let Some(value) = runtime.create_array_data_property(
                    self.realm,
                    &state.matches,
                    state.count,
                    Value::String(matched),
                )? {
                    return Ok(RegExpMatchStep::Complete(Completion::Throw(value)));
                }
                state.count = next;
                if empty {
                    Ok(RegExpMatchStep::Read {
                        object: self.regexp.clone(),
                        key: runtime.intern_property_key("lastIndex")?,
                        resume: Self {
                            phase: MatchPhase::LastIndex(state),
                            ..self
                        },
                    })
                } else {
                    Ok(Self {
                        phase: MatchPhase::Single,
                        ..self
                    }
                    .execute(state))
                }
            }
            MatchPhase::LastIndex(state) => Ok(RegExpMatchStep::Primitive {
                value,
                hint: ToPrimitiveHint::Number,
                resume: Self {
                    phase: MatchPhase::LastIndexNumber(state),
                    ..self
                },
            }),
            MatchPhase::LastIndexNumber(state) => {
                if matches!(value, Value::Object(_)) {
                    return Err(RuntimeError::Invariant(
                        "RegExp match lastIndex conversion returned an object",
                    ));
                }
                let current = match runtime.native_to_length(self.realm, &value)? {
                    NativeConversion::Value(value) => value,
                    NativeConversion::Throw(value) => {
                        return Ok(RegExpMatchStep::Complete(Completion::Throw(value)));
                    }
                };
                let next = advance_string_index(&state.input, current, state.unicode);
                Ok(RegExpMatchStep::Set {
                    object: self.regexp.clone(),
                    key: runtime.intern_property_key("lastIndex")?,
                    value: Value::number(next as f64),
                    resume: Self {
                        phase: MatchPhase::AdvancedSet(state),
                        ..self
                    },
                })
            }
            MatchPhase::InitialSet { .. } | MatchPhase::AdvancedSet(_) => Err(
                RuntimeError::Invariant("RegExp match Set received an untyped reply"),
            ),
        }
    }
}
fn match_string(
    runtime: &Runtime,
    realm: ContextId,
    value: Value,
) -> Result<NativeConversion<JsString>, RuntimeError> {
    if matches!(value, Value::Object(_)) {
        return Err(RuntimeError::Invariant(
            "RegExp match String conversion returned an object",
        ));
    }
    runtime.native_to_js_string(realm, &value)
}
impl Runtime {
    pub(crate) fn call_regexp_symbol_match(
        &self,
        realm: ContextId,
        invocation: NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Completion, RuntimeError> {
        let mut step = RegExpMatchStep::start(self, realm, &invocation, arguments)?;
        loop {
            step = match step {
                RegExpMatchStep::Complete(result) => return Ok(result),
                RegExpMatchStep::Primitive {
                    value,
                    hint,
                    resume,
                } => {
                    let result = if matches!(value, Value::Object(_)) {
                        self.to_primitive(realm, value, hint)?
                    } else {
                        Completion::Return(value)
                    };
                    resume.resume(self, result)?
                }
                RegExpMatchStep::Read {
                    object,
                    key,
                    resume,
                } => resume.resume(self, self.get_property_in_realm(realm, &object, &key)?)?,
                RegExpMatchStep::Exec {
                    regexp,
                    input,
                    resume,
                } => resume.resume(self, self.regexp_exec_abstract(realm, regexp, input)?)?,
                RegExpMatchStep::Set {
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
