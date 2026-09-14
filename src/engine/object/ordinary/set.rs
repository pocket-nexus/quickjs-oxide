//! Ordinary Set phases. Storage probes never retain a borrow across a request.
use super::*;

pub(crate) enum SetStep {
    Complete(PropertySetAction),
    Continue {
        resume: SetResume,
    },
    Proxy {
        object: ObjectRef,
        key: PropertyKey,
        value: Value,
        receiver: Value,
        resume: SetResume,
    },
    Special {
        object: ObjectRef,
        key: PropertyKey,
        value: Value,
        receiver: Value,
        resume: SetResume,
    },
    ArrayLength {
        object: ObjectRef,
        key: PropertyKey,
        value: Value,
        resume: SetResume,
    },
    Descriptor {
        object: ObjectRef,
        key: PropertyKey,
        resume: SetResume,
    },
    Define {
        object: ObjectRef,
        key: PropertyKey,
        descriptor: OrdinaryPropertyDescriptor,
        resume: SetResume,
    },
}

pub(crate) struct SetResume {
    state: State,
    phase: Phase,
}
enum Phase {
    Walk(ObjectRef),
    Forward,
    Special(ObjectRef),
    Receiver,
    Define(ObjectRef),
}
struct State {
    realm: Option<ContextId>,
    _target: ObjectRef,
    key: PropertyKey,
    value: Value,
    receiver: Value,
}

impl SetStep {
    pub(crate) fn start(
        runtime: &Runtime,
        realm: Option<ContextId>,
        object: ObjectRef,
        key: PropertyKey,
        value: Value,
        receiver: Value,
    ) -> Result<Self, RuntimeError> {
        let _operation = runtime.operation();
        runtime.validate_object_and_key(&object, &key)?;
        runtime.validate_value_domain(&value, "property value")?;
        runtime.validate_value_domain(&receiver, "property receiver")?;
        if realm.is_none()
            && (runtime.is_proxy_object(&object)?
                || matches!(&receiver,Value::Object(object) if runtime.is_proxy_object(object)?))
        {
            return Err(RuntimeError::Invariant("exotic Set requires a realm"));
        }
        State {
            realm,
            _target: object.clone(),
            key,
            value,
            receiver,
        }
        .walk(runtime, object)
    }

    /// Finish local storage phases without installing scheduler parents. Stop
    /// before any Proxy, descriptor, setter or user conversion request.
    #[cfg(feature = "stack-vm")]
    pub(crate) fn advance_without_callback(
        mut self,
        runtime: &Runtime,
    ) -> Result<Self, RuntimeError> {
        loop {
            self = match self {
                Self::Continue { resume } => resume.advance(runtime)?,
                Self::Special {
                    object,
                    key,
                    value,
                    receiver,
                    resume,
                } if !matches!(value, Value::Object(_)) => {
                    let realm = resume
                        .state
                        .realm
                        .ok_or(RuntimeError::Invariant("typed Set requires a realm"))?;
                    let result = match runtime
                        .prepare_typed_array_set(&object, &key, &value, &receiver)?
                    {
                        None => None,
                        Some(request) => {
                            let crate::engine::builtins::TypedWriteStep::Complete(result) =
                                request.complete_primitive(runtime, realm)?
                            else {
                                return Err(RuntimeError::Invariant(
                                    "primitive typed write suspended",
                                ));
                            };
                            Some(match result {
                                NativeConversion::Value(_) => {
                                    NativeConversion::Value(InternalSetResult::Accepted)
                                }
                                NativeConversion::Throw(value) => NativeConversion::Throw(value),
                            })
                        }
                    };
                    resume.special(runtime, result)?
                }
                Self::Descriptor {
                    object,
                    key,
                    resume,
                } if matches!(runtime.array_own_key(&object, &key)?, ArrayOwnKey::Index(_)) => {
                    let descriptor = runtime.get_own_property(&object, &key)?;
                    resume.descriptor(runtime, NativeConversion::Value(descriptor))?
                }
                Self::Define {
                    object,
                    key,
                    descriptor,
                    resume,
                } if matches!(runtime.array_own_key(&object, &key)?, ArrayOwnKey::Index(_)) => {
                    // A genuine Array index cannot request length/value coercion.
                    // The common definition kernel still enforces flags, sparse
                    // transitions, extensibility and the writable length bound.
                    let result = match runtime.define_own_property_in_realm(
                        resume.state.realm,
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
                    resume.defined(runtime, result)?
                }
                step => return Ok(step),
            };
        }
    }

    /// Old callers consume the same domain protocol synchronously. The owned
    /// VM only uses this for the remaining Array/TypedArray conversion steps.
    pub(crate) fn finish_sync(self, runtime: &Runtime) -> Result<Self, RuntimeError> {
        match self {
            Self::Complete(_) => Ok(self),
            Self::Continue { resume } => resume.advance(runtime),
            Self::Proxy {
                object,
                key,
                value,
                receiver,
                resume,
            } => {
                let realm = resume
                    .state
                    .realm
                    .ok_or(RuntimeError::Invariant("exotic Set requires a realm"))?;
                let result = runtime.proxy_set(realm, &object, &key, value, receiver)?;
                resume.forward(set_completion(result))
            }
            Self::Special {
                object,
                key,
                value,
                receiver,
                resume,
            } => {
                let realm = resume
                    .state
                    .realm
                    .ok_or(RuntimeError::Invariant("typed Set requires a realm"))?;
                let result = runtime.try_special_set(
                    SpecialKind::TypedArray,
                    realm,
                    &object,
                    &key,
                    &value,
                    &receiver,
                )?;
                resume.special(runtime, result)
            }
            Self::ArrayLength {
                object,
                key,
                value,
                resume,
            } => {
                let action =
                    runtime.prepare_set_array_length(resume.state.realm, &object, &key, value)?;
                resume.forward(action)
            }
            Self::Descriptor {
                object,
                key,
                resume,
            } => {
                let result = match resume.state.realm {
                    Some(realm) => runtime.internal_get_own_property(realm, &object, &key)?,
                    None => NativeConversion::Value(runtime.get_own_property(&object, &key)?),
                };
                resume.descriptor(runtime, result)
            }
            Self::Define {
                object,
                key,
                descriptor,
                resume,
            } => {
                let result = match resume.state.realm {
                    Some(realm) => {
                        runtime.internal_define_own_property(realm, &object, &key, &descriptor)?
                    }
                    None => match runtime.define_own_property_in_realm(
                        None,
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
                    },
                };
                resume.defined(runtime, result)
            }
        }
    }
}

fn complete(action: PropertySetAction) -> Result<SetStep, RuntimeError> {
    Ok(SetStep::Complete(action))
}
fn rejected(reason: PropertySetRejection) -> Result<SetStep, RuntimeError> {
    complete(PropertySetAction::Rejected(reason))
}
fn stored(accepted: bool) -> Result<SetStep, RuntimeError> {
    if accepted {
        complete(PropertySetAction::Complete)
    } else {
        rejected(PropertySetRejection::ReadOnly)
    }
}

impl State {
    fn walk(self, runtime: &Runtime, mut current: ObjectRef) -> Result<SetStep, RuntimeError> {
        loop {
            let same_receiver = matches!(&self.receiver,Value::Object(target) if target==&current);
            match runtime.ordinary_set_probe(&current, &self.key, &self.value, same_receiver)? {
                SetProbe::Stored(accepted) => return stored(accepted),
                SetProbe::Writable => return self.receiver(runtime),
                SetProbe::Setter(set) => {
                    return complete(match set {
                        Some(setter) => PropertySetAction::Call {
                            setter: crate::engine::object::CallableRef::from_validated_object(
                                ObjectRef::from_borrowed_handle(runtime.clone(), setter)?,
                            ),
                            receiver: self.receiver,
                            argument: self.value,
                        },
                        None => PropertySetAction::Rejected(PropertySetRejection::NoSetter),
                    });
                }
                SetProbe::Missing(next) => {
                    let Some(next) = next else {
                        return self.receiver(runtime);
                    };
                    current = next;
                }
                SetProbe::Special(kind) => {
                    if matches!(kind, SpecialKind::Proxy) {
                        if self.realm.is_none() {
                            return Err(RuntimeError::Invariant("exotic Set requires a realm"));
                        }
                        return Ok(SetStep::Proxy {
                            object: current,
                            key: self.key.clone(),
                            value: self.value.clone(),
                            receiver: self.receiver.clone(),
                            resume: SetResume {
                                state: self,
                                phase: Phase::Forward,
                            },
                        });
                    }
                    if self.realm.is_some() && matches!(kind, SpecialKind::ModuleNamespace) {
                        return rejected(PropertySetRejection::ReadOnly);
                    }
                    if self.realm.is_some()
                        && matches!(kind, SpecialKind::TypedArray)
                        && runtime
                            .typed_array_canonical_numeric_index(&self.key)?
                            .is_some()
                    {
                        return Ok(SetStep::Special {
                            object: current.clone(),
                            key: self.key.clone(),
                            value: self.value.clone(),
                            receiver: self.receiver.clone(),
                            resume: SetResume {
                                state: self,
                                phase: Phase::Special(current),
                            },
                        });
                    }
                    return self.special_own(runtime, current);
                }
            }
        }
    }

    fn special_own(self, runtime: &Runtime, current: ObjectRef) -> Result<SetStep, RuntimeError> {
        let same_receiver = matches!(&self.receiver,Value::Object(target) if target==&current);
        if let Some(property) = runtime.get_own_property(&current, &self.key)? {
            match property {
                CompleteOrdinaryPropertyDescriptor::Data { writable, .. } => {
                    if same_receiver
                        && runtime.array_own_key(&current, &self.key)? == ArrayOwnKey::Length
                    {
                        return Ok(SetStep::ArrayLength {
                            object: current,
                            key: self.key.clone(),
                            value: self.value.clone(),
                            resume: SetResume {
                                state: self,
                                phase: Phase::Forward,
                            },
                        });
                    }
                    if !writable {
                        return rejected(PropertySetRejection::ReadOnly);
                    }
                    return self.receiver(runtime);
                }
                CompleteOrdinaryPropertyDescriptor::Accessor { set, .. } => {
                    return complete(match set {
                        Some(setter) => PropertySetAction::Call {
                            setter,
                            receiver: self.receiver,
                            argument: self.value,
                        },
                        None => PropertySetAction::Rejected(PropertySetRejection::NoSetter),
                    });
                }
            }
        }
        match runtime.get_prototype_of(&current)? {
            Some(next) => Ok(SetStep::Continue {
                resume: SetResume {
                    state: self,
                    phase: Phase::Walk(next),
                },
            }),
            None => self.receiver(runtime),
        }
    }

    fn receiver(self, runtime: &Runtime) -> Result<SetStep, RuntimeError> {
        let Value::Object(receiver) = &self.receiver else {
            return rejected(PropertySetRejection::NotObject);
        };
        let receiver = receiver.clone();
        match runtime.ordinary_set_probe(&receiver, &self.key, &self.value, true)? {
            SetProbe::Stored(accepted) => stored(accepted),
            SetProbe::Setter(set) => rejected(if set.is_some() {
                PropertySetRejection::ReadOnly
            } else {
                PropertySetRejection::NoSetter
            }),
            SetProbe::Missing(_) => self.define(receiver, false),
            SetProbe::Special(_) => Ok(SetStep::Descriptor {
                object: receiver,
                key: self.key.clone(),
                resume: SetResume {
                    state: self,
                    phase: Phase::Receiver,
                },
            }),
            SetProbe::Writable => unreachable!("receiver probe commits a writable data slot"),
        }
    }

    fn define(self, receiver: ObjectRef, existing: bool) -> Result<SetStep, RuntimeError> {
        let descriptor = if existing {
            OrdinaryPropertyDescriptor {
                value: DescriptorField::Present(self.value.clone()),
                ..OrdinaryPropertyDescriptor::new()
            }
        } else {
            OrdinaryPropertyDescriptor {
                value: DescriptorField::Present(self.value.clone()),
                writable: DescriptorField::Present(true),
                enumerable: DescriptorField::Present(true),
                configurable: DescriptorField::Present(true),
                ..OrdinaryPropertyDescriptor::new()
            }
        };
        Ok(SetStep::Define {
            object: receiver.clone(),
            key: self.key.clone(),
            descriptor,
            resume: SetResume {
                state: self,
                phase: Phase::Define(receiver),
            },
        })
    }
}

impl SetResume {
    #[cfg(feature = "stack-vm")]
    pub(crate) fn array_length(
        self,
        runtime: &Runtime,
        result: crate::engine::object::operations::ArrayLengthConversion,
    ) -> Result<SetStep, RuntimeError> {
        if !matches!(self.phase, Phase::Forward) {
            return Err(RuntimeError::Invariant(
                "Set continuation received an Array length reply",
            ));
        }
        let action = match result {
            crate::engine::object::operations::ArrayLengthConversion::Throw(value) => {
                PropertySetAction::Throw(value)
            }
            crate::engine::object::operations::ArrayLengthConversion::Length(length) => {
                let Value::Object(object) = &self.state.receiver else {
                    return Err(RuntimeError::Invariant(
                        "Array length receiver lost its object",
                    ));
                };
                runtime.apply_set_array_length(object, &self.state.key, length)?
            }
        };
        complete(action)
    }

    pub(crate) fn advance(self, runtime: &Runtime) -> Result<SetStep, RuntimeError> {
        let Phase::Walk(object) = self.phase else {
            return Err(RuntimeError::Invariant(
                "Set continuation received a walk reply",
            ));
        };
        self.state.walk(runtime, object)
    }
    pub(crate) fn forward(self, action: PropertySetAction) -> Result<SetStep, RuntimeError> {
        if !matches!(self.phase, Phase::Forward) {
            return Err(RuntimeError::Invariant(
                "Set continuation received a forward reply",
            ));
        }
        complete(action)
    }
    pub(crate) fn special(
        self,
        runtime: &Runtime,
        result: Option<NativeConversion<InternalSetResult>>,
    ) -> Result<SetStep, RuntimeError> {
        let Phase::Special(current) = self.phase else {
            return Err(RuntimeError::Invariant(
                "Set continuation received a special reply",
            ));
        };
        match result {
            Some(result) => complete(set_completion(result)),
            None => self.state.special_own(runtime, current),
        }
    }
    pub(crate) fn descriptor(
        self,
        runtime: &Runtime,
        result: NativeConversion<Option<CompleteOrdinaryPropertyDescriptor>>,
    ) -> Result<SetStep, RuntimeError> {
        if !matches!(self.phase, Phase::Receiver) {
            return Err(RuntimeError::Invariant(
                "Set continuation received a descriptor reply",
            ));
        }
        let existing = match result {
            NativeConversion::Value(value) => value,
            NativeConversion::Throw(value) => return complete(PropertySetAction::Throw(value)),
        };
        let Value::Object(receiver) = &self.state.receiver else {
            return Err(RuntimeError::Invariant("Set receiver lost its object"));
        };
        let receiver = receiver.clone();
        match existing {
            Some(CompleteOrdinaryPropertyDescriptor::Data {
                writable: false, ..
            }) => rejected(PropertySetRejection::ReadOnly),
            Some(CompleteOrdinaryPropertyDescriptor::Accessor { set, .. }) => {
                rejected(if set.is_some() {
                    PropertySetRejection::ReadOnly
                } else {
                    PropertySetRejection::NoSetter
                })
            }
            Some(CompleteOrdinaryPropertyDescriptor::Data { .. }) => {
                if runtime.set_arguments_index_value(
                    &receiver,
                    &self.state.key,
                    &self.state.value,
                )? {
                    return complete(PropertySetAction::Complete);
                }
                self.state.define(receiver, true)
            }
            None => self.state.define(receiver, false),
        }
    }
    pub(crate) fn defined(
        self,
        runtime: &Runtime,
        result: NativeConversion<InternalDefineResult>,
    ) -> Result<SetStep, RuntimeError> {
        let Phase::Define(receiver) = self.phase else {
            return Err(RuntimeError::Invariant(
                "Set continuation received a define reply",
            ));
        };
        let rejected_object = match result {
            NativeConversion::Value(InternalDefineResult::Defined) => {
                return complete(PropertySetAction::Complete);
            }
            NativeConversion::Value(InternalDefineResult::RejectedProxyTrap) => {
                return complete(PropertySetAction::RejectedProxyTrap);
            }
            NativeConversion::Throw(value) => return complete(PropertySetAction::Throw(value)),
            NativeConversion::Value(InternalDefineResult::RejectedOrdinary(object)) => Some(object),
        };
        let receiver = rejected_object.as_ref().unwrap_or(&receiver);
        rejected(
            if !runtime.has_own_property(receiver, &self.state.key)?
                && !runtime.is_extensible(receiver)?
            {
                PropertySetRejection::NotExtensible
            } else if matches!(
                runtime.array_own_key(receiver, &self.state.key)?,
                ArrayOwnKey::Index(_)
            ) && !runtime.array_length_state(receiver)?.1
            {
                PropertySetRejection::ArrayLengthReadOnly
            } else {
                PropertySetRejection::ReadOnly
            },
        )
    }
}

impl Runtime {
    pub(crate) fn finish_property_set(
        &self,
        result: NativeConversion<InternalSetResult>,
        key: &PropertyKey,
        strict: bool,
    ) -> Result<crate::engine::vm::Completion, RuntimeError> {
        use crate::engine::api::{Error, ErrorKind};
        use crate::engine::vm::Completion;
        match result {
            NativeConversion::Value(InternalSetResult::Accepted) => {
                Ok(Completion::Return(Value::Undefined))
            }
            NativeConversion::Value(_) if !strict => Ok(Completion::Return(Value::Undefined)),
            NativeConversion::Value(InternalSetResult::RejectedProxyTrap) => {
                Err(Error::new(ErrorKind::Type, "proxy: cannot set property").into())
            }
            NativeConversion::Value(InternalSetResult::Rejected(
                PropertySetRejection::ReadOnly,
            )) => {
                let error = self.native_atom_error(ErrorKind::Type, "'", key, "' is read-only")?;
                Err(error.into())
            }
            NativeConversion::Value(InternalSetResult::Rejected(
                PropertySetRejection::ArrayLengthReadOnly,
            )) => {
                let length = self.intern_property_key("length")?;
                let error =
                    self.native_atom_error(ErrorKind::Type, "'", &length, "' is read-only")?;
                Err(error.into())
            }
            NativeConversion::Value(InternalSetResult::Rejected(
                PropertySetRejection::NotConfigurable,
            )) => Err(Error::new(ErrorKind::Type, "not configurable").into()),
            NativeConversion::Value(InternalSetResult::Rejected(
                PropertySetRejection::NoSetter,
            )) => Err(Error::new(ErrorKind::Type, "no setter for property").into()),
            NativeConversion::Value(InternalSetResult::Rejected(
                PropertySetRejection::NotExtensible,
            )) => Err(Error::new(ErrorKind::Type, "object is not extensible").into()),
            NativeConversion::Value(InternalSetResult::Rejected(
                PropertySetRejection::NotObject,
            )) => Err(Error::new(ErrorKind::Type, "not an object").into()),
            NativeConversion::Throw(value) => Ok(Completion::Throw(value)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn take_descriptor(step: SetStep) -> SetResume {
        let SetStep::Descriptor { resume, .. } = step else {
            panic!("expected receiver descriptor query")
        };
        resume
    }

    #[test]
    fn abandoned_set_receiver_queries_keep_then_release_target_receiver_and_value() {
        for after_descriptor in [false, true] {
            let runtime = Runtime::new();
            let weak = std::rc::Rc::downgrade(&runtime.0);
            let context = runtime.new_context();
            let target = runtime.new_object(None).unwrap();
            let target_id = target.object_id();
            let handler = runtime.new_object(None).unwrap();
            let handler_id = handler.object_id();
            let NativeConversion::Value(receiver) = runtime
                .new_proxy(
                    context.realm,
                    Value::Object(runtime.new_object(None).unwrap()),
                    Value::Object(handler),
                )
                .unwrap()
            else {
                panic!("proxy allocation failed")
            };
            let receiver_id = receiver.object_id();
            let value = runtime.new_object(None).unwrap();
            let value_id = value.object_id();
            let mut step = SetStep::start(
                &runtime,
                Some(context.realm),
                target,
                runtime.intern_property_key("x").unwrap(),
                Value::Object(value),
                Value::Object(receiver),
            )
            .unwrap();
            if after_descriptor {
                step = take_descriptor(step)
                    .descriptor(&runtime, NativeConversion::Value(None))
                    .unwrap();
            }
            runtime.run_gc().unwrap();
            for id in [target_id, handler_id, receiver_id, value_id] {
                assert!(runtime.0.state.borrow().heap.object(id).is_ok());
            }
            drop(step);
            runtime.run_gc().unwrap();
            for id in [target_id, handler_id, receiver_id, value_id] {
                assert!(runtime.0.state.borrow().heap.object(id).is_err());
            }
            drop(context);
            drop(runtime);
            assert!(weak.upgrade().is_none());
        }
    }

    #[test]
    fn set_rejects_an_unrelated_reply_before_mutating_the_receiver() {
        let runtime = Runtime::new();
        let context = runtime.new_context();
        let target = runtime.new_object(None).unwrap();
        let NativeConversion::Value(receiver) = runtime
            .new_proxy(
                context.realm,
                Value::Object(runtime.new_object(None).unwrap()),
                Value::Object(runtime.new_object(None).unwrap()),
            )
            .unwrap()
        else {
            panic!("proxy allocation failed")
        };
        let resume = take_descriptor(
            SetStep::start(
                &runtime,
                Some(context.realm),
                target,
                runtime.intern_property_key("x").unwrap(),
                Value::Int(42),
                Value::Object(receiver),
            )
            .unwrap(),
        );
        assert!(
            resume
                .defined(
                    &runtime,
                    NativeConversion::Value(InternalDefineResult::Defined)
                )
                .is_err()
        );
        assert_eq!(runtime.0.proxy_method_depth.get(), 0);
    }
}
