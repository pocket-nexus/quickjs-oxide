//! Thread-local, one-shot failure at the recoverable word-buffer boundary.
//! This never simulates Rc's abort-on-OOM global allocator policy.
use std::cell::Cell;

thread_local! {
    static REMAINING: Cell<Option<usize>> = const { Cell::new(None) };
}

pub(crate) struct FailureGuard;

pub(crate) fn after(successful_reservations: usize) -> FailureGuard {
    REMAINING.with(|remaining| {
        assert!(
            remaining.replace(Some(successful_reservations)).is_none(),
            "nested QuickOp reservation failure injection"
        );
    });
    FailureGuard
}

impl Drop for FailureGuard {
    fn drop(&mut self) {
        REMAINING.with(|remaining| remaining.set(None));
    }
}

pub(super) fn should_fail() -> bool {
    REMAINING.with(|remaining| match remaining.get() {
        None => false,
        Some(0) => {
            remaining.set(None);
            true
        }
        Some(count) => {
            remaining.set(Some(count - 1));
            false
        }
    })
}
