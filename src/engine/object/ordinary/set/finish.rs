//! Assignment strictness and message production share one finite body.
use super::*;

/// Only the selected atom-message branches invoke their concrete formatter.
/// Pure completions require no State lease or extra boundary admission.
fn finish_property_set(
    result: NativeConversion<InternalSetResult>,
    strict: bool,
    mut atom_error: impl FnMut(bool) -> Result<crate::engine::api::Error, RuntimeError>,
) -> Result<crate::engine::vm::Completion, RuntimeError> {
    use crate::engine::api::{Error, ErrorKind};
    use crate::engine::vm::Completion;
    match result {
        NativeConversion::Value(InternalSetResult::Accepted) => {
            Ok(Completion::Return(JsValue::Undefined))
        }
        NativeConversion::Value(_) if !strict => Ok(Completion::Return(JsValue::Undefined)),
        NativeConversion::Value(InternalSetResult::RejectedProxyTrap) => {
            Err(Error::new(ErrorKind::Type, "proxy: cannot set property").into())
        }
        NativeConversion::Value(InternalSetResult::Rejected(PropertySetRejection::ReadOnly)) => {
            Err(atom_error(false)?.into())
        }
        NativeConversion::Value(InternalSetResult::Rejected(
            PropertySetRejection::ArrayLengthReadOnly,
        )) => Err(atom_error(true)?.into()),
        NativeConversion::Value(InternalSetResult::Rejected(reason)) => Err(Error::new(
            ErrorKind::Type,
            match reason {
                PropertySetRejection::NotConfigurable => "not configurable",
                PropertySetRejection::NoSetter => "no setter for property",
                PropertySetRejection::NotExtensible => "object is not extensible",
                PropertySetRejection::NotObject => "not an object",
                PropertySetRejection::ReadOnly | PropertySetRejection::ArrayLengthReadOnly => {
                    unreachable!()
                }
            },
        )
        .into()),
        NativeConversion::Throw(value) => Ok(Completion::Throw(value)),
    }
}

impl RuntimeState {
    pub(crate) fn finish_property_set_in_state(
        &self,
        result: NativeConversion<InternalSetResult>,
        atom: Atom,
        strict: bool,
    ) -> Result<crate::engine::vm::Completion, RuntimeError> {
        finish_property_set(result, strict, |length| {
            let atom = if length {
                self.pinned_atoms
                    .get(crate::engine::atom::pinned::PinnedAtom::Length)
            } else {
                atom
            };
            self.native_atom_error(
                crate::engine::api::ErrorKind::Type,
                "'",
                atom,
                "' is read-only",
            )
        })
    }
}

impl Runtime {
    pub(crate) fn finish_property_set(
        &self,
        result: NativeConversion<InternalSetResult>,
        key: &PropertyKey,
        strict: bool,
    ) -> Result<crate::engine::vm::Completion, RuntimeError> {
        // Preserve actual public atom/pinned-key admission only when its old
        // branch was selected; accepted, non-strict and Throw stay pure.
        finish_property_set(result, strict, |length| {
            let pinned;
            let key = if length {
                pinned =
                    self.pinned_property_key(crate::engine::atom::pinned::PinnedAtom::Length)?;
                &pinned
            } else {
                key
            };
            self.native_atom_error(
                crate::engine::api::ErrorKind::Type,
                "'",
                key,
                "' is read-only",
            )
        })
    }
}

impl SetAction {
    pub(crate) fn into_result(self) -> Result<NativeConversion<InternalSetResult>, Self> {
        Ok(match self {
            Self::Complete => NativeConversion::Value(InternalSetResult::Accepted),
            Self::Rejected(reason) => NativeConversion::Value(InternalSetResult::Rejected(reason)),
            Self::RejectedProxyTrap => {
                NativeConversion::Value(InternalSetResult::RejectedProxyTrap)
            }
            Self::Throw(value) => NativeConversion::Throw(value),
            action @ Self::Call { .. } => return Err(action),
        })
    }
}

impl RuntimeState {
    /// Strict builtin Set uses the exact assignment formatter. Only the actual
    /// rejection producer adds the fresh collectible Error publication fact;
    /// an existing propagated Throw remains an ordinary completion.
    pub(crate) fn finish_set_property_or_throw_in_state(
        &mut self,
        poisoned: &Cell<bool>,
        realm: ContextId,
        atom: Atom,
        action: SetAction,
    ) -> Result<SetProgress, RuntimeError> {
        let result = match action.into_result() {
            Ok(result) => result,
            Err(action) => {
                action.retire(self, poisoned)?;
                return Err(RuntimeError::Invariant(
                    "setter result bypassed callback consumer",
                ));
            }
        };
        match self.finish_property_set_in_state(result, atom, true) {
            Ok(crate::engine::vm::Completion::Return(_)) => {
                Ok(SetProgress::Complete(SetAction::Complete))
            }
            Ok(crate::engine::vm::Completion::Throw(value)) => {
                Ok(SetProgress::Complete(SetAction::Throw(value)))
            }
            Err(RuntimeError::Engine(error)) => {
                let message = error.native_message().cloned().unwrap_or_else(|| {
                    crate::engine::api::error::NativeErrorMessage::from_utf8(error.message())
                });
                let result = self.new_native_error_from_message(
                    poisoned,
                    realm,
                    crate::engine::api::error::NativeErrorKind::Type,
                    message,
                );
                if poisoned.get() {
                    return Err(RuntimeError::Poisoned);
                }
                let object = result?;
                Ok(SetProgress::CyclePublished(SetAction::Throw(
                    JsValue::Object(object),
                )))
            }
            Err(error) => Err(error),
        }
    }
}
