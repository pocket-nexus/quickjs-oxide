//! Complete read-only Date bodies use their admitted State and borrowed host.
use super::calendar::get_date_fields;
use super::format::{DateStringKind, format_date_string};
use crate::engine::{
    api::{
        error::{NativeErrorKind, NativeErrorMessage},
        runtime::RuntimeUnwindGuard,
        runtime_error::RuntimeError,
    },
    builtins::native::{DateNativeKind, DateStringMethod},
    heap::{
        ContextId, ObjectId, ObjectPayload,
        runtime::{RuntimeState, owned_values::OwnedValueGuard},
    },
    host::HostServices,
    value::{JsString, JsValue, conversion::NativeConversion, number::operations::Number},
    vm::{Completion, call::NativeInvocation},
};
use std::cell::Cell;

fn date_format_kind(method: DateStringMethod) -> DateStringKind {
    match method {
        DateStringMethod::String => DateStringKind::String,
        DateStringMethod::DateString => DateStringKind::DateString,
        DateStringMethod::TimeString => DateStringKind::TimeString,
        DateStringMethod::UtcString => DateStringKind::UtcString,
        DateStringMethod::IsoString => DateStringKind::IsoString,
        DateStringMethod::LocaleString => DateStringKind::LocaleString,
        DateStringMethod::LocaleDateString => DateStringKind::LocaleDateString,
        DateStringMethod::LocaleTimeString => DateStringKind::LocaleTimeString,
    }
}

impl RuntimeState {
    pub(super) fn genuine_date_value(&self, value: &JsValue) -> Result<Option<f64>, RuntimeError> {
        let JsValue::Object(object) = value else {
            return Ok(None);
        };
        Ok(match &self.heap.object(*object)?.payload {
            ObjectPayload::Date(value) => Some(*value),
            _ => None,
        })
    }

    /// Return the original checked temporary receiver edge. Each caller must
    /// retire it at its established consumption point, including error exits.
    pub(super) fn date_this_time_value_jsvalue(
        &mut self,
        poisoned: &Cell<bool>,
        realm: ContextId,
        this_value: &JsValue,
    ) -> Result<NativeConversion<(ObjectId, f64)>, RuntimeError> {
        let JsValue::Object(object) = this_value else {
            return Ok(NativeConversion::Throw(self.date_error(
                poisoned,
                realm,
                NativeErrorKind::Type,
                "not a Date object",
            )?));
        };
        self.heap.retain_object(*object)?;
        let mut receiver = OwnedValueGuard::new(self, poisoned, JsValue::Object(*object));
        let (state, receiver) = receiver.parts();
        if let Some(value) = state.genuine_date_value(this_value)? {
            receiver.take();
            return Ok(NativeConversion::Value((*object, value)));
        }
        let error =
            state.date_error(poisoned, realm, NativeErrorKind::Type, "not a Date object")?;
        let mut error_owner = OwnedValueGuard::new(state, poisoned, error);
        let (state, error_owner) = error_owner.parts();
        state.release_owned_jsvalue(
            poisoned,
            receiver.take().expect("checked Date receiver temporary"),
        )?;
        Ok(NativeConversion::Throw(
            error_owner.take().expect("Date brand error result"),
        ))
    }

    /// This fixed group contains every input shape of all 28 read-only Date
    /// selectors. Clock/timezone providers forbid reentry and borrow the
    /// existing Runtime header's host service without another Rc owner.
    pub(crate) fn call_date_readonly_native(
        &mut self,
        poisoned: &Cell<bool>,
        host: &dyn HostServices,
        realm: ContextId,
        kind: DateNativeKind,
        invocation: &NativeInvocation,
    ) -> Result<Completion, RuntimeError> {
        let _unwind = RuntimeUnwindGuard::from_flag(poisoned);
        let NativeInvocation::Call { this_value } = invocation else {
            return Err(RuntimeError::Invariant(
                "read-only Date native did not receive a generic invocation",
            ));
        };
        match kind {
            DateNativeKind::Now => {
                return Ok(Completion::Return(
                    Number::compact(host.now_millis() as f64).into(),
                ));
            }
            DateNativeKind::TimeValue
            | DateNativeKind::String(_)
            | DateNativeKind::TimezoneOffset
            | DateNativeKind::GetField(_) => {}
            _ => {
                return Err(RuntimeError::Invariant(
                    "converting Date method reached read-only state body",
                ));
            }
        }
        let (object, value) =
            match self.date_this_time_value_jsvalue(poisoned, realm, this_value)? {
                NativeConversion::Value(value) => value,
                NativeConversion::Throw(error) => return Ok(Completion::Throw(error)),
            };
        let mut receiver = OwnedValueGuard::new(self, poisoned, JsValue::Object(object));
        let (state, receiver) = receiver.parts();
        // Every original read-only handler discarded the ObjectRef with
        // `let (_, value) = ...` before consulting host policy or formatting.
        // Keep that checked temporary release at the same observation point.
        state.release_owned_jsvalue(
            poisoned,
            receiver.take().expect("Date body receiver temporary"),
        )?;
        state.finish_date_readonly(poisoned, host, realm, kind, value)
    }

    fn finish_date_readonly(
        &mut self,
        poisoned: &Cell<bool>,
        host: &dyn HostServices,
        realm: ContextId,
        kind: DateNativeKind,
        value: f64,
    ) -> Result<Completion, RuntimeError> {
        let number = match kind {
            DateNativeKind::TimeValue => value,
            DateNativeKind::String(method) => {
                // toGMTString remains the same installed callable as
                // toUTCString, rather than an additional formatter selector.
                let kind = date_format_kind(method);
                let fields = get_date_fields(value, kind.uses_local_time(), false, |instant| {
                    host.timezone_offset_minutes(instant)
                });
                let output = match format_date_string(fields.as_ref(), kind) {
                    Ok(output) => output,
                    Err(_) => {
                        return Ok(Completion::Throw(self.date_error(
                            poisoned,
                            realm,
                            NativeErrorKind::Range,
                            "Date value is NaN",
                        )?));
                    }
                };
                return Ok(Completion::Return(JsValue::String(
                    self.heap
                        .allocate_string(JsString::try_from_utf8(&output)?)?,
                )));
            }
            DateNativeKind::GetField(field) => {
                let Some(fields) =
                    get_date_fields(value, field.uses_local_time(), false, |instant| {
                        host.timezone_offset_minutes(instant)
                    })
                else {
                    return Ok(Completion::Return(Number::compact(f64::NAN).into()));
                };
                let value = fields[usize::from(field.field_index())];
                if field.is_legacy_year() {
                    value - 1900.0
                } else {
                    value
                }
            }
            DateNativeKind::TimezoneOffset => {
                if value.is_nan() {
                    f64::NAN
                } else {
                    f64::from(host.timezone_offset_minutes(value.trunc() as i64))
                }
            }
            _ => {
                return Err(RuntimeError::Invariant(
                    "Date read-only selector did not reach its body",
                ));
            }
        };
        Ok(Completion::Return(Number::compact(number).into()))
    }

    fn date_error(
        &mut self,
        poisoned: &Cell<bool>,
        realm: ContextId,
        kind: NativeErrorKind,
        message: &str,
    ) -> Result<JsValue, RuntimeError> {
        let mut text = NativeErrorMessage::new();
        text.push_utf8(message);
        self.new_native_error_from_message(poisoned, realm, kind, text)
            .map(JsValue::Object)
    }
}

#[cfg(test)]
mod tests;
