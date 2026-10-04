use crate::engine::api::error::{ErrorKind, NativeErrorKind};
use crate::engine::api::runtime::Runtime;
use crate::engine::api::runtime_error::RuntimeError;
use crate::engine::atom::{Atom, AtomIdx};
use crate::engine::heap::runtime::RuntimeState;

use crate::engine::heap::{ContextId, ObjectId, PropertySlot, RawValue};
use crate::engine::object::operations::RawStringProperty;
use crate::engine::object::ordinary::OrdinaryRead;
use crate::engine::object::{ObjectRef, PropertyKey};
use crate::engine::value::conversion::NativeConversion;
use crate::engine::value::{JsString, JsValue, Value};
use crate::engine::vm::Completion;

impl Runtime {
    /// Internal primitive bases remain borrowed arena handles throughout delete.
    pub(crate) fn primitive_delete_property_jsvalue(
        &self,
        base: &JsValue,
        key: &PropertyKey,
    ) -> Result<bool, RuntimeError> {
        if !key.belongs_to(self) {
            return Err(RuntimeError::WrongRuntime("delete property key"));
        }
        Ok(match base {
            JsValue::Null | JsValue::Undefined => {
                return Err(RuntimeError::Engine(crate::engine::api::Error::new(
                    ErrorKind::Type,
                    "cannot convert to object",
                )));
            }
            JsValue::Object(_) => {
                return Err(RuntimeError::Invariant(
                    "primitive Delete received an object",
                ));
            }
            JsValue::String(id) => {
                let state = self.0.state.borrow();
                let string = state.heap.string(*id)?;
                let index = state.atoms.array_index(key.atom())?;
                let indexed = index.is_some_and(|index| {
                    usize::try_from(index).is_ok_and(|index| index < string.len())
                });
                !indexed
                    && key.atom()
                        != state
                            .pinned_atoms
                            .get(crate::engine::atom::pinned::PinnedAtom::Length)
            }
            _ => true,
        })
    }

    pub(crate) fn finish_property_delete(
        &self,
        result: NativeConversion<bool>,
        strict: bool,
    ) -> Result<Completion, RuntimeError> {
        match result {
            NativeConversion::Throw(value) => Ok(Completion::Throw(value)),
            NativeConversion::Value(false) if strict => Err(RuntimeError::Engine(
                crate::engine::api::Error::new(ErrorKind::Type, "could not delete property"),
            )),
            NativeConversion::Value(value) => Ok(Completion::Return(JsValue::Bool(value))),
        }
    }

    pub(crate) fn get_property_in_realm(
        &self,
        realm: ContextId,
        object: &ObjectRef,
        key: &PropertyKey,
    ) -> Result<Completion, RuntimeError> {
        self.internal_get(realm, object, key, Value::Object(object.try_clone()?))
    }

    /// Primitive inputs and prototype lookup use one shared State algorithm.
    pub(crate) fn prepare_value_property_read_borrowed_jsvalue(
        &self,
        realm: ContextId,
        receiver: &JsValue,
        key: &PropertyKey,
    ) -> Result<OrdinaryRead, RuntimeError> {
        self.prepare_value_property_read_selected_jsvalue(realm, receiver, key, None)
    }

    pub(crate) fn prepare_value_property_read_selected_jsvalue(
        &self,
        realm: ContextId,
        receiver: &JsValue,
        key: &PropertyKey,
        native: Option<&mut Option<crate::engine::object::LinkedNativeSelection>>,
    ) -> Result<OrdinaryRead, RuntimeError> {
        // Internal read admission is operation-first for every value kind.
        // State owns the complete algorithm; Runtime alone owns its FIFO.
        let _operation = self.operation()?;
        if !key.belongs_to(self) {
            return Err(RuntimeError::WrongRuntime("property key"));
        }
        self.prepare_admitted_value_property_read_jsvalue(realm, receiver, key.atom(), native)
    }

    /// The executing PublishedFunctionSnapshot owns this linked atom. Normal
    /// selection consumes no key owner; a pending Proxy request promotes one
    /// at the actual callback boundary after lookup has completed.
    pub(crate) fn prepare_linked_value_property_read_jsvalue(
        &self,
        realm: ContextId,
        receiver: &JsValue,
        atom: Atom,
    ) -> Result<OrdinaryRead, RuntimeError> {
        let _operation = self.operation()?;
        self.prepare_admitted_value_property_read_jsvalue(realm, receiver, atom, None)
    }

    fn prepare_admitted_value_property_read_jsvalue(
        &self,
        realm: ContextId,
        receiver: &JsValue,
        atom: Atom,
        native: Option<&mut Option<crate::engine::object::LinkedNativeSelection>>,
    ) -> Result<OrdinaryRead, RuntimeError> {
        let step = self.0.state.borrow_mut().prepare_value_read_in_state(
            &self.0.poisoned,
            self.domain_id(),
            realm,
            receiver,
            atom,
            native,
        )?;
        self.finish_read_boundary(step)
    }

    fn finish_value_property_read(
        &self,
        realm: ContextId,
        key: &PropertyKey,
        read: OrdinaryRead,
    ) -> Result<Completion, RuntimeError> {
        Ok(match self.finish_prepared_read_jsvalue(realm, key, read)? {
            NativeConversion::Value(value) => {
                Completion::Return(value.unwrap_or(JsValue::Undefined))
            }
            NativeConversion::Throw(value) => Completion::Throw(value),
        })
    }

    /// Keep JavaScript-visible read failures as replies to the selected operation.
    pub(crate) fn prepare_value_property_read_completion(
        &self,
        realm: ContextId,
        receiver: JsValue,
        key: &PropertyKey,
    ) -> Result<NativeConversion<OrdinaryRead>, RuntimeError> {
        let nullish = matches!(receiver, JsValue::Null | JsValue::Undefined);
        let read = self.prepare_value_property_read_borrowed_jsvalue(realm, &receiver, key);
        self.release_jsvalue(receiver)?;
        match read {
            Ok(read) => Ok(NativeConversion::Value(read)),
            Err(RuntimeError::Engine(error)) if nullish && error.kind() == ErrorKind::Type => {
                Ok(NativeConversion::Throw(
                    self.new_native_error_from_error_jsvalue(realm, NativeErrorKind::Type, &error)?,
                ))
            }
            Err(error) => Err(error),
        }
    }

    #[cfg(test)]
    pub(crate) fn get_value_property_in_realm(
        &self,
        realm: ContextId,
        receiver: Value,
        key: &PropertyKey,
    ) -> Result<Completion, RuntimeError> {
        self.get_value_property_in_realm_jsvalue(realm, self.into_jsvalue(receiver)?, key)
    }

    pub(crate) fn get_value_property_in_realm_jsvalue(
        &self,
        realm: ContextId,
        receiver: JsValue,
        key: &PropertyKey,
    ) -> Result<Completion, RuntimeError> {
        let nullish = matches!(receiver, JsValue::Null | JsValue::Undefined);
        let read = self.prepare_value_property_read_borrowed_jsvalue(realm, &receiver, key);
        self.release_jsvalue(receiver)?;
        match read {
            Ok(read) => self.finish_value_property_read(realm, key, read),
            Err(RuntimeError::Engine(error)) if nullish && error.kind() == ErrorKind::Type => {
                Ok(Completion::Throw(
                    self.new_native_error_from_error_jsvalue(realm, NativeErrorKind::Type, &error)?,
                ))
            }
            Err(error) => Err(error),
        }
    }

    #[cfg(test)]
    pub(crate) fn has_property(
        &self,
        object: &ObjectRef,
        key: &PropertyKey,
    ) -> Result<bool, RuntimeError> {
        let mut cursor = Some(object.try_clone()?);
        while let Some(current) = cursor {
            if self.has_own_property(&current, key)? {
                return Ok(true);
            }
            cursor = self.get_prototype_of(&current)?;
        }
        Ok(false)
    }
}

pub(crate) fn raw_string_property_on_object(
    state: &RuntimeState,
    object: ObjectId,
    atom: Atom,
) -> Result<RawStringProperty, RuntimeError> {
    let object = state.heap.object(object)?;
    let shape = state.heap.shape(object.shape)?;
    let Some(index) = shape.find(AtomIdx::from_raw(atom.raw())) else {
        return Ok(RawStringProperty::Missing);
    };
    let slot = object
        .slots
        .get(index as usize)
        .ok_or(RuntimeError::Invariant(
            "backtrace name shape has no parallel property slot",
        ))?;
    Ok(match slot {
        PropertySlot::Data(RawValue::String(value)) => {
            let string = state.heap.string(*value)?;
            if string.is_flat() {
                RawStringProperty::String(string.clone())
            } else {
                RawStringProperty::Other
            }
        }
        PropertySlot::Data(_)
        | PropertySlot::VarRef(_)
        | PropertySlot::Accessor { .. }
        | PropertySlot::AutoInit(_) => RawStringProperty::Other,
    })
}

pub(crate) fn raw_string_property_one_level(
    state: &RuntimeState,
    object: ObjectId,
    atom: Atom,
) -> Result<Option<JsString>, RuntimeError> {
    match raw_string_property_on_object(state, object, atom)? {
        RawStringProperty::String(name) => return Ok(Some(name)),
        RawStringProperty::Other => return Ok(None),
        RawStringProperty::Missing => {}
    }

    let object = state.heap.object(object)?;
    let Some(prototype) = state.heap.shape(object.shape)?.prototype() else {
        return Ok(None);
    };
    Ok(
        match raw_string_property_on_object(state, prototype, atom)? {
            RawStringProperty::String(name) => Some(name),
            RawStringProperty::Missing | RawStringProperty::Other => None,
        },
    )
}
