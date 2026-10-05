//! Callback-free String and Arguments definition shares canonical publication.
use super::*;
use crate::engine::object::property::{CompletePropertyDescriptor, PropertyDescriptor};

#[derive(Clone)]
enum StringDescriptorValue {
    Raw(RawValue),
    Unit(u16),
}
impl RuntimeState {
    pub(super) fn define_string_unit_in_state(
        &self,
        object: ObjectId,
        unit: u16,
        descriptor: &PropertyDescriptor<RawValue>,
    ) -> Result<bool, RuntimeError> {
        // A virtual character already exists as a code unit. Validation need
        // not allocate an arena string merely to compare its immutable value.
        let raw = |value: &RawValue| StringDescriptorValue::Raw(value.clone());
        let descriptor = PropertyDescriptor {
            value: descriptor.value.as_ref().map(raw),
            get: descriptor.get.as_ref().map(|v| v.as_ref().map(raw)),
            set: descriptor.set.as_ref().map(|v| v.as_ref().map(raw)),
            writable: descriptor.writable,
            enumerable: descriptor.enumerable,
            configurable: descriptor.configurable,
        };
        let current = CompletePropertyDescriptor::Data {
            value: StringDescriptorValue::Unit(unit),
            writable: false,
            enumerable: true,
            configurable: false,
        };
        let same = |a: &StringDescriptorValue, b: &StringDescriptorValue| match (a, b) {
            (
                StringDescriptorValue::Raw(RawValue::String(id)),
                StringDescriptorValue::Unit(unit),
            )
            | (
                StringDescriptorValue::Unit(unit),
                StringDescriptorValue::Raw(RawValue::String(id)),
            ) => self
                .heap
                .string(*id)
                .is_ok_and(|string| string.len() == 1 && string.code_unit_at(0) == Some(*unit)),
            (StringDescriptorValue::Unit(a), StringDescriptorValue::Unit(b)) => a == b,
            (StringDescriptorValue::Raw(a), StringDescriptorValue::Raw(b)) => {
                crate::engine::value::collection_key::same_value(&self.heap, a, b)
            }
            _ => false,
        };
        match validate_and_apply_property_descriptor(
            self.heap.object(object)?.extensible,
            &descriptor,
            Some(&current),
            &StringDescriptorValue::Raw(RawValue::Undefined),
            same,
        ) {
            Ok(_) => Ok(true),
            Err(PropertyDefinitionError::InvalidDescriptor) => {
                Err(PropertyDefinitionError::InvalidDescriptor.into())
            }
            Err(_) => Ok(false),
        }
    }

    pub(in crate::engine::object) fn define_arguments_index_in_state(
        &mut self,
        poisoned: &Cell<bool>,
        object: ObjectId,
        atom: Atom,
        index: u32,
        descriptor: &PropertyDescriptor<RawValue>,
    ) -> Result<bool, RuntimeError> {
        let (mapped, fast_len) = self.heap.arguments_state(object)?;
        if fast_len.is_some_and(|len| index < len) {
            self.heap.set_arguments_fast_len(object, None)?;
        }
        let data = self.heap.object(object)?;
        let shape = self.heap.shape(data.shape)?;
        let var_ref = shape.find(AtomIdx::from_raw(atom.raw())).and_then(|index| {
            match data.slots[index as usize] {
                PropertySlot::VarRef(id) => Some(id),
                _ => None,
            }
        });
        let Some(var_ref) = var_ref else {
            return self.define_raw_property_with_poison(poisoned, object, atom, descriptor);
        };
        if !mapped {
            return Err(RuntimeError::Invariant(
                "unmapped Arguments object contains a mapped VarRef slot",
            ));
        }
        let Some(complete) = self.validate_raw_property(object, atom, descriptor)? else {
            return Ok(false);
        };
        let mut value_published = false;
        // Data definitions update the mapped binding before preserving or
        // detaching the map. Accessors detach it without invoking any getter.
        let result = (|| {
            if let CompletePropertyDescriptor::Data { value, .. } = &complete
                && descriptor.value.is_some()
            {
                let value = self.dup_jsvalue(
                    &JsValue::from_raw(value.clone())
                        .ok_or(RuntimeError::Invariant("uninitialized mapped argument"))?,
                )?;
                self.write_var_ref(poisoned, var_ref, value)?;
                value_published = true;
            }
            match complete {
                CompletePropertyDescriptor::Data {
                    writable: true,
                    enumerable,
                    configurable,
                    ..
                } => self.store_property_slot_with_poison(
                    poisoned,
                    object,
                    atom,
                    PropertyFlags::data(true, enumerable, configurable),
                    PropertySlot::VarRef(var_ref),
                ),
                complete => {
                    self.store_complete_raw_property_inner(Some(poisoned), object, atom, complete)
                }
            }
        })();
        if value_published && result.is_err() {
            poisoned.set(true);
        }
        result.map(|()| true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::heap::RawId;

    #[test]
    fn mapped_definitions_preserve_alias_then_detach_for_readonly_or_accessor() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        assert_eq!(
            context
                .eval(
                    r#"(function(){
            function f(x) {
                const marker={};
                Object.defineProperty(arguments,'0',{value:marker});
                if(x!==marker || arguments[0]!==marker) return false;
                Object.defineProperty(arguments,'0',{writable:false});
                x=42;
                if(arguments[0]!==marker) return false;
                Object.defineProperty(arguments,'0',{get(){return 7}});
                return arguments[0]===7 && x===42;
            }
            function g(x) {
                Object.defineProperty(arguments,'0',{get(){return 9}});
                return arguments[0]===9 && x===3;
            }
            return f(3) && g(3);
        })()"#
                )
                .unwrap(),
            Value::Bool(true)
        );
    }

    #[test]
    fn mapped_retain_failure_leaves_binding_value_and_property_unchanged() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        let Value::Object(arguments) = context.eval("(function(x){return arguments})(3)").unwrap()
        else {
            panic!()
        };
        let incoming = runtime.new_object(None).unwrap();
        let atom = Atom::from_immediate_integer(0).unwrap();
        let mut state = runtime.0.state.borrow_mut();
        let data = state.heap.object(arguments.object_id()).unwrap();
        let slot = state
            .heap
            .shape(data.shape)
            .unwrap()
            .find(AtomIdx::from_raw(atom.raw()))
            .unwrap() as usize;
        let PropertySlot::VarRef(cell) = data.slots[slot] else {
            panic!()
        };
        state
            .heap
            .set_strong_count_for_test(RawId::Object(incoming.object_id()), u32::MAX);
        let result = state.try_define_own_property_in_state(
            &runtime.0.poisoned,
            arguments.object_id(),
            atom,
            &PropertyDescriptor {
                value: Some(RawValue::Object(incoming.object_id())),
                ..PropertyDescriptor::new()
            },
        );
        state
            .heap
            .set_strong_count_for_test(RawId::Object(incoming.object_id()), 1);
        assert!(matches!(
            result,
            Err(RuntimeError::Heap(HeapError::Overflow { .. }))
        ));
        assert!(matches!(
            state.heap.var_ref(cell).unwrap().value,
            RawValue::Int(3)
        ));
        assert!(
            matches!(state.heap.object(arguments.object_id()).unwrap().slots[slot],PropertySlot::VarRef(id) if id==cell)
        );
        assert!(!runtime.is_poisoned());
    }

    #[test]
    fn string_unit_validation_preserves_utf16_and_descriptor_permissions() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        assert_eq!(
            context
                .eval(
                    r#"(function(){
            const s=new String('\ud800x');
            if(!Reflect.defineProperty(s,'0',{value:'\ud800'}))return false;
            if(Reflect.defineProperty(s,'0',{value:'x'}))return false;
            if(Reflect.defineProperty(s,'0',{value:'\ud800x'}))return false;
            if(Reflect.defineProperty(s,'0',{writable:true}))return false;
            if(Reflect.defineProperty(s,'0',{get(){throw 1}}))return false;
            if(!Reflect.defineProperty(s,'4',{value:s,writable:true}))return false;
            return s[0]==='\ud800' && s[4]===s;
        })()"#
                )
                .unwrap(),
            Value::Bool(true)
        );
    }
}
