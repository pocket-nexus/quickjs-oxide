//! Owner-level storage accounting and safe arena allocation observation.
use super::*;
use crate::engine::api::profiling::{AllocationTrace, MemoryCategory};
use std::collections::HashSet;
use std::ops::{Deref, DerefMut};

// Deliberately exposes a slice, not Vec mutators: every capacity-changing
// operation goes through push and every backing release goes through Drop.
pub(super) struct ArenaStorage<T> {
    values: Vec<T>,
    trace: Option<AllocationTrace>,
    /// Storage-lifetime identity in the allocation trace: each arena owns one.
    allocation_id: u64,
}

impl<T> ArenaStorage<T> {
    pub(super) const fn new(allocation_id: u64) -> Self {
        Self {
            values: Vec::new(),
            trace: None,
            allocation_id,
        }
    }

    pub(super) fn capacity(&self) -> usize {
        self.values.capacity()
    }

    pub(super) fn push(&mut self, value: T) {
        let previous = self.values.capacity();
        self.values.push(value);
        if let Some(trace) = &self.trace {
            trace.record(
                self.allocation_id,
                previous * size_of::<T>(),
                self.capacity() * size_of::<T>(),
            );
        }
    }
}

impl<T> Deref for ArenaStorage<T> {
    type Target = [T];
    fn deref(&self) -> &Self::Target {
        &self.values
    }
}

impl<T> DerefMut for ArenaStorage<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.values
    }
}

impl<'a, T> IntoIterator for &'a ArenaStorage<T> {
    type Item = &'a T;
    type IntoIter = std::slice::Iter<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.values.iter()
    }
}

impl<'a, T> IntoIterator for &'a mut ArenaStorage<T> {
    type Item = &'a mut T;
    type IntoIter = std::slice::IterMut<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.values.iter_mut()
    }
}

impl<T> Drop for ArenaStorage<T> {
    fn drop(&mut self) {
        if let Some(trace) = &self.trace {
            let previous = self.capacity() * size_of::<T>();
            // Record F after the backing allocation and all its payloads drop.
            drop(std::mem::take(&mut self.values));
            trace.record(self.allocation_id, previous, 0);
            trace.finish();
        }
    }
}

impl Heap {
    pub(crate) fn with_allocation_trace(trace: Option<AllocationTrace>) -> Self {
        let mut heap = Self::new();
        heap.slots.trace = trace.clone();
        heap.leaf_slots.trace = trace.clone();
        heap.var_refs.slots.trace = trace.clone();
        heap.shapes.slots.trace = trace;
        heap
    }

    pub(crate) fn memory_categories(&self) -> Vec<MemoryCategory> {
        let counts = self.counts();
        let mut result = vec![
            logical("objects", counts.object_nodes),
            logical("shapes", counts.shape_nodes),
            logical("var_refs", counts.var_ref_nodes),
            logical("contexts", counts.context_nodes),
            logical("functions", counts.function_bytecode_nodes),
            storage(
                "arena_slots",
                self.slots.len(),
                self.slots.capacity(),
                size_of::<ArenaSlot>(),
            ),
            storage(
                "arena_free_indices",
                self.free.len(),
                self.free.capacity(),
                size_of::<u32>(),
            ),
            storage(
                "var_ref_arena_slots",
                self.var_refs.slots.len(),
                self.var_refs.slots.capacity(),
                size_of::<auxiliary_arena::AuxiliarySlot<VarRefData>>(),
            ),
            storage(
                "var_ref_arena_free_indices",
                self.var_refs.free.len(),
                self.var_refs.free.capacity(),
                size_of::<u32>(),
            ),
            storage(
                "shape_arena_slots",
                self.shapes.slots.len(),
                self.shapes.slots.capacity(),
                size_of::<auxiliary_arena::AuxiliarySlot<Shape>>(),
            ),
            storage(
                "shape_arena_free_indices",
                self.shapes.free.len(),
                self.shapes.free.capacity(),
                size_of::<u32>(),
            ),
            storage(
                "leaf_arena_slots",
                self.leaf_slots.len(),
                self.leaf_slots.capacity(),
                size_of::<LeafSlot>(),
            ),
            storage(
                "leaf_arena_free_indices",
                self.leaf_free.len(),
                self.leaf_free.capacity(),
                size_of::<u32>(),
            ),
            storage(
                "arena_zero_queue",
                self.zero_queue.len(),
                self.zero_queue.capacity(),
                size_of::<RawId>(),
            ),
            MemoryCategory {
                name: "collection_scratch_peak",
                count: None,
                used_bytes: None,
                capacity_bytes: Some(self.collection_scratch_peak_bytes),
                basis: "non-resident high-water capacity; trial arrays, reachability arrays, worklist and anchors; not additive to current storage",
            },
        ];
        let mut properties = storage("property_slots", 0, 0, 1);
        let mut arrays = logical("arrays", 0);
        let mut elements = storage("dense_array_elements", 0, 0, 1);
        let mut buffers = storage("array_buffer_bytes", 0, 0, 1);
        let mut shared_buffers = logical("shared_array_buffer_wrappers", 0);
        shared_buffers.basis = "wrapper-count-only; shared backing bytes unavailable";
        let mut string_nodes = logical("string_nodes", 0);
        string_nodes.basis = "node-count-only; payloads are Rc-owned outside the arena accounting";
        let mut bigint_nodes = logical("bigint_nodes", 0);
        bigint_nodes.basis = "node-count-only; payloads are Rc-owned outside the arena accounting";
        let mut code = storage("exec_words", 0, 0, 1);
        code.basis =
            "deduplicated-Rc-slice-inline-bytes; includes extension words, excludes Rc headers";
        let mut seen_code = HashSet::new();
        let mut boundaries = storage("exec_boundaries", 0, 0, 1);
        boundaries.basis = "deduplicated-Rc-slice-inline-bytes; excludes Rc headers";
        let mut seen_boundaries = HashSet::new();
        let mut regions = storage("exec_numeric_regions", 0, 0, 1);
        regions.basis = "deduplicated-Rc-slice-inline-bytes; excludes Rc headers";
        let mut seen_regions = HashSet::new();
        let mut product_sources = storage("exec_numeric_product_sources", 0, 0, 1);
        product_sources.basis = "deduplicated-Rc-slice-inline-bytes; excludes Rc headers";
        let mut seen_product_sources = HashSet::new();
        let mut copy_sources = storage("exec_numeric_copy_sources", 0, 0, 1);
        copy_sources.basis = "deduplicated-Rc-slice-inline-bytes; excludes Rc headers";
        let mut seen_copy_sources = HashSet::new();
        let mut executable_projections = storage("bytecode_executable_projections", 0, 0, 1);
        executable_projections.basis = "one initialized shared projection per bytecode node; excludes Rc headers and shared slice payloads";
        let mut property_keys = storage("bytecode_property_keys", 0, 0, 1);
        property_keys.count = Some(0);
        property_keys.basis = "linked-name-count; deduplicated map slice bytes including unused slots; excludes Rc headers and auxiliary atom references";
        let mut seen_property_keys = HashSet::new();
        let mut shape_entries = storage(
            "shape_entries",
            0,
            0,
            size_of::<crate::engine::object::shape::ShapeEntry>(),
        );
        let mut shape_lookup = storage("shape_lookup_minimum", 0, 0, size_of::<(AtomIdx, u32)>());
        shape_lookup.basis =
            "lower-bound-hashmap-entry-storage; excludes control bytes and allocator overhead";
        let mut shape_dictionary_headers = storage("shape_dictionary_headers", 0, 0, 1);
        let mut shape_dictionary_links = storage("shape_dictionary_links", 0, 0, 1);
        shape_dictionary_links.count = None;
        for slot in &self.slots {
            let node = match &slot.state {
                SlotState::Live(node) | SlotState::ZeroQueued(node) => node,
                _ => continue,
            };
            match &node.data {
                NodeData::Object(object) => {
                    add_storage(
                        &mut properties,
                        object.slots.len(),
                        object.slots.accounted_capacity(),
                        size_of::<PropertySlot>(),
                    );
                    match &object.payload {
                        ObjectPayload::Array { dense } => {
                            *arrays.count.as_mut().unwrap() += 1;
                            if let Some(dense) = dense {
                                add_storage(
                                    &mut elements,
                                    dense.len(),
                                    dense.capacity(),
                                    size_of::<RawValue>(),
                                );
                            }
                        }
                        ObjectPayload::ArrayBuffer(data) => {
                            add_storage(&mut buffers, data.bytes.len(), data.bytes.capacity(), 1);
                        }
                        ObjectPayload::SharedArrayBuffer(_) => {
                            *shared_buffers.count.as_mut().unwrap() += 1;
                        }
                        _ => {}
                    }
                }
                NodeData::FunctionBytecode(data) => {
                    if data.executable.get().is_some() {
                        add_storage(
                            &mut executable_projections,
                            1,
                            1,
                            size_of::<crate::engine::code::runtime::PublishedFunctionData>(),
                        );
                    }
                    // Pointer identity is local to this read-only snapshot and
                    // is never serialized. Shared slices are counted once.
                    if seen_code.insert(data.exec.word_storage_identity()) {
                        add_storage(
                            &mut code,
                            data.exec.word_len(),
                            data.exec.word_len(),
                            size_of::<u32>(),
                        );
                    }
                    if seen_boundaries.insert(data.exec.boundary_storage_identity()) {
                        add_storage(
                            &mut boundaries,
                            data.exec.boundary_len(),
                            data.exec.boundary_len(),
                            size_of::<u32>(),
                        );
                    }
                    if seen_regions.insert(data.exec.region_storage_identity()) {
                        add_storage(
                            &mut regions,
                            data.exec.region_len(),
                            data.exec.region_len(),
                            size_of::<crate::engine::code::region::PublishedNumericRegion>(),
                        );
                    }
                    let (identity, len) = data.exec.product_source_storage();
                    if len != 0 && seen_product_sources.insert(identity) {
                        add_storage(
                            &mut product_sources,
                            len,
                            len,
                            size_of::<crate::engine::code::region::ArrayProductSource>(),
                        );
                    }
                    let (identity, len) = data.exec.copy_source_storage();
                    if len != 0 && seen_copy_sources.insert(identity) {
                        add_storage(
                            &mut copy_sources,
                            len,
                            len,
                            size_of::<crate::engine::code::region::ArrayReadSource>(),
                        );
                    }
                    if let Some(keys) = &data.property_key_atoms
                        && seen_property_keys.insert(Rc::as_ptr(keys))
                    {
                        add_storage(
                            &mut property_keys,
                            keys.len(),
                            keys.len(),
                            size_of::<Atom>(),
                        );
                        *property_keys.count.as_mut().unwrap() -=
                            keys.iter().filter(|atom| atom.is_null()).count();
                    }
                }
                _ => {}
            }
        }
        for slot in &self.leaf_slots {
            match slot.value.kind() {
                Some(HeapNodeKind::String) => {
                    *string_nodes.count.as_mut().unwrap() += 1;
                }
                Some(HeapNodeKind::BigInt) => {
                    *bigint_nodes.count.as_mut().unwrap() += 1;
                }
                Some(_) | None => {}
            }
        }
        for slot in &self.shapes.slots {
            let shape = match &slot.state {
                AuxiliaryState::Live(node) | AuxiliaryState::ZeroQueued(node) => &node.data,
                _ => continue,
            };
            let backing = shape.backing_storage();
            add_storage(
                &mut shape_entries,
                backing.entries_len,
                backing.entries_capacity,
                size_of::<crate::engine::object::shape::ShapeEntry>(),
            );
            add_storage(
                &mut shape_lookup,
                backing.lookup_len,
                backing.lookup_capacity,
                size_of::<(AtomIdx, u32)>(),
            );
            add_storage(
                &mut shape_dictionary_headers,
                backing.dictionary_header_bytes,
                backing.dictionary_header_bytes,
                1,
            );
            *shape_dictionary_links.used_bytes.as_mut().unwrap() +=
                backing.dictionary_links_used_bytes;
            *shape_dictionary_links.capacity_bytes.as_mut().unwrap() +=
                backing.dictionary_links_capacity_bytes;
        }
        result.extend([
            properties,
            arrays,
            elements,
            buffers,
            shared_buffers,
            string_nodes,
            bigint_nodes,
            code,
            boundaries,
            regions,
            product_sources,
            copy_sources,
            property_keys,
            executable_projections,
            shape_entries,
            shape_lookup,
            shape_dictionary_headers,
            shape_dictionary_links,
        ]);
        result
    }
}

fn logical(name: &'static str, count: usize) -> MemoryCategory {
    MemoryCategory {
        name,
        count: Some(count),
        used_bytes: None,
        capacity_bytes: None,
        basis: "logical-node-count; not total bytes",
    }
}

fn storage(name: &'static str, len: usize, capacity: usize, size: usize) -> MemoryCategory {
    MemoryCategory {
        name,
        count: Some(len),
        used_bytes: Some(len * size),
        capacity_bytes: Some(capacity * size),
        basis: "owned-inline-storage; excludes nested allocations and allocator overhead",
    }
}

fn add_storage(category: &mut MemoryCategory, len: usize, capacity: usize, size: usize) {
    *category.count.as_mut().unwrap() += len;
    *category.used_bytes.as_mut().unwrap() += len * size;
    *category.capacity_bytes.as_mut().unwrap() += capacity * size;
}
