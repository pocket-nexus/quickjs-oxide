//! `RegExp`: the constructor, `exec`, `test`, `toString` and the accessors
//! `source`, `flags`, `global`, `ignoreCase` and `multiline` on
//! `%RegExp.prototype%`; and the regular-expression paths of
//! `String.prototype`'s `split`, `replace` and `match`.

use oxide_regexp::{ExecError, RegExpFlags, RegExpMatch};

use super::string::{substitution, substring_value};
use super::{
    Native, install_constructor, methods, native_function, string_units, this_string, to_integer,
    to_string_id,
};
use crate::heap::HeapError;
use crate::heap::object::ObjectKind;
use crate::heap::string::StringRepr;
use crate::runtime::{Intrinsic, Names};
use crate::shape::PropertyFlags;
use crate::value::{ObjectId, StringId, Value};
use crate::vm::error::{self, ErrorKind};
use crate::vm::native::CallSite;
use crate::vm::property;
use crate::vm::{Cx, Throw};

pub(super) fn install(cx: &mut Cx<'_>) -> Result<(), HeapError> {
    let prototype = cx.realm.intrinsic(Intrinsic::RegExpPrototype);
    install_constructor(cx, "RegExp", Native::RegExpConstructor, prototype)?;
    methods(
        cx,
        prototype,
        &[
            ("exec", Native::RegExpExec),
            ("test", Native::RegExpTest),
            ("toString", Native::RegExpToString),
        ],
    )?;
    for (name, native) in [
        ("source", Native::RegExpSource),
        ("flags", Native::RegExpFlags),
        ("global", Native::RegExpGlobal),
        ("ignoreCase", Native::RegExpIgnoreCase),
        ("multiline", Native::RegExpMultiline),
    ] {
        let getter = native_function(cx, native)?;
        let key = cx.heap.property_key(name)?;
        cx.heap.add_accessor(
            prototype,
            key,
            PropertyFlags::accessor(false, true),
            Value::object(getter),
            Value::UNDEFINED,
        )?;
    }
    Ok(())
}

/// The receiver's program index, or `None` for `%RegExp.prototype%`
/// itself; a TypeError for anything else.
fn this_record(cx: &mut Cx<'_>, site: &CallSite) -> Result<Option<u32>, Throw> {
    let this = site.this(cx);
    if let Some(object) = this.as_object() {
        if cx.heap.object_kind(object) == ObjectKind::RegExp {
            return Ok(Some(cx.heap.header(object).payload));
        }
        if object == cx.realm.intrinsic(Intrinsic::RegExpPrototype) {
            return Ok(None);
        }
    }
    Err(error::type_error(cx, "not a RegExp"))
}

/// `EscapeRegExpPattern`: `/` outside a class and line terminators escaped,
/// `(?:)` for the empty pattern.
fn escaped_source(cx: &Cx<'_>, record: Option<u32>) -> Vec<u16> {
    let Some(record) = record.map(|index| cx.regexps.get(index)) else {
        return "(?:)".encode_utf16().collect();
    };
    let source: Vec<u16> = cx.heap.string(record.source).repr.units().collect();
    if source.is_empty() {
        return "(?:)".encode_utf16().collect();
    }
    let mut out = Vec::with_capacity(source.len());
    let (mut class, mut escaped) = (false, false);
    for &unit in &source {
        if escaped {
            escaped = false;
            out.push(unit);
            continue;
        }
        match unit {
            0x5C => escaped = true,
            0x5B => class = true,
            0x5D => class = false,
            0x2F if !class => {
                out.extend([0x5C, 0x2F]);
                continue;
            }
            0x0A => {
                out.extend("\\n".encode_utf16());
                continue;
            }
            0x0D => {
                out.extend("\\r".encode_utf16());
                continue;
            }
            0x2028 => {
                out.extend("\\u2028".encode_utf16());
                continue;
            }
            0x2029 => {
                out.extend("\\u2029".encode_utf16());
                continue;
            }
            _ => {}
        }
        out.push(unit);
    }
    out
}

/// The flags in canonical order (`dgimsuvy`), as the program parsed them.
fn flag_units(cx: &Cx<'_>, record: Option<u32>) -> Vec<u16> {
    record.map_or_else(Vec::new, |index| {
        let flags = cx.regexps.program(index).flags().canonical_string();
        flags.encode_utf16().collect()
    })
}

pub(super) fn source(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let record = this_record(cx, &site)?;
    let text = escaped_source(cx, record);
    string_units(cx, &text)
}

pub(super) fn flags(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let record = this_record(cx, &site)?;
    let text = flag_units(cx, record);
    string_units(cx, &text)
}

fn has_flag(cx: &mut Cx<'_>, site: CallSite, flag: u8) -> Result<Value, Throw> {
    match this_record(cx, &site)? {
        Some(record) => {
            let set = flag_units(cx, Some(record)).contains(&u16::from(flag));
            Ok(Value::bool(set))
        }
        None => Ok(Value::UNDEFINED),
    }
}

pub(super) fn global(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    has_flag(cx, site, b'g')
}

pub(super) fn ignore_case(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    has_flag(cx, site, b'i')
}

pub(super) fn multiline(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    has_flag(cx, site, b'm')
}

/// `RegExp.prototype.toString`: `/source/flags`.
pub(super) fn to_string(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let record = this_record(cx, &site)?;
    let mut text = vec![u16::from(b'/')];
    text.extend(escaped_source(cx, record));
    text.push(u16::from(b'/'));
    text.extend(flag_units(cx, record));
    string_units(cx, &text)
}

/// `RegExp(pattern, flags)` and `new RegExp(…)`. Called on a regular
/// expression with no flags, it returns the argument itself.
pub(super) fn constructor(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let pattern = site.arg(cx, 0);
    let flags = site.arg(cx, 1);
    let (source, flags) = if let Some(object) = regexp_object(cx, pattern) {
        if !site.construct && flags.is_undefined() {
            return Ok(pattern);
        }
        let record = cx.regexps.get(cx.heap.header(object).payload);
        let flags = if flags.is_undefined() {
            record.flags
        } else {
            atom(cx, flags)?
        };
        (record.source, flags)
    } else {
        let source = if pattern.is_undefined() {
            cx.realm.names.empty
        } else {
            atom(cx, pattern)?
        };
        let flags = if flags.is_undefined() {
            cx.realm.names.empty
        } else {
            atom(cx, flags)?
        };
        (source, flags)
    };
    crate::vm::regexp::regexp_object(cx, source, flags)
}

/// `ToString(value)` as an atom: the program store is keyed by atoms.
fn atom(cx: &mut Cx<'_>, value: Value) -> Result<StringId, Throw> {
    let string = to_string_id(cx, value)?;
    cx.heap
        .intern_string(string)
        .map_err(|error| error::heap_error(cx, error))
}

pub(crate) fn regexp_object(cx: &Cx<'_>, value: Value) -> Option<ObjectId> {
    value
        .as_object()
        .filter(|&object| cx.heap.object_kind(object) == ObjectKind::RegExp)
}

fn this_regexp(cx: &mut Cx<'_>, site: &CallSite) -> Result<ObjectId, Throw> {
    let this = site.this(cx);
    regexp_object(cx, this).ok_or_else(|| error::type_error(cx, "not a RegExp"))
}

fn flags_of(cx: &Cx<'_>, object: ObjectId) -> RegExpFlags {
    cx.regexps.program(cx.heap.header(object).payload).flags()
}

/// `ToLength(regexp.lastIndex)`.
fn last_index(cx: &mut Cx<'_>, object: ObjectId) -> Result<usize, Throw> {
    let names = cx.realm.names;
    let value = property::get_value(cx, object, Names::key(names.last_index))?;
    if let Some(index) = value.as_int32() {
        return Ok(index.max(0) as usize);
    }
    let index = to_integer(cx, value)?;
    Ok(index.clamp(0.0, 9_007_199_254_740_991.0) as usize)
}

fn set_last_index(cx: &mut Cx<'_>, object: ObjectId, index: usize) -> Result<(), Throw> {
    let names = cx.realm.names;
    let value = property::length_value(index.min(u32::MAX as usize) as u32);
    property::set_on(cx, object, Names::key(names.last_index), value, true)
}

/// Runs a program on a string's own code units (one-byte strings are not
/// widened).
fn run(
    cx: &mut Cx<'_>,
    program: u32,
    string: StringId,
    start: usize,
) -> Result<Option<RegExpMatch>, Throw> {
    let compiled = cx.regexps.program(program);
    let result = match &cx.heap.string(string).repr {
        StringRepr::OneByte(bytes) => {
            oxide_regexp::execute_latin1_with_interrupt(compiled, bytes, start, || false)
        }
        StringRepr::TwoByte(units) => oxide_regexp::execute(compiled, units, start),
    };
    result.map_err(|failure| match failure {
        ExecError::OutOfMemory => error::oom(cx.realm),
        other => error::throw(cx, ErrorKind::Internal, &other.to_string()),
    })
}

/// `RegExpBuiltinExec`: a match from `lastIndex` for a global or sticky
/// expression (which then moves `lastIndex`), from 0 otherwise.
fn builtin_exec(
    cx: &mut Cx<'_>,
    object: ObjectId,
    string: StringId,
) -> Result<Option<RegExpMatch>, Throw> {
    let flags = flags_of(cx, object);
    let moves = flags.contains(RegExpFlags::GLOBAL) || flags.contains(RegExpFlags::STICKY);
    let mut start = last_index(cx, object)?;
    if !moves {
        start = 0;
    }
    let length = cx.heap.string(string).repr.len();
    if start > length {
        if moves {
            set_last_index(cx, object, 0)?;
        }
        return Ok(None);
    }
    let program = cx.heap.header(object).payload;
    match run(cx, program, string, start)? {
        None => {
            if moves {
                set_last_index(cx, object, 0)?;
            }
            Ok(None)
        }
        Some(found) => {
            if moves {
                let end = found.capture(0).map_or(start, |range| range.end);
                set_last_index(cx, object, end)?;
            }
            Ok(Some(found))
        }
    }
}

/// The match array: the matched text, the captures (`undefined` for one
/// that did not participate), `index`, `input` and `groups`.
fn match_array(
    cx: &mut Cx<'_>,
    object: ObjectId,
    string: StringId,
    found: &RegExpMatch,
) -> Result<Value, Throw> {
    let captures = found.captures().to_vec();
    let prototype = cx.realm.intrinsic(Intrinsic::ArrayPrototype);
    let array = cx
        .heap
        .alloc_array(Some(prototype), captures.len() as u32)
        .map_err(|error| error::heap_error(cx, error))?;
    cx.heap.roots().push(Value::object(array));
    let mut values = Vec::with_capacity(captures.len());
    for capture in &captures {
        let value = match capture {
            Some(range) => substring_value(cx, string, range.start, range.end)?,
            None => Value::UNDEFINED,
        };
        cx.heap.roots().push(value);
        values.push(value);
    }
    let elements = cx.heap.header(array).elements();
    for (index, &value) in values.iter().enumerate() {
        cx.heap.values.set(elements, index as u32, value);
    }
    cx.heap.set_array_length(array, captures.len() as u32);
    let program = cx.heap.header(object).payload;
    let groups = match cx.regexps.program(program).group_names() {
        Some(names) => {
            let names: Vec<Option<Vec<u16>>> = names
                .iter()
                .map(|name| name.as_ref().map(|name| name.to_vec()))
                .collect();
            let groups = cx
                .heap
                .alloc_object(ObjectKind::Ordinary, None, names.len() as u32)
                .map_err(|error| error::heap_error(cx, error))?;
            cx.heap.roots().push(Value::object(groups));
            for (index, name) in names.iter().enumerate() {
                if let Some(name) = name {
                    let key = cx
                        .heap
                        .property_key_units(name)
                        .map_err(|error| error::heap_error(cx, error))?;
                    property::define(cx, groups, key, values[index + 1])?;
                }
            }
            Value::object(groups)
        }
        None => Value::UNDEFINED,
    };
    let start = captures[0].as_ref().map_or(0, |range| range.start);
    let names = cx.realm.names;
    for (name, value) in [
        (names.index, Value::int32(start as i32)),
        (names.input, Value::string(string)),
        (names.groups, groups),
    ] {
        property::define(cx, array, Names::key(name), value)?;
    }
    Ok(Value::object(array))
}

pub(super) fn exec(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let object = this_regexp(cx, &site)?;
    let string = site.arg(cx, 0);
    let string = to_string_id(cx, string)?;
    match builtin_exec(cx, object, string)? {
        Some(found) => match_array(cx, object, string, &found),
        None => Ok(Value::NULL),
    }
}

pub(super) fn test(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let object = this_regexp(cx, &site)?;
    let string = site.arg(cx, 0);
    let string = to_string_id(cx, string)?;
    Ok(Value::bool(builtin_exec(cx, object, string)?.is_some()))
}

/// `AdvanceStringIndex`: one unit, or a surrogate pair in Unicode mode.
fn advance(cx: &Cx<'_>, string: StringId, index: usize, unicode: bool) -> usize {
    let repr = &cx.heap.string(string).repr;
    if !unicode || index + 1 >= repr.len() {
        return index + 1;
    }
    let lead = repr.unit(index);
    let trail = repr.unit(index + 1);
    if (0xD800..0xDC00).contains(&lead) && (0xDC00..0xE000).contains(&trail) {
        index + 2
    } else {
        index + 1
    }
}

fn is_unicode(flags: RegExpFlags) -> bool {
    flags.contains(RegExpFlags::UNICODE) || flags.contains(RegExpFlags::UNICODE_SETS)
}

/// `String.prototype.match`: a non-regular-expression argument is made one
/// (`new RegExp(ToString(arg))`).
pub(super) fn match_(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let string = this_string(cx, &site)?;
    let argument = site.arg(cx, 0);
    let object = match regexp_object(cx, argument) {
        Some(object) => object,
        None => {
            let source = if argument.is_undefined() {
                cx.realm.names.empty
            } else {
                atom(cx, argument)?
            };
            let empty = cx.realm.names.empty;
            let made = crate::vm::regexp::regexp_object(cx, source, empty)?;
            cx.heap.roots().push(made);
            made.as_object().unwrap()
        }
    };
    let flags = flags_of(cx, object);
    if !flags.contains(RegExpFlags::GLOBAL) {
        return match builtin_exec(cx, object, string)? {
            Some(found) => match_array(cx, object, string, &found),
            None => Ok(Value::NULL),
        };
    }
    set_last_index(cx, object, 0)?;
    let mut matches = Vec::new();
    while let Some(found) = builtin_exec(cx, object, string)? {
        let range = found.capture(0).cloned().unwrap_or(0..0);
        let matched = substring_value(cx, string, range.start, range.end)?;
        cx.heap.roots().push(matched);
        matches.push(matched);
        if range.is_empty() {
            let index = last_index(cx, object)?;
            let next = advance(cx, string, index, is_unicode(flags));
            set_last_index(cx, object, next)?;
        }
    }
    if matches.is_empty() {
        return Ok(Value::NULL);
    }
    let prototype = cx.realm.intrinsic(Intrinsic::ArrayPrototype);
    let array = cx
        .heap
        .alloc_array(Some(prototype), matches.len() as u32)
        .map_err(|error| error::heap_error(cx, error))?;
    let elements = cx.heap.header(array).elements();
    for (index, &value) in matches.iter().enumerate() {
        cx.heap.values.set(elements, index as u32, value);
    }
    cx.heap.set_array_length(array, matches.len() as u32);
    Ok(Value::object(array))
}

/// `String.prototype.replace` with a regular expression and a string
/// replacement: the first match, or every match of a global expression,
/// with `GetSubstitution`'s patterns.
pub(super) fn replace(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let string = this_string(cx, &site)?;
    let object = regexp_object(cx, site.arg(cx, 0)).expect("a RegExp pattern");
    let replacement = site.arg(cx, 1);
    let replacement = to_string_id(cx, replacement)?;
    let flags = flags_of(cx, object);
    let global = flags.contains(RegExpFlags::GLOBAL);
    if global {
        set_last_index(cx, object, 0)?;
    }
    let mut found_all = Vec::new();
    while let Some(found) = builtin_exec(cx, object, string)? {
        let empty = found.capture(0).is_none_or(|range| range.is_empty());
        found_all.push(found);
        if !global {
            break;
        }
        if empty {
            let index = last_index(cx, object)?;
            let next = advance(cx, string, index, is_unicode(flags));
            set_last_index(cx, object, next)?;
        }
    }
    if found_all.is_empty() {
        return Ok(Value::string(string));
    }
    let text: Vec<u16> = cx.heap.string(string).repr.units().collect();
    let replacement: Vec<u16> = cx.heap.string(replacement).repr.units().collect();
    let program = cx.heap.header(object).payload;
    let names: Option<Vec<Option<Vec<u16>>>> =
        cx.regexps.program(program).group_names().map(|names| {
            names
                .iter()
                .map(|name| name.as_ref().map(|name| name.to_vec()))
                .collect()
        });
    let mut out = Vec::with_capacity(text.len());
    let mut next = 0;
    for found in &found_all {
        let range = found.capture(0).cloned().unwrap_or(0..0);
        let captures: Vec<Option<Vec<u16>>> = found.captures()[1..]
            .iter()
            .map(|capture| capture.as_ref().map(|range| text[range.clone()].to_vec()))
            .collect();
        let named = |name: &[u16]| -> Option<Option<Vec<u16>>> {
            let names = names.as_ref()?;
            let index = names
                .iter()
                .position(|group| group.as_deref() == Some(name));
            Some(index.and_then(|index| captures[index].clone()))
        };
        let substituted = substitution(
            &text,
            range.start,
            &text[range.clone()],
            &captures,
            &named,
            &replacement,
        );
        if range.start >= next {
            out.extend_from_slice(&text[next..range.start]);
            out.extend(substituted);
            next = range.end;
        }
    }
    out.extend_from_slice(&text[next.min(text.len())..]);
    string_units(cx, &out)
}

/// `String.prototype.split` with a regular expression. The specification
/// runs a sticky copy of the expression at each position; searching forward
/// from a position finds the same next match, so the expression runs as it
/// is and the user's `lastIndex` is untouched.
pub(super) fn split(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let string = this_string(cx, &site)?;
    let object = regexp_object(cx, site.arg(cx, 0)).expect("a RegExp separator");
    let limit = site.arg(cx, 1);
    let limit = if limit.is_undefined() {
        u32::MAX
    } else {
        crate::vm::number::to_uint32(crate::vm::ops::to_number(cx, limit)?)
    } as usize;
    let program = cx.heap.header(object).payload;
    let flags = flags_of(cx, object);
    let sticky = flags.contains(RegExpFlags::STICKY);
    let unicode = is_unicode(flags);
    let size = cx.heap.string(string).repr.len();
    let mut parts: Vec<Value> = Vec::new();
    let push = |cx: &mut Cx<'_>, parts: &mut Vec<Value>, value: Value| {
        cx.heap.roots().push(value);
        parts.push(value);
    };
    if limit != 0 {
        if size == 0 {
            let matched = run(cx, program, string, 0)?
                .is_some_and(|found| found.capture(0).is_some_and(|range| range.start == 0));
            if !matched {
                push(cx, &mut parts, Value::string(string));
            }
        } else {
            let (mut p, mut q) = (0, 0);
            'split: while q < size {
                let Some(found) = run(cx, program, string, q)? else {
                    if sticky {
                        q = advance(cx, string, q, unicode);
                        continue;
                    }
                    break;
                };
                let range = found.capture(0).cloned().unwrap_or(q..q);
                if range.start >= size {
                    break;
                }
                let end = range.end.min(size);
                if end == p {
                    q = advance(cx, string, range.start.max(q), unicode);
                    continue;
                }
                let piece = substring_value(cx, string, p, range.start)?;
                push(cx, &mut parts, piece);
                if parts.len() == limit {
                    break 'split;
                }
                p = end;
                for capture in &found.captures()[1..] {
                    let value = match capture {
                        Some(range) => substring_value(cx, string, range.start, range.end)?,
                        None => Value::UNDEFINED,
                    };
                    push(cx, &mut parts, value);
                    if parts.len() == limit {
                        break 'split;
                    }
                }
                q = p;
            }
            if parts.len() < limit {
                let rest = substring_value(cx, string, p, size)?;
                push(cx, &mut parts, rest);
            }
        }
    }
    let prototype = cx.realm.intrinsic(Intrinsic::ArrayPrototype);
    let array = cx
        .heap
        .alloc_array(Some(prototype), parts.len() as u32)
        .map_err(|error| error::heap_error(cx, error))?;
    let elements = cx.heap.header(array).elements();
    for (index, &value) in parts.iter().enumerate() {
        cx.heap.values.set(elements, index as u32, value);
    }
    cx.heap.set_array_length(array, parts.len() as u32);
    Ok(Value::object(array))
}
