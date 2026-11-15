//! Waitable shared-memory addresses, and the waiter the host installs.
//!
//! `Atomics.wait` is the one operation in the language that blocks a thread, and
//! the core has no threads. So the core knows the questions — a location token,
//! an outcome, a process-wide order for the non-blocking byte leaves — and the
//! host brings the mechanism: a mutex, condition variables, a monotonic clock.
//!
//! A location deliberately carries no runtime, heap, object or backing handle.
//! It is a stable shared-backing identity plus an absolute byte offset, so a
//! blocked thread keeps its backing alive through its own access token instead
//! of creating a process-global heap edge.

/// One waitable shared-memory address.
///
/// QuickJS keys waiters by backing allocation and absolute byte address. The
/// TypedArray wrapper, element kind, and element width are intentionally not
/// part of the key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WaitLocation {
    pub backing_id: u64,
    pub byte_offset: usize,
}

impl WaitLocation {
    pub const fn new(backing_id: u64, byte_offset: usize) -> Self {
        Self {
            backing_id,
            byte_offset,
        }
    }
}

/// Observable completion of a synchronous wait.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WaitOutcome {
    NotEqual,
    Ok,
    TimedOut,
}

/// The blocking mechanism, installed by a host that has threads.
///
/// Every method holds the implementation's process-wide coordinator, so the
/// non-blocking byte leaves run in one order across threads and a waiter can
/// not slip in between a comparison and its registration. A host with no
/// threads installs none: `Atomics.wait` then has nothing to wait with and
/// throws, and `Atomics.notify` wakes nobody.
pub trait Waiter: Send + Sync {
    /// Run one non-blocking atomic byte leaf in the process-wide
    /// sequentially-consistent order.
    fn seq_cst(&self, operation: &mut dyn FnMut());

    /// Compare, enqueue, and synchronously wait at one location.
    ///
    /// `compare` runs while the coordinator is held and may only enter the
    /// target backing's byte leaf. It must not retain a runtime borrow, enter
    /// JavaScript, recursively invoke an atomic operation, or block.
    fn wait(
        &self,
        location: WaitLocation,
        timeout: Option<u64>,
        compare: &mut dyn FnMut() -> bool,
    ) -> WaitOutcome;

    /// Wake the first `count` waiters at a location and return the wake count.
    fn notify(&self, location: WaitLocation, count: usize) -> usize;
}

/// Run one non-blocking atomic byte leaf in the process-wide order.
///
/// With no waiter installed nothing can be waiting anywhere, so the order is
/// vacuous and the leaf runs as it is.
pub(crate) fn with_seq_cst(waiter: Option<&dyn Waiter>, mut operation: impl FnMut()) {
    match waiter {
        Some(waiter) => waiter.seq_cst(&mut operation),
        None => operation(),
    }
}
