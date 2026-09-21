use crate::engine::api::runtime::Runtime;
use crate::engine::api::runtime_error::RuntimeError;
use crate::engine::atom::AtomIdx;

use crate::engine::code::function::metadata::{ClosureVariable, ClosureVariableKind};
use crate::engine::heap::{HeapError, RawValue, VarRefData, VarRefId};
use crate::engine::object::{ObjectRef, SymbolRef};
use crate::engine::value::{JsValue, Value};
use crate::engine::vm::bindings::closure_view_matches_cell;

impl Runtime {
    /// Store an internal value into a fresh captured cell, consuming the
    /// value's edges directly. A rejected allocation releases the input.
    pub(crate) fn new_var_ref(
        &self,
        value: JsValue,
        is_lexical: bool,
        is_const: bool,
        kind: ClosureVariableKind,
    ) -> Result<VarRefRoot, RuntimeError> {
        let _operation = self.operation();
        let data = VarRefData::captured(value.into_raw(), is_lexical, is_const, kind);
        let allocation = self.0.state.borrow_mut().heap.allocate_var_ref_owned(data);
        match allocation {
            Ok(id) => Ok(VarRefRoot::from_owned_handle(self.clone(), id)),
            Err((error, data)) => {
                self.release_jsvalue(
                    JsValue::from_raw(data.value).expect("captured internal value"),
                )?;
                Err(error.into())
            }
        }
    }

    /// Public-root boundary form of [`Runtime::new_var_ref`]: converts the
    /// root into an internal value (allocating string/BigInt nodes) and
    /// consumes it.
    pub(crate) fn new_var_ref_rooted(
        &self,
        value: Value,
        is_lexical: bool,
        is_const: bool,
        kind: ClosureVariableKind,
    ) -> Result<VarRefRoot, RuntimeError> {
        self.validate_value_domain(&value, "captured variable")?;
        let value = self.into_jsvalue(value)?;
        self.new_var_ref(value, is_lexical, is_const, kind)
    }

    pub(crate) fn new_uninitialized_var_ref(&self) -> Result<VarRefRoot, RuntimeError> {
        let _operation = self.operation();
        let id = self
            .0
            .state
            .borrow_mut()
            .heap
            .allocate_var_ref(VarRefData::captured(
                RawValue::Uninitialized,
                false,
                false,
                ClosureVariableKind::Normal,
            ))?;
        Ok(VarRefRoot::from_owned_handle(self.clone(), id))
    }

    pub(crate) fn new_uninitialized_captured_var_ref(
        &self,
        is_lexical: bool,
        is_const: bool,
        kind: ClosureVariableKind,
    ) -> Result<VarRefRoot, RuntimeError> {
        let _operation = self.operation();
        let id = self
            .0
            .state
            .borrow_mut()
            .heap
            .allocate_var_ref(VarRefData::captured(
                RawValue::Uninitialized,
                is_lexical,
                is_const,
                kind,
            ))?;
        Ok(VarRefRoot::from_owned_handle(self.clone(), id))
    }

    pub(crate) fn set_var_ref_metadata(
        &self,
        root: &impl crate::engine::heap::roots::VarRefHandle,
        is_lexical: bool,
        is_const: bool,
        kind: ClosureVariableKind,
    ) -> Result<(), RuntimeError> {
        if !root.belongs_to(self) {
            return Err(RuntimeError::WrongRuntime("closure variable"));
        }
        self.0.state.borrow_mut().heap.set_var_ref_metadata(
            root.id(),
            is_lexical,
            is_const,
            kind,
        )?;
        Ok(())
    }

    pub(crate) fn reset_var_ref_uninitialized(
        &self,
        root: &impl crate::engine::heap::roots::VarRefHandle,
    ) -> Result<(), RuntimeError> {
        if !root.belongs_to(self) {
            return Err(RuntimeError::WrongRuntime("closure variable"));
        }
        let mut state = self.0.state.borrow_mut();
        let cleanup = state
            .heap
            .replace_var_ref_value(root.id(), RawValue::Uninitialized)?;
        state.apply_cleanup(cleanup)
    }

    /// Read a captured cell as an owned internal value, duplicating every
    /// heap edge the cell carries.
    pub(crate) fn read_var_ref(
        &self,
        root: &impl crate::engine::heap::roots::VarRefHandle,
    ) -> Result<JsValue, RuntimeError> {
        let _operation = self.operation();
        if !root.belongs_to(self) {
            return Err(RuntimeError::WrongRuntime("closure variable"));
        }
        let raw = {
            let state = self.0.state.borrow();
            let var_ref = state.heap.var_ref(root.id())?;
            if var_ref.kind.is_private() {
                return Err(RuntimeError::Invariant(
                    "ordinary VarRef read reached a private-element binding",
                ));
            }
            var_ref.value.clone()
        };
        let mut state = self.0.state.borrow_mut();
        state.retain_raw_root(raw.clone())?;
        JsValue::from_raw(raw).ok_or(RuntimeError::Invariant(
            "internal value sentinel occupied a captured variable cell",
        ))
    }

    /// Public-root boundary form of [`Runtime::read_var_ref`].
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn read_var_ref_rooted(
        &self,
        root: &impl crate::engine::heap::roots::VarRefHandle,
    ) -> Result<Value, RuntimeError> {
        let value = self.read_var_ref(root)?;
        let rooted = self.root_value(&value);
        self.release_jsvalue(value)?;
        rooted
    }

    /// Root a fresh non-immediate cell value without entering an operation or
    /// draining either release queue. A run-window caller must use its normal
    /// boundary when this guarded read declines.
    pub(crate) fn try_read_owned_var_ref(
        &self,
        root: &impl crate::engine::heap::roots::VarRefHandle,
    ) -> Result<Option<JsValue>, RuntimeError> {
        if !root.belongs_to(self) || self.0.deferred_references.has_pending() {
            return Ok(None);
        }
        let Ok(mut state) = self.0.state.try_borrow_mut() else {
            return Ok(None);
        };
        if !state.heap.zero_queue.is_empty() {
            return Ok(None);
        }
        let Ok(cell) = state.heap.var_ref(root.id()) else {
            return Ok(None);
        };
        if cell.kind.is_private()
            || !matches!(
                cell.value,
                RawValue::Object(_)
                    | RawValue::Symbol(_)
                    | RawValue::String(_)
                    | RawValue::BigInt(_)
            )
        {
            return Ok(None);
        }
        let raw = cell.value.clone();
        // The cell keeps its own edge; this read retains one new edge for
        // every heap-backed kind before handing out the owned value.
        state.retain_raw_root(raw.clone())?;
        drop(state);
        Ok(Some(JsValue::from_raw(raw).ok_or(
            RuntimeError::Invariant("internal value sentinel occupied a captured cell"),
        )?))
    }

    /// Trusted shared-borrow read of a proven live captured cell.
    ///
    /// Handles the object, string, BigInt and symbol cases without a mutable
    /// state borrow: node payloads clone their inner `Rc`, objects take the
    /// trusted retain fast path, and symbols retain through the atom table's
    /// shared-borrow `Cell` counter.  A declined read claims no owner and
    /// leaves the cell unchanged.
    #[inline]
    pub(crate) fn read_owned_cell_fast(
        &self,
        root: &impl crate::engine::heap::roots::VarRefHandle,
    ) -> Option<JsValue> {
        if !root.belongs_to(self) || self.0.deferred_references.has_pending() {
            return None;
        }
        let state = self.0.state.try_borrow().ok()?;
        if !state.heap.zero_queue.is_empty() {
            return None;
        }
        let cell = state.heap.var_ref_fast(root.id());
        if cell.kind.is_private() {
            return None;
        }
        match &cell.value {
            RawValue::Object(object) => {
                state.heap.retain_object_fast(*object);
                Some(JsValue::Object(*object))
            }
            RawValue::String(id) => {
                state.heap.retain_string_shared(*id).ok()?;
                Some(JsValue::String(*id))
            }
            RawValue::BigInt(id) => {
                state.heap.retain_bigint_shared(*id).ok()?;
                Some(JsValue::BigInt(*id))
            }
            RawValue::Symbol(index) => {
                state.atoms.retain_index_shared(*index).ok()?;
                Some(JsValue::Symbol(*index))
            }
            RawValue::Undefined
            | RawValue::Null
            | RawValue::Bool(_)
            | RawValue::Int(_)
            | RawValue::Float(_)
            | RawValue::Private(_)
            | RawValue::Uninitialized
            | RawValue::Exception => None,
        }
    }

    /// Guarded global own-data read for an unresolved, non-lexical binding.
    /// No lookup fact escapes this borrow, and autoinit/accessor/prototype
    /// cases retain the normal environment driver. As with owned cell reads,
    /// pending cleanup declines before any retain or public owner is created.
    pub(crate) fn try_read_unresolved_global(
        &self,
        root: &impl VarRefHandle,
        realm: crate::engine::heap::ContextId,
        atom: crate::engine::atom::Atom,
    ) -> Result<Option<JsValue>, RuntimeError> {
        use crate::engine::heap::{ObjectKind, ObjectPayload, PropertySlot};
        use crate::engine::object::shape::PropertyStorageKind;
        if !root.belongs_to(self) || self.0.deferred_references.has_pending() {
            return Ok(None);
        }
        let Ok(mut state) = self.0.state.try_borrow_mut() else {
            return Ok(None);
        };
        if !state.heap.zero_queue.is_empty() {
            return Ok(None);
        }
        let cell = state.heap.var_ref(root.id())?;
        if cell.is_lexical
            || cell.kind.is_private()
            || !matches!(cell.value, RawValue::Uninitialized)
        {
            return Ok(None);
        }
        let global = state.heap.context(realm)?.global_object;
        let object = state.heap.object(global)?;
        if !matches!(
            (object.kind, &object.payload),
            (ObjectKind::GlobalObject, ObjectPayload::GlobalObject { .. })
        ) {
            return Ok(None);
        }
        let shape = state.heap.shape(object.shape)?;
        let revision = shape.layout_revision();
        let cached = cell.global_location.get().filter(|entry| {
            entry.realm == realm
                && entry.atom == atom
                && entry.shape == object.shape
                && entry.revision == revision
                && revision != u64::MAX
        });
        let index = if let Some(entry) = cached {
            entry.index
        } else {
            let Some(index) = shape.find(AtomIdx::from_raw(atom.raw())) else {
                cell.global_location.set(None);
                return Ok(None);
            };
            cell.global_location
                .set(Some(super::binding_records::GlobalLocation {
                    realm,
                    atom,
                    shape: object.shape,
                    revision,
                    index,
                }));
            index
        };
        let index = index as usize;
        if shape.entries()[index].flags.storage != PropertyStorageKind::Data {
            return Ok(None);
        }
        let raw = match object.slots.get(index) {
            Some(PropertySlot::Data(raw)) => raw,
            Some(PropertySlot::VarRef(id)) => &state.heap.var_ref(*id)?.value,
            _ => return Ok(None),
        };
        if !matches!(
            raw,
            RawValue::Undefined
                | RawValue::Null
                | RawValue::Bool(_)
                | RawValue::Int(_)
                | RawValue::Float(_)
                | RawValue::String(_)
                | RawValue::Object(_)
                | RawValue::Symbol(_)
                | RawValue::BigInt(_)
        ) {
            return Ok(None);
        }
        let raw = raw.clone();
        state.retain_raw_root(raw.clone())?;
        drop(state);
        Ok(Some(JsValue::from_raw(raw).ok_or(
            RuntimeError::Invariant("internal value sentinel occupied a global binding cell"),
        )?))
    }

    pub(crate) fn raw_var_ref_value(
        &self,
        root: &impl crate::engine::heap::roots::VarRefHandle,
    ) -> Result<RawValue, RuntimeError> {
        let _operation = self.operation();
        if !root.belongs_to(self) {
            return Err(RuntimeError::WrongRuntime("closure variable"));
        }
        Ok(self.0.state.borrow().heap.var_ref(root.id())?.value.clone())
    }

    pub(crate) fn validate_var_ref_metadata(
        &self,
        root: &impl crate::engine::heap::roots::VarRefHandle,
        descriptor: ClosureVariable,
    ) -> Result<(), RuntimeError> {
        let _operation = self.operation();
        if !root.belongs_to(self) {
            return Err(RuntimeError::WrongRuntime("closure variable"));
        }
        let var_ref = self.0.state.borrow();
        let var_ref = var_ref.heap.var_ref(root.id())?;
        if !closure_view_matches_cell(
            (var_ref.is_lexical, var_ref.is_const, var_ref.kind),
            descriptor,
        ) {
            return Err(RuntimeError::Invariant(
                "closure descriptor metadata does not match the shared variable cell",
            ));
        }
        Ok(())
    }

    /// Replace a captured value by moving its edge into the cell. Validation
    /// failure releases the input; success releases the previous cell edge.
    pub(crate) fn write_var_ref(
        &self,
        root: &impl crate::engine::heap::roots::VarRefHandle,
        value: JsValue,
    ) -> Result<(), RuntimeError> {
        let _operation = self.operation();
        let validation = (|| {
            if !root.belongs_to(self) {
                return Err(RuntimeError::WrongRuntime("closure variable"));
            }
            if self
                .0
                .state
                .borrow()
                .heap
                .var_ref(root.id())?
                .kind
                .is_private()
            {
                return Err(RuntimeError::Invariant(
                    "ordinary VarRef write reached a private-element binding",
                ));
            }
            Ok(())
        })();
        if let Err(error) = validation {
            self.release_jsvalue(value)?;
            return Err(error);
        }
        let result = self
            .0
            .state
            .borrow_mut()
            .heap
            .replace_var_ref_value_owned(root.id(), value.into_raw());
        match result {
            Ok(previous) => {
                if let Some(previous) = JsValue::from_raw(previous) {
                    self.release_jsvalue(previous)?;
                }
                Ok(())
            }
            Err((error, rejected)) => {
                self.release_jsvalue(JsValue::from_raw(rejected).expect("internal replacement"))?;
                Err(error.into())
            }
        }
    }

    /// Public-root boundary form of [`Runtime::write_var_ref`].
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn write_var_ref_rooted(
        &self,
        root: &impl crate::engine::heap::roots::VarRefHandle,
        value: Value,
    ) -> Result<(), RuntimeError> {
        self.validate_value_domain(&value, "captured variable")?;
        let value = self.into_jsvalue(value)?;
        self.write_var_ref(root, value)
    }

    pub(crate) fn take_owned_raw_value(&self, value: RawValue) -> Result<Value, RuntimeError> {
        // The caller consumed one owned root edge for this value.  Object and
        // Symbol edges transfer into the public root wrappers below; a string
        // or BigInt node edge does not (the public value owns an `Rc` payload
        // clone), so it is released once the payload has been read out.
        let node_edge = value.conversion_node_edge();
        let state = self.0.state.borrow();
        let converted = Ok(match value {
            RawValue::Undefined => Value::Undefined,
            RawValue::Null => Value::Null,
            RawValue::Bool(value) => Value::Bool(value),
            RawValue::Int(value) => Value::Int(value),
            RawValue::Float(value) => Value::Float(value),
            RawValue::BigInt(id) => Value::BigInt(state.heap.bigint(id)?.clone()),
            RawValue::String(id) => Value::String(state.heap.string(id)?.clone()),
            RawValue::Symbol(index) => {
                let atom = state.atoms.brand(index)?;
                Value::Symbol(SymbolRef::from_owned_atom(self.clone(), atom))
            }
            RawValue::Private(_) => {
                return Err(RuntimeError::Invariant(
                    "private-name identity occupied a public runtime root",
                ));
            }
            RawValue::Object(object) => {
                Value::Object(ObjectRef::from_owned_handle(self.clone(), object))
            }
            RawValue::Uninitialized | RawValue::Exception => {
                return Err(RuntimeError::Invariant(
                    "internal value sentinel occupied the pending exception slot",
                ));
            }
        });
        drop(state);
        if let Some(edge) = node_edge {
            self.release_converted_node_edge(edge);
        }
        converted
    }

    pub(crate) fn root_raw_value(&self, value: RawValue) -> Result<Value, RuntimeError> {
        let state = self.0.state.borrow();
        Ok(match value {
            RawValue::Undefined => Value::Undefined,
            RawValue::Null => Value::Null,
            RawValue::Bool(value) => Value::Bool(value),
            RawValue::Int(value) => Value::Int(value),
            RawValue::Float(value) => Value::Float(value),
            RawValue::BigInt(id) => Value::BigInt(state.heap.bigint(id)?.clone()),
            RawValue::String(id) => Value::String(state.heap.string(id)?.clone()),
            RawValue::Symbol(index) => {
                let atom = state.atoms.brand(index)?;
                Value::Symbol(SymbolRef::from_borrowed_atom(self.clone(), atom)?)
            }
            RawValue::Private(_) => {
                return Err(RuntimeError::Invariant(
                    "private-name identity escaped into an ECMAScript Value",
                ));
            }
            RawValue::Object(object) => {
                Value::Object(ObjectRef::from_borrowed_handle(self.clone(), object)?)
            }
            RawValue::Uninitialized | RawValue::Exception => {
                return Err(RuntimeError::Invariant(
                    "internal value sentinel escaped from an object property",
                ));
            }
        })
    }
}

pub(crate) struct VarRefRoot {
    pub(crate) runtime: Runtime,
    pub(crate) id: VarRefId,
}

impl VarRefRoot {
    pub(crate) fn from_owned_handle(runtime: Runtime, id: VarRefId) -> Self {
        Self { runtime, id }
    }

    pub(crate) fn from_borrowed_handle(runtime: Runtime, id: VarRefId) -> Result<Self, HeapError> {
        runtime.retain_var_ref_handle(id)?;
        Ok(Self { runtime, id })
    }

    pub(crate) const fn id(&self) -> VarRefId {
        self.id
    }

    pub(crate) fn belongs_to(&self, runtime: &Runtime) -> bool {
        self.runtime.is_same_runtime(runtime)
    }
}

impl Clone for VarRefRoot {
    fn clone(&self) -> Self {
        self.runtime
            .retain_var_ref_handle(self.id)
            .expect("a live VarRef root must retain its cell");
        Self {
            runtime: self.runtime.clone(),
            id: self.id,
        }
    }
}

impl Drop for VarRefRoot {
    fn drop(&mut self) {
        self.runtime.release_var_ref_handle(self.id);
    }
}

#[cfg(test)]
mod owned_cell_tests {
    use super::*;
    use crate::engine::heap::RawId;

    #[test]
    fn unresolved_global_leaf_observes_replacement_and_declines_accessors_and_tdz() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        drop(
            context
                .eval("globalThis.nativeLeaf = { value: 1 }")
                .unwrap(),
        );
        let atom = runtime
            .0
            .state
            .borrow_mut()
            .atoms
            .intern("nativeLeaf")
            .unwrap();
        let root = runtime.new_uninitialized_var_ref().unwrap();
        let first = runtime
            .try_read_unresolved_global(&root, context.realm, atom)
            .unwrap()
            .unwrap();
        assert!(matches!(&first, JsValue::Object(_)));
        runtime.release_jsvalue(first).unwrap();
        drop(context.eval("nativeLeaf = 7").unwrap());
        assert_eq!(
            runtime
                .try_read_unresolved_global(&root, context.realm, atom)
                .unwrap(),
            Some(JsValue::Int(7))
        );
        drop(context.eval("Object.defineProperty(globalThis, 'nativeLeaf', { get() { throw 99; }, configurable: true })").unwrap());
        assert!(
            runtime
                .try_read_unresolved_global(&root, context.realm, atom)
                .unwrap()
                .is_none()
        );
        drop(context.eval("delete globalThis.nativeLeaf").unwrap());
        assert!(
            runtime
                .try_read_unresolved_global(&root, context.realm, atom)
                .unwrap()
                .is_none()
        );
        let lexical = runtime
            .new_uninitialized_captured_var_ref(true, false, ClosureVariableKind::Normal)
            .unwrap();
        assert!(
            runtime
                .try_read_unresolved_global(&lexical, context.realm, atom)
                .unwrap()
                .is_none()
        );
        let foreign = Runtime::new();
        assert!(
            foreign
                .try_read_unresolved_global(&root, context.realm, atom)
                .unwrap()
                .is_none()
        );
        runtime.0.state.borrow_mut().atoms.release(atom).unwrap();
    }

    #[test]
    fn owned_cell_read_retains_one_owner_and_survives_replacement() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        for source in ["({})", "Symbol('cell')", "'cell'", "123456789012345678901n"] {
            let value = context.eval(source).unwrap();
            let root = runtime
                .new_var_ref_rooted(value.clone(), false, false, ClosureVariableKind::Normal)
                .unwrap();
            let object_id = match &value {
                Value::Object(object) => Some(object.object_id()),
                _ => None,
            };
            let before = object_id.map(|id| {
                runtime
                    .0
                    .state
                    .borrow()
                    .heap
                    .object_strong_count(id)
                    .unwrap()
            });
            let copied = runtime.try_read_owned_var_ref(&root).unwrap().unwrap();
            assert_eq!(runtime.root_value(&copied).unwrap(), value);
            if let (Some(id), Some(before)) = (object_id, before) {
                assert_eq!(
                    runtime
                        .0
                        .state
                        .borrow()
                        .heap
                        .object_strong_count(id)
                        .unwrap(),
                    before + 1
                );
            }
            drop(value);
            runtime.write_var_ref(&root, JsValue::Int(1)).unwrap();
            runtime.run_gc().unwrap();
            if let Some(id) = object_id {
                assert!(runtime.0.state.borrow().heap.object(id).is_ok());
            }
            runtime.release_jsvalue(copied).unwrap();
            runtime.run_gc().unwrap();
            if let Some(id) = object_id {
                assert!(runtime.0.state.borrow().heap.object(id).is_err());
            }
        }
    }

    #[test]
    fn owned_cell_read_declines_without_draining_or_using_stale_values() {
        let runtime = Runtime::new();
        let foreign = Runtime::new();
        let root = runtime
            .new_var_ref_rooted(
                Value::Object(runtime.new_object(None).unwrap()),
                false,
                false,
                ClosureVariableKind::Normal,
            )
            .unwrap();
        assert!(foreign.try_read_owned_var_ref(&root).unwrap().is_none());
        {
            let _state = runtime.0.state.borrow();
            assert!(runtime.try_read_owned_var_ref(&root).unwrap().is_none());
        }
        let released = runtime.new_object(None).unwrap();
        {
            let _state = runtime.0.state.borrow();
            drop(released);
        }
        assert!(runtime.0.deferred_references.has_pending());
        assert!(runtime.try_read_owned_var_ref(&root).unwrap().is_none());
        assert!(runtime.0.deferred_references.has_pending());
        runtime.drain_deferred_references().unwrap();
        let queued = runtime.new_object(None).unwrap();
        let queued_id = queued.object_id();
        runtime
            .0
            .state
            .borrow_mut()
            .heap
            .retain_object(queued_id)
            .unwrap();
        drop(queued);
        runtime
            .0
            .state
            .borrow_mut()
            .heap
            .release_raw_no_drain(RawId::Object(queued_id))
            .unwrap();
        assert!(runtime.try_read_owned_var_ref(&root).unwrap().is_none());
        {
            let mut state = runtime.0.state.borrow_mut();
            assert_eq!(state.heap.zero_queue.len(), 1);
            let cleanup = state.heap.drain_zero_queue().unwrap();
            state.apply_cleanup(cleanup).unwrap();
        }
        let read = runtime.try_read_owned_var_ref(&root).unwrap();
        assert!(read.is_some());
        if let Some(value) = read {
            runtime.release_jsvalue(value).unwrap();
        }
        runtime.reset_var_ref_uninitialized(&root).unwrap();
        assert!(runtime.try_read_owned_var_ref(&root).unwrap().is_none());
    }

    #[test]
    fn owned_cell_read_retain_failure_is_atomic() {
        let runtime = Runtime::new();
        let value = runtime.new_object(None).unwrap();
        let id = value.object_id();
        let root = runtime
            .new_var_ref_rooted(
                Value::Object(value),
                false,
                false,
                ClosureVariableKind::Normal,
            )
            .unwrap();
        let before = runtime
            .0
            .state
            .borrow()
            .heap
            .object_strong_count(id)
            .unwrap();
        runtime
            .0
            .state
            .borrow_mut()
            .heap
            .live_node_mut(RawId::Object(id))
            .unwrap()
            .strong
            .set(u32::MAX);
        let result = runtime.try_read_owned_var_ref(&root);
        let after = runtime
            .0
            .state
            .borrow()
            .heap
            .object_strong_count(id)
            .unwrap();
        // Restore fixture ownership before assertions or unwinding drop roots.
        runtime
            .0
            .state
            .borrow_mut()
            .heap
            .live_node_mut(RawId::Object(id))
            .unwrap()
            .strong
            .set(before);
        assert!(result.is_err());
        assert_eq!(after, u32::MAX);
        assert!(matches!(
            runtime.raw_var_ref_value(&root).unwrap(),
            RawValue::Object(object) if object == id
        ));
    }
}

mod sealed {
    pub trait CellOwner {}
}
/// A cell reference cannot outlive either its independent root or its callee.
pub(crate) trait VarRefHandle: sealed::CellOwner {
    fn id(&self) -> VarRefId;
    fn runtime(&self) -> &Runtime;
    fn belongs_to(&self, runtime: &Runtime) -> bool {
        self.runtime().is_same_runtime(runtime)
    }
    fn to_root(&self) -> VarRefRoot {
        VarRefRoot::from_borrowed_handle(self.runtime().clone(), self.id())
            .expect("a live cell owner must retain its cell")
    }
}
impl sealed::CellOwner for VarRefRoot {}
impl VarRefHandle for VarRefRoot {
    fn id(&self) -> VarRefId {
        self.id
    }
    fn runtime(&self) -> &Runtime {
        &self.runtime
    }
}

pub(crate) struct VarRefView<'a> {
    runtime: &'a Runtime,
    id: VarRefId,
}
impl<'a> VarRefView<'a> {
    // No constructor accepts an arbitrary raw ID. The immutable environment
    // carries either the authentic callee or an independently rooted cell.
    pub(crate) fn from_closure(
        slots: &'a crate::engine::vm::closure::ClosureSlots,
        index: usize,
    ) -> Option<Self> {
        slots
            .borrowed_cell(index)
            .map(|(runtime, id)| Self { runtime, id })
    }
    pub(crate) fn id(&self) -> VarRefId {
        self.id
    }
    pub(crate) fn belongs_to(&self, runtime: &Runtime) -> bool {
        VarRefHandle::belongs_to(self, runtime)
    }
    pub(crate) fn clone(&self) -> VarRefRoot {
        self.to_root()
    }
}
impl sealed::CellOwner for VarRefView<'_> {}
impl VarRefHandle for VarRefView<'_> {
    fn id(&self) -> VarRefId {
        self.id
    }
    fn runtime(&self) -> &Runtime {
        self.runtime
    }
}
impl<T: VarRefHandle + ?Sized> sealed::CellOwner for &T {}
impl<T: VarRefHandle + ?Sized> VarRefHandle for &T {
    fn id(&self) -> VarRefId {
        (**self).id()
    }
    fn runtime(&self) -> &Runtime {
        (**self).runtime()
    }
}
impl<T: VarRefHandle + ?Sized> sealed::CellOwner for &mut T {}
impl<T: VarRefHandle + ?Sized> VarRefHandle for &mut T {
    fn id(&self) -> VarRefId {
        (**self).id()
    }
    fn runtime(&self) -> &Runtime {
        (**self).runtime()
    }
}
