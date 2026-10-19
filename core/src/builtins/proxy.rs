//! `Proxy` and `Proxy.revocable`; the internal methods are in `vm::proxy`.

use super::{Native, define, methods, native_closure, native_data};
use crate::heap::HeapError;
use crate::heap::object::ObjectKind;
use crate::shape::PropertyFlags;
use crate::value::Value;
use crate::vm::native::CallSite;
use crate::vm::{Cx, Throw, error, proxy};

pub(super) fn install(cx: &mut Cx<'_>) -> Result<(), HeapError> {
    let function = super::native_function(cx, Native::ProxyConstructor)?;
    methods(cx, function, &[Native::ProxyRevocable])?;
    cx.set_intrinsic(crate::Intrinsic::Proxy, function);
    let global = cx.realm.global();
    define(
        cx,
        global,
        "Proxy",
        Value::object(function),
        super::hidden(),
    )
}

/// `new Proxy(target, handler)`; calling it without `new` throws.
pub(super) fn constructor(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    if !site.construct {
        return Err(error::type_error(cx, "must be called with new"));
    }
    let (target, handler) = (site.arg(cx, 0), site.arg(cx, 1));
    proxy::create(cx, target, handler).map(Value::object)
}

/// `Proxy.revocable(target, handler)`: `{ proxy, revoke }`.
pub(super) fn revocable(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let (target, handler) = (site.arg(cx, 0), site.arg(cx, 1));
    let proxy = proxy::create(cx, target, handler)?;
    cx.heap.roots().push(Value::object(proxy));
    let data = native_data(cx, &[Value::object(proxy)])?;
    let revoke = native_closure(cx, Native::ProxyRevoke, data)?;
    cx.heap.roots().push(Value::object(revoke));
    let result = super::new_plain(cx, 2)?;
    cx.heap.roots().push(Value::object(result));
    let proxy_key = cx
        .heap
        .property_key_pinned("proxy")
        .map_err(|error| error::heap_error(cx, error))?;
    let revoke_key = cx
        .heap
        .property_key_pinned("revoke")
        .map_err(|error| error::heap_error(cx, error))?;
    let heap = &mut *cx.heap;
    heap.add_property(
        result,
        proxy_key,
        PropertyFlags::DEFAULT,
        Value::object(proxy),
    )
    .and_then(|()| {
        heap.add_property(
            result,
            revoke_key,
            PropertyFlags::DEFAULT,
            Value::object(revoke),
        )
    })
    .map_err(|error| error::heap_error(cx, error))?;
    Ok(Value::object(result))
}

/// A revoker: revokes its proxy once. As in QuickJS, the proxy keeps its
/// target and handler.
pub(super) fn revoke(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let proxy = site.data(cx, 0);
    if let Some(proxy) = proxy.as_object() {
        debug_assert_eq!(cx.heap.object_kind(proxy), ObjectKind::Proxy);
        proxy::revoke(cx.heap, proxy);
        site.set_data(cx, 0, Value::NULL);
    }
    Ok(Value::UNDEFINED)
}
