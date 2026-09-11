//! Short, non-reentrant access to ordinary own slots. Slot positions never
//! leave this module and a write locates and commits under one state borrow.
use crate::engine::api::runtime::Runtime;
use crate::engine::api::runtime_error::RuntimeError;
use crate::engine::atom::Atom;
use crate::engine::heap::runtime::RuntimeState;
use crate::engine::heap::{ObjectId, ObjectPayload, PropertySlot};
use crate::engine::object::shape::PropertyFlags;
use crate::engine::object::{ObjectRef, PropertyKey};
use crate::engine::value::Value;

struct OwnSlot {
    index: usize,
    flags: PropertyFlags,
}

// The caller keeps the state borrowed until the located slot is consumed.
fn locate(
    state: &RuntimeState,
    object: ObjectId,
    atom: Atom,
) -> Result<Option<OwnSlot>, RuntimeError> {
    let data = state.heap.object(object)?;
    let shape = state.heap.shape(data.shape)?;
    let Some(index) = shape.find(atom) else {
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

pub(super) enum SetProbe {
    Stored(bool),
    Writable,
    Setter(Option<crate::engine::object::CallableRef>),
    Missing(Option<ObjectRef>),
    Special,
}

impl Runtime {
    /// The target stays rooted until the selected setter/prototype has been
    /// promoted. No callback, mutation or cleanup occurs between snapshot and
    /// promotion. Data writes locate and commit in a single mutable borrow.
    pub(super) fn ordinary_set_probe(
        &self,
        object: &ObjectRef,
        key: &PropertyKey,
        value: &Value,
        receiver_is_target: bool,
    ) -> Result<SetProbe, RuntimeError> {
        enum Selected {
            Setter(Option<ObjectId>),
            Missing(Option<ObjectId>),
        }
        let selected = {
            let mut state = self.0.state.borrow_mut();
            let id = object.object_id();
            if !matches!(state.heap.object(id)?.payload, ObjectPayload::Ordinary) {
                return Ok(SetProbe::Special);
            }
            match locate(&state, id, key.atom())? {
                None => {
                    let data = state.heap.object(id)?;
                    Selected::Missing(state.heap.shape(data.shape)?.prototype())
                }
                Some(slot) => match &state.heap.object(id)?.slots[slot.index] {
                    PropertySlot::Data(_) => {
                        if !slot.flags.writable {
                            return Ok(SetProbe::Stored(false));
                        }
                        if !receiver_is_target {
                            return Ok(SetProbe::Writable);
                        }
                        let replacement = PropertySlot::Data(self.raw_property_value(value)?);
                        replace_data(&mut state, id, slot, replacement)?;
                        return Ok(SetProbe::Stored(true));
                    }
                    PropertySlot::Accessor { set, .. } => Selected::Setter(*set),
                    PropertySlot::AutoInit(_) | PropertySlot::VarRef(_) => {
                        return Ok(SetProbe::Special);
                    }
                },
            }
        };
        Ok(match selected {
            Selected::Setter(set) => SetProbe::Setter(
                set.map(|id| {
                    ObjectRef::from_borrowed_handle(self.clone(), id)
                        .map(crate::engine::object::CallableRef::from_validated_object)
                })
                .transpose()?,
            ),
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
    let atoms = state.retain_slot_atoms(std::slice::from_ref(&replacement))?;
    match state.heap.replace_object_slot(id, slot.index, replacement) {
        Ok(cleanup) => state.apply_cleanup(cleanup),
        Err(error) => {
            state.release_atoms(atoms)?;
            Err(error.into())
        }
    }
}

pub(super) enum ReadProbe {
    Value(Value),
    Getter(Option<crate::engine::object::CallableRef>),
    Missing(Option<ObjectRef>),
    Special,
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
        if !matches!(state.heap.object(id)?.payload, ObjectPayload::Ordinary) {
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
        if !matches!(state.heap.object(id)?.payload, ObjectPayload::Ordinary) {
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
                    get: *get,
                    set: *set,
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

    pub(super) fn ordinary_read_probe(
        &self,
        object: &ObjectRef,
        key: &PropertyKey,
    ) -> Result<ReadProbe, RuntimeError> {
        use crate::engine::object::operations::PropertySnapshot;
        let Some(snapshot) = self.ordinary_property_snapshot(object, key)? else {
            return Ok(ReadProbe::Special);
        };
        Ok(match snapshot {
            Some(PropertySnapshot::Data { value, .. }) => {
                ReadProbe::Value(self.root_raw_value(&value)?)
            }
            Some(PropertySnapshot::Accessor { get, .. }) => ReadProbe::Getter(
                get.map(|id| {
                    ObjectRef::from_borrowed_handle(self.clone(), id)
                        .map(crate::engine::object::CallableRef::from_validated_object)
                })
                .transpose()?,
            ),
            Some(PropertySnapshot::AutoInit | PropertySnapshot::VarRef { .. }) => {
                ReadProbe::Special
            }
            None => ReadProbe::Missing(self.get_prototype_of(object)?),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::object::{DescriptorField, OrdinaryPropertyDescriptor};

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
