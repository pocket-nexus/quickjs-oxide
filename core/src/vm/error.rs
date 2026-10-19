//! Error objects the loop and the natives throw: an ordinary object flagged
//! `ERROR`, with the kind's prototype and an own `message`; `name` lives on the
//! prototype. Each carries QuickJS's `stack` backtrace.

use std::fmt::Write;

use super::{Cx, Throw, frame_flags};
use crate::heap::HeapError;
use crate::runtime::{Intrinsic, Names, Realm};
use crate::shape::PropertyFlags;
use crate::value::{ObjectId, Value};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorKind {
    Error,
    Eval,
    Range,
    Reference,
    Syntax,
    Type,
    Internal,
    Uri,
    Aggregate,
}

impl ErrorKind {
    pub const ALL: [ErrorKind; 9] = [
        ErrorKind::Error,
        ErrorKind::Eval,
        ErrorKind::Range,
        ErrorKind::Reference,
        ErrorKind::Syntax,
        ErrorKind::Type,
        ErrorKind::Internal,
        ErrorKind::Uri,
        ErrorKind::Aggregate,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            ErrorKind::Error => "Error",
            ErrorKind::Eval => "EvalError",
            ErrorKind::Range => "RangeError",
            ErrorKind::Reference => "ReferenceError",
            ErrorKind::Syntax => "SyntaxError",
            ErrorKind::Type => "TypeError",
            ErrorKind::Internal => "InternalError",
            ErrorKind::Uri => "URIError",
            ErrorKind::Aggregate => "AggregateError",
        }
    }

    pub const fn prototype(self) -> Intrinsic {
        match self {
            ErrorKind::Error => Intrinsic::ErrorPrototype,
            ErrorKind::Eval => Intrinsic::EvalErrorPrototype,
            ErrorKind::Range => Intrinsic::RangeErrorPrototype,
            ErrorKind::Reference => Intrinsic::ReferenceErrorPrototype,
            ErrorKind::Syntax => Intrinsic::SyntaxErrorPrototype,
            ErrorKind::Type => Intrinsic::TypeErrorPrototype,
            ErrorKind::Internal => Intrinsic::InternalErrorPrototype,
            ErrorKind::Uri => Intrinsic::UriErrorPrototype,
            ErrorKind::Aggregate => Intrinsic::AggregateErrorPrototype,
        }
    }
}

/// Where in which source an error is: a syntax error's file, line and
/// column (one-based).
#[derive(Clone, Copy, Debug)]
pub(crate) struct Location<'a> {
    pub file: &'a str,
    pub line: u32,
    pub column: u32,
}

/// A new error object of `kind`; `message` must be rooted (or not a heap
/// value) by the caller.
pub(crate) fn new_error(
    cx: &mut Cx<'_>,
    kind: ErrorKind,
    message: Value,
) -> Result<Value, HeapError> {
    new_error_at(cx, kind, message, None, 0)
}

/// [`new_error`] located at `location` (its `fileName`, `lineNumber` and
/// `columnNumber`, and the first line of its `stack`), with the running
/// frames' backtrace after it but the `omit` innermost ([`add_stack_at`]).
pub(crate) fn new_error_at(
    cx: &mut Cx<'_>,
    kind: ErrorKind,
    message: Value,
    location: Option<Location<'_>>,
    omit: usize,
) -> Result<Value, HeapError> {
    let prototype = cx.realm.intrinsic(kind.prototype());
    let heap = &mut *cx.heap;
    let mark = heap.roots().mark();
    heap.roots().push(message);
    let result = (|| {
        let object = heap.alloc_object(crate::ObjectKind::Ordinary, Some(prototype), 1)?;
        heap.set_object_flag(object, crate::heap::object::flags::ERROR);
        if !message.is_undefined() {
            let key = Names::key(cx.realm.names.message);
            heap.add_property(object, key, PropertyFlags::data(true, false, true), message)?;
        }
        Ok(object)
    })();
    let result = result.and_then(|object| {
        cx.heap.roots().push(Value::object(object));
        add_stack_at(cx, object, false, location, omit).map(|()| Value::object(object))
    });
    cx.heap.roots().truncate(mark);
    result
}

/// An error object of `kind` with neither message nor `stack`: QuickJS's
/// internal `AggregateError` of `Promise.any`, made by
/// `JS_NewObjectProtoClass` without a backtrace.
pub(crate) fn new_bare_error(cx: &mut Cx<'_>, kind: ErrorKind) -> Result<Value, HeapError> {
    let prototype = cx.realm.intrinsic(kind.prototype());
    let object = cx
        .heap
        .alloc_object(crate::ObjectKind::Ordinary, Some(prototype), 1)?;
    cx.heap
        .set_object_flag(object, crate::heap::object::flags::ERROR);
    Ok(Value::object(object))
}

/// Most frames a backtrace lists.
const BACKTRACE_FRAMES: usize = 64;

/// QuickJS's `build_backtrace`: the bytecode frames, innermost first, as
/// `    at name (file:line:column)` lines in the own `stack` property of
/// `object`, which the caller roots. A frame whose current instruction is
/// unknown (below a re-entry from Rust) has no position.
///
/// A native running (called from the innermost frame) is listed first as
/// `    at name (native)`, unless `skip_native` (an Error constructor, which
/// QuickJS's `BACKTRACE_FLAG_SKIP_FIRST_LEVEL` leaves out). When the
/// innermost frame's instruction is unknown (the loop itself throws), the
/// loop's unwind fills its position in later ([`locate_first_frame`]).
pub(crate) fn add_stack(
    cx: &mut Cx<'_>,
    object: ObjectId,
    skip_native: bool,
) -> Result<(), HeapError> {
    add_stack_at(cx, object, skip_native, None, 0)
}

/// [`add_stack`] after QuickJS's `build_backtrace` with a `filename`: the
/// location's own `fileName`, `lineNumber` and `columnNumber` (writable,
/// configurable), and the backtrace's first line; the running frames follow
/// but the `omit` innermost (`usize::MAX`: an error of a source no frame
/// runs; one: an error of a script's declaration prologue, which QuickJS
/// checks in `js_closure2` before the script's frame exists).
pub(crate) fn add_stack_at(
    cx: &mut Cx<'_>,
    object: ObjectId,
    skip_native: bool,
    location: Option<Location<'_>>,
    omit: usize,
) -> Result<(), HeapError> {
    let mut text = String::new();
    if let Some(Location { file, line, column }) = location {
        let _ = writeln!(text, "    at {file}:{line}:{column}");
        let names = cx.realm.names;
        let flags = PropertyFlags::data(true, false, true);
        let file = cx.heap.alloc_string(file)?;
        cx.heap.add_property(
            object,
            Names::key(names.file_name),
            flags,
            Value::string(file),
        )?;
        cx.heap.add_property(
            object,
            Names::key(names.line_number),
            flags,
            Value::int32(line as i32),
        )?;
        cx.heap.add_property(
            object,
            Names::key(names.column_number),
            flags,
            Value::int32(column as i32),
        )?;
    }
    let vm = &cx.heap.vm;
    let len = vm.frames.len();
    let top = len.saturating_sub(omit);
    let mut line_index = u32::from(location.is_some());
    let eval = Value::object(cx.realm.intrinsic(crate::Intrinsic::Eval));
    let mut sites = vm.natives.iter().rev().peekable();
    while sites.next_if(|site| site.depth as usize > top).is_some() {}
    let mut unpositioned = Vec::new();
    for index in (0..top).rev().take(BACKTRACE_FRAMES) {
        let frame = &vm.frames[index];
        let unit = cx.unit(frame.unit);
        // The natives this frame called and which are still running (one
        // the loop called, then those it called from Rust): their lines,
        // innermost first, then this frame at the loop's call.
        let mut site_pc = None;
        let mut matched = false;
        while let Some(site) = sites.next_if(|site| site.depth as usize == index + 1) {
            let direct_eval =
                site.callee == eval && unit.direct_eval_sites.binary_search(&site.pc).is_ok();
            // A site with no callee only places the frame (an async
            // function's body entered from its call).
            if let Some(callee) = site.callee.as_object()
                && !(skip_native && index + 1 == len && !matched)
                && !direct_eval
            {
                let name = native_name(cx, callee);
                let name = if name.is_empty() {
                    "<anonymous>"
                } else {
                    &name
                };
                let _ = writeln!(text, "    at {name} (native)");
                line_index += 1;
            }
            matched = true;
            site_pc = (site.pc != super::NO_PC).then_some(site.pc);
        }
        let pc = match site_pc {
            Some(pc) => Some(pc),
            None if index + 1 == len => None,
            None => {
                let above = &vm.frames[index + 1];
                (above.flags & frame_flags::ENTRY == 0 && above.return_pc > 0)
                    .then(|| above.return_pc - 1)
            }
        };
        if pc.is_none() {
            unpositioned.push((index as u32 + 1, line_index));
        }
        line_index += 1;
        let debug = &unit.debug;
        // QuickJS reads the running function's `name` (`get_prop_string`);
        // its bytecode functions all have one, which a script function here
        // lacks: the unit's name stands in.
        let key = Names::key(cx.realm.names.name);
        let name =
            if cx.heap.lookup_own(frame.function, key).is_none() && !debug_name(&unit).is_empty() {
                debug_name(&unit).to_owned()
            } else {
                native_name(cx, frame.function)
            };
        let name = if name.is_empty() {
            "<anonymous>"
        } else {
            &name
        };
        // A unit compiled without debug information (`-s`) has no file, and
        // QuickJS's line then stops at the name.
        let _ = match (&debug.filename, pc.and_then(|pc| debug.position(pc))) {
            (None, _) => writeln!(text, "    at {name}"),
            (Some(file), Some((line, column))) => {
                writeln!(text, "    at {name} ({file}:{line}:{column})")
            }
            (Some(file), None) => writeln!(text, "    at {name} ({file})"),
        };
        // Entered through a rewritten `call` or `apply`: the native QuickJS ran
        // between this frame and its caller.
        if frame.flags & frame_flags::VIA_CALL != 0 {
            let _ = writeln!(text, "    at call (native)");
            line_index += 1;
        } else if frame.flags & frame_flags::VIA_APPLY != 0 {
            let _ = writeln!(text, "    at apply (native)");
            line_index += 1;
        } else if frame.flags & frame_flags::VIA_CHAIN != 0 {
            let vias = vm
                .via
                .frames
                .iter()
                .rev()
                .filter(|(at, _)| *at as usize == index);
            for &(_, via) in vias {
                let name = via
                    .as_object()
                    .map(|via| native_name(cx, via))
                    .unwrap_or_default();
                let _ = writeln!(text, "    at {name} (native)");
                line_index += 1;
            }
        }
    }
    let text = cx.heap.alloc_string(&text)?;
    let key = Names::key(cx.realm.names.stack);
    cx.heap.add_property(
        object,
        key,
        PropertyFlags::data(true, false, true),
        Value::string(text),
    )?;
    cx.heap.vm.unpositioned = (!unpositioned.is_empty()).then_some((object, unpositioned));
    Ok(())
}

fn debug_name(unit: &crate::bytecode::CodeUnit) -> &str {
    &unit.debug.name
}

/// A function's `name` for its backtrace line, as QuickJS's
/// `get_prop_string` reads it: an own data property, else the prototype's
/// (one level); empty unless a string.
fn native_name(cx: &Cx<'_>, function: ObjectId) -> String {
    let key = Names::key(cx.realm.names.name);
    let read = |object: ObjectId| {
        cx.heap.lookup_own(object, key).map(|(slot, flags)| {
            if flags.has(PropertyFlags::ACCESSOR) {
                String::new()
            } else {
                // A rope (`JS_TAG_STRING_ROPE`) reads as absent.
                cx.heap
                    .get_slot(object, slot)
                    .as_string()
                    .filter(|id| cx.heap.strings.get(id.0).flat().is_some())
                    .map(|id| cx.heap.string_text(id))
                    .unwrap_or_default()
            }
        })
    };
    read(function)
        .or_else(|| cx.heap.prototype_of(function).and_then(read))
        .unwrap_or_default()
}

/// Where the throw of `thrown` passes through the frame at the top of the
/// stack, at instruction `pc` of `unit`: places that frame's backtrace line
/// if [`add_stack`] could not.
#[cold]
#[inline(never)]
pub(crate) fn place_frame(
    cx: &mut Cx<'_>,
    thrown: Value,
    unit: &crate::bytecode::CodeUnit,
    pc: u32,
) {
    let depth = cx.heap.vm.frames.len() as u32;
    let Some((error, entries)) = &mut cx.heap.vm.unpositioned else {
        return;
    };
    if thrown.as_object() != Some(*error) {
        return;
    }
    let error = *error;
    let Some(entry) = entries.iter().position(|&(frame, _)| frame == depth) else {
        return;
    };
    let (_, line) = entries.swap_remove(entry);
    locate_line(cx, error, unit, pc, line as usize);
}

/// Puts the line and column of instruction `pc` of `unit` into line `index`
/// of `error`'s backtrace (`    at name (file)` becomes
/// `    at name (file:line:column)`).
fn locate_line(
    cx: &mut Cx<'_>,
    error: ObjectId,
    unit: &crate::bytecode::CodeUnit,
    pc: u32,
    index: usize,
) {
    let Some((line, column)) = unit.debug.position(pc) else {
        return;
    };
    let key = Names::key(cx.realm.names.stack);
    let Some((slot, flags)) = cx.heap.lookup_own(error, key) else {
        return;
    };
    if flags.has(PropertyFlags::ACCESSOR) {
        return;
    }
    let Some(id) = cx.heap.get_slot(error, slot).as_string() else {
        return;
    };
    let text = cx.heap.string_text(id);
    let mut lines: Vec<String> = text.split('\n').map(str::to_owned).collect();
    let Some(target) = lines.get_mut(index) else {
        return;
    };
    let Some(head) = target.strip_suffix(')') else {
        return;
    };
    if head.ends_with("native")
        || head
            .rsplit('(')
            .next()
            .is_some_and(|file| file.contains(':'))
    {
        return;
    }
    *target = format!("{head}:{line}:{column})");
    let located = lines.join("\n");
    let mark = cx.heap.roots().mark();
    cx.heap.roots().push(Value::object(error));
    if let Ok(located) = cx.heap.alloc_string(&located) {
        cx.heap.set_slot(error, slot, Value::string(located));
    }
    cx.heap.roots().truncate(mark);
}

/// The thrown value for an allocation failure.
#[inline(never)]
pub(crate) fn out_of_memory(cx: &Cx<'_>) -> Throw {
    oom(cx.realm)
}

/// [`out_of_memory`] with only the realm borrowed.
pub(crate) fn oom(realm: &Realm) -> Throw {
    Throw(Value::object(realm.intrinsic(Intrinsic::OutOfMemoryError)))
}

#[inline(never)]
pub(crate) fn heap_error(cx: &Cx<'_>, _: HeapError) -> Throw {
    out_of_memory(cx)
}

/// A new `kind` error with `message`, formatted as QuickJS's
/// `JS_ThrowError2` buffer leaves it ([`buffered_message`]).
#[inline(never)]
pub(crate) fn throw(cx: &mut Cx<'_>, kind: ErrorKind, message: &str) -> Throw {
    let units: Vec<u16> = message.encode_utf16().collect();
    throw_units(cx, kind, &units)
}

/// QuickJS's `JS_ThrowError2` message: the formatted text through a
/// 256-byte `vsnprintf` buffer (255 bytes of its WTF-8, cut at a NUL), read
/// back by `JS_NewString`, which turns a sequence the cut split into one
/// U+FFFD.
pub(crate) fn buffered_message(units: &[u16]) -> Vec<u16> {
    const BUFFER: usize = 255;
    let mut out = Vec::with_capacity(units.len().min(BUFFER));
    let mut used = 0;
    let mut index = 0;
    while index < units.len() {
        let unit = units[index];
        if unit == 0 {
            break;
        }
        let pair = (0xd800..0xdc00).contains(&unit)
            && units
                .get(index + 1)
                .is_some_and(|next| (0xdc00..0xe000).contains(next));
        let (bytes, count) = match unit {
            _ if pair => (4, 2),
            0..0x80 => (1, 1),
            0x80..0x800 => (2, 1),
            _ => (3, 1),
        };
        if used + bytes > BUFFER {
            if used < BUFFER {
                out.push(0xfffd);
            }
            break;
        }
        out.extend_from_slice(&units[index..index + count]);
        used += bytes;
        index += count;
    }
    out
}

/// [`throw`] of a message given as UTF-16 units (a name with a lone
/// surrogate keeps it).
#[inline(never)]
pub(crate) fn throw_units(cx: &mut Cx<'_>, kind: ErrorKind, units: &[u16]) -> Throw {
    let units = buffered_message(units);
    let result = cx
        .heap
        .alloc_string_units(&units)
        .and_then(|text| new_error(cx, kind, Value::string(text)));
    match result {
        Ok(error) => Throw(error),
        Err(_) => out_of_memory(cx),
    }
}

/// A new `kind` error with `message` thrown by a script's declaration
/// prologue: its backtrace leaves the script's own frame out, as QuickJS
/// checks the declarations before it runs the script.
#[cold]
#[inline(never)]
pub(crate) fn throw_prologue(cx: &mut Cx<'_>, kind: ErrorKind, message: &str) -> Throw {
    let result = cx
        .heap
        .alloc_string(message)
        .and_then(|text| new_error_at(cx, kind, Value::string(text), None, 1));
    match result {
        Ok(error) => Throw(error),
        Err(_) => out_of_memory(cx),
    }
}

/// A new `kind` error with `message` located at `line` and `column` of
/// `file` (a module's source that no frame runs): its `stack` is the
/// location alone.
#[inline(never)]
pub(crate) fn throw_at(
    cx: &mut Cx<'_>,
    kind: ErrorKind,
    message: &str,
    file: &str,
    line: u32,
    column: u32,
) -> Throw {
    located(cx, kind, message, Location { file, line, column }, false)
}

/// A new `kind` error located at `file:line:column` (where a compile
/// error is), the running frames' backtrace after the location.
#[inline(never)]
pub(crate) fn throw_located(
    cx: &mut Cx<'_>,
    kind: ErrorKind,
    message: &str,
    file: &str,
    line: u32,
    column: u32,
) -> Throw {
    located(cx, kind, message, Location { file, line, column }, true)
}

fn located(
    cx: &mut Cx<'_>,
    kind: ErrorKind,
    message: &str,
    location: Location<'_>,
    frames: bool,
) -> Throw {
    let result = cx.heap.alloc_string(message).and_then(|text| {
        let omit = if frames { 0 } else { usize::MAX };
        new_error_at(cx, kind, Value::string(text), Some(location), omit)
    });
    match result {
        Ok(error) => Throw(error),
        Err(_) => out_of_memory(cx),
    }
}

#[inline(never)]
pub(crate) fn type_error(cx: &mut Cx<'_>, message: &str) -> Throw {
    throw(cx, ErrorKind::Type, message)
}

#[inline(never)]
pub(crate) fn range_error(cx: &mut Cx<'_>, message: &str) -> Throw {
    throw(cx, ErrorKind::Range, message)
}

#[inline(never)]
pub(crate) fn stack_overflow(cx: &mut Cx<'_>) -> Throw {
    throw(cx, ErrorKind::Internal, "stack overflow")
}

/// QuickJS's `JS_ThrowTypeErrorNotAConstructor`: a callable `value` whose
/// `name` (an own string data property, else its prototype's) reads without
/// running code is named, `" is not a constructor"` for an empty name.
#[cold]
#[inline(never)]
pub(crate) fn not_a_constructor(cx: &mut Cx<'_>, value: Value) -> Throw {
    match function_name_units(cx, value) {
        Some(mut name) => {
            name.extend(" is not a constructor".encode_utf16());
            throw_units(cx, ErrorKind::Type, &name)
        }
        None => type_error(cx, "not a constructor"),
    }
}

/// QuickJS's `JS_AtomGetStr` into its 64-byte buffer, as a `%s` of a key
/// prints: an ASCII name whole (to a NUL), any other a unit at a time
/// (UTF-8, a surrogate on its own) while 58 bytes are used at most, and
/// an index in decimal.
pub(crate) fn atom_units(heap: &crate::Heap, key: crate::PropertyKey) -> Vec<u16> {
    let units: Vec<u16> = match key.kind() {
        crate::KeyKind::String(id) => heap.string(id).repr().units().collect(),
        _ => {
            return super::property::atom_text(heap, key)
                .encode_utf16()
                .collect();
        }
    };
    let mut out = Vec::new();
    if units.iter().all(|&unit| unit < 0x80) {
        out = units;
    } else {
        const LIMIT: usize = 64 - 6;
        let mut used = 0;
        for &unit in &units {
            if used >= LIMIT {
                break;
            }
            used += match unit {
                0..0x80 => 1,
                0x80..0x800 => 2,
                _ => 3,
            };
            out.push(unit);
        }
    }
    // `%s` stops at a NUL.
    if let Some(end) = out.iter().position(|&unit| unit == 0) {
        out.truncate(end);
    }
    out
}

/// `{before}{key}{after}` with the key as [`atom_units`] prints it.
pub(crate) fn atom_message(
    heap: &crate::Heap,
    before: &str,
    key: crate::PropertyKey,
    after: &str,
) -> Vec<u16> {
    let mut units: Vec<u16> = before.encode_utf16().collect();
    units.extend(atom_units(heap, key));
    units.extend(after.encode_utf16());
    units
}

/// QuickJS's `JS_CallConstructor` failure on `value`: a non-object is
/// `not a function`, an object [`not_a_constructor`].
#[cold]
#[inline(never)]
pub(crate) fn construct_error(cx: &mut Cx<'_>, value: Value) -> Throw {
    if value.as_object().is_none() {
        return type_error(cx, "not a function");
    }
    not_a_constructor(cx, value)
}

/// QuickJS's `get_prop_string(func, "name")` on a callable `value`, cut at
/// the first NUL as `%s` prints it.
fn function_name_units(cx: &Cx<'_>, value: Value) -> Option<Vec<u16>> {
    let object = value.as_object()?;
    let heap = &cx.heap;
    if !heap
        .header(object)
        .has(crate::heap::object::flags::CALLABLE)
        || heap.object_kind(object) == crate::heap::object::ObjectKind::Proxy
    {
        return None;
    }
    let key = Names::key(cx.realm.names.name);
    let read = |object: ObjectId| -> Option<Option<Vec<u16>>> {
        let (slot, flags) = heap.lookup_own(object, key)?;
        if flags.has(PropertyFlags::ACCESSOR) {
            return Some(None);
        }
        // `get_prop_string` takes only `JS_TAG_STRING`: a rope reads as
        // absent.
        Some(
            heap.get_slot(object, slot)
                .as_string()
                .filter(|id| heap.strings.get(id.0).flat().is_some())
                .map(|id| heap.string(id).repr().units().collect()),
        )
    };
    let units = match read(object) {
        Some(found) => found,
        None => heap.prototype_of(object).and_then(read).flatten(),
    }?;
    let end = units
        .iter()
        .position(|&unit| unit == 0)
        .unwrap_or(units.len());
    Some(units[..end].to_vec())
}

/// `"{class} object expected"`, QuickJS's `JS_ThrowTypeErrorInvalidClass`.
#[cold]
#[inline(never)]
pub(crate) fn object_expected(cx: &mut Cx<'_>, class: &str) -> Throw {
    type_error(cx, &format!("{class} object expected"))
}

/// InternalError `string too long`, QuickJS's string-building overflow.
#[cold]
#[inline(never)]
pub(crate) fn string_too_long(cx: &mut Cx<'_>) -> Throw {
    throw(cx, ErrorKind::Internal, "string too long")
}

/// QuickJS's `JS_GetIterator` on `null` or `undefined`: the `@@iterator`
/// read fails.
#[cold]
#[inline(never)]
pub(crate) fn not_iterable_nullish(cx: &mut Cx<'_>, value: Value) -> Throw {
    let base = if value.is_null() { "null" } else { "undefined" };
    type_error(
        cx,
        &format!("cannot read property 'Symbol.iterator' of {base}"),
    )
}
