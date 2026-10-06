//! Per-site facts for adding one missing data property at a static-key store.
//! Entries own no shape, object, or value. A hit proves under the caller's
//! borrow that the receiver still has the recorded parent layout and that no
//! prototype layout changed since the miss selected "define on the receiver";
//! the caller then publishes the recorded successor without another selection.
use std::cell::Cell;

use crate::engine::heap::{Heap, ObjectData, ShapeId};

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
}

impl PropertyAppendCache {
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
    pub(crate) fn learn(&self, heap: &Heap, miss: AppendMiss, successor: ShapeId) {
        if successor == miss.parent
            || miss.parent_revision == u64::MAX
            || (miss.has_prototype && miss.prototype_epoch == u64::MAX)
        {
            return;
        }
        let Ok(shape) = heap.shape(successor) else {
            return;
        };
        if shape.is_dictionary() || shape.layout_revision() == u64::MAX {
            return;
        }
        self.location.set(Some(AppendLocation {
            domain: miss.domain,
            parent: miss.parent,
            parent_revision: miss.parent_revision,
            successor,
            successor_revision: shape.layout_revision(),
            prototype_epoch: miss.has_prototype.then_some(miss.prototype_epoch),
        }));
    }
}
