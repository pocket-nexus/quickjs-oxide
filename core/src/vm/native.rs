//! The native ABI: host functions that read their receiver and arguments from
//! the register file by index and call back into JavaScript through the
//! re-entrant [`call`](super::call) and [`construct`](super::construct) as
//! straight-line Rust. A native roots what it holds across an allocation or a
//! call on the root stack, which is cut back when it returns. The natives
//! themselves are the realm's builtins (`crate::builtins`).

use std::rc::Rc;

use super::{Cx, Throw};
use crate::builtins::Native;
use crate::embed::NativeCx;
use crate::heap::object::InternalData;
use crate::value::Value;

/// Where a native finds its receiver (register `this`) and its arguments
/// (`argc` registers from `args`), whether `new` called it and with which
/// `new.target`, and the function object called (whose record holds a
/// native closure's data).
#[derive(Clone, Copy, Debug)]
pub(crate) struct CallSite {
    pub this: usize,
    pub args: usize,
    pub argc: usize,
    pub construct: bool,
    pub function: Value,
    pub new_target: Value,
}

impl CallSite {
    #[inline]
    pub fn this(&self, cx: &Cx<'_>) -> Value {
        cx.heap.vm.registers[self.this]
    }

    #[inline]
    pub fn arg(&self, cx: &Cx<'_>, index: usize) -> Value {
        if index < self.argc {
            cx.heap.vm.registers[self.args + index]
        } else {
            Value::UNDEFINED
        }
    }

    /// The arguments as a slice of the register file.
    #[inline]
    pub fn args<'a>(&self, cx: &'a Cx<'_>) -> &'a [Value] {
        &cx.heap.vm.registers[self.args..self.args + self.argc]
    }

    /// The arguments copied out (they stay rooted in their registers).
    pub fn arg_list(&self, cx: &Cx<'_>) -> Vec<Value> {
        self.args(cx).to_vec()
    }

    /// Internal slot `slot` of the called native closure's data object.
    pub fn data(&self, cx: &Cx<'_>, slot: usize) -> Value {
        let function = self.function.as_object().expect("a native closure");
        let data = cx
            .heap
            .function_scope(function)
            .expect("a native closure's data");
        cx.heap.internal_slot(data.object(), slot)
    }

    /// Sets internal slot `slot` of the called native closure's data object.
    pub fn set_data(&self, cx: &mut Cx<'_>, slot: usize, value: Value) {
        let function = self.function.as_object().expect("a native closure");
        let data = cx
            .heap
            .function_scope(function)
            .expect("a native closure's data");
        cx.heap.set_internal_slot(data.object(), slot, value);
    }
}

/// Calls a native. A native may root what it holds across allocations on
/// the root stack freely: the stack is cut back when it returns.
#[inline(never)]
pub(crate) fn call(cx: &mut Cx<'_>, native: Native, site: CallSite) -> Result<Value, Throw> {
    let mark = cx.heap.roots().mark();
    let result = (native.function())(cx, site);
    cx.heap.roots().truncate(mark);
    result
}

/// A host function's body: the embedder's Rust code, called with the
/// engine's view and the call.
pub type HostFn = dyn Fn(&mut NativeCx<'_>, NativeCall) -> Result<Value, Throw>;

/// `Rc<HostFn>` with a `Debug` that names the kind and not the closure.
#[derive(Clone)]
pub struct HostBody(pub Rc<HostFn>);

impl std::fmt::Debug for HostBody {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("HostBody")
    }
}

/// Where a host function finds its receiver, arguments, `new.target` and
/// captured values, read through a [`NativeCx`]. The arguments stay in the
/// register file, rooted, while the body runs.
#[derive(Clone, Copy, Debug)]
pub struct NativeCall(CallSite);

impl NativeCall {
    pub fn this(&self, cx: &NativeCx<'_>) -> Value {
        cx.heap.vm.registers[self.0.this]
    }

    /// Argument `index`; `undefined` past [`NativeCall::argc`].
    pub fn arg(&self, cx: &NativeCx<'_>, index: usize) -> Value {
        if index < self.0.argc {
            cx.heap.vm.registers[self.0.args + index]
        } else {
            Value::UNDEFINED
        }
    }

    pub fn argc(&self) -> usize {
        self.0.argc
    }

    pub fn args<'a>(&self, cx: &'a NativeCx<'_>) -> &'a [Value] {
        &cx.heap.vm.registers[self.0.args..self.0.args + self.0.argc]
    }

    /// `new.target`; `undefined` for a call.
    pub fn new_target(&self) -> Value {
        self.0.new_target
    }

    pub fn is_construct(&self) -> bool {
        self.0.construct
    }

    /// The function object called.
    pub fn callee(&self) -> Value {
        self.0.function
    }

    /// Captured value `slot`.
    pub fn data(&self, cx: &NativeCx<'_>, slot: usize) -> Value {
        cx.heap.internal_slot(self.record(cx).object(), slot)
    }

    pub fn set_data(&self, cx: &mut NativeCx<'_>, slot: usize, value: Value) {
        let record = self.record(cx);
        cx.heap.set_internal_slot(record.object(), slot, value);
    }

    /// The number of captured values.
    pub fn data_len(&self, cx: &NativeCx<'_>) -> usize {
        cx.heap.internal(self.record(cx).object()).slots.len()
    }

    fn record(&self, cx: &NativeCx<'_>) -> crate::ScopeId {
        let function = self.0.function.as_object().expect("a host function");
        cx.heap
            .function_scope(function)
            .expect("a host closure's data")
    }
}

/// The native of [`Native::HostFunction`] and [`Native::HostConstructor`]:
/// calls the body in the function's data record through a clone of its
/// `Rc`, so the body may re-enter the function.
pub(crate) fn host_call(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let function = site.function.as_object().expect("a host function");
    let data = cx
        .heap
        .function_scope(function)
        .expect("a host closure's data");
    let InternalData::HostFn(body) = &cx.heap.internal(data.object()).data else {
        unreachable!("a host function's data holds its body");
    };
    let body = Rc::clone(&body.0);
    // The body may call back in (and so recurse through itself): it counts
    // as a re-entry against the native-stack budget, as a C function's call
    // does in QuickJS.
    if !cx.heap.vm.enter_native() {
        return Err(super::error::stack_overflow(cx));
    }
    let result = body(&mut NativeCx::from_cx(cx), NativeCall(site));
    cx.heap.vm.leave_native();
    result
}
