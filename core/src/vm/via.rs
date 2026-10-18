//! What QuickJS's frames for the `Function.prototype.call`s and `apply`s the
//! loop rewrites would show in a backtrace: the rewrite records them, a
//! bytecode callee's frame carries `frame_flags::VIA_CALL`, `VIA_APPLY` or (for
//! a chain) `VIA_CHAIN`, and a native callee runs with them listed below it.
//!
//! Kept out of `interp` so the loop's code does not move.

use super::{Cx, NativeSite};
use crate::value::Value;

/// The rewritten calls in progress and those recorded for frames.
#[derive(Default)]
pub(crate) struct ViaCalls {
    /// The `call`s and `apply`s, outermost first, of the rewrite running or
    /// of the native callee it reached; taken by that callee.
    pub(crate) calls: Vec<Value>,
    /// Those of the frames marked `frame_flags::VIA_CHAIN`: each native with the
    /// depth (index in `frames`) of the frame it follows, outermost first.
    /// An entry outlives its frame until a call at its depth or below
    /// replaces it.
    pub(crate) frames: Vec<(u32, Value)>,
}

/// A rewrite goes on past its first `call` or `apply`, `first`, or reaches
/// a callee that is not bytecode.
#[cold]
#[inline(never)]
pub(super) fn begin(cx: &mut Cx<'_>, first: Value) {
    let calls = &mut cx.heap.vm.via.calls;
    calls.clear();
    calls.push(first);
}

/// The rewrite went through `function` (a `call` or an `apply`).
#[cold]
#[inline(never)]
pub(super) fn record(cx: &mut Cx<'_>, function: Value) {
    cx.heap.vm.via.calls.push(function);
}

/// The rewrite threw: nothing is listed.
#[cold]
#[inline(never)]
pub(super) fn abandon(cx: &mut Cx<'_>) {
    cx.heap.vm.via.calls.clear();
}

/// The rewrite reached a bytecode callee, whose frame (at the current
/// depth) is about to be entered with `frame_flags::VIA_CHAIN`.
#[inline(never)]
pub(super) fn enter_bytecode(cx: &mut Cx<'_>) {
    let depth = cx.heap.vm.frames.len() as u32;
    let via = &mut *cx.heap.vm.via;
    let keep = via.frames.partition_point(|&(at, _)| at < depth);
    via.frames.truncate(keep);
    via.frames
        .extend(via.calls.drain(..).map(|function| (depth, function)));
}

/// Lists the recorded natives (if any) for a native callee called at `pc`;
/// returns how many, for [`unlist`].
#[inline]
pub(super) fn list(cx: &mut Cx<'_>, pc: usize) -> usize {
    if cx.heap.vm.via.calls.is_empty() {
        return 0;
    }
    list_recorded(cx, pc)
}

#[cold]
#[inline(never)]
fn list_recorded(cx: &mut Cx<'_>, pc: usize) -> usize {
    let depth = cx.heap.vm.frames.len() as u32;
    let vm = &mut cx.heap.vm;
    let count = vm.via.calls.len();
    vm.natives
        .extend(vm.via.calls.drain(..).map(|callee| NativeSite {
            depth,
            pc: pc as u32,
            callee,
        }));
    count
}

/// Ends [`list`].
#[inline]
pub(super) fn unlist(cx: &mut Cx<'_>, count: usize) {
    if count == 0 {
        return;
    }
    let keep = cx.heap.vm.natives.len() - count;
    cx.heap.vm.natives.truncate(keep);
}
