//! QuickJS-compatible mapped and unmapped Arguments exotic objects.
//!
//! The compiler/VM boundary decides when an implicit binding is materialized
//! and supplies either copied actual values or shared argument VarRefs. This
//! module owns the class shape, cached realm intrinsics, representation state,
//! and the mapped `[[DefineOwnProperty]]` transitions.

use crate::engine::api::runtime::Runtime;
use crate::engine::api::runtime_error::RuntimeError;
#[cfg(test)]
use crate::engine::heap::roots::VarRefRoot;

#[cfg(test)]
use crate::engine::heap::ContextId;
#[cfg(test)]
use crate::engine::heap::ObjectPayload;
#[cfg(test)]
use crate::engine::object::CompleteOrdinaryPropertyDescriptor;
#[cfg(test)]
use crate::engine::object::WellKnownSymbol;
use crate::engine::object::{ObjectRef, OrdinaryPropertyDescriptor, PropertyKey};
#[cfg(test)]
use crate::engine::value::JsValue;

mod allocation;

impl Runtime {
    #[cfg(test)]
    pub(crate) fn new_unmapped_arguments_object(
        &self,
        realm: ContextId,
        values: Vec<JsValue>,
    ) -> Result<ObjectRef, RuntimeError> {
        let _unwind = self.unwind_guard();
        let _operation = match self.operation() {
            Ok(operation) => operation,
            Err(error) => {
                for value in values {
                    if self.skip_cleanup() {
                        break;
                    }
                    self.release_jsvalue(value)?;
                }
                self.check_poison()?;
                return Err(error);
            }
        };
        let result = self.0.state.borrow_mut().new_unmapped_arguments_object(
            &self.0.poisoned,
            realm,
            values,
        );
        self.check_poison()?;
        result.map(|object| ObjectRef::from_owned_handle(self.clone(), object))
    }
    #[cfg(test)]
    pub(crate) fn new_mapped_arguments_object(
        &self,
        realm: ContextId,
        current_function: &ObjectRef,
        roots: Vec<VarRefRoot>,
    ) -> Result<ObjectRef, RuntimeError> {
        if !current_function.belongs_to(self) {
            return Err(RuntimeError::WrongRuntime("mapped arguments callee"));
        }
        // Preserve the real public prefix before looking at any cell domain.
        let _operation = self.operation()?;
        let callee = self
            .0
            .state
            .borrow_mut()
            .checked_arguments_callee(&self.0.poisoned, current_function.object_id())?;
        self.check_poison()?;
        for root in &roots {
            if !root.belongs_to(self) {
                return Err(RuntimeError::WrongRuntime("mapped arguments element"));
            }
        }
        let cells = roots
            .into_iter()
            .map(VarRefRoot::into_execution_handle)
            .collect();
        let result = self.0.state.borrow_mut().new_mapped_arguments_object(
            &self.0.poisoned,
            realm,
            callee,
            cells,
        );
        self.check_poison()?;
        result.map(|object| ObjectRef::from_owned_handle(self.clone(), object))
    }

    #[cfg(test)]
    pub(crate) fn arguments_fast_len(
        &self,
        object: &ObjectRef,
    ) -> Result<Option<u32>, RuntimeError> {
        Ok(self
            .0
            .state
            .borrow()
            .heap
            .arguments_state(object.object_id())?
            .1)
    }

    #[cfg(test)]
    pub(crate) fn set_arguments_fast_len(
        &self,
        object: &ObjectRef,
        fast_len: Option<u32>,
    ) -> Result<(), RuntimeError> {
        self.0
            .state
            .borrow_mut()
            .heap
            .set_arguments_fast_len(object.object_id(), fast_len)?;
        Ok(())
    }

    /// QuickJS's arguments exotic hook converts a fast existing numeric field
    /// to slow storage before applying an explicit DefineOwnProperty. Mapped
    /// VarRef slots remain aliases unless the descriptor makes them accessors
    /// or non-writable.
    pub(crate) fn define_arguments_index(
        &self,
        object: &ObjectRef,
        key: &PropertyKey,
        descriptor: &OrdinaryPropertyDescriptor,
    ) -> Result<Option<bool>, RuntimeError> {
        self.validate_object_and_key(object, key)?;
        let _unwind = self.unwind_guard();
        self.0.state.borrow_mut().define_arguments_index_public(
            &self.0.poisoned,
            object.object_id(),
            key.atom(),
            descriptor,
        )
    }
}

#[cfg(test)]
mod tests {
    use crate::engine::code::function::metadata::ClosureVariableKind;
    use crate::engine::object::DescriptorField;
    use crate::engine::value::Value;

    use super::*;

    fn data_descriptor(
        descriptor: Option<CompleteOrdinaryPropertyDescriptor>,
    ) -> (Value, bool, bool, bool) {
        let Some(CompleteOrdinaryPropertyDescriptor::Data {
            value,
            writable,
            enumerable,
            configurable,
        }) = descriptor
        else {
            panic!("expected an own data property")
        };
        (value, writable, enumerable, configurable)
    }

    #[test]
    fn unmapped_arguments_use_exact_values_realm_roots_and_quickjs_descriptors() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        let array_prototype = context.array_prototype().unwrap();
        let values_key = runtime.intern_property_key("values").unwrap();
        let original_values = context.get_property(&array_prototype, &values_key).unwrap();
        assert!(
            context
                .set_property(&array_prototype, &values_key, Value::Int(99))
                .unwrap()
        );

        let arguments = runtime
            .new_unmapped_arguments_object(context.realm, vec![JsValue::Int(10), JsValue::Int(20)])
            .unwrap();
        assert_eq!(
            runtime
                .get_prototype_of(&arguments)
                .unwrap()
                .as_ref()
                .map(ObjectRef::object_id),
            Some(context.object_prototype().unwrap().object_id())
        );
        assert_eq!(runtime.arguments_fast_len(&arguments), Ok(Some(2)));
        assert!(matches!(
            runtime
                .0
                .state
                .borrow()
                .heap
                .object(arguments.object_id())
                .unwrap()
                .payload,
            ObjectPayload::Arguments {
                mapped: false,
                fast_len: Some(2)
            }
        ));

        let zero = runtime.intern_property_key("0").unwrap();
        assert_eq!(
            data_descriptor(runtime.get_own_property(&arguments, &zero).unwrap()),
            (Value::Int(10), true, true, true)
        );
        let length = runtime.intern_property_key("length").unwrap();
        assert_eq!(
            data_descriptor(runtime.get_own_property(&arguments, &length).unwrap()),
            (Value::Int(2), true, false, true)
        );

        let callee = runtime.intern_property_key("callee").unwrap();
        let Some(CompleteOrdinaryPropertyDescriptor::Accessor {
            get: Some(get),
            set: Some(set),
            enumerable,
            configurable,
        }) = runtime.get_own_property(&arguments, &callee).unwrap()
        else {
            panic!("unmapped callee was not the poison accessor")
        };
        assert_eq!(get.as_object(), set.as_object());
        assert!(!enumerable);
        assert!(!configurable);

        let iterator = PropertyKey::from(
            runtime
                .well_known_symbol(WellKnownSymbol::Iterator)
                .expect("well-known symbol"),
        );
        assert_eq!(
            data_descriptor(runtime.get_own_property(&arguments, &iterator).unwrap()),
            (original_values, true, false, true)
        );
        assert_eq!(
            runtime.own_property_keys(&arguments).unwrap(),
            [
                runtime.intern_property_key("0").unwrap(),
                runtime.intern_property_key("1").unwrap(),
                length,
                callee,
                iterator,
            ]
        );
    }

    #[test]
    fn arguments_intrinsic_properties_are_cached_per_realm() {
        let runtime = Runtime::new();
        let mut first = runtime.new_context().expect("create context");
        let mut second = runtime.new_context().expect("create context");
        let values = runtime.intern_property_key("values").unwrap();
        let first_values = first
            .get_property(&first.array_prototype().unwrap(), &values)
            .unwrap();
        let second_values = second
            .get_property(&second.array_prototype().unwrap(), &values)
            .unwrap();
        assert_ne!(first_values, second_values);

        let first_arguments = runtime
            .new_unmapped_arguments_object(first.realm, Vec::new())
            .unwrap();
        let second_arguments = runtime
            .new_unmapped_arguments_object(second.realm, Vec::new())
            .unwrap();
        let iterator = PropertyKey::from(
            runtime
                .well_known_symbol(WellKnownSymbol::Iterator)
                .expect("well-known symbol"),
        );
        assert_eq!(
            data_descriptor(
                runtime
                    .get_own_property(&first_arguments, &iterator)
                    .unwrap()
            )
            .0,
            first_values
        );
        assert_eq!(
            data_descriptor(
                runtime
                    .get_own_property(&second_arguments, &iterator)
                    .unwrap()
            )
            .0,
            second_values
        );

        let callee = runtime.intern_property_key("callee").unwrap();
        let poison = |object: &ObjectRef| {
            let Some(CompleteOrdinaryPropertyDescriptor::Accessor {
                get: Some(get),
                set: Some(set),
                ..
            }) = runtime.get_own_property(object, &callee).unwrap()
            else {
                panic!("unmapped arguments lost its poison callee")
            };
            assert_eq!(get.as_object(), set.as_object());
            get
        };
        assert_ne!(
            poison(&first_arguments).as_object(),
            poison(&second_arguments).as_object()
        );
    }

    #[test]
    fn mapped_arguments_keep_aliases_until_delete_accessor_or_read_only_transition() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        let callee = context.function_prototype().unwrap();
        let root = runtime
            .new_var_ref(JsValue::Int(1), false, false, ClosureVariableKind::Normal)
            .unwrap();
        let arguments = runtime
            .new_mapped_arguments_object(
                context.realm,
                &callee,
                vec![root.try_clone().expect("duplicate root")],
            )
            .unwrap();
        let zero = runtime.intern_property_key("0").unwrap();

        assert!(
            context
                .set_property(&arguments, &zero, Value::Int(2))
                .unwrap()
        );
        assert_eq!(runtime.read_var_ref(&root).unwrap(), JsValue::Int(2));
        assert_eq!(runtime.arguments_fast_len(&arguments), Ok(Some(1)));

        runtime.write_var_ref(&root, JsValue::Int(3)).unwrap();
        assert_eq!(
            context.get_property(&arguments, &zero).unwrap(),
            Value::Int(3)
        );
        assert!(
            context
                .define_own_property(
                    &arguments,
                    &zero,
                    &OrdinaryPropertyDescriptor {
                        value: DescriptorField::Present(Value::Int(4)),
                        enumerable: DescriptorField::Present(false),
                        ..OrdinaryPropertyDescriptor::new()
                    },
                )
                .unwrap()
        );
        assert_eq!(runtime.read_var_ref(&root).unwrap(), JsValue::Int(4));
        assert_eq!(runtime.arguments_fast_len(&arguments), Ok(None));
        runtime.write_var_ref(&root, JsValue::Int(5)).unwrap();
        assert_eq!(
            context.get_property(&arguments, &zero).unwrap(),
            Value::Int(5)
        );

        assert!(
            context
                .define_own_property(
                    &arguments,
                    &zero,
                    &OrdinaryPropertyDescriptor {
                        value: DescriptorField::Present(Value::Int(6)),
                        writable: DescriptorField::Present(false),
                        ..OrdinaryPropertyDescriptor::new()
                    },
                )
                .unwrap()
        );
        assert_eq!(runtime.read_var_ref(&root).unwrap(), JsValue::Int(6));
        runtime.write_var_ref(&root, JsValue::Int(7)).unwrap();
        assert_eq!(
            context.get_property(&arguments, &zero).unwrap(),
            Value::Int(6)
        );
        assert_eq!(
            data_descriptor(runtime.get_own_property(&arguments, &zero).unwrap()),
            (Value::Int(6), false, false, true)
        );

        let callee_key = runtime.intern_property_key("callee").unwrap();
        assert_eq!(
            data_descriptor(runtime.get_own_property(&arguments, &callee_key).unwrap()),
            (Value::Object(callee), true, false, true)
        );
    }

    #[test]
    fn arguments_delete_updates_fast_state_and_never_reconnects_a_mapping() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        let callee = context.function_prototype().unwrap();
        let first = runtime
            .new_var_ref(JsValue::Int(1), false, false, ClosureVariableKind::Normal)
            .unwrap();
        let second = runtime
            .new_var_ref(JsValue::Int(2), false, false, ClosureVariableKind::Normal)
            .unwrap();
        let tail = runtime
            .new_mapped_arguments_object(
                context.realm,
                &callee,
                vec![
                    first.try_clone().expect("duplicate root"),
                    second.try_clone().expect("duplicate root"),
                ],
            )
            .unwrap();
        let one = runtime.intern_property_key("1").unwrap();
        assert!(runtime.delete_property(&tail, &one).unwrap());
        assert_eq!(runtime.arguments_fast_len(&tail), Ok(Some(1)));

        let middle = runtime
            .new_mapped_arguments_object(
                context.realm,
                &callee,
                vec![
                    first.try_clone().expect("duplicate root"),
                    second.try_clone().expect("duplicate root"),
                ],
            )
            .unwrap();
        let zero = runtime.intern_property_key("0").unwrap();
        assert!(runtime.delete_property(&middle, &zero).unwrap());
        assert_eq!(runtime.arguments_fast_len(&middle), Ok(None));
        assert!(context.set_property(&middle, &zero, Value::Int(8)).unwrap());
        runtime.write_var_ref(&first, JsValue::Int(9)).unwrap();
        assert_eq!(context.get_property(&middle, &zero).unwrap(), Value::Int(8));
    }
}
