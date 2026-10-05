//! ECMAScript Module Namespace Exotic Object storage helpers.
//!
//! QuickJS keeps namespace live bindings in ordinary `JS_PROP_VARREF` slots
//! and selects the exceptional write/define/own-key behavior through the
//! `JS_CLASS_MODULE_NS` marker.  Oxide uses the same split: the arena already
//! traces `PropertySlot::VarRef`, so this module owns only class-specific
//! construction and dispatch helpers.

use crate::engine::api::runtime::Runtime;
use crate::engine::api::runtime_error::RuntimeError;
use crate::engine::atom::{Atom, AtomIdx, PropertyKeyKind};

use crate::engine::api::error::ErrorKind;
use crate::engine::heap::{
    ObjectData, ObjectId, ObjectKind, PropertySlot, RawValue, runtime::RuntimeState,
};
use crate::engine::object::property::PropertyDescriptor;
use crate::engine::object::{ObjectRef, OrdinaryPropertyDescriptor, PropertyKey};

impl Runtime {
    pub(crate) fn is_module_namespace_object(
        &self,
        object: &ObjectRef,
    ) -> Result<bool, RuntimeError> {
        if !object.belongs_to(self) {
            return Err(RuntimeError::WrongRuntime("object"));
        }
        Ok(matches!(
            self.0.state.borrow().heap.object(object.object_id())?.kind,
            ObjectKind::ModuleNamespace
        ))
    }

    /// Allocate the null-prototype, already non-extensible namespace shell.
    ///
    /// The linker caches this root before populating it, so cyclic
    /// `export * as` graphs can refer to the unique placeholder identity.
    pub(crate) fn new_module_namespace_object(&self) -> Result<ObjectRef, RuntimeError> {
        let mut state = self.0.state.borrow_mut();
        let shape = state.get_or_create_shape(None, &[])?;
        let object = match state
            .heap
            .allocate_object(ObjectData::module_namespace(shape, Vec::new()))
        {
            Ok(object) => object,
            Err(error) => {
                let cleanup = state.heap.release_shape(shape)?;
                state.apply_cleanup(cleanup)?;
                return Err(error.into());
            }
        };
        let cleanup = state.heap.release_shape(shape)?;
        state.apply_cleanup(cleanup)?;
        drop(state);
        Ok(ObjectRef::from_owned_handle(self.clone(), object))
    }

    /// Whether `key` is one of the live export VarRef properties rather than
    /// the ordinary non-configurable `@@toStringTag` data property.
    pub(crate) fn module_namespace_export_slot(
        &self,
        object: &ObjectRef,
        key: &PropertyKey,
    ) -> Result<bool, RuntimeError> {
        if !self.is_module_namespace_object(object)? {
            return Ok(false);
        }
        if !key.belongs_to(self) {
            return Err(RuntimeError::WrongRuntime("property key"));
        }
        let state = self.0.state.borrow();
        let object = state.heap.object(object.object_id())?;
        let shape = state.heap.shape(object.shape)?;
        let Some(index) = shape.find(AtomIdx::from_raw(key.atom().raw())) else {
            return Ok(false);
        };
        Ok(matches!(
            object.slots.get(index as usize),
            Some(PropertySlot::VarRef(_))
        ))
    }

    /// Return namespace own keys in physical insertion order.
    ///
    /// The linker inserts UTF-16-sorted export names followed by
    /// `Symbol.toStringTag`.  The ordinary shape helper cannot be used here:
    /// it would reclassify integer-looking export names as array indices and
    /// thereby destroy the mandated lexicographic ordering.
    pub(crate) fn module_namespace_own_property_keys(
        &self,
        object: &ObjectRef,
    ) -> Result<Option<Vec<PropertyKey>>, RuntimeError> {
        if !self.is_module_namespace_object(object)? {
            return Ok(None);
        }
        let atoms = {
            let state = self.0.state.borrow();
            let object = state.heap.object(object.object_id())?;
            let mut atoms = Vec::new();
            for entry in state.heap.shape(object.shape)?.entries() {
                // Public exit boundary: re-brand the stored unbranded index
                // before handing the atom to `PropertyKey` construction.
                let atom = state.atoms.brand(entry.atom)?;
                if state.atoms.property_key_kind(atom)? != PropertyKeyKind::Private {
                    atoms.push(atom);
                }
            }
            atoms
        };
        atoms
            .into_iter()
            .map(|atom| PropertyKey::from_borrowed_atom(self.clone(), atom).map_err(Into::into))
            .collect::<Result<Vec<_>, RuntimeError>>()
            .map(Some)
    }

    /// Implement the Module Namespace `[[DefineOwnProperty]]` compatibility
    /// rule for a live export property. `None` delegates to the ordinary path
    /// for non-namespace objects, missing keys, and `@@toStringTag`.
    pub(crate) fn define_module_namespace_export_owned(
        &self,
        object: &ObjectRef,
        key: &PropertyKey,
        descriptor: &crate::engine::object::OwnedPropertyDescriptor,
    ) -> Result<Option<bool>, RuntimeError> {
        self.validate_object_and_key(object, key)?;
        self.0
            .state
            .borrow()
            .define_module_namespace_export_in_state(
                object.object_id(),
                key.atom(),
                &descriptor.raw_record(),
            )
    }

    pub(crate) fn define_module_namespace_export(
        &self,
        object: &ObjectRef,
        key: &PropertyKey,
        descriptor: &OrdinaryPropertyDescriptor,
    ) -> Result<Option<bool>, RuntimeError> {
        self.validate_object_and_key(object, key)?;
        if !self.module_namespace_export_slot(object, key)? {
            return Ok(None);
        }
        // TDZ is observed before admitting public values, which can allocate.
        // The admission cannot call JS; the final borrowed kernel remains the
        // only compatibility algorithm and uses the live cell again.
        self.0
            .state
            .borrow()
            .define_module_namespace_export_in_state(
                object.object_id(),
                key.atom(),
                &PropertyDescriptor::new(),
            )?;
        // Public descriptor values are admitted once at this external adapter.
        // The borrowed State kernel remains the only compatibility algorithm.
        let descriptor =
            crate::engine::object::OwnedPropertyDescriptor::from_public(self, descriptor)?;
        self.define_module_namespace_export_owned(object, key, &descriptor)
    }
}

impl RuntimeState {
    /// Namespace exports are live cells. Read the cell unconditionally before
    /// validating requested attributes; an attribute-only definition still
    /// observes TDZ. Compatibility never modifies or retains any supplied edge.
    pub(crate) fn define_module_namespace_export_in_state(
        &self,
        object: ObjectId,
        atom: Atom,
        descriptor: &PropertyDescriptor<RawValue>,
    ) -> Result<Option<bool>, RuntimeError> {
        if descriptor.is_data_descriptor() && descriptor.is_accessor_descriptor() {
            return Err(
                crate::engine::object::property::PropertyDefinitionError::InvalidDescriptor.into(),
            );
        }
        let object = self.heap.object(object)?;
        if object.kind != ObjectKind::ModuleNamespace {
            return Ok(None);
        }
        let shape = self.heap.shape(object.shape)?;
        let Some(index) = shape.find(AtomIdx::from_raw(atom.raw())) else {
            return Ok(None);
        };
        let Some(PropertySlot::VarRef(id)) = object.slots.get(index as usize) else {
            return Ok(None);
        };
        let current = &self.heap.var_ref(*id)?.value;
        if matches!(current, RawValue::Uninitialized) {
            return Err(RuntimeError::Engine(self.native_atom_error(
                ErrorKind::Reference,
                "",
                atom,
                " is not initialized",
            )?));
        }
        Ok(Some(
            !(descriptor.is_accessor_descriptor()
                || descriptor.configurable == Some(true)
                || descriptor.enumerable == Some(false)
                || descriptor.writable == Some(false)
                || descriptor.value.as_ref().is_some_and(|value| {
                    !crate::engine::value::collection_key::same_value(&self.heap, value, current)
                })),
        ))
    }
}

#[cfg(test)]
mod state_tests {
    use super::*;
    use crate::engine::heap::VarRefData;
    use crate::engine::object::shape::PropertyFlags;

    #[test]
    fn namespace_define_uses_live_cell_without_retaining_or_changing_storage() {
        let runtime = Runtime::new();
        let namespace = runtime.new_module_namespace_object().unwrap();
        let value = runtime.new_object(None).unwrap();
        let key = runtime.intern_property_key("exported").unwrap();
        let cell = runtime
            .0
            .state
            .borrow_mut()
            .heap
            .allocate_var_ref(VarRefData::local(RawValue::Object(value.object_id())))
            .unwrap();
        runtime
            .store_property_slot(
                &namespace,
                &key,
                PropertyFlags::data(true, true, false),
                PropertySlot::VarRef(cell),
            )
            .unwrap();
        let before = std::rc::Rc::strong_count(&runtime.0);
        let mut state = runtime.0.state.borrow_mut();
        let shape = state.heap.object(namespace.object_id()).unwrap().shape;
        let count = state.heap.object_strong_count(value.object_id()).unwrap();
        for (descriptor, accepted) in [
            (PropertyDescriptor::new(), true),
            (
                PropertyDescriptor {
                    value: Some(RawValue::Object(value.object_id())),
                    ..PropertyDescriptor::new()
                },
                true,
            ),
            (
                PropertyDescriptor {
                    value: Some(RawValue::Undefined),
                    ..PropertyDescriptor::new()
                },
                false,
            ),
            (
                PropertyDescriptor {
                    writable: Some(false),
                    ..PropertyDescriptor::new()
                },
                false,
            ),
            (
                PropertyDescriptor {
                    get: Some(None),
                    ..PropertyDescriptor::new()
                },
                false,
            ),
        ] {
            assert_eq!(
                state.try_define_own_property_in_state(
                    &runtime.0.poisoned,
                    namespace.object_id(),
                    key.atom(),
                    &descriptor
                ),
                Ok(Some(accepted))
            );
        }
        assert_eq!(
            state.heap.object(namespace.object_id()).unwrap().shape,
            shape
        );
        assert_eq!(state.heap.object_strong_count(value.object_id()), Ok(count));
        assert_eq!(std::rc::Rc::strong_count(&runtime.0), before);
        assert!(!runtime.0.deferred_references.has_pending());
        let cleanup = state.heap.release_var_ref(cell).unwrap();
        state.apply_cleanup(cleanup).unwrap();
    }

    #[test]
    fn namespace_attribute_only_definition_observes_uninitialized_live_cell() {
        let runtime = Runtime::new();
        let namespace = runtime.new_module_namespace_object().unwrap();
        let key = runtime.intern_property_key("early").unwrap();
        let cell = runtime
            .0
            .state
            .borrow_mut()
            .heap
            .allocate_var_ref(VarRefData::local(RawValue::Uninitialized))
            .unwrap();
        runtime
            .store_property_slot(
                &namespace,
                &key,
                PropertyFlags::data(true, true, false),
                PropertySlot::VarRef(cell),
            )
            .unwrap();
        let mut state = runtime.0.state.borrow_mut();
        for descriptor in [
            PropertyDescriptor::new(),
            PropertyDescriptor {
                configurable: Some(true),
                ..PropertyDescriptor::new()
            },
        ] {
            let error = state
                .try_define_own_property_in_state(
                    &runtime.0.poisoned,
                    namespace.object_id(),
                    key.atom(),
                    &descriptor,
                )
                .unwrap_err();
            assert!(
                matches!(error, RuntimeError::Engine(error) if error.kind() == ErrorKind::Reference)
            );
        }
        assert!(!runtime.is_poisoned());
        let cleanup = state.heap.release_var_ref(cell).unwrap();
        state.apply_cleanup(cleanup).unwrap();
    }
}
