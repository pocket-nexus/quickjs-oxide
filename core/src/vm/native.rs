//! The phase 2 native ABI: leaf host functions that read their arguments
//! from the register file by index (section 4.3). Natives that call back into
//! JavaScript are phase 4's Step/Resume machines; the ones here only convert
//! their arguments, which may call a user `toString` through the re-entrant
//! [`call`](super::call).

use std::io::Write;

use super::error::{self, ErrorKind};
use super::{Cx, Output, Throw, ops};
use crate::heap::HeapError;
use crate::heap::object::NONE;
use crate::runtime::{Intrinsic, Names};
use crate::shape::PropertyFlags;
use crate::value::{ObjectId, Value};

/// A host function, stored in its function record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Native {
    /// qjs's `print`: the arguments' strings joined by a space, then a
    /// newline.
    Print,
    /// `Error`, `TypeError`, …: called or constructed, a new error.
    ErrorConstructor(ErrorKind),
}

impl Native {
    pub(crate) const fn index(self) -> u32 {
        match self {
            Native::Print => 0,
            Native::ErrorConstructor(kind) => 1 + kind.index(),
        }
    }

    pub(crate) fn from_index(index: u32) -> Option<Native> {
        match index {
            NONE => None,
            0 => Some(Native::Print),
            _ => ErrorKind::from_index(index - 1).map(Native::ErrorConstructor),
        }
    }

    pub const fn is_constructor(self) -> bool {
        matches!(self, Native::ErrorConstructor(_))
    }
}

/// Where a native finds its arguments: `argc` registers from `args`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct CallSite {
    pub args: usize,
    pub argc: usize,
}

impl CallSite {
    #[inline]
    pub fn arg(&self, cx: &Cx<'_>, index: usize) -> Value {
        if index < self.argc {
            cx.heap.vm.registers[self.args + index]
        } else {
            Value::UNDEFINED
        }
    }
}

#[inline(never)]
pub(crate) fn call(cx: &mut Cx<'_>, native: Native, site: CallSite) -> Result<Value, Throw> {
    match native {
        Native::Print => print(cx, site),
        Native::ErrorConstructor(kind) => {
            let message = site.arg(cx, 0);
            let message = if message.is_undefined() {
                message
            } else {
                ops::to_string(cx, message)?
            };
            error::new_error(cx, kind, message).map_err(|error| error::heap_error(cx, error))
        }
    }
}

fn print(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let mut line = String::new();
    for index in 0..site.argc {
        if index > 0 {
            line.push(' ');
        }
        let argument = site.arg(cx, index);
        line.push_str(&ops::display(cx, argument)?);
    }
    line.push('\n');
    match &mut cx.host.output {
        Output::Stdout => {
            let mut stdout = std::io::stdout().lock();
            let _ = stdout.write_all(line.as_bytes());
            let _ = stdout.flush();
        }
        Output::Capture(text) => text.push_str(&line),
    }
    Ok(Value::UNDEFINED)
}

fn hidden() -> PropertyFlags {
    PropertyFlags::data(true, false, true)
}

fn define(
    cx: &mut Cx<'_>,
    object: ObjectId,
    name: &str,
    value: Value,
    flags: PropertyFlags,
) -> Result<(), HeapError> {
    let mark = cx.heap.roots().mark();
    cx.heap.roots().push(value);
    let result = cx
        .heap
        .property_key(name)
        .and_then(|key| cx.heap.add_property(object, key, flags, value));
    cx.heap.roots().truncate(mark);
    result
}

/// A rooted native function object.
fn native_function(cx: &mut Cx<'_>, native: Native) -> Result<ObjectId, HeapError> {
    let prototype = cx.realm.intrinsic(Intrinsic::FunctionPrototype);
    let function = cx
        .heap
        .alloc_native_function(Some(prototype), native.index())?;
    cx.heap.roots().push(Value::object(function));
    Ok(function)
}

/// The globals phase 2 has: the error constructors and prototypes,
/// `globalThis`, `undefined`, `NaN` and `Infinity`. Returns the
/// preallocated out-of-memory error.
pub(crate) fn install_globals(cx: &mut Cx<'_>) -> Result<ObjectId, HeapError> {
    let mark = cx.heap.roots().mark();
    let result = install_globals_rooted(cx);
    cx.heap.roots().truncate(mark);
    result
}

fn install_globals_rooted(cx: &mut Cx<'_>) -> Result<ObjectId, HeapError> {
    let global = cx.realm.global();
    let constant = PropertyFlags::data(false, false, false);
    define(cx, global, "globalThis", Value::object(global), hidden())?;
    define(cx, global, "undefined", Value::UNDEFINED, constant)?;
    define(cx, global, "NaN", Value::number(f64::NAN), constant)?;
    define(
        cx,
        global,
        "Infinity",
        Value::number(f64::INFINITY),
        constant,
    )?;
    let names = cx.realm.names;
    for kind in ErrorKind::ALL {
        let constructor = native_function(cx, Native::ErrorConstructor(kind))?;
        let prototype = cx.realm.intrinsic(kind.prototype());
        let key = Names::key(names.prototype);
        cx.heap.add_property(
            constructor,
            key,
            PropertyFlags::data(false, false, false),
            Value::object(prototype),
        )?;
        let key = Names::key(names.constructor);
        cx.heap
            .add_property(prototype, key, hidden(), Value::object(constructor))?;
        let name = cx.heap.intern(kind.name())?;
        cx.heap.add_property(
            prototype,
            Names::key(names.name),
            hidden(),
            Value::string(name),
        )?;
        if kind == ErrorKind::Error {
            let empty = Value::string(names.empty);
            cx.heap
                .add_property(prototype, Names::key(names.message), hidden(), empty)?;
        }
        define(
            cx,
            global,
            kind.name(),
            Value::object(constructor),
            hidden(),
        )?;
    }
    let message = cx.heap.intern("out of memory")?;
    let error = error::new_error(cx, ErrorKind::Internal, Value::string(message))?;
    Ok(error.as_object().unwrap())
}

/// Defines `print` on the global object.
pub(crate) fn install_print(cx: &mut Cx<'_>) -> Result<(), HeapError> {
    let mark = cx.heap.roots().mark();
    let result = native_function(cx, Native::Print).and_then(|print| {
        let global = cx.realm.global();
        define(cx, global, "print", Value::object(print), hidden())
    });
    cx.heap.roots().truncate(mark);
    result
}
