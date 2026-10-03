use crate::engine::api::runtime::Runtime;
use crate::engine::api::runtime_error::RuntimeError;

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
    #[cfg(test)]
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
            RawValue::ShortBigInt(value) => {
                Value::BigInt(crate::engine::value::bigint::JsBigInt::from(value))
            }
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
            RawValue::ShortBigInt(value) => {
                Value::BigInt(crate::engine::value::bigint::JsBigInt::from(value))
            }
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
    owns_edge: bool,
}

impl VarRefRoot {
    pub(crate) fn from_owned_handle(runtime: Runtime, id: VarRefId) -> Self {
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_runtime_event(
            "runtime.var_ref_root.adopt",
            "core.var_ref_root.adopt",
        );
        Self {
            runtime,
            id,
            owns_edge: true,
        }
    }

    pub(crate) fn from_borrowed_handle(runtime: Runtime, id: VarRefId) -> Result<Self, HeapError> {
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_runtime_event(
            "runtime.var_ref_root.promote",
            "core.var_ref_root.promote",
        );
        runtime.retain_var_ref_handle(id)?;
        Ok(Self {
            runtime,
            id,
            owns_edge: true,
        })
    }

    pub(crate) const fn id(&self) -> VarRefId {
        self.id
    }

    pub(crate) fn into_execution_handle(mut self) -> VarRefId {
        self.owns_edge = false;
        self.id
    }

    pub(crate) fn belongs_to(&self, runtime: &Runtime) -> bool {
        self.runtime.is_same_runtime(runtime)
    }
}

impl VarRefRoot {
    pub(crate) fn try_clone(&self) -> Result<Self, RuntimeError> {
        self.runtime.check_poison()?;
        self.runtime.retain_var_ref_handle(self.id)?;
        Ok(Self {
            runtime: self.runtime.clone(),
            id: self.id,
            owns_edge: true,
        })
    }
}

impl Drop for VarRefRoot {
    fn drop(&mut self) {
        if self.owns_edge {
            self.runtime.release_var_ref_handle(self.id);
        }
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
    fn to_root(&self) -> Result<VarRefRoot, RuntimeError> {
        self.runtime().check_poison()?;
        Ok(VarRefRoot::from_borrowed_handle(
            self.runtime().clone(),
            self.id(),
        )?)
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
        runtime: &'a Runtime,
        index: usize,
    ) -> Option<Self> {
        slots
            .borrowed_cell(runtime, index)
            .map(|(runtime, id)| Self { runtime, id })
    }
    /// Borrow a cell owned by a live frame binding.
    ///
    /// Trust argument: the caller holds the owning frame-storage edge for the
    /// whole view lifetime, so the cell cannot be reclaimed while the view is
    /// used. Frame bindings retain their cell exactly like closure slots do.
    pub(crate) fn from_frame(runtime: &'a Runtime, id: VarRefId) -> Self {
        Self { runtime, id }
    }
    pub(crate) fn id(&self) -> VarRefId {
        self.id
    }
    pub(crate) fn belongs_to(&self, runtime: &Runtime) -> bool {
        VarRefHandle::belongs_to(self, runtime)
    }
    pub(crate) fn try_clone(&self) -> Result<VarRefRoot, RuntimeError> {
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
