//! Host capabilities for sequentially-consistent ordering and synchronous waits.
//!
//! A waiter location contains only a stable shared-backing identity and an
//! absolute byte offset. Callers retain the backing for the entire operation;
//! the host coordinator must not retain runtime, heap, or backing handles.

use core::time::Duration;

/// One waitable shared-memory address.
///
/// QuickJS keys waiters by backing allocation and absolute byte address. The
/// TypedArray wrapper, element kind, and element width are intentionally not
/// part of the key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WaitLocation {
    backing_id: u64,
    byte_offset: usize,
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

/// Host coordination shared by every agent that accesses the same memory.
///
/// Each method enters the same sequentially-consistent order before entering
/// a backing byte leaf, maintaining coordinator -> backing lock order.
pub trait Waiter: Send + Sync {
    /// Invoke `operation` exactly once, synchronously, in the shared SC order.
    fn seq_cst(&self, operation: &mut dyn FnMut());

    /// Compare and register under the same coordinator, then wait.
    ///
    /// `compare` is invoked exactly once while the coordinator is held. It may
    /// only enter the target backing's byte leaf: no JavaScript, recursive
    /// atomic operations, retained runtime borrows, or blocking.
    fn wait(
        &self,
        location: WaitLocation,
        timeout: Option<Duration>,
        compare: &mut dyn FnMut() -> bool,
    ) -> WaitOutcome;

    /// Wake the first `count` waiters at `location` and return the wake count.
    fn notify(&self, location: WaitLocation, count: usize) -> usize;
}

/// Run one non-blocking atomic byte leaf in the host's shared SC order.
///
/// A missing capability is suitable only for a single-agent embedding.
pub(crate) fn with_seq_cst<R>(waiter: Option<&dyn Waiter>, operation: impl FnOnce() -> R) -> R {
    let Some(waiter) = waiter else {
        return operation();
    };
    let mut operation = Some(operation);
    let mut result = None;
    waiter.seq_cst(&mut || {
        result = Some(operation.take().expect("SC operation called only once")());
    });
    result.expect("SC operation called synchronously")
}
