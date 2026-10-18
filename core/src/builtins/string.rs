//! `String` (phase 4, section 6.1 row 3): the constructor, `fromCharCode`,
//! `fromCodePoint`, `raw`, and every `String.prototype` method including
//! the Annex B HTML methods, `substr` and the `trimLeft`/`trimRight`
//! aliases. `match`, `matchAll`, `replace`, `replaceAll`, `search` and
//! `split` delegate to their argument's `Symbol` method first. `localeCompare`
//! is QuickJS's code-unit comparison; the locale case mappings are the
//! plain ones.

use super::{
    Native, install_constructor, link_constructor, methods, string_repr, string_units, this_string,
    to_string_id,
};
use crate::heap::HeapError;
use crate::heap::object::ObjectKind;
use crate::heap::string::StringRepr;
use crate::runtime::{Intrinsic, Names};
use crate::shape::PropertyFlags;
use crate::value::{Kind, ObjectId, StringId, Value};
use crate::vm::native::CallSite;
use crate::vm::object as internal;
use crate::vm::{Cx, Throw, error, number, ops};

/// QuickJS's string length limit.
pub(crate) const MAX_STRING: usize = (1 << 30) - 1;

pub(super) fn install(cx: &mut Cx<'_>) -> Result<(), HeapError> {
    let prototype = cx.realm.intrinsic(Intrinsic::StringPrototype);
    // QuickJS's `js_string_funcs` and `js_string_proto_funcs`, in order;
    // `normalize` and `localeCompare` follow the constructor link
    // (`JS_AddIntrinsicStringNormalize`).
    let string = install_constructor(cx, Native::StringConstructor)?;
    cx.set_intrinsic(Intrinsic::String, string);
    methods(
        cx,
        string,
        &[
            Native::StringFromCharCode,
            Native::StringFromCodePoint,
            Native::StringRaw,
        ],
    )?;
    let names = cx.realm.names;
    cx.heap.add_property(
        prototype,
        Names::key(names.length),
        PropertyFlags::data(false, false, true),
        Value::int32(0),
    )?;
    methods(
        cx,
        prototype,
        &[
            Native::StringAt,
            Native::StringCharCodeAt,
            Native::StringCharAt,
            Native::StringConcat,
            Native::StringCodePointAt,
            Native::StringIsWellFormed,
            Native::StringToWellFormed,
            Native::StringIndexOf,
            Native::StringLastIndexOf,
            Native::StringIncludes,
            Native::StringEndsWith,
            Native::StringStartsWith,
            Native::StringMatch,
            Native::StringMatchAll,
            Native::StringSearch,
            Native::StringSplit,
            Native::StringSubstring,
            Native::StringSubstr,
            Native::StringSlice,
            Native::StringRepeat,
            Native::StringReplace,
            Native::StringReplaceAll,
            Native::StringPadEnd,
            Native::StringPadStart,
            Native::StringTrim,
            Native::StringTrimEnd,
        ],
    )?;
    // `trimRight` and `trimLeft` are the same function objects as `trimEnd`
    // and `trimStart` (`JS_ALIAS_DEF`).
    alias(cx, prototype, "trimRight", "trimEnd")?;
    methods(cx, prototype, &[Native::StringTrimStart])?;
    alias(cx, prototype, "trimLeft", "trimStart")?;
    methods(
        cx,
        prototype,
        &[
            Native::StringToString,
            Native::StringValueOf,
            Native::StringToLowerCase,
            Native::StringToUpperCase,
            Native::StringToLocaleLowerCase,
            Native::StringToLocaleUpperCase,
            Native::StringAnchor,
            Native::StringBig,
            Native::StringBlink,
            Native::StringBold,
            Native::StringFixed,
            Native::StringFontcolor,
            Native::StringFontsize,
            Native::StringItalics,
            Native::StringLink,
            Native::StringSmall,
            Native::StringStrike,
            Native::StringSub,
            Native::StringSup,
        ],
    )?;
    let iterator = cx.realm.symbols.iterator;
    super::symbol_method(
        cx,
        prototype,
        iterator,
        Native::StringIterator,
        PropertyFlags::data(true, false, true),
    )?;
    link_constructor(cx, string, prototype)?;
    methods(
        cx,
        prototype,
        &[Native::StringNormalize, Native::StringLocaleCompare],
    )
}

/// Defines `alias` on `object` as the function already at `name`.
fn alias(cx: &mut Cx<'_>, object: ObjectId, alias: &str, name: &str) -> Result<(), HeapError> {
    let key = cx.heap.property_key(name)?;
    let (slot, _) = cx.heap.lookup_own(object, key).expect("the method");
    let function = cx.heap.get_slot(object, slot);
    super::define(cx, object, alias, function, super::hidden())
}

fn root(cx: &mut Cx<'_>, value: Value) {
    cx.heap.roots().push(value);
}

fn units(cx: &Cx<'_>, string: StringId) -> Vec<u16> {
    cx.heap.string(string).repr().units().collect()
}

fn length(cx: &Cx<'_>, string: StringId) -> usize {
    cx.heap.string_len(string)
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
    let repr = match &cx.heap.string(string).repr() {
        StringRepr::OneByte(bytes) => StringRepr::OneByte(bytes[start..end].into()),
        StringRepr::TwoByte(units) => StringRepr::from_units(&units[start..end]),
    };
    string_repr(cx, repr)
}

/// `ToIntegerOrInfinity(arg)` (with the `Int32` case inline).
fn integer(cx: &mut Cx<'_>, value: Value) -> Result<f64, Throw> {
    internal::to_integer_or_infinity(cx, value)
}

/// `String(value)`: `ToString`, with a symbol's description when called;
/// `new String(value)` wraps the string.
pub(super) fn constructor(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let string = if site.argc == 0 {
        Value::string(cx.realm.names.empty)
    } else {
        let value = site.arg(cx, 0);
        match value.as_symbol() {
            Some(symbol) if !site.construct => {
                return super::symbol::descriptive_string(cx, symbol);
            }
            _ => ops::to_string(cx, value)?,
        }
    };
    if !site.construct {
        return Ok(string);
    }
    root(cx, string);
    let prototype =
        internal::prototype_from_constructor(cx, site.new_target, Intrinsic::StringPrototype)?;
    internal::wrap(cx, ObjectKind::StringWrapper, string, prototype).map(Value::object)
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

pub(super) fn from_code_point(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let mut text = Vec::with_capacity(site.argc);
    for index in 0..site.argc {
        let code = site.arg(cx, index);
        let code = ops::to_number(cx, code)?;
        if code.fract() != 0.0 || !(0.0..=1_114_111.0).contains(&code) {
            return Err(error::range_error(cx, "invalid code point"));
        }
        push_code_point(&mut text, code as u32);
    }
    string_units(cx, &text)
}

fn push_code_point(text: &mut Vec<u16>, code: u32) {
    if code < 0x10000 {
        text.push(code as u16);
    } else {
        let adjusted = code - 0x10000;
        text.push(0xD800 | (adjusted >> 10) as u16);
        text.push(0xDC00 | (adjusted & 0x3FF) as u16);
    }
}

/// `String.raw(template, ...substitutions)`.
pub(super) fn raw(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let template = site.arg(cx, 0);
    let cooked = internal::to_object(cx, template)?;
    root(cx, Value::object(cooked));
    let raw = internal::get_property(cx, cooked, Names::key(cx.realm.names.raw))?;
    let raw = internal::to_object(cx, raw)?;
    root(cx, Value::object(raw));
    let literal_count = internal::length_of_array_like(cx, raw)?;
    let mut text = Vec::new();
    let mut index = 0u64;
    while index < literal_count {
        let key = internal::number_key(cx, index as f64)?;
        let segment = internal::get_property(cx, raw, key)?;
        let segment = to_string_id(cx, segment)?;
        text.extend(cx.heap.string(segment).repr().units());
        if index + 1 == literal_count {
            break;
        }
        if (index as usize + 1) < site.argc {
            let substitution = site.arg(cx, index as usize + 1);
            let substitution = to_string_id(cx, substitution)?;
            text.extend(cx.heap.string(substitution).repr().units());
        }
        index += 1;
    }
    string_units(cx, &text)
}

// ---- characters ----

pub(super) fn at(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let string = this_string(cx, &site)?;
    let length = length(cx, string) as f64;
    let index = site.arg(cx, 0);
    let relative = integer(cx, index)?;
    let index = if relative >= 0.0 {
        relative
    } else {
        length + relative
    };
    if index < 0.0 || index >= length {
        return Ok(Value::UNDEFINED);
    }
    let unit = cx.heap.string(string).repr().unit(index as usize);
    string_units(cx, &[unit])
}

pub(super) fn char_at(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let string = this_string(cx, &site)?;
    let position = site.arg(cx, 0);
    let position = integer(cx, position)?;
    let length = length(cx, string);
    if position < 0.0 || position >= length as f64 {
        return Ok(Value::string(cx.realm.names.empty));
    }
    let unit = cx.heap.string(string).repr().unit(position as usize);
    string_units(cx, &[unit])
}

pub(super) fn char_code_at(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let string = this_string(cx, &site)?;
    let position = site.arg(cx, 0);
    let position = match position.as_int32() {
        Some(position) => f64::from(position),
        None => integer(cx, position)?,
    };
    let repr = &cx.heap.string(string).repr();
    if position < 0.0 || position >= repr.len() as f64 {
        return Ok(Value::number(f64::NAN));
    }
    Ok(Value::int32(i32::from(repr.unit(position as usize))))
}

pub(super) fn code_point_at(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let string = this_string(cx, &site)?;
    let position = site.arg(cx, 0);
    let position = integer(cx, position)?;
    let repr = &cx.heap.string(string).repr();
    if position < 0.0 || position >= repr.len() as f64 {
        return Ok(Value::UNDEFINED);
    }
    let at = position as usize;
    let first = repr.unit(at);
    if (0xD800..0xDC00).contains(&first) && at + 1 < repr.len() {
        let second = repr.unit(at + 1);
        if (0xDC00..0xE000).contains(&second) {
            let code = 0x10000 + ((u32::from(first) - 0xD800) << 10) + (u32::from(second) - 0xDC00);
            return Ok(Value::int32(code as i32));
        }
    }
    Ok(Value::int32(i32::from(first)))
}

// ---- searching ----

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
    let position = integer(cx, position)?;
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

/// A search string argument that must not be a regular expression
/// (`includes`, `startsWith`, `endsWith`).
fn plain_search(cx: &mut Cx<'_>, value: Value) -> Result<StringId, Throw> {
    if let Some(object) = value.as_object() {
        let key = crate::runtime::WellKnown::key(cx.realm.symbols.match_);
        let matcher = internal::get_property(cx, object, key)?;
        let is_regexp = if matcher.is_undefined() {
            cx.heap.object_kind(object) == ObjectKind::RegExp
        } else {
            ops::to_boolean(cx.heap, matcher)
        };
        if is_regexp {
            return Err(error::type_error(cx, "regexp not supported"));
        }
    }
    to_string_id(cx, value)
}

pub(super) fn includes(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let string = this_string(cx, &site)?;
    let search = site.arg(cx, 0);
    let search = plain_search(cx, search)?;
    let position = site.arg(cx, 1);
    let position = integer(cx, position)?;
    let haystack = units(cx, string);
    let from = position.clamp(0.0, haystack.len() as f64) as usize;
    Ok(Value::bool(
        find(&haystack, &units(cx, search), from).is_some(),
    ))
}

pub(super) fn starts_with(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let string = this_string(cx, &site)?;
    let search = site.arg(cx, 0);
    let search = plain_search(cx, search)?;
    let position = site.arg(cx, 1);
    let position = integer(cx, position)?;
    let haystack = units(cx, string);
    let needle = units(cx, search);
    let start = position.clamp(0.0, haystack.len() as f64) as usize;
    Ok(Value::bool(
        start + needle.len() <= haystack.len()
            && haystack[start..start + needle.len()] == needle[..],
    ))
}

pub(super) fn ends_with(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let string = this_string(cx, &site)?;
    let search = site.arg(cx, 0);
    let search = plain_search(cx, search)?;
    let haystack = units(cx, string);
    let end_position = site.arg(cx, 1);
    let end = if end_position.is_undefined() {
        haystack.len()
    } else {
        integer(cx, end_position)?.clamp(0.0, haystack.len() as f64) as usize
    };
    let needle = units(cx, search);
    Ok(Value::bool(
        needle.len() <= end && haystack[end - needle.len()..end] == needle[..],
    ))
}

// ---- extraction ----

pub(super) fn slice(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let string = this_string(cx, &site)?;
    let length = length(cx, string) as f64;
    let relative = |value: f64| {
        if value < 0.0 {
            (length + value).max(0.0)
        } else {
            value.min(length)
        }
    };
    let start = site.arg(cx, 0);
    let start = relative(integer(cx, start)?) as usize;
    let end = site.arg(cx, 1);
    let end = if end.is_undefined() {
        length as usize
    } else {
        relative(integer(cx, end)?) as usize
    };
    substring_value(cx, string, start, end.max(start))
}

pub(super) fn substring(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let string = this_string(cx, &site)?;
    let length = length(cx, string);
    let clamp = |value: f64| value.clamp(0.0, length as f64) as usize;
    let start = site.arg(cx, 0);
    let start = clamp(integer(cx, start)?);
    let end = site.arg(cx, 1);
    let end = if end.is_undefined() {
        length
    } else {
        clamp(integer(cx, end)?)
    };
    substring_value(cx, string, start.min(end), start.max(end))
}

pub(super) fn substr(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let string = this_string(cx, &site)?;
    let length = length(cx, string) as f64;
    let start = site.arg(cx, 0);
    let start = integer(cx, start)?;
    let start = if start < 0.0 {
        (length + start).max(0.0)
    } else {
        start.min(length)
    };
    let count = site.arg(cx, 1);
    let count = if count.is_undefined() {
        length
    } else {
        integer(cx, count)?
    };
    let end = (start + count).clamp(start, length);
    substring_value(cx, string, start as usize, end as usize)
}

pub(super) fn concat(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let string = this_string(cx, &site)?;
    let mut text = units(cx, string);
    for index in 0..site.argc {
        let part = site.arg(cx, index);
        let part = to_string_id(cx, part)?;
        text.extend(cx.heap.string(part).repr().units());
        if text.len() > MAX_STRING {
            return Err(error::string_too_long(cx));
        }
    }
    string_units(cx, &text)
}

pub(super) fn repeat(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let string = this_string(cx, &site)?;
    let count = site.arg(cx, 0);
    let count = integer(cx, count)?;
    // QuickJS's `JS_ToInt64Sat`, then an Int32 count.
    if !(0.0..=2_147_483_647.0).contains(&count) {
        return Err(error::range_error(cx, "invalid repeat count"));
    }
    let text = units(cx, string);
    if text.is_empty() || count == 0.0 {
        return Ok(Value::string(cx.realm.names.empty));
    }
    if text.len() as f64 * count > MAX_STRING as f64 {
        return Err(error::range_error(cx, "invalid string length"));
    }
    let repr = &cx.heap.string(string).repr();
    let repr = match repr {
        StringRepr::OneByte(bytes) => StringRepr::OneByte(bytes.repeat(count as usize).into()),
        StringRepr::TwoByte(units) => StringRepr::from_units(&units.repeat(count as usize)),
    };
    string_repr(cx, repr)
}

fn pad(cx: &mut Cx<'_>, site: CallSite, at_start: bool) -> Result<Value, Throw> {
    let string = this_string(cx, &site)?;
    let max = site.arg(cx, 0);
    let max = internal::to_length(cx, max)?;
    let length = length(cx, string) as u64;
    if max <= length {
        return Ok(Value::string(string));
    }
    let filler = site.arg(cx, 1);
    let filler: Vec<u16> = if filler.is_undefined() {
        vec![u16::from(b' ')]
    } else {
        let filler = to_string_id(cx, filler)?;
        units(cx, filler)
    };
    if filler.is_empty() {
        return Ok(Value::string(string));
    }
    if max as usize > MAX_STRING {
        return Err(error::range_error(cx, "invalid string length"));
    }
    let fill_length = (max - length) as usize;
    let mut fill: Vec<u16> = filler.iter().copied().cycle().take(fill_length).collect();
    let text = units(cx, string);
    let out = if at_start {
        fill.extend(text);
        fill
    } else {
        let mut out = text;
        out.extend(fill);
        out
    };
    string_units(cx, &out)
}

pub(super) fn pad_start(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    pad(cx, site, true)
}

pub(super) fn pad_end(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    pad(cx, site, false)
}

/// The white space and line terminators `trim` removes.
pub(crate) fn is_trimmable(unit: u16) -> bool {
    matches!(
        unit,
        0x09 | 0x0A | 0x0B | 0x0C | 0x0D | 0x20 | 0xA0 | 0x1680 | 0x2000
            ..=0x200A | 0x2028 | 0x2029 | 0x202F | 0x205F | 0x3000 | 0xFEFF
    )
}

fn trim_impl(cx: &mut Cx<'_>, site: CallSite, start: bool, end: bool) -> Result<Value, Throw> {
    let string = this_string(cx, &site)?;
    let text = units(cx, string);
    let mut from = 0;
    let mut to = text.len();
    if start {
        while from < to && is_trimmable(text[from]) {
            from += 1;
        }
    }
    if end {
        while to > from && is_trimmable(text[to - 1]) {
            to -= 1;
        }
    }
    substring_value(cx, string, from, to)
}

pub(super) fn trim(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    trim_impl(cx, site, true, true)
}

pub(super) fn trim_start(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    trim_impl(cx, site, true, false)
}

pub(super) fn trim_end(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    trim_impl(cx, site, false, true)
}

pub(super) fn is_well_formed(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let string = this_string(cx, &site)?;
    let text = units(cx, string);
    Ok(Value::bool(well_formed(&text).is_none()))
}

/// The index of the first lone surrogate, if any.
fn well_formed(text: &[u16]) -> Option<usize> {
    let mut at = 0;
    while at < text.len() {
        let unit = text[at];
        if (0xD800..0xDC00).contains(&unit) {
            if at + 1 < text.len() && (0xDC00..0xE000).contains(&text[at + 1]) {
                at += 2;
                continue;
            }
            return Some(at);
        }
        if (0xDC00..0xE000).contains(&unit) {
            return Some(at);
        }
        at += 1;
    }
    None
}

pub(super) fn to_well_formed(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let string = this_string(cx, &site)?;
    let mut text = units(cx, string);
    if well_formed(&text).is_none() {
        return Ok(Value::string(string));
    }
    let mut at = 0;
    while at < text.len() {
        let unit = text[at];
        if (0xD800..0xDC00).contains(&unit)
            && at + 1 < text.len()
            && (0xDC00..0xE000).contains(&text[at + 1])
        {
            at += 2;
            continue;
        }
        if (0xD800..0xE000).contains(&unit) {
            text[at] = 0xFFFD;
        }
        at += 1;
    }
    string_units(cx, &text)
}

/// `localeCompare`: QuickJS compares code units (after `normalize`).
pub(super) fn locale_compare(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let string = this_string(cx, &site)?;
    let that = site.arg(cx, 0);
    let that = to_string_id(cx, that)?;
    let normalize = |text: Vec<u16>| match oxide_regexp::unicode::normalize::normalize(
        &text[..],
        oxide_regexp::unicode::normalize::NormalizationForm::Nfc,
    ) {
        Ok(oxide_regexp::unicode::CodeUnitBuf::Latin1(bytes)) => {
            bytes.into_iter().map(u16::from).collect()
        }
        Ok(oxide_regexp::unicode::CodeUnitBuf::Utf16(units)) => units,
        Err(_) => text,
    };
    let (a, b) = (normalize(units(cx, string)), normalize(units(cx, that)));
    Ok(Value::int32(match a.cmp(&b) {
        std::cmp::Ordering::Less => -1,
        std::cmp::Ordering::Equal => 0,
        std::cmp::Ordering::Greater => 1,
    }))
}

pub(super) fn normalize(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    use oxide_regexp::unicode::normalize::NormalizationForm;
    let string = this_string(cx, &site)?;
    let form = site.arg(cx, 0);
    let form = if form.is_undefined() {
        NormalizationForm::Nfc
    } else {
        let form = to_string_id(cx, form)?;
        match cx.heap.string_text(form).as_str() {
            "NFC" => NormalizationForm::Nfc,
            "NFD" => NormalizationForm::Nfd,
            "NFKC" => NormalizationForm::Nfkc,
            "NFKD" => NormalizationForm::Nfkd,
            _ => return Err(error::range_error(cx, "bad normalization form")),
        }
    };
    let text = units(cx, string);
    match oxide_regexp::unicode::normalize::normalize(&text[..], form) {
        Ok(oxide_regexp::unicode::CodeUnitBuf::Latin1(bytes)) => {
            string_repr(cx, StringRepr::OneByte(bytes.into()))
        }
        Ok(oxide_regexp::unicode::CodeUnitBuf::Utf16(units)) => string_units(cx, &units),
        Err(_) => Err(error::range_error(cx, "invalid string length")),
    }
}

// ---- the regular-expression protocol ----

/// `split(separator, limit)`.
pub(super) fn split(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let limit = site.arg(cx, 1);
    let symbol = cx.realm.symbols.split;
    if let Some(result) = super::regexp::delegate(cx, &site, symbol, &[limit])? {
        return Ok(result);
    }
    let string = this_string(cx, &site)?;
    let limit = if limit.is_undefined() {
        u32::MAX
    } else {
        number::to_uint32(ops::to_number(cx, limit)?)
    };
    let separator = site.arg(cx, 0);
    let separator = to_string_id(cx, separator)?;
    let parts: Vec<(usize, usize)> = if limit == 0 {
        Vec::new()
    } else if site.arg(cx, 0).is_undefined() {
        vec![(0, length(cx, string))]
    } else {
        let haystack = units(cx, string);
        let needle = units(cx, separator);
        if haystack.is_empty() {
            if needle.is_empty() {
                Vec::new()
            } else {
                vec![(0, 0)]
            }
        } else {
            split_ranges(&haystack, &needle)
        }
    };
    let count = (parts.len() as u64).min(u64::from(limit)) as usize;
    let mut values = Vec::with_capacity(count);
    for &(start, end) in parts.iter().take(count) {
        let part = substring_value(cx, string, start, end)?;
        root(cx, part);
        values.push(part);
    }
    internal::array_from_list(cx, &values).map(Value::object)
}

/// The pieces of `haystack` between occurrences of `needle`: every unit
/// for an empty separator.
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
/// (`None` for one that did not participate) and its named groups; `None`
/// when it would be longer than a string can be.
pub(crate) fn substitution(
    string: &[u16],
    position: usize,
    matched: &[u16],
    captures: &[Option<Vec<u16>>],
    named: NamedCapture<'_>,
    replacement: &[u16],
) -> Option<Vec<u16>> {
    // The parts are added only while the result fits.
    fn append(out: &mut Vec<u16>, part: &[u16]) -> Option<()> {
        (out.len() + part.len() <= MAX_STRING).then(|| out.extend_from_slice(part))
    }
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
                append(&mut out, matched)?;
                at += 2;
            }
            _ if next == u16::from(b'`') => {
                append(&mut out, &string[..position.min(string.len())])?;
                at += 2;
            }
            _ if next == u16::from(b'\'') => {
                let end = (position + matched.len()).min(string.len());
                append(&mut out, &string[end..])?;
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
                        append(&mut out, capture)?;
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
                            append(&mut out, &value)?;
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
    (out.len() <= MAX_STRING).then_some(out)
}

/// `replace(searchValue, replaceValue)` and `replaceAll`: the argument's
/// `Symbol.replace` first; a string pattern replaces its first (or every)
/// occurrence.
fn replace_impl(cx: &mut Cx<'_>, site: CallSite, all: bool) -> Result<Value, Throw> {
    let pattern = site.arg(cx, 0);
    let replace_value = site.arg(cx, 1);
    if all && let Some(object) = pattern.as_object() {
        // `replaceAll` with a regular expression requires the `g` flag.
        super::regexp::check_global_flag(cx, object)?;
    }
    let symbol = cx.realm.symbols.replace;
    if let Some(result) = super::regexp::delegate(cx, &site, symbol, &[replace_value])? {
        return Ok(result);
    }
    let string = this_string(cx, &site)?;
    let search = to_string_id(cx, pattern)?;
    let functional = ops::is_callable(cx.heap, replace_value);
    let replacement = if functional {
        None
    } else {
        let replacement = to_string_id(cx, replace_value)?;
        Some(units(cx, replacement))
    };
    let haystack = units(cx, string);
    let needle = units(cx, search);
    let mut positions = Vec::new();
    let mut from = 0;
    while let Some(at) = find(&haystack, &needle, from) {
        positions.push(at);
        if !all {
            break;
        }
        from = at + needle.len().max(1);
        if from > haystack.len() {
            break;
        }
    }
    if positions.is_empty() {
        return Ok(Value::string(string));
    }
    let mut out = Vec::with_capacity(haystack.len());
    let mut end_of_last = 0;
    for position in positions {
        out.extend_from_slice(&haystack[end_of_last..position]);
        let replaced = match &replacement {
            Some(replacement) => {
                substitution(&haystack, position, &needle, &[], &|_| None, replacement)
                    .ok_or_else(|| error::string_too_long(cx))?
            }
            None => {
                let value = crate::vm::call(
                    cx,
                    replace_value,
                    Value::UNDEFINED,
                    &[
                        Value::string(search),
                        Value::int32(position as i32),
                        Value::string(string),
                    ],
                )?;
                let value = to_string_id(cx, value)?;
                units(cx, value)
            }
        };
        out.extend(replaced);
        end_of_last = position + needle.len();
    }
    out.extend_from_slice(&haystack[end_of_last.min(haystack.len())..]);
    string_units(cx, &out)
}

pub(super) fn replace(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    replace_impl(cx, site, false)
}

pub(super) fn replace_all(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    replace_impl(cx, site, true)
}

/// `match(regexp)`.
pub(super) fn match_(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let symbol = cx.realm.symbols.match_;
    super::regexp::string_match(cx, &site, symbol, false)
}

/// `matchAll(regexp)`: a regular expression must be global.
pub(super) fn match_all(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let symbol = cx.realm.symbols.match_all;
    super::regexp::string_match(cx, &site, symbol, true)
}

/// `search(regexp)`.
pub(super) fn search(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let symbol = cx.realm.symbols.search;
    super::regexp::string_match(cx, &site, symbol, false)
}

// ---- case ----

/// Case mapping: an ASCII pass, and the full Unicode mapping for the rest.
fn map_case(cx: &mut Cx<'_>, site: CallSite, upper: bool) -> Result<Value, Throw> {
    let string = this_string(cx, &site)?;
    let repr = &cx.heap.string(string).repr();
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
        None => Err(error::string_too_long(cx)),
    }
}

pub(super) fn to_lower_case(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    map_case(cx, site, false)
}

pub(super) fn to_upper_case(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    map_case(cx, site, true)
}

/// `String.prototype.toString` and `valueOf` (`thisStringValue`).
pub(super) fn value_of(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let this = site.this(cx);
    match this.kind() {
        Kind::String => Ok(this),
        Kind::Object
            if cx.heap.object_kind(this.as_object().unwrap()) == ObjectKind::StringWrapper =>
        {
            Ok(cx.heap.internal_slot(this.as_object().unwrap(), 0))
        }
        // `%String.prototype%` is the wrapper of "" (kept ordinary).
        Kind::Object
            if this.as_object() == Some(cx.realm.intrinsic(Intrinsic::StringPrototype)) =>
        {
            Ok(Value::string(cx.realm.names.empty))
        }
        _ => Err(error::type_error(cx, "not a string")),
    }
}

// ---- Annex B HTML methods ----

/// `CreateHTML(string, tag, attribute, value)`.
fn html(
    cx: &mut Cx<'_>,
    site: CallSite,
    tag: &str,
    attribute: Option<&str>,
) -> Result<Value, Throw> {
    let string = this_string(cx, &site)?;
    let mut out: Vec<u16> = format!("<{tag}").encode_utf16().collect();
    if let Some(attribute) = attribute {
        // QuickJS's `JS_ToStringCheckObject`.
        let value = site.arg(cx, 0);
        if value.is_undefined() || value.is_null() {
            return Err(error::type_error(cx, "null or undefined are forbidden"));
        }
        let value = to_string_id(cx, value)?;
        out.extend(format!(" {attribute}=\"").encode_utf16());
        for unit in cx.heap.string(value).repr().units() {
            if unit == u16::from(b'"') {
                out.extend("&quot;".encode_utf16());
            } else {
                out.push(unit);
            }
        }
        out.push(u16::from(b'"'));
    }
    out.push(u16::from(b'>'));
    out.extend(cx.heap.string(string).repr().units());
    out.extend(format!("</{tag}>").encode_utf16());
    string_units(cx, &out)
}

macro_rules! html_methods {
    ($($name:ident => $tag:expr, $attribute:expr;)*) => {
        $(
            pub(super) fn $name(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
                html(cx, site, $tag, $attribute)
            }
        )*
    };
}

html_methods! {
    anchor => "a", Some("name");
    big => "big", None;
    blink => "blink", None;
    bold => "b", None;
    fixed => "tt", None;
    fontcolor => "font", Some("color");
    fontsize => "font", Some("size");
    italics => "i", None;
    link => "a", Some("href");
    small => "small", None;
    strike => "strike", None;
    sub => "sub", None;
    sup => "sup", None;
}
