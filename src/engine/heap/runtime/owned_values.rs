//! Local temporary owners clean up through their existing state borrow.
//! These guards hold no Runtime owner and never reacquire a RefCell borrow.

use super::RuntimeState;
use crate::engine::value::JsValue;
use std::cell::Cell;

/// One owned temporary. The poison flag is the owning RuntimeInner's header
/// flag, borrowed independently of its state access.
#[must_use]
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) struct OwnedValueGuard<'a> {
    state: &'a mut RuntimeState,
    poisoned: &'a Cell<bool>,
    value: Option<JsValue>,
}

#[cfg_attr(not(test), allow(dead_code))]
impl<'a> OwnedValueGuard<'a> {
    pub(crate) fn new(
        state: &'a mut RuntimeState,
        poisoned: &'a Cell<bool>,
        value: JsValue,
    ) -> Self {
        Self {
            state,
            poisoned,
            value: Some(value),
        }
    }

    /// Split short borrows so computation can use state without surrendering
    /// the temporary's cleanup responsibility. Taking the value transfers
    /// that responsibility to the receiving execution storage.
    pub(crate) fn parts(&mut self) -> (&mut RuntimeState, &mut Option<JsValue>) {
        (self.state, &mut self.value)
    }
}

impl Drop for OwnedValueGuard<'_> {
    fn drop(&mut self) {
        if skip_cleanup(self.poisoned) {
            return;
        }
        let _unwind = PoisonOnUnwind(self.poisoned);
        if let Some(value) = self.value.take()
            && self.state.release_jsvalue(value).is_err()
        {
            self.poisoned.set(true);
        }
    }
}

/// An already allocated argument/scratch buffer. The guard introduces no
/// extra allocation; taking values through parts() transfers their edges to
/// execution storage before any subsequent fallible operation.
#[must_use]
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) struct OwnedValuesGuard<'a> {
    state: &'a mut RuntimeState,
    poisoned: &'a Cell<bool>,
    values: Vec<JsValue>,
}

#[cfg_attr(not(test), allow(dead_code))]
impl<'a> OwnedValuesGuard<'a> {
    pub(crate) fn new(
        state: &'a mut RuntimeState,
        poisoned: &'a Cell<bool>,
        values: Vec<JsValue>,
    ) -> Self {
        Self {
            state,
            poisoned,
            values,
        }
    }

    pub(crate) fn parts(&mut self) -> (&mut RuntimeState, &mut Vec<JsValue>) {
        (self.state, &mut self.values)
    }
}

impl Drop for OwnedValuesGuard<'_> {
    fn drop(&mut self) {
        if skip_cleanup(self.poisoned) {
            return;
        }
        let _unwind = PoisonOnUnwind(self.poisoned);
        // Preserve the original ascending argument-release order. Once an
        // invariant failure poisons state, no remaining edge may traverse it.
        for value in self.values.drain(..) {
            if self.state.release_jsvalue(value).is_err() {
                self.poisoned.set(true);
                break;
            }
        }
    }
}

fn skip_cleanup(poisoned: &Cell<bool>) -> bool {
    if std::thread::panicking() {
        poisoned.set(true);
    }
    poisoned.get()
}

struct PoisonOnUnwind<'a>(&'a Cell<bool>);
impl Drop for PoisonOnUnwind<'_> {
    fn drop(&mut self) {
        if std::thread::panicking() {
            self.0.set(true);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::api::Runtime;
    use crate::engine::heap::{HeapError, RawId};

    #[test]
    fn nested_guards_release_aliases_under_one_state_borrow() {
        let runtime = Runtime::new();
        let id = runtime.new_object(None).unwrap().into_handle();
        let poison = Cell::new(false);
        let mut state = runtime.0.state.borrow_mut();
        {
            let mut outer = OwnedValueGuard::new(&mut state, &poison, JsValue::Object(id));
            {
                let (state, value) = outer.parts();
                let copied = state.dup_jsvalue(value.as_ref().unwrap()).unwrap();
                let mut inner = OwnedValueGuard::new(state, &poison, copied);
                let (state, _) = inner.parts();
                assert_eq!(state.heap.object_strong_count(id), Ok(2));
            }
            let (state, _) = outer.parts();
            assert_eq!(state.heap.object_strong_count(id), Ok(1));
        }
        assert!(state.heap.object(id).is_err());
        assert!(!runtime.0.deferred_references.has_pending());
        assert!(!poison.get());
    }

    #[test]
    fn failed_retain_cleans_preceding_argument_owners() {
        fn prepare(
            state: &mut RuntimeState,
            poison: &Cell<bool>,
            first: &JsValue,
            blocked: &JsValue,
        ) -> Result<(), crate::engine::api::runtime_error::RuntimeError> {
            let mut arguments = OwnedValuesGuard::new(state, poison, Vec::new());
            let (state, values) = arguments.parts();
            values.push(state.dup_jsvalue(first)?);
            values.push(state.dup_jsvalue(blocked)?);
            Ok(())
        }

        let runtime = Runtime::new();
        let first = runtime.new_object(None).unwrap();
        let blocked = runtime.new_object(None).unwrap();
        let first_id = first.object_id();
        let blocked_id = blocked.object_id();
        let poison = Cell::new(false);
        let mut state = runtime.0.state.borrow_mut();
        state
            .heap
            .live_node_mut(RawId::Object(blocked_id))
            .unwrap()
            .strong
            .set(u32::MAX);
        let result = prepare(
            &mut state,
            &poison,
            &JsValue::Object(first_id),
            &JsValue::Object(blocked_id),
        );
        let blocked_count = state.heap.object_strong_count(blocked_id).unwrap();
        // Restore the actual root count before assertions may unwind roots.
        state
            .heap
            .live_node_mut(RawId::Object(blocked_id))
            .unwrap()
            .strong
            .set(1);
        assert!(matches!(
            result,
            Err(crate::engine::api::runtime_error::RuntimeError::Heap(
                HeapError::Overflow { .. }
            ))
        ));
        assert_eq!(blocked_count, u32::MAX);
        assert_eq!(state.heap.object_strong_count(first_id), Ok(1));
        assert!(!poison.get());
        assert!(!runtime.0.deferred_references.has_pending());
    }

    #[test]
    fn poisoned_guard_leaves_remaining_edges_for_quarantined_state() {
        let runtime = Runtime::new();
        let invalid = runtime.new_object(None).unwrap().into_handle();
        runtime.release_jsvalue(JsValue::Object(invalid)).unwrap();
        let valid = runtime.new_object(None).unwrap().into_handle();
        let poison = Cell::new(false);
        let mut state = runtime.0.state.borrow_mut();
        drop(OwnedValuesGuard::new(
            &mut state,
            &poison,
            vec![JsValue::Object(invalid), JsValue::Object(valid)],
        ));
        assert!(poison.get());
        assert_eq!(state.heap.object_strong_count(valid), Ok(1));
        // The test uses a stand-alone flag so it can inspect the stopped
        // cleanup. Real execution supplies RuntimeInner's poison header and
        // forgets its state at destruction.
        state.release_jsvalue(JsValue::Object(valid)).unwrap();
    }

    #[test]
    fn unwind_poison_skips_heap_cleanup_without_reborrowing_state() {
        let runtime = Runtime::new();
        let id = runtime.new_object(None).unwrap().into_handle();
        let poison = Cell::new(false);
        let mut state = runtime.0.state.borrow_mut();
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _temporary = OwnedValueGuard::new(&mut state, &poison, JsValue::Object(id));
            panic!("operation interrupted before commit");
        }));
        assert!(caught.is_err());
        assert!(poison.get());
        assert_eq!(state.heap.object_strong_count(id), Ok(1));
        assert!(!runtime.0.deferred_references.has_pending());
        state.release_jsvalue(JsValue::Object(id)).unwrap();
    }
}
