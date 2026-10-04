//! Callback-free Set selection; layout facts never leave this State lease.
use super::*;

fn stored(accepted: bool) -> SetAction {
    if accepted {
        SetAction::Complete
    } else {
        SetAction::Rejected(PropertySetRejection::ReadOnly)
    }
}

impl SetResumeState {
    pub(super) fn select_walk(
        &mut self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
        mut current: ObjectId,
    ) -> Result<Selected, RuntimeError> {
        loop {
            let same =
                matches!(self.receiver.as_ref(), Some(JsValue::Object(id)) if *id == current);
            let probe = state.ordinary_set_probe_in_state(
                poisoned,
                current,
                self.atom,
                self.value.as_ref().expect("Set value"),
                same,
                true,
            )?;
            match probe {
                StateSetProbe::Stored(value) => return Ok(Selected::Complete(stored(value))),
                StateSetProbe::Rejected(reason) => {
                    return Ok(Selected::Complete(SetAction::Rejected(reason)));
                }
                StateSetProbe::SpecialAt(object, kind) => {
                    return self.select_special(state, poisoned, object, kind);
                }
                StateSetProbe::Writable => return self.select_receiver(state, poisoned),
                StateSetProbe::Setter(setter) => {
                    return Ok(Selected::Complete(self.select_setter(state, setter)?));
                }
                StateSetProbe::Missing(Some(next)) => current = next,
                StateSetProbe::Missing(None) => return self.select_receiver(state, poisoned),
                StateSetProbe::Special(kind) => {
                    return self.select_special(state, poisoned, current, kind);
                }
            }
        }
    }

    fn select_setter(
        &mut self,
        state: &mut RuntimeState,
        setter: Option<ObjectId>,
    ) -> Result<SetAction, RuntimeError> {
        let Some(function) = setter else {
            return Ok(SetAction::Rejected(PropertySetRejection::NoSetter));
        };
        // Preserve checked callee promotion before either no-Drop input moves.
        state.heap.retain_object(function)?;
        Ok(SetAction::Call {
            function,
            receiver: self.receiver.take().expect("Set receiver"),
            argument: self.value.take().expect("Set value"),
        })
    }

    fn select_special(
        &mut self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
        current: ObjectId,
        kind: SpecialKind,
    ) -> Result<Selected, RuntimeError> {
        match kind {
            SpecialKind::Proxy => {
                if self.realm.is_none() {
                    return Err(RuntimeError::Invariant("exotic Set requires a realm"));
                }
                return Ok(Selected::Proxy(current));
            }
            SpecialKind::ModuleNamespace if self.realm.is_some() => {
                return Ok(Selected::Complete(SetAction::Rejected(
                    PropertySetRejection::ReadOnly,
                )));
            }
            SpecialKind::TypedArray if self.realm.is_some() => {
                match state.select_typed_array_set(
                    current,
                    self.atom,
                    self.receiver.as_ref().expect("Set receiver"),
                )? {
                    SelectedTypedSet::Ignore => return Ok(Selected::Complete(SetAction::Complete)),
                    SelectedTypedSet::Element { index, element } => {
                        return Ok(Selected::Typed(current, index, element));
                    }
                    SelectedTypedSet::SharedDecline(word) => {
                        return Ok(Selected::Shared(SharedRequest::TypedDecline {
                            word: Some(word),
                            object: current,
                        }));
                    }
                    SelectedTypedSet::Decline => {}
                }
            }
            _ => {}
        }
        self.select_special_own(state, poisoned, current)
    }

    pub(super) fn select_special_own(
        &mut self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
        current: ObjectId,
    ) -> Result<Selected, RuntimeError> {
        match state.select_own_property(poisoned, current, self.atom)? {
            OwnPropertySelection::Missing | OwnPropertySelection::TerminalMissing => {
                self.select_missing_own(state, poisoned, current)
            }
            OwnPropertySelection::Shared(word) => Ok(Selected::Shared(SharedRequest::Own {
                word: Some(word),
                object: current,
                receiver: false,
            })),
            OwnPropertySelection::Ready(ready) => {
                let record = state.own_selected_property_descriptor(poisoned, ready)?;
                self.select_owned_own(state, poisoned, current, record, false)
            }
            OwnPropertySelection::CyclePublished(ready) => {
                self.cycle_published = true;
                let record = state.own_selected_property_descriptor(poisoned, ready)?;
                self.select_owned_own(state, poisoned, current, record, false)
            }
        }
    }

    fn select_missing_own(
        &mut self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
        current: ObjectId,
    ) -> Result<Selected, RuntimeError> {
        match state
            .heap
            .shape(state.heap.object(current)?.shape)?
            .prototype()
        {
            Some(next) => Ok(Selected::Walk(next)),
            None => self.select_receiver(state, poisoned),
        }
    }

    pub(super) fn select_owned_own(
        &mut self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
        current: ObjectId,
        record: CompletePropertyDescriptor<RawValue>,
        receiver: bool,
    ) -> Result<Selected, RuntimeError> {
        let mut owner = CompleteDescriptorGuard::from_owned_record(state, poisoned, record);
        let (state, record) = owner.parts();
        let selected = if receiver {
            self.select_receiver_record(state, poisoned, current, Some(record))
        } else {
            match record {
                CompletePropertyDescriptor::Data { writable, .. } => {
                    if matches!(self.receiver.as_ref(), Some(JsValue::Object(id)) if *id == current)
                        && state.array_own_key(current, self.atom)? == ArrayOwnKey::Length
                    {
                        let initial =
                            crate::engine::object::array_length::InitialArrayLength::select(
                                self.value.as_ref().expect("Array length value"),
                            );
                        match initial {
                            crate::engine::object::array_length::InitialArrayLength::Length(
                                length,
                            ) => self
                                .array_length_action_in_state(state, poisoned, length)
                                .map(Selected::Complete),
                            initial => Ok(Selected::ArrayLength(current, initial)),
                        }
                    } else if !*writable {
                        Ok(Selected::Complete(SetAction::Rejected(
                            PropertySetRejection::ReadOnly,
                        )))
                    } else {
                        self.select_receiver(state, poisoned)
                    }
                }
                CompletePropertyDescriptor::Accessor { set, .. } => {
                    let setter = match set {
                        Some(RawValue::Object(id)) => Some(*id),
                        None => None,
                        _ => {
                            return Err(RuntimeError::Invariant(
                                "Set descriptor setter was not an object",
                            ));
                        }
                    };
                    self.select_setter(state, setter).map(Selected::Complete)
                }
            }
        };
        if poisoned.get() {
            return Err(RuntimeError::Poisoned);
        }
        // A terminal callback/throw remains guarded while old descriptor edges
        // retire. In particular a getter-zero cleanup cannot leak its setter.
        match selected {
            Ok(Selected::Complete(action)) => {
                if let Err(error) = owner.retire() {
                    if !poisoned.get() {
                        action.retire(owner.state(), poisoned)?;
                    }
                    return Err(error);
                }
                Ok(Selected::Complete(action))
            }
            result => {
                owner.retire()?;
                result
            }
        }
    }

    fn select_receiver(
        &mut self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
    ) -> Result<Selected, RuntimeError> {
        let Some(JsValue::Object(receiver)) = self.receiver.as_ref() else {
            return Ok(Selected::Complete(SetAction::Rejected(
                PropertySetRejection::NotObject,
            )));
        };
        let receiver = *receiver;
        match state.ordinary_set_probe_in_state(
            poisoned,
            receiver,
            self.atom,
            self.value.as_ref().expect("Set value"),
            true,
            false,
        )? {
            StateSetProbe::Stored(value) => Ok(Selected::Complete(stored(value))),
            StateSetProbe::Rejected(reason) => Ok(Selected::Complete(SetAction::Rejected(reason))),
            StateSetProbe::Setter(setter) => Ok(Selected::Complete(SetAction::Rejected(
                if setter.is_some() {
                    PropertySetRejection::ReadOnly
                } else {
                    PropertySetRejection::NoSetter
                },
            ))),
            StateSetProbe::Missing(_) => Ok(Selected::Define(receiver, false)),
            StateSetProbe::Special(_) => {
                if matches!(
                    state.heap.object(receiver)?.payload,
                    ObjectPayload::Proxy(_)
                ) {
                    return Ok(Selected::Descriptor(receiver));
                }
                match state.select_own_property(poisoned, receiver, self.atom)? {
                    OwnPropertySelection::Missing | OwnPropertySelection::TerminalMissing => {
                        Ok(Selected::Define(receiver, false))
                    }
                    OwnPropertySelection::Shared(word) => {
                        Ok(Selected::Shared(SharedRequest::Own {
                            word: Some(word),
                            object: receiver,
                            receiver: true,
                        }))
                    }
                    OwnPropertySelection::Ready(ready) => {
                        let record = state.own_selected_property_descriptor(poisoned, ready)?;
                        self.select_owned_own(state, poisoned, receiver, record, true)
                    }
                    OwnPropertySelection::CyclePublished(ready) => {
                        self.cycle_published = true;
                        let record = state.own_selected_property_descriptor(poisoned, ready)?;
                        self.select_owned_own(state, poisoned, receiver, record, true)
                    }
                }
            }
            StateSetProbe::SpecialAt(..) | StateSetProbe::Writable => Err(RuntimeError::Invariant(
                "receiver probe walked or omitted a data store",
            )),
        }
    }

    fn select_receiver_record(
        &mut self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
        receiver: ObjectId,
        record: Option<&CompletePropertyDescriptor<RawValue>>,
    ) -> Result<Selected, RuntimeError> {
        match record {
            Some(CompletePropertyDescriptor::Data {
                writable: false, ..
            }) => Ok(Selected::Complete(SetAction::Rejected(
                PropertySetRejection::ReadOnly,
            ))),
            Some(CompletePropertyDescriptor::Accessor { set, .. }) => {
                Ok(Selected::Complete(SetAction::Rejected(if set.is_some() {
                    PropertySetRejection::ReadOnly
                } else {
                    PropertySetRejection::NoSetter
                })))
            }
            Some(CompletePropertyDescriptor::Data { .. }) => {
                if state.set_arguments_index_value(
                    poisoned,
                    receiver,
                    self.atom,
                    self.value.as_ref().expect("Set value"),
                )? {
                    Ok(Selected::Complete(SetAction::Complete))
                } else {
                    Ok(Selected::Define(receiver, true))
                }
            }
            None => Ok(Selected::Define(receiver, false)),
        }
    }

    pub(super) fn select_descriptor(
        &mut self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
        record: Option<CompletePropertyDescriptor<RawValue>>,
    ) -> Result<Selected, RuntimeError> {
        let Some(JsValue::Object(receiver)) = self.receiver.as_ref() else {
            ownership::retire_descriptor_reply(state, poisoned, NativeConversion::Value(record))?;
            return Err(RuntimeError::Invariant("Set receiver lost its object"));
        };
        let receiver = *receiver;
        match record {
            Some(record) => self.select_owned_own(state, poisoned, receiver, record, true),
            None => self.select_receiver_record(state, poisoned, receiver, None),
        }
    }

    pub(super) fn rejected_definition(
        &mut self,
        state: &mut RuntimeState,
        _poisoned: &Cell<bool>,
        receiver: ObjectId,
    ) -> Result<Selected, RuntimeError> {
        let own = match state.select_own_presence(receiver, self.atom)? {
            crate::engine::object::own_properties::OwnPresenceSelection::Ready(own) => own,
            crate::engine::object::own_properties::OwnPresenceSelection::Shared(word) => {
                return Ok(Selected::Shared(SharedRequest::Rejection {
                    word: Some(word),
                    object: receiver,
                }));
            }
        };
        self.rejected_definition_with_own(state, receiver, own)
    }

    pub(super) fn rejected_definition_with_own(
        &self,
        state: &RuntimeState,
        receiver: ObjectId,
        own: bool,
    ) -> Result<Selected, RuntimeError> {
        Ok(Selected::Complete(SetAction::Rejected(
            if !own && !state.heap.object(receiver)?.extensible {
                PropertySetRejection::NotExtensible
            } else if matches!(
                state.array_own_key(receiver, self.atom)?,
                ArrayOwnKey::Index(_)
            ) && !state.array_length_state(receiver)?.1
            {
                PropertySetRejection::ArrayLengthReadOnly
            } else {
                PropertySetRejection::ReadOnly
            },
        )))
    }

    pub(super) fn advance_selected(
        &mut self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
        mut selected: Selected,
    ) -> Result<Selected, RuntimeError> {
        loop {
            if poisoned.get() {
                return Err(RuntimeError::Poisoned);
            }
            selected = match selected {
                Selected::Walk(object) => self.select_walk(state, poisoned, object)?,
                Selected::Define(object, existing) => {
                    let payload = &state.heap.object(object)?.payload;
                    let key = state.array_own_key(object, self.atom)?;
                    let typed_index = matches!(payload, ObjectPayload::TypedArray(_))
                        && state
                            .typed_canonical_numeric_index(
                                self.atom,
                                state.typed_own_key(self.atom)?,
                            )?
                            .is_some();
                    if matches!(payload, ObjectPayload::Proxy(_))
                        || key == ArrayOwnKey::Length
                        || typed_index
                    {
                        return Ok(Selected::Define(object, existing));
                    }
                    let descriptor = PropertyDescriptor {
                        value: Some(self.value.as_ref().expect("Set value").as_raw()),
                        writable: (!existing).then_some(true),
                        enumerable: (!existing).then_some(true),
                        configurable: (!existing).then_some(true),
                        ..PropertyDescriptor::new()
                    };
                    let accepted =
                        state.define_own_raw_property(poisoned, object, self.atom, &descriptor)?;
                    if accepted {
                        Selected::Complete(SetAction::Complete)
                    } else {
                        self.rejected_definition(state, poisoned, object)?
                    }
                }
                selected => return Ok(selected),
            };
        }
    }
}
