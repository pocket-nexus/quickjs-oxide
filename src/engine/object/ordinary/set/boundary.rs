//! Borrowed boundary adapters around the sole raw Set producer.
use super::*;

/// Inputs are protected before operation admission. Existing target/key roots
/// retain their public domain contract until validation; no Runtime is cloned
/// solely to provide raw-input abandonment.
struct StartGuard<'a> {
    runtime: &'a Runtime,
    value: Option<JsValue>,
    receiver: Option<JsValue>,
}
impl StartGuard<'_> {
    fn retire(&mut self) -> Result<(), RuntimeError> {
        if self.runtime.skip_cleanup() {
            return Err(RuntimeError::Poisoned);
        }
        for edge in [&mut self.value, &mut self.receiver] {
            if let Some(value) = edge.take() {
                self.runtime.release_jsvalue(value)?;
                self.runtime.check_poison()?;
            }
        }
        Ok(())
    }
}
impl Drop for StartGuard<'_> {
    fn drop(&mut self) {
        if !self.runtime.skip_cleanup() {
            let _ = self.retire();
        }
    }
}

/// A selected terminal boundary result stays owned across pressure service.
struct PublishedBoundary<'a> {
    runtime: &'a Runtime,
    step: Option<SetStep>,
}
impl Drop for PublishedBoundary<'_> {
    fn drop(&mut self) {
        if !self.runtime.skip_cleanup()
            && let Some(step) = self.step.take()
        {
            step.release(self.runtime);
        }
    }
}

impl SetAction {
    pub(crate) fn into_boundary(self, runtime: &Runtime) -> PropertySetAction {
        match self {
            Self::Complete => PropertySetAction::Complete,
            Self::Rejected(reason) => PropertySetAction::Rejected(reason),
            Self::RejectedProxyTrap => PropertySetAction::RejectedProxyTrap,
            Self::Throw(value) => PropertySetAction::Throw(value),
            Self::Call {
                function,
                receiver,
                argument,
            } => PropertySetAction::Call {
                payload: Box::new(crate::engine::object::operations::PropertySetterCall::new(
                    runtime,
                    crate::engine::object::CallableRef::from_validated_object(
                        ObjectRef::from_owned_handle(runtime.clone(), function),
                    ),
                    receiver,
                    argument,
                )),
            },
        }
    }
    pub(crate) fn from_boundary(action: PropertySetAction) -> Self {
        match action {
            PropertySetAction::Complete => Self::Complete,
            PropertySetAction::Rejected(reason) => Self::Rejected(reason),
            PropertySetAction::RejectedProxyTrap => Self::RejectedProxyTrap,
            PropertySetAction::Throw(value) => Self::Throw(value),
            PropertySetAction::Call { payload } => {
                let (function, receiver, argument) = payload.into_parts();
                Self::Call {
                    function: function.into_object().into_execution_handle(),
                    receiver,
                    argument,
                }
            }
        }
    }
}

impl SetStep {
    pub(crate) fn from_progress(runtime: &Runtime, progress: SetProgress) -> Self {
        match progress {
            SetProgress::Complete(action) => Self::Complete(action.into_boundary(runtime)),
            SetProgress::CyclePublished(action) => {
                Self::CyclePublishedComplete(action.into_boundary(runtime))
            }
            SetProgress::Waiting { phase, resume } => match phase {
                SetWait::Continue => Self::Continue { resume },
                SetWait::Proxy => Self::Proxy { resume },
                SetWait::Special => Self::Special { resume },
                SetWait::ArrayLength => Self::ArrayLength { resume },
                SetWait::Descriptor => Self::Descriptor { resume },
                SetWait::Define => Self::Define { resume },
            },
        }
    }
    pub(crate) fn into_progress(self) -> SetProgress {
        match self {
            Self::Complete(action) => SetProgress::Complete(SetAction::from_boundary(action)),
            Self::CyclePublishedComplete(action) => {
                SetProgress::CyclePublished(SetAction::from_boundary(action))
            }
            Self::Continue { resume } => SetProgress::Waiting {
                phase: SetWait::Continue,
                resume,
            },
            Self::Proxy { resume } => SetProgress::Waiting {
                phase: SetWait::Proxy,
                resume,
            },
            Self::Special { resume } => SetProgress::Waiting {
                phase: SetWait::Special,
                resume,
            },
            Self::ArrayLength { resume } => SetProgress::Waiting {
                phase: SetWait::ArrayLength,
                resume,
            },
            Self::Descriptor { resume } => SetProgress::Waiting {
                phase: SetWait::Descriptor,
                resume,
            },
            Self::Define { resume } => SetProgress::Waiting {
                phase: SetWait::Define,
                resume,
            },
        }
    }
    pub(crate) fn release(self, runtime: &Runtime) {
        let _ = self.into_progress().retire_at_boundary(runtime);
    }

    /// Root operation construction converts public inputs in the original
    /// value-then-receiver order, protecting the first produced raw owner.
    pub(crate) fn prepare_inputs(
        runtime: &Runtime,
        value: Value,
        receiver: Value,
    ) -> Result<(JsValue, JsValue), RuntimeError> {
        let _unwind = runtime.unwind_guard();
        let mut inputs = StartGuard {
            runtime,
            value: Some(runtime.into_jsvalue(value)?),
            receiver: None,
        };
        match runtime.into_jsvalue(receiver) {
            Ok(receiver) => inputs.receiver = Some(receiver),
            Err(error) => {
                inputs.retire()?;
                return Err(error);
            }
        }
        Ok((
            inputs.value.take().expect("converted Set value"),
            inputs.receiver.take().expect("converted Set receiver"),
        ))
    }

    pub(crate) fn start(
        runtime: &Runtime,
        realm: Option<ContextId>,
        object: ObjectRef,
        key: PropertyKey,
        value: JsValue,
        receiver: JsValue,
    ) -> Result<Self, RuntimeError> {
        let mut inputs = StartGuard {
            runtime,
            value: Some(value),
            receiver: Some(receiver),
        };
        let _operation = match runtime.operation() {
            Ok(operation) => operation,
            Err(error) => {
                inputs.retire()?;
                return Err(error);
            }
        };
        if let Err(error) = runtime.validate_object_and_key(&object, &key) {
            inputs.retire()?;
            return Err(error);
        }
        // Transfer root owners before acquiring State; the empty public roots
        // dispose their Runtime headers outside this lease.
        let target = object.into_execution_handle();
        let atom = key.into_atom();
        let _unwind = runtime.unwind_guard();
        let progress = runtime.0.state.borrow_mut().start_set_owned(
            &runtime.0.poisoned,
            realm,
            target,
            atom,
            inputs.value.take().expect("Set value"),
            inputs.receiver.take().expect("Set receiver"),
        )?;
        runtime.check_poison()?;
        Ok(Self::from_progress(runtime, progress))
    }

    /// The synchronous caller keeps these public inputs live through every
    /// reply. Only a real waiting request needs separate target/key owners.
    pub(crate) fn start_borrowed(
        runtime: &Runtime,
        realm: Option<ContextId>,
        object: &ObjectRef,
        key: &PropertyKey,
        value: JsValue,
        receiver: JsValue,
    ) -> Result<Self, RuntimeError> {
        let mut inputs = StartGuard {
            runtime,
            value: Some(value),
            receiver: Some(receiver),
        };
        let _operation = match runtime.operation() {
            Ok(operation) => operation,
            Err(error) => {
                inputs.retire()?;
                return Err(error);
            }
        };
        if let Err(error) = runtime.validate_object_and_key(object, key) {
            inputs.retire()?;
            return Err(error);
        }
        let _unwind = runtime.unwind_guard();
        let progress = runtime.0.state.borrow_mut().start_set_borrowed(
            &runtime.0.poisoned,
            realm,
            object.object_id(),
            key.atom(),
            inputs.value.take().expect("Set value"),
            inputs.receiver.take().expect("Set receiver"),
        )?;
        runtime.check_poison()?;
        Ok(Self::from_progress(runtime, progress))
    }

    // Existing admission/delivery witnesses use this forwarding adapter only.
    // Production uses the canonical raw producer through its actual consumer.
    #[cfg(test)]
    pub(crate) fn start_into(
        runtime: &Runtime,
        realm: Option<ContextId>,
        object: ObjectRef,
        key: PropertyKey,
        value: JsValue,
        receiver: JsValue,
        mut waiting: impl FnMut(Self),
    ) -> Result<Option<PropertySetAction>, RuntimeError> {
        match Self::start(runtime, realm, object, key, value, receiver)? {
            Self::Complete(action) => Ok(Some(action)),
            Self::CyclePublishedComplete(action) => {
                // Preserve this real producer fact when transported to Query.
                waiting(Self::CyclePublishedComplete(action));
                Ok(None)
            }
            step => {
                waiting(step);
                Ok(None)
            }
        }
    }

    pub(crate) fn start_receiver_into(
        runtime: &Runtime,
        realm: ContextId,
        key: &PropertyKey,
        value: JsValue,
        receiver: JsValue,
        mut waiting: impl FnMut(Self),
    ) -> Result<Option<PropertySetAction>, RuntimeError> {
        let mut inputs = StartGuard {
            runtime,
            value: Some(value),
            receiver: Some(receiver),
        };
        let operation = match runtime.operation() {
            Ok(operation) => operation,
            Err(error) => {
                inputs.retire()?;
                return Err(error);
            }
        };
        if !key.belongs_to(runtime) {
            inputs.retire()?;
            return Err(RuntimeError::WrongRuntime("property key"));
        }
        let Some(JsValue::Object(target)) = inputs.receiver.as_ref() else {
            inputs.retire()?;
            return Err(RuntimeError::Invariant(
                "borrowed Set target is not an object",
            ));
        };
        let target = *target;
        let _unwind = runtime.unwind_guard();
        let progress = SetOperands::start_in_state(
            &mut runtime.0.state.borrow_mut(),
            &runtime.0.poisoned,
            Some(realm),
            target,
            None,
            key.atom(),
            None,
            inputs.value.take().expect("Set value"),
            inputs.receiver.take().expect("Set receiver"),
        )?;
        drop(operation);
        runtime.check_poison()?;
        match Self::from_progress(runtime, progress) {
            Self::Complete(action) => Ok(Some(action)),
            step => {
                waiting(step);
                Ok(None)
            }
        }
    }

    /// The raw producer already drains every callback-free storage phase. No
    /// primitive eligibility corridor or second advance algorithm exists.
    pub(crate) fn advance_without_callback(self, _runtime: &Runtime) -> Result<Self, RuntimeError> {
        Ok(self)
    }

    pub(crate) fn finish_sync(self, runtime: &Runtime) -> Result<Self, RuntimeError> {
        let _unwind = runtime.unwind_guard();
        runtime.check_poison()?;
        match self {
            Self::Complete(_) => Ok(self),
            Self::CyclePublishedComplete(action) => {
                let mut result = PublishedBoundary {
                    runtime,
                    step: Some(Self::Complete(action)),
                };
                runtime.collect_if_requested()?;
                Ok(result.step.take().expect("published Set completion"))
            }
            Self::Continue { resume } => resume.advance(runtime),
            Self::Proxy { mut resume } => {
                let realm = match resume.inputs.realm {
                    Some(realm) => realm,
                    None => {
                        resume.retire_at_boundary(runtime)?;
                        return Err(RuntimeError::Invariant("exotic Set requires a realm"));
                    }
                };
                let object = resume.take_object(runtime);
                let key = resume.take_key(runtime);
                let value = resume.take_value();
                let receiver = resume.take_receiver();
                let result = runtime.proxy_set(realm, &object, &key, value, receiver);
                match result {
                    Ok(result) => resume.forward(runtime, set_completion(result)),
                    Err(error) => {
                        resume.retire_at_boundary(runtime)?;
                        Err(error)
                    }
                }
            }
            Self::Special { resume } => resume.finish_special_boundary(runtime),
            Self::ArrayLength { mut resume } => {
                let object = resume.take_object(runtime);
                let key = resume.take_key(runtime);
                let realm = resume.inputs.realm;
                let value = resume.take_value();
                let initial = resume.take_array_length_initial();
                let result = runtime.prepare_set_array_length(realm, &object, &key, value, initial);
                match result {
                    Ok(action) => resume.forward(runtime, action),
                    Err(error) => {
                        resume.retire_at_boundary(runtime)?;
                        Err(error)
                    }
                }
            }
            Self::Descriptor { mut resume } => {
                let object = resume.take_object(runtime);
                let key = resume.take_key(runtime);
                let result = match resume.inputs.realm {
                    Some(realm) => runtime.internal_get_own_property_owned(realm, &object, &key),
                    None => runtime
                        .get_own_property_owned(&object, &key)
                        .map(NativeConversion::Value),
                };
                match result {
                    Ok(result) => resume.descriptor(runtime, result),
                    Err(error) => {
                        resume.retire_at_boundary(runtime)?;
                        Err(error)
                    }
                }
            }
            Self::Define { mut resume } => {
                let object = resume.take_object(runtime);
                let key = resume.take_key(runtime);
                let descriptor = resume.take_descriptor(runtime);
                let result = match resume.inputs.realm {
                    Some(realm) => {
                        runtime.internal_define_owned_property(realm, &object, &key, descriptor)
                    }
                    None => runtime
                        .define_owned_property_in_realm(None, &object, &key, &descriptor)
                        .map(|result| match result {
                            PropertyDefineOutcome::Defined(true) => {
                                NativeConversion::Value(InternalDefineResult::Defined)
                            }
                            PropertyDefineOutcome::Defined(false) => NativeConversion::Value(
                                InternalDefineResult::RejectedOrdinary(object),
                            ),
                            PropertyDefineOutcome::Throw(value) => NativeConversion::Throw(value),
                        }),
                };
                match result {
                    Ok(result) => resume.defined(runtime, result),
                    Err(error) => {
                        resume.retire_at_boundary(runtime)?;
                        Err(error)
                    }
                }
            }
        }
    }
}

impl RuntimeState {
    pub(crate) fn start_set_owned(
        &mut self,
        poisoned: &Cell<bool>,
        realm: Option<ContextId>,
        object: ObjectId,
        atom: Atom,
        value: JsValue,
        receiver: JsValue,
    ) -> Result<SetProgress, RuntimeError> {
        SetOperands::start_in_state(
            self,
            poisoned,
            realm,
            object,
            Some(object),
            atom,
            Some(atom),
            value,
            receiver,
        )
    }
    pub(crate) fn start_set_borrowed(
        &mut self,
        poisoned: &Cell<bool>,
        realm: Option<ContextId>,
        object: ObjectId,
        atom: Atom,
        value: JsValue,
        receiver: JsValue,
    ) -> Result<SetProgress, RuntimeError> {
        SetOperands::start_in_state(
            self, poisoned, realm, object, None, atom, None, value, receiver,
        )
    }
}
