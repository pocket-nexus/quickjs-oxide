//! Object string conversions retain tag fallback and method lookup across callbacks.
#[cfg(feature = "stack-vm")]
use crate::engine::builtins::native::NativeFunctionId;
use crate::engine::{
    api::{error::NativeErrorKind, runtime::Runtime, runtime_error::RuntimeError},
    heap::ContextId,
    object::{PropertyKey, WellKnownSymbol},
    value::{JsString, Value, conversion::NativeConversion},
    vm::{
        Completion,
        call::{DirectCallTarget, NativeInvocation},
    },
};
#[derive(Clone, Copy)]
pub(crate) enum ObjectStringKind {
    Tag,
    Locale,
}
impl ObjectStringKind {
    #[cfg(feature = "stack-vm")]
    pub(crate) fn for_target(target: NativeFunctionId) -> Option<Self> {
        Some(match target {
            NativeFunctionId::ObjectPrototypeToString => Self::Tag,
            NativeFunctionId::ObjectPrototypeToLocaleString => Self::Locale,
            _ => return None,
        })
    }
}
pub(crate) enum ObjectStringStep {
    Complete(Completion),
    Read {
        receiver: Value,
        key: PropertyKey,
        resume: ObjectStringResume,
    },
    Call {
        target: DirectCallTarget,
        receiver: Value,
        resume: ObjectStringResume,
    },
}
pub(crate) struct ObjectStringResume {
    realm: ContextId,
    receiver: Value,
    phase: Phase,
}
enum Phase {
    Tag(JsString),
    LocaleMethod,
    LocaleResult,
}
fn tag_string(tag: JsString) -> Result<ObjectStringStep, RuntimeError> {
    let value = JsString::from_static("[object ")
        .try_concat(&tag)?
        .try_concat(&JsString::from_static("]"))?;
    Ok(ObjectStringStep::Complete(Completion::Return(
        Value::String(value),
    )))
}
impl ObjectStringStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        kind: ObjectStringKind,
        invocation: &NativeInvocation,
    ) -> Result<Self, RuntimeError> {
        let NativeInvocation::Call { this_value } = invocation else {
            return Err(RuntimeError::Invariant(
                "Object string conversion did not receive a call",
            ));
        };
        match kind {
            ObjectStringKind::Tag => {
                match this_value {
                    Value::Undefined => return tag_string(JsString::from_static("Undefined")),
                    Value::Null => return tag_string(JsString::from_static("Null")),
                    _ => {}
                }
                let object = match runtime.native_to_object(realm, this_value.clone())? {
                    NativeConversion::Value(object) => object,
                    NativeConversion::Throw(value) => {
                        return Ok(Self::Complete(Completion::Throw(value)));
                    }
                };
                let tag = match runtime.object_default_to_string_tag(realm, &object)? {
                    NativeConversion::Value(tag) => tag,
                    NativeConversion::Throw(value) => {
                        return Ok(Self::Complete(Completion::Throw(value)));
                    }
                };
                let receiver = Value::Object(object);
                Ok(Self::Read {
                    receiver: receiver.clone(),
                    key: PropertyKey::from(runtime.well_known_symbol(WellKnownSymbol::ToStringTag)),
                    resume: ObjectStringResume {
                        realm,
                        receiver,
                        phase: Phase::Tag(tag),
                    },
                })
            }
            ObjectStringKind::Locale => {
                if matches!(this_value, Value::Null | Value::Undefined) {
                    let message = if matches!(this_value, Value::Null) {
                        "cannot read property 'toString' of null"
                    } else {
                        "cannot read property 'toString' of undefined"
                    };
                    return Ok(Self::Complete(Completion::Throw(
                        runtime.new_native_error(realm, NativeErrorKind::Type, message)?,
                    )));
                }
                Ok(Self::Read {
                    receiver: this_value.clone(),
                    key: runtime.intern_property_key("toString")?,
                    resume: ObjectStringResume {
                        realm,
                        receiver: this_value.clone(),
                        phase: Phase::LocaleMethod,
                    },
                })
            }
        }
    }
}
impl ObjectStringResume {
    pub(crate) fn resume(
        self,
        runtime: &Runtime,
        result: Completion,
    ) -> Result<ObjectStringStep, RuntimeError> {
        let value = match result {
            Completion::Return(value) => value,
            Completion::Throw(value) => {
                return Ok(ObjectStringStep::Complete(Completion::Throw(value)));
            }
        };
        match self.phase {
            Phase::Tag(default_tag) => tag_string(match value {
                Value::String(tag) => tag,
                _ => default_tag,
            }),
            Phase::LocaleResult => Ok(ObjectStringStep::Complete(Completion::Return(value))),
            Phase::LocaleMethod => {
                let callable = match value {
                    Value::Object(object) => runtime.as_callable(&object)?,
                    _ => None,
                };
                let Some(callable) = callable else {
                    return Ok(ObjectStringStep::Complete(Completion::Throw(
                        runtime.new_native_error(
                            self.realm,
                            NativeErrorKind::Type,
                            "not a function",
                        )?,
                    )));
                };
                Ok(ObjectStringStep::Call {
                    target: DirectCallTarget::Callable(callable),
                    receiver: self.receiver.clone(),
                    resume: Self {
                        phase: Phase::LocaleResult,
                        ..self
                    },
                })
            }
        }
    }
}
pub(super) fn finish(
    runtime: &Runtime,
    realm: ContextId,
    mut step: ObjectStringStep,
) -> Result<Completion, RuntimeError> {
    loop {
        step = match step {
            ObjectStringStep::Complete(result) => return Ok(result),
            ObjectStringStep::Read {
                receiver,
                key,
                resume,
            } => resume.resume(
                runtime,
                runtime.get_value_property_in_realm(realm, receiver, &key)?,
            )?,
            ObjectStringStep::Call {
                target,
                receiver,
                resume,
            } => {
                let result = match target {
                    DirectCallTarget::Callable(callable) => {
                        runtime.call_internal(realm, &callable, receiver, &[])?
                    }
                    DirectCallTarget::NonCallableProxy(proxy) => {
                        runtime.call_proxy(realm, &proxy, receiver, &[])?
                    }
                };
                resume.resume(runtime, result)?
            }
        };
    }
}
