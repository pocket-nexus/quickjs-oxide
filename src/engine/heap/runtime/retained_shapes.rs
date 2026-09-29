//! Bounded roots for reusable construction prefixes. Canonical and transition
//! indexes remain weak; this pool is the sole optional owner and never mutates
//! admitted shapes. FIFO eviction bounds one-off constructor churn.
use super::RuntimeState;
use crate::engine::{
    api::RuntimeError,
    hash::FxBuildHasher,
    heap::{HeapCleanup, ShapeId},
};
use std::collections::{HashSet, VecDeque};

const MAX_SHAPES: usize = 256;
const MAX_BYTES: usize = 256 * 1024;
const MAX_PROPERTIES: usize = 64;

#[derive(Default)]
pub(crate) struct RetainedShapes {
    order: VecDeque<(ShapeId, usize)>,
    members: HashSet<ShapeId, FxBuildHasher>,
    bytes: usize,
}

impl RuntimeState {
    pub(crate) fn retain_construction_shape(&mut self, id: ShapeId) -> Result<(), RuntimeError> {
        if self.retained_shapes.members.contains(&id) {
            #[cfg(feature = "profiling")]
            crate::engine::api::profiling::record_owned_execution_event("shape_prefix_reused");
            return Ok(());
        }
        let shape = self.heap.shape(id)?;
        if shape.is_dictionary() || shape.entries().len() > MAX_PROPERTIES {
            return Ok(());
        }
        let bytes = shape.retention_bytes();
        if bytes > MAX_BYTES {
            return Ok(());
        }
        // Admission is optional. Allocation pressure skips caching, without a
        // new language-visible failure or a half-published root.
        if self.retained_shapes.order.try_reserve(1).is_err()
            || self.retained_shapes.members.try_reserve(1).is_err()
        {
            return Ok(());
        }
        while self.retained_shapes.order.len() >= MAX_SHAPES
            || self.retained_shapes.bytes + bytes > MAX_BYTES
        {
            #[cfg(feature = "profiling")]
            crate::engine::api::profiling::record_owned_execution_event("shape_prefix_evicted");
            let (old, charge) = self.retained_shapes.order.pop_front().unwrap();
            self.retained_shapes.members.remove(&old);
            self.retained_shapes.bytes -= charge;
            let cleanup = self.heap.release_shape(old)?;
            self.apply_cleanup(cleanup)?;
        }
        self.heap.retain_shape(id)?;
        self.retained_shapes.order.push_back((id, bytes));
        self.retained_shapes.members.insert(id);
        self.retained_shapes.bytes += bytes;
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_execution_event("shape_prefix_retained");
        Ok(())
    }

    pub(crate) fn release_retained_shapes(&mut self) -> Result<HeapCleanup, RuntimeError> {
        let mut cleanup = HeapCleanup::default();
        while let Some((id, bytes)) = self.retained_shapes.order.pop_front() {
            self.retained_shapes.members.remove(&id);
            self.retained_shapes.bytes -= bytes;
            cleanup.merge(self.heap.release_shape(id)?);
        }
        Ok(cleanup)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{
        api::Runtime,
        atom::AtomIdx,
        object::shape::{PropertyFlags, ShapeEntry},
    };

    #[test]
    fn prefix_pool_is_bounded_and_gc_releases_its_roots() {
        let runtime = Runtime::new();
        let mut state = runtime.0.state.borrow_mut();
        for index in 0..1000 {
            let entry = ShapeEntry {
                atom: AtomIdx::from_raw(0x80000000 | index),
                flags: PropertyFlags::data(true, true, true),
            };
            let id = state.get_or_create_shape(None, &[entry]).unwrap();
            state.retain_construction_shape(id).unwrap();
            let cleanup = state.heap.release_shape(id).unwrap();
            state.apply_cleanup(cleanup).unwrap();
            assert!(state.retained_shapes.order.len() <= MAX_SHAPES);
            assert!(state.retained_shapes.bytes <= MAX_BYTES);
        }
        assert_eq!(state.retained_shapes.order.len(), MAX_SHAPES);
        drop(state);
        runtime.run_gc().unwrap();
        let state = runtime.0.state.borrow();
        assert_eq!(state.retained_shapes.bytes, 0);
        assert!(state.retained_shapes.members.is_empty());
        assert!(state.shape_cache.is_empty());
    }
}
