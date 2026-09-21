//! `%Proxy%` allocation and revocation lifecycle.
//!
//! Observable internal-method dispatch lives in the runtime's exotic-method
//! layer. This module mirrors the pinned QuickJS constructor boundary:
//! genuine Proxy objects have a null physical prototype, cache the target's
//! call/construct capabilities at creation time, and retain target and handler
//! even after a one-shot revocation closure is consumed.

use crate::engine::api::error::NativeErrorKind;
use crate::engine::api::runtime::Runtime;
use crate::engine::api::runtime_error::RuntimeError;
use crate::engine::builtins::native::NativeFunctionId;

use crate::engine::heap::{ContextId, InternalCallableData, ObjectData, ObjectPayload};
use crate::engine::object::{DescriptorField, ObjectRef, OrdinaryPropertyDescriptor};
use crate::engine::value::conversion::NativeConversion;
use crate::engine::value::{JsValue, Value};
use crate::engine::vm::Completion;

use crate::engine::vm::call::{NativeArguments, NativeInvocation};

fn runtime_value_at<'a>(
    arguments: &'a NativeArguments,
    index: usize,
    message: &'static str,
) -> Result<&'a JsValue, RuntimeError> {
    arguments
        .readable
        .get(index)
        .ok_or(RuntimeError::Invariant(message))
}

impl Runtime {
    /// Publish `%Proxy%` with QuickJS's exact initial own-property surface.
    ///
    /// The constructor owns `length`, `name`, then `revocable` in that order.
    /// It deliberately has no own `prototype` property.
    pub(crate) fn initialize_proxy_intrinsic(
        &self,
        realm: ContextId,
        function_prototype: &ObjectRef,
        global_object: &ObjectRef,
    ) -> Result<(), RuntimeError> {
        let constructor = self.new_native_builtin(
            function_prototype,
            realm,
            NativeFunctionId::ProxyConstructor,
            2,
            "Proxy",
            2,
        )?;
        self.set_constructor_bit(constructor.as_object(), true)?;
        self.define_native_builtin_auto_init(
            constructor.as_object(),
            realm,
            NativeFunctionId::ProxyRevocable,
            "revocable",
            2,
            2,
        )?;
        self.define_function_data_property(
            global_object,
            "Proxy",
            Value::Object(constructor.as_object().clone()),
            true,
            true,
        )
    }

    /// Allocate one genuine Proxy after validating the two object operands.
    ///
    /// QuickJS uses a null physical prototype for `JS_CLASS_PROXY`; every
    /// observable prototype operation is therefore handled by the exotic
    /// internal-method dispatcher rather than this shape.
    #[cfg(test)]
    pub(crate) fn new_proxy(
        &self,
        realm: ContextId,
        target: Value,
        handler: Value,
    ) -> Result<NativeConversion<ObjectRef>, RuntimeError> {
        let (Value::Object(target), Value::Object(handler)) = (&target, &handler) else {
            return Ok(NativeConversion::Throw(self.new_native_error(
                realm,
                NativeErrorKind::Type,
                "not an object",
            )?));
        };
        if !target.belongs_to(self) || !handler.belongs_to(self) {
            return Err(RuntimeError::WrongRuntime("Proxy target or handler"));
        }
        self.new_proxy_jsvalue(
            realm,
            &JsValue::Object(target.object_id()),
            &JsValue::Object(handler.object_id()),
        )
    }

    fn new_proxy_jsvalue(
        &self,
        realm: ContextId,
        target: &JsValue,
        handler: &JsValue,
    ) -> Result<NativeConversion<ObjectRef>, RuntimeError> {
        let (JsValue::Object(target), JsValue::Object(handler)) = (target, handler) else {
            return Ok(NativeConversion::Throw(self.new_native_error(
                realm,
                NativeErrorKind::Type,
                "not an object",
            )?));
        };

        let (is_callable, is_constructor) = {
            let state = self.0.state.borrow();
            let target_data = state.heap.object(*target)?;
            let is_callable = matches!(
                &target_data.payload,
                ObjectPayload::NativeFunction { .. }
                    | ObjectPayload::BoundFunction { .. }
                    | ObjectPayload::BytecodeFunction { .. }
                    | ObjectPayload::Proxy(crate::engine::heap::ProxyData {
                        is_callable: true,
                        ..
                    })
            );
            (is_callable, target_data.is_constructor)
        };

        let mut state = self.0.state.borrow_mut();
        let shape = state.get_or_create_shape(None, &[])?;
        let object = match state.heap.allocate_object(ObjectData::proxy(
            shape,
            Vec::new(),
            *target,
            *handler,
            is_callable,
            is_constructor,
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

        Ok(NativeConversion::Value(ObjectRef::from_owned_handle(
            self.clone(),
            object,
        )))
    }

    /// Native `%Proxy%` constructor entrypoint.
    pub(crate) fn call_proxy_constructor(
        &self,
        realm: ContextId,
        invocation: NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Completion, RuntimeError> {
        let NativeInvocation::Construct { .. } = &invocation else {
            let _ = invocation.release(self);
            return Err(RuntimeError::Invariant(
                "Proxy constructor did not receive a constructor invocation",
            ));
        };
        invocation.release(self)?;
        let target = runtime_value_at(arguments, 0, "Proxy target argv was not padded")?;
        let handler = runtime_value_at(arguments, 1, "Proxy handler argv was not padded")?;
        match self.new_proxy_jsvalue(realm, target, handler)? {
            NativeConversion::Value(proxy) => {
                Ok(Completion::Return(JsValue::Object(proxy.into_handle())))
            }
            NativeConversion::Throw(value) => Ok(Completion::Throw(self.into_jsvalue(value)?)),
        }
    }

    /// Native `Proxy.revocable` entrypoint.
    pub(crate) fn call_proxy_revocable(
        &self,
        realm: ContextId,
        invocation: NativeInvocation,
        arguments: &NativeArguments,
    ) -> Result<Completion, RuntimeError> {
        let NativeInvocation::Call { .. } = &invocation else {
            let _ = invocation.release(self);
            return Err(RuntimeError::Invariant(
                "Proxy.revocable did not receive a call invocation",
            ));
        };
        invocation.release(self)?;
        let target = runtime_value_at(arguments, 0, "Proxy.revocable target argv was not padded")?;
        let handler =
            runtime_value_at(arguments, 1, "Proxy.revocable handler argv was not padded")?;
        let proxy = match self.new_proxy_jsvalue(realm, target, handler)? {
            NativeConversion::Value(proxy) => proxy,
            NativeConversion::Throw(value) => {
                return Ok(Completion::Throw(self.into_jsvalue(value)?));
            }
        };
        let revoke = self.new_internal_promise_function(
            realm,
            NativeFunctionId::ProxyRevoke,
            0,
            0,
            InternalCallableData::ProxyRevoke {
                proxy: Some(proxy.object_id()),
            },
        )?;

        let object_prototype = self.0.state.borrow().heap.context(realm)?.object_prototype;
        let object_prototype = ObjectRef::from_borrowed_handle(self.clone(), object_prototype)?;
        let result = self.new_object(Some(&object_prototype))?;
        for (name, value) in [
            ("proxy", Value::Object(proxy)),
            ("revoke", Value::Object(revoke.as_object().clone())),
        ] {
            let key = self.intern_property_key(name)?;
            if !self.define_own_property(
                &result,
                &key,
                &OrdinaryPropertyDescriptor {
                    value: DescriptorField::Present(value),
                    writable: DescriptorField::Present(true),
                    enumerable: DescriptorField::Present(true),
                    configurable: DescriptorField::Present(true),
                    ..OrdinaryPropertyDescriptor::new()
                },
            )? {
                return Err(RuntimeError::Invariant(
                    "Proxy.revocable result property definition was rejected",
                ));
            }
        }
        Ok(Completion::Return(JsValue::Object(result.into_handle())))
    }

    /// Consume a revocation closure's capture exactly once.
    pub(crate) fn call_proxy_revoke(
        &self,
        invocation: NativeInvocation,
    ) -> Result<Completion, RuntimeError> {
        let NativeInvocation::Call { .. } = &invocation else {
            let _ = invocation.release(self);
            return Err(RuntimeError::Invariant(
                "Proxy revoke function did not receive a call invocation",
            ));
        };
        invocation.release(self)?;
        let active = self.active_function()?;
        let mut state = self.0.state.borrow_mut();
        let (_, cleanup) = state.heap.revoke_proxy_from_callable(active.object_id())?;
        state.apply_cleanup(cleanup)?;
        Ok(Completion::Return(JsValue::Undefined))
    }
}
