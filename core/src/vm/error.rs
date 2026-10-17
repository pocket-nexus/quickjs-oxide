//! Error objects the loop throws. Phase 2 has no `Error` builtins beyond the
//! constructors and prototypes `instanceof` needs: an error is an ordinary
//! object with the kind's prototype and an own `message`; `name` lives on the
//! prototype.

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

/// A new error object of `kind`; `message` must be rooted (or not a heap
/// value) by the caller.
pub(crate) fn new_error(
    cx: &mut Cx<'_>,
    kind: ErrorKind,
    message: Value,
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
        add_stack(cx, object).map(|()| Value::object(object))
    });
    cx.heap.roots().truncate(mark);
    result
}

/// Most frames a backtrace lists.
const BACKTRACE_FRAMES: usize = 64;

/// QuickJS's `build_backtrace`: the bytecode frames, innermost first, as
/// `    at name (file:line:column)` lines in the own `stack` property of
/// `object`, which the caller roots. A frame whose current instruction is
/// unknown (below a re-entry from Rust) has no position.
pub(crate) fn add_stack(cx: &mut Cx<'_>, object: ObjectId) -> Result<(), HeapError> {
    let vm = &cx.heap.vm;
    let mut text = String::new();
    let (depth, site) = vm.native_site;
    let mut pc = (depth as usize == vm.frames.len()).then_some(site);
    for frame in vm.frames.iter().rev().take(BACKTRACE_FRAMES) {
        let debug = &cx.code.get(frame.unit).debug;
        let name = if debug.name.is_empty() {
            "<anonymous>"
        } else {
            &debug.name
        };
        let file = debug.filename.as_deref().unwrap_or("");
        let _ = match pc.and_then(|pc| debug.position(pc)) {
            Some((line, column)) => writeln!(text, "    at {name} ({file}:{line}:{column})"),
            None => writeln!(text, "    at {name} ({file})"),
        };
        pc = (frame.flags & frame_flags::ENTRY == 0 && frame.return_pc > 0)
            .then(|| frame.return_pc - 1);
    }
    let text = cx.heap.alloc_string(&text)?;
    let key = Names::key(cx.realm.names.stack);
    cx.heap.add_property(
        object,
        key,
        PropertyFlags::data(true, false, true),
        Value::string(text),
    )
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

/// A new `kind` error with `message`.
#[inline(never)]
pub(crate) fn throw(cx: &mut Cx<'_>, kind: ErrorKind, message: &str) -> Throw {
    let result = cx
        .heap
        .alloc_string(message)
        .and_then(|text| new_error(cx, kind, Value::string(text)));
    match result {
        Ok(error) => Throw(error),
        Err(_) => out_of_memory(cx),
    }
}

/// A new `kind` error with `message` located at `line` and `column` of
/// `file` (a module's source that no frame runs), its `stack` naming them.
#[inline(never)]
pub(crate) fn throw_at(
    cx: &mut Cx<'_>,
    kind: ErrorKind,
    message: &str,
    file: &str,
    line: u32,
    column: u32,
) -> Throw {
    let Throw(error) = throw(cx, kind, message);
    let Some(object) = error.as_object() else {
        return Throw(error);
    };
    let mark = cx.heap.roots().mark();
    cx.heap.roots().push(error);
    let stack = cx
        .heap
        .alloc_string(&format!("    at {file}:{line}:{column}\n"));
    cx.heap.roots().truncate(mark);
    let key = Names::key(cx.realm.names.stack);
    if let (Ok(stack), Some((slot, _))) = (stack, cx.heap.lookup_own(object, key)) {
        cx.heap.set_slot(object, slot, Value::string(stack));
    }
    Throw(error)
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
