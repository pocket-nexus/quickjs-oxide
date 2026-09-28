//! Compact generational storage for non-anchor nodes with owned outgoing edges.
//!
//! This layer manages slot identity and lifecycle only. The heap retains and
//! releases payload edges, returns atom cleanup, and drives the zero queue.

use super::*;

/// The payload fixes both the accepted slot identity and its diagnostic kind.
/// Heterogeneous `RawId` dispatch belongs to `Heap`, before entering an arena.
pub(super) trait AuxiliaryPayload: Sized {
    type Id: Copy;
    const KIND: HeapNodeKind;

    fn id(index: u32, generation: u32) -> Self::Id;
    fn parts(id: Self::Id) -> (u32, u32);
}

impl AuxiliaryPayload for VarRefData {
    type Id = VarRefId;
    const KIND: HeapNodeKind = HeapNodeKind::VarRef;

    fn id(index: u32, generation: u32) -> Self::Id {
        VarRefId { index, generation }
    }

    fn parts(id: Self::Id) -> (u32, u32) {
        (id.index, id.generation)
    }
}

impl AuxiliaryPayload for Shape {
    type Id = ShapeId;
    const KIND: HeapNodeKind = HeapNodeKind::Shape;

    fn id(index: u32, generation: u32) -> Self::Id {
        ShapeId { index, generation }
    }

    fn parts(id: Self::Id) -> (u32, u32) {
        (id.index, id.generation)
    }
}

pub(super) struct AuxiliaryNode<T> {
    pub(super) strong: Cell<u32>,
    pub(super) data: T,
}

pub(super) enum AuxiliaryState<T> {
    Vacant,
    Initializing { strong: u32 },
    Live(AuxiliaryNode<T>),
    ZeroQueued(AuxiliaryNode<T>),
    Retired,
}

impl<T> AuxiliaryState<T> {
    pub(super) fn strong(&self) -> Option<u32> {
        match self {
            Self::Initializing { strong } => Some(*strong),
            Self::Live(node) | Self::ZeroQueued(node) => Some(node.strong.get()),
            Self::Vacant | Self::Retired => None,
        }
    }
}

pub(super) struct AuxiliarySlot<T> {
    pub(super) generation: u32,
    pub(super) state: AuxiliaryState<T>,
}

pub(super) struct AuxiliaryArena<T: AuxiliaryPayload> {
    #[cfg(not(feature = "profiling"))]
    pub(super) slots: Vec<AuxiliarySlot<T>>,
    #[cfg(feature = "profiling")]
    pub(super) slots: profiling::ArenaStorage<AuxiliarySlot<T>>,
    pub(super) free: Vec<u32>,
    #[cfg(debug_assertions)]
    pub(super) alloc_sites: Vec<Option<gc::AllocSite>>,
}

impl<T: AuxiliaryPayload> AuxiliaryArena<T> {
    pub(super) const fn new(allocation_id: u64) -> Self {
        #[cfg(not(feature = "profiling"))]
        let _ = allocation_id;
        Self {
            #[cfg(not(feature = "profiling"))]
            slots: Vec::new(),
            #[cfg(feature = "profiling")]
            slots: profiling::ArenaStorage::new(allocation_id),
            free: Vec::new(),
            #[cfg(debug_assertions)]
            alloc_sites: Vec::new(),
        }
    }

    pub(super) fn reserve(&mut self) -> Result<T::Id, HeapError> {
        let index = if let Some(index) = self.free.pop() {
            index
        } else {
            let index = u32::try_from(self.slots.len()).map_err(|_| HeapError::Overflow {
                operation: "allocating an auxiliary arena slot",
            })?;
            self.slots.push(AuxiliarySlot {
                generation: 1,
                state: AuxiliaryState::Vacant,
            });
            index
        };
        let slot = self
            .slots
            .get_mut(index as usize)
            .ok_or(HeapError::Invariant(
                "auxiliary free list referenced a missing slot",
            ))?;
        if !matches!(slot.state, AuxiliaryState::Vacant) {
            return Err(HeapError::Invariant(
                "auxiliary free list referenced an occupied slot",
            ));
        }
        slot.state = AuxiliaryState::Initializing { strong: 1 };
        let generation = slot.generation;
        #[cfg(debug_assertions)]
        self.record_alloc_site(index, generation);
        Ok(T::id(index, generation))
    }

    pub(super) fn abort_initializing(&mut self, id: T::Id) -> Result<(), HeapError> {
        let (index, generation) = T::parts(id);
        let slot = self
            .slots
            .get_mut(index as usize)
            .ok_or(HeapError::Invariant(
                "initializing auxiliary slot disappeared",
            ))?;
        if slot.generation != generation
            || !matches!(slot.state, AuxiliaryState::Initializing { .. })
        {
            return Err(HeapError::Invariant(
                "attempted to abort a published auxiliary slot",
            ));
        }
        slot.state = AuxiliaryState::Vacant;
        self.free.push(index);
        #[cfg(debug_assertions)]
        self.clear_alloc_site(index);
        Ok(())
    }

    pub(super) fn publish(&mut self, id: T::Id, data: T) -> Result<(), HeapError> {
        let (index, generation) = T::parts(id);
        let slot = self
            .slots
            .get_mut(index as usize)
            .ok_or(HeapError::Invariant(
                "initializing auxiliary slot disappeared",
            ))?;
        if slot.generation != generation
            || !matches!(slot.state, AuxiliaryState::Initializing { strong: 1 })
        {
            return Err(HeapError::Invariant(
                "auxiliary slot was not singly owned before publication",
            ));
        }
        slot.state = AuxiliaryState::Live(AuxiliaryNode {
            strong: Cell::new(1),
            data,
        });
        Ok(())
    }

    pub(super) fn validate_identity(&self, id: T::Id) -> Result<usize, HeapError> {
        let (raw_index, generation) = T::parts(id);
        let index = raw_index as usize;
        let slot = self.slots.get(index).ok_or(HeapError::Stale {
            index: raw_index,
            generation,
        })?;
        if slot.generation != generation
            || matches!(slot.state, AuxiliaryState::Vacant | AuxiliaryState::Retired)
        {
            return Err(HeapError::Stale {
                index: raw_index,
                generation,
            });
        }
        Ok(index)
    }

    pub(super) fn live(&self, id: T::Id) -> Result<&AuxiliaryNode<T>, HeapError> {
        let index = self.validate_identity(id)?;
        let (raw_index, generation) = T::parts(id);
        match &self.slots[index].state {
            AuxiliaryState::Live(node) => Ok(node),
            _ => Err(HeapError::Stale {
                index: raw_index,
                generation,
            }),
        }
    }

    pub(super) fn live_mut(&mut self, id: T::Id) -> Result<&mut AuxiliaryNode<T>, HeapError> {
        let index = self.validate_identity(id)?;
        let (raw_index, generation) = T::parts(id);
        match &mut self.slots[index].state {
            AuxiliaryState::Live(node) => Ok(node),
            _ => Err(HeapError::Stale {
                index: raw_index,
                generation,
            }),
        }
    }

    pub(super) fn live_fast(&self, id: T::Id) -> &AuxiliaryNode<T> {
        debug_assert!(self.validate_identity(id).is_ok());
        match &self.slots[T::parts(id).0 as usize].state {
            AuxiliaryState::Live(node) => node,
            _ => unreachable!("trusted auxiliary handle reached a non-live slot"),
        }
    }

    #[cfg(test)]
    pub(super) fn live_fast_mut(&mut self, id: T::Id) -> &mut AuxiliaryNode<T> {
        debug_assert!(self.validate_identity(id).is_ok());
        match &mut self.slots[T::parts(id).0 as usize].state {
            AuxiliaryState::Live(node) => node,
            _ => unreachable!("trusted auxiliary handle reached a non-live slot"),
        }
    }

    pub(super) fn strong_count(&self, id: T::Id) -> Result<u32, HeapError> {
        let index = self.validate_identity(id)?;
        let (raw_index, generation) = T::parts(id);
        self.slots[index].state.strong().ok_or(HeapError::Stale {
            index: raw_index,
            generation,
        })
    }

    pub(super) fn is_live(&self, id: T::Id) -> bool {
        self.validate_identity(id)
            .is_ok_and(|index| matches!(self.slots[index].state, AuxiliaryState::Live(_)))
    }

    /// Decrement an owned reference. The caller enqueues the handle on `true`.
    pub(super) fn release_no_drain(&mut self, id: T::Id) -> Result<bool, HeapError> {
        let index = self.validate_identity(id)?;
        let (raw_index, generation) = T::parts(id);
        let slot = &mut self.slots[index];
        match &mut slot.state {
            AuxiliaryState::Live(node) => {
                if node.strong.get() == gc::IMMORTAL_STRONG {
                    return Ok(false);
                }
                let strong = node
                    .strong
                    .get()
                    .checked_sub(1)
                    .ok_or(HeapError::Underflow {
                        kind: T::KIND,
                        index: raw_index,
                        generation,
                    })?;
                node.strong.set(strong);
                if strong == 0 {
                    let state = std::mem::replace(&mut slot.state, AuxiliaryState::Vacant);
                    let AuxiliaryState::Live(node) = state else {
                        unreachable!("auxiliary live state changed before queueing");
                    };
                    slot.state = AuxiliaryState::ZeroQueued(node);
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
            AuxiliaryState::Initializing { .. } | AuxiliaryState::ZeroQueued(_) => {
                Err(HeapError::Underflow {
                    kind: T::KIND,
                    index: raw_index,
                    generation,
                })
            }
            AuxiliaryState::Vacant | AuxiliaryState::Retired => Err(HeapError::Stale {
                index: raw_index,
                generation,
            }),
        }
    }

    pub(super) fn detach_zero_queued(&mut self, id: T::Id) -> Result<T, HeapError> {
        let index = self.validate_identity(id)?;
        let slot = &mut self.slots[index];
        if !matches!(slot.state, AuxiliaryState::ZeroQueued(_)) {
            return Err(HeapError::Invariant(
                "zero queue referenced an auxiliary node outside ZeroQueued state",
            ));
        }
        let state = std::mem::replace(&mut slot.state, AuxiliaryState::Vacant);
        let AuxiliaryState::ZeroQueued(node) = state else {
            unreachable!("validated auxiliary state changed before detachment");
        };
        if node.strong.get() != 0 {
            return Err(HeapError::Invariant(
                "zero queue contained a nonzero auxiliary reference count",
            ));
        }
        Ok(node.data)
    }

    pub(super) fn reclaim_vacant(&mut self, id: T::Id) -> Result<(), HeapError> {
        let (index, generation) = T::parts(id);
        let slot = self
            .slots
            .get_mut(index as usize)
            .ok_or(HeapError::Invariant("reclaimed auxiliary slot disappeared"))?;
        if slot.generation != generation || !matches!(slot.state, AuxiliaryState::Vacant) {
            return Err(HeapError::Invariant(
                "auxiliary generation advanced before payload detachment",
            ));
        }
        if let Some(generation) = slot.generation.checked_add(1) {
            slot.generation = generation;
            self.free.push(index);
        } else {
            slot.state = AuxiliaryState::Retired;
        }
        #[cfg(debug_assertions)]
        self.clear_alloc_site(index);
        Ok(())
    }

    #[cfg(debug_assertions)]
    fn record_alloc_site(&mut self, index: u32, generation: u32) {
        if !super::ownership::alloc_site_capture_enabled() {
            return;
        }
        let index = index as usize;
        if self.alloc_sites.len() <= index {
            self.alloc_sites.resize_with(index + 1, || None);
        }
        self.alloc_sites[index] = Some(gc::AllocSite {
            generation,
            kind: T::KIND,
            backtrace: super::ownership::compact_backtrace(),
        });
    }

    #[cfg(debug_assertions)]
    fn clear_alloc_site(&mut self, index: u32) {
        if let Some(site) = self.alloc_sites.get_mut(index as usize) {
            *site = None;
        }
    }
}
