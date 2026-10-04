//! One own-property selector with concrete public and owned materialization.

use super::property::CompletePropertyDescriptor;
use super::shape::PropertyFlags;
use super::{
    CompleteOrdinaryPropertyDescriptor, ObjectRef, OwnedCompletePropertyDescriptor, PropertyKey,
};
use crate::engine::api::error::ErrorKind;
use crate::engine::api::runtime::Runtime;
use crate::engine::api::runtime_error::RuntimeError;
use crate::engine::atom::{Atom, AtomIdx};
use crate::engine::builtins::{
    CanonicalNumericIndex, SharedTypedOwnWord, TypedOwnProperty, TypedOwnWord,
};
use crate::engine::heap::runtime::RuntimeState;
use crate::engine::heap::{
    ObjectId, ObjectKind, ObjectPayload, PrimitiveObjectData, PropertySlot, RawValue,
};
use crate::engine::value::{JsString, Value};
use std::cell::Cell;

/// Ready records are borrowed heap edges protected by the caller's object
/// root. Consume them immediately under State or at the public root boundary;
/// they cannot cross JavaScript or a property mutation.
pub(crate) enum ReadyOwnProperty {
    Stored(CompletePropertyDescriptor<RawValue>),
    StringIndex(JsString),
    TypedWord(TypedOwnWord),
}

/// Ready snapshots or the actual shared backing mutex boundary. AutoInit
/// transitions finish inside the canonical State selector before this reply.
pub(crate) enum OwnPropertySelection {
    Missing,
    /// Numeric TypedArray misses terminate Get before prototype lookup.
    TerminalMissing,
    Ready(ReadyOwnProperty),
    Shared(SharedTypedOwnWord),
}

impl RuntimeState {
    pub(crate) fn string_exotic_index_value(
        &self,
        object: ObjectId,
        atom: Atom,
    ) -> Result<Option<JsString>, RuntimeError> {
        let data = self.heap.object(object)?;
        let ObjectPayload::Primitive(PrimitiveObjectData::String(value)) = &data.payload else {
            return Ok(None);
        };
        let Some(index) = self.atoms.array_index(atom)? else {
            return Ok(None);
        };
        let Ok(index) = usize::try_from(index) else {
            return Ok(None);
        };
        Ok(self
            .heap
            .string(*value)?
            .code_unit_at(index)
            .map(JsString::from_code_unit))
    }

    pub(crate) fn string_exotic_length(
        &self,
        object: ObjectId,
    ) -> Result<Option<usize>, RuntimeError> {
        let data = self.heap.object(object)?;
        Ok(match &data.payload {
            ObjectPayload::Primitive(PrimitiveObjectData::String(value)) => {
                Some(self.heap.string(*value)?.len())
            }
            _ => None,
        })
    }

    pub(crate) fn dense_array_index_value(
        &self,
        object: ObjectId,
        atom: Atom,
    ) -> Result<Option<RawValue>, RuntimeError> {
        let data = self.heap.object(object)?;
        let ObjectPayload::Array { dense: Some(dense) } = &data.payload else {
            return Ok(None);
        };
        let Some(index) = self.atoms.array_index(atom)? else {
            return Ok(None);
        };
        Ok(dense.get(index as usize).cloned())
    }

    /// Inputs are admitted same-runtime identities. No Runtime deferred drain,
    /// public root, callback, or shared backing mutex is entered here.
    pub(crate) fn select_own_property(
        &mut self,
        poisoned: &Cell<bool>,
        object: ObjectId,
        atom: Atom,
    ) -> Result<OwnPropertySelection, RuntimeError> {
        match &self.heap.object(object)?.payload {
            ObjectPayload::TypedArray(_) => {
                let key = self.typed_own_key(atom)?;
                if let Some(numeric) = self.typed_canonical_numeric_index(atom, key)? {
                    return match numeric {
                        CanonicalNumericIndex::Invalid => Ok(OwnPropertySelection::TerminalMissing),
                        CanonicalNumericIndex::Valid(index) => {
                            Ok(match self.select_typed_own_property(object, index)? {
                                TypedOwnProperty::Missing => OwnPropertySelection::TerminalMissing,
                                TypedOwnProperty::Word(word) => {
                                    OwnPropertySelection::Ready(ReadyOwnProperty::TypedWord(word))
                                }
                                TypedOwnProperty::Shared(word) => {
                                    OwnPropertySelection::Shared(word)
                                }
                            })
                        }
                    };
                }
            }
            ObjectPayload::Primitive(PrimitiveObjectData::String(_)) => {
                if let Some(value) = self.string_exotic_index_value(object, atom)? {
                    return Ok(OwnPropertySelection::Ready(ReadyOwnProperty::StringIndex(
                        value,
                    )));
                }
            }
            ObjectPayload::Array { dense: Some(_) } => {
                if let Some(value) = self.dense_array_index_value(object, atom)? {
                    return Ok(OwnPropertySelection::Ready(ReadyOwnProperty::Stored(
                        CompletePropertyDescriptor::Data {
                            value,
                            writable: true,
                            enumerable: true,
                            configurable: true,
                        },
                    )));
                }
            }
            _ => {}
        }
        self.select_stored_own_property(poisoned, object, atom)
    }

    /// Canonical shape/parallel-slot selection for every stored class. Owned
    /// reads use the same checked malformed-layout errors as public reads.
    fn select_stored_own_property(
        &mut self,
        poisoned: &Cell<bool>,
        object: ObjectId,
        atom: Atom,
    ) -> Result<OwnPropertySelection, RuntimeError> {
        loop {
            let data = self.heap.object(object)?;
            let ordinary = matches!(
                (data.kind, &data.payload),
                (ObjectKind::Ordinary, ObjectPayload::Ordinary)
            );
            let shape = self.heap.shape(data.shape)?;
            let Some(index) = shape.find(AtomIdx::from_raw(atom.raw())) else {
                return Ok(OwnPropertySelection::Missing);
            };
            let index = usize::try_from(index)
                .map_err(|_| RuntimeError::Invariant("shape index does not fit usize"))?;
            let entry = shape
                .entries()
                .get(index)
                .ok_or(RuntimeError::Invariant(if ordinary {
                    "ordinary shape index is out of bounds"
                } else {
                    "shape lookup index was out of bounds"
                }))?;
            let slot = data
                .slots
                .get(index)
                .ok_or(RuntimeError::Invariant(if ordinary {
                    "ordinary shape has no parallel slot"
                } else {
                    "object property slot was missing"
                }))?;
            let flags = entry.flags;
            let record = match slot {
                PropertySlot::Data(value) => Self::own_data_record(value.clone(), flags),
                PropertySlot::VarRef(cell) => {
                    let value = self.raw_var_ref_value(*cell)?;
                    if matches!(value, RawValue::Uninitialized) {
                        return Err(RuntimeError::Engine(self.native_atom_error(
                            ErrorKind::Reference,
                            "",
                            atom,
                            " is not initialized",
                        )?));
                    }
                    Self::own_data_record(value, flags)
                }
                PropertySlot::Accessor { get, set } => CompletePropertyDescriptor::Accessor {
                    get: get.option().map(RawValue::Object),
                    set: set.option().map(RawValue::Object),
                    enumerable: flags.enumerable,
                    configurable: flags.configurable,
                },
                PropertySlot::AutoInit(_) => {
                    // The complete State materializer performs the canonical
                    // factory and slot transition. Reselect only after that
                    // mutation, with no Runtime or public descriptor roundtrip.
                    self.materialize_auto_init_property(poisoned, object, atom)?;
                    continue;
                }
            };
            return Ok(OwnPropertySelection::Ready(ReadyOwnProperty::Stored(
                record,
            )));
        }
    }

    fn own_data_record(
        value: RawValue,
        flags: PropertyFlags,
    ) -> CompletePropertyDescriptor<RawValue> {
        CompletePropertyDescriptor::Data {
            value,
            writable: flags.writable,
            enumerable: flags.enumerable,
            configurable: flags.configurable,
        }
    }

    pub(crate) fn own_selected_property_descriptor(
        &mut self,
        poisoned: &Cell<bool>,
        ready: ReadyOwnProperty,
    ) -> Result<CompletePropertyDescriptor<RawValue>, RuntimeError> {
        match ready {
            ReadyOwnProperty::Stored(record) => self.retain_complete_descriptor(poisoned, &record),
            ReadyOwnProperty::StringIndex(value) => {
                self.own_virtual_data_descriptor(poisoned, Value::String(value), false, true, false)
            }
            ReadyOwnProperty::TypedWord(word) => self.own_virtual_data_descriptor(
                poisoned,
                word.into_public_value(),
                true,
                true,
                true,
            ),
        }
    }
}

impl Runtime {
    /// Public admission intentionally precedes domain errors, as before.
    pub fn get_own_property(
        &self,
        object: &ObjectRef,
        key: &PropertyKey,
    ) -> Result<Option<CompleteOrdinaryPropertyDescriptor>, RuntimeError> {
        let _operation = self.operation()?;
        self.validate_object_and_key(object, key)?;
        self.get_own_property_in_operation(object, key)
    }

    pub(super) fn get_own_property_in_operation(
        &self,
        object: &ObjectRef,
        key: &PropertyKey,
    ) -> Result<Option<CompleteOrdinaryPropertyDescriptor>, RuntimeError> {
        self.select_public_own_property(object, key)?
            .as_ref()
            .map(|ready| self.root_selected_own_property(ready))
            .transpose()
    }

    pub(crate) fn get_own_property_owned(
        &self,
        object: &ObjectRef,
        key: &PropertyKey,
    ) -> Result<Option<OwnedCompletePropertyDescriptor>, RuntimeError> {
        // Internal migration adapter contract: domains first, then one
        // operation admission. Final State consumers bypass this adapter.
        // This intentionally replaces the old selective virtual/TDZ drains.
        self.validate_object_and_key(object, key)?;
        let operation = self.operation()?;
        let result = self.get_own_property_owned_in_operation(object, key);
        // Runtime owns its deferred FIFO. State retires only heap/atom edges;
        // no historical release-finish metadata belongs to a descriptor.
        drop(operation);
        if result.is_ok() && self.0.poisoned.get() {
            return Err(RuntimeError::Poisoned);
        }
        result
    }

    fn get_own_property_owned_in_operation(
        &self,
        object: &ObjectRef,
        key: &PropertyKey,
    ) -> Result<Option<OwnedCompletePropertyDescriptor>, RuntimeError> {
        let record = {
            let mut state = self.0.state.borrow_mut();
            match state.select_own_property(&self.0.poisoned, object.object_id(), key.atom())? {
                OwnPropertySelection::Missing | OwnPropertySelection::TerminalMissing => {
                    return Ok(None);
                }
                OwnPropertySelection::Ready(ready) => {
                    state.own_selected_property_descriptor(&self.0.poisoned, ready)?
                }
                OwnPropertySelection::Shared(word) => {
                    // The Arc seed owns backing identity and selected
                    // bounds. Leave State before the shared mutex read.
                    drop(state);
                    let ready = ReadyOwnProperty::TypedWord(word.read()?);
                    self.0
                        .state
                        .borrow_mut()
                        .own_selected_property_descriptor(&self.0.poisoned, ready)?
                }
            }
        };
        // No fallible work or callback between releasing State and adoption.
        Ok(Some(OwnedCompletePropertyDescriptor::from_owned_record(
            self, record,
        )))
    }

    fn select_public_own_property(
        &self,
        object: &ObjectRef,
        key: &PropertyKey,
    ) -> Result<Option<ReadyOwnProperty>, RuntimeError> {
        let selected = self.0.state.borrow_mut().select_own_property(
            &self.0.poisoned,
            object.object_id(),
            key.atom(),
        )?;
        match selected {
            OwnPropertySelection::Missing | OwnPropertySelection::TerminalMissing => Ok(None),
            OwnPropertySelection::Ready(ready) => Ok(Some(ready)),
            OwnPropertySelection::Shared(word) => {
                Ok(Some(ReadyOwnProperty::TypedWord(word.read()?)))
            }
        }
    }

    /// Public String/BigInt payloads clone without an arena retain. Object,
    /// Symbol, getter and setter roots each preserve one checked promotion.
    fn root_selected_own_property(
        &self,
        ready: &ReadyOwnProperty,
    ) -> Result<CompleteOrdinaryPropertyDescriptor, RuntimeError> {
        Ok(match ready {
            ReadyOwnProperty::Stored(record) => self.root_complete_descriptor(record)?,
            ReadyOwnProperty::StringIndex(value) => CompleteOrdinaryPropertyDescriptor::Data {
                value: Value::String(value.clone()),
                writable: false,
                enumerable: true,
                configurable: false,
            },
            ReadyOwnProperty::TypedWord(word) => CompleteOrdinaryPropertyDescriptor::Data {
                value: TypedOwnWord {
                    element: word.element,
                    bytes: word.bytes,
                }
                .into_public_value(),
                writable: true,
                enumerable: true,
                configurable: true,
            },
        })
    }
}
