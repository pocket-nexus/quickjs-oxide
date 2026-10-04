use crate::engine::api::runtime::{Runtime, RuntimeUnwindGuard};
use crate::engine::api::runtime_error::RuntimeError;
use crate::engine::atom::{Atom, AtomIdx};
use crate::engine::code::function::metadata::ClosureVariableKind;
use crate::engine::heap::ownership::ConvertedValue;
use crate::engine::heap::roots::VarRefRoot;
use crate::engine::heap::runtime::RuntimeState;
use crate::engine::heap::runtime::owned_values::OwnedValueGuard;

use crate::engine::heap::{ObjectId, ObjectPayload, PropertySlot, RawValue, ShapeId, VarRefId};
use crate::engine::object::property::CompletePropertyDescriptor;
use crate::engine::object::shape::{PropertyFlags, ShapeEntry};
use crate::engine::object::{
    AccessorValue, CompleteOrdinaryPropertyDescriptor, DescriptorField, ObjectRef,
    OrdinaryPropertyDescriptor, PropertyKey, properties,
};
use crate::engine::value::{JsValue, Value};
use std::cell::Cell;

/// One-use missing selection minted only by the storage transaction below.
/// It never leaves that exclusive RuntimeState borrow or enters a VM/JS state.
/// Independent heap append callers cannot construct this capability.
pub(crate) struct SelectedMissingAppend {
    object: ObjectId,
    shape: ShapeId,
    atom: Atom,
    slot_count: usize,
}
impl SelectedMissingAppend {
    pub(crate) fn object(&self) -> ObjectId {
        self.object
    }
    pub(crate) fn atom(&self) -> Atom {
        self.atom
    }
    pub(crate) fn into_parts(self) -> (ObjectId, ShapeId, Atom, usize) {
        (self.object, self.shape, self.atom, self.slot_count)
    }
}

/// A storage input either borrows a descriptor payload or transfers an existing
/// execution owner. It is consumed in the same state borrow as slot selection;
/// it never enters a continuation or stores a Runtime.
#[must_use]
pub(crate) enum SlotAppendInput<'a> {
    Borrowed(PropertySlot),
    Owned(&'a mut JsValue),
}

impl<'a> SlotAppendInput<'a> {
    pub(crate) fn into_parts(self) -> (PropertySlot, Option<&'a mut JsValue>) {
        match self {
            Self::Borrowed(slot) => (slot, None),
            Self::Owned(owner) => (PropertySlot::Data(owner.as_raw()), Some(owner)),
        }
    }
}

impl Runtime {
    pub(crate) fn validate_object_and_key(
        &self,
        object: &ObjectRef,
        key: &PropertyKey,
    ) -> Result<(), RuntimeError> {
        if !object.belongs_to(self) {
            return Err(RuntimeError::WrongRuntime("object"));
        }
        if !key.belongs_to(self) {
            return Err(RuntimeError::WrongRuntime("property key"));
        }
        Ok(())
    }

    pub(crate) fn validate_descriptor_domains(
        &self,
        descriptor: &OrdinaryPropertyDescriptor,
    ) -> Result<(), RuntimeError> {
        if let DescriptorField::Present(value) = &descriptor.value {
            match value {
                Value::Object(object) if !object.belongs_to(self) => {
                    return Err(RuntimeError::WrongRuntime("descriptor value"));
                }
                Value::Symbol(symbol) if !symbol.belongs_to(self) => {
                    return Err(RuntimeError::WrongRuntime("descriptor value"));
                }
                _ => {}
            }
        }
        for accessor in [&descriptor.get, &descriptor.set] {
            if let DescriptorField::Present(AccessorValue::Callable(callable)) = accessor {
                if !callable.belongs_to(self) {
                    return Err(RuntimeError::WrongRuntime("descriptor accessor"));
                }
            }
        }
        Ok(())
    }

    pub(crate) fn validate_value_domain(
        &self,
        value: &Value,
        role: &'static str,
    ) -> Result<(), RuntimeError> {
        match value {
            Value::Object(object) if !object.belongs_to(self) => {
                Err(RuntimeError::WrongRuntime(role))
            }
            Value::Symbol(symbol) if !symbol.belongs_to(self) => {
                Err(RuntimeError::WrongRuntime(role))
            }
            _ => Ok(()),
        }
    }

    /// Return whether `value` carries QuickJS's identity-local Annex B
    /// `is_HTMLDDA` object bit.
    pub(crate) fn value_is_html_dda(&self, value: &Value) -> Result<bool, RuntimeError> {
        let Value::Object(object) = value else {
            return Ok(false);
        };
        if !object.belongs_to(self) {
            return Err(RuntimeError::WrongRuntime("IsHTMLDDA value"));
        }
        self.value_is_html_dda_jsvalue(&JsValue::Object(object.object_id()))
    }

    pub(crate) fn value_is_html_dda_jsvalue(&self, value: &JsValue) -> Result<bool, RuntimeError> {
        let JsValue::Object(id) = value else {
            return Ok(false);
        };
        Ok(self.0.state.borrow().heap.object(*id)?.is_html_dda)
    }

    pub(crate) fn value_to_boolean_jsvalue(&self, value: &JsValue) -> Result<bool, RuntimeError> {
        match value {
            JsValue::Object(_) | JsValue::String(_) | JsValue::BigInt(_) => {
                self.0.state.borrow().value_to_boolean_jsvalue(value)
            }
            _ => Ok(value.to_boolean_primitive()),
        }
    }

    pub(crate) fn strict_equal_jsvalue(
        &self,
        left: &JsValue,
        right: &JsValue,
    ) -> Result<bool, RuntimeError> {
        let needs_heap = match (left, right) {
            (JsValue::String(left), JsValue::String(right)) => left != right,
            (JsValue::BigInt(left), JsValue::BigInt(right)) => left != right,
            (JsValue::ShortBigInt(_), JsValue::BigInt(_))
            | (JsValue::BigInt(_), JsValue::ShortBigInt(_)) => true,
            _ => false,
        };
        if needs_heap {
            self.0.state.borrow().strict_equal_jsvalue(left, right)
        } else {
            Ok(strict_equal_immediate(left, right))
        }
    }

    /// Mirror `JS_SetIsHTMLDDA` for a runtime-owned object.
    #[cfg(feature = "test262-host")]
    pub(crate) fn set_object_is_html_dda(&self, object: &ObjectRef) -> Result<(), RuntimeError> {
        if !object.belongs_to(self) {
            return Err(RuntimeError::WrongRuntime("IsHTMLDDA object"));
        }
        self.0
            .state
            .borrow_mut()
            .heap
            .set_object_is_html_dda(object.object_id())?;
        Ok(())
    }

    /// Convert a public value into its heap-stored form at a boundary.
    ///
    /// String and heap BigInt payloads allocate one arena node each (API input
    /// conversion is a genuine creation point); short BigInts stay immediate.
    /// The returned guard owns that
    /// one producer-owned node edge and releases it on `Drop` unless the
    /// caller adopts it: clone with [`ConvertedValue::raw`] for a store that
    /// retained its own copy edge, [`ConvertedValue::take`] for a sink that
    /// owns and releases the edge itself, or [`ConvertedValue::disarm`] for a
    /// by-value owner that releases it later.  Object edges and Symbol atoms
    /// are *not* retained here — the historical contract stands:
    /// transactional stores retain object and string/BigInt edges, and
    /// Symbol/Private atoms are retained by the store's atom accounting or
    /// explicitly by transfer points.
    ///
    /// This conversion takes its own state borrow; callers must not hold one.
    pub(crate) fn raw_property_value(
        &self,
        value: &Value,
    ) -> Result<ConvertedValue<'_>, RuntimeError> {
        let raw = match value {
            Value::Undefined => RawValue::Undefined,
            Value::Null => RawValue::Null,
            Value::Bool(value) => RawValue::Bool(*value),
            Value::Int(value) => RawValue::Int(*value),
            Value::Float(value) => RawValue::Float(*value),
            Value::BigInt(value) if value.as_i64().is_some() => {
                RawValue::ShortBigInt(value.as_i64().expect("short BigInt"))
            }
            Value::BigInt(value) => {
                let mut state = self.0.state.borrow_mut();
                let id = state.heap.allocate_bigint(value.clone())?;
                #[cfg(debug_assertions)]
                if std::env::var("QJS_TRACE_BIGINT_ID")
                    .is_ok_and(|value| format!("{id:?}").contains(&format!("index: {value},")))
                {
                    eprintln!(
                        "[raw-b] {id:?}\n{}",
                        std::backtrace::Backtrace::force_capture()
                    );
                }
                RawValue::BigInt(id)
            }
            Value::String(value) => {
                let mut state = self.0.state.borrow_mut();
                let id = state.heap.allocate_string(value.clone())?;
                RawValue::String(id)
            }
            Value::Symbol(symbol) => {
                if !symbol.belongs_to(self) {
                    return Err(RuntimeError::WrongRuntime("property value"));
                }
                let atom = symbol.atom();
                let index = self.0.state.borrow().atoms.unbrand(atom)?;
                RawValue::Symbol(index)
            }
            Value::Object(object) => {
                if !object.belongs_to(self) {
                    return Err(RuntimeError::WrongRuntime("property value"));
                }
                RawValue::Object(object.object_id())
            }
        };
        Ok(ConvertedValue::new(self, raw))
    }

    pub(crate) fn store_complete_property(
        &self,
        object: &ObjectRef,
        key: &PropertyKey,
        complete: CompleteOrdinaryPropertyDescriptor,
    ) -> Result<(), RuntimeError> {
        let converted = match &complete {
            CompleteOrdinaryPropertyDescriptor::Data { value, .. } => {
                Some(self.raw_property_value(value)?)
            }
            CompleteOrdinaryPropertyDescriptor::Accessor { .. } => None,
        };
        let complete = match &complete {
            CompleteOrdinaryPropertyDescriptor::Data {
                writable,
                enumerable,
                configurable,
                ..
            } => CompletePropertyDescriptor::Data {
                value: converted.as_ref().expect("converted data").raw(),
                writable: *writable,
                enumerable: *enumerable,
                configurable: *configurable,
            },
            CompleteOrdinaryPropertyDescriptor::Accessor {
                get,
                set,
                enumerable,
                configurable,
            } => CompletePropertyDescriptor::Accessor {
                get: get
                    .as_ref()
                    .map(|v| RawValue::Object(v.as_object().object_id())),
                set: set
                    .as_ref()
                    .map(|v| RawValue::Object(v.as_object().object_id())),
                enumerable: *enumerable,
                configurable: *configurable,
            },
        };
        self.store_complete_raw_property(object, key, complete)
    }

    pub(crate) fn store_complete_raw_property(
        &self,
        object: &ObjectRef,
        key: &PropertyKey,
        complete: CompletePropertyDescriptor<RawValue>,
    ) -> Result<(), RuntimeError> {
        let mut state = self.0.state.borrow_mut();
        if let ObjectPayload::GlobalObject { uninitialized_vars } =
            state.heap.object(object.object_id())?.payload
        {
            state.store_complete_global_raw_property(
                &self.0.poisoned,
                object.object_id(),
                uninitialized_vars,
                key.atom(),
                complete,
            )
        } else {
            state.store_complete_raw_property(object.object_id(), key.atom(), complete)
        }
    }

    pub(crate) fn own_var_ref_root(
        &self,
        object: &ObjectRef,
        key: &PropertyKey,
    ) -> Result<Option<VarRefRoot>, RuntimeError> {
        self.validate_object_and_key(object, key)?;
        let id = self
            .0
            .state
            .borrow()
            .var_ref_property(object.object_id(), key.atom())?;
        id.map(|id| VarRefRoot::from_borrowed_handle(self.clone(), id).map_err(Into::into))
            .transpose()
    }

    pub(crate) fn store_property_slot(
        &self,
        object: &ObjectRef,
        key: &PropertyKey,
        flags: PropertyFlags,
        replacement: PropertySlot,
    ) -> Result<(), RuntimeError> {
        self.0.state.borrow_mut().store_property_slot(
            object.object_id(),
            key.atom(),
            flags,
            replacement,
        )
    }
}

/// One temporary cell edge used by Global property storage. This borrows
/// current state and never creates a Runtime-backed root or reborrows state.
struct GlobalVarRefGuard<'a> {
    state: &'a mut RuntimeState,
    poisoned: &'a Cell<bool>,
    id: VarRefId,
    owns_edge: bool,
}

impl<'a> GlobalVarRefGuard<'a> {
    fn retain(
        state: &'a mut RuntimeState,
        poisoned: &'a Cell<bool>,
        id: VarRefId,
    ) -> Result<Self, RuntimeError> {
        state.heap.retain_var_ref(id)?;
        Ok(Self::adopt(state, poisoned, id))
    }

    fn adopt(state: &'a mut RuntimeState, poisoned: &'a Cell<bool>, id: VarRefId) -> Self {
        Self {
            state,
            poisoned,
            id,
            owns_edge: true,
        }
    }

    fn parts(&mut self) -> (&mut RuntimeState, VarRefId) {
        (self.state, self.id)
    }

    /// Normal retirement exposes the first cleanup failure to the caller.
    /// A failed retirement is not retried by Drop; quarantine stops the suffix.
    fn retire(mut self) -> Result<(), RuntimeError> {
        self.owns_edge = false;
        let _unwind = RuntimeUnwindGuard::from_flag(self.poisoned);
        self.state
            .release_var_ref_handle(self.id)
            .inspect_err(|_| self.poisoned.set(true))
    }
}

impl Drop for GlobalVarRefGuard<'_> {
    fn drop(&mut self) {
        if !self.owns_edge {
            return;
        }
        if std::thread::panicking() {
            self.poisoned.set(true);
        }
        if self.poisoned.get() {
            return;
        }
        let _unwind = RuntimeUnwindGuard::from_flag(self.poisoned);
        if self.state.release_var_ref_handle(self.id).is_err() {
            self.poisoned.set(true);
        }
    }
}

impl RuntimeState {
    /// Inspect a same-runtime object's slot without acquiring a cell edge.
    /// The object owner keeps the returned cell alive until a checked retain
    /// transfers it into the caller's concrete temporary guard.
    pub(super) fn var_ref_property(
        &self,
        object: ObjectId,
        atom: Atom,
    ) -> Result<Option<crate::engine::heap::VarRefId>, RuntimeError> {
        let state = self;
        let object = state.heap.object(object)?;
        let shape = state.heap.shape(object.shape)?;
        let Some(index) = shape.find(AtomIdx::from_raw(atom.raw())) else {
            return Ok(None);
        };
        Ok(match object.slots.get(index as usize) {
            Some(PropertySlot::VarRef(id)) => Some(*id),
            Some(
                PropertySlot::Data(_) | PropertySlot::Accessor { .. } | PropertySlot::AutoInit(_),
            ) => None,
            None => {
                return Err(RuntimeError::Invariant(
                    "shape property has no parallel object slot",
                ));
            }
        })
    }

    /// Keep the legacy hidden-object and cell temporary edge counts/order
    /// while committing Global descriptors under the caller's state access.
    /// Descriptor edges stay borrowed until checked duplication or slot retain.
    pub(crate) fn store_complete_global_raw_property(
        &mut self,
        poisoned: &Cell<bool>,
        object: ObjectId,
        hidden: ObjectId,
        atom: Atom,
        complete: CompletePropertyDescriptor<RawValue>,
    ) -> Result<(), RuntimeError> {
        // Legacy storage retained this object even when no hidden cell was
        // needed. Preserve that checked retain and its final release order.
        self.heap.retain_object(hidden)?;
        let mut hidden_owner = OwnedValueGuard::new(self, poisoned, JsValue::Object(hidden));
        let (state, _) = hidden_owner.parts();
        let result = match complete {
            CompletePropertyDescriptor::Data {
                value,
                writable,
                enumerable,
                configurable,
            } => {
                let existing = if let Some(id) = state.var_ref_property(object, atom)? {
                    Some((id, false))
                } else {
                    state.var_ref_property(hidden, atom)?.map(|id| (id, true))
                };
                let mut root = if let Some((id, from_hidden)) = existing {
                    let mut root = GlobalVarRefGuard::retain(state, poisoned, id)?;
                    let (state, id) = root.parts();
                    if from_hidden
                        && !state
                            .delete_ordinary_property_with_poison(poisoned, hidden, atom, None)?
                    {
                        return Err(RuntimeError::Invariant(
                            "hidden global VarRef property was not configurable",
                        ));
                    }
                    let value = state.dup_jsvalue(&JsValue::from_raw(value).ok_or(
                        RuntimeError::Invariant("global data held internal sentinel"),
                    )?)?;
                    state.write_var_ref(poisoned, id, value)?;
                    root
                } else {
                    let value = state.dup_jsvalue(&JsValue::from_raw(value).ok_or(
                        RuntimeError::Invariant("global data held internal sentinel"),
                    )?)?;
                    let id = state.new_var_ref(
                        poisoned,
                        value,
                        false,
                        !writable,
                        ClosureVariableKind::Normal,
                    )?;
                    GlobalVarRefGuard::adopt(state, poisoned, id)
                };
                let (state, id) = root.parts();
                state.set_var_ref_metadata(id, false, !writable, ClosureVariableKind::Normal)?;
                state.store_property_slot_with_poison(
                    poisoned,
                    object,
                    atom,
                    PropertyFlags::data(writable, enumerable, configurable),
                    PropertySlot::VarRef(id),
                )?;
                root.retire()
            }
            CompletePropertyDescriptor::Accessor {
                get,
                set,
                enumerable,
                configurable,
            } => {
                if let Some(id) = state.var_ref_property(object, atom)? {
                    let mut root = GlobalVarRefGuard::retain(state, poisoned, id)?;
                    let (state, id) = root.parts();
                    // The temporary checked cell edge participates in the
                    // original >2 threshold; ordinary slot ownership is one.
                    if state.heap.var_ref_strong_count(id)? > 2 {
                        if let Some(hidden_id) = state.var_ref_property(hidden, atom)? {
                            // The probe retained and dropped its own temporary
                            // before producing the legacy mismatch error.
                            GlobalVarRefGuard::retain(state, poisoned, hidden_id)?.retire()?;
                            return Err(RuntimeError::Invariant(
                                "global property and hidden table contain distinct VarRefs",
                            ));
                        }
                        state.reset_var_ref_uninitialized(poisoned, id)?;
                        state.set_var_ref_metadata(
                            id,
                            false,
                            false,
                            ClosureVariableKind::Normal,
                        )?;
                        state.store_property_slot_with_poison(
                            poisoned,
                            hidden,
                            atom,
                            PropertyFlags::data(true, true, true),
                            PropertySlot::VarRef(id),
                        )?;
                    }
                    // A release may observe an older Heap zero queue even
                    // though this cell remains shared. Stop before publishing
                    // the accessor if that direct retirement fails.
                    root.retire()?;
                }
                // The cell temporary ends before the accessor publication,
                // while the hidden-object temporary ends after it.
                state.store_property_slot_with_poison(
                    poisoned,
                    object,
                    atom,
                    PropertyFlags::accessor(enumerable, configurable),
                    PropertySlot::accessor(
                        get.map(|value| match value {
                            RawValue::Object(id) => id,
                            _ => unreachable!("validated accessor"),
                        }),
                        set.map(|value| match value {
                            RawValue::Object(id) => id,
                            _ => unreachable!("validated accessor"),
                        }),
                    ),
                )
            }
        };
        result?;
        let (state, value) = hidden_owner.parts();
        state.release_owned_jsvalue(
            poisoned,
            value.take().expect("retained hidden global object"),
        )
    }

    /// Raw descriptor edges stay borrowed until the slot transaction retains them.
    /// Global data bindings use the shared cell kernel before this slot-only path.
    pub(super) fn store_complete_raw_property(
        &mut self,
        object: ObjectId,
        atom: Atom,
        complete: CompletePropertyDescriptor<RawValue>,
    ) -> Result<(), RuntimeError> {
        // Legacy entry remains for unconverted consumers; remove in B5.
        self.store_complete_raw_property_inner(None, object, atom, complete)
    }

    pub(super) fn store_complete_raw_property_inner(
        &mut self,
        poisoned: Option<&Cell<bool>>,
        object: ObjectId,
        atom: Atom,
        complete: CompletePropertyDescriptor<RawValue>,
    ) -> Result<(), RuntimeError> {
        let (flags, replacement) = match complete {
            CompletePropertyDescriptor::Data {
                value,
                writable,
                enumerable,
                configurable,
            } => (
                PropertyFlags::data(writable, enumerable, configurable),
                PropertySlot::Data(value),
            ),
            CompletePropertyDescriptor::Accessor {
                get,
                set,
                enumerable,
                configurable,
            } => {
                let id = |value| match value {
                    None => Ok(None),
                    Some(RawValue::Object(id)) => Ok(Some(id)),
                    _ => Err(RuntimeError::Invariant("raw accessor is not an object")),
                };
                (
                    PropertyFlags::accessor(enumerable, configurable),
                    PropertySlot::accessor(id(get)?, id(set)?),
                )
            }
        };
        self.store_property_slot_inner(poisoned, object, atom, flags, replacement)
    }

    pub(crate) fn store_property_slot(
        &mut self,
        object: ObjectId,
        atom: Atom,
        flags: PropertyFlags,
        replacement: PropertySlot,
    ) -> Result<(), RuntimeError> {
        self.store_property_slot_inner(None, object, atom, flags, replacement)
    }

    pub(super) fn store_property_slot_with_poison(
        &mut self,
        poisoned: &Cell<bool>,
        object: ObjectId,
        atom: Atom,
        flags: PropertyFlags,
        replacement: PropertySlot,
    ) -> Result<(), RuntimeError> {
        self.store_property_slot_inner(Some(poisoned), object, atom, flags, replacement)
    }

    fn store_property_slot_inner(
        &mut self,
        poisoned: Option<&Cell<bool>>,
        object: ObjectId,
        atom: Atom,
        flags: PropertyFlags,
        replacement: PropertySlot,
    ) -> Result<(), RuntimeError> {
        let state = self;
        let object_id = object;
        let existing = {
            let object_data = state.heap.object(object_id)?;
            let shape = state.heap.shape(object_data.shape)?;
            shape
                .find(AtomIdx::from_raw(atom.raw()))
                .map(|index| {
                    let index = index as usize;
                    let entry = shape.entries().get(index).ok_or(RuntimeError::Invariant(
                        "shape lookup index was out of bounds",
                    ))?;
                    Ok::<_, RuntimeError>((index, entry.flags))
                })
                .transpose()?
        };
        state.store_selected_property_slot_inner(
            poisoned,
            object_id,
            atom,
            flags,
            replacement,
            existing,
        )
    }

    /// Consume an own-slot selection made under this same exclusive state borrow.
    /// Callers must not release the borrow or perform another mutation between
    /// selecting `existing` and committing it. No slot index is cached in a VM state.
    pub(super) fn store_selected_property_slot(
        &mut self,
        object_id: ObjectId,
        atom: Atom,
        flags: PropertyFlags,
        replacement: PropertySlot,
        existing: Option<(usize, PropertyFlags)>,
    ) -> Result<(), RuntimeError> {
        // Legacy entry remains for unconverted consumers; remove in B5.
        self.store_selected_property_slot_inner(None, object_id, atom, flags, replacement, existing)
    }

    fn store_selected_property_slot_inner(
        &mut self,
        poisoned: Option<&Cell<bool>>,
        object_id: ObjectId,
        atom: Atom,
        flags: PropertyFlags,
        replacement: PropertySlot,
        existing: Option<(usize, PropertyFlags)>,
    ) -> Result<(), RuntimeError> {
        let state = self;
        if existing.is_none() {
            return state
                .append_selected_missing_slot(
                    poisoned,
                    object_id,
                    atom,
                    flags,
                    SlotAppendInput::Borrowed(replacement),
                )
                .map(drop);
        }
        let (shape_id, dictionary) = {
            let object_data = state.heap.object(object_id)?;
            let shape = state.heap.shape(object_data.shape)?;
            (object_data.shape, shape.is_dictionary())
        };

        // Replacing a value does not change the layout. Resolve that case while
        // borrowing the shape, before cloning entries or unrelated value slots.
        if let Some((index, existing_flags)) = existing {
            if existing_flags == flags {
                return if let Some(poisoned) = poisoned {
                    state.replace_property_slot_with_poison(poisoned, object_id, index, replacement)
                } else {
                    state.replace_property_slot(object_id, index, replacement)
                };
            }
        }
        if let Some((index, _)) = existing {
            if dictionary && state.heap.shape_strong_count(shape_id)? == 1 {
                let atoms = state.retain_slot_atoms(std::slice::from_ref(&replacement))?;
                return match state.heap.replace_dictionary_property_with_status(
                    object_id,
                    index,
                    flags,
                    replacement,
                ) {
                    Ok(cleanup) => state.apply_cleanup(cleanup).inspect_err(|_| {
                        if let Some(poisoned) = poisoned {
                            poisoned.set(true);
                        }
                    }),
                    Err(failure) => {
                        if failure.published {
                            if let Some(poisoned) = poisoned {
                                poisoned.set(true);
                            }
                        } else {
                            state.release_atoms(atoms).inspect_err(|_| {
                                if let Some(poisoned) = poisoned {
                                    poisoned.set(true);
                                }
                            })?;
                        }
                        Err(failure.error.into())
                    }
                };
            }
        }
        // Reconfiguration does not inherit append-only facts.
        state.unlink_shape_transitions(shape_id);
        let (prototype, mut entries, mut slots) = {
            let object_data = state.heap.object(object_id)?;
            let shape = state.heap.shape(object_data.shape)?;
            (
                shape.prototype(),
                shape.entries().to_vec(),
                object_data.slots.clone(),
            )
        };

        let (index, _) = existing.expect("missing append returned before reconfiguration");
        entries[index].flags = flags;
        slots[index] = replacement;
        if let Some(poisoned) = poisoned {
            state.replace_layout_with_poison(poisoned, object_id, prototype, &entries, slots)
        } else {
            state.replace_layout(object_id, prototype, &entries, slots)
        }
    }
    /// Append policy shared by ordinary descriptors and execution-owned stores.
    /// The caller selected this missing property under the current state borrow.
    pub(super) fn append_selected_missing_slot(
        &mut self,
        poisoned: Option<&Cell<bool>>,
        object_id: ObjectId,
        atom: Atom,
        flags: PropertyFlags,
        input: SlotAppendInput<'_>,
    ) -> Result<bool, RuntimeError> {
        let state = self;
        let object = state.heap.object(object_id)?;
        let shape_id = object.shape;
        let shape = state.heap.shape(shape_id)?;
        let shape_len = shape.entries().len();
        let dictionary = shape.is_dictionary();
        // Decline before mutating caches/layout; the frame keeps its owner.
        if dictionary
            && matches!(&input, SlotAppendInput::Owned(_))
            && state.heap.shape_strong_count(shape_id)? != 1
        {
            return Ok(false);
        }
        if !dictionary {
            state.retain_construction_shape(shape_id)?;
        }
        // An exclusively owned layout may append in place only when no
        // canonical successor already exists; otherwise the successor (and the
        // sharing it enables) would be stranded by an equivalent duplicate.
        if (dictionary || shape_len >= properties::MIN_UNIQUE_SHAPE_APPEND_ENTRIES)
            && state.heap.shape_strong_count(shape_id)? == 1
            && (dictionary
                || state
                    .canonical_successor(
                        shape_id,
                        ShapeEntry {
                            atom: AtomIdx::from_raw(atom.raw()),
                            flags,
                        },
                    )
                    .is_none())
        {
            return state
                .append_selected_unique_layout_input(
                    poisoned,
                    SelectedMissingAppend {
                        object: object_id,
                        shape: shape_id,
                        atom,
                        slot_count: shape_len,
                    },
                    flags,
                    input,
                )
                .map(|()| true);
        }
        if !dictionary {
            let target = state.append_transition(
                shape_id,
                ShapeEntry {
                    atom: AtomIdx::from_raw(atom.raw()),
                    flags,
                },
            )?;
            return state
                .append_slot_with_owned_shape_input(poisoned, object_id, target, input)
                .map(|()| true);
        }
        // Shared dictionaries require the complete rebuild algorithm. An
        // owning VM store declines this case before calling this kernel.
        let SlotAppendInput::Borrowed(replacement) = input else {
            return Err(RuntimeError::Invariant(
                "owned append reached shared dictionary",
            ));
        };
        state.unlink_shape_transitions(shape_id);
        let object = state.heap.object(object_id)?;
        let shape = state.heap.shape(shape_id)?;
        let prototype = shape.prototype();
        let mut entries = shape.entries().to_vec();
        let mut slots = object.slots.clone();
        entries.push(ShapeEntry {
            atom: AtomIdx::from_raw(atom.raw()),
            flags,
        });
        slots.push(replacement);
        if let Some(poisoned) = poisoned {
            state.replace_layout_with_poison(poisoned, object_id, prototype, &entries, slots)
        } else {
            state.replace_layout(object_id, prototype, &entries, slots)
        }
        .map(|()| true)
    }
}

#[cfg(test)]
mod global_state_tests;

#[cfg(test)]
mod selected_append_tests {
    use super::*;
    use crate::engine::heap::HeapError;

    #[test]
    fn canonical_append_preserves_symbols_accessors_self_edges_and_cache_updates() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        let result = context
            .eval(
                r#"(function () {
            var first = Symbol('first'), second = Symbol('second');
            var a = {old: first}, b = {old: first};
            a.self = a; b.self = b;
            a.next = second; b.next = second;
            var calls = 0;
            function both(v) { if (arguments.length) calls += v; return this.next; }
            Object.defineProperty(a, 'access', {get: both, set: both, configurable: true});
            Object.defineProperty(b, 'access', {get: both, set: both, configurable: true});
            if (a.access !== second || b.access !== second) return 1;
            a.access = 3; b.access = 4;
            if (calls !== 7 || a.old !== first || b.old !== first) return 2;
            if (a.self !== a || b.self !== b) return 3;
            function read(o) { return o.next; }
            if (read(a) !== second || read(b) !== second) return 4;
            delete a.next; a.next = first;
            if (read(a) !== first || read(b) !== second) return 5;
            var child = Object.create(a);
            if (child.next !== first) return 6;
            delete a.next; a.next = second;
            if (child.next !== second) return 7;
            a[first] = second; b[first] = first;
            if (a[first] !== second || b[first] !== first) return 8;
            return 42;
        })()"#,
            )
            .unwrap();
        assert_eq!(result, crate::engine::value::Value::Int(42));
    }

    #[test]
    fn canonical_append_rolls_back_new_symbol_owner_on_bad_successor() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        let crate::engine::value::Value::Object(owner) =
            context.eval("({old: Symbol('owned')})").unwrap()
        else {
            panic!()
        };
        let mut state = runtime.0.state.borrow_mut();
        let object = state.heap.object(owner.object_id()).unwrap();
        let shape = object.shape;
        let PropertySlot::Data(RawValue::Symbol(symbol)) = object.slots[0] else {
            panic!()
        };
        let symbol_atom = state.atoms.brand(symbol).unwrap();
        let before_atoms = state.atoms.resolve(symbol_atom).unwrap().ref_count;
        let before_shape = state.heap.shape_strong_count(shape).unwrap();
        // A shape with no appended entry fails before slot publication. The
        // runtime consumes its temporary shape owner and undoes only new atoms.
        state.heap.retain_shape(shape).unwrap();
        assert!(
            state
                .append_slot_with_owned_shape(
                    owner.object_id(),
                    shape,
                    PropertySlot::Data(RawValue::Symbol(symbol))
                )
                .is_err()
        );
        assert_eq!(
            state.atoms.resolve(symbol_atom).unwrap().ref_count,
            before_atoms
        );
        assert_eq!(state.heap.shape_strong_count(shape).unwrap(), before_shape);
        assert_eq!(state.heap.object(owner.object_id()).unwrap().slots.len(), 1);
        assert_eq!(state.heap.object(owner.object_id()).unwrap().shape, shape);
    }

    #[test]
    fn selected_missing_append_failure_restores_cache_and_atom_ownership() {
        let runtime = Runtime::new();
        let owner = runtime.new_object(None).unwrap();
        let stale = runtime.new_object(None).unwrap();
        let stale_id = stale.object_id();
        drop(stale);
        let key = runtime
            .intern_property_key("selected-failed-append")
            .unwrap();
        let mut state = runtime.0.state.borrow_mut();
        let shape = state.heap.object(owner.object_id()).unwrap().shape;
        assert_eq!(state.heap.shape_strong_count(shape), Ok(1));
        assert!(
            state
                .heap
                .shape(shape)
                .unwrap()
                .find(AtomIdx::from_raw(key.atom().raw()))
                .is_none()
        );
        let before_atoms = state.atoms.resolve(key.atom()).unwrap().ref_count;
        assert!(state.shape_is_canonical(shape));
        let selected = SelectedMissingAppend {
            object: owner.object_id(),
            shape,
            atom: key.atom(),
            slot_count: 0,
        };
        assert!(matches!(
            state.append_selected_unique_layout_input(
                None,
                selected,
                PropertyFlags::data(true, true, true),
                SlotAppendInput::Borrowed(PropertySlot::Data(RawValue::Object(stale_id))),
            ),
            Err(RuntimeError::Heap(HeapError::Stale { .. }))
        ));
        assert_eq!(
            state.atoms.resolve(key.atom()).unwrap().ref_count,
            before_atoms
        );
        assert!(state.heap.shape(shape).unwrap().entries().is_empty());
        assert!(state.shape_is_canonical(shape));
    }
}

fn strict_equal_immediate(left: &JsValue, right: &JsValue) -> bool {
    match (left, right) {
        (JsValue::Undefined, JsValue::Undefined) | (JsValue::Null, JsValue::Null) => true,
        (JsValue::Bool(left), JsValue::Bool(right)) => left == right,
        (JsValue::Symbol(left), JsValue::Symbol(right)) => left == right,
        (JsValue::Object(left), JsValue::Object(right)) => left == right,
        (JsValue::String(left), JsValue::String(right)) => left == right,
        (JsValue::BigInt(left), JsValue::BigInt(right)) => left == right,
        (JsValue::ShortBigInt(left), JsValue::ShortBigInt(right)) => left == right,
        (left, right) => match (left.as_number(), right.as_number()) {
            (Some(left), Some(right)) => left == right,
            _ => false,
        },
    }
}

#[cfg(test)]
mod direct_state_value_tests {
    use super::*;
    use crate::engine::value::{JsString, bigint::JsBigInt};

    #[test]
    fn direct_state_boolean_and_comparison_use_checked_leaf_content() {
        let runtime = Runtime::new();
        let left = runtime
            .into_jsvalue(Value::String(JsString::from_static("same content")))
            .unwrap();
        let right = runtime
            .into_jsvalue(Value::String(JsString::from_static("same content")))
            .unwrap();
        let empty = runtime
            .into_jsvalue(Value::String(JsString::from_static("")))
            .unwrap();
        let bigint = JsBigInt::parse_js_string("170141183460469231731687303715884105729").unwrap();
        let big_left = runtime.into_jsvalue(Value::BigInt(bigint.clone())).unwrap();
        let big_right = runtime.into_jsvalue(Value::BigInt(bigint)).unwrap();
        let mut state = runtime.0.state.borrow_mut();
        assert!(state.value_to_boolean_jsvalue(&left).unwrap());
        assert!(!state.value_to_boolean_jsvalue(&empty).unwrap());
        assert!(
            !state
                .value_to_boolean_jsvalue(&JsValue::ShortBigInt(0))
                .unwrap()
        );
        assert!(state.strict_equal_jsvalue(&left, &right).unwrap());
        assert!(!state.strict_equal_jsvalue(&left, &empty).unwrap());
        assert!(state.strict_equal_jsvalue(&big_left, &big_right).unwrap());
        assert!(
            !state
                .strict_equal_jsvalue(&big_left, &JsValue::ShortBigInt(1))
                .unwrap()
        );
        assert!(
            state
                .strict_equal_jsvalue(&JsValue::Int(0), &JsValue::Float(-0.0))
                .unwrap()
        );
        assert!(
            !state
                .strict_equal_jsvalue(&JsValue::Float(f64::NAN), &JsValue::Float(f64::NAN))
                .unwrap()
        );
        let JsValue::String(stale) = left else {
            panic!("string")
        };
        state.release_jsvalue(JsValue::String(stale)).unwrap();
        assert!(
            state
                .value_to_boolean_jsvalue(&JsValue::String(stale))
                .is_err()
        );
        assert!(
            state
                .strict_equal_jsvalue(&JsValue::String(stale), &right)
                .is_err()
        );
        for owner in [right, empty, big_left, big_right] {
            state.release_jsvalue(owner).unwrap();
        }
        assert!(!runtime.0.deferred_references.has_pending());
    }
}

impl RuntimeState {
    /// Apply ECMAScript `ToBoolean`, including QuickJS's Annex B falsy
    /// `is_HTMLDDA` object exception, under the current state access.
    pub(crate) fn value_to_boolean_jsvalue(&self, value: &JsValue) -> Result<bool, RuntimeError> {
        match value {
            JsValue::Object(id) => Ok(!self.heap.object(*id)?.is_html_dda),
            JsValue::String(id) => Ok(!self.heap.string(*id)?.is_empty()),
            JsValue::BigInt(id) => Ok(!self.heap.bigint(*id)?.is_zero()),
            _ => Ok(value.to_boolean_primitive()),
        }
    }

    /// Preserve handle identity shortcuts and compare separate String or
    /// BigInt handles through checked arena content.
    pub(crate) fn strict_equal_jsvalue(
        &self,
        left: &JsValue,
        right: &JsValue,
    ) -> Result<bool, RuntimeError> {
        Ok(match (left, right) {
            (JsValue::String(left), JsValue::String(right)) if left != right => {
                self.heap.string(*left)? == self.heap.string(*right)?
            }
            (JsValue::BigInt(left), JsValue::BigInt(right)) if left != right => {
                self.heap.bigint(*left)? == self.heap.bigint(*right)?
            }
            (JsValue::ShortBigInt(value), JsValue::BigInt(id))
            | (JsValue::BigInt(id), JsValue::ShortBigInt(value)) => {
                self.heap.bigint(*id)? == &crate::engine::value::bigint::JsBigInt::from(*value)
            }
            _ => strict_equal_immediate(left, right),
        })
    }
}

#[cfg(test)]
mod state_descriptor_tests {
    use super::*;
    use crate::engine::heap::RawId;
    use crate::engine::object::property::{PropertyDefinitionError, PropertyDescriptor};

    #[test]
    fn state_descriptor_tracks_aliased_accessors_and_self_edges() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        let Value::Object(function) = context.eval("(function(v){return v})").unwrap() else {
            panic!("callable")
        };
        let object = runtime.new_object(None).unwrap();
        let key = runtime.intern_property_key("owned-accessor").unwrap();
        let mut state = runtime.0.state.borrow_mut();
        let before = state
            .heap
            .object_strong_count(function.object_id())
            .unwrap();
        let getter = RawValue::Object(function.object_id());
        assert!(
            state
                .define_raw_property(
                    object.object_id(),
                    key.atom(),
                    &PropertyDescriptor {
                        get: Some(Some(getter.clone())),
                        set: Some(Some(getter)),
                        configurable: Some(true),
                        ..PropertyDescriptor::new()
                    }
                )
                .unwrap()
        );
        assert_eq!(
            state
                .heap
                .object_strong_count(function.object_id())
                .unwrap(),
            before + 2
        );
        let before_object = state.heap.object_strong_count(object.object_id()).unwrap();
        assert!(
            state
                .define_raw_property(
                    object.object_id(),
                    key.atom(),
                    &PropertyDescriptor {
                        value: Some(RawValue::Object(object.object_id())),
                        writable: Some(true),
                        ..PropertyDescriptor::new()
                    }
                )
                .unwrap()
        );
        assert_eq!(
            state
                .heap
                .object_strong_count(function.object_id())
                .unwrap(),
            before
        );
        assert_eq!(
            state.heap.object_strong_count(object.object_id()).unwrap(),
            before_object + 1
        );
        assert!(
            state
                .define_raw_property(
                    object.object_id(),
                    key.atom(),
                    &PropertyDescriptor {
                        value: Some(RawValue::Bool(true)),
                        ..PropertyDescriptor::new()
                    }
                )
                .unwrap()
        );
        assert_eq!(
            state.heap.object_strong_count(object.object_id()).unwrap(),
            before_object
        );
        assert_eq!(
            state.define_raw_property(
                object.object_id(),
                key.atom(),
                &PropertyDescriptor {
                    value: Some(RawValue::Bool(false)),
                    get: Some(None),
                    ..PropertyDescriptor::new()
                }
            ),
            Err(RuntimeError::Property(
                PropertyDefinitionError::InvalidDescriptor
            ))
        );
        assert!(!runtime.0.deferred_references.has_pending());
    }

    #[test]
    fn state_descriptor_rolls_back_first_accessor_when_second_retain_overflows() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        let Value::Object(getter) = context.eval("(function(){return 1})").unwrap() else {
            panic!("getter")
        };
        let Value::Object(setter) = context.eval("(function(v){})").unwrap() else {
            panic!("setter")
        };
        let object = runtime.new_object(None).unwrap();
        let key = runtime.intern_property_key("rollback-accessor").unwrap();
        let mut state = runtime.0.state.borrow_mut();
        let getter_before = state.heap.object_strong_count(getter.object_id()).unwrap();
        let setter_before = state.heap.object_strong_count(setter.object_id()).unwrap();
        state
            .heap
            .set_strong_count_for_test(RawId::Object(setter.object_id()), u32::MAX);
        let result = state.define_raw_property(
            object.object_id(),
            key.atom(),
            &PropertyDescriptor {
                get: Some(Some(RawValue::Object(getter.object_id()))),
                set: Some(Some(RawValue::Object(setter.object_id()))),
                ..PropertyDescriptor::new()
            },
        );
        state
            .heap
            .set_strong_count_for_test(RawId::Object(setter.object_id()), setter_before);
        assert!(matches!(
            result,
            Err(RuntimeError::Heap(
                crate::engine::heap::HeapError::Overflow { .. }
            ))
        ));
        assert_eq!(
            state.heap.object_strong_count(getter.object_id()).unwrap(),
            getter_before
        );
        let data = state.heap.object(object.object_id()).unwrap();
        assert!(
            state
                .heap
                .shape(data.shape)
                .unwrap()
                .find(AtomIdx::from_raw(key.atom().raw()))
                .is_none()
        );
        assert!(!runtime.0.poisoned.get());
        assert!(!runtime.0.deferred_references.has_pending());
    }
}
