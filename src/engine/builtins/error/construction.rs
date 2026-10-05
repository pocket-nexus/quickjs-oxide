use crate::engine::api::error::{Error, NativeErrorKind, NativeErrorMessage};
use crate::engine::api::runtime::Runtime;
use crate::engine::api::runtime_error::RuntimeError;

use crate::engine::heap::runtime::{RuntimeState, owned_values::OwnedValueGuard};
use crate::engine::heap::{ContextId, ObjectData, ObjectId, ObjectKind, RawValue};
use crate::engine::object::ObjectRef;
use crate::engine::object::property::PropertyDescriptor;
use crate::engine::value::{JsValue, Value};
use crate::engine::vm::frames::ActiveFrameKind;
use std::cell::Cell;

impl Runtime {
    /// Internal-value form of native Error construction: the returned value
    /// owns the new error object's single edge.
    pub(crate) fn new_native_error_jsvalue(
        &self,
        realm: ContextId,
        kind: NativeErrorKind,
        message: &str,
    ) -> Result<JsValue, RuntimeError> {
        self.new_native_error_from_message_jsvalue(
            realm,
            kind,
            NativeErrorMessage::from_utf8(message),
        )
    }

    #[cfg(test)]
    pub(crate) fn new_native_error(
        &self,
        realm: ContextId,
        kind: NativeErrorKind,
        message: &str,
    ) -> Result<Value, RuntimeError> {
        self.new_native_error_from_message(realm, kind, NativeErrorMessage::from_utf8(message))
    }

    /// Internal-value form of [`Runtime::new_native_error_from_error`].
    pub(crate) fn new_native_error_from_error_jsvalue(
        &self,
        realm: ContextId,
        kind: NativeErrorKind,
        error: &Error,
    ) -> Result<JsValue, RuntimeError> {
        let message = error
            .native_message()
            .cloned()
            .unwrap_or_else(|| NativeErrorMessage::from_utf8(error.message()));
        self.new_native_error_from_message_jsvalue(realm, kind, message)
    }

    pub(crate) fn new_native_error_from_error(
        &self,
        realm: ContextId,
        kind: NativeErrorKind,
        error: &Error,
    ) -> Result<Value, RuntimeError> {
        let message = error
            .native_message()
            .cloned()
            .unwrap_or_else(|| NativeErrorMessage::from_utf8(error.message()));
        self.new_native_error_from_message(realm, kind, message)
    }

    /// Internal-value form of [`Runtime::new_native_error_from_message`].
    pub(crate) fn new_native_error_from_message_jsvalue(
        &self,
        realm: ContextId,
        kind: NativeErrorKind,
        message: NativeErrorMessage,
    ) -> Result<JsValue, RuntimeError> {
        let _operation = self.operation()?;
        let object = self.0.state.borrow_mut().new_native_error_from_message(
            &self.0.poisoned,
            realm,
            kind,
            message,
        )?;
        Ok(JsValue::Object(object))
    }

    pub(crate) fn new_native_error_from_message(
        &self,
        realm: ContextId,
        kind: NativeErrorKind,
        message: NativeErrorMessage,
    ) -> Result<Value, RuntimeError> {
        let JsValue::Object(object) =
            self.new_native_error_from_message_jsvalue(realm, kind, message)?
        else {
            unreachable!("native Error factory allocated an object")
        };
        Ok(Value::Object(ObjectRef::from_owned_handle(
            self.clone(),
            object,
        )))
    }

    #[cfg(test)]
    pub(crate) fn new_native_error_without_backtrace_from_error(
        &self,
        realm: ContextId,
        kind: NativeErrorKind,
        error: &Error,
    ) -> Result<Value, RuntimeError> {
        let value =
            self.new_native_error_without_backtrace_from_error_jsvalue(realm, kind, error)?;
        self.root_and_release_jsvalue(value)
    }

    pub(crate) fn new_native_error_without_backtrace_from_error_jsvalue(
        &self,
        realm: ContextId,
        kind: NativeErrorKind,
        error: &Error,
    ) -> Result<JsValue, RuntimeError> {
        let message = error
            .native_message()
            .cloned()
            .unwrap_or_else(|| NativeErrorMessage::from_utf8(error.message()));
        self.new_native_error_without_backtrace_from_message_jsvalue(realm, kind, message)
    }

    /// Allocate one owned native Error message without backtrace completion.
    pub(crate) fn new_native_error_without_backtrace_from_message_jsvalue(
        &self,
        realm: ContextId,
        kind: NativeErrorKind,
        message: NativeErrorMessage,
    ) -> Result<JsValue, RuntimeError> {
        let _operation = self.operation()?;
        let object = self
            .0
            .state
            .borrow_mut()
            .new_native_error_without_backtrace_from_message(
                &self.0.poisoned,
                realm,
                kind,
                message,
            )?;
        Ok(JsValue::Object(object))
    }

    #[cfg(test)]
    pub(crate) fn new_native_error_without_backtrace_from_message(
        &self,
        realm: ContextId,
        kind: NativeErrorKind,
        message: NativeErrorMessage,
    ) -> Result<Value, RuntimeError> {
        let JsValue::Object(object) =
            self.new_native_error_without_backtrace_from_message_jsvalue(realm, kind, message)?
        else {
            unreachable!("native Error factory allocated an object")
        };
        Ok(Value::Object(ObjectRef::from_owned_handle(
            self.clone(),
            object,
        )))
    }

    pub(crate) fn new_error_object(
        &self,
        prototype: &ObjectRef,
    ) -> Result<ObjectRef, RuntimeError> {
        let _operation = self.operation()?;
        if !prototype.belongs_to(self) {
            return Err(RuntimeError::WrongRuntime("Error prototype"));
        }
        let mut state = self.0.state.borrow_mut();
        let object = state.allocate_object_with_layout_with_poison(
            &self.0.poisoned,
            Some(prototype.object_id()),
            &[],
            Vec::new(),
            ObjectData::error,
        )?;
        drop(state);
        Ok(ObjectRef::from_owned_handle(self.clone(), object))
    }

    /// Return whether `object` carries the native Error class tag. Prototype
    /// spoofing alone does not make an object an Error.
    pub fn is_error_object(&self, object: &ObjectRef) -> Result<bool, RuntimeError> {
        let _operation = self.operation()?;
        if !object.belongs_to(self) {
            return Err(RuntimeError::WrongRuntime("object"));
        }
        Ok(self.0.state.borrow().heap.object(object.object_id())?.kind == ObjectKind::Error)
    }
}

impl RuntimeState {
    /// Build the native Error and capture immediately only when the already
    /// materialized frame registry permits the existing native/no-frame rule.
    /// Bytecode callers still complete the fault frame before later capture.
    pub(crate) fn new_native_error_from_message(
        &mut self,
        poisoned: &Cell<bool>,
        realm: ContextId,
        kind: NativeErrorKind,
        message: NativeErrorMessage,
    ) -> Result<ObjectId, RuntimeError> {
        let object =
            self.new_native_error_without_backtrace_from_message(poisoned, realm, kind, message)?;
        let mut result_owner = OwnedValueGuard::new(self, poisoned, JsValue::Object(object));
        let (state, result_owner) = result_owner.parts();
        let capture_now = state
            .active_frames
            .last()
            .is_none_or(|frame| matches!(frame.kind, ActiveFrameKind::Native { .. }));
        if capture_now {
            state.complete_fresh_native_error_backtrace(poisoned, object, false, None)?;
        }
        let JsValue::Object(object) = result_owner.take().expect("native Error result") else {
            unreachable!("native Error factory allocated an object")
        };
        Ok(object)
    }

    /// Construct a fresh native Error message without callbacks or public roots.
    /// The returned ObjectId owns one edge; backtrace completion remains outside
    /// this state access so the executing fault frame can first be materialized.
    pub(crate) fn new_native_error_without_backtrace_from_message(
        &mut self,
        poisoned: &Cell<bool>,
        realm: ContextId,
        kind: NativeErrorKind,
        message: NativeErrorMessage,
    ) -> Result<ObjectId, RuntimeError> {
        let prototype = self.heap.context(realm)?.native_error_prototypes[kind.index()].ok_or(
            RuntimeError::Invariant("realm has no native Error prototype"),
        )?;
        self.heap.retain_object(prototype)?;
        let mut prototype_owner = OwnedValueGuard::new(self, poisoned, JsValue::Object(prototype));
        let (state, prototype_owner) = prototype_owner.parts();
        let object = state.allocate_object_with_layout_with_poison(
            poisoned,
            Some(prototype),
            &[],
            Vec::new(),
            ObjectData::error,
        )?;
        let mut object_owner = OwnedValueGuard::new(state, poisoned, JsValue::Object(object));
        let (state, object_owner) = object_owner.parts();
        state.initialize_native_error_message(poisoned, object, message)?;
        state.release_owned_jsvalue(poisoned, prototype_owner.take().expect("prototype owner"))?;
        let JsValue::Object(object) = object_owner.take().expect("native Error owner") else {
            unreachable!("native Error factory allocated an object")
        };
        Ok(object)
    }

    /// Initialize the freshly allocated native Error before it is exposed.
    /// Keep the producer local so publication failures quarantine before any
    /// message/result/prototype owners can traverse partially retired state.
    fn initialize_native_error_message(
        &mut self,
        poisoned: &Cell<bool>,
        object: ObjectId,
        message: NativeErrorMessage,
    ) -> Result<(), RuntimeError> {
        let key = self
            .pinned_atoms
            .get(crate::engine::atom::pinned::PinnedAtom::Message);
        let string = self.heap.allocate_string(message.to_js_string()?)?;
        {
            let mut message_owner = OwnedValueGuard::new(self, poisoned, JsValue::String(string));
            let (state, message_owner) = message_owner.parts();
            let defined = state.define_raw_property_with_poison(
                poisoned,
                object,
                key,
                &PropertyDescriptor {
                    value: Some(RawValue::String(string)),
                    writable: Some(true),
                    enumerable: Some(false),
                    configurable: Some(true),
                    ..PropertyDescriptor::new()
                },
            )?;
            if !defined {
                return Err(RuntimeError::Invariant(
                    "native Error message definition was rejected",
                ));
            }
            // The accepted descriptor retained its own string edge. Surrender
            // the producer before the prototype temporary, preserving order.
            state.release_owned_jsvalue(poisoned, message_owner.take().expect("message owner"))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod publication_tests;

#[cfg(test)]
mod state_factory_tests {
    use super::*;
    use crate::engine::atom::{AtomIdx, pinned::PinnedAtom};
    use crate::engine::heap::{HeapError, PropertySlot, RawId};

    #[test]
    fn native_error_state_factory_owns_message_and_preserves_realm_class_and_utf16() {
        let runtime = Runtime::new();
        let context = runtime.new_context().unwrap();
        let mut message = NativeErrorMessage::new();
        message.push_bytes([0xed, 0xa0, 0x80, 0, b'x']);
        let mut state = runtime.0.state.borrow_mut();
        let prototype = state
            .heap
            .context(context.realm)
            .unwrap()
            .native_error_prototypes[NativeErrorKind::Type.index()]
        .unwrap();
        let object = state
            .new_native_error_without_backtrace_from_message(
                &runtime.0.poisoned,
                context.realm,
                NativeErrorKind::Type,
                message,
            )
            .unwrap();
        assert_eq!(state.heap.object_strong_count(object).unwrap(), 1);
        let data = state.heap.object(object).unwrap();
        assert_eq!(data.kind, ObjectKind::Error);
        let shape = state.heap.shape(data.shape).unwrap();
        assert_eq!(shape.prototype(), Some(prototype));
        let message_key = state.pinned_atoms.get(PinnedAtom::Message);
        let slot = shape.find(AtomIdx::from_raw(message_key.raw())).unwrap() as usize;
        assert!(shape.entries()[slot].flags.writable);
        assert!(!shape.entries()[slot].flags.enumerable);
        assert!(shape.entries()[slot].flags.configurable);
        assert!(
            shape
                .find(AtomIdx::from_raw(
                    state.pinned_atoms.get(PinnedAtom::Stack).raw()
                ))
                .is_none()
        );
        let PropertySlot::Data(RawValue::String(string)) = data.slots[slot] else {
            panic!("message slot")
        };
        assert_eq!(
            state
                .heap
                .string(string)
                .unwrap()
                .utf16_units()
                .collect::<Vec<_>>(),
            [0xd800]
        );
        state
            .release_owned_jsvalue(&runtime.0.poisoned, JsValue::Object(object))
            .unwrap();
        assert!(state.heap.object(object).is_err());
        assert!(state.heap.string(string).is_err());
        assert!(!runtime.0.deferred_references.has_pending());
        assert!(!runtime.0.poisoned.get());
    }

    #[test]
    fn state_factories_preserve_checked_prototype_retain_with_cached_empty_shapes() {
        let runtime = Runtime::new();
        let context = runtime.new_context().unwrap();
        let ordinary = runtime.new_ordinary_object_in_realm(context.realm).unwrap();
        let error = runtime
            .new_native_error_without_backtrace_from_message_jsvalue(
                context.realm,
                NativeErrorKind::Type,
                NativeErrorMessage::from_utf8("warm"),
            )
            .unwrap();
        runtime.release_jsvalue(error).unwrap();
        drop(ordinary);
        let mut state = runtime.0.state.borrow_mut();
        let ordinary_prototype = state.heap.context(context.realm).unwrap().object_prototype;
        let error_prototype = state
            .heap
            .context(context.realm)
            .unwrap()
            .native_error_prototypes[NativeErrorKind::Type.index()]
        .unwrap();
        for (prototype, error_factory) in [(ordinary_prototype, false), (error_prototype, true)] {
            let before = state.heap.object_strong_count(prototype).unwrap();
            state
                .heap
                .set_strong_count_for_test(RawId::Object(prototype), u32::MAX);
            let result = if error_factory {
                state.new_native_error_without_backtrace_from_message(
                    &runtime.0.poisoned,
                    context.realm,
                    NativeErrorKind::Type,
                    NativeErrorMessage::from_utf8("blocked"),
                )
            } else {
                state.new_ordinary_object_in_realm(&runtime.0.poisoned, context.realm)
            };
            state
                .heap
                .set_strong_count_for_test(RawId::Object(prototype), before);
            assert!(matches!(
                result,
                Err(RuntimeError::Heap(HeapError::Overflow { .. }))
            ));
        }
        assert!(!runtime.0.poisoned.get());
        assert!(!runtime.0.deferred_references.has_pending());
    }

    #[test]
    fn native_error_state_consumer_captures_materialized_native_frame_and_owns_stack() {
        use crate::engine::builtins::native::NativeFunctionId;
        let runtime = Runtime::new();
        let context = runtime.new_context().unwrap();
        let function = runtime
            .new_bound_native_function(
                &context.function_prototype().unwrap(),
                context.realm,
                NativeFunctionId::ActiveFrameProbe,
                0,
            )
            .unwrap();
        runtime
            .define_function_data_property(
                function.as_object(),
                "name",
                Value::String(
                    crate::engine::value::JsString::try_from_utf16([0xd800, 0, u16::from(b'x')])
                        .unwrap(),
                ),
                false,
                true,
            )
            .unwrap();
        let frame = runtime
            .push_native_active_frame(
                function.as_object().try_clone().unwrap(),
                context.realm,
                NativeFunctionId::ActiveFrameProbe,
                0,
                0,
            )
            .unwrap();
        {
            let mut state = runtime.0.state.borrow_mut();
            let object = state
                .new_native_error_from_message(
                    &runtime.0.poisoned,
                    context.realm,
                    NativeErrorKind::Type,
                    NativeErrorMessage::from_utf8("message"),
                )
                .unwrap();
            assert_eq!(state.heap.object_strong_count(object).unwrap(), 1);
            let data = state.heap.object(object).unwrap();
            let shape = state.heap.shape(data.shape).unwrap();
            let stack_slot = shape
                .find(AtomIdx::from_raw(
                    state.pinned_atoms.get(PinnedAtom::Stack).raw(),
                ))
                .unwrap() as usize;
            let PropertySlot::Data(RawValue::String(stack)) = data.slots[stack_slot] else {
                panic!("stack string")
            };
            let expected = crate::engine::value::JsString::try_from_utf16(
                "    at "
                    .encode_utf16()
                    .chain([0xd800])
                    .chain(" (native)\n".encode_utf16()),
            )
            .unwrap();
            assert_eq!(state.heap.string(stack).unwrap(), &expected);
            assert_eq!(state.heap.strong_count(RawId::String(stack)).unwrap(), 1);
            assert_eq!(data.slots.len(), 2);
            state
                .release_owned_jsvalue(&runtime.0.poisoned, JsValue::Object(object))
                .unwrap();
            assert!(state.heap.string(stack).is_err());
            assert!(state.heap.object(object).is_err());
            assert!(!runtime.0.deferred_references.has_pending());
            assert!(!runtime.0.poisoned.get());
        }
        frame.finish().unwrap();
    }

    #[test]
    fn native_error_state_consumer_defers_capture_for_existing_bytecode_frame() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        let bytecode = context
            .compile_with_filename("void 0;", "deferred.js")
            .unwrap();
        let function = runtime
            .new_bytecode_closure(context.realm, &bytecode)
            .unwrap();
        let frame = runtime
            .push_bytecode_active_frame(
                function.as_object().try_clone().unwrap(),
                bytecode,
                context.realm,
                false,
            )
            .unwrap();
        {
            let mut state = runtime.0.state.borrow_mut();
            let object = state
                .new_native_error_from_message(
                    &runtime.0.poisoned,
                    context.realm,
                    NativeErrorKind::Type,
                    NativeErrorMessage::from_utf8("message"),
                )
                .unwrap();
            let data = state.heap.object(object).unwrap();
            let shape = state.heap.shape(data.shape).unwrap();
            assert!(
                shape
                    .find(AtomIdx::from_raw(
                        state.pinned_atoms.get(PinnedAtom::Stack).raw()
                    ))
                    .is_none()
            );
            assert_eq!(data.slots.len(), 1);
            assert_eq!(state.heap.object_strong_count(object).unwrap(), 1);
            state
                .release_owned_jsvalue(&runtime.0.poisoned, JsValue::Object(object))
                .unwrap();
            assert!(!runtime.0.deferred_references.has_pending());
            assert!(!runtime.0.poisoned.get());
        }
        frame.finish().unwrap();
    }
}
