//! The Test262 host object `$262`: `createRealm` throws (realms are dropped),
//! `gc` collects, `global` is the global object; `evalScript`,
//! `detachArrayBuffer` and `agent` arrive with the compile host and buffers.

use super::{Native, define, plain_object};
use crate::heap::HeapError;
use crate::runtime::Intrinsic;
use crate::shape::PropertyFlags;
use crate::value::Value;
use crate::vm::native::CallSite;
use crate::vm::{Cx, Throw, error};

/// Defines `$262` on the global object.
pub(crate) fn install(cx: &mut Cx<'_>) -> Result<(), HeapError> {
    let mark = cx.heap.roots().mark();
    let result = (|| {
        let prototype = cx.realm.intrinsic(Intrinsic::ObjectPrototype);
        let host = plain_object(cx, Some(prototype), 8)?;
        let all = PropertyFlags::DEFAULT;
        // Pinned QuickJS's order; every property writable, enumerable and
        // configurable.
        for native in [
            Native::Test262DetachArrayBuffer,
            Native::Test262EvalScript,
            Native::Test262CodePointRange,
        ] {
            let function = super::native_function(cx, native)?;
            define(cx, host, native.js_name(), Value::object(function), all)?;
        }
        let global = cx.realm.global();
        define(cx, host, "global", Value::object(global), all)?;
        let function = super::native_function(cx, Native::Test262CreateRealm)?;
        define(cx, host, "createRealm", Value::object(function), all)?;
        let function_prototype = cx.realm.intrinsic(Intrinsic::FunctionPrototype);
        let dda = cx.heap.alloc_internal(
            crate::heap::object::ObjectKind::Builtin,
            crate::heap::object::InternalTag::HtmlDda,
            Some(function_prototype),
            0,
            &[],
            Default::default(),
            crate::heap::object::flags::CALLABLE,
        )?;
        cx.heap.roots().push(Value::object(dda));
        cx.heap.html_dda = true;
        // QuickJS's `IsHTMLDDA` is a function named so, of length 0.
        let name = cx.heap.intern("IsHTMLDDA")?;
        cx.heap.roots().push(Value::string(name));
        let key = cx.heap.property_key("name")?;
        cx.heap.add_property(
            dda,
            key,
            crate::PropertyFlags::data(false, false, true),
            Value::string(name),
        )?;
        define(cx, host, "IsHTMLDDA", Value::object(dda), all)?;
        let function = super::native_function(cx, Native::Test262Gc)?;
        define(cx, host, "gc", Value::object(function), all)?;
        super::agent::install(cx, host, true)?;
        define(cx, global, "$262", Value::object(host), all)
    })();
    cx.heap.roots().truncate(mark);
    result
}

/// `$262.codePointRange(start, end)`: the string of code points
/// `start..end`, built natively (the harness's fallback is slow).
pub(super) fn code_point_range(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let (start, end) = (site.arg(cx, 0), site.arg(cx, 1));
    let start = crate::vm::number::to_uint32(crate::vm::ops::to_number(cx, start)?);
    let end = crate::vm::number::to_uint32(crate::vm::ops::to_number(cx, end)?);
    let mut text = Vec::new();
    for code in start..end.min(0x110000) {
        if code < 0x10000 {
            text.push(code as u16);
        } else {
            let adjusted = code - 0x10000;
            text.push(0xD800 | (adjusted >> 10) as u16);
            text.push(0xDC00 | (adjusted & 0x3FF) as u16);
        }
    }
    super::string_units(cx, &text)
}

pub(super) fn create_realm(cx: &mut Cx<'_>, _: CallSite) -> Result<Value, Throw> {
    Err(error::type_error(cx, "realms are not supported"))
}

pub(super) fn gc(cx: &mut Cx<'_>, _: CallSite) -> Result<Value, Throw> {
    cx.heap.collect();
    Ok(Value::UNDEFINED)
}

pub(super) fn eval_script(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    crate::vm::compile::eval_script(cx, site)
}

pub(super) fn detach_array_buffer(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    super::detach_array_buffer(cx, site)
}
