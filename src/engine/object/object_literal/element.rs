//! Computed literal definitions convert the retained key once before own definition.
use crate::engine::{
    api::{Error, ErrorKind, runtime::Runtime, runtime_error::RuntimeError},
    heap::ContextId,
    object::{
        ObjectRef, OrdinaryPropertyDescriptor, PropertyKey,
        operations::{InternalDefineResult, PropertyDefineOutcome},
    },
    value::{Value, conversion::NativeConversion},
    vm::{Completion, ToPrimitiveHint},
};

pub(crate) enum LiteralDefinitionStep {
    Complete(Completion),
    Primitive {
        value: Value,
        resume: LiteralDefinitionResume,
    },
    Define {
        object: ObjectRef,
        key: PropertyKey,
        descriptor: OrdinaryPropertyDescriptor,
        resume: LiteralDefinitionResume,
    },
}
pub(crate) enum LiteralDefinitionResume {
    Key {
        realm: ContextId,
        object: ObjectRef,
        value: Value,
    },
    Defined,
}
impl LiteralDefinitionStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        object: ObjectRef,
        key: Value,
        value: Value,
    ) -> Result<Self, RuntimeError> {
        let resume = LiteralDefinitionResume::Key {
            realm,
            object,
            value,
        };
        if matches!(key, Value::Object(_)) {
            Ok(Self::Primitive { value: key, resume })
        } else {
            resume.resume(runtime, Completion::Return(key))
        }
    }
}
impl LiteralDefinitionResume {
    pub(crate) fn resume(
        self,
        runtime: &Runtime,
        result: Completion,
    ) -> Result<LiteralDefinitionStep, RuntimeError> {
        let key = match result {
            Completion::Return(value) => value,
            Completion::Throw(value) => {
                return Ok(LiteralDefinitionStep::Complete(Completion::Throw(value)));
            }
        };
        let Self::Key {
            realm,
            object,
            value,
        } = self
        else {
            return Err(RuntimeError::Invariant(
                "literal definition lost its key conversion owner",
            ));
        };
        let key = match runtime.property_key_from_primitive(realm, key)? {
            NativeConversion::Value(key) => key,
            NativeConversion::Throw(value) => {
                return Ok(LiteralDefinitionStep::Complete(Completion::Throw(value)));
            }
        };
        Ok(LiteralDefinitionStep::Define {
            object,
            key,
            descriptor: Runtime::public_class_field_descriptor(value),
            resume: Self::Defined,
        })
    }
    pub(crate) fn defined(
        self,
        result: NativeConversion<InternalDefineResult>,
    ) -> Result<LiteralDefinitionStep, RuntimeError> {
        if !matches!(self, Self::Defined) {
            return Err(RuntimeError::Invariant(
                "literal definition reply has wrong owner",
            ));
        }
        Ok(LiteralDefinitionStep::Complete(match result {
            NativeConversion::Value(InternalDefineResult::Defined) => {
                Completion::Return(Value::Undefined)
            }
            NativeConversion::Throw(value) => Completion::Throw(value),
            NativeConversion::Value(InternalDefineResult::RejectedOrdinary(_)) => {
                return Err(Error::new(ErrorKind::Type, "property is not configurable").into());
            }
            NativeConversion::Value(InternalDefineResult::RejectedProxyTrap) => {
                return Err(RuntimeError::Invariant(
                    "literal own definition unexpectedly entered Proxy trap",
                ));
            }
        }))
    }
}
pub(crate) fn finish(
    runtime: &Runtime,
    realm: ContextId,
    mut step: LiteralDefinitionStep,
) -> Result<Completion, RuntimeError> {
    loop {
        step = match step {
            LiteralDefinitionStep::Complete(result) => return Ok(result),
            LiteralDefinitionStep::Primitive { value, resume } => resume.resume(
                runtime,
                runtime.to_primitive(realm, value, ToPrimitiveHint::String)?,
            )?,
            LiteralDefinitionStep::Define {
                object,
                key,
                descriptor,
                resume,
            } => {
                let result = match runtime.define_own_property_in_realm(
                    Some(realm),
                    &object,
                    &key,
                    &descriptor,
                )? {
                    PropertyDefineOutcome::Defined(true) => {
                        NativeConversion::Value(InternalDefineResult::Defined)
                    }
                    PropertyDefineOutcome::Defined(false) => {
                        NativeConversion::Value(InternalDefineResult::RejectedOrdinary(object))
                    }
                    PropertyDefineOutcome::Throw(value) => NativeConversion::Throw(value),
                };
                resume.defined(result)?
            }
        };
    }
}
