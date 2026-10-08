//! Per-site facts for adding one missing data property at a static-key store.
//! Entries own no shape, object, or value. A hit proves under the caller's
//! borrow that the receiver still has the recorded parent layout and that no
//! prototype layout changed since the miss selected "define on the receiver";
//! the caller then publishes the recorded successor without another selection.
//! The same site also records an existing writable own data slot, so a store
//! to that slot skips the own-key selection.
use std::cell::Cell;

use crate::engine::heap::{Heap, ObjectData, ObjectKind, ObjectPayload, ShapeId};

/// An existing writable own data property of a plain ordinary receiver.
/// Writability and the slot index are part of the layout: reconfiguring a
/// property gives the object a new shape or advances its shape's revision.
#[derive(Clone, Copy, Debug)]
struct ExistingLocation {
    domain: u64,
    shape: ShapeId,
    revision: u64,
    slot: u32,
}

fn plain_ordinary(receiver: &ObjectData) -> bool {
    matches!(
        (receiver.kind, &receiver.payload),
        (ObjectKind::Ordinary, ObjectPayload::Ordinary)
    )
}

#[derive(Clone, Copy, Debug)]
struct AppendLocation {
    domain: u64,
    parent: ShapeId,
    parent_revision: u64,
    successor: ShapeId,
    successor_revision: u64,
    /// The prototype layout epoch observed by the miss's chain walk; `None`
    /// when the parent layout has no prototype to consult.
    prototype_epoch: Option<u64>,
}

/// The miss facts a successful general append offers to its site.
#[derive(Clone, Copy)]
pub(crate) struct AppendMiss {
    pub(crate) domain: u64,
    pub(crate) parent: ShapeId,
    pub(crate) parent_revision: u64,
    pub(crate) has_prototype: bool,
    pub(crate) prototype_epoch: u64,
}

/// One monomorphic entry: constructor and literal sites see one parent
/// layout. Another layout misses and replaces the entry after its own append.
#[derive(Debug, Default)]
pub(crate) struct PropertyAppendCache {
    location: Cell<Option<AppendLocation>>,
    existing: Cell<Option<ExistingLocation>>,
    /// Misses of a learned existing-slot fact since it was learned. A site
    /// that alternates layouts replaces its fact only after this many misses
    /// instead of relearning on every store.
    existing_misses: Cell<u8>,
}

const EXISTING_RELEARN_MISSES: u8 = 16;

impl PropertyAppendCache {
    /// The slot of the key when `receiver` still has the layout this site
    /// learned it as a writable own data property. The prototype chain is not
    /// consulted: an own writable data property decides [[Set]] on itself.
    #[inline]
    pub(crate) fn existing_slot(
        &self,
        heap: &Heap,
        domain: u64,
        receiver: &ObjectData,
    ) -> Option<usize> {
        let location = self.existing.get()?;
        if location.domain != domain
            || receiver.shape != location.shape
            || !plain_ordinary(receiver)
        {
            return None;
        }
        (heap.shape_fast(location.shape).layout_revision() == location.revision)
            .then_some(location.slot as usize)
    }

    /// Count a miss of the learned existing-slot fact.
    pub(crate) fn existing_missed(&self) {
        if self.existing.get().is_some() {
            self.existing_misses
                .set(self.existing_misses.get().saturating_add(1));
        }
    }

    /// An empty site learns at once; a learned one only after enough misses.
    pub(crate) fn should_learn_existing(&self) -> bool {
        self.existing.get().is_none() || self.existing_misses.get() >= EXISTING_RELEARN_MISSES
    }

    /// Record the writable own data slot an ordinary selection just stored
    /// into. Dictionary layouts and saturated revisions stay uncached.
    pub(crate) fn learn_existing(
        &self,
        heap: &Heap,
        domain: u64,
        receiver: &ObjectData,
        slot: usize,
    ) -> bool {
        let (Ok(shape), Ok(slot)) = (heap.shape(receiver.shape), u32::try_from(slot)) else {
            return false;
        };
        if !plain_ordinary(receiver) || shape.is_dictionary() || shape.layout_revision() == u64::MAX
        {
            return false;
        }
        self.existing_misses.set(0);
        self.existing.set(Some(ExistingLocation {
            domain,
            shape: receiver.shape,
            revision: shape.layout_revision(),
            slot,
        }));
        true
    }

    /// The successor for `receiver`'s current layout when this site's fact
    /// still holds. The caller already selected the key as a missing own
    /// property of an ordinary-set receiver under the same borrow.
    #[inline]
    pub(crate) fn successor(
        &self,
        heap: &Heap,
        domain: u64,
        receiver: &ObjectData,
    ) -> Option<ShapeId> {
        let location = self.location.get()?;
        if location.domain != domain || receiver.shape != location.parent || !receiver.extensible {
            return None;
        }
        // The receiver owns its parent shape; in-place layout mutation of a
        // uniquely owned or dictionary shape advances its revision.
        if heap.shape_fast(location.parent).layout_revision() != location.parent_revision {
            return None;
        }
        // Every prototype is marked when a shape first names it, and any
        // layout change of a marked object advances this epoch.
        if location
            .prototype_epoch
            .is_some_and(|epoch| heap.property_layout_epoch() != epoch)
        {
            return None;
        }
        // The successor is weak: its generation proves identity and its
        // revision proves the layout recorded when this site learned it.
        let successor = heap.shape(location.successor).ok()?;
        (successor.layout_revision() == location.successor_revision).then_some(location.successor)
    }

    /// Record the transition just published by a general append. Unique
    /// in-place appends, dictionaries and saturated counters stay uncached.
    pub(crate) fn learn(&self, heap: &Heap, miss: AppendMiss, successor: ShapeId) -> bool {
        if successor == miss.parent
            || miss.parent_revision == u64::MAX
            || (miss.has_prototype && miss.prototype_epoch == u64::MAX)
        {
            return false;
        }
        let Ok(shape) = heap.shape(successor) else {
            return false;
        };
        if shape.is_dictionary() || shape.layout_revision() == u64::MAX {
            return false;
        }
        self.location.set(Some(AppendLocation {
            domain: miss.domain,
            parent: miss.parent,
            parent_revision: miss.parent_revision,
            successor,
            successor_revision: shape.layout_revision(),
            prototype_epoch: miss.has_prototype.then_some(miss.prototype_epoch),
        }));
        true
    }
}

/// Which consumer a site serves. Stores and definitions keep separate learned
/// bitmaps, so a function whose literal definitions learn facts does not make
/// its existing-slot stores test a populated bitmap. `Existing` marks store
/// sites that learned an existing writable slot, so append-only sites never
/// call into the existing-slot check.
#[derive(Clone, Copy)]
pub(crate) enum AppendKind {
    Store,
    Definition,
    Existing,
}

/// Append facts for every static-key store and definition site of one
/// executable. Dispatch tests only the learned bit for its PC; a site's entry
/// is addressed only once it has learned a fact. Each bitmap is allocated on
/// its kind's first learned fact, so executables that never append keep no
/// extra allocation and their stores test one empty cell.
#[derive(Debug)]
pub(crate) struct PropertyAppendCacheTable {
    sites: super::property_ic::SiteCacheTable<PropertyAppendCache>,
    stores: std::cell::OnceCell<Box<[Cell<u64>]>>,
    definitions: std::cell::OnceCell<Box<[Cell<u64>]>>,
    existing: std::cell::OnceCell<Box<[Cell<u64>]>>,
}

impl PropertyAppendCacheTable {
    pub(crate) fn new_exec(code: &crate::engine::code::exec::ExecCode) -> Self {
        Self {
            sites: super::property_ic::SiteCacheTable::<PropertyAppendCache>::new_exec(code),
            stores: std::cell::OnceCell::new(),
            definitions: std::cell::OnceCell::new(),
            existing: std::cell::OnceCell::new(),
        }
    }

    #[inline(always)]
    fn learned(&self, kind: AppendKind) -> &std::cell::OnceCell<Box<[Cell<u64>]>> {
        match kind {
            AppendKind::Store => &self.stores,
            AppendKind::Definition => &self.definitions,
            AppendKind::Existing => &self.existing,
        }
    }

    #[inline(always)]
    pub(crate) fn has_fact(&self, kind: AppendKind, pc: usize) -> bool {
        self.learned(kind).get().is_some_and(|learned| {
            learned
                .get(pc / 64)
                .is_some_and(|word| word.get() & (1u64 << (pc % 64)) != 0)
        })
    }

    pub(crate) fn site(&self, pc: usize) -> Option<&PropertyAppendCache> {
        self.sites.site(pc)
    }

    /// Record a general append at `pc`; the bit stays set once learned, and a
    /// stale fact only costs its site one failed check.
    pub(crate) fn learn(
        &self,
        kind: AppendKind,
        pc: usize,
        heap: &Heap,
        miss: AppendMiss,
        successor: ShapeId,
    ) {
        if !self
            .site(pc)
            .is_some_and(|site| site.learn(heap, miss, successor))
        {
            return;
        }
        self.mark(kind, pc);
    }

    /// Set the learned bit of `pc` for `kind`; it stays set once learned.
    pub(crate) fn mark(&self, kind: AppendKind, pc: usize) {
        let learned = self
            .learned(kind)
            .get_or_init(|| (0..self.sites.pc_words()).map(|_| Cell::new(0)).collect());
        if let Some(word) = learned.get(pc / 64) {
            word.set(word.get() | 1u64 << (pc % 64));
        }
    }
}
