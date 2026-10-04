//! ECMAScript Module Namespace Exotic Object storage helpers.
//!
//! QuickJS keeps namespace live bindings in ordinary `JS_PROP_VARREF` slots
//! and selects the exceptional write/define/own-key behavior through the
//! `JS_CLASS_MODULE_NS` marker.  Oxide uses the same split: the arena already
//! traces `PropertySlot::VarRef`, so this module owns only class-specific
//! construction and dispatch helpers.

use crate::engine::api::runtime::Runtime;
use crate::engine::api::runtime_error::RuntimeError;
use crate::engine::atom::PropertyKeyKind;

use crate::engine::heap::{ObjectData, ObjectKind};
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
    pub(crate) fn define_module_namespace_export(
        &self,
        object: &ObjectRef,
        key: &PropertyKey,
        descriptor: &OrdinaryPropertyDescriptor,
    ) -> Result<Option<bool>, RuntimeError> {
        if !self.is_module_namespace_object(object)? {
            return Ok(None);
        }
        if !key.belongs_to(self) {
            return Err(RuntimeError::WrongRuntime("property key"));
        }
        let _unwind = self.unwind_guard();
        self.0
            .state
            .borrow_mut()
            .define_module_namespace_export_public(
                &self.0.poisoned,
                object.object_id(),
                key.atom(),
                descriptor,
            )
    }
}
