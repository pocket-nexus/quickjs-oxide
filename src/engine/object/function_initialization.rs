mod bytecode;

use crate::engine::api::error::{Error, ErrorKind};
use crate::engine::api::runtime::Runtime;
use crate::engine::api::runtime_error::RuntimeError;
use crate::engine::atom::{Atom, AtomIdx};
use crate::engine::builtins::native::NativeFunctionId;

use crate::engine::code::rooted::FunctionBytecodeRef;
use crate::engine::heap::roots::VarRefRoot;

use crate::engine::heap::runtime::{RuntimeState, owned_values::OwnedValueGuard};
use crate::engine::heap::{AutoInitProperty, ContextId, ObjectId, PropertySlot, RawValue};
use crate::engine::object::property::PropertyDescriptor;
use crate::engine::object::shape::{PropertyFlags, ShapeEntry};
use crate::engine::object::{
    CallableRef, CompleteOrdinaryPropertyDescriptor, DescriptorField, ObjectRef,
    OrdinaryPropertyDescriptor, PropertyKey,
};
use crate::engine::value::{JsString, JsValue, Value};
use std::cell::Cell;

// Only the fields produced by fresh function initialization enter this
// sequence. Object carries one owned, admitted producer edge.
enum FreshFunctionField {
    Integer(i32),
    String(JsString),
    Object(ObjectId),
}

impl Runtime {
    pub(crate) fn new_bytecode_closure_with_slots(
        &self,
        caller_realm: ContextId,
        function: &FunctionBytecodeRef,
        closure_slots: &[VarRefRoot],
    ) -> Result<CallableRef, RuntimeError> {
        let _operation = self.operation()?;
        if !function.belongs_to(self) {
            return Err(RuntimeError::WrongRuntime("function bytecode"));
        }
        if closure_slots.iter().any(|slot| !slot.belongs_to(self)) {
            return Err(RuntimeError::WrongRuntime("closure variable"));
        }
        self.ensure_dynamic_import_bytecode_tree_authorized(function)?;

        let object = self.0.state.borrow_mut().new_bytecode_closure_with_slots(
            &self.0.poisoned,
            caller_realm,
            function.bytecode_id(),
            closure_slots.iter().map(VarRefRoot::id).collect(),
        )?;
        Ok(CallableRef::from_validated_object(
            ObjectRef::from_owned_handle(self.clone(), object),
        ))
    }

    #[cfg(test)]
    pub(crate) fn define_function_auto_init_prototype(
        &self,
        function: &ObjectRef,
        realm: ContextId,
    ) -> Result<(), RuntimeError> {
        self.0
            .state
            .borrow_mut()
            .define_function_auto_init_prototype(&self.0.poisoned, function.object_id(), realm)
    }

    pub(crate) fn define_native_builtin_auto_init(
        &self,
        object: &ObjectRef,
        realm: ContextId,
        target: NativeFunctionId,
        name: &'static str,
        length: u8,
        min_readable_args: u8,
    ) -> Result<(), RuntimeError> {
        let key = self.intern_property_key(name)?;
        self.define_native_builtin_auto_init_with_key(
            object,
            realm,
            &key,
            target,
            name,
            length,
            min_readable_args,
            PropertyFlags::data(true, false, true),
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn define_native_builtin_auto_init_with_key(
        &self,
        object: &ObjectRef,
        realm: ContextId,
        key: &PropertyKey,
        target: NativeFunctionId,
        name: &'static str,
        length: u8,
        min_readable_args: u8,
        flags: PropertyFlags,
    ) -> Result<(), RuntimeError> {
        self.validate_object_and_key(object, key)?;
        self.0
            .state
            .borrow_mut()
            .define_native_builtin_auto_init_with_key(
                &self.0.poisoned,
                object.object_id(),
                realm,
                key.atom(),
                target,
                name,
                length,
                min_readable_args,
                flags,
            )
    }

    pub(crate) fn define_string_auto_init(
        &self,
        object: &ObjectRef,
        realm: ContextId,
        name: &str,
        value: &'static str,
    ) -> Result<(), RuntimeError> {
        let key = self.intern_property_key(name)?;
        self.0.state.borrow_mut().define_string_auto_init_with_key(
            &self.0.poisoned,
            object.object_id(),
            realm,
            key.atom(),
            value,
        )
    }

    #[cfg(test)]
    pub(crate) fn define_failure_auto_init(
        &self,
        object: &ObjectRef,
        realm: ContextId,
        name: &str,
    ) -> Result<(), RuntimeError> {
        let key = self.intern_property_key(name)?;
        let mut state = self.0.state.borrow_mut();
        state.heap.context(realm)?;
        let object_id = object.object_id();
        let (prototype, mut entries, mut slots) = {
            let object = state.heap.object(object_id)?;
            let shape = state.heap.shape(object.shape)?;
            (
                shape.prototype(),
                shape.entries().to_vec(),
                object.slots.clone(),
            )
        };
        entries.push(ShapeEntry {
            atom: AtomIdx::from_raw(key.atom().raw()),
            flags: PropertyFlags::data(true, false, true),
        });
        slots.push(PropertySlot::auto_init(AutoInitProperty::FailureProbe {
            realm,
        }));
        state.replace_layout(object_id, prototype, &entries, slots)
    }

    pub(crate) fn define_function_data_property(
        &self,
        object: &ObjectRef,
        name: &str,
        value: Value,
        writable: bool,
        configurable: bool,
    ) -> Result<(), RuntimeError> {
        let key = self.intern_property_key(name)?;
        let defined = self.define_own_property(
            object,
            &key,
            &OrdinaryPropertyDescriptor {
                value: DescriptorField::Present(value),
                writable: DescriptorField::Present(writable),
                enumerable: DescriptorField::Present(false),
                configurable: DescriptorField::Present(configurable),
                ..OrdinaryPropertyDescriptor::new()
            },
        )?;
        if !defined {
            return Err(RuntimeError::Invariant(
                "function intrinsic property definition was rejected",
            ));
        }
        Ok(())
    }

    /// QuickJS `JS_DefineObjectName`: define a configurable, non-writable,
    /// non-enumerable own `name` only when the object does not already carry a
    /// non-empty (or otherwise authoritative) own name.
    pub(crate) fn define_object_name_for_object(
        &self,
        object: &ObjectRef,
        name: &JsString,
    ) -> Result<(), RuntimeError> {
        let key = self.pinned_property_key(crate::engine::atom::pinned::PinnedAtom::Name)?;
        let should_define = match self.get_own_property(object, &key)? {
            None => true,
            Some(CompleteOrdinaryPropertyDescriptor::Data {
                value: Value::String(current),
                ..
            }) => current.is_empty(),
            Some(
                CompleteOrdinaryPropertyDescriptor::Data { .. }
                | CompleteOrdinaryPropertyDescriptor::Accessor { .. },
            ) => false,
        };
        if !should_define {
            return Ok(());
        }
        let defined = self.define_own_property(
            object,
            &key,
            &OrdinaryPropertyDescriptor {
                value: DescriptorField::Present(Value::String(name.clone())),
                writable: DescriptorField::Present(false),
                enumerable: DescriptorField::Present(false),
                configurable: DescriptorField::Present(true),
                ..OrdinaryPropertyDescriptor::new()
            },
        )?;
        if !defined {
            return Err(RuntimeError::Engine(Error::new(
                ErrorKind::Type,
                "cannot define function name",
            )));
        }
        Ok(())
    }
}

impl RuntimeState {
    /// Define one field on a newly allocated function or factory object.
    /// The caller keeps the admitted key/value producers owned until this
    /// canonical descriptor transaction retained its stored copies.
    pub(crate) fn define_fresh_function_data_property(
        &mut self,
        poisoned: &Cell<bool>,
        object: ObjectId,
        key: Atom,
        value: &RawValue,
        writable: bool,
        configurable: bool,
    ) -> Result<(), RuntimeError> {
        if !self.define_raw_property_with_poison(
            poisoned,
            object,
            key,
            &PropertyDescriptor {
                value: Some(value.clone()),
                writable: Some(writable),
                enumerable: Some(false),
                configurable: Some(configurable),
                ..PropertyDescriptor::new()
            },
        )? {
            return Err(RuntimeError::Invariant(
                "function intrinsic property definition was rejected",
            ));
        }
        Ok(())
    }

    pub(crate) fn define_function_auto_init_prototype(
        &mut self,
        poisoned: &Cell<bool>,
        function: ObjectId,
        realm: ContextId,
    ) -> Result<(), RuntimeError> {
        let key = self
            .pinned_atoms
            .get(crate::engine::atom::pinned::PinnedAtom::Prototype);
        let state = self;
        state.heap.context(realm)?;
        let object_id = function;
        let (prototype, mut entries, mut slots) = {
            let object = state.heap.object(object_id)?;
            let shape = state.heap.shape(object.shape)?;
            if shape.find(AtomIdx::from_raw(key.raw())).is_some() {
                return Err(RuntimeError::Invariant(
                    "function prototype autoinit property already exists",
                ));
            }
            (
                shape.prototype(),
                shape.entries().to_vec(),
                object.slots.clone(),
            )
        };
        entries.push(ShapeEntry {
            atom: AtomIdx::from_raw(key.raw()),
            flags: PropertyFlags::data(true, false, false),
        });
        slots.push(PropertySlot::auto_init(
            AutoInitProperty::FunctionPrototype { realm },
        ));
        state.replace_layout_with_poison(poisoned, object_id, prototype, &entries, slots)
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn define_native_builtin_auto_init_with_key(
        &mut self,
        poisoned: &Cell<bool>,
        object: ObjectId,
        realm: ContextId,
        key: Atom,
        target: NativeFunctionId,
        name: &'static str,
        length: u8,
        min_readable_args: u8,
        flags: PropertyFlags,
    ) -> Result<(), RuntimeError> {
        let state = self;
        state.heap.context(realm)?;
        let object_id = object;
        let (prototype, mut entries, mut slots) = {
            let object = state.heap.object(object_id)?;
            let shape = state.heap.shape(object.shape)?;
            if shape.find(AtomIdx::from_raw(key.raw())).is_some() {
                return Err(RuntimeError::Invariant(
                    "native builtin autoinit property already exists",
                ));
            }
            (
                shape.prototype(),
                shape.entries().to_vec(),
                object.slots.clone(),
            )
        };
        entries.push(ShapeEntry {
            atom: AtomIdx::from_raw(key.raw()),
            flags,
        });
        slots.push(PropertySlot::auto_init(AutoInitProperty::NativeBuiltin {
            realm,
            target,
            name,
            length,
            min_readable_args,
        }));
        state.replace_layout_with_poison(poisoned, object_id, prototype, &entries, slots)
    }

    pub(crate) fn define_string_auto_init_with_key(
        &mut self,
        poisoned: &Cell<bool>,
        object: ObjectId,
        realm: ContextId,
        key: Atom,
        value: &'static str,
    ) -> Result<(), RuntimeError> {
        let state = self;
        state.heap.context(realm)?;
        let object_id = object;
        let (prototype, mut entries, mut slots) = {
            let object = state.heap.object(object_id)?;
            let shape = state.heap.shape(object.shape)?;
            if shape.find(AtomIdx::from_raw(key.raw())).is_some() {
                return Err(RuntimeError::Invariant(
                    "string autoinit property already exists",
                ));
            }
            (
                shape.prototype(),
                shape.entries().to_vec(),
                object.slots.clone(),
            )
        };
        entries.push(ShapeEntry {
            atom: AtomIdx::from_raw(key.raw()),
            flags: PropertyFlags::data(true, false, true),
        });
        slots.push(PropertySlot::auto_init(AutoInitProperty::String {
            realm,
            value,
        }));
        state.replace_layout_with_poison(poisoned, object_id, prototype, &entries, slots)
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn define_native_builtin_auto_init(
        &mut self,
        poisoned: &Cell<bool>,
        object: ObjectId,
        realm: ContextId,
        target: NativeFunctionId,
        name: &'static str,
        length: u8,
        min_readable_args: u8,
    ) -> Result<(), RuntimeError> {
        let key = self.intern_property_key_js_string(&JsString::try_from_utf8(name)?)?;
        let defined = self.define_native_builtin_auto_init_with_key(
            poisoned,
            object,
            realm,
            key,
            target,
            name,
            length,
            min_readable_args,
            PropertyFlags::data(true, false, true),
        );
        if !poisoned.get()
            && let Err(error) = self.atoms.release(key)
        {
            poisoned.set(true);
            return defined.and(Err(error.into()));
        }
        defined
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn define_fresh_function_string_property(
        &mut self,
        poisoned: &Cell<bool>,
        object: ObjectId,
        name: &str,
        value: JsString,
        writable: bool,
        configurable: bool,
    ) -> Result<(), RuntimeError> {
        self.define_fresh_function_property(
            poisoned,
            object,
            name,
            FreshFunctionField::String(value),
            writable,
            configurable,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn define_fresh_function_integer_property(
        &mut self,
        poisoned: &Cell<bool>,
        object: ObjectId,
        name: &str,
        value: i32,
        writable: bool,
        configurable: bool,
    ) -> Result<(), RuntimeError> {
        self.define_fresh_function_property(
            poisoned,
            object,
            name,
            FreshFunctionField::Integer(value),
            writable,
            configurable,
        )
    }

    /// Consume one owned object producer; a required checked duplicate is
    /// performed by the caller before entering this field sequence.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn define_fresh_function_object_property(
        &mut self,
        poisoned: &Cell<bool>,
        object: ObjectId,
        name: &str,
        value: ObjectId,
        writable: bool,
        configurable: bool,
    ) -> Result<(), RuntimeError> {
        self.define_fresh_function_property(
            poisoned,
            object,
            name,
            FreshFunctionField::Object(value),
            writable,
            configurable,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn define_fresh_function_property(
        &mut self,
        poisoned: &Cell<bool>,
        object: ObjectId,
        name: &str,
        value: FreshFunctionField,
        writable: bool,
        configurable: bool,
    ) -> Result<(), RuntimeError> {
        let (value, string) = match value {
            FreshFunctionField::Integer(value) => (JsValue::Int(value), None),
            FreshFunctionField::String(value) => (JsValue::Undefined, Some(value)),
            FreshFunctionField::Object(value) => (JsValue::Object(value), None),
        };
        // An Object argument already owns its producer at method entry; it
        // must retire even when constructing or interning the key fails.
        let mut producer = OwnedValueGuard::new(self, poisoned, value);
        let (state, producer) = producer.parts();
        let key = state.intern_property_key_js_string(&JsString::try_from_utf8(name)?)?;
        let defined: Result<(), RuntimeError> = (|| {
            if let Some(string) = string {
                *producer = Some(JsValue::String(state.heap.allocate_string(string)?));
            }
            state.define_fresh_function_data_property(
                poisoned,
                object,
                key,
                &producer
                    .as_ref()
                    .expect("fresh function field producer")
                    .as_raw(),
                writable,
                configurable,
            )
        })();
        // Retire the producer before the key on both success and rejection.
        // Published cleanup failure already quarantined both owners.
        let retired = if poisoned.get() {
            Ok(())
        } else {
            state.release_owned_jsvalue(
                poisoned,
                producer.take().expect("fresh function field producer"),
            )
        };
        let defined = defined.and(retired);
        // Preserve the public field helper's value-before-key retirement.
        if !poisoned.get()
            && let Err(error) = state.atoms.release(key)
        {
            poisoned.set(true);
            return defined.and(Err(error.into()));
        }
        defined
    }
}
