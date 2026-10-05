//! Integer-indexed selection and synchronous writes under current State.
//! Borrowed input owners remain with the consumer. A selected object conversion
//! is a fact, not a durable owner; the consumer publishes storage only after
//! canonical ToPrimitive selects an actual effect.

use super::*;
use crate::engine::api::error::NativeErrorMessage;
use crate::engine::heap::{RawValue, runtime::RuntimeState};
use crate::engine::object::property::{PropertyDefinitionError, PropertyDescriptor};
use std::cell::Cell;

pub(crate) enum StateTypedWrite {
    Complete(NativeConversion<bool>),
    Shared(SharedTypedWrite),
    Object(TypedWriteSelection),
}

/// An exact view and index chosen before conversion. The consumer must keep an
/// owner of `object` until completion. No buffer range survives a callback.
#[derive(Clone, Copy)]
pub(crate) struct TypedWriteSelection {
    pub object: ObjectId,
    pub index: Option<u64>,
    pub element: TypedArrayElementKind,
}

/// Owns only sendable backing storage and the selected write bytes. It must be
/// consumed after the State borrow ends; no runtime or JS edge is held here.
pub(crate) struct SharedTypedWrite {
    handle: crate::engine::heap::shared_memory::SharedBufferHandle,
    offset: u32,
    width: u8,
    bytes: [u8; 8],
}
impl SharedTypedWrite {
    pub(crate) fn write(self) -> Result<(), RuntimeError> {
        self.handle
            .write_word(self.offset, &self.bytes[..usize::from(self.width)])
            .map_err(crate::engine::builtins::buffer_access::shared_memory_runtime_error)
    }
}

impl TypedWriteSelection {
    /// Conversion has completed. Re-admit the selected view and current backing
    /// bounds; detached/shrunken/out-of-range writes still complete successfully.
    pub(crate) fn finish_in_state(
        self,
        state: &mut RuntimeState,
        bytes: [u8; 8],
    ) -> Result<StateTypedWrite, RuntimeError> {
        let snapshot = state.typed_array_snapshot_in_state(self.object)?;
        if snapshot.element != self.element {
            return Err(RuntimeError::Invariant(
                "selected TypedArray changed element kind",
            ));
        }
        let Some(index) = self.index else {
            return Ok(StateTypedWrite::Complete(NativeConversion::Value(true)));
        };
        Ok(
            match ordinary_typed_array_word_in_heap(&mut state.heap, snapshot, index, Some(&bytes))?
            {
                OrdinaryTypedWord::Missing | OrdinaryTypedWord::Word(_) => {
                    StateTypedWrite::Complete(NativeConversion::Value(true))
                }
                OrdinaryTypedWord::Shared => {
                    let buffer = state.heap.buffer_state(snapshot.buffer)?;
                    let Some((offset, width)) = typed_array_word_range(snapshot, buffer, index)?
                    else {
                        return Ok(StateTypedWrite::Complete(NativeConversion::Value(true)));
                    };
                    StateTypedWrite::Shared(SharedTypedWrite {
                        handle: state
                            .heap
                            .clone_shared_array_buffer_handle(snapshot.buffer)?,
                        offset: u32::try_from(offset).map_err(|_| {
                            RuntimeError::Invariant("shared typed byte offset overflowed u32")
                        })?,
                        width: width as u8,
                        bytes,
                    })
                }
            },
        )
    }

    fn convert_in_state(
        self,
        state: &mut RuntimeState,
        poisoned: &Cell<bool>,
        realm: Option<ContextId>,
        value: &JsValue,
    ) -> Result<StateTypedWrite, RuntimeError> {
        if matches!(value, JsValue::Object(_)) {
            return Ok(StateTypedWrite::Object(self));
        }
        match state.encode_typed_primitive_in_state(poisoned, realm, self.element, value)? {
            NativeConversion::Value(bytes) => self.finish_in_state(state, bytes),
            NativeConversion::Throw(value) => {
                Ok(StateTypedWrite::Complete(NativeConversion::Throw(value)))
            }
        }
    }
}

impl RuntimeState {
    pub(crate) fn typed_array_snapshot_in_state(
        &self,
        object: ObjectId,
    ) -> Result<TypedArraySnapshot, RuntimeError> {
        typed_array_snapshot_from_payload(&self.heap.object(object)?.payload).ok_or(
            RuntimeError::Invariant("integer-indexed access reached a non-TypedArray"),
        )
    }

    /// Metadata-only presence. It does not read bytes, allocate a BigInt, obtain
    /// a shared-memory lock, or materialize an owning descriptor.
    pub(crate) fn typed_array_index_exists_in_state(
        &self,
        object: ObjectId,
        index: u64,
    ) -> Result<bool, RuntimeError> {
        let snapshot = self.typed_array_snapshot_in_state(object)?;
        typed_array_word_range(snapshot, self.heap.buffer_state(snapshot.buffer)?, index)
            .map(|range| range.is_some())
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn prepare_typed_array_set_in_state(
        &mut self,
        poisoned: &Cell<bool>,
        realm: Option<ContextId>,
        object: ObjectId,
        atom: crate::engine::atom::Atom,
        value: &JsValue,
        receiver: &JsValue,
    ) -> Result<Option<StateTypedWrite>, RuntimeError> {
        let Some(numeric) = self.typed_array_canonical_numeric_index(atom)? else {
            return Ok(None);
        };
        let snapshot = self.typed_array_snapshot_in_state(object)?;
        let index = match numeric {
            CanonicalNumericIndex::Valid(index) => Some(index),
            CanonicalNumericIndex::Invalid => None,
        };
        if !matches!(receiver, JsValue::Object(id) if *id == object) {
            // A valid index follows ordinary receiver Define; invalid indices
            // ignore the input entirely, including Symbol/BigInt conversion.
            if let Some(index) = index
                && self.typed_array_index_exists_in_state(object, index)?
            {
                return Ok(None);
            }
            return Ok(Some(StateTypedWrite::Complete(NativeConversion::Value(
                true,
            ))));
        }
        TypedWriteSelection {
            object,
            index,
            element: snapshot.element,
        }
        .convert_in_state(self, poisoned, realm, value)
        .map(Some)
    }

    pub(crate) fn prepare_typed_array_define_in_state(
        &mut self,
        poisoned: &Cell<bool>,
        realm: Option<ContextId>,
        object: ObjectId,
        atom: crate::engine::atom::Atom,
        descriptor: &PropertyDescriptor<RawValue>,
    ) -> Result<Option<StateTypedWrite>, RuntimeError> {
        if descriptor.is_data_descriptor() && descriptor.is_accessor_descriptor() {
            return Err(PropertyDefinitionError::InvalidDescriptor.into());
        }
        let Some(numeric) = self.typed_array_canonical_numeric_index(atom)? else {
            return Ok(None);
        };
        let CanonicalNumericIndex::Valid(index) = numeric else {
            return Ok(Some(StateTypedWrite::Complete(NativeConversion::Value(
                false,
            ))));
        };
        if descriptor.is_accessor_descriptor()
            || descriptor.writable == Some(false)
            || descriptor.enumerable == Some(false)
            || descriptor.configurable == Some(false)
            || !self.typed_array_index_exists_in_state(object, index)?
        {
            return Ok(Some(StateTypedWrite::Complete(NativeConversion::Value(
                false,
            ))));
        }
        let Some(value) = descriptor.value.as_ref() else {
            return Ok(Some(StateTypedWrite::Complete(NativeConversion::Value(
                true,
            ))));
        };
        let value = JsValue::from_raw(value.clone()).ok_or(RuntimeError::Invariant(
            "typed descriptor value is uninitialized",
        ))?;
        let element = self.typed_array_snapshot_in_state(object)?.element;
        TypedWriteSelection {
            object,
            index: Some(index),
            element,
        }
        .convert_in_state(self, poisoned, realm, &value)
        .map(Some)
    }

    pub(crate) fn encode_typed_primitive_in_state(
        &mut self,
        poisoned: &Cell<bool>,
        realm: Option<ContextId>,
        element: TypedArrayElementKind,
        value: &JsValue,
    ) -> Result<NativeConversion<[u8; 8]>, RuntimeError> {
        match self.typed_array_convert_primitive_element_in_state(element, value) {
            Ok(bytes) => Ok(NativeConversion::Value(bytes)),
            Err(RuntimeError::Engine(error)) => {
                let Some(realm) = realm else {
                    return Err(RuntimeError::Engine(error));
                };
                let Some(kind) = NativeErrorKind::from_javascript_error(error.kind()) else {
                    return Err(RuntimeError::Engine(error));
                };
                let message = error
                    .native_message()
                    .cloned()
                    .unwrap_or_else(|| NativeErrorMessage::from_utf8(error.message()));
                self.new_native_error_from_message(poisoned, realm, kind, message)
                    .map(|object| NativeConversion::Throw(JsValue::Object(object)))
            }
            Err(error) => Err(error),
        }
    }

    pub(crate) fn typed_array_convert_primitive_element_in_state(
        &self,
        element: TypedArrayElementKind,
        value: &JsValue,
    ) -> Result<[u8; 8], RuntimeError> {
        if element.is_bigint() {
            let bigint = match value {
                JsValue::ShortBigInt(value) => crate::engine::value::bigint::JsBigInt::from(*value),
                JsValue::BigInt(id) => self.heap.bigint(*id)?.clone(),
                JsValue::Bool(value) => {
                    crate::engine::value::bigint::JsBigInt::from(i64::from(*value))
                }
                JsValue::String(id) => typed_array_parse_primitive_bigint(self.heap.string(*id)?)?,
                _ => {
                    return Err(RuntimeError::Engine(Error::new(
                        ErrorKind::Type,
                        "cannot convert to bigint",
                    )));
                }
            };
            return typed_array_encode_bigint(&bigint);
        }
        if matches!(value, JsValue::Object(_)) {
            return Err(RuntimeError::Engine(Error::new(
                ErrorKind::Internal,
                "object ToPrimitive requires an execution context",
            )));
        }
        crate::engine::vm::to_number_jsvalue_in_state(self, value)
            .map(|number| typed_array_encode_number(element, number))
            .map_err(RuntimeError::Engine)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::object::property::PropertyDescriptor;

    fn complete(result: StateTypedWrite) -> bool {
        let StateTypedWrite::Complete(NativeConversion::Value(value)) = result else {
            panic!("expected synchronous boolean");
        };
        value
    }

    #[test]
    fn state_typed_set_and_define_preserve_index_receiver_and_conversion_order() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        let Value::Object(view) = context.eval("new Uint8Array(1)").unwrap() else {
            panic!()
        };
        let other = runtime.new_object(None).unwrap();
        let zero = runtime.intern_property_key("0").unwrap();
        let negative_zero = runtime.intern_property_key("-0").unwrap();
        let ordinary = runtime.intern_property_key("01").unwrap();
        let symbol = runtime
            .into_jsvalue(context.eval("Symbol()").unwrap())
            .unwrap();
        let baseline = std::rc::Rc::strong_count(&runtime.0);
        let mut state = runtime.0.state.borrow_mut();
        let receiver = JsValue::Object(view.object_id());
        assert!(complete(
            state
                .prepare_typed_array_set_in_state(
                    &runtime.0.poisoned,
                    Some(context.realm),
                    view.object_id(),
                    zero.atom(),
                    &JsValue::Int(257),
                    &receiver
                )
                .unwrap()
                .unwrap()
        ));
        assert!(
            state
                .prepare_typed_array_set_in_state(
                    &runtime.0.poisoned,
                    Some(context.realm),
                    view.object_id(),
                    ordinary.atom(),
                    &symbol,
                    &receiver
                )
                .unwrap()
                .is_none()
        );
        assert!(
            state
                .prepare_typed_array_set_in_state(
                    &runtime.0.poisoned,
                    Some(context.realm),
                    view.object_id(),
                    zero.atom(),
                    &symbol,
                    &JsValue::Object(other.object_id())
                )
                .unwrap()
                .is_none()
        );
        assert!(complete(
            state
                .prepare_typed_array_set_in_state(
                    &runtime.0.poisoned,
                    Some(context.realm),
                    view.object_id(),
                    negative_zero.atom(),
                    &symbol,
                    &JsValue::Object(other.object_id())
                )
                .unwrap()
                .unwrap()
        ));
        // Define rejects invalid indices before trying to convert Symbol.
        assert!(!complete(
            state
                .prepare_typed_array_define_in_state(
                    &runtime.0.poisoned,
                    Some(context.realm),
                    view.object_id(),
                    negative_zero.atom(),
                    &PropertyDescriptor {
                        value: Some(symbol.as_raw()),
                        ..PropertyDescriptor::new()
                    }
                )
                .unwrap()
                .unwrap()
        ));
        // Set on the same view still converts an invalid index and throws.
        let StateTypedWrite::Complete(NativeConversion::Throw(thrown)) = state
            .prepare_typed_array_set_in_state(
                &runtime.0.poisoned,
                Some(context.realm),
                view.object_id(),
                negative_zero.atom(),
                &symbol,
                &receiver,
            )
            .unwrap()
            .unwrap()
        else {
            panic!()
        };
        state
            .release_owned_jsvalue(&runtime.0.poisoned, thrown)
            .unwrap();
        assert_eq!(
            state.try_define_own_property_in_state(
                &runtime.0.poisoned,
                view.object_id(),
                zero.atom(),
                &PropertyDescriptor {
                    value: Some(RawValue::Int(258)),
                    ..PropertyDescriptor::new()
                }
            ),
            Ok(Some(true))
        );
        assert_eq!(std::rc::Rc::strong_count(&runtime.0), baseline);
        assert!(!runtime.0.deferred_references.has_pending());
        state
            .release_owned_jsvalue(&runtime.0.poisoned, symbol)
            .unwrap();
        drop(state);
        assert_eq!(
            runtime.typed_array_read_index(&view, 0).unwrap(),
            Some(Value::Int(2))
        );
    }

    #[test]
    fn shared_typed_write_services_selected_bytes_after_state_borrow() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        let Value::Object(view) = context
            .eval("new Uint16Array(new SharedArrayBuffer(2))")
            .unwrap()
        else {
            panic!()
        };
        let key = runtime.intern_property_key("0").unwrap();
        let result = {
            let mut state = runtime.0.state.borrow_mut();
            state
                .prepare_typed_array_define_in_state(
                    &runtime.0.poisoned,
                    Some(context.realm),
                    view.object_id(),
                    key.atom(),
                    &PropertyDescriptor {
                        value: Some(RawValue::Int(65539)),
                        ..PropertyDescriptor::new()
                    },
                )
                .unwrap()
                .unwrap()
        };
        let StateTypedWrite::Shared(write) = result else {
            panic!("expected shared service");
        };
        assert!(runtime.0.state.try_borrow_mut().is_ok());
        write.write().unwrap();
        assert_eq!(
            runtime.typed_array_read_index(&view, 0).unwrap(),
            Some(Value::Int(3))
        );
    }

    #[test]
    fn object_typed_selection_has_no_owner_and_rechecks_resize_after_conversion() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        let Value::Object(view) = context
            .eval("globalThis.b=new ArrayBuffer(1,{maxByteLength:2}); new Uint8Array(b)")
            .unwrap()
        else {
            panic!()
        };
        let value = runtime
            .into_jsvalue(
                context
                    .eval("({valueOf(){b.resize(0); return 257}})")
                    .unwrap(),
            )
            .unwrap();
        let key = runtime.intern_property_key("0").unwrap();
        let baseline = std::rc::Rc::strong_count(&runtime.0);
        let selection = {
            let mut state = runtime.0.state.borrow_mut();
            let count = state.heap.object_strong_count(view.object_id()).unwrap();
            let StateTypedWrite::Object(selection) = state
                .prepare_typed_array_set_in_state(
                    &runtime.0.poisoned,
                    Some(context.realm),
                    view.object_id(),
                    key.atom(),
                    &value,
                    &JsValue::Object(view.object_id()),
                )
                .unwrap()
                .unwrap()
            else {
                panic!()
            };
            assert_eq!(state.heap.object_strong_count(view.object_id()), Ok(count));
            assert_eq!(std::rc::Rc::strong_count(&runtime.0), baseline);
            selection
        };
        let NativeConversion::Value(bytes) = super::super::element::ElementStep::start(
            &runtime,
            context.realm,
            selection.element,
            value,
        )
        .unwrap()
        .finish_sync(&runtime, context.realm)
        .unwrap() else {
            panic!()
        };
        assert!(matches!(
            runtime.finish_selected_typed_write(selection, bytes),
            Ok(NativeConversion::Value(true))
        ));
        assert!(
            !runtime
                .0
                .state
                .borrow()
                .typed_array_index_exists_in_state(view.object_id(), 0)
                .unwrap()
        );
        assert_eq!(runtime.typed_array_read_index(&view, 0).unwrap(), None);
    }
}
