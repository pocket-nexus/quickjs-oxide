//! ArrayBuffer construction orders lengths, options, and new.target lookup.
use super::MAX_SAFE_INTEGER_I64;
use crate::engine::{
    api::{error::NativeErrorKind, runtime::Runtime, runtime_error::RuntimeError},
    heap::ContextId,
    object::{ObjectRef, PropertyKey},
    value::{JsValue, conversion::NativeConversion},
    vm::{
        Completion, ToPrimitiveHint,
        call::{
            ConstructorPrototypeSource, NativeArguments, NativeInvocation,
            prototype::{ProtoSourceStep, finish as finish_source},
        },
    },
};
pub(crate) enum BufferConstructorStep {
    Complete(Completion),
    Primitive {
        value: JsValue,
        resume: BufferConstructorResume,
    },
    Read {
        object: ObjectRef,
        key: PropertyKey,
        resume: BufferConstructorResume,
    },
    Prototype {
        new_target: JsValue,
        resume: BufferConstructorResume,
    },
}
pub(crate) struct BufferConstructorResume(Box<BufferConstructorResumeState>);
impl std::ops::Deref for BufferConstructorResume {
    type Target = BufferConstructorResumeState;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl std::ops::DerefMut for BufferConstructorResume {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
const _: () = assert!(std::mem::size_of::<BufferConstructorResume>() <= 8);
pub(crate) struct BufferConstructorResumeState {
    runtime: Runtime,
    realm: ContextId,
    shared: bool,
    new_target: JsValue,
    options: Option<ObjectRef>,
    phase: ConstructorPhase,
}
impl Drop for BufferConstructorResumeState {
    fn drop(&mut self) {
        let _ = self
            .runtime
            .release_jsvalue(std::mem::replace(&mut self.new_target, JsValue::Undefined));
    }
}

pub(super) fn primitive_index(
    runtime: &Runtime,
    realm: ContextId,
    value: JsValue,
) -> Result<NativeConversion<u64>, RuntimeError> {
    let number = runtime.number_from_primitive_jsvalue(realm, &value);
    runtime.release_jsvalue(value)?;
    match number? {
        NativeConversion::Value(number) => runtime.index_from_number(realm, number),
        NativeConversion::Throw(value) => Ok(NativeConversion::Throw(value)),
    }
}
enum ConstructorPhase {
    Length,
    Maximum(u64),
    MaximumNumber(u64),
    Prototype { length: u64, maximum: Option<u64> },
}
impl BufferConstructorStep {
    pub(crate) fn start_shared(
        runtime: &Runtime,
        realm: ContextId,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Self, RuntimeError> {
        let Self::Primitive { value, mut resume } =
            Self::start(runtime, realm, invocation, arguments)?
        else {
            return Err(RuntimeError::Invariant(
                "buffer constructor initial step was not primitive",
            ));
        };
        resume.shared = true;
        Ok(Self::Primitive { value, resume })
    }

    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Self, RuntimeError> {
        let NativeInvocation::Construct { new_target } = invocation else {
            return Err(RuntimeError::Invariant(
                "ArrayBuffer constructor did not receive a constructor invocation",
            ));
        };
        let options = if arguments.actual_arg_count >= 2 {
            match arguments.readable.get(1) {
                Some(JsValue::Object(id)) => {
                    Some(ObjectRef::from_borrowed_handle(runtime.clone(), *id)?)
                }
                _ => None,
            }
        } else {
            None
        };
        let mut resume = BufferConstructorResume(Box::new(BufferConstructorResumeState {
            runtime: runtime.clone(),
            realm,
            shared: false,
            new_target: JsValue::Undefined,
            options,
            phase: ConstructorPhase::Length,
        }));
        resume.0.new_target = runtime.dup_jsvalue(new_target)?;
        let value = runtime.dup_jsvalue(arguments.readable.first().ok_or(
            RuntimeError::Invariant("ArrayBuffer length argument was not padded"),
        )?)?;
        Ok(Self::Primitive { value, resume })
    }
}
impl BufferConstructorResume {
    fn lookup(
        mut self,
        runtime: &Runtime,
        length: u64,
        maximum: Option<u64>,
    ) -> Result<BufferConstructorStep, RuntimeError> {
        Ok(BufferConstructorStep::Prototype {
            new_target: runtime.dup_jsvalue(&self.0.new_target)?,
            resume: {
                let updated_0 = ConstructorPhase::Prototype { length, maximum };
                self.0.phase = updated_0;
                self
            },
        })
    }
    pub(crate) fn resume(
        mut self,
        runtime: &Runtime,
        result: Completion,
    ) -> Result<BufferConstructorStep, RuntimeError> {
        let value = match result {
            Completion::Return(value) => value,
            Completion::Throw(value) => {
                return Ok(BufferConstructorStep::Complete(Completion::Throw(value)));
            }
        };
        match self.0.phase {
            ConstructorPhase::Length => {
                if matches!(value, JsValue::Object(_)) {
                    runtime.release_jsvalue(value)?;
                    return Err(RuntimeError::Invariant(
                        "ArrayBuffer length conversion returned an object",
                    ));
                }
                let length = match primitive_index(runtime, self.0.realm, value)? {
                    NativeConversion::Value(value) => value,
                    NativeConversion::Throw(value) => {
                        return Ok(BufferConstructorStep::Complete(Completion::Throw(value)));
                    }
                };
                if let Some(options) = &self.0.options {
                    Ok(BufferConstructorStep::Read {
                        object: options.clone(),
                        key: runtime.pinned_property_key(
                            crate::engine::atom::pinned::PinnedAtom::MaxByteLength,
                        )?,
                        resume: {
                            let updated_0 = ConstructorPhase::Maximum(length);
                            self.0.phase = updated_0;
                            self
                        },
                    })
                } else {
                    self.lookup(runtime, length, None)
                }
            }
            ConstructorPhase::Maximum(length) => {
                if matches!(value, JsValue::Undefined) {
                    self.lookup(runtime, length, None)
                } else {
                    Ok(BufferConstructorStep::Primitive {
                        value,
                        resume: {
                            let updated_0 = ConstructorPhase::MaximumNumber(length);
                            self.0.phase = updated_0;
                            self
                        },
                    })
                }
            }
            ConstructorPhase::MaximumNumber(length) => {
                if matches!(value, JsValue::Object(_)) {
                    runtime.release_jsvalue(value)?;
                    return Err(RuntimeError::Invariant(
                        "ArrayBuffer maximum conversion returned an object",
                    ));
                }
                let number = runtime.number_from_primitive_jsvalue(self.0.realm, &value);
                runtime.release_jsvalue(value)?;
                let maximum = match number? {
                    NativeConversion::Value(number) => super::quickjs_to_int64_free(number),
                    NativeConversion::Throw(value) => {
                        return Ok(BufferConstructorStep::Complete(Completion::Throw(value)));
                    }
                };
                if maximum > MAX_SAFE_INTEGER_I64 || length > maximum as u64 {
                    return Ok(BufferConstructorStep::Complete(Completion::Throw(
                        runtime.new_native_error_jsvalue(
                            self.0.realm,
                            NativeErrorKind::Range,
                            "invalid array buffer max length",
                        )?,
                    )));
                }
                self.lookup(runtime, length, Some(maximum as u64))
            }
            ConstructorPhase::Prototype { .. } => {
                runtime.release_jsvalue(value)?;
                Err(RuntimeError::Invariant(
                    "ArrayBuffer prototype request received an untyped reply",
                ))
            }
        }
    }
    pub(crate) fn prototype(
        self,
        runtime: &Runtime,
        result: NativeConversion<ConstructorPrototypeSource>,
    ) -> Result<BufferConstructorStep, RuntimeError> {
        let prototype = match result {
            NativeConversion::Value(ConstructorPrototypeSource::Explicit(value)) => value,
            NativeConversion::Value(ConstructorPrototypeSource::Realm(realm)) => {
                if self.0.shared {
                    runtime.shared_array_buffer_default_prototype(realm)?
                } else {
                    runtime.array_buffer_default_prototype(realm)?
                }
            }
            NativeConversion::Throw(value) => {
                return Ok(BufferConstructorStep::Complete(Completion::Throw(value)));
            }
        };
        let ConstructorPhase::Prototype { length, maximum } = self.0.phase else {
            return Err(RuntimeError::Invariant(
                "ArrayBuffer constructor received an unexpected prototype reply",
            ));
        };
        Ok(BufferConstructorStep::Complete(if self.0.shared {
            runtime.finish_shared_array_buffer_construction(
                self.0.realm,
                prototype,
                length,
                maximum,
            )?
        } else {
            runtime.finish_array_buffer_construction(self.0.realm, prototype, length, maximum)?
        }))
    }
}
pub(in crate::engine::builtins) fn finish(
    runtime: &Runtime,
    realm: ContextId,
    mut step: BufferConstructorStep,
) -> Result<Completion, RuntimeError> {
    loop {
        step = match step {
            BufferConstructorStep::Complete(result) => return Ok(result),
            BufferConstructorStep::Primitive { value, resume } => {
                let result = if matches!(value, JsValue::Object(_)) {
                    runtime.to_primitive_jsvalue(realm, value, ToPrimitiveHint::Number)?
                } else {
                    Completion::Return(value)
                };
                resume.resume(runtime, result)?
            }
            BufferConstructorStep::Read {
                object,
                key,
                resume,
            } => resume.resume(
                runtime,
                runtime.internal_get_jsvalue(
                    realm,
                    &object,
                    &key,
                    JsValue::Object(object.clone().into_handle()),
                )?,
            )?,
            BufferConstructorStep::Prototype { new_target, resume } => resume.prototype(
                runtime,
                finish_source(
                    runtime,
                    realm,
                    ProtoSourceStep::start(runtime, realm, new_target)?,
                )?,
            )?,
        };
    }
}

// S11 all-domain protocol bound; inline completion stays allocation-free.
const _: () = assert!(std::mem::size_of::<BufferConstructorStep>() <= 64);
