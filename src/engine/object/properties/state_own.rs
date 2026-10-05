//! Own descriptors borrow published storage until a real effect needs owners.
use super::*;
use crate::engine::builtins::TypedIndexRead;
use crate::engine::heap::Heap;
use crate::engine::object::StateOwnedCompleteDescriptor;
use crate::engine::object::property::CompletePropertyDescriptor;

/// The heap borrow keeps the descriptor's layout and edge owners valid. A
/// callback or mutation requires ending this borrow before it can start.
pub(crate) struct BorrowedOwnPropertyDescriptor<'a> {
    heap: &'a Heap,
    record: CompletePropertyDescriptor<RawValue>,
}
impl BorrowedOwnPropertyDescriptor<'_> {
    pub(crate) fn record(&self) -> &CompletePropertyDescriptor<RawValue> {
        &self.record
    }
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn heap(&self) -> &Heap {
        self.heap
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::heap::RawId;

    #[test]
    fn synchronous_own_snapshot_borrows_aliased_accessors_without_retains() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        let Value::Object(object) = context
            .eval("({get x(){throw 42}, set x(v){throw 43}})")
            .unwrap()
        else {
            panic!()
        };
        let key = runtime.intern_property_key("x").unwrap();
        let mut state = runtime.0.state.borrow_mut();
        let StateOwnPropertySnapshot::Borrowed(snapshot) = state
            .own_property_snapshot_in_state(&runtime.0.poisoned, object.object_id(), key.atom())
            .unwrap()
        else {
            panic!()
        };
        let CompletePropertyDescriptor::Accessor {
            get: Some(RawValue::Object(get)),
            set: Some(RawValue::Object(set)),
            ..
        } = snapshot.record()
        else {
            panic!()
        };
        assert_eq!(snapshot.heap().object_strong_count(*get), Ok(1));
        assert_eq!(snapshot.heap().object_strong_count(*set), Ok(1));
        assert!(!runtime.0.deferred_references.has_pending());
    }

    #[test]
    fn virtual_and_dense_snapshots_preserve_flags_and_owner_responsibility() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        let Value::Object(string) = context.eval("new String('xy')").unwrap() else {
            panic!()
        };
        let Value::Object(array) = context.eval("[{x:1}]").unwrap() else {
            panic!()
        };
        let key = runtime.property_key_for_index(0).unwrap();
        let mut state = runtime.0.state.borrow_mut();
        let StateOwnPropertySnapshot::Owned(owned) = state
            .own_property_snapshot_in_state(&runtime.0.poisoned, string.object_id(), key.atom())
            .unwrap()
        else {
            panic!()
        };
        let CompletePropertyDescriptor::Data {
            value: RawValue::String(id),
            writable: false,
            enumerable: true,
            configurable: false,
        } = owned.record()
        else {
            panic!()
        };
        assert_eq!(state.heap.string(*id).unwrap(), &JsString::from_static("x"));
        owned
            .release_in_state(&mut state, &runtime.0.poisoned)
            .unwrap();
        let StateOwnPropertySnapshot::Borrowed(snapshot) = state
            .own_property_snapshot_in_state(&runtime.0.poisoned, array.object_id(), key.atom())
            .unwrap()
        else {
            panic!()
        };
        let CompletePropertyDescriptor::Data {
            value: RawValue::Object(id),
            writable: true,
            enumerable: true,
            configurable: true,
        } = snapshot.record()
        else {
            panic!()
        };
        assert_eq!(snapshot.heap().object_strong_count(*id), Ok(1));
    }

    #[test]
    fn complete_snapshot_failed_second_retain_rolls_back_first_owner() {
        let runtime = Runtime::new();
        let first = runtime.new_object(None).unwrap();
        let second = runtime.new_object(None).unwrap();
        let mut state = runtime.0.state.borrow_mut();
        state
            .heap
            .set_strong_count_for_test(RawId::Object(second.object_id()), u32::MAX);
        let result = StateOwnedCompleteDescriptor::retain_in_state(
            &mut state,
            &runtime.0.poisoned,
            &CompletePropertyDescriptor::Accessor {
                get: Some(RawValue::Object(first.object_id())),
                set: Some(RawValue::Object(second.object_id())),
                enumerable: true,
                configurable: true,
            },
        );
        state
            .heap
            .set_strong_count_for_test(RawId::Object(second.object_id()), 1);
        assert!(matches!(
            result,
            Err(RuntimeError::Heap(HeapError::Overflow { .. }))
        ));
        assert_eq!(state.heap.object_strong_count(first.object_id()), Ok(1));
        assert!(!runtime.is_poisoned());
    }
}

#[must_use]
pub(crate) enum StateOwnPropertySnapshot<'a> {
    Absent,
    Borrowed(BorrowedOwnPropertyDescriptor<'a>),
    Owned(StateOwnedCompleteDescriptor),
    Shared(crate::engine::builtins::SharedTypedRead),
    Proxy,
}

impl RuntimeState {
    pub(crate) fn own_property_snapshot_in_state(
        &mut self,
        poisoned: &Cell<bool>,
        object: ObjectId,
        atom: Atom,
    ) -> Result<StateOwnPropertySnapshot<'_>, RuntimeError> {
        self.atoms.resolve(atom)?;
        if matches!(self.heap.object(object)?.payload, ObjectPayload::Proxy(_)) {
            return Ok(StateOwnPropertySnapshot::Proxy);
        }
        if matches!(
            self.heap.object(object)?.payload,
            ObjectPayload::TypedArray(_)
        ) && let Some(numeric) = self.typed_array_canonical_numeric_index(atom)?
        {
            return Ok(match numeric {
                CanonicalNumericIndex::Invalid => StateOwnPropertySnapshot::Absent,
                CanonicalNumericIndex::Valid(index) => {
                    match self.typed_array_read_index_in_state(object, index)? {
                        TypedIndexRead::Value(None) => StateOwnPropertySnapshot::Absent,
                        TypedIndexRead::Value(Some(value)) => StateOwnPropertySnapshot::Owned(
                            StateOwnedCompleteDescriptor::from_owned_data(value, true, true, true),
                        ),
                        TypedIndexRead::Shared(read) => StateOwnPropertySnapshot::Shared(read),
                    }
                }
            });
        }
        if let Some(index) = self.atoms.array_index(atom)? {
            let data = self.heap.object(object)?;
            if let ObjectPayload::Primitive(PrimitiveObjectData::String(id)) = data.payload
                && let Some(unit) = self.heap.string(id)?.code_unit_at(index as usize)
            {
                let value =
                    JsValue::String(self.heap.allocate_string(JsString::from_code_unit(unit))?);
                return Ok(StateOwnPropertySnapshot::Owned(
                    StateOwnedCompleteDescriptor::from_owned_data(value, false, true, false),
                ));
            }
            if let Some(value) = data.dense_array_value(index) {
                return Ok(StateOwnPropertySnapshot::Borrowed(
                    BorrowedOwnPropertyDescriptor {
                        heap: &self.heap,
                        record: CompletePropertyDescriptor::Data {
                            value: value.clone(),
                            writable: true,
                            enumerable: true,
                            configurable: true,
                        },
                    },
                ));
            }
        }
        // AutoInit is synchronous semantic work. Its published result is then
        // consumed through the same slot snapshot as an already eager value.
        let data = self.heap.object(object)?;
        let shape = self.heap.shape(data.shape)?;
        let Some(mut index) = shape.find(AtomIdx::from_raw(atom.raw())) else {
            return Ok(StateOwnPropertySnapshot::Absent);
        };
        if matches!(data.slots[index as usize], PropertySlot::AutoInit(_)) {
            self.materialize_auto_init_property(poisoned, object, atom)?;
            // Materialization may publish a different layout. Ordinary slots
            // keep the location already selected under this State access.
            let data = self.heap.object(object)?;
            let shape = self.heap.shape(data.shape)?;
            let Some(materialized_index) = shape.find(AtomIdx::from_raw(atom.raw())) else {
                return Ok(StateOwnPropertySnapshot::Absent);
            };
            index = materialized_index;
        }
        let data = self.heap.object(object)?;
        let shape = self.heap.shape(data.shape)?;
        let index = index as usize;
        let flags = shape.entries()[index].flags;
        let record = match &data.slots[index] {
            PropertySlot::Data(value) => CompletePropertyDescriptor::Data {
                value: value.clone(),
                writable: flags.writable,
                enumerable: flags.enumerable,
                configurable: flags.configurable,
            },
            PropertySlot::VarRef(id) => {
                let value = self.heap.var_ref(*id)?.value.clone();
                if matches!(value, RawValue::Uninitialized) {
                    return Err(RuntimeError::Engine(self.native_atom_error(
                        ErrorKind::Reference,
                        "",
                        atom,
                        " is not initialized",
                    )?));
                }
                CompletePropertyDescriptor::Data {
                    value,
                    writable: flags.writable,
                    enumerable: flags.enumerable,
                    configurable: flags.configurable,
                }
            }
            PropertySlot::Accessor { get, set } => CompletePropertyDescriptor::Accessor {
                get: get.option().map(RawValue::Object),
                set: set.option().map(RawValue::Object),
                enumerable: flags.enumerable,
                configurable: flags.configurable,
            },
            PropertySlot::AutoInit(_) => {
                return Err(RuntimeError::Invariant(
                    "lazy own descriptor did not materialize",
                ));
            }
        };
        Ok(StateOwnPropertySnapshot::Borrowed(
            BorrowedOwnPropertyDescriptor {
                heap: &self.heap,
                record,
            },
        ))
    }
}
