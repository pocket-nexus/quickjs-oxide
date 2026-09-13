//! Owned ToPrimitive phases. A reply consumes its continuation exactly once.
use super::*;
use crate::engine::object::CallableRef;

pub(crate) enum PrimitiveStep {
    Get {
        object: ObjectRef,
        key: PropertyKey,
        resume: PrimitiveResume,
    },
    Call {
        callable: CallableRef,
        receiver: Value,
        arguments: Vec<Value>,
        resume: PrimitiveResume,
    },
    Complete(Completion),
}

pub(crate) struct PrimitiveResume {
    object: ObjectRef,
    realm: ContextId,
    hint: ToPrimitiveHint,
    phase: Phase,
}

enum Phase {
    ExoticMethod,
    ExoticResult,
    OrdinaryMethod(bool),
    OrdinaryResult(bool),
}

impl PrimitiveResume {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        value: Value,
        hint: ToPrimitiveHint,
    ) -> PrimitiveStep {
        let Value::Object(object) = value else {
            return PrimitiveStep::Complete(Completion::Return(value));
        };
        let key = PropertyKey::from(runtime.well_known_symbol(WellKnownSymbol::ToPrimitive));
        PrimitiveStep::Get {
            object: object.clone(),
            key,
            resume: Self {
                object,
                realm,
                hint,
                phase: Phase::ExoticMethod,
            },
        }
    }

    pub(crate) fn ordinary(
        runtime: &Runtime,
        realm: ContextId,
        object: ObjectRef,
        hint: ToPrimitiveHint,
    ) -> Result<PrimitiveStep, RuntimeError> {
        Self {
            object,
            realm,
            hint,
            phase: Phase::OrdinaryMethod(false),
        }
        .read_ordinary(runtime, false)
    }

    fn read_ordinary(
        mut self,
        runtime: &Runtime,
        second: bool,
    ) -> Result<PrimitiveStep, RuntimeError> {
        let string_first = matches!(self.hint, ToPrimitiveHint::String);
        let name = if string_first != second {
            "toString"
        } else {
            "valueOf"
        };
        let key = runtime.intern_property_key(name)?;
        self.phase = Phase::OrdinaryMethod(second);
        Ok(PrimitiveStep::Get {
            object: self.object.clone(),
            key,
            resume: self,
        })
    }

    fn failed_method(self, runtime: &Runtime, second: bool) -> Result<PrimitiveStep, RuntimeError> {
        if second {
            self.type_error(runtime, "toPrimitive")
        } else {
            self.read_ordinary(runtime, true)
        }
    }

    fn type_error(self, runtime: &Runtime, message: &str) -> Result<PrimitiveStep, RuntimeError> {
        Ok(PrimitiveStep::Complete(Completion::Throw(
            runtime.new_native_error(self.realm, NativeErrorKind::Type, message)?,
        )))
    }

    pub(crate) fn resume(
        mut self,
        runtime: &Runtime,
        completion: Completion,
    ) -> Result<PrimitiveStep, RuntimeError> {
        let value = match completion {
            Completion::Throw(value) => {
                return Ok(PrimitiveStep::Complete(Completion::Throw(value)));
            }
            Completion::Return(value) => value,
        };
        match self.phase {
            Phase::ExoticMethod => {
                if matches!(value, Value::Undefined | Value::Null) {
                    return self.read_ordinary(runtime, false);
                }
                let Value::Object(method) = value else {
                    return self.type_error(runtime, "not a function");
                };
                let Some(callable) = runtime.as_callable(&method)? else {
                    return self.type_error(runtime, "not a function");
                };
                let argument = Value::String(JsString::from_static(match self.hint {
                    ToPrimitiveHint::String => "string",
                    ToPrimitiveHint::Number => "number",
                    ToPrimitiveHint::Default => "default",
                }));
                self.phase = Phase::ExoticResult;
                Ok(PrimitiveStep::Call {
                    callable,
                    receiver: Value::Object(self.object.clone()),
                    arguments: vec![argument],
                    resume: self,
                })
            }
            Phase::ExoticResult => {
                if matches!(value, Value::Object(_)) {
                    self.type_error(runtime, "toPrimitive")
                } else {
                    Ok(PrimitiveStep::Complete(Completion::Return(value)))
                }
            }
            Phase::OrdinaryMethod(second) => {
                let Value::Object(method) = value else {
                    return self.failed_method(runtime, second);
                };
                let Some(callable) = runtime.as_callable(&method)? else {
                    return self.failed_method(runtime, second);
                };
                self.phase = Phase::OrdinaryResult(second);
                Ok(PrimitiveStep::Call {
                    callable,
                    receiver: Value::Object(self.object.clone()),
                    arguments: Vec::new(),
                    resume: self,
                })
            }
            Phase::OrdinaryResult(second) => {
                if matches!(value, Value::Object(_)) {
                    self.failed_method(runtime, second)
                } else {
                    Ok(PrimitiveStep::Complete(Completion::Return(value)))
                }
            }
        }
    }
}

impl Runtime {
    /// Transitional synchronous consumer. The domain steps themselves never
    /// invoke JavaScript; the explicit driver can own the same continuations.
    pub(super) fn finish_primitive_steps(
        &self,
        realm: ContextId,
        mut step: PrimitiveStep,
    ) -> Result<Completion, RuntimeError> {
        loop {
            step = match step {
                PrimitiveStep::Complete(completion) => return Ok(completion),
                PrimitiveStep::Get {
                    object,
                    key,
                    resume,
                } => {
                    let completion = self.get_property_in_realm(realm, &object, &key)?;
                    resume.resume(self, completion)?
                }
                PrimitiveStep::Call {
                    callable,
                    receiver,
                    arguments,
                    resume,
                } => {
                    let completion = self.call_internal(realm, &callable, receiver, &arguments)?;
                    resume.resume(self, completion)?
                }
            };
        }
    }

    pub(crate) fn ordinary_to_primitive(
        &self,
        realm: ContextId,
        object: &ObjectRef,
        hint: ToPrimitiveHint,
    ) -> Result<Completion, RuntimeError> {
        let step = PrimitiveResume::ordinary(self, realm, object.clone(), hint)?;
        self.finish_primitive_steps(realm, step)
    }
}
