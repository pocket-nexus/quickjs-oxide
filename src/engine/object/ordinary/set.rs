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

enum InitialSet {
    Action(PropertySetAction),
    Pending(SetProbe),
}

// Both owning and borrowed starts enter with an active RuntimeOperation.
// This is the only validation/probe/primitive-conversion selector.
#[inline(always)]
fn initial_set(
    runtime: &Runtime,
    realm: Option<ContextId>,
    object: &ObjectRef,
    key: &PropertyKey,
    value: &Value,
    receiver: &Value,
) -> Result<InitialSet, RuntimeError> {
    runtime.validate_object_and_key(object, key)?;
    runtime.validate_value_domain(value, "property value")?;
    runtime.validate_value_domain(receiver, "property receiver")?;
    if realm.is_none()
        && (runtime.is_proxy_object(object)?
            || matches!(receiver,Value::Object(object) if runtime.is_proxy_object(object)?))
    {
        return Err(RuntimeError::Invariant("exotic Set requires a realm"));
    }
    // Probe the initial receiver before constructing a waiting owner. A
    // completed data write does not need any continuation roots.
    let same_receiver = matches!(receiver, Value::Object(target) if target == object);
    let probe = runtime.ordinary_set_probe(object, key, value, same_receiver)?;
    if let SetProbe::Stored(accepted) = probe {
        return Ok(InitialSet::Action(stored_action(accepted)));
    }
    #[cfg(feature = "stack-vm")]
    if matches!(probe, SetProbe::Special(SpecialKind::TypedArray))
        && let Some(realm) = realm
        && !matches!(value, Value::Object(_))
        && let Some(result) =
            runtime.try_typed_array_set_primitive(realm, object, key, value, receiver)?
    {
        // The shared converter reacquires the view before writing. Object
        // conversion, Proxy receivers and non-canonical keys retain their
        // original state machine; no callback is hidden in this shortcut.
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_execution_event(
            "typed_write_completed_without_set_state",
        );
        return Ok(InitialSet::Action(set_completion(match result {
            NativeConversion::Value(_) => NativeConversion::Value(InternalSetResult::Accepted),
            NativeConversion::Throw(value) => NativeConversion::Throw(value),
        })));
    }
    Ok(InitialSet::Pending(probe))
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
        let mut waiting = None;
        let action = Self::start_into(runtime, realm, object, key, value, receiver, |step| {
            waiting = Some(step);
        })?;
        match action {
            Some(action) => complete(action),
            None => waiting.ok_or(RuntimeError::Invariant(
                "Set start omitted its waiting step",
            )),
        }
    }

    /// Return an initial action without constructing the wide waiting enum.
    /// The sink receives exactly one pending state after the initial operation
    /// guard is released, so it may advance shared waiting phases immediately.
    /// Validation, storage probes and typed conversion stay in this kernel.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn start_into(
        runtime: &Runtime,
        realm: Option<ContextId>,
        object: ObjectRef,
        key: PropertyKey,
        value: Value,
        receiver: Value,
        waiting: impl FnMut(Self),
    ) -> Result<Option<PropertySetAction>, RuntimeError> {
        let _operation = runtime.operation();
        let probe = match initial_set(runtime, realm, &object, &key, &value, &receiver)? {
            InitialSet::Action(action) => return Ok(Some(action)),
            InitialSet::Pending(probe) => probe,
        };
        start_waiting(
            runtime, realm, object, key, value, receiver, probe, waiting, _operation,
        )
    }

    /// Borrow the finalization key and use the receiver's existing target root.
    /// Only a pending state acquires a separate target and domain-key owner.
    #[cfg(feature = "stack-vm")]
    pub(crate) fn start_receiver_into(
        runtime: &Runtime,
        realm: ContextId,
        key: &PropertyKey,
        value: Value,
        receiver: Value,
        waiting: impl FnMut(Self),
    ) -> Result<Option<PropertySetAction>, RuntimeError> {
        let operation = runtime.operation();
        let selected = match &receiver {
            Value::Object(object) => {
                initial_set(runtime, Some(realm), object, key, &value, &receiver)
            }
            _ => Err(RuntimeError::Invariant(
                "borrowed Set target is not an object",
            )),
        };
        if let Ok(InitialSet::Pending(probe)) = selected {
            let Value::Object(object) = &receiver else {
                unreachable!()
            };
            return start_waiting(
                runtime,
                Some(realm),
                object.clone(),
                key.clone(),
                value,
                receiver,
                probe,
                waiting,
                operation,
            );
        }
        // The owning start drops its guard, receiver duplicate, value, domain
        // key duplicate, then target. With duplicate owners omitted, preserve
        // the final value-before-target release and keep the VM key alive.
        drop(operation);
        drop(value);
        drop(receiver);
        selected.map(|selected| match selected {
            InitialSet::Action(action) => Some(action),
            InitialSet::Pending(_) => unreachable!(),
        })
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
                } if matches!(runtime.array_own_key(&object, &key)?, ArrayOwnKey::Index(_))
                    || matches!(runtime.ordinary_property_flags(&object, &key)?, Some(None)) =>
                {
                    // An already-selected new ordinary own property needs no
                    // callback. Prototype setters/Proxy traversal were handled
                    // before selecting Define; existing/lazy slots stay on the
                    // general protocol. Genuine Array indices also need no
                    // length/value coercion. Keep the shared definition kernel
                    // for flags, ordering, extensibility and array transitions.
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
                    #[cfg(feature = "profiling")]
                    crate::engine::api::profiling::record_owned_execution_event(
                        "set_definition_completed_without_query",
                    );
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

// State construction and wide SetStep transport belong only to this outlined
// branch. The operation guard leaves before handing a pending state to its
// caller, matching the original start-return/advance boundary.
#[inline(never)]
#[allow(clippy::too_many_arguments)]
fn start_waiting(
    runtime: &Runtime,
    realm: Option<ContextId>,
    object: ObjectRef,
    key: PropertyKey,
    value: Value,
    receiver: Value,
    probe: SetProbe,
    mut waiting: impl FnMut(SetStep),
    operation: crate::engine::heap::runtime::RuntimeOperation<'_>,
) -> Result<Option<PropertySetAction>, RuntimeError> {
    let step = State {
        realm,
        _target: object.clone(),
        key,
        value,
        receiver,
    }
    .walk_probe(runtime, object, probe)?;
    drop(operation);
    match step {
        SetStep::Complete(action) => Ok(Some(action)),
        step => {
            waiting(step);
            Ok(None)
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
    complete(stored_action(accepted))
}
fn stored_action(accepted: bool) -> PropertySetAction {
    if accepted {
        PropertySetAction::Complete
    } else {
        PropertySetAction::Rejected(PropertySetRejection::ReadOnly)
    }
}

impl State {
    fn walk(self, runtime: &Runtime, current: ObjectRef) -> Result<SetStep, RuntimeError> {
        let same_receiver = matches!(&self.receiver,Value::Object(target) if target==&current);
        let probe = runtime.ordinary_set_probe(&current, &self.key, &self.value, same_receiver)?;
        self.walk_probe(runtime, current, probe)
    }

    fn walk_probe(
        self,
        runtime: &Runtime,
        mut current: ObjectRef,
        mut probe: SetProbe,
    ) -> Result<SetStep, RuntimeError> {
        loop {
            match probe {
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
                    let same_receiver =
                        matches!(&self.receiver,Value::Object(target) if target==&current);
                    probe = runtime.ordinary_set_probe(
                        &current,
                        &self.key,
                        &self.value,
                        same_receiver,
                    )?;
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

    #[cfg(feature = "stack-vm")]
    #[test]
    fn initial_actions_bypass_waiting_transport_and_match_owned_wrapper() {
        for entry in 0..3 {
            let runtime = Runtime::new();
            let mut context = runtime.new_context();
            for (source, key, value, expected) in [
                ("({x:1})", "x", "42", "stored"),
                ("Object.freeze({x:1})", "x", "42", "rejected"),
                ("new Uint8Array(1)", "0", "257", "stored"),
                ("new Uint8Array(1)", "-0", "Symbol()", "throw"),
                ("new BigInt64Array(1)", "1", "1", "throw"),
            ] {
                let Value::Object(object) = context.eval(source).unwrap() else {
                    panic!("expected target");
                };
                let key = runtime.intern_property_key(key).unwrap();
                let value = context.eval(value).unwrap();
                let receiver = Value::Object(object.clone());
                let action = if entry == 0 {
                    let SetStep::Complete(action) =
                        SetStep::start(&runtime, Some(context.realm), object, key, value, receiver)
                            .unwrap()
                    else {
                        panic!("initial action constructed waiting state");
                    };
                    action
                } else if entry == 2 {
                    drop(object);
                    SetStep::start_receiver_into(
                        &runtime,
                        context.realm,
                        &key,
                        value,
                        receiver,
                        |_| panic!("initial action reached waiting sink"),
                    )
                    .unwrap()
                    .expect("expected immediate action")
                } else {
                    SetStep::start_into(
                        &runtime,
                        Some(context.realm),
                        object,
                        key,
                        value,
                        receiver,
                        |_| panic!("initial action reached waiting sink"),
                    )
                    .unwrap()
                    .expect("expected immediate action")
                };
                assert!(matches!(
                    (expected, action),
                    ("stored", PropertySetAction::Complete)
                        | (
                            "rejected",
                            PropertySetAction::Rejected(PropertySetRejection::ReadOnly)
                        )
                        | ("throw", PropertySetAction::Throw(_))
                ));
            }
            assert!(runtime.0.state.borrow().active_frames.is_empty());
        }
    }

    #[cfg(feature = "stack-vm")]
    #[test]
    fn initial_waiting_transport_preserves_conversion_throw_and_roots() {
        for entry in 0..3 {
            let runtime = Runtime::new();
            let mut context = runtime.new_context();
            let Value::Object(object) = context.eval("globalThis.trace=''; globalThis.marker={}; globalThis.target=new Uint8Array(1); target").unwrap() else {
                panic!("expected target");
            };
            let value = context
                .eval("({valueOf(){trace+='v';throw marker}})")
                .unwrap();
            let marker = context.eval("marker").unwrap();
            let key = runtime.intern_property_key("0").unwrap();
            let receiver = Value::Object(object.clone());
            let mut step = if entry == 0 {
                SetStep::start(&runtime, Some(context.realm), object, key, value, receiver).unwrap()
            } else {
                let mut waiting = None;
                let sink = |step| {
                    assert!(waiting.is_none(), "waiting sink called twice");
                    waiting = Some(step);
                };
                let action = if entry == 2 {
                    drop(object);
                    SetStep::start_receiver_into(
                        &runtime,
                        context.realm,
                        &key,
                        value,
                        receiver,
                        sink,
                    )
                } else {
                    SetStep::start_into(
                        &runtime,
                        Some(context.realm),
                        object,
                        key,
                        value,
                        receiver,
                        sink,
                    )
                }
                .unwrap();
                assert!(action.is_none());
                waiting.expect("conversion state missing")
            };
            assert_eq!(
                context.eval("trace").unwrap(),
                Value::String(crate::engine::value::JsString::from_static(""))
            );
            runtime.run_gc().unwrap();
            loop {
                match step {
                    SetStep::Complete(PropertySetAction::Throw(value)) => {
                        assert_eq!(value, marker);
                        break;
                    }
                    SetStep::Complete(_) => panic!("conversion throw was lost"),
                    pending => step = pending.finish_sync(&runtime).unwrap(),
                }
            }
            assert_eq!(
                context.eval("trace==='v' && target[0]===0").unwrap(),
                Value::Bool(true)
            );
            assert!(runtime.0.state.borrow().active_frames.is_empty());
        }
    }

    #[cfg(feature = "stack-vm")]
    #[test]
    fn outlined_waiting_delivery_drains_initial_operation_before_advancing() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let Value::Object(object) = context.eval("new Proxy({}, {})").unwrap() else {
            panic!("expected proxy");
        };
        let target_id = object.object_id();
        let receiver = Value::Object(object.clone());
        let value = runtime.new_object(None).unwrap();
        let value_id = value.object_id();
        let key = runtime.intern_property_key("x").unwrap();
        let released = runtime.new_object(None).unwrap();
        let released_id = released.object_id();
        let operation = runtime.operation();
        {
            let _borrow = runtime.0.state.borrow();
            drop(released);
        }
        assert!(runtime.0.deferred_references.has_pending());
        let mut delivered = false;
        let action = start_waiting(
            &runtime,
            Some(context.realm),
            object,
            key,
            Value::Object(value),
            receiver,
            SetProbe::Special(SpecialKind::Proxy),
            |step| {
                assert!(!delivered);
                delivered = true;
                // No new Runtime operation has run since queuing the release.
                // The transferred initial guard must drain before this sink.
                assert!(!runtime.0.deferred_references.has_pending());
                assert!(runtime.0.state.borrow().heap.object(released_id).is_err());
                runtime.run_gc().unwrap();
                for id in [target_id, value_id] {
                    assert!(runtime.0.state.borrow().heap.object(id).is_ok());
                }
                let step = step.advance_without_callback(&runtime).unwrap();
                assert!(matches!(&step, SetStep::Proxy { .. }));
                drop(step);
            },
            operation,
        )
        .unwrap();
        assert!(action.is_none() && delivered);
        runtime.run_gc().unwrap();
        for id in [target_id, value_id] {
            assert!(runtime.0.state.borrow().heap.object(id).is_err());
        }
    }

    #[cfg(feature = "stack-vm")]
    #[test]
    fn outlined_start_keeps_immediate_throw_root_until_its_action_is_released() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let Value::Object(object) = context.eval("new Uint8Array(1)").unwrap() else {
            panic!("expected typed array");
        };
        let receiver = Value::Object(object.clone());
        let value = context.eval("Symbol()").unwrap();
        let action = SetStep::start_into(
            &runtime,
            Some(context.realm),
            object,
            runtime.intern_property_key("-0").unwrap(),
            value,
            receiver,
            |_| panic!("primitive error entered waiting sink"),
        )
        .unwrap()
        .expect("expected immediate action");
        let PropertySetAction::Throw(Value::Object(error)) = &action else {
            panic!("expected rooted TypeError");
        };
        let id = error.object_id();
        runtime.run_gc().unwrap();
        assert!(runtime.0.state.borrow().heap.object(id).is_ok());
        drop(action);
        runtime.run_gc().unwrap();
        assert!(runtime.0.state.borrow().heap.object(id).is_err());
    }

    #[cfg(feature = "stack-vm")]
    #[test]
    fn borrowed_set_validates_target_key_and_value_before_storage() {
        let runtime = Runtime::new();
        let foreign = Runtime::new();
        let context = runtime.new_context();
        for (foreign_target, foreign_key, expected) in [
            (true, true, "object"),
            (false, true, "property key"),
            (false, false, "property value"),
        ] {
            let target_runtime = if foreign_target { &foreign } else { &runtime };
            let key_runtime = if foreign_key { &foreign } else { &runtime };
            let receiver = Value::Object(target_runtime.new_object(None).unwrap());
            let key = key_runtime.intern_property_key("x").unwrap();
            let value = Value::Object(foreign.new_object(None).unwrap());
            let result = SetStep::start_receiver_into(
                &runtime,
                context.realm,
                &key,
                value,
                receiver,
                |_| panic!("invalid input reached waiting sink"),
            );
            assert!(matches!(result, Err(RuntimeError::WrongRuntime(role)) if role == expected));
            assert!(!runtime.0.deferred_references.has_pending());
        }
    }

    #[cfg(feature = "stack-vm")]
    #[test]
    fn local_new_property_definition_uses_selected_receiver_and_shared_rejection() {
        for (source, name, rejected) in [
            ("({})", "x", false),
            ("Object.create({x: 9})", "x", false),
            ("Object.create(null)", "__proto__", false),
            ("Object.preventExtensions({})", "x", true),
        ] {
            let runtime = Runtime::new();
            let mut context = runtime.new_context();
            let Value::Object(target) = context.eval(source).unwrap() else {
                panic!("target")
            };
            let value = context.eval("({valueOf(){throw 99}})").unwrap();
            let key = runtime.intern_property_key(name).unwrap();
            let mut deliveries = 0;
            let mut action = None;
            let initial = SetStep::start_receiver_into(
                &runtime,
                context.realm,
                &key,
                value.clone(),
                Value::Object(target.clone()),
                |step| {
                    deliveries += 1;
                    let SetStep::Complete(completed) =
                        step.advance_without_callback(&runtime).unwrap()
                    else {
                        panic!("new ordinary definition escaped to query");
                    };
                    action = Some(completed);
                },
            )
            .unwrap();
            assert!(initial.is_none());
            assert_eq!(deliveries, 1);
            if rejected {
                assert!(matches!(
                    action,
                    Some(PropertySetAction::Rejected(
                        PropertySetRejection::NotExtensible
                    ))
                ));
                assert!(runtime.get_own_property(&target, &key).unwrap().is_none());
            } else {
                assert!(matches!(action, Some(PropertySetAction::Complete)));
                let Some(CompleteOrdinaryPropertyDescriptor::Data {
                    value: stored,
                    writable,
                    enumerable,
                    configurable,
                }) = runtime.get_own_property(&target, &key).unwrap()
                else {
                    panic!("own data")
                };
                assert_eq!(stored, value);
                assert!(writable && enumerable && configurable);
            }
        }
    }

    #[cfg(feature = "stack-vm")]
    #[test]
    fn local_new_property_definition_preserves_prototype_callbacks_and_key_order() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        #[cfg(feature = "profiling")]
        let profile = crate::engine::api::profiling::CostProfile::start();
        assert_eq!(
            context
                .eval(
                    r#"
            (() => {
                let calls = 0;
                const inherited = Object.create({set x(value) { calls++; this.y = value; }});
                inherited.x = 7;
                const proxy = new Proxy({}, {set(target,key,value,receiver) {
                    calls++; return Reflect.set(target,key,value,receiver);
                }});
                const receiver = Object.create(proxy);
                receiver.a = 8;
                const o = {}, symbol = Symbol();
                o.b = 1; o[3] = 3; o.a = 2; o[1] = 1; o[symbol] = 4;
                const keys = Reflect.ownKeys(o);
                return calls === 2 && inherited.y === 7
                    && !Object.hasOwn(inherited, 'x') && receiver.a === 8
                    && keys.length === 5 && keys[0] === '1' && keys[1] === '3'
                    && keys[2] === 'b' && keys[3] === 'a' && keys[4] === symbol;
            })()
        "#
                )
                .unwrap(),
            Value::Bool(true)
        );
        #[cfg(feature = "profiling")]
        assert!(
            profile
                .snapshot()
                .owned_execution_events
                .get("set_definition_completed_without_query")
                .copied()
                .unwrap_or(0)
                > 0
        );
        assert!(runtime.0.state.borrow().active_frames.is_empty());
    }

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
