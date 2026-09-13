//! String scalar operations retain the converted receiver before argument coercions.
#[cfg(feature = "stack-vm")]
use crate::engine::builtins::native::NativeFunctionId;
use crate::engine::{
    api::{
        error::{Error, NativeErrorKind},
        runtime::Runtime,
        runtime_error::RuntimeError,
    },
    builtins::native::{StringCharAtKind, StringWellFormedKind},
    heap::ContextId,
    value::{JsString, Value, conversion::NativeConversion},
    vm::{
        Completion,
        call::{NativeArguments, NativeInvocation},
    },
};
#[derive(Clone, Copy)]
pub(crate) enum ScalarTextKind {
    CharAt(StringCharAtKind),
    CharCodeAt,
    CodePointAt,
    Concat,
    WellFormed(StringWellFormedKind),
    Iterator,
}
impl ScalarTextKind {
    #[cfg(feature = "stack-vm")]
    pub(crate) fn for_target(target: NativeFunctionId) -> Option<Self> {
        Some(match target {
            NativeFunctionId::StringPrototypeCharAt(kind) => Self::CharAt(kind),
            NativeFunctionId::StringPrototypeCharCodeAt => Self::CharCodeAt,
            NativeFunctionId::StringPrototypeCodePointAt => Self::CodePointAt,
            NativeFunctionId::StringPrototypeConcat => Self::Concat,
            NativeFunctionId::StringPrototypeWellFormed(kind) => Self::WellFormed(kind),
            NativeFunctionId::StringPrototypeIterator => Self::Iterator,
            _ => return None,
        })
    }
}
pub(crate) enum ScalarTextStep {
    Complete(Completion),
    String {
        value: Value,
        resume: ScalarTextResume,
    },
    Number {
        value: Value,
        resume: ScalarTextResume,
    },
}
enum Phase {
    Source,
    Chunk,
    Index,
}
pub(crate) struct ScalarTextResume {
    realm: ContextId,
    kind: ScalarTextKind,
    phase: Phase,
    string: JsString,
    arguments: std::vec::IntoIter<Value>,
}
impl ScalarTextStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        kind: ScalarTextKind,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Self, RuntimeError> {
        let NativeInvocation::Call { this_value } = invocation else {
            return Err(RuntimeError::Invariant(
                "String scalar method requires generic invocation",
            ));
        };
        if matches!(this_value, Value::Null | Value::Undefined) {
            return Ok(Self::Complete(Completion::Throw(
                runtime.new_native_error(
                    realm,
                    NativeErrorKind::Type,
                    "null or undefined are forbidden",
                )?,
            )));
        }
        let arguments = match kind {
            ScalarTextKind::Concat => arguments.readable[..arguments.actual_arg_count].to_vec(),
            _ => vec![
                arguments
                    .readable
                    .first()
                    .cloned()
                    .unwrap_or(Value::Undefined),
            ],
        };
        Ok(Self::String {
            value: this_value.clone(),
            resume: ScalarTextResume {
                realm,
                kind,
                phase: Phase::Source,
                string: JsString::from_static(""),
                arguments: arguments.into_iter(),
            },
        })
    }
}
impl ScalarTextResume {
    pub(crate) fn string(
        mut self,
        runtime: &Runtime,
        result: NativeConversion<JsString>,
    ) -> Result<ScalarTextStep, RuntimeError> {
        let string = match result {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => {
                return Ok(ScalarTextStep::Complete(Completion::Throw(value)));
            }
        };
        match self.phase {
            Phase::Source => self.string = string,
            Phase::Chunk => self.string = self.string.try_concat(&string).map_err(Error::from)?,
            _ => {
                return Err(RuntimeError::Invariant(
                    "String scalar string phase mismatch",
                ));
            }
        }
        match self.kind {
            ScalarTextKind::WellFormed(kind) => {
                Ok(ScalarTextStep::Complete(Completion::Return(match kind {
                    StringWellFormedKind::IsWellFormed => Value::Bool(self.string.is_well_formed()),
                    StringWellFormedKind::ToWellFormed => {
                        Value::String(self.string.to_well_formed())
                    }
                })))
            }
            ScalarTextKind::Iterator => Ok(ScalarTextStep::Complete(Completion::Return(
                Value::Object(runtime.new_string_iterator(self.realm, self.string)?),
            ))),
            ScalarTextKind::Concat => self.concat(),
            _ => {
                self.phase = Phase::Index;
                Ok(ScalarTextStep::Number {
                    value: self.arguments.next().unwrap_or(Value::Undefined),
                    resume: self,
                })
            }
        }
    }
    fn concat(mut self) -> Result<ScalarTextStep, RuntimeError> {
        loop {
            match self.arguments.next() {
                None => {
                    return Ok(ScalarTextStep::Complete(Completion::Return(Value::String(
                        self.string,
                    ))));
                }
                Some(Value::String(chunk)) => {
                    self.string = self.string.try_concat(&chunk).map_err(Error::from)?
                }
                Some(value) => {
                    self.phase = Phase::Chunk;
                    return Ok(ScalarTextStep::String {
                        value,
                        resume: self,
                    });
                }
            }
        }
    }
    pub(crate) fn number(
        self,
        result: NativeConversion<f64>,
    ) -> Result<ScalarTextStep, RuntimeError> {
        if !matches!(self.phase, Phase::Index) {
            return Err(RuntimeError::Invariant(
                "String scalar index phase mismatch",
            ));
        }
        let number = match result {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => {
                return Ok(ScalarTextStep::Complete(Completion::Throw(value)));
            }
        };
        let mut index = crate::engine::value::number::to_int32_sat(number);
        let value =
            match self.kind {
                ScalarTextKind::CharAt(kind) => {
                    let length = i32::try_from(self.string.len()).map_err(|_| {
                        RuntimeError::Invariant("String length exceeded QuickJS signed index range")
                    })?;
                    if kind == StringCharAtKind::At && index < 0 {
                        index += length;
                    }
                    if index < 0 || index >= length {
                        match kind {
                            StringCharAtKind::At => Value::Undefined,
                            StringCharAtKind::CharAt => Value::String(JsString::from_static("")),
                        }
                    } else {
                        Value::String(JsString::from_code_unit(
                            self.string.code_unit_at(index as usize).ok_or(
                                RuntimeError::Invariant("validated String index missing code unit"),
                            )?,
                        ))
                    }
                }
                ScalarTextKind::CharCodeAt => usize::try_from(index)
                    .ok()
                    .and_then(|index| self.string.code_unit_at(index))
                    .map_or(Value::Float(f64::NAN), |unit| Value::Int(i32::from(unit))),
                ScalarTextKind::CodePointAt => usize::try_from(index)
                    .ok()
                    .and_then(|index| self.string.code_point_at(index))
                    .map_or(Value::Undefined, |point| Value::Int(point as i32)),
                _ => return Err(RuntimeError::Invariant("String scalar index kind mismatch")),
            };
        Ok(ScalarTextStep::Complete(Completion::Return(value)))
    }
}
pub(crate) fn finish(
    runtime: &Runtime,
    realm: ContextId,
    mut step: ScalarTextStep,
) -> Result<Completion, RuntimeError> {
    loop {
        step = match step {
            ScalarTextStep::Complete(result) => return Ok(result),
            ScalarTextStep::String { value, resume } => {
                resume.string(runtime, runtime.native_to_js_string(realm, &value)?)?
            }
            ScalarTextStep::Number { value, resume } => {
                resume.number(runtime.native_to_number(realm, &value)?)?
            }
        };
    }
}
