//! Selected-effect replies reuse the same State Set advance body.
use super::*;
use crate::engine::atom::AtomIdx;

pub(crate) enum SetDefinitionReply {
    Defined,
    RejectedProxyTrap,
    Rejected(ObjectId),
    Throw(JsValue),
}

/// Only actual legacy reply admission borrows Runtime. Both the selected reply
/// and its domain remain armed until a lease is available; ordinary contention
/// uses the established deferred-release coordinator, without poisoning.
enum Reply {
    Advance,
    Forward(SetAction),
    Special(Option<NativeConversion<InternalSetResult>>),
    Length(crate::engine::object::operations::ArrayLengthConversion),
    Descriptor(NativeConversion<Option<CompletePropertyDescriptor<RawValue>>>),
    Defined(SetDefinitionReply),
    Shared(TypedOwnWord),
}

impl Reply {
    fn retire_at_boundary(self, runtime: &Runtime) -> Result<(), RuntimeError> {
        let release = |value| {
            runtime.release_jsvalue(value)?;
            runtime.check_poison()
        };
        match self {
            Self::Forward(action) => action.retire_at_boundary(runtime),
            Self::Special(Some(NativeConversion::Throw(value)))
            | Self::Length(crate::engine::object::operations::ArrayLengthConversion::Throw(
                value,
            ))
            | Self::Descriptor(NativeConversion::Throw(value))
            | Self::Defined(SetDefinitionReply::Throw(value)) => release(value),
            Self::Defined(SetDefinitionReply::Rejected(object)) => release(JsValue::Object(object)),
            Self::Descriptor(NativeConversion::Value(Some(record))) => match record {
                CompletePropertyDescriptor::Data { value, .. } => {
                    if let Some(value) = JsValue::from_raw(value) {
                        release(value)?;
                    }
                    Ok(())
                }
                CompletePropertyDescriptor::Accessor { get, set, .. } => {
                    if let Some(value) = get.and_then(JsValue::from_raw) {
                        release(value)?;
                    }
                    if let Some(value) = set.and_then(JsValue::from_raw) {
                        release(value)?;
                    }
                    Ok(())
                }
            },
            _ => Ok(()),
        }
    }
}

struct ReplyGuard<'a> {
    runtime: &'a Runtime,
    resume: Option<SetResume>,
    reply: Option<Reply>,
}
impl<'a> ReplyGuard<'a> {
    fn new(runtime: &'a Runtime, resume: SetResume, reply: Reply) -> Self {
        Self {
            runtime,
            resume: Some(resume),
            reply: Some(reply),
        }
    }
    fn retire(&mut self) -> Result<(), RuntimeError> {
        self.runtime.check_poison()?;
        if let Some(reply) = self.reply.take() {
            reply.retire_at_boundary(self.runtime)?;
        }
        if let Some(resume) = self.resume.take() {
            resume.retire_at_boundary(self.runtime)?;
        }
        Ok(())
    }
    fn run(mut self) -> Result<SetStep, RuntimeError> {
        let runtime = self.runtime;
        let _unwind = runtime.unwind_guard();
        runtime.check_poison()?;
        let mut state = match runtime.0.state.try_borrow_mut() {
            Ok(state) => state,
            Err(_) => {
                self.retire()?;
                return Err(RuntimeError::Invariant(
                    "Set reply state is already borrowed",
                ));
            }
        };
        self.resume
            .as_mut()
            .expect("armed Set reply domain")
            .0
            .retire_completed_request(&mut state, &runtime.0.poisoned)?;
        let resume = self.resume.take().expect("armed Set reply domain");
        let progress = match self.reply.take().expect("armed Set reply") {
            Reply::Advance => resume.advance_in_state(&mut state, &runtime.0.poisoned),
            Reply::Forward(action) => {
                resume.forward_in_state(&mut state, &runtime.0.poisoned, action)
            }
            Reply::Special(value) => {
                resume.special_in_state(&mut state, &runtime.0.poisoned, value)
            }
            Reply::Length(value) => {
                resume.array_length_in_state(&mut state, &runtime.0.poisoned, value)
            }
            Reply::Descriptor(value) => {
                resume.descriptor_in_state(&mut state, &runtime.0.poisoned, value)
            }
            Reply::Defined(value) => {
                resume.defined_in_state(&mut state, &runtime.0.poisoned, value)
            }
            Reply::Shared(word) => resume.shared_in_state(&mut state, &runtime.0.poisoned, word),
        };
        drop(state);
        runtime.check_poison()?;
        progress.map(|progress| SetStep::from_progress(runtime, progress))
    }
}
impl Drop for ReplyGuard<'_> {
    fn drop(&mut self) {
        if !self.runtime.skip_cleanup() {
            let _ = self.retire();
        }
    }
}

impl SetOperands {
    /// The same completed Array length applies both at initial State selection
    /// and after the selected conversion's actual observable effects.
    pub(super) fn array_length_action_in_state(
        &mut self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
        length: u32,
    ) -> Result<SetAction, RuntimeError> {
        let Some(JsValue::Object(receiver)) = self.receiver.as_ref() else {
            return Err(RuntimeError::Invariant(
                "Array length receiver lost its object",
            ));
        };
        let receiver = *receiver;
        let (old_length, writable) = state.array_length_state(receiver)?;
        if !writable {
            return Ok(SetAction::Rejected(
                PropertySetRejection::ArrayLengthReadOnly,
            ));
        }
        let record = PropertyDescriptor {
            value: Some(RuntimeState::array_length_raw(length)),
            ..PropertyDescriptor::new()
        };
        let accepted = state.apply_array_length_descriptor(
            poisoned,
            receiver,
            self.atom,
            &record,
            length,
            (old_length, writable),
        )?;
        Ok(if accepted {
            SetAction::Complete
        } else {
            SetAction::Rejected(PropertySetRejection::NotConfigurable)
        })
    }
}

impl SetResume {
    pub(crate) fn take_object(&mut self, runtime: &Runtime) -> ObjectRef {
        ObjectRef::from_owned_handle(
            runtime.clone(),
            self.0.request_object.take().expect("selected Set object"),
        )
    }
    pub(crate) fn take_key(&mut self, runtime: &Runtime) -> PropertyKey {
        PropertyKey::from_owned_atom(
            runtime.clone(),
            self.0.request_key.take().expect("selected Set key"),
        )
    }
    pub(crate) fn take_value(&mut self) -> JsValue {
        self.0.request_value.take().expect("selected Set value")
    }
    pub(crate) fn take_array_length_initial(
        &mut self,
    ) -> crate::engine::object::array_length::InitialArrayLength {
        self.0
            .array_length_initial
            .take()
            .expect("selected initial Array length conversion")
    }
    pub(crate) fn take_receiver(&mut self) -> JsValue {
        self.0
            .request_receiver
            .take()
            .expect("selected Set receiver")
    }
    pub(crate) fn take_descriptor(&mut self, runtime: &Runtime) -> OwnedPropertyDescriptor {
        let record = self
            .0
            .request_descriptor
            .take()
            .expect("selected Set descriptor");
        let mut owned = OwnedPropertyDescriptor::new(runtime);
        owned.value = record
            .value
            .and_then(JsValue::from_raw)
            .map_or(DescriptorField::Absent, DescriptorField::Present);
        owned.writable = record
            .writable
            .map_or(DescriptorField::Absent, DescriptorField::Present);
        owned.enumerable = record
            .enumerable
            .map_or(DescriptorField::Absent, DescriptorField::Present);
        owned.configurable = record
            .configurable
            .map_or(DescriptorField::Absent, DescriptorField::Present);
        debug_assert!(record.get.is_none() && record.set.is_none());
        owned
    }

    pub(crate) fn array_length_in_state(
        mut self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
        result: crate::engine::object::operations::ArrayLengthConversion,
    ) -> Result<SetProgress, RuntimeError> {
        use crate::engine::object::operations::ArrayLengthConversion;
        let action = match result {
            ArrayLengthConversion::Throw(value) => SetAction::Throw(value),
            ArrayLengthConversion::Length(length) => {
                match self
                    .0
                    .inputs
                    .array_length_action_in_state(state, poisoned, length)
                {
                    Ok(action) => action,
                    Err(error) => {
                        if poisoned.get() {
                            return Err(RuntimeError::Poisoned);
                        }
                        self.0.retire(state, poisoned)?;
                        return Err(error);
                    }
                }
            }
        };
        self.forward_in_state(state, poisoned, action)
    }

    pub(crate) fn special_in_state(
        mut self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
        result: Option<NativeConversion<InternalSetResult>>,
    ) -> Result<SetProgress, RuntimeError> {
        if !matches!(self.0.phase, Phase::Special) {
            if let Some(NativeConversion::Throw(value)) = result {
                state.release_owned_jsvalue(poisoned, value)?;
            }
            self.0.retire(state, poisoned)?;
            return Err(RuntimeError::Invariant(
                "Set continuation received a special reply",
            ));
        }
        match result {
            Some(result) => self.0.finish_action(
                state,
                poisoned,
                SetAction::from_boundary(set_completion(result)),
            ),
            None => {
                let current = self.0.phase_object.expect("special Set owner");
                let selected = self.0.inputs.select_special_own(state, poisoned, current);
                self.finish_selected(state, poisoned, selected)
            }
        }
    }

    pub(crate) fn shared_in_state(
        mut self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
        word: TypedOwnWord,
    ) -> Result<SetProgress, RuntimeError> {
        let Some(request) = self.0.shared.take() else {
            self.0.retire(state, poisoned)?;
            return Err(RuntimeError::Invariant(
                "Set continuation received an unselected shared word",
            ));
        };
        let selected = match request {
            SharedRequest::TypedDecline { object, .. } => {
                self.0.inputs.select_special_own(state, poisoned, object)
            }
            SharedRequest::Own {
                object, receiver, ..
            } => {
                let record = state
                    .own_selected_property_descriptor(poisoned, ReadyOwnProperty::TypedWord(word));
                match record {
                    Ok(record) => self
                        .0
                        .inputs
                        .select_owned_own(state, poisoned, object, record, receiver),
                    Err(error) => Err(error),
                }
            }
            SharedRequest::Rejection { object, .. } => self
                .0
                .inputs
                .rejected_definition_with_own(state, object, true),
        };
        self.finish_selected(state, poisoned, selected)
    }

    pub(crate) fn defined_in_state(
        mut self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
        result: SetDefinitionReply,
    ) -> Result<SetProgress, RuntimeError> {
        if !matches!(self.0.phase, Phase::Define) {
            match result {
                SetDefinitionReply::Rejected(object) => {
                    state.release_owned_jsvalue(poisoned, JsValue::Object(object))?
                }
                SetDefinitionReply::Throw(value) => state.release_owned_jsvalue(poisoned, value)?,
                _ => {}
            }
            self.0.retire(state, poisoned)?;
            return Err(RuntimeError::Invariant(
                "Set continuation received a define reply",
            ));
        }
        let selected = match result {
            SetDefinitionReply::Defined => Ok(Selected::Complete(SetAction::Complete)),
            SetDefinitionReply::RejectedProxyTrap => {
                Ok(Selected::Complete(SetAction::RejectedProxyTrap))
            }
            SetDefinitionReply::Throw(value) => Ok(Selected::Complete(SetAction::Throw(value))),
            SetDefinitionReply::Rejected(object) => {
                let mut owner = crate::engine::heap::runtime::owned_values::OwnedValueGuard::new(
                    state,
                    poisoned,
                    JsValue::Object(object),
                );
                let (state, edge) = owner.parts();
                let result = self.0.inputs.rejected_definition(state, poisoned, object);
                if poisoned.get() {
                    return Err(RuntimeError::Poisoned);
                }
                state.release_owned_jsvalue(
                    poisoned,
                    edge.take().expect("rejected receiver owner"),
                )?;
                result
            }
        };
        self.finish_selected(state, poisoned, selected)
    }

    pub(crate) fn advance(self, runtime: &Runtime) -> Result<SetStep, RuntimeError> {
        ReplyGuard::new(runtime, self, Reply::Advance).run()
    }
    pub(crate) fn forward(
        self,
        runtime: &Runtime,
        action: PropertySetAction,
    ) -> Result<SetStep, RuntimeError> {
        ReplyGuard::new(
            runtime,
            self,
            Reply::Forward(SetAction::from_boundary(action)),
        )
        .run()
    }
    pub(crate) fn special(
        self,
        runtime: &Runtime,
        result: Option<NativeConversion<InternalSetResult>>,
    ) -> Result<SetStep, RuntimeError> {
        ReplyGuard::new(runtime, self, Reply::Special(result)).run()
    }
    pub(crate) fn array_length(
        self,
        runtime: &Runtime,
        result: crate::engine::object::operations::ArrayLengthConversion,
    ) -> Result<SetStep, RuntimeError> {
        ReplyGuard::new(runtime, self, Reply::Length(result)).run()
    }
    pub(crate) fn descriptor(
        self,
        runtime: &Runtime,
        result: NativeConversion<Option<OwnedCompletePropertyDescriptor>>,
    ) -> Result<SetStep, RuntimeError> {
        // Empty Runtime descriptor adapters disappear before the State lease.
        let result = match result {
            NativeConversion::Throw(value) => NativeConversion::Throw(value),
            NativeConversion::Value(value) => NativeConversion::Value(
                value.map(OwnedCompletePropertyDescriptor::into_owned_record),
            ),
        };
        ReplyGuard::new(runtime, self, Reply::Descriptor(result)).run()
    }
    pub(crate) fn defined(
        self,
        runtime: &Runtime,
        result: NativeConversion<InternalDefineResult>,
    ) -> Result<SetStep, RuntimeError> {
        let result = match result {
            NativeConversion::Throw(value) => SetDefinitionReply::Throw(value),
            NativeConversion::Value(InternalDefineResult::Defined) => SetDefinitionReply::Defined,
            NativeConversion::Value(InternalDefineResult::RejectedProxyTrap) => {
                SetDefinitionReply::RejectedProxyTrap
            }
            NativeConversion::Value(InternalDefineResult::RejectedOrdinary(object)) => {
                SetDefinitionReply::Rejected(object.into_execution_handle())
            }
        };
        ReplyGuard::new(runtime, self, Reply::Defined(result)).run()
    }

    pub(crate) fn shared(
        self,
        runtime: &Runtime,
        word: TypedOwnWord,
    ) -> Result<SetStep, RuntimeError> {
        ReplyGuard::new(runtime, self, Reply::Shared(word)).run()
    }

    pub(crate) fn take_shared_word(&mut self) -> Option<SharedTypedOwnWord> {
        let request = self.0.shared.as_mut()?;
        // The selected word is transferred separately while the semantic
        // request discriminator/receiver identity remains in this raw owner.
        match request {
            SharedRequest::TypedDecline { word, .. }
            | SharedRequest::Own { word, .. }
            | SharedRequest::Rejection { word, .. } => word.take(),
        }
    }

    pub(crate) fn start_selected_typed(
        &mut self,
        runtime: &Runtime,
    ) -> Result<crate::engine::builtins::TypedWriteStep, RuntimeError> {
        let (index, element) = self
            .0
            .typed
            .take()
            .expect("selected TypedArray Set metadata");
        let object = self.0.request_object.expect("typed request object");
        // Preserve genuine child's checked owner roles before retiring request
        // temporaries. No key selection or TypedArray snapshot is repeated.
        let object = ObjectRef::from_borrowed_handle(runtime.clone(), object)?;
        let value =
            runtime.dup_jsvalue(self.0.request_value.as_ref().expect("typed request value"))?;
        crate::engine::builtins::TypedWriteStep::set_selected(
            runtime, object, index, element, value,
        )
    }

    pub(crate) fn retire_request_at_boundary(
        &mut self,
        runtime: &Runtime,
    ) -> Result<(), RuntimeError> {
        // These have been adopted by the child before this release. Domain
        // and phase owners stay armed across every fallible retirement.
        for edge in [&mut self.0.request_value, &mut self.0.request_receiver] {
            if let Some(value) = edge.take() {
                runtime.release_jsvalue(value)?;
                runtime.check_poison()?;
            }
        }
        if let Some(object) = self.0.request_object.take() {
            runtime.release_jsvalue(JsValue::Object(object))?;
            runtime.check_poison()?;
        }
        if let Some(atom) = self.0.request_key.take() {
            runtime.release_jsvalue(JsValue::Symbol(AtomIdx::from_raw(atom.raw())))?;
            runtime.check_poison()?;
        }
        Ok(())
    }

    pub(super) fn finish_special_boundary(
        mut self,
        runtime: &Runtime,
    ) -> Result<SetStep, RuntimeError> {
        if let Some(word) = self.take_shared_word() {
            let word = match word.read() {
                Ok(word) => word,
                Err(error) => {
                    self.retire_at_boundary(runtime)?;
                    return Err(error);
                }
            };
            return self.shared(runtime, word);
        }
        let realm = match self.0.inputs.realm {
            Some(realm) => realm,
            None => {
                self.retire_at_boundary(runtime)?;
                return Err(RuntimeError::Invariant("typed Set requires a realm"));
            }
        };
        let child = match self.start_selected_typed(runtime) {
            Ok(child) => child,
            Err(error) => {
                self.retire_at_boundary(runtime)?;
                return Err(error);
            }
        };
        if let Err(error) = self.retire_request_at_boundary(runtime) {
            child.retire_at_boundary(runtime)?;
            self.retire_at_boundary(runtime)?;
            return Err(error);
        }
        let result = child.finish_sync(runtime, realm);
        match result {
            Ok(result) => self.special(
                runtime,
                Some(match result {
                    NativeConversion::Value(_) => {
                        NativeConversion::Value(InternalSetResult::Accepted)
                    }
                    NativeConversion::Throw(value) => NativeConversion::Throw(value),
                }),
            ),
            Err(error) => {
                self.retire_at_boundary(runtime)?;
                Err(error)
            }
        }
    }
}
