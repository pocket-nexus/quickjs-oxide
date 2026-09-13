//! Reverse observes both indexed properties before performing either mutation.
use crate::engine::{
    api::{error::NativeErrorKind, runtime::Runtime, runtime_error::RuntimeError},
    heap::ContextId,
    object::{ObjectRef, PropertyKey, operations::InternalSetResult},
    value::{Value, conversion::NativeConversion},
    vm::{Completion, call::NativeInvocation},
};
pub(crate) enum ReverseStep {
    Complete(Completion),
    Read {
        object: ObjectRef,
        key: PropertyKey,
        resume: ReverseResume,
    },
    Number {
        value: Value,
        resume: ReverseResume,
    },
    Has {
        object: ObjectRef,
        key: PropertyKey,
        resume: ReverseResume,
    },
    Set {
        object: ObjectRef,
        key: PropertyKey,
        value: Value,
        resume: ReverseResume,
    },
    Delete {
        object: ObjectRef,
        key: PropertyKey,
        resume: ReverseResume,
    },
}
enum Phase {
    Length,
    Number,
    LowerHas,
    LowerRead,
    UpperHas,
    UpperRead,
    LowerWrite,
    UpperWrite,
}
pub(crate) struct ReverseResume {
    realm: ContextId,
    object: ObjectRef,
    phase: Phase,
    lower: u64,
    upper: u64,
    lower_value: Option<Value>,
    upper_value: Option<Value>,
}
impl ReverseStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        invocation: &NativeInvocation,
    ) -> Result<Self, RuntimeError> {
        let NativeInvocation::Call { this_value } = invocation else {
            return Err(RuntimeError::Invariant(
                "Array reverse requires generic invocation",
            ));
        };
        let object = match runtime.native_to_object(realm, this_value.clone())? {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => return Ok(Self::Complete(Completion::Throw(value))),
        };
        Ok(Self::Read {
            object: object.clone(),
            key: runtime.intern_property_key("length")?,
            resume: ReverseResume {
                realm,
                object,
                phase: Phase::Length,
                lower: 0,
                upper: 0,
                lower_value: None,
                upper_value: None,
            },
        })
    }
}
impl ReverseResume {
    pub(crate) fn resume(
        mut self,
        runtime: &Runtime,
        result: Completion,
    ) -> Result<ReverseStep, RuntimeError> {
        let value = match result {
            Completion::Return(value) => value,
            Completion::Throw(value) => return Ok(ReverseStep::Complete(Completion::Throw(value))),
        };
        match self.phase {
            Phase::Length => {
                self.phase = Phase::Number;
                Ok(ReverseStep::Number {
                    value,
                    resume: self,
                })
            }
            Phase::LowerRead => {
                self.lower_value = Some(value);
                self.upper(runtime)
            }
            Phase::UpperRead => {
                self.upper_value = Some(value);
                self.write_lower(runtime)
            }
            _ => Err(RuntimeError::Invariant(
                "Array reverse value phase mismatch",
            )),
        }
    }
    pub(crate) fn number(
        mut self,
        runtime: &Runtime,
        result: NativeConversion<f64>,
    ) -> Result<ReverseStep, RuntimeError> {
        if !matches!(self.phase, Phase::Number) {
            return Err(RuntimeError::Invariant(
                "Array reverse number phase mismatch",
            ));
        }
        self.upper = match result {
            NativeConversion::Value(value) => Runtime::length_from_number(value).saturating_sub(1),
            NativeConversion::Throw(value) => {
                return Ok(ReverseStep::Complete(Completion::Throw(value)));
            }
        };
        self.next(runtime)
    }
    fn next(mut self, runtime: &Runtime) -> Result<ReverseStep, RuntimeError> {
        if self.lower >= self.upper {
            return Ok(ReverseStep::Complete(Completion::Return(Value::Object(
                self.object,
            ))));
        }
        self.phase = Phase::LowerHas;
        self.lower_value = None;
        self.upper_value = None;
        Ok(ReverseStep::Has {
            object: self.object.clone(),
            key: runtime.property_key_for_index(self.lower)?,
            resume: self,
        })
    }
    fn upper(mut self, runtime: &Runtime) -> Result<ReverseStep, RuntimeError> {
        self.phase = Phase::UpperHas;
        Ok(ReverseStep::Has {
            object: self.object.clone(),
            key: runtime.property_key_for_index(self.upper)?,
            resume: self,
        })
    }
    fn write_lower(mut self, runtime: &Runtime) -> Result<ReverseStep, RuntimeError> {
        self.phase = Phase::LowerWrite;
        if let Some(value) = self.upper_value.take() {
            Ok(ReverseStep::Set {
                object: self.object.clone(),
                key: runtime.property_key_for_index(self.lower)?,
                value,
                resume: self,
            })
        } else if self.lower_value.is_some() {
            Ok(ReverseStep::Delete {
                object: self.object.clone(),
                key: runtime.property_key_for_index(self.lower)?,
                resume: self,
            })
        } else {
            self.advance(runtime)
        }
    }
    fn write_upper(mut self, runtime: &Runtime) -> Result<ReverseStep, RuntimeError> {
        self.phase = Phase::UpperWrite;
        if let Some(value) = self.lower_value.take() {
            Ok(ReverseStep::Set {
                object: self.object.clone(),
                key: runtime.property_key_for_index(self.upper)?,
                value,
                resume: self,
            })
        } else {
            Ok(ReverseStep::Delete {
                object: self.object.clone(),
                key: runtime.property_key_for_index(self.upper)?,
                resume: self,
            })
        }
    }
    fn advance(mut self, runtime: &Runtime) -> Result<ReverseStep, RuntimeError> {
        self.lower += 1;
        self.upper -= 1;
        self.next(runtime)
    }
    pub(crate) fn boolean(
        mut self,
        runtime: &Runtime,
        result: NativeConversion<bool>,
    ) -> Result<ReverseStep, RuntimeError> {
        let value = match result {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => {
                return Ok(ReverseStep::Complete(Completion::Throw(value)));
            }
        };
        match self.phase {
            Phase::LowerHas | Phase::UpperHas => {
                let lower = matches!(self.phase, Phase::LowerHas);
                if value {
                    self.phase = if lower {
                        Phase::LowerRead
                    } else {
                        Phase::UpperRead
                    };
                    Ok(ReverseStep::Read {
                        object: self.object.clone(),
                        key: runtime.property_key_for_index(if lower {
                            self.lower
                        } else {
                            self.upper
                        })?,
                        resume: self,
                    })
                } else if lower {
                    self.upper(runtime)
                } else {
                    self.write_lower(runtime)
                }
            }
            Phase::LowerWrite | Phase::UpperWrite => {
                if !value {
                    return Ok(ReverseStep::Complete(Completion::Throw(
                        runtime.new_native_error(
                            self.realm,
                            NativeErrorKind::Type,
                            "could not delete property",
                        )?,
                    )));
                }
                if matches!(self.phase, Phase::LowerWrite) {
                    self.write_upper(runtime)
                } else {
                    self.advance(runtime)
                }
            }
            _ => Err(RuntimeError::Invariant(
                "Array reverse boolean phase mismatch",
            )),
        }
    }
    pub(crate) fn set(
        self,
        runtime: &Runtime,
        key: PropertyKey,
        result: NativeConversion<InternalSetResult>,
    ) -> Result<ReverseStep, RuntimeError> {
        if let Some(value) = runtime.finish_set_property_or_throw(self.realm, &key, result)? {
            return Ok(ReverseStep::Complete(Completion::Throw(value)));
        }
        match self.phase {
            Phase::LowerWrite => self.write_upper(runtime),
            Phase::UpperWrite => self.advance(runtime),
            _ => Err(RuntimeError::Invariant("Array reverse set phase mismatch")),
        }
    }
}
pub(crate) fn finish(
    runtime: &Runtime,
    realm: ContextId,
    mut step: ReverseStep,
) -> Result<Completion, RuntimeError> {
    loop {
        step = match step {
            ReverseStep::Complete(result) => return Ok(result),
            ReverseStep::Read {
                object,
                key,
                resume,
            } => resume.resume(
                runtime,
                runtime.get_property_in_realm(realm, &object, &key)?,
            )?,
            ReverseStep::Number { value, resume } => {
                resume.number(runtime, runtime.native_to_number(realm, &value)?)?
            }
            ReverseStep::Has {
                object,
                key,
                resume,
            } => resume.boolean(
                runtime,
                runtime.internal_has_property(realm, &object, &key)?,
            )?,
            ReverseStep::Set {
                object,
                key,
                value,
                resume,
            } => {
                let result = runtime.internal_set(
                    realm,
                    &object,
                    &key,
                    value,
                    Value::Object(object.clone()),
                )?;
                resume.set(runtime, key, result)?
            }
            ReverseStep::Delete {
                object,
                key,
                resume,
            } => resume.boolean(
                runtime,
                runtime.internal_delete_property(realm, &object, &key)?,
            )?,
        };
    }
}
