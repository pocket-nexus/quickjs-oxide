//! Context-aware object creation and property operations.

use super::*;

impl Context {
    #[cfg(test)]
    pub fn create_global_lexical_for_test(
        &self,
        name: &str,
        is_const: bool,
        initial_value: Option<Value>,
    ) -> Result<(), RuntimeError> {
        self.runtime
            .create_global_lexical_for_test(self.realm, name, is_const, initial_value)
    }

    #[cfg(test)]
    pub fn initialize_global_lexical_for_test(
        &self,
        name: &str,
        value: Value,
    ) -> Result<(), RuntimeError> {
        self.runtime
            .initialize_global_lexical_for_test(self.realm, name, value)
    }

    /// Allocate an ordinary object with this realm's `%Object.prototype%`.
    pub fn new_object(&mut self) -> Result<ObjectRef, RuntimeError> {
        let prototype = self.object_prototype()?;
        self.runtime.new_object(Some(&prototype))
    }

    /// Allocate one genuine empty Array in this realm.
    pub fn new_array(&mut self) -> Result<ObjectRef, RuntimeError> {
        self.runtime.new_array(self.realm)
    }

    /// Allocate one genuine Array initialized from consecutive values.
    pub fn new_array_from_values(&mut self, values: Vec<Value>) -> Result<ObjectRef, RuntimeError> {
        self.runtime.new_array_from_values(self.realm, values)
    }

    /// Allocate an ordinary object with an explicit object-or-null prototype.
    pub fn new_object_with_prototype(
        &mut self,
        prototype: Option<&ObjectRef>,
    ) -> Result<ObjectRef, RuntimeError> {
        self.runtime.new_object(prototype)
    }

    pub fn get_own_property(
        &mut self,
        object: &ObjectRef,
        key: &PropertyKey,
    ) -> Result<Option<CompleteOrdinaryPropertyDescriptor>, RuntimeError> {
        let result = crate::engine::vm::entry::own(&self.runtime, self.realm, object, key)?;

        match result {
            NativeConversion::Value(value) => Ok(value),
            NativeConversion::Throw(value) => {
                self.runtime.set_pending_exception_jsvalue(value)?;
                Err(RuntimeError::Exception)
            }
        }
    }

    pub fn define_own_property(
        &mut self,
        object: &ObjectRef,
        key: &PropertyKey,
        descriptor: &OrdinaryPropertyDescriptor,
    ) -> Result<bool, RuntimeError> {
        {
            self.finish_property_bool(crate::engine::vm::entry::define(
                &self.runtime,
                self.realm,
                object,
                key,
                descriptor,
            )?)
        }
    }

    pub fn get_property(
        &mut self,
        object: &ObjectRef,
        key: &PropertyKey,
    ) -> Result<Value, RuntimeError> {
        let completion = crate::engine::vm::entry::get(
            &self.runtime,
            self.realm,
            object,
            key,
            Value::Object(object.clone()),
        )?;

        self.finish_completion(completion)
    }

    pub fn get_property_with_receiver(
        &mut self,
        object: &ObjectRef,
        key: &PropertyKey,
        receiver: Value,
    ) -> Result<Value, RuntimeError> {
        let completion =
            crate::engine::vm::entry::get(&self.runtime, self.realm, object, key, receiver)?;

        self.finish_completion(completion)
    }

    pub fn set_property(
        &mut self,
        object: &ObjectRef,
        key: &PropertyKey,
        value: Value,
    ) -> Result<bool, RuntimeError> {
        {
            self.finish_property_bool(crate::engine::vm::entry::set(
                &self.runtime,
                self.realm,
                object,
                key,
                value,
                Value::Object(object.clone()),
            )?)
        }
    }

    pub fn set_property_with_receiver(
        &mut self,
        object: &ObjectRef,
        key: &PropertyKey,
        value: Value,
        receiver: Value,
    ) -> Result<bool, RuntimeError> {
        {
            self.finish_property_bool(crate::engine::vm::entry::set(
                &self.runtime,
                self.realm,
                object,
                key,
                value,
                receiver,
            )?)
        }
    }

    fn finish_property_bool(
        &mut self,
        result: NativeConversion<bool>,
    ) -> Result<bool, RuntimeError> {
        match result {
            NativeConversion::Value(value) => Ok(value),
            NativeConversion::Throw(value) => {
                self.runtime.set_pending_exception_jsvalue(value)?;
                Err(RuntimeError::Exception)
            }
        }
    }
}
