//! Only an actual callback or shared backing service promotes read facts.
use crate::engine::{
    api::runtime_error::RuntimeError,
    heap::{
        ObjectId,
        runtime::{RuntimeState, owned_values::OwnedValueGuard},
    },
    object::{ReadBoundary, SpecialKind},
    value::JsValue,
};
use std::cell::Cell;

#[must_use]
pub(crate) enum StateReadEffect {
    Getter { callee: ObjectId, receiver: JsValue },
    Proxy { object: ObjectId, receiver: JsValue },
    Shared(crate::engine::builtins::SharedTypedRead),
}

impl StateReadEffect {
    /// The selected holder and callee are still pinned by the source window or
    /// algorithm owner. Promote them before ending this access; never relookup.
    pub(crate) fn prepare(
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
        boundary: ReadBoundary,
        receiver: &JsValue,
    ) -> Result<Self, RuntimeError> {
        let (object, getter) = match boundary {
            ReadBoundary::Shared(read) => return Ok(Self::Shared(read)),
            ReadBoundary::Getter(callee) => (callee, true),
            ReadBoundary::Special {
                object,
                kind: SpecialKind::Proxy,
            } => (object, false),
            _ => {
                return Err(RuntimeError::Invariant(
                    "read effect has no selected callback",
                ));
            }
        };
        let owner = state.dup_jsvalue(&JsValue::Object(object))?;
        let mut owner = OwnedValueGuard::new(state, poisoned, owner);
        let (state, object) = owner.parts();
        let receiver = state.dup_jsvalue(receiver)?;
        let Some(JsValue::Object(object)) = object.take() else {
            unreachable!("selected read owns its object")
        };
        Ok(if getter {
            Self::Getter {
                callee: object,
                receiver,
            }
        } else {
            Self::Proxy { object, receiver }
        })
    }

    pub(crate) fn release_in_state(
        self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
    ) -> Result<(), RuntimeError> {
        match self {
            Self::Getter {
                callee: object,
                receiver,
            }
            | Self::Proxy { object, receiver } => {
                state.release_owned_jsvalue(poisoned, JsValue::Object(object))?;
                state.release_owned_jsvalue(poisoned, receiver)
            }
            Self::Shared(_) => Ok(()),
        }
    }
}
