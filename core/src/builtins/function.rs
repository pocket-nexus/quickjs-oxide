//! `Function`: `%Function.prototype%` (callable, returning `undefined`) and
//! its `call`, `apply` and `toString`. `new Function` compiles source, which
//! is phase 4's.

use super::{Native, install_constructor, methods, string_text, unsupported};
use crate::heap::HeapError;
use crate::runtime::Intrinsic;
use crate::value::Value;
use crate::vm::native::CallSite;
use crate::vm::{Cx, Throw, arguments, error, ops};

pub(super) fn install(cx: &mut Cx<'_>) -> Result<(), HeapError> {
    let prototype = cx.realm.intrinsic(Intrinsic::FunctionPrototype);
    install_constructor(cx, "Function", Native::FunctionConstructor, prototype)?;
    methods(
        cx,
        prototype,
        &[
            ("call", Native::FunctionCall),
            ("apply", Native::FunctionApply),
            ("toString", Native::FunctionToString),
        ],
    )
}

pub(super) fn prototype(_: &mut Cx<'_>, _: CallSite) -> Result<Value, Throw> {
    Ok(Value::UNDEFINED)
}

pub(super) fn constructor(cx: &mut Cx<'_>, _: CallSite) -> Result<Value, Throw> {
    Err(unsupported(cx, "new Function"))
}

/// `Function.prototype.toString`. Source text is phase 4's: every function
/// reads as QuickJS prints a native one.
pub(super) fn to_string(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let this = site.this(cx);
    if !ops::is_callable(cx.heap, this) {
        return Err(error::type_error(cx, "not a function"));
    }
    string_text(cx, "function () {\n    [native code]\n}")
}

/// `f.call(thisArg, …)` outside a rewritten call site.
pub(super) fn call(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let function = site.this(cx);
    let this = site.arg(cx, 0);
    let args: Vec<Value> = (1..site.argc).map(|index| site.arg(cx, index)).collect();
    crate::vm::call(cx, function, this, &args)
}

/// `f.apply(thisArg, list)` outside a rewritten call site.
pub(super) fn apply(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let function = site.this(cx);
    let this = site.arg(cx, 0);
    let list = site.arg(cx, 1);
    let mark = cx.heap.roots().mark();
    let result = arguments::list_from_array_like(cx, list).and_then(|start| {
        let args = cx.heap.roots().values()[start..].to_vec();
        crate::vm::call(cx, function, this, &args)
    });
    cx.heap.roots().truncate(mark);
    result
}
