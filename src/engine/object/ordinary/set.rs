//! One ordinary Set state machine. Internal continuations own raw finite edges.
use super::*;
use crate::engine::{
    atom::Atom,
    builtins::{SharedTypedOwnWord, TypedOwnWord, native::TypedArrayElementKind},
    heap::{ObjectId, ObjectPayload, RawValue, runtime::RuntimeState},
    object::{
        OwnedCompletePropertyDescriptor, OwnedPropertyDescriptor,
        internal_methods::SelectedTypedSet,
        ordinary_storage::StateSetProbe,
        own_properties::{OwnPropertySelection, ReadyOwnProperty},
        owned_descriptor::CompleteDescriptorGuard,
        property::{CompletePropertyDescriptor, PropertyDescriptor},
    },
};
use std::cell::Cell;

mod boundary;
mod finish;
mod ownership;
pub(crate) use ownership::SetProgressGuard;
mod reply;
mod selection;

/// Rooted transport exists only at an actual public/legacy boundary. Suspended
/// records below own no Runtime, ObjectRef, CallableRef or PropertyKey.
#[must_use]
pub(crate) enum SetStep {
    Complete(PropertySetAction),
    CyclePublishedComplete(PropertySetAction),
    Continue { resume: SetResume },
    Proxy { resume: SetResume },
    Special { resume: SetResume },
    ArrayLength { resume: SetResume },
    Descriptor { resume: SetResume },
    Define { resume: SetResume },
}

/// Terminal state result. Setter edges transfer to the canonical RawCall
/// consumer; no public root is constructed under the selecting State lease.
#[must_use]
pub(crate) enum SetAction {
    Complete,
    Rejected(PropertySetRejection),
    RejectedProxyTrap,
    Throw(JsValue),
    Call {
        function: ObjectId,
        receiver: JsValue,
        argument: JsValue,
    },
}

#[must_use]
pub(crate) enum SetProgress {
    Complete(SetAction),
    CyclePublished(SetAction),
    Waiting { phase: SetWait, resume: SetResume },
}

#[derive(Clone, Copy)]
pub(crate) enum SetWait {
    Continue,
    Proxy,
    Special,
    ArrayLength,
    Descriptor,
    Define,
}

#[must_use]
pub(crate) struct SetResume(Box<SetResumeState>);
impl std::ops::Deref for SetResume {
    type Target = SetResumeState;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl std::ops::DerefMut for SetResume {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// Actual inputs for the sole Set selector. Local completion owns only these
/// roles; callback-free selection does not construct a suspension ledger.
struct SetOperands {
    realm: Option<ContextId>,
    target: ObjectId,
    target_owner: Option<ObjectId>,
    atom: Atom,
    key_owner: Option<Atom>,
    value: Option<JsValue>,
    receiver: Option<JsValue>,
    cycle_published: bool,
}

/// Only a selected wait needs these independent phase/request roles. Empty
/// fields have transferred to a request or retired explicitly; no raw Drop.
pub(crate) struct SetResumeState {
    inputs: SetOperands,
    phase: Phase,
    phase_object: Option<ObjectId>,
    retiring_phase: Option<ObjectId>,
    request_object: Option<ObjectId>,
    request_key: Option<Atom>,
    request_value: Option<JsValue>,
    request_receiver: Option<JsValue>,
    request_descriptor: Option<PropertyDescriptor<RawValue>>,
    typed: Option<(Option<u64>, TypedArrayElementKind)>,
    array_length_initial: Option<crate::engine::object::array_length::InitialArrayLength>,
    shared: Option<SharedRequest>,
}

#[derive(Clone, Copy)]
enum Phase {
    Walk,
    Forward,
    Special,
    Receiver,
    Define,
}

enum SharedRequest {
    /// The exact different-receiver IntegerIndexedElementSet check; its mutex
    /// result precedes the subsequent ordinary own-descriptor selection.
    TypedDecline {
        word: Option<SharedTypedOwnWord>,
        object: ObjectId,
    },
    Own {
        word: Option<SharedTypedOwnWord>,
        object: ObjectId,
        receiver: bool,
    },
    Rejection {
        word: Option<SharedTypedOwnWord>,
        object: ObjectId,
    },
}

enum Selected {
    Complete(SetAction),
    Walk(ObjectId),
    Proxy(ObjectId),
    Typed(ObjectId, Option<u64>, TypedArrayElementKind),
    Shared(SharedRequest),
    ArrayLength(
        ObjectId,
        crate::engine::object::array_length::InitialArrayLength,
    ),
    Descriptor(ObjectId),
    Define(ObjectId, bool),
}

impl SetOperands {
    fn new(
        realm: Option<ContextId>,
        target: ObjectId,
        target_owner: Option<ObjectId>,
        atom: Atom,
        key_owner: Option<Atom>,
        value: JsValue,
        receiver: JsValue,
    ) -> Self {
        Self {
            realm,
            target,
            target_owner,
            atom,
            key_owner,
            value: Some(value),
            receiver: Some(receiver),
            cycle_published: false,
        }
    }

    /// Every caller uses this producer. Borrowed target/key inputs remain
    /// protected by execution metadata/receiver until a real wait is published.
    // State/poison and realm are separate from the independently owned
    // target/key/value/receiver roles; keep their transfer boundaries explicit.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn start_in_state(
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
        realm: Option<ContextId>,
        target: ObjectId,
        target_owner: Option<ObjectId>,
        atom: Atom,
        key_owner: Option<Atom>,
        value: JsValue,
        receiver: JsValue,
    ) -> Result<SetProgress, RuntimeError> {
        let _unwind = crate::engine::api::runtime::RuntimeUnwindGuard::from_flag(poisoned);
        let mut owner = Self::new(
            realm,
            target,
            target_owner,
            atom,
            key_owner,
            value,
            receiver,
        );
        let result = (|| {
            if realm.is_none()
                && (matches!(state.heap.object(target)?.payload, ObjectPayload::Proxy(_))
                    || matches!(owner.receiver.as_ref(), Some(JsValue::Object(id))
                    if matches!(state.heap.object(*id)?.payload, ObjectPayload::Proxy(_))))
            {
                return Err(RuntimeError::Invariant("exotic Set requires a realm"));
            }
            state.atoms.resolve(atom)?;
            let selected = owner.select_walk(state, poisoned, target)?;
            owner.advance_selected(state, poisoned, selected)
        })();
        owner.finish_start(state, poisoned, result)
    }

    fn finish_start(
        mut self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
        result: Result<Selected, RuntimeError>,
    ) -> Result<SetProgress, RuntimeError> {
        if poisoned.get() {
            return Err(RuntimeError::Poisoned);
        }
        match result {
            Err(error) => {
                self.retire(state, poisoned)?;
                Err(error)
            }
            Ok(Selected::Complete(action)) => self.finish_action(state, poisoned, action),
            Ok(selected) => {
                let mut owner = SetResumeState::from_inputs(self);
                let phase = match owner.publish(state, poisoned, selected) {
                    Ok(phase) => phase,
                    Err(error) => {
                        if poisoned.get() {
                            return Err(RuntimeError::Poisoned);
                        }
                        owner.retire(state, poisoned)?;
                        return Err(error);
                    }
                };
                Ok(SetProgress::Waiting {
                    phase,
                    resume: SetResume(Box::new(owner)),
                })
            }
        }
    }

    fn finish_action(
        mut self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
        action: SetAction,
    ) -> Result<SetProgress, RuntimeError> {
        let mut action = ownership::SetActionGuard::new(state, poisoned, action);
        let (state, _) = action.parts();
        self.retire(state, poisoned)?;
        Ok(action.finish_progress(self.cycle_published))
    }
}

impl SetResumeState {
    fn from_inputs(inputs: SetOperands) -> Self {
        Self {
            inputs,
            phase: Phase::Forward,
            phase_object: None,
            retiring_phase: None,
            request_object: None,
            request_key: None,
            request_value: None,
            request_receiver: None,
            request_descriptor: None,
            typed: None,
            array_length_initial: None,
            shared: None,
        }
    }

    fn finish_action(
        mut self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
        action: SetAction,
    ) -> Result<SetProgress, RuntimeError> {
        let mut action = ownership::SetActionGuard::new(state, poisoned, action);
        let (state, _) = action.parts();
        self.retire(state, poisoned)?;
        Ok(action.finish_progress(self.inputs.cycle_published))
    }
}

impl SetResume {
    fn finish_selected(
        mut self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
        selected: Result<Selected, RuntimeError>,
    ) -> Result<SetProgress, RuntimeError> {
        if poisoned.get() {
            return Err(RuntimeError::Poisoned);
        }
        let selected = match selected {
            Ok(selected) => selected,
            Err(error) => {
                self.0.retire(state, poisoned)?;
                return Err(error);
            }
        };
        let selected = match self.0.inputs.advance_selected(state, poisoned, selected) {
            Ok(selected) => selected,
            Err(error) => {
                if poisoned.get() {
                    return Err(RuntimeError::Poisoned);
                }
                self.0.retire(state, poisoned)?;
                return Err(error);
            }
        };
        match selected {
            Selected::Complete(action) => self.0.finish_action(state, poisoned, action),
            selected => {
                let phase = match self.0.publish(state, poisoned, selected) {
                    Ok(phase) => phase,
                    Err(error) => {
                        if poisoned.get() {
                            return Err(RuntimeError::Poisoned);
                        }
                        self.0.retire(state, poisoned)?;
                        return Err(error);
                    }
                };
                Ok(SetProgress::Waiting {
                    phase,
                    resume: self,
                })
            }
        }
    }

    pub(crate) fn advance_in_state(
        mut self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
    ) -> Result<SetProgress, RuntimeError> {
        if !matches!(self.0.phase, Phase::Walk) {
            self.0.retire(state, poisoned)?;
            return Err(RuntimeError::Invariant(
                "Set continuation received a walk reply",
            ));
        }
        let object = self.0.phase_object.expect("walk owner");
        let selected = self.0.inputs.select_walk(state, poisoned, object);
        // Keep the phase owner armed until the selected request is published.
        self.finish_selected(state, poisoned, selected)
    }

    pub(crate) fn descriptor_in_state(
        mut self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
        result: NativeConversion<Option<CompletePropertyDescriptor<RawValue>>>,
    ) -> Result<SetProgress, RuntimeError> {
        if !matches!(self.0.phase, Phase::Receiver) {
            ownership::retire_descriptor_reply(state, poisoned, result)?;
            self.0.retire(state, poisoned)?;
            return Err(RuntimeError::Invariant(
                "Set continuation received a descriptor reply",
            ));
        }
        let selected = match result {
            NativeConversion::Throw(value) => Ok(Selected::Complete(SetAction::Throw(value))),
            NativeConversion::Value(record) => {
                self.0.inputs.select_descriptor(state, poisoned, record)
            }
        };
        self.finish_selected(state, poisoned, selected)
    }

    pub(crate) fn forward_in_state(
        self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
        action: SetAction,
    ) -> Result<SetProgress, RuntimeError> {
        if !matches!(self.0.phase, Phase::Forward) {
            action.retire(state, poisoned)?;
            let mut owner = self.0;
            owner.retire(state, poisoned)?;
            return Err(RuntimeError::Invariant(
                "Set continuation received a forward reply",
            ));
        }
        self.0.finish_action(state, poisoned, action)
    }
}

const _: () = assert!(std::mem::size_of::<SetStep>() <= 64);
const _: () = assert!(std::mem::size_of::<SetResume>() <= 8);
include!("set/legacy_tests.rs");

#[cfg(test)]
mod state_tests;
