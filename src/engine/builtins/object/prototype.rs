//! Shared Object/Reflect prototype algorithms, returning internal-method requests.
#[cfg(feature = "stack-vm")]
use crate::engine::builtins::native::{NativeFunctionId, ReflectKind};
use crate::engine::{
    api::{error::NativeErrorKind, runtime::Runtime, runtime_error::RuntimeError},
    heap::ContextId,
    object::ObjectRef,
    value::{Value, conversion::NativeConversion},
    vm::{Completion, call::NativeArguments},
};

#[derive(Clone, Copy)]
pub(crate) enum BuiltinPrototypeKind {
    ObjectGet,
    ObjectSet,
    ReflectGet,
    ReflectSet,
}
impl BuiltinPrototypeKind {
    #[cfg(feature = "stack-vm")]
    pub(crate) fn for_target(target: NativeFunctionId) -> Option<Self> {
        Some(match target {
            NativeFunctionId::ObjectGetPrototypeOf => Self::ObjectGet,
            NativeFunctionId::ObjectSetPrototypeOf => Self::ObjectSet,
            NativeFunctionId::Reflect(ReflectKind::GetPrototypeOf) => Self::ReflectGet,
            NativeFunctionId::Reflect(ReflectKind::SetPrototypeOf) => Self::ReflectSet,
            _ => return None,
        })
    }
}
pub(crate) enum BuiltinPrototypeStep {
    Complete(Completion),
    Get {
        object: ObjectRef,
        resume: BuiltinPrototypeResume,
    },
    Set {
        object: ObjectRef,
        prototype: Option<ObjectRef>,
        resume: BuiltinPrototypeResume,
    },
}
pub(crate) struct BuiltinPrototypeResume {
    object: ObjectRef,
    realm: ContextId,
    kind: BuiltinPrototypeKind,
}
impl BuiltinPrototypeStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        kind: BuiltinPrototypeKind,
        arguments: &NativeArguments,
    ) -> Result<Self, RuntimeError> {
        let target = arguments
            .readable
            .first()
            .ok_or(RuntimeError::Invariant(match kind {
                BuiltinPrototypeKind::ObjectGet => "Object.getPrototypeOf argv was not padded",
                BuiltinPrototypeKind::ObjectSet => {
                    "Object.setPrototypeOf target argv was not padded"
                }
                _ => "Reflect target argv was not padded",
            }))?;
        if matches!(target, Value::Null | Value::Undefined)
            || (matches!(
                kind,
                BuiltinPrototypeKind::ReflectGet | BuiltinPrototypeKind::ReflectSet
            ) && !matches!(target, Value::Object(_)))
        {
            return not_object(runtime, realm);
        }
        if matches!(
            kind,
            BuiltinPrototypeKind::ObjectGet | BuiltinPrototypeKind::ReflectGet
        ) {
            let object = match runtime.native_to_object(realm, target.clone())? {
                NativeConversion::Value(object) => object,
                NativeConversion::Throw(value) => {
                    return Ok(Self::Complete(Completion::Throw(value)));
                }
            };
            return Ok(Self::Get {
                object: object.clone(),
                resume: BuiltinPrototypeResume {
                    object,
                    realm,
                    kind,
                },
            });
        }
        let prototype = match arguments.readable.get(1).ok_or(RuntimeError::Invariant(
            "Object.setPrototypeOf prototype argv was not padded",
        ))? {
            Value::Object(object) => Some(object.clone()),
            Value::Null => None,
            _ => return not_object(runtime, realm),
        };
        let Value::Object(object) = target else {
            return Ok(Self::Complete(Completion::Return(target.clone())));
        };
        Ok(Self::Set {
            object: object.clone(),
            prototype,
            resume: BuiltinPrototypeResume {
                object: object.clone(),
                realm,
                kind,
            },
        })
    }
}
fn not_object(runtime: &Runtime, realm: ContextId) -> Result<BuiltinPrototypeStep, RuntimeError> {
    Ok(BuiltinPrototypeStep::Complete(Completion::Throw(
        runtime.new_native_error(realm, NativeErrorKind::Type, "not an object")?,
    )))
}
impl BuiltinPrototypeResume {
    pub(crate) fn prototype(
        self,
        result: NativeConversion<Option<ObjectRef>>,
    ) -> Result<BuiltinPrototypeStep, RuntimeError> {
        if !matches!(
            self.kind,
            BuiltinPrototypeKind::ObjectGet | BuiltinPrototypeKind::ReflectGet
        ) {
            return Err(RuntimeError::Invariant(
                "prototype builtin received a Get reply for Set",
            ));
        }
        Ok(BuiltinPrototypeStep::Complete(match result {
            NativeConversion::Value(prototype) => {
                Completion::Return(prototype.map_or(Value::Null, Value::Object))
            }
            NativeConversion::Throw(value) => Completion::Throw(value),
        }))
    }
    pub(crate) fn boolean(
        self,
        runtime: &Runtime,
        result: NativeConversion<bool>,
    ) -> Result<BuiltinPrototypeStep, RuntimeError> {
        Ok(BuiltinPrototypeStep::Complete(match self.kind {
            BuiltinPrototypeKind::ReflectSet => match result {
                NativeConversion::Value(value) => Completion::Return(Value::Bool(value)),
                NativeConversion::Throw(value) => Completion::Throw(value),
            },
            BuiltinPrototypeKind::ObjectSet => {
                match runtime.finish_set_prototype_or_throw(self.realm, &self.object, result)? {
                    Some(value) => Completion::Throw(value),
                    None => Completion::Return(Value::Object(self.object)),
                }
            }
            _ => {
                return Err(RuntimeError::Invariant(
                    "prototype builtin received a Set reply for Get",
                ));
            }
        }))
    }
}
pub(in crate::engine::builtins) fn finish(
    runtime: &Runtime,
    realm: ContextId,
    mut step: BuiltinPrototypeStep,
) -> Result<Completion, RuntimeError> {
    loop {
        step = match step {
            BuiltinPrototypeStep::Complete(result) => return Ok(result),
            BuiltinPrototypeStep::Get { object, resume } => {
                resume.prototype(runtime.internal_get_prototype_of(realm, &object)?)?
            }
            BuiltinPrototypeStep::Set {
                object,
                prototype,
                resume,
            } => resume.boolean(
                runtime,
                runtime.internal_set_prototype_of(realm, &object, prototype.as_ref())?,
            )?,
        }
    }
}
