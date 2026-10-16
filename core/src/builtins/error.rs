//! The error constructors and `Error.prototype.toString`.

use super::{Native, install_constructor, methods};
use crate::heap::HeapError;
use crate::runtime::Names;
use crate::shape::PropertyFlags;
use crate::value::Value;
use crate::vm::error::{self as vm_error, ErrorKind};
use crate::vm::native::CallSite;
use crate::vm::{Cx, Throw, ops};

pub(super) fn install(cx: &mut Cx<'_>) -> Result<(), HeapError> {
    let names = cx.realm.names;
    let hidden = PropertyFlags::data(true, false, true);
    for kind in ErrorKind::ALL {
        let prototype = cx.realm.intrinsic(kind.prototype());
        install_constructor(cx, kind.name(), Native::error(kind), prototype)?;
        // Atoms are strong roots: the name needs no rooting.
        let name = cx.heap.intern(kind.name())?;
        cx.heap.add_property(
            prototype,
            Names::key(names.name),
            hidden,
            Value::string(name),
        )?;
        if kind == ErrorKind::Error {
            let empty = Value::string(names.empty);
            cx.heap
                .add_property(prototype, Names::key(names.message), hidden, empty)?;
            methods(cx, prototype, &[("toString", Native::ErrorToString)])?;
        }
    }
    Ok(())
}

fn construct(cx: &mut Cx<'_>, site: CallSite, kind: ErrorKind) -> Result<Value, Throw> {
    let message = site.arg(cx, 0);
    let message = if message.is_undefined() {
        message
    } else {
        ops::to_string(cx, message)?
    };
    vm_error::new_error(cx, kind, message).map_err(|error| vm_error::heap_error(cx, error))
}

macro_rules! constructors {
    ($($name:ident => $kind:ident;)*) => {
        $(
            pub(super) fn $name(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
                construct(cx, site, ErrorKind::$kind)
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
}

/// `Error.prototype.toString`.
pub(super) fn to_string(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let this = site.this(cx);
    let Some(object) = this.as_object() else {
        return Err(vm_error::type_error(cx, "not an object"));
    };
    ops::error_text(cx, object)
}
