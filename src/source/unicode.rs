//! The checksum-pinned Unicode 17 tables of the `oxide-regexp` crate at their
//! historical path. Case conversion and normalization keep `JsString`
//! wrappers here: they hand the crate the string's flat code units and wrap
//! its result in the storage the crate chose.

pub use oxide_regexp::unicode::{is_id_continue, is_id_start, property};

use crate::engine::value::{JsString, JsStringError};
use oxide_regexp::unicode::{CodeUnitBuf, StringError};

const _: () = assert!(JsString::MAX_LEN == oxide_regexp::unicode::MAX_STRING_LEN);

/// Run `latin1` or `utf16` over the flat code units of `input`. A rope is
/// read into a temporary buffer rather than linearized, so its representation
/// does not change.
fn with_code_units<R>(
    input: &JsString,
    latin1: impl FnOnce(&[u8]) -> R,
    utf16: impl FnOnce(&[u16]) -> R,
) -> R {
    if let Some(units) = input.flat_latin1() {
        latin1(units)
    } else if let Some(units) = input.flat_utf16() {
        utf16(units)
    } else {
        utf16(&input.utf16_units().collect::<Vec<_>>())
    }
}

fn into_js_string(output: Result<CodeUnitBuf, StringError>) -> Result<JsString, JsStringError> {
    match output {
        Ok(CodeUnitBuf::Latin1(units)) => Ok(JsString::from_owned_latin1(units)),
        Ok(CodeUnitBuf::Utf16(units)) => Ok(JsString::from_owned_utf16(units)),
        Err(error) => Err(into_js_string_error(error)),
    }
}

const fn into_js_string_error(error: StringError) -> JsStringError {
    match error {
        StringError::TooLong => JsStringError::TooLong,
        StringError::OutOfMemory => JsStringError::OutOfMemory,
    }
}

pub mod case {
    //! Case conversion over engine strings; see `oxide_regexp::unicode::case`.

    pub use oxide_regexp::unicode::case::*;

    use crate::engine::value::{JsString, JsStringError};

    /// Convert one raw UTF-16 string with QuickJS 2026-06-04's Unicode 17
    /// case tables and contextual lowercase rule. An empty input is returned
    /// as is, without reserving a buffer.
    pub fn convert_case_with_limit(
        input: &JsString,
        upper: bool,
        max_len: usize,
    ) -> Result<JsString, JsStringError> {
        if input.is_empty() {
            return Ok(input.clone());
        }
        super::into_js_string(super::with_code_units(
            input,
            |units| oxide_regexp::unicode::case::convert_case_with_limit(units, upper, max_len),
            |units| oxide_regexp::unicode::case::convert_case_with_limit(units, upper, max_len),
        ))
    }
}

pub mod normalize {
    //! Normalization over engine strings; see
    //! `oxide_regexp::unicode::normalize`.

    pub use oxide_regexp::unicode::normalize::*;

    use crate::engine::value::{JsString, JsStringError};

    /// Normalize to code points; lone surrogates are kept.
    pub fn normalize_code_points(
        input: &JsString,
        form: NormalizationForm,
    ) -> Result<Vec<u32>, JsStringError> {
        super::with_code_units(
            input,
            |units| oxide_regexp::unicode::normalize::normalize_code_points(units, form),
            |units| oxide_regexp::unicode::normalize::normalize_code_points(units, form),
        )
        .map_err(super::into_js_string_error)
    }

    /// [`normalize`] with a UTF-16 output length limit.
    pub fn normalize_with_limit(
        input: &JsString,
        form: NormalizationForm,
        max_len: usize,
    ) -> Result<JsString, JsStringError> {
        super::into_js_string(super::with_code_units(
            input,
            |units| oxide_regexp::unicode::normalize::normalize_with_limit(units, form, max_len),
            |units| oxide_regexp::unicode::normalize::normalize_with_limit(units, form, max_len),
        ))
    }

    /// Normalize an ECMAScript UTF-16 string with QuickJS 2026-06-04's
    /// Unicode 17 data. Valid surrogate pairs are decoded and re-encoded;
    /// lone surrogates are preserved as lone code units.
    pub fn normalize(input: &JsString, form: NormalizationForm) -> Result<JsString, JsStringError> {
        normalize_with_limit(input, form, JsString::MAX_LEN)
    }
}

#[cfg(test)]
mod tests {
    use super::case::{convert_case_with_limit, fail_next_case_reservation_for_test};
    use super::normalize::{NormalizationForm, normalize, normalize_code_points};
    use crate::engine::value::{JsString, JsStringError};

    #[test]
    fn empty_case_conversion_returns_the_input_without_reserving() {
        let empty = JsString::from_static("");
        fail_next_case_reservation_for_test();
        let converted_empty = convert_case_with_limit(&empty, true, 0).unwrap();
        assert!(converted_empty.same_representation(&empty));

        // The empty fast path did not consume the injected allocation failure.
        let ascii = JsString::from_static("a");
        assert_eq!(
            convert_case_with_limit(&ascii, true, 1),
            Err(JsStringError::OutOfMemory)
        );
    }

    #[test]
    fn wrappers_read_latin1_utf16_and_rope_strings() {
        let latin1 = JsString::try_from_utf8("ÿ").unwrap();
        assert!(!latin1.is_wide());
        let upper = convert_case_with_limit(&latin1, true, JsString::MAX_LEN).unwrap();
        assert_eq!(upper.to_utf8_lossy(), "Ÿ");
        assert!(upper.is_wide());

        let wide = JsString::try_from_utf8("Ÿ").unwrap();
        let lower = convert_case_with_limit(&wide, false, JsString::MAX_LEN).unwrap();
        assert_eq!(lower.to_utf8_lossy(), "ÿ");
        assert!(!lower.is_wide());

        let rope = JsString::try_from_utf8(&"e".repeat(600))
            .unwrap()
            .try_concat(&JsString::try_from_utf8("\u{301}").unwrap())
            .unwrap();
        let normalized = normalize(&rope, NormalizationForm::Nfc).unwrap();
        assert_eq!(normalized.len(), 600);
        assert_eq!(normalized.code_unit_at(599), Some(0xe9));
        assert_eq!(
            normalize_code_points(&rope, NormalizationForm::Nfd)
                .unwrap()
                .len(),
            601
        );
    }
}
