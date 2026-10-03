//! Allocation consumes a budget; collection is serviced at VM boundaries.
//! Leaves cannot participate in cycles and do not consume the budget.
use super::{AuxiliaryState, Heap, HeapCleanup, SlotState};
use crate::engine::api::Runtime;
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
    #[must_use]
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
