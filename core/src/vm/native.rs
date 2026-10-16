//! The native ABI: leaf host functions that read their receiver and
//! arguments from the register file by index (phase 2, section 4.3). The
//! natives themselves are the realm's builtins (`crate::builtins`). Natives
//! that call back into JavaScript for their own work are phase 4's
//! Step/Resume machines; the ones here only convert their arguments, which
//! may call a user `toString` through the re-entrant [`call`](super::call).

use super::{Cx, Throw};
use crate::builtins::Native;
use crate::value::Value;

/// Where a native finds its receiver (register `this`) and its arguments
/// (`argc` registers from `args`), and whether `new` called it.
#[derive(Clone, Copy, Debug)]
pub(crate) struct CallSite {
    pub this: usize,
    pub args: usize,
    pub argc: usize,
    pub construct: bool,
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
