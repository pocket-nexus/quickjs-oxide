//! Array endpoint mutations retain their copy cursor across observable property operations.
#[cfg(feature = "stack-vm")]
use crate::engine::builtins::native::NativeFunctionId;
use crate::engine::{
    api::{error::NativeErrorKind, runtime::Runtime, runtime_error::RuntimeError},
    builtins::native::{ArrayPopKind, ArrayPushKind},
    heap::ContextId,
    object::{ObjectRef, PropertyKey, operations::InternalSetResult},
    value::{Value, conversion::NativeConversion},
    vm::{
        Completion,
        call::{NativeArguments, NativeInvocation},
    },
};
#[derive(Clone, Copy)]
pub(crate) enum MutationKind {
    Push(ArrayPushKind),
    Pop(ArrayPopKind),
}
impl MutationKind {
    #[cfg(feature = "stack-vm")]
    pub(crate) fn for_target(target: NativeFunctionId) -> Option<Self> {
        match target {
            NativeFunctionId::ArrayPrototypePush(kind) => Some(Self::Push(kind)),
            NativeFunctionId::ArrayPrototypePop(kind) => Some(Self::Pop(kind)),
            _ => None,
        }
    }
}
pub(crate) enum MutationStep {
    Complete(Completion),
    Read {
        object: ObjectRef,
        key: PropertyKey,
        resume: MutationResume,
    },
    Number {
        value: Value,
        resume: MutationResume,
    },
    Copy {
        object: ObjectRef,
        to: u64,
        from: u64,
        count: u64,
        backwards: bool,
        resume: MutationResume,
    },
    Set {
        object: ObjectRef,
        key: PropertyKey,
        value: Value,
        resume: MutationResume,
    },
    Delete {
        object: ObjectRef,
        key: PropertyKey,
        resume: MutationResume,
    },
}
enum Phase {
    Length,
    Number,
    Result,
    Copy,
    Write,
    DeleteLast,
    LengthWrite,
}
pub(crate) struct MutationResume {
    realm: ContextId,
    kind: MutationKind,
    object: ObjectRef,
    arguments: Vec<Value>,
    phase: Phase,
    length: u64,
    new_length: u64,
    cursor: u64,
    result: Value,
}
impl MutationStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        kind: MutationKind,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Self, RuntimeError> {
        let NativeInvocation::Call { this_value } = invocation else {
            return Err(RuntimeError::Invariant(
                "Array mutation requires generic invocation",
            ));
        };
        Self::start_values(
            runtime,
            realm,
            kind,
            this_value.clone(),
            arguments.readable[..arguments.actual_arg_count].to_vec(),
        )
    }
    pub(crate) fn start_values(
        runtime: &Runtime,
        realm: ContextId,
        kind: MutationKind,
        receiver: Value,
        arguments: Vec<Value>,
    ) -> Result<Self, RuntimeError> {
        let object = match runtime.native_to_object(realm, receiver)? {
            NativeConversion::Value(object) => object,
            NativeConversion::Throw(value) => return Ok(Self::Complete(Completion::Throw(value))),
        };
        Ok(Self::Read {
            object: object.clone(),
            key: runtime.intern_property_key("length")?,
            resume: MutationResume {
                realm,
                kind,
                object,
                arguments,
                phase: Phase::Length,
                length: 0,
                new_length: 0,
                cursor: 0,
                result: Value::Undefined,
            },
        })
    }
}
impl MutationResume {
    pub(crate) fn resume(
        mut self,
        runtime: &Runtime,
        result: Completion,
    ) -> Result<MutationStep, RuntimeError> {
        let value = match result {
            Completion::Return(value) => value,
            Completion::Throw(value) => {
                return Ok(MutationStep::Complete(Completion::Throw(value)));
            }
        };
        match self.phase {
            Phase::Length => {
                self.phase = Phase::Number;
                Ok(MutationStep::Number {
                    value,
                    resume: self,
                })
            }
            Phase::Result => {
                self.result = value;
                self.copy_next(runtime)
            }
            Phase::Copy => self.copied(runtime),
            _ => Err(RuntimeError::Invariant(
                "Array mutation received unexpected value reply",
            )),
        }
    }
    pub(crate) fn number(
        mut self,
        runtime: &Runtime,
        result: NativeConversion<f64>,
    ) -> Result<MutationStep, RuntimeError> {
        if !matches!(self.phase, Phase::Number) {
            return Err(RuntimeError::Invariant(
                "Array mutation number phase mismatch",
            ));
        }
        self.length = match result {
            NativeConversion::Value(number) => Runtime::length_from_number(number),
            NativeConversion::Throw(value) => {
                return Ok(MutationStep::Complete(Completion::Throw(value)));
            }
        };
        match self.kind {
            MutationKind::Push(_) => {
                self.new_length = self.length.saturating_add(self.arguments.len() as u64);
                if self.new_length > (1_u64 << 53) - 1 {
                    return Ok(MutationStep::Complete(Completion::Throw(
                        runtime.new_native_error(
                            self.realm,
                            NativeErrorKind::Type,
                            "Array loo long",
                        )?,
                    )));
                }
                self.copy_next(runtime)
            }
            MutationKind::Pop(kind) => {
                self.new_length = self.length.saturating_sub(1);
                if self.length == 0 {
                    return self.write_length(runtime);
                }
                let index = if kind == ArrayPopKind::Shift {
                    0
                } else {
                    self.new_length
                };
                self.phase = Phase::Result;
                Ok(MutationStep::Read {
                    object: self.object.clone(),
                    key: runtime.property_key_for_index(index)?,
                    resume: self,
                })
            }
        }
    }
    fn copy_next(mut self, runtime: &Runtime) -> Result<MutationStep, RuntimeError> {
        let (to, from, count, backwards) = match self.kind {
            MutationKind::Push(ArrayPushKind::Unshift) if !self.arguments.is_empty() => {
                (self.arguments.len() as u64, 0, self.length, true)
            }
            MutationKind::Pop(ArrayPopKind::Shift) => (0, 1, self.new_length, false),
            _ => return self.copied(runtime),
        };
        self.phase = Phase::Copy;
        Ok(MutationStep::Copy {
            object: self.object.clone(),
            to,
            from,
            count,
            backwards,
            resume: self,
        })
    }
    fn copied(mut self, runtime: &Runtime) -> Result<MutationStep, RuntimeError> {
        self.cursor = 0;
        match self.kind {
            MutationKind::Push(_) => self.write_next(runtime),
            MutationKind::Pop(_) => {
                self.phase = Phase::DeleteLast;
                Ok(MutationStep::Delete {
                    object: self.object.clone(),
                    key: runtime.property_key_for_index(self.new_length)?,
                    resume: self,
                })
            }
        }
    }
    fn write_next(mut self, runtime: &Runtime) -> Result<MutationStep, RuntimeError> {
        if let Some(value) = self.arguments.get(self.cursor as usize).cloned() {
            let from = match self.kind {
                MutationKind::Push(ArrayPushKind::Unshift) if !self.arguments.is_empty() => 0,
                _ => self.length,
            };
            self.phase = Phase::Write;
            return Ok(MutationStep::Set {
                object: self.object.clone(),
                key: runtime.property_key_for_index(from + self.cursor)?,
                value,
                resume: self,
            });
        }
        let redundant = matches!(self.kind, MutationKind::Push(ArrayPushKind::Push))
            && self.new_length <= u64::from(u32::MAX)
            && matches!(runtime.array_length_state_if_genuine(&self.object)?, Some((length, true)) if u64::from(length) == self.new_length);
        if redundant {
            self.complete()
        } else {
            self.write_length(runtime)
        }
    }
    fn write_length(mut self, runtime: &Runtime) -> Result<MutationStep, RuntimeError> {
        self.phase = Phase::LengthWrite;
        Ok(MutationStep::Set {
            object: self.object.clone(),
            key: runtime.intern_property_key("length")?,
            value: Value::number(self.new_length as f64),
            resume: self,
        })
    }
    fn complete(self) -> Result<MutationStep, RuntimeError> {
        Ok(MutationStep::Complete(Completion::Return(
            match self.kind {
                MutationKind::Push(_) => Value::number(self.new_length as f64),
                MutationKind::Pop(_) => self.result,
            },
        )))
    }
    pub(crate) fn boolean(
        self,
        runtime: &Runtime,
        result: NativeConversion<bool>,
    ) -> Result<MutationStep, RuntimeError> {
        let value = match result {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => {
                return Ok(MutationStep::Complete(Completion::Throw(value)));
            }
        };
        match self.phase {
            Phase::DeleteLast => {
                if !value {
                    return Ok(MutationStep::Complete(Completion::Throw(
                        runtime.new_native_error(
                            self.realm,
                            NativeErrorKind::Type,
                            "could not delete property",
                        )?,
                    )));
                }
                self.write_length(runtime)
            }
            _ => Err(RuntimeError::Invariant(
                "Array mutation boolean phase mismatch",
            )),
        }
    }
    pub(crate) fn set(
        mut self,
        runtime: &Runtime,
        key: PropertyKey,
        result: NativeConversion<InternalSetResult>,
    ) -> Result<MutationStep, RuntimeError> {
        if let Some(value) = runtime.finish_set_property_or_throw(self.realm, &key, result)? {
            return Ok(MutationStep::Complete(Completion::Throw(value)));
        }
        match self.phase {
            Phase::Write => {
                self.cursor += 1;
                self.write_next(runtime)
            }
            Phase::LengthWrite => self.complete(),
            _ => Err(RuntimeError::Invariant("Array mutation set phase mismatch")),
        }
    }
}
pub(crate) fn finish(
    runtime: &Runtime,
    realm: ContextId,
    mut step: MutationStep,
) -> Result<Completion, RuntimeError> {
    loop {
        step = match step {
            MutationStep::Complete(result) => return Ok(result),
            MutationStep::Read {
                object,
                key,
                resume,
            } => resume.resume(
                runtime,
                runtime.get_property_in_realm(realm, &object, &key)?,
            )?,
            MutationStep::Number { value, resume } => {
                resume.number(runtime, runtime.native_to_number(realm, &value)?)?
            }
            MutationStep::Copy {
                object,
                to,
                from,
                count,
                backwards,
                resume,
            } => resume.resume(
                runtime,
                super::copy::finish(
                    runtime,
                    realm,
                    super::copy::CopyStep::start(
                        runtime, realm, object, to, from, count, backwards,
                    )?,
                )?,
            )?,
            MutationStep::Set {
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
            MutationStep::Delete {
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
