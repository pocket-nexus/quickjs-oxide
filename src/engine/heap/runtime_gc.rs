use crate::engine::api::runtime::Runtime;
use crate::engine::api::runtime_error::RuntimeError;
use crate::engine::heap::runtime::RuntimeState;

use crate::engine::heap::{GcStats, HeapCounts, WeakSymbolGcEvent};
use crate::engine::jobs;
#[cfg(feature = "test262-host")]
use crate::engine::value::JsValue;
#[cfg(feature = "test262-host")]
use crate::engine::vm::Completion;
#[cfg(feature = "test262-host")]
use crate::engine::vm::call::NativeInvocation;

impl Runtime {
    /// Run QuickJS-style cycle collection for this runtime.
    pub fn run_gc(&self) -> Result<GcStats, RuntimeError> {
        self.check_poison()?;
        let _collection = self.0.gc_pressure.begin_collection()?;
        self.run_gc_admitted()
    }

    fn run_gc_admitted(&self) -> Result<GcStats, RuntimeError> {
        self.check_poison()?;
        let _operation = self.operation();
        let mut state = self.0.state.borrow_mut();
        let stats = state.collect_cycles()?;
        drop(state);
        // The operation guard drains deferred root releases. Trim only after
        // that drain, so a release queued during collection cannot be lost.
        drop(_operation);
        let mut state = self.0.state.borrow_mut();
        if !self.0.deferred_references.has_pending() {
            state.heap.trim_empty_zero_queue_after_gc();
        }
        state.heap.rearm_gc_budget();
        Ok(stats)
    }

    /// Outside-state service at the driver and outer execution boundary.
    /// Continuous execution services the same request under its current state
    /// only at fully published allocation or scheduler safe points.
    #[inline]
    pub(crate) fn collect_if_requested(&self) -> Result<(), RuntimeError> {
        let pressure = &self.0.gc_pressure;
        if !pressure.requested() {
            return Ok(());
        }
        self.collect_requested()
    }

    #[cold]
    #[inline(never)]
    fn collect_requested(&self) -> Result<(), RuntimeError> {
        if self.0.gc_pressure.collecting.get() || std::thread::panicking() {
            return Ok(());
        }
        let Ok(borrow) = self.0.state.try_borrow_mut() else {
            return Ok(());
        };
        drop(borrow);
        let Some(_collection) = self.0.gc_pressure.begin_requested_collection()? else {
            return Ok(());
        };
        // The existing operation guard drains deferred releases before
        // borrowing the graph and again before rearming the allocation budget.
        self.run_gc_admitted()?;
        Ok(())
    }

    /// Execute QuickJS's test262-only `js_gc` host callback.
    ///
    /// The callback runs collection synchronously on the current runtime and
    /// deliberately does not drain the pending-job queue. Active JavaScript
    /// and native frames keep their ordinary stack-owned roots while the
    /// collector runs.
    #[cfg(feature = "test262-host")]
    pub(crate) fn call_test262_gc(
        &self,
        invocation: &NativeInvocation,
    ) -> Result<Completion, RuntimeError> {
        let NativeInvocation::Call { .. } = invocation else {
            return Err(RuntimeError::Invariant(
                "Test262 gc received a constructor invocation",
            ));
        };
        self.run_gc()?;
        Ok(Completion::Return(JsValue::Undefined))
    }

    /// Runtime heap population for diagnostics and lifecycle tests.
    #[must_use]
    pub fn heap_counts(&self) -> Result<HeapCounts, RuntimeError> {
        self.check_poison()?;
        let _unwind = self.unwind_guard();
        Ok(self.0.state.borrow().heap.counts())
    }
}

impl RuntimeState {
    /// Collect a fully published graph under the executor's existing state
    /// access. The caller owns collection admission, external-root draining
    /// and budget rearming; this kernel neither reborrows Runtime nor executes
    /// finalization jobs. Temporary strong edges must already have an owner.
    pub(crate) fn collect_cycles(&mut self) -> Result<GcStats, RuntimeError> {
        // Optional shape roots must not keep prototype graphs alive across GC.
        let retained_cleanup = self.release_retained_shapes()?;
        let mut atom_error = None;
        let mut stats = {
            let RuntimeState {
                atoms,
                heap,
                pending_jobs,
                ..
            } = self;
            let mut finalization_sink = jobs::RuntimeFinalizationJobSink::new(pending_jobs);
            heap.run_gc_with_finalization_sink(
                |event| {
                    Ok(match event {
                        WeakSymbolGcEvent::IsLive(index) => atoms.is_live_index(index),
                        WeakSymbolGcEvent::Release(index) => {
                            if let Err(error) = atoms.release_index(index) {
                                // A detached weak value owned this atom, so this
                                // can fail only after an ownership invariant has
                                // already been violated. Latch the exact error but
                                // continue without scheduling a double release.
                                atom_error.get_or_insert(error);
                            }
                            true
                        }
                    })
                },
                &mut finalization_sink,
            )?
        };
        stats.cleanup.merge(retained_cleanup);
        if let Some(error) = atom_error {
            return Err(error.into());
        }
        let atom_indices = std::mem::take(&mut stats.cleanup.atoms);
        self.unlink_finalized_shapes(stats.cleanup.finalized_shape_ids.iter().copied());
        self.release_atom_indices(atom_indices)?;
        self.atoms.sweep_released_strings();
        Ok(stats)
    }
}
