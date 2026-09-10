//! Dictionary transitions stay at the runtime boundary so atom ownership and
//! weak shape-cache entries remain consistent with heap layout transactions.

use super::shape::{Shape, ShapeEntry};
use crate::engine::api::runtime_error::RuntimeError;
use crate::engine::heap::runtime::RuntimeState;
use crate::engine::heap::{ObjectId, PropertySlot, ShapeId};
use std::collections::HashMap;

impl RuntimeState {
    pub(crate) fn ensure_dictionary_layout(
        &mut self,
        object: ObjectId,
    ) -> Result<(), RuntimeError> {
        let shape_id = self.heap.object(object)?.shape;
        if self.heap.shape_strong_count(shape_id)? == 1 {
            self.heap.enable_object_dictionary(object)?;
            // Metadata conversion preserves entries; unlink before the first
            // mutation changes the fingerprint of this exclusively owned shape.
            self.unlink_finalized_shapes([shape_id]);
            return Ok(());
        }
        let mut shape = self.heap.shape(shape_id)?.clone();
        shape.enable_dictionary();
        let slots = self.heap.object(object)?.slots.clone();
        let shape_id = self.allocate_uncached_shape(shape)?;
        self.replace_layout_with_owned_shape(object, shape_id, slots)
    }

    fn allocate_uncached_shape(&mut self, shape: Shape) -> Result<ShapeId, RuntimeError> {
        let atoms = self.retain_shape_atoms(shape.entries())?;
        match self.heap.allocate_shape(shape) {
            Ok(id) => Ok(id),
            Err(error) => {
                self.release_atoms(atoms)?;
                Err(error.into())
            }
        }
    }

    /// Rare whole-layout operations supply physical entries and parallel slots.
    /// Restore semantic insertion order before rebuilding, preserving dictionary
    /// mode through prototype/private-element changes and shared-shape detaches.
    pub(crate) fn replace_dictionary_layout(
        &mut self,
        object: ObjectId,
        prototype: Option<ObjectId>,
        entries: &[ShapeEntry],
        slots: Vec<PropertySlot>,
    ) -> Result<(), RuntimeError> {
        if entries.len() != slots.len() {
            return Err(RuntimeError::Invariant(
                "dictionary replacement has mismatched entries and slots",
            ));
        }
        let mut incoming = HashMap::with_capacity(entries.len());
        for (index, entry) in entries.iter().enumerate() {
            if incoming.insert(entry.atom, index).is_some() {
                return Err(super::shape::ShapeError::DuplicateAtom(entry.atom).into());
            }
        }
        let mut pairs = entries
            .iter()
            .copied()
            .zip(slots)
            .map(Some)
            .collect::<Vec<_>>();
        let mut ordered = Vec::with_capacity(pairs.len());
        let shape = self.heap.shape(self.heap.object(object)?.shape)?;
        for index in shape.ordered_indices() {
            if let Some(&replacement) = incoming.get(&shape.entries()[index].atom) {
                if let Some(pair) = pairs[replacement].take() {
                    ordered.push(pair);
                }
            }
        }
        ordered.extend(pairs.into_iter().flatten());
        let (entries, slots): (Vec<_>, Vec<_>) = ordered.into_iter().unzip();
        let mut shape = Shape::new(prototype, entries)?;
        shape.enable_dictionary();
        let shape_id = self.allocate_uncached_shape(shape)?;
        self.replace_layout_with_owned_shape(object, shape_id, slots)
    }
}
