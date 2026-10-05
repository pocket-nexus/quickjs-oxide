//! Array descriptor publication shares the caller's current State access.
use super::*;
use crate::engine::object::property::{CompletePropertyDescriptor, PropertyDescriptor};

struct IndexAtomsGuard<'a> {
    state: &'a mut RuntimeState,
    poisoned: &'a Cell<bool>,
    atoms: Vec<Atom>,
}
impl IndexAtomsGuard<'_> {
    fn finish(&mut self) -> Result<(), RuntimeError> {
        if std::thread::panicking() {
            self.poisoned.set(true);
        }
        if self.poisoned.get() {
            return Err(RuntimeError::Poisoned);
        }
        self.state
            .release_atoms(self.atoms.drain(..))
            .inspect_err(|_| self.poisoned.set(true))
    }
}
impl Drop for IndexAtomsGuard<'_> {
    fn drop(&mut self) {
        let _ = self.finish();
    }
}

impl RuntimeState {
    pub(super) fn materialize_dense_array_in_state(
        &mut self,
        poisoned: &Cell<bool>,
        object: ObjectId,
        #[cfg(feature = "profiling")] reason: &'static str,
    ) -> Result<(), RuntimeError> {
        let (prototype, dense_len, mut entries) =
            {
                let data = self.heap.object(object)?;
                let ObjectPayload::Array { dense: Some(dense) } = &data.payload else {
                    return Ok(());
                };
                let shape = self.heap.shape(data.shape)?;
                for entry in shape.entries() {
                    if self
                        .atoms
                        .array_index(self.atoms.brand(entry.atom)?)?
                        .is_some()
                    {
                        return Err(RuntimeError::Invariant(
                            "fast Array shape already contained a numeric property",
                        ));
                    }
                }
                let count = shape.entries().len().checked_add(dense.len()).ok_or(
                    RuntimeError::Invariant("materialized Array shape length overflowed"),
                )?;
                let mut entries = Vec::new();
                entries
                    .try_reserve_exact(count)
                    .map_err(|_| HeapError::Allocation {
                        operation: "materializing fast Array shape",
                    })?;
                entries.extend_from_slice(shape.entries());
                (shape.prototype(), dense.len(), entries)
            };
        let mut atoms = Vec::new();
        atoms
            .try_reserve(dense_len)
            .map_err(|_| HeapError::Allocation {
                operation: "owning materialized Array indices",
            })?;
        let mut guard = IndexAtomsGuard {
            state: self,
            poisoned,
            atoms,
        };
        for index in 0..dense_len {
            let index = u32::try_from(index)
                .map_err(|_| RuntimeError::Invariant("fast Array count exceeded Uint32"))?;
            let atom = match Atom::from_immediate_integer(index) {
                Some(atom) => atom,
                None => guard
                    .state
                    .intern_property_key_js_string(&JsString::try_from_utf8(&index.to_string())?)?,
            };
            // Reserved before acquiring the first atom owner.
            guard.atoms.push(atom);
            entries.push(ShapeEntry {
                atom: AtomIdx::from_raw(atom.raw()),
                flags: PropertyFlags::data(true, true, true),
            });
        }
        guard
            .state
            .materialize_array_layout_with_poison(poisoned, object, prototype, &entries)?;
        guard.finish()?;
        #[cfg(feature = "profiling")]
        {
            crate::engine::api::profiling::record_owned_execution_event(
                "array_storage_dense_materialization",
            );
            crate::engine::api::profiling::record_owned_execution_event(reason);
        }
        Ok(())
    }

    pub(super) fn append_dense_array_raw_in_state(
        &mut self,
        poisoned: &Cell<bool>,
        object: ObjectId,
        raw: RawValue,
    ) -> Result<(), RuntimeError> {
        let atoms = self.retain_raw_value_atoms(std::iter::once(&raw))?;
        if let Err(error) = self.heap.append_array_dense_value(object, raw) {
            self.release_atoms(atoms)
                .inspect_err(|_| poisoned.set(true))?;
            return Err(error.into());
        }
        Ok(())
    }

    pub(super) fn replace_dense_array_raw_in_state(
        &mut self,
        poisoned: &Cell<bool>,
        object: ObjectId,
        index: u32,
        raw: RawValue,
    ) -> Result<(), RuntimeError> {
        let atoms = self.retain_raw_value_atoms(std::iter::once(&raw))?;
        match self
            .heap
            .replace_array_dense_value_with_status(object, index, raw)
        {
            Ok(cleanup) => self
                .apply_cleanup(cleanup)
                .inspect_err(|_| poisoned.set(true)),
            Err(failure) => {
                if failure.published {
                    poisoned.set(true);
                } else {
                    self.release_atoms(atoms)
                        .inspect_err(|_| poisoned.set(true))?;
                }
                Err(failure.error.into())
            }
        }
    }

    pub(super) fn grow_dense_array_length_in_state(
        &mut self,
        poisoned: &Cell<bool>,
        object: ObjectId,
        index: u32,
        old_length: u32,
    ) -> Result<(), RuntimeError> {
        if index < old_length {
            return Ok(());
        }
        let next = index
            .checked_add(1)
            .ok_or(RuntimeError::Invariant("Array index exceeded Uint32 range"))?;
        let atom = self
            .pinned_atoms
            .get(crate::engine::atom::pinned::PinnedAtom::Length);
        let result = self.define_raw_property_with_poison(
            poisoned,
            object,
            atom,
            &PropertyDescriptor {
                value: Some(array_length_raw(next)),
                ..PropertyDescriptor::new()
            },
        );
        match result {
            Ok(true) => Ok(()),
            Ok(false) => {
                poisoned.set(true);
                Err(RuntimeError::Invariant(
                    "writable Array length rejected index growth",
                ))
            }
            Err(error) => {
                poisoned.set(true);
                Err(error)
            }
        }
    }

    pub(super) fn define_array_index_in_state(
        &mut self,
        poisoned: &Cell<bool>,
        object: ObjectId,
        atom: Atom,
        index: u32,
        record: &PropertyDescriptor<RawValue>,
    ) -> Result<bool, RuntimeError> {
        let length_atom = self
            .pinned_atoms
            .get(crate::engine::atom::pinned::PinnedAtom::Length);
        let (old_length, writable) =
            Runtime::array_length_state_in_heap(&self.heap, object, length_atom)?.ok_or(
                RuntimeError::Invariant("indexed Array definition reached a non-Array"),
            )?;
        if index >= old_length && !writable {
            return Ok(false);
        }
        if let Some(dense_len) = self.heap.array_dense_len(object)? {
            let data = self.heap.object(object)?;
            let current =
                data.dense_array_value(index)
                    .map(|value| CompletePropertyDescriptor::Data {
                        value: value.clone(),
                        writable: true,
                        enumerable: true,
                        configurable: true,
                    });
            let complete = match validate_and_apply_property_descriptor(
                data.extensible,
                record,
                current.as_ref(),
                &RawValue::Undefined,
                |a, b| crate::engine::value::collection_key::same_value(&self.heap, a, b),
            ) {
                Ok(value) => value,
                Err(PropertyDefinitionError::InvalidDescriptor) => {
                    return Err(PropertyDefinitionError::InvalidDescriptor.into());
                }
                Err(_) => return Ok(false),
            };
            if let CompletePropertyDescriptor::Data {
                value,
                writable: true,
                enumerable: true,
                configurable: true,
            } = &complete
            {
                if index < dense_len {
                    if record.value.is_some() {
                        self.replace_dense_array_raw_in_state(
                            poisoned,
                            object,
                            index,
                            value.clone(),
                        )?;
                    }
                    return Ok(true);
                }
                if index == dense_len {
                    self.append_dense_array_raw_in_state(poisoned, object, value.clone())?;
                    self.grow_dense_array_length_in_state(poisoned, object, index, old_length)?;
                    return Ok(true);
                }
            }
            #[cfg(feature = "profiling")]
            self.materialize_dense_array_in_state(
                poisoned,
                object,
                "array_storage_dense_materialization_descriptor_path",
            )?;
            #[cfg(not(feature = "profiling"))]
            self.materialize_dense_array_in_state(poisoned, object)?;
        }
        let data = self.heap.object(object)?;
        let recover = index == 0
            && self
                .heap
                .shape(data.shape)?
                .find(AtomIdx::from_raw(atom.raw()))
                .is_none();
        if !self.define_raw_property_with_poison(poisoned, object, atom, record)? {
            return Ok(false);
        }
        self.grow_dense_array_length_in_state(poisoned, object, index, old_length)?;
        if recover && index < old_length {
            self.try_recover_dense_array_in_state(poisoned, object)?;
        }
        Ok(true)
    }

    pub(super) fn apply_array_length_descriptor_in_state(
        &mut self,
        poisoned: &Cell<bool>,
        object: ObjectId,
        atom: Atom,
        descriptor: &PropertyDescriptor<RawValue>,
        new_length: u32,
    ) -> Result<bool, RuntimeError> {
        let (old_length, old_writable) =
            Runtime::array_length_state_in_heap(&self.heap, object, atom)?.ok_or(
                RuntimeError::Invariant("Array length definition reached a non-Array"),
            )?;
        let mut canonical = descriptor.clone();
        canonical.value = Some(array_length_raw(new_length));
        if new_length >= old_length || !old_writable {
            return self.define_raw_property_with_poison(poisoned, object, atom, &canonical);
        }
        let finish_read_only = canonical.writable == Some(false);
        if finish_read_only {
            canonical.writable = Some(true);
        }
        if self
            .validate_raw_property(object, atom, &canonical)?
            .is_none()
        {
            return Ok(false);
        }
        let truncation = if self
            .heap
            .array_dense_len(object)?
            .is_some_and(|len| new_length < len)
        {
            Some(
                self.heap
                    .prepare_array_dense_truncation(object, new_length)?,
            )
        } else {
            None
        };
        if !self.define_raw_property_with_poison(poisoned, object, atom, &canonical)? {
            return Ok(false);
        }
        // The requested shorter length is published. Any destructive or
        // allocation failure below quarantines the partially shortened Array.
        (|| {
            if let Some(prepared) = truncation {
                let cleanup = self.heap.commit_array_dense_truncation(prepared)?;
                self.apply_cleanup(cleanup)?;
            }
            if let Some(index) =
                self.truncate_sparse_array_indices_in_state(poisoned, object, new_length)?
            {
                let restored = index
                    .checked_add(1)
                    .ok_or(RuntimeError::Invariant("Array index exceeded Uint32 range"))?;
                if !self.define_raw_property_with_poison(
                    poisoned,
                    object,
                    atom,
                    &PropertyDescriptor {
                        value: Some(array_length_raw(restored)),
                        writable: finish_read_only.then_some(false),
                        ..PropertyDescriptor::new()
                    },
                )? {
                    return Err(RuntimeError::Invariant(
                        "Array length rollback was rejected",
                    ));
                }
                return Ok(false);
            }
            if finish_read_only
                && !self.define_raw_property_with_poison(
                    poisoned,
                    object,
                    atom,
                    &PropertyDescriptor {
                        writable: Some(false),
                        ..PropertyDescriptor::new()
                    },
                )?
            {
                return Err(RuntimeError::Invariant(
                    "Array length writable transition was rejected",
                ));
            }
            Ok(true)
        })()
        .inspect_err(|_| poisoned.set(true))
    }
}

fn array_length_raw(length: u32) -> RawValue {
    i32::try_from(length)
        .map(RawValue::Int)
        .unwrap_or_else(|_| RawValue::Float(f64::from(length)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::heap::RawId;

    fn one_object_array(runtime: &Runtime) -> (crate::engine::api::Context, ObjectRef, ObjectId) {
        let mut context = runtime.new_context().unwrap();
        let Value::Object(array) = context.eval("[{owned:1}]").unwrap() else {
            panic!()
        };
        let state = runtime.0.state.borrow();
        let RawValue::Object(value) = state
            .heap
            .object(array.object_id())
            .unwrap()
            .dense_array_value(0)
            .unwrap()
        else {
            panic!()
        };
        let id = *value;
        drop(state);
        (context, array, id)
    }

    #[test]
    fn failed_dense_retain_leaves_published_owner_and_length_unchanged() {
        let runtime = Runtime::new();
        let (_context, array, previous) = one_object_array(&runtime);
        let incoming = runtime.new_object(None).unwrap();
        let key = runtime.property_key_for_index(0).unwrap();
        let mut state = runtime.0.state.borrow_mut();
        state
            .heap
            .set_strong_count_for_test(RawId::Object(incoming.object_id()), u32::MAX);
        let result = state.try_define_own_property_in_state(
            &runtime.0.poisoned,
            array.object_id(),
            key.atom(),
            &PropertyDescriptor {
                value: Some(RawValue::Object(incoming.object_id())),
                ..PropertyDescriptor::new()
            },
        );
        state
            .heap
            .set_strong_count_for_test(RawId::Object(incoming.object_id()), 1);
        assert!(matches!(
            result,
            Err(RuntimeError::Heap(HeapError::Overflow { .. }))
        ));
        assert!(matches!(
            state
                .heap
                .object(array.object_id())
                .unwrap()
                .dense_array_value(0),
            Some(RawValue::Object(id)) if *id == previous
        ));
        assert_eq!(state.heap.object_strong_count(previous), Ok(1));
        assert!(!runtime.is_poisoned());
    }

    #[test]
    fn published_dense_replacement_keeps_new_symbol_owner_after_cleanup_failure() {
        let runtime = Runtime::new();
        let (_context, array, previous) = one_object_array(&runtime);
        let symbol = runtime.new_symbol(None).unwrap();
        let key = runtime.property_key_for_index(0).unwrap();
        let atom = symbol.atom();
        let mut state = runtime.0.state.borrow_mut();
        let count = state.atoms.resolve(atom).unwrap().ref_count.unwrap();
        state
            .heap
            .set_strong_count_for_test(RawId::Object(previous), 0);
        let result = state.try_define_own_property_in_state(
            &runtime.0.poisoned,
            array.object_id(),
            key.atom(),
            &PropertyDescriptor {
                value: Some(RawValue::Symbol(AtomIdx::from_raw(atom.raw()))),
                ..PropertyDescriptor::new()
            },
        );
        assert!(result.is_err());
        assert!(runtime.is_poisoned());
        assert_eq!(
            state.atoms.resolve(atom).unwrap().ref_count,
            Some(count + 1)
        );
        assert!(matches!(
            state
                .heap
                .object(array.object_id())
                .unwrap()
                .dense_array_value(0),
            Some(RawValue::Symbol(index)) if index.raw() == atom.raw()
        ));
    }

    #[test]
    fn materialized_layout_moves_owners_before_old_shape_failure_and_poison() {
        let runtime = Runtime::new();
        let (_context, array, value) = one_object_array(&runtime);
        let key = runtime.property_key_for_index(0).unwrap();
        let mut state = runtime.0.state.borrow_mut();
        let old_shape = state.heap.object(array.object_id()).unwrap().shape;
        state
            .heap
            .set_strong_count_for_test(RawId::Shape(old_shape), 0);
        let result = state.try_define_own_property_in_state(
            &runtime.0.poisoned,
            array.object_id(),
            key.atom(),
            &PropertyDescriptor {
                writable: Some(false),
                ..PropertyDescriptor::new()
            },
        );
        assert!(result.is_err());
        assert!(runtime.is_poisoned());
        let data = state.heap.object(array.object_id()).unwrap();
        assert!(matches!(data.payload, ObjectPayload::Array { dense: None }));
        assert_ne!(data.shape, old_shape);
        assert_eq!(state.heap.object_strong_count(value), Ok(1));
    }

    #[test]
    fn state_length_shrink_preserves_spec_partial_deletion_without_roots() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        let Value::Object(array) = context.eval("(()=>{const a=[];Object.defineProperty(a,'3',{value:3,configurable:false});a[6]=6;return a})()").unwrap() else { panic!() };
        let count = std::rc::Rc::strong_count(&runtime.0);
        let mut state = runtime.0.state.borrow_mut();
        let atom = state
            .pinned_atoms
            .get(crate::engine::atom::pinned::PinnedAtom::Length);
        assert_eq!(
            state
                .try_define_own_property_in_state(
                    &runtime.0.poisoned,
                    array.object_id(),
                    atom,
                    &PropertyDescriptor {
                        value: Some(RawValue::Int(1)),
                        writable: Some(false),
                        ..PropertyDescriptor::new()
                    }
                )
                .unwrap(),
            Some(false)
        );
        assert_eq!(
            Runtime::array_length_state_in_heap(&state.heap, array.object_id(), atom).unwrap(),
            Some((4, false))
        );
        let StateOwnPropertySnapshot::Absent = state
            .own_property_snapshot_in_state(
                &runtime.0.poisoned,
                array.object_id(),
                Atom::from_immediate_integer(6).unwrap(),
            )
            .unwrap()
        else {
            panic!()
        };
        assert_eq!(std::rc::Rc::strong_count(&runtime.0), count);
        assert!(!runtime.0.deferred_references.has_pending());
        assert!(!runtime.is_poisoned());
    }
}
