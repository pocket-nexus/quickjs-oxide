//! Callback-free DefineOwnProperty admission and canonical publication.
//! Inputs remain borrowed under the caller's exclusive State access. Only the
//! slot transaction acquires the accepted edges; rejection retains nothing.

use super::*;
use crate::engine::heap::ObjectKind;
use crate::engine::object::property::PropertyDescriptor;

impl RuntimeState {
    /// Define an ordinary own property without rooting a descriptor, receiver,
    /// or key. `None` is an exotic boundary selected before observable work;
    /// `Some(false)` is a semantic rejection and must never be replayed.
    pub(crate) fn try_define_own_property_in_state(
        &mut self,
        poisoned: &Cell<bool>,
        object: ObjectId,
        atom: Atom,
        descriptor: &PropertyDescriptor<RawValue>,
    ) -> Result<Option<bool>, RuntimeError> {
        self.atoms.resolve(atom)?;
        let data = self.heap.object(object)?;
        let indexed = self.atoms.array_index(atom)?.is_some();
        match (&data.kind, &data.payload) {
            (ObjectKind::ModuleNamespace, _)
            | (_, ObjectPayload::Proxy(_))
            | (_, ObjectPayload::TypedArray(_)) => return Ok(None),
            (_, ObjectPayload::Array { .. }) => {
                // Index publication can change length or dense representation;
                // length can run observable conversion. Their shared class
                // kernel owns those stages, not the ordinary slot transaction.
                if indexed
                    || atom
                        == self
                            .pinned_atoms
                            .get(crate::engine::atom::pinned::PinnedAtom::Length)
                {
                    return Ok(None);
                }
            }
            (_, ObjectPayload::Arguments { .. }) if indexed => return Ok(None),
            (_, ObjectPayload::Primitive(PrimitiveObjectData::String(_))) if indexed => {
                return Ok(None);
            }
            _ => {}
        }
        let shape = self.heap.shape(data.shape)?;
        if let Some(index) = shape.find(AtomIdx::from_raw(atom.raw())) {
            let index = index as usize;
            let flags = shape.entries()[index].flags;
            match &data.slots[index] {
                PropertySlot::AutoInit(_) => {
                    if descriptor.is_data_descriptor() && descriptor.is_accessor_descriptor() {
                        return Err(PropertyDefinitionError::InvalidDescriptor.into());
                    }
                    // QuickJS rejects incompatible lazy flags before creating
                    // the value; even an empty compatible descriptor materializes.
                    if !flags.configurable
                        && (descriptor.configurable == Some(true)
                            || descriptor.enumerable.is_some_and(|v| v != flags.enumerable)
                            || descriptor.is_accessor_descriptor()
                            || (!flags.writable && descriptor.writable == Some(true)))
                    {
                        return Ok(Some(false));
                    }
                    self.materialize_auto_init_property(poisoned, object, atom)?;
                }
                PropertySlot::VarRef(id)
                    if matches!(self.heap.var_ref(*id)?.value, RawValue::Uninitialized) =>
                {
                    return Ok(None);
                }
                _ => {}
            }
        }
        let accepted = self.define_raw_property_with_poison(poisoned, object, atom, descriptor)?;
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_execution_event(
            "ordinary_descriptor_defined_in_state",
        );
        Ok(Some(accepted))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::heap::RawId;

    #[test]
    fn borrowed_descriptor_publication_preserves_aliases_and_canonical_layouts() {
        let runtime = Runtime::new();
        let left = runtime.new_object(None).unwrap();
        let right = runtime.new_object(None).unwrap();
        let getter = runtime.new_object(None).unwrap();
        let key = runtime.intern_property_key("defined").unwrap();
        let mut state = runtime.0.state.borrow_mut();
        let incoming = PropertyDescriptor {
            value: Some(RawValue::Object(left.object_id())),
            writable: Some(true),
            enumerable: Some(true),
            configurable: Some(true),
            ..PropertyDescriptor::new()
        };
        for object in [&left, &right] {
            assert_eq!(
                state.try_define_own_property_in_state(
                    &runtime.0.poisoned,
                    object.object_id(),
                    key.atom(),
                    &incoming,
                ),
                Ok(Some(true))
            );
        }
        assert_eq!(
            state.heap.object(left.object_id()).unwrap().shape,
            state.heap.object(right.object_id()).unwrap().shape
        );
        assert_eq!(state.heap.object_strong_count(left.object_id()), Ok(3));
        assert_eq!(
            state.try_define_own_property_in_state(
                &runtime.0.poisoned,
                left.object_id(),
                key.atom(),
                &PropertyDescriptor {
                    get: Some(Some(RawValue::Object(getter.object_id()))),
                    set: Some(Some(RawValue::Object(getter.object_id()))),
                    ..PropertyDescriptor::new()
                },
            ),
            Ok(Some(true))
        );
        assert_eq!(state.heap.object_strong_count(left.object_id()), Ok(2));
        assert_eq!(state.heap.object_strong_count(getter.object_id()), Ok(3));
        assert!(!runtime.0.deferred_references.has_pending());
        assert!(!runtime.is_poisoned());
    }

    #[test]
    fn failed_descriptor_edge_retain_keeps_layout_and_all_existing_owners() {
        let runtime = Runtime::new();
        let owner = runtime.new_object(None).unwrap();
        let first = runtime.new_object(None).unwrap();
        let blocked = runtime.new_object(None).unwrap();
        let key = runtime.intern_property_key("failed-accessor").unwrap();
        let mut state = runtime.0.state.borrow_mut();
        let shape = state.heap.object(owner.object_id()).unwrap().shape;
        state
            .heap
            .set_strong_count_for_test(RawId::Object(blocked.object_id()), u32::MAX);
        let result = state.try_define_own_property_in_state(
            &runtime.0.poisoned,
            owner.object_id(),
            key.atom(),
            &PropertyDescriptor {
                get: Some(Some(RawValue::Object(first.object_id()))),
                set: Some(Some(RawValue::Object(blocked.object_id()))),
                ..PropertyDescriptor::new()
            },
        );
        state
            .heap
            .set_strong_count_for_test(RawId::Object(blocked.object_id()), 1);
        assert!(matches!(
            result,
            Err(RuntimeError::Heap(HeapError::Overflow { .. }))
        ));
        assert_eq!(state.heap.object(owner.object_id()).unwrap().shape, shape);
        assert!(
            state
                .heap
                .object(owner.object_id())
                .unwrap()
                .slots
                .is_empty()
        );
        assert_eq!(state.heap.object_strong_count(first.object_id()), Ok(1));
        assert_eq!(state.heap.object_strong_count(blocked.object_id()), Ok(1));
        assert!(!runtime.is_poisoned());
    }

    #[test]
    fn state_descriptor_admission_preserves_lazy_and_exotic_definition_rules() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        assert_eq!(context.eval(r#"(function(){
            function F(){};
            if (Reflect.defineProperty(F, 'prototype', {get(){return 1}})) return false;
            if (!Reflect.defineProperty(F, 'name', {get(){return 'changed'}})) return false;
            if (F.name !== 'changed') return false;
            const o={}; Object.preventExtensions(o);
            if (Reflect.defineProperty(o, 'new', {value: undefined})) return false;
            const a=[]; Object.defineProperty(a, 'length', {writable:false});
            if (Reflect.defineProperty(a, '0', {value:2})) return false;
            if (!Reflect.defineProperty(a, 'extra', {value:o})) return false;
            const s=new String('x');
            if (Reflect.defineProperty(s, '0', {value:'y'})) return false;
            if (!Reflect.defineProperty(s, 'extra', {value:F})) return false;
            let count=0; const p=new Proxy({}, {defineProperty(t,k,d){count++; return Reflect.defineProperty(t,k,d)}});
            Object.defineProperty(p, 'x', {value:o});
            return p.x===o && count===1;
        })()"#).unwrap(), Value::Bool(true));
    }
}
