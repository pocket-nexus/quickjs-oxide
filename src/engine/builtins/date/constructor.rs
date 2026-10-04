//! Pinned QuickJS `Date` constructor and static native handlers.
//!
//! The observable conversion order in this file follows QuickJS 2026-06-04
//! `js_date_constructor`, `js_Date_UTC`, `js_Date_parse`, and `js_Date_now`.
//! In particular, a function call ignores every supplied argument, the
//! multi-argument forms coerce at most seven values from left to right before
//! inspecting finiteness, and parsing applies its explicit offset after the
//! calendar kernel's TimeClip without clipping the static `Date.parse` result a
//! second time.

use crate::engine::builtins::native::DateNativeKind;

use super::calendar::{DateInputFields, get_date_fields, set_date_fields, time_clip};
use super::format::{DateStringKind, format_date_string};
use super::parse::ParsedDateString;
use crate::engine::api::runtime::Runtime;
use crate::engine::api::runtime_error::RuntimeError;

use crate::engine::heap::{ContextId, ObjectData, ObjectId, runtime::RuntimeState};
use crate::engine::host::HostServices;
use std::cell::Cell;

use crate::engine::value::{JsString, JsValue};
use crate::engine::vm::Completion;
use crate::engine::vm::call::{NativeArguments, NativeInvocation};

pub(crate) mod operation;

const DEFAULT_DATE_FIELDS: DateInputFields = [0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0];
const MAX_DATE_ARGUMENTS: usize = DEFAULT_DATE_FIELDS.len();
const MS_PER_MINUTE: f64 = 60_000.0;

impl Runtime {
    /// Dispatch the constructor and three static functions represented by the
    /// constructor-side portion of [`DateNativeKind`]. Prototype operations
    /// are deliberately rejected here so `date/mod.rs` remains the sole
    /// top-level Date dispatcher.
    pub(crate) fn call_date_constructor_native(
        &self,
        realm: ContextId,
        kind: DateNativeKind,
        invocation: &NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Completion, RuntimeError> {
        operation::finish(
            self,
            realm,
            operation::DateConstructorStep::start(self, realm, kind, invocation, arguments)?,
        )
    }
}

impl RuntimeState {
    fn call_date_as_function(
        &mut self,
        host: &dyn HostServices,
    ) -> Result<Completion, RuntimeError> {
        let date_value = host.now_millis() as f64;
        let fields = get_date_fields(date_value, true, false, |instant| {
            host.timezone_offset_minutes(instant)
        });
        let text = format_date_string(fields.as_ref(), DateStringKind::String).map_err(|_| {
            RuntimeError::Invariant("the host clock produced an invalid Date string")
        })?;
        Ok(Completion::Return(JsValue::String(
            self.heap.allocate_string(JsString::try_from_utf8(&text)?)?,
        )))
    }

    /// The prototype input is borrowed from a real owned getter/fallback edge.
    /// The shared allocator owns publication status and shape rollback.
    fn new_date_object(
        &mut self,
        poisoned: &Cell<bool>,
        prototype: ObjectId,
        value: f64,
    ) -> Result<ObjectId, RuntimeError> {
        let value = time_clip(value);
        self.allocate_object_with_layout(
            poisoned,
            Some(prototype),
            &[],
            Vec::new(),
            |shape, slots| ObjectData::date(shape, slots, value),
        )
    }
}

/// Convert parser output to the static `Date.parse` result. The explicit
/// timezone subtraction intentionally occurs after `set_date_fields` and is
/// intentionally *not* followed by TimeClip, matching pinned QuickJS.
fn parsed_date_value<F>(parsed: Option<ParsedDateString>, timezone_offset: F) -> f64
where
    F: FnMut(i64) -> i32,
{
    let Some(parsed) = parsed else {
        return f64::NAN;
    };
    let mut fields = [0.0; 7];
    for (target, source) in fields.iter_mut().zip(parsed.fields) {
        *target = f64::from(source);
    }
    let value = set_date_fields(&fields, parsed.is_local, timezone_offset);
    let explicit_offset = f64::from(parsed.fields[8]) * MS_PER_MINUTE;
    value - explicit_offset
}

#[cfg(test)]
mod tests {
    use super::super::calendar::set_date_fields_checked;
    use super::*;

    const UTC: fn(i64) -> i32 = |_| 0;

    #[test]
    fn date_parse_applies_explicit_offset_after_inner_time_clip_without_reclipping() {
        let parsed = ParsedDateString {
            fields: [275_760, 8, 13, 0, 0, 0, 0, 0, -60],
            is_local: false,
        };

        let value = parsed_date_value(Some(parsed), UTC);
        assert_eq!(value, 8.64e15 + 3_600_000.0);
        assert!(time_clip(value).is_nan());
    }

    #[test]
    fn date_parse_invalid_syntax_is_nan() {
        assert!(parsed_date_value(None, UTC).is_nan());
    }

    #[test]
    fn utc_defaults_and_legacy_year_adjustment_match_quickjs() {
        assert_eq!(
            set_date_fields_checked(DEFAULT_DATE_FIELDS, false, UTC),
            -2_208_988_800_000.0
        );
        let mut fields = DEFAULT_DATE_FIELDS;
        fields[0] = 99.0;
        assert_eq!(
            set_date_fields_checked(fields, false, UTC),
            915_148_800_000.0
        );
    }
}
