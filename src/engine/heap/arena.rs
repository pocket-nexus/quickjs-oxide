use super::*;
use std::cell::Cell;

impl Heap {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            gc_pressure: None,
            property_layout_epoch: 0,
            #[cfg(not(feature = "profiling"))]
            slots: Vec::new(),
            #[cfg(feature = "profiling")]
            slots: profiling::ArenaStorage::new(1),
            free: Vec::new(),
            var_refs: AuxiliaryArena::new(3),
            shapes: AuxiliaryArena::new(4),
            #[cfg(not(feature = "profiling"))]
            leaf_slots: Vec::new(),
            #[cfg(feature = "profiling")]
            leaf_slots: profiling::ArenaStorage::new(2),
            leaf_free: Vec::new(),
            zero_queue: VecDeque::new(),
            weak_head: None,
            weak_tail: None,
            #[cfg(debug_assertions)]
            alloc_sites: Vec::new(),
            #[cfg(debug_assertions)]
            leaf_alloc_sites: Vec::new(),
            #[cfg(feature = "profiling")]
            collection_scratch_peak_bytes: 0,
        }
    }

    /// Strong count for diagnostics.  A zombie remains queryable until all
    /// candidate incoming edges have been detached.
    pub fn object_strong_count(&self, id: ObjectId) -> Result<u32, HeapError> {
        self.shared_strong_count(RawId::Object(id))
    }

    /// Strong count for diagnostics.
    pub fn shape_strong_count(&self, id: ShapeId) -> Result<u32, HeapError> {
        self.shapes.strong_count(id)
    }

    /// Strong count for captured-variable diagnostics.
    pub fn var_ref_strong_count(&self, id: VarRefId) -> Result<u32, HeapError> {
        self.var_refs.strong_count(id)
    }

    /// Strong count for context diagnostics.
    pub fn context_strong_count(&self, id: ContextId) -> Result<u32, HeapError> {
        self.shared_strong_count(RawId::Context(id))
    }

    /// Strong count for function-bytecode diagnostics.
    #[cfg(test)]
    pub fn function_bytecode_strong_count(&self, id: FunctionBytecodeId) -> Result<u32, HeapError> {
        self.shared_strong_count(RawId::FunctionBytecode(id))
    }

    /// Snapshot aggregate arena counts for tests and runtime diagnostics.
    #[must_use]
    pub fn counts(&self) -> HeapCounts {
        let mut counts = HeapCounts::default();
        for slot in &self.slots {
            match &slot.state {
                SlotState::Initializing { kind, .. } => {
                    counts.initializing = counts.initializing.saturating_add(1);
                    increment_kind_count(&mut counts, *kind);
                }
                SlotState::Resident(node) => {
                    if node.strong.get() == 0 {
                        counts.zero_queued = counts.zero_queued.saturating_add(1);
                    } else {
                        counts.live = counts.live.saturating_add(1);
                    }
                    increment_kind_count(&mut counts, node.data.kind());
                }
                SlotState::Zombie { kind, .. } => {
                    counts.zombies = counts.zombies.saturating_add(1);
                    increment_kind_count(&mut counts, *kind);
                }
                SlotState::Vacant => counts.vacant = counts.vacant.saturating_add(1),
                SlotState::Retired => counts.retired = counts.retired.saturating_add(1),
            }
        }
        for slot in &self.leaf_slots {
            match slot.value.kind() {
                Some(kind) => {
                    increment_kind_count(&mut counts, kind);
                    if slot.strong.get() == 0 {
                        counts.zero_queued = counts.zero_queued.saturating_add(1);
                    } else {
                        counts.live = counts.live.saturating_add(1);
                    }
                }
                None => {
                    if matches!(slot.value, LeafValue::Retired) {
                        counts.retired = counts.retired.saturating_add(1);
                    } else {
                        counts.vacant = counts.vacant.saturating_add(1);
                    }
                }
            }
        }
        for slot in &self.var_refs.slots {
            match &slot.state {
                AuxiliaryState::Initializing { .. } => {
                    counts.initializing = counts.initializing.saturating_add(1);
                    counts.var_ref_nodes = counts.var_ref_nodes.saturating_add(1);
                }
                AuxiliaryState::Live(_) => {
                    counts.live = counts.live.saturating_add(1);
                    counts.var_ref_nodes = counts.var_ref_nodes.saturating_add(1);
                }
                AuxiliaryState::ZeroQueued(_) => {
                    counts.zero_queued = counts.zero_queued.saturating_add(1);
                    counts.var_ref_nodes = counts.var_ref_nodes.saturating_add(1);
                }
                AuxiliaryState::Vacant => counts.vacant = counts.vacant.saturating_add(1),
                AuxiliaryState::Retired => counts.retired = counts.retired.saturating_add(1),
            }
        }
        for slot in &self.shapes.slots {
            match &slot.state {
                AuxiliaryState::Initializing { .. } => {
                    counts.initializing = counts.initializing.saturating_add(1);
                    counts.shape_nodes = counts.shape_nodes.saturating_add(1);
                }
                AuxiliaryState::Live(_) => {
                    counts.live = counts.live.saturating_add(1);
                    counts.shape_nodes = counts.shape_nodes.saturating_add(1);
                }
                AuxiliaryState::ZeroQueued(_) => {
                    counts.zero_queued = counts.zero_queued.saturating_add(1);
                    counts.shape_nodes = counts.shape_nodes.saturating_add(1);
                }
                AuxiliaryState::Vacant => counts.vacant = counts.vacant.saturating_add(1),
                AuxiliaryState::Retired => counts.retired = counts.retired.saturating_add(1),
            }
        }
        counts
    }

    #[inline]
    pub(in crate::engine::heap) fn reserve(
        &mut self,
        kind: HeapNodeKind,
    ) -> Result<(u32, u32), HeapError> {
        let (index, generation) = self.reserve_vacant()?;
        self.slots[index as usize].state = SlotState::Initializing { kind, strong: 1 };
        #[cfg(debug_assertions)]
        self.record_alloc_site(index, generation, kind);
        Ok((index, generation))
    }

    /// Reserve storage without transporting a wide node payload. Callers must
    /// publish immediately or install Initializing before any fallible work.
    fn reserve_vacant(&mut self) -> Result<(u32, u32), HeapError> {
        let index = if let Some(index) = self.free.pop() {
            index
        } else {
            let index = u32::try_from(self.slots.len()).map_err(|_| HeapError::Overflow {
                operation: "allocating an arena slot",
            })?;
            self.slots.push(ArenaSlot {
                generation: 1,
                state: SlotState::Vacant,
                weak_prev: None,
                weak_next: None,
            });
            index
        };
        let slot = self
            .slots
            .get_mut(index as usize)
            .ok_or(HeapError::Invariant("free list referenced a missing slot"))?;
        if !matches!(slot.state, SlotState::Vacant) {
            return Err(HeapError::Invariant(
                "free list referenced an occupied slot",
            ));
        }
        if slot.weak_prev.is_some() || slot.weak_next.is_some() {
            return Err(HeapError::Invariant(
                "free list referenced a linked weak-collection slot",
            ));
        }
        Ok((index, slot.generation))
    }

    /// Reserve one vacant leaf slot from the leaf free list.
    fn leaf_reserve_vacant(&mut self) -> Result<(u32, u32), HeapError> {
        let index = if let Some(index) = self.leaf_free.pop() {
            index
        } else {
            let index = u32::try_from(self.leaf_slots.len()).map_err(|_| HeapError::Overflow {
                operation: "allocating a leaf arena slot",
            })?;
            self.leaf_slots.push(LeafSlot {
                generation: 1,
                strong: Cell::new(0),
                value: LeafValue::Vacant,
            });
            index
        };
        let slot = self
            .leaf_slots
            .get_mut(index as usize)
            .ok_or(HeapError::Invariant(
                "leaf free list referenced a missing slot",
            ))?;
        if !matches!(slot.value, LeafValue::Vacant) {
            return Err(HeapError::Invariant(
                "leaf free list referenced an occupied slot",
            ));
        }
        Ok((index, slot.generation))
    }

    // Leaf payloads own no outgoing heap edges and never carry weak links, so
    // they live in their own compact arena.
    pub(in crate::engine::heap) fn allocate_string_leaf(
        &mut self,
        value: JsString,
    ) -> Result<StringId, HeapError> {
        let (index, generation) = self.leaf_reserve_vacant()?;
        let slot = &mut self.leaf_slots[index as usize];
        slot.strong.set(1);
        slot.value = LeafValue::String(value);
        #[cfg(debug_assertions)]
        self.record_leaf_alloc_site(index, generation, HeapNodeKind::String);
        Ok(StringId { index, generation })
    }

    pub(in crate::engine::heap) fn allocate_bigint_leaf(
        &mut self,
        value: JsBigInt,
    ) -> Result<BigIntId, HeapError> {
        let (index, generation) = self.leaf_reserve_vacant()?;
        let slot = &mut self.leaf_slots[index as usize];
        slot.strong.set(1);
        slot.value = LeafValue::BigInt(value);
        #[cfg(debug_assertions)]
        self.record_leaf_alloc_site(index, generation, HeapNodeKind::BigInt);
        Ok(BigIntId { index, generation })
    }

    pub(in crate::engine::heap) fn abort_initializing(
        &mut self,
        index: u32,
    ) -> Result<(), HeapError> {
        let slot = self
            .slots
            .get_mut(index as usize)
            .ok_or(HeapError::Invariant("initializing slot disappeared"))?;
        if !matches!(slot.state, SlotState::Initializing { .. }) {
            return Err(HeapError::Invariant(
                "attempted to abort a published arena slot",
            ));
        }
        slot.state = SlotState::Vacant;
        self.free.push(index);
        #[cfg(debug_assertions)]
        self.clear_alloc_site(index);
        Ok(())
    }

    // Expose the concrete payload variant to allocation sites, so leaf nodes
    // do not travel through an opaque wide-enum copy in no-LTO builds.
    #[inline(always)]
    pub(in crate::engine::heap) fn publish(
        &mut self,
        index: u32,
        data: NodeData,
    ) -> Result<(), HeapError> {
        let expected = data.kind();
        let slot = self
            .slots
            .get_mut(index as usize)
            .ok_or(HeapError::Invariant("initializing slot disappeared"))?;
        let (kind, strong) = match &slot.state {
            SlotState::Initializing { kind, strong } => (*kind, *strong),
            _ => {
                return Err(HeapError::Invariant(
                    "attempted to publish a non-initializing slot",
                ));
            }
        };
        if kind != expected || strong != 1 {
            return Err(HeapError::Invariant(
                "initializing slot metadata did not match its payload",
            ));
        }
        slot.state = SlotState::Resident(Node {
            strong: Cell::new(strong),
            data,
        });
        self.record_cycle_allocation();
        Ok(())
    }

    /// Validate a leaf handle against the leaf arena, returning its slot index.
    ///
    /// Accepts live and zero-queued payloads; dead slots and wrong-kind
    /// handles surface the same diagnostics as [`Heap::validate_slot_identity`].
    pub(in crate::engine::heap) fn validate_leaf_identity(
        &self,
        id: RawId,
    ) -> Result<usize, HeapError> {
        debug_assert!(id.is_leaf(), "non-leaf handle reached the leaf arena");
        let index = id.index() as usize;
        let slot = self.leaf_slots.get(index).ok_or(HeapError::Stale {
            index: id.index(),
            generation: id.generation(),
        })?;
        if slot.generation != id.generation() {
            return Err(HeapError::Stale {
                index: id.index(),
                generation: id.generation(),
            });
        }
        let actual = slot.value.kind().ok_or(HeapError::Stale {
            index: id.index(),
            generation: id.generation(),
        })?;
        if actual != id.kind() {
            return Err(HeapError::WrongKind {
                expected: id.kind(),
                actual,
            });
        }
        Ok(index)
    }

    /// Shared-borrow leaf slot in any payload state, for count diagnostics.
    /// Root promotion still uses the checked live accessor.
    fn leaf_slot(&self, id: RawId) -> Result<&LeafSlot, HeapError> {
        let index = self.validate_leaf_identity(id)?;
        Ok(&self.leaf_slots[index])
    }

    /// Shared-borrow leaf slot for a live handle; a zero-queued or dead slot
    /// declines with `Stale`, matching the shared arena's [`Heap::live_node`].
    pub(in crate::engine::heap) fn live_leaf_slot(
        &self,
        id: RawId,
    ) -> Result<&LeafSlot, HeapError> {
        let index = self.validate_leaf_identity(id)?;
        let slot = &self.leaf_slots[index];
        if !slot.is_live() {
            return Err(HeapError::Stale {
                index: id.index(),
                generation: id.generation(),
            });
        }
        Ok(slot)
    }

    pub(in crate::engine::heap) fn live_leaf_slot_mut(
        &mut self,
        id: RawId,
    ) -> Result<&mut LeafSlot, HeapError> {
        let index = self.validate_leaf_identity(id)?;
        let slot = &mut self.leaf_slots[index];
        if !slot.is_live() {
            return Err(HeapError::Stale {
                index: id.index(),
                generation: id.generation(),
            });
        }
        Ok(slot)
    }

    /// Trusted leaf accessor paired with [`Heap::live_node_fast`].
    #[inline(always)]
    pub(in crate::engine::heap) fn live_leaf_fast(&self, id: RawId) -> &LeafSlot {
        assert_trusted_handle!(
            self.validate_leaf_identity(id).is_ok(),
            "trusted leaf handle failed its debug identity check"
        );
        let slot = &self.leaf_slots[id.index() as usize];
        if slot.is_live() {
            slot
        } else {
            unreachable!("trusted leaf handle reached a non-live slot")
        }
    }

    /// Trusted mutable leaf accessor paired with [`Heap::live_leaf_fast`].
    #[inline]
    pub(in crate::engine::heap) fn live_leaf_fast_mut(&mut self, id: RawId) -> &mut LeafSlot {
        assert_trusted_handle!(
            self.validate_leaf_identity(id).is_ok(),
            "trusted leaf handle failed its debug identity check"
        );
        let slot = &mut self.leaf_slots[id.index() as usize];
        if slot.is_live() {
            slot
        } else {
            unreachable!("trusted leaf handle reached a non-live slot")
        }
    }

    pub(in crate::engine::heap) fn live_index(&self, id: RawId) -> Result<usize, HeapError> {
        let index = self.validate_slot_identity(id)?;
        if !matches!(&self.slots[index].state, SlotState::Resident(node) if node.strong.get() != 0)
        {
            return Err(HeapError::Invariant(
                "heap edge targeted a node outside Live state",
            ));
        }
        Ok(index)
    }

    pub(in crate::engine::heap) fn live_node(&self, id: RawId) -> Result<&Node, HeapError> {
        let index = self.validate_slot_identity(id)?;
        match &self.slots[index].state {
            SlotState::Resident(node) if node.strong.get() != 0 => Ok(node),
            _ => Err(HeapError::Stale {
                index: id.index(),
                generation: id.generation(),
            }),
        }
    }

    /// Trusted accessor for a handle that a live owning edge keeps valid.
    ///
    /// The generation and nonzero-count checks run only in debug and
    /// `checked-handles` builds; release builds keep the `Vec` bounds check
    /// and the resident-slot check. A zero-count resident node is still valid
    /// memory, so skipping that check cannot read outside safe storage; it only
    /// moves detection of a counting bug to the checked builds. A non-live
    /// slot or wrong kind at a trusted call site is a heap invariant violation,
    /// so it panics rather than returning an error. General and untrusted
    /// callers must keep using [`Heap::live_node`].
    #[inline(always)]
    pub(in crate::engine::heap) fn live_node_fast(&self, id: RawId) -> &Node {
        assert_trusted_handle!(
            self.validate_slot_identity(id).is_ok(),
            "trusted handle failed its debug identity check"
        );
        let SlotState::Resident(node) = &self.slots[id.index() as usize].state else {
            unreachable!("trusted handle reached a non-live slot")
        };
        assert_trusted_handle!(
            node.strong.get() != 0,
            "trusted handle reached a zero-count node"
        );
        node
    }

    /// Trusted object access with one combined slot-state and payload test.
    #[inline(always)]
    pub(in crate::engine::heap) fn object_node_fast(&self, id: ObjectId) -> &ObjectData {
        assert_trusted_handle!(
            self.validate_slot_identity(RawId::Object(id)).is_ok(),
            "trusted handle failed its debug identity check"
        );
        let SlotState::Resident(Node {
            strong,
            data: NodeData::Object(object),
        }) = &self.slots[RawId::Object(id).index() as usize].state
        else {
            unreachable!("trusted object handle reached a non-object slot")
        };
        assert_trusted_handle!(
            strong.get() != 0,
            "trusted handle reached a zero-count node"
        );
        object
    }

    /// Trusted mutable access for an object handle validated earlier under
    /// the same exclusive borrow. Release builds keep only the bounds check.
    #[inline(always)]
    pub(in crate::engine::heap) fn object_mut_fast(&mut self, id: ObjectId) -> &mut ObjectData {
        assert_trusted_handle!(
            self.validate_slot_identity(RawId::Object(id)).is_ok(),
            "trusted handle failed its debug identity check"
        );
        match &mut self.slots[RawId::Object(id).index() as usize].state {
            SlotState::Resident(Node {
                data: NodeData::Object(object),
                ..
            }) => object,
            _ => unreachable!("trusted object handle reached a non-object slot"),
        }
    }

    pub(in crate::engine::heap) fn live_node_mut(
        &mut self,
        id: RawId,
    ) -> Result<&mut Node, HeapError> {
        let index = self.validate_slot_identity(id)?;
        match &mut self.slots[index].state {
            SlotState::Resident(node) if node.strong.get() != 0 => Ok(node),
            _ => Err(HeapError::Stale {
                index: id.index(),
                generation: id.generation(),
            }),
        }
    }

    pub(in crate::engine::heap) fn object_mut(
        &mut self,
        id: ObjectId,
    ) -> Result<&mut ObjectData, HeapError> {
        match &mut self.live_node_mut(RawId::Object(id))?.data {
            NodeData::Object(object) => Ok(object),
            NodeData::Context(_) | NodeData::FunctionBytecode(_) => Err(HeapError::Invariant(
                "typed object lookup reached another node payload",
            )),
        }
    }

    pub(in crate::engine::heap) fn var_ref_mut(
        &mut self,
        id: VarRefId,
    ) -> Result<&mut VarRefData, HeapError> {
        Ok(&mut self.var_refs.live_mut(id)?.data)
    }

    /// Strong count for a shared-arena handle. Leaf callers use
    /// [`Heap::leaf_slot`] instead; keeping this branch-free preserves the
    /// inlining of the typed diagnostics on the property-deletion path.
    #[inline]
    fn shared_strong_count(&self, id: RawId) -> Result<u32, HeapError> {
        let index = self.validate_slot_identity(id)?;
        self.slots[index].state.strong().ok_or(HeapError::Stale {
            index: id.index(),
            generation: id.generation(),
        })
    }

    /// Read a checked reference count without promoting an owner. Admission
    /// uses this to leave saturation transitions to the canonical protocol.
    pub(crate) fn strong_count(&self, id: RawId) -> Result<u32, HeapError> {
        if id.is_leaf() {
            return Ok(self.leaf_slot(id)?.strong.get());
        }
        if let RawId::VarRef(cell) = id {
            return self.var_refs.strong_count(cell);
        }
        if let RawId::Shape(shape) = id {
            return self.shapes.strong_count(shape);
        }
        self.shared_strong_count(id)
    }

    /// Overwrite one live node's strong count for saturation tests.
    #[cfg(test)]
    pub(crate) fn set_strong_count_for_test(&mut self, id: RawId, count: u32) {
        if id.is_leaf() {
            self.live_leaf_fast_mut(id).strong.set(count);
            return;
        }
        if let RawId::VarRef(cell) = id {
            self.var_refs.live_fast_mut(cell).strong.set(count);
            return;
        }
        if let RawId::Shape(shape) = id {
            self.shapes.live_fast_mut(shape).strong.set(count);
            return;
        }
        // Fault-injection fixtures also repair intentionally zeroed resident
        // counts. Production live access must keep rejecting such nodes.
        let index = self.validate_slot_identity(id).unwrap();
        let SlotState::Resident(node) = &self.slots[index].state else {
            panic!("count fixture requires a resident node");
        };
        node.strong.set(count);
    }

    /// Consume a test fixture's edge while leaving zero cleanup pending.
    #[cfg(test)]
    pub(crate) fn queue_release_for_test(&mut self, id: RawId) -> Result<(), HeapError> {
        self.release_raw_no_drain(id)
    }

    pub(in crate::engine::heap) fn is_live(&self, id: RawId) -> bool {
        if let RawId::VarRef(cell) = id {
            return self.var_refs.is_live(cell);
        }
        if let RawId::Shape(shape) = id {
            return self.shapes.is_live(shape);
        }
        if id.is_leaf() {
            return self
                .validate_leaf_identity(id)
                .is_ok_and(|index| self.leaf_slots[index].is_live());
        }
        self.validate_slot_identity(id)
            .is_ok_and(|index| matches!(&self.slots[index].state, SlotState::Resident(node) if node.strong.get() != 0))
    }
}
