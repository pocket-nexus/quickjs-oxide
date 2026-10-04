//! Publication of a prepared object layout. Language-specific builders own
//! property order and descriptors; this boundary owns atom rollback and heap
//! publication. Raw slots are borrowed from roots kept alive by the caller.

use super::*;
use crate::engine::heap::ObjectData;

impl RuntimeState {
    pub(crate) fn allocate_object_with_layout(
        &mut self,
        prototype: Option<ObjectId>,
        entries: &[ShapeEntry],
        slots: Vec<PropertySlot>,
        build: impl FnOnce(ShapeId, Vec<PropertySlot>) -> ObjectData,
    ) -> Result<ObjectId, RuntimeError> {
        self.allocate_object_with_layout_inner(None, prototype, entries, slots, build)
    }

    /// Direct state consumers quarantine interrupted publication/cleanup
    /// before their temporary guards can traverse the heap again.
    pub(crate) fn allocate_object_with_layout_with_poison(
        &mut self,
        poisoned: &Cell<bool>,
        prototype: Option<ObjectId>,
        entries: &[ShapeEntry],
        slots: Vec<PropertySlot>,
        build: impl FnOnce(ShapeId, Vec<PropertySlot>) -> ObjectData,
    ) -> Result<ObjectId, RuntimeError> {
        self.allocate_object_with_layout_inner(Some(poisoned), prototype, entries, slots, build)
    }

    fn allocate_object_with_layout_inner(
        &mut self,
        poisoned: Option<&Cell<bool>>,
        prototype: Option<ObjectId>,
        entries: &[ShapeEntry],
        slots: Vec<PropertySlot>,
        build: impl FnOnce(ShapeId, Vec<PropertySlot>) -> ObjectData,
    ) -> Result<ObjectId, RuntimeError> {
        let shape = self.get_or_create_shape(prototype, entries)?;
        let atoms = match self.retain_slot_atoms(&slots) {
            Ok(atoms) => atoms,
            Err(error) => {
                let cleanup = self.heap.release_shape(shape).inspect_err(|_| {
                    if let Some(poisoned) = poisoned {
                        poisoned.set(true);
                    }
                })?;
                self.apply_cleanup(cleanup).inspect_err(|_| {
                    if let Some(poisoned) = poisoned {
                        poisoned.set(true);
                    }
                })?;
                return Err(error);
            }
        };
        let result = match self.heap.allocate_object_with_status(build(shape, slots)) {
            Err(failure) if failure.published && poisoned.is_some() => {
                poisoned.expect("direct allocation poison flag").set(true);
                return Err(failure.error.into());
            }
            result => result,
        };
        if let Err(failure) = &result {
            if !failure.published {
                self.release_atoms(atoms).inspect_err(|_| {
                    if let Some(poisoned) = poisoned {
                        poisoned.set(true);
                    }
                })?;
            }
        }
        let cleanup = self.heap.release_shape(shape).inspect_err(|_| {
            if let Some(poisoned) = poisoned {
                poisoned.set(true);
            }
        })?;
        self.apply_cleanup(cleanup).inspect_err(|_| {
            if let Some(poisoned) = poisoned {
                poisoned.set(true);
            }
        })?;
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
            let error = state.allocate_object_with_layout_with_poison(
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
            .allocate_object_with_layout_with_poison(
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
            let result = state.allocate_object_with_layout_with_poison(
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
