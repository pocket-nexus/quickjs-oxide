//! Shared Object/Reflect prototype algorithms, returning internal-method requests.

use crate::engine::builtins::native::{NativeFunctionId, ReflectKind};
use crate::engine::{
    api::{error::NativeErrorKind, runtime::Runtime, runtime_error::RuntimeError},
    heap::ContextId,
    object::ObjectRef,
    value::{JsValue, conversion::NativeConversion},
    vm::{
        Completion,
        call::{NativeArguments, NativeInvocation},
    },
};

#[derive(Clone, Copy)]
pub(crate) enum BuiltinPrototypeKind {
    Getter,
    Setter,
    IsPrototype,
    ObjectGet,
    ObjectSet,
    ReflectGet,
    ReflectSet,
}
impl BuiltinPrototypeKind {
    pub(crate) fn for_target(target: NativeFunctionId) -> Option<Self> {
        Some(match target {
            NativeFunctionId::ObjectPrototypeProtoGetter => Self::Getter,
            NativeFunctionId::ObjectPrototypeProtoSetter => Self::Setter,
            NativeFunctionId::ObjectPrototypeIsPrototypeOf => Self::IsPrototype,
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
pub(crate) struct BuiltinPrototypeResume(Box<BuiltinPrototypeResumeState>);
impl std::ops::Deref for BuiltinPrototypeResume {
    type Target = BuiltinPrototypeResumeState;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl std::ops::DerefMut for BuiltinPrototypeResume {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
const _: () = assert!(std::mem::size_of::<BuiltinPrototypeResume>() <= 8);
pub(crate) struct BuiltinPrototypeResumeState {
    object: ObjectRef,
    realm: ContextId,
    kind: BuiltinPrototypeKind,
}
impl BuiltinPrototypeStep {
    pub(crate) fn start_invocation(
        runtime: &Runtime,
        realm: ContextId,
        kind: BuiltinPrototypeKind,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Self, RuntimeError> {
        let receiver = match (kind, invocation) {
            (BuiltinPrototypeKind::Getter, NativeInvocation::Getter { this_value })
            | (BuiltinPrototypeKind::Setter, NativeInvocation::Setter { this_value })
            | (BuiltinPrototypeKind::IsPrototype, NativeInvocation::Call { this_value }) => {
                this_value
            }
            (
                BuiltinPrototypeKind::Getter
                | BuiltinPrototypeKind::Setter
                | BuiltinPrototypeKind::IsPrototype,
                _,
            ) => {
                return Err(RuntimeError::Invariant(
                    "prototype builtin has wrong native invocation",
                ));
            }
            _ => return Self::start(runtime, realm, kind, arguments),
        };
        if matches!(kind, BuiltinPrototypeKind::Setter) {
            if matches!(receiver, JsValue::Null | JsValue::Undefined) {
                return not_object(runtime, realm);
            }
            let prototype = match arguments.readable.first().ok_or(RuntimeError::Invariant(
                "prototype setter argv was not padded",
            ))? {
                JsValue::Object(id) => Some(ObjectRef::from_borrowed_handle(runtime.clone(), *id)?),
                JsValue::Null => None,
                _ => return Ok(Self::Complete(Completion::Return(JsValue::Undefined))),
            };
            let JsValue::Object(id) = receiver else {
                return Ok(Self::Complete(Completion::Return(JsValue::Undefined)));
            };
            let object = ObjectRef::from_borrowed_handle(runtime.clone(), *id)?;
            return Ok(Self::Set {
                object: object.clone(),
                prototype,
                resume: BuiltinPrototypeResume(Box::new(BuiltinPrototypeResumeState {
                    object: object.clone(),
                    realm,
                    kind,
                })),
            });
        }
        // isPrototypeOf checks the candidate before converting its receiver.
        let candidate = if matches!(kind, BuiltinPrototypeKind::IsPrototype) {
            match arguments.readable.first() {
                Some(JsValue::Object(id)) => {
                    Some(ObjectRef::from_borrowed_handle(runtime.clone(), *id)?)
                }
                _ => return Ok(Self::Complete(Completion::Return(JsValue::Bool(false)))),
            }
        } else {
            None
        };
        let object =
            match runtime.native_to_object_jsvalue(realm, runtime.dup_jsvalue(receiver)?)? {
                NativeConversion::Value(object) => object,
                NativeConversion::Throw(value) => {
                    return Ok(Self::Complete(Completion::Throw(value)));
                }
            };
        Ok(Self::Get {
            object: candidate.unwrap_or_else(|| object.clone()),
            resume: BuiltinPrototypeResume(Box::new(BuiltinPrototypeResumeState {
                object,
                realm,
                kind,
            })),
        })
    }

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
        if matches!(target, JsValue::Null | JsValue::Undefined)
            || (matches!(
                kind,
                BuiltinPrototypeKind::ReflectGet | BuiltinPrototypeKind::ReflectSet
            ) && !matches!(target, JsValue::Object(_)))
        {
            return not_object(runtime, realm);
        }
        if matches!(
            kind,
            BuiltinPrototypeKind::ObjectGet | BuiltinPrototypeKind::ReflectGet
        ) {
            let object =
                match runtime.native_to_object_jsvalue(realm, runtime.dup_jsvalue(target)?)? {
                    NativeConversion::Value(object) => object,
                    NativeConversion::Throw(value) => {
                        return Ok(Self::Complete(Completion::Throw(value)));
                    }
                };
            return Ok(Self::Get {
                object: object.clone(),
                resume: BuiltinPrototypeResume(Box::new(BuiltinPrototypeResumeState {
                    object,
                    realm,
                    kind,
                })),
            });
        }
        let prototype = match arguments.readable.get(1).ok_or(RuntimeError::Invariant(
            "Object.setPrototypeOf prototype argv was not padded",
        ))? {
            JsValue::Object(id) => Some(ObjectRef::from_borrowed_handle(runtime.clone(), *id)?),
            JsValue::Null => None,
            _ => return not_object(runtime, realm),
        };
        let JsValue::Object(id) = target else {
            return Ok(Self::Complete(Completion::Return(
                runtime.dup_jsvalue(target)?,
            )));
        };
        let object = ObjectRef::from_borrowed_handle(runtime.clone(), *id)?;
        Ok(Self::Set {
            object: object.clone(),
            prototype,
            resume: BuiltinPrototypeResume(Box::new(BuiltinPrototypeResumeState {
                object: object.clone(),
                realm,
                kind,
            })),
        })
    }
}
fn not_object(runtime: &Runtime, realm: ContextId) -> Result<BuiltinPrototypeStep, RuntimeError> {
    Ok(BuiltinPrototypeStep::Complete(Completion::Throw(
        runtime.new_native_error_jsvalue(realm, NativeErrorKind::Type, "not an object")?,
    )))
}
impl BuiltinPrototypeResume {
    pub(crate) fn prototype(
        self,
        _runtime: &Runtime,
        result: NativeConversion<Option<ObjectRef>>,
    ) -> Result<BuiltinPrototypeStep, RuntimeError> {
        if !matches!(
            self.0.kind,
            BuiltinPrototypeKind::ObjectGet
                | BuiltinPrototypeKind::ReflectGet
                | BuiltinPrototypeKind::Getter
                | BuiltinPrototypeKind::IsPrototype
        ) {
            if let NativeConversion::Throw(value) = result {
                let _ = _runtime.release_jsvalue(value);
            }
            return Err(RuntimeError::Invariant(
                "prototype builtin received a Get reply for Set",
            ));
        }
        if matches!(self.0.kind, BuiltinPrototypeKind::IsPrototype) {
            return Ok(match result {
                NativeConversion::Throw(value) => {
                    BuiltinPrototypeStep::Complete(Completion::Throw(value))
                }
                NativeConversion::Value(None) => {
                    BuiltinPrototypeStep::Complete(Completion::Return(JsValue::Bool(false)))
                }
                NativeConversion::Value(Some(object)) if object == self.0.object => {
                    BuiltinPrototypeStep::Complete(Completion::Return(JsValue::Bool(true)))
                }
                NativeConversion::Value(Some(object)) => BuiltinPrototypeStep::Get {
                    object,
                    resume: self,
                },
            });
        }
        Ok(BuiltinPrototypeStep::Complete(match result {
            NativeConversion::Value(prototype) => {
                Completion::Return(prototype.map_or(JsValue::Null, |object| {
                    JsValue::Object(object.into_handle())
                }))
            }
            NativeConversion::Throw(value) => Completion::Throw(value),
        }))
    }
    pub(crate) fn boolean(
        self,
        runtime: &Runtime,
        result: NativeConversion<bool>,
    ) -> Result<BuiltinPrototypeStep, RuntimeError> {
        Ok(BuiltinPrototypeStep::Complete(match self.0.kind {
            BuiltinPrototypeKind::ReflectSet => match result {
                NativeConversion::Value(value) => Completion::Return(JsValue::Bool(value)),
                NativeConversion::Throw(value) => Completion::Throw(value),
            },
            BuiltinPrototypeKind::ObjectSet | BuiltinPrototypeKind::Setter => {
                match runtime.finish_set_prototype_or_throw(self.0.realm, &self.0.object, result)? {
                    Some(value) => Completion::Throw(value),
                    None => {
                        Completion::Return(if matches!(self.0.kind, BuiltinPrototypeKind::Setter) {
                            JsValue::Undefined
                        } else {
                            JsValue::Object(self.0.object.into_handle())
                        })
                    }
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
                resume.prototype(runtime, runtime.internal_get_prototype_of(realm, &object)?)?
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

// S11 all-domain protocol bound; inline completion stays allocation-free.
const _: () = assert!(std::mem::size_of::<BuiltinPrototypeStep>() <= 64);
