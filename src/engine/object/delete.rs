//! Canonical callback-free DeleteOwnProperty selection and mutation.
//! Shared bytes are the only selected ordinary boundary; Proxy deletion uses
//! the existing internal-method protocol, after this entry is not selected.
use crate::engine::{
    api::{
        runtime::{Runtime, RuntimeUnwindGuard},
        runtime_error::RuntimeError,
    },
    atom::{Atom, AtomIdx},
    builtins::{CanonicalNumericIndex, SharedTypedOwnWord, TypedOwnProperty},
    code::function::metadata::ClosureVariableKind,
    heap::{
        ObjectId, ObjectPayload, PropertySlot,
        runtime::{RuntimeState, owned_values::OwnedValueGuard},
    },
    object::{
        ObjectRef, PropertyKey, operations::ArrayOwnKey, shape::PropertyFlags,
        storage::GlobalVarRefGuard,
    },
    value::JsValue,
};
use std::cell::Cell;

pub(crate) enum DeleteOwnProperty {
    Complete(bool),
    Shared(SharedTypedOwnWord),
}
impl DeleteOwnProperty {
    /// The exact selected byte read finishes without reselecting the wrapper.
    pub(crate) fn finish_shared(self) -> Result<bool, RuntimeError> {
        match self {
            Self::Complete(result) => Ok(result),
            Self::Shared(word) => {
                word.read()?;
                Ok(false)
            }
        }
    }
}
impl Runtime {
    /// Delete an ordinary own property without invoking accessors.
    pub fn delete_property(
        &self,
        object: &ObjectRef,
        key: &PropertyKey,
    ) -> Result<bool, RuntimeError> {
        let _operation = self.operation()?;
        self.validate_object_and_key(object, key)?;
        let selected = {
            let _unwind = self.unwind_guard();
            self.0.state.borrow_mut().delete_own_property_in_state(
                &self.0.poisoned,
                object.object_id(),
                key.atom(),
            )?
        };
        selected.finish_shared()
    }
}
impl RuntimeState {
    /// Inputs borrow admitted source/key owners. No coordinator FIFO is drained
    /// under this lease, and no public root or callback is created.
    pub(crate) fn delete_own_property_in_state(
        &mut self,
        poisoned: &Cell<bool>,
        object: ObjectId,
        atom: Atom,
    ) -> Result<DeleteOwnProperty, RuntimeError> {
        let _unwind = RuntimeUnwindGuard::from_flag(poisoned);
        if matches!(
            self.heap.object(object)?.payload,
            ObjectPayload::TypedArray(_)
        ) {
            let key = self.typed_own_key(atom)?;
            if let Some(numeric) = self.typed_canonical_numeric_index(atom, key)? {
                return Ok(match numeric {
                    CanonicalNumericIndex::Invalid => DeleteOwnProperty::Complete(true),
                    CanonicalNumericIndex::Valid(index) => {
                        match self.select_typed_own_property(object, index)? {
                            TypedOwnProperty::Missing => DeleteOwnProperty::Complete(true),
                            TypedOwnProperty::Word(_) => DeleteOwnProperty::Complete(false),
                            TypedOwnProperty::Shared(word) => DeleteOwnProperty::Shared(word),
                        }
                    }
                });
            }
        }
        if self.string_exotic_index_value(object, atom)?.is_some() {
            return Ok(DeleteOwnProperty::Complete(false));
        }
        if let ArrayOwnKey::Index(index) = self.array_own_key(object, atom)?
            && let Some(dense_len) = self.heap.array_dense_len(object)?
            && index < dense_len
        {
            if index.checked_add(1) == Some(dense_len) {
                let prepared = self.heap.prepare_array_dense_truncation(object, index)?;
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
                return Ok(DeleteOwnProperty::Complete(true));
            }
            self.materialize_dense_array(
                poisoned,
                object,
                #[cfg(feature = "profiling")]
                "array_storage_dense_materialization_interior_delete",
            )?;
        }
        let arguments_index = self
            .arguments_index_state(object, atom)?
            .map(|(index, _, _)| index);
        let global_cell = {
            let data = self.heap.object(object)?;
            if let ObjectPayload::GlobalObject { uninitialized_vars } = data.payload {
                let shape = self.heap.shape(data.shape)?;
                let Some(index) = shape.find(AtomIdx::from_raw(atom.raw())) else {
                    return Ok(DeleteOwnProperty::Complete(true));
                };
                let index = index as usize;
                let entry = shape.entries().get(index).ok_or(RuntimeError::Invariant(
                    "shape lookup index was out of bounds",
                ))?;
                match data.slots.get(index).ok_or(RuntimeError::Invariant(
                    "shape property has no parallel object slot",
                ))? {
                    PropertySlot::VarRef(cell) if self.heap.var_ref_strong_count(*cell)? > 1 => {
                        Some((uninitialized_vars, *cell, entry.flags.configurable))
                    }
                    _ => None,
                }
            } else {
                None
            }
        };
        if let Some((hidden, cell, configurable)) = global_cell {
            if !configurable {
                return Ok(DeleteOwnProperty::Complete(false));
            }
            let mut cell_owner = GlobalVarRefGuard::retain(self, poisoned, cell)?;
            let (state, cell) = cell_owner.parts();
            if let Err(error) = state.heap.retain_object(hidden) {
                cell_owner.retire()?;
                return Err(error.into());
            }
            let (state, _) = cell_owner.parts();
            let mut hidden_owner = OwnedValueGuard::new(state, poisoned, JsValue::Object(hidden));
            let result = (|| {
                let (state, _) = hidden_owner.parts();
                if let Some(existing) = state.var_ref_property(hidden, atom)? {
                    // Preserve the probe's checked edge and retirement before
                    // either the mismatch error or the reset publication.
                    GlobalVarRefGuard::retain(state, poisoned, existing)?.retire()?;
                    if existing != cell {
                        return Err(RuntimeError::Invariant(
                            "hidden global table contains a different VarRef",
                        ));
                    }
                } else {
                    state.store_property_slot_with_poison(
                        poisoned,
                        hidden,
                        atom,
                        PropertyFlags::data(true, true, true),
                        PropertySlot::VarRef(cell),
                    )?;
                }
                state.reset_var_ref_uninitialized(poisoned, cell)?;
                state.set_var_ref_metadata(cell, false, false, ClosureVariableKind::Normal)
            })();
            if poisoned.get() {
                return Err(RuntimeError::Poisoned);
            }
            let (state, value) = hidden_owner.parts();
            state
                .release_owned_jsvalue(poisoned, value.take().expect("hidden object temporary"))?;
            drop(hidden_owner);
            // Both temporaries ended before the old ordinary delete call.
            cell_owner.retire()?;
            result?;
        }
        self.delete_ordinary_property_with_poison(poisoned, object, atom, arguments_index)
            .map(DeleteOwnProperty::Complete)
    }
}

#[cfg(test)]
mod tests;
