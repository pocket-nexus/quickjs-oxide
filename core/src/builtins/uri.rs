//! The URI functions and the Annex B `escape` and `unescape`: QuickJS's codecs
//! over UTF-16 code units.

use super::{Native, methods, string_units, to_string_id};
use crate::heap::HeapError;
use crate::value::Value;
use crate::vm::native::CallSite;
use crate::vm::{Cx, Throw, error};

const HEX: &[u8; 16] = b"0123456789ABCDEF";

pub(super) fn install(cx: &mut Cx<'_>) -> Result<(), HeapError> {
    let global = cx.realm.global();
    methods(
        cx,
        global,
        &[
            Native::DecodeUri,
            Native::DecodeUriComponent,
            Native::EncodeUri,
            Native::EncodeUriComponent,
            Native::Escape,
            Native::Unescape,
        ],
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum UriCodecError {
    ExpectingPercent,
    ExpectingHexDigit,
    MalformedUtf8,
    InvalidCharacter,
    ExpectingSurrogatePair,
}

impl UriCodecError {
    const fn message(self) -> &'static str {
        match self {
            Self::ExpectingPercent => "expecting %",
            Self::ExpectingHexDigit => "expecting hex digit",
            Self::MalformedUtf8 => "malformed UTF-8",
            Self::InvalidCharacter => "invalid character",
            Self::ExpectingSurrogatePair => "expecting surrogate pair",
        }
    }
}

fn argument_units(cx: &mut Cx<'_>, site: &CallSite) -> Result<Vec<u16>, Throw> {
    let value = site.arg(cx, 0);
    let string = to_string_id(cx, value)?;
    Ok(cx.heap.string(string).repr().units().collect())
}

fn finish(cx: &mut Cx<'_>, result: Result<Vec<u16>, UriCodecError>) -> Result<Value, Throw> {
    match result {
        Ok(units) => string_units(cx, &units),
        Err(failure) => Err(error::throw(cx, crate::ErrorKind::Uri, failure.message())),
    }
}

pub(super) fn decode_uri(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let units = argument_units(cx, &site)?;
    finish(cx, decode(&units, false))
}

pub(super) fn decode_uri_component(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let units = argument_units(cx, &site)?;
    finish(cx, decode(&units, true))
}

pub(super) fn encode_uri(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let units = argument_units(cx, &site)?;
    finish(cx, encode(&units, false))
}

pub(super) fn encode_uri_component(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let units = argument_units(cx, &site)?;
    finish(cx, encode(&units, true))
}

pub(super) fn escape(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let units = argument_units(cx, &site)?;
    let mut output = Vec::with_capacity(units.len());
    for unit in units {
        if is_legacy_unescaped(unit) {
            output.push(unit);
        } else if unit <= 0xff {
            output.push(u16::from(b'%'));
            output.push(u16::from(HEX[usize::from((unit >> 4) & 0x0f)]));
            output.push(u16::from(HEX[usize::from(unit & 0x0f)]));
        } else {
            output.extend("%u".encode_utf16());
            for shift in [12, 8, 4, 0] {
                output.push(u16::from(HEX[usize::from((unit >> shift) & 0x0f)]));
            }
        }
    }
    string_units(cx, &output)
}

pub(super) fn unescape(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let units = argument_units(cx, &site)?;
    let mut output = Vec::with_capacity(units.len());
    let mut index = 0;
    while index < units.len() {
        if units[index] == u16::from(b'%') {
            if index + 6 <= units.len()
                && units[index + 1] == u16::from(b'u')
                && let Some(value) = decode_hex_units(&units[index + 2..index + 6])
            {
                output.push(value);
                index += 6;
                continue;
            }
            if index + 3 <= units.len()
                && let Some(value) = decode_hex_units(&units[index + 1..index + 3])
            {
                output.push(value);
                index += 3;
                continue;
            }
        }
        output.push(units[index]);
        index += 1;
    }
    string_units(cx, &output)
}

/// `decodeURI` (`component == false`) or `decodeURIComponent`.
fn decode(units: &[u16], component: bool) -> Result<Vec<u16>, UriCodecError> {
    let mut output = Vec::with_capacity(units.len());
    let mut index = 0;
    while index < units.len() {
        if units[index] != u16::from(b'%') {
            output.push(units[index]);
            index += 1;
            continue;
        }
        let first = decode_percent_byte(units, index)?;
        index += 3;
        if first < 0x80 {
            if !component && is_uri_reserved(first) {
                // QuickJS preserves the original hex digit spelling and case.
                output.push(u16::from(b'%'));
                output.push(units[index - 2]);
                output.push(units[index - 1]);
            } else {
                output.push(u16::from(first));
            }
            continue;
        }
        let (continuations, minimum, mut code_point) = match first {
            0xc0..=0xdf => (1, 0x80, u32::from(first & 0x1f)),
            0xe0..=0xef => (2, 0x800, u32::from(first & 0x0f)),
            0xf0..=0xf7 => (3, 0x1_0000, u32::from(first & 0x07)),
            _ => (0, 1, 0),
        };
        for _ in 0..continuations {
            let byte = decode_percent_byte(units, index)?;
            index += 3;
            if byte & 0xc0 != 0x80 {
                code_point = 0;
                break;
            }
            code_point = (code_point << 6) | u32::from(byte & 0x3f);
        }
        if code_point < minimum || code_point > 0x10_ffff || (0xd800..=0xdfff).contains(&code_point)
        {
            return Err(UriCodecError::MalformedUtf8);
        }
        push_code_point(&mut output, code_point);
    }
    Ok(output)
}

/// `encodeURI` (`component == false`) or `encodeURIComponent`.
fn encode(units: &[u16], component: bool) -> Result<Vec<u16>, UriCodecError> {
    let mut output = Vec::with_capacity(units.len());
    let mut index = 0;
    while index < units.len() {
        let first = units[index];
        index += 1;
        if is_uri_unescaped(first, component) {
            output.push(first);
            continue;
        }
        let code_point = if is_low_surrogate(first) {
            return Err(UriCodecError::InvalidCharacter);
        } else if is_high_surrogate(first) {
            let Some(&second) = units.get(index) else {
                return Err(UriCodecError::ExpectingSurrogatePair);
            };
            index += 1;
            if !is_low_surrogate(second) {
                return Err(UriCodecError::ExpectingSurrogatePair);
            }
            0x1_0000 + ((u32::from(first) - 0xd800) << 10) + (u32::from(second) - 0xdc00)
        } else {
            u32::from(first)
        };
        push_utf8_percent_encoding(&mut output, code_point);
    }
    Ok(output)
}

fn decode_percent_byte(units: &[u16], index: usize) -> Result<u8, UriCodecError> {
    if units.get(index).copied() != Some(u16::from(b'%')) {
        return Err(UriCodecError::ExpectingPercent);
    }
    let Some(digits) = units.get(index + 1..index + 3) else {
        return Err(UriCodecError::ExpectingHexDigit);
    };
    decode_hex_units(digits)
        .and_then(|value| u8::try_from(value).ok())
        .ok_or(UriCodecError::ExpectingHexDigit)
}

fn decode_hex_units(units: &[u16]) -> Option<u16> {
    let mut value = 0_u16;
    for &unit in units {
        value = (value << 4) | u16::from(hex_value(unit)?);
    }
    Some(value)
}

fn hex_value(unit: u16) -> Option<u8> {
    match unit {
        0x30..=0x39 => Some(u8::try_from(unit - 0x30).expect("decimal hex digit fits u8")),
        0x41..=0x46 => Some(u8::try_from(unit - 0x41 + 10).expect("upper hex digit fits u8")),
        0x61..=0x66 => Some(u8::try_from(unit - 0x61 + 10).expect("lower hex digit fits u8")),
        _ => None,
    }
}

fn is_uri_reserved(byte: u8) -> bool {
    b";/?:@&=+$,#".contains(&byte)
}

fn is_uri_unescaped(unit: u16, component: bool) -> bool {
    let Ok(byte) = u8::try_from(unit) else {
        return false;
    };
    byte.is_ascii_alphanumeric()
        || b"-_.!~*'()".contains(&byte)
        || (!component && is_uri_reserved(byte))
}

fn is_legacy_unescaped(unit: u16) -> bool {
    let Ok(byte) = u8::try_from(unit) else {
        return false;
    };
    byte.is_ascii_alphanumeric() || b"@*_+-./".contains(&byte)
}

fn is_high_surrogate(unit: u16) -> bool {
    (0xd800..=0xdbff).contains(&unit)
}

fn is_low_surrogate(unit: u16) -> bool {
    (0xdc00..=0xdfff).contains(&unit)
}

fn push_code_point(output: &mut Vec<u16>, code_point: u32) {
    if code_point < 0x1_0000 {
        output.push(u16::try_from(code_point).expect("validated BMP code point fits u16"));
    } else {
        let scalar = code_point - 0x1_0000;
        output.push(
            u16::try_from(0xd800 + (scalar >> 10))
                .expect("validated code point high surrogate fits u16"),
        );
        output.push(
            u16::try_from(0xdc00 + (scalar & 0x3ff))
                .expect("validated code point low surrogate fits u16"),
        );
    }
}

fn push_utf8_percent_encoding(output: &mut Vec<u16>, code_point: u32) {
    if code_point < 0x80 {
        push_percent_byte(
            output,
            u8::try_from(code_point).expect("ASCII code point fits u8"),
        );
    } else if code_point < 0x800 {
        push_percent_byte(
            output,
            u8::try_from((code_point >> 6) | 0xc0).expect("UTF-8 byte"),
        );
        push_percent_byte(
            output,
            u8::try_from((code_point & 0x3f) | 0x80).expect("UTF-8 byte"),
        );
    } else if code_point < 0x1_0000 {
        push_percent_byte(
            output,
            u8::try_from((code_point >> 12) | 0xe0).expect("UTF-8 byte"),
        );
        push_percent_byte(
            output,
            u8::try_from(((code_point >> 6) & 0x3f) | 0x80).expect("UTF-8 byte"),
        );
        push_percent_byte(
            output,
            u8::try_from((code_point & 0x3f) | 0x80).expect("UTF-8 byte"),
        );
    } else {
        push_percent_byte(
            output,
            u8::try_from((code_point >> 18) | 0xf0).expect("UTF-8 byte"),
        );
        push_percent_byte(
            output,
            u8::try_from(((code_point >> 12) & 0x3f) | 0x80).expect("UTF-8 byte"),
        );
        push_percent_byte(
            output,
            u8::try_from(((code_point >> 6) & 0x3f) | 0x80).expect("UTF-8 byte"),
        );
        push_percent_byte(
            output,
            u8::try_from((code_point & 0x3f) | 0x80).expect("UTF-8 byte"),
        );
    }
}

fn push_percent_byte(output: &mut Vec<u16>, byte: u8) {
    output.push(u16::from(b'%'));
    output.push(u16::from(HEX[usize::from(byte >> 4)]));
    output.push(u16::from(HEX[usize::from(byte & 0x0f)]));
}
