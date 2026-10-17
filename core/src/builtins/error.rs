//! The error constructors (`Error`, the native errors, `AggregateError`)
//! with the `cause` option, `Error.prototype.toString` and `Error.isError`.

use super::{Native, install_constructor, methods};
use crate::heap::HeapError;
use crate::heap::object::{ObjectKind, flags};
use crate::runtime::Names;
use crate::shape::PropertyFlags;
use crate::value::{ObjectId, Value};
use crate::vm::error::{self as vm_error, ErrorKind};
use crate::vm::native::CallSite;
use crate::vm::object as internal;
use crate::vm::{Cx, Throw, iter, ops};

pub(super) fn install(cx: &mut Cx<'_>) -> Result<(), HeapError> {
    let names = cx.realm.names;
    let hidden = PropertyFlags::data(true, false, true);
    let mut error_constructor = None;
    for kind in ErrorKind::ALL {
        let prototype = cx.realm.intrinsic(kind.prototype());
        let constructor = install_constructor(cx, Native::error(kind), prototype)?;
        if let Some(error) = error_constructor {
            cx.heap.set_prototype(constructor, Some(error))?;
        } else {
            error_constructor = Some(constructor);
            methods(cx, constructor, &[Native::ErrorIsError])?;
        }
        // Atoms are strong roots: the name needs no rooting.
        let name = cx.heap.intern(kind.name())?;
        cx.heap.add_property(
            prototype,
            Names::key(names.name),
            hidden,
            Value::string(name),
        )?;
        let empty = Value::string(names.empty);
        cx.heap
            .add_property(prototype, Names::key(names.message), hidden, empty)?;
        if kind == ErrorKind::Error {
            methods(cx, prototype, &[Native::ErrorToString])?;
        }
    }
    Ok(())
}

/// `OrdinaryCreateFromConstructor(newTarget, %NativeError.prototype%)` with
/// the error flag, then the message and the cause.
fn construct(
    cx: &mut Cx<'_>,
    site: &CallSite,
    kind: ErrorKind,
    message: Value,
    options: Value,
) -> Result<ObjectId, Throw> {
    let new_target = if site.construct {
        site.new_target
    } else {
        site.function
    };
    let prototype = internal::prototype_from_constructor(cx, new_target, kind.prototype())?;
    let object = cx
        .heap
        .alloc_object(ObjectKind::Ordinary, Some(prototype), 2)
        .map_err(|error| vm_error::heap_error(cx, error))?;
    cx.heap.set_object_flag(object, flags::ERROR);
    cx.heap.roots().push(Value::object(object));
    let names = cx.realm.names;
    let hidden = PropertyFlags::data(true, false, true);
    if !message.is_undefined() {
        let message = ops::to_string(cx, message)?;
        cx.heap.roots().push(message);
        cx.heap
            .add_property(object, Names::key(names.message), hidden, message)
            .map_err(|error| vm_error::heap_error(cx, error))?;
    }
    if let Some(options) = options.as_object()
        && internal::has_property(cx, options, Names::key(names.cause))?
    {
        let cause = internal::get_property(cx, options, Names::key(names.cause))?;
        cx.heap.roots().push(cause);
        cx.heap
            .add_property(object, Names::key(names.cause), hidden, cause)
            .map_err(|error| vm_error::heap_error(cx, error))?;
    }
    Ok(object)
}

fn native_error(cx: &mut Cx<'_>, site: CallSite, kind: ErrorKind) -> Result<Value, Throw> {
    let (message, options) = (site.arg(cx, 0), site.arg(cx, 1));
    construct(cx, &site, kind, message, options).map(Value::object)
}

macro_rules! constructors {
    ($($name:ident => $kind:ident;)*) => {
        $(
            pub(super) fn $name(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
                native_error(cx, site, ErrorKind::$kind)
            }
        )*
    };
}

constructors! {
    error => Error;
    eval_error => Eval;
    range_error => Range;
    reference_error => Reference;
    syntax_error => Syntax;
    type_error => Type;
    internal_error => Internal;
    uri_error => Uri;
}

/// `AggregateError(errors, message, options)`.
pub(super) fn aggregate_error(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let (errors, message, options) = (site.arg(cx, 0), site.arg(cx, 1), site.arg(cx, 2));
    let object = construct(cx, &site, ErrorKind::Aggregate, message, options)?;
    let list = iter::collect(cx, errors)?;
    let array = internal::array_from_list(cx, &list)?;
    let key = Names::key(cx.realm.names.errors);
    cx.heap
        .add_property(
            object,
            key,
            PropertyFlags::data(true, false, true),
            Value::object(array),
        )
        .map_err(|error| vm_error::heap_error(cx, error))?;
    Ok(Value::object(object))
}

/// `Error.prototype.toString`.
pub(super) fn to_string(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let this = site.this(cx);
    let Some(object) = this.as_object() else {
        return Err(vm_error::type_error(cx, "not an object"));
    };
    ops::error_text(cx, object)
}

/// `Error.isError(value)`: whether it is an error object (a proxy is not).
pub(super) fn is_error(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let value = site.arg(cx, 0);
    Ok(Value::bool(value.as_object().is_some_and(|object| {
        cx.heap.header(object).has(flags::ERROR)
    })))
}
