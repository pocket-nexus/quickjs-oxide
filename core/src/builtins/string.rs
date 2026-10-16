//! `String`: `String(v)`, `fromCharCode`, and the prototype methods the V8
//! v7 programs call. Strings are flat (ropes are phase 4), so `slice`,
//! `substring` and `concat` copy. `split`, `replace` and `match` dispatch on
//! the `RegExp` kind directly: the `Symbol.replace` protocol is phase 4's,
//! so a regular-expression argument runs the regular-expression algorithm
//! and anything else is converted to a string.

use super::{
    Native, install_constructor, methods, relative, string_repr, string_units, this_string,
    to_integer, to_string_id, unsupported,
};
use crate::heap::HeapError;
use crate::heap::object::ObjectKind;
use crate::heap::string::StringRepr;
use crate::runtime::Intrinsic;
use crate::value::{Kind, StringId, Value};
use crate::vm::native::CallSite;
use crate::vm::{Cx, Throw, error, number, ops};

pub(super) fn install(cx: &mut Cx<'_>) -> Result<(), HeapError> {
    let prototype = cx.realm.intrinsic(Intrinsic::StringPrototype);
    let string = install_constructor(cx, "String", Native::StringConstructor, prototype)?;
    methods(cx, string, &[("fromCharCode", Native::StringFromCharCode)])?;
    methods(
        cx,
        prototype,
        &[
            ("charAt", Native::StringCharAt),
            ("charCodeAt", Native::StringCharCodeAt),
            ("indexOf", Native::StringIndexOf),
            ("lastIndexOf", Native::StringLastIndexOf),
            ("slice", Native::StringSlice),
            ("substring", Native::StringSubstring),
            ("substr", Native::StringSubstr),
            ("concat", Native::StringConcat),
            ("split", Native::StringSplit),
            ("replace", Native::StringReplace),
            ("match", Native::StringMatch),
            ("toLowerCase", Native::StringToLowerCase),
            ("toUpperCase", Native::StringToUpperCase),
            ("toString", Native::StringValueOf),
            ("valueOf", Native::StringValueOf),
        ],
    )
}

fn units(cx: &Cx<'_>, string: StringId) -> Vec<u16> {
    cx.heap.string(string).repr.units().collect()
}

fn length(cx: &Cx<'_>, string: StringId) -> usize {
    cx.heap.string(string).repr.len()
}

/// A copy of `string[start..end]`, keeping the one-byte representation.
pub(crate) fn substring_value(
    cx: &mut Cx<'_>,
    string: StringId,
    start: usize,
    end: usize,
) -> Result<Value, Throw> {
    if start == 0 && end == length(cx, string) {
        return Ok(Value::string(string));
    }
    let repr = match &cx.heap.string(string).repr {
        StringRepr::OneByte(bytes) => StringRepr::OneByte(bytes[start..end].into()),
        StringRepr::TwoByte(units) => StringRepr::from_units(&units[start..end]),
    };
    string_repr(cx, repr)
}

/// `String(value)`: `ToString`, with a symbol's description; `new String`
/// makes a wrapper object, which is phase 4's.
pub(super) fn constructor(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    if site.construct {
        return Err(unsupported(cx, "new String"));
    }
    if site.argc == 0 {
        return Ok(Value::string(cx.realm.names.empty));
    }
    let value = site.arg(cx, 0);
    if let Some(symbol) = value.as_symbol() {
        let description = cx
            .heap
            .symbol_description(symbol)
            .map(|id| cx.heap.string_text(id))
            .unwrap_or_default();
        return super::string_text(cx, &format!("Symbol({description})"));
    }
    ops::to_string(cx, value)
}

pub(super) fn from_char_code(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let mut text = Vec::with_capacity(site.argc);
    for index in 0..site.argc {
        let code = site.arg(cx, index);
        let code = match code.as_int32() {
            Some(code) => code as u16,
            None => number::to_uint32(ops::to_number(cx, code)?) as u16,
        };
        text.push(code);
    }
    string_units(cx, &text)
}

pub(super) fn char_at(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let string = this_string(cx, &site)?;
    let position = site.arg(cx, 0);
    let position = to_integer(cx, position)?;
    let length = length(cx, string);
    if position < 0.0 || position >= length as f64 {
        return Ok(Value::string(cx.realm.names.empty));
    }
    let unit = cx.heap.string(string).repr.unit(position as usize);
    string_units(cx, &[unit])
}

pub(super) fn char_code_at(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let string = this_string(cx, &site)?;
    let position = site.arg(cx, 0);
    let position = match position.as_int32() {
        Some(position) => f64::from(position),
        None => to_integer(cx, position)?,
    };
    let repr = &cx.heap.string(string).repr;
    if position < 0.0 || position >= repr.len() as f64 {
        return Ok(Value::number(f64::NAN));
    }
    Ok(Value::int32(i32::from(repr.unit(position as usize))))
}

/// The first index from `from` at which `needle` occurs in `haystack`.
pub(crate) fn find(haystack: &[u16], needle: &[u16], from: usize) -> Option<usize> {
    if needle.is_empty() {
        return (from <= haystack.len()).then_some(from);
    }
    if needle.len() > haystack.len() {
        return None;
    }
    (from..=haystack.len() - needle.len()).find(|&at| haystack[at..at + needle.len()] == *needle)
}

pub(super) fn index_of(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let string = this_string(cx, &site)?;
    let search = site.arg(cx, 0);
    let search = to_string_id(cx, search)?;
    let position = site.arg(cx, 1);
    let position = to_integer(cx, position)?;
    let haystack = units(cx, string);
    let from = position.clamp(0.0, haystack.len() as f64) as usize;
    let found = find(&haystack, &units(cx, search), from);
    Ok(Value::int32(found.map_or(-1, |at| at as i32)))
}

pub(super) fn last_index_of(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let string = this_string(cx, &site)?;
    let search = site.arg(cx, 0);
    let search = to_string_id(cx, search)?;
    let position = site.arg(cx, 1);
    let position = ops::to_number(cx, position)?;
    let haystack = units(cx, string);
    let needle = units(cx, search);
    let position = if position.is_nan() {
        f64::INFINITY
    } else {
        position.trunc()
    };
    let start = position.clamp(0.0, haystack.len() as f64) as usize;
    if needle.len() > haystack.len() {
        return Ok(Value::int32(-1));
    }
    let start = start.min(haystack.len() - needle.len());
    let found = (0..=start)
        .rev()
        .find(|&at| haystack[at..at + needle.len()] == *needle);
    Ok(Value::int32(found.map_or(-1, |at| at as i32)))
}

pub(super) fn slice(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let string = this_string(cx, &site)?;
    let length = length(cx, string);
    let start = site.arg(cx, 0);
    let start = relative(to_integer(cx, start)?, length);
    let end = site.arg(cx, 1);
    let end = if end.is_undefined() {
        length
    } else {
        relative(to_integer(cx, end)?, length)
    };
    substring_value(cx, string, start, end.max(start))
}

pub(super) fn substring(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let string = this_string(cx, &site)?;
    let length = length(cx, string);
    let clamp = |value: f64| value.clamp(0.0, length as f64) as usize;
    let start = site.arg(cx, 0);
    let start = clamp(to_integer(cx, start)?);
    let end = site.arg(cx, 1);
    let end = if end.is_undefined() {
        length
    } else {
        clamp(to_integer(cx, end)?)
    };
    substring_value(cx, string, start.min(end), start.max(end))
}

pub(super) fn substr(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let string = this_string(cx, &site)?;
    let length = length(cx, string);
    let start = site.arg(cx, 0);
    let start = relative(to_integer(cx, start)?, length);
    let count = site.arg(cx, 1);
    let count = if count.is_undefined() {
        length - start
    } else {
        to_integer(cx, count)?.clamp(0.0, (length - start) as f64) as usize
    };
    substring_value(cx, string, start, start + count)
}

pub(super) fn concat(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let string = this_string(cx, &site)?;
    let mut text = units(cx, string);
    for index in 0..site.argc {
        let part = site.arg(cx, index);
        let part = to_string_id(cx, part)?;
        text.extend(cx.heap.string(part).repr.units());
    }
    string_units(cx, &text)
}

fn is_regexp(cx: &Cx<'_>, value: Value) -> bool {
    value
        .as_object()
        .is_some_and(|object| cx.heap.object_kind(object) == ObjectKind::RegExp)
}

/// `split(separator, limit)`.
pub(super) fn split(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let separator = site.arg(cx, 0);
    if is_regexp(cx, separator) {
        return super::regexp::split(cx, site);
    }
    let string = this_string(cx, &site)?;
    let limit = site.arg(cx, 1);
    let limit = if limit.is_undefined() {
        u32::MAX
    } else {
        number::to_uint32(ops::to_number(cx, limit)?)
    };
    let parts: Vec<(usize, usize)> = if separator.is_undefined() {
        vec![(0, length(cx, string))]
    } else {
        let separator = to_string_id(cx, separator)?;
        let haystack = units(cx, string);
        let needle = units(cx, separator);
        split_ranges(&haystack, &needle)
    };
    let count = (parts.len() as u64).min(u64::from(limit)) as u32;
    let prototype = cx.realm.intrinsic(Intrinsic::ArrayPrototype);
    let array = cx
        .heap
        .alloc_array(Some(prototype), count)
        .map_err(|error| error::heap_error(cx, error))?;
    let mark = cx.heap.roots().mark();
    cx.heap.roots().push(Value::object(array));
    let result = (|| {
        for (index, &(start, end)) in parts.iter().take(count as usize).enumerate() {
            let part = substring_value(cx, string, start, end)?;
            cx.heap
                .set_element(array, index as u32, part)
                .map_err(|_| error::oom(cx.realm))?;
        }
        Ok(Value::object(array))
    })();
    cx.heap.roots().truncate(mark);
    result
}

/// The pieces of `haystack` between occurrences of `needle`: every unit
/// for an empty separator, none for an empty string split by one.
fn split_ranges(haystack: &[u16], needle: &[u16]) -> Vec<(usize, usize)> {
    if needle.is_empty() {
        return (0..haystack.len()).map(|at| (at, at + 1)).collect();
    }
    let mut ranges = Vec::new();
    let mut start = 0;
    while let Some(at) = find(haystack, needle, start) {
        ranges.push((start, at));
        start = at + needle.len();
    }
    ranges.push((start, haystack.len()));
    ranges
}

/// A named group's capture by name for `$<name>`: `None` when the
/// expression has no named groups (the pattern is literal), `Some(None)` for
/// an unknown name or a group that did not participate.
pub(crate) type NamedCapture<'a> = &'a dyn Fn(&[u16]) -> Option<Option<Vec<u16>>>;

/// `GetSubstitution`: the replacement text for a match of `matched` at
/// `position` in `string`, with the captures of a regular expression
/// (`None` for one that did not participate) and its named groups.
pub(crate) fn substitution(
    string: &[u16],
    position: usize,
    matched: &[u16],
    captures: &[Option<Vec<u16>>],
    named: NamedCapture<'_>,
    replacement: &[u16],
) -> Vec<u16> {
    let dollar = u16::from(b'$');
    let mut out = Vec::with_capacity(replacement.len());
    let mut at = 0;
    while at < replacement.len() {
        let unit = replacement[at];
        if unit != dollar || at + 1 == replacement.len() {
            out.push(unit);
            at += 1;
            continue;
        }
        let next = replacement[at + 1];
        let digit = |unit: u16| (u16::from(b'0')..=u16::from(b'9')).contains(&unit);
        match next {
            _ if next == dollar => {
                out.push(dollar);
                at += 2;
            }
            _ if next == u16::from(b'&') => {
                out.extend_from_slice(matched);
                at += 2;
            }
            _ if next == u16::from(b'`') => {
                out.extend_from_slice(&string[..position]);
                at += 2;
            }
            _ if next == u16::from(b'\'') => {
                let end = (position + matched.len()).min(string.len());
                out.extend_from_slice(&string[end..]);
                at += 2;
            }
            _ if digit(next) => {
                let first = usize::from(next - u16::from(b'0'));
                let two = replacement
                    .get(at + 2)
                    .filter(|&&unit| digit(unit))
                    .map(|&unit| first * 10 + usize::from(unit - u16::from(b'0')));
                let (index, width) = match two {
                    Some(index) if index >= 1 && index <= captures.len() => (index, 3),
                    _ => (first, 2),
                };
                if index >= 1 && index <= captures.len() {
                    if let Some(capture) = &captures[index - 1] {
                        out.extend_from_slice(capture);
                    }
                    at += width;
                } else {
                    out.push(dollar);
                    at += 1;
                }
            }
            _ if next == u16::from(b'<') => {
                let close = replacement[at + 2..]
                    .iter()
                    .position(|&unit| unit == u16::from(b'>'));
                match close.and_then(|close| {
                    named(&replacement[at + 2..at + 2 + close]).map(|value| (close, value))
                }) {
                    Some((close, value)) => {
                        if let Some(value) = value {
                            out.extend_from_slice(&value);
                        }
                        at += close + 3;
                    }
                    None => {
                        out.push(dollar);
                        at += 1;
                    }
                }
            }
            _ => {
                out.push(dollar);
                at += 1;
            }
        }
    }
    out
}

/// `replace(pattern, replacement)`: the first occurrence of a string
/// pattern; a regular expression runs its own algorithm. A function
/// replacement calls back into JavaScript, which is phase 4's.
pub(super) fn replace(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let pattern = site.arg(cx, 0);
    let replacement = site.arg(cx, 1);
    if ops::is_callable(cx.heap, replacement) {
        return Err(unsupported(cx, "replace with a function"));
    }
    if is_regexp(cx, pattern) {
        return super::regexp::replace(cx, site);
    }
    let string = this_string(cx, &site)?;
    let search = to_string_id(cx, pattern)?;
    let replacement = to_string_id(cx, replacement)?;
    let haystack = units(cx, string);
    let needle = units(cx, search);
    let Some(position) = find(&haystack, &needle, 0) else {
        return Ok(Value::string(string));
    };
    let replacement = units(cx, replacement);
    let mut text = haystack[..position].to_vec();
    text.extend(substitution(
        &haystack,
        position,
        &needle,
        &[],
        &|_| None,
        &replacement,
    ));
    text.extend_from_slice(&haystack[position + needle.len()..]);
    string_units(cx, &text)
}

/// `match(regexp)`: a string argument is made a regular expression first.
pub(super) fn match_(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    super::regexp::match_(cx, site)
}

/// Case mapping: an ASCII pass, and the full Unicode mapping for the rest.
fn map_case(cx: &mut Cx<'_>, site: CallSite, upper: bool) -> Result<Value, Throw> {
    let string = this_string(cx, &site)?;
    let repr = &cx.heap.string(string).repr;
    if let StringRepr::OneByte(bytes) = repr
        && bytes.is_ascii()
    {
        let changes = bytes.iter().any(|byte| {
            if upper {
                byte.is_ascii_lowercase()
            } else {
                byte.is_ascii_uppercase()
            }
        });
        if !changes {
            return Ok(Value::string(string));
        }
        let mapped: Box<[u8]> = bytes
            .iter()
            .map(|byte| {
                if upper {
                    byte.to_ascii_uppercase()
                } else {
                    byte.to_ascii_lowercase()
                }
            })
            .collect();
        return string_repr(cx, StringRepr::OneByte(mapped));
    }
    let text = units(cx, string);
    let mapped = if upper {
        super::case::to_upper(&text)
    } else {
        super::case::to_lower(&text)
    };
    match mapped {
        Some(mapped) => string_units(cx, &mapped),
        None => Err(error::range_error(cx, "invalid string length")),
    }
}

pub(super) fn to_lower_case(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    map_case(cx, site, false)
}

pub(super) fn to_upper_case(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    map_case(cx, site, true)
}

/// `String.prototype.toString` and `valueOf`: the receiver must be a string.
pub(super) fn value_of(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let this = site.this(cx);
    match this.kind() {
        Kind::String => Ok(this),
        _ => Err(error::type_error(cx, "not a string")),
    }
}
