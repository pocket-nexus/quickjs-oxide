//! Module Namespace compatibility reuses the checked stored descriptor read.

use super::super::OrdinaryPropertyDescriptor;
use super::super::own_properties::{OwnPropertySelection, ReadyOwnProperty};
use super::super::property::{CompletePropertyDescriptor, PropertyDescriptor};
use super::public_descriptor::PublicCompleteDescriptorGuard;
use crate::engine::api::runtime_error::RuntimeError;
use crate::engine::atom::Atom;
use crate::engine::heap::runtime::RuntimeState;
use crate::engine::heap::{ObjectId, ObjectKind, PropertySlot, RawValue};
use std::cell::Cell;

fn compatible<V>(
    descriptor: &PropertyDescriptor<V>,
    current: &V,
    same_value: impl FnOnce(&V, &V) -> bool,
) -> bool {
    !descriptor.is_accessor_descriptor()
        && descriptor.configurable != Some(true)
        && descriptor.enumerable != Some(false)
        && descriptor.writable != Some(false)
        && descriptor
            .value
            .as_ref()
            .is_none_or(|value| same_value(value, current))
}

impl RuntimeState {
    /// Live export VarRef rather than the ordinary @@toStringTag data slot.
    pub(crate) fn module_namespace_export_slot(
        &self,
        object: ObjectId,
        atom: Atom,
    ) -> Result<bool, RuntimeError> {
        if self.heap.object(object)?.kind != ObjectKind::ModuleNamespace {
            return Ok(false);
        }
        Ok(self
            .stored_own_property(object, atom)?
            .is_some_and(|selected| matches!(selected.slot, PropertySlot::VarRef(_))))
    }

    fn namespace_current(
        &mut self,
        poisoned: &Cell<bool>,
        object: ObjectId,
        atom: Atom,
    ) -> Result<CompletePropertyDescriptor<RawValue>, RuntimeError> {
        match self.select_stored_own_property(poisoned, object, atom)? {
            OwnPropertySelection::Ready(ReadyOwnProperty::Stored(
                record @ CompletePropertyDescriptor::Data { .. },
            )) => Ok(record),
            OwnPropertySelection::Missing => Err(RuntimeError::Invariant(
                "module namespace export slot has no own descriptor",
            )),
            _ => Err(RuntimeError::Invariant(
                "module namespace export slot is not a data descriptor",
            )),
        }
    }

    pub(crate) fn define_module_namespace_export_raw(
        &mut self,
        poisoned: &Cell<bool>,
        object: ObjectId,
        atom: Atom,
        descriptor: &PropertyDescriptor<RawValue>,
    ) -> Result<Option<bool>, RuntimeError> {
        if !self.module_namespace_export_slot(object, atom)? {
            return Ok(None);
        }
        // Attribute-only requests still perform the unconditional TDZ read.
        let CompletePropertyDescriptor::Data { value, .. } =
            self.namespace_current(poisoned, object, atom)?
        else {
            unreachable!("namespace_current is data");
        };
        Ok(Some(compatible(descriptor, &value, |left, right| {
            crate::engine::value::collection_key::same_value(&self.heap, left, right)
        })))
    }

    pub(crate) fn define_module_namespace_export_public(
        &mut self,
        poisoned: &Cell<bool>,
        object: ObjectId,
        atom: Atom,
        descriptor: &OrdinaryPropertyDescriptor,
    ) -> Result<Option<bool>, RuntimeError> {
        if !self.module_namespace_export_slot(object, atom)? {
            return Ok(None);
        }
        let record = self.namespace_current(poisoned, object, atom)?;
        let mut current = PublicCompleteDescriptorGuard::current(self, poisoned, &record)?;
        let record = current.record().clone();
        let CompletePropertyDescriptor::Data { value, .. } = record else {
            unreachable!("namespace_current is data");
        };
        let result = (|| {
            let state = current.state();
            let record = state.public_definition_record(descriptor)?;
            Ok(Some(compatible(&record, &value, |left, right| {
                left.same_value(right, state)
            })))
        })();
        if !poisoned.get() {
            current.retire()?;
        }
        result
    }
}
