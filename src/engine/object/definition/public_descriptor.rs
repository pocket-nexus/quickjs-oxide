//! Public mapped-Arguments descriptor roles without Runtime-backed roots.

use super::super::owned_descriptor::CompleteDescriptorGuard;
use super::super::property::{CompletePropertyDescriptor, PropertyDescriptor};
use super::super::{DescriptorField, OrdinaryPropertyDescriptor};
use crate::engine::api::runtime_error::RuntimeError;
use crate::engine::heap::RawValue;
use crate::engine::heap::runtime::RuntimeState;
use crate::engine::value::{JsString, JsValue, Value, bigint::JsBigInt};
use std::cell::Cell;

/// Public String/BigInt snapshots own payload clones, never arena edges.
/// Other variants borrow raw identities until the finite promotion guard owns
/// their exact Object/Symbol/accessor roles.
#[derive(Clone)]
pub(in crate::engine::object) enum PublicDefinitionValue {
    Raw(RawValue),
    String(JsString),
    BigInt(JsBigInt),
}

impl PublicDefinitionValue {
    fn from_raw(state: &RuntimeState, value: &RawValue) -> Result<Self, RuntimeError> {
        Ok(match value {
            RawValue::String(id) => Self::String(state.heap.string(*id)?.clone()),
            RawValue::BigInt(id) => Self::BigInt(state.heap.bigint(*id)?.clone()),
            RawValue::ShortBigInt(value) => Self::BigInt(JsBigInt::from(*value)),
            RawValue::Uninitialized | RawValue::Private(_) => {
                return Err(RuntimeError::Invariant(
                    "public descriptor held internal sentinel",
                ));
            }
            value => Self::Raw(value.clone()),
        })
    }

    fn from_public(state: &RuntimeState, value: &Value) -> Result<Self, RuntimeError> {
        Ok(match value {
            Value::String(value) => Self::String(value.clone()),
            Value::BigInt(value) => Self::BigInt(value.clone()),
            Value::Object(value) => Self::Raw(RawValue::Object(value.object_id())),
            Value::Symbol(value) => Self::Raw(RawValue::Symbol(state.atoms.unbrand(value.atom())?)),
            Value::Undefined => Self::Raw(RawValue::Undefined),
            Value::Null => Self::Raw(RawValue::Null),
            Value::Bool(value) => Self::Raw(RawValue::Bool(*value)),
            Value::Int(value) => Self::Raw(RawValue::Int(*value)),
            Value::Float(value) => Self::Raw(RawValue::Float(*value)),
        })
    }

    pub(in crate::engine::object) fn same_value(&self, other: &Self, state: &RuntimeState) -> bool {
        match (self, other) {
            (Self::String(left), Self::String(right)) => left == right,
            (Self::BigInt(left), Self::BigInt(right)) => left == right,
            (Self::Raw(left), Self::Raw(right)) => {
                crate::engine::value::collection_key::same_value(&state.heap, left, right)
            }
            _ => false,
        }
    }

    fn promotion_role(&self) -> RawValue {
        match self {
            Self::Raw(value) => value.clone(),
            Self::String(_) | Self::BigInt(_) => RawValue::Undefined,
        }
    }

    /// Match public unroot_value: checked Object/Symbol duplication, a fresh
    /// String/heap BigInt producer, or an immediate primitive.
    pub(in crate::engine::object) fn own_value(
        &self,
        state: &mut RuntimeState,
    ) -> Result<JsValue, RuntimeError> {
        match self {
            Self::Raw(value) => state.dup_jsvalue(&JsValue::from_raw(value.clone()).ok_or(
                RuntimeError::Invariant("public definition held internal sentinel"),
            )?),
            Self::String(value) => Ok(JsValue::String(state.heap.allocate_string(value.clone())?)),
            Self::BigInt(value) => match value.as_i64() {
                Some(value) => Ok(JsValue::ShortBigInt(value)),
                None => Ok(JsValue::BigInt(state.heap.allocate_bigint(value.clone())?)),
            },
        }
    }
}

pub(in crate::engine::object) struct PublicCompleteDescriptorGuard<'a> {
    owners: CompleteDescriptorGuard<'a>,
    record: CompletePropertyDescriptor<PublicDefinitionValue>,
}

impl<'a> PublicCompleteDescriptorGuard<'a> {
    pub(in crate::engine::object) fn promote(
        state: &'a mut RuntimeState,
        poisoned: &'a Cell<bool>,
        record: CompletePropertyDescriptor<PublicDefinitionValue>,
    ) -> Result<Self, RuntimeError> {
        let roles = match &record {
            CompletePropertyDescriptor::Data {
                value,
                writable,
                enumerable,
                configurable,
            } => CompletePropertyDescriptor::Data {
                value: value.promotion_role(),
                writable: *writable,
                enumerable: *enumerable,
                configurable: *configurable,
            },
            CompletePropertyDescriptor::Accessor {
                get,
                set,
                enumerable,
                configurable,
            } => CompletePropertyDescriptor::Accessor {
                get: get.as_ref().map(PublicDefinitionValue::promotion_role),
                set: set.as_ref().map(PublicDefinitionValue::promotion_role),
                enumerable: *enumerable,
                configurable: *configurable,
            },
        };
        let mut owners = CompleteDescriptorGuard::new(state, poisoned);
        if let Err(error) = owners.duplicate(&roles) {
            owners.retire()?;
            return Err(error);
        }
        Ok(Self { owners, record })
    }

    pub(in crate::engine::object) fn current(
        state: &'a mut RuntimeState,
        poisoned: &'a Cell<bool>,
        record: &CompletePropertyDescriptor<RawValue>,
    ) -> Result<Self, RuntimeError> {
        let public = match record {
            CompletePropertyDescriptor::Data {
                value,
                writable,
                enumerable,
                configurable,
            } => CompletePropertyDescriptor::Data {
                value: PublicDefinitionValue::from_raw(state, value)?,
                writable: *writable,
                enumerable: *enumerable,
                configurable: *configurable,
            },
            CompletePropertyDescriptor::Accessor {
                get,
                set,
                enumerable,
                configurable,
            } => CompletePropertyDescriptor::Accessor {
                get: get
                    .as_ref()
                    .map(|value| PublicDefinitionValue::from_raw(state, value))
                    .transpose()?,
                set: set
                    .as_ref()
                    .map(|value| PublicDefinitionValue::from_raw(state, value))
                    .transpose()?,
                enumerable: *enumerable,
                configurable: *configurable,
            },
        };
        Self::promote(state, poisoned, public)
    }

    pub(in crate::engine::object) fn record(
        &self,
    ) -> &CompletePropertyDescriptor<PublicDefinitionValue> {
        &self.record
    }
    pub(in crate::engine::object) fn state(&mut self) -> &mut RuntimeState {
        self.owners.state()
    }
    pub(in crate::engine::object) fn retire(&mut self) -> Result<(), RuntimeError> {
        self.owners.retire()
    }

    /// Public complete storage performs a separate String/BigInt producer
    /// conversion after the mapped cell write. Do not reuse its arena owner.
    pub(in crate::engine::object) fn store_mapped_argument(
        &mut self,
        poisoned: &Cell<bool>,
        object: crate::engine::heap::ObjectId,
        atom: crate::engine::atom::Atom,
        cell: crate::engine::heap::VarRefId,
    ) -> Result<(), RuntimeError> {
        let record = self.record.clone();
        if let CompletePropertyDescriptor::Data {
            writable: true,
            enumerable,
            configurable,
            ..
        } = &record
        {
            return self.state().store_mapped_argument_descriptor(
                poisoned,
                object,
                atom,
                cell,
                CompletePropertyDescriptor::Data {
                    value: RawValue::Undefined,
                    writable: true,
                    enumerable: *enumerable,
                    configurable: *configurable,
                },
            );
        }
        let mut producer = CompleteDescriptorGuard::new(self.state(), poisoned);
        let raw = match record {
            CompletePropertyDescriptor::Data {
                value,
                writable,
                enumerable,
                configurable,
            } => {
                let raw = match value {
                    PublicDefinitionValue::Raw(value) => value,
                    PublicDefinitionValue::String(value) => {
                        producer.public_value(&Value::String(value))?
                    }
                    PublicDefinitionValue::BigInt(value) => {
                        producer.public_value(&Value::BigInt(value))?
                    }
                };
                CompletePropertyDescriptor::Data {
                    value: raw,
                    writable,
                    enumerable,
                    configurable,
                }
            }
            CompletePropertyDescriptor::Accessor {
                get,
                set,
                enumerable,
                configurable,
            } => CompletePropertyDescriptor::Accessor {
                get: get.map(|value| value.promotion_role()),
                set: set.map(|value| value.promotion_role()),
                enumerable,
                configurable,
            },
        };
        let result = producer
            .state()
            .store_mapped_argument_descriptor(poisoned, object, atom, cell, raw);
        if !poisoned.get() {
            producer.retire()?;
        }
        result
    }
}

impl RuntimeState {
    pub(in crate::engine::object) fn public_definition_record(
        &self,
        descriptor: &OrdinaryPropertyDescriptor,
    ) -> Result<PropertyDescriptor<PublicDefinitionValue>, RuntimeError> {
        let accessor = |value: &super::super::AccessorValue| {
            value.as_callable().map(|value| {
                PublicDefinitionValue::Raw(RawValue::Object(value.as_object().object_id()))
            })
        };
        Ok(PropertyDescriptor {
            value: match &descriptor.value {
                DescriptorField::Absent => None,
                DescriptorField::Present(value) => {
                    Some(PublicDefinitionValue::from_public(self, value)?)
                }
            },
            writable: descriptor.writable.as_ref().into_option().copied(),
            get: descriptor.get.as_ref().into_option().map(accessor),
            set: descriptor.set.as_ref().into_option().map(accessor),
            enumerable: descriptor.enumerable.as_ref().into_option().copied(),
            configurable: descriptor.configurable.as_ref().into_option().copied(),
        })
    }
}
