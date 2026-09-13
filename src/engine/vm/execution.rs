//! Scoped execution ownership. The registry contains identities, never Values
//! or Runtime-owning frames, and its guard unregisters during Rust unwinding.

use crate::engine::api::error::Error;
use crate::engine::api::runtime::Runtime;
use crate::engine::value::Value;
use crate::engine::vm::frame::FrameStore;
use crate::engine::vm::stack::SlotStore;
use std::cell::{Cell, RefCell};

thread_local! {
    static NEXT_EXECUTION: Cell<u64> = const { Cell::new(1) };
    static ACTIVE_EXECUTIONS: RefCell<Vec<(u64, u64)>> = const { RefCell::new(Vec::new()) };
}

pub(super) struct ExecutionLimits {
    pub frames: usize,
    pub slots: usize,
}

impl Default for ExecutionLimits {
    fn default() -> Self {
        // Native recursion still uses its existing budget during migration.
        // Arena sizes are fallible and bounded by Rust's addressable storage;
        // S04 adds the explicit-call budget at the frame push boundary.
        Self {
            frames: u16::MAX as usize,
            slots: isize::MAX as usize
                / std::mem::size_of::<Option<crate::engine::vm::bindings::FrameBinding>>(),
        }
    }
}

struct ExecutionGuard {
    domain: u64,
    id: u64,
}

impl ExecutionGuard {
    fn enter(runtime: &Runtime) -> Result<Self, Error> {
        let id = NEXT_EXECUTION.with(|next| -> Result<u64, Error> {
            let id = next.get();
            let after = id
                .checked_add(1)
                .ok_or_else(|| Error::internal("execution identity exhausted"))?;
            next.set(after);
            Ok(id)
        })?;
        let domain = runtime.domain_id();
        ACTIVE_EXECUTIONS.with(|active| -> Result<(), Error> {
            let mut active = active.borrow_mut();
            active
                .try_reserve(1)
                .map_err(|_| Error::internal("execution registration allocation failed"))?;
            active.push((domain, id));
            Ok(())
        })?;
        Ok(Self { domain, id })
    }
}

impl Drop for ExecutionGuard {
    fn drop(&mut self) {
        ACTIVE_EXECUTIONS.with(|active| {
            let mut active = active.borrow_mut();
            if let Some(index) = active
                .iter()
                .rposition(|entry| *entry == (self.domain, self.id))
            {
                active.remove(index);
            }
        });
    }
}

pub(super) struct RunningExecution {
    pub frames: FrameStore,
    pub slots: SlotStore,
    /// Cold completion owns its payload before the active window is cleared.
    pub pending: Option<Value>,
    _guard: ExecutionGuard,
}

impl RunningExecution {
    pub(super) fn new(runtime: &Runtime, limits: ExecutionLimits) -> Result<Self, Error> {
        let guard = ExecutionGuard::enter(runtime)?;
        Ok(Self {
            frames: FrameStore::new(guard.id, limits.frames),
            slots: SlotStore::new(limits.slots),
            pending: None,
            _guard: guard,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{ACTIVE_EXECUTIONS, ExecutionLimits, RunningExecution};
    use crate::engine::api::Runtime;
    use std::rc::Rc;

    #[test]
    fn nested_execution_registration_is_removed_on_panic() {
        let runtime = Runtime::new();
        let outer = RunningExecution::new(&runtime, ExecutionLimits::default()).unwrap();
        let before = ACTIVE_EXECUTIONS.with(|active| active.borrow().clone());
        assert_eq!(before.len(), 1);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _inner = RunningExecution::new(&runtime, ExecutionLimits::default()).unwrap();
            assert_eq!(ACTIVE_EXECUTIONS.with(|active| active.borrow().len()), 2);
            panic!("exercise execution guard unwinding");
        }));
        assert!(result.is_err());
        assert_eq!(
            ACTIVE_EXECUTIONS.with(|active| active.borrow().clone()),
            before
        );
        drop(outer);
        assert!(ACTIVE_EXECUTIONS.with(|active| active.borrow().is_empty()));
    }

    #[test]
    fn identity_registration_cannot_keep_a_runtime_alive() {
        let runtime = Runtime::new();
        let weak = Rc::downgrade(&runtime.0);
        let execution = RunningExecution::new(&runtime, ExecutionLimits::default()).unwrap();
        drop(runtime);
        assert!(weak.upgrade().is_none());
        drop(execution);
        assert!(ACTIVE_EXECUTIONS.with(|active| active.borrow().is_empty()));
    }
}
