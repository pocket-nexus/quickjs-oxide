//! Shared pure length validation under the current State access.
use super::*;
use crate::engine::api::error::{Error, ErrorKind, NativeErrorKind, NativeErrorMessage};

impl RuntimeState {
    pub(crate) fn array_length_from_primitive(
        &mut self,
        poisoned: &Cell<bool>,
        realm: Option<ContextId>,
        value: &JsValue,
    ) -> Result<ArrayLengthConversion, RuntimeError> {
        match crate::engine::vm::to_number_jsvalue_in_state(self, value) {
            Ok(number) => self.validate_array_length_number_in_state(poisoned, realm, number, None),
            Err(error) => {
                let Some(realm) = realm else {
                    return Err(RuntimeError::Engine(error));
                };
                let Some(kind) = NativeErrorKind::from_javascript_error(error.kind()) else {
                    return Err(RuntimeError::Engine(error));
                };
                let message = error
                    .native_message()
                    .cloned()
                    .unwrap_or_else(|| NativeErrorMessage::from_utf8(error.message()));
                self.new_native_error_from_message(poisoned, realm, kind, message)
                    .map(|object| ArrayLengthConversion::Throw(JsValue::Object(object)))
            }
        }
    }
    pub(crate) fn validate_array_length_number_in_state(
        &mut self,
        poisoned: &Cell<bool>,
        realm: Option<ContextId>,
        value: f64,
        expected_uint32: Option<u32>,
    ) -> Result<ArrayLengthConversion, RuntimeError> {
        if value >= 0.0 && value <= f64::from(u32::MAX) && value.fract() == 0.0 {
            let length = value as u32;
            if expected_uint32.is_none_or(|expected| expected == length) {
                return Ok(ArrayLengthConversion::Length(length));
            }
        }
        self.invalid_array_length_in_state(poisoned, realm)
    }
    pub(crate) fn invalid_array_length_in_state(
        &mut self,
        poisoned: &Cell<bool>,
        realm: Option<ContextId>,
    ) -> Result<ArrayLengthConversion, RuntimeError> {
        match realm {
            Some(realm) => self
                .new_native_error_from_message(
                    poisoned,
                    realm,
                    NativeErrorKind::Range,
                    NativeErrorMessage::from_utf8("invalid array length"),
                )
                .map(|object| ArrayLengthConversion::Throw(JsValue::Object(object))),
            None => Err(RuntimeError::Engine(Error::new(
                ErrorKind::Range,
                "invalid array length",
            ))),
        }
    }
}
