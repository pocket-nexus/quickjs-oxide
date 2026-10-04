//! Complete ordinary descriptor validation and commit under one State lease.

use super::operations::{
    ValidationValue, complete_to_validation_record, descriptor_to_validation_record,
};
use super::own_properties::{OwnPropertySelection, ReadyOwnProperty};
use super::owned_descriptor::CompleteDescriptorGuard;
use super::property::{
    CompletePropertyDescriptor, PropertyDefinitionError, PropertyDescriptor,
    validate_and_apply_property_descriptor,
};
use super::{CompleteOrdinaryPropertyDescriptor, DescriptorField, OrdinaryPropertyDescriptor};
use crate::engine::api::runtime_error::RuntimeError;
use crate::engine::atom::Atom;
use crate::engine::heap::runtime::RuntimeState;
use crate::engine::heap::{ObjectId, ObjectPayload, PropertySlot, RawValue};
use crate::engine::value::{JsString, Value};
use std::cell::Cell;

mod namespace;
pub(super) mod public_descriptor;

#[derive(Clone, Copy)]
enum StringValidation<'a> {
    Raw(&'a RawValue),
    String(&'a JsString),
    Undefined,
}

pub(super) fn accepted<T>(
    result: Result<T, PropertyDefinitionError>,
) -> Result<Option<T>, RuntimeError> {
    match result {
        Ok(value) => Ok(Some(value)),
        Err(PropertyDefinitionError::InvalidDescriptor) => {
            Err(PropertyDefinitionError::InvalidDescriptor.into())
        }
        Err(_) => Ok(None),
    }
}

impl RuntimeState {
    /// Complete callback-free own definition after the boundary selected any
    /// Proxy/typed-index coercion and Array length value conversion. No such
    /// request is replayed here. Typed named keys keep ordinary slot semantics.
    pub(crate) fn define_own_raw_property(
        &mut self,
        poisoned: &Cell<bool>,
        object: ObjectId,
        atom: Atom,
        descriptor: &PropertyDescriptor<RawValue>,
    ) -> Result<bool, RuntimeError> {
        if let super::operations::ArrayOwnKey::Index(index) = self.array_own_key(object, atom)? {
            // Owned Array-index admission historically rejects a read-only
            // length before validating even a malformed mixed descriptor.
            return self.define_array_index_property(poisoned, object, atom, index, descriptor);
        }
        if descriptor.is_accessor_descriptor() && descriptor.is_data_descriptor() {
            return Err(PropertyDefinitionError::InvalidDescriptor.into());
        }
        if let Some(accepted) =
            self.define_module_namespace_export_raw(poisoned, object, atom, descriptor)?
        {
            return Ok(accepted);
        }
        if let Some(accepted) =
            self.define_arguments_index_raw(poisoned, object, atom, descriptor)?
        {
            return Ok(accepted);
        }
        self.define_ordinary_raw_property(poisoned, object, atom, descriptor)
    }

    /// Check lazy flags before materializing, then borrow the current record.
    /// The caller retains its receiver throughout production/validation/commit.
    pub(super) fn prepare_ordinary_definition(
        &mut self,
        poisoned: &Cell<bool>,
        object: ObjectId,
        atom: Atom,
        descriptor: &PropertyDescriptor<RawValue>,
    ) -> Result<Option<Option<CompletePropertyDescriptor<RawValue>>>, RuntimeError> {
        if descriptor.is_accessor_descriptor() && descriptor.is_data_descriptor() {
            return Err(PropertyDefinitionError::InvalidDescriptor.into());
        }
        if let Some(selected) = self.stored_own_property(object, atom)?
            && matches!(selected.slot, PropertySlot::AutoInit(_))
        {
            let flags = selected.flags;
            if !flags.configurable
                && (descriptor.configurable == Some(true)
                    || descriptor
                        .enumerable
                        .is_some_and(|value| value != flags.enumerable)
                    || descriptor.is_accessor_descriptor()
                    || (!flags.writable && descriptor.writable == Some(true)))
            {
                return Ok(None);
            }
        }
        let current = match self.select_stored_own_property(poisoned, object, atom)? {
            OwnPropertySelection::Missing => None,
            OwnPropertySelection::Ready(ReadyOwnProperty::Stored(record))
            | OwnPropertySelection::CyclePublished(ReadyOwnProperty::Stored(record)) => {
                Some(record)
            }
            _ => {
                return Err(RuntimeError::Invariant(
                    "stored selection produced a virtual descriptor",
                ));
            }
        };
        Ok(Some(current))
    }

    pub(super) fn apply_ordinary_definition(
        &mut self,
        poisoned: &Cell<bool>,
        object: ObjectId,
        atom: Atom,
        descriptor: &PropertyDescriptor<RawValue>,
        current: Option<&CompletePropertyDescriptor<RawValue>>,
    ) -> Result<bool, RuntimeError> {
        let Some(complete) = accepted(validate_and_apply_property_descriptor(
            self.heap.object(object)?.extensible,
            descriptor,
            current,
            &RawValue::Undefined,
            |left, right| crate::engine::value::collection_key::same_value(&self.heap, left, right),
        ))?
        else {
            return Ok(false);
        };
        match self.heap.object(object)?.payload {
            ObjectPayload::GlobalObject { uninitialized_vars } => self
                .store_complete_global_raw_property(
                    poisoned,
                    object,
                    uninitialized_vars,
                    atom,
                    complete,
                )?,
            _ => self.store_complete_raw_property_with_poison(poisoned, object, atom, complete)?,
        }
        Ok(true)
    }

    /// No temporary current-descriptor owner is requested under this lease.
    /// Incoming and published edges still use their ordinary checked retains.
    pub(crate) fn define_ordinary_raw_property(
        &mut self,
        poisoned: &Cell<bool>,
        object: ObjectId,
        atom: Atom,
        descriptor: &PropertyDescriptor<RawValue>,
    ) -> Result<bool, RuntimeError> {
        if let Some(value) = self.string_exotic_index_value(object, atom)? {
            return self.validate_string_raw_definition(object, &value, descriptor);
        }
        let Some(current) = self.prepare_ordinary_definition(poisoned, object, atom, descriptor)?
        else {
            return Ok(false);
        };
        self.apply_ordinary_definition(poisoned, object, atom, descriptor, current.as_ref())
    }

    /// Public representation has no current String arena owner. Preserve the
    /// original payload-only virtual compatibility algorithm before production.
    pub(super) fn define_ordinary_public_property(
        &mut self,
        poisoned: &Cell<bool>,
        object: ObjectId,
        atom: Atom,
        descriptor: &OrdinaryPropertyDescriptor,
    ) -> Result<bool, RuntimeError> {
        if let Some(value) = self.string_exotic_index_value(object, atom)? {
            let current = CompleteOrdinaryPropertyDescriptor::Data {
                value: Value::String(value),
                writable: false,
                enumerable: true,
                configurable: false,
            };
            return accepted(validate_and_apply_property_descriptor(
                self.heap.object(object)?.extensible,
                &descriptor_to_validation_record(descriptor),
                Some(&complete_to_validation_record(&current)),
                &ValidationValue::Undefined,
                ValidationValue::same_value,
            ))
            .map(|value| value.is_some());
        }
        let mut record = Self::public_descriptor_attributes(descriptor);
        // Only presence is needed for the mixed/lazy prechecks. Production
        // happens after TDZ selection, but before ordinary permission checks.
        record.value = descriptor.value.is_present().then_some(RawValue::Undefined);
        let Some(current) = self.prepare_ordinary_definition(poisoned, object, atom, &record)?
        else {
            return Ok(false);
        };
        let mut producer = CompleteDescriptorGuard::new(self, poisoned);
        if let DescriptorField::Present(value) = &descriptor.value {
            record.value = Some(producer.public_value(value)?);
        }
        let result = producer.state().apply_ordinary_definition(
            poisoned,
            object,
            atom,
            &record,
            current.as_ref(),
        );
        if !poisoned.get() {
            producer.retire()?;
        }
        result
    }

    pub(super) fn public_descriptor_attributes(
        descriptor: &OrdinaryPropertyDescriptor,
    ) -> PropertyDescriptor<RawValue> {
        PropertyDescriptor {
            value: None,
            writable: descriptor.writable.as_ref().into_option().copied(),
            get: descriptor.get.as_ref().into_option().map(|value| {
                value
                    .as_callable()
                    .map(|value| RawValue::Object(value.as_object().object_id()))
            }),
            set: descriptor.set.as_ref().into_option().map(|value| {
                value
                    .as_callable()
                    .map(|value| RawValue::Object(value.as_object().object_id()))
            }),
            enumerable: descriptor.enumerable.as_ref().into_option().copied(),
            configurable: descriptor.configurable.as_ref().into_option().copied(),
        }
    }

    fn validate_string_raw_definition(
        &self,
        object: ObjectId,
        value: &JsString,
        descriptor: &PropertyDescriptor<RawValue>,
    ) -> Result<bool, RuntimeError> {
        let record = PropertyDescriptor {
            value: descriptor.value.as_ref().map(StringValidation::Raw),
            writable: descriptor.writable,
            get: descriptor
                .get
                .as_ref()
                .map(|value| value.as_ref().map(StringValidation::Raw)),
            set: descriptor
                .set
                .as_ref()
                .map(|value| value.as_ref().map(StringValidation::Raw)),
            enumerable: descriptor.enumerable,
            configurable: descriptor.configurable,
        };
        let current = CompletePropertyDescriptor::Data {
            value: StringValidation::String(value),
            writable: false,
            enumerable: true,
            configurable: false,
        };
        accepted(validate_and_apply_property_descriptor(
            self.heap.object(object)?.extensible, &record, Some(&current), &StringValidation::Undefined,
            |left, right| match (left, right) {
                (StringValidation::Raw(left), StringValidation::Raw(right)) =>
                    crate::engine::value::collection_key::same_value(&self.heap, left, right),
                (StringValidation::String(left), StringValidation::String(right)) => left == right,
                (StringValidation::String(value), StringValidation::Raw(RawValue::String(id)))
                | (StringValidation::Raw(RawValue::String(id)), StringValidation::String(value)) =>
                    self.heap.string(*id).is_ok_and(|stored| stored == *value),
                (StringValidation::Undefined, StringValidation::Undefined)
                | (StringValidation::Undefined, StringValidation::Raw(RawValue::Undefined))
                | (StringValidation::Raw(RawValue::Undefined), StringValidation::Undefined) => true,
                _ => false,
            },
        )).map(|value| value.is_some())
    }
}
