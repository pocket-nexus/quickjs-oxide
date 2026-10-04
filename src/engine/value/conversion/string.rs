//! ToString replies consume their primitive edge through the admitted State.
use crate::engine::{
    api::{
        error::{NativeErrorKind, NativeErrorMessage},
        runtime_error::RuntimeError,
    },
    heap::{
        ContextId,
        runtime::{RuntimeState, owned_values::OwnedValueGuard},
    },
    value::{JsString, JsValue, conversion::NativeConversion},
    vm::Completion,
};
use std::cell::Cell;

/// ToString may preserve a child's throw or create its own Error. Only the
/// latter carries a collectible publication fact.
pub(crate) enum StringPrimitiveStep {
    Value(JsString),
    Throw(JsValue),
    CyclePublishedThrow(JsValue),
}
impl StringPrimitiveStep {
    pub(crate) fn into_conversion(self) -> NativeConversion<JsString> {
        match self {
            Self::Value(value) => NativeConversion::Value(value),
            Self::Throw(value) | Self::CyclePublishedThrow(value) => NativeConversion::Throw(value),
        }
    }
}

impl RuntimeState {
    pub(crate) fn string_from_primitive_jsvalue(
        &mut self,
        poisoned: &Cell<bool>,
        realm: ContextId,
        value: &JsValue,
    ) -> Result<NativeConversion<JsString>, RuntimeError> {
        self.string_from_primitive_jsvalue_with_publication(poisoned, realm, value)
            .map(StringPrimitiveStep::into_conversion)
    }

    pub(crate) fn string_from_primitive_jsvalue_with_publication(
        &mut self,
        poisoned: &Cell<bool>,
        realm: ContextId,
        value: &JsValue,
    ) -> Result<StringPrimitiveStep, RuntimeError> {
        if matches!(value, JsValue::Object(_)) {
            return Err(RuntimeError::Invariant(
                "ToString primitive completion received an object",
            ));
        }
        match self.to_js_string_jsvalue(value) {
            Ok(value) => Ok(StringPrimitiveStep::Value(value.linearize())),
            Err(error) => {
                let Some(kind) = NativeErrorKind::from_javascript_error(error.kind()) else {
                    return Err(RuntimeError::Engine(error));
                };
                let message = error
                    .native_message()
                    .cloned()
                    .unwrap_or_else(|| NativeErrorMessage::from_utf8(error.message()));
                Ok(StringPrimitiveStep::CyclePublishedThrow(JsValue::Object(
                    self.new_native_error_from_message(poisoned, realm, kind, message)?,
                )))
            }
        }
    }

    pub(crate) fn finish_string_value(
        &mut self,
        poisoned: &Cell<bool>,
        realm: ContextId,
        completion: Completion,
    ) -> Result<NativeConversion<JsString>, RuntimeError> {
        self.finish_string_value_with_publication(poisoned, realm, completion)
            .map(StringPrimitiveStep::into_conversion)
    }

    pub(crate) fn finish_string_value_with_publication(
        &mut self,
        poisoned: &Cell<bool>,
        realm: ContextId,
        completion: Completion,
    ) -> Result<StringPrimitiveStep, RuntimeError> {
        let value = match completion {
            Completion::Throw(value) => return Ok(StringPrimitiveStep::Throw(value)),
            Completion::Return(value) => value,
        };
        let mut input_owner = OwnedValueGuard::new(self, poisoned, value);
        let (state, input_owner) = input_owner.parts();
        let result = state.string_from_primitive_jsvalue_with_publication(
            poisoned,
            realm,
            input_owner.as_ref().expect("string conversion input"),
        );
        if poisoned.get() {
            return result;
        }
        match result {
            Ok(StringPrimitiveStep::CyclePublishedThrow(value)) => {
                let mut throw_owner = OwnedValueGuard::new(state, poisoned, value);
                let (state, throw_owner) = throw_owner.parts();
                state.release_owned_jsvalue(
                    poisoned,
                    input_owner.take().expect("string input owner"),
                )?;
                Ok(StringPrimitiveStep::CyclePublishedThrow(
                    throw_owner.take().expect("string error owner"),
                ))
            }
            result => {
                state.release_owned_jsvalue(
                    poisoned,
                    input_owner.take().expect("string input owner"),
                )?;
                result
            }
        }
    }
}

#[cfg(test)]
mod tests;
