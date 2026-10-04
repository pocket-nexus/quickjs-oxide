//! One callback-free Arguments layout algorithm and its actual raw producers.
use crate::engine::{
    api::runtime_error::RuntimeError,
    atom::{Atom, AtomIdx, pinned::PinnedAtom},
    heap::{
        ContextId, ObjectData, ObjectId, PropertySlot, RawValue, VarRefId,
        runtime::{
            RuntimeState,
            owned_values::{OwnedValueGuard, OwnedValuesGuard},
        },
    },
    object::{
        WellKnownSymbol,
        shape::{PropertyFlags, ShapeEntry},
    },
    value::JsValue,
};
use std::cell::Cell;

/// The caller keeps its original callee edge live from the checked prefix
/// through allocation; this fact introduces no additional owner.
pub(crate) struct CheckedArgumentsCallee(ObjectId);
struct ArgumentsLayout {
    entries: Vec<ShapeEntry>,
    slots: Vec<PropertySlot>,
}
impl ArgumentsLayout {
    fn new(count: usize) -> Self {
        Self {
            entries: Vec::with_capacity(count + 3),
            slots: Vec::with_capacity(count + 3),
        }
    }
    fn push(&mut self, atom: Atom, flags: PropertyFlags, slot: PropertySlot) {
        self.entries.push(ShapeEntry {
            atom: AtomIdx::from_raw(atom.raw()),
            flags,
        });
        self.slots.push(slot);
    }
}
impl RuntimeState {
    /// Preserve as_callable's actual checked temporary and retirement before
    /// cell-domain validation at the public adapter or resident layout work.
    pub(crate) fn checked_arguments_callee(
        &mut self,
        poisoned: &Cell<bool>,
        function: ObjectId,
    ) -> Result<CheckedArgumentsCallee, RuntimeError> {
        if !self.object_id_has_call_capability(function)? {
            return Err(RuntimeError::Invariant(
                "mapped arguments callee has no [[Call]] method",
            ));
        }
        let callable = self.dup_jsvalue(&JsValue::Object(function))?;
        self.release_owned_jsvalue(poisoned, callable)?;
        Ok(CheckedArgumentsCallee(function))
    }
    pub(crate) fn new_unmapped_arguments_object(
        &mut self,
        poisoned: &Cell<bool>,
        realm: ContextId,
        values: Vec<JsValue>,
    ) -> Result<ObjectId, RuntimeError> {
        let mut values = OwnedValuesGuard::new(self, poisoned, values);
        let (state, values) = values.parts();
        let length = u32::try_from(values.len()).map_err(|_| {
            RuntimeError::Invariant("actual argument count exceeded QuickJS Uint32 storage")
        })?;
        let slots = values
            .iter()
            .map(|value| PropertySlot::Data(value.as_raw()));
        let object = state.new_arguments_layout(poisoned, realm, length, None, slots)?;
        let mut result = OwnedValueGuard::new(state, poisoned, JsValue::Object(object));
        let (state, result) = result.parts();
        for value in values.iter_mut() {
            state.release_owned_jsvalue(poisoned, std::mem::replace(value, JsValue::Undefined))?;
        }
        let Some(JsValue::Object(object)) = result.take() else {
            unreachable!()
        };
        Ok(object)
    }
    /// Consume each admitted cell producer exactly once after keys retire.
    pub(crate) fn new_mapped_arguments_object(
        &mut self,
        poisoned: &Cell<bool>,
        realm: ContextId,
        callee: CheckedArgumentsCallee,
        cells: Vec<VarRefId>,
    ) -> Result<ObjectId, RuntimeError> {
        let mut cells = MappedArgumentsInput {
            state: self,
            poisoned,
            cells,
        };
        let length = u32::try_from(cells.cells.len()).map_err(|_| {
            RuntimeError::Invariant("actual argument count exceeded QuickJS Uint32 storage")
        })?;
        let slots = cells.cells.iter().copied().map(PropertySlot::VarRef);
        let object =
            cells
                .state
                .new_arguments_layout(poisoned, realm, length, Some(callee.0), slots)?;
        let mut result = OwnedValueGuard::new(&mut *cells.state, poisoned, JsValue::Object(object));
        let (state, result) = result.parts();
        for cell in cells.cells.drain(..) {
            state
                .release_var_ref_handle(cell)
                .inspect_err(|_| poisoned.set(true))?;
        }
        let Some(JsValue::Object(object)) = result.take() else {
            unreachable!()
        };
        Ok(object)
    }
    fn new_arguments_layout(
        &mut self,
        poisoned: &Cell<bool>,
        realm: ContextId,
        length: u32,
        function: Option<ObjectId>,
        slots: impl Iterator<Item = PropertySlot>,
    ) -> Result<ObjectId, RuntimeError> {
        let mut layout = ArgumentsLayout::new(length as usize);
        let mut keys =
            OwnedValuesGuard::new(self, poisoned, Vec::with_capacity(length as usize + 3));
        let (state, keys) = keys.parts();
        for (index, slot) in slots.enumerate() {
            let key = state.property_key_atom_for_index(index as u64)?;
            keys.push(JsValue::Symbol(AtomIdx::from_raw(key.raw())));
            layout.push(key, PropertyFlags::data(true, true, true), slot);
        }
        let context = state.heap.context(realm)?;
        let array_values = context
            .array_prototype_values
            .ok_or(RuntimeError::Invariant(
                "realm has no cached Array.prototype.values root",
            ))?;
        let thrower = context.throw_type_error.ok_or(RuntimeError::Invariant(
            "realm has no shared %ThrowTypeError% root",
        ))?;
        let key = state.pinned_atoms.get(PinnedAtom::Length);
        keys.push(JsValue::Symbol(AtomIdx::from_raw(key.raw())));
        let length_value = if let Ok(value) = i32::try_from(length) {
            RawValue::Int(value)
        } else {
            RawValue::Float(f64::from(length))
        };
        layout.push(
            key,
            PropertyFlags::data(true, false, true),
            PropertySlot::Data(length_value),
        );
        let key = state.pinned_atoms.get(PinnedAtom::Callee);
        keys.push(JsValue::Symbol(AtomIdx::from_raw(key.raw())));
        let (flags, slot) = if let Some(function) = function {
            (
                PropertyFlags::data(true, false, true),
                PropertySlot::Data(RawValue::Object(function)),
            )
        } else {
            (
                PropertyFlags::accessor(false, false),
                PropertySlot::accessor(Some(thrower), Some(thrower)),
            )
        };
        layout.push(key, flags, slot);
        let key = state.well_known_symbols[&WellKnownSymbol::Iterator];
        keys.push(JsValue::Symbol(AtomIdx::from_raw(key.raw())));
        layout.push(
            key,
            PropertyFlags::data(true, false, true),
            PropertySlot::Data(RawValue::Object(array_values)),
        );
        let prototype = state.heap.context(realm)?.object_prototype;
        let object = state.allocate_object_with_layout(
            poisoned,
            Some(prototype),
            &layout.entries,
            layout.slots,
            |shape, slots| ObjectData::arguments(shape, slots, function.is_some(), length),
        )?;
        let mut result = OwnedValueGuard::new(state, poisoned, JsValue::Object(object));
        let (state, result) = result.parts();
        // Old layout PropertyKey owners retired before consumed input values.
        for key in keys.iter_mut() {
            state.release_owned_jsvalue(poisoned, std::mem::replace(key, JsValue::Undefined))?;
        }
        let Some(JsValue::Object(object)) = result.take() else {
            unreachable!()
        };
        Ok(object)
    }
}
/// Concrete producer cells belonging to this factory, with no Runtime owner.
struct MappedArgumentsInput<'a> {
    state: &'a mut RuntimeState,
    poisoned: &'a Cell<bool>,
    cells: Vec<VarRefId>,
}
impl Drop for MappedArgumentsInput<'_> {
    fn drop(&mut self) {
        if std::thread::panicking() {
            self.poisoned.set(true);
        }
        if self.poisoned.get() {
            return;
        }
        let _unwind = crate::engine::api::runtime::RuntimeUnwindGuard::from_flag(self.poisoned);
        for cell in self.cells.drain(..) {
            if self.state.release_var_ref_handle(cell).is_err() {
                self.poisoned.set(true);
                break;
            }
        }
    }
}
