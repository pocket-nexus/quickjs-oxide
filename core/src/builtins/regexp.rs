//! `RegExp` (phase 4, section 6.1 row 4): the constructor with `IsRegExp`,
//! `RegExp.escape`, `%RegExp.prototype%`'s `exec` (with `groups` and
//! `indices`), `test`, `toString`, `compile`, the flag accessors and the
//! `Symbol.match`, `matchAll`, `replace`, `search` and `split` protocol, and
//! `%RegExpStringIteratorPrototype%`. Programs run on a string's own
//! one-byte or two-byte storage. `Symbol.species` is dropped (phase 0):
//! `matchAll` and `split` construct `%RegExp%`.

use oxide_regexp::{ExecError, RegExpFlags, RegExpMatch};

use super::string::{substitution, substring_value};
use super::{
    Native, accessor, install_constructor, methods, plain_object, string_units, symbol_method,
    this_string, to_string_id, to_string_tag,
};
use crate::heap::HeapError;
use crate::heap::object::{InternalTag, ObjectKind};
use crate::heap::string::StringRepr;
use crate::runtime::{Intrinsic, Names, WellKnown};
use crate::shape::PropertyFlags;
use crate::value::{ObjectId, StringId, Value};
use crate::vm::error::{self, ErrorKind};
use crate::vm::native::CallSite;
use crate::vm::object as internal;
use crate::vm::{Cx, Throw, iter, ops, property};

/// The flag accessors, in `flags`' order.
const FLAG_GETTERS: [(Native, u8); 8] = [
    (Native::RegExpHasIndices, b'd'),
    (Native::RegExpGlobal, b'g'),
    (Native::RegExpIgnoreCase, b'i'),
    (Native::RegExpMultiline, b'm'),
    (Native::RegExpDotAll, b's'),
    (Native::RegExpUnicode, b'u'),
    (Native::RegExpUnicodeSets, b'v'),
    (Native::RegExpSticky, b'y'),
];

pub(super) fn install(cx: &mut Cx<'_>) -> Result<(), HeapError> {
    let prototype = cx.realm.intrinsic(Intrinsic::RegExpPrototype);
    let constructor = install_constructor(cx, Native::RegExpConstructor, prototype)?;
    cx.set_intrinsic(Intrinsic::RegExp, constructor);
    methods(cx, constructor, &[Native::RegExpEscape])?;
    let species = WellKnown::key(cx.realm.symbols.species);
    accessor(
        cx,
        constructor,
        species,
        "[Symbol.species]",
        Some(Native::SpeciesGetter),
        None,
    )?;
    methods(
        cx,
        prototype,
        &[
            Native::RegExpCompile,
            Native::RegExpExec,
            Native::RegExpTest,
            Native::RegExpToString,
        ],
    )?;
    super::getter(cx, prototype, "flags", Native::RegExpFlags)?;
    super::getter(cx, prototype, "source", Native::RegExpSource)?;
    for (native, _) in FLAG_GETTERS {
        super::getter(cx, prototype, native.js_name(), native)?;
    }
    let symbols = cx.realm.symbols;
    let flags = PropertyFlags::data(true, false, true);
    symbol_method(
        cx,
        prototype,
        symbols.match_,
        Native::RegExpSymbolMatch,
        flags,
    )?;
    symbol_method(
        cx,
        prototype,
        symbols.match_all,
        Native::RegExpSymbolMatchAll,
        flags,
    )?;
    symbol_method(
        cx,
        prototype,
        symbols.replace,
        Native::RegExpSymbolReplace,
        flags,
    )?;
    symbol_method(
        cx,
        prototype,
        symbols.search,
        Native::RegExpSymbolSearch,
        flags,
    )?;
    symbol_method(
        cx,
        prototype,
        symbols.split,
        Native::RegExpSymbolSplit,
        flags,
    )?;
    let iterator_prototype = cx.realm.intrinsic(Intrinsic::IteratorPrototype);
    let string_iterator = plain_object(cx, Some(iterator_prototype), 2)?;
    cx.set_intrinsic(Intrinsic::RegExpStringIteratorPrototype, string_iterator);
    methods(cx, string_iterator, &[Native::RegExpStringIteratorNext])?;
    to_string_tag(cx, string_iterator, "RegExp String Iterator")
}

fn root(cx: &mut Cx<'_>, value: Value) {
    cx.heap.roots().push(value);
}

pub(crate) fn regexp_object(cx: &Cx<'_>, value: Value) -> Option<ObjectId> {
    value
        .as_object()
        .filter(|&object| cx.heap.object_kind(object) == ObjectKind::RegExp)
}

/// `ToString(value)` as an atom: the program store is keyed by atoms.
fn atom(cx: &mut Cx<'_>, value: Value) -> Result<StringId, Throw> {
    let string = to_string_id(cx, value)?;
    cx.heap
        .intern_string(string)
        .map_err(|error| error::heap_error(cx, error))
}

fn units(cx: &Cx<'_>, string: StringId) -> Vec<u16> {
    cx.heap.string(string).repr().units().collect()
}

fn flags_of(cx: &Cx<'_>, object: ObjectId) -> RegExpFlags {
    cx.regexps.program(cx.heap.header(object).payload).flags()
}

/// `IsRegExp(value)`.
fn is_regexp(cx: &mut Cx<'_>, value: Value) -> Result<bool, Throw> {
    let Some(object) = value.as_object() else {
        return Ok(false);
    };
    let key = WellKnown::key(cx.realm.symbols.match_);
    let matcher = internal::get_property(cx, object, key)?;
    if !matcher.is_undefined() {
        return Ok(ops::to_boolean(cx.heap, matcher));
    }
    Ok(cx.heap.object_kind(object) == ObjectKind::RegExp)
}

// ---- the constructor ----

/// `RegExp(pattern, flags)` and `new RegExp(…)`.
pub(super) fn constructor(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let (pattern, flags) = (site.arg(cx, 0), site.arg(cx, 1));
    let pattern_is_regexp = is_regexp(cx, pattern)?;
    let new_target = if site.construct {
        site.new_target
    } else {
        let new_target = site.function;
        if pattern_is_regexp && flags.is_undefined() {
            let object = pattern.as_object().unwrap();
            let constructor =
                internal::get_property(cx, object, Names::key(cx.realm.names.constructor))?;
            if constructor == new_target {
                return Ok(pattern);
            }
        }
        new_target
    };
    // `P` and `F`, converted by `RegExpInitialize` after the allocation.
    let (source, flags) = if let Some(object) = regexp_object(cx, pattern) {
        let record = cx.regexps.get(cx.heap.header(object).payload);
        let flags = if flags.is_undefined() {
            Value::string(record.flags)
        } else {
            flags
        };
        (Value::string(record.source), flags)
    } else if pattern_is_regexp {
        let object = pattern.as_object().unwrap();
        let names = cx.realm.names;
        let source = internal::get_property(cx, object, Names::key(names.source))?;
        root(cx, source);
        let flags = if flags.is_undefined() {
            internal::get_property(cx, object, Names::key(names.flags))?
        } else {
            flags
        };
        root(cx, flags);
        (source, flags)
    } else {
        (pattern, flags)
    };
    let prototype =
        internal::prototype_from_constructor(cx, new_target, Intrinsic::RegExpPrototype)?;
    root(cx, Value::object(prototype));
    let source = if source.is_undefined() {
        cx.realm.names.empty
    } else {
        atom(cx, source)?
    };
    let flags = if flags.is_undefined() {
        cx.realm.names.empty
    } else {
        atom(cx, flags)?
    };
    crate::vm::regexp::regexp_object_with(cx, source, flags, prototype)
}

/// `RegExp.prototype.compile(pattern, flags)` (Annex B): re-initializes the
/// receiver.
pub(super) fn compile(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let this = site.this(cx);
    let Some(object) = regexp_object(cx, this) else {
        return Err(error::object_expected(cx, "RegExp"));
    };
    let (pattern, flags) = (site.arg(cx, 0), site.arg(cx, 1));
    let (source, flags) = if let Some(other) = regexp_object(cx, pattern) {
        if !flags.is_undefined() {
            return Err(error::type_error(cx, "flags must be undefined"));
        }
        let record = cx.regexps.get(cx.heap.header(other).payload);
        (record.source, record.flags)
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
    let program = crate::vm::regexp::program(cx, source, flags)?;
    cx.heap.set_payload(object, program);
    set_last_index(cx, object, Value::int32(0))?;
    Ok(this)
}

/// `RegExp.escape(string)`.
pub(super) fn escape(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let Some(string) = site.arg(cx, 0).as_string() else {
        return Err(error::type_error(cx, "not a string"));
    };
    let text = units(cx, string);
    let mut out = Vec::with_capacity(text.len() * 2);
    let hex = |out: &mut Vec<u16>, unit: u16| {
        let text = if unit <= 0xFF {
            format!("\\x{unit:02x}")
        } else {
            format!("\\u{unit:04x}")
        };
        out.extend(text.encode_utf16());
    };
    for (index, &unit) in text.iter().enumerate() {
        let alphanumeric = (unit as u8 as u16 == unit) && (unit as u8).is_ascii_alphanumeric();
        if index == 0 && alphanumeric {
            hex(&mut out, unit);
            continue;
        }
        match unit {
            0x5E | 0x24 | 0x5C | 0x2E | 0x2A | 0x2B | 0x3F | 0x28 | 0x29 | 0x5B | 0x5D | 0x7B
            | 0x7D | 0x7C | 0x2F => {
                out.push(u16::from(b'\\'));
                out.push(unit);
            }
            0x09 => out.extend("\\t".encode_utf16()),
            0x0A => out.extend("\\n".encode_utf16()),
            0x0B => out.extend("\\v".encode_utf16()),
            0x0C => out.extend("\\f".encode_utf16()),
            0x0D => out.extend("\\r".encode_utf16()),
            0x2C | 0x2D | 0x3D | 0x3C | 0x3E | 0x23 | 0x26 | 0x21 | 0x25 | 0x3A | 0x3B | 0x40
            | 0x7E | 0x27 | 0x60 | 0x22 => hex(&mut out, unit),
            0x20
            | 0xA0
            | 0x1680
            | 0x2000..=0x200A
            | 0x2028
            | 0x2029
            | 0x202F
            | 0x205F
            | 0x3000
            | 0xFEFF => hex(&mut out, unit),
            0xD800..=0xDFFF => {
                let paired = if unit < 0xDC00 {
                    text.get(index + 1)
                        .is_some_and(|&next| (0xDC00..0xE000).contains(&next))
                } else {
                    index > 0 && (0xD800..0xDC00).contains(&text[index - 1])
                };
                if paired {
                    out.push(unit);
                } else {
                    hex(&mut out, unit);
                }
            }
            _ => out.push(unit),
        }
    }
    string_units(cx, &out)
}

// ---- accessors ----

/// The receiver's program, `None` for `%RegExp.prototype%` itself; a
/// `TypeError` for anything else that is not a RegExp object.
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
    if this.as_object().is_none() {
        return Err(error::type_error(cx, "not an object"));
    }
    Err(error::object_expected(cx, "RegExp"))
}

/// `EscapeRegExpPattern`: `/` outside a class and line terminators escaped,
/// `(?:)` for the empty pattern.
fn escaped_source(cx: &Cx<'_>, record: Option<u32>) -> Vec<u16> {
    let Some(record) = record.map(|index| cx.regexps.get(index)) else {
        return "(?:)".encode_utf16().collect();
    };
    let source: Vec<u16> = cx.heap.string(record.source).repr().units().collect();
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

pub(super) fn source(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let record = this_record(cx, &site)?;
    let text = escaped_source(cx, record);
    string_units(cx, &text)
}

/// `%RegExp.prototype%`'s eight flag accessors as installed: its shape,
/// each accessor's slot and getter, and the keys.
#[derive(Clone, Copy, Debug)]
pub(crate) struct RegExpFlagCache {
    shape: crate::value::ShapeId,
    slots: [u32; 8],
    getters: [ObjectId; 8],
    keys: [crate::PropertyKey; 8],
    /// `exec`'s slot and the intrinsic `exec`.
    exec_slot: u32,
    exec: ObjectId,
    /// The slots of `@@match`, `@@matchAll`, `@@replace`, `@@search` and
    /// `@@split`.
    symbol_slots: [(crate::SymbolId, u32); 5],
}

/// Records the flag accessors once every builtin is installed.
pub(super) fn record_flag_cache(cx: &mut Cx<'_>) -> Result<(), HeapError> {
    let prototype = cx.realm.intrinsic(Intrinsic::RegExpPrototype);
    let mut slots = [0; 8];
    let mut getters = [prototype; 8];
    let mut keys = [crate::PropertyKey::index(0).unwrap(); 8];
    for (index, (native, _)) in FLAG_GETTERS.iter().enumerate() {
        let key = cx.heap.property_key(native.js_name())?;
        let (slot, _) = cx.heap.lookup_own(prototype, key).expect("a flag accessor");
        slots[index] = slot;
        getters[index] = cx
            .heap
            .get_slot(prototype, slot)
            .as_object()
            .expect("a getter");
        keys[index] = key;
    }
    let (exec_slot, _) = cx
        .heap
        .lookup_own(prototype, Names::key(cx.realm.names.exec))
        .expect("RegExp.prototype.exec");
    let exec = cx
        .heap
        .get_slot(prototype, exec_slot)
        .as_object()
        .expect("exec");
    let symbols = cx.realm.symbols;
    let mut symbol_slots = [(symbols.match_, 0); 5];
    for (index, symbol) in [
        symbols.match_,
        symbols.match_all,
        symbols.replace,
        symbols.search,
        symbols.split,
    ]
    .into_iter()
    .enumerate()
    {
        let (slot, _) = cx
            .heap
            .lookup_own(prototype, WellKnown::key(symbol))
            .expect("a RegExp protocol method");
        symbol_slots[index] = (symbol, slot);
    }
    cx.realm.regexp_flags.set(Some(RegExpFlagCache {
        shape: cx.heap.shape_of(prototype),
        slots,
        getters,
        keys,
        exec_slot,
        exec,
        symbol_slots,
    }));
    Ok(())
}

/// The flags of a `%RegExp%` instance whose flag accessors no one changed
/// (the instance has only `lastIndex`, the prototype its intrinsic getters):
/// reading them through the getters would give the program's flags.
fn pristine_flags(cx: &Cx<'_>, object: ObjectId, cache: &RegExpFlagCache) -> Option<RegExpFlags> {
    let heap = &*cx.heap;
    let prototype = cx.realm.intrinsic(Intrinsic::RegExpPrototype);
    if heap.object_kind(object) != ObjectKind::RegExp
        || Some(heap.shape_of(object))
            != cx.realm.regexp_instance_shape.get().map(|(shape, _)| shape)
        || heap.prototype_of(object) != Some(prototype)
        || heap.shape_of(prototype) != cache.shape
    {
        return None;
    }
    for index in 0..8 {
        if heap.get_slot(prototype, cache.slots[index]) != Value::object(cache.getters[index]) {
            return None;
        }
    }
    Some(cx.regexps.program(heap.header(object).payload).flags())
}

/// `get RegExp.prototype.flags`: each flag accessor read in order.
pub(super) fn flags(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let this = site.this(cx);
    let Some(object) = this.as_object() else {
        return Err(error::type_error(cx, "not an object"));
    };
    let cache = cx.realm.regexp_flags.get().expect("the flag cache");
    let mut text = Vec::new();
    if let Some(flags) = pristine_flags(cx, object, &cache) {
        for (native, letter) in FLAG_GETTERS {
            let flag = match native {
                Native::RegExpHasIndices => RegExpFlags::HAS_INDICES,
                Native::RegExpGlobal => RegExpFlags::GLOBAL,
                Native::RegExpIgnoreCase => RegExpFlags::IGNORE_CASE,
                Native::RegExpMultiline => RegExpFlags::MULTILINE,
                Native::RegExpDotAll => RegExpFlags::DOT_ALL,
                Native::RegExpUnicode => RegExpFlags::UNICODE,
                Native::RegExpUnicodeSets => RegExpFlags::UNICODE_SETS,
                _ => RegExpFlags::STICKY,
            };
            if flags.contains(flag) {
                text.push(u16::from(letter));
            }
        }
        return string_units(cx, &text);
    }
    for (index, (_, letter)) in FLAG_GETTERS.iter().enumerate() {
        let value = internal::get_property(cx, object, cache.keys[index])?;
        if ops::to_boolean(cx.heap, value) {
            text.push(u16::from(*letter));
        }
    }
    string_units(cx, &text)
}

fn has_flag(cx: &mut Cx<'_>, site: CallSite, flag: RegExpFlags) -> Result<Value, Throw> {
    match this_record(cx, &site)? {
        Some(record) => Ok(Value::bool(
            cx.regexps.program(record).flags().contains(flag),
        )),
        None => Ok(Value::UNDEFINED),
    }
}

macro_rules! flag_getters {
    ($($name:ident => $flag:ident;)*) => {
        $(
            pub(super) fn $name(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
                has_flag(cx, site, RegExpFlags::$flag)
            }
        )*
    };
}

flag_getters! {
    has_indices => HAS_INDICES;
    global => GLOBAL;
    ignore_case => IGNORE_CASE;
    multiline => MULTILINE;
    dot_all => DOT_ALL;
    unicode => UNICODE;
    unicode_sets => UNICODE_SETS;
    sticky => STICKY;
}

/// `RegExp.prototype.toString`: `/source/flags` through the properties.
pub(super) fn to_string(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let this = site.this(cx);
    let Some(object) = this.as_object() else {
        return Err(error::type_error(cx, "not an object"));
    };
    let names = cx.realm.names;
    let source = internal::get_property(cx, object, Names::key(names.source))?;
    let source = to_string_id(cx, source)?;
    // The `flags` getter allocates: `source` stays rooted.
    let mark = cx.heap.roots().mark();
    root(cx, Value::string(source));
    let flags = internal::get_property(cx, object, Names::key(names.flags))
        .and_then(|flags| to_string_id(cx, flags));
    cx.heap.roots().truncate(mark);
    let flags = flags?;
    let mut text = vec![u16::from(b'/')];
    text.extend(cx.heap.string(source).repr().units());
    text.push(u16::from(b'/'));
    text.extend(cx.heap.string(flags).repr().units());
    string_units(cx, &text)
}

// ---- exec ----

/// The slot of `lastIndex` when `object` has a fresh instance's shape
/// (its only property, a writable data property).
#[inline]
fn last_index_slot(cx: &Cx<'_>, object: ObjectId) -> Option<u32> {
    let (shape, slot) = cx.realm.regexp_instance_shape.get()?;
    (cx.heap.shape_of(object) == shape).then_some(slot)
}

fn last_index(cx: &mut Cx<'_>, object: ObjectId) -> Result<u64, Throw> {
    let value = match last_index_slot(cx, object) {
        Some(slot) => cx.heap.get_slot(object, slot),
        None => {
            let names = cx.realm.names;
            property::get_value(cx, object, Names::key(names.last_index))?
        }
    };
    if let Some(index) = value.as_int32()
        && index >= 0
    {
        return Ok(index as u64);
    }
    internal::to_length(cx, value)
}

fn set_last_index(cx: &mut Cx<'_>, object: ObjectId, value: Value) -> Result<(), Throw> {
    if let Some(slot) = last_index_slot(cx, object) {
        cx.heap.set_slot(object, slot, value);
        return Ok(());
    }
    let names = cx.realm.names;
    internal::put(cx, object, Names::key(names.last_index), value, true)
}

fn index_value(index: u64) -> Value {
    crate::vm::number::number(index as f64)
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
    let result = match &cx.heap.string(string).repr() {
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

/// `RegExpBuiltinExec` without building the result: a match from
/// `lastIndex` for a global or sticky expression (which then moves
/// `lastIndex`), from 0 otherwise.
fn builtin_match(
    cx: &mut Cx<'_>,
    object: ObjectId,
    string: StringId,
) -> Result<Option<RegExpMatch>, Throw> {
    // `ToLength(lastIndex)` may recompile the expression (`compile`): the
    // flags and the program are read after it.
    let start = last_index(cx, object)?;
    let flags = flags_of(cx, object);
    let moves = flags.contains(RegExpFlags::GLOBAL) || flags.contains(RegExpFlags::STICKY);
    let start = if moves { start } else { 0 };
    let length = cx.heap.string_len(string) as u64;
    if start > length {
        if moves {
            set_last_index(cx, object, Value::int32(0))?;
        }
        return Ok(None);
    }
    let program = cx.heap.header(object).payload;
    match run(cx, program, string, start as usize)? {
        None => {
            if moves {
                set_last_index(cx, object, Value::int32(0))?;
            }
            Ok(None)
        }
        Some(found) => {
            if moves {
                let end = found.capture(0).map_or(start as usize, |range| range.end);
                set_last_index(cx, object, index_value(end as u64))?;
            }
            Ok(Some(found))
        }
    }
}

/// The match array: the matched text and the captures (`undefined` for one
/// that did not participate), `index`, `input`, `groups`, and `indices` for
/// an expression with the `d` flag.
fn match_array(
    cx: &mut Cx<'_>,
    object: ObjectId,
    string: StringId,
    found: &RegExpMatch,
) -> Result<Value, Throw> {
    let captures = found.captures().to_vec();
    // A fresh array: the elements and then `index`, `input` and `groups`
    // go straight into its storage (no setter or prototype can intervene).
    let prototype = cx.realm.intrinsic(Intrinsic::ArrayPrototype);
    let array = cx
        .heap
        .alloc_array(Some(prototype), captures.len() as u32)
        .map_err(|error| error::heap_error(cx, error))?;
    root(cx, Value::object(array));
    let start = captures[0].as_ref().map_or(0, |range| range.start);
    let names = cx.realm.names;
    let mut values = Vec::with_capacity(captures.len());
    for capture in &captures {
        let value = match capture {
            Some(range) => substring_value(cx, string, range.start, range.end)?,
            None => Value::UNDEFINED,
        };
        root(cx, value);
        values.push(value);
        cx.heap
            .push_element(array, value)
            .map_err(|error| error::heap_error(cx, error))?;
    }
    for (key, value) in [
        (names.index, index_value(start as u64)),
        (names.input, Value::string(string)),
    ] {
        cx.heap
            .add_property(array, Names::key(key), crate::PropertyFlags::DEFAULT, value)
            .map_err(|error| error::heap_error(cx, error))?;
    }
    let program = cx.heap.header(object).payload;
    let group_names: Option<Vec<Option<Vec<u16>>>> =
        cx.regexps.program(program).group_names().map(|names| {
            names
                .iter()
                .map(|name| name.as_ref().map(|name| name.to_vec()))
                .collect()
        });
    let groups = match &group_names {
        Some(group_names) => Value::object(groups_object(cx, group_names, &values, |_, value| {
            Ok(value)
        })?),
        None => Value::UNDEFINED,
    };
    root(cx, groups);
    cx.heap
        .add_property(
            array,
            Names::key(names.groups),
            crate::PropertyFlags::DEFAULT,
            groups,
        )
        .map_err(|error| error::heap_error(cx, error))?;
    if flags_of(cx, object).contains(RegExpFlags::HAS_INDICES) {
        let mut pairs = Vec::with_capacity(captures.len());
        for capture in &captures {
            let pair = match capture {
                Some(range) => Value::object(internal::array_from_list(
                    cx,
                    &[
                        index_value(range.start as u64),
                        index_value(range.end as u64),
                    ],
                )?),
                None => Value::UNDEFINED,
            };
            root(cx, pair);
            pairs.push(pair);
        }
        let indices = internal::array_from_list(cx, &pairs)?;
        root(cx, Value::object(indices));
        let index_groups = match &group_names {
            Some(group_names) => {
                Value::object(groups_object(cx, group_names, &pairs, |_, value| {
                    Ok(value)
                })?)
            }
            None => Value::UNDEFINED,
        };
        root(cx, index_groups);
        internal::create_data_property_or_throw(
            cx,
            indices,
            Names::key(names.groups),
            index_groups,
        )?;
        internal::create_data_property_or_throw(
            cx,
            array,
            Names::key(names.indices),
            Value::object(indices),
        )?;
    }
    Ok(Value::object(array))
}

/// A null-prototype `groups` object: each named group's value (of the
/// participating group when a name is duplicated).
fn groups_object(
    cx: &mut Cx<'_>,
    group_names: &[Option<Vec<u16>>],
    values: &[Value],
    map: impl Fn(&mut Cx<'_>, Value) -> Result<Value, Throw>,
) -> Result<ObjectId, Throw> {
    let groups = cx
        .heap
        .alloc_object(ObjectKind::Ordinary, None, group_names.len() as u32)
        .map_err(|error| error::heap_error(cx, error))?;
    root(cx, Value::object(groups));
    for (index, name) in group_names.iter().enumerate() {
        let Some(name) = name else { continue };
        let value = map(cx, values[index + 1])?;
        let key = cx
            .heap
            .property_key_units(name)
            .map_err(|error| error::heap_error(cx, error))?;
        let existing = internal::ordinary_get_own(cx, groups, key);
        if existing.is_some() && value.is_undefined() {
            continue;
        }
        internal::create_data_property_or_throw(cx, groups, key, value)?;
    }
    Ok(groups)
}

/// `RegExpBuiltinExec`.
fn builtin_exec(cx: &mut Cx<'_>, object: ObjectId, string: StringId) -> Result<Value, Throw> {
    match builtin_match(cx, object, string)? {
        Some(found) => match_array(cx, object, string, &found),
        None => Ok(Value::NULL),
    }
}

/// Whether `object`'s `exec` is the intrinsic one.
fn intrinsic_exec(cx: &mut Cx<'_>, object: ObjectId) -> Result<bool, Throw> {
    // An unmodified instance over an unmodified prototype: one slot read.
    if let Some(cache) = cx.realm.regexp_flags.get() {
        let heap = &*cx.heap;
        let prototype = cx.realm.intrinsic(Intrinsic::RegExpPrototype);
        if heap.object_kind(object) == ObjectKind::RegExp
            && Some(heap.shape_of(object))
                == cx.realm.regexp_instance_shape.get().map(|(shape, _)| shape)
            && heap.prototype_of(object) == Some(prototype)
            && heap.shape_of(prototype) == cache.shape
        {
            return Ok(heap.get_slot(prototype, cache.exec_slot) == Value::object(cache.exec));
        }
    }
    let key = Names::key(cx.realm.names.exec);
    let exec = match property::lookup(cx.heap, &cx.realm.names, object, key) {
        property::Lookup::Hit(property::Found::Data(exec)) => exec,
        _ => return Ok(false),
    };
    Ok(exec.as_object().is_some_and(|exec| {
        cx.heap.object_kind(exec) == ObjectKind::Function
            && cx.heap.function_record(exec).native == Native::RegExpExec.index()
    }))
}

/// `RegExpExec(R, S)`.
fn regexp_exec(cx: &mut Cx<'_>, object: ObjectId, string: StringId) -> Result<Value, Throw> {
    if cx.heap.object_kind(object) == ObjectKind::RegExp && intrinsic_exec(cx, object)? {
        return builtin_exec(cx, object, string);
    }
    let exec = internal::get_property(cx, object, Names::key(cx.realm.names.exec))?;
    if ops::is_callable(cx.heap, exec) {
        let result = crate::vm::call(cx, exec, Value::object(object), &[Value::string(string)])?;
        if result.as_object().is_none() && !result.is_null() {
            return Err(error::type_error(
                cx,
                "RegExp exec method must return an object or null",
            ));
        }
        return Ok(result);
    }
    if cx.heap.object_kind(object) != ObjectKind::RegExp {
        return Err(error::object_expected(cx, "RegExp"));
    }
    builtin_exec(cx, object, string)
}

pub(super) fn exec(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let this = site.this(cx);
    let Some(object) = regexp_object(cx, this) else {
        return Err(error::object_expected(cx, "RegExp"));
    };
    let string = site.arg(cx, 0);
    let string = to_string_id(cx, string)?;
    builtin_exec(cx, object, string)
}

pub(super) fn test(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let this = site.this(cx);
    let Some(object) = this.as_object() else {
        return Err(error::object_expected(cx, "RegExp"));
    };
    let string = site.arg(cx, 0);
    let string = to_string_id(cx, string)?;
    if cx.heap.object_kind(object) == ObjectKind::RegExp && intrinsic_exec(cx, object)? {
        return Ok(Value::bool(builtin_match(cx, object, string)?.is_some()));
    }
    Ok(Value::bool(!regexp_exec(cx, object, string)?.is_null()))
}

// ---- the Symbol protocol ----

fn this_object(cx: &mut Cx<'_>, site: &CallSite) -> Result<ObjectId, Throw> {
    match site.this(cx).as_object() {
        Some(object) => Ok(object),
        None => Err(error::type_error(cx, "not an object")),
    }
}

/// `ToString(Get(rx, "flags"))` as code units.
fn flags_text(cx: &mut Cx<'_>, object: ObjectId) -> Result<Vec<u16>, Throw> {
    let flags = internal::get_property(cx, object, Names::key(cx.realm.names.flags))?;
    let flags = to_string_id(cx, flags)?;
    Ok(units(cx, flags))
}

fn has(flags: &[u16], flag: u8) -> bool {
    flags.contains(&u16::from(flag))
}

/// `AdvanceStringIndex`: one unit, or a surrogate pair in Unicode mode.
fn advance(cx: &Cx<'_>, string: StringId, index: u64, unicode: bool) -> u64 {
    let repr = &cx.heap.string(string).repr();
    if !unicode || index + 1 >= repr.len() as u64 {
        return index + 1;
    }
    let lead = repr.unit(index as usize);
    let trail = repr.unit(index as usize + 1);
    if (0xD800..0xDC00).contains(&lead) && (0xDC00..0xE000).contains(&trail) {
        index + 2
    } else {
        index + 1
    }
}

/// After an empty match: `lastIndex` advances past it.
fn advance_empty(
    cx: &mut Cx<'_>,
    object: ObjectId,
    string: StringId,
    unicode: bool,
) -> Result<(), Throw> {
    let index = last_index(cx, object)?;
    let next = advance(cx, string, index, unicode);
    set_last_index(cx, object, index_value(next))
}

/// `ToString(Get(result, "0"))`.
fn matched_text(cx: &mut Cx<'_>, result: Value) -> Result<StringId, Throw> {
    let result = result.as_object().expect("a match result");
    let key = crate::PropertyKey::index(0).expect("an index key");
    let matched = internal::get_property(cx, result, key)?;
    to_string_id(cx, matched)
}

/// `RegExp.prototype[Symbol.match](string)`.
pub(super) fn symbol_match(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let object = this_object(cx, &site)?;
    let string = site.arg(cx, 0);
    let string = to_string_id(cx, string)?;
    if let Some(flags) = pristine_exec_flags(cx, object)?
        && flags.contains(RegExpFlags::GLOBAL)
    {
        // The matched texts straight from the matcher.
        let unicode =
            flags.contains(RegExpFlags::UNICODE) || flags.contains(RegExpFlags::UNICODE_SETS);
        set_last_index(cx, object, Value::int32(0))?;
        let array = internal::array_create(cx, 0)?;
        root(cx, Value::object(array));
        let mut count = 0u32;
        while let Some(found) = builtin_match(cx, object, string)? {
            let range = found.captures()[0].clone().unwrap_or(0..0);
            let matched = substring_value(cx, string, range.start, range.end)?;
            root(cx, matched);
            cx.heap
                .push_element(array, matched)
                .map_err(|error| error::heap_error(cx, error))?;
            count += 1;
            if range.is_empty() {
                advance_empty(cx, object, string, unicode)?;
            }
        }
        return Ok(if count == 0 {
            Value::NULL
        } else {
            Value::object(array)
        });
    }
    let flags = flags_text(cx, object)?;
    if !has(&flags, b'g') {
        return regexp_exec(cx, object, string);
    }
    let unicode = has(&flags, b'u') || has(&flags, b'v');
    set_last_index(cx, object, Value::int32(0))?;
    let array = internal::array_create(cx, 0)?;
    root(cx, Value::object(array));
    let mut count = 0u32;
    loop {
        let result = regexp_exec(cx, object, string)?;
        if result.is_null() {
            return Ok(if count == 0 {
                Value::NULL
            } else {
                Value::object(array)
            });
        }
        root(cx, result);
        let matched = matched_text(cx, result)?;
        let key = internal::index_key(cx, count)?;
        internal::create_data_property_or_throw(cx, array, key, Value::string(matched))?;
        count += 1;
        if cx.heap.string_len(matched) == 0 {
            advance_empty(cx, object, string, unicode)?;
        }
    }
}

/// `RegExp.prototype[Symbol.matchAll](string)`.
pub(super) fn symbol_match_all(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let object = this_object(cx, &site)?;
    let string = site.arg(cx, 0);
    let string = to_string_id(cx, string)?;
    let flags = flags_text(cx, object)?;
    let flags_value = string_units(cx, &flags)?;
    root(cx, flags_value);
    let constructor = Value::object(cx.realm.intrinsic(Intrinsic::RegExp));
    let matcher = crate::vm::construct(
        cx,
        constructor,
        &[Value::object(object), flags_value],
        constructor,
    )?;
    root(cx, matcher);
    let matcher_object = matcher.as_object().expect("a RegExp");
    let index = last_index(cx, object)?;
    set_last_index(cx, matcher_object, index_value(index))?;
    let global = has(&flags, b'g');
    let unicode = has(&flags, b'u') || has(&flags, b'v');
    let prototype = cx.realm.intrinsic(Intrinsic::RegExpStringIteratorPrototype);
    cx.heap
        .alloc_internal(
            ObjectKind::Builtin,
            InternalTag::RegExpStringIterator,
            Some(prototype),
            0,
            &[
                matcher,
                Value::string(string),
                Value::bool(global),
                Value::bool(unicode),
                Value::FALSE,
            ],
            Default::default(),
            0,
        )
        .map(Value::object)
        .map_err(|error| error::heap_error(cx, error))
}

/// `%RegExpStringIteratorPrototype%.next`.
pub(super) fn string_iterator_next(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let this = site.this(cx);
    let Some(iterator) = this
        .as_object()
        .filter(|&object| cx.heap.internal_tag(object) == InternalTag::RegExpStringIterator)
    else {
        return Err(error::object_expected(cx, "RegExp String Iterator"));
    };
    let slots = cx.heap.internal(iterator).slots.clone();
    if slots[4] == Value::TRUE {
        return iter::result_object(cx, Value::UNDEFINED, true);
    }
    let matcher = slots[0].as_object().expect("a matcher");
    let string = slots[1].as_string().expect("a string");
    let result = regexp_exec(cx, matcher, string)?;
    if result.is_null() {
        cx.heap.set_internal_slot(iterator, 4, Value::TRUE);
        return iter::result_object(cx, Value::UNDEFINED, true);
    }
    root(cx, result);
    if slots[2] != Value::TRUE {
        cx.heap.set_internal_slot(iterator, 4, Value::TRUE);
        return iter::result_object(cx, result, false);
    }
    let matched = matched_text(cx, result)?;
    if cx.heap.string_len(matched) == 0 {
        advance_empty(cx, matcher, string, slots[3] == Value::TRUE)?;
    }
    iter::result_object(cx, result, false)
}

/// `RegExp.prototype[Symbol.replace](string, replaceValue)`.
pub(super) fn symbol_replace(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let object = this_object(cx, &site)?;
    let string = site.arg(cx, 0);
    let string = to_string_id(cx, string)?;
    let length = cx.heap.string_len(string);
    let replace_value = site.arg(cx, 1);
    let functional = ops::is_callable(cx.heap, replace_value);
    let replacement = if functional {
        None
    } else {
        let replacement = to_string_id(cx, replace_value)?;
        Some(units(cx, replacement))
    };
    if let Some(flags) = pristine_exec_flags(cx, object)? {
        return replace_pristine(cx, object, string, flags, replace_value, replacement);
    }
    let flags = flags_text(cx, object)?;
    let global = has(&flags, b'g');
    let unicode = has(&flags, b'u') || has(&flags, b'v');
    if global {
        set_last_index(cx, object, Value::int32(0))?;
    }
    let mut results = Vec::new();
    loop {
        let result = regexp_exec(cx, object, string)?;
        if result.is_null() {
            break;
        }
        root(cx, result);
        results.push(result);
        if !global {
            break;
        }
        let matched = matched_text(cx, result)?;
        if cx.heap.string_len(matched) == 0 {
            advance_empty(cx, object, string, unicode)?;
        }
    }
    let text = units(cx, string);
    let mut accumulated: Vec<u16> = Vec::with_capacity(text.len());
    let mut next_position = 0usize;
    for result in results {
        let result_object = result.as_object().expect("a match result");
        let result_length = internal::length_of_array_like(cx, result_object)?;
        let capture_count = result_length.saturating_sub(1);
        let matched = matched_text(cx, result)?;
        root(cx, Value::string(matched));
        let matched_units = units(cx, matched);
        let index = internal::get_property(cx, result_object, Names::key(cx.realm.names.index))?;
        let position =
            internal::to_integer_or_infinity(cx, index)?.clamp(0.0, length as f64) as usize;
        let mut captures: Vec<Value> = Vec::with_capacity(capture_count as usize);
        for n in 1..=capture_count {
            let key = internal::number_key(cx, n as f64)?;
            let capture = internal::get_property(cx, result_object, key)?;
            let capture = if capture.is_undefined() {
                capture
            } else {
                ops::to_string(cx, capture)?
            };
            root(cx, capture);
            captures.push(capture);
        }
        let groups = internal::get_property(cx, result_object, Names::key(cx.realm.names.groups))?;
        root(cx, groups);
        let replaced: Vec<u16> = if functional {
            let mut args = vec![Value::string(matched)];
            args.extend_from_slice(&captures);
            args.push(index_value(position as u64));
            args.push(Value::string(string));
            if !groups.is_undefined() {
                args.push(groups);
            }
            let value = crate::vm::call(cx, replace_value, Value::UNDEFINED, &args)?;
            let value = to_string_id(cx, value)?;
            units(cx, value)
        } else {
            let groups = if groups.is_undefined() {
                None
            } else {
                Some(internal::to_object(cx, groups)?)
            };
            let capture_units: Vec<Option<Vec<u16>>> = captures
                .iter()
                .map(|capture| capture.as_string().map(|id| units(cx, id)))
                .collect();
            let replacement = replacement.clone().unwrap_or_default();
            substitute_with_groups(
                cx,
                &text,
                position,
                &matched_units,
                &capture_units,
                groups,
                &replacement,
            )?
        };
        if position >= next_position {
            accumulated.extend_from_slice(&text[next_position..position]);
            accumulated.extend(replaced);
            next_position = position + matched_units.len();
        }
    }
    if next_position < text.len() {
        accumulated.extend_from_slice(&text[next_position..]);
    }
    string_units(cx, &accumulated)
}

/// The flags of an unmodified `%RegExp%` instance whose `exec` is the
/// intrinsic one and whose program has no named groups: then `RegExpExec`'s
/// result arrays are fresh, ordinary and unobservable, and the protocol
/// methods may work from the matcher's captures directly.
fn pristine_exec_flags(cx: &mut Cx<'_>, object: ObjectId) -> Result<Option<RegExpFlags>, Throw> {
    let Some(cache) = cx.realm.regexp_flags.get() else {
        return Ok(None);
    };
    let Some(flags) = pristine_flags(cx, object, &cache) else {
        return Ok(None);
    };
    let program = cx.heap.header(object).payload;
    if cx.regexps.program(program).group_names().is_some() || !intrinsic_exec(cx, object)? {
        return Ok(None);
    }
    Ok(Some(flags))
}

/// `RegExp.prototype[Symbol.replace]` for a pristine expression
/// ([`pristine_exec_flags`]): the same `lastIndex` traffic, the matches
/// taken from the matcher.
fn replace_pristine(
    cx: &mut Cx<'_>,
    object: ObjectId,
    string: StringId,
    flags: RegExpFlags,
    replace_value: Value,
    replacement: Option<Vec<u16>>,
) -> Result<Value, Throw> {
    let global = flags.contains(RegExpFlags::GLOBAL);
    let unicode = flags.contains(RegExpFlags::UNICODE) || flags.contains(RegExpFlags::UNICODE_SETS);
    if global {
        set_last_index(cx, object, Value::int32(0))?;
    }
    let mut matches = Vec::new();
    while let Some(found) = builtin_match(cx, object, string)? {
        let empty = found.captures()[0]
            .as_ref()
            .is_none_or(|range| range.is_empty());
        matches.push(found);
        if !global {
            break;
        }
        if empty {
            advance_empty(cx, object, string, unicode)?;
        }
    }
    if matches.is_empty() {
        return Ok(Value::string(string));
    }
    let text = units(cx, string);
    let mut accumulated: Vec<u16> = Vec::with_capacity(text.len());
    let mut next_position = 0usize;
    // A replacement without `$` is copied as is.
    let literal = replacement
        .as_ref()
        .filter(|replacement| !replacement.contains(&u16::from(b'$')));
    for found in matches {
        let captures = found.captures();
        let range = captures[0].clone().unwrap_or(0..0);
        if let Some(literal) = literal {
            if range.start >= next_position {
                accumulated.extend_from_slice(&text[next_position..range.start]);
                accumulated.extend_from_slice(literal);
                next_position = range.end;
            }
            continue;
        }
        let replaced: Vec<u16> = match &replacement {
            None => {
                let mark = cx.heap.roots().mark();
                let mut args = Vec::with_capacity(captures.len() + 2);
                for capture in captures {
                    let value = match capture {
                        Some(range) => substring_value(cx, string, range.start, range.end)?,
                        None => Value::UNDEFINED,
                    };
                    root(cx, value);
                    args.push(value);
                }
                args.push(index_value(range.start as u64));
                args.push(Value::string(string));
                let value = crate::vm::call(cx, replace_value, Value::UNDEFINED, &args);
                cx.heap.roots().truncate(mark);
                let value = to_string_id(cx, value?)?;
                units(cx, value)
            }
            Some(replacement) => {
                let capture_units: Vec<Option<Vec<u16>>> = captures[1..]
                    .iter()
                    .map(|capture| capture.as_ref().map(|range| text[range.clone()].to_vec()))
                    .collect();
                substitute_with_groups(
                    cx,
                    &text,
                    range.start,
                    &text[range.clone()],
                    &capture_units,
                    None,
                    replacement,
                )?
            }
        };
        if range.start >= next_position {
            accumulated.extend_from_slice(&text[next_position..range.start]);
            accumulated.extend(replaced);
            next_position = range.end;
        }
        if accumulated.len() > super::string::MAX_STRING {
            return Err(error::string_too_long(cx));
        }
    }
    if next_position < text.len() {
        accumulated.extend_from_slice(&text[next_position..]);
    }
    string_units(cx, &accumulated)
}

/// `GetSubstitution` with a `groups` object read for `$<name>`.
fn substitute_with_groups(
    cx: &mut Cx<'_>,
    text: &[u16],
    position: usize,
    matched: &[u16],
    captures: &[Option<Vec<u16>>],
    groups: Option<ObjectId>,
    replacement: &[u16],
) -> Result<Vec<u16>, Throw> {
    // Group values are read in replacement order first: `Get` may run
    // getters, which `substitution`'s callback cannot.
    let mut named: Vec<(Vec<u16>, Option<Vec<u16>>)> = Vec::new();
    if let Some(groups) = groups {
        let mut at = 0;
        while at + 1 < replacement.len() {
            if replacement[at] == u16::from(b'$') && replacement[at + 1] == u16::from(b'<') {
                if let Some(close) = replacement[at + 2..]
                    .iter()
                    .position(|&unit| unit == u16::from(b'>'))
                {
                    let name = replacement[at + 2..at + 2 + close].to_vec();
                    let key = cx
                        .heap
                        .property_key_units(&name)
                        .map_err(|error| error::heap_error(cx, error))?;
                    let value = internal::get_property(cx, groups, key)?;
                    let value = if value.is_undefined() {
                        None
                    } else {
                        let value = to_string_id(cx, value)?;
                        Some(units(cx, value))
                    };
                    named.push((name, value));
                    at += close + 3;
                    continue;
                }
                break;
            }
            at += 1;
        }
    }
    let lookup = |name: &[u16]| -> Option<Option<Vec<u16>>> {
        groups?;
        Some(
            named
                .iter()
                .find(|(key, _)| key.as_slice() == name)
                .and_then(|(_, value)| value.clone()),
        )
    };
    substitution(text, position, matched, captures, &lookup, replacement)
        .ok_or_else(|| error::string_too_long(cx))
}

/// `RegExp.prototype[Symbol.search](string)`.
pub(super) fn symbol_search(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let object = this_object(cx, &site)?;
    let string = site.arg(cx, 0);
    let string = to_string_id(cx, string)?;
    let key = Names::key(cx.realm.names.last_index);
    let previous = internal::get_property(cx, object, key)?;
    root(cx, previous);
    if !internal::same_value(cx, previous, Value::int32(0)) {
        set_last_index(cx, object, Value::int32(0))?;
    }
    let result = regexp_exec(cx, object, string)?;
    root(cx, result);
    let current = internal::get_property(cx, object, key)?;
    if !internal::same_value(cx, current, previous) {
        set_last_index(cx, object, previous)?;
    }
    match result.as_object() {
        None => Ok(Value::int32(-1)),
        Some(result) => internal::get_property(cx, result, Names::key(cx.realm.names.index)),
    }
}

/// `RegExp.prototype[Symbol.split](string, limit)`.
pub(super) fn symbol_split(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let object = this_object(cx, &site)?;
    let string = site.arg(cx, 0);
    let string = to_string_id(cx, string)?;
    let mut flags = flags_text(cx, object)?;
    let unicode = has(&flags, b'u') || has(&flags, b'v');
    // A non-sticky original searches for the next match where the sticky
    // splitter would try each position.
    let searcher = match cx.realm.regexp_flags.get() {
        Some(cache) if !has(&flags, b'y') && pristine_flags(cx, object, &cache).is_some() => {
            Some(cx.heap.header(object).payload)
        }
        _ => None,
    };
    if !has(&flags, b'y') {
        flags.push(u16::from(b'y'));
    }
    // The splitter of a pristine expression is unobservable: the search
    // uses the expression's own program.
    let splitter = if searcher.is_some() {
        object
    } else {
        let flags_value = string_units(cx, &flags)?;
        root(cx, flags_value);
        let constructor = Value::object(cx.realm.intrinsic(Intrinsic::RegExp));
        let splitter = crate::vm::construct(
            cx,
            constructor,
            &[Value::object(object), flags_value],
            constructor,
        )?;
        root(cx, splitter);
        splitter.as_object().expect("a RegExp")
    };
    let limit = site.arg(cx, 1);
    let limit = if limit.is_undefined() {
        u32::MAX
    } else {
        crate::vm::number::to_uint32(ops::to_number(cx, limit)?)
    } as usize;
    let mut parts: Vec<Value> = Vec::new();
    if limit == 0 {
        return internal::array_from_list(cx, &parts).map(Value::object);
    }
    let size = cx.heap.string_len(string);
    if size == 0 {
        let matched = match searcher {
            Some(searcher) => run(cx, searcher, string, 0)?.is_some(),
            None => !regexp_exec(cx, splitter, string)?.is_null(),
        };
        if !matched {
            parts.push(Value::string(string));
        }
        return internal::array_from_list(cx, &parts).map(Value::object);
    }
    let fast = intrinsic_exec(cx, splitter)?;
    let push = |cx: &mut Cx<'_>, parts: &mut Vec<Value>, value: Value| {
        root(cx, value);
        parts.push(value);
    };
    let (mut p, mut q) = (0usize, 0usize);
    'split: while q < size {
        // The splitter is sticky: a match must start at `q`.
        let (end, captures) = if fast {
            let program = cx.heap.header(splitter).payload;
            let found = match searcher {
                Some(searcher) => match run(cx, searcher, string, q)? {
                    Some(found) => {
                        // The sticky attempts before the match all fail.
                        q = found.capture(0).map_or(q, |range| range.start);
                        if q >= size {
                            break 'split;
                        }
                        Some(found)
                    }
                    None => break 'split,
                },
                None => run(cx, program, string, q)?,
            };
            match found {
                Some(found) if found.capture(0).is_some_and(|range| range.start == q) => {
                    let end = found.capture(0).map_or(q, |range| range.end).min(size);
                    let mut captures = Vec::new();
                    for capture in &found.captures()[1..] {
                        let value = match capture {
                            Some(range) => substring_value(cx, string, range.start, range.end)?,
                            None => Value::UNDEFINED,
                        };
                        root(cx, value);
                        captures.push(value);
                    }
                    (Some(end), captures)
                }
                _ => (None, Vec::new()),
            }
        } else {
            set_last_index(cx, splitter, index_value(q as u64))?;
            let result = regexp_exec(cx, splitter, string)?;
            match result.as_object() {
                None => (None, Vec::new()),
                Some(result_object) => {
                    root(cx, result);
                    let end = last_index(cx, splitter)?.min(size as u64) as usize;
                    let count =
                        internal::length_of_array_like(cx, result_object)?.saturating_sub(1);
                    let mut captures = Vec::new();
                    for n in 1..=count {
                        let key = internal::number_key(cx, n as f64)?;
                        let value = internal::get_property(cx, result_object, key)?;
                        root(cx, value);
                        captures.push(value);
                    }
                    (Some(end), captures)
                }
            }
        };
        match end {
            Some(end) if end != p => {
                let piece = substring_value(cx, string, p, q)?;
                push(cx, &mut parts, piece);
                if parts.len() == limit {
                    break 'split;
                }
                p = end;
                for capture in captures {
                    push(cx, &mut parts, capture);
                    if parts.len() == limit {
                        break 'split;
                    }
                }
                q = p;
            }
            _ => q = advance(cx, string, q as u64, unicode) as usize,
        }
    }
    if parts.len() < limit {
        let rest = substring_value(cx, string, p, size)?;
        push(cx, &mut parts, rest);
    }
    internal::array_from_list(cx, &parts).map(Value::object)
}

/// The receiver of a `String.prototype` method that delegates to a
/// `Symbol` method of its argument, when the argument has one.
pub(crate) fn delegate(
    cx: &mut Cx<'_>,
    site: &CallSite,
    symbol: crate::SymbolId,
    args: &[Value],
) -> Result<Option<Value>, Throw> {
    let this = site.this(cx);
    if this.is_undefined() || this.is_null() {
        return Err(error::type_error(cx, "cannot convert to object"));
    }
    // Only an object's method is consulted (a primitive's prototype is
    // not, as of ES2025).
    let argument = site.arg(cx, 0);
    if argument.as_object().is_none() {
        return Ok(None);
    }
    // An unmodified instance over an unmodified prototype: the method is
    // the prototype's slot.
    let cached = cx.realm.regexp_flags.get().and_then(|cache| {
        let object = argument.as_object()?;
        let heap = &*cx.heap;
        let prototype = cx.realm.intrinsic(Intrinsic::RegExpPrototype);
        let (_, slot) = cache
            .symbol_slots
            .iter()
            .find(|(known, _)| *known == symbol)?;
        (last_index_slot(cx, object).is_some()
            && heap.prototype_of(object) == Some(prototype)
            && heap.shape_of(prototype) == cache.shape)
            .then(|| heap.get_slot(prototype, *slot))
    });
    let method = match cached {
        Some(method) if ops::is_callable(cx.heap, method) => method,
        _ => {
            let key = WellKnown::key(symbol);
            let Some(method) = internal::get_method(cx, argument, key)? else {
                return Ok(None);
            };
            method
        }
    };
    root(cx, method);
    let mut call_args = vec![this];
    call_args.extend_from_slice(args);
    crate::vm::call(cx, method, argument, &call_args).map(Some)
}

/// `String.prototype.match`/`matchAll`/`search` without a delegate: a new
/// RegExp over the argument, then its `Symbol` method.
pub(crate) fn create_and_invoke(
    cx: &mut Cx<'_>,
    site: &CallSite,
    symbol: crate::SymbolId,
    flags: &str,
) -> Result<Value, Throw> {
    let string = this_string(cx, site)?;
    let argument = site.arg(cx, 0);
    let source = if argument.is_undefined() {
        cx.realm.names.empty
    } else {
        atom(cx, argument)?
    };
    let flags = cx
        .heap
        .intern(flags)
        .map_err(|error| error::heap_error(cx, error))?;
    let regexp = crate::vm::regexp::regexp_object(cx, source, flags)?;
    root(cx, regexp);
    let key = WellKnown::key(symbol);
    internal::invoke(cx, regexp, key, &[Value::string(string)])
}
