//! Object-environment lookup owns presence checks across unscopables and RHS callbacks.
use crate::engine::{
    api::{ErrorKind, runtime::Runtime, runtime_error::RuntimeError},
    heap::ContextId,
    object::{ObjectRef, PropertyKey, WellKnownSymbol, operations::InternalSetResult},
    value::{Value, conversion::NativeConversion},
    vm::Completion,
};

pub(in crate::engine::vm) enum EnvironmentStep {
    Complete(Completion),
    Has {
        object: ObjectRef,
        key: PropertyKey,
        resume: EnvironmentResume,
    },
    Read {
        object: ObjectRef,
        key: PropertyKey,
        receiver: Value,
        resume: EnvironmentResume,
    },
    Set {
        object: ObjectRef,
        key: PropertyKey,
        value: Value,
        resume: EnvironmentResume,
    },
    Delete {
        object: ObjectRef,
        key: PropertyKey,
        resume: EnvironmentResume,
    },
}
pub(in crate::engine::vm) struct EnvironmentResume {
    realm: ContextId,
    phase: Phase,
}
enum Phase {
    Binding {
        object: ObjectRef,
        key: PropertyKey,
        with: bool,
    },
    Unscopables {
        key: PropertyKey,
    },
    Excluded,
    Get {
        object: ObjectRef,
        key: PropertyKey,
        strict: bool,
    },
    Put {
        object: ObjectRef,
        key: PropertyKey,
        value: Value,
        strict: bool,
        reference: bool,
    },
    Reference {
        object: ObjectRef,
    },
    DeleteGlobal {
        object: ObjectRef,
        key: PropertyKey,
    },
    Set {
        key: PropertyKey,
        strict: bool,
    },
    Value,
    Boolean,
}
impl EnvironmentStep {
    pub(in crate::engine::vm) fn read(
        realm: ContextId,
        object: ObjectRef,
        key: PropertyKey,
        receiver: Value,
    ) -> Self {
        Self::Read {
            object,
            key,
            receiver,
            resume: EnvironmentResume {
                realm,
                phase: Phase::Value,
            },
        }
    }
    pub(in crate::engine::vm) fn has_binding(
        realm: ContextId,
        object: ObjectRef,
        key: PropertyKey,
        with: bool,
    ) -> Self {
        Self::Has {
            object: object.clone(),
            key: key.clone(),
            resume: EnvironmentResume {
                realm,
                phase: Phase::Binding { object, key, with },
            },
        }
    }
    pub(in crate::engine::vm) fn get(
        realm: ContextId,
        object: ObjectRef,
        key: PropertyKey,
        strict: bool,
    ) -> Self {
        Self::Has {
            object: object.clone(),
            key: key.clone(),
            resume: EnvironmentResume {
                realm,
                phase: Phase::Get {
                    object,
                    key,
                    strict,
                },
            },
        }
    }
    pub(in crate::engine::vm) fn put(
        realm: ContextId,
        object: ObjectRef,
        key: PropertyKey,
        value: Value,
        strict: bool,
        reference: bool,
    ) -> Self {
        Self::Has {
            object: object.clone(),
            key: key.clone(),
            resume: EnvironmentResume {
                realm,
                phase: Phase::Put {
                    object,
                    key,
                    value,
                    strict,
                    reference,
                },
            },
        }
    }
    pub(in crate::engine::vm) fn set(
        realm: ContextId,
        object: ObjectRef,
        key: PropertyKey,
        value: Value,
        strict: bool,
    ) -> Self {
        Self::Set {
            object,
            key: key.clone(),
            value,
            resume: EnvironmentResume {
                realm,
                phase: Phase::Set { key, strict },
            },
        }
    }
    pub(in crate::engine::vm) fn reference(
        realm: ContextId,
        object: ObjectRef,
        key: PropertyKey,
    ) -> Self {
        Self::Has {
            object: object.clone(),
            key,
            resume: EnvironmentResume {
                realm,
                phase: Phase::Reference { object },
            },
        }
    }
    pub(in crate::engine::vm) fn delete_global(
        realm: ContextId,
        object: ObjectRef,
        key: PropertyKey,
    ) -> Self {
        Self::Has {
            object: object.clone(),
            key: key.clone(),
            resume: EnvironmentResume {
                realm,
                phase: Phase::DeleteGlobal { object, key },
            },
        }
    }
    pub(in crate::engine::vm) fn delete(
        realm: ContextId,
        object: ObjectRef,
        key: PropertyKey,
    ) -> Self {
        Self::Delete {
            object,
            key,
            resume: EnvironmentResume {
                realm,
                phase: Phase::Boolean,
            },
        }
    }
}
impl EnvironmentResume {
    pub(in crate::engine::vm) fn boolean(
        self,
        runtime: &Runtime,
        reply: NativeConversion<bool>,
    ) -> Result<EnvironmentStep, RuntimeError> {
        let present = match reply {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => {
                return Ok(EnvironmentStep::Complete(Completion::Throw(value)));
            }
        };
        let realm = self.realm;
        match self.phase {
            Phase::Binding { object, key, with } => {
                if !present || !with {
                    return Ok(EnvironmentStep::Complete(Completion::Return(Value::Bool(
                        present,
                    ))));
                }
                Ok(EnvironmentStep::Read {
                    receiver: Value::Object(object.clone()),
                    object,
                    key: PropertyKey::from(runtime.well_known_symbol(WellKnownSymbol::Unscopables)),
                    resume: EnvironmentResume {
                        realm,
                        phase: Phase::Unscopables { key },
                    },
                })
            }
            Phase::Get {
                object,
                key,
                strict,
            } => {
                if !present {
                    if strict {
                        return Err(runtime
                            .native_atom_error(ErrorKind::Reference, "'", &key, "' is not defined")?
                            .into());
                    }
                    return Ok(EnvironmentStep::Complete(Completion::Return(
                        Value::Undefined,
                    )));
                }
                Ok(EnvironmentStep::Read {
                    receiver: Value::Object(object.clone()),
                    object,
                    key,
                    resume: EnvironmentResume {
                        realm,
                        phase: Phase::Value,
                    },
                })
            }
            Phase::Put {
                object,
                key,
                value,
                strict,
                reference,
            } => {
                if !present && strict {
                    return Err(runtime
                        .native_atom_error(ErrorKind::Reference, "'", &key, "' is not defined")?
                        .into());
                }
                if reference && let Some(root) = runtime.own_var_ref_root(&object, &key)? {
                    let cell = runtime.0.state.borrow().heap.var_ref(root.id())?.clone();
                    if matches!(cell.value, crate::engine::heap::RawValue::Uninitialized) {
                        return Err(super::super::bindings::lexical_uninitialized_error(
                            runtime,
                            Some(key.atom()),
                            true,
                        )?
                        .into());
                    }
                    if cell.is_const && strict {
                        return Err(super::super::bindings::lexical_read_only_error(
                            runtime,
                            Some(key.atom()),
                        )?
                        .into());
                    }
                    if !cell.is_const {
                        runtime.write_var_ref(&root, value)?;
                    }
                    return Ok(EnvironmentStep::Complete(Completion::Return(
                        Value::Undefined,
                    )));
                }
                Ok(EnvironmentStep::set(realm, object, key, value, strict))
            }
            Phase::Reference { object } => {
                Ok(EnvironmentStep::Complete(Completion::Return(if present {
                    Value::Object(object)
                } else {
                    Value::Undefined
                })))
            }
            Phase::DeleteGlobal { object, key } => Ok(if present {
                EnvironmentStep::delete(realm, object, key)
            } else {
                EnvironmentStep::Complete(Completion::Return(Value::Bool(true)))
            }),
            Phase::Boolean => Ok(EnvironmentStep::Complete(Completion::Return(Value::Bool(
                present,
            )))),
            _ => Err(RuntimeError::Invariant(
                "environment Boolean reply has wrong phase",
            )),
        }
    }
    pub(in crate::engine::vm) fn resume(
        self,
        runtime: &Runtime,
        reply: Completion,
    ) -> Result<EnvironmentStep, RuntimeError> {
        let value = match reply {
            Completion::Return(value) => value,
            Completion::Throw(value) => {
                return Ok(EnvironmentStep::Complete(Completion::Throw(value)));
            }
        };
        match self.phase {
            Phase::Unscopables { key } => Ok(match value {
                Value::Object(object) => EnvironmentStep::Read {
                    receiver: Value::Object(object.clone()),
                    object,
                    key,
                    resume: EnvironmentResume {
                        realm: self.realm,
                        phase: Phase::Excluded,
                    },
                },
                _ => EnvironmentStep::Complete(Completion::Return(Value::Bool(true))),
            }),
            Phase::Excluded => Ok(EnvironmentStep::Complete(Completion::Return(Value::Bool(
                !runtime.value_to_boolean(&value)?,
            )))),
            Phase::Value => Ok(EnvironmentStep::Complete(Completion::Return(value))),
            _ => Err(RuntimeError::Invariant(
                "environment value reply has wrong phase",
            )),
        }
    }
    pub(in crate::engine::vm) fn set(
        self,
        runtime: &Runtime,
        reply: NativeConversion<InternalSetResult>,
    ) -> Result<EnvironmentStep, RuntimeError> {
        let Phase::Set { key, strict } = self.phase else {
            return Err(RuntimeError::Invariant(
                "environment Set reply has wrong phase",
            ));
        };
        Ok(EnvironmentStep::Complete(
            runtime.finish_property_set(reply, &key, strict)?,
        ))
    }
}
pub(in crate::engine::vm) fn finish(
    runtime: &Runtime,
    realm: ContextId,
    mut step: EnvironmentStep,
) -> Result<Completion, RuntimeError> {
    loop {
        step = match step {
            EnvironmentStep::Complete(result) => return Ok(result),
            EnvironmentStep::Has {
                object,
                key,
                resume,
            } => resume.boolean(
                runtime,
                runtime.internal_has_property(realm, &object, &key)?,
            )?,
            EnvironmentStep::Read {
                object,
                key,
                receiver,
                resume,
            } => resume.resume(
                runtime,
                runtime.internal_get(realm, &object, &key, receiver)?,
            )?,
            EnvironmentStep::Set {
                object,
                key,
                value,
                resume,
            } => resume.set(
                runtime,
                runtime.internal_set(realm, &object, &key, value, Value::Object(object.clone()))?,
            )?,
            EnvironmentStep::Delete {
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
