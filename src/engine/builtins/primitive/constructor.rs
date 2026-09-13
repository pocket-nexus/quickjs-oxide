//! Primitive constructors finish coercion before observing a supplied new.target prototype.
use crate::engine::{
    api::{runtime::Runtime, runtime_error::RuntimeError},
    builtins::native::PrimitiveKind,
    heap::ContextId,
    object::PropertyKey,
    value::{JsString, Value, conversion::NativeConversion},
    vm::{
        Completion,
        call::{NativeArguments, NativeInvocation},
    },
};
pub(crate) enum PrimitiveConstructorStep {
    Complete(Completion),
    Primitive {
        value: Value,
        resume: PrimitiveConstructorResume,
    },
    String {
        value: Value,
        resume: PrimitiveConstructorResume,
    },
    Read {
        receiver: Value,
        key: PropertyKey,
        resume: PrimitiveConstructorResume,
    },
}
enum Phase {
    Value,
    Prototype,
}
pub(crate) struct PrimitiveConstructorResume {
    realm: ContextId,
    kind: PrimitiveKind,
    new_target: Value,
    value: Value,
    phase: Phase,
}
impl PrimitiveConstructorStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        kind: PrimitiveKind,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Self, RuntimeError> {
        let argument = arguments
            .readable
            .first()
            .cloned()
            .ok_or(RuntimeError::Invariant(
                "primitive constructor argv was not padded",
            ))?;
        let NativeInvocation::Construct { new_target } = invocation else {
            return Err(RuntimeError::Invariant(
                "primitive constructor requires constructor-or-function invocation",
            ));
        };
        if matches!(kind, PrimitiveKind::Symbol | PrimitiveKind::BigInt)
            && !matches!(new_target, Value::Undefined)
        {
            return Ok(Self::Complete(Completion::Throw(
                runtime.new_not_constructor_error(realm, new_target)?,
            )));
        }
        let resume = PrimitiveConstructorResume {
            realm,
            kind,
            new_target: new_target.clone(),
            value: Value::Undefined,
            phase: Phase::Value,
        };
        match kind {
            PrimitiveKind::Boolean => {
                resume.converted(runtime, Value::Bool(runtime.value_to_boolean(&argument)?))
            }
            PrimitiveKind::Number if arguments.actual_arg_count == 0 => {
                resume.converted(runtime, Value::Int(0))
            }
            PrimitiveKind::String if arguments.actual_arg_count == 0 => {
                resume.converted(runtime, Value::String(JsString::from_static("")))
            }
            PrimitiveKind::Symbol if matches!(argument, Value::Undefined) => Ok(Self::Complete(
                Completion::Return(Value::Symbol(runtime.new_symbol(None)?)),
            )),
            PrimitiveKind::String
                if matches!(new_target, Value::Undefined)
                    && matches!(argument, Value::Symbol(_)) =>
            {
                let Value::Symbol(symbol) = argument else {
                    unreachable!()
                };
                resume.converted(
                    runtime,
                    Value::String(runtime.symbol_descriptive_string(&symbol)?),
                )
            }
            PrimitiveKind::String | PrimitiveKind::Symbol => Ok(Self::String {
                value: argument,
                resume,
            }),
            PrimitiveKind::Number | PrimitiveKind::BigInt => Ok(Self::Primitive {
                value: argument,
                resume,
            }),
        }
    }
}
impl PrimitiveConstructorResume {
    pub(crate) fn primitive(
        self,
        runtime: &Runtime,
        result: Completion,
    ) -> Result<PrimitiveConstructorStep, RuntimeError> {
        if !matches!(self.phase, Phase::Value) {
            return Err(RuntimeError::Invariant(
                "primitive constructor coercion phase mismatch",
            ));
        }
        let value = match result {
            Completion::Return(value) => value,
            Completion::Throw(value) => {
                return Ok(PrimitiveConstructorStep::Complete(Completion::Throw(value)));
            }
        };
        let value = match self.kind {
            PrimitiveKind::Number => {
                match runtime.number_constructor_from_primitive(self.realm, &value)? {
                    NativeConversion::Value(value) => Value::number(value),
                    NativeConversion::Throw(value) => {
                        return Ok(PrimitiveConstructorStep::Complete(Completion::Throw(value)));
                    }
                }
            }
            PrimitiveKind::BigInt => {
                match runtime.bigint_constructor_from_primitive(self.realm, &value)? {
                    NativeConversion::Value(value) => Value::BigInt(value),
                    NativeConversion::Throw(value) => {
                        return Ok(PrimitiveConstructorStep::Complete(Completion::Throw(value)));
                    }
                }
            }
            _ => {
                return Err(RuntimeError::Invariant(
                    "primitive constructor coercion kind mismatch",
                ));
            }
        };
        self.converted(runtime, value)
    }
    pub(crate) fn string(
        self,
        runtime: &Runtime,
        result: NativeConversion<JsString>,
    ) -> Result<PrimitiveConstructorStep, RuntimeError> {
        if !matches!(self.phase, Phase::Value) {
            return Err(RuntimeError::Invariant(
                "primitive constructor string phase mismatch",
            ));
        }
        let value = match result {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => {
                return Ok(PrimitiveConstructorStep::Complete(Completion::Throw(value)));
            }
        };
        if self.kind == PrimitiveKind::Symbol {
            return Ok(PrimitiveConstructorStep::Complete(Completion::Return(
                Value::Symbol(runtime.new_symbol(Some(value))?),
            )));
        }
        self.converted(runtime, Value::String(value))
    }
    fn converted(
        mut self,
        runtime: &Runtime,
        value: Value,
    ) -> Result<PrimitiveConstructorStep, RuntimeError> {
        if matches!(self.new_target, Value::Undefined) {
            return Ok(PrimitiveConstructorStep::Complete(Completion::Return(
                value,
            )));
        }
        self.value = value;
        self.phase = Phase::Prototype;
        Ok(PrimitiveConstructorStep::Read {
            receiver: self.new_target.clone(),
            key: runtime.intern_property_key("prototype")?,
            resume: self,
        })
    }
    pub(crate) fn resume(
        self,
        runtime: &Runtime,
        result: Completion,
    ) -> Result<PrimitiveConstructorStep, RuntimeError> {
        if !matches!(self.phase, Phase::Prototype) {
            return Err(RuntimeError::Invariant(
                "primitive constructor prototype phase mismatch",
            ));
        }
        let prototype = match result {
            Completion::Return(Value::Object(object)) => object,
            Completion::Throw(value) => {
                return Ok(PrimitiveConstructorStep::Complete(Completion::Throw(value)));
            }
            Completion::Return(_) => {
                let realm = match runtime.function_realm_from_value(self.realm, &self.new_target)? {
                    NativeConversion::Value(realm) => realm,
                    NativeConversion::Throw(value) => {
                        return Ok(PrimitiveConstructorStep::Complete(Completion::Throw(value)));
                    }
                };
                runtime.primitive_prototype_for_realm(realm, self.kind)?
            }
        };
        Ok(PrimitiveConstructorStep::Complete(Completion::Return(
            Value::Object(runtime.new_primitive_object(&prototype, self.kind, self.value)?),
        )))
    }
}
pub(crate) fn finish(
    runtime: &Runtime,
    realm: ContextId,
    mut step: PrimitiveConstructorStep,
) -> Result<Completion, RuntimeError> {
    loop {
        step = match step {
            PrimitiveConstructorStep::Complete(result) => return Ok(result),
            PrimitiveConstructorStep::Primitive { value, resume } => resume.primitive(
                runtime,
                runtime.to_primitive(realm, value, crate::engine::vm::ToPrimitiveHint::Number)?,
            )?,
            PrimitiveConstructorStep::String { value, resume } => {
                resume.string(runtime, runtime.native_to_js_string(realm, &value)?)?
            }
            PrimitiveConstructorStep::Read {
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
