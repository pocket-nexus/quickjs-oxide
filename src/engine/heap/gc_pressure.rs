//! Allocation consumes a budget; collection is serviced at VM boundaries.
//! Leaves cannot participate in cycles and do not consume the budget.
use super::{AuxiliaryState, Heap, HeapCleanup, SlotState};
use crate::engine::api::{Runtime, RuntimeError};
use crate::engine::heap::runtime::RuntimeState;
use std::{cell::Cell, rc::Rc};

/// Automatic cycle collection policy. Explicit `Runtime::run_gc` always works.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GcPolicy {
    Manual,
    #[default]
    Automatic,
}

// Amortize full-graph scans over uniform net growth, including small heaps.
// This counts eligible nodes, not bytes; ordinary RC churn returns credits.
pub(super) const MIN_GC_HEADROOM: usize = 16 * 1024;

pub(crate) struct GcPressure {
    pub(crate) policy: Cell<GcPolicy>,
    /// Net allocation headroom. Zero latches a request until successful GC;
    /// ordinary RC batches return credit while the budget is still positive.
    pub(crate) remaining: Cell<usize>,
    pub(crate) collecting: Cell<bool>,
}

impl GcPressure {
    pub(crate) fn new() -> Self {
        Self {
            policy: Cell::new(GcPolicy::default()),
            remaining: Cell::new(MIN_GC_HEADROOM),
            collecting: Cell::new(false),
        }
    }

    #[inline]
    fn allocated(&self) {
        self.remaining.set(self.remaining.get().saturating_sub(1));
    }

    fn reclaimed(&self, nodes: usize) {
        let remaining = self.remaining.get();
        if remaining != 0 {
            self.remaining.set(remaining.saturating_add(nodes));
        }
    }

    fn collected(&self, live: usize) {
        self.remaining.set(live.max(MIN_GC_HEADROOM));
    }

    #[inline]
    pub(crate) fn requested(&self) -> bool {
        self.remaining.get() == 0 && self.policy.get() == GcPolicy::Automatic
    }

    pub(crate) fn begin_collection(&self) -> Result<CollectionGuard<'_>, RuntimeError> {
        if self.collecting.replace(true) {
            return Err(RuntimeError::Invariant("cycle collection reentered"));
        }
        Ok(CollectionGuard {
            collecting: &self.collecting,
            #[cfg(feature = "profiling")]
            _timer: None,
        })
    }

    /// Automatic admission is shared by external and current-state service.
    /// A skipped or failed collection leaves an exhausted request latched.
    pub(crate) fn begin_requested_collection(
        &self,
    ) -> Result<Option<CollectionGuard<'_>>, RuntimeError> {
        if !self.requested() || self.collecting.get() || std::thread::panicking() {
            return Ok(None);
        }
        let guard = self.begin_collection()?;
        #[cfg(feature = "profiling")]
        let guard = {
            let mut guard = guard;
            guard._timer = Some(crate::engine::api::profiling::PhaseTimer::start_vm(
                "gc.automatic",
            ));
            crate::engine::api::profiling::record_owned_execution_event("gc.automatic.started");
            guard
        };
        Ok(Some(guard))
    }
}

pub(crate) struct CollectionGuard<'a> {
    collecting: &'a Cell<bool>,
    #[cfg(feature = "profiling")]
    _timer: Option<crate::engine::api::profiling::PhaseTimer>,
}

impl Drop for CollectionGuard<'_> {
    fn drop(&mut self) {
        self.collecting.set(false);
    }
}

impl RuntimeState {
    /// Service at a fully published allocation or scheduler safe point. The
    /// caller pairs this state with its RuntimeInner pressure and has already
    /// assigned every temporary strong edge a cleanup owner. This neither
    /// reborrows Runtime nor drains external deferred releases or runs jobs.
    #[inline]
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn collect_if_requested(
        &mut self,
        pressure: &GcPressure,
        poisoned: &Cell<bool>,
    ) -> Result<(), RuntimeError> {
        if !pressure.requested() {
            return Ok(());
        }
        self.collect_requested(pressure, poisoned)
    }

    #[cold]
    #[inline(never)]
    fn collect_requested(
        &mut self,
        pressure: &GcPressure,
        poisoned: &Cell<bool>,
    ) -> Result<(), RuntimeError> {
        let Some(_collection) = pressure.begin_requested_collection()? else {
            return Ok(());
        };
        self.collect_cycles(poisoned)?;
        // External roots can still be queued while this access is held. Keep
        // zero-queue capacity until their outside-borrow coordination point.
        self.heap.rearm_gc_budget();
        Ok(())
    }
}

impl Heap {
    pub(crate) fn with_gc_pressure(mut self, pressure: Rc<GcPressure>) -> Self {
        assert!(
            self.slots.is_empty() && self.var_refs.slots.is_empty() && self.shapes.slots.is_empty()
        );
        self.gc_pressure = Some(pressure);
        self
    }

    #[inline]
    pub(super) fn record_cycle_allocation(&self) {
        if let Some(pressure) = &self.gc_pressure {
            pressure.allocated();
        }
    }

    /// Ordinary releases already produce this cleanup summary. Return credit
    /// once per zero-queue batch, with no scheduler fields or parameters in the
    /// arena's reserve/abort/reclaim methods.
    pub(super) fn credit_cycle_reclamation(&self, cleanup: &HeapCleanup) {
        if let Some(pressure) = &self.gc_pressure {
            let nodes = cleanup
                .finalized_objects
                .saturating_add(cleanup.finalized_contexts)
                .saturating_add(cleanup.finalized_function_bytecodes)
                .saturating_add(cleanup.finalized_shapes)
                .saturating_add(cleanup.finalized_var_refs);
            if nodes != 0 {
                pressure.reclaimed(nodes);
            }
        }
    }

    /// Count only at collection completion, rather than maintaining an exact
    /// global population through every abort and reclamation path.
    pub(crate) fn rearm_gc_budget(&self) {
        if let Some(pressure) = &self.gc_pressure {
            let shared = self
                .slots
                .iter()
                .filter(|s| !matches!(s.state, SlotState::Vacant | SlotState::Retired))
                .count();
            let cells = self
                .var_refs
                .slots
                .iter()
                .filter(|s| !matches!(s.state, AuxiliaryState::Vacant | AuxiliaryState::Retired))
                .count();
            let shapes = self
                .shapes
                .slots
                .iter()
                .filter(|s| !matches!(s.state, AuxiliaryState::Vacant | AuxiliaryState::Retired))
                .count();
            pressure.collected(shared.saturating_add(cells).saturating_add(shapes));
        }
    }
}

impl Runtime {
    pub fn gc_policy(&self) -> Result<GcPolicy, crate::engine::api::RuntimeError> {
        self.check_poison()?;
        Ok(self.0.gc_pressure.policy.get())
    }

    /// Changing policy does not collect or execute JS. Exhausted budget remains
    /// pending under Manual and will be serviced after Automatic is enabled.
    pub fn set_gc_policy(&self, policy: GcPolicy) -> Result<(), crate::engine::api::RuntimeError> {
        self.check_poison()?;
        self.0.gc_pressure.policy.set(policy);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::api::Value;

    #[test]
    fn cycle_budget_requests_without_collecting_and_explicit_gc_rearms() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        runtime
            .set_gc_policy(GcPolicy::Manual)
            .expect("set GC policy");
        drop(
            context
                .eval("for(let i=0;i<20000;i++){let x={};x.self=x}")
                .unwrap(),
        );
        assert_eq!(runtime.0.gc_pressure.remaining.get(), 0);
        assert!(runtime.heap_counts().expect("runtime state").object_nodes >= 20000);
        runtime.run_gc().unwrap();
        let n = runtime.heap_counts().expect("runtime state");
        let live = n.object_nodes
            + n.shape_nodes
            + n.var_ref_nodes
            + n.context_nodes
            + n.function_bytecode_nodes;
        assert_eq!(
            runtime.0.gc_pressure.remaining.get(),
            live.max(MIN_GC_HEADROOM)
        );
        assert_eq!(context.eval("1+1").unwrap(), Value::Int(2));
    }

    #[test]
    fn cycle_budget_excludes_leaves_and_failed_publication() {
        use super::super::{HeapNodeKind, ObjectId, Shape};
        let pressure = Rc::new(GcPressure::new());
        let mut heap = Heap::new().with_gc_pressure(pressure.clone());
        let before = pressure.remaining.get();
        let (index, _) = heap.reserve(HeapNodeKind::Object).unwrap();
        heap.abort_initializing(index).unwrap();
        assert_eq!(pressure.remaining.get(), before);
        assert!(
            heap.allocate_shape(
                Shape::new(
                    Some(ObjectId {
                        index: u32::MAX,
                        generation: 1
                    }),
                    []
                )
                .unwrap()
            )
            .is_err()
        );
        assert_eq!(pressure.remaining.get(), before);
        heap.allocate_string(crate::engine::value::JsString::from_static("leaf"))
            .unwrap();
        assert_eq!(pressure.remaining.get(), before);
        pressure.remaining.set(1);
        pressure.allocated();
        pressure.allocated();
        assert_eq!(pressure.remaining.get(), 0);
        heap.rearm_gc_budget();
        assert_eq!(pressure.remaining.get(), MIN_GC_HEADROOM);
    }
}

#[cfg(test)]
mod credit_tests {
    use super::*;

    #[test]
    fn rc_churn_returns_budget_without_clearing_a_latched_request() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        runtime
            .set_gc_policy(GcPolicy::Manual)
            .expect("set GC policy");
        runtime.run_gc().unwrap();
        drop(
            context
                .eval(
                    "for(let i=0;i<20000;i++){let x={n:i}}
1",
                )
                .unwrap(),
        );
        assert!(
            runtime.0.gc_pressure.remaining.get() > 0,
            "acyclic churn must not exhaust the net budget"
        );
        drop(
            context
                .eval("for(let i=0;i<20000;i++){let x={};x.self=x}")
                .unwrap(),
        );
        assert_eq!(runtime.0.gc_pressure.remaining.get(), 0);
        drop(context.eval("for(let i=0;i<100;i++){let x={n:i}}").unwrap());
        assert_eq!(
            runtime.0.gc_pressure.remaining.get(),
            0,
            "a request stays latched until collection"
        );
        runtime.run_gc().unwrap();
        assert!(runtime.0.gc_pressure.remaining.get() > 0);
    }
}

#[cfg(test)]
mod net_budget_tests {
    use super::*;

    fn cycle_nodes(runtime: &Runtime) -> usize {
        let n = runtime.heap_counts().expect("runtime state");
        n.object_nodes
            + n.context_nodes
            + n.function_bytecode_nodes
            + n.shape_nodes
            + n.var_ref_nodes
    }

    #[test]
    fn net_budget_matches_published_growth_across_object_shape_and_capture_lifetimes() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        runtime
            .set_gc_policy(GcPolicy::Manual)
            .expect("set GC policy");
        runtime.run_gc().unwrap();
        let initial = cycle_nodes(&runtime);
        let budget = runtime.0.gc_pressure.remaining.get();
        for source in [
            "var keep=[]; for(let i=0;i<100;i++){let x={n:i};x.self=x;keep.push(()=>x)}",
            "keep=null;for(let i=0;i<100;i++){let x={n:i};x.n++}",
        ] {
            drop(context.eval(source).unwrap());
            assert_eq!(
                runtime.0.gc_pressure.remaining.get() + cycle_nodes(&runtime),
                budget + initial
            );
        }
    }
}

#[cfg(test)]
mod direct_state_tests {
    use super::*;
    use crate::engine::heap::{RawId, Shape};
    use crate::engine::value::JsValue;

    #[test]
    fn direct_state_requested_gc_collects_cycles_and_preserves_owned_values() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        runtime.set_gc_policy(GcPolicy::Manual).unwrap();
        let live = runtime
            .into_jsvalue(
                context
                    .eval("(()=>{let dead={};dead.self=dead;return {n:42}})()")
                    .unwrap(),
            )
            .unwrap();
        let JsValue::Object(live_id) = live else {
            panic!("object")
        };
        let mut state = runtime.0.state.borrow_mut();
        let before = state.heap.counts().object_nodes;
        let owners = state.heap.object_strong_count(live_id).unwrap();
        runtime.0.gc_pressure.policy.set(GcPolicy::Automatic);
        runtime.0.gc_pressure.remaining.set(0);
        state
            .collect_if_requested(&runtime.0.gc_pressure, &runtime.0.poisoned)
            .unwrap();
        assert!(state.heap.counts().object_nodes < before);
        assert_eq!(state.heap.object_strong_count(live_id).unwrap(), owners);
        assert!(runtime.0.gc_pressure.remaining.get() >= MIN_GC_HEADROOM);
        assert!(!runtime.0.gc_pressure.collecting.get());
        assert!(!runtime.0.deferred_references.has_pending());
        state.release_jsvalue(JsValue::Object(live_id)).unwrap();
    }

    #[test]
    fn direct_state_requested_gc_defers_policy_and_reentry_without_clearing_request() {
        let runtime = Runtime::new();
        let mut state = runtime.0.state.borrow_mut();
        runtime.0.gc_pressure.remaining.set(0);
        runtime.0.gc_pressure.policy.set(GcPolicy::Manual);
        state
            .collect_if_requested(&runtime.0.gc_pressure, &runtime.0.poisoned)
            .unwrap();
        assert_eq!(runtime.0.gc_pressure.remaining.get(), 0);
        runtime.0.gc_pressure.policy.set(GcPolicy::Automatic);
        runtime.0.gc_pressure.collecting.set(true);
        state
            .collect_if_requested(&runtime.0.gc_pressure, &runtime.0.poisoned)
            .unwrap();
        assert!(runtime.0.gc_pressure.collecting.get());
        assert_eq!(runtime.0.gc_pressure.remaining.get(), 0);
        runtime.0.gc_pressure.collecting.set(false);
        state
            .collect_if_requested(&runtime.0.gc_pressure, &runtime.0.poisoned)
            .unwrap();
        assert_eq!(runtime.0.gc_pressure.remaining.get(), MIN_GC_HEADROOM);
    }

    #[test]
    fn direct_state_requested_gc_failure_resets_admission_and_keeps_budget_latched() {
        let runtime = Runtime::new();
        let mut state = runtime.0.state.borrow_mut();
        let shape = state
            .heap
            .allocate_shape(Shape::new(None, []).unwrap())
            .unwrap();
        state.retain_construction_shape(shape).unwrap();
        runtime.0.gc_pressure.remaining.set(0);
        state.heap.set_strong_count_for_test(RawId::Shape(shape), 0);
        let result = state.collect_if_requested(&runtime.0.gc_pressure, &runtime.0.poisoned);
        assert!(result.is_err());
        assert!(runtime.is_poisoned());
        assert_eq!(runtime.0.gc_pressure.remaining.get(), 0);
        assert!(!runtime.0.gc_pressure.collecting.get());
        assert!(matches!(
            state.collect_if_requested(&runtime.0.gc_pressure, &runtime.0.poisoned),
            Err(RuntimeError::Poisoned)
        ));
        // The pool removed its owner before release failed. Do not repair and
        // resume a state whose cleanup contract is already broken.
        drop(state);
        assert!(matches!(runtime.run_gc(), Err(RuntimeError::Poisoned)));
    }

    #[test]
    fn direct_state_requested_gc_leaves_external_deferred_roots_for_boundary() {
        let runtime = Runtime::new();
        let root = runtime.new_object(None).unwrap();
        let id = root.object_id();
        let mut state = runtime.0.state.borrow_mut();
        drop(root);
        assert!(runtime.0.deferred_references.has_pending());
        runtime.0.gc_pressure.remaining.set(0);
        state
            .collect_if_requested(&runtime.0.gc_pressure, &runtime.0.poisoned)
            .unwrap();
        assert!(runtime.0.deferred_references.has_pending());
        assert!(state.heap.object(id).is_ok());
        drop(state);
        runtime.drain_deferred_references().unwrap();
        assert!(!runtime.0.deferred_references.has_pending());
        assert!(runtime.0.state.borrow().heap.object(id).is_err());
    }

    #[test]
    fn direct_state_requested_gc_defers_during_unwind() {
        use std::panic::{AssertUnwindSafe, catch_unwind};
        struct ServiceOnDrop<'a> {
            state: &'a mut RuntimeState,
            pressure: &'a GcPressure,
            poisoned: &'a Cell<bool>,
            deferred: &'a Cell<bool>,
        }
        impl Drop for ServiceOnDrop<'_> {
            fn drop(&mut self) {
                self.deferred.set(
                    self.state
                        .collect_if_requested(self.pressure, self.poisoned)
                        .is_ok()
                        && self.pressure.remaining.get() == 0
                        && !self.pressure.collecting.get(),
                );
            }
        }
        let runtime = Runtime::new();
        let mut state = runtime.0.state.borrow_mut();
        let deferred = Cell::new(false);
        runtime.0.gc_pressure.remaining.set(0);
        let result = catch_unwind(AssertUnwindSafe(|| {
            let _guard = ServiceOnDrop {
                state: &mut state,
                pressure: &runtime.0.gc_pressure,
                poisoned: &runtime.0.poisoned,
                deferred: &deferred,
            };
            panic!("exercise direct-state automatic GC admission while unwinding");
        }));
        assert!(result.is_err());
        assert!(deferred.get());
        state
            .collect_if_requested(&runtime.0.gc_pressure, &runtime.0.poisoned)
            .unwrap();
        assert_eq!(runtime.0.gc_pressure.remaining.get(), MIN_GC_HEADROOM);
    }
}
