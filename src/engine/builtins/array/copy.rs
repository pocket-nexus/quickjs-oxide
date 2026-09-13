//! One directional Has/Get/Set-or-Delete range algorithm for Array mutations.
use crate::engine::{
    api::{error::NativeErrorKind, runtime::Runtime, runtime_error::RuntimeError},
    heap::ContextId,
    object::{ObjectRef, PropertyKey, operations::InternalSetResult},
    value::{Value, conversion::NativeConversion},
    vm::Completion,
};
pub(crate) enum CopyStep {
    Complete(Completion),
    Has {
        object: ObjectRef,
        key: PropertyKey,
        resume: CopyResume,
    },
    Read {
        object: ObjectRef,
        key: PropertyKey,
        resume: CopyResume,
    },
    Set {
        object: ObjectRef,
        key: PropertyKey,
        value: Value,
        resume: CopyResume,
    },
    Delete {
        object: ObjectRef,
        key: PropertyKey,
        resume: CopyResume,
    },
}
enum Phase {
    Has,
    Read,
    Write,
}
pub(crate) struct CopyResume {
    realm: ContextId,
    object: ObjectRef,
    to: u64,
    from: u64,
    count: u64,
    backwards: bool,
    offset: u64,
    phase: Phase,
}
impl CopyStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        object: ObjectRef,
        to: u64,
        from: u64,
        count: u64,
        backwards: bool,
    ) -> Result<Self, RuntimeError> {
        CopyResume {
            realm,
            object,
            to,
            from,
            count,
            backwards,
            offset: 0,
            phase: Phase::Has,
        }
        .next(runtime)
    }
}
impl CopyResume {
    fn relative(&self) -> u64 {
        if self.backwards {
            self.count - self.offset - 1
        } else {
            self.offset
        }
    }
    fn to_key(&self, runtime: &Runtime) -> Result<PropertyKey, RuntimeError> {
        Ok(
            runtime.property_key_for_index(self.to.checked_add(self.relative()).ok_or(
                RuntimeError::Invariant("Array copy target index overflowed"),
            )?)?,
        )
    }
    fn from_key(&self, runtime: &Runtime) -> Result<PropertyKey, RuntimeError> {
        Ok(
            runtime.property_key_for_index(self.from.checked_add(self.relative()).ok_or(
                RuntimeError::Invariant("Array copy source index overflowed"),
            )?)?,
        )
    }
    fn next(mut self, runtime: &Runtime) -> Result<CopyStep, RuntimeError> {
        if self.offset == self.count {
            return Ok(CopyStep::Complete(Completion::Return(Value::Undefined)));
        }
        self.phase = Phase::Has;
        Ok(CopyStep::Has {
            object: self.object.clone(),
            key: self.from_key(runtime)?,
            resume: self,
        })
    }
    pub(crate) fn boolean(
        mut self,
        runtime: &Runtime,
        result: NativeConversion<bool>,
    ) -> Result<CopyStep, RuntimeError> {
        let value = match result {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => {
                return Ok(CopyStep::Complete(Completion::Throw(value)));
            }
        };
        match self.phase {
            Phase::Has if value => {
                self.phase = Phase::Read;
                Ok(CopyStep::Read {
                    object: self.object.clone(),
                    key: self.from_key(runtime)?,
                    resume: self,
                })
            }
            Phase::Has => {
                self.phase = Phase::Write;
                Ok(CopyStep::Delete {
                    object: self.object.clone(),
                    key: self.to_key(runtime)?,
                    resume: self,
                })
            }
            Phase::Write => {
                if !value {
                    return Ok(CopyStep::Complete(Completion::Throw(
                        runtime.new_native_error(
                            self.realm,
                            NativeErrorKind::Type,
                            "could not delete property",
                        )?,
                    )));
                }
                self.offset += 1;
                self.next(runtime)
            }
            _ => Err(RuntimeError::Invariant("Array copy boolean phase mismatch")),
        }
    }
    pub(crate) fn resume(
        mut self,
        runtime: &Runtime,
        result: Completion,
    ) -> Result<CopyStep, RuntimeError> {
        if !matches!(self.phase, Phase::Read) {
            return Err(RuntimeError::Invariant("Array copy value phase mismatch"));
        }
        let value = match result {
            Completion::Return(value) => value,
            Completion::Throw(value) => return Ok(CopyStep::Complete(Completion::Throw(value))),
        };
        self.phase = Phase::Write;
        Ok(CopyStep::Set {
            object: self.object.clone(),
            key: self.to_key(runtime)?,
            value,
            resume: self,
        })
    }
    pub(crate) fn set(
        mut self,
        runtime: &Runtime,
        key: PropertyKey,
        result: NativeConversion<InternalSetResult>,
    ) -> Result<CopyStep, RuntimeError> {
        if !matches!(self.phase, Phase::Write) {
            return Err(RuntimeError::Invariant("Array copy set phase mismatch"));
        }
        if let Some(value) = runtime.finish_set_property_or_throw(self.realm, &key, result)? {
            return Ok(CopyStep::Complete(Completion::Throw(value)));
        }
        self.offset += 1;
        self.next(runtime)
    }
}
pub(crate) fn finish(
    runtime: &Runtime,
    realm: ContextId,
    mut step: CopyStep,
) -> Result<Completion, RuntimeError> {
    loop {
        step = match step {
            CopyStep::Complete(result) => return Ok(result),
            CopyStep::Read {
                object,
                key,
                resume,
            } => resume.resume(
                runtime,
                runtime.get_property_in_realm(realm, &object, &key)?,
            )?,
            CopyStep::Has {
                object,
                key,
                resume,
            } => resume.boolean(
                runtime,
                runtime.internal_has_property(realm, &object, &key)?,
            )?,
            CopyStep::Set {
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
            CopyStep::Delete {
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
