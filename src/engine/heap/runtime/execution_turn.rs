//! ECMAScript kept objects belong to a synchronous execution turn, not a frame.
use super::RuntimeState;
use crate::engine::api::{Runtime, RuntimeError};
use crate::engine::heap::WeakCollectionKey;

pub(crate) struct ExecutionTurn {
    runtime: std::rc::Weak<super::RuntimeInner>,
    active: bool,
}

impl Runtime {
    /// Group consecutive embedding calls into one synchronous execution turn.
    ///
    /// WeakRef targets constructed or successfully dereferenced during the
    /// closure survive nested calls and explicit GC until the outer turn ends.
    /// Pending jobs must be dispatched after this closure returns. Nested turns
    /// share their outer turn. An ordinary top-level engine entry creates an
    /// implicit turn when no explicit turn is active.
    pub fn with_execution_turn<T>(
        &self,
        body: impl FnOnce() -> Result<T, RuntimeError>,
    ) -> Result<T, RuntimeError> {
        self.check_poison()?;
        let _operation = self.operation()?;
        let turn = self.enter_execution_turn()?;
        let result = body();
        // The embedder may catch a child panic inside the closure.
        self.check_poison()?;
        let cleanup = turn.finish();
        match result {
            Err(error) => Err(error),
            Ok(value) => cleanup.map(|()| value),
        }
    }

    pub(crate) fn enter_execution_turn(&self) -> Result<ExecutionTurn, RuntimeError> {
        self.check_poison()?;
        let depth = self
            .0
            .execution_turn_depth
            .get()
            .checked_add(1)
            .ok_or(RuntimeError::Invariant("execution turn depth overflow"))?;
        self.0.execution_turn_depth.set(depth);
        Ok(ExecutionTurn {
            runtime: std::rc::Rc::downgrade(&self.0),
            active: true,
        })
    }

    pub(crate) fn add_to_kept_objects(
        &self,
        target: WeakCollectionKey,
    ) -> Result<(), RuntimeError> {
        let mut state = self.0.state.borrow_mut();
        if state.kept_objects.contains(&target) {
            return Ok(());
        }
        state
            .kept_objects
            .try_reserve(1)
            .map_err(|_| RuntimeError::Invariant("kept-object allocation failed"))?;
        match target {
            WeakCollectionKey::Object(id) => state.heap.retain_object(id)?,
            WeakCollectionKey::Symbol(atom) => {
                state.atoms.retain(atom)?;
            }
        }
        state.kept_objects.insert(target);
        Ok(())
    }
}

impl RuntimeState {
    pub(super) fn clear_kept_objects(&mut self) -> Result<(), RuntimeError> {
        let mut kept = std::mem::take(&mut self.kept_objects);
        for target in kept.drain() {
            let result = match target {
                WeakCollectionKey::Object(id) => self
                    .heap
                    .release_object(id)
                    .map_err(RuntimeError::from)
                    .and_then(|cleanup| self.apply_cleanup(cleanup)),
                WeakCollectionKey::Symbol(atom) => self.release_atoms([atom]),
            };
            result?;
        }
        self.kept_objects = kept;
        Ok(())
    }
}

impl ExecutionTurn {
    pub(crate) fn finish(mut self) -> Result<(), RuntimeError> {
        self.close()
    }

    fn close(&mut self) -> Result<(), RuntimeError> {
        if !self.active {
            return Ok(());
        }
        self.active = false;
        let Some(inner) = self.runtime.upgrade() else {
            return Ok(());
        };
        let runtime = Runtime(inner);
        let _unwind = runtime.unwind_guard();
        let depth = runtime.0.execution_turn_depth.get() - 1;
        runtime.0.execution_turn_depth.set(depth);
        if runtime.skip_cleanup() {
            return Ok(());
        }
        if depth == 0 {
            runtime
                .0
                .state
                .borrow_mut()
                .clear_kept_objects()
                .inspect_err(|_| runtime.0.poisoned.set(true))?;
            runtime.drain_deferred_references()?;
            runtime.collect_if_requested()?;
        }
        Ok(())
    }
}

impl Drop for ExecutionTurn {
    fn drop(&mut self) {
        // Explicit finish reports recoverable collection failures. Drop cannot
        // report them; close quarantines a failed owned release before returning
        // and never services deferred work or GC on that state.
        let _ = self.close();
    }
}
