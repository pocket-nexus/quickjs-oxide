//! Short, non-reentrant access to ordinary own slots. Slot positions never
//! leave this module and a write locates and commits under one state borrow.

mod ic;
use crate::engine::api::runtime::Runtime;
use crate::engine::api::runtime_error::RuntimeError;
use crate::engine::atom::{Atom, AtomIdx};
use crate::engine::heap::runtime::RuntimeState;
use crate::engine::heap::{ObjectId, ObjectKind, ObjectPayload, PropertySlot, RawValue};
use crate::engine::object::shape::PropertyFlags;
use crate::engine::object::{ObjectRef, PropertyKey};
use crate::engine::value::number::operations::Number;
use crate::engine::value::{JsValue, Value};

/// Affine native payload fact selected together with an own property value.
/// Its callee remains retained by the result/operand owner; consumption checks
/// runtime and generational identity, never re-reads a property or payload.
pub(crate) struct LinkedNativeSelection {
    runtime: Runtime,
    function: ObjectId,
    data: crate::engine::builtins::native::NativeFunctionData,
}
impl LinkedNativeSelection {
    pub(crate) fn into_parts_jsvalue(
        self,
        runtime: &Runtime,
        function: ObjectId,
    ) -> Option<crate::engine::builtins::native::NativeFunctionData> {
        (runtime.domain_id() == self.runtime.domain_id() && function == self.function)
            .then_some(self.data)
    }
}

#[derive(Clone, Copy)]
struct OwnSlot {
    index: usize,
    flags: PropertyFlags,
}

// Payload alone is insufficient: module namespaces share Ordinary storage.
fn is_ordinary(data: &crate::engine::heap::ObjectData) -> bool {
    matches!(
        (data.kind, &data.payload),
        (ObjectKind::Ordinary, ObjectPayload::Ordinary)
    )
}

/// These payloads keep every own property in their ordinary shape/slot
/// arrays, so their reads can use the borrowed probe instead of the
/// owned-descriptor kernel. Functions matter for the
/// `Get(constructor, "prototype")` step of every `new` expression; strong
/// collections and their iterators matter for uncached method reads such as
/// `set.has`. Lazy slots (`name`, `length`, function `prototype`) are
/// AutoInit entries and decline per slot below; collection records live in
/// the payload, never as virtual own properties.
fn reads_are_slot_faithful(data: &crate::engine::heap::ObjectData) -> bool {
    matches!(
        &data.payload,
        ObjectPayload::NativeFunction { .. }
            | ObjectPayload::BoundFunction { .. }
            | ObjectPayload::BytecodeFunction { .. }
            | ObjectPayload::Map { .. }
            | ObjectPayload::Set { .. }
            | ObjectPayload::WeakMap { .. }
            | ObjectPayload::WeakSet { .. }
            | ObjectPayload::MapIterator { .. }
            | ObjectPayload::SetIterator { .. }
    )
}

// The caller keeps the state borrowed until the located slot is consumed.
fn locate(
    state: &RuntimeState,
    object: ObjectId,
    atom: Atom,
) -> Result<Option<OwnSlot>, RuntimeError> {
    let data = state.heap.object(object)?;
    let shape = state.heap.shape(data.shape)?;
    let Some(index) = shape.find(AtomIdx::from_raw(atom.raw())) else {
        return Ok(None);
    };
    let index = index as usize;
    let entry = shape.entries().get(index).ok_or(RuntimeError::Invariant(
        "ordinary shape index is out of bounds",
    ))?;
    if data.slots.get(index).is_none() {
        return Err(RuntimeError::Invariant(
            "ordinary shape has no parallel slot",
        ));
    }
    Ok(Some(OwnSlot {
        index,
        flags: entry.flags,
    }))
}

// A materialized Array keeps indexed properties in its ordinary shape and
// slots. Select a single own data Number without constructing a key,
// retaining an owner, or walking the prototype chain. Missing and exotic
// properties must continue through the canonical [[Get]] operation.
fn materialized_array_own_number(
    state: &RuntimeState,
    data: &crate::engine::heap::ObjectData,
    index: u32,
) -> Option<Number> {
    let atom = Atom::from_immediate_integer(index)?;
    let shape = state.heap.shape(data.shape).ok()?;
    let slot = shape.find(AtomIdx::from_raw(atom.raw()))? as usize;
    let number = match data.slots.get(slot)? {
        PropertySlot::Data(RawValue::Int(value)) => Some(Number::Int(*value)),
        PropertySlot::Data(RawValue::Float(value)) => Some(Number::Float(*value)),
        _ => None,
    }?;
    #[cfg(feature = "profiling")]
    crate::engine::api::profiling::record_owned_execution_event(
        "array_materialized_own_number_read",
    );
    Some(number)
}

// The shared physical selector used by both single-step Set and the local
// missing-receiver walk. It neither roots handles nor performs observable work.
enum BorrowedSet {
    Missing(Option<ObjectId>),
    Data(OwnSlot),
    Setter(Option<ObjectId>),
    Special(SpecialKind),
}
fn select_set_slot(
    state: &RuntimeState,
    id: ObjectId,
    atom: Atom,
) -> Result<BorrowedSet, RuntimeError> {
    let data = state.heap.object(id)?;
    let ordinary = is_ordinary(data);

    let ordinary = ordinary
        || match &data.payload {
            // RegExp's lastIndex is an ordinary data property. The branded
            // executor changes its value but adds no exotic [[Set]] semantics.
            ObjectPayload::RegExp(_) => true,
            ObjectPayload::Array { .. } => {
                let shape = state.heap.shape(data.shape)?;
                state.atoms.array_index(atom)?.is_none()
                    && shape
                        .entries()
                        .first()
                        .is_some_and(|entry| entry.atom != AtomIdx::from_raw(atom.raw()))
            }
            _ => false,
        };
    if !ordinary {
        return Ok(BorrowedSet::Special(special_kind(data)));
    }
    Ok(match locate(state, id, atom)? {
        None => BorrowedSet::Missing(state.heap.shape(data.shape)?.prototype()),
        Some(slot) => match &data.slots[slot.index] {
            PropertySlot::Data(_) => BorrowedSet::Data(slot),
            PropertySlot::Accessor { set, .. } => BorrowedSet::Setter(set.option()),
            PropertySlot::AutoInit(_) | PropertySlot::VarRef(_) => {
                BorrowedSet::Special(SpecialKind::Other)
            }
        },
    })
}

enum MissingSelection {
    Define,
    Complete(SetProbe),
    Special(ObjectId, SpecialKind),
}

fn select_missing_prototypes(
    state: &RuntimeState,
    atom: Atom,
    mut prototype: Option<ObjectId>,
) -> Result<MissingSelection, RuntimeError> {
    while let Some(id) = prototype {
        let data = state.heap.object(id)?;
        if let ObjectPayload::Array { dense } = &data.payload
            && let Some(index) = state.atoms.array_index(atom)?
        {
            // Prototype lookup does not grow this Array. Dense elements are
            // writable data properties; missing indices continue the chain.
            if dense
                .as_ref()
                .is_some_and(|values| (index as usize) < values.len())
            {
                break;
            }
            if let Some(slot) = locate(state, id, atom)? {
                match &data.slots[slot.index] {
                    PropertySlot::Data(_) if slot.flags.writable => break,
                    PropertySlot::Data(_) => {
                        return Ok(MissingSelection::Complete(SetProbe::Stored(false)));
                    }
                    PropertySlot::Accessor { set, .. } => {
                        return Ok(MissingSelection::Complete(SetProbe::Setter(set.option())));
                    }
                    _ => return Ok(MissingSelection::Special(id, SpecialKind::Other)),
                }
            }
            prototype = state.heap.shape(data.shape)?.prototype();
            continue;
        }
        match select_set_slot(state, id, atom)? {
            BorrowedSet::Missing(next) => prototype = next,
            BorrowedSet::Data(slot) if slot.flags.writable => break,
            BorrowedSet::Data(_) => return Ok(MissingSelection::Complete(SetProbe::Stored(false))),
            BorrowedSet::Setter(setter) => {
                return Ok(MissingSelection::Complete(SetProbe::Setter(setter)));
            }
            BorrowedSet::Special(kind) => return Ok(MissingSelection::Special(id, kind)),
        }
    }
    Ok(MissingSelection::Define)
}

/// Only a proof for the current uninterrupted borrow, never cached.
pub(super) fn prototypes_allow_dense_append(
    state: &RuntimeState,
    atom: Atom,
    prototype: Option<ObjectId>,
) -> Result<bool, RuntimeError> {
    Ok(matches!(
        select_missing_prototypes(state, atom, prototype)?,
        MissingSelection::Define
    ))
}

/// Continue an already-selected missing own property without releasing the
/// borrow. Any exotic boundary declines before changing the receiver; the
/// ordinary state machine then performs its original observable protocol.
///
/// A `Define` result means the define is validated and still missing: the
/// caller ends the borrow and commits the existing value handle through
/// [`Runtime::store_property_slot`]. No callback or mutation can run between
/// this selection and that commit, so splitting the borrow is unobservable.
fn set_missing_local(
    state: &mut RuntimeState,
    receiver: ObjectId,
    atom: Atom,
    _value: &JsValue,
    prototype: Option<ObjectId>,
) -> Result<MissingSelection, RuntimeError> {
    match select_missing_prototypes(state, atom, prototype)? {
        MissingSelection::Define => {}
        selected => return Ok(selected),
    }
    // A complete new data descriptor has no compatibility comparison; the
    // shared descriptor algorithm rejects it exactly when not extensible.
    if !state.heap.object(receiver)?.extensible {
        return Ok(MissingSelection::Complete(SetProbe::Rejected(
            crate::engine::object::operations::PropertySetRejection::NotExtensible,
        )));
    }
    Ok(MissingSelection::Define)
}

#[derive(Clone, Copy)]
pub(crate) enum SpecialKind {
    Proxy,
    TypedArray,
    ModuleNamespace,
    Other,
}

// Reuse only for the immediate fallback, before any observable operation.
fn special_kind(data: &crate::engine::heap::ObjectData) -> SpecialKind {
    match (data.kind, &data.payload) {
        (_, ObjectPayload::Proxy(_)) => SpecialKind::Proxy,
        (_, ObjectPayload::TypedArray(_)) => SpecialKind::TypedArray,
        (ObjectKind::ModuleNamespace, _) => SpecialKind::ModuleNamespace,
        _ => SpecialKind::Other,
    }
}

pub(super) enum SetProbe {
    Stored(bool),

    Rejected(crate::engine::object::operations::PropertySetRejection),

    SpecialAt(ObjectRef, SpecialKind),
    Writable,
    Setter(Option<ObjectId>),
    Missing(Option<ObjectRef>),
    Special(SpecialKind),
}

impl Runtime {
    /// The target stays rooted until the selected setter/prototype has been
    /// promoted. No callback, mutation or cleanup occurs between snapshot and
    /// promotion. Data writes locate and commit in a single mutable borrow.
    pub(super) fn ordinary_set_probe(
        &self,
        object: &ObjectRef,
        key: &PropertyKey,
        value: &JsValue,
        receiver_is_target: bool,
    ) -> Result<SetProbe, RuntimeError> {
        self.ordinary_set_probe_inner(object, key, value, receiver_is_target, true)
    }

    // OrdinarySetWithOwnDescriptor checks only Receiver's own descriptor. Its
    // prototype must not be consulted again after the target selected a writable
    // data descriptor (Reflect.set may have a completely different receiver).
    pub(super) fn ordinary_set_receiver_probe(
        &self,
        object: &ObjectRef,
        key: &PropertyKey,
        value: &JsValue,
    ) -> Result<SetProbe, RuntimeError> {
        self.ordinary_set_probe_inner(object, key, value, true, false)
    }

    fn ordinary_set_probe_inner(
        &self,
        object: &ObjectRef,
        key: &PropertyKey,
        value: &JsValue,
        receiver_is_target: bool,
        _walk_missing: bool,
    ) -> Result<SetProbe, RuntimeError> {
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_execution_event("property_storage_set_probe");
        enum Selected {
            Setter(Option<ObjectId>),
            Missing(Option<ObjectId>),
            Dense(u32),

            DenseAppend(u32),

            SpecialAt(ObjectId, SpecialKind),

            /// A validated missing define on the target: commit after the
            /// borrow, once the value conversion can take it.
            Define,

            /// A selected writable own data slot awaiting replacement.
            DataReplace(OwnSlot),
        }
        let selected = {
            let mut state = self.0.state.borrow_mut();
            let id = object.object_id();
            let data = state.heap.object(id)?;

            let dense_index = if receiver_is_target && matches!(data.kind, ObjectKind::Array) {
                key.atom().immediate_integer().and_then(|index| {
                    if let ObjectPayload::Array { dense: Some(dense) } = &data.payload {
                        (index as usize <= dense.len()).then_some((index, dense.len()))
                    } else {
                        None
                    }
                })
            } else {
                None
            };

            if let Some((index, dense_len)) = dense_index {
                if (index as usize) < dense_len {
                    Selected::Dense(index)
                } else {
                    {
                        let prototype = if _walk_missing {
                            state.heap.shape(data.shape)?.prototype()
                        } else {
                            None
                        };
                        match select_missing_prototypes(&state, key.atom(), prototype)? {
                            MissingSelection::Define => Selected::DenseAppend(index),
                            MissingSelection::Complete(result) => return Ok(result),
                            MissingSelection::Special(id, kind) => Selected::SpecialAt(id, kind),
                        }
                    }
                }
            } else {
                match select_set_slot(&state, id, key.atom())? {
                    BorrowedSet::Missing(prototype) => {
                        if receiver_is_target {
                            match set_missing_local(
                                &mut state,
                                id,
                                key.atom(),
                                value,
                                if _walk_missing { prototype } else { None },
                            )? {
                                MissingSelection::Complete(result) => return Ok(result),
                                MissingSelection::Special(id, kind) => {
                                    Selected::SpecialAt(id, kind)
                                }
                                MissingSelection::Define => Selected::Define,
                            }
                        } else {
                            Selected::Missing(prototype)
                        }
                    }
                    BorrowedSet::Data(slot) => {
                        if !slot.flags.writable {
                            return Ok(SetProbe::Stored(false));
                        }
                        if !receiver_is_target {
                            return Ok(SetProbe::Writable);
                        }
                        Selected::DataReplace(slot)
                    }
                    BorrowedSet::Setter(set) => Selected::Setter(set),
                    BorrowedSet::Special(kind) => return Ok(SetProbe::Special(kind)),
                }
            }
        };
        Ok(match selected {
            Selected::Dense(index) => {
                // No callback or owner release occurs between selection and
                // this authoritative transaction, which rechecks dense bounds.
                self.replace_dense_array_value_jsvalue(object, index, value)?;
                SetProbe::Stored(true)
            }

            Selected::SpecialAt(id, kind) => {
                SetProbe::SpecialAt(ObjectRef::from_borrowed_handle(self.clone(), id)?, kind)
            }

            Selected::DenseAppend(index) => {
                // This is the exact missing element selected above, with no
                // callback or owner release before the shared Array definition.
                match self.define_selected_dense_array_append(object, index, value)? {
                    None => SetProbe::Stored(true),
                    Some(reason) => SetProbe::Rejected(reason),
                }
            }
            Selected::Define => {
                // Retain the existing handle into storage; the borrowed input
                // owns its producer edge throughout this transaction.
                let stored = self.store_property_slot(
                    object,
                    key,
                    PropertyFlags::data(true, true, true),
                    PropertySlot::Data(value.as_raw()),
                );
                stored?;
                #[cfg(feature = "profiling")]
                crate::engine::api::profiling::record_owned_execution_event(
                    "set_missing_committed_from_selection",
                );
                SetProbe::Stored(true)
            }
            Selected::DataReplace(slot) => {
                let raw = value.as_raw();
                let mut state = self.0.state.borrow_mut();
                let replaced = replace_data(
                    &mut state,
                    object.object_id(),
                    slot,
                    PropertySlot::Data(raw),
                );
                drop(state);
                replaced?;
                return Ok(SetProbe::Stored(true));
            }
            Selected::Setter(set) => SetProbe::Setter(set),
            Selected::Missing(prototype) => SetProbe::Missing(
                prototype
                    .map(|id| ObjectRef::from_borrowed_handle(self.clone(), id))
                    .transpose()?,
            ),
        })
    }
}

fn replace_data(
    state: &mut RuntimeState,
    id: ObjectId,
    slot: OwnSlot,
    replacement: PropertySlot,
) -> Result<(), RuntimeError> {
    state.replace_property_slot(id, slot.index, replacement)
}

pub(super) enum ReadProbe {
    Value(JsValue),
    Getter(Option<crate::engine::object::CallableRef>),
    Missing(Option<ObjectRef>),
    Special(SpecialKind),
}

pub(super) struct OwnFlags {
    pub(super) flags: PropertyFlags,
    pub(super) needs_materialization: bool,
}

impl Runtime {
    pub(super) fn ordinary_property_flags(
        &self,
        object: &ObjectRef,
        key: &PropertyKey,
    ) -> Result<Option<Option<OwnFlags>>, RuntimeError> {
        self.validate_object_and_key(object, key)?;
        let state = self.0.state.borrow();
        let id = object.object_id();
        if !is_ordinary(state.heap.object(id)?) {
            return Ok(None);
        }
        Ok(Some(locate(&state, id, key.atom())?.map(|slot| OwnFlags {
            flags: slot.flags,
            needs_materialization: matches!(
                state.heap.object(id).expect("located live object").slots[slot.index],
                PropertySlot::AutoInit(_) | PropertySlot::VarRef(_)
            ),
        })))
    }

    pub(super) fn ordinary_property_snapshot(
        &self,
        object: &ObjectRef,
        key: &PropertyKey,
    ) -> Result<Option<Option<crate::engine::object::operations::PropertySnapshot>>, RuntimeError>
    {
        use crate::engine::object::operations::PropertySnapshot;
        let state = self.0.state.borrow();
        let id = object.object_id();
        if !is_ordinary(state.heap.object(id)?) {
            return Ok(None);
        }
        let Some(slot) = locate(&state, id, key.atom())? else {
            return Ok(Some(None));
        };
        let flags = slot.flags;
        Ok(Some(Some(
            match &state.heap.object(id)?.slots[slot.index] {
                PropertySlot::Data(value) => PropertySnapshot::Data {
                    value: value.clone(),
                    flags,
                },
                PropertySlot::Accessor { get, set } => PropertySnapshot::Accessor {
                    get: get.option(),
                    set: set.option(),
                    flags,
                },
                PropertySlot::VarRef(var_ref) => PropertySnapshot::VarRef {
                    var_ref: *var_ref,
                    flags,
                },
                PropertySlot::AutoInit(_) => PropertySnapshot::AutoInit,
            },
        )))
    }

    #[cfg(test)]
    pub(super) fn ordinary_read_probe(
        &self,
        object: &ObjectRef,
        key: &PropertyKey,
    ) -> Result<ReadProbe, RuntimeError> {
        if !object.belongs_to(self) {
            return Err(RuntimeError::WrongRuntime("property object"));
        }
        self.ordinary_read_probe_atom(object.object_id(), key.atom(), false, None)
    }

    pub(super) fn ordinary_read_probe_selected_id(
        &self,
        object: ObjectId,
        key: &PropertyKey,
        native: Option<&mut Option<LinkedNativeSelection>>,
    ) -> Result<ReadProbe, RuntimeError> {
        self.ordinary_read_probe_atom(object, key.atom(), false, native)
    }

    // The caller owns the receiver throughout this non-reentrant probe.
    // Output values/getters/prototypes acquire their own edges below.
    fn ordinary_read_probe_atom(
        &self,
        id: ObjectId,
        atom: Atom,
        own_only: bool,
        mut native: Option<&mut Option<LinkedNativeSelection>>,
    ) -> Result<ReadProbe, RuntimeError> {
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_execution_event("property_storage_read_probe");
        enum Selected {
            Value(crate::engine::heap::RawValue),
            Getter(Option<ObjectId>),
            Missing(Option<ObjectId>),
        }
        let mut native_data = None;
        let selected = {
            let state = self.0.state.borrow();
            let data = state.heap.object(id)?;
            let is_array = matches!(
                (data.kind, &data.payload),
                (ObjectKind::Array, ObjectPayload::Array { .. })
            );
            // Dense elements are own data properties. Read the value under
            // this same classification borrow. Other own Array slots share
            // value/getter selection; exotic misses retain their fallback.
            let selected = if let Some(index) = atom.immediate_integer()
                && let Some(value) = data.dense_array_value(index)
            {
                Selected::Value(value.clone())
            } else if !is_ordinary(data) && !is_array && !reads_are_slot_faithful(data) {
                return Ok(ReadProbe::Special(special_kind(data)));
            } else {
                match locate(&state, id, atom)? {
                    // Numeric misses may still select non-immediate dense indices.
                    // Named/symbol misses have ordinary prototype lookup and need no exotic call.
                    None if is_array && state.atoms.array_index(atom)?.is_some() => {
                        return Ok(ReadProbe::Special(SpecialKind::Other));
                    }
                    None => Selected::Missing(if own_only {
                        None
                    } else {
                        state.heap.shape(data.shape)?.prototype()
                    }),
                    Some(slot) => match &data.slots[slot.index] {
                        PropertySlot::Data(value) => Selected::Value(value.clone()),
                        PropertySlot::Accessor { get, .. } => Selected::Getter(get.option()),
                        PropertySlot::AutoInit(_) | PropertySlot::VarRef(_) => {
                            return Ok(ReadProbe::Special(SpecialKind::Other));
                        }
                    },
                }
            };

            if native.is_some()
                && let Selected::Value(crate::engine::heap::RawValue::Object(id)) = &selected
            {
                // Invalid native metadata must still fail at the original Call,
                // not during this GetField. Such payloads simply get no hint.
                native_data = state.heap.object(*id).ok().and_then(|object| {
                    let ObjectPayload::NativeFunction { data, .. } = &object.payload else {
                        return None;
                    };
                    let realm = data.realm?;
                    (data.operation().is_some() && state.heap.context(realm).is_ok())
                        .then_some(*data)
                });
            }
            selected
        };
        Ok(match selected {
            Selected::Value(value) => {
                // The slot owns the borrowed handle until this retain completes.
                // No callback or mutation occurs between selection and duplication.
                let borrowed = JsValue::from_raw(value).ok_or(RuntimeError::Invariant(
                    "internal sentinel in ordinary data property",
                ))?;
                let value = self.dup_jsvalue(&borrowed)?;
                if let (Some(output), Some(data), JsValue::Object(function)) =
                    (native.as_mut(), native_data, &value)
                {
                    **output = Some(LinkedNativeSelection {
                        runtime: self.clone(),
                        function: *function,
                        data,
                    });
                }
                ReadProbe::Value(value)
            }
            Selected::Getter(get) => ReadProbe::Getter(
                get.map(|id| {
                    ObjectRef::from_borrowed_handle(self.clone(), id)
                        .map(crate::engine::object::CallableRef::from_validated_object)
                })
                .transpose()?,
            ),
            Selected::Missing(prototype) => ReadProbe::Missing(
                prototype
                    .map(|id| ObjectRef::from_borrowed_handle(self.clone(), id))
                    .transpose()?,
            ),
        })
    }
}

impl Runtime {
    pub(super) fn try_define_ordinary_value(
        &self,
        object: &ObjectRef,
        key: &PropertyKey,
        descriptor: &crate::engine::object::OrdinaryPropertyDescriptor,
    ) -> Result<Option<bool>, RuntimeError> {
        use crate::engine::object::DescriptorField;
        let DescriptorField::Present(value) = &descriptor.value else {
            return Ok(None);
        };
        if !matches!(descriptor.writable, DescriptorField::Absent)
            || !matches!(descriptor.enumerable, DescriptorField::Absent)
            || !matches!(descriptor.configurable, DescriptorField::Absent)
            || !matches!(descriptor.get, DescriptorField::Absent)
            || !matches!(descriptor.set, DescriptorField::Absent)
        {
            return Ok(None);
        }
        // Select before conversion: declined fast paths must not allocate nodes.
        let id = object.object_id();
        let slot = {
            let state = self.0.state.borrow();
            if !is_ordinary(state.heap.object(id)?) {
                return Ok(None);
            }
            let Some(slot) = locate(&state, id, key.atom())? else {
                return Ok(None);
            };
            if !matches!(
                state.heap.object(id)?.slots[slot.index],
                PropertySlot::Data(_)
            ) {
                return Ok(None);
            }
            slot
        };
        // Conversion can allocate, but cannot execute JS or mutate this layout.
        let converted = self.raw_property_value(value)?;
        let raw = converted.raw();
        let mut state = self.0.state.borrow_mut();
        let old = match state.heap.object(id) {
            Ok(data) => match &data.slots[slot.index] {
                PropertySlot::Data(old) => old,
                _ => {
                    drop(state);
                    return Ok(None);
                }
            },
            Err(error) => {
                drop(state);
                return Err(error.into());
            }
        };
        if !crate::engine::object::property::data_value_update_allowed(
            slot.flags.configurable,
            slot.flags.writable,
            old,
            &raw,
            |left, right| {
                crate::engine::value::collection_key::same_value(&state.heap, left, right)
            },
        ) {
            drop(state);
            return Ok(Some(false));
        }
        let replaced = replace_data(&mut state, id, slot, PropertySlot::Data(raw));
        drop(state);
        // The slot retained its own copy edge on success; a rejected update
        // kept nothing. The guard balances the producer edge either way.
        replaced?;
        Ok(Some(true))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::object::{DescriptorField, OrdinaryPropertyDescriptor};

    #[test]
    fn ordinary_read_duplicates_existing_string_node_without_materialization() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let Value::Object(object) = context.eval("({value:'payload'})").unwrap() else {
            unreachable!()
        };
        let key = runtime.intern_property_key("value").unwrap();
        let before = runtime.heap_counts().string_nodes;
        let first = runtime.ordinary_read_probe(&object, &key).unwrap();
        let second = runtime.ordinary_read_probe(&object, &key).unwrap();
        let (ReadProbe::Value(first), ReadProbe::Value(second)) = (first, second) else {
            unreachable!()
        };
        assert!(matches!((&first, &second), (JsValue::String(a), JsValue::String(b)) if a == b));
        assert_eq!(runtime.heap_counts().string_nodes, before);
        runtime.release_jsvalue(first).unwrap();
        runtime.release_jsvalue(second).unwrap();
    }

    #[test]
    fn ordinary_property_replacement_preserves_roots_and_rejects_foreign_values() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let object = runtime.new_object(None).unwrap();
        let old = runtime.new_object(None).unwrap();
        let key = runtime.intern_property_key("x").unwrap();
        runtime
            .define_own_property(
                &object,
                &key,
                &OrdinaryPropertyDescriptor {
                    value: DescriptorField::Present(Value::Object(old.clone())),
                    writable: DescriptorField::Present(true),
                    configurable: DescriptorField::Present(true),
                    ..OrdinaryPropertyDescriptor::new()
                },
            )
            .unwrap();
        assert_eq!(
            runtime
                .0
                .state
                .borrow()
                .heap
                .object_strong_count(old.object_id()),
            Ok(2)
        );
        assert!(context.set_property(&object, &key, Value::Int(42)).unwrap());
        assert_eq!(
            runtime
                .0
                .state
                .borrow()
                .heap
                .object_strong_count(old.object_id()),
            Ok(1)
        );
        let foreign = Runtime::new().new_object(None).unwrap();
        assert!(
            context
                .set_property(&object, &key, Value::Object(foreign))
                .is_err()
        );
        assert_eq!(context.get_property(&object, &key).unwrap(), Value::Int(42));
    }

    #[test]
    fn ordinary_property_own_write_stops_before_revoked_proxy_prototype() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        assert_eq!(
            context
                .eval(
                    r#"
            var rev = Proxy.revocable({}, {});
            var obj = Object.create(rev.proxy);
            Object.defineProperty(obj, 'x', {value: 1, writable: true});
            rev.revoke();
            obj.x = 42;
            obj.x;
        "#
                )
                .unwrap(),
            Value::Int(42)
        );
    }
}

#[cfg(test)]
mod dense_set_tests {
    use super::*;
    use crate::engine::object::operations::PropertySetAction;
    use crate::engine::object::ordinary::SetStep;

    #[test]
    fn existing_dense_set_preserves_hole_prototype_descriptor_and_receiver_rules() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        assert_eq!(
            context
                .eval(
                    r#"(()=>{
            let calls=0, seen, a=[1,2];
            const proto=Object.create(Array.prototype);
            Object.defineProperty(proto,'0',{set(v){calls++;seen=[this,v]},configurable:true});
            Object.setPrototypeOf(a,proto);
            a[0]=7;
            if(a[0]!==7 || calls!==0)return false;
            delete a[0]; a[0]=8;
            if(calls!==1 || seen[0]!==a || seen[1]!==8 || Object.hasOwn(a,'0'))return false;
            let b=[1]; Object.defineProperty(b,'length',{writable:false}); b[0]=9;
            if(b[0]!==9 || Reflect.set(b,'1',2))return false;
            Object.freeze(b); if(Reflect.set(b,'0',10) || b[0]!==9)return false;
            let c=[3]; Object.defineProperty(c,'0',{set(v){calls++;seen=v},get(){return 4}});
            c[0]=11; if(c[0]!==4 || seen!==11)return false;
            let d=[5], receiver={};
            if(!Reflect.set(d,'0',12,receiver) || d[0]!==5 || receiver[0]!==12)return false;
            let marker={}, trace='';
            let proxy=new Proxy({}, {getOwnPropertyDescriptor(){trace+='d';throw marker}});
            try{Reflect.set(d,'0',13,proxy);return false}catch(e){if(e!==marker)return false}
            return trace==='d' && d[0]===5;
        })()"#
                )
                .unwrap(),
            Value::Bool(true)
        );
    }

    #[test]
    fn existing_dense_set_finishes_without_waiting_and_releases_last_owners() {
        let runtime = Runtime::new();
        let weak = std::rc::Rc::downgrade(&runtime.0);
        let mut context = runtime.new_context();
        let Value::Object(array) = context.eval("[{}]").unwrap() else {
            panic!("expected array")
        };
        let array_id = array.object_id();
        let key = runtime.intern_property_key("0").unwrap();
        let ReadProbe::Value(JsValue::Object(old)) =
            runtime.ordinary_read_probe(&array, &key).unwrap()
        else {
            panic!("expected old value")
        };
        let old_id = old;
        runtime.release_jsvalue(JsValue::Object(old)).unwrap();
        let replacement = runtime.new_object(None).unwrap();
        let replacement_id = replacement.object_id();
        let released = runtime.new_object(None).unwrap();
        let released_id = released.object_id();
        let operation = runtime.operation();
        {
            let _borrow = runtime.0.state.borrow();
            drop(released);
        }
        assert!(runtime.0.deferred_references.has_pending());
        let action = SetStep::start_into(
            &runtime,
            Some(context.realm),
            array.clone(),
            key.clone(),
            runtime.into_jsvalue(Value::Object(replacement)).unwrap(),
            runtime.into_jsvalue(Value::Object(array.clone())).unwrap(),
            |_| panic!("dense overwrite suspended"),
        )
        .unwrap();
        assert!(matches!(action, Some(PropertySetAction::Complete)));
        drop(operation);
        assert!(!runtime.0.deferred_references.has_pending());
        runtime.run_gc().unwrap();
        for id in [old_id, released_id] {
            assert!(runtime.0.state.borrow().heap.object(id).is_err());
        }
        assert!(runtime.0.state.borrow().heap.object(replacement_id).is_ok());
        let other = Runtime::new();
        let foreign = other.new_object(None).unwrap();
        assert!(matches!(
            runtime.prepare_set_property_with_receiver_in_realm(
                Some(context.realm),
                &array,
                &key,
                Value::Object(foreign),
                Value::Object(array.clone()),
            ),
            Err(RuntimeError::WrongRuntime(_))
        ));
        let ReadProbe::Value(JsValue::Object(current)) =
            runtime.ordinary_read_probe(&array, &key).unwrap()
        else {
            panic!("replacement disappeared")
        };
        assert_eq!(current, replacement_id);
        runtime.release_jsvalue(JsValue::Object(current)).unwrap();
        drop(array);
        drop(key);
        runtime.run_gc().unwrap();
        for id in [array_id, replacement_id] {
            assert!(runtime.0.state.borrow().heap.object(id).is_err());
        }
        drop(context);
        drop(runtime);
        assert!(weak.upgrade().is_none());
    }
}

fn immediate_value(raw: &crate::engine::heap::RawValue) -> Option<Value> {
    use crate::engine::heap::RawValue;
    Some(match raw {
        RawValue::Undefined => Value::Undefined,
        RawValue::Null => Value::Null,
        RawValue::Bool(value) => Value::Bool(*value),
        RawValue::Int(value) => Value::Int(*value),
        RawValue::Float(value) => Value::Float(*value),
        RawValue::ShortBigInt(value) => {
            Value::BigInt(crate::engine::value::bigint::JsBigInt::from(*value))
        }
        _ => return None,
    })
}

/// Scalar projection for an immediate leaf result: the returned internal
/// value owns no heap edge, matching [`immediate_value`].
fn immediate_value_jsvalue(raw: &crate::engine::heap::RawValue) -> Option<JsValue> {
    use crate::engine::heap::RawValue;
    Some(match raw {
        RawValue::Undefined => JsValue::Undefined,
        RawValue::Null => JsValue::Null,
        RawValue::Bool(value) => JsValue::Bool(*value),
        RawValue::Int(value) => JsValue::Int(*value),
        RawValue::Float(value) => JsValue::Float(*value),
        RawValue::ShortBigInt(value) => JsValue::ShortBigInt(*value),
        _ => return None,
    })
}

fn linked_field_atom(
    runtime: &Runtime,
    executable: &crate::engine::code::runtime::PublishedFunctionSnapshot,
    index: u32,
) -> Option<Atom> {
    if !executable.belongs_to(runtime) {
        return None;
    }
    executable
        .property_key_atoms
        .as_ref()?
        .get(index as usize)
        .copied()
        .filter(|atom| !atom.is_null())
}

impl Runtime {
    /// A published same-domain bytecode root owns this linked static atom.
    /// Own-slot classification and projection share the ordinary read kernel;
    /// every decline leaves input owners, lazy properties and prototypes alone.
    #[cfg(test)]
    pub(crate) fn try_ordinary_field_immediate_read(
        &self,
        base: &JsValue,
        executable: &crate::engine::code::runtime::PublishedFunctionSnapshot,
        index: u32,
    ) -> Option<JsValue> {
        use crate::engine::heap::SlotReleaseReadiness;
        let atom = linked_field_atom(self, executable, index)?;
        if !matches!(
            self.slot_value_release_readiness_jsvalue(base),
            Ok(SlotReleaseReadiness::Ready)
        ) {
            return None;
        }
        let state = self.0.state.try_borrow().ok()?;
        immediate_field_in_state(&state, base, atom)
    }
}

#[cfg(test)]
fn immediate_field_in_state(state: &RuntimeState, base: &JsValue, atom: Atom) -> Option<JsValue> {
    field_in_state(state, base, atom, immediate_value_jsvalue)
}

/// Select an ordinary own data slot once, including at megamorphic sites.
/// The caller promotes the borrowed slot while this same heap borrow is live.
/// It has already proved whether consuming `base` can drain storage. Missing,
/// accessor, lazy and exotic reads still take the canonical driver path.
fn field_in_state(
    state: &RuntimeState,
    base: &JsValue,
    atom: Atom,
    promote: impl FnOnce(&RawValue) -> Option<JsValue>,
) -> Option<JsValue> {
    if let JsValue::String(id) = base {
        // The executable owns a same-runtime interned atom; the pinned
        // spelling has that same canonical identity. No text traversal is
        // needed for each primitive string length read.
        if atom
            != state
                .pinned_atoms
                .get(crate::engine::atom::pinned::PinnedAtom::Length)
        {
            return None;
        }
        let length = state.heap.string_fast(*id).len();
        return Some(if let Ok(length) = i32::try_from(length) {
            JsValue::Int(length)
        } else {
            JsValue::Float(length as f64)
        });
    }
    let JsValue::Object(object) = base else {
        return None;
    };
    let id = *object;
    let data = state.heap.object(id).ok()?;
    if matches!(
        (data.kind, &data.payload),
        (ObjectKind::Array, ObjectPayload::Array { .. })
    ) {
        let first = state.heap.shape(data.shape).ok()?.entries().first()?;
        if first.atom == AtomIdx::from_raw(atom.raw()) {
            let (length, _) = Runtime::array_length_state_in_heap(&state.heap, id, atom).ok()??;
            return Some(if let Ok(length) = i32::try_from(length) {
                JsValue::Int(length)
            } else {
                JsValue::Float(f64::from(length))
            });
        }
        return None;
    }
    if !is_ordinary(data) {
        return None;
    }
    let slot = locate(state, id, atom).ok()??;
    let PropertySlot::Data(value) = &data.slots[slot.index] else {
        return None;
    };
    promote(value)
}

impl Runtime {
    /// A published function already owns its static key. Only the selected
    /// result/getter is promoted here; fallback will acquire an owning key.
    #[cfg(test)]
    pub(crate) fn prepare_linked_own_read(
        &self,
        base: &Value,
        executable: &crate::engine::code::runtime::PublishedFunctionSnapshot,
        index: u32,
    ) -> Result<Option<crate::engine::object::OrdinaryRead>, RuntimeError> {
        let internal = self.unroot_value(base)?;
        let result = self.prepare_linked_own_read_selected(&internal, executable, index, None);
        self.release_jsvalue(internal)?;
        result
    }

    pub(crate) fn prepare_linked_own_read_selected(
        &self,
        base: &JsValue,
        executable: &crate::engine::code::runtime::PublishedFunctionSnapshot,
        index: u32,
        native: Option<&mut Option<LinkedNativeSelection>>,
    ) -> Result<Option<crate::engine::object::OrdinaryRead>, RuntimeError> {
        let Some(atom) = linked_field_atom(self, executable, index) else {
            return Ok(None);
        };
        let JsValue::Object(object) = base else {
            return Ok(None);
        };
        let _operation = self.operation();
        // The borrowed base already pins this object until probing finishes.
        Ok(
            match self.ordinary_read_probe_atom(*object, atom, true, native)? {
                ReadProbe::Value(value) => {
                    Some(crate::engine::object::OrdinaryRead::Complete(Some(value)))
                }
                ReadProbe::Getter(None) => Some(crate::engine::object::OrdinaryRead::Complete(
                    Some(JsValue::Undefined),
                )),
                ReadProbe::Getter(Some(getter)) => {
                    let receiver = self.dup_jsvalue(base)?;
                    #[cfg(feature = "profiling")]
                    crate::engine::api::profiling::record_owned_execution_event(
                        "linked_read_owner_clone.ReceiverObject",
                    );
                    Some(crate::engine::object::OrdinaryRead::Call { getter, receiver })
                }
                ReadProbe::Missing(_) | ReadProbe::Special(_) => None,
            },
        )
    }
    /// Only an existing writable own scalar slot reaches the ordinary Set
    /// replacement transaction. There are no callback or owner-bearing edges.
    #[cfg(test)]
    pub(crate) fn try_ordinary_field_immediate_write(
        &self,
        base: &JsValue,
        executable: &crate::engine::code::runtime::PublishedFunctionSnapshot,
        index: u32,
        value: &JsValue,
    ) -> bool {
        use crate::engine::heap::SlotReleaseReadiness;
        if !matches!(
            value,
            JsValue::Undefined
                | JsValue::Null
                | JsValue::Bool(_)
                | JsValue::Int(_)
                | JsValue::Float(_)
                | JsValue::ShortBigInt(_)
        ) {
            return false;
        }
        let Some(atom) = linked_field_atom(self, executable, index) else {
            return false;
        };
        let JsValue::Object(object) = base else {
            return false;
        };
        if !matches!(
            self.slot_value_release_readiness_jsvalue(base),
            Ok(SlotReleaseReadiness::Ready)
        ) {
            return false;
        }
        let Ok(mut state) = self.0.state.try_borrow_mut() else {
            return false;
        };
        let id = *object;
        let Ok(data) = state.heap.object(id) else {
            return false;
        };
        if !is_ordinary(data) {
            return false;
        }
        let Ok(Some(slot)) = locate(&state, id, atom) else {
            return false;
        };
        if !slot.flags.writable {
            return false;
        }
        let PropertySlot::Data(old) = &data.slots[slot.index] else {
            return false;
        };
        if immediate_value(old).is_none() {
            return false;
        }
        // `value` was matched to a scalar above, so this id copy allocates
        // nothing and never takes the state borrow the caller still holds.
        let raw = value.as_raw();
        // The canonical transaction validates shape storage before publication.
        // With scalar old/new values it retains/releases no edges or atoms;
        // the already-empty zero queue makes post-commit cleanup infallible.
        replace_data(&mut state, id, slot, PropertySlot::Data(raw)).is_ok()
    }
}

impl Runtime {
    /// Read an own Number from a genuine Array while its base owner remains in
    /// the frame. Dense elements and materialized data slots qualify; read
    /// semantics do not depend on a data property's attribute flags. Holes and
    /// accessors fall back to canonical [[Get]].
    /// The heap borrow ends before the Copy result leaves.
    pub(crate) fn peek_dense_number(&self, base: &JsValue, index: u32) -> Option<Number> {
        let JsValue::Object(id) = base else {
            return None;
        };
        let state = self.0.state.try_borrow().ok()?;
        let data = state.heap.object(*id).ok()?;
        if !matches!(data.kind, ObjectKind::Array) {
            return None;
        }
        match &data.payload {
            ObjectPayload::Array { dense: Some(dense) } => match dense.get(index as usize)? {
                RawValue::Int(value) => Some(Number::Int(*value)),
                RawValue::Float(value) => Some(Number::Float(*value)),
                _ => None,
            },
            ObjectPayload::Array { dense: None } => {
                materialized_array_own_number(&state, data, index)
            }
            _ => None,
        }
    }

    /// Diagnose a *previously failed* numeric dense read. This performs an
    /// extra heap borrow only in profiling builds and never changes storage.
    #[cfg(feature = "profiling")]
    pub(crate) fn diagnose_dense_number_read_miss(
        &self,
        base: &JsValue,
        index: u32,
    ) -> &'static str {
        if !matches!(base, JsValue::Object(_)) {
            return "base_not_object";
        }
        let Ok(state) = self.0.state.try_borrow() else {
            return "read_borrow_unavailable";
        };
        dense_number_miss_in_state(&state, base, index)
    }

    /// Diagnose a *previously failed* numeric dense write. The mutable
    /// borrow probe distinguishes storage readiness from an active lease.
    #[cfg(feature = "profiling")]
    pub(crate) fn diagnose_dense_number_write_miss(
        &self,
        base: &JsValue,
        index: u32,
    ) -> &'static str {
        if !matches!(base, JsValue::Object(_)) {
            return "base_not_object";
        }
        let Ok(state) = self.0.state.try_borrow_mut() else {
            return "write_borrow_unavailable";
        };
        dense_number_miss_in_state(&state, base, index)
    }

    /// Replace one existing own dense Number under a single heap borrow. A
    /// dense element has the default writable data descriptor; descriptor
    /// changes materialize the Array and make this leaf decline. Both the old
    /// and new values are immediate, so this cannot release an owner, drain
    /// cleanup, change layout or length, or invoke user code.
    #[inline]
    pub(crate) fn try_write_dense_number(&self, base: &JsValue, index: u32, value: Number) -> bool {
        let JsValue::Object(id) = base else {
            return false;
        };
        let Ok(mut state) = self.0.state.try_borrow_mut() else {
            return false;
        };
        let replacement = match value {
            Number::Int(number) => RawValue::Int(number),
            Number::Float(number) => RawValue::Float(number),
        };
        state
            .heap
            .try_replace_dense_number_value(*id, index, replacement)
    }

    /// Borrow an existing dense own immediate value while proving that the VM
    /// can subsequently release its base operand without draining heap work.
    /// Every decline leaves owners and storage untouched; the general property
    /// lookup retains all missing/exotic/reference-valued cases.
    #[cfg(test)]
    pub(crate) fn try_dense_array_immediate_read(
        &self,
        base: &JsValue,
        index: u32,
    ) -> Option<JsValue> {
        self.try_array_immediate_read_kind(base, index, false)
    }

    /// One release proof and heap borrow select either existing array kernel.
    pub(crate) fn try_array_immediate_read(&self, base: &JsValue, index: u32) -> Option<JsValue> {
        self.try_array_immediate_read_kind(base, index, true)
    }

    fn try_array_immediate_read_kind(
        &self,
        base: &JsValue,
        index: u32,
        include_typed: bool,
    ) -> Option<JsValue> {
        use crate::engine::heap::SlotReleaseReadiness;
        let JsValue::Object(object) = base else {
            return None;
        };
        if !matches!(
            self.slot_value_release_readiness_jsvalue(base),
            Ok(SlotReleaseReadiness::Ready)
        ) {
            return None;
        }
        let mut state = self.0.state.try_borrow_mut().ok()?;
        let data = state.heap.object(*object).ok()?;
        if matches!(data.kind, ObjectKind::Array) {
            match &data.payload {
                ObjectPayload::Array { dense: Some(dense) } => {
                    return immediate_value_jsvalue(dense.get(index as usize)?);
                }
                ObjectPayload::Array { dense: None } => {
                    return match materialized_array_own_number(&state, data, index)? {
                        Number::Int(value) => Some(JsValue::Int(value)),
                        Number::Float(value) => Some(JsValue::Float(value)),
                    };
                }
                _ => {}
            }
        }
        if !include_typed {
            return None;
        }
        if matches!(data.payload, ObjectPayload::Arguments { .. }) {
            let atom = Atom::from_immediate_integer(index)?;
            let slot = locate(&state, *object, atom).ok()??;
            return match &data.slots[slot.index] {
                PropertySlot::Data(value) => immediate_value_jsvalue(value),
                PropertySlot::VarRef(cell) => {
                    immediate_value_jsvalue(&state.heap.var_ref(*cell).ok()?.value)
                }
                PropertySlot::Accessor { .. } | PropertySlot::AutoInit(_) => None,
            };
        }
        let value = Self::typed_array_number_read_in_heap(&mut state.heap, *object, index)?;
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_execution_event("typed_array_number_read_leaf");
        Some(value)
    }
}

/// Report the first currently observable guard failure in the same order as
/// the dense leaf. A later canonical property operation can have additional
/// semantics (prototype, accessor, etc.); this label is not a unique cause.
#[cfg(feature = "profiling")]
fn dense_number_miss_in_state(state: &RuntimeState, base: &JsValue, index: u32) -> &'static str {
    let JsValue::Object(id) = base else {
        return "base_not_object";
    };
    let Ok(data) = state.heap.object(*id) else {
        return "object_unavailable";
    };
    if !matches!(data.kind, ObjectKind::Array) {
        return "not_array";
    }
    let ObjectPayload::Array { dense } = &data.payload else {
        return "array_payload_unavailable";
    };
    let Some(dense) = dense else {
        return materialized_number_miss_in_state(state, *id, index);
    };
    let Some(index) = usize::try_from(index).ok() else {
        return "index_unrepresentable";
    };
    let Some(value) = dense.get(index) else {
        let length = match data.slots.first() {
            Some(PropertySlot::Data(RawValue::Int(length))) if *length >= 0 => Some(*length as u32),
            Some(PropertySlot::Data(RawValue::Float(length)))
                if length.is_finite()
                    && *length >= 0.0
                    && *length <= f64::from(u32::MAX)
                    && length.fract() == 0.0 =>
            {
                Some(*length as u32)
            }
            _ => None,
        };
        return match length {
            Some(length) if (index as u64) < u64::from(length) => "outside_dense_prefix_in_length",
            Some(_) => "beyond_array_length",
            None => "array_length_unavailable",
        };
    };
    if matches!(value, RawValue::Int(_) | RawValue::Float(_)) {
        "dense_ready_after_failure"
    } else {
        "dense_non_number"
    }
}

/// Refine the representation failure without invoking [[Get]], walking the
/// prototype chain, interning a key, or retaining a value. These are observed
/// own-slot states, not claims about why the whole Array stayed materialized.
#[cfg(feature = "profiling")]
fn materialized_number_miss_in_state(
    state: &RuntimeState,
    object: ObjectId,
    index: u32,
) -> &'static str {
    let Some(atom) = Atom::from_immediate_integer(index) else {
        return "array_materialized.index_not_immediate";
    };
    let slot = match locate(state, object, atom) {
        Ok(Some(slot)) => slot,
        Ok(None) => return "array_materialized.missing_own_index",
        Err(_) => return "array_materialized.layout_unavailable",
    };
    let Ok(data) = state.heap.object(object) else {
        return "array_materialized.layout_unavailable";
    };
    match &data.slots[slot.index] {
        PropertySlot::Accessor { .. } => "array_materialized.own_accessor",
        PropertySlot::Data(_) if slot.flags != PropertyFlags::data(true, true, true) => {
            "array_materialized.own_nondefault_descriptor"
        }
        PropertySlot::Data(RawValue::Int(_) | RawValue::Float(_)) => {
            "array_materialized.own_default_number"
        }
        PropertySlot::Data(_) => "array_materialized.own_non_number",
        PropertySlot::VarRef(_) | PropertySlot::AutoInit(_) => {
            "array_materialized.own_special_slot"
        }
    }
}

#[cfg(test)]
mod dense_array_read_tests {
    use super::*;

    #[cfg(feature = "profiling")]
    #[test]
    fn dense_diagnostic_priority_reports_nonobject_before_borrow() {
        let runtime = Runtime::new();
        let base = JsValue::Int(3);
        let _lease = runtime.0.state.borrow_mut();
        assert!(runtime.peek_dense_number(&base, 0).is_none());
        assert!(!runtime.try_write_dense_number(&base, 0, Number::Int(1)));
        assert_eq!(
            runtime.diagnose_dense_number_read_miss(&base, 0),
            "base_not_object"
        );
        assert_eq!(
            runtime.diagnose_dense_number_write_miss(&base, 0),
            "base_not_object"
        );
    }

    fn receiver(runtime: &Runtime, expression: &str) -> Value {
        let mut context = runtime.new_context();
        let value = context.eval(expression).unwrap();
        drop(context);
        runtime.run_gc().unwrap();
        value
    }

    #[test]
    fn dense_number_peek_reads_only_existing_numeric_array_elements() {
        let runtime = Runtime::new();
        let base = runtime
            .into_jsvalue(receiver(&runtime, "[7, -0, NaN, 'x']"))
            .unwrap();
        let JsValue::Object(id) = &base else {
            panic!("array");
        };
        let count = runtime.0.state.borrow().heap.object_strong_count(*id);
        assert!(matches!(
            runtime.peek_dense_number(&base, 0),
            Some(Number::Int(7))
        ));
        assert!(
            matches!(runtime.peek_dense_number(&base, 1), Some(Number::Float(n)) if n == 0.0 && n.is_sign_negative())
        );
        assert!(
            matches!(runtime.peek_dense_number(&base, 2), Some(Number::Float(n)) if n.is_nan())
        );
        for index in [3, 4, 5, u32::MAX] {
            assert!(runtime.peek_dense_number(&base, index).is_none());
        }
        assert_eq!(
            runtime.0.state.borrow().heap.object_strong_count(*id),
            count
        );
        {
            let _borrow = runtime.0.state.borrow_mut();
            assert!(runtime.peek_dense_number(&base, 0).is_none());
        }
        runtime.release_jsvalue(base).unwrap();
    }

    #[test]
    fn materialized_array_own_numbers_use_the_shared_read_leaf() {
        let runtime = Runtime::new();
        let base = runtime
            .into_jsvalue(receiver(
                &runtime,
                "(function(){let a=[];a[5]=5;a[2]=-0;a[4]=NaN;return a})()",
            ))
            .unwrap();
        let JsValue::Object(id) = &base else {
            panic!("array");
        };
        // Canonical GetArrayEl consumes its base, so its read leaf must still
        // decline the final owner. The borrowed numeric span need not do so.
        assert!(runtime.try_array_immediate_read(&base, 5).is_none());
        let keeper = runtime.dup_jsvalue(&base).unwrap();
        let before = runtime.0.state.borrow().heap.object_strong_count(*id);
        assert!(matches!(
            runtime.peek_dense_number(&base, 5),
            Some(Number::Int(5))
        ));
        assert!(matches!(
            runtime.peek_dense_number(&base, 2),
            Some(Number::Float(value)) if value == 0.0 && value.is_sign_negative()
        ));
        assert!(matches!(
            runtime.peek_dense_number(&base, 4),
            Some(Number::Float(value)) if value.is_nan()
        ));
        assert!(matches!(
            runtime.try_array_immediate_read(&base, 5),
            Some(JsValue::Int(5))
        ));
        assert!(matches!(
            runtime.try_array_immediate_read(&base, 2),
            Some(JsValue::Float(value)) if value == 0.0 && value.is_sign_negative()
        ));
        assert!(runtime.peek_dense_number(&base, 3).is_none());
        assert!(runtime.try_array_immediate_read(&base, 3).is_none());
        assert!(runtime.peek_dense_number(&base, u32::MAX).is_none());
        assert_eq!(
            runtime.0.state.borrow().heap.object_strong_count(*id),
            before
        );
        runtime.release_jsvalue(base).unwrap();
        runtime.release_jsvalue(keeper).unwrap();
    }

    #[test]
    fn materialized_array_read_checks_each_own_descriptor_and_hole() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let base = runtime
            .into_jsvalue(
                context
                    .eval(
                        "globalThis.readEffects=0;globalThis.readArray=(function(){\
                         let a=[3,4];Object.defineProperty(a,'0',{writable:false});\
                         Object.setPrototypeOf(a,{get 1(){readEffects++;return 19}});return a\
                         })();readArray",
                    )
                    .unwrap(),
            )
            .unwrap();
        assert!(matches!(
            runtime.peek_dense_number(&base, 0),
            Some(Number::Int(3))
        ));
        assert!(matches!(
            runtime.try_array_immediate_read(&base, 0),
            Some(JsValue::Int(3))
        ));
        assert!(matches!(
            runtime.peek_dense_number(&base, 1),
            Some(Number::Int(4))
        ));
        assert!(matches!(
            runtime.try_array_immediate_read(&base, 1),
            Some(JsValue::Int(4))
        ));
        assert_eq!(context.eval("readEffects").unwrap(), Value::Int(0));
        assert_eq!(
            context.eval("delete readArray[1]").unwrap(),
            Value::Bool(true)
        );
        assert!(runtime.peek_dense_number(&base, 1).is_none());
        assert!(runtime.try_array_immediate_read(&base, 1).is_none());
        assert_eq!(context.eval("readArray[1]").unwrap(), Value::Int(19));
        assert_eq!(context.eval("readEffects").unwrap(), Value::Int(1));
        runtime.release_jsvalue(base).unwrap();
    }

    #[test]
    fn materialized_array_read_ignores_data_attributes_but_writes_still_decline() {
        let runtime = Runtime::new();
        for source in [
            "Object.freeze([7])",
            "Object.defineProperty([7], '0', {writable:false,enumerable:false})",
            "Object.defineProperty([7], '0', {configurable:false})",
        ] {
            let base = runtime.into_jsvalue(receiver(&runtime, source)).unwrap();
            assert!(matches!(
                runtime.peek_dense_number(&base, 0),
                Some(Number::Int(7))
            ));
            assert!(!runtime.try_write_dense_number(&base, 0, Number::Int(9)));
            runtime.release_jsvalue(base).unwrap();
        }
    }

    #[test]
    fn dense_number_peek_declines_slow_exotic_and_foreign_receivers() {
        for expression in [
            "[, 1]",
            "Object.defineProperty([1], '0', {get(){throw 71}})",
            "new Proxy([1], {get(){throw 72}})",
            "new Uint8Array([1])",
            "({0:1,length:1})",
        ] {
            let runtime = Runtime::new();
            let base = runtime
                .into_jsvalue(receiver(&runtime, expression))
                .unwrap();
            assert!(
                runtime.peek_dense_number(&base, 0).is_none(),
                "{expression}"
            );
            runtime.release_jsvalue(base).unwrap();
        }
        let runtime = Runtime::new();
        let base = runtime.into_jsvalue(receiver(&runtime, "[1]")).unwrap();
        assert!(Runtime::new().peek_dense_number(&base, 0).is_none());
        assert!(runtime.peek_dense_number(&JsValue::Int(1), 0).is_none());
        runtime.release_jsvalue(base).unwrap();
    }

    #[cfg(feature = "profiling")]
    #[test]
    fn reverse_fill_recovers_dense_number_reads_and_writes() {
        let runtime = Runtime::new();
        let sequential = runtime
            .into_jsvalue(receiver(
                &runtime,
                "(function(){let a=[];for(let i=0;i<4;i++)a[i]=i;return a})()",
            ))
            .unwrap();
        let reverse = runtime
            .into_jsvalue(receiver(
                &runtime,
                "(function(){let a=[];for(let i=3;i>=0;i--)a[i]=i;return a})()",
            ))
            .unwrap();
        assert!(matches!(
            runtime.peek_dense_number(&sequential, 3),
            Some(Number::Int(3))
        ));
        assert!(matches!(
            runtime.peek_dense_number(&reverse, 3),
            Some(Number::Int(3))
        ));
        assert!(runtime.try_write_dense_number(&reverse, 3, Number::Int(7)));
        assert!(matches!(
            runtime.peek_dense_number(&reverse, 3),
            Some(Number::Int(7))
        ));
        runtime.release_jsvalue(sequential).unwrap();
        runtime.release_jsvalue(reverse).unwrap();
    }

    #[cfg(feature = "profiling")]
    #[test]
    fn materialized_miss_probe_observes_own_slots_without_getters_or_owners() {
        for (source, index, suffix) in [
            (
                "(function(){let a=[];a[3]=3;a[1]=1;return a})()",
                2,
                "missing_own_index",
            ),
            (
                "Object.defineProperty([1], '0', {get(){throw 71}})",
                0,
                "own_accessor",
            ),
            (
                "(function(){let a=[];a[3]=3;a[1]={};return a})()",
                1,
                "own_non_number",
            ),
            (
                "(function(){let a=[];a[2147483648]=1;return a})()",
                2147483648,
                "index_not_immediate",
            ),
        ] {
            let runtime = Runtime::new();
            let base = runtime.into_jsvalue(receiver(&runtime, source)).unwrap();
            let JsValue::Object(id) = &base else {
                panic!("Array expected")
            };
            let before = runtime.0.state.borrow().heap.object_strong_count(*id);
            assert!(runtime.peek_dense_number(&base, index).is_none());
            let expected = format!("array_materialized.{suffix}");
            assert_eq!(
                runtime.diagnose_dense_number_read_miss(&base, index),
                expected,
                "{source}"
            );
            assert_eq!(
                runtime.diagnose_dense_number_write_miss(&base, index),
                expected,
                "{source}"
            );
            assert_eq!(
                runtime.0.state.borrow().heap.object_strong_count(*id),
                before
            );
            runtime.release_jsvalue(base).unwrap();
        }

        let runtime = Runtime::new();
        let base = runtime
            .into_jsvalue(receiver(
                &runtime,
                "(function(){let a=[];a[3]=3;a[1]=1;return a})()",
            ))
            .unwrap();
        assert!(matches!(
            runtime.peek_dense_number(&base, 1),
            Some(Number::Int(1))
        ));
        // Writes still require dense storage, so the write diagnostic keeps
        // describing this materialized own slot as a write miss.
        assert_eq!(
            runtime.diagnose_dense_number_write_miss(&base, 1),
            "array_materialized.own_default_number"
        );
        runtime.release_jsvalue(base).unwrap();
    }

    #[cfg(feature = "profiling")]
    #[test]
    fn dense_miss_probe_distinguishes_length_and_element_class() {
        for (source, index, reason) in [
            ("new Array(4)", 2, "outside_dense_prefix_in_length"),
            ("[1]", 2, "beyond_array_length"),
            ("['x']", 0, "dense_non_number"),
            ("({0:1})", 0, "not_array"),
        ] {
            let runtime = Runtime::new();
            let base = runtime.into_jsvalue(receiver(&runtime, source)).unwrap();
            assert!(runtime.peek_dense_number(&base, index).is_none());
            assert_eq!(
                runtime.diagnose_dense_number_read_miss(&base, index),
                reason,
                "{source}"
            );
            runtime.release_jsvalue(base).unwrap();
        }
    }

    #[test]
    fn dense_number_write_overwrites_existing_elements_without_changing_layout_or_owners() {
        let runtime = Runtime::new();
        let base = runtime
            .into_jsvalue(receiver(&runtime, "[7, -0, NaN]"))
            .unwrap();
        let JsValue::Object(id) = &base else {
            panic!("array");
        };
        let (shape, length, strong_count) = {
            let state = runtime.0.state.borrow();
            let data = state.heap.object(*id).unwrap();
            (
                data.shape,
                data.slots[0].clone(),
                state.heap.object_strong_count(*id),
            )
        };
        assert!(runtime.try_write_dense_number(&base, 0, Number::Float(-0.0)));
        assert!(runtime.try_write_dense_number(&base, 1, Number::Int(23)));
        assert!(runtime.try_write_dense_number(&base, 2, Number::Float(f64::NAN)));
        assert!(
            matches!(runtime.peek_dense_number(&base, 0), Some(Number::Float(n)) if n == 0.0 && n.is_sign_negative())
        );
        assert!(matches!(
            runtime.peek_dense_number(&base, 1),
            Some(Number::Int(23))
        ));
        assert!(
            matches!(runtime.peek_dense_number(&base, 2), Some(Number::Float(n)) if n.is_nan())
        );
        let state = runtime.0.state.borrow();
        let data = state.heap.object(*id).unwrap();
        assert_eq!(data.shape, shape);
        assert!(
            matches!((&data.slots[0], &length), (PropertySlot::Data(RawValue::Int(a)), PropertySlot::Data(RawValue::Int(b))) if a == b)
        );
        assert_eq!(state.heap.object_strong_count(*id), strong_count);
        drop(state);
        runtime.release_jsvalue(base).unwrap();
    }

    #[test]
    fn dense_number_write_allows_readonly_length_and_same_array_source() {
        let runtime = Runtime::new();
        let base = runtime
            .into_jsvalue(receiver(
                &runtime,
                "(function(){let a=[3,4];Object.defineProperty(a,'length',{writable:false});return a})()",
            ))
            .unwrap();
        let copied = runtime.peek_dense_number(&base, 0).expect("dense source");
        assert!(runtime.try_write_dense_number(&base, 1, copied));
        assert!(matches!(
            runtime.peek_dense_number(&base, 1),
            Some(Number::Int(3))
        ));
        runtime.release_jsvalue(base).unwrap();
    }

    #[test]
    fn dense_number_write_uses_own_element_before_prototype_setter() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let base = runtime
            .into_jsvalue(
                context
                    .eval("var writes=0;var a=[1];Object.setPrototypeOf(a,{set 0(v){writes++}});a")
                    .unwrap(),
            )
            .unwrap();
        assert!(runtime.try_write_dense_number(&base, 0, Number::Int(9)));
        assert!(matches!(
            runtime.peek_dense_number(&base, 0),
            Some(Number::Int(9))
        ));
        assert_eq!(context.eval("writes").unwrap(), Value::Int(0));
        runtime.release_jsvalue(base).unwrap();
    }

    #[test]
    fn dense_number_write_declines_before_changing_nonwritable_or_non_dense_receivers() {
        for (expression, index) in [
            ("[,'hole']", 0),
            ("[1, 'text']", 1),
            ("Object.defineProperty([1], '0', {writable:false})", 0),
            ("Object.defineProperty([1], '0', {get(){throw 71}})", 0),
            ("Object.freeze([1])", 0),
            ("Object.seal([1])", 0),
            ("new Proxy([1], {set(){throw 72}})", 0),
            ("new Uint8Array([1])", 0),
            ("({0:1,length:1})", 0),
        ] {
            let runtime = Runtime::new();
            let base = runtime
                .into_jsvalue(receiver(&runtime, expression))
                .unwrap();
            assert!(
                !runtime.try_write_dense_number(&base, index, Number::Int(42)),
                "{expression}"
            );
            runtime.release_jsvalue(base).unwrap();
        }
        let runtime = Runtime::new();
        let base = runtime.into_jsvalue(receiver(&runtime, "[1]")).unwrap();
        for index in [1, 2, u32::MAX] {
            assert!(!runtime.try_write_dense_number(&base, index, Number::Int(42)));
        }
        assert!(!runtime.try_write_dense_number(&JsValue::Int(1), 0, Number::Int(42)));
        assert!(!Runtime::new().try_write_dense_number(&base, 0, Number::Int(42)));
        {
            let _borrow = runtime.0.state.borrow();
            assert!(!runtime.try_write_dense_number(&base, 0, Number::Int(42)));
        }
        assert!(matches!(
            runtime.peek_dense_number(&base, 0),
            Some(Number::Int(1))
        ));
        runtime.release_jsvalue(base).unwrap();
    }

    #[test]
    fn dense_array_read_leaf_reads_scalars_without_changing_owners() {
        let runtime = Runtime::new();
        let base = runtime
            .into_jsvalue(receiver(&runtime, "[undefined,null,true,17,-0,1n,NaN]"))
            .unwrap();
        let keep = runtime.dup_jsvalue(&base).unwrap();
        let JsValue::Object(object) = &base else {
            panic!("array");
        };
        let object = *object;
        let count = runtime.0.state.borrow().heap.object_strong_count(object);
        for (index, expected) in [
            Value::Undefined,
            Value::Null,
            Value::Bool(true),
            Value::Int(17),
            Value::Float(-0.0),
            Value::BigInt(crate::engine::value::bigint::JsBigInt::one()),
        ]
        .iter()
        .enumerate()
        {
            let result = runtime
                .try_dense_array_immediate_read(&base, index as u32)
                .unwrap();
            assert!(
                runtime
                    .root_value(&result)
                    .unwrap()
                    .same_quickjs_representation(expected)
            );
        }
        assert!(
            matches!(runtime.try_dense_array_immediate_read(&base, 6), Some(JsValue::Float(v)) if v.is_nan())
        );
        assert_eq!(
            runtime.0.state.borrow().heap.object_strong_count(object),
            count
        );
        runtime.release_jsvalue(keep).unwrap();
        assert!(runtime.try_dense_array_immediate_read(&base, 0).is_none());
        assert!(runtime.0.state.borrow().heap.object(object).is_ok());
        runtime.release_jsvalue(base).unwrap();
    }

    #[test]
    fn dense_array_read_leaf_declines_missing_slow_reference_and_exotic_values() {
        for expression in [
            "[,1]",
            "Object.defineProperty([1], '0', {get(){throw 71}})",
            "Object.defineProperty([1], '0', {writable:false})",
            "[{}]",
            "['x']",
            // Short BigInts are edge-free; this case must retain a heap payload.
            "[9223372036854775808n]",
            "[Symbol()]",
            "new Proxy([1], {get(){throw 72}})",
            "new Uint8Array([1])",
            "({0:1,length:1})",
        ] {
            let runtime = Runtime::new();
            let base = runtime
                .into_jsvalue(receiver(&runtime, expression))
                .unwrap();
            let keep = runtime.dup_jsvalue(&base).unwrap();
            let JsValue::Object(object) = &base else {
                panic!("object");
            };
            let object = *object;
            let count = runtime.0.state.borrow().heap.object_strong_count(object);
            let result = runtime.try_dense_array_immediate_read(&base, 0);
            let declined = result.is_none();
            if let Some(value) = result {
                runtime.release_jsvalue(value).unwrap();
            }
            let count_after = runtime.0.state.borrow().heap.object_strong_count(object);
            let still_live = runtime.0.state.borrow().heap.object(object).is_ok();
            runtime.release_jsvalue(keep).unwrap();
            runtime.release_jsvalue(base).unwrap();
            assert!(declined, "{expression}");
            assert_eq!(count_after, count);
            assert!(still_live);
        }
        let runtime = Runtime::new();
        let base = runtime.into_jsvalue(receiver(&runtime, "[1]")).unwrap();
        let keep = runtime.dup_jsvalue(&base).unwrap();
        assert!(runtime.try_dense_array_immediate_read(&base, 1).is_none());
        assert!(
            runtime
                .try_dense_array_immediate_read(&base, u32::MAX)
                .is_none()
        );
        let other = Runtime::new();
        assert!(other.try_dense_array_immediate_read(&base, 0).is_none());
        runtime.release_jsvalue(keep).unwrap();
        runtime.release_jsvalue(base).unwrap();
    }

    #[test]
    fn dense_array_read_leaf_keeps_frozen_materialized_values_on_canonical_path() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let base = runtime
            .into_jsvalue(
                context
                    .eval("globalThis.frozenRead = Object.freeze([7, -0])")
                    .unwrap(),
            )
            .unwrap();
        runtime.run_gc().unwrap();
        assert!(runtime.try_dense_array_immediate_read(&base, 0).is_none());
        assert_eq!(
            context
                .eval("frozenRead[0] === 7 && 1 / frozenRead[1] === -Infinity")
                .unwrap(),
            Value::Bool(true)
        );
        runtime.release_jsvalue(base).unwrap();
    }

    #[test]
    fn dense_array_read_leaf_does_not_drain_deferred_or_borrowed_state() {
        let runtime = Runtime::new();
        let base = runtime.into_jsvalue(receiver(&runtime, "[1]")).unwrap();
        let keep = runtime.dup_jsvalue(&base).unwrap();
        {
            let _borrow = runtime.0.state.borrow();
            assert!(runtime.try_dense_array_immediate_read(&base, 0).is_none());
        }
        let released = runtime.new_object(None).unwrap();
        let released_id = released.object_id();
        {
            let _borrow = runtime.0.state.borrow();
            drop(released);
        }
        assert!(runtime.0.deferred_references.has_pending());
        assert!(runtime.try_dense_array_immediate_read(&base, 0).is_none());
        assert!(runtime.0.deferred_references.has_pending());
        assert!(runtime.0.state.borrow().heap.object(released_id).is_ok());
        runtime.run_gc().unwrap();
        assert!(matches!(
            runtime.try_dense_array_immediate_read(&base, 0),
            Some(JsValue::Int(1))
        ));
        runtime.release_jsvalue(keep).unwrap();
        runtime.release_jsvalue(base).unwrap();
    }
}

#[cfg(test)]
mod ordinary_field_leaf_tests {
    use super::*;
    use crate::engine::code::{bytecode::Instruction, runtime::PublishedFunctionSnapshot};
    use crate::engine::object::OrdinaryRead;

    fn executable(runtime: &Runtime, field: &str) -> (PublishedFunctionSnapshot, u32) {
        let mut context = runtime.new_context();
        let callable = runtime
            .callable_from_value(
                context
                    .eval(&format!("(function(o,v){{o.{field}=v;return o.{field}}})"))
                    .unwrap(),
            )
            .unwrap();
        let crate::engine::vm::call::CallableExecution::Bytecode { bytecode, .. } =
            runtime.bytecode_for_callable(&callable).unwrap()
        else {
            panic!("bytecode")
        };
        let executable = runtime.snapshot_function_bytecode(&bytecode).unwrap();
        let index = executable
            .code
            .iter()
            .find_map(|op| match op {
                Instruction::GetField(index) => Some(*index),
                _ => None,
            })
            .unwrap();
        (executable, index)
    }

    fn release_read(runtime: &Runtime, read: Option<OrdinaryRead>) {
        if let Some(OrdinaryRead::Complete(Some(value))) = read {
            runtime.release_jsvalue(value).unwrap();
        }
    }

    #[test]
    fn ordinary_field_leaf_preserves_scalar_values_flags_and_declines_nonlocal_rules() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let (executable, index) = executable(&runtime, "x");
        for source in [
            "({x:undefined})",
            "({x:null})",
            "({x:true})",
            "({x:42})",
            "({x:1.5})",
            "({x:-0})",
            "({x:42n})",
        ] {
            let base = runtime.into_jsvalue(context.eval(source).unwrap()).unwrap();
            let retained = runtime.dup_jsvalue(&base).unwrap();
            assert!(
                runtime
                    .try_ordinary_field_immediate_read(&base, &executable, index)
                    .is_some()
            );
            assert!(runtime.try_ordinary_field_immediate_write(
                &base,
                &executable,
                index,
                &JsValue::Int(17)
            ));
            assert_eq!(
                runtime.try_ordinary_field_immediate_read(&base, &executable, index),
                Some(JsValue::Int(17))
            );
            runtime.release_jsvalue(retained).unwrap();
            runtime.release_jsvalue(base).unwrap();
        }
        for source in [
            "({get x(){throw 42}})",
            "Object.create({x:42})",
            "new Proxy({x:42},{get(){throw 42},set(){throw 42}})",
            "({x:{}})",
            "({x:'text'})",
            // Heap BigInts still decline; Short BigInts are covered above.
            "({x:9223372036854775808n})",
            "({x:Symbol()})",
            "([])",
            "new Uint8Array(1)",
            "globalThis",
        ] {
            let base = runtime.into_jsvalue(context.eval(source).unwrap()).unwrap();
            let retained = runtime.dup_jsvalue(&base).unwrap();
            let result = runtime.try_ordinary_field_immediate_read(&base, &executable, index);
            let read_declined = result.is_none();
            if let Some(value) = result {
                runtime.release_jsvalue(value).unwrap();
            }
            let write_declined = !runtime.try_ordinary_field_immediate_write(
                &base,
                &executable,
                index,
                &JsValue::Int(17),
            );
            runtime.release_jsvalue(retained).unwrap();
            runtime.release_jsvalue(base).unwrap();
            assert!(read_declined, "{source}");
            assert!(write_declined, "{source}");
        }
        for source in [
            "Object.freeze({x:42})",
            "Object.defineProperty({},'x',{value:42,writable:false})",
        ] {
            let base = runtime.into_jsvalue(context.eval(source).unwrap()).unwrap();
            let retained = runtime.dup_jsvalue(&base).unwrap();
            assert_eq!(
                runtime.try_ordinary_field_immediate_read(&base, &executable, index),
                Some(JsValue::Int(42))
            );
            assert!(!runtime.try_ordinary_field_immediate_write(
                &base,
                &executable,
                index,
                &JsValue::Int(17)
            ));
            assert_eq!(
                runtime.try_ordinary_field_immediate_read(&base, &executable, index),
                Some(JsValue::Int(42))
            );
            runtime.release_jsvalue(retained).unwrap();
            runtime.release_jsvalue(base).unwrap();
        }
    }

    #[test]
    fn ordinary_field_leaf_checks_published_atom_domain_and_release_guards() {
        let runtime = Runtime::new();
        let foreign = Runtime::new();
        let mut context = runtime.new_context();
        let (code, index) = executable(&runtime, "x");
        let (foreign_code, foreign_index) = executable(&foreign, "x");
        let base = runtime
            .into_jsvalue(context.eval("({x:42})").unwrap())
            .unwrap();
        assert!(
            runtime
                .try_ordinary_field_immediate_read(&base, &code, index)
                .is_none()
        );
        assert!(!runtime.try_ordinary_field_immediate_write(
            &base,
            &code,
            index,
            &JsValue::Int(17)
        ));
        let retained = runtime.dup_jsvalue(&base).unwrap();
        assert!(
            runtime
                .try_ordinary_field_immediate_read(&base, &foreign_code, foreign_index)
                .is_none()
        );
        assert!(!runtime.try_ordinary_field_immediate_write(
            &base,
            &foreign_code,
            foreign_index,
            &JsValue::Int(17)
        ));
        assert!(
            runtime
                .try_ordinary_field_immediate_read(&base, &code, u32::MAX)
                .is_none()
        );
        let unrooted = PublishedFunctionSnapshot::empty_for_test(context.realm);
        assert!(
            runtime
                .try_ordinary_field_immediate_read(&base, &unrooted, 0)
                .is_none()
        );
        let queued = runtime.new_object(None).unwrap();
        {
            let _state = runtime.0.state.borrow();
            assert!(
                runtime
                    .try_ordinary_field_immediate_read(&base, &code, index)
                    .is_none()
            );
            assert!(!runtime.try_ordinary_field_immediate_write(
                &base,
                &code,
                index,
                &JsValue::Int(17)
            ));
            drop(queued);
        }
        assert!(
            runtime
                .try_ordinary_field_immediate_read(&base, &code, index)
                .is_none()
        );
        assert!(!runtime.try_ordinary_field_immediate_write(
            &base,
            &code,
            index,
            &JsValue::Int(17)
        ));
        assert!(runtime.0.deferred_references.has_pending());
        runtime.drain_deferred_references().unwrap();
        assert_eq!(
            runtime.try_ordinary_field_immediate_read(&base, &code, index),
            Some(JsValue::Int(42))
        );
        let object_value = runtime
            .into_jsvalue(Value::Object(runtime.new_object(None).unwrap()))
            .unwrap();
        assert!(!runtime.try_ordinary_field_immediate_write(&base, &code, index, &object_value));
        assert_eq!(
            runtime.try_ordinary_field_immediate_read(&base, &code, index),
            Some(JsValue::Int(42))
        );
        let (lazy_code, lazy_index) = executable(&runtime, "min");
        let math = runtime.into_jsvalue(context.eval("Math").unwrap()).unwrap();
        assert!(
            runtime
                .try_ordinary_field_immediate_read(&math, &lazy_code, lazy_index)
                .is_none()
        );
        assert!(!runtime.try_ordinary_field_immediate_write(
            &math,
            &lazy_code,
            lazy_index,
            &JsValue::Int(17)
        ));
        assert!(matches!(
            context.eval("typeof Math.min").unwrap(),
            Value::String(_)
        ));
        runtime.release_jsvalue(retained).unwrap();
        runtime.release_jsvalue(base).unwrap();
        runtime.release_jsvalue(object_value).unwrap();
        runtime.release_jsvalue(math).unwrap();
    }
    #[test]
    fn recovery_length_leaf_preserves_utf16_brand_and_final_owner_boundaries() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let (code, index) = executable(&runtime, "length");
        for (source, expected) in [("[1,,3]", 3), ("'a\\ud83d\\ude00'", 3)] {
            let base = runtime.into_jsvalue(context.eval(source).unwrap()).unwrap();
            let retained = runtime.dup_jsvalue(&base).unwrap();
            let actual = runtime
                .try_ordinary_field_immediate_read(&base, &code, index)
                .unwrap();
            assert_eq!(runtime.root_value(&actual).unwrap(), Value::Int(expected));
            runtime.release_jsvalue(retained).unwrap();
            runtime.release_jsvalue(base).unwrap();
        }
        let proxy = runtime
            .into_jsvalue(context.eval("new Proxy([], {get(){throw 91}})").unwrap())
            .unwrap();
        let proxy_retained = runtime.dup_jsvalue(&proxy).unwrap();
        assert!(
            runtime
                .try_ordinary_field_immediate_read(&proxy, &code, index)
                .is_none()
        );
        let unique = runtime
            .into_jsvalue(Value::String(
                crate::engine::value::JsString::from_owned_utf16(vec![97, 0xd800]),
            ))
            .unwrap();
        // A final string owner is ready when retirement cannot allocate.
        drop(runtime.new_object(None).unwrap());
        let keep_capacity = runtime.new_object(None).unwrap();
        let unique_read = runtime.try_ordinary_field_immediate_read(&unique, &code, index);
        let unique_retained = runtime.dup_jsvalue(&unique).unwrap();
        let actual = runtime
            .try_ordinary_field_immediate_read(&unique, &code, index)
            .unwrap();
        assert_eq!(runtime.root_value(&actual).unwrap(), Value::Int(2));
        runtime.release_jsvalue(proxy_retained).unwrap();
        runtime.release_jsvalue(proxy).unwrap();
        runtime.release_jsvalue(unique_retained).unwrap();
        runtime.release_jsvalue(unique).unwrap();
        drop(keep_capacity);
        assert_eq!(unique_read, Some(JsValue::Int(2)));
    }

    #[test]
    fn recovery_arguments_leaf_reads_current_cell_and_declines_redefinitions() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let base = runtime
            .into_jsvalue(
                context
                    .eval(
                        "globalThis.args=(function(a){globalThis.change=x=>a=x;return arguments})(1);args",
                    )
                    .unwrap(),
            )
            .unwrap();
        let retained = runtime.dup_jsvalue(&base).unwrap();
        assert_eq!(
            runtime.try_array_immediate_read(&base, 0),
            Some(JsValue::Int(1))
        );
        drop(context.eval("change(7)").unwrap());
        assert_eq!(
            runtime.try_array_immediate_read(&base, 0),
            Some(JsValue::Int(7))
        );
        drop(
            context
                .eval("Object.defineProperty(args,'0',{value:8,writable:false});change(9)")
                .unwrap(),
        );
        assert_eq!(
            runtime.try_array_immediate_read(&base, 0),
            Some(JsValue::Int(8))
        );
        drop(
            context
                .eval("Object.defineProperty(args,'0',{get(){return 11},configurable:true})")
                .unwrap(),
        );
        assert!(runtime.try_array_immediate_read(&base, 0).is_none());
        assert_eq!(context.eval("args[0]").unwrap(), Value::Int(11));
        drop(context.eval("delete args[0]").unwrap());
        assert!(runtime.try_array_immediate_read(&base, 0).is_none());
        runtime.release_jsvalue(retained).unwrap();
        runtime.release_jsvalue(base).unwrap();
    }

    #[test]
    fn linked_native_fact_is_bound_to_the_selected_callee_not_a_property_cache() {
        use crate::engine::builtins::native::{MathMinMaxKind, NativeFunctionId};
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let (code, index) = executable(&runtime, "x");
        let base = runtime
            .into_jsvalue(
                context
                    .eval("globalThis.selectedNative={x:Math.min};selectedNative")
                    .unwrap(),
            )
            .unwrap();
        let mut fact = None;
        let Some(OrdinaryRead::Complete(Some(value))) = runtime
            .prepare_linked_own_read_selected(&base, &code, index, Some(&mut fact))
            .unwrap()
        else {
            panic!("own native")
        };
        let Value::Object(callee) = runtime.root_and_release_jsvalue(value).unwrap() else {
            panic!("own native")
        };
        drop(context.eval("selectedNative.x=Math.max").unwrap());
        let data = fact
            .take()
            .unwrap()
            .into_parts_jsvalue(callee.runtime(), callee.object_id())
            .unwrap();
        assert_eq!(
            data.target,
            NativeFunctionId::MathMinMax(MathMinMaxKind::Min)
        );
        release_read(
            &runtime,
            runtime
                .prepare_linked_own_read_selected(&base, &code, index, Some(&mut fact))
                .unwrap(),
        );
        assert!(
            fact.take()
                .unwrap()
                .into_parts_jsvalue(callee.runtime(), callee.object_id())
                .is_none()
        );
        release_read(
            &runtime,
            runtime
                .prepare_linked_own_read_selected(&base, &code, index, Some(&mut fact))
                .unwrap(),
        );
        let foreign = Runtime::new();
        let mut foreign_context = foreign.new_context();
        let Value::Object(foreign_callee) = foreign_context.eval("Math.max").unwrap() else {
            panic!("native")
        };
        assert!(
            fact.take()
                .unwrap()
                .into_parts_jsvalue(foreign_callee.runtime(), foreign_callee.object_id())
                .is_none()
        );
        runtime.release_jsvalue(base).unwrap();
    }

    #[test]
    fn borrowed_fallback_read_preserves_inherited_getter_and_receiver_owner() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let base = runtime
            .into_jsvalue(
                context
                    .eval("({__proto__: {get x(){return this.marker}}, marker: 7})")
                    .unwrap(),
            )
            .unwrap();
        let key = runtime.intern_property_key("x").unwrap();
        let read = runtime
            .prepare_value_property_read_borrowed_jsvalue(context.realm, &base, &key)
            .unwrap();
        assert!(matches!(&read, OrdinaryRead::Call { .. }));
        runtime.release_jsvalue(base).unwrap();
        let result = runtime
            .finish_prepared_read_jsvalue(context.realm, &key, read)
            .unwrap();
        assert!(matches!(
            result,
            crate::engine::value::conversion::NativeConversion::Value(Some(JsValue::Int(7)))
        ));
    }

    #[test]
    fn recovery_linked_read_keeps_the_selected_getter_and_return_owner() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let (code, index) = executable(&runtime, "x");
        let base = context
            .eval("globalThis.readLog=0;globalThis.o={get x(){readLog++;return 7}};o")
            .unwrap();
        let read = runtime
            .prepare_linked_own_read(&base, &code, index)
            .unwrap()
            .unwrap();
        assert_eq!(context.eval("readLog").unwrap(), Value::Int(0));
        drop(
            context
                .eval("Object.defineProperty(o,'x',{get(){throw 99}})")
                .unwrap(),
        );
        let key = PropertyKey::from_borrowed_atom(
            runtime.clone(),
            code.property_key_atoms.as_ref().unwrap()[index as usize],
        )
        .unwrap();
        let result = runtime
            .finish_prepared_read(context.realm, &key, read)
            .unwrap();
        assert!(matches!(
            result,
            crate::engine::value::conversion::NativeConversion::Value(Some(Value::Int(7)))
        ));
        assert_eq!(context.eval("readLog").unwrap(), Value::Int(1));
    }
}
