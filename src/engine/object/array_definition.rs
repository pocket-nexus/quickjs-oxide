//! Canonical callback-free Array descriptor and representation mutations.

use super::definition::accepted;
use super::operations::{ArrayOwnKey, PropertySetRejection};
use super::owned_descriptor::CompleteDescriptorGuard;
use super::property::{
    CompletePropertyDescriptor, PropertyDescriptor, validate_and_apply_property_descriptor,
};
use super::shape::{PropertyFlags, ShapeEntry};
use super::{DescriptorField, OrdinaryPropertyDescriptor};
use crate::engine::api::runtime::{Runtime, RuntimeUnwindGuard};
use crate::engine::api::runtime_error::RuntimeError;
use crate::engine::atom::{Atom, AtomIdx, pinned::PinnedAtom};
use crate::engine::heap::runtime::RuntimeState;
use crate::engine::heap::{HeapError, ObjectId, ObjectPayload, RawValue};
use crate::engine::value::JsValue;
use std::cell::Cell;

/// Index atoms produced while preparing one dense layout. Successful storage
/// retains its own atoms; normal temporary retirement exposes cleanup errors.
struct DenseIndexAtoms<'a> {
    state: &'a mut RuntimeState,
    poisoned: &'a Cell<bool>,
    atoms: Vec<Atom>,
}

impl DenseIndexAtoms<'_> {
    fn retire(&mut self) -> Result<(), RuntimeError> {
        let _unwind = RuntimeUnwindGuard::from_flag(self.poisoned);
        for atom in self.atoms.drain(..) {
            self.state
                .atoms
                .release(atom)
                .inspect_err(|_| self.poisoned.set(true))?;
        }
        Ok(())
    }
}

impl Drop for DenseIndexAtoms<'_> {
    fn drop(&mut self) {
        if self.atoms.is_empty() {
            return;
        }
        if std::thread::panicking() {
            self.poisoned.set(true);
        }
        if !self.poisoned.get() {
            let _ = self.retire();
        }
    }
}

impl RuntimeState {
    pub(super) fn array_own_key(
        &self,
        object: ObjectId,
        atom: Atom,
    ) -> Result<ArrayOwnKey, RuntimeError> {
        if !matches!(
            self.heap.object(object)?.payload,
            ObjectPayload::Array { .. }
        ) {
            return Ok(ArrayOwnKey::Other);
        }
        if let Some(index) = self.atoms.array_index(atom)? {
            return Ok(ArrayOwnKey::Index(index));
        }
        let info = self.atoms.resolve(atom)?;
        if info.kind == crate::engine::atom::AtomKind::String
            && matches!(info.spelling, crate::engine::atom::AtomSpelling::Text(text) if text.utf16_units().eq("length".encode_utf16()))
        {
            return Ok(ArrayOwnKey::Length);
        }
        Ok(ArrayOwnKey::Other)
    }

    pub(super) fn array_length_state(&self, object: ObjectId) -> Result<(u32, bool), RuntimeError> {
        Runtime::array_length_state_in_heap(
            &self.heap,
            object,
            self.pinned_atoms.get(PinnedAtom::Length),
        )?
        .ok_or(RuntimeError::Invariant(
            "Array length state requested for a non-Array object",
        ))
    }

    pub(super) fn materialize_dense_array(
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
                operation: "rooting materialized Array indices",
            })?;
        let mut keys = DenseIndexAtoms {
            state: self,
            poisoned,
            atoms,
        };
        let result = (|| {
            for index in 0..dense_len {
                let index = u32::try_from(index)
                    .map_err(|_| RuntimeError::Invariant("fast Array count exceeded Uint32"))?;
                let atom = match Atom::from_immediate_integer(index) {
                    Some(atom) => atom,
                    None => keys.state.intern_property_key_js_string(
                        &crate::engine::value::JsString::try_from_utf8(&index.to_string())?,
                    )?,
                };
                keys.atoms.push(atom);
                entries.push(ShapeEntry {
                    atom: AtomIdx::from_raw(atom.raw()),
                    flags: PropertyFlags::data(true, true, true),
                });
            }
            keys.state
                .materialize_array_layout_with_poison(poisoned, object, prototype, &entries)
        })();
        if !poisoned.get() {
            keys.retire()?;
        }
        result?;
        #[cfg(feature = "profiling")]
        {
            crate::engine::api::profiling::record_owned_execution_event(
                "array_storage_dense_materialization",
            );
            crate::engine::api::profiling::record_owned_execution_event(reason);
        }
        Ok(())
    }

    pub(super) fn append_dense_array_raw(
        &mut self,
        poisoned: &Cell<bool>,
        object: ObjectId,
        raw: RawValue,
    ) -> Result<(), RuntimeError> {
        let atoms = self.retain_raw_value_atoms(std::iter::once(&raw))?;
        match self.heap.append_array_dense_value(object, raw) {
            Ok(()) => Ok(()),
            Err(error) => {
                self.release_atoms(atoms)
                    .inspect_err(|_| poisoned.set(true))?;
                Err(error.into())
            }
        }
    }

    pub(super) fn replace_dense_array_raw(
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

    pub(super) fn grow_dense_array_length(
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
        if !self.define_ordinary_raw_property(
            poisoned,
            object,
            self.pinned_atoms.get(PinnedAtom::Length),
            &PropertyDescriptor {
                value: Some(Self::array_length_raw(next)),
                ..PropertyDescriptor::new()
            },
        )? {
            return Err(RuntimeError::Invariant(
                "writable Array length rejected dense index growth",
            ));
        }
        Ok(())
    }

    pub(super) fn array_length_raw(length: u32) -> RawValue {
        i32::try_from(length)
            .map(RawValue::Int)
            .unwrap_or_else(|_| RawValue::Float(f64::from(length)))
    }

    pub(super) fn define_selected_dense_array_append(
        &mut self,
        poisoned: &Cell<bool>,
        object: ObjectId,
        index: u32,
        value: &JsValue,
    ) -> Result<Option<PropertySetRejection>, RuntimeError> {
        let (length, writable) = self.array_length_state(object)?;
        let extensible = self.heap.object(object)?.extensible;
        if index >= length && !writable {
            return Ok(Some(if extensible {
                PropertySetRejection::ArrayLengthReadOnly
            } else {
                PropertySetRejection::NotExtensible
            }));
        }
        if !extensible {
            return Ok(Some(PropertySetRejection::NotExtensible));
        }
        self.append_dense_array_raw(poisoned, object, value.as_raw())?;
        self.grow_dense_array_length(poisoned, object, index, length)?;
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_execution_event(
            "set_dense_append_from_selection",
        );
        Ok(None)
    }

    pub(super) fn define_array_index_public(
        &mut self,
        poisoned: &Cell<bool>,
        object: ObjectId,
        atom: Atom,
        index: u32,
        descriptor: &OrdinaryPropertyDescriptor,
    ) -> Result<bool, RuntimeError> {
        let (length, writable) = self.array_length_state(object)?;
        if index >= length && !writable {
            return Ok(false);
        }
        let mut producer = CompleteDescriptorGuard::new(self, poisoned);
        let mut record = Self::public_descriptor_attributes(descriptor);
        if let DescriptorField::Present(value) = &descriptor.value {
            record.value = Some(producer.public_value(value)?);
        }
        let result = producer
            .state()
            .define_array_index_raw(poisoned, object, atom, index, length, &record);
        if !poisoned.get() {
            producer.retire()?;
        }
        result
    }

    pub(super) fn define_array_index_raw(
        &mut self,
        poisoned: &Cell<bool>,
        object: ObjectId,
        atom: Atom,
        index: u32,
        old_length: u32,
        descriptor: &PropertyDescriptor<RawValue>,
    ) -> Result<bool, RuntimeError> {
        if let Some(dense_len) = self.heap.array_dense_len(object)? {
            let current = if index < dense_len {
                Some(CompletePropertyDescriptor::Data {
                    value: self.dense_array_index_value(object, atom)?.ok_or(
                        RuntimeError::Invariant("validated dense Array index disappeared"),
                    )?,
                    writable: true,
                    enumerable: true,
                    configurable: true,
                })
            } else {
                None
            };
            let Some(complete) = accepted(validate_and_apply_property_descriptor(
                self.heap.object(object)?.extensible,
                descriptor,
                current.as_ref(),
                &RawValue::Undefined,
                |left, right| {
                    crate::engine::value::collection_key::same_value(&self.heap, left, right)
                },
            ))?
            else {
                return Ok(false);
            };
            if let CompletePropertyDescriptor::Data {
                value,
                writable: true,
                enumerable: true,
                configurable: true,
            } = &complete
            {
                if index < dense_len {
                    if descriptor.value.is_some() {
                        self.replace_dense_array_raw(poisoned, object, index, value.clone())?;
                    }
                    return Ok(true);
                }
                if index == dense_len {
                    self.append_dense_array_raw(poisoned, object, value.clone())?;
                    self.grow_dense_array_length(poisoned, object, index, old_length)?;
                    return Ok(true);
                }
            }
            #[cfg(feature = "profiling")]
            self.materialize_dense_array(
                poisoned,
                object,
                "array_storage_dense_materialization_descriptor_path",
            )?;
            #[cfg(not(feature = "profiling"))]
            self.materialize_dense_array(poisoned, object)?;
        }
        let recover = index == 0 && self.stored_own_property(object, atom)?.is_none();
        if !self.define_ordinary_raw_property(poisoned, object, atom, descriptor)? {
            return Ok(false);
        }
        if index < old_length {
            if recover {
                self.try_recover_dense_array(poisoned, object)?;
            }
            return Ok(true);
        }
        self.grow_dense_array_length(poisoned, object, index, old_length)?;
        Ok(true)
    }

    pub(super) fn define_array_index_property(
        &mut self,
        poisoned: &Cell<bool>,
        object: ObjectId,
        atom: Atom,
        index: u32,
        descriptor: &PropertyDescriptor<RawValue>,
    ) -> Result<bool, RuntimeError> {
        let (length, writable) = self.array_length_state(object)?;
        if index >= length && !writable {
            return Ok(false);
        }
        self.define_array_index_raw(poisoned, object, atom, index, length, descriptor)
    }

    pub(crate) fn define_selected_set_data(
        &mut self,
        poisoned: &Cell<bool>,
        object: ObjectId,
        atom: Atom,
        value: &JsValue,
        existing: bool,
    ) -> Result<bool, RuntimeError> {
        let array = match self.array_own_key(object, atom)? {
            ArrayOwnKey::Index(index) => {
                let (length, writable) = self.array_length_state(object)?;
                if index >= length && !writable {
                    return Ok(false);
                }
                if let Some(dense_len) = self.heap.array_dense_len(object)? {
                    if index < dense_len {
                        self.replace_dense_array_raw(poisoned, object, index, value.as_raw())?;
                        return Ok(true);
                    }
                    if !self.heap.object(object)?.extensible {
                        return Ok(false);
                    }
                    if index == dense_len {
                        self.append_dense_array_raw(poisoned, object, value.as_raw())?;
                        self.grow_dense_array_length(poisoned, object, index, length)?;
                        return Ok(true);
                    }
                    #[cfg(feature = "profiling")]
                    self.materialize_dense_array(
                        poisoned,
                        object,
                        "array_storage_dense_materialization_gap_write",
                    )?;
                    #[cfg(not(feature = "profiling"))]
                    self.materialize_dense_array(poisoned, object)?;
                }
                Some((index, length))
            }
            ArrayOwnKey::Other => None,
            ArrayOwnKey::Length => {
                return Err(RuntimeError::Invariant(
                    "selected data define reached Array length",
                ));
            }
        };
        let descriptor = PropertyDescriptor {
            value: Some(value.as_raw()),
            writable: (!existing).then_some(true),
            enumerable: (!existing).then_some(true),
            configurable: (!existing).then_some(true),
            ..PropertyDescriptor::new()
        };
        if !self.define_ordinary_raw_property(poisoned, object, atom, &descriptor)? {
            return Ok(false);
        }
        if let Some((index, length)) = array {
            self.grow_dense_array_length(poisoned, object, index, length)?;
            if index == 0 && !existing {
                self.try_recover_dense_array(poisoned, object)?;
            }
        }
        Ok(true)
    }

    /// Coercion has finished outside State. The supplied length snapshot was
    /// reloaded after that callback; public completion promotion happened next.
    pub(super) fn apply_array_length_descriptor(
        &mut self,
        poisoned: &Cell<bool>,
        object: ObjectId,
        atom: Atom,
        descriptor: &PropertyDescriptor<RawValue>,
        new_length: u32,
        old_state: (u32, bool),
    ) -> Result<bool, RuntimeError> {
        let (old_length, old_writable) = old_state;
        let mut canonical = descriptor.clone();
        canonical.value = Some(Self::array_length_raw(new_length));
        if new_length >= old_length || !old_writable {
            return self.define_ordinary_raw_property(poisoned, object, atom, &canonical);
        }
        let finish_read_only = canonical.writable == Some(false);
        if finish_read_only {
            canonical.writable = Some(true);
        }
        let Some(current) = self.prepare_ordinary_definition(poisoned, object, atom, &canonical)?
        else {
            return Ok(false);
        };
        let current = current.ok_or(RuntimeError::Invariant(
            "genuine Array lost its mandatory length property",
        ))?;
        if accepted(validate_and_apply_property_descriptor(
            self.heap.object(object)?.extensible,
            &canonical,
            Some(&current),
            &RawValue::Undefined,
            |left, right| crate::engine::value::collection_key::same_value(&self.heap, left, right),
        ))?
        .is_none()
        {
            return Ok(false);
        }
        let truncation = if self
            .heap
            .array_dense_len(object)?
            .is_some_and(|length| new_length < length)
        {
            Some(
                self.heap
                    .prepare_array_dense_truncation(object, new_length)?,
            )
        } else {
            None
        };
        if !self.apply_ordinary_definition(poisoned, object, atom, &canonical, Some(&current))? {
            return Ok(false);
        }
        if let Some(prepared) = truncation {
            let cleanup = self
                .heap
                .commit_array_dense_truncation_with_status(prepared)
                .map_err(|failure| {
                    if failure.published {
                        poisoned.set(true);
                    }
                    RuntimeError::from(failure.error)
                })?;
            self.apply_cleanup(cleanup)
                .inspect_err(|_| poisoned.set(true))?;
        }
        if let Some(index) = self.truncate_sparse_array_indices(poisoned, object, new_length)? {
            let restored = index
                .checked_add(1)
                .ok_or(RuntimeError::Invariant("Array index exceeded Uint32 range"))?;
            if !self.define_ordinary_raw_property(
                poisoned,
                object,
                atom,
                &PropertyDescriptor {
                    value: Some(Self::array_length_raw(restored)),
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
            && !self.define_ordinary_raw_property(
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
    }
}
