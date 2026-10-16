//! Full Unicode case mapping for `toLowerCase` and `toUpperCase` past their
//! ASCII pass, from the tables of `oxide-regexp` (the current core's own),
//! including the final-sigma rule and the multi-unit expansions.

use oxide_regexp::unicode::case::convert_case_with_limit;
use oxide_regexp::unicode::{CodeUnitBuf, MAX_STRING_LEN};

/// `String.prototype.toLowerCase` of UTF-16 text; `None` past the string
/// length limit.
pub(crate) fn to_lower(text: &[u16]) -> Option<Vec<u16>> {
    convert(text, false)
}

/// `String.prototype.toUpperCase` of UTF-16 text.
pub(crate) fn to_upper(text: &[u16]) -> Option<Vec<u16>> {
    convert(text, true)
}

fn convert(text: &[u16], upper: bool) -> Option<Vec<u16>> {
    match convert_case_with_limit(text, upper, MAX_STRING_LEN).ok()? {
        CodeUnitBuf::Latin1(bytes) => Some(bytes.into_iter().map(u16::from).collect()),
        CodeUnitBuf::Utf16(units) => Some(units),
    }
}
