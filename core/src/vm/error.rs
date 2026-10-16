//! Error objects the loop throws. Phase 2 has no `Error` builtins beyond the
//! constructors and prototypes `instanceof` needs: an error is an ordinary
//! object with the kind's prototype and an own `message`; `name` lives on the
//! prototype.

use super::{Cx, Throw};
use crate::heap::HeapError;
use crate::runtime::{Intrinsic, Names, Realm};
use crate::shape::PropertyFlags;
use crate::value::Value;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorKind {
    Error,
    Eval,
    Range,
    Reference,
    Syntax,
    Type,
    Internal,
}

impl ErrorKind {
    pub const ALL: [ErrorKind; 7] = [
        ErrorKind::Error,
        ErrorKind::Eval,
        ErrorKind::Range,
        ErrorKind::Reference,
        ErrorKind::Syntax,
        ErrorKind::Type,
        ErrorKind::Internal,
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
        }
    }

    pub(crate) const fn index(self) -> u32 {
        self as u32
    }

    pub(crate) fn from_index(index: u32) -> Option<ErrorKind> {
        ErrorKind::ALL.get(index as usize).copied()
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
        if !message.is_undefined() {
            let key = Names::key(cx.realm.names.message);
            heap.add_property(object, key, PropertyFlags::data(true, false, true), message)?;
        }
        Ok(Value::object(object))
    })();
    heap.roots().truncate(mark);
    result
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
