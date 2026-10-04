//! ToPropertyKey's primitive suffix uses the admitted State and returns an atom
//! edge. Public roots are created only after that access ends.
use crate::engine::{
    api::runtime_error::RuntimeError,
    atom::Atom,
    heap::{
        ContextId,
        runtime::{RuntimeState, owned_values::OwnedValueGuard},
    },
    value::{JsValue, conversion::NativeConversion},
};
use std::cell::Cell;

/// Only this suffix's formatting failure constructs a fresh Error. A child
/// ToPrimitive's propagated Throw is handled by its owning Query beforehand.
pub(crate) enum PropertyKeyAtomStep {
    Value(Atom),
    CyclePublishedThrow(JsValue),
}
impl PropertyKeyAtomStep {
    fn into_conversion(self) -> NativeConversion<Atom> {
        match self {
            Self::Value(atom) => NativeConversion::Value(atom),
            Self::CyclePublishedThrow(value) => NativeConversion::Throw(value),
        }
    }
}

impl RuntimeState {
    /// The source remains live through this callback-free suffix. Every Value
    /// reply owns one atom edge, including a checked Symbol retain.
    pub(crate) fn property_key_atom_from_primitive_with_publication(
        &mut self,
        poisoned: &Cell<bool>,
        realm: ContextId,
        value: &JsValue,
    ) -> Result<PropertyKeyAtomStep, RuntimeError> {
        if matches!(value, JsValue::Object(_)) {
            return Err(RuntimeError::Invariant(
                "property key conversion received an object",
            ));
        }
        if let Some(atom) = Self::immediate_numeric_property_key_atom(value) {
            return Ok(PropertyKeyAtomStep::Value(atom));
        }
        if let JsValue::Symbol(index) = value {
            let atom = self.atoms.brand(*index)?;
            return Ok(PropertyKeyAtomStep::Value(self.atoms.retain(atom)?));
        }
        if let JsValue::String(id) = value {
            return Ok(PropertyKeyAtomStep::Value(
                self.intern_property_key_string_id(*id)?,
            ));
        }
        match self.string_from_primitive_jsvalue(poisoned, realm, value)? {
            NativeConversion::Value(text) => Ok(PropertyKeyAtomStep::Value(
                self.intern_property_key_js_string(&text)?,
            )),
            NativeConversion::Throw(value) => Ok(PropertyKeyAtomStep::CyclePublishedThrow(value)),
        }
    }

    /// Consume the suffix input at its original retirement point. The reply
    /// owner is protected while retiring the input; fatal cleanup quarantines
    /// State before another edge can be traversed.
    pub(crate) fn property_key_from_primitive_jsvalue(
        &mut self,
        poisoned: &Cell<bool>,
        realm: ContextId,
        value: JsValue,
    ) -> Result<NativeConversion<Atom>, RuntimeError> {
        self.property_key_from_primitive_jsvalue_with_publication(poisoned, realm, value)
            .map(PropertyKeyAtomStep::into_conversion)
    }

    pub(crate) fn property_key_from_primitive_jsvalue_with_publication(
        &mut self,
        poisoned: &Cell<bool>,
        realm: ContextId,
        value: JsValue,
    ) -> Result<PropertyKeyAtomStep, RuntimeError> {
        let mut input = OwnedValueGuard::new(self, poisoned, value);
        let (state, input) = input.parts();
        let result = state.property_key_atom_from_primitive_with_publication(
            poisoned,
            realm,
            input.as_ref().expect("property key input owner"),
        );
        if poisoned.get() {
            return result;
        }
        match result {
            Ok(PropertyKeyAtomStep::CyclePublishedThrow(value)) => {
                let mut thrown = OwnedValueGuard::new(state, poisoned, value);
                let (state, thrown) = thrown.parts();
                state.release_owned_jsvalue(
                    poisoned,
                    input.take().expect("property key input owner"),
                )?;
                Ok(PropertyKeyAtomStep::CyclePublishedThrow(
                    thrown.take().expect("property key error owner"),
                ))
            }
            result => {
                let retired = state.release_owned_jsvalue(
                    poisoned,
                    input.take().expect("property key input owner"),
                );
                match (result, retired) {
                    (Ok(value), Ok(())) => Ok(value),
                    (_, Err(error)) | (Err(error), Ok(())) => Err(error),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
