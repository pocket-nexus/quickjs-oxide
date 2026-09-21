use crate::engine::api::runtime::Runtime;
use crate::engine::api::runtime_error::RuntimeError;
use crate::engine::atom::{Atom, AtomIdx};

use crate::engine::builtins::native::{NativeFunctionId, PrimitiveKind};
use crate::engine::code::function::metadata::{
    ClosureSource, ClosureVariableKind, ClosureVariableName,
};
use crate::engine::code::rooted::FunctionBytecodeRef;
use crate::engine::heap::roots::VarRefRoot;

use crate::engine::heap::{
    ContextId, ObjectData, ObjectPayload, PrimitiveObjectData, PropertySlot, RawValue, ShapeId,
};
use crate::engine::object::shape::{PropertyFlags, ShapeEntry};
use crate::engine::object::{
    CallableRef, DescriptorField, ObjectRef, OrdinaryPropertyDescriptor, PropertyKey,
};
use crate::engine::realm::bindings::GlobalBindingCreationMode;
use crate::engine::value::{JsString, JsValue, Value};
use std::collections::HashMap;

impl Runtime {
    /// Allocate an ordinary object whose prototype is `prototype` or null.
    pub fn new_object(&self, prototype: Option<&ObjectRef>) -> Result<ObjectRef, RuntimeError> {
        self.new_empty_object_with(prototype, ObjectData::ordinary)
    }

    pub(crate) fn new_iterator_object(
        &self,
        prototype: &ObjectRef,
    ) -> Result<ObjectRef, RuntimeError> {
        self.new_empty_object_with(Some(prototype), ObjectData::iterator)
    }

    pub(crate) fn new_empty_object_with(
        &self,
        prototype: Option<&ObjectRef>,
        build: fn(ShapeId, Vec<PropertySlot>) -> ObjectData,
    ) -> Result<ObjectRef, RuntimeError> {
        let _operation = self.operation();
        if prototype.is_some_and(|prototype| !prototype.belongs_to(self)) {
            return Err(RuntimeError::WrongRuntime("prototype"));
        }
        let prototype = prototype.map(ObjectRef::object_id);

        let mut state = self.0.state.borrow_mut();
        let shape = state.get_or_create_shape(prototype, &[])?;
        let object = match state.heap.allocate_object(build(shape, Vec::new())) {
            Ok(object) => object,
            Err(error) => {
                let cleanup = state.heap.release_shape(shape)?;
                state.apply_cleanup(cleanup)?;
                return Err(error.into());
            }
        };
        let cleanup = state.heap.release_shape(shape)?;
        state.apply_cleanup(cleanup)?;
        drop(state);
        Ok(ObjectRef::from_owned_handle(self.clone(), object))
    }

    /// Allocate one genuine empty Array with an explicit prototype. The
    /// non-configurable `length` data property is installed as physical slot
    /// zero so later ArraySetLength updates never depend on insertion order.
    pub(crate) fn new_empty_array_with_prototype(
        &self,
        prototype: &ObjectRef,
    ) -> Result<ObjectRef, RuntimeError> {
        let _operation = self.operation();
        if !prototype.belongs_to(self) {
            return Err(RuntimeError::WrongRuntime("Array prototype"));
        }
        let length = self.pinned_property_key(crate::engine::atom::pinned::PinnedAtom::Length)?;
        let entries = [ShapeEntry {
            atom: AtomIdx::from_raw(length.atom().raw()),
            flags: PropertyFlags::data(true, false, false),
        }];
        let mut state = self.0.state.borrow_mut();
        let shape = state.get_or_create_shape(Some(prototype.object_id()), &entries)?;
        let object = match state.heap.allocate_object(ObjectData::array(
            shape,
            vec![PropertySlot::Data(RawValue::Int(0))],
        )) {
            Ok(object) => object,
            Err(error) => {
                let cleanup = state.heap.release_shape(shape)?;
                state.apply_cleanup(cleanup)?;
                return Err(error.into());
            }
        };
        let cleanup = state.heap.release_shape(shape)?;
        state.apply_cleanup(cleanup)?;
        drop(state);
        Ok(ObjectRef::from_owned_handle(self.clone(), object))
    }

    /// Allocate an empty Array rooted in `realm`'s `%Array.prototype%`.
    pub(crate) fn new_array(&self, realm: ContextId) -> Result<ObjectRef, RuntimeError> {
        let prototype = self.0.state.borrow().heap.context(realm)?.array_prototype;
        let prototype = ObjectRef::from_borrowed_handle(self.clone(), prototype)?;
        self.new_empty_array_with_prototype(&prototype)
    }

    /// Append one element to an Array that has not observed any mutation
    /// outside its consecutive construction path. This is the common
    /// QuickJS `add_fast_array_element` substrate used by VM literals,
    /// builtin result arrays, and JSON parsing; no decimal property atom is
    /// created for the dense index.
    pub(crate) fn append_fresh_array_value(
        &self,
        array: &ObjectRef,
        value: Value,
    ) -> Result<(), RuntimeError> {
        if !array.belongs_to(self) {
            return Err(RuntimeError::WrongRuntime("Array"));
        }
        self.validate_value_domain(&value, "Array element")?;
        let converted = self.raw_property_value(&value)?;
        let raw = converted.raw();
        // Clone duplicates only the handle; the guard keeps the producer edge
        // accountable through every store-or-decline path below.
        let mut state = self.0.state.borrow_mut();
        let retained_atoms = state.retain_raw_value_atoms(std::iter::once(&raw))?;
        let appended = state
            .heap
            .append_fresh_array_dense_value(array.object_id(), raw);
        match appended {
            Ok(()) => Ok(()),
            Err(error) => {
                let released = state.release_atoms(retained_atoms);
                released?;
                Err(error.into())
            }
        }
    }

    /// Allocate a realm-correct Array and create consecutive C/W/E indexed
    /// data properties from `values`. This is the final VM-facing substrate
    /// for QuickJS `OP_array_from` and the dense prefix of Array literals.
    pub(crate) fn new_array_from_values(
        &self,
        realm: ContextId,
        values: Vec<Value>,
    ) -> Result<ObjectRef, RuntimeError> {
        for value in &values {
            self.validate_value_domain(value, "Array element")?;
        }
        let array = self.new_array(realm)?;
        for value in values {
            self.append_fresh_array_value(&array, value)?;
        }
        Ok(array)
    }

    /// Internal-value form of [`Runtime::new_array_from_values`]: consumes the
    /// values' edges after each dense store has retained its own copy.
    pub(crate) fn new_array_from_values_jsvalue(
        &self,
        realm: ContextId,
        values: Vec<crate::engine::value::JsValue>,
    ) -> Result<ObjectRef, RuntimeError> {
        let mut values = values.into_iter();
        let result = (|| {
            let array = self.new_array(realm)?;
            for value in values.by_ref() {
                self.append_fresh_array_value_jsvalue(&array, value)?;
            }
            Ok(array)
        })();
        // Allocation or publication may fail before the suffix was consumed.
        // These owners never entered the Array and must all be surrendered.
        let mut cleanup = Ok(());
        for value in values {
            let released = self.release_jsvalue(value);
            if cleanup.is_ok() {
                cleanup = released;
            }
        }
        cleanup?;
        result
    }

    /// Adopt an internal element's heap/atom edge directly into dense storage.
    /// Both success and failure consume the producer owner; a failed transaction
    /// returns its unchanged raw owner for release outside the state borrow.
    pub(crate) fn append_fresh_array_value_jsvalue(
        &self,
        array: &ObjectRef,
        value: crate::engine::value::JsValue,
    ) -> Result<(), RuntimeError> {
        if !array.belongs_to(self) {
            self.release_jsvalue(value)?;
            return Err(RuntimeError::WrongRuntime("Array"));
        }
        let appended = self
            .0
            .state
            .borrow_mut()
            .heap
            .append_fresh_array_dense_value_owned(array.object_id(), value.into_raw());
        match appended {
            Ok(()) => Ok(()),
            Err((error, raw)) => {
                self.release_jsvalue(
                    crate::engine::value::JsValue::from_raw(raw).expect("internal Array element"),
                )?;
                Err(error.into())
            }
        }
    }

    pub(crate) fn new_string_iterator(
        &self,
        realm: ContextId,
        string: JsString,
    ) -> Result<ObjectRef, RuntimeError> {
        let _operation = self.operation();
        let prototype_id = self
            .0
            .state
            .borrow()
            .heap
            .context(realm)?
            .string_iterator_prototype;
        let prototype = ObjectRef::from_borrowed_handle(self.clone(), prototype_id)?;
        let mut state = self.0.state.borrow_mut();
        let shape = state.get_or_create_shape(Some(prototype.object_id()), &[])?;
        let object =
            match state
                .heap
                .allocate_object(ObjectData::string_iterator(shape, Vec::new(), string))
            {
                Ok(object) => object,
                Err(error) => {
                    let cleanup = state.heap.release_shape(shape)?;
                    state.apply_cleanup(cleanup)?;
                    return Err(error.into());
                }
            };
        let cleanup = state.heap.release_shape(shape)?;
        state.apply_cleanup(cleanup)?;
        drop(state);
        Ok(ObjectRef::from_owned_handle(self.clone(), object))
    }

    /// A fresh result object copies the internal value handle into its data
    /// slot; the consumed producer edge is released on every exit.
    pub(crate) fn new_iterator_result_jsvalue(
        &self,
        realm: ContextId,
        value: crate::engine::value::JsValue,
        done: bool,
    ) -> Result<ObjectRef, RuntimeError> {
        let outcome = (|| {
            #[cfg(test)]
            {
                let mut state = self.0.state.borrow_mut();
                state.iterator_result_allocations = state
                    .iterator_result_allocations
                    .checked_add(1)
                    .expect("iterator-result allocation counter overflow");
            }
            let prototype_id = self.0.state.borrow().heap.context(realm)?.object_prototype;
            let prototype = ObjectRef::from_borrowed_handle(self.clone(), prototype_id)?;
            let result = self.new_object(Some(&prototype))?;
            for (name, stored) in [
                ("value", &value),
                ("done", &crate::engine::value::JsValue::Bool(done)),
            ] {
                let key = self.intern_property_key(name)?;
                match self.define_selected_set_data(&result, &key, stored, false)? {
                    crate::engine::object::operations::PropertyDefineOutcome::Defined(true) => {}
                    _ => {
                        return Err(RuntimeError::Invariant(
                            "iterator result property definition was rejected",
                        ));
                    }
                }
            }
            Ok(result)
        })();
        let released = self.release_jsvalue(value);
        match outcome {
            Ok(value) => {
                released?;
                Ok(value)
            }
            Err(error) => Err(error),
        }
    }

    pub(crate) fn new_primitive_object(
        &self,
        prototype: &ObjectRef,
        kind: PrimitiveKind,
        value: Value,
    ) -> Result<ObjectRef, RuntimeError> {
        self.new_primitive_object_with_string_length(prototype, kind, value, false)
    }

    /// Internal-value form of [`Runtime::new_primitive_object`]: consumes the
    /// wrapper payload's edges after the wrapper has retained its own copies.
    pub(crate) fn new_primitive_object_jsvalue(
        &self,
        prototype: &ObjectRef,
        kind: PrimitiveKind,
        value: JsValue,
    ) -> Result<ObjectRef, RuntimeError> {
        self.new_primitive_object_jsvalue_with_string_length(prototype, kind, value, false)
    }

    fn new_primitive_object_jsvalue_with_string_length(
        &self,
        prototype: &ObjectRef,
        kind: PrimitiveKind,
        mut value: JsValue,
        length_configurable: bool,
    ) -> Result<ObjectRef, RuntimeError> {
        let result = (|| {
            let _operation = self.operation();
            if !prototype.belongs_to(self) {
                return Err(RuntimeError::WrongRuntime("primitive prototype"));
            }
            // ToObject linearizes a rope before selecting its stored primitive.
            // Keep flat inputs' exact ID; only a genuinely new flat representation
            // receives a new arena node. Never replace a shared input node's payload.
            let string_length = if let (PrimitiveKind::String, JsValue::String(id)) = (kind, &value)
            {
                let string = self.0.state.borrow().heap.string(*id)?.clone();
                let flat = string.linearize();
                let length = flat.len();
                if !string.same_representation(&flat) {
                    let normalized = self.into_jsvalue(Value::String(flat))?;
                    let previous = std::mem::replace(&mut value, normalized);
                    self.release_jsvalue(previous)?;
                }
                Some(length)
            } else {
                None
            };
            let (data, payload_atom) = {
                let state = self.0.state.borrow();
                match (kind, &value) {
                    (PrimitiveKind::Number, JsValue::Int(value)) => {
                        (PrimitiveObjectData::Number(f64::from(*value)), None)
                    }
                    (PrimitiveKind::Number, JsValue::Float(value)) => {
                        (PrimitiveObjectData::Number(*value), None)
                    }
                    (PrimitiveKind::String, JsValue::String(id)) => {
                        (PrimitiveObjectData::String(*id), None)
                    }
                    (PrimitiveKind::Boolean, JsValue::Bool(value)) => {
                        (PrimitiveObjectData::Boolean(*value), None)
                    }
                    (PrimitiveKind::Symbol, JsValue::Symbol(index)) => {
                        let atom = state.atoms.brand(*index)?;
                        (PrimitiveObjectData::Symbol(atom), Some(atom))
                    }
                    (PrimitiveKind::BigInt, JsValue::ShortBigInt(value)) => {
                        (PrimitiveObjectData::ShortBigInt(*value), None)
                    }
                    (PrimitiveKind::BigInt, JsValue::BigInt(id)) => {
                        (PrimitiveObjectData::BigInt(*id), None)
                    }
                    _ => {
                        return Err(RuntimeError::Invariant(
                            "primitive wrapper class or payload is not implemented yet",
                        ));
                    }
                }
            };
            // allocate_object retains payload edges transactionally via object_edges.
            self.allocate_primitive_wrapper(
                prototype,
                data,
                payload_atom,
                string_length,
                length_configurable,
            )
        })();
        let released = self.release_jsvalue(value);
        match result {
            Err(error) => Err(error),
            Ok(object) => {
                released?;
                Ok(object)
            }
        }
    }

    pub(crate) fn new_string_object(
        &self,
        prototype: &ObjectRef,
        value: JsString,
        length_configurable: bool,
    ) -> Result<ObjectRef, RuntimeError> {
        self.new_primitive_object_with_string_length(
            prototype,
            PrimitiveKind::String,
            Value::String(value),
            length_configurable,
        )
    }

    pub(crate) fn new_primitive_object_with_string_length(
        &self,
        prototype: &ObjectRef,
        kind: PrimitiveKind,
        value: Value,
        string_length_configurable: bool,
    ) -> Result<ObjectRef, RuntimeError> {
        let _operation = self.operation();
        if !prototype.belongs_to(self) {
            return Err(RuntimeError::WrongRuntime("primitive prototype"));
        }
        let value = match (kind, value) {
            (PrimitiveKind::String, Value::String(value)) => {
                // QuickJS `JS_ToObject` always linearizes a rope before a
                // JS_CLASS_STRING wrapper owns its object_data payload.
                Value::String(value.linearize())
            }
            (_, value) => value,
        };
        self.validate_value_domain(&value, "primitive wrapper payload")?;
        let value = self.into_jsvalue(value)?;
        self.new_primitive_object_jsvalue_with_string_length(
            prototype,
            kind,
            value,
            string_length_configurable,
        )
    }

    fn allocate_primitive_wrapper(
        &self,
        prototype: &ObjectRef,
        data: PrimitiveObjectData,
        payload_atom: Option<Atom>,
        string_length: Option<usize>,
        string_length_configurable: bool,
    ) -> Result<ObjectRef, RuntimeError> {
        let mut state = self.0.state.borrow_mut();
        let shape = state.get_or_create_shape(Some(prototype.object_id()), &[])?;
        if let Some(atom) = payload_atom
            && let Err(error) = state.atoms.retain(atom)
        {
            let cleanup = state.heap.release_shape(shape)?;
            state.apply_cleanup(cleanup)?;
            return Err(error.into());
        }
        let object =
            match state
                .heap
                .allocate_object(ObjectData::primitive(shape, Vec::new(), data))
            {
                Ok(object) => object,
                Err(error) => {
                    if let Some(atom) = payload_atom {
                        state.atoms.release(atom)?;
                    }
                    let cleanup = state.heap.release_shape(shape)?;
                    state.apply_cleanup(cleanup)?;
                    return Err(error.into());
                }
            };
        let cleanup = state
            .heap
            .release_shape(shape)
            .map_err(RuntimeError::from)
            .and_then(|cleanup| state.apply_cleanup(cleanup));
        drop(state);
        let object = ObjectRef::from_owned_handle(self.clone(), object);
        cleanup?;
        if let Some(length) = string_length {
            let length = i32::try_from(length)
                .map(Value::Int)
                .unwrap_or_else(|_| Value::number(length as f64));
            let key = self.pinned_property_key(crate::engine::atom::pinned::PinnedAtom::Length)?;
            let defined = self.define_own_property(
                &object,
                &key,
                &OrdinaryPropertyDescriptor {
                    value: DescriptorField::Present(length),
                    writable: DescriptorField::Present(false),
                    enumerable: DescriptorField::Present(false),
                    configurable: DescriptorField::Present(string_length_configurable),
                    ..OrdinaryPropertyDescriptor::new()
                },
            )?;
            if !defined {
                return Err(RuntimeError::Invariant(
                    "String wrapper length definition was rejected",
                ));
            }
        }
        Ok(object)
    }

    pub(crate) fn new_global_object(
        &self,
        prototype: &ObjectRef,
        uninitialized_vars: &ObjectRef,
    ) -> Result<ObjectRef, RuntimeError> {
        let _operation = self.operation();
        if !prototype.belongs_to(self) || !uninitialized_vars.belongs_to(self) {
            return Err(RuntimeError::WrongRuntime("global object edge"));
        }
        let mut state = self.0.state.borrow_mut();
        let shape = state.get_or_create_shape(Some(prototype.object_id()), &[])?;
        let object = match state.heap.allocate_object(ObjectData::global_object(
            shape,
            Vec::new(),
            uninitialized_vars.object_id(),
        )) {
            Ok(object) => object,
            Err(error) => {
                let cleanup = state.heap.release_shape(shape)?;
                state.apply_cleanup(cleanup)?;
                return Err(error.into());
            }
        };
        let cleanup = state.heap.release_shape(shape)?;
        state.apply_cleanup(cleanup)?;
        drop(state);
        Ok(ObjectRef::from_owned_handle(self.clone(), object))
    }

    pub(crate) fn new_native_function(
        &self,
        prototype: &ObjectRef,
        target: NativeFunctionId,
        min_readable_args: u8,
    ) -> Result<ObjectRef, RuntimeError> {
        let _operation = self.operation();
        if !prototype.belongs_to(self) {
            return Err(RuntimeError::WrongRuntime("prototype"));
        }
        let mut state = self.0.state.borrow_mut();
        let shape = state.get_or_create_shape(Some(prototype.object_id()), &[])?;
        let object =
            match state
                .heap
                .allocate_bootstrap_native_function(ObjectData::native_function(
                    shape,
                    Vec::new(),
                    target,
                    min_readable_args,
                )) {
                Ok(object) => object,
                Err(error) => {
                    let cleanup = state.heap.release_shape(shape)?;
                    state.apply_cleanup(cleanup)?;
                    return Err(error.into());
                }
            };
        let cleanup = state.heap.release_shape(shape)?;
        state.apply_cleanup(cleanup)?;
        drop(state);
        Ok(ObjectRef::from_owned_handle(self.clone(), object))
    }

    /// Allocate a native callable after its defining realm has been
    /// published. `%Function.prototype%` cannot use this path because it is
    /// itself one of the roots needed to publish the realm.
    pub(crate) fn new_bound_native_function(
        &self,
        prototype: &ObjectRef,
        realm: ContextId,
        target: NativeFunctionId,
        min_readable_args: u8,
    ) -> Result<CallableRef, RuntimeError> {
        let _operation = self.operation();
        if !prototype.belongs_to(self) {
            return Err(RuntimeError::WrongRuntime("prototype"));
        }
        let mut state = self.0.state.borrow_mut();
        state.heap.context(realm)?;
        let shape = state.get_or_create_shape(Some(prototype.object_id()), &[])?;
        let object = match state
            .heap
            .allocate_object(ObjectData::bound_native_function(
                shape,
                Vec::new(),
                target,
                realm,
                min_readable_args,
            )) {
            Ok(object) => object,
            Err(error) => {
                let cleanup = state.heap.release_shape(shape)?;
                state.apply_cleanup(cleanup)?;
                return Err(error.into());
            }
        };
        let cleanup = state.heap.release_shape(shape)?;
        state.apply_cleanup(cleanup)?;
        drop(state);
        Ok(CallableRef::from_validated_object(
            ObjectRef::from_owned_handle(self.clone(), object),
        ))
    }

    pub(crate) fn new_bound_function(
        &self,
        realm: ContextId,
        target: &CallableRef,
        this_value: &JsValue,
        arguments: &[JsValue],
    ) -> Result<CallableRef, RuntimeError> {
        let _operation = self.operation();
        if !target.belongs_to(self) {
            return Err(RuntimeError::WrongRuntime("bound function target"));
        }
        // The object transaction retains each borrowed input edge exactly once.
        let raw_this = this_value.as_raw();
        let raw_arguments = arguments.iter().map(JsValue::as_raw).collect::<Vec<_>>();
        let is_constructor = self.is_constructor(target.as_object())?;

        let mut state = self.0.state.borrow_mut();
        let shape = {
            let created = match state
                .heap
                .context(realm)
                .map(|context| context.function_prototype)
                .map_err(RuntimeError::from)
            {
                Ok(prototype) => state.get_or_create_shape(Some(prototype), &[]),
                Err(error) => Err(error),
            };
            match created {
                Ok(shape) => shape,
                Err(error) => {
                    drop(state);
                    return Err(error);
                }
            }
        };
        let retained_atoms = match state
            .retain_raw_value_atoms(std::iter::once(&raw_this).chain(raw_arguments.iter()))
        {
            Ok(atoms) => atoms,
            Err(error) => {
                let applied = state
                    .heap
                    .release_shape(shape)
                    .map_err(RuntimeError::from)
                    .and_then(|cleanup| state.apply_cleanup(cleanup));
                drop(state);
                applied?;
                return Err(error);
            }
        };
        let object = match state.heap.allocate_object(ObjectData::bound_function(
            shape,
            Vec::new(),
            target.as_object().object_id(),
            raw_this,
            raw_arguments.into(),
            is_constructor,
        )) {
            Ok(object) => object,
            Err(error) => {
                let released = state.release_atoms(retained_atoms);
                let applied = state
                    .heap
                    .release_shape(shape)
                    .map_err(RuntimeError::from)
                    .and_then(|cleanup| state.apply_cleanup(cleanup));
                drop(state);
                released?;
                applied?;
                return Err(error.into());
            }
        };
        let finalized = state
            .heap
            .release_shape(shape)
            .map_err(RuntimeError::from)
            .and_then(|cleanup| state.apply_cleanup(cleanup));
        drop(state);
        // The bound function retained its own copy edges; the guards balance
        // the boundary conversions' producer edges.
        finalized?;
        Ok(CallableRef::from_validated_object(
            ObjectRef::from_owned_handle(self.clone(), object),
        ))
    }

    /// Allocate a fully initialized realm-bound native builtin. Internal
    /// readable arity remains in the payload while own `length` is an
    /// independent configurable ordinary property.
    pub(crate) fn new_native_builtin(
        &self,
        prototype: &ObjectRef,
        realm: ContextId,
        target: NativeFunctionId,
        min_readable_args: u8,
        name: &str,
        length: i32,
    ) -> Result<CallableRef, RuntimeError> {
        let callable =
            self.new_bound_native_function(prototype, realm, target, min_readable_args)?;
        self.define_function_data_property(
            callable.as_object(),
            "length",
            Value::Int(length),
            false,
            true,
        )?;
        self.define_function_data_property(
            callable.as_object(),
            "name",
            Value::String(JsString::try_from_utf8(name)?),
            false,
            true,
        )?;
        Ok(callable)
    }

    /// Return whether `object` carries the genuine Array exotic class tag.
    /// Prototype spoofing alone never makes an ordinary object an Array.
    pub fn is_array_object(&self, object: &ObjectRef) -> Result<bool, RuntimeError> {
        let _operation = self.operation();
        if !object.belongs_to(self) {
            return Err(RuntimeError::WrongRuntime("object"));
        }
        Ok(matches!(
            self.0
                .state
                .borrow()
                .heap
                .object(object.object_id())?
                .payload,
            ObjectPayload::Array { .. }
        ))
    }

    /// Return the object's `[[Construct]]` capability bit. Callability and
    /// constructability are intentionally independent, as in QuickJS.
    pub fn is_constructor(&self, object: &ObjectRef) -> Result<bool, RuntimeError> {
        let _operation = self.operation();
        if !object.belongs_to(self) {
            return Err(RuntimeError::WrongRuntime("object"));
        }
        Ok(self
            .0
            .state
            .borrow()
            .heap
            .object(object.object_id())?
            .is_constructor)
    }

    /// Set the object's `[[Construct]]` capability independently of its call
    /// protocol, matching QuickJS `JS_SetConstructorBit`.
    pub(crate) fn set_constructor_bit(
        &self,
        object: &ObjectRef,
        enabled: bool,
    ) -> Result<(), RuntimeError> {
        let _operation = self.operation();
        if !object.belongs_to(self) {
            return Err(RuntimeError::WrongRuntime("object"));
        }
        self.0
            .state
            .borrow_mut()
            .heap
            .set_object_constructor_bit(object.object_id(), enabled)?;
        Ok(())
    }

    /// Promote an ordinary object root to a checked callable capability.
    /// Returns `None` for objects without `[[Call]]`; runtime-domain and stale
    /// handle failures remain explicit errors.
    pub fn as_callable(&self, object: &ObjectRef) -> Result<Option<CallableRef>, RuntimeError> {
        self.as_callable_object(object.object_id())
    }

    /// Handle form of [`Runtime::as_callable`]; borrows the object's edge.
    pub(crate) fn as_callable_object(
        &self,
        object: crate::engine::heap::ObjectId,
    ) -> Result<Option<CallableRef>, RuntimeError> {
        let _operation = self.operation();
        if !self.object_id_has_call_capability(object)? {
            return Ok(None);
        }
        Ok(Some(CallableRef::from_validated_object(
            ObjectRef::from_borrowed_handle(self.clone(), object)?,
        )))
    }

    fn object_id_has_call_capability(
        &self,
        object: crate::engine::heap::ObjectId,
    ) -> Result<bool, RuntimeError> {
        Ok(matches!(
            self.0.state.borrow().heap.object(object)?.payload,
            crate::engine::heap::ObjectPayload::NativeFunction { .. }
                | crate::engine::heap::ObjectPayload::BoundFunction { .. }
                | crate::engine::heap::ObjectPayload::BytecodeFunction { .. }
                | crate::engine::heap::ObjectPayload::Proxy(crate::engine::heap::ProxyData {
                    is_callable: true,
                    ..
                })
        ))
    }

    /// The inner error returns the unchanged non-callable owner so callers can
    /// continue Proxy classification without retaining a second object root.
    pub(crate) fn try_into_callable(
        &self,
        object: ObjectRef,
    ) -> Result<Result<CallableRef, ObjectRef>, RuntimeError> {
        let _operation = self.operation();
        if self.object_has_call_capability(&object)? {
            Ok(Ok(CallableRef::from_validated_object(object)))
        } else {
            Ok(Err(object))
        }
    }

    /// Shared payload authority. Both callers hold an operation boundary and
    /// keep the root alive through the immutable heap lookup and promotion.
    fn object_has_call_capability(&self, object: &ObjectRef) -> Result<bool, RuntimeError> {
        if !object.belongs_to(self) {
            return Err(RuntimeError::WrongRuntime("object"));
        }
        Ok(matches!(
            self.0
                .state
                .borrow()
                .heap
                .object(object.object_id())?
                .payload,
            ObjectPayload::NativeFunction { .. }
                | ObjectPayload::BoundFunction { .. }
                | ObjectPayload::BytecodeFunction { .. }
                | ObjectPayload::Proxy(crate::engine::heap::ProxyData {
                    is_callable: true,
                    ..
                })
        ))
    }

    /// Instantiate one runtime-owned bytecode node as a callable object in the
    /// caller's realm, matching QuickJS's `js_closure` boundary.
    pub(crate) fn new_bytecode_closure(
        &self,
        caller_realm: ContextId,
        function: &FunctionBytecodeRef,
    ) -> Result<CallableRef, RuntimeError> {
        if !function.belongs_to(self) {
            return Err(RuntimeError::WrongRuntime("function bytecode"));
        }
        self.ensure_dynamic_import_bytecode_tree_authorized(function)?;
        let descriptors = {
            let state = self.0.state.borrow();
            let bytecode = state.heap.function_bytecode(function.bytecode_id())?;
            bytecode.closure_variables.clone()
        };
        // QuickJS checks every GLOBAL_DECL before creating any binding. A
        // later redeclaration must not leave earlier declarations installed.
        for descriptor in descriptors.iter().copied() {
            if !matches!(
                descriptor.source,
                ClosureSource::GlobalDeclaration | ClosureSource::Global
            ) {
                return Err(RuntimeError::Invariant(
                    "root bytecode closure descriptor did not use a root global source",
                ));
            }
            let ClosureVariableName::Atom(name) = descriptor.name else {
                return Err(RuntimeError::Invariant(
                    "published global closure descriptor has no atom",
                ));
            };
            if descriptor.source == ClosureSource::Global
                && descriptor.kind != ClosureVariableKind::Normal
            {
                return Err(RuntimeError::Invariant(
                    "resolved global has declaration-only binding metadata",
                ));
            }
            if descriptor.source == ClosureSource::GlobalDeclaration {
                let key = PropertyKey::from_borrowed_atom(self.clone(), name)?;
                match descriptor.kind {
                    ClosureVariableKind::Normal if descriptor.is_lexical => {
                        self.check_global_lexical_declaration(caller_realm, &key)?;
                    }
                    ClosureVariableKind::Normal => {
                        self.check_global_var_declaration(caller_realm, &key)?;
                    }
                    ClosureVariableKind::GlobalFunction
                        if !descriptor.is_lexical && !descriptor.is_const =>
                    {
                        self.check_global_function_declaration(caller_realm, &key)?;
                    }
                    ClosureVariableKind::ModuleImportView
                    | ClosureVariableKind::FunctionName
                    | ClosureVariableKind::GlobalFunction
                    | ClosureVariableKind::EvalVariableObject
                    | ClosureVariableKind::ArgEvalVariableObject
                    | ClosureVariableKind::WithObject
                    | ClosureVariableKind::PrivateField
                    | ClosureVariableKind::PrivateMethod
                    | ClosureVariableKind::PrivateGetter
                    | ClosureVariableKind::PrivateSetter
                    | ClosureVariableKind::PrivateGetterSetter => {
                        return Err(RuntimeError::Invariant(
                            "global declaration has non-global binding metadata",
                        ));
                    }
                }
            }
        }
        let mut slots = Vec::with_capacity(descriptors.len());
        let mut first_lexical_roots: HashMap<Atom, VarRefRoot> = HashMap::new();
        for descriptor in descriptors.iter().copied() {
            let ClosureVariableName::Atom(name) = descriptor.name else {
                return Err(RuntimeError::Invariant(
                    "published global closure descriptor has no atom",
                ));
            };
            let root = match descriptor.source {
                ClosureSource::GlobalDeclaration => {
                    let key = PropertyKey::from_borrowed_atom(self.clone(), name)?;
                    match descriptor.kind {
                        ClosureVariableKind::Normal if descriptor.is_lexical => {
                            if let Some(root) = first_lexical_roots.get(&name) {
                                root.clone()
                            } else {
                                let root = self.create_global_lexical_binding(
                                    caller_realm,
                                    &key,
                                    descriptor.is_const,
                                    None,
                                )?;
                                first_lexical_roots.insert(name, root.clone());
                                root
                            }
                        }
                        ClosureVariableKind::Normal => self.create_global_var_binding(
                            caller_realm,
                            &key,
                            GlobalBindingCreationMode::Script,
                        )?,
                        ClosureVariableKind::GlobalFunction
                            if !descriptor.is_lexical && !descriptor.is_const =>
                        {
                            self.create_global_function_binding(
                                caller_realm,
                                &key,
                                GlobalBindingCreationMode::Script,
                            )?
                        }
                        ClosureVariableKind::ModuleImportView
                        | ClosureVariableKind::FunctionName
                        | ClosureVariableKind::GlobalFunction
                        | ClosureVariableKind::EvalVariableObject
                        | ClosureVariableKind::ArgEvalVariableObject
                        | ClosureVariableKind::WithObject
                        | ClosureVariableKind::PrivateField
                        | ClosureVariableKind::PrivateMethod
                        | ClosureVariableKind::PrivateGetter
                        | ClosureVariableKind::PrivateSetter
                        | ClosureVariableKind::PrivateGetterSetter => {
                            return Err(RuntimeError::Invariant(
                                "global declaration has non-global binding metadata",
                            ));
                        }
                    }
                }
                ClosureSource::Global => self.resolve_global_var(caller_realm, name)?,
                ClosureSource::ParentLocal(_)
                | ClosureSource::ParentArgument(_)
                | ClosureSource::ParentClosure(_)
                | ClosureSource::ParentGlobal(_)
                | ClosureSource::EvalEnvironment(_) => {
                    return Err(RuntimeError::Invariant(
                        "root bytecode closure descriptor used a child source",
                    ));
                }
                ClosureSource::ModuleDeclaration
                | ClosureSource::ModuleImport
                | ClosureSource::ModuleImportCollision
                | ClosureSource::ModuleImportMeta => {
                    return Err(RuntimeError::Invariant(
                        "ordinary root publication received a module descriptor",
                    ));
                }
            };
            slots.push(root);
        }
        self.new_bytecode_closure_with_slots(caller_realm, function, &slots)
    }
}

#[cfg(test)]
mod owned_callable_tests {
    use super::*;
    use crate::engine::vm::call::DirectCallTarget;

    #[test]
    fn internal_array_adopts_edges_and_releases_rejected_input() {
        use crate::engine::value::JsValue;
        let runtime = Runtime::new();
        let context = runtime.new_context();
        let string = runtime
            .into_jsvalue(Value::String(JsString::from_static("owned")))
            .unwrap();
        let JsValue::String(string_id) = &string else {
            unreachable!()
        };
        let string_id = *string_id;
        let before = runtime.heap_counts().string_nodes;
        let array = runtime
            .new_array_from_values_jsvalue(context.realm, vec![string])
            .unwrap();
        assert_eq!(runtime.heap_counts().string_nodes, before);
        {
            let state = runtime.0.state.borrow();
            let stored = state
                .heap
                .object(array.object_id())
                .unwrap()
                .dense_array_value(0)
                .unwrap();
            assert!(matches!(stored, RawValue::String(id) if *id == string_id));
        }
        drop(array);
        runtime.run_gc().unwrap();
        assert!(runtime.0.state.borrow().heap.string(string_id).is_err());

        let object = runtime.new_object(None).unwrap();
        let rejected = runtime
            .into_jsvalue(Value::String(JsString::from_static("rejected")))
            .unwrap();
        let JsValue::String(rejected_id) = &rejected else {
            unreachable!()
        };
        let rejected_id = *rejected_id;
        assert!(
            runtime
                .append_fresh_array_value_jsvalue(&object, rejected)
                .is_err()
        );
        runtime.run_gc().unwrap();
        assert!(runtime.0.state.borrow().heap.string(rejected_id).is_err());
    }

    #[test]
    fn owned_and_borrowed_callable_promotion_share_payload_rules() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        for source in [
            "(function(){})",
            "Math.min",
            "(function(){}).bind(null)",
            "new Proxy(function(){},{apply(){throw 'not during classification';}})",
            "(()=>{let r=Proxy.revocable(function(){},{});r.revoke();return r.proxy;})()",
        ] {
            let Value::Object(object) = context.eval(source).unwrap() else {
                panic!("expected object");
            };
            let id = object.object_id();
            let before = std::rc::Rc::strong_count(&runtime.0);
            let borrowed = runtime.as_callable(&object).unwrap().unwrap();
            assert_eq!(std::rc::Rc::strong_count(&runtime.0), before + 1);
            assert_eq!(borrowed.as_object().object_id(), id);
            drop(borrowed);
            let owned = runtime.try_into_callable(object).unwrap().unwrap();
            // This measures Runtime handle owners, not GC edge counts: promotion
            // transports the existing handle instead of cloning a second one.
            assert_eq!(std::rc::Rc::strong_count(&runtime.0), before);
            assert_eq!(owned.as_object().object_id(), id);
        }
        for source in ["({})", "new Proxy({}, {})"] {
            let Value::Object(object) = context.eval(source).unwrap() else {
                panic!("expected object");
            };
            let id = object.object_id();
            assert!(runtime.as_callable(&object).unwrap().is_none());
            let object = runtime.try_into_callable(object).unwrap().unwrap_err();
            assert_eq!(object.object_id(), id);
        }
    }

    #[test]
    fn owned_direct_call_promotion_preserves_domain_and_proxy_errors() {
        let runtime = Runtime::new();
        let foreign = Runtime::new();
        let object = foreign.new_object(None).unwrap();
        assert!(matches!(
            runtime.direct_call_target_from_value(Value::Object(object)),
            Err(RuntimeError::WrongRuntime("call target"))
        ));
        assert!(matches!(
            runtime.direct_call_target_from_value(Value::Int(1)),
            Err(RuntimeError::Engine(_))
        ));
        assert!(matches!(
            runtime.direct_call_target_from_value(Value::Object(runtime.new_object(None).unwrap())),
            Err(RuntimeError::Engine(_))
        ));
        let mut context = runtime.new_context();
        let value = context
            .eval("new Proxy({}, {get(){throw 'not during classification';}})")
            .unwrap();
        assert!(matches!(
            runtime.direct_call_target_from_value(value).unwrap(),
            DirectCallTarget::NonCallableProxy(_)
        ));
        let value = context
            .eval("new Proxy(function(){}, {get(){throw 'not during classification';}})")
            .unwrap();
        assert!(matches!(
            runtime.direct_call_target_from_value(value).unwrap(),
            DirectCallTarget::Callable(_)
        ));
    }

    #[test]
    fn owned_callable_promotion_keeps_the_operation_cleanup_boundary() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let Value::Object(object) = context.eval("(function(){})").unwrap() else {
            panic!("expected function");
        };
        let doomed = runtime.new_object(None).unwrap();
        {
            let _state = runtime.0.state.borrow();
            drop(doomed);
        }
        assert!(runtime.0.deferred_references.has_pending());
        let callable = runtime.try_into_callable(object).unwrap().unwrap();
        assert!(!runtime.0.deferred_references.has_pending());
        assert!(callable.belongs_to(&runtime));
    }
}
