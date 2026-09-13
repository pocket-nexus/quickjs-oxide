//! Object.create/defineProperties: snapshot enumerable keys, then convert and publish each.
#[cfg(feature = "stack-vm")]
use crate::engine::builtins::native::NativeFunctionId;
use crate::engine::{
    api::{error::NativeErrorKind, runtime::Runtime, runtime_error::RuntimeError},
    heap::ContextId,
    object::{
        ObjectRef, OrdinaryPropertyDescriptor, PropertyKey, operations::InternalDefineResult,
    },
    value::{Value, conversion::NativeConversion},
    vm::{Completion, call::NativeArguments},
};
#[derive(Clone, Copy)]
pub(crate) enum DefinitionsKind {
    Create,
    Define,
}
impl DefinitionsKind {
    #[cfg(feature = "stack-vm")]
    pub(crate) fn for_target(target: NativeFunctionId) -> Option<Self> {
        Some(match target {
            NativeFunctionId::ObjectCreate => Self::Create,
            NativeFunctionId::ObjectDefineProperties => Self::Define,
            _ => return None,
        })
    }
}
pub(crate) enum DefinitionsStep {
    Complete(Completion),
    Keys {
        object: ObjectRef,
        resume: DefinitionsResume,
    },
    Enumerable {
        object: ObjectRef,
        key: PropertyKey,
        resume: DefinitionsResume,
    },
    Read {
        object: ObjectRef,
        key: PropertyKey,
        resume: DefinitionsResume,
    },
    Convert {
        value: Value,
        resume: DefinitionsResume,
    },
    Define {
        object: ObjectRef,
        key: PropertyKey,
        descriptor: OrdinaryPropertyDescriptor,
        resume: DefinitionsResume,
    },
}
pub(crate) struct DefinitionsResume {
    realm: ContextId,
    target: ObjectRef,
    source: ObjectRef,
    phase: Phase,
}
enum Phase {
    Keys,
    Snapshot {
        remaining: std::vec::IntoIter<PropertyKey>,
        selected: Vec<PropertyKey>,
        key: PropertyKey,
    },
    Read {
        remaining: std::vec::IntoIter<PropertyKey>,
        key: PropertyKey,
    },
    Convert {
        remaining: std::vec::IntoIter<PropertyKey>,
        key: PropertyKey,
    },
    Define {
        remaining: std::vec::IntoIter<PropertyKey>,
        key: PropertyKey,
    },
}
impl DefinitionsStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        kind: DefinitionsKind,
        arguments: &NativeArguments,
    ) -> Result<Self, RuntimeError> {
        let value = arguments.readable.first().ok_or(RuntimeError::Invariant(
            "Object definitions target argv was not padded",
        ))?;
        let target = match kind {
            DefinitionsKind::Create => match value {
                Value::Object(prototype) => runtime.new_object(Some(prototype))?,
                Value::Null => runtime.new_object(None)?,
                _ => {
                    return Ok(Self::Complete(Completion::Throw(
                        runtime.new_native_error(
                            realm,
                            NativeErrorKind::Type,
                            "not a prototype",
                        )?,
                    )));
                }
            },
            DefinitionsKind::Define => match value {
                Value::Object(object) => object.clone(),
                _ => {
                    return Ok(Self::Complete(Completion::Throw(
                        runtime.new_native_error(realm, NativeErrorKind::Type, "not an object")?,
                    )));
                }
            },
        };
        let value = arguments
            .readable
            .get(1)
            .cloned()
            .ok_or(RuntimeError::Invariant(
                "Object definitions properties argv was not padded",
            ))?;
        if matches!(kind, DefinitionsKind::Create) && matches!(value, Value::Undefined) {
            return Ok(Self::Complete(Completion::Return(Value::Object(target))));
        }
        let source = match runtime.native_to_object(realm, value)? {
            NativeConversion::Value(object) => object,
            NativeConversion::Throw(value) => return Ok(Self::Complete(Completion::Throw(value))),
        };
        Ok(Self::Keys {
            object: source.clone(),
            resume: DefinitionsResume {
                realm,
                target,
                source,
                phase: Phase::Keys,
            },
        })
    }
}
impl DefinitionsResume {
    pub(crate) fn keys(
        self,
        runtime: &Runtime,
        result: NativeConversion<Vec<PropertyKey>>,
    ) -> Result<DefinitionsStep, RuntimeError> {
        if !matches!(self.phase, Phase::Keys) {
            return Err(RuntimeError::Invariant(
                "Object definitions received unexpected keys",
            ));
        }
        let keys = match result {
            NativeConversion::Value(keys) => keys,
            NativeConversion::Throw(value) => {
                return Ok(DefinitionsStep::Complete(Completion::Throw(value)));
            }
        };
        let mut selected = Vec::new();
        selected.try_reserve_exact(keys.len()).map_err(|_| {
            RuntimeError::Invariant("Object definitions key snapshot allocation failed")
        })?;
        self.snapshot(runtime, keys.into_iter(), selected)
    }
    fn snapshot(
        self,
        runtime: &Runtime,
        mut remaining: std::vec::IntoIter<PropertyKey>,
        selected: Vec<PropertyKey>,
    ) -> Result<DefinitionsStep, RuntimeError> {
        let Some(key) = remaining.next() else {
            return self.next(runtime, selected.into_iter());
        };
        Ok(DefinitionsStep::Enumerable {
            object: self.source.clone(),
            key: key.clone(),
            resume: Self {
                phase: Phase::Snapshot {
                    remaining,
                    selected,
                    key,
                },
                ..self
            },
        })
    }
    pub(crate) fn boolean(
        self,
        runtime: &Runtime,
        result: NativeConversion<bool>,
    ) -> Result<DefinitionsStep, RuntimeError> {
        let Phase::Snapshot {
            remaining,
            mut selected,
            key,
        } = self.phase
        else {
            return Err(RuntimeError::Invariant(
                "Object definitions received unexpected enumerable reply",
            ));
        };
        match result {
            NativeConversion::Throw(value) => {
                return Ok(DefinitionsStep::Complete(Completion::Throw(value)));
            }
            NativeConversion::Value(true) => selected.push(key),
            NativeConversion::Value(false) => {}
        }
        Self {
            phase: Phase::Keys,
            ..self
        }
        .snapshot(runtime, remaining, selected)
    }
    fn next(
        self,
        _runtime: &Runtime,
        mut remaining: std::vec::IntoIter<PropertyKey>,
    ) -> Result<DefinitionsStep, RuntimeError> {
        let Some(key) = remaining.next() else {
            return Ok(DefinitionsStep::Complete(Completion::Return(
                Value::Object(self.target),
            )));
        };
        Ok(DefinitionsStep::Read {
            object: self.source.clone(),
            key: key.clone(),
            resume: Self {
                phase: Phase::Read { remaining, key },
                ..self
            },
        })
    }
    pub(crate) fn read(self, result: Completion) -> Result<DefinitionsStep, RuntimeError> {
        let Phase::Read { remaining, key } = self.phase else {
            return Err(RuntimeError::Invariant(
                "Object definitions received unexpected value reply",
            ));
        };
        Ok(match result {
            Completion::Throw(value) => DefinitionsStep::Complete(Completion::Throw(value)),
            Completion::Return(value) => DefinitionsStep::Convert {
                value,
                resume: Self {
                    phase: Phase::Convert { remaining, key },
                    ..self
                },
            },
        })
    }
    pub(crate) fn converted(
        self,
        result: NativeConversion<OrdinaryPropertyDescriptor>,
    ) -> Result<DefinitionsStep, RuntimeError> {
        let Phase::Convert { remaining, key } = self.phase else {
            return Err(RuntimeError::Invariant(
                "Object definitions received unexpected conversion reply",
            ));
        };
        Ok(match result {
            NativeConversion::Throw(value) => DefinitionsStep::Complete(Completion::Throw(value)),
            NativeConversion::Value(descriptor) => DefinitionsStep::Define {
                object: self.target.clone(),
                key: key.clone(),
                descriptor,
                resume: Self {
                    phase: Phase::Define { remaining, key },
                    ..self
                },
            },
        })
    }
    pub(crate) fn defined(
        self,
        runtime: &Runtime,
        result: NativeConversion<InternalDefineResult>,
    ) -> Result<DefinitionsStep, RuntimeError> {
        let Phase::Define { remaining, key } = self.phase else {
            return Err(RuntimeError::Invariant(
                "Object definitions received unexpected definition reply",
            ));
        };
        if let Some(value) = runtime.finish_define_property_or_throw(self.realm, &key, result)? {
            return Ok(DefinitionsStep::Complete(Completion::Throw(value)));
        }
        Self {
            phase: Phase::Keys,
            ..self
        }
        .next(runtime, remaining)
    }
}
pub(super) fn finish(
    runtime: &Runtime,
    realm: ContextId,
    mut step: DefinitionsStep,
) -> Result<Completion, RuntimeError> {
    loop {
        step = match step {
            DefinitionsStep::Complete(result) => return Ok(result),
            DefinitionsStep::Keys { object, resume } => {
                resume.keys(runtime, runtime.internal_own_property_keys(realm, &object)?)?
            }
            DefinitionsStep::Enumerable {
                object,
                key,
                resume,
            } => resume.boolean(
                runtime,
                runtime.internal_snapshot_own_property_is_enumerable(realm, &object, &key)?,
            )?,
            DefinitionsStep::Read {
                object,
                key,
                resume,
            } => resume.read(runtime.get_property_in_realm(realm, &object, &key)?)?,
            DefinitionsStep::Convert { value, resume } => {
                resume.converted(runtime.native_to_property_descriptor(realm, value)?)?
            }
            DefinitionsStep::Define {
                object,
                key,
                descriptor,
                resume,
            } => resume.defined(
                runtime,
                runtime.internal_define_own_property(realm, &object, &key, &descriptor)?,
            )?,
        };
    }
}
