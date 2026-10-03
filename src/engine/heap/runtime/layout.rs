//! Publication of a prepared object layout. Language-specific builders own
//! property order and descriptors; this boundary owns atom rollback and heap
//! publication. Raw slots and payloads borrow owners kept alive by the caller.

use super::*;
use crate::engine::heap::ObjectData;

impl RuntimeState {
    /// All callers carry the runtime's quarantine flag. Publication or
    /// destructive cleanup failure must stop temporary-owner traversal before
    /// this function returns, including at public object/Array boundaries.
    pub(crate) fn allocate_object_with_layout(
        &mut self,
        poisoned: &Cell<bool>,
        prototype: Option<ObjectId>,
        entries: &[ShapeEntry],
        slots: Vec<PropertySlot>,
        build: impl FnOnce(ShapeId, Vec<PropertySlot>) -> ObjectData,
    ) -> Result<ObjectId, RuntimeError> {
        let shape = self.get_or_create_shape(prototype, entries)?;
        // Selection precedes payload atom retention, as primitive Symbol
        // allocation requires. Concrete builders only pack their borrowed
        // inputs; the canonical finalizer visitor names every atom owner.
        let object = build(shape, slots);
        let mut atoms = Vec::new();
        for index in crate::engine::heap::gc::object_atoms(&object) {
            if let Err(error) = self.atoms.retain_index(index) {
                self.release_atoms(atoms)
                    .inspect_err(|_| poisoned.set(true))?;
                let cleanup = self
                    .heap
                    .release_shape(shape)
                    .inspect_err(|_| poisoned.set(true))?;
                self.apply_cleanup(cleanup)
                    .inspect_err(|_| poisoned.set(true))?;
                return Err(error.into());
            }
            atoms.push(Atom::from_raw(index.raw()));
        }
        let result = match self.heap.allocate_object_with_status(object) {
            Err(failure) if failure.published => {
                poisoned.set(true);
                return Err(failure.error.into());
            }
            result => result,
        };
        if let Err(failure) = &result {
            if !failure.published {
                self.release_atoms(atoms)
                    .inspect_err(|_| poisoned.set(true))?;
            }
        }
        let cleanup = self
            .heap
            .release_shape(shape)
            .inspect_err(|_| poisoned.set(true))?;
        self.apply_cleanup(cleanup)
            .inspect_err(|_| poisoned.set(true))?;
        result.map_err(|failure| failure.error.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::api::runtime::Runtime;
    use crate::engine::heap::RawId;
    use crate::engine::heap::runtime::owned_values::OwnedValueGuard;
    use crate::engine::object::shape::PropertyFlags;

    #[test]
    fn public_object_and_array_publication_quarantine_before_boundary_cleanup() {
        for array in [false, true] {
            let runtime = Runtime::new();
            let prototype = runtime.new_object(None).unwrap();
            // Reuse an admitted shape so the fault is in final publication
            // cleanup rather than shape creation or input authentication.
            let _matching_layout = if array {
                runtime.new_empty_array_with_prototype(&prototype).unwrap()
            } else {
                runtime.new_object(Some(&prototype)).unwrap()
            };
            let first = runtime.new_object(None).unwrap().into_handle();
            let later = runtime.new_object(None).unwrap().into_handle();
            let (objects, prototype_count) = {
                let mut state = runtime.0.state.borrow_mut();
                state
                    .heap
                    .queue_release_for_test(RawId::Object(first))
                    .unwrap();
                state
                    .heap
                    .queue_release_for_test(RawId::Object(later))
                    .unwrap();
                state
                    .heap
                    .set_strong_count_for_test(RawId::Object(first), 1);
                (
                    state.heap.counts().object_nodes,
                    state
                        .heap
                        .object_strong_count(prototype.object_id())
                        .unwrap(),
                )
            };
            let result = if array {
                runtime.new_empty_array_with_prototype(&prototype)
            } else {
                runtime.new_object(Some(&prototype))
            };
            assert!(result.is_err());
            assert!(runtime.is_poisoned());
            let state = runtime.0.state.borrow();
            assert_eq!(state.heap.counts().object_nodes, objects + 1);
            assert_eq!(
                state.heap.object_strong_count(prototype.object_id()),
                Ok(prototype_count),
            );
            assert_eq!(state.heap.object_strong_count(later), Ok(0));
            assert!(state.heap.has_pending_zero_cleanup());
            assert!(!runtime.0.deferred_references.has_pending());
        }
    }

    #[test]
    fn resident_object_allocation_quarantines_before_prototype_cleanup() {
        let runtime = Runtime::new();
        let context = runtime.new_context().unwrap();
        let prototype = runtime
            .0
            .state
            .borrow()
            .heap
            .context(context.realm)
            .unwrap()
            .object_prototype;
        let prototype_root =
            crate::engine::object::ObjectRef::from_borrowed_handle(runtime.clone(), prototype)
                .unwrap();
        let _matching_layout = runtime.new_object(Some(&prototype_root)).unwrap();
        let first = runtime.new_object(None).unwrap().into_handle();
        let later = runtime.new_object(None).unwrap().into_handle();
        let mut state = runtime.0.state.borrow_mut();
        let before = state.heap.counts();
        let prototype_count = state.heap.object_strong_count(prototype).unwrap();
        state
            .heap
            .queue_release_for_test(RawId::Object(first))
            .unwrap();
        state
            .heap
            .queue_release_for_test(RawId::Object(later))
            .unwrap();
        state
            .heap
            .set_strong_count_for_test(RawId::Object(first), 1);
        assert!(
            state
                .new_ordinary_object_in_realm(&runtime.0.poisoned, context.realm)
                .is_err()
        );
        assert!(runtime.is_poisoned());
        assert_eq!(
            state.heap.object_strong_count(prototype),
            Ok(prototype_count + 1)
        );
        assert_eq!(state.heap.object_strong_count(later), Ok(0));
        assert!(state.heap.has_pending_zero_cleanup());
        assert_eq!(state.heap.counts().object_nodes, before.object_nodes + 1);
    }

    #[test]
    fn prepared_layout_rejection_preserves_symbol_owner_and_runtime_access() {
        let runtime = Runtime::new();
        let key = runtime.intern_property_key("rejected-layout").unwrap();
        let symbol = runtime.new_symbol(None).unwrap();
        let mut runtime_state = runtime.0.state.borrow_mut();
        let before = runtime_state
            .atoms
            .resolve(symbol.atom())
            .unwrap()
            .ref_count;
        runtime_state.atoms.retain(symbol.atom()).unwrap();
        let mut producer = OwnedValueGuard::new(
            &mut runtime_state,
            &runtime.0.poisoned,
            crate::engine::value::JsValue::Symbol(AtomIdx::from_raw(symbol.atom().raw())),
        );
        let entries = [ShapeEntry {
            atom: AtomIdx::from_raw(key.atom().raw()),
            flags: PropertyFlags::accessor(true, true),
        }];
        {
            let (state, _) = producer.parts();
            let error = state.allocate_object_with_layout(
                &runtime.0.poisoned,
                None,
                &entries,
                vec![PropertySlot::Data(RawValue::Symbol(AtomIdx::from_raw(
                    symbol.atom().raw(),
                )))],
                ObjectData::ordinary,
            );
            assert!(error.is_err());
        }
        assert!(!runtime.is_poisoned());
        drop(producer);
        assert_eq!(
            runtime_state
                .atoms
                .resolve(symbol.atom())
                .unwrap()
                .ref_count,
            before
        );
    }

    #[test]
    fn published_weak_layout_failure_keeps_owned_symbol_edges_quarantined() {
        let runtime = Runtime::new();
        let key = runtime.intern_property_key("weak-layout-symbol").unwrap();
        let symbol = runtime.new_symbol(None).unwrap();
        let mut runtime_state = runtime.0.state.borrow_mut();
        runtime_state
            .allocate_object_with_layout(
                &runtime.0.poisoned,
                None,
                &[],
                Vec::new(),
                ObjectData::weak_map,
            )
            .unwrap();
        runtime_state.heap.weak_head = None;
        let before = runtime_state
            .atoms
            .resolve(symbol.atom())
            .unwrap()
            .ref_count;
        runtime_state.atoms.retain(symbol.atom()).unwrap();
        let mut producer = OwnedValueGuard::new(
            &mut runtime_state,
            &runtime.0.poisoned,
            crate::engine::value::JsValue::Symbol(AtomIdx::from_raw(symbol.atom().raw())),
        );
        {
            let (state, _) = producer.parts();
            let entries = [ShapeEntry {
                atom: AtomIdx::from_raw(key.atom().raw()),
                flags: PropertyFlags::data(true, true, true),
            }];
            let result = state.allocate_object_with_layout(
                &runtime.0.poisoned,
                None,
                &entries,
                vec![PropertySlot::Data(RawValue::Symbol(AtomIdx::from_raw(
                    symbol.atom().raw(),
                )))],
                ObjectData::weak_map,
            );
            assert!(result.is_err());
            assert!(runtime.is_poisoned());
        }
        drop(producer);
        assert_eq!(
            runtime_state
                .atoms
                .resolve(symbol.atom())
                .unwrap()
                .ref_count,
            before.map(|count| count + 2)
        );
    }

    #[test]
    fn prepared_layout_failure_rolls_back_shape_and_slot_atom_ownership() {
        let runtime = Runtime::new();
        let key = runtime.intern_property_key("prepared").unwrap();
        let symbol = runtime.new_symbol(None).unwrap();
        let mut state = runtime.0.state.borrow_mut();
        let before_key = state.atoms.resolve(key.atom()).unwrap().ref_count;
        let before_symbol = state.atoms.resolve(symbol.atom()).unwrap().ref_count;
        let entries = [ShapeEntry {
            atom: AtomIdx::from_raw(key.atom().raw()),
            flags: PropertyFlags::accessor(true, true),
        }];
        let result = state.allocate_object_with_layout(
            &runtime.0.poisoned,
            None,
            &entries,
            vec![PropertySlot::Data(RawValue::Symbol(AtomIdx::from_raw(
                symbol.atom().raw(),
            )))],
            ObjectData::ordinary,
        );
        assert!(
            result.is_err(),
            "mismatched storage must fail before publication"
        );
        assert_eq!(
            state.atoms.resolve(key.atom()).unwrap().ref_count,
            before_key
        );
        assert_eq!(
            state.atoms.resolve(symbol.atom()).unwrap().ref_count,
            before_symbol
        );
        assert!(state.shape_cache.is_empty());
        assert!(state.shape_hashes.is_empty());
    }
}
