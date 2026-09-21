//! Iterator classification and snapshot kernels shared by both VM adapters.
use super::exception::runtime_error_to_vm_error;
use crate::engine::{
    api::{Error, runtime::Runtime},
    builtins::native::NativeFunctionId,
    heap::ObjectPayload,
    object::ObjectRef,
    value::JsValue,
};

pub(super) fn is_direct_native_target(
    runtime: &Runtime,
    value: &JsValue,
    expected: NativeFunctionId,
) -> Result<bool, Error> {
    let JsValue::Object(object) = value else {
        return Ok(false);
    };
    let state = runtime.0.state.borrow();
    let object = state
        .heap
        .object(*object)
        .map_err(|error| Error::internal(error.to_string()))?;
    Ok(matches!(
        &object.payload,
        ObjectPayload::NativeFunction { data, .. } if data.target == expected
    ))
}

/// Snapshot the exact values used by QuickJS's `js_append_enumerate`
/// fast branch. Named properties may be interleaved in our shape, so the
/// shared fast Array/Arguments storage reader reconstructs numeric order
/// rather than slicing physical slots.
pub(super) fn append_fast_array_values(
    runtime: &Runtime,
    source: &JsValue,
    next_method: &JsValue,
    builtin_values_probe: bool,
) -> Result<Option<Vec<JsValue>>, Error> {
    if !builtin_values_probe
        || !is_direct_native_target(runtime, next_method, NativeFunctionId::ArrayIteratorNext)?
    {
        return Ok(None);
    }
    let JsValue::Object(source) = source else {
        return Ok(None);
    };
    let source = ObjectRef::from_borrowed_handle(runtime.clone(), *source)
        .map_err(|error| runtime_error_to_vm_error(error.into()))?;
    let is_array = {
        let state = runtime.0.state.borrow();
        matches!(
            &state
                .heap
                .object(source.object_id())
                .map_err(|error| Error::internal(error.to_string()))?
                .payload,
            ObjectPayload::Array { .. }
        )
    };
    if !is_array {
        return Ok(None);
    }
    let fast_len = runtime
        .array_fast_len(&source)
        .map_err(runtime_error_to_vm_error)?;
    let Some(fast_len) = fast_len else {
        return Ok(None);
    };
    let (length, _) = runtime
        .array_length_state(&source)
        .map_err(runtime_error_to_vm_error)?;
    if length != fast_len {
        return Ok(None);
    }

    runtime
        .fast_array_like_values_jsvalue(&source, fast_len)
        .map_err(runtime_error_to_vm_error)
}

pub(super) fn missing_throw() -> Error {
    Error::new(
        crate::engine::api::ErrorKind::Type,
        "iterator does not have a throw method",
    )
}
