//! Own-property predicates retain their distinct receiver/key conversion order.
#[cfg(feature = "stack-vm")]
use crate::engine::builtins::native::NativeFunctionId;
use crate::engine::builtins::native::ObjectAccessorKind;
use crate::engine::object::{
    AccessorValue, CallableRef, CompleteOrdinaryPropertyDescriptor, DescriptorField,
    OrdinaryPropertyDescriptor, operations::InternalDefineResult,
};
use crate::engine::{
    api::{runtime::Runtime, runtime_error::RuntimeError},
    heap::ContextId,
    object::{ObjectRef, PropertyKey},
    value::{Value, conversion::NativeConversion},
    vm::{
        Completion, ToPrimitiveHint,
        call::{NativeArguments, NativeInvocation},
    },
};
#[derive(Clone, Copy)]
pub(crate) enum PredicateKind {
    Define(ObjectAccessorKind),
    Lookup(ObjectAccessorKind),
    HasOwn,
    PrototypeHasOwn,
    Enumerable,
}
impl PredicateKind {
    #[cfg(feature = "stack-vm")]
    pub(crate) fn for_target(target: NativeFunctionId) -> Option<Self> {
        Some(match target {
            NativeFunctionId::ObjectPrototypeDefineAccessor(kind) => Self::Define(kind),
            NativeFunctionId::ObjectPrototypeLookupAccessor(kind) => Self::Lookup(kind),
            NativeFunctionId::ObjectHasOwn => Self::HasOwn,
            NativeFunctionId::ObjectPrototypeHasOwnProperty => Self::PrototypeHasOwn,
            NativeFunctionId::ObjectPrototypePropertyIsEnumerable => Self::Enumerable,
            _ => return None,
        })
    }
}
pub(crate) enum PredicateStep {
    Descriptor {
        object: ObjectRef,
        key: PropertyKey,
        resume: PredicateResume,
    },
    Prototype {
        object: ObjectRef,
        resume: PredicateResume,
    },
    Define {
        object: ObjectRef,
        key: PropertyKey,
        descriptor: OrdinaryPropertyDescriptor,
        resume: PredicateResume,
    },
    Complete(Completion),
    Key {
        value: Value,
        resume: PredicateResume,
    },
    Own {
        object: ObjectRef,
        key: PropertyKey,
        enumerable: bool,
        resume: PredicateResume,
    },
}
pub(crate) struct PredicateResume {
    realm: ContextId,
    receiver: Value,
    kind: PredicateKind,
    phase: Phase,
}
enum Phase {
    Key { accessor: Option<CallableRef> },
    Own(PropertyKey),
    Prototype(PropertyKey),
}
impl PredicateStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: ContextId,
        kind: PredicateKind,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Self, RuntimeError> {
        let NativeInvocation::Call { this_value } = invocation else {
            return Err(RuntimeError::Invariant(
                "own predicate did not receive a call",
            ));
        };
        let (receiver, key) = if matches!(kind, PredicateKind::HasOwn) {
            let value = arguments
                .readable
                .first()
                .cloned()
                .ok_or(RuntimeError::Invariant("hasOwn target argv was not padded"))?;
            let object = match runtime.native_to_object(realm, value)? {
                NativeConversion::Value(object) => object,
                NativeConversion::Throw(value) => {
                    return Ok(Self::Complete(Completion::Throw(value)));
                }
            };
            (Value::Object(object), arguments.readable.get(1))
        } else if matches!(kind, PredicateKind::Define(_) | PredicateKind::Lookup(_)) {
            let object = match runtime.native_to_object(realm, this_value.clone())? {
                NativeConversion::Value(object) => object,
                NativeConversion::Throw(value) => {
                    return Ok(Self::Complete(Completion::Throw(value)));
                }
            };
            (Value::Object(object), arguments.readable.first())
        } else {
            (this_value.clone(), arguments.readable.first())
        };
        let accessor = if matches!(kind, PredicateKind::Define(_)) {
            let value = arguments
                .readable
                .get(1)
                .ok_or(RuntimeError::Invariant("accessor argv was not padded"))?;
            let callable = match value {
                Value::Object(object) => runtime.as_callable(object)?,
                _ => None,
            };
            let Some(callable) = callable else {
                return Ok(Self::Complete(Completion::Throw(
                    runtime.new_native_error(
                        realm,
                        crate::engine::api::error::NativeErrorKind::Type,
                        "not a function",
                    )?,
                )));
            };
            Some(callable)
        } else {
            None
        };
        let value = key.cloned().ok_or(RuntimeError::Invariant(
            "own predicate key argv was not padded",
        ))?;
        Ok(Self::Key {
            value,
            resume: PredicateResume {
                realm,
                receiver,
                kind,
                phase: Phase::Key { accessor },
            },
        })
    }
}
impl PredicateResume {
    pub(crate) fn key(
        self,
        runtime: &Runtime,
        result: Completion,
    ) -> Result<PredicateStep, RuntimeError> {
        let Phase::Key { accessor } = self.phase else {
            return Err(RuntimeError::Invariant(
                "own predicate received a second key reply",
            ));
        };
        let value = match result {
            Completion::Return(value) => value,
            Completion::Throw(value) => {
                return Ok(PredicateStep::Complete(Completion::Throw(value)));
            }
        };
        let key = match runtime.property_key_from_primitive(self.realm, value)? {
            NativeConversion::Value(key) => key,
            NativeConversion::Throw(value) => {
                return Ok(PredicateStep::Complete(Completion::Throw(value)));
            }
        };
        let object = match runtime.native_to_object(self.realm, self.receiver.clone())? {
            NativeConversion::Value(object) => object,
            NativeConversion::Throw(value) => {
                return Ok(PredicateStep::Complete(Completion::Throw(value)));
            }
        };
        let resume = Self {
            phase: Phase::Own(key.clone()),
            ..self
        };
        Ok(match resume.kind {
            PredicateKind::Define(kind) => {
                let accessor = accessor.ok_or(RuntimeError::Invariant(
                    "accessor definition lost its callable",
                ))?;
                let mut descriptor = OrdinaryPropertyDescriptor {
                    enumerable: DescriptorField::Present(true),
                    configurable: DescriptorField::Present(true),
                    ..OrdinaryPropertyDescriptor::new()
                };
                match kind {
                    ObjectAccessorKind::Getter => {
                        descriptor.get = DescriptorField::Present(AccessorValue::Callable(accessor))
                    }
                    ObjectAccessorKind::Setter => {
                        descriptor.set = DescriptorField::Present(AccessorValue::Callable(accessor))
                    }
                }
                PredicateStep::Define {
                    object,
                    key,
                    descriptor,
                    resume,
                }
            }
            PredicateKind::Lookup(_) => PredicateStep::Descriptor {
                object,
                key,
                resume,
            },
            _ => PredicateStep::Own {
                object,
                key,
                enumerable: matches!(resume.kind, PredicateKind::Enumerable),
                resume,
            },
        })
    }
    pub(crate) fn boolean(
        self,
        result: NativeConversion<bool>,
    ) -> Result<PredicateStep, RuntimeError> {
        if !matches!(self.phase, Phase::Own(_))
            || matches!(
                self.kind,
                PredicateKind::Define(_) | PredicateKind::Lookup(_)
            )
        {
            return Err(RuntimeError::Invariant(
                "own predicate received a boolean before key conversion",
            ));
        }
        Ok(PredicateStep::Complete(match result {
            NativeConversion::Value(value) => Completion::Return(Value::Bool(value)),
            NativeConversion::Throw(value) => Completion::Throw(value),
        }))
    }
}
impl PredicateResume {
    pub(crate) fn defined(
        self,
        runtime: &Runtime,
        result: NativeConversion<InternalDefineResult>,
    ) -> Result<PredicateStep, RuntimeError> {
        let Phase::Own(key) = self.phase else {
            return Err(RuntimeError::Invariant(
                "accessor definition has wrong phase",
            ));
        };
        if !matches!(self.kind, PredicateKind::Define(_)) {
            return Err(RuntimeError::Invariant(
                "accessor definition has wrong kind",
            ));
        }
        Ok(PredicateStep::Complete(
            match runtime.finish_define_property_or_throw(self.realm, &key, result)? {
                Some(value) => Completion::Throw(value),
                None => Completion::Return(Value::Undefined),
            },
        ))
    }
    pub(crate) fn descriptor(
        self,
        result: NativeConversion<Option<CompleteOrdinaryPropertyDescriptor>>,
    ) -> Result<PredicateStep, RuntimeError> {
        let Phase::Own(key) = self.phase else {
            return Err(RuntimeError::Invariant("accessor lookup has wrong phase"));
        };
        let PredicateKind::Lookup(kind) = self.kind else {
            return Err(RuntimeError::Invariant("accessor lookup has wrong kind"));
        };
        let descriptor = match result {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => {
                return Ok(PredicateStep::Complete(Completion::Throw(value)));
            }
        };
        let Some(descriptor) = descriptor else {
            let Value::Object(object) = &self.receiver else {
                return Err(RuntimeError::Invariant("accessor lookup lost its object"));
            };
            return Ok(PredicateStep::Prototype {
                object: object.clone(),
                resume: Self {
                    phase: Phase::Prototype(key),
                    ..self
                },
            });
        };
        let value = match descriptor {
            CompleteOrdinaryPropertyDescriptor::Data { .. } => Value::Undefined,
            CompleteOrdinaryPropertyDescriptor::Accessor { get, set, .. } => match kind {
                ObjectAccessorKind::Getter => get,
                ObjectAccessorKind::Setter => set,
            }
            .map_or(Value::Undefined, |callable| {
                Value::Object(callable.into_object())
            }),
        };
        Ok(PredicateStep::Complete(Completion::Return(value)))
    }
    pub(crate) fn prototype(
        self,
        result: NativeConversion<Option<ObjectRef>>,
    ) -> Result<PredicateStep, RuntimeError> {
        let Phase::Prototype(key) = self.phase else {
            return Err(RuntimeError::Invariant(
                "accessor prototype reply has wrong phase",
            ));
        };
        Ok(match result {
            NativeConversion::Throw(value) => PredicateStep::Complete(Completion::Throw(value)),
            NativeConversion::Value(None) => {
                PredicateStep::Complete(Completion::Return(Value::Undefined))
            }
            NativeConversion::Value(Some(object)) => PredicateStep::Descriptor {
                object: object.clone(),
                key: key.clone(),
                resume: Self {
                    receiver: Value::Object(object),
                    phase: Phase::Own(key),
                    ..self
                },
            },
        })
    }
}

pub(in crate::engine::builtins) fn finish(
    runtime: &Runtime,
    realm: ContextId,
    mut step: PredicateStep,
) -> Result<Completion, RuntimeError> {
    loop {
        step = match step {
            PredicateStep::Complete(result) => return Ok(result),
            PredicateStep::Descriptor {
                object,
                key,
                resume,
            } => resume.descriptor(runtime.internal_get_own_property(realm, &object, &key)?)?,
            PredicateStep::Prototype { object, resume } => {
                resume.prototype(runtime.internal_get_prototype_of(realm, &object)?)?
            }
            PredicateStep::Define {
                object,
                key,
                descriptor,
                resume,
            } => resume.defined(
                runtime,
                runtime.internal_define_own_property(realm, &object, &key, &descriptor)?,
            )?,
            PredicateStep::Key { value, resume } => resume.key(
                runtime,
                runtime.to_primitive(realm, value, ToPrimitiveHint::String)?,
            )?,
            PredicateStep::Own {
                object,
                key,
                enumerable,
                resume,
            } => resume.boolean(if enumerable {
                runtime.internal_own_property_is_enumerable(realm, &object, &key)?
            } else {
                runtime.internal_has_own_property(realm, &object, &key)?
            })?,
        };
    }
}
