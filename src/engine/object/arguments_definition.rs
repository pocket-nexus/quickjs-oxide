//! Shared mapped-Arguments preparation, validation and slot publication.

use super::OrdinaryPropertyDescriptor;
use super::definition::accepted;
use super::definition::public_descriptor::{PublicCompleteDescriptorGuard, PublicDefinitionValue};
use super::own_properties::{OwnPropertySelection, ReadyOwnProperty};
use super::property::{
    CompletePropertyDescriptor, PropertyDescriptor, validate_and_apply_property_descriptor,
};
use super::shape::PropertyFlags;
use super::storage::GlobalVarRefGuard;
use crate::engine::api::runtime_error::RuntimeError;
use crate::engine::atom::Atom;
use crate::engine::heap::runtime::RuntimeState;
use crate::engine::heap::{ObjectId, ObjectPayload, PropertySlot, RawValue, VarRefId};
use crate::engine::value::JsValue;
use std::cell::Cell;

impl RuntimeState {
    pub(super) fn arguments_index_state(
        &self,
        object: ObjectId,
        atom: Atom,
    ) -> Result<Option<(u32, bool, Option<u32>)>, RuntimeError> {
        let ObjectPayload::Arguments { mapped, fast_len } = self.heap.object(object)?.payload
        else {
            return Ok(None);
        };
        Ok(self
            .atoms
            .array_index(atom)?
            .map(|index| (index, mapped, fast_len)))
    }

    fn select_arguments_definition(
        &mut self,
        object: ObjectId,
        atom: Atom,
    ) -> Result<Option<(bool, Option<VarRefId>)>, RuntimeError> {
        let Some((index, mapped, fast_len)) = self.arguments_index_state(object, atom)? else {
            return Ok(None);
        };
        // This representation update precedes compatibility rejection, exactly
        // as in the public/owned algorithms being extracted.
        if fast_len.is_some_and(|length| index < length) {
            self.heap.set_arguments_fast_len(object, None)?;
        }
        Ok(Some((mapped, self.var_ref_property(object, atom)?)))
    }

    fn mapped_argument_current(
        &mut self,
        poisoned: &Cell<bool>,
        object: ObjectId,
        atom: Atom,
    ) -> Result<CompletePropertyDescriptor<RawValue>, RuntimeError> {
        match self.select_stored_own_property(poisoned, object, atom)? {
            OwnPropertySelection::Ready(ReadyOwnProperty::Stored(record)) => Ok(record),
            _ => Err(RuntimeError::Invariant(
                "mapped Arguments VarRef lost its property",
            )),
        }
    }

    /// Both representations use the exact same alias-retaining/detaching
    /// publication algorithm after their representation-specific value write.
    pub(super) fn store_mapped_argument_descriptor(
        &mut self,
        poisoned: &Cell<bool>,
        object: ObjectId,
        atom: Atom,
        cell: VarRefId,
        complete: CompletePropertyDescriptor<RawValue>,
    ) -> Result<(), RuntimeError> {
        match complete {
            CompletePropertyDescriptor::Data {
                writable: true,
                enumerable,
                configurable,
                ..
            } => self.store_property_slot_with_poison(
                poisoned,
                object,
                atom,
                PropertyFlags::data(true, enumerable, configurable),
                PropertySlot::VarRef(cell),
            ),
            complete => {
                self.store_complete_raw_property_with_poison(poisoned, object, atom, complete)
            }
        }
    }

    pub(super) fn define_arguments_index_raw(
        &mut self,
        poisoned: &Cell<bool>,
        object: ObjectId,
        atom: Atom,
        descriptor: &PropertyDescriptor<RawValue>,
    ) -> Result<Option<bool>, RuntimeError> {
        let Some((mapped, cell)) = self.select_arguments_definition(object, atom)? else {
            return Ok(None);
        };
        let Some(cell) = cell else {
            return self
                .define_ordinary_raw_property(poisoned, object, atom, descriptor)
                .map(Some);
        };
        let mut cell_owner = GlobalVarRefGuard::retain(self, poisoned, cell)?;
        let (state, _) = cell_owner.parts();
        let result = (|| {
            if !mapped {
                return Err(RuntimeError::Invariant(
                    "unmapped Arguments object contains a mapped VarRef slot",
                ));
            }
            // The State lease and checked cell owner protect this borrowed
            // current record. No independent descriptor owner is requested.
            let current = state.mapped_argument_current(poisoned, object, atom)?;
            let Some(complete) = accepted(validate_and_apply_property_descriptor(
                state.heap.object(object)?.extensible,
                descriptor,
                Some(&current),
                &RawValue::Undefined,
                |left, right| {
                    crate::engine::value::collection_key::same_value(&state.heap, left, right)
                },
            ))?
            else {
                return Ok(Some(false));
            };
            if let CompletePropertyDescriptor::Data { value, .. } = &complete {
                let value = state.dup_jsvalue(&JsValue::from_raw(value.clone()).ok_or(
                    RuntimeError::Invariant("initialized mapped argument held a sentinel"),
                )?)?;
                state.write_var_ref(poisoned, cell, value)?;
            }
            state.store_mapped_argument_descriptor(poisoned, object, atom, cell, complete)?;
            Ok(Some(true))
        })();
        if !poisoned.get() {
            cell_owner.retire()?;
        }
        result
    }

    pub(super) fn define_arguments_index_public(
        &mut self,
        poisoned: &Cell<bool>,
        object: ObjectId,
        atom: Atom,
        descriptor: &OrdinaryPropertyDescriptor,
    ) -> Result<Option<bool>, RuntimeError> {
        let Some((mapped, cell)) = self.select_arguments_definition(object, atom)? else {
            return Ok(None);
        };
        let Some(cell) = cell else {
            return self
                .define_ordinary_public_property(poisoned, object, atom, descriptor)
                .map(Some);
        };
        let mut cell_owner = GlobalVarRefGuard::retain(self, poisoned, cell)?;
        let (state, _) = cell_owner.parts();
        let result = (|| {
            if !mapped {
                return Err(RuntimeError::Invariant(
                    "unmapped Arguments object contains a mapped VarRef slot",
                ));
            }
            let current_record = state.mapped_argument_current(poisoned, object, atom)?;
            // Match GetOwnProperty's checked Object/Symbol/accessor roles;
            // String/BigInt only clone their public payloads.
            let mut current =
                PublicCompleteDescriptorGuard::current(state, poisoned, &current_record)?;
            let result = (|| {
                let current_record = current.record().clone();
                let state = current.state();
                let record = state.public_definition_record(descriptor)?;
                let Some(complete) = accepted(validate_and_apply_property_descriptor(
                    state.heap.object(object)?.extensible,
                    &record,
                    Some(&current_record),
                    &PublicDefinitionValue::Raw(RawValue::Undefined),
                    |left, right| left.same_value(right, state),
                ))?
                else {
                    return Ok(Some(false));
                };
                // Public validation_record_to_complete independently promoted
                // the accepted result. Keep those exact checked roles/order.
                let mut complete =
                    PublicCompleteDescriptorGuard::promote(state, poisoned, complete)?;
                let result = (|| {
                    let record = complete.record().clone();
                    if let CompletePropertyDescriptor::Data { value, .. } = &record {
                        let state = complete.state();
                        let value = value.own_value(state)?;
                        state.write_var_ref(poisoned, cell, value)?;
                    }
                    complete.store_mapped_argument(poisoned, object, atom, cell)?;
                    Ok(Some(true))
                })();
                if !poisoned.get() {
                    complete.retire()?;
                }
                result
            })();
            if !poisoned.get() {
                current.retire()?;
            }
            result
        })();
        // Completion before current descriptor before checked cell owner.
        // A first destructive failure poisons and leaves the suffix untouched.
        if !poisoned.get() {
            cell_owner.retire()?;
        }
        result
    }
}
